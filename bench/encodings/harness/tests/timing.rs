use bench_stress::bench::stress::ItemListPayload;
use bench_stress::bench::stress::{Count, Item, Label};
use bench_veh_cluster::veh::common::Speed;
use encoding_bench::timing::*;
use encoding_bench::CountingAlloc;
use ridl_rt::encoding::{FlatBuffers, Podbuf, Proto3};
use std::cell::Cell;

#[global_allocator]
static ALLOC: CountingAlloc = CountingAlloc;

fn stat(median: f64, p05: f64, p95: f64) -> Stat {
    Stat {
        median_ns: median,
        p05_ns: p05,
        p95_ns: p95,
        min_ns: p05,
        max_ns: p95,
        reps: 30,
    }
}

#[test]
fn sustained_cpu_percent_converts_ns_per_update_at_a_rate() {
    assert_eq!(sustained_cpu_percent(250_000.0, 4000.0), 100.0);
    assert_eq!(sustained_cpu_percent(125_000.0, 4000.0), 50.0);
    assert_eq!(sustained_cpu_percent(1_000.0, 4000.0), 0.4);
    assert_eq!(sustained_cpu_percent(0.0, 4000.0), 0.0);
    assert_eq!(sustained_cpu_percent(1_000.0, 8000.0), 0.8);
}

#[test]
fn spread_above_ten_percent_is_flagged() {
    // (p95 - p05) / median = 0.2
    let wide = stat(100.0, 90.0, 110.0);
    assert!((wide.spread() - 0.2).abs() < 1e-12);
    assert!(wide.spread_flagged());
    // 0.05
    let narrow = stat(100.0, 98.0, 103.0);
    assert!(!narrow.spread_flagged());
    // just above and just below the 10 % limit
    assert!(stat(100.0, 95.0, 105.1).spread_flagged());
    assert!(!stat(100.0, 95.0, 104.9).spread_flagged());
}

#[test]
fn summarize_reports_median_and_percentiles_of_unsorted_samples() {
    let mut samples: Vec<f64> = (1..=101).map(f64::from).collect();
    samples.reverse();
    let s = summarize(&samples);
    assert_eq!(s.median_ns, 51.0);
    assert_eq!(s.p05_ns, 6.0);
    assert_eq!(s.p95_ns, 96.0);
    assert_eq!(s.reps, 101);
    // an even count averages the middle pair
    assert_eq!(summarize(&[4.0, 1.0, 3.0, 2.0]).median_ns, 2.5);
}

#[test]
fn the_start_of_the_rotation_moves_by_one_per_sample() {
    assert_eq!(interleave_order(0, 3), [0, 1, 2]);
    assert_eq!(interleave_order(1, 3), [1, 2, 0]);
    assert_eq!(interleave_order(2, 3), [2, 0, 1]);
    assert_eq!(interleave_order(3, 3), [0, 1, 2]);
}

#[test]
fn collect_interleaved_visits_every_encoding_once_per_sample_in_rotation() {
    let mut calls = Vec::new();
    let mut counter = 0.0;
    let out = collect_interleaved(4, 3, &mut |enc| {
        calls.push(enc);
        counter += 1.0;
        counter
    });
    assert_eq!(calls, [0, 1, 2, 1, 2, 0, 2, 0, 1, 0, 1, 2]);
    // values land in the vector of the encoding that produced them, in sample order
    assert_eq!(out[0], [1.0, 6.0, 8.0, 10.0]);
    assert_eq!(out[1], [2.0, 4.0, 9.0, 11.0]);
    assert_eq!(out[2], [3.0, 5.0, 7.0, 12.0]);
}

