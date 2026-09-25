//! `block_on` and `noop_waker` under the `std` feature (story E11.17).
//!
//! The whole file is gated on the feature, because `just test` builds the
//! workspace with default features and the module does not exist there. The
//! tests run through the `cargo test -p ridl-rt --all-features` line of
//! `just test`, and again under `just compat-check`.
//!
//! Every timing assertion here is a lower bound — "the wait lasted at least
//! the delay" — or a bound so wide that a loaded machine cannot cross it, so
//! the tests do not depend on scheduling latency. A poll-count ceiling is wide
//! in the same way: a parked wait polls once before the wait and once per wake
//! or timeout, so a ceiling of eight catches a wait that spins, and tolerates
//! the few extra wakes a platform may deliver. That each park is given only
//! the time left until the deadline is not pinned by any test here. The
//! spurious-wake test asserts that at least one such wake polled the future,
//! not how many did, because the thread that sends them is not scheduled at a
//! fixed cadence.
#![cfg(feature = "std")]

use std::future::{poll_fn, ready, Future};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Poll, Waker};
use std::thread;
use std::time::{Duration, Instant};

use ridl_rt::task::{block_on, noop_waker};

/// A future that stays pending until `done` is set, and that stores the waker
/// of its last poll so another thread can wake it. `polls` counts the polls.
struct Gate {
    done: AtomicBool,
    waker: Mutex<Option<Waker>>,
    polls: AtomicUsize,
}

impl Gate {
    fn new() -> Arc<Gate> {
        Arc::new(Gate {
            done: AtomicBool::new(false),
            waker: Mutex::new(None),
            polls: AtomicUsize::new(0),
        })
    }

    fn future(self: &Arc<Self>) -> impl Future<Output = u32> {
        let gate = Arc::clone(self);
        poll_fn(move |cx| {
            gate.polls.fetch_add(1, Ordering::SeqCst);
            if gate.done.load(Ordering::SeqCst) {
                Poll::Ready(42)
            } else {
                *gate.waker.lock().unwrap() = Some(cx.waker().clone());
                Poll::Pending
            }
        })
    }

    /// Complete the future and wake whoever polled it last.
    fn open(&self) {
        self.done.store(true, Ordering::SeqCst);
        if let Some(waker) = self.waker.lock().unwrap().take() {
            waker.wake();
        }
    }
}

#[test]
fn block_on_returns_the_output_when_another_thread_wakes_the_future() {
    let gate = Gate::new();
    let delay = Duration::from_millis(50);
    let start = Instant::now();
    let opener = {
        let gate = Arc::clone(&gate);
        thread::spawn(move || {
            thread::sleep(delay);
            gate.open();
        })
    };

    // With no deadline, a lost wake would park this thread forever. The
    // rescue thread turns that into a failure: after `limit` it completes the
    // future and unparks this thread directly, without the waker, so the
    // assertion on the elapsed time fails instead of the test never ending.
    let limit = Duration::from_secs(5);
    let stop = Arc::new(AtomicBool::new(false));
    let rescue = {
        let gate = Arc::clone(&gate);
        let stop = Arc::clone(&stop);
        let waiter = thread::current();
        thread::spawn(move || {
            while !stop.load(Ordering::SeqCst) {
                if start.elapsed() > limit {
                    gate.done.store(true, Ordering::SeqCst);
                    waiter.unpark();
                    return;
                }
                thread::sleep(Duration::from_millis(10));
            }
        })
    };

    let out = block_on(gate.future(), None);
    let elapsed = start.elapsed();
    stop.store(true, Ordering::SeqCst);
    opener.join().unwrap();
    rescue.join().unwrap();

    assert_eq!(out, Some(42));
    assert!(
        elapsed >= delay,
        "returned after {elapsed:?}, before the opener ran"
    );
    assert!(
        elapsed < limit,
        "the wake was lost: the rescue thread ended the wait at {elapsed:?}"
    );
    let polls = gate.polls.load(Ordering::SeqCst);
    assert!(
        polls >= 2,
        "the future is polled once before the wait and once after the wake"
    );
    assert!(
        polls <= 8,
        "a parked wait polls on a wake only, not in a loop: {polls} polls"
    );
}

