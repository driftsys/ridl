//! Prints the size and allocation table of the corpus: one row per payload
//! and encoding. With `--out FILE` it also times every measured cell and
//! writes the result as JSON.
//!
//! Flags: `--out FILE` (turns timing on), `--core-label TEXT` (names the core
//! the run is pinned to), `--duration SECONDS` (CPU time per operation
//! of a cell and encoding, default 0.5), `--paced` (also run each cell at 4 000 updates per
//! second and read process CPU time), `--date YYYY-MM-DD` (for a device with a
//! wrong clock), `--commit HASH` (for a device without git), `--only TEXT`
//! (run only the corpus entries whose name contains the text, for profiling),
//! `--vary` (time encode, verify, decode and access on a pool of 64 distinct
//! values for the payloads that have one; needs `--out`).

use encoding_bench::args::parse_args;
use encoding_bench::corpus::{corpus, Cell, Kind, Recorder};
use encoding_bench::report::{collect_provenance, to_json, RunInfo};
use encoding_bench::timing::{TimingConfig, RATE_PER_S};
use encoding_bench::CountingAlloc;

#[global_allocator]
static ALLOC: CountingAlloc = CountingAlloc;

fn main() {
    let flags: Vec<String> = std::env::args().skip(1).collect();
    let args = match parse_args(&flags) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(2);
        }
    };
    let timing = args
        .out
        .as_ref()
        .map(|_| TimingConfig::new(args.duration_s));
    let mut rec = Recorder {
        timing: timing.clone(),
        paced: args.paced,
        vary: args.vary,
        ..Recorder::default()
    };
    for entry in corpus() {
        if args
            .only
            .as_deref()
            .is_some_and(|only| !entry.name.contains(only))
        {
            continue;
        }
        (entry.run)(&mut rec);
    }
    print_table(&rec);
    if let (Some(path), Some(cfg)) = (&args.out, &timing) {
        let provenance = collect_provenance(
            &args.core_label,
            args.commit.as_deref(),
            args.date.as_deref(),
        );
        let run = RunInfo {
            rate_per_s: RATE_PER_S,
            samples: cfg.samples,
            duration_s: cfg.duration_s,
            paced: args.paced,
            vary: args.vary,
        };
        let json = to_json(&provenance, &run, &rec.rows);
        let text = serde_json::to_string_pretty(&json).expect("the result is JSON");
        std::fs::write(path, text + "\n").unwrap_or_else(|e| {
            eprintln!("error: writing {path}: {e}");
            std::process::exit(1);
        });
        println!("wrote {path}");
    }
}

fn print_table(rec: &Recorder) {
    println!(
        "{:<48} {:<7} {:<12} {:>6} {:>7} {:>7} {:>7} {:>10} {:>8}  enc/ver/dec ns (access)",
        "payload", "kind", "encoding", "size", "enc_al", "ver_al", "dec_al", "ns/update", "cpu%@4k"
    );
    for row in &rec.rows {
        let kind = match row.kind {
            Kind::Signal => "signal",
            Kind::Stress => "stress",
        };
        match &row.cell {
            Cell::Measured(c) => {
                let (ns, pct, ops) = match &row.timing {
                    Some(t) => (
                        format!(
                            "{:.0}{}",
                            t.stat.median_ns,
                            if t.flagged() { "*" } else { "" }
                        ),
                        format!(
                            "{:.3}",
                            encoding_bench::timing::sustained_cpu_percent(
                                t.stat.median_ns,
                                RATE_PER_S
                            )
                        ),
                        format!(
                            "{:.0}/{:.0}/{:.0}{}",
                            t.ops.encode.median_ns,
                            t.ops.verify.median_ns,
                            t.ops.decode.median_ns,
                            t.ops
                                .access
                                .map_or(String::new(), |a| format!(" access {:.0}", a.median_ns))
                        ),
                    ),
                    None => ("-".into(), "-".into(), "-".into()),
                };
                println!(
                    "{:<48} {:<7} {:<12} {:>6} {:>7} {:>7} {:>7} {:>10} {:>8}  {}",
                    row.payload,
                    kind,
                    row.encoding,
                    c.size,
                    c.encode_allocs,
                    c.verify_allocs,
                    c.decode_allocs,
                    ns,
                    pct,
                    ops
                )
            }
            Cell::Refused { reason } => println!(
                "{:<48} {:<7} {:<12} refused: {reason}",
                row.payload, kind, row.encoding
            ),
            Cell::NotBuilt { reason } => println!(
                "{:<48} {:<7} {:<12} not built: {reason}",
                row.payload, kind, row.encoding
            ),
        }
    }
}
