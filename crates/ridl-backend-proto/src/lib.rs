//! IR v2 package to a proto3 schema (roadmap story E9.8, ADR-0013 decision 2).
//!
//! A **wire backend** in the sense ADR-0013 decision 1 gives the term: the
//! target describes bytes in transit, so the emit ceiling is two tiers — the
//! typl surface, and the interaction identity table — and nothing above them.
//! No `service` block, no call face, no value store.
//!
//! Text is written directly rather than through a `FileDescriptorProto`,
//! matching `c_header.rs`. The constraint information typl carries and proto3
//! cannot represent — units, ranges, steps — is emitted as comments, which a
//! descriptor would have addressed by index path through `SourceCodeInfo`.

use ridl_ir::v2;
use std::collections::HashMap;

#[cfg(test)]
mod tests;

/// The generated proto3 schema for one package.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Generated {
    pub proto_source: String,
}

/// A failure to generate a schema from a package.
///
/// Carried as a value so codegen stays total: no stage in the pipeline panics
/// (ADR-0004 section 5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenerateError {
    pub message: String,
}

/// Generates the proto3 schema for `package`.
pub fn generate(package: &v2::Package) -> Result<Generated, GenerateError> {
    let mut out = String::new();
    out.push_str("syntax = \"proto3\";\n\n");
    out.push_str(&format!("package {};\n", package.name));
    emit_messages(&mut out, package)?;
    emit_identity_tables(&mut out, package)?;
    Ok(Generated { proto_source: out })
}

/// proto reserves field numbers 19,000 through 19,999 for its own use.
const PROTO_RESERVED: std::ops::RangeInclusive<u32> = 19_000..=19_999;
/// The largest field number proto admits.
const PROTO_MAX_FIELD_NUMBER: u32 = 536_870_911;

