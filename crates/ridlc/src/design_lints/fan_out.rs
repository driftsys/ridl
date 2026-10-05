//! Report packages with too many distinct workspace dependencies.

use ridl_core::diag::{DiagCode, Diagnostic};
use ridl_core::lint::lint_by_name;

use super::Ctx;

pub(crate) const PACKAGE_FAN_OUT_MAX: usize = 3;

pub(crate) fn check(ctx: &Ctx<'_>) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    for (package, dependencies) in &ctx.package_edges {
        if package == "ridl.std" || dependencies.len() <= PACKAGE_FAN_OUT_MAX {
            continue;
        }
        let Some(primary) = ctx.sites.package_line(package) else {
            continue;
        };
        diagnostics.push(Diagnostic {
            code: DiagCode::RIDL_415,
            severity: lint_by_name("package-fan-out")
                .expect("registered lint")
                .severity,
            message: format!(
                "package `{package}` depends on {} workspace packages: {}",
                dependencies.len(),
                dependencies.iter().cloned().collect::<Vec<_>>().join(", ")
            ),
            primary,
            labels: Vec::new(),
            fixits: Vec::new(),
        });
    }
    diagnostics
}
