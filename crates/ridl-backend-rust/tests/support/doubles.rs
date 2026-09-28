//! Port doubles over `ridl-loopback`'s role handles, for the face tests of
//! the async client and of `serve` in `tests/interaction_face.rs` (the async
//! face design, note F-14).
//!
//! [`RecordingPorts`] is the port set a `Client` over an interface with a
//! signal, an event and a call needs — `Attached`, `Clock`, `SignalReader`,
//! `EventSource`, `Caller` and `Wakeable` — built from one `Loopback`'s role
//! handles rather than from the aggregate. Two things follow. The `Loopback`
//! stays free while a future holds the ports, so a test can `advance` its
//! clock, take a `handler()` for the provider side, and take a `caller()` to
//! fill the call table. And every `Caller` and `Wakeable` call the face makes
//! is recorded, in order, in a log the test reads while the future is alive,
//! which is what shows that a future forgets its call exactly once, and that
//! a poll registers its interest before it reads the port.
//!
//! [`FailingHandler`] wraps a `HandlerHandle` and fails where the loopback
//! never does: `next_claim` answers `ReadError::Detached` once a set number of
//! claims were presented, or `serve` refuses the members.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::task::Waker;

use ridl_loopback::{CallerHandle, HandlerHandle, Loopback, ReaderHandle, SourceHandle};
use ridl_rt::contract::{CatalogRef, InterfaceNo, Ordinal};
use ridl_rt::error::CallError;
use ridl_rt::port::{
    Attached, Caller, Claim, ClaimId, Clock, Correlation, EventSource, Handler, Interest,
    RawOccurrence, RawSample, ReadError, SendError, ServeError, SettleError, SignalReader,
    SubscribeError, Wakeable,
};
use ridl_rt::sample::Timestamp;

/// One `Caller` or `Wakeable` call the face made, with what the port
/// answered where the answer is what a test asserts on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
    Command(Result<Correlation, SendError>),
    Query(Result<Correlation, SendError>),
    Ack(Correlation),
    Reply(Correlation),
    Forget(Correlation),
    WakeOn(Interest),
}

/// The log [`RecordingPorts`] writes, shared with the test that reads it.
pub type Log = Rc<RefCell<Vec<Op>>>;

/// Takes every operation logged so far, so a test can assert on what one
/// poll or one drop did, and nothing before it.
pub fn take(log: &Log) -> Vec<Op> {
    std::mem::take(&mut *log.borrow_mut())
}

/// The correlations `forget` was called with, in order, over the whole log.
pub fn forgets(log: &Log) -> Vec<Correlation> {
    log.borrow()
        .iter()
        .filter_map(|op| match op {
            Op::Forget(c) => Some(*c),
            _ => None,
        })
        .collect()
}

/// Failures the loopback never produces, injected into the next port call of
/// their kind and then cleared. Shared with the test, which sets them while a
/// future holds the ports.
#[derive(Default)]
pub struct Faults {
    /// The next `command` or `query` answers this instead of sending.
    pub send: Cell<Option<SendError>>,
    /// The next `EventSource::next` answers this instead of reading.
    pub next: Cell<Option<ReadError>>,
}

/// The consumer-side ports of one `Loopback`, as its role handles, with a
/// log of every `Caller` and `Wakeable` call.
pub struct RecordingPorts {
    reader: ReaderHandle,
    source: SourceHandle,
    caller: CallerHandle,
    log: Log,
    faults: Rc<Faults>,
    /// The waker last registered under `Interest::Outcome`, kept for a test
    /// that wakes the waiting call from another thread the way a runtime
    /// that measures a call's bound would, which `ridl-loopback` does not.
    outcome_waker: Arc<Mutex<Option<Waker>>>,
}

impl RecordingPorts {
    pub fn new(rt: &Loopback) -> Self {
        RecordingPorts {
            reader: rt.reader(),
            source: rt.source(),
            caller: rt.caller(),
            log: Log::default(),
            faults: Rc::default(),
            outcome_waker: Arc::default(),
        }
    }

    /// A handle on the `Outcome` waker slot, taken before the ports move
    /// into a `Client`; `Send`, so another thread can wake the call.
    pub fn outcome_waker(&self) -> Arc<Mutex<Option<Waker>>> {
        Arc::clone(&self.outcome_waker)
    }

    /// A handle on the log, taken before the ports move into a `Client`.
    pub fn log(&self) -> Log {
        Rc::clone(&self.log)
    }

    /// A handle on the injected failures, taken before the ports move into a
    /// `Client`.
    pub fn faults(&self) -> Rc<Faults> {
        Rc::clone(&self.faults)
    }

    fn record(&self, op: Op) {
        self.log.borrow_mut().push(op);
    }
}

impl Attached for RecordingPorts {
    fn catalog(&self) -> &CatalogRef {
        self.reader.catalog()
    }
}

impl Clock for RecordingPorts {
    fn now(&self) -> Timestamp {
        self.caller.now()
    }
}

