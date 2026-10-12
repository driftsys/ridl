//! The read path for a remote `[imports]` unit, against the built `ridl`
//! binary: a unit is fetched once, read from the cache after that, resolved,
//! checked as a read-only dependency and generated, and never cataloged or
//! published as the consumer's own.

mod remote_support;

use remote_support::*;

/// `ridl check` fetches the unit once, pins both hashes, and the import
/// resolves: a clean run prints nothing.
#[test]
fn check_fetches_once_and_resolves_the_import() {
    let dir = TempDir::new("check");
    let stub = Stub::serving(unit_tar(COMMON_UNIT));
    let root = consumer(&dir, stub.url());
    let (code, _, stderr) = ridl(&root, dir.path(), &["check"]);
    assert_eq!(code, 0, "{stderr}");
    assert!(stderr.is_empty(), "a clean run prints nothing: {stderr}");
    assert_eq!(stub.hits(), 1);
    let lock = std::fs::read_to_string(root.join("ridl.lock")).unwrap();
    assert!(
        lock.contains("sha256 = ") && lock.contains("tree = "),
        "{lock}"
    );

    stub.reset_hits();
    let (code, _, stderr) = ridl(&root, dir.path(), &["check", "--frozen"]);
    assert_eq!(code, 0, "{stderr}");
    assert_eq!(stub.hits(), 0, "a frozen check reads the cache");
}

/// A type the unit does not declare is an error at the consumer's use of
/// it, as a member's missing declaration is: the import binds nothing, and
/// the name is unknown where the consumer uses it.
#[test]
fn a_missing_declaration_is_the_consumers_error() {
    let dir = TempDir::new("missing-decl");
    let stub = Stub::serving(unit_tar(COMMON_UNIT));
    let root = consumer(&dir, stub.url());
    dir.write(
        "app/app.ridl",
        "package app\nimport veh.common.Nope\n/// Fast.\ntype Fast: Nope\n",
    );
    let (code, _, stderr) = ridl(&root, dir.path(), &["check"]);
    assert_eq!(code, 1, "{stderr}");
    assert!(
        stderr.contains("error[TYPL-110]") && stderr.contains("app.ridl"),
        "{stderr}"
    );
    assert!(
        !stderr.contains("remote unit"),
        "the error is the consumer's, not the unit's: {stderr}"
    );
}

/// The consumer's own untimed signal: RIDL-100 (`missing-timing`), the
/// positive half of every assertion that the unit's RIDL-100 is not shown.
const UNTIMED_CONSUMER: &str = "package app\n\nimport veh.common.Speed\n\n/// The dashboard.\ninterface Dash {\n  /// The speed.\n  signal speed: Speed\n}\n";

/// A Warning inside the remote unit is dropped while the same Warning in the
/// consumer is shown; an Error inside the unit is reported with the unit
/// name and the cache path, and fails the check.
#[test]
fn a_remote_error_fails_and_a_remote_warning_is_silent() {
    let dir = TempDir::new("severity");
    let stub = Stub::serving(unit_tar(WARNING_UNIT));
    let root = consumer(&dir, stub.url());
    dir.write("app/app.ridl", UNTIMED_CONSUMER);
    let (code, _, stderr) = ridl(&root, dir.path(), &["check"]);
    assert_eq!(code, 0, "{stderr}");
    assert!(
        stderr.contains("RIDL-100") && stderr.contains("app.ridl"),
        "the consumer's untimed signal is shown: {stderr}"
    );
    assert!(
        !stderr.contains("common.ridl"),
        "the untimed signal of the unit is not shown: {stderr}"
    );

    let dir = TempDir::new("severity-error");
    let stub = Stub::serving(unit_tar(BROKEN_UNIT));
    let root = consumer(&dir, stub.url());
    let (code, _, stderr) = ridl(&root, dir.path(), &["check"]);
    assert_eq!(code, 1, "{stderr}");
    assert!(stderr.contains("TYPL-104"), "{stderr}");
    assert!(stderr.contains("remote unit `veh.common`"), "{stderr}");
    assert!(
        stderr.contains(".ridl/cache"),
        "the path is in the cache: {stderr}"
    );
}

