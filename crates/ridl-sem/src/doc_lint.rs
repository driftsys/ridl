//! The doc lints (ADR-0025): checks over the doc comments of a file.
//!
//! Two entry points share the per-file checks: [`lint_package`] runs inside
//! `check_package` over the package's `.typl` and `.ridl` files, and
//! [`lint_rsdl_file`] runs inside `check_system` over each `.rsdl` file. A
//! `.rsdl` file is also a package file, so [`lint_package`] skips it and the
//! file is linted once.
//!
//! The checks so far:
//!
//! - TYPL-410 `doc-comment-style`: a doc comment written as `/** */`. The lint
//!   is `allow` by default (ADR-0024 decision 1), so a project opts in to
//!   requiring `///` doc comments.

use ridl_core::db::{InputFile, profile_of_path};
use ridl_core::diag::{DiagCode, Diagnostic, FileId, FixIt, Severity, Span};
use ridl_syntax::ast::{AstNode, SourceFile};
use ridl_syntax::{Profile, SyntaxKind, SyntaxToken};

use crate::check::Checker;
use crate::resolve::source_file;

/// Runs the doc lints over every `.typl` and `.ridl` file of the package and
/// adds their diagnostics to `checker`.
pub(crate) fn lint_package(checker: &mut Checker<'_>, files: &[InputFile]) {
    for (index, file) in files.iter().enumerate() {
        if profile_of_path(file.path(checker.db)) == Profile::Rsdl {
            continue;
        }
        let source = source_file(checker.db, *file);
        let diagnostics = lint_file(&source, checker.file_ids[index]);
        checker.diagnostics.extend(diagnostics);
    }
}

/// Runs the doc lints over one `.rsdl` file, whose diagnostics carry
/// `file_id`.
pub(crate) fn lint_rsdl_file(file: &SourceFile, file_id: FileId) -> Vec<Diagnostic> {
    lint_file(file, file_id)
}

/// The doc lints of one file.
fn lint_file(file: &SourceFile, file_id: FileId) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    for token in file
        .syntax()
        .descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .filter(|token| token.kind() == SyntaxKind::DocComment)
    {
        if let Some(diagnostic) = doc_comment_style(&token, file_id) {
            diagnostics.push(diagnostic);
        }
    }
    diagnostics
}

/// TYPL-410: a doc comment written as `/** */`, with one fix-it that rewrites
/// it as `///` lines.
fn doc_comment_style(token: &SyntaxToken, file_id: FileId) -> Option<Diagnostic> {
    let text = token.text();
    // An unterminated `/**` is FORM-004 and is not a `DocComment` token, so
    // both delimiters are present here; the check keeps the slice safe.
    let body = text.strip_prefix("/**")?.strip_suffix("*/")?;
    let span = Span {
        file: file_id,
        range: token.text_range(),
    };
    Some(Diagnostic {
        code: DiagCode::TYPL_410,
        severity: Severity::Warning,
        message: "doc comment written as `/** */`; the house style is `///`".to_string(),
        primary: span,
        labels: Vec::new(),
        fixits: vec![FixIt {
            span,
            replacement: line_doc_replacement(body, &indentation_before(token)),
            label: "rewrite as `///` lines".to_string(),
        }],
    })
}

/// The `///` lines that replace a `/** */` comment whose text between the
/// delimiters is `body`. On each line, leading whitespace and then one `*`
/// and one space after it are removed, and trailing whitespace is removed.
/// An empty first or last line is dropped. The lines after the first start
/// with `indent`, the indentation of the comment itself, because the
/// replacement starts at the comment's first character. The line ending is
/// `\r\n` when the comment uses it.
fn line_doc_replacement(body: &str, indent: &str) -> String {
    let newline = if body.contains("\r\n") { "\r\n" } else { "\n" };
    let mut lines: Vec<&str> = body
        .split('\n')
        .map(|line| {
            let line = line.trim_start();
            let line = match line.strip_prefix('*') {
                Some(rest) => rest.strip_prefix(' ').unwrap_or(rest),
                None => line,
            };
            line.trim_end()
        })
        .collect();
    if lines.len() > 1 && lines.last().is_some_and(|line| line.is_empty()) {
        lines.pop();
    }
    if lines.len() > 1 && lines.first().is_some_and(|line| line.is_empty()) {
        lines.remove(0);
    }
    let separator = format!("{newline}{indent}");
    lines
        .iter()
        .map(|line| {
            if line.is_empty() {
                "///".to_string()
            } else {
                format!("/// {line}")
            }
        })
        .collect::<Vec<_>>()
        .join(&separator)
}

