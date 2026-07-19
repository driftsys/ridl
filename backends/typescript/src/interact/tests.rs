//! Interaction-layer tests: one snapshot per interaction kind, the Appendix A
//! corpus (every kind in one interface), services in both forms, the timing
//! and contract data, totality (`GenerateError`, never a panic), and the
//! best-effort tsc strict-compile check.

use crate::{GenerateError, generate};
use ridl_ir::v2;

// ---------------------------------------------------------------------------
// Fixture builders.
// ---------------------------------------------------------------------------

/// An interaction inside an interface. Visibility and `is_error` stay unset
/// on interactions (ridl §14.1); the ordinal is the 1-based declaration order
/// across all interactions of the enclosing interface, tombstones counted
/// (ridl §11).
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

fn documented(name: &str, ordinal: u32, doc: &str, kind: v2::decl::Kind) -> v2::Decl {
    v2::Decl {
        doc: doc.to_string(),
        ..interaction(name, ordinal, kind)
    }
}

fn reserved(ordinal: u32, name: &str) -> v2::Decl {
    interaction(
        "",
        ordinal,
        v2::decl::Kind::ReservedSlot(v2::Reserved {
            ordinal,
            name: Some(name.to_string()),
            value: None,
        }),
    )
}

fn named(name: &str) -> v2::FieldType {
    v2::FieldType {
        optional: false,
        kind: Some(v2::field_type::Kind::Named(name.to_string())),
    }
}

fn stream_named(name: &str) -> v2::FieldType {
    v2::FieldType {
        optional: false,
        kind: Some(v2::field_type::Kind::Stream(v2::StreamType {
            element: Some(v2::stream_type::Element::Named(name.to_string())),
        })),
    }
}

fn stream_primitive(prim: v2::PrimitiveType) -> v2::FieldType {
    v2::FieldType {
        optional: false,
        kind: Some(v2::field_type::Kind::Stream(v2::StreamType {
            element: Some(v2::stream_type::Element::Primitive(prim as i32)),
        })),
    }
}

fn param(name: &str, ty: v2::FieldType) -> v2::Param {
    v2::Param {
        name: name.to_string(),
        r#type: Some(ty),
    }
}

fn timing(mode: v2::TimingMode, min: Option<&str>, max: Option<&str>, applied: bool) -> v2::Timing {
    v2::Timing {
        mode: mode as i32,
        min_us: min.map(str::to_string),
        max_us: max.map(str::to_string),
        default_applied: applied,
    }
}

fn signal(payload: &str, init: Option<&str>, timing: v2::Timing) -> v2::decl::Kind {
    v2::decl::Kind::SignalDef(v2::SignalDef {
        payload: payload.to_string(),
        declared_init: None,
        init: Some(v2::InitValue {
            derivable: true,
            value: init.map(str::to_string),
        }),
        timing: Some(timing),
    })
}

fn event(payload: &str, timing: v2::Timing) -> v2::decl::Kind {
    v2::decl::Kind::EventDef(v2::EventDef {
        payload: payload.to_string(),
        timing: Some(timing),
    })
}

fn contract(
    kind: v2::ContractKind,
    source: &str,
    signals: &[&str],
    params: &[&str],
    uses_result: bool,
    observer_id: &str,
) -> v2::Contract {
    v2::Contract {
        kind: kind as i32,
        source: source.to_string(),
        signal_refs: signals.iter().map(|s| s.to_string()).collect(),
        param_refs: params.iter().map(|s| s.to_string()).collect(),
        uses_result,
        observer_id: observer_id.to_string(),
    }
}

fn command(params: Vec<v2::Param>, contracts: Vec<v2::Contract>) -> v2::decl::Kind {
    v2::decl::Kind::CommandDef(v2::CommandDef { params, contracts })
}

fn query(
    params: Vec<v2::Param>,
    return_type: v2::return_type::Kind,
    contracts: Vec<v2::Contract>,
) -> v2::decl::Kind {
    v2::decl::Kind::QueryDef(v2::QueryDef {
        params,
        return_type: Some(v2::ReturnType {
            kind: Some(return_type),
        }),
        contracts,
    })
}

fn fallible(ok: &str, err: &str) -> v2::return_type::Kind {
    v2::return_type::Kind::Fallible(v2::FallibleType {
        ok: ok.to_string(),
        err: err.to_string(),
    })
}

