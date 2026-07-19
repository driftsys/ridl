//! The salsa package model (docs/ROADMAP.md epic E1.3, ADR-0002 §1).
//!
//! A **package is a directory** (ADR-0002 §1): a named set of [`InputFile`]s
//! loaded as one unit, carrying its own manifest's `[imports]`. A
//! [`Workspace`] holds every loaded [`Package`] plus the workspace root's
//! `[imports]`, kept separate so the ADR-0002 §5 resolution order stays
//! expressible per member. Both are salsa inputs — the filesystem loader
//! ([`load_workspace`](crate::workspace::load_workspace), behind the `fs`
//! feature) builds them; queries downstream read them through the database,
//! so an edit to one file invalidates only that file's parse.
//!
//! This module touches no filesystem: it defines the inputs, the
//! [`package_of`] lookup query, and the pure package-declaration reader the
//! loader's law checks build on.

use std::collections::{BTreeMap, HashMap, HashSet};

use ridl_syntax::ast::{AstNode as _, ServiceDef, SourceFile};
use ridl_syntax::{SyntaxKind, SyntaxNode};
use rowan::TextRange;

use crate::db::InputFile;
use crate::diag::{DiagCode, Diagnostic, Label, Severity, SourceMap, Span};

/// Where a package's sources come from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PackageOrigin {
    /// Loaded from the local filesystem: a workspace member, a standalone
    /// package, or a single-file synthetic package.
    WorkspaceMember,
    /// Fetched from a remote URL (E1.6).
    Remote,
    /// The embedded `ridl.std` (ADR-0007 decision 15).
    Std,
}

/// One loaded package: its dotted name (e.g. `veh.common`), its files, where
/// it came from, and its own manifest's `[imports]` map.
///
/// `imports` is ADR-0002 §5 step 2: the `[imports]` of the manifest governing
/// this package's directory tree. It shadows the workspace `[imports]` for
/// this package only — a member's pin is never visible to a sibling member.
/// Every package loaded from one manifest's tree (the root and its
/// subdirectory packages) carries that manifest's map; a single-file package
/// and `ridl.std` carry an empty map.
#[salsa::input(debug)]
pub struct Package {
    pub name: String,
    #[returns(ref)]
    pub files: Vec<InputFile>,
    pub origin: PackageOrigin,
    #[returns(ref)]
    pub imports: BTreeMap<String, String>,
    /// The raw `[defaults].timing` string that governs this package (ADR-0002
    /// §5 precedence: package `[defaults]` shadows workspace `[defaults]`,
    /// merged at load), or `None` when no manifest configures one — the
    /// checker then applies the built-in `[100ms..1000ms]` (ridl §9.1). Stored
    /// unparsed: `ridl-core` cannot depend on `ridl-sem` (E2 task 9).
    #[returns(ref)]
    pub default_timing: Option<String>,
}

/// Every loaded package plus the workspace root's own `[imports]` map.
///
/// `imports` is ADR-0002 §5 step 3 only — the shared default for every
/// member. It is **not** merged with member maps: the task 9 resolver walks
/// the order itself — workspace member ([`package_of`]) → the referencing
/// package's own [`Package::imports`] → this map → error. In a standalone
/// package load the manifest's `[imports]` ride on the packages and this map
/// is empty.
#[salsa::input(debug)]
pub struct Workspace {
    #[returns(ref)]
    pub packages: Vec<Package>,
    #[returns(ref)]
    pub imports: BTreeMap<String, String>,
}

/// The package named `name` in `ws`, or `None` when no loaded package carries
/// that name. The embedded `ridl.std` is not part of `ws` — the resolver
/// reaches it through [`std_package`](crate::std_lib::std_package).
#[salsa::tracked(returns(clone))]
pub fn package_of(db: &dyn salsa::Database, ws: Workspace, name: String) -> Option<Package> {
    ws.packages(db)
        .iter()
        .copied()
        .find(|package| package.name(db) == &name)
}

/// Every `package` declaration in a parsed file, in source order, as the
/// dotted name plus the declaration's source range. The loader's law checks
/// read this: the first entry names the file's package, every later entry is a
/// TYPL-001, and a first entry that does not match the directory's expected
/// name is a TYPL-002.
///
/// The only caller outside the tests is the `fs`-gated loader, so the
/// `--no-default-features` build allows the helper to be unused.
#[cfg_attr(not(feature = "fs"), allow(dead_code))]
pub(crate) fn package_declarations(file: &SourceFile) -> Vec<(String, TextRange)> {
    file.syntax()
        .children()
        .filter_map(ridl_syntax::ast::PackageDecl::cast)
        .map(|decl| {
            let name = decl
                .qualified_name()
                .map(|name| dotted_name(&name))
                .unwrap_or_default();
            (name, decl.syntax().text_range())
        })
        .collect()
}

