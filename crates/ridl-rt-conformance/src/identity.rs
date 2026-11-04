//! Descriptor-based attachment tests, selected with `suite!(F; identity)`.
//!
//! Implement [`IdentityFactory`] to share one node between attachments. The
//! `identity_debug` flag additionally checks twins and listeners and requires
//! this crate's `debug` feature. No loopback or compiler dependency is needed.

use ridl_rt::contract::{CatalogHash, CatalogRef};
#[cfg(feature = "debug")]
use ridl_rt::contract::{InterfaceNo, Ordinal};
use ridl_rt::port::*;

/// The `alpha` catalog of the identity fixture.
pub const ALPHA: CatalogRef = CatalogRef {
    name: "alpha",
    hash: CatalogHash([
        14, 60, 221, 186, 156, 205, 215, 219, 138, 114, 25, 248, 25, 135, 168, 28, 142, 146, 39,
        76, 181, 65, 141, 187, 106, 210, 72, 117, 79, 48, 188, 186,
    ]),
};

/// The `beta` catalog of the identity fixture.
pub const BETA: CatalogRef = CatalogRef {
    name: "beta",
    hash: CatalogHash([
        106, 51, 207, 177, 105, 204, 94, 46, 138, 153, 55, 124, 40, 244, 103, 187, 255, 250, 147,
        162, 24, 114, 164, 87, 184, 81, 11, 222, 103, 180, 64, 60,
    ]),
};

/// The `mapped` catalog of the identity fixture.
pub const MAPPED: CatalogRef = CatalogRef {
    name: "mapped",
    hash: CatalogHash([
        125, 88, 71, 62, 114, 234, 207, 220, 77, 103, 47, 109, 172, 59, 11, 223, 77, 34, 114, 80,
        30, 91, 114, 52, 47, 216, 210, 202, 228, 234, 72, 169,
    ]),
};

/// The `remote` catalog of the identity fixture.
pub const REMOTE: CatalogRef = CatalogRef {
    name: "remote",
    hash: CatalogHash([
        33, 39, 131, 90, 243, 30, 21, 66, 212, 240, 12, 242, 114, 180, 159, 236, 253, 9, 87, 186,
        172, 195, 179, 109, 8, 97, 166, 74, 34, 7, 114, 94,
    ]),
};

/// The deployment descriptor shared by the identity tests.
pub const DESCRIPTOR: &[u8] = include_bytes!("../fixtures/identity/identity.deployment.binfb");

/// The node and attachment hooks required by the opt-in identity group.
///
/// `Local` in [`DESCRIPTOR`] places instances 1 and 3; instance 2 is remote.
/// Instance 1 offers [`ALPHA`] and maps [`MAPPED`]; instance 3 offers
/// [`BETA`] and maps [`ALPHA`]. Each call to `node` creates independent state.
/// Attachments share stores and identity reservations within that node.
pub trait IdentityFactory {
    /// One machine's stores and grants.
    type Node;
    /// A reserved identity with checked access to its catalog's ports.
    type Attachment: Attached
        + Clock
        + SignalReader
        + SignalWriter
        + EventSource
        + EventSink
        + Caller
        + Handler;
    /// An attachment refusal, whose concrete variants are runtime-specific.
    type Error: core::fmt::Debug;

