//! `vendor/` and `ridl vendor`, against the built `ridl` binary.

mod remote_support;

use remote_support::*;

/// `ridl fmt` over the manifest root walks the consumer's sources only: a
/// file under `vendor/` is neither rewritten nor reported, and an explicit
/// path under `vendor/` is refused.
#[test]
fn fmt_never_touches_vendor() {
    let dir = TempDir::new("fmt-vendor");
    dir.write(
        "app/ridl.toml",
        "[package]\nname = \"app\"\nversion = \"1.0.0\"\n",
    );
    dir.write("app/app.typl", "package app\n");
    let unformatted = "package   veh.common\ntype   Speed :  km/h\n";
    dir.write(
        "app/vendor/veh.common/ridl.toml",
        "[package]\nname = \"veh.common\"\nversion = \"1.0.0\"\n",
    );
    let vendored = dir.write("app/vendor/veh.common/common.typl", unformatted);
    let root = dir.path().join("app");
    let (code, _, stderr) = ridl(&root, dir.path(), &["fmt", "--check"]);
    assert_eq!(code, 0, "{stderr}");
    assert!(stderr.is_empty(), "{stderr}");
    let (code, _, stderr) = ridl(&root, dir.path(), &["fmt"]);
    assert_eq!(code, 0, "{stderr}");
    assert_eq!(std::fs::read_to_string(&vendored).unwrap(), unformatted);
    // An explicit path under `vendor/` is refused, whether it names the
    // directory itself or a unit inside it.
    let (code, _, stderr) = ridl(&root, dir.path(), &["fmt", "vendor/veh.common"]);
    assert_eq!(code, 2, "{stderr}");
    assert!(stderr.contains("vendor/"), "{stderr}");
    assert_eq!(std::fs::read_to_string(&vendored).unwrap(), unformatted);
    let (code, _, stderr) = ridl(&root, dir.path(), &["fmt", "vendor"]);
    assert_eq!(code, 2, "{stderr}");
    // A directory named `vendor` deeper in the tree is formatted as usual,
    // by the walk and as an explicit path.
    let deeper = dir.write("app/parts/vendor/v.typl", "package   app.parts.vendor\n");
    let (code, _, stderr) = ridl(&root, dir.path(), &["fmt"]);
    assert_eq!(code, 0, "{stderr}");
    assert_eq!(
        std::fs::read_to_string(&deeper).unwrap(),
        "package app.parts.vendor\n"
    );
    let (code, _, stderr) = ridl(&root, dir.path(), &["fmt", "--check", "parts/vendor"]);
    assert_eq!(code, 0, "{stderr}");
    assert!(stderr.is_empty(), "{stderr}");
}

/// A frozen `ridl check` with an empty cache reads `vendor/`: the
/// air-gapped build. An edited vendored tree is MANI-107 naming the vendor
/// path and `ridl vendor`.
#[test]
fn frozen_check_reads_vendor_with_an_empty_cache() {
    let dir = TempDir::new("frozen-vendor");
    let stub = Stub::serving(unit_tar(COMMON_UNIT));
    let root = consumer(&dir, stub.url());
    let (code, _, stderr) = ridl(&root, dir.path(), &["check"]);
    assert_eq!(code, 0, "{stderr}");
    vendor_common(&root);
    std::fs::remove_dir_all(dir.path().join(".ridl")).unwrap();
    stub.reset_hits();
    let (code, _, stderr) = ridl(&root, dir.path(), &["check", "--frozen"]);
    assert_eq!(code, 0, "{stderr}");
    assert!(stderr.is_empty(), "a clean run prints nothing: {stderr}");
    assert_eq!(stub.hits(), 0);
    // An error inside the vendored tree names the vendor path.
    std::fs::write(root.join("vendor/veh.common/common.typl"), BROKEN_UNIT[1].1).unwrap();
    let (code, _, stderr) = ridl(&root, dir.path(), &["check", "--frozen"]);
    assert_eq!(code, 1, "{stderr}");
    assert!(
        stderr.contains("MANI-107")
            && stderr.contains("vendor/veh.common")
            && stderr.contains("ridl vendor"),
        "the edited tree no longer matches: {stderr}"
    );
    assert!(
        !stderr.contains("TYPL-104"),
        "a stale tree is not loaded: {stderr}"
    );
}

