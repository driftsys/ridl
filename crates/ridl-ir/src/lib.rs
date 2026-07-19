//! RIDL intermediate representation.
//!
//! The v2 schema (`proto/ridl/ir/v2/ir.proto`) is compiled from its protobuf
//! source by `build.rs` (protox + prost-build, ADR-0006 decision 3) and
//! exposed as [`v2`]. v2 is the typl surface plus the ridl interaction layer
//! (ridl language reference §3–§14) with exact decimal values — every numeric
//! value is a canonical decimal string, never a floating-point field (ADR-0007
//! decision 9, ADR-0008 decision 12). The v1 schema was removed when its last
//! consumer moved to v2 (task 6 of the E2 plan), mirroring the E1 v0→v1
//! retirement.

pub mod v2 {
    //! IR v2 — the typl surface plus the ridl interaction layer (ridl
    //! language reference §3–§14) with exact decimal values (ADR-0007
    //! decision 9, ADR-0008 decision 12).

    include!(concat!(env!("OUT_DIR"), "/ridl.ir.v2.rs"));

    /// Derives the synthesized transport identity of an inline `T | E`
    /// result union (ADR-0008 decision 4): the enclosing interface name plus
    /// the interaction ordinal plus the ordered arm references. The single
    /// derivation every consumer — backends and the diff classifier — calls,
    /// so the identity stays stable under compatible evolution.
    pub fn fallible_transport_identity(
        interface: &str,
        ordinal: u32,
        fallible: &FallibleType,
    ) -> String {
        format!(
            "{interface}#{ordinal}:{ok}|{err}",
            ok = fallible.ok,
            err = fallible.err
        )
    }

    /// Renders a package as pretty-printed JSON — the debug rendering of the
    /// IR (ADR-0004 §4), used by golden tests and diagnostic output.
    pub fn to_json_pretty(package: &Package) -> String {
        serde_json::to_string_pretty(package)
            .expect("IR serialization to JSON cannot fail: the generated types hold only JSON-representable values")
    }
}

#[cfg(test)]
mod v2_round_trip {
    use crate::v2;
    use prost::Message;

    /// Wraps an interaction kind in the shared `Decl` envelope. Visibility
    /// and `is_error` stay unset on interactions (ridl §14.1); the ordinal is
    /// the 1-based declaration order across all interactions of the
    /// enclosing interface (ridl §11).
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

    fn named_type(name: &str) -> v2::FieldType {
        v2::FieldType {
            optional: false,
            kind: Some(v2::field_type::Kind::Named(name.to_string())),
        }
    }

    fn stream_of(element: v2::stream_type::Element) -> v2::FieldType {
        v2::FieldType {
            optional: false,
            kind: Some(v2::field_type::Kind::Stream(v2::StreamType {
                element: Some(element),
            })),
        }
    }

