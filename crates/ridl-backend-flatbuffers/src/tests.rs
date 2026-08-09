use crate::generate;
use ridl_ir::v2;

/// Compiles `source` as a FlatBuffers schema with planus, panicking with the
/// compiler's own message on failure. This is the story's acceptance check:
/// every test that emits a schema runs it through here.
pub(crate) fn compile_with_planus(file_name: &str, source: &str) {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join(file_name);
    std::fs::write(&path, source).expect("write schema");
    if planus_translation::translate_files(&[path]).is_none() {
        panic!("emitted schema is not a valid FlatBuffers schema:\n\n{source}");
    }
}

fn package(name: &str) -> v2::Package {
    v2::Package {
        name: name.to_string(),
        ..Default::default()
    }
}

#[test]
fn an_empty_package_emits_a_valid_namespace_header() {
    let generated = generate(&package("veh.common")).expect("generate");
    assert_eq!(generated.fbs_source, "namespace veh.common;\n");
    compile_with_planus("veh.common.fbs", &generated.fbs_source);
}

#[test]
fn an_interface_emits_its_ordinal_table() {
    let package = v2::Package {
        name: "veh.cluster".to_string(),
        interfaces: vec![v2::Interface {
            name: "VehicleStatus".to_string(),
            interactions: vec![
                signal_decl("currentSpeed", 1),
                signal_decl("doorOpened", 2),
                signal_decl("tyrePressure", 4),
            ],
            ..Default::default()
        }],
        ..Default::default()
    };

    let generated = generate(&package).expect("generate");

    // FlatBuffers scopes enum values to their enum, so values are NOT
    // prefixed with the enum name — unlike the proto3 backend.
    assert!(
        generated.fbs_source.contains(
            "enum VehicleStatusOrdinal : uint {\n  \
             CURRENT_SPEED = 1,\n  \
             DOOR_OPENED = 2,\n  \
             TYRE_PRESSURE = 4,\n}"
        ),
        "got:\n{}",
        generated.fbs_source
    );

    compile_with_planus("veh.cluster.fbs", &generated.fbs_source);
}

#[test]
fn two_enums_may_share_a_value_name() {
    // The scoping difference from proto3, pinned so nobody reintroduces
    // prefixing by copying the proto backend.
    let package = v2::Package {
        name: "veh.cluster".to_string(),
        interfaces: vec![
            v2::Interface {
                name: "Alpha".to_string(),
                interactions: vec![signal_decl("ok", 1)],
                ..Default::default()
            },
            v2::Interface {
                name: "Beta".to_string(),
                interactions: vec![signal_decl("ok", 1)],
                ..Default::default()
            },
        ],
        ..Default::default()
    };

    let generated = generate(&package).expect("generate");
    assert!(generated.fbs_source.contains("enum AlphaOrdinal : uint {"));
    assert!(generated.fbs_source.contains("enum BetaOrdinal : uint {"));
    compile_with_planus("veh.cluster.fbs", &generated.fbs_source);
}

#[test]
fn an_inline_service_shape_is_named_from_the_service_address() {
    let package = v2::Package {
        name: "corpus.baseline".to_string(),
        services: vec![v2::Service {
            name: "corpus.baseline.hvac".to_string(),
            shapes: vec![v2::ServiceShape {
                id: 1,
                kind: Some(v2::service_shape::Kind::Inline(v2::Interface {
                    name: String::new(),
                    interactions: vec![signal_decl("cabinTemp", 1)],
                    ..Default::default()
                })),
            }],
            ..Default::default()
        }],
        ..Default::default()
    };

    let generated = generate(&package).expect("generate");
    assert!(
        generated
            .fbs_source
            .contains("enum CorpusBaselineHvacOrdinal : uint {"),
        "got:\n{}",
        generated.fbs_source
    );
    compile_with_planus("corpus.baseline.fbs", &generated.fbs_source);
}

/// A signal interaction at `ordinal`. The kind is immaterial to tier 2: the
/// table is interface-wide and kind-blind (ridl §11, ADR-0013 decision 3).
fn signal_decl(name: &str, ordinal: u32) -> v2::Decl {
    v2::Decl {
        name: name.to_string(),
        ordinal,
        kind: Some(v2::decl::Kind::SignalDef(v2::SignalDef::default())),
        ..Default::default()
    }
}