/// A `ridl check` run from inside a workspace member still reports MANI-014
/// raised against a vendored unit, which is spanned at the manifest under
/// `vendor/`: a member and the vendored unit both declare `veh.common`.
#[test]
fn a_member_entry_still_sees_mani_014_on_a_vendored_unit() {
    let dir = TempDir::new("member-scope-vendor-014");
    let stub = Stub::serving(unit_tar(COMMON_UNIT));
    let member = member_consumer(&dir, stub.url());
    let root = member.parent().unwrap().to_path_buf();
    // The first run writes `ridl.lock`; the member's own findings are not
    // this test's subject.
    ridl(&member, dir.path(), &["check"]);
    assert!(root.join("ridl.lock").is_file());
    vendor_common(&root);
    std::fs::remove_dir_all(dir.path().join(".ridl")).unwrap();
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
    stub.reset_hits();
    let (code, _, stderr) = ridl(&member, dir.path(), &["check", "--frozen"]);
    assert_eq!(code, 1, "{stderr}");
    assert!(
        stderr.contains("MANI-014") && stderr.contains("vendor/veh.common"),
        "{stderr}"
    );
    assert_eq!(stub.hits(), 0);
}

/// `ridl vendor` writes each imported unit to `vendor/<key>/`, pins it in
/// `ridl.lock`, and prints what it wrote; a second run reads the cache and
/// reports nothing to change. Both runs print nothing to stderr.
#[test]
fn vendor_writes_the_units_and_is_idempotent() {
    let dir = TempDir::new("vendor");
    let stub = Stub::serving(unit_tar(COMMON_UNIT));
    let root = consumer(&dir, stub.url());
    let (code, stdout, stderr) = ridl(&root, dir.path(), &["vendor"]);
    assert_eq!(code, 0, "{stderr}");
    assert!(stderr.is_empty(), "a clean run prints nothing: {stderr}");
    assert_eq!(stdout, "vendored veh.common\n");
    assert_eq!(
        std::fs::read_to_string(root.join("vendor/veh.common/common.typl")).unwrap(),
        COMMON_UNIT[1].1
    );
    let lock = std::fs::read_to_string(root.join("ridl.lock")).unwrap();
    assert!(lock.contains("tree = "), "{lock}");
    let (code, stdout, stderr) = ridl(&root, dir.path(), &["vendor"]);
    assert_eq!(code, 0, "{stderr}");
    assert!(stderr.is_empty(), "{stderr}");
    assert_eq!(stdout, "vendor/ is up to date; nothing to change\n");
    assert_eq!(stub.hits(), 1, "the second run reads the cache");
}

/// A hidden directory under `vendor/` is not a source the consumer's check
/// reports (no MANI-018), and a `.staging-*` directory an interrupted
/// `ridl vendor` left is removed by the next run.
#[test]
fn a_staging_leftover_is_silent_and_removed_by_the_next_run() {
    let dir = TempDir::new("vendor-staging");
    let stub = Stub::serving(unit_tar(COMMON_UNIT));
    let root = consumer(&dir, stub.url());
    assert_eq!(ridl(&root, dir.path(), &["vendor"]).0, 0);
    dir.write("app/vendor/.staging-x/x.typl", "package x\n");
    let (code, _, stderr) = ridl(&root, dir.path(), &["check", "--frozen"]);
    assert_eq!(code, 0, "{stderr}");
    assert!(
        stderr.is_empty(),
        "no MANI-018 for a hidden directory: {stderr}"
    );
    let (code, _, stderr) = ridl(&root, dir.path(), &["vendor"]);
    assert_eq!(code, 0, "{stderr}");
    assert!(!root.join("vendor/.staging-x").exists());
}

/// A unit's own `vendor/` is part of its tree: it is copied byte for byte to
/// `vendor/<key>/vendor/`, and a frozen check over the copy, with no cache,
/// is clean.
#[test]
fn vendor_copies_a_units_own_vendor_directory() {
    let dir = TempDir::new("vendor-own-vendor");
    let stub = Stub::serving(unit_tar(&[
        COMMON_UNIT[0],
        COMMON_UNIT[1],
        ("vendor/other.dep/d.typl", "package other.dep\r\n"),
    ]));
    let root = consumer(&dir, stub.url());
    let (code, _, stderr) = ridl(&root, dir.path(), &["vendor"]);
    assert_eq!(code, 0, "{stderr}");
    assert_eq!(
        std::fs::read(root.join("vendor/veh.common/vendor/other.dep/d.typl")).unwrap(),
        b"package other.dep\r\n"
    );
    std::fs::remove_dir_all(dir.path().join(".ridl")).unwrap();
    let (code, _, stderr) = ridl(&root, dir.path(), &["check", "--frozen"]);
    assert_eq!(code, 0, "{stderr}");
    assert!(stderr.is_empty(), "{stderr}");
}

