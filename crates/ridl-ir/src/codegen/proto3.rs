//! The proto3 message layout and refusals of the lowered model, using the
//! shared scalar and field-number rules of [`crate::projection::proto3`].
//! The Rust codec and plugins read these facts from `Model.proto3`; the
//! codec retains the derivations specific to Rust described in
//! `docs/design/proto3-codec.md`. A message that needs a refused local
//! message is refused transitively.

use super::v1;
use crate::projection::proto3 as pb;
use v1::Proto3RefusalCause as Cause;

/// Every representable declaration and named tuple, and every refusal, in model order.
pub(crate) fn project(model: &v1::Model) -> v1::Proto3Projection {
    use v1::proto3_message::Source;

    let mut sources = Vec::new();
    let mut declarations = vec![None; model.declarations.len()];
    let mut tuples = vec![None; model.tuples.len()];
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
        declarations[index] = Some(sources.len() as u32);
        sources.push(source);
    }
    for (index, target) in tuples.iter_mut().enumerate() {
        *target = Some(sources.len() as u32);
        sources.push(Source::Tuple(index as u32));
    }
    // Assign indices before walking any fields, including recursive ones.
    let walk = Walk {
        model,
        declarations: &declarations,
        tuples: &tuples,
    };
    let mut messages = Vec::new();
    let mut refused = Vec::new();
    for source in &sources {
        let (message, failures) = walk.message(*source);
        messages.push(message);
        refused.push(failures);
    }
    // References still use the original indices until propagation is complete.
    loop {
        let mut removals = Vec::new();
        for (index, message) in messages.iter().enumerate() {
            let Some(message) = message else { continue };
            let dependency = message
                .fields
                .iter()
                .filter_map(|field| field.wire.as_ref())
                .flat_map(local_references)
                .find(|other| messages[*other as usize].is_none());
            if let Some(other) = dependency {
                let failure = refused[other as usize].first().map(|first| Failure {
                    cause: Cause::RefusedDependency,
                    member: None,
                    message: format!(
                        "`{}` needs the refused message `{}`: {}",
                        message.name,
                        walk.message_name(sources[other as usize]),
                        first.message
                    ),
                });
                removals.push((index, failure));
            }
        }
        if removals.is_empty() {
            break;
        }
        for (index, failure) in removals {
            messages[index] = None;
            if let Some(failure) = failure {
                refused[index].push(failure);
            }
        }
    }
    let mut next = 0;
    let remap: Vec<_> = messages
        .iter()
        .map(|message| {
            message.as_ref().map(|_| {
                let index = next;
                next += 1;
                index
            })
        })
        .collect();
    let messages = messages
        .into_iter()
        .flatten()
        .map(|mut message| {
            for field in &mut message.fields {
                if let Some(wire) = &mut field.wire {
                    remap_references(wire, &remap);
                }
            }
            message
        })
        .collect();
    let refused = sources
        .into_iter()
        .zip(refused)
        .flat_map(|(source, failures)| {
            let source = match source {
                Source::StructDeclaration(index)
                | Source::UnionDeclaration(index)
                | Source::RootBox(index) => v1::proto3_refused::Source::Declaration(index),
                Source::Tuple(index) => v1::proto3_refused::Source::Tuple(index),
            };
            failures.into_iter().map(move |failure| v1::Proto3Refused {
                source: Some(source),
                cause: failure.cause as i32,
                member: failure.member,
                message: failure.message,
            })
        })
        .collect();
    v1::Proto3Projection { messages, refused }
}

fn local_references(wire: &v1::Proto3Wire) -> Vec<u32> {
    use v1::proto3_wire::Kind;
    match wire.kind.as_ref() {
        Some(Kind::Message(reference)) => match reference.target {
            Some(v1::proto3_message_ref::Target::Message(index)) => vec![index],
            _ => Vec::new(),
        },
        Some(Kind::Array(array)) => array
            .element
            .as_deref()
            .map(local_references)
            .unwrap_or_default(),
        Some(Kind::Map(map)) => map
            .key
            .iter()
            .chain(map.value.iter())
            .flat_map(|wire| local_references(wire))
            .collect(),
        _ => Vec::new(),
    }
}

