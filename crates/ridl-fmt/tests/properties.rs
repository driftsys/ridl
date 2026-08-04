//! Formatter properties checked over the whole `ridl-syntax` parser `ok`
//! corpus (docs/ROADMAP.md epic E1.14):
//!
//! - **idempotence** — `format(format(x)) == format(x)` for every file;
//! - **content preservation** — the formatted text carries the same content
//!   token stream as the original. The stream is every token except whitespace
//!   and separator commas (the two things the formatter is licensed to
//!   normalise), so it includes identifiers, keywords, literals, punctuation,
//!   **and comments**. Comparing it catches a dropped or renamed identifier, a
//!   mutated literal, and a dropped comment — none of which a node-kind-only
//!   comparison could see. Comment text is compared after trimming trailing
//!   whitespace, which the formatter strips as insignificant.
//!
//! The corpus is reached by a path relative to this crate's manifest, so the
//! two crates stay decoupled at the filesystem level.

use std::fs;
use std::path::{Path, PathBuf};

use ridl_fmt::{FormatOutcome, format};
use ridl_syntax::{Profile, SyntaxKind};

/// The `ok` parser corpus files, sorted by name.
fn ok_corpus_files() -> Vec<PathBuf> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../ridl-syntax/test_data/parser/ok");
    let mut files: Vec<PathBuf> = fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("read_dir {}: {e}", dir.display()))
        .map(|entry| entry.expect("dir entry").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "typl"))
        .collect();
    files.sort();
    assert!(!files.is_empty(), "the parser ok corpus must not be empty");
    files
}

fn format_ok(text: &str, context: &str) -> String {
    match format(text, Profile::Typl) {
        FormatOutcome::Formatted(out) => out,
        FormatOutcome::ParseErrors(errors) => {
            panic!("{context} produced parse errors: {errors:?}")
        }
    }
}

fn is_comment(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::LineComment | SyntaxKind::BlockComment | SyntaxKind::DocComment
    )
}

/// The content token stream: every token in document order except whitespace
/// and separator commas, as `(kind, text)`. Comment text is trimmed of trailing
/// whitespace (insignificant, and stripped by the formatter). This is the
/// invariant the formatter must not disturb — only whitespace and separator
/// commas may change.
fn content_tokens(text: &str) -> Vec<(SyntaxKind, String)> {
    ridl_syntax::parse(text, Profile::Typl)
        .syntax()
        .descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .filter(|token| !matches!(token.kind(), SyntaxKind::Whitespace | SyntaxKind::Comma))
        .map(|token| {
            let text = if is_comment(token.kind()) {
                token.text().trim_end().to_string()
            } else {
                token.text().to_string()
            };
            (token.kind(), text)
        })
        .collect()
}

#[test]
fn formatting_is_idempotent_over_the_ok_corpus() {
    for path in ok_corpus_files() {
        let name = path.file_name().unwrap().to_str().unwrap().to_string();
        let source = fs::read_to_string(&path).expect("read corpus file");
        let once = format_ok(&source, &name);
        let twice = format_ok(&once, &name);
        assert_eq!(once, twice, "`{name}` is not an idempotent format");
    }
}

#[test]
fn formatting_preserves_content_tokens_over_the_ok_corpus() {
    for path in ok_corpus_files() {
        let name = path.file_name().unwrap().to_str().unwrap().to_string();
        let source = fs::read_to_string(&path).expect("read corpus file");
        let formatted = format_ok(&source, &name);
        assert_eq!(
            content_tokens(&source),
            content_tokens(&formatted),
            "`{name}` changed its content tokens when formatted",
        );
    }
}

/// The content-token stream is sensitive to exactly the mutations the corpus
/// test guards against: a dropped comment and a mutated literal both change it.
/// (A bare node-kind comparison would miss both — which is why the comment-drop
/// bug slipped through the earlier node-shape test.)
#[test]
fn content_tokens_detect_a_dropped_comment_and_a_mutated_literal() {
    // Dropping the in-constraint comment changes the stream — so had the
    // formatter still dropped it,
    // `formatting_preserves_content_tokens_over_the_ok_corpus` would fail on
    // such an input.
    let with_comment = content_tokens("package p\ntype F: bytes [/* fixed */ 8]\n");
    let without_comment = content_tokens("package p\ntype F: bytes [8]\n");
    assert_ne!(with_comment, without_comment, "a dropped comment must show");

    // Mutating a literal changes the stream too.
    let five = content_tokens("package p\nconst X: integer = 5\n");
    let six = content_tokens("package p\nconst X: integer = 6\n");
    assert_ne!(five, six, "a mutated literal must show");

    // Whitespace and separator commas do not — those the formatter may change.
    let spaced = content_tokens("package p\nenum E { A = 0, B = 1 }\n");
    let newlined = content_tokens("package p\nenum E {\n  A = 0\n  B = 1\n}\n");
    assert_eq!(spaced, newlined, "whitespace and commas must be ignored");
}
