//! This backend behind the backend contract (ADR-0020 decision 9): the
//! in-process face `ridlc` calls, and the one the reference plugin
//! `ridlc-gen-rust` wraps.
//!
//! Since stage P4 of the lane P driver it reads the request and nothing
//! else: the model the request carries is what every emitter of this crate
//! reads, so the in-process host and the process host hand the backend the
//! same thing, and the parity test compares the two
//! (`docs/design/codegen-plugins.md`).

use ridl_ir::codegen::{self, v1};

use crate::{WireEncoding, generate_pipeline_over};

/// The one option this backend reads: the payload encoding the generated
/// face encodes and verifies over. The only value today is `flatbuffers`,
/// which is also the default when the option is absent (`WireEncoding`'s
/// `#[non_exhaustive]` note names the encodings that join it).
pub const WIRE_ENCODING_OPTION: &str = "wire-encoding";

/// The Rust backend as a [`codegen::Backend`]: one file,
/// `<artifact_base>.rs`, from
/// [`generate_pipeline`](crate::generate_pipeline) over the request's model.
pub struct Backend;

impl codegen::Backend for Backend {
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
        let Some(model) = &request.model else {
            return refusal("the request carries no model".to_string());
        };
        match generate_pipeline_over(model, wire) {
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
