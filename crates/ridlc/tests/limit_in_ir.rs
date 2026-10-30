//! A declared payload limit reaches the checked IR (ADR-0030 decision 13): the
//! interaction `Decl` carries `limit` when a `[limits]` entry gives it one, and
//! the catalog hash covers it.

use ridl_core::RidlDatabase;
use ridl_core::diag::Severity;
use ridl_ir::catalog_hash::catalog_hash;
use ridl_ir::v2;
use ridlc::{WorkspaceOutput, compile_workspace};

const SOURCE: &str = "package demo\ntype Payload: bytes [0..100]\n\
                      interface Iface {\n  signal sig: Payload @[10ms..100ms]\n  \
                      event ev: Payload\n  query ask(): Payload\n}\n\
                      service demo.svc {\n  signal sig: Payload @[10ms..100ms]\n}\n";

/// Compiles a one-package workspace whose manifest ends with `limits`, with
/// `source` as its only file.
fn compile(source: &str, limits: &str) -> WorkspaceOutput {
    let dir = tempfile::tempdir().expect("a temp dir");
    std::fs::write(
        dir.path().join("ridl.toml"),
        format!("[package]\nname = \"demo\"\nversion = \"1.0.0\"\n{limits}"),
    )
    .expect("write the manifest");
    std::fs::write(dir.path().join("m.ridl"), source).expect("write the source");
    let mut db = RidlDatabase::default();
    compile_workspace(&mut db, dir.path()).expect("the package loads")
}

fn errors(output: &WorkspaceOutput) -> Vec<String> {
    output
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.severity == Severity::Error)
        .map(|diagnostic| format!("{} {}", diagnostic.code.as_str(), diagnostic.message))
        .collect()
}

/// The limit of every interaction of the package `demo`, as `Shape.member`.
fn limits_of(output: &WorkspaceOutput) -> Vec<(String, Option<u32>)> {
    let ir = &output
        .checked
        .iter()
        .find(|package| package.ir.name == "demo")
        .expect("the package is checked")
        .ir;
    ir.shapes()
        .flat_map(|shape| {
            let short = shape.name.rsplit('.').next().unwrap().to_string();
            shape
                .interface
                .interactions
                .iter()
                .map(move |decl| (format!("{short}.{}", decl.name), decl.limit))
        })
        .collect()
}

fn limit(output: &WorkspaceOutput, name: &str) -> Option<u32> {
    limits_of(output)
        .into_iter()
        .find(|(entry, _)| entry == name)
        .unwrap_or_else(|| panic!("no interaction {name}"))
        .1
}

fn hash(output: &WorkspaceOutput) -> [u8; 32] {
    let packages: Vec<&v2::Package> = output.checked.iter().map(|p| &p.ir).collect();
    catalog_hash("demo", &packages)
}

#[test]
fn a_name_entry_is_recorded_in_the_ir() {
    let output = compile(SOURCE, "[limits.interaction]\n\"Iface.sig\" = 300\n");
    assert_eq!(errors(&output), Vec::<String>::new());
    assert_eq!(limit(&output, "Iface.sig"), Some(300));
    assert_eq!(limit(&output, "Iface.ev"), None);
    assert_eq!(limit(&output, "Iface.ask"), None);
    assert_eq!(limit(&output, "svc.sig"), None);
}

#[test]
fn an_inline_service_entry_is_recorded_in_the_ir() {
    let output = compile(SOURCE, "[limits.interaction]\n\"svc.sig\" = 250\n");
    assert_eq!(errors(&output), Vec::<String>::new());
    assert_eq!(limit(&output, "svc.sig"), Some(250));
    assert_eq!(limit(&output, "Iface.sig"), None);
}

#[test]
fn kind_and_default_are_recorded() {
    let output = compile(
        SOURCE,
        "[limits]\ndefault = 400\n[limits.kind]\nsignal = 300\n\
         [limits.interaction]\n\"Iface.ev\" = 200\n",
    );
    assert_eq!(errors(&output), Vec::<String>::new());
    // The name beats the kind, and the kind beats the default.
    assert_eq!(limit(&output, "Iface.ev"), Some(200));
    assert_eq!(limit(&output, "Iface.sig"), Some(300));
    assert_eq!(limit(&output, "svc.sig"), Some(300));
    assert_eq!(limit(&output, "Iface.ask"), Some(400));
}

