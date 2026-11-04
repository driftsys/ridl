//! Descriptor-based attachments to one machine's catalog stores.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::{Arc, Mutex, MutexGuard};

use ridl_descriptor::deployment;
use ridl_rt::contract::{CatalogHash, CatalogRef, InstanceNo};

use crate::Loopback;
mod attachment;
pub use attachment::Attachment;

// Only unit tests can pause an attach after its conflict check.
#[cfg(test)]
std::thread_local! {
    static AFTER_CONFLICT_CHECK: std::cell::RefCell<Option<Box<dyn FnOnce()>>> =
        const { std::cell::RefCell::new(None) };
}

type CatalogKey = (&'static str, CatalogHash);
type Reservations = Arc<Mutex<BTreeMap<i32, usize>>>;

fn reservations(state: &Reservations) -> MutexGuard<'_, BTreeMap<i32, usize>> {
    state
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// A failure to construct a machine's node.
#[derive(Debug)]
#[non_exhaustive]
pub enum NodeError {
    /// The descriptor or an embedded catalog failed verification.
    InvalidDescriptor(deployment::VerifyError),
    /// The requested machine is absent from the descriptor.
    UnknownMachine,
    /// An offer or map names a catalog not embedded in the descriptor.
    MissingCatalog,
}
impl fmt::Display for NodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidDescriptor(e) => write!(f, "invalid deployment descriptor: {e}"),
            Self::UnknownMachine => f.write_str("unknown machine"),
            Self::MissingCatalog => f.write_str("an offer or map names a missing catalog"),
        }
    }
}
impl std::error::Error for NodeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidDescriptor(e) => Some(e),
            _ => None,
        }
    }
}

/// A refusal to attach an identity to a catalog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum AttachError {
    /// The positive instance is not placed on this machine.
    UnknownInstance,
    /// No local store has this catalog name and hash.
    UnknownCatalog,
    /// The instance neither offers nor maps this catalog.
    CatalogNotGranted,
    /// An attachment of the opposite identity is still alive.
    IdentityConflict,
}
impl fmt::Display for AttachError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::UnknownInstance => "unknown local instance",
            Self::UnknownCatalog => "unknown local catalog",
            Self::CatalogNotGranted => "catalog not granted to instance",
            Self::IdentityConflict => "opposite identity is attached",
        })
    }
}
impl std::error::Error for AttachError {}

/// The catalog stores and attachment grants of one machine.
///
/// Available with `node`. `debug` additionally permits twins and read-only
/// listeners. Stores and reservations outlive this value while attachments live.
pub struct Node {
    stores: BTreeMap<CatalogKey, Loopback>,
    grants: BTreeMap<i32, BTreeSet<CatalogKey>>,
    reservations: Reservations,
}
impl Node {
    /// Verifies the descriptor before selecting the machine and its catalogs.
    /// The bytes remain alive because catalog names borrow them.
    pub fn from_descriptor(bytes: &'static [u8], machine: &str) -> Result<Self, NodeError> {
        let view = deployment::verify(bytes).map_err(NodeError::InvalidDescriptor)?;
        if !view
            .machines()
            .expect("verified")
            .into_iter()
            .any(|m| m.expect("verified").name().expect("verified") == machine)
        {
            return Err(NodeError::UnknownMachine);
        }
        let catalogs: Vec<_> = view
            .catalogs()
            .expect("verified")
            .into_iter()
            .map(|c| {
                let c = c.expect("verified");
                (
                    c.name().expect("verified"),
                    CatalogHash(
                        c.hash()
                            .expect("verified")
                            .try_into()
                            .expect("verified hash"),
                    ),
                )
            })
            .collect();
        let mut grants = BTreeMap::new();
        let mut stores = BTreeMap::new();
        for instance in view.instances().expect("verified") {
            let instance = instance.expect("verified");
            if instance.machine().expect("verified") != machine {
                continue;
            }
            let mut names = BTreeSet::new();
            for offer in instance.offers().expect("verified") {
                names.insert(offer.expect("verified").catalog().expect("verified"));
            }
            for name in instance.maps().expect("verified") {
                names.insert(name.expect("verified"));
            }
            let mut granted = BTreeSet::new();
            for name in names {
                let mut found = false;
                for &(embedded_name, hash) in &catalogs {
                    if embedded_name != name {
                        continue;
                    }
                    found = true;
                    let key = (embedded_name, hash);
                    granted.insert(key);
                    stores.entry(key).or_insert_with(|| {
                        Loopback::new(CatalogRef {
                            name: embedded_name,
                            hash,
                        })
                    });
                }
                if !found {
                    return Err(NodeError::MissingCatalog);
                }
            }
            grants.insert(instance.number().expect("verified"), granted);
        }
        Ok(Self {
            stores,
            grants,
            reservations: Arc::new(Mutex::new(BTreeMap::new())),
        })
    }