/// The consumer's `[lints]` do not apply to the remote unit: `deny` on a
/// lint the unit draws fails the consumer's own instance only, whether the
/// cache lies beside the consumer's directory or inside it.
#[test]
fn the_consumers_lints_do_not_reach_the_remote_unit() {
    for cache_inside_consumer in [false, true] {
        let dir = TempDir::new("lints");
        let stub = Stub::serving(unit_tar(WARNING_UNIT));
        let root = consumer(&dir, stub.url());
        dir.write(
            "app/ridl.toml",
            &format!(
                "[package]\nname = \"app\"\nversion = \"1.0.0\"\n\n[imports]\n\"veh.common\" = \"{}\"\n\n[lints]\nmissing-timing = \"deny\"\n",
                stub.url()
            ),
        );
        dir.write("app/app.ridl", UNTIMED_CONSUMER);
        // With `HOME` at the consumer's root, the cache is `app/.ridl/cache`:
        // a path the consumer's `[lints]` scope is a prefix of.
        let home = if cache_inside_consumer {
            root.clone()
        } else {
            dir.path().to_path_buf()
        };
        let (code, _, stderr) = ridl(&root, &home, &["check"]);
        assert_eq!(code, 1, "{stderr}");
        assert!(
            stderr.contains("error[RIDL-100]") && stderr.contains("app.ridl"),
            "the consumer's untimed signal is denied: {stderr}"
        );
        assert!(
            !stderr.contains("unresolved"),
            "the import resolves: {stderr}"
        );
        assert!(
            !stderr.contains("common.ridl"),
            "the unit's untimed signal is not denied (cache inside the consumer: {cache_inside_consumer}): {stderr}"
        );
        assert!(
            root.join(if cache_inside_consumer {
                ".ridl/cache"
            } else {
                "../.ridl/cache"
            })
            .is_dir(),
            "the cache is where the test put it"
        );
    }
}

/// `ridl build --emit rust,ir-json,catalog` writes the remote package's Rust
/// module and names it in `lib.rs`, but writes no IR snapshot and no catalog
/// for it.
#[test]
fn build_generates_the_remote_package_but_writes_no_snapshot_or_catalog_for_it() {
    let dir = TempDir::new("build");
    let out = TempDir::new("build-out");
    let stub = Stub::serving(unit_tar(&[
        COMMON_UNIT[0],
        (
            "common.ridl",
            "package veh.common\n\ntype Speed : km/h [0.0..250.0 step 0.5]\n\ninterface Probe {\n  signal s: Speed @10ms\n}\n",
        ),
    ]));
    let root = consumer(&dir, stub.url());
    let (code, _, stderr) = ridl(
        &root,
        dir.path(),
        &[
            "build",
            "--out-dir",
            out.path().to_str().unwrap(),
            "--emit",
            "rust,ir-json,catalog",
        ],
    );
    assert_eq!(code, 0, "{stderr}");
    assert!(out.path().join("veh.common.rs").is_file());
    assert!(out.path().join("app.rs").is_file());
    assert!(
        std::fs::read_to_string(out.path().join("lib.rs"))
            .unwrap()
            .contains("common")
    );
    assert!(out.path().join("app.ir.json").is_file());
    assert!(
        !out.path().join("veh.common.ir.json").exists(),
        "no snapshot for a remote package"
    );
    assert!(out.path().join("app.catalog.binfb").is_file());
    assert!(
        !out.path().join("veh.common.catalog.binfb").exists(),
        "no catalog for a remote unit"
    );
}

