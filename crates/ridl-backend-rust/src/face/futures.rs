//! The futures: the emitter of one named future per command and per query,
//! `<Name>Call`; of the stream value `<Name>Stream` a streamed query's future
//! resolves to; and of `NextEvent` when the interface declares an event. The
//! face as a whole is described in the documentation of the parent module,
//! `face.rs`.

use super::{
    Call, Member, correlation_type, encoding_bounds, future_type, ident, interface_number,
    phase_type, read_fn, send_fn, stream_type, ty,
};
use proc_macro2::{Ident, TokenStream};
use quote::quote;

/// The named future of every call — commands, then queries, then the
/// streamed queries, each with its stream value — and, when the interface
/// declares an event, `NextEvent` after them.
pub(super) fn futures(
    iface: &Ident,
    iface_name: &str,
    payloads: &[&str],
    events: &[(Member, &str)],
    commands: &[Call],
    queries: &[Call],
    streams: &[Call],
) -> Vec<TokenStream> {
    // Each future holds the port and reads the encoding of the bytes it sends
    // or checks from it, so each impl that encodes or decodes carries the
    // face's `where` clause.
    let encoding = encoding_bounds(&ident("P"), iface, payloads);
    let mut items = Vec::new();
    for call in commands {
        items.push(call_future(call, "command", &encoding));
    }
    for call in queries {
        items.push(call_future(call, "query", &encoding));
    }
    if !streams.is_empty() {
        items.push(stream_state());
    }
    for call in streams {
        items.push(stream_call_future(call, &encoding));
        items.push(stream_value(call, &encoding));
    }
    if !events.is_empty() {
        items.push(next_event_future(iface, iface_name, &encoding));
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
fn call_future(call: &Call, kind: &str, encoding: &TokenStream) -> TokenStream {
    let member = &call.member;
    let method = &member.method;
    let arg_path = ty(call.arg_type);
    let future = future_type(call);
    let phase = phase_type(call);
    let correlation = correlation_type(call);
    let send = send_fn(call);
    let read = read_fn(call);
    let bounds = quote! {
        ::ridl_rt::port::Encoded
            + ::ridl_rt::port::Caller
            + ::ridl_rt::port::Clock
            + ::ridl_rt::port::Wakeable
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
    let polled_after = format!("`{future}` polled after completion");
    // The phase enum, its variants, `expired` and `sent` are private, so they
    // carry no doc. The phase is where the call is, as its future's `poll`
    // moves it: `Unsent` when the port answered `SendError::Busy` and the
    // argument is kept for the retry; `Waiting` when sent, for the outcome
    // under this correlation; `Failed` when the send failed before anything
    // was sent, which the first poll reports; `Done` once the output was
    // taken. `expired` says whether the call's bound has passed on the port's
    // clock; a call with no bound never expires here. `sent` says whether the
    // call was sent and waits for its outcome: the blocking client asks this
    // when `block_on` gives up, to answer as the future would at its own
    // deadline, and it is under `std` with that client, so a build without
    // the feature has no unused item.

    quote! {
        #[doc = #doc]
        #[must_use = #must_use]
        pub struct #future<'a, P: #bounds> {
            port: &'a mut P,
            phase: #phase,
            deadline: ::core::option::Option<::ridl_rt::sample::Timestamp>,
        }

        enum #phase {
            Unsent(#arg_path),
            Waiting(#correlation),
            Failed(::ridl_rt::port::SendError),
            Done,
        }

        impl<P: #bounds> #future<'_, P>
        where
            #encoding
        {
            fn expired(&self) -> bool {
                self.deadline.is_some_and(|deadline| self.port.now() > deadline)
            }

            #[cfg(feature = "std")]
            fn sent(&self) -> bool {
                matches!(self.phase, #phase::Waiting(_))
            }
        }

        impl<P: #bounds> ::core::future::Future for #future<'_, P>
        where
            #encoding
        {
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

        // A `Drop` impl must repeat its type's bounds exactly, so this one
        // carries no `where` clause; it forgets a waiting call at the port and
        // encodes and decodes nothing.
        impl<P: #bounds> ::core::ops::Drop for #future<'_, P> {
            fn drop(&mut self) {
                if let #phase::Waiting(correlation) = &self.phase {
                    self.port.forget(correlation.0);
                }
            }
        }
    }
}

/// The port bounds of a streamed query's future and of its stream value: a
/// call's, plus the stream extension.
fn stream_bounds() -> TokenStream {
    quote! {
        ::ridl_rt::port::Encoded
            + ::ridl_rt::port::Caller
            + ::ridl_rt::port::StreamCaller
            + ::ridl_rt::port::Clock
            + ::ridl_rt::port::Wakeable
    }
}

/// The state of every stream value of the module, emitted once when the
/// interface declares a streamed query. It is a fixed name, which no
/// member-derived name can be: every one of those ends with a call suffix.
fn stream_state() -> TokenStream {
    // `First` holds the length of the chunk the call's future pulled, kept in
    // the stream's buffer for the first `poll_next`; `Open` pulls; `Refused`
    // holds the refusal the call's future made of a first chunk that is no
    // legal element, which the first `poll_next` yields; `Stopped` is after
    // a refusal, with the call kept until the stream is dropped, so that the
    // provider's next push reads the refusal and not an interruption; `Done`
    // is after the end or the provider's failure, once the call is
    // forgotten. Private: no doc.
    quote! {
        #[derive(Clone, Copy, PartialEq, Eq)]
        enum StreamState {
            First(usize),
            Open,
            Refused(::ridl_rt::error::CallError),
            Stopped,
            Done,
        }
    }
}

/// A streamed query's future: [`call_future`]'s state machine, where the
/// waiting phase pulls the reply stream instead of reading a reply, and the
/// output is the stream value.
///
/// The port is held as an `Option`, because the future has a `Drop` impl, so
/// a field cannot be moved out of it, and the stream value needs the `&'a mut
/// P` the future holds: the resolving poll takes it. The port is `None` only
/// once the future resolved, when its phase is `Done` and a poll panics
/// before it reaches the port.
///
/// The first chunk is pulled into a stack buffer and moved into the stream
/// value, so it is not lost; no second pull is made for it. A first chunk
/// longer than the element's buffer is refused at once, and the refusal is
/// handed to the stream value, which yields it on its first poll and keeps
/// the call until it is dropped, as it does for a refusal of its own. The
/// future has no `sent` helper: the blocking module carries nothing for a
/// stream.
fn stream_call_future(call: &Call, encoding: &TokenStream) -> TokenStream {
    let member = &call.member;
    let method = &member.method;
    let element = ty(call.reply_type.unwrap_or(call.arg_type));
    let arg_path = ty(call.arg_type);
    let future = future_type(call);
    let stream = stream_type(call);
    let phase = phase_type(call);
    let correlation = correlation_type(call);
    let send = send_fn(call);
    let bounds = stream_bounds();
    let doc = format!(
        "The future of query `{}`, returned by `Client::{method}`. It resolves \
         to `{stream}`, the reply stream, once its first element or its end \
         arrived; to `ClientError::Send` when the call was not sent, including \
         `SendError::Busy` when no slot was free within the call's bound; to \
         `Transport::Timeout` when the call was sent and its bound passed \
         before the first element or the end; and to the outcome the provider \
         settled when it failed the call before any element. A first chunk \
         that is no legal element resolves it to the stream, which yields \
         the refusal, `Transport::Corrupt`, as its one error. Each poll \
         registers its interest, reads the port once, and returns. Dropping \
         the future while it waits calls `Caller::forget` on the call, which \
         interrupts the stream; a future that resolved to an error has \
         already done so, and one that resolved to the stream handed the call \
         to it. Polling it again after it resolved panics.",
        member.declared
    );
    let must_use = "the query was sent, or waits for a slot; its stream is taken only when \
                    the future is polled";
    let polled_after = format!("`{future}` polled after completion");
    // The phase enum is the one `call_future` describes, with `Done` also the
    // phase after the stream was handed out. A chunk longer than the
    // element's buffer is no legal element: the stream is aborted with
    // `Transport::Corrupt` and handed out in `Refused`, so `ReadError::Short`
    // never reaches the application and the call is kept for the provider to
    // read the refusal.
    quote! {
        #[doc = #doc]
        #[must_use = #must_use]
        pub struct #future<'a, P: #bounds> {
            port: ::core::option::Option<&'a mut P>,
            phase: #phase,
            deadline: ::core::option::Option<::ridl_rt::sample::Timestamp>,
        }

        enum #phase {
            Unsent(#arg_path),
            Waiting(#correlation),
            Failed(::ridl_rt::port::SendError),
            Done,
        }

        impl<P: #bounds> #future<'_, P>
        where
            #encoding
        {
            fn expired(&self) -> bool {
                match (&self.port, self.deadline) {
                    (Some(port), Some(deadline)) => port.now() > deadline,
                    _ => false,
                }
            }

            fn port(&mut self) -> &mut P {
                match self.port.as_deref_mut() {
                    Some(port) => port,
                    None => panic!(#polled_after),
                }
            }
        }

        impl<'a, P: #bounds> ::core::future::Future for #future<'a, P>
        where
            #encoding
        {
            type Output = ::core::result::Result<#stream<'a, P>, ::ridl_rt::error::ClientError>;

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
                            if this.expired() {
                                return ::core::task::Poll::Ready(Err(
                                    ::ridl_rt::error::ClientError::Send(
                                        ::ridl_rt::port::SendError::Busy,
                                    ),
                                ));
                            }
                            let port = this.port();
                            port.wake_on(::ridl_rt::port::Interest::Slot, cx.waker());
                            match #send(port, &__arg) {
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
                            let expired = this.expired();
                            let port = this.port();
                            port.wake_on(
                                ::ridl_rt::port::Interest::Stream(
                                    correlation.0,
                                    ::ridl_rt::port::REPLY_STREAM,
                                ),
                                cx.waker(),
                            );
                            let mut __buf =
                                <#element as ::ridl_rt::payload::Payload<P::Encoding>>::BUFFER;
                            let buf: &mut [::core::primitive::u8] =
                                ::core::convert::AsMut::as_mut(&mut __buf);
                            let state = match port.pull(
                                correlation.0,
                                ::ridl_rt::port::REPLY_STREAM,
                                buf,
                            ) {
                                Ok(::ridl_rt::port::Pull::Chunk(len)) => StreamState::First(len),
                                Ok(::ridl_rt::port::Pull::Pending) => {
                                    if !expired {
                                        this.phase = #phase::Waiting(correlation);
                                        return ::core::task::Poll::Pending;
                                    }
                                    port.forget(correlation.0);
                                    return ::core::task::Poll::Ready(Err(
                                        ::ridl_rt::error::ClientError::Call(
                                            ::ridl_rt::error::CallError::Transport(
                                                ::ridl_rt::error::Transport::Timeout,
                                            ),
                                        ),
                                    ));
                                }
                                Ok(::ridl_rt::port::Pull::End) => {
                                    port.forget(correlation.0);
                                    StreamState::Done
                                }
                                Ok(::ridl_rt::port::Pull::Failed(error)) => {
                                    port.forget(correlation.0);
                                    return ::core::task::Poll::Ready(Err(
                                        ::ridl_rt::error::ClientError::Call(error),
                                    ));
                                }
                                Err(::ridl_rt::port::ReadError::Short { .. }) => {
                                    let error = ::ridl_rt::error::CallError::Transport(
                                        ::ridl_rt::error::Transport::Corrupt,
                                    );
                                    let _ = port.abort(
                                        correlation.0,
                                        ::ridl_rt::port::REPLY_STREAM,
                                        error,
                                    );
                                    StreamState::Refused(error)
                                }
                                Err(error) => {
                                    port.forget(correlation.0);
                                    return ::core::task::Poll::Ready(Err(
                                        ::ridl_rt::error::ClientError::Read(error),
                                    ));
                                }
                            };
                            let port = match this.port.take() {
                                Some(port) => port,
                                None => panic!(#polled_after),
                            };
                            return ::core::task::Poll::Ready(Ok(#stream {
                                port,
                                correlation,
                                buffer: __buf,
                                state,
                            }));
                        }
                    }
                }
            }
        }

        // A `Drop` impl must repeat its type's bounds exactly, so this one
        // carries no `where` clause; it forgets a waiting call at the port,
        // which interrupts the stream, and encodes and decodes nothing.
        impl<P: #bounds> ::core::ops::Drop for #future<'_, P> {
            fn drop(&mut self) {
                if let (Some(port), #phase::Waiting(correlation)) = (&mut self.port, &self.phase) {
                    port.forget(correlation.0);
                }
            }
        }
    }
}

