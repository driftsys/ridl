//! The `[defaults].command_timing` and `[defaults].query_timing` keys of a
//! `ridl.toml` reach the lowered IR end to end: a standalone package is loaded
//! from disk, checked, and lowered, and every `command` and `query` that writes
//! no response bound carries the configured default (ridl §9.3).

use ridl_core::RidlDatabase;
use ridl_core::diag::Severity;
use ridl_ir::v2;
use ridlc::compile_workspace;

const MANIFEST: &str = "[package]\nname = \"p\"\nversion = \"1.0.0\"\n\n[defaults]\n\
                        command_timing = \"[..2s]\"\nquery_timing = \"[5ms..4s]\"\n";

const SOURCE: &str = "package p\n\ntype Level: integer [0..10]\n\ninterface I {\n  \
                      command set(level: Level)\n  query get(): Level\n  \
                      command throttled(level: Level) @[1ms..]\n}\n";

/// The resolved timing of the interaction `name` in `ir`.
fn timing<'a>(ir: &'a v2::Package, name: &str) -> &'a v2::Timing {
    let decl = ir
        .shapes()
        .flat_map(|shape| shape.interface.interactions.iter())
        .find(|decl| decl.name == name)
        .unwrap_or_else(|| panic!("`{name}` is lowered"));
    match &decl.kind {
        Some(v2::decl::Kind::CommandDef(command)) => command.timing.as_ref(),
        Some(v2::decl::Kind::QueryDef(query)) => query.timing.as_ref(),
        other => panic!("`{name}` is not a command or query: {other:?}"),
    }
    .unwrap_or_else(|| panic!("`{name}` carries a timing"))
}

#[test]
fn manifest_rpc_defaults_reach_the_lowered_ir() {
    let dir = tempfile::tempdir().expect("a temp dir");
    std::fs::write(dir.path().join("ridl.toml"), MANIFEST).expect("write the manifest");
    std::fs::write(dir.path().join("p.ridl"), SOURCE).expect("write the source");

    let mut db = RidlDatabase::default();
    let output = compile_workspace(&mut db, dir.path()).expect("the package loads");
    let errors: Vec<_> = output
        .diagnostics
        .iter()
        .filter(|diag| diag.severity == Severity::Error)
        .collect();
    assert!(errors.is_empty(), "no error diagnostics: {errors:?}");
    let ir = &output
        .checked
        .iter()
        .find(|package| package.ir.name == "p")
        .expect("the package is checked")
        .ir;

    let command = timing(ir, "set");
    assert_eq!(command.min_us, None);
    assert_eq!(command.max_us.as_deref(), Some("2000000"));
    assert!(command.default_applied);

    let query = timing(ir, "get");
    assert_eq!(query.min_us.as_deref(), Some("5000"));
    assert_eq!(query.max_us.as_deref(), Some("4000000"));
    assert!(query.default_applied);

    // `@[1ms..]` keeps its written minimum and takes only the maximum.
    let throttled = timing(ir, "throttled");
    assert_eq!(throttled.min_us.as_deref(), Some("1000"));
    assert_eq!(throttled.max_us.as_deref(), Some("2000000"));
    assert!(throttled.default_applied);
}
