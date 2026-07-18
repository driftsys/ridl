//! Generates the IR v0 Rust types from the protobuf schema at build time.
//!
//! The schema is compiled with `protox`, a pure-Rust protobuf front end, so
//! the build needs no system `protoc` binary (ADR-0006 decision 3). The
//! resulting descriptor set is handed to `prost-build`, which emits the Rust
//! types. `type_attribute` adds the `serde` derives so the JSON debug
//! rendering of the IR exists (ADR-0004 §4).

use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    let proto = "proto/ridl/ir/v0/ir.proto";
    let include = "proto";

    println!("cargo:rerun-if-changed={proto}");
    println!("cargo:rerun-if-changed={include}");

    let file_descriptors = protox::compile([proto], [include])?;

    prost_build::Config::new()
        .type_attribute(".", "#[derive(serde::Serialize, serde::Deserialize)]")
        .compile_fds(file_descriptors)?;

    Ok(())
}
