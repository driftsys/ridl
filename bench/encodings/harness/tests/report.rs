use bench_stress::bench::stress::ItemListPayload;
use bench_veh_cluster::veh::common::Speed;
use encoding_bench::corpus::{Cell, Kind, Recorder, Row, RowTiming};
use encoding_bench::report::{
    civil_date, collect_provenance, resolve_date, to_json, Provenance, RunInfo,
};
use encoding_bench::timing::{paced_summary, OpStats, Stat, TimingConfig};
use encoding_bench::{CountingAlloc, Counts};
use serde_json::Value;

#[global_allocator]
static ALLOC: CountingAlloc = CountingAlloc;

fn prov() -> Provenance {
    Provenance {
        date: "2026-11-05".into(),
        commit: "abc123".into(),
        os: "linux".into(),
        arch: "aarch64".into(),
        core_label: "A55".into(),
        toolchain: "rustc 1.88.0".into(),
        frequency_khz: Some(1_800_000),
        governor: Some("performance".into()),
    }
}

fn info(paced: bool) -> RunInfo {
    RunInfo {
        rate_per_s: 4000.0,
        samples: 30,
        duration_s: 1.0,
        paced,
        vary: false,
    }
}

fn measured_row(encoding: &'static str, median: f64, p05: f64, p95: f64) -> Row {
    Row {
        payload: "veh.cluster.VehicleStatus.currentSpeed",
        payload_type: "Speed",
        kind: Kind::Signal,
        encoding,
        cell: Cell::Measured(Counts {
            size: 24,
            encode_allocs: 1,
            verify_allocs: 2,
            decode_allocs: 3,
        }),
        timing: Some(timing_of(median, p05, p95)),
    }
}

/// A timing whose operations are one fifth (encode), one fifth (verify) and
/// three fifths (decode) of the per-update figures, so that the per-update
/// stat is the sum of the three.
fn timing_of(median: f64, p05: f64, p95: f64) -> RowTiming {
    let part = |k: f64| Stat {
        median_ns: median * k / 5.0,
        p05_ns: p05 * k / 5.0,
        p95_ns: p95 * k / 5.0,
        min_ns: p05 * k / 5.0,
        max_ns: p95 * k / 5.0,
        reps: 30,
    };
    let ops = OpStats {
        encode: part(1.0),
        verify: part(1.0),
        decode: part(3.0),
        access: None,
    };
    RowTiming {
        stat: ops.per_update(),
        ops,
        paced: None,
    }
}

fn rows_of(v: &Value) -> &Vec<Value> {
    v["rows"].as_array().expect("rows array")
}

#[test]
fn provenance_and_run_info_are_in_the_json() {
    let v = to_json(&prov(), &info(false), &[]);
    let p = &v["provenance"];
    assert_eq!(p["date"], "2026-11-05");
    assert_eq!(p["commit"], "abc123");
    assert_eq!(p["platform"]["os"], "linux");
    assert_eq!(p["platform"]["arch"], "aarch64");
    assert_eq!(p["core_label"], "A55");
    assert_eq!(p["toolchain"], "rustc 1.88.0");
    assert_eq!(p["frequency_khz"], 1_800_000);
    assert_eq!(p["governor"], "performance");
    assert_eq!(v["run"]["rate_per_s"], 4000.0);
    assert_eq!(v["run"]["samples"], 30);
    assert_eq!(v["run"]["paced"], false);
    assert_eq!(v["run"]["vary"], false);
    let mut unreadable = prov();
    unreadable.frequency_khz = None;
    unreadable.governor = None;
    let v = to_json(&unreadable, &info(false), &[]);
    assert!(v["provenance"]["frequency_khz"].is_null());
    assert!(v["provenance"]["governor"].is_null());
}

#[test]
fn civil_date_formats_unix_time_as_a_utc_date() {
    assert_eq!(civil_date(0), "1970-01-01");
    assert_eq!(civil_date(951_782_400), "2000-02-29");
    assert_eq!(civil_date(1_793_836_800), "2026-11-05");
}

