//! Reads a signal with its provenance, using `ridl-rt` and nothing else.
//!
//! This program writes by hand what a code generator and a runtime would
//! provide: one catalog with one interface, `Drivetrain`; one signal, `speed`;
//! a `repr(C)` codec for its payload type; an in-memory runtime that
//! implements `Clock`, `SignalReader` and `SignalWriter`; and the accessor a
//! generated client would contain. `main` moves the signal through its
//! provenance and freshness states and prints the sample read after each
//! step.
//!
//! Run it with `cargo run -p ridl-rt --example read_sample`.
//! `tests/read_sample.rs` runs the same steps and checks each sample.

#![forbid(unsafe_code)]

use std::cell::Cell;

use ridl_rt::contract::{
    CatalogHash, CatalogRef, EncodedSizes, Interaction, Interface, InterfaceNo, Kind, Member,
    Ordinal, PayloadInfo, Signal, Timing, TimingMode,
};
use ridl_rt::encoding::ReprC;
use ridl_rt::error::Contract;
use ridl_rt::payload::{
    EncodeError, Encoded, Malformed, Payload, Ref, Rule, VerifyError, Violation,
};
use ridl_rt::port::{
    Attached, Clock, RawSample, ReadError, SignalReader, SignalWriter, WriteError,
};
use ridl_rt::sample::{
    Cause, Detection, Duration, Envelope, Freshness, Provenance, Sample, Timestamp,
};

// ---- What a code generator writes -------------------------------------------

/// Speed in km/h, declared in the range 0 to 300.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Speed(pub u16);

impl Payload<ReprC> for Speed {
    const MAX_SIZE: usize = 2;
    type View<'a> = &'a [u8];

    fn encode<'o>(&self, out: &'o mut [u8]) -> Result<Encoded<'o, &'o [u8]>, EncodeError> {
        let available = out.len();
        let Some(front) = out.get_mut(..2) else {
            return Err(EncodeError::Capacity {
                needed: 2,
                available,
            });
        };
        front.copy_from_slice(&self.0.to_le_bytes());
        let bytes: &'o [u8] = front;
        Ok(Encoded { bytes, view: bytes })
    }

    fn verify(buf: &[u8]) -> Result<&[u8], VerifyError> {
        let bytes: [u8; 2] = buf
            .try_into()
            .map_err(|_| VerifyError::Structure(Malformed::OutOfBounds))?;
        if u16::from_le_bytes(bytes) > 300 {
            return Err(VerifyError::Contract(Violation {
                type_name: "Speed",
                rule: Rule::Range,
            }));
        }
        Ok(buf)
    }

    fn decode(r: Ref<'_, Self, ReprC>) -> Self {
        let b = r.bytes();
        Speed(u16::from_le_bytes([b[0], b[1]]))
    }
}

/// The catalog of the package `vehicle`.
pub const VEHICLE: CatalogRef = CatalogRef {
    name: "vehicle",
    hash: CatalogHash([7; 32]),
};

/// `interface Drivetrain { signal speed : Speed @[20ms..500ms] }`
pub struct Drivetrain;

impl Interface for Drivetrain {
    const CATALOG: &'static CatalogRef = &VEHICLE;
    const NUMBER: InterfaceNo = InterfaceNo(1);
    const PROVISIONAL: bool = false;
    const NAME: &'static str = "Drivetrain";
    const MEMBERS: &'static [Member] = &[Member {
        ordinal: Ordinal(1),
        kind: Kind::Signal,
        name: "speed",
        timing: Some(Timing {
            mode: TimingMode::Range,
            min: Some(Duration(20_000)),
            max: Some(Duration(500_000)),
        }),
        // A generated descriptor fills in every encoding that can carry the
        // payload. This program writes only the `repr(C)` codec, so it fills
        // in only that size.
        payloads: &[PayloadInfo {
            type_name: "Speed",
            max_size: EncodedSizes {
                proto3: None,
                flatbuffers: None,
                repr_c: Some(2),
            },
        }],
    }];
}

/// The descriptor of `Drivetrain.speed`.
pub struct SpeedSignal;

impl Interaction for SpeedSignal {
    type Iface = Drivetrain;
    const MEMBER: &'static Member = &Drivetrain::MEMBERS[0];
}

