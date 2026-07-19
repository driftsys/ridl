//! The `ridl` toolchain facade — the porcelain layer (concept note §8.1,
//! docs/ROADMAP.md epic E1.13). The cargo/deno-style front door with humane
//! defaults: `PATH` defaults to the current directory.
//!
//! `ridl check` and `ridl build` delegate to the `ridlc` library face;
//! `ridl fmt` runs the `ridl-fmt` formatter over `.typl` files (E1.14). The exit
//! code is 0 clean, 1 on a diagnostic error (or, for `fmt --check`, a file that
//! would change), and 2 on an input/output or usage error.
//!
//! `ridl diff` compares two IR snapshots or source trees through the
//! `ridl-diff` engine (E2.8a). It carries its own exit contract — 0 compatible
//! or identical, 1 breaking, 2 error (concept note §9.1, ADR-0008 decision 9) —
//! and never touches `ridlc`'s source→IR boundary beyond compiling each side.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use ridl_core::diag::{SourceMap, render};
use ridl_fmt::{FormatOutcome, format};
use ridlc::{CliRun, Emit};

#[derive(Parser)]
#[command(name = "ridl", about = "The RIDL toolchain")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Type-check a file, package, or workspace (defaults to the current
    /// directory).
    Check {
        #[arg(default_value = ".")]
        path: PathBuf,
        #[arg(long)]
        frozen: bool,
    },
    /// Compile to the selected artifacts (defaults to the current directory).
    Build {
        #[arg(default_value = ".")]
        path: PathBuf,
        #[arg(long, default_value = "out")]
        out_dir: PathBuf,
        #[arg(long, value_delimiter = ',', default_value = "rust")]
        emit: Vec<Emit>,
        #[arg(long)]
        frozen: bool,
    },
    /// Reformat `.typl` and `.ridl` files in place (defaults to the current
    /// directory).
    Fmt {
        #[arg(default_value = ".")]
        path: PathBuf,
        /// Do not write; exit 1 if any file would change.
        #[arg(long)]
        check: bool,
    },
    /// Compare two IR snapshots or source trees and classify the change:
    /// exit 0 compatible or identical, 1 breaking, 2 error.
    Diff {
        /// The baseline: an `.ir.json` snapshot, a `.typl`/`.ridl` file, a
        /// package directory, or a workspace root.
        old: Option<PathBuf>,
        /// The candidate, in the same forms as the baseline.
        new: Option<PathBuf>,
        /// Output format for the report.
        #[arg(long, value_enum, default_value = "text")]
        format: DiffFormat,
        /// Print the classification rule for one change category and exit,
        /// instead of comparing snapshots. Takes a category exactly as the
        /// report prints it, e.g. `timing_changed`.
        #[arg(long, value_name = "CATEGORY")]
        explain: Option<String>,
    },
}

/// The `ridl diff` output format — human-readable text or machine-readable
/// JSON with a stable schema.
#[derive(Clone, Copy, Debug, clap::ValueEnum)]
enum DiffFormat {
    Text,
    Json,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli.command {
        Command::Check { path, frozen } => finish(ridlc::run_check(&path, frozen.into())),
        Command::Build {
            path,
            out_dir,
            emit,
            frozen,
        } => finish(ridlc::run_build(&path, &out_dir, &emit, frozen.into())),
        Command::Fmt { path, check } => run_fmt(&path, check),
        Command::Diff {
            old,
            new,
            format,
            explain,
        } => match explain {
            Some(category) => run_explain(&category),
            None => match (old, new) {
                (Some(old), Some(new)) => run_diff(&old, &new, format),
                _ => {
                    eprintln!(
                        "error: `ridl diff` needs both an old and a new input, \
                         or `--explain <CATEGORY>`"
                    );
                    ExitCode::from(2)
                }
            },
        },
    }
}

/// Prints the classification rule for one change category — the table of
/// ADR-0008 decision 14 as text, and the CI-facing documentation of record until
/// the E4 error index publishes it. An unknown category is a usage error: exit 2
/// with the valid words listed.
fn run_explain(category: &str) -> ExitCode {
    match ridl_diff::category_from_word(category) {
        Some(category) => {
            println!("{}", ridl_diff::category_word(category));
            println!("{}", ridl_diff::explain(category));
            ExitCode::SUCCESS
        }
        None => {
            eprintln!("error: unknown change category `{category}`");
            eprintln!("the categories `ridl diff` reports are:");
            for known in ridl_diff::CATEGORIES {
                eprintln!("  {}", ridl_diff::category_word(known));
            }
            ExitCode::from(2)
        }
    }
}

