//! Source locations indexed once from current ASTs and resolved declarations.

use std::collections::BTreeMap;

use ridl_core::RidlDatabase;
use ridl_core::diag::{SourceMap, Span};
use ridl_core::package::Package;
use ridl_core::parse_file;
use ridl_sem::Resolution;
use ridl_syntax::ast::{self, AstNode as _, HasName as _};

#[derive(Default)]
pub(crate) struct SiteIndex {
    fields: BTreeMap<(String, String, String), Span>,
    members: BTreeMap<(String, String, String), Span>,
    params: BTreeMap<(String, String, String, String), Span>,
    declarations: BTreeMap<(String, String), Span>,
}

impl SiteIndex {
    pub fn new(
        db: &RidlDatabase,
        packages: &[Package],
        resolutions: &[Resolution],
        sources: &mut SourceMap,
    ) -> Self {
        let mut index = Self::default();
        for (package, resolution) in packages.iter().zip(resolutions) {
            let pkg = package.name(db);
            if pkg == "ridl.std" {
                continue;
            }
            for symbol in resolution.symbols.values().filter(|s| s.package == *pkg) {
                // Imported symbols have another package; the resolver retains
                // the first declaration when a local name is duplicated.
                let file = sources.file_id(symbol.file.path(db), symbol.file.text(db));
                index.declarations.insert(
                    (pkg.clone(), symbol.name.clone()),
                    Span {
                        file,
                        range: symbol.range,
                    },
                );
            }
            let mut files = package.files(db).clone();
            // Fields and members keep the first site seen, so a fixed path order
            // makes the kept site deterministic.
            files.sort_by(|a, b| a.path(db).cmp(b.path(db)));
            for input in &files {
                let file = sources.file_id(input.path(db), input.text(db));
                let Some(ast) = ast::SourceFile::cast(parse_file(db, *input).syntax()) else {
                    continue;
                };
                for def in ast.definitions() {
                    let Some(name) = def.name() else { continue };
                    let name_text = name.syntax().text().to_string();
                    if index.decl(pkg, &name_text)
                        != Some(Span {
                            file,
                            range: name.syntax().text_range(),
                        })
                    {
                        continue;
                    }
                    if let ast::Definition::Struct(def) = def {
                        for field in def.members().filter_map(|member| match member {
                            ast::StructMember::Field(field) => Some(field),
                            _ => None,
                        }) {
                            if let Some(name) = field.name() {
                                index
                                    .fields
                                    .entry((
                                        pkg.clone(),
                                        name_text.clone(),
                                        name.syntax().text().to_string(),
                                    ))
                                    .or_insert(Span {
                                        file,
                                        range: name.syntax().text_range(),
                                    });
                            }
                        }
                    }
                }
                for shape in ast.shapes() {
                    let Some(name) = shape.identity() else {
                        continue;
                    };
                    if matches!(shape, ast::InterfaceShape::Interface(_))
                        && index.decl(pkg, &name)
                            != shape.identity_range().map(|range| Span { file, range })
                    {
                        continue;
                    }
                    for member in shape.members() {
                        let Some(member_name) = member.name() else {
                            continue;
                        };
                        let member_text = member_name.syntax().text().to_string();
                        index
                            .members
                            .entry((pkg.clone(), name.clone(), member_text.clone()))
                            .or_insert(Span {
                                file,
                                range: member_name.syntax().text_range(),
                            });
                        let params = match member {
                            ast::InterfaceMember::Command(def) => def.params(),
                            ast::InterfaceMember::Query(def) => def.params(),
                            _ => None,
                        };
                        if let Some(params) = params {
                            for param in params.params() {
                                if let Some(param_name) = param.name() {
                                    index
                                        .params
                                        .entry((
                                            pkg.clone(),
                                            name.clone(),
                                            member_text.clone(),
                                            param_name.syntax().text().to_string(),
                                        ))
                                        .or_insert(Span {
                                            file,
                                            range: param_name.syntax().text_range(),
                                        });
                                }
                            }
                        }
                    }
                }
            }
        }
        index
    }

    pub fn field(&self, pkg: &str, name: &str, field: &str) -> Option<Span> {
        self.fields
            .get(&(pkg.into(), name.into(), field.into()))
            .copied()
    }

    pub fn member(&self, pkg: &str, iface: &str, member: &str) -> Option<Span> {
        self.members
            .get(&(pkg.into(), iface.into(), member.into()))
            .copied()
    }

    pub fn param(&self, pkg: &str, iface: &str, member: &str, param: &str) -> Option<Span> {
        self.params
            .get(&(pkg.into(), iface.into(), member.into(), param.into()))
            .copied()
    }

    pub fn decl(&self, pkg: &str, name: &str) -> Option<Span> {
        self.declarations.get(&(pkg.into(), name.into())).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ridl_core::TimingDefaults;
    use ridl_core::db::InputFile;
    use ridl_core::package::{PackageOrigin, Workspace};

    #[test]
    fn index_uses_resolution_winners() {
        let mut db = RidlDatabase::default();
        let std = ridl_core::std_package(&mut db);
        let later = "package a\nstruct Entry { value: boolean }\n";
        let first = "// First path\npackage a\nstruct Entry { losing: boolean }\n";
        let z = InputFile::new(&db, "z.ridl".to_string(), later.to_string());
        let a = InputFile::new(&db, "a.ridl".to_string(), first.to_string());
        // Resolution follows input order, so the later file wins `Entry`.
        let package = Package::new(
            &db,
            "a".to_string(),
            vec![z, a],
            PackageOrigin::WorkspaceMember,
            BTreeMap::new(),
            TimingDefaults::default(),
            None,
        );
        let workspace = Workspace::new(&db, vec![package], BTreeMap::new());
        let resolution = ridl_sem::resolve_package(&db, workspace, package, std);
        let mut sources = SourceMap::new();
        sources.file_id("unrelated.ridl", "package unrelated\n");
        let index = SiteIndex::new(&db, &[package], &[resolution], &mut sources);
        let text = |span: Span| {
            &sources.text(span.file).unwrap()
                [usize::from(span.range.start())..usize::from(span.range.end())]
        };
        let declaration = index.decl("a", "Entry").unwrap();
        assert_eq!(sources.path(declaration.file), Some("z.ridl"));
        assert_eq!(text(declaration), "Entry");
        assert_eq!(text(index.field("a", "Entry", "value").unwrap()), "value");
        assert!(index.field("a", "Entry", "losing").is_none());
        assert!(index.decl("a", "Duration").is_none());
    }
}
