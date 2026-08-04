//! The `ridlc` command-line front end — the plumbing layer (concept note §8.1,
//! docs/ROADMAP.md epic E1.13). Stable flags for CI and build systems.
//!
//! `ridlc check <PATH>` type-checks and `ridlc build <PATH> --out-dir <DIR>`
//! compiles, where `<PATH>` is a `.typl` file (single-file mode), a package
//! directory, or a workspace root. Coded diagnostics render to stderr via
//! `codespan-reporting`. The exit code is 0 when there is no error diagnostic
//! (warnings and info alone still exit 0), 1 when at least one error diagnostic
//! is present, and 2 on an input/output or usage error.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use ridl_core::Frozen;
use ridl_core::diag::render;
use ridlc::{CliRun, Emit};

#[derive(Parser)]
#[command(name = "ridlc", about = "The RIDL family compiler (plumbing)", version)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Type-check a file, package directory, or workspace.
    Check {
        /// The `.typl` file, package directory, or workspace root.
        path: PathBuf,
        /// Verify remote imports against `ridl.lock` without fetching or
        /// regenerating it (CI mode, ADR-0002 §7).
        #[arg(long)]
        frozen: bool,
    },
    /// Compile to the selected artifacts.
    Build {
        /// The `.typl` file, package directory, or workspace root.
        path: PathBuf,
        /// The directory to write generated artifacts into.
        #[arg(long)]
        out_dir: PathBuf,
        /// The artifacts to emit: `rust` (default), `c-header`, `ir-json`,
        /// `ir-text`, `ir-binary`, `typescript`.
        #[arg(long, value_delimiter = ',', default_value = "rust")]
        emit: Vec<Emit>,
        /// Verify remote imports against `ridl.lock` without fetching or
        /// regenerating it (CI mode, ADR-0002 §7).
        #[arg(long)]
        frozen: bool,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let run = match cli.command {
        Command::Check { path, frozen } => ridlc::run_check(&path, Frozen::from(frozen)),
        Command::Build {
            path,
            out_dir,
            emit,
            frozen,
        } => ridlc::run_build(&path, &out_dir, &emit, Frozen::from(frozen)),
    };
    finish(run)
}

/// Renders the run's diagnostics to stderr and turns the outcome into an exit
/// code: 2 on an I/O error, 1 when any diagnostic is an error, 0 otherwise.
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
