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
