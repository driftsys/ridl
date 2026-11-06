//! The payloads the bench measures, and the rows it records.

use ridl_rt::encoding::{Encoding, FlatBuffers, Podbuf, Proto3};
use ridl_rt::payload::Payload;

use crate::timing::{
    op_runners_pool, paced_run, runner, sustained_cpu_percent, thread_cpu_ns, time_cells, OpStats,
    Paced, Runner, Stat, TimingConfig, RATE_PER_S,
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

/// The CPU time of a measured cell: per operation, per update, and the paced
/// run when one was asked for.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RowTiming {
    /// The per update figure: the sum of the three operation stats
    /// (`OpStats::per_update`).
    pub stat: Stat,
    /// Encode, verify and decode, each timed in batches of its own.
    pub ops: OpStats,
    pub paced: Option<Paced>,
}

impl RowTiming {
    /// Whether the spread of any operation is flagged.
    pub fn flagged(&self) -> bool {
        self.ops.flagged()
    }
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
    /// Time encode, verify and decode on a pool of distinct values, for the
    /// payloads declared with `stress_pool!`.
    pub vary: bool,
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
        self.record_values(payload, kind, std::slice::from_ref(value), false);
    }

    /// Like `record`, for a payload with a pool of distinct values of one
    /// shape. Size and allocations come from the first value. With `vary` set
    /// on the recorder, encode, verify and decode cycle through the pool; with
    /// it unset they repeat the first value, as `record` does.
    pub fn record_pool<T>(&mut self, payload: &'static str, kind: Kind, pool: &[T])
    where
        T: Payload<FlatBuffers> + Payload<Proto3> + Payload<Podbuf>,
    {
        self.record_values(payload, kind, pool, self.vary);
    }

    fn record_values<T>(&mut self, payload: &'static str, kind: Kind, pool: &[T], cycle: bool)
    where
        T: Payload<FlatBuffers> + Payload<Proto3> + Payload<Podbuf>,
    {
        let value = &pool[0];
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
            let timed_pool = if cycle { pool } else { &pool[..1] };
            self.time_rows::<T>(first, timed_pool, &cfg);
        }
    }

    /// Times the `Measured` rows from `first` on: one run of the operations
    /// of the encodings that accepted the value, each operation in batches of
    /// its own, interleaved in rotation; then the paced run of each encoding,
    /// which repeats the whole update.
    fn time_rows<T>(&mut self, first: usize, pool: &[T], cfg: &TimingConfig)
    where
        T: Payload<FlatBuffers> + Payload<Proto3> + Payload<Podbuf>,
    {
        let measured = |rows: &[Row], enc: &str| {
            rows.iter()
                .any(|r| r.encoding == enc && matches!(r.cell, Cell::Measured(_)))
        };
        let mut runners: Vec<Runner> = Vec::new();
        let mut whole: Vec<Runner> = Vec::new();
        if measured(&self.rows[first..], FlatBuffers::NAME) {
            runners.extend(op_runners_pool::<T, FlatBuffers>(pool));
            whole.push(runner::<T, FlatBuffers>(&pool[0]));
        }
        if measured(&self.rows[first..], Proto3::NAME) {
            runners.extend(op_runners_pool::<T, Proto3>(pool));
            whole.push(runner::<T, Proto3>(&pool[0]));
        }
        if measured(&self.rows[first..], Podbuf::NAME) {
            runners.extend(op_runners_pool::<T, Podbuf>(pool));
            whole.push(runner::<T, Podbuf>(&pool[0]));
        }
        if runners.is_empty() {
            return;
        }
        let timed = time_cells(&mut runners, cfg, &thread_cpu_ns);
        for r in &mut whole {
            let ops = OpStats::of(&timed, r.encoding);
            let stat = ops.per_update();
            let paced = if self.paced {
                let derived = sustained_cpu_percent(stat.median_ns, RATE_PER_S);
                Some(paced_run(&mut *r.run, RATE_PER_S, cfg.duration_s, derived))
            } else {
                None
            };
            let row = self.rows[first..]
                .iter_mut()
                .find(|row| row.encoding == r.encoding)
                .expect("a row of the encoding");
            row.timing = Some(RowTiming { stat, ops, paced });
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

/// The number of distinct values in a pool.
pub const POOL_SIZE: i64 = 64;

/// A stress entry with a pool: `$build(seed)` makes one value, and seed 0 is
/// the fixed value. With `--vary` the pool is the values of seeds 0 to 63.
macro_rules! stress_pool {
    ($name:literal, $build:expr) => {
        entry!($name, Kind::Stress, |r| {
            let pool: Vec<_> = if r.vary {
                (0..POOL_SIZE).map($build).collect()
            } else {
                vec![$build(0)]
            };
            r.record_pool($name, Kind::Stress, &pool)
        })
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

fn label(text: String) -> st::Label {
    st::Label::new(text).unwrap()
}

fn count(n: i64) -> st::Count {
    st::Count::new(n % 65536).unwrap()
}

/// 100 items. The seed changes the counts and nothing else; seed 0 is the
/// fixed value.
fn big_item_list(seed: i64) -> st::BigItemListPayload {
    st::BigItemListPayload {
        items: (0..100i64)
            .map(|i| st::Item {
                label: label(format!("item-{i:03}")),
                count: count(i * 300 + seed * 7),
            })
            .collect(),
    }
}

/// 1500 samples. The seed shifts every sample.
fn big_samples(seed: i64) -> st::BigSamplesPayload {
    st::BigSamplesPayload {
        samples: (0..1500i64)
            .map(|i| count_sample(i * 40 + seed * 3))
            .collect(),
    }
}

fn count_sample(n: i64) -> st::Sample {
    st::Sample::new(n % 65536).unwrap()
}

/// 120 map entries. The seed changes the counts and rotates the keys.
fn big_map(seed: i64) -> st::BigMapPayload {
    st::BigMapPayload {
        counts: (0..120i64)
            .map(|i| {
                (
                    label(format!("key-{:03}", (i + seed) % 120)),
                    count(i * 100 + seed * 5),
                )
            })
            .collect(),
    }
}

/// Ten rows of four cells each: the deepest payload that stays under the
/// interaction ceiling.
fn deep_nested(seed: i64) -> st::DeepNestedPayload {
    st::DeepNestedPayload {
        rows: (0..10i64)
            .map(|r| st::DeepRow {
                label: label(format!("row-{:03}", (seed * 10 + r) % 1000)),
                pair: st::DeepPair {
                    first: count(seed * 31 + r * 7),
                    second: count(seed * 17 + r * 13),
                },
                cells: (0..4i64)
                    .map(|c| st::DeepCell {
                        name: label(format!("cell-{:03}", (seed * 4 + c + r) % 1000)),
                        value: count(seed * 101 + r * 1000 + c * 37),
                    })
                    .collect(),
            })
            .collect(),
    }
}

/// The arm of the union and the mode cycle with the seed, so that a pool of
/// values holds every arm and a mode that no name covers.
fn variant(seed: i64) -> st::UnionPayload {
    let reading = match (seed + 2) % 3 {
        0 => st::Reading::Count(count(seed * 977)),
        1 => st::Reading::Text(label(format!("text-{seed:03}"))),
        _ => st::Reading::Item(st::Item {
            label: label(format!("item-{seed:03}")),
            count: count(seed * 313),
        }),
    };
    let mode = st::Mode::from((seed + 2) % 4);
    st::UnionPayload { reading, mode }
}

/// Two of the 30 optional fields set: one in the outer layer, one in the
/// innermost layer.
#[allow(clippy::field_reassign_with_default)]
fn deep_sparse(seed: i64) -> st::DeepSparsePayload {
    let mut v = st::DeepSparsePayload::default();
    v.a0 = Some(count(seed * 53 + 5));
    v.mid.inner.z9 = Some(label(format!("z-{seed:03}")));
    v
}

/// All 30 optional fields set.
#[allow(clippy::field_reassign_with_default)]
fn deep_full(seed: i64) -> st::DeepSparsePayload {
    let c = |i: i64| Some(count(seed * 59 + i * 11));
    let l = |i: i64| Some(label(format!("f{i:02}-{seed:03}")));
    let mut v = st::DeepSparsePayload::default();
    v.a0 = c(0);
    v.a1 = l(1);
    v.a2 = c(2);
    v.a3 = l(3);
    v.a4 = c(4);
    v.a5 = l(5);
    v.a6 = c(6);
    v.a7 = l(7);
    v.a8 = c(8);
    v.a9 = l(9);
    v.mid.m0 = c(10);
    v.mid.m1 = l(11);
    v.mid.m2 = c(12);
    v.mid.m3 = l(13);
    v.mid.m4 = c(14);
    v.mid.m5 = l(15);
    v.mid.m6 = c(16);
    v.mid.m7 = l(17);
    v.mid.m8 = c(18);
    v.mid.m9 = l(19);
    v.mid.inner.z0 = c(20);
    v.mid.inner.z1 = l(21);
    v.mid.inner.z2 = c(22);
    v.mid.inner.z3 = l(23);
    v.mid.inner.z4 = c(24);
    v.mid.inner.z5 = l(25);
    v.mid.inner.z6 = c(26);
    v.mid.inner.z7 = l(27);
    v.mid.inner.z8 = c(28);
    v.mid.inner.z9 = l(29);
    v
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
        stress!(
            "bench.stress.Stress.bigString",
            st::BigStringPayload {
                text: st::BigText::new("x".repeat(3000)).unwrap()
            }
        ),
        stress_pool!("bench.stress.Stress.bigItemList", big_item_list),
        stress_pool!("bench.stress.Stress.bigSamples", big_samples),
        stress_pool!("bench.stress.Stress.bigMap", big_map),
        stress_pool!("bench.stress.Stress.deepNested", deep_nested),
        stress_pool!("bench.stress.Stress.variant", variant),
        stress_pool!("bench.stress.Stress.deepSparse", deep_sparse),
        stress_pool!("bench.stress.Stress.deepFull", deep_full),
    ]
}
