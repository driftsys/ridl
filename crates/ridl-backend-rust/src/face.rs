//! The generated interaction face (Lane M stage M3, Tasks 2 and 3,
//! `docs/design/interaction-face.md`; the approved design is
//! `docs/archive/2026-09-16-interaction-face-v0-design.md` §6).
//!
//! For each named interface this module emits one `pub mod`, named after the
//! interface, holding what the design note's §8 calls the face, as story
//! E11.21 reshaped it (`docs/archive/2026-09-25-async-face-design.md`, notes
//! F-2 to F-7, F-10 and F-11):
//!
//! - `Client<P>`, the consumer face, generic over exactly the ports the
//!   interface's own interactions need and no others (RA-19): `SignalReader`
//!   when it declares a signal, `EventSource` when it declares an event,
//!   `Caller` when it declares a command or a query, plus `Clock` for a call's
//!   deadline and `Wakeable` for a call's or an event's wait. A signal read
//!   returns its sample; a command, a query and `next_event` each return a
//!   named future;
//! - one future type per command and per query, `<Name>Call`, which sends
//!   when the method is called, waits for a slot while the port is busy,
//!   resolves on the outcome or at the member's bound, and forgets its call
//!   when it leaves the waiting phase, on drop included; and `NextEvent`,
//!   the event future;
//! - `Publisher<W>`, the provider face for signals and events, over
//!   `SignalWriter` and `EventSink` on the same rule;
//! - `Provider`, the trait the application implements, with one method per
//!   command and query;
//! - `serve`, which registers the interface's calls with the handler and
//!   returns the future that settles every claim, and resolves only when the
//!   handler port fails;
//! - `blocking`, under the emitted crate's `std` feature, when the interface
//!   declares an event, a command or a query: a `Client` that is the async
//!   one with a timeout per client, each waiting method `block_on` over the
//!   async method's future, and, when the interface declares a command or a
//!   query, a `serve` that is `block_on` over `serve` and returns `Ok(())`
//!   at its timeout;
//! - the internal poll face those futures are built over, `pub(crate)`: one
//!   `Copy` correlation newtype per command and per query,
//!   `<Name>Correlation`; `send_<name>`, `poll_<name>_ack`,
//!   `poll_<name>_reply` and `poll_next_event` over a bare port; and
//!   `dispatch`, the one-pass step that routes a claim, checks the argument
//!   bytes, evaluates `require`, calls the provider, evaluates a query's
//!   `ensure`, and settles.
//!
//! An item whose interface declares nothing for it is not emitted: a
//! signal-only interface gets a `Client` and a `Publisher` and no `Provider`,
//! `dispatch` or `serve`, because no call can reach it.
//!
//! RA-20, as note F-15 restates it: generated code contains no thread, socket
//! or timer, and no port waits; a face may return a future, and that future
//! never blocks. Each future's `poll` registers its interest with the port,
//! then reads the port — a call or event future once, `Serve` until the
//! handler has no claim waiting — and returns; what waits is the executor or
//! the frame loop that polls it, or `ridl_rt::task::block_on` under the
//! `blocking` module, which is the library's and not generated. No method is
//! bounded on `CoherentSignals`.
//!
//! Since E11.14 the face is reached from [`crate::generate_pipeline`], which
//! is what `ridl build --emit rust` calls, as well as from
//! [`crate::generate_face`]. It is not reached from [`crate::generate`],
//! which still emits no face.
//!
//! The emitter is split by section. This file holds the per-interface walk,
//! the consumer face (`Client` and the `Event` enum), the provider face
//! (`Publisher` and `Provider`) and the pieces the sections share. The
//! submodules hold the rest: `futures` the named futures, `poll` the internal
//! poll face, `dispatch` the one-pass step, `serve` the `serve` function and
//! its future, and `blocking` the `blocking` module.

