//! Calls: delivery, settlement, the caller's sequence number, `forget`, and
//! which handler a claim belongs to (`Caller` and `Handler`).
//!
//! Every handler here calls `serve` for the members it takes claims on before
//! it takes one, because `Handler::serve` is what starts presentation. What a
//! runtime presents to a handler that has served nothing is left to it.

use ridl_rt::contract::InterfaceNo;
use ridl_rt::error::{CallError, Contract};
use ridl_rt::port::{Caller, ClaimId, Handler, ReadError, SettleError};

use crate::{Factory, IFACE, ORD, runtime};

/// A command reaches the handler with its arguments, and the settlement is
/// observable through `ack`.
pub fn a_command_is_delivered_and_acknowledged<F: Factory>() {
    let mut rt = runtime::<F>();
    rt.serve(IFACE, &[ORD]).expect("serve");
    let correlation = rt.command(IFACE, ORD, &[1, 2, 3]).expect("command sent");

    assert_eq!(rt.ack(correlation), None, "not yet settled");

    let mut buf = [0u8; 8];
    let claim = rt
        .next_claim(&mut buf)
        .expect("next_claim")
        .expect("a claim is waiting");
    assert_eq!(claim.iface, IFACE);
    assert_eq!(claim.ord, ORD);
    assert_eq!(&buf[..claim.len], &[1, 2, 3]);

    rt.settle(claim.id, Ok(&[])).expect("settle succeeds");
    assert_eq!(
        rt.ack(correlation),
        Some(Ok(())),
        "settlement is observable through ack"
    );
}

/// A query reaches the handler, and the reply bytes it settles are read back
/// through `reply`. `ack` answers `None` for a query's correlation.
pub fn a_query_is_delivered_and_replied<F: Factory>() {
    let mut rt = runtime::<F>();
    rt.serve(IFACE, &[ORD]).expect("serve");
    let correlation = rt.query(IFACE, ORD, &[9]).expect("query sent");

    let mut buf = [0u8; 8];
    let claim = rt
        .next_claim(&mut buf)
        .expect("next_claim")
        .expect("a claim is waiting");
    assert_eq!(&buf[..claim.len], &[9]);

    rt.settle(claim.id, Ok(&[7, 7])).expect("settle succeeds");

    let mut out = [0u8; 8];
    let reply = rt.reply(correlation, &mut out).expect("reply read");
    let Some(Ok(len)) = reply else {
        panic!("expected a successful reply, got {reply:?}");
    };
    assert_eq!(&out[..len], &[7, 7]);
    // A query's correlation always answers `None` from `ack` (`Caller::ack`'s
    // own documentation).
    assert_eq!(rt.ack(correlation), None);
}

/// A settlement that fails records no outcome, and the claim can still be
/// settled. The generated `dispatch` advances its returned count only past a
/// settlement the handler accepted; [`Factory::fail_next_settle`] is the
/// injected failure that makes that path reachable.
pub fn settle_can_be_made_to_fail_once_then_succeed<F: Factory>() {
    let mut rt = runtime::<F>();
    rt.serve(IFACE, &[ORD]).expect("serve");
    let correlation = rt.command(IFACE, ORD, &[1]).expect("command sent");
    let mut buf = [0u8; 8];
    let claim = rt
        .next_claim(&mut buf)
        .expect("next_claim")
        .expect("a claim is waiting");

    F::fail_next_settle(&mut rt);
    assert!(
        rt.settle(claim.id, Ok(&[])).is_err(),
        "the injected failure surfaces from settle"
    );
    assert_eq!(
        rt.ack(correlation),
        None,
        "a failed settle records no outcome"
    );

    rt.settle(claim.id, Ok(&[]))
        .expect("the next settle succeeds");
    assert_eq!(rt.ack(correlation), Some(Ok(())));
}

