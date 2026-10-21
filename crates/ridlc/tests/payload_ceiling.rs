//! RIDL-150 `payload-above-ceiling` (ADR-0030 decision 7): a payload carried
//! by an interaction whose largest encoded size, in an encoding the package
//! enables, is above 16,256 bytes or has no finite bound.

use ridl_core::Frozen;
use ridl_core::diag::{Diagnostic, Severity};

/// A workspace whose root manifest ends with `extra`, with one member per
/// `(package, source)` pair, each holding its source as one file. Returns the
/// diagnostics of `ridl check` from the root.
fn check_members(members: &[(&str, &str)], extra: &str) -> Vec<Diagnostic> {
    let root = tempfile::tempdir().expect("a temp dir");
    let list = members
        .iter()
        .map(|(name, _)| format!("\"{name}\""))
        .collect::<Vec<_>>()
        .join(", ");
    std::fs::write(
        root.path().join("ridl.toml"),
        format!("[workspace]\nmembers = [{list}]\n{extra}"),
    )
    .unwrap();
    for (name, source) in members {
        let member = root.path().join(name);
        std::fs::create_dir(&member).unwrap();
        std::fs::write(
            member.join("ridl.toml"),
            format!("[package]\nname = \"{name}\"\nversion = \"1.0.0\"\n"),
        )
        .unwrap();
        std::fs::write(member.join("m.ridl"), source).unwrap();
    }
    ridlc::run_check(root.path(), Frozen::No)
        .unwrap()
        .diagnostics
}

/// A one-member workspace, the package `demo`, whose root manifest ends with
/// `extra`.
fn check(source: &str, extra: &str) -> Vec<Diagnostic> {
    check_members(&[("demo", source)], extra)
}

fn ridl_150(diagnostics: &[Diagnostic]) -> Vec<&Diagnostic> {
    diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code.as_str() == "RIDL-150")
        .collect()
}

/// A signal that carries `Payload`, with a timing so no other warning draws
/// attention.
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
        found[0].message.starts_with("the payload of `demo.I.s`")
            && found[0].message.contains("16256")
            && found[0].message.contains("#996"),
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
    // Every string and bytes bound is finite, so the payload with no finite
    // bound is an array whose largest size does not fit a `u32`.
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
/// bytes: proto3 alone reports nothing, and the union of both tables reports
/// whichever table comes first.
#[test]
fn encodings_of_every_backend_table_are_checked() {
    let source = carried("type Payload: bytes [0..16240]\n");
    let proto3_only = check(&source, "[backend.a]\nencodings = [\"proto3\"]\n");
    assert!(ridl_150(&proto3_only).is_empty(), "{proto3_only:?}");
    for tables in [
        "[backend.a]\nencodings = [\"proto3\"]\n[backend.b]\nencodings = [\"flatbuffers\"]\n",
        "[backend.a]\nencodings = [\"flatbuffers\"]\n[backend.b]\nencodings = [\"proto3\"]\n",
    ] {
        let both = check(&source, tables);
        let found = ridl_150(&both);
        assert_eq!(found.len(), 1, "{tables}: {both:?}");
        assert!(
            found[0].message.contains("flatbuffers"),
            "{}",
            found[0].message
        );
        assert!(!found[0].message.contains("proto3"), "{}", found[0].message);
    }
}

/// A payload above the ceiling in both encodings names both in one message.
#[test]
fn a_payload_above_the_ceiling_in_both_encodings_names_both() {
    let source = carried("type Payload: bytes [0..16300]\n");
    let diagnostics = check(
        &source,
        "[backend.a]\nencodings = [\"flatbuffers\", \"proto3\"]\n",
    );
    let found = ridl_150(&diagnostics);
    assert_eq!(found.len(), 1, "{diagnostics:?}");
    assert!(
        found[0].message.contains("in flatbuffers and"),
        "{}",
        found[0].message
    );
    assert!(
        found[0].message.contains("in proto3"),
        "{}",
        found[0].message
    );
}

