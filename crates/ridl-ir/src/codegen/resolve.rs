//! Reference resolution over the lowering's scope (design note D-3).
//!
//! The one thing every backend does again and the one thing a codegen plugin
//! cannot do for itself, because it is handed one package and a reference can
//! name another. The rule is the IR's canonical form: a cross-package
//! reference is the fully qualified `pkg.Name`, a same-package one the bare
//! `Name`, never an import alias (`proto/ridl/ir/v2/ir.proto`).

use crate::v2;

/// The packages one lowering was handed: the package being lowered and the
/// others given beside it, in the order given.
#[derive(Clone, Copy)]
pub(crate) struct Scope<'a> {
    pub package: &'a v2::Package,
    pub others: &'a [&'a v2::Package],
}

impl<'a> Scope<'a> {
    /// The declaration `reference` names, and the package that declares it —
    /// which is the package a bare reference *inside* that declaration then
    /// resolves against. `home` is the package the reference was written in.
    ///
    /// The same walk `projection::flatbuffers`'s `Sizer::resolve` makes, and
    /// the same one the two wire backends make in `resolve_reference`.
    pub fn resolve(
        &self,
        home: &'a v2::Package,
        reference: &str,
    ) -> Option<(&'a v2::Decl, &'a v2::Package)> {
        match reference.rsplit_once('.') {
            Some((package, member)) => self
                .packages()
                .find(|candidate| candidate.name == package)
                .and_then(|candidate| {
                    candidate
                        .decls
                        .iter()
                        .find(|decl| decl.name == member)
                        .map(|decl| (decl, candidate))
                }),
            None => home
                .decls
                .iter()
                .find(|decl| decl.name == reference)
                .map(|decl| (decl, home)),
        }
    }

    /// Every package in the scope: the one being lowered, then the others.
    pub fn packages(&self) -> impl Iterator<Item = &'a v2::Package> {
        std::iter::once(self.package).chain(self.others.iter().copied())
    }

    /// The `Packages` bundle `projection::flatbuffers` takes, with `home` as
    /// the package a bare reference resolves against and every other package
    /// of the scope beside it.
    pub fn projection_others(&self, home: &v2::Package) -> Vec<&'a v2::Package> {
        self.packages()
            .filter(|candidate| candidate.name != home.name)
            .collect()
    }
}

/// A reference is cross-package exactly when it carries a dot — the IR's
/// canonical form, not a property of what the scope happens to hold.
pub(crate) fn is_foreign(reference: &str) -> bool {
    reference.contains('.')
}