/// A clock that only the runners advance, so the result does not depend on
/// real time: each runner costs a fixed number of nanoseconds per update.
#[test]
fn time_cells_divides_batch_time_by_batch_size_and_labels_by_runner() {
    let clock = Cell::new(0u64);
    let log = std::cell::RefCell::new(Vec::<&'static str>::new());
    let mk = |label: &'static str, cost: u64| Runner {
        encoding: label,
        op: Op::Encode,
        run: Box::new({
            let clock = &clock;
            let log = &log;
            move |n: u64| {
                log.borrow_mut().push(label);
                clock.set(clock.get() + n * cost);
            }
        }) as Box<dyn FnMut(u64) + '_>,
    };
    let mut runners = [mk("a", 100), mk("b", 200), mk("c", 300)];
    let cfg = TimingConfig {
        samples: 30,
        duration_s: 0.03,
        warmup_batches: 2,
    };
    let out = time_cells(&mut runners, &cfg, &|| clock.get());
    assert_eq!(
        out.iter().map(|t| t.encoding).collect::<Vec<_>>(),
        ["a", "b", "c"]
    );
    assert_eq!(out[0].stat.median_ns, 100.0);
    assert_eq!(out[1].stat.median_ns, 200.0);
    assert_eq!(out[2].stat.median_ns, 300.0);
    assert!(out.iter().all(|t| t.stat.reps == 30));
    assert!(out.iter().all(|t| !t.stat.spread_flagged()));

    // The last 90 batches are the measured samples: rotation, 30 samples.
    let log = log.borrow();
    let tail = &log[log.len() - 90..];
    let order = ["a", "b", "c"];
    for (i, chunk) in tail.chunks(3).enumerate() {
        let expect: Vec<_> = interleave_order(i, 3).iter().map(|&k| order[k]).collect();
        assert_eq!(chunk, expect, "sample {i}");
    }
}

#[test]
fn a_runner_carries_the_name_of_its_encoding() {
    let v = Speed::new(72.5).unwrap();
    assert_eq!(runner::<_, FlatBuffers>(&v).encoding, "flatbuffers");
    assert_eq!(runner::<_, Proto3>(&v).encoding, "proto3");
    assert_eq!(runner::<_, Podbuf>(&v).encoding, "podbuf");
}

#[test]
fn one_outlier_among_thirty_samples_is_flagged_though_p95_does_not_see_it() {
    let mut samples = vec![100.0; 29];
    samples.push(200.0);
    let s = summarize(&samples);
    assert_eq!(s.p95_ns, 100.0, "p95 does not include the maximum");
    assert_eq!(s.min_ns, 100.0);
    assert_eq!(s.max_ns, 200.0);
    assert!(s.spread_flagged());
    assert!(!summarize(&[100.0; 30]).spread_flagged());
    // a low outlier counts too, and a range of exactly 5 % does not flag
    let mut low = vec![100.0; 29];
    low.push(50.0);
    assert!(summarize(&low).spread_flagged());
    let mut mild = vec![100.0; 29];
    mild.push(105.0);
    assert!(!summarize(&mild).spread_flagged());
}

type Log = std::cell::RefCell<Vec<u64>>;

/// A runner with a fixed cost per update on an injected clock. The first
/// call also costs `first_call_extra` ns (a disturbed first measurement).
fn costed<'a>(clock: &'a Cell<u64>, log: &'a Log, cost: u64, first_call_extra: u64) -> Runner<'a> {
    let mut first = true;
    Runner {
        encoding: "x",
        op: Op::Encode,
        run: Box::new(move |n: u64| {
            log.borrow_mut().push(n);
            let extra = if first { first_call_extra } else { 0 };
            first = false;
            clock.set(clock.get() + n * cost + extra);
        }),
    }
}

#[test]
fn a_slow_first_call_does_not_collapse_the_batch_size() {
    let clock = Cell::new(0u64);
    let log = Log::default();
    let mut runners = [costed(&clock, &log, 1, 1_000_000)];
    let cfg = TimingConfig {
        samples: 30,
        duration_s: 0.03,
        warmup_batches: 0,
    };
    let out = time_cells(&mut runners, &cfg, &|| clock.get());
    // 1 ns per update: a 1 ms sample batch is 1 000 000 updates. The
    // outlier must not shrink the batch to a handful of updates, and the
    // floor loop must not be what repairs it.
    let log = log.borrow();
    let sampled = &log[log.len() - 30..];
    assert!(sampled.iter().all(|&n| n >= 900_000), "{sampled:?}");
    assert_eq!(out[0].stat.median_ns, 1.0);
}

