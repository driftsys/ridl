//! The minimal AST -> IR checker (docs/ROADMAP.md epic E0.7): lowers every
//! `type`/`const` definition to IR and runs one real semantic check — a const
//! value must lie inside its resolved type's range. Composite definitions are
//! not lowered yet — the full checker is E1 scope.
//!
//! Diagnostics accumulate; lowering continues past errors — the checker never
//! returns a hard error (ADR-0004 §5).
//!
//! Reads the `typl.ungram`-generated typed AST (`ridl_syntax::ast`) — ported
//! from the E0 accessor layer in E1.2b.

use std::collections::HashMap;

use ridl_ir::{ConstDef, Module, Range, TypeDef};
use ridl_syntax::ast::{self, AstNode, Definition, SourceFile};
use rowan::TextRange;

use crate::resolve::{FileResolution, SymbolKind, const_type_name, declared_name, literal_f64};

/// A semantic diagnostic raised while lowering the AST to IR. It carries the
/// stable diagnostic code it maps to in the coded model (`ridl_core::diag`,
/// E1.10) and the source range of the offending construct.
#[derive(Debug, Clone, PartialEq)]
pub struct CheckError {
    pub message: String,
    pub code: &'static str,
    pub range: TextRange,
}

/// Lowers every `type`/`const` definition in `file` to IR under `module_name`,
/// checking that each const value lies inside its resolved type's range.
///
/// A const whose type reference did not resolve is left unlowered: the resolver
/// already reported the name error, so emitting IR for the const would
/// fabricate a `type_name`. An out-of-range value is still lowered — the value
/// is representable and the range violation is a semantic fact worth carrying
/// downstream — but it also raises a [`CheckError`].
pub fn check(
    file: &SourceFile,
    resolution: &FileResolution,
    module_name: &str,
) -> (Module, Vec<CheckError>) {
    let mut errors = Vec::new();

    // Lower each `type` and record its declared range for the const check.
    let mut types = Vec::new();
    let mut type_ranges: HashMap<String, Range> = HashMap::new();
    for definition in file.definitions() {
        let Definition::Type(decl) = definition else {
            continue;
        };
        // A nameless type means a malformed tree; the parser already reported
        // the syntax error, so skip it rather than fabricate a name.
        let Some(name) = declared_name(&decl) else {
            continue;
        };
        let range = scalar_range(&decl);
        if let Some(range) = range {
            type_ranges.insert(name.clone(), range);
        }
        types.push(TypeDef {
            name,
            unit: backing_text(&decl),
            range,
        });
    }

    // Lower each `const` whose type resolved, checking its value against range.
    let mut consts = Vec::new();
    for definition in file.definitions() {
        let Definition::Const(decl) = definition else {
            continue;
        };
        // A malformed tree (or a non-numeric constant, e.g. a regex): the
        // parser or the E1 checker owns those; the E0-scope lowering covers
        // numeric constants only.
        let value_literal = decl.value();
        let (Some(name), Some(type_name), Some(value)) = (
            declared_name(&decl),
            const_type_name(&decl),
            value_literal.as_ref().and_then(literal_f64),
        ) else {
            continue;
        };
        // The value literal is present whenever `value` parsed, so its range is
        // the caret site for an out-of-range diagnostic.
        let value_range = value_literal
            .as_ref()
            .map(|literal| literal.syntax().text_range())
            .unwrap_or_default();

        // Skip a const whose type reference did not resolve to a declared
        // `type`: the resolver owns that diagnostic, and lowering the const
        // would fabricate a `type_name`.
        if resolution.symbols.get(&type_name) != Some(&SymbolKind::Type) {
            continue;
        }

        // typl §5.5: a const value must lie within its type's inclusive range
        // (TYPL-108, typl reference §16.2). Step conformance (quantization) is
        // checked in a later task, not here.
        if let Some(range) = type_ranges.get(&type_name)
            && (value < range.min || value > range.max)
        {
            errors.push(CheckError {
                message: format!(
                    "const `{name}` value {value} outside `{type_name}` range [{}, {}]",
                    range.min, range.max
                ),
                code: "TYPL-108",
                range: value_range,
            });
        }

        consts.push(ConstDef {
            name,
            type_name,
            value,
        });
    }

    let module = Module {
        name: module_name.to_string(),
        types,
        consts,
    };
    (module, errors)
}

