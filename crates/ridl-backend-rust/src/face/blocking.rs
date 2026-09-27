//! The blocking client and `blocking::serve`: the emitter of the `blocking`
//! module, under the emitted crate's `std` feature (the async face design,
//! notes F-10 and F-11). The face as a whole is described in the
//! documentation of the parent module, `face.rs`.
//!
//! Every waiting method here — a command, a query, `next_event` and `serve` —
//! is `ridl_rt::task::block_on` over the async face's future, pinned locally,
//! so the two clients cannot diverge: the future is the one source of a call's
//! behaviour, and this module adds the thread's wait and one timeout per
//! client. A signal read and a `subscribe_*` never wait, and delegate to the
//! async client unchanged. The module is emitted only when the
//! interface declares something that waits — an event, a command or a query;
//! a signal-only interface's `Client` never blocks, so it gets no `blocking`
//! module.

use super::{Call, Member, client_bounds, ident, type_path};
use proc_macro2::TokenStream;
use quote::quote;

/// A type reference as the blocking module names it: one module deeper than
/// the face module, so a same-package declaration is `super::super::<Name>`,
/// and a cross-package reference keeps its `crate::`-anchored path.
fn ty(reference: &str) -> TokenStream {
    if reference.contains('.') {
        type_path(reference)
    } else {
        let id = ident(reference);
        quote! { super::super::#id }
    }
}

/// The `blocking` module, or nothing for an interface with no event, command
/// or query.
pub(super) fn blocking(
    iface_name: &str,
    signals: &[(Member, &str)],
    events: &[(Member, &str)],
    commands: &[Call],
    queries: &[Call],
) -> Option<TokenStream> {
    let has_calls = !commands.is_empty() || !queries.is_empty();
    if events.is_empty() && !has_calls {
        return None;
    }
    let bounds = client_bounds(signals, events, commands, queries);
    let mut items = vec![client(
        iface_name, &bounds, signals, events, commands, queries,
    )];
    if has_calls {
        items.push(serve(iface_name));
    }

    let doc = format!(
        "The blocking face of interface `{iface_name}`, under the crate's `std` \
         feature: `Client` and `serve` as `ridl_rt::task::block_on` over the \
         async face's futures, each bounded by a timeout. What a call does is \
         the future's; this module adds the thread's wait and the timeout."
    );
    Some(quote! {
        #[doc = #doc]
        #[cfg(feature = "std")]
        pub mod blocking {
            #(#items)*
        }
    })
}

fn client(
    iface_name: &str,
    bounds: &[TokenStream],
    signals: &[(Member, &str)],
    events: &[(Member, &str)],
    commands: &[Call],
    queries: &[Call],
) -> TokenStream {
    let mut methods: Vec<TokenStream> = Vec::new();

    for (member, payload) in signals {
        let method = &member.method;
        let path = ty(payload);
        let doc = format!(
            "Reads signal `{}`, as `Client::{method}` does: a read never waits.",
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
                self.inner.#method()
            }
        });
    }

    for (member, _) in events {
        let method = ident(&format!("subscribe_{}", member.method));
        let doc = format!(
            "Starts delivery of event `{}`, as `Client::{method}` does.",
            member.declared
        );
        methods.push(quote! {
            #[doc = #doc]
            pub fn #method(
                &mut self,
            ) -> ::core::result::Result<(), ::ridl_rt::port::SubscribeError> {
                self.inner.#method()
            }
        });
    }

    if !events.is_empty() {
        let doc = format!(
            "Waits for the next occurrence of any subscribed event of interface \
             `{iface_name}` and returns it, routed to its variant by ordinal, \
             or `Ok(None)` when this client's timeout passes first. With no \
             timeout it returns only with an occurrence or a read failure. It \
             is `block_on` over `Client::next_event`."
        );
        methods.push(quote! {
            #[doc = #doc]
            pub fn next_event(
                &mut self,
            ) -> ::core::result::Result<
                ::core::option::Option<super::Event>,
                ::ridl_rt::port::ReadError,
            > {
                let __deadline = self.timeout.map(|timeout| ::std::time::Instant::now() + timeout);
                let mut __next = self.inner.next_event();
                match ::ridl_rt::task::block_on(&mut __next, __deadline) {
                    Some(Ok(event)) => Ok(Some(event)),
                    Some(Err(error)) => Err(error),
                    None => Ok(None),
                }
            }
        });
    }

    for call in commands {
        methods.push(call_method(call, "command", quote! { () }, "Undelivered"));
    }

    for call in queries {
        methods.push(call_method(
            call,
            "query",
            ty(call.reply_type.unwrap_or(call.arg_type)),
            "Timeout",
        ));
    }

    let doc = format!(
        "The blocking consumer face of interface `{iface_name}`: the async \
         `Client` with a timeout, over the same ports. Every call is \
         `ridl_rt::task::block_on` over the async call's future, so a call \
         parks the calling thread until its outcome, the member's bound on \
         the port's clock, or this client's timeout, whichever comes first. \
         The timeout is `None` until `with_timeout` or `set_timeout` sets it: \
         a member with a `max` is then bounded by the future alone, and one \
         with none waits without a bound."
    );
    quote! {
        #[doc = #doc]
        pub struct Client<P: #(#bounds)+*> {
            inner: super::Client<P>,
            timeout: ::core::option::Option<::std::time::Duration>,
        }

        impl<P: #(#bounds)+*> Client<P> {
            /// Binds the face to a port, with no timeout. The port is held
            /// by value: pass a handle, or a `&mut` borrow of one.
            pub fn new(port: P) -> Self {
                Client {
                    inner: super::Client::new(port),
                    timeout: None,
                }
            }

            /// Sets the timeout every waiting method of this client is
            /// bounded by, and returns the client. A timeout shorter than a
            /// member's `max` is accepted: the earlier of the two ends the
            /// call.
            pub fn with_timeout(mut self, timeout: ::std::time::Duration) -> Self {
                self.timeout = Some(timeout);
                self
            }

            /// Sets or clears the timeout every waiting method of this
            /// client is bounded by.
            pub fn set_timeout(&mut self, timeout: ::core::option::Option<::std::time::Duration>) {
                self.timeout = timeout;
            }

            #(#methods)*
        }
    }
}

