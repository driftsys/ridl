//! `serve`: the emitter of the `serve` function and of its future, `Serve`.
//! The face as a whole is described in the documentation of the parent
//! module, `face.rs`.

use super::{Call, catalog_panics_doc, encoding_bounds, ident, interface_number, ty};
use proc_macro2::{Ident, Literal, TokenStream};
use quote::quote;

/// The most claims one poll of the generated `Serve` takes
/// (driftsys/ridl#568). It is emitted as the value of the interface module's
/// private `SERVE_BUDGET` and is the number the emitted documentation states.
const SERVE_BUDGET: usize = 32;

/// `serve` and its future (the async face design, note F-7): the handler
/// port's catalog is checked (ADR-0023 decision 8), then the served set is
/// registered when the function is called; each poll registers
/// `Interest::Claim`, then settles claims through `dispatch`, at most
/// `SERVE_BUDGET` of them (driftsys/ridl#568), and wakes its own waker when it
/// stopped at that bound; the handler port's failure is the value the future
/// resolves to, and the future never resolves to `Ok`.
pub(super) fn serve(
    iface: &Ident,
    iface_name: &str,
    payloads: &[&str],
    commands: &[Call],
    queries: &[Call],
    streams: &[Call],
) -> TokenStream {
    let number = interface_number(iface);
    let ordinals = commands
        .iter()
        .chain(queries.iter())
        .chain(streams.iter())
        .map(|call| &call.member.ordinal);
    let budget = Literal::usize_unsuffixed(SERVE_BUDGET);
    // The handler is bound on `StreamHandler` only when the interface
    // declares a stream member, so an interface without one is served over
    // a runtime without the stream extension.
    let stream_bound = (!streams.is_empty()).then(|| quote! { + ::ridl_rt::port::StreamHandler });
    let bounds = quote! {
        H: ::ridl_rt::port::Encoded + ::ridl_rt::port::Handler + ::ridl_rt::port::Wakeable
            #stream_bound,
        P: Provider
    };
    // The handler port states the encoding of the claim bytes this future
    // reads and of the reply bytes it writes.
    let port = ident("H");
    let encoding = encoding_bounds(&port, iface, payloads);
    // The future's claim-buffer field projects through `Sizes`, so the struct
    // itself carries that one predicate; the payload predicates sit on the
    // impls, which are what encodes and decodes. The element buffers of the
    // stream slots project through each element's `Payload`, so the struct
    // carries those predicates as well.
    let elements: Vec<&str> = streams
        .iter()
        .map(|call| call.reply_type.unwrap_or(call.arg_type))
        .collect();
    let buffer_bound = encoding_bounds(&port, iface, &elements);
    let stream_items = stream_items(streams);
    let streams_field = (!streams.is_empty()).then(|| quote! { streams: Streams<H, P>, });
    let streams_init = (!streams.is_empty()).then(|| {
        let slots = streams.iter().map(|call| &call.member.method);
        quote! {
            streams: Streams {
                __started: false,
                #(#slots: None,)*
            },
        }
    });
    let streams_arg = (!streams.is_empty()).then(|| quote! { &mut this.streams, });
    // Each stream slot is driven before the claims are taken, so that a
    // stream is not held back while callers keep sending. A driven stream
    // that may have more to send now, and a stream `dispatch` has just
    // started, which no poll of its source has registered a waker for, make
    // the future wake itself, as a poll that stopped at the bound does.
    let drive = streams.iter().map(|call| {
        let method = &call.member.method;
        let source = ident(call.member.camel);
        let element = ty(call.reply_type.unwrap_or(call.arg_type));
        quote! {
            again |= drive_stream::<H, #element, P::#source>(
                &mut this.handler,
                &mut this.streams.#method,
                cx,
                &mut budget,
            );
        }
    });
    let again_init = (!streams.is_empty()).then(|| quote! { let mut again = false; });
    let wake_condition = if streams.is_empty() {
        quote! { budget == 0 }
    } else {
        quote! {
            budget == 0 || again || ::core::mem::take(&mut this.streams.__started)
        }
    };

    let serve_doc = format!(
        "Serves interface `{iface_name}`'s commands and queries with `p`, over \
         the handler port `h`, and returns the future that does the serving. \
         `Handler::serve` is called with the interface's command and query \
         ordinals when this function runs; a refusal is a future that is ready \
         with `ProviderError::Serve`. Each poll of the future registers its \
         interest in the interface's claims, then takes and settles the claims \
         the handler has, at most {SERVE_BUDGET} in one poll, so that one poll \
         does not hold a single-threaded executor while callers keep sending. \
         A poll that took {SERVE_BUDGET} claims wakes the future's waker and \
         is `Pending`, so the executor polls it again after other tasks have \
         run; a poll that found no claim left before {SERVE_BUDGET} is \
         `Pending` without waking it. Under `ridl_rt::task::noop_waker` that \
         wake is discarded, so a frame loop that polls the future once per \
         frame settles at most {SERVE_BUDGET} claims per frame; a frame loop \
         that polls with `ridl_rt::task::flag_waker` polls again while its \
         flag was set, up to the loop's own limit of polls per frame. The \
         future resolves only when the handler port fails, to `ProviderError::Claim`; every \
         claim settled before the failure stays settled. `h` is held by value \
         and `p` by `&mut` until the future is dropped.\n\n{panics}",
        panics = catalog_panics_doc(
            iface,
            "h",
            "the comparison is made once, before `Handler::serve` is called",
        ),
    );
    let serve_doc = if streams.is_empty() {
        serve_doc
    } else {
        format!(
            "{serve_doc}\n\n# Streamed replies\n\nA query whose reply is a stream \
             is served from the source its `Provider` method returns, once the \
             arguments and the `require` clauses pass. Each poll of the future \
             first drives every stream it holds, then takes claims: a stream \
             whose last element found the window full pushes that element \
             again, after it registered its interest in the stream, and polls \
             its source only once the element was taken; otherwise the source \
             is polled once, which counts as one of the {SERVE_BUDGET} claims \
             of the poll. An element is encoded into a buffer inline in the \
             future and checked against its type before it is pushed; one \
             that fails stops the stream with `Contract::InvalidValue` and is \
             not sent. `Ready(None)` ends the stream, and the claim is settled \
             `Ok`; `Ready(Some(Err(Interrupted)))` stops it with \
             `Transport::Interrupted`; a push above the member's limit \
             (`PushError::TooLarge`) stops it with `Transport::Interrupted`; a \
             push the caller refused or stopped reading (`PushError::Failed`) \
             ends the claim. The source is dropped once its stream ended or \
             stopped. The future drives one stream per member at a time: a \
             call of a member whose stream is still running is settled \
             `Transport::Busy`. The handler port gives the future no signal \
             when a caller forgets the call, by its bound passing or by \
             dropping its stream: the future reads the forget from its next \
             push. So while a member's source answers `Pending`, a caller \
             that stopped keeps the member busy until the source yields its \
             next element, whose push fails, and only then is the source \
             dropped and the member free for a new call."
        )
    };
    let future_doc = format!(
        "The future `serve` returns over interface `{iface_name}`. It holds \
         the handler, the provider, and the claim buffer of \
         `<{iface_name} as Sizes<H::Encoding>>::MAX_BUFFER_SIZE` bytes, in the \
         encoding the handler port carries. It never resolves to `Ok`, and \
         polling it again after it resolved panics."
    );

    // `SERVE_BUDGET` is the most claims one poll of `Serve` takes
    // (driftsys/ridl#568). A future that holds one poll for an unbounded time
    // blocks every other task on a single-threaded executor; the budget bounds
    // one poll and keeps the cost of registering the claim interest, paid once
    // per poll, small beside the claims the poll settles. It, `ServeState` and
    // its variants are internals, so they carry no doc; `serve`'s own doc
    // states the bound for the user.
    //
    // `ServeState` is where `serve` is, as its future's `poll` moves it:
    // `Refused` when `Handler::serve` refused the members, which the first poll
    // reports; `Serving`, where each poll registers the claim interest and
    // settles at most `SERVE_BUDGET` claims; `Done` once the failure was
    // reported.
    quote! {
        const SERVE_BUDGET: usize = #budget;

        #stream_items

        #[doc = #serve_doc]
        #[track_caller]
        pub fn serve<H, P>(mut h: H, p: &mut P) -> Serve<'_, H, P>
        where
            #bounds,
            #encoding
        {
            check_catalog(::ridl_rt::port::Attached::catalog(&h));
            let state = match h.serve(#number, &[#(#ordinals),*]) {
                Ok(()) => ServeState::Serving,
                Err(error) => ServeState::Refused(error),
            };
            Serve {
                handler: h,
                provider: p,
                buf: <super::#iface as ::ridl_rt::contract::Sizes<
                    H::Encoding,
                >>::CLAIM_BUFFER,
                state,
                #streams_init
            }
        }

        #[doc = #future_doc]
        #[must_use = "claims are served only while the future is polled"]
        pub struct Serve<'a, #bounds>
        where
            #buffer_bound
        {
            handler: H,
            provider: &'a mut P,
            buf: <super::#iface as ::ridl_rt::contract::Sizes<
                H::Encoding,
            >>::ClaimBuffer,
            state: ServeState,
            #streams_field
        }

        #[derive(Clone, Copy)]
        enum ServeState {
            Refused(::ridl_rt::port::ServeError),
            Serving,
            Done,
        }

        /// `Unpin` whatever `H` is: nothing in the future is pinned, and a
        /// frame loop that stores it in its own state polls it through
        /// `Pin::new`.
        impl<#bounds> ::core::marker::Unpin for Serve<'_, H, P>
        where
            #encoding
        {}

        impl<#bounds> ::core::future::Future for Serve<'_, H, P>
        where
            #encoding
        {
            type Output = ::core::result::Result<
                ::core::convert::Infallible,
                ::ridl_rt::error::ProviderError,
            >;

            fn poll(
                self: ::core::pin::Pin<&mut Self>,
                cx: &mut ::core::task::Context<'_>,
            ) -> ::core::task::Poll<Self::Output> {
                let this = self.get_mut();
                match this.state {
                    ServeState::Done => panic!("`Serve` polled after completion"),
                    ServeState::Refused(error) => {
                        this.state = ServeState::Done;
                        ::core::task::Poll::Ready(Err(::ridl_rt::error::ProviderError::Serve(error)))
                    }
                    ServeState::Serving => {
                        this.handler.wake_on(
                            ::ridl_rt::port::Interest::Claim(#number),
                            cx.waker(),
                        );
                        let mut budget = SERVE_BUDGET;
                        #again_init
                        #(#drive)*
                        match dispatch(
                            &mut this.handler,
                            &mut *this.provider,
                            ::core::convert::AsMut::as_mut(&mut this.buf),
                            &mut budget,
                            #streams_arg
                        ) {
                            Ok(_) => {
                                // The poll stopped at the bound, so claims
                                // may still be waiting. A claim that was
                                // already waiting is not a change the
                                // registered interest wakes for (`Wakeable`),
                                // so the future wakes itself to be polled
                                // again.
                                if #wake_condition {
                                    cx.waker().wake_by_ref();
                                }
                                ::core::task::Poll::Pending
                            }
                            Err(error) => {
                                this.state = ServeState::Done;
                                ::core::task::Poll::Ready(Err(
                                    ::ridl_rt::error::ProviderError::Claim(error),
                                ))
                            }
                        }
                    }
                }
            }
        }
    }
}

