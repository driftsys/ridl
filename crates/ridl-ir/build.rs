//! Generates the IR Rust types from the protobuf schema at build time.
//!
//! The v1 schema (the typl surface with exact values) is compiled with
//! `protox`, a pure-Rust protobuf front end, so the build needs no system
//! `protoc` binary (ADR-0006 decision 3). The resulting descriptor set is
//! handed to `prost-build`, which emits the Rust types. `type_attribute` adds
//! the `serde` derives so the JSON debug rendering of the IR exists
//! (ADR-0004 §4). The E0 v0 schema was removed when its last consumer moved
//! to v1 (task 13 of the E1 plan).

use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    let protos = ["proto/ridl/ir/v1/ir.proto"];
    let include = "proto";

    for proto in protos {
        println!("cargo:rerun-if-changed={proto}");
    }
    println!("cargo:rerun-if-changed={include}");

    let file_descriptors = protox::compile(protos, [include])?;

    prost_build::Config::new()
        .type_attribute(".", "#[derive(serde::Serialize, serde::Deserialize)]")
        // Box the inline-scalar oneof member: an inline TypeDef is the
        // largest FieldType kind by far, and boxing it keeps FieldType (and
        // everything holding one) small. prost boxes the recursive
        // array/map members on its own; this one is not recursive, so it is
        // boxed explicitly.
        .boxed(".ridl.ir.v1.FieldType.kind.inline_scalar")
        .compile_fds(file_descriptors)?;

    Ok(())
}