fn remap_references(wire: &mut v1::Proto3Wire, remap: &[Option<u32>]) {
    use v1::proto3_wire::Kind;
    match wire.kind.as_mut() {
        Some(Kind::Message(reference)) => {
            if let Some(v1::proto3_message_ref::Target::Message(index)) = &mut reference.target {
                *index =
                    remap[*index as usize].expect("a remaining message has no refused dependency");
            }
        }
        Some(Kind::Array(array)) => {
            if let Some(element) = array.element.as_deref_mut() {
                remap_references(element, remap);
            }
        }
        Some(Kind::Map(map)) => {
            for wire in map.key.iter_mut().chain(map.value.iter_mut()) {
                remap_references(wire, remap);
            }
        }
        _ => {}
    }
}

#[derive(Debug)]
struct Failure {
    cause: Cause,
    member: Option<String>,
    message: String,
}

impl Failure {
    fn new(cause: Cause, message: impl Into<String>) -> Self {
        Self {
            cause,
            member: None,
            message: message.into(),
        }
    }

    fn other(message: impl Into<String>) -> Self {
        Self::new(Cause::Other, message)
    }

    fn member(mut self, member: String) -> Self {
        self.member = Some(member);
        self
    }
}

struct Failures(Vec<Failure>);

impl From<Failure> for Failures {
    fn from(failure: Failure) -> Self {
        Self(vec![failure])
    }
}

type WireResult = Result<v1::Proto3Wire, Failures>;

struct Walk<'a> {
    model: &'a v1::Model,
    declarations: &'a [Option<u32>],
    tuples: &'a [Option<u32>],
}

