//! RIDL-151 and RIDL-152 (ADR-0030 decisions 12 and 14): the `[limits]` table
//! of a unit's manifest gives each interaction a payload limit; a payload
//! above its limit is an error, and an entry that names no interaction of the
//! unit is an error.

use ridl_core::Frozen;
use ridl_core::diag::{Diagnostic, Severity};

/// One member of a test workspace: its package name, its source, and the text
/// appended to its own `ridl.toml`.
struct Member<'a> {
    name: &'a str,
    source: &'a str,
    manifest: &'a str,
}

/// The result of `ridl check` from the root of a workspace whose root manifest
/// ends with `root_extra`.
fn run(members: &[Member<'_>], root_extra: &str) -> ridlc::CliRun {
    let root = tempfile::tempdir().expect("a temp dir");
    let list = members
        .iter()
        .map(|member| format!("\"{}\"", member.name))
        .collect::<Vec<_>>()
        .join(", ");
    std::fs::write(
        root.path().join("ridl.toml"),
        format!("[workspace]\nmembers = [{list}]\n{root_extra}"),
    )
    .unwrap();
    for member in members {
        let dir = root.path().join(member.name);
        std::fs::create_dir(&dir).unwrap();
        std::fs::write(
            dir.join("ridl.toml"),
            format!(
                "[package]\nname = \"{}\"\nversion = \"1.0.0\"\n{}",
                member.name, member.manifest
            ),
        )
        .unwrap();
        std::fs::write(dir.join("m.ridl"), member.source).unwrap();
    }
    ridlc::run_check(root.path(), Frozen::No).unwrap()
}

const PROTO3: &str = "[backend.rust]\nencodings = [\"proto3\"]\n";

/// The proto3 size of a payload that is a named `bytes` type of 1,000 bytes
/// is the root box message: a one-byte tag, a two-byte length and the bytes.
const N: u64 = 1003;

/// A one-member workspace, the package `demo`, whose own manifest ends with
/// `limits`. The root enables proto3 so the payload sizes are exact.
fn check(source: &str, limits: &str) -> Vec<Diagnostic> {
    run(
        &[Member {
            name: "demo",
            source,
            manifest: limits,
        }],
        PROTO3,
    )
    .diagnostics
}

fn with(code: &str, diagnostics: &[Diagnostic]) -> Vec<Diagnostic> {
    diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code.as_str() == code)
        .cloned()
        .collect()
}

/// A signal `sig` of `Iface` that carries `Payload`, N bytes in proto3.
fn signal() -> String {
    "package demo\ntype Payload: bytes [0..1000]\n\
     interface Iface {\n  signal sig: Payload @[10ms..100ms]\n}\n"
        .to_string()
}

#[test]
fn no_limits_table_reports_nothing_new() {
    let source = "package demo\ntype Text: string [0..4032]\n\
                  struct Payload { a: Text, b: Text }\n\
                  interface Iface {\n  signal sig: Payload @[10ms..100ms]\n}\n";
    let diagnostics = check(source, "");
    assert_eq!(with("RIDL-150", &diagnostics).len(), 1, "{diagnostics:?}");
    assert!(with("RIDL-151", &diagnostics).is_empty(), "{diagnostics:?}");
    assert!(with("RIDL-152", &diagnostics).is_empty(), "{diagnostics:?}");
}

#[test]
fn the_declared_limit_boundary_is_exact() {
    let at = format!("[limits.interaction]\n\"Iface.sig\" = {N}\n");
    let diagnostics = check(&signal(), &at);
    assert!(with("RIDL-151", &diagnostics).is_empty(), "{diagnostics:?}");
    let below = format!("[limits.interaction]\n\"Iface.sig\" = {}\n", N - 1);
    let source = signal();
    let diagnostics = check(&source, &below);
    let found = with("RIDL-151", &diagnostics);
    assert_eq!(found.len(), 1, "{diagnostics:?}");
    assert_eq!(found[0].severity, Severity::Error);
    let message = &found[0].message;
    assert!(
        message.contains("`Iface.sig`")
            && message.contains("payload")
            && message.contains(&format!("{N} bytes"))
            && message.contains(&format!("{} bytes", N - 1)),
        "{message}"
    );
    let range = found[0].primary.range;
    assert_eq!(
        &source[usize::from(range.start())..usize::from(range.end())],
        "sig"
    );
}