/// One blocking call: the async call's future, waited on until it resolves
/// or the client's timeout passes. At the timeout the future is asked
/// whether the call was sent, and the answer is the one the future gives at
/// its own deadline (note F-11): unsent is `SendError::Busy`, sent is the
/// port's expired outcome for the call's kind. Dropping the future then
/// forgets a sent call.
///
/// The parameter keeps the ridl name, as `Client`'s does, and the locals are
/// `__deadline` and `__call`, which no ridl identifier can be. The deadline
/// is computed inline rather than by a helper method, because a method of
/// `blocking::Client` with a name of the emitter's own would collide with a
/// member of that name: `face_compile.rs` declares a command `deadline`.
fn call_method(call: &Call, kind: &str, output: TokenStream, expired: &str) -> TokenStream {
    let member = &call.member;
    let method = &member.method;
    let arg = &call.arg;
    let path = ty(call.arg_type);
    let doc = format!(
        "Sends {kind} `{}` and waits for its outcome, as `block_on` over \
         `Client::{method}`. At this client's timeout a call that was never \
         sent, because no slot was free, is `ClientError::Send(SendError::Busy)`, \
         and one that was sent is the port's expired outcome, \
         `Transport::{expired}`; either way nothing is left waiting at the port.",
        member.declared,
    );
    let expired = ident(expired);
    let expired = quote! { ::ridl_rt::error::Transport::#expired };
    quote! {
        #[doc = #doc]
        pub fn #method(
            &mut self,
            #arg: #path,
        ) -> ::core::result::Result<#output, ::ridl_rt::error::ClientError> {
            let __deadline = self.timeout.map(|timeout| ::std::time::Instant::now() + timeout);
            let mut __call = self.inner.#method(#arg);
            match ::ridl_rt::task::block_on(&mut __call, __deadline) {
                Some(outcome) => outcome,
                None if __call.sent() => Err(::ridl_rt::error::ClientError::Call(
                    ::ridl_rt::error::CallError::Transport(#expired),
                )),
                None => Err(::ridl_rt::error::ClientError::Send(
                    ::ridl_rt::port::SendError::Busy,
                )),
            }
        }
    }
}

/// `blocking::serve`: `block_on` over `serve`'s future, which never resolves
/// to `Ok`, so `Ok(())` here means the timeout passed (note F-11).
fn serve(iface_name: &str) -> TokenStream {
    let doc = format!(
        "Serves interface `{iface_name}`'s commands and queries with `p`, over \
         the handler port `h`, on the calling thread, until the handler port \
         fails or `timeout` passes: it is `block_on` over `serve`. A failure \
         is returned as `serve`'s future resolves to it; the timeout is \
         `Ok(())`, so a loop that also does other work can call this \
         repeatedly. With `None` it returns only on a failure. `h` is dropped \
         when this returns."
    );
    quote! {
        #[doc = #doc]
        pub fn serve<H, P>(
            h: H,
            p: &mut P,
            timeout: ::core::option::Option<::std::time::Duration>,
        ) -> ::core::result::Result<(), ::ridl_rt::error::ProviderError>
        where
            H: ::ridl_rt::port::Handler + ::ridl_rt::port::Wakeable,
            P: super::Provider,
        {
            let __deadline = timeout.map(|timeout| ::std::time::Instant::now() + timeout);
            let mut __serve = super::serve(h, p);
            match ::ridl_rt::task::block_on(&mut __serve, __deadline) {
                Some(Ok(never)) => match never {},
                Some(Err(error)) => Err(error),
                None => Ok(()),
            }
        }
    }
}
