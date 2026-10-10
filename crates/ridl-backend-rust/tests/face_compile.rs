//! Compile proofs of the emitted face over sources the checked-in fixture does
//! not hold: the face is emitted from an inline ridl source, written to a
//! temporary directory, and checked with a bare `rustc` that links `ridl-rt`,
//! the way the codec proofs in `flatbuffers_roundtrip.rs` compile emitted
//! code. What is proven is that the source compiles, not what it does; the
//! round trips in `interaction_face.rs` are the behavioural proofs.

#[path = "support/rustc.rs"]
mod rustc;

use ridl_backend_rust::{WireEncoding, generate_face, generate_pipeline};

/// Emits the face of `source` and checks it as a library crate
/// (`--emit=metadata -D warnings`), panicking with rustc's diagnostics when it
/// does not compile. It is checked twice: with the `std` cfg on, against a
/// `ridl-rt` built with `std`, so the `blocking` module is part of what is
/// checked; and with the cfg off, against a `ridl-rt` built without `std`,
/// so an item the emitter leaves outside `cfg(feature = "std")` — the
/// `blocking` module, which names `ridl_rt::task::block_on`, or the call
/// future's `sent()`, which only that module calls and which is dead code
/// without it — fails the build.
fn face_compiles(name: &str, source: &str) {
    face_compiles_with(name, source, "");
}

/// [`face_compiles`], with `consumer` — Rust source of a module that uses the
/// face — appended to the emitted face before it is checked, so a proof can
/// cover what a consumer writes and not only what the emitter writes.
fn face_compiles_with(name: &str, source: &str, consumer: &str) {
    for std in [true, false] {
        let (face, compiled) = compile_face(name, source, consumer, std);
        assert!(
            compiled.status.success(),
            "the emitted face must compile with the std cfg {}, rustc said:\n{}\nsource:\n{face}",
            if std { "on" } else { "off" },
            String::from_utf8_lossy(&compiled.stderr)
        );
    }
}

/// One `rustc` check of the face of `source` with `consumer` appended, with
/// the `std` cfg on or off, returning the source checked and rustc's output,
/// so a proof can expect a failure as well as a success.
fn compile_face(
    name: &str,
    source: &str,
    consumer: &str,
    std: bool,
) -> (String, std::process::Output) {
    let output = ridlc::compile(&format!("{name}.ridl"), source);
    // Errors only: a call with no response bound draws RIDL-112, a warning,
    // and such a call is one of the shapes proven here.
    let errors: Vec<_> = output
        .diagnostics
        .iter()
        .filter(|diagnostic| matches!(diagnostic.severity, ridl_core::diag::Severity::Error))
        .collect();
    assert!(
        errors.is_empty(),
        "{name}.ridl must compile with no error, got: {errors:?}"
    );
    let mut face = generate_face(&output.package)
        .expect("generate_face")
        .rust_source;
    face.push_str(consumer);

    let dir = tempfile::tempdir().expect("a temp dir is created");
    let source_path = dir.path().join(format!("{name}.rs"));
    std::fs::write(&source_path, &face).expect("the generated source is written");
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
    (face, compiled)
}

/// An `internal` interface over `internal` payload types, beside a public
/// interface over a public type. The package-private interface gets no face:
/// a face is public API, and its signatures would name `pub(crate)` types
/// (E0446), so the emitted crate compiles only because no face is written.
const INTERNAL_BESIDE_PUBLIC: &str = r#"
package face.hidden

type Level : integer [0..100]
internal type Raw : integer [0..1000]
internal struct Frame { ticks: Raw }

interface Summary {
  signal level: Level @10ms
  command setLevel(level: Level) @[..50ms]
}

internal interface Diagnostics {
  signal ticks: Raw @10ms
  event frame: Frame @[100ms..1s]
  command reset(arg: Raw) @[..50ms]
  query read(arg: Raw): Frame @[..50ms]
}
"#;

#[test]
fn an_internal_interface_beside_a_public_one_compiles() {
    face_compiles("hidden", INTERNAL_BESIDE_PUBLIC);
}

/// The face of an `internal` interface is absent, and the public interface
/// beside it keeps its face. The descriptors of the internal interface stay,
/// crate-visible, because the catalog describes it whether or not it has a face.
#[test]
fn an_internal_interface_gets_no_face_module() {
    let (face, _) = compile_face("hidden_text", INTERNAL_BESIDE_PUBLIC, "", true);
    assert!(
        face.contains("pub mod summary {"),
        "the public interface keeps its face:\n{face}"
    );
    for absent in ["mod diagnostics", "__RIDL_NO_FACE"] {
        assert!(
            !face.contains(absent),
            "an internal interface emits no `{absent}`:\n{face}"
        );
    }
    assert!(
        face.contains("pub(crate) struct Diagnostics;"),
        "the descriptor of the internal interface is crate-visible:\n{face}"
    );
    assert!(
        face.contains("#[allow(dead_code)]\npub(crate) struct Diagnostics;"),
        "the crate-visible descriptor allows dead code:\n{face}"
    );
    assert!(
        !face.contains("pub struct Diagnostics"),
        "no descriptor of the internal interface is public:\n{face}"
    );
}

