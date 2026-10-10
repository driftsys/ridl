//! The generated interaction face
//! (`docs/design/interaction-face.md`; the approved design is
//! `docs/archive/2026-09-16-interaction-face-v0-design.md` §6).
//!
//! For each named interface this module emits one `pub mod`, named after the
//! interface, holding what the design note's §8 calls the face, as the async
//! face reshaped it (`docs/archive/2026-09-25-async-face-design.md`, notes
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
//! - the two per-interface traits, `Subscribe` (one `subscribe_<event>` per
//!   event) and `Invalidate` (one `invalidate_<signal>` per signal), and a
//!   `prelude` module that re-exports them as `_` beside the `ridl_rt::face`
//!   traits the module's types implement. The face's fixed methods — `new`,
//!   `next_event`, `commit`, and the blocking client's `with_timeout` and
//!   `set_timeout` — are those traits' methods, and only the member methods
//!   are inherent, so a member may carry any of those names (ADR-0023
//!   decision 7, driftsys/ridl#580). The emitter's own calls to a fixed or a
//!   derived method are written through the trait's path, so a member of
//!   that name cannot capture them;
//! - `Provider`, the trait the application implements, with one method per
//!   command and query;
//! - `check_catalog`, private, which panics unless a port's catalog is the
//!   interface's `CATALOG` (ADR-0023 decision 8). Every `Bind::new` of the
//!   face, and `serve` where the interface emits one, call it once, before
//!   they store or use the port; the blocking face reaches it through the
//!   async face;
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
//! handler has no claim waiting, it has taken `SERVE_BUDGET` claims in
//! that poll, or it refused the settlement of an oversized claim
//! (driftsys/ridl#569) — and returns; what waits is the executor or
//! the frame loop that polls it, or `ridl_rt::task::block_on` under the
//! `blocking` module, which is the library's and not generated. No method is
//! bounded on `CoherentSignals`.
//!
//! The face is reached from [`crate::generate_pipeline`], which
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
    declared_name, descriptor_ident, interactions, query_param_type, query_reply_type,
    single_param_type,
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

