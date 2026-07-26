//! One test per row of the ADR-0008 decision 14 classification table, and both
//! directions of every directional row.
//!
//! Every case runs through [`diff_packages`](crate::diff_packages), not through
//! [`classify`](super::classify) alone, so each test also proves the verdict
//! flows from the walk through the classifier into the report.

use ridl_ir::v2;

use crate::{Category, Change, DiffReport, Verdict, diff_packages};

// --------------------------------------------------------------------------
// Builders.
// --------------------------------------------------------------------------

fn decl(name: &str, ordinal: u32, kind: v2::decl::Kind) -> v2::Decl {
    v2::Decl {
        name: name.to_string(),
        visibility: v2::Visibility::Public as i32,
        is_error: false,
        doc: String::new(),
        labels: Vec::new(),
        deprecated: None,
        ordinal,
        kind: Some(kind),
    }
}

fn signal(name: &str, ordinal: u32, payload: &str) -> v2::Decl {
    decl(
        name,
        ordinal,
        v2::decl::Kind::SignalDef(v2::SignalDef {
            payload: payload.to_string(),
            declared_init: None,
            init: None,
            timing: Some(range(Some("10000"), Some("100000"))),
        }),
    )
}

fn event(name: &str, ordinal: u32, payload: &str) -> v2::Decl {
    decl(
        name,
        ordinal,
        v2::decl::Kind::EventDef(v2::EventDef {
            payload: payload.to_string(),
            timing: Some(range(Some("10000"), Some("100000"))),
        }),
    )
}

fn reserved(name: &str, ordinal: u32) -> v2::Decl {
    v2::Decl {
        name: String::new(),
        visibility: v2::Visibility::Unspecified as i32,
        is_error: false,
        doc: String::new(),
        labels: Vec::new(),
        deprecated: None,
        ordinal,
        kind: Some(v2::decl::Kind::ReservedSlot(v2::Reserved {
            ordinal,
            name: Some(name.to_string()),
            value: None,
        })),
    }
}

fn range(min_us: Option<&str>, max_us: Option<&str>) -> v2::Timing {
    v2::Timing {
        mode: v2::TimingMode::Range as i32,
        min_us: min_us.map(str::to_string),
        max_us: max_us.map(str::to_string),
        default_applied: false,
    }
}

fn strict(period_us: &str) -> v2::Timing {
    v2::Timing {
        mode: v2::TimingMode::StrictPeriodic as i32,
        min_us: Some(period_us.to_string()),
        max_us: Some(period_us.to_string()),
        default_applied: false,
    }
}

fn named(name: &str) -> v2::FieldType {
    v2::FieldType {
        optional: false,
        kind: Some(v2::field_type::Kind::Named(name.to_string())),
    }
}

fn stream(element: &str) -> v2::FieldType {
    v2::FieldType {
        optional: false,
        kind: Some(v2::field_type::Kind::Stream(v2::StreamType {
            element: Some(v2::stream_type::Element::Named(element.to_string())),
        })),
    }
}

fn param(name: &str, r#type: v2::FieldType) -> v2::Param {
    v2::Param {
        name: name.to_string(),
        r#type: Some(r#type),
    }
}

fn clause(kind: v2::ContractKind, source: &str) -> v2::Contract {
    v2::Contract {
        kind: kind as i32,
        source: source.to_string(),
        signal_refs: Vec::new(),
        param_refs: Vec::new(),
        uses_result: false,
        observer_id: String::new(),
    }
}

fn command(
    name: &str,
    ordinal: u32,
    params: Vec<v2::Param>,
    contracts: Vec<v2::Contract>,
) -> v2::Decl {
    decl(
        name,
        ordinal,
        v2::decl::Kind::CommandDef(v2::CommandDef { params, contracts }),
    )
}

fn query(
    name: &str,
    ordinal: u32,
    params: Vec<v2::Param>,
    return_type: v2::ReturnType,
    contracts: Vec<v2::Contract>,
) -> v2::Decl {
    decl(
        name,
        ordinal,
        v2::decl::Kind::QueryDef(v2::QueryDef {
            params,
            return_type: Some(return_type),
            contracts,
        }),
    )
}

fn value_return(name: &str) -> v2::ReturnType {
    v2::ReturnType {
        kind: Some(v2::return_type::Kind::Value(named(name))),
    }
}

fn fallible_return(ok: &str, err: &str) -> v2::ReturnType {
    v2::ReturnType {
        kind: Some(v2::return_type::Kind::Fallible(v2::FallibleType {
            ok: ok.to_string(),
            err: err.to_string(),
        })),
    }
}

fn interface(name: &str, interactions: Vec<v2::Decl>) -> v2::Interface {
    v2::Interface {
        name: name.to_string(),
        visibility: v2::Visibility::Public as i32,
        doc: String::new(),
        labels: Vec::new(),
        deprecated: None,
        interactions,
    }
}

/// A package holding one interface named `I`.
fn pkg(interactions: Vec<v2::Decl>) -> v2::Package {
    v2::Package {
        name: "veh.cluster".to_string(),
        decls: Vec::new(),
        interfaces: vec![interface("I", interactions)],
        services: Vec::new(),
    }
}

/// A package holding one declaration and no interfaces.
fn decl_pkg(decls: Vec<v2::Decl>) -> v2::Package {
    v2::Package {
        name: "veh.cluster".to_string(),
        decls,
        interfaces: Vec::new(),
        services: Vec::new(),
    }
}

fn scalar(name: &str, constraint: v2::Constraint, width: v2::IntWidth) -> v2::Decl {
    decl(
        name,
        0,
        v2::decl::Kind::TypeDef(v2::TypeDef {
            backing: Some(v2::Backing {
                kind: Some(v2::backing::Kind::Primitive(
                    v2::PrimitiveType::Integer as i32,
                )),
            }),
            constraint: Some(constraint),
            declared_init: None,
            init: None,
            width: Some(v2::type_def::Width::IntWidth(width as i32)),
        }),
    )
}

/// The same scalar with no constraint block at all. `None` and an
/// all-facets-unset `Some` are different states to the classifier: only `None`
/// says the value was never bounded.
fn unconstrained(name: &str, width: v2::IntWidth) -> v2::Decl {
    let mut decl = scalar(name, bounds(None, None), width);
    let Some(v2::decl::Kind::TypeDef(def)) = &mut decl.kind else {
        unreachable!("scalar builds a TypeDef")
    };
    def.constraint = None;
    decl
}

/// The same scalar carrying a declared init value. A named type's init is a
/// separate emission site from a signal's, in `diff_type_def`.
fn scalar_with_init(name: &str, width: v2::IntWidth, init: &str) -> v2::Decl {
    let mut decl = scalar(name, bounds(Some("0.0"), Some("250.0")), width);
    let Some(v2::decl::Kind::TypeDef(def)) = &mut decl.kind else {
        unreachable!("scalar builds a TypeDef")
    };
    def.declared_init = Some(init.to_string());
    decl
}

/// An all-facets-unset constraint, so each test states only the facet it moves.
fn bounds(min: Option<&str>, max: Option<&str>) -> v2::Constraint {
    v2::Constraint {
        min: min.map(str::to_string),
        max: max.map(str::to_string),
        step: None,
        len_min: None,
        len_max: None,
        pattern: None,
        pattern_const: None,
    }
}

fn enum_value(name: &str, value: i64) -> v2::EnumValue {
    v2::EnumValue {
        name: name.to_string(),
        value,
        doc: String::new(),
    }
}

fn enum_decl(name: &str, values: Vec<v2::EnumValue>, reserved: Vec<i64>) -> v2::Decl {
    decl(
        name,
        0,
        v2::decl::Kind::EnumDef(v2::EnumDef {
            values,
            reserved: reserved
                .into_iter()
                .map(|value| v2::Reserved {
                    ordinal: 0,
                    name: None,
                    value: Some(value),
                })
                .collect(),
        }),
    )
}

fn union_arm(name: &str, ordinal: u32, type_ref: &str) -> v2::UnionArm {
    v2::UnionArm {
        name: name.to_string(),
        ordinal,
        type_ref: type_ref.to_string(),
        doc: String::new(),
    }
}