fn final_def(payload: v2::FieldType) -> v2::decl::Kind {
    v2::decl::Kind::FinalDef(v2::FinalDef {
        payload: Some(payload),
    })
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

fn service(name: &str, shape: v2::service::Shape) -> v2::Service {
    v2::Service {
        name: name.to_string(),
        visibility: v2::Visibility::Public as i32,
        doc: String::new(),
        labels: Vec::new(),
        deprecated: None,
        shape: Some(shape),
    }
}

/// A package carrying interactions only — the typl surface is supplied by the
/// referenced packages, so each per-kind snapshot shows the interaction
/// mapping alone.
fn interact_package(interfaces: Vec<v2::Interface>, services: Vec<v2::Service>) -> v2::Package {
    v2::Package {
        name: "veh.cluster".to_string(),
        decls: Vec::new(),
        interfaces,
        services,
    }
}

/// The single-interaction package behind a per-kind snapshot.
fn one(kind_decl: v2::Decl) -> v2::Package {
    interact_package(
        vec![interface("VehicleStatus", vec![kind_decl])],
        Vec::new(),
    )
}

fn render(package: &v2::Package) -> String {
    generate(package).expect("the package generates").source
}

/// Marks a declaration deprecated with a reason (typl §14.2).
fn deprecate(mut decl: v2::Decl, reason: &str) -> v2::Decl {
    decl.deprecated = Some(reason.to_string());
    decl
}

// ---------------------------------------------------------------------------
// One snapshot per interaction kind.
// ---------------------------------------------------------------------------

/// A signal carries a channel init and a provenance-bearing read (ridl §4.4,
/// §4.5): the consumer face holds a `SignalHandle`, the provider face
/// publishes.
#[test]
fn signal_with_init_and_provenance() {
    let package = one(documented(
        "currentSpeed",
        1,
        "Current vehicle speed — isochronous, drives rmdl clocks downstream",
        signal(
            "veh.common.Speed",
            Some("0.0"),
            timing(
                v2::TimingMode::StrictPeriodic,
                Some("10000"),
                Some("10000"),
                false,
            ),
        ),
    ));
    insta::assert_snapshot!(render(&package));
}

/// An event has occurrences and no readable current value (ridl §5), so the
/// consumer face gets an `EventHandle` with no `read`.
#[test]
fn event_has_no_readable_value() {
    let package = one(documented(
        "doorOpened",
        1,
        "Raised on every door state change; stale after 500ms",
        event(
            "DoorPayload",
            timing(v2::TimingMode::Range, Some("50000"), Some("500000"), false),
        ),
    ));
    insta::assert_snapshot!(render(&package));
}

/// A command declares no result (ridl §6.1) and admits `require` only
/// (ridl §13): the method resolves `void` and the clause lands in the
/// contract data.
#[test]
fn command_with_require() {
    let package = one(documented(
        "setGear",
        1,
        "Request a gear change — outcome observed via currentGear, not returned",
        command(
            vec![param("position", named("veh.common.GearPosition"))],
            vec![contract(
                v2::ContractKind::Require,
                "position != GearPosition.PARK || currentSpeed == 0.0",
                &["currentSpeed"],
                &["position"],
                false,
                "VehicleStatus.setGear.require[0]",
            )],
        ),
    ));
    insta::assert_snapshot!(render(&package));
}

/// A fallible query returns `Promise<Result<Ok, Err>>` and carries the
/// synthesized transport identity as a JSDoc tag (ADR-0008 decision 4). The
/// ordinal is part of the identity, so it is pinned here at 9 — Appendix A's
/// position for this query.
#[test]
fn fallible_query_carries_transport_identity() {
    let package = one(documented(
        "getFaultPage",
        9,
        "Paged fault snapshot — fallible query (ridl §10.1)",
        query(
            vec![param("filter", named("DiagFilter"))],
            fallible("FaultPage", "DiagError"),
            vec![contract(
                v2::ContractKind::Ensure,
                "result.faults.length <= filter.limit",
                &[],
                &["filter"],
                true,
                "VehicleStatus.getFaultPage.ensure[0]",
            )],
        ),
    ));
    let source = render(&package);
    assert!(
        source.contains("@transportIdentity VehicleStatus#9:FaultPage|DiagError"),
        "the transport identity must come from the IR derivation, got:\n{source}"
    );
    insta::assert_snapshot!(source);
}

/// A named-field tuple return (ridl §7.1) becomes an inline object type.
#[test]
fn tuple_return_query() {
    let tuple = v2::FieldType {
        optional: false,
        kind: Some(v2::field_type::Kind::Tuple(v2::TupleType {
            fields: vec![
                tuple_field("min", "veh.common.Speed"),
                tuple_field("max", "veh.common.Speed"),
            ],
        })),
    };
    let package = one(interaction(
        "getMinMax",
        1,
        query(
            vec![param("window", named("ridl.std.Duration"))],
            v2::return_type::Kind::Value(tuple),
            Vec::new(),
        ),
    ));
    insta::assert_snapshot!(render(&package));
}

fn tuple_field(name: &str, type_ref: &str) -> v2::TupleField {
    v2::TupleField {
        name: name.to_string(),
        r#type: Some(named(type_ref)),
    }
}

/// A stream on both the parameter and the return is bidirectional
/// (ridl §12.1). A stream return is an `AsyncIterable` directly, not wrapped
/// in a promise — it is consumed as it arrives.
#[test]
fn bidirectional_stream_query() {
    let package = one(documented(
        "pipe",
        1,
        "Full-duplex processing pipeline",
        query(
            vec![
                param("input", stream_named("SensorSample")),
                param("tag", stream_primitive(v2::PrimitiveType::String)),
            ],
            v2::return_type::Kind::Value(stream_named("ProcessedSample")),
            Vec::new(),
        ),
    ));
    let source = render(&package);
    assert!(
        source.contains("pipe(input: AsyncIterable<SensorSample>, tag: AsyncIterable<string>): AsyncIterable<ProcessedSample>;"),
        "a stream return is not promise-wrapped, got:\n{source}"
    );
    insta::assert_snapshot!(source);
}

/// A final is a provisioned constant (ridl §8): a plain readonly property on
/// both faces, collections permitted.
#[test]
fn final_with_array_type() {
    let capabilities = v2::FieldType {
        optional: false,
        kind: Some(v2::field_type::Kind::Array(Box::new(v2::ArrayType {
            element: Some(Box::new(named("ridl.std.Label"))),
            min: 0,
            max: 32,
        }))),
    };
    let package = one(interaction("capabilities", 1, final_def(capabilities)));
    insta::assert_snapshot!(render(&package));
}

// ---------------------------------------------------------------------------
// Timing and contract data.
// ---------------------------------------------------------------------------

/// Timing bounds are bigint microseconds, exact (ADR-0008 decision 12): a
/// strict period fills both bounds, a resolved default is marked, and the
/// absent side of an explicit half-open range is `undefined`.
#[test]
fn timing_modes_and_bounds() {
    let package = interact_package(
        vec![interface(
            "Timings",
            vec![
                interaction(
                    "strict",
                    1,
                    signal(
                        "Speed",
                        Some("0.0"),
                        timing(
                            v2::TimingMode::StrictPeriodic,
                            Some("10000"),
                            Some("10000"),
                            false,
                        ),
                    ),
                ),
                interaction(
                    "defaulted",
                    2,
                    event(
                        "Ping",
                        timing(v2::TimingMode::Range, Some("20000"), Some("500000"), true),
                    ),
                ),
                interaction(
                    "halfOpen",
                    3,
                    signal(
                        "Speed",
                        Some("0.0"),
                        timing(v2::TimingMode::Range, Some("1000000"), None, false),
                    ),
                ),
                // A bound past 2^53 microseconds proves the bigint choice:
                // as a `number` literal it would round.
                interaction(
                    "enormous",
                    4,
                    event(
                        "Ping",
                        timing(
                            v2::TimingMode::Range,
                            Some("9007199254740993"),
                            Some("9007199254740995"),
                            false,
                        ),
                    ),
                ),
            ],
        )],
        Vec::new(),
    );
    let source = render(&package);
    assert!(
        source.contains("minUs: 9007199254740993n, maxUs: 9007199254740995n"),
        "a bound past Number.MAX_SAFE_INTEGER must survive exactly, got:\n{source}"
    );
    insta::assert_snapshot!(source);
}

/// An interface with no timing and no contracts still emits both consts, so
/// the generated surface is the same shape for every interface.
#[test]
fn empty_timing_and_contract_consts() {
    let package = one(interaction("vin", 1, final_def(named("Vin"))));
    let source = render(&package);
    assert!(source.contains("export const vehicleStatusTiming = {} as const;"));
    assert!(source.contains("export const vehicleStatusContracts = [] as const;"));
}

// ---------------------------------------------------------------------------
// Services, both forms.
// ---------------------------------------------------------------------------

/// A service is either a named interface reference or an inline shape
/// (ridl §14.5). The inline shape's generated interface is named after the
/// service; the service's own identity stays the dotted name, which is what
/// the `services` map is keyed by.
#[test]
fn services_named_reference_and_inline_shape() {
    let tail_logs = interaction(
        "tailLogs",
        1,
        query(
            vec![param(
                "pattern",
                stream_primitive(v2::PrimitiveType::String),
            )],
            v2::return_type::Kind::Value(stream_named("LogLine")),
            Vec::new(),
        ),
    );
    let package = interact_package(
        vec![interface(
            "CruiseControl",
            vec![interaction(
                "engaged",
                1,
                signal(
                    "Engaged",
                    Some("false"),
                    timing(
                        v2::TimingMode::StrictPeriodic,
                        Some("50000"),
                        Some("50000"),
                        false,
                    ),
                ),
            )],
        )],
        vec![
            service(
                "veh.adas.cruise",
                v2::service::Shape::InterfaceRef("CruiseControl".to_string()),
            ),
            service(
                "veh.adas.logs",
                v2::service::Shape::Inline(interface("", vec![tail_logs])),
            ),
        ],
    );
    let source = render(&package);
    assert!(
        source.contains("'veh.adas.cruise': { interface: 'CruiseControl' },"),
        "a named service maps to its interface, got:\n{source}"
    );
    assert!(
        source.contains("export interface Service_veh_adas_logsConsumer {"),
        "an inline shape names its generated interface after the service, got:\n{source}"
    );
    insta::assert_snapshot!(source);
}

// ---------------------------------------------------------------------------
// The Appendix A corpus — every interaction kind in one interface.
// ---------------------------------------------------------------------------

/// The ridl reference Appendix A contract, with the gf §6.1 errata applied:
/// the fallible query's named result union becomes the inline `T | E` return
/// (ADR-0008 decision 1). Ordinals follow declaration order with the
/// tombstone counted (ridl §11), which puts `getFaultPage` at 9.
fn appendix_a() -> v2::Package {
    let decls = vec![
        // struct DoorPayload { sensorId : integer [0..15]; isOpen : boolean }
        struct_decl(
            "DoorPayload",
            vec![
                scalar_field("sensorId", 1, v2::PrimitiveType::Integer),
                scalar_field("isOpen", 2, v2::PrimitiveType::Boolean),
            ],
        ),
        // struct DiagFilter { severity : integer [0..5]; category : Label? }
        struct_decl(
            "DiagFilter",
            vec![
                scalar_field("severity", 1, v2::PrimitiveType::Integer),
                optional_named_field("category", 2, "ridl.std.Label"),
            ],
        ),
        // struct FaultEvent { code; message : Message; timestamp : Timestamp }
        struct_decl(
            "FaultEvent",
            vec![
                scalar_field("code", 1, v2::PrimitiveType::Integer),
                named_field("message", 2, "ridl.std.Message"),
                named_field("timestamp", 3, "ridl.std.Timestamp"),
            ],
        ),
        // error enum DiagError { … }
        v2::Decl {
            name: "DiagError".to_string(),
            visibility: v2::Visibility::Public as i32,
            is_error: true,
            doc: "Failure vocabulary — typl §10.1".to_string(),
            labels: Vec::new(),
            deprecated: None,
            ordinal: 0,
            kind: Some(v2::decl::Kind::EnumDef(v2::EnumDef {
                values: vec![
                    enum_value("FILTER_INVALID", 0),
                    enum_value("STORAGE_BUSY", 1),
                    enum_value("ACCESS_DENIED", 2),
                ],
                reserved: Vec::new(),
            })),
        },
        // struct FaultPage { faults : [FaultEvent; 0..64] }
        struct_decl(
            "FaultPage",
            vec![v2::Field {
                name: "faults".to_string(),
                ordinal: 1,
                r#type: Some(v2::FieldType {
                    optional: false,
                    kind: Some(v2::field_type::Kind::Array(Box::new(v2::ArrayType {
                        element: Some(Box::new(named("FaultEvent"))),
                        min: 0,
                        max: 64,
                    }))),
                }),
                declared_init: None,
                init: None,
                doc: String::new(),
                labels: Vec::new(),
                deprecated: None,
            }],
        ),
    ];

    let vehicle_status = v2::Interface {
        name: "VehicleStatus".to_string(),
        visibility: v2::Visibility::Public as i32,
        doc: "Main vehicle status interface.".to_string(),
        labels: vec!["SIL_B".to_string(), "CAL_2".to_string()],
        deprecated: None,
        interactions: vec![
            documented(
                "currentSpeed",
                1,
                "Current vehicle speed — isochronous, drives rmdl clocks downstream",
                signal(
                    "veh.common.Speed",
                    Some("0.0"),
                    timing(
                        v2::TimingMode::StrictPeriodic,
                        Some("10000"),
                        Some("10000"),
                        false,
                    ),
                ),
            ),
            documented(
                "engineTemp",
                2,
                "Engine temperature — change-driven, 100ms freshness SLO",
                signal(
                    "veh.common.Temperature",
                    Some("0.0"),
                    timing(v2::TimingMode::Range, Some("20000"), Some("100000"), false),
                ),
            ),
            documented(
                "warnings",
                3,
                "Active warnings — last-value delivered on subscribe (ridl §4.4)",
                signal(
                    "veh.common.WarningFlags",
                    Some("0"),
                    timing(v2::TimingMode::Range, Some("50000"), Some("1000000"), false),
                ),
            ),
            documented(
                "doorOpened",
                4,
                "Raised on every door state change; stale after 500ms",
                event(
                    "DoorPayload",
                    timing(v2::TimingMode::Range, Some("50000"), Some("500000"), false),
                ),
            ),
            documented(
                "setGear",
                5,
                "Request a gear change — outcome observed via currentGear, not returned",
                command(
                    vec![param("position", named("veh.common.GearPosition"))],
                    vec![contract(
                        v2::ContractKind::Require,
                        "position != GearPosition.PARK || currentSpeed == 0.0",
                        &["currentSpeed"],
                        &["position"],
                        false,
                        "VehicleStatus.setGear.require[0]",
                    )],
                ),
            ),
            // reserved resetCounters — a retired ordinal, never reused.
            reserved(6, "resetCounters"),
            documented(
                "getAverageSpeed",
                7,
                "Sliding-window average",
                query(
                    vec![param("window", named("ridl.std.Duration"))],
                    v2::return_type::Kind::Value(named("veh.common.Speed")),
                    vec![
                        contract(
                            v2::ContractKind::Require,
                            "window > 0ms",
                            &[],
                            &["window"],
                            false,
                            "VehicleStatus.getAverageSpeed.require[0]",
                        ),
                        contract(
                            v2::ContractKind::Ensure,
                            "result >= 0.0",
                            &[],
                            &[],
                            true,
                            "VehicleStatus.getAverageSpeed.ensure[0]",
                        ),
                    ],
                ),
            ),
            documented(
                "streamFaults",
                8,
                "Fault history as a finite stream",
                query(
                    vec![param("filter", named("DiagFilter"))],
                    v2::return_type::Kind::Value(stream_named("FaultEvent")),
                    Vec::new(),
                ),
            ),
            documented(
                "getFaultPage",
                9,
                "Paged fault snapshot — fallible query via the inline `T | E` return \
                 (ridl §10.1, gf §6.1)",
                query(
                    vec![param("filter", named("DiagFilter"))],
                    fallible("FaultPage", "DiagError"),
                    Vec::new(),
                ),
            ),
            interaction("softwareVersion", 10, final_def(named("ridl.std.Version"))),
            interaction(
                "capabilities",
                11,
                final_def(v2::FieldType {
                    optional: false,
                    kind: Some(v2::field_type::Kind::Array(Box::new(v2::ArrayType {
                        element: Some(Box::new(named("ridl.std.Label"))),
                        min: 0,
                        max: 32,
                    }))),
                }),
            ),
        ],
    };

    // Both service forms, so one corpus proves the whole mapping.
    let services = vec![
        service(
            "veh.cluster.status",
            v2::service::Shape::InterfaceRef("VehicleStatus".to_string()),
        ),
        service(
            "veh.cluster.logs",
            v2::service::Shape::Inline(interface(
                "",
                vec![interaction(
                    "tailLogs",
                    1,
                    query(
                        vec![param(
                            "pattern",
                            stream_primitive(v2::PrimitiveType::String),
                        )],
                        v2::return_type::Kind::Value(stream_primitive(v2::PrimitiveType::Bytes)),
                        Vec::new(),
                    ),
                )],
            )),
        ),
    ];

    v2::Package {
        name: "veh.cluster".to_string(),
        decls,
        interfaces: vec![vehicle_status],
        services,
    }
}

