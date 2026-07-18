//! The lexer corpus (docs/ROADMAP.md epic E1.1, ADR-0007 decision 3).
//!
//! Each `.typl` file under `test_data/lexer/` is lexed, its token stream is
//! snapshotted, and totality is asserted — concatenating every token's text
//! must reproduce the file byte for byte. `insta::glob!` names one snapshot per
//! corpus file.

use std::fmt::Write as _;

use ridl_syntax::{Token, lex};

/// One line per token: its kind and its exact source slice (escaped).
fn dump(tokens: &[Token<'_>]) -> String {
    let mut out = String::new();
    for token in tokens {
        writeln!(out, "{:?} {:?}", token.kind, token.text).expect("writing to a String");
    }
    out
}

#[test]
fn lexer_corpus_round_trips_and_matches_snapshots() {
    insta::glob!("../test_data/lexer", "*.typl", |path| {
        let input = std::fs::read_to_string(path).expect("a readable corpus file");
        let tokens = lex(&input);

        let round_trip: String = tokens.iter().map(|token| token.text).collect();
        assert_eq!(
            round_trip,
            input,
            "lexer is not total for {}",
            path.display(),
        );

        insta::assert_snapshot!(dump(&tokens));
    });
}