impl Walk<'_> {
    fn message_name(&self, source: v1::proto3_message::Source) -> String {
        use v1::proto3_message::Source;
        match source {
            Source::Tuple(index) => {
                let name = self.model.tuples[index as usize].name.as_ref();
                name.map(|n| if n.wire.is_empty() { &n.rust } else { &n.wire })
                    .cloned()
                    .unwrap_or_default()
            }
            Source::RootBox(index) => format!(
                "{}Box",
                declared(self.model.declarations[index as usize].name.as_ref())
            ),
            Source::StructDeclaration(index) | Source::UnionDeclaration(index) => {
                declared(self.model.declarations[index as usize].name.as_ref()).to_string()
            }
        }
    }

    fn message(
        &self,
        source: v1::proto3_message::Source,
    ) -> (Option<v1::Proto3Message>, Vec<Failure>) {
        use v1::proto3_field::Source as FieldSource;
        use v1::proto3_message::Source;

        let mut fields = Vec::new();
        let mut failures = Vec::new();
        let mut record = |result: Result<_, Failures>| match result {
            Ok(field) => fields.push(field),
            Err(found) => failures.extend(found.0),
        };
        match source {
            Source::Tuple(index) => {
                let tuple = &self.model.tuples[index as usize];
                let owner = tuple
                    .name
                    .as_ref()
                    .map(|name| name.rust.as_str())
                    .unwrap_or_default();
                // A tuple lifted from a declaration of another package has no
                // name in this package and has neither a message nor a refusal
                // here. The declaring package's projection lists that tuple.
                // Local users refer to the declaration through `Model.foreign`.
                if owner.is_empty() {
                    return (None, failures);
                }
                for (position, field) in tuple.fields.iter().enumerate() {
                    let result = (|| {
                        let name = declared(field.name.as_ref());
                        let ty = field
                            .r#type
                            .as_ref()
                            .ok_or_else(|| missing_type(owner, name, "carries no type"))?;
                        let number = u32::try_from(position + 1).map_err(|_| {
                            Failure::other(format!(
                                "`{owner}` has more tuple fields than a proto3 field number holds"
                            ))
                        })?;
                        self.field(
                            owner,
                            name,
                            true,
                            number,
                            ty,
                            FieldSource::TuplePosition(position as u32),
                        )
                    })();
                    record(result);
                }
            }
            Source::StructDeclaration(index)
            | Source::UnionDeclaration(index)
            | Source::RootBox(index) => {
                let declaration = &self.model.declarations[index as usize];
                let owner = declared(declaration.name.as_ref());
                match declaration.kind.as_ref() {
                    Some(v1::declaration::Kind::Struct(def)) => {
                        for slot in &def.slots {
                            let Some(v1::slot::Occupant::Field(field)) = slot.occupant.as_ref()
                            else {
                                continue;
                            };
                            let result = (|| {
                                let name = declared(field.name.as_ref());
                                let ty = field
                                    .r#type
                                    .as_ref()
                                    .ok_or_else(|| missing_type(owner, name, "carries no type"))?;
                                self.field(
                                    owner,
                                    name,
                                    false,
                                    slot.ordinal,
                                    ty,
                                    FieldSource::Field(name.to_string()),
                                )
                            })();
                            record(result);
                        }
                    }
                    Some(v1::declaration::Kind::Union(def)) => {
                        if def.arms.is_empty() {
                            return (
                                None,
                                vec![Failure::other(format!(
                                    "`{owner}` declares no arm, so it encodes no value"
                                ))],
                            );
                        }
                        for arm in &def.arms {
                            let result = (|| {
                                let name = declared(arm.name.as_ref());
                                let reference = arm
                                    .r#type
                                    .as_ref()
                                    .ok_or_else(|| missing_type(owner, name, "names no type"))?;
                                // The codec resolves an arm before checking its number.
                                self.resolve(reference)
                                    .ok_or_else(|| unresolved(reference))?;
                                let number = field_number(owner, name, arm.ordinal)?;
                                let wire = self
                                    .declaration_wire(reference)
                                    .map_err(|e| member_enum(e, owner, name))?;
                                Ok(v1::Proto3Field {
                                    number,
                                    wire: Some(wire),
                                    source: Some(FieldSource::Arm(name.to_string())),
                                })
                            })();
                            record(result);
                        }
                    }
                    _ => {
                        let reference = v1::TypeRef {
                            reference: owner.to_string(),
                            resolved: true,
                            package: self
                                .model
                                .scope
                                .as_ref()
                                .map(|s| s.package.clone())
                                .unwrap_or_default(),
                            foreign: false,
                            index,
                            kind: declaration_kind(declaration),
                        };
                        record(
                            self.declaration_wire(&reference)
                                .map(|wire| v1::Proto3Field {
                                    number: 1,
                                    wire: Some(wire),
                                    source: Some(FieldSource::BoxValue(true)),
                                }),
                        );
                    }
                }
            }
        }
        if !failures.is_empty() {
            return (None, failures);
        }
        fields.sort_by_key(|field| field.number);
        (
            Some(v1::Proto3Message {
                name: self.message_name(source),
                fields,
                source: Some(source),
            }),
            failures,
        )
    }

    fn field(
        &self,
        owner: &str,
        name: &str,
        tuple: bool,
        number: u32,
        ty: &v1::Type,
        source: v1::proto3_field::Source,
    ) -> Result<v1::Proto3Field, Failures> {
        refuse_optional_repeated(owner, name, tuple, ty)?;
        Ok(v1::Proto3Field {
            number: field_number(owner, name, number)?,
            wire: Some(self.wire(ty).map_err(|e| member_enum(e, owner, name))?),
            source: Some(source),
        })
    }

    fn wire(&self, ty: &v1::Type) -> WireResult {
        use v1::proto3_wire::Kind;

        let (kind, default_is_legal) = match ty.kind.as_ref() {
            Some(v1::r#type::Kind::Primitive(primitive)) => {
                match v1::PrimitiveType::try_from(*primitive).ok() {
                    Some(
                        v1::PrimitiveType::Boolean
                        | v1::PrimitiveType::Integer
                        | v1::PrimitiveType::Float,
                    ) => return Ok(scalar_wire(pb::primitive(*primitive), true)),
                    _ => {
                        return Err(Failure::other(
                            "a proto3 codec cannot carry an unbounded primitive",
                        )
                        .into());
                    }
                }
            }
            Some(v1::r#type::Kind::Inline(scalar)) => return project_scalar(scalar),
            Some(v1::r#type::Kind::Named(reference)) => return self.declaration_wire(reference),
            Some(v1::r#type::Kind::Tuple(reference)) => (
                Kind::Message(v1::Proto3MessageRef {
                    target: Some(v1::proto3_message_ref::Target::Message(
                        self.tuples
                            .get(reference.index as usize)
                            .copied()
                            .flatten()
                            .ok_or_else(|| {
                                Failure::other("the lowered model names a tuple it does not carry")
                            })?,
                    )),
                }),
                true,
            ),
            Some(v1::r#type::Kind::Array(array)) => {
                let element = array
                    .element
                    .as_deref()
                    .ok_or_else(|| Failure::other("an array carries no element type"))?;
                refuse_optional(element, "an array element")?;
                let element = self.wire(element)?;
                if repeated(&element) {
                    return Err(Failure::new(
                        Cause::NestedCollection,
                        "an array of arrays or of maps is one proto3 does not admit, \
                         as the proto backend refuses it",
                    )
                    .into());
                }
                let packed = matches!(element.kind, Some(Kind::Scalar(_) | Kind::Enumeration(_)));
                (
                    Kind::Array(
                        v1::Proto3Array {
                            element: Some(element.into()),
                            packed,
                        }
                        .into(),
                    ),
                    array.min == 0,
                )
            }
            Some(v1::r#type::Kind::Map(map)) => {
                let key = map
                    .key
                    .as_deref()
                    .ok_or_else(|| Failure::other("a map carries no key type"))?;
                let value = map
                    .value
                    .as_deref()
                    .ok_or_else(|| Failure::other("a map carries no value type"))?;
                refuse_optional(key, "a map key")?;
                refuse_optional(value, "a map value")?;
                let key_wire = self.wire(key)?;
                let admitted = match key_wire.kind.as_ref() {
                    Some(Kind::Text(_) | Kind::Enumeration(_)) => true,
                    Some(Kind::Scalar(_)) => self
                        .map_key_scalar(key)
                        .is_some_and(pb::Scalar::admitted_as_map_key),
                    _ => false,
                };
                if !admitted {
                    return Err(Failure::new(
                        Cause::MapKey,
                        "a map key must be an integral or string type, as the proto \
                         backend requires",
                    )
                    .into());
                }
                let value = self.wire(value)?;
                if repeated(&value) {
                    return Err(Failure::new(
                        Cause::NestedCollection,
                        "a map whose value is an array or a map is one proto3 does \
                         not admit, as the proto backend refuses it",
                    )
                    .into());
                }
                (
                    Kind::Map(
                        v1::Proto3Map {
                            key: Some(key_wire.into()),
                            value: Some(value.into()),
                        }
                        .into(),
                    ),
                    map.min == 0,
                )
            }
            _ => {
                return Err(
                    Failure::other("a proto3 codec cannot carry this type position").into(),
                );
            }
        };
        Ok(v1::Proto3Wire {
            default_is_legal,
            kind: Some(kind),
        })
    }

    fn resolve(&self, reference: &v1::TypeRef) -> Option<(&v1::Declaration, String)> {
        if reference.package.is_empty() {
            return None;
        }
        if self
            .model
            .scope
            .as_ref()
            .is_some_and(|scope| scope.package == reference.package)
        {
            let declaration = self.model.declarations.get(reference.index as usize)?;
            Some((declaration, declared(declaration.name.as_ref()).to_string()))
        } else {
            Some((
                self.model
                    .foreign
                    .get(reference.index as usize)?
                    .declaration
                    .as_ref()?,
                reference.reference.clone(),
            ))
        }
    }

    fn declaration_wire(&self, reference: &v1::TypeRef) -> WireResult {
        use v1::proto3_wire::Kind;

        let (declaration, owner) = self
            .resolve(reference)
            .ok_or_else(|| unresolved(reference))?;
        let name = declared(declaration.name.as_ref());
        let (kind, default_is_legal) = match declaration.kind.as_ref() {
            Some(v1::declaration::Kind::Scalar(scalar)) => return project_scalar(scalar),
            Some(v1::declaration::Kind::Enum(def)) => {
                if !def.open && def.values.is_empty() {
                    return Err(Failure::other(format!(
                        "`{name}` declares no value, so no proto3 codec can decode one"
                    ))
                    .into());
                }
                let failures: Vec<_> = def
                    .values
                    .iter()
                    .map(|value| (declared(value.name.as_ref()), value.value))
                    .chain(def.retired.iter().filter_map(|retired| {
                        retired
                            .value
                            .map(|value| (declared(retired.name.as_ref()), value))
                    }))
                    .filter(|(_, value)| i32::try_from(*value).is_err())
                    .map(|(member, value)| {
                        let full = if owner.contains('.') {
                            owner.clone()
                        } else {
                            format!(
                                "{}.{owner}",
                                self.model
                                    .scope
                                    .as_ref()
                                    .map(|s| s.package.as_str())
                                    .unwrap_or_default()
                            )
                        };
                        let failure = Failure::new(
                            Cause::EnumValue,
                            format!(
                                "`{full}` has no proto3 codec: its value `{member}` is {value}, \
                             outside int32, which the proto backend refuses"
                            ),
                        );
                        if member.is_empty() {
                            failure
                        } else {
                            failure.member(format!("{name}.{member}"))
                        }
                    })
                    .collect();
                if !failures.is_empty() {
                    return Err(Failures(failures));
                }
                let default_is_legal = def.open
                    || def
                        .zero_member
                        .and_then(|index| def.values.get(index as usize))
                        .is_some();
                (
                    Kind::Enumeration(v1::Proto3Enum {
                        r#type: Some(reference.clone()),
                    }),
                    default_is_legal,
                )
            }
            Some(v1::declaration::Kind::EnumSet(def)) => {
                let scalar = v1::IntWidth::try_from(def.width)
                    .ok()
                    .and_then(pb::int_width_scalar)
                    .ok_or_else(|| Failure::other(format!("`{name}` carries no integer width")))?;
                return Ok(scalar_wire(scalar, true));
            }
            Some(v1::declaration::Kind::Struct(_) | v1::declaration::Kind::Union(_)) => {
                let target = if self
                    .model
                    .scope
                    .as_ref()
                    .is_some_and(|scope| scope.package == reference.package)
                {
                    v1::proto3_message_ref::Target::Message(
                        self.declarations[reference.index as usize]
                            .expect("a struct or union has a message index"),
                    )
                } else {
                    v1::proto3_message_ref::Target::Foreign(reference.index)
                };
                (
                    Kind::Message(v1::Proto3MessageRef {
                        target: Some(target),
                    }),
                    true,
                )
            }
            _ => {
                return Err(Failure::other(format!(
                    "`{}` names a declaration a proto3 codec cannot carry",
                    reference.reference
                ))
                .into());
            }
        };
        Ok(v1::Proto3Wire {
            default_is_legal,
            kind: Some(kind),
        })
    }

    fn map_key_scalar(&self, ty: &v1::Type) -> Option<pb::Scalar> {
        match ty.kind.as_ref()? {
            v1::r#type::Kind::Primitive(primitive) => Some(pb::primitive(*primitive)),
            v1::r#type::Kind::Inline(scalar) => scalar_type(scalar).ok(),
            v1::r#type::Kind::Named(reference) => match self.resolve(reference)?.0.kind.as_ref()? {
                v1::declaration::Kind::Scalar(scalar) => scalar_type(scalar).ok(),
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

fn declared(name: Option<&v1::Spellings>) -> &str {
    name.map(|name| name.declared.as_str()).unwrap_or_default()
}

fn unresolved(reference: &v1::TypeRef) -> Failure {
    let text = &reference.reference;
    Failure::other(format!(
        "`{text}` resolves in no package of this build, so no proto3 codec \
         can be emitted for it"
    ))
}

fn missing_type(owner: &str, name: &str, text: &str) -> Failure {
    Failure::other(format!("`{owner}.{name}` {text}")).member(format!("{owner}.{name}"))
}

fn member_enum(mut failures: Failures, owner: &str, name: &str) -> Failures {
    for failure in &mut failures.0 {
        if failure.cause == Cause::EnumValue {
            failure.member = Some(format!("{owner}.{name}"));
        }
    }
    failures
}

fn repeated(wire: &v1::Proto3Wire) -> bool {
    matches!(
        wire.kind,
        Some(v1::proto3_wire::Kind::Array(_) | v1::proto3_wire::Kind::Map(_))
    )
}

fn field_number(owner: &str, name: &str, number: u32) -> Result<u32, Failure> {
    if number == 0 || pb::PROTO_RESERVED.contains(&number) || number > pb::PROTO_MAX_FIELD_NUMBER {
        return Err(Failure::new(
            Cause::FieldNumber,
            format!(
                "`{owner}.{name}` has field number {number}, which proto3 reserves or does not \
                 admit, as the proto backend refuses it"
            ),
        )
        .member(format!("{owner}.{name}")));
    }
    Ok(number)
}

fn refuse_optional_repeated(
    owner: &str,
    name: &str,
    tuple: bool,
    ty: &v1::Type,
) -> Result<(), Failure> {
    let (kind, proto_field) = match ty.kind.as_ref() {
        Some(v1::r#type::Kind::Array(_)) if ty.optional => ("array", "repeated"),
        Some(v1::r#type::Kind::Map(_)) if ty.optional => ("map", "map"),
        _ => return Ok(()),
    };
    let advice = if tuple {
        format!(
            "Remove the `?` from the tuple field, or declare a struct that holds the {kind} \
             and give the tuple field that struct's type, whose absence proto3 can track."
        )
    } else {
        format!("Remove the `?`, or wrap the {kind} in a struct, whose absence proto3 can track.")
    };
    Err(Failure::new(
        Cause::OptionalCollection,
        format!(
            "`{owner}.{name}` is an optional {kind}, and proto3 cannot mark a {proto_field} \
             field absent: an absent {kind} is indistinguishable from an empty one, and no \
             value is spare to carry the fact in-band, so this backend refuses rather than \
             choosing silently (ADR-0013 decision 7). {advice}"
        ),
    )
    .member(format!("{owner}.{name}")))
}

fn refuse_optional(ty: &v1::Type, what: &str) -> Result<(), Failure> {
    if ty.optional {
        return Err(Failure::new(
            Cause::OptionalElement,
            format!(
                "{what} is optional, which proto3 cannot represent — only a field may be absent"
            ),
        ));
    }
    Ok(())
}

fn scalar_type(scalar: &v1::Scalar) -> Result<pb::Scalar, Failure> {
    match scalar.width {
        Some(v1::scalar::Width::IntWidth(width)) => v1::IntWidth::try_from(width)
            .ok()
            .and_then(pb::int_width_scalar)
            .ok_or_else(|| Failure::other("a scalar carries no integer width")),
        Some(v1::scalar::Width::FloatWidth(width)) => v1::FloatWidth::try_from(width)
            .ok()
            .and_then(pb::float_width_scalar)
            .ok_or_else(|| Failure::other("a scalar carries no float width")),
        None => match v1::ScalarClass::try_from(scalar.class).ok() {
            Some(v1::ScalarClass::Boolean) => Ok(pb::Scalar::Bool),
            Some(v1::ScalarClass::String) => Ok(pb::Scalar::String),
            Some(v1::ScalarClass::Bytes) => Ok(pb::Scalar::Bytes),
            _ => Err(Failure::other("a numeric scalar carries no width")),
        },
    }
}

fn project_scalar(scalar: &v1::Scalar) -> WireResult {
    let ty = scalar_type(scalar)?;
    let default_is_legal = match ty {
        pb::Scalar::String | pb::Scalar::Bytes => {
            scalar
                .constraint
                .as_ref()
                .and_then(|c| c.len_min)
                .unwrap_or(0)
                == 0
        }
        _ => scalar.constraint.as_ref().is_none_or(|constraint| {
            crate::zero::range_holds_zero(
                constraint.min.as_deref(),
                constraint.max.as_deref(),
                constraint.step.as_deref(),
            )
        }),
    };
    Ok(scalar_wire(ty, default_is_legal))
}

fn scalar_wire(scalar: pb::Scalar, default_is_legal: bool) -> v1::Proto3Wire {
    use v1::proto3_wire::Kind;

    let ty = match scalar {
        pb::Scalar::String => {
            return v1::Proto3Wire {
                default_is_legal,
                kind: Some(Kind::Text(true)),
            };
        }
        pb::Scalar::Bytes => {
            return v1::Proto3Wire {
                default_is_legal,
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
        default_is_legal,
        kind: Some(Kind::Scalar(v1::Proto3Scalar {
            r#type: ty as i32,
            wire_type: wire_type as i32,
        })),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tuple_with_no_wire_name_uses_its_rust_name() {
        let model = v1::Model {
            tuples: vec![v1::InducedTuple {
                name: Some(v1::InducedName {
                    rust: "Pair".into(),
                    wire: String::new(),
                }),
                fields: vec![v1::Field {
                    name: Some(v1::Spellings {
                        declared: "value".into(),
                        ..Default::default()
                    }),
                    r#type: Some(v1::Type {
                        kind: Some(v1::r#type::Kind::Primitive(
                            v1::PrimitiveType::Boolean as i32,
                        )),
                        ..Default::default()
                    }),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        };
        let p = project(&model);
        assert_eq!(p.messages.len(), 1);
        assert_eq!(p.messages[0].name, "Pair");
        assert_eq!(
            p.messages[0].source,
            Some(v1::proto3_message::Source::Tuple(0))
        );
        assert!(p.refused.is_empty());
    }

    #[test]
    fn a_union_arm_without_a_type_is_refused_at_that_arm() {
        // Raw IR always lowers an arm to Some(TypeRef), even if unresolved.
        let model = v1::Model {
            declarations: vec![v1::Declaration {
                name: Some(v1::Spellings {
                    declared: "Bad".into(),
                    ..Default::default()
                }),
                kind: Some(v1::declaration::Kind::Union(v1::Union {
                    arms: vec![v1::Arm {
                        name: Some(v1::Spellings {
                            declared: "value".into(),
                            ..Default::default()
                        }),
                        ordinal: 1,
                        r#type: None,
                        ..Default::default()
                    }],
                    ..Default::default()
                })),
                ..Default::default()
            }],
            ..Default::default()
        };
        let p = project(&model);
        assert!(p.messages.is_empty());
        assert_eq!(p.refused.len(), 1);
        assert_eq!(
            p.refused[0].source,
            Some(v1::proto3_refused::Source::Declaration(0))
        );
        assert_eq!(p.refused[0].cause, Cause::Other as i32);
        assert_eq!(p.refused[0].member.as_deref(), Some("Bad.value"));
        assert_eq!(p.refused[0].message, "`Bad.value` names no type");
    }

    fn name(value: &str) -> Option<v1::Spellings> {
        Some(v1::Spellings {
            declared: value.into(),
            ..Default::default()
        })
    }

    #[test]
    fn a_tuple_field_without_a_type_names_the_member() {
        let model = v1::Model {
            tuples: vec![v1::InducedTuple {
                name: Some(v1::InducedName {
                    rust: "Pair".into(),
                    wire: "Pair".into(),
                }),
                fields: vec![v1::Field {
                    name: name("value"),
                    r#type: None,
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        };
        let p = project(&model);
        assert!(p.messages.is_empty());
        assert_eq!(p.refused.len(), 1);
        assert_eq!(
            p.refused[0].source,
            Some(v1::proto3_refused::Source::Tuple(0))
        );
        assert_eq!(p.refused[0].cause, Cause::Other as i32);
        assert_eq!(p.refused[0].member.as_deref(), Some("Pair.value"));
        assert_eq!(p.refused[0].message, "`Pair.value` carries no type");
    }

    #[test]
    fn foreign_message_references_index_the_foreign_declarations() {
        let foreign = ["First", "Second"].map(|value| v1::ForeignDeclaration {
            package: "p.other".into(),
            declaration: Some(v1::Declaration {
                name: name(value),
                kind: Some(v1::declaration::Kind::Struct(v1::Struct::default())),
                ..Default::default()
            }),
        });
        let slots = [("second", "Second", 1), ("first", "First", 0)]
            .into_iter()
            .enumerate()
            .map(|(ordinal, (member, target, index))| v1::Slot {
                ordinal: ordinal as u32 + 1,
                occupant: Some(v1::slot::Occupant::Field(Box::new(v1::Field {
                    name: name(member),
                    r#type: Some(v1::Type {
                        optional: false,
                        kind: Some(v1::r#type::Kind::Named(v1::TypeRef {
                            reference: format!("p.other.{target}"),
                            package: "p.other".into(),
                            resolved: true,
                            foreign: true,
                            index,
                            kind: v1::DeclKind::Struct as i32,
                        })),
                    }),
                    ..Default::default()
                }))),
            })
            .collect();
        let model = v1::Model {
            scope: Some(v1::Scope {
                package: "p.local".into(),
                ..Default::default()
            }),
            declarations: vec![v1::Declaration {
                name: name("Holder"),
                kind: Some(v1::declaration::Kind::Struct(v1::Struct {
                    slots,
                    ..Default::default()
                })),
                ..Default::default()
            }],
            foreign: foreign.into(),
            ..Default::default()
        };
        let p = project(&model);
        assert!(p.refused.is_empty());
        assert_eq!(p.messages.len(), 1);
        for (field, expected) in p.messages[0].fields.iter().zip([1, 0]) {
            let Some(v1::proto3_wire::Kind::Message(reference)) =
                field.wire.as_ref().and_then(|wire| wire.kind.as_ref())
            else {
                panic!("a foreign message reference");
            };
            assert_eq!(
                reference.target,
                Some(v1::proto3_message_ref::Target::Foreign(expected))
            );
        }
    }
}