fn union_decl(name: &str, arms: Vec<v2::UnionArm>, is_result: bool) -> v2::Decl {
    decl(
        name,
        0,
        v2::decl::Kind::UnionDef(v2::UnionDef {
            arms,
            is_result,
            reserved: Vec::new(),
        }),
    )
}

/// A union whose body also holds `reserved` tombstones at the given ordinals.
fn union_with_reserved(name: &str, arms: Vec<v2::UnionArm>, reserved: Vec<u32>) -> v2::Decl {
    let mut decl = union_decl(name, arms, false);
    let Some(v2::decl::Kind::UnionDef(def)) = &mut decl.kind else {
        unreachable!("union_decl builds a UnionDef")
    };
    def.reserved = reserved
        .into_iter()
        .map(|ordinal| v2::Reserved {
            ordinal,
            name: None,
            value: None,
        })
        .collect();
    decl
}

fn field(name: &str, ordinal: u32, type_ref: &str) -> v2::StructMember {
    v2::StructMember {
        member: Some(v2::struct_member::Member::Field(v2::Field {
            name: name.to_string(),
            ordinal,
            r#type: Some(named(type_ref)),
            declared_init: None,
            init: None,
            doc: String::new(),
            labels: Vec::new(),
            deprecated: None,
        })),
    }
}

/// A `reserved` tombstone in a struct body, holding its ordinal slot.
fn reserved_field(ordinal: u32) -> v2::StructMember {
    v2::StructMember {
        member: Some(v2::struct_member::Member::Reserved(v2::Reserved {
            ordinal,
            name: None,
            value: None,
        })),
    }
}

fn struct_decl(name: &str, members: Vec<v2::StructMember>) -> v2::Decl {
    decl(
        name,
        0,
        v2::decl::Kind::StructDef(v2::StructDef {
            members,
            fixed_layout: false,
        }),
    )
}

// --------------------------------------------------------------------------
// Assertions.
// --------------------------------------------------------------------------

/// The one change of `category` in the report, failing loudly when the walk
/// produced none or several — a rule row is only tested when the case it
/// describes is the case that reached the classifier.
#[track_caller]
fn row(report: &DiffReport, category: Category) -> &Change {
    let matching: Vec<&Change> = report
        .changes
        .iter()
        .filter(|change| change.category == category)
        .collect();
    assert_eq!(
        matching.len(),
        1,
        "expected exactly one {category:?}, got {:?}",
        report.changes
    );
    matching[0]
}

#[track_caller]
fn assert_row(old: &v2::Package, new: &v2::Package, category: Category, verdict: Verdict) {
    let report = diff_packages(old, new);
    let change = row(&report, category);
    assert_eq!(
        change.verdict, verdict,
        "{category:?} on {} classified {:?}, expected {verdict:?}",
        change.path, change.verdict
    );
}

// ==========================================================================
// Ordinals and wire identity.
// ==========================================================================

#[test]
fn an_interaction_appended_at_the_end_is_compatible() {
    let old = pkg(vec![signal("a", 1, "T")]);
    let new = pkg(vec![signal("a", 1, "T"), event("b", 2, "U")]);
    assert_row(
        &old,
        &new,
        Category::InteractionAppended,
        Verdict::Compatible,
    );
}

#[test]
fn an_interaction_inserted_before_the_end_is_breaking() {
    let old = pkg(vec![signal("a", 1, "T"), signal("c", 2, "T")]);
    let new = pkg(vec![
        signal("a", 1, "T"),
        signal("b", 2, "T"),
        signal("c", 3, "T"),
    ]);
    assert_row(&old, &new, Category::InteractionInserted, Verdict::Breaking);
}

#[test]
fn a_reordered_interaction_is_breaking() {
    let old = pkg(vec![signal("a", 1, "T"), signal("b", 2, "T")]);
    let new = pkg(vec![signal("b", 1, "T"), signal("a", 2, "T")]);
    let report = diff_packages(&old, &new);
    assert_eq!(report.verdict, Verdict::Breaking);
    assert!(
        report
            .changes
            .iter()
            .all(|change| change.category == Category::InteractionReordered
                && change.verdict == Verdict::Breaking),
        "a swap reorders both interactions, got {:?}",
        report.changes
    );
}

#[test]
fn an_interaction_removed_without_a_tombstone_is_breaking() {
    let old = pkg(vec![signal("a", 1, "T"), signal("b", 2, "T")]);
    let new = pkg(vec![signal("a", 1, "T")]);
    assert_row(&old, &new, Category::InteractionRemoved, Verdict::Breaking);
}

#[test]
fn an_interaction_retired_into_its_own_slot_is_compatible() {
    let old = pkg(vec![signal("a", 1, "T"), signal("b", 2, "T")]);
    let new = pkg(vec![signal("a", 1, "T"), reserved("b", 2)]);
    assert_row(
        &old,
        &new,
        Category::InteractionRetired,
        Verdict::Compatible,
    );
}

#[test]
fn redeclaring_under_a_reserved_name_is_breaking() {
    let old = pkg(vec![signal("a", 1, "T"), reserved("b", 2)]);
    let new = pkg(vec![signal("a", 1, "T"), signal("b", 2, "T")]);
    assert_row(
        &old,
        &new,
        Category::ReservedNameRedeclared,
        Verdict::Breaking,
    );
}

/// The carried finding from the task-16 review. `b` is removed with no
/// tombstone and `c` takes the ordinal it vacated. The walk labels `c`
/// `InteractionAppended` — it does sit after every surviving slot — but the
/// ordinal is a reused wire identity, which ADR-0008 decision 14 lists first
/// among breaking changes. The classifier must reach that verdict on the
/// appended change's own merits, not borrow it from the removal.
#[test]
fn an_ordinal_freed_by_an_untombstoned_removal_and_reused_is_breaking() {
    let old = pkg(vec![signal("a", 1, "T"), signal("b", 2, "T")]);
    let new = pkg(vec![signal("a", 1, "T"), signal("c", 2, "T")]);

    let report = diff_packages(&old, &new);
    let appended = row(&report, Category::InteractionAppended);
    assert_eq!(
        appended.path, "veh.cluster/I/c",
        "the reused slot is c's, got {:?}",
        report.changes
    );
    assert_eq!(
        appended.verdict,
        Verdict::Breaking,
        "reusing b's freed ordinal 2 is breaking on its own merits, got {:?}",
        report.changes
    );
    // The removal is independently breaking; the point of the test is that the
    // appended row does not depend on it.
    assert_eq!(
        row(&report, Category::InteractionRemoved).verdict,
        Verdict::Breaking
    );
}

// ==========================================================================
// Interaction kind, payload, params, return.
// ==========================================================================

#[test]
fn an_interaction_kind_change_from_signal_to_event_is_breaking() {
    let old = pkg(vec![signal("a", 1, "T")]);
    let new = pkg(vec![event("a", 1, "T")]);
    assert_row(&old, &new, Category::KindChanged, Verdict::Breaking);
}

#[test]
fn an_interaction_kind_change_from_event_to_signal_is_breaking() {
    let old = pkg(vec![event("a", 1, "T")]);
    let new = pkg(vec![signal("a", 1, "T")]);
    assert_row(&old, &new, Category::KindChanged, Verdict::Breaking);
}

#[test]
fn a_payload_type_change_is_breaking() {
    let old = pkg(vec![signal("a", 1, "T")]);
    let new = pkg(vec![signal("a", 1, "U")]);
    assert_row(&old, &new, Category::PayloadChanged, Verdict::Breaking);
}

#[test]
fn a_parameter_type_change_is_breaking() {
    let old = pkg(vec![command("c", 1, vec![param("p", named("T"))], vec![])]);
    let new = pkg(vec![command("c", 1, vec![param("p", named("U"))], vec![])]);
    assert_row(&old, &new, Category::ParamsChanged, Verdict::Breaking);
}

#[test]
fn a_return_type_change_is_breaking() {
    let old = pkg(vec![query("q", 1, vec![], value_return("T"), vec![])]);
    let new = pkg(vec![query("q", 1, vec![], value_return("U"), vec![])]);
    assert_row(&old, &new, Category::ReturnChanged, Verdict::Breaking);
}