#[test]
fn a_measured_row_carries_counts_timing_and_the_derived_load() {
    let rows = [measured_row("proto3", 125_000.0, 100_000.0, 150_000.0)];
    let v = to_json(&prov(), &info(false), &rows);
    let r = &rows_of(&v)[0];
    assert_eq!(r["payload"], "veh.cluster.VehicleStatus.currentSpeed");
    assert_eq!(r["payload_type"], "Speed");
    assert_eq!(r["kind"], "signal");
    assert_eq!(r["encoding"], "proto3");
    assert_eq!(r["status"], "measured");
    assert_eq!(r["size"], 24);
    assert_eq!(r["encode_allocs"], 1);
    assert_eq!(r["verify_allocs"], 2);
    assert_eq!(r["decode_allocs"], 3);
    assert_eq!(r["per_update_ns"]["median"], 125_000.0);
    assert_eq!(r["per_update_ns"]["p05"], 100_000.0);
    assert_eq!(r["per_update_ns"]["p95"], 150_000.0);
    assert_eq!(r["per_update_ns"]["reps"], 30);
    assert_eq!(r["per_update_ns"]["min"], 100_000.0);
    assert_eq!(r["per_update_ns"]["max"], 150_000.0);
    assert_eq!(r["per_update_ns"]["range_spread"], 0.4);
    assert_eq!(r["per_update_ns"]["spread_flagged"], true);
    assert_eq!(r["cpu_percent_at_rate"], 50.0);
    assert!(r["paced"].is_null());
    // per operation: encode and verify a fifth each, decode three fifths
    let ops = &r["per_op_ns"];
    for (op, median, p05, p95, pct) in [
        ("encode", 25_000.0, 20_000.0, 30_000.0, 10.0),
        ("verify", 25_000.0, 20_000.0, 30_000.0, 10.0),
        ("decode", 75_000.0, 60_000.0, 90_000.0, 30.0),
    ] {
        let o = &ops[op];
        assert_eq!(o["median"], median, "{op}");
        assert_eq!(o["p05"], p05, "{op}");
        assert_eq!(o["p95"], p95, "{op}");
        assert_eq!(o["min"], p05, "{op}");
        assert_eq!(o["max"], p95, "{op}");
        assert_eq!(o["reps"], 30, "{op}");
        assert_eq!(o["range_spread"], 0.4, "{op}");
        assert_eq!(o["spread_flagged"], true, "{op}");
        assert_eq!(o["cpu_percent_at_rate"], pct, "{op}");
    }
    // the headline is the sum of the three medians
    assert_eq!(
        r["per_update_ns"]["median"].as_f64().unwrap(),
        25_000.0 + 25_000.0 + 75_000.0
    );
}

#[test]
fn a_quiet_cell_is_not_flagged_and_a_paced_run_is_reported_next_to_the_derived_figure() {
    let mut row = measured_row("podbuf", 1_000.0, 990.0, 1_010.0);
    let paced = paced_summary(4000, 1_000_000_000, 5_000_000, 0.4);
    row.timing.as_mut().unwrap().paced = Some(paced);
    let v = to_json(&prov(), &info(true), &[row]);
    let r = &rows_of(&v)[0];
    assert_eq!(r["per_update_ns"]["spread_flagged"], false);
    for op in ["encode", "verify", "decode"] {
        assert_eq!(r["per_op_ns"][op]["spread_flagged"], false, "{op}");
    }
    assert_eq!(r["paced"]["updates"], 4000);
    assert_eq!(r["paced"]["measured_percent"], 0.5);
    assert_eq!(r["paced"]["derived_percent"], 0.4);
    assert!((r["paced"]["difference"].as_f64().unwrap() - 0.1).abs() < 1e-9);
    assert_eq!(v["run"]["paced"], true);
}

