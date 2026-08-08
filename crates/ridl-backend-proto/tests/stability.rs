//! ADR-0016 decision 6, property 3: a compatible change never moves a number
//! already assigned. Driven from `ridl-diff`'s own classifier, so the property
//! is tested rather than a list of examples.
//!
//! The mutation set is taken from `crates/ridl-diff/src/classify.rs` — one
//! mutation per compatible arm of `classify` that a single self-contained
//! package can reach:
//!
//! - `DocOnly` — a doc comment edit.
//! - `InteractionRetired` — an interaction replaced by a tombstone in its own
//!   slot.
//! - `VisibilityChanged` — `internal` widened to `public`.
//! - `InteractionAppended` — a new interaction, and separately a freshly
//!   minted tombstone, at the next ordinal.
//! - `DeclAdded` — a new package-level struct; a struct field, an enum value,
//!   an enum-set bit, and a union arm each appended above the high-water
//!   ordinal or value.
//! - `ConstraintChanged` — a declared range widened.
//! - `TimingChanged` — a signal's rate floor raised.
//! - `RpcBoundChanged` — a command's call throttle lowered.
//! - `ContractChanged` — a `require` clause removed.
//!
//! The two remaining compatible arms, `ServiceShapeRetired` and
//! `ServiceShapeAppended`, mutate a service's named shape list, which this
//! backend does not project at all (ADR-0013 decision 2 stops at the
//! interaction identity table), so no number could move under them.

use proptest::prelude::*;
use ridl_ir::v2;

/// Every `name = number` pair the schema assigns, keyed by the enclosing
/// message or enum so two declarations cannot share a key. A number that moves
/// between two schemas is exactly a change to this map's values.
///
/// This reads the emitted *text* rather than the emitter's own data, so a bug
/// in the emitter cannot hide itself in the assertion.
fn assigned_numbers(schema: &str) -> std::collections::BTreeMap<String, u32> {
    let mut out = std::collections::BTreeMap::new();
    let mut scope = String::new();
    for line in schema.lines() {
        let line = line.trim();
        if let Some(rest) = line
            .strip_prefix("message ")
            .or_else(|| line.strip_prefix("enum "))
        {
            scope = rest.trim_end_matches(" {").trim().to_string();
            continue;
        }
        if line == "}" {
            scope.clear();
            continue;
        }
        // Both `double current_speed = 1;` and `GEAR_POSITION_PARK = 1;` end
        // in `= <digits>;`. `reserved 3;` deliberately does not match: a
        // tombstone assigns no name.
        let Some((left, right)) = line.split_once('=') else {
            continue;
        };
        let Some(digits) = right.trim().strip_suffix(';') else {
            continue;
        };
        let Ok(number) = digits.trim().parse::<u32>() else {
            continue;
        };
        let Some(name) = left.split_whitespace().last() else {
            continue;
        };
        out.insert(format!("{scope}.{name}"), number);
    }
    out
}

proptest! {
    #[test]
    fn a_compatible_change_never_moves_an_assigned_number(
        delta in compatible_delta_strategy()
    ) {
        let (old, new) = delta;
        let report = ridl_diff::diff_packages(&old, &new);
        prop_assume!(report.verdict == ridl_diff::Verdict::Compatible);

        let old_numbers = assigned_numbers(
            &ridl_backend_proto::generate(&old).expect("generate old").proto_source
        );
        let new_numbers = assigned_numbers(
            &ridl_backend_proto::generate(&new).expect("generate new").proto_source
        );

        // The base package always assigns numbers (the struct's fields at
        // minimum), so an empty map means the parser stopped reading the
        // emitted text — a vacuous pass, not a clean one.
        prop_assert!(
            !old_numbers.is_empty(),
            "assigned_numbers read no assignment out of the old schema"
        );

        for (name, number) in &old_numbers {
            if let Some(moved) = new_numbers.get(name) {
                prop_assert!(
                    number == moved,
                    "{name} moved from {number} to {moved} under a change \
                     ridl-diff calls compatible"
                );
            }
        }
    }
}

