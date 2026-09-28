//! Waiting on a future from a thread, a waker that does nothing, and a waker
//! that sets a flag.
//!
//! This module is gated by the `std` cargo feature, the one feature of this
//! crate that links the standard library. It is for three callers:
//!
//! - **A blocking client built over an async one.** A client that returns a
//!   future is the one source of truth for a call's behaviour; a blocking
//!   variant of the same call is [`block_on`] over that future, so the two
//!   cannot diverge. The `deadline` bounds the whole wait.
//! - **A frame loop that polls a future once per frame.** Such a loop needs a
//!   `Context`, and a `Context` needs a `Waker`, but the loop polls on its own
//!   schedule and has no use for a wake. [`noop_waker`] is that waker: created
//!   once when the loop starts and cloned into each `Context`.
//! - **A frame loop that polls again when the future asks for it.** A future
//!   that does a bounded amount of work per poll and wakes itself when work is
//!   left — the generated `Serve` takes at most 32 claims per poll — makes
//!   progress under [`noop_waker`] only at the loop's next frame, because that
//!   wake is discarded. [`flag_waker`] records the wake in a flag the loop
//!   reads: poll, then poll again while the flag was set, up to the loop's own
//!   limit of polls per frame.
//!
//! All three are written without `unsafe`, which this crate forbids, over
//! `std::task::Wake` on an `Arc`. That is why the module is under `std`:
//! `Waker::noop()` needs Rust 1.85, above this crate's minimum, and building a
//! `Waker` from a raw vtable needs `unsafe`. The module takes no dependency:
//! the wait is `std::thread::park_timeout`, whose unpark token cannot be lost
//! between a poll and the park that follows it.
//!
//! The feature compiles for `wasm32-unknown-unknown`, because `just wasm-check`
//! requires every feature to, and [`block_on`] is not usable on that target:
//! `Instant::now()` panics there, and a park does not block the thread. A
//! frame loop on wasm polls with [`noop_waker`] or [`flag_waker`] and never
//! calls [`block_on`]. A `no_std` frame loop, where this module does not
//! exist, writes the same small waker over its own atomic flag.
//!
//! Nothing here names a port, an interface or a generated type; the module
//! knows only `core::future::Future`.

extern crate std;

use core::future::Future;
use core::pin::pin;
use core::sync::atomic::{AtomicBool, Ordering};
use core::task::{Context, Poll, Waker};
use std::sync::Arc;
use std::task::Wake;
use std::thread::{self, Thread};
use std::time::Instant;

/// Wake by unparking the thread that is blocked in [`block_on`].
struct Unpark(Thread);

impl Wake for Unpark {
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.0.unpark();
    }
}

/// A waker whose wake does nothing.
struct Noop;

impl Wake for Noop {
    fn wake(self: Arc<Self>) {}

    fn wake_by_ref(self: &Arc<Self>) {}
}

/// Run `fut` to completion on the current thread, parking the thread between
/// polls, and give up at `deadline`.
///
/// The future is polled once immediately, then once more after every wake of
/// the waker it was polled with. Between polls the thread is parked, so the
/// wait costs no CPU. The waker unparks the thread through `std::task::Wake`
/// on an `Arc`; an unpark that arrives while the thread is not parked is kept
/// as a token that ends the next park at once, so a wake between a poll and
/// the park that follows it is not lost. A wake that is not from the waker —
/// `Thread::unpark` called by someone else, or a park ending on its own — is
/// harmless: it causes one extra poll, and the wait continues.
///
/// # The deadline
///
/// - `None`: wait until the future is ready, however long that takes.
/// - `Some(deadline)`: return `Some(output)` from the first poll that returns
///   `Ready`, whenever that poll runs — also after a park that ended late —
///   and `None` once a poll returns `Pending` when
///   `Instant::now() >= deadline`. The future is polled at least once even
///   when the deadline has already passed at entry, so a future that is
///   already ready returns `Some` whatever the deadline. On `None`, the future
///   is dropped without another poll.
///
/// The deadline is checked after each `Pending` poll, so `None` can be
/// returned only after a poll, and each park is given at most the time left
/// until the deadline.
///
/// ```
/// use std::time::{Duration, Instant};
///
/// let out = ridl_rt::task::block_on(std::future::ready(7), None);
/// assert_eq!(out, Some(7));
///
/// let pending = std::future::pending::<u8>();
/// let out = ridl_rt::task::block_on(pending, Some(Instant::now() + Duration::from_millis(10)));
/// assert_eq!(out, None);
/// ```
pub fn block_on<F: Future>(fut: F, deadline: Option<Instant>) -> Option<F::Output> {
    let mut fut = pin!(fut);
    let waker = Waker::from(Arc::new(Unpark(thread::current())));
    let mut cx = Context::from_waker(&waker);
    loop {
        if let Poll::Ready(out) = fut.as_mut().poll(&mut cx) {
            return Some(out);
        }
        match deadline {
            None => thread::park(),
            Some(deadline) => {
                let now = Instant::now();
                if now >= deadline {
                    return None;
                }
                thread::park_timeout(deadline - now);
            }
        }
    }
}