#[test]
fn refused_and_not_built_rows_are_kept_with_their_reasons_and_no_timing() {
    let rows = [
        Row {
            payload: "bench.stress.Stress.itemList",
            payload_type: "ItemListPayload",
            kind: Kind::Stress,
            encoding: "flatbuffers",
            cell: Cell::Refused {
                reason: "encode: bad".into(),
            },
            timing: None,
        },
        Row {
            payload: "veh.cluster.WheelDiagnostics.wheelTicks",
            payload_type: "RawTickCount",
            kind: Kind::Signal,
            encoding: "podbuf",
            cell: Cell::NotBuilt {
                reason: "internal".into(),
            },
            timing: None,
        },
    ];
    let v = to_json(&prov(), &info(false), &rows);
    let out = rows_of(&v);
    assert_eq!(out.len(), 2);
    assert_eq!(out[0]["status"], "refused");
    assert_eq!(out[0]["reason"], "encode: bad");
    assert_eq!(out[0]["kind"], "stress");
    assert_eq!(out[0]["payload_type"], "ItemListPayload");
    assert_eq!(out[1]["status"], "not_built");
    assert_eq!(out[1]["reason"], "internal");
    assert_eq!(out[1]["payload_type"], "RawTickCount");
    for r in out {
        assert!(r["per_update_ns"].is_null());
        assert!(r["cpu_percent_at_rate"].is_null());
        assert!(r["size"].is_null());
    }
}

#[test]
fn the_recorder_times_only_measured_cells_and_labels_each_row() {
    let mut rec = Recorder {
        timing: Some(TimingConfig::new(0.03)),
        ..Recorder::default()
    };
    rec.record("a.b.c", Kind::Signal, &Speed::new(72.5).unwrap());
    // An empty list breaks the 1..16 count, so every encoding refuses it.
    rec.record("a.b.d", Kind::Stress, &ItemListPayload { items: vec![] });
    let labels: Vec<_> = rec.rows.iter().map(|r| (r.payload, r.encoding)).collect();
    assert_eq!(
        labels,
        [
            ("a.b.c", "flatbuffers"),
            ("a.b.c", "proto3"),
            ("a.b.c", "podbuf"),
            ("a.b.d", "flatbuffers"),
            ("a.b.d", "proto3"),
            ("a.b.d", "podbuf"),
        ]
    );
    for r in &rec.rows[..3] {
        let t = r.timing.expect("a measured cell is timed");
        assert_eq!(t.stat.reps, 30);
        assert!(t.paced.is_none());
        assert_eq!(r.payload_type, "Speed");
    }
    for r in &rec.rows[3..] {
        assert!(matches!(r.cell, Cell::Refused { .. }));
        assert!(r.timing.is_none());
        assert_eq!(r.payload_type, "ItemListPayload");
    }
}

#[test]
fn a_recorder_without_timing_config_times_nothing() {
    let mut rec = Recorder::default();
    rec.record("a.b.c", Kind::Signal, &Speed::new(72.5).unwrap());
    assert!(rec.rows.iter().all(|r| r.timing.is_none()));
}

#[test]
fn a_paced_recorder_runs_the_rate_for_the_duration_and_reports_next_to_the_derived_figure() {
    let mut rec = Recorder {
        timing: Some(TimingConfig::new(0.05)),
        paced: true,
        ..Recorder::default()
    };
    rec.record("a.b.c", Kind::Signal, &Speed::new(72.5).unwrap());
    for r in &rec.rows {
        let t = r.timing.expect("timed");
        let p = t.paced.expect("paced");
        // 4000 updates per second for 0.05 s
        assert_eq!(p.updates, 200);
        assert!(p.wall_s >= 0.049, "wall {}", p.wall_s);
        assert_eq!(
            p.derived_percent,
            encoding_bench::timing::sustained_cpu_percent(t.stat.median_ns, 4000.0)
        );
        assert!((p.difference - (p.measured_percent - p.derived_percent)).abs() < 1e-9);
    }
}

#[test]
fn the_payload_type_is_the_bare_type_name_for_buildable_and_not_built_rows() {
    let mut rec = Recorder::default();
    for entry in encoding_bench::corpus::corpus() {
        if entry.name == "veh.cluster.VehicleStatus.currentSpeed"
            || entry.name == "veh.cluster.WheelDiagnostics.wheelTicks"
        {
            (entry.run)(&mut rec);
        }
    }
    let types: Vec<_> = rec
        .rows
        .iter()
        .map(|r| (r.payload, r.payload_type))
        .collect();
    assert_eq!(types.len(), 6);
    for (payload, ty) in types {
        let expected = if payload.ends_with("currentSpeed") {
            "Speed"
        } else {
            "RawTickCount"
        };
        assert_eq!(ty, expected, "{payload}");
    }
}

