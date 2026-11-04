//! The test catalog and its provider: one interface with a signal, an event,
//! commands, queries, upload and reply streams, and fixed values, served
//! by a `ridl-loopback` runtime.

use std::collections::BTreeMap;
use std::time::Duration;

use ridl_loopback::Loopback;
use ridl_rt::contract::{CatalogHash, CatalogRef, InterfaceNo, Kind, Ordinal, CALL_CEILING};
use ridl_rt::error::{CallError, Contract, Transport};
use ridl_rt::payload::{Rule, Violation};
use ridl_rt::port::{
    ClaimId, EventSink, Handler, Pull, PushError, RaiseError, SignalWriter, StreamHandler,
    StreamNo, WriteError, REPLY_STREAM,
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

/// A command that stores stream 1 under the argument's key.
pub const UPLOAD: Ordinal = Ordinal(11);
/// A query that replies with the upload's byte count and wrapping sum.
pub const UPLOAD_QUERY: Ordinal = Ordinal(12);
/// A query that streams the stored bytes for its key.
pub const FETCH: Ordinal = Ordinal(13);
/// A query that echoes stream 1 on stream 0.
pub const DUPLEX: Ordinal = Ordinal(14);
/// Refuses the third chunk with the range rule.
pub const MODE_REFUSE_RANGE: u8 = 1;
/// Refuses the third chunk with the UTF-8 rule.
pub const MODE_REFUSE_UTF8: u8 = 2;
/// The largest chunk of a fetched value.
pub const FETCH_CHUNK: usize = 16_128;

/// The `n` bytes whose byte `i` is `i mod 251`, so that a block placed at a
/// wrong offset changes the bytes.
pub fn pattern(n: usize) -> Vec<u8> {
    (0..n).map(|i| (i % 251) as u8).collect()
}

/// The member table of [`CATALOG`]: the signal, the event, the command, the
/// queries, four `fixed` members, uploads, fetch and duplex echo. All commands
/// and queries have a bound of 1 s. Ordinal 6 is reserved
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
    for (ord, kind) in [
        (UPLOAD, Kind::Command),
        (UPLOAD_QUERY, Kind::Query),
        (FETCH, Kind::Query),
        (DUPLEX, Kind::Query),
    ] {
        table.insert(IFACE, ord, kind, bound);
    }
    for ord in [UPLOAD, UPLOAD_QUERY, DUPLEX] {
        table.set_parameter_streams(IFACE, ord, 1);
    }
    for ord in [FETCH, DUPLEX] {
        table.set_reply_stream(IFACE, ord);
    }
    table
}

