//! `cargo xtask bench`: tasks of the encoding bench under `bench/encodings/`.
//!
//! - `host` runs `gen`, the tests of the harness, and the program that prints
//!   the size and allocation table of the corpus.
//! - `pixel` cross-builds the harness and runs it on a Pixel 5 over adb, pinned
//!   to an A55 core and to an A76 core (`pixel.rs`).
//! - `report` reads the result files under `bench/encodings/results/` and
//!   prints Markdown tables (the `bench/encodings/report` program).
//! - `gen` writes, under `bench/encodings/generated/`, the Rust crate that
//!   `ridl build --emit rust` generates for each of the veh-cluster corpus,
//!   `examples/cabin` and the stress package `bench/encodings/stress`, with
//!   the encodings each `ridl.toml` enables. It also writes `signals.txt`, the
//!   name of every signal of those sources, which the corpus test reads.

mod pixel;

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
        Some("pixel") => pixel::run(),
        Some("report") => report(),
        _ => Err("usage: cargo xtask bench gen|host|pixel|report".to_string()),
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

/// The arguments of one cargo call of `bench host`. The bench workspace holds
/// two binaries (the harness and the report), so `run` names the harness.
fn host_args(subcommand: &str, manifest: &Path) -> Vec<std::ffi::OsString> {
    let mut args: Vec<std::ffi::OsString> = vec![subcommand.into()];
    args.push("--manifest-path".into());
    args.push(manifest.into());
    args.push("-p".into());
    args.push("encoding-bench".into());
    args
}

fn host() -> Result<(), String> {
    generate()?;
    let manifest = root().join("bench/encodings/Cargo.toml");
    for subcommand in ["test", "run"] {
        let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
        let status = Command::new(cargo)
            .args(host_args(subcommand, &manifest))
            .status()
            .map_err(|e| format!("run cargo {subcommand}: {e}"))?;
        if !status.success() {
            return Err(format!("cargo {subcommand} failed: {status}"));
        }
    }
    Ok(())
}

/// The result files in `dir`, sorted by name, so that the sections of the
/// report come in a fixed order.
fn result_files(dir: &Path) -> Result<Vec<PathBuf>, String> {
    let entries = std::fs::read_dir(dir).map_err(|e| {
        format!(
            "read {}: {e} (run `just bench-pixel`, or write a result with `--out`)",
            dir.display()
        )
    })?;
    let mut files: Vec<PathBuf> = entries
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .collect();
    files.sort();
    if files.is_empty() {
        return Err(format!("no .json result file in {}", dir.display()));
    }
    Ok(files)
}

fn report() -> Result<(), String> {
    let root = root();
    let files = result_files(&root.join("bench/encodings/results"))?;
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let status = Command::new(cargo)
        .args(["run", "--quiet", "--release", "--manifest-path"])
        .arg(root.join("bench/encodings/Cargo.toml"))
        .args(["-p", "encoding-bench-report", "--"])
        .args(&files)
        .status()
        .map_err(|e| format!("run the report program: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("the report program failed: {status}"))
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_names_the_harness_package_for_test_and_run() {
        let m = Path::new("/x/Cargo.toml");
        for sub in ["test", "run"] {
            let args = host_args(sub, m);
            assert_eq!(
                args,
                [
                    sub,
                    "--manifest-path",
                    "/x/Cargo.toml",
                    "-p",
                    "encoding-bench"
                ]
            );
        }
    }

    #[test]
    fn result_files_lists_json_files_sorted_and_nothing_else() {
        let dir = std::env::temp_dir().join(format!("bench-results-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        for name in ["b.json", "a.json", "a.cpufreq.txt"] {
            std::fs::write(dir.join(name), "").unwrap();
        }
        let files = result_files(&dir).unwrap();
        let names: Vec<_> = files
            .iter()
            .map(|f| f.file_name().unwrap().to_str().unwrap())
            .collect();
        assert_eq!(names, ["a.json", "b.json"]);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn result_files_fails_when_there_is_no_json_file() {
        let dir = std::env::temp_dir().join(format!("bench-empty-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.cpufreq.txt"), "").unwrap();
        let err = result_files(&dir).unwrap_err();
        assert!(err.contains("no .json result file"), "{err}");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