use crate::descriptors::{
    declared_name, interactions, query_param_type, query_reply_type, single_param_type,
};
use crate::{GenerateError, camel_of, declared, ident, snake_of, type_path};
use proc_macro2::{Ident, Literal, TokenStream};
use quote::quote;
use ridl_ir::codegen::v1;

mod blocking;
mod dispatch;
mod futures;
mod poll;
mod serve;

use self::blocking::blocking;
use self::dispatch::dispatch;
use self::futures::futures;
use self::poll::plumbing;
use self::serve::serve;

/// The face module for every named interface of the lowered model, in source
/// order.
///
/// The model's `interfaces` list is `Package::shapes()` order, and a service's
/// inline shape is skipped, for the same two reasons the descriptor layer
/// gives.
pub(crate) fn interface_items(model: &v1::Model) -> Result<Vec<TokenStream>, GenerateError> {
    let mut items = Vec::new();
    for interface in &model.interfaces {
        if declared_name(interface).is_none() {
            continue;
        }
        if let Some(module) = one_interface(interface)? {
            items.push(module);
        }
    }
    Ok(items)
}

/// One interaction of the four kinds the face covers, with the ordinal, the
/// method name and the descriptor type the emitter needs for it.
struct Member<'a> {
    /// The interaction's ordinal, as a `u32` literal.
    ordinal: TokenStream,
    /// The Rust method name: the snake case of the declared name.
    method: Ident,
    /// The interaction's descriptor type, `<Interface><Member>`.
    descriptor: TokenStream,
    /// The declared name, for a generated doc comment.
    declared: &'a str,
    /// The pinned CamelCase of the declared name, which the correlation
    /// newtype and the event enum's variant are spelled with.
    camel: &'a str,
}

/// One command or query, with the declared argument name and type the face
/// names directly, and a query's reply type.
struct Call<'a> {
    member: Member<'a>,
    /// The declared parameter's name, in snake case. It is the name the client
    /// method and the `Provider` method take, so a reader of the generated
    /// code sees the name the ridl source used.
    arg: Ident,
    arg_type: &'a str,
    /// The reply type, present on a query and absent on a command.
    reply_type: Option<&'a str>,
}

