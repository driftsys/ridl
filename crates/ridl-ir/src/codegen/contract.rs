//! The backend contract — `generate(CodegenRequest) -> CodegenResponse`
//! (ADR-0020 decision 9) — as the in-process host sees it, plus the
//! encodings the process host puts on the pipe and the one rule both hosts
//! apply to a response.
//!
//! The schema is `proto/ridl/codegen/v1/plugin.proto`. The process host
//! itself — spawning `ridlc-gen-<language>`, the timeout, the error that
//! names the plugin — lives in `ridlc`, because it needs `std::process` and
//! this crate builds for `wasm32` with `--no-default-features`.
//!
//! **The transition.** The Rust backend reads the model alone; the other
//! in-tree backends still read the raw IR.
//! The trait is nevertheless the contract's own signature, over the request
//! alone: an in-tree backend that still reads the IR is constructed with a
//! [`RawIr`] it keeps beside the request, and its `generate` reads that
//! rather than `request.model`. So what changes when a backend is ported is
//! how it is built, never how it is called, and a plugin — which has no raw
//! IR — is called exactly as an in-tree backend is. [`ModelBackend`] is the
//! one backend that reads the request alone today, and the reference plugin
//! `ridlc-gen-model` wraps it.

use crate::v2;

use super::v1;

/// The `schema` value every request this toolchain writes carries, and the
/// one value a `ridl.codegen.v1` consumer accepts (IR specification §7).
pub const SCHEMA: &str = "ridl.codegen.v1";

/// A codegen backend: one `generate` over one request, per package.
///
/// The in-process host calls it directly; the process host calls it across
/// a pipe, with [`request_to_json`] and [`response_from_json`] on its side
/// of the pipe and their inverses on the plugin's. Both hosts apply
/// [`check_path`] to every file of the response and write the files
/// themselves; a backend never touches the filesystem (ADR-0020 decision 9).
pub trait Backend {
    /// The `<language>` of `ridlc-gen-<language>`: the name the host reports
    /// the backend under, and the `--emit` value of an in-tree one.
    fn language(&self) -> &str;

    /// Generates for the one package the request carries. Total: a backend
    /// that cannot generate returns a response with an error-severity
    /// diagnostic and no files rather than panicking.
    fn generate(&self, request: &v1::CodegenRequest) -> v1::CodegenResponse;
}

/// The raw IR an in-tree backend still reads while it is not yet ported
/// onto the model — the package and the scope `generate_with` takes today
/// (ADR-0017 decision 1). Held by the backend value, not carried by the
/// request, so that the request stays what the contract says it is: the
/// model and the options, never the raw IR.
///
/// Removed when the last in-tree backend is ported.
#[derive(Clone, Copy)]
pub struct RawIr<'a> {
    /// The package the request's model was lowered from.
    pub package: &'a v2::Package,
    /// Every other package the build handed the backends.
    pub others: &'a [&'a v2::Package],
}

/// The backend behind `--emit codegen-model`: the request's model, written
/// back as canonical protobuf JSON to `<artifact_base>.codegen.json`.
///
/// It reads nothing but the request, which makes it the one backend a
/// plugin can wrap today with no raw IR in hand, and so the reference plugin
/// `ridlc-gen-model` wraps it: the process host's parity test is this backend
/// in process against this backend across the pipe, and what it proves is
/// the host — the framing, the two encodings on the pipe, the file writing
/// — and that the model survives a parse and a re-render byte for byte.
pub struct ModelBackend;

impl Backend for ModelBackend {
    fn language(&self) -> &str {
        "model"
    }