/// A URL change in `[imports]` after a vendor run is fetched and the
/// directory replaced: `ridl vendor` never reads `vendor/` as a source.
#[test]
fn vendor_replaces_a_unit_whose_url_changed() {
    let dir = TempDir::new("vendor-url-change");
    let stub = Stub::serving(unit_tar(COMMON_UNIT));
    let root = consumer(&dir, stub.url());
    assert_eq!(ridl(&root, dir.path(), &["vendor"]).0, 0);
    let newer = Stub::serving(unit_tar(&[
        COMMON_UNIT[0],
        (
            "common.typl",
            "package veh.common\n\ntype Speed : km/h [0.0..400.0 step 0.5]\n",
        ),
    ]));
    consumer(&dir, newer.url());
    let (code, stdout, stderr) = ridl(&root, dir.path(), &["vendor"]);
    assert_eq!(code, 0, "{stderr}");
    assert!(stderr.is_empty(), "{stderr}");
    assert_eq!(stdout, "vendored veh.common\n");
    assert_eq!(newer.hits(), 1);
    assert!(
        std::fs::read_to_string(root.join("vendor/veh.common/common.typl"))
            .unwrap()
            .contains("400")
    );
    std::fs::remove_dir_all(dir.path().join(".ridl")).unwrap();
    let (code, _, stderr) = ridl(&root, dir.path(), &["check", "--frozen"]);
    assert_eq!(code, 0, "{stderr}");
}

/// A unit no longer imported is removed from `vendor/`.
#[test]
fn vendor_removes_a_unit_no_longer_imported() {
    let dir = TempDir::new("vendor-remove");
    let stub = Stub::serving(unit_tar(COMMON_UNIT));
    let root = consumer(&dir, stub.url());
    assert_eq!(ridl(&root, dir.path(), &["vendor"]).0, 0);
    dir.write(
        "app/ridl.toml",
        "[package]\nname = \"app\"\nversion = \"1.0.0\"\n",
    );
    dir.write("app/app.ridl", "package app\n");
    let (code, stdout, stderr) = ridl(&root, dir.path(), &["vendor"]);
    assert_eq!(code, 0, "{stderr}");
    assert!(stderr.is_empty(), "{stderr}");
    assert_eq!(stdout, "removed veh.common\n");
    assert!(!root.join("vendor/veh.common").exists());
}

/// A consumer with no `[imports]` and no `vendor/` gets no `vendor/`.
#[test]
fn vendor_with_nothing_to_vendor_creates_no_directory() {
    let dir = TempDir::new("vendor-none");
    dir.write(
        "app/ridl.toml",
        "[package]\nname = \"app\"\nversion = \"1.0.0\"\n",
    );
    dir.write("app/app.ridl", "package app\n");
    let root = dir.path().join("app");
    let (code, stdout, stderr) = ridl(&root, dir.path(), &["vendor"]);
    assert_eq!(code, 0, "{stderr}");
    assert!(stderr.is_empty(), "{stderr}");
    assert_eq!(stdout, "vendor/ is up to date; nothing to change\n");
    assert!(!root.join("vendor").exists());
}

/// `ridl vendor` applies no `[lints]` level, so a lint that is `allow` by
/// default (TYPL-410) is left out of what it prints.
#[test]
fn vendor_does_not_print_an_allow_by_default_lint() {
    let dir = TempDir::new("vendor-allow");
    let stub = Stub::serving(unit_tar(COMMON_UNIT));
    let root = consumer(&dir, stub.url());
    dir.write(
        "app/extra.typl",
        "package app\n\n/** A count. */\ntype Count: integer [0..300]\n",
    );
    let (code, _, stderr) = ridl(&root, dir.path(), &["vendor"]);
    assert_eq!(code, 0, "{stderr}");
    assert!(stderr.is_empty(), "{stderr}");
}

