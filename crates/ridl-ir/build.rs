//! Generates the IR and codegen-model Rust types from the protobuf schemas at
//! build time.
//!
//! The v2 schema — `ir.proto`, the typl surface plus the ridl interaction
//! layer (ADR-0008 decision 8), and `system.proto`, the rsdl system layer
//! (rsdl reference §13) — is compiled with `protox`, a pure-Rust protobuf
//! front end, so the build needs no system `protoc` binary (ADR-0006 decision
//! 3). The resulting descriptor set is used three times, all from the one
//! `protox` compilation, so no two outputs can disagree: `prost-build` emits the Rust
//! types; `pbjson-build` generates the canonical protobuf JSON serde impls
//! for those types (ADR-0014 decision 14); and the set is written to
//! `OUT_DIR`, where `lib.rs` embeds it as the `prost-reflect` descriptor
//! pool behind prototext (ADR-0014 decision 7, amended by decision 14). The
//! v1 schema was removed when its last consumer moved to v2 (task 6 of the
//! E2 plan), mirroring the E0 v0 retirement.
//!
//! `proto/ridl/codegen/v1/model.proto` — the lowered codegen model
//! (`ridl.codegen.v1`, the codegen model design note §3) — is compiled in the
//! same `protox` call and registered with the same `pbjson-build` builder, so
//! the model is written in the exact dialect ADR-0014 decision 14 fixes for
//! the IR, and the two schemas share one descriptor pool.

use std::error::Error;

use prost::Message;

fn main() -> Result<(), Box<dyn Error>> {
    let protos = [
        "proto/ridl/ir/v2/ir.proto",
        "proto/ridl/ir/v2/system.proto",
        "proto/ridl/codegen/v1/model.proto",
    ];
    let include = "proto";

    for proto in protos {
        println!("cargo:rerun-if-changed={proto}");
    }
    println!("cargo:rerun-if-changed={include}");

    let file_descriptors = protox::compile(protos, [include])?;
    let descriptor_bytes = file_descriptors.encode_to_vec();

    let out_dir = std::env::var("OUT_DIR")?;
    std::fs::write(
        std::path::Path::new(&out_dir).join("ir_descriptor.binpb"),
        &descriptor_bytes,
    )?;

    prost_build::Config::new()
        // Box the inline-scalar oneof member: an inline TypeDef is the
        // largest FieldType kind by far, and boxing it keeps FieldType (and
        // everything holding one) small. prost boxes the recursive
        // array/map members on its own; this one is not recursive, so it is
        // boxed explicitly. pbjson-build needs no matching configuration:
        // serde's blanket `Box<T>` impls cover the boxed member.
        .boxed(".ridl.ir.v2.FieldType.kind.inline_scalar")
        // The codegen model's two slot oneofs, for the same reason: a live
        // field and a live interaction are each far larger than the tombstone
        // beside them, and a slot list is one `Vec` of the larger of the two.
        // prost boxes the recursive members on its own; these are not
        // recursive, so they are boxed explicitly.
        .boxed(".ridl.codegen.v1.Slot.occupant.field")
        .boxed(".ridl.codegen.v1.InteractionSlot.occupant.interaction")
        .compile_fds(file_descriptors)?;

    // The canonical protobuf JSON serde impls (ADR-0014 decision 14), written
    // to `OUT_DIR` as `ridl.ir.v2.serde.rs` and included next to the types.
    // `emit_fields()` is decision 2's contract: a non-`optional` field holding
    // its default is emitted rather than skipped, a proto3 `optional` field is
    // still gated on `is_some()`, and `null` never appears. It is the only
    // option set: `retain_enum_prefix()` governs the assumed Rust variant
    // naming (not the JSON) and breaks compilation against prost's
    // prefix-stripped variants, and `ignore_unknown_fields()` must stay unset
    // so the generated deserializer keeps rejecting unknown fields — the
    // strictness ADR-0014 decision 11's conformance test relies on.
    pbjson_build::Builder::new()
        .register_descriptors(&descriptor_bytes)?
        .emit_fields()
        .build(&[".ridl.ir.v2", ".ridl.codegen.v1"])?;

    Ok(())
}