/// The cost per update falls from 10 ns to 1 ns after the 54 calls of the
/// size calibration (a runner that is faster once warm). The size estimate
/// then gives a batch of 20 000 updates that lasts 20 us; the batch must be
/// grown until it lasts at least 200 us.
#[test]
fn the_sampled_batch_lasts_at_least_the_floor() {
    let clock = Cell::new(0u64);
    let log = Log::default();
    let mut runners = [Runner {
        encoding: "x",
        op: Op::Encode,
        run: Box::new(|n: u64| {
            let calls = log.borrow().len();
            log.borrow_mut().push(n);
            let cost = if calls < 54 { 10 } else { 1 };
            clock.set(clock.get() + n * cost);
        }),
    }];
    let cfg = TimingConfig {
        samples: 30,
        duration_s: 0.003,
        warmup_batches: 0,
    };
    time_cells(&mut runners, &cfg, &|| clock.get());
    let log = log.borrow();
    let last = *log.last().unwrap();
    assert!(last >= 200_000, "batch of {last} updates lasts {last} ns");
}

#[test]
fn warm_up_batches_run_before_the_samples_and_are_not_sampled() {
    let calls = |warmup: u32| {
        let clock = Cell::new(0u64);
        let log = Log::default();
        let mut runners = [costed(&clock, &log, 5, 0), costed(&clock, &log, 5, 0)];
        let cfg = TimingConfig {
            samples: 30,
            duration_s: 0.03,
            warmup_batches: warmup,
        };
        let out = time_cells(&mut runners, &cfg, &|| clock.get());
        assert!(out.iter().all(|t| t.stat.reps == 30));
        let n = log.borrow().len();
        n
    };
    // two runners, so three warm-up batches each are six more calls
    assert_eq!(calls(3), calls(0) + 6);
    assert_eq!(calls(5), calls(0) + 10);
}

/// Thread CPU time does not advance while the thread sleeps. A wall clock
/// (CLOCK_MONOTONIC) would advance by 20 ms or more; the margin is 5 ms.
#[test]
fn thread_cpu_time_does_not_advance_during_a_sleep() {
    let t0 = thread_cpu_ns();
    std::thread::sleep(std::time::Duration::from_millis(20));
    let advanced = thread_cpu_ns() - t0;
    assert!(
        advanced < 5_000_000,
        "advanced {advanced} ns in a 20 ms sleep"
    );
}

#[test]
fn the_default_config_has_30_samples_and_3_warm_up_batches() {
    let cfg = TimingConfig::new(1.0);
    assert_eq!(cfg.samples, 30);
    assert_eq!(cfg.warmup_batches, 3);
    assert_eq!(cfg.duration_s, 1.0);
}

fn stat_of(median: f64, p05: f64, p95: f64, min: f64, max: f64, reps: usize) -> Stat {
    Stat {
        median_ns: median,
        p05_ns: p05,
        p95_ns: p95,
        min_ns: min,
        max_ns: max,
        reps,
    }
}

#[test]
fn the_sum_of_stats_adds_every_field_and_keeps_the_smallest_reps() {
    let a = stat_of(10.0, 9.0, 12.0, 8.0, 14.0, 30);
    let b = stat_of(20.0, 18.0, 25.0, 17.0, 27.0, 40);
    let c = stat_of(300.0, 280.0, 330.0, 270.0, 340.0, 35);
    let s = Stat::sum(&[a, b, c]);
    assert_eq!(s.median_ns, 330.0);
    assert_eq!(s.p05_ns, 307.0);
    assert_eq!(s.p95_ns, 367.0);
    assert_eq!(s.min_ns, 295.0);
    assert_eq!(s.max_ns, 381.0);
    assert_eq!(s.reps, 30);
}