/// A streamed query's stream value: the pull API over the `StreamCaller`
/// port, one element per chunk, and `async fn next` over it.
///
/// The struct holds the element's `Payload::Buffer` inline, so it carries
/// that one predicate itself, as `Serve` carries its claim buffer's; the
/// `Drop` impl repeats it.
fn stream_value(call: &Call, encoding: &TokenStream) -> TokenStream {
    let member = &call.member;
    let element = ty(call.reply_type.unwrap_or(call.arg_type));
    let future = future_type(call);
    let stream = stream_type(call);
    let correlation = correlation_type(call);
    let bounds = stream_bounds();
    let doc = format!(
        "The reply stream of query `{}`, which `{future}` resolves to. \
         `poll_next` is the pull API, the whole face on a target without \
         `std`: it registers the stream's interest, pulls one chunk, and \
         verifies and decodes it as one element; `next` awaits it. An element \
         that fails its check refuses the stream with `Contract::InvalidValue`, \
         and a chunk that is no well-formed element, or longer than any legal \
         one, refuses it with `Transport::Corrupt`: the stream is aborted with \
         that outcome, which the provider reads from its next push, and the \
         refusal is the one error the stream yields; the call is kept until \
         the stream is dropped. A stream the provider stopped yields its \
         outcome, `Transport::Interrupted` for a stop on purpose, as its one \
         error, and the call is forgotten at once, as it is after the end. \
         After the end or an error every later `poll_next` is `Ready(None)`. \
         Dropping the stream forgets the call, once, unless the end or the \
         provider's failure already did; before the end that interrupts the \
         stream (ridl §12).",
        member.declared
    );
    let must_use = "dropping the stream before its end interrupts it";
    // `finish` forgets the call once and moves to `Done`; `refuse` is the
    // receiver's refusal of ridl §12.4, an abort with the refusal's outcome,
    // whose answer is not read, then `Stopped`: the call is kept, so that the
    // provider's next push reads the refusal and not the interruption a
    // forget would make of it, and `Drop` forgets it. Both are private: no
    // doc.
    quote! {
        #[doc = #doc]
        #[must_use = #must_use]
        pub struct #stream<'a, P: #bounds>
        where
            #element: ::ridl_rt::payload::Payload<P::Encoding>,
        {
            port: &'a mut P,
            correlation: #correlation,
            buffer: <#element as ::ridl_rt::payload::Payload<P::Encoding>>::Buffer,
            state: StreamState,
        }

        impl<P: #bounds> #stream<'_, P>
        where
            #encoding
        {
            /// Pulls the next element: `Ready(Some(Ok(element)))` with the
            /// next element, `Ready(None)` once the stream ended,
            /// `Ready(Some(Err(error)))` once and then `Ready(None)` when
            /// the stream stopped or was refused, and `Pending` while no
            /// chunk is waiting, after the waker of `cx` was registered
            /// under `Interest::Stream`. A refused stream keeps its call
            /// until this value is dropped.
            pub fn poll_next(
                &mut self,
                cx: &mut ::core::task::Context<'_>,
            ) -> ::core::task::Poll<
                ::core::option::Option<
                    ::core::result::Result<#element, ::ridl_rt::error::ClientError>,
                >,
            > {
                let len = match self.state {
                    StreamState::Done | StreamState::Stopped => {
                        return ::core::task::Poll::Ready(None);
                    }
                    StreamState::Refused(error) => {
                        self.state = StreamState::Stopped;
                        return ::core::task::Poll::Ready(Some(Err(
                            ::ridl_rt::error::ClientError::Call(error),
                        )));
                    }
                    StreamState::First(len) => {
                        self.state = StreamState::Open;
                        len
                    }
                    StreamState::Open => {
                        self.port.wake_on(
                            ::ridl_rt::port::Interest::Stream(
                                self.correlation.0,
                                ::ridl_rt::port::REPLY_STREAM,
                            ),
                            cx.waker(),
                        );
                        match self.port.pull(
                            self.correlation.0,
                            ::ridl_rt::port::REPLY_STREAM,
                            ::core::convert::AsMut::as_mut(&mut self.buffer),
                        ) {
                            Ok(::ridl_rt::port::Pull::Chunk(len)) => len,
                            Ok(::ridl_rt::port::Pull::Pending) => {
                                return ::core::task::Poll::Pending;
                            }
                            Ok(::ridl_rt::port::Pull::End) => {
                                self.finish();
                                return ::core::task::Poll::Ready(None);
                            }
                            Ok(::ridl_rt::port::Pull::Failed(error)) => {
                                self.finish();
                                return ::core::task::Poll::Ready(Some(Err(
                                    ::ridl_rt::error::ClientError::Call(error),
                                )));
                            }
                            Err(::ridl_rt::port::ReadError::Short { .. }) => {
                                return ::core::task::Poll::Ready(Some(Err(self.refuse(
                                    ::ridl_rt::error::CallError::Transport(
                                        ::ridl_rt::error::Transport::Corrupt,
                                    ),
                                ))));
                            }
                            Err(error) => {
                                self.finish();
                                return ::core::task::Poll::Ready(Some(Err(
                                    ::ridl_rt::error::ClientError::Read(error),
                                )));
                            }
                        }
                    }
                };
                let buf: &[::core::primitive::u8] = ::core::convert::AsRef::as_ref(&self.buffer);
                let element = match ::ridl_rt::payload::Ref::<#element, P::Encoding>::verify(
                    &buf[..len],
                ) {
                    Ok(checked) => Ok(checked.decode()),
                    Err(::ridl_rt::payload::VerifyError::Contract(violation)) => {
                        Err(::ridl_rt::error::CallError::Contract(
                            ::ridl_rt::error::Contract::InvalidValue(violation),
                        ))
                    }
                    Err(_) => Err(::ridl_rt::error::CallError::Transport(
                        ::ridl_rt::error::Transport::Corrupt,
                    )),
                };
                ::core::task::Poll::Ready(Some(match element {
                    Ok(element) => Ok(element),
                    Err(error) => Err(self.refuse(error)),
                }))
            }

            /// Awaits the next element: `poll_next` as a future.
            pub async fn next(
                &mut self,
            ) -> ::core::option::Option<
                ::core::result::Result<#element, ::ridl_rt::error::ClientError>,
            > {
                ::core::future::poll_fn(|cx| self.poll_next(cx)).await
            }

            fn finish(&mut self) {
                self.state = StreamState::Done;
                self.port.forget(self.correlation.0);
            }

            fn refuse(&mut self, error: ::ridl_rt::error::CallError) -> ::ridl_rt::error::ClientError {
                let _ = self.port.abort(
                    self.correlation.0,
                    ::ridl_rt::port::REPLY_STREAM,
                    error,
                );
                self.state = StreamState::Stopped;
                ::ridl_rt::error::ClientError::Call(error)
            }
        }

        // A `Drop` impl must repeat its type's bounds exactly: the struct's
        // one predicate, for its buffer field. It forgets a call whose stream
        // is not done: before the end that interrupts the stream, and after a
        // refusal it releases the call the refusal kept.
        impl<P: #bounds> ::core::ops::Drop for #stream<'_, P>
        where
            #element: ::ridl_rt::payload::Payload<P::Encoding>,
        {
            fn drop(&mut self) {
                if self.state != StreamState::Done {
                    self.port.forget(self.correlation.0);
                }
            }
        }
    }
}

/// The event future: no deadline, because an event has no response bound,
/// and no `Drop`, because it holds nothing at the port (note F-10).
fn next_event_future(iface: &Ident, iface_name: &str, encoding: &TokenStream) -> TokenStream {
    let number = interface_number(iface);
    let bounds = quote! {
        ::ridl_rt::port::Encoded + ::ridl_rt::port::EventSource + ::ridl_rt::port::Wakeable
    };
    let doc = format!(
        "The future of `ridl_rt::face::Events::next_event` on the `Client` of \
         interface `{iface_name}`. Each \
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

        impl<P: #bounds> ::core::future::Future for NextEvent<'_, P>
        where
            #encoding
        {
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