// ==========================================================================
// The base package.
// ==========================================================================

/// The generated dimensions of the base package. Every declaration a mutation
/// targets is always present, so every mutation always applies — that is what
/// keeps the `prop_assume!` discard rate at zero (see
/// [`every_generated_delta_classifies_compatible`]).
#[derive(Debug, Clone)]
struct BaseShape {
    /// One primitive per struct field `f1..fN`.
    field_primitives: Vec<v2::PrimitiveType>,
    /// Whether the struct body ends in a retired ordinal.
    struct_tombstone: bool,
    /// Live enum values `V0..V{n-1}`, numbered from zero.
    enum_value_count: i64,
    /// Whether the enum retires the value after its last live one.
    enum_retired: bool,
    /// Enum-set bits `B0..B{n-1}`.
    bit_count: i64,
    /// Whether the union retires the ordinal after its last arm.
    union_tombstone: bool,
    /// Signals `s1..sK` at ordinals 1..K.
    signal_count: u32,
    /// Whether a tombstone sits between the signals and the command.
    interface_tombstone: bool,
    /// The upper bound of the named scalar's declared range.
    bound_max: u64,
}

fn decl(name: &str, visibility: v2::Visibility, kind: v2::decl::Kind) -> v2::Decl {
    v2::Decl {
        name: name.to_string(),
        visibility: visibility as i32,
        is_error: false,
        doc: String::new(),
        labels: Vec::new(),
        deprecated: None,
        ordinal: 0,
        kind: Some(kind),
    }
}

fn field(name: &str, ordinal: u32, field_type: v2::FieldType) -> v2::Field {
    v2::Field {
        name: name.to_string(),
        ordinal,
        r#type: Some(field_type),
        declared_init: None,
        init: None,
        doc: String::new(),
        labels: Vec::new(),
        deprecated: None,
    }
}

fn primitive_type(primitive: v2::PrimitiveType) -> v2::FieldType {
    v2::FieldType {
        optional: false,
        kind: Some(v2::field_type::Kind::Primitive(primitive as i32)),
    }
}

fn named_type(reference: &str) -> v2::FieldType {
    v2::FieldType {
        optional: false,
        kind: Some(v2::field_type::Kind::Named(reference.to_string())),
    }
}

fn live_member(field: v2::Field) -> v2::StructMember {
    v2::StructMember {
        member: Some(v2::struct_member::Member::Field(field)),
    }
}

fn interaction(name: &str, ordinal: u32, kind: v2::decl::Kind) -> v2::Decl {
    v2::Decl {
        name: name.to_string(),
        visibility: v2::Visibility::Unspecified as i32,
        is_error: false,
        doc: String::new(),
        labels: Vec::new(),
        deprecated: None,
        ordinal,
        kind: Some(kind),
    }
}