fn struct_decl(name: &str, fields: Vec<v2::Field>) -> v2::Decl {
    v2::Decl {
        name: name.to_string(),
        visibility: v2::Visibility::Public as i32,
        is_error: false,
        doc: String::new(),
        labels: Vec::new(),
        deprecated: None,
        ordinal: 0,
        kind: Some(v2::decl::Kind::StructDef(v2::StructDef {
            members: fields
                .into_iter()
                .map(|field| v2::StructMember {
                    member: Some(v2::struct_member::Member::Field(field)),
                })
                .collect(),
            fixed_layout: false,
        })),
    }
}

fn scalar_field(name: &str, ordinal: u32, prim: v2::PrimitiveType) -> v2::Field {
    v2::Field {
        name: name.to_string(),
        ordinal,
        r#type: Some(v2::FieldType {
            optional: false,
            kind: Some(v2::field_type::Kind::Primitive(prim as i32)),
        }),
        declared_init: None,
        init: None,
        doc: String::new(),
        labels: Vec::new(),
        deprecated: None,
    }
}

fn named_field(name: &str, ordinal: u32, type_ref: &str) -> v2::Field {
    v2::Field {
        name: name.to_string(),
        ordinal,
        r#type: Some(named(type_ref)),
        declared_init: None,
        init: None,
        doc: String::new(),
        labels: Vec::new(),
        deprecated: None,
    }
}

