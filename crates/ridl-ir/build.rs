//! Generates the IR Rust types from the protobuf schemas at build time.
//!
//! The v2 schema (the typl surface plus the ridl interaction layer, ADR-0008
//! decision 8) is compiled with `protox`, a pure-Rust protobuf front end, so
//! the build needs no system `protoc` binary (ADR-0006 decision 3). The
//! resulting descriptor set is handed to `prost-build`, which emits the Rust
//! types, and is also written to `OUT_DIR`, where `lib.rs` embeds it as the
//! `prost-reflect` descriptor pool behind the canonical protobuf JSON surface
//! (ADR-0014 decision 7) — pool and types come from one schema compilation,
//! so the two cannot disagree. The v1 schema was removed when its last
//! consumer moved to v2 (task 6 of the E2 plan), mirroring the E0 v0
//! retirement.

use std::error::Error;

use prost::Message;

fn main() -> Result<(), Box<dyn Error>> {
    let protos = ["proto/ridl/ir/v2/ir.proto"];
    let include = "proto";

    for proto in protos {
        println!("cargo:rerun-if-changed={proto}");
    }
    println!("cargo:rerun-if-changed={include}");

    let file_descriptors = protox::compile(protos, [include])?;

    let out_dir = std::env::var("OUT_DIR")?;
    std::fs::write(
        std::path::Path::new(&out_dir).join("ir_descriptor.binpb"),
        file_descriptors.encode_to_vec(),
    )?;

    prost_build::Config::new()
        // Box the inline-scalar oneof member: an inline TypeDef is the
        // largest FieldType kind by far, and boxing it keeps FieldType (and
        // everything holding one) small. prost boxes the recursive
        // array/map members on its own; this one is not recursive, so it is
        // boxed explicitly.
        .boxed(".ridl.ir.v2.FieldType.kind.inline_scalar")
        .compile_fds(file_descriptors)?;

    Ok(())
}
