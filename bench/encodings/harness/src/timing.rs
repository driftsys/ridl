//! CPU time per update: encode, verify and decode one value through
//! `Payload<E>`, timed as thread CPU time over batches, and the CPU load that
//! cost means at a fixed update rate.

use std::hint::black_box;
use std::time::{Duration, Instant};

use ridl_rt::encoding::{Encoding, FlatBuffers, Podbuf};
use ridl_rt::payload::{Payload, Ref};

/// The update rate of the headline figure, in updates per second.
pub const RATE_PER_S: f64 = 4000.0;
/// The fewest samples a cell may report.
pub const MIN_SAMPLES: usize = 30;
/// A cell whose `(p95 - p05) / median` is above this is flagged.
pub const SPREAD_LIMIT: f64 = 0.10;

/// The CPU time per update of one cell, over many samples.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Stat {
    pub median_ns: f64,
    pub p05_ns: f64,
    pub p95_ns: f64,
    /// The fastest and the slowest sample. With 30 samples the 95th
    /// percentile never includes the slowest one, so a single disturbed
    /// repetition is visible only here.
    pub min_ns: f64,
    pub max_ns: f64,
    /// The number of samples the figures come from.
    pub reps: usize,
}

impl Stat {
    /// `(p95 - p05) / median`: the width of the middle 90 % of the samples
    /// relative to the median.
    pub fn spread(&self) -> f64 {
        (self.p95_ns - self.p05_ns) / self.median_ns
    }

    /// `(max - min) / median`: the width of all the samples.
    pub fn range_spread(&self) -> f64 {
        (self.max_ns - self.min_ns) / self.median_ns
    }

    /// Whether the spread or the range is above `SPREAD_LIMIT`, so that the
    /// report can mark the cell as noisy.
    pub fn spread_flagged(&self) -> bool {
        self.spread() > SPREAD_LIMIT || self.range_spread() > SPREAD_LIMIT
    }
}

impl Stat {
    /// The sum of the stats, field by field: the median of the sum is the sum
    /// of the medians, and the other bounds are sums of the bounds, an
    /// envelope and not percentiles of the summed samples. `reps` is the
    /// smallest of the inputs.
    pub fn sum(stats: &[Stat]) -> Stat {
        Stat {
            median_ns: stats.iter().map(|s| s.median_ns).sum(),
            p05_ns: stats.iter().map(|s| s.p05_ns).sum(),
            p95_ns: stats.iter().map(|s| s.p95_ns).sum(),
            min_ns: stats.iter().map(|s| s.min_ns).sum(),
            max_ns: stats.iter().map(|s| s.max_ns).sum(),
            reps: stats.iter().map(|s| s.reps).min().unwrap_or(0),
        }
    }
}

/// The percentile `p` (0 to 1) of sorted values, by linear interpolation
/// between the two nearest ranks.
fn percentile(sorted: &[f64], p: f64) -> f64 {
    let pos = p * (sorted.len() - 1) as f64;
    let lo = pos.floor() as usize;
    let hi = pos.ceil() as usize;
    sorted[lo] + (sorted[hi] - sorted[lo]) * (pos - lo as f64)
}

/// Median, 5th and 95th percentile of `samples` (nanoseconds per update).
/// Panics when `samples` is empty.
pub fn summarize(samples: &[f64]) -> Stat {
    let mut sorted = samples.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).expect("a sample is not NaN"));
    Stat {
        median_ns: percentile(&sorted, 0.5),
        p05_ns: percentile(&sorted, 0.05),
        p95_ns: percentile(&sorted, 0.95),
        min_ns: sorted[0],
        max_ns: sorted[sorted.len() - 1],
        reps: sorted.len(),
    }
}

/// The CPU load, in percent of one core, of `rate_per_s` updates per second
/// at `per_update_ns` of CPU time each.
pub fn sustained_cpu_percent(per_update_ns: f64, rate_per_s: f64) -> f64 {
    // ns * (1/s) is ns per second; 1e9 ns per second is 100 %.
    per_update_ns * rate_per_s / 1e7
}

/// The order in which the `n` encodings are visited in sample number
/// `sample`: the rotation starts one position later at each sample, so that
/// every encoding runs first, second and last equally often.
pub fn interleave_order(sample: usize, n: usize) -> Vec<usize> {
    (0..n).map(|k| (sample + k) % n).collect()
}