fn optional_named_field(name: &str, ordinal: u32, type_ref: &str) -> v2::Field {
    v2::Field {
        r#type: Some(v2::FieldType {
            optional: true,
            ..named(type_ref)
        }),
        ..named_field(name, ordinal, type_ref)
    }
}

fn enum_value(name: &str, value: i64) -> v2::EnumValue {
    v2::EnumValue {
        name: name.to_string(),
        value,
        doc: String::new(),
    }
}

#[test]
fn appendix_a_snapshot() {
    insta::assert_snapshot!(render(&appendix_a()));
}

/// The Stratum-3 wording is normative and must appear verbatim (gf §6.4) —
/// with the em dash, and never as "undefined behavior".
#[test]
fn stratum_three_wording_is_verbatim() {
    let source = render(&appendix_a());
    assert!(
        source.contains("infrastructure failure \u{2014} detected, undeclared"),
        "the gf §6.4 wording must appear verbatim, got:\n{source}"
    );
    assert!(
        !source.to_lowercase().contains("undefined behavior")
            && !source.to_lowercase().contains("undefined behaviour"),
        "Stratum 3 is never described as undefined behavior (gf §6.4), got:\n{source}"
    );
}

/// Generation is deterministic: the same package renders byte-identically.
#[test]
fn generation_is_deterministic() {
    assert_eq!(render(&appendix_a()), render(&appendix_a()));
}

