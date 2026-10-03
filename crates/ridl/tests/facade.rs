//! Integration tests for the `ridl` porcelain facade (docs/ROADMAP.md epic
//! E1.13, E1.14): `check` / `build` delegating to the compiler, humane default
//! paths, and `ridl fmt` (rewrite in place, `--check`, refuse a broken file).

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
            "ridl-facade-{label}-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::SeqCst),
        ));
        std::fs::create_dir_all(&path).expect("create the temp dir");
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }

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

/// Runs `ridl` with `args`, returning `(exit_code, stderr)`.
fn ridl(args: &[&std::ffi::OsStr]) -> (i32, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_ridl"))
        .args(args)
        .output()
        .expect("the ridl binary must run");
    let code = output.status.code().expect("the process exits with a code");
    (code, String::from_utf8_lossy(&output.stderr).into_owned())
}

const SPEED_SOURCE: &str = "package veh.common\ntype Speed: km/h [0.0..250.0 step 0.5]\n";
const PACKAGE_MANIFEST: &str = "[package]\nname = \"veh.common\"\nversion = \"1.0.0\"\n";

/// `ridl check <file>` on a clean single file exits 0.
#[test]
fn check_clean_file_exits_zero() {
    let dir = TempDir::new("check");
    let file = dir.write("speed.typl", SPEED_SOURCE);
    let (code, stderr) = ridl(&["check".as_ref(), file.as_os_str()]);
    assert_eq!(code, 0, "a clean file must exit 0, stderr:\n{stderr}");
}

/// `ridl build <file> --out-dir <tmp>` writes the single-file `<stem>.rs`.
#[test]
fn build_single_file_writes_stem_rs() {
    let dir = TempDir::new("build");
    let file = dir.write("speed.typl", SPEED_SOURCE);
    let out = TempDir::new("build-out");
    let (code, stderr) = ridl(&[
        "build".as_ref(),
        file.as_os_str(),
        "--out-dir".as_ref(),
        out.path().as_os_str(),
    ]);
    assert_eq!(code, 0, "a clean build must exit 0, stderr:\n{stderr}");
    let generated = std::fs::read_to_string(out.path().join("speed.rs"))
        .expect("single-file mode writes <input-stem>.rs");
    assert!(generated.contains("pub struct Speed"));
}

/// `ridl check` with no PATH defaults to the current directory.
#[test]
fn check_defaults_to_current_directory() {
    let dir = TempDir::new("default-path");
    dir.write("ridl.toml", PACKAGE_MANIFEST);
    dir.write("speed.typl", SPEED_SOURCE);
    let output = Command::new(env!("CARGO_BIN_EXE_ridl"))
        .arg("check")
        .current_dir(dir.path())
        .output()
        .expect("the ridl binary must run");
    assert_eq!(
        output.status.code(),
        Some(0),
        "a clean package in the current directory must check clean, stderr:\n{}",
        String::from_utf8_lossy(&output.stderr),
    );
}

/// `ridl check` reads the `.rsdl` files of a package directory: a diagnostic in
/// one is reported against that file and fails the check, where before the
/// loader skipped the file and the check passed.
#[test]
fn check_reports_a_diagnostic_in_an_rsdl_file() {
    let dir = TempDir::new("check-rsdl");
    dir.write("ridl.toml", PACKAGE_MANIFEST);
    dir.write("speed.typl", SPEED_SOURCE);
    // rsdl reference §2: `internal` before an rsdl keyword is FORM-102.
    dir.write(
        "system.rsdl",
        "package veh.common\n\ninternal component Cruise {}\n",
    );

    let (code, stderr) = ridl(&["check".as_ref(), dir.path().as_os_str()]);
    assert_eq!(
        code, 1,
        "the .rsdl diagnostic fails the check, stderr:\n{stderr}"
    );
    assert!(
        stderr.contains("FORM-102") && stderr.contains("system.rsdl"),
        "the diagnostic names its code and the .rsdl file, got:\n{stderr}"
    );
}

