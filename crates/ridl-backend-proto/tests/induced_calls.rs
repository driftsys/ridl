#[allow(dead_code)]
mod support;

#[test]
fn induced_call_messages_match_the_lowered_names() {
    let output = ridlc::compile(
        "calls.ridl",
        r#"
package wire.calls
type Level : integer [0..100]
interface Pump {
  command empty() @[..50ms]
  command both(a: Level, b: Level) @[..50ms]
  query bounds(): (a: Level, b: Level) @[..50ms]
  query nested(): (span: (a: Level, b: Level), tags: [Level: (a: Level, b: Level); 0..2]) @[..50ms]
}
"#,
    );
    let model = ridl_ir::codegen::lower(&output.package, &[]);
    let source = ridl_backend_proto::generate(&output.package)
        .expect("generate")
        .proto_source;
    for tuple in &model.tuples {
        let name = &tuple.name.as_ref().expect("a name").wire;
        assert!(source.contains(&format!("message {name} {{")), "{source}");
    }
    assert!(source.contains("message PumpEmptyParams {\n}"), "{source}");
    assert!(source.contains("uint32 field_2 = 2"), "{source}");
    support::compile_with_protox("calls.proto", &source);

    let mut requested = model;
    requested.tuples[0].name.as_mut().expect("a name").wire = "AssignedEmptyParams".to_string();
    let response = ridl_ir::codegen::Backend::generate(
        &ridl_backend_proto::Backend::new(ridl_ir::codegen::RawIr {
            package: &output.package,
            others: &[],
        }),
        &ridl_ir::codegen::v1::CodegenRequest {
            model: Some(requested),
            artifact_base: "calls".to_string(),
            ..Default::default()
        },
    );
    assert!(
        response.diagnostics.is_empty(),
        "{:?}",
        response.diagnostics
    );
    let Some(ridl_ir::codegen::v1::generated_file::Content::Text(source)) =
        response.files[0].content.as_ref()
    else {
        panic!("a text schema");
    };
    assert!(source.contains("message AssignedEmptyParams {"), "{source}");
    support::compile_with_protox("requested.proto", source);
}
