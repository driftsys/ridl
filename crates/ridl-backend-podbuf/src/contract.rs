//! This backend behind the backend contract (ADR-0020 decision 9): the
//! in-process face `ridlc` calls.
//!
//! The backend value holds a [`RawIr`] because the size bound that refuses a
//! type too large for a `u32` offset is computed from the IR declarations.
//! The head layout itself is read from the request's model when it carries
//! one, so a header is never laid out twice.

use ridl_ir::codegen::{self, RawIr, v1};

use crate::{generate_model, generate_with};

/// The podbuf backend as a [`codegen::Backend`]: one file, `<artifact_base>.h`,
/// from [`generate_with`]. It takes no option.
pub struct Backend<'a> {
    raw: RawIr<'a>,
}

impl<'a> Backend<'a> {
    /// A backend over the raw IR it reads for the size bound.
    pub fn new(raw: RawIr<'a>) -> Self {
        Self { raw }
    }
}

impl codegen::Backend for Backend<'_> {
    fn language(&self) -> &str {
        "podbuf"
    }

    fn generate(&self, request: &v1::CodegenRequest) -> v1::CodegenResponse {
        if let Some(option) = request.options.first() {
            return refusal(format!(
                "the podbuf backend takes no option; `{}` is not one it knows",
                option.key
            ));
        }
        let preamble = codegen::comment_preamble(&request.generated_marker, &request.header, "//");
        let generated = match request.model.as_ref() {
            Some(model) => generate_model(self.raw.package, self.raw.others, model),
            None => generate_with(self.raw.package, self.raw.others),
        };
        match generated {
            Ok(generated) => v1::CodegenResponse {
                files: vec![codegen::text_file(
                    format!("{}.h", request.artifact_base),
                    format!("{preamble}{}", generated.header_source),
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
