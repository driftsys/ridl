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
