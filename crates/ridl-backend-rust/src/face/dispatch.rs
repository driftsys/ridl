//! Dispatch: the emitter of `dispatch`, the internal one-pass step that
//! routes a claim, checks the argument bytes, evaluates `require`, calls the
//! provider, evaluates a query's `ensure`, and settles. The face as a whole is
//! described in the documentation of the parent module, `face.rs`.

use super::{Call, decode_args, encode_into, interface_number};
use proc_macro2::{Ident, TokenStream};
use quote::quote;

pub(super) fn dispatch(
    iface: &Ident,
    iface_name: &str,
    commands: &[Call],
    queries: &[Call],
) -> TokenStream {
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
         returns how many were settled, or the handler port's failure. It is \
         the one-pass step `serve` drains through on each poll.\n\nIt does not \
         wait: it makes one pass over the claims the handler already has and \
         returns. `Ok` means the handler has no claim waiting; `Err` means \
         `Handler::next_claim` failed, and every claim settled before the \
         failure stays settled.\n\n`buf` must be at least \
         `{iface_name}::MAX_BUFFER_SIZE` bytes, because a reply is encoded \
         into the same buffer as the arguments. A shorter buffer returns \
         `Ok(0)` without consuming a claim.\n\nEvery claim that is taken is \
         settled, including one whose interface number or ordinal this \
         interface does not recognise, which settles \
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
        pub(crate) fn dispatch<H, P>(
            h: &mut H,
            p: &mut P,
            buf: &mut [u8],
        ) -> ::core::result::Result<usize, ::ridl_rt::port::ReadError>
        where
            H: ::ridl_rt::port::Handler,
            P: Provider,
        {
            if buf.len() < super::#iface::MAX_BUFFER_SIZE {
                return Ok(0);
            }
            let mut settled = 0usize;
            loop {
                let Some(claim) = h.next_claim(buf)? else {
                    return Ok(settled);
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