/// The face module of one interface, or `None` when the interface is `internal`
/// or declares nothing the face carries. Reachable from the crate for the pipeline's
/// per-interface walk (interaction-face design rule 2).
///
/// The module is named by the interface's `snake_case`, so an interface
/// `Climate` of package `veh` beside a package `veh.climate` gives a face
/// module `climate` that the child package's module hides in the crate tree
/// `ridlc` writes (driftsys/ridl#416): `veh::climate` names the child package.
/// Generated code never names a face module by a path from another module,
/// so the crate compiles, and a consumer reaches the face as
/// `veh::__ridl_package::climate`.
pub(crate) fn one_interface(
    interface: &v1::Interface,
) -> Result<Option<TokenStream>, GenerateError> {
    if is_internal(interface) {
        return Ok(None);
    }
    let iface_name = declared_name(interface).unwrap_or_default();
    let iface = ident(iface_name);
    let module = module_ident(interface);

    // Payload type names are kept beside each member, because the face names
    // the declared type directly (the face emits no induced argument struct).
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
                let ident = descriptor_ident(&iface, interaction);
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
        // Every interface that gets a `Client` gets `check_catalog`: the
        // client's `Bind::new` calls it, as do `Publisher`'s and `serve`.
        body.push(check_catalog(&iface, iface_name));
        if !events.is_empty() {
            body.push(subscribe_trait(iface_name, &events));
        }
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
        if !signals.is_empty() {
            body.push(invalidate_trait(iface_name, &signals));
        }
        body.push(publisher(&iface, iface_name, &signals, &events));
    }
    if !commands.is_empty() || !queries.is_empty() {
        body.push(provider(iface_name, &commands, &queries));
        body.push(dispatch(&iface, iface_name, &commands, &queries));
        body.push(serve(&iface, iface_name, &commands, &queries));
    }

    if body.is_empty() {
        return Ok(None);
    }

    // The prelude follows every trait it re-exports, and the blocking module
    // stays the module's last item.
    let blocking = blocking(&iface, iface_name, &signals, &events, &commands, &queries);
    body.push(prelude(iface_name, &signals, &events, blocking.is_some()));
    body.extend(blocking);

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
    call_type_ident(call.member.camel, CALL_SUFFIXES[2])
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
                    _ => match ::ridl_rt::payload::Ref::<#path, ::ridl_rt::encoding::FlatBuffers>::verify(
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

    for call in commands {
        methods.push(call_method(call, "command"));
    }

    for call in queries {
        methods.push(call_method(call, "query"));
    }

    // The member methods above are inherent; the fixed methods `new` and
    // `next_event` and the derived `subscribe_<event>` are trait methods
    // (ADR-0023 decision 7), so a member of one of those names compiles.
    let mut fixed: Vec<TokenStream> = Vec::new();

    if !events.is_empty() {
        let subscribes = events.iter().map(|(member, _)| {
            let method = ident(&format!("subscribe_{}", member.method));
            let ordinal = &member.ordinal;
            quote! {
                fn #method(
                    &mut self,
                ) -> ::core::result::Result<(), ::ridl_rt::port::SubscribeError> {
                    self.port.subscribe(#number, &[#ordinal])
                }
            }
        });
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
        fixed.push(quote! {
            impl<P: #(#bounds)+*> ::ridl_rt::face::Events for Client<P> {
                type Next<'a> = NextEvent<'a, P> where Self: 'a;

                #[doc = #doc]
                fn next_event(&mut self) -> NextEvent<'_, P> {
                    NextEvent { port: &mut self.port }
                }
            }

            impl<P: #(#bounds)+*> Subscribe for Client<P> {
                #(#subscribes)*
            }
        });
    }

    let inherent = (!methods.is_empty()).then(|| {
        quote! {
            impl<P: #(#bounds)+*> Client<P> {
                #(#methods)*
            }
        }
    });

    // The rustdoc names only the traits this interface's client implements:
    // `Events` and `Subscribe` exist on it only when the interface declares
    // an event.
    let traits = if events.is_empty() {
        "`new` is `ridl_rt::face::Bind`'s, in scope through `prelude`."
    } else {
        "`new` is `ridl_rt::face::Bind`'s, `next_event` is \
         `ridl_rt::face::Events`'s and `subscribe_<event>` is this module's \
         `Subscribe`'s, all in scope through `prelude`."
    };
    let new_doc = bind_new_doc(iface);
    let doc = format!(
        "The consumer face of interface `{iface_name}`, generic over exactly \
         the ports the interface's interactions need. Its member methods are \
         inherent; {traits}"
    );
    quote! {
        #[doc = #doc]
        pub struct Client<P: #(#bounds)+*> {
            port: P,
        }

        #inherent

        impl<P: #(#bounds)+*> ::ridl_rt::face::Bind for Client<P> {
            type Port = P;

            #[doc = #new_doc]
            #[track_caller]
            fn new(port: P) -> Self {
                check_catalog(::ridl_rt::port::Attached::catalog(&port));
                Client { port }
            }
        }

        #(#fixed)*
    }
}

/// The per-interface `Subscribe` trait, one `subscribe_<event>` per event,
/// for an interface that declares one. It lives in the interface module,
/// where every derived type carries a fixed suffix, so its fixed name meets
/// no derived one; both clients implement it. The blocking client's
/// delegation names it by path, `super::Subscribe::subscribe_<event>`, so a
/// member named `subscribe<Event>` cannot capture that call.
fn subscribe_trait(iface_name: &str, events: &[(Member, &str)]) -> TokenStream {
    let methods = events.iter().map(|(member, _)| {
        let method = ident(&format!("subscribe_{}", member.method));
        let doc = format!("Starts delivery of event `{}`.", member.declared);
        quote! {
            #[doc = #doc]
            fn #method(
                &mut self,
            ) -> ::core::result::Result<(), ::ridl_rt::port::SubscribeError>;
        }
    });
    let doc = format!(
        "Starts delivery of one event of interface `{iface_name}`: one method \
         per event, implemented by `Client` and by `blocking::Client`. A trait \
         rather than inherent methods so that a member of the interface may be \
         named `subscribe<Event>` (ADR-0023 decision 7); `prelude` brings it \
         into scope anonymously."
    );
    quote! {
        #[doc = #doc]
        pub trait Subscribe {
            #(#methods)*
        }
    }
}

