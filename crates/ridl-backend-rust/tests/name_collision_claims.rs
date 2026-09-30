//! The claim tables of the generated-name collision design (stage (c),
//! driftsys/ridl#449, #453, #455): two names derived from ridl names that one
//! Rust namespace cannot hold are refused at `ridl build`, with one message
//! that names the generated name and both sources.
//!
//! Each case is an experiment of that design's appendix. The source checks
//! clean with `ridlc::check_source`, which runs no backend — so the refusal is
//! the backend's, not the language's — and `generate_pipeline`, the entry point `ridl build --emit
//! rust` calls, returns a `GenerateError` whose message holds the generated
//! name and each source. Without the claim tables every one of these sources
//! emits a crate that rustc rejects (E0124 or E0428), except X-7a and X-7b,
//! which the lowering's tuple collision already refused, and X-11, which the
//! FlatBuffers projection refused with a message that named neither source.
//!
//! The skip rule (decision 13) is pinned beside them: an interface whose face
//! the pipeline skips emits no descriptors and no face module, so its members
//! claim nothing (X-8b builds), and the same members with a call the face
//! carries are refused (X-8c). X-8b's compile proof is in
//! `name_collision_compile.rs`.

use ridl_backend_rust::{WireEncoding, generate_pipeline};

/// Requires that `source` checks with no error, and returns the message of
/// the `GenerateError` the pipeline entry point returns for it, panicking
/// when the pipeline succeeds.
///
/// The check is `ridlc::check_source`, which runs no backend.
/// `ridlc::compile` is not used for it, because it runs `generate` and folds
/// that backend's refusal into its diagnostics; it is used only for the IR.
fn refusal(name: &str, source: &str) -> String {
    let path = format!("{name}.ridl");
    let checked = ridlc::check_source(&path, source);
    let errors: Vec<_> = checked
        .diagnostics
        .iter()
        .filter(|diagnostic| matches!(diagnostic.severity, ridl_core::diag::Severity::Error))
        .collect();
    assert!(
        errors.is_empty(),
        "{name}.ridl must check clean, so the refusal is the backend's, got: {errors:?}"
    );
    let output = ridlc::compile(&path, source);
    match generate_pipeline(&output.package, WireEncoding::FlatBuffers, &[]) {
        Ok(generated) => panic!(
            "{name}.ridl must be refused by the claim table, but the pipeline emitted:\n{}",
            generated.rust_source
        ),
        Err(err) => err.message,
    }
}

/// Asserts that `message` holds every one of `parts`.
fn names_all(message: &str, parts: &[&str]) {
    for part in parts {
        assert!(
            message.contains(part),
            "the refusal must name {part:?}, got: {message}"
        );
    }
    assert!(
        message.contains("rename one of them"),
        "the refusal must say what to do, got: {message}"
    );
}

/// X-1d: members `self` and `self_` are both `Self` under `camel_case`, so
/// both descriptors are `CabinSelf`.
#[test]
fn two_members_equal_under_camel_case_after_the_escape_are_refused() {
    let message = refusal(
        "x01d",
        "package probe.x01d

type Level : integer [0..100]

interface Cabin {
  signal self : Level @10ms
  signal self_ : Level @10ms
}
",
    );
    names_all(
        &message,
        &[
            "`CabinSelf`",
            "member `self`",
            "member `self_`",
            "interface `Cabin`",
        ],
    );
}

/// X-6a: `minSpeed` and `min_speed` are both the Rust field `min_speed` of
/// the induced tuple `ReadingBounds` (driftsys/ridl#449).
#[test]
fn two_tuple_fields_equal_under_snake_case_are_refused() {
    let message = refusal(
        "x06a",
        "package probe.x06

type Speed : integer [0..250]

struct Reading {
  bounds : (minSpeed : Speed, min_speed : Speed)
}
",
    );
    names_all(
        &message,
        &[
            "`min_speed`",
            "field `minSpeed`",
            "field `min_speed`",
            "`ReadingBounds`",
            "`Reading.bounds`",
        ],
    );
}