/// The face module of one interface, or `None` when the interface declares
/// nothing the face carries. Reachable from the crate for the pipeline's
/// per-interface walk (E11.14 decision 2).
pub(crate) fn one_interface(
    interface: &v1::Interface,
) -> Result<Option<TokenStream>, GenerateError> {
    let iface_name = declared_name(interface).unwrap_or_default();
    let iface = ident(iface_name);
    let module = ident(interface_snake(interface));

    // Payload type names are kept beside each member, because the face names
    // the declared type directly (M3 emits no induced argument struct).
    let mut signals: Vec<(Member, &str)> = Vec::new();
    let mut events: Vec<(Member, &str)> = Vec::new();
    let mut commands: Vec<Call> = Vec::new();
    let mut queries: Vec<Call> = Vec::new();

    for (ordinal, interaction) in interactions(interface) {
        let name = declared(interaction.name.as_ref());
        let member = Member {
            ordinal: {
                let value = Literal::u32_suffixed(ordinal);
                quote! { ::ridl_rt::contract::Ordinal(#value) }
            },
            method: ident(snake_of(interaction.name.as_ref())),
            descriptor: {
                let ident = ident(&format!("{iface}{}", camel_of(interaction.name.as_ref())));
                quote! { super::#ident }
            },
            declared: name,
            camel: camel_of(interaction.name.as_ref()),
        };
        match interaction.shape.as_ref() {
            Some(v1::interaction::Shape::Signal(signal)) => {
                signals.push((member, payload_reference(signal.payload.as_ref())));
            }
            Some(v1::interaction::Shape::Event(event)) => {
                events.push((member, payload_reference(event.payload.as_ref())));
            }
            Some(v1::interaction::Shape::Command(command)) => {
                commands.push(Call {
                    arg_type: single_param_type(command, name)?,
                    arg: single_param_name(&command.params),
                    member,
                    reply_type: None,
                });
            }
            Some(v1::interaction::Shape::Query(query)) => {
                queries.push(Call {
                    arg_type: query_param_type(query, name)?,
                    arg: single_param_name(&query.params),
                    member,
                    reply_type: Some(query_reply_type(query, name)?),
                });
            }
            // A `fixed` is provisioned, not interacted with, so the MVP's face
            // carries no method for it; its descriptor is emitted all the
            // same.
            _ => {}
        }
    }

    let mut body: Vec<TokenStream> = Vec::new();
    body.extend(correlations(&commands, &queries));
    if !signals.is_empty() || !events.is_empty() || !commands.is_empty() || !queries.is_empty() {
        body.push(client(
            &iface, iface_name, &signals, &events, &commands, &queries,
        ));
    }
    if !events.is_empty() {
        body.push(event_enum(iface_name, &events));
    }
    body.extend(futures(&iface, iface_name, &events, &commands, &queries));
    body.extend(plumbing(&iface, iface_name, &events, &commands, &queries));
    if !signals.is_empty() || !events.is_empty() {
        body.push(publisher(&iface, iface_name, &signals, &events));
    }
    if !commands.is_empty() || !queries.is_empty() {
        body.push(provider(iface_name, &commands, &queries));
        body.push(dispatch(&iface, iface_name, &commands, &queries));
        body.push(serve(&iface, iface_name, &commands, &queries));
    }
    body.extend(blocking(iface_name, &signals, &events, &commands, &queries));

    if body.is_empty() {
        return Ok(None);
    }

    let module_doc = format!("The generated interaction face of interface `{iface_name}`.");
    Ok(Some(quote! {
        #[doc = #module_doc]
        pub mod #module {
            #(#body)*
        }
    }))
}

// ---------------------------------------------------------------------------
// The consumer face.
// ---------------------------------------------------------------------------

/// The name of one call's correlation newtype, `<Name>Correlation`.
fn correlation_type(call: &Call) -> Ident {
    ident(&format!("{}Correlation", call.member.camel))
}

/// One `Copy` correlation newtype per command and per query, `pub(crate)`
/// since the public surface became the futures (the async face design, note
/// F-12).
///
/// The newtype is the face's, not the port's: `ridl_rt::port::Correlation`
/// stays untyped, because which interaction a correlation belongs to is a
/// payload-shaped fact and a port never carries one (ADR-0023 decision 4's
/// 2026-09-20 amendment). What the newtype buys is that a query's correlation
/// cannot be passed to `poll_<name>_ack` and a command's cannot be passed to
/// `poll_<name>_reply`.
fn correlations(commands: &[Call], queries: &[Call]) -> Vec<TokenStream> {
    let mut items = Vec::new();
    for (call, kind) in commands
        .iter()
        .map(|c| (c, "command"))
        .chain(queries.iter().map(|q| (q, "query")))
    {
        let name = correlation_type(call);
        let doc = format!(
            "Identifies one sent {kind} `{}` to its caller. It is returned by \
             the internal send and accepted by that call's own outcome read, \
             and by no other.",
            call.member.declared
        );
        items.push(quote! {
            #[doc = #doc]
            #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
            pub(crate) struct #name(pub ::ridl_rt::port::Correlation);
        });
    }
    items
}

/// The port bounds of `Client<P>`: exactly the traits the interface's
/// interactions need (RA-19). The blocking client repeats them, because it
/// is the async client and its timeout and nothing more.
fn client_bounds(
    signals: &[(Member, &str)],
    events: &[(Member, &str)],
    commands: &[Call],
    queries: &[Call],
) -> Vec<TokenStream> {
    let has_calls = !commands.is_empty() || !queries.is_empty();
    let mut bounds: Vec<TokenStream> = Vec::new();
    if !signals.is_empty() {
        bounds.push(quote! { ::ridl_rt::port::SignalReader });
    }
    if !events.is_empty() {
        bounds.push(quote! { ::ridl_rt::port::EventSource });
    }
    if has_calls {
        bounds.push(quote! { ::ridl_rt::port::Caller });
        // A call's future measures the member's bound on the port's clock
        // (the async face design, note F-2).
        bounds.push(quote! { ::ridl_rt::port::Clock });
    }
    if has_calls || !events.is_empty() {
        // A call's future waits on a slot and on its outcome, and the event
        // future waits on the queue; an event has no response bound, so an
        // event-only client needs no `Clock` (note F-10).
        bounds.push(quote! { ::ridl_rt::port::Wakeable });
    }
    bounds
}

fn client(
    iface: &Ident,
    iface_name: &str,
    signals: &[(Member, &str)],
    events: &[(Member, &str)],
    commands: &[Call],
    queries: &[Call],
) -> TokenStream {
    let number = interface_number(iface);
    let bounds = client_bounds(signals, events, commands, queries);

    let mut methods: Vec<TokenStream> = Vec::new();

    for (member, payload) in signals {
        let method = &member.method;
        let ordinal = &member.ordinal;
        let descriptor = &member.descriptor;
        let path = ty(payload);
        let buffer = payload_buffer(payload);
        let doc = format!(
            "Reads signal `{}` and returns its value with the provenance, the \
             freshness and the envelope the runtime resolved. Before the \
             first publication, and when the channel is invalidated with no \
             prior publication, there is no payload to check and the value \
             is the channel's init value, under `Provenance::Init` or \
             `Provenance::Invalid(Cause::Declared)` respectively (ridl §4.4, \
             §4.5). A payload that fails its check is reported as \
             `Provenance::Invalid` with the detection, and the value is the \
             channel's init value.",
            member.declared
        );
        methods.push(quote! {
            #[doc = #doc]
            pub fn #method(
                &self,
            ) -> ::core::result::Result<
                ::ridl_rt::sample::Sample<#path>,
                ::ridl_rt::port::ReadError,
            > {
                let mut buf = #buffer;
                let raw = self.port.read(#number, #ordinal, &mut buf)?;
                let (value, provenance) = match raw.provenance {
                    ::ridl_rt::sample::Provenance::Init
                    | ::ridl_rt::sample::Provenance::Invalid(
                        ::ridl_rt::sample::Cause::Declared,
                    ) if raw.len == 0 => {
                        (<#descriptor as ::ridl_rt::contract::Signal>::init(), raw.provenance)
                    }
                    _ => match ::ridl_rt::payload::Ref::<#path, super::Wire>::verify(
                        &buf[..raw.len],
                    ) {
                        Ok(checked) => (checked.decode(), raw.provenance),
                        Err(error) => (
                            <#descriptor as ::ridl_rt::contract::Signal>::init(),
                            ::ridl_rt::sample::Provenance::Invalid(
                                ::ridl_rt::sample::Cause::Detected(match error {
                                    ::ridl_rt::payload::VerifyError::Contract(violation) => {
                                        ::ridl_rt::sample::Detection::InvalidValue(violation)
                                    }
                                    _ => ::ridl_rt::sample::Detection::Corrupt,
                                }),
                            ),
                        ),
                    },
                };
                Ok(::ridl_rt::sample::Sample {
                    value,
                    provenance,
                    freshness: raw.freshness,
                    envelope: raw.envelope,
                })
            }
        });
    }

    for (member, _) in events {
        let method = ident(&format!("subscribe_{}", member.method));
        let ordinal = &member.ordinal;
        let doc = format!("Starts delivery of event `{}`.", member.declared);
        methods.push(quote! {
            #[doc = #doc]
            pub fn #method(
                &mut self,
            ) -> ::core::result::Result<(), ::ridl_rt::port::SubscribeError> {
                self.port.subscribe(#number, &[#ordinal])
            }
        });
    }

    if !events.is_empty() {
        let doc = format!(
            "Takes the next occurrence of any subscribed event of interface \
             `{iface_name}`, routed to its variant by ordinal, as a future: it \
             resolves when an occurrence is waiting and is `Pending` while \
             none is. One method serves every event, because the payload type \
             is not known until the occurrence's ordinal is read. The future \
             holds this client's port until it is dropped.\n\nThe interface \
             number is checked before the ordinal, for the reason `serve` \
             checks it: a port is attached to a whole catalog, ordinals \
             restart at 1 in each interface, and an occurrence of a sibling \
             interface at the same ordinal would otherwise be decoded as this \
             interface's payload. Such an occurrence is reported as \
             `Contract::UnknownInteraction`; `EventSource::next` has already \
             consumed it, so this face cannot hand it back to the interface it \
             belongs to. Subscribe on a port this interface owns."
        );
        methods.push(quote! {
            #[doc = #doc]
            pub fn next_event(&mut self) -> NextEvent<'_, P> {
                NextEvent { port: &mut self.port }
            }
        });
    }

    for call in commands {
        methods.push(call_method(call, "command"));
    }

    for call in queries {
        methods.push(call_method(call, "query"));
    }

    let doc = format!(
        "The consumer face of interface `{iface_name}`, generic over exactly \
         the ports the interface's interactions need."
    );
    quote! {
        #[doc = #doc]
        pub struct Client<P: #(#bounds)+*> {
            port: P,
        }

        impl<P: #(#bounds)+*> Client<P> {
            /// Binds the face to a port. The port is held by value: pass a
            /// handle, or a `&mut` borrow of one.
            pub fn new(port: P) -> Self {
                Client { port }
            }

            #(#methods)*
        }
    }
}

