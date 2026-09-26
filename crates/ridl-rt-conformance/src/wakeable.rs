//! `Wakeable`: which change wakes which stored waker, and when.
//!
//! The contract is the trait's own documentation and ADR-0021 decision 13: a
//! handle stores one waker per kind of key — `Slot`, `Event`, `Claim` — and
//! an `Outcome` waker with its call; a change to any key of that kind wakes
//! the stored waker, which is cleared when woken; a registration by the same
//! task is a refresh and wakes nothing; a registration by another task
//! displaces the stored waker and wakes it. The slot rules are ADR-0021
//! decision 15: every `Slot` waker is woken when a slot is reclaimed.
//!
//! Every test here asks for `Wakeable` on each handle it registers on, so
//! these tests are in their own arm of [`suite!`](crate::suite).
//!
//! A spurious wake is allowed: a runtime may wake a waiter for a change that
//! is not its key's. So no test here asserts that a change leaves a waker
//! unwoken, except the one change the contract rules out, a refresh. A test
//! asserts that a waker is not yet woken only right after its registration,
//! with no change made since, or after it was woken once and cleared.
//! Whether a registration whose key already holds is woken at once is the
//! runtime's too: the task reads the port after it registers and finds the
//! change either way. The crate documentation, "What the suite leaves out",
//! lists both.

use std::cell::RefCell;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, mpsc};
use std::task::{Wake, Waker};

use ridl_rt::contract::InterfaceNo;
use ridl_rt::port::{Caller, Correlation, EventSink, EventSource, Handler, Interest, Wakeable};

use crate::{Factory, IFACE, ORD, fill, runtime};

/// A second interface, for the tests of a key's interface.
const IFACE2: InterfaceNo = InterfaceNo(2);

/// A waker that counts its wakes. Each one stands for a task: two `Count`s
/// are two tasks, and a clone of one waker is the same task.
struct Count(AtomicUsize);

