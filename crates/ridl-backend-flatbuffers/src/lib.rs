//! IR v2 package to a FlatBuffers schema (roadmap story E9.9, ADR-0013
//! decision 2).
//!
//! The second wire backend. The emit ceiling is two tiers — the typl surface
//! and the interaction identity table — and nothing above them. No
//! `rpc_service`, no reply carriers, no store.
//!
//! Four rules here differ from the proto3 backend and are not interchangeable
//! with it: a union is isolated in a wrapper table because a native union
//! owns two id slots; a struct is always emitted as a `table` because a
//! FlatBuffers `struct` fabricates a value after a compatible append; enum
//! values are scoped to their enum rather than to the namespace, so no value
//! prefixing is emitted and no zero member is synthesized into the
//! declaration; and a table field whose enum declares no zero-valued member
//! takes `= null`, because FlatBuffers gives every table field a default and
//! cannot mark a scalar or enum field required in any case.

use ridl_ir::v2;

#[cfg(test)]
mod tests;

/// The generated FlatBuffers schema for one package.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Generated {
    pub fbs_source: String,
}

/// A failure to generate a schema from a package.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenerateError {
    pub message: String,
}

/// Generates the FlatBuffers schema for `package`, resolving foreign
/// references against `others` (ADR-0017 decision 1).
pub fn generate_with(
    package: &v2::Package,
    others: &[&v2::Package],
) -> Result<Generated, GenerateError> {
    let _ = others;
    let mut out = String::new();
    out.push_str(&format!("namespace {};\n", package.name));
    emit_structs(&mut out, package)?;
    emit_identity_tables(&mut out, package);
    Ok(Generated { fbs_source: out })
}

/// A dotted address becomes one CamelCase FlatBuffers identifier:
/// `corpus.baseline.hvac` gives `CorpusBaselineHvac`. A named interface has
/// no dots, so its name passes through unchanged.
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

/// Tier 1 (ADR-0013 decision 2): walks every top-level declaration, emitting
/// a `table` for each struct ([`emit_struct`]) and an `enum` for each typl
/// enum ([`emit_enum`]). A named scalar and an enum set inline at each use
/// site instead of becoming a declaration of their own, the same as in
/// `ridl-backend-proto`'s tier 1, and a constant is never emitted (ADR-0013
/// decision 5) — so a `TypeDef`, `EnumSetDef` or `ConstDef` declaration
/// simply contributes nothing here. A union is tier 1 as well, but is out of
/// this task's scope; a later task adds its match arm alongside these.
fn emit_structs(out: &mut String, package: &v2::Package) -> Result<(), GenerateError> {
    for decl in &package.decls {
        match &decl.kind {
            Some(v2::decl::Kind::StructDef(def)) => {
                emit_struct(out, package, &decl.name, def)?;
            }
            Some(v2::decl::Kind::EnumDef(def)) => emit_enum(out, &decl.name, def),
            _ => {}
        }
    }
    Ok(())
}

/// One enum as one FlatBuffers `enum` (typl §8): values keep their explicitly
/// assigned numbers, unprefixed and in declaration order, because FlatBuffers
/// scopes an enum's values inside the enum rather than as siblings of it —
/// two typl enums in one package may each declare a value named `OK` without
/// conflict, unlike proto3. A `Reserved` retired value ([`v2::EnumDef::reserved`])
/// contributes nothing: FlatBuffers has no `reserved` construct for an enum
/// declaration.
///
/// The underlying type is always `long`. `EnumValue.value` is `int64` in the
/// IR, and narrowing to the smallest type that fits the declared values would
/// make the underlying type a function of those values — a later value could
/// then widen it, which is a wire change under an otherwise compatible edit.
fn emit_enum(out: &mut String, name: &str, def: &v2::EnumDef) {
    out.push_str(&format!("\nenum {name} : long {{\n"));
    for value in &def.values {
        out.push_str(&format!("  {} = {},\n", value.name, value.value));
    }
    out.push_str("}\n");
}

