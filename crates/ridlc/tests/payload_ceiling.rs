//! RIDL-157 (ADR-0030 decisions 16 and 17): a payload carried by an
//! interaction that no `[limits]` entry gives a limit, whose largest encoded
//! size, in an encoding the workspace manifest enables, is above the default
//! ceiling of the interaction's kind or has no finite bound. The retired lint
//! RIDL-150 `payload-above-ceiling` is a reserved name (decision 7 as
//! amended). TYPL-116 is also checked here, through the same workspace.

use ridl_core::Frozen;
use ridl_core::diag::{Diagnostic, Severity};

/// A workspace whose root manifest ends with `extra`, with one member per
/// `(package, source)` pair, each holding its source as one file. Returns the
/// diagnostics of `ridl check` from the root.
fn check_members(members: &[(&str, &str)], extra: &str) -> Vec<Diagnostic> {
    let members: Vec<(&str, Vec<(&str, &str)>)> = members
        .iter()
        .map(|(name, source)| (*name, vec![("m.ridl", *source)]))
        .collect();
    check_files(&members, extra)
}

/// Like [`check_members`], with the files of each member given by name.
fn check_files(members: &[(&str, Vec<(&str, &str)>)], extra: &str) -> Vec<Diagnostic> {
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
    for (name, files) in members {
        let member = root.path().join(name);
        std::fs::create_dir(&member).unwrap();
        std::fs::write(
            member.join("ridl.toml"),
            format!("[package]\nname = \"{name}\"\nversion = \"1.0.0\"\n"),
        )
        .unwrap();
        for (file, source) in files {
            std::fs::write(member.join(file), source).unwrap();
        }
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

fn with<'a>(code: &str, diagnostics: &'a [Diagnostic]) -> Vec<&'a Diagnostic> {
    diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code.as_str() == code)
        .collect()
}

fn ridl_157(diagnostics: &[Diagnostic]) -> Vec<&Diagnostic> {
    with("RIDL-157", diagnostics)
}

/// The codes of the size pass that `diagnostics` holds, sorted.
fn size_codes(diagnostics: &[Diagnostic]) -> Vec<&str> {
    let mut codes: Vec<&str> = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code.as_str())
        .filter(|code| matches!(*code, "RIDL-151" | "RIDL-152" | "RIDL-157" | "MANI-022"))
        .collect();
    codes.sort_unstable();
    codes
}

/// A signal that carries `Payload`, with a timing so no other warning draws
/// attention.
fn carried(types: &str) -> String {
    format!("package demo\n{types}\ninterface I {{\n  signal s: Payload @[10ms..100ms]\n}}\n")
}

const PROTO3: &str = "[backend.rust]\nencodings = [\"proto3\"]\n";
const PODBUF: &str = "[backend.rust]\nencodings = [\"podbuf\"]\n";
const FLATBUFFERS: &str = "[backend.rust]\nencodings = [\"flatbuffers\"]\n";

/// With no `[limits]` table, a signal whose payload is above 3,840 bytes in
/// FlatBuffers (a `string [0..946]` is 3,842 bytes) draws the error RIDL-157,
/// on the interaction, naming the kind's default ceiling and the raise.
#[test]
fn a_signal_above_its_default_ceiling_draws_ridl_157() {
    let source = carried("type Payload: string [0..946]\n");
    let diagnostics = check(&source, "");
    let found = ridl_157(&diagnostics);
    assert_eq!(found.len(), 1, "{diagnostics:?}");
    assert_eq!(found[0].severity, Severity::Error);
    assert_eq!(
        found[0].message,
        "the payload of `I.s` has up to 3842 bytes in flatbuffers; the default ceiling of a \
         signal is 3840 bytes, and a `[limits.interaction]` entry may raise it up to 16128"
    );
    // Reported on the interaction, not on the type.
    let text = &source
        [usize::from(found[0].primary.range.start())..usize::from(found[0].primary.range.end())];
    assert_eq!(text, "s");
}

