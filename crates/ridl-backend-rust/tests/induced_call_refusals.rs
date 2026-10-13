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
    let package = package(v2::FieldType {
        kind: Some(v2::field_type::Kind::Primitive(
            v2::PrimitiveType::String as i32,
        )),
        ..Default::default()
    });
    let source = generate_pipeline(&package, &[WireEncoding::FlatBuffers], &[])
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
    let source = generate_pipeline(&package, &[WireEncoding::FlatBuffers], &[])
        .expect("generate pipeline")
        .rust_source;
    assert!(!source.contains("__RIDL_NO_FACE_Pump"), "{source}");
    compiles(&source);
}
