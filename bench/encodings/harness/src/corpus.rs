//! The payloads the bench measures, and the rows it records.

use ridl_rt::encoding::{FlatBuffers, Podbuf, Proto3};
use ridl_rt::payload::Payload;

use crate::{try_measure_counts, Counts};

/// Which part of the corpus a payload belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// The payload of a signal of the veh-cluster corpus or of examples/cabin.
    Signal,
    /// A payload of the stress package, built to be large or deeply nested.
    Stress,
}

/// The result for one payload under one encoding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Cell {
    /// The encoding accepted the value.
    Measured(Counts),
    /// The encoding refused the value, or the payload cannot be built.
    Refused { reason: String },
}

/// One row: a payload under one encoding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub payload: &'static str,
    pub kind: Kind,
    pub encoding: &'static str,
    pub cell: Cell,
}

/// Collects one row per `(payload, encoding)`.
#[derive(Default)]
pub struct Recorder {
    pub rows: Vec<Row>,
}

/// A payload and the code that measures it.
pub struct CorpusEntry {
    pub name: &'static str,
    pub kind: Kind,
    pub run: &'static dyn Fn(&mut Recorder),
}

impl Recorder {
    /// Measures `value` under each encoding and records one row per encoding.
    /// An encoding that refuses the value gets a `Refused` row.
    pub fn record<T>(&mut self, payload: &'static str, kind: Kind, value: &T)
    where
        T: Payload<FlatBuffers> + Payload<Proto3> + Payload<Podbuf>,
    {
        self.push(
            payload,
            kind,
            "flatbuffers",
            try_measure_counts::<T, FlatBuffers>(value),
        );
        self.push(
            payload,
            kind,
            "proto3",
            try_measure_counts::<T, Proto3>(value),
        );
        self.push(
            payload,
            kind,
            "podbuf",
            try_measure_counts::<T, Podbuf>(value),
        );
    }

    /// Records a `Refused` row under each encoding for a payload that the
    /// bench cannot build a value of.
    pub fn refuse_all(&mut self, payload: &'static str, kind: Kind, reason: &str) {
        for encoding in ENCODINGS {
            self.push(payload, kind, encoding, Err(reason.to_string()));
        }
    }

    fn push(
        &mut self,
        payload: &'static str,
        kind: Kind,
        encoding: &'static str,
        result: Result<Counts, String>,
    ) {
        let cell = match result {
            Ok(counts) => Cell::Measured(counts),
            Err(reason) => Cell::Refused { reason },
        };
        self.rows.push(Row {
            payload,
            kind,
            encoding,
            cell,
        });
    }
}

const ENCODINGS: [&str; 3] = ["flatbuffers", "proto3", "podbuf"];

macro_rules! entry {
    ($name:literal, $kind:expr, |$rec:ident| $body:expr) => {
        CorpusEntry {
            name: $name,
            kind: $kind,
            run: &|$rec: &mut Recorder| $body,
        }
    };
}

/// A signal entry: `name` is `<package>.<owner>.<signal>`.
macro_rules! signal {
    ($name:literal, $value:expr) => {
        entry!($name, Kind::Signal, |r| r.record(
            $name,
            Kind::Signal,
            &$value
        ))
    };
}

macro_rules! stress {
    ($name:literal, $value:expr) => {
        entry!($name, Kind::Stress, |r| r.record(
            $name,
            Kind::Stress,
            &$value
        ))
    };
}

use bench_cabin::veh::cabin;
use bench_stress::bench::stress as st;
use bench_veh_cluster::veh::cluster as cl;
use bench_veh_cluster::veh::common as cm;

fn speed() -> cm::Speed {
    cm::Speed::new(72.5).unwrap()
}

fn temperature() -> cm::Temperature {
    cm::Temperature::new(91.5).unwrap()
}

fn ratio() -> cm::Ratio {
    cm::Ratio::new(42.5).unwrap()
}

fn fan() -> cl::FanLevel {
    cl::FanLevel::new(3).unwrap()
}

/// Every entry of the corpus: one per signal of the veh-cluster corpus and of
/// examples/cabin, then one per stress payload. Each builds a representative
/// value by hand.
pub fn corpus() -> Vec<CorpusEntry> {
    vec![
        signal!("veh.cluster.VehicleStatus.currentSpeed", speed()),
        signal!("veh.cluster.VehicleStatus.engineTemp", temperature()),
        signal!(
            "veh.cluster.VehicleStatus.warnings",
            cm::WarningFlags::LOW_FUEL
        ),
        signal!("veh.cluster.WheelHistory.wheelTicks", fan()),
        signal!("veh.cluster.WheelHistory.wheelTemp", temperature()),
        signal!("veh.cluster.WheelHistory.wheelSlipRate", ratio()),
        signal!("veh.cluster.WheelSummary.wheelHealth", fan()),
        signal!("veh.cluster.wheels.ticks", fan()),
        entry!(
            "veh.cluster.WheelDiagnostics.wheelTicks",
            Kind::Signal,
            |r| {
                r
            .refuse_all(
                "veh.cluster.WheelDiagnostics.wheelTicks",
                Kind::Signal,
                "the payload type RawTickCount is internal to its package, so a value cannot be built outside the generated crate",
            )
            }
        ),
        signal!("veh.hvac.cabin.cabinTemp", temperature()),
        signal!("veh.hvac.cabin.targetTemp", temperature()),
        signal!("veh.hvac.cabin.blowerDuty", ratio()),
        signal!("veh.hvac.cabin.fanLevel", fan()),
        signal!("veh.hvac.cabin.compressorLoad", ratio()),
        signal!(
            "veh.cabin.Cabin.temperature",
            cabin::Temperature::new(21).unwrap()
        ),
        signal!("veh.cabin.Horn.active", cabin::Health::Warn),
        stress!(
            "bench.stress.Stress.longString",
            st::LongStringPayload {
                text: st::LongText::new("x".repeat(512)).unwrap()
            }
        ),
        stress!(
            "bench.stress.Stress.itemList",
            st::ItemListPayload {
                items: (0..16)
                    .map(|i| st::Item {
                        label: st::Label::new(format!("item-{i:02}")).unwrap(),
                        count: st::Count::new(i * 1000).unwrap(),
                    })
                    .collect()
            }
        ),
        stress!(
            "bench.stress.Stress.nestedOptional",
            st::NestedOptionalPayload {
                outer: Some(st::Inner {
                    leaf: Some(st::Leaf {
                        label: st::Label::new("leaf".to_string()).unwrap()
                    }),
                    count: Some(st::Count::new(7).unwrap()),
                }),
                tag: Some(st::Label::new("tag".to_string()).unwrap()),
            }
        ),
        stress!(
            "bench.stress.Stress.map",
            st::MapPayload {
                counts: (0..16)
                    .map(|i| (
                        st::Label::new(format!("key-{i:02}")).unwrap(),
                        st::Count::new(i * 100).unwrap()
                    ))
                    .collect()
            }
        ),
    ]
}
