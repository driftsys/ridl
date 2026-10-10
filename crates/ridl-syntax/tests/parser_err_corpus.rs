//! The parser err-corpus (ADR-0007 decision 3).
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

/// The ridl half of the err corpus: broken `.ridl` input parsed
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

/// The rsdl half of the err corpus: broken `.rsdl` input parsed under
/// [`Profile::Rsdl`], with the same lossless, at-least-one-diagnostic recovery
/// contract.
#[test]
fn rsdl_err_corpus_is_lossless_reports_errors_and_matches_snapshots() {
    insta::glob!("../test_data/parser/err", "*.rsdl", |path| {
        let input = std::fs::read_to_string(path).expect("a readable corpus file");
        let parsed = parse(&input, Profile::Rsdl);

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

/// The bare `= value` init parses only before the timing (ADR-0008
/// decision 2): on `signal`, `event` and `fixed`, an init after a timing,
/// or after an attribute block, is a parse error reported on the `=`,
/// while the same members with the init first parse cleanly.
#[test]
fn ridl_init_value_parses_only_before_the_annotations() {
    let wrap = |member: &str| format!("package p\n\ninterface I {{\n  {member}\n}}\n");
    for kw in ["signal", "event", "fixed"] {
        for tail in [
            "= 1 @10ms",
            "= 1 [require true]",
            "= 1 @10ms [require true]",
        ] {
            let member = format!("{kw} s : Speed {tail}");
            let parsed = parse(&wrap(&member), Profile::Ridl);
            assert!(
                parsed.errors().is_empty(),
                "`{member}` must parse cleanly: {:?}",
                parsed.errors(),
            );
        }
        for tail in [
            "@10ms = 1",
            "[require true] = 1",
            "@10ms [require true] = 1",
        ] {
            let member = format!("{kw} s : Speed {tail}");
            let source = wrap(&member);
            let parsed = parse(&source, Profile::Ridl);
            let equals = source.find(" = 1").expect("the init in the source") + 1;
            assert!(
                parsed
                    .errors()
                    .iter()
                    .any(|error| usize::from(error.range.start()) == equals),
                "`{member}` must draw an error on its `=`: {:?}",
                parsed.errors(),
            );
        }
    }
}

/// `during` is a reserved keyword with no grammar yet: a `during` clause on
/// an interface member does not parse cleanly.
#[test]
fn ridl_during_on_an_interface_member_is_rejected() {
    let source = "package p\n\ninterface I {\n  signal s : Speed during READY\n}\n";
    let parsed = parse(source, Profile::Ridl);
    assert!(
        !parsed.errors().is_empty(),
        "`during` must not parse cleanly"
    );
}
