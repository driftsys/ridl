//! This backend behind the backend contract (ADR-0020 decision 9): the
//! in-process face `ridlc` calls.
//!
//! **Transition.** The backend reads its doc comments from `request.model`,
//! and everything else from the raw IR: it is ported onto the model in steps,
//! after the Rust backend (`docs/design/codegen-plugins.md`). Until the port
//! is complete the backend value holds a [`RawIr`] as well.

use ridl_ir::codegen::{self, RawIr, v1};

use crate::{GenerateError, generate_with};

/// The TypeScript backend as a [`codegen::Backend`]: one file,
/// `<artifact_base>.ts`, from [`generate`]. It takes no option.
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
        if let Some(option) = request.options.first() {
            return refusal(format!(
                "the typescript backend takes no option; `{}` is not one it knows",
                option.key
            ));
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