/// The path `ridl build --emit rust` takes is `generate_pipeline`, which skips
/// an interface it refuses instead of failing. An internal interface gets no
/// face there either, and no skip note: the note marks a refusal.
#[test]
fn an_internal_interface_gets_no_face_through_the_pipeline() {
    let output = ridlc::compile("hidden_pipeline.ridl", INTERNAL_BESIDE_PUBLIC);
    let generated = generate_pipeline(&output.package, WireEncoding::FlatBuffers, &[])
        .expect("generate_pipeline");
    let source = generated.rust_source;
    assert!(source.contains("pub mod summary {"), "{source}");
    for absent in ["mod diagnostics", "__RIDL_NO_FACE"] {
        assert!(!source.contains(absent), "no `{absent}`:\n{source}");
    }
    assert!(
        source.contains("#[allow(dead_code)]\npub(crate) struct Diagnostics;"),
        "the crate-visible descriptor allows dead code:\n{source}"
    );
    assert!(!source.contains("pub struct Diagnostics"), "{source}");
}

/// The skip note of a public interface whose call the face cannot carry names
/// the call shape and its tracking issue.
#[test]
fn a_public_interface_with_an_uncarried_call_leaves_the_call_shape_note() {
    let source = r#"
package face.shape

type Level : integer [0..100]

interface Wide {
  command set(first: Level, second: Level) @[..50ms]
}
"#;
    let output = ridlc::compile("shape.ridl", source);
    let generated = generate_pipeline(&output.package, WireEncoding::FlatBuffers, &[])
        .expect("generate_pipeline");
    let text = generated.rust_source;
    assert!(
        text.contains("carries no generated interaction face"),
        "{text}"
    );
    assert!(text.contains("A call the face cannot carry"), "{text}");
    let owner = &text[text
        .find("A call the face cannot carry")
        .expect("owner line")..];
    assert!(owner.contains("driftsys/ridl#713"), "{text}");
    assert!(!text.contains("The interface is `internal`"), "{text}");
}

/// The skip note of a public interface whose query the face cannot carry
/// names the call shape and its tracking issue. The command case above reaches
/// the command arm of the owner match; this one reaches the query arm.
#[test]
fn a_public_interface_with_an_uncarried_query_leaves_the_call_shape_note() {
    let source = r#"
package face.query_shape

type Level : integer [0..100]

interface Wide {
  query read(first: Level, second: Level): Level @[..50ms]
}
"#;
    let output = ridlc::compile("query_shape.ridl", source);
    let generated = generate_pipeline(&output.package, WireEncoding::FlatBuffers, &[])
        .expect("generate_pipeline");
    let text = generated.rust_source;
    assert!(
        text.contains("carries no generated interaction face"),
        "{text}"
    );
    let owner = &text[text
        .find("A call the face cannot carry")
        .unwrap_or_else(|| panic!("the call-shape owner line:\n{text}"))..];
    assert!(owner.contains("driftsys/ridl#713"), "{text}");
}

/// An internal interface whose descriptors are refused for a contract clause
/// leaves the internal note: the descriptors headline, the clause reason, and
/// the internal owner line in place of the clause owner. Its trailer does not
/// say that a face is built on the descriptors, because the interface has no
/// face.
#[test]
fn an_internal_interface_refused_for_a_clause_leaves_the_internal_note() {
    let source = r#"
package face.internal_clause

type Level : integer [0..100]

interface Summary {
  signal level: Level @10ms
}

internal interface Guarded {
  command set(arg: Level) [
    require arg > 10 || arg < 5
  ] @[..50ms]
}
"#;
    let output = ridlc::compile("internal_clause.ridl", source);
    let generated = generate_pipeline(&output.package, WireEncoding::FlatBuffers, &[])
        .expect("generate_pipeline");
    let text = generated.rust_source;
    let note = text
        .split("const __RIDL_NO_FACE_Guarded")
        .next()
        .expect("the note precedes its constant");
    let note = &note[note
        .rfind("Interface `Guarded`")
        .unwrap_or_else(|| panic!("the note's headline:\n{text}"))..];
    assert!(
        note.contains("carries no generated interaction descriptors"),
        "{note}"
    );
    assert!(note.contains("cannot translate contract clause"), "{note}");
    assert!(
        note.contains("The interface is `internal`, so it has no face in any case"),
        "{note}"
    );
    for absent in [
        "carries no generated interaction face",
        "driftsys/ridl#704",
        "which the face is built on",
    ] {
        assert!(!note.contains(absent), "no `{absent}`:\n{note}");
    }
    assert!(
        note.contains("The refusal is raised by the descriptor emitter."),
        "{note}"
    );
    assert!(text.contains("pub mod summary {"), "{text}");
}

