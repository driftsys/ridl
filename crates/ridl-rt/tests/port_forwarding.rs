//! Every port trait is implemented for a mutable borrow of a port, and the
//! eight whose methods all take `&self`, or have no methods, also for a shared
//! borrow. What these
//! impls buy is one thing: a value generic over a port trait can be given a
//! reference to a port rather than the port itself. A wrapper or a test
//! double is accepted by such a bound with or without them, because it
//! implements the port traits itself.
//!
//! Each test below hands a borrow of one stub to a function generic over one
//! port trait and calls the trait's methods through it, so a removed
//! forwarding impl fails this build. `Wakeable`'s forwarding is tested in
//! `tests/ports.rs`, beside its trait-object test, where a recorder checks
//! that the key and the waker both arrive unchanged. `Encoded`'s forwarding is
//! tested in `tests/encoded.rs`, because it has no methods to call.

#![forbid(unsafe_code)]

use ridl_rt::contract::StreamNo;
use ridl_rt::contract::{CatalogHash, CatalogRef, InterfaceNo, Ordinal};
use ridl_rt::error::{CallError, Contract, Transport};
use ridl_rt::port::{
    Attached, Caller, Changed, Claim, ClaimId, Clock, CoherentSignals, Correlation, EventSink,
    EventSource, FixedReader, Handler, Pull, PushError, RaiseError, RawOccurrence, RawSample,
    ReadError, ScannableSignals, SendError, ServeError, SettleError, SignalReader, SignalWriter,
    StreamCaller, StreamHandler, SubscribeError, Watermark, WriteError, REPLY_STREAM,
};
use ridl_rt::sample::{Envelope, Freshness, Provenance, Timestamp};
use ridl_rt::trace::TraceContext;

const CATALOG: CatalogRef = CatalogRef {
    name: "vehicle",
    hash: CatalogHash([0; 32]),
};

const ENVELOPE: Envelope = Envelope {
    stamp: Timestamp(0),
    seq: 0,
};

const RAW: RawSample = RawSample {
    provenance: Provenance::Init,
    freshness: Freshness::Unbounded,
    envelope: ENVELOPE,
    len: 0,
};

const IFACE: InterfaceNo = InterfaceNo(1);
const ORD: Ordinal = Ordinal(1);

const TRACE: TraceContext = TraceContext {
    trace_id: [1; 16],
    span_id: [2; 8],
    flags: 1,
};

/// A port set that answers every call with a fixed value, and records the
/// trace context its send methods receive.
#[derive(Default)]
struct Stub {
    raised: Option<TraceContext>,
    commanded: Option<TraceContext>,
    queried: Option<TraceContext>,
    /// The stream number of every stream call, in the order received.
    streams: Vec<StreamNo>,
}

impl Attached for Stub {
    fn catalog(&self) -> &CatalogRef {
        &CATALOG
    }
}

impl Clock for Stub {
    fn now(&self) -> Timestamp {
        Timestamp(42)
    }
}

impl SignalReader for Stub {
    fn read(&self, _: InterfaceNo, _: Ordinal, _: &mut [u8]) -> Result<RawSample, ReadError> {
        Ok(RAW)
    }
}

impl SignalWriter for Stub {
    fn set(&mut self, _: InterfaceNo, _: Ordinal, _: &[u8]) -> Result<(), WriteError> {
        Err(WriteError::NotOwner)
    }
    fn invalidate(&mut self, _: InterfaceNo, _: Ordinal) -> Result<(), WriteError> {
        Err(WriteError::Contract(Contract::UnknownInteraction))
    }
    fn touch(&mut self, _: InterfaceNo, _: Ordinal) -> Result<(), WriteError> {
        Err(WriteError::Contract(Contract::UnknownInteraction))
    }
    fn commit(&mut self) {}
}

impl EventSource for Stub {
    fn subscribe(&mut self, _: InterfaceNo, _: &[Ordinal]) -> Result<(), SubscribeError> {
        Err(SubscribeError::Contract(Contract::UnknownInteraction))
    }
    fn unsubscribe(&mut self, _: InterfaceNo, _: &[Ordinal]) {}
    fn next(&mut self, _: &mut [u8]) -> Result<Option<RawOccurrence>, ReadError> {
        Ok(None)
    }
}

