//! The descriptor's FlatBuffers size state agrees with the generated codec:
//! every `MAX_SIZE` the Rust backend writes for the corpus package is the
//! `Bounded(n)` this crate advertises for the same type. Both read
//! `ridl_ir::projection::flatbuffers::max_size`, the one implementation of
//! the bound (`docs/design/flatbuffers-codec.md`); this is the test that
//! fails if either side stops doing so.

use std::path::Path;

use ridl_descriptor::Encoding;
use ridl_descriptor::size::{Ctx, SizeState, size_state};

/// The corpus package's IR snapshot.
fn corpus() -> ridl_ir::v2::Package {
    let snapshot = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../ridl/tests/baseline-corpus/.ridl/baseline/corpus.baseline.ir.json");
    let text = std::fs::read_to_string(&snapshot).expect("the corpus snapshot is checked in");
    ridl_ir::v2::from_json(&text).expect("the snapshot is canonical IR JSON")
}

/// Every `(type, MAX_SIZE)` pair in the generated source, read from the
/// `impl Payload<FlatBuffers> for T` header and the `const MAX_SIZE` item the
/// codec emitter writes under it (`payload_impl` in
/// `crates/ridl-backend-rust/src/codec.rs`).
fn max_sizes(source: &str) -> Vec<(String, u32)> {
    const HEADER: &str = "impl ::ridl_rt::payload::Payload<::ridl_rt::encoding::FlatBuffers> for ";
    const CONST: &str = "const MAX_SIZE: ::core::primitive::usize = ";
    let mut pairs = Vec::new();
    let mut rest = source;
    while let Some(at) = rest.find(HEADER) {
        let after = &rest[at + HEADER.len()..];
        let type_name = after
            .split(|c: char| !(c.is_alphanumeric() || c == '_'))
            .next()
            .expect("a type name follows the header")
            .to_owned();
        // This impl's text ends where the next impl header begins, so a
        // missing constant is reported for the type that lacks it.
        let end = after.find(HEADER).unwrap_or(after.len());
        let at_const = after[..end]
            .find(CONST)
            .unwrap_or_else(|| panic!("a MAX_SIZE follows the header for {type_name}"));
        let digits: String = after[at_const + CONST.len()..]
            .chars()
            .take_while(char::is_ascii_digit)
            .collect();
        let value: u32 = digits.parse().expect("MAX_SIZE is a usize literal");
        pairs.push((type_name, value));
        rest = &after[end..];
    }
    pairs
}

#[test]
fn every_generated_max_size_is_the_descriptors_flatbuffers_bound() {
    let package = corpus();
    let generated = ridl_backend_rust::generate(&package).expect("the corpus generates");
    let pairs = max_sizes(&generated.rust_source);
    assert!(
        !pairs.is_empty(),
        "the corpus has at least one FlatBuffers root"
    );

    let others: [&ridl_ir::v2::Package; 0] = [];
    let ctx = Ctx::new(&package, &others);
    for (type_name, value) in pairs {
        assert_eq!(
            size_state(&type_name, &ctx, Encoding::FlatBuffers),
            Some(SizeState::Bounded(value)),
            "{type_name}"
        );
    }
}
