//! The port contract suite of `ridl-rt-conformance`, run over this runtime.
//!
//! Five extension flags run here: those of both signal extensions
//! (`scannable`, `coherent`), of `Wakeable`, of the two stream extensions
//! (`streams`), which the loopback implements, and `trace`, because the
//! loopback carries the trace context. The tests
//! of what only this runtime can express are in `tests/ports.rs`. The identity
//! group runs with `node`, and `identity_debug` runs with `debug`.

use ridl_loopback::{CallerHandle, HandlerHandle, Loopback, SourceHandle};
use ridl_rt::contract::CatalogRef;
use ridl_rt::sample::Duration;
use ridl_rt_conformance::Factory;

// Compile the production wrapper here so the factory can drive its private
// clock and fault hooks without adding a raw-handle escape to the API.
// The identity tests attach through the exported Node and its real
// reservations; the base suite run through `Attachment` uses the stub
// `Reservation` below.
#[cfg(feature = "node")]
struct Reservation;
#[cfg(feature = "node")]
#[path = "../src/node/attachment.rs"]
mod attachment;
#[cfg(feature = "node")]
use attachment::Attachment;

/// The loopback as the suite builds it: the aggregate, the additional role
/// handles `Loopback` hands out, its call table's size, and its two test
/// hooks.
struct LoopbackFactory;

impl Factory for LoopbackFactory {
    type Runtime = Loopback;
    type Source = SourceHandle;
    type Caller = CallerHandle;
    type Handler = HandlerHandle;

    const SLOTS: usize = Loopback::SLOTS;

    fn runtime(catalog: CatalogRef) -> Loopback {
        Loopback::new(catalog)
    }

    fn source(runtime: &Loopback) -> SourceHandle {
        runtime.source()
    }

    fn caller(runtime: &Loopback) -> CallerHandle {
        runtime.caller()
    }

    fn handler(runtime: &Loopback) -> HandlerHandle {
        runtime.handler()
    }

    fn advance(runtime: &mut Loopback, by: Duration) {
        runtime.advance(by);
    }

    fn fail_next_settle(runtime: &mut Loopback) {
        runtime.fail_next_settle();
    }
}

ridl_rt_conformance::suite!(LoopbackFactory; scannable, coherent, wakeable, trace, streams);

#[cfg(feature = "node")]
mod identity {
    use super::*;
    use ridl_rt_conformance::identity::IdentityFactory;

    struct NodeFactory;
    impl Factory for NodeFactory {
        type Runtime = Attachment;
        type Source = SourceHandle;
        type Caller = CallerHandle;
        type Handler = HandlerHandle;

        const SLOTS: usize = Loopback::SLOTS;

        fn runtime(catalog: CatalogRef) -> Attachment {
            Attachment {
                ports: Loopback::new(catalog),
                _reservation: Reservation,
                listener: false,
            }
        }

        fn source(runtime: &Attachment) -> SourceHandle {
            runtime.ports.source()
        }

        fn caller(runtime: &Attachment) -> CallerHandle {
            runtime.ports.caller()
        }

        fn handler(runtime: &Attachment) -> HandlerHandle {
            runtime.ports.handler()
        }

        fn advance(runtime: &mut Attachment, by: Duration) {
            runtime.ports.advance(by);
        }

        fn fail_next_settle(runtime: &mut Attachment) {
            runtime.ports.fail_next_settle();
        }
    }

    impl IdentityFactory for NodeFactory {
        type Node = ridl_loopback::Node;
        type Attachment = ridl_loopback::Attachment;
        type Error = ridl_loopback::AttachError;

        fn node(bytes: &'static [u8], machine: &str) -> Self::Node {
            Self::Node::from_descriptor(bytes, machine).unwrap()
        }

        fn attach(
            node: &Self::Node,
            who: ridl_rt::contract::InstanceNo,
            catalog: &CatalogRef,
        ) -> Result<Self::Attachment, Self::Error> {
            node.attach(who, catalog)
        }
    }

    // The public extension syntax always includes the base suite, so it runs again here.
    #[cfg(not(feature = "debug"))]
    ridl_rt_conformance::suite!(NodeFactory; scannable, coherent, wakeable, trace, identity);
    #[cfg(feature = "debug")]
    ridl_rt_conformance::suite!(NodeFactory; scannable, coherent, wakeable, trace, identity, identity_debug);
}

#[test]
fn identity_debug_selection_uses_dependency_feature() {
    use std::{fs, process::Command};
    struct Probe(std::path::PathBuf);
    impl Drop for Probe {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    let probe = Probe(std::env::temp_dir().join(format!(
        "ridl-identity-suite-probe-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    )));
    fs::create_dir_all(probe.0.join("src")).unwrap();
    fs::write(
        probe.0.join("src/lib.rs"),
        "ridl_rt_conformance::suite!(@identity_debug ());\n",
    )
    .unwrap();
    let dependency =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../ridl-rt-conformance");
    for dependency_debug in [true, false] {
        fs::write(
            probe.0.join("Cargo.toml"),
            format!(
                r#"[package]
name = "identity-suite-probe"
version = "0.0.0"
edition = "2021"
[workspace]
[features]
debug = []
[dependencies]
ridl-rt-conformance = {{ path = {dependency:?}, features = {features:?} }}
"#,
                features = if dependency_debug {
                    vec!["debug"]
                } else {
                    vec![]
                },
            ),
        )
        .unwrap();
        let mut command = Command::new(env!("CARGO"));
        command
            .current_dir(&probe.0)
            .args(["check", "--tests", "--offline"])
            .env("RUSTC_WRAPPER", "");
        if !dependency_debug {
            command.args(["--features", "debug"]);
        }
        let output = command.output().unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            !output.status.success(),
            "the unit type is not an identity factory"
        );
        if dependency_debug {
            assert!(
                stderr.contains("IdentityFactory"),
                "debug tests must expand: {stderr}"
            );
            assert!(!stderr.contains("identity_debug requires"), "{stderr}");
        } else {
            assert!(
                stderr.contains("identity_debug requires ridl-rt-conformance's debug feature"),
                "{stderr}"
            );
        }
    }
}