/// Calls `f(encoding_index)` once per encoding per sample, in the rotation of
/// `interleave_order`, and returns the values of each encoding in sample
/// order.
pub fn collect_interleaved(
    samples: usize,
    n: usize,
    f: &mut dyn FnMut(usize) -> f64,
) -> Vec<Vec<f64>> {
    let mut out = vec![Vec::with_capacity(samples); n];
    for sample in 0..samples {
        for enc in interleave_order(sample, n) {
            out[enc].push(f(enc));
        }
    }
    out
}

/// How a cell is timed.
#[derive(Debug, Clone)]
pub struct TimingConfig {
    /// Samples per cell; at least `MIN_SAMPLES`.
    pub samples: usize,
    /// The CPU time spent in the samples of one operation of one cell, per
    /// encoding.
    pub duration_s: f64,
    /// Batches run and discarded before the samples.
    pub warmup_batches: u32,
}

impl TimingConfig {
    pub fn new(duration_s: f64) -> Self {
        TimingConfig {
            samples: MIN_SAMPLES,
            duration_s,
            warmup_batches: 3,
        }
    }
}

/// What a runner repeats in a batch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    /// Encode the value into the caller's buffer.
    Encode,
    /// Verify a fixed set of already-encoded bytes.
    Verify,
    /// Decode from a proof that was built once, before the batch.
    Decode,
    /// Read one number through the accessors of a verified view: a scalar
    /// field, or the length of a string field. Whether the read avoids
    /// decoding the rest of the value depends on the encoding's accessor. Not
    /// part of an update.
    Access,
    /// The whole update: encode, verify and decode.
    Update,
}

impl Op {
    pub fn name(self) -> &'static str {
        match self {
            Op::Encode => "encode",
            Op::Verify => "verify",
            Op::Decode => "decode",
            Op::Access => "access",
            Op::Update => "update",
        }
    }
}

/// The three operations of one update, in the order of an update.
pub const OPS: [Op; 3] = [Op::Encode, Op::Verify, Op::Decode];

/// One operation of one encoding of one payload, ready to run a batch.
pub struct Runner<'a> {
    pub encoding: &'static str,
    pub op: Op,
    pub run: Box<dyn FnMut(u64) + 'a>,
}

/// The timing of one runner.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Timed {
    pub encoding: &'static str,
    pub op: Op,
    pub stat: Stat,
}

/// The timing of the three operations of one cell.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OpStats {
    pub encode: Stat,
    pub verify: Stat,
    pub decode: Stat,
    /// The read of one number through the view (see `Op::Access`), for a
    /// payload and an encoding that have an accessor. It is not part of the per-update figure.
    pub access: Option<Stat>,
}

impl OpStats {
    /// The stats of `encoding` out of `timed`. Panics when one of the three
    /// operations is missing.
    pub fn of(timed: &[Timed], encoding: &str) -> OpStats {
        let find = |op: Op| {
            timed
                .iter()
                .find(|t| t.encoding == encoding && t.op == op)
                .unwrap_or_else(|| panic!("no {} timing for {encoding}", op.name()))
                .stat
        };
        OpStats {
            encode: find(Op::Encode),
            verify: find(Op::Verify),
            decode: find(Op::Decode),
            access: timed
                .iter()
                .find(|t| t.encoding == encoding && t.op == Op::Access)
                .map(|t| t.stat),
        }
    }

    /// The per-update stat: the sum of encode, verify and decode (see
    /// `Stat::sum`); the access is not part of it.
    pub fn per_update(&self) -> Stat {
        Stat::sum(&[self.encode, self.verify, self.decode])
    }

    /// Whether any operation is flagged.
    pub fn flagged(&self) -> bool {
        self.encode.spread_flagged()
            || self.verify.spread_flagged()
            || self.decode.spread_flagged()
            || self.access.is_some_and(|a| a.spread_flagged())
    }
}

/// A runner that does `n` updates of `value` under `E`: encode into a stack
/// buffer, verify the bytes, decode them. Panics when `E` refuses the value;
/// the caller times only a value that `try_measure_counts` accepted.
pub fn runner<'a, T: Payload<E>, E: Encoding>(value: &'a T) -> Runner<'a> {
    Runner {
        encoding: E::NAME,
        op: Op::Update,
        run: Box::new(move |n| {
            let mut buf = T::BUFFER;
            for _ in 0..n {
                let encoded = black_box(Ref::<T, E>::encode(black_box(value), buf.as_mut()))
                    .expect("the encoding accepts the value");
                let verified = black_box(Ref::<T, E>::verify(black_box(encoded.bytes())))
                    .expect("the encoding verifies its own bytes");
                black_box(verified.decode());
            }
        }),
    }
}