/// Compares the `old` and `new` inputs and renders the report to stdout,
/// returning the diff exit code: 2 on an I/O or compile error while loading
/// either side, 1 when the change is breaking, 0 when it is compatible or the
/// two are identical.
fn run_diff(old: &Path, new: &Path, format: DiffFormat) -> ExitCode {
    let old_packages = match load_diff_side(old) {
        Ok(packages) => packages,
        Err(code) => return code,
    };
    let new_packages = match load_diff_side(new) {
        Ok(packages) => packages,
        Err(code) => return code,
    };

    let report = ridl_diff::diff_sets(&old_packages, &new_packages);
    // `render_text` already terminates every line, so it prints as is; the JSON
    // rendering has no trailing newline and gets one.
    match format {
        DiffFormat::Text => print!("{}", ridl_diff::render_text(&report)),
        DiffFormat::Json => println!("{}", ridl_diff::render_json(&report)),
    }

    match report.verdict {
        ridl_diff::Verdict::Breaking => ExitCode::FAILURE,
        ridl_diff::Verdict::Compatible | ridl_diff::Verdict::Identical => ExitCode::SUCCESS,
    }
}

/// Loads one side of a diff into a set of resolved packages.
///
/// An `.ir.json` file is deserialized directly; anything else (a `.typl` or
/// `.ridl` file, a package directory, or a workspace root) is compiled in
/// process through `ridlc::compile_workspace`. A read, parse, or compile error
/// renders to stderr and yields exit code 2 — `ridl diff` never emits a diff
/// report over a snapshot it could not build.
fn load_diff_side(entry: &Path) -> Result<Vec<ridl_ir::v2::Package>, ExitCode> {
    if is_ir_json(entry) {
        return match ridl_diff::load_ir_json(entry) {
            Ok(package) => Ok(vec![package]),
            Err(err) => {
                eprintln!("error: {}: {err}", entry.display());
                Err(ExitCode::from(2))
            }
        };
    }

    let mut db = ridl_core::RidlDatabase::default();
    match ridlc::compile_workspace(&mut db, entry) {
        Ok(output) => {
            if output
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.severity == ridl_core::diag::Severity::Error)
            {
                eprint!("{}", render(&output.diagnostics, &output.sources));
                return Err(ExitCode::from(2));
            }
            Ok(output
                .checked
                .into_iter()
                .map(|checked| checked.ir)
                .collect())
        }
        Err(err) => {
            eprintln!("error: {}: {err}", entry.display());
            Err(ExitCode::from(2))
        }
    }
}

/// Whether `path` is an `.ir.json` snapshot (a file whose name ends `.ir.json`)
/// rather than a source input.
fn is_ir_json(path: &Path) -> bool {
    path.is_file()
        && path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.ends_with(".ir.json"))
}

/// Renders a check/build run's diagnostics to stderr and turns the outcome into
/// an exit code: 2 on an I/O error, 1 when any diagnostic is an error, 0
/// otherwise.
fn finish(run: std::io::Result<CliRun>) -> ExitCode {
    match run {
        Ok(run) => {
            eprint!("{}", render(&run.diagnostics, &run.sources));
            if run.has_error() {
                ExitCode::FAILURE
            } else {
                ExitCode::SUCCESS
            }
        }
        Err(err) => {
            eprintln!("error: {err}");
            ExitCode::from(2)
        }
    }
}

/// Formats every `.typl` and `.ridl` file under `path`, each parsed under the
/// profile its extension selects.
///
/// A file with parse errors is never rewritten (a formatter must not eat broken
/// code); its diagnostics render to stderr and the run exits 1. In `--check`
/// mode nothing is written and a file that would change also exits 1.
fn run_fmt(path: &Path, check: bool) -> ExitCode {
    let mut sources = SourceMap::new();
    let mut diagnostics = Vec::new();
    let mut any_would_change = false;
    let mut any_broken = false;

    for file in collect_source_files(path) {
        let text = match std::fs::read_to_string(&file) {
            Ok(text) => text,
            Err(err) => {
                eprintln!("error: cannot read {}: {err}", file.display());
                return ExitCode::from(2);
            }
        };
        let profile = ridl_core::profile_of_path(&file.to_string_lossy());
        match format(&text, profile) {
            FormatOutcome::Formatted(formatted) => {
                if formatted != text {
                    any_would_change = true;
                    if !check && let Err(err) = std::fs::write(&file, &formatted) {
                        eprintln!("error: cannot write {}: {err}", file.display());
                        return ExitCode::from(2);
                    }
                }
            }
            FormatOutcome::ParseErrors(errors) => {
                any_broken = true;
                let file_id = sources.file_id(&file.to_string_lossy(), &text);
                for error in &errors {
                    diagnostics.push(ridlc::syntax_error_diagnostic(error, file_id));
                }
            }
        }
    }

    eprint!("{}", render(&diagnostics, &sources));
    if any_broken || (check && any_would_change) {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

/// Every `.typl` and `.ridl` file under `path`: `path` itself when it is a
/// file, otherwise a recursive walk that skips hidden directories.
fn collect_source_files(path: &Path) -> Vec<PathBuf> {
    if path.is_file() {
        return vec![path.to_path_buf()];
    }
    let mut files = Vec::new();
    let mut stack = vec![path.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let child = entry.path();
            if child.is_dir() {
                let hidden = child
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with('.'));
                if !hidden {
                    stack.push(child);
                }
            } else if child
                .extension()
                .is_some_and(|ext| ext == "typl" || ext == "ridl")
            {
                files.push(child);
            }
        }
    }
    files.sort();
    files
}
