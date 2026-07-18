//! The `ridlc` command-line front end (docs/ROADMAP.md epic E0.9).
//!
//! `ridlc build <INPUT> --out-dir <DIR>` compiles one source file and writes
//! `<DIR>/<input-stem>.rs`. Diagnostics print to stderr, one per line. The exit
//! code is 0 when there are no diagnostics, 1 when there are, and 2 on an
//! input/output error (the file cannot be read, the output directory cannot be
//! created, or the output file cannot be written).

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand};

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

    let stem = input
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("module");
    let out_path = out_dir.join(format!("{stem}.rs"));
    if let Err(err) = std::fs::write(&out_path, &output.rust_source) {
        eprintln!("error: cannot write {}: {err}", out_path.display());
        return ExitCode::from(2);
    }

    for diagnostic in &output.diagnostics {
        eprintln!("{diagnostic}");
    }

    if output.diagnostics.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