/// X-7a: two nested tuple fields of one tuple, `XY` and `x_y`, both induce
/// the struct `STXY`, with different shapes. The lowering's tuple collision
/// finds the pair; the message is the one the backend gave it before the
/// claim tables, and names both shapes (driftsys/ridl#453).
#[test]
fn two_nested_tuples_equal_under_camel_case_are_refused() {
    let message = refusal(
        "x07a",
        "package probe.x07a

type Counter : integer [0..100]

struct S {
  t : (XY : (a : Counter), x_y : (b : Counter))
}
",
    );
    for part in ["STXY", "(a)", "(b)"] {
        assert!(message.contains(part), "must name {part:?}, got: {message}");
    }
}

/// X-7b: the struct fields `XY` and `x_y` both induce the struct `SXY`, with
/// different shapes (driftsys/ridl#453).
#[test]
fn two_struct_fields_whose_tuples_are_equal_under_camel_case_are_refused() {
    let message = refusal(
        "x07b",
        "package probe.x07b

type Counter : integer [0..100]

struct S {
  XY : (a : Counter)
  x_y : (b : Counter)
}
",
    );
    for part in ["SXY", "(a)", "(b)"] {
        assert!(message.contains(part), "must name {part:?}, got: {message}");
    }
}

/// X-8: members `XY` and `x_y` are both `XY` under `camel_case`, so both
/// descriptors are `CabinXY` (driftsys/ridl#455).
#[test]
fn two_members_equal_under_camel_case_are_refused() {
    let message = refusal(
        "x08",
        "package probe.x08

type Level : integer [0..100]

interface Cabin {
  signal XY : Level @10ms
  signal x_y : Level @10ms
}
",
    );
    names_all(
        &message,
        &[
            "`CabinXY`",
            "member `XY`",
            "member `x_y`",
            "interface `Cabin`",
        ],
    );
}

/// X-8b: the same two members beside a call the face cannot carry. The
/// pipeline skips the interface's face and descriptors (E11.14 decision 2),
/// so the members claim nothing and the package builds (decision 13).
#[test]
fn a_skipped_face_claims_nothing() {
    let source = "package probe.x08b

type Level : integer [0..100]

interface Cabin {
  signal XY : Level @10ms
  signal x_y : Level @10ms
  command set(a: Level, b: Level) @[..50ms]
}
";
    let output = ridlc::compile("x08b.ridl", source);
    let generated = generate_pipeline(&output.package, WireEncoding::FlatBuffers, &[])
        .expect("a skipped face claims nothing, so the package builds");
    assert!(
        generated.rust_source.contains("__RIDL_NO_FACE_Cabin"),
        "the face must be skipped, or this test proves nothing:\n{}",
        generated.rust_source
    );
    assert!(
        !generated.rust_source.contains("CabinXY"),
        "a skipped face emits no descriptor"
    );
}

/// X-8c: X-8b with a call the face carries. The face and the descriptors are
/// emitted, so the pair is refused.
#[test]
fn an_emitted_face_claims_its_members() {
    let message = refusal(
        "x08c",
        "package probe.x08c

type Level : integer [0..100]

interface Cabin {
  signal XY : Level @10ms
  signal x_y : Level @10ms
  command set(level: Level) @[..50ms]
}
",
    );
    names_all(&message, &["`CabinXY`", "member `XY`", "member `x_y`"]);
}

/// X-9: the descriptor of member `temperature` of interface `Cabin` is
/// `CabinTemperature`, which a declaration already names.
#[test]
fn a_descriptor_named_like_a_declaration_is_refused() {
    let message = refusal(
        "x09",
        "package probe.x09

type CabinTemperature : integer [-40..85]

interface Cabin {
  signal temperature : CabinTemperature @10ms
}
",
    );
    names_all(
        &message,
        &[
            "`CabinTemperature`",
            "declaration `CabinTemperature`",
            "member `temperature`",
            "interface `Cabin`",
        ],
    );
}

