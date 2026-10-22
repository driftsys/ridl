//! This backend behind the backend contract (ADR-0020 decision 9): the
//! in-process face `ridlc` calls.
//!
//! **Transition.** The backend reads its doc comments from `request.model`,
//! and everything else from the raw IR: it is ported onto the model in steps,
//! after the Rust backend (`docs/design/codegen-plugins.md`). Until the port
//! is complete the backend value holds a [`RawIr`] as well.

use ridl_ir::codegen::{self, RawIr, v1};

use crate::{GenerateError, generate_with};

/// The option that asks for the codec file beside the module, as the
/// manifest's `[backend.typescript] codec = true` sets it, with the value
/// `true` or `false`.
pub const CODEC_OPTION: &str = "codec";

/// The TypeScript backend as a [`codegen::Backend`]: one file,
/// `<artifact_base>.ts`, the module [`crate::generate`] emits with the doc
/// comments read from the request's model. It takes the option
/// [`CODEC_OPTION`].
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
        // The flag is read and checked here; the codec file is not written
        // yet, so the response is the same with either value.
        let _ = codec;
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
        match generate_with(self.raw.package, model) {
            Ok(generated) => v1::CodegenResponse {
                files: vec![codegen::text_file(
                    format!("{}.ts", request.artifact_base),
                    format!("{preamble}/* eslint-disable */\n\n{}", generated.source),
                )],
                diagnostics: Vec::new(),
            },
            Err(GenerateError::Unrepresentable(message)) => refusal(message),
        }
    }
}

fn refusal(message: String) -> v1::CodegenResponse {
    v1::CodegenResponse {
        files: Vec::new(),
        diagnostics: vec![codegen::error(message)],
    }
}