/// `ridl fmt` formats an explicit `.rsdl` path under the rsdl profile: the file
/// and declaration layouts are applied, and a second `--check` pass changes
/// nothing.
#[test]
fn fmt_formats_an_rsdl_file() {
    let dir = TempDir::new("fmt-rsdl");
    let input = "package veh.topology\n\ncomponent   Cruise {}\nsystem Vehicle { Cruise }\n";
    let file = dir.write("system.rsdl", input);

    let (code, stderr) = ridl(&["fmt".as_ref(), file.as_os_str()]);
    assert_eq!(code, 0, "the .rsdl file formats, stderr:\n{stderr}");
    let formatted = std::fs::read_to_string(&file).expect("the file is still readable");
    assert_eq!(
        formatted,
        "package veh.topology\n\ncomponent Cruise {}\n\nsystem Vehicle {\n  Cruise\n}\n"
    );

    let (code, stderr) = ridl(&["fmt".as_ref(), "--check".as_ref(), file.as_os_str()]);
    assert_eq!(
        code, 0,
        "the formatted file is a fixed point, stderr:\n{stderr}"
    );
}

/// A directory walk collects `.rsdl` files: `ridl fmt --check` over a tree
/// whose one file to change is an `.rsdl` file exits 1.
#[test]
fn fmt_check_walks_into_rsdl_files() {
    let dir = TempDir::new("fmt-rsdl-walk");
    let input = "package veh.topology\n\ncomponent Cruise {}\nsystem Vehicle { Cruise }\n";
    let file = dir.write("system.rsdl", input);

    let (code, stderr) = ridl(&["fmt".as_ref(), "--check".as_ref(), dir.path().as_os_str()]);
    assert_eq!(code, 1, "the .rsdl file would change, stderr:\n{stderr}");
    let unchanged = std::fs::read_to_string(&file).expect("the file is still readable");
    assert_eq!(unchanged, input, "`--check` writes nothing");
}

/// `ridl fmt <file>` rewrites a non-canonical file in place to the tight style.
#[test]
fn fmt_rewrites_in_place() {
    let dir = TempDir::new("fmt-rewrite");
    let input = "package p\ntype Speed  :  km/h [0.0..250.0 step 0.5]\n";
    let file = dir.write("speed.typl", input);

    let (code, stderr) = ridl(&["fmt".as_ref(), file.as_os_str()]);
    assert_eq!(
        code, 0,
        "formatting a clean file exits 0, stderr:\n{stderr}"
    );

    let formatted = std::fs::read_to_string(&file).expect("the file is rewritten");
    assert_ne!(formatted, input, "the non-canonical file must be rewritten");
    assert!(
        formatted.contains("type Speed: km/h"),
        "the colon must be tightened, got:\n{formatted}"
    );

    // A second `--check` pass is now a fixed point: nothing left to change.
    let (recheck, _) = ridl(&["fmt".as_ref(), "--check".as_ref(), file.as_os_str()]);
    assert_eq!(
        recheck, 0,
        "the formatted file is a fixed point under --check"
    );
}

fn tuple_width_fixture(width: usize) -> (String, String, String) {
    let names = width - "  pair: (: integer, : boolean)".chars().count();
    let first = "a".repeat(names / 2);
    let second = "b".repeat(names - names / 2);
    let line = format!("  pair: ({first}: integer, {second}: boolean)");
    assert_eq!(line.chars().count(), width);
    (
        format!("package p\nstruct S {{ pair: ({first}:integer,{second}:boolean) }}\n"),
        format!("package p\n\nstruct S {{\n{line}\n}}\n"),
        format!(
            "package p\n\nstruct S {{\n  pair: (\n    {first}: integer,\n    {second}: boolean\n  )\n}}\n"
        ),
    )
}

fn fmt_directory_and_recheck(dir: &TempDir) {
    let (code, stderr) = ridl(&["fmt".as_ref(), dir.path().as_os_str()]);
    assert_eq!(code, 0, "{stderr}");
    let (code, stderr) = ridl(&["fmt".as_ref(), "--check".as_ref(), dir.path().as_os_str()]);
    assert_eq!(code, 0, "the resolved rendering is a fixed point: {stderr}");
}

#[test]
fn fmt_editorconfig_exact_width_applies_to_explicit_file_arguments() {
    let dir = TempDir::new("fmt-width-explicit-files");
    for width in [60, 61] {
        dir.write(
            ".editorconfig",
            &format!("root = true\n[*.typl]\nmax_line_length = {width}\n"),
        );
        for columns in [width, width + 1] {
            let (source, inline, broken) = tuple_width_fixture(columns);
            let file = dir.write(&format!("width-{width}-columns-{columns}.typl"), &source);
            let (code, stderr) = ridl(&["fmt".as_ref(), file.as_os_str()]);
            assert_eq!(code, 0, "{stderr}");
            assert_eq!(
                std::fs::read_to_string(&file).unwrap(),
                if columns <= width { inline } else { broken },
                "configured width {width}, line length {columns}"
            );
            let (code, stderr) = ridl(&["fmt".as_ref(), "--check".as_ref(), file.as_os_str()]);
            assert_eq!(
                code, 0,
                "explicit-file formatting must be a fixed point: {stderr}"
            );
        }
    }
}