#[test]
fn a_stream_added_on_a_parameter_is_breaking() {
    let old = pkg(vec![command("c", 1, vec![param("p", named("T"))], vec![])]);
    let new = pkg(vec![command("c", 1, vec![param("p", stream("T"))], vec![])]);
    assert_row(&old, &new, Category::ParamsChanged, Verdict::Breaking);
}

#[test]
fn a_stream_removed_from_a_return_is_breaking() {
    let streamed = v2::ReturnType {
        kind: Some(v2::return_type::Kind::Value(stream("T"))),
    };
    let old = pkg(vec![query("q", 1, vec![], streamed, vec![])]);
    let new = pkg(vec![query("q", 1, vec![], value_return("T"), vec![])]);
    assert_row(&old, &new, Category::ReturnChanged, Verdict::Breaking);
}

// ==========================================================================
// Wire width.
// ==========================================================================

#[test]
fn an_int_width_flip_is_breaking() {
    let old = decl_pkg(vec![scalar(
        "S",
        bounds(Some("0"), Some("250")),
        v2::IntWidth::U8,
    )]);
    let new = decl_pkg(vec![scalar(
        "S",
        bounds(Some("0"), Some("300")),
        v2::IntWidth::U16,
    )]);
    assert_row(&old, &new, Category::WidthChanged, Verdict::Breaking);
}

/// The same 64-bit footprint, a different signedness: still a wire-width flip.
#[test]
fn uint64_versus_int64_is_breaking() {
    let constraint = bounds(Some("0"), Some("1000"));
    let old = decl_pkg(vec![scalar("S", constraint.clone(), v2::IntWidth::U64)]);
    let new = decl_pkg(vec![scalar("S", constraint, v2::IntWidth::I64)]);
    assert_row(&old, &new, Category::WidthChanged, Verdict::Breaking);
}

// ==========================================================================
// Constraints — narrowed versus widened.
// ==========================================================================

/// Both sides keep the same declared width, so the case isolates the
/// constraint row from the width row.
fn constraint_case(
    old_bounds: v2::Constraint,
    new_bounds: v2::Constraint,
) -> (v2::Package, v2::Package) {
    (
        decl_pkg(vec![scalar("S", old_bounds, v2::IntWidth::U16)]),
        decl_pkg(vec![scalar("S", new_bounds, v2::IntWidth::U16)]),
    )
}

#[test]
fn a_raised_minimum_is_breaking() {
    let (old, new) = constraint_case(
        bounds(Some("0"), Some("250")),
        bounds(Some("10"), Some("250")),
    );
    assert_row(&old, &new, Category::ConstraintChanged, Verdict::Breaking);
}

#[test]
fn a_lowered_minimum_is_compatible() {
    let (old, new) = constraint_case(
        bounds(Some("10"), Some("250")),
        bounds(Some("0"), Some("250")),
    );
    assert_row(&old, &new, Category::ConstraintChanged, Verdict::Compatible);
}

#[test]
fn a_lowered_maximum_is_breaking() {
    let (old, new) = constraint_case(
        bounds(Some("0"), Some("250")),
        bounds(Some("0"), Some("200")),
    );
    assert_row(&old, &new, Category::ConstraintChanged, Verdict::Breaking);
}

#[test]
fn a_raised_maximum_is_compatible() {
    let (old, new) = constraint_case(
        bounds(Some("0"), Some("200")),
        bounds(Some("0"), Some("250")),
    );
    assert_row(&old, &new, Category::ConstraintChanged, Verdict::Compatible);
}

/// Negative bounds, which the whole suite otherwise never uses even though
/// `Temperature: Cel [-40.0..125.0]` is the first type in the repo's own
/// `appendix_b.typl`. Ordering below zero runs the other half of `cmp_decimal`:
/// magnitudes compare the same way but the result is reversed, and a bound pair
/// that straddles zero is decided by the sign alone.
#[test]
fn negative_bounds_order_by_sign_and_reversed_magnitude() {
    // -40.0 -> -10.0 raises the floor: every value between them was legal.
    let (old, new) = constraint_case(
        bounds(Some("-40.0"), Some("125.0")),
        bounds(Some("-10.0"), Some("125.0")),
    );
    assert_row(&old, &new, Category::ConstraintChanged, Verdict::Breaking);

    // The same pair the other way round only widens.
    let (old, new) = constraint_case(
        bounds(Some("-10.0"), Some("125.0")),
        bounds(Some("-40.0"), Some("125.0")),
    );
    assert_row(&old, &new, Category::ConstraintChanged, Verdict::Compatible);

    // Across zero, where the two bounds carry different signs and the larger
    // magnitude is the smaller number.
    let (old, new) = constraint_case(
        bounds(Some("-40.0"), Some("125.0")),
        bounds(Some("10.0"), Some("125.0")),
    );
    assert_row(&old, &new, Category::ConstraintChanged, Verdict::Breaking);
}

/// The length rule reads both ends. `a_tightened_length_is_breaking` holds
/// `len_min` at 1 and moves only `len_max`, so the raised-floor half is decided
/// by a term of its own.
#[test]
fn a_raised_minimum_length_is_breaking() {
    let length = |min: u64| {
        let mut constraint = bounds(None, None);
        constraint.len_min = Some(min);
        constraint.len_max = Some(20);
        constraint
    };
    let (old, new) = constraint_case(length(5), length(10));
    assert_row(&old, &new, Category::ConstraintChanged, Verdict::Breaking);
}

/// The row `--explain constraint_changed` documents verbatim: "a composite body
/// changed in place is breaking: the walk cannot say what moved inside it".
///
/// A struct field retyped keeps every member name, so the walk reports no
/// addition and no removal and falls through to a bodiless `ConstraintChanged`
/// on the type itself. The classifier then finds a `StructDef` where its
/// constraint rules expect a `TypeDef` and takes the unlisted-is-breaking path.
/// Both halves have to hold: with the walk's emission gone the change vanishes
/// from the report entirely, and with the classifier's fallback relaxed it
/// reports as compatible.
#[test]
fn a_composite_body_changed_in_place_is_breaking() {
    let old = decl_pkg(vec![struct_decl("Frame", vec![field("a", 1, "Speed")])]);
    let new = decl_pkg(vec![struct_decl("Frame", vec![field("a", 1, "Distance")])]);

    let report = diff_packages(&old, &new);
    let change = row(&report, Category::ConstraintChanged);
    assert_eq!(
        change.path, "veh.cluster/Frame",
        "the change is the body's, not a member's, got {:?}",
        report.changes
    );
    assert_eq!(change.verdict, Verdict::Breaking);
    assert_eq!(report.verdict, Verdict::Breaking);
}

/// Exact decimal ordering, not float ordering: `9.5` sits below `10.25`.
#[test]
fn decimal_bounds_order_exactly() {
    let (old, new) = constraint_case(
        bounds(Some("9.5"), Some("250.0")),
        bounds(Some("10.25"), Some("250.0")),
    );
    assert_row(&old, &new, Category::ConstraintChanged, Verdict::Breaking);
}

/// `type Speed: km/h` gaining `[0.0..300.0]` bounds a value that was unbounded,
/// so every consumer that was sending 320 is now sending an illegal value. There
/// is no facet to compare — the old side has no constraint at all — so this is
/// decided by the fallback of `narrows`, not by its per-facet rules.
#[test]
fn a_constraint_added_where_there_was_none_is_breaking() {
    let old = decl_pkg(vec![unconstrained("Speed", v2::IntWidth::U16)]);
    let new = decl_pkg(vec![scalar(
        "Speed",
        bounds(Some("0.0"), Some("300.0")),
        v2::IntWidth::U16,
    )]);
    assert_row(&old, &new, Category::ConstraintChanged, Verdict::Breaking);
}

/// The other direction of the same fallback: dropping the constraint entirely
/// leaves the value unbounded again, which only widens.
#[test]
fn a_constraint_dropped_entirely_is_compatible() {
    let old = decl_pkg(vec![scalar(
        "Speed",
        bounds(Some("0.0"), Some("300.0")),
        v2::IntWidth::U16,
    )]);
    let new = decl_pkg(vec![unconstrained("Speed", v2::IntWidth::U16)]);
    assert_row(&old, &new, Category::ConstraintChanged, Verdict::Compatible);
}