/// driftsys/ridl#308, and the rule ADR-0021 decision 5 fixes over it: a
/// caller's sequence number is unique per caller, not per channel, so two
/// callers on their first call both carry seq 1 — and they are still two
/// claims, never merged. A provider deduplicating on the number alone is what
/// #308 reports as wrong; what tells these two apart here is the claim.
pub fn two_callers_on_one_provider_are_two_claims_under_one_seq<F: Factory>() {
    let mut rt = runtime::<F>();
    let mut second = F::caller(&rt);
    rt.serve(IFACE, &[ORD]).expect("serve");

    let a = rt.command(IFACE, ORD, &[1]).expect("send");
    let b = second.command(IFACE, ORD, &[2]).expect("send");
    assert_ne!(a, b, "the correlations are distinct");

    let mut buf = [0u8; 8];
    let first_claim = rt
        .next_claim(&mut buf)
        .expect("next_claim")
        .expect("waiting");
    assert_eq!(&buf[..first_claim.len], &[1]);
    let second_claim = rt
        .next_claim(&mut buf)
        .expect("next_claim")
        .expect("waiting");
    assert_eq!(&buf[..second_claim.len], &[2]);
    assert_eq!(first_claim.envelope.seq, 1);
    assert_eq!(
        second_claim.envelope.seq, 1,
        "both callers are on their first call, so both carry seq 1"
    );
    assert_ne!(
        first_claim.id, second_claim.id,
        "and they are two claims, not one"
    );

    rt.settle(first_claim.id, Ok(&[])).expect("settle");
    rt.settle(second_claim.id, Ok(&[])).expect("settle");
    assert_eq!(rt.ack(a), Some(Ok(())));
    assert_eq!(second.ack(b), Some(Ok(())));
}

/// A caller's sequence number counts that caller's calls, commands and
/// queries on one counter.
pub fn a_caller_sequence_number_counts_that_caller_calls<F: Factory>() {
    let mut rt = runtime::<F>();
    rt.serve(IFACE, &[ORD]).expect("serve");
    rt.command(IFACE, ORD, &[1]).expect("send");
    rt.query(IFACE, ORD, &[2]).expect("send");

    let mut buf = [0u8; 8];
    let first = rt.next_claim(&mut buf).expect("read").expect("waiting");
    let second = rt.next_claim(&mut buf).expect("read").expect("waiting");
    assert_eq!(first.envelope.seq, 1);
    assert_eq!(
        second.envelope.seq, 2,
        "a command and a query share one counter"
    );
}

/// A contract error the provider settles is the outcome the caller sees.
pub fn a_settled_outcome_reports_the_contract_error_the_provider_settled<F: Factory>() {
    let mut rt = runtime::<F>();
    rt.serve(IFACE, &[ORD]).expect("serve");
    let correlation = rt.command(IFACE, ORD, &[1]).expect("send");
    let mut buf = [0u8; 8];
    let claim = rt.next_claim(&mut buf).expect("read").expect("waiting");
    rt.settle(
        claim.id,
        Err(CallError::Contract(Contract::PreconditionFailed)),
    )
    .expect("settle");

    assert_eq!(
        rt.ack(correlation),
        Some(Err(CallError::Contract(Contract::PreconditionFailed)))
    );
}

/// `next_claim` presents a call once, and a claim already settled is unknown
/// to a second settlement.
pub fn a_claim_is_presented_once_and_settled_once<F: Factory>() {
    let mut rt = runtime::<F>();
    rt.serve(IFACE, &[ORD]).expect("serve");
    rt.command(IFACE, ORD, &[1]).expect("send");
    let mut buf = [0u8; 8];
    let claim = rt.next_claim(&mut buf).expect("read").expect("waiting");
    assert!(
        rt.next_claim(&mut buf).expect("read").is_none(),
        "the claim is presented once"
    );

    rt.settle(claim.id, Ok(&[])).expect("settle");
    assert_eq!(
        rt.settle(claim.id, Ok(&[])),
        Err(SettleError::UnknownClaim),
        "a claim already settled is unknown to a second settlement"
    );
}

