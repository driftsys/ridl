//! Identity, and the interaction descriptors generated code writes.
//!
//! A descriptor is a zero-sized marker type with constants. The descriptors
//! are the per-member form of the ordinal table of ADR-0013 decision 3,
//! extended with the interface number and the catalog hash. Their fields have
//! the names and meanings of the catalog descriptor's fields.

/// A member's ordinal: its position in the interface body, counted from 1
/// (ridl §11). An ordinal is never 0.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Ordinal(pub u32);

/// An interface's number in its catalog: the number the lock file froze, or a
/// provisional number. An interface number is never 0.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct InterfaceNo(pub u32);

/// The catalog hash: SHA-256 over the catalog's interfaces, their numbers, and
/// the types they reach.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CatalogHash(pub [u8; 32]);

/// A catalog: one package's interfaces.
///
/// Two `CatalogRef`s are equal only when both the names and the hashes are
/// equal, because two packages with the same contents can have the same hash.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CatalogRef {
    /// The package name.
    pub name: &'static str,
    /// The catalog hash.
    pub hash: CatalogHash,
}

/// The five interaction kinds of the language.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Kind {
    /// A `signal`.
    Signal = 1,
    /// An `event`.
    Event = 2,
    /// A `command`.
    Command = 3,
    /// A `query`.
    Query = 4,
    /// A `fixed`.
    Fixed = 5,
}

#[cfg(test)]
mod tests {
    use super::{CatalogHash, CatalogRef, Kind};

    #[test]
    fn catalog_refs_are_equal_only_when_name_and_hash_are_equal() {
        let base = CatalogRef {
            name: "vehicle",
            hash: CatalogHash([1; 32]),
        };
        let same = CatalogRef {
            name: "vehicle",
            hash: CatalogHash([1; 32]),
        };
        let other_hash = CatalogRef {
            name: "vehicle",
            hash: CatalogHash([2; 32]),
        };
        let other_name = CatalogRef {
            name: "cabin",
            hash: CatalogHash([1; 32]),
        };
        assert_eq!(base, same);
        assert_ne!(base, other_hash);
        assert_ne!(base, other_name);
    }

    #[test]
    fn kind_values_are_the_language_order_from_one() {
        assert_eq!(Kind::Signal as u8, 1);
        assert_eq!(Kind::Event as u8, 2);
        assert_eq!(Kind::Command as u8, 3);
        assert_eq!(Kind::Query as u8, 4);
        assert_eq!(Kind::Fixed as u8, 5);
    }
}
