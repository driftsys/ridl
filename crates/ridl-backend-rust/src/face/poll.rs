//! The internal poll face the futures are built over. `pub(crate)`: the
//! public surface is the futures (the async face design, note F-12).
//!
//! The emitter of `send_<name>`, `poll_<name>_ack`, `poll_<name>_reply` and
//! `poll_next_event`. The face as a whole is described in the documentation
//! of the parent module, `face.rs`.

use super::{
    Call, Member, correlation_type, encode_into, future_type, ident, interface_number,
    payload_buffer, read_fn, send_fn, ty,
};
use proc_macro2::{Ident, TokenStream};
use quote::quote;

/// The send, the outcome read of each call, and the event read, as functions
/// over a bare port, so a future that holds `&mut P` can call them.
pub(super) fn plumbing(
    iface: &Ident,
    iface_name: &str,
    events: &[(Member, &str)],
    commands: &[Call],
    queries: &[Call],
) -> Vec<TokenStream> {
    let number = interface_number(iface);
    let mut items = Vec::new();
    for call in commands {
        items.push(send(
            &number,
            call,
            quote! { ::ridl_rt::contract::Command },
            quote! { command },
            "command",
        ));
        items.push(poll_ack(call));
    }
    for call in queries {
        items.push(send(
            &number,
            call,
            quote! { ::ridl_rt::contract::Query },
            quote! { query },
            "query",
        ));
        items.push(poll_reply(call));
    }
    if !events.is_empty() {
        items.push(poll_next_event(iface, iface_name, events));
    }
    items
}

/// One call's send: evaluate `require`, encode the argument, hand it to the
/// `Caller` port once.
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
    let future = future_type(call);
    let name = send_fn(call);
    let arg = &call.arg;
    let path = ty(call.arg_type);
    let buffer = payload_buffer(call.arg_type);
    let encode = encode_into(
        call.arg_type,
        quote! { __arg },
        quote! { &mut buf },
        "the argument buffer",
    );
    let doc = format!(
        "Sends {kind} `{}` once and returns the correlation of its outcome, or \
         the send's failure. A `require` clause that fails is \
         `SendError::Contract(Contract::PreconditionFailed)` and nothing is \
         sent. `Client::{method}` calls it when the method runs, and \
         `{future}` calls it again on each poll while the port answers \
         `SendError::Busy`.",
        member.declared
    );
    // The port parameter and the rebinding carry emitter-owned names, so a
    // ridl parameter named `port`, `buf` or `bytes` collides with nothing in
    // this body (see `call_method`).
    quote! {
        #[doc = #doc]
        pub(crate) fn #name<P: ::ridl_rt::port::Caller>(
            __port: &mut P,
            #arg: &#path,
        ) -> ::core::result::Result<#correlation, ::ridl_rt::port::SendError> {
            let __arg = #arg;
            <#descriptor as #contract_trait>::require(__arg).map_err(|()| {
                ::ridl_rt::port::SendError::Contract(
                    ::ridl_rt::error::Contract::PreconditionFailed,
                )
            })?;
            let mut buf = #buffer;
            let bytes = #encode;
            __port.#port_method(#number, #ordinal, bytes).map(#correlation)
        }
    }
}

/// One command's acknowledgment read.
fn poll_ack(call: &Call) -> TokenStream {
    let member = &call.member;
    let correlation = correlation_type(call);
    let future = future_type(call);
    let name = read_fn(call);
    let doc = format!(
        "Reads command `{}`'s delivery acknowledgment: `Some` once it is \
         known, `None` while it is not. It does not wait; `{future}` polls it.",
        member.declared
    );
    quote! {
        #[doc = #doc]
        pub(crate) fn #name<P: ::ridl_rt::port::Caller>(
            port: &mut P,
            correlation: #correlation,
        ) -> ::core::option::Option<
            ::core::result::Result<(), ::ridl_rt::error::CallError>,
        > {
            port.ack(correlation.0)
        }
    }
}

/// One query's reply read, with the reply's own check.
fn poll_reply(call: &Call) -> TokenStream {
    let member = &call.member;
    let reply = call.reply_type.unwrap_or(call.arg_type);
    let correlation = correlation_type(call);
    let future = future_type(call);
    let name = read_fn(call);
    let path = ty(reply);
    let buffer = payload_buffer(reply);
    let doc = format!(
        "Reads query `{}`'s reply: `Ok(Some)` once it is known, `Ok(None)` \
         while it is not, and the port's own failure when the read itself \
         fails. It does not wait; `{future}` polls it.",
        member.declared
    );
    quote! {
        #[doc = #doc]
        pub(crate) fn #name<P: ::ridl_rt::port::Caller>(
            port: &mut P,
            correlation: #correlation,
        ) -> ::core::result::Result<
            ::core::option::Option<
                ::core::result::Result<#path, ::ridl_rt::error::CallError>,
            >,
            ::ridl_rt::port::ReadError,
        > {
            let mut buf = #buffer;
            match port.reply(correlation.0, &mut buf)? {
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
    }
}

/// The event read `NextEvent` polls: one occurrence of any subscribed event,
/// routed to its variant by ordinal.
fn poll_next_event(iface: &Ident, iface_name: &str, events: &[(Member, &str)]) -> TokenStream {
    let buffer = quote! { [0u8; super::#iface::EVENT_SOURCE_BUFFER_SIZE] };
    let arms = events.iter().map(|(member, payload)| {
        let ordinal = &member.ordinal;
        let variant = ident(member.camel);
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
        "Reads the next occurrence of any subscribed event of interface \
         `{iface_name}`, routed to its variant by ordinal: `Ok(None)` when \
         none is waiting. It does not wait; `NextEvent` polls it. An \
         occurrence of another interface is reported as \
         `Contract::UnknownInteraction`, for the reason `Client::next_event` \
         gives."
    );
    quote! {
        #[doc = #doc]
        pub(crate) fn poll_next_event<P: ::ridl_rt::port::EventSource>(
            port: &mut P,
        ) -> ::core::result::Result<
            ::core::option::Option<Event>,
            ::ridl_rt::port::ReadError,
        > {
            let mut buf = #buffer;
            let Some(occurrence) = port.next(&mut buf)? else {
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
    }
}
