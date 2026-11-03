//! The test catalog and its provider: one interface with a signal, an event,
//! a command, a query, a query whose reply is a stream and a `fixed`, served
//! by a `ridl-loopback` runtime.

use std::time::Duration;

use ridl_loopback::Loopback;
use ridl_rt::contract::{CatalogHash, CatalogRef, InterfaceNo, Kind, Ordinal, CALL_CEILING};
use ridl_rt::error::{CallError, Contract, Transport};
use ridl_rt::port::{
    ClaimId, EventSink, Handler, PushError, RaiseError, SignalWriter, StreamHandler, WriteError,
    REPLY_STREAM,
};
use ridl_transport_ws::MemberTable;

/// The test catalog: the unit `test.ws` with a hash of thirty-two 7s.
pub const CATALOG: CatalogRef = CatalogRef {
    name: "test.ws",
    hash: CatalogHash([7; 32]),
};
/// The one interface of the catalog.
pub const IFACE: InterfaceNo = InterfaceNo(1);
/// The signal, which the provider sets to each accepted command's argument.
pub const SIGNAL: Ordinal = Ordinal(1);
/// The event, which the provider raises with each accepted command's argument.
pub const EVENT: Ordinal = Ordinal(2);
/// The command.
pub const COMMAND: Ordinal = Ordinal(3);
/// The query, whose reply adds 1 to each argument byte, wrapping at 256.
pub const QUERY: Ordinal = Ordinal(4);
/// The `fixed`.
pub const FIXED: Ordinal = Ordinal(5);
/// The value the provider provisions for [`FIXED`].
pub const FIXED_VALUE: &[u8] = &[0xf0, 0x0d];
/// A `fixed` of 65,280 bytes, the largest a `fixed` may be, which crosses the
/// binding in 16 blocks. Its value is `pattern(FIXED_LARGE_LEN)`.
pub const FIXED_LARGE: Ordinal = Ordinal(7);
/// The length of the value of [`FIXED_LARGE`].
pub const FIXED_LARGE_LEN: usize = 65_280;
/// A `fixed` of 16,137 bytes, the longest that one `publish` carries within
/// the message limit of 16,164 bytes. Its value is `pattern(FIXED_WHOLE_LEN)`.
pub const FIXED_WHOLE: Ordinal = Ordinal(8);
/// The length of the value of [`FIXED_WHOLE`].
pub const FIXED_WHOLE_LEN: usize = 16_137;
/// A `fixed` of 16,138 bytes, one byte more than one `publish` carries, which
/// crosses the binding in 4 blocks. Its value is `pattern(FIXED_SPLIT_LEN)`.
pub const FIXED_SPLIT: Ordinal = Ordinal(9);
/// The length of the value of [`FIXED_SPLIT`].
pub const FIXED_SPLIT_LEN: usize = 16_138;
/// A query whose reply is a stream of named elements. Its argument is one
/// byte `n`, and an empty argument is read as 0. The reply is `n` elements,
/// where element `i`, from 0, is the [`ELEMENT_LEN`] bytes `[i; 1024]`, then
/// the end of the stream. An argument of [`STREAM_INTERRUPT`] is answered
/// with 3 elements, and then the provider interrupts the stream.
pub const STREAM_QUERY: Ordinal = Ordinal(10);
/// The length of each element of the reply of [`STREAM_QUERY`].
pub const ELEMENT_LEN: usize = 1024;
/// The argument of [`STREAM_QUERY`] whose reply stream the provider
/// interrupts after 3 elements.
pub const STREAM_INTERRUPT: u8 = 0xff;

/// The `n` bytes whose byte `i` is `i mod 251`, so that a block placed at a
/// wrong offset changes the bytes.
pub fn pattern(n: usize) -> Vec<u8> {
    (0..n).map(|i| (i % 251) as u8).collect()
}

/// The member table of [`CATALOG`]: the signal, the event, the command, the
/// query, four `fixed` members and the query whose reply is a stream, the
/// command and the two queries with a bound of 1 s. Ordinal 6 is reserved
/// and has no entry.
pub fn member_table() -> MemberTable {
    let mut table = MemberTable::new(CATALOG.name, CATALOG.hash);
    let bound = Some(Duration::from_secs(1));
    table.insert(IFACE, SIGNAL, Kind::Signal, None);
    table.insert(IFACE, EVENT, Kind::Event, None);
    table.insert(IFACE, COMMAND, Kind::Command, bound);
    table.insert(IFACE, QUERY, Kind::Query, bound);
    table.insert(IFACE, FIXED, Kind::Fixed, None);
    table.insert(IFACE, FIXED_LARGE, Kind::Fixed, None);
    table.insert(IFACE, FIXED_WHOLE, Kind::Fixed, None);
    table.insert(IFACE, FIXED_SPLIT, Kind::Fixed, None);
    table.insert(IFACE, STREAM_QUERY, Kind::Query, bound);
    table.set_reply_stream(IFACE, STREAM_QUERY);
    table
}