/// An internal interface has no face module, but its descriptors are still
/// emitted, so it still claims their names: a declaration named like one of
/// them is refused, as it is for a public interface.
#[test]
fn an_internal_interface_still_claims_its_descriptor_names() {
    let source = r#"
package face.internal_claims

type CabinTemperature : integer [-40..85]

internal interface Cabin {
  signal temperature: CabinTemperature @10ms
}
"#;
    let output = ridlc::compile("internal_claims.ridl", source);
    let err = generate_pipeline(&output.package, WireEncoding::FlatBuffers, &[])
        .expect_err("the descriptor name collides with a declaration");
    for part in [
        "`CabinTemperature`",
        "member `temperature`",
        "interface `Cabin`",
    ] {
        assert!(err.message.contains(part), "no {part} in: {}", err.message);
    }
}

/// An internal interface claims no face-module name, so its `snake_case` name
/// can equal another interface's without a collision.
#[test]
fn an_internal_interface_claims_no_face_module_name() {
    let source = r#"
package face.claims

type Level : integer [0..100]

interface HttpServer {
  signal level: Level @10ms
}

internal interface HTTPServer {
  signal level: Level @10ms
}
"#;
    let output = ridlc::compile("claims.ridl", source);
    generate_pipeline(&output.package, WireEncoding::FlatBuffers, &[])
        .expect("the internal interface claims no face module");
    generate_face(&output.package).expect("generate_face");
}

/// A ridl member or parameter may carry a name the emitter uses for a local
/// of its own. The async face put `port`, `deadline`, `this` and
/// `cx` beside a binding that carries the ridl parameter's name, in the call
/// method, the call future's `poll` and the internal send; each of the four
/// compiled before it. Those three bodies now rebind the ridl-named argument
/// to an emitter-owned name first, and the internal send names its port
/// parameter the same way, with the `__` prefix the codec's own locals use
/// (`__p`, `__v`). The blocking module's deadline helper is a module-level
/// function, and a parameter named `deadlineAfter` is bound as `deadline_after`
/// in the method body, so the function is `__deadline_after` for the same
/// reason. The generated `dispatch` has the same treatment; its cases follow.
#[test]
fn a_call_parameter_named_like_a_future_local_compiles() {
    face_compiles(
        "names",
        r#"
package face.names

type Level : integer [0..100]
type Window : integer [0..100000]
type Average : integer [0..1000]

interface Names {
  command port(port: Level) @[..50ms]
  command deadline(deadline: Level)
  query this(this: Window): Average @[..50ms]
  query cx(cx: Window): Average
  command deadlineAfter(deadlineAfter: Level)
}
"#,
    );
}

/// The generated `dispatch` binds a claim's decoded argument next to its own
/// parameters and locals. Before issue #570 it bound the argument under the
/// ridl parameter's own name, so a parameter named like one of them failed
/// to compile; it now binds the argument as `__arg`, which no ridl identifier
/// can be. These six are the names that collided: the parameters `h`, `p`
/// and `buf`, and the locals `claim`, `accepted` and `reply`. `h`, `p` and
/// `claim` collided in both arms, `accepted` in the command arm only, and
/// `buf` and `reply` in the query arm only. Each name is a parameter of one
/// command (`set<Name>`) and of one query (`read<Name>`), so the generated
/// source that the failure message prints shows which arm failed. One face
/// holds every case, because each `face_compiles` call builds ridl-rt twice.
#[test]
fn a_call_parameter_named_like_a_dispatch_local_compiles() {
    let members: String = ["claim", "h", "accepted", "buf", "p", "reply"]
        .iter()
        .map(|param| {
            let suffix = format!("{}{}", param[..1].to_uppercase(), &param[1..]);
            format!(
                "  command set{suffix}({param}: Level) @[..50ms]\n  \
                 query read{suffix}({param}: Window): Average @[..50ms]\n"
            )
        })
        .collect();
    face_compiles(
        "dispatch_locals",
        &format!(
            r#"
package face.dispatchlocals

type Level : integer [0..100]
type Window : integer [0..100000]
type Average : integer [0..1000]

interface Dispatch {{
{members}}}
"#
        ),
    );
}

