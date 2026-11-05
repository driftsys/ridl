//! Tests of the report tables on injected result JSON. No real timing.

use encoding_bench_report::*;
use serde_json::{json, Value};

fn measured(
    payload: &str,
    ty: &str,
    enc: &str,
    size: u64,
    ns: f64,
    flagged: bool,
    cpu: f64,
) -> Value {
    json!({
        "payload": payload, "payload_type": ty, "kind": "signal", "encoding": enc,
        "status": "measured", "size": size,
        "encode_allocs": 1, "verify_allocs": 2, "decode_allocs": 3,
        "per_update_ns": {
            "median": ns, "p05": ns, "p95": ns, "min": ns - 1.0, "max": ns + 1.0,
            "range_spread": 0.05, "reps": 30, "spread": 0.02, "spread_flagged": flagged
        },
        "cpu_percent_at_rate": cpu,
        "paced": { "updates": 10, "wall_s": 1.0, "process_cpu_s": 0.01,
                   "measured_percent": 0.96, "derived_percent": cpu, "difference": 0.9 }
    })
}

fn refused(payload: &str, ty: &str, enc: &str) -> Value {
    json!({
        "payload": payload, "payload_type": ty, "kind": "signal", "encoding": enc,
        "status": "refused", "reason": "the encoding cannot carry it"
    })
}

fn not_built(payload: &str, ty: &str, enc: &str) -> Value {
    json!({
        "payload": payload, "payload_type": ty, "kind": "signal", "encoding": enc,
        "status": "not_built", "reason": "the payload type is internal"
    })
}

fn file(rows: Vec<Value>) -> ResultFile {
    let v = json!({
        "provenance": {
            "date": "2026-11-05", "commit": "abc123", "platform": {"os": "linux", "arch": "aarch64"},
            "core_label": "A55", "toolchain": "rustc 1.98.1", "frequency_khz": 1800000,
            "governor": "schedutil"
        },
        "run": { "rate_per_s": 4000.0, "samples": 30, "duration_s": 2.0, "paced": true },
        "rows": rows
    });
    parse(&v.to_string()).expect("fixture parses")
}

/// One signal measured in all three encodings.
fn trio(payload: &str, ty: &str, sizes: [u64; 3], ns: [f64; 3]) -> Vec<Value> {
    ["flatbuffers", "proto3", "podbuf"]
        .iter()
        .enumerate()
        .map(|(i, e)| measured(payload, ty, e, sizes[i], ns[i], false, ns[i] / 2500.0))
        .collect()
}

/// The line of `table` that starts with the cell text `first`.
fn line_of<'a>(table: &'a str, first: &str) -> &'a str {
    table
        .lines()
        .find(|l| l.starts_with(&format!("| {first} ")))
        .unwrap_or_else(|| panic!("no row {first} in\n{table}"))
}

fn cells(line: &str) -> Vec<String> {
    line.trim_matches('|')
        .split('|')
        .map(|c| c.trim().to_string())
        .collect()
}

#[test]
fn ratio_table_divides_by_the_podbuf_column_and_marks_refused_cells() {
    let mut rows = trio("p.A.x", "Speed", [24, 30, 12], [40.0, 30.0, 20.0]);
    rows.extend([
        measured("p.A.y", "Temp", "flatbuffers", 8, 10.0, false, 0.01),
        refused("p.A.y", "Temp", "proto3"),
        measured("p.A.y", "Temp", "podbuf", 4, 5.0, false, 0.005),
    ]);
    rows.extend([
        measured("p.A.z", "Gear", "flatbuffers", 8, 10.0, false, 0.01),
        measured("p.A.z", "Gear", "proto3", 8, 10.0, false, 0.01),
        refused("p.A.z", "Gear", "podbuf"),
    ]);
    let t = ratio_table(&file(rows));
    // Header names the two numerators, each divided by podbuf.
    let header = cells(t.lines().find(|l| l.starts_with("| Payload")).unwrap());
    assert_eq!(header[1], "Size: FlatBuffers / podbuf");
    assert_eq!(header[2], "Size: proto3 / podbuf");
    assert_eq!(header[3], "Time: FlatBuffers / podbuf");
    assert_eq!(header[4], "Time: proto3 / podbuf");
    assert_eq!(header[5], "CPU: FlatBuffers / podbuf");
    assert_eq!(header[6], "CPU: proto3 / podbuf");
    let x = cells(line_of(&t, "p.A.x"));
    assert_eq!(&x[1..5], ["2.00", "2.50", "2.00", "1.50"]);
    let y = cells(line_of(&t, "p.A.y"));
    assert_eq!(y[1], "2.00");
    assert_eq!(y[2], "refused");
    assert_eq!(y[4], "refused");
    let z = cells(line_of(&t, "p.A.z"));
    assert_eq!(z[1], "podbuf refused");
    assert_eq!(z[2], "podbuf refused");
    // No ratio has a refused cell in it.
    assert!(!y.iter().any(|c| c == "NaN" || c == "inf"));
}

