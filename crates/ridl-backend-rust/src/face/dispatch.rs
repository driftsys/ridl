//! Dispatch: the emitter of `dispatch`, the internal one-pass step that
//! routes a claim, checks the argument bytes, evaluates `require`, calls the
//! provider, evaluates a query's `ensure`, and settles. The face as a whole is
//! described in the documentation of the parent module, `face.rs`.

use super::{Call, decode_args, encode_into, interface_number};
use proc_macro2::{Ident, TokenStream};
use quote::quote;

/// Emits `dispatch`. Each arm binds the claim's decoded argument as `__arg`
/// and never under the ridl parameter's name, so no ridl parameter name is
/// bound in the body. Before issue #570 a parameter named like one of the body's own
/// parameters (`h`, `p`, `buf`) or locals (`claim`, `accepted`, `reply`)
/// shadowed them or was shadowed by them. A ridl identifier cannot start with an
/// underscore (`ridl check` refuses one), so `__arg` cannot be a parameter's
/// name, as in the call method and the call futures.
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
        quote! {
            #ordinal => {
                let decoded = #decode;
                match decoded {
                    Err(error) => h.settle(claim.id, Err(error)),
                    Ok(__arg) => {
                        match <#descriptor as ::ridl_rt::contract::Command>::require(&__arg) {
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
                                p.#method(&__arg);
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
        quote! {
            #ordinal => {
                let decoded = #decode;
                match decoded {
                    Err(error) => h.settle(claim.id, Err(error)),
                    Ok(__arg) => {
                        match <#descriptor as ::ridl_rt::contract::Query>::require(&__arg) {
                            Err(()) => h.settle(
                                claim.id,
                                Err(::ridl_rt::error::CallError::Contract(
                                    ::ridl_rt::error::Contract::PreconditionFailed,
                                )),
                            ),
                            Ok(()) => {
                                let reply = p.#method(&__arg);
                                match <#descriptor as ::ridl_rt::contract::Query>::ensure(
                                    &__arg,
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
        "Settles the claims of interface `{iface_name}` that are waiting, up \
         to `budget` of them, and returns how many were settled, or the \
         handler port's failure. It is the one-pass step `serve` calls on \
         each poll.\n\nIt does not wait: it makes one pass over the claims \
         the handler already has and returns. `budget` is decreased by one \
         for each claim taken from `Handler::next_claim`, whether or not its \
         settlement is accepted, and the pass stops when it reaches 0 without \
         asking for another claim. With a buffer of at least \
         `{iface_name}::MAX_BUFFER_SIZE` bytes, `Ok` with `budget` above 0 \
         means the handler has no claim waiting; `Ok` with `budget` at 0 means claims \
         may still be waiting; `Err` means `Handler::next_claim` failed, and \
         every claim settled before the failure stays settled. \
         `ReadError::ShortClaim` is not a failure: the claim's arguments \
         exceed `{iface_name}::MAX_BUFFER_SIZE`, the member's largest valid \
         encoding, so the claim is settled `Transport::Corrupt` by its id \
         without being read, counts toward `budget`, and the pass continues \
         (driftsys/ridl#569).\n\n`buf` must \
         be at least `{iface_name}::MAX_BUFFER_SIZE` bytes, because a reply is \
         encoded into the same buffer as the arguments. A shorter buffer \
         returns `Ok(0)` without consuming a claim or changing \
         `budget`.\n\nEvery claim that is taken is \
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
            budget: &mut usize,
        ) -> ::core::result::Result<usize, ::ridl_rt::port::ReadError>
        where
            H: ::ridl_rt::port::Handler,
            P: Provider,
        {
            if buf.len() < super::#iface::MAX_BUFFER_SIZE {
                return Ok(0);
            }
            let mut settled = 0usize;
            while *budget > 0 {
                let claim = match h.next_claim(buf) {
                    Ok(Some(claim)) => claim,
                    Ok(None) => break,
                    // The claim's arguments exceed `MAX_BUFFER_SIZE`, which
                    // is the member's largest valid encoding, so they are
                    // not a well-formed encoding: the claim is settled
                    // `Transport::Corrupt` without being read, as argument
                    // bytes that fail the structure check are, and counts
                    // toward `budget` like any claim taken
                    // (driftsys/ridl#569).
                    Err(::ridl_rt::port::ReadError::ShortClaim { claim, .. }) => {
                        *budget -= 1;
                        let settlement = h.settle(
                            claim,
                            Err(::ridl_rt::error::CallError::Transport(
                                ::ridl_rt::error::Transport::Corrupt,
                            )),
                        );
                        if settlement.is_ok() {
                            settled += 1;
                        }
                        continue;
                    }
                    Err(error) => return Err(error),
                };
                *budget -= 1;
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
            Ok(settled)
        }
    }
}
