//! Workspace automation, invoked as `cargo xtask <task>` (the alias lives
//! in `.cargo/config.toml`).
//!
//! Available tasks:
//!
//! - `codegen` regenerates the typed AST from
//!   `crates/ridl-syntax/family.ungram` (ADR-0007 decision 1).
//! - `descriptor-codegen` regenerates the catalog-descriptor accessors from
//!   `crates/ridl-descriptor/schema/catalog.fbs`.
//!
//! - `calibrate` dumps corpus findings and derives levels from reviewed labels.
//! - `freeze update|check` writes or checks the dumps of the frozen surfaces
//!   under `freeze/`.
//! - `eval` runs the eval tasks under `evals/tasks/` on the agent hosts, with
//!   and without the ridl-authoring skill, and scores each run.
//!
//! The code generation drift tests fail whenever the committed output is
//! stale, so a generated file can never silently diverge from its source.

mod calibrate;
mod codegen;
mod descriptor;
mod eval;
mod freeze;

use std::process::ExitCode;

fn main() -> ExitCode {
    match std::env::args().nth(1).as_deref() {
        Some("codegen") => {
            let path = codegen::write_generated();
            println!("wrote {}", path.display());
            ExitCode::SUCCESS
        }
        Some("descriptor-codegen") => {
            let path = descriptor::write_generated();
            println!("wrote {}", path.display());
            ExitCode::SUCCESS
        }
        Some("calibrate") => match calibrate::run(&std::env::args().skip(2).collect::<Vec<_>>()) {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("calibrate: {error}");
                ExitCode::from(2)
            }
        },
        Some("freeze") => match freeze::run(&std::env::args().skip(2).collect::<Vec<_>>()) {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("freeze: {error}");
                ExitCode::from(1)
            }
        },
        Some("eval") => match eval::run(&std::env::args().skip(2).collect::<Vec<_>>()) {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("eval: {error}");
                ExitCode::from(2)
            }
        },
        _ => {
            eprintln!("usage: cargo xtask <codegen|descriptor-codegen|calibrate|freeze|eval>");
            ExitCode::from(2)
        }
    }
}
