//! This backend behind the backend contract (ADR-0020 decision 9): the
//! in-process face `ridlc` calls.
//!
//! **Transition.** [`generate`](crate::generate) still reads the raw IR, and
//! this backend is ported onto the model in its own story, after the Rust
//! backend (`docs/design/codegen-plugins.md`). Until then the backend value
//! holds a [`RawIr`] and `generate` reads that in place of `request.model`.

use ridl_ir::codegen::{self, RawIr, v1};

use crate::{GenerateError, generate};

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
        match generate(self.raw.package) {
            Ok(generated) => v1::CodegenResponse {
                files: vec![codegen::text_file(
                    format!("{}.ts", request.artifact_base),
                    generated.source,
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
