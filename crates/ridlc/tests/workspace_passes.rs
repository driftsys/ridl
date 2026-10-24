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
    // TYPL-406 (`missing-docs`) is left out: the fixture has no docs.
    let found: Vec<(&str, &str, &str)> = json
        .iter()
        .filter(|diagnostic| diagnostic.code != "TYPL-406")
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

/// `ridlc::std_ir` checks `ridl.std` in an empty workspace, and
/// `compile_workspace` checks it in the loaded workspace. The two promise the
/// same IR, so a consumer that holds only snapshots (`ridl diff`) resolves
/// `ridl.std` as a consumer of a compiled workspace does.
#[test]
fn std_ir_equals_the_std_ir_of_a_compiled_workspace() {
    let dir = tempfile::tempdir().expect("a temp dir");
    std::fs::write(
        dir.path().join("ridl.toml"),
        "[package]\nname = \"a\"\nversion = \"1.0.0\"\n",
    )
    .expect("write the manifest");
    std::fs::write(
        dir.path().join("a.ridl"),
        "package a\ntype Level: integer [0..3]\nstruct S {\n  at: Timestamp\n  level: Level\n}\n",
    )
    .expect("write the source");

    let mut db = ridl_core::RidlDatabase::default();
    let output = ridlc::compile_workspace(&mut db, dir.path()).expect("the workspace loads");

    let standalone = ridlc::std_ir();
    assert_eq!(standalone.name, "ridl.std");
    assert!(
        !standalone.decls.is_empty(),
        "ridl.std holds declarations: {standalone:?}"
    );
    assert_eq!(standalone, output.std_ir);
}