/// A package with no interfaces and no services carries no interaction
/// vocabulary — a typl-only module is unchanged by this backend.
#[test]
fn typl_only_package_emits_no_vocabulary() {
    let package = v2::Package {
        name: "veh.common".to_string(),
        decls: vec![struct_decl(
            "Empty",
            vec![scalar_field("flag", 1, v2::PrimitiveType::Boolean)],
        )],
        interfaces: Vec::new(),
        services: Vec::new(),
    };
    let source = render(&package);
    assert!(!source.contains("Provenance"), "got:\n{source}");
    assert!(!source.contains("SignalHandle"), "got:\n{source}");
}

// ---------------------------------------------------------------------------
// Transport identity inside an inline service shape.
// ---------------------------------------------------------------------------

/// A fallible query inside a service's inline shape takes its transport
/// identity from the service's DOTTED global name.
///
/// An inline shape's `Interface.name` is `""` by construction (ridl §14.5),
/// so an emitter that reached for it would produce `#3:FaultPage|DiagError` —
/// no interface component at all, and therefore not an identity: a second
/// service with a fallible query at the same ordinal over the same arms would
/// emit the identical string. The dotted name is also what `ridl diff` keys a
/// service's interactions on and what the observer stubs in this same module
/// are scoped to (E2.5), so all three have to agree.
#[test]
fn inline_service_fallible_query_uses_the_dotted_service_name() {
    let fetch = interaction(
        "fetchFaults",
        3,
        query(
            vec![param("filter", named("DiagFilter"))],
            fallible("FaultPage", "DiagError"),
            Vec::new(),
        ),
    );
    let package = interact_package(
        Vec::new(),
        vec![service(
            "veh.adas.logs",
            v2::service::Shape::Inline(interface("", vec![fetch])),
        )],
    );
    let source = render(&package);

    assert!(
        source.contains("@transportIdentity veh.adas.logs#3:FaultPage|DiagError"),
        "the identity must carry the dotted service name, got:\n{source}"
    );
    // The empty-interface-name regression, named directly: the bug emitted an
    // identity whose interface component was blank.
    assert!(
        !source.contains("@transportIdentity #3:"),
        "the identity must never lose its interface component, got:\n{source}"
    );
    // The emitted string is exactly what the single IR derivation produces —
    // no separate spelling of the rule lives in this backend.
    let expected = v2::fallible_transport_identity(
        "veh.adas.logs",
        3,
        &v2::FallibleType {
            ok: "FaultPage".to_string(),
            err: "DiagError".to_string(),
        },
    );
    assert!(
        source.contains(&format!("@transportIdentity {expected}")),
        "the identity must match the IR helper ({expected}), got:\n{source}"
    );
}

