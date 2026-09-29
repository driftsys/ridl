//! Compile proofs that a ridl name equal to a name the backend chose does not
//! stop the emitted crate from compiling (the generated-name collision design,
//! driftsys/ridl#583, #587, #588, #423, #424).
//!
//! Each test is one namespace of that design's inventory. The source is
//! compiled with `ridlc::compile`, emitted through `generate_pipeline` — the
//! entry point `ridl build --emit rust` calls, so the face and the descriptors
//! are in what is checked — and checked with a bare `rustc` that links
//! `ridl-rt`, the way `face_compile.rs` checks the face. What is proven is that
//! the source compiles, not what it does.
//!
//! A source whose names are off-convention on purpose — a lowercase type, a
//! lowercase enum set bit, a type named `Self` — draws rustc's naming lints in
//! the consumer's build today, and a naming lint is not a collision, so such a
//! test allows the two lints and nothing else.

#[path = "support/rustc.rs"]
mod rustc;

use ridl_backend_rust::{WireEncoding, generate_pipeline};

/// The flags that silence the two naming lints an off-convention source draws.
const NAMING_LINTS: &[&str] = &["-A", "non_camel_case_types", "-A", "non_upper_case_globals"];

/// Emits `source` through the pipeline entry point and checks it as a library
/// crate (`--emit=metadata -D warnings`), with the `std` cfg on and off,
/// panicking with rustc's diagnostics when it does not compile. Returns the
/// emitted source, so a test can assert on its text as well.
fn pipeline_compiles(name: &str, source: &str) -> String {
    pipeline_compiles_with(name, source, "", &[])
}

/// [`pipeline_compiles`], with `consumer` — Rust source that uses the emitted
/// items — appended before the check, and `extra_args` handed to rustc.
fn pipeline_compiles_with(name: &str, source: &str, consumer: &str, extra_args: &[&str]) -> String {
    let mut emitted = String::new();
    for std in [true, false] {
        let (checked, compiled) = compile_pipeline(name, source, consumer, std, extra_args);
        assert!(
            compiled.status.success(),
            "the emitted crate must compile with the std cfg {}, rustc said:\n{}\nsource:\n{checked}",
            if std { "on" } else { "off" },
            String::from_utf8_lossy(&compiled.stderr)
        );
        emitted = checked;
    }
    emitted
}

