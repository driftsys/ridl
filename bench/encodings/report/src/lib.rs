//! Reads the result JSON of the encoding bench and writes Markdown tables.

use serde_json::Value;

/// The encodings of the bench, in column order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Encoding {
    Flatbuffers,
    Proto3,
    Podbuf,
}

impl Encoding {
    pub const ALL: [Encoding; 3] = [Encoding::Flatbuffers, Encoding::Proto3, Encoding::Podbuf];

    /// The label in the JSON file.
    pub fn json_label(self) -> &'static str {
        match self {
            Encoding::Flatbuffers => "flatbuffers",
            Encoding::Proto3 => "proto3",
            Encoding::Podbuf => "podbuf",
        }
    }

    /// The label in a table header.
    pub fn column_label(self) -> &'static str {
        match self {
            Encoding::Flatbuffers => "FlatBuffers",
            Encoding::Proto3 => "proto3",
            Encoding::Podbuf => "podbuf",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Timing {
    pub median: f64,
    pub min: f64,
    pub max: f64,
    pub range_spread: f64,
    pub flagged: bool,
}

/// The operations of one update, in column order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    Encode,
    Verify,
    Decode,
}

impl Op {
    pub const ALL: [Op; 3] = [Op::Encode, Op::Verify, Op::Decode];

    /// The key in `per_op_ns`.
    pub fn json_key(self) -> &'static str {
        match self {
            Op::Encode => "encode",
            Op::Verify => "verify",
            Op::Decode => "decode",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Op::Encode => "Encode",
            Op::Verify => "Verify",
            Op::Decode => "Decode",
        }
    }

    fn index(self) -> usize {
        self as usize
    }
}

/// The timing of one operation of one cell.
#[derive(Debug, Clone)]
pub struct OpTiming {
    pub timing: Timing,
    pub cpu_percent: f64,
}

#[derive(Debug, Clone)]
pub struct Paced {
    pub measured_percent: f64,
    pub derived_percent: f64,
    pub difference: f64,
}

#[derive(Debug, Clone)]
pub struct Measured {
    pub size: f64,
    pub encode_allocs: f64,
    pub verify_allocs: f64,
    pub decode_allocs: f64,
    pub timing: Option<Timing>,
    pub cpu_percent: Option<f64>,
    /// Encode, verify and decode apart, in `Op::ALL` order.
    pub ops: Option<Box<[OpTiming; 3]>>,
    pub paced: Option<Paced>,
}

#[derive(Debug, Clone)]
pub enum Cell {
    Measured(Measured),
    Refused(String),
    NotBuilt(String),
}

#[derive(Debug, Clone)]
pub struct Row {
    pub payload: String,
    pub payload_type: String,
    pub is_signal: bool,
    pub encoding: Encoding,
    pub cell: Cell,
}

#[derive(Debug, Clone)]
pub struct ResultFile {
    pub date: String,
    pub commit: String,
    pub platform: String,
    pub core_label: String,
    pub toolchain: String,
    pub frequency_khz: Option<u64>,
    pub governor: Option<String>,
    pub rate_per_s: f64,
    pub samples: u64,
    pub duration_s: f64,
    pub paced: bool,
    /// Whether the run cycled through a pool of values; absent in a file
    /// written before the flag existed, which then reads as false.
    pub vary: bool,
    pub rows: Vec<Row>,
}

fn num(v: &Value, key: &str) -> Result<f64, String> {
    v[key]
        .as_f64()
        .ok_or_else(|| format!("missing or non-numeric `{key}`"))
}

fn text(v: &Value, key: &str) -> Result<String, String> {
    v[key]
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| format!("missing or non-text `{key}`"))
}

fn parse_timing(t: &Value) -> Result<Timing, String> {
    Ok(Timing {
        median: num(t, "median")?,
        min: num(t, "min")?,
        max: num(t, "max")?,
        range_spread: num(t, "range_spread")?,
        flagged: t["spread_flagged"]
            .as_bool()
            .ok_or("missing `spread_flagged`")?,
    })
}

