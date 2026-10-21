//! RIDL-150 `payload-above-ceiling` (ADR-0030 decision 7): a payload carried
//! by an interaction whose largest encoded size, in an encoding the package
//! enables, is above 16,256 bytes or has no finite bound.

use ridl_core::Frozen;
use ridl_core::diag::{Diagnostic, Severity};

/// A one-member workspace whose root manifest ends with `extra` and whose
/// member holds `source` as its only file. Returns the diagnostics of `ridl check`.
fn check(source: &str, extra: &str) -> Vec<Diagnostic> {
    let root = tempfile::tempdir().expect("a temp dir");
    std::fs::write(
        root.path().join("ridl.toml"),
        format!("[workspace]\nmembers = [\"m\"]\n{extra}"),
    )
    .unwrap();
    let member = root.path().join("m");
    std::fs::create_dir(&member).unwrap();
    std::fs::write(
        member.join("ridl.toml"),
        "[package]\nname = \"demo\"\nversion = \"1.0.0\"\n",
    )
    .unwrap();
    std::fs::write(member.join("m.ridl"), source).unwrap();
    let run = ridlc::run_check(root.path(), Frozen::No).unwrap();
    run.diagnostics
}

fn ridl_150(diagnostics: &[Diagnostic]) -> Vec<&Diagnostic> {
    diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code.as_str() == "RIDL-150")
        .collect()
}

/// A signal and an event that carry `Payload`, with timings so no other
/// warning draws attention.
fn carried(types: &str) -> String {
    format!("package demo\n{types}\ninterface I {{\n  signal s: Payload @[10ms..100ms]\n}}\n")
}

#[test]
fn a_struct_of_two_strings_at_the_advised_bound_is_above_the_ceiling() {
    // 2 x 4,032 characters is 32,256 bytes of UTF-8, above 16,256.
    let source = carried("type Text: string [0..4032]\nstruct Payload { a: Text, b: Text }\n");
    let diagnostics = check(&source, "");
    let found = ridl_150(&diagnostics);
    assert_eq!(found.len(), 1, "{diagnostics:?}");
    assert_eq!(found[0].severity, Severity::Warning);
    assert!(
        found[0].message.contains("16256") && found[0].message.contains("run time"),
        "{}",
        found[0].message
    );
    assert!(
        found[0].message.contains("flatbuffers"),
        "{}",
        found[0].message
    );
    // Reported on the interaction, not on the type.
    let text = &source
        [usize::from(found[0].primary.range.start())..usize::from(found[0].primary.range.end())];
    assert_eq!(text, "s");
}

#[test]
fn a_payload_within_the_ceiling_draws_nothing() {
    // One string of 4,032 characters is at most 16,128 bytes plus overhead,
    // which fits in 16,256.
    let source = carried("type Text: string [0..4032]\nstruct Payload { a: Text }\n");
    let diagnostics = check(&source, "");
    assert!(ridl_150(&diagnostics).is_empty(), "{diagnostics:?}");
}

#[test]
fn a_payload_with_no_finite_bound_is_reported() {
    // A bare `string` takes the TYPL-103 default bound, so the payload with
    // no finite bound is an array whose largest size does not fit a `u32`.
    let source =
        carried("type Big: string [0..4032]\nstruct Payload { rows: [Big; 0..4294967295] }\n");
    let diagnostics = check(&source, "");
    let found = ridl_150(&diagnostics);
    assert_eq!(found.len(), 1, "{diagnostics:?}");
    assert!(
        found[0].message.contains("no finite bound"),
        "{}",
        found[0].message
    );
}

#[test]
fn a_type_no_interaction_carries_is_not_reported() {
    let source = "package demo\ntype Text: string [0..4032]\nstruct Payload { a: Text, b: Text }\n\
                  interface I {\n  signal s: Flag @[10ms..100ms]\n}\ntype Flag: boolean\n";
    let diagnostics = check(source, "");
    assert!(ridl_150(&diagnostics).is_empty(), "{diagnostics:?}");
}

#[test]
fn the_encoding_checked_is_the_one_the_package_enables() {
    let source = carried("type Text: string [0..4032]\nstruct Payload { a: Text, b: Text }\n");
    let diagnostics = check(&source, "[backend.rust]\nencodings = [\"proto3\"]\n");
    let found = ridl_150(&diagnostics);
    assert_eq!(found.len(), 1, "{diagnostics:?}");
    assert!(found[0].message.contains("proto3"), "{}", found[0].message);
    assert!(
        !found[0].message.contains("flatbuffers"),
        "{}",
        found[0].message
    );
}

