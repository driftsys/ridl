//! Writes the generated Rust codec of three fixtures into `OUT_DIR`, one file
//! per wire encoding, for the targets to `include!`. The fixtures are the ones
//! the codec tests of `ridl-backend-rust` compile, so a target reads the same
//! types those tests do.

use std::path::Path;

use ridl_backend_rust::WireEncoding;

fn main() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let fixtures = manifest.join("../crates/ridl-backend-rust/tests/fixtures");
    let out = std::env::var_os("OUT_DIR").expect("cargo sets OUT_DIR");
    for (fixture, encoding, output) in [
        ("proto3_roundtrip.ridl", WireEncoding::Proto3, "proto3.rs"),
        (
            "flatbuffers_roundtrip.ridl",
            WireEncoding::FlatBuffers,
            "flatbuffers.rs",
        ),
        ("podbuf_roundtrip.ridl", WireEncoding::Podbuf, "podbuf.rs"),
    ] {
        let path = fixtures.join(fixture);
        println!("cargo:rerun-if-changed={}", path.display());
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
        let compiled = ridlc::compile(&path.display().to_string(), &text);
        let errors: Vec<_> = compiled
            .diagnostics
            .iter()
            .filter(|d| matches!(d.severity, ridl_core::diag::Severity::Error))
            .collect();
        assert!(errors.is_empty(), "{fixture} must compile: {errors:?}");
        let generated =
            ridl_backend_rust::generate_pipeline(&compiled.package, &[encoding], false, &[])
                .unwrap_or_else(|error| panic!("{fixture} generates: {}", error.message))
                .rust_source;
        std::fs::write(Path::new(&out).join(output), generated).expect("write generated source");
    }
    println!("cargo:rerun-if-changed=build.rs");
}
