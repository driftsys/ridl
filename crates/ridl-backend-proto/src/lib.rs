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
/// proto3 `enum` ([`emit_enum`]). A constant is not emitted (ADR-0013
/// decision 5). An enum set becomes no declaration of its own either — like a
/// named scalar, it resolves at each use site ([`named_field_type`]). Unions
/// join in a later commit of this story; until then a declaration of that
/// kind emits nothing here, and a field referencing one is refused by
/// [`named_field_type`] rather than emitted as a type `protoc` cannot
/// resolve.
fn emit_messages(out: &mut String, package: &v2::Package) -> Result<(), GenerateError> {
    for decl in &package.decls {
        match &decl.kind {
            Some(v2::decl::Kind::StructDef(def)) => emit_struct(out, package, &decl.name, def)?,
            Some(v2::decl::Kind::EnumDef(def)) => emit_enum(out, &decl.name, def)?,
            _ => {}
        }
    }
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
) -> Result<(), GenerateError> {
    out.push_str(&format!("\nmessage {name} {{\n"));
    for member in &def.members {
        match &member.member {
            Some(v2::struct_member::Member::Reserved(reserved)) => {
                out.push_str(&format!("  reserved {};\n", reserved.ordinal));
            }
            Some(v2::struct_member::Member::Field(field)) => {
                check_field_number(name, &field.name, field.ordinal)?;
                let (scalar, comment) = field_type_text(package, name, field)?;
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

/// The proto3 type of one field, with the comment a named scalar leaves at
/// its use site (design §3.2).
fn field_type_text(
    package: &v2::Package,
    owner: &str,
    field: &v2::Field,
) -> Result<(String, Option<String>), GenerateError> {
    let kind = field.r#type.as_ref().and_then(|ty| ty.kind.as_ref());
    match kind {
        Some(v2::field_type::Kind::Primitive(primitive)) => {
            Ok((proto_primitive(*primitive).to_string(), None))
        }
        Some(v2::field_type::Kind::InlineScalar(td)) => Ok((proto_scalar(td).to_string(), None)),
        Some(v2::field_type::Kind::Named(reference)) => {
            named_field_type(package, owner, &field.name, reference)
        }
        Some(_) => Err(GenerateError {
            message: format!(
                "{owner}.{} has a type this backend does not project yet (E9.8 tier 1 \
                 lands over several commits).",
                field.name
            ),
        }),
        None => Err(GenerateError {
            message: format!("{owner}.{} carries no type in the IR.", field.name),
        }),
    }
}

/// A resolved named-type reference at a field position. A named scalar
/// inlines to its backing scalar and leaves its name, unit, range and step
/// as a comment; a struct reference names the message it becomes; an enum
/// reference names the `enum` it becomes ([`emit_enum`]).
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
        _ => Err(GenerateError {
            message: format!(
                "{owner}.{field_name} references `{reference}`, a declaration kind this \
                 backend does not project yet (E9.8 tier 1 lands over several commits)."
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
