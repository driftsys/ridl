//! Backend tests: per-construct snapshots, the Appendix B rustc-compile proof,
//! Default derivation behaviour (the leaf-recursion rule), and the C header
//! snapshot.

use super::{Ctx, Generated, check_flatbuffers_bound, generate};
use ridl_ir::v2;

// ---------------------------------------------------------------------------
// Fixture builders.
// ---------------------------------------------------------------------------

fn public_decl(name: &str, kind: v2::decl::Kind) -> v2::Decl {
    v2::Decl {
        name: name.to_string(),
        visibility: v2::Visibility::Public as i32,
        is_error: false,
        doc: String::new(),
        labels: Vec::new(),
        deprecated: None,
        ordinal: 0,
        kind: Some(kind),
    }
}

fn package(name: &str, decls: Vec<v2::Decl>) -> v2::Package {
    v2::Package {
        name: name.to_string(),
        decls,
        interfaces: Vec::new(),
        services: Vec::new(),
        retired: Vec::new(),
    }
}

fn init_value(derivable: bool, value: Option<&str>) -> v2::InitValue {
    v2::InitValue {
        derivable,
        value: value.map(str::to_string),
    }
}

fn constraint(min: Option<&str>, max: Option<&str>, step: Option<&str>) -> v2::Constraint {
    v2::Constraint {
        min: min.map(str::to_string),
        max: max.map(str::to_string),
        step: step.map(str::to_string),
        len_min: None,
        len_max: None,
        pattern: None,
        pattern_const: None,
    }
}

fn unit_type(unit: &str, min: &str, max: &str, step: &str, init: &str) -> v2::decl::Kind {
    v2::decl::Kind::TypeDef(v2::TypeDef {
        backing: Some(v2::Backing {
            kind: Some(v2::backing::Kind::Unit(unit.to_string())),
        }),
        constraint: Some(constraint(Some(min), Some(max), Some(step))),
        declared_init: None,
        init: Some(init_value(true, Some(init))),
        width: Some(v2::type_def::Width::FloatWidth(v2::FloatWidth::F32 as i32)),
    })
}

fn primitive_type(
    prim: v2::PrimitiveType,
    init: v2::InitValue,
    width: Option<v2::type_def::Width>,
) -> v2::decl::Kind {
    v2::decl::Kind::TypeDef(v2::TypeDef {
        backing: Some(v2::Backing {
            kind: Some(v2::backing::Kind::Primitive(prim as i32)),
        }),
        constraint: None,
        declared_init: None,
        init: Some(init),
        width,
    })
}

/// A `string`-backed named scalar carrying typl §4.4's default `[0..256]`
/// length bound.
///
/// The checker always materializes that bound (TYPL-103), so IR that reaches
/// a backend always has one; a hand-built fixture without one has no finite
/// FlatBuffers bound and the codec refuses it (design note D-7, §4a).
fn bounded_string_type(init: v2::InitValue) -> v2::decl::Kind {
    v2::decl::Kind::TypeDef(v2::TypeDef {
        backing: Some(v2::Backing {
            kind: Some(v2::backing::Kind::Primitive(
                v2::PrimitiveType::String as i32,
            )),
        }),
        constraint: Some(v2::Constraint {
            len_max: Some(256),
            ..Default::default()
        }),
        declared_init: None,
        init: Some(init),
        width: None,
    })
}

fn named_field(
    name: &str,
    ordinal: u32,
    type_ref: &str,
    optional: bool,
    init: v2::InitValue,
) -> v2::Field {
    v2::Field {
        name: name.to_string(),
        ordinal,
        r#type: Some(v2::FieldType {
            optional,
            kind: Some(v2::field_type::Kind::Named(type_ref.to_string())),
        }),
        declared_init: None,
        init: Some(init),
        doc: String::new(),
        labels: Vec::new(),
        deprecated: None,
    }
}

fn field_member(field: v2::Field) -> v2::StructMember {
    v2::StructMember {
        member: Some(v2::struct_member::Member::Field(field)),
    }
}

fn reserved_member(ordinal: u32, name: &str) -> v2::StructMember {
    v2::StructMember {
        member: Some(v2::struct_member::Member::Reserved(v2::Reserved {
            ordinal,
            name: Some(name.to_string()),
            value: None,
        })),
    }
}

fn enum_value(name: &str, value: i64) -> v2::EnumValue {
    v2::EnumValue {
        name: name.to_string(),
        value,
        doc: String::new(),
    }
}

fn warning_bits() -> Vec<v2::EnumValue> {
    vec![
        enum_value("LOW_FUEL", 0),
        enum_value("CHECK_ENGINE", 1),
        enum_value("DOOR_OPEN", 2),
        enum_value("SEATBELT", 3),
    ]
}

/// A `GearPosition`-like enum declaration used by the raw-discriminant
/// conversion tests.
///
/// The discriminants are deliberately not contiguous from zero, which is what
/// the `enum_with_discriminants` snapshot fixture uses. A conversion built
/// from each variant's position rather than its declared value emits the same
/// source for a contiguous fixture, so a gap is what makes the arm mapping
/// observable.
fn gear_position_decl() -> v2::Decl {
    public_decl(
        "GearPosition",
        v2::decl::Kind::EnumDef(v2::EnumDef {
            values: vec![
                enum_value("PARK", 0),
                enum_value("DRIVE", 1),
                enum_value("REVERSE", 7),
                enum_value("NEUTRAL", 9),
            ],
            reserved: Vec::new(),
        }),
    )
}

/// A `Features`-like enum set declaration used by the raw bit-pattern
/// conversion tests.
fn features_decl() -> v2::Decl {
    public_decl(
        "Features",
        v2::decl::Kind::EnumSetDef(v2::EnumSetDef {
            backing_enum: None,
            bits: warning_bits(),
            width: v2::IntWidth::U8 as i32,
        }),
    )
}

/// A `Speed`-like unit type declaration used across the small fixtures.
fn speed_decl() -> v2::Decl {
    v2::Decl {
        doc: "Vehicle speed over ground".to_string(),
        ..public_decl("Speed", unit_type("km/h", "0.0", "250.0", "0.5", "0.0"))
    }
}

/// A `string`-backed type carrying a positive length bound and a literal
/// pattern, used to pin that the pattern check is gated behind
/// `validate-pattern` while the length check is not. `len_min` and `len_max`
/// are both 17 so the minimum branch is emitted (a `len_min` of 0 emits no
/// branch at all).
fn vin_decl() -> v2::Decl {
    public_decl(
        "Vin",
        v2::decl::Kind::TypeDef(v2::TypeDef {
            backing: Some(v2::Backing {
                kind: Some(v2::backing::Kind::Primitive(
                    v2::PrimitiveType::String as i32,
                )),
            }),
            constraint: Some(v2::Constraint {
                len_min: Some(17),
                len_max: Some(17),
                pattern: Some("/[A-HJ-NPR-Z0-9]{17}/".to_string()),
                ..constraint(None, None, None)
            }),
            declared_init: None,
            init: Some(init_value(false, None)),
            width: None,
        }),
    )
}

/// A `bytes`-backed type carrying a positive length bound and a literal
/// pattern.
///
/// **No typl source produces this.** The reference gives bytes no `match`
/// (§4.5, §5.4), and `lower_scalar` passes `allow_pattern: false` for that
/// backing, so a `match` written on a bytes type is dropped and the IR
/// carries `{len_min, len_max}` and nothing else. The fixture is built by
/// hand to exercise the backend's totality over an IR it did not lower
/// itself, which is the same thing `a_range_on_a_non_numeric_backing_emits_no_range_check`
/// does for `min`/`max` — `lower_len_scalar` always leaves those absent too.
///
/// What it pins: `newtype_inner` gives such a type a `Vec<u8>`, against which
/// `regex::Regex::is_match` (which takes `&str`) does not type-check, so
/// `constraint_checks` must emit no pattern branch while still emitting its
/// length checks.
fn bytes_pattern_decl() -> v2::Decl {
    public_decl(
        "Sig",
        v2::decl::Kind::TypeDef(v2::TypeDef {
            backing: Some(v2::Backing {
                kind: Some(v2::backing::Kind::Primitive(
                    v2::PrimitiveType::Bytes as i32,
                )),
            }),
            constraint: Some(v2::Constraint {
                len_min: Some(1),
                len_max: Some(8),
                pattern: Some("/^[A-Z]+$/".to_string()),
                ..constraint(None, None, None)
            }),
            declared_init: None,
            init: Some(init_value(false, None)),
            width: None,
        }),
    )
}

/// A `string`-backed type whose pattern is a literal the stand-in `regex`
/// can decide, with `len_min` and `len_max` both 3 so a matching and a
/// non-matching value are the same length and the length checks cannot be
/// what separates them. Used by the executed proof of the pattern check.
fn literal_pattern_decl() -> v2::Decl {
    public_decl(
        "Code",
        v2::decl::Kind::TypeDef(v2::TypeDef {
            backing: Some(v2::Backing {
                kind: Some(v2::backing::Kind::Primitive(
                    v2::PrimitiveType::String as i32,
                )),
            }),
            constraint: Some(v2::Constraint {
                len_min: Some(3),
                len_max: Some(3),
                pattern: Some("/ABC/".to_string()),
                ..constraint(None, None, None)
            }),
            declared_init: None,
            init: Some(init_value(false, None)),
            width: None,
        }),
    )
}

fn counter_decl() -> v2::Decl {
    public_decl(
        "Counter",
        primitive_type(
            v2::PrimitiveType::Integer,
            init_value(true, Some("0")),
            Some(v2::type_def::Width::IntWidth(v2::IntWidth::U16 as i32)),
        ),
    )
}

// ---------------------------------------------------------------------------
// Per-construct snapshots.
// ---------------------------------------------------------------------------

fn rust_for(decls: Vec<v2::Decl>) -> String {
    generate(&package("veh.common", decls))
        .expect("generation succeeds")
        .rust_source
}

#[test]
fn scalar_unit_type_with_doc() {
    insta::assert_snapshot!(rust_for(vec![speed_decl()]));
}

#[test]
fn named_scalar_backings() {
    let decls = vec![
        speed_decl(),
        counter_decl(),
        public_decl(
            "Enabled",
            primitive_type(
                v2::PrimitiveType::Boolean,
                init_value(true, Some("false")),
                None,
            ),
        ),
        public_decl(
            "Label",
            primitive_type(v2::PrimitiveType::String, init_value(true, Some("")), None),
        ),
        public_decl(
            "Blob",
            primitive_type(v2::PrimitiveType::Bytes, init_value(true, Some("")), None),
        ),
    ];
    insta::assert_snapshot!(rust_for(decls));
}

#[test]
fn constrained_scalar_is_a_value_object() {
    let source = rust_for(vec![speed_decl()]);
    // The field is private: no `pub` inside the tuple struct.
    assert!(
        source.contains("pub struct Speed(f64)"),
        "inner field must be private, got:\n{source}"
    );
    // The plan's Task 3 spells `Result`, `TryFrom` and `From` unqualified.
    // The generated code names them by absolute path instead, because a
    // package may declare a type of the same name in the same module
    // (`prelude_names_declared_by_the_package_compile`); these assertions
    // follow the generated code, not the plan's text. The signature is
    // checked in two parts because prettyplease wraps it at its own width.
    assert!(source.contains("pub fn new("));
    assert!(source.contains(") -> ::core::result::Result<Self, ::ridl_rt::payload::Violation> {"));
    assert!(source.contains("pub const fn new_unchecked(value: f64) -> Self"));
    assert!(source.contains("pub const fn get(self) -> f64"));
    assert!(source.contains("impl ::core::convert::TryFrom<f64> for Speed"));
    assert!(source.contains("impl ::core::convert::From<Speed> for f64"));
    // The infallible inbound conversion must never appear on a constrained type.
    assert!(
        !source.contains("impl ::core::convert::From<f64> for Speed")
            && !source.contains("impl From<f64> for Speed"),
        "From<Inner> reintroduces unchecked construction"
    );
}

#[test]
fn vacuous_scalar_constructs_infallibly() {
    let decls = vec![public_decl(
        "Enabled",
        primitive_type(
            v2::PrimitiveType::Boolean,
            init_value(true, Some("false")),
            None,
        ),
    )];
    let source = rust_for(decls);
    assert!(source.contains("pub const fn new(value: bool) -> Self"));
    assert!(source.contains("impl ::core::convert::From<bool> for Enabled"));
    assert!(source.contains("impl ::core::convert::From<Enabled> for bool"));
    // No escape hatch is emitted: `new` already is one.
    assert!(
        !source.contains("Enabled::new_unchecked")
            && !source.contains("fn new_unchecked(value: bool)"),
        "new_unchecked would duplicate new on a vacuous type, got:\n{source}"
    );
    // And no manual TryFrom, which would collide with core's blanket impl.
    assert!(
        !source.contains("impl ::core::convert::TryFrom<bool> for Enabled")
            && !source.contains("impl TryFrom<bool> for Enabled"),
        "a manual TryFrom collides with core's blanket impl, got:\n{source}"
    );
}

#[test]
fn constant_of_a_constrained_type_uses_new_unchecked() {
    let decls = vec![
        speed_decl(),
        public_decl(
            "MAX_SPEED",
            v2::decl::Kind::ConstDef(v2::ConstDef {
                type_ref: Some("Speed".to_string()),
                value: "250.0".to_string(),
                regex: None,
            }),
        ),
    ];
    let source = rust_for(decls);
    assert!(source.contains("Speed::new_unchecked(250.0)"));
}

/// A string- or bytes-backed named scalar with a length bound and no range.
/// A type whose minimum length is positive has no derivable init (typl §5.8).
fn bounded_text_type(prim: v2::PrimitiveType, len_min: u64, len_max: u64) -> v2::decl::Kind {
    v2::decl::Kind::TypeDef(v2::TypeDef {
        backing: Some(v2::Backing {
            kind: Some(v2::backing::Kind::Primitive(prim as i32)),
        }),
        constraint: Some(v2::Constraint {
            len_min: Some(len_min),
            len_max: Some(len_max),
            ..constraint(None, None, None)
        }),
        declared_init: None,
        init: Some(if len_min == 0 {
            init_value(true, Some(""))
        } else {
            init_value(false, None)
        }),
        width: None,
    })
}

/// A float named scalar with a range and no `step`.
fn ratio_decl() -> v2::Decl {
    public_decl(
        "Ratio",
        v2::decl::Kind::TypeDef(v2::TypeDef {
            backing: Some(v2::Backing {
                kind: Some(v2::backing::Kind::Primitive(
                    v2::PrimitiveType::Float as i32,
                )),
            }),
            constraint: Some(constraint(Some("0.0"), Some("1.0"), None)),
            declared_init: None,
            init: Some(init_value(true, Some("0.0"))),
            width: None,
        }),
    )
}

/// The constraint branches `named_scalar_backings` does not reach: a string
/// length bound, a bytes length bound, and a range without a `step`.
#[test]
fn constrained_scalar_backings() {
    let decls = vec![
        public_decl("Name", bounded_text_type(v2::PrimitiveType::String, 1, 64)),
        public_decl("Digest", bounded_text_type(v2::PrimitiveType::Bytes, 0, 32)),
        ratio_decl(),
    ];
    insta::assert_snapshot!(rust_for(decls));
}

#[test]
fn a_string_length_bound_counts_characters() {
    let source = rust_for(vec![public_decl(
        "Name",
        bounded_text_type(v2::PrimitiveType::String, 1, 64),
    )]);
    for check in [
        "if (value.chars().count() as u64) < 1 {",
        "if (value.chars().count() as u64) > 64 {",
        "rule: ::ridl_rt::payload::Rule::Length,",
    ] {
        assert!(source.contains(check), "expected `{check}` in:\n{source}");
    }
}

#[test]
fn a_bytes_length_bound_counts_bytes() {
    let source = rust_for(vec![public_decl(
        "Digest",
        bounded_text_type(v2::PrimitiveType::Bytes, 4, 32),
    )]);
    for check in [
        "if (value.len() as u64) < 4 {",
        "if (value.len() as u64) > 32 {",
    ] {
        assert!(source.contains(check), "expected `{check}` in:\n{source}");
    }
}

/// `[0..256]` is the default length bound of string and bytes (typl §4.4,
/// §4.5), so a minimum of 0 is the common case. `(… as u64) < 0` is never
/// true and draws rustc's `unused_comparisons` warning in the consumer's
/// build, so the branch is not emitted.
#[test]
fn a_zero_length_minimum_emits_no_check() {
    for (name, prim) in [
        ("Text", v2::PrimitiveType::String),
        ("Blob", v2::PrimitiveType::Bytes),
    ] {
        let source = rust_for(vec![public_decl(name, bounded_text_type(prim, 0, 256))]);
        assert!(
            !source.contains("< 0"),
            "a zero minimum must emit no comparison, got:\n{source}"
        );
        assert!(
            source.contains("> 256"),
            "the maximum is still checked, got:\n{source}"
        );
    }
}

/// The newtype backing an integer named scalar is always `i64`
/// (`newtype_inner`). A maximum equal to `i64::MAX` (9223372036854775807)
/// makes `value > 9223372036854775807` never true: rustc draws its
/// `unused_comparisons` warning on it in the consumer's build, so the branch
/// is not emitted when the declared maximum equals the inner type's maximum.
#[test]
fn a_maximum_at_the_inner_types_maximum_emits_no_check() {
    let source = rust_for(vec![public_decl(
        "Timestamp",
        v2::decl::Kind::TypeDef(v2::TypeDef {
            backing: Some(v2::Backing {
                kind: Some(v2::backing::Kind::Primitive(
                    v2::PrimitiveType::Integer as i32,
                )),
            }),
            constraint: Some(constraint(Some("0"), Some("9223372036854775807"), None)),
            declared_init: None,
            init: Some(init_value(true, Some("0"))),
            width: None,
        }),
    )]);
    assert!(
        !source.contains("> 9223372036854775807"),
        "a maximum at i64::MAX must emit no comparison, got:\n{source}"
    );
    assert!(
        source.contains("if value < 0 {"),
        "the minimum is still checked, got:\n{source}"
    );
}

/// The suppression above is an exact equality, not a threshold: a maximum one
/// below `i64::MAX` is a comparison rustc cannot fold, so the branch is still
/// emitted. Without this test a guard that suppressed a range of large maxima
/// would pass the suite.
#[test]
fn a_maximum_below_the_inner_types_maximum_still_emits_a_check() {
    let source = rust_for(vec![public_decl(
        "Timestamp",
        v2::decl::Kind::TypeDef(v2::TypeDef {
            backing: Some(v2::Backing {
                kind: Some(v2::backing::Kind::Primitive(
                    v2::PrimitiveType::Integer as i32,
                )),
            }),
            constraint: Some(constraint(Some("0"), Some("9223372036854775806"), None)),
            declared_init: None,
            init: Some(init_value(true, Some("0"))),
            width: None,
        }),
    )]);
    assert!(
        source.contains("> 9223372036854775806"),
        "a maximum below i64::MAX must still be checked, got:\n{source}"
    );
}

/// The suppression applies to an integer backing only. A float backing is
/// never asked whether its maximum looks like `i64::MAX`, because the float
/// newtype is not backed by `i64` and the comparison is not a tautology there.
/// This pins that short circuit: without it, a float maximum that happens to
/// spell `i64::MAX` would lose its check.
#[test]
fn a_float_maximum_spelling_the_integer_maximum_still_emits_a_check() {
    let source = rust_for(vec![public_decl(
        "Ratio",
        v2::decl::Kind::TypeDef(v2::TypeDef {
            backing: Some(v2::Backing {
                kind: Some(v2::backing::Kind::Primitive(
                    v2::PrimitiveType::Float as i32,
                )),
            }),
            constraint: Some(constraint(Some("0.0"), Some("9223372036854775807"), None)),
            declared_init: None,
            init: Some(init_value(true, Some("0.0"))),
            width: None,
        }),
    )]);
    assert!(
        source.contains("> 9223372036854775807"),
        "a float maximum must be checked whatever it spells, got:\n{source}"
    );
}

/// A `min` or `max` on a non-numeric backing is not something `ridl-sem`
/// produces, but the IR is an artifact other tools write (ADR-0014), so the
/// backend ignores the two rather than rendering a numeric comparison against
/// a `String`.
#[test]
fn a_range_on_a_non_numeric_backing_emits_no_range_check() {
    let source = rust_for(vec![public_decl(
        "Handle",
        v2::decl::Kind::TypeDef(v2::TypeDef {
            backing: Some(v2::Backing {
                kind: Some(v2::backing::Kind::Primitive(
                    v2::PrimitiveType::String as i32,
                )),
            }),
            constraint: Some(v2::Constraint {
                len_min: Some(1),
                len_max: Some(8),
                ..constraint(Some("0"), Some("10"), None)
            }),
            declared_init: None,
            init: Some(init_value(false, None)),
            width: None,
        }),
    )]);
    assert!(
        !source.contains("Rule::Range")
            && !source.contains("if value <")
            && !source.contains("if value >"),
        "a range on a string backing must emit no range check, got:\n{source}"
    );
    assert!(
        source.contains("Rule::Length"),
        "the length bound is still checked, got:\n{source}"
    );
}

