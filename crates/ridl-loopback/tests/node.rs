use ridl_loopback::{AttachError, Node, NodeError};
use ridl_rt::contract::{CatalogHash, CatalogRef, InstanceNo, InterfaceNo, Ordinal};
use ridl_rt::port::*;

const BYTES: &[u8] =
    include_bytes!("../../ridl-rt-conformance/fixtures/identity/identity.deployment.binfb");
fn node() -> Node {
    Node::from_descriptor(BYTES, "Local").unwrap()
}
fn id(n: i32) -> InstanceNo {
    InstanceNo::new(n).unwrap()
}
fn catalog(node: &Node, name: &str) -> CatalogRef {
    *node.catalogs().find(|c| c.name == name).unwrap()
}
const IFACE: InterfaceNo = InterfaceNo(1);
const ORD: Ordinal = Ordinal(1);

#[test]
fn node_verifies_descriptor_and_machine() {
    assert!(matches!(
        Node::from_descriptor(b"broken", "Local"),
        Err(NodeError::InvalidDescriptor(_))
    ));
    assert!(matches!(
        Node::from_descriptor(BYTES, "absent"),
        Err(NodeError::UnknownMachine)
    ));
    let mut corrupt = BYTES.to_vec();
    let offset = corrupt.windows(4).position(|w| w == b"RDLC").unwrap();
    corrupt[offset] = 0;
    let error = Node::from_descriptor(Box::leak(corrupt.into_boxed_slice()), "Local")
        .err()
        .unwrap();
    assert!(matches!(error, NodeError::InvalidDescriptor(_)));
    assert!(std::error::Error::source(&error).is_some());
}
#[test]
fn node_catalogs_follow_local_offers_and_maps() {
    assert_eq!(
        node().catalogs().map(|c| c.name).collect::<Vec<_>>(),
        ["alpha", "beta", "mapped"]
    );
}
#[test]
fn positive_grants_use_name_and_hash() {
    let n = node();
    for name in ["alpha", "mapped"] {
        assert!(n.attach(id(1), &catalog(&n, name)).is_ok());
    }
    assert!(matches!(
        n.attach(id(1), &catalog(&n, "beta")),
        Err(AttachError::CatalogNotGranted)
    ));
    for number in [2, 999] {
        assert!(matches!(
            n.attach(id(number), &catalog(&n, "alpha")),
            Err(AttachError::UnknownInstance)
        ));
    }
    let wrong = CatalogRef {
        name: "alpha",
        hash: CatalogHash([0; 32]),
    };
    assert!(matches!(
        n.attach(id(1), &wrong),
        Err(AttachError::UnknownCatalog)
    ));
    let remote = CatalogRef {
        name: "remote",
        hash: CatalogHash([0; 32]),
    };
    assert!(matches!(
        n.attach(id(1), &remote),
        Err(AttachError::UnknownCatalog)
    ));
}
#[test]
fn attachments_survive_node_drop() {
    let n = node();
    let c = catalog(&n, "alpha");
    let mut a = n.attach(id(1), &c).unwrap();
    let b = n.attach(id(3), &c).unwrap();
    drop(n);
    a.set(IFACE, ORD, &[42]).unwrap();
    a.commit();
    let mut out = [0; 8];
    let sample = b.read(IFACE, ORD, &mut out).unwrap();
    assert_eq!(&out[..sample.len], &[42]);
}

