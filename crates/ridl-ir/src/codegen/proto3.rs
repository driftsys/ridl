//! The proto3 message layout of the lowered model, using the shared scalar
//! and field-number rules of [`crate::projection::proto3`].

use super::v1;
use crate::projection::proto3 as pb;

/// Every representable declaration and tuple, in model order. Unsupported
/// messages and their users are omitted; `refused` is empty.
pub(crate) fn project(model: &v1::Model) -> v1::Proto3Projection {
    use v1::proto3_message::Source;

    let mut sources = Vec::new();
    for (index, declaration) in model.declarations.iter().enumerate() {
        let source = match declaration.kind.as_ref() {
            Some(v1::declaration::Kind::Struct(_)) => Source::StructDeclaration(index as u32),
            Some(v1::declaration::Kind::Union(_)) => Source::UnionDeclaration(index as u32),
            Some(
                v1::declaration::Kind::Scalar(_)
                | v1::declaration::Kind::Enum(_)
                | v1::declaration::Kind::EnumSet(_),
            ) => Source::RootBox(index as u32),
            _ => continue,
        };
        sources.push(source);
    }
    sources.extend((0..model.tuples.len()).map(|index| Source::Tuple(index as u32)));

    let mut included = vec![true; sources.len()];
    loop {
        // Assign every index before walking fields, including recursive
        // references. Rebuild after an omission so that indices stay dense.
        let mut declarations = vec![None; model.declarations.len()];
        let mut tuples = vec![None; model.tuples.len()];
        let mut next = 0;
        for (source, include) in sources.iter().zip(&included) {
            if !include {
                continue;
            }
            let target = match *source {
                Source::StructDeclaration(index)
                | Source::UnionDeclaration(index)
                | Source::RootBox(index) => &mut declarations[index as usize],
                Source::Tuple(index) => &mut tuples[index as usize],
            };
            *target = Some(next);
            next += 1;
        }
        let walk = Walk {
            model,
            declarations: &declarations,
            tuples: &tuples,
        };
        let mut messages = Vec::new();
        let mut omitted = false;
        for (source, include) in sources.iter().zip(&mut included) {
            if !*include {
                continue;
            }
            match walk.message(*source) {
                Some(message) => messages.push(message),
                None => {
                    *include = false;
                    omitted = true;
                }
            }
        }
        if !omitted {
            return v1::Proto3Projection {
                messages,
                refused: Vec::new(),
            };
        }
    }
}

struct Walk<'a> {
    model: &'a v1::Model,
    declarations: &'a [Option<u32>],
    tuples: &'a [Option<u32>],
}

