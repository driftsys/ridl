//! Remote layout growth attributed to published consumer uses.

use std::collections::{BTreeMap, BTreeSet};

use ridl_ir::{
    projection::{flatbuffers::Packages, podbuf},
    v2,
};

use crate::{Category, Change, ContextIssue, ContextSide, Verdict};

pub(crate) fn compare(
    holders: &[&v2::Package],
    old: &[&v2::Package],
    new: &[&v2::Package],
    encodings: &[&str],
) -> (Vec<Change>, Vec<ContextIssue>) {
    let mut pass = Pass {
        old,
        new,
        locals: holders.iter().map(|p| p.name.as_str()).collect(),
        changes: BTreeMap::new(),
        issues: BTreeSet::new(),
        verdict: if encodings.contains(&"podbuf") {
            Verdict::Breaking
        } else {
            Verdict::Compatible
        },
    };
    for side in [ContextSide::Old, ContextSide::New] {
        for home in holders {
            for decl in &home.decls {
                if let Some(v2::decl::Kind::StructDef(def)) = &decl.kind {
                    for slot in &def.members {
                        if let Some(v2::struct_member::Member::Field(field)) = &slot.member
                            && let Some(ty) = &field.r#type
                        {
                            pass.visit(
                                home,
                                ty,
                                &format!("{}/{}/{}", home.name, decl.name, field.name),
                                true,
                                side,
                                &mut BTreeSet::new(),
                            );
                        }
                    }
                }
                if let Some(v2::decl::Kind::UnionDef(def)) = &decl.kind {
                    for arm in &def.arms {
                        let ty = v2::FieldType {
                            optional: false,
                            kind: Some(v2::field_type::Kind::Named(arm.type_ref.clone())),
                        };
                        pass.visit(
                            home,
                            &ty,
                            &format!("{}/{}/{}", home.name, decl.name, arm.name),
                            false,
                            side,
                            &mut BTreeSet::new(),
                        );
                    }
                }
            }
            for shape in home.shapes() {
                for interaction in &shape.interface.interactions {
                    let mut roots = crate::walk::interaction_payloads(interaction);
                    let payload = match &interaction.kind {
                        Some(v2::decl::Kind::SignalDef(def)) => Some(&def.payload),
                        Some(v2::decl::Kind::EventDef(def)) => Some(&def.payload),
                        _ => None,
                    };
                    if let Some(reference) = payload {
                        roots.push(v2::FieldType {
                            optional: false,
                            kind: Some(v2::field_type::Kind::Named(reference.clone())),
                        });
                    }
                    for ty in roots {
                        // Exact arrays at the root have no head. Inside a
                        // head or descriptor element, they fix element offsets.
                        let mut root = &ty;
                        while let Some(v2::field_type::Kind::Array(array)) = &root.kind
                            && array.min == array.max
                            && let Some(element) = &array.element
                        {
                            root = element;
                        }
                        pass.visit(
                            home,
                            root,
                            &format!("{}/{}/{}", home.name, shape.name, interaction.name),
                            false,
                            side,
                            &mut BTreeSet::new(),
                        );
                    }
                }
            }
        }
    }
    (
        pass.changes.into_values().collect(),
        pass.issues.into_iter().collect(),
    )
}

struct Pass<'a> {
    old: &'a [&'a v2::Package],
    new: &'a [&'a v2::Package],
    locals: BTreeSet<&'a str>,
    changes: BTreeMap<(String, String), Change>,
    issues: BTreeSet<ContextIssue>,
    verdict: Verdict,
}

