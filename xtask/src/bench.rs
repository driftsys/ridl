//! `cargo xtask bench`: tasks of the encoding bench under `bench/encodings/`.
//!
//! - `gen` writes the Rust crate that `ridl build --emit rust` generates for
//!   the veh-cluster corpus, with the encodings its `ridl.toml` enables, to
//!   `bench/encodings/generated/veh-cluster`.

use std::path::{Path, PathBuf};
use std::process::Command;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask has a parent directory")
        .to_path_buf()
}

pub fn run(args: &[String]) -> Result<(), String> {
    match args.first().map(String::as_str) {
        Some("gen") => generate(),
        _ => Err("usage: cargo xtask bench gen".to_string()),
    }
}

fn generate() -> Result<(), String> {
    let root = root();
    let out = root.join("bench/encodings/generated/veh-cluster");
    // `--out-dir` writes over what it writes and leaves everything else, so a
    // file the emitter stops writing would survive without this removal.
    if out.exists() {
        std::fs::remove_dir_all(&out).map_err(|e| format!("remove {}: {e}", out.display()))?;
    }
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let status = Command::new(cargo)
        .current_dir(&root)
        .args(["run", "--quiet", "-p", "ridl-cli", "--bin", "ridl", "--"])
        .args(["build", "crates/ridlc/tests/corpus/veh-cluster"])
        .args(["--emit", "rust", "--out-dir"])
        .arg(&out)
        .status()
        .map_err(|e| format!("run ridl build: {e}"))?;
    if !status.success() {
        return Err(format!("ridl build failed: {status}"));
    }
    println!("wrote {}", out.display());
    Ok(())
}