#[test]
fn a_range_without_a_step_names_nothing_unchecked() {
    let source = rust_for(vec![ratio_decl()]);
    assert!(
        !source.contains("is not checked by `new`"),
        "a constraint with no step and no pattern has nothing unchecked to name, got:\n{source}"
    );
    assert!(
        source.contains("if value > 1.0 {"),
        "the range is still checked, got:\n{source}"
    );
}

/// An unresolved `pattern_const` names the pattern as unchecked, because no
/// check is emitted for it (design, "Not validated, and documented as such"
/// still applies to that case). A length bound alone leaves nothing unchecked
/// to name.
#[test]
fn an_unresolved_pattern_const_is_named_on_the_type() {
    let with_pattern = |pattern_const: Option<&str>| {
        rust_for(vec![public_decl(
            "Handle",
            v2::decl::Kind::TypeDef(v2::TypeDef {
                backing: Some(v2::Backing {
                    kind: Some(v2::backing::Kind::Primitive(
                        v2::PrimitiveType::String as i32,
                    )),
                }),
                constraint: Some(v2::Constraint {
                    len_min: Some(3),
                    len_max: Some(8),
                    pattern: None,
                    pattern_const: pattern_const.map(str::to_string),
                    ..constraint(None, None, None)
                }),
                declared_init: None,
                init: Some(init_value(false, None)),
                width: None,
            }),
        )])
    };
    let source = with_pattern(Some("HANDLE_PATTERN"));
    assert!(
        source.contains("/// The `match` pattern is not checked by `new`."),
        "an unresolved pattern constant must be named as unchecked, got:\n{source}"
    );
    let source = with_pattern(None);
    assert!(
        !source.contains("is not checked by `new`"),
        "a length bound alone leaves nothing unchecked to name, got:\n{source}"
    );
}

/// A literal pattern is checked by `new` under `validate-pattern`, so the
/// type names that condition instead of claiming the pattern goes unchecked
/// outright.
#[test]
fn a_literal_pattern_is_named_as_feature_gated() {
    let source = rust_for(vec![public_decl(
        "Handle",
        v2::decl::Kind::TypeDef(v2::TypeDef {
            backing: Some(v2::Backing {
                kind: Some(v2::backing::Kind::Primitive(
                    v2::PrimitiveType::String as i32,
                )),
            }),
            constraint: Some(v2::Constraint {
                len_min: Some(3),
                len_max: Some(8),
                pattern: Some("/[a-z]+/".to_string()),
                pattern_const: None,
                ..constraint(None, None, None)
            }),
            declared_init: None,
            init: Some(init_value(false, None)),
            width: None,
        }),
    )]);
    assert!(
        !source.contains("The `match` pattern is not checked by `new`."),
        "a literal pattern is checked by `new` under the feature, got:\n{source}"
    );
    // The assertion names the doc line itself. A bare `contains`
    // ("validate-pattern") is satisfied by the `#[cfg(feature =
    // "validate-pattern")]` attribute in the constructor body, so deleting
    // the doc line entirely would leave it green.
    assert!(
        source.contains(
            "/// The `match` pattern is checked by `new` only when the crate is built with the `validate-pattern` feature."
        ),
        "the type must name the condition its pattern guarantee depends on, got:\n{source}"
    );
}

/// A `step` and a literal pattern together are both named: the step is never
/// checked, and the pattern is checked only under `validate-pattern`. The
/// `unchecked_doc` check for `step` reads no backing, so it is unconditional;
/// the backing here is `String` so the pattern's own guard (a check is
/// emitted only for a `String` backing) also applies.
#[test]
fn a_step_and_a_literal_pattern_are_both_named() {
    let source = rust_for(vec![public_decl(
        "Stepped",
        v2::decl::Kind::TypeDef(v2::TypeDef {
            backing: Some(v2::Backing {
                kind: Some(v2::backing::Kind::Primitive(
                    v2::PrimitiveType::String as i32,
                )),
            }),
            constraint: Some(v2::Constraint {
                pattern: Some("/x/".to_string()),
                ..constraint(None, None, Some("0.5"))
            }),
            declared_init: None,
            init: Some(init_value(true, Some("0.0"))),
            width: None,
        }),
    )]);
    assert!(source.contains("/// Quantization (`step`) is not checked by `new`."));
    assert!(!source.contains("The `match` pattern is not checked by `new`."));
    // The doc line itself, not the `cfg` attribute that also carries the
    // feature name.
    assert!(source.contains(
        "/// The `match` pattern is checked by `new` only when the crate is built with the `validate-pattern` feature."
    ));
}

/// The pattern check is emitted only under `validate-pattern`; the range and
/// length checks above it are not gated, since they need no dependency.
#[test]
fn pattern_check_is_feature_gated() {
    let source = rust_for(vec![vin_decl()]);
    assert!(source.contains("#[cfg(feature = \"validate-pattern\")]"));
    assert!(source.contains("::ridl_rt::payload::Rule::Pattern"));
    // Both crate paths are absolute, so an interface named `Std` or `Regex`
    // cannot shadow them from the module the constructor lives in.
    assert!(source.contains("::std::sync::LazyLock"));
    // The regex source is the pattern with its `/` delimiters stripped. The
    // IR stores them (`ridl-sem` keeps the literal as written), and emitting
    // them would compile into a regex that never matches, so every `new`
    // would reject every value. Asserting only on `Regex::new` leaves that
    // undetected, so the argument is pinned too.
    assert!(
        source.contains(r#"::regex::Regex::new("[A-HJ-NPR-Z0-9]{17}")"#),
        "the emitted regex source must have its delimiters stripped, got:\n{source}"
    );
    // The length check is not gated - it needs no dependency. This is the
    // text before the first gate, so the name says ungated, not gated.
    let ungated = source
        .split("#[cfg(feature = \"validate-pattern\")]")
        .next()
        .unwrap();
    assert!(ungated.contains("::ridl_rt::payload::Rule::Length"));
}

/// Outside this test and [`the_generated_pattern_check_runs`], no proof in
/// this repository compiles the `#[cfg(feature = "validate-pattern")]` block:
/// every other `rustc` compile proof, here and in
/// `crates/ridlc/tests/rust_crate_emit.rs`, drives bare `rustc` with no
/// `--cfg` for that feature, so the block is compiled out. `regex` is not a
/// declared dependency of any workspace crate, so neither proof can link the
/// real one; both link [`regex_stub_rlib`], a hand-written stand-in built the
/// same way [`ridl_rt_rlib`] builds `ridl-rt` (see its doc comment for why one
/// `rustc` call is the whole build).
///
/// The two are not redundant. This one uses `vin_decl` and is the only test
/// asserting that the gate attribute is emitted at all, so it alone catches
/// the gate being dropped. [`the_generated_pattern_check_runs`] uses a fixture
/// whose pattern its stand-in can decide, and runs the result, so it catches
/// what compiling cannot see.
#[test]
fn pattern_check_compiles_under_validate_pattern_against_a_regex_stand_in() {
    let source = rust_for(vec![vin_decl()]);
    assert!(
        source.contains("#[cfg(feature = \"validate-pattern\")]"),
        "the fixture must actually exercise the gated block, got:\n{source}"
    );

    let dir = tempfile::tempdir().expect("a temp dir is created");
    let source_path = dir.path().join("pattern_gated.rs");
    std::fs::write(&source_path, &source).expect("the generated source is written");
    let ridl_rt = ridl_rt_rlib(dir.path());
    let regex = regex_stub_rlib(dir.path());

    let status = std::process::Command::new("rustc")
        .args([
            "--edition",
            "2024",
            "--crate-type",
            "lib",
            "--emit",
            "metadata",
        ])
        .arg("-o")
        .arg(dir.path().join("pattern_gated.rmeta"))
        .arg("--cfg")
        .arg(r#"feature="validate-pattern""#)
        .arg("--extern")
        .arg(format!("ridl_rt={}", ridl_rt.display()))
        .arg("--extern")
        .arg(format!("regex={}", regex.display()))
        .arg(&source_path)
        .status()
        .expect("rustc must be installed and runnable for this test to be meaningful");
    assert!(
        status.success(),
        "the validate-pattern block must compile against the regex stand-in, source:\n{source}"
    );
}

/// The pattern check is compiled **and run**, with the feature enabled.
///
/// `pattern_check_compiles_under_validate_pattern_against_a_regex_stand_in`
/// stops at `--emit metadata`, so it type-checks the gated block and never
/// evaluates it. Two things the emitter can get wrong survive that: the
/// polarity of the test (`if !PATTERN.is_match(…)` inverted to
/// `if PATTERN.is_match(…)` still type-checks), and the stripping of the
/// pattern's `/` delimiters (emitting `"/ABC/"` is a valid regex source that
/// simply never matches, so every `new` would reject every value). Both leave
/// every string assertion in this file green, so only an executed assertion
/// catches them.
///
/// The stand-in's `Regex::new` refuses a delimiter-carrying pattern and its
/// `is_match` compares for equality, so `Code::new("ABC")` must be accepted
/// and `Code::new("XYZ")` must be refused with `Rule::Pattern`. Both values
/// are three characters, which is exactly the declared length bound, so the
/// length checks cannot be what decides either case.
#[test]
fn the_generated_pattern_check_runs() {
    let source = format!(
        "{}\n{}",
        rust_for(vec![literal_pattern_decl()]),
        r#"
fn main() {
    // The matching value. If the emitted test were inverted, this would be
    // refused; if the delimiters were left on the pattern, the stand-in's
    // `new` would return `Err` and the `expect` in the generated code would
    // panic before this line.
    match Code::new(String::from("ABC")) {
        Ok(c) => assert_eq!(c.get(), "ABC"),
        Err(v) => panic!("ABC matches the pattern, got {:?}", v.rule),
    }
    // The non-matching value, the same length as the matching one, so the
    // length bounds cannot be what refuses it.
    match Code::new(String::from("XYZ")) {
        Err(v) => assert_eq!(v.rule, ::ridl_rt::payload::Rule::Pattern),
        Ok(_) => panic!("XYZ does not match the pattern"),
    }
    // A value outside the length bound is still refused on length, which
    // proves the ungated checks survive with the feature enabled.
    match Code::new(String::from("ABCD")) {
        Err(v) => assert_eq!(v.rule, ::ridl_rt::payload::Rule::Length),
        Ok(_) => panic!("ABCD is outside the declared length bound"),
    }
}
"#
    );

    let dir = tempfile::tempdir().expect("a temp dir is created");
    let source_path = dir.path().join("pattern_run.rs");
    let bin_path = dir.path().join("pattern_run");
    std::fs::write(&source_path, &source).expect("the generated source is written");
    let ridl_rt = ridl_rt_rlib(dir.path());
    let regex = regex_stub_rlib(dir.path());

    let status = std::process::Command::new("rustc")
        .args(["--edition", "2024", "--crate-type", "bin"])
        .arg("-o")
        .arg(&bin_path)
        .arg("--cfg")
        .arg(r#"feature="validate-pattern""#)
        .arg("--extern")
        .arg(format!("ridl_rt={}", ridl_rt.display()))
        .arg("--extern")
        .arg(format!("regex={}", regex.display()))
        .arg(&source_path)
        .status()
        .expect("rustc must be installed and runnable for this test to be meaningful");
    assert!(
        status.success(),
        "the gated pattern check must compile as a program, source:\n{source}"
    );

    let run = std::process::Command::new(&bin_path)
        .output()
        .expect("the compiled program runs");
    assert!(
        run.status.success(),
        "the generated pattern check must behave as declared, stderr:\n{}",
        String::from_utf8_lossy(&run.stderr)
    );
}

/// A literal pattern on a `bytes` backing emits no pattern check at all: a
/// `Vec<u8>` value does not type-check against `regex::Regex::is_match`
/// (`&str`). The length checks are unaffected.
#[test]
fn bytes_backed_pattern_emits_no_pattern_check() {
    let source = rust_for(vec![bytes_pattern_decl()]);
    assert!(
        !source.contains("::ridl_rt::payload::Rule::Pattern"),
        "a bytes backing must emit no pattern check, got:\n{source}"
    );
    assert!(
        !source.contains("#[cfg(feature = \"validate-pattern\")]"),
        "a bytes backing must emit no feature-gated block, got:\n{source}"
    );
    assert!(
        !source.contains("::regex::"),
        "a bytes backing must name no regex engine, got:\n{source}"
    );
    assert!(
        source.contains("::ridl_rt::payload::Rule::Length"),
        "the length bound is still checked, got:\n{source}"
    );
}

/// The doc for a bytes-backed pattern must not claim the feature-gated
/// guarantee it does not implement: since `constraint_checks` emits no
/// pattern branch for this backing, the type must carry the plain "not
/// checked" line instead.
///
/// This is about an IR the backend did not lower, not about a typl source.
/// A bytes type written with a `match` reaches the backend with no pattern
/// at all, so `unchecked_doc` emits no line for it whatsoever; see
/// [`bytes_pattern_decl`]. The behaviour pinned here is that a pattern
/// arriving on a backing the check cannot cover is described accurately
/// rather than advertised as gated.
#[test]
fn bytes_backed_pattern_is_named_as_unchecked_not_feature_gated() {
    let source = rust_for(vec![bytes_pattern_decl()]);
    assert!(
        source.contains(" The `match` pattern is not checked by `new`."),
        "a bytes-backed pattern must be named plainly unchecked, got:\n{source}"
    );
    assert!(
        !source.contains("validate-pattern"),
        "a bytes-backed pattern must not name the feature it is not gated on, got:\n{source}"
    );
}

/// A deprecated declaration's own impl blocks use the deprecated type, which
/// would draw the `deprecated` lint in the consumer's build about code the
/// consumer did not write.
#[test]
fn a_deprecated_scalar_allows_deprecated_on_its_impls() {
    let source = rust_for(vec![v2::Decl {
        deprecated: Some("use Velocity".to_string()),
        ..speed_decl()
    }]);
    // The inherent impl and the two trait impls. This is a lower bound, not an
    // exact total: `defaults.rs` emits a `Default` impl that still draws the
    // lint, and closing that gap must not fail this test.
    assert!(
        source.matches("#[allow(deprecated)]").count() >= 3,
        "each generated impl of a deprecated type allows the lint, got:\n{source}"
    );
    let plain = rust_for(vec![speed_decl()]);
    assert!(
        !plain.contains("allow(deprecated)"),
        "a type that is not deprecated allows nothing, got:\n{plain}"
    );
}

/// A `boolean` type is vacuous, so it has no `new_unchecked`; its `new` is
/// `const` and infallible, and a constant constructs through that instead
/// (`scalar_ctor`).
#[test]
fn a_boolean_constant_constructs_through_new() {
    let decls = vec![
        public_decl(
            "Flag",
            primitive_type(
                v2::PrimitiveType::Boolean,
                init_value(true, Some("false")),
                None,
            ),
        ),
        public_decl(
            "ENABLED",
            v2::decl::Kind::ConstDef(v2::ConstDef {
                type_ref: Some("Flag".to_string()),
                value: "true".to_string(),
                regex: None,
            }),
        ),
    ];
    let source = rust_for(decls);
    assert!(
        source.contains("pub const ENABLED: Flag = Flag::new(true);"),
        "a boolean constant constructs through new, got:\n{source}"
    );
}

/// A typl type name is CamelCase (typl §15.1) and `ridl-sem` reserves no
/// identifier, so a package may declare `type Result`, `type Ok`, `type Err`,
/// `type TryFrom` or `type From`. Each is a struct in the same module as the
/// generated constructors, where it shadows the prelude name; the constructors
/// must therefore name the five by absolute path. A `rustc` run is the proof:
/// with `pub struct Result(i64);` in scope, an unqualified `Result<Self, _>`
/// fails with E0107.
#[test]
fn prelude_names_declared_by_the_package_compile() {
    let mut decls: Vec<v2::Decl> = ["Result", "Ok", "Err", "TryFrom", "From"]
        .into_iter()
        .map(|name| {
            public_decl(
                name,
                primitive_type(
                    v2::PrimitiveType::Integer,
                    init_value(true, Some("0")),
                    Some(v2::type_def::Width::IntWidth(v2::IntWidth::I32 as i32)),
                ),
            )
        })
        .collect();
    decls.push(speed_decl());
    let rust_source = rust_for(decls);

    let dir = tempfile::tempdir().expect("a temp dir is created");
    let source_path = dir.path().join("prelude_names.rs");
    std::fs::write(&source_path, &rust_source).expect("the generated source is written");
    let rlib = ridl_rt_rlib(dir.path());
    let status = std::process::Command::new("rustc")
        .args([
            "--edition",
            "2024",
            "--crate-type",
            "lib",
            "--emit",
            "metadata",
        ])
        .arg("-o")
        .arg(dir.path().join("prelude_names.rmeta"))
        .arg("--extern")
        .arg(format!("ridl_rt={}", rlib.display()))
        .arg(&source_path)
        .status()
        .expect("rustc must be installed and runnable for this test to be meaningful");
    assert!(
        status.success(),
        "a package declaring the prelude's names must compile, source:\n{rust_source}"
    );
}

#[test]
fn constants_all_forms() {
    let decls = vec![
        speed_decl(),
        public_decl(
            "MAX_SPEED",
            v2::decl::Kind::ConstDef(v2::ConstDef {
                type_ref: Some("Speed".to_string()),
                value: "250.0".to_string(),
                regex: None,
            }),
        ),
        public_decl(
            "MAX_GEAR",
            v2::decl::Kind::ConstDef(v2::ConstDef {
                type_ref: Some("integer".to_string()),
                value: "6".to_string(),
                regex: None,
            }),
        ),
        public_decl(
            "Greeting",
            primitive_type(v2::PrimitiveType::String, init_value(true, Some("")), None),
        ),
        public_decl(
            "BANNER",
            v2::decl::Kind::ConstDef(v2::ConstDef {
                type_ref: Some("Greeting".to_string()),
                value: "hello".to_string(),
                regex: None,
            }),
        ),
        public_decl(
            "VIN_PATTERN",
            v2::decl::Kind::ConstDef(v2::ConstDef {
                type_ref: None,
                value: String::new(),
                regex: Some("^[A-HJ-NPR-Z0-9]{17}$".to_string()),
            }),
        ),
    ];
    insta::assert_snapshot!(rust_for(decls));
}

#[test]
fn struct_with_optional_and_reserved() {
    // A struct that mixes a normal field, a reserved tombstone (occupies an
    // ordinal, emits no field), and an optional field.
    let struct_def = v2::StructDef {
        members: vec![
            field_member(named_field(
                "name",
                1,
                "Label",
                false,
                init_value(false, None),
            )),
            reserved_member(2, "legacyChecksum"),
            field_member(named_field(
                "speed",
                3,
                "Speed",
                false,
                init_value(true, None),
            )),
            field_member(named_field(
                "override",
                4,
                "Speed",
                true,
                init_value(true, None),
            )),
        ],
        fixed_layout: false,
    };
    let decls = vec![
        speed_decl(),
        public_decl(
            "Label",
            // A match-constrained string is not derivable.
            bounded_string_type(init_value(false, None)),
        ),
        public_decl("DriverProfile", v2::decl::Kind::StructDef(struct_def)),
    ];
    insta::assert_snapshot!(rust_for(decls));
}

/// One struct with one named field, for the name-projection tests.
fn struct_with_field(name: &str, field_name: &str, type_ref: &str) -> v2::Decl {
    public_decl(
        name,
        v2::decl::Kind::StructDef(v2::StructDef {
            members: vec![field_member(named_field(
                field_name,
                1,
                type_ref,
                false,
                init_value(true, None),
            ))],
            fixed_layout: false,
        }),
    )
}

/// typl §15.1 makes field names camelCase, so an untransformed name draws
/// `non_snake_case` at every consumer of the generated module. The field name
/// goes through the pinned transform (ADR-0016 decisions 1 and 2).
///
/// Two sites project the name, and the assertions discriminate them: the
/// struct declaration in `emit_field`, and the `Default` initializer in
/// `struct_default`. Projecting one and not the other is worse than
/// projecting neither — the initializer would name a field the struct does
/// not have, which is E0560 — so the negative assertion is that the written
/// name appears nowhere at all, rather than that one particular line is
/// absent.
#[test]
fn a_struct_field_name_is_projected_to_snake_case() {
    let source = rust_for(vec![
        speed_decl(),
        struct_with_field("Reading", "sensorId", "Speed"),
    ]);
    assert!(source.contains("pub sensor_id:"), "got:\n{source}");
    assert!(
        source.contains("sensor_id: Speed::default()"),
        "the `Default` initializer must name the projected field, got:\n{source}"
    );
    assert!(
        !source.contains("sensorId"),
        "the written name must reach no generated site, got:\n{source}"
    );
}

#[test]
fn enum_with_discriminants() {
    let decls = vec![public_decl(
        "GearPosition",
        v2::decl::Kind::EnumDef(v2::EnumDef {
            values: vec![
                enum_value("PARK", 0),
                enum_value("DRIVE", 1),
                enum_value("REVERSE", 2),
                enum_value("NEUTRAL", 3),
            ],
            reserved: Vec::new(),
        }),
    )];
    insta::assert_snapshot!(rust_for(decls));
}

#[test]
fn enumset_standalone_form() {
    let decls = vec![public_decl(
        "WarningFlags",
        v2::decl::Kind::EnumSetDef(v2::EnumSetDef {
            backing_enum: None,
            bits: warning_bits(),
            width: v2::IntWidth::U8 as i32,
        }),
    )];
    insta::assert_snapshot!(rust_for(decls));
}

#[test]
fn enum_converts_from_a_raw_discriminant() {
    let source = rust_for(vec![gear_position_decl()]);
    assert!(source.contains("impl ::core::convert::TryFrom<i64> for GearPosition"));
    assert!(source.contains("impl ::core::convert::From<GearPosition> for i64"));
    assert!(source.contains("::ridl_rt::payload::Rule::Variant"));
    // Each arm carries the declared discriminant, not the variant's position.
    // Asserting one arm of the gap is what distinguishes the two: over a
    // fixture numbered contiguously from zero, arms built from the position
    // emit identical source.
    assert!(
        source.contains("7 => ::core::result::Result::Ok(Self::REVERSE)"),
        "the arm maps the declared discriminant, got:\n{source}"
    );
}

#[test]
fn enum_set_rejects_bits_outside_the_declared_mask() {
    let source = rust_for(vec![features_decl()]);
    assert!(source.contains("impl ::core::convert::TryFrom<i64> for Features"));
    // The declared bits are positions 0 to 3, so the mask is 0b1111. The
    // value is asserted rather than the constant's presence: a fold that
    // ORed the bit positions instead of shifting by them would still emit a
    // `DECLARED_MASK`, and would still pass a presence check.
    assert!(
        source.contains("const DECLARED_MASK: i64 = 15"),
        "the mask is the union of the declared bits, got:\n{source}"
    );
}

#[test]
fn a_bit_position_outside_the_int64_domain_does_not_panic_codegen() {
    // `ridl-sem` reports TYPL-111 for a position outside 0..=63 and still
    // carries the bit into the IR, so codegen is handed one. Folding it into
    // the mask would shift by 64 and panic in a debug build; codegen is
    // total, so the bit contributes nothing and the checker's diagnostic is
    // what reports it.
    let source = rust_for(vec![public_decl(
        "Odd",
        v2::decl::Kind::EnumSetDef(v2::EnumSetDef {
            backing_enum: None,
            bits: vec![
                enum_value("IN_DOMAIN", 1),
                enum_value("TOO_HIGH", 64),
                enum_value("NEGATIVE", -1),
            ],
            width: v2::IntWidth::U8 as i32,
        }),
    )]);
    assert!(
        source.contains("const DECLARED_MASK: i64 = 2"),
        "only the in-domain bit reaches the mask, got:\n{source}"
    );
}

#[test]
fn the_highest_declared_bit_is_in_domain() {
    // Bit 63 is the highest position the int64 domain admits (typl §9.3), so
    // it belongs in the mask. The domain guard is an inclusive range for this
    // reason: an exclusive one would drop a declared bit, and `TryFrom` would
    // then refuse a value that is in contract.
    let source = rust_for(vec![public_decl(
        "Wide",
        v2::decl::Kind::EnumSetDef(v2::EnumSetDef {
            backing_enum: None,
            bits: vec![enum_value("TOP", 63)],
            width: v2::IntWidth::U64 as i32,
        }),
    )]);
    assert!(
        source.contains("const DECLARED_MASK: i64 = -9223372036854775808"),
        "bit 63 is in the mask, got:\n{source}"
    );
}

/// The `#[allow(deprecated)]` branch of `emit_enum` and `emit_enum_set`.
///
/// [`a_deprecated_scalar_allows_deprecated_on_its_impls`] covers
/// `emit_type_def` only. A deprecated enum or enum set reaches neither of the
/// two emitters this exercises, so without this test both branches could be
/// replaced by an empty token stream with nothing failing.
#[test]
fn a_deprecated_enum_and_enum_set_allow_deprecated_on_their_impls() {
    let source = rust_for(vec![
        v2::Decl {
            deprecated: Some("use Gear".to_string()),
            ..gear_position_decl()
        },
        v2::Decl {
            deprecated: Some("use Flags".to_string()),
            ..features_decl()
        },
    ]);
    // The enum's two trait impls, and the enum set's inherent block and two
    // trait impls. A lower bound, not an exact total: `defaults.rs` emits a
    // `Default` impl that still draws the lint, and closing that gap must not
    // fail this test.
    assert!(
        source.matches("#[allow(deprecated)]").count() >= 5,
        "each generated impl of a deprecated enum or enum set allows the lint, got:\n{source}"
    );
    let plain = rust_for(vec![gear_position_decl(), features_decl()]);
    assert!(
        !plain.contains("allow(deprecated)"),
        "declarations that are not deprecated allow nothing, got:\n{plain}"
    );
}

#[test]
fn an_internal_enum_set_keeps_its_visibility_on_every_generated_item() {
    // The mask and the accessor carry the declaration's visibility, as the
    // bit constants do.
    //
    // No lint catches a literal `pub` here. An associated item's effective
    // visibility is capped by the impl's self type, so `pub const` inside an
    // inherent impl of a `pub(crate)` type is accepted in silence — checked
    // with rustc under `-D private-interfaces -D private-bounds`, which exits
    // 0. `private_interfaces` fires on a public field or signature that
    // exposes a private type, which is the driftsys/ridl#161 shape and not
    // this one. This assertion is the only thing that observes the
    // visibility, so do not weaken it on the assumption that the corpus
    // proof's lint flags would catch a regression.
    let source = rust_for(vec![v2::Decl {
        visibility: v2::Visibility::Internal as i32,
        ..features_decl()
    }]);
    assert!(
        source.contains("pub(crate) const DECLARED_MASK"),
        "the mask carries the declaration's visibility, got:\n{source}"
    );
    assert!(
        source.contains("pub(crate) const fn get"),
        "the accessor carries the declaration's visibility, got:\n{source}"
    );
    assert!(
        !source.contains("pub const "),
        "no generated item of an internal enum set is public, got:\n{source}"
    );
}

#[test]
fn enumset_derived_form() {
    // The derived form carries a backing enum name; the checker copies the
    // backing enum's values into `bits`, so the emitted Rust is identical to
    // the standalone form.
    let decls = vec![
        public_decl(
            "Warning",
            v2::decl::Kind::EnumDef(v2::EnumDef {
                values: warning_bits(),
                reserved: Vec::new(),
            }),
        ),
        public_decl(
            "WarningFlags",
            v2::decl::Kind::EnumSetDef(v2::EnumSetDef {
                backing_enum: Some("Warning".to_string()),
                bits: warning_bits(),
                width: v2::IntWidth::U8 as i32,
            }),
        ),
    ];
    insta::assert_snapshot!(rust_for(decls));
}

#[test]
fn result_union() {
    let reading = v2::StructDef {
        members: vec![field_member(named_field(
            "value",
            1,
            "Speed",
            false,
            init_value(true, None),
        ))],
        fixed_layout: true,
    };
    let fault = v2::StructDef {
        members: vec![field_member(named_field(
            "code",
            1,
            "Counter",
            false,
            init_value(true, None),
        ))],
        fixed_layout: true,
    };
    let union = v2::UnionDef {
        arms: vec![
            v2::UnionArm {
                name: "ok".to_string(),
                ordinal: 1,
                type_ref: "SensorReading".to_string(),
                doc: "Successful reading".to_string(),
            },
            v2::UnionArm {
                name: "err".to_string(),
                ordinal: 2,
                type_ref: "SensorFault".to_string(),
                doc: String::new(),
            },
        ],
        is_result: true,
        reserved: Vec::new(),
    };
    let decls = vec![
        speed_decl(),
        counter_decl(),
        public_decl("SensorReading", v2::decl::Kind::StructDef(reading)),
        v2::Decl {
            is_error: true,
            ..public_decl("SensorFault", v2::decl::Kind::StructDef(fault))
        },
        public_decl("SensorResult", v2::decl::Kind::UnionDef(union)),
    ];
    insta::assert_snapshot!(rust_for(decls));
}

fn tuple_field(name: &str, type_ref: &str) -> v2::TupleField {
    v2::TupleField {
        name: name.to_string(),
        r#type: Some(v2::FieldType {
            optional: false,
            kind: Some(v2::field_type::Kind::Named(type_ref.to_string())),
        }),
    }
}

#[test]
fn tuple_field_generates_named_struct() {
    let range = v2::Field {
        r#type: Some(v2::FieldType {
            optional: false,
            kind: Some(v2::field_type::Kind::Tuple(v2::TupleType {
                fields: vec![tuple_field("min", "Speed"), tuple_field("max", "Speed")],
            })),
        }),
        ..named_field("range", 1, "", false, init_value(true, None))
    };
    let struct_def = v2::StructDef {
        members: vec![field_member(range)],
        fixed_layout: false,
    };
    let decls = vec![
        speed_decl(),
        public_decl("SensorBounds", v2::decl::Kind::StructDef(struct_def)),
    ];
    insta::assert_snapshot!(rust_for(decls));
}

