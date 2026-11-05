use bench_veh_cluster::veh::common::Speed;
use encoding_bench::timing::*;
use ridl_rt::encoding::{FlatBuffers, Podbuf, Proto3};
use std::cell::Cell;

fn stat(median: f64, p05: f64, p95: f64) -> Stat {
    Stat {
        median_ns: median,
        p05_ns: p05,
        p95_ns: p95,
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
        out.iter().map(|(l, _)| *l).collect::<Vec<_>>(),
        ["a", "b", "c"]
    );
    assert_eq!(out[0].1.median_ns, 100.0);
    assert_eq!(out[1].1.median_ns, 200.0);
    assert_eq!(out[2].1.median_ns, 300.0);
    assert!(out.iter().all(|(_, s)| s.reps == 30));
    assert!(out.iter().all(|(_, s)| !s.spread_flagged()));

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