impl Walk<'_> {
    fn message(&self, source: v1::proto3_message::Source) -> Option<v1::Proto3Message> {
        use v1::proto3_field::Source as FieldSource;
        use v1::proto3_message::Source;

        let mut fields = Vec::new();
        let name = match source {
            Source::Tuple(index) => {
                let tuple = self.model.tuples.get(index as usize)?;
                let name = tuple.name.as_ref()?;
                if name.rust.is_empty() || name.wire.is_empty() {
                    return None;
                }
                for (position, field) in tuple.fields.iter().enumerate() {
                    let number = u32::try_from(position + 1).ok()?;
                    fields.push(self.field(
                        number,
                        field.r#type.as_ref()?,
                        FieldSource::TuplePosition(position as u32),
                    )?);
                }
                name.wire.clone()
            }
            Source::StructDeclaration(index)
            | Source::UnionDeclaration(index)
            | Source::RootBox(index) => {
                let declaration = self.model.declarations.get(index as usize)?;
                let name = declaration.name.as_ref()?.declared.clone();
                match declaration.kind.as_ref()? {
                    v1::declaration::Kind::Struct(def) => {
                        for slot in &def.slots {
                            let Some(v1::slot::Occupant::Field(field)) = slot.occupant.as_ref()
                            else {
                                continue;
                            };
                            fields.push(self.field(
                                slot.ordinal,
                                field.r#type.as_ref()?,
                                FieldSource::Field(field.name.as_ref()?.declared.clone()),
                            )?);
                        }
                        name
                    }
                    v1::declaration::Kind::Union(def) => {
                        if def.arms.is_empty() {
                            return None;
                        }
                        for arm in &def.arms {
                            fields.push(v1::Proto3Field {
                                number: field_number(arm.ordinal)?,
                                wire: Some(self.declaration_wire(arm.r#type.as_ref()?)?),
                                source: Some(FieldSource::Arm(arm.name.as_ref()?.declared.clone())),
                            });
                        }
                        name
                    }
                    _ => {
                        let reference = v1::TypeRef {
                            reference: name.clone(),
                            resolved: true,
                            package: self.model.scope.as_ref()?.package.clone(),
                            foreign: false,
                            index,
                            kind: declaration_kind(declaration),
                        };
                        fields.push(v1::Proto3Field {
                            number: 1,
                            wire: Some(self.declaration_wire(&reference)?),
                            source: Some(FieldSource::BoxValue(true)),
                        });
                        format!("{name}Box")
                    }
                }
            }
        };
        fields.sort_by_key(|field| field.number);
        Some(v1::Proto3Message {
            name,
            fields,
            source: Some(source),
        })
    }

    fn field(
        &self,
        number: u32,
        ty: &v1::Type,
        source: v1::proto3_field::Source,
    ) -> Option<v1::Proto3Field> {
        if ty.optional
            && matches!(
                ty.kind,
                Some(v1::r#type::Kind::Array(_) | v1::r#type::Kind::Map(_))
            )
        {
            return None;
        }
        Some(v1::Proto3Field {
            number: field_number(number)?,
            wire: Some(self.wire(ty)?),
            source: Some(source),
        })
    }

    fn wire(&self, ty: &v1::Type) -> Option<v1::Proto3Wire> {
        use v1::proto3_wire::Kind;

        let kind = match ty.kind.as_ref()? {
            v1::r#type::Kind::Primitive(primitive) => {
                match v1::PrimitiveType::try_from(*primitive).ok()? {
                    v1::PrimitiveType::Boolean
                    | v1::PrimitiveType::Integer
                    | v1::PrimitiveType::Float => {
                        return Some(scalar_wire(pb::primitive(*primitive)));
                    }
                    _ => return None,
                }
            }
            v1::r#type::Kind::Inline(scalar) => return project_scalar(scalar),
            v1::r#type::Kind::Named(reference) => return self.declaration_wire(reference),
            v1::r#type::Kind::Tuple(reference) => Kind::Message(v1::Proto3MessageRef {
                target: Some(v1::proto3_message_ref::Target::Message(
                    *self.tuples.get(reference.index as usize)?.as_ref()?,
                )),
            }),
            v1::r#type::Kind::Array(array) => {
                let element = self.element(array.element.as_deref()?)?;
                let packed = matches!(element.kind, Some(Kind::Scalar(_) | Kind::Enumeration(_)));
                Kind::Array(
                    v1::Proto3Array {
                        element: Some(element.into()),
                        packed,
                    }
                    .into(),
                )
            }
            v1::r#type::Kind::Map(map) => {
                let key = self.element(map.key.as_deref()?)?;
                let admitted = match key.kind.as_ref()? {
                    Kind::Text(_) => true,
                    Kind::Enumeration(_) => pb::Scalar::Int64.admitted_as_map_key(),
                    Kind::Scalar(_) => {
                        // Read the shared map-key table from the key's type.
                        let scalar = self.map_key_scalar(map.key.as_deref()?)?;
                        scalar.admitted_as_map_key()
                    }
                    _ => false,
                };
                if !admitted {
                    return None;
                }
                let value = self.element(map.value.as_deref()?)?;
                Kind::Map(
                    v1::Proto3Map {
                        key: Some(key.into()),
                        value: Some(value.into()),
                    }
                    .into(),
                )
            }
            _ => return None,
        };
        Some(v1::Proto3Wire { kind: Some(kind) })
    }

    fn element(&self, ty: &v1::Type) -> Option<v1::Proto3Wire> {
        if ty.optional {
            return None;
        }
        let wire = self.wire(ty)?;
        if matches!(
            wire.kind,
            Some(v1::proto3_wire::Kind::Array(_) | v1::proto3_wire::Kind::Map(_))
        ) {
            return None;
        }
        Some(wire)
    }

    fn resolve(&self, reference: &v1::TypeRef) -> Option<&v1::Declaration> {
        if !reference.resolved || reference.package.is_empty() {
            return None;
        }
        if reference.foreign {
            self.model
                .foreign
                .get(reference.index as usize)?
                .declaration
                .as_ref()
        } else {
            self.model.declarations.get(reference.index as usize)
        }
    }

    fn declaration_wire(&self, reference: &v1::TypeRef) -> Option<v1::Proto3Wire> {
        use v1::proto3_wire::Kind;

        let declaration = self.resolve(reference)?;
        let kind = match declaration.kind.as_ref()? {
            v1::declaration::Kind::Scalar(scalar) => return project_scalar(scalar),
            v1::declaration::Kind::Enum(def) => {
                if (!def.open && def.values.is_empty())
                    || def
                        .values
                        .iter()
                        .map(|value| value.value)
                        .chain(def.retired.iter().filter_map(|value| value.value))
                        .any(|value| i32::try_from(value).is_err())
                {
                    return None;
                }
                Kind::Enumeration(v1::Proto3Enum {
                    r#type: Some(reference.clone()),
                })
            }
            v1::declaration::Kind::EnumSet(def) => {
                pb::int_width_scalar(v1::IntWidth::try_from(def.width).ok()?)?;
                return Some(scalar_wire(pb::enum_set_scalar(def.width)));
            }
            v1::declaration::Kind::Struct(_) | v1::declaration::Kind::Union(_) => {
                let target = if reference.foreign {
                    v1::proto3_message_ref::Target::Foreign(reference.index)
                } else {
                    v1::proto3_message_ref::Target::Message(
                        *self.declarations.get(reference.index as usize)?.as_ref()?,
                    )
                };
                Kind::Message(v1::Proto3MessageRef {
                    target: Some(target),
                })
            }
            _ => return None,
        };
        Some(v1::Proto3Wire { kind: Some(kind) })
    }

    fn map_key_scalar(&self, ty: &v1::Type) -> Option<pb::Scalar> {
        match ty.kind.as_ref()? {
            v1::r#type::Kind::Primitive(primitive) => Some(pb::primitive(*primitive)),
            v1::r#type::Kind::Inline(scalar) => scalar_type(scalar),
            v1::r#type::Kind::Named(reference) => match self.resolve(reference)?.kind.as_ref()? {
                v1::declaration::Kind::Scalar(scalar) => scalar_type(scalar),
                v1::declaration::Kind::EnumSet(def) => Some(pb::enum_set_scalar(def.width)),
                _ => None,
            },
            _ => None,
        }
    }
}

