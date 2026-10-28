#[path = "support/rustc.rs"]
mod rustc;

use ridl_backend_rust::{WireEncoding, generate_pipeline};
use ridl_ir::v2;

fn package(ty: v2::FieldType) -> v2::Package {
    let package = ridlc::compile(
        "calls.ridl",
        r#"
package face.calls
type Level : integer [0..100]
interface Pump {
  command set(params: Level) @[..50ms]
}
interface Healthy {
  command set(level: Level) @[..50ms]
}
"#,
    )
    .package;
    let mut interfaces: Vec<_> = package
        .shapes()
        .map(|shape| shape.interface.clone())
        .collect();
    let Some(v2::decl::Kind::CommandDef(command)) = interfaces[0].interactions[0].kind.as_mut()
    else {
        panic!("a command");
    };
    command.params[0].r#type = Some(ty);
    v2::Package {
        interfaces,
        ..package
    }
}

fn compiles(source: &str) {
    let dir = tempfile::tempdir().expect("create directory");
    let path = dir.path().join("calls.rs");
    std::fs::write(&path, source).expect("write source");
    let runtime = rustc::ridl_rt_rlib_without_std(dir.path());
    let output = std::process::Command::new("rustc")
        .args([
            "--edition=2024",
            "--crate-type=lib",
            "--emit=metadata",
            "-Dwarnings",
        ])
        .arg("--extern")
        .arg(format!("ridl_rt={}", runtime.display()))
        .arg("-o")
        .arg(dir.path().join("calls.rmeta"))
        .arg(path)
        .output()
        .expect("run rustc");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn an_unbounded_induced_request_refuses_only_its_interface() {
    let package = package_with_an_unbounded_pump();
    let source = generate_pipeline(&package, &[WireEncoding::FlatBuffers], false, &[])
        .expect("generate pipeline")
        .rust_source;
    compiles(&source);
    assert!(source.contains("__RIDL_NO_FACE_Pump"), "{source}");
    assert!(source.contains("pub mod healthy"), "{source}");
    assert!(source.contains("no finite FlatBuffers bound"), "{source}");
    assert!(
        !source.contains("FlatBuffers> for PumpSetParams"),
        "{source}"
    );
}

/// The package of [`package`] with `Pump`'s parameter a bare `string`, which
/// has no finite bound in either encoding.
fn package_with_an_unbounded_pump() -> v2::Package {
    package(v2::FieldType {
        kind: Some(v2::field_type::Kind::Primitive(
            v2::PrimitiveType::String as i32,
        )),
        ..Default::default()
    })
}

/// Under `proto3`, alone or beside `flatbuffers`, an induced request with
/// no finite bound skips its interface with the same marker the FlatBuffers
/// refusal leaves, and the other interface and the types still generate:
/// the proto3 codec refuses exactly as the FlatBuffers codec does.
#[test]
fn an_unbounded_induced_request_refuses_only_its_interface_under_proto3() {
    let package = package_with_an_unbounded_pump();
    for (encodings, reason) in [
        (
            &[WireEncoding::Proto3][..],
            "`face.calls.PumpSetParams` has no finite proto3 bound: a member of it has no \
             finite bound (NoBound)",
        ),
        // Both named: the FlatBuffers check runs first and names its cause.
        (
            &[WireEncoding::FlatBuffers, WireEncoding::Proto3][..],
            "`face.calls.PumpSetParams` has no finite FlatBuffers bound",
        ),
    ] {
        let source = generate_pipeline(&package, encodings, false, &[])
            .expect("generate pipeline")
            .rust_source;
        compiles(&source);
        assert!(source.contains("__RIDL_NO_FACE_Pump"), "{source}");
        assert!(source.contains("pub mod healthy"), "{source}");
        assert!(source.contains(reason), "{encodings:?}: {source}");
        assert!(
            !source.contains("Proto3> for PumpSetParams"),
            "no `Payload<Proto3>` for the unbounded tuple: {source}"
        );
        assert!(
            !source.contains("Proto3> for Pump {"),
            "no `Sizes<Proto3>` for the skipped interface: {source}"
        );
        assert!(
            source.contains("Proto3> for Healthy {"),
            "the bounded interface keeps its `Sizes<Proto3>`: {source}"
        );
    }
}

/// `[Level; 0..3]?`: an optional array, which a FlatBuffers table holds and
/// proto3 cannot represent, so a tuple that holds it has a finite
/// FlatBuffers bound and no proto3 bound.
fn optional_array_of_level() -> v2::FieldType {
    v2::FieldType {
        optional: true,
        kind: Some(v2::field_type::Kind::Array(Box::new(v2::ArrayType {
            element: Some(Box::new(v2::FieldType {
                kind: Some(v2::field_type::Kind::Named("Level".to_string())),
                ..Default::default()
            })),
            min: 0,
            max: 3,
        }))),
    }
}

/// The bound is read per encoding of the list: an induced request whose
/// FlatBuffers bound is finite and whose proto3 bound is not is faced under
/// `flatbuffers` alone, and skipped under `proto3` alone and under both,
/// where the FlatBuffers check passes and the proto3 check is the one that
/// refuses; the output with both codecs compiles, so no descriptor names a
/// `Payload<Proto3>` the codec did not emit.
#[test]
fn an_induced_request_is_judged_in_each_encoding_the_list_names() {
    let package = package(optional_array_of_level());
    let flatbuffers = generate_pipeline(&package, &[WireEncoding::FlatBuffers], false, &[])
        .expect("generate pipeline")
        .rust_source;
    assert!(
        !flatbuffers.contains("__RIDL_NO_FACE_Pump"),
        "the argument message has a finite FlatBuffers bound: {flatbuffers}"
    );
    for encodings in [
        &[WireEncoding::Proto3][..],
        &[WireEncoding::FlatBuffers, WireEncoding::Proto3][..],
    ] {
        let proto3 = generate_pipeline(&package, encodings, false, &[])
            .expect("generate pipeline")
            .rust_source;
        compiles(&proto3);
        assert!(
            proto3.contains("__RIDL_NO_FACE_Pump"),
            "{encodings:?}: {proto3}"
        );
        assert!(
            proto3.contains("`face.calls.PumpSetParams` has no finite proto3 bound")
                && proto3.contains("(RefusedMember)"),
            "{encodings:?}: the note names the tuple and the cause: {proto3}"
        );
        assert!(
            !proto3.contains("Proto3> for PumpSetParams"),
            "{encodings:?}: no `Payload<Proto3>` for the refused tuple: {proto3}"
        );
    }
}

/// A query's tuple reply is judged like a request: a reply tuple that holds
/// an optional array has a finite FlatBuffers bound and no proto3 bound, so
/// `Pump` is faced under `flatbuffers` and skipped under `proto3`, with the
/// note naming the reply tuple; the query's request, which has no parameter,
/// induces a bounded argument message, so the reply is what refuses.
#[test]
fn an_induced_reply_is_judged_in_each_encoding_the_list_names() {
    let package = ridlc::compile(
        "replies.ridl",
        r#"
package face.replies
type Level : integer [0..100]
interface Pump {
  query span(): (lo: Level, hi: Level) @[..50ms]
}
interface Healthy {
  command set(level: Level) @[..50ms]
}
"#,
    )
    .package;
    let mut interfaces: Vec<_> = package
        .shapes()
        .map(|shape| shape.interface.clone())
        .collect();
    let Some(v2::decl::Kind::QueryDef(query)) = interfaces[0].interactions[0].kind.as_mut() else {
        panic!("a query");
    };
    query.return_type = Some(v2::ReturnType {
        kind: Some(v2::return_type::Kind::Value(v2::FieldType {
            kind: Some(v2::field_type::Kind::Tuple(v2::TupleType {
                fields: vec![
                    v2::TupleField {
                        name: "lo".to_string(),
                        r#type: Some(v2::FieldType {
                            kind: Some(v2::field_type::Kind::Named("Level".to_string())),
                            ..Default::default()
                        }),
                    },
                    v2::TupleField {
                        name: "hi".to_string(),
                        r#type: Some(optional_array_of_level()),
                    },
                ],
            })),
            ..Default::default()
        })),
    });
    let package = v2::Package {
        interfaces,
        ..package
    };
    let flatbuffers = generate_pipeline(&package, &[WireEncoding::FlatBuffers], false, &[])
        .expect("generate pipeline")
        .rust_source;
    assert!(
        !flatbuffers.contains("__RIDL_NO_FACE_Pump"),
        "the reply message has a finite FlatBuffers bound: {flatbuffers}"
    );
    let proto3 = generate_pipeline(&package, &[WireEncoding::Proto3], false, &[])
        .expect("generate pipeline")
        .rust_source;
    compiles(&proto3);
    assert!(proto3.contains("__RIDL_NO_FACE_Pump"), "{proto3}");
    assert!(
        proto3.contains("`face.replies.PumpSpanReply` has no finite proto3 bound")
            && proto3.contains("(RefusedMember)"),
        "the note names the reply tuple and the cause: {proto3}"
    );
    assert!(
        !proto3.contains("Proto3> for PumpSpanReply"),
        "no `Payload<Proto3>` for the refused reply: {proto3}"
    );
    assert!(proto3.contains("pub mod healthy"), "{proto3}");
}

