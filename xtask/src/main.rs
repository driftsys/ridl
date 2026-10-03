//! Workspace automation, invoked as `cargo xtask <task>` (the alias lives
//! in `.cargo/config.toml`).
//!
//! Two tasks exist today:
//!
//! - `codegen` regenerates the typed AST from
//!   `crates/ridl-syntax/family.ungram` (ADR-0007 decision 1).
//! - `descriptor-codegen` regenerates the catalog-descriptor accessors from
//!   `crates/ridl-descriptor/schema/catalog.fbs`.
//!
//! The drift test in each module fails whenever the committed output is
//! stale, so a generated file can never silently diverge from its source.

mod codegen;
mod descriptor;

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
        _ => {
            eprintln!("usage: cargo xtask <codegen|descriptor-codegen>");
            ExitCode::from(2)
        }
    }
}