/// A tuple field name is camelCase in typl exactly as a struct field name is
/// (typl §15.1), and the struct a tuple generates carries it as a Rust field
/// name, so it goes through the same pinned transform (ADR-0016 decisions 1
/// and 2).
///
/// Two sites project it, and the assertions discriminate them: the generated
/// struct in `emit_tuple_struct`, and the `Default` initializer in
/// `tuple_default_expr`. Projecting one and not the other is E0560, so the
/// negative assertion is that the written name reaches no generated site.
///
/// Two tuple field names distinct in typl can still project to one Rust field
/// name. That is unchecked, and rustc rejects the result with E0124 —
/// driftsys/ridl#449.
#[test]
fn a_tuple_field_name_is_projected_to_snake_case() {
    let bounds = shaped_field("range", 1, tuple_of(&[("minSpeed", "Speed")]));
    let decls = vec![
        speed_decl(),
        public_decl(
            "SensorBounds",
            v2::decl::Kind::StructDef(v2::StructDef {
                members: vec![field_member(bounds)],
                fixed_layout: false,
            }),
        ),
    ];
    let source = rust_for(decls);
    assert!(source.contains("pub min_speed:"), "got:\n{source}");
    assert!(
        source.contains("min_speed: Speed::default()"),
        "the `Default` initializer must name the projected field, got:\n{source}"
    );
    assert!(
        !source.contains("minSpeed"),
        "the written name must reach no generated site, got:\n{source}"
    );
}

/// A struct field whose type is `kind`, at `ordinal`.
fn shaped_field(name: &str, ordinal: u32, kind: v2::field_type::Kind) -> v2::Field {
    v2::Field {
        ordinal,
        r#type: Some(v2::FieldType {
            optional: false,
            kind: Some(kind),
        }),
        ..named_field(name, ordinal, "", false, init_value(true, None))
    }
}

/// A tuple type whose fields are the given `(name, type_ref)` pairs.
fn tuple_of(fields: &[(&str, &str)]) -> v2::field_type::Kind {
    v2::field_type::Kind::Tuple(v2::TupleType {
        fields: fields
            .iter()
            .map(|(name, type_ref)| tuple_field(name, type_ref))
            .collect(),
    })
}

/// A tuple under an `internal` declaration generates a **`pub(crate)`** struct,
/// in every typl position that can reach one, and the struct compiles under
/// `-D private-interfaces` (issue #167).
///
/// A tuple is anonymous in source, so the struct it generates has no visibility
/// of its own to declare; before this fix it was fixed at `pub`, which put a
/// `pub(crate)` payload type into a `pub` struct's field. `ridlc check` accepts
/// that source — an `internal` declaration may name `internal` declarations
/// freely (typl §3.3) — so the generated module was the only thing wrong with
/// it, and only rustc could say so.
///
/// **All five typl positions are exercised, not one.** Issue #161 was reported
/// as two positions and turned out to be twenty; a fix verified on the direct
/// field alone would leave the array element, the optional, the map value and
/// the nested tuple emitting `pub`, and every one of them reaches
/// `field_type_tokens` by its own branch. The nested tuple is the one that
/// cannot be found during the declaration walk at all: it is discovered only
/// while emitting the outer tuple's fields, from the flat worklist, which is
/// exactly where the inducing declaration is out of reach.
///
/// The `rustc` run is the proof rather than the string assertions: a snapshot
/// records what was emitted, not that it is valid Rust, and this defect
/// produced output that a plain `rustc` invocation accepts with a warning.
#[test]
fn a_tuple_under_an_internal_declaration_is_package_private() {
    let hidden = v2::Decl {
        visibility: v2::Visibility::Internal as i32,
        ..public_decl(
            "Hidden",
            primitive_type(
                v2::PrimitiveType::Integer,
                init_value(true, Some("0")),
                Some(v2::type_def::Width::IntWidth(v2::IntWidth::U16 as i32)),
            ),
        )
    };
    let holder = v2::StructDef {
        members: vec![
            // 1. a direct tuple field
            field_member(shaped_field("direct", 1, tuple_of(&[("a", "Hidden")]))),
            // 2. a tuple as an array element
            field_member(shaped_field(
                "arr",
                2,
                v2::field_type::Kind::Array(Box::new(v2::ArrayType {
                    element: Some(Box::new(v2::FieldType {
                        optional: false,
                        kind: Some(tuple_of(&[("b", "Hidden")])),
                    })),
                    min: 3,
                    max: 3,
                })),
            )),
            // 3. an optional tuple field
            field_member(v2::Field {
                r#type: Some(v2::FieldType {
                    optional: true,
                    kind: Some(tuple_of(&[("c", "Hidden")])),
                }),
                ..shaped_field("opt", 3, tuple_of(&[("c", "Hidden")]))
            }),
            // 4. a tuple as a map value
            field_member(shaped_field(
                "mp",
                4,
                v2::field_type::Kind::Map(Box::new(v2::MapType {
                    key: Some(Box::new(v2::FieldType {
                        optional: false,
                        // A bare `string` map key is lowered with typl
                        // §4.4's `[0..256]` default (driftsys/ridl#459), so
                        // it reaches a backend as an inline scalar.
                        kind: Some(v2::field_type::Kind::InlineScalar(Box::new(v2::TypeDef {
                            backing: Some(v2::Backing {
                                kind: Some(v2::backing::Kind::Primitive(
                                    v2::PrimitiveType::String as i32,
                                )),
                            }),
                            constraint: Some(v2::Constraint {
                                len_max: Some(256),
                                ..Default::default()
                            }),
                            declared_init: None,
                            init: None,
                            width: None,
                        }))),
                    })),
                    value: Some(Box::new(v2::FieldType {
                        optional: false,
                        kind: Some(tuple_of(&[("d", "Hidden")])),
                    })),
                    min: 0,
                    max: 4,
                })),
            )),
            // 5. a tuple nested inside a tuple — reached only by draining the
            //    worklist, after the declaration walk has finished.
            field_member(shaped_field(
                "nested",
                5,
                v2::field_type::Kind::Tuple(v2::TupleType {
                    fields: vec![v2::TupleField {
                        name: "e".to_string(),
                        r#type: Some(v2::FieldType {
                            optional: false,
                            kind: Some(tuple_of(&[("f", "Hidden")])),
                        }),
                    }],
                }),
            )),
        ],
        fixed_layout: false,
    };
    // The regression direction, in the same module: a public struct's tuple
    // stays `pub`. Its payload is the public `Counter`, because a public
    // declaration naming an `internal` one is TYPL-005 and never reaches a
    // backend.
    let shown = v2::StructDef {
        members: vec![field_member(shaped_field(
            "direct",
            1,
            tuple_of(&[("g", "Counter")]),
        ))],
        fixed_layout: false,
    };

    let rust_source = rust_for(vec![
        counter_decl(),
        hidden,
        v2::Decl {
            visibility: v2::Visibility::Internal as i32,
            ..public_decl("Holder", v2::decl::Kind::StructDef(holder))
        },
        public_decl("Shown", v2::decl::Kind::StructDef(shown)),
    ]);

    for item in [
        "pub(crate) struct HolderDirect",
        "pub(crate) struct HolderArrElement",
        "pub(crate) struct HolderOpt",
        "pub(crate) struct HolderMpValue",
        "pub(crate) struct HolderNested",
        "pub(crate) struct HolderNestedE",
    ] {
        assert!(
            rust_source.contains(item),
            "a tuple under an `internal` declaration must emit `{item}`, got:\n{rust_source}"
        );
    }
    // `pub(crate) struct X` does not contain `pub struct X`, so these are
    // genuine negatives rather than restatements of the loop above.
    for leaked in [
        "pub struct HolderDirect",
        "pub struct HolderArrElement",
        "pub struct HolderOpt",
        "pub struct HolderMpValue",
        "pub struct HolderNested",
        "pub struct HolderNestedE",
    ] {
        assert!(
            !rust_source.contains(leaked),
            "a tuple under an `internal` declaration must not emit `{leaked}`, \
             got:\n{rust_source}"
        );
    }
    assert!(
        rust_source.contains("pub struct ShownDirect"),
        "a public declaration's tuple must still be `pub`, got:\n{rust_source}"
    );

    // The proof. `private-interfaces` and `private-bounds` are denied by name
    // rather than with a blanket `-D warnings`, matching `rustc_accepts` in
    // `crates/ridlc/tests/corpus.rs`: the generated code carries by-design
    // naming and dead-code lints that say nothing about visibility.
    //
    // `non_snake_case` is denied beside them so that a field name reaching
    // generated Rust verbatim fails this run rather than warning in it. It is
    // inert on this fixture, whose every field name is a single word: the
    // proof that guards issue #243 is `appendix_a_compiles_with_rustc`, whose
    // IR carries `sensorId` and `isOpen`. The deny here is what makes this
    // proof stay a proof if a multi-word field name is ever added to the
    // fixture. An enum variant keeps its typl `SCREAMING_SNAKE` spelling and
    // draws `non_camel_case_types`, a different lint, which stays undenied.
    let dir = tempfile::tempdir().expect("a temp dir is created");
    let source_path = dir.path().join("internal_tuple.rs");
    std::fs::write(&source_path, &rust_source).expect("the generated source is written");
    let rlib = ridl_rt_rlib(dir.path());
    let status = std::process::Command::new("rustc")
        .args([
            "--edition",
            "2024",
            "--crate-type",
            "lib",
            "--emit",
            "metadata",
            "-D",
            "private-interfaces",
            "-D",
            "private-bounds",
            "-D",
            "non_snake_case",
        ])
        .arg("-o")
        .arg(dir.path().join("internal_tuple.rmeta"))
        .arg("--extern")
        .arg(format!("ridl_rt={}", rlib.display()))
        .arg(&source_path)
        .status()
        .expect("rustc must be installed and runnable for this test to be meaningful");
    assert!(
        status.success(),
        "a tuple under an `internal` declaration must compile under \
         `-D private-interfaces`, source:\n{rust_source}"
    );
}

fn array_field(name: &str, element: &str, min: u64, max: u64) -> v2::Field {
    v2::Field {
        r#type: Some(v2::FieldType {
            optional: false,
            kind: Some(v2::field_type::Kind::Array(Box::new(v2::ArrayType {
                element: Some(Box::new(v2::FieldType {
                    optional: false,
                    kind: Some(v2::field_type::Kind::Named(element.to_string())),
                })),
                min,
                max,
            }))),
        }),
        ..named_field(name, 1, "", false, init_value(true, None))
    }
}

#[test]
fn fixed_and_bounded_arrays() {
    let struct_def = v2::StructDef {
        members: vec![
            field_member(array_field("readings", "Speed", 8, 8)),
            field_member(v2::Field {
                ordinal: 2,
                ..array_field("history", "Speed", 2, 8)
            }),
        ],
        fixed_layout: false,
    };
    let decls = vec![
        speed_decl(),
        public_decl("Samples", v2::decl::Kind::StructDef(struct_def)),
    ];
    insta::assert_snapshot!(rust_for(decls));
}

#[test]
fn bounded_map() {
    let map_field = v2::Field {
        r#type: Some(v2::FieldType {
            optional: false,
            kind: Some(v2::field_type::Kind::Map(Box::new(v2::MapType {
                key: Some(Box::new(v2::FieldType {
                    optional: false,
                    kind: Some(v2::field_type::Kind::Named("Counter".to_string())),
                })),
                value: Some(Box::new(v2::FieldType {
                    optional: false,
                    kind: Some(v2::field_type::Kind::Named("Speed".to_string())),
                })),
                min: 0,
                max: 32,
            }))),
        }),
        ..named_field("meta", 1, "", false, init_value(true, None))
    };
    let struct_def = v2::StructDef {
        members: vec![field_member(map_field)],
        fixed_layout: false,
    };
    let decls = vec![
        speed_decl(),
        counter_decl(),
        public_decl("Table", v2::decl::Kind::StructDef(struct_def)),
    ];
    insta::assert_snapshot!(rust_for(decls));
}

