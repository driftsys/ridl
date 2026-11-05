use bench_stress::bench::stress::ItemListPayload;
use bench_veh_cluster::veh::common::Speed;
use encoding_bench::corpus::{Cell, Kind, Recorder, Row, RowTiming};
use encoding_bench::report::{
    civil_date, collect_provenance, resolve_date, to_json, Provenance, RunInfo,
};
use encoding_bench::timing::{paced_summary, Stat, TimingConfig};
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
        timing: Some(RowTiming {
            stat: Stat {
                median_ns: median,
                p05_ns: p05,
                p95_ns: p95,
                min_ns: p05,
                max_ns: p95,
                reps: 30,
            },
            paced: None,
        }),
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
}

#[test]
fn a_quiet_cell_is_not_flagged_and_a_paced_run_is_reported_next_to_the_derived_figure() {
    let mut row = measured_row("podbuf", 1_000.0, 990.0, 1_010.0);
    let paced = paced_summary(4000, 1_000_000_000, 5_000_000, 0.4);
    row.timing.as_mut().unwrap().paced = Some(paced);
    let v = to_json(&prov(), &info(true), &[row]);
    let r = &rows_of(&v)[0];
    assert_eq!(r["per_update_ns"]["spread_flagged"], false);
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
