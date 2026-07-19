//! Init-value derivation (typl language reference §5.8; docs/ROADMAP.md epic
//! E1.9). Every lowered [`v2::TypeDef`] and [`v2::Field`] carries a populated
//! [`v2::InitValue`] — either the source-declared `= value` (validated as
//! TYPL-109 by the checker) or the value derived here per the §5.8 table.
//!
//! The [`v2::InitValue`] carrier has three states (documented on the proto
//! message):
//!
//! - `{ derivable: true, value: Some(text) }` — a **scalar** init: the
//!   canonical text form of a boolean, numeric, string/bytes, enum, or enumset
//!   default.
//! - `{ derivable: true, value: None }` — a **composite** init (struct, tuple,
//!   collection, union) or an absent optional. Composite inits are not
//!   materialized in the IR; a consumer derives them recursively from member
//!   inits (typl §5.8, the proto comment on `InitValue`).
//! - `{ derivable: false, value: None }` — **not derivable** (a string/bytes
//!   type whose bounds forbid length 0, or a type carrying a `match` pattern).
//!   The checker emits TYPL-115 (info) for a named type in this state.
//!
//! Derived numeric values are always constructed through [`ExactValue`] and
//! rendered with [`ExactValue::to_decimal_string`], so every value in the IR
//! stays a terminating canonical decimal string (ADR-0007 decision 9); a raw
//! `numer/denom` fraction never reaches the IR.

use ridl_ir::v2;

use crate::scalar::ExactValue;

/// A scalar init: derivable, with the canonical text form of the value.
fn scalar(value: &str) -> v2::InitValue {
    v2::InitValue {
        derivable: true,
        value: Some(value.to_string()),
    }
}

/// A composite or absent init: derivable, with no materialized scalar value —
/// a consumer reconstructs it from the member inits (typl §5.8).
fn composite() -> v2::InitValue {
    v2::InitValue {
        derivable: true,
        value: None,
    }
}

/// A non-derivable init (TYPL-115): no value can be synthesised and none was
/// declared.
fn not_derivable() -> v2::InitValue {
    v2::InitValue {
        derivable: false,
        value: None,
    }
}

/// Derives the init of a scalar [`v2::TypeDef`] from its backing and constraint
/// (typl §5.8). Used for a named `type` declaration and for an inline
/// constrained scalar in field position. The `TypeDef` carries its resolved
/// `constraint` (exact decimal bounds, length bounds, `match` pattern) but not
/// yet an `init`.
pub fn derive_type_init(type_def: &v2::TypeDef) -> v2::InitValue {
    let constraint = type_def.constraint.as_ref();
    match type_def
        .backing
        .as_ref()
        .and_then(|backing| backing.kind.as_ref())
    {
        // A unit backing is numeric (its underlying primitive is `float`,
        // typl §5.1).
        Some(v2::backing::Kind::Unit(_)) => numeric_init(constraint),
        Some(v2::backing::Kind::Primitive(code)) => match primitive(*code) {
            v2::PrimitiveType::Boolean => scalar("false"),
            v2::PrimitiveType::Integer | v2::PrimitiveType::Float => numeric_init(constraint),
            v2::PrimitiveType::String | v2::PrimitiveType::Bytes => string_init(constraint),
            v2::PrimitiveType::Unspecified => composite(),
        },
        None => composite(),
    }
}

/// Derives the init of a bare primitive field type — one used directly, without
/// an inline constraint (typl §5.8). A bare `string`/`bytes` field is rejected
/// elsewhere (TYPL-208); its derived init follows the default `[0..256]` bound,
/// which admits the empty value.
pub fn derive_primitive_init(primitive: v2::PrimitiveType) -> v2::InitValue {
    match primitive {
        v2::PrimitiveType::Boolean => scalar("false"),
        v2::PrimitiveType::Integer | v2::PrimitiveType::Float => scalar("0"),
        v2::PrimitiveType::String | v2::PrimitiveType::Bytes => scalar(""),
        v2::PrimitiveType::Unspecified => composite(),
    }
}