/// The name of one call's future type, `<Name>Call`.
fn future_type(call: &Call) -> Ident {
    ident(&format!("{}Call", call.member.camel))
}

/// The name of one call's phase enum, `<Name>Phase`, private to the module.
fn phase_type(call: &Call) -> Ident {
    ident(&format!("{}Phase", call.member.camel))
}

/// The name of one call's internal send, `send_<name>`.
fn send_fn(call: &Call) -> Ident {
    ident(&format!("send_{}", call.member.method))
}

/// The name of one call's internal outcome read: `poll_<name>_ack` for a
/// command, `poll_<name>_reply` for a query.
fn read_fn(call: &Call) -> Ident {
    let suffix = if call.reply_type.is_some() {
        "reply"
    } else {
        "ack"
    };
    ident(&format!("poll_{}_{suffix}", call.member.method))
}

/// One public call method: sends when it is called, and returns the call's
/// future with the result of that attempt inside it (the async face design,
/// note F-4).
///
/// The parameter keeps the ridl name, so a reader sees the name the source
/// used, and the body rebinds it to `__arg` on its first line, before any
/// local is declared. A ridl identifier cannot start with an underscore
/// (`ridl check` refuses one), so `__arg`, like the codec's `__p` and `__v`,
/// cannot be the name of a parameter, and the locals that follow — `deadline`,
/// `phase`, `correlation`, `error` — cannot shadow the argument whatever it is
/// called. The futures' `poll` binds the same `__arg` in place of the ridl
/// name, for the same reason, and the internal send names its port parameter
/// `__port`. `dispatch` binds the decoded argument as `__arg` too.
fn call_method(call: &Call, kind: &str) -> TokenStream {
    let member = &call.member;
    let method = &member.method;
    let descriptor = &member.descriptor;
    let arg = &call.arg;
    let path = ty(call.arg_type);
    let future = future_type(call);
    let phase = phase_type(call);
    let send = send_fn(call);
    let doc = format!(
        "Sends {kind} `{}` and returns its future. The call is sent when this \
         method runs, not when the future is first polled, and the future \
         resolves on the outcome. A `require` clause that fails, or a send \
         failure other than `SendError::Busy`, is a future that is ready with \
         `ClientError::Send` and sends nothing. `SendError::Busy` is a future \
         that waits for a free slot and sends on a later poll.\n\nThe call's \
         bound is the member's `max`, measured from the port's clock when \
         this method runs; a member with no `max` waits without a bound. The \
         future holds this client's port until it is dropped.",
        member.declared
    );
    quote! {
        #[doc = #doc]
        pub fn #method(&mut self, #arg: #path) -> #future<'_, P> {
            let __arg = #arg;
            let deadline = <#descriptor as ::ridl_rt::contract::Interaction>::MEMBER
                .call_deadline()
                .map(|max| ::ridl_rt::sample::Timestamp(
                    self.port.now().0.saturating_add(max.0),
                ));
            let phase = match #send(&mut self.port, &__arg) {
                Ok(correlation) => #phase::Waiting(correlation),
                Err(::ridl_rt::port::SendError::Busy) => #phase::Unsent(__arg),
                Err(error) => #phase::Failed(error),
            };
            #future {
                port: &mut self.port,
                phase,
                deadline,
            }
        }
    }
}