#[test]
fn block_on_with_a_far_deadline_returns_the_output_when_woken_before_it() {
    let gate = Gate::new();
    let delay = Duration::from_millis(20);
    let far = Duration::from_secs(30);
    let start = Instant::now();
    let opener = {
        let gate = Arc::clone(&gate);
        thread::spawn(move || {
            thread::sleep(delay);
            gate.open();
        })
    };

    let out = block_on(gate.future(), Some(start + far));
    let elapsed = start.elapsed();
    opener.join().unwrap();

    assert_eq!(out, Some(42));
    assert!(elapsed >= delay);
    assert!(elapsed < far, "the wake, not the deadline, ended the wait");
}

#[test]
fn a_wake_during_a_poll_is_not_lost() {
    // The future wakes its own waker by reference inside its first poll, before
    // the thread parks, and is ready on its second poll. The unpark token has
    // to survive from the poll to the park that follows it, or the wait runs
    // to the deadline.
    let polls = Arc::new(AtomicUsize::new(0));
    let fut = {
        let polls = Arc::clone(&polls);
        poll_fn(move |cx| {
            let n = polls.fetch_add(1, Ordering::SeqCst) + 1;
            if n == 1 {
                cx.waker().wake_by_ref();
                Poll::Pending
            } else {
                Poll::Ready(n)
            }
        })
    };
    let limit = Duration::from_secs(5);

    let start = Instant::now();
    let out = block_on(fut, Some(start + limit));
    let elapsed = start.elapsed();

    assert_eq!(out, Some(2));
    assert!(
        elapsed < Duration::from_secs(1),
        "the wake from inside the poll was lost: the wait took {elapsed:?}"
    );
}

#[test]
fn a_wake_between_a_poll_and_the_park_is_not_lost_without_a_deadline() {
    // With no deadline, `block_on` waits with `thread::park`, not
    // `park_timeout`, so a lost wake parks this thread forever. The future,
    // on its first poll, hands a clone of its waker to another thread, which
    // wakes it by value; the poll joins that thread before it returns
    // `Pending`, so the wake lands after the poll and before the park that
    // follows it. The unpark token has to survive that gap. The future is
    // ready on its second poll.
    let polls = Arc::new(AtomicUsize::new(0));
    let fut = {
        let polls = Arc::clone(&polls);
        poll_fn(move |cx| {
            let n = polls.fetch_add(1, Ordering::SeqCst) + 1;
            if n == 1 {
                let waker = cx.waker().clone();
                thread::spawn(move || waker.wake()).join().unwrap();
                Poll::Pending
            } else {
                Poll::Ready(n)
            }
        })
    };

    // The rescue thread turns a lost wake into a failure: after `limit` it
    // unparks this thread directly, without the waker, so the assertion on
    // the elapsed time fails instead of the test never ending.
    let limit = Duration::from_secs(5);
    let stop = Arc::new(AtomicBool::new(false));
    let start = Instant::now();
    let rescue = {
        let stop = Arc::clone(&stop);
        let waiter = thread::current();
        thread::spawn(move || {
            while !stop.load(Ordering::SeqCst) {
                if start.elapsed() > limit {
                    waiter.unpark();
                    return;
                }
                thread::sleep(Duration::from_millis(10));
            }
        })
    };

    let out = block_on(fut, None);
    let elapsed = start.elapsed();
    stop.store(true, Ordering::SeqCst);
    rescue.join().unwrap();

    assert_eq!(out, Some(2));
    assert!(
        elapsed < limit,
        "the wake was lost: the rescue thread ended the wait at {elapsed:?}"
    );
}

#[test]
fn block_on_returns_none_when_the_deadline_passes() {
    let gate = Gate::new();
    let bound = Duration::from_millis(50);

    let start = Instant::now();
    let out = block_on(gate.future(), Some(start + bound));
    let elapsed = start.elapsed();

    assert_eq!(out, None);
    assert!(
        elapsed >= bound,
        "returned after {elapsed:?}, before the deadline"
    );
    assert!(
        elapsed < bound + Duration::from_millis(500),
        "the wait ended long after the deadline: {elapsed:?}"
    );
    let polls = gate.polls.load(Ordering::SeqCst);
    assert!(
        polls >= 2,
        "the future is polled once before the wait and once at the deadline"
    );
    assert!(
        polls <= 8,
        "a parked wait polls on a wake or the deadline only: {polls} polls"
    );
}

#[test]
fn block_on_polls_once_even_when_the_deadline_has_already_passed() {
    let past = Instant::now() - Duration::from_secs(1);
    assert_eq!(block_on(ready(7u8), Some(past)), Some(7));

    let gate = Gate::new();
    assert_eq!(block_on(gate.future(), Some(past)), None);
    assert_eq!(gate.polls.load(Ordering::SeqCst), 1);
}

