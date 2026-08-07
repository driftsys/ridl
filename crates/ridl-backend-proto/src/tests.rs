use crate::generate;
use ridl_ir::v2;

/// Compiles `source` as proto3 with protox, panicking with the compiler's own
/// message on failure. This is the story's acceptance check: every test that
/// emits a schema runs it through here.
pub(crate) fn compile_with_protox(file_name: &str, source: &str) {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join(file_name);
    std::fs::write(&path, source).expect("write schema");
    if let Err(error) = protox::compile([file_name], [dir.path()]) {
        panic!("emitted schema is not valid proto3:\n{error}\n\n{source}");
    }
}

fn package(name: &str) -> v2::Package {
    v2::Package {
        name: name.to_string(),
        ..Default::default()
    }
}

#[test]
fn an_empty_package_emits_a_valid_file_header() {
    let generated = generate(&package("veh.common")).expect("generate");
    assert_eq!(
        generated.proto_source,
        "syntax = \"proto3\";\n\npackage veh.common;\n"
    );
    compile_with_protox("veh.common.proto", &generated.proto_source);
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
                reserved_decl(3),
                signal_decl("tyrePressure", 4),
            ],
            ..Default::default()
        }],
        ..Default::default()
    };

    let generated = generate(&package).expect("generate");

    assert!(
        generated.proto_source.contains(
            "enum VehicleStatusOrdinal {\n  \
         VEHICLE_STATUS_ORDINAL_UNSPECIFIED = 0;\n  \
         VEHICLE_STATUS_ORDINAL_CURRENT_SPEED = 1;\n  \
         VEHICLE_STATUS_ORDINAL_DOOR_OPENED = 2;\n  \
         reserved 3;\n  \
         VEHICLE_STATUS_ORDINAL_TYRE_PRESSURE = 4;\n}"
        ),
        "got:\n{}",
        generated.proto_source
    );

    compile_with_protox("veh.cluster.proto", &generated.proto_source);
}

#[test]
fn an_inline_service_shape_is_named_from_the_service_address() {
    // Interface.name is "" for an inline shape (ridl §14.5), so the enum takes
    // the service's dotted address instead.
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
            .proto_source
            .contains("enum CorpusBaselineHvacOrdinal {"),
        "got:\n{}",
        generated.proto_source
    );
    assert!(
        generated
            .proto_source
            .contains("CORPUS_BASELINE_HVAC_ORDINAL_CABIN_TEMP = 1;"),
        "got:\n{}",
        generated.proto_source
    );

    compile_with_protox("corpus.baseline.proto", &generated.proto_source);
}

#[test]
fn an_ordinal_in_the_protobuf_reserved_span_is_refused() {
    // Field numbers 19,000 to 19,999 belong to protobuf itself (note §4.2).
    let package = v2::Package {
        name: "veh.cluster".to_string(),
        interfaces: vec![v2::Interface {
            name: "Wide".to_string(),
            interactions: vec![signal_decl("far", 19_000)],
            ..Default::default()
        }],
        ..Default::default()
    };

    let error = generate(&package).expect_err("must refuse");
    assert!(error.message.contains("19000"), "got: {}", error.message);
    assert!(
        error.message.contains("reserved by protobuf"),
        "got: {}",
        error.message
    );
}

#[test]
fn an_ordinal_above_the_proto_ceiling_is_refused() {
    let package = v2::Package {
        name: "veh.cluster".to_string(),
        interfaces: vec![v2::Interface {
            name: "Wide".to_string(),
            interactions: vec![signal_decl("far", 536_870_912)],
            ..Default::default()
        }],
        ..Default::default()
    };

    let error = generate(&package).expect_err("must refuse");
    assert!(
        error.message.contains("536870911"),
        "got: {}",
        error.message
    );
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

fn reserved_decl(ordinal: u32) -> v2::Decl {
    v2::Decl {
        ordinal,
        kind: Some(v2::decl::Kind::ReservedSlot(v2::Reserved {
            ordinal,
            ..Default::default()
        })),
        ..Default::default()
    }
}
