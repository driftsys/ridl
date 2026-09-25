//! The workspace-wide passes: the ones that run once over the whole workspace,
//! after the per-package `resolve_package` and `check_package`. Every command
//! driver — `ridlc`, which `ridl check` and the `ridl_check` MCP tool share,
//! and the language server — runs them through [`check_workspace`], so a later
//! workspace-wide pass reaches each face at once (issues #345, #386).

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