/// The face's fixed methods are trait methods, and its member methods are
/// inherent (ADR-0023 decision 7, driftsys/ridl#580): a member whose snake
/// case equals a fixed name — `new`, `next_event`, `with_timeout`,
/// `set_timeout`, `commit` — or a derived name of another member —
/// `subscribe_<event>`, `invalidate_<signal>` — compiles, because Rust gives a
/// trait its own namespace. Before decision 7 every one of these was rustc
/// E0592. One case per name; the member's kind is chosen so that the name
/// lands on the type whose fixed method it collides with: a signal is read by
/// both clients and set by the publisher, a command reaches both clients, and
/// the blocking client exists only when something waits, which is why each
/// source declares an event.
///
/// Each case also compiles a consumer with the interface's `prelude` in scope
/// that dot-calls the member and reaches the fixed operation through the
/// trait's path. The member is told apart from the trait method by its
/// arguments and its result: a command takes `level`, a signal set takes
/// `level`, a signal read returns a `Sample`, and no trait method does. What
/// the dot call reaches follows Rust's method probe, which tries the
/// receiver by value before `&` and before `&mut`, and at each step an
/// inherent method before a trait method: a member takes `&self` or
/// `&mut self`; `Bind::new` takes no receiver and is reached by a path call,
/// and every other trait method takes `&mut self` except
/// `Timeout::with_timeout`, which takes `self`. So the member keeps the dot
/// call for every name but `with_timeout`, where the by-value step comes
/// first and the trait method wins on a client held by value; the consumer
/// reaches that member through the inherent path
/// `blocking::Client::with_timeout(&mut client, level)`, and the dot call
/// reaches it again through a `&mut` borrow of the client, whose own type is
/// the first candidate.
fn fixed_name_face(members: &str) -> String {
    format!(
        r#"
package face.fixednames

type Level : integer [0..100]
type Temperature : integer [-40..85]

struct Warning {{
  code : Level
}}

interface Cabin {{
  signal  temperature : Temperature @10ms
  event   warning     : Warning @[100ms..1s]
{members}}}
"#
    )
}

/// The bounds of `Cabin`'s two clients in [`fixed_name_face`], with a command
/// declared; a case without one names a superset, which is accepted.
const CLIENT_BOUNDS: &str = "::ridl_rt::port::SignalReader \
    + ::ridl_rt::port::EventSource \
    + ::ridl_rt::port::Caller \
    + ::ridl_rt::port::Clock \
    + ::ridl_rt::port::Wakeable";

/// The bounds of `Cabin`'s publisher in [`fixed_name_face`].
const PUBLISHER_BOUNDS: &str = "::ridl_rt::port::SignalWriter + ::ridl_rt::port::EventSink";

/// A signal named `new` is a read on both clients and a setter on the
/// publisher: it meets `Bind::new` on all three types. `Client::new(port)` is
/// the member, so the consumer binds through `Bind`'s path; the dot call
/// `client.new()` is the read, and `publisher.new(level)` the set.
#[test]
fn a_member_named_new_compiles() {
    face_compiles_with(
        "member_new",
        &fixed_name_face("  signal new : Level @10ms\n"),
        &format!(
            r#"
pub mod consumer {{
    use super::cabin::prelude::*;

    pub fn client<P: {CLIENT_BOUNDS}>(port: P) {{
        let client = <super::cabin::Client<P> as Bind>::new(port);
        let _: ::core::result::Result<::ridl_rt::sample::Sample<super::Level>, ::ridl_rt::port::ReadError> =
            client.new();
    }}

    pub fn publisher<W: {PUBLISHER_BOUNDS}>(port: W, level: super::Level) {{
        let mut publisher: super::cabin::Publisher<W> = Bind::new(port);
        let _: ::core::result::Result<(), ::ridl_rt::port::WriteError> = publisher.new(level);
    }}

    #[cfg(feature = "std")]
    pub fn blocking<P: {CLIENT_BOUNDS}>(port: P) {{
        let client = <super::cabin::blocking::Client<P> as Bind>::new(port);
        let _: ::core::result::Result<::ridl_rt::sample::Sample<super::Level>, ::ridl_rt::port::ReadError> =
            client.new();
    }}
}}
"#
        ),
    );
}

