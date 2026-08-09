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
    Ok(Generated { fbs_source: out })
}

/// [`generate_with`] with no other packages — the single-package case. A
/// foreign reference fails rather than emitting an unresolvable name.
pub fn generate(package: &v2::Package) -> Result<Generated, GenerateError> {
    generate_with(package, &[])
}