/// A dotted address becomes one CamelCase proto identifier:
/// `corpus.baseline.hvac` gives `CorpusBaselineHvac`. A named interface has no
/// dots, so its name passes through unchanged.
fn type_name(dotted: &str) -> String {
    dotted
        .split('.')
        .filter(|segment| !segment.is_empty())
        .map(|segment| {
            let mut chars = segment.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect()
}

/// SCREAMING_SNAKE of a ridl name, built on the pinned transform so there is
/// exactly one case algorithm in the toolchain (ADR-0016 decision 2).
fn screaming_snake_case(name: &str) -> String {
    ridl_ir::name::snake_case(name).to_uppercase()
}

/// Rejects an ordinal proto cannot carry, making ADR-0016 decision 6's
/// totality property true as stated rather than true by luck. Neither case is
/// reachable in practice: it would take one interface accumulating nineteen
/// thousand interactions and tombstones (note §4.2).
fn check_field_number(owner: &str, name: &str, number: u32) -> Result<(), GenerateError> {
    if PROTO_RESERVED.contains(&number) {
        return Err(GenerateError {
            message: format!(
                "{owner}.{name} takes field number {number}, which is reserved by protobuf \
                 itself (19000 to 19999). Renumber the declaration."
            ),
        });
    }
    if number > PROTO_MAX_FIELD_NUMBER {
        return Err(GenerateError {
            message: format!(
                "{owner}.{name} takes field number {number}, above proto's largest field \
                 number {PROTO_MAX_FIELD_NUMBER}."
            ),
        });
    }
    Ok(())
}

/// Tier 1 (ADR-0013 decision 2): the typl surface. A struct becomes a
/// `message`; a named scalar becomes no declaration of its own — it inlines
/// to its backing scalar at each use site (design §3.1). An enum becomes a
/// proto3 `enum` ([`emit_enum`]). A union becomes a `message` wrapping a
/// `oneof` ([`emit_union`]). A constant is not emitted (ADR-0013 decision 5).
/// An enum set becomes no declaration of its own either — like a named
/// scalar, it resolves at each use site ([`named_field_type`]). An array
/// becomes `repeated`, and a map becomes `map<K, V>` (both [`field_type_text`]
/// through [`resolve_field_type`]). A tuple has no proto3 equivalent, so it
/// induces a message of its own, collected into `tuples` during the walk and
/// emitted by [`emit_induced_tuples`] after the declarations that reached it.
fn emit_messages(out: &mut String, package: &v2::Package) -> Result<(), GenerateError> {
    let mut tuples: Vec<InducedTuple> = Vec::new();
    for decl in &package.decls {
        match &decl.kind {
            Some(v2::decl::Kind::StructDef(def)) => {
                emit_struct(out, package, &decl.name, def, &mut tuples)?;
            }
            Some(v2::decl::Kind::EnumDef(def)) => emit_enum(out, &decl.name, def)?,
            Some(v2::decl::Kind::UnionDef(def)) => emit_union(out, package, &decl.name, def)?,
            _ => {}
        }
    }
    emit_induced_tuples(out, package, &mut tuples)?;
    Ok(())
}

/// One enum as one proto3 `enum` (design §3.1, §3.4): values keep their
/// explicitly assigned numbers, and a `Reserved` retires its number rather
/// than let it be reused (typl §7.4). Two proto3 rules force choices no typl
/// surface states. proto3 scopes an enum's values as siblings of the enum
/// itself, so two typl enums in one package that each declare a value named
/// `OK` would emit a redefinition `protoc` rejects — every emitted value is
/// therefore prefixed with `screaming_snake_case` of its own enum's name.
/// proto3 also requires the first value in the emitted enum to be zero.
/// typl §8 assigns every value explicitly and does not require the
/// zero-valued member to be declared first, so a declared zero is moved to
/// lead regardless of its source position (`protoc` rejects a zero declared
/// out of order the same as a missing one). When no declared value is zero —
/// including when the zero slot is retired with `reserved 0` rather than
/// given a live member — `<PREFIX>_UNSPECIFIED = 0` is synthesized to fill
/// it; a retired zero slot is then a live value, not a reservation, so the
/// matching `reserved 0;` is dropped instead of also being emitted, which
/// `protoc` would reject as the same number claimed twice. A value outside
/// `i32` is refused: proto3 enum and reserved values are int32, and typl
/// admits int64 (typl §8).
fn emit_enum(out: &mut String, name: &str, def: &v2::EnumDef) -> Result<(), GenerateError> {
    let prefix = screaming_snake_case(name);
    out.push_str(&format!("\nenum {name} {{\n"));

    let zero_declared = def.values.iter().any(|value| value.value == 0);
    if !zero_declared {
        out.push_str(&format!("  {prefix}_UNSPECIFIED = 0;\n"));
    }

    // A declared zero leads, whatever position it held in typl source order.
    let (zero_values, other_values): (Vec<_>, Vec<_>) =
        def.values.iter().partition(|value| value.value == 0);
    for value in zero_values.into_iter().chain(other_values) {
        let number = i32::try_from(value.value).map_err(|_| GenerateError {
            message: format!(
                "{name}.{} takes value {}, outside proto3's int32 range for an enum value.",
                value.name, value.value
            ),
        })?;
        out.push_str(&format!(
            "  {prefix}_{} = {number};\n",
            screaming_snake_case(&value.name)
        ));
    }

    for reserved in &def.reserved {
        let retired = reserved.value.unwrap_or(0);
        // The synthesized `UNSPECIFIED = 0` above already fills a retired
        // zero slot as a live value; reserving it too is the same number
        // claimed twice, which `protoc` rejects.
        if retired == 0 && !zero_declared {
            continue;
        }
        let retired = i32::try_from(retired).map_err(|_| GenerateError {
            message: format!(
                "{name} retires value {retired}, outside proto3's int32 range for an enum \
                 value."
            ),
        })?;
        out.push_str(&format!("  reserved {retired};\n"));
    }
    out.push_str("}\n");
    Ok(())
}

/// One struct as one `message`: field numbers are the typl §7.4 ordinals, a
/// tombstone emits proto `reserved`, and a `?` field takes the proto3
/// `optional` keyword, because proto3 represents absence structurally
/// (ADR-0013 decision 7). Field names go through the pinned transform —
/// proto3's field namespace is snake_case, and RIDL-149 has already refused
/// any package where two field names collide under it (ADR-0016 decision 4).
fn emit_struct(
    out: &mut String,
    package: &v2::Package,
    name: &str,
    def: &v2::StructDef,
    tuples: &mut Vec<InducedTuple>,
) -> Result<(), GenerateError> {
    out.push_str(&format!("\nmessage {name} {{\n"));
    for member in &def.members {
        match &member.member {
            Some(v2::struct_member::Member::Reserved(reserved)) => {
                out.push_str(&format!("  reserved {};\n", reserved.ordinal));
            }
            Some(v2::struct_member::Member::Field(field)) => {
                check_field_number(name, &field.name, field.ordinal)?;
                let (scalar, comment) = field_type_text(package, name, field, tuples)?;
                if let Some(comment) = comment {
                    out.push_str(&format!("  {comment}\n"));
                }
                let optional = match &field.r#type {
                    Some(ty) if ty.optional => "optional ",
                    _ => "",
                };
                out.push_str(&format!(
                    "  {optional}{scalar} {} = {};\n",
                    ridl_ir::name::snake_case(&field.name),
                    field.ordinal
                ));
            }
            None => {}
        }
    }
    out.push_str("}\n");
    Ok(())
}

/// One union as one `message` wrapping a `oneof` (design §3.1). An arm's
/// field number is its typl §7.4 ordinal — an identity mapping — and an
/// arm's name goes through the pinned transform, the same as a struct field
/// (ADR-0016 decisions 1 and 2). A retired arm's `reserved N;` is emitted at
/// the message level, outside the `oneof` body, because proto3 does not
/// admit a `reserved` statement inside one. An arm carries no constraint
/// comment: unlike a struct field, a `oneof` arm has no line of its own
/// before it to hold one.
fn emit_union(
    out: &mut String,
    package: &v2::Package,
    name: &str,
    def: &v2::UnionDef,
) -> Result<(), GenerateError> {
    out.push_str(&format!("\nmessage {name} {{\n  oneof value {{\n"));
    for arm in &def.arms {
        check_field_number(name, &arm.name, arm.ordinal)?;
        let (scalar, _comment) = named_field_type(package, name, &arm.name, &arm.type_ref)?;
        out.push_str(&format!(
            "    {scalar} {} = {};\n",
            ridl_ir::name::snake_case(&arm.name),
            arm.ordinal
        ));
    }
    out.push_str("  }\n");
    for reserved in &def.reserved {
        out.push_str(&format!("  reserved {};\n", reserved.ordinal));
    }
    out.push_str("}\n");
    Ok(())
}

/// The proto3 type of one field, with the comment a named scalar leaves at
/// its use site (design §3.2). `tuples` collects a tuple type reached along
/// the way, for [`emit_induced_tuples`] to emit as a message after the
/// declaration that reached it (proto3 has no tuple, design §3.1).
fn field_type_text(
    package: &v2::Package,
    owner: &str,
    field: &v2::Field,
    tuples: &mut Vec<InducedTuple>,
) -> Result<(String, Option<String>), GenerateError> {
    let Some(ty) = field.r#type.as_ref() else {
        return Err(GenerateError {
            message: format!("{owner}.{} carries no type in the IR.", field.name),
        });
    };
    let hint = format!("{owner}{}", type_name(&field.name));
    resolve_field_type(package, owner, &field.name, &hint, ty, tuples)
}