impl EventSink for Stub {
    fn raise(
        &mut self,
        _: InterfaceNo,
        _: Ordinal,
        _: &[u8],
        trace: Option<TraceContext>,
    ) -> Result<(), RaiseError> {
        self.raised = trace;
        Err(RaiseError::Busy)
    }
}

impl Caller for Stub {
    fn command(
        &mut self,
        _: InterfaceNo,
        _: Ordinal,
        _: &[u8],
        trace: Option<TraceContext>,
    ) -> Result<Correlation, SendError> {
        self.commanded = trace;
        Ok(Correlation(1))
    }
    fn query(
        &mut self,
        _: InterfaceNo,
        _: Ordinal,
        _: &[u8],
        trace: Option<TraceContext>,
    ) -> Result<Correlation, SendError> {
        self.queried = trace;
        Err(SendError::Busy)
    }
    fn ack(&mut self, _: Correlation) -> Option<Result<(), CallError>> {
        Some(Ok(()))
    }
    fn reply(
        &mut self,
        _: Correlation,
        _: &mut [u8],
    ) -> Result<Option<Result<usize, CallError>>, ReadError> {
        Err(ReadError::Short { needed: 8 })
    }
    fn forget(&mut self, _: Correlation) {}
}

impl Handler for Stub {
    fn serve(&mut self, _: InterfaceNo, _: &[Ordinal]) -> Result<(), ServeError> {
        Err(ServeError::NotOwner)
    }
    fn next_claim(&mut self, _: &mut [u8]) -> Result<Option<Claim>, ReadError> {
        Ok(Some(Claim {
            id: ClaimId(9),
            iface: IFACE,
            ord: ORD,
            envelope: ENVELOPE,
            trace: None,
            remaining: None,
            len: 0,
        }))
    }
    fn settle(&mut self, _: ClaimId, _: Result<&[u8], CallError>) -> Result<(), SettleError> {
        Err(SettleError::UnknownClaim)
    }
}

const INTERRUPTED: CallError = CallError::Transport(Transport::Interrupted);

/// Each stream method answers from its arguments, so a forwarding impl that
/// passed another argument, or called another method, fails an assertion.
impl StreamCaller for Stub {
    fn pull(&mut self, c: Correlation, s: StreamNo, out: &mut [u8]) -> Result<Pull, ReadError> {
        self.streams.push(s);
        assert_eq!(c, Correlation(1));
        Ok(Pull::Chunk(out.len()))
    }
    fn push(&mut self, c: Correlation, s: StreamNo, chunk: &[u8]) -> Result<usize, PushError> {
        self.streams.push(s);
        assert_eq!(c, Correlation(1));
        Ok(chunk.len())
    }
    fn end(&mut self, c: Correlation, s: StreamNo) -> Result<(), PushError> {
        self.streams.push(s);
        assert_eq!(c, Correlation(1));
        Err(PushError::Ended)
    }
    fn abort(&mut self, c: Correlation, s: StreamNo, e: CallError) -> Result<(), PushError> {
        self.streams.push(s);
        assert_eq!(c, Correlation(1));
        Err(PushError::Failed(e))
    }
}

impl StreamHandler for Stub {
    fn pull_claim(
        &mut self,
        claim: ClaimId,
        s: StreamNo,
        out: &mut [u8],
    ) -> Result<Pull, ReadError> {
        self.streams.push(s);
        assert_eq!(claim, ClaimId(9));
        Err(ReadError::Short {
            needed: out.len() + 1,
        })
    }
    fn push_claim(
        &mut self,
        claim: ClaimId,
        s: StreamNo,
        chunk: &[u8],
    ) -> Result<usize, PushError> {
        self.streams.push(s);
        assert_eq!(claim, ClaimId(9));
        Err(PushError::TooLarge { cap: chunk.len() })
    }
    fn end_claim(&mut self, claim: ClaimId, s: StreamNo) -> Result<(), PushError> {
        self.streams.push(s);
        assert_eq!(claim, ClaimId(9));
        Err(PushError::Detached)
    }
    fn abort_claim(&mut self, claim: ClaimId, s: StreamNo, e: CallError) -> Result<(), PushError> {
        self.streams.push(s);
        assert_eq!(claim, ClaimId(9));
        Err(PushError::Failed(e))
    }
}