#[test]
fn fmt_editorconfig_resolves_the_width_for_each_file() {
    let dir = TempDir::new("fmt-width-files");
    dir.write(
        ".editorconfig",
        "root = true\n[*.ridl]\nmax_line_length = 60\n",
    );
    let (source, inline, broken) = tuple_width_fixture(80);
    let ridl_file = dir.write("types.ridl", &source);
    let typl_file = dir.write("types.typl", &source);
    fmt_directory_and_recheck(&dir);
    assert_eq!(std::fs::read_to_string(ridl_file).unwrap(), broken);
    assert_eq!(std::fs::read_to_string(typl_file).unwrap(), inline);
}

#[test]
fn fmt_editorconfig_brace_glob_matches_a_typl_file() {
    let dir = TempDir::new("fmt-width-brace-glob");
    dir.write(
        ".editorconfig",
        "root = true\n[*.{typl,ridl,rsdl}]\nmax_line_length = 60\n",
    );
    let (source, _, broken) = tuple_width_fixture(80);
    let file = dir.write("types.typl", &source);
    fmt_directory_and_recheck(&dir);
    assert_eq!(std::fs::read_to_string(file).unwrap(), broken);
}

#[test]
fn fmt_editorconfig_brace_glob_controls_valid_rsdl_header_width() {
    for (width, expected) in [
        (
            100,
            "package p\n\ncomponent Cruise [ instances = (primary, backup) ] {}\n\nsystem Vehicle {\n  Cruise.primary\n  Cruise.backup\n}\n",
        ),
        (
            40,
            "package p\n\ncomponent Cruise [\n  instances = (primary, backup)\n] {}\n\nsystem Vehicle {\n  Cruise.primary\n  Cruise.backup\n}\n",
        ),
    ] {
        let dir = TempDir::new("fmt-rsdl-width-brace-glob");
        dir.write(
            ".editorconfig",
            &format!("root = true\n[*.{{typl,ridl,rsdl}}]\nmax_line_length = {width}\n"),
        );
        let file = dir.write("system.rsdl", "package p\ncomponent Cruise [instances=(primary,backup)] {}\nsystem Vehicle { Cruise.primary, Cruise.backup }\n");
        fmt_directory_and_recheck(&dir);
        assert_eq!(std::fs::read_to_string(file).unwrap(), expected);
    }
}

#[test]
fn fmt_editorconfig_off_keeps_a_two_hundred_column_line() {
    let dir = TempDir::new("fmt-width-off");
    dir.write(".editorconfig", "root = true\n[*]\nmax_line_length = off\n");
    let (source, inline, _) = tuple_width_fixture(200);
    let file = dir.write("types.typl", &source);
    fmt_directory_and_recheck(&dir);
    assert_eq!(std::fs::read_to_string(file).unwrap(), inline);
}

#[test]
fn fmt_editorconfig_nested_root_hides_the_outer_width() {
    let dir = TempDir::new("fmt-width-root");
    dir.write(".editorconfig", "root = true\n[*]\nmax_line_length = 60\n");
    dir.write(
        "nested/.editorconfig",
        "root = true\n[*]\nindent_size = 4\n",
    );
    let (source, inline, broken) = tuple_width_fixture(80);
    let outer = dir.write("outer.typl", &source);
    let nested = dir.write("nested/types.typl", &source);
    fmt_directory_and_recheck(&dir);
    assert_eq!(std::fs::read_to_string(outer).unwrap(), broken);
    assert_eq!(std::fs::read_to_string(nested).unwrap(), inline);
}

#[test]
fn fmt_editorconfig_absence_uses_one_hundred_columns() {
    let dir = TempDir::new("fmt-width-default");
    let (source, inline, _) = tuple_width_fixture(100);
    let at_limit = dir.write("limit.typl", &source);
    let (source, _, broken) = tuple_width_fixture(101);
    let over_limit = dir.write("over.typl", &source);
    fmt_directory_and_recheck(&dir);
    assert_eq!(std::fs::read_to_string(at_limit).unwrap(), inline);
    assert_eq!(std::fs::read_to_string(over_limit).unwrap(), broken);
}