/// A diagnostic error — in a remote unit or in the consumer — leaves
/// `vendor/` as it was and exits 1; a fetch failure exits 2.
#[test]
fn vendor_writes_nothing_on_an_error() {
    let dir = TempDir::new("vendor-error");
    let stub = Stub::serving(unit_tar(COMMON_UNIT));
    let root = consumer(&dir, stub.url());
    assert_eq!(ridl(&root, dir.path(), &["vendor"]).0, 0);
    let before = std::fs::read_to_string(root.join("vendor/veh.common/common.typl")).unwrap();
    let unchanged = || {
        assert_eq!(
            std::fs::read_to_string(root.join("vendor/veh.common/common.typl")).unwrap(),
            before
        );
    };

    let broken = Stub::serving(unit_tar(BROKEN_UNIT));
    consumer(&dir, broken.url());
    let (code, stdout, stderr) = ridl(&root, dir.path(), &["vendor"]);
    assert_eq!(code, 1, "{stderr}");
    assert!(stderr.contains("TYPL-104"), "{stderr}");
    assert_eq!(stdout, "");
    unchanged();

    // A unit that would vendor cleanly, with an error in the consumer.
    let newer = Stub::serving(unit_tar(&[
        COMMON_UNIT[0],
        (
            "common.typl",
            "package veh.common\n\ntype Speed : km/h [0.0..400.0 step 0.5]\n",
        ),
    ]));
    consumer(&dir, newer.url());
    dir.write("app/bad.typl", "package app\n\ntype Bad: Missing\n");
    let (code, stdout, stderr) = ridl(&root, dir.path(), &["vendor"]);
    assert_eq!(code, 1, "{stderr}");
    assert!(stderr.contains("error"), "{stderr}");
    assert_eq!(stdout, "");
    unchanged();
    std::fs::remove_file(root.join("bad.typl")).unwrap();

    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let dead = format!("http://{}/package.tar", listener.local_addr().unwrap());
    drop(listener);
    consumer(&dir, &dead);
    let (code, stdout, stderr) = ridl(&root, dir.path(), &["vendor"]);
    assert_eq!(code, 2, "{stderr}");
    assert!(stderr.contains("MANI-101"), "{stderr}");
    assert_eq!(stdout, "");
    unchanged();
}

/// A missing path exits 2 before root discovery: run from inside a consumer
/// that imports a unit, it fetches nothing, writes no `vendor/`, and prints
/// nothing to stdout.
#[test]
fn vendor_on_a_missing_path_exits_2() {
    let dir = TempDir::new("vendor-missing");
    let stub = Stub::serving(unit_tar(COMMON_UNIT));
    let root = consumer(&dir, stub.url());
    let (code, stdout, stderr) = ridl(&root, dir.path(), &["vendor", "nowhere"]);
    assert_eq!(code, 2, "{stderr}");
    assert!(stderr.contains("nowhere"), "{stderr}");
    assert_eq!(stdout, "");
    assert!(!root.join("vendor").exists());
    assert_eq!(stub.hits(), 0);
}

/// In workspace mode `vendor/` is at the workspace root, whichever member
/// the command runs from (V-6).
#[test]
fn vendor_writes_at_the_workspace_root() {
    let dir = TempDir::new("vendor-ws");
    let stub = Stub::serving(unit_tar(COMMON_UNIT));
    dir.write("ws/ridl.toml", "[workspace]\nmembers = [\"app\"]\n");
    dir.write(
        "ws/app/ridl.toml",
        &format!(
            "[package]\nname = \"app\"\nversion = \"1.0.0\"\n\n[imports]\n\"veh.common\" = \"{}\"\n",
            stub.url()
        ),
    );
    dir.write(
        "ws/app/app.ridl",
        "package app\n\nimport veh.common.Speed\n\n/// The dashboard.\ninterface Dash {\n  /// The speed.\n  signal speed: Speed @10ms\n}\n",
    );
    let (code, _, stderr) = ridl(&dir.path().join("ws/app"), dir.path(), &["vendor"]);
    assert_eq!(code, 0, "{stderr}");
    assert!(stderr.is_empty(), "{stderr}");
    assert!(
        dir.path()
            .join("ws/vendor/veh.common/common.typl")
            .is_file()
    );
    assert!(!dir.path().join("ws/app/vendor").exists());
}