/// One struct as one FlatBuffers `table`, one field per member, its `id` the
/// typl ordinal minus one — FlatBuffers ids start at 0, typl ordinals at 1
/// (typl §7.4). A field name goes through the pinned transform
/// ([`ridl_ir::name::snake_case`]). A `Reserved` member holds its ordinal's
/// slot as a placeholder field marked `deprecated`, so a later id never moves
/// onto it; the placeholder's declared type is inert, because a deprecated
/// field generates no accessor, so `ubyte` is used regardless of what was
/// retired.
///
/// **`def.fixed_layout` is read nowhere in this function.** typl Appendix D
/// permits a struct whose fields are all fixed-width and non-optional to be
/// emitted as a FlatBuffers `struct` instead — inline, zero indirection.
/// This backend never takes that allowance: appending a struct field is a
/// compatible change in typl, and a FlatBuffers `struct` reads past what an
/// older writer wrote after such an append, fabricating a value from padding,
/// rather than reporting the new field absent — which makes ADR-0016
/// decision 6 property 3 unsatisfiable, and silently. A `table` reports the
/// field correctly absent, so every struct is emitted as one, whatever
/// `fixed_layout` says.
fn emit_struct(
    out: &mut String,
    package: &v2::Package,
    name: &str,
    def: &v2::StructDef,
) -> Result<(), GenerateError> {
    out.push_str(&format!("\ntable {name} {{\n"));
    for member in &def.members {
        match &member.member {
            Some(v2::struct_member::Member::Field(field)) => {
                let id = member_id(name, field.ordinal)?;
                let field_name = ridl_ir::name::snake_case(&field.name);
                let ty = field.r#type.as_ref().ok_or_else(|| GenerateError {
                    message: format!("{name}.{} carries no type in the IR.", field.name),
                })?;
                let (scalar, needs_null_default, comment) =
                    resolve_field_type(package, name, &field.name, ty)?;
                if let Some(comment) = comment {
                    out.push_str(&format!("  {comment}\n"));
                }
                // `flatc` refuses a field whose implicit default of 0 is not
                // a member of its enum. This applies whether or not the typl
                // field is optional: FlatBuffers cannot mark a scalar or
                // enum field `required` in any case, so `= null` is the
                // rendering that never fabricates a reading.
                let default_clause = if needs_null_default { " = null" } else { "" };
                out.push_str(&format!(
                    "  {field_name}: {scalar}{default_clause} (id: {id});\n"
                ));
            }
            Some(v2::struct_member::Member::Reserved(reserved)) => {
                let id = member_id(name, reserved.ordinal)?;
                out.push_str(&format!(
                    "  reserved_{}: ubyte (id: {id}, deprecated);\n",
                    reserved.ordinal
                ));
            }
            None => {}
        }
    }
    out.push_str("}\n");
    Ok(())
}

/// The FlatBuffers `id` for one struct member: the typl ordinal minus one.
/// Refused rather than subtracted with a wrapping or panicking underflow if
/// the IR ever carries ordinal 0 — an ordinal typl itself never assigns
/// (typl §7.4), so this is defensive against malformed IR, not a case
/// reachable through the compiler.
fn member_id(owner: &str, ordinal: u32) -> Result<u32, GenerateError> {
    ordinal.checked_sub(1).ok_or_else(|| GenerateError {
        message: format!(
            "`{owner}` carries a struct member with ordinal 0, which FlatBuffers ids cannot \
             represent — typl ordinals start at 1 (typl §7.4)."
        ),
    })
}