/// A signal named `commit` is a setter on the publisher: it meets
/// `Publish::commit`. `publisher.commit(level)` is the member, and
/// `Publish::commit(&mut publisher)` the face's.
#[test]
fn a_member_named_commit_compiles() {
    face_compiles_with(
        "member_commit",
        &fixed_name_face("  signal commit : Level @10ms\n"),
        &format!(
            r#"
pub mod consumer {{
    use super::cabin::prelude::*;

    pub fn publisher<W: {PUBLISHER_BOUNDS}>(port: W, level: super::Level) {{
        let mut publisher = super::cabin::Publisher::new(port);
        let _: ::core::result::Result<(), ::ridl_rt::port::WriteError> = publisher.commit(level);
        let () = Publish::commit(&mut publisher);
    }}
}}
"#
        ),
    );
}

/// A command named `nextEvent` is `next_event` on both clients: it meets
/// `Events::next_event`. `client.next_event(level)` is the member, and
/// `Events::next_event(&mut client)` the face's.
#[test]
fn a_member_named_next_event_compiles() {
    face_compiles_with(
        "member_next_event",
        &fixed_name_face("  command nextEvent(level: Level) @[..50ms]\n"),
        &format!(
            r#"
pub mod consumer {{
    use super::cabin::prelude::*;

    pub fn client<P: {CLIENT_BOUNDS}>(port: P, level: super::Level) {{
        let mut client = super::cabin::Client::new(port);
        let _: super::cabin::NextEventCall<'_, P> = client.next_event(level);
        let _: super::cabin::NextEvent<'_, P> = Events::next_event(&mut client);
    }}

    #[cfg(feature = "std")]
    pub fn blocking<P: {CLIENT_BOUNDS}>(port: P, level: super::Level) {{
        let mut client = super::cabin::blocking::Client::new(port);
        let _: ::core::result::Result<(), ::ridl_rt::error::ClientError> = client.next_event(level);
        let _: ::core::result::Result<::core::option::Option<super::cabin::Event>, ::ridl_rt::port::ReadError> =
            Events::next_event(&mut client);
    }}
}}
"#
        ),
    );
}

/// A command named `withTimeout` is `with_timeout` on the blocking client: it
/// meets `Timeout::with_timeout`, the one fixed method that takes `self` by
/// value. On a blocking client held by value the dot call reaches the trait
/// method, because the probe's by-value step comes before its `&mut` step;
/// the consumer reaches the member through the inherent path
/// `blocking::Client::with_timeout(&mut client, level)`, and the dot call on
/// a `&mut` borrow of the client is the member again. On the async client,
/// which implements no `Timeout`, the dot call is the member.
#[test]
fn a_member_named_with_timeout_compiles() {
    face_compiles_with(
        "member_with_timeout",
        &fixed_name_face("  command withTimeout(level: Level) @[..50ms]\n"),
        &format!(
            r#"
pub mod consumer {{
    use super::cabin::prelude::*;

    pub fn client<P: {CLIENT_BOUNDS}>(port: P, level: super::Level) {{
        let mut client = super::cabin::Client::new(port);
        let _: super::cabin::WithTimeoutCall<'_, P> = client.with_timeout(level);
    }}

    #[cfg(feature = "std")]
    pub fn blocking<P: {CLIENT_BOUNDS}>(port: P, level: super::Level) {{
        let client = super::cabin::blocking::Client::new(port);
        let mut client: super::cabin::blocking::Client<P> =
            client.with_timeout(::std::time::Duration::from_millis(10));
        let _: ::core::result::Result<(), ::ridl_rt::error::ClientError> =
            super::cabin::blocking::Client::with_timeout(&mut client, level);
        let borrowed: &mut super::cabin::blocking::Client<P> = &mut client;
        let _: ::core::result::Result<(), ::ridl_rt::error::ClientError> =
            borrowed.with_timeout(level);
    }}
}}
"#
        ),
    );
}

/// A signal named `withTimeout` is a read, `with_timeout(&self)`, on both
/// clients. On the blocking client the dot call on a client held by value is
/// `Timeout::with_timeout` as for the command, and the inherent path
/// `blocking::Client::with_timeout(&client)` is the member's one route: a
/// `&mut` borrow does not reach a `&self` member, because the probe meets the
/// by-value trait method at the dereferenced step before `&Client`. On the
/// async client the dot call is the member.
#[test]
fn a_signal_named_with_timeout_compiles() {
    face_compiles_with(
        "signal_with_timeout",
        &fixed_name_face("  signal withTimeout : Level @10ms\n"),
        &format!(
            r#"
pub mod consumer {{
    use super::cabin::prelude::*;

    pub fn client<P: {CLIENT_BOUNDS}>(port: P) {{
        let client = super::cabin::Client::new(port);
        let _: ::core::result::Result<::ridl_rt::sample::Sample<super::Level>, ::ridl_rt::port::ReadError> =
            client.with_timeout();
    }}

    #[cfg(feature = "std")]
    pub fn blocking<P: {CLIENT_BOUNDS}>(port: P) {{
        let client = super::cabin::blocking::Client::new(port);
        let client: super::cabin::blocking::Client<P> =
            client.with_timeout(::std::time::Duration::from_millis(10));
        let _: ::core::result::Result<::ridl_rt::sample::Sample<super::Level>, ::ridl_rt::port::ReadError> =
            super::cabin::blocking::Client::with_timeout(&client);
    }}
}}
"#
        ),
    );
}

