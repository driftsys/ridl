//! The workspace-wide passes: the ones that run once over the whole workspace,
//! after the per-package `resolve_package` and `check_package`. Every driver
//! that reports diagnostics — `ridlc`, which `ridl check` and the `ridl_check`
//! MCP tool share, and the language server — runs them through
//! [`check_workspace`], so a later workspace-wide pass reaches each face at
//! once (issues #345, #386). The language server's hover and navigation read
//! `check_system` and `service_catalog` directly, for the model rather than
//! the diagnostics.

use ridl_core::diag::Diagnostic;
use ridl_core::package::{Package, Workspace, service_catalog};

use crate::rsdl::{CheckedSystem, check_system};

/// What the workspace-wide passes return.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedWorkspace {
    /// Every diagnostic of the workspace-wide passes: the service catalog's
    /// (RIDL-140) first, then the rsdl system query's. Each [`FileId`] indexes
    /// the workspace's source files in package-then-file order. A package's
    /// `interfaces.lock` is not one of those files, so a driver remaps these
    /// diagnostics onto the render ids of the source files alone, in that
    /// order.
    ///
    /// [`FileId`]: ridl_core::diag::FileId
    pub diagnostics: Vec<Diagnostic>,
    /// The checked rsdl model. Its diagnostics are in `diagnostics` above, and
    /// its own list is empty.
    pub system: CheckedSystem,
}

/// Runs the workspace-wide passes over `ws`: the service catalog (E2.13),
/// whose RIDL-140 duplicate-name diagnostics span the whole workspace, and the
/// rsdl system query (rsdl reference v0.2), which checks every `.rsdl` file at
/// once because the closure is workspace-wide.
///
/// RSDL-804 is not raised here: only a driver knows which backends are
/// configured, so each driver calls
/// [`unclaimed_backend_keys`](crate::rsdl::unclaimed_backend_keys) with the
/// namespaces it claims.
pub fn check_workspace(db: &dyn salsa::Database, ws: Workspace, std: Package) -> CheckedWorkspace {
    let mut diagnostics = service_catalog(db, ws, std).diagnostics;
    let mut system = check_system(db, ws, std);
    diagnostics.append(&mut system.diagnostics);
    CheckedWorkspace {
        diagnostics,
        system,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use ridl_core::db::{InputFile, RidlDatabase};
    use ridl_core::diag::{FileId, SourceMap};
    use ridl_core::package::PackageOrigin;
    use ridl_core::std_package;

    use super::*;

    /// One package whose first file draws RSDL-602 from the rsdl system query
    /// and whose second draws RIDL-140 from the service catalog: file order
    /// and pass order disagree, so the result shows which one the list follows.
    #[test]
    fn the_catalog_diagnostics_come_first_and_the_system_keeps_none() {
        let mut db = RidlDatabase::default();
        let std = std_package(&mut db);
        let texts = [
            ("p/a.rsdl", "package p\nsystem S { Missing }\n"),
            (
                "p/b.ridl",
                "package p\ninterface I {}\nservice p.s : I\nservice p.s : I\n",
            ),
        ];
        // The ids the diagnostics carry: the files interned in package-then-file
        // order, as a driver rebuilds it.
        let mut sources = SourceMap::new();
        let ids: Vec<FileId> = texts
            .iter()
            .map(|(path, text)| sources.file_id(path, text))
            .collect();
        let files = texts
            .iter()
            .map(|(path, text)| InputFile::new(&db, path.to_string(), text.to_string()))
            .collect();
        let package = Package::new(
            &db,
            "p".to_string(),
            files,
            PackageOrigin::WorkspaceMember,
            BTreeMap::new(),
            None,
            None,
        );
        let ws = Workspace::new(&db, vec![package], BTreeMap::new());

        let checked = check_workspace(&db, ws, std);
        let found: Vec<(&str, FileId)> = checked
            .diagnostics
            .iter()
            .map(|diagnostic| (diagnostic.code.as_str(), diagnostic.primary.file))
            .collect();
        assert_eq!(
            found,
            [("RIDL-140", ids[1]), ("RSDL-602", ids[0])],
            "{:?}",
            checked.diagnostics
        );
        assert_eq!(checked.system.diagnostics, Vec::new());
    }
}
