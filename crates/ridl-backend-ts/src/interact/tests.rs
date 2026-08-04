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

/// The nameless tombstone form, `reserved <ordinal>` (typl §7.4 grammar): a
/// slot that records the ordinal it protects and nothing else. `name` is the
/// two spellings of "no retired name" a `Reserved` can carry — absent, which
/// is what the checker lowers, and present-but-empty, which only a hand-built
/// IR produces and which the Rust backend guards against too.
fn reserved_ordinal(ordinal: u32, name: Option<&str>) -> v2::Decl {
    interaction(
        "",
        ordinal,
        v2::decl::Kind::ReservedSlot(v2::Reserved {
            ordinal,
            name: name.map(str::to_string),
            value: Some(i64::from(ordinal)),
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
    v2::decl::Kind::CommandDef(v2::CommandDef {
        params,
        contracts,
        timing: None,
    })
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
        timing: None,
    })
}

fn fallible(ok: &str, err: &str) -> v2::return_type::Kind {
    v2::return_type::Kind::Fallible(v2::FallibleType {
        ok: ok.to_string(),
        err: err.to_string(),
    })
}

fn fixed_def(payload: v2::FieldType) -> v2::decl::Kind {
    v2::decl::Kind::FixedDef(v2::FixedDef {
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

fn service(name: &str, shape: v2::service_shape::Kind) -> v2::Service {
    v2::Service {
        name: name.to_string(),
        visibility: v2::Visibility::Public as i32,
        doc: String::new(),
        labels: Vec::new(),
        deprecated: None,
        shapes: vec![v2::ServiceShape {
            id: 1,
            kind: Some(shape),
        }],
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

/// A fixed is a provisioned constant (ridl §8): a plain readonly property on
/// both faces, collections permitted.
#[test]
fn fixed_with_array_type() {
    let capabilities = v2::FieldType {
        optional: false,
        kind: Some(v2::field_type::Kind::Array(Box::new(v2::ArrayType {
            element: Some(Box::new(named("ridl.std.Label"))),
            min: 0,
            max: 32,
        }))),
    };
    let package = one(interaction("capabilities", 1, fixed_def(capabilities)));
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

/// The declared RPC bounds ride the same timing table as a signal's
/// (ADR-0015): the call throttle and the response bound are the same two
/// bigint constants, on two more kinds. An undeclared RPC contributes no
/// entry — its bounds are never defaulted.
#[test]
fn rpc_bounds_ride_the_timing_table() {
    let package = interact_package(
        vec![interface(
            "VehicleStatus",
            vec![
                interaction(
                    "setGear",
                    1,
                    v2::decl::Kind::CommandDef(v2::CommandDef {
                        params: vec![param("position", named("GearPosition"))],
                        contracts: Vec::new(),
                        timing: Some(timing(
                            v2::TimingMode::Range,
                            Some("20000"),
                            Some("200000"),
                            false,
                        )),
                    }),
                ),
                interaction(
                    "getAverageSpeed",
                    2,
                    v2::decl::Kind::QueryDef(v2::QueryDef {
                        params: Vec::new(),
                        return_type: Some(v2::ReturnType {
                            kind: Some(v2::return_type::Kind::Value(named("Speed"))),
                        }),
                        contracts: Vec::new(),
                        // The half-open `@[..50ms]`: a response bound and no
                        // throttle.
                        timing: Some(timing(v2::TimingMode::Range, None, Some("50000"), false)),
                    }),
                ),
                interaction("resetFaults", 3, command(Vec::new(), Vec::new())),
            ],
        )],
        Vec::new(),
    );
    let source = render(&package);
    assert!(
        source.contains("setGear: { mode: 'range', minUs: 20000n, maxUs: 200000n"),
        "a command's bounds land in the timing table, got:\n{source}"
    );
    assert!(
        source.contains("getAverageSpeed: { mode: 'range', minUs: undefined, maxUs: 50000n"),
        "a query's half-open bounds land in the timing table, got:\n{source}"
    );
    assert!(
        !source.contains("resetFaults: { mode:"),
        "an undeclared RPC contributes no timing entry, got:\n{source}"
    );
    insta::assert_snapshot!(source);
}

/// An interface with no timing and no contracts still emits both consts, so
/// the generated surface is the same shape for every interface.
#[test]
fn empty_timing_and_contract_consts() {
    let package = one(interaction("vin", 1, fixed_def(named("Vin"))));
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
                v2::service_shape::Kind::InterfaceRef("CruiseControl".to_string()),
            ),
            service(
                "veh.adas.logs",
                v2::service_shape::Kind::Inline(interface("", vec![tail_logs])),
            ),
        ],
    );
    let source = render(&package);
    assert!(
        source.contains("'veh.adas.cruise': { interfaces: ['CruiseControl'] },"),
        "a named service maps to its interface list, got:\n{source}"
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
            interaction("softwareVersion", 10, fixed_def(named("ridl.std.Version"))),
            interaction(
                "capabilities",
                11,
                fixed_def(v2::FieldType {
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
            v2::service_shape::Kind::InterfaceRef("VehicleStatus".to_string()),
        ),
        service(
            "veh.cluster.logs",
            v2::service_shape::Kind::Inline(interface(
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
            v2::service_shape::Kind::Inline(interface("", vec![fetch])),
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
            deprecate(interaction("oldVin", 5, fixed_def(named("Vin"))), "use vin"),
        ],
    );
    iface.deprecated = Some("superseded by VehicleStatus".to_string());
    let package = interact_package(vec![iface], Vec::new());
    let source = render(&package);

    // Twice each — once per face — for the four kinds both faces carry, and
    // for the interface's own deprecation.
    for reason in [
        "@deprecated use currentSpeed",
        "@deprecated no replacement",
        "@deprecated use setGear",
        "@deprecated use getAverageSpeed",
        "@deprecated superseded by VehicleStatus",
    ] {
        assert_eq!(
            source.matches(reason).count(),
            2,
            "{reason:?} must appear on both faces, got:\n{source}"
        );
    }
    // A fixed is consumer-only (ridl §3, §8), so its deprecation is emitted
    // once — and on the consumer face specifically.
    assert_eq!(
        source.matches("@deprecated use vin").count(),
        1,
        "a fixed's deprecation belongs to the one face that carries it, got:\n{source}"
    );
    assert!(face_body(&source, "LegacyConsumer").contains("@deprecated use vin"));
}

/// A reserved tombstone declares no member but is recorded with the ordinal
/// it protects (ridl §11).
///
/// The two halves are separate claims and both matter. A tombstone must not
/// become a member — nothing named after a retired interaction can be called,
/// read, or subscribed — and it must not vanish either: it exists to hold an
/// ordinal against reuse, so a TypeScript consumer that cannot see it cannot
/// reconstruct the wire identity across the retired slot, which is the exact
/// property `ridl diff` guards. The wording is the Rust backend's verbatim, so
/// the two generated outputs read as one system.
#[test]
fn reserved_tombstone_is_recorded_with_its_ordinal() {
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
                interaction("vin", 3, fixed_def(named("Vin"))),
            ],
        )],
        Vec::new(),
    );
    let source = render(&package);

    assert!(
        source.contains("@reserved ordinal 2 (`resetCounters`) — retired, never reused."),
        "the retired ordinal must be recorded, got:\n{source}"
    );
    // Recorded once, on the consumer face — the placement the Rust backend
    // gives interface-level metadata.
    assert_eq!(
        source.matches("@reserved ordinal 2").count(),
        1,
        "a tombstone is recorded once, got:\n{source}"
    );
    assert!(
        source
            .split("export interface VehicleStatusConsumer")
            .next()
            .expect("the split yields the text before the consumer face")
            .contains("@reserved ordinal 2"),
        "the record belongs to the consumer face's own doc, got:\n{source}"
    );
    // Recorded, but never a member: the retired name is not callable,
    // readable, or subscribable on either face.
    for face in ["VehicleStatusConsumer", "VehicleStatusProvider"] {
        assert!(
            !face_body(&source, face).contains("resetCounters"),
            "a retired interaction must not become a member of {face}, got:\n{source}"
        );
    }
    // The tombstone sits between two live interactions; both survive it, and
    // each states the ordinal the tombstone displaced it to.
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

/// The nameless form, `reserved 3`, states the ordinal alone (typl §7.4
/// grammar). There is no retired name to quote, so the record must not carry
/// an empty pair of backticks where a name would go.
///
/// Both spellings of "no name" are covered: `None`, which is what the checker
/// lowers for `reserved <ordinal>`, and `Some("")`, which only a hand-built IR
/// produces. The Rust backend guards the second as well, and a guard no test
/// reaches is a guard nobody can trust.
#[test]
fn a_nameless_tombstone_states_its_ordinal_alone() {
    let package = interact_package(
        vec![interface(
            "WheelHistory",
            vec![
                interaction(
                    "wheelTicks",
                    1,
                    signal(
                        "FanLevel",
                        Some("0"),
                        timing(
                            v2::TimingMode::Range,
                            Some("100000"),
                            Some("1000000"),
                            false,
                        ),
                    ),
                ),
                reserved_ordinal(2, None),
                reserved_ordinal(3, Some("")),
                interaction("vin", 4, fixed_def(named("Vin"))),
            ],
        )],
        Vec::new(),
    );
    let source = render(&package);

    for ordinal in [2, 3] {
        assert!(
            source.contains(&format!(
                "@reserved ordinal {ordinal} — retired, never reused."
            )),
            "a nameless tombstone states its ordinal, got:\n{source}"
        );
    }
    assert!(
        !source.contains("``"),
        "a nameless tombstone has no name to quote, got:\n{source}"
    );
}

/// Every interaction states its ordinal — the wire identity (ridl §11) a
/// tag-based transport derives its numeric ids from and `ridl diff` keys on.
///
/// The fixture is deliberately wide enough that position cannot stand in for
/// the ordinal in either direction. A `fixed` at ordinal 1 is emitted on the
/// consumer face only, so the provider face starts at ordinal 3; a tombstone
/// at ordinal 2 displaces everything after it on both faces. An emitter that
/// numbered members by position, or that dropped the tag from any one of the
/// five kinds, disagrees with the expected pairs below.
#[test]
fn every_interaction_states_its_ordinal_on_every_face_that_carries_it() {
    let package = interact_package(
        vec![interface(
            "VehicleStatus",
            vec![
                interaction("vin", 1, fixed_def(named("Vin"))),
                reserved(2, "legacyPing"),
                interaction(
                    "speed",
                    3,
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
                    "doorOpened",
                    4,
                    event(
                        "DoorPayload",
                        timing(v2::TimingMode::Range, Some("50000"), Some("500000"), false),
                    ),
                ),
                interaction(
                    "setGear",
                    5,
                    command(vec![param("position", named("GearPosition"))], Vec::new()),
                ),
                interaction(
                    "getAverageSpeed",
                    6,
                    query(Vec::new(), fallible("Speed", "SpeedError"), Vec::new()),
                ),
            ],
        )],
        Vec::new(),
    );
    let source = render(&package);

    assert_eq!(
        face_ordinals(&source, "VehicleStatusConsumer"),
        vec![
            (1, "vin".to_string()),
            (3, "speed".to_string()),
            (4, "doorOpened".to_string()),
            (5, "setGear".to_string()),
            (6, "getAverageSpeed".to_string()),
        ],
        "got:\n{source}"
    );
    assert_eq!(
        face_ordinals(&source, "VehicleStatusProvider"),
        vec![
            (3, "speed".to_string()),
            (4, "doorOpened".to_string()),
            (5, "setGear".to_string()),
            (6, "getAverageSpeed".to_string()),
        ],
        "got:\n{source}"
    );
}

/// An empty canonical init text emits no `@init` tag.
///
/// Two unrelated values lower to an empty text — the empty string of a
/// string-backed payload and the empty set of an enum-set payload (typl §5.8)
/// — and this emitter sees the text, not the payload's declaration, so it
/// cannot render either without guessing which it holds. `@init` with nothing
/// after it stated neither, and left trailing whitespace in generated source;
/// the tag is now absent, the way an absent value is already treated. The Rust
/// backend collapses the same case onto one sentence. `veh-cluster`'s
/// `warnings: WarningFlags` is the live instance in the corpus.
#[test]
fn an_empty_init_value_emits_no_init_tag() {
    let package = one(interaction(
        "speed",
        1,
        signal(
            "Speed",
            Some(""),
            timing(
                v2::TimingMode::StrictPeriodic,
                Some("10000"),
                Some("10000"),
                false,
            ),
        ),
    ));
    let source = render(&package);

    assert!(
        !source.contains("@init"),
        "an empty init value states nothing, so it emits no tag, got:\n{source}"
    );
    // The member itself is unaffected — the tag is what is absent, not the
    // signal.
    assert!(
        source.contains("speed: SignalHandle<Speed>;"),
        "got:\n{source}"
    );
    assert!(source.contains("@ordinal 1"), "got:\n{source}");
}

// ---------------------------------------------------------------------------
// The consumer/provider split of `fixed`.
// ---------------------------------------------------------------------------

/// A `fixed` appears on the consumer face only.
///
/// The ridl §3 interaction-model table gives every kind an initiator and
/// gives `fixed` "neither (provisioned)" — the one kind naming no side. §8
/// has it provisioned externally (build, factory, FOTA) and read through a
/// plain accessor "free of the query machinery", and §14.6 defines providing
/// as producing signals/events and accepting commands/queries, four kinds
/// with `fixed` absent. A provider can neither publish, answer, nor write
/// one, so `readonly vin: Vin` on the provider face would assert an
/// obligation the language places on the provisioning plane.
#[test]
fn fixed_appears_on_the_consumer_face_only() {
    let package = interact_package(
        vec![interface(
            "VehicleStatus",
            vec![
                interaction("vin", 1, fixed_def(named("Vin"))),
                interaction(
                    "speed",
                    2,
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
            ],
        )],
        Vec::new(),
    );
    let source = render(&package);

    let consumer = face_body(&source, "VehicleStatusConsumer");
    let provider = face_body(&source, "VehicleStatusProvider");

    assert!(
        consumer.contains("readonly vin: Vin;"),
        "the consumer face must carry the fixed, got:\n{consumer}"
    );
    assert!(
        !provider.contains("vin"),
        "the provider face must not mention the fixed, got:\n{provider}"
    );
    // The signal still splits across both faces, so the provider face is not
    // simply empty.
    assert!(provider.contains("speed: { publish(value: Speed): void };"));
}

/// An interface holding nothing but fixed values still emits a provider face — an
/// empty one, which is the honest shape: there is nothing for a provider to
/// do.
#[test]
fn fixed_only_interface_emits_an_empty_provider_face() {
    let package = one(interaction("vin", 1, fixed_def(named("Vin"))));
    let source = render(&package);
    assert!(
        source.contains("export interface VehicleStatusProvider {}"),
        "got:\n{source}"
    );
}

/// The body of a generated face, for assertions that must not be satisfied by
/// a match in the sibling face.
fn face_body<'a>(source: &'a str, face: &str) -> &'a str {
    let start = source
        .find(&format!("export interface {face} {{"))
        .unwrap_or_else(|| panic!("{face} must be generated, got:\n{source}"));
    let rest = &source[start..];
    let end = rest.find("\n}").map(|i| i + 2).unwrap_or(rest.len());
    &rest[..end]
}

/// The `(ordinal, member name)` pairs of a face, in emission order: every
/// member paired with the ordinal its own JSDoc block states.
///
/// A member with no `@ordinal` tag panics rather than being skipped — an
/// interaction whose wire identity went missing is the failure this helper
/// exists to surface, not a row to leave out of the comparison.
fn face_ordinals(source: &str, face: &str) -> Vec<(u32, String)> {
    let body = face_body(source, face);
    let mut pairs = Vec::new();
    let mut pending: Option<u32> = None;
    for line in body.lines() {
        let trimmed = line.trim_start();
        if let Some(value) = trimmed.strip_prefix("* @ordinal ") {
            pending = Some(value.parse().expect("an @ordinal tag carries a number"));
            continue;
        }
        // A member declaration is the only two-space-indented line outside a
        // JSDoc block; every line of a JSDoc block starts with `/*` or `*`.
        if trimmed.starts_with("/*") || trimmed.starts_with('*') {
            continue;
        }
        let Some(rest) = line.strip_prefix("  ") else {
            continue;
        };
        let declaration = rest.strip_prefix("readonly ").unwrap_or(rest);
        let name: String = declaration
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
            .collect();
        let ordinal = pending
            .take()
            .unwrap_or_else(|| panic!("member {name} of {face} states no ordinal, got:\n{body}"));
        pairs.push((ordinal, name));
    }
    pairs
}

// ---------------------------------------------------------------------------
// Generated-name collisions — one case per name family.
// ---------------------------------------------------------------------------

/// The package behind a collision case: one interface, one inline service,
/// and a single typl declaration under test.
fn collision_package(decl_name: &str) -> v2::Package {
    let mut package = interact_package(
        vec![interface(
            "VehicleStatus",
            vec![interaction("vin", 1, fixed_def(named("Vin")))],
        )],
        vec![service(
            "veh.adas.logs",
            v2::service_shape::Kind::Inline(interface(
                "",
                vec![interaction("tailLogs", 1, fixed_def(named("Vin")))],
            )),
        )],
    );
    package.decls.push(struct_decl(
        decl_name,
        vec![scalar_field("count", 1, v2::PrimitiveType::Integer)],
    ));
    package
}

fn collision_message(decl_name: &str) -> String {
    match generate(&collision_package(decl_name)) {
        Err(GenerateError::Unrepresentable(message)) => message,
        other => panic!("{decl_name} must collide, got: {other:?}"),
    }
}

/// A declaration named after a generated face is refused (the pre-existing
/// family, kept covered).
#[test]
fn declaration_colliding_with_a_face_is_refused() {
    assert!(collision_message("VehicleStatusConsumer").contains("consumer face"));
    assert!(collision_message("VehicleStatusProvider").contains("provider face"));
}

/// A declaration named after a generated const is refused.
///
/// Nothing upstream prevents this: typl §15.1 is a conventions table with no
/// enforcing diagnostic, so `struct vehicleStatusTiming` draws no ridl
/// diagnostic at all. Reaching `tsc`, the enum form collides outright and the
/// const form is TS2451 (cannot redeclare block-scoped variable).
#[test]
fn declaration_colliding_with_a_generated_const_is_refused() {
    assert!(collision_message("vehicleStatusTiming").contains("timing table"));
    assert!(collision_message("vehicleStatusContracts").contains("contract table"));
}

/// A declaration named after an inline service shape's face is refused.
///
/// The service name is constrained (`check_service_name` in `ridl-sem`), but
/// the *typl* name is not, so `struct Service_veh_adas_logsConsumer` compiles
/// clean and then reaches `tsc` as TS2741: the module emits
/// `export interface Service_veh_adas_logsConsumer` twice and TypeScript
/// merges them.
#[test]
fn declaration_colliding_with_an_inline_shape_face_is_refused() {
    let message = collision_message("Service_veh_adas_logsConsumer");
    assert!(
        message.contains("inline shape of service veh.adas.logs"),
        "the message must name the service, got: {message}"
    );
}

/// A declaration named after an inline shape's generated const is refused
/// too — the fourth family, reached through the mangled stem.
#[test]
fn declaration_colliding_with_an_inline_shape_const_is_refused() {
    assert!(collision_message("service_veh_adas_logsTiming").contains("timing table"));
}

/// The generated service map is a module-level name like any other.
#[test]
fn declaration_colliding_with_the_service_map_is_refused() {
    assert!(collision_message("services").contains("service map"));
}

/// Two generated names claiming one identifier is refused before anything is
/// emitted: an interface can be named so that its faces coincide with an
/// inline service shape's.
#[test]
fn two_generated_names_claiming_one_identifier_are_refused() {
    let package = interact_package(
        vec![interface(
            "Service_veh_adas_logs",
            vec![interaction("vin", 1, fixed_def(named("Vin")))],
        )],
        vec![service(
            "veh.adas.logs",
            v2::service_shape::Kind::Inline(interface(
                "",
                vec![interaction("tailLogs", 1, fixed_def(named("Vin")))],
            )),
        )],
    );
    assert!(matches!(
        generate(&package),
        Err(GenerateError::Unrepresentable(message))
            if message.contains("claimed by both")
    ));
}

/// A name that merely resembles a generated one still generates — the check
/// rejects collisions, not a naming style.
#[test]
fn a_near_miss_name_still_generates() {
    assert!(generate(&collision_package("VehicleStatusConsumerExtra")).is_ok());
    assert!(generate(&collision_package("VehicleStatusTiming")).is_ok());
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

/// A shape slot with no kind carries nothing to generate (ridl §14.5).
#[test]
fn service_shape_slot_without_a_kind_is_an_error() {
    let package = v2::Package {
        services: vec![v2::Service {
            shapes: vec![v2::ServiceShape { id: 1, kind: None }],
            ..service(
                "veh.cluster.status",
                v2::service_shape::Kind::InterfaceRef("VehicleStatus".to_string()),
            )
        }],
        ..appendix_a()
    };
    assert!(matches!(
        generate(&package),
        Err(GenerateError::Unrepresentable(message)) if message.contains("no kind")
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

// ---------------------------------------------------------------------------
// `internal` visibility on the interaction layer (ADR-0008 decision 7).
// ---------------------------------------------------------------------------

/// An inline service shape as the CHECKER emits it: an anonymous interface
/// (`name == ""`) whose visibility is `VISIBILITY_UNSPECIFIED`, not public.
///
/// This helper exists because [`interface`] hardcodes `Visibility::Public`,
/// which no inline shape ever carries: `Checker::lower_service_inline` leaves
/// the field unset, because an anonymous shape is not addressable as a type
/// and has no visibility of its own — the enclosing `Service` carries the
/// authoritative one, and a service takes no `internal` modifier (ridl §14.5).
/// A fixture that says `Public` here would be testing a value the real
/// producer never emits, and the UNSPECIFIED default arm of [`export_kw`]
/// would go unexercised.
fn inline_shape(interactions: Vec<v2::Decl>) -> v2::Interface {
    v2::Interface {
        visibility: v2::Visibility::Unspecified as i32,
        ..interface("", interactions)
    }
}

/// The package both visibility tests read. Three shapes side by side, so the
/// rule is exercised per declaration rather than per module:
///
/// 1. `internal interface Hidden` — every generated name module-local;
/// 2. public `interface Shown` — every generated name exported, the
///    regression direction;
/// 3. an **inline service shape** whose `Interface.visibility` is
///    `VISIBILITY_UNSPECIFIED` ([`inline_shape`]) — every generated name
///    exported, which is what makes the "no special case for inline shapes"
///    reasoning true rather than merely asserted.
///
/// Each of the three carries a timed signal and a contract-bearing query, so
/// all four generated names of all three are present and non-empty. An empty
/// constant is exactly how the contracts const was overlooked in the first
/// place.
///
/// `Hidden`'s query returns a tuple, which this backend renders as an inline
/// object type (typl §11): it introduces no name of its own and so has nothing
/// to hide. That is the one place the two backends' name sets legitimately
/// differ — the Rust backend generates a named struct there, TypeScript does
/// not.
fn visibility_package() -> v2::Package {
    let hidden = v2::Interface {
        visibility: v2::Visibility::Internal as i32,
        ..interface(
            "Hidden",
            vec![
                interaction(
                    "rawTicks",
                    1,
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
                interaction(
                    "getBounds",
                    2,
                    query(
                        Vec::new(),
                        v2::return_type::Kind::Value(v2::FieldType {
                            optional: false,
                            kind: Some(v2::field_type::Kind::Tuple(v2::TupleType {
                                fields: vec![
                                    v2::TupleField {
                                        name: "min".to_string(),
                                        r#type: Some(named("veh.common.Speed")),
                                    },
                                    v2::TupleField {
                                        name: "max".to_string(),
                                        r#type: Some(named("veh.common.Speed")),
                                    },
                                ],
                            })),
                        }),
                        vec![contract(
                            v2::ContractKind::Ensure,
                            "result.min <= result.max",
                            &[],
                            &[],
                            true,
                            "Hidden.getBounds.ensure[0]",
                        )],
                    ),
                ),
            ],
        )
    };
    // A signal and a contract-bearing query, so both metadata constants of the
    // interface built from them are non-empty.
    let pair = |ensure_id: &str| {
        vec![
            interaction(
                "cabinTemp",
                1,
                signal(
                    "veh.common.Speed",
                    Some("0.0"),
                    timing(v2::TimingMode::Range, Some("50000"), Some("500000"), false),
                ),
            ),
            interaction(
                "readSummary",
                2,
                query(
                    Vec::new(),
                    v2::return_type::Kind::Value(named("veh.common.Speed")),
                    vec![contract(
                        v2::ContractKind::Ensure,
                        "result >= 0.0",
                        &[],
                        &[],
                        true,
                        ensure_id,
                    )],
                ),
            ),
        ]
    };
    let shown = interface("Shown", pair("Shown.readSummary.ensure[0]"));
    interact_package(
        vec![hidden, shown],
        vec![
            service(
                "veh.cluster.hidden",
                v2::service_shape::Kind::InterfaceRef("Hidden".to_string()),
            ),
            service(
                "veh.cluster.diag",
                v2::service_shape::Kind::Inline(inline_shape(pair(
                    "veh.cluster.diag.readSummary.ensure[0]",
                ))),
            ),
        ],
    )
}

/// An **`internal` service's inline shape generates module-local items** — the
/// authoritative visibility is the enclosing `Service`'s, never the shape's own
/// `VISIBILITY_UNSPECIFIED` field (ridl §14.5,
/// `crates/ridlc/tests/corpus/veh-cluster/NOTES`).
///
/// This is the one assertion that separates `InterfaceShape::visibility()` from
/// the field it replaced. Every other visibility case agrees by accident:
/// [`export_kw`](crate::export_kw) maps both `UNSPECIFIED` and `PUBLIC` to
/// `export `, so reading the inline shape's own field and reading the service's
/// produce identical output for a public service — which is every service the
/// checker can currently emit, since `internal service` is FORM-102 from
/// source.
///
/// Unreachable through the checker is not unreachable in principle, and the gap
/// is not hypothetical: the IR admits an internal service, `ridl diff` already
/// reasons about one (`classify_tests::service_inline` builds exactly this
/// package, and `find_visibility` classifies internal-to-public on it as a
/// widening), and any producer that is not the checker — a hand-written
/// `.ir.json`, a registry, the rsdl lowering E3 adds — reaches it immediately.
/// Under the old read the whole generated API of such a service is published.
///
/// It also guards a tidy-up. `export_kw`'s `_ => "export "` catch-all silently
/// equates the proto default with `PUBLIC`; making that match exhaustive, which
/// is the discipline ADR-0008 decision 21 argues for elsewhere, would break
/// every inline shape under the old read. This test is what turns that from a
/// silent breakage into a red one.
#[test]
fn an_internal_services_inline_shape_is_module_local() {
    let mut package = visibility_package();
    let diag = package
        .services
        .iter_mut()
        .find(|service| service.name == "veh.cluster.diag")
        .expect("the shared fixture declares the inline-shape service");
    assert!(
        matches!(
            diag.shapes.first().and_then(|slot| slot.kind.as_ref()),
            Some(v2::service_shape::Kind::Inline(_))
        ),
        "this test is only meaningful on an INLINE shape",
    );
    diag.visibility = v2::Visibility::Internal as i32;

    let source = render(&package);

    // Each is matched with the leading newline that begins its line, so
    // `interface Service_…Consumer` cannot be satisfied by the `export
    // interface Service_…Consumer` this test exists to forbid.
    for item in [
        "\ninterface Service_veh_cluster_diagConsumer {",
        "\ninterface Service_veh_cluster_diagProvider {",
        "\nconst service_veh_cluster_diagTiming = {",
        "\nconst service_veh_cluster_diagContracts = [",
    ] {
        assert!(
            source.contains(item),
            "an internal service's inline shape must emit `{}` unexported, got:\n{source}",
            item.trim_start()
        );
    }
    for leaked in [
        "export interface Service_veh_cluster_diagConsumer",
        "export interface Service_veh_cluster_diagProvider",
        "export const service_veh_cluster_diagTiming",
        "export const service_veh_cluster_diagContracts",
    ] {
        assert!(
            !source.contains(leaked),
            "an internal service's inline shape must not emit `{leaked}`, got:\n{source}"
        );
    }

    // The regression direction, in the same module: the public interface and
    // the package-level service map are untouched.
    for item in ["export interface ShownConsumer", "export const services"] {
        assert!(
            source.contains(item),
            "`{item}` is unaffected by a service's visibility, got:\n{source}"
        );
    }
}

/// Every name an `internal interface` generates is module-local; every name a
/// public interface generates stays exported; every name an **inline service
/// shape** generates stays exported; and the package-level names — the
/// interaction vocabulary and the service map — stay exported regardless.
///
/// The three shapes sit in one package on purpose: `internal` is a property of
/// the declaration, so a module holding all three must generate all three
/// spellings.
#[test]
fn internal_interface_generates_module_local_shapes() {
    let source = render(&visibility_package());

    // The four names `Hidden` generates, all module-local. Each is matched
    // with the leading newline that begins its line, so `interface
    // HiddenConsumer` cannot be satisfied by the `export interface
    // HiddenConsumer` this test exists to forbid.
    for item in [
        "\ninterface HiddenConsumer {",
        "\ninterface HiddenProvider {",
        "\nconst hiddenTiming = {",
        "\nconst hiddenContracts = [",
    ] {
        assert!(
            source.contains(item),
            "an internal interface must emit `{}` unexported, got:\n{source}",
            item.trim_start()
        );
    }
    for leaked in [
        "export interface HiddenConsumer",
        "export interface HiddenProvider",
        "export const hiddenTiming",
        "export const hiddenContracts",
    ] {
        assert!(
            !source.contains(leaked),
            "an internal interface must not emit `{leaked}`, got:\n{source}"
        );
    }

    // The regression direction: a public interface in the same module is
    // untouched.
    for item in [
        "export interface ShownConsumer",
        "export interface ShownProvider",
        "export const shownTiming",
        "export const shownContracts",
    ] {
        assert!(
            source.contains(item),
            "a public interface must still emit `{item}`, got:\n{source}"
        );
    }

    // An inline service shape carries `VISIBILITY_UNSPECIFIED`, which is not a
    // missing value to be guessed at: an anonymous shape has no visibility of
    // its own and the enclosing service — which cannot be `internal` at all
    // (ridl §14.5) — carries the authoritative one. So all four of its names
    // are exported. This is asserted rather than left implicit because it is
    // the link that makes "no special case for inline shapes" true.
    for item in [
        "export interface Service_veh_cluster_diagConsumer",
        "export interface Service_veh_cluster_diagProvider",
        "export const service_veh_cluster_diagTiming",
        "export const service_veh_cluster_diagContracts",
    ] {
        assert!(
            source.contains(item),
            "an inline service shape is public, so it must emit `{item}`, got:\n{source}"
        );
    }

    // The names that are deliberately NOT affected: the vocabulary is emitted
    // once per module and shared by every interface in it, and the service map
    // is the package's published deployment surface — a service takes no
    // `internal` modifier (ridl §14.5).
    for item in [
        "export type Provenance",
        "export interface SignalHandle<T>",
        "export interface EventHandle<T>",
        "export type Result<T, E>",
        "export const services",
    ] {
        assert!(
            source.contains(item),
            "`{item}` is package-level and stays exported, got:\n{source}"
        );
    }

    insta::assert_snapshot!(source);
}

/// Not exporting is not cosmetic: the generated module compiles strict, and
/// the shapes an `internal interface` produces are genuinely unimportable
/// while the public interface's are importable.
///
/// A snapshot alone cannot show this — it records the spelling, not what the
/// spelling means to `tsc`. Both directions are asserted, because a test that
/// only checked the failure would also pass if the whole module failed to
/// compile. This is best-effort local evidence in the established shape:
/// [`internal_interface_generates_module_local_shapes`] is the gate, and this
/// check is skipped with a printed notice when no tsc binary is discoverable.
#[test]
fn internal_interface_shapes_are_not_importable() {
    let Some(tsc) = crate::tests::discover_tsc() else {
        println!(
            "SKIPPED: no tsc binary discoverable (`tsc` on PATH or `npx --no-install tsc`); \
             internal_interface_generates_module_local_shapes remains the gate"
        );
        return;
    };

    const VEH_COMMON: &str = "\
export type Speed = number & { readonly __ridl: 'veh.common.Speed' };
";
    let source = render(&visibility_package());

    let dir = tempfile::tempdir().expect("a temp dir is created");
    std::fs::write(dir.path().join("veh.common.ts"), VEH_COMMON).expect("the prelude is written");
    std::fs::write(dir.path().join("veh.cluster.ts"), &source).expect("the module is written");

    // Type-checks a one-line consumer module against the generated one, and
    // returns tsc's own output together with whether it accepted the program.
    let probe = |name: &str, body: &str| -> (bool, String) {
        let path = dir.path().join(format!("{name}.ts"));
        std::fs::write(&path, body).expect("the probe is written");
        let output = std::process::Command::new(&tsc.0)
            .args(&tsc.1)
            .args([
                "--noEmit", "--strict", "--target", "es2020", "--module", "commonjs",
            ])
            .arg(&path)
            .output()
            .expect("the discovered tsc must be runnable");
        (
            output.status.success(),
            String::from_utf8_lossy(&output.stdout).into_owned(),
        )
    };

    // A face is an interface, so it is imported into a type alias; a const is
    // imported into a value binding. Each name is used the only way its
    // namespace admits.
    let names_type = |item: &str| {
        format!("import {{ {item} }} from './veh.cluster';\nexport type A = {item};\n")
    };
    let names_value = |item: &str| {
        format!("import {{ {item} }} from './veh.cluster';\nexport const v = {item};\n")
    };

    // Importable: the public interface, and the inline service shape — whose
    // `Interface.visibility` is UNSPECIFIED, the value the checker actually
    // emits for an anonymous shape.
    for (probe_name, what, body) in [
        (
            "reaches_shown_consumer",
            "a public interface's consumer face",
            names_type("ShownConsumer"),
        ),
        (
            "reaches_shown_provider",
            "a public interface's provider face",
            names_type("ShownProvider"),
        ),
        (
            "reaches_shown_timing",
            "a public interface's timing const",
            names_value("shownTiming"),
        ),
        (
            "reaches_shown_contracts",
            "a public interface's contract const",
            names_value("shownContracts"),
        ),
        (
            "reaches_inline_consumer",
            "an inline service shape's consumer face",
            names_type("Service_veh_cluster_diagConsumer"),
        ),
        (
            "reaches_inline_provider",
            "an inline service shape's provider face",
            names_type("Service_veh_cluster_diagProvider"),
        ),
        (
            "reaches_inline_timing",
            "an inline service shape's timing const",
            names_value("service_veh_cluster_diagTiming"),
        ),
        (
            "reaches_inline_contracts",
            "an inline service shape's contract const",
            names_value("service_veh_cluster_diagContracts"),
        ),
    ] {
        let (ok, out) = probe(probe_name, &body);
        assert!(ok, "{what} must stay importable, tsc said:\n{out}");
    }

    // Unimportable: every one of the four names the internal interface
    // generates, each refused specifically as declared-locally-but-not-exported
    // rather than as any error at all.
    for (probe_name, what, item, body) in [
        (
            "hides_consumer",
            "consumer face",
            "HiddenConsumer",
            names_type("HiddenConsumer"),
        ),
        (
            "hides_provider",
            "provider face",
            "HiddenProvider",
            names_type("HiddenProvider"),
        ),
        (
            "hides_timing",
            "timing const",
            "hiddenTiming",
            names_value("hiddenTiming"),
        ),
        (
            "hides_contracts",
            "contract const",
            "hiddenContracts",
            names_value("hiddenContracts"),
        ),
    ] {
        let (ok, out) = probe(probe_name, &body);
        assert!(
            !ok,
            "an internal interface's {what} must not be importable, source:\n{source}"
        );
        assert!(
            out.contains(item) && out.contains("not exported"),
            "the refusal for the {what} must say the name is declared locally but not \
             exported, tsc said:\n{out}"
        );
    }
}