/// The numeric derived init (typl §5.8): `0` when it lies within the range,
/// otherwise the range minimum. Bounds are the exact decimal strings the
/// checker resolved during lowering.
fn numeric_init(constraint: Option<&v2::Constraint>) -> v2::InitValue {
    let min = constraint
        .and_then(|constraint| constraint.min.as_deref())
        .and_then(ExactValue::parse);
    let max = constraint
        .and_then(|constraint| constraint.max.as_deref())
        .and_then(ExactValue::parse);
    numeric_zero_or_min(min, max)
}

/// The numeric derived init (typl §5.8) from exact bounds: `0` when it lies in
/// `[min, max]`, otherwise the minimum (or, for a range that is entirely below
/// zero with an open minimum, the maximum). The chosen value is always rendered
/// through [`ExactValue::to_decimal_string`] so the IR keeps a terminating
/// canonical decimal.
pub fn numeric_zero_or_min(min: Option<ExactValue>, max: Option<ExactValue>) -> v2::InitValue {
    let zero = ExactValue::parse("0").expect("`0` parses");
    let above_min = min.as_ref().is_none_or(|bound| zero.0 >= bound.0);
    let below_max = max.as_ref().is_none_or(|bound| zero.0 <= bound.0);
    let value = if above_min && below_max {
        zero
    } else {
        min.or(max).unwrap_or(zero)
    };
    v2::InitValue {
        derivable: true,
        value: Some(value.to_decimal_string()),
    }
}

/// The string/bytes derived init (typl §5.8): the empty value when the bounds
/// admit length 0 and no `match` pattern is present, otherwise not derivable.
pub fn string_init(constraint: Option<&v2::Constraint>) -> v2::InitValue {
    let has_pattern = constraint.is_some_and(|constraint| {
        constraint.pattern.is_some() || constraint.pattern_const.is_some()
    });
    if has_pattern {
        // A `match`-typed value cannot be synthesised (typl §5.8).
        return not_derivable();
    }
    let len_min = constraint
        .and_then(|constraint| constraint.len_min)
        .unwrap_or(0);
    if len_min == 0 {
        scalar("")
    } else {
        not_derivable()
    }
}

/// Derives the init of a struct field from its lowered field type (typl §5.8).
/// Scalar field types (primitive, inline scalar) materialize their value;
/// composite field types (tuple, collection) and named references defer to
/// `resolve_named`, which the checker supplies to resolve a named type to its
/// own init. The optional flag wins: an optional field is absent by default,
/// which is always derivable.
pub fn derive_field_init(
    field_type: &v2::FieldType,
    resolve_named: &dyn Fn(&str) -> v2::InitValue,
) -> v2::InitValue {
    if field_type.optional {
        // Optional fields are absent in the derived init (typl §5.8).
        return composite();
    }
    match &field_type.kind {
        Some(v2::field_type::Kind::Primitive(code)) => derive_primitive_init(primitive(*code)),
        Some(v2::field_type::Kind::InlineScalar(type_def)) => derive_type_init(type_def),
        Some(v2::field_type::Kind::Named(name)) => resolve_named(name),
        Some(v2::field_type::Kind::Tuple(tuple)) => {
            // A tuple derives to each field's init; it is derivable only when
            // every field is (typl §5.8).
            let derivable = tuple.fields.iter().all(|field| {
                field
                    .r#type
                    .as_ref()
                    .is_none_or(|inner| derive_field_init(inner, resolve_named).derivable)
            });
            v2::InitValue {
                derivable,
                value: None,
            }
        }
        Some(v2::field_type::Kind::Array(array)) => {
            // A collection derives to a `min`-count of element inits: empty
            // (derivable) when `min == 0`, otherwise derivable only when the
            // element is (typl §5.8).
            let element = array
                .element
                .as_ref()
                .is_none_or(|inner| derive_field_init(inner, resolve_named).derivable);
            v2::InitValue {
                derivable: array.min == 0 || element,
                value: None,
            }
        }
        Some(v2::field_type::Kind::Map(map)) => {
            let value = map
                .value
                .as_ref()
                .is_none_or(|inner| derive_field_init(inner, resolve_named).derivable);
            v2::InitValue {
                derivable: map.min == 0 || value,
                value: None,
            }
        }
        // A stream is an interaction-position type (ridl §12.3); it never
        // reaches a struct field in checked IR and carries no init.
        Some(v2::field_type::Kind::Stream(_)) => not_derivable(),
        None => composite(),
    }
}

