//! The futures: the emitter of one named future per command and per query,
//! `<Name>Call`, and of `NextEvent` when the interface declares an event. The
//! face as a whole is described in the documentation of the parent module,
//! `face.rs`.

use super::{
    Call, Member, correlation_type, future_type, interface_number, phase_type, read_fn, send_fn, ty,
};
use proc_macro2::{Ident, TokenStream};
use quote::quote;

/// The named future of every call — commands, then queries — and, when the
/// interface declares an event, `NextEvent` after them.
pub(super) fn futures(
    iface: &Ident,
    iface_name: &str,
    events: &[(Member, &str)],
    commands: &[Call],
    queries: &[Call],
) -> Vec<TokenStream> {
    let mut items = Vec::new();
    for call in commands {
        items.push(call_future(call, "command"));
    }
    for call in queries {
        items.push(call_future(call, "query"));
    }
    if !events.is_empty() {
        items.push(next_event_future(iface, iface_name));
    }
    items
}

/// One call's future: the state machine of note F-4, with the deadline rule
/// of notes F-2 and F-3.
///
/// The phases are `Unsent` with the argument, kept for the retry while the
/// port answers `Busy`; `Waiting` with the correlation; `Failed` with a send
/// error other than `Busy`, which the first poll reports; and `Done`. Every
/// poll registers the interest of its phase, then reads the port, then
/// returns (note F-5). Leaving `Waiting` inside `poll` forgets the call
/// there, so `Drop` forgets only a future that is still waiting.
fn call_future(call: &Call, kind: &str) -> TokenStream {
    let member = &call.member;
    let method = &member.method;
    let arg_path = ty(call.arg_type);
    let future = future_type(call);
    let phase = phase_type(call);
    let correlation = correlation_type(call);
    let send = send_fn(call);
    let read = read_fn(call);
    let bounds = quote! {
        ::ridl_rt::port::Caller + ::ridl_rt::port::Clock + ::ridl_rt::port::Wakeable
    };

    // A sent call whose bound passes resolves to the port's own expired
    // variant for its kind (note F-3).
    let not_yet = |expired: TokenStream| {
        quote! {
            if !this.expired() {
                this.phase = #phase::Waiting(correlation);
                return ::core::task::Poll::Pending;
            }
            Err(::ridl_rt::error::ClientError::Call(
                ::ridl_rt::error::CallError::Transport(#expired),
            ))
        }
    };
    let (output, read_outcome) = match call.reply_type {
        None => {
            let not_yet = not_yet(quote! { ::ridl_rt::error::Transport::Undelivered });
            (
                quote! { () },
                quote! {
                    match #read(&mut *this.port, correlation) {
                        Some(outcome) => outcome.map_err(::ridl_rt::error::ClientError::Call),
                        None => { #not_yet }
                    }
                },
            )
        }
        Some(reply) => {
            let not_yet = not_yet(quote! { ::ridl_rt::error::Transport::Timeout });
            (
                ty(reply),
                quote! {
                    match #read(&mut *this.port, correlation) {
                        Ok(Some(outcome)) => outcome.map_err(::ridl_rt::error::ClientError::Call),
                        Err(error) => Err(::ridl_rt::error::ClientError::Read(error)),
                        Ok(None) => { #not_yet }
                    }
                },
            )
        }
    };

    let resolves_to = if call.reply_type.is_some() {
        "the decoded reply, or the outcome the provider settled"
    } else {
        "the delivery acknowledgment: `Ok(())`, or the outcome the provider settled"
    };
    let doc = format!(
        "The future of {kind} `{}`, returned by `Client::{method}`. It \
         resolves to {resolves_to}; to `ClientError::Send` when the call was \
         not sent, including `SendError::Busy` when no slot was free within \
         the call's bound; and to the port's own expired outcome when the call \
         was sent and its bound passed. Each poll registers its interest, reads \
         the port once, and returns. Dropping the future while it waits for its \
         outcome calls `Caller::forget` on the call; a future that has taken \
         its outcome has already done so. Polling it again after it resolved \
         panics.",
        member.declared
    );
    let must_use = format!(
        "the {kind} was sent, or waits for a slot; its outcome is taken only when \
         the future is polled"
    );
    let phase_doc = format!(
        "Where {kind} `{}` is, as its future's `poll` moves it.",
        member.declared
    );
    let polled_after = format!("`{future}` polled after completion");

    quote! {
        #[doc = #doc]
        #[must_use = #must_use]
        pub struct #future<'a, P: #bounds> {
            port: &'a mut P,
            phase: #phase,
            deadline: ::core::option::Option<::ridl_rt::sample::Timestamp>,
        }

        #[doc = #phase_doc]
        enum #phase {
            /// The port answered `SendError::Busy`; the argument is kept for
            /// the retry.
            Unsent(#arg_path),
            /// Sent, and waiting for the outcome under this correlation.
            Waiting(#correlation),
            /// The send failed before anything was sent; the first poll
            /// reports it.
            Failed(::ridl_rt::port::SendError),
            /// The output was taken.
            Done,
        }

        impl<P: #bounds> #future<'_, P> {
            /// Whether the call's bound has passed on the port's clock. A
            /// call with no bound never expires here.
            fn expired(&self) -> bool {
                self.deadline.is_some_and(|deadline| self.port.now() > deadline)
            }
        }

        impl<P: #bounds> ::core::future::Future for #future<'_, P> {
            type Output = ::core::result::Result<#output, ::ridl_rt::error::ClientError>;

            fn poll(
                self: ::core::pin::Pin<&mut Self>,
                cx: &mut ::core::task::Context<'_>,
            ) -> ::core::task::Poll<Self::Output> {
                let this = self.get_mut();
                loop {
                    match ::core::mem::replace(&mut this.phase, #phase::Done) {
                        #phase::Done => panic!(#polled_after),
                        #phase::Failed(error) => {
                            return ::core::task::Poll::Ready(Err(
                                ::ridl_rt::error::ClientError::Send(error),
                            ));
                        }
                        #phase::Unsent(__arg) => {
                            // The bound covers the wait for a slot: a call
                            // still unsent when it passes is not sent, even
                            // when a slot is free now.
                            if this.expired() {
                                return ::core::task::Poll::Ready(Err(
                                    ::ridl_rt::error::ClientError::Send(
                                        ::ridl_rt::port::SendError::Busy,
                                    ),
                                ));
                            }
                            this.port.wake_on(::ridl_rt::port::Interest::Slot, cx.waker());
                            match #send(&mut *this.port, &__arg) {
                                Ok(correlation) => this.phase = #phase::Waiting(correlation),
                                Err(::ridl_rt::port::SendError::Busy) => {
                                    this.phase = #phase::Unsent(__arg);
                                    return ::core::task::Poll::Pending;
                                }
                                Err(error) => {
                                    return ::core::task::Poll::Ready(Err(
                                        ::ridl_rt::error::ClientError::Send(error),
                                    ));
                                }
                            }
                        }
                        #phase::Waiting(correlation) => {
                            this.port.wake_on(
                                ::ridl_rt::port::Interest::Outcome(correlation.0),
                                cx.waker(),
                            );
                            let outcome = #read_outcome;
                            this.port.forget(correlation.0);
                            return ::core::task::Poll::Ready(outcome);
                        }
                    }
                }
            }
        }

        impl<P: #bounds> ::core::ops::Drop for #future<'_, P> {
            fn drop(&mut self) {
                if let #phase::Waiting(correlation) = &self.phase {
                    self.port.forget(correlation.0);
                }
            }
        }
    }
}