#[test]
fn no_table_records_nothing() {
    let output = compile(SOURCE, "");
    assert_eq!(limits_of(&output).len(), 4);
    assert!(limits_of(&output).iter().all(|(_, limit)| limit.is_none()));
}

#[test]
fn a_table_that_reaches_nothing_leaves_the_catalog_hash_unchanged() {
    let without = compile(SOURCE, "");
    // The source has no `fixed`, so this entry applies to no interaction.
    let unreached = compile(SOURCE, "[limits.kind]\nfixed = 10\n");
    assert_eq!(errors(&unreached), Vec::<String>::new());
    assert!(
        limits_of(&unreached)
            .iter()
            .all(|(_, limit)| limit.is_none())
    );
    assert_eq!(hash(&without), hash(&unreached));
    // A limit that does reach an interaction changes the hash.
    let reached = compile(SOURCE, "[limits.interaction]\n\"Iface.sig\" = 300\n");
    assert_ne!(hash(&without), hash(&reached));
}

#[test]
fn an_ambiguous_short_name_falls_back_to_kind() {
    // Two inline services share the short name `svc`, so no name entry can
    // apply to either; the kind entry still does.
    let source = "package demo\ntype Payload: bytes [0..100]\n\
                  service demo.a.svc {\n  signal sig: Payload @[10ms..100ms]\n}\n\
                  service demo.b.svc {\n  signal sig: Payload @[10ms..100ms]\n}\n";
    let output = compile(
        source,
        "[limits.kind]\nsignal = 300\n[limits.interaction]\n\"svc.sig\" = 10\n",
    );
    let limits = limits_of(&output);
    assert_eq!(limits.len(), 2);
    assert!(
        limits.iter().all(|(_, limit)| *limit == Some(300)),
        "{limits:?}"
    );
    // With no kind or default entry, nothing applies.
    let output = compile(source, "[limits.interaction]\n\"svc.sig\" = 10\n");
    assert!(limits_of(&output).iter().all(|(_, limit)| limit.is_none()));
}

#[test]
fn a_name_is_ambiguous_across_the_packages_of_a_unit() {
    let main = "package demo\ntype Payload: bytes [0..100]\n\
                interface Iface {\n  signal sig: Payload @[10ms..100ms]\n}\n";
    let sub = "package demo.sub\ntype Payload: bytes [0..100]\n\
               interface Iface {\n  signal sig: Payload @[10ms..100ms]\n}\n";
    let dir = tempfile::tempdir().expect("a temp dir");
    std::fs::write(
        dir.path().join("ridl.toml"),
        "[package]\nname = \"demo\"\nversion = \"1.0.0\"\n\
         [limits.kind]\nsignal = 300\n[limits.interaction]\n\"Iface.sig\" = 10\n",
    )
    .unwrap();
    std::fs::write(dir.path().join("m.ridl"), main).unwrap();
    std::fs::create_dir(dir.path().join("sub")).unwrap();
    std::fs::write(dir.path().join("sub/s.ridl"), sub).unwrap();
    let mut db = RidlDatabase::default();
    let output = compile_workspace(&mut db, dir.path()).unwrap();
    let mut seen = 0;
    for package in &output.checked {
        for shape in package.ir.shapes() {
            for decl in &shape.interface.interactions {
                assert_eq!(decl.limit, Some(300), "{}.{}", shape.name, decl.name);
                seen += 1;
            }
        }
    }
    assert_eq!(seen, 2);
}

#[test]
fn a_payload_above_its_limit_still_has_the_limit_recorded() {
    // The payload is above the 10-byte limit, so RIDL-151 is an error, and
    // the checked IR carries the limit all the same.
    let output = compile(SOURCE, "[limits.interaction]\n\"Iface.sig\" = 10\n");
    let found = errors(&output);
    assert!(
        found.iter().any(|error| error.starts_with("RIDL-151")),
        "{found:?}"
    );
    assert_eq!(limit(&output, "Iface.sig"), Some(10));
}