impl Wake for Count {
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

/// A new task: the counter and a waker that increments it.
fn task() -> (Arc<Count>, Waker) {
    let count = Arc::new(Count(AtomicUsize::new(0)));
    (Arc::clone(&count), Waker::from(count))
}

fn wakes(count: &Count) -> usize {
    count.0.load(Ordering::SeqCst)
}

/// An `Outcome` waker is kept with its call: two calls hold two wakers, and
/// neither registration displaces the other. A settlement wakes its call's
/// waker once, after which the outcome reads; the waker is cleared when
/// woken, so neither another task's registration on the settled call nor a
/// later settlement wakes it again. A task that registers after the
/// settlement is woken at once or reads the outcome on the read that follows.
///
/// This is the test note F-14 names the mutation for: a runtime whose
/// settlement does not wake the waiter fails it.
pub fn an_outcome_waker_is_kept_with_its_call_and_woken_once_by_the_settlement<F>()
where
    F: Factory,
    F::Runtime: Wakeable,
{
    let mut rt = runtime::<F>();
    rt.serve(IFACE, &[ORD]).expect("serve");
    let first = rt.command(IFACE, ORD, &[1]).expect("send");
    let second = rt.query(IFACE, ORD, &[2]).expect("send");
    // Both calls are claimed before the wakers are registered, so nothing
    // changes between a registration and the settlement that wakes it.
    let mut buf = [0u8; 8];
    let mut claims = [None, None];
    for _ in 0..2 {
        let claim = rt
            .next_claim(&mut buf)
            .expect("next_claim")
            .expect("waiting");
        claims[usize::from(buf[0] - 1)] = Some(claim.id);
    }
    let [Some(first_claim), Some(second_claim)] = claims else {
        panic!("each call was presented once");
    };

    let (a, a_waker) = task();
    let (b, b_waker) = task();
    rt.wake_on(Interest::Outcome(first), &a_waker);
    rt.wake_on(Interest::Outcome(second), &b_waker);
    assert_eq!(
        (wakes(&a), wakes(&b)),
        (0, 0),
        "two calls hold two wakers, and neither displaced the other"
    );

    rt.settle(second_claim, Ok(&[7])).expect("settle");
    assert_eq!(wakes(&b), 1, "the settlement wakes its call's waker");
    assert_eq!(rt.reply(second, &mut buf), Ok(Some(Ok(1))));
    rt.settle(first_claim, Ok(&[])).expect("settle");
    assert_eq!(wakes(&a), 1, "the settlement wakes its call's waker");
    assert_eq!(rt.ack(first), Some(Ok(())));

    // Another task registers on the settled call. Had the settlement left
    // the first waker stored, this registration would displace it and wake
    // it a second time.
    let (late, late_waker) = task();
    rt.wake_on(Interest::Outcome(first), &late_waker);
    assert_eq!(wakes(&a), 1, "a woken waker is cleared");
    assert!(
        wakes(&late) == 1 || rt.ack(first).is_some(),
        "a task registering after the settlement is woken or reads the outcome"
    );

    let third = rt.command(IFACE, ORD, &[3]).expect("send");
    let claim = rt
        .next_claim(&mut buf)
        .expect("next_claim")
        .expect("waiting");
    rt.settle(claim.id, Ok(&[])).expect("settle");
    assert_eq!(rt.ack(third), Some(Ok(())));
    assert_eq!(
        (wakes(&a), wakes(&b)),
        (1, 1),
        "a waker is woken at most once for one registration"
    );
}

/// With every slot taken, each caller's `Slot` waker is woken when a slot is
/// reclaimed, and the send that follows succeeds. A slot is reclaimed two
/// ways: a forget of a settled call, and the settlement of a claimed call
/// that was forgotten. A runtime may reclaim the second at the forget or at
/// the settlement; either way both wakers have been woken once both are
/// done.
pub fn every_slot_waker_is_woken_by_a_reclaim_and_the_send_that_follows_succeeds<F>()
where
    F: Factory,
    F::Runtime: Wakeable,
    F::Caller: Wakeable,
{
    let mut rt = runtime::<F>();
    let mut second = F::caller(&rt);
    rt.serve(IFACE, &[ORD]).expect("serve");
    let calls = fill::<F>(&mut rt);
    let mut buf = [0u8; 8];

    // A forget of a settled call.
    let (mine, my_waker) = task();
    let (theirs, their_waker) = task();
    rt.wake_on(Interest::Slot, &my_waker);
    second.wake_on(Interest::Slot, &their_waker);
    assert_eq!(
        (wakes(&mine), wakes(&theirs)),
        (0, 0),
        "no slot is free, and nothing has changed"
    );
    rt.forget(calls[0]);
    assert_eq!(
        (wakes(&mine), wakes(&theirs)),
        (1, 1),
        "the reclaim wakes every caller's slot waker"
    );
    let taken = second
        .command(IFACE, ORD, &[1])
        .expect("the send that follows takes the reclaimed slot");

    // The settlement of a claimed call that was forgotten.
    let (mine, my_waker) = task();
    let (theirs, their_waker) = task();
    rt.wake_on(Interest::Slot, &my_waker);
    second.wake_on(Interest::Slot, &their_waker);
    assert_eq!(
        (wakes(&mine), wakes(&theirs)),
        (0, 0),
        "the table is full again"
    );
    let claim = rt
        .next_claim(&mut buf)
        .expect("next_claim")
        .expect("the call the second caller sent");
    second.forget(taken);
    rt.settle(claim.id, Ok(&[]))
        .expect("the provider's settlement is still accepted");
    assert_eq!(
        (wakes(&mine), wakes(&theirs)),
        (1, 1),
        "the reclaim wakes every caller's slot waker"
    );
    rt.command(IFACE, ORD, &[1])
        .expect("the send that follows takes the reclaimed slot");
}

/// A raise wakes the `Event` waker of every source subscribed to it, once:
/// a second task waiting for the same events holds a second handle, and each
/// handle's waiter is woken. The occurrence reads after the wake, and a
/// woken waker is cleared, so a second raise does not wake it again.
pub fn every_subscribed_source_is_woken_once_by_a_raise<F>()
where
    F: Factory,
    F::Runtime: Wakeable,
    F::Source: Wakeable,
{
    let mut rt = runtime::<F>();
    let mut second = F::source(&rt);
    rt.subscribe(IFACE, &[ORD]).expect("subscribe");
    second.subscribe(IFACE, &[ORD]).expect("subscribe");

    let (a, a_waker) = task();
    let (b, b_waker) = task();
    rt.wake_on(Interest::Event(IFACE), &a_waker);
    second.wake_on(Interest::Event(IFACE), &b_waker);
    assert_eq!((wakes(&a), wakes(&b)), (0, 0), "nothing has been raised");

    rt.raise(IFACE, ORD, &[1]).expect("raise");
    assert_eq!(
        (wakes(&a), wakes(&b)),
        (1, 1),
        "each subscribed source's waker is woken"
    );
    let mut out = [0u8; 8];
    assert!(rt.next(&mut out).expect("next").is_some(), "and it reads");
    assert!(second.next(&mut out).expect("next").is_some());

    rt.raise(IFACE, ORD, &[2]).expect("raise");
    assert_eq!(
        (wakes(&a), wakes(&b)),
        (1, 1),
        "a woken waker is cleared, so a second raise does not reach it"
    );
}

/// A command or a query to a member a handler serves wakes that handler's
/// `Claim` waker, once, and the claim reads after the wake. A woken waker is
/// cleared, so a second call does not wake it again.
pub fn a_call_to_a_served_member_wakes_the_handler_once<F>()
where
    F: Factory,
    F::Handler: Wakeable,
{
    let mut rt = runtime::<F>();
    let mut handler = F::handler(&rt);
    handler.serve(IFACE, &[ORD]).expect("serve");
    let mut buf = [0u8; 8];

    for query in [false, true] {
        let (count, waker) = task();
        handler.wake_on(Interest::Claim(IFACE), &waker);
        assert_eq!(wakes(&count), 0, "no call is waiting");
        let sent = if query {
            rt.query(IFACE, ORD, &[1])
        } else {
            rt.command(IFACE, ORD, &[1])
        };
        sent.expect("send");
        assert_eq!(wakes(&count), 1, "the call wakes the serving handler");
        rt.command(IFACE, ORD, &[2]).expect("send");
        assert_eq!(
            wakes(&count),
            1,
            "a woken waker is cleared, so a second call does not reach it"
        );
        for _ in 0..2 {
            let claim = handler
                .next_claim(&mut buf)
                .expect("next_claim")
                .expect("the claim reads after the wake");
            handler.settle(claim.id, Ok(&[])).expect("settle");
        }
    }
}

/// A registration by another task displaces the stored waker of its kind and
/// wakes it, so no task waits on a registration that can no longer fire; the
/// change then wakes the waker that displaced it. `Slot`, `Event` and `Claim`
/// hold one waker per kind, so a registration under another interface of the
/// same kind displaces too. An `Outcome` waker is kept with its call, so a
/// registration on the same call displaces.
pub fn another_tasks_registration_displaces_the_stored_waker_and_wakes_it<F>()
where
    F: Factory,
    F::Caller: Wakeable,
    F::Source: Wakeable,
    F::Handler: Wakeable,
{
    let mut rt = runtime::<F>();
    let mut caller = F::caller(&rt);
    let mut source = F::source(&rt);
    let mut handler = F::handler(&rt);
    rt.serve(IFACE, &[ORD]).expect("serve");
    source.subscribe(IFACE, &[ORD]).expect("subscribe");
    let mut buf = [0u8; 8];

    // An outcome.
    let c = caller.command(IFACE, ORD, &[1]).expect("send");
    let claim = rt
        .next_claim(&mut buf)
        .expect("next_claim")
        .expect("waiting");
    let (a, a_waker) = task();
    let (b, b_waker) = task();
    caller.wake_on(Interest::Outcome(c), &a_waker);
    caller.wake_on(Interest::Outcome(c), &b_waker);
    assert_eq!((wakes(&a), wakes(&b)), (1, 0), "`Outcome`: B displaces A");
    rt.settle(claim.id, Ok(&[])).expect("settle");
    assert_eq!(
        (wakes(&a), wakes(&b)),
        (1, 1),
        "`Outcome`: the settlement wakes B"
    );
    caller.forget(c);

    // An event, under one interface and then under another.
    let (a, a_waker) = task();
    let (b, b_waker) = task();
    let (c, c_waker) = task();
    source.wake_on(Interest::Event(IFACE), &a_waker);
    source.wake_on(Interest::Event(IFACE), &b_waker);
    assert_eq!(
        (wakes(&a), wakes(&b)),
        (1, 0),
        "`Event`: B displaces A under the same interface"
    );
    source.wake_on(Interest::Event(IFACE2), &c_waker);
    assert_eq!(
        (wakes(&b), wakes(&c)),
        (1, 0),
        "`Event`: C displaces B under another interface of the kind"
    );
    rt.raise(IFACE, ORD, &[1]).expect("raise");
    assert_eq!(
        (wakes(&a), wakes(&b), wakes(&c)),
        (1, 1, 1),
        "`Event`: the raise wakes C, the one waker stored"
    );

    // A claim, under one interface and then under another.
    handler.serve(IFACE, &[ORD]).expect("serve");
    let (a, a_waker) = task();
    let (b, b_waker) = task();
    let (c, c_waker) = task();
    handler.wake_on(Interest::Claim(IFACE), &a_waker);
    handler.wake_on(Interest::Claim(IFACE), &b_waker);
    assert_eq!(
        (wakes(&a), wakes(&b)),
        (1, 0),
        "`Claim`: B displaces A under the same interface"
    );
    handler.wake_on(Interest::Claim(IFACE2), &c_waker);
    assert_eq!(
        (wakes(&b), wakes(&c)),
        (1, 0),
        "`Claim`: C displaces B under another interface of the kind"
    );
    caller.command(IFACE, ORD, &[2]).expect("send");
    assert_eq!(
        (wakes(&a), wakes(&b), wakes(&c)),
        (1, 1, 1),
        "`Claim`: the call wakes C, the one waker stored"
    );

    // A slot. The table is filled on a new runtime, so no call above holds a
    // slot of it.
    let mut rt = runtime::<F>();
    let mut caller = F::caller(&rt);
    rt.serve(IFACE, &[ORD]).expect("serve");
    let calls = fill::<F>(&mut rt);
    let (a, a_waker) = task();
    let (b, b_waker) = task();
    caller.wake_on(Interest::Slot, &a_waker);
    caller.wake_on(Interest::Slot, &b_waker);
    assert_eq!((wakes(&a), wakes(&b)), (1, 0), "`Slot`: B displaces A");
    rt.forget(calls[0]);
    assert_eq!(
        (wakes(&a), wakes(&b)),
        (1, 1),
        "`Slot`: the reclaim wakes B"
    );
    caller
        .command(IFACE, ORD, &[1])
        .expect("the reclaimed slot");
}

/// A registration whose waker [`will_wake`](Waker::will_wake) the stored
/// one, which a task makes on every poll, is a refresh: it wakes nothing,
/// whatever interface either key names, and the change still wakes the task
/// once. A refresh that woke the task would schedule its next poll from every
/// poll, and the task would never become idle.
pub fn the_same_tasks_registration_is_a_refresh_that_wakes_nothing<F>()
where
    F: Factory,
    F::Caller: Wakeable,
    F::Source: Wakeable,
    F::Handler: Wakeable,
{
    let mut rt = runtime::<F>();
    let mut caller = F::caller(&rt);
    let mut source = F::source(&rt);
    let mut handler = F::handler(&rt);
    rt.serve(IFACE, &[ORD]).expect("serve");
    source.subscribe(IFACE, &[ORD]).expect("subscribe");
    let mut buf = [0u8; 8];

    // An outcome.
    let c = caller.command(IFACE, ORD, &[1]).expect("send");
    let claim = rt
        .next_claim(&mut buf)
        .expect("next_claim")
        .expect("waiting");
    let (count, waker) = task();
    caller.wake_on(Interest::Outcome(c), &waker);
    caller.wake_on(Interest::Outcome(c), &waker.clone());
    assert_eq!(wakes(&count), 0, "`Outcome`: a refresh wakes nothing");
    rt.settle(claim.id, Ok(&[])).expect("settle");
    assert_eq!(wakes(&count), 1, "`Outcome`: the settlement wakes the task");
    caller.forget(c);

    // An event, refreshed under another interface of the kind.
    let (count, waker) = task();
    source.wake_on(Interest::Event(IFACE2), &waker);
    source.wake_on(Interest::Event(IFACE), &waker.clone());
    assert_eq!(wakes(&count), 0, "`Event`: a refresh wakes nothing");
    rt.raise(IFACE, ORD, &[1]).expect("raise");
    assert_eq!(wakes(&count), 1, "`Event`: the raise wakes the task");

    // A claim, refreshed under another interface of the kind.
    handler.serve(IFACE, &[ORD]).expect("serve");
    let (count, waker) = task();
    handler.wake_on(Interest::Claim(IFACE2), &waker);
    handler.wake_on(Interest::Claim(IFACE), &waker.clone());
    assert_eq!(wakes(&count), 0, "`Claim`: a refresh wakes nothing");
    caller.command(IFACE, ORD, &[2]).expect("send");
    assert_eq!(wakes(&count), 1, "`Claim`: the call wakes the task");

    // A slot, on a new runtime whose table is full.
    let mut rt = runtime::<F>();
    let mut caller = F::caller(&rt);
    rt.serve(IFACE, &[ORD]).expect("serve");
    let calls = fill::<F>(&mut rt);
    let (count, waker) = task();
    caller.wake_on(Interest::Slot, &waker);
    caller.wake_on(Interest::Slot, &waker.clone());
    assert_eq!(wakes(&count), 0, "`Slot`: a refresh wakes nothing");
    rt.forget(calls[0]);
    assert_eq!(wakes(&count), 1, "`Slot`: the reclaim wakes the task");
    caller
        .command(IFACE, ORD, &[1])
        .expect("the reclaimed slot");
}

/// A handle keeps one `Event` waker and one `Claim` waker, not one per
/// interface, so a change on any interface the handle observes wakes the
/// waker registered under another. A runtime that keeps a waker per key, and
/// wakes it only for that key, leaves both tasks here waiting.
pub fn one_waker_per_kind_is_woken_by_a_change_on_any_interface_of_that_kind<F>()
where
    F: Factory,
    F::Source: Wakeable,
    F::Handler: Wakeable,
{
    let mut rt = runtime::<F>();
    let mut source = F::source(&rt);
    let mut handler = F::handler(&rt);
    source.subscribe(IFACE2, &[ORD]).expect("subscribe");
    handler.serve(IFACE2, &[ORD]).expect("serve");

    let (event, event_waker) = task();
    source.wake_on(Interest::Event(IFACE), &event_waker);
    assert_eq!(wakes(&event), 0, "nothing has been raised");
    rt.raise(IFACE2, ORD, &[1]).expect("raise");
    assert_eq!(wakes(&event), 1, "an occurrence on interface 2 wakes it");

    let (claim, claim_waker) = task();
    handler.wake_on(Interest::Claim(IFACE), &claim_waker);
    assert_eq!(wakes(&claim), 0, "no call is waiting");
    rt.command(IFACE2, ORD, &[1]).expect("send");
    assert_eq!(wakes(&claim), 1, "a call on interface 2 wakes it");
}

// ---------------------------------------------------------------------------
// A wake runs with the runtime's lock released.
// ---------------------------------------------------------------------------

thread_local! {
    /// What a [`CallsBack`] waker runs when it is woken on this thread: a
    /// read through a handle of the runtime under test. It is kept here, not
    /// in the waker, because a `Waker` must be `Send` and `Sync` and a
    /// runtime's handles need not be (ADR-0021 decision 12).
    static PROBE: RefCell<Option<Box<dyn FnMut()>>> = const { RefCell::new(None) };
}

/// A waker that, when woken, calls into the runtime through the probe of its
/// thread, and counts the wakes in which it did.
struct CallsBack(AtomicUsize);

impl Wake for CallsBack {
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        PROBE.with(|probe| {
            if let Some(probe) = probe.borrow_mut().as_mut() {
                probe();
                self.0.fetch_add(1, Ordering::SeqCst);
            }
        });
    }
}