/// The exact boundary of each kind, in proto3, where a named `bytes [0..N]`
/// payload is N plus a one-byte tag and a two-byte length (three bytes above
/// 16,383): a payload at the default ceiling draws nothing, and one byte more
/// draws RIDL-157. A call checks its request and its reply.
#[test]
fn each_kind_is_checked_against_its_own_default_ceiling() {
    let cases: [(&str, &str, u64, u64); 6] = [
        ("signal s: Payload @[10ms..100ms]", "payload", 3_840, 3),
        ("event e: Payload @[10ms..100ms]", "payload", 3_840, 3),
        ("command c(data: Payload) @[..1s]", "request", 16_128, 3),
        ("query q(data: Payload): Flag @[..1s]", "request", 16_128, 3),
        ("query r(): Payload @[..1s]", "reply", 16_128, 3),
        ("fixed f: Payload", "value", 65_280, 4),
    ];
    for (member, role, ceiling, framing) in cases {
        let source = |bound: u64| {
            format!(
                "package demo\ntype Payload: bytes [0..{bound}]\ntype Flag: boolean\n\
                 interface I {{\n  {member}\n}}\n"
            )
        };
        let at = check(&source(ceiling - framing), PROTO3);
        assert!(ridl_157(&at).is_empty(), "{member}: {at:?}");
        let above = check(&source(ceiling - framing + 1), PROTO3);
        let found = ridl_157(&above);
        assert_eq!(found.len(), 1, "{member}: {above:?}");
        assert!(
            found[0].message.starts_with(&format!("the {role} of `I.")),
            "{member}: {}",
            found[0].message
        );
        assert!(
            found[0]
                .message
                .contains(&format!("up to {} bytes in proto3", ceiling + 1)),
            "{member}: {}",
            found[0].message
        );
        assert!(
            found[0].message.contains(&format!("is {ceiling} bytes")),
            "{member}: {}",
            found[0].message
        );
    }
}

/// The largest `string [0..N]` a signal carries within 3,840 bytes, measured
/// per encoding (ADR-0030 decision 16): 945 in FlatBuffers, 959 in proto3,
/// 955 in podbuf. One more character draws RIDL-157 in that encoding alone.
#[test]
fn the_largest_string_of_a_signal_is_measured_per_encoding() {
    for (manifest, encoding, largest) in [
        (FLATBUFFERS, "flatbuffers", 945u32),
        (PROTO3, "proto3", 959),
        (PODBUF, "podbuf", 955),
    ] {
        let fits = check(
            &carried(&format!("type Payload: string [0..{largest}]\n")),
            manifest,
        );
        assert!(ridl_157(&fits).is_empty(), "{encoding}: {fits:?}");
        let above = check(
            &carried(&format!("type Payload: string [0..{}]\n", largest + 1)),
            manifest,
        );
        let found = ridl_157(&above);
        assert_eq!(found.len(), 1, "{encoding}: {above:?}");
        assert!(
            found[0].message.contains(&format!("in {encoding}")),
            "{}",
            found[0].message
        );
    }
}

/// A payload with no finite bound draws RIDL-157 whatever its kind: for each
/// kind, an array whose largest size does not fit a `u32`.
#[test]
fn a_payload_with_no_finite_bound_is_reported_for_every_kind() {
    for member in [
        "signal s: Payload @[10ms..100ms]",
        "event e: Payload @[10ms..100ms]",
        "command c(data: Payload) @[..1s]",
        "query q(): Payload @[..1s]",
        "fixed f: Payload",
    ] {
        let source = format!(
            "package demo\ntype Big: string [0..4032]\n\
             struct Payload {{ rows: [Big; 0..4294967295] }}\n\
             interface I {{\n  {member}\n}}\n"
        );
        let diagnostics = check(&source, "");
        let found = ridl_157(&diagnostics);
        assert_eq!(found.len(), 1, "{member}: {diagnostics:?}");
        assert!(
            found[0].message.contains("no finite bound"),
            "{member}: {}",
            found[0].message
        );
    }
}

#[test]
fn a_payload_within_the_ceiling_draws_nothing() {
    let source = carried("type Text: string [0..900]\nstruct Payload { a: Text }\n");
    let diagnostics = check(&source, "");
    assert!(ridl_157(&diagnostics).is_empty(), "{diagnostics:?}");
}

