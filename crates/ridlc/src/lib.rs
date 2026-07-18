//! The `ridlc` compile pipeline as a library (docs/ROADMAP.md epic E0.9).
//!
//! [`compile`] runs the walking-skeleton pipeline end to end: it routes the
//! source through the salsa [`parse_file`] query so the incremental query graph
//! is exercised, then resolves names, checks and lowers to IR, and generates
//! Rust source. The function is total: it never panics. Every parser, resolver,
//! and checker diagnostic is collected into [`CompileOutput::diagnostics`] as a
//! message string; if the Rust backend fails, its error joins that list and
//! [`CompileOutput::rust_source`] is left empty. The caller (the CLI or a test)
//! decides what a non-empty diagnostic list means.

use ridl_core::{RidlDatabase, SourceFile as InputFile, check, parse_file, resolve};
use ridl_syntax::ast::{AstNode as _, SourceFile as AstFile};

/// The result of [`compile`]: the generated Rust source, the lowered IR module,
/// and every parser, resolver, and checker diagnostic rendered as a message.
pub struct CompileOutput {
    pub rust_source: String,
    pub module: ridl_ir::Module,
    pub diagnostics: Vec<String>,
}

/// Compiles `text` (named `path` for module-name derivation) end to end.
///
/// The pipeline is `parse_file` (through the salsa database) → `resolve` →
/// `check` → `generate`. Diagnostics are concatenated in that order: parser
/// errors first, then resolver, then checker, then any Rust backend error. The
/// module name is the input path's file stem.
pub fn compile(path: &str, text: &str) -> CompileOutput {
    let db = RidlDatabase::default();
    let input = InputFile::new(&db, path.to_string(), text.to_string());
    let parse = parse_file(&db, input);

    let mut diagnostics: Vec<String> = parse.errors().iter().map(|e| e.message.clone()).collect();

    let ast = AstFile::cast(parse.syntax()).expect("parser roots every tree in a SourceFile");

    let resolution = resolve(&ast);
    diagnostics.extend(resolution.diagnostics.iter().map(|d| d.message.clone()));

    let module_name = module_name_from_path(path);
    let (module, check_errors) = check(&ast, &resolution, &module_name);
    diagnostics.extend(check_errors.iter().map(|e| e.message.clone()));

    let rust_source = match ridl_backend_rust::generate(&module) {
        Ok(source) => source,
        Err(err) => {
            diagnostics.push(err.message);
            String::new()
        }
    };

    CompileOutput {
        rust_source,
        module,
        diagnostics,
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
