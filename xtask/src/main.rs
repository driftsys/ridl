//! Workspace automation, invoked as `cargo xtask <task>` (the alias lives
//! in `.cargo/config.toml`).
//!
//! One task exists today: `codegen` regenerates the typed AST from
//! `crates/ridl-syntax/typl.ungram` (ADR-0007 decision 1). The drift test
//! in [`codegen`] fails whenever the committed output is stale, so the
//! generated file can never silently diverge from the grammar.

mod codegen;

use std::process::ExitCode;

fn main() -> ExitCode {
    match std::env::args().nth(1).as_deref() {
        Some("codegen") => {
            let path = codegen::write_generated();
            println!("wrote {}", path.display());
            ExitCode::SUCCESS
        }
        _ => {
            eprintln!("usage: cargo xtask codegen");
            ExitCode::from(2)
        }
    }
}
