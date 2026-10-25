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
/// names it has a codec for are `flatbuffers` and `proto3`; `podbuf` is
/// refused with the issue that builds its codec, and any other name is
/// refused as unknown (`WireEncoding`'s `#[non_exhaustive]` note names the
/// encodings that join it).
pub const ENCODINGS_OPTION: &str = "encodings";

/// The option that says the crate carries the `wasm` module, as the manifest's
/// `[backend.rust] wasm = true` sets it, with the value `true`. The module is
/// written once per crate by `ridlc` ([`generate_wasm_module`]), so this
/// backend's per-package output does not change with it; the backend reads it
/// to refuse a package whose `encodings` does not name `flatbuffers`, because
/// every export of the module checks FlatBuffers bytes. The value `implied`,
/// which `ridlc` sends when `codec = true` turns the module on, reads as `true`
/// and changes only the wording of that refusal.
///
/// [`generate_wasm_module`]: crate::generate_wasm_module
pub const WASM_OPTION: &str = "wasm";

/// The option that asks for the `transcode_<s>_to_<d>` functions, as the
/// manifest's `[backend.rust] transcode = true` sets it, with the value `true`
/// (`false` is the same as absent). The functions are emitted for each type
/// that has a `Payload` implementation under two listed encodings.
pub const TRANSCODE_OPTION: &str = "transcode";

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
        let mut wasm = false;
        let mut transcode = false;
        let mut implied = false;
        for option in &request.options {
            if option.key == WASM_OPTION {
                match option.value.as_str() {
                    "true" => (wasm, implied) = (true, false),
                    "implied" => (wasm, implied) = (true, true),
                    "false" => (wasm, implied) = (false, false),
                    other => {
                        return refusal(format!(
                            "`{WASM_OPTION}` is `{other}`; the value is `true` or `false`"
                        ));
                    }
                }
                continue;
            }
            if option.key == TRANSCODE_OPTION {
                match option.value.as_str() {
                    "true" => transcode = true,
                    "false" => transcode = false,
                    other => {
                        return refusal(format!(
                            "`{TRANSCODE_OPTION}` is `{other}`; the value is `true` or `false`"
                        ));
                    }
                }
                continue;
            }
            if option.key != ENCODINGS_OPTION {
                return refusal(format!(
                    "the Rust backend takes the options `{ENCODINGS_OPTION}`, \
                     `{WASM_OPTION}` and `{TRANSCODE_OPTION}`; `{}` is not one it knows",
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
        if implied && !encodings.contains(&WireEncoding::FlatBuffers) {
            return refusal(format!(
                "`{WASM_OPTION}` is implied by `codec = true` in the manifest, and \
                 `[backend.rust] {ENCODINGS_OPTION}` does not name `flatbuffers`, which every \
                 export of the `{WASM_OPTION}` module checks"
            ));
        }
        if wasm && !encodings.contains(&WireEncoding::FlatBuffers) {
            return refusal(format!(
                "`[backend.rust] {WASM_OPTION} = true` needs `{ENCODINGS_OPTION}` to name \
                 `flatbuffers`, because the `wasm` module checks FlatBuffers payloads"
            ));
        }
        let Some(model) = &request.model else {
            return refusal("the request carries no model".to_string());
        };
        let preamble = codegen::comment_preamble(&request.generated_marker, &request.header, "//");
        match generate_pipeline_over(model, &encodings, transcode) {
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
/// element, the encoding this backend has no codec for yet (named with the
/// issue that builds it), or a name that is no encoding at all.
fn parse_encoding(name: &str) -> Result<WireEncoding, String> {
    if name.is_empty() {
        return Err(format!(
            "`{ENCODINGS_OPTION}` names an empty element; each element is one of \
             `flatbuffers`, `proto3` and `podbuf`"
        ));
    }
    if let Some(encoding) = WireEncoding::parse(name) {
        return Ok(encoding);
    }
    Err(match name {
        "podbuf" => "the Rust backend has no `podbuf` codec (driftsys/ridl#317)".to_string(),
        other => format!(
            "`{ENCODINGS_OPTION}` names `{other}`, which is not an encoding; the names are \
             `flatbuffers`, `proto3` and `podbuf`"
        ),
    })
}

fn refusal(message: String) -> v1::CodegenResponse {
    v1::CodegenResponse {
        files: Vec::new(),
        diagnostics: vec![codegen::error(message)],
    }
}