/// The proto3 type at one field position, recursing into an array element, a
/// map key and value, and — through [`field_type_text`] and
/// [`emit_induced_tuple`] — a tuple field, every position typl gives a
/// [`v2::FieldType`] of its own (typl §7, §11, §12).
///
/// `hint` is the CamelCase name a tuple reached at this exact position would
/// take as a generated message (design §3.1): `<OwnerType><FieldName>` at
/// the field itself, extended with `Element` for an array element and
/// `Value` for a map value — proto3 forbids a map key from being a message,
/// so a tuple never reaches the key position (TYPL-209 already requires an
/// integral or string key, and [`map_key_text`] refuses the rest).
fn resolve_field_type(
    package: &v2::Package,
    owner: &str,
    field_name: &str,
    hint: &str,
    ty: &v2::FieldType,
    tuples: &mut Vec<InducedTuple>,
) -> Result<(String, Option<String>), GenerateError> {
    match ty.kind.as_ref() {
        Some(v2::field_type::Kind::Primitive(primitive)) => {
            Ok((proto_primitive(*primitive).to_string(), None))
        }
        Some(v2::field_type::Kind::InlineScalar(td)) => Ok((proto_scalar(td).to_string(), None)),
        Some(v2::field_type::Kind::Named(reference)) => {
            named_field_type(package, owner, field_name, reference)
        }
        Some(v2::field_type::Kind::Array(array)) => {
            let element = array.element.as_ref().ok_or_else(|| GenerateError {
                message: format!(
                    "{owner}.{field_name} declares an array with no element type in the IR."
                ),
            })?;
            // proto3 has no nested `repeated` and no `repeated` map field —
            // checked here, before resolving the element, so neither
            // rejection reaches `protoc` as a schema it must itself refuse.
            match element.kind.as_ref() {
                Some(v2::field_type::Kind::Array(_)) => {
                    return Err(GenerateError {
                        message: format!(
                            "{owner}.{field_name} is an array of arrays, which proto3 does \
                             not admit as an array element type."
                        ),
                    });
                }
                Some(v2::field_type::Kind::Map(_)) => {
                    return Err(GenerateError {
                        message: format!(
                            "{owner}.{field_name} is an array of maps, which proto3 does \
                             not admit as an array element type."
                        ),
                    });
                }
                _ => {}
            }
            let (element_text, comment) = resolve_field_type(
                package,
                owner,
                field_name,
                &format!("{hint}Element"),
                element,
                tuples,
            )?;
            Ok((format!("repeated {element_text}"), comment))
        }
        Some(v2::field_type::Kind::Map(map)) => {
            let key = map.key.as_ref().ok_or_else(|| GenerateError {
                message: format!("{owner}.{field_name} declares a map with no key type in the IR."),
            })?;
            let value = map.value.as_ref().ok_or_else(|| GenerateError {
                message: format!(
                    "{owner}.{field_name} declares a map with no value type in the IR."
                ),
            })?;
            // proto3 forbids a map value from being `repeated` or another
            // map — checked here, before resolving the value, so neither
            // rejection reaches `protoc` as a schema it must itself refuse.
            match value.kind.as_ref() {
                Some(v2::field_type::Kind::Array(_)) => {
                    return Err(GenerateError {
                        message: format!(
                            "{owner}.{field_name} maps to a repeated value, which proto3 \
                             does not admit as a map value type."
                        ),
                    });
                }
                Some(v2::field_type::Kind::Map(_)) => {
                    return Err(GenerateError {
                        message: format!(
                            "{owner}.{field_name} maps to another map, which proto3 does \
                             not admit as a map value type."
                        ),
                    });
                }
                _ => {}
            }
            let key_text = map_key_text(package, owner, field_name, key)?;
            let (value_text, comment) = resolve_field_type(
                package,
                owner,
                field_name,
                &format!("{hint}Value"),
                value,
                tuples,
            )?;
            Ok((format!("map<{key_text}, {value_text}>"), comment))
        }
        Some(v2::field_type::Kind::Tuple(tuple)) => {
            tuples.push(InducedTuple {
                name: hint.to_string(),
                tuple: tuple.clone(),
            });
            Ok((hint.to_string(), None))
        }
        Some(_) => Err(GenerateError {
            message: format!(
                "{owner}.{field_name} has a stream type, which this backend does not \
                 project — tier 1 covers the typl surface only (ADR-0013 decision 2)."
            ),
        }),
        None => Err(GenerateError {
            message: format!("{owner}.{field_name} carries no type in the IR."),
        }),
    }
}