/// A bound the exact-decimal comparison cannot order — here an exponent form a
/// foreign or hand-edited `.ir.json` can carry, which `load_ir_json` accepts as
/// the plain string it is. `1e2` raises the floor from 10 to 100, a narrowing,
/// but the classifier does not evaluate exponents. It cannot prove the bound
/// relaxed, so it falls back to breaking rather than passing a bound it never
/// read as compatible.
#[test]
fn a_bound_the_classifier_cannot_order_is_breaking() {
    let (old, new) = constraint_case(
        bounds(Some("10"), Some("250")),
        bounds(Some("1e2"), Some("250")),
    );
    assert_row(&old, &new, Category::ConstraintChanged, Verdict::Breaking);
}

#[test]
fn a_step_added_is_breaking() {
    let mut stepped = bounds(Some("0"), Some("250"));
    stepped.step = Some("0.5".to_string());
    let (old, new) = constraint_case(bounds(Some("0"), Some("250")), stepped);
    assert_row(&old, &new, Category::ConstraintChanged, Verdict::Breaking);
}

#[test]
fn a_step_removed_is_compatible() {
    let mut stepped = bounds(Some("0"), Some("250"));
    stepped.step = Some("0.5".to_string());
    let (old, new) = constraint_case(stepped, bounds(Some("0"), Some("250")));
    assert_row(&old, &new, Category::ConstraintChanged, Verdict::Compatible);
}

#[test]
fn a_step_coarsened_is_breaking() {
    let step = |text: &str| {
        let mut constraint = bounds(Some("0"), Some("250"));
        constraint.step = Some(text.to_string());
        constraint
    };
    let (old, new) = constraint_case(step("0.5"), step("1.0"));
    assert_row(&old, &new, Category::ConstraintChanged, Verdict::Breaking);
}

#[test]
fn a_tightened_length_is_breaking() {
    let length = |max: u64| {
        let mut constraint = bounds(None, None);
        constraint.len_min = Some(1);
        constraint.len_max = Some(max);
        constraint
    };
    let (old, new) = constraint_case(length(64), length(32));
    assert_row(&old, &new, Category::ConstraintChanged, Verdict::Breaking);
}

#[test]
fn a_loosened_length_is_compatible() {
    let length = |max: u64| {
        let mut constraint = bounds(None, None);
        constraint.len_min = Some(1);
        constraint.len_max = Some(max);
        constraint
    };
    let (old, new) = constraint_case(length(32), length(64));
    assert_row(&old, &new, Category::ConstraintChanged, Verdict::Compatible);
}

#[test]
fn a_changed_pattern_is_breaking() {
    let pattern = |text: &str| {
        let mut constraint = bounds(None, None);
        constraint.pattern = Some(text.to_string());
        constraint
    };
    let (old, new) = constraint_case(pattern("^[A-Z]+$"), pattern("^[A-Z]{3}$"));
    assert_row(&old, &new, Category::ConstraintChanged, Verdict::Breaking);
}

#[test]
fn a_removed_pattern_is_compatible() {
    let mut patterned = bounds(None, None);
    patterned.pattern = Some("^[A-Z]+$".to_string());
    let (old, new) = constraint_case(patterned, bounds(None, None));
    assert_row(&old, &new, Category::ConstraintChanged, Verdict::Compatible);
}

// ==========================================================================
// Timing.
// ==========================================================================

fn timed(timing: v2::Timing) -> v2::Package {
    pkg(vec![decl(
        "a",
        1,
        v2::decl::Kind::SignalDef(v2::SignalDef {
            payload: "T".to_string(),
            declared_init: None,
            init: None,
            timing: Some(timing),
        }),
    )])
}

/// The same signal with no `Timing` at all — a different state from a `Timing`
/// whose bounds are unset.
fn untimed() -> v2::Package {
    let mut package = timed(range(None, None));
    let Some(v2::decl::Kind::SignalDef(def)) = &mut package.interfaces[0].interactions[0].kind
    else {
        unreachable!("timed builds a SignalDef")
    };
    def.timing = None;
    package
}

/// The event carrier of the same timing, so the rules can be read on both kinds
/// that carry one.
fn timed_event(timing: v2::Timing) -> v2::Package {
    pkg(vec![decl(
        "a",
        1,
        v2::decl::Kind::EventDef(v2::EventDef {
            payload: "T".to_string(),
            timing: Some(timing),
        }),
    )])
}

#[test]
fn a_lowered_timing_minimum_is_breaking() {
    let old = timed(range(Some("10000"), Some("100000")));
    let new = timed(range(Some("5000"), Some("100000")));
    assert_row(&old, &new, Category::TimingChanged, Verdict::Breaking);
}

#[test]
fn a_raised_timing_minimum_is_compatible() {
    let old = timed(range(Some("5000"), Some("100000")));
    let new = timed(range(Some("10000"), Some("100000")));
    assert_row(&old, &new, Category::TimingChanged, Verdict::Compatible);
}

#[test]
fn a_raised_timing_maximum_is_breaking() {
    let old = timed(range(Some("10000"), Some("100000")));
    let new = timed(range(Some("10000"), Some("200000")));
    assert_row(&old, &new, Category::TimingChanged, Verdict::Breaking);
}

#[test]
fn a_lowered_timing_maximum_is_compatible() {
    let old = timed(range(Some("10000"), Some("200000")));
    let new = timed(range(Some("10000"), Some("100000")));
    assert_row(&old, &new, Category::TimingChanged, Verdict::Compatible);
}

#[test]
fn a_timing_bound_added_where_none_was_is_breaking() {
    let old = timed(range(None, Some("100000")));
    let new = timed(range(Some("10000"), Some("100000")));
    assert_row(&old, &new, Category::TimingChanged, Verdict::Breaking);
}

#[test]
fn a_timing_bound_removed_is_breaking() {
    let old = timed(range(Some("10000"), Some("100000")));
    let new = timed(range(Some("10000"), None));
    assert_row(&old, &new, Category::TimingChanged, Verdict::Breaking);
}

/// The floor removed, which is a separate term from the ceiling removed above.
/// A consumer that could assume a publish rate can assume nothing now.
#[test]
fn a_timing_minimum_removed_is_breaking() {
    let old = timed(range(Some("20000"), Some("100000")));
    let new = timed(range(None, Some("100000")));
    assert_row(&old, &new, Category::TimingChanged, Verdict::Breaking);
}

/// The whole `Timing` gone rather than one bound within it. `ridlc` resolves a
/// timing onto every signal and event (ADR-0008 decision 12), so this state
/// arrives the way an unorderable bound does: through
/// [`load_ir_json`](crate::load_ir_json), which deserializes whatever the
/// snapshot on disk holds.
#[test]
fn timing_present_on_one_side_only_is_breaking() {
    let old = timed(range(Some("20000"), Some("100000")));
    let new = untimed();
    assert_row(&old, &new, Category::TimingChanged, Verdict::Breaking);

    assert_row(&new, &old, Category::TimingChanged, Verdict::Breaking);
}

/// Timing rides two interaction kinds and every test above uses a signal. The
/// compatible direction is the one that proves the event arm is read: a kind the
/// classifier cannot recognise falls through to breaking, so only a case that
/// must come out compatible can tell the difference.
#[test]
fn a_lowered_timing_maximum_on_an_event_is_compatible() {
    let old = timed_event(range(Some("10000"), Some("200000")));
    let new = timed_event(range(Some("10000"), Some("100000")));
    assert_row(&old, &new, Category::TimingChanged, Verdict::Compatible);
}

#[test]
fn a_timing_mode_flip_from_strict_to_range_is_breaking() {
    let old = timed(strict("10000"));
    let new = timed(range(Some("10000"), Some("10000")));
    assert_row(&old, &new, Category::TimingChanged, Verdict::Breaking);
}

#[test]
fn a_timing_mode_flip_from_range_to_strict_is_breaking() {
    let old = timed(range(Some("10000"), Some("10000")));
    let new = timed(strict("10000"));
    assert_row(&old, &new, Category::TimingChanged, Verdict::Breaking);
}