#[test]
fn deprecated_and_internal_visibility() {
    let decls = vec![
        v2::Decl {
            deprecated: Some("use Velocity".to_string()),
            ..speed_decl()
        },
        v2::Decl {
            deprecated: Some(String::new()),
            ..public_decl(
                "OldFlag",
                primitive_type(
                    v2::PrimitiveType::Boolean,
                    init_value(true, Some("false")),
                    None,
                ),
            )
        },
        v2::Decl {
            visibility: v2::Visibility::Internal as i32,
            ..public_decl(
                "RawTicks",
                primitive_type(
                    v2::PrimitiveType::Integer,
                    init_value(true, Some("0")),
                    Some(v2::type_def::Width::IntWidth(v2::IntWidth::U32 as i32)),
                ),
            )
        },
    ];
    insta::assert_snapshot!(rust_for(decls));
}

// ---------------------------------------------------------------------------
// Default derivation — the leaf-recursion rule.
// ---------------------------------------------------------------------------

#[test]
fn derivable_scalar_gets_default_with_derived_value() {
    // A numeric type whose range excludes 0 defaults to `min`, not 0.
    let ranged = v2::decl::Kind::TypeDef(v2::TypeDef {
        backing: Some(v2::Backing {
            kind: Some(v2::backing::Kind::Unit("Cel".to_string())),
        }),
        constraint: Some(constraint(Some("10.0"), Some("40.0"), Some("0.5"))),
        declared_init: None,
        init: Some(init_value(true, Some("10.0"))),
        width: Some(v2::type_def::Width::FloatWidth(v2::FloatWidth::F32 as i32)),
    });
    let source = rust_for(vec![public_decl("Warm", ranged)]);
    assert!(
        source.contains("impl Default for Warm"),
        "a derivable scalar must get a Default impl, got:\n{source}"
    );
    assert!(
        source.contains("Warm::new_unchecked(10.0)"),
        "the derived init must be the range minimum 10.0, got:\n{source}"
    );
}

#[test]
fn leaf_recursion_denies_default_through_a_composite_field() {
    // The trap: `Outer { inner: Inner }` where `Inner` holds a non-derivable
    // leaf. T15 marks `Outer.inner.init.derivable == true` (a one-level flag),
    // yet neither `Inner` nor `Outer` is Default-constructible, so neither may
    // receive an `impl Default`.
    let inner = v2::StructDef {
        members: vec![field_member(named_field(
            "pattern",
            1,
            "Vin",
            false,
            // A match-constrained string leaf: not derivable.
            init_value(false, None),
        ))],
        fixed_layout: false,
    };
    let outer = v2::StructDef {
        members: vec![field_member(named_field(
            "inner",
            1,
            "Inner",
            false,
            // The one-level composite flag reads derivable despite the leaf.
            init_value(true, None),
        ))],
        fixed_layout: false,
    };
    let decls = vec![
        public_decl("Vin", bounded_string_type(init_value(false, None))),
        public_decl("Inner", v2::decl::Kind::StructDef(inner)),
        public_decl("Outer", v2::decl::Kind::StructDef(outer)),
    ];
    let source = rust_for(decls);
    assert!(
        !source.contains("impl Default for Inner"),
        "Inner has a non-derivable leaf; it must not get a Default, got:\n{source}"
    );
    assert!(
        !source.contains("impl Default for Outer"),
        "Outer transitively contains a non-derivable leaf; it must not get a Default despite the one-level flag, got:\n{source}"
    );
}

#[test]
fn enumset_default_is_the_empty_set() {
    let source = rust_for(vec![public_decl(
        "WarningFlags",
        v2::decl::Kind::EnumSetDef(v2::EnumSetDef {
            backing_enum: None,
            bits: warning_bits(),
            width: v2::IntWidth::U8 as i32,
        }),
    )]);
    assert!(
        source.contains("WarningFlags(0)"),
        "the enumset default must be the empty-set sentinel 0, got:\n{source}"
    );
}

#[test]
fn enum_default_prefers_the_zero_discriminant() {
    // Values start at 5; none is 0, so the default is the lowest declared.
    let source = rust_for(vec![public_decl(
        "Mode",
        v2::decl::Kind::EnumDef(v2::EnumDef {
            values: vec![enum_value("SLOW", 5), enum_value("FAST", 9)],
            reserved: Vec::new(),
        }),
    )]);
    assert!(
        source.contains("Mode::SLOW"),
        "with no zero value the default is the lowest discriminant, got:\n{source}"
    );
}

// ---------------------------------------------------------------------------
// Appendix B — the full typl example.
// ---------------------------------------------------------------------------

/// The typl reference Appendix B package `veh.common`, built as IR v2 with
/// populated init values (T15). Cross-package references to `ridl.std` are
/// fully qualified, as the resolver leaves them (typl §3.2).
#[allow(clippy::vec_init_then_push)]
fn appendix_b() -> v2::Package {
    let mut decls = Vec::new();

    // ---- Units and scalars ----
    decls.push(v2::Decl {
        doc: "Vehicle speed over ground".to_string(),
        ..public_decl("Speed", unit_type("km/h", "0.0", "250.0", "0.5", "0.0"))
    });
    decls.push(v2::Decl {
        doc: "Coolant / ambient temperature".to_string(),
        ..public_decl(
            "Temperature",
            unit_type("Cel", "-40.0", "125.0", "0.1", "0.0"),
        )
    });
    decls.push(v2::Decl {
        doc: "Engine crankshaft speed".to_string(),
        ..public_decl("RPM", unit_type("/min", "0.0", "8000.0", "10.0", "0.0"))
    });
    decls.push(v2::Decl {
        doc: "Normalised ratio".to_string(),
        ..public_decl("Ratio", unit_type("%", "0.0", "100.0", "0.1", "0.0"))
    });
    decls.push(counter_decl());
    decls.push(public_decl(
        "Gain",
        v2::decl::Kind::TypeDef(v2::TypeDef {
            backing: Some(v2::Backing {
                kind: Some(v2::backing::Kind::Primitive(
                    v2::PrimitiveType::Float as i32,
                )),
            }),
            constraint: Some(constraint(Some("0.0"), Some("1.0"), Some("0.01"))),
            declared_init: None,
            init: Some(init_value(true, Some("0.0"))),
            width: Some(v2::type_def::Width::FloatWidth(v2::FloatWidth::F32 as i32)),
        }),
    ));

    // ---- Constants ----
    for (name, type_ref, value) in [
        ("MAX_SPEED", "Speed", "250.0"),
        ("SPEED_LIMIT_EU", "Speed", "130.0"),
        ("IDLE_RPM", "RPM", "800.0"),
    ] {
        decls.push(public_decl(
            name,
            v2::decl::Kind::ConstDef(v2::ConstDef {
                type_ref: Some(type_ref.to_string()),
                value: value.to_string(),
                regex: None,
            }),
        ));
    }
    decls.push(public_decl(
        "MAX_GEAR",
        v2::decl::Kind::ConstDef(v2::ConstDef {
            type_ref: Some("integer".to_string()),
            value: "6".to_string(),
            regex: None,
        }),
    ));

    // ---- Enums ----
    decls.push(public_decl(
        "GearPosition",
        v2::decl::Kind::EnumDef(v2::EnumDef {
            values: vec![
                enum_value("PARK", 0),
                enum_value("DRIVE", 1),
                enum_value("REVERSE", 2),
                enum_value("NEUTRAL", 3),
            ],
            reserved: Vec::new(),
        }),
    ));
    decls.push(public_decl(
        "Warning",
        v2::decl::Kind::EnumDef(v2::EnumDef {
            values: warning_bits(),
            reserved: Vec::new(),
        }),
    ));
    decls.push(public_decl(
        "WarningFlags",
        v2::decl::Kind::EnumSetDef(v2::EnumSetDef {
            backing_enum: Some("Warning".to_string()),
            bits: warning_bits(),
            width: v2::IntWidth::U8 as i32,
        }),
    ));

    // ---- Composites ----
    decls.push(public_decl(
        "SpeedLimitPayload",
        v2::decl::Kind::StructDef(v2::StructDef {
            members: vec![
                field_member(named_field(
                    "limit",
                    1,
                    "Speed",
                    false,
                    init_value(true, None),
                )),
                field_member(named_field(
                    "actual",
                    2,
                    "Speed",
                    false,
                    init_value(true, None),
                )),
            ],
            fixed_layout: true,
        }),
    ));

    // gears : integer [0..MAX_GEAR] = 6 (inline scalar with declared init)
    let gears = v2::Field {
        r#type: Some(v2::FieldType {
            optional: false,
            kind: Some(v2::field_type::Kind::InlineScalar(Box::new(v2::TypeDef {
                backing: Some(v2::Backing {
                    kind: Some(v2::backing::Kind::Primitive(
                        v2::PrimitiveType::Integer as i32,
                    )),
                }),
                constraint: Some(constraint(Some("0"), Some("6"), None)),
                declared_init: None,
                init: None,
                width: Some(v2::type_def::Width::IntWidth(v2::IntWidth::U8 as i32)),
            }))),
        }),
        declared_init: Some("6".to_string()),
        ..named_field("gears", 4, "", false, init_value(true, Some("6")))
    };
    decls.push(public_decl(
        "DriverProfile",
        v2::decl::Kind::StructDef(v2::StructDef {
            members: vec![
                // name : Name — ridl.std, string+match, not derivable.
                field_member(named_field(
                    "name",
                    1,
                    "ridl.std.Name",
                    false,
                    init_value(false, None),
                )),
                field_member(named_field(
                    "speed",
                    2,
                    "Speed",
                    false,
                    init_value(true, None),
                )),
                field_member(named_field(
                    "override",
                    3,
                    "Speed",
                    true,
                    init_value(true, None),
                )),
                field_member(gears),
            ],
            fixed_layout: false,
        }),
    ));

    // SensorBounds
    let range = v2::Field {
        r#type: Some(v2::FieldType {
            optional: false,
            kind: Some(v2::field_type::Kind::Tuple(v2::TupleType {
                fields: vec![tuple_field("min", "Speed"), tuple_field("max", "Speed")],
            })),
        }),
        ..named_field("range", 1, "", false, init_value(true, None))
    };
    let readings = v2::Field {
        ordinal: 2,
        ..array_field("readings", "Speed", 8, 8)
    };
    // labels : [Label; 1..16] — Label is ridl.std, not derivable; min 1 makes
    // the field non-derivable.
    let labels = v2::Field {
        r#type: Some(v2::FieldType {
            optional: false,
            kind: Some(v2::field_type::Kind::Array(Box::new(v2::ArrayType {
                element: Some(Box::new(v2::FieldType {
                    optional: false,
                    kind: Some(v2::field_type::Kind::Named("ridl.std.Label".to_string())),
                })),
                min: 1,
                max: 16,
            }))),
        }),
        ..named_field("labels", 3, "", false, init_value(false, None))
    };
    let meta = v2::Field {
        r#type: Some(v2::FieldType {
            optional: false,
            kind: Some(v2::field_type::Kind::Map(Box::new(v2::MapType {
                key: Some(Box::new(v2::FieldType {
                    optional: false,
                    kind: Some(v2::field_type::Kind::Named("ridl.std.Label".to_string())),
                })),
                value: Some(Box::new(v2::FieldType {
                    optional: false,
                    kind: Some(v2::field_type::Kind::Named("ridl.std.Name".to_string())),
                })),
                min: 0,
                max: 32,
            }))),
        }),
        ..named_field("meta", 4, "", false, init_value(true, None))
    };
    decls.push(public_decl(
        "SensorBounds",
        v2::decl::Kind::StructDef(v2::StructDef {
            members: vec![
                field_member(range),
                field_member(readings),
                field_member(labels),
                field_member(meta),
            ],
            fixed_layout: false,
        }),
    ));

    decls.push(public_decl(
        "SensorResult",
        v2::decl::Kind::UnionDef(v2::UnionDef {
            arms: vec![
                v2::UnionArm {
                    name: "ok".to_string(),
                    ordinal: 1,
                    type_ref: "SensorReading".to_string(),
                    doc: String::new(),
                },
                v2::UnionArm {
                    name: "err".to_string(),
                    ordinal: 2,
                    type_ref: "SensorFault".to_string(),
                    doc: String::new(),
                },
            ],
            is_result: true,
            reserved: Vec::new(),
        }),
    ));

    decls.push(public_decl(
        "SensorReading",
        v2::decl::Kind::StructDef(v2::StructDef {
            members: vec![
                field_member(named_field(
                    "value",
                    1,
                    "Speed",
                    false,
                    init_value(true, None),
                )),
                // timestamp : Timestamp — ridl.std integer, derivable.
                field_member(named_field(
                    "timestamp",
                    2,
                    "ridl.std.Timestamp",
                    false,
                    init_value(true, None),
                )),
            ],
            fixed_layout: true,
        }),
    ));

    decls.push(v2::Decl {
        is_error: true,
        ..public_decl(
            "SensorFault",
            v2::decl::Kind::StructDef(v2::StructDef {
                members: vec![
                    field_member(named_field(
                        "code",
                        1,
                        "Counter",
                        false,
                        init_value(true, None),
                    )),
                    field_member(named_field(
                        "message",
                        2,
                        "ridl.std.Message",
                        false,
                        init_value(false, None),
                    )),
                ],
                fixed_layout: false,
            }),
        )
    });

    // internal struct RawWheelFrame { ticks : Counter, frame : bytes [8] }
    let frame = v2::Field {
        r#type: Some(v2::FieldType {
            optional: false,
            kind: Some(v2::field_type::Kind::InlineScalar(Box::new(v2::TypeDef {
                backing: Some(v2::Backing {
                    kind: Some(v2::backing::Kind::Primitive(
                        v2::PrimitiveType::Bytes as i32,
                    )),
                }),
                constraint: Some(v2::Constraint {
                    len_min: Some(8),
                    len_max: Some(8),
                    ..constraint(None, None, None)
                }),
                declared_init: None,
                init: None,
                width: None,
            }))),
        }),
        ..named_field("frame", 2, "", false, init_value(false, None))
    };
    decls.push(v2::Decl {
        visibility: v2::Visibility::Internal as i32,
        ..public_decl(
            "RawWheelFrame",
            v2::decl::Kind::StructDef(v2::StructDef {
                members: vec![
                    field_member(named_field(
                        "ticks",
                        1,
                        "Counter",
                        false,
                        init_value(true, None),
                    )),
                    field_member(frame),
                ],
                fixed_layout: false,
            }),
        )
    });

    package("veh.common", decls)
}

#[test]
fn appendix_b_rust_snapshot() {
    let Generated { rust_source, .. } = generate(&appendix_b()).expect("Appendix B generates");
    insta::assert_snapshot!(rust_source);
}