/// A command named `setTimeout` is `set_timeout` on the blocking client: it
/// meets `Timeout::set_timeout`. `client.set_timeout(level)` is the member,
/// and `Timeout::set_timeout(&mut client, None)` the face's.
#[test]
fn a_member_named_set_timeout_compiles() {
    face_compiles_with(
        "member_set_timeout",
        &fixed_name_face("  command setTimeout(level: Level) @[..50ms]\n"),
        &format!(
            r#"
pub mod consumer {{
    #[cfg(feature = "std")]
    pub fn blocking<P: {CLIENT_BOUNDS}>(port: P, level: super::Level) {{
        use super::cabin::prelude::*;
        let mut client = super::cabin::blocking::Client::new(port);
        let _: ::core::result::Result<(), ::ridl_rt::error::ClientError> = client.set_timeout(level);
        let () = Timeout::set_timeout(&mut client, None);
    }}
}}
"#
        ),
    );
}

/// A command named `subscribeWarning` beside the event `warning` is
/// `subscribe_warning` on both clients: it meets the generated
/// `Subscribe::subscribe_warning`. RIDL-149 does not refuse the pair, because
/// `subscribe_warning` and `warning` are distinct names.
/// `client.subscribe_warning(level)` is the member, and
/// `Subscribe::subscribe_warning(&mut client)` the face's.
#[test]
fn a_member_named_like_a_subscribe_method_compiles() {
    face_compiles_with(
        "member_subscribe_warning",
        &fixed_name_face("  command subscribeWarning(level: Level) @[..50ms]\n"),
        &format!(
            r#"
pub mod consumer {{
    use super::cabin::prelude::*;

    pub fn client<P: {CLIENT_BOUNDS}>(port: P, level: super::Level) {{
        let mut client = super::cabin::Client::new(port);
        let _: super::cabin::SubscribeWarningCall<'_, P> = client.subscribe_warning(level);
        let _: ::core::result::Result<(), ::ridl_rt::port::SubscribeError> =
            super::cabin::Subscribe::subscribe_warning(&mut client);
    }}

    #[cfg(feature = "std")]
    pub fn blocking<P: {CLIENT_BOUNDS}>(port: P, level: super::Level) {{
        let mut client = super::cabin::blocking::Client::new(port);
        let _: ::core::result::Result<(), ::ridl_rt::error::ClientError> = client.subscribe_warning(level);
        let _: ::core::result::Result<(), ::ridl_rt::port::SubscribeError> =
            super::cabin::Subscribe::subscribe_warning(&mut client);
    }}
}}
"#
        ),
    );
}

/// A signal named `invalidateTemperature` beside the signal `temperature` is
/// `invalidate_temperature` on the publisher: it meets the generated
/// `Invalidate::invalidate_temperature`. `publisher.invalidate_temperature(level)`
/// is the member, and `Invalidate::invalidate_temperature(&mut publisher)` the
/// face's.
#[test]
fn a_member_named_like_an_invalidate_method_compiles() {
    face_compiles_with(
        "member_invalidate_temperature",
        &fixed_name_face("  signal invalidateTemperature : Level @10ms\n"),
        &format!(
            r#"
pub mod consumer {{
    use super::cabin::prelude::*;

    pub fn publisher<W: {PUBLISHER_BOUNDS}>(port: W, level: super::Level) {{
        let mut publisher = super::cabin::Publisher::new(port);
        let _: ::core::result::Result<(), ::ridl_rt::port::WriteError> =
            publisher.invalidate_temperature(level);
        let _: ::core::result::Result<(), ::ridl_rt::port::WriteError> =
            super::cabin::Invalidate::invalidate_temperature(&mut publisher);
    }}
}}
"#
        ),
    );
}