/// The FlatBuffers type at one field position — tier 1's data-only subset: a
/// bare primitive resolves directly ([`fbs_primitive`]), and a named-type
/// reference inlines a named scalar, enum or enum set to its backing
/// FlatBuffers type ([`named_field_type`]). The middle element of the result
/// is whether the field needs an explicit `= null` default (only ever true
/// for an enum reference — see [`emit_struct`]); a container field (array,
/// map, tuple) or a stream is out of this task's scope, and later tasks
/// extend this match.
fn resolve_field_type(
    package: &v2::Package,
    owner: &str,
    field_name: &str,
    ty: &v2::FieldType,
) -> Result<(String, bool, Option<String>), GenerateError> {
    match ty.kind.as_ref() {
        Some(v2::field_type::Kind::Primitive(primitive)) => {
            Ok((fbs_primitive(*primitive).to_string(), false, None))
        }
        Some(v2::field_type::Kind::Named(reference)) => {
            named_field_type(package, owner, field_name, reference)
        }
        _ => Err(GenerateError {
            message: format!(
                "{owner}.{field_name} uses a field type this tier does not project yet."
            ),
        }),
    }
}

/// A resolved named-type reference at a field position, same-package only —
/// [`generate_with`]'s `others` is not yet consulted here.
///
/// A named scalar inlines to its FlatBuffers scalar and leaves its declared
/// form as a comment on the line above it ([`fbs_scalar`],
/// [`constraint_comment`]).
///
/// An enum resolves to its own declared name ([`emit_enum`]), and reports
/// whether the field needs `= null`: `flatc` requires every table field to
/// carry a default, and refuses one whose implicit default of 0 is not a
/// member of the referenced enum — so this is true exactly when the enum
/// declares no zero-valued member.
///
/// An enum set has no declaration of its own — a FlatBuffers enum field
/// holds one value and cannot represent a combination of bits — so it
/// resolves to the FlatBuffers scalar for its declared width, with its bit
/// names and positions as a comment ([`enum_set_field_type`]).
///
/// Any other declaration kind is refused: only a named scalar, enum or enum
/// set is projected from a field position yet.
fn named_field_type(
    package: &v2::Package,
    owner: &str,
    field_name: &str,
    reference: &str,
) -> Result<(String, bool, Option<String>), GenerateError> {
    let Some(decl) = package.decls.iter().find(|decl| decl.name == *reference) else {
        return Err(GenerateError {
            message: format!(
                "{owner}.{field_name} references `{reference}`, which is not a declaration \
                 of this package."
            ),
        });
    };
    match &decl.kind {
        Some(v2::decl::Kind::TypeDef(td)) => Ok((
            fbs_scalar(td).to_string(),
            false,
            Some(constraint_comment(reference, td)),
        )),
        Some(v2::decl::Kind::EnumDef(def)) => {
            let zero_declared = def.values.iter().any(|value| value.value == 0);
            Ok((decl.name.clone(), !zero_declared, None))
        }
        Some(v2::decl::Kind::EnumSetDef(esd)) => {
            let (scalar, comment) = enum_set_field_type(esd);
            Ok((scalar, false, comment))
        }
        _ => Err(GenerateError {
            message: format!(
                "{owner}.{field_name} references `{reference}`, a declaration kind this \
                 tier does not project yet — only a named scalar, enum or enum set may be \
                 referenced from a field position here."
            ),
        }),
    }
}

