//! Restricted port delegation for a descriptor attachment.
use super::Reservation;
use crate::Loopback;
use ridl_rt::contract::{CatalogRef, InterfaceNo, Ordinal};
use ridl_rt::encoding::FlatBuffers;
use ridl_rt::error::CallError;
use ridl_rt::port::*;
use ridl_rt::sample::Timestamp;
use ridl_rt::trace::TraceContext;
use std::task::Waker;

/// One reserved identity and its ports for one granted catalog.
/// No raw handles can be extracted or duplicated. A listener can read,
/// subscribe and call, but cannot publish, raise, serve or take a claim.
pub struct Attachment {
    pub(super) ports: Loopback,
    pub(super) _reservation: Reservation,
    pub(super) listener: bool,
}

impl Attached for Attachment {
    fn catalog(&self) -> &CatalogRef {
        self.ports.catalog()
    }
}

impl Encoded for Attachment {
    type Encoding = FlatBuffers;
}

impl Clock for Attachment {
    fn now(&self) -> Timestamp {
        self.ports.now()
    }
}

impl SignalReader for Attachment {
    fn read(
        &self,
        iface: InterfaceNo,
        ord: Ordinal,
        out: &mut [u8],
    ) -> Result<RawSample, ReadError> {
        self.ports.read(iface, ord, out)
    }
}

impl FixedReader for Attachment {
    fn read_fixed(
        &self,
        iface: InterfaceNo,
        ord: Ordinal,
        out: &mut [u8],
    ) -> Result<usize, ReadError> {
        self.ports.read_fixed(iface, ord, out)
    }
}

impl ScannableSignals for Attachment {
    fn generation(&self, iface: InterfaceNo) -> u64 {
        self.ports.generation(iface)
    }

    fn scan(&self, marks: &mut [Watermark], out: &mut [Changed]) -> usize {
        self.ports.scan(marks, out)
    }
}

impl CoherentSignals for Attachment {
    fn read_coherent(
        &self,
        iface: InterfaceNo,
        ords: &[Ordinal],
        out: &mut [u8],
        samples: &mut [RawSample],
    ) -> Result<usize, ReadError> {
        self.ports.read_coherent(iface, ords, out, samples)
    }
}

impl SignalWriter for Attachment {
    fn set(&mut self, iface: InterfaceNo, ord: Ordinal, bytes: &[u8]) -> Result<(), WriteError> {
        if self.listener {
            return Err(WriteError::NotOwner);
        }
        self.ports.set(iface, ord, bytes)
    }

    fn invalidate(&mut self, iface: InterfaceNo, ord: Ordinal) -> Result<(), WriteError> {
        if self.listener {
            return Err(WriteError::NotOwner);
        }
        self.ports.invalidate(iface, ord)
    }

    fn touch(&mut self, iface: InterfaceNo, ord: Ordinal) -> Result<(), WriteError> {
        if self.listener {
            return Err(WriteError::NotOwner);
        }
        self.ports.touch(iface, ord)
    }

    fn commit(&mut self) {
        if !self.listener {
            self.ports.commit();
        }
    }
}

impl EventSource for Attachment {
    fn subscribe(&mut self, iface: InterfaceNo, ords: &[Ordinal]) -> Result<(), SubscribeError> {
        self.ports.subscribe(iface, ords)
    }

    fn unsubscribe(&mut self, iface: InterfaceNo, ords: &[Ordinal]) {
        self.ports.unsubscribe(iface, ords);
    }

    fn next(&mut self, out: &mut [u8]) -> Result<Option<RawOccurrence>, ReadError> {
        self.ports.next(out)
    }
}

impl EventSink for Attachment {
    fn raise(
        &mut self,
        iface: InterfaceNo,
        ord: Ordinal,
        bytes: &[u8],
        trace: Option<TraceContext>,
    ) -> Result<(), RaiseError> {
        if self.listener {
            return Err(RaiseError::NotOwner);
        }
        self.ports.raise(iface, ord, bytes, trace)
    }
}

impl Caller for Attachment {
    fn command(
        &mut self,
        iface: InterfaceNo,
        ord: Ordinal,
        args: &[u8],
        trace: Option<TraceContext>,
    ) -> Result<Correlation, SendError> {
        self.ports.command(iface, ord, args, trace)
    }

    fn query(
        &mut self,
        iface: InterfaceNo,
        ord: Ordinal,
        args: &[u8],
        trace: Option<TraceContext>,
    ) -> Result<Correlation, SendError> {
        self.ports.query(iface, ord, args, trace)
    }

    fn ack(&mut self, c: Correlation) -> Option<Result<(), CallError>> {
        self.ports.ack(c)
    }

    fn reply(
        &mut self,
        c: Correlation,
        out: &mut [u8],
    ) -> Result<Option<Result<usize, CallError>>, ReadError> {
        self.ports.reply(c, out)
    }

    fn forget(&mut self, c: Correlation) {
        self.ports.forget(c);
    }
}

impl Handler for Attachment {
    fn serve(&mut self, iface: InterfaceNo, ords: &[Ordinal]) -> Result<(), ServeError> {
        if self.listener {
            return Err(ServeError::NotOwner);
        }
        self.ports.serve(iface, ords)
    }

    fn next_claim(&mut self, out: &mut [u8]) -> Result<Option<Claim>, ReadError> {
        if self.listener {
            Ok(None)
        } else {
            self.ports.next_claim(out)
        }
    }

    fn settle(
        &mut self,
        claim: ClaimId,
        outcome: Result<&[u8], CallError>,
    ) -> Result<(), SettleError> {
        self.ports.settle(claim, outcome)
    }
}

impl Wakeable for Attachment {
    fn wake_on(&self, what: Interest, waker: &Waker) {
        self.ports.wake_on(what, waker);
    }
}

const _: () = {
    const fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<Attachment>();
};