/// The three operations of `E` on `value`, each a runner of its own. See
/// `op_runners_pool`; this is the pool of one value.
pub fn op_runners<'a, T: Payload<E>, E: Encoding>(value: &'a T) -> [Runner<'a>; 3] {
    op_runners_pool::<T, E>(std::slice::from_ref(value))
}

/// The encoded bytes of each value of the pool, leaked so that proofs over
/// them can live in a runner.
fn encoded_pool<T: Payload<E>, E: Encoding>(pool: &[T]) -> Vec<&'static [u8]> {
    pool.iter()
        .map(|value| {
            let mut scratch = T::BUFFER;
            let encoded = Ref::<T, E>::encode(value, scratch.as_mut())
                .expect("the encoding accepts the value");
            &*Box::leak(encoded.bytes().to_vec().into_boxed_slice())
        })
        .collect()
}

fn proofs_of<T: Payload<E>, E: Encoding>(buffers: &[&'static [u8]]) -> Vec<Ref<'static, T, E>> {
    buffers
        .iter()
        .map(|&bytes| Ref::<T, E>::verify(bytes).expect("the encoding verifies its own bytes"))
        .collect()
}

/// Reads one number out of a verified view: a scalar field, or the length of
/// a string field.
pub type Accessor<T, E> = for<'v> fn(&<T as Payload<E>>::View<'v>) -> u64;

/// The accessors of a payload: one function per encoding that has accessors
/// on its view. proto3 has none, so it has no entry.
pub struct Accessors<T: Payload<FlatBuffers> + Payload<Podbuf>> {
    pub flatbuffers: Accessor<T, FlatBuffers>,
    pub podbuf: Accessor<T, Podbuf>,
}

/// A runner of the access operation: each repetition reads one number through
/// the view of the next of the proofs that `Ref::verify` built once, here,
/// before any batch. The result goes through `black_box`. The pool has the
/// bytes of every value, leaked as in `op_runners_pool`: one buffer per value.
/// A pool of one value does no indexing.
pub fn access_runner<'a, T: Payload<E>, E: Encoding>(
    pool: &'a [T],
    read: Accessor<T, E>,
) -> Runner<'a> {
    let proofs = proofs_of::<T, E>(&encoded_pool::<T, E>(pool));
    let run: Box<dyn FnMut(u64) + 'a> = if proofs.len() == 1 {
        let proof = proofs.into_iter().next().expect("one proof");
        Box::new(move |n| {
            for _ in 0..n {
                black_box(read(black_box(&proof).view()));
            }
        })
    } else {
        Box::new(move |n| {
            for k in 0..n as usize {
                let proof = &proofs[k % proofs.len()];
                black_box(read(black_box(proof).view()));
            }
        })
    };
    Runner {
        encoding: E::NAME,
        op: Op::Access,
        run,
    }
}