/// The dotted text of a qualified name, with any trivia between its tokens
/// dropped (`veh . common` reads as `veh.common`).
#[cfg_attr(not(feature = "fs"), allow(dead_code))]
fn dotted_name(name: &ridl_syntax::ast::QualifiedName) -> String {
    name.syntax()
        .children_with_tokens()
        .filter_map(|element| element.into_token())
        .filter(|token| !token.kind().is_trivia())
        .map(|token| token.text().to_string())
        .collect()
}

// ==========================================================================
// The service catalog (E2.13, ridl reference §14.5)
// ==========================================================================

/// One entry of the [`ServiceCatalog`]: the package that declared a service
/// and the interface it names.
///
/// `interface_ref` is the canonical interface reference — a bare `Name` for a
/// same-package or unresolved shape, a fully qualified `pkg.Name` when the
/// name resolves through the declaring file's imports — or the empty string
/// for a service with an inline shape (ridl §14.5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogEntry {
    pub package: String,
    pub interface_ref: String,
}

/// The system-wide service catalog: every `service` declaration across the
/// workspace keyed by its dotted global name (ridl §14.5).
///
/// The catalog is the flat global namespace the whole system agrees on. A
/// dotted name declared twice anywhere in the workspace is a RIDL-140 error
/// (both declarations labeled). The diagnostics carry a [`FileId`] indexing
/// the workspace's files in package-then-file order — the order
/// [`service_catalog`] interns them — so a driver remaps them onto its render
/// [`SourceMap`] exactly as it does the package-scoped passes.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ServiceCatalog {
    pub entries: BTreeMap<String, CatalogEntry>,
    pub diagnostics: Vec<Diagnostic>,
}

/// Builds the workspace-wide [`ServiceCatalog`] and reports the RIDL-140
/// duplicate-name diagnostics (ridl §14.5, §16.4).
///
/// The `std` package is threaded in for signature parity with the
/// package-scoped passes (`resolve_package`, `check_package`); `ridl.std`
/// declares no services, so it contributes nothing here. The catalog owns the
/// global namespace: a service's dotted name is deliberately not a
/// [`SymbolKind`](../../ridl_sem/resolve/enum.SymbolKind.html) — it lives in
/// this namespace, not the type namespace — so the resolver never sees it and
/// the catalog is the sole authority on service uniqueness.
#[salsa::tracked(returns(clone))]
pub fn service_catalog(db: &dyn salsa::Database, ws: Workspace, std: Package) -> ServiceCatalog {
    // `ridl.std` carries no services; the parameter keeps the signature
    // uniform with the package-scoped passes the callers already drive.
    let _ = std;

    // Diagnostics point into the workspace's files; their FileId indexes every
    // file in package-then-file order, the order interned here. A driver
    // rebuilds the same order to remap onto its render source map.
    let mut sources = SourceMap::new();

    let mut entries: BTreeMap<String, CatalogEntry> = BTreeMap::new();
    let mut first_span: HashMap<String, Span> = HashMap::new();
    let mut diagnostics = Vec::new();

    for package in ws.packages(db) {
        let package_name = package.name(db).clone();
        // The package-wide binding view the resolver builds: local
        // declarations and imports are collected across every file of the
        // package, and a local declaration shadows an import.
        let names = package_names(db, package);
        for file in package.files(db) {
            let file_id = sources.file_id(file.path(db), file.text(db));
            let source = service_source(db, *file);
            for service in source.services() {
                let Some(dotted) = service.name() else {
                    continue;
                };
                let name = significant_node_text(dotted.syntax());
                if name.is_empty() {
                    continue;
                }
                let span = Span {
                    file: file_id,
                    range: dotted.syntax().text_range(),
                };
                if let Some(&first) = first_span.get(&name) {
                    // RIDL-140: the flat global namespace already holds this
                    // name. Label both declarations (the first via a secondary
                    // label, the duplicate as the primary span).
                    diagnostics.push(Diagnostic {
                        code: DiagCode::RIDL_140,
                        severity: Severity::Error,
                        message: format!(
                            "duplicate service name `{name}` — the service catalog is a flat global namespace"
                        ),
                        primary: span,
                        labels: vec![Label {
                            span: first,
                            message: format!("`{name}` is first declared here"),
                        }],
                        fixits: Vec::new(),
                    });
                    continue;
                }
                first_span.insert(name.clone(), span);
                entries.insert(
                    name,
                    CatalogEntry {
                        package: package_name.clone(),
                        interface_ref: canonical_interface_ref(&names, &service),
                    },
                );
            }
        }
    }

    ServiceCatalog {
        entries,
        diagnostics,
    }
}

