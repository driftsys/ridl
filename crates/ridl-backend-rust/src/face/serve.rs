//! `serve`: the emitter of the `serve` function and of its future, `Serve`.
//! The face as a whole is described in the documentation of the parent
//! module, `face.rs`.

use super::{Call, interface_number};
use proc_macro2::{Ident, TokenStream};
use quote::quote;

/// `serve` and its future (the async face design, note F-7): the served set
/// is registered when the function is called; each poll registers
/// `Interest::Claim`, then settles claims through `dispatch`, at most
/// `SERVE_BUDGET` of them (driftsys/ridl#568), and wakes its own waker when it
/// stopped at that bound; the handler port's failure is the value the future
/// resolves to, and the future never resolves to `Ok`.
pub(super) fn serve(
    iface: &Ident,
    iface_name: &str,
    commands: &[Call],
    queries: &[Call],
) -> TokenStream {
    let number = interface_number(iface);
    let ordinals = commands
        .iter()
        .chain(queries.iter())
        .map(|call| &call.member.ordinal);
    let bounds = quote! { H: ::ridl_rt::port::Handler + ::ridl_rt::port::Wakeable, P: Provider };

    let serve_doc = format!(
        "Serves interface `{iface_name}`'s commands and queries with `p`, over \
         the handler port `h`, and returns the future that does the serving. \
         `Handler::serve` is called with the interface's command and query \
         ordinals when this function runs; a refusal is a future that is ready \
         with `ProviderError::Serve`. Each poll of the future registers its \
         interest in the interface's claims, then takes and settles the claims \
         the handler has, at most 32 in one poll, so that one poll does not \
         hold a single-threaded executor while callers keep sending. A poll \
         that took 32 claims wakes the future's waker and is `Pending`, so \
         the executor polls it again after other tasks have run; a poll that \
         found no claim left before 32 is `Pending` without waking it. The \
         future resolves \
         only when the handler port fails, to `ProviderError::Claim`; every \
         claim settled before the failure stays settled. `h` is held by value \
         and `p` by `&mut` until the future is dropped."
    );
    let future_doc = format!(
        "The future `serve` returns over interface `{iface_name}`. It holds \
         the handler, the provider, and the claim buffer of \
         `{iface_name}::MAX_BUFFER_SIZE` bytes. It never resolves to `Ok`, and \
         polling it again after it resolved panics."
    );

    quote! {
        /// The most claims one poll of `Serve` takes (driftsys/ridl#568). A
        /// future that holds one poll for an unbounded time blocks every other
        /// task on a single-threaded executor; 32 bounds one poll and keeps the
        /// cost of registering the claim interest, paid once per poll, small
        /// beside the claims the poll settles.
        const SERVE_BUDGET: usize = 32;

        #[doc = #serve_doc]
        pub fn serve<H, P>(mut h: H, p: &mut P) -> Serve<'_, H, P>
        where
            #bounds,
        {
            let state = match h.serve(#number, &[#(#ordinals),*]) {
                Ok(()) => ServeState::Serving,
                Err(error) => ServeState::Refused(error),
            };
            Serve {
                handler: h,
                provider: p,
                buf: [0u8; super::#iface::MAX_BUFFER_SIZE],
                state,
            }
        }

        #[doc = #future_doc]
        #[must_use = "claims are served only while the future is polled"]
        pub struct Serve<'a, #bounds> {
            handler: H,
            provider: &'a mut P,
            buf: [u8; super::#iface::MAX_BUFFER_SIZE],
            state: ServeState,
        }

        /// Where `serve` is, as its future's `poll` moves it.
        #[derive(Clone, Copy)]
        enum ServeState {
            /// `Handler::serve` refused the members; the first poll reports
            /// it.
            Refused(::ridl_rt::port::ServeError),
            /// Each poll registers the claim interest and settles at most
            /// `SERVE_BUDGET` claims.
            Serving,
            /// The failure was reported.
            Done,
        }

        /// `Unpin` whatever `H` is: nothing in the future is pinned, and a
        /// frame loop that stores it in its own state polls it through
        /// `Pin::new`.
        impl<#bounds> ::core::marker::Unpin for Serve<'_, H, P> {}

        impl<#bounds> ::core::future::Future for Serve<'_, H, P> {
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
                        match dispatch(
                            &mut this.handler,
                            &mut *this.provider,
                            &mut this.buf,
                            &mut budget,
                        ) {
                            Ok(_) => {
                                // The poll stopped at the bound, so claims
                                // may still be waiting. A claim that was
                                // already waiting is not a change the
                                // registered interest wakes for (`Wakeable`),
                                // so the future wakes itself to be polled
                                // again.
                                if budget == 0 {
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