/// The backing of a `type` definition as IR text: the UCUM unit expression
/// (`km/h`) or the primitive keyword (`integer`). Empty when the backing is
/// missing (a malformed tree).
fn backing_text(decl: &ast::TypeDef) -> String {
    decl.backing()
        .map(|backing| crate::resolve::significant_text(backing.syntax()))
        .unwrap_or_default()
}

/// The scalar range of a `type` definition's constraint, lowered to IR: both
/// numeric endpoints must be present (open ranges and constant-reference
/// bounds resolve in E1), and an unstated step maps to proto3's `0.0`.
fn scalar_range(decl: &ast::TypeDef) -> Option<Range> {
    let constraint = decl.constraint()?;
    let min = literal_f64(&constraint.min()?)?;
    let max = literal_f64(&constraint.max()?)?;
    let step = constraint
        .step()
        .and_then(|literal| literal_f64(&literal))
        .unwrap_or(0.0);
    Some(Range { min, max, step })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resolve::resolve;
    use ridl_ir::{ConstDef, Range, TypeDef};
    use ridl_syntax::parse;

    const FIXTURE: &str = include_str!("../../ridl-syntax/fixtures/walking_skeleton.typl");

    fn check_source(input: &str, module_name: &str) -> (Module, Vec<CheckError>) {
        let file = SourceFile::cast(parse(input).syntax()).expect("root is a SourceFile");
        let resolution = resolve(&file);
        check(&file, &resolution, module_name)
    }

    #[test]
    fn fixture_lowers_to_the_expected_module() {
        let (module, errors) = check_source(FIXTURE, "walking_skeleton");
        assert!(errors.is_empty(), "expected no errors, got: {errors:?}");
        assert_eq!(
            module,
            Module {
                name: "walking_skeleton".to_string(),
                types: vec![TypeDef {
                    name: "Speed".to_string(),
                    unit: "km/h".to_string(),
                    range: Some(Range {
                        min: 0.0,
                        max: 250.0,
                        step: 0.5,
                    }),
                }],
                consts: vec![ConstDef {
                    name: "MAX_SPEED".to_string(),
                    type_name: "Speed".to_string(),
                    value: 250.0,
                }],
            },
        );
    }

    #[test]
    fn out_of_range_const_yields_one_error_and_is_still_lowered() {
        let input = "type Speed: km/h [0.0..250.0 step 0.5]\nconst TOO_FAST: Speed = 300.0\n";
        let (module, errors) = check_source(input, "m");
        assert_eq!(errors.len(), 1, "expected one error, got: {errors:?}");
        assert_eq!(errors[0].code, "TYPL-108");
        assert!(errors[0].message.contains("TOO_FAST"));
        assert!(errors[0].message.contains("Speed"));
        // The value is representable, so the const is still lowered.
        assert_eq!(module.consts.len(), 1);
        assert_eq!(module.consts[0].name, "TOO_FAST");
        assert_eq!(module.consts[0].value, 300.0);
    }

    #[test]
    fn const_with_unresolved_type_is_skipped_without_panic() {
        let input = "const X: Missing = 1.0\n";
        let (module, errors) = check_source(input, "m");
        // The resolver owns the unknown-type diagnostic; the checker adds none.
        assert!(
            errors.is_empty(),
            "expected no check errors, got: {errors:?}"
        );
        // The const is not lowered — its type reference did not resolve.
        assert!(module.consts.is_empty());
    }

    #[test]
    fn negative_bounds_lower_with_their_sign() {
        let input = "package p\ntype T: Cel [-40.0..125.0 step 0.1]\n";
        let (module, errors) = check_source(input, "m");
        assert!(errors.is_empty(), "expected no errors, got: {errors:?}");
        assert_eq!(
            module.types[0].range,
            Some(Range {
                min: -40.0,
                max: 125.0,
                step: 0.1,
            }),
        );
    }
}