/// Builds `ridl-rt` as an rlib with plain `rustc`, so a compile proof can
/// pass `--extern ridl_rt=<path>` for the generated source that names the
/// runtime.
///
/// Every generated named scalar names `::ridl_rt::payload::Violation` from its
/// `new`, so every compile proof over generated code passes `--extern`;
/// [`the_compile_proof_harness_links_ridl_rt`] checks the helper on its own,
/// with a source that has no other way to fail.
///
/// One `rustc` call over its `lib.rs` is the whole build: `ridl-rt` is
/// `no_std` and has no dependency in any feature combination (ADR-0021
/// decision 8). It is built with the same `rustc` the proof itself spawns; an
/// rlib built by another toolchain is rejected with E0514, which is what
/// happens if this is hoisted to a shared location outside the repository.
///
/// **The three encoding features are enabled here**, which is the proof
/// mechanism E11.7 stage K3 settled. A generated `Payload<E>` implementation
/// names items that a feature gates — `ridl_rt::flatbuffers` is the first —
/// and this rlib is what every compile proof links, so a proof over generated
/// codec code does not compile against a bare build. One `--cfg` argument per
/// encoding is the whole mechanism; the alternative considered was a cargo
/// build of an emitted crate with path dependencies, which costs a manifest,
/// a target directory and a cargo run per proof and still does not build what
/// a consumer outside this repository builds. Enabling all three rather than
/// only `flatbuffers` is so that E11.8 and E11.12 inherit a working proof
/// without editing this helper again.
///
/// Edition 2021 is the edition `crates/ridl-rt/Cargo.toml` declares. The
/// generated code keeps compiling as edition 2024; the two are independent.
fn ridl_rt_rlib(dir: &std::path::Path) -> std::path::PathBuf {
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("ridl-rt")
        .join("src")
        .join("lib.rs");
    let rlib = dir.join("libridl_rt.rlib");
    let status = std::process::Command::new("rustc")
        .args([
            "--edition",
            "2021",
            "--crate-type",
            "rlib",
            "--crate-name",
            "ridl_rt",
        ])
        .arg("--cfg")
        .arg(r#"feature="flatbuffers""#)
        .arg("--cfg")
        .arg(r#"feature="proto3""#)
        .arg("--cfg")
        .arg(r#"feature="repr-c""#)
        .arg(&source)
        .arg("-o")
        .arg(&rlib)
        .status()
        .expect("rustc must be installed and runnable for this test to be meaningful");
    assert!(
        status.success(),
        "ridl-rt must build as an rlib for the compile proofs to see it"
    );
    // An rlib is an `ar` archive. Asserting the magic distinguishes a real
    // rlib from a metadata-only file under the same name, which a proof using
    // `--emit metadata` would accept while nothing that links could.
    let head = std::fs::read(&rlib).expect("the rlib is readable");
    assert!(
        head.starts_with(b"!<arch>\n"),
        "the helper must produce an rlib archive, not metadata under an rlib name"
    );
    rlib
}

/// A hand-written stand-in for the `regex` crate, built as an rlib with plain
/// `rustc` the same way [`ridl_rt_rlib`] builds `ridl-rt`. `regex` is not a
/// declared dependency of any workspace crate (`validate-pattern` names it as
/// an optional dependency only in the crate emitted for a consumer, never
/// here), so the real crate cannot be linked; this stand-in exposes just
/// enough surface for the emitted `#[cfg(feature = "validate-pattern")]`
/// block to type-check: a `Regex` with a fallible `new` and an `is_match`.
///
/// `is_match` takes `&str` and nothing more general (not `&[u8]`, not an
/// `AsRef<str>` bound), so the proof pins the emitted call's argument type
/// rather than accepting whatever the generator produces. Passing the value
/// by move instead of by reference, for instance, fails here.
///
/// It does **not** guard the backing guard in `constraint_checks`. Doing that
/// would need a compile proof over a bytes fixture with the feature on, and
/// no proof here feeds one: both proofs use a `String`-backed fixture, since
/// a bytes fixture emits no gated block to compile. Removing the backing
/// guard is caught by `bytes_backed_pattern_emits_no_pattern_check`, which
/// reads the generated text, not by anything that compiles it.
const REGEX_STAND_IN_SOURCE: &str = r#"
pub struct Regex {
    pattern: String,
}

#[derive(Debug)]
pub struct Error;

impl Regex {
    /// Refuses a pattern that still carries its `/` delimiters. The real
    /// engine accepts `"/ABC/"` — it is a valid regex whose first and last
    /// characters are literal slashes, so it simply never matches a value
    /// that has none — which is why an executed proof needs this refusal to
    /// notice that the emitter stopped stripping them.
    ///
    /// Both ends must be slashes, not either end. `strip_regex_delimiters`
    /// removes one leading and one trailing `/`, so a typl pattern written
    /// `/a\//` strips to `a\/`, which ends in a slash and is correct. A
    /// refusal keyed on either end alone would reject that.
    pub fn new(pattern: &str) -> Result<Regex, Error> {
        if pattern.len() >= 2 && pattern.starts_with('/') && pattern.ends_with('/') {
            return Err(Error);
        }
        Ok(Regex { pattern: pattern.to_string() })
    }

    /// Matches when the text equals the pattern. This is not a regex engine
    /// and does not pretend to be one: it is the smallest predicate that
    /// distinguishes a match from a non-match, which is all an executed
    /// proof of the constructor's polarity needs.
    pub fn is_match(&self, text: &str) -> bool {
        text == self.pattern
    }
}
"#;

fn regex_stub_rlib(dir: &std::path::Path) -> std::path::PathBuf {
    let source_path = dir.join("regex_stand_in.rs");
    std::fs::write(&source_path, REGEX_STAND_IN_SOURCE)
        .expect("the regex stand-in source is written");
    let rlib = dir.join("libregex.rlib");
    let status = std::process::Command::new("rustc")
        .args([
            "--edition",
            "2024",
            "--crate-type",
            "rlib",
            "--crate-name",
            "regex",
        ])
        .arg(&source_path)
        .arg("-o")
        .arg(&rlib)
        .status()
        .expect("rustc must be installed and runnable for this test to be meaningful");
    assert!(
        status.success(),
        "the regex stand-in must build as an rlib for the compile proof to link"
    );
    let head = std::fs::read(&rlib).expect("the rlib is readable");
    assert!(
        head.starts_with(b"!<arch>\n"),
        "the helper must produce an rlib archive, not metadata under an rlib name"
    );
    rlib
}

#[test]
fn the_compile_proof_harness_links_ridl_rt() {
    // The harness the compile proofs use. Source naming the runtime by its
    // absolute path must compile against the rlib this helper builds, and
    // must fail without it. Without the second half, a helper that produced
    // an unusable rlib would only be noticed through a proof failing for a
    // reason that says nothing about the generated code.
    let dir = tempfile::tempdir().expect("a temp dir is created");
    let source_path = dir.path().join("names_the_runtime.rs");
    std::fs::write(
        &source_path,
        // The shape the generated constructors use: the absolute path, no
        // `use` line, so no name enters the module's namespace.
        r#"
pub struct Speed(pub u16);

impl Speed {
    pub fn new(value: u16) -> Result<Self, ::ridl_rt::payload::Violation> {
        if value > 300 {
            return Err(::ridl_rt::payload::Violation {
                type_name: "Speed",
                rule: ::ridl_rt::payload::Rule::Range,
            });
        }
        Ok(Self(value))
    }
}
"#,
    )
    .expect("the source is written");
    let rlib = ridl_rt_rlib(dir.path());

    let with_extern = std::process::Command::new("rustc")
        .args([
            "--edition",
            "2024",
            "--crate-type",
            "lib",
            "--emit",
            "metadata",
        ])
        .arg("-o")
        .arg(dir.path().join("with_extern.rmeta"))
        .arg("--extern")
        .arg(format!("ridl_rt={}", rlib.display()))
        .arg(&source_path)
        .status()
        .expect("rustc must be installed and runnable for this test to be meaningful");
    assert!(
        with_extern.success(),
        "source naming ::ridl_rt::payload::Violation must compile against the helper's rlib"
    );
    // Where it was asked to put it. Without this, a run with no `-o` would
    // also exit zero, writing its output into the package directory instead.
    assert!(
        dir.path().join("with_extern.rmeta").exists(),
        "the proof's output must land in its own temp directory"
    );

    // Captured, so the expected failure does not print an error into an
    // otherwise passing test run.
    let without_extern = std::process::Command::new("rustc")
        .args([
            "--edition",
            "2024",
            "--crate-type",
            "lib",
            "--emit",
            "metadata",
        ])
        .arg("-o")
        .arg(dir.path().join("without_extern.rmeta"))
        .arg(&source_path)
        .output()
        .expect("rustc must be installed and runnable for this test to be meaningful");
    assert!(
        !without_extern.status.success(),
        "without --extern the same source must fail, or the proof proves nothing"
    );
    // And it must fail because `ridl_rt` is not linked, rather than because
    // the source above has a mistake in it, which would pass this test while
    // proving nothing about the rlib. The check is on the error code, not on
    // the crate name: `rustc` echoes the offending source line into its
    // diagnostic, so `ridl_rt` appears in stderr for any error whose span
    // falls on that line.
    let stderr = String::from_utf8_lossy(&without_extern.stderr);
    assert!(
        stderr.contains("E0433"),
        "the failure must be the unresolved crate, got:\n{stderr}"
    );
}

#[test]
fn the_harness_rlib_belongs_to_its_caller() {
    // Two callers must get two files, each under the directory it passed.
    // `cargo test` runs the compile proofs in parallel threads, so a helper
    // writing to one shared path would have two `rustc` processes writing one
    // file while a third read it.
    let first = tempfile::tempdir().expect("a temp dir is created");
    let second = tempfile::tempdir().expect("a temp dir is created");
    let first_rlib = ridl_rt_rlib(first.path());
    let second_rlib = ridl_rt_rlib(second.path());

    assert_ne!(first_rlib, second_rlib);
    assert!(first_rlib.starts_with(first.path()));
    assert!(second_rlib.starts_with(second.path()));
    assert!(first_rlib.exists() && second_rlib.exists());
}

/// The generated Rust for the full Appendix B package compiles with `rustc`.
/// A minimal prelude stands in for the `ridl.std` types the package imports,
/// declared in the module path the cross-package references map to. The temp
/// directory is removed on drop, closing the #102 temp-file leak.
#[test]
fn appendix_b_compiles_with_rustc() {
    let Generated { rust_source, .. } = generate(&appendix_b()).expect("Appendix B generates");

    // The stand-in types carry the three derives every generated type carries
    // (design decision 7), because that is what the real `ridl.std` package
    // generates. Without them the proof fails on the stand-in rather than on
    // the code under test: a struct with a cross-package field still derives
    // `Debug`, `Clone` and `PartialEq`, and those three reach the field's type.
    // The conditional derives are deliberately absent here — a cross-package
    // reference disables them on this side, so the proof would not notice if
    // the stand-in had them.
    const PRELUDE: &str = "\
pub mod ridl {
    pub mod std {
        #[derive(Debug, Clone, PartialEq)]
        pub struct Name(pub String);
        #[derive(Debug, Clone, PartialEq)]
        pub struct Message(pub String);
        #[derive(Debug, Clone, PartialEq)]
        pub struct Label(pub String);
        #[derive(Debug, Clone, PartialEq)]
        pub struct Timestamp(pub i64);
        impl Default for Timestamp {
            fn default() -> Self {
                Timestamp(0)
            }
        }
    }
}
";
    let source = format!("{PRELUDE}\n{rust_source}");

    let dir = tempfile::tempdir().expect("a temp dir is created");
    let source_path = dir.path().join("appendix_b.rs");
    let meta_path = dir.path().join("appendix_b.rmeta");
    std::fs::write(&source_path, &source).expect("the generated source is written");
    let rlib = ridl_rt_rlib(dir.path());

    // `non_snake_case` is denied so that a field name reaching generated Rust
    // verbatim fails this run rather than warning in it — the assertion below
    // is on the exit status, which a warning does not change. It is inert on
    // Appendix B, whose every field name is a single word; the proof that
    // guards issue #243 is `appendix_a_compiles_with_rustc`. The deny here is
    // what makes this proof stay a proof if a multi-word field name is ever
    // added to the fixture. An enum variant keeps its typl `SCREAMING_SNAKE`
    // spelling and draws `non_camel_case_types`, a different lint, which
    // stays undenied.
    let status = std::process::Command::new("rustc")
        .args([
            "--edition",
            "2024",
            "--crate-type",
            "lib",
            "--emit",
            "metadata",
            "-D",
            "non_snake_case",
        ])
        .arg("-o")
        .arg(&meta_path)
        .arg("--extern")
        .arg(format!("ridl_rt={}", rlib.display()))
        .arg(&source_path)
        .status()
        .expect("rustc must be installed and runnable for this test to be meaningful");

    assert!(
        status.success(),
        "generated Rust for Appendix B must compile under `-D non_snake_case`, \
         source:\n{source}"
    );
}

/// The generated conversions are compiled **and run**.
///
/// Every other proof in this file stops at `--emit metadata`, which
/// type-checks the emitted source and never evaluates it. That leaves the
/// semantics of the validating seam resting on the snapshots alone, and the
/// task's own workflow blesses a snapshot with `--accept`, so a wrong
/// emission introduced alongside a right one is written into the `.snap` in
/// the same command and never fails again. Inverting the enum set's mask test
/// to `value & Self::DECLARED_MASK != 0` type-checks, so only an executed
/// assertion catches it.
#[test]
fn the_generated_conversions_run() {
    let source = format!(
        "{}\n{}",
        rust_for(vec![gear_position_decl(), features_decl()]),
        r#"
fn main() {
    // The declared discriminants are 0, 1, 7, 9 — not contiguous, so a
    // conversion keyed on the variant's position would map 7 to nothing.
    match GearPosition::try_from(7) {
        Ok(GearPosition::REVERSE) => {}
        _ => panic!("7 is REVERSE"),
    }
    // 2 is a gap in the declared discriminants, so it is out of contract.
    match GearPosition::try_from(2) {
        Err(v) => assert_eq!(v.rule, ::ridl_rt::payload::Rule::Variant),
        Ok(_) => panic!("2 names no declared variant"),
    }
    assert_eq!(i64::from(GearPosition::NEUTRAL), 9);

    // Bits 0 to 3 are declared, so the mask is 0b1111.
    assert_eq!(Features::DECLARED_MASK, 15);
    // Bits 0 and 2, both declared.
    match Features::try_from(5) {
        Ok(f) => assert_eq!(f.get(), 5),
        Err(_) => panic!("5 carries only declared bits"),
    }
    // Bit 4 is not declared. This is the assertion that fails if the mask
    // test is inverted.
    match Features::try_from(16) {
        Err(v) => assert_eq!(v.rule, ::ridl_rt::payload::Rule::Variant),
        Ok(_) => panic!("16 carries an undeclared bit"),
    }
    // A declared bit on its own is accepted, so the test above is not passing
    // because everything is refused.
    match Features::try_from(8) {
        Ok(f) => assert_eq!(f.get(), 8),
        Err(_) => panic!("8 is the declared bit 3"),
    }
    assert_eq!(i64::from(Features::LOW_FUEL), 1);
}
"#
    );

    let dir = tempfile::tempdir().expect("a temp dir is created");
    let source_path = dir.path().join("conversions.rs");
    let bin_path = dir.path().join("conversions");
    std::fs::write(&source_path, &source).expect("the generated source is written");
    let rlib = ridl_rt_rlib(dir.path());

    let status = std::process::Command::new("rustc")
        .args(["--edition", "2024", "--crate-type", "bin"])
        .arg("-o")
        .arg(&bin_path)
        .arg("--extern")
        .arg(format!("ridl_rt={}", rlib.display()))
        .arg(&source_path)
        .status()
        .expect("rustc must be installed and runnable for this test to be meaningful");
    assert!(
        status.success(),
        "the generated conversions must compile, source:\n{source}"
    );

    let run = std::process::Command::new(&bin_path)
        .output()
        .expect("the compiled program runs");
    assert!(
        run.status.success(),
        "the generated conversions must behave as declared, stderr:\n{}",
        String::from_utf8_lossy(&run.stderr)
    );
}

/// A package whose collection fields are Default-constructible, so the array
/// and map default forms (`core::array::from_fn`, range-map collect) are
/// emitted and compiled.
#[test]
fn constructible_collections_compile() {
    let bag = v2::StructDef {
        members: vec![
            field_member(array_field("fixed", "Speed", 3, 3)),
            field_member(v2::Field {
                ordinal: 2,
                ..array_field("bounded", "Speed", 2, 5)
            }),
            field_member(v2::Field {
                r#type: Some(v2::FieldType {
                    optional: false,
                    kind: Some(v2::field_type::Kind::Map(Box::new(v2::MapType {
                        key: Some(Box::new(v2::FieldType {
                            optional: false,
                            kind: Some(v2::field_type::Kind::Named("Counter".to_string())),
                        })),
                        value: Some(Box::new(v2::FieldType {
                            optional: false,
                            kind: Some(v2::field_type::Kind::Named("Speed".to_string())),
                        })),
                        min: 1,
                        max: 4,
                    }))),
                }),
                ..named_field("pairs", 3, "", false, init_value(true, None))
            }),
        ],
        fixed_layout: false,
    };
    let decls = vec![
        speed_decl(),
        counter_decl(),
        public_decl("Bag", v2::decl::Kind::StructDef(bag)),
    ];
    let Generated { rust_source, .. } = generate(&package("veh.common", decls)).expect("generates");

    assert!(
        rust_source.contains("impl Default for Bag"),
        "Bag with derivable collection fields must get a Default, got:\n{rust_source}"
    );

    let dir = tempfile::tempdir().expect("a temp dir is created");
    let source_path = dir.path().join("bag.rs");
    let meta_path = dir.path().join("bag.rmeta");
    std::fs::write(&source_path, &rust_source).expect("the generated source is written");
    let rlib = ridl_rt_rlib(dir.path());
    // `-D non_snake_case` is inert on this fixture, whose every field name is
    // a single word. It is denied for the same reason as in
    // `appendix_b_compiles_with_rustc`: to keep this proof a proof if a
    // multi-word field name is ever added. Issue #243 is guarded by
    // `appendix_a_compiles_with_rustc`.
    let status = std::process::Command::new("rustc")
        .args([
            "--edition",
            "2024",
            "--crate-type",
            "lib",
            "--emit",
            "metadata",
            "-D",
            "non_snake_case",
        ])
        .arg("-o")
        .arg(&meta_path)
        .arg("--extern")
        .arg(format!("ridl_rt={}", rlib.display()))
        .arg(&source_path)
        .status()
        .expect("rustc runs");
    assert!(
        status.success(),
        "the collection default forms must compile under `-D non_snake_case`, \
         source:\n{rust_source}"
    );
}

#[test]
fn keyword_field_name_is_raw_escaped() {
    // `override` is a reserved Rust keyword; it must be emitted as a raw
    // identifier rather than panicking the emitter.
    let struct_def = v2::StructDef {
        members: vec![field_member(named_field(
            "override",
            1,
            "Speed",
            false,
            init_value(true, None),
        ))],
        fixed_layout: false,
    };
    let source = rust_for(vec![
        speed_decl(),
        public_decl("Config", v2::decl::Kind::StructDef(struct_def)),
    ]);
    assert!(
        source.contains("r#override"),
        "a keyword field name must be raw-escaped, got:\n{source}"
    );
}

// ---------------------------------------------------------------------------
// Epic E1 whole-epic review — regression fixtures.
// ---------------------------------------------------------------------------

/// C1b: a non-optional self-reference `struct S { next: S }` is a cycle. The
/// checker rejects it (TYPL-206), but the backend must not trust that gate:
/// the Default recursion must terminate rather than overflow the stack.
#[test]
fn recursive_struct_default_terminates() {
    let recursive = v2::StructDef {
        members: vec![field_member(named_field(
            "next",
            1,
            "S",
            false,
            init_value(true, None),
        ))],
        fixed_layout: false,
    };
    let generated = generate(&package(
        "veh.common",
        vec![public_decl("S", v2::decl::Kind::StructDef(recursive))],
    ))
    .expect("a cyclic struct's Default derivation must terminate, not overflow");
    assert!(
        !generated.rust_source.contains("impl Default for S"),
        "a cyclic struct must get no Default, got:\n{}",
        generated.rust_source
    );
}

/// I1: `type Plate : string [8..9 match P] = "AA-000-AA"` — the declared init
/// makes the type derivable and IS its default, so the backend emits the
/// declared value, never an empty string.
#[test]
fn declared_string_init_becomes_the_default() {
    let plate = v2::decl::Kind::TypeDef(v2::TypeDef {
        backing: Some(v2::Backing {
            kind: Some(v2::backing::Kind::Primitive(
                v2::PrimitiveType::String as i32,
            )),
        }),
        constraint: Some(v2::Constraint {
            len_min: Some(8),
            len_max: Some(9),
            ..constraint(None, None, None)
        }),
        declared_init: Some("AA-000-AA".to_string()),
        init: Some(init_value(true, Some("AA-000-AA"))),
        width: None,
    });
    let source = rust_for(vec![public_decl("Plate", plate)]);
    assert!(
        source.contains("impl Default for Plate"),
        "a declared-init string type gets a Default, got:\n{source}"
    );
    assert!(
        source.contains("\"AA-000-AA\"") && source.contains("to_string"),
        "the Default must be the declared init, not an empty string, got:\n{source}"
    );
    assert!(
        !source.contains("String::new()"),
        "the empty-string form is only for the derived case, got:\n{source}"
    );
}

/// I2: a cross-package field with a declared init cannot be faithfully wrapped
/// without the remote backing; emitting the referenced type's own default would
/// be a wrong value, so the whole struct gets no Default.
#[test]
fn cross_package_declared_init_omits_the_default() {
    let field = v2::Field {
        declared_init: Some("1".to_string()),
        ..named_field(
            "gearIndex",
            1,
            "veh.other.GearIndex",
            false,
            init_value(true, Some("1")),
        )
    };
    let struct_def = v2::StructDef {
        members: vec![field_member(field)],
        fixed_layout: false,
    };
    let source = rust_for(vec![public_decl(
        "Selection",
        v2::decl::Kind::StructDef(struct_def),
    )]);
    assert!(
        !source.contains("impl Default for Selection"),
        "a struct with a cross-package declared-init field must get no Default, got:\n{source}"
    );
}

/// A `Selection` struct holding one `GearIndex` field whose declared init is
/// the value 1. The `range` argument carries the named type's constraint,
/// which decides whether the wrapping constructor is `new_unchecked` or
/// `new`.
fn selection_with_gear_index(range: Option<v2::Constraint>) -> String {
    let mut gear_kind = primitive_type(
        v2::PrimitiveType::Integer,
        init_value(true, Some("0")),
        Some(v2::type_def::Width::IntWidth(v2::IntWidth::U8 as i32)),
    );
    if let v2::decl::Kind::TypeDef(td) = &mut gear_kind {
        td.constraint = range;
    }
    let gear_index = public_decl("GearIndex", gear_kind);
    let field = v2::Field {
        declared_init: Some("1".to_string()),
        ..named_field(
            "gearIndex",
            1,
            "GearIndex",
            false,
            init_value(true, Some("1")),
        )
    };
    let struct_def = v2::StructDef {
        members: vec![field_member(field)],
        fixed_layout: false,
    };
    rust_for(vec![
        gear_index,
        public_decl("Selection", v2::decl::Kind::StructDef(struct_def)),
    ])
}

/// I2: the same-package equivalent CAN be wrapped: the backend knows
/// `GearIndex`'s integer backing, so the declared init 1 wraps to
/// `GearIndex::new_unchecked(1)`.
#[test]
fn same_package_declared_init_gets_the_correct_default() {
    let source = selection_with_gear_index(Some(constraint(Some("0"), Some("8"), None)));
    assert!(
        source.contains("impl Default for Selection"),
        "the same-package equivalent gets a Default, got:\n{source}"
    );
    assert!(
        source.contains("GearIndex::new_unchecked(1)"),
        "the declared init 1 must wrap to GearIndex::new_unchecked(1), got:\n{source}"
    );
}

/// The same wrapping on a vacuous named type. It has no `new_unchecked`
/// (`emit_vacuous_type_def`), so the declared init wraps through its `const
/// fn new` instead — the same substitution `emit_const` makes.
#[test]
fn same_package_declared_init_of_a_vacuous_type_wraps_through_new() {
    let source = selection_with_gear_index(None);
    assert!(
        source.contains("GearIndex::new(1)") && !source.contains("GearIndex::new_unchecked(1)"),
        "the declared init 1 must wrap to GearIndex::new(1), got:\n{source}"
    );
}

/// M1: the IR stores a regex constant's source with its `/…/` delimiters; the
/// emitted `&str` must hold the pattern only.
#[test]
fn regex_const_strips_its_delimiters() {
    let source = rust_for(vec![public_decl(
        "PLATE_PATTERN",
        v2::decl::Kind::ConstDef(v2::ConstDef {
            type_ref: None,
            value: String::new(),
            regex: Some("/^[A-Z]{2}-[0-9]{3}$/".to_string()),
        }),
    )]);
    assert!(
        source.contains("PLATE_PATTERN: &str = \"^[A-Z]{2}-[0-9]{3}$\""),
        "the regex const must emit its pattern without delimiters, got:\n{source}"
    );
    assert!(
        !source.contains("/^[A-Z]"),
        "the surrounding slash delimiters must be stripped, got:\n{source}"
    );
}

// ---------------------------------------------------------------------------
// Identifier totality.
// ---------------------------------------------------------------------------

/// `ident` is total. A valid typl name can never be empty (typl §2.3), so an
/// empty string only reaches here from malformed IR — but the backend is fed
/// by `ridlc::compile`, whose documented contract is that it never panics, and
/// by the language server, which sees IR lowered from in-progress source. An
/// empty name therefore lowers to `_` instead of reaching `Ident::new_raw`,
/// which panics on the empty string.
#[test]
fn ident_is_total_on_the_empty_name() {
    assert_eq!(super::ident("").to_string(), "_");
}

/// A typl name of `_` keeps its own mangling, so the empty-name placeholder
/// can never be confused with a real declaration.
#[test]
fn ident_maps_the_underscore_name_away_from_the_placeholder() {
    assert_eq!(super::ident("_").to_string(), "__");
}