    /// A representative ridl package: one interface holding all five
    /// interaction kinds plus a reserved tombstone (ordinals 1–6, the
    /// tombstone counted, ridl §11), a strict-periodic and a defaulted
    /// range timing, a fallible query, and two services — a named
    /// reference and an inline shape holding a stream query.
    fn fixture() -> v2::Package {
        // signal speed : Speed @10ms — strict periodic stores the period
        // in both bounds (ADR-0008 decision 12).
        let speed = v2::SignalDef {
            payload: "Speed".to_string(),
            declared_init: None,
            init: Some(v2::InitValue {
                derivable: true,
                value: Some("0.0".to_string()),
            }),
            timing: Some(v2::Timing {
                mode: v2::TimingMode::StrictPeriodic as i32,
                min_us: Some("10000".to_string()),
                max_us: Some("10000".to_string()),
                default_applied: false,
            }),
        };

        // event doorOpened : DoorEvent — untimed in source, so the
        // configured default range is resolved at compile time (ridl §9.1).
        let door_opened = v2::EventDef {
            payload: "DoorEvent".to_string(),
            timing: Some(v2::Timing {
                mode: v2::TimingMode::Range as i32,
                min_us: Some("20000".to_string()),
                max_us: Some("500000".to_string()),
                default_applied: true,
            }),
        };

        // command setTarget(target : Speed) [ require target >= speed ]
        let set_target = v2::CommandDef {
            params: vec![v2::Param {
                name: "target".to_string(),
                r#type: Some(named_type("Speed")),
            }],
            contracts: vec![v2::Contract {
                kind: v2::ContractKind::Require as i32,
                source: "target >= speed".to_string(),
                signal_refs: vec!["speed".to_string()],
                param_refs: vec!["target".to_string()],
                uses_result: false,
                observer_id: "VehicleStatus.setTarget.require[0]".to_string(),
            }],
        };

        // query fetchFaults(page : PageSpec) : FaultPage | DiagError
        //   [ ensure result.count <= page.limit ]
        let fetch_faults = v2::QueryDef {
            params: vec![v2::Param {
                name: "page".to_string(),
                r#type: Some(named_type("PageSpec")),
            }],
            return_type: Some(v2::ReturnType {
                kind: Some(v2::return_type::Kind::Fallible(v2::FallibleType {
                    ok: "FaultPage".to_string(),
                    err: "DiagError".to_string(),
                })),
            }),
            contracts: vec![v2::Contract {
                kind: v2::ContractKind::Ensure as i32,
                source: "result.count <= page.limit".to_string(),
                signal_refs: Vec::new(),
                param_refs: vec!["page".to_string()],
                uses_result: true,
                observer_id: "VehicleStatus.fetchFaults.ensure[0]".to_string(),
            }],
        };

        // final vin : Vin
        let vin = v2::FinalDef {
            payload: Some(named_type("Vin")),
        };

        let vehicle_status = v2::Interface {
            name: "VehicleStatus".to_string(),
            visibility: v2::Visibility::Public as i32,
            doc: "Vehicle status contract".to_string(),
            labels: Vec::new(),
            deprecated: None,
            interactions: vec![
                interaction("speed", 1, v2::decl::Kind::SignalDef(speed)),
                interaction("doorOpened", 2, v2::decl::Kind::EventDef(door_opened)),
                // reserved legacyMode — the tombstone keeps ordinal 3
                // occupied in the one interaction sequence (ridl §11).
                v2::Decl {
                    ordinal: 3,
                    kind: Some(v2::decl::Kind::ReservedSlot(v2::Reserved {
                        ordinal: 3,
                        name: Some("legacyMode".to_string()),
                        value: None,
                    })),
                    ..interaction("", 3, v2::decl::Kind::ReservedSlot(v2::Reserved::default()))
                },
                interaction("setTarget", 4, v2::decl::Kind::CommandDef(set_target)),
                interaction("fetchFaults", 5, v2::decl::Kind::QueryDef(fetch_faults)),
                interaction("vin", 6, v2::decl::Kind::FinalDef(vin)),
            ],
        };

        // query tailLogs(pattern : <string>) : <LogLine> — a stream param
        // and a stream return (ridl §12), inside the inline service shape.
        let tail_logs = v2::QueryDef {
            params: vec![v2::Param {
                name: "pattern".to_string(),
                r#type: Some(stream_of(v2::stream_type::Element::Primitive(
                    v2::PrimitiveType::String as i32,
                ))),
            }],
            return_type: Some(v2::ReturnType {
                kind: Some(v2::return_type::Kind::Value(stream_of(
                    v2::stream_type::Element::Named("LogLine".to_string()),
                ))),
            }),
            contracts: Vec::new(),
        };

        // service veh.adas.status : VehicleStatus — named reference.
        let status_service = v2::Service {
            name: "veh.adas.status".to_string(),
            visibility: v2::Visibility::Public as i32,
            doc: String::new(),
            labels: Vec::new(),
            deprecated: None,
            shape: Some(v2::service::Shape::InterfaceRef(
                "VehicleStatus".to_string(),
            )),
        };
        // service veh.adas.logs { … } — inline shape, Interface.name == ""
        // (ridl §14.5).
        let logs_service = v2::Service {
            name: "veh.adas.logs".to_string(),
            visibility: v2::Visibility::Public as i32,
            doc: String::new(),
            labels: Vec::new(),
            deprecated: None,
            shape: Some(v2::service::Shape::Inline(v2::Interface {
                name: String::new(),
                visibility: v2::Visibility::Unspecified as i32,
                doc: String::new(),
                labels: Vec::new(),
                deprecated: None,
                interactions: vec![interaction(
                    "tailLogs",
                    1,
                    v2::decl::Kind::QueryDef(tail_logs),
                )],
            })),
        };

        v2::Package {
            name: "veh.adas".to_string(),
            // One typl declaration proves the verbatim v1 surface rides
            // along unchanged in v2; package-level declarations carry
            // ordinal 0.
            decls: vec![v2::Decl {
                name: "Speed".to_string(),
                visibility: v2::Visibility::Public as i32,
                is_error: false,
                doc: String::new(),
                labels: Vec::new(),
                deprecated: None,
                ordinal: 0,
                kind: Some(v2::decl::Kind::TypeDef(v2::TypeDef {
                    backing: Some(v2::Backing {
                        kind: Some(v2::backing::Kind::Unit("km/h".to_string())),
                    }),
                    constraint: None,
                    declared_init: None,
                    init: None,
                    width: Some(v2::type_def::Width::FloatWidth(v2::FloatWidth::F32 as i32)),
                })),
            }],
            interfaces: vec![vehicle_status],
            services: vec![status_service, logs_service],
        }
    }

    #[test]
    fn protobuf_round_trip_preserves_package() {
        let package = fixture();

        let mut buf = Vec::new();
        package.encode(&mut buf).expect("encode must succeed");
        let decoded = v2::Package::decode(buf.as_slice()).expect("decode must succeed");

        assert_eq!(package, decoded);

        let interface = &decoded.interfaces[0];
        let ordinals: Vec<u32> = interface.interactions.iter().map(|d| d.ordinal).collect();
        assert_eq!(
            ordinals,
            [1, 2, 3, 4, 5, 6],
            "one ordinal sequence, tombstone counted (ridl §11)"
        );
        let Some(v2::decl::Kind::ReservedSlot(tombstone)) = &interface.interactions[2].kind else {
            panic!("ordinal 3 must decode as a reserved tombstone");
        };
        assert_eq!(tombstone.name.as_deref(), Some("legacyMode"));
        let Some(v2::service::Shape::Inline(inline)) = &decoded.services[1].shape else {
            panic!("veh.adas.logs must decode as an inline shape");
        };
        assert_eq!(inline.name, "", "an inline shape carries no name");
    }

    #[test]
    fn json_round_trip_preserves_package() {
        let package = fixture();

        let json = v2::to_json_pretty(&package);
        let decoded: v2::Package =
            serde_json::from_str(&json).expect("json deserialization must succeed");

        assert_eq!(package, decoded);
    }

    #[test]
    fn json_renders_timing_bounds_and_fallible_arms_exactly() {
        let json = v2::to_json_pretty(&fixture());

        // Exactness is visible: timing bounds are exact-decimal microsecond
        // strings, never floating-point numbers (ADR-0008 decision 12).
        assert!(
            json.contains(r#""min_us": "10000""#),
            "the timing bound must be a JSON string, got: {json}"
        );
        // Both arms of the inline T | E return are visible by name.
        assert!(
            json.contains(r#""ok": "FaultPage""#),
            "the ok arm must render, got: {json}"
        );
        assert!(
            json.contains(r#""err": "DiagError""#),
            "the err arm must render, got: {json}"
        );
    }

    #[test]
    fn fallible_transport_identity_follows_the_derivation_rule() {
        // The ADR-0008 decision 4 rule: interface + interaction ordinal +
        // both arm references, in that order.
        let fallible = v2::FallibleType {
            ok: "FaultPage".to_string(),
            err: "DiagError".to_string(),
        };
        assert_eq!(
            v2::fallible_transport_identity("VehicleStatus", 9, &fallible),
            "VehicleStatus#9:FaultPage|DiagError"
        );

        // Derived from the fixture: the fallible query sits at ordinal 5.
        let package = fixture();
        let interface = &package.interfaces[0];
        let query_decl = &interface.interactions[4];
        let Some(v2::decl::Kind::QueryDef(query)) = &query_decl.kind else {
            panic!("ordinal 5 must be the fallible query");
        };
        let Some(v2::return_type::Kind::Fallible(arms)) = &query.return_type.as_ref().unwrap().kind
        else {
            panic!("fetchFaults must return a fallible type");
        };
        assert_eq!(
            v2::fallible_transport_identity(&interface.name, query_decl.ordinal, arms),
            "VehicleStatus#5:FaultPage|DiagError"
        );
    }
}