impl Signal for SpeedSignal {
    type Payload = Speed;
    fn init() -> Speed {
        Speed(30)
    }
}

/// The accessor a generated client writes for `speed`: read the bytes, check
/// them, decode them. When there is no payload to check — before the first
/// publication, or when the channel is invalidated with no prior publication
/// — the value is the init value under the port's own provenance. Otherwise
/// the check runs; when it fails, the accessor reports the detection as the
/// provenance and substitutes the init value. A generated accessor would
/// substitute its own last good value when it has one.
pub fn read_speed(port: &dyn SignalReader) -> Result<Sample<Speed>, ReadError> {
    let mut buf = [0u8; <Speed as Payload<ReprC>>::MAX_SIZE];
    let raw = port.read(Drivetrain::NUMBER, SpeedSignal::MEMBER.ordinal, &mut buf)?;
    let (value, provenance) = match raw.provenance {
        Provenance::Init | Provenance::Invalid(Cause::Declared) if raw.len == 0 => {
            (SpeedSignal::init(), raw.provenance)
        }
        _ => match Ref::<Speed, ReprC>::verify(&buf[..raw.len]) {
            Ok(proof) => (proof.decode(), raw.provenance),
            Err(VerifyError::Contract(violation)) => (
                SpeedSignal::init(),
                Provenance::Invalid(Cause::Detected(Detection::InvalidValue(violation))),
            ),
            Err(_) => (
                SpeedSignal::init(),
                Provenance::Invalid(Cause::Detected(Detection::Corrupt)),
            ),
        },
    };
    Ok(Sample {
        value,
        provenance,
        freshness: raw.freshness,
        envelope: raw.envelope,
    })
}

// ---- What a runtime writes ----------------------------------------------------

/// A runtime for one signal of at most 2 bytes: a manual clock, one slot, and
/// one staged change.
pub struct Memory {
    now: Cell<i64>,
    bytes: [u8; 2],
    len: usize,
    provenance: Provenance,
    envelope: Envelope,
    staged: Option<Staged>,
}

enum Staged {
    Value([u8; 2], usize),
    Invalid,
    Touch,
}

impl Memory {
    /// Creates the channel at `now`, holding the init value's bytes.
    pub fn new(now: Timestamp, init: &[u8]) -> Memory {
        let mut bytes = [0u8; 2];
        bytes[..init.len()].copy_from_slice(init);
        Memory {
            now: Cell::new(now.0),
            bytes,
            len: init.len(),
            provenance: Provenance::Init,
            envelope: Envelope { stamp: now, seq: 0 },
            staged: None,
        }
    }

    /// Moves the clock forward.
    pub fn advance(&self, by: Duration) {
        self.now.set(self.now.get() + by.0);
    }

    fn member(iface: InterfaceNo, ord: Ordinal) -> Option<&'static Member> {
        if iface != Drivetrain::NUMBER {
            return None;
        }
        Drivetrain::MEMBERS
            .iter()
            .find(|m| m.ordinal == ord && m.kind == Kind::Signal)
    }
}

impl Attached for Memory {
    fn catalog(&self) -> &CatalogRef {
        Drivetrain::CATALOG
    }
}

impl Clock for Memory {
    fn now(&self) -> Timestamp {
        Timestamp(self.now.get())
    }
}

impl SignalReader for Memory {
    fn read(
        &self,
        iface: InterfaceNo,
        ord: Ordinal,
        out: &mut [u8],
    ) -> Result<RawSample, ReadError> {
        let member =
            Memory::member(iface, ord).ok_or(ReadError::Contract(Contract::UnknownInteraction))?;
        let Some(front) = out.get_mut(..self.len) else {
            return Err(ReadError::Short { needed: self.len });
        };
        front.copy_from_slice(&self.bytes[..self.len]);
        let freshness = match member.timing.and_then(|t| t.max) {
            None => Freshness::Unbounded,
            Some(max) => {
                let age = self.now().0 - self.envelope.stamp.0;
                if age > max.0 {
                    Freshness::Stale {
                        by: Duration(age - max.0),
                    }
                } else {
                    Freshness::Fresh
                }
            }
        };
        Ok(RawSample {
            provenance: self.provenance,
            freshness,
            envelope: self.envelope,
            len: self.len,
        })
    }
}