/// The per-interface `Invalidate` trait, one `invalidate_<signal>` per
/// signal, for an interface that declares one; `Publisher` implements it.
/// The same reasoning as [`subscribe_trait`].
fn invalidate_trait(iface_name: &str, signals: &[(Member, &str)]) -> TokenStream {
    let methods = signals.iter().map(|(member, _)| {
        let method = ident(&format!("invalidate_{}", member.method));
        let doc = format!(
            "Stages the invalid state for signal `{}`, with \
             `Cause::Declared`. It is published by `commit`.",
            member.declared
        );
        quote! {
            #[doc = #doc]
            fn #method(
                &mut self,
            ) -> ::core::result::Result<(), ::ridl_rt::port::WriteError>;
        }
    });
    let doc = format!(
        "Stages the invalid state of one signal of interface `{iface_name}`: \
         one method per signal, implemented by `Publisher`. A trait rather \
         than inherent methods so that a member of the interface may be named \
         `invalidate<Signal>` (ADR-0023 decision 7); `prelude` brings it into \
         scope anonymously."
    );
    quote! {
        #[doc = #doc]
        pub trait Invalidate {
            #(#methods)*
        }
    }
}

/// The `prelude` module of one interface: the `ridl-rt` face traits the
/// module's types implement, re-exported by name so that a consumer can
/// write `<Client<_> as Bind>::new(port)` from the prelude alone, and the
/// module's own `Subscribe` and `Invalidate` re-exported as `_`, so that two
/// interfaces' preludes glob-imported into one scope do not conflict. Nothing
/// is re-exported that no type of the module implements: no `Events` with no
/// event, no `Publish` and no `Invalidate` with no signal, no `Subscribe`
/// with no event, and `Timeout` only where the `blocking` module is emitted.
fn prelude(
    iface_name: &str,
    signals: &[(Member, &str)],
    events: &[(Member, &str)],
    has_blocking: bool,
) -> TokenStream {
    let mut uses: Vec<TokenStream> = vec![quote! { pub use ::ridl_rt::face::Bind; }];
    if !events.is_empty() {
        uses.push(quote! { pub use ::ridl_rt::face::Events; });
        uses.push(quote! { pub use super::Subscribe as _; });
    }
    if !signals.is_empty() {
        uses.push(quote! { pub use ::ridl_rt::face::Publish; });
        uses.push(quote! { pub use super::Invalidate as _; });
    }
    if has_blocking {
        uses.push(quote! {
            #[cfg(feature = "std")]
            pub use ::ridl_rt::face::Timeout;
        });
    }
    // The rustdoc names only the methods the re-exported traits carry, so
    // an interface without an event, a signal or a blocking module is not
    // documented with a method its face lacks.
    let mut methods = vec!["`new`"];
    if !events.is_empty() {
        methods.extend(["`next_event`", "`subscribe_<event>`"]);
    }
    if !signals.is_empty() {
        methods.extend(["`commit`", "`invalidate_<signal>`"]);
    }
    if has_blocking {
        methods.extend(["`with_timeout`", "`set_timeout`"]);
    }
    let methods = methods.join(", ");
    let doc = format!(
        "The traits a consumer of interface `{iface_name}`'s face needs in \
         scope. Glob-import this module, `use <this interface's \
         module>::prelude::*;`, and every method of those traits — \
         {methods} — is called as an inherent method would be. Only \
         the `ridl-rt` traits are re-exported by name; this module's own \
         traits are re-exported as `_`, so the preludes of two interfaces can \
         share one scope."
    );
    quote! {
        #[doc = #doc]
        pub mod prelude {
            #(#uses)*
        }
    }
}

/// The name of one call's future type, `<Name>Call`.
fn future_type(call: &Call) -> Ident {
    call_type_ident(call.member.camel, CALL_SUFFIXES[0])
}

/// The name of one call's phase enum, `<Name>Phase`, private to the module.
fn phase_type(call: &Call) -> Ident {
    call_type_ident(call.member.camel, CALL_SUFFIXES[1])
}

