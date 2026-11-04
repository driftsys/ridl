//! The local ports into which a remote provider delivers.

use ridl_rt::port::{Attached, Clock, Encoded, EventSink, Handler, SignalWriter};

use super::{Config, MemberTable, Note, TableMismatch};
use crate::frame::Target;
use ridl_rt::sample::{Cause, Occurrence, Provenance, Sample, Timestamp};

/// The local destination of one remote provider's session.
///
/// The table contains only members offered by this instance in this catalog.
/// The embedder filters it before construction. The backend is dropped on
/// every session exit, including selection and handshake failures.
pub trait ProvidedBackend {
    /// The local providing ports.
    type Ports: Attached + Encoded + Clock + Handler;
    /// The ports.
    fn ports(&mut self) -> &mut Self::Ports;
    /// Only the members this instance offers.
    fn members(&self) -> &MemberTable;
    /// Delivers an accepted signal sample. `None` means the catalog init value;
    /// `Some` is the last good encoded value, including on invalid samples.
    /// The generated binding verifies live bytes before application use.
    fn sample(&mut self, target: Target, sample: Sample<Option<&[u8]>>, config: &Config);
    /// Delivers an occurrence with its original envelope. `gap` counts lost
    /// occurrences since the last accepted sequence on this channel.
    fn occurrence(
        &mut self,
        target: Target,
        occurrence: Occurrence<&[u8]>,
        gap: Option<u64>,
        config: &Config,
    );
    /// Ends a group of consecutive accepted deliveries with one remote stamp.
    /// Called on stamp change or after one idle poll interval; never on drop.
    fn commit(&mut self, remote_stamp: Timestamp);
}

/// Providing ports paired with their offered member table.
///
/// This debug adapter loses information the local ports cannot carry. `Init`
/// writes nothing, leaving the machine's last value in place. Live samples use
/// `set`; declared invalid samples use `invalidate`. Detected invalid samples
/// also use `invalidate` (the machine sees `Declared`) and record
/// [`Note::DetectedInvalid`]. A corrupt occurrence is not raised and records
/// [`Note::CorruptOccurrence`]. Every sequence gap records [`Note::EventGap`].
/// Occurrences that can be raised receive a new local envelope and sequence.
/// Commits use the local port clock, never the remote stamp. Staged signal
/// writes are discarded when the session drops its ports before a group ends.
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
    fn sample(&mut self, target: Target, sample: Sample<Option<&[u8]>>, config: &Config) {
        let result = match sample.provenance {
            Provenance::Init => return,
            Provenance::Live => self.ports.set(
                target.iface,
                target.ord,
                sample.value.expect("live sample has bytes"),
            ),
            Provenance::Invalid(cause) => {
                if let Cause::Detected(cause) = cause {
                    record(config, Note::DetectedInvalid { target, cause });
                }
                self.ports.invalidate(target.iface, target.ord)
            }
        };
        if result.is_err() {
            record(config, Note::PortRefused { target });
        }
    }
    fn occurrence(
        &mut self,
        target: Target,
        occurrence: Occurrence<&[u8]>,
        gap: Option<u64>,
        config: &Config,
    ) {
        if let Some(count) = gap {
            record(config, Note::EventGap { target, count });
        }
        match occurrence.payload {
            Ok(bytes) => {
                if self
                    .ports
                    .raise(target.iface, target.ord, bytes, None)
                    .is_err()
                {
                    record(config, Note::PortRefused { target });
                }
            }
            Err(cause) => record(config, Note::CorruptOccurrence { target, cause }),
        }
    }
    fn commit(&mut self, _: Timestamp) {
        self.ports.commit();
    }
}

fn record(config: &Config, note: Note) {
    if let Some(record) = config.record() {
        record(&note);
    }
}
