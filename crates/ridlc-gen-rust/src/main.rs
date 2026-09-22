//! `ridlc-gen-rust` — the reference codegen plugin over the Rust backend
//! (ADR-0020 decision 10; `docs/design/codegen-plugins.md`).
//!
//! It is the process-host form of `--emit rust`: the request on standard
//! input, in canonical protobuf JSON; the response on standard output, in
//! the same encoding, with one file, `<artifact_base>.rs`. It generates
//! through the same [`Backend`] the in-process host calls, so what the
//! parity test in `tests/parity.rs` compares is the two hosts over one
//! backend — the framing, the two encodings on the pipe, and `ridlc`
//! writing the file.
//!
//! It exists because stage P4 of the lane P driver ported the Rust backend
//! onto the lowered model: before that the backend read the raw IR, a
//! plugin has none, and the exit test of story E4.5b could only be run over
//! `ridlc-gen-model`.
//!
//! What a plugin owes the host, in order, and what this one does:
//!
//! - **Read one document from standard input.** Standard input that is not
//!   a `CodegenRequest` is a message on standard error and exit 1; the host
//!   reports the exit, naming the plugin.
//! - **Refuse a `schema` it does not know** (IR specification §7), with a
//!   diagnostic naming both values — a response, exit 0, because the host
//!   can read it.
//! - **Never touch the filesystem** (ADR-0020 decision 9). This binary opens
//!   no file.
//! - **Write one document to standard output** and exit 0.

use std::io::{Read as _, Write as _};
use std::process::ExitCode;

use ridl_backend_rust::Backend;
use ridl_ir::codegen::{self, Backend as _, v1};

fn main() -> ExitCode {
    let mut input = String::new();
    if let Err(err) = std::io::stdin().read_to_string(&mut input) {
        eprintln!("ridlc-gen-rust: cannot read standard input: {err}");
        return ExitCode::FAILURE;
    }
    let request = match codegen::request_from_json(&input) {
        Ok(request) => request,
        Err(err) => {
            eprintln!(
                "ridlc-gen-rust: standard input is not a `ridl.codegen.v1.CodegenRequest`: {err}"
            );
            return ExitCode::FAILURE;
        }
    };

    let response = if request.schema == codegen::SCHEMA {
        Backend.generate(&request)
    } else {
        v1::CodegenResponse {
            files: Vec::new(),
            diagnostics: vec![codegen::error(format!(
                "ridlc-gen-rust reads `{}` and the request is `{}`",
                codegen::SCHEMA,
                request.schema
            ))],
        }
    };

    let output = match codegen::response_to_json(&response) {
        Ok(output) => output,
        Err(err) => {
            eprintln!("ridlc-gen-rust: cannot render the response: {err}");
            return ExitCode::FAILURE;
        }
    };
    let mut stdout = std::io::stdout().lock();
    if let Err(err) = stdout
        .write_all(output.as_bytes())
        .and_then(|()| stdout.flush())
    {
        eprintln!("ridlc-gen-rust: cannot write standard output: {err}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}