    fn generate(&self, request: &v1::CodegenRequest) -> v1::CodegenResponse {
        if let Some(option) = request.options.first() {
            return v1::CodegenResponse {
                files: Vec::new(),
                diagnostics: vec![error(format!(
                    "the model backend takes no option; `{}` is not one it knows",
                    option.key
                ))],
            };
        }
        let Some(model) = &request.model else {
            return v1::CodegenResponse {
                files: Vec::new(),
                diagnostics: vec![error("the request carries no model".to_string())],
            };
        };
        match super::to_json_pretty(model) {
            Ok(json) => v1::CodegenResponse {
                files: vec![text_file(
                    format!("{}.codegen.json", request.artifact_base),
                    json,
                )],
                diagnostics: Vec::new(),
            },
            Err(err) => v1::CodegenResponse {
                files: Vec::new(),
                diagnostics: vec![error(err.to_string())],
            },
        }
    }
}

/// A generated text file, for a backend building its response.
pub fn text_file(path: String, text: String) -> v1::GeneratedFile {
    v1::GeneratedFile {
        path,
        content: Some(v1::generated_file::Content::Text(text)),
    }
}

/// The text every generated-file marker starts with.
pub const GENERATED_MARKER_PREFIX: &str = "@generated by ridl";

/// The marker line, without a comment token. `None` names no package, for the
/// crate files of a workspace build.
pub fn generated_marker(package: Option<&str>) -> String {
    match package {
        Some(package) => format!("{GENERATED_MARKER_PREFIX} from package {package}. Do not edit."),
        None => format!("{GENERATED_MARKER_PREFIX}. Do not edit."),
    }
}

/// The first character of a header text that a generated file cannot carry:
/// every character for which [`char::is_control`] is true (C0, DEL and C1,
/// U+0085 included) other than a tab, a `\n` and a `\r`, and the line and
/// paragraph separators U+2028 and U+2029, which end a `//` comment in
/// TypeScript. `None` when the text holds none. The workspace loader reports
/// such a header as MANI-011, so [`normalise_header`] only sees text without
/// one.
pub fn header_control_character(text: &str) -> Option<char> {
    text.chars().find(|c| {
        (c.is_control() && !matches!(c, '\t' | '\n' | '\r')) || matches!(c, '\u{2028}' | '\u{2029}')
    })
}

/// Normalises a header text: each of `\r\n`, `\n` and a lone `\r` is one line
/// break, no line keeps trailing whitespace, and the text has no leading or
/// trailing blank line. A whitespace-only line inside the text becomes an
/// empty line. `None` when no text is left. A caller rejects a text with
/// [`header_control_character`] first.
pub fn normalise_header(text: &str) -> Option<String> {
    let text = text.replace("\r\n", "\n").replace('\r', "\n");
    let lines: Vec<&str> = text.lines().map(str::trim_end).collect();
    let first = lines.iter().position(|line| !line.is_empty())?;
    let last = lines.iter().rposition(|line| !line.is_empty())?;
    Some(lines[first..=last].join("\n"))
}

/// The comment block that starts a generated file: the marker, then each
/// header line, each behind `comment`, then one blank line. An empty line is
/// the bare `comment`, so an empty `marker` with a non-empty `header` gives a
/// bare `comment` as line 1. Empty when `marker` and `header` are both empty.
/// `header` is expected to be normalised ([`normalise_header`]): a line break
/// other than `\n` is not handled here.
pub fn comment_preamble(marker: &str, header: &str, comment: &str) -> String {
    if marker.is_empty() && header.is_empty() {
        return String::new();
    }
    let mut out = String::new();
    for line in std::iter::once(marker).chain(header.lines()) {
        if line.is_empty() {
            out.push_str(comment);
        } else {
            out.push_str(comment);
            out.push(' ');
            out.push_str(line);
        }
        out.push('\n');
    }
    out.push('\n');
    out
}

/// An error-severity diagnostic, for a backend building its response.
pub fn error(message: String) -> v1::Diagnostic {
    v1::Diagnostic {
        severity: v1::DiagnosticSeverity::Error as i32,
        message,
    }
}