impl SignalReader for RecordingPorts {
    fn read(
        &self,
        iface: InterfaceNo,
        ord: Ordinal,
        out: &mut [u8],
    ) -> Result<RawSample, ReadError> {
        self.reader.read(iface, ord, out)
    }
}

impl EventSource for RecordingPorts {
    fn subscribe(&mut self, iface: InterfaceNo, ords: &[Ordinal]) -> Result<(), SubscribeError> {
        self.source.subscribe(iface, ords)
    }

    fn unsubscribe(&mut self, iface: InterfaceNo, ords: &[Ordinal]) {
        self.source.unsubscribe(iface, ords);
    }

    fn next(&mut self, out: &mut [u8]) -> Result<Option<RawOccurrence>, ReadError> {
        if let Some(failure) = self.faults.next.take() {
            return Err(failure);
        }
        self.source.next(out)
    }
}

impl Caller for RecordingPorts {
    fn command(
        &mut self,
        iface: InterfaceNo,
        ord: Ordinal,
        args: &[u8],
    ) -> Result<Correlation, SendError> {
        let sent = match self.faults.send.take() {
            Some(failure) => Err(failure),
            None => self.caller.command(iface, ord, args),
        };
        self.record(Op::Command(sent));
        sent
    }

    fn query(
        &mut self,
        iface: InterfaceNo,
        ord: Ordinal,
        args: &[u8],
    ) -> Result<Correlation, SendError> {
        let sent = match self.faults.send.take() {
            Some(failure) => Err(failure),
            None => self.caller.query(iface, ord, args),
        };
        self.record(Op::Query(sent));
        sent
    }

    fn ack(&mut self, c: Correlation) -> Option<Result<(), CallError>> {
        self.record(Op::Ack(c));
        self.caller.ack(c)
    }

    fn reply(
        &mut self,
        c: Correlation,
        out: &mut [u8],
    ) -> Result<Option<Result<usize, CallError>>, ReadError> {
        self.record(Op::Reply(c));
        self.caller.reply(c, out)
    }

    fn forget(&mut self, c: Correlation) {
        self.record(Op::Forget(c));
        self.caller.forget(c);
    }
}

/// Routes each key as the aggregate does. There is no handler role here, so
/// `Claim` goes to the caller handle, which wakes a kind it does not observe
/// at once.
impl Wakeable for RecordingPorts {
    fn wake_on(&self, what: Interest, waker: &Waker) {
        self.record(Op::WakeOn(what));
        match what {
            Interest::Event(_) => self.source.wake_on(what, waker),
            Interest::Outcome(_) => {
                *self.outcome_waker.lock().expect("no poisoned waker slot") = Some(waker.clone());
                self.caller.wake_on(what, waker);
            }
            Interest::Slot | Interest::Claim(_) => {
                self.caller.wake_on(what, waker);
            }
        }
    }
}

/// A handler port that fails where `ridl-loopback` never does.
pub struct FailingHandler {
    inner: HandlerHandle,
    /// Claims presented so far, counted from `next_claim`'s `Ok(Some)`.
    presented: usize,
    /// `next_claim` answers `ReadError::Detached` once this many claims were
    /// presented. `None` never fails.
    fail_after: Option<usize>,
    /// `serve` answers this refusal instead of registering the members.
    refusal: Option<ServeError>,
}

impl FailingHandler {
    /// A handler whose `next_claim` fails with `ReadError::Detached` once
    /// `presented` claims were presented, each of which it served normally.
    pub fn failing_after(inner: HandlerHandle, presented: usize) -> Self {
        FailingHandler {
            inner,
            presented: 0,
            fail_after: Some(presented),
            refusal: None,
        }
    }

    /// A handler whose `serve` refuses with `refusal`.
    pub fn refusing(inner: HandlerHandle, refusal: ServeError) -> Self {
        FailingHandler {
            inner,
            presented: 0,
            fail_after: None,
            refusal: Some(refusal),
        }
    }
}

impl Attached for FailingHandler {
    fn catalog(&self) -> &CatalogRef {
        self.inner.catalog()
    }
}

impl Handler for FailingHandler {
    fn serve(&mut self, iface: InterfaceNo, ords: &[Ordinal]) -> Result<(), ServeError> {
        match self.refusal {
            Some(refusal) => Err(refusal),
            None => self.inner.serve(iface, ords),
        }
    }

    fn next_claim(&mut self, out: &mut [u8]) -> Result<Option<Claim>, ReadError> {
        if self.fail_after.is_some_and(|limit| self.presented >= limit) {
            return Err(ReadError::Detached);
        }
        let claim = self.inner.next_claim(out);
        if let Ok(Some(_)) = claim {
            self.presented += 1;
        }
        claim
    }

    fn settle(
        &mut self,
        claim: ClaimId,
        outcome: Result<&[u8], CallError>,
    ) -> Result<(), SettleError> {
        self.inner.settle(claim, outcome)
    }
}

impl Wakeable for FailingHandler {
    fn wake_on(&self, what: Interest, waker: &Waker) {
        self.inner.wake_on(what, waker);
    }
}