#[test]
fn block_on_returns_the_output_of_a_ready_poll_after_the_deadline() {
    // The future is pending while the deadline is ahead and ready once it has
    // passed, and it never wakes its waker. So the first poll is `Pending`,
    // the park runs to the deadline, and the poll after that park, which runs
    // when `Instant::now() >= deadline`, is `Ready`. Its output is returned
    // as `Some`: the deadline turns a `Pending` poll into `None`, not a
    // `Ready` one.
    let bound = Duration::from_millis(100);
    let start = Instant::now();
    let deadline = start + bound;
    let polls = Arc::new(AtomicUsize::new(0));
    let fut = {
        let polls = Arc::clone(&polls);
        poll_fn(move |_cx| {
            polls.fetch_add(1, Ordering::SeqCst);
            if Instant::now() >= deadline {
                Poll::Ready(42)
            } else {
                Poll::Pending
            }
        })
    };

    let out = block_on(fut, Some(deadline));
    let elapsed = start.elapsed();

    assert_eq!(
        out,
        Some(42),
        "a poll that is ready after the deadline returns its output"
    );
    assert!(
        elapsed >= bound,
        "returned after {elapsed:?}, before the deadline"
    );
    assert!(
        polls.load(Ordering::SeqCst) >= 2,
        "the ready poll is the one after the park, not the first"
    );
}

#[test]
fn a_spurious_unpark_does_not_end_the_wait_early() {
    let gate = Gate::new();
    let bound = Duration::from_millis(100);
    let stop = Arc::new(AtomicBool::new(false));
    let waiter = thread::current();
    let nagger = {
        let stop = Arc::clone(&stop);
        thread::spawn(move || {
            while !stop.load(Ordering::SeqCst) {
                waiter.unpark();
                thread::sleep(Duration::from_millis(5));
            }
        })
    };

    let start = Instant::now();
    let out = block_on(gate.future(), Some(start + bound));
    let elapsed = start.elapsed();
    stop.store(true, Ordering::SeqCst);
    nagger.join().unwrap();

    assert_eq!(out, None);
    assert!(
        elapsed >= bound,
        "a spurious unpark ended the wait at {elapsed:?}"
    );
    assert!(
        gate.polls.load(Ordering::SeqCst) >= 3,
        "a spurious unpark polls the future once, and the wait continues"
    );
}

#[test]
fn noop_waker_can_be_cloned_and_woken_without_effect() {
    let waker = noop_waker();
    let clone = waker.clone();
    assert!(waker.will_wake(&clone));
    assert!(
        !waker.will_wake(&noop_waker()),
        "two calls give two wakers, which is why one is created per loop"
    );

    waker.wake_by_ref();
    clone.wake_by_ref();
    clone.wake();

    // A future polled with it makes progress on the caller's polls only: the
    // waker does not poll, and waking it is not observable from the future.
    let gate = Gate::new();
    let mut fut = std::pin::pin!(gate.future());
    let mut cx = std::task::Context::from_waker(&waker);
    assert_eq!(fut.as_mut().poll(&mut cx), Poll::Pending);
    gate.open();
    assert_eq!(fut.as_mut().poll(&mut cx), Poll::Ready(42));
}

#[test]
fn waking_a_noop_waker_from_another_thread_does_not_poll_the_future() {
    // A thread wakes a no-op waker every 5 ms while this thread waits on a
    // pending future under a 100 ms deadline. A waker that unparked this
    // thread would add a poll per wake; a no-op one adds none, so the wait
    // polls only before the park and at the deadline.
    let gate = Gate::new();
    let bound = Duration::from_millis(100);
    let stop = Arc::new(AtomicBool::new(false));
    let waker = noop_waker();
    let nagger = {
        let stop = Arc::clone(&stop);
        let waker = waker.clone();
        thread::spawn(move || {
            while !stop.load(Ordering::SeqCst) {
                waker.wake_by_ref();
                thread::sleep(Duration::from_millis(5));
            }
        })
    };

    let start = Instant::now();
    let out = block_on(gate.future(), Some(start + bound));
    stop.store(true, Ordering::SeqCst);
    nagger.join().unwrap();

    assert_eq!(out, None);
    let polls = gate.polls.load(Ordering::SeqCst);
    assert!(
        (2..=8).contains(&polls),
        "a no-op wake unparked the waiting thread: {polls} polls in {bound:?}"
    );
}
