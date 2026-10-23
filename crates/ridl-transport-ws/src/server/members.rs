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
/// the member's kind and its `max` bound. A retired interface and a reserved
/// ordinal have no entry. The embedder builds the table with [`new`](Self::new)
/// and [`insert`](Self::insert).
#[derive(Clone, Debug)]
pub struct MemberTable {
    name: String,
    hash: CatalogHash,
    members: BTreeMap<(InterfaceNo, Ordinal), (Kind, Option<Duration>)>,
}

impl MemberTable {
    /// An empty table for the unit `name` with catalog hash `hash`.
    pub fn new(name: impl Into<String>, hash: CatalogHash) -> Self {
        Self {
            name: name.into(),
            hash,
            members: BTreeMap::new(),
        }
    }

    /// Inserts or replaces the entry under `(iface, ord)`; returns the entry
    /// it replaced.
    pub fn insert(
        &mut self,
        iface: InterfaceNo,
        ord: Ordinal,
        kind: Kind,
        max: Option<Duration>,
    ) -> Option<(Kind, Option<Duration>)> {
        self.members.insert((iface, ord), (kind, max))
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
        self.members.get(&(iface, ord)).copied()
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
}