/// The three operations of `E` on a pool of values, each a runner of its own.
/// A batch of `n` repeats one operation `n` times and nothing else. With a
/// pool of one value every repetition works on that value and the loop does
/// no indexing. With a larger pool the k-th repetition works on element
/// `k % pool.len()`, so that a pool of distinct values keeps the work from
/// being hoisted out of the loop:
///
/// - encode writes the next value into the same buffer each time;
/// - verify checks the next of the already-encoded buffers each time (one
///   buffer per value, built here before any batch);
/// - decode builds a value from the next of the proofs that `Ref::verify`
///   built once, here, before any batch. `Ref::decode` takes the proof by
///   value and the proof cannot be cloned, so each decode reads a bitwise
///   copy of it. That is sound only when the proof owns nothing that is
///   dropped; the function panics otherwise. The bytes of the proofs are
///   leaked, one buffer per value of the pool and so up to 64 buffers of up
///   to the maximum encoded size per call, so that the proofs can live in the
///   runner.
///
/// Every input and every result goes through `black_box`. Panics when `E`
/// refuses a value or the pool is empty; the caller times only values that
/// `try_measure_counts` accepted.
pub fn op_runners_pool<'a, T: Payload<E>, E: Encoding>(pool: &'a [T]) -> [Runner<'a>; 3] {
    assert!(!pool.is_empty(), "the pool holds at least one value");
    let mut buf = T::BUFFER;
    let buffers = encoded_pool::<T, E>(pool);
    let proofs = proofs_of::<T, E>(&buffers);
    assert!(
        !std::mem::needs_drop::<Ref<'static, T, E>>(),
        "the proof of this payload owns data that is dropped, so it cannot be copied"
    );
    let [encode_run, verify_run, decode_run]: [Box<dyn FnMut(u64) + 'a>; 3] = if pool.len() == 1 {
        let value = &pool[0];
        let bytes = buffers[0];
        let proof = proofs.into_iter().next().expect("one proof");
        [
            Box::new(move |n| {
                for _ in 0..n {
                    let _ = black_box(Ref::<T, E>::encode(black_box(value), buf.as_mut()));
                }
            }),
            Box::new(move |n| {
                for _ in 0..n {
                    let _ = black_box(Ref::<T, E>::verify(black_box(bytes)));
                }
            }),
            Box::new(move |n| {
                for _ in 0..n {
                    // SAFETY: see the indexed decode below.
                    let copy = unsafe { std::ptr::read(black_box(&proof)) };
                    black_box(copy.decode());
                }
            }),
        ]
    } else {
        let verify_buffers = buffers.clone();
        [
            Box::new(move |n| {
                for k in 0..n as usize {
                    let value = &pool[k % pool.len()];
                    let _ = black_box(Ref::<T, E>::encode(black_box(value), buf.as_mut()));
                }
            }),
            Box::new(move |n| {
                for k in 0..n as usize {
                    let bytes = verify_buffers[k % verify_buffers.len()];
                    let _ = black_box(Ref::<T, E>::verify(black_box(bytes)));
                }
            }),
            Box::new(move |n| {
                for k in 0..n as usize {
                    let proof = &proofs[k % proofs.len()];
                    // SAFETY: `Ref` has no `Drop` implementation and the
                    // assertion above shows that its view needs none, so a
                    // bitwise copy owns nothing and the original is never
                    // used after the copies.
                    let copy = unsafe { std::ptr::read(black_box(proof)) };
                    black_box(copy.decode());
                }
            }),
        ]
    };
    [
        Runner {
            encoding: E::NAME,
            op: Op::Encode,
            run: encode_run,
        },
        Runner {
            encoding: E::NAME,
            op: Op::Verify,
            run: verify_run,
        },
        Runner {
            encoding: E::NAME,
            op: Op::Decode,
            run: decode_run,
        },
    ]
}

/// Thread CPU time in nanoseconds.
pub fn thread_cpu_ns() -> u64 {
    let mut ts = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // SAFETY: `ts` is a valid timespec for the call to fill.
    let rc = unsafe { libc::clock_gettime(libc::CLOCK_THREAD_CPUTIME_ID, &mut ts) };
    assert_eq!(rc, 0, "clock_gettime(CLOCK_THREAD_CPUTIME_ID)");
    ts.tv_sec as u64 * 1_000_000_000 + ts.tv_nsec as u64
}

/// Process CPU time (user plus system) in nanoseconds.
pub fn process_cpu_ns() -> u64 {
    // SAFETY: an all-zero rusage is a valid value for the call to fill.
    let mut ru: libc::rusage = unsafe { std::mem::zeroed() };
    // SAFETY: `ru` is a valid rusage for the call to fill.
    let rc = unsafe { libc::getrusage(libc::RUSAGE_SELF, &mut ru) };
    assert_eq!(rc, 0, "getrusage");
    let tv = |t: libc::timeval| t.tv_sec as u64 * 1_000_000_000 + t.tv_usec as u64 * 1_000;
    tv(ru.ru_utime) + tv(ru.ru_stime)
}

/// The shortest batch the size calibration accepts, in nanoseconds of the
/// clock. A batch shorter than this is dominated by the clock's resolution.
const CALIBRATION_FLOOR_NS: u64 = 1_000_000;
/// The shortest batch that is sampled, in nanoseconds of the clock.
const MIN_BATCH_NS: u64 = 200_000;
/// Runs of one batch size whose minimum is taken, so that one disturbed run
/// does not decide the batch size.
const CALIBRATION_RUNS: usize = 3;

/// The shortest of `CALIBRATION_RUNS` runs of a batch of `n` updates.
fn min_batch_ns(run: &mut dyn FnMut(u64), n: u64, now: &dyn Fn() -> u64) -> u64 {
    (0..CALIBRATION_RUNS)
        .map(|_| {
            let t0 = now();
            run(n);
            now().saturating_sub(t0)
        })
        .min()
        .expect("at least one run")
}