/// The provider's runtime: a loopback on [`CATALOG`] that serves the command
/// and the two queries, with [`FIXED`] provisioned to [`FIXED_VALUE`] and the three
/// large `fixed` members provisioned to their patterns.
pub fn provider() -> Loopback {
    let mut rt = Loopback::new(CATALOG);
    rt.serve(IFACE, &[COMMAND, QUERY, STREAM_QUERY])
        .expect("the loopback serves");
    rt.provision_fixed(IFACE, FIXED, FIXED_VALUE)
        .expect("the loopback stores the fixed");
    for (ord, len) in [
        (FIXED_LARGE, FIXED_LARGE_LEN),
        (FIXED_WHOLE, FIXED_WHOLE_LEN),
        (FIXED_SPLIT, FIXED_SPLIT_LEN),
    ] {
        rt.provision_fixed(IFACE, ord, &pattern(len))
            .expect("the loopback stores the large fixed");
    }
    rt
}

/// The reply streams of [`STREAM_QUERY`] the provider has not finished, kept
/// from one [`provider_step`] to the next because a push answers 0 when the
/// stream's window is full, and the failures its pushes read.
#[derive(Default)]
pub struct Streams {
    open: Vec<Open>,
    /// Each `PushError::Failed` a push or an end of a reply stream answered,
    /// in the order the provider read them.
    pub failures: Vec<CallError>,
}

/// One reply stream in progress.
struct Open {
    claim: ClaimId,
    /// The number of elements to push.
    count: u8,
    /// The element pushed next.
    next: u8,
    /// Whether the stream is interrupted after its elements, not ended.
    interrupt: bool,
}

impl Streams {
    /// Pushes the next elements of every open stream until its window is
    /// full, then ends or interrupts each stream whose elements are all
    /// pushed. A stream whose push or end answers `PushError::Failed` is
    /// recorded in [`Streams::failures`] and dropped.
    fn advance(&mut self, rt: &mut Loopback) {
        let failures = &mut self.failures;
        self.open.retain_mut(|open| {
            while open.next < open.count {
                let element = [open.next; ELEMENT_LEN];
                match rt.push_claim(open.claim, REPLY_STREAM, &element) {
                    Ok(0) => return true,
                    Ok(_) => open.next += 1,
                    Err(PushError::Failed(e)) => {
                        failures.push(e);
                        // The claim is still held: an end settles it.
                        let _ = rt.end_claim(open.claim, REPLY_STREAM);
                        return false;
                    }
                    Err(e) => panic!("pushed: {e:?}"),
                }
            }
            let done = if open.interrupt {
                let interrupted = CallError::Transport(Transport::Interrupted);
                rt.abort_claim(open.claim, REPLY_STREAM, interrupted)
            } else {
                rt.end_claim(open.claim, REPLY_STREAM)
            };
            match done {
                Ok(()) => {}
                Err(PushError::Failed(e)) => failures.push(e),
                Err(e) => panic!("ended: {e:?}"),
            }
            false
        });
    }
}

/// Makes one pass over the claims waiting on `rt` and settles each, then
/// moves every reply stream in `streams` forward.
///
/// - [`COMMAND`]: settles `Ok(&[])`, then sets [`SIGNAL`] to the argument,
///   commits, and raises [`EVENT`] with the argument. An argument longer
///   than the limit of the signal or the event, 3,840 bytes, is refused by
///   the loopback on send, and that member is left as it is.
/// - [`QUERY`]: settles with a reply whose byte `i` is argument byte `i` plus
///   1, wrapping at 256.
/// - [`STREAM_QUERY`]: opens the reply stream its argument names, which
///   [`Streams`] pushes over this step and the next ones.
pub fn provider_step(rt: &mut Loopback, streams: &mut Streams) {
    // The arguments of a command or a query are at most the default ceiling
    // of their kind, so every claim fits.
    let mut args = vec![0u8; CALL_CEILING];
    while let Some(claim) = rt.next_claim(&mut args).expect("a claim fits") {
        let arg = &args[..claim.len];
        match claim.ord {
            COMMAND => {
                rt.settle(claim.id, Ok(&[])).expect("settled");
                match rt.set(IFACE, SIGNAL, arg) {
                    Ok(()) | Err(WriteError::TooLarge { .. }) => rt.commit(),
                    Err(e) => panic!("staged: {e:?}"),
                }
                match rt.raise(IFACE, EVENT, arg, None) {
                    Ok(()) | Err(RaiseError::TooLarge { .. }) => {}
                    Err(e) => panic!("raised: {e:?}"),
                }
            }
            QUERY => {
                let reply: Vec<u8> = arg.iter().map(|b| b.wrapping_add(1)).collect();
                rt.settle(claim.id, Ok(&reply)).expect("settled");
            }
            STREAM_QUERY => {
                let n = arg.first().copied().unwrap_or(0);
                let interrupt = n == STREAM_INTERRUPT;
                streams.open.push(Open {
                    claim: claim.id,
                    count: if interrupt { 3 } else { n },
                    next: 0,
                    interrupt,
                });
            }
            _ => {
                let unknown = Err(CallError::Contract(Contract::UnknownInteraction));
                rt.settle(claim.id, unknown).expect("settled");
            }
        }
    }
    streams.advance(rt);
}
