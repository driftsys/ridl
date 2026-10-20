//! The backend of one session: the ports the server reads and calls.

use ridl_rt::contract::{InterfaceNo, Ordinal};
use ridl_rt::error::Contract;
use ridl_rt::port::{
    Attached, Caller, Clock, Encoded, EventSource, FixedReader, ReadError, ScannableSignals,
    SignalReader,
};

/// The ports one session is served from.
///
/// One value per connection: its ports have their own event queue (their
/// [`EventSource`]) and their own sequence counter (their [`Caller`]), which
/// is what makes the caller's identity the connection (frame specification
/// §11.1 item 5). The value is dropped at the end of the session, after every
/// call in flight has been forgotten, and the backend's event subscriptions
/// end with it: the server does not unsubscribe them one by one, so a backend
/// whose subscriptions outlive its port value must release them in its own
/// `Drop`. The ports are an associated type rather than supertraits so
/// that an embedder can serve a runtime type it does not own: the orphan rule
/// forbids implementing this trait on another crate's type, and [`Polling`]
/// and [`Scanning`] cover the two common cases with no code. A backend with
/// one optional capability and not the other implements the three methods
/// itself.
pub trait Backend {
    /// The port set every session needs.
    type Ports: Attached + Encoded + Clock + SignalReader + EventSource + Caller;

    /// The ports.
    fn ports(&mut self) -> &mut Self::Ports;

    /// The signal store as a scannable view, when the runtime has one. With
    /// `Some`, a tick with no change costs one `scan` call; with `None`, a
    /// tick reads every subscribed signal.
    fn scannable(&self) -> Option<&dyn ScannableSignals> {
        None
    }

    /// Copies a `fixed`'s provisioned value into the front of `out` and
    /// returns its length. The default serves no `fixed`: every read is
    /// answered `Contract::UnknownInteraction`.
    fn read_fixed(
        &self,
        iface: InterfaceNo,
        ord: Ordinal,
        out: &mut [u8],
    ) -> Result<usize, ReadError> {
        let _ = (iface, ord, out);
        Err(ReadError::Contract(Contract::UnknownInteraction))
    }
}

/// A backend over ports with no scannable view and no `fixed`: the server
/// reads every subscribed signal on each tick.
pub struct Polling<P>(pub P);

impl<P> Backend for Polling<P>
where
    P: Attached + Encoded + Clock + SignalReader + EventSource + Caller,
{
    type Ports = P;

    fn ports(&mut self) -> &mut P {
        &mut self.0
    }
}

/// A backend over ports that present [`ScannableSignals`] and
/// [`FixedReader`], such as `ridl-loopback`'s aggregate handle.
pub struct Scanning<P>(pub P);

impl<P> Backend for Scanning<P>
where
    P: Attached
        + Encoded
        + Clock
        + SignalReader
        + EventSource
        + Caller
        + ScannableSignals
        + FixedReader,
{
    type Ports = P;

    fn ports(&mut self) -> &mut P {
        &mut self.0
    }

    fn scannable(&self) -> Option<&dyn ScannableSignals> {
        Some(&self.0)
    }

    fn read_fixed(
        &self,
        iface: InterfaceNo,
        ord: Ordinal,
        out: &mut [u8],
    ) -> Result<usize, ReadError> {
        self.0.read_fixed(iface, ord, out)
    }
}
