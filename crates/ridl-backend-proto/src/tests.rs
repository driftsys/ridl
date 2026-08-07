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