impl FixedReader for Stub {
    fn read_fixed(&self, _: InterfaceNo, _: Ordinal, _: &mut [u8]) -> Result<usize, ReadError> {
        Err(ReadError::Detached)
    }
}

impl ScannableSignals for Stub {
    fn generation(&self, _: InterfaceNo) -> u64 {
        7
    }
    fn scan(&self, _: &mut [Watermark], _: &mut [Changed]) -> usize {
        0
    }
}

impl CoherentSignals for Stub {
    fn read_coherent(
        &self,
        _: InterfaceNo,
        _: &[Ordinal],
        _: &mut [u8],
        _: &mut [RawSample],
    ) -> Result<usize, ReadError> {
        Ok(0)
    }
}

#[test]
fn attached_is_reached_through_a_borrow() {
    fn over<P: Attached>(port: P) {
        assert_eq!(port.catalog(), &CATALOG);
    }
    let mut stub = Stub::default();
    over(&stub);
    over(&mut stub);
}

#[test]
fn clock_is_reached_through_a_borrow() {
    fn over<P: Clock>(port: P) -> Timestamp {
        port.now()
    }
    let mut stub = Stub::default();
    assert_eq!(over(&stub), Timestamp(42));
    assert_eq!(over(&mut stub), Timestamp(42));
}

#[test]
fn signal_reader_is_reached_through_a_borrow() {
    fn over<P: SignalReader>(port: P) -> Result<RawSample, ReadError> {
        assert_eq!(port.catalog(), &CATALOG);
        port.read(IFACE, ORD, &mut [0u8; 8])
    }
    let mut stub = Stub::default();
    assert_eq!(over(&stub), Ok(RAW));
    assert_eq!(over(&mut stub), Ok(RAW));
}

#[test]
fn signal_writer_is_reached_through_a_borrow() {
    fn over<P: SignalWriter>(mut port: P) {
        assert_eq!(port.catalog(), &CATALOG);
        assert_eq!(port.set(IFACE, ORD, &[1]), Err(WriteError::NotOwner));
        assert_eq!(
            port.invalidate(IFACE, ORD),
            Err(WriteError::Contract(Contract::UnknownInteraction))
        );
        assert_eq!(
            port.touch(IFACE, ORD),
            Err(WriteError::Contract(Contract::UnknownInteraction))
        );
        port.commit();
    }
    let mut stub = Stub::default();
    over(&mut stub);
}

#[test]
fn event_source_is_reached_through_a_borrow() {
    fn over<P: EventSource>(mut port: P) {
        assert_eq!(port.catalog(), &CATALOG);
        assert_eq!(
            port.subscribe(IFACE, &[ORD]),
            Err(SubscribeError::Contract(Contract::UnknownInteraction))
        );
        port.unsubscribe(IFACE, &[ORD]);
        assert_eq!(port.next(&mut [0u8; 8]), Ok(None));
    }
    let mut stub = Stub::default();
    over(&mut stub);
}

#[test]
fn event_sink_is_reached_through_a_borrow() {
    fn over<P: EventSink>(mut port: P) {
        assert_eq!(port.catalog(), &CATALOG);
        assert_eq!(
            port.raise(IFACE, ORD, &[1], Some(TRACE)),
            Err(RaiseError::Busy)
        );
    }
    let mut stub = Stub::default();
    over(&mut stub);
    assert_eq!(stub.raised, Some(TRACE));
}

#[test]
fn caller_is_reached_through_a_borrow() {
    fn over<P: Caller>(mut port: P) {
        assert_eq!(port.catalog(), &CATALOG);
        assert_eq!(
            port.command(IFACE, ORD, &[], Some(TRACE)),
            Ok(Correlation(1))
        );
        assert_eq!(
            port.query(IFACE, ORD, &[], Some(TRACE)),
            Err(SendError::Busy)
        );
        assert_eq!(port.ack(Correlation(1)), Some(Ok(())));
        assert_eq!(
            port.reply(Correlation(1), &mut [0u8; 8]),
            Err(ReadError::Short { needed: 8 })
        );
        port.forget(Correlation(1));
    }
    let mut stub = Stub::default();
    over(&mut stub);
    assert_eq!(stub.commanded, Some(TRACE));
    assert_eq!(stub.queried, Some(TRACE));
}

