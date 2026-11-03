//! The port contract suite of `ridl-rt-conformance`, run over this runtime.
//!
//! Every test the suite has runs here, including those of both signal
//! extensions, of `Wakeable` and of the two stream extensions, which the
//! loopback implements, and those of the `trace` arm, because the loopback
//! carries the trace context. The tests
//! of what only this runtime can express are in `tests/ports.rs`.

use ridl_loopback::{CallerHandle, HandlerHandle, Loopback, SourceHandle};
use ridl_rt::contract::CatalogRef;
use ridl_rt::sample::Duration;
use ridl_rt_conformance::Factory;

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

// This factory deliberately implements only the original hooks.
#[test]
fn existing_factory_needs_no_identity_hook() {
    fn accepts_original_factory<F: Factory>() {}
    accepts_original_factory::<LoopbackFactory>();
}

#[cfg(feature = "node")]
mod identity {
    use super::*;
    use ridl_rt_conformance::identity::IdentityFactory;

    struct NodeFactory;
    impl Factory for NodeFactory {
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

    #[cfg(not(feature = "debug"))]
    ridl_rt_conformance::suite!(NodeFactory; identity);
    #[cfg(feature = "debug")]
    ridl_rt_conformance::suite!(NodeFactory; identity, identity_debug);
}

#[test]
fn identity_debug_selection_uses_dependency_feature() {
    use std::{fs, process::Command};
    struct Probe(std::path::PathBuf);
    impl Drop for Probe {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
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