fn parse_row(v: &Value) -> Result<Row, String> {
    let payload = text(v, "payload")?;
    let label = text(v, "encoding")?;
    let encoding = Encoding::ALL
        .into_iter()
        .find(|e| e.json_label() == label)
        .ok_or_else(|| format!("{payload}: unknown encoding `{label}`"))?;
    let is_signal = match text(v, "kind")?.as_str() {
        "signal" => true,
        "stress" => false,
        other => return Err(format!("{payload}: unknown kind `{other}`")),
    };
    let cell = match text(v, "status")?.as_str() {
        "measured" => {
            let timing = match v.get("per_update_ns") {
                Some(t) => Some(parse_timing(t)?),
                None => None,
            };
            let ops = match v.get("per_op_ns") {
                Some(o) => {
                    let mut out = Vec::new();
                    for op in Op::ALL {
                        let t = o
                            .get(op.json_key())
                            .ok_or_else(|| format!("{payload}: no `{}` timing", op.json_key()))?;
                        out.push(OpTiming {
                            timing: parse_timing(t)?,
                            cpu_percent: num(t, "cpu_percent_at_rate")?,
                        });
                    }
                    out.try_into().ok().map(Box::new)
                }
                None => None,
            };
            let paced = match v.get("paced") {
                Some(p) => Some(Paced {
                    measured_percent: num(p, "measured_percent")?,
                    derived_percent: num(p, "derived_percent")?,
                    difference: num(p, "difference")?,
                }),
                None => None,
            };
            Cell::Measured(Measured {
                size: num(v, "size")?,
                encode_allocs: num(v, "encode_allocs")?,
                verify_allocs: num(v, "verify_allocs")?,
                decode_allocs: num(v, "decode_allocs")?,
                timing,
                ops,
                cpu_percent: v["cpu_percent_at_rate"].as_f64(),
                paced,
            })
        }
        "refused" => Cell::Refused(text(v, "reason")?),
        "not_built" => Cell::NotBuilt(text(v, "reason")?),
        other => return Err(format!("{payload}: unknown status `{other}`")),
    };
    Ok(Row {
        payload,
        payload_type: text(v, "payload_type")?,
        is_signal,
        encoding,
        cell,
    })
}

pub fn parse(text_in: &str) -> Result<ResultFile, String> {
    let v: Value = serde_json::from_str(text_in).map_err(|e| format!("not JSON: {e}"))?;
    let p = &v["provenance"];
    let run = &v["run"];
    let rows = v["rows"]
        .as_array()
        .ok_or("missing `rows`")?
        .iter()
        .map(parse_row)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(ResultFile {
        date: text(p, "date")?,
        commit: text(p, "commit")?,
        platform: format!(
            "{} {}",
            text(&p["platform"], "os")?,
            text(&p["platform"], "arch")?
        ),
        core_label: text(p, "core_label")?,
        toolchain: text(p, "toolchain")?,
        frequency_khz: p["frequency_khz"].as_u64(),
        governor: p["governor"].as_str().map(str::to_string),
        rate_per_s: num(run, "rate_per_s")?,
        samples: run["samples"].as_u64().ok_or("missing `samples`")?,
        duration_s: num(run, "duration_s")?,
        paced: run["paced"].as_bool().ok_or("missing `paced`")?,
        vary: run["vary"].as_bool().unwrap_or(false),
        rows,
    })
}

/// Which unit an equal-weight mix counts once.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Weighting {
    PerSignal,
    PerPayloadType,
}

/// A number a table reports for a measured cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Metric {
    Size,
    Allocs,
    Time,
    Cpu,
    OpTime(Op),
    OpCpu(Op),
    OpAllocs(Op),
}