/// The limit of the `_` placeholder, pinned rather than assumed. syn accepts
/// `_` in *field* position, so the `syn::parse2` gate does not catch an
/// empty-named field. A derived `Default` usually catches it instead, because
/// the struct expression it builds has no valid `Member` — but
/// `defaults::struct_default` returns `None` for a non-constructible field (a
/// cross-package reference carrying a declared init), and then nothing is left
/// to catch it and `generate` returns `Ok`.
///
/// This is unreachable from source today: `Parser::block_body` announces a
/// member only on `SyntaxKind::Ident`, so a nameless `FieldDef` is never
/// built. The test exists so that a change making one reachable shows up here
/// rather than as silently emitted Rust.
#[test]
fn generate_emits_an_empty_field_name_without_a_derivable_default() {
    let field = v2::Field {
        name: String::new(),
        ordinal: 1,
        r#type: Some(v2::FieldType {
            optional: false,
            kind: Some(v2::field_type::Kind::Named("other.pkg.Thing".to_string())),
        }),
        // A declared init on a cross-package reference makes the field
        // non-constructible, so no `Default` is derived.
        declared_init: Some("1".to_string()),
        init: Some(init_value(false, None)),
        doc: String::new(),
        labels: Vec::new(),
        deprecated: None,
    };
    let decls = vec![public_decl(
        "S",
        v2::decl::Kind::StructDef(v2::StructDef {
            members: vec![field_member(field)],
            fixed_layout: false,
        }),
    )];
    let generated = generate(&package("app", decls)).expect("the parse gate does not catch `_`");
    assert!(
        generated.rust_source.contains("pub _:"),
        "expected an emitted `_` field, got:\n{}",
        generated.rust_source,
    );
    assert!(
        !generated.rust_source.contains("impl Default for S"),
        "the struct must not derive Default, got:\n{}",
        generated.rust_source,
    );
}

/// An empty-named *declaration* is reported, not emitted. `_` is illegal in a
/// Rust declaration-name position, so the `syn::parse2` gate in `generate`
/// turns it into a `GenerateError` — the codegen totality contract
/// (ADR-0004 §5) holds without the malformed name becoming plausible-looking
/// output.
#[test]
fn generate_reports_an_empty_named_decl_instead_of_panicking() {
    let decls = vec![public_decl(
        "",
        primitive_type(
            v2::PrimitiveType::Boolean,
            init_value(true, Some("false")),
            None,
        ),
    )];
    let error = generate(&package("app", decls)).expect_err("an empty name cannot generate");
    assert!(
        error.message.contains("does not parse"),
        "expected the parse gate to reject `_`, got: {}",
        error.message,
    );
}

// ---------------------------------------------------------------------------
// Interactions and services (E2 task 15) — fixture builders.
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Per-kind interaction snapshots.
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Appendix A — the ridl reference corpus package `veh.cluster`.
// ---------------------------------------------------------------------------

/// The ridl reference Appendix A package `veh.cluster`, as **`ridl-sem`
/// actually lowers it**.
///
/// The IR is not rebuilt here. It is deserialized from the golden that
/// `ridl-sem` writes for its own `appendix_a_ir` test, so this backend and the
/// lowering that feeds it cannot drift: there is one artifact, and a change in
/// lowering reaches this test the moment that golden is regenerated. A
/// hand-built copy could not do that — it would keep compiling happily while
/// describing an IR the compiler no longer produces, which for the E2 exit
/// criterion's compile proof would make the proof about the fixture rather
/// than about the pipeline.
///
/// Cross-package references to `veh.common` and `ridl.std` stay fully
/// qualified, as the resolver leaves them (typl §3.2).
///
/// One residual is worth recording. The coupling holds as long as the golden is
/// a live artifact: if the `ridl-sem` test that writes it were deleted and the
/// `.snap` file left behind, this loader would keep reading an orphan and keep
/// passing. `cargo insta test --unreferenced` detects exactly that, and it is
/// not in the local gate — adding it is a repo-wide call for the epic
/// close-out, not this backend's to make. Deleting the file outright is caught
/// here immediately, so the gap is narrow: an orphaned snapshot, not a missing
/// one.
fn appendix_a() -> v2::Package {
    // `CARGO_MANIFEST_DIR` is `<workspace>/crates/ridl-backend-rust`.
    let golden = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../ridl-sem/src/snapshots/ridl_sem__check__tests__appendix_a_ir.snap"
    );
    let text = std::fs::read_to_string(golden).unwrap_or_else(|err| {
        panic!(
            "the ridl-sem Appendix A golden must be readable at {golden}: {err}\n\
             this test consumes that crate's lowering rather than a copy of it"
        )
    });

    // An insta snapshot is a YAML header, a `---` line, then the payload.
    let body = text
        .split_once("\n---\n")
        .map(|(_, body)| body)
        .unwrap_or(&text);
    let package = v2::from_json(body)
        .expect("the ridl-sem Appendix A golden deserializes as an IR v2 package");

    // The golden is the whole point, so its shape is asserted rather than
    // assumed: a silently empty or renamed package would turn the compile proof
    // below into a proof about nothing.
    assert_eq!(package.name, "veh.cluster");
    assert_eq!(
        package.interfaces.len(),
        1,
        "Appendix A declares exactly one interface"
    );
    assert!(
        package.services.is_empty(),
        "Appendix A declares no service — services are covered by their own tests"
    );
    package
}

/// The generated Rust for the full Appendix A package compiles with `rustc` —
/// the E2 exit criterion's proof for the Rust half. A minimal prelude stands in
/// for the `veh.common` and `ridl.std` types the package imports, declared in
/// the module path the cross-package references map to.
#[test]
fn appendix_a_compiles_with_rustc() {
    let Generated { rust_source, .. } = generate(&appendix_a()).expect("Appendix A generates");

    // Every stand-in carries the three derives a generated type always
    // carries (design decision 7), because the real packages generate them
    // and the importing package's own derives reach these types through its
    // fields. The conditional ones are left off: a cross-package reference
    // disables them on the importing side, so the proof cannot observe them.
    const PRELUDE: &str = "\
pub mod ridl {
    pub mod std {
        #[derive(Debug, Clone, PartialEq)]
        pub struct Message(pub String);
        #[derive(Debug, Clone, PartialEq)]
        pub struct Label(pub String);
        #[derive(Debug, Clone, PartialEq)]
        pub struct Version(pub String);
        #[derive(Debug, Clone, PartialEq)]
        pub struct Duration(pub i64);
        #[derive(Debug, Clone, PartialEq, Default)]
        pub struct Timestamp(pub i64);
    }
}
pub mod veh {
    pub mod common {
        #[derive(Debug, Clone, PartialEq)]
        pub struct Speed(pub f64);
        #[derive(Debug, Clone, PartialEq)]
        pub struct Temperature(pub f64);
        #[derive(Debug, Clone, PartialEq)]
        pub struct WarningFlags(pub i64);
        #[derive(Debug, Clone, PartialEq)]
        pub enum GearPosition {
            PARK = 0,
            DRIVE = 1,
        }
    }
}
";
    let source = format!("{PRELUDE}\n{rust_source}");

    let dir = tempfile::tempdir().expect("a temp dir is created");
    let source_path = dir.path().join("appendix_a.rs");
    let meta_path = dir.path().join("appendix_a.rmeta");
    std::fs::write(&source_path, &source).expect("the generated source is written");
    let rlib = ridl_rt_rlib(dir.path());

    // `non_snake_case` is denied by name, and this is the proof that guards
    // issue #243: the Appendix A IR carries the field names `sensorId` and
    // `isOpen`, so a field name reaching generated Rust verbatim fails this
    // run. The assertion is otherwise on the exit status, which a warning
    // does not change. `non_camel_case_types`, which a screaming-case enum
    // variant draws by design, stays undenied.
    let status = std::process::Command::new("rustc")
        .args([
            "--edition",
            "2024",
            "--crate-type",
            "lib",
            "--emit",
            "metadata",
            "-D",
            "non_snake_case",
        ])
        .arg("-o")
        .arg(&meta_path)
        .arg("--extern")
        .arg(format!("ridl_rt={}", rlib.display()))
        .arg(&source_path)
        .status()
        .expect("rustc must be installed and runnable for this test to be meaningful");

    assert!(
        status.success(),
        "generated Rust for Appendix A must compile under `-D non_snake_case`, \
         source:\n{source}"
    );
}

// ---------------------------------------------------------------------------
// Transport identity inside an inline service shape.
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Contract data.
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Streams, services, and vocabulary collisions.
// ---------------------------------------------------------------------------

/// A package with no interactions is unaffected by the collision check: the
/// vocabulary is not emitted, so the name is free.
#[test]
fn a_typl_only_package_may_declare_a_vocabulary_name() {
    let source = rust_for(vec![public_decl(
        "Provenance",
        primitive_type(
            v2::PrimitiveType::Integer,
            init_value(true, Some("0")),
            None,
        ),
    )]);
    assert!(source.contains("struct Provenance"), "got:\n{source}");
    assert!(
        !source.contains("enum Provenance"),
        "the vocabulary is not emitted for a typl-only package, got:\n{source}"
    );
}

// ---------------------------------------------------------------------------
// The generated-name classes a package must not collide with.
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// What the check must NOT reject: Rust's namespaces genuinely separate these.
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Nested tuple names.
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Generated names colliding with each other.
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// `internal` visibility on the interaction layer (ADR-0008 decision 7).
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// The package-segment spelling shared with `ridlc`'s module tree.
// ---------------------------------------------------------------------------

/// `module_segment` exists so that the module tree `ridlc` writes for
/// `--emit rust` and the paths `type_path` emits cannot drift apart: a tree
/// that spells a segment differently from the reference emits a module the
/// reference cannot name. Nothing pinned that agreement at the definition
/// site, so a change to either side that kept both internally consistent went
/// unnoticed here.
///
/// The segments are the ones that force a spelling. `is_valid_name_segment`
/// accepts any lowercase ASCII segment, so `veh.mod`, `veh.type`, `veh.fn`,
/// `veh.crate`, `veh.self` and `veh.super` are all legal typl package names.
/// `mod`, `type` and `fn` become raw identifiers; `crate`, `self` and `super`
/// cannot be raw identifiers at all and take a trailing underscore instead.
#[test]
fn module_segment_spells_a_segment_the_way_type_path_does() {
    for segment in ["mod", "type", "fn", "crate", "self", "super", "speed"] {
        let rendered = super::type_path(&format!("{segment}.T")).to_string();
        let expected = format!("crate :: {} :: T", super::module_segment(segment));
        assert_eq!(
            rendered, expected,
            "a cross-package reference into `{segment}` and the module tree's spelling of \
             `{segment}` must agree, or the crate root emits a module the reference cannot name"
        );
    }
}

// ---------------------------------------------------------------------------
// Derives (design decision 7, Task 6).
// ---------------------------------------------------------------------------

/// The trait names inside the `#[derive(...)]` attribute that belongs to the
/// item whose rendered header is `header` (for example `pub struct Bag {`).
///
/// The attribute is found by scanning backwards from the header to the nearest
/// `#[derive(`, and the text between that attribute and the header is required
/// to hold nothing but further attributes and doc comments. Without that guard
/// the helper would silently report the *previous* item's derive for an item
/// that carries none, which is exactly the failure mode a negative assertion
/// has to rule out. prettyplease may wrap a long trait list over several
/// lines, so the list is split on commas and each name trimmed rather than
/// compared as one string.
fn derives_of(source: &str, header: &str) -> Vec<String> {
    let header_at = source
        .find(header)
        .unwrap_or_else(|| panic!("`{header}` must be emitted, got:\n{source}"));
    let open = "#[derive(";
    let attr_at = source[..header_at]
        .rfind(open)
        .unwrap_or_else(|| panic!("`{header}` must carry a derive attribute, got:\n{source}"));
    let list_at = attr_at + open.len();
    let close = source[list_at..header_at]
        .find(")]")
        .unwrap_or_else(|| panic!("the derive attribute must close, got:\n{source}"));
    let list = &source[list_at..list_at + close];
    let gap = &source[list_at + close + ")]".len()..header_at];
    for line in gap.lines().map(str::trim).filter(|line| !line.is_empty()) {
        assert!(
            line.starts_with("#[") || line.starts_with("///") || line.starts_with("//"),
            "the derive attribute found for `{header}` belongs to another item; \
             the text between them is:\n{gap}"
        );
    }
    list.split(',')
        .map(|name| name.trim().to_string())
        .filter(|name| !name.is_empty())
        .collect()
}

/// Every generated type carries `Debug`, `Clone` and `PartialEq` — the three
/// that are sound on every backing (design decision 7). Asserted as the
/// positive companion of each negative assertion below.
fn assert_always_derived(derived: &[String], header: &str) {
    for name in ["Debug", "Clone", "PartialEq"] {
        assert!(
            derived.iter().any(|d| d == name),
            "`{header}` must derive {name}, got {derived:?}"
        );
    }
}

#[test]
fn float_backed_scalar_derives_partial_ord_but_not_ord() {
    let source = rust_for(vec![speed_decl()]);
    let derived = derives_of(&source, "pub struct Speed(f64);");
    assert_always_derived(&derived, "Speed");
    // Ord requires Eq, and f64 is neither Eq nor Hash.
    assert_eq!(
        derived,
        ["Debug", "Clone", "Copy", "PartialEq", "PartialOrd"]
    );
}

#[test]
fn integer_backed_scalar_derives_the_full_ordering_set() {
    let source = rust_for(vec![counter_decl()]);
    let derived = derives_of(&source, "pub struct Counter(i64);");
    assert_always_derived(&derived, "Counter");
    assert_eq!(
        derived,
        [
            "Debug",
            "Clone",
            "Copy",
            "PartialEq",
            "Eq",
            "Hash",
            "PartialOrd",
            "Ord"
        ]
    );
}

#[test]
fn string_backed_scalar_is_not_copy() {
    let decls = vec![public_decl(
        "Label",
        primitive_type(v2::PrimitiveType::String, init_value(true, Some("")), None),
    )];
    let source = rust_for(decls);
    let derived = derives_of(&source, "pub struct Label(String);");
    assert_always_derived(&derived, "Label");
    // A String is Eq and Hash but not Copy, and a string backing is not
    // numeric, so it takes no ordering.
    assert_eq!(derived, ["Debug", "Clone", "PartialEq", "Eq", "Hash"]);
}

/// A struct whose transitive closure reaches a float is neither `Eq` nor
/// `Hash`, but is still `Copy` — the two conditions are independent.
#[test]
fn struct_with_a_float_field_is_not_eq() {
    let speed_field = named_field("speed", 1, "Speed", false, init_value(true, None));
    let telemetry = public_decl(
        "Telemetry",
        v2::decl::Kind::StructDef(v2::StructDef {
            members: vec![field_member(speed_field)],
            fixed_layout: false,
        }),
    );
    let source = rust_for(vec![speed_decl(), telemetry]);
    let derived = derives_of(&source, "pub struct Telemetry {");
    assert_always_derived(&derived, "Telemetry");
    assert_eq!(derived, ["Debug", "Clone", "Copy", "PartialEq"]);
}

/// A struct whose closure holds no float is `Eq` and `Hash`. This is the
/// positive companion of [`struct_with_a_float_field_is_not_eq`]: without it,
/// a rule that never emits `Eq` would pass that test.
#[test]
fn struct_over_integers_only_is_eq_and_hash() {
    let count_field = named_field("count", 1, "Counter", false, init_value(true, None));
    let tally = public_decl(
        "Tally",
        v2::decl::Kind::StructDef(v2::StructDef {
            members: vec![field_member(count_field)],
            fixed_layout: false,
        }),
    );
    let source = rust_for(vec![counter_decl(), tally]);
    let derived = derives_of(&source, "pub struct Tally {");
    assert_always_derived(&derived, "Tally");
    assert_eq!(
        derived,
        ["Debug", "Clone", "Copy", "PartialEq", "Eq", "Hash"]
    );
}

/// Ordering is a named-scalar property. A struct takes none of it, because
/// ordering a struct's fields lexicographically is not something typl states
/// (design decision 7).
#[test]
fn a_struct_takes_no_ordering() {
    let count_field = named_field("count", 1, "Counter", false, init_value(true, None));
    let tally = public_decl(
        "Tally",
        v2::decl::Kind::StructDef(v2::StructDef {
            members: vec![field_member(count_field)],
            fixed_layout: false,
        }),
    );
    let source = rust_for(vec![counter_decl(), tally]);
    let derived = derives_of(&source, "pub struct Tally {");
    // The positive companion: the numeric named scalar in the same package
    // does take both, so an emitter that never emits ordering at all cannot
    // pass this pair.
    let scalar = derives_of(&source, "pub struct Counter(i64);");
    assert!(
        scalar.iter().any(|d| d == "PartialOrd") && scalar.iter().any(|d| d == "Ord"),
        "the numeric named scalar must take both, got {scalar:?}"
    );
    assert!(
        !derived.iter().any(|d| d == "PartialOrd" || d == "Ord"),
        "a struct must take no ordering, got {derived:?}"
    );
}

/// A union takes no ordering either, for the same reason: arm declaration
/// order is not a contract typl states.
#[test]
fn a_union_takes_no_ordering() {
    let arm_struct = |name: &str, field: &str| {
        public_decl(
            name,
            v2::decl::Kind::StructDef(v2::StructDef {
                members: vec![field_member(named_field(
                    field,
                    1,
                    "Counter",
                    false,
                    init_value(true, None),
                ))],
                fixed_layout: false,
            }),
        )
    };
    let union = public_decl(
        "Outcome",
        v2::decl::Kind::UnionDef(v2::UnionDef {
            arms: vec![
                v2::UnionArm {
                    name: "ok".to_string(),
                    ordinal: 1,
                    type_ref: "Reading".to_string(),
                    doc: String::new(),
                },
                v2::UnionArm {
                    name: "err".to_string(),
                    ordinal: 2,
                    type_ref: "Fault".to_string(),
                    doc: String::new(),
                },
            ],
            is_result: true,
            reserved: Vec::new(),
        }),
    );
    let source = rust_for(vec![
        counter_decl(),
        arm_struct("Reading", "value"),
        arm_struct("Fault", "code"),
        union,
    ]);
    let derived = derives_of(&source, "pub enum Outcome {");
    assert_always_derived(&derived, "Outcome");
    assert_eq!(
        derived,
        ["Debug", "Clone", "Copy", "PartialEq", "Eq", "Hash"]
    );
}

/// A union whose arm reaches a float loses `Eq` and `Hash` through the arm,
/// which is the recursion working through a union rather than a struct.
#[test]
fn a_union_arm_reaching_a_float_loses_eq() {
    let reading = public_decl(
        "Reading",
        v2::decl::Kind::StructDef(v2::StructDef {
            members: vec![field_member(named_field(
                "value",
                1,
                "Speed",
                false,
                init_value(true, None),
            ))],
            fixed_layout: false,
        }),
    );
    let union = public_decl(
        "Outcome",
        v2::decl::Kind::UnionDef(v2::UnionDef {
            arms: vec![v2::UnionArm {
                name: "ok".to_string(),
                ordinal: 1,
                type_ref: "Reading".to_string(),
                doc: String::new(),
            }],
            is_result: false,
            reserved: Vec::new(),
        }),
    );
    let source = rust_for(vec![speed_decl(), reading, union]);
    let derived = derives_of(&source, "pub enum Outcome {");
    assert_always_derived(&derived, "Outcome");
    assert_eq!(derived, ["Debug", "Clone", "Copy", "PartialEq"]);
}

/// An unresolvable cross-package reference disables every conditional derive.
/// An unsound `#[derive(Copy)]` is a hard error in the consumer's build, so
/// the backend cannot be optimistic the way `defaults.rs` is.
#[test]
fn struct_with_a_cross_package_field_drops_conditional_derives() {
    let field = named_field("speed", 1, "veh.other.Speed", false, init_value(true, None));
    let telemetry = public_decl(
        "Telemetry",
        v2::decl::Kind::StructDef(v2::StructDef {
            members: vec![field_member(field)],
            fixed_layout: false,
        }),
    );
    let source = rust_for(vec![telemetry]);
    let derived = derives_of(&source, "pub struct Telemetry {");
    assert_always_derived(&derived, "Telemetry");
    assert_eq!(derived, ["Debug", "Clone", "PartialEq"]);
}

/// A collection field keeps `Eq` but never `Copy`: the emitted Rust is a
/// `Vec`, which is not `Copy`.
#[test]
fn a_collection_field_keeps_eq_and_loses_copy() {
    let bag = public_decl(
        "Bag",
        v2::decl::Kind::StructDef(v2::StructDef {
            members: vec![field_member(array_field("counts", "Counter", 2, 8))],
            fixed_layout: false,
        }),
    );
    let source = rust_for(vec![counter_decl(), bag]);
    let derived = derives_of(&source, "pub struct Bag {");
    assert_always_derived(&derived, "Bag");
    assert_eq!(derived, ["Debug", "Clone", "PartialEq", "Eq", "Hash"]);
}

