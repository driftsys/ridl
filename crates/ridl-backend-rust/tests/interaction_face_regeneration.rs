//! The regeneration guard of the interaction-face fixture
//! (`docs/design/interaction-face.md`).
//!
//! `tests/generated/interaction_face.rs` is `generate_face`'s output for
//! `tests/fixtures/interaction_face.ridl`, checked in and `include!`d by
//! `tests/interaction_face.rs`. This guard lives in its own test target, apart
//! from the tests that consume the fixture, so that when a change to the
//! emitter changes the face's shape and those tests no longer compile against
//! the stale fixture, `RIDL_UPDATE_GENERATED=1 cargo test -p
//! ridl-backend-rust --test interaction_face_regeneration` still builds and
//! rewrites it.

#[path = "support/ir.rs"]
mod ir;

/// Regenerates the fixture through `generate_face`, using the crate's own
/// compile helper rather than a new cargo subprocess, and compares it
/// byte-for-byte against the checked-in file. `RIDL_UPDATE_GENERATED=1`
/// writes the fresh output instead of comparing.
#[test]
fn generated_interaction_face_matches_the_emitter() {
    let package = ir::compile_fixture("interaction_face.ridl");
    let face = ridl_backend_rust::generate_face(&package)
        .expect("generate_face")
        .rust_source;

    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/generated/interaction_face.rs");

    if std::env::var_os("RIDL_UPDATE_GENERATED").is_some() {
        std::fs::write(&path, &face)
            .unwrap_or_else(|error| panic!("write {}: {error}", path.display()));
        return;
    }

    let checked_in = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
    assert_eq!(
        face, checked_in,
        "generated interaction_face.rs is stale; regenerate it with \
         RIDL_UPDATE_GENERATED=1 cargo test -p ridl-backend-rust --test interaction_face_regeneration"
    );
}
