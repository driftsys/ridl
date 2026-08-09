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