/// The occurrence enum one `next_event` routes into.
fn event_enum(iface_name: &str, events: &[(Member, &str)]) -> TokenStream {
    let variants = events.iter().map(|(member, payload)| {
        let variant = ident(member.camel);
        let path = ty(payload);
        let doc = format!("An occurrence of event `{}`.", member.declared);
        quote! {
            #[doc = #doc]
            #variant(::ridl_rt::sample::Occurrence<#path>)
        }
    });
    let doc = format!("One occurrence of an event of interface `{iface_name}`.");
    quote! {
        #[doc = #doc]
        pub enum Event {
            #(#variants),*
        }
    }
}

// ---------------------------------------------------------------------------
// The provider face.
// ---------------------------------------------------------------------------

fn publisher(
    iface: &Ident,
    iface_name: &str,
    signals: &[(Member, &str)],
    events: &[(Member, &str)],
) -> TokenStream {
    let number = interface_number(iface);
    let mut bounds: Vec<TokenStream> = Vec::new();
    if !signals.is_empty() {
        bounds.push(quote! { ::ridl_rt::port::SignalWriter });
    }
    if !events.is_empty() {
        bounds.push(quote! { ::ridl_rt::port::EventSink });
    }

    let mut methods: Vec<TokenStream> = Vec::new();

    for (member, payload) in signals {
        let method = &member.method;
        let ordinal = &member.ordinal;
        let path = ty(payload);
        let buffer = payload_buffer(payload);
        let encode = encode_into(
            payload,
            quote! { &value },
            quote! { &mut buf },
            "the payload buffer",
        );
        let set_doc = format!(
            "Stages a new value for signal `{}`. It is published by `commit`.",
            member.declared
        );
        let invalidate_doc = format!(
            "Stages the invalid state for signal `{}`, with \
             `Cause::Declared`. It is published by `commit`.",
            member.declared
        );
        let invalidate = ident(&format!("invalidate_{}", member.method));
        methods.push(quote! {
            #[doc = #set_doc]
            pub fn #method(
                &mut self,
                value: #path,
            ) -> ::core::result::Result<(), ::ridl_rt::port::WriteError> {
                let mut buf = #buffer;
                let bytes = #encode;
                self.port.set(#number, #ordinal, bytes)
            }

            #[doc = #invalidate_doc]
            pub fn #invalidate(
                &mut self,
            ) -> ::core::result::Result<(), ::ridl_rt::port::WriteError> {
                self.port.invalidate(#number, #ordinal)
            }
        });
    }

    for (member, payload) in events {
        let method = &member.method;
        let ordinal = &member.ordinal;
        let path = ty(payload);
        let buffer = payload_buffer(payload);
        let encode = encode_into(
            payload,
            quote! { &value },
            quote! { &mut buf },
            "the payload buffer",
        );
        let doc = format!("Raises one occurrence of event `{}`.", member.declared);
        methods.push(quote! {
            #[doc = #doc]
            pub fn #method(
                &mut self,
                value: #path,
            ) -> ::core::result::Result<(), ::ridl_rt::port::RaiseError> {
                let mut buf = #buffer;
                let bytes = #encode;
                self.port.raise(#number, #ordinal, bytes)
            }
        });
    }

    if !signals.is_empty() {
        methods.push(quote! {
            /// Publishes every staged signal change.
            pub fn commit(&mut self) {
                self.port.commit()
            }
        });
    }

    let doc = format!("The provider face of interface `{iface_name}`'s signals and events.");
    quote! {
        #[doc = #doc]
        pub struct Publisher<W: #(#bounds)+*> {
            port: W,
        }

        impl<W: #(#bounds)+*> Publisher<W> {
            /// Binds the face to a port. The port is held by value: pass a
            /// handle, or a `&mut` borrow of one.
            pub fn new(port: W) -> Self {
                Publisher { port }
            }

            #(#methods)*
        }
    }
}

