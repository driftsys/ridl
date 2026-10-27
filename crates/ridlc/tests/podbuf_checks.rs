//! RIDL-153, RIDL-154 and RIDL-155: the build-time errors of the podbuf
//! encoding. Each is raised only when a `[backend.<name>]` table of the
//! workspace manifest names `podbuf` in its `encodings`.

use ridl_core::Frozen;
use ridl_core::diag::Diagnostic;

const PODBUF: &str = "[backend.rust]\nencodings = [\"podbuf\"]\n";
const FLATBUFFERS: &str = "[backend.rust]\nencodings = [\"flatbuffers\"]\n";

/// The diagnostics of `ridl check` over a one-member workspace, the package
/// named by `source`, whose root manifest ends with `extra`.
fn check(source: &str, extra: &str) -> Vec<Diagnostic> {
    let root = tempfile::tempdir().expect("a temp dir");
    std::fs::write(
        root.path().join("ridl.toml"),
        format!("[workspace]\nmembers = [\"demo\"]\n{extra}"),
    )
    .unwrap();
    let member = root.path().join("demo");
    std::fs::create_dir(&member).unwrap();
    std::fs::write(
        member.join("ridl.toml"),
        "[package]\nname = \"demo\"\nversion = \"1.0.0\"\n",
    )
    .unwrap();
    std::fs::write(member.join("m.ridl"), source).unwrap();
    ridlc::run_check(root.path(), Frozen::No)
        .unwrap()
        .diagnostics
}

/// The podbuf errors among `diagnostics`.
fn podbuf_errors(diagnostics: &[Diagnostic]) -> Vec<&Diagnostic> {
    diagnostics
        .iter()
        .filter(|diagnostic| {
            matches!(
                diagnostic.code.as_str(),
                "RIDL-153" | "RIDL-154" | "RIDL-155"
            )
        })
        .collect()
}

fn codes(diagnostics: &[Diagnostic]) -> Vec<&'static str> {
    podbuf_errors(diagnostics)
        .into_iter()
        .map(|diagnostic| diagnostic.code.as_str())
        .collect()
}

/// Two arrays of 65,535 elements of 60,000 bytes each: every descriptor is
/// within its limits and the type is about 7.8 GB, above `u32::MAX`.
const TOO_LARGE: &str = "package demo\n\
    type Blob: bytes [60000..60000]\n\
    struct Huge {\n  first: [Blob; 0..65535]\n  second: [Blob; 0..65535]\n}\n";

/// `count` has 70,000 elements and `wide` has an element of 70,000 bytes.
const DESCRIPTOR_LIMITS: &str = "package demo\n\
    type Flag: boolean\n\
    type Wide: bytes [70000..70000]\n\
    struct Limits {\n  count: [Flag; 0..70000]\n  wide: [Wide; 0..4]\n}\n";

/// `many` has 70,000 entries and `wide` has an entry of 70,000 bytes, both in
/// maps.
const MAP_LIMITS: &str = "package demo\n\
    type Wide: bytes [70000..70000]\n\
    struct Maps {\n  many: [integer : boolean; 0..70000]\n  wide: [integer : Wide; 0..2]\n}\n";

/// A deprecated field of a string type that cannot be empty: typl derives no
/// init value for it (§5.8), and the empty string is not a valid value.
const DEPRECATED: &str = "package demo\n\
    type Tag: string [1..8]\n\
    struct Dash {\n  /// @deprecated \"use gauge\"\n  tag: Tag\n  gear: integer [0..8]\n}\n";

#[test]
fn a_type_above_u32_max_bytes_is_ridl_153_naming_it() {
    let out = check(TOO_LARGE, PODBUF);
    assert_eq!(codes(&out), vec!["RIDL-153"], "{out:?}");
    assert!(
        podbuf_errors(&out)[0].message.contains("`demo.Huge`"),
        "{}",
        podbuf_errors(&out)[0].message
    );
}

/// `Big` is about 3.9 GB, inside `u32::MAX`; the argument struct a call with
/// two of them induces, and a tuple reply of two, are about 7.8 GB.
const TOO_LARGE_TUPLES: &str = "package demo\n\
    type Blob: bytes [60000..60000]\n\
    struct Big {\n  xs: [Blob; 0..65535]\n}\n\
    interface Ctl {\n  command load(a: Big, b: Big) @[..1s]\n  \
    query pair(): (a: Big, b: Big) @[..1s]\n  query one(): Big @[..1s]\n}\n";