/// `ridl fmt --check` detects a file that would change: exit 1, no rewrite.
#[test]
fn fmt_check_detects_and_does_not_write() {
    let dir = TempDir::new("fmt-check");
    let input = "package p\ntype Speed  :  km/h [0.0..250.0 step 0.5]\n";
    let file = dir.write("speed.typl", input);

    let (code, _) = ridl(&["fmt".as_ref(), "--check".as_ref(), file.as_os_str()]);
    assert_eq!(code, 1, "a would-change file exits 1 under --check");

    let unchanged = std::fs::read_to_string(&file).expect("the file is still readable");
    assert_eq!(unchanged, input, "--check must never rewrite the file");
}

/// `ridl fmt` refuses to reformat a file with parse errors: exit 1, no rewrite,
/// and the parse diagnostics render.
#[test]
fn fmt_refuses_a_broken_file() {
    let dir = TempDir::new("fmt-broken");
    let input = "package p\ntype X: integer [0..10ms]\n";
    let file = dir.write("broken.typl", input);

    let (code, stderr) = ridl(&["fmt".as_ref(), file.as_os_str()]);
    assert_eq!(code, 1, "a broken file must not format cleanly");
    assert!(
        stderr.contains("TYPL-302"),
        "the parse diagnostics must render, got:\n{stderr}"
    );

    let unchanged = std::fs::read_to_string(&file).expect("the file is still readable");
    assert_eq!(unchanged, input, "a broken file must never be rewritten");
}

/// `ridl fmt` on a path that does not exist is a usage error, exit 2, naming
/// the cause (driftsys/ridl#194). Before the fix, `collect_source_files`
/// walked the missing path, found nothing, and `ridl fmt` reported success —
/// the one subcommand that did not fail closed on a bad path.
#[test]
fn fmt_on_a_missing_path_exits_two() {
    let dir = TempDir::new("fmt-missing");
    let missing = dir.path().join("does-not-exist");

    let (code, stderr) = ridl(&["fmt".as_ref(), missing.as_os_str()]);
    assert_eq!(
        code, 2,
        "a missing path is a usage error, stderr:\n{stderr}"
    );
    assert!(
        stderr.contains("cannot read"),
        "the message must name the cause, got:\n{stderr}"
    );
}

/// Whether `chmod 000` actually denies this process a directory read.
///
/// uid 0 and `CAP_DAC_OVERRIDE` both bypass the permission bits, so a test
/// that makes a directory unreadable has nothing to assert there. This probes
/// the property directly instead of reading the uid, so it is right for the
/// capability case as well as for root. A setup step that fails reports `true`
/// where it can return at all, and `TempDir::new` panics outright; either way a
/// temp directory the probe cannot use fails the test rather than silently
/// skipping it.
#[cfg(unix)]
fn mode_000_denies_reads() -> bool {
    use std::os::unix::fs::PermissionsExt;

    let dir = TempDir::new("perm-probe");
    let sub = dir.path().join("sub");
    if std::fs::create_dir_all(&sub).is_err() {
        return true;
    }
    if std::fs::set_permissions(&sub, std::fs::Permissions::from_mode(0o000)).is_err() {
        return true;
    }
    let denied = std::fs::read_dir(&sub).is_err();
    let _ = std::fs::set_permissions(&sub, std::fs::Permissions::from_mode(0o755));
    denied
}