/// A named interface keeps using its own name, so threading the identity
/// separately from the generated type name did not disturb the common case.
#[test]
fn named_interface_identity_is_the_interface_name() {
    let package = one(interaction(
        "getFaultPage",
        9,
        query(
            vec![param("filter", named("DiagFilter"))],
            fallible("FaultPage", "DiagError"),
            Vec::new(),
        ),
    ));
    assert!(render(&package).contains("@transportIdentity VehicleStatus#9:FaultPage|DiagError"));
}

// ---------------------------------------------------------------------------
// Attribute surfaces that the per-kind snapshots do not exercise.
// ---------------------------------------------------------------------------

/// An optional parameter uses the `name?:` property form (typl §7.1 carried
/// into interaction position) — absence is in the signature, not a
/// `| undefined` union.
#[test]
fn optional_parameter_uses_the_question_mark_form() {
    let package = one(interaction(
        "search",
        1,
        query(
            vec![
                param("filter", named("DiagFilter")),
                param(
                    "cursor",
                    v2::FieldType {
                        optional: true,
                        ..named("Cursor")
                    },
                ),
            ],
            v2::return_type::Kind::Value(named("FaultPage")),
            Vec::new(),
        ),
    ));
    let source = render(&package);
    assert!(
        source.contains("search(filter: DiagFilter, cursor?: Cursor): Promise<FaultPage>;"),
        "an optional parameter must render as `cursor?:`, got:\n{source}"
    );
    // The marker is what carries the optionality; without it the signature
    // would demand an argument the contract says is optional.
    assert!(
        !source.contains("cursor: Cursor"),
        "the required form must not be emitted for an optional parameter, got:\n{source}"
    );
}

