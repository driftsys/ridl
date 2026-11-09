//! Remote layout growth, integer widths and strict enum appends attributed to
//! published consumer uses (ADR-0031 decision 13).
//!
//! Reports `podbuf_remote_inline_growth`, `flatbuffers_width_change`,
//! `proto3_width_change` and `strict_enum_append` at consumer paths, once per
//! path and remote declaration reached by a surviving old use. Strict appends
//! are breaking for every encoding. Local declarations use the local rules;
//! missing per-side remote context supplies no remote comparison.

use std::collections::{BTreeMap, BTreeSet};

use ridl_ir::{
    projection::{
        flatbuffers::{self, Packages},
        podbuf, proto3,
    },
    v2,
};

use crate::{
    Category, Change, ContextIssue, ContextIssueReason, ContextSide, Verdict, category_word,
};

pub(crate) fn compare(
    old_holders: &[&v2::Package],
    new_holders: &[&v2::Package],
    old: &[&v2::Package],
    new: &[&v2::Package],
    encodings: &[&str],
) -> (Vec<Change>, Vec<ContextIssue>) {
    let mut pass = Pass {
        old,
        new,
        locals: BTreeSet::new(),
        require_new: true,
        changes: BTreeMap::new(),
        width_changes: BTreeMap::new(),
        issues: BTreeSet::new(),
        flatbuffers: encodings.contains(&"flatbuffers"),
        proto3: encodings.contains(&"proto3"),
        widths_only: false,
        verdict: if encodings.contains(&"podbuf") {
            Verdict::Breaking
        } else {
            Verdict::Compatible
        },
    };
    let mut new_uses: BTreeMap<String, Vec<v2::FieldType>> = BTreeMap::new();
    walk_holders(new_holders, |_, ty, path, _| {
        new_uses
            .entry(path.to_string())
            .or_default()
            .push(ty.clone());
    });
    for (side, holders) in [
        (ContextSide::Old, old_holders),
        (ContextSide::New, new_holders),
    ] {
        pass.locals = holders.iter().map(|p| p.name.as_str()).collect();
        walk_holders(holders, |home, ty, path, fixed| {
            pass.require_new = new_uses.get(path).is_some_and(|types| types.contains(ty));
            pass.visit(home, ty, path, fixed, side, &mut BTreeSet::new());
        });
    }
    (
        pass.changes
            .into_values()
            .chain(pass.width_changes.into_values())
            .collect(),
        pass.issues.into_iter().collect(),
    )
}