/// The proto3 scalars a map key may take: any integral or string type, never
/// a floating-point type, `bytes`, or a message/enum name (the proto3
/// language guide, "Maps"). typl admits a broader set at a map key position
/// (typl §12.2, TYPL-209 — any primitive, or a named string type), so a key
/// this backend resolves to a scalar outside this set is refused rather than
/// emitted as a `map<...>` `protoc` rejects.
const PROTO_MAP_KEY_SCALARS: [&str; 7] = [
    "bool", "int64", "uint32", "uint64", "sint32", "sint64", "string",
];

fn map_key_text(
    package: &v2::Package,
    owner: &str,
    field_name: &str,
    key: &v2::FieldType,
) -> Result<String, GenerateError> {
    let text = match key.kind.as_ref() {
        Some(v2::field_type::Kind::Primitive(primitive)) => proto_primitive(*primitive).to_string(),
        Some(v2::field_type::Kind::Named(reference)) => {
            named_field_type(package, owner, field_name, reference)?.0
        }
        _ => {
            return Err(GenerateError {
                message: format!(
                    "{owner}.{field_name} uses a map key type proto3 cannot carry — a map \
                     key must be an integral or string type."
                ),
            });
        }
    };
    if PROTO_MAP_KEY_SCALARS.contains(&text.as_str()) {
        Ok(text)
    } else {
        Err(GenerateError {
            message: format!(
                "{owner}.{field_name} uses `{text}` as a map key, which proto3 does not \
                 admit — a map key must be an integral or string type."
            ),
        })
    }
}