/// A lockfile written before `tree` existed fails a frozen check with
/// MANI-108 and is repaired by the next non-frozen check without a fetch.
#[test]
fn an_old_lock_is_mani_108_frozen_and_repaired_otherwise() {
    let dir = TempDir::new("old-lock");
    let stub = Stub::serving(unit_tar(COMMON_UNIT));
    let root = consumer(&dir, stub.url());
    let (code, _, stderr) = ridl(&root, dir.path(), &["check"]);
    assert_eq!(code, 0, "{stderr}");
    let lock = std::fs::read_to_string(root.join("ridl.lock")).unwrap();
    let old: String = lock
        .lines()
        .filter(|l| !l.starts_with("tree = "))
        .map(|l| format!("{l}\n"))
        .collect();
    std::fs::write(root.join("ridl.lock"), &old).unwrap();

    let (code, _, stderr) = ridl(&root, dir.path(), &["check", "--frozen"]);
    assert_eq!(code, 1, "{stderr}");
    assert!(stderr.contains("MANI-108"), "{stderr}");

    stub.reset_hits();
    let (code, _, stderr) = ridl(&root, dir.path(), &["check"]);
    assert_eq!(code, 0, "{stderr}");
    assert_eq!(stub.hits(), 0, "the cache hit needs no fetch");
    assert!(
        std::fs::read_to_string(root.join("ridl.lock"))
            .unwrap()
            .contains("tree = ")
    );
}

/// A workspace `ws` whose root pins `veh.common` to `url`, with one member
/// `app` that imports `veh.common.Speed`. Returns the member directory.
fn member_consumer(dir: &TempDir, url: &str) -> std::path::PathBuf {
    dir.write(
        "ws/ridl.toml",
        &format!("[workspace]\nmembers = [\"app\"]\n\n[imports]\n\"veh.common\" = \"{url}\"\n"),
    );
    dir.write(
        "ws/app/ridl.toml",
        "[package]\nname = \"app\"\nversion = \"1.0.0\"\n",
    );
    dir.write(
        "ws/app/app.ridl",
        "package app\nimport veh.common.Speed\ntype Fast: Speed\n",
    );
    dir.path().join("ws/app")
}

/// A `ridl check` run from inside a workspace member still reports a remote
/// unit's spanned error, which no member scope hides.
#[test]
fn a_member_entry_still_sees_a_remote_error() {
    let dir = TempDir::new("member-scope");
    let stub = Stub::serving(unit_tar(BROKEN_UNIT));
    let member = member_consumer(&dir, stub.url());
    let (code, _, stderr) = ridl(&member, dir.path(), &["check"]);
    assert_eq!(code, 1, "{stderr}");
    assert!(
        stderr.contains("TYPL-104") && stderr.contains("remote unit `veh.common`"),
        "{stderr}"
    );
}

/// A `ridl check` run from inside a workspace member still reports a remote
/// unit's detached error: a unit whose manifest names another package than
/// the `[imports]` key is MANI-105, with no source span.
#[test]
fn a_member_entry_still_sees_a_detached_remote_error() {
    let dir = TempDir::new("member-scope-detached");
    let stub = Stub::serving(unit_tar(&[
        (
            "ridl.toml",
            "[package]\nname = \"veh.other\"\nversion = \"1.0.0\"\n",
        ),
        COMMON_UNIT[1],
    ]));
    let member = member_consumer(&dir, stub.url());
    let (code, _, stderr) = ridl(&member, dir.path(), &["check"]);
    assert_eq!(code, 1, "{stderr}");
    assert!(stderr.contains("MANI-105"), "{stderr}");
}

/// A `ridl check` run from inside a workspace member still reports MANI-014
/// raised against a remote unit, which is spanned at the remote manifest: a
/// member and the remote unit both declare `veh.common`.
#[test]
fn a_member_entry_still_sees_mani_014_on_the_remote_manifest() {
    let dir = TempDir::new("member-scope-mani-014");
    let stub = Stub::serving(unit_tar(COMMON_UNIT));
    let member = member_consumer(&dir, stub.url());
    dir.write(
        "ws/ridl.toml",
        &format!(
            "[workspace]\nmembers = [\"app\", \"common\"]\n\n[imports]\n\"veh.common\" = \"{}\"\n",
            stub.url()
        ),
    );
    dir.write(
        "ws/common/ridl.toml",
        "[package]\nname = \"veh.common\"\nversion = \"1.0.0\"\n",
    );
    dir.write(
        "ws/common/common.typl",
        "package veh.common\n\ntype Local : integer [0..1]\n",
    );
    let (code, _, stderr) = ridl(&member, dir.path(), &["check"]);
    assert_eq!(code, 1, "{stderr}");
    assert!(
        stderr.contains("MANI-014") && stderr.contains(".ridl/cache"),
        "{stderr}"
    );
}