#[test]
fn a_type_no_interaction_carries_is_not_reported() {
    let source = "package demo\ntype Text: string [0..4032]\nstruct Payload { a: Text, b: Text }\n\
                  interface I {\n  signal s: Flag @[10ms..100ms]\n}\ntype Flag: boolean\n";
    let diagnostics = check(source, "");
    assert!(ridl_157(&diagnostics).is_empty(), "{diagnostics:?}");
}

#[test]
fn the_encoding_checked_is_the_one_the_package_enables() {
    let source = carried("type Text: string [0..4032]\nstruct Payload { a: Text, b: Text }\n");
    let diagnostics = check(&source, PROTO3);
    let found = ridl_157(&diagnostics);
    assert_eq!(found.len(), 1, "{diagnostics:?}");
    assert!(found[0].message.contains("proto3"), "{}", found[0].message);
    assert!(
        !found[0].message.contains("flatbuffers"),
        "{}",
        found[0].message
    );
}

/// `payload-above-ceiling` is a reserved name: a `[lints]` key that names it
/// draws MANI-010 and is ignored, and the size pass still runs.
#[test]
fn the_retired_lint_name_is_unknown_and_does_not_stop_the_pass() {
    let source = carried("type Payload: string [0..946]\n");
    for level in ["deny", "allow"] {
        let diagnostics = check(
            &source,
            &format!("[lints]\npayload-above-ceiling = \"{level}\"\n"),
        );
        assert_eq!(with("MANI-010", &diagnostics).len(), 1, "{diagnostics:?}");
        let found = ridl_157(&diagnostics);
        assert_eq!(found.len(), 1, "{diagnostics:?}");
        assert_eq!(found[0].severity, Severity::Error);
    }
}

/// A `[limits.interaction]` entry gives the interaction its limit: at 16,128
/// a signal of 3,842 bytes draws nothing, and a payload above the entry draws
/// RIDL-151, never RIDL-157 as well.
#[test]
fn an_interaction_entry_removes_ridl_157_and_ridl_151_checks_the_entry() {
    let raised = check_files(
        &[(
            "demo",
            vec![
                (
                    "m.ridl",
                    carried("type Payload: string [0..946]\n").as_str(),
                ),
                (
                    "ridl.toml",
                    "[package]\nname = \"demo\"\nversion = \"1.0.0\"\n\
                     [limits.interaction]\n\"I.s\" = 16128\n",
                ),
            ],
        )],
        "",
    );
    assert_eq!(size_codes(&raised), Vec::<&str>::new(), "{raised:?}");
    let above = check_files(
        &[(
            "demo",
            vec![
                (
                    "m.ridl",
                    carried("type Payload: string [0..4100]\n").as_str(),
                ),
                (
                    "ridl.toml",
                    "[package]\nname = \"demo\"\nversion = \"1.0.0\"\n\
                     [limits.interaction]\n\"I.s\" = 16128\n",
                ),
            ],
        )],
        "",
    );
    assert_eq!(size_codes(&above), ["RIDL-151"], "{above:?}");
    assert!(
        with("RIDL-151", &above)[0]
            .message
            .ends_with("its limit is 16128 bytes"),
        "{above:?}"
    );
}

/// `default` applies to a signal up to 3,840 bytes: under `default = 65280` a
/// signal of 3,841 bytes draws RIDL-151 against the limit 3,840.
#[test]
fn default_is_clamped_to_the_ceiling_of_the_kind() {
    let diagnostics = check_files(
        &[(
            "demo",
            vec![
                (
                    "m.ridl",
                    carried("type Payload: bytes [0..3838]\n").as_str(),
                ),
                (
                    "ridl.toml",
                    "[package]\nname = \"demo\"\nversion = \"1.0.0\"\n[limits]\ndefault = 65280\n",
                ),
            ],
        )],
        PROTO3,
    );
    assert_eq!(size_codes(&diagnostics), ["RIDL-151"], "{diagnostics:?}");
    assert!(
        with("RIDL-151", &diagnostics)[0]
            .message
            .ends_with("its limit is 3840 bytes"),
        "{diagnostics:?}"
    );
}