#[test]
fn a_limits_table_stays_inside_its_unit() {
    // Two units in one workspace: `a` has a `[limits]` table and `b` has none.
    // The table of `a` must not reach the interactions of `b`.
    let source = |name: &str| {
        format!(
            "package {name}\ntype Payload: bytes [0..100]\n\
             interface Iface {{\n  signal sig: Payload @[10ms..100ms]\n}}\n"
        )
    };
    let dir = tempfile::tempdir().expect("a temp dir");
    std::fs::write(
        dir.path().join("ridl.toml"),
        "[workspace]\nmembers = [\"a\", \"b\"]\n",
    )
    .unwrap();
    for (name, manifest) in [("a", "[limits]\ndefault = 300\n"), ("b", "")] {
        let member = dir.path().join(name);
        std::fs::create_dir(&member).unwrap();
        std::fs::write(
            member.join("ridl.toml"),
            format!("[package]\nname = \"{name}\"\nversion = \"1.0.0\"\n{manifest}"),
        )
        .unwrap();
        std::fs::write(member.join("m.ridl"), source(name)).unwrap();
    }
    let mut db = RidlDatabase::default();
    let output = compile_workspace(&mut db, dir.path()).expect("the workspace loads");
    assert_eq!(errors(&output), Vec::<String>::new());
    let limits_in = |name: &str| -> Vec<Option<u32>> {
        output
            .checked
            .iter()
            .filter(|package| package.ir.name == name)
            .flat_map(|package| package.ir.shapes())
            .flat_map(|shape| shape.interface.interactions.iter().map(|decl| decl.limit))
            .collect()
    };
    assert_eq!(limits_in("a"), vec![Some(300)]);
    assert_eq!(limits_in("b"), vec![None]);
}

#[test]
fn an_interface_tombstone_keeps_its_limit_unset() {
    let source = "package demo\ntype Payload: bytes [0..100]\n\
                  interface Iface {\n  signal sig: Payload @[10ms..100ms]\n  reserved old\n}\n";
    let output = compile(source, "[limits]\ndefault = 300\n");
    assert_eq!(errors(&output), Vec::<String>::new());
    let ir = &output
        .checked
        .iter()
        .find(|package| package.ir.name == "demo")
        .expect("the package is checked")
        .ir;
    let tombstones: Vec<&v2::Decl> = ir
        .shapes()
        .flat_map(|shape| shape.interface.interactions.iter())
        .filter(|decl| matches!(decl.kind, Some(v2::decl::Kind::ReservedSlot(_))))
        .collect();
    assert_eq!(tombstones.len(), 1, "the interface holds one tombstone");
    assert_eq!(tombstones[0].limit, None);
    assert_eq!(limit(&output, "Iface.sig"), Some(300));
}

/// A raised limit (ADR-0030 decision 18) is an entry like any other: it is
/// recorded on the interaction and changes the catalog hash, while a unit with
/// no `[limits]` table keeps its hash.
#[test]
fn a_raised_limit_is_recorded_and_changes_the_hash() {
    let without = compile(SOURCE, "");
    assert_eq!(hash(&without), hash(&compile(SOURCE, "")));
    let raised = compile(SOURCE, "[limits.interaction]\n\"Iface.sig\" = 16128\n");
    assert_eq!(errors(&raised), Vec::<String>::new());
    assert_eq!(limit(&raised, "Iface.sig"), Some(16_128));
    assert_ne!(hash(&without), hash(&raised));
}

/// `default` applies to each kind up to the default ceiling of that kind
/// (ADR-0030 decision 12), and the clamped value is what the IR records: an
/// entry applied, so the limit is part of the catalog.
#[test]
fn a_clamped_default_records_the_ceiling_of_the_kind() {
    let output = compile(SOURCE, "[limits]\ndefault = 65280\n");
    assert_eq!(errors(&output), Vec::<String>::new());
    assert_eq!(limit(&output, "Iface.sig"), Some(3_840));
    assert_eq!(limit(&output, "Iface.ev"), Some(3_840));
    assert_eq!(limit(&output, "Iface.ask"), Some(16_128));
}

/// An interaction entry above 16,128 bytes that names a member other than a
/// fixed is MANI-022 and is dropped, so the IR records what the next entry
/// gives, or nothing.
#[test]
fn a_dropped_entry_records_nothing() {
    let output = compile(SOURCE, "[limits.interaction]\n\"Iface.sig\" = 16129\n");
    assert_eq!(errors(&output).len(), 1, "{:?}", errors(&output));
    assert!(errors(&output)[0].starts_with("MANI-022 "));
    assert_eq!(limit(&output, "Iface.sig"), None);
    let output = compile(
        SOURCE,
        "[limits.kind]\nsignal = 7\n[limits.interaction]\n\"Iface.sig\" = 16129\n",
    );
    assert_eq!(limit(&output, "Iface.sig"), Some(7));
}
