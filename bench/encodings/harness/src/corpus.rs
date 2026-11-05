//! The payloads the bench measures, and the rows it records.

use ridl_rt::encoding::{Encoding, FlatBuffers, Podbuf, Proto3};
use ridl_rt::payload::Payload;

use crate::timing::{
    paced_run, runner, sustained_cpu_percent, thread_cpu_ns, time_cells, Paced, Runner, Stat,
    TimingConfig, RATE_PER_S,
};
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
    /// The encoding refused the value: an encode or verify error.
    Refused { reason: String },
    /// The bench could not build a value of the payload, so no encoding was
    /// asked. This is a limit of the bench, not a refusal by an encoding.
    NotBuilt { reason: String },
}

/// The CPU time per update of a measured cell, and the paced run when one was
/// asked for.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RowTiming {
    pub stat: Stat,
    pub paced: Option<Paced>,
}

/// One row: a payload under one encoding.
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    pub payload: &'static str,
    /// The Rust name of the payload type. Signals that share a payload type
    /// share this name, so that a report can group by type.
    pub payload_type: &'static str,
    pub kind: Kind,
    pub encoding: &'static str,
    pub cell: Cell,
    /// Set only on a `Measured` cell, and only when the recorder times.
    pub timing: Option<RowTiming>,
}

/// Collects one row per `(payload, encoding)`.
#[derive(Default)]
pub struct Recorder {
    pub rows: Vec<Row>,
    /// When set, every `Measured` cell is also timed.
    pub timing: Option<TimingConfig>,
    /// With `timing` set: also run each timed cell paced at `RATE_PER_S`
    /// for `timing.duration_s` of wall time.
    pub paced: bool,
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
        let first = self.rows.len();
        self.push::<T>(
            payload,
            kind,
            FlatBuffers::NAME,
            try_measure_counts::<T, FlatBuffers>(value),
        );
        self.push::<T>(
            payload,
            kind,
            Proto3::NAME,
            try_measure_counts::<T, Proto3>(value),
        );
        self.push::<T>(
            payload,
            kind,
            Podbuf::NAME,
            try_measure_counts::<T, Podbuf>(value),
        );
        if let Some(cfg) = self.timing.clone() {
            self.time_rows::<T>(first, value, &cfg);
        }
    }

    /// Times the `Measured` rows from `first` on: one interleaved run of the
    /// encodings that accepted the value, then the paced run of each.
    fn time_rows<T>(&mut self, first: usize, value: &T, cfg: &TimingConfig)
    where
        T: Payload<FlatBuffers> + Payload<Proto3> + Payload<Podbuf>,
    {
        let measured = |rows: &[Row], enc: &str| {
            rows.iter()
                .any(|r| r.encoding == enc && matches!(r.cell, Cell::Measured(_)))
        };
        let mut runners: Vec<Runner> = Vec::new();
        if measured(&self.rows[first..], FlatBuffers::NAME) {
            runners.push(runner::<T, FlatBuffers>(value));
        }
        if measured(&self.rows[first..], Proto3::NAME) {
            runners.push(runner::<T, Proto3>(value));
        }
        if measured(&self.rows[first..], Podbuf::NAME) {
            runners.push(runner::<T, Podbuf>(value));
        }
        if runners.is_empty() {
            return;
        }
        let stats = time_cells(&mut runners, cfg, &thread_cpu_ns);
        for (encoding, stat) in stats {
            let paced = if self.paced {
                let derived = sustained_cpu_percent(stat.median_ns, RATE_PER_S);
                let r = runners
                    .iter_mut()
                    .find(|r| r.encoding == encoding)
                    .expect("a runner of the stat");
                Some(paced_run(&mut *r.run, RATE_PER_S, cfg.duration_s, derived))
            } else {
                None
            };
            let row = self.rows[first..]
                .iter_mut()
                .find(|r| r.encoding == encoding)
                .expect("a row of the encoding");
            row.timing = Some(RowTiming { stat, paced });
        }
    }

    /// Records a `NotBuilt` row under each encoding for a payload that the
    /// bench cannot build a value of.
    pub fn not_built(
        &mut self,
        payload: &'static str,
        payload_type: &'static str,
        kind: Kind,
        reason: &str,
    ) {
        for encoding in ENCODINGS {
            self.rows.push(Row {
                payload,
                payload_type,
                kind,
                encoding,
                cell: Cell::NotBuilt {
                    reason: reason.to_string(),
                },
                timing: None,
            });
        }
    }

    fn push<T>(
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
            payload_type: short_type_name::<T>(),
            kind,
            encoding,
            cell,
            timing: None,
        });
    }
}

/// The name of `T` without its module path: the last path segment of
/// `std::any::type_name`, which does not depend on where the type is declared.
fn short_type_name<T>() -> &'static str {
    let full = std::any::type_name::<T>();
    let head = full.find('<').unwrap_or(full.len());
    let start = full[..head].rfind("::").map_or(0, |i| i + 2);
    &full[start..]
}

const ENCODINGS: [&str; 3] = [FlatBuffers::NAME, Proto3::NAME, Podbuf::NAME];

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
            .not_built(
                "veh.cluster.WheelDiagnostics.wheelTicks",
                "RawTickCount",
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