#[test]
fn not_built_rows_are_listed_apart_from_refused_rows_and_marked_in_ratios() {
    let mut rows = trio("p.A.x", "Speed", [24, 30, 12], [40.0, 30.0, 20.0]);
    for e in ["flatbuffers", "proto3", "podbuf"] {
        rows.push(not_built("p.B.raw", "RawTickCount", e));
    }
    rows.push(refused("p.C.r", "Big", "proto3"));
    rows.push(measured(
        "p.C.r",
        "Big",
        "flatbuffers",
        8,
        10.0,
        false,
        0.01,
    ));
    rows.push(measured("p.C.r", "Big", "podbuf", 4, 5.0, false, 0.005));
    let r = file(rows);

    let nb = not_built_table(&r);
    assert!(nb.contains("p.B.raw"), "{nb}");
    assert!(nb.contains("the payload type is internal"));
    assert!(!nb.contains("p.C.r"), "a refused payload is not not-built");
    assert!(!nb.contains("p.A.x"));
    // One line per payload, not one per encoding.
    assert_eq!(nb.matches("p.B.raw").count(), 1);

    let rf = refused_table(&r);
    assert!(rf.contains("p.C.r"), "{rf}");
    assert!(rf.contains("proto3"));
    assert!(rf.contains("the encoding cannot carry it"));
    assert!(
        !rf.contains("p.B.raw"),
        "a not-built payload is not refused"
    );

    let ratios = ratio_table(&r);
    let raw = cells(line_of(&ratios, "p.B.raw"));
    assert!(raw[1..].iter().all(|c| c == "not built"), "{raw:?}");
    assert!(!raw.iter().any(|c| c == "refused"));
    let c = cells(line_of(&ratios, "p.C.r"));
    assert_eq!(c[2], "refused");
}

#[test]
fn the_mix_weights_each_signal_once_or_each_payload_type_once() {
    // Type A has three signals of size 10 in podbuf, type B one signal of size 40.
    let mut rows = Vec::new();
    for s in ["p.s1", "p.s2", "p.s3"] {
        rows.extend(trio(s, "A", [20, 20, 10], [10.0, 10.0, 10.0]));
    }
    rows.extend(trio("p.s4", "B", [80, 80, 40], [10.0, 10.0, 10.0]));
    let r = file(rows);

    let per_signal = mix_table(&r, Weighting::PerSignal);
    let size = cells(line_of(&per_signal, "Size (bytes)"));
    // (10 + 10 + 10 + 40) / 4 = 17.5 for podbuf; (20*3 + 80) / 4 = 35.
    assert_eq!(&size[1..4], ["35.0", "35.0", "17.5"], "{per_signal}");
    assert_eq!(size[6], "4");

    let per_type = mix_table(&r, Weighting::PerPayloadType);
    let size = cells(line_of(&per_type, "Size (bytes)"));
    // (10 + 40) / 2 = 25 for podbuf; (20 + 80) / 2 = 50.
    assert_eq!(&size[1..4], ["50.0", "50.0", "25.0"], "{per_type}");
    assert_eq!(size[6], "2");

    // Each caption states its weighting.
    assert!(
        per_signal.contains("each signal counts once"),
        "{per_signal}"
    );
    assert!(
        per_type.contains("each payload type counts once"),
        "{per_type}"
    );
}