impl SignalWriter for Memory {
    fn set(&mut self, iface: InterfaceNo, ord: Ordinal, bytes: &[u8]) -> Result<(), WriteError> {
        if Memory::member(iface, ord).is_none() {
            return Err(WriteError::Contract(Contract::UnknownInteraction));
        }
        let mut value = [0u8; 2];
        let Some(front) = value.get_mut(..bytes.len()) else {
            return Err(WriteError::TooLarge { cap: 2 });
        };
        front.copy_from_slice(bytes);
        self.staged = Some(Staged::Value(value, bytes.len()));
        Ok(())
    }

    fn invalidate(&mut self, iface: InterfaceNo, ord: Ordinal) -> Result<(), WriteError> {
        if Memory::member(iface, ord).is_none() {
            return Err(WriteError::Contract(Contract::UnknownInteraction));
        }
        self.staged = Some(Staged::Invalid);
        Ok(())
    }

    fn touch(&mut self, iface: InterfaceNo, ord: Ordinal) -> Result<(), WriteError> {
        if Memory::member(iface, ord).is_none() {
            return Err(WriteError::Contract(Contract::UnknownInteraction));
        }
        if self.staged.is_none() {
            self.staged = Some(Staged::Touch);
        }
        Ok(())
    }

    fn commit(&mut self) {
        let Some(staged) = self.staged.take() else {
            return;
        };
        match staged {
            Staged::Value(bytes, len) => {
                self.bytes = bytes;
                self.len = len;
                self.provenance = Provenance::Live;
            }
            Staged::Invalid => self.provenance = Provenance::Invalid(Cause::Declared),
            Staged::Touch => {}
        }
        self.envelope = Envelope {
            stamp: self.now(),
            seq: self.envelope.seq + 1,
        };
    }
}

// ---- The program --------------------------------------------------------------

/// Publishes `speed` the way a generated publisher does: encode, stage, commit.
fn publish(runtime: &mut Memory, speed: Speed) {
    let mut buf = [0u8; <Speed as Payload<ReprC>>::MAX_SIZE];
    let proof = Ref::<Speed, ReprC>::encode(&speed, &mut buf).expect("the buffer is MAX_SIZE");
    runtime
        .set(
            Drivetrain::NUMBER,
            SpeedSignal::MEMBER.ordinal,
            proof.bytes(),
        )
        .expect("the runtime owns speed");
    runtime.commit();
}

/// Moves `speed` through six states and returns the sample read after each:
/// the init value; a live value; the same value once its staleness bound has
/// passed; the invalid state the provider declares; a value the accessor's
/// check rejects; and bytes the accessor cannot decode.
pub fn walk() -> [Sample<Speed>; 6] {
    let mut init = [0u8; <Speed as Payload<ReprC>>::MAX_SIZE];
    let proof = Ref::<Speed, ReprC>::encode(&SpeedSignal::init(), &mut init)
        .expect("the buffer is MAX_SIZE");
    let mut runtime = Memory::new(Timestamp(1_000_000), proof.bytes());
    let read = |runtime: &Memory| read_speed(runtime).expect("speed is in the catalog");

    let at_init = read(&runtime);

    runtime.advance(Duration(10_000));
    publish(&mut runtime, Speed(88));
    let live = read(&runtime);

    runtime.advance(Duration(600_000));
    let stale = read(&runtime);

    runtime
        .invalidate(Drivetrain::NUMBER, SpeedSignal::MEMBER.ordinal)
        .expect("the runtime owns speed");
    runtime.commit();
    let declared = read(&runtime);

    runtime
        .set(
            Drivetrain::NUMBER,
            SpeedSignal::MEMBER.ordinal,
            &400u16.to_le_bytes(),
        )
        .expect("the runtime owns speed");
    runtime.commit();
    let detected = read(&runtime);

    // One byte where the encoding needs two.
    runtime
        .set(Drivetrain::NUMBER, SpeedSignal::MEMBER.ordinal, &[1])
        .expect("the runtime owns speed");
    runtime.commit();
    let corrupt = read(&runtime);

    [at_init, live, stale, declared, detected, corrupt]
}

fn main() {
    let steps = [
        "init",
        "live",
        "stale",
        "declared invalid",
        "detected invalid",
        "corrupt",
    ];
    for (step, sample) in steps.iter().zip(walk()) {
        println!("{step}: {sample:?} usable={}", sample.usable());
    }
}
