//! The catalog hash of the corpus package is pinned (driver §4 answer 4).
//! A different value here means the reduced package or the binary encoding
//! changed; ADR-0014 decision 15 says when the pin may move.
//!
//! driftsys/ridl#275's criterion, that the hash does not depend on which
//! wire schema a build emits, is tested end to end by
//! `catalog_hash_is_the_same_whether_a_build_emits_proto_flatbuffers_or_both`
//! in `crates/ridl/tests/facade.rs`.

use std::path::Path;

use ridl_descriptor::hash::catalog_hash;

/// Pinned on the first run; see ADR-0014 decision 15 for the rule on moving it.
const CORPUS_HASH: &str = "ff876b7ac7f4ba17e20be3863c31956059f26f17bf905ceea67af0aab4d49bb2";

/// The corpus snapshot was published before shapes carried a number (`"number"`
/// does not occur in the file). The pin must cover the numbers, so the test
/// numbers the shapes in `Package::shapes()` order, 1.., all provisional,
/// before hashing.
fn numbered_corpus() -> ridl_ir::v2::Package {
    let snapshot = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../ridl/tests/baseline-corpus/.ridl/baseline/corpus.baseline.ir.json");
    let text = std::fs::read_to_string(&snapshot).expect("the corpus snapshot is checked in");
    let mut package = ridl_ir::v2::from_json(&text).expect("the snapshot is canonical IR JSON");
    let mut next = 1u32;
    for interface in &mut package.interfaces {
        interface.number = next;
        interface.provisional = true;
        next += 1;
    }
    for service in &mut package.services {
        for shape in &mut service.shapes {
            if let Some(ridl_ir::v2::service_shape::Kind::Inline(interface)) = &mut shape.kind {
                interface.number = next;
                interface.provisional = true;
                next += 1;
            }
        }
    }
    package
}

#[test]
fn the_corpus_hash_is_pinned() {
    let package = numbered_corpus();
    assert!(package.shapes().all(|shape| shape.interface.number != 0));
    let hash = catalog_hash(&package, &[]);
    let hex: String = hash.iter().map(|b| format!("{b:02x}")).collect();
    assert_eq!(hex, CORPUS_HASH);
}
