//! Documentation in the source, end to end (typl §14, ADR-0026): a fixture
//! workspace with a doc comment on every carrier — declarations, members,
//! call parameters, the rsdl declarations and their member lines — with
//! links, `@see`, `@since`, `@labels` and `@deprecated` among them.
//!
//! Two checks the design mandates: the stored `links`, `see` and `since` of
//! every carrier are pinned in one IR snapshot, and deleting every doc
//! comment of the workspace leaves the catalog hash of every package
//! unchanged — a doc is never part of a package's identity, a `@deprecated`
//! reason included.

use std::collections::BTreeMap;

use ridl_core::TimingDefaults;
use ridl_core::db::{InputFile, RidlDatabase, profile_of_path};
use ridl_core::diag::Severity;
use ridl_core::package::{Package, PackageOrigin, Workspace};
use ridl_core::std_package;
use ridl_ir::catalog_hash::catalog_hash;
use ridl_ir::v2;
use ridl_sem::{check_package, check_system, lower_system, resolve_package};
use ridl_syntax::{SyntaxKind, parse};

/// The fixture workspace: `(package, path, text)` per file.
fn fixture() -> Vec<(&'static str, &'static str, String)> {
    vec![
        (
            "veh.common",
            "veh/common/common.typl",
            include_str!("../fixtures/docs/veh/common/common.typl").to_string(),
        ),
        (
            "veh.adas",
            "veh/adas/adas.ridl",
            include_str!("../fixtures/docs/veh/adas/adas.ridl").to_string(),
        ),
        (
            "veh.topology",
            "veh/topology/topology.rsdl",
            include_str!("../fixtures/docs/veh/topology/topology.rsdl").to_string(),
        ),
    ]
}

/// The compiled workspace: the IR of each package, in fixture order, the
/// lowered system, and every diagnostic as `(code, severity, message)`.
struct Compiled {
    packages: Vec<v2::Package>,
    system: Option<v2::System>,
    diagnostics: Vec<(String, Severity, String)>,
}

fn compile(files: &[(&str, &str, String)]) -> Compiled {
    let mut db = RidlDatabase::default();
    let std = std_package(&mut db);
    let packages: Vec<Package> = files
        .iter()
        .map(|(name, path, text)| {
            Package::new(
                &db,
                name.to_string(),
                vec![InputFile::new(&db, path.to_string(), text.clone())],
                PackageOrigin::WorkspaceMember,
                BTreeMap::new(),
                TimingDefaults::default(),
                None,
            )
        })
        .collect();
    let ws = Workspace::new(&db, packages.clone(), BTreeMap::new());
    let mut diagnostics = Vec::new();
    let mut record = |list: &[ridl_core::diag::Diagnostic]| {
        diagnostics.extend(list.iter().map(|diagnostic| {
            (
                diagnostic.code.as_str().to_string(),
                diagnostic.severity,
                diagnostic.message.clone(),
            )
        }));
    };
    let irs: Vec<v2::Package> = packages
        .iter()
        .map(|package| {
            record(&resolve_package(&db, ws, *package, std).diagnostics);
            let checked = check_package(&db, ws, *package, std);
            record(&checked.diagnostics);
            checked.ir
        })
        .collect();
    let system = check_system(&db, ws, std);
    record(&system.diagnostics);
    let refs: Vec<&v2::Package> = irs.iter().collect();
    let lowered = lower_system(&system, &refs);
    Compiled {
        packages: irs,
        system: lowered,
        diagnostics,
    }
}

/// `text` with every doc comment token removed, through the lexer, so a
/// `/** */` block and a `///` line go the same way and nothing else changes.
fn strip_doc_comments(path: &str, text: &str) -> String {
    parse(text, profile_of_path(path))
        .syntax()
        .descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .filter(|token| token.kind() != SyntaxKind::DocComment)
        .map(|token| token.text().to_string())
        .collect()
}

/// The fixture compiles with no diagnostic — every link and `@see` resolves
/// — and the IR of every package and the system is pinned, so the stored
/// `doc`, `links`, `see`, `since`, `labels` and `deprecated` of every carrier
/// are visible in one place.
#[test]
fn every_carrier_doc_reaches_the_ir() {
    let compiled = compile(&fixture());
    assert_eq!(compiled.diagnostics, Vec::new());
    let mut json = String::new();
    for package in &compiled.packages {
        json.push_str(&v2::to_json_pretty(package).expect("a package serializes as IR JSON"));
        json.push('\n');
    }
    let system = compiled.system.as_ref().expect("the closure lowers");
    json.push_str(&v2::system_to_json_pretty(system).expect("the system serializes as IR JSON"));
    insta::assert_snapshot!("docs_fixture_ir", json);
}

/// The deletion test (ADR-0026): the catalog hash of every package is the
/// same with and without the workspace's doc comments. A `@deprecated` with
/// a reason goes with the docs, so the test also pins that `deprecated` is
/// outside the hash.
#[test]
fn deleting_every_doc_comment_keeps_every_catalog_hash() {
    let documented = compile(&fixture());
    let stripped: Vec<(&str, &str, String)> = fixture()
        .into_iter()
        .map(|(name, path, text)| (name, path, strip_doc_comments(path, &text)))
        .collect();
    let bare = compile(&stripped);
    assert!(
        bare.diagnostics
            .iter()
            .all(|(_, severity, _)| *severity != Severity::Error),
        "{:?}",
        bare.diagnostics
    );

    // The stripping was real on both sides: the documented IR carries a
    // deprecation and links, the bare IR carries no doc at all.
    let decls = |compiled: &Compiled| -> Vec<v2::Decl> {
        compiled
            .packages
            .iter()
            .flat_map(|package| package.decls.iter().cloned())
            .collect()
    };
    assert!(
        decls(&documented)
            .iter()
            .any(|decl| decl.deprecated.is_some())
    );
    assert!(decls(&documented).iter().any(|decl| !decl.links.is_empty()));
    for decl in decls(&bare) {
        assert!(decl.doc.is_empty(), "{}", decl.name);
        assert!(decl.deprecated.is_none(), "{}", decl.name);
        assert!(
            decl.links.is_empty() && decl.see.is_empty() && decl.since.is_empty(),
            "{}",
            decl.name
        );
    }

    for (with, without) in documented.packages.iter().zip(&bare.packages) {
        assert_eq!(with.name, without.name);
        let others = |compiled: &'_ Compiled| -> Vec<v2::Package> {
            compiled
                .packages
                .iter()
                .filter(|package| package.name != with.name)
                .cloned()
                .collect()
        };
        let (others_with, others_without) = (others(&documented), others(&bare));
        assert_eq!(
            catalog_hash(with, &others_with.iter().collect::<Vec<_>>()),
            catalog_hash(without, &others_without.iter().collect::<Vec<_>>()),
            "the catalog hash of `{}` changed when its docs were deleted",
            with.name
        );
    }
}
