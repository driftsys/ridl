//! Workspace design checks over current checked inputs, before lint levels apply.

use ridl_core::RidlDatabase;
use ridl_core::diag::{Diagnostic, SourceMap};
use ridl_core::package::{Package, PackageOrigin};
use ridl_sem::{CheckedPackage, Resolution};

mod ceiling;
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
    // A package of a remote unit is a read-only dependency (ADR-0002 §5):
    // the checks read it as a target, for the unit of a type it declares,
    // and never examine its own declarations.
    let subject = packages
        .iter()
        .map(|package| !matches!(package.origin(db), PackageOrigin::Remote { .. }))
        .collect();
    run(&Ctx {
        checked,
        subject,
        sites,
    })
}

/// RIDL-150 over one source set, before lint levels apply. `encodings` are the
/// names that every `[backend.<name>]` table of the workspace manifest enables,
/// as one set; FlatBuffers is checked when it is empty. `std_ir` is the lowered
/// `ridl.std`, which a package's references resolve against besides `checked`.
/// The check lowers each package to the codegen model, so `blocked` holds, per
/// entry of `packages`, whether the package or one it depends on has an error;
/// a blocked package is read as a target and never examined.
#[allow(clippy::too_many_arguments)]
pub(crate) fn check_payload_ceiling(
    db: &RidlDatabase,
    packages: &[Package],
    checked: &[CheckedPackage],
    resolutions: &[Resolution],
    blocked: &[bool],
    std_ir: &ridl_ir::v2::Package,
    encodings: &[String],
    sources: &mut SourceMap,
) -> Vec<Diagnostic> {
    let sites = SiteIndex::new(db, packages, resolutions, sources);
    let subject = packages
        .iter()
        .zip(blocked)
        .map(|(package, blocked)| {
            !blocked && !matches!(package.origin(db), PackageOrigin::Remote { .. })
        })
        .collect();
    ceiling::check(
        &Ctx {
            checked,
            subject,
            sites,
        },
        std_ir,
        encodings,
    )
}

// The design checks consume the checked packages and the indexed sites.
pub(crate) struct Ctx<'a> {
    pub checked: &'a [CheckedPackage],
    /// Per entry of `checked`, whether the checks examine its declarations:
    /// `false` for a package of a remote unit.
    subject: Vec<bool>,
    pub sites: SiteIndex,
}

impl Ctx<'_> {
    /// The packages whose declarations the checks examine: the consumer's,
    /// without `ridl.std` and without the packages of the remote units,
    /// which stay in `checked` as targets.
    pub(crate) fn subjects(&self) -> impl Iterator<Item = &CheckedPackage> {
        self.checked
            .iter()
            .zip(&self.subject)
            .filter(|(checked, subject)| **subject && checked.ir.name != "ridl.std")
            .map(|(checked, _)| checked)
    }
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