#[test]
fn a_named_tuple_above_u32_max_bytes_is_ridl_153_naming_the_interaction() {
    let out = check(TOO_LARGE_TUPLES, PODBUF);
    assert_eq!(codes(&out), vec!["RIDL-153", "RIDL-153"], "{out:?}");
    let messages: Vec<&str> = podbuf_errors(&out)
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect();
    assert!(
        messages[0].starts_with("the request of `demo.Ctl.load` is too large for podbuf"),
        "{messages:?}"
    );
    assert!(
        messages[1].starts_with("the reply of `demo.Ctl.pair` is too large for podbuf"),
        "{messages:?}"
    );
}

#[test]
fn an_array_past_65535_elements_or_bytes_per_element_is_ridl_154() {
    let out = check(DESCRIPTOR_LIMITS, PODBUF);
    // Only the two fields draw: the struct itself is not also "too large".
    assert_eq!(codes(&out), vec!["RIDL-154", "RIDL-154"], "{out:?}");
    let messages: Vec<&str> = podbuf_errors(&out)
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect();
    assert!(
        messages.iter().any(|m| m.contains("`demo.Limits.count`")),
        "{messages:?}"
    );
    assert!(
        messages.iter().any(|m| m.contains("`demo.Limits.wide`")),
        "{messages:?}"
    );
}

#[test]
fn a_map_past_65535_entries_or_bytes_per_entry_is_ridl_154() {
    let out = check(MAP_LIMITS, PODBUF);
    assert_eq!(codes(&out), vec!["RIDL-154", "RIDL-154"], "{out:?}");
    let messages: Vec<&str> = podbuf_errors(&out)
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect();
    assert!(
        messages.iter().any(|m| m.contains("`demo.Maps.many`")),
        "{messages:?}"
    );
    assert!(
        messages.iter().any(|m| m.contains("`demo.Maps.wide`")),
        "{messages:?}"
    );
}

#[test]
fn a_deprecated_field_with_no_valid_value_is_ridl_155() {
    let out = check(DEPRECATED, PODBUF);
    assert_eq!(codes(&out), vec!["RIDL-155"], "{out:?}");
    assert!(
        podbuf_errors(&out)[0].message.contains("`demo.Dash.tag`"),
        "{}",
        podbuf_errors(&out)[0].message
    );
}

#[test]
fn a_deprecated_field_with_a_declared_init_draws_nothing() {
    let source = DEPRECATED.replace("tag: Tag\n", "tag: Tag = \"none\"\n");
    let out = check(&source, PODBUF);
    assert!(codes(&out).is_empty(), "{out:?}");
}

#[test]
fn a_deprecated_field_with_a_derivable_init_draws_nothing() {
    // The empty string is derivable when the bound admits length 0.
    let source = DEPRECATED.replace("[1..8]", "[0..8]");
    assert!(codes(&check(&source, PODBUF)).is_empty());
    // A numeric type that excludes 0 derives its lower bound (typl §5.8), a
    // valid value for the encoder to write.
    let numeric = "package demo\ntype Speed: km/h [1.0..250.0 step 0.5]\n\
        struct Dash {\n  /// @deprecated \"use gauge\"\n  speed: Speed\n  gear: integer [0..8]\n}\n";
    assert!(codes(&check(numeric, PODBUF)).is_empty());
}

#[test]
fn an_optional_deprecated_field_draws_nothing() {
    let source = DEPRECATED.replace("tag: Tag\n", "tag: Tag?\n");
    let out = check(&source, PODBUF);
    assert!(codes(&out).is_empty(), "{out:?}");
}

#[test]
fn a_field_that_is_not_deprecated_draws_nothing() {
    let source = DEPRECATED.replace("  /// @deprecated \"use gauge\"\n", "");
    let out = check(&source, PODBUF);
    assert!(codes(&out).is_empty(), "{out:?}");
}

#[test]
fn the_checks_are_silent_when_no_backend_names_podbuf() {
    for source in [TOO_LARGE, DESCRIPTOR_LIMITS, DEPRECATED] {
        assert!(codes(&check(source, FLATBUFFERS)).is_empty());
        assert!(codes(&check(source, "")).is_empty());
    }
}
