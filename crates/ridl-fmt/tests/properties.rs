//! Formatter properties checked over the whole `ridl-syntax` parser `ok`
//! corpus:
//!
//! - **idempotence** — `format(format(x)) == format(x)` for every file;
//! - **structure preservation** — node entry and exit and non-trivia tokens;
//! - **totality** — each error-corpus input is left unformatted;
//! - **content preservation** — the formatted text carries the same content
//!   token stream as the original. The stream is every token except whitespace
//!   and separator commas, after the D-4 Timing/AttrBlock pair normalization, so it includes identifiers, keywords, literals, punctuation,
//!   **and comments**. Comparing it catches a dropped or renamed identifier, a
//!   mutated literal, and a dropped comment — none of which a node-kind-only
//!   comparison could see. Comment text is compared after trimming trailing
//!   whitespace, which the formatter strips as insignificant. The lint
//!   directives are compared in their own order, after the other tokens,
//!   because a directive on the first line of a declaration stays on that
//!   line when the declaration is broken over several lines (ADR-0024
//!   decision 2); every other comment keeps its place, so a plain comment
//!   moved to another declaration is a failure.
//!
//! The corpus is reached by a path relative to this crate's manifest, so the
//! two crates stay decoupled at the filesystem level.

use std::fs;
use std::path::{Path, PathBuf};

use ridl_fmt::{FormatOptions, FormatOutcome, format};
use ridl_syntax::Profile;

#[path = "support/invariants.rs"]
mod invariants;
use invariants::{content_tokens, syntax_structure};

/// The parser corpus files for the three implemented profiles, sorted by name.
fn corpus_files(sub: &str) -> Vec<PathBuf> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../ridl-syntax/test_data/parser")
        .join(sub);
    let mut files: Vec<PathBuf> = fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("read_dir {}: {e}", dir.display()))
        .map(|entry| entry.expect("dir entry").path())
        .filter(|path| profile_of_path(path).is_some())
        .collect();
    files.sort();
    assert!(
        !files.is_empty(),
        "the parser {sub} corpus must not be empty"
    );
    files
}

fn format_ok(text: &str, profile: Profile, options: &FormatOptions, context: &str) -> String {
    match format(text, profile, options) {
        FormatOutcome::Formatted(out) => out,
        FormatOutcome::ParseErrors(errors) => {
            panic!("{context} produced parse errors: {errors:?}")
        }
    }
}

/// Each corpus extension selects its language profile.
fn profile_of_path(path: &Path) -> Option<Profile> {
    match path.extension()?.to_str()? {
        "typl" => Some(Profile::Typl),
        "ridl" => Some(Profile::Ridl),
        "rsdl" => Some(Profile::Rsdl),
        _ => None,
    }
}

#[test]
fn formatting_is_idempotent_over_the_ok_corpus() {
    for path in corpus_files("ok") {
        let name = path.file_name().unwrap().to_str().unwrap();
        let profile = profile_of_path(&path).unwrap();
        let source = fs::read_to_string(&path).expect("read corpus file");
        for width in [100, 60, 40] {
            let options = FormatOptions {
                max_line_length: Some(width),
            };
            let once = format_ok(&source, profile, &options, name);
            let twice = format_ok(&once, profile, &options, name);
            assert_eq!(once, twice, "`{name}` is not idempotent at width {width}");
            assert_eq!(
                syntax_structure(&source, profile),
                syntax_structure(&once, profile),
                "`{name}` changed structure at width {width}"
            );
        }
    }
}

/// Sources with a plain `//` comment after a declaration the formatter
/// breaks, after a member on the first line of one, and after the second
/// member of a line: the comment keeps its place among the other tokens.
const TRAILING_COMMENT_SHAPES: &[(&str, Profile)] = &[
    (
        "package p\nstruct S { pair: (a: A, b: B) // pair detail\n  t: (x: X, y: Y, z: Z) // t\n}\n",
        Profile::Typl,
    ),
    (
        "package p\nstruct S {\n  a: (x: A,\n    y: B) // note\n  b: B, c: C // bc\n  d: D /* x\n  y */ e: E, f: F // ef\n}\n",
        Profile::Typl,
    ),
    (
        "package p\ninterface I {\n  query getMinMax(window: Duration): (min: Speed, max: Speed) // mm\n  command c(x: u32) [require x > 0, // attrs\n    ensure x > 1]\n}\n",
        Profile::Ridl,
    ),
    (
        "package p\nservice veh.body.composite: DoorControl, MotorControl // shapes\n",
        Profile::Ridl,
    ),
    (
        "package p\ncomponent Backend [ external ] { // c\n  requires X\n}\ndeployment D for S {\n  machine A { First } // note\n  machine B { Second }\n}\n",
        Profile::Rsdl,
    ),
];

#[test]
fn formatting_preserves_content_tokens_over_trailing_comment_shapes() {
    for (i, (source, profile)) in TRAILING_COMMENT_SHAPES.iter().enumerate() {
        for width in [100, 40, 20] {
            let options = FormatOptions {
                max_line_length: Some(width),
            };
            let name = format!("shape {i}");
            let formatted = format_ok(source, *profile, &options, &name);
            assert_eq!(
                content_tokens(source, *profile),
                content_tokens(&formatted, *profile),
                "`{name}` changed content tokens at width {width}:\n{formatted}"
            );
        }
    }
}

#[test]
fn formatting_preserves_content_tokens_over_the_ok_corpus() {
    for path in corpus_files("ok") {
        let name = path.file_name().unwrap().to_str().unwrap();
        let profile = profile_of_path(&path).unwrap();
        let source = fs::read_to_string(&path).expect("read corpus file");
        for width in [100, 60, 40] {
            let options = FormatOptions {
                max_line_length: Some(width),
            };
            let formatted = format_ok(&source, profile, &options, name);
            assert_eq!(
                content_tokens(&source, profile),
                content_tokens(&formatted, profile),
                "`{name}` changed content tokens at width {width}"
            );
        }
    }
}

