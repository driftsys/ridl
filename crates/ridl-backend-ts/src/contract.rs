//! This backend behind the backend contract (ADR-0020 decision 9): the
//! in-process face `ridlc` calls.
//!
//! **Transition.** The backend reads its doc comments from `request.model`,
//! and everything else from the raw IR: it is ported onto the model in steps,
//! after the Rust backend (`docs/design/codegen-plugins.md`). Until the port
//! is complete the backend value holds a [`RawIr`] as well. The codec file
//! reads the model only.

use ridl_ir::codegen::{self, RawIr, v1};

use crate::{GenerateError, generate_codec, generate_with};

/// The option that asks for the codec file beside the module, as the
/// manifest's `[backend.typescript] codec = true` sets it, with the value
/// `true` or `false`.
pub const CODEC_OPTION: &str = "codec";

/// The TypeScript backend as a [`codegen::Backend`]: the file
/// `<artifact_base>.ts`, the module [`crate::generate`] emits with the doc
/// comments read from the request's model, and, when the option
/// [`CODEC_OPTION`] is `true`, the file `<artifact_base>.codec.ts` that
/// [`crate::generate_codec`] emits.
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
        "typescript"
    }

    fn generate(&self, request: &v1::CodegenRequest) -> v1::CodegenResponse {
        let mut codec = false;
        for option in &request.options {
            if option.key != CODEC_OPTION {
                return refusal(format!(
                    "the typescript backend takes the option `{CODEC_OPTION}`; `{}` is not one \
                     it knows",
                    option.key
                ));
            }
            match option.value.as_str() {
                "true" => codec = true,
                "false" => codec = false,
                other => {
                    return refusal(format!(
                        "`{CODEC_OPTION}` is `{other}`; the value is `true` or `false`"
                    ));
                }
            }
        }
        let preamble = codegen::comment_preamble(&request.generated_marker, &request.header, "//");
        // A request built without a model is lowered here from the raw IR,
        // the same lowering `ridlc` performs for the request.
        let lowered;
        let model = match &request.model {
            Some(model) => model,
            None => {
                lowered = codegen::lower(self.raw.package, self.raw.others);
                &lowered
            }
        };
        let generated = match generate_with(self.raw.package, model) {
            Ok(generated) => generated,
            Err(GenerateError::Unrepresentable(message)) => return refusal(message),
        };
        let mut files = vec![codegen::text_file(
            format!("{}.ts", request.artifact_base),
            format!("{preamble}/* eslint-disable */\n\n{}", generated.source),
        )];
        if codec {
            match generate_codec(model, &request.artifact_base) {
                Ok(codec) => files.push(codegen::text_file(
                    format!("{}.codec.ts", request.artifact_base),
                    format!("{preamble}/* eslint-disable */\n\n{}", codec.source),
                )),
                Err(GenerateError::Unrepresentable(message)) => return refusal(message),
            }
        }
        v1::CodegenResponse {
            files,
            diagnostics: Vec::new(),
        }
    }
}

fn refusal(message: String) -> v1::CodegenResponse {
    v1::CodegenResponse {
        files: Vec::new(),
        diagnostics: vec![codegen::error(message)],
    }
}