/// The parsed [`SourceFile`] of one input, through the memoized parse query.
fn service_source(db: &dyn salsa::Database, file: InputFile) -> SourceFile {
    let parse = crate::parse_file(db, file);
    SourceFile::cast(parse.syntax()).expect("the parser roots every tree in a SourceFile")
}

/// The canonical interface reference a service names, or the empty string for
/// an inline shape.
///
/// A multi-segment reference is already package-qualified and is kept as
/// written. A single-segment reference resolves against the **package-wide**
/// binding view ([`package_names`]), in the resolver's precedence: a local
/// declaration anywhere in the package shadows an import, and an import bound
/// in any file of the package canonicalizes the name to its full `pkg.Name`
/// path. An unresolved name stays bare. This is the same bare/qualified split
/// the checker's IR lowering produces, so the catalog and the IR always agree.
fn canonical_interface_ref(names: &PackageNames, service: &ServiceDef) -> String {
    let Some(path) = service.interface_ref() else {
        return String::new();
    };
    let written = significant_node_text(path.syntax());
    if written.is_empty() || written.contains('.') {
        return written;
    }
    // A local declaration shadows an import (ridl-sem `apply_imports`), so a
    // name declared anywhere in this package stays bare.
    if names.locals.contains(&written) {
        return written;
    }
    if let Some(full) = names.imports.get(&written) {
        return full.clone();
    }
    written
}

/// The package-wide names a reference can bind to, mirroring the resolver's
/// precedence: local declarations (any file) shadow imports (any file).
struct PackageNames {
    locals: HashSet<String>,
    imports: HashMap<String, String>,
}

fn package_names(db: &dyn salsa::Database, package: &Package) -> PackageNames {
    let mut locals = HashSet::new();
    let mut imports: HashMap<String, String> = HashMap::new();
    for file in package.files(db) {
        let source = service_source(db, *file);
        for definition in source.definitions() {
            if let Some(name) = declaration_name(definition.syntax()) {
                locals.insert(name);
            }
        }
        for interface in source.interfaces() {
            if let Some(name) = declaration_name(interface.syntax()) {
                locals.insert(name);
            }
        }
        for import in source.imports() {
            let Some(qualified) = import.qualified_name() else {
                continue;
            };
            let full = significant_node_text(qualified.syntax());
            let base = full.rsplit('.').next().unwrap_or(full.as_str()).to_string();
            let local = import
                .alias()
                .and_then(|alias| alias.ident_token())
                .map(|token| token.text().to_string())
                .unwrap_or(base);
            // Among colliding imports the first wins, as in `apply_imports`.
            imports.entry(local).or_insert(full);
        }
    }
    PackageNames { locals, imports }
}

/// The identifier of a declaration's `Name` child.
fn declaration_name(node: &SyntaxNode) -> Option<String> {
    node.children()
        .find(|child| child.kind() == SyntaxKind::Name)?
        .children_with_tokens()
        .filter_map(|element| element.into_token())
        .find(|token| token.kind() == SyntaxKind::Ident)
        .map(|token| token.text().to_string())
}