#[test]
fn the_date_flag_overrides_the_clock() {
    // 2024-01-24 is the clock; the flag states another date.
    let clock = 1_706_054_400;
    assert_eq!(civil_date(clock), "2024-01-24");
    assert_eq!(resolve_date(Some("2001-02-03"), clock), "2001-02-03");
    assert_eq!(resolve_date(None, clock), "2024-01-24");
    let stated = collect_provenance("A55", Some("abc"), Some("2001-02-03"));
    assert_eq!(stated.date, "2001-02-03");
    let clocked = collect_provenance("A55", Some("abc"), None);
    assert_ne!(clocked.date, "2001-02-03");
    assert_eq!(clocked.date.len(), 10);
}

/// The per-update flag is raised when any operation is noisy, even when the
/// sum of the bounds is quiet.
#[test]
fn the_per_update_flag_follows_the_noisiest_operation() {
    let mut row = measured_row("podbuf", 1_000.0, 990.0, 1_010.0);
    // decode is three fifths of the update; give encode a 1.3x outlier
    let t = row.timing.as_mut().unwrap();
    t.ops.encode.max_ns = t.ops.encode.median_ns * 1.3;
    t.stat = t.ops.per_update();
    assert!(!t.stat.spread_flagged(), "the sum alone is mild");
    let v = to_json(&prov(), &info(false), &[row]);
    let r = &rows_of(&v)[0];
    assert_eq!(r["per_op_ns"]["encode"]["spread_flagged"], true);
    assert_eq!(r["per_op_ns"]["verify"]["spread_flagged"], false);
    assert_eq!(r["per_update_ns"]["spread_flagged"], true);
}

/// The headline stat of a timed row is the sum of the three operation
/// medians, whatever the clock read.
#[test]
fn a_timed_row_sums_its_operation_medians() {
    let mut rec = Recorder {
        timing: Some(TimingConfig::new(0.03)),
        ..Recorder::default()
    };
    rec.record("a.b.c", Kind::Signal, &Speed::new(72.5).unwrap());
    for r in &rec.rows {
        let t = r.timing.expect("timed");
        assert_eq!(
            t.stat.median_ns,
            t.ops.encode.median_ns + t.ops.verify.median_ns + t.ops.decode.median_ns
        );
        assert_eq!(t.stat, t.ops.per_update());
        assert_eq!(t.ops.encode.reps, 30);
        assert_eq!(t.ops.verify.reps, 30);
        assert_eq!(t.ops.decode.reps, 30);
    }
}

#[test]
fn the_result_file_records_whether_the_values_changed() {
    let mut run = info(false);
    run.vary = true;
    let v = to_json(&prov(), &run, &[]);
    assert_eq!(v["run"]["vary"], true);
}

fn item_list(n: usize) -> ItemListPayload {
    use bench_stress::bench::stress::{Count, Item, Label};
    ItemListPayload {
        items: (0..n)
            .map(|i| Item {
                label: Label::new(format!("i{i}")).unwrap(),
                count: Count::new(i as i64).unwrap(),
            })
            .collect(),
    }
}

static SEEN_VARY: std::sync::Mutex<Vec<u64>> = std::sync::Mutex::new(Vec::new());
static SEEN_FIXED: std::sync::Mutex<Vec<u64>> = std::sync::Mutex::new(Vec::new());

fn see_vary_fb(v: &bench_stress::bench::stress::ItemListPayloadFbView<'_>) -> u64 {
    let n = v.items().len() as u64;
    SEEN_VARY.lock().unwrap().push(n);
    n
}

fn see_vary_pb(v: &bench_stress::bench::stress::ItemListPayloadPodbufView<'_>) -> u64 {
    let n = v.items().count() as u64;
    SEEN_VARY.lock().unwrap().push(n);
    n
}

fn see_fixed_fb(v: &bench_stress::bench::stress::ItemListPayloadFbView<'_>) -> u64 {
    let n = v.items().len() as u64;
    SEEN_FIXED.lock().unwrap().push(n);
    n
}

