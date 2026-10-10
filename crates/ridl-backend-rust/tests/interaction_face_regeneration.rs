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

/// The fixture declares public interfaces only, and the guard above pins what
/// `generate_face` emits for them. `ridl build --emit rust` goes through the
/// backend contract (`src/contract.rs`), which calls `generate_pipeline_over`,
/// the function `generate_pipeline` wraps; it emits the same items in another
/// order. This test compares the two outputs as sorted lists of
/// top-level items, so the checked-in fixture pins the pipeline's item set for
/// a public interface as well; the pipeline's item order is not pinned. A
/// change to what either entry point emits for a public interface fails it, or
/// fails the guard above.
#[test]
fn the_pipeline_emits_the_items_of_the_checked_in_face() {
    let package = ir::compile_fixture("interaction_face.ridl");
    let pipeline = ridl_backend_rust::generate_pipeline(
        &package,
        ridl_backend_rust::WireEncoding::FlatBuffers,
        &[],
    )
    .expect("generate_pipeline")
    .rust_source;
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/generated/interaction_face.rs");
    let checked_in = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
    let pipeline = sorted_items(&pipeline);
    let checked_in = sorted_items(&checked_in);
    let only_pipeline = missing_from(&pipeline, &checked_in);
    let only_fixture = missing_from(&checked_in, &pipeline);
    assert!(
        only_pipeline.is_empty() && only_fixture.is_empty(),
        "generate_pipeline and the checked-in face differ. If the guard above \
         also fails, the fixture is stale: regenerate it with \
         RIDL_UPDATE_GENERATED=1 cargo test -p ridl-backend-rust --test \
         interaction_face_regeneration, then rerun. Otherwise the pipeline's \
         output for a public interface has diverged from generate_face's.\n\
         items only in the pipeline output ({}):\n{}\n\
         items only in the checked-in face ({}):\n{}",
        only_pipeline.len(),
        only_pipeline.join("\n"),
        only_fixture.len(),
        only_fixture.join("\n"),
    );
}

/// The items of `left` (a sorted list) that `right` (a sorted list) does not
/// hold, counted with multiplicity.
fn missing_from<'a>(left: &'a [String], right: &[String]) -> Vec<&'a str> {
    let mut right = right.iter().peekable();
    let mut missing = Vec::new();
    for item in left {
        while right.next_if(|other| *other < item).is_some() {}
        if right.next_if(|other| *other == item).is_none() {
            missing.push(item.as_str());
        }
    }
    missing
}

/// Every top-level item of `source`, rendered as one string, in sorted order.
fn sorted_items(source: &str) -> Vec<String> {
    use quote::ToTokens;
    let file = syn::parse_file(source).expect("the generated source parses");
    let mut items: Vec<String> = file
        .items
        .iter()
        .map(|item| item.to_token_stream().to_string())
        .collect();
    items.sort();
    items
}
