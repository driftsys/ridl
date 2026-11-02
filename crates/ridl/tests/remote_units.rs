//! The read path for a remote `[imports]` unit, against the built `ridl`
//! binary: a unit is fetched once, read from the cache after that, resolved,
//! checked as a read-only dependency and generated, and never cataloged or
//! published as the consumer's own.

mod remote_support;

use std::path::{Path, PathBuf};

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

/// `ridl build --emit deployment` embeds the catalog descriptor of a remote
/// unit that a region names, although the build writes no catalog file for
/// that unit. The embedded bytes carry the unit's own catalog hash.
#[test]
fn a_remote_unit_catalog_is_embedded_in_the_deployment() {
    let dir = TempDir::new("deployment");
    let out = TempDir::new("deployment-out");
    let stub = Stub::serving(unit_tar(&[
        COMMON_UNIT[0],
        (
            "common.ridl",
            "package veh.common\n\ntype Speed : km/h [0.0..250.0 step 0.5]\n\ninterface Probe {\n  signal s: Speed @10ms\n}\n\nservice veh.common.probes: Probe\n",
        ),
    ]));
    let root = consumer(&dir, stub.url());
    dir.write(
        "app/system.rsdl",
        "package app\n\nimport veh.common.Probe\n\nsystem Rig { Source, Sink }\n\ncomponent Source { offers veh.common.probes }\ncomponent Sink { requires Probe }\n\ndeployment Bench for Rig {\n  machine Host { Source, Sink }\n}\n",
    );
    let (code, _, stderr) = ridl(
        &root,
        dir.path(),
        &[
            "build",
            "--out-dir",
            out.path().to_str().unwrap(),
            "--emit",
            "deployment",
        ],
    );
    assert_eq!(code, 0, "{stderr}");
    assert!(
        !out.path().join("veh.common.catalog.binfb").exists(),
        "no catalog file for a remote unit"
    );
    let bytes = std::fs::read(out.path().join("app.Bench.deployment.binfb")).unwrap();
    let view = ridl_descriptor::deployment::verify(&bytes).expect("the descriptor verifies");
    let json = ridl_descriptor::deployment::to_json(view).unwrap();
    let names: Vec<&str> = json["catalogs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|catalog| catalog["name"].as_str().unwrap())
        .collect();
    assert!(names.contains(&"veh.common"), "{names:?}");
    let remote = json["catalogs"]
        .as_array()
        .unwrap()
        .iter()
        .find(|catalog| catalog["name"] == "veh.common")
        .unwrap();
    assert!(
        !remote["catalog"].is_null(),
        "the embedded remote catalog verifies and prints"
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

/// Two consumers of one stub, the old one in `dir` and the new one in `new`,
/// each checked so that the cache holds the unit: `(old_root, new_root)`.
/// `HOME` is `dir` for both, so the two share the cache.
fn inline_pair(dir: &TempDir, new: &TempDir, stub: &Stub, appended: &str) -> (PathBuf, PathBuf) {
    let old = inline_consumer(dir, stub.url(), "");
    let new_root = inline_consumer(new, stub.url(), appended);
    for root in [&old, &new_root] {
        let (code, _, stderr) = ridl(root, dir.path(), &["check"]);
        assert_eq!(code, 0, "{stderr}");
    }
    (old, new_root)
}

/// `ridl diff --format json --encoding podbuf old new`, run with `HOME` at
/// `home`: `(exit_code, report)`.
fn podbuf_diff(home: &Path, old: &Path, new: &Path) -> (i32, serde_json::Value) {
    let (code, stdout, stderr) = ridl(
        old,
        home,
        &[
            "diff",
            "--format",
            "json",
            "--encoding",
            "podbuf",
            old.to_str().unwrap(),
            new.to_str().unwrap(),
        ],
    );
    assert_ne!(code, 2, "{stderr}");
    (
        code,
        serde_json::from_str(&stdout).expect("the report is JSON"),
    )
}

/// The categories of the report's changes, in order.
fn categories(report: &serde_json::Value) -> Vec<&str> {
    report["changes"]
        .as_array()
        .expect("the report lists its changes")
        .iter()
        .map(|change| change["category"].as_str().unwrap())
        .collect()
}

/// A struct held inline whose head reaches a remote type: the remote package
/// is laid out from the side that compiled it, so a growing append to the
/// struct is a podbuf break and a pad-fit append is not.
#[test]
fn diff_judges_a_podbuf_inline_append_whose_head_reaches_a_remote_type() {
    let dir = TempDir::new("podbuf-diff");
    let new = TempDir::new("podbuf-diff-new");
    let stub = Stub::serving(unit_tar(COMMON_UNIT));
    let (old, new_root) = inline_pair(&dir, &new, &stub, "  quality: Wide\n");
    let (code, report) = podbuf_diff(dir.path(), &old, &new_root);
    assert_eq!(code, 1, "the growing append is a podbuf break: {report}");
    assert_eq!(report["verdict"], "breaking", "{report}");
    assert!(
        categories(&report).contains(&"podbuf_inline_append"),
        "{report}"
    );

    let (code, report) = podbuf_diff(
        dir.path(),
        &old,
        &inline_consumer(&new, stub.url(), "  quality: Level\n"),
    );
    assert_eq!(code, 0, "the pad-fit append is compatible: {report}");
    assert_eq!(report["verdict"], "compatible", "{report}");
    assert!(
        !categories(&report).contains(&"podbuf_inline_append"),
        "{report}"
    );
}

/// A snapshot side carries no remote package, so when the new side is read
/// from snapshots the remote packages the old side compiled are the diff's
/// context: a pad-fit append to the struct whose head reaches the remote type
/// is compatible, where with no context the old head would have no size and
/// the append would be read as a break.
#[test]
fn diff_resolves_the_old_sides_remotes_when_the_new_side_is_a_snapshot() {
    let dir = TempDir::new("podbuf-diff-snapshot");
    let new = TempDir::new("podbuf-diff-snapshot-new");
    let out = TempDir::new("podbuf-diff-snapshot-out");
    let stub = Stub::serving(unit_tar(COMMON_UNIT));
    let (old, new_root) = inline_pair(&dir, &new, &stub, "  quality: Level\n");
    let (code, _, stderr) = ridl(
        &new_root,
        dir.path(),
        &[
            "build",
            "--out-dir",
            out.path().to_str().unwrap(),
            "--emit",
            "ir-json",
        ],
    );
    assert_eq!(code, 0, "{stderr}");
    assert!(out.path().join("app.ir.json").is_file());
    let (code, report) = podbuf_diff(dir.path(), &old, out.path());
    assert_eq!(code, 0, "the pad-fit append is compatible: {report}");
    assert_eq!(report["verdict"], "compatible", "{report}");
    assert!(
        !categories(&report).contains(&"podbuf_inline_append"),
        "{report}"
    );
}

/// A field appended with a remote type that holds 0 is judged by that type's
/// declaration, as a field of a `ridl.std` type is: compatible, not breaking
/// as unresolved.
#[test]
fn diff_resolves_the_type_of_a_field_appended_from_a_remote_package() {
    let dir = TempDir::new("remote-append");
    let new = TempDir::new("remote-append-new");
    let stub = Stub::serving(unit_tar(COMMON_UNIT));
    let (old, new_root) = inline_pair(&dir, &new, &stub, "");
    new.write(
        "app/app.ridl",
        &std::fs::read_to_string(new_root.join("app.ridl"))
            .unwrap()
            .replace("  limit: Level\n", "  limit: Level\n  top: Speed\n"),
    );
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
    assert_eq!(
        code, 0,
        "the appended field's type resolves: {stdout}{stderr}"
    );
    let report: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(report["verdict"], "compatible", "{report}");
}

/// Publishes the base `inline_consumer` as a baseline, rewrites its source
/// with `appended`, and returns the `compatible` list `ridl build --emit
/// catalog` writes for `app`, as lowercase hex, the hash lines of the history
/// `ridl baseline` then publishes, and the one hash line the first baseline
/// published.
fn judge_remote_edit(label: &str, appended: &str) -> (Vec<String>, Vec<String>, String) {
    let dir = TempDir::new(label);
    let out = TempDir::new(&format!("{label}-out"));
    let stub = Stub::serving(unit_tar(COMMON_UNIT));
    let root = inline_consumer(&dir, stub.url(), "");
    for args in [["check"], ["lock"], ["baseline"]] {
        let (code, _, stderr) = ridl(&root, dir.path(), &args);
        assert_eq!(code, 0, "{stderr}");
    }
    let baseline_hash = {
        let lines = history_lines(&root);
        assert_eq!(
            lines.len(),
            1,
            "the first baseline starts the chain: {lines:?}"
        );
        lines[0].clone()
    };
    inline_consumer(&dir, stub.url(), appended);
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
    let descriptor = out.path().join("app.catalog.binfb");
    let (code, stdout, stderr) = ridl(
        &root,
        dir.path(),
        &["describe", descriptor.to_str().unwrap()],
    );
    assert_eq!(code, 0, "{stderr}");
    let described: serde_json::Value = serde_json::from_str(&stdout).expect("stdout is JSON");
    let hex = |bytes: &serde_json::Value| -> String {
        bytes
            .as_array()
            .expect("a hash is an array of bytes")
            .iter()
            .map(|byte| format!("{:02x}", byte.as_u64().expect("a hash byte is a number")))
            .collect()
    };
    let compatible = described["compatible"]
        .as_array()
        .expect("the descriptor has a compatible array")
        .iter()
        .map(hex)
        .collect();
    let (code, _, stderr) = ridl(&root, dir.path(), &["baseline"]);
    assert_eq!(code, 0, "{stderr}");
    (compatible, history_lines(&root), baseline_hash)
}

/// The hash lines of the published `app.catalogs` of `root`, newest first.
fn history_lines(root: &Path) -> Vec<String> {
    std::fs::read_to_string(root.join(".ridl/baseline/app.catalogs"))
        .unwrap()
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(str::to_string)
        .collect()
}

/// With `podbuf` in the root manifest, a growing append to a struct held
/// inline whose head reaches a remote type ends the unit's history: `ridl
/// build` lists no earlier catalog and `ridl baseline` restarts the chain
/// from one new hash, not the published one.
#[test]
fn a_podbuf_break_through_a_remote_type_ends_the_catalog_history() {
    let (compatible, history, baseline_hash) =
        judge_remote_edit("podbuf-break", "  quality: Wide\n");
    assert!(
        compatible.is_empty(),
        "ridl build lists nothing: {compatible:?}"
    );
    assert_eq!(
        history.len(),
        1,
        "ridl baseline restarts the chain: {history:?}"
    );
    assert_ne!(
        history[0], baseline_hash,
        "the chain starts from the new catalog's hash"
    );
}

/// The same struct with a pad-fit append carries the history under both
/// commands: the remote type resolves on both sides, so the old head has a
/// size and the one-byte field fits inside it.
#[test]
fn a_pad_fit_append_through_a_remote_type_carries_the_catalog_history() {
    let (compatible, history, baseline_hash) =
        judge_remote_edit("podbuf-pads", "  quality: Level\n");
    assert_eq!(
        compatible,
        vec![baseline_hash.clone()],
        "ridl build carries the chain"
    );
    assert_eq!(
        history.len(),
        2,
        "ridl baseline carries the chain: {history:?}"
    );
    assert_ne!(
        history[0], baseline_hash,
        "the new hash is first: {history:?}"
    );
    assert_eq!(
        history[1], baseline_hash,
        "the published hash follows: {history:?}"
    );
}

/// With no edit, the catalog of a unit that reaches a remote type is the
/// published one: `ridl build` lists no earlier catalog (the one hash is the
/// current one) and `ridl baseline` keeps the one-hash history. The hash the
/// history compares is computed over the unit's own packages; one computed
/// over the remote package too would differ from the published one, which
/// would then be listed as an earlier catalog.
#[test]
fn an_unchanged_unit_reaching_a_remote_type_keeps_its_catalog_hash() {
    let (compatible, history, baseline_hash) = judge_remote_edit("unchanged", "");
    assert!(
        compatible.is_empty(),
        "ridl build lists nothing: {compatible:?}"
    );
    assert_eq!(
        history,
        vec![baseline_hash],
        "the history holds the one hash"
    );
}

/// The desk check judges an appended field's type with the remote packages,
/// as `ridl diff` does: a field of a remote type that holds 0, appended
/// beside a moved sibling, draws the RIDL-407 warning for the sibling change
/// and is not said to be breaking on its own.
#[test]
fn the_desk_check_does_not_call_a_remote_typed_append_breaking_on_its_own() {
    let dir = TempDir::new("desk-remote-append");
    let stub = Stub::serving(unit_tar(COMMON_UNIT));
    let root = inline_consumer(&dir, stub.url(), "");
    for args in [["check"], ["lock"], ["baseline"]] {
        let (code, _, stderr) = ridl(&root, dir.path(), &args);
        assert_eq!(code, 0, "{stderr}");
    }
    let source = std::fs::read_to_string(root.join("app.ridl")).unwrap();
    let edited = source.replace(
        "struct Bounds {\n  range: Reading\n  limit: Level\n}",
        "struct Bounds {\n  limit: Level\n  range: Reading\n  top: Speed\n}",
    );
    assert_ne!(edited, source, "the edit applies");
    dir.write("app/app.ridl", &edited);
    let (code, _, stderr) = ridl(&root, dir.path(), &["check"]);
    assert_eq!(code, 0, "{stderr}");
    assert!(
        stderr.contains("warning[RIDL-407]: `top` is declared in `Bounds`")
            && stderr.contains("changed ordinal"),
        "the appended field's warning names the moved siblings: {stderr}"
    );
    assert!(
        !stderr.contains("breaking on its own"),
        "a remote type that holds 0 is not breaking on its own: {stderr}"
    );
}