    /// Builds the named machine from a verified deployment descriptor.
    fn node(bytes: &'static [u8], machine: &str) -> Self::Node;
    /// Reserves an identity on a granted catalog, or refuses it.
    fn attach(
        node: &Self::Node,
        who: ridl_rt::contract::InstanceNo,
        catalog: &CatalogRef,
    ) -> Result<Self::Attachment, Self::Error>;
}

fn id(number: i32) -> ridl_rt::contract::InstanceNo {
    ridl_rt::contract::InstanceNo::new(number).unwrap()
}

#[cfg(feature = "debug")]
const IFACE: InterfaceNo = InterfaceNo(1);
#[cfg(feature = "debug")]
// fixtures/identity/{alpha,beta,mapped,remote}/face.ridl declares Face members
// in ordinal order: value (signal) 1, changed (event) 2, put(value) (command) 3,
// get() -> Value (query) 4. Keep these constants and call signatures in sync.
const SIGNAL: Ordinal = Ordinal(1);
#[cfg(feature = "debug")]
const EVENT: Ordinal = Ordinal(2);
#[cfg(feature = "debug")]
const COMMAND: Ordinal = Ordinal(3);
#[cfg(feature = "debug")]
const QUERY: Ordinal = Ordinal(4);

/// Local instances receive only their offered and mapped catalogs.
pub fn identity_positive_grants<F: IdentityFactory>() {
    let node = F::node(DESCRIPTOR, "Local");
    for (number, catalog) in [(1, ALPHA), (1, MAPPED), (3, ALPHA), (3, BETA)] {
        let attachment = F::attach(&node, id(number), &catalog).unwrap();
        assert_eq!(attachment.catalog(), &catalog);
    }
    for (number, catalog) in [
        (1, BETA),
        (3, MAPPED),
        (2, REMOTE),
        (2, ALPHA),
        (999, ALPHA),
        (1, REMOTE),
    ] {
        assert!(
            F::attach(&node, id(number), &catalog).is_err(),
            "{number} / {catalog:?}"
        );
    }
    let wrong = CatalogRef {
        hash: CatalogHash([0; 32]),
        ..ALPHA
    };
    assert!(F::attach(&node, id(1), &wrong).is_err());
}

/// A twin receives exactly the grants of its positive counterpart.
#[cfg(feature = "debug")]
pub fn identity_twin_grants<F: IdentityFactory>() {
    let node = F::node(DESCRIPTOR, "Local");
    for number in [1, 3, 2, 999] {
        for catalog in [ALPHA, BETA, MAPPED, REMOTE] {
            let positive = F::attach(&node, id(number), &catalog);
            let granted = positive.is_ok();
            drop(positive);
            let twin = F::attach(&node, id(number).twin().unwrap(), &catalog);
            assert_eq!(twin.is_ok(), granted, "{number} / {catalog:?}");
            if let Ok(mut twin) = twin {
                assert_eq!(twin.catalog(), &catalog);
                twin.set(IFACE, SIGNAL, &[7]).unwrap();
                twin.commit();
                let mut out = [0; 8];
                let observer =
                    F::attach(&node, ridl_rt::contract::InstanceNo::LISTENER, &catalog).unwrap();
                assert_eq!(observer.read(IFACE, SIGNAL, &mut out).unwrap().len, 1);
                assert_eq!(out[0], 7);
            }
        }
    }
    for (number, catalog) in [(1, ALPHA), (1, MAPPED), (3, ALPHA), (3, BETA)] {
        let wrong = CatalogRef {
            hash: CatalogHash([0; 32]),
            ..catalog
        };
        assert!(
            F::attach(&node, id(number).twin().unwrap(), &wrong).is_err(),
            "twin of {number} must reject a wrong hash for {}",
            catalog.name
        );
    }
}

/// Opposite identities exclude each other across catalogs in either order.
#[cfg(feature = "debug")]
pub fn identity_exclusion_both_orders<F: IdentityFactory>() {
    // Opposites exclude only their own positive instance, in either order.
    for (first, second) in [(1_i32, -3_i32), (-3, 1), (3, -1), (-1, 3)] {
        let node = F::node(DESCRIPTOR, "Local");
        let signed = |number: i32| {
            if number < 0 {
                id(-number).twin().unwrap()
            } else {
                id(number)
            }
        };
        let a = F::attach(&node, signed(first), &ALPHA).unwrap();
        let b = F::attach(&node, signed(second), &ALPHA).unwrap();
        drop((a, b));
    }
    let positive = id(1);
    let twin = positive.twin().unwrap();
    for (first, second) in [(positive, twin), (twin, positive)] {
        let node = F::node(DESCRIPTOR, "Local");
        let attachment = F::attach(&node, first, &ALPHA).unwrap();
        assert!(F::attach(&node, second, &MAPPED).is_err());
        drop(attachment);
        drop(F::attach(&node, second, &MAPPED).unwrap());
        assert!(F::attach(&node, first, &ALPHA).is_ok());
    }
}

/// Only the last equal identity's drop permits its opposite to attach.
#[cfg(feature = "debug")]
pub fn identity_last_drop<F: IdentityFactory>() {
    let positive = id(1);
    let twin = positive.twin().unwrap();
    for (first, second) in [(positive, twin), (twin, positive)] {
        let node = F::node(DESCRIPTOR, "Local");
        assert!(F::attach(&node, first, &BETA).is_err());
        drop(F::attach(&node, second, &ALPHA).unwrap());
        let a = F::attach(&node, first, &ALPHA).unwrap();
        let b = F::attach(&node, first, &MAPPED).unwrap();
        drop(a);
        assert!(F::attach(&node, second, &ALPHA).is_err());
        drop(b);
        assert!(F::attach(&node, second, &ALPHA).is_ok());
    }
}

/// A listener reads each catalog's own store.
#[cfg(feature = "debug")]
pub fn identity_listener_all_catalogs<F: IdentityFactory>() {
    for who in [id(1), id(1).twin().unwrap()] {
        for listener_first in [false, true] {
            let node = F::node(DESCRIPTOR, "Local");
            let (first, second) = if listener_first {
                (ridl_rt::contract::InstanceNo::LISTENER, who)
            } else {
                (who, ridl_rt::contract::InstanceNo::LISTENER)
            };
            let a = F::attach(&node, first, &ALPHA).unwrap();
            let b = F::attach(&node, second, &ALPHA).unwrap();
            let c = F::attach(&node, ridl_rt::contract::InstanceNo::LISTENER, &ALPHA).unwrap();
            drop((a, b, c));
        }
    }
    let node = F::node(DESCRIPTOR, "Local");
    let mut providers = Vec::new();
    for (number, catalog, byte) in [(1, ALPHA, 11), (3, BETA, 22), (1, MAPPED, 33)] {
        let mut provider = F::attach(&node, id(number), &catalog).unwrap();
        provider.set(IFACE, SIGNAL, &[byte]).unwrap();
        provider.commit();
        providers.push(provider);
    }
    for (catalog, byte) in [(ALPHA, 11), (BETA, 22), (MAPPED, 33)] {
        let listener = F::attach(&node, ridl_rt::contract::InstanceNo::LISTENER, &catalog).unwrap();
        assert_eq!(listener.catalog(), &catalog);
        let mut out = [0; 8];
        assert_eq!(listener.read(IFACE, SIGNAL, &mut out).unwrap().len, 1);
        assert_eq!(out[0], byte);
    }
    assert!(F::attach(&node, ridl_rt::contract::InstanceNo::LISTENER, &REMOTE).is_err());
}

#[cfg(feature = "debug")]
fn listener<F: IdentityFactory>() -> (F::Node, F::Attachment, F::Attachment) {
    let node = F::node(DESCRIPTOR, "Local");
    let mut provider = F::attach(&node, id(1), &ALPHA).unwrap();
    provider.set(IFACE, SIGNAL, &[42]).unwrap();
    provider.commit();
    let listener = F::attach(&node, ridl_rt::contract::InstanceNo::LISTENER, &ALPHA).unwrap();
    (node, provider, listener)
}

#[cfg(feature = "debug")]
fn unchanged(provider: &impl SignalReader) {
    let mut out = [0; 8];
    assert_eq!(provider.read(IFACE, SIGNAL, &mut out).unwrap().len, 1);
    assert_eq!(out[0], 42);
}

/// A listener cannot stage a value.
#[cfg(feature = "debug")]
pub fn identity_listener_write_refusals_set<F: IdentityFactory>() {
    let (_node, provider, mut listener) = listener::<F>();
    assert_eq!(listener.set(IFACE, SIGNAL, &[9]), Err(WriteError::NotOwner));
    listener.commit();
    unchanged(&provider);
}

/// A listener cannot stage an invalidation.
#[cfg(feature = "debug")]
pub fn identity_listener_write_refusals_invalidate<F: IdentityFactory>() {
    let (_node, provider, mut listener) = listener::<F>();
    assert_eq!(
        listener.invalidate(IFACE, SIGNAL),
        Err(WriteError::NotOwner)
    );
    listener.commit();
    unchanged(&provider);
}

/// A listener cannot stage a touch.
#[cfg(feature = "debug")]
pub fn identity_listener_write_refusals_touch<F: IdentityFactory>() {
    let (_node, provider, mut listener) = listener::<F>();
    assert_eq!(listener.touch(IFACE, SIGNAL), Err(WriteError::NotOwner));
    listener.commit();
    unchanged(&provider);
}

/// A listener cannot raise an event.
#[cfg(feature = "debug")]
pub fn identity_listener_write_refusals_raise<F: IdentityFactory>() {
    let (_node, mut provider, mut listener) = listener::<F>();
    provider.subscribe(IFACE, &[EVENT]).unwrap();
    assert_eq!(
        listener.raise(IFACE, EVENT, &[9], None),
        Err(RaiseError::NotOwner)
    );
    listener.commit();
    assert!(provider.next(&mut [0; 8]).unwrap().is_none());
}

/// A listener cannot serve or claim a call.
#[cfg(feature = "debug")]
pub fn identity_listener_write_refusals_serve<F: IdentityFactory>() {
    let (_node, mut provider, mut listener) = listener::<F>();
    provider.serve(IFACE, &[COMMAND]).unwrap();
    assert_eq!(listener.serve(IFACE, &[COMMAND]), Err(ServeError::NotOwner));
    let call = listener.command(IFACE, COMMAND, &[9], None).unwrap();
    assert!(listener.next_claim(&mut [0; 8]).unwrap().is_none());
    let claim = provider.next_claim(&mut [0; 8]).unwrap().unwrap();
    provider.settle(claim.id, Ok(&[])).unwrap();
    listener.forget(call);
}

/// A listener can complete commands and queries through a positive provider.
#[cfg(feature = "debug")]
pub fn identity_listener_can_call<F: IdentityFactory>() {
    let (_node, mut provider, mut listener) = listener::<F>();
    provider.serve(IFACE, &[COMMAND, QUERY]).unwrap();
    for query in [false, true] {
        let call = if query {
            listener.query(IFACE, QUERY, &[], None)
        } else {
            listener.command(IFACE, COMMAND, &[3], None)
        }
        .unwrap();
        let mut out = [0; 8];
        assert!(listener.next_claim(&mut out).unwrap().is_none());
        let claim = provider.next_claim(&mut out).unwrap().unwrap();
        assert_eq!(&out[..claim.len], if query { &[][..] } else { &[3][..] });
        provider
            .settle(claim.id, Ok(if query { &[5] } else { &[] }))
            .unwrap();
        if query {
            assert_eq!(listener.reply(call, &mut out).unwrap(), Some(Ok(1)));
            assert_eq!(out[0], 5);
        } else {
            assert_eq!(listener.ack(call), Some(Ok(())));
        }
        listener.forget(call);
    }
}