/// `ridl fmt --check` does not treat an unreadable directory reached mid-walk
/// as zero files (driftsys/ridl#194). Before the fix, `chmod 000` on a
/// subdirectory made a would-reformat tree exit 0 with no output on either
/// stream — the defect that matters, because it is what let a permissions
/// change flip `ridl fmt --check` from failing the CI gate to passing it
/// silently.
///
/// The test is a no-op where `chmod` does not restrict the running process —
/// uid 0, or any process holding `CAP_DAC_OVERRIDE` — because there is no
/// unreadable directory to reach and the assertions would be vacuous
/// (driftsys/ridl#430). The precondition is probed rather than inferred from
/// the uid, so the skip covers the capability case too.
#[cfg(unix)]
#[test]
fn fmt_on_an_unreadable_subdirectory_exits_two() {
    use std::os::unix::fs::PermissionsExt;

    if !mode_000_denies_reads() {
        eprintln!(
            "skipped: chmod 000 does not deny this process a read, so there is \
             no unreadable directory to test (driftsys/ridl#430)"
        );
        return;
    }

    // Restores the subdirectory's permissions on scope exit, panic or not, so
    // a failed assertion below never leaves an unreadable directory behind for
    // the `TempDir`'s own `Drop` to trip over.
    struct RestorePermissions(PathBuf);
    impl Drop for RestorePermissions {
        fn drop(&mut self) {
            let _ = std::fs::set_permissions(&self.0, std::fs::Permissions::from_mode(0o755));
        }
    }

    let dir = TempDir::new("fmt-unreadable");
    dir.write(
        "a.typl",
        "package p\n\ntype Speed: km/h [0.0..250.0 step 0.5]\n",
    );
    let sub = dir.path().join("sub");
    std::fs::create_dir_all(&sub).expect("create the subdirectory");
    dir.write("sub/b.typl", "package q\n\ntype Level: integer [0..10]\n");

    std::fs::set_permissions(&sub, std::fs::Permissions::from_mode(0o000))
        .expect("chmod the subdirectory unreadable");
    let _restore = RestorePermissions(sub);

    let (code, stderr) = ridl(&["fmt".as_ref(), "--check".as_ref(), dir.path().as_os_str()]);

    assert_eq!(
        code, 2,
        "an unreadable directory reached mid-walk must not read as zero files, stderr:\n{stderr}"
    );
    assert!(
        stderr.contains("cannot read"),
        "the message must name the cause, got:\n{stderr}"
    );
    assert!(
        stderr.contains("sub"),
        "the message must name the unreadable directory itself, got:\n{stderr}"
    );
}

/// `ridl --version` and its short form `-V` both report the binary's own name
/// and version and exit 0 (driftsys/ridl#194); before the fix both flags were
/// unrecognised arguments and exited 2.
#[test]
fn version_flag_and_short_form_both_exit_zero() {
    for flag in ["--version", "-V"] {
        let output = Command::new(env!("CARGO_BIN_EXE_ridl"))
            .arg(flag)
            .output()
            .expect("the ridl binary must run");
        assert_eq!(
            output.status.code(),
            Some(0),
            "`ridl {flag}` must exit 0, stderr:\n{}",
            String::from_utf8_lossy(&output.stderr),
        );
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            stdout.trim_start().starts_with("ridl "),
            "`ridl {flag}` must report the binary's own name, got:\n{stdout}"
        );
    }
}

const CATALOG_SOURCE: &str = "package veh.cabin\n\
type Level: integer [0..100]\n\
struct Warning {\n  code: Level\n}\n\
interface Cabin {\n  signal level: Level @10ms\n  event warning: Warning @[100ms..1s]\n}\n";

/// Builds `CATALOG_SOURCE` with `--emit <emit>` and returns the
/// `Catalog.hash` of the codegen model it writes, as the canonical JSON
/// spells it (base64).
fn built_catalog_hash(emit: &str) -> String {
    let dir = TempDir::new("catalog-hash");
    let file = dir.write("cabin.ridl", CATALOG_SOURCE);
    let out = TempDir::new("catalog-hash-out");
    let (code, stderr) = ridl(&[
        "build".as_ref(),
        file.as_os_str(),
        "--emit".as_ref(),
        emit.as_ref(),
        "--out-dir".as_ref(),
        out.path().as_os_str(),
    ]);
    assert_eq!(code, 0, "`--emit {emit}` must build, stderr:\n{stderr}");
    let json = std::fs::read_to_string(out.path().join("cabin.codegen.json"))
        .expect("`--emit codegen-model` writes <input-stem>.codegen.json");
    let model: serde_json::Value = serde_json::from_str(&json).expect("the model is JSON");
    model["catalog"]["hash"]
        .as_str()
        .expect("the model carries `catalog.hash`")
        .to_owned()
}

/// driftsys/ridl#275's criterion: the catalog hash is the same whether a
/// build emits proto3, FlatBuffers or both, and it is not the all-zero
/// placeholder.
#[test]
fn catalog_hash_is_the_same_whether_a_build_emits_proto_flatbuffers_or_both() {
    let proto = built_catalog_hash("codegen-model,proto");
    let flatbuffers = built_catalog_hash("codegen-model,flatbuffers");
    let both = built_catalog_hash("codegen-model,proto,flatbuffers");
    assert_eq!(proto, flatbuffers);
    assert_eq!(proto, both);
    // 32 zero bytes in base64.
    assert_ne!(proto, "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=");
    assert_eq!(proto.len(), 44, "32 bytes are 44 base64 characters");
}