fn provider(iface_name: &str, commands: &[Call], queries: &[Call]) -> TokenStream {
    let command_methods = commands.iter().map(|call| {
        let member = &call.member;
        let method = &member.method;
        let arg = &call.arg;
        let path = ty(call.arg_type);
        let doc = format!(
            "Serves command `{}`. It returns nothing: a command has no failure \
             the application reports (ridl §6.1). Arguments that break their \
             typl constraints or the `require` clauses never reach it.",
            member.declared
        );
        quote! {
            #[doc = #doc]
            fn #method(&mut self, #arg: &#path);
        }
    });
    let query_methods = queries.iter().map(|call| {
        let member = &call.member;
        let method = &member.method;
        let arg = &call.arg;
        let path = ty(call.arg_type);
        let reply_path = ty(call.reply_type.unwrap_or(call.arg_type));
        let doc = format!(
            "Serves query `{}`. A reply that breaks an `ensure` clause is \
             discarded by `serve`, which settles `ContractBroken` instead.",
            member.declared
        );
        quote! {
            #[doc = #doc]
            fn #method(&mut self, #arg: &#path) -> #reply_path;
        }
    });
    let doc = format!(
        "What an application implements to serve interface `{iface_name}`'s \
         calls.\n\nAn argument is taken by reference because `serve` reads \
         it again when it evaluates a query's `ensure` clauses, and the \
         generated payload types implement neither `Copy` nor `Clone`."
    );
    quote! {
        #[doc = #doc]
        pub trait Provider {
            #(#command_methods)*
            #(#query_methods)*
        }
    }
}

