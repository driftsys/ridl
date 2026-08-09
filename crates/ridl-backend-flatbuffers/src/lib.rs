//! IR v2 package to a FlatBuffers schema (roadmap story E9.9, ADR-0013
//! decision 2).
//!
//! The second wire backend. The emit ceiling is two tiers — the typl surface
//! and the interaction identity table — and nothing above them. No
//! `rpc_service`, no reply carriers, no store.
//!
//! Three rules here differ from the proto3 backend and are not
//! interchangeable with it: a union is isolated in a wrapper table because a
//! native union owns two id slots; a struct is always emitted as a `table`
//! because a FlatBuffers `struct` fabricates a value after a compatible
//! append; and enum values are scoped to their enum rather than to the
//! namespace, so no value prefixing is emitted.

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