/// Doubles the batch size until the fastest of several batches takes
/// `CALIBRATION_FLOOR_NS`, sizes a batch to take `target_ns` (at least
/// `MIN_BATCH_NS`), and grows it until the fastest of several runs of that
/// batch lasts `MIN_BATCH_NS`.
fn calibrate(run: &mut dyn FnMut(u64), target_ns: f64, now: &dyn Fn() -> u64) -> u64 {
    let mut n = 1u64;
    let per_update = loop {
        let elapsed = min_batch_ns(run, n, now);
        if elapsed >= CALIBRATION_FLOOR_NS || n >= 1 << 40 {
            break elapsed.max(1) as f64 / n as f64;
        }
        n *= 2;
    };
    let mut batch = ((target_ns.max(MIN_BATCH_NS as f64) / per_update) as u64).max(1);
    while min_batch_ns(run, batch, now) < MIN_BATCH_NS && batch < 1 << 40 {
        batch *= 2;
    }
    batch
}

/// Times the runners against each other. Each is calibrated to a batch of
/// `duration_s / samples` of clock time, warmed up, and then sampled
/// `cfg.samples` times, with the encodings interleaved in rotation. A sample
/// is the clock time of one batch divided by the batch size. `now` is the
/// clock in nanoseconds (thread CPU time in the bench, injected in tests).
/// Returns one `Timed` per runner, in runner order.
pub fn time_cells(runners: &mut [Runner], cfg: &TimingConfig, now: &dyn Fn() -> u64) -> Vec<Timed> {
    assert!(cfg.samples >= MIN_SAMPLES, "at least {MIN_SAMPLES} samples");
    let target_ns = cfg.duration_s * 1e9 / cfg.samples as f64;
    let batches: Vec<u64> = runners
        .iter_mut()
        .map(|r| calibrate(&mut r.run, target_ns, now))
        .collect();
    for _ in 0..cfg.warmup_batches {
        for (r, &n) in runners.iter_mut().zip(&batches) {
            (r.run)(n);
        }
    }
    let samples = collect_interleaved(cfg.samples, runners.len(), &mut |i| {
        let n = batches[i];
        let t0 = now();
        (runners[i].run)(n);
        now().saturating_sub(t0) as f64 / n as f64
    });
    runners
        .iter()
        .zip(&samples)
        .map(|(r, s)| Timed {
            encoding: r.encoding,
            op: r.op,
            stat: summarize(s),
        })
        .collect()
}

/// The result of a run paced at a fixed rate.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Paced {
    pub updates: u64,
    pub wall_s: f64,
    pub process_cpu_s: f64,
    /// Process CPU time over wall time, in percent of one core.
    pub measured_percent: f64,
    /// The figure derived from the unpaced per-update cost at the same rate.
    pub derived_percent: f64,
    /// `measured_percent - derived_percent`, in percentage points.
    pub difference: f64,
}

/// Builds a `Paced` from the raw readings of a paced run.
pub fn paced_summary(updates: u64, wall_ns: u64, cpu_ns: u64, derived_percent: f64) -> Paced {
    let measured_percent = cpu_ns as f64 / wall_ns as f64 * 100.0;
    Paced {
        updates,
        wall_s: wall_ns as f64 / 1e9,
        process_cpu_s: cpu_ns as f64 / 1e9,
        measured_percent,
        derived_percent,
        difference: measured_percent - derived_percent,
    }
}

/// Runs one update per tick at `rate_per_s` for `secs` of wall time, sleeping
/// to the next tick, and reads process CPU time over the run.
pub fn paced_run(
    run: &mut dyn FnMut(u64),
    rate_per_s: f64,
    secs: f64,
    derived_percent: f64,
) -> Paced {
    let total = (rate_per_s * secs).round() as u64;
    let cpu0 = process_cpu_ns();
    let start = Instant::now();
    for i in 0..total {
        let tick = start + Duration::from_secs_f64(i as f64 / rate_per_s);
        let now = Instant::now();
        if tick > now {
            std::thread::sleep(tick - now);
        }
        run(1);
    }
    let wall_ns = start.elapsed().as_nanos() as u64;
    let cpu_ns = process_cpu_ns() - cpu0;
    paced_summary(total, wall_ns, cpu_ns, derived_percent)
}
