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
    variants: BTreeMap<(String, String, String), Span>,
    other_children: BTreeMap<(String, String, String), Span>,
    tuple_fields: Vec<IdSite>,
    declarations: BTreeMap<(String, String), Span>,
    packages: BTreeMap<String, Span>,
}

/// One identifier token with its qualified identity and source location.
#[derive(Clone)]
pub(crate) struct IdSite {
    pub package: String,
    pub full_name: String,
    pub name: String,
    pub span: Span,
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
            files.sort_by(|a, b| a.path(db).cmp(b.path(db)));
            for (position, input) in files.iter().enumerate() {
                let file = sources.file_id(input.path(db), input.text(db));
                let Some(ast) = ast::SourceFile::cast(parse_file(db, *input).syntax()) else {
                    continue;
                };
                if position == 0
                    && let Some(decl) = ast.package_decl()
                {
                    index.packages.insert(
                        pkg.clone(),
                        Span {
                            file,
                            range: decl.syntax().text_range(),
                        },
                    );
                }
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
                    // Keep enum variants separate for the existing variant lookup.
                    // The shared identifier inventory also includes bits and arms.
                    let children: Vec<_> = match &def {
                        ast::Definition::EnumSet(def) => {
                            def.bits().filter_map(|bit| bit.name()).collect()
                        }
                        ast::Definition::Union(def) => {
                            def.arms().filter_map(|arm| arm.name()).collect()
                        }
                        _ => Vec::new(),
                    };
                    for name in children {
                        index
                            .other_children
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
                    index.index_tuple_fields(pkg, &name_text, def.syntax(), file);
                    match def {
                        ast::Definition::Struct(def) => {
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
                        ast::Definition::Enum(def) => {
                            for variant in def.values() {
                                if let Some(name) = variant.name() {
                                    index
                                        .variants
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
                        _ => {}
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
                        index.index_tuple_fields(
                            pkg,
                            &format!("{name}.{member_text}"),
                            member.syntax(),
                            file,
                        );
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

    /// Walk nested type nodes, including tuples inside containers and returns.
    /// Keep each occurrence: equal field names in different tuples are sites.
    fn index_tuple_fields(
        &mut self,
        pkg: &str,
        owner: &str,
        root: &ridl_syntax::SyntaxNode,
        file: ridl_core::diag::FileId,
    ) {
        for field in root.descendants().filter_map(ast::TupleField::cast) {
            let Some(name) = field.name() else { continue };
            let mut parents = field
                .syntax()
                .ancestors()
                .skip(1)
                .take_while(|node| node != root)
                .filter_map(|node| {
                    if let Some(field) = ast::TupleField::cast(node.clone()) {
                        field.name()
                    } else if let Some(field) = ast::FieldDef::cast(node.clone()) {
                        field.name()
                    } else if let Some(arm) = ast::UnionArm::cast(node) {
                        arm.name()
                    } else {
                        None
                    }
                })
                .map(|name| name.syntax().text().to_string())
                .collect::<Vec<_>>();
            parents.reverse();
            parents.insert(0, owner.into());
            let name_text = name.syntax().text().to_string();
            self.tuple_fields.push(IdSite {
                package: pkg.into(),
                full_name: format!("{pkg}.{}.{}", parents.join("."), name_text),
                name: name_text,
                span: Span {
                    file,
                    range: name.syntax().text_range(),
                },
            });
        }
    }

    /// Enumerates identifier tokens in qualified-name order. Qualifiers identify
    /// a site, but are not part of the identifier's words.
    pub fn identifiers(&self) -> Vec<IdSite> {
        let mut sites = Vec::new();
        let mut add = |pkg: &str, owner: &str, name: &str, span: Span| {
            let full_name = if owner.is_empty() {
                format!("{pkg}.{name}")
            } else {
                format!("{pkg}.{owner}.{name}")
            };
            sites.push(IdSite {
                package: pkg.into(),
                full_name,
                name: name.into(),
                span,
            });
        };
        for ((pkg, name), span) in &self.declarations {
            add(pkg, "", name, *span);
        }
        for ((pkg, owner, name), span) in self
            .fields
            .iter()
            .chain(&self.variants)
            .chain(&self.members)
            .chain(&self.other_children)
        {
            add(pkg, owner, name, *span);
        }
        for ((pkg, iface, member, name), span) in &self.params {
            add(pkg, &format!("{iface}.{member}"), name, *span);
        }
        sites.extend(self.tuple_fields.iter().cloned());
        sites.sort_by(|a, b| (&a.package, &a.full_name).cmp(&(&b.package, &b.full_name)));
        sites
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

    // Used by the subsequent abbreviation and shape checks.
    #[allow(dead_code)]
    pub fn variant(&self, pkg: &str, name: &str, variant: &str) -> Option<Span> {
        self.variants
            .get(&(pkg.into(), name.into(), variant.into()))
            .copied()
    }

    pub fn decl(&self, pkg: &str, name: &str) -> Option<Span> {
        self.declarations.get(&(pkg.into(), name.into())).copied()
    }

    // Used by the subsequent package fan-out check.
    #[allow(dead_code)]
    pub fn package_line(&self, pkg: &str) -> Option<Span> {
        self.packages.get(pkg).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ridl_core::db::InputFile;
    use ridl_core::package::{PackageOrigin, Workspace};

    #[test]
    fn index_uses_resolution_winners_and_first_path_package_line() {
        let mut db = RidlDatabase::default();
        let std = ridl_core::std_package(&mut db);
        let later =
            "package a\nstruct Entry { value: boolean }\nenum Mode { First = 0, Second = 1 }\n";
        let first = "// First path\npackage a\nstruct Entry { losing: boolean }\n";
        let z = InputFile::new(&db, "z.ridl".to_string(), later.to_string());
        let a = InputFile::new(&db, "a.ridl".to_string(), first.to_string());
        // Resolution follows input order; the package location follows paths.
        let package = Package::new(
            &db,
            "a".to_string(),
            vec![z, a],
            PackageOrigin::WorkspaceMember,
            BTreeMap::new(),
            None,
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
        let package_line = index.package_line("a").unwrap();
        assert_eq!(sources.path(package_line.file), Some("a.ridl"));
        assert_eq!(text(package_line), "package a");
        let declaration = index.decl("a", "Entry").unwrap();
        assert_eq!(sources.path(declaration.file), Some("z.ridl"));
        assert_eq!(text(declaration), "Entry");
        assert_eq!(text(index.field("a", "Entry", "value").unwrap()), "value");
        assert!(index.field("a", "Entry", "losing").is_none());
        let variant = index.variant("a", "Mode", "Second").unwrap();
        assert_eq!(sources.path(variant.file), Some("z.ridl"));
        assert_eq!(text(variant), "Second");
        assert!(index.variant("a", "Other", "Second").is_none());
        assert!(index.decl("a", "Duration").is_none());
    }
}