/// The items of an interface with a stream member that `serve` and
/// `dispatch` share: the slots the sources are kept in, `Streams` and
/// `StreamSlot`, with `ElementBuffer`, the alias that names an element's
/// buffer in the handler's encoding and keeps the slot types short enough
/// for clippy's `type_complexity`, and the step that drives one slot, `drive_stream`, with its
/// helper `stop_stream`. Nothing for an interface without one.
///
/// They are internals and carry no doc; what they do:
///
/// `StreamSlot` is one running stream: the claim it answers, the source the
/// provider returned, the element buffer the source's elements are encoded
/// into, inline (`Payload::BUFFER`, so the face allocates nothing), and
/// `held`, the range of that buffer an element that found the window full
/// still occupies. The encoded bytes are a subslice of the buffer and not
/// always a prefix (a FlatBuffers builder fills it from its end), so the
/// range is kept and not a length.
///
/// `Streams` holds one optional slot per stream member, named by the
/// member's method, and `__started`, set by `dispatch` when it starts a
/// stream and taken by the poll of `Serve`, which then wakes itself so that
/// the new source is polled. The flag's name starts with an underscore, which
/// no ridl identifier can, so no member's slot is named like it.
///
/// `drive_stream` drives one slot through one poll of `Serve`, and answers
/// whether the future must wake itself: true when the source gave an
/// element the window took, because the source may have the next one ready
/// and stored no waker, and when the budget was spent before the source was
/// polled. An element the window had no room for is pushed again first,
/// after the interest in the stream was registered, and the source is not
/// polled until it is taken, so an element is never dropped and never asked
/// for twice. Otherwise the source is polled once, which spends one unit of
/// the budget: `Pending` leaves the source's own registration of the waker to
/// wake the future, `Ready(None)` ends the stream with `end_claim`,
/// `Ready(Some(Err(Interrupted)))` stops it with `abort_claim` and
/// `Transport::Interrupted`, and an element is encoded, checked as the
/// receiver will check it (ridl §10.2), and pushed. An element that fails its
/// check stops the stream with `abort_claim` and the outcome the receiver
/// would give it, and is not pushed (ridl §12.4).
///
/// `stop_stream` answers a push the port refused: `PushError::Failed` means
/// the caller refused the stream or stopped reading it, and `end_claim`
/// settles the claim with that outcome, so that the claim, and the caller's
/// call slot, are not held; `PushError::TooLarge` means the element is above
/// the member's limit, and the stream is stopped with
/// `Transport::Interrupted`, which the caller reads as an interruption; any
/// other refusal
/// leaves nothing to settle. Either way the caller drops the slot, and the
/// source with it.
fn stream_items(streams: &[Call]) -> TokenStream {
    if streams.is_empty() {
        return TokenStream::new();
    }
    let port = ident("H");
    let elements: Vec<&str> = streams
        .iter()
        .map(|call| call.reply_type.unwrap_or(call.arg_type))
        .collect();
    let element_bounds = elements.iter().map(|name| {
        let path = ty(name);
        quote! { #path: ::ridl_rt::payload::Payload<#port::Encoding>, }
    });
    let slots = streams.iter().zip(&elements).map(|(call, element)| {
        let method = &call.member.method;
        let source = ident(call.member.camel);
        let path = ty(element);
        quote! {
            #method: ::core::option::Option<StreamSlot<P::#source, ElementBuffer<H, #path>>>,
        }
    });
    quote! {
        type ElementBuffer<H, T> = <T as ::ridl_rt::payload::Payload<
            <H as ::ridl_rt::port::Encoded>::Encoding,
        >>::Buffer;

        struct StreamSlot<S, B> {
            claim: ::ridl_rt::port::ClaimId,
            source: S,
            buffer: B,
            held: ::core::option::Option<(usize, usize)>,
        }

        pub(crate) struct Streams<H, P>
        where
            H: ::ridl_rt::port::Encoded,
            P: Provider,
            #(#element_bounds)*
        {
            __started: bool,
            #(#slots)*
        }

        fn drive_stream<H, T, S>(
            h: &mut H,
            slot: &mut ::core::option::Option<StreamSlot<S, ElementBuffer<H, T>>>,
            cx: &mut ::core::task::Context<'_>,
            budget: &mut usize,
        ) -> bool
        where
            H: ::ridl_rt::port::Encoded
                + ::ridl_rt::port::StreamHandler
                + ::ridl_rt::port::Wakeable,
            T: ::ridl_rt::payload::Payload<H::Encoding>,
            S: ::ridl_rt::face::Source<Item = T>,
        {
            let Some(stream) = slot.as_mut() else {
                return false;
            };
            let claim = stream.claim;
            let interest = ::ridl_rt::port::Interest::ClaimStream(
                claim,
                ::ridl_rt::port::REPLY_STREAM,
            );
            if let Some((start, end)) = stream.held {
                h.wake_on(interest, cx.waker());
                let bytes = &::core::convert::AsRef::<[u8]>::as_ref(&stream.buffer)[start..end];
                match h.push_claim(claim, ::ridl_rt::port::REPLY_STREAM, bytes) {
                    Ok(taken) if taken == bytes.len() => stream.held = None,
                    Ok(_) => return false,
                    Err(error) => {
                        stop_stream(h, claim, error);
                        *slot = None;
                        return false;
                    }
                }
            }
            if *budget == 0 {
                return true;
            }
            *budget -= 1;
            let element = match stream.source.poll_next(cx) {
                ::core::task::Poll::Pending => return false,
                ::core::task::Poll::Ready(None) => {
                    let _ = h.end_claim(claim, ::ridl_rt::port::REPLY_STREAM);
                    *slot = None;
                    return false;
                }
                ::core::task::Poll::Ready(Some(Err(::ridl_rt::face::Interrupted))) => {
                    let _ = h.abort_claim(
                        claim,
                        ::ridl_rt::port::REPLY_STREAM,
                        ::ridl_rt::error::CallError::Transport(
                            ::ridl_rt::error::Transport::Interrupted,
                        ),
                    );
                    *slot = None;
                    return false;
                }
                ::core::task::Poll::Ready(Some(Ok(element))) => element,
            };
            let buf: &mut [u8] = ::core::convert::AsMut::as_mut(&mut stream.buffer);
            let base = buf.as_ptr() as usize;
            let checked = match ::ridl_rt::payload::Ref::<T, H::Encoding>::encode(&element, buf) {
                Ok(encoded) => {
                    let bytes = encoded.bytes();
                    match ::ridl_rt::payload::Ref::<T, H::Encoding>::verify(bytes) {
                        Ok(_) => {
                            let start = bytes.as_ptr() as usize - base;
                            Ok((start, start + bytes.len()))
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
                Err(::ridl_rt::payload::EncodeError::Contract(violation)) => {
                    Err(::ridl_rt::error::CallError::Contract(
                        ::ridl_rt::error::Contract::InvalidValue(violation),
                    ))
                }
                Err(::ridl_rt::payload::EncodeError::Capacity { needed, available }) => {
                    unreachable!(
                        "encoding a stream element needs {} bytes and its buffer has {}; a legal value cannot exceed the `MAX_SIZE` of its `Payload` in the port's encoding, so the value breaks a constraint the encoder does not check or its `Payload` implementation does not honor `MAX_SIZE`",
                        needed, available
                    )
                }
                Err(_) => unreachable!("encoding a stream element failed"),
            };
            let (start, end) = match checked {
                Ok(range) => range,
                Err(error) => {
                    let _ = h.abort_claim(claim, ::ridl_rt::port::REPLY_STREAM, error);
                    *slot = None;
                    return false;
                }
            };
            h.wake_on(interest, cx.waker());
            let bytes = &::core::convert::AsRef::<[u8]>::as_ref(&stream.buffer)[start..end];
            match h.push_claim(claim, ::ridl_rt::port::REPLY_STREAM, bytes) {
                Ok(taken) if taken == bytes.len() => true,
                Ok(_) => {
                    stream.held = Some((start, end));
                    false
                }
                Err(error) => {
                    stop_stream(h, claim, error);
                    *slot = None;
                    false
                }
            }
        }

        fn stop_stream<H: ::ridl_rt::port::StreamHandler>(
            h: &mut H,
            claim: ::ridl_rt::port::ClaimId,
            error: ::ridl_rt::port::PushError,
        ) {
            match error {
                ::ridl_rt::port::PushError::Failed(_) => {
                    let _ = h.end_claim(claim, ::ridl_rt::port::REPLY_STREAM);
                }
                ::ridl_rt::port::PushError::TooLarge { .. } => {
                    let _ = h.abort_claim(
                        claim,
                        ::ridl_rt::port::REPLY_STREAM,
                        ::ridl_rt::error::CallError::Transport(
                            ::ridl_rt::error::Transport::Interrupted,
                        ),
                    );
                }
                _ => {}
            }
        }
    }
}