/// A reserved tombstone emits no field (typl §7.4), so it constrains nothing.
///
/// The fixture is integer-only on purpose. `struct_with_optional_and_reserved`
/// also holds a tombstone, but it holds a `String` leaf and an `f64` leaf as
/// well, so its struct already meets to no conditional derive and a tombstone
/// that contributed `Eligibility::NONE` would change nothing there. Here the
/// fields alone permit `Copy`, `Eq` and `Hash`, so the tombstone is the only
/// thing that could take them away.
#[test]
fn a_reserved_tombstone_does_not_constrain_the_derives() {
    let ledger = public_decl(
        "Ledger",
        v2::decl::Kind::StructDef(v2::StructDef {
            members: vec![
                field_member(named_field(
                    "count",
                    1,
                    "Counter",
                    false,
                    init_value(true, None),
                )),
                reserved_member(2, "legacyChecksum"),
                field_member(named_field(
                    "total",
                    3,
                    "Counter",
                    false,
                    init_value(true, None),
                )),
            ],
            fixed_layout: false,
        }),
    );
    let source = rust_for(vec![counter_decl(), ledger]);
    let derived = derives_of(&source, "pub struct Ledger {");
    assert_always_derived(&derived, "Ledger");
    assert_eq!(
        derived,
        ["Debug", "Clone", "Copy", "PartialEq", "Eq", "Hash"],
        "a reserved tombstone must not take a conditional derive away"
    );
}

/// A map field at the given key and value type, holding up to 32 entries.
fn map_field(name: &str, key: &str, value: &str) -> v2::Field {
    let named = |type_ref: &str| {
        Some(Box::new(v2::FieldType {
            optional: false,
            kind: Some(v2::field_type::Kind::Named(type_ref.to_string())),
        }))
    };
    v2::Field {
        r#type: Some(v2::FieldType {
            optional: false,
            kind: Some(v2::field_type::Kind::Map(Box::new(v2::MapType {
                key: named(key),
                value: named(value),
                min: 0,
                max: 32,
            }))),
        }),
        ..named_field(name, 1, "", false, init_value(true, None))
    }
}

/// A map emits `Vec<(K, V)>`, which is not `Copy` however `Copy` its halves
/// are. The fixture's key and value are both integer-backed, so the leaves
/// alone would permit `Copy`; the map position is what refuses it.
#[test]
fn a_map_field_loses_copy() {
    let table = public_decl(
        "Table",
        v2::decl::Kind::StructDef(v2::StructDef {
            members: vec![field_member(map_field("meta", "Counter", "Counter"))],
            fixed_layout: false,
        }),
    );
    let source = rust_for(vec![counter_decl(), table]);
    assert!(
        source.contains("Vec<(Counter, Counter)>"),
        "the map must emit the Vec form this test reasons about, got:\n{source}"
    );
    let derived = derives_of(&source, "pub struct Table {");
    assert_always_derived(&derived, "Table");
    assert_eq!(
        derived,
        ["Debug", "Clone", "PartialEq", "Eq", "Hash"],
        "a map field must lose Copy and keep the equality pair"
    );
}

/// A map whose value reaches a float loses `Eq` and `Hash`: `f64` has neither,
/// and `Vec<(K, V)>` has them only when both halves do. The key is
/// integer-backed, so the float reaches the struct through the value half
/// alone.
#[test]
fn a_map_whose_value_reaches_a_float_loses_eq() {
    let table = public_decl(
        "Table",
        v2::decl::Kind::StructDef(v2::StructDef {
            members: vec![field_member(map_field("meta", "Counter", "Speed"))],
            fixed_layout: false,
        }),
    );
    let source = rust_for(vec![speed_decl(), counter_decl(), table]);
    let derived = derives_of(&source, "pub struct Table {");
    assert_always_derived(&derived, "Table");
    assert_eq!(
        derived,
        ["Debug", "Clone", "PartialEq"],
        "a map whose value reaches a float must lose Eq and Hash"
    );
}

/// A map whose **key** reaches a float loses `Eq` and `Hash`, for the same
/// reason the value half does. This is the mirror of
/// [`a_map_whose_value_reaches_a_float_loses_eq`], and it is a separate test
/// because both halves of `eq: key.eq && value.eq` need pinning: with only
/// the value case covered, dropping `key.eq` passes the whole suite.
///
/// The shape is reachable from a typl source. `Checker::lower_map_key`
/// accepts a bare primitive key under TYPL-209, so a float key lowers with no
/// diagnostic, and deriving `Eq` on the resulting `Vec<(f64, Counter)>` would
/// not compile.
#[test]
fn a_map_whose_key_reaches_a_float_loses_eq() {
    let table = public_decl(
        "KeyTable",
        v2::decl::Kind::StructDef(v2::StructDef {
            members: vec![field_member(map_field("meta", "Speed", "Counter"))],
            fixed_layout: false,
        }),
    );
    let source = rust_for(vec![speed_decl(), counter_decl(), table]);
    assert!(
        source.contains("Vec<(Speed, Counter)>"),
        "the fixture must emit the map shape it reasons about, got:\n{source}"
    );
    let derived = derives_of(&source, "pub struct KeyTable {");
    assert_always_derived(&derived, "KeyTable");
    assert_eq!(
        derived,
        ["Debug", "Clone", "PartialEq"],
        "a map whose key reaches a float must lose Eq and Hash"
    );
}

/// A cyclic IR takes no conditional derive.
///
/// A cycle is TYPL-206 upstream, but this pass does not trust that gate: on a
/// repeat visit it returns `Eligibility::NONE` rather than recursing forever.
/// [`recursive_struct_default_terminates`] pins that the recursion terminates;
/// this pins what it terminates *with*, which a guard returning
/// `Eligibility::ALL` would also satisfy.
#[test]
fn a_cyclic_struct_takes_no_conditional_derives() {
    let recursive = v2::StructDef {
        members: vec![field_member(named_field(
            "next",
            1,
            "S",
            false,
            init_value(true, None),
        ))],
        fixed_layout: false,
    };
    let source = rust_for(vec![public_decl("S", v2::decl::Kind::StructDef(recursive))]);
    let derived = derives_of(&source, "pub struct S {");
    assert_always_derived(&derived, "S");
    assert_eq!(
        derived,
        ["Debug", "Clone", "PartialEq"],
        "a cycle must refuse every conditional derive"
    );
}

/// A `Stream` position refuses every conditional derive.
///
/// A stream is an interaction-position type (ridl §12.3) and never reaches a
/// struct field in checked IR. The fixture is built by hand to exercise the
/// backend's totality over an IR it did not lower itself: the field emits
/// `()`, and the conservative answer is what the arm returns.
#[test]
fn a_stream_field_takes_no_conditional_derives() {
    let feed = public_decl(
        "Feed",
        v2::decl::Kind::StructDef(v2::StructDef {
            members: vec![field_member(shaped_field(
                "items",
                1,
                v2::field_type::Kind::Stream(v2::StreamType {
                    element: Some(v2::stream_type::Element::Primitive(
                        v2::PrimitiveType::String as i32,
                    )),
                }),
            ))],
            fixed_layout: false,
        }),
    );
    let source = rust_for(vec![feed]);
    let derived = derives_of(&source, "pub struct Feed {");
    assert_always_derived(&derived, "Feed");
    assert_eq!(
        derived,
        ["Debug", "Clone", "PartialEq"],
        "a stream position must refuse every conditional derive"
    );
}

/// An `Unspecified` field primitive refuses every conditional derive.
///
/// This is the field-primitive path only. An `Unspecified` *backing* never
/// reaches it: `backing_scalar` maps an unspecified primitive backing to
/// [`ScalarBacking::Bytes`], so such a type emits `Vec<u8>` and is
/// `Eq` and `Hash` without being `Copy` — which is what
/// [`string_backed_scalar_is_not_copy`] already covers for the other
/// allocating backing.
///
/// [`ScalarBacking::Bytes`]: super::ScalarBacking::Bytes
#[test]
fn an_unspecified_field_primitive_takes_no_conditional_derives() {
    let hole = public_decl(
        "Hole",
        v2::decl::Kind::StructDef(v2::StructDef {
            members: vec![field_member(shaped_field(
                "nothing",
                1,
                v2::field_type::Kind::Primitive(v2::PrimitiveType::Unspecified as i32),
            ))],
            fixed_layout: false,
        }),
    );
    let source = rust_for(vec![hole]);
    let derived = derives_of(&source, "pub struct Hole {");
    assert_always_derived(&derived, "Hole");
    assert_eq!(
        derived,
        ["Debug", "Clone", "PartialEq"],
        "an Unspecified field primitive must refuse every conditional derive"
    );
}

/// A **fixed** array refuses `Copy` too, and that is a policy rather than a
/// soundness rule: `[Counter; 4]` is `Copy` in Rust, so deriving it would
/// compile. The backend refuses it anyway, so that `Copy` never depends on the
/// array bounds being equal — a rule no reader of the generated crate could
/// predict from the declaration.
///
/// [`a_collection_field_keeps_eq_and_loses_copy`] covers the bounded form,
/// where `Vec<T>` makes the refusal a soundness rule instead.
#[test]
fn a_fixed_array_field_loses_copy_by_policy() {
    let readings = public_decl(
        "Readings",
        v2::decl::Kind::StructDef(v2::StructDef {
            members: vec![field_member(array_field("counts", "Counter", 4, 4))],
            fixed_layout: false,
        }),
    );
    let source = rust_for(vec![counter_decl(), readings]);
    assert!(
        source.contains("[Counter; 4]"),
        "the array must emit the fixed form this test reasons about, got:\n{source}"
    );
    let derived = derives_of(&source, "pub struct Readings {");
    assert_always_derived(&derived, "Readings");
    assert_eq!(
        derived,
        ["Debug", "Clone", "PartialEq", "Eq", "Hash"],
        "a fixed array must lose Copy even though [T; N] would permit it"
    );
}

/// `Default` is never derived (design decision 8): it comes from the typl init
/// value through `defaults.rs`, which may be a declared `= 0.5` that
/// `#[derive(Default)]` would silently replace with the backing's zero.
///
/// The fixture covers every declaration kind that receives a derive attribute
/// — `type`, `struct`, `enum`, `enum set` and `union` — not just the scalars.
/// `defaults.rs` emits an `impl Default` for several of them, so a rule that
/// derived `Default` on the composites while sparing the scalars would be an
/// easy thing to write and would otherwise go unnoticed here.
#[test]
fn default_is_never_derived() {
    let tally = public_decl(
        "Tally",
        v2::decl::Kind::StructDef(v2::StructDef {
            members: vec![field_member(named_field(
                "count",
                1,
                "Counter",
                false,
                init_value(true, None),
            ))],
            fixed_layout: false,
        }),
    );
    let outcome = public_decl(
        "Outcome",
        v2::decl::Kind::UnionDef(v2::UnionDef {
            arms: vec![v2::UnionArm {
                name: "ok".to_string(),
                ordinal: 1,
                type_ref: "Tally".to_string(),
                doc: String::new(),
            }],
            is_result: false,
            reserved: Vec::new(),
        }),
    );
    let source = rust_for(vec![
        speed_decl(),
        counter_decl(),
        features_decl(),
        gear_position_decl(),
        tally,
        outcome,
    ]);
    for header in [
        "pub struct Speed(f64);",
        "pub struct Counter(i64);",
        "pub struct Features(i64);",
        "pub enum GearPosition {",
        "pub struct Tally {",
        "pub enum Outcome {",
    ] {
        let derived = derives_of(&source, header);
        assert!(
            !derived.iter().any(|d| d == "Default"),
            "`{header}` must not derive Default, got {derived:?}"
        );
    }
    // The positive companion: the Default the backend does emit is an impl
    // built from the init value, not a derive.
    for emitted in [
        "impl Default for Speed",
        "impl Default for GearPosition",
        "impl Default for Tally",
    ] {
        assert!(
            source.contains(emitted),
            "the init-value Default is still emitted (`{emitted}`), got:\n{source}"
        );
    }
}

/// An induced tuple struct carries its own derive attribute. It has to: the
/// struct that holds it derives `Debug`, `Clone` and `PartialEq`
/// unconditionally, and those derives do not compile unless the tuple struct
/// has them too.
///
/// The fixture is deliberately heterogeneous, and the tuple's answer is
/// deliberately different from its holder's. The tuple is an integer and a
/// float, so it is `Copy` and not `Eq` — neither `Eligibility::ALL` nor
/// `Eligibility::NONE`, so a `tuple_derive_attr` that ignored the eligibility
/// computation and returned a fixed answer could not pass. The holder adds a
/// `String`-backed field, so it is neither `Copy` nor `Eq`, and the two
/// attributes cannot be each other's.
#[test]
fn an_induced_tuple_struct_carries_its_derives() {
    let range = v2::Field {
        r#type: Some(v2::FieldType {
            optional: false,
            kind: Some(v2::field_type::Kind::Tuple(v2::TupleType {
                fields: vec![tuple_field("min", "Counter"), tuple_field("max", "Speed")],
            })),
        }),
        ..named_field("range", 1, "", false, init_value(true, None))
    };
    let label = public_decl("Label", bounded_string_type(init_value(true, Some(""))));
    let bounds = public_decl(
        "Bounds",
        v2::decl::Kind::StructDef(v2::StructDef {
            members: vec![
                field_member(range),
                field_member(named_field(
                    "name",
                    2,
                    "Label",
                    false,
                    init_value(true, None),
                )),
            ],
            fixed_layout: false,
        }),
    );
    let source = rust_for(vec![speed_decl(), counter_decl(), label, bounds]);
    let tuple = derives_of(&source, "pub struct BoundsRange {");
    assert_always_derived(&tuple, "BoundsRange");
    assert_eq!(
        tuple,
        ["Debug", "Clone", "Copy", "PartialEq"],
        "the tuple's own closure is an integer and a float: Copy, not Eq"
    );
    let outer = derives_of(&source, "pub struct Bounds {");
    assert_always_derived(&outer, "Bounds");
    assert_eq!(
        outer,
        ["Debug", "Clone", "PartialEq"],
        "the holder reaches a String as well as a float, so it takes neither"
    );
}

/// An enum set is `Copy`, and that is what lets a struct field holding one be
/// read through a shared reference.
///
/// `get` takes `self` by value (driftsys/ridl#433), so `warnings.flags.get()`
/// behind a `&Warnings` moves out of the borrow and rustc reports E0507
/// unless the enum set is `Copy`. #433 declined to fix that and recorded it as
/// this task's to cover, so the proof is a `rustc` run rather than a string
/// assertion: the string says `Copy` is in the list, the compile says the list
/// is enough.
#[test]
fn an_enum_set_in_a_struct_field_is_readable_through_a_shared_reference() {
    let warnings = public_decl(
        "Warnings",
        v2::decl::Kind::StructDef(v2::StructDef {
            members: vec![field_member(named_field(
                "flags",
                1,
                "Features",
                false,
                init_value(true, None),
            ))],
            fixed_layout: false,
        }),
    );
    let generated = rust_for(vec![features_decl(), warnings]);
    let derived = derives_of(&generated, "pub struct Features(i64);");
    assert_always_derived(&derived, "Features");
    assert!(
        derived.iter().any(|d| d == "Copy"),
        "an enum set must be Copy, got {derived:?}"
    );

    let source = format!(
        "{generated}\n{}",
        r#"
pub fn read_through_a_shared_reference(warnings: &Warnings) -> i64 {
    warnings.flags.get()
}
"#
    );

    let dir = tempfile::tempdir().expect("a temp dir is created");
    let source_path = dir.path().join("enum_set_read.rs");
    let meta_path = dir.path().join("enum_set_read.rmeta");
    std::fs::write(&source_path, &source).expect("the generated source is written");
    let rlib = ridl_rt_rlib(dir.path());
    let status = std::process::Command::new("rustc")
        .args([
            "--edition",
            "2024",
            "--crate-type",
            "lib",
            "--emit",
            "metadata",
        ])
        .arg("-o")
        .arg(&meta_path)
        .arg("--extern")
        .arg(format!("ridl_rt={}", rlib.display()))
        .arg(&source_path)
        .status()
        .expect("rustc must be installed and runnable for this test to be meaningful");
    assert!(
        status.success(),
        "an enum set held in a struct field must be readable through a shared \
         reference, source:\n{source}"
    );
}

/// The derive attribute is placed under the declaration's doc comment, not
/// above it. A naive prepend in `emit_decl` renders it above, which is
/// backwards from how Rust is written everywhere else.
#[test]
fn the_derive_attribute_sits_under_the_doc_comment() {
    let source = rust_for(vec![speed_decl()]);
    let doc_at = source
        .find("/// Vehicle speed over ground")
        .expect("the doc comment is emitted");
    let derive_at = source.find("#[derive(").expect("the derive is emitted");
    assert!(
        doc_at < derive_at,
        "the doc comment must come first, got:\n{source}"
    );
    let repr_at = source
        .find("#[repr(transparent)]")
        .expect("the repr is emitted");
    assert!(
        derive_at < repr_at,
        "the derive must precede the repr, got:\n{source}"
    );
}

// ---------------------------------------------------------------------------
// The FlatBuffers size bound's refusal (design note D-7, stages K4 and K5).
//
// `check_flatbuffers_bound` is called per type by the codec emitter (stage
// K5). These tests call it directly over hand-built IR, because no typl
// source reaches an unbounded type — D-7's diagnostic is totality over the
// IR, not a case a user meets.
// ---------------------------------------------------------------------------

/// Runs the per-type refusal over every declaration of `package`, which is
/// what the codec emitter does one declaration at a time, and returns the
/// first refusal.
fn check_flatbuffers_bounds(package: &v2::Package) -> Result<(), super::GenerateError> {
    let ctx = Ctx::new(package);
    for decl in &package.decls {
        check_flatbuffers_bound(&ctx, package, decl)?;
    }
    Ok(())
}

/// A struct whose only field is a bounded named scalar sizes to a finite
/// buffer: this is the shape the corpus proves after driftsys/ridl#459, and
/// the refusal must not fire on it.
#[test]
fn flatbuffers_bound_accepts_a_bounded_struct() {
    let holder = v2::StructDef {
        members: vec![field_member(named_field(
            "speed",
            1,
            "Speed",
            false,
            init_value(true, None),
        ))],
        fixed_layout: false,
    };
    let pkg = package(
        "veh.common",
        vec![
            speed_decl(),
            public_decl("Holder", v2::decl::Kind::StructDef(holder)),
        ],
    );
    assert_eq!(
        check_flatbuffers_bounds(&pkg),
        Ok(()),
        "a struct with only bounded fields must not be refused"
    );
}

/// A bare `string` map key with no length bound has no finite FlatBuffers
/// size — [`ridl_ir::projection::flatbuffers::max_size`]'s own doc lists it
/// first among the `None` cases, and
/// `ridl_ir::projection::flatbuffers::tests::a_bare_string_map_key_has_no_bound`
/// pins the fact this test pins the refusal over. typl never hands the
/// compiler one: §4.4–§4.5 default a bare `string` or `bytes` at a map key to
/// `[0..256]` (TYPL-103, driftsys/ridl#459), so this is IR built directly,
/// the same as every other fixture in this module that documents "the
/// checker rejects this, but the backend must not trust that gate".
#[test]
fn flatbuffers_bound_refuses_a_bare_string_map_key() {
    let holder = v2::StructDef {
        members: vec![field_member(shaped_field(
            "byId",
            1,
            v2::field_type::Kind::Map(Box::new(v2::MapType {
                key: Some(Box::new(v2::FieldType {
                    optional: false,
                    kind: Some(v2::field_type::Kind::Primitive(
                        v2::PrimitiveType::String as i32,
                    )),
                })),
                value: Some(Box::new(v2::FieldType {
                    optional: false,
                    kind: Some(v2::field_type::Kind::Primitive(
                        v2::PrimitiveType::Boolean as i32,
                    )),
                })),
                min: 0,
                max: 8,
            })),
        ))],
        fixed_layout: false,
    };
    let pkg = package(
        "veh.cruise",
        vec![public_decl("Holder", v2::decl::Kind::StructDef(holder))],
    );
    let err = check_flatbuffers_bounds(&pkg).expect_err("a bare string map key has no bound");
    assert_eq!(
        err.message, "`veh.cruise.Holder.byId` has no finite FlatBuffers bound",
        "the refusal must name the package, the declaration, and the member"
    );
}

/// The unbounded member is named even when a sibling member is one this
/// backend cannot judge: the cross-package exemption is per-member, not
/// per-declaration, so it must not swallow a genuine refusal over another
/// member of the same struct.
#[test]
fn flatbuffers_bound_names_the_unbounded_member_beside_an_exempt_one() {
    let holder = v2::StructDef {
        members: vec![
            field_member(shaped_field(
                "byId",
                1,
                v2::field_type::Kind::Map(Box::new(v2::MapType {
                    key: Some(Box::new(v2::FieldType {
                        optional: false,
                        kind: Some(v2::field_type::Kind::Primitive(
                            v2::PrimitiveType::String as i32,
                        )),
                    })),
                    value: Some(Box::new(v2::FieldType {
                        optional: false,
                        kind: Some(v2::field_type::Kind::Primitive(
                            v2::PrimitiveType::Boolean as i32,
                        )),
                    })),
                    min: 0,
                    max: 8,
                })),
            )),
            field_member(named_field(
                "speed",
                2,
                "veh.other.Speed",
                false,
                init_value(true, None),
            )),
        ],
        fixed_layout: false,
    };
    let pkg = package(
        "veh.cruise",
        vec![public_decl("Holder", v2::decl::Kind::StructDef(holder))],
    );
    let err = check_flatbuffers_bounds(&pkg)
        .expect_err("the unbounded member must still be refused beside an exempt one");
    assert_eq!(
        err.message, "`veh.cruise.Holder.byId` has no finite FlatBuffers bound",
        "the cross-package field must not exempt the whole declaration"
    );
}