/// A tuple type reached while walking the package, to be emitted as a
/// message after the declaration that reached it — proto3 has no tuple
/// (design §3.1). Matches the pattern `ridl-backend-rust`'s `InducedTuple`
/// uses: a tuple is anonymous in source, so it is collected into a flat
/// worklist during the walk and drained afterwards ([`emit_induced_tuples`]),
/// which is also where a tuple nested inside another tuple, an array element
/// or a map value is found.
#[derive(Debug, Clone)]
struct InducedTuple {
    /// The generated message name: `<OwnerType><FieldName>` in CamelCase, or
    /// that name extended for a position reached through the field
    /// ([`resolve_field_type`]).
    name: String,
    tuple: v2::TupleType,
}

/// Emits every tuple type the walk in [`emit_messages`] reached, as a
/// message named for the path that reached it. The worklist is drained
/// rather than iterated once: emitting one induced message's fields can
/// discover a tuple nested inside it, which is appended to `tuples` and
/// picked up by a later pass of this same loop.
///
/// Two different tuples reaching the same generated name is refused rather
/// than resolved by picking one: nothing upstream keeps two field paths from
/// mangling to the same CamelCase string, and there is no sound way to
/// choose between two different message shapes for one name.
fn emit_induced_tuples(
    out: &mut String,
    package: &v2::Package,
    tuples: &mut Vec<InducedTuple>,
) -> Result<(), GenerateError> {
    let mut seen: HashMap<String, v2::TupleType> = HashMap::new();
    let mut index = 0;
    while index < tuples.len() {
        let induced = tuples[index].clone();
        index += 1;
        if let Some(previous) = seen.get(&induced.name) {
            if *previous != induced.tuple {
                return Err(GenerateError {
                    message: format!(
                        "the generated message name {} is claimed by two different tuple \
                         types; a tuple generates a message named for the path that reaches \
                         it, and two different paths spelled one name here — rename a field \
                         so they differ.",
                        induced.name
                    ),
                });
            }
            continue;
        }
        seen.insert(induced.name.clone(), induced.tuple.clone());
        emit_induced_tuple(out, package, &induced, tuples)?;
    }
    Ok(())
}

/// One induced tuple message: positional fields `field_1`, `field_2`, …
/// numbered from 1 (design §3.1). A tuple field is always named in typl
/// source (typl §11), but positional access is what a tuple actually offers,
/// so the generated message uses the position rather than carry the source
/// name onto the wire.
fn emit_induced_tuple(
    out: &mut String,
    package: &v2::Package,
    induced: &InducedTuple,
    tuples: &mut Vec<InducedTuple>,
) -> Result<(), GenerateError> {
    out.push_str(&format!("\nmessage {} {{\n", induced.name));
    for (index, field) in induced.tuple.fields.iter().enumerate() {
        let ordinal = u32::try_from(index + 1).map_err(|_| GenerateError {
            message: format!(
                "{} has more tuple fields than fit a proto field number.",
                induced.name
            ),
        })?;
        let field_name = format!("field_{ordinal}");
        check_field_number(&induced.name, &field_name, ordinal)?;
        let ty = field.r#type.as_ref().ok_or_else(|| GenerateError {
            message: format!("{}.{field_name} carries no type in the IR.", induced.name),
        })?;
        let hint = format!("{}Field{ordinal}", induced.name);
        let (scalar, comment) =
            resolve_field_type(package, &induced.name, &field_name, &hint, ty, tuples)?;
        if let Some(comment) = comment {
            out.push_str(&format!("  {comment}\n"));
        }
        out.push_str(&format!("  {scalar} {field_name} = {ordinal};\n"));
    }
    out.push_str("}\n");
    Ok(())
}

