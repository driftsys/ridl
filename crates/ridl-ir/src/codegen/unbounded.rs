//! The attribution of a missing FlatBuffers bound (design note §3.7).
//!
//! `ridl-backend-rust`'s `unbounded_member` and `unjudgeable_members`, moved
//! here as model facts. It is computed **over the package alone**, as
//! `check_flatbuffers_bound` computes it today (`others: &[]`) even when the
//! pipeline handed the codec a scope — which is what byte identity requires,
//! and which design note §9 item 7 records as less precise than it could be.

use std::collections::HashSet;

use super::v1;
use crate::projection::flatbuffers as fb;
use crate::v2;

/// Why `decl`'s own `max_size` answered `None`.
pub(crate) fn attribute(package: &v2::Package, decl: &v2::Decl) -> v1::FbUnbounded {
    let mut any_exempt = false;
    match &decl.kind {
        Some(v2::decl::Kind::StructDef(def)) => {
            // The layout is refused for the declaration as a whole, and it is
            // checked first: with two members on one ordinal, every member
            // probes as bounded and only the aggregate answers `None`.
            if let Err(err) = fb::struct_table(&decl.name, def) {
                return cause(v1::FbUnboundedCause::Layout, None, Some(err.message));
            }
            for member in &def.members {
                let Some(v2::struct_member::Member::Field(field)) = &member.member else {
                    continue;
                };
                let Some(ty) = field.r#type.as_ref() else {
                    return cause(
                        v1::FbUnboundedCause::Untyped,
                        Some(field.name.clone()),
                        None,
                    );
                };
                match judge(package, ty) {
                    Verdict::Unbounded => {
                        return cause(v1::FbUnboundedCause::Member, Some(field.name.clone()), None);
                    }
                    Verdict::Unjudgeable => any_exempt = true,
                    Verdict::Bounded => {}
                }
            }
        }
        Some(v2::decl::Kind::UnionDef(def)) => {
            for arm in &def.arms {
                let ty = named(&arm.type_ref);
                match judge(package, &ty) {
                    Verdict::Unbounded => {
                        return cause(v1::FbUnboundedCause::Member, Some(arm.name.clone()), None);
                    }
                    Verdict::Unjudgeable => any_exempt = true,
                    Verdict::Bounded => {}
                }
            }
        }
        Some(
            v2::decl::Kind::TypeDef(_) | v2::decl::Kind::EnumDef(_) | v2::decl::Kind::EnumSetDef(_),
        ) => {
            // A named scalar with no backing at all is malformed IR rather
            // than an unbounded shape.
            if let Some(v2::decl::Kind::TypeDef(td)) = &decl.kind
                && td.backing.is_none()
            {
                return cause(
                    v1::FbUnboundedCause::Untyped,
                    Some("value".to_string()),
                    None,
                );
            }
            match judge(package, &named(&decl.name)) {
                Verdict::Unbounded => {
                    return cause(
                        v1::FbUnboundedCause::Member,
                        Some("value".to_string()),
                        None,
                    );
                }
                Verdict::Unjudgeable => any_exempt = true,
                Verdict::Bounded => {}
            }
        }
        _ => {}
    }
    if !any_exempt {
        return cause(v1::FbUnboundedCause::Aggregate, None, None);
    }
    // Every judged member is bounded and at least one could not be judged, so
    // the aggregate is still open: two members each under the ceiling can sum
    // over it, and a member that cannot be judged must not shield that sum.
    if let Some(v2::decl::Kind::StructDef(def)) = &decl.kind {
        let stand_in = v2::Decl {
            kind: Some(v2::decl::Kind::StructDef(v2::StructDef {
                members: def
                    .members
                    .iter()
                    .map(|member| match &member.member {
                        Some(v2::struct_member::Member::Field(field)) => v2::StructMember {
                            member: Some(v2::struct_member::Member::Field(v2::Field {
                                r#type: field
                                    .r#type
                                    .as_ref()
                                    .map(|ty| lower_bound_stand_in(package, ty)),
                                ..field.clone()
                            })),
                        },
                        _ => member.clone(),
                    })
                    .collect(),
                fixed_layout: def.fixed_layout,
            })),
            ..decl.clone()
        };
        if fb::max_size(alone(package), &stand_in).is_none() {
            return cause(v1::FbUnboundedCause::Aggregate, None, None);
        }
    }
    v1::FbUnbounded {
        cause: v1::FbUnboundedCause::Exempt as i32,
        member: None,
        layout_message: None,
        unjudgeable_members: unjudgeable_members(package, decl),
    }
}

