//! Every port is usable as a trait object, and each port's supertrait is
//! reachable through that trait object. A later signature that breaks `dyn`
//! use, or a removed supertrait, fails this build. The forwarding of
//! `Wakeable` through `&P` and `&mut P` is tested here too, beside its
//! trait-object test; the other port traits' forwarding is in
//! `tests/port_forwarding.rs`.

#![forbid(unsafe_code)]

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::task::{Wake, Waker};

use ridl_rt::contract::{CatalogHash, CatalogRef, InterfaceNo, Ordinal};
use ridl_rt::error::{CallError, Contract};
use ridl_rt::port::{
    Attached, Caller, Changed, Claim, ClaimId, Clock, CoherentSignals, Correlation, EventSink,
    EventSource, FixedReader, Handler, Interest, RaiseError, RawOccurrence, RawSample, ReadError,
    ScannableSignals, SendError, ServeError, SettleError, SignalReader, SignalWriter,
    SubscribeError, Wakeable, Watermark, WriteError,
};
use ridl_rt::sample::{Envelope, Freshness, Provenance, Timestamp};

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

/// A port set that answers every call with a fixed value.
struct Stub;

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
    fn raise(&mut self, _: InterfaceNo, ord: Ordinal, _: &[u8]) -> Result<(), RaiseError> {
        if ord == ORD {
            Err(RaiseError::Busy)
        } else {
            Err(RaiseError::Contract(Contract::UnknownInteraction))
        }
    }
}

impl Caller for Stub {
    fn command(&mut self, _: InterfaceNo, _: Ordinal, _: &[u8]) -> Result<Correlation, SendError> {
        Ok(Correlation(1))
    }
    fn query(&mut self, _: InterfaceNo, _: Ordinal, _: &[u8]) -> Result<Correlation, SendError> {
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
            iface: InterfaceNo(1),
            ord: Ordinal(3),
            envelope: ENVELOPE,
            remaining: None,
            len: 0,
        }))
    }
    fn settle(&mut self, _: ClaimId, _: Result<&[u8], CallError>) -> Result<(), SettleError> {
        Err(SettleError::UnknownClaim)
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
        ords: &[Ordinal],
        _: &mut [u8],
        samples: &mut [RawSample],
    ) -> Result<usize, ReadError> {
        if samples.len() < ords.len() {
            return Err(ReadError::TooFewSamples { needed: ords.len() });
        }
        Ok(0)
    }
}

/// Wakes the waker at once for `Interest::Slot` and stores nothing for any
/// other key, so a test can tell which key reached it.
impl Wakeable for Stub {
    fn wake_on(&self, what: Interest, waker: &Waker) {
        if what == Interest::Slot {
            waker.wake_by_ref();
        }
    }
}

/// A waker that counts its wakes.
struct Count(AtomicUsize);

