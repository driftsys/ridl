//! The minimal AST -> IR checker (docs/ROADMAP.md epic E0.7): lowers every
//! `type`/`const` declaration to IR and runs one real semantic check — a const
//! value must lie inside its resolved type's range.
//!
//! Diagnostics accumulate; lowering continues past errors — the checker never
//! returns a hard error (ADR-0004 §5).

use std::collections::HashMap;

use ridl_ir::{ConstDef, Module, Range, TypeDef};
use ridl_syntax::{RangeSpec, SourceFile};

use crate::resolve::{Resolution, SymbolKind};

/// A semantic diagnostic raised while lowering the AST to IR.
#[derive(Debug, Clone, PartialEq)]
pub struct CheckError {
    pub message: String,
}

/// Lowers every `type`/`const` declaration in `file` to IR under `module_name`,
/// checking that each const value lies inside its resolved type's range.
///
/// A const whose type reference did not resolve is left unlowered: the resolver
/// already reported the name error, so emitting IR for the const would
/// fabricate a `type_name`. An out-of-range value is still lowered — the value
/// is representable and the range violation is a semantic fact worth carrying
/// downstream — but it also raises a [`CheckError`].
pub fn check(
    file: &SourceFile,
    resolution: &Resolution,
    module_name: &str,
) -> (Module, Vec<CheckError>) {
    let mut errors = Vec::new();

    // Lower each `type` and record its declared range for the const check.
    let mut types = Vec::new();
    let mut type_ranges: HashMap<String, RangeSpec> = HashMap::new();
    for decl in file.type_decls() {
        // A nameless type means a malformed tree; the parser already reported
        // the syntax error, so skip it rather than fabricate a name.
        let Some(name) = decl.name() else { continue };
        let range = decl.range();
        if let Some(range) = &range {
            type_ranges.insert(name.clone(), range.clone());
        }
        types.push(TypeDef {
            name,
            unit: decl.unit().unwrap_or_default(),
            range: range.map(lower_range),
        });
    }

    // Lower each `const` whose type resolved, checking its value against range.
    let mut consts = Vec::new();
    for decl in file.const_decls() {
        // A malformed tree; the parser already reported the syntax error.
        let (Some(name), Some(type_name), Some(value)) =
            (decl.name(), decl.type_name(), decl.value())
        else {
            continue;
        };

        // Skip a const whose type reference did not resolve to a declared
        // `type`: the resolver owns that diagnostic, and lowering the const
        // would fabricate a `type_name`.
        if resolution.symbols.get(&type_name) != Some(&SymbolKind::Type) {
            continue;
        }

        // typl §5.5: a const value must lie within its type's inclusive range.
        // Step conformance (quantization) is checked in E1, not here.
        if let Some(range) = type_ranges.get(&type_name)
            && (value < range.min || value > range.max)
        {
            errors.push(CheckError {
                message: format!(
                    "const `{name}` value {value} outside `{type_name}` range [{}, {}]",
                    range.min, range.max
                ),
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

/// Lowers an AST range to IR, mapping an unstated step to proto3's `0.0`.
fn lower_range(range: RangeSpec) -> Range {
    Range {
        min: range.min,
        max: range.max,
        step: range.step.unwrap_or(0.0),
    }
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
}