/// The members of `decl` this attribution cannot judge — a cross-package
/// reference, a same-package cycle, a stream, or an unspecified primitive.
/// What a withheld codec's note lists.
pub(crate) fn unjudgeable_members(package: &v2::Package, decl: &v2::Decl) -> Vec<String> {
    let mut names = Vec::new();
    match &decl.kind {
        Some(v2::decl::Kind::StructDef(def)) => {
            for member in &def.members {
                if let Some(v2::struct_member::Member::Field(field)) = &member.member
                    && let Some(ty) = field.r#type.as_ref()
                    && !field_type_resolves_locally(package, ty, &mut HashSet::new())
                {
                    names.push(field.name.clone());
                }
            }
        }
        Some(v2::decl::Kind::UnionDef(def)) => {
            for arm in &def.arms {
                if !decl_resolves_locally(package, &arm.type_ref, &mut HashSet::new()) {
                    names.push(arm.name.clone());
                }
            }
        }
        _ => {}
    }
    names
}

fn cause(
    cause: v1::FbUnboundedCause,
    member: Option<String>,
    layout_message: Option<String>,
) -> v1::FbUnbounded {
    v1::FbUnbounded {
        cause: cause as i32,
        member,
        layout_message,
        unjudgeable_members: Vec::new(),
    }
}

fn alone(package: &v2::Package) -> fb::Packages<'_> {
    fb::Packages {
        package,
        others: &[],
    }
}

fn named(reference: &str) -> v2::FieldType {
    v2::FieldType {
        optional: false,
        kind: Some(v2::field_type::Kind::Named(reference.to_string())),
    }
}

/// What one type position contributes to the attribution.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Verdict {
    Bounded,
    Unbounded,
    /// The position reaches a cross-package reference or a cycle.
    Unjudgeable,
}

fn judge(package: &v2::Package, ty: &v2::FieldType) -> Verdict {
    if field_type_resolves_locally(package, ty, &mut HashSet::new()) {
        return if probe(package, ty).is_some() {
            Verdict::Bounded
        } else {
            Verdict::Unbounded
        };
    }
    if probe(package, &lower_bound_stand_in(package, ty)).is_some() {
        Verdict::Unjudgeable
    } else {
        Verdict::Unbounded
    }
}

/// `ty` with every leaf that cannot be judged replaced by a `boolean`, the
/// smallest thing the projection charges anything for.
fn lower_bound_stand_in(package: &v2::Package, ty: &v2::FieldType) -> v2::FieldType {
    fn boolean(optional: bool) -> v2::FieldType {
        v2::FieldType {
            optional,
            kind: Some(v2::field_type::Kind::Primitive(
                v2::PrimitiveType::Boolean as i32,
            )),
        }
    }
    let kind = match ty.kind.as_ref() {
        Some(v2::field_type::Kind::Array(array)) => {
            v2::field_type::Kind::Array(Box::new(v2::ArrayType {
                element: array
                    .element
                    .as_deref()
                    .map(|leaf| Box::new(lower_bound_stand_in(package, leaf))),
                min: array.min,
                max: array.max,
            }))
        }
        Some(v2::field_type::Kind::Map(map)) => v2::field_type::Kind::Map(Box::new(v2::MapType {
            key: map
                .key
                .as_deref()
                .map(|leaf| Box::new(lower_bound_stand_in(package, leaf))),
            value: map
                .value
                .as_deref()
                .map(|leaf| Box::new(lower_bound_stand_in(package, leaf))),
            min: map.min,
            max: map.max,
        })),
        Some(v2::field_type::Kind::Tuple(tuple)) => v2::field_type::Kind::Tuple(v2::TupleType {
            fields: tuple
                .fields
                .iter()
                .map(|field| v2::TupleField {
                    r#type: field
                        .r#type
                        .as_ref()
                        .map(|leaf| lower_bound_stand_in(package, leaf)),
                    ..field.clone()
                })
                .collect(),
        }),
        _ if field_type_resolves_locally(package, ty, &mut HashSet::new()) => return ty.clone(),
        _ => return boolean(ty.optional),
    };
    v2::FieldType {
        optional: ty.optional,
        kind: Some(kind),
    }
}