/// The primitive enum from its wire code, defaulting to `Unspecified` for an
/// out-of-range value.
fn primitive(code: i32) -> v2::PrimitiveType {
    v2::PrimitiveType::try_from(code).unwrap_or(v2::PrimitiveType::Unspecified)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn boolean() -> v2::TypeDef {
        v2::TypeDef {
            backing: Some(v2::Backing {
                kind: Some(v2::backing::Kind::Primitive(
                    v2::PrimitiveType::Boolean as i32,
                )),
            }),
            constraint: None,
            declared_init: None,
            init: None,
            width: None,
        }
    }

    fn numeric(min: &str, max: &str) -> v2::TypeDef {
        v2::TypeDef {
            backing: Some(v2::Backing {
                kind: Some(v2::backing::Kind::Primitive(
                    v2::PrimitiveType::Integer as i32,
                )),
            }),
            constraint: Some(v2::Constraint {
                min: Some(min.to_string()),
                max: Some(max.to_string()),
                step: None,
                len_min: None,
                len_max: None,
                pattern: None,
                pattern_const: None,
            }),
            declared_init: None,
            init: None,
            width: None,
        }
    }

    fn string(len_min: u64, len_max: u64, pattern: Option<&str>) -> v2::TypeDef {
        v2::TypeDef {
            backing: Some(v2::Backing {
                kind: Some(v2::backing::Kind::Primitive(
                    v2::PrimitiveType::String as i32,
                )),
            }),
            constraint: Some(v2::Constraint {
                min: None,
                max: None,
                step: None,
                len_min: Some(len_min),
                len_max: Some(len_max),
                pattern: pattern.map(str::to_string),
                pattern_const: None,
            }),
            declared_init: None,
            init: None,
            width: None,
        }
    }

    fn some(value: &str) -> v2::InitValue {
        scalar(value)
    }

    #[test]
    fn boolean_derives_false() {
        assert_eq!(derive_type_init(&boolean()), some("false"));
    }

    #[test]
    fn numeric_derives_zero_in_range_else_min() {
        assert_eq!(derive_type_init(&numeric("0", "20")), some("0"));
        assert_eq!(derive_type_init(&numeric("10", "20")), some("10"));
        // A range entirely below zero derives its minimum.
        assert_eq!(derive_type_init(&numeric("-100", "-10")), some("-100"));
    }

    #[test]
    fn string_derives_empty_or_not_derivable() {
        assert_eq!(derive_type_init(&string(0, 8, None)), some(""));
        assert_eq!(derive_type_init(&string(2, 8, None)), not_derivable());
        // A `match` pattern makes even a zero-length-admitting type
        // non-derivable.
        assert_eq!(
            derive_type_init(&string(0, 8, Some("/^[A-Z]*$/"))),
            not_derivable()
        );
    }

    #[test]
    fn derived_numeric_values_are_terminating_decimals() {
        // The chosen minimum is rendered through ExactValue: a value with a
        // terminating decimal keeps that form, never a raw fraction.
        assert_eq!(
            derive_type_init(&numeric("0.5", "1.0")).value.as_deref(),
            Some("0.5")
        );
    }
}
