//! Consumer direction and transitive type reachability for scalar constraints.

use std::collections::{BTreeMap, BTreeSet};

use ridl_ir::v2;

use crate::Verdict;

/// Interaction roots paired with whether the consumer receives the value.
/// Request streams are sent values even though they have no request head.
pub(crate) fn roots(interaction: &v2::Decl) -> Vec<(v2::FieldType, bool)> {
    use v2::decl::Kind;
    let mut roots = Vec::new();
    let mut params = |params: &[v2::Param]| {
        roots.extend(
            params
                .iter()
                .filter_map(|p| p.r#type.clone())
                .map(|ty| (ty, false)),
        );
    };
    match &interaction.kind {
        Some(Kind::CommandDef(def)) => params(&def.params),
        Some(Kind::QueryDef(def)) => {
            params(&def.params);
            match def.return_type.as_ref().and_then(|ret| ret.kind.as_ref()) {
                Some(v2::return_type::Kind::Value(ty)) => roots.push((ty.clone(), true)),
                Some(v2::return_type::Kind::Fallible(ret)) => {
                    roots.push((named(&ret.ok), true));
                    roots.push((named(&ret.err), true));
                }
                None => {}
            }
        }
        Some(Kind::SignalDef(def)) => roots.push((named(&def.payload), true)),
        Some(Kind::EventDef(def)) => roots.push((named(&def.payload), true)),
        Some(Kind::FixedDef(def)) => roots.extend(def.payload.clone().map(|ty| (ty, true))),
        _ => {}
    }
    roots
}

fn named(name: &str) -> v2::FieldType {
    v2::FieldType {
        kind: Some(v2::field_type::Kind::Named(name.to_string())),
        ..Default::default()
    }
}

/// All declarations reached from a root, keyed by canonical name. Bare names
/// resolve in the declaring package, as in `ridl_ir::catalog_hash`.
pub(crate) fn reached<'a>(
    home: &'a v2::Package,
    ty: &v2::FieldType,
    scope: &[&'a v2::Package],
) -> BTreeMap<String, (&'a v2::Package, &'a v2::Decl)> {
    let mut reached = BTreeMap::new();
    visit(home, ty, &packages(home, scope), &mut reached);
    reached
}

// Local packages precede context packages; a shadowed package contributes
// neither declarations nor interactions, even when its declaration is absent
// from the package that takes precedence.
fn packages<'a>(home: &'a v2::Package, scope: &[&'a v2::Package]) -> Vec<&'a v2::Package> {
    let mut names = BTreeSet::new();
    std::iter::once(home)
        .chain(scope.iter().copied())
        .filter(|p| names.insert(p.name.as_str()))
        .collect()
}

fn visit<'a>(
    home: &'a v2::Package,
    ty: &v2::FieldType,
    scope: &[&'a v2::Package],
    reached: &mut BTreeMap<String, (&'a v2::Package, &'a v2::Decl)>,
) {
    use v2::{decl, field_type::Kind};
    match &ty.kind {
        Some(Kind::Named(name)) => {
            let found = home
                .decls
                .iter()
                .find(|d| &d.name == name)
                .map(|d| (home, d))
                .or_else(|| {
                    std::iter::once(home)
                        .chain(scope.iter().copied())
                        .find_map(|p| {
                            p.decls
                                .iter()
                                .find(|d| {
                                    name.strip_prefix(&p.name).and_then(|n| n.strip_prefix('.'))
                                        == Some(d.name.as_str())
                                })
                                .map(|d| (p, d))
                        })
                });
            let Some((home, decl)) = found else {
                return;
            };
            if reached
                .insert(format!("{}.{}", home.name, decl.name), (home, decl))
                .is_some()
            {
                return;
            }
            match &decl.kind {
                Some(decl::Kind::StructDef(def)) => {
                    for member in &def.members {
                        if let Some(v2::struct_member::Member::Field(field)) = &member.member
                            && let Some(ty) = &field.r#type
                        {
                            visit(home, ty, scope, reached);
                        }
                    }
                }
                Some(decl::Kind::UnionDef(def)) => {
                    for arm in &def.arms {
                        visit(home, &named(&arm.type_ref), scope, reached);
                    }
                }
                // Backing has primitive and UCUM-unit variants, no type alias.
                _ => {}
            }
        }
        Some(Kind::Tuple(tuple)) => {
            for field in &tuple.fields {
                if let Some(ty) = &field.r#type {
                    visit(home, ty, scope, reached);
                }
            }
        }
        Some(Kind::Array(array)) => {
            if let Some(ty) = &array.element {
                visit(home, ty, scope, reached);
            }
        }
        Some(Kind::Map(map)) => {
            for ty in [&map.key, &map.value].into_iter().flatten() {
                visit(home, ty, scope, reached);
            }
        }
        Some(Kind::Stream(stream)) => {
            if let Some(v2::stream_type::Element::Named(name)) = &stream.element {
                visit(home, &named(name), scope, reached);
            }
        }
        _ => {}
    }
}

type ScopeKey = (usize, Vec<usize>);
type Uses = BTreeMap<String, (bool, bool)>;

/// Direction maps retained only for one comparison, keyed by package addresses.
#[derive(Default)]
pub(crate) struct DirectionCache {
    uses: BTreeMap<ScopeKey, Uses>,
}

impl DirectionCache {
    /// A widening needs a sent use and no received use; unreached types break.
    pub(crate) fn widening_verdict(
        &mut self,
        new: &v2::Package,
        scope: &[&v2::Package],
        identity: &str,
    ) -> Verdict {
        let key = (
            std::ptr::from_ref(new) as usize,
            scope
                .iter()
                .map(|p| std::ptr::from_ref(*p) as usize)
                .collect(),
        );
        let uses = self.uses.entry(key).or_insert_with(|| {
            let mut uses = BTreeMap::<String, (bool, bool)>::new();
            let scope = packages(new, scope);
            for home in &scope {
                for shape in home.shapes() {
                    for interaction in &shape.interface.interactions {
                        for (ty, received) in roots(interaction) {
                            for identity in reached(home, &ty, &scope).into_keys() {
                                let flags = uses.entry(identity).or_default();
                                if received {
                                    flags.1 = true;
                                } else {
                                    flags.0 = true;
                                }
                            }
                        }
                    }
                }
            }
            uses
        });
        if uses.get(identity) == Some(&(true, false)) {
            Verdict::Compatible
        } else {
            Verdict::Breaking
        }
    }
}