/// `package` with `Pump` moved into the inline shape of the service
/// `face.calls.pump`, which has no declared name.
fn package_with_pump_as_a_service_shape(ty: v2::FieldType) -> v2::Package {
    let mut package = package(ty);
    let pump = package.interfaces.remove(0);
    package.services.push(v2::Service {
        name: "face.calls.pump".to_string(),
        shapes: vec![v2::ServiceShape {
            kind: Some(v2::service_shape::Kind::Inline(v2::Interface {
                name: String::new(),
                ..pump
            })),
        }],
        ..Default::default()
    });
    package
}

/// A service's inline shape has no declared name, so the pipeline emits no
/// descriptors and no face for it and leaves no note; the codec still walks
/// its interactions. Its induced request with no finite bound is left out
/// of the roots, as any unbounded root is, and refuses nothing under either
/// codec.
#[test]
fn an_unbounded_induced_request_of_a_service_shape_refuses_nothing() {
    let package = package_with_pump_as_a_service_shape(v2::FieldType {
        kind: Some(v2::field_type::Kind::Primitive(
            v2::PrimitiveType::String as i32,
        )),
        ..Default::default()
    });
    for encodings in [
        &[WireEncoding::Proto3][..],
        &[WireEncoding::FlatBuffers, WireEncoding::Proto3][..],
    ] {
        let source = generate_pipeline(&package, encodings, false, &[])
            .expect("generate pipeline")
            .rust_source;
        compiles(&source);
        assert!(
            !source.contains("__RIDL_NO_FACE_"),
            "{encodings:?}: {source}"
        );
        assert!(source.contains("pub mod healthy"), "{source}");
    }
}