/// A waker whose wake does nothing, for a loop that polls on its own schedule.
///
/// Waking it, by value or by reference, has no effect and never panics; a
/// future polled with it makes progress only when the caller polls it again.
/// The waker is built as `Waker::from(Arc<Noop>)`, so each call allocates one
/// `Arc`: create it once per loop and clone it into each `Context`, rather
/// than calling this function per poll. Two clones of one waker report
/// `will_wake` as `true`; two wakers from two calls do not.
///
/// ```
/// use std::future::Future;
/// use std::task::Context;
///
/// let waker = ridl_rt::task::noop_waker();
/// let mut cx = Context::from_waker(&waker);
/// let mut fut = std::pin::pin!(std::future::ready(1));
/// assert!(fut.as_mut().poll(&mut cx).is_ready());
/// ```
pub fn noop_waker() -> Waker {
    Waker::from(Arc::new(Noop))
}

/// A waker whose wake sets a flag, and the handle that reads and clears it.
#[derive(Debug)]
struct Flag(AtomicBool);

impl Wake for Flag {
    fn wake(self: Arc<Self>) {
        self.0.store(true, Ordering::Release);
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.0.store(true, Ordering::Release);
    }
}

/// The read side of a [`flag_waker`]: whether the waker was woken since the
/// last [`take`](WakeFlag::take).
#[derive(Debug)]
pub struct WakeFlag(Arc<Flag>);

impl WakeFlag {
    /// Return `true` if the waker was woken, by value or by reference, from
    /// any thread, since the flag was created or last taken, and clear the
    /// flag.
    pub fn take(&self) -> bool {
        self.0 .0.swap(false, Ordering::Acquire)
    }
}

/// A waker whose wake sets a flag, for a frame loop that polls again in the
/// same frame when the future asks for it.
///
/// The flag starts clear. A wake of the waker or of any clone of it, by value
/// or by reference, from any thread, sets it; [`WakeFlag::take`] reads and
/// clears it. Nothing is polled by the wake itself. The waker is built as
/// `Waker::from(Arc<Flag>)`, so each call allocates one `Arc`, which the
/// waker and the flag share: create the pair once per loop.
///
/// The frame-loop pattern: poll, then poll again while the flag was set, up to
/// the loop's own limit of polls per frame, so that a future that wakes itself
/// on every poll cannot hold the frame. A wake that arrived between two frames
/// leaves the flag set, which costs at most one extra poll in the next frame.
///
/// ```
/// use std::future::Future;
/// use std::task::Context;
///
/// const POLLS_PER_FRAME: usize = 8;
///
/// let (waker, woken) = ridl_rt::task::flag_waker();
/// let mut cx = Context::from_waker(&waker);
/// let mut fut = std::pin::pin!(std::future::ready(1));
///
/// // One frame.
/// for _ in 0..POLLS_PER_FRAME {
///     if fut.as_mut().poll(&mut cx).is_ready() || !woken.take() {
///         break;
///     }
/// }
///
/// waker.wake_by_ref();
/// assert!(woken.take());
/// assert!(!woken.take());
/// ```
pub fn flag_waker() -> (Waker, WakeFlag) {
    let flag = Arc::new(Flag(AtomicBool::new(false)));
    (Waker::from(Arc::clone(&flag)), WakeFlag(flag))
}