/// A `[limits.interaction]` entry between 16,129 and 65,280 names a fixed
/// only: on a command or a signal it is MANI-022 from the size pass, on the
/// value, and the entry is dropped; on a fixed it applies.
#[test]
fn an_entry_above_the_largest_raise_is_mani_022_unless_it_names_a_fixed() {
    let member = |body: &str, manifest: &str| {
        let source =
            format!("package demo\ntype Payload: bytes [0..100]\ninterface I {{\n  {body}\n}}\n");
        let manifest = format!("[package]\nname = \"demo\"\nversion = \"1.0.0\"\n{manifest}");
        let root = tempfile::tempdir().expect("a temp dir");
        std::fs::write(
            root.path().join("ridl.toml"),
            "[workspace]\nmembers = [\"demo\"]\n",
        )
        .unwrap();
        std::fs::create_dir(root.path().join("demo")).unwrap();
        std::fs::write(root.path().join("demo/ridl.toml"), &manifest).unwrap();
        std::fs::write(root.path().join("demo/m.ridl"), source).unwrap();
        ridlc::run_check(root.path(), Frozen::No).unwrap()
    };
    for body in [
        "command c(data: Payload) @[..1s]",
        "signal c: Payload @[10ms..100ms]",
    ] {
        let run = member(body, "[limits.interaction]\n\"I.c\" = 16129\n");
        assert_eq!(
            size_codes(&run.diagnostics),
            ["MANI-022"],
            "{body}: {:?}",
            run.diagnostics
        );
        let found = with("MANI-022", &run.diagnostics)[0];
        let text = run.sources.text(found.primary.file).unwrap();
        let value =
            &text[usize::from(found.primary.range.start())..usize::from(found.primary.range.end())];
        assert_eq!(value, "16129", "{body}: {found:?}");
        assert!(
            found.message.contains("`I.c` is 16129") && found.message.contains("16128 bytes"),
            "{body}: {}",
            found.message
        );
    }
    let run = member(
        "fixed c: Payload",
        "[limits.interaction]\n\"I.c\" = 16129\n",
    );
    assert_eq!(
        size_codes(&run.diagnostics),
        Vec::<&str>::new(),
        "{:?}",
        run.diagnostics
    );
    // A dropped entry gives no limit, so the kind's ceiling is checked: a
    // signal of 4,000 bytes, within the dropped 16,129, is RIDL-157 against
    // 3,840, not RIDL-151 against the entry.
    let diagnostics = check_files(
        &[(
            "demo",
            vec![
                (
                    "m.ridl",
                    "package demo\ntype Payload: bytes [0..4000]\ninterface I {\n  \
                     signal c: Payload @[10ms..100ms]\n}\n",
                ),
                (
                    "ridl.toml",
                    "[package]\nname = \"demo\"\nversion = \"1.0.0\"\n\
                     [limits.interaction]\n\"I.c\" = 16129\n",
                ),
            ],
        )],
        PROTO3,
    );
    assert_eq!(
        size_codes(&diagnostics),
        ["MANI-022", "RIDL-157"],
        "{diagnostics:?}"
    );
}

/// The MANI-022 of the size pass needs the member's kind, so a package with
/// an error that blocks the pass draws none, and no RIDL-152, for an entry
/// above 16,128 bytes that names one of its members.
#[test]
fn a_blocked_package_draws_no_mani_022_from_the_size_pass() {
    let diagnostics = check_files(
        &[(
            "demo",
            vec![
                (
                    "m.ridl",
                    "package demo\ntype Payload: bytes [0..100]\ntype Broken: Nope\n\
                     interface I {\n  signal c: Payload @[10ms..100ms]\n}\n",
                ),
                (
                    "ridl.toml",
                    "[package]\nname = \"demo\"\nversion = \"1.0.0\"\n\
                     [limits.interaction]\n\"I.c\" = 16129\n",
                ),
            ],
        )],
        "",
    );
    assert!(
        diagnostics.iter().any(|d| d.severity == Severity::Error),
        "{diagnostics:?}"
    );
    assert_eq!(
        size_codes(&diagnostics),
        Vec::<&str>::new(),
        "{diagnostics:?}"
    );
}

