//! `block_on` and `noop_waker` under the `std` feature (story E11.17).
//!
//! The whole file is gated on the feature, because `just test` builds the
//! workspace with default features and the module does not exist there.
//!
//! Every timing assertion here is a lower bound — "the wait lasted at least
//! the delay" — or a bound so wide that a loaded machine cannot cross it, so
//! the tests do not depend on scheduling latency. A poll-count ceiling is wide
//! in the same way: a parked wait polls once before the wait and once per wake
//! or timeout, so a ceiling of eight catches a wait that spins or parks for a
//! fixed short time, and tolerates the few extra wakes a platform may deliver.
//! The spurious-wake test asserts that at least one such wake polled the
//! future, not how many did, because the thread that sends them is not
//! scheduled at a fixed cadence.
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
    let opener = {
        let gate = Arc::clone(&gate);
        thread::spawn(move || {
            thread::sleep(delay);
            gate.open();
        })
    };

    let start = Instant::now();
    let out = block_on(gate.future(), None);
    let elapsed = start.elapsed();
    opener.join().unwrap();

    assert_eq!(out, Some(42));
    assert!(
        elapsed >= delay,
        "returned after {elapsed:?}, before the opener ran"
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
    let opener = {
        let gate = Arc::clone(&gate);
        thread::spawn(move || {
            thread::sleep(delay);
            gate.open();
        })
    };

    let start = Instant::now();
    let out = block_on(gate.future(), Some(start + far));
    let elapsed = start.elapsed();
    opener.join().unwrap();

    assert_eq!(out, Some(42));
    assert!(elapsed >= delay);
    assert!(elapsed < far, "the wake, not the deadline, ended the wait");
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
        "a park is bounded by the time left, not by a fixed duration: {elapsed:?}"
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

    waker.wake_by_ref();
    clone.wake_by_ref();
    clone.wake();

    // A future polled with it makes progress on the caller's polls only: the
    // waker does not poll, and waking it is not observable from the future.
    // (Whether the wake left an unpark token on this thread is not asserted:
    // `park_timeout` may return early on its own, so the check would be
    // unreliable.)
    let gate = Gate::new();
    let mut fut = std::pin::pin!(gate.future());
    let mut cx = std::task::Context::from_waker(&waker);
    assert_eq!(fut.as_mut().poll(&mut cx), Poll::Pending);
    gate.open();
    assert_eq!(fut.as_mut().poll(&mut cx), Poll::Ready(42));
}
