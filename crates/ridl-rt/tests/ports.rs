//! Every port is usable as a trait object, and each port's supertrait is
//! reachable through that trait object. A later signature that breaks `dyn`
//! use, or a removed supertrait, fails this build.

#![forbid(unsafe_code)]

use ridl_rt::contract::{CatalogHash, CatalogRef, InterfaceNo, Ordinal};
use ridl_rt::error::{CallError, Contract};
use ridl_rt::port::{
    Attached, Caller, Changed, Claim, ClaimId, Clock, CoherentSignals, Correlation, EventSink,
    EventSource, FixedReader, Handler, RaiseError, RawOccurrence, RawSample, ReadError,
    ScannableSignals, SendError, ServeError, SettleError, SignalReader, SignalWriter,
    SubscribeError, Watermark, WriteError,
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
    fn invalidate(&mut self, _: InterfaceNo, _: Ordinal) {}
    fn touch(&mut self, _: InterfaceNo, _: Ordinal) {}
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
    fn raise(&mut self, _: InterfaceNo, _: Ordinal, _: &[u8]) -> Result<(), RaiseError> {
        Err(RaiseError::Busy)
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
    fn settle(&mut self, _: ClaimId, _: Result<&[u8], Contract>) -> Result<(), SettleError> {
        Err(SettleError::UnknownClaim)
    }
}

impl FixedReader for Stub {
    fn read(&self, _: InterfaceNo, _: Ordinal, _: &mut [u8]) -> Result<usize, ReadError> {
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
    assert_eq!(fixed.read(IFACE, ORD, &mut out), Err(ReadError::Detached));
    assert_eq!(fixed.catalog(), &CATALOG);

    let writer: &mut dyn SignalWriter = &mut stub;
    assert_eq!(writer.set(IFACE, ORD, &[1]), Err(WriteError::NotOwner));
    writer.invalidate(IFACE, ORD);
    writer.touch(IFACE, ORD);
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
    assert_eq!(coherent.read(IFACE, ORD, &mut out), Ok(RAW));
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