/// A `reserved` tombstone in an interface body: the retired name lives in
/// `Reserved.name` and the `Decl` envelope's own name is empty, matching what
/// the checker writes (ridl §11).
fn interaction_tombstone(name: &str, ordinal: u32) -> v2::Decl {
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

fn range_timing(min_us: &str, max_us: &str) -> v2::Timing {
    v2::Timing {
        mode: v2::TimingMode::Range as i32,
        min_us: Some(min_us.to_string()),
        max_us: Some(max_us.to_string()),
        default_applied: false,
    }
}

fn signal(payload: &str) -> v2::decl::Kind {
    v2::decl::Kind::SignalDef(v2::SignalDef {
        payload: payload.to_string(),
        declared_init: None,
        init: None,
        timing: Some(range_timing("10000", "500000")),
    })
}

/// A self-contained package holding one target for every mutation: a named
/// scalar with a declared range (`Bound`), an enum (`Mode`), an enum set
/// (`Flags`), an internal union (`Choice`), a struct referencing all of them
/// (`Data`), and an interface (`Ctrl`) with signals and a command carrying a
/// contract and a declared RPC bound.
fn base_package(shape: &BaseShape) -> v2::Package {
    let bound = v2::TypeDef {
        backing: Some(v2::Backing {
            kind: Some(v2::backing::Kind::Primitive(
                v2::PrimitiveType::Integer as i32,
            )),
        }),
        constraint: Some(v2::Constraint {
            min: Some("10".to_string()),
            max: Some(shape.bound_max.to_string()),
            step: None,
            len_min: None,
            len_max: None,
            pattern: None,
            pattern_const: None,
        }),
        declared_init: None,
        init: None,
        width: Some(v2::type_def::Width::IntWidth(v2::IntWidth::U32 as i32)),
    };

    let mode = v2::EnumDef {
        values: (0..shape.enum_value_count)
            .map(|value| v2::EnumValue {
                name: format!("V{value}"),
                value,
                doc: String::new(),
            })
            .collect(),
        reserved: if shape.enum_retired {
            vec![v2::Reserved {
                ordinal: 0,
                name: None,
                value: Some(shape.enum_value_count),
            }]
        } else {
            Vec::new()
        },
    };

    let flags = v2::EnumSetDef {
        backing_enum: None,
        bits: (0..shape.bit_count)
            .map(|value| v2::EnumValue {
                name: format!("B{value}"),
                value,
                doc: String::new(),
            })
            .collect(),
        width: v2::IntWidth::U32 as i32,
    };

    let choice = v2::UnionDef {
        arms: vec![
            v2::UnionArm {
                name: "first".to_string(),
                ordinal: 1,
                type_ref: "Data".to_string(),
                doc: String::new(),
            },
            v2::UnionArm {
                name: "second".to_string(),
                ordinal: 2,
                type_ref: "Mode".to_string(),
                doc: String::new(),
            },
        ],
        is_result: false,
        reserved: if shape.union_tombstone {
            vec![v2::Reserved {
                ordinal: 3,
                name: Some("old_arm".to_string()),
                value: None,
            }]
        } else {
            Vec::new()
        },
    };

    let mut members: Vec<v2::StructMember> = shape
        .field_primitives
        .iter()
        .enumerate()
        .map(|(index, primitive)| {
            let ordinal = u32::try_from(index).expect("few fields") + 1;
            live_member(field(
                &format!("f{ordinal}"),
                ordinal,
                primitive_type(*primitive),
            ))
        })
        .collect();
    let field_count = u32::try_from(shape.field_primitives.len()).expect("few fields");
    members.push(live_member(field(
        "limit",
        field_count + 1,
        named_type("Bound"),
    )));
    members.push(live_member(field(
        "flags",
        field_count + 2,
        named_type("Flags"),
    )));
    if shape.struct_tombstone {
        members.push(v2::StructMember {
            member: Some(v2::struct_member::Member::Reserved(v2::Reserved {
                ordinal: field_count + 3,
                name: Some("legacy_field".to_string()),
                value: None,
            })),
        });
    }
    let data = v2::StructDef {
        members,
        fixed_layout: false,
    };

    let mut interactions = Vec::new();
    let mut ordinal = 0u32;
    for index in 1..=shape.signal_count {
        ordinal += 1;
        interactions.push(interaction(&format!("s{index}"), ordinal, signal("Data")));
    }
    if shape.interface_tombstone {
        ordinal += 1;
        interactions.push(interaction_tombstone("legacy", ordinal));
    }
    ordinal += 1;
    interactions.push(interaction(
        "apply",
        ordinal,
        v2::decl::Kind::CommandDef(v2::CommandDef {
            params: vec![v2::Param {
                name: "amount".to_string(),
                r#type: Some(named_type("Bound")),
            }],
            contracts: vec![v2::Contract {
                kind: v2::ContractKind::Require as i32,
                source: "amount > 0".to_string(),
                signal_refs: Vec::new(),
                param_refs: vec!["amount".to_string()],
                uses_result: false,
                observer_id: "Ctrl.apply.require[0]".to_string(),
            }],
            timing: Some(range_timing("1000", "2000")),
        }),
    ));
    let ctrl = v2::Interface {
        name: "Ctrl".to_string(),
        visibility: v2::Visibility::Public as i32,
        doc: String::new(),
        labels: Vec::new(),
        deprecated: None,
        interactions,
    };

    v2::Package {
        name: "stability.base".to_string(),
        decls: vec![
            decl(
                "Bound",
                v2::Visibility::Public,
                v2::decl::Kind::TypeDef(bound),
            ),
            decl(
                "Mode",
                v2::Visibility::Public,
                v2::decl::Kind::EnumDef(mode),
            ),
            decl(
                "Flags",
                v2::Visibility::Public,
                v2::decl::Kind::EnumSetDef(flags),
            ),
            decl(
                "Choice",
                v2::Visibility::Internal,
                v2::decl::Kind::UnionDef(choice),
            ),
            decl(
                "Data",
                v2::Visibility::Public,
                v2::decl::Kind::StructDef(data),
            ),
        ],
        interfaces: vec![ctrl],
        services: Vec::new(),
    }
}

// ==========================================================================
// The mutations — one per compatible arm of the classifier.
// ==========================================================================

const MUTATION_COUNT: usize = 14;

fn decl_mut<'a>(package: &'a mut v2::Package, name: &str) -> &'a mut v2::Decl {
    package
        .decls
        .iter_mut()
        .find(|decl| decl.name == name)
        .expect("the base package declares every mutation target")
}

