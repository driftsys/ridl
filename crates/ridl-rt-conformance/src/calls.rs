//! Calls: delivery, settlement, the caller's sequence number, `forget`, and
//! which handler a claim belongs to (`Caller` and `Handler`).
//!
//! Every handler here calls `serve` for the members it takes claims on before
//! it takes one, because `Handler::serve` is what starts presentation. The
//! suite has no test of a handler that has served nothing; the crate
//! documentation, "What the suite leaves out", says why.

use ridl_rt::contract::InterfaceNo;
use ridl_rt::error::{CallError, Contract, Transport};
use ridl_rt::port::{Caller, ClaimId, Handler, ReadError, SendError, SettleError};

use crate::{Factory, IFACE, ORD, TRACE_A, TRACE_B, runtime};

/// A command reaches the handler with its arguments, and the settlement is
/// observable through `ack`.
pub fn a_command_is_delivered_and_acknowledged<F: Factory>() {
    let mut rt = runtime::<F>();
    rt.serve(IFACE, &[ORD]).expect("serve");
    let correlation = rt
        .command(IFACE, ORD, &[1, 2, 3], None)
        .expect("command sent");

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
    let correlation = rt.query(IFACE, ORD, &[9], None).expect("query sent");

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
    let correlation = rt.command(IFACE, ORD, &[1], None).expect("command sent");
    let mut buf = [0u8; 8];
    let claim = rt
        .next_claim(&mut buf)
        .expect("next_claim")
        .expect("a claim is waiting");

    F::fail_next_settle(&mut rt);
    let failed = rt.settle(claim.id, Ok(&[]));
    assert!(
        matches!(failed, Err(error) if error != SettleError::UnknownClaim),
        "the injected failure surfaces from settle as an error other than \
         `UnknownClaim`, got {failed:?}"
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

    let a = rt.command(IFACE, ORD, &[1], None).expect("send");
    let b = second.command(IFACE, ORD, &[2], None).expect("send");
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
    rt.command(IFACE, ORD, &[1], None).expect("send");
    rt.query(IFACE, ORD, &[2], None).expect("send");

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
    let correlation = rt.command(IFACE, ORD, &[1], None).expect("send");
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
    rt.command(IFACE, ORD, &[1], None).expect("send");
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

/// A buffer shorter than the next call's arguments gives
/// `ReadError::ShortClaim` with that call's id and the bytes it needs, and
/// does not consume the call: a later `next_claim` with a buffer of at least
/// `needed` bytes presents the same call under the same id, and its
/// settlement reaches the caller (driftsys/ridl#569).
pub fn an_oversized_claim_is_reported_with_its_id_and_is_not_consumed<F: Factory>() {
    let mut rt = runtime::<F>();
    rt.serve(IFACE, &[ORD]).expect("serve");
    let correlation = rt.command(IFACE, ORD, &[1, 2, 3], None).expect("send");

    let mut short = [0u8; 1];
    let Err(ReadError::ShortClaim {
        claim: unread,
        needed,
    }) = rt.next_claim(&mut short)
    else {
        panic!("a buffer shorter than the arguments reports ShortClaim");
    };
    assert_eq!(needed, 3, "the bytes the arguments need");
    assert_eq!(
        rt.next_claim(&mut short),
        Err(ReadError::ShortClaim {
            claim: unread,
            needed: 3
        }),
        "the call is not consumed, and is presented again under the same id"
    );

    let mut buf = [0u8; 8];
    let claim = rt
        .next_claim(&mut buf)
        .expect("read")
        .expect("still waiting");
    assert_eq!(claim.id, unread, "the read presents the same claim");
    assert_eq!(&buf[..claim.len], &[1, 2, 3]);

    rt.settle(claim.id, Ok(&[])).expect("settle");
    assert_eq!(rt.ack(correlation), Some(Ok(())));
}

/// A claim presented through `ShortClaim` is settled by its id with its
/// arguments never read; the caller sees the outcome, the call leaves the
/// waiting calls, and a second settlement is unknown (driftsys/ridl#569).
pub fn an_unread_claim_is_settled_by_its_id<F: Factory>() {
    let mut rt = runtime::<F>();
    rt.serve(IFACE, &[ORD]).expect("serve");
    let correlation = rt.command(IFACE, ORD, &[1, 2, 3], None).expect("send");

    let mut short = [0u8; 1];
    let Err(ReadError::ShortClaim { claim, .. }) = rt.next_claim(&mut short) else {
        panic!("a buffer shorter than the arguments reports ShortClaim");
    };
    rt.settle(claim, Err(CallError::Transport(Transport::Corrupt)))
        .expect("an unread claim is settled by its id");
    assert_eq!(
        rt.ack(correlation),
        Some(Err(CallError::Transport(Transport::Corrupt))),
        "the caller sees the outcome"
    );

    let mut buf = [0u8; 8];
    assert!(
        rt.next_claim(&mut buf).expect("read").is_none(),
        "the settled call is no longer waiting"
    );
    assert_eq!(
        rt.settle(claim, Ok(&[])),
        Err(SettleError::UnknownClaim),
        "a claim already settled is unknown to a second settlement"
    );
}

/// An oversized call blocks the calls sent after it until it is settled:
/// each `next_claim` with the short buffer reports the same claim, and the
/// settlement of that claim by its id lets the next call be presented, under
/// an id of its own (driftsys/ridl#569).
pub fn the_calls_behind_an_oversized_claim_are_presented_once_it_is_settled<F: Factory>() {
    let mut rt = runtime::<F>();
    rt.serve(IFACE, &[ORD]).expect("serve");
    let oversized = rt.command(IFACE, ORD, &[1, 2, 3], None).expect("send");
    let behind = rt.command(IFACE, ORD, &[4], None).expect("send");

    let mut buf = [0u8; 2];
    let Err(ReadError::ShortClaim { claim: first, .. }) = rt.next_claim(&mut buf) else {
        panic!("a buffer shorter than the arguments reports ShortClaim");
    };
    assert_eq!(
        rt.next_claim(&mut buf),
        Err(ReadError::ShortClaim {
            claim: first,
            needed: 3
        }),
        "the oversized call stays the next one until it is settled"
    );

    rt.settle(first, Err(CallError::Transport(Transport::Corrupt)))
        .expect("settle the unread claim");
    let second = rt
        .next_claim(&mut buf)
        .expect("read")
        .expect("the call behind it is presented");
    assert_ne!(second.id, first, "a claim id names one call");
    assert_eq!(&buf[..second.len], &[4]);

    rt.settle(second.id, Ok(&[])).expect("settle");
    assert_eq!(
        rt.ack(oversized),
        Some(Err(CallError::Transport(Transport::Corrupt)))
    );
    assert_eq!(rt.ack(behind), Some(Ok(())));
}

/// After `forget`, a settled outcome is no longer retrievable.
pub fn forget_releases_a_settled_correlation<F: Factory>() {
    let mut rt = runtime::<F>();
    rt.serve(IFACE, &[ORD]).expect("serve");
    let correlation = rt.command(IFACE, ORD, &[1], None).expect("send");
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

/// `Caller::forget` releases the caller's interest in an outcome. What
/// happens to a call no provider has claimed yet is the runtime's: one may
/// withdraw it, and one whose transport has already sent the request cannot
/// recall it, so the call is still presented and settled. The test accepts
/// either result. Either way the caller is not told the outcome, and once
/// the call is withdrawn or settled it holds no slot: the runtime accepts
/// [`Factory::SLOTS`] further sends before `SendError::Busy`.
pub fn forget_before_the_claim_is_presented_withdraws_or_leaves_the_call<F: Factory>() {
    let mut rt = runtime::<F>();
    rt.serve(IFACE, &[ORD]).expect("serve");
    let correlation = rt.command(IFACE, ORD, &[1], None).expect("send");
    rt.forget(correlation);

    let mut buf = [0u8; 8];
    let result = if let Some(claim) = rt.next_claim(&mut buf).expect("next_claim") {
        assert_eq!(&buf[..claim.len], &[1]);
        rt.settle(claim.id, Ok(&[]))
            .expect("a call that is presented is still settled");
        "held until settled"
    } else {
        "withdrawn"
    };
    assert_eq!(rt.ack(correlation), None, "the caller asked not to be told");
    assert_eq!(
        sends_until_busy::<F>(&mut rt),
        F::SLOTS,
        "the forgotten call, {result}, gave its slot back"
    );
}

/// The number of commands `caller` accepts before it answers
/// `SendError::Busy`, counting at most one more than [`Factory::SLOTS`].
fn sends_until_busy<F: Factory>(caller: &mut impl Caller) -> usize {
    for sent in 0..=F::SLOTS {
        match caller.command(IFACE, ORD, &[2], None) {
            Ok(_) => {}
            Err(SendError::Busy) => return sent,
            Err(error) => panic!("a send failed other than busy: {error:?}"),
        }
    }
    F::SLOTS + 1
}

/// The call table is the runtime's, shared by every caller on it: with
/// [`Factory::SLOTS`] calls held, whichever callers sent them, a send by any
/// caller is refused with `SendError::Busy`. Reading an outcome does not
/// free a slot; forgetting a settled call does, and whichever caller sends
/// next takes it (ADR-0021 decision 15).
pub fn a_send_with_every_slot_taken_is_busy_for_every_caller<F: Factory>() {
    assert!(F::SLOTS > 0, "a runtime holds at least one call");
    let mut rt = runtime::<F>();
    let mut second = F::caller(&rt);
    rt.serve(IFACE, &[ORD]).expect("serve");
    let mut buf = [0u8; 8];

    // The two callers take turns, so each holds part of the table.
    let mut mine = Vec::new();
    let mut theirs = Vec::new();
    for n in 0..F::SLOTS {
        if n % 2 == 0 {
            mine.push(rt.command(IFACE, ORD, &[1], None).expect("a slot is free"));
        } else {
            theirs.push(
                second
                    .command(IFACE, ORD, &[1], None)
                    .expect("a slot is free"),
            );
        }
        let claim = rt
            .next_claim(&mut buf)
            .expect("next_claim")
            .expect("the call just sent");
        rt.settle(claim.id, Ok(&[])).expect("settle");
    }

    assert_eq!(rt.command(IFACE, ORD, &[2], None), Err(SendError::Busy));
    assert_eq!(rt.query(IFACE, ORD, &[2], None), Err(SendError::Busy));
    assert_eq!(
        second.command(IFACE, ORD, &[2], None),
        Err(SendError::Busy),
        "the table is the runtime's, shared by every caller"
    );

    for c in &mine {
        assert_eq!(rt.ack(*c), Some(Ok(())));
    }
    for c in &theirs {
        assert_eq!(second.ack(*c), Some(Ok(())));
    }
    assert_eq!(
        second.command(IFACE, ORD, &[2], None),
        Err(SendError::Busy),
        "reading an outcome frees no slot"
    );

    rt.forget(mine[0]);
    second
        .command(IFACE, ORD, &[3], None)
        .expect("the slot the first caller's forget freed is the second caller's to take");
    assert_eq!(
        rt.command(IFACE, ORD, &[4], None),
        Err(SendError::Busy),
        "and the table is full again"
    );
}

/// A slot reclaimed and taken by a new call does not answer to the
/// correlation it had before: the old correlation reads no outcome, not the
/// new call's, and forgetting it again leaves the new call alone. With every
/// other slot held, the new call can only take the slot that was reclaimed.
pub fn a_reclaimed_slots_old_correlation_answers_none<F: Factory>() {
    let mut rt = runtime::<F>();
    rt.serve(IFACE, &[ORD]).expect("serve");
    let old = crate::fill::<F>(&mut rt)[0];
    rt.forget(old);

    let new = rt
        .query(IFACE, ORD, &[2], None)
        .expect("the reclaimed slot");
    assert_ne!(new, old, "the slot is taken under a new correlation");
    let mut buf = [0u8; 8];
    let claim = rt
        .next_claim(&mut buf)
        .expect("next_claim")
        .expect("the new call");
    rt.settle(claim.id, Ok(&[8, 8])).expect("settle");

    assert_eq!(rt.ack(old), None, "the old correlation has no outcome");
    assert_eq!(
        rt.reply(old, &mut buf),
        Ok(None),
        "and does not read the new call's reply"
    );
    rt.forget(old);
    assert_eq!(
        rt.reply(new, &mut buf),
        Ok(Some(Ok(2))),
        "forgetting the old correlation again leaves the new call alone"
    );
    assert_eq!(&buf[..2], &[8, 8]);
}

/// A `forget` between the claim and the settlement does not revoke the
/// provider's settlement, and the caller is not told of it.
pub fn forget_between_the_claim_and_the_settlement_leaves_the_settlement_valid<F: Factory>() {
    let mut rt = runtime::<F>();
    rt.serve(IFACE, &[ORD]).expect("serve");
    let correlation = rt.query(IFACE, ORD, &[1], None).expect("send");
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

/// A `forget` between the offer of a claim through `ShortClaim` and its
/// settlement does not revoke the settlement either: the offered claim is the
/// provider's, and settling it by the id `ShortClaim` carried frees the slot
/// (driftsys/ridl#569).
pub fn forget_between_the_offer_and_the_settlement_leaves_the_settlement_valid<F: Factory>() {
    let mut rt = runtime::<F>();
    rt.serve(IFACE, &[ORD]).expect("serve");
    let correlation = rt.command(IFACE, ORD, &[1, 2, 3], None).expect("send");
    let mut short = [0u8; 1];
    let Err(ReadError::ShortClaim { claim, .. }) = rt.next_claim(&mut short) else {
        panic!("a buffer shorter than the arguments reports ShortClaim");
    };
    rt.forget(correlation);

    rt.settle(claim, Ok(&[]))
        .expect("the provider's settlement is not the caller's to revoke");
    assert_eq!(
        rt.ack(correlation),
        None,
        "nothing is readable for a forgotten call"
    );
    assert_eq!(
        sends_until_busy::<F>(&mut rt),
        F::SLOTS,
        "the settlement freed the slot"
    );
}

/// A correlation is not a claim. Before this call is presented there is no
/// claim to settle, and a settlement accepted here would acknowledge a call
/// the provider has not seen.
pub fn a_claim_that_was_never_presented_cannot_be_settled<F: Factory>() {
    let mut rt = runtime::<F>();
    rt.serve(IFACE, &[ORD]).expect("serve");
    let correlation = rt.command(IFACE, ORD, &[1], None).expect("send");
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
    rt.command(IFACE, ORD, &[1], None).expect("send");
    rt.command(IFACE, ORD, &[2], None).expect("send");
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

    let correlation = rt.command(IFACE, ORD, &[1], None).expect("send");
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

    rt.command(InterfaceNo(2), ORD, &[7], None).expect("send");
    rt.command(IFACE, ORD, &[8], None).expect("send");

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

/// The trace context a command is sent with arrives on its claim.
pub fn a_commands_context_arrives_on_its_claim<F: Factory>() {
    let mut rt = runtime::<F>();
    rt.serve(IFACE, &[ORD]).expect("serve");
    rt.command(IFACE, ORD, &[1], Some(TRACE_A)).expect("send");

    let mut buf = [0u8; 8];
    let claim = rt
        .next_claim(&mut buf)
        .expect("next_claim")
        .expect("a claim is waiting");
    assert_eq!(claim.trace, Some(TRACE_A));
}

/// The trace context a query is sent with arrives on its claim.
pub fn a_querys_context_arrives_on_its_claim<F: Factory>() {
    let mut rt = runtime::<F>();
    rt.serve(IFACE, &[ORD]).expect("serve");
    rt.query(IFACE, ORD, &[1], Some(TRACE_A)).expect("send");

    let mut buf = [0u8; 8];
    let claim = rt
        .next_claim(&mut buf)
        .expect("next_claim")
        .expect("a claim is waiting");
    assert_eq!(claim.trace, Some(TRACE_A));
}

/// A command and a query sent without a trace context arrive without one.
pub fn a_call_sent_without_a_context_arrives_without_one<F: Factory>() {
    let mut rt = runtime::<F>();
    rt.serve(IFACE, &[ORD]).expect("serve");
    let mut buf = [0u8; 8];

    rt.command(IFACE, ORD, &[1], None).expect("send");
    let claim = rt
        .next_claim(&mut buf)
        .expect("next_claim")
        .expect("the command");
    assert_eq!(claim.trace, None, "the command");
    rt.settle(claim.id, Ok(&[])).expect("settle");

    rt.query(IFACE, ORD, &[2], None).expect("send");
    let claim = rt
        .next_claim(&mut buf)
        .expect("next_claim")
        .expect("the query");
    assert_eq!(claim.trace, None, "the query");
}

/// Two calls in flight from two callers each keep the trace context they were
/// sent with, and the two are not exchanged.
pub fn two_calls_in_flight_each_keep_their_own_context<F: Factory>() {
    let mut rt = runtime::<F>();
    let mut second = F::caller(&rt);
    rt.serve(IFACE, &[ORD]).expect("serve");

    rt.command(IFACE, ORD, &[1], Some(TRACE_A)).expect("send");
    second
        .command(IFACE, ORD, &[2], Some(TRACE_B))
        .expect("send");

    let mut buf = [0u8; 8];
    let first_claim = rt
        .next_claim(&mut buf)
        .expect("next_claim")
        .expect("waiting");
    assert_eq!(&buf[..first_claim.len], &[1]);
    assert_eq!(first_claim.trace, Some(TRACE_A));
    let second_claim = rt
        .next_claim(&mut buf)
        .expect("next_claim")
        .expect("waiting");
    assert_eq!(&buf[..second_claim.len], &[2]);
    assert_eq!(second_claim.trace, Some(TRACE_B));
}

/// Two calls in flight from one caller each keep the trace context they were
/// sent with: the second call does not take the context of the first, which
/// is still in flight when the second is sent. Each claim is identified by
/// its argument bytes, so the case does not depend on the presentation
/// order.
pub fn one_callers_calls_in_flight_each_keep_their_own_context<F: Factory>() {
    let mut rt = runtime::<F>();
    rt.serve(IFACE, &[ORD]).expect("serve");

    rt.command(IFACE, ORD, &[1], Some(TRACE_A)).expect("send");
    rt.command(IFACE, ORD, &[2], Some(TRACE_B)).expect("send");

    let mut buf = [0u8; 8];
    let mut seen = Vec::new();
    for _ in 0..2 {
        let claim = rt
            .next_claim(&mut buf)
            .expect("next_claim")
            .expect("waiting");
        match buf[..claim.len] {
            [1] => assert_eq!(claim.trace, Some(TRACE_A), "the first call"),
            [2] => assert_eq!(claim.trace, Some(TRACE_B), "the second call"),
            ref other => panic!("a claim no call was sent with: {other:?}"),
        }
        seen.push(buf[0]);
        rt.settle(claim.id, Ok(&[])).expect("settle");
    }
    seen.sort_unstable();
    assert_eq!(seen, [1, 2], "each call is presented once");
}

/// A claim reported through `ReadError::ShortClaim` keeps its trace context:
/// the claim presented once the buffer is large enough carries it.
pub fn an_oversized_claims_context_survives_its_second_presentation<F: Factory>() {
    let mut rt = runtime::<F>();
    rt.serve(IFACE, &[ORD]).expect("serve");
    rt.command(IFACE, ORD, &[1, 2, 3], Some(TRACE_A))
        .expect("send");

    let mut short = [0u8; 1];
    assert!(matches!(
        rt.next_claim(&mut short),
        Err(ReadError::ShortClaim { needed: 3, .. })
    ));

    let mut buf = [0u8; 8];
    let claim = rt
        .next_claim(&mut buf)
        .expect("read")
        .expect("still waiting");
    assert_eq!(claim.trace, Some(TRACE_A));
}

/// A call that takes a slot another call held does not keep that call's trace
/// context. With every other slot held, the new call can only take the slot
/// that was reclaimed.
pub fn a_reused_call_slot_does_not_keep_the_previous_context<F: Factory>() {
    let mut rt = runtime::<F>();
    rt.serve(IFACE, &[ORD]).expect("serve");
    let mut buf = [0u8; 8];
    let sent: Vec<_> = (0..F::SLOTS)
        .map(|_| {
            let c = rt
                .command(IFACE, ORD, &[1], Some(TRACE_A))
                .expect("a slot is free");
            let claim = rt
                .next_claim(&mut buf)
                .expect("next_claim")
                .expect("the call just sent");
            assert_eq!(claim.trace, Some(TRACE_A), "the old call carries it");
            rt.settle(claim.id, Ok(&[])).expect("settle");
            c
        })
        .collect();
    rt.forget(sent[0]);

    rt.command(IFACE, ORD, &[2], None)
        .expect("the reclaimed slot");
    let claim = rt
        .next_claim(&mut buf)
        .expect("next_claim")
        .expect("the new call");
    assert_eq!(claim.trace, None);
}
