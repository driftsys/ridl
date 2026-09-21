//! The generated interaction face (Lane M stage M3, Tasks 2 and 3,
//! `docs/design/interaction-face.md`; the approved design is
//! `docs/archive/2026-09-16-interaction-face-v0-design.md` §6).
//!
//! For each named interface this module emits one `pub mod`, named after the
//! interface, holding what the design note's §8 calls the face:
//!
//! - one `Copy` correlation newtype per command and per query,
//!   `<Name>Correlation`, which the call's send method returns and its
//!   `<name>_reply` or `<name>_ack` takes;
//! - `Client<P>`, the consumer face, generic over exactly the ports the
//!   interface's own interactions need and no others (RA-19): `SignalReader`
//!   when it declares a signal, `EventSource` when it declares an event,
//!   `Caller` when it declares a command or a query;
//! - `Publisher<W>`, the provider face for signals and events, over
//!   `SignalWriter` and `EventSink` on the same rule;
//! - `Provider`, the trait the application implements, with one method per
//!   command and query;
//! - `dispatch`, which routes a claim, checks the argument bytes, evaluates
//!   `require`, calls the provider, evaluates a query's `ensure`, and settles.
//!
//! An item whose interface declares nothing for it is not emitted: a
//! signal-only interface gets a `Client` and a `Publisher` and no `Provider`
//! or `dispatch`, because no call can reach it.
//!
//! Every method returns without waiting (RA-20). There is no thread, future,
//! socket or timer here, and no method is bounded on `CoherentSignals`.
//!
//! The face is reached only from [`crate::generate_face`], never from the
//! pipeline [`crate::generate`].

use crate::descriptors::{query_reply_type, single_param_type};
use crate::{GenerateError, ident, type_path};
use proc_macro2::{Ident, Literal, TokenStream};
use quote::quote;
use ridl_ir::name::camel_case;
use ridl_ir::v2;