/// Whether a response carries an error-severity diagnostic — the condition
/// under which a host writes none of its files. An unset severity counts as
/// an error (`plugin.proto`, `DiagnosticSeverity`), as does a value outside
/// the schema, which a lenient reader can carry.
pub fn has_error(response: &v1::CodegenResponse) -> bool {
    response.diagnostics.iter().any(|diagnostic| {
        !matches!(
            v1::DiagnosticSeverity::try_from(diagnostic.severity),
            Ok(v1::DiagnosticSeverity::Warning | v1::DiagnosticSeverity::Info)
        )
    })
}

/// The rule a generated file's path must meet before a host writes it,
/// `plugin.proto`'s own words: `/`-separated components, none empty, none
/// `.` or `..`, no leading `/`, no drive letter, no NUL — and no `\`, which
/// one host would read as a separator and another as a name. Returns the
/// reason the path is refused.
pub fn check_path(path: &str) -> Result<(), String> {
    if path.is_empty() {
        return Err("the path is empty".to_string());
    }
    if path.contains('\0') {
        return Err("the path contains a NUL byte".to_string());
    }
    if path.contains('\\') {
        return Err("the path contains `\\`; components are separated by `/`".to_string());
    }
    if path.starts_with('/') {
        return Err("the path is absolute".to_string());
    }
    if path.as_bytes().get(1).is_some_and(|byte| *byte == b':')
        && path.as_bytes()[0].is_ascii_alphabetic()
    {
        return Err("the path starts with a drive letter".to_string());
    }
    for component in path.split('/') {
        match component {
            "" => return Err("the path has an empty component".to_string()),
            "." | ".." => {
                return Err(format!("the path has a `{component}` component"));
            }
            _ => {}
        }
    }
    Ok(())
}

/// Renders a request as pretty-printed canonical protobuf JSON — the bytes
/// the process host writes to a plugin's standard input. The `model` inside
/// it is [`super::to_json_pretty`]'s output, one indentation level deeper.
pub fn request_to_json(request: &v1::CodegenRequest) -> Result<String, super::SerializeError> {
    v2::render_json(request).map_err(super::SerializeError::from_v2)
}

/// Reads a request from canonical protobuf JSON — what a plugin does with
/// its standard input — under the guards [`super::from_json`] states. The
/// request nests the model one level deeper, so the bound a reader must
/// provision is 265 JSON levels (design note §6.2, plus one).
///
/// A key the reader does not know is ignored, at every nesting level of the
/// request: the top level, inside `model` and `deployment`, and inside the
/// elements of a repeated message field. A plugin built on this crate then
/// keeps reading requests from a later `ridl` that adds a request field (the
/// IR specification's compatibility rule, section 6). An unknown value is not
/// ignored: an enum name the reader does not know is an error, because it
/// changes the meaning of a known field, so a later `ridl` that adds an enum
/// value is a change an older plugin reports as an error, and that plugin must
/// be rebuilt against the `ridl-ir` that adds the value. Everything else the
/// strict reader rejects is still rejected: a value of the wrong type, a field
/// written twice, malformed JSON and input past the nesting cap. A plugin
/// built against a `ridl-ir` release from before this rule must be rebuilt
/// once.
pub fn request_from_json(text: &str) -> Result<v1::CodegenRequest, serde_json::Error> {
    v2::read_json_ignoring_unknown(v2::codegen_request_descriptor(), text)
}

/// Renders a response as pretty-printed canonical protobuf JSON — the bytes
/// a plugin writes to its standard output.
pub fn response_to_json(response: &v1::CodegenResponse) -> Result<String, super::SerializeError> {
    v2::render_json(response).map_err(super::SerializeError::from_v2)
}

/// Reads a response from canonical protobuf JSON — what the process host
/// does with a plugin's standard output — under the same guards as
/// [`request_from_json`].
pub fn response_from_json(text: &str) -> Result<v1::CodegenResponse, serde_json::Error> {
    v2::read_json(text)
}