#[test]
fn deny_raises_the_lint_to_an_error_and_allow_removes_it() {
    let source = carried("type Text: string [0..4032]\nstruct Payload { a: Text, b: Text }\n");
    let diagnostics = check(&source, "[lints]\npayload-above-ceiling = \"deny\"\n");
    let found = ridl_150(&diagnostics);
    assert_eq!(found.len(), 1, "{diagnostics:?}");
    assert_eq!(found[0].severity, Severity::Error);
    let diagnostics = check(&source, "[lints]\npayload-above-ceiling = \"allow\"\n");
    assert!(ridl_150(&diagnostics).is_empty(), "{diagnostics:?}");
}

/// The proto3 size of a payload that is a named `bytes` type is the root box
/// message: a one-byte tag, a two-byte length and the bytes. At 16,253 bytes
/// that is the ceiling itself, which passes; one byte more is above it.
#[test]
fn the_ceiling_boundary_is_exact() {
    let proto3 = "[backend.rust]\nencodings = [\"proto3\"]\n";
    let at = carried("type Payload: bytes [0..16253]\n");
    let diagnostics = check(&at, proto3);
    assert!(ridl_150(&diagnostics).is_empty(), "{diagnostics:?}");
    let above = carried("type Payload: bytes [0..16254]\n");
    let diagnostics = check(&above, proto3);
    let found = ridl_150(&diagnostics);
    assert_eq!(found.len(), 1, "{diagnostics:?}");
    assert!(
        found[0].message.contains("16257 bytes"),
        "{}",
        found[0].message
    );
}

/// A payload that holds a `ridl.std` type is sized over the standard package
/// too, so it has a finite bound and draws nothing.
#[test]
fn a_payload_holding_a_std_type_has_a_finite_bound() {
    let source = carried("struct Payload { at: Timestamp }\n");
    let diagnostics = check(&source, "");
    assert!(ridl_150(&diagnostics).is_empty(), "{diagnostics:?}");
}

const BIG: &str =
    "type Text: string [0..4032]\nstruct Payload { a: Text, b: Text }\ntype Flag: boolean\n";

fn with_interface(body: &str) -> String {
    format!("package demo\n{BIG}interface I {{\n  {body}\n}}\n")
}

fn assert_one(source: &str, role: &str, member: &str) {
    let diagnostics = check(source, "");
    let found = ridl_150(&diagnostics);
    assert_eq!(found.len(), 1, "{diagnostics:?}");
    assert!(
        found[0].message.starts_with(&format!("the {role} of ")),
        "{}",
        found[0].message
    );
    let text = &source
        [usize::from(found[0].primary.range.start())..usize::from(found[0].primary.range.end())];
    assert_eq!(text, member);
}

#[test]
fn a_command_request_above_the_ceiling_is_reported() {
    assert_one(
        &with_interface("command put(data: Payload) @[..1s]"),
        "request",
        "put",
    );
}

#[test]
fn a_query_request_above_the_ceiling_is_reported() {
    assert_one(
        &with_interface("query get(data: Payload): Flag @[..1s]"),
        "request",
        "get",
    );
}

#[test]
fn a_query_reply_above_the_ceiling_is_reported() {
    assert_one(
        &with_interface("query get(): Payload @[..1s]"),
        "reply",
        "get",
    );
}

#[test]
fn nothing_is_reported_while_the_package_has_an_error() {
    let source = format!(
        "{}struct Broken {{ x: Missing }}\n",
        carried("type Text: string [0..4032]\nstruct Payload { a: Text, b: Text }\n")
    );
    let diagnostics = check(&source, "");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == Severity::Error),
        "{diagnostics:?}"
    );
    assert!(ridl_150(&diagnostics).is_empty(), "{diagnostics:?}");
}

/// Two backend tables enable different encodings, and the payload is above the
/// ceiling only in FlatBuffers, whose bound adds table overhead to the 16,240
/// bytes: proto3 alone reports nothing, and the union of both tables reports.
#[test]
fn encodings_of_every_backend_table_are_checked() {
    let source = carried("type Payload: bytes [0..16240]\n");
    let proto3_only = check(&source, "[backend.a]\nencodings = [\"proto3\"]\n");
    assert!(ridl_150(&proto3_only).is_empty(), "{proto3_only:?}");
    let both = check(
        &source,
        "[backend.a]\nencodings = [\"proto3\"]\n[backend.b]\nencodings = [\"flatbuffers\"]\n",
    );
    let found = ridl_150(&both);
    assert_eq!(found.len(), 1, "{both:?}");
    assert!(
        found[0].message.contains("flatbuffers"),
        "{}",
        found[0].message
    );
    assert!(!found[0].message.contains("proto3"), "{}", found[0].message);
}