impl Wake for Count {
    fn wake(self: Arc<Self>) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

const IFACE: InterfaceNo = InterfaceNo(1);
const ORD: Ordinal = Ordinal(1);

#[test]
fn every_core_port_is_usable_as_a_trait_object() {
    let mut stub = Stub;
    let mut out = [0u8; 8];

    let clock: &dyn Clock = &stub;
    assert_eq!(clock.now(), Timestamp(42));

    let attached: &dyn Attached = &stub;
    assert_eq!(attached.catalog(), &CATALOG);

    let reader: &dyn SignalReader = &stub;
    assert_eq!(reader.read(IFACE, ORD, &mut out), Ok(RAW));
    assert_eq!(reader.catalog(), &CATALOG);

    let fixed: &dyn FixedReader = &stub;
    assert_eq!(
        fixed.read_fixed(IFACE, ORD, &mut out),
        Err(ReadError::Detached)
    );
    assert_eq!(fixed.catalog(), &CATALOG);

    let writer: &mut dyn SignalWriter = &mut stub;
    assert_eq!(writer.set(IFACE, ORD, &[1]), Err(WriteError::NotOwner));
    assert_eq!(
        writer.invalidate(IFACE, ORD),
        Err(WriteError::Contract(Contract::UnknownInteraction))
    );
    assert_eq!(
        writer.touch(IFACE, ORD),
        Err(WriteError::Contract(Contract::UnknownInteraction))
    );
    writer.commit();
    assert_eq!(writer.catalog(), &CATALOG);

    let source: &mut dyn EventSource = &mut stub;
    assert_eq!(
        source.subscribe(IFACE, &[ORD]),
        Err(SubscribeError::Contract(Contract::UnknownInteraction))
    );
    source.unsubscribe(IFACE, &[ORD]);
    assert_eq!(source.next(&mut out), Ok(None));
    assert_eq!(source.catalog(), &CATALOG);

    let sink: &mut dyn EventSink = &mut stub;
    assert_eq!(sink.raise(IFACE, ORD, &[1]), Err(RaiseError::Busy));
    assert_eq!(
        sink.raise(IFACE, Ordinal(2), &[1]),
        Err(RaiseError::Contract(Contract::UnknownInteraction))
    );
    assert_eq!(sink.catalog(), &CATALOG);

    let caller: &mut dyn Caller = &mut stub;
    assert_eq!(caller.command(IFACE, ORD, &[]), Ok(Correlation(1)));
    assert_eq!(caller.query(IFACE, ORD, &[]), Err(SendError::Busy));
    assert_eq!(caller.ack(Correlation(1)), Some(Ok(())));
    assert_eq!(
        caller.reply(Correlation(1), &mut out),
        Err(ReadError::Short { needed: 8 })
    );
    caller.forget(Correlation(1));
    assert_eq!(caller.catalog(), &CATALOG);

    let handler: &mut dyn Handler = &mut stub;
    assert_eq!(handler.catalog(), &CATALOG);
    assert_eq!(handler.serve(IFACE, &[ORD]), Err(ServeError::NotOwner));
    assert_eq!(
        handler.next_claim(&mut out).map(|c| c.map(|c| c.id)),
        Ok(Some(ClaimId(9)))
    );
    assert_eq!(
        handler.settle(ClaimId(9), Ok(&[])),
        Err(SettleError::UnknownClaim)
    );
}

#[test]
fn both_extensions_are_usable_as_trait_objects() {
    let stub = Stub;
    let mut out = [0u8; 8];

    let watermark = Watermark {
        iface: IFACE,
        generation: 7,
        seq: 0,
    };
    let changed = Changed {
        iface: IFACE,
        ord: ORD,
        seq: 1,
    };

    let scannable: &dyn ScannableSignals = &stub;
    assert_eq!(scannable.generation(IFACE), 7);
    assert_eq!(scannable.scan(&mut [watermark], &mut [changed]), 0);
    assert_eq!(scannable.read(IFACE, ORD, &mut out), Ok(RAW));

    let coherent: &dyn CoherentSignals = &stub;
    assert_eq!(
        coherent.read_coherent(IFACE, &[ORD], &mut out, &mut [RAW]),
        Ok(0)
    );
    assert_eq!(
        coherent.read_coherent(IFACE, &[ORD, ORD], &mut out, &mut [RAW]),
        Err(ReadError::TooFewSamples { needed: 2 })
    );
    assert_eq!(coherent.read(IFACE, ORD, &mut out), Ok(RAW));
}

#[test]
fn an_interest_is_copy_and_compares_by_its_key() {
    fn owns_nothing<T: Copy + Eq + core::fmt::Debug + 'static>() {}
    owns_nothing::<Interest>();
    assert_eq!(Interest::Event(IFACE), Interest::Event(InterfaceNo(1)));
    assert_ne!(Interest::Event(IFACE), Interest::Claim(IFACE));
    assert_ne!(Interest::Event(IFACE), Interest::Event(InterfaceNo(2)));
    assert_ne!(Interest::Claim(IFACE), Interest::Claim(InterfaceNo(2)));
    assert_ne!(
        Interest::Outcome(Correlation(1)),
        Interest::Outcome(Correlation(2))
    );
    assert_ne!(Interest::Slot, Interest::Outcome(Correlation(1)));
}

#[test]
fn a_raw_occurrence_is_built_from_its_four_fields() {
    let occurrence = RawOccurrence {
        iface: IFACE,
        ord: ORD,
        envelope: ENVELOPE,
        len: 3,
    };
    assert_eq!(occurrence.len, 3);
}

#[test]
fn every_port_error_is_copy_and_owns_nothing() {
    fn owns_nothing<T: Copy + Eq + core::fmt::Debug + 'static>() {}
    owns_nothing::<ReadError>();
    owns_nothing::<WriteError>();
    owns_nothing::<RaiseError>();
    owns_nothing::<SendError>();
    owns_nothing::<SubscribeError>();
    owns_nothing::<ServeError>();
    owns_nothing::<SettleError>();
}

#[test]
fn wakeable_is_usable_as_a_trait_object() {
    let count = Arc::new(Count(AtomicUsize::new(0)));
    let waker = Waker::from(Arc::clone(&count));
    let wakeable: &dyn Wakeable = &Stub;
    wakeable.wake_on(Interest::Slot, &waker);
    assert_eq!(count.0.load(Ordering::SeqCst), 1);
}

// `Wakeable` is reached through `&P` and `&mut P` the way every other port
// trait is (`tests/port_forwarding.rs`), and the key reaches the port
// unchanged.

/// Records the last key it was given, and wakes the waker it was given, so a
/// test can tell that both arrived unchanged.
struct Recorder(std::sync::Mutex<Option<Interest>>);

impl Wakeable for Recorder {
    fn wake_on(&self, what: Interest, waker: &Waker) {
        *self.0.lock().expect("not poisoned") = Some(what);
        waker.wake_by_ref();
    }
}

#[test]
fn wakeable_is_reached_through_a_borrow() {
    fn over<P: Wakeable>(port: P, what: Interest, waker: &Waker) {
        port.wake_on(what, waker);
    }
    fn take(recorder: &Recorder) -> Option<Interest> {
        recorder.0.lock().expect("not poisoned").take()
    }
    let count = Arc::new(Count(AtomicUsize::new(0)));
    let waker = Waker::from(Arc::clone(&count));
    let mut recorder = Recorder(std::sync::Mutex::new(None));
    let keys = [
        Interest::Outcome(Correlation(7)),
        Interest::Slot,
        Interest::Event(IFACE),
        Interest::Claim(InterfaceNo(2)),
    ];
    let mut expected = 0;
    for what in keys {
        over(&recorder, what, &waker);
        expected += 1;
        assert_eq!(take(&recorder), Some(what), "the key through `&P`");
        assert_eq!(
            count.0.load(Ordering::SeqCst),
            expected,
            "the waker through `&P`"
        );
        over(&mut recorder, what, &waker);
        expected += 1;
        assert_eq!(take(&recorder), Some(what), "the key through `&mut P`");
        assert_eq!(
            count.0.load(Ordering::SeqCst),
            expected,
            "the waker through `&mut P`"
        );
    }
}