// ---------------------------------------------------------------------------
// Shared pieces.
// ---------------------------------------------------------------------------

/// The interface's number, named through its descriptor rather than repeated
/// as a literal.
fn interface_number(iface: &Ident) -> TokenStream {
    quote! { <super::#iface as ::ridl_rt::contract::Interface>::NUMBER }
}

/// A type reference as the face names it. A same-package declaration is
/// emitted beside the face module, so it is reached through `super::`; a
/// cross-package reference keeps the `crate::`-anchored path [`type_path`]
/// builds.
fn ty(reference: &str) -> TokenStream {
    if reference.contains('.') {
        type_path(reference)
    } else {
        let id = ident(reference);
        quote! { super::#id }
    }
}

/// A stack buffer exactly as large as any legal value of `type_name`.
fn payload_buffer(type_name: &str) -> TokenStream {
    let path = ty(type_name);
    quote! {
        [0u8; <#path as ::ridl_rt::payload::Payload<super::Wire>>::MAX_SIZE]
    }
}

/// Encodes `value` into `target` and evaluates to the encoded bytes.
///
/// The expression evaluates to `Encoded::bytes`, the subslice of `target` the
/// encoder actually wrote, and a call site passes that slice on unchanged. It
/// is **not** a length, and the bytes are **not** necessarily a prefix of
/// `target`: a FlatBuffers builder fills a buffer from its end, which is why
/// [`Encoded::bytes`](ridl_rt::payload::Encoded::bytes) is documented as a
/// subslice. A call site that reconstructed `&target[..len]` would send the
/// wrong bytes for any such encoding.
///
/// `target` is at least `<T as Payload<Wire>>::MAX_SIZE` bytes at every call
/// site, and `MAX_SIZE` is the largest encoded size of any legal value, so
/// `EncodeError::Capacity` cannot arise from a legal value. Reaching it means
/// the value is outside its own type's range or its `Payload` implementation
/// does not honor `MAX_SIZE` — a provider-side implementation defect, not one
/// of the five protocol outcomes. It is therefore an explicit `unreachable!`
/// naming the type, the needed size and the available size, and never a
/// manufactured contract error: `EncodeError::Capacity` carries no received
/// bytes and no violated rule to report.
fn encode_into(
    type_name: &str,
    value: TokenStream,
    target: TokenStream,
    target_name: &str,
) -> TokenStream {
    let path = ty(type_name);
    let capacity = format!(
        "encoding `{type_name}` needs {{}} bytes and {target_name} has {{}}; a legal value \
         cannot exceed `<{type_name} as Payload<Wire>>::MAX_SIZE`, so the value is outside \
         its own type's range or its `Payload` implementation does not honor `MAX_SIZE`"
    );
    let other = format!("encoding `{type_name}` failed");
    quote! {
        match ::ridl_rt::payload::Ref::<#path, super::Wire>::encode(
            #value,
            #target,
        ) {
            Ok(encoded) => encoded.bytes(),
            Err(::ridl_rt::payload::EncodeError::Capacity { needed, available }) => {
                unreachable!(#capacity, needed, available)
            }
            Err(_) => unreachable!(#other),
        }
    }
}

/// Checks a claim's argument bytes and decodes them, or gives the settlement
/// the failure maps to: malformed bytes are a transport corruption, and a
/// value that breaks its typl constraints is a contract error carrying the
/// violation.
fn decode_args(type_name: &str) -> TokenStream {
    let path = ty(type_name);
    quote! {
        match ::ridl_rt::payload::Ref::<#path, super::Wire>::verify(
            &buf[..claim.len],
        ) {
            Ok(checked) => Ok(checked.decode()),
            Err(::ridl_rt::payload::VerifyError::Structure(_)) => {
                Err(::ridl_rt::error::CallError::Transport(
                    ::ridl_rt::error::Transport::Corrupt,
                ))
            }
            Err(::ridl_rt::payload::VerifyError::Contract(violation)) => {
                Err(::ridl_rt::error::CallError::Contract(
                    ::ridl_rt::error::Contract::InvalidValue(violation),
                ))
            }
            Err(_) => Err(::ridl_rt::error::CallError::Transport(
                ::ridl_rt::error::Transport::Corrupt,
            )),
        }
    }
}

/// The single declared parameter's name, in snake case.
///
/// [`single_param_type`] has already refused an interaction that does not
/// declare exactly one parameter, so the fallback is unreachable; it is a
/// value rather than a panic because codegen is total.
fn single_param_name(params: &[v1::Param]) -> Ident {
    params.first().map_or_else(
        || ident("value"),
        |param| ident(snake_of(param.name.as_ref())),
    )
}

/// The canonical IR text of the type one payload names.
fn payload_reference(payload: Option<&v1::Payload>) -> &str {
    payload
        .and_then(|payload| payload.r#type.as_ref())
        .map(|reference| reference.reference.as_str())
        .unwrap_or_default()
}

/// The pinned `snake_case` of an interface's declared name, which names its
/// generated module.
///
/// Before stage P4 this module had a `snake_case` of its own, which differed
/// from the transform ADR-0016 decision 1 pins on an acronym followed by a
/// word — `HTTPServer` became `httpserver` rather than `http_server`
/// (driftsys/ridl#450). The model carries the pinned spelling only, so
/// reading it closes that divergence.
fn interface_snake(interface: &v1::Interface) -> &str {
    match interface.identity.as_ref() {
        Some(v1::interface::Identity::Declared(name)) => name.snake.as_str(),
        _ => "",
    }
}
