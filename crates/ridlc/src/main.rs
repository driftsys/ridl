//! The `ridlc` command-line front end (docs/ROADMAP.md epic E0.9, E1.10).
//!
//! `ridlc build <INPUT> --out-dir <DIR>` compiles one source file and writes
//! `<DIR>/<input-stem>.rs`. Coded diagnostics render to stderr via
//! `codespan-reporting`. The exit code is 0 when there is no error diagnostic
//! (warnings and info alone still exit 0), 1 when at least one error diagnostic
//! is present, and 2 on an input/output error (the file cannot be read, the
//! output directory cannot be created, or the output file cannot be written).

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use ridl_core::diag::{Severity, render};

#[derive(Parser)]
#[command(name = "ridlc", about = "The RIDL family compiler")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Compile a source file to Rust.
    Build {
        /// The input source file.
        input: PathBuf,
        /// The directory to write the generated Rust file into.
        #[arg(long)]
        out_dir: PathBuf,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli.command {
        Command::Build { input, out_dir } => build(&input, &out_dir),
    }
}

/// Runs the `build` subcommand: read `input`, compile it, write the generated
/// Rust into `out_dir`, and report diagnostics.
fn build(input: &Path, out_dir: &Path) -> ExitCode {
    let text = match std::fs::read_to_string(input) {
        Ok(text) => text,
        Err(err) => {
            eprintln!("error: cannot read {}: {err}", input.display());
            return ExitCode::from(2);
        }
    };

    let output = ridlc::compile(&input.to_string_lossy(), &text);

    if let Err(err) = std::fs::create_dir_all(out_dir) {
        eprintln!("error: cannot create {}: {err}", out_dir.display());
        return ExitCode::from(2);
    }

    let stem = ridlc::module_name_from_path(&input.to_string_lossy());
    let out_path = out_dir.join(format!("{stem}.rs"));

    // Writing an empty `.rs` file when compilation failed would be misleading,
    // so skip the write when the backend produced nothing and diagnostics
    // explain why. Exit codes are unchanged: the diagnostics below still drive
    // exit 1.
    let write_suppressed = output.rust_source.is_empty() && !output.diagnostics.is_empty();
    if !write_suppressed && let Err(err) = std::fs::write(&out_path, &output.rust_source) {
        eprintln!("error: cannot write {}: {err}", out_path.display());
        return ExitCode::from(2);
    }

    eprint!("{}", render(&output.diagnostics, &output.sources));

    // Warnings and info diagnostics alone do not fail the build; only an error
    // diagnostic drives exit 1.
    let has_error = output
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == Severity::Error);
    if has_error {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}