impl Metric {
    fn value(self, m: &Measured) -> Option<f64> {
        match self {
            Metric::Size => Some(m.size),
            Metric::Allocs => Some(m.encode_allocs + m.verify_allocs + m.decode_allocs),
            Metric::Time => m.timing.as_ref().map(|t| t.median),
            Metric::Cpu => m.cpu_percent,
            Metric::OpTime(op) => m.ops.as_ref().map(|o| o[op.index()].timing.median),
            Metric::OpCpu(op) => m.ops.as_ref().map(|o| o[op.index()].cpu_percent),
            Metric::OpAllocs(op) => Some(match op {
                Op::Encode => m.encode_allocs,
                Op::Verify => m.verify_allocs,
                Op::Decode => m.decode_allocs,
            }),
        }
    }
}

const HARNESS_NOTE: &str = "A time under about 10 ns includes harness cost and is an upper \
     bound, so a ratio whose podbuf time is under 10 ns is not comparable.";

const FLAG: &str = "\u{2020}";

fn esc(s: &str) -> String {
    s.replace('|', "\\|").replace('\n', " ")
}

fn table(headers: &[String], rows: &[Vec<String>]) -> String {
    let mut out = format!("| {} |\n", headers.join(" | "));
    out.push_str(&format!("|{}\n", " --- |".repeat(headers.len())));
    for row in rows {
        out.push_str(&format!("| {} |\n", row.join(" | ")));
    }
    out
}

fn payloads(r: &ResultFile) -> Vec<&str> {
    let mut seen: Vec<&str> = Vec::new();
    for row in &r.rows {
        if !seen.contains(&row.payload.as_str()) {
            seen.push(&row.payload);
        }
    }
    seen
}

fn cell<'a>(r: &'a ResultFile, payload: &str, e: Encoding) -> Option<&'a Cell> {
    r.rows
        .iter()
        .find(|row| row.payload == payload && row.encoding == e)
        .map(|row| &row.cell)
}

fn measured<'a>(r: &'a ResultFile, payload: &str, e: Encoding) -> Option<&'a Measured> {
    match cell(r, payload, e) {
        Some(Cell::Measured(m)) => Some(m),
        _ => None,
    }
}

/// The text of a cell that holds no number, or `None` for a measured cell.
fn marker(c: Option<&Cell>) -> Option<&'static str> {
    match c {
        Some(Cell::Measured(_)) => None,
        Some(Cell::Refused(_)) => Some("refused"),
        Some(Cell::NotBuilt(_)) => Some("not built"),
        None => Some("absent"),
    }
}

fn header(first: &str) -> Vec<String> {
    let mut h = vec![first.to_string()];
    h.extend(Encoding::ALL.iter().map(|e| e.column_label().to_string()));
    h
}

/// One row per payload, one column per encoding; `f` formats a measured cell.
fn per_encoding(r: &ResultFile, f: impl Fn(&Measured) -> String) -> String {
    let rows: Vec<Vec<String>> = payloads(r)
        .into_iter()
        .map(|p| {
            let mut row = vec![esc(p)];
            for e in Encoding::ALL {
                row.push(match (marker(cell(r, p, e)), measured(r, p, e)) {
                    (Some(m), _) => m.to_string(),
                    (None, Some(m)) => f(m),
                    (None, None) => "absent".to_string(),
                });
            }
            row
        })
        .collect();
    table(&header("Payload"), &rows)
}

pub fn size_table(r: &ResultFile) -> String {
    format!(
        "Encoded size in bytes of one payload value.\n\n{}",
        per_encoding(r, |m| format!("{:.0}", m.size))
    )
}

pub fn alloc_table(r: &ResultFile) -> String {
    format!(
        "Heap allocations of one encode / verify / decode.\n\n{}",
        per_encoding(r, |m| format!(
            "{:.0} / {:.0} / {:.0}",
            m.encode_allocs, m.verify_allocs, m.decode_allocs
        ))
    )
}

