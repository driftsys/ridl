//! Integration tests for the `ridlc` plumbing binary (docs/ROADMAP.md epic
//! E1.13): the stable `check` / `build` flag surface, the exit-code contract,
//! the `--emit` outputs, and the `--frozen` lockfile gate.
//!
//! Every fixture is built in an isolated temp directory (no manifest up the
//! tree unless the test writes one), so single-file and package modes are both
//! exercised without touching the repository or the network.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

/// A unique directory under the system temp dir, removed on drop.
struct TempDir(PathBuf);

impl TempDir {
    fn new(label: &str) -> Self {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let mut path = std::env::temp_dir();
        path.push(format!(
            "ridlc-cli-{label}-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::SeqCst),
        ));
        std::fs::create_dir_all(&path).expect("create the temp dir");
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }

    /// Writes `text` at `relative`, creating parent directories, and returns the
    /// full path.
    fn write(&self, relative: &str, text: &str) -> PathBuf {
        let path = self.0.join(relative);
        std::fs::create_dir_all(path.parent().expect("a relative path has a parent"))
            .expect("create parent directories");
        std::fs::write(&path, text).expect("write the fixture file");
        path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Runs `ridlc` with `args` and returns `(exit_code, stderr)`.
fn ridlc(args: &[&std::ffi::OsStr]) -> (i32, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_ridlc"))
        .args(args)
        .output()
        .expect("the ridlc binary must run");
    let code = output.status.code().expect("the process exits with a code");
    (code, String::from_utf8_lossy(&output.stderr).into_owned())
}

const PACKAGE_MANIFEST: &str = "[package]\nname = \"veh.common\"\nversion = \"1.0.0\"\n";
const SPEED_SOURCE: &str = "package veh.common\ntype Speed: km/h [0.0..250.0 step 0.5]\n";

/// `check` on a clean single `.typl` file (single-file mode) exits 0.
#[test]
fn check_clean_single_file_exits_zero() {
    let dir = TempDir::new("check-file");
    let file = dir.write("speed.typl", SPEED_SOURCE);
    let (code, stderr) = ridlc(&["check".as_ref(), file.as_os_str()]);
    assert_eq!(code, 0, "a clean file must exit 0, stderr:\n{stderr}");
}

/// `check` on a file with a duplicate declaration exits 1 and renders TYPL-009.
#[test]
fn check_duplicate_declaration_exits_one() {
    let dir = TempDir::new("check-dup");
    let file = dir.write("dup.typl", "package veh.common\ntype A: m\ntype A: s\n");
    let (code, stderr) = ridlc(&["check".as_ref(), file.as_os_str()]);
    assert_eq!(code, 1, "an error diagnostic must exit 1");
    assert!(
        stderr.contains("TYPL-009"),
        "the duplicate declaration must render TYPL-009, got:\n{stderr}"
    );
}

/// `build` on a package directory writes `<pkg-name>.rs` and exits 0.
#[test]
fn build_package_writes_generated_rust() {
    let dir = TempDir::new("build-pkg");
    dir.write("pkg/ridl.toml", PACKAGE_MANIFEST);
    dir.write("pkg/speed.typl", SPEED_SOURCE);
    let out = TempDir::new("build-pkg-out");

    let (code, stderr) = ridlc(&[
        "build".as_ref(),
        dir.path().join("pkg").as_os_str(),
        "--out-dir".as_ref(),
        out.path().as_os_str(),
    ]);
    assert_eq!(code, 0, "a clean package must exit 0, stderr:\n{stderr}");

    let generated = std::fs::read_to_string(out.path().join("veh.common.rs"))
        .expect("the package name is the artifact base");
    assert!(
        generated.contains("pub struct Speed"),
        "the generated Rust must declare Speed, got:\n{generated}"
    );
}

/// `build --emit ir-json` writes `<pkg-name>.ir.json` with exact-decimal values
/// (ADR-0007 decision 9): the range bound is a JSON string, never a float.
#[test]
fn build_emit_ir_json_is_exact_decimal() {
    let dir = TempDir::new("build-json");
    dir.write("pkg/ridl.toml", PACKAGE_MANIFEST);
    dir.write("pkg/speed.typl", SPEED_SOURCE);
    let out = TempDir::new("build-json-out");

    let (code, _) = ridlc(&[
        "build".as_ref(),
        dir.path().join("pkg").as_os_str(),
        "--out-dir".as_ref(),
        out.path().as_os_str(),
        "--emit".as_ref(),
        "ir-json".as_ref(),
    ]);
    assert_eq!(code, 0);

    let json = std::fs::read_to_string(out.path().join("veh.common.ir.json"))
        .expect("ir-json writes <pkg-name>.ir.json");
    // Exactness is visible: the step is a quoted decimal *string*, never a JSON
    // float (a float field would render `"step": 0.5`).
    assert!(
        json.contains(r#""step": "0.5""#),
        "the step must be an exact-decimal JSON string, got:\n{json}"
    );
    assert!(
        !out.path().join("veh.common.rs").exists(),
        "ir-json alone writes no Rust file"
    );
}

/// `build --emit c-header` writes `<pkg-name>.h`.
#[test]
fn build_emit_c_header_writes_header() {
    let dir = TempDir::new("build-h");
    dir.write("pkg/ridl.toml", PACKAGE_MANIFEST);
    dir.write("pkg/speed.typl", SPEED_SOURCE);
    let out = TempDir::new("build-h-out");

    let (code, _) = ridlc(&[
        "build".as_ref(),
        dir.path().join("pkg").as_os_str(),
        "--out-dir".as_ref(),
        out.path().as_os_str(),
        "--emit".as_ref(),
        "c-header".as_ref(),
    ]);
    assert_eq!(code, 0);

    let header = std::fs::read_to_string(out.path().join("veh.common.h"))
        .expect("c-header writes <pkg-name>.h");
    assert!(!header.is_empty(), "the header must not be empty");
}

/// `build --emit typescript` writes `<pkg-name>.ts` holding the TypeScript
/// backend's output for that package.
///
/// The expected source is regenerated inside the test from the same entry over
/// the public library path (`compile_workspace` then
/// `ridl_backend_ts::generate`), so the assertion is byte equality against the
/// backend rather than a restatement of whatever the emit happens to write. The
/// non-empty guard is what keeps the equality meaningful: an emit arm that
/// wrote nothing would otherwise have to be compared against an empty string.
#[test]
fn build_emit_typescript_writes_the_backend_module() {
    let dir = TempDir::new("build-ts");
    dir.write("pkg/ridl.toml", PACKAGE_MANIFEST);
    dir.write("pkg/speed.typl", SPEED_SOURCE);
    let out = TempDir::new("build-ts-out");
    let entry = dir.path().join("pkg");

    let (code, stderr) = ridlc(&[
        "build".as_ref(),
        entry.as_os_str(),
        "--out-dir".as_ref(),
        out.path().as_os_str(),
        "--emit".as_ref(),
        "typescript".as_ref(),
    ]);
    assert_eq!(code, 0, "a clean package must exit 0, stderr:\n{stderr}");

    let written = std::fs::read_to_string(out.path().join("veh.common.ts"))
        .expect("typescript writes <pkg-name>.ts");

    let mut db = ridl_core::RidlDatabase::default();
    let checked = ridlc::compile_workspace(&mut db, &entry)
        .expect("the fixture loads")
        .checked;
    let [package] = checked.as_slice() else {
        panic!("the fixture is one package, got {}", checked.len());
    };
    let expected = ridl_backend_ts::generate(&package.ir)
        .expect("the fixture generates TypeScript")
        .source;

    assert!(
        expected.contains("export type Speed"),
        "the fixture must generate a branded Speed, or the equality below proves nothing, got:\n{expected}"
    );
    assert_eq!(
        written, expected,
        "the emitted file must hold the TypeScript backend's output"
    );
    assert!(
        !out.path().join("veh.common.rs").exists(),
        "typescript alone writes no Rust file"
    );
}

/// `build --help` offers exactly the artifacts [`ridlc::Emit`] defines, and the
/// `--emit` summary line names every one of them.
///
/// A variant added to the enum without extending that summary line compiles,
/// runs, and leaves the flag's own documentation stale — the defect this test
/// guards.
#[test]
fn build_help_documents_every_emit_value() {
    let output = Command::new(env!("CARGO_BIN_EXE_ridlc"))
        .args(["build", "--help"])
        .output()
        .expect("the ridlc binary must run");
    let help = String::from_utf8_lossy(&output.stdout).into_owned();

    // The only list items in `build --help` are the `--emit` possible values.
    let listed: Vec<&str> = help
        .lines()
        .filter_map(|line| line.trim().strip_prefix("- "))
        .filter_map(|item| item.split_once(':'))
        .map(|(value, _)| value)
        .collect();
    assert_eq!(
        listed,
        ["rust", "c-header", "ir-json", "typescript"],
        "`--emit` must offer exactly these artifacts, help:\n{help}"
    );

    let summary = help
        .lines()
        .find(|line| line.contains("The artifacts to emit"))
        .expect("the --emit flag carries a summary line");
    for value in &listed {
        assert!(
            summary.contains(value),
            "the --emit summary must name `{value}`, got:{summary}"
        );
    }
}

/// A workspace build emits every member package's artifacts.
#[test]
fn build_workspace_emits_every_member() {
    let dir = TempDir::new("build-ws");
    dir.write("ridl.toml", "[workspace]\nmembers = [\"a\", \"b\"]\n");
    dir.write(
        "a/ridl.toml",
        "[package]\nname = \"veh.a\"\nversion = \"1.0.0\"\n",
    );
    dir.write("a/a.typl", "package veh.a\ntype A: m\n");
    dir.write(
        "b/ridl.toml",
        "[package]\nname = \"veh.b\"\nversion = \"1.0.0\"\n",
    );
    dir.write("b/b.typl", "package veh.b\ntype B: s\n");
    let out = TempDir::new("build-ws-out");

    let (code, stderr) = ridlc(&[
        "build".as_ref(),
        dir.path().as_os_str(),
        "--out-dir".as_ref(),
        out.path().as_os_str(),
    ]);
    assert_eq!(code, 0, "a clean workspace must exit 0, stderr:\n{stderr}");
    assert!(
        out.path().join("veh.a.rs").exists(),
        "member veh.a must be emitted"
    );
    assert!(
        out.path().join("veh.b.rs").exists(),
        "member veh.b must be emitted"
    );
}

/// A workspace that names a standard type gets the standard artifact beside
/// its packages, for each selected emit kind. The corpus entry is used rather
/// than a fixture because it is the same input the compile proofs use.
#[test]
fn build_emits_the_standard_package_when_referenced() {
    let out = TempDir::new("emits-std-out");

    let (code, stderr) = ridlc(&[
        "build".as_ref(),
        "tests/corpus/veh-cluster".as_ref(),
        "--out-dir".as_ref(),
        out.path().as_os_str(),
        "--emit".as_ref(),
        "typescript,rust".as_ref(),
    ]);
    assert_eq!(code, 0, "the corpus entry must exit 0, stderr:\n{stderr}");
    assert!(
        out.path().join("ridl.std.rs").is_file(),
        "the Rust standard artifact must be written beside the packages",
    );
    assert!(
        out.path().join("ridl.std.ts").is_file(),
        "the TypeScript standard artifact must be written beside the packages",
    );
}

/// A workspace naming no standard type gets no standard artifact. This is the
/// only guard against a detection rule that reports every package: without it,
/// "always emit" would pass the test above.
#[test]
fn build_omits_the_standard_package_when_unreferenced() {
    let dir = TempDir::new("omits-std");
    dir.write("pkg/ridl.toml", PACKAGE_MANIFEST);
    dir.write(
        "pkg/counter.typl",
        "package veh.common\ntype Counter : integer [0..65535]\n",
    );
    let out = TempDir::new("omits-std-out");

    let (code, stderr) = ridlc(&[
        "build".as_ref(),
        dir.path().join("pkg").as_os_str(),
        "--out-dir".as_ref(),
        out.path().as_os_str(),
        "--emit".as_ref(),
        "rust".as_ref(),
    ]);
    assert_eq!(code, 0, "a clean package must exit 0, stderr:\n{stderr}");
    assert!(
        out.path().join("veh.common.rs").is_file(),
        "the package itself is still emitted",
    );
    assert!(
        !out.path().join("ridl.std.rs").exists(),
        "no standard artifact for a workspace that references none",
    );
}

/// `check --frozen` on a package with a remote import but no `ridl.lock` fails
/// with MANI-103 and never touches the network (a frozen build never fetches).
#[test]
fn frozen_without_lockfile_is_mani_103() {
    let dir = TempDir::new("frozen");
    dir.write(
        "pkg/ridl.toml",
        "[package]\nname = \"veh.common\"\nversion = \"1.0.0\"\n\n[imports]\n\"other.dep\" = \"https://registry.example.com/other/dep@v1.0.0\"\n",
    );
    dir.write("pkg/speed.typl", SPEED_SOURCE);

    let (code, stderr) = ridlc(&[
        "check".as_ref(),
        dir.path().join("pkg").as_os_str(),
        "--frozen".as_ref(),
    ]);
    assert_eq!(code, 1, "a missing lockfile under --frozen must exit 1");
    assert!(
        stderr.contains("MANI-103"),
        "the missing lockfile entry must render MANI-103, got:\n{stderr}"
    );
    assert!(
        !dir.path().join("pkg").join("ridl.lock").exists(),
        "a frozen build must never write the lockfile"
    );
}

/// `build --frozen` on a package with a remote import but no `ridl.lock` fails
/// with MANI-103 and writes no artifact: a manifest/lockfile error suppresses
/// code generation exactly like a compile error, because materialization runs
/// before the emit gate (C1).
#[test]
fn frozen_build_without_lockfile_writes_nothing() {
    let dir = TempDir::new("frozen-build");
    dir.write(
        "pkg/ridl.toml",
        "[package]\nname = \"veh.common\"\nversion = \"1.0.0\"\n\n[imports]\n\"other.dep\" = \"https://registry.example.com/other/dep@v1.0.0\"\n",
    );
    dir.write("pkg/speed.typl", SPEED_SOURCE);
    let out = TempDir::new("frozen-build-out");

    let (code, stderr) = ridlc(&[
        "build".as_ref(),
        dir.path().join("pkg").as_os_str(),
        "--out-dir".as_ref(),
        out.path().as_os_str(),
        "--frozen".as_ref(),
    ]);
    assert_eq!(
        code, 1,
        "a missing lockfile under --frozen must exit 1, stderr:\n{stderr}"
    );
    assert!(
        stderr.contains("MANI-103"),
        "the missing lockfile entry must render MANI-103, got:\n{stderr}"
    );
    assert!(
        !out.path().join("veh.common.rs").exists(),
        "a manifest/lockfile error must write no Rust artifact"
    );
    assert!(
        !dir.path().join("pkg").join("ridl.lock").exists(),
        "a frozen build must never write the lockfile"
    );
}

/// `build` on an error-bearing package writes no artifacts and exits 1: code
/// generation over error-bearing IR is skipped, matching `check` (C1). A
/// non-optional recursive struct is TYPL-206; before the build gate it also
/// crashed the backend's Default recursion with a stack overflow, so this
/// fixture proves the build both refuses to emit and does not overflow.
#[test]
fn build_recursive_struct_writes_nothing_and_does_not_overflow() {
    let dir = TempDir::new("build-recursive");
    let file = dir.write(
        "recursive.typl",
        "package veh.common\nstruct S {\n  next : S\n}\n",
    );
    let out = TempDir::new("build-recursive-out");

    // The helper asserts the process exits with a code; a stack overflow would
    // terminate it by signal (no exit code) and fail that assertion here.
    let (code, stderr) = ridlc(&[
        "build".as_ref(),
        file.as_os_str(),
        "--out-dir".as_ref(),
        out.path().as_os_str(),
        "--emit".as_ref(),
        "rust".as_ref(),
        "--emit".as_ref(),
        "ir-json".as_ref(),
    ]);
    assert_eq!(
        code, 1,
        "an error-bearing build must exit 1, stderr:\n{stderr}"
    );
    assert!(
        stderr.contains("TYPL-206"),
        "the recursive struct must render TYPL-206, got:\n{stderr}"
    );
    assert!(
        !out.path().join("recursive.rs").exists(),
        "an error-bearing build must write no Rust artifact"
    );
    assert!(
        !out.path().join("recursive.ir.json").exists(),
        "an error-bearing build must write no ir-json artifact"
    );
}

/// A non-existent entry is an I/O error: exit 2.
#[test]
fn missing_entry_is_io_error() {
    let dir = TempDir::new("missing");
    let (code, _) = ridlc(&[
        "check".as_ref(),
        dir.path().join("does-not-exist.typl").as_os_str(),
    ]);
    assert_eq!(code, 2, "a missing entry is an I/O error, exit 2");
}

/// `ridlc --version` reports the binary's own name and version and exits 0
/// (driftsys/ridl#194); before the fix it was an unrecognised argument and
/// exited 2.
#[test]
fn version_flag_exits_zero() {
    let output = Command::new(env!("CARGO_BIN_EXE_ridlc"))
        .arg("--version")
        .output()
        .expect("the ridlc binary must run");
    assert_eq!(
        output.status.code(),
        Some(0),
        "`ridlc --version` must exit 0, stderr:\n{}",
        String::from_utf8_lossy(&output.stderr),
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.trim_start().starts_with("ridlc "),
        "`ridlc --version` must report the binary's own name, got:\n{stdout}"
    );
}