/// A default made explicit: the same resolved bounds, now written in source.
#[test]
fn a_default_applied_flip_over_identical_bounds_is_compatible() {
    let mut defaulted = range(Some("100000"), Some("1000000"));
    defaulted.default_applied = true;
    let old = timed(defaulted);
    let new = timed(range(Some("100000"), Some("1000000")));
    assert_row(&old, &new, Category::TimingChanged, Verdict::Compatible);
}

/// The `[defaults].timing` rule at the unit level: a manifest edit reaches the
/// classifier as resolved bounds on a still-defaulted interaction, and the bound
/// rules decide it. The end-to-end proof over two source trees lives in
/// `crates/ridl/tests/diff_gate.rs`.
#[test]
fn a_defaults_timing_edit_classifies_by_its_resolved_bounds() {
    let defaulted = |min: &str, max: &str| {
        let mut timing = range(Some(min), Some(max));
        timing.default_applied = true;
        timing
    };
    // The configured default loosened its staleness bound.
    let old = timed(defaulted("100000", "1000000"));
    let new = timed(defaulted("100000", "2000000"));
    assert_row(&old, &new, Category::TimingChanged, Verdict::Breaking);

    // The configured default tightened it.
    let old = timed(defaulted("100000", "2000000"));
    let new = timed(defaulted("100000", "1000000"));
    assert_row(&old, &new, Category::TimingChanged, Verdict::Compatible);
}

// ==========================================================================
// Fallible returns and the inline `T | E` transport identity.
// ==========================================================================

/// ADR-0008 decision 4: the identity is interface + interaction ordinal +
/// ordered arm types, so swapping the error arm flips it. The rendered
/// before/after must name both identities, because the identity — not the arm
/// spelling — is what a backend keys on.
#[test]
fn an_error_arm_swapped_for_another_error_type_is_breaking_and_names_the_identity() {
    let old = pkg(vec![query(
        "q",
        3,
        vec![],
        fallible_return("Speed", "FaultCode"),
        vec![],
    )]);
    let new = pkg(vec![query(
        "q",
        3,
        vec![],
        fallible_return("Speed", "OtherFault"),
        vec![],
    )]);

    let report = diff_packages(&old, &new);
    let change = row(&report, Category::ReturnChanged);
    assert_eq!(change.verdict, Verdict::Breaking);
    let before = change.before.as_deref().expect("a before value");
    let after = change.after.as_deref().expect("an after value");
    assert!(
        before.contains(&v2::fallible_transport_identity(
            "I",
            3,
            &v2::FallibleType {
                ok: "Speed".to_string(),
                err: "FaultCode".to_string(),
            }
        )),
        "the old transport identity must be named, got {before:?}"
    );
    assert!(
        after.contains(&v2::fallible_transport_identity(
            "I",
            3,
            &v2::FallibleType {
                ok: "Speed".to_string(),
                err: "OtherFault".to_string(),
            }
        )),
        "the new transport identity must be named, got {after:?}"
    );
}

#[test]
fn an_error_arm_added_is_breaking() {
    let old = pkg(vec![query("q", 1, vec![], value_return("Speed"), vec![])]);
    let new = pkg(vec![query(
        "q",
        1,
        vec![],
        fallible_return("Speed", "FaultCode"),
        vec![],
    )]);
    assert_row(&old, &new, Category::ReturnChanged, Verdict::Breaking);
}

#[test]
fn an_error_arm_removed_is_breaking() {
    let old = pkg(vec![query(
        "q",
        1,
        vec![],
        fallible_return("Speed", "FaultCode"),
        vec![],
    )]);
    let new = pkg(vec![query("q", 1, vec![], value_return("Speed"), vec![])]);
    assert_row(&old, &new, Category::ReturnChanged, Verdict::Breaking);
}

#[test]
fn an_ok_arm_change_is_breaking() {
    let old = pkg(vec![query(
        "q",
        1,
        vec![],
        fallible_return("Speed", "FaultCode"),
        vec![],
    )]);
    let new = pkg(vec![query(
        "q",
        1,
        vec![],
        fallible_return("Rpm", "FaultCode"),
        vec![],
    )]);
    assert_row(&old, &new, Category::ReturnChanged, Verdict::Breaking);
}

// ==========================================================================
// Contracts.
// ==========================================================================

fn with_contracts(contracts: Vec<v2::Contract>) -> v2::Package {
    pkg(vec![command("c", 1, vec![], contracts)])
}

#[test]
fn a_require_added_is_breaking() {
    let old = with_contracts(vec![]);
    let new = with_contracts(vec![clause(v2::ContractKind::Require, "speed > 0")]);
    assert_row(&old, &new, Category::ContractChanged, Verdict::Breaking);
}

#[test]
fn a_require_removed_is_compatible() {
    let old = with_contracts(vec![clause(v2::ContractKind::Require, "speed > 0")]);
    let new = with_contracts(vec![]);
    assert_row(&old, &new, Category::ContractChanged, Verdict::Compatible);
}

/// Any `require` text change is breaking: the classifier compares canonical
/// clause text and never proves that the new clause is implied by the old one.
#[test]
fn a_require_text_change_is_breaking() {
    let old = with_contracts(vec![clause(v2::ContractKind::Require, "speed > 0")]);
    let new = with_contracts(vec![clause(v2::ContractKind::Require, "speed >= 0")]);
    assert_row(&old, &new, Category::ContractChanged, Verdict::Breaking);
}

#[test]
fn an_ensure_removed_is_breaking() {
    let old = with_contracts(vec![clause(v2::ContractKind::Ensure, "result > 0")]);
    let new = with_contracts(vec![]);
    assert_row(&old, &new, Category::ContractChanged, Verdict::Breaking);
}

#[test]
fn an_ensure_added_is_compatible() {
    let old = with_contracts(vec![]);
    let new = with_contracts(vec![clause(v2::ContractKind::Ensure, "result > 0")]);
    assert_row(&old, &new, Category::ContractChanged, Verdict::Compatible);
}

/// The mirror of the `require` rule, for the same reason.
#[test]
fn an_ensure_text_change_is_breaking() {
    let old = with_contracts(vec![clause(v2::ContractKind::Ensure, "result > 0")]);
    let new = with_contracts(vec![clause(v2::ContractKind::Ensure, "result >= 0")]);
    assert_row(&old, &new, Category::ContractChanged, Verdict::Breaking);
}

/// Contracts ride two interaction kinds and every case above uses a command.
/// As with the event timing row, the compatible direction is the one that
/// proves the query arm is read: an unrecognised kind falls through to
/// breaking, so a breaking case cannot tell the two apart.
#[test]
fn a_require_removed_from_a_query_is_compatible() {
    let with_query_contracts = |contracts: Vec<v2::Contract>| {
        pkg(vec![query("q", 1, vec![], value_return("T"), contracts)])
    };
    let old = with_query_contracts(vec![clause(v2::ContractKind::Require, "speed > 0")]);
    let new = with_query_contracts(vec![]);
    assert_row(&old, &new, Category::ContractChanged, Verdict::Compatible);
}

/// The category-mismatch fallback in `timing`. The walk cannot reach it — it
/// emits `TimingChanged` only from its signal and event arms — but
/// [`classify`](super::classify) is public and takes any `Change`, so the arm is
/// reachable and is pinned by a test rather than asserted away. A
/// `debug_assert!` here would abort a debug build on a legal call.
///
/// The input change carries `Compatible` so a verdict read back off the input
/// rather than computed would fail this.
#[test]
fn a_timing_change_on_a_kind_that_carries_no_timing_is_breaking() {
    let package = pkg(vec![command("c", 1, vec![], vec![])]);
    let change = Change {
        path: "veh.cluster/I/c".to_string(),
        category: Category::TimingChanged,
        verdict: Verdict::Compatible,
        before: None,
        after: None,
    };
    assert_eq!(
        super::classify(&change, &package, &package),
        Verdict::Breaking
    );
}

// ==========================================================================
// Interfaces and services.
// ==========================================================================