/// What a consumer writes: `use <iface>::prelude::*;` puts the fixed traits
/// and the per-interface traits in scope; two interfaces' preludes
/// glob-imported together do not conflict (each `Subscribe` and each
/// `Invalidate` is re-exported as `_`, and `Bind` re-exported by both is one
/// item); and every call site is the one an inherent method had. `Cabin` and
/// `Siren` each declare a signal and an event, so each prelude carries a
/// `Subscribe` and an `Invalidate` of its own, and the consumer dot-calls
/// both interfaces' `subscribe_<event>` and `invalidate_<signal>`: each
/// prelude is used, so neither is an unused import under `-D warnings`.
/// Interface `Names` declares a signal `new`, so `names::Client::new(port)` is
/// the member and the consumer binds the face through the qualified path
/// `<Client<_> as Bind>::new(port)` or the typed `let c: Client<_> =
/// Bind::new(port)` (ADR-0023 decision 7). The blocking half is under the
/// `std` cfg, as the module it uses is.
#[test]
fn a_consumer_of_two_preludes_compiles() {
    face_compiles_with(
        "preludes",
        r#"
package face.preludes

type Level : integer [0..100]
type Temperature : integer [-40..85]

struct Warning {
  code : Level
}

interface Cabin {
  signal  temperature : Temperature @10ms
  event   warning     : Warning @[100ms..1s]
  command setLevel(level: Level) @[..50ms]
}

interface Siren {
  signal level   : Level @10ms
  event  tripped : Warning @[100ms..1s]
}

interface Names {
  signal new : Level @10ms
  event  warning : Warning @[100ms..1s]
}
"#,
        r#"
pub mod consumer {
    use super::cabin::prelude::*;
    use super::siren::prelude::*;

    pub fn cabin<P>(port: P) -> Result<(), ::ridl_rt::port::SubscribeError>
    where
        P: ::ridl_rt::port::SignalReader
            + ::ridl_rt::port::EventSource
            + ::ridl_rt::port::Caller
            + ::ridl_rt::port::Clock
            + ::ridl_rt::port::Wakeable,
    {
        let mut client = super::cabin::Client::new(port);
        let _sample = client.temperature();
        client.subscribe_warning()?;
        let _next: super::cabin::NextEvent<'_, P> = client.next_event();
        Ok(())
    }

    pub fn siren<P>(port: P) -> Result<(), ::ridl_rt::port::SubscribeError>
    where
        P: ::ridl_rt::port::SignalReader + ::ridl_rt::port::EventSource + ::ridl_rt::port::Wakeable,
    {
        let mut client = super::siren::Client::new(port);
        let _sample = client.level();
        client.subscribe_tripped()?;
        let _next: super::siren::NextEvent<'_, P> = client.next_event();
        Ok(())
    }

    pub fn publish<W: ::ridl_rt::port::SignalWriter + ::ridl_rt::port::EventSink>(
        cabin: W,
        siren: W,
    ) -> Result<(), ::ridl_rt::port::WriteError> {
        let mut publisher = super::cabin::Publisher::new(cabin);
        publisher.invalidate_temperature()?;
        publisher.commit();
        let mut publisher = super::siren::Publisher::new(siren);
        publisher.invalidate_level()?;
        publisher.commit();
        Ok(())
    }

    #[cfg(feature = "std")]
    pub fn blocking<P>(port: P) -> Result<(), ::ridl_rt::port::SubscribeError>
    where
        P: ::ridl_rt::port::SignalReader
            + ::ridl_rt::port::EventSource
            + ::ridl_rt::port::Caller
            + ::ridl_rt::port::Clock
            + ::ridl_rt::port::Wakeable,
    {
        let mut client = super::cabin::blocking::Client::new(port)
            .with_timeout(::std::time::Duration::from_millis(10));
        client.set_timeout(None);
        client.subscribe_warning()?;
        let _event: Result<Option<super::cabin::Event>, ::ridl_rt::port::ReadError> =
            client.next_event();
        Ok(())
    }

    /// The qualified forms, on the interface whose member is named `new`.
    pub fn names<P>(port: P, other: P) -> Result<(), ::ridl_rt::port::ReadError>
    where
        P: ::ridl_rt::port::SignalReader
            + ::ridl_rt::port::EventSource
            + ::ridl_rt::port::Wakeable,
    {
        use super::names::prelude::*;
        let client = <super::names::Client<P> as Bind>::new(port);
        let _sample = client.new()?;
        let typed: super::names::Client<P> = Bind::new(other);
        let _sample = typed.new()?;
        Ok(())
    }
}
"#,
    );
}