/// The suffixes of the three types the face module holds per command and per
/// query: the future `<Name>Call`, the phase enum `<Name>Phase` and the
/// correlation newtype `<Name>Correlation`. No fixed type name of the face
/// module ends with one of them, so a name spelled here can meet only another
/// name spelled here.
pub(crate) const CALL_SUFFIXES: [&str; 3] = ["Call", "Phase", "Correlation"];

/// One call's type of the face module: the member's `camel_case` followed by
/// one of [`CALL_SUFFIXES`]. The claim table (`crate::claims`) claims the
/// names it returns.
pub(crate) fn call_type_ident(camel: &str, suffix: &str) -> Ident {
    ident(&format!("{camel}{suffix}"))
}

/// Whether an interface is declared `internal`. Such an interface gets no
/// face: a face is public API whose signatures name the interface's payload
/// types, and those types may be `pub(crate)` under an `internal` interface,
/// so a public item could name a private type (E0446). The interface's
/// descriptors are still emitted, with the interface's own visibility.
pub(crate) fn is_internal(interface: &v1::Interface) -> bool {
    interface.visibility == v1::Visibility::Internal as i32
}

/// The interaction kinds the face module carries: a signal, an event, a
/// command and a query. An interface that declares at least one live member
/// of those kinds gets a face module when its face is emitted, and one that
/// declares none (only `fixed` members, or no member) gets none, which is
/// when [`one_interface`] returns `None`. An `internal` interface gets none
/// either (see [`is_internal`]).
pub(crate) fn emits_module(interface: &v1::Interface) -> bool {
    !is_internal(interface)
        && interactions(interface).iter().any(|(_, interaction)| {
            matches!(
                interaction.shape.as_ref(),
                Some(
                    v1::interaction::Shape::Signal(_)
                        | v1::interaction::Shape::Event(_)
                        | v1::interaction::Shape::Command(_)
                        | v1::interaction::Shape::Query(_)
                )
            )
        })
}

/// The name of an interface's face module: the pinned `snake_case` of the
/// interface's declared name, through [`ident`].
pub(crate) fn module_ident(interface: &v1::Interface) -> Ident {
    ident(interface_snake(interface))
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
                self.port.raise(#number, #ordinal, bytes, ::core::option::Option::None)
            }
        });
    }

    // The member methods above are inherent; `new` and `commit` and the
    // derived `invalidate_<signal>` are trait methods (ADR-0023 decision 7).
    let fixed = (!signals.is_empty()).then(|| {
        let invalidates = signals.iter().map(|(member, _)| {
            let invalidate = ident(&format!("invalidate_{}", member.method));
            let ordinal = &member.ordinal;
            quote! {
                fn #invalidate(
                    &mut self,
                ) -> ::core::result::Result<(), ::ridl_rt::port::WriteError> {
                    self.port.invalidate(#number, #ordinal)
                }
            }
        });
        quote! {
            impl<W: #(#bounds)+*> ::ridl_rt::face::Publish for Publisher<W> {
                /// Publishes every staged signal change.
                fn commit(&mut self) {
                    self.port.commit()
                }
            }

            impl<W: #(#bounds)+*> Invalidate for Publisher<W> {
                #(#invalidates)*
            }
        }
    });

    // The rustdoc names only the traits this interface's publisher
    // implements: `Publish` and `Invalidate` exist on it only when the
    // interface declares a signal.
    let traits = if signals.is_empty() {
        "`new` is `ridl_rt::face::Bind`'s, in scope through `prelude`."
    } else {
        "`new` is `ridl_rt::face::Bind`'s, `commit` is \
         `ridl_rt::face::Publish`'s and `invalidate_<signal>` is this \
         module's `Invalidate`'s, all in scope through `prelude`."
    };
    let new_doc = bind_new_doc(iface);
    let doc = format!(
        "The provider face of interface `{iface_name}`'s signals and events. \
         Its member methods are inherent; {traits}"
    );
    quote! {
        #[doc = #doc]
        pub struct Publisher<W: #(#bounds)+*> {
            port: W,
        }

        impl<W: #(#bounds)+*> Publisher<W> {
            #(#methods)*
        }

        impl<W: #(#bounds)+*> ::ridl_rt::face::Bind for Publisher<W> {
            type Port = W;

            #[doc = #new_doc]
            #[track_caller]
            fn new(port: W) -> Self {
                check_catalog(::ridl_rt::port::Attached::catalog(&port));
                Publisher { port }
            }
        }

        #fixed
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