fn calling_back() -> (Arc<CallsBack>, Waker) {
    let count = Arc::new(CallsBack(AtomicUsize::new(0)));
    (Arc::clone(&count), Waker::from(count))
}

/// Every wake runs with the runtime's lock released: a waker that calls back
/// into the port when woken, as an executor that polls the woken task at
/// once does, returns. A runtime that wakes while it holds its lock
/// deadlocks here, or panics when its lock refuses to be taken twice, and
/// the test fails after five seconds rather than hanging.
///
/// The waker calls back through three handles of its own — a caller, a
/// source and a handler — so the lock of each role is taken. Each wake path
/// the tests above use is taken once: a settlement, a raise, a command, a
/// query, a displacement, a forget that reclaims a slot, and the reclaim of
/// a forgotten call in flight.
///
/// The scenario runs on a thread of its own, and builds its runtime there,
/// so no handle moves between threads; a runtime that wakes a waiter on a
/// thread of its own, not on the one that made the change, fails the count
/// of wakes that called back.
pub fn every_wake_runs_with_the_runtime_lock_released<F>()
where
    F: Factory + 'static,
    F::Caller: Wakeable + 'static,
    F::Source: Wakeable + 'static,
    F::Handler: Wakeable + 'static,
{
    let (done, finished) = mpsc::channel();
    let scenario = std::thread::spawn(move || {
        wakes_with_the_lock_released::<F>();
        let _ = done.send(());
    });
    match finished.recv_timeout(std::time::Duration::from_secs(5)) {
        Ok(()) => scenario.join().expect("the scenario finished"),
        // The scenario panicked: report its own message.
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            std::panic::resume_unwind(scenario.join().expect_err("the scenario panicked"));
        }
        // The thread is not joined, on purpose: a deadlocked scenario never
        // finishes, and a join would wait for it forever.
        Err(mpsc::RecvTimeoutError::Timeout) => panic!(
            "a wake did not return within five seconds: the runtime woke a \
             waker while it held a lock the waker's call back into the port \
             takes"
        ),
    }
}