/// When a run removes the directory of a unit no longer imported, the
/// MANI-018 the load drew for that directory is not printed, while the
/// MANI-018 of a stray source file left in `vendor/` is.
#[test]
fn vendor_reports_a_stray_file_but_not_a_removed_directory() {
    let dir = TempDir::new("vendor-stray");
    let stub = Stub::serving(unit_tar(COMMON_UNIT));
    let root = consumer(&dir, stub.url());
    assert_eq!(ridl(&root, dir.path(), &["vendor"]).0, 0);
    dir.write(
        "app/ridl.toml",
        "[package]\nname = \"app\"\nversion = \"1.0.0\"\n",
    );
    dir.write("app/app.ridl", "package app\n");
    dir.write("app/vendor/stray.typl", "package stray\n");
    let (code, stdout, stderr) = ridl(&root, dir.path(), &["vendor"]);
    assert_eq!(code, 0, "{stderr}");
    assert_eq!(stdout, "removed veh.common\n");
    assert_eq!(stderr.matches("warning[MANI-018]").count(), 1, "{stderr}");
    assert!(stderr.contains("stray.typl"), "{stderr}");
    assert!(!stderr.contains("common.typl"), "{stderr}");
}

/// A path with no `ridl.toml` at or above it exits 2 and writes nothing.
#[test]
fn vendor_without_a_manifest_exits_2() {
    let dir = TempDir::new("vendor-no-manifest");
    std::fs::create_dir_all(dir.path().join("empty")).unwrap();
    let empty = dir.path().join("empty");
    let (code, stdout, stderr) = ridl(&empty, dir.path(), &["vendor"]);
    assert_eq!(code, 2, "{stderr}");
    assert!(stderr.contains("no `ridl.toml`"), "{stderr}");
    assert_eq!(stdout, "");
    assert!(!empty.join("vendor").exists());
}

/// Every file and directory under `dir`, with the bytes of each file, so a
/// test can assert that a command wrote nothing.
fn snapshot(dir: &std::path::Path) -> Vec<(std::path::PathBuf, Option<Vec<u8>>)> {
    let mut entries = Vec::new();
    let mut pending = vec![dir.to_path_buf()];
    while let Some(next) = pending.pop() {
        for entry in std::fs::read_dir(&next).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                entries.push((path.clone(), None));
                pending.push(path);
            } else {
                let bytes = std::fs::read(&path).unwrap();
                entries.push((path, Some(bytes)));
            }
        }
    }
    entries.sort();
    entries
}

/// `ridl vendor --check` exits 0 with no output on a match, 1 on drift of
/// any kind, and never fetches or writes: the consumer tree and the cache
/// are byte for byte the same after each run.
#[test]
fn vendor_check_reports_drift_without_network_or_writes() {
    let dir = TempDir::new("vendor-check");
    let stub = Stub::serving(unit_tar(COMMON_UNIT));
    let root = consumer(&dir, stub.url());
    assert_eq!(ridl(&root, dir.path(), &["vendor"]).0, 0);
    stub.reset_hits();
    let before = snapshot(dir.path());
    let (code, stdout, stderr) = ridl(&root, dir.path(), &["vendor", "--check"]);
    assert_eq!(code, 0, "{stderr}");
    assert_eq!(stdout, "");
    assert!(stderr.is_empty(), "a clean run prints nothing: {stderr}");
    assert_eq!(snapshot(dir.path()), before, "--check writes nothing");

    // A vendored file rewritten with CRLF line ends no longer matches.
    std::fs::write(
        root.join("vendor/veh.common/common.typl"),
        COMMON_UNIT[1].1.replace('\n', "\r\n"),
    )
    .unwrap();
    let before = snapshot(dir.path());
    let (code, stdout, stderr) = ridl(&root, dir.path(), &["vendor", "--check"]);
    assert_eq!(code, 1, "{stderr}");
    assert_eq!(stdout, "");
    assert!(stderr.contains("error[MANI-107]"), "{stderr}");
    assert!(stderr.contains("vendor/veh.common"), "{stderr}");
    assert_eq!(snapshot(dir.path()), before, "--check writes nothing");

    std::fs::create_dir_all(root.join("vendor/stray")).unwrap();
    std::fs::remove_dir_all(root.join("vendor/veh.common")).unwrap();
    let before = snapshot(dir.path());
    let (code, _, stderr) = ridl(&root, dir.path(), &["vendor", "--check"]);
    assert_eq!(code, 1, "{stderr}");
    assert!(
        stderr.contains("error[MANI-109]") && stderr.contains("error[MANI-110]"),
        "{stderr}"
    );
    assert!(!stderr.contains("MANI-107"), "{stderr}");
    assert_eq!(snapshot(dir.path()), before, "--check writes nothing");
    assert_eq!(stub.hits(), 0, "--check never fetches");
}