/// X-10: the FlatBuffers view of `Level` is `LevelFbView`, which a
/// declaration already names.
#[test]
fn a_view_named_like_a_declaration_is_refused() {
    let message = refusal(
        "x10",
        "package probe.x10

type Level : integer [0..100]

struct LevelFbView {
  level : Level
}
",
    );
    names_all(
        &message,
        &[
            "`LevelFbView`",
            "declaration `LevelFbView`",
            "declaration `Level`",
            "`FbView`",
        ],
    );
}

/// X-11: the tuple at `Reading.bounds` is `ReadingBounds`, which a declaration
/// already names. The refusal comes from the claim table, before the codec,
/// so it does not speak of the FlatBuffers projection.
#[test]
fn a_tuple_named_like_a_declaration_is_refused_before_the_codec() {
    let message = refusal(
        "x11",
        "package probe.x11

type Speed : integer [0..250]

struct Reading {
  bounds : (low : Speed, high : Speed)
}

struct ReadingBounds {
  low : Speed
}
",
    );
    names_all(
        &message,
        &[
            "`ReadingBounds`",
            "declaration `ReadingBounds`",
            "`Reading.bounds`",
        ],
    );
    assert!(
        !message.contains("FlatBuffers projection"),
        "the claim table runs before the codec, got: {message}"
    );
}

/// X-12: `A` + `BC` and `AB` + `C` are both `ABC`.
#[test]
fn two_descriptors_across_interfaces_are_refused() {
    let message = refusal(
        "x12",
        "package probe.x12

type Level : integer [0..100]

interface A {
  signal bC : Level @10ms
}

interface AB {
  signal c : Level @10ms
}
",
    );
    names_all(
        &message,
        &[
            "`ABC`",
            "member `bC`",
            "interface `A`",
            "member `c`",
            "interface `AB`",
        ],
    );
}

/// X-13: the descriptor of member `active` of interface `Horn` is
/// `HornActive`, which is also the descriptor of interface `HornActive`.
#[test]
fn a_descriptor_named_like_an_interface_is_refused() {
    let message = refusal(
        "x13",
        "package probe.x13

type Level : integer [0..100]

interface Horn {
  signal active : Level @10ms
}

interface HornActive {
  signal level : Level @10ms
}
",
    );
    names_all(
        &message,
        &[
            "`HornActive`",
            "member `active`",
            "interface `Horn`",
            "interface `HornActive`",
        ],
    );
}

/// X-14a: the face modules of `HTTPServer` and `HttpServer` are both
/// `http_server`.
#[test]
fn two_face_modules_equal_under_snake_case_are_refused() {
    let message = refusal(
        "x14a",
        "package probe.x14a

type Level : integer [0..100]

interface HTTPServer {
  signal level : Level @10ms
}

interface HttpServer {
  signal level : Level @10ms
}
",
    );
    names_all(
        &message,
        &[
            "`http_server`",
            "interface `HTTPServer`",
            "interface `HttpServer`",
        ],
    );
}

/// X-14b: the face module of interface `Cabin` is `cabin`, which a
/// declaration already names.
#[test]
fn a_face_module_named_like_a_declaration_is_refused() {
    let message = refusal(
        "x14b",
        "package probe.x14b

type cabin : integer [0..100]

interface Cabin {
  signal level : cabin @10ms
}
",
    );
    names_all(
        &message,
        &["`cabin`", "declaration `cabin`", "interface `Cabin`"],
    );
}

/// X-17: the unit struct `CabinLevel`, the descriptor of member `level`, is a
/// value as well as a type, and a constant already names that value.
#[test]
fn a_constant_named_like_a_descriptor_is_refused() {
    let message = refusal(
        "x17",
        "package probe.x17

type Level : integer [0..100]

const CabinLevel : Level = 5

interface Cabin {
  signal level : Level @10ms
}
",
    );
    names_all(
        &message,
        &[
            "`CabinLevel`",
            "declaration `CabinLevel`",
            "member `level`",
            "interface `Cabin`",
            "value namespace",
        ],
    );
}