fn fmt_timing(t: &Timing) -> String {
    format!(
        "{:.1} ({:.1} to {:.1}; {:.0} %){}",
        t.median,
        t.min,
        t.max,
        t.range_spread * 100.0,
        if t.flagged {
            format!(" {FLAG}")
        } else {
            String::new()
        }
    )
}

pub fn time_table(r: &ResultFile) -> String {
    format!(
        "CPU time of one update (encode, verify and decode), in ns: the sum of the \
         medians of the three operations, with the sum of their fastest and the sum \
         of their slowest sample (an envelope, not a percentile) and its range as a \
         share of the sum. {FLAG} spread above 10 %: the figure is not stable.\n\n{}",
        per_encoding(r, |m| match &m.timing {
            Some(t) => fmt_timing(t),
            None => "not timed".to_string(),
        })
    )
}

pub fn cpu_table(r: &ResultFile) -> String {
    format!(
        "CPU load in percent of one core at {:.0} updates per second, derived \
         from the median time of one update (the headline figure).\n\n{}",
        r.rate_per_s,
        per_encoding(r, |m| match m.cpu_percent {
            Some(c) => format!("{c:.4}"),
            None => "not timed".to_string(),
        })
    )
}

pub fn paced_table(r: &ResultFile) -> String {
    let mut rows = Vec::new();
    for p in payloads(r) {
        for e in Encoding::ALL {
            if let Some(Measured {
                paced: Some(pc), ..
            }) = measured(r, p, e)
            {
                rows.push(vec![
                    esc(p),
                    e.column_label().to_string(),
                    format!("{:.4}", pc.measured_percent),
                    format!("{:.4}", pc.derived_percent),
                    format!("{:.4}", pc.difference),
                ]);
            }
        }
    }
    let headers: Vec<String> = ["Payload", "Encoding", "Paced %", "Derived %", "Difference"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    format!(
        "CPU load in percent of one core at {:.0} updates per second: paced (process CPU \
         time over wall time, includes the cost of sleep and wake-up) next to the \
         derived figure, and their difference.\n\n{}",
        r.rate_per_s,
        table(&headers, &rows)
    )
}

fn ratio_cell(r: &ResultFile, p: &str, e: Encoding, metric: Metric) -> String {
    if let Some(m) = marker(cell(r, p, e)) {
        return m.to_string();
    }
    if let Some(m) = marker(cell(r, p, Encoding::Podbuf)) {
        return format!("podbuf {m}");
    }
    let (Some(a), Some(b)) = (measured(r, p, e), measured(r, p, Encoding::Podbuf)) else {
        return "n/a".to_string();
    };
    match (metric.value(a), metric.value(b)) {
        (Some(x), Some(y)) if y != 0.0 => format!("{:.2}", x / y),
        _ => "n/a".to_string(),
    }
}

pub fn ratio_table(r: &ResultFile) -> String {
    let others = [Encoding::Flatbuffers, Encoding::Proto3];
    let metrics = [
        ("Size", Metric::Size),
        ("Time", Metric::Time),
        ("CPU", Metric::Cpu),
    ];
    let mut headers = vec!["Payload".to_string()];
    for (name, _) in metrics {
        for e in others {
            headers.push(format!("{name}: {} / podbuf", e.column_label()));
        }
    }
    let rows: Vec<Vec<String>> = payloads(r)
        .into_iter()
        .map(|p| {
            let mut row = vec![esc(p)];
            for (_, metric) in metrics {
                for e in others {
                    row.push(ratio_cell(r, p, e, metric));
                }
            }
            row
        })
        .collect();
    format!(
        "Each encoding divided by podbuf: above 1.00 the encoding is larger or \
         slower than podbuf, below 1.00 smaller or faster. The CPU ratios equal \
         the time ratios, because the derived CPU load is proportional to the \
         median time. A refused or not-built cell has no ratio and is marked.\n\n{}",
        table(&headers, &rows)
    )
}

/// Per-unit values of `metric`: one vector of three (one per encoding) for
/// each unit whose three cells are measured and carry the metric.
fn unit_values(r: &ResultFile, w: Weighting, metric: Metric) -> Vec<[f64; 3]> {
    let signals: Vec<&str> = payloads(r)
        .into_iter()
        .filter(|p| r.rows.iter().any(|row| row.payload == *p && row.is_signal))
        .collect();
    let per_signal = |p: &str| -> Option<[f64; 3]> {
        let mut out = [0.0; 3];
        for (i, e) in Encoding::ALL.into_iter().enumerate() {
            out[i] = metric.value(measured(r, p, e)?)?;
        }
        Some(out)
    };
    match w {
        Weighting::PerSignal => signals.iter().filter_map(|p| per_signal(p)).collect(),
        Weighting::PerPayloadType => {
            let mut types: Vec<&str> = Vec::new();
            for row in r.rows.iter().filter(|row| row.is_signal) {
                if !types.contains(&row.payload_type.as_str()) {
                    types.push(&row.payload_type);
                }
            }
            types
                .into_iter()
                .filter_map(|ty| {
                    let members: Vec<&str> = signals
                        .iter()
                        .copied()
                        .filter(|p| {
                            r.rows
                                .iter()
                                .any(|row| row.payload == *p && row.payload_type == ty)
                        })
                        .collect();
                    let values: Option<Vec<[f64; 3]>> =
                        members.iter().map(|p| per_signal(p)).collect();
                    let values = values?;
                    let mut mean = [0.0; 3];
                    for v in &values {
                        for i in 0..3 {
                            mean[i] += v[i] / values.len() as f64;
                        }
                    }
                    Some(mean)
                })
                .collect()
        }
    }
}

fn mix_caption(w: Weighting) -> &'static str {
    match w {
        Weighting::PerSignal => {
            "Equal-weight mix, per signal: each signal counts once, so a payload type \
             that several signals share counts once for each of them. Only signals \
             whose three encodings are measured are counted; the last column gives \
             how many."
        }
        Weighting::PerPayloadType => {
            "Equal-weight mix, per payload type: each payload type counts once, \
             however many signals share it (the value of a type is the mean over its \
             signals). Only payload types whose signals are measured in all three \
             encodings are counted; the last column gives how many."
        }
    }
}

