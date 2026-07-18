//! Formatter properties checked over the whole `ridl-syntax` parser `ok`
//! corpus (docs/ROADMAP.md epic E1.14):
//!
//! - **idempotence** — `format(format(x)) == format(x)` for every file;
//! - **node-shape preservation** — the formatted text re-parses to the same
//!   tree of nodes as the original. The formatter normalises whitespace and
//!   drops separator commas, so tokens change; the node structure — the wire
//!   identity — never does.
//!
//! The corpus is reached by a path relative to this crate's manifest, so the
//! two crates stay decoupled at the filesystem level.

use std::fs;
use std::path::{Path, PathBuf};

use ridl_fmt::{FormatOutcome, format};
use ridl_syntax::SyntaxKind;

/// The `ok` parser corpus files, sorted by name.
fn ok_corpus_files() -> Vec<PathBuf> {
    let dir =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../crates/ridl-syntax/test_data/parser/ok");
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
    match format(text) {
        FormatOutcome::Formatted(out) => out,
        FormatOutcome::ParseErrors(errors) => {
            panic!("{context} produced parse errors: {errors:?}")
        }
    }
}

/// The kinds of every node in the tree, in pre-order — the node shape, ignoring
/// all tokens and therefore all trivia and separators.
fn node_shape(text: &str) -> Vec<SyntaxKind> {
    ridl_syntax::parse(text)
        .syntax()
        .descendants()
        .map(|node| node.kind())
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
fn formatting_preserves_node_shape_over_the_ok_corpus() {
    for path in ok_corpus_files() {
        let name = path.file_name().unwrap().to_str().unwrap().to_string();
        let source = fs::read_to_string(&path).expect("read corpus file");
        let formatted = format_ok(&source, &name);
        assert_eq!(
            node_shape(&source),
            node_shape(&formatted),
            "`{name}` changed its node shape when formatted",
        );
    }
}
