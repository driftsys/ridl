//! The claim tables of `docs/technotes/rust-backend-name-collisions.md` (stage (c),
//! driftsys/ridl#449, #453, #455): two names derived from ridl names that one
//! Rust namespace cannot hold are refused at `ridl build`, with one message
//! that names the generated name and both sources.
//!
//! Each case in the first part is an experiment of that design's appendix.
//! The source checks clean with `ridlc::check_source`, which runs no backend
//! — so the refusal is the backend's, not the language's — and
//! `generate_pipeline`, the entry point `ridl build --emit rust` calls,
//! returns a `GenerateError` whose message holds the generated name and each
//! source. Without the claim tables every one of these sources
//! emits a crate that rustc rejects (E0124 or E0428), except X-7a and X-7b,
//! which the lowering's tuple collision already refused, and X-11, which the
//! FlatBuffers projection refused with a message that named neither source.
//!
//! The skip rule (decision 13) is pinned beside them: an interface whose face
//! the pipeline skips emits no descriptors and no face module, so its members
//! claim nothing (X-8b builds), and the same members with a call the face
//! carries are refused (X-8c). X-8b's compile proof is in
//! `name_collision_compile.rs`.
//!
//! The second part is not from the appendix. It pins each claim on its own,
//! and pins which interfaces each entry point claims for, so that dropping a
//! claim, or claiming a name the backend does not emit, fails a test. Some of
//! its cases expect a refusal and some expect a build, and a few call
//! `generate`, `generate_with` or `generate_face` in place of the pipeline.

use ridl_backend_rust::{WireEncoding, generate, generate_face, generate_pipeline, generate_with};

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
/// pipeline skips the interface's face and descriptors (interaction-face decision 2),
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

// ---------------------------------------------------------------------------
// Each claim alone. The experiments above often meet in two namespaces at
// once — a descriptor is a type and a value — so one claim can hide that
// another is missing. Each refused source below meets its pair in one
// namespace only, so dropping that one claim lets the package through and
// fails the test. The entry-point tests and the tests that expect a build
// pin the other direction: a name the entry point does not emit is not
// claimed.
// ---------------------------------------------------------------------------

/// Requires that `source` checks clean and that the pipeline emits it, and
/// returns the emitted source.
fn builds(name: &str, source: &str) -> String {
    let path = format!("{name}.ridl");
    let checked = ridlc::check_source(&path, source);
    let errors: Vec<_> = checked
        .diagnostics
        .iter()
        .filter(|diagnostic| matches!(diagnostic.severity, ridl_core::diag::Severity::Error))
        .collect();
    assert!(
        errors.is_empty(),
        "{name}.ridl must check clean, got: {errors:?}"
    );
    let output = ridlc::compile(&path, source);
    generate_pipeline(&output.package, WireEncoding::FlatBuffers, &[])
        .unwrap_or_else(|err| panic!("{name}.ridl must build, got: {}", err.message))
        .rust_source
}

/// A braced struct is a type and no value, so a struct named like a member's
/// descriptor meets it in the type namespace only.
#[test]
fn a_struct_named_like_a_descriptor_is_refused_in_the_type_namespace() {
    let message = refusal(
        "struct_descriptor",
        "package probe.structdesc

type Level : integer [0..100]

struct CabinTemperature {
  level : Level
}

interface Cabin {
  signal temperature : Level @10ms
}
",
    );
    names_all(
        &message,
        &[
            "`CabinTemperature`",
            "type namespace",
            "declaration `CabinTemperature`",
            "member `temperature`",
        ],
    );
}

/// An enum and a union are types and no value; each named like a member's
/// descriptor is refused in the type namespace.
#[test]
fn an_enum_or_a_union_named_like_a_descriptor_is_refused() {
    let enum_message = refusal(
        "enum_descriptor",
        "package probe.enumdesc

type Level : integer [0..100]

enum CabinMode {
  OFF = 0
  ON = 1
}

interface Cabin {
  signal mode : Level @10ms
}
",
    );
    names_all(
        &enum_message,
        &["`CabinMode`", "type namespace", "declaration `CabinMode`"],
    );

    let union_message = refusal(
        "union_descriptor",
        "package probe.uniondesc

type Level : integer [0..100]

union CabinMode {
  on : Level
  off : Level
}

interface Cabin {
  signal mode : Level @10ms
}
",
    );
    names_all(
        &union_message,
        &["`CabinMode`", "type namespace", "declaration `CabinMode`"],
    );
}

/// An enum set is a tuple struct, a type and a value. A face module is a type
/// only, so an enum set named like one is refused by the enum set's type
/// claim alone.
#[test]
fn an_enum_set_named_like_a_face_module_is_refused() {
    let message = refusal(
        "enumset_module",
        "package probe.enumsetmod

type Level : integer [0..100]

enumset cabin {
  LOW = 0
}

interface Cabin {
  signal level : Level @10ms
}
",
    );
    names_all(
        &message,
        &[
            "`cabin`",
            "type namespace",
            "declaration `cabin`",
            "the face module of interface `Cabin`",
        ],
    );
}