/// A resolved named-type reference at a field position. A named scalar
/// inlines to its backing scalar and leaves its name, unit, range and step
/// as a comment; a struct reference names the message it becomes; an enum
/// reference names the `enum` it becomes ([`emit_enum`]); a union reference
/// names the `message` it becomes ([`emit_union`]) — a union is legal
/// wherever data is legal (typl §10), the same as a struct or an enum.
fn named_field_type(
    package: &v2::Package,
    owner: &str,
    field_name: &str,
    reference: &str,
) -> Result<(String, Option<String>), GenerateError> {
    let Some(decl) = package.decls.iter().find(|decl| decl.name == *reference) else {
        return Err(GenerateError {
            message: format!(
                "{owner}.{field_name} references `{reference}`, which is not a declaration \
                 of this package (cross-package references land later in E9.8)."
            ),
        });
    };
    match &decl.kind {
        Some(v2::decl::Kind::TypeDef(td)) => Ok((
            proto_scalar(td).to_string(),
            Some(constraint_comment(&decl.name, td)),
        )),
        Some(v2::decl::Kind::StructDef(_)) => Ok((decl.name.clone(), None)),
        Some(v2::decl::Kind::EnumDef(_)) => Ok((decl.name.clone(), None)),
        Some(v2::decl::Kind::EnumSetDef(esd)) => Ok(enum_set_field_type(esd)),
        Some(v2::decl::Kind::UnionDef(_)) => Ok((decl.name.clone(), None)),
        _ => Err(GenerateError {
            message: format!(
                "{owner}.{field_name} references `{reference}`, a declaration kind that \
                 cannot be a field's type — only a named scalar, struct, enum, enum set or \
                 union may be referenced from a field position."
            ),
        }),
    }
}

/// The proto3 scalar and use-site comment for an enum set (design §3.3). A
/// proto enum field holds one value, and an enum set is a combination of
/// bits, so it gains no declaration of its own — like a named scalar, it
/// resolves at each use site instead. The scalar is the one `proto_scalar`
/// gives its declared width; the bit names and positions become one comment
/// line each, in the form `LOW_FUEL = bit 0`.
fn enum_set_field_type(esd: &v2::EnumSetDef) -> (String, Option<String>) {
    let scalar = proto_scalar(&v2::TypeDef {
        width: Some(v2::type_def::Width::IntWidth(esd.width)),
        ..Default::default()
    });
    let lines: Vec<String> = esd
        .bits
        .iter()
        .map(|bit| format!("// {} = bit {}", bit.name, bit.value))
        .collect();
    let comment = if lines.is_empty() {
        None
    } else {
        Some(lines.join("\n  "))
    };
    (scalar.to_string(), comment)
}

/// The proto3 scalar for a resolved typl width (typl Appendix D). proto3 has
/// no `uint8`/`uint16` — varint keeps small values small — so both widen to
/// `uint32`. A signed width means the declared range contains negatives, and
/// such a range takes `sint32`/`sint64`, because plain `int32` varint costs
/// 10 bytes for every negative value (ADR-0013 decision 4). A quantized
/// float keeps its native form: the scaled-integer encoding of typl §4.3
/// belongs to CAN/DBC and to SOME/IP per deployment, and a wire backend must
/// not apply it unasked.
fn proto_scalar(td: &v2::TypeDef) -> &'static str {
    match &td.width {
        Some(v2::type_def::Width::IntWidth(width)) => match v2::IntWidth::try_from(*width) {
            Ok(v2::IntWidth::U8 | v2::IntWidth::U16 | v2::IntWidth::U32) => "uint32",
            Ok(v2::IntWidth::U64) => "uint64",
            Ok(v2::IntWidth::I8 | v2::IntWidth::I16 | v2::IntWidth::I32) => "sint32",
            Ok(v2::IntWidth::I64) => "sint64",
            _ => "int64",
        },
        Some(v2::type_def::Width::FloatWidth(width)) => match v2::FloatWidth::try_from(*width) {
            Ok(v2::FloatWidth::F32) => "float",
            _ => "double",
        },
        // No width table: boolean, string and bytes backings. A unit backing
        // implies the float primitive (typl §5.1), so its width is always
        // derived and never reaches this arm.
        None => match td
            .backing
            .as_ref()
            .and_then(|backing| backing.kind.as_ref())
        {
            Some(v2::backing::Kind::Primitive(primitive)) => {
                match v2::PrimitiveType::try_from(*primitive) {
                    Ok(v2::PrimitiveType::Boolean) => "bool",
                    Ok(v2::PrimitiveType::Bytes) => "bytes",
                    _ => "string",
                }
            }
            _ => "string",
        },
    }
}

