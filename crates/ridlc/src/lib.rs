//! The `ridlc` compile pipeline as a library (docs/ROADMAP.md epic E0.9,
//! E1.10).
//!
//! [`compile`] runs the walking-skeleton pipeline end to end: it routes the
//! source through the salsa [`parse_file`] query so the incremental query graph
//! is exercised, then resolves names, checks and lowers to IR, and generates
//! Rust source. The function is total: it never panics. Every parser, resolver,
//! and checker diagnostic is mapped into a coded [`Diagnostic`] and collected
//! into [`CompileOutput::diagnostics`], reclaiming the source offsets each pass
//! records; if the Rust backend fails, its error joins that list and
//! [`CompileOutput::rust_source`] is left empty. The caller (the CLI or a test)
//! renders the diagnostics against [`CompileOutput::sources`] and decides what a
//! non-empty diagnostic list means.

use ridl_core::db::InputFile;
use ridl_core::diag::{
    DiagCode, Diagnostic, FileId, Severity, SourceMap, Span, house_style_message,
};
use ridl_core::{RidlDatabase, parse_file};
use ridl_sem::{check, resolve};
use ridl_syntax::ast::{AstNode as _, SourceFile};
use rowan::TextRange;

/// The result of [`compile`]: the generated Rust source, the lowered IR module,
/// every coded diagnostic, and the source map the diagnostics point into (for
/// rendering).
pub struct CompileOutput {
    pub rust_source: String,
    pub module: ridl_ir::Module,
    pub diagnostics: Vec<Diagnostic>,
    pub sources: SourceMap,
}

/// Compiles `text` (named `path` for module-name derivation) end to end.
///
/// The pipeline is `parse_file` (through the salsa database) → `resolve` →
/// `check` → `generate`. Diagnostics are concatenated in that order: parser
/// errors first, then resolver, then checker, then any Rust backend error. Each
/// pass tags its diagnostic with a stable code and a source range; this function
/// wraps them in the coded [`Diagnostic`] model against a single interned file.
/// The module name is the input path's file stem.
pub fn compile(path: &str, text: &str) -> CompileOutput {
    let db = RidlDatabase::default();
    let input = InputFile::new(&db, path.to_string(), text.to_string());
    let parse = parse_file(&db, input);

    let mut sources = SourceMap::new();
    let file = sources.file_id(path, text);

    // Parser diagnostics carry the raw message; `house_style_message` polishes
    // the `expect`-path Debug forms into the backticked house style (issue #102).
    let mut diagnostics: Vec<Diagnostic> = parse
        .errors()
        .iter()
        .map(|error| {
            error_diagnostic(
                error.code,
                house_style_message(&error.message),
                file,
                error.range,
            )
        })
        .collect();

    let ast = SourceFile::cast(parse.syntax()).expect("parser roots every tree in a SourceFile");

    let resolution = resolve(&ast);
    diagnostics.extend(
        resolution
            .diagnostics
            .iter()
            .map(|error| error_diagnostic(error.code, error.message.clone(), file, error.range)),
    );

    let module_name = module_name_from_path(path);
    let (module, check_errors) = check(&ast, &resolution, &module_name);
    diagnostics.extend(
        check_errors
            .iter()
            .map(|error| error_diagnostic(error.code, error.message.clone(), file, error.range)),
    );

    let rust_source = match ridl_backend_rust::generate(&module) {
        Ok(source) => source,
        Err(err) => {
            // The backend does not carry source ranges yet, so its diagnostic
            // has no code and points at the file start.
            diagnostics.push(error_diagnostic(
                "",
                err.message,
                file,
                TextRange::default(),
            ));
            String::new()
        }
    };

    CompileOutput {
        rust_source,
        module,
        diagnostics,
        sources,
    }
}

/// Builds an error-severity [`Diagnostic`] from a pass's `code`, `message`, and
/// source `range`. Every diagnostic the E1.10 pipeline emits is an error;
/// warnings and info arrive with later passes.
fn error_diagnostic(
    code: &'static str,
    message: String,
    file: FileId,
    range: TextRange,
) -> Diagnostic {
    Diagnostic {
        code: DiagCode(code),
        severity: Severity::Error,
        message,
        primary: Span { file, range },
        labels: Vec::new(),
        fixits: Vec::new(),
    }
}

/// Derives a module name from the input path's file stem, e.g.
/// `walking_skeleton.typl` becomes `walking_skeleton`; falls back to `module`.
pub fn module_name_from_path(path: &str) -> String {
    std::path::Path::new(path)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("module")
        .to_string()
}