/// The rows of a mix table: `(name, metric, decimals)`.
fn mix_rows(r: &ResultFile, w: Weighting, metrics: &[(String, Metric, usize)]) -> String {
    let mut headers = vec!["Metric".to_string()];
    headers.extend(Encoding::ALL.iter().map(|e| e.column_label().to_string()));
    headers.push("FlatBuffers / podbuf".into());
    headers.push("proto3 / podbuf".into());
    headers.push("Units".into());
    let rows: Vec<Vec<String>> = metrics
        .iter()
        .map(|(name, metric, digits)| {
            let units = unit_values(r, w, *metric);
            let mut row = vec![name.clone()];
            if units.is_empty() {
                row.extend(std::iter::repeat_n("n/a".to_string(), 5));
            } else {
                let n = units.len() as f64;
                let mean: Vec<f64> = (0..3)
                    .map(|i| units.iter().map(|u| u[i]).sum::<f64>() / n)
                    .collect();
                row.extend(mean.iter().map(|m| format!("{m:.digits$}")));
                for i in 0..2 {
                    row.push(if mean[2] != 0.0 {
                        format!("{:.2}", mean[i] / mean[2])
                    } else {
                        "n/a".to_string()
                    });
                }
            }
            row.push(units.len().to_string());
            row
        })
        .collect();
    table(&headers, &rows)
}