#[test]
fn handler_is_reached_through_a_borrow() {
    fn over<P: Handler>(mut port: P) {
        assert_eq!(port.catalog(), &CATALOG);
        assert_eq!(port.serve(IFACE, &[ORD]), Err(ServeError::NotOwner));
        assert_eq!(
            port.next_claim(&mut [0u8; 8]).map(|c| c.map(|c| c.id)),
            Ok(Some(ClaimId(9)))
        );
        assert_eq!(
            port.settle(ClaimId(9), Ok(&[])),
            Err(SettleError::UnknownClaim)
        );
    }
    let mut stub = Stub::default();
    over(&mut stub);
}

#[test]
fn fixed_reader_is_reached_through_a_borrow() {
    fn over<P: FixedReader>(port: P) -> Result<usize, ReadError> {
        assert_eq!(port.catalog(), &CATALOG);
        port.read_fixed(IFACE, ORD, &mut [0u8; 8])
    }
    let mut stub = Stub::default();
    assert_eq!(over(&stub), Err(ReadError::Detached));
    assert_eq!(over(&mut stub), Err(ReadError::Detached));
}

#[test]
fn scannable_signals_is_reached_through_a_borrow() {
    fn over<P: ScannableSignals>(port: P) -> u64 {
        assert_eq!(port.read(IFACE, ORD, &mut [0u8; 8]), Ok(RAW));
        let mut marks = [Watermark {
            iface: IFACE,
            generation: 7,
        }];
        let mut out = [Changed {
            iface: IFACE,
            ord: ORD,
            seq: 1,
        }];
        assert_eq!(port.scan(&mut marks, &mut out), 0);
        port.generation(IFACE)
    }
    let mut stub = Stub::default();
    assert_eq!(over(&stub), 7);
    assert_eq!(over(&mut stub), 7);
}

#[test]
fn coherent_signals_is_reached_through_a_borrow() {
    fn over<P: CoherentSignals>(port: P) -> Result<usize, ReadError> {
        assert_eq!(port.read(IFACE, ORD, &mut [0u8; 8]), Ok(RAW));
        port.read_coherent(IFACE, &[ORD], &mut [0u8; 8], &mut [RAW])
    }
    let mut stub = Stub::default();
    assert_eq!(over(&stub), Ok(0));
    assert_eq!(over(&mut stub), Ok(0));
}

#[test]
fn stream_caller_and_stream_handler_forward_through_mut_ref() {
    fn caller<P: StreamCaller>(mut port: P) {
        assert_eq!(port.ack(Correlation(1)), Some(Ok(())), "a `Caller` too");
        assert_eq!(
            port.pull(Correlation(1), REPLY_STREAM, &mut [0u8; 5]),
            Ok(Pull::Chunk(5))
        );
        assert_eq!(port.push(Correlation(1), StreamNo(1), &[0u8; 3]), Ok(3));
        assert_eq!(port.end(Correlation(1), StreamNo(2)), Err(PushError::Ended));
        assert_eq!(
            port.abort(Correlation(1), StreamNo(3), INTERRUPTED),
            Err(PushError::Failed(INTERRUPTED))
        );
    }
    fn handler<P: StreamHandler>(mut port: P) {
        assert_eq!(
            port.settle(ClaimId(9), Ok(&[])),
            Err(SettleError::UnknownClaim),
            "a `Handler` too"
        );
        assert_eq!(
            port.pull_claim(ClaimId(9), StreamNo(4), &mut [0u8; 2]),
            Err(ReadError::Short { needed: 3 })
        );
        assert_eq!(
            port.push_claim(ClaimId(9), StreamNo(5), &[0u8; 7]),
            Err(PushError::TooLarge { cap: 7 })
        );
        assert_eq!(
            port.end_claim(ClaimId(9), REPLY_STREAM),
            Err(PushError::Detached)
        );
        assert_eq!(
            port.abort_claim(ClaimId(9), StreamNo(6), INTERRUPTED),
            Err(PushError::Failed(INTERRUPTED))
        );
    }
    let mut stub = Stub::default();
    caller(&mut stub);
    handler(&mut stub);
    assert_eq!(
        stub.streams,
        [0, 1, 2, 3, 4, 5, 0, 6].map(StreamNo),
        "every stream number arrives unchanged"
    );
}
