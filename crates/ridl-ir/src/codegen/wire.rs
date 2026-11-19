//! Adapts the model's interaction tuples for the wire printers that still
//! read raw declarations. Names and tuple references come from the model;
//! the appended structs are resolution entries, not new IR identities.

use super::v1;
use crate::v2;

/// The name projection an interaction tuple uses in a wire schema.
#[derive(Clone, Copy)]
pub enum Schema {
    Proto3,
    FlatBuffers,
}

/// Appends resolution entries for the interaction tuples the lowering named.
///
/// The original declarations stay in place. A printer emits the appended
/// structs as positional tuple messages or tables, not as declared structs.
pub fn with_interaction_tuples(
    package: &v2::Package,
    model: &v1::Model,
    schema: Schema,
) -> Result<v2::Package, String> {
    let mut package = package.clone();
    for tuple in &model.tuples {
        if !matches!(
            tuple.origin,
            Some(v1::induced_tuple::Origin::Interaction(_))
        ) {
            continue;
        }
        let name = tuple_name(model, tuple, schema)?;
        let members = tuple
            .fields
            .iter()
            .enumerate()
            .map(|(index, field)| {
                Ok(v2::StructMember {
                    member: Some(v2::struct_member::Member::Field(Box::new(v2::Field {
                        name: field
                            .name
                            .as_ref()
                            .map(|name| name.declared.clone())
                            .unwrap_or_default(),
                        ordinal: u32::try_from(index + 1).map_err(|_| "too many tuple fields")?,
                        r#type: field
                            .r#type
                            .as_ref()
                            .map(|ty| type_position(model, ty, schema))
                            .transpose()?,
                        ..Default::default()
                    }))),
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        package.decls.push(v2::Decl {
            name: name.to_string(),
            visibility: tuple.visibility,
            kind: Some(v2::decl::Kind::StructDef(v2::StructDef {
                members,
                ..Default::default()
            })),
            ..Default::default()
        });
    }
    Ok(package)
}

fn tuple_name<'a>(
    model: &'a v1::Model,
    tuple: &'a v1::InducedTuple,
    schema: Schema,
) -> Result<&'a str, String> {
    let name = match schema {
        Schema::Proto3 => tuple.name.as_ref().map(|name| name.wire.as_str()),
        Schema::FlatBuffers => model.flatbuffers.as_ref().and_then(|projection| {
            projection
                .tables
                .get(tuple.flatbuffers_table? as usize)
                .map(|table| table.name.as_str())
        }),
    };
    name.filter(|name| !name.is_empty())
        .ok_or_else(|| "an interaction tuple has no lowered wire name".to_string())
}

fn type_position(
    model: &v1::Model,
    ty: &v1::Type,
    schema: Schema,
) -> Result<v2::FieldType, String> {
    let kind = match ty.kind.as_ref() {
        Some(v1::r#type::Kind::Named(reference)) => {
            Some(v2::field_type::Kind::Named(reference.reference.clone()))
        }
        Some(v1::r#type::Kind::Primitive(primitive)) => {
            Some(v2::field_type::Kind::Primitive(*primitive))
        }
        Some(v1::r#type::Kind::Inline(scalar)) => Some(v2::field_type::Kind::InlineScalar(
            Box::new(scalar_type(scalar)),
        )),
        Some(v1::r#type::Kind::Tuple(reference)) => {
            let tuple = model
                .tuples
                .get(reference.index as usize)
                .ok_or_else(|| "an interaction field names a missing tuple".to_string())?;
            Some(v2::field_type::Kind::Named(
                tuple_name(model, tuple, schema)?.to_string(),
            ))
        }
        Some(v1::r#type::Kind::Array(array)) => {
            Some(v2::field_type::Kind::Array(Box::new(v2::ArrayType {
                element: array
                    .element
                    .as_deref()
                    .map(|element| type_position(model, element, schema).map(Box::new))
                    .transpose()?,
                min: array.min,
                max: array.max,
            })))
        }
        Some(v1::r#type::Kind::Map(map)) => {
            Some(v2::field_type::Kind::Map(Box::new(v2::MapType {
                key: map
                    .key
                    .as_deref()
                    .map(|key| type_position(model, key, schema).map(Box::new))
                    .transpose()?,
                value: map
                    .value
                    .as_deref()
                    .map(|value| type_position(model, value, schema).map(Box::new))
                    .transpose()?,
                min: map.min,
                max: map.max,
            })))
        }
        Some(v1::r#type::Kind::Stream(_)) => {
            return Err("an interaction tuple contains a stream".to_string());
        }
        None => None,
    };
    Ok(v2::FieldType {
        optional: ty.optional,
        kind,
    })
}

#[allow(deprecated)]
fn scalar_type(scalar: &v1::Scalar) -> v2::TypeDef {
    let primitive = match v1::ScalarClass::try_from(scalar.class).ok() {
        Some(v1::ScalarClass::Integer) => v2::PrimitiveType::Integer,
        Some(v1::ScalarClass::Float) => v2::PrimitiveType::Float,
        Some(v1::ScalarClass::Boolean) => v2::PrimitiveType::Boolean,
        Some(v1::ScalarClass::String) => v2::PrimitiveType::String,
        _ => v2::PrimitiveType::Bytes,
    };
    v2::TypeDef {
        backing: Some(v2::Backing {
            kind: Some(v2::backing::Kind::Primitive(primitive as i32)),
        }),
        width: scalar.width.as_ref().map(|width| match width {
            v1::scalar::Width::IntWidth(width) => v2::type_def::Width::IntWidth(*width),
            v1::scalar::Width::FloatWidth(width) => v2::type_def::Width::FloatWidth(*width),
        }),
        constraint: scalar.constraint.as_ref().map(|constraint| v2::Constraint {
            min: constraint.min.clone(),
            max: constraint.max.clone(),
            step: constraint.step.clone(),
            len_min: constraint.len_min,
            len_max: constraint.len_max,
            pattern: constraint.pattern.clone(),
            pattern_const: constraint.pattern_const.clone(),
            matches: constraint
                .matches
                .iter()
                .map(|clause| v2::Match {
                    pattern: clause.pattern.clone(),
                    pattern_const: clause.pattern_const.clone(),
                })
                .collect(),
        }),
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    //! Checks that wire tuple adaptation retains every scalar clause.
    use super::*;

    #[test]
    fn match_clauses_survive_model_to_ir_adaptation() {
        let sc = v1::Scalar {
            class: v1::ScalarClass::String as i32,
            constraint: Some(v1::Constraint {
                matches: vec![
                    v1::Match {
                        pattern: Some("first".into()),
                        pattern_const: None,
                    },
                    v1::Match {
                        pattern: Some("second".into()),
                        pattern_const: Some("P".into()),
                    },
                ],
                ..Default::default()
            }),
            ..Default::default()
        };
        let c = scalar_type(&sc).constraint.unwrap();
        assert_eq!(
            c.matches,
            vec![
                v2::Match {
                    pattern: Some("first".into()),
                    pattern_const: None
                },
                v2::Match {
                    pattern: Some("second".into()),
                    pattern_const: Some("P".into())
                },
            ]
        );
    }
}
