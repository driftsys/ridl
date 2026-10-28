//! The `transcode` key of `[backend.rust]`, driven from a manifest through
//! `ridlc::run_build` to the emitted Rust source: with the key set to `true`
//! and two encodings listed the source holds the transcode functions, and
//! with the key absent it holds none.

use ridlc::Emit;

/// Builds a one-package workspace whose manifest holds `backend_rust` under
/// `[backend.rust]`, and returns the emitted `lib.rs`.
fn emitted_source(backend_rust: &str) -> String {
    let work = tempfile::tempdir().expect("a temp dir is created");
    let src = work.path().join("src");
    let out = work.path().join("out");
    std::fs::create_dir_all(&src).expect("the directory is created");
    std::fs::write(
        src.join("ridl.toml"),
        format!("[package]\nname = \"tx\"\nversion = \"1.0.0\"\n\n[backend.rust]\n{backend_rust}"),
    )
    .expect("the manifest is written");
    std::fs::write(
        src.join("tx.ridl"),
        "package tx\n\n/// A level.\ntype Level: integer [0..100]\n",
    )
    .expect("the source is written");
    let run = ridlc::run_build(&src, &out, &[Emit::Rust], false.into()).expect("the build runs");
    assert!(!run.has_error(), "{:?}", run.diagnostics);
    let mut source = String::new();
    append_rust_files(&out, &mut source);
    assert!(!source.is_empty(), "the build emitted Rust source");
    source
}

/// Appends the text of every `.rs` file under `dir`, whatever the crate's
/// module layout.
fn append_rust_files(dir: &std::path::Path, into: &mut String) {
    for entry in std::fs::read_dir(dir).expect("the directory is read") {
        let path = entry.expect("an entry is read").path();
        if path.is_dir() {
            append_rust_files(&path, into);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            into.push_str(&std::fs::read_to_string(&path).expect("the file is read"));
        }
    }
}

#[test]
fn a_manifest_with_transcode_true_emits_the_transcode_functions() {
    let source = emitted_source("encodings = [\"flatbuffers\", \"proto3\"]\ntranscode = true\n");
    assert!(
        source.contains("fn transcode_flatbuffers_to_proto3"),
        "the source holds the FlatBuffers to proto3 function"
    );
    assert!(
        source.contains("fn transcode_proto3_to_flatbuffers"),
        "the source holds the proto3 to FlatBuffers function"
    );
}

#[test]
fn a_manifest_without_the_transcode_key_emits_no_transcode_function() {
    let source = emitted_source("encodings = [\"flatbuffers\", \"proto3\"]\n");
    assert!(
        source.contains("Payload<::ridl_rt::encoding::Proto3> for Level"),
        "the source holds the codec"
    );
    assert!(!source.contains("transcode_"), "the key is absent");
}
