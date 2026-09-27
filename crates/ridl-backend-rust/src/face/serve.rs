//! `serve`: the emitter of the `serve` function and of its future, `Serve`.
//! The face as a whole is described in the documentation of the parent
//! module, `face.rs`.

use super::{Call, interface_number};
use proc_macro2::{Ident, TokenStream};
use quote::quote;

/// `serve` and its future (the async face design, note F-7): the served set
/// is registered when the function is called; each poll registers
/// `Interest::Claim`, then drains the handler through `dispatch`; the
/// handler port's failure is the value the future resolves to, and the
/// future never resolves to `Ok`.
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
         interest in the interface's claims, then settles every claim the \
         handler has, and is `Pending` once none is left. The future resolves \
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
            /// Each poll registers the claim interest and drains the handler.
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
                        match dispatch(&mut this.handler, &mut *this.provider, &mut this.buf) {
                            Ok(_) => ::core::task::Poll::Pending,
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