/// One `rustc` check of the pipeline's output for `source` with `consumer`
/// appended, returning the source checked and rustc's output.
fn compile_pipeline(
    name: &str,
    source: &str,
    consumer: &str,
    std: bool,
    extra_args: &[&str],
) -> (String, std::process::Output) {
    let output = ridlc::compile(&format!("{name}.ridl"), source);
    let errors: Vec<_> = output
        .diagnostics
        .iter()
        .filter(|diagnostic| matches!(diagnostic.severity, ridl_core::diag::Severity::Error))
        .collect();
    assert!(
        errors.is_empty(),
        "{name}.ridl must compile with no error, got: {errors:?}"
    );
    let mut emitted = generate_pipeline(&output.package, WireEncoding::FlatBuffers, &[])
        .expect("generate_pipeline")
        .rust_source;
    emitted.push_str(consumer);

    let dir = tempfile::tempdir().expect("a temp dir is created");
    let source_path = dir.path().join(format!("{name}.rs"));
    std::fs::write(&source_path, &emitted).expect("the generated source is written");
    let ridl_rt = if std {
        rustc::ridl_rt_rlib(dir.path())
    } else {
        rustc::ridl_rt_rlib_without_std(dir.path())
    };
    let mut command = std::process::Command::new("rustc");
    command.args([
        "--edition",
        "2024",
        "--crate-type",
        "lib",
        "--emit=metadata",
        "-D",
        "warnings",
    ]);
    command.args(extra_args);
    if std {
        command.arg("--cfg").arg(r#"feature="std""#);
    }
    let compiled = command
        .arg("-o")
        .arg(dir.path().join(format!("lib{name}_std_{std}.rmeta")))
        .arg("--extern")
        .arg(format!("ridl_rt={}", ridl_rt.display()))
        .arg(&source_path)
        .output()
        .expect("rustc must be installed and runnable for this test to be meaningful");
    (emitted, compiled)
}

/// The codec round-trip fixture under another package name, plus an interface
/// whose calls carry a `require` and an `ensure` clause, so the clause
/// translator runs and the face is emitted: one declaration per shape the
/// codec carries, so every emitter that writes a prelude name or a primitive
/// is reached. The `fixed` member carries no timing, so the descriptor
/// emitter's `Fixed` arm writes its timing field as
/// `::core::option::Option::None` (`descriptors.rs`) — reached beside
/// `type None` in `declarations_named_like_prelude_names_compile`, the one
/// caller of this function that declares that name.
///
/// The mutation that proves this: change that site to the bare `None` in
/// `descriptors.rs`; `rustc` reports `error[E0308]: mismatched types` at
/// `timing: None,` — "expected `Option<Timing>`, found struct constructor
/// `fn(i64) -> None {None}`" — because `None` now resolves to the declared
/// newtype's tuple constructor rather than `Option::None`. Reverted after
/// confirming the failure.
fn base_package(package: &str) -> String {
    let fixture = include_str!("fixtures/flatbuffers_roundtrip.ridl").replacen(
        "package fb.demo",
        &format!("package {package}"),
        1,
    );
    format!(
        "{fixture}
interface Cabin {{
  signal speed : Speed @10ms
  event  warn  : Inner @[100ms..1s]
  command set(level: Speed) @[..50ms] [
    require level < 100
  ]
  query  avg(window: Count): Speed @[..50ms] [
    require window > 0
    ensure  result >= 0
  ]
  fixed serial : Count
}}
"
    )
}

/// A package may declare a type named like a prelude item the backend writes
/// unqualified — `Default`, `String`, `Vec`, `Option`, `Some`, `None` — and the
/// emitted crate compiles, because every such name is written by its
/// `::core::` or `::std::` path at package scope (design §4.2). `Result`, `Ok`,
/// `Err`, `From` and `TryFrom` compiled before and are kept as the control.
/// This is driftsys/ridl#424's recipe over the pipeline entry point.
#[test]
fn declarations_named_like_prelude_names_compile() {
    let declarations: String = [
        "Default", "String", "Vec", "Option", "Some", "None", "Result", "Ok", "Err", "From",
        "TryFrom",
    ]
    .iter()
    .map(|name| format!("type {name} : integer [0..100]\n"))
    .collect();
    let emitted = pipeline_compiles(
        "prelude_names",
        &format!("{}\n{declarations}", base_package("probe.prelude")),
    );
    // `base_package` gives `Cabin` a `require` and an `ensure` clause so the
    // face is emitted; a regression that skipped it instead (leaving a
    // `__RIDL_NO_FACE_` note in its place) would still pass every assertion
    // above, since none of them reads the face.
    assert!(
        emitted.contains("pub mod cabin"),
        "the interface's face module must be emitted, got:\n{emitted}"
    );
    assert!(
        !emitted.contains("__RIDL_NO_FACE_"),
        "the face must not be skipped, got:\n{emitted}"
    );
}

/// A package may declare a type named like a primitive the backend writes —
/// the thirteen of design §4.2 — and the emitted crate compiles, because the
/// backend writes `::core::primitive::<name>` at package scope. The base
/// reaches every integer width, so the codec writes every primitive it can
/// write; a width it does not reach would leave that primitive's sites
/// unproven. `Wide`'s highest bit is 40, which `enumset_width`
/// (`ridl-sem/src/scalar.rs`) backs at `U64`, the one width the four `W*`
/// types below do not reach: they stop at 32 bits (`U32w`), so without `Wide`
/// `codec::Scalar::widen`'s `Prim::U64` arm — the only one that casts rather
/// than widens (`#raw as ::core::primitive::i64`) — is never compiled here.
///
/// The mutation that proves `Wide` reaches that arm: change
/// `Prim::U64 => quote! { #raw as ::core::primitive::i64 }` to
/// `Prim::U64 => quote! { #raw as i64 }` (dropping the `::core::primitive`
/// path, leaving the cast itself intact so this stays a naming mutation, not
/// a behavior one) in `codec.rs`; `rustc` then reports
/// `` error[E0605]: non-primitive cast: `u64` as `i64` `` in the `Widths`
/// codec, because this package also declares `type i64`, and the bare path
/// resolves to that newtype instead of the primitive, so the cast is no
/// longer between two primitives. Reverted after confirming the failure.
///
/// `LOW`, typed `i64`, compiles `emit_const`'s named-type path
/// (`ridl-backend-rust/src/lib.rs`) beside `type i64`: the constant's type is
/// the bare path `i64`, which resolves to this package's declared scalar, not
/// the primitive. `RAW` and `RAW_STR`, typed with the plain typl primitives
/// `integer` and `string`, declare no named type at all, so they compile
/// `emit_const`'s backend-chosen-primitive path instead, which writes
/// `::core::primitive::i64` and `&::core::primitive::str` outright.
#[test]
fn declarations_named_like_primitives_compile() {
    let declarations: String = [
        "bool", "i8", "u8", "i16", "u16", "i32", "u32", "i64", "u64", "f32", "f64", "usize", "str",
    ]
    .iter()
    .map(|name| format!("type {name} : integer [0..100]\n"))
    .collect();
    let emitted = pipeline_compiles_with(
        "primitive_names",
        &format!(
            "{}
type W8 : integer [-10..10]
type W16 : integer [-1000..1000]
type W32 : integer [-100000..100000]
type U32w : integer [0..100000]

enumset Wide {{
  a = 0
  b = 40
}}

struct Widths {{
  a : W8
  b : W16
  c : W32
  d : U32w
  e : Wide
}}

{declarations}

const LOW : i64 = 5
const RAW : integer = 5
const RAW_STR : string = \"hi\"",
            base_package("probe.primitives")
        ),
        "",
        NAMING_LINTS,
    );
    // `base_package` gives `Cabin` a `require` and an `ensure` clause so the
    // face is emitted; see `declarations_named_like_prelude_names_compile`
    // above for why this must be asserted rather than left implicit.
    assert!(
        emitted.contains("pub mod cabin"),
        "the interface's face module must be emitted, got:\n{emitted}"
    );
    assert!(
        !emitted.contains("__RIDL_NO_FACE_"),
        "the face must not be skipped, got:\n{emitted}"
    );
}

/// An enum set bit named `default` is an inherent constant of the type, and a
/// path call `Flags::default()` resolves to it before the trait method. The
/// backend writes `<Flags as ::core::default::Default>::default()` at both
/// sites that build a default — a struct field's and a signal's `init` — so
/// the bit does not capture the call. The other bits are names of trait
/// items and inherent items the type carries, kept as the control: a trait
/// item has its own namespace.
#[test]
fn an_enum_set_bit_named_default_compiles() {
    pipeline_compiles_with(
        "enum_set_default",
        r#"
package probe.enumsetdefault

type Level : integer [0..100]

enumset Flags {
  try_from = 0
  new = 1
  MAX_SIZE = 2
  encode = 3
  decode = 4
  verify = 5
  check = 6
  default = 7
  View = 8
  Error = 9
  from = 10
  into = 11
  value = 12
  bytes_ = 13
  clone = 14
  eq = 15
  fmt = 16
  hash = 17
}

struct Holder {
  flags : Flags
  level : Level
}

interface Panel {
  signal flags : Flags @10ms
}
"#,
        "",
        NAMING_LINTS,
    );
}

/// A struct field, or a tuple field, whose `snake_case` is `bytes` gives the
/// view an accessor named `bytes` beside the fixed `bytes` that hands back the
/// verified buffer. The fixed one is a method of `ridl_rt::payload::View`, so
/// the two do not meet: the accessor is inherent and wins the dot call, and
/// the buffer is reached through the trait's path. The consumer proves both
/// resolutions have the type the design states (`bytes` is a typl keyword, so
/// the field is spelled `Bytes`; driftsys/ridl#587).
#[test]
fn a_field_whose_accessor_is_bytes_compiles() {
    pipeline_compiles_with(
        "bytes_accessor",
        r#"
package probe.bytes

type Level : integer [0..100]

struct Packet {
  Bytes : Level
}

struct Frame {
  t : (Bytes : Level, other : Level)
}
"#,
        r#"
pub fn packet_consumer(view: PacketFbView<'_>) -> (Level, usize) {
    let field: Level = view.bytes();
    let raw: &[u8] = ::ridl_rt::payload::View::bytes(&view);
    (field, raw.len())
}

pub fn tuple_consumer(view: FrameTFbView<'_>) -> (Level, usize) {
    let field: Level = view.bytes();
    let raw: &[u8] = ::ridl_rt::payload::View::bytes(&view);
    (field, raw.len())
}
"#,
        &[],
    );
}

/// A declaration named `Wire`, and an interface named `Wire`, each compile:
/// the package carries no `pub type Wire` alias any more, and every site that
/// named it writes `::ridl_rt::encoding::FlatBuffers` (design decision 5,
/// driftsys/ridl#588). No name is reserved.
#[test]
fn a_declaration_or_interface_named_wire_compiles() {
    let with_type = pipeline_compiles(
        "wire_type",
        r#"
package probe.wiretype

type Level : integer [0..100]
type Wire : integer [0..100]

interface Cabin {
  signal level : Wire @10ms
}
"#,
    );
    assert!(
        !with_type.contains("pub type Wire"),
        "the package carries no alias, got:\n{with_type}"
    );

    let with_interface = pipeline_compiles(
        "wire_interface",
        r#"
package probe.wireiface

type Level : integer [0..100]

interface Wire {
  signal level : Level @10ms
}
"#,
    );
    assert!(
        !with_interface.contains("pub type Wire"),
        "the package carries no alias, got:\n{with_interface}"
    );
}

/// The four keywords that cannot be raw identifiers are escaped with a
/// trailing underscore, and the escape is injective: a name that is one of
/// them followed by any number of underscores gets one more, so `self` is
/// `self_` and `self_` is `self__`. Before that, `self` and `self_` both
/// emitted `self_` — twice in one enum set (E0592), one struct (E0124), one
/// tuple (E0124), or one package (E0428); driftsys/ridl#583.
#[test]
fn a_keyword_and_its_escape_compile() {
    pipeline_compiles_with(
        "keyword_escape",
        r#"
package probe.keywords

type Level : integer [0..100]
type Self : integer [0..100]
type Self_ : integer [0..100]

enumset W {
  self = 0
  self_ = 1
  super = 2
  super_ = 3
}

struct S {
  self : Level
  self_ : Level
  crate : Level
  crate_ : Level
  t : (self : Level, self_ : Level)
}
"#,
        "",
        NAMING_LINTS,
    );
}

/// Two declarations, or two skipped interfaces, whose `snake_case` agrees are
/// distinct names of the package (TYPL-009 accepts them), and the backend's
/// internal names — `__ridl_fb_{encode,verify,decode}_*` for a declaration's
/// codec, `__RIDL_NO_FACE_*` for an interface whose face is skipped — are
/// spelled from the declared name rather than through that lossy transform,
/// with the naming lint allowed on the item. Before, each pair emitted one
/// internal name twice (E0428). The interfaces declare a call the face does
/// not carry, so that the skipped-face note is what they emit.
#[test]
fn names_equal_under_snake_case_compile() {
    pipeline_compiles(
        "snake_case_names",
        r#"
package probe.snake

type Level : integer [0..100]
type HTTPServer : integer [0..100]
type HttpServer : integer [0..100]

struct Both {
  a : HTTPServer
  b : HttpServer
}

interface XMLParser {
  command set(a: Level, b: Level) @[..50ms]
}

interface XmlParser {
  command set(a: Level, b: Level) @[..50ms]
}
"#,
    );
}
