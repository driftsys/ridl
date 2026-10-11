//! The doc-content test of the Rust backend (the generated-code documentation
//! design, test 4): no doc comment in the generated output contains an issue
//! reference or a URL.
//!
//! An issue reference is `#` followed by a digit, or `driftsys/ridl`. A URL is
//! anything with `://`. Generated docs state the contract for the user of the
//! generated code; an explanation for a maintainer of the emitter belongs in a
//! `//` comment beside the code that emits it.
//!
//! The outputs checked are the checked-in face, `generate` and
//! `generate_pipeline` over every fixture in `tests/fixtures/`, and the skip
//! notes `generate_pipeline` leaves for an interface it refuses for a
//! contract clause and for a call shape.

#[path = "support/ir.rs"]
mod ir;

use proc_macro2::{TokenStream, TokenTree};

/// Every doc string in `source`: each `///`, `//!`, `/** */` and `/*! */`
/// comment and each `#[doc = "..."]` attribute, which the tokenizer reads as
/// the same `doc` attribute.
fn doc_strings(source: &str) -> Vec<String> {
    let tokens: TokenStream = source.parse().expect("the generated source tokenizes");
    let mut docs = Vec::new();
    collect(tokens, &mut docs);
    docs
}

fn collect(tokens: TokenStream, docs: &mut Vec<String>) {
    for tree in tokens {
        if let TokenTree::Group(group) = tree {
            if let Some(text) = doc_value(&group.stream()) {
                docs.push(text);
            }
            collect(group.stream(), docs);
        }
    }
}

/// The text of `doc = "..."`, the inside of a doc attribute's brackets.
fn doc_value(inner: &TokenStream) -> Option<String> {
    let trees: Vec<TokenTree> = inner.clone().into_iter().collect();
    match trees.as_slice() {
        [
            TokenTree::Ident(name),
            TokenTree::Punct(eq),
            TokenTree::Literal(literal),
        ] if name == "doc" && eq.as_char() == '=' => {
            let lit: syn::LitStr = syn::parse_str(&literal.to_string()).ok()?;
            Some(lit.value())
        }
        _ => None,
    }
}

/// The first forbidden reference in `doc`, if any.
fn forbidden(doc: &str) -> Option<&'static str> {
    let bytes = doc.as_bytes();
    if bytes
        .windows(2)
        .any(|pair| pair[0] == b'#' && pair[1].is_ascii_digit())
    {
        return Some("`#` followed by a digit");
    }
    if doc.contains("driftsys/ridl") {
        return Some("`driftsys/ridl`");
    }
    if doc.contains("://") {
        return Some("a URL (`://`)");
    }
    None
}

/// Fails with every doc string of `source` that holds a forbidden reference.
fn assert_clean(label: &str, source: &str) {
    let docs = doc_strings(source);
    assert!(!docs.is_empty(), "{label}: no doc string was found");
    let offending: Vec<String> = docs
        .iter()
        .filter_map(|doc| forbidden(doc).map(|why| format!("{why}: {doc:?}")))
        .collect();
    assert!(
        offending.is_empty(),
        "{label}: {} generated doc string(s) hold an issue reference or a URL:\n{}",
        offending.len(),
        offending.join("\n"),
    );
}

fn fixtures() -> Vec<String> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let mut names: Vec<String> = std::fs::read_dir(&dir)
        .unwrap_or_else(|error| panic!("read {}: {error}", dir.display()))
        .map(|entry| entry.expect("a fixture entry").file_name())
        .filter_map(|name| name.into_string().ok())
        .filter(|name| name.ends_with(".ridl"))
        .collect();
    names.sort();
    assert!(!names.is_empty(), "no fixture in {}", dir.display());
    names
}

/// The detector itself: each forbidden form is found, and plain text is not.
#[test]
fn the_detector_finds_each_forbidden_form() {
    assert!(forbidden("see driftsys/ridl for it").is_some());
    assert!(forbidden("removed by #704").is_some());
    assert!(forbidden("at https://example.org").is_some());
    assert!(forbidden("a `# Rules` heading, decision 8, and C#").is_none());
    let source = "/// first\n#[doc = \"(#713)\"]\nfn f() { /** inner */ struct S; }";
    assert_eq!(
        doc_strings(source),
        vec![
            " first".to_string(),
            "(#713)".to_string(),
            " inner ".to_string()
        ]
    );
}

#[test]
fn the_checked_in_face_cites_no_issue() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/generated/interaction_face.rs");
    let source = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
    assert_clean("tests/generated/interaction_face.rs", &source);
}

#[test]
fn generate_over_every_fixture_cites_no_issue() {
    for name in fixtures() {
        let package = ir::compile_fixture(&name);
        let source = ridl_backend_rust::generate(&package)
            .unwrap_or_else(|error| panic!("generate {name}: {error:?}"))
            .rust_source;
        assert_clean(&format!("generate({name})"), &source);
    }
}

#[test]
fn generate_pipeline_over_every_fixture_cites_no_issue() {
    for name in fixtures() {
        let package = ir::compile_fixture(&name);
        let source = ridl_backend_rust::generate_pipeline(
            &package,
            &[ridl_backend_rust::WireEncoding::FlatBuffers],
            &[],
        )
        .unwrap_or_else(|error| panic!("generate_pipeline {name}: {error:?}"))
        .rust_source;
        assert_clean(&format!("generate_pipeline({name})"), &source);
    }
}

/// The skip note of an interface the clause translator refuses, and of one
/// whose call the face cannot carry: each is emitted, so the check is not
/// vacuous, and neither cites an issue.
#[test]
fn the_skip_notes_cite_no_issue() {
    let sources = [
        (
            "clause",
            r#"
package docs.clause

type Level : integer [0..100]

interface Guarded {
  command set(arg: Level) [
    require arg > 10 || arg < 5
  ] @[..50ms]
}
"#,
        ),
        (
            "call shape",
            r#"
package docs.shape

type Level : integer [0..100]

interface Wide {
  command set(first: Level, second: Level) @[..50ms]
}
"#,
        ),
    ];
    for (label, text) in sources {
        let output = ridlc::compile(&format!("{label}.ridl"), text);
        let source = ridl_backend_rust::generate_pipeline(
            &output.package,
            &[ridl_backend_rust::WireEncoding::FlatBuffers],
            &[],
        )
        .expect("generate_pipeline skips rather than refuses")
        .rust_source;
        assert!(
            source.contains("const __RIDL_NO_FACE_"),
            "{label}: the interface must be skipped, or this test proves nothing:\n{source}"
        );
        assert_clean(&format!("the {label} skip note"), &source);
    }
}
