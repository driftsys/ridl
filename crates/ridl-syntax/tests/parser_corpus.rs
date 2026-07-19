//! The parser ok-corpus (docs/ROADMAP.md epic E1.2b, ADR-0007 decision 3).
//!
//! Each `.typl` file under `test_data/parser/ok/` is parsed and its CST is
//! snapshotted together with the error list. Every ok-corpus file must parse
//! losslessly — the tree text reproduces the file byte for byte — and with
//! zero errors. `insta::glob!` names one snapshot per corpus file.

use std::fmt::Write as _;

use ridl_syntax::{Parse, Profile, parse};

/// The review dump: the full CST (nodes, tokens, ranges) plus the errors.
fn dump(parse: &Parse) -> String {
    let mut out = format!("{:#?}", parse.syntax());
    out.push_str("errors:");
    if parse.errors().is_empty() {
        out.push_str(" none\n");
    } else {
        out.push('\n');
        for error in parse.errors() {
            writeln!(out, "  {} {:?} {}", error.code, error.range, error.message)
                .expect("writing to a String");
        }
    }
    out
}

#[test]
fn ok_corpus_is_lossless_error_free_and_matches_snapshots() {
    insta::glob!("../test_data/parser/ok", "*.typl", |path| {
        let input = std::fs::read_to_string(path).expect("a readable corpus file");
        let parsed = parse(&input, Profile::Typl);

        assert_eq!(
            parsed.syntax().text().to_string(),
            input,
            "parse is not lossless for {}",
            path.display(),
        );
        assert!(
            parsed.errors().is_empty(),
            "ok-corpus file {} must parse with zero errors, got: {:?}",
            path.display(),
            parsed.errors(),
        );

        insta::assert_snapshot!(dump(&parsed));
    });
}

/// The ridl half of the ok corpus (epic E2.1a): every `.ridl` file parses
/// under [`Profile::Ridl`] with the same lossless, zero-error contract.
#[test]
fn ridl_ok_corpus_is_lossless_error_free_and_matches_snapshots() {
    insta::glob!("../test_data/parser/ok", "*.ridl", |path| {
        let input = std::fs::read_to_string(path).expect("a readable corpus file");
        let parsed = parse(&input, Profile::Ridl);

        assert_eq!(
            parsed.syntax().text().to_string(),
            input,
            "parse is not lossless for {}",
            path.display(),
        );
        assert!(
            parsed.errors().is_empty(),
            "ok-corpus file {} must parse with zero errors, got: {:?}",
            path.display(),
            parsed.errors(),
        );

        insta::assert_snapshot!(dump(&parsed));
    });
}