/// `ReadError::Short` does not consume the call.
pub fn a_short_buffer_leaves_the_claim_for_the_next_call<F: Factory>() {
    let mut rt = runtime::<F>();
    rt.serve(IFACE, &[ORD]).expect("serve");
    rt.command(IFACE, ORD, &[1, 2, 3]).expect("send");

    let mut short = [0u8; 1];
    assert_eq!(
        rt.next_claim(&mut short),
        Err(ReadError::Short { needed: 3 })
    );

    let mut buf = [0u8; 8];
    let claim = rt
        .next_claim(&mut buf)
        .expect("read")
        .expect("still waiting");
    assert_eq!(&buf[..claim.len], &[1, 2, 3]);
}

/// After `forget`, a settled outcome is no longer retrievable.
pub fn forget_releases_a_settled_correlation<F: Factory>() {
    let mut rt = runtime::<F>();
    rt.serve(IFACE, &[ORD]).expect("serve");
    let correlation = rt.command(IFACE, ORD, &[1]).expect("send");
    let mut buf = [0u8; 8];
    let claim = rt.next_claim(&mut buf).expect("read").expect("waiting");
    rt.settle(claim.id, Ok(&[])).expect("settle");
    assert_eq!(rt.ack(correlation), Some(Ok(())));

    rt.forget(correlation);
    assert_eq!(
        rt.ack(correlation),
        None,
        "the outcome is no longer retrievable"
    );
}

/// `Caller::forget` releases the caller's interest in an outcome. It is not a
/// cancellation: `Handler`'s contract is that every claim is settled, and a
/// call already sent is the provider's.
pub fn forget_before_the_claim_is_presented_leaves_the_call_for_the_provider<F: Factory>() {
    let mut rt = runtime::<F>();
    rt.serve(IFACE, &[ORD]).expect("serve");
    let correlation = rt.command(IFACE, ORD, &[1]).expect("send");
    rt.forget(correlation);

    let mut buf = [0u8; 8];
    let claim = rt
        .next_claim(&mut buf)
        .expect("next_claim")
        .expect("the call is still presented");
    assert_eq!(&buf[..claim.len], &[1]);
    rt.settle(claim.id, Ok(&[])).expect("and is still settled");
    assert_eq!(
        rt.ack(correlation),
        None,
        "but the caller asked not to be told"
    );
}

/// A `forget` between the claim and the settlement does not revoke the
/// provider's settlement, and the caller is not told of it.
pub fn forget_between_the_claim_and_the_settlement_leaves_the_settlement_valid<F: Factory>() {
    let mut rt = runtime::<F>();
    rt.serve(IFACE, &[ORD]).expect("serve");
    let correlation = rt.query(IFACE, ORD, &[1]).expect("send");
    let mut buf = [0u8; 8];
    let claim = rt.next_claim(&mut buf).expect("read").expect("waiting");
    rt.forget(correlation);

    rt.settle(claim.id, Ok(&[7]))
        .expect("the provider's settlement is not the caller's to revoke");
    let mut out = [0u8; 8];
    assert!(
        rt.reply(correlation, &mut out)
            .expect("reply read")
            .is_none()
    );
}

/// A correlation is not a claim. Before this call is presented there is no
/// claim to settle, and a settlement accepted here would acknowledge a call
/// the provider has not seen.
pub fn a_claim_that_was_never_presented_cannot_be_settled<F: Factory>() {
    let mut rt = runtime::<F>();
    rt.serve(IFACE, &[ORD]).expect("serve");
    let correlation = rt.command(IFACE, ORD, &[1]).expect("send");
    assert_eq!(
        rt.settle(ClaimId(correlation.0), Ok(&[])),
        Err(SettleError::UnknownClaim)
    );
    assert_eq!(rt.ack(correlation), None, "and nothing was acknowledged");

    let mut buf = [0u8; 8];
    let claim = rt.next_claim(&mut buf).expect("read").expect("waiting");
    rt.settle(claim.id, Ok(&[]))
        .expect("the real claim settles");
    assert_eq!(rt.ack(correlation), Some(Ok(())));
}

