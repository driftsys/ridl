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
    run_with(members, root_extra, &[])
}

/// [`run`], with more files in the directory of the first member, given by
/// path relative to that directory.
fn run_with(members: &[Member<'_>], root_extra: &str, more: &[(&str, &str)]) -> ridlc::CliRun {
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
    for (path, text) in more {
        let file = root.path().join(members[0].name).join(path);
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(file, text).unwrap();
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

/// With no `[limits]` table, a payload above the default ceiling of its kind
/// is RIDL-157 (`payload_ceiling.rs`), and neither RIDL-151 nor RIDL-152.
#[test]
fn no_limits_table_reports_ridl_157_alone() {
    let source = "package demo\ntype Text: string [0..4032]\n\
                  struct Payload { a: Text, b: Text }\n\
                  interface Iface {\n  signal sig: Payload @[10ms..100ms]\n}\n";
    let diagnostics = check(source, "");
    assert_eq!(with("RIDL-157", &diagnostics).len(), 1, "{diagnostics:?}");
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

/// The podbuf size of `signal()`'s payload is the preamble, the 8-byte head
/// that holds the descriptor and the 1,000 bytes: 1,016 bytes.
#[test]
fn the_declared_limit_reads_the_podbuf_size_when_the_manifest_names_podbuf() {
    let podbuf = "[backend.rust]\nencodings = [\"podbuf\"]\n";
    let members = |limits| {
        run(
            &[Member {
                name: "demo",
                source: &signal(),
                manifest: limits,
            }],
            podbuf,
        )
        .diagnostics
    };
    let at = members("[limits.interaction]\n\"Iface.sig\" = 1016\n");
    assert!(with("RIDL-151", &at).is_empty(), "{at:?}");
    let below = members("[limits.interaction]\n\"Iface.sig\" = 1015\n");
    let found = with("RIDL-151", &below);
    assert_eq!(found.len(), 1, "{below:?}");
    assert!(
        found[0].message.contains("up to 1016 bytes in podbuf"),
        "{}",
        found[0].message
    );
}

#[test]
fn resolution_order_is_name_then_kind_then_default() {
    let source = "package demo\ntype Payload: bytes [0..1000]\n\
                  interface Iface {\n  signal sig: Payload @[10ms..100ms]\n  \
                  event first: Payload @[10ms..100ms]\n  \
                  event second: Payload @[10ms..100ms]\n}\n";
    let limits = "[limits]\ndefault = 1\n[limits.kind]\nsignal = 3000\n\
                  [limits.interaction]\n\"Iface.first\" = 3000\n";
    let diagnostics = check(source, limits);
    let found = with("RIDL-151", &diagnostics);
    assert_eq!(found.len(), 1, "{diagnostics:?}");
    assert!(found[0].message.contains("`Iface.second`"), "{found:?}");
    // The name beats the kind and the default, in the lowering direction too.
    let limits = "[limits]\ndefault = 10000\n[limits.kind]\nsignal = 3000\n\
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
fn a_fixed_is_checked_against_a_declared_limit() {
    let source = "package demo\ntype Text: string [0..4032]\n\
                  struct Payload { a: Text, b: Text }\ntype Small: bytes [0..10]\n\
                  interface Iface {\n  fixed big: Payload\n  fixed small: Small\n}\n";
    // No table: a fixed of 32,000 bytes is within its default ceiling of
    // 65,280 bytes and draws nothing.
    let diagnostics = check(source, "");
    assert!(with("RIDL-157", &diagnostics).is_empty(), "{diagnostics:?}");
    assert!(with("RIDL-151", &diagnostics).is_empty(), "{diagnostics:?}");
    // A table with only `default` sizes the fixed.
    let diagnostics = check(source, "[limits]\ndefault = 16256\n");
    let found = with("RIDL-151", &diagnostics);
    assert_eq!(found.len(), 1, "{diagnostics:?}");
    assert!(found[0].message.contains("`Iface.big`"), "{found:?}");
    assert!(found[0].message.starts_with("the value of "), "{found:?}");
    assert!(with("RIDL-157", &diagnostics).is_empty(), "{diagnostics:?}");
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
fn a_payload_over_the_limit_in_a_disabled_encoding_is_not_reported() {
    // A list of 100 wide integers: the proto3 bound is 503
    // bytes and the FlatBuffers bound is 457, so a limit of 480 is broken in
    // proto3 alone.
    let source = "package demo\ntype Wide: integer [-1000000000..1000000000]\n\
                  struct Payload { rows: [Wide; 0..100] }\n\
                  interface Iface {\n  signal sig: Payload @[10ms..100ms]\n}\n";
    let member = || Member {
        name: "demo",
        source,
        manifest: "[limits]\ndefault = 480\n",
    };
    let flatbuffers = run(
        &[member()],
        "[backend.rust]\nencodings = [\"flatbuffers\"]\n",
    );
    assert!(
        with("RIDL-151", &flatbuffers.diagnostics).is_empty(),
        "{:?}",
        flatbuffers.diagnostics
    );
    let proto3 = run(&[member()], PROTO3);
    let found = with("RIDL-151", &proto3.diagnostics);
    assert_eq!(found.len(), 1, "{:?}", proto3.diagnostics);
    assert!(
        found[0].message.contains("503 bytes in proto3"),
        "{found:?}"
    );
}

#[test]
fn a_package_with_an_error_is_not_examined() {
    let source = format!("{}\nstruct Broken {{ x: Missing }}\n", signal());
    let diagnostics = check(&source, "[limits]\ndefault = 1\n");
    assert!(with("RIDL-151", &diagnostics).is_empty(), "{diagnostics:?}");
}

#[test]
fn a_package_in_a_subdirectory_of_the_unit_is_checked() {
    // The unit `demo` holds the packages `demo` and `demo.sub`; its one table
    // applies to both, and an entry may name an interface of either.
    let main = "package demo\ntype Payload: bytes [0..1000]\n\
                interface Top {\n  signal sig: Payload @[10ms..100ms]\n}\n";
    let sub = "package demo.sub\ntype Payload: bytes [0..1000]\n\
               interface Low {\n  signal sig: Payload @[10ms..100ms]\n}\n";
    let member = |manifest| Member {
        name: "demo",
        source: main,
        manifest,
    };
    let result = run_with(
        &[member("[limits]\ndefault = 64\n")],
        PROTO3,
        &[("sub/s.ridl", sub)],
    );
    let found = with("RIDL-151", &result.diagnostics);
    assert_eq!(found.len(), 2, "{:?}", result.diagnostics);
    assert!(
        found.iter().any(|d| d.message.contains("`Low.sig`")),
        "{found:?}"
    );
    let table = "[limits.interaction]\n\"Low.sig\" = 5000\n\"Top.sig\" = 5000\n";
    let result = run_with(&[member(table)], PROTO3, &[("sub/s.ridl", sub)]);
    assert!(
        with("RIDL-151", &result.diagnostics).is_empty()
            && with("RIDL-152", &result.diagnostics).is_empty(),
        "{:?}",
        result.diagnostics
    );
}

#[test]
fn a_manifest_error_of_one_member_does_not_stop_the_checks_of_another() {
    let source_a = signal().replace("package demo", "package a");
    let source_b = signal().replace("package demo", "package b");
    let members = [
        Member {
            name: "a",
            source: &source_a,
            manifest: "[limits]\ndefault = 64\n[limits.kind]\nstream = 1\n",
        },
        Member {
            name: "b",
            source: &source_b,
            manifest: "[limits]\ndefault = 64\n",
        },
    ];
    let result = run(&members, PROTO3);
    assert_eq!(with("MANI-021", &result.diagnostics).len(), 1);
    // Member `a` has a usable limit too, but its manifest error stops its own
    // checks: only `b` is reported.
    let found = with("RIDL-151", &result.diagnostics);
    assert_eq!(found.len(), 1, "{:?}", result.diagnostics);
    let path = result.sources.path(found[0].primary.file).unwrap();
    assert!(path.ends_with("b/m.ridl"), "{path}");
}

#[test]
fn the_event_kind_has_its_own_limit() {
    let source = "package demo\ntype Payload: bytes [0..1000]\n\
                  interface Iface {\n  signal sig: Payload @[10ms..100ms]\n  \
                  event ev: Payload @[10ms..100ms]\n}\n";
    let limits = format!("[limits.kind]\nevent = {}\n", N - 1);
    let found = with("RIDL-151", &check(source, &limits));
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].message.contains("`Iface.ev`"), "{found:?}");
}

#[test]
fn a_unit_with_limits_and_no_interface_reports_each_entry() {
    let source = "package demo\ntype Payload: bytes [0..1000]\n";
    let limits = "[limits.interaction]\n\"Iface.a\" = 10\n\"Iface.b\" = 10\n";
    let diagnostics = check(source, limits);
    assert_eq!(with("RIDL-152", &diagnostics).len(), 2, "{diagnostics:?}");
}

/// Where an entry gives the limit, RIDL-151 alone applies: a signal above its
/// default ceiling under a declared limit draws no RIDL-157.
#[test]
fn a_payload_above_the_ceiling_under_a_declared_limit_draws_ridl_151_alone() {
    let source = "package demo\ntype Text: string [0..4032]\n\
                  struct Payload { a: Text, b: Text }\n\
                  interface Iface {\n  signal sig: Payload @[10ms..100ms]\n}\n";
    let diagnostics = check(source, "[limits]\ndefault = 16256\n");
    assert!(with("RIDL-157", &diagnostics).is_empty(), "{diagnostics:?}");
    assert_eq!(with("RIDL-151", &diagnostics).len(), 1, "{diagnostics:?}");
}

#[test]
fn a_fixed_is_sized_in_the_enabled_encoding() {
    // The proto3 bound is 503 bytes and the FlatBuffers bound is 457: a limit
    // of 480 is broken in proto3 alone.
    let source = "package demo\ntype Wide: integer [-1000000000..1000000000]\n\
                  struct Payload { rows: [Wide; 0..100] }\n\
                  interface Iface {\n  fixed table: Payload\n}\n";
    let member = || Member {
        name: "demo",
        source,
        manifest: "[limits]\ndefault = 480\n",
    };
    let flatbuffers = run(
        &[member()],
        "[backend.rust]\nencodings = [\"flatbuffers\"]\n",
    );
    assert!(with("RIDL-151", &flatbuffers.diagnostics).is_empty());
    let found = with("RIDL-151", &run(&[member()], PROTO3).diagnostics);
    assert_eq!(found.len(), 1);
    assert!(
        found[0].message.contains("503 bytes in proto3"),
        "{found:?}"
    );
}

#[test]
fn a_fallible_reply_names_the_type_that_breaks_the_limit() {
    let source = "package demo\ntype Text: string [0..4032]\n\
                  error struct Failure { a: Text }\nstruct Small { n: boolean }\n\
                  interface Iface {\n  query get(): Small | Failure @[..1s]\n}\n";
    let found = with("RIDL-151", &check(source, "[limits]\ndefault = 100\n"));
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].message.contains("for `Failure`"), "{found:?}");
}

#[test]
fn a_skipped_package_of_a_unit_draws_no_unmatched_name() {
    // `Low` is declared in `demo.sub`, which has an error, so it is not
    // examined and its interfaces are unknown: the entry is not unmatched.
    let main = signal();
    let sub = "package demo.sub\nstruct Broken { x: Missing }\n\
               interface Low {\n  signal sig: Broken @[10ms..100ms]\n}\n";
    let member = Member {
        name: "demo",
        source: &main,
        manifest: "[limits.interaction]\n\"Low.sig\" = 10\n",
    };
    let result = run_with(&[member], PROTO3, &[("sub/s.ridl", sub)]);
    assert!(
        with("RIDL-152", &result.diagnostics).is_empty(),
        "{:?}",
        result.diagnostics
    );
}

#[test]
fn a_short_name_shared_by_two_packages_of_a_unit_is_ambiguous() {
    let main = "package demo\ntype Payload: bytes [0..1000]\n\
                interface Same {\n  signal sig: Payload @[10ms..100ms]\n}\n";
    let sub = "package demo.sub\ntype Payload: bytes [0..1000]\n\
               interface Same {\n  signal sig: Payload @[10ms..100ms]\n}\n";
    let member = Member {
        name: "demo",
        source: main,
        manifest: "[limits.interaction]\n\"Same.sig\" = 10\n",
    };
    let result = run_with(&[member], PROTO3, &[("sub/s.ridl", sub)]);
    let found = with("RIDL-152", &result.diagnostics);
    assert_eq!(found.len(), 1, "{:?}", result.diagnostics);
    assert!(
        found[0]
            .message
            .contains("ambiguous: more than one interface or inline service of the unit"),
        "{found:?}"
    );
    assert!(
        with("RIDL-151", &result.diagnostics).is_empty(),
        "{:?}",
        result.diagnostics
    );
}

#[test]
fn a_tombstone_is_not_an_interaction_of_the_unit() {
    let source = "package demo\ntype Payload: bytes [0..1000]\n\
                  interface Iface {\n  signal sig: Payload @[10ms..100ms]\n  reserved old\n}\n";
    let diagnostics = check(source, "[limits.interaction]\n\"Iface.\" = 10\n");
    assert_eq!(with("RIDL-152", &diagnostics).len(), 1, "{diagnostics:?}");
}

#[test]
fn the_unmatched_name_message_names_the_unit() {
    let diagnostics = check(&signal(), "[limits.interaction]\n\"Nope.sig\" = 10\n");
    let found = with("RIDL-152", &diagnostics);
    assert_eq!(found.len(), 1, "{diagnostics:?}");
    assert!(
        found[0]
            .message
            .contains("names no interaction of its unit"),
        "{found:?}"
    );
}

#[test]
fn a_manifest_error_of_one_member_does_not_stop_ridl_157_in_another() {
    // Member `a` has a manifest error; member `b` has a payload above the
    // ceiling and no limits. The size pass skips only the packages beneath the
    // manifest, so `b` draws RIDL-157.
    let big = "package b\ntype Text: string [0..4032]\n\
               struct Payload { a: Text, b: Text }\n\
               interface Iface {\n  signal sig: Payload @[10ms..100ms]\n}\n";
    let small = signal().replace("package demo", "package a");
    let members = [
        Member {
            name: "a",
            source: &small,
            manifest: "[limits.kind]\nstream = 1\n",
        },
        Member {
            name: "b",
            source: big,
            manifest: "",
        },
    ];
    let result = run(&members, PROTO3);
    assert_eq!(with("MANI-021", &result.diagnostics).len(), 1);
    let found = with("RIDL-157", &result.diagnostics);
    assert_eq!(found.len(), 1, "{:?}", result.diagnostics);
    let path = result.sources.path(found[0].primary.file).unwrap();
    assert!(path.ends_with("b/m.ridl"), "{path}");
}