impl Pass<'_> {
    fn visit(
        &mut self,
        home: &v2::Package,
        ty: &v2::FieldType,
        path: &str,
        fixed: bool,
        side: ContextSide,
        active: &mut BTreeSet<String>,
    ) {
        use v2::field_type::Kind;
        match &ty.kind {
            Some(Kind::Named(reference)) => {
                let scope = match side {
                    ContextSide::Old => self.old,
                    ContextSide::New => self.new,
                };
                let Some(root) = scope.first() else {
                    return;
                };
                let packages = Packages {
                    package: root,
                    others: &scope[1..],
                };
                // Bare names belong to the declaration's source-side home.
                let source_home = scope
                    .iter()
                    .copied()
                    .find(|p| p.name == home.name)
                    .unwrap_or(home);
                let identity = if reference.contains('.') {
                    reference.clone()
                } else {
                    format!("{}.{}", home.name, reference)
                };
                let Some((decl, declaring)) = packages.resolve(source_home, reference) else {
                    let owner = reference
                        .rsplit_once('.')
                        .map_or(home.name.as_str(), |(p, _)| p);
                    if !self.locals.contains(owner) {
                        self.issues.insert(ContextIssue {
                            path: path.to_string(),
                            declaration: identity,
                            side,
                        });
                    }
                    return;
                };
                // Local declarations are visited at their own nearest field paths.
                if self.locals.contains(declaring.name.as_str()) {
                    return;
                }
                if !active.insert(identity.clone()) {
                    return;
                }
                match &decl.kind {
                    Some(v2::decl::Kind::StructDef(def)) => {
                        if fixed && side == ContextSide::Old {
                            let old_size = head_size(self.old, declaring, &decl.name, def);
                            let new_decl = self
                                .new
                                .iter()
                                .copied()
                                .find(|p| p.name == declaring.name)
                                .and_then(|p| {
                                    p.decls.iter().find(|d| d.name == decl.name).map(|d| (p, d))
                                });
                            if new_decl.is_none() {
                                self.issues.insert(ContextIssue {
                                    path: path.to_string(),
                                    declaration: identity.clone(),
                                    side: ContextSide::New,
                                });
                            }
                            let new_size = new_decl.and_then(|(p, d)| match &d.kind {
                                Some(v2::decl::Kind::StructDef(def)) => {
                                    head_size(self.new, p, &d.name, def)
                                }
                                _ => None,
                            });
                            if let (Some(before), Some(after)) = (old_size, new_size)
                                && after > before
                            {
                                self.changes.insert(
                                    (path.to_string(), identity.clone()),
                                    Change {
                                        path: path.to_string(),
                                        category: Category::PodbufRemoteInlineGrowth,
                                        verdict: self.verdict,
                                        before: Some(format!("{identity}: {before}")),
                                        after: Some(format!("{identity}: {after}")),
                                    },
                                );
                            }
                        }
                        for member in &def.members {
                            if let Some(v2::struct_member::Member::Field(field)) = &member.member
                                && let Some(ty) = &field.r#type
                            {
                                self.visit(declaring, ty, path, true, side, active);
                            }
                        }
                    }
                    Some(v2::decl::Kind::UnionDef(def)) => {
                        for arm in &def.arms {
                            let ty = v2::FieldType {
                                optional: false,
                                kind: Some(Kind::Named(arm.type_ref.clone())),
                            };
                            self.visit(declaring, &ty, path, false, side, active);
                        }
                    }
                    _ => {}
                }
                active.remove(&identity);
            }
            Some(Kind::Tuple(tuple)) => {
                for field in &tuple.fields {
                    if let Some(ty) = &field.r#type {
                        self.visit(home, ty, path, true, side, active);
                    }
                }
            }
            Some(Kind::Array(array)) => {
                if let Some(ty) = &array.element {
                    self.visit(home, ty, path, array.min == array.max, side, active);
                }
            }
            Some(Kind::Map(map)) => {
                for ty in [&map.key, &map.value].into_iter().flatten() {
                    self.visit(home, ty, path, false, side, active);
                }
            }
            _ => {}
        }
    }
}

fn head_size(
    scope: &[&v2::Package],
    home: &v2::Package,
    name: &str,
    def: &v2::StructDef,
) -> Option<u32> {
    let others: Vec<_> = scope
        .iter()
        .copied()
        .filter(|p| p.name != home.name)
        .collect();
    podbuf::struct_head(
        Packages {
            package: home,
            others: &others,
        },
        name,
        def,
    )
    .ok()
    .map(|head| head.size)
}