fn first_signal_mut(package: &mut v2::Package) -> &mut v2::Decl {
    package.interfaces[0]
        .interactions
        .iter_mut()
        .find(|decl| matches!(decl.kind, Some(v2::decl::Kind::SignalDef(_))))
        .expect("the base package's interface holds at least one signal")
}

fn command_mut(package: &mut v2::Package) -> &mut v2::CommandDef {
    let decl = package.interfaces[0]
        .interactions
        .iter_mut()
        .find(|decl| matches!(decl.kind, Some(v2::decl::Kind::CommandDef(_))))
        .expect("the base package's interface holds a command");
    let Some(v2::decl::Kind::CommandDef(def)) = &mut decl.kind else {
        unreachable!("found above");
    };
    def
}

/// One ordinal above every slot the struct body ever used, live or retired.
fn struct_next_ordinal(def: &v2::StructDef) -> u32 {
    def.members
        .iter()
        .filter_map(|member| match &member.member {
            Some(v2::struct_member::Member::Field(field)) => Some(field.ordinal),
            Some(v2::struct_member::Member::Reserved(reserved)) => Some(reserved.ordinal),
            None => None,
        })
        .max()
        .unwrap_or(0)
        + 1
}

/// One above every enum value ever used, live or retired.
fn enum_next_value(def: &v2::EnumDef) -> i64 {
    def.values
        .iter()
        .map(|value| value.value)
        .chain(def.reserved.iter().filter_map(|reserved| reserved.value))
        .max()
        .unwrap_or(-1)
        + 1
}

/// One above every union arm ordinal ever used, live or retired.
fn union_next_ordinal(def: &v2::UnionDef) -> u32 {
    def.arms
        .iter()
        .map(|arm| arm.ordinal)
        .chain(def.reserved.iter().map(|reserved| reserved.ordinal))
        .max()
        .unwrap_or(0)
        + 1
}

fn interface_next_ordinal(interface: &v2::Interface) -> u32 {
    interface
        .interactions
        .iter()
        .map(|decl| decl.ordinal)
        .max()
        .unwrap_or(0)
        + 1
}

