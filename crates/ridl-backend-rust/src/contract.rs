//! This backend behind the backend contract (ADR-0020 decision 9): the
//! in-process face `ridlc` calls, and the shape a `ridlc-gen-rust` plugin
//! will wrap once the backend reads the model.
//!
//! **Transition.** [`generate_pipeline`](crate::generate_pipeline) still
//! reads the raw IR; stage P4 of the lane P driver ports it onto the model
//! one layer at a time (`docs/design/codegen-plugins.md`). Until then the
//! backend value holds a [`RawIr`] and `generate` reads that in place of
//! `request.model`, which is what keeps the request the contract's request —
//! the model and the options, never the raw IR — while the port is under
//! way.

use ridl_ir::codegen::{self, RawIr, v1};

use crate::{WireEncoding, generate_pipeline};

/// The one option this backend reads: the payload encoding the generated
/// face encodes and verifies over. The only value today is `flatbuffers`,
/// which is also the default when the option is absent (`WireEncoding`'s
/// `#[non_exhaustive]` note names the encodings that join it).
pub const WIRE_ENCODING_OPTION: &str = "wire-encoding";

/// The Rust backend as a [`codegen::Backend`]: one file,
/// `<artifact_base>.rs`, from [`generate_pipeline`].
pub struct Backend<'a> {
    raw: RawIr<'a>,
}

impl<'a> Backend<'a> {
    /// A backend over the raw IR it still reads (module documentation).
    pub fn new(raw: RawIr<'a>) -> Self {
        Self { raw }
    }
}

impl codegen::Backend for Backend<'_> {
    fn language(&self) -> &str {
        "rust"
    }

    fn generate(&self, request: &v1::CodegenRequest) -> v1::CodegenResponse {
        let mut wire = WireEncoding::default();
        for option in &request.options {
            match (option.key.as_str(), option.value.as_str()) {
                (WIRE_ENCODING_OPTION, "flatbuffers") => wire = WireEncoding::FlatBuffers,
                (WIRE_ENCODING_OPTION, value) => {
                    return refusal(format!(
                        "the rust backend has no `{WIRE_ENCODING_OPTION}` named `{value}`; the \
                         one it has is `flatbuffers`"
                    ));
                }
                (key, _) => {
                    return refusal(format!(
                        "the rust backend takes one option, `{WIRE_ENCODING_OPTION}`; `{key}` \
                         is not one it knows"
                    ));
                }
            }
        }
        match generate_pipeline(self.raw.package, wire, self.raw.others) {
            Ok(generated) => v1::CodegenResponse {
                files: vec![codegen::text_file(
                    format!("{}.rs", request.artifact_base),
                    generated.rust_source,
                )],
                diagnostics: Vec::new(),
            },
            Err(err) => refusal(err.message),
        }
    }
}

fn refusal(message: String) -> v1::CodegenResponse {
    v1::CodegenResponse {
        files: Vec::new(),
        diagnostics: vec![codegen::error(message)],
    }
}