    /// The machine's deduplicated catalogs, ordered by name and hash.
    pub fn catalogs(&self) -> impl Iterator<Item = &CatalogRef> {
        use ridl_rt::port::Attached;
        self.stores.values().map(|store| store.catalog())
    }

    /// Attaches with the instance's grant and reserves its identity across catalogs.
    pub fn attach(&self, who: InstanceNo, catalog: &CatalogRef) -> Result<Attachment, AttachError> {
        let number = who.get();
        let listener = is_listener(who);
        let positive = if listener {
            number
        } else {
            if number < 0 && !cfg!(feature = "debug") {
                return Err(AttachError::UnknownInstance);
            }
            number.checked_abs().ok_or(AttachError::UnknownInstance)?
        };
        let grant = if listener {
            None
        } else {
            Some(
                self.grants
                    .get(&positive)
                    .ok_or(AttachError::UnknownInstance)?,
            )
        };
        let key = (catalog.name, catalog.hash);
        let store = self.stores.get(&key).ok_or(AttachError::UnknownCatalog)?;
        if grant.is_some_and(|g| !g.contains(&key)) {
            return Err(AttachError::CatalogNotGranted);
        }
        {
            let mut counts = reservations(&self.reservations);
            if !listener && counts.contains_key(&-number) {
                return Err(AttachError::IdentityConflict);
            }
            #[cfg(test)]
            AFTER_CONFLICT_CHECK.with(|hook| {
                if let Some(pause) = hook.borrow_mut().take() {
                    pause();
                }
            });
            *counts.entry(number).or_default() += 1;
        }
        Ok(Attachment {
            ports: store.attach(),
            _reservation: Reservation {
                state: Arc::clone(&self.reservations),
                number,
            },
            listener,
        })
    }
}
fn is_listener(who: InstanceNo) -> bool {
    #[cfg(feature = "debug")]
    {
        who.is_listener()
    }
    #[cfg(not(feature = "debug"))]
    {
        let _ = who;
        false
    }
}

// Dropped after the attachment's ports, so a replacement cannot overlap them.
struct Reservation {
    state: Reservations,
    number: i32,
}
impl Drop for Reservation {
    fn drop(&mut self) {
        let mut counts = reservations(&self.state);
        let count = counts.get_mut(&self.number).expect("live reservation");
        *count -= 1;
        if *count == 0 {
            counts.remove(&self.number);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "debug")]
    use std::sync::{mpsc, TryLockError};

