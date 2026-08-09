//! Shared test support for `ridl-backend-flatbuffers`: compiling an emitted
//! FlatBuffers schema with `planus`, the validity check of ADR-0017's
//! totality property.
//!
//! Lives under `tests/support/` (a subdirectory) rather than directly under
//! `tests/`, because Cargo's test-target auto-discovery only turns a file
//! placed directly in `tests/` into its own integration test binary — a
//! nested file is only compiled when a real test target names it with `mod
//! support;`, so it does not also become an empty test of its own.
//!
//! Loaded from two places: `crates/ridl-backend-flatbuffers/tests/corpus.rs`
//! (an integration test, via `mod support;`) and
//! `crates/ridl-backend-flatbuffers/src/tests.rs` (the unit tests, via
//! `#[path = "../tests/support/mod.rs"] mod support;`) — one definition for
//! both, rather than two copies (mirrors `ridl-backend-proto`'s
//! `tests/support/mod.rs`).

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

/// [`compile_with_planus`], with `siblings` written into the same temp
/// directory first — for a schema whose `include` names another package's
/// own generated file, which must exist for `planus` to resolve it.
/// `siblings` is `(file_name, source)` pairs. Every test that produces a
/// schema calls one of these two harnesses, so a broken include cannot hide
/// behind a skipped validity check.
pub(crate) fn compile_with_planus_and_siblings(
    entry_file: &str,
    entry_source: &str,
    siblings: &[(&str, &str)],
) {
    let dir = tempfile::tempdir().expect("temp dir");
    for (file_name, source) in siblings {
        std::fs::write(dir.path().join(file_name), source).expect("write sibling schema");
    }
    let path = dir.path().join(entry_file);
    std::fs::write(&path, entry_source).expect("write schema");
    if planus_translation::translate_files(&[path]).is_none() {
        panic!("emitted schema is not a valid FlatBuffers schema:\n\n{entry_source}");
    }
}