/// The reverse order: the exempt member comes first in declaration order,
/// which must not short-circuit the walk before the unbounded member is
/// reached.
#[test]
fn flatbuffers_bound_names_the_unbounded_member_after_an_exempt_one() {
    let holder = v2::StructDef {
        members: vec![
            field_member(named_field(
                "speed",
                1,
                "veh.other.Speed",
                false,
                init_value(true, None),
            )),
            field_member(shaped_field(
                "byId",
                2,
                v2::field_type::Kind::Map(Box::new(v2::MapType {
                    key: Some(Box::new(v2::FieldType {
                        optional: false,
                        kind: Some(v2::field_type::Kind::Primitive(
                            v2::PrimitiveType::String as i32,
                        )),
                    })),
                    value: Some(Box::new(v2::FieldType {
                        optional: false,
                        kind: Some(v2::field_type::Kind::Primitive(
                            v2::PrimitiveType::Boolean as i32,
                        )),
                    })),
                    min: 0,
                    max: 8,
                })),
            )),
        ],
        fixed_layout: false,
    };
    let pkg = package(
        "veh.cruise",
        vec![public_decl("Holder", v2::decl::Kind::StructDef(holder))],
    );
    let err = check_flatbuffers_bounds(&pkg)
        .expect_err("the unbounded member must still be refused after an exempt one");
    assert_eq!(
        err.message, "`veh.cruise.Holder.byId` has no finite FlatBuffers bound",
        "declaration order must not change the attribution"
    );
}

/// A union arm that is individually unbounded is named the same way a
/// struct field is.
#[test]
fn flatbuffers_bound_names_the_unbounded_union_arm() {
    // `Reading` is a named scalar (a `TypeDef`), not a struct or a union, so
    // `fb_projection::mints_root_table` is false for it and the outer walk
    // in `check_flatbuffers_bounds` never checks it on its own — only the
    // union arm that names it does, through `probe_union_arm`. That is what
    // this test needs to isolate: an arm whose own reference is unbounded,
    // with nothing else in the package that would independently refuse it
    // first.
    let bad_arm = v2::UnionArm {
        name: "reading".to_string(),
        ordinal: 1,
        type_ref: "Reading".to_string(),
        doc: String::new(),
    };
    let union_def = v2::UnionDef {
        arms: vec![bad_arm],
        is_result: false,
        reserved: Vec::new(),
    };
    let pkg = package(
        "veh.cruise",
        vec![
            public_decl(
                "Reading",
                primitive_type(v2::PrimitiveType::String, init_value(false, None), None),
            ),
            public_decl("Outcome", v2::decl::Kind::UnionDef(union_def)),
        ],
    );
    let err = check_flatbuffers_bounds(&pkg).expect_err("an unbounded union arm has no bound");
    assert_eq!(
        err.message, "`veh.cruise.Outcome.reading` has no finite FlatBuffers bound",
        "the refusal must name the union's arm"
    );
}

/// A same-package cycle is left alone even beside an unrelated bounded
/// member: the cycle exemption is per-member, and it must not stop the walk
/// from checking the rest of the struct's own fields (there are none of
/// concern here, so this pins that the walk does not error over the cycle
/// alone).
///
/// The cycle exemption beside an *unbounded* member is pinned separately, by
/// [`flatbuffers_bound_names_the_unbounded_member_beside_a_cycle`] — the
/// third gap design note §4a left to stage K5.
#[test]
fn flatbuffers_bound_leaves_a_cycle_alone_beside_a_bounded_member() {
    let recursive = v2::StructDef {
        members: vec![
            field_member(named_field("next", 1, "S", false, init_value(true, None))),
            field_member(named_field(
                "speed",
                2,
                "Speed",
                false,
                init_value(true, None),
            )),
        ],
        fixed_layout: false,
    };
    let pkg = package(
        "veh.common",
        vec![
            speed_decl(),
            public_decl("S", v2::decl::Kind::StructDef(recursive)),
        ],
    );
    assert_eq!(
        check_flatbuffers_bounds(&pkg),
        Ok(()),
        "a cycle beside a bounded member must not be refused"
    );
}

/// A same-package cycle beside a genuinely unbounded member refuses over the
/// unbounded one and names it. The cycle is exempted, the sibling is not.
///
/// This is the third gap design note §4a carried forward: the path was
/// believed correct and was covered by no test.
#[test]
fn flatbuffers_bound_names_the_unbounded_member_beside_a_cycle() {
    let recursive = v2::StructDef {
        members: vec![
            field_member(named_field("next", 1, "S", false, init_value(true, None))),
            // A bare `string` at a map key, with no length bound: the one
            // shape this module already uses for "unbounded".
            field_member(shaped_field(
                "byId",
                2,
                v2::field_type::Kind::Map(Box::new(v2::MapType {
                    key: Some(Box::new(v2::FieldType {
                        optional: false,
                        kind: Some(v2::field_type::Kind::Primitive(
                            v2::PrimitiveType::String as i32,
                        )),
                    })),
                    value: Some(Box::new(v2::FieldType {
                        optional: false,
                        kind: Some(v2::field_type::Kind::Primitive(
                            v2::PrimitiveType::Boolean as i32,
                        )),
                    })),
                    min: 0,
                    max: 8,
                })),
            )),
        ],
        fixed_layout: false,
    };
    let pkg = package(
        "veh.common",
        vec![public_decl("S", v2::decl::Kind::StructDef(recursive))],
    );
    let err = check_flatbuffers_bounds(&pkg).expect_err("the map key has no bound");
    assert_eq!(
        err.message, "`veh.common.S.byId` has no finite FlatBuffers bound",
        "the cycle is exempted and the unbounded sibling is still named"
    );
}

/// An anonymous composite that mixes a leaf this backend cannot judge with
/// one that is genuinely unbounded is refused over the unbounded leaf.
///
/// This is the first gap design note §4a carried forward. The exemption used
/// to answer for the whole member: `member_resolves_locally` reached the
/// cross-package key, said `false`, and the bare `string` value inside the
/// same map was never probed. `judge` now descends into an array, a map and a
/// tuple rather than exempting one whole.
#[test]
fn flatbuffers_bound_names_an_unbounded_leaf_beside_an_unjudgeable_one() {
    let holder = v2::StructDef {
        members: vec![field_member(shaped_field(
            "byId",
            1,
            v2::field_type::Kind::Map(Box::new(v2::MapType {
                // Cross-package: this backend resolves none, so it cannot
                // judge the key.
                key: Some(Box::new(v2::FieldType {
                    optional: false,
                    kind: Some(v2::field_type::Kind::Named("veh.other.Speed".to_string())),
                })),
                // Bare `string`, no length bound: genuinely unbounded.
                value: Some(Box::new(v2::FieldType {
                    optional: false,
                    kind: Some(v2::field_type::Kind::Primitive(
                        v2::PrimitiveType::String as i32,
                    )),
                })),
                min: 0,
                max: 8,
            })),
        ))],
        fixed_layout: false,
    };
    let pkg = package(
        "veh.cruise",
        vec![public_decl("Holder", v2::decl::Kind::StructDef(holder))],
    );
    let err = check_flatbuffers_bounds(&pkg)
        .expect_err("an unbounded leaf beside an unjudgeable one is still unbounded");
    assert_eq!(
        err.message, "`veh.cruise.Holder.byId` has no finite FlatBuffers bound",
        "the member is named over its unbounded leaf, not exempted over its unjudgeable one"
    );
}

/// A collection whose **count alone** is unbounded is refused, even when its
/// element is one this backend cannot judge.
///
/// `judge` descends into an anonymous composite, but a collection carries a
/// bound of its own that its elements have nothing to do with:
/// `fb_projection::max_size` charges `count × element` and answers `None`
/// when the total overflows `u64` or exceeds `MAX_ENCODABLE`. A count of
/// 2^40 is over the ceiling at one byte an element, so it is over it
/// whatever the element turns out to be. Answering `Unjudgeable` for the
/// whole position would exempt this silently, while the same field over a
/// local element is refused.
#[test]
fn flatbuffers_bound_names_a_collection_whose_count_alone_is_unbounded() {
    let holder = v2::StructDef {
        members: vec![field_member(shaped_field(
            "readings",
            1,
            v2::field_type::Kind::Array(Box::new(v2::ArrayType {
                element: Some(Box::new(v2::FieldType {
                    optional: false,
                    kind: Some(v2::field_type::Kind::Named("veh.other.Speed".to_string())),
                })),
                min: 0,
                // Over `fb_projection::MAX_ENCODABLE` at one byte an element.
                max: 1 << 40,
            })),
        ))],
        fixed_layout: false,
    };
    let pkg = package(
        "veh.cruise",
        vec![public_decl("Holder", v2::decl::Kind::StructDef(holder))],
    );
    let err = check_flatbuffers_bounds(&pkg).expect_err("the count alone is over the ceiling");
    assert_eq!(
        err.message, "`veh.cruise.Holder.readings` has no finite FlatBuffers bound",
        "the count is judged even though the element is not"
    );
}

/// The same, one level down: an unbounded count inside a collection whose own
/// count is fine. Each level probes its own count, so nesting is covered by
/// the recursion rather than by a special case.
#[test]
fn flatbuffers_bound_names_a_nested_collection_whose_count_alone_is_unbounded() {
    let inner = v2::FieldType {
        optional: false,
        kind: Some(v2::field_type::Kind::Array(Box::new(v2::ArrayType {
            element: Some(Box::new(v2::FieldType {
                optional: false,
                kind: Some(v2::field_type::Kind::Named("veh.other.Speed".to_string())),
            })),
            min: 0,
            max: 1 << 40,
        }))),
    };
    let holder = v2::StructDef {
        members: vec![field_member(shaped_field(
            "grid",
            1,
            v2::field_type::Kind::Array(Box::new(v2::ArrayType {
                element: Some(Box::new(inner)),
                min: 0,
                max: 2,
            })),
        ))],
        fixed_layout: false,
    };
    let pkg = package(
        "veh.cruise",
        vec![public_decl("Holder", v2::decl::Kind::StructDef(holder))],
    );
    let err = check_flatbuffers_bounds(&pkg).expect_err("the inner count is over the ceiling");
    assert_eq!(
        err.message, "`veh.cruise.Holder.grid` has no finite FlatBuffers bound",
        "a nested count is judged the same way"
    );
}

/// A map's entry count is judged on the same footing as an array's.
#[test]
fn flatbuffers_bound_names_a_map_whose_entry_count_alone_is_unbounded() {
    let holder = v2::StructDef {
        members: vec![field_member(shaped_field(
            "byId",
            1,
            v2::field_type::Kind::Map(Box::new(v2::MapType {
                key: Some(Box::new(v2::FieldType {
                    optional: false,
                    kind: Some(v2::field_type::Kind::Named("veh.other.Key".to_string())),
                })),
                value: Some(Box::new(v2::FieldType {
                    optional: false,
                    kind: Some(v2::field_type::Kind::Named("veh.other.Speed".to_string())),
                })),
                min: 0,
                max: 1 << 40,
            })),
        ))],
        fixed_layout: false,
    };
    let pkg = package(
        "veh.cruise",
        vec![public_decl("Holder", v2::decl::Kind::StructDef(holder))],
    );
    let err = check_flatbuffers_bounds(&pkg).expect_err("the entry count is over the ceiling");
    assert_eq!(
        err.message, "`veh.cruise.Holder.byId` has no finite FlatBuffers bound",
        "a map's entry count is judged even though neither half is"
    );
}

/// The control for the three above: a collection with a count that fits and
/// an element this backend cannot judge stays exempt. Without it, the three
/// would pass for a `judge` that simply stopped exempting a collection.
#[test]
fn flatbuffers_bound_leaves_a_collection_with_a_bounded_count_alone() {
    let holder = v2::StructDef {
        members: vec![field_member(shaped_field(
            "readings",
            1,
            v2::field_type::Kind::Array(Box::new(v2::ArrayType {
                element: Some(Box::new(v2::FieldType {
                    optional: false,
                    kind: Some(v2::field_type::Kind::Named("veh.other.Speed".to_string())),
                })),
                min: 0,
                max: 4,
            })),
        ))],
        fixed_layout: false,
    };
    let pkg = package(
        "veh.cruise",
        vec![public_decl("Holder", v2::decl::Kind::StructDef(holder))],
    );
    assert_eq!(
        check_flatbuffers_bounds(&pkg),
        Ok(()),
        "a count that fits leaves the verdict to the element, which is unjudgeable"
    );
}

/// The same shape with **no** unbounded leaf beside the unjudgeable one is
/// still exempt. Without this, the test above would pass for a `judge` that
/// simply stopped exempting anything.
#[test]
fn flatbuffers_bound_leaves_an_unjudgeable_leaf_alone_when_nothing_beside_it_is_unbounded() {
    let holder = v2::StructDef {
        members: vec![field_member(shaped_field(
            "byId",
            1,
            v2::field_type::Kind::Map(Box::new(v2::MapType {
                key: Some(Box::new(v2::FieldType {
                    optional: false,
                    kind: Some(v2::field_type::Kind::Named("veh.other.Speed".to_string())),
                })),
                value: Some(Box::new(v2::FieldType {
                    optional: false,
                    kind: Some(v2::field_type::Kind::Primitive(
                        v2::PrimitiveType::Boolean as i32,
                    )),
                })),
                min: 0,
                max: 8,
            })),
        ))],
        fixed_layout: false,
    };
    let pkg = package(
        "veh.cruise",
        vec![public_decl("Holder", v2::decl::Kind::StructDef(holder))],
    );
    assert_eq!(
        check_flatbuffers_bounds(&pkg),
        Ok(()),
        "a cross-package leaf with nothing unbounded beside it is exempt"
    );
}

/// Two members on one ordinal is a layout error over the whole declaration,
/// and it is reported as one: every member probes as bounded, so an
/// attribution that only looked at members would call it an aggregate
/// overflow. The second gap design note §4a carried forward.
#[test]
fn flatbuffers_bound_reports_a_layout_error_over_the_declaration() {
    let holder = v2::StructDef {
        members: vec![
            field_member(named_field("a", 1, "Speed", false, init_value(true, None))),
            field_member(named_field("b", 1, "Speed", false, init_value(true, None))),
        ],
        fixed_layout: false,
    };
    let pkg = package(
        "veh.common",
        vec![
            speed_decl(),
            public_decl("Holder", v2::decl::Kind::StructDef(holder)),
        ],
    );
    let err = check_flatbuffers_bounds(&pkg).expect_err("two members on one ordinal");
    assert!(
        err.message
            .starts_with("`veh.common.Holder` has no FlatBuffers table layout:"),
        "a layout error is told apart from an aggregate overflow, got: {}",
        err.message
    );
    assert!(
        err.message.contains("two struct members with ordinal 1"),
        "the projection's own message is carried through, got: {}",
        err.message
    );
}

/// A member with no type at all is named as such rather than folded into the
/// aggregate case. The second gap design note §4a carried forward, second
/// cause.
#[test]
fn flatbuffers_bound_names_a_member_with_no_type() {
    let holder = v2::StructDef {
        members: vec![field_member(v2::Field {
            r#type: None,
            ..named_field("mystery", 1, "Speed", false, init_value(true, None))
        })],
        fixed_layout: false,
    };
    let pkg = package(
        "veh.common",
        vec![public_decl("Holder", v2::decl::Kind::StructDef(holder))],
    );
    let err = check_flatbuffers_bounds(&pkg).expect_err("a member with no type has no bound");
    assert_eq!(
        err.message,
        "`veh.common.Holder.mystery` carries no type, so `veh.common.Holder` has no FlatBuffers \
         bound",
        "an untyped member is named, and told apart from an aggregate overflow"
    );
}

/// An `Unspecified` field primitive is exempted rather than refused, on the
/// same footing as a `Stream`: both emit `()`, both are charged nothing, and
/// `derives` lists the two side by side among its refusing positions.
#[test]
fn flatbuffers_bound_leaves_an_unspecified_primitive_alone() {
    let hole = v2::StructDef {
        members: vec![field_member(shaped_field(
            "nothing",
            1,
            v2::field_type::Kind::Primitive(v2::PrimitiveType::Unspecified as i32),
        ))],
        fixed_layout: false,
    };
    let pkg = package(
        "veh.common",
        vec![public_decl("Hole", v2::decl::Kind::StructDef(hole))],
    );
    assert_eq!(
        check_flatbuffers_bounds(&pkg),
        Ok(()),
        "an unspecified field primitive must not be refused"
    );
}

/// A `Stream` field position is left alone, the same as an unresolved
/// reference: it never reaches a struct field in checked IR, and
/// `fb_projection::max_size` charges nothing for it, so a probe would answer
/// `None` for a case this backend does not consider a bound failure.
#[test]
fn flatbuffers_bound_leaves_a_stream_field_alone() {
    let feed = v2::StructDef {
        members: vec![field_member(shaped_field(
            "items",
            1,
            v2::field_type::Kind::Stream(v2::StreamType {
                element: Some(v2::stream_type::Element::Primitive(
                    v2::PrimitiveType::String as i32,
                )),
            }),
        ))],
        fixed_layout: false,
    };
    let pkg = package(
        "veh.common",
        vec![public_decl("Feed", v2::decl::Kind::StructDef(feed))],
    );
    assert_eq!(
        check_flatbuffers_bounds(&pkg),
        Ok(()),
        "a Stream field position must not be refused"
    );
}

/// When every individual member is bounded but their sum exceeds
/// `fb_projection::MAX_ENCODABLE`, the refusal names the declaration alone:
/// there is no single member to attribute an aggregate overflow to.
#[test]
fn flatbuffers_bound_names_the_declaration_when_the_cause_is_aggregate() {
    fn huge_integer_array(name: &str, ordinal: u32) -> v2::StructMember {
        field_member(shaped_field(
            name,
            ordinal,
            v2::field_type::Kind::Array(Box::new(v2::ArrayType {
                element: Some(Box::new(v2::FieldType {
                    optional: false,
                    kind: Some(v2::field_type::Kind::Primitive(
                        v2::PrimitiveType::Integer as i32,
                    )),
                })),
                min: 0,
                max: 300_000_000,
            })),
        ))
    }
    // Each array alone charges roughly 2.4 GB, well under
    // `fb_projection::MAX_ENCODABLE` (`u32::MAX`, ~4.29 GB); the two together
    // charge roughly 4.8 GB, over it.
    let holder = v2::StructDef {
        members: vec![huge_integer_array("a", 1), huge_integer_array("b", 2)],
        fixed_layout: false,
    };
    let pkg = package(
        "veh.cruise",
        vec![public_decl("Holder", v2::decl::Kind::StructDef(holder))],
    );
    let err = check_flatbuffers_bounds(&pkg).expect_err("the summed size exceeds MAX_ENCODABLE");
    assert_eq!(
        err.message,
        "`veh.cruise.Holder` has no finite FlatBuffers bound: every member is bounded on its own \
         and the total is not",
        "an aggregate cause must name the declaration alone, with no member"
    );
}

/// A struct that reaches a cross-package reference is left alone: this
/// backend resolves no cross-package reference itself
/// (`struct_with_a_cross_package_field_drops_conditional_derives` already
/// pins that `generate` still emits one), so `fb_projection::max_size` cannot
/// tell "unresolved here" from "unbounded", and `decl_resolves_locally`
/// answers `false` rather than letting it guess.
#[test]
fn flatbuffers_bound_leaves_a_cross_package_reference_alone() {
    let holder = v2::StructDef {
        members: vec![field_member(named_field(
            "speed",
            1,
            "veh.other.Speed",
            false,
            init_value(true, None),
        ))],
        fixed_layout: false,
    };
    let pkg = package(
        "veh.common",
        vec![public_decl("Holder", v2::decl::Kind::StructDef(holder))],
    );
    assert_eq!(
        check_flatbuffers_bounds(&pkg),
        Ok(()),
        "a cross-package reference must not be refused: this backend cannot judge it"
    );
}

/// A same-package struct that reaches itself is left alone, the same as a
/// cross-package reference: `recursive_struct_default_terminates` and
/// `a_cyclic_struct_takes_no_conditional_derives` already pin that `generate`
/// still emits `S`'s domain type over exactly this shape, and this refusal
/// must not take that away before K5 has a codec to withhold instead.
#[test]
fn flatbuffers_bound_leaves_a_cycle_alone() {
    let recursive = v2::StructDef {
        members: vec![field_member(named_field(
            "next",
            1,
            "S",
            false,
            init_value(true, None),
        ))],
        fixed_layout: false,
    };
    let pkg = package(
        "veh.common",
        vec![public_decl("S", v2::decl::Kind::StructDef(recursive))],
    );
    assert_eq!(
        check_flatbuffers_bounds(&pkg),
        Ok(()),
        "a same-package cycle must not be refused before K5 has a codec to withhold"
    );
}