#[test]
fn resolution_order_is_name_then_kind_then_default() {
    let source = "package demo\ntype Payload: bytes [0..1000]\n\
                  interface Iface {\n  signal sig: Payload @[10ms..100ms]\n  \
                  event first: Payload @[10ms..100ms]\n  \
                  event second: Payload @[10ms..100ms]\n}\n";
    let limits = "[limits]\ndefault = 1\n[limits.kind]\nsignal = 10000\n\
                  [limits.interaction]\n\"Iface.first\" = 10000\n";
    let diagnostics = check(source, limits);
    let found = with("RIDL-151", &diagnostics);
    assert_eq!(found.len(), 1, "{diagnostics:?}");
    assert!(found[0].message.contains("`Iface.second`"), "{found:?}");
    // The name beats the kind and the default, in the lowering direction too.
    let limits = "[limits]\ndefault = 10000\n[limits.kind]\nsignal = 10000\n\
                  [limits.interaction]\n\"Iface.sig\" = 10\n";
    let diagnostics = check(source, limits);
    let found = with("RIDL-151", &diagnostics);
    assert_eq!(found.len(), 1, "{diagnostics:?}");
    assert!(found[0].message.contains("`Iface.sig`"), "{found:?}");
}

#[test]
fn a_query_checks_request_and_reply_against_one_limit() {
    let source = "package demo\ntype Payload: bytes [0..1000]\ntype Flag: boolean\n\
                  interface Iface {\n  query get(data: Flag): Payload @[..1s]\n  \
                  query put(data: Payload): Flag @[..1s]\n}\n";
    let limits = format!("[limits.kind]\nquery = {}\n", N - 1);
    let diagnostics = check(source, &limits);
    let found = with("RIDL-151", &diagnostics);
    assert_eq!(found.len(), 2, "{diagnostics:?}");
    let get = found.iter().find(|d| d.message.contains("`Iface.get`"));
    let put = found.iter().find(|d| d.message.contains("`Iface.put`"));
    assert!(
        get.is_some_and(|d| d.message.contains("reply")),
        "{found:?}"
    );
    assert!(
        put.is_some_and(|d| d.message.contains("request")),
        "{found:?}"
    );
    let limits = format!("[limits.kind]\nquery = {N}\n");
    assert!(with("RIDL-151", &check(source, &limits)).is_empty());
}

#[test]
fn a_command_request_is_checked() {
    let source = "package demo\ntype Payload: bytes [0..1000]\n\
                  interface Iface {\n  command put(data: Payload) @[..1s]\n}\n";
    let limits = format!("[limits.kind]\ncommand = {}\n", N - 1);
    let found = with("RIDL-151", &check(source, &limits));
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].message.contains("request"), "{found:?}");
}

#[test]
fn an_unbounded_payload_under_a_declared_limit_is_an_error() {
    let source = "package demo\ntype Big: string [0..4032]\n\
                  struct Payload { rows: [Big; 0..4294967295] }\n\
                  interface Iface {\n  signal sig: Payload @[10ms..100ms]\n}\n";
    let diagnostics = check(source, "[limits]\ndefault = 16256\n");
    let found = with("RIDL-151", &diagnostics);
    assert_eq!(found.len(), 1, "{diagnostics:?}");
    assert!(
        found[0].message.contains("no finite bound") || found[0].message.contains("too large"),
        "{found:?}"
    );
}

#[test]
fn a_fixed_is_checked_though_the_ceiling_check_skips_it() {
    let source = "package demo\ntype Text: string [0..4032]\n\
                  struct Payload { a: Text, b: Text }\ntype Small: bytes [0..10]\n\
                  interface Iface {\n  fixed big: Payload\n  fixed small: Small\n}\n";
    // No table: a fixed above the ceiling draws nothing, as before.
    let diagnostics = check(source, "");
    assert!(with("RIDL-150", &diagnostics).is_empty(), "{diagnostics:?}");
    assert!(with("RIDL-151", &diagnostics).is_empty(), "{diagnostics:?}");
    // A table with only `default` sizes the fixed.
    let diagnostics = check(source, "[limits]\ndefault = 16256\n");
    let found = with("RIDL-151", &diagnostics);
    assert_eq!(found.len(), 1, "{diagnostics:?}");
    assert!(found[0].message.contains("`Iface.big`"), "{found:?}");
    assert!(with("RIDL-150", &diagnostics).is_empty(), "{diagnostics:?}");
    // A kind entry reaches it too.
    let diagnostics = check(source, "[limits.kind]\nfixed = 5\n");
    let found = with("RIDL-151", &diagnostics);
    assert_eq!(found.len(), 2, "{diagnostics:?}");
}

#[test]
fn an_inline_service_is_keyed_by_its_short_name() {
    let source = "package demo\ntype Payload: bytes [0..1000]\n\
                  service demo.svc {\n  signal sig: Payload @[10ms..100ms]\n}\n";
    let diagnostics = check(source, "[limits.interaction]\n\"svc.sig\" = 10\n");
    let found = with("RIDL-151", &diagnostics);
    assert_eq!(found.len(), 1, "{diagnostics:?}");
    assert!(found[0].message.contains("`svc.sig`"), "{found:?}");
    assert!(with("RIDL-152", &diagnostics).is_empty(), "{diagnostics:?}");
    // The dotted form is not a key.
    let diagnostics = check(source, "[limits.interaction]\n\"demo.svc.sig\" = 10\n");
    assert!(with("RIDL-151", &diagnostics).is_empty(), "{diagnostics:?}");
    assert_eq!(with("RIDL-152", &diagnostics).len(), 1, "{diagnostics:?}");
}