fn service_pkg(services: Vec<v2::Service>) -> v2::Package {
    v2::Package {
        name: "veh.cluster".to_string(),
        decls: Vec::new(),
        interfaces: Vec::new(),
        services,
    }
}

fn service_ref(name: &str, interface_ref: &str) -> v2::Service {
    v2::Service {
        name: name.to_string(),
        visibility: v2::Visibility::Public as i32,
        doc: String::new(),
        labels: Vec::new(),
        deprecated: None,
        shape: Some(v2::service::Shape::InterfaceRef(interface_ref.to_string())),
    }
}

/// A service with an inline shape (ridl §14.5). Its `Interface.name` is `""`
/// and its `Interface.visibility` is `VISIBILITY_UNSPECIFIED`, both by
/// construction — the service carries the identity and the authoritative
/// visibility.
fn service_inline(
    name: &str,
    visibility: v2::Visibility,
    interactions: Vec<v2::Decl>,
) -> v2::Service {
    v2::Service {
        name: name.to_string(),
        visibility: visibility as i32,
        doc: String::new(),
        labels: Vec::new(),
        deprecated: None,
        shape: Some(v2::service::Shape::Inline(v2::Interface {
            name: String::new(),
            visibility: v2::Visibility::Unspecified as i32,
            doc: String::new(),
            labels: Vec::new(),
            deprecated: None,
            interactions,
        })),
    }
}

#[test]
fn a_service_removed_is_breaking() {
    let old = service_pkg(vec![service_ref("Cluster", "I")]);
    let new = service_pkg(vec![]);
    assert_row(&old, &new, Category::DeclRemoved, Verdict::Breaking);
}

/// The classifier resolves a member path under a service's dotted name into
/// the service's inline shape. That shape is not in `Package.interfaces`, so
/// `find_interface` walks `Package::shapes()`; without it the append guard
/// cannot see which slot the new interaction took, and every append inside an
/// inline shape would classify breaking on a missing container.
#[test]
fn an_interaction_appended_inside_an_inline_shape_is_compatible() {
    let old = service_pkg(vec![service_inline(
        "veh.hvac.cabin",
        v2::Visibility::Public,
        vec![signal("a", 1, "T")],
    )]);
    let new = service_pkg(vec![service_inline(
        "veh.hvac.cabin",
        v2::Visibility::Public,
        vec![signal("a", 1, "T"), event("b", 2, "U")],
    )]);
    assert_row(
        &old,
        &new,
        Category::InteractionAppended,
        Verdict::Compatible,
    );
}

/// The same path with the ordinal reused rather than appended: the guard has
/// to read the OLD inline shape's slots to see that ordinal 2 was occupied.
#[test]
fn an_ordinal_reused_inside_an_inline_shape_is_breaking() {
    let old = service_pkg(vec![service_inline(
        "veh.hvac.cabin",
        v2::Visibility::Public,
        vec![signal("a", 1, "T"), signal("b", 2, "T")],
    )]);
    let new = service_pkg(vec![service_inline(
        "veh.hvac.cabin",
        v2::Visibility::Public,
        vec![signal("a", 1, "T"), event("c", 2, "U")],
    )]);
    assert_row(&old, &new, Category::InteractionAppended, Verdict::Breaking);
}

/// A service's visibility is read from the `Service`, never from its inline
/// shape's own field, which is `VISIBILITY_UNSPECIFIED` by construction. If it
/// were read from the shape, both sides would be unspecified and the widening
/// below would classify breaking under the unlisted-is-breaking rule.
#[test]
fn widening_an_inline_shape_service_reads_the_services_own_visibility() {
    let interactions = vec![signal("a", 1, "T")];
    let old = service_pkg(vec![service_inline(
        "veh.hvac.cabin",
        v2::Visibility::Internal,
        interactions.clone(),
    )]);
    let new = service_pkg(vec![service_inline(
        "veh.hvac.cabin",
        v2::Visibility::Public,
        interactions,
    )]);
    assert_row(&old, &new, Category::VisibilityChanged, Verdict::Compatible);
}

#[test]
fn a_service_appended_is_compatible() {
    let old = service_pkg(vec![]);
    let new = service_pkg(vec![service_ref("Cluster", "I")]);
    assert_row(&old, &new, Category::DeclAdded, Verdict::Compatible);
}

#[test]
fn a_service_interface_ref_change_is_breaking() {
    let old = service_pkg(vec![service_ref("Cluster", "I")]);
    let new = service_pkg(vec![service_ref("Cluster", "J")]);
    assert_row(&old, &new, Category::ServiceChanged, Verdict::Breaking);
}

#[test]
fn an_interface_removed_is_breaking() {
    let old = pkg(vec![signal("a", 1, "T")]);
    let new = v2::Package {
        name: "veh.cluster".to_string(),
        decls: Vec::new(),
        interfaces: Vec::new(),
        services: Vec::new(),
    };
    assert_row(&old, &new, Category::DeclRemoved, Verdict::Breaking);
}

#[test]
fn an_interface_appended_is_compatible() {
    let old = v2::Package {
        name: "veh.cluster".to_string(),
        decls: Vec::new(),
        interfaces: Vec::new(),
        services: Vec::new(),
    };
    let new = pkg(vec![signal("a", 1, "T")]);
    assert_row(&old, &new, Category::DeclAdded, Verdict::Compatible);
}

#[test]
fn a_new_package_level_declaration_is_compatible() {
    let old = decl_pkg(vec![]);
    let new = decl_pkg(vec![scalar(
        "S",
        bounds(Some("0"), Some("250")),
        v2::IntWidth::U8,
    )]);
    assert_row(&old, &new, Category::DeclAdded, Verdict::Compatible);
}

// ==========================================================================
// Composite bodies.
// ==========================================================================

#[test]
fn an_enum_value_appended_is_compatible() {
    let old = decl_pkg(vec![enum_decl(
        "E",
        vec![enum_value("A", 0), enum_value("B", 1)],
        vec![],
    )]);
    let new = decl_pkg(vec![enum_decl(
        "E",
        vec![enum_value("A", 0), enum_value("B", 1), enum_value("C", 2)],
        vec![],
    )]);
    assert_row(&old, &new, Category::DeclAdded, Verdict::Compatible);
}

/// A value squeezed in below the top renumbers the values above it, so the same
/// wire number now means something else.
#[test]
fn an_enum_value_inserted_below_the_top_is_breaking() {
    let old = decl_pkg(vec![enum_decl(
        "E",
        vec![enum_value("A", 0), enum_value("B", 1)],
        vec![],
    )]);
    let new = decl_pkg(vec![enum_decl(
        "E",
        vec![enum_value("A", 0), enum_value("X", 1), enum_value("B", 2)],
        vec![],
    )]);
    assert_row(&old, &new, Category::DeclAdded, Verdict::Breaking);
}

/// A retired number is never reused with a new meaning (typl §7.4).
#[test]
fn an_enum_value_taking_a_retired_number_is_breaking() {
    let old = decl_pkg(vec![enum_decl("E", vec![enum_value("A", 0)], vec![3])]);
    let new = decl_pkg(vec![enum_decl(
        "E",
        vec![enum_value("A", 0), enum_value("C", 2)],
        vec![3],
    )]);
    assert_row(&old, &new, Category::DeclAdded, Verdict::Breaking);
}

#[test]
fn an_enum_value_removed_is_breaking() {
    let old = decl_pkg(vec![enum_decl(
        "E",
        vec![enum_value("A", 0), enum_value("B", 1)],
        vec![],
    )]);
    let new = decl_pkg(vec![enum_decl("E", vec![enum_value("A", 0)], vec![])]);
    assert_row(&old, &new, Category::DeclRemoved, Verdict::Breaking);
}

#[test]
fn a_union_arm_appended_at_the_end_is_compatible() {
    let old = decl_pkg(vec![union_decl(
        "U",
        vec![union_arm("a", 1, "A"), union_arm("b", 2, "B")],
        false,
    )]);
    let new = decl_pkg(vec![union_decl(
        "U",
        vec![
            union_arm("a", 1, "A"),
            union_arm("b", 2, "B"),
            union_arm("c", 3, "C"),
        ],
        false,
    )]);
    assert_row(&old, &new, Category::DeclAdded, Verdict::Compatible);
}