pub fn mix_table(r: &ResultFile, w: Weighting) -> String {
    let metrics: [(String, Metric, usize); 4] = [
        ("Size (bytes)".into(), Metric::Size, 1),
        (
            "Heap allocations per update (encode + verify + decode)".into(),
            Metric::Allocs,
            1,
        ),
        ("Time per update (median ns)".into(), Metric::Time, 1),
        (
            format!(
                "CPU % of one core at {:.0} updates per second (derived)",
                r.rate_per_s
            ),
            Metric::Cpu,
            4,
        ),
    ];
    format!("{}\n\n{}", mix_caption(w), mix_rows(r, w, &metrics))
}

/// The mix of each operation apart: time, CPU load and allocations.
pub fn op_mix_table(r: &ResultFile, w: Weighting) -> String {
    let mut metrics: Vec<(String, Metric, usize)> = Vec::new();
    for op in Op::ALL {
        let l = op.label();
        metrics.push((
            format!("{l}: time per operation (median ns)"),
            Metric::OpTime(op),
            1,
        ));
        metrics.push((
            format!(
                "{l}: CPU % of one core at {:.0} updates per second (derived)",
                r.rate_per_s
            ),
            Metric::OpCpu(op),
            4,
        ));
        metrics.push((format!("{l}: heap allocations"), Metric::OpAllocs(op), 1));
    }
    format!(
        "{} The rows are per operation. {HARNESS_NOTE}\n\n{}",
        mix_caption(w),
        mix_rows(r, w, &metrics)
    )
}

pub fn op_time_table(r: &ResultFile, op: Op) -> String {
    format!(
        "CPU time of the {} operation of one update, in ns: the median of {} samples \
         of a batch of this operation alone, with the fastest and the slowest sample \
         and the range as a share of the median. {FLAG} spread above 10 %: the \
         figure is not stable.\n\n{}",
        op.label().to_lowercase(),
        r.samples,
        per_encoding(r, |m| match &m.ops {
            Some(o) => fmt_timing(&o[op.index()].timing),
            None => "not timed".to_string(),
        })
    )
}

pub fn op_cpu_table(r: &ResultFile) -> String {
    let mut headers = vec!["Payload".to_string()];
    for op in Op::ALL {
        for e in Encoding::ALL {
            headers.push(format!("{}: {}", op.label(), e.column_label()));
        }
    }
    let rows: Vec<Vec<String>> = payloads(r)
        .into_iter()
        .map(|p| {
            let mut row = vec![esc(p)];
            for op in Op::ALL {
                for e in Encoding::ALL {
                    row.push(match marker(cell(r, p, e)) {
                        Some(m) => m.to_string(),
                        None => match measured(r, p, e).and_then(|m| Metric::OpCpu(op).value(m)) {
                            Some(c) => format!("{c:.4}"),
                            None => "not timed".to_string(),
                        },
                    });
                }
            }
            row
        })
        .collect();
    format!(
        "CPU load in percent of one core at {:.0} updates per second of each \
         operation alone, derived from the median time of that operation. The three \
         operations of an encoding add up to its derived load of one update.\n\n{}",
        r.rate_per_s,
        table(&headers, &rows)
    )
}

pub fn op_ratio_table(r: &ResultFile) -> String {
    let others = [Encoding::Flatbuffers, Encoding::Proto3];
    let mut headers = vec!["Payload".to_string()];
    for op in Op::ALL {
        for e in others {
            headers.push(format!("{}: {} / podbuf", op.label(), e.column_label()));
        }
    }
    let rows: Vec<Vec<String>> = payloads(r)
        .into_iter()
        .map(|p| {
            let mut row = vec![esc(p)];
            for op in Op::ALL {
                for e in others {
                    row.push(ratio_cell(r, p, e, Metric::OpTime(op)));
                }
            }
            row
        })
        .collect();
    format!(
        "Time of each operation of each encoding divided by the time of the same \
         operation in podbuf: above 1.00 the encoding is slower than podbuf in that \
         operation, below 1.00 faster. A refused or not-built cell has no ratio and \
         is marked. {HARNESS_NOTE}\n\n{}",
        table(&headers, &rows)
    )
}

