//! The result file of a timing run: provenance, the run parameters, and one
//! JSON row per `(payload, encoding)` cell.

use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{json, Value};

use crate::corpus::{Cell, Kind, Row};
use crate::timing::sustained_cpu_percent;

/// Where and when a result was produced.
#[derive(Debug, Clone)]
pub struct Provenance {
    pub date: String,
    pub commit: String,
    pub os: String,
    pub arch: String,
    pub core_label: String,
    pub toolchain: String,
    /// The current frequency of the core the run is on, when readable.
    pub frequency_khz: Option<u64>,
    /// The cpufreq governor of that core, when readable.
    pub governor: Option<String>,
}

/// The parameters of a run.
#[derive(Debug, Clone)]
pub struct RunInfo {
    pub rate_per_s: f64,
    pub samples: usize,
    pub duration_s: f64,
    pub paced: bool,
}

/// The UTC date, `YYYY-MM-DD`, of a Unix time in seconds.
pub fn civil_date(unix_secs: u64) -> String {
    // Days since 1970-01-01 to a proleptic Gregorian date (Hinnant).
    let z = (unix_secs / 86_400) as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}")
}

fn read_trimmed(path: &str) -> Option<String> {
    std::fs::read_to_string(path)
        .ok()
        .map(|s| s.trim().to_string())
}

/// The core the calling thread runs on, on Linux.
#[cfg(target_os = "linux")]
fn current_cpu() -> Option<usize> {
    // SAFETY: `sched_getcpu` takes no arguments and only reads kernel state.
    let cpu = unsafe { libc::sched_getcpu() };
    usize::try_from(cpu).ok()
}

#[cfg(not(target_os = "linux"))]
fn current_cpu() -> Option<usize> {
    None
}

/// The date of the run: `date_override` when given, else the UTC date of the
/// clock. A device may have a wrong clock, so the caller can state the date.
pub fn resolve_date(date_override: Option<&str>, unix_secs: u64) -> String {
    date_override
        .map(str::to_string)
        .unwrap_or_else(|| civil_date(unix_secs))
}

/// Collects the provenance of this run. `commit` overrides the commit read
/// from `git`, for a device that has no git. `date` overrides the clock date.
pub fn collect_provenance(
    core_label: &str,
    commit: Option<&str>,
    date: Option<&str>,
) -> Provenance {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let commit = commit.map(str::to_string).unwrap_or_else(|| {
        std::process::Command::new("git")
            .args(["rev-parse", "HEAD"])
            .output()
            .ok()
            .filter(|o| o.status.success())
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .map(|s| s.trim().to_string())
            .unwrap_or_else(|| "unknown".into())
    });
    let cpu_dir = current_cpu().map(|c| format!("/sys/devices/system/cpu/cpu{c}/cpufreq"));
    let frequency_khz = cpu_dir
        .as_ref()
        .and_then(|d| read_trimmed(&format!("{d}/scaling_cur_freq")))
        .and_then(|s| s.parse().ok());
    let governor = cpu_dir
        .as_ref()
        .and_then(|d| read_trimmed(&format!("{d}/scaling_governor")));
    Provenance {
        date: resolve_date(date, secs),
        commit,
        os: std::env::consts::OS.into(),
        arch: std::env::consts::ARCH.into(),
        core_label: core_label.into(),
        toolchain: env!("BENCH_RUSTC_VERSION").into(),
        frequency_khz,
        governor,
    }
}

fn row_json(row: &Row, rate_per_s: f64) -> Value {
    let kind = match row.kind {
        Kind::Signal => "signal",
        Kind::Stress => "stress",
    };
    let mut v = json!({
        "payload": row.payload,
        "payload_type": row.payload_type,
        "kind": kind,
        "encoding": row.encoding,
    });
    let obj = v.as_object_mut().expect("an object");
    match &row.cell {
        Cell::Measured(c) => {
            obj.insert("status".into(), "measured".into());
            obj.insert("size".into(), c.size.into());
            obj.insert("encode_allocs".into(), c.encode_allocs.into());
            obj.insert("verify_allocs".into(), c.verify_allocs.into());
            obj.insert("decode_allocs".into(), c.decode_allocs.into());
            if let Some(t) = &row.timing {
                obj.insert(
                    "per_update_ns".into(),
                    json!({
                        "median": t.stat.median_ns,
                        "p05": t.stat.p05_ns,
                        "p95": t.stat.p95_ns,
                        "min": t.stat.min_ns,
                        "max": t.stat.max_ns,
                        "range_spread": t.stat.range_spread(),
                        "reps": t.stat.reps,
                        "spread": t.stat.spread(),
                        "spread_flagged": t.stat.spread_flagged(),
                    }),
                );
                obj.insert(
                    "cpu_percent_at_rate".into(),
                    sustained_cpu_percent(t.stat.median_ns, rate_per_s).into(),
                );
                if let Some(p) = &t.paced {
                    obj.insert(
                        "paced".into(),
                        json!({
                            "updates": p.updates,
                            "wall_s": p.wall_s,
                            "process_cpu_s": p.process_cpu_s,
                            "measured_percent": p.measured_percent,
                            "derived_percent": p.derived_percent,
                            "difference": p.difference,
                        }),
                    );
                }
            }
        }
        Cell::Refused { reason } => {
            obj.insert("status".into(), "refused".into());
            obj.insert("reason".into(), reason.as_str().into());
        }
        Cell::NotBuilt { reason } => {
            obj.insert("status".into(), "not_built".into());
            obj.insert("reason".into(), reason.as_str().into());
        }
    }
    v
}

/// The whole result file.
pub fn to_json(provenance: &Provenance, run: &RunInfo, rows: &[Row]) -> Value {
    json!({
        "provenance": {
            "date": provenance.date,
            "commit": provenance.commit,
            "platform": { "os": provenance.os, "arch": provenance.arch },
            "core_label": provenance.core_label,
            "toolchain": provenance.toolchain,
            "frequency_khz": provenance.frequency_khz,
            "governor": provenance.governor,
        },
        "run": {
            "rate_per_s": run.rate_per_s,
            "samples": run.samples,
            "duration_s": run.duration_s,
            "paced": run.paced,
        },
        "rows": rows.iter().map(|r| row_json(r, run.rate_per_s)).collect::<Vec<_>>(),
    })
}
