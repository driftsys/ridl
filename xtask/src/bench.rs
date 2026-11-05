//! `cargo xtask bench`: tasks of the encoding bench under `bench/encodings/`.
//!
//! - `host` runs `gen`, the tests of the harness, and the program that prints
//!   the size and allocation table of the corpus.
//! - `gen` writes, under `bench/encodings/generated/`, the Rust crate that
//!   `ridl build --emit rust` generates for each of the veh-cluster corpus,
//!   `examples/cabin` and the stress package `bench/encodings/stress`, with
//!   the encodings each `ridl.toml` enables. It also writes `signals.txt`, the
//!   name of every signal of those sources, which the corpus test reads.

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
        Some("host") => host(),
        _ => Err("usage: cargo xtask bench gen|host".to_string()),
    }
}

/// A source package or workspace of the bench and the crate it becomes.
struct Source {
    /// Path of the source, relative to the repository root.
    path: &'static str,
    /// Directory under `bench/encodings/generated/` and crate name.
    crate_name: &'static str,
}

const SOURCES: [Source; 3] = [
    Source {
        path: "crates/ridlc/tests/corpus/veh-cluster",
        crate_name: "bench_veh_cluster",
    },
    Source {
        path: "examples/cabin",
        crate_name: "bench_cabin",
    },
    Source {
        path: "bench/encodings/stress",
        crate_name: "bench_stress",
    },
];

fn generate() -> Result<(), String> {
    let root = root();
    let generated = root.join("bench/encodings/generated");
    // `--out-dir` writes over what it writes and leaves everything else, so a
    // file the emitter stops writing would survive without this removal.
    if generated.exists() {
        std::fs::remove_dir_all(&generated)
            .map_err(|e| format!("remove {}: {e}", generated.display()))?;
    }
    let mut signals = Vec::new();
    for source in &SOURCES {
        let out = generated.join(source.crate_name);
        ridl_build(&root, source.path, "rust", &out)?;
        rename_crate(&out, source.crate_name)?;
        let ir = generated.join("ir").join(source.crate_name);
        ridl_build(&root, source.path, "ir-json", &ir)?;
        collect_signals(&ir, &mut signals)?;
        println!("wrote {}", out.display());
    }
    signals.sort();
    signals.dedup();
    let list = generated.join("signals.txt");
    std::fs::write(&list, signals.join("\n") + "\n")
        .map_err(|e| format!("write {}: {e}", list.display()))?;
    println!("wrote {} ({} signals)", list.display(), signals.len());
    Ok(())
}

fn host() -> Result<(), String> {
    generate()?;
    let manifest = root().join("bench/encodings/Cargo.toml");
    for args in [["test"], ["run"]] {
        let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
        let status = Command::new(cargo)
            .args(args)
            .arg("--manifest-path")
            .arg(&manifest)
            .status()
            .map_err(|e| format!("run cargo {}: {e}", args[0]))?;
        if !status.success() {
            return Err(format!("cargo {} failed: {status}", args[0]));
        }
    }
    Ok(())
}

fn ridl_build(root: &Path, source: &str, emit: &str, out: &Path) -> Result<(), String> {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let status = Command::new(cargo)
        .current_dir(root)
        .args(["run", "--quiet", "-p", "ridl-cli", "--bin", "ridl", "--"])
        .args(["build", source, "--emit", emit, "--out-dir"])
        .arg(out)
        .status()
        .map_err(|e| format!("run ridl build: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "ridl build {source} --emit {emit} failed: {status}"
        ))
    }
}

/// A generated crate is named after its package, or `ridl_generated` for a
/// workspace, and one cargo workspace cannot hold two packages of one name, so
/// each gets the name the harness depends on.
fn rename_crate(dir: &Path, name: &str) -> Result<(), String> {
    let manifest = dir.join("Cargo.toml");
    let text = std::fs::read_to_string(&manifest)
        .map_err(|e| format!("read {}: {e}", manifest.display()))?;
    let mut renamed = false;
    let lines: Vec<String> = text
        .lines()
        .map(|line| {
            if !renamed && line.starts_with("name = ") {
                renamed = true;
                format!("name = \"{name}\"")
            } else {
                line.to_string()
            }
        })
        .collect();
    if !renamed {
        return Err(format!("{} has no `name = ` line", manifest.display()));
    }
    std::fs::write(&manifest, lines.join("\n") + "\n")
        .map_err(|e| format!("write {}: {e}", manifest.display()))
}

/// Adds `<package>.<owner>.<signal>` for every signal of the IR files in
/// `dir`. The owner is an interface name, or the name of a service that
/// declares the signal in an inline shape.
fn collect_signals(dir: &Path, out: &mut Vec<String>) -> Result<(), String> {
    let entries = std::fs::read_dir(dir).map_err(|e| format!("read {}: {e}", dir.display()))?;
    for entry in entries {
        let path = entry.map_err(|e| e.to_string())?.path();
        let is_ir = path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.ends_with(".ir.json"));
        if !is_ir {
            continue;
        }
        let text =
            std::fs::read_to_string(&path).map_err(|e| format!("read {}: {e}", path.display()))?;
        let ir: serde_json::Value =
            serde_json::from_str(&text).map_err(|e| format!("parse {}: {e}", path.display()))?;
        let package = ir["name"].as_str().ok_or("IR file without a name")?;
        for interface in ir["interfaces"].as_array().into_iter().flatten() {
            push_signals(package, interface["name"].as_str(), interface, out);
        }
        for service in ir["services"].as_array().into_iter().flatten() {
            let name = service["name"].as_str();
            for shape in service["shapes"].as_array().into_iter().flatten() {
                if let Some(inline) = shape.get("inline") {
                    push_signals("", name, inline, out);
                }
            }
        }
    }
    Ok(())
}

fn push_signals(
    package: &str,
    owner: Option<&str>,
    shape: &serde_json::Value,
    out: &mut Vec<String>,
) {
    let Some(owner) = owner else { return };
    let prefix = if package.is_empty() {
        String::new()
    } else {
        format!("{package}.")
    };
    for interaction in shape["interactions"].as_array().into_iter().flatten() {
        if interaction.get("signalDef").is_none() {
            continue;
        }
        if let Some(name) = interaction["name"].as_str() {
            out.push(format!("{prefix}{owner}.{name}"));
        }
    }
}