pub fn not_built_table(r: &ResultFile) -> String {
    let mut rows = Vec::new();
    for p in payloads(r) {
        let found = r.rows.iter().find_map(|row| match &row.cell {
            Cell::NotBuilt(reason) if row.payload == p => Some((row, reason)),
            _ => None,
        });
        if let Some((row, reason)) = found {
            rows.push(vec![esc(p), esc(&row.payload_type), esc(reason)]);
        }
    }
    if rows.is_empty() {
        return "None.\n".to_string();
    }
    let headers = ["Payload", "Payload type", "Reason"].map(String::from);
    format!(
        "The bench could not build a value of these payloads, so no encoding was \
         tried on them. This is not an encoding refusal.\n\n{}",
        table(&headers, &rows)
    )
}

pub fn refused_table(r: &ResultFile) -> String {
    let mut rows = Vec::new();
    for row in &r.rows {
        if let Cell::Refused(reason) = &row.cell {
            rows.push(vec![
                esc(&row.payload),
                row.encoding.column_label().to_string(),
                esc(reason),
            ]);
        }
    }
    if rows.is_empty() {
        return "None.\n".to_string();
    }
    let headers = ["Payload", "Encoding", "Reason"].map(String::from);
    format!(
        "The encoding refused to encode the payload.\n\n{}",
        table(&headers, &rows)
    )
}

/// The whole report of one result file. `name` is the file name.
pub fn render(name: &str, r: &ResultFile) -> String {
    let freq = r
        .frequency_khz
        .map_or("not readable".to_string(), |f| format!("{f} kHz"));
    let gov = r.governor.clone().unwrap_or_else(|| "not readable".into());
    let mut out = format!(
        "# Encoding bench: {name}\n\n\
         - Date: {}\n- Commit: {}\n- Platform: {}\n- Core: {}\n\
         - Frequency of the core: {freq}; governor: {gov}\n- Toolchain: {}\n\
         - Run: {:.0} updates per second, {} samples per cell, {} s of CPU time \
         per operation of a cell and encoding, paced run: {}, changing values: {}\n\n",
        r.date,
        r.commit,
        r.platform,
        r.core_label,
        r.toolchain,
        r.rate_per_s,
        r.samples,
        r.duration_s,
        if r.paced { "yes" } else { "no" },
        if r.vary { "yes" } else { "no" },
    );
    out.push_str("## Mix\n\n");
    out.push_str(&mix_table(r, Weighting::PerSignal));
    out.push('\n');
    out.push_str(&mix_table(r, Weighting::PerPayloadType));
    out.push('\n');
    out.push_str(&op_mix_table(r, Weighting::PerSignal));
    out.push('\n');
    out.push_str(&op_mix_table(r, Weighting::PerPayloadType));
    out.push_str("\n## Per payload\n\n");
    for (title, body) in [
        ("Encoded size", size_table(r)),
        ("Heap allocations", alloc_table(r)),
        ("Time per update", time_table(r)),
        ("CPU load, derived", cpu_table(r)),
        ("Ratios to podbuf", ratio_table(r)),
        ("Time per operation, encode", op_time_table(r, Op::Encode)),
        ("Time per operation, verify", op_time_table(r, Op::Verify)),
        ("Time per operation, decode", op_time_table(r, Op::Decode)),
        ("CPU load per operation", op_cpu_table(r)),
        ("Ratios per operation", op_ratio_table(r)),
        ("CPU load, paced and derived", paced_table(r)),
    ] {
        out.push_str(&format!("### {title}\n\n{body}\n"));
    }
    out.push_str("## Payloads not built\n\n");
    out.push_str(&not_built_table(r));
    out.push_str("\n## Refused cells\n\n");
    out.push_str(&refused_table(r));
    out.push('\n');
    out
}