#[cfg(feature = "debug")]
mod debug {
    use super::*;
    fn twin() -> InstanceNo {
        id(1).twin().unwrap()
    }
    #[test]
    fn twins_receive_the_same_grants() {
        let n = node();
        for name in ["alpha", "mapped"] {
            let c = catalog(&n, name);
            let mut a = n.attach(twin(), &c).unwrap();
            a.set(IFACE, ORD, &[7]).unwrap();
            a.commit();
            let mut out = [0; 4];
            assert_eq!(a.read(IFACE, ORD, &mut out).unwrap().len, 1);
            assert_eq!(out[0], 7);
        }
        assert!(matches!(
            n.attach(twin(), &catalog(&n, "beta")),
            Err(AttachError::CatalogNotGranted)
        ));
        assert!(matches!(
            n.attach(id(2).twin().unwrap(), &catalog(&n, "alpha")),
            Err(AttachError::UnknownInstance)
        ));
    }
    #[test]
    fn opposite_identities_exclude_in_both_orders() {
        for (first, second) in [(id(1), twin()), (twin(), id(1))] {
            let n = node();
            let _a = n.attach(first, &catalog(&n, "alpha")).unwrap();
            assert!(matches!(
                n.attach(second, &catalog(&n, "mapped")),
                Err(AttachError::IdentityConflict)
            ));
        }
    }
    #[test]
    fn last_attachment_releases_identity() {
        let n = node();
        let c = catalog(&n, "alpha");
        assert!(n.attach(id(1), &catalog(&n, "beta")).is_err());
        drop(n.attach(twin(), &c).unwrap());
        let a = n.attach(id(1), &c).unwrap();
        let b = n.attach(id(1), &catalog(&n, "mapped")).unwrap();
        drop(a);
        assert!(matches!(
            n.attach(twin(), &c),
            Err(AttachError::IdentityConflict)
        ));
        drop(b);
        assert!(n.attach(twin(), &c).is_ok());
    }
    fn listener() -> (Node, ridl_loopback::Attachment, ridl_loopback::Attachment) {
        let n = node();
        let c = catalog(&n, "alpha");
        let mut provider = n.attach(id(1), &c).unwrap();
        provider.set(IFACE, ORD, &[42]).unwrap();
        provider.commit();
        let listener = n.attach(InstanceNo::LISTENER, &c).unwrap();
        (n, provider, listener)
    }
    fn unchanged(provider: &impl SignalReader) {
        let mut out = [0; 8];
        let s = provider.read(IFACE, ORD, &mut out).unwrap();
        assert_eq!(&out[..s.len], &[42]);
    }
    #[test]
    fn listener_refuses_set_without_side_effects() {
        let (_, p, mut l) = listener();
        assert_eq!(l.set(IFACE, ORD, &[9]), Err(WriteError::NotOwner));
        l.commit();
        unchanged(&p);
    }
    #[test]
    fn listener_refuses_invalidate_without_side_effects() {
        let (_, p, mut l) = listener();
        assert_eq!(l.invalidate(IFACE, ORD), Err(WriteError::NotOwner));
        l.commit();
        unchanged(&p);
    }
    #[test]
    fn listener_refuses_touch_without_side_effects() {
        let (_, p, mut l) = listener();
        let before = p.generation(IFACE);
        assert_eq!(l.touch(IFACE, ORD), Err(WriteError::NotOwner));
        l.commit();
        unchanged(&p);
        assert_eq!(p.generation(IFACE), before);
    }
    #[test]
    fn listener_refuses_raise_without_side_effects() {
        let (_, mut p, mut l) = listener();
        p.subscribe(IFACE, &[ORD]).unwrap();
        assert_eq!(l.raise(IFACE, ORD, &[9], None), Err(RaiseError::NotOwner));
        l.commit();
        assert!(p.next(&mut [0; 8]).unwrap().is_none());
        unchanged(&p);
    }
    #[test]
    fn listener_refuses_serve_without_side_effects() {
        let (_, mut p, mut l) = listener();
        assert_eq!(l.serve(IFACE, &[ORD]), Err(ServeError::NotOwner));
        let c = l.command(IFACE, ORD, &[9], None).unwrap();
        assert!(l.next_claim(&mut [0; 8]).unwrap().is_none());
        assert!(p.next_claim(&mut [0; 8]).unwrap().is_some());
        l.forget(c);
    }
    #[test]
    fn listener_reads_and_calls_but_never_claims() {
        let n = node();
        for name in ["alpha", "beta", "mapped"] {
            let c = catalog(&n, name);
            let who = if name == "beta" { id(3) } else { id(1) };
            let mut p = n.attach(who, &c).unwrap();
            let mut l = n.attach(InstanceNo::LISTENER, &c).unwrap();
            p.set(IFACE, ORD, &[42]).unwrap();
            p.commit();
            unchanged(&l);
            assert!(l.generation(IFACE) > 0);
            let mut marks = [Watermark {
                iface: IFACE,
                generation: 0,
            }];
            let mut changes = [Changed {
                iface: IFACE,
                ord: ORD,
                seq: 0,
            }];
            assert_eq!(l.scan(&mut marks, &mut changes), 1);
            let mut samples = [l.read(IFACE, ORD, &mut [0; 8]).unwrap()];
            assert_eq!(
                l.read_coherent(IFACE, &[ORD], &mut [0; 8], &mut samples)
                    .unwrap(),
                1
            );
            assert!(l.read_fixed(IFACE, ORD, &mut [0; 8]).is_err());
            l.subscribe(IFACE, &[ORD]).unwrap();
            p.raise(IFACE, ORD, &[7], None).unwrap();
            let mut out = [0; 8];
            assert_eq!(l.next(&mut out).unwrap().unwrap().len, 1);
            assert_eq!(out[0], 7);
            l.unsubscribe(IFACE, &[ORD]);
            p.raise(IFACE, ORD, &[8], None).unwrap();
            assert!(l.next(&mut out).unwrap().is_none());
            for query in [false, true] {
                let call = if query {
                    l.query(IFACE, ORD, &[3], None)
                } else {
                    l.command(IFACE, ORD, &[3], None)
                }
                .unwrap();
                assert!(l.next_claim(&mut out).unwrap().is_none());
                let claim = p.next_claim(&mut out).unwrap().unwrap();
                assert_eq!(out[0], 3);
                p.settle(claim.id, Ok(&[5])).unwrap();
                if query {
                    assert_eq!(l.reply(call, &mut out).unwrap(), Some(Ok(1)));
                    assert_eq!(out[0], 5);
                } else {
                    assert_eq!(l.ack(call), Some(Ok(())));
                }
                l.forget(call);
            }
        }
    }
}

