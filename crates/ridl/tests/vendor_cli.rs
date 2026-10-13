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