fn see_fixed_pb(v: &bench_stress::bench::stress::ItemListPayloadPodbufView<'_>) -> u64 {
    let n = v.items().count() as u64;
    SEEN_FIXED.lock().unwrap().push(n);
    n
}

/// With `vary`, the access operation of both encodings reads every value of
/// the pool; the pool has items lists of 1, 2 and 3 elements.
#[test]
fn with_vary_the_access_operation_reads_every_value_of_the_pool() {
    use encoding_bench::timing::Accessors;
    let mut rec = Recorder {
        timing: Some(TimingConfig::new(0.03)),
        vary: true,
        ..Recorder::default()
    };
    let pool: Vec<_> = (1..=3).map(item_list).collect();
    let access = Accessors::<ItemListPayload> {
        flatbuffers: see_vary_fb,
        podbuf: see_vary_pb,
    };
    rec.record_pool_access("a.b.c", Kind::Stress, &pool, &access);
    let mut seen = SEEN_VARY.lock().unwrap().clone();
    seen.sort();
    seen.dedup();
    assert_eq!(seen, [1, 2, 3]);
    assert_eq!(rec.pool_sizes, [("a.b.c", 3)]);
}

#[test]
fn without_vary_the_access_operation_reads_the_first_value_only() {
    use encoding_bench::timing::Accessors;
    let mut rec = Recorder {
        timing: Some(TimingConfig::new(0.03)),
        ..Recorder::default()
    };
    let pool: Vec<_> = (1..=3).map(item_list).collect();
    let access = Accessors::<ItemListPayload> {
        flatbuffers: see_fixed_fb,
        podbuf: see_fixed_pb,
    };
    rec.record_pool_access("a.b.c", Kind::Stress, &pool, &access);
    let mut seen = SEEN_FIXED.lock().unwrap().clone();
    seen.sort();
    seen.dedup();
    assert_eq!(seen, [1]);
    assert_eq!(rec.pool_sizes, [("a.b.c", 1)]);
}

#[test]
fn without_vary_the_timed_operations_repeat_the_first_value_of_the_pool() {
    let mut rec = Recorder {
        timing: Some(TimingConfig::new(0.03)),
        ..Recorder::default()
    };
    rec.record_pool("a.b.c", Kind::Stress, &[item_list(3), item_list(0)]);
    assert!(rec.rows.iter().all(|r| r.timing.is_some()));
}

#[test]
fn a_varied_pool_of_distinct_values_is_timed() {
    let mut rec = Recorder {
        timing: Some(TimingConfig::new(0.03)),
        vary: true,
        ..Recorder::default()
    };
    let pool: Vec<_> = (1..=8).map(item_list).collect();
    rec.record_pool("a.b.c", Kind::Stress, &pool);
    assert!(rec.rows.iter().all(|r| r.timing.is_some()));
}

#[test]
fn the_access_operation_is_timed_for_encodings_with_accessors_only() {
    use encoding_bench::timing::{Accessors, Op};
    let mut rec = Recorder {
        timing: Some(TimingConfig::new(0.03)),
        ..Recorder::default()
    };
    let access = Accessors::<ItemListPayload> {
        flatbuffers: |v| v.items().len() as u64,
        podbuf: |v| v.items().count() as u64,
    };
    rec.record_pool_access("a.b.c", Kind::Stress, &[item_list(3)], &access);
    for r in &rec.rows {
        let t = r.timing.expect("timed");
        match r.encoding {
            "proto3" => assert!(t.ops.access.is_none()),
            _ => assert!(t.ops.access.is_some(), "{}", r.encoding),
        }
    }
    assert_eq!(Op::Access.name(), "access");
}

#[test]
fn the_access_stat_is_in_the_json_only_when_it_was_timed() {
    let mut with = measured_row("flatbuffers", 100.0, 90.0, 110.0);
    let t = with.timing.as_mut().unwrap();
    t.ops.access = Some(t.ops.encode);
    let without = measured_row("proto3", 100.0, 90.0, 110.0);
    let v = to_json(&prov(), &info(false), &[with, without]);
    let out = rows_of(&v);
    assert!(out[0]["per_op_ns"]["access"]["median"].is_number());
    assert!(out[1]["per_op_ns"]["access"].is_null());
}