#[test]
fn fixture_catalog_constants_match_descriptor() {
    use ridl_rt_conformance::identity::{ALPHA, BETA, MAPPED, REMOTE};
    let view = ridl_descriptor::deployment::verify(BYTES).unwrap();
    let actual: Vec<_> = view
        .catalogs()
        .unwrap()
        .into_iter()
        .map(|c| {
            let c = c.unwrap();
            CatalogRef {
                name: c.name().unwrap(),
                hash: CatalogHash(c.hash().unwrap().try_into().unwrap()),
            }
        })
        .collect();
    assert_eq!(actual, [ALPHA, BETA, MAPPED, REMOTE]);
}

#[cfg(not(feature = "debug"))]
#[test]
fn runtime_debug_does_not_enable_node_debug() {
    use std::{fs, process::Command};
    struct Probe(std::path::PathBuf);
    impl Drop for Probe {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let probe = Probe(std::env::temp_dir().join(format!(
        "ridl-loopback-feature-probe-{}-{}",
        std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
    )));
    fs::create_dir(&probe.0).unwrap();
    fs::create_dir(probe.0.join("src")).unwrap();
    fs::write(
        probe.0.join("Cargo.toml"),
        format!(
            r#"[package]
name = "loopback-feature-probe"
version = "0.0.0"
edition = "2021"
[workspace]
[dependencies]
ridl-loopback = {{ path = {loopback:?}, default-features = false, features = ["node"] }}
ridl-rt = {{ path = {runtime:?}, features = ["debug"] }}
"#,
            loopback = root,
            runtime = root.join("../ridl-rt"),
        ),
    )
    .unwrap();
    fs::write(probe.0.join("src/main.rs"), format!(
        r#"use ridl_loopback::{{AttachError, Node}};
use ridl_rt::contract::InstanceNo;
fn main() {{
    let node = Node::from_descriptor(include_bytes!({fixture:?}), "Local").unwrap();
    let catalog = *node.catalogs().find(|c| c.name == "alpha").unwrap();
    let positive = InstanceNo::new(1).unwrap();
    // Establish the grant, then release it so conflict cannot hide the feature guard.
    drop(node.attach(positive, &catalog).unwrap());
    assert!(matches!(node.attach(positive.twin().unwrap(), &catalog), Err(AttachError::UnknownInstance)));
    assert!(matches!(node.attach(InstanceNo::LISTENER, &catalog), Err(AttachError::UnknownInstance)));
}}
"#,
        fixture = root.join("../ridl-rt-conformance/fixtures/identity/identity.deployment.binfb"),
    )).unwrap();
    let output = Command::new(env!("CARGO"))
        .current_dir(&probe.0)
        .args(["run", "--offline", "--quiet"])
        .env("RUSTC_WRAPPER", "")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "probe failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