#[test]
fn every_error_corpus_file_is_left_unformatted() {
    for path in corpus_files("err") {
        let source = fs::read_to_string(&path).expect("read corpus file");
        let profile = profile_of_path(&path).unwrap();
        assert!(
            matches!(
                format(&source, profile, &FormatOptions::default()),
                FormatOutcome::ParseErrors(_)
            ),
            "{} must yield parse errors",
            path.display()
        );
    }
}

/// The content-token stream is sensitive to exactly the mutations the corpus
/// test guards against: a dropped comment and a mutated literal both change it.
/// (A bare node-kind comparison would miss both — which is why the comment-drop
/// bug slipped through the earlier node-shape test.)
#[test]
fn content_tokens_detect_a_dropped_comment_and_a_mutated_literal() {
    let members = "package p\nstruct S { a: A, b: B }\n";
    let removed = "package p\nstruct S { a: A }\n";
    assert_ne!(
        syntax_structure(members, Profile::Typl),
        syntax_structure(removed, Profile::Typl),
        "a dropped member must change the structure stream"
    );
    assert_ne!(
        syntax_structure("package p\nconst X: integer = 5\n", Profile::Typl),
        syntax_structure("package p\nconst X: integer = 6\n", Profile::Typl),
        "a mutated literal must change the structure stream"
    );
    assert_eq!(
        syntax_structure(members, Profile::Typl),
        syntax_structure("package p\nstruct S {\n a: A\n b: B\n}\n", Profile::Typl),
        "whitespace and separator commas must not change structure"
    );

    // Dropping the in-constraint comment changes the stream — so had the
    // formatter still dropped it,
    // `formatting_preserves_content_tokens_over_the_ok_corpus` would fail on
    // such an input.
    let with_comment = content_tokens("package p\ntype F: bytes [/* fixed */ 8]\n", Profile::Typl);
    let without_comment = content_tokens("package p\ntype F: bytes [8]\n", Profile::Typl);
    assert_ne!(with_comment, without_comment, "a dropped comment must show");

    // Mutating a literal changes the stream too.
    let five = content_tokens("package p\nconst X: integer = 5\n", Profile::Typl);
    let six = content_tokens("package p\nconst X: integer = 6\n", Profile::Typl);
    assert_ne!(five, six, "a mutated literal must show");

    // Whitespace and separator commas do not — those the formatter may change.
    let spaced = content_tokens("package p\nenum E { A = 0, B = 1 }\n", Profile::Typl);
    let newlined = content_tokens("package p\nenum E {\n  A = 0\n  B = 1\n}\n", Profile::Typl);
    assert_eq!(spaced, newlined, "whitespace and commas must be ignored");
}

#[test]
fn annotation_normalization_preserves_comments_members_and_literal_identity() {
    let attributes_first = "package p\ninterface I {\n query q(): T [ persist /* attribute */ ] @ /* timing */ 10ms\n signal z: B\n}\n";
    let timing_first = "package p\ninterface I {\n query q(): T @ /* timing */ 10ms [ persist /* attribute */ ]\n signal z: B\n}\n";
    for source in [attributes_first, timing_first] {
        assert!(
            ridl_syntax::parse(source, Profile::Ridl)
                .errors()
                .is_empty()
        );
    }
    assert_eq!(
        content_tokens(attributes_first, Profile::Ridl),
        content_tokens(timing_first, Profile::Ridl),
        "D-4 allows only the annotation-pair order change"
    );
    assert_eq!(
        syntax_structure(attributes_first, Profile::Ridl),
        syntax_structure(timing_first, Profile::Ridl)
    );
    for comment in ["/* attribute */", "/* timing */"] {
        assert_ne!(
            content_tokens(attributes_first, Profile::Ridl),
            content_tokens(&timing_first.replace(comment, ""), Profile::Ridl),
            "normalization must detect a dropped {comment}"
        );
    }
    let missing_member = timing_first.replace(" signal z: B\n", "");
    let changed_literal = timing_first.replace("10ms", "11ms");
    for changed in [missing_member, changed_literal] {
        assert_ne!(
            syntax_structure(attributes_first, Profile::Ridl),
            syntax_structure(&changed, Profile::Ridl)
        );
        assert_ne!(
            content_tokens(attributes_first, Profile::Ridl),
            content_tokens(&changed, Profile::Ridl)
        );
    }
    let reordered_members = "package p\ninterface I {\n signal z: B\n query q(): T @ /* timing */ 10ms [ persist /* attribute */ ]\n}\n";
    assert_ne!(
        syntax_structure(attributes_first, Profile::Ridl),
        syntax_structure(reordered_members, Profile::Ridl),
        "normalization must not permit member reordering"
    );
}

#[test]
fn annotation_normalization_detects_a_dropped_intervening_comment() {
    let source = "package p\ninterface I { query q(): T [persist] /* note */ @10ms }\n";
    let canonical = "package p\ninterface I { query q(): T @10ms [ persist ] /* note */ }\n";
    assert_eq!(
        content_tokens(source, Profile::Ridl),
        content_tokens(canonical, Profile::Ridl)
    );
    assert_eq!(
        syntax_structure(source, Profile::Ridl),
        syntax_structure(canonical, Profile::Ridl)
    );
    assert_ne!(
        content_tokens(source, Profile::Ridl),
        content_tokens(&canonical.replace("/* note */", ""), Profile::Ridl)
    );
}
