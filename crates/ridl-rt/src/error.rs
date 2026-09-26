//! The error strata of ridl §10 that a runtime reports, and the two errors a
//! generated face returns.
//!
//! Stratum 1, the errors an interface declares, is data and travels in a
//! payload (§10.1). This module holds stratum 2, the contract errors (§10.2),
//! and stratum 3, the detected infrastructure failures (§10.3). It also holds
//! [`ClientError`] and [`ProviderError`], which compose those with the port
//! errors into one error per side of a call (ADR-0021 decision 16).

use crate::payload::Violation;
use crate::port::{ReadError, SendError, ServeError};

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
    /// The providing runtime refused the call at admission and the caller may
    /// retry later. Crosses the frame as a `response` outcome (frame
    /// specification §9.6).
    Busy,
}

/// The outcome of a call that did not succeed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CallError {
    /// A contract error.
    Contract(Contract),
    /// An infrastructure failure.
    Transport(Transport),
}

/// The error of a generated client call.
///
/// One type for every call of every generated client, so that `?` works with
/// one type and an application over two generated packages handles one
/// error. A send failure is here and not in [`CallError`], because it is not
/// a settlement outcome: nothing was sent, and no provider settled anything.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClientError {
    /// The call was not sent.
    Send(SendError),
    /// The call was sent and its outcome is a failure.
    Call(CallError),
    /// The port failed while the outcome was read. Only
    /// [`ReadError::Detached`] is reachable, because the reply buffer is sized
    /// from the reply's `MAX_SIZE`; it is kept as what it is and not mapped
    /// onto a [`Transport`] variant, which would give a local failure a
    /// frame-level meaning.
    Read(ReadError),
}

impl From<SendError> for ClientError {
    fn from(error: SendError) -> Self {
        ClientError::Send(error)
    }
}

impl From<CallError> for ClientError {
    fn from(error: CallError) -> Self {
        ClientError::Call(error)
    }
}

impl From<ReadError> for ClientError {
    fn from(error: ReadError) -> Self {
        ClientError::Read(error)
    }
}

/// The error a generated `serve` resolves to.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProviderError {
    /// `Handler::serve` refused the interface's members.
    Serve(ServeError),
    /// The handler port failed while a claim was read; the claims already
    /// settled stay settled.
    Claim(ReadError),
}

impl From<ServeError> for ProviderError {
    fn from(error: ServeError) -> Self {
        ProviderError::Serve(error)
    }
}

impl From<ReadError> for ProviderError {
    fn from(error: ReadError) -> Self {
        ProviderError::Claim(error)
    }
}

#[cfg(test)]
mod tests {
    use super::{CallError, ClientError, Contract, ProviderError, Transport};
    use crate::port::{ReadError, SendError, ServeError};

    fn owns_nothing<T: Copy + Eq + core::fmt::Debug + 'static>() {}

    #[test]
    fn every_error_is_copy_and_owns_nothing() {
        owns_nothing::<Contract>();
        owns_nothing::<Transport>();
        owns_nothing::<CallError>();
        owns_nothing::<ClientError>();
        owns_nothing::<ProviderError>();
    }

    /// `?` over a port error or a call outcome in a function returning
    /// `ClientError` lands in the variant for that side of the call.
    #[test]
    fn each_inner_error_converts_into_its_own_client_variant() {
        fn send() -> Result<(), ClientError> {
            Err(SendError::Busy)?
        }
        fn call() -> Result<(), ClientError> {
            Err(CallError::Transport(Transport::Timeout))?
        }
        fn read() -> Result<(), ClientError> {
            Err(ReadError::Detached)?
        }
        assert_eq!(send(), Err(ClientError::Send(SendError::Busy)));
        assert_eq!(
            call(),
            Err(ClientError::Call(CallError::Transport(Transport::Timeout)))
        );
        assert_eq!(read(), Err(ClientError::Read(ReadError::Detached)));
    }

    #[test]
    fn each_inner_error_converts_into_its_own_provider_variant() {
        fn serve() -> Result<(), ProviderError> {
            Err(ServeError::NotOwner)?
        }
        fn claim() -> Result<(), ProviderError> {
            Err(ReadError::Detached)?
        }
        assert_eq!(serve(), Err(ProviderError::Serve(ServeError::NotOwner)));
        assert_eq!(claim(), Err(ProviderError::Claim(ReadError::Detached)));
    }
}
