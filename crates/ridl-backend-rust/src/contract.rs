//! This backend behind the backend contract (ADR-0020 decision 9): the
//! in-process face `ridlc` calls, and the one the reference plugin
//! `ridlc-gen-rust` wraps.
//!
//! It reads the request and nothing else: the model the request carries is what
//! every emitter of this crate reads, so the in-process host and the process
//! host hand the backend the same thing, and the parity test compares the two
//! (`docs/design/codegen-plugins.md`).

use ridl_ir::codegen::{self, v1};

use crate::{WireEncoding, generate_pipeline_over};

/// The one option this backend reads: the encodings the generated package
/// carries a codec for, as the manifest's `[backend.rust] encodings` names
/// them, joined by `,`. Absent, the backend emits the domain types only. The
/// only name it has a codec for today is `flatbuffers`; `proto3` and `repr-c`
/// are refused with the issue that builds each codec, and any other name is
/// refused as unknown (`WireEncoding`'s `#[non_exhaustive]` note names the
/// encodings that join it).
pub const ENCODINGS_OPTION: &str = "encodings";

/// The Rust backend as a [`codegen::Backend`]: one file,
/// `<artifact_base>.rs`, from
/// [`generate_pipeline`](crate::generate_pipeline) over the request's model.
pub struct Backend;

impl codegen::Backend for Backend {
    fn language(&self) -> &str {
        "rust"
    }

    fn generate(&self, request: &v1::CodegenRequest) -> v1::CodegenResponse {
        let mut encodings: Vec<WireEncoding> = Vec::new();
        for option in &request.options {
            if option.key != ENCODINGS_OPTION {
                return refusal(format!(
                    "the Rust backend takes one option, `{ENCODINGS_OPTION}`; `{}` is not one \
                     it knows",
                    option.key
                ));
            }
            for name in option.value.split(',') {
                match parse_encoding(name) {
                    Ok(encoding) if encodings.contains(&encoding) => {
                        return refusal(format!(
                            "`{ENCODINGS_OPTION}` names `{name}` twice; each encoding is named \
                             once"
                        ));
                    }
                    Ok(encoding) => encodings.push(encoding),
                    Err(message) => return refusal(message),
                }
            }
        }
        let Some(model) = &request.model else {
            return refusal("the request carries no model".to_string());
        };
        let preamble = codegen::comment_preamble(&request.generated_marker, &request.header, "//");
        match generate_pipeline_over(model, &encodings) {
            Ok(generated) => v1::CodegenResponse {
                files: vec![codegen::text_file(
                    format!("{}.rs", request.artifact_base),
                    format!("{preamble}{}", generated.rust_source),
                )],
                diagnostics: Vec::new(),
            },
            Err(err) => refusal(err.message),
        }
    }
}

/// One element of the `encodings` value, or the refusal it draws: an empty
/// element, an encoding this backend has no codec for yet (named with the
/// issue that builds it), or a name that is no encoding at all.
fn parse_encoding(name: &str) -> Result<WireEncoding, String> {
    if name.is_empty() {
        return Err(format!(
            "`{ENCODINGS_OPTION}` names an empty element; each element is one of \
             `flatbuffers`, `proto3` and `repr-c`"
        ));
    }
    if let Some(encoding) = WireEncoding::parse(name) {
        return Ok(encoding);
    }
    Err(match name {
        "proto3" => "the Rust backend has no `proto3` codec (driftsys/ridl#264)".to_string(),
        "repr-c" => "the Rust backend has no `repr-c` codec (driftsys/ridl#317)".to_string(),
        other => format!(
            "`{ENCODINGS_OPTION}` names `{other}`, which is not an encoding; the names are \
             `flatbuffers`, `proto3` and `repr-c`"
        ),
    })
}

fn refusal(message: String) -> v1::CodegenResponse {
    v1::CodegenResponse {
        files: Vec::new(),
        diagnostics: vec![codegen::error(message)],
    }
}
