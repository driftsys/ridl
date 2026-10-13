#[allow(dead_code)]
mod support;

#[test]
fn induced_call_tables_match_the_lowered_names() {
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
    let source = ridl_backend_flatbuffers::generate(&output.package)
        .expect("generate")
        .fbs_source;
    for tuple in &model.tuples {
        let name = &model.flatbuffers.as_ref().expect("projection").tables
            [tuple.flatbuffers_table.expect("a table") as usize]
            .name;
        assert!(source.contains(&format!("table {name} {{")), "{source}");
    }
    assert!(source.contains("table PumpEmptyParams {\n}"), "{source}");
    assert!(source.contains("field_2: ubyte (id: 1)"), "{source}");
    support::compile_with_planus("calls.fbs", &source);

    let mut requested = model;
    let tuple = &requested.tuples[0];
    let table = tuple.flatbuffers_table.expect("a table");
    requested.flatbuffers.as_mut().expect("projection").tables[table as usize].name =
        "AssignedEmptyParams".to_string();
    let response = ridl_ir::codegen::Backend::generate(
        &ridl_backend_flatbuffers::Backend::new(ridl_ir::codegen::RawIr {
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
    assert!(source.contains("table AssignedEmptyParams {"), "{source}");
    support::compile_with_planus("requested.fbs", source);
}
