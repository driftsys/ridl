//! The `ridl` toolchain facade — the porcelain layer (concept note §8.1,
//! docs/ROADMAP.md epic E1.13). The cargo/deno-style front door with humane
//! defaults: `PATH` defaults to the current directory.
//!
//! `ridl check` and `ridl build` delegate to the `ridlc` library face;
//! `ridl fmt` runs the `ridl-fmt` formatter over `.typl` files (E1.14). The exit
//! code is 0 clean, 1 on a diagnostic error (or, for `fmt --check`, a file that
//! would change), and 2 on an input/output or usage error.

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
    }
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