/// A settlement of a claim the handler does not hold is checked before the
/// injected failure is consumed, so the failure still has the next real
/// settlement to fail. The claim the handler does not hold is one it has
/// already settled, which no runtime can hold again.
pub fn an_injected_settle_failure_is_not_spent_on_an_unknown_claim<F: Factory>() {
    let mut rt = runtime::<F>();
    rt.serve(IFACE, &[ORD]).expect("serve");
    rt.command(IFACE, ORD, &[1]).expect("send");
    rt.command(IFACE, ORD, &[2]).expect("send");
    let mut buf = [0u8; 8];
    let settled = rt.next_claim(&mut buf).expect("read").expect("waiting");
    let claim = rt.next_claim(&mut buf).expect("read").expect("waiting");
    rt.settle(settled.id, Ok(&[])).expect("settle");

    F::fail_next_settle(&mut rt);
    assert_eq!(
        rt.settle(settled.id, Ok(&[])),
        Err(SettleError::UnknownClaim),
        "the claim is checked before the injected failure is consumed"
    );
    let failed = rt.settle(claim.id, Ok(&[]));
    assert!(
        matches!(failed, Err(error) if error != SettleError::UnknownClaim),
        "so the injected failure still has the next real settlement to fail, got {failed:?}"
    );
}

/// Two providers in one process settle their own calls and not each other's:
/// a claim belongs to the handler it was presented to.
pub fn a_handler_cannot_settle_another_handlers_claim<F: Factory>() {
    let mut rt = runtime::<F>();
    let mut second = F::handler(&rt);
    rt.serve(IFACE, &[ORD]).expect("serve");
    second.serve(InterfaceNo(2), &[ORD]).expect("serve");

    let correlation = rt.command(IFACE, ORD, &[1]).expect("send");
    let mut buf = [0u8; 8];
    let claim = rt
        .next_claim(&mut buf)
        .expect("next_claim")
        .expect("waiting");

    assert_eq!(
        second.settle(claim.id, Ok(&[])),
        Err(SettleError::UnknownClaim),
        "the claim is not the second handler's to settle"
    );
    assert_eq!(
        rt.ack(correlation),
        None,
        "and nothing was acknowledged in the caller's name"
    );

    rt.settle(claim.id, Ok(&[]))
        .expect("its own handler settles it");
    assert_eq!(rt.ack(correlation), Some(Ok(())));
}

/// Two components providing different interfaces in one process: each
/// handler is presented only the calls to what it served. A handler presented
/// another handler's call would settle it `UnknownInteraction` through the
/// generated dispatch, and the call would be lost.
pub fn two_handlers_each_receive_only_what_they_served<F: Factory>() {
    let mut rt = runtime::<F>();
    let mut second = F::handler(&rt);
    rt.serve(IFACE, &[ORD]).expect("serve");
    second.serve(InterfaceNo(2), &[ORD]).expect("serve");

    rt.command(InterfaceNo(2), ORD, &[7]).expect("send");
    rt.command(IFACE, ORD, &[8]).expect("send");

    let mut buf = [0u8; 8];
    let claim = rt
        .next_claim(&mut buf)
        .expect("next_claim")
        .expect("waiting");
    assert_eq!(claim.iface, IFACE, "the call the first handler served");
    assert_eq!(&buf[..claim.len], &[8]);

    let claim = second
        .next_claim(&mut buf)
        .expect("next_claim")
        .expect("waiting");
    assert_eq!(claim.iface, InterfaceNo(2), "and the second handler's own");
    assert_eq!(&buf[..claim.len], &[7]);

    assert!(
        rt.next_claim(&mut buf).expect("next_claim").is_none(),
        "neither handler consumed the other's call"
    );
    assert!(second.next_claim(&mut buf).expect("next_claim").is_none());
}