/// `@deprecated` reaches every interaction kind and the interface itself
/// (typl §14.2), on both faces.
#[test]
fn deprecated_reaches_interactions_and_both_faces() {
    let mut iface = interface(
        "Legacy",
        vec![
            deprecate(
                interaction(
                    "oldSpeed",
                    1,
                    signal(
                        "Speed",
                        Some("0.0"),
                        timing(
                            v2::TimingMode::StrictPeriodic,
                            Some("10000"),
                            Some("10000"),
                            false,
                        ),
                    ),
                ),
                "use currentSpeed",
            ),
            deprecate(
                interaction(
                    "oldPing",
                    2,
                    event(
                        "Ping",
                        timing(v2::TimingMode::Range, Some("1000"), Some("2000"), false),
                    ),
                ),
                "no replacement",
            ),
            deprecate(
                interaction("oldSet", 3, command(Vec::new(), Vec::new())),
                "use setGear",
            ),
            deprecate(
                interaction(
                    "oldGet",
                    4,
                    query(
                        Vec::new(),
                        v2::return_type::Kind::Value(named("Speed")),
                        Vec::new(),
                    ),
                ),
                "use getAverageSpeed",
            ),
            deprecate(interaction("oldVin", 5, final_def(named("Vin"))), "use vin"),
        ],
    );
    iface.deprecated = Some("superseded by VehicleStatus".to_string());
    let package = interact_package(vec![iface], Vec::new());
    let source = render(&package);

    for reason in [
        "@deprecated use currentSpeed",
        "@deprecated no replacement",
        "@deprecated use setGear",
        "@deprecated use getAverageSpeed",
        "@deprecated use vin",
        "@deprecated superseded by VehicleStatus",
    ] {
        // Twice each: once per face.
        assert_eq!(
            source.matches(reason).count(),
            2,
            "{reason:?} must appear on both faces, got:\n{source}"
        );
    }
}

/// A reserved tombstone occupies an ordinal but emits no member (ridl §11):
/// the interactions around it are unaffected, and nothing named after the
/// retired interaction appears anywhere in the module.
#[test]
fn reserved_tombstone_emits_no_member() {
    let package = interact_package(
        vec![interface(
            "VehicleStatus",
            vec![
                interaction(
                    "speed",
                    1,
                    signal(
                        "Speed",
                        Some("0.0"),
                        timing(
                            v2::TimingMode::StrictPeriodic,
                            Some("10000"),
                            Some("10000"),
                            false,
                        ),
                    ),
                ),
                reserved(2, "resetCounters"),
                interaction("vin", 3, final_def(named("Vin"))),
            ],
        )],
        Vec::new(),
    );
    let source = render(&package);

    assert!(
        !source.contains("resetCounters"),
        "a retired interaction must not appear in the generated module, got:\n{source}"
    );
    // The tombstone sits between two live interactions; both survive it.
    assert!(
        source.contains("speed: SignalHandle<Speed>;"),
        "got:\n{source}"
    );
    assert!(source.contains("readonly vin: Vin;"), "got:\n{source}");
    // It contributes no timing entry either — a tombstone carries no timing.
    assert!(
        !source.contains("undefined, maxUs: undefined"),
        "a tombstone must not reach the timing table, got:\n{source}"
    );
}

// ---------------------------------------------------------------------------
// Totality — every failure is a value, never a panic.
// ---------------------------------------------------------------------------

/// A typl declaration colliding with the generated vocabulary is named here
/// rather than left for `tsc` to reject.
#[test]
fn vocabulary_name_collision_is_an_error() {
    let mut package = appendix_a();
    package.decls.push(struct_decl(
        "Result",
        vec![scalar_field("flag", 1, v2::PrimitiveType::Boolean)],
    ));
    assert!(matches!(
        generate(&package),
        Err(GenerateError::Unrepresentable(message)) if message.contains("Result")
    ));
}

