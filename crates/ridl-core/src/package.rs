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

use std::collections::BTreeMap;

use ridl_syntax::ast::{AstNode as _, SourceFile};
use rowan::TextRange;

use crate::db::InputFile;

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
}
