//! The local ports into which a remote provider delivers.

use ridl_rt::port::{Attached, Clock, Encoded, EventSink, Handler, SignalWriter};

use super::{MemberTable, TableMismatch};

/// The local destination of one remote provider's session.
///
/// The table contains only members offered by this instance in this catalog.
/// The embedder filters it before construction. The backend is dropped on
/// every session exit, including selection and handshake failures.
pub trait ProvidedBackend {
    /// The local providing ports.
    type Ports: Attached + Encoded + Clock + SignalWriter + EventSink + Handler;
    /// The ports.
    fn ports(&mut self) -> &mut Self::Ports;
    /// Only the members this instance offers.
    fn members(&self) -> &MemberTable;
}

/// Providing ports paired with their offered member table.
pub struct Providing<P> {
    ports: P,
    members: MemberTable,
}

impl<P: Attached> Providing<P> {
    /// Pairs the ports and table, refusing a different catalog name or hash.
    pub fn new(ports: P, members: MemberTable) -> Result<Self, TableMismatch> {
        let catalog = *ports.catalog();
        if members.name() != catalog.name || *members.hash() != catalog.hash {
            return Err(TableMismatch {
                ports: catalog,
                name: members.name().to_owned(),
                hash: *members.hash(),
            });
        }
        Ok(Self { ports, members })
    }
}

impl<P> ProvidedBackend for Providing<P>
where
    P: Attached + Encoded + Clock + SignalWriter + EventSink + Handler,
{
    type Ports = P;
    fn ports(&mut self) -> &mut P {
        &mut self.ports
    }
    fn members(&self) -> &MemberTable {
        &self.members
    }
}
