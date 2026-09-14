//! The error strata of ridl §10 that a runtime reports.
//!
//! Stratum 1, the errors an interface declares, is data and travels in a
//! payload (§10.1). This module holds stratum 2, the contract errors (§10.2),
//! and stratum 3, the detected infrastructure failures (§10.3).

use crate::payload::Violation;

/// A contract error, ridl §10.2. Derived from the contract and never declared.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Contract {
    /// `INVALID_VALUE`: a payload breaks its typl constraints.
    InvalidValue(Violation),
    /// `PRECONDITION_FAILED`: a `require` clause evaluates to false.
    PreconditionFailed,
    /// `CONTRACT_BROKEN`: an `ensure` clause evaluates to false.
    ContractBroken,
    /// `UNKNOWN_INTERACTION`: the peers disagree on an interface number or an
    /// ordinal.
    UnknownInteraction,
}

/// An infrastructure failure that the runtime detected, ridl §10.3.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Transport {
    /// The response bound passed without a reply.
    Timeout,
    /// A command received no delivery acknowledgment within its bound.
    Undelivered,
    /// The connection to the peer is lost.
    Down,
    /// A payload is not a well-formed encoding.
    Corrupt,
}

/// The outcome of a call that did not succeed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CallError {
    /// A contract error.
    Contract(Contract),
    /// An infrastructure failure.
    Transport(Transport),
}

#[cfg(test)]
mod tests {
    use super::{CallError, Contract, Transport};

    fn owns_nothing<T: Copy + Eq + core::fmt::Debug + 'static>() {}

    #[test]
    fn every_error_is_copy_and_owns_nothing() {
        owns_nothing::<Contract>();
        owns_nothing::<Transport>();
        owns_nothing::<CallError>();
    }
}