/// `check_catalog`, the interface module's private comparison of a port's
/// catalog with the interface's `CATALOG` (ADR-0023 decision 8). It is
/// emitted once per module and called once per binding, so the generated code
/// carries one comparison and one panic message per interface. The whole
/// `CatalogRef` is compared, name and hash. `::core::panic!` keeps it valid
/// under `no_std`. It and every generated function that calls it — both
/// `Bind::new`s, the blocking `Bind::new` and both `serve`s — are
/// `#[track_caller]`, so the panic reports the program's binding call as its
/// location, not a line of the generated file.
fn check_catalog(iface: &Ident, iface_name: &str) -> TokenStream {
    let message = format!(
        "the face of interface `{iface_name}` was generated from catalog {{:?}}, \
         but the port is attached to catalog {{:?}}"
    );
    let doc = format!(
        "Panics unless `found` is the catalog the face of interface \
         `{iface_name}` was generated from, the interface's `CATALOG` \
         (ADR-0023 decision 8). Every `Bind::new` of the face, and `serve` where \
         the interface emits one, call it once, before they store or use the \
         port."
    );
    quote! {
        #[doc = #doc]
        #[track_caller]
        fn check_catalog(found: &::ridl_rt::contract::CatalogRef) {
            let expected = <super::#iface as ::ridl_rt::contract::Interface>::CATALOG;
            if *found != *expected {
                ::core::panic!(#message, expected, found);
            }
        }
    }
}

/// The rustdoc of the async `Client`'s and of `Publisher`'s `Bind::new`: the
/// binding, then its `# Panics` section.
fn bind_new_doc(iface: &Ident) -> String {
    format!(
        "Binds the face to a port. The port is held by value: pass a handle, \
         or a `&mut` borrow of one.\n\n{}",
        catalog_panics_doc(
            iface,
            "port",
            "the comparison is made once, before the port is stored",
        )
    )
}

/// The `# Panics` section of every rustdoc whose function compares the
/// port's catalog, directly or through the async face: what panics, where the
/// comparison is made (`when`), and the comparison a program makes first to
/// handle a mismatch without the panic, in the paths a consumer of the
/// generated crate writes.
pub(super) fn catalog_panics_doc(iface: &Ident, port: &str, when: &str) -> String {
    format!(
        "# Panics\n\nPanics when `{port}` is attached to a catalog other than \
         the one this face was generated from, that is, when the package name \
         or the catalog hash differs (ADR-0023 decision 8); {when}. A program \
         that must not panic makes the same comparison first, \
         `{port}.catalog() == <{iface} as ridl_rt::contract::Interface>::CATALOG` \
         with `ridl_rt::port::Attached` in scope, where `{iface}` is the \
         interface's descriptor type, declared beside this interface's \
         module, and handles a mismatch its own way."
    )
}

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
        [0u8; <#path as ::ridl_rt::payload::Payload<::ridl_rt::encoding::FlatBuffers>>::MAX_SIZE]
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
/// `target` is at least `<T as Payload<FlatBuffers>>::MAX_SIZE` bytes at every
/// call site, and `MAX_SIZE` is the largest encoded size of any legal value, so
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
         cannot exceed `<{type_name} as Payload<FlatBuffers>>::MAX_SIZE`, so the value is outside \
         its own type's range or its `Payload` implementation does not honor `MAX_SIZE`"
    );
    let other = format!("encoding `{type_name}` failed");
    quote! {
        match ::ridl_rt::payload::Ref::<#path, ::ridl_rt::encoding::FlatBuffers>::encode(
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
        match ::ridl_rt::payload::Ref::<#path, ::ridl_rt::encoding::FlatBuffers>::verify(
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
/// This module once had a `snake_case` of its own, which differed
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
