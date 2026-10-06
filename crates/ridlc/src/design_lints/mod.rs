//! Workspace design checks over current checked inputs, before lint levels apply.

use ridl_core::RidlDatabase;
use ridl_core::diag::{Diagnostic, SourceMap};
use ridl_core::package::Package;
use ridl_sem::{CheckedPackage, Resolution};

mod cohesion;
mod shapes;
mod sites;
mod units;

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
    sources: &mut SourceMap,
) -> Vec<Diagnostic> {
    let sites = SiteIndex::new(db, packages, resolutions, sources);
    run(&Ctx { checked, sites })
}

// The design checks consume the checked packages and the indexed sites.
pub(crate) struct Ctx<'a> {
    pub checked: &'a [CheckedPackage],
    pub sites: SiteIndex,
}

pub(crate) fn run(ctx: &Ctx<'_>) -> Vec<Diagnostic> {
    let mut diagnostics = units::check(ctx);
    diagnostics.extend(shapes::check(ctx));
    diagnostics.extend(cohesion::check(ctx));
    diagnostics
}

fn qualify(pkg: &str, name: &mut String) {
    if !name.contains('.') {
        *name = format!("{pkg}.{name}");
    }
}