#[test]
fn a_short_name_shared_by_two_shapes_is_ambiguous() {
    let source = "package demo\ntype Payload: bytes [0..1000]\n\
                  service demo.a.svc {\n  signal sig: Payload @[10ms..100ms]\n}\n\
                  service demo.b.svc {\n  signal sig: Payload @[10ms..100ms]\n}\n";
    let diagnostics = check(source, "[limits.interaction]\n\"svc.sig\" = 10\n");
    let found = with("RIDL-152", &diagnostics);
    assert_eq!(found.len(), 1, "{diagnostics:?}");
    assert!(found[0].message.contains("ambiguous"), "{found:?}");
    // The entry applies to neither shape.
    assert!(with("RIDL-151", &diagnostics).is_empty(), "{diagnostics:?}");
    // A kind entry still applies to both.
    let diagnostics = check(source, "[limits.kind]\nsignal = 10\n");
    assert_eq!(with("RIDL-151", &diagnostics).len(), 2, "{diagnostics:?}");
}

#[test]
fn an_unmatched_name_is_ridl_152() {
    let demo = signal();
    let members = [
        Member {
            name: "demo",
            source: &demo,
            manifest: "[limits.interaction]\n\"Nope.sig\" = 10\n\"Iface.nope\" = 10\n\
                       \"demo.Iface.sig\" = 10\n\"Other.sig\" = 10\n",
        },
        Member {
            name: "other",
            source: "package other\ntype Payload: bytes [0..1000]\n\
                     interface Other {\n  signal sig: Payload @[10ms..100ms]\n}\n",
            manifest: "",
        },
    ];
    let result = run(&members, PROTO3);
    let found = with("RIDL-152", &result.diagnostics);
    assert_eq!(found.len(), 4, "{:?}", result.diagnostics);
    let mut keys = Vec::new();
    for diagnostic in &found {
        assert_eq!(diagnostic.severity, Severity::Error);
        let path = result.sources.path(diagnostic.primary.file).unwrap();
        assert!(path.ends_with("demo/ridl.toml"), "{path}");
        let text = result.sources.text(diagnostic.primary.file).unwrap();
        let range = diagnostic.primary.range;
        keys.push(
            text[usize::from(range.start())..usize::from(range.end())]
                .trim_matches('"')
                .to_string(),
        );
    }
    keys.sort();
    assert_eq!(
        keys,
        ["Iface.nope", "Nope.sig", "Other.sig", "demo.Iface.sig"]
    );
    // A valid key is not reported.
    let diagnostics = check(&signal(), "[limits.interaction]\n\"Iface.sig\" = 5000\n");
    assert!(with("RIDL-152", &diagnostics).is_empty(), "{diagnostics:?}");
}

#[test]
fn a_root_table_does_not_apply_to_members() {
    let demo = signal();
    let result = run(
        &[Member {
            name: "demo",
            source: &demo,
            manifest: "",
        }],
        &format!("{PROTO3}[limits]\ndefault = 1\n[limits.interaction]\n\"Iface.sig\" = 1\n"),
    );
    assert!(
        with("RIDL-151", &result.diagnostics).is_empty(),
        "{:?}",
        result.diagnostics
    );
    assert!(
        with("RIDL-152", &result.diagnostics).is_empty(),
        "{:?}",
        result.diagnostics
    );
}

#[test]
fn only_enabled_encodings_are_checked() {
    // 1,000 bytes of `bytes` is more than N in FlatBuffers, so a limit of N
    // holds in proto3 alone.
    let limits = format!("[limits]\ndefault = {N}\n");
    let source = signal();
    let member = || Member {
        name: "demo",
        source: &source,
        manifest: &limits,
    };
    let proto3 = run(&[member()], PROTO3);
    assert!(
        with("RIDL-151", &proto3.diagnostics).is_empty(),
        "{:?}",
        proto3.diagnostics
    );
    let flatbuffers = run(
        &[member()],
        "[backend.rust]\nencodings = [\"flatbuffers\"]\n",
    );
    let found = with("RIDL-151", &flatbuffers.diagnostics);
    assert_eq!(found.len(), 1, "{:?}", flatbuffers.diagnostics);
    assert!(found[0].message.contains("flatbuffers"), "{found:?}");
    assert!(!found[0].message.contains("proto3"), "{found:?}");
}

#[test]
fn a_package_with_an_error_is_not_examined() {
    let source = format!("{}\nstruct Broken {{ x: Missing }}\n", signal());
    let diagnostics = check(&source, "[limits]\ndefault = 1\n");
    assert!(with("RIDL-151", &diagnostics).is_empty(), "{diagnostics:?}");
}