/// An induced tuple is a type and no value. The tuple at `Ho.rn` is `HoRn`,
/// which is also the name of the descriptor of interface `HoRn`. That
/// descriptor is a type and a value, so the two meet in the type namespace
/// only, and the tuple's type claim and the interface's type claim are each
/// needed.
#[test]
fn a_tuple_named_like_an_interface_is_refused() {
    let message = refusal(
        "tuple_interface",
        "package probe.tupleiface

type Level : integer [0..100]

struct Ho {
  rn : (a : Level, b : Level)
}

interface HoRn {
  signal level : Level @10ms
}
",
    );
    names_all(
        &message,
        &[
            "`HoRn`",
            "type namespace",
            "the tuple at field path `Ho.rn`",
            "the descriptor of interface `HoRn`",
        ],
    );
}

/// The view of the tuple at `Reading.bounds` is `ReadingBoundsFbView`, which a
/// declaration already names.
#[test]
fn a_declaration_named_like_a_tuple_view_is_refused() {
    let message = refusal(
        "tuple_view",
        "package probe.tupleview

type Speed : integer [0..250]

struct Reading {
  bounds : (low : Speed, high : Speed)
}

struct ReadingBoundsFbView {
  low : Speed
}
",
    );
    names_all(
        &message,
        &[
            "`ReadingBoundsFbView`",
            "declaration `ReadingBoundsFbView`",
            "the FlatBuffers view of the tuple at field path `Reading.bounds`",
            "`FbView`",
        ],
    );
}

/// A nested tuple's field path has every segment: `S.t.u`, not `S.u`.
#[test]
fn a_nested_tuple_is_named_by_its_whole_field_path() {
    let message = refusal(
        "nested_tuple",
        "package probe.nested

type Speed : integer [0..250]

struct S {
  t : (u : (minSpeed : Speed, min_speed : Speed), v : Speed)
}
",
    );
    names_all(
        &message,
        &["`min_speed`", "`STU`", "the tuple at field path `S.t.u`"],
    );
}

/// `generate_face` emits every interface's descriptors and face, so it claims
/// their names as the pipeline does.
#[test]
fn generate_face_claims_the_interfaces_it_emits() {
    let output = ridlc::compile(
        "face_entry.ridl",
        "package probe.faceentry

type Level : integer [0..100]

interface Cabin {
  signal XY : Level @10ms
  signal x_y : Level @10ms
}
",
    );
    let err = generate_face(&output.package).expect_err("the pair is refused");
    for part in ["`CabinXY`", "member `XY`", "member `x_y`"] {
        assert!(
            err.message.contains(part),
            "must name {part:?}: {}",
            err.message
        );
    }
}

/// `generate` and `generate_with` emit no descriptor and no face, so an
/// interface's names claim nothing there, and the same source builds.
#[test]
fn generate_and_generate_with_claim_no_interface() {
    let output = ridlc::compile(
        "domain_entry.ridl",
        "package probe.domainentry

type Level : integer [0..100]

interface Cabin {
  signal XY : Level @10ms
  signal x_y : Level @10ms
}
",
    );
    let alone = generate(&output.package).expect("generate emits no interface");
    assert!(!alone.rust_source.contains("CabinXY"));
    let with = generate_with(&output.package, &[]).expect("generate_with emits no interface");
    assert!(!with.rust_source.contains("CabinXY"));
}

/// An interface whose members are all `fixed` has descriptors and no face
/// module, so its face module name claims nothing and a declaration of that
/// name builds.
#[test]
fn an_interface_with_no_face_module_claims_no_module_name() {
    let emitted = builds(
        "fixed_only",
        "package probe.fixedonly

type cabin : integer [0..100]

interface Cabin {
  fixed serial : cabin
}
",
    );
    assert!(
        !emitted.contains("pub mod cabin"),
        "the interface must emit no face module, or this test proves nothing:\n{emitted}"
    );
}

/// An interface that declares only a command, or only a query, has a face
/// module, which claims its name.
#[test]
fn a_call_only_interface_claims_its_module_name() {
    let command_message = refusal(
        "command_only",
        "package probe.commandonly

type cabin : integer [0..100]

interface Cabin {
  command set(level: cabin) @[..50ms]
}
",
    );
    names_all(
        &command_message,
        &[
            "`cabin`",
            "declaration `cabin`",
            "the face module of interface `Cabin`",
        ],
    );

    let query_message = refusal(
        "query_only",
        "package probe.queryonly

type cabin : integer [0..100]

interface Cabin {
  query read(level: cabin): cabin @[..50ms]
}
",
    );
    names_all(
        &query_message,
        &[
            "`cabin`",
            "declaration `cabin`",
            "the face module of interface `Cabin`",
        ],
    );
}

/// An interface that declares no member has descriptors and no face module,
/// so its face module name claims nothing and a declaration of that name
/// builds.
#[test]
fn an_interface_with_no_member_claims_no_module_name() {
    let emitted = builds(
        "no_member",
        "package probe.nomember

type cabin : integer [0..100]

interface Cabin {
}
",
    );
    assert!(
        !emitted.contains("pub mod cabin"),
        "the interface must emit no face module, or this test proves nothing:\n{emitted}"
    );
}

/// An interface that declares only an event has a face module, which claims
/// its name.
#[test]
fn an_event_only_interface_claims_its_module_name() {
    let message = refusal(
        "event_only",
        "package probe.eventonly

type cabin : integer [0..100]

interface Cabin {
  event warn : cabin @[100ms..1s]
}
",
    );
    names_all(
        &message,
        &[
            "`cabin`",
            "declaration `cabin`",
            "the face module of interface `Cabin`",
        ],
    );
}