/// Three operations of one encoding with distinct costs per operation: the
/// stat of each operation lands in its own field, and the per-update median
/// is the sum.
#[test]
fn each_operation_is_timed_on_its_own_and_its_label_is_kept() {
    let clock = Cell::new(0u64);
    let mk = |op: Op, cost: u64| Runner {
        encoding: "e",
        op,
        run: Box::new({
            let clock = &clock;
            move |n: u64| clock.set(clock.get() + n * cost)
        }) as Box<dyn FnMut(u64) + '_>,
    };
    // runners in a scrambled order, so the result is found by label
    let mut runners = [
        mk(Op::Decode, 300),
        mk(Op::Encode, 100),
        mk(Op::Verify, 200),
    ];
    let cfg = TimingConfig::new(0.03);
    let timed = time_cells(&mut runners, &cfg, &|| clock.get());
    let ops = OpStats::of(&timed, "e");
    assert_eq!(ops.encode.median_ns, 100.0);
    assert_eq!(ops.verify.median_ns, 200.0);
    assert_eq!(ops.decode.median_ns, 300.0);
    assert_eq!(ops.per_update().median_ns, 600.0);
    assert!(!ops.flagged());
}

#[test]
fn one_noisy_operation_flags_the_cell() {
    let quiet = stat_of(100.0, 100.0, 100.0, 100.0, 100.0, 30);
    let noisy = stat_of(100.0, 100.0, 100.0, 100.0, 200.0, 30);
    for k in 0..3 {
        let mut v = [quiet; 3];
        v[k] = noisy;
        let ops = OpStats {
            encode: v[0],
            verify: v[1],
            decode: v[2],
            access: None,
        };
        assert!(ops.flagged(), "operation {k}");
    }
}

#[test]
fn op_runners_are_labeled_with_the_encoding_and_the_operation_in_update_order() {
    let v = Speed::new(72.5).unwrap();
    let r = op_runners::<_, Proto3>(&v);
    assert_eq!(
        r.iter().map(|r| (r.encoding, r.op)).collect::<Vec<_>>(),
        [
            ("proto3", Op::Encode),
            ("proto3", Op::Verify),
            ("proto3", Op::Decode)
        ]
    );
    assert_eq!(OPS, [Op::Encode, Op::Verify, Op::Decode]);
    assert_eq!(runner::<_, Podbuf>(&v).op, Op::Update);
}

fn item_list() -> ItemListPayload {
    ItemListPayload {
        items: (0..16)
            .map(|i| Item {
                label: Label::new(format!("item-{i:02}")).unwrap(),
                count: Count::new(i * 1000).unwrap(),
            })
            .collect(),
    }
}

/// Allocations made by a batch of `n` of each operation. The counts are
/// exact per call (FlatBuffers: encode 1, verify 0, decode 17 for this
/// value), so a runner that repeats more than its own operation, or the
/// whole update, shows other numbers.
fn batch_allocs(runners: &mut [Runner; 3], n: u64) -> Vec<u64> {
    runners
        .iter_mut()
        .map(|r| {
            (r.run)(1); // the first call may allocate lazily
            CountingAlloc::reset();
            (r.run)(n);
            CountingAlloc::count()
        })
        .collect()
}

#[test]
fn a_batch_of_one_operation_repeats_only_that_operation() {
    let v = item_list();
    let mut fb = op_runners::<_, FlatBuffers>(&v);
    assert_eq!(batch_allocs(&mut fb, 5), [5, 0, 85]);
    let mut pb = op_runners::<_, Podbuf>(&v);
    assert_eq!(batch_allocs(&mut pb, 5), [0, 0, 85]);
    let mut p3 = op_runners::<_, Proto3>(&v);
    assert_eq!(batch_allocs(&mut p3, 5), [0, 0, 95]);
}

#[test]
fn a_noisy_access_flags_the_cell_and_is_not_part_of_the_per_update_figure() {
    let quiet = stat_of(100.0, 100.0, 100.0, 100.0, 100.0, 30);
    let noisy = stat_of(100.0, 100.0, 100.0, 100.0, 200.0, 30);
    let ops = OpStats {
        encode: quiet,
        verify: quiet,
        decode: quiet,
        access: Some(noisy),
    };
    assert!(ops.flagged());
    assert_eq!(ops.per_update().median_ns, 300.0);
}