#[test]
fn a_tuple_parameter_named_params_has_a_distinct_wrapper() {
    let package = package(v2::FieldType {
        kind: Some(v2::field_type::Kind::Tuple(v2::TupleType {
            fields: ["a", "b"]
                .into_iter()
                .map(|name| v2::TupleField {
                    name: name.to_string(),
                    r#type: Some(v2::FieldType {
                        kind: Some(v2::field_type::Kind::Named("Level".to_string())),
                        ..Default::default()
                    }),
                })
                .collect(),
        })),
        ..Default::default()
    });
    let model = ridl_ir::codegen::lower(&package, &[]);
    assert!(
        model.tuple_collisions.is_empty(),
        "{:?}",
        model.tuple_collisions
    );
    let source = generate_pipeline(&package, &[WireEncoding::FlatBuffers], false, &[])
        .expect("generate pipeline")
        .rust_source;
    assert!(!source.contains("__RIDL_NO_FACE_Pump"), "{source}");
    compiles(&source);
}

/// A service's inline shape's induced request with a finite bound is a
/// root: it gets a `Payload<Proto3>` as it gets a `Payload<FlatBuffers>`,
/// although the shape has no declared name, because the FlatBuffers
/// projection roots every induced tuple that crosses the wire whatever its
/// interface's identity. The tuple is named from the service's dotted name.
#[test]
fn a_bounded_induced_request_of_a_service_shape_is_a_root() {
    let package = package_with_pump_as_a_service_shape(v2::FieldType {
        kind: Some(v2::field_type::Kind::Tuple(v2::TupleType {
            fields: ["a", "b"]
                .into_iter()
                .map(|name| v2::TupleField {
                    name: name.to_string(),
                    r#type: Some(v2::FieldType {
                        kind: Some(v2::field_type::Kind::Named("Level".to_string())),
                        ..Default::default()
                    }),
                })
                .collect(),
        })),
        ..Default::default()
    });
    for (encodings, encoding) in [
        (&[WireEncoding::FlatBuffers][..], "FlatBuffers"),
        (&[WireEncoding::Proto3][..], "Proto3"),
    ] {
        let source = generate_pipeline(&package, encodings, false, &[])
            .expect("generate pipeline")
            .rust_source;
        compiles(&source);
        // Whitespace aside: the printer breaks the long `impl` line.
        let dense: String = source.chars().filter(|c| !c.is_whitespace()).collect();
        assert!(
            dense.contains(&format!(
                "impl::ridl_rt::payload::Payload<::ridl_rt::encoding::{encoding}>\
                 forFaceCallsPumpSetParams{{"
            )),
            "{encodings:?}: {source}"
        );
    }
}