/// A declaration colliding with a generated face name is likewise refused.
#[test]
fn face_name_collision_is_an_error() {
    let mut package = appendix_a();
    package.decls.push(struct_decl(
        "VehicleStatusProvider",
        vec![scalar_field("flag", 1, v2::PrimitiveType::Boolean)],
    ));
    assert!(matches!(
        generate(&package),
        Err(GenerateError::Unrepresentable(message))
            if message.contains("VehicleStatusProvider")
    ));
}

/// Timing is resolved at compile time (ridl §9.1), so an unresolved mode is a
/// malformed IR — refused with a named reason, not guessed around.
#[test]
fn unresolved_timing_mode_is_an_error() {
    let package = one(interaction(
        "speed",
        1,
        signal(
            "Speed",
            Some("0.0"),
            timing(
                v2::TimingMode::Unspecified,
                Some("10000"),
                Some("10000"),
                false,
            ),
        ),
    ));
    assert!(matches!(
        generate(&package),
        Err(GenerateError::Unrepresentable(message)) if message.contains("no mode")
    ));
}

/// A timing bound that is not an exact integer microsecond count has no
/// bigint literal form; emitting a rounded `number` instead is refused.
#[test]
fn fractional_timing_bound_is_an_error() {
    let package = one(interaction(
        "speed",
        1,
        signal(
            "Speed",
            Some("0.0"),
            timing(v2::TimingMode::Range, Some("10000.5"), Some("20000"), false),
        ),
    ));
    assert!(matches!(
        generate(&package),
        Err(GenerateError::Unrepresentable(message)) if message.contains("10000.5")
    ));
}

/// A service with no shape carries nothing to generate (ridl §14.5).
#[test]
fn service_without_a_shape_is_an_error() {
    let package = v2::Package {
        services: vec![v2::Service {
            shape: None,
            ..service(
                "veh.cluster.status",
                v2::service::Shape::InterfaceRef("VehicleStatus".to_string()),
            )
        }],
        ..appendix_a()
    };
    assert!(matches!(
        generate(&package),
        Err(GenerateError::Unrepresentable(message)) if message.contains("no shape")
    ));
}

// ---------------------------------------------------------------------------
// tsc strict compile — best-effort local evidence, mirroring the typl surface.
// ---------------------------------------------------------------------------

/// The generated TypeScript for the Appendix A package compiles with
/// `tsc --noEmit --strict` when a tsc binary is discoverable; otherwise the
/// check is skipped with a printed notice. The snapshot tests are the gate;
/// this is the evidence that the emitted interaction surface is real
/// TypeScript.
#[test]
fn appendix_a_compiles_with_tsc_strict() {
    let Some(tsc) = crate::tests::discover_tsc() else {
        println!(
            "SKIPPED: no tsc binary discoverable (`tsc` on PATH or `npx --no-install tsc`); \
             the snapshot tests remain the gate"
        );
        return;
    };

    // Minimal modules standing in for the packages Appendix A imports from,
    // matching the module specifiers the cross-package references map to.
    const VEH_COMMON: &str = "\
export type Speed = number & { readonly __ridl: 'veh.common.Speed' };
export type Temperature = number & { readonly __ridl: 'veh.common.Temperature' };
export type WarningFlags = number & { readonly __ridl: 'veh.common.WarningFlags' };
export enum GearPosition {
  PARK = 0,
  DRIVE = 1,
}
";
    const RIDL_STD: &str = "\
export type Label = string & { readonly __ridl: 'ridl.std.Label' };
export type Message = string & { readonly __ridl: 'ridl.std.Message' };
export type Version = string & { readonly __ridl: 'ridl.std.Version' };
export type Timestamp = bigint & { readonly __ridl: 'ridl.std.Timestamp' };
export type Duration = bigint & { readonly __ridl: 'ridl.std.Duration' };
";

    let generated = generate(&appendix_a()).expect("Appendix A generates");

    let dir = tempfile::tempdir().expect("a temp dir is created");
    std::fs::write(dir.path().join("veh.common.ts"), VEH_COMMON).expect("the prelude is written");
    std::fs::write(dir.path().join("ridl.std.ts"), RIDL_STD).expect("the prelude is written");
    let module_path = dir.path().join("veh.cluster.ts");
    std::fs::write(&module_path, &generated.source).expect("the generated source is written");

    let status = std::process::Command::new(&tsc.0)
        .args(&tsc.1)
        .args([
            "--noEmit", "--strict", "--target", "es2020", "--module", "commonjs",
        ])
        .arg(&module_path)
        .status()
        .expect("the discovered tsc must be runnable");
    assert!(
        status.success(),
        "generated TypeScript for Appendix A must compile strict, source:\n{}",
        generated.source
    );
}