/// A vendored unit that `ridl.lock` does not pin cannot be verified:
/// MANI-103, exit 1, with a message that names `ridl vendor` and not
/// `--frozen`. Nothing is fetched and no lockfile is written.
#[test]
fn vendor_check_on_an_unpinned_unit_is_mani_103() {
    let dir = TempDir::new("vendor-check-unpinned");
    let stub = Stub::serving(unit_tar(COMMON_UNIT));
    let root = consumer(&dir, stub.url());
    vendor_common(&root);
    let (code, _, stderr) = ridl(&root, dir.path(), &["vendor", "--check"]);
    assert_eq!(code, 1, "{stderr}");
    assert!(stderr.contains("error[MANI-103]"), "{stderr}");
    assert!(stderr.contains("ridl vendor"), "{stderr}");
    assert!(!stderr.contains("--frozen"), "{stderr}");
    assert!(!root.join("ridl.lock").exists());
    assert_eq!(stub.hits(), 0);
}

/// A manifest that does not parse is reported, and `--check` exits 1
/// rather than reporting a clean `vendor/`.
#[test]
fn vendor_check_reports_a_broken_manifest() {
    let dir = TempDir::new("vendor-check-manifest");
    dir.write("app/ridl.toml", "[package\nname = \"app\"\n");
    dir.write("app/app.typl", "package app\n");
    let root = dir.path().join("app");
    let (code, stdout, stderr) = ridl(&root, dir.path(), &["vendor", "--check"]);
    assert_eq!(code, 1, "{stderr}");
    assert_eq!(stdout, "");
    assert!(stderr.contains("error[MANI-001]"), "{stderr}");
    assert!(!root.join("vendor").exists());
}

/// A missing path exits 2 before root discovery, even when the working
/// directory is inside a consumer whose `vendor/` would draw drift.
#[test]
fn vendor_check_on_a_missing_path_exits_2() {
    let dir = TempDir::new("vendor-check-missing");
    let stub = Stub::serving(unit_tar(COMMON_UNIT));
    let root = consumer(&dir, stub.url());
    let (code, stdout, stderr) = ridl(&root, dir.path(), &["vendor", "--check", "nowhere"]);
    assert_eq!(code, 2, "{stderr}");
    assert!(stderr.contains("nowhere"), "{stderr}");
    assert!(!stderr.contains("MANI-109"), "{stderr}");
    assert_eq!(stdout, "");
    assert_eq!(stub.hits(), 0);
}

/// A path with no `ridl.toml` at or above it exits 2.
#[test]
fn vendor_check_without_a_manifest_exits_2() {
    let dir = TempDir::new("vendor-check-no-manifest");
    std::fs::create_dir_all(dir.path().join("empty")).unwrap();
    let empty = dir.path().join("empty");
    let (code, stdout, stderr) = ridl(&empty, dir.path(), &["vendor", "--check"]);
    assert_eq!(code, 2, "{stderr}");
    assert!(stderr.contains("no `ridl.toml`"), "{stderr}");
    assert_eq!(stdout, "");
}

/// Run from a workspace member, `--check` compares the `vendor/` at the
/// workspace root.
#[test]
fn vendor_check_reads_the_workspace_root() {
    let dir = TempDir::new("vendor-check-ws");
    let stub = Stub::serving(unit_tar(COMMON_UNIT));
    dir.write("ws/ridl.toml", "[workspace]\nmembers = [\"app\"]\n");
    dir.write(
        "ws/app/ridl.toml",
        &format!(
            "[package]\nname = \"app\"\nversion = \"1.0.0\"\n\n[imports]\n\"veh.common\" = \"{}\"\n",
            stub.url()
        ),
    );
    dir.write("ws/app/app.typl", "package app\n");
    let member = dir.path().join("ws/app");
    let (code, _, stderr) = ridl(&member, dir.path(), &["vendor"]);
    assert_eq!(code, 0, "{stderr}");
    let (code, _, stderr) = ridl(&member, dir.path(), &["vendor", "--check"]);
    assert_eq!(code, 0, "{stderr}");
    assert!(stderr.is_empty(), "{stderr}");
    std::fs::remove_dir_all(dir.path().join("ws/vendor/veh.common")).unwrap();
    let (code, _, stderr) = ridl(&member, dir.path(), &["vendor", "--check"]);
    assert_eq!(code, 1, "{stderr}");
    assert!(stderr.contains("error[MANI-109]"), "{stderr}");
}
