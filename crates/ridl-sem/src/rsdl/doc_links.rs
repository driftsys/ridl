//! Doc links on the rsdl carriers (typl §14, ADR-0026): every link candidate
//! and `@see` target of a declaration or a body line is resolved in the view
//! of the package that declares it, with the resolver the package checker
//! uses (`crate::resolve::resolve_doc_link`). A resolved one is stored on the
//! model entry for the lowering; one that does not resolve is TYPL-401 at the
//! candidate's span. The lowering has no database, so this pass runs in
//! `check_system`, right after collection.
//!
//! A placement line of a `machine` is resolved for its diagnostics only: the
//! IR `Placement` message carries no doc.

use std::collections::HashMap;

use ridl_core::diag::DiagCode;
use ridl_core::package::{Package, Workspace, package_of};
use rowan::TextRange;

use super::{CheckedSystem, MemberRef, Reporter, Site};
use crate::docs::DocInfo;
use crate::resolve::{DocLinks, Resolution, resolve_doc_info, resolve_package};

/// Resolves the doc links of every declaration and line of `system`, stores
/// the resolved ones, and reports each broken one.
pub(super) fn resolve(
    db: &dyn salsa::Database,
    ws: Workspace,
    std: Package,
    system: &mut CheckedSystem,
    reporter: &mut Reporter,
) {
    let mut views = Views {
        db,
        ws,
        std,
        by_package: HashMap::new(),
    };
    for decl in &mut system.systems {
        let (file, package) = (decl.name.site.file, decl.package.clone());
        store(
            &mut views,
            &package,
            file,
            &decl.doc,
            &mut decl.links,
            &mut decl.see,
            reporter,
        );
        lines(&mut views, &package, &mut decl.members, reporter);
    }
    for decl in &mut system.components {
        let (file, package) = (decl.name.site.file, decl.package.clone());
        store(
            &mut views,
            &package,
            file,
            &decl.doc,
            &mut decl.links,
            &mut decl.see,
            reporter,
        );
        lines(&mut views, &package, &mut decl.offers, reporter);
        lines(&mut views, &package, &mut decl.requires, reporter);
    }
    for decl in &mut system.distributions {
        let (file, package) = (decl.name.site.file, decl.package.clone());
        store(
            &mut views,
            &package,
            file,
            &decl.doc,
            &mut decl.links,
            &mut decl.see,
            reporter,
        );
        lines(&mut views, &package, &mut decl.members, reporter);
    }
    for decl in &mut system.deployments {
        let (file, package) = (decl.name.site.file, decl.package.clone());
        store(
            &mut views,
            &package,
            file,
            &decl.doc,
            &mut decl.links,
            &mut decl.see,
            reporter,
        );
        for machine in &mut decl.machines {
            let file = machine.name.site.file;
            store(
                &mut views,
                &package,
                file,
                &machine.doc,
                &mut machine.links,
                &mut machine.see,
                reporter,
            );
            lines(&mut views, &package, &mut machine.members, reporter);
        }
    }
}

/// The resolved views of the packages the pass has met, built once each.
struct Views<'a> {
    db: &'a dyn salsa::Database,
    ws: Workspace,
    std: Package,
    by_package: HashMap<String, Option<(Package, Resolution)>>,
}

impl Views<'_> {
    /// Resolves `info` in the view of the package `name`, or returns no links
    /// and no reports when the workspace has no such package — the collector
    /// reads the name from the file's own `package` line, so that is a loader
    /// error already reported.
    fn resolve(&mut self, name: &str, info: &DocInfo) -> Option<DocLinks> {
        let (db, ws, std) = (self.db, self.ws, self.std);
        let view = self
            .by_package
            .entry(name.to_string())
            .or_insert_with(|| {
                let pkg = package_of(db, ws, name.to_string())?;
                Some((pkg, resolve_package(db, ws, pkg, std)))
            })
            .as_ref()?;
        Some(resolve_doc_info(db, ws, std, view.0, &view.1, info))
    }
}

/// Resolves one carrier's doc in `package`'s view, stores the resolved links
/// in `links` and `see`, and reports each broken candidate at its span in
/// `file`.
fn store(
    views: &mut Views<'_>,
    package: &str,
    file: ridl_core::db::InputFile,
    info: &DocInfo,
    links: &mut Vec<ridl_ir::v2::DocLink>,
    see: &mut Vec<ridl_ir::v2::DocLink>,
    reporter: &mut Reporter,
) {
    let Some(resolved) = views.resolve(package, info) else {
        return;
    };
    *links = resolved.links;
    *see = resolved.see;
    for (range, message) in resolved.broken {
        report(reporter, file, range, message);
    }
}

/// [`store`] over every line of a body.
fn lines(views: &mut Views<'_>, package: &str, lines: &mut [MemberRef], reporter: &mut Reporter) {
    for line in lines {
        let file = line.reference.site.file;
        store(
            views,
            package,
            file,
            &line.doc,
            &mut line.links,
            &mut line.see,
            reporter,
        );
    }
}

fn report(
    reporter: &mut Reporter,
    file: ridl_core::db::InputFile,
    range: TextRange,
    message: String,
) {
    reporter.warning(DiagCode::TYPL_401, Site { file, range }, message);
}