/// The concatenation of every non-trivia token in `node`'s subtree — the
/// node's text with whitespace and comments removed (`veh . adas . cruise`
/// reads as `veh.adas.cruise`).
fn significant_node_text(node: &SyntaxNode) -> String {
    node.descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .filter(|token| !token.kind().is_trivia())
        .map(|token| token.text().to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::RidlDatabase;

    fn file(db: &RidlDatabase, path: &str, text: &str) -> InputFile {
        InputFile::new(db, path.to_string(), text.to_string())
    }

    #[test]
    fn package_of_finds_a_package_by_name() {
        let db = RidlDatabase::default();
        let common = Package::new(
            &db,
            "veh.common".to_string(),
            vec![file(&db, "veh-common/a.typl", "package veh.common")],
            PackageOrigin::WorkspaceMember,
            BTreeMap::new(),
            None,
        );
        let cluster = Package::new(
            &db,
            "veh.cluster".to_string(),
            vec![file(&db, "veh-cluster/b.typl", "package veh.cluster")],
            PackageOrigin::WorkspaceMember,
            BTreeMap::new(),
            None,
        );
        let ws = Workspace::new(&db, vec![common, cluster], BTreeMap::new());

        assert_eq!(
            package_of(&db, ws, "veh.cluster".to_string()),
            Some(cluster)
        );
        assert_eq!(package_of(&db, ws, "veh.common".to_string()), Some(common));
        assert_eq!(package_of(&db, ws, "veh.absent".to_string()), None);
    }

    #[test]
    fn package_declarations_reads_names_and_ranges() {
        let parse = ridl_syntax::parse(
            "package veh.common\npackage veh.extra\ntype A: m\n",
            ridl_syntax::Profile::Typl,
        );
        let source = SourceFile::cast(parse.syntax()).expect("root is a SourceFile");
        let decls = package_declarations(&source);
        assert_eq!(decls.len(), 2, "both declarations are read");
        assert_eq!(decls[0].0, "veh.common");
        assert_eq!(decls[0].1, TextRange::new(0.into(), 18.into()));
        assert_eq!(decls[1].0, "veh.extra");
        assert_eq!(decls[1].1, TextRange::new(19.into(), 36.into()));
    }

    // --- the service catalog (E2.13, ridl reference §14.5) ---------------

    fn ridl_package(db: &RidlDatabase, name: &str, text: &str) -> Package {
        Package::new(
            db,
            name.to_string(),
            vec![file(db, &format!("{}.ridl", name.replace('.', "/")), text)],
            PackageOrigin::WorkspaceMember,
            BTreeMap::new(),
            None,
        )
    }

    #[test]
    fn service_catalog_flags_a_duplicate_name_across_packages() {
        use crate::std_lib::std_package;
        let mut db = RidlDatabase::default();
        let std = std_package(&mut db);
        // Two packages both declare `veh.adas.cruise` — the flat global
        // namespace forbids it (RIDL-140).
        let adas = ridl_package(
            &db,
            "veh.adas",
            "package veh.adas\nservice veh.adas.cruise : Foo\n",
        );
        let redundant = ridl_package(
            &db,
            "veh.redundant",
            "package veh.redundant\nservice veh.adas.cruise : Bar\n",
        );
        let ws = Workspace::new(&db, vec![adas, redundant], BTreeMap::new());

        let catalog = service_catalog(&db, ws, std);

        assert_eq!(catalog.diagnostics.len(), 1);
        let diagnostic = &catalog.diagnostics[0];
        assert_eq!(diagnostic.code, DiagCode::RIDL_140);
        // Both declarations are labeled: the duplicate as the primary span, the
        // first via one secondary label.
        assert_eq!(diagnostic.labels.len(), 1);
        // The name survives in the catalog once (the first declaration).
        assert!(catalog.entries.contains_key("veh.adas.cruise"));
        assert_eq!(catalog.entries["veh.adas.cruise"].package, "veh.adas");
    }

    #[test]
    fn service_catalog_resolves_a_cross_package_interface_ref_through_imports() {
        use crate::std_lib::std_package;
        let mut db = RidlDatabase::default();
        let std = std_package(&mut db);
        let common = ridl_package(
            &db,
            "veh.common",
            "package veh.common\ninterface CruiseControl {\n  signal engaged : boolean\n}\n",
        );
        let adas = ridl_package(
            &db,
            "veh.adas",
            "package veh.adas\nimport veh.common.CruiseControl\nservice veh.adas.cruise : CruiseControl\n",
        );
        let ws = Workspace::new(&db, vec![common, adas], BTreeMap::new());

        let catalog = service_catalog(&db, ws, std);

        assert!(catalog.diagnostics.is_empty());
        let entry = &catalog.entries["veh.adas.cruise"];
        assert_eq!(entry.package, "veh.adas");
        // The bare `CruiseControl` reference canonicalizes to its full
        // `pkg.Name` path through the declaring file's import.
        assert_eq!(entry.interface_ref, "veh.common.CruiseControl");
    }

    #[test]
    fn service_catalog_records_an_inline_shape_with_an_empty_ref() {
        use crate::std_lib::std_package;
        let mut db = RidlDatabase::default();
        let std = std_package(&mut db);
        let hvac = ridl_package(
            &db,
            "veh.hvac",
            "package veh.hvac\nservice veh.hvac.cabin {\n  signal temperature : boolean\n}\n",
        );
        let ws = Workspace::new(&db, vec![hvac], BTreeMap::new());

        let catalog = service_catalog(&db, ws, std);

        assert!(catalog.diagnostics.is_empty());
        let entry = &catalog.entries["veh.hvac.cabin"];
        assert_eq!(entry.package, "veh.hvac");
        assert_eq!(entry.interface_ref, "");
    }
}
