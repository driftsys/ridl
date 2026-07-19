//! The parser err-corpus (docs/ROADMAP.md epic E1.2c, ADR-0007 decision 3).
//!
//! Each `.typl` file under `test_data/parser/err/` is broken input the parser
//! must recover from. Every err-corpus file parses losslessly — the tree text
//! reproduces the file byte for byte — and produces at least one diagnostic.
//! The snapshot pins the recovery contract: which tokens the parser wrapped in
//! `ErrorNode`s, which real declaration nodes it still produced after the
//! garbage, and the coded diagnostic list. `insta::glob!` names one snapshot
//! per corpus file. Review each snapshot before accepting: the recovery shape
//! is the review artifact.

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
fn err_corpus_is_lossless_reports_errors_and_matches_snapshots() {
    insta::glob!("../test_data/parser/err", "*.typl", |path| {
        let input = std::fs::read_to_string(path).expect("a readable corpus file");
        let parsed = parse(&input, Profile::Typl);

        assert_eq!(
            parsed.syntax().text().to_string(),
            input,
            "recovery is not lossless for {}",
            path.display(),
        );
        assert!(
            !parsed.errors().is_empty(),
            "err-corpus file {} must report at least one diagnostic",
            path.display(),
        );

        insta::assert_snapshot!(dump(&parsed));
    });
}

/// The ridl half of the err corpus (epic E2.1a): broken `.ridl` input parsed
/// under [`Profile::Ridl`], with the same lossless, at-least-one-diagnostic
/// recovery contract.
#[test]
fn ridl_err_corpus_is_lossless_reports_errors_and_matches_snapshots() {
    insta::glob!("../test_data/parser/err", "*.ridl", |path| {
        let input = std::fs::read_to_string(path).expect("a readable corpus file");
        let parsed = parse(&input, Profile::Ridl);

        assert_eq!(
            parsed.syntax().text().to_string(),
            input,
            "recovery is not lossless for {}",
            path.display(),
        );
        assert!(
            !parsed.errors().is_empty(),
            "err-corpus file {} must report at least one diagnostic",
            path.display(),
        );

        insta::assert_snapshot!(dump(&parsed));
    });
}
