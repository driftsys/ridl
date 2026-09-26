//! `correlate::Table` and `correlate::Waiters`: the caller-side call table and
//! the per-handle waiter registry every runtime with asynchronous replies
//! needs (ADR-0021 decision 15; notes F-5 and F-9 of the async face design).
//! Each test is named for the sentence of that record it pins.

#![forbid(unsafe_code)]

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::task::{Wake, Waker};

use ridl_rt::contract::InterfaceNo;
use ridl_rt::correlate::{Forgotten, Settled, Table, Waiters};
use ridl_rt::error::{CallError, Contract, Transport};
use ridl_rt::port::{Correlation, Interest};

/// A waker that counts its wakes, built over `std::task::Wake` so the test
/// needs no `unsafe` and no toolchain newer than the crate's.
struct Count(AtomicUsize);

impl Wake for Count {
    fn wake(self: Arc<Self>) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

fn counter() -> (Arc<Count>, Waker) {
    let count = Arc::new(Count(AtomicUsize::new(0)));
    let waker = Waker::from(Arc::clone(&count));
    (count, waker)
}

/// Wakes what a table or registry operation handed back, as a runtime does
/// after releasing its lock.
fn wake(wakers: impl IntoIterator<Item = Waker>) {
    for waker in wakers {
        waker.wake();
    }
}

fn wakes(count: &Count) -> usize {
    count.0.load(Ordering::SeqCst)
}

const REFUSED: CallError = CallError::Contract(Contract::PreconditionFailed);

// ---------------------------------------------------------------------------
// Slots and generations.
// ---------------------------------------------------------------------------

#[test]
fn a_correlation_is_the_generation_above_the_slot_index() {
    let mut table = Table::<4>::new(None);
    let first = table.insert(0).expect("a slot is free");
    let second = table.insert(0).expect("a slot is free");
    assert_eq!(Table::<4>::slot(first), 0);
    assert_eq!(Table::<4>::slot(second), 1);
    assert_eq!(first.0 >> 16, second.0 >> 16, "both slots are fresh");
}

#[test]
fn a_forgotten_slot_is_reused_under_a_new_generation() {
    let mut table = Table::<1>::new(None);
    let old = table.insert(0).expect("a slot is free");
    assert!(matches!(table.settle(old, Ok(())), Settled::Recorded(None)));
    assert!(matches!(table.forget(old), Forgotten::Reclaimed));

    let new = table.insert(0).expect("the forgotten slot is free again");
    assert_eq!(Table::<1>::slot(new), Table::<1>::slot(old), "same slot");
    assert_ne!(new, old, "a new generation, so a new correlation");
}

#[test]
fn a_generation_mismatch_answers_none_to_the_old_correlation() {
    let mut table = Table::<1>::new(None);
    let old = table.insert(0).expect("a slot is free");
    let _ = table.settle(old, Err(REFUSED));
    let _ = table.forget(old);
    let new = table.insert(0).expect("the slot is free again");
    let _ = table.settle(new, Ok(()));

    assert_eq!(table.outcome(new), Some(Ok(())));
    assert_eq!(table.outcome(old), None, "the old generation is gone");
    assert!(
        matches!(table.settle(old, Err(REFUSED)), Settled::Unknown),
        "a settlement under the old generation changes nothing"
    );
    assert_eq!(table.outcome(new), Some(Ok(())));
    assert!(matches!(table.forget(old), Forgotten::Unknown));
    assert_eq!(table.outcome(new), Some(Ok(())), "nor does a forget");
}

#[test]
fn a_correlation_the_table_never_issued_is_unknown() {
    let mut table = Table::<2>::new(None);
    let never = Correlation(1);
    assert_eq!(table.outcome(never), None);
    assert!(matches!(table.settle(never, Ok(())), Settled::Unknown));
    assert!(matches!(table.forget(never), Forgotten::Unknown));
    let beyond = Correlation(7);
    assert_eq!(table.outcome(beyond), None, "a slot index past N");
    assert!(table.insert(0).is_some(), "and both slots are still free");
    assert!(table.insert(0).is_some());
}

#[test]
fn insert_is_none_with_n_calls_in_flight_and_some_after_one_is_forgotten() {
    let mut table = Table::<3>::new(None);
    let calls: Vec<Correlation> = (0..3)
        .map(|_| table.insert(0).expect("a slot is free"))
        .collect();
    assert_eq!(table.insert(0), None, "every slot is in flight");

    let _ = table.settle(calls[1], Ok(()));
    assert_eq!(table.insert(0), None, "a settled slot is not free");
    assert!(matches!(table.forget(calls[1]), Forgotten::Reclaimed));
    let reused = table.insert(0).expect("the forgotten slot is free");
    assert_eq!(Table::<3>::slot(reused), 1);
}

#[test]
fn outcome_does_not_reclaim() {
    let mut table = Table::<1>::new(None);
    let c = table.insert(0).expect("a slot is free");
    assert_eq!(table.outcome(c), None, "in flight");
    let _ = table.settle(c, Err(CallError::Transport(Transport::Busy)));
    for _ in 0..3 {
        assert_eq!(
            table.outcome(c),
            Some(Err(CallError::Transport(Transport::Busy)))
        );
    }
    assert_eq!(table.insert(0), None, "reading the outcome freed nothing");
}

#[test]
fn the_first_settlement_is_the_outcome_and_a_second_is_unknown() {
    let mut table = Table::<1>::new(None);
    let c = table.insert(0).expect("a slot is free");
    assert!(matches!(table.settle(c, Ok(())), Settled::Recorded(None)));
    assert!(matches!(table.settle(c, Err(REFUSED)), Settled::Unknown));
    assert_eq!(table.outcome(c), Some(Ok(())));
}

// ---------------------------------------------------------------------------
// The byte budget.
// ---------------------------------------------------------------------------

#[test]
fn the_budget_refuses_a_reservation_that_does_not_fit_and_accepts_it_after_a_reclaim() {
    let mut table = Table::<4>::new(Some(100));
    let big = table.insert(60).expect("60 of 100");
    assert_eq!(table.insert(50), None, "40 left, a slot free, no budget");
    let small = table.insert(40).expect("exactly the 40 left");
    assert_eq!(table.insert(1), None, "nothing left");

    let _ = table.settle(big, Ok(()));
    assert_eq!(table.insert(50), None, "a settlement credits nothing");
    let _ = table.forget(big);
    assert!(table.insert(50).is_some(), "the reclaim credited 60");
    assert!(matches!(table.forget(small), Forgotten::Marked(None)));
    assert!(
        table.insert(11).is_none(),
        "10 left: a call in flight credits nothing when it is marked"
    );
    assert!(matches!(table.settle(small, Ok(())), Settled::Reclaimed));
    assert!(
        table.insert(50).is_some(),
        "the settlement of the marked call credited its 40"
    );
}

#[test]
fn no_budget_admits_any_reservation() {
    let mut table = Table::<2>::new(None);
    assert!(table.insert(u64::MAX).is_some());
    assert!(table.insert(u64::MAX).is_some());
}

// ---------------------------------------------------------------------------
// Reclaim: `forget` alone reclaims.
// ---------------------------------------------------------------------------

#[test]
fn forget_of_a_settled_call_reclaims_at_once() {
    let mut table = Table::<1>::new(Some(10));
    let c = table.insert(10).expect("a slot is free");
    let _ = table.settle(c, Ok(()));
    assert!(matches!(table.forget(c), Forgotten::Reclaimed));
    assert_eq!(table.outcome(c), None);
    assert!(
        table.insert(10).is_some(),
        "the slot and its bytes are free"
    );
}

#[test]
fn forget_of_a_call_in_flight_marks_it_and_the_settlement_then_reclaims() {
    let mut table = Table::<1>::new(Some(10));
    let c = table.insert(10).expect("a slot is free");
    assert!(matches!(table.forget(c), Forgotten::Marked(None)));
    assert_eq!(table.outcome(c), None, "a forgotten call has no outcome");
    assert_eq!(table.insert(1), None, "the slot is still held");
    assert!(
        matches!(table.forget(c), Forgotten::Unknown),
        "a second forget finds nothing to release"
    );

    assert!(matches!(table.settle(c, Ok(())), Settled::Reclaimed));
    assert_eq!(table.outcome(c), None, "the settlement is not readable");
    assert!(
        table.insert(10).is_some(),
        "the slot and its bytes are free"
    );
}

// ---------------------------------------------------------------------------
// The `Outcome` waker kept in the slot.
// ---------------------------------------------------------------------------

#[test]
fn settle_returns_the_stored_waker_once() {
    let mut table = Table::<1>::new(None);
    let c = table.insert(0).expect("a slot is free");
    let (count, waker) = counter();
    assert!(table.wake_on(c, &waker).is_none(), "nothing to displace");

    match table.settle(c, Ok(())) {
        Settled::Recorded(Some(stored)) => wake([stored]),
        other => panic!("expected the stored waker, found {other:?}"),
    }
    assert_eq!(wakes(&count), 1);
    assert!(
        matches!(table.settle(c, Ok(())), Settled::Unknown),
        "the waker was handed back once and is no longer stored"
    );
    let _ = table.forget(c);
    assert_eq!(wakes(&count), 1);
}

#[test]
fn wake_on_returns_the_displaced_waker_of_another_task() {
    let mut table = Table::<1>::new(None);
    let c = table.insert(0).expect("a slot is free");
    let (first, first_waker) = counter();
    let (second, second_waker) = counter();
    assert!(table.wake_on(c, &first_waker).is_none());
    wake(table.wake_on(c, &second_waker));
    assert_eq!(wakes(&first), 1, "the displaced waker is handed back");

    if let Settled::Recorded(stored) = table.settle(c, Ok(())) {
        wake(stored);
    }
    assert_eq!(
        (wakes(&first), wakes(&second)),
        (1, 1),
        "the new one is kept"
    );
}

#[test]
fn wake_on_of_the_same_task_is_a_refresh_and_returns_nothing() {
    let mut table = Table::<1>::new(None);
    let c = table.insert(0).expect("a slot is free");
    let (count, waker) = counter();
    assert!(table.wake_on(c, &waker).is_none());
    assert!(
        table.wake_on(c, &waker.clone()).is_none(),
        "a waker that will_wake the stored one displaces nothing"
    );
    if let Settled::Recorded(stored) = table.settle(c, Ok(())) {
        wake(stored);
    }
    assert_eq!(wakes(&count), 1, "the refreshed waker is still woken");
}

#[test]
fn wake_on_hands_the_waker_back_at_once_when_no_outcome_is_still_to_come() {
    let mut table = Table::<2>::new(None);
    let (count, waker) = counter();

    let settled = table.insert(0).expect("a slot is free");
    let _ = table.settle(settled, Ok(()));
    wake(table.wake_on(settled, &waker));
    assert_eq!(wakes(&count), 1, "the outcome is already known");

    let forgotten = table.insert(0).expect("a slot is free");
    let _ = table.forget(forgotten);
    wake(table.wake_on(forgotten, &waker));
    assert_eq!(wakes(&count), 2, "a forgotten call's outcome is never read");

    wake(table.wake_on(Correlation(1 << 16), &waker));
    assert_eq!(wakes(&count), 3, "a correlation the table does not hold");

    // None of the three was stored: a settlement and a reclaim hand back no
    // waker.
    assert!(matches!(
        table.settle(forgotten, Ok(())),
        Settled::Reclaimed
    ));
    assert!(matches!(table.forget(settled), Forgotten::Reclaimed));
    assert_eq!(wakes(&count), 3);
}

#[test]
fn forget_of_a_call_in_flight_hands_back_its_waker() {
    let mut table = Table::<1>::new(None);
    let c = table.insert(0).expect("a slot is free");
    let (count, waker) = counter();
    let _ = table.wake_on(c, &waker);
    match table.forget(c) {
        Forgotten::Marked(Some(stored)) => wake([stored]),
        other => panic!("expected the stored waker, found {other:?}"),
    }
    assert_eq!(wakes(&count), 1, "no outcome will ever be readable for it");
    assert!(matches!(table.settle(c, Ok(())), Settled::Reclaimed));
    assert_eq!(wakes(&count), 1, "the reclaim hands back nothing more");
}

/// `Table::new` is a `const fn`, so a runtime can hold its table in a
/// `static` behind its own lock.
#[test]
fn a_table_can_be_built_in_a_const_context() {
    const EMPTY: Table<2> = Table::new(Some(8));
    let mut table = EMPTY;
    assert!(table.insert(8).is_some());
}

// ---------------------------------------------------------------------------
// `Waiters`.
// ---------------------------------------------------------------------------

const IFACE: InterfaceNo = InterfaceNo(1);
const OTHER: InterfaceNo = InterfaceNo(2);

#[test]
fn waiters_hold_one_waker_per_kind() {
    let mut waiters = Waiters::new();
    let (slot, slot_waker) = counter();
    let (event, event_waker) = counter();
    let (claim, claim_waker) = counter();
    assert!(waiters.register(Interest::Slot, &slot_waker).is_none());
    assert!(waiters
        .register(Interest::Event(IFACE), &event_waker)
        .is_none());
    assert!(waiters
        .register(Interest::Claim(IFACE), &claim_waker)
        .is_none());
    assert_eq!(
        (wakes(&slot), wakes(&event), wakes(&claim)),
        (0, 0, 0),
        "three kinds, three wakers, nothing displaced"
    );

    wake(waiters.take(Interest::Event(IFACE)));
    assert_eq!((wakes(&slot), wakes(&event), wakes(&claim)), (0, 1, 0));
    wake(waiters.take(Interest::Slot));
    assert_eq!((wakes(&slot), wakes(&event), wakes(&claim)), (1, 1, 0));
    wake(waiters.take(Interest::Claim(IFACE)));
    assert_eq!((wakes(&slot), wakes(&event), wakes(&claim)), (1, 1, 1));
}

#[test]
fn any_key_of_the_kind_takes_the_waker() {
    let mut waiters = Waiters::new();
    let (count, waker) = counter();
    let _ = waiters.register(Interest::Event(IFACE), &waker);
    wake(waiters.take(Interest::Event(OTHER)));
    assert_eq!(wakes(&count), 1, "registered under 1, taken under 2");

    let _ = waiters.register(Interest::Claim(OTHER), &waker);
    wake(waiters.take(Interest::Claim(IFACE)));
    assert_eq!(wakes(&count), 2);
}

#[test]
fn a_registration_under_another_key_of_the_kind_by_the_same_task_is_a_refresh() {
    let mut waiters = Waiters::new();
    let (count, waker) = counter();
    let _ = waiters.register(Interest::Event(IFACE), &waker);
    assert!(
        waiters
            .register(Interest::Event(OTHER), &waker.clone())
            .is_none(),
        "one waker per kind, and the same task"
    );
    wake(waiters.take(Interest::Event(IFACE)));
    assert_eq!(wakes(&count), 1, "the refreshed waker is still stored");
}

#[test]
fn register_returns_the_displaced_waker_of_another_task() {
    let mut waiters = Waiters::new();
    let (first, first_waker) = counter();
    let (second, second_waker) = counter();
    let _ = waiters.register(Interest::Claim(IFACE), &first_waker);
    wake(waiters.register(Interest::Claim(OTHER), &second_waker));
    assert_eq!((wakes(&first), wakes(&second)), (1, 0));
    wake(waiters.take(Interest::Claim(IFACE)));
    assert_eq!(
        (wakes(&first), wakes(&second)),
        (1, 1),
        "the new one is kept"
    );
}

#[test]
fn take_clears_the_kind() {
    let mut waiters = Waiters::new();
    let (count, waker) = counter();
    let _ = waiters.register(Interest::Slot, &waker);
    assert!(waiters.take(Interest::Slot).is_some());
    assert!(waiters.take(Interest::Slot).is_none(), "taken once");
    assert!(
        waiters.register(Interest::Slot, &waker).is_none(),
        "nothing is stored to displace"
    );
    assert_eq!(wakes(&count), 0);
}

#[test]
fn an_outcome_key_is_not_stored_and_is_handed_back_to_be_woken() {
    let mut waiters = Waiters::new();
    let (count, waker) = counter();
    wake(waiters.register(Interest::Outcome(Correlation(0)), &waker));
    assert_eq!(wakes(&count), 1, "the table holds `Outcome`, not `Waiters`");
    assert!(waiters.take(Interest::Outcome(Correlation(0))).is_none());
    assert_eq!(waiters.take_all().count(), 0, "nothing was stored");
}

#[test]
fn take_all_takes_every_kind_for_an_unkeyed_runtime() {
    let mut waiters = Waiters::default();
    let (count, waker) = counter();
    let (other, other_waker) = counter();
    let _ = waiters.register(Interest::Slot, &waker);
    let _ = waiters.register(Interest::Event(IFACE), &other_waker);
    let _ = waiters.register(Interest::Claim(IFACE), &waker);
    wake(waiters.take_all());
    assert_eq!((wakes(&count), wakes(&other)), (2, 1));
    assert_eq!(waiters.take_all().count(), 0, "every kind is cleared");
}