/// Applies one compatible edit to a copy of `old`. Each arm names the
/// `classify` arm in `crates/ridl-diff/src/classify.rs` it exercises.
fn apply_mutation(old: &v2::Package, index: usize) -> v2::Package {
    let mut new = old.clone();
    match index {
        // DeclAdded, composite append: a struct field at the next ordinal
        // (typl §7.4, `added` → `appended_slot`).
        0 => {
            let Some(v2::decl::Kind::StructDef(def)) = &mut decl_mut(&mut new, "Data").kind else {
                unreachable!("Data is a struct");
            };
            let next = struct_next_ordinal(def);
            def.members.push(live_member(field(
                "fresh",
                next,
                primitive_type(v2::PrimitiveType::Integer),
            )));
        }
        // DeclAdded, composite append: an enum value above every live and
        // retired number.
        1 => {
            let Some(v2::decl::Kind::EnumDef(def)) = &mut decl_mut(&mut new, "Mode").kind else {
                unreachable!("Mode is an enum");
            };
            let next = enum_next_value(def);
            def.values.push(v2::EnumValue {
                name: "VFRESH".to_string(),
                value: next,
                doc: String::new(),
            });
        }
        // DeclAdded, composite append: a union arm at the next ordinal, on a
        // union that is not a result union.
        2 => {
            let Some(v2::decl::Kind::UnionDef(def)) = &mut decl_mut(&mut new, "Choice").kind else {
                unreachable!("Choice is a union");
            };
            let next = union_next_ordinal(def);
            def.arms.push(v2::UnionArm {
                name: "fresh_arm".to_string(),
                ordinal: next,
                type_ref: "Mode".to_string(),
                doc: String::new(),
            });
        }
        // DeclAdded, composite append: an enum-set bit above every live one.
        3 => {
            let Some(v2::decl::Kind::EnumSetDef(def)) = &mut decl_mut(&mut new, "Flags").kind
            else {
                unreachable!("Flags is an enum set");
            };
            let next = def.bits.iter().map(|bit| bit.value).max().unwrap_or(-1) + 1;
            def.bits.push(v2::EnumValue {
                name: "BFRESH".to_string(),
                value: next,
                doc: String::new(),
            });
        }
        // DeclAdded, package level: a whole new declaration.
        4 => {
            new.decls.push(decl(
                "Extra",
                v2::Visibility::Public,
                v2::decl::Kind::StructDef(v2::StructDef {
                    members: vec![live_member(field(
                        "a",
                        1,
                        primitive_type(v2::PrimitiveType::Integer),
                    ))],
                    fixed_layout: false,
                }),
            ));
        }
        // InteractionAppended: a new interaction after every slot that
        // existed before (`appended`).
        5 => {
            let interface = &mut new.interfaces[0];
            let next = interface_next_ordinal(interface);
            interface
                .interactions
                .push(interaction("fresh_sig", next, signal("Data")));
        }
        // InteractionAppended: a freshly minted tombstone at the next
        // ordinal — the parenthetical case of `appended`'s doc comment.
        6 => {
            let interface = &mut new.interfaces[0];
            let next = interface_next_ordinal(interface);
            interface
                .interactions
                .push(interaction_tombstone("fresh_reserved", next));
        }
        // InteractionRetired: a live interaction replaced by a tombstone in
        // its own slot (ridl §11).
        7 => {
            let slot = first_signal_mut(&mut new);
            let name = slot.name.clone();
            *slot = interaction_tombstone(&name, slot.ordinal);
        }
        // ConstraintChanged, widened: the declared range's ceiling raised
        // (`constraint` → `narrows` is false).
        8 => {
            let Some(v2::decl::Kind::TypeDef(def)) = &mut decl_mut(&mut new, "Bound").kind else {
                unreachable!("Bound is a named scalar");
            };
            let constraint = def.constraint.as_mut().expect("Bound declares a range");
            let max: u64 = constraint
                .max
                .as_deref()
                .expect("the range has a ceiling")
                .parse()
                .expect("the ceiling is a canonical decimal");
            constraint.max = Some((max + 50).to_string());
        }
        // VisibilityChanged: internal widened to public (`visibility`).
        9 => {
            decl_mut(&mut new, "Choice").visibility = v2::Visibility::Public as i32;
        }
        // DocOnly: a doc comment edit reaches no consumer's build.
        10 => {
            decl_mut(&mut new, "Data").doc = "Updated documentation.".to_string();
        }
        // TimingChanged, compatible direction: the rate floor raised
        // (`timing`).
        11 => {
            let Some(v2::decl::Kind::SignalDef(def)) = &mut first_signal_mut(&mut new).kind else {
                unreachable!("found a signal above");
            };
            def.timing.as_mut().expect("signals carry timing").min_us = Some("20000".to_string());
        }
        // ContractChanged, compatible direction: a require clause removed
        // (`contract`).
        12 => {
            command_mut(&mut new).contracts = Vec::new();
        }
        // RpcBoundChanged, compatible direction: the call throttle lowered
        // (`rpc_bound`).
        13 => {
            command_mut(&mut new)
                .timing
                .as_mut()
                .expect("the command declares an RPC bound")
                .min_us = Some("500".to_string());
        }
        _ => unreachable!("the strategy draws indices below MUTATION_COUNT"),
    }
    new
}