/// The provider's runtime: a loopback on [`CATALOG`] that serves the command
/// and the queries, including uploads, fetch and duplex echo, with [`FIXED`] provisioned to [`FIXED_VALUE`] and the three
/// large `fixed` members provisioned to their patterns.
pub fn provider() -> Loopback {
    let mut rt = Loopback::new(CATALOG);
    rt.serve(
        IFACE,
        &[
            COMMAND,
            QUERY,
            STREAM_QUERY,
            UPLOAD,
            UPLOAD_QUERY,
            FETCH,
            DUPLEX,
        ],
    )
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

/// The provider's open uploads and reply streams, stored values, and failures
/// observed by [`STREAM_QUERY`]. State persists between [`provider_step`] calls
/// when a stream is pending or its window is full.
#[derive(Default)]
pub struct Streams {
    open: Vec<Open>,
    uploads: Vec<OpenUpload>,
    fetches: Vec<OpenFetch>,
    store: BTreeMap<u8, Vec<u8>>,
    /// Each `PushError::Failed` a push or an end of [`STREAM_QUERY`] answered,
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

struct OpenUpload {
    claim: ClaimId,
    ord: Ordinal,
    key: u8,
    mode: u8,
    chunks: usize,
    count: u64,
    sum: u32,
    held: Option<Vec<u8>>,
    first: Option<Pull>,
}

struct OpenFetch {
    claim: ClaimId,
    bytes: Vec<u8>,
    offset: usize,
}

impl Streams {
    fn advance_uploads(&mut self, rt: &mut Loopback) {
        let store = &mut self.store;
        self.uploads.retain_mut(|upload| {
            let mut buf = vec![0; FETCH_CHUNK];
            loop {
                if let Some(bytes) = &upload.held {
                    match rt.push_claim(upload.claim, REPLY_STREAM, bytes) {
                        Ok(0) if !bytes.is_empty() => return true,
                        Ok(_) => upload.held = None,
                        Err(_) => {
                            let _ = rt.abort_claim(
                                upload.claim,
                                StreamNo(1),
                                CallError::Transport(Transport::Interrupted),
                            );
                            let _ = rt.end_claim(upload.claim, REPLY_STREAM);
                            return false;
                        }
                    }
                }
                match upload
                    .first
                    .take()
                    .map(Ok)
                    .unwrap_or_else(|| rt.pull_claim(upload.claim, StreamNo(1), &mut buf))
                {
                    Ok(Pull::Pending) => return true,
                    Ok(Pull::Chunk(n)) => {
                        upload.chunks += 1;
                        if upload.chunks == 3
                            && matches!(upload.mode, MODE_REFUSE_RANGE | MODE_REFUSE_UTF8)
                        {
                            let error = CallError::Contract(Contract::InvalidValue(Violation {
                                type_name: Some("Block"),
                                rule: if upload.mode == MODE_REFUSE_RANGE {
                                    Rule::Range
                                } else {
                                    Rule::Utf8
                                },
                            }));
                            rt.abort_claim(upload.claim, StreamNo(1), error).unwrap();
                            if upload.ord == UPLOAD_QUERY {
                                rt.settle(upload.claim, Err(error)).unwrap();
                            }
                            return false;
                        }
                        if upload.ord == UPLOAD {
                            store
                                .entry(upload.key)
                                .or_default()
                                .extend_from_slice(&buf[..n]);
                        } else if upload.ord == DUPLEX {
                            upload.held = Some(buf[..n].to_vec());
                        } else {
                            upload.count += n as u64;
                            for byte in &buf[..n] {
                                upload.sum = upload.sum.wrapping_add(u32::from(*byte));
                            }
                        }
                    }
                    Ok(Pull::End) => {
                        if upload.ord == DUPLEX {
                            let _ = rt.end_claim(upload.claim, REPLY_STREAM);
                        } else if upload.ord == UPLOAD_QUERY {
                            let mut reply = [0; 12];
                            reply[..8].copy_from_slice(&upload.count.to_le_bytes());
                            reply[8..].copy_from_slice(&upload.sum.to_le_bytes());
                            rt.settle(upload.claim, Ok(&reply)).unwrap();
                        }
                        return false;
                    }
                    Ok(Pull::Failed(e)) => {
                        if upload.ord == DUPLEX {
                            let _ = rt.abort_claim(upload.claim, REPLY_STREAM, e);
                        } else if upload.ord == UPLOAD_QUERY {
                            let _ = rt.settle(upload.claim, Err(e));
                        }
                        return false;
                    }
                    Err(e) => panic!("pulled upload: {e:?}"),
                }
            }
        });
    }

    /// Advances uploads and fetches, then pushes the next elements of each
    /// open [`STREAM_QUERY`] until its window is full. It ends or interrupts
    /// completed streams and records their failures in [`Streams::failures`].
    fn advance(&mut self, rt: &mut Loopback) {
        self.advance_uploads(rt);
        self.fetches.retain_mut(|fetch| {
            while fetch.offset < fetch.bytes.len() {
                let end = (fetch.offset + FETCH_CHUNK).min(fetch.bytes.len());
                match rt.push_claim(fetch.claim, REPLY_STREAM, &fetch.bytes[fetch.offset..end]) {
                    Ok(0) => return true,
                    Ok(_) => fetch.offset = end,
                    Err(_) => {
                        let _ = rt.end_claim(fetch.claim, REPLY_STREAM);
                        return false;
                    }
                }
            }
            let _ = rt.end_claim(fetch.claim, REPLY_STREAM);
            false
        });
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
/// - [`UPLOAD`]: acknowledges, replaces the key's stored value, then stores chunks.
/// - [`UPLOAD_QUERY`]: replies with the byte count and wrapping sum after upload end.
///   Both uploads refuse the third chunk in either refusing mode.
/// - [`FETCH`]: streams a snapshot of the key's stored bytes.
/// - [`DUPLEX`]: echoes each upload chunk, retaining it while the reply window is full.
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
            UPLOAD | UPLOAD_QUERY | DUPLEX => {
                // Name stream 1 before acknowledging a command so the runtime retains it.
                // An empty buffer leaves a queued nonempty chunk untouched.
                let first = match rt.pull_claim(claim.id, StreamNo(1), &mut []) {
                    Ok(Pull::Pending) | Err(ridl_rt::port::ReadError::Short { .. }) => None,
                    Ok(pull) => Some(pull),
                    Err(e) => panic!("opened upload: {e:?}"),
                };
                let (key, mode) = if claim.ord == UPLOAD {
                    (
                        arg.first().copied().unwrap_or(0),
                        arg.get(1).copied().unwrap_or(0),
                    )
                } else {
                    (0, arg.first().copied().unwrap_or(0))
                };
                if claim.ord == UPLOAD {
                    streams.store.insert(key, Vec::new());
                    rt.settle(claim.id, Ok(&[])).unwrap();
                }
                streams.uploads.push(OpenUpload {
                    claim: claim.id,
                    ord: claim.ord,
                    key,
                    mode,
                    chunks: 0,
                    count: 0,
                    sum: 0,
                    held: None,
                    first,
                });
            }
            FETCH => {
                // Consume earlier uploads before taking the stored value's snapshot.
                streams.advance_uploads(rt);
                let key = arg.first().copied().unwrap_or(0);
                streams.fetches.push(OpenFetch {
                    claim: claim.id,
                    bytes: streams.store.get(&key).cloned().unwrap_or_default(),
                    offset: 0,
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

#[cfg(test)]
mod upload_tests {
    use super::*;
    use ridl_rt::payload::{Rule, Violation};
    use ridl_rt::port::{Caller, Pull, StreamCaller, StreamNo};

    // Mutation: omit storing a chunk, or fail to replace a stored value.
    #[test]
    fn stored_uploads_are_fetched_byte_for_byte_and_replaced() {
        let mut p = provider();
        let mut c = p.attach();
        let mut state = Streams::default();
        for bytes in [pattern(200_000), vec![8, 7], vec![]] {
            let call = c.command(IFACE, UPLOAD, &[1, 0], None).unwrap();
            if bytes.is_empty() {
                c.push(call, StreamNo(1), &[]).unwrap();
                c.end(call, StreamNo(1)).unwrap();
            }
            provider_step(&mut p, &mut state);
            assert_eq!(c.ack(call), Some(Ok(())));
            for (i, chunk) in bytes.chunks(FETCH_CHUNK).enumerate() {
                assert_eq!(c.push(call, StreamNo(1), chunk), Ok(chunk.len()));
                if (i + 1) * FETCH_CHUNK < bytes.len() {
                    provider_step(&mut p, &mut state);
                }
            }
            if !bytes.is_empty() {
                assert_eq!(c.end(call, StreamNo(1)), Ok(()));
            }
            let fetch = c.query(IFACE, FETCH, &[1], None).unwrap();
            let mut read = Vec::new();
            let mut buf = vec![0; FETCH_CHUNK];
            for _ in 0..100 {
                provider_step(&mut p, &mut state);
                match c.pull(fetch, REPLY_STREAM, &mut buf).unwrap() {
                    Pull::Chunk(n) => read.extend_from_slice(&buf[..n]),
                    Pull::End => break,
                    Pull::Pending => continue,
                    other => panic!("unexpected fetch: {other:?}"),
                }
            }
            assert_eq!(read, bytes);
            assert_eq!(c.pull(fetch, REPLY_STREAM, &mut buf), Ok(Pull::End));
            c.forget(fetch);
            c.forget(call);
        }
    }

    // Mutation: settle before upload end, corrupt the count, or omit stream abort.
    #[test]
    fn upload_queries_count_sum_and_refuse_both_rules() {
        for mode in [0, MODE_REFUSE_RANGE, MODE_REFUSE_UTF8] {
            let mut p = provider();
            let mut c = p.attach();
            let mut state = Streams::default();
            let call = c.query(IFACE, UPLOAD_QUERY, &[mode], None).unwrap();
            provider_step(&mut p, &mut state);
            let mut reply = [0; 12];
            assert_eq!(c.reply(call, &mut reply), Ok(None));
            for _ in 0..3 {
                assert_eq!(c.push(call, StreamNo(1), &[255, 2]), Ok(2));
                provider_step(&mut p, &mut state);
            }
            if mode == 0 {
                assert_eq!(c.reply(call, &mut reply), Ok(None));
                c.end(call, StreamNo(1)).unwrap();
                provider_step(&mut p, &mut state);
                assert_eq!(c.reply(call, &mut reply), Ok(Some(Ok(12))));
                assert_eq!(&reply[..8], &6u64.to_le_bytes());
                assert_eq!(&reply[8..], &771u32.to_le_bytes());
            } else {
                let error = CallError::Contract(Contract::InvalidValue(Violation {
                    type_name: Some("Block"),
                    rule: if mode == MODE_REFUSE_RANGE {
                        Rule::Range
                    } else {
                        Rule::Utf8
                    },
                }));
                assert_eq!(
                    c.push(call, StreamNo(1), &[1]),
                    Err(PushError::Failed(error))
                );
                assert_eq!(c.reply(call, &mut reply), Ok(Some(Err(error))));
            }
        }
    }

    // Mutation: discard an echo when the reply window is full.
    #[test]
    fn duplex_echo_preserves_chunks_under_backpressure() {
        let mut p = provider();
        let mut c = p.attach();
        let mut state = Streams::default();
        let call = c.query(IFACE, DUPLEX, &[], None).unwrap();
        provider_step(&mut p, &mut state);
        for n in 1..=8 {
            assert_eq!(c.push(call, StreamNo(1), &[n]), Ok(1));
            provider_step(&mut p, &mut state);
        }
        c.end(call, StreamNo(1)).unwrap();
        let mut read = Vec::new();
        for _ in 0..30 {
            provider_step(&mut p, &mut state);
            let mut buf = [0; 8];
            match c.pull(call, REPLY_STREAM, &mut buf).unwrap() {
                Pull::Chunk(n) => read.extend_from_slice(&buf[..n]),
                Pull::End => break,
                Pull::Pending => continue,
                other => panic!("unexpected echo: {other:?}"),
            }
        }
        assert_eq!(read, [1, 2, 3, 4, 5, 6, 7, 8]);
        assert_eq!(c.pull(call, REPLY_STREAM, &mut []), Ok(Pull::End));
    }
}