#[test]
fn an_event_above_the_ceiling_is_reported() {
    let source =
        format!("package demo\n{BIG}interface I {{\n  event e: Payload @[10ms..100ms]\n}}\n");
    let diagnostics = check(&source, "");
    let found = ridl_150(&diagnostics);
    assert_eq!(found.len(), 1, "{diagnostics:?}");
    assert!(
        found[0].message.starts_with("the payload of `demo.I.e`"),
        "{}",
        found[0].message
    );
}

/// A fallible `T | E` reply has no size of its own: its success type and its
/// error type are sized separately, and the larger problem is reported with
/// the type that has it. The success type is the large one here.
#[test]
fn a_fallible_reply_is_checked_through_both_of_its_types() {
    let big_ok = format!(
        "package demo\n{BIG}error enum Failure {{ BAD = 0 }}\ninterface I {{\n  query get(): Payload | Failure @[..1s]\n}}\n"
    );
    let diagnostics = check(&big_ok, "");
    let found = ridl_150(&diagnostics);
    assert_eq!(found.len(), 1, "{diagnostics:?}");
    assert!(
        found[0].message.starts_with("the reply of `demo.I.get`")
            && found[0].message.contains("for `Payload`"),
        "{}",
        found[0].message
    );
}

/// A payload type declared in a sibling package is sized through the sibling.
#[test]
fn a_payload_type_from_a_sibling_package_is_sized() {
    let types = "package types\ntype Text: string [0..4032]\n";
    let user = "package user\nimport types.Text\nstruct Payload { a: Text, b: Text }\n\
                interface I {\n  signal s: Payload @[10ms..100ms]\n}\n";
    let diagnostics = check_members(&[("types", types), ("user", user)], "");
    let found = ridl_150(&diagnostics);
    assert_eq!(found.len(), 1, "{diagnostics:?}");
    assert!(
        found[0].message.contains("`user.I.s`"),
        "{}",
        found[0].message
    );
}

/// An error in one member turns the lint off for that member and for the
/// members that depend on it, and for no other member.
#[test]
fn an_error_blocks_only_its_own_package_and_its_dependents() {
    let broken = "package broken\ntype Text: string [0..4032]\nstruct Bad { x: Missing }\n";
    let dependent = "package dependent\nimport broken.Text\nstruct Payload { a: Text, b: Text }\n\
                     interface I {\n  signal s: Payload @[10ms..100ms]\n}\n";
    let independent = carried("type Text: string [0..4032]\nstruct Payload { a: Text, b: Text }\n")
        .replace("package demo", "package independent");
    let diagnostics = check_members(
        &[
            ("broken", broken),
            ("dependent", dependent),
            ("independent", &independent),
        ],
        "",
    );
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == Severity::Error),
        "{diagnostics:?}"
    );
    let found = ridl_150(&diagnostics);
    assert_eq!(found.len(), 1, "{diagnostics:?}");
    assert!(
        found[0].message.contains("`independent.I.s`"),
        "{}",
        found[0].message
    );
}

/// An inline service shape is named by its service, once.
#[test]
fn an_inline_service_shape_is_named_without_repeating_the_package() {
    let source =
        format!("package demo\n{BIG}service demo.svc {{\n  signal s: Payload @[10ms..100ms]\n}}\n");
    let diagnostics = check(&source, "");
    let found = ridl_150(&diagnostics);
    assert_eq!(found.len(), 1, "{diagnostics:?}");
    assert!(
        found[0].message.starts_with("the payload of `demo.svc.s`"),
        "{}",
        found[0].message
    );
}

/// TYPL-116 takes its level from `[lints]` like any other lint.
#[test]
fn bound_above_transfer_takes_its_level_from_the_lints_table() {
    let source = "package demo\ntype Text: string [0..5000]\n";
    let typl_116 = |extra: &str| {
        check(source, extra)
            .into_iter()
            .filter(|diagnostic| diagnostic.code.as_str() == "TYPL-116")
            .collect::<Vec<_>>()
    };
    let found = typl_116("");
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].severity, Severity::Warning);
    let found = typl_116("[lints]\nbound-above-transfer = \"deny\"\n");
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].severity, Severity::Error);
    assert!(typl_116("[lints]\nbound-above-transfer = \"allow\"\n").is_empty());
}