/// The podbuf size of a payload that is a named `bytes` type is the preamble,
/// the 8-byte head that holds the descriptor, and the bytes: 16 bytes more.
/// At 3,824 bytes that is the ceiling itself, which passes; one byte more is
/// above it, and the proto3 size of the same payload (3,828 bytes) is not.
#[test]
fn ridl_157_reads_the_podbuf_size_when_the_manifest_names_podbuf() {
    let at = carried("type Payload: bytes [0..3824]\n");
    let diagnostics = check(&at, PODBUF);
    assert!(ridl_157(&diagnostics).is_empty(), "{diagnostics:?}");
    let above = carried("type Payload: bytes [0..3825]\n");
    let diagnostics = check(&above, PODBUF);
    let found = ridl_157(&diagnostics);
    assert_eq!(found.len(), 1, "{diagnostics:?}");
    assert!(
        found[0].message.contains("up to 3841 bytes in podbuf"),
        "{}",
        found[0].message
    );
    // The same payload is within the ceiling when only proto3 is enabled.
    let diagnostics = check(&above, PROTO3);
    assert!(ridl_157(&diagnostics).is_empty(), "{diagnostics:?}");
}

/// A command with two parameters is sized as the struct of its fields, which
/// only the podbuf sizer states a size for among the three encodings.
#[test]
fn a_podbuf_request_of_several_parameters_is_sized_as_a_struct() {
    let source = "package demo\ntype Blob: bytes [0..9000]\n\
                  interface I {\n  command c(a: Blob, b: Blob) @[10ms..100ms]\n}\n";
    let diagnostics = check(source, PODBUF);
    let found = ridl_157(&diagnostics);
    assert_eq!(found.len(), 1, "{diagnostics:?}");
    assert!(
        found[0].message.contains("in podbuf") && found[0].message.contains("request"),
        "{}",
        found[0].message
    );
}

/// A query reply is sized under podbuf as the lowering sizes it: a named
/// reply as that type, a tuple reply as the struct of its fields.
#[test]
fn a_podbuf_query_reply_is_sized_for_a_named_and_a_tuple_reply() {
    let named = "package demo\ntype Reply: bytes [0..16113]\n\
                 interface I {\n  query get(): Reply @[..1s]\n}\n";
    let tuple = "package demo\ntype Blob: bytes [0..9000]\n\
                 interface I {\n  query get(): (a: Blob, b: Blob) @[..1s]\n}\n";
    for source in [named, tuple] {
        let diagnostics = check(source, PODBUF);
        let found = ridl_157(&diagnostics);
        assert_eq!(found.len(), 1, "{diagnostics:?}");
        assert!(
            found[0].message.contains("reply") && found[0].message.contains("in podbuf"),
            "{}",
            found[0].message
        );
    }
}

/// A payload that holds a `ridl.std` type is sized over the standard package
/// too, so it has a finite bound and draws nothing.
#[test]
fn a_payload_holding_a_std_type_has_a_finite_bound() {
    let source = carried("struct Payload { at: Timestamp }\n");
    let diagnostics = check(&source, "");
    assert!(ridl_157(&diagnostics).is_empty(), "{diagnostics:?}");
}

const BIG: &str =
    "type Text: string [0..4032]\nstruct Payload { a: Text, b: Text }\ntype Flag: boolean\n";

fn with_interface(body: &str) -> String {
    format!("package demo\n{BIG}interface I {{\n  {body}\n}}\n")
}

/// A package with an error draws none of the size pass's codes, with or
/// without a `[limits]` table.
#[test]
fn nothing_is_reported_while_the_package_has_an_error() {
    let source = format!(
        "{}struct Broken {{ x: Missing }}\n",
        with_interface("signal s: Payload @[10ms..100ms]")
    );
    for manifest in ["", "[limits.interaction]\n\"I.s\" = 10\n\"I.gone\" = 10\n"] {
        let diagnostics = check_files(
            &[(
                "demo",
                vec![
                    ("m.ridl", source.as_str()),
                    (
                        "ridl.toml",
                        &format!("[package]\nname = \"demo\"\nversion = \"1.0.0\"\n{manifest}"),
                    ),
                ],
            )],
            "",
        );
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.severity == Severity::Error),
            "{diagnostics:?}"
        );
        assert_eq!(
            size_codes(&diagnostics),
            Vec::<&str>::new(),
            "{diagnostics:?}"
        );
    }
}

