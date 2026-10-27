//! Helpers shared by the integration tests: the corpus loader and the C
//! compiler driver.

// Each test binary uses a different part of this module.
#![allow(dead_code)]

use std::path::Path;
use std::process::Command;

use ridl_ir::v2;

/// The packages of a workspace under `crates/ridlc/tests/corpus/<entry>`,
/// followed by the standard package, each lowered to IR.
pub fn corpus(entry: &str) -> Vec<v2::Package> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../ridlc/tests/corpus")
        .join(entry);
    workspace(&path)
}

/// The packages of the workspace at `path`, followed by the standard package.
pub fn workspace(path: &Path) -> Vec<v2::Package> {
    let mut db = ridl_core::RidlDatabase::default();
    let output = ridlc::compile_workspace(&mut db, path)
        .unwrap_or_else(|error| panic!("load {}: {error}", path.display()));
    let mut packages: Vec<v2::Package> = output
        .checked
        .iter()
        .map(|checked| checked.ir.clone())
        .collect();
    packages.push(output.std_ir.clone());
    packages
}

/// The header of the package named `name`, with every package in scope.
pub fn header(packages: &[v2::Package], name: &str) -> String {
    let package = packages
        .iter()
        .find(|package| package.name == name)
        .unwrap_or_else(|| panic!("no package {name}"));
    let others: Vec<&v2::Package> = packages.iter().collect();
    ridl_backend_podbuf::generate_with(package, &others)
        .unwrap_or_else(|error| panic!("generate {name}: {}", error.message))
        .header_source
}

/// Whether `RIDL_REQUIRE_CC=1` turns a skipped C check into a failure.
fn required() -> bool {
    std::env::var("RIDL_REQUIRE_CC").is_ok_and(|value| value == "1")
}

/// Records that a check could not run: a notice on stderr, or a panic when
/// `RIDL_REQUIRE_CC=1` is set.
pub fn skip(reason: &str) {
    assert!(
        !required(),
        "RIDL_REQUIRE_CC=1 and the check is skipped: {reason}"
    );
    eprintln!("skipped: {reason}");
}

/// Whether `program` runs.
fn runs(program: &str) -> bool {
    Command::new(program)
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success())
}

/// A C compiler for the host: `cc`, else `clang`, else `gcc`.
pub fn host_cc() -> Option<&'static str> {
    ["cc", "clang", "gcc"].into_iter().find(|cc| runs(cc))
}

/// A `clang`, for the cross targets.
pub fn clang() -> Option<&'static str> {
    runs("clang").then_some("clang")
}

/// Runs `program` with `args` in `dir`; `Err` carries the diagnostics.
pub fn run(program: &str, args: &[&str], dir: &Path) -> Result<(), String> {
    let output = Command::new(program)
        .args(args)
        .current_dir(dir)
        .output()
        .map_err(|error| format!("{program}: {error}"))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "{program} {}\n{}{}",
            args.join(" "),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ))
    }
}

/// Writes `files` into a fresh directory.
pub fn directory(files: &[(&str, &str)]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("a temporary directory");
    for (name, text) in files {
        std::fs::write(dir.path().join(name), text).expect("write");
    }
    dir
}