/// The spaces and tabs between the start of the token's line and the token.
/// Empty when anything other than whitespace precedes the token on its line.
fn indentation_before(token: &SyntaxToken) -> String {
    let Some(previous) = token.prev_token() else {
        return String::new();
    };
    if previous.kind() != SyntaxKind::Whitespace {
        return String::new();
    }
    let text = previous.text();
    match text.rfind('\n') {
        Some(index) => text[index + 1..].to_string(),
        // A whitespace token with no line break is the indentation only when
        // it starts the file or follows a line break.
        None => match previous.prev_token() {
            None => text.to_string(),
            Some(before) if before.text().ends_with('\n') => text.to_string(),
            Some(_) => String::new(),
        },
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use ridl_core::db::RidlDatabase;
    use ridl_core::package::{Package, PackageOrigin, Workspace};
    use ridl_core::std_lib::std_package;

    use super::*;
    use crate::check::{CheckedPackage, check_package};

    /// Checks a one-file package `demo` whose file is `path`.
    fn check_source(path: &str, text: &str) -> CheckedPackage {
        let mut db = RidlDatabase::default();
        let std = std_package(&mut db);
        let file = InputFile::new(&db, path.to_string(), text.to_string());
        let pkg = Package::new(
            &db,
            "demo".to_string(),
            vec![file],
            PackageOrigin::WorkspaceMember,
            BTreeMap::new(),
            None,
            None,
        );
        let ws = Workspace::new(&db, vec![pkg], BTreeMap::new());
        check_package(&db, ws, pkg, std)
    }

    fn typl_410(checked: &CheckedPackage) -> Vec<&Diagnostic> {
        checked
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == DiagCode::TYPL_410)
            .collect()
    }

    #[test]
    fn doc_comment_style_flags_block_doc() {
        let text = "package demo\n\n/** A speed. */\ntype S: integer [0..300]\n";
        let checked = check_source("demo.typl", text);
        let found = typl_410(&checked);
        assert_eq!(found.len(), 1, "{:?}", checked.diagnostics);
        let diagnostic = found[0];
        assert_eq!(diagnostic.severity, Severity::Warning);
        let start = text.find("/**").expect("the fixture has a block doc");
        let range = diagnostic.primary.range;
        assert_eq!(
            &text[usize::from(range.start())..usize::from(range.end())],
            "/** A speed. */"
        );
        assert_eq!(usize::from(range.start()), start);
        assert_eq!(diagnostic.fixits.len(), 1);
        assert_eq!(diagnostic.fixits[0].span, diagnostic.primary);
        assert_eq!(diagnostic.fixits[0].replacement, "/// A speed.");
    }

    #[test]
    fn doc_comment_style_accepts_line_doc() {
        let checked = check_source(
            "demo.typl",
            "package demo\n\n/// A speed.\ntype S: integer [0..300]\n",
        );
        assert_eq!(checked.diagnostics, Vec::new());
    }

    #[test]
    fn doc_comment_style_multiline_fixit() {
        let text = "package demo\n\ninterface Gauge {\n    /**\n     * The level.\n     *\n     \
                    *     indented\n     */\n    signal level: integer [0..10] @[..10ms]\n}\n";
        let checked = check_source("demo.ridl", text);
        let found = typl_410(&checked);
        assert_eq!(found.len(), 1, "{:?}", checked.diagnostics);
        assert_eq!(
            found[0].fixits[0].replacement,
            "/// The level.\n    ///\n    ///     indented"
        );
    }

    #[test]
    fn doc_comment_style_fixit_without_decoration_and_with_crlf() {
        let text =
            "package demo\r\n\r\n/** First.\r\n    Second. */\r\ntype S: integer [0..300]\r\n";
        let checked = check_source("demo.typl", text);
        let found = typl_410(&checked);
        assert_eq!(found.len(), 1, "{:?}", checked.diagnostics);
        assert_eq!(found[0].fixits[0].replacement, "/// First.\r\n/// Second.");
    }

    /// `check_package` leaves a `.rsdl` file to `check_system`, so its doc
    /// comments are not linted twice.
    #[test]
    fn lint_package_skips_rsdl_files() {
        let checked = check_source(
            "demo.rsdl",
            "package demo\n\n/** A system. */\nsystem S {}\n",
        );
        assert_eq!(typl_410(&checked), Vec::<&Diagnostic>::new());
    }

    #[test]
    fn lint_rsdl_file_flags_block_doc() {
        let text = "package demo\n\n/** A system. */\nsystem S {\n}\n";
        let parse = ridl_syntax::parse(text, Profile::Rsdl);
        let file = SourceFile::cast(parse.syntax()).expect("the root is a SourceFile");
        let found = lint_rsdl_file(&file, FileId::DETACHED);
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].code, DiagCode::TYPL_410);
        assert_eq!(found[0].fixits[0].replacement, "/// A system.");
    }
}