fn walk_holders(
    holders: &[&v2::Package],
    mut visit: impl FnMut(&v2::Package, &v2::FieldType, &str, bool),
) {
    for home in holders {
        for decl in &home.decls {
            if let Some(v2::decl::Kind::StructDef(def)) = &decl.kind {
                for slot in &def.members {
                    if let Some(v2::struct_member::Member::Field(field)) = &slot.member
                        && let Some(ty) = &field.r#type
                    {
                        visit(
                            home,
                            ty,
                            &format!("{}/{}/{}", home.name, decl.name, field.name),
                            true,
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
                    visit(
                        home,
                        &ty,
                        &format!("{}/{}/{}", home.name, decl.name, arm.name),
                        false,
                    );
                }
            }
        }
        for shape in home.shapes() {
            for interaction in &shape.interface.interactions {
                let mut roots = crate::walk::interaction_payloads(interaction);
                let params = match &interaction.kind {
                    Some(v2::decl::Kind::CommandDef(def)) => &def.params[..],
                    Some(v2::decl::Kind::QueryDef(def)) => &def.params[..],
                    _ => &[],
                };
                // Streams are excluded from request heads. Visit their elements
                // only for per-declaration checks (integer widths and strict enum
                // appends), not for inline growth, at the interaction's consumer path.
                roots.extend(params.iter().filter_map(|param| {
                    param
                        .r#type
                        .as_ref()
                        .filter(|ty| matches!(ty.kind, Some(v2::field_type::Kind::Stream(_))))
                        .cloned()
                }));
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
                    visit(
                        home,
                        root,
                        &format!("{}/{}/{}", home.name, shape.name, interaction.name),
                        false,
                    );
                }
            }
        }
    }
}

struct Pass<'a> {
    old: &'a [&'a v2::Package],
    new: &'a [&'a v2::Package],
    locals: BTreeSet<&'a str>,
    require_new: bool,
    changes: BTreeMap<(String, String), Change>,
    width_changes: BTreeMap<(String, String, &'static str), Change>,
    issues: BTreeSet<ContextIssue>,
    verdict: Verdict,
    flatbuffers: bool,
    proto3: bool,
    widths_only: bool,
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
                            reason: ContextIssueReason::Missing,
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
                if side == ContextSide::Old && self.require_new {
                    self.widths(declaring, decl, path, &identity);
                }
                if fixed && side == ContextSide::Old && !self.widths_only {
                    let old_size = self.head_size(
                        self.old,
                        declaring,
                        decl,
                        path,
                        ContextSide::Old,
                        &identity,
                    );
                    let new_decl = self
                        .new
                        .iter()
                        .copied()
                        .find(|p| p.name == declaring.name)
                        .and_then(|p| p.decls.iter().find(|d| d.name == decl.name).map(|d| (p, d)));
                    let new_size = match new_decl {
                        // A changed consumer use needs only its own new type.
                        _ if !self.require_new => None,
                        Some((_, d))
                            if matches!(decl.kind, Some(v2::decl::Kind::StructDef(_)))
                                && !matches!(d.kind, Some(v2::decl::Kind::StructDef(_))) =>
                        {
                            self.issues.insert(ContextIssue {
                                path: path.to_string(),
                                declaration: identity.clone(),
                                side: ContextSide::New,
                                reason: ContextIssueReason::KindChanged,
                            });
                            None
                        }
                        Some((p, d)) => {
                            self.head_size(self.new, p, d, path, ContextSide::New, &identity)
                        }
                        None => {
                            self.issues.insert(ContextIssue {
                                path: path.to_string(),
                                declaration: identity.clone(),
                                side: ContextSide::New,
                                reason: ContextIssueReason::Missing,
                            });
                            None
                        }
                    };
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
                match &decl.kind {
                    Some(v2::decl::Kind::StructDef(def)) => {
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
            Some(Kind::Stream(stream)) => {
                if let Some(v2::stream_type::Element::Named(reference)) = &stream.element {
                    let ty = v2::FieldType {
                        optional: false,
                        kind: Some(Kind::Named(reference.clone())),
                    };
                    // Neither an element nor its nested members participates in
                    // the podbuf growth rule for a stream.
                    let widths_only = self.widths_only;
                    self.widths_only = true;
                    self.visit(home, &ty, path, false, side, active);
                    self.widths_only = widths_only;
                }
            }
            _ => {}
        }
    }

    fn widths(&mut self, declaring: &v2::Package, decl: &v2::Decl, path: &str, identity: &str) {
        use crate::walk::int_width_name;
        use v2::decl::Kind;

        let Some(new_decl) = self
            .new
            .iter()
            .copied()
            .find(|p| p.name == declaring.name)
            .and_then(|p| p.decls.iter().find(|d| d.name == decl.name))
        else {
            return;
        };
        let values = match (&decl.kind, &new_decl.kind) {
            (Some(Kind::EnumDef(old)), Some(Kind::EnumDef(new))) => {
                Some((&old.values, &new.values))
            }
            (Some(Kind::EnumSetDef(old)), Some(Kind::EnumSetDef(new))) => {
                Some((&old.bits, &new.bits))
            }
            _ => None,
        };
        // The local caller establishes additions. Here require a nonempty
        // append that preserves the old names, values and order.
        if let Some((before, after)) = values
            && crate::classify::is_strict_enum_append(decl, new_decl)
            && after.len() > before.len()
            && before
                .iter()
                .zip(after)
                .all(|(old, new)| old.name == new.name && old.value == new.value)
        {
            let names = |values: &[v2::EnumValue]| {
                values
                    .iter()
                    .map(|value| value.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            let category = Category::StrictEnumAppend;
            self.width_changes.insert(
                (
                    path.to_string(),
                    identity.to_string(),
                    category_word(category),
                ),
                Change {
                    path: path.to_string(),
                    category,
                    verdict: Verdict::Breaking,
                    before: Some(format!("{identity}: {}", names(before))),
                    after: Some(format!("{identity}: {}", names(after))),
                },
            );
            return;
        }
        let facts = match (&decl.kind, &new_decl.kind) {
            (Some(Kind::EnumSetDef(old)), Some(Kind::EnumSetDef(new))) => vec![
                (
                    Category::FlatbuffersWidthChange,
                    int_width_name(flatbuffers::enum_set_integer(old) as i32),
                    int_width_name(flatbuffers::enum_set_integer(new) as i32),
                    self.flatbuffers,
                ),
                (
                    Category::Proto3WidthChange,
                    proto3::enum_set_scalar(old.width).as_str(),
                    proto3::enum_set_scalar(new.width).as_str(),
                    self.proto3,
                ),
            ],
            (Some(Kind::EnumDef(old)), Some(Kind::EnumDef(new))) => vec![(
                Category::FlatbuffersWidthChange,
                int_width_name(flatbuffers::enum_integer(old) as i32),
                int_width_name(flatbuffers::enum_integer(new) as i32),
                self.flatbuffers,
            )],
            _ => return,
        };
        for (category, before, after, breaking) in facts {
            if before == after {
                continue;
            }
            self.width_changes.insert(
                (
                    path.to_string(),
                    identity.to_string(),
                    category_word(category),
                ),
                Change {
                    path: path.to_string(),
                    category,
                    verdict: if breaking {
                        Verdict::Breaking
                    } else {
                        Verdict::Compatible
                    },
                    before: Some(format!("{identity}: {before}")),
                    after: Some(format!("{identity}: {after}")),
                },
            );
        }
    }

    fn head_size(
        &mut self,
        scope: &[&v2::Package],
        home: &v2::Package,
        decl: &v2::Decl,
        path: &str,
        side: ContextSide,
        identity: &str,
    ) -> Option<u32> {
        let others: Vec<_> = scope
            .iter()
            .copied()
            .filter(|p| p.name != home.name)
            .collect();
        let ty = v2::FieldType {
            optional: false,
            kind: Some(v2::field_type::Kind::Named(decl.name.clone())),
        };
        match podbuf::field_form(
            Packages {
                package: home,
                others: &others,
            },
            &ty,
        ) {
            Ok(form) => Some(podbuf::elem_size(&form)),
            Err(_) => {
                self.issues.insert(ContextIssue {
                    path: path.to_string(),
                    declaration: identity.to_string(),
                    side,
                    reason: ContextIssueReason::Unlayable,
                });
                None
            }
        }
    }
}