/// The proto3 scalar for a direct primitive use at a field position. A bare
/// `integer` or `float` carries no derived width, so the full typl domain is
/// emitted: int64 and float64 (typl §4).
fn proto_primitive(primitive: i32) -> &'static str {
    match v2::PrimitiveType::try_from(primitive) {
        Ok(v2::PrimitiveType::Boolean) => "bool",
        Ok(v2::PrimitiveType::Integer) => "int64",
        Ok(v2::PrimitiveType::Float) => "double",
        Ok(v2::PrimitiveType::Bytes) => "bytes",
        _ => "string",
    }
}

/// The constraint information proto3 has no construct for, as a comment
/// (design §3.2). This is the only home for it: the alternative — a
/// published options extension over `google.protobuf.FieldOptions` — was
/// rejected for v0.1 because it serves a consumer that does not exist, and
/// the IR is already the machine-readable contract.
fn constraint_comment(declared: &str, td: &v2::TypeDef) -> String {
    let mut form = Vec::new();
    if let Some(v2::backing::Kind::Unit(unit)) = td.backing.as_ref().and_then(|b| b.kind.as_ref()) {
        form.push(unit.clone());
    }
    if let Some(constraint) = &td.constraint {
        let rendered = render_constraint(constraint);
        if !rendered.is_empty() {
            form.push(rendered);
        }
    }
    if form.is_empty() {
        format!("// {declared}")
    } else {
        format!("// {declared} — {}", form.join(" "))
    }
}

/// The source form of a scalar constraint (typl §5.2–§5.5): the range
/// bracket with its optional step, the length bracket, and the `match`
/// pattern. The strings are canonical text already (ADR-0007 decision 9), so
/// they are joined, never reformatted.
fn render_constraint(constraint: &v2::Constraint) -> String {
    let mut parts = Vec::new();
    if constraint.min.is_some() || constraint.max.is_some() || constraint.step.is_some() {
        let min = constraint.min.as_deref().unwrap_or("");
        let max = constraint.max.as_deref().unwrap_or("");
        let step = match &constraint.step {
            Some(step) => format!(" step {step}"),
            None => String::new(),
        };
        parts.push(format!("[{min}..{max}{step}]"));
    }
    match (constraint.len_min, constraint.len_max) {
        // A fixed `[N]` constraint has len_min == len_max == N.
        (Some(min), Some(max)) if min == max => parts.push(format!("[{min}]")),
        (None, None) => {}
        (min, max) => parts.push(format!(
            "[{}..{}]",
            min.map(|bound| bound.to_string()).unwrap_or_default(),
            max.map(|bound| bound.to_string()).unwrap_or_default()
        )),
    }
    // A pattern given by a named regex constant renders by that name, the
    // way it was written (typl §5.3, §6.2).
    if let Some(name) = &constraint.pattern_const {
        parts.push(format!("match {name}"));
    } else if let Some(pattern) = &constraint.pattern {
        parts.push(format!("match {pattern}"));
    }
    parts.join(" ")
}

/// Tier 2 (ADR-0013 decision 3): one enum per interface shape, interface-wide
/// and kind-blind, matching ridl §11's single ordinal sequence. Retired
/// ordinals are held against reuse with `reserved`, and an `UNSPECIFIED = 0`
/// member leads because ridl ordinals are 1-based.
fn emit_identity_tables(out: &mut String, package: &v2::Package) -> Result<(), GenerateError> {
    for shape in package.shapes() {
        let enum_name = format!("{}Ordinal", type_name(shape.name));
        let prefix = screaming_snake_case(&enum_name);

        out.push_str(&format!("\nenum {enum_name} {{\n"));
        out.push_str(&format!("  {prefix}_UNSPECIFIED = 0;\n"));

        for decl in &shape.interface.interactions {
            match &decl.kind {
                Some(v2::decl::Kind::ReservedSlot(reserved)) => {
                    out.push_str(&format!("  reserved {};\n", reserved.ordinal));
                }
                _ => {
                    check_field_number(shape.name, &decl.name, decl.ordinal)?;
                    out.push_str(&format!(
                        "  {prefix}_{} = {};\n",
                        screaming_snake_case(&decl.name),
                        decl.ordinal
                    ));
                }
            }
        }
        out.push_str("}\n");
    }
    Ok(())
}
