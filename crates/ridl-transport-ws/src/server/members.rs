//! The member table of a catalog: which interactions it holds, under which
//! kind, and with which bound.

use std::collections::BTreeMap;
use std::time::Duration;

use ridl_rt::contract::{CatalogHash, InterfaceNo, Kind, Ordinal};

/// The members of one catalog, as the server needs them to refuse an
/// interaction the catalog does not hold (frame specification §6.4).
///
/// The table names its unit and carries the catalog hash, so that
/// [`Described::new`](super::Described::new) can check it against the ports
/// it is paired with. Each entry maps an interface number and an ordinal to
/// the member's kind, its `max` bound and its payload limit, the largest
/// number of argument bytes a `request` to the member may carry. A retired interface and a reserved
/// ordinal have no entry. The embedder builds the table with [`new`](Self::new)
/// and [`insert`](Self::insert), or, with the `descriptor` feature, with
/// `from_descriptor` from the bytes of a catalog descriptor.
#[derive(Clone, Debug)]
pub struct MemberTable {
    name: String,
    hash: CatalogHash,
    members: BTreeMap<(InterfaceNo, Ordinal), (Kind, Option<Duration>, Option<u32>)>,
    compatible: Vec<CatalogHash>,
}

impl MemberTable {
    /// An empty table for the unit `name` with catalog hash `hash`.
    pub fn new(name: impl Into<String>, hash: CatalogHash) -> Self {
        Self {
            name: name.into(),
            hash,
            members: BTreeMap::new(),
            compatible: Vec::new(),
        }
    }

    /// Inserts or replaces the entry under `(iface, ord)`; returns the entry
    /// it replaced. The new entry has no payload limit; a replaced entry's
    /// limit is dropped.
    pub fn insert(
        &mut self,
        iface: InterfaceNo,
        ord: Ordinal,
        kind: Kind,
        max: Option<Duration>,
    ) -> Option<(Kind, Option<Duration>)> {
        self.members
            .insert((iface, ord), (kind, max, None))
            .map(|(kind, max, _)| (kind, max))
    }

    /// Sets the payload limit of the entry under `(iface, ord)`; returns
    /// `false`, and stores nothing, when the table holds no entry under the key.
    pub fn set_limit(&mut self, iface: InterfaceNo, ord: Ordinal, limit: u32) -> bool {
        match self.members.get_mut(&(iface, ord)) {
            Some(entry) => {
                entry.2 = Some(limit);
                true
            }
            None => false,
        }
    }

    /// The payload limit of a member, or `None` for a key the catalog does not
    /// hold and for a member with no limit.
    pub fn limit(&self, iface: InterfaceNo, ord: Ordinal) -> Option<u32> {
        self.members.get(&(iface, ord)).and_then(|entry| entry.2)
    }

    /// Adds an earlier catalog hash this catalog is compatible with (§6.1).
    /// The own hash and a hash already listed are not added.
    pub fn add_compatible(&mut self, hash: CatalogHash) {
        if hash != self.hash && !self.compatible.contains(&hash) {
            self.compatible.push(hash);
        }
    }

    /// The compatible hashes, in insertion order.
    pub fn compatible(&self) -> &[CatalogHash] {
        &self.compatible
    }

    /// Whether an `attach` naming `hash` is accepted: the own hash or a compatible one.
    pub fn accepts(&self, hash: &CatalogHash) -> bool {
        *hash == self.hash || self.compatible.contains(hash)
    }

    /// The unit name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The catalog hash.
    pub fn hash(&self) -> &CatalogHash {
        &self.hash
    }

    /// The kind and the bound of a member, or `None` for a key the catalog
    /// does not hold.
    pub fn member(&self, iface: InterfaceNo, ord: Ordinal) -> Option<(Kind, Option<Duration>)> {
        self.members
            .get(&(iface, ord))
            .map(|&(kind, max, _)| (kind, max))
    }

    /// The number of members.
    pub fn len(&self) -> usize {
        self.members.len()
    }

    /// Whether the table holds no member.
    pub fn is_empty(&self) -> bool {
        self.members.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_table_is_empty_and_knows_no_member() {
        let t = MemberTable::new("ws.test", CatalogHash([7; 32]));
        assert_eq!(
            (t.name(), t.hash(), t.len(), t.is_empty()),
            ("ws.test", &CatalogHash([7; 32]), 0, true)
        );
        assert_eq!(t.member(InterfaceNo(1), Ordinal(1)), None);
    }

    #[test]
    fn insert_replaces_the_entry_under_the_same_key() {
        let mut t = MemberTable::new("ws.test", CatalogHash([7; 32]));
        assert_eq!(
            t.insert(InterfaceNo(1), Ordinal(1), Kind::Signal, None),
            None
        );
        assert_eq!(
            t.insert(
                InterfaceNo(1),
                Ordinal(1),
                Kind::Event,
                Some(Duration::from_secs(1))
            ),
            Some((Kind::Signal, None))
        );
        assert_eq!(
            t.member(InterfaceNo(1), Ordinal(1)),
            Some((Kind::Event, Some(Duration::from_secs(1))))
        );
        assert_eq!(t.len(), 1);
    }

    #[test]
    fn a_limit_is_set_on_an_existing_entry_only() {
        let mut t = MemberTable::new("ws.test", CatalogHash([7; 32]));
        assert!(!t.set_limit(InterfaceNo(1), Ordinal(1), 64));
        assert_eq!(t.limit(InterfaceNo(1), Ordinal(1)), None);
        assert_eq!(t.len(), 0);
        t.insert(InterfaceNo(1), Ordinal(1), Kind::Command, None);
        assert_eq!(t.limit(InterfaceNo(1), Ordinal(1)), None);
        assert!(t.set_limit(InterfaceNo(1), Ordinal(1), 64));
        assert_eq!(t.limit(InterfaceNo(1), Ordinal(1)), Some(64));
        assert_eq!(t.limit(InterfaceNo(1), Ordinal(2)), None);
    }

    #[test]
    fn insert_under_the_same_key_clears_the_limit() {
        let mut t = MemberTable::new("ws.test", CatalogHash([7; 32]));
        t.insert(InterfaceNo(1), Ordinal(1), Kind::Command, None);
        t.set_limit(InterfaceNo(1), Ordinal(1), 64);
        t.insert(InterfaceNo(1), Ordinal(1), Kind::Command, None);
        assert_eq!(t.limit(InterfaceNo(1), Ordinal(1)), None);
    }

    #[test]
    fn accepts_the_own_hash_and_each_compatible_one_and_lists_each_once() {
        let mut t = MemberTable::new("ws.test", CatalogHash([7; 32]));
        assert!(t.accepts(&CatalogHash([7; 32])));
        assert!(!t.accepts(&CatalogHash([6; 32])));
        t.add_compatible(CatalogHash([6; 32]));
        t.add_compatible(CatalogHash([6; 32]));
        t.add_compatible(CatalogHash([7; 32]));
        assert_eq!(t.compatible(), &[CatalogHash([6; 32])]);
        assert!(t.accepts(&CatalogHash([6; 32])));
        assert!(!t.accepts(&CatalogHash([5; 32])));
    }
}