/// Two backend tables enable different encodings, and the payload is above the
/// ceiling only in FlatBuffers, whose bound adds table overhead to the 3,824
/// bytes: proto3 alone reports nothing, and the union of both tables reports
/// the same message in either order.
#[test]
fn encodings_of_every_backend_table_are_checked() {
    let source = carried("type Payload: bytes [0..3824]\n");
    let proto3_only = check(&source, "[backend.a]\nencodings = [\"proto3\"]\n");
    assert!(ridl_157(&proto3_only).is_empty(), "{proto3_only:?}");
    for tables in [
        "[backend.a]\nencodings = [\"proto3\"]\n[backend.b]\nencodings = [\"flatbuffers\"]\n",
        "[backend.a]\nencodings = [\"flatbuffers\"]\n[backend.b]\nencodings = [\"proto3\"]\n",
    ] {
        let both = check(&source, tables);
        let found = ridl_157(&both);
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
    let source = carried("type Payload: bytes [0..4000]\n");
    let diagnostics = check(
        &source,
        "[backend.a]\nencodings = [\"flatbuffers\", \"proto3\"]\n",
    );
    let found = ridl_157(&diagnostics);
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

/// A fallible `T | E` reply has no size of its own: its success type and its
/// error type are sized separately, and the larger problem is reported with
/// the type that has it. The success type is the large one here.
#[test]
fn a_fallible_reply_is_checked_through_both_of_its_types() {
    let big_ok = format!(
        "package demo\n{BIG}error enum Failure {{ BAD = 0 }}\ninterface I {{\n  query get(): Payload | Failure @[..1s]\n}}\n"
    );
    let diagnostics = check(&big_ok, "");
    let found = ridl_157(&diagnostics);
    assert_eq!(found.len(), 1, "{diagnostics:?}");
    assert!(
        found[0].message.starts_with("the reply of `I.get`")
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
    let found = ridl_157(&diagnostics);
    assert_eq!(found.len(), 1, "{diagnostics:?}");
    assert!(found[0].message.contains("`I.s`"), "{}", found[0].message);
}

/// An error in one member stops the pass for that member and for the members
/// that depend on it, and for no other member.
#[test]
fn an_error_blocks_only_its_own_package_and_its_dependents() {
    let broken = "package broken\ntype Text: string [0..4032]\nstruct Bad { x: Missing }\n";
    let dependent = "package dependent\nimport broken.Text\nstruct Payload { a: Text, b: Text }\n\
                     interface I {\n  signal s: Payload @[10ms..100ms]\n}\n";
    let independent = carried("type Text: string [0..4032]\nstruct Payload { a: Text, b: Text }\n")
        .replace("package demo", "package independent");
    let run = {
        let root = tempfile::tempdir().expect("a temp dir");
        std::fs::write(
            root.path().join("ridl.toml"),
            "[workspace]\nmembers = [\"broken\", \"dependent\", \"independent\"]\n",
        )
        .unwrap();
        for (name, source) in [
            ("broken", broken),
            ("dependent", dependent),
            ("independent", independent.as_str()),
        ] {
            let member = root.path().join(name);
            std::fs::create_dir(&member).unwrap();
            std::fs::write(
                member.join("ridl.toml"),
                format!("[package]\nname = \"{name}\"\nversion = \"1.0.0\"\n"),
            )
            .unwrap();
            std::fs::write(member.join("m.ridl"), source).unwrap();
        }
        ridlc::run_check(root.path(), Frozen::No).unwrap()
    };
    let found = ridl_157(&run.diagnostics);
    assert_eq!(found.len(), 1, "{:?}", run.diagnostics);
    let path = run.sources.path(found[0].primary.file).unwrap();
    assert!(path.ends_with("independent/m.ridl"), "{path}");
}

/// An inline service shape is named by the last segment of its service name,
/// the name a `[limits.interaction]` key uses.
#[test]
fn an_inline_service_shape_is_named_by_its_short_name() {
    let source =
        format!("package demo\n{BIG}service demo.svc {{\n  signal s: Payload @[10ms..100ms]\n}}\n");
    let diagnostics = check(&source, "");
    let found = ridl_157(&diagnostics);
    assert_eq!(found.len(), 1, "{diagnostics:?}");
    assert!(
        found[0].message.starts_with("the payload of `svc.s`"),
        "{}",
        found[0].message
    );
}

/// TYPL-116 takes its level from `[lints]` like any other lint.
#[test]
fn bound_above_transfer_takes_its_level_from_the_lints_table() {
    let source = "package demo\ntype Text: string [0..16257]\n";
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

/// TYPL-116 thresholds: a `string` bound above 16,256 characters and a
/// `bytes` bound above 65,280 bytes. The threshold itself draws nothing.
#[test]
fn bound_above_transfer_thresholds() {
    let typl_116 = |source: &str| {
        check(source, "")
            .into_iter()
            .filter(|diagnostic| diagnostic.code.as_str() == "TYPL-116")
            .collect::<Vec<_>>()
    };
    assert!(typl_116("package demo\ntype Text: string [0..16256]\n").is_empty());
    assert!(typl_116("package demo\ntype Blob: bytes [0..65280]\n").is_empty());
    let found = typl_116("package demo\ntype Text: string [0..16257]\n");
    assert_eq!(found.len(), 1);
    assert!(
        found[0].message.contains("16256 characters"),
        "{}",
        found[0].message
    );
    assert!(
        found[0]
            .message
            .contains("the largest payload of any kind is 65280 bytes"),
        "{}",
        found[0].message
    );
    assert!(
        !found[0].message.contains("do not yet enforce") && !found[0].message.contains("#996"),
        "{}",
        found[0].message
    );
    let found = typl_116("package demo\ntype Blob: bytes [0..65281]\n");
    assert_eq!(found.len(), 1);
    assert!(
        found[0]
            .message
            .contains("the largest payload of any kind is 65280 bytes"),
        "{}",
        found[0].message
    );
    assert!(
        !found[0].message.contains("do not yet enforce")
            && !found[0].message.contains("#996")
            && !found[0].message.contains("16256"),
        "{}",
        found[0].message
    );
}

/// The error type of a fallible reply is sized too: here it is the large one,
/// and the success type is small.
#[test]
fn a_fallible_reply_with_a_large_error_type_is_reported() {
    let source = "package demo\ntype Text: string [0..4032]\n\
                  error struct Failure { a: Text, b: Text }\nstruct Small { n: boolean }\n\
                  interface I {\n  query get(): Small | Failure @[..1s]\n}\n";
    let diagnostics = check(source, "");
    let found = ridl_157(&diagnostics);
    assert_eq!(found.len(), 1, "{diagnostics:?}");
    assert!(
        found[0].message.contains("for `Failure`"),
        "{}",
        found[0].message
    );
}

/// A fallible reply is checked in the encodings the manifest enables, each
/// against its own bound: `bytes [0..16112]` is above the query ceiling only
/// in FlatBuffers.
#[test]
fn a_fallible_reply_is_checked_per_encoding() {
    let source = "package demo\ntype Payload: bytes [0..16112]\nerror enum Failure { BAD = 0 }\n\
                  interface I {\n  query get(): Payload | Failure @[..1s]\n}\n";
    let proto3 = check(source, "[backend.a]\nencodings = [\"proto3\"]\n");
    assert!(ridl_157(&proto3).is_empty(), "{proto3:?}");
    let default = check(source, "");
    let found = ridl_157(&default);
    assert_eq!(found.len(), 1, "{default:?}");
    assert!(
        found[0].message.contains("in flatbuffers"),
        "{}",
        found[0].message
    );
    let flatbuffers = check(source, "[backend.a]\nencodings = [\"flatbuffers\"]\n");
    assert_eq!(ridl_157(&flatbuffers).len(), 1, "{flatbuffers:?}");
}

/// The types of a fallible reply that come from a sibling package are sized
/// through the sibling.
#[test]
fn a_fallible_reply_with_types_from_a_sibling_package_is_sized() {
    let types = "package types\ntype Text: string [0..4032]\n\
                 struct Payload { a: Text, b: Text }\nerror enum Failure { BAD = 0 }\n";
    let user = "package user\nimport types.Payload\nimport types.Failure\n\
                interface I {\n  query get(): Payload | Failure @[..1s]\n}\n";
    let diagnostics = check_members(&[("types", types), ("user", user)], "");
    let found = ridl_157(&diagnostics);
    assert_eq!(found.len(), 1, "{diagnostics:?}");
    assert!(
        found[0].message.starts_with("the reply of `I.get`")
            && found[0].message.contains("for `types.Payload`"),
        "{}",
        found[0].message
    );
}

/// A package that depends on a blocked package is blocked too, through any
/// number of steps.
#[test]
fn a_blocked_package_blocks_its_dependents_transitively() {
    let broken = "package broken\ntype Text: string [0..4032]\nstruct Bad { x: Missing }\n";
    let middle = "package middle\nimport broken.Text\nstruct Payload { a: Text, b: Text }\n";
    let user = "package user\nimport middle.Payload\n\
                interface I {\n  signal s: Payload @[10ms..100ms]\n}\n";
    let diagnostics = check_members(
        &[("broken", broken), ("middle", middle), ("user", user)],
        "",
    );
    assert!(ridl_157(&diagnostics).is_empty(), "{diagnostics:?}");
}

/// An RSDL-7xx error stops no artifact of `ridl build`, so it does not stop
/// the size pass.
#[test]
fn an_rsdl_error_does_not_stop_the_pass() {
    let ridl = carried("type Text: string [0..4032]\nstruct Payload { a: Text, b: Text }\n");
    let rsdl = "package demo\ndeployment Lab for Nowhere {}\n";
    let diagnostics = check_files(&[("demo", vec![("m.ridl", &ridl), ("m.rsdl", rsdl)])], "");
    let errors: Vec<_> = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.severity == Severity::Error)
        .map(|diagnostic| diagnostic.code.as_str())
        .filter(|code| *code != "RIDL-157")
        .collect();
    assert!(
        !errors.is_empty() && errors.iter().all(|code| code.starts_with("RSDL-7")),
        "{diagnostics:?}"
    );
    assert_eq!(ridl_157(&diagnostics).len(), 1, "{diagnostics:?}");
}

/// An error in the root manifest is beneath no member in particular and above
/// every member, so it skips every package.
#[test]
fn a_root_manifest_error_stops_the_pass_everywhere() {
    let source = carried("type Text: string [0..4032]\nstruct Payload { a: Text, b: Text }\n");
    let diagnostics = check(&source, "[backend.a]\nencodings = \"proto3\"\n");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code.as_str() == "MANI-015"),
        "{diagnostics:?}"
    );
    assert!(ridl_157(&diagnostics).is_empty(), "{diagnostics:?}");
}

/// A package with a podbuf error is blocked for the size pass like a package
/// with any other error: with podbuf enabled, a struct above `u32::MAX`
/// (RIDL-153) beside a signal whose payload is above the ceiling draws the
/// RIDL-153 alone, and no RIDL-157.
#[test]
fn a_package_with_a_podbuf_error_draws_no_ridl_157() {
    let source = "package demo\n\
        type Blob: bytes [60000..60000]\n\
        struct Huge {\n  first: [Blob; 0..65535]\n  second: [Blob; 0..65535]\n}\n\
        type Text: string [0..4032]\n\
        struct Payload { a: Text, b: Text }\n\
        interface I {\n  signal s: Payload @[10ms..100ms]\n}\n";
    let diagnostics = check(source, PODBUF);
    let mut codes: Vec<&str> = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code.as_str())
        .filter(|code| code.starts_with("RIDL-15"))
        .collect();
    codes.sort_unstable();
    assert_eq!(codes, ["RIDL-153"], "{diagnostics:?}");
}