/// The face module for every named interface in `package`, in source order.
///
/// The walk is over [`Package::shapes()`](ridl_ir::v2::Package::shapes), and a
/// service's inline shape is skipped, for the same two reasons the descriptor
/// layer gives.
pub(crate) fn interface_items(package: &v2::Package) -> Result<Vec<TokenStream>, GenerateError> {
    let mut items = Vec::new();
    for shape in package.shapes() {
        if shape.service.is_some() {
            continue;
        }
        if let Some(module) = one_interface(shape.name, shape.interface)? {
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

fn one_interface(
    iface_name: &str,
    interface: &v2::Interface,
) -> Result<Option<TokenStream>, GenerateError> {
    let iface = ident(iface_name);
    let module = ident(&snake_case(iface_name));

    // Payload type names are kept beside each member, because the face names
    // the declared type directly (M3 emits no induced argument struct).
    let mut signals: Vec<(Member, &str)> = Vec::new();
    let mut events: Vec<(Member, &str)> = Vec::new();
    let mut commands: Vec<Call> = Vec::new();
    let mut queries: Vec<Call> = Vec::new();

    for decl in &interface.interactions {
        let member = Member {
            ordinal: {
                let value = Literal::u32_suffixed(decl.ordinal);
                quote! { ::ridl_rt::contract::Ordinal(#value) }
            },
            method: ident(&snake_case(&decl.name)),
            descriptor: {
                let name = ident(&format!("{iface}{}", camel_case(&decl.name)));
                quote! { super::#name }
            },
            declared: decl.name.as_str(),
        };
        match decl.kind.as_ref() {
            Some(v2::decl::Kind::SignalDef(signal)) => {
                signals.push((member, signal.payload.as_str()));
            }
            Some(v2::decl::Kind::EventDef(event)) => {
                events.push((member, event.payload.as_str()));
            }
            Some(v2::decl::Kind::CommandDef(command)) => {
                commands.push(Call {
                    arg_type: single_param_type(&command.params, &decl.name)?,
                    arg: single_param_name(&command.params),
                    member,
                    reply_type: None,
                });
            }
            Some(v2::decl::Kind::QueryDef(query)) => {
                queries.push(Call {
                    arg_type: single_param_type(&query.params, &decl.name)?,
                    arg: single_param_name(&query.params),
                    member,
                    reply_type: Some(query_reply_type(query, &decl.name)?),
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
    if !signals.is_empty() || !events.is_empty() {
        body.push(publisher(&iface, iface_name, &signals, &events));
    }
    if !commands.is_empty() || !queries.is_empty() {
        body.push(provider(iface_name, &commands, &queries));
        body.push(dispatch(&iface, iface_name, &commands, &queries));
    }

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
    ident(&format!("{}Correlation", camel_case(call.member.declared)))
}

/// One `Copy` correlation newtype per command and per query.
///
/// The newtype is the face's, not the port's: `ridl_rt::port::Correlation`
/// stays untyped, because which interaction a correlation belongs to is a
/// payload-shaped fact and a port never carries one (ADR-0023 decision 4's
/// 2026-09-20 amendment). What the newtype buys is that a query's correlation
/// cannot be passed to an `ack` and a command's cannot be passed to a
/// `*_reply`.
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
             the send method and accepted by that call's own outcome method, \
             and by no other.",
            call.member.declared
        );
        items.push(quote! {
            #[doc = #doc]
            #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
            pub struct #name(pub ::ridl_rt::port::Correlation);
        });
    }
    items
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
    let mut bounds: Vec<TokenStream> = Vec::new();
    if !signals.is_empty() {
        bounds.push(quote! { ::ridl_rt::port::SignalReader });
    }
    if !events.is_empty() {
        bounds.push(quote! { ::ridl_rt::port::EventSource });
    }
    if !commands.is_empty() || !queries.is_empty() {
        bounds.push(quote! { ::ridl_rt::port::Caller });
    }

    let mut methods: Vec<TokenStream> = Vec::new();

    for (member, payload) in signals {
        let method = &member.method;
        let ordinal = &member.ordinal;
        let descriptor = &member.descriptor;
        let path = ty(payload);
        let buffer = payload_buffer(payload);
        let doc = format!(
            "Reads signal `{}` and returns its value with the provenance, the \
             freshness and the envelope the runtime resolved. A payload that \
             fails its check is reported as `Provenance::Invalid` with the \
             detection, and the value is the channel's init value.",
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
                match ::ridl_rt::payload::Ref::<#path, super::Wire>::verify(
                    &buf[..raw.len],
                ) {
                    Ok(checked) => Ok(::ridl_rt::sample::Sample {
                        value: checked.decode(),
                        provenance: raw.provenance,
                        freshness: raw.freshness,
                        envelope: raw.envelope,
                    }),
                    Err(error) => Ok(::ridl_rt::sample::Sample {
                        value: <#descriptor as ::ridl_rt::contract::Signal>::init(),
                        provenance: ::ridl_rt::sample::Provenance::Invalid(
                            ::ridl_rt::sample::Cause::Detected(match error {
                                ::ridl_rt::payload::VerifyError::Contract(violation) => {
                                    ::ridl_rt::sample::Detection::InvalidValue(violation)
                                }
                                _ => ::ridl_rt::sample::Detection::Corrupt,
                            }),
                        ),
                        freshness: raw.freshness,
                        envelope: raw.envelope,
                    }),
                }
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
        let buffer = quote! { [0u8; super::#iface::EVENT_SOURCE_BUFFER_SIZE] };
        let arms = events.iter().map(|(member, payload)| {
            let ordinal = &member.ordinal;
            let variant = ident(&camel_case(member.declared));
            let path = ty(payload);
            quote! {
                #ordinal => Ok(Some(Event::#variant(::ridl_rt::sample::Occurrence {
                    payload: match ::ridl_rt::payload::Ref::<
                        #path,
                        super::Wire,
                    >::verify(&buf[..occurrence.len]) {
                        Ok(checked) => Ok(checked.decode()),
                        Err(::ridl_rt::payload::VerifyError::Contract(violation)) => {
                            Err(::ridl_rt::sample::Detection::InvalidValue(violation))
                        }
                        Err(_) => Err(::ridl_rt::sample::Detection::Corrupt),
                    },
                    envelope: occurrence.envelope,
                })))
            }
        });
        let doc = format!(
            "Takes the next occurrence of any subscribed event of interface \
             `{iface_name}`, routed to its variant by ordinal. `Ok(None)` when \
             none is waiting. One method serves every event, because the \
             payload type is not known until the occurrence's ordinal is \
             read.\n\nThe interface number is checked before the ordinal, \
             for the reason `dispatch` checks it: a port is attached to a \
             whole catalog, ordinals restart at 1 in each interface, and an \
             occurrence of a sibling interface at the same ordinal would \
             otherwise be decoded as this interface's payload. Such an \
             occurrence is reported as `Contract::UnknownInteraction`; \
             `EventSource::next` has already consumed it, so this face cannot \
             hand it back to the interface it belongs to. Subscribe on a port \
             this interface owns."
        );
        methods.push(quote! {
            #[doc = #doc]
            pub fn next_event(
                &mut self,
            ) -> ::core::result::Result<
                ::core::option::Option<Event>,
                ::ridl_rt::port::ReadError,
            > {
                let mut buf = #buffer;
                let Some(occurrence) = self.port.next(&mut buf)? else {
                    return Ok(None);
                };
                if occurrence.iface
                    != <super::#iface as ::ridl_rt::contract::Interface>::NUMBER
                {
                    return Err(::ridl_rt::port::ReadError::Contract(
                        ::ridl_rt::error::Contract::UnknownInteraction,
                    ));
                }
                match occurrence.ord {
                    #(#arms,)*
                    _ => Err(::ridl_rt::port::ReadError::Contract(
                        ::ridl_rt::error::Contract::UnknownInteraction,
                    )),
                }
            }
        });
    }

    for call in commands {
        methods.push(send(
            &number,
            call,
            quote! { ::ridl_rt::contract::Command },
            quote! { command },
            "command",
        ));
    }

    for call in queries {
        methods.push(send(
            &number,
            call,
            quote! { ::ridl_rt::contract::Query },
            quote! { query },
            "query",
        ));

        let member = &call.member;
        let reply = call.reply_type.unwrap_or(call.arg_type);
        let correlation = correlation_type(call);
        let method = ident(&format!("{}_reply", member.method));
        let path = ty(reply);
        let buffer = payload_buffer(reply);
        let doc = format!(
            "Takes query `{}`'s reply once it is known, or `Ok(None)` while it \
             is not. It does not wait.",
            member.declared
        );
        methods.push(quote! {
            #[doc = #doc]
            pub fn #method(
                &mut self,
                correlation: #correlation,
            ) -> ::core::result::Result<
                ::core::option::Option<
                    ::core::result::Result<#path, ::ridl_rt::error::CallError>,
                >,
                ::ridl_rt::port::ReadError,
            > {
                let mut buf = #buffer;
                match self.port.reply(correlation.0, &mut buf)? {
                    None => Ok(None),
                    Some(Err(error)) => Ok(Some(Err(error))),
                    Some(Ok(len)) => Ok(Some(
                        match ::ridl_rt::payload::Ref::<
                            #path,
                            super::Wire,
                        >::verify(&buf[..len]) {
                            Ok(checked) => Ok(checked.decode()),
                            Err(::ridl_rt::payload::VerifyError::Contract(violation)) => {
                                Err(::ridl_rt::error::CallError::Contract(
                                    ::ridl_rt::error::Contract::InvalidValue(violation),
                                ))
                            }
                            Err(_) => Err(::ridl_rt::error::CallError::Transport(
                                ::ridl_rt::error::Transport::Corrupt,
                            )),
                        },
                    )),
                }
            }
        });
    }

    for call in commands {
        let member = &call.member;
        let correlation = correlation_type(call);
        let method = ident(&format!("{}_ack", member.method));
        let doc = format!(
            "Takes command `{}`'s delivery acknowledgment once it is known, \
             or `None` while it is not. It does not wait.",
            member.declared
        );
        methods.push(quote! {
            #[doc = #doc]
            pub fn #method(
                &mut self,
                correlation: #correlation,
            ) -> ::core::option::Option<
                ::core::result::Result<(), ::ridl_rt::error::CallError>,
            > {
                self.port.ack(correlation.0)
            }
        });
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

/// One consumer-side call method: evaluate `require`, encode the argument,
/// hand it to the `Caller` port.
fn send(
    number: &TokenStream,
    call: &Call,
    contract_trait: TokenStream,
    port_method: TokenStream,
    kind: &str,
) -> TokenStream {
    let member = &call.member;
    let method = &member.method;
    let ordinal = &member.ordinal;
    let descriptor = &member.descriptor;
    let correlation = correlation_type(call);
    let arg = &call.arg;
    let path = ty(call.arg_type);
    let buffer = payload_buffer(call.arg_type);
    let encode = encode_into(
        call.arg_type,
        quote! { &#arg },
        quote! { &mut buf },
        "the argument buffer",
    );
    let doc = format!(
        "Sends {kind} `{}` and returns the correlation of its outcome. A \
         `require` clause that fails is reported as \
         `SendError::Contract(Contract::PreconditionFailed)` and nothing is \
         sent.",
        member.declared
    );
    quote! {
        #[doc = #doc]
        pub fn #method(
            &mut self,
            #arg: #path,
        ) -> ::core::result::Result<#correlation, ::ridl_rt::port::SendError> {
            <#descriptor as #contract_trait>::require(&#arg).map_err(|()| {
                ::ridl_rt::port::SendError::Contract(
                    ::ridl_rt::error::Contract::PreconditionFailed,
                )
            })?;
            let mut buf = #buffer;
            let bytes = #encode;
            self.port.#port_method(#number, #ordinal, bytes).map(#correlation)
        }
    }
}

/// The occurrence enum one `next_event` routes into.
fn event_enum(iface_name: &str, events: &[(Member, &str)]) -> TokenStream {
    let variants = events.iter().map(|(member, payload)| {
        let variant = ident(&camel_case(member.declared));
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
             discarded by `dispatch`, which settles `ContractBroken` instead.",
            member.declared
        );
        quote! {
            #[doc = #doc]
            fn #method(&mut self, #arg: &#path) -> #reply_path;
        }
    });
    let doc = format!(
        "What an application implements to serve interface `{iface_name}`'s \
         calls.\n\nAn argument is taken by reference because `dispatch` reads \
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
// Dispatch.
// ---------------------------------------------------------------------------

fn dispatch(iface: &Ident, iface_name: &str, commands: &[Call], queries: &[Call]) -> TokenStream {
    let number = interface_number(iface);

    let command_arms = commands.iter().map(|call| {
        let member = &call.member;
        let ordinal = &member.ordinal;
        let method = &member.method;
        let descriptor = &member.descriptor;
        let decode = decode_args(call.arg_type);
        let arg = &call.arg;
        quote! {
            #ordinal => {
                let decoded = #decode;
                match decoded {
                    Err(error) => h.settle(claim.id, Err(error)),
                    Ok(#arg) => {
                        match <#descriptor as ::ridl_rt::contract::Command>::require(&#arg) {
                            Err(()) => h.settle(
                                claim.id,
                                Err(::ridl_rt::error::CallError::Contract(
                                    ::ridl_rt::error::Contract::PreconditionFailed,
                                )),
                            ),
                            Ok(()) => {
                                // A command's acknowledgment is a delivery
                                // acknowledgment, not a completion one (ridl
                                // §6.1), so the claim is settled once the
                                // arguments and `require` pass and before the
                                // application's method runs (`Handler`).
                                let accepted = h.settle(claim.id, Ok(&[]));
                                p.#method(&#arg);
                                accepted
                            }
                        }
                    }
                }
            }
        }
    });

    let query_arms = queries.iter().map(|call| {
        let member = &call.member;
        let ordinal = &member.ordinal;
        let method = &member.method;
        let descriptor = &member.descriptor;
        let decode = decode_args(call.arg_type);
        let encode = encode_into(
            call.reply_type.unwrap_or(call.arg_type),
            quote! { &reply },
            quote! { buf },
            "the dispatch buffer",
        );
        let arg = &call.arg;
        quote! {
            #ordinal => {
                let decoded = #decode;
                match decoded {
                    Err(error) => h.settle(claim.id, Err(error)),
                    Ok(#arg) => {
                        match <#descriptor as ::ridl_rt::contract::Query>::require(&#arg) {
                            Err(()) => h.settle(
                                claim.id,
                                Err(::ridl_rt::error::CallError::Contract(
                                    ::ridl_rt::error::Contract::PreconditionFailed,
                                )),
                            ),
                            Ok(()) => {
                                let reply = p.#method(&#arg);
                                match <#descriptor as ::ridl_rt::contract::Query>::ensure(
                                    &#arg,
                                    &reply,
                                ) {
                                    Err(()) => h.settle(
                                        claim.id,
                                        Err(::ridl_rt::error::CallError::Contract(
                                            ::ridl_rt::error::Contract::ContractBroken,
                                        )),
                                    ),
                                    Ok(()) => {
                                        let bytes = #encode;
                                        h.settle(claim.id, Ok(bytes))
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    });

    let doc = format!(
        "Settles every claim of interface `{iface_name}` that is waiting, and \
         returns how many were settled.\n\nIt does not wait: it makes one pass \
         over the claims the handler already has and returns. The loop that \
         calls it belongs to the application or to the runtime.\n\n`buf` is \
         caller-owned and must be at least `{iface_name}::MAX_BUFFER_SIZE` \
         bytes, because a reply is encoded into the same buffer as the \
         arguments. A shorter buffer returns `0` without consuming a claim, so \
         the caller can retry with a correctly sized one.\n\nEvery claim that \
         is taken is settled, including one whose interface number or ordinal \
         this interface does not recognise, which settles \
         `Contract::UnknownInteraction`. A claim is counted only once \
         `Handler::settle` has accepted it; a `SettleError` is left to the \
         handler, which already owns that claim's settlement, and the pass \
         continues with the next claim.\n\nA command is settled `Ok(&[])` \
         once its arguments and its `require` clauses pass and **before** the \
         application's method runs, because a command's acknowledgment is a \
         delivery acknowledgment and not a completion one (ridl §6.1, and \
         `Handler`'s own contract). A query is settled after the application \
         returns, because its settlement carries the reply."
    );

    quote! {
        #[doc = #doc]
        pub fn dispatch<H, P>(h: &mut H, p: &mut P, buf: &mut [u8]) -> usize
        where
            H: ::ridl_rt::port::Handler,
            P: Provider,
        {
            if buf.len() < super::#iface::MAX_BUFFER_SIZE {
                return 0;
            }
            let mut settled = 0usize;
            loop {
                let Ok(Some(claim)) = h.next_claim(buf) else {
                    return settled;
                };
                let settlement = if claim.iface != #number {
                    h.settle(
                        claim.id,
                        Err(::ridl_rt::error::CallError::Contract(
                            ::ridl_rt::error::Contract::UnknownInteraction,
                        )),
                    )
                } else {
                    match claim.ord {
                        #(#command_arms)*
                        #(#query_arms)*
                        _ => h.settle(
                            claim.id,
                            Err(::ridl_rt::error::CallError::Contract(
                                ::ridl_rt::error::Contract::UnknownInteraction,
                            )),
                        ),
                    }
                };
                if settlement.is_ok() {
                    settled += 1;
                }
            }
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
fn single_param_name(params: &[v2::Param]) -> Ident {
    params
        .first()
        .map_or_else(|| ident("value"), |param| ident(&snake_case(&param.name)))
}

/// The snake case of a declared name: `Cabin` becomes `cabin`, `setLevel`
/// becomes `set_level`. It is the only spelling of a generated module or
/// method name.
fn snake_case(name: &str) -> String {
    let mut out = String::new();
    let mut previous_was_lower = false;
    for ch in name.chars() {
        if ch == '_' {
            out.push('_');
            previous_was_lower = false;
            continue;
        }
        if ch.is_uppercase() {
            if previous_was_lower {
                out.push('_');
            }
            out.extend(ch.to_lowercase());
            previous_was_lower = false;
        } else {
            out.push(ch);
            previous_was_lower = ch.is_lowercase() || ch.is_numeric();
        }
    }
    out
}