/// The event future: no deadline, because an event has no response bound,
/// and no `Drop`, because it holds nothing at the port (note F-10).
fn next_event_future(iface: &Ident, iface_name: &str) -> TokenStream {
    let number = interface_number(iface);
    let bounds = quote! { ::ridl_rt::port::EventSource + ::ridl_rt::port::Wakeable };
    let doc = format!(
        "The future of `Client::next_event` over interface `{iface_name}`. Each \
         poll registers its interest in the interface's events, reads the \
         queue once, and returns: an occurrence resolves it, and a read \
         failure resolves it with that failure. It can be polled again after \
         it resolved, for the next occurrence."
    );
    quote! {
        #[doc = #doc]
        #[must_use = "an occurrence is taken only when the future is polled"]
        pub struct NextEvent<'a, P: #bounds> {
            port: &'a mut P,
        }

        impl<P: #bounds> ::core::future::Future for NextEvent<'_, P> {
            type Output = ::core::result::Result<Event, ::ridl_rt::port::ReadError>;

            fn poll(
                self: ::core::pin::Pin<&mut Self>,
                cx: &mut ::core::task::Context<'_>,
            ) -> ::core::task::Poll<Self::Output> {
                let this = self.get_mut();
                this.port.wake_on(::ridl_rt::port::Interest::Event(#number), cx.waker());
                match poll_next_event(&mut *this.port) {
                    Ok(Some(event)) => ::core::task::Poll::Ready(Ok(event)),
                    Ok(None) => ::core::task::Poll::Pending,
                    Err(error) => ::core::task::Poll::Ready(Err(error)),
                }
            }
        }
    }
}
