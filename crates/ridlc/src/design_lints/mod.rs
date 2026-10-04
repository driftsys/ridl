//! Workspace design checks over current checked inputs, before lint levels apply.

use std::collections::{BTreeMap, BTreeSet};

use ridl_core::RidlDatabase;
use ridl_core::diag::{Diagnostic, SourceMap};
use ridl_core::package::Package;
use ridl_ir::v2;
use ridl_sem::{CheckedPackage, Resolution};

mod cohesion;
mod shapes;
mod sites;
mod units;
mod words;

use sites::SiteIndex;

pub use cohesion::cohesion_groups;

/// Checks one source set without loading files or applying lint levels.
///
/// `packages`, `checked` and `resolutions` describe the same packages in the
/// same order. Pass current database inputs, including unsaved text, and the
/// source map used to render all other diagnostics. A standalone editor buffer
/// is its own source set, separate from the loaded workspace.
pub fn check_design_lints(
    db: &RidlDatabase,
    packages: &[Package],
    checked: &[CheckedPackage],
    resolutions: &[Resolution],
    std_ir: &v2::Package,
    system: Option<&v2::System>,
    sources: &mut SourceMap,
) -> Vec<Diagnostic> {
    let sites = SiteIndex::new(db, packages, resolutions, sources);
    let complete_edges = crate::deps::package_edges(checked, system);
    let package_edges = crate::deps::workspace_package_edges(&complete_edges);
    run(&Ctx {
        db,
        checked,
        resolutions,
        std_ir,
        system,
        sites,
        package_edges,
    })
}

// The remaining design checks consume the shared context and indexed sites.
#[allow(dead_code)]
pub(crate) struct Ctx<'a> {
    pub db: &'a RidlDatabase,
    pub checked: &'a [CheckedPackage],
    pub resolutions: &'a [Resolution],
    pub std_ir: &'a v2::Package,
    pub system: Option<&'a v2::System>,
    pub sites: SiteIndex,
    pub package_edges: BTreeMap<String, BTreeSet<String>>,
}

pub(crate) fn run(ctx: &Ctx<'_>) -> Vec<Diagnostic> {
    let mut diagnostics = units::check(ctx);
    diagnostics.extend(words::check(ctx));
    diagnostics.extend(shapes::check(ctx));
    diagnostics.extend(cohesion::check(ctx));
    diagnostics
}

fn qualify(pkg: &str, name: &mut String) {
    if !name.contains('.') {
        *name = format!("{pkg}.{name}");
    }
}