#[test]
fn a_union_arm_inserted_before_the_end_is_breaking() {
    let old = decl_pkg(vec![union_decl(
        "U",
        vec![union_arm("a", 1, "A"), union_arm("b", 2, "B")],
        false,
    )]);
    let new = decl_pkg(vec![union_decl(
        "U",
        vec![
            union_arm("a", 1, "A"),
            union_arm("x", 2, "X"),
            union_arm("b", 3, "B"),
        ],
        false,
    )]);
    assert_row(&old, &new, Category::DeclAdded, Verdict::Breaking);
}

/// A result union's arms are its transport identity (ADR-0008 decision 4), so
/// even an appended arm flips it.
#[test]
fn a_union_arm_appended_to_a_result_union_is_breaking() {
    let old = decl_pkg(vec![union_decl(
        "R",
        vec![union_arm("ok", 1, "A"), union_arm("err", 2, "E")],
        true,
    )]);
    let new = decl_pkg(vec![union_decl(
        "R",
        vec![
            union_arm("ok", 1, "A"),
            union_arm("err", 2, "E"),
            union_arm("other", 3, "O"),
        ],
        true,
    )]);
    assert_row(&old, &new, Category::DeclAdded, Verdict::Breaking);
}

#[test]
fn a_struct_field_appended_at_the_end_is_compatible() {
    let old = decl_pkg(vec![struct_decl("S", vec![field("a", 1, "A")])]);
    let new = decl_pkg(vec![struct_decl(
        "S",
        vec![field("a", 1, "A"), field("b", 2, "B")],
    )]);
    assert_row(&old, &new, Category::DeclAdded, Verdict::Compatible);
}

/// The classifier is the **only** line of defence here, so this is the module's
/// own documented worst case: "a false compatible ships a wire break".
///
/// `reserved 2` is written into the middle of the body, which pushes the
/// surviving `b` from ordinal 2 to 3 while `c` is appended at 4. The walk's
/// composite comparison is keyed on member names, so it reports one thing only —
/// that `c` was added — and says nothing about `b` moving. Without
/// `appended_slot`'s surviving-member check the addition reads as a clean
/// append, the report comes out compatible, and `ridl diff` exits 0 over a
/// struct whose field `b` now carries a different wire ordinal.
///
/// The verdict is asserted on the report as well as on the change: the report
/// verdict is what the `ridl diff` facade turns into its exit code — breaking is
/// exit 1, compatible or identical is exit 0 (`run_diff` in
/// `crates/ridl/src/main.rs`, the contract of ADR-0008 decision 9).
#[test]
fn an_addition_that_moves_a_surviving_struct_field_is_breaking() {
    let old = decl_pkg(vec![struct_decl(
        "Frame",
        vec![field("a", 1, "Speed"), field("b", 2, "Speed")],
    )]);
    let new = decl_pkg(vec![struct_decl(
        "Frame",
        vec![
            field("a", 1, "Speed"),
            reserved_field(2),
            field("b", 3, "Speed"),
            field("c", 4, "Speed"),
        ],
    )]);

    let report = diff_packages(&old, &new);
    assert_eq!(
        report.changes.len(),
        1,
        "the name-keyed walk reports only the added field — nothing about b \
         moving — so the classifier is the sole line of defence, got {:?}",
        report.changes
    );
    let added = row(&report, Category::DeclAdded);
    assert_eq!(added.path, "veh.cluster/Frame/c");
    assert_eq!(
        added.verdict,
        Verdict::Breaking,
        "b moved from ordinal 2 to 3, so the addition is not an append, got {:?}",
        report.changes
    );
    assert_eq!(
        report.verdict,
        Verdict::Breaking,
        "the report verdict is the exit code: this must be exit 1, got {:?}",
        report.changes
    );
}

/// The same guard over the other half of `appended_slot`: a retired ordinal is
/// never handed to a new member (typl §7.4). The tombstone lives in the *old*
/// body — deleting it and reusing ordinal 3 is what an author does by hand, and
/// the new body on its own looks perfectly legal.
#[test]
fn a_struct_field_taking_a_retired_ordinal_is_breaking() {
    let old = decl_pkg(vec![struct_decl(
        "Frame",
        vec![field("a", 1, "A"), field("b", 2, "B"), reserved_field(3)],
    )]);
    let new = decl_pkg(vec![struct_decl(
        "Frame",
        vec![field("a", 1, "A"), field("b", 2, "B"), field("c", 3, "C")],
    )]);
    assert_row(&old, &new, Category::DeclAdded, Verdict::Breaking);
}

/// The union body carries the same rule over its own `reserved` list.
#[test]
fn a_union_arm_taking_a_retired_ordinal_is_breaking() {
    let old = decl_pkg(vec![union_with_reserved(
        "U",
        vec![union_arm("a", 1, "A"), union_arm("b", 2, "B")],
        vec![3],
    )]);
    let new = decl_pkg(vec![union_with_reserved(
        "U",
        vec![
            union_arm("a", 1, "A"),
            union_arm("b", 2, "B"),
            union_arm("c", 3, "C"),
        ],
        vec![],
    )]);
    assert_row(&old, &new, Category::DeclAdded, Verdict::Breaking);
}

#[test]
fn a_struct_field_inserted_before_the_end_is_breaking() {
    let old = decl_pkg(vec![struct_decl(
        "S",
        vec![field("a", 1, "A"), field("b", 2, "B")],
    )]);
    let new = decl_pkg(vec![struct_decl(
        "S",
        vec![field("a", 1, "A"), field("x", 2, "X"), field("b", 3, "B")],
    )]);
    assert_row(&old, &new, Category::DeclAdded, Verdict::Breaking);
}

// ==========================================================================
// Metadata and inits.
// ==========================================================================

#[test]
fn a_doc_only_change_is_compatible() {
    let old = pkg(vec![signal("a", 1, "T")]);
    let mut new = old.clone();
    new.interfaces[0].interactions[0].doc = "the current road speed".to_string();
    assert_row(&old, &new, Category::DocOnly, Verdict::Compatible);
}

/// The interface-level twin of the case above. Each envelope field is exercised
/// on its own, because dropping a single term from the comparison drops that one
/// edit out of the report: an interface's `@deprecated` notice appearing or
/// changing is a migration-planning signal, and a diff that never mentions it
/// tells a reader the interface is unchanged.
#[test]
fn an_interface_envelope_change_is_doc_only_and_compatible() {
    let base = pkg(vec![signal("a", 1, "T")]);

    let mut documented = base.clone();
    documented.interfaces[0].doc = "the vehicle status interface".to_string();
    let report = diff_packages(&base, &documented);
    let change = row(&report, Category::DocOnly);
    assert_eq!(
        change.path, "veh.cluster/I",
        "the change is the interface's own, not an interaction's, got {:?}",
        report.changes
    );
    assert_eq!(change.verdict, Verdict::Compatible);

    let mut labelled = base.clone();
    labelled.interfaces[0].labels = vec!["cluster".to_string()];
    assert_row(&base, &labelled, Category::DocOnly, Verdict::Compatible);

    let mut deprecated = base.clone();
    deprecated.interfaces[0].deprecated = Some("use VehicleStatus2".to_string());
    assert_row(&base, &deprecated, Category::DocOnly, Verdict::Compatible);
}

/// And the service-level twin, which carries a third copy of the same
/// comparison.
#[test]
fn a_service_envelope_change_is_doc_only_and_compatible() {
    let base = service_pkg(vec![service_ref("Cluster", "I")]);

    let mut documented = base.clone();
    documented.services[0].doc = "the cluster service".to_string();
    assert_row(&base, &documented, Category::DocOnly, Verdict::Compatible);

    let mut labelled = base.clone();
    labelled.services[0].labels = vec!["cluster".to_string()];
    assert_row(&base, &labelled, Category::DocOnly, Verdict::Compatible);

    let mut deprecated = base.clone();
    deprecated.services[0].deprecated = Some("use Cluster2".to_string());
    assert_row(&base, &deprecated, Category::DocOnly, Verdict::Compatible);
}

