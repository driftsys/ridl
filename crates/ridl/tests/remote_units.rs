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

/// No lint directive is reported for a file of a remote unit: a used and a
/// malformed directive in the unit draw neither TYPL-411 nor TYPL-412.
#[test]
fn no_directive_is_reported_for_a_remote_unit() {
    let dir = TempDir::new("directives");
    let stub = Stub::serving(unit_tar(DIRECTIVE_UNIT));
    let root = consumer(&dir, stub.url());
    let (code, _, stderr) = ridl(&root, dir.path(), &["check"]);
    assert_eq!(code, 0, "{stderr}");
    assert!(
        !stderr.contains("TYPL-411") && !stderr.contains("TYPL-412"),
        "{stderr}"
    );
    assert!(!stderr.contains("common.ridl"), "{stderr}");
}

/// `ridl build` reports no lint directive for a file of a remote unit either.
#[test]
fn build_reports_no_directive_for_a_remote_unit() {
    let dir = TempDir::new("build-directives");
    let out = TempDir::new("build-directives-out");
    let stub = Stub::serving(unit_tar(DIRECTIVE_UNIT));
    let root = consumer(&dir, stub.url());
    let (code, _, stderr) = ridl(
        &root,
        dir.path(),
        &[
            "build",
            "--out-dir",
            out.path().to_str().unwrap(),
            "--emit",
            "rust",
        ],
    );
    assert_eq!(code, 0, "{stderr}");
    assert!(
        !stderr.contains("TYPL-411") && !stderr.contains("TYPL-412"),
        "{stderr}"
    );
    assert!(!stderr.contains("common.ridl"), "{stderr}");
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

/// The unit of the build test above, with `limits` appended to its manifest.
fn probe_unit_with_limits(limits: &str) -> Vec<(&'static str, String)> {
    vec![
        (
            "ridl.toml",
            format!("{}\n{limits}", COMMON_UNIT[0].1),
        ),
        (
            "common.ridl",
            "package veh.common\n\ntype Speed : km/h [0.0..250.0 step 0.5]\n\ninterface Probe {\n  signal s: Speed @10ms\n}\n".to_string(),
        ),
    ]
}

/// The table that makes the Rust backend emit descriptors for FlatBuffers.
const BACKEND_TABLE: &str = "\n[backend.rust]\nencodings = [\"flatbuffers\"]\n";

/// The text between `CatalogHash([` and `])` in a generated module, with
/// its whitespace removed.
fn catalog_hash_text(module: &std::path::Path) -> String {
    let text = std::fs::read_to_string(module).unwrap();
    let start = text.find("CatalogHash([").expect("the module has a hash") + "CatalogHash([".len();
    let end = start + text[start..].find("])").expect("the hash ends");
    text[start..end].split_whitespace().collect()
}

/// The consumer records the limits of a remote unit, so it computes the same
/// catalog hash as the owner that builds the unit itself.
#[test]
fn a_remote_units_limits_reach_its_catalog_hash() {
    let files = probe_unit_with_limits("[limits]\ndefault = 64\n");
    let borrowed: Vec<(&str, &str)> = files.iter().map(|(n, c)| (*n, c.as_str())).collect();

    let owner = TempDir::new("limits-owner");
    for (name, contents) in &borrowed {
        owner.write(&format!("unit/{name}"), contents);
    }
    // The generated module carries a catalog hash only for a unit that
    // names an encoding.
    let owner_manifest = format!("{}{BACKEND_TABLE}", borrowed[0].1);
    owner.write("unit/ridl.toml", &owner_manifest);
    let owner_out = TempDir::new("limits-owner-out");
    let (code, _, stderr) = ridl(
        &owner.path().join("unit"),
        owner.path(),
        &[
            "build",
            "--emit",
            "rust",
            "--out-dir",
            owner_out.path().to_str().unwrap(),
        ],
    );
    assert_eq!(code, 0, "{stderr}");

    let dir = TempDir::new("limits-consumer");
    let out = TempDir::new("limits-consumer-out");
    let stub = Stub::serving(unit_tar(&borrowed));
    let root = consumer(&dir, stub.url());
    let consumer_manifest = std::fs::read_to_string(root.join("ridl.toml")).unwrap();
    dir.write(
        "app/ridl.toml",
        &format!("{consumer_manifest}{BACKEND_TABLE}"),
    );
    let (code, _, stderr) = ridl(
        &root,
        dir.path(),
        &[
            "build",
            "--emit",
            "rust",
            "--out-dir",
            out.path().to_str().unwrap(),
        ],
    );
    assert_eq!(code, 0, "{stderr}");
    assert_eq!(
        catalog_hash_text(&out.path().join("veh.common.rs")),
        catalog_hash_text(&owner_out.path().join("veh.common.rs")),
    );
}

/// The consumer does not check the limits of a remote unit again: a kind
/// limit of 0 and a key that names no interaction draw nothing.
#[test]
fn a_remote_units_limits_are_not_checked_by_the_consumer() {
    let files = probe_unit_with_limits(
        "[limits.kind]\nsignal = 0\n\n[limits.interaction]\n\"Nope.x\" = 8\n",
    );
    let borrowed: Vec<(&str, &str)> = files.iter().map(|(n, c)| (*n, c.as_str())).collect();
    let dir = TempDir::new("limits-unchecked");
    let stub = Stub::serving(unit_tar(&borrowed));
    let root = consumer(&dir, stub.url());
    let (code, _, stderr) = ridl(&root, dir.path(), &["check"]);
    assert_eq!(code, 0, "{stderr}");
    assert!(stderr.is_empty(), "a clean run prints nothing: {stderr}");
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

/// `ridl baseline` publishes the consumer's snapshots only: the remote
/// package gets no `.ir.json` under `.ridl/baseline`.
#[test]
fn baseline_publishes_the_consumers_packages_only() {
    let dir = TempDir::new("baseline");
    let stub = Stub::serving(unit_tar(COMMON_UNIT));
    let root = consumer(&dir, stub.url());
    let (code, _, stderr) = ridl(&root, dir.path(), &["check"]);
    assert_eq!(code, 0, "{stderr}");
    let (code, _, stderr) = ridl(&root, dir.path(), &["lock"]);
    assert_eq!(code, 0, "{stderr}");
    stub.reset_hits();
    let (code, _, stderr) = ridl(&root, dir.path(), &["baseline"]);
    assert_eq!(code, 0, "{stderr}");
    assert_eq!(stub.hits(), 0, "`ridl baseline` reads the cache");
    let published: Vec<String> = std::fs::read_dir(root.join(".ridl/baseline"))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert!(
        published.iter().any(|f| f == "app.ir.json"),
        "{published:?}"
    );
    assert!(
        !published.iter().any(|f| f.starts_with("veh.common")),
        "{published:?}"
    );
}

/// `ridl lock` allocates numbers in the consumer's units only, reading the
/// remote unit from the cache without a fetch; the remote unit's
/// `interfaces.lock` is its owner's, so nothing under the cache is written.
#[test]
fn lock_writes_the_consumers_units_only() {
    let dir = TempDir::new("lock");
    let stub = Stub::serving(unit_tar(&[
        COMMON_UNIT[0],
        (
            "common.ridl",
            "package veh.common\n\ntype Speed : km/h [0.0..250.0 step 0.5]\n\ninterface Probe {\n  signal s: Speed @10ms\n}\n",
        ),
    ]));
    let root = consumer(&dir, stub.url());
    let (code, _, stderr) = ridl(&root, dir.path(), &["check"]);
    assert_eq!(code, 0, "{stderr}");
    stub.reset_hits();
    let (code, stdout, stderr) = ridl(&root, dir.path(), &["lock"]);
    assert_eq!(code, 0, "{stderr}");
    assert_eq!(stub.hits(), 0, "`ridl lock` never fetches");
    assert!(stdout.contains("allocated Dash"), "{stdout}");
    assert!(!stdout.contains("Probe"), "{stdout}");
    assert!(root.join("interfaces.lock").is_file());
    let cache = dir.path().join(".ridl/cache");
    let mut stack = vec![cache];
    let mut visited = 0;
    while let Some(next) = stack.pop() {
        for entry in std::fs::read_dir(&next).unwrap() {
            let path = entry.unwrap().path();
            visited += 1;
            assert_ne!(
                path.file_name().unwrap(),
                "interfaces.lock",
                "the cache is never written"
            );
            if path.is_dir() {
                stack.push(path);
            }
        }
    }
    assert!(visited > 0, "the cache holds the unit's tree");
}

/// `ridl test` samples the consumer's clauses only: a clause of the remote
/// unit that faults on every sample does not fail the run.
#[test]
fn test_runs_the_consumers_packages_only() {
    let dir = TempDir::new("test");
    // The clause faults on every sample (division by zero), the `ridl test`
    // exit-1 cell of ADR-0010 decision 1; the clause form follows
    // `crates/ridl/tests/property_cli.rs`.
    let stub = Stub::serving(unit_tar(&[
        COMMON_UNIT[0],
        (
            "common.ridl",
            "package veh.common\n\ntype Speed : km/h [0.0..250.0 step 0.5]\n\ninterface Probe {\n  command half(s: Speed) [\n    require 100.0 / (s - s) > 0.0\n  ] @[..50ms]\n}\n",
        ),
    ]));
    let root = consumer(&dir, stub.url());
    let (code, _, stderr) = ridl(&root, dir.path(), &["check"]);
    assert_eq!(code, 0, "{stderr}");
    stub.reset_hits();
    let (code, stdout, stderr) = ridl(&root, dir.path(), &["test"]);
    assert_eq!(
        code, 0,
        "the faulting clause of the remote unit is not sampled: {stdout}{stderr}"
    );
    assert_eq!(stub.hits(), 0, "`ridl test` never fetches");
    assert!(stdout.contains("package app"), "{stdout}");
    assert!(!stdout.contains("veh.common"), "{stdout}");
}

/// `ridl diff` over two source trees compares the consumer's packages only:
/// a dependency whose type changed between the two sides is not the
/// consumer's breaking change.
#[test]
fn diff_compares_the_consumers_packages_only() {
    let dir = TempDir::new("diff");
    let old_stub = Stub::serving(unit_tar(COMMON_UNIT));
    let new_stub = Stub::serving(unit_tar(&[
        COMMON_UNIT[0],
        (
            "common.typl",
            "package veh.common\n\ntype Speed : m/s [0.0..70.0 step 0.5]\n",
        ),
    ]));
    let old = consumer(&dir, old_stub.url());
    let new = TempDir::new("diff-new");
    let new_root = consumer(&new, new_stub.url());
    for root in [&old, &new_root] {
        let (code, _, stderr) = ridl(root, dir.path(), &["check"]);
        assert_eq!(code, 0, "{stderr}");
    }
    let (code, stdout, stderr) = ridl(
        &old,
        dir.path(),
        &[
            "diff",
            "--format",
            "json",
            old.to_str().unwrap(),
            new_root.to_str().unwrap(),
        ],
    );
    assert_eq!(code, 0, "the consumer did not change: {stderr}");
    assert!(stdout.contains("\"identical\""), "{stdout}");
    assert!(!stdout.contains("veh.common"), "{stdout}");

    // The positive half: the same change in the consumer's own type is its
    // breaking change.
    new.write(
        "app/app.ridl",
        "package app\n\nimport veh.common.Speed\n\n/// The dashboard.\ninterface Dash {\n  /// The speed.\n  signal speed: Speed @20ms\n}\n",
    );
    let (code, stdout, _) = ridl(
        &old,
        dir.path(),
        &[
            "diff",
            "--format",
            "json",
            old.to_str().unwrap(),
            new_root.to_str().unwrap(),
        ],
    );
    assert_ne!(code, 0, "the consumer's own change is reported: {stdout}");
    assert!(stdout.contains("app"), "{stdout}");
}

/// The note `ridl build` prints beside a compatible list says a remote
/// unit's catalog is its owner's.
#[test]
fn the_catalog_note_names_the_owner() {
    let dir = TempDir::new("note");
    let stub = Stub::serving(unit_tar(COMMON_UNIT));
    let root = consumer(&dir, stub.url());
    let out = TempDir::new("note-out");
    let (code, _, stderr) = ridl(&root, dir.path(), &["check"]);
    assert_eq!(code, 0, "{stderr}");
    for _ in 0..2 {
        let (code, _, stderr) = ridl(&root, dir.path(), &["lock"]);
        assert_eq!(code, 0, "{stderr}");
        let (code, _, stderr) = ridl(&root, dir.path(), &["baseline"]);
        assert_eq!(code, 0, "{stderr}");
    }
    let (code, _, stderr) = ridl(
        &root,
        dir.path(),
        &[
            "build",
            "--out-dir",
            out.path().to_str().unwrap(),
            "--emit",
            "catalog",
        ],
    );
    assert_eq!(code, 0, "{stderr}");
    assert!(
        stderr.contains("note: the workspace names `[imports]`"),
        "{stderr}"
    );
    assert!(stderr.contains("published by its owner"), "{stderr}");
}