/// The FlatBuffers scalar and use-site comment for an enum set (mirrors
/// `ridl-backend-proto`'s `enum_set_field_type`). A FlatBuffers enum field
/// holds one value, and an enum set is a combination of bits, so it gains no
/// declaration of its own — like a named scalar, it resolves at each use
/// site instead. The scalar is [`fbs_scalar`] at the enum set's declared
/// width; the bit names and positions become one comment line each, in the
/// form `LOW_FUEL = bit 0`. Emitting a FlatBuffers enum here instead would
/// imply a guarantee the target does not make (ADR-0013 decision 2): one
/// enum field holds one value, never a combination of bits.
fn enum_set_field_type(esd: &v2::EnumSetDef) -> (String, Option<String>) {
    let scalar = fbs_scalar(&v2::TypeDef {
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

/// The FlatBuffers scalar for a direct primitive use at a field position. A
/// bare `integer` or `float` carries no derived width, so the full typl
/// domain is emitted: `long` and `double` (typl §4).
fn fbs_primitive(primitive: i32) -> &'static str {
    match v2::PrimitiveType::try_from(primitive) {
        Ok(v2::PrimitiveType::Boolean) => "bool",
        Ok(v2::PrimitiveType::Integer) => "long",
        Ok(v2::PrimitiveType::Float) => "double",
        Ok(v2::PrimitiveType::Bytes) => "[ubyte]",
        _ => "string",
    }
}

/// The FlatBuffers scalar for a resolved typl width (typl Appendix D).
/// Unlike proto3's varint encoding, FlatBuffers stores every scalar at its
/// declared byte width, so the full `uint8`..`uint64` palette is used rather
/// than widened to the nearest transport-native size — the narrow width is
/// real bytes saved on the wire, which is the reason to choose this target
/// (ADR-0013 decision 4).
fn fbs_scalar(td: &v2::TypeDef) -> &'static str {
    match &td.width {
        Some(v2::type_def::Width::IntWidth(width)) => match v2::IntWidth::try_from(*width) {
            Ok(v2::IntWidth::U8) => "ubyte",
            Ok(v2::IntWidth::U16) => "ushort",
            Ok(v2::IntWidth::U32) => "uint",
            Ok(v2::IntWidth::U64) => "ulong",
            Ok(v2::IntWidth::I8) => "byte",
            Ok(v2::IntWidth::I16) => "short",
            Ok(v2::IntWidth::I32) => "int",
            Ok(v2::IntWidth::I64) => "long",
            _ => "long",
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
                    Ok(v2::PrimitiveType::Bytes) => "[ubyte]",
                    _ => "string",
                }
            }
            _ => "string",
        },
    }
}

/// The constraint information FlatBuffers has no construct for, as a comment
/// on the line above the field it describes (design mirrors
/// `ridl-backend-proto`'s comment of the same purpose — the constraint text
/// itself does not depend on the target).
fn constraint_comment(declared: &str, td: &v2::TypeDef) -> String {
    let mut form = Vec::new();
    if let Some(v2::backing::Kind::Unit(unit)) = td.backing.as_ref().and_then(|b| b.kind.as_ref())
    {
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

/// Tier 2 (ADR-0013 decision 2): one enum per interface shape, interface-wide
/// and kind-blind, matching ridl §11's single ordinal sequence.
///
/// Two rules here differ from the proto3 backend's identity table.
/// FlatBuffers scopes an enum's values inside the enum, not as siblings of
/// it, so no zero member needs synthesizing and no value is prefixed with
/// the enum's own name — two enums may each declare a member named `OK`
/// without conflict. And a retired ordinal (a `Reserved` tombstone)
/// contributes no member at all: a FlatBuffers enum declaration has no
/// `reserved` construct, and this table is a name-to-number map rather than
/// a wire layout, so a gap in the numbering is simply a gap.
fn emit_identity_tables(out: &mut String, package: &v2::Package) {
    for shape in package.shapes() {
        let enum_name = format!("{}Ordinal", type_name(shape.name));
        out.push_str(&format!("\nenum {enum_name} : uint {{\n"));
        for decl in &shape.interface.interactions {
            if matches!(decl.kind, Some(v2::decl::Kind::ReservedSlot(_))) {
                continue;
            }
            let member = screaming_snake_case(&decl.name);
            out.push_str(&format!("  {member} = {},\n", decl.ordinal));
        }
        out.push_str("}\n");
    }
}

/// [`generate_with`] with no other packages — the single-package case. A
/// foreign reference fails rather than emitting an unresolvable name.
pub fn generate(package: &v2::Package) -> Result<Generated, GenerateError> {
    generate_with(package, &[])
}