fn declaration_kind(declaration: &v1::Declaration) -> i32 {
    match declaration.kind.as_ref() {
        Some(v1::declaration::Kind::Scalar(_)) => v1::DeclKind::Scalar as i32,
        Some(v1::declaration::Kind::Enum(_)) => v1::DeclKind::Enum as i32,
        Some(v1::declaration::Kind::EnumSet(_)) => v1::DeclKind::EnumSet as i32,
        _ => v1::DeclKind::Unspecified as i32,
    }
}

fn field_number(number: u32) -> Option<u32> {
    (number != 0 && !pb::PROTO_RESERVED.contains(&number) && number <= pb::PROTO_MAX_FIELD_NUMBER)
        .then_some(number)
}

fn scalar_type(scalar: &v1::Scalar) -> Option<pb::Scalar> {
    match scalar.width {
        Some(v1::scalar::Width::IntWidth(width)) => {
            pb::int_width_scalar(v1::IntWidth::try_from(width).ok()?)
        }
        Some(v1::scalar::Width::FloatWidth(width)) => {
            pb::float_width_scalar(v1::FloatWidth::try_from(width).ok()?)
        }
        None => match v1::ScalarClass::try_from(scalar.class).ok()? {
            v1::ScalarClass::Boolean => Some(pb::Scalar::Bool),
            v1::ScalarClass::String => Some(pb::Scalar::String),
            v1::ScalarClass::Bytes => Some(pb::Scalar::Bytes),
            _ => None,
        },
    }
}

fn project_scalar(scalar: &v1::Scalar) -> Option<v1::Proto3Wire> {
    scalar_type(scalar).map(scalar_wire)
}

fn scalar_wire(scalar: pb::Scalar) -> v1::Proto3Wire {
    use v1::proto3_wire::Kind;

    let ty = match scalar {
        pb::Scalar::String => {
            return v1::Proto3Wire {
                kind: Some(Kind::Text(true)),
            };
        }
        pb::Scalar::Bytes => {
            return v1::Proto3Wire {
                kind: Some(Kind::Bytes(true)),
            };
        }
        pb::Scalar::Bool => v1::Proto3ScalarType::Bool,
        pb::Scalar::Int64 => v1::Proto3ScalarType::Int64,
        pb::Scalar::Uint32 => v1::Proto3ScalarType::Uint32,
        pb::Scalar::Uint64 => v1::Proto3ScalarType::Uint64,
        pb::Scalar::Sint32 => v1::Proto3ScalarType::Sint32,
        pb::Scalar::Sint64 => v1::Proto3ScalarType::Sint64,
        pb::Scalar::Float => v1::Proto3ScalarType::Float,
        pb::Scalar::Double => v1::Proto3ScalarType::Double,
    };
    let wire_type = match scalar.wire_type() {
        pb::WireType::Varint => v1::Proto3WireType::Varint,
        pb::WireType::I64 => v1::Proto3WireType::I64,
        pb::WireType::Len => v1::Proto3WireType::Len,
        pb::WireType::I32 => v1::Proto3WireType::I32,
    };
    v1::Proto3Wire {
        kind: Some(Kind::Scalar(v1::Proto3Scalar {
            r#type: ty as i32,
            wire_type: wire_type as i32,
        })),
    }
}