/// A prelude brings the `ridl_rt::face` traits its own interface's types
/// implement, and no other (ADR-0023 decision 7): `Horn` declares a signal
/// only, so `horn::prelude` re-exports `Bind`, `Publish` and `Invalidate` and
/// no `Timeout`, and `valve::blocking::Client`'s `with_timeout` is E0599
/// ("items from traits can only be used if the trait is in scope") with
/// `horn::prelude` alone in scope. With `valve::prelude` imported as well the
/// consumer compiles. The failing half is one `rustc` call with the `std`
/// cfg on, where the blocking module exists, whose failure is asserted; the
/// passing half is a `face_compiles_with` proof, with `valve::prelude`
/// imported inside the `std`-only function, because with the cfg off
/// `valve::prelude` adds nothing to what `horn::prelude` brings — `Bind`,
/// and `Timeout` only under `std` — and rustc reports it as an unused
/// import, which a third `rustc` call, with the cfg off and the import at
/// module level, pins as well.
#[test]
fn a_prelude_brings_only_its_own_interfaces_traits() {
    let source = r#"
package face.preludescope

type Level : integer [0..100]

interface Horn {
  signal active : Level @10ms
}

interface Valve {
  command open(level: Level) @[..50ms]
}
"#;
    let consumer = |module_preludes: &str, valve_preludes: &str| {
        format!(
            r#"
pub mod consumer {{
    {module_preludes}

    #[cfg(feature = "std")]
    pub fn valve<P>(port: P)
    where
        P: ::ridl_rt::port::Caller + ::ridl_rt::port::Clock + ::ridl_rt::port::Wakeable,
    {{
        {valve_preludes}
        let _client = super::valve::blocking::Client::new(port)
            .with_timeout(::std::time::Duration::from_millis(10));
    }}

    pub fn horn<W: ::ridl_rt::port::SignalWriter>(port: W) {{
        let mut publisher = super::horn::Publisher::new(port);
        publisher.commit();
    }}
}}
"#
        )
    };

    let (face, compiled) = compile_face(
        "prelude_scope_horn_only",
        source,
        &consumer("use super::horn::prelude::*;", ""),
        true,
    );
    let stderr = String::from_utf8_lossy(&compiled.stderr);
    assert!(
        !compiled.status.success()
            && stderr.contains("error[E0599]")
            && stderr.contains("with_timeout"),
        "with_timeout must be E0599 under horn's prelude alone, rustc said:\n{stderr}\nsource:\n{face}"
    );

    face_compiles_with(
        "prelude_scope_both",
        source,
        &consumer(
            "use super::horn::prelude::*;",
            "use super::valve::prelude::*;",
        ),
    );

    let (face, compiled) = compile_face(
        "prelude_scope_unused",
        source,
        &consumer(
            "use super::horn::prelude::*;\n    use super::valve::prelude::*;",
            "",
        ),
        false,
    );
    let stderr = String::from_utf8_lossy(&compiled.stderr);
    assert!(
        !compiled.status.success() && stderr.contains("unused import: `super::valve::prelude::*`"),
        "valve's prelude must be an unused import with the std cfg off beside horn's, rustc said:\n{stderr}\nsource:\n{face}"
    );
}

/// `ridl_rt::face::Timeout` is under `ridl-rt`'s `std` feature (ADR-0021
/// decision 19), as the generated prelude's `#[cfg(feature = "std")]`
/// re-export of it assumes: a crate that names the trait compiles against a
/// `ridl-rt` built with `std` and fails to resolve the import (E0432) against
/// one built without. The proof is two bare `rustc` calls over a one-line
/// crate, the way the face proofs above are checked.
#[test]
fn timeout_is_under_ridl_rt_s_std_feature() {
    let dir = tempfile::tempdir().expect("a temp dir is created");
    let source_path = dir.path().join("names_timeout.rs");
    std::fs::write(&source_path, "pub use ridl_rt::face::Timeout;\n")
        .expect("the source is written");
    for std in [true, false] {
        let ridl_rt = if std {
            rustc::ridl_rt_rlib(dir.path())
        } else {
            rustc::ridl_rt_rlib_without_std(dir.path())
        };
        let compiled = std::process::Command::new("rustc")
            .args([
                "--edition",
                "2024",
                "--crate-type",
                "lib",
                "--emit=metadata",
            ])
            .arg("-o")
            .arg(dir.path().join(format!("libnames_timeout_std_{std}.rmeta")))
            .arg("--extern")
            .arg(format!("ridl_rt={}", ridl_rt.display()))
            .arg(&source_path)
            .output()
            .expect("rustc must be installed and runnable for this test to be meaningful");
        let stderr = String::from_utf8_lossy(&compiled.stderr);
        if std {
            assert!(
                compiled.status.success(),
                "`ridl_rt::face::Timeout` resolves with the std feature on, rustc said:\n{stderr}"
            );
        } else {
            assert!(
                !compiled.status.success() && stderr.contains("error[E0432]"),
                "`ridl_rt::face::Timeout` must not resolve with the std feature off, rustc said:\n{stderr}"
            );
        }
    }
}