#[test]
fn the_mix_leaves_out_a_unit_with_a_refused_or_not_built_cell() {
    let mut rows = trio("p.s1", "A", [20, 20, 10], [10.0, 10.0, 10.0]);
    rows.extend([
        measured("p.s2", "B", "flatbuffers", 100, 10.0, false, 0.01),
        refused("p.s2", "B", "proto3"),
        measured("p.s2", "B", "podbuf", 100, 10.0, false, 0.01),
    ]);
    for e in ["flatbuffers", "proto3", "podbuf"] {
        rows.push(not_built("p.s3", "C", e));
    }
    // Type D: one signal fully measured, one with a refused cell.
    rows.extend(trio("p.s5", "D", [1000, 1000, 1000], [10.0, 10.0, 10.0]));
    rows.extend([
        measured("p.s6", "D", "flatbuffers", 100, 10.0, false, 0.01),
        refused("p.s6", "D", "proto3"),
        measured("p.s6", "D", "podbuf", 100, 10.0, false, 0.01),
    ]);
    let r = file(rows);
    // Per signal: p.s1 and p.s5 are complete. Per type: only A is complete.
    let t = mix_table(&r, Weighting::PerSignal);
    let size = cells(line_of(&t, "Size (bytes)"));
    assert_eq!(&size[1..4], ["510.0", "510.0", "505.0"], "{t}");
    assert_eq!(size[6], "2");
    let t = mix_table(&r, Weighting::PerPayloadType);
    let size = cells(line_of(&t, "Size (bytes)"));
    assert_eq!(&size[1..4], ["20.0", "20.0", "10.0"], "{t}");
    assert_eq!(size[6], "1");
}

#[test]
fn column_labels_name_the_encoding_of_each_column() {
    // Distinct sizes per encoding: FlatBuffers 1, proto3 2, podbuf 3.
    let r = file(trio("p.A.x", "Speed", [1, 2, 3], [10.0, 20.0, 30.0]));
    for table in [
        size_table(&r),
        time_table(&r),
        cpu_table(&r),
        alloc_table(&r),
    ] {
        let header = cells(table.lines().find(|l| l.starts_with("| Payload")).unwrap());
        assert_eq!(
            &header[1..4],
            ["FlatBuffers", "proto3", "podbuf"],
            "{table}"
        );
    }
    let size = cells(line_of(&size_table(&r), "p.A.x"));
    assert_eq!(&size[1..4], ["1", "2", "3"]);
    let time = cells(line_of(&time_table(&r), "p.A.x"));
    assert!(time[1].starts_with("10.0"), "{time:?}");
    assert!(time[2].starts_with("20.0"));
    assert!(time[3].starts_with("30.0"));
    let alloc = cells(line_of(&alloc_table(&r), "p.A.x"));
    assert_eq!(alloc[1], "1 / 2 / 3");
}

#[test]
fn a_flagged_spread_is_marked_and_an_unflagged_one_is_not() {
    let rows = vec![
        measured("p.A.x", "Speed", "flatbuffers", 1, 10.0, true, 0.1),
        measured("p.A.x", "Speed", "proto3", 1, 20.0, false, 0.2),
        measured("p.A.x", "Speed", "podbuf", 1, 30.0, true, 0.3),
    ];
    let t = time_table(&file(rows));
    let x = cells(line_of(&t, "p.A.x"));
    assert_eq!(x[1], "10.0 (9.0 to 11.0; 5 %) \u{2020}", "{t}");
    assert_eq!(x[2], "20.0 (19.0 to 21.0; 5 %)");
    assert!(x[3].ends_with('\u{2020}'));
    assert!(t.contains("\u{2020} spread above 10 %"), "{t}");
}

