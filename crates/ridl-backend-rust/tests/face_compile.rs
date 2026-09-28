//! Compile proofs of the emitted face over sources the checked-in fixture does
//! not hold: the face is emitted from an inline ridl source, written to a
//! temporary directory, and checked with a bare `rustc` that links `ridl-rt`,
//! the way the codec proofs in `flatbuffers_roundtrip.rs` compile emitted
//! code. What is proven is that the source compiles, not what it does; the
//! round trips in `interaction_face.rs` are the behavioural proofs.

#[path = "support/rustc.rs"]
mod rustc;

use ridl_backend_rust::generate_face;

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
    for std in [true, false] {
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
        assert!(
            compiled.status.success(),
            "the emitted face must compile with the std cfg {}, rustc said:\n{}\nsource:\n{face}",
            if std { "on" } else { "off" },
            String::from_utf8_lossy(&compiled.stderr)
        );
    }
}

/// A ridl member or parameter may carry a name the emitter uses for a local
/// of its own. Story E11.21's first half put `port`, `deadline`, `this` and
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
/// `&mut self`, and so does every trait method except
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
