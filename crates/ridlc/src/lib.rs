//! The `ridlc` compile pipeline as a library (docs/ROADMAP.md epics E0.9,
//! E1.10, E1.7a).
//!
//! [`compile`] runs the pipeline end to end over the package model: it wraps
//! the source in a single-file synthetic package, resolves it
//! ([`resolve_package`]), checks and lowers it to IR v1 ([`check_package`]),
//! and generates Rust source. The function is total: it never panics. Every
//! parser, resolver, and checker diagnostic is a coded [`Diagnostic`]
//! collected into [`CompileOutput::diagnostics`]; if the Rust backend fails,
//! its error joins that list and [`CompileOutput::rust_source`] is left
//! empty. The caller (the CLI or a test) renders the diagnostics against
//! [`CompileOutput::sources`] and decides what a non-empty diagnostic list
//! means.

use std::collections::BTreeMap;

use ridl_core::db::InputFile;
use ridl_core::diag::{DiagCode, Diagnostic, FileId, Severity, SourceMap, Span, remap_diagnostics};
use ridl_core::package::{Package, PackageOrigin, Workspace};
use ridl_core::{RidlDatabase, parse_file, std_package};
use ridl_sem::{check_package, resolve_package};
use ridl_syntax::ast::{AstNode as _, SourceFile};
use rowan::TextRange;

/// The result of [`compile`]: the generated Rust source, the lowered IR v1
/// package, every coded diagnostic, and the source map the diagnostics point
/// into (for rendering).
pub struct CompileOutput {
    pub rust_source: String,
    pub package: ridl_ir::v1::Package,
    pub diagnostics: Vec<Diagnostic>,
    pub sources: SourceMap,
}

/// Compiles `text` (registered under `path`) end to end.
///
/// The pipeline is `parse_file` (through the salsa database) →
/// `resolve_package` → `check_package` → `generate`. Diagnostics are
/// concatenated in that order: parser errors first, then resolver, then
/// checker, then any Rust backend error. The source becomes a single-file
/// synthetic package named from its `package` declaration, falling back to
/// the path's file stem — the loader's single-file rule (E1.3).
///
/// The package-scoped passes stamp their spans with a [`FileId`] indexing the
/// package's files in order; [`remap_diagnostics`] rewrites them onto this
/// function's own [`SourceMap`] before they are merged.
pub fn compile(path: &str, text: &str) -> CompileOutput {
    let mut db = RidlDatabase::default();
    let std = std_package(&mut db);
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
                ridl_core::diag::house_style_message(&error.message),
                file,
                error.range,
            )
        })
        .collect();

    let ast = SourceFile::cast(parse.syntax()).expect("parser roots every tree in a SourceFile");
    let package_name = declared_package_name(&ast).unwrap_or_else(|| module_name_from_path(path));
    let pkg = Package::new(
        &db,
        package_name,
        vec![input],
        PackageOrigin::WorkspaceMember,
        BTreeMap::new(),
    );
    let ws = Workspace::new(&db, vec![pkg], BTreeMap::new());

    let resolution = resolve_package(&db, ws, pkg, std);
    let checked = check_package(&db, ws, pkg, std);

    // The single package file is the file interned above.
    let render_ids = vec![file];
    diagnostics.extend(remap_diagnostics(resolution.diagnostics, &render_ids));
    diagnostics.extend(remap_diagnostics(checked.diagnostics, &render_ids));

    let rust_source = match ridl_backend_rust::generate(&checked.ir) {
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
        package: checked.ir,
        diagnostics,
        sources,
    }
}

/// The dotted name of the file's `package` declaration, with trivia between
/// its tokens dropped; `None` when no declaration parses (the parser already
/// reported FORM-104).
fn declared_package_name(ast: &SourceFile) -> Option<String> {
    let name = ast.package_decl()?.qualified_name()?;
    let text: String = name
        .syntax()
        .descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .filter(|token| !token.kind().is_trivia())
        .map(|token| token.text().to_string())
        .collect();
    (!text.is_empty()).then_some(text)
}

/// Builds an error-severity [`Diagnostic`] from a pass's `code`, `message`,
/// and source `range`.
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