// ==========================================================================
// Visibility.
//
// `internal` maps to the target's package-private mechanism (ADR-0002 §8), so
// narrowing deletes the declaration from every out-of-package consumer. The
// wire layout never moves, which is exactly why this was folded into the doc
// envelope and slipped through as compatible.
// ==========================================================================

/// An interaction published at `visibility`.
fn visible_signal(visibility: v2::Visibility) -> v2::Package {
    let mut package = pkg(vec![signal("a", 1, "T")]);
    package.interfaces[0].interactions[0].visibility = visibility as i32;
    package
}

/// An interface published at `visibility`.
fn visible_interface(visibility: v2::Visibility) -> v2::Package {
    let mut package = pkg(vec![signal("a", 1, "T")]);
    package.interfaces[0].visibility = visibility as i32;
    package
}

#[test]
fn narrowing_a_declaration_from_public_to_internal_is_breaking() {
    let old = visible_signal(v2::Visibility::Public);
    let new = visible_signal(v2::Visibility::Internal);
    assert_row(&old, &new, Category::VisibilityChanged, Verdict::Breaking);
    assert_eq!(
        diff_packages(&old, &new).verdict,
        Verdict::Breaking,
        "the report must gate, not just the change"
    );
}

#[test]
fn widening_a_declaration_from_internal_to_public_is_compatible() {
    let old = visible_signal(v2::Visibility::Internal);
    let new = visible_signal(v2::Visibility::Public);
    assert_row(&old, &new, Category::VisibilityChanged, Verdict::Compatible);
}

#[test]
fn narrowing_an_interface_from_public_to_internal_is_breaking() {
    let old = visible_interface(v2::Visibility::Public);
    let new = visible_interface(v2::Visibility::Internal);
    assert_row(&old, &new, Category::VisibilityChanged, Verdict::Breaking);
    assert_eq!(diff_packages(&old, &new).verdict, Verdict::Breaking);
}

#[test]
fn widening_an_interface_from_internal_to_public_is_compatible() {
    let old = visible_interface(v2::Visibility::Internal);
    let new = visible_interface(v2::Visibility::Public);
    assert_row(&old, &new, Category::VisibilityChanged, Verdict::Compatible);
}

/// Visibility no longer rides the doc envelope, so a visibility edit and a doc
/// edit are two separately classified changes rather than one `DocOnly`.
#[test]
fn a_visibility_change_is_not_reported_as_doc_only() {
    let old = visible_signal(v2::Visibility::Public);
    let new = visible_signal(v2::Visibility::Internal);
    let report = diff_packages(&old, &new);
    assert!(
        report
            .changes
            .iter()
            .all(|change| change.category != Category::DocOnly),
        "visibility must not hide inside the doc envelope, got {:?}",
        report.changes
    );
}

// ==========================================================================
// Tombstones without a name.
// ==========================================================================

/// A tombstone with no name — the `reserved 5` and `reserved "x"` forms both
/// lower to `Reserved { name: None }` — still holds its ordinal. Dropping it
/// from the slot list made the freed ordinal look unused, so a new interaction
/// taking it classified as a clean append.
fn nameless_reserved(ordinal: u32) -> v2::Decl {
    v2::Decl {
        name: String::new(),
        visibility: v2::Visibility::Unspecified as i32,
        is_error: false,
        doc: String::new(),
        labels: Vec::new(),
        deprecated: None,
        ordinal,
        kind: Some(v2::decl::Kind::ReservedSlot(v2::Reserved {
            ordinal,
            name: None,
            value: None,
        })),
    }
}

#[test]
fn reusing_the_ordinal_of_a_nameless_tombstone_is_breaking() {
    let old = pkg(vec![signal("a", 1, "T"), nameless_reserved(2)]);
    let new = pkg(vec![signal("a", 1, "T"), signal("c", 2, "T")]);

    let report = diff_packages(&old, &new);
    let appended = row(&report, Category::InteractionAppended);
    assert_eq!(appended.path, "veh.cluster/I/c");
    assert_eq!(
        appended.verdict,
        Verdict::Breaking,
        "a name-less tombstone still reserves ordinal 2, got {:?}",
        report.changes
    );
    assert_eq!(report.verdict, Verdict::Breaking);
}

/// The control for the case above: the same reuse against a *named* tombstone
/// was already breaking and must stay so.
#[test]
fn reusing_the_ordinal_of_a_named_tombstone_is_breaking() {
    let old = pkg(vec![signal("a", 1, "T"), reserved("b", 2)]);
    let new = pkg(vec![signal("a", 1, "T"), signal("c", 2, "T")]);

    let report = diff_packages(&old, &new);
    assert_eq!(
        row(&report, Category::InteractionAppended).verdict,
        Verdict::Breaking
    );
    assert_eq!(report.verdict, Verdict::Breaking);
}

/// And the control in the other direction: a name-less tombstone must not turn
/// an honest append into a false breaking. `c` takes a fresh ordinal above it.
fn nameless_tombstone_case() -> (v2::Package, v2::Package) {
    (
        pkg(vec![signal("a", 1, "T"), nameless_reserved(2)]),
        pkg(vec![
            signal("a", 1, "T"),
            nameless_reserved(2),
            signal("c", 3, "T"),
        ]),
    )
}

#[test]
fn appending_past_a_nameless_tombstone_is_still_compatible() {
    let (old, new) = nameless_tombstone_case();
    assert_row(
        &old,
        &new,
        Category::InteractionAppended,
        Verdict::Compatible,
    );
}

#[test]
fn an_init_change_is_breaking() {
    let with_init = |init: &str| {
        pkg(vec![decl(
            "a",
            1,
            v2::decl::Kind::SignalDef(v2::SignalDef {
                payload: "T".to_string(),
                declared_init: Some(init.to_string()),
                init: None,
                timing: Some(range(Some("10000"), Some("100000"))),
            }),
        )])
    };
    let old = with_init("0");
    let new = with_init("1");
    assert_row(&old, &new, Category::InitChanged, Verdict::Breaking);
}

/// The init of a *named type*, which the walk emits from `diff_type_def` — a
/// different site from the signal init above, and the one a consumer reads as
/// the pre-publish value of every field of that type.
#[test]
fn a_type_init_change_is_breaking() {
    let old = decl_pkg(vec![scalar_with_init("Speed", v2::IntWidth::U16, "10.0")]);
    let new = decl_pkg(vec![scalar_with_init("Speed", v2::IntWidth::U16, "20.0")]);
    assert_row(&old, &new, Category::InitChanged, Verdict::Breaking);
}

// ==========================================================================
// `--explain` coverage.
// ==========================================================================

/// Every category the report can print has a rule row, and every row names its
/// verdicts — `--explain` is the CI-facing documentation of record, so a
/// category added without a row would leave a reader with nothing.
///
/// This iterates `CATEGORIES`, and since ADR-0008 decision 21 `CATEGORIES` is
/// generated from the `Category` declaration by `declare_categories!` rather
/// than maintained beside it, so iterating it is iterating every variant. The
/// companion test that used to check that claim — one list of names expanded
/// into an exhaustive `match` and an array, asserted to agree with `CATEGORIES`
/// — is gone: it compared `CATEGORIES` against a list inside the test, which is
/// exactly the shadow the generated array removes.
///
/// What is still not proved: rustc forces *an* arm in `classify`, `explain`, and
/// `category_word` for a new variant, not the right one. The assertions below
/// catch a rule row with no verdict in it and a word that does not round-trip;
/// they do not catch a row that describes the wrong rule, or a `classify` arm
/// that silently answers compatible.
#[test]
fn every_category_has_a_rule_row_naming_its_verdicts() {
    for category in crate::CATEGORIES {
        let word = crate::category_word(category);
        let text = super::explain(category);
        assert!(
            text.contains("compatible") || text.contains("breaking"),
            "{word} has no verdict in its rule row: {text}"
        );
        assert_eq!(
            super::category_from_word(word),
            Some(category),
            "{word} must round-trip through category_from_word"
        );
    }
}

#[test]
fn an_unknown_category_word_has_no_rule_row() {
    assert_eq!(super::category_from_word("no_such_category"), None);
}