#[test]
fn the_cpu_table_shows_the_derived_figure_and_the_paced_table_both_with_the_difference() {
    let r = file(trio("p.A.x", "Speed", [1, 2, 3], [40.0, 40.0, 40.0]));
    let cpu = cells(line_of(&cpu_table(&r), "p.A.x"));
    assert_eq!(cpu[1], "0.0160");
    let paced = paced_table(&r);
    let row = cells(
        paced
            .lines()
            .find(|l| l.starts_with("| p.A.x ") && l.contains("FlatBuffers"))
            .unwrap_or_else(|| panic!("{paced}")),
    );
    assert_eq!(&row[2..5], ["0.9600", "0.0160", "0.9000"], "{paced}");
}

#[test]
fn the_report_states_provenance_and_orders_the_sections() {
    let r = file(trio("p.A.x", "Speed", [1, 2, 3], [40.0, 40.0, 40.0]));
    let out = render("result.json", &r);
    for needle in [
        "abc123",
        "2026-11-05",
        "A55",
        "linux",
        "schedutil",
        "1800000",
        "rustc 1.98.1",
    ] {
        assert!(out.contains(needle), "missing {needle}");
    }
    let at = |s: &str| out.find(s).unwrap_or_else(|| panic!("missing section {s}"));
    assert!(at("## Mix") < at("## Per payload"));
    assert!(at("## Per payload") < at("## Payloads not built"));
}

#[test]
fn parse_rejects_an_unknown_encoding_label() {
    let v = json!({
        "provenance": {"date":"d","commit":"c","platform":{"os":"o","arch":"a"},
                       "core_label":"x","toolchain":"t","frequency_khz":null,"governor":null},
        "run": {"rate_per_s": 4000.0, "samples": 30, "duration_s": 1.0, "paced": false},
        "rows": [ measured("p.A.x", "T", "capnp", 1, 1.0, false, 0.1) ]
    });
    assert!(parse(&v.to_string()).is_err());
}

/// A stress row: like `measured`, with kind `stress`.
fn stress_trio(payload: &str, ty: &str, size: u64) -> Vec<Value> {
    trio(payload, ty, [size; 3], [10.0; 3])
        .into_iter()
        .map(|mut v| {
            v["kind"] = json!("stress");
            v
        })
        .collect()
}

#[test]
fn stress_rows_are_in_the_payload_tables_and_in_neither_mix() {
    let mut rows = trio("p.s1", "A", [20, 20, 10], [10.0, 10.0, 10.0]);
    rows.extend(stress_trio("p.stress", "Big", 1000));
    let r = file(rows);
    for w in [Weighting::PerSignal, Weighting::PerPayloadType] {
        let t = mix_table(&r, w);
        let size = cells(line_of(&t, "Size (bytes)"));
        assert_eq!(&size[1..4], ["20.0", "20.0", "10.0"], "{t}");
        assert_eq!(size[6], "1", "{t}");
    }
    let sizes = cells(line_of(&size_table(&r), "p.stress"));
    assert_eq!(&sizes[1..4], ["1000", "1000", "1000"]);
    assert!(ratio_table(&r).contains("| p.stress "));
    assert!(time_table(&r).contains("| p.stress "));
}

#[test]
fn each_mix_caption_names_its_own_weighting_only() {
    let r = file(trio("p.s1", "A", [20, 20, 10], [10.0, 10.0, 10.0]));
    let per_signal = mix_table(&r, Weighting::PerSignal);
    let per_type = mix_table(&r, Weighting::PerPayloadType);
    assert!(per_signal.contains("per signal: each signal counts once"));
    assert!(!per_signal.contains("per payload type"));
    assert!(per_type.contains("per payload type: each payload type counts once"));
    assert!(!per_type.contains("per signal"));
}

#[test]
fn the_allocation_mix_sums_encode_verify_and_decode() {
    // The fixture counts are 1, 2 and 3 for every cell.
    let r = file(trio("p.s1", "A", [20, 20, 10], [10.0, 10.0, 10.0]));
    for w in [Weighting::PerSignal, Weighting::PerPayloadType] {
        let t = mix_table(&r, w);
        let row = cells(line_of(
            &t,
            "Heap allocations per update (encode + verify + decode)",
        ));
        assert_eq!(&row[1..4], ["6.0", "6.0", "6.0"], "{t}");
    }
}