/// The bound of one type position alone, charged as `max_size` charges it as
/// one member of a struct.
fn probe(package: &v2::Package, ty: &v2::FieldType) -> Option<u64> {
    let probe = v2::Decl {
        kind: Some(v2::decl::Kind::StructDef(v2::StructDef {
            members: vec![v2::StructMember {
                member: Some(v2::struct_member::Member::Field(v2::Field {
                    ordinal: 1,
                    r#type: Some(ty.clone()),
                    ..Default::default()
                })),
            }],
            fixed_layout: false,
        })),
        ..Default::default()
    };
    fb::max_size(alone(package), &probe)
}

fn decl_resolves_locally(
    package: &v2::Package,
    reference: &str,
    visiting: &mut HashSet<String>,
) -> bool {
    let Some(decl) = package.decls.iter().find(|decl| decl.name == reference) else {
        return false;
    };
    if !visiting.insert(reference.to_string()) {
        return false;
    }
    let resolves = match &decl.kind {
        Some(v2::decl::Kind::StructDef(def)) => {
            def.members.iter().all(|member| match &member.member {
                Some(v2::struct_member::Member::Field(field)) => field
                    .r#type
                    .as_ref()
                    .is_some_and(|ty| field_type_resolves_locally(package, ty, visiting)),
                _ => true,
            })
        }
        Some(v2::decl::Kind::UnionDef(def)) => def
            .arms
            .iter()
            .all(|arm| decl_resolves_locally(package, &arm.type_ref, visiting)),
        _ => true,
    };
    visiting.remove(reference);
    resolves
}

fn field_type_resolves_locally(
    package: &v2::Package,
    ty: &v2::FieldType,
    visiting: &mut HashSet<String>,
) -> bool {
    match &ty.kind {
        Some(v2::field_type::Kind::Named(reference)) => {
            decl_resolves_locally(package, reference, visiting)
        }
        Some(v2::field_type::Kind::Primitive(primitive)) => v2::PrimitiveType::try_from(*primitive)
            .is_ok_and(|primitive| primitive != v2::PrimitiveType::Unspecified),
        Some(v2::field_type::Kind::InlineScalar(_)) => true,
        Some(v2::field_type::Kind::Tuple(tuple)) => tuple.fields.iter().all(|field| {
            field
                .r#type
                .as_ref()
                .is_some_and(|ty| field_type_resolves_locally(package, ty, visiting))
        }),
        Some(v2::field_type::Kind::Array(array)) => array
            .element
            .as_deref()
            .is_some_and(|element| field_type_resolves_locally(package, element, visiting)),
        Some(v2::field_type::Kind::Map(map)) => {
            map.key
                .as_deref()
                .is_some_and(|key| field_type_resolves_locally(package, key, visiting))
                && map
                    .value
                    .as_deref()
                    .is_some_and(|value| field_type_resolves_locally(package, value, visiting))
        }
        Some(v2::field_type::Kind::Stream(_)) => false,
        None => true,
    }
}