/// Installs the probe on this thread: a read through each of a new caller,
/// a new source and a new handler of `rt`. The handler serves an interface
/// nothing is sent to, and the source subscribes to nothing, so the reads
/// find nothing and change nothing.
fn install_probe<F>(rt: &F::Runtime)
where
    F: Factory,
    F::Caller: 'static,
    F::Source: 'static,
    F::Handler: 'static,
{
    let mut caller = F::caller(rt);
    let mut source = F::source(rt);
    let mut handler = F::handler(rt);
    handler.serve(InterfaceNo(99), &[ORD]).expect("serve");
    PROBE.with(|probe| {
        *probe.borrow_mut() = Some(Box::new(move || {
            let mut buf = [0u8; 8];
            let _ = caller.ack(Correlation(0));
            let _ = source.next(&mut buf);
            let _ = handler.next_claim(&mut buf);
        }));
    });
}

fn assert_called_back(count: &CallsBack, path: &str) {
    assert_eq!(
        count.0.load(Ordering::SeqCst),
        1,
        "{path}: the woken waker called back into the port"
    );
}

fn wakes_with_the_lock_released<F>()
where
    F: Factory,
    F::Caller: Wakeable + 'static,
    F::Source: Wakeable + 'static,
    F::Handler: Wakeable + 'static,
{
    let mut rt = runtime::<F>();
    let mut caller = F::caller(&rt);
    let mut source = F::source(&rt);
    let mut handler = F::handler(&rt);
    source.subscribe(IFACE, &[ORD]).expect("subscribe");
    handler.serve(IFACE, &[ORD]).expect("serve");
    install_probe::<F>(&rt);
    let mut buf = [0u8; 8];

    let c = caller.command(IFACE, ORD, &[1]).expect("send");
    let claim = handler
        .next_claim(&mut buf)
        .expect("next_claim")
        .expect("waiting");
    let (count, waker) = calling_back();
    caller.wake_on(Interest::Outcome(c), &waker);
    handler.settle(claim.id, Ok(&[])).expect("settle");
    assert_called_back(&count, "a settlement");
    caller.forget(c);

    let (count, waker) = calling_back();
    source.wake_on(Interest::Event(IFACE), &waker);
    rt.raise(IFACE, ORD, &[1]).expect("raise");
    assert_called_back(&count, "a raise");

    // Each call is claimed and settled before the next registration, so no
    // call is waiting when the handler registers.
    for query in [false, true] {
        let (count, waker) = calling_back();
        handler.wake_on(Interest::Claim(IFACE), &waker);
        let c = if query {
            caller.query(IFACE, ORD, &[1])
        } else {
            caller.command(IFACE, ORD, &[1])
        }
        .expect("send");
        assert_called_back(&count, if query { "a query" } else { "a command" });
        let claim = handler
            .next_claim(&mut buf)
            .expect("next_claim")
            .expect("waiting");
        handler.settle(claim.id, Ok(&[])).expect("settle");
        caller.forget(c);
    }

    let (count, waker) = calling_back();
    let (_, other) = task();
    handler.wake_on(Interest::Claim(IFACE), &waker);
    handler.wake_on(Interest::Claim(IFACE), &other);
    assert_called_back(&count, "a displacement");

    // A slot, on a new runtime whose table is full, with a probe of its own.
    let mut rt = runtime::<F>();
    let mut caller = F::caller(&rt);
    install_probe::<F>(&rt);
    rt.serve(IFACE, &[ORD]).expect("serve");
    let calls = fill::<F>(&mut rt);

    let (count, waker) = calling_back();
    caller.wake_on(Interest::Slot, &waker);
    rt.forget(calls[0]);
    assert_called_back(&count, "a forget that reclaims a slot");

    let taken = caller
        .command(IFACE, ORD, &[1])
        .expect("the reclaimed slot");
    let claim = rt
        .next_claim(&mut buf)
        .expect("next_claim")
        .expect("the call just sent");
    // Registered before the forget, so the waker is woken by whichever of
    // the forget and the settlement reclaims the slot.
    let (count, waker) = calling_back();
    caller.wake_on(Interest::Slot, &waker);
    caller.forget(taken);
    rt.settle(claim.id, Ok(&[])).expect("settle");
    assert_called_back(&count, "a reclaim of a forgotten call in flight");

    PROBE.with(|probe| probe.borrow_mut().take());
}