// ==========================================================================
// The strategy.
// ==========================================================================

/// A `(old, new)` package pair one compatible edit apart: a base package of
/// generated shape, and a copy with one mutation from the set above applied.
fn compatible_delta_strategy() -> impl Strategy<Value = (v2::Package, v2::Package)> {
    let primitive = prop_oneof![
        Just(v2::PrimitiveType::Boolean),
        Just(v2::PrimitiveType::Integer),
        Just(v2::PrimitiveType::Float),
        Just(v2::PrimitiveType::String),
        Just(v2::PrimitiveType::Bytes),
    ];
    let shape = (
        (
            prop::collection::vec(primitive, 1..=3),
            any::<bool>(),
            1i64..=3i64,
            any::<bool>(),
            1i64..=2i64,
        ),
        (any::<bool>(), 1u32..=2u32, any::<bool>(), 50u64..=100u64),
    )
        .prop_map(
            |(
                (field_primitives, struct_tombstone, enum_value_count, enum_retired, bit_count),
                (union_tombstone, signal_count, interface_tombstone, bound_max),
            )| BaseShape {
                field_primitives,
                struct_tombstone,
                enum_value_count,
                enum_retired,
                bit_count,
                union_tombstone,
                signal_count,
                interface_tombstone,
                bound_max,
            },
        );
    (shape, 0..MUTATION_COUNT).prop_map(|(shape, mutation)| {
        let old = base_package(&shape);
        let new = apply_mutation(&old, mutation);
        (old, new)
    })
}

// ==========================================================================
// The discard rate.
// ==========================================================================

/// The property above discards a delta the classifier calls breaking
/// (`prop_assume!`), so its strength depends on the discard rate: a strategy
/// producing mostly breaking deltas would stay green while testing almost
/// nothing. Every mutation is built to classify compatible, so the rate is
/// pinned at zero here rather than watched by hand. A failure means a
/// mutation drifted from the classifier, or the classifier's own rule moved —
/// find out which before touching this assertion.
#[test]
fn every_generated_delta_classifies_compatible() {
    use proptest::strategy::ValueTree;
    use proptest::test_runner::TestRunner;

    let mut runner = TestRunner::deterministic();
    let strategy = compatible_delta_strategy();
    let total = 1024u32;
    let mut compatible = 0u32;
    for _ in 0..total {
        let (old, new) = strategy
            .new_tree(&mut runner)
            .expect("the strategy generates")
            .current();
        if ridl_diff::diff_packages(&old, &new).verdict == ridl_diff::Verdict::Compatible {
            compatible += 1;
        }
    }
    println!("compatible deltas: {compatible} of {total}");
    assert_eq!(
        compatible, total,
        "{compatible} of {total} generated deltas classify compatible; the rest would be \
         discarded by prop_assume! in the stability property"
    );
}
