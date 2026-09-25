//! `ridlc::run_check` over a workspace read from disk: the workspace-wide
//! passes (`ridl_sem::check_workspace`) index the workspace's source files in
//! package-then-file order, and a package's `interfaces.lock` is not one of
//! them.

use ridl_core::Frozen;
use ridl_core::diag::to_json;

/// A two-member workspace whose first member carries an `interfaces.lock`:
/// the RIDL-140 and RSDL-602 raised in the second member are reported on the
/// second member's files, not moved by one file onto the lock.
#[test]
fn a_lock_in_an_earlier_package_does_not_move_the_workspace_diagnostics() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let write = |relative: &str, text: &str| {
        let path = dir.path().join(relative);
        std::fs::create_dir_all(path.parent().expect("a parent")).expect("create the directory");
        std::fs::write(path, text).expect("write the fixture file");
    };
    write("ridl.toml", "[workspace]\nmembers = [\"a\", \"b\"]\n");
    for member in ["a", "b"] {
        write(
            &format!("{member}/ridl.toml"),
            &format!("[package]\nname = \"{member}\"\nversion = \"1.0.0\"\n"),
        );
    }
    write("a/a.ridl", "package a\ninterface I {}\nservice x.s : I\n");
    // What `ridl lock` writes for `a/a.ridl`.
    write(
        "a/interfaces.lock",
        "# interfaces.lock — written by ridl lock; do not edit by hand.\nnext 2\nI 1\n",
    );
    write("b/b.ridl", "package b\ninterface J {}\nservice x.s : J\n");
    write("b/sys.rsdl", "package b\nsystem S { Missing }\n");

    let run = ridlc::run_check(dir.path(), Frozen::Yes).expect("the workspace loads");
    let json = to_json(&run.diagnostics, &run.sources);
    let found: Vec<(&str, &str, &str)> = json
        .iter()
        .map(|diagnostic| {
            let label = diagnostic
                .labels
                .first()
                .map_or("", |label| label.span.path.as_str());
            (
                diagnostic.code.as_str(),
                diagnostic.span.path.as_str(),
                label,
            )
        })
        .collect();
    let path = |relative: &str| dir.path().join(relative).display().to_string();
    let (a, b, sys) = (path("a/a.ridl"), path("b/b.ridl"), path("b/sys.rsdl"));
    assert_eq!(
        found,
        [
            ("RIDL-140", b.as_str(), a.as_str()),
            ("RSDL-602", sys.as_str(), ""),
        ],
    );
}