    #[test]
    fn attachment_reads_fixed_bytes_and_coherent_sample_metadata() {
        use ridl_rt::contract::{InterfaceNo, Ordinal};
        use ridl_rt::port::*;
        use ridl_rt::sample::{Duration, Envelope, Freshness, Provenance, Timestamp};
        let catalog = CatalogRef {
            name: "test",
            hash: CatalogHash([0; 32]),
        };
        let key = (catalog.name, catalog.hash);
        let mut node = Node {
            stores: BTreeMap::from([(key, Loopback::new(catalog))]),
            grants: BTreeMap::from([(1, BTreeSet::from([key])), (3, BTreeSet::from([key]))]),
            reservations: Arc::new(Mutex::new(BTreeMap::new())),
        };
        let iface = InterfaceNo(1);
        let key = (catalog.name, catalog.hash);
        let store = node.stores.get_mut(&key).unwrap();
        store
            .provision_fixed(iface, Ordinal(5), &[31, 32, 33])
            .unwrap();
        store.advance(Duration(17));
        let mut publisher = node.attach(InstanceNo::new(1).unwrap(), &catalog).unwrap();
        let reader = node.attach(InstanceNo::new(3).unwrap(), &catalog).unwrap();
        publisher.set(iface, Ordinal(1), &[41, 42]).unwrap();
        publisher.set(iface, Ordinal(6), &[51]).unwrap();
        publisher.commit();
        let mut out = [0; 8];
        assert_eq!(reader.read_fixed(iface, Ordinal(5), &mut out).unwrap(), 3);
        assert_eq!(&out[..3], &[31, 32, 33]);
        let blank = RawSample {
            provenance: Provenance::Init,
            freshness: Freshness::Unbounded,
            envelope: Envelope {
                stamp: Timestamp(0),
                seq: 0,
            },
            len: 0,
        };
        let mut samples = [blank; 2];
        assert_eq!(
            reader
                .read_coherent(iface, &[Ordinal(6), Ordinal(1)], &mut out, &mut samples)
                .unwrap(),
            3
        );
        assert_eq!(&out[..3], &[51, 41, 42]);
        assert_eq!(
            samples,
            [
                RawSample {
                    provenance: Provenance::Live,
                    freshness: Freshness::Unbounded,
                    envelope: Envelope {
                        stamp: Timestamp(17),
                        seq: 1
                    },
                    len: 1
                },
                RawSample {
                    provenance: Provenance::Live,
                    freshness: Freshness::Unbounded,
                    envelope: Envelope {
                        stamp: Timestamp(17),
                        seq: 1
                    },
                    len: 2
                },
            ]
        );
    }

    #[cfg(feature = "debug")]
    #[test]
    fn simultaneous_opposites_have_one_winner() {
        let catalog = CatalogRef {
            name: "test",
            hash: CatalogHash([0; 32]),
        };
        let key = (catalog.name, catalog.hash);
        let positive = InstanceNo::new(1).unwrap();
        let twin = positive.twin().unwrap();
        for (first, second) in [(positive, twin), (twin, positive)] {
            let node = Node {
                stores: BTreeMap::from([(key, Loopback::new(catalog))]),
                grants: BTreeMap::from([(1, BTreeSet::from([key]))]),
                reservations: Arc::new(Mutex::new(BTreeMap::new())),
            };
            std::thread::scope(|scope| {
                let (paused_tx, paused_rx) = mpsc::channel();
                let (resume_tx, resume_rx) = mpsc::channel();
                let node_ref = &node;
                let first_thread = scope.spawn(move || {
                    AFTER_CONFLICT_CHECK.with(|hook| {
                        *hook.borrow_mut() = Some(Box::new(move || {
                            paused_tx.send(()).unwrap();
                            resume_rx.recv().unwrap();
                        }));
                    });
                    node_ref.attach(first, &catalog)
                });
                paused_rx.recv().unwrap();
                // Probe from the competing thread while the first attach is paused.
                // If the check released its lock, complete the opposite attach first.
                let locked = matches!(node.reservations.try_lock(), Err(TryLockError::WouldBlock));
                let second_result = if locked {
                    resume_tx.send(()).unwrap();
                    node.attach(second, &catalog)
                } else {
                    let result = node.attach(second, &catalog);
                    resume_tx.send(()).unwrap();
                    result
                };
                let first_result = first_thread.join().unwrap();
                let results = [first_result, second_result];
                assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
                assert!(results
                    .iter()
                    .any(|r| matches!(r, Err(AttachError::IdentityConflict))));
                assert!(
                    locked,
                    "conflict check must retain the reservation lock until increment"
                );
            });
        }
    }
}
