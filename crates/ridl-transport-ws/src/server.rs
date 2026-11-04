//! The server: the backend trait, the configuration, the session core that
//! turns decoded frames and backend samples into frames to send, and
//! [`serve`], the blocking loop that runs a session over one connection.

mod backend;
mod calls;
mod config;
#[cfg(feature = "descriptor")]
mod descriptor;
mod members;
mod provided;
mod provided_backend;
mod session;

use std::io::{self, Read, Write};
use std::net::{Shutdown, TcpStream};
use std::time::{Duration, Instant};

use ridl_rt::contract::{default_ceiling, CatalogHash, InterfaceNo, Kind, Ordinal};
use ridl_rt::sample::{Envelope, Timestamp};

pub use backend::{Backend, Described, Polling, Scanning, Streaming, TableMismatch};
pub use config::{Config, LimitOutOfRange, Note, RetainedTooSmall};
#[cfg(feature = "descriptor")]
pub use descriptor::DescriptorError;
pub use members::MemberTable;
pub use provided::{serve_provided, serve_provided_selected, SelectionError};
pub use provided_backend::{ProvidedBackend, Providing};

use crate::frame::{
    AttachOutcome, Frame, Reason, ResponseOutcome, Target, BLOCK_BYTES, ENVELOPE_BYTES,
    FRAME_VERSION, FRAME_VERSION_3,
};
use crate::ws::{self, CloseCode, Event, FrameWriter, Parser, Upgrade};
use session::{Session, Step};

/// A connected stream that [`serve`] runs one session over.
///
/// The embedder accepts the connection and applies its access policy before
/// it calls [`serve`]; the crate binds, listens and connects nothing. The
/// stream must be in blocking mode: [`serve`] waits in each read for up to
/// the timeout it set with [`Connection::set_read_timeout`], and takes the
/// `WouldBlock` a non-blocking stream fails with at once as that timeout
/// (see [`serve`] for what follows).
///
/// [`serve`] writes each WebSocket frame with one write, so a frame does not
/// wait on Nagle's algorithm between its header and its payload. The crate
/// sets no socket option: an embedder that wants each frame sent at once,
/// without Nagle's algorithm holding a small frame back, may set
/// `TCP_NODELAY` on the stream (`TcpStream::set_nodelay`) before it calls
/// [`serve`].
pub trait Connection: Read + Write {
    /// Sets how long one read waits for bytes before it fails with
    /// `WouldBlock` or `TimedOut`. `None` waits without limit.
    ///
    /// # Errors
    ///
    /// The error of the underlying stream, such as `InvalidInput` for a zero
    /// duration.
    fn set_read_timeout(&self, t: Option<Duration>) -> io::Result<()>;

    /// Sets how long one write waits for room in the stream before it fails
    /// with `WouldBlock` or `TimedOut`. `None` waits without limit.
    /// [`serve`] sets it to [`Config::liveness_timeout`], so that a client
    /// that stops reading cannot hold the thread. The timeout bounds each
    /// write call, not a whole message: a message written in several calls
    /// can take longer, as long as each call makes progress.
    ///
    /// # Errors
    ///
    /// The error of the underlying stream, such as `InvalidInput` for a zero
    /// duration.
    fn set_write_timeout(&self, t: Option<Duration>) -> io::Result<()>;

    /// Ends the write side of the stream, so that the peer reads the end of
    /// the stream after the bytes already written. [`serve`] calls it after
    /// every close frame it sends and after a handshake `400 Bad Request` or
    /// `403 Forbidden`.
    /// The default does nothing, for a stream that has no write side of its
    /// own to end.
    ///
    /// # Errors
    ///
    /// The error of the underlying stream. [`serve`] ignores it.
    fn shutdown_write(&self) -> io::Result<()> {
        Ok(())
    }
}

impl Connection for TcpStream {
    fn set_read_timeout(&self, t: Option<Duration>) -> io::Result<()> {
        TcpStream::set_read_timeout(self, t)
    }

    fn set_write_timeout(&self, t: Option<Duration>) -> io::Result<()> {
        TcpStream::set_write_timeout(self, t)
    }

    fn shutdown_write(&self) -> io::Result<()> {
        self.shutdown(Shutdown::Write)
    }
}

#[cfg(unix)]
impl Connection for std::os::unix::net::UnixStream {
    fn set_read_timeout(&self, t: Option<Duration>) -> io::Result<()> {
        std::os::unix::net::UnixStream::set_read_timeout(self, t)
    }

    fn set_write_timeout(&self, t: Option<Duration>) -> io::Result<()> {
        std::os::unix::net::UnixStream::set_write_timeout(self, t)
    }

    fn shutdown_write(&self) -> io::Result<()> {
        self.shutdown(Shutdown::Write)
    }
}

/// Why [`serve`] returned.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ended {
    /// The client sent a close frame, with its status code when the frame
    /// carried one. The server answered with a close frame that carries the
    /// same code, or 1000 when the client's carried none.
    ClientClosed(Option<u16>),
    /// The server sent a close frame with this code (frame specification
    /// §11.1 item 4): 1000 after `attached(refused)`, 1001 when the backend
    /// detached, 1002 on a protocol violation, 1003 on a text message, 1009
    /// on a message over [`Config::max_message_bytes`], 1008 when the
    /// responses the session keeps for a repeated `request` pass
    /// [`Config::max_retained_bytes`]. A violation of the frame is also
    /// recorded as a [`Note`].
    ServerClosed(u16),
    /// The opening handshake was answered `400 Bad Request`, because the
    /// request was malformed, its head passed 8 KiB, it lacked a header the
    /// upgrade needs, its `Sec-WebSocket-Version` was not 13, or it offered
    /// no subprotocol this server serves (`ridl-ws-v3` counts for [`serve`]
    /// only when the backend serves streams and holds a member table, and for
    /// [`serve_selected`] only when the caller's `max_version` is 3);
    /// or `403 Forbidden`,
    /// because the check set by [`Config::with_origin_check`] refused its
    /// `Origin`; or 404/409 because a providing backend selector refused
    /// the path.
    Rejected,
    /// The opening handshake did not finish within
    /// [`Config::liveness_timeout`], no message of any kind, data or
    /// control, arrived for that long after it, or a write waited that long
    /// for the client to read. No close frame is sent.
    LivenessTimeout,
    /// A read or a write of the connection failed with this kind of error.
    /// `UnexpectedEof` is a client that closed the connection without a
    /// close frame.
    Io(io::ErrorKind),
    /// [`Config::max_message_bytes`] is below what one message of a member
    /// of the catalog needs (frame specification §11.1 "The sizes"), 36
    /// bytes of header and envelope plus the member's limit, and for a
    /// `fixed` the smaller of that and one block message, 4,130 bytes: the
    /// configuration cannot carry a payload the catalog allows, which is a
    /// configuration error, not a session event. [`serve`] returned before
    /// the opening handshake, without reading the connection, and recorded
    /// [`Note::MessageLimitTooSmall`]; the connection closes when it is
    /// dropped. Without a member table there is no check.
    ///
    /// With [`serve_selected`], this also covers an unsupported advertised
    /// capability (before reading), or a selected backend lacking the
    /// negotiated capability or message capacity. Selected-backend refusals
    /// answer `attach` with `attached(refused)` and close 1000 before any
    /// session data: `frame_version_unsupported` for capability and `range`
    /// for message capacity. Capability takes precedence if both checks fail.
    /// When ordinary attach checks pass, insufficient message capacity records
    /// the same note with either reply.
    /// for message capacity. A capacity refusal records the same note.
    ///
    /// With [`serve_provided_selected`], capacity is checked after selection
    /// and before upgrade; a refusal answers HTTP 400 and records the note.
    /// An unencodable catalog name or an encoding without a frame tag also
    /// answers HTTP 400 and returns this variant; no matching `Note` exists.
    ConfigRefused,
}

/// The size of the buffer each socket read fills.
const READ_BUFFER: usize = 16 * 1024;

/// The shortest read timeout the loop sets when the next poll is due at
/// once: a zero timeout is refused by the stream.
const MIN_READ_TIMEOUT: Duration = Duration::from_millis(1);

/// How long the server goes on reading after it answers a handshake `400`
/// or `403`, or sends a close frame, the answer to the client's included, so
/// that the client reads the answer or the close frame before the connection
/// closes.
const LINGER: Duration = Duration::from_millis(100);

/// Serves `backend` to the WebSocket client on `conn`, and returns when the
/// session ends.
///
/// `serve` blocks the calling thread for the whole session. It answers the
/// opening handshake, then loops: it reads `conn` with a read timeout that
/// ends when the next poll is due, at most [`Config::poll_interval`] after
/// the last one, handles each whole message read, and, when a poll interval
/// has passed since the last poll, polls the backend once and sends what
/// changed; it sends a ping each [`Config::ping_interval`]. A message is
/// handled as soon as it is read, and the backend is polled once per poll
/// interval, whatever the client's message rate: a client that sends often
/// does not multiply the polls. The messages that arrive in one socket read
/// are all answered before the next poll, so a client that sends many
/// `read`s of a large `fixed` delays its own session's polls for as long as
/// their blocks take to write. This delay is accepted because it affects
/// only the session of the client that sent the reads. A change on the
/// backend is sent at most one poll interval after it, plus the write. Each
/// write call has a write timeout of [`Config::liveness_timeout`]: a client
/// that reads nothing for that long, so that a write call makes no progress,
/// ends the session with [`Ended::LivenessTimeout`].
///
/// The liveness clock measures time since the client's last message. Time
/// spent writing the frames of one message's answer or of one poll is not
/// counted, because the server cannot read while it writes. A client that
/// stops reading is ended by the write timeout described above. The time a
/// client takes to read bytes the kernel has already accepted is still
/// counted.
///
/// Each `read` of a `fixed` sends the whole value again, in blocks when it
/// does not fit one message. The frames the session writes after a message
/// hold at most the blocks of one `read`, because `serve` writes them after
/// each message it handles.
///
/// `conn` must be in blocking mode: `serve` waits in each read for up to the
/// read timeout it set. On a stream in non-blocking mode a read with no bytes
/// waiting fails at once with `WouldBlock`, which `serve` takes as a read
/// timeout: during the handshake that ends the session with
/// [`Ended::LivenessTimeout`] at the first such read, and after the handshake
/// the loop spins without waiting, polling at each poll interval, until the
/// liveness timeout ends the session. On some platforms a stream accepted
/// from a non-blocking listener inherits that mode;
/// `TcpStream::set_nonblocking(false)` before `serve` sets it right.
///
/// At every end, each backend call in flight is forgotten and `backend` is
/// dropped, with its event subscriptions; then `conn` is dropped. After
/// every close frame it sends, in answer to the client's or on its own, the
/// server ends its write side and reads and drops what the client still
/// sends, until the client closes the connection or 100 ms pass: a
/// connection closed with unread bytes is reset, and the client could lose
/// the close frame. It waits no longer than that for the client to close
/// the connection (RFC 6455 §7.1.1 has the client wait for the server to
/// close it).
///
/// The session keeps state for each signal the client subscribes, and for
/// each signal a `read` found published, for as long as the session lasts:
/// the channel's session `seq` never restarts (frame specification §7). No
/// limit in the crate bounds the number of these signals, so per-session
/// memory, and in polling mode the work of each tick, grows with the
/// distinct signals the client names. The embedder's access policy decides
/// which clients may open a session; it must trust each one for that
/// growth. A `read` of a signal that was never published keeps no state.
///
/// With a member table ([`Backend::members`] returns `Some`, as [`Described`]
/// does), a `subscribe`, a `read` or a `request` naming an interaction the
/// table does not hold is refused with `contract(UnknownInteraction)` before
/// it reaches the backend, so the signal and event state of a session holds
/// at most one entry per signal or event member of the catalog. Without a
/// table, nothing bounds it, and the paragraph above applies.
///
/// With a member table, `serve` first checks that
/// [`Config::max_message_bytes`] carries one message of every member: 36
/// bytes of header and envelope plus the member's limit, which is the limit
/// the table gives it or the default ceiling of its kind. A `fixed` that
/// one `publish` does not carry crosses in blocks, so a `fixed` needs the
/// smaller of that and one block message, 4,130 bytes. A limit below that
/// is a configuration error: `serve` records [`Note::MessageLimitTooSmall`]
/// and returns [`Ended::ConfigRefused`] before the opening handshake,
/// without reading the connection. The default limit carries every catalog.
///
/// A zero [`Config::poll_interval`] or [`Config::liveness_timeout`] is
/// refused by the stream's `set_read_timeout` or `set_write_timeout`, and
/// `serve` returns [`Ended::Io`] with `InvalidInput`.
pub fn serve<C: Connection, B: Backend>(conn: C, mut backend: B, config: &Config) -> Ended {
    if let Some((target, needed)) = backend
        .members()
        .and_then(|table| limit_short(table, config.max_message_bytes()))
    {
        if let Some(record) = config.record() {
            record(&Note::MessageLimitTooSmall { target, needed });
        }
        return Ended::ConfigRefused;
    }
    // Frame version 3 carries streams, so the server offers it only when it
    // can serve them: the ports carry streams and a member table says which
    // members reply with one.
    let max_version = if backend.streams().is_some() && backend.members().is_some() {
        FRAME_VERSION_3
    } else {
        FRAME_VERSION
    };
    run_session(
        conn,
        Some(Session::new(backend, config)),
        max_version,
        |_| None,
        config,
    )
}

/// A client's catalog identity, borrowed only for backend selection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct RequestedCatalog<'a> {
    /// The catalog name in the client's `attach`.
    pub name: &'a str,
    /// The catalog hash in the client's `attach`.
    pub hash: CatalogHash,
}

/// Serves a backend chosen once from the client's first `attach`.
///
/// `max_version` must be 2 or 3; other values return
/// [`Ended::ConfigRefused`] without reading the connection. It states the
/// capability available before selection for subprotocol negotiation. Version
/// 1 is negotiated only to refuse attach, as in [`serve`].
///
/// After parsing `attach`, `select` receives its borrowed name and hash.
/// Prefer a matching name and hash when available; otherwise select by name
/// so the session can refuse an incompatible hash. `None` means an unknown
/// name and refuses with `unknown_catalog`, except that a wrong attach version
/// or a version 1 connection is refused with `frame_version_unsupported`.
/// A selected backend undergoes the
/// ordinary version, name, hash and encoding checks. Before accepting attach,
/// it must also support the negotiated capability and message capacity;
/// otherwise the server sends `attached(refused)` and close 1000 and returns
/// [`Ended::ConfigRefused`]. See that variant for the refusal reasons.
///
/// Origin checks precede selection. The blocking, liveness and cleanup rules
/// of [`serve`] apply, including dropping the selected backend on every exit.
pub fn serve_selected<C, B, S>(conn: C, max_version: u32, select: S, config: &Config) -> Ended
where
    C: Connection,
    B: Backend,
    S: for<'a> FnOnce(RequestedCatalog<'a>) -> Option<B>,
{
    if !matches!(max_version, FRAME_VERSION | FRAME_VERSION_3) {
        return Ended::ConfigRefused;
    }
    run_session(conn, None, max_version, select, config)
}

fn run_session<C, B, S>(
    mut conn: C,
    session: Option<Session<B>>,
    max_version: u32,
    select: S,
    config: &Config,
) -> Ended
where
    C: Connection,
    B: Backend,
    S: for<'a> FnOnce(RequestedCatalog<'a>) -> Option<B>,
{
    let mut selected = Selection {
        session,
        select: Some(select),
        frame_version: FRAME_VERSION,
        config_refused: false,
    };
    let ended = run(&mut conn, &mut selected, config, max_version);
    if let Some(session) = selected.session {
        session.close();
    }
    ended
}

struct Selection<B: Backend, S> {
    session: Option<Session<B>>,
    select: Option<S>,
    frame_version: u32,
    config_refused: bool,
}

impl<B, S> Selection<B, S>
where
    B: Backend,
    S: for<'a> FnOnce(RequestedCatalog<'a>) -> Option<B>,
{
    fn on_message(
        &mut self,
        msg: &[u8],
        now: Instant,
        out: &mut Vec<Vec<u8>>,
        config: &Config,
    ) -> Step {
        if let Some(session) = &mut self.session {
            return session.on_message(msg, now, out);
        }
        let attach = match session::first_attach(msg, self.frame_version, config) {
            Ok(attach) => attach,
            Err(step) => return step,
        };
        let backend = self.select.take().expect("selection runs once")(RequestedCatalog {
            name: attach.name(),
            hash: *attach.hash(),
        });
        let Some(mut backend) = backend else {
            let reason = if attach.frame_version() != self.frame_version
                || self.frame_version < FRAME_VERSION
            {
                Reason::FrameVersionUnsupported
            } else {
                Reason::UnknownCatalog
            };
            return refuse_selected(out, reason);
        };
        let capability_short = self.frame_version == FRAME_VERSION_3
            && (backend.streams().is_none() || backend.members().is_none());
        let limit = backend
            .members()
            .and_then(|table| limit_short(table, config.max_message_bytes()));
        // Ordinary attach refusals take precedence over configuration errors.
        let mut session = Session::new(backend, config);
        session.upgraded(self.frame_version);
        let step = session.on_message(msg, now, out);
        if step == Step::Continue && (capability_short || limit.is_some()) {
            out.clear();
            self.config_refused = true;
            if let Some((target, needed)) = limit {
                if let Some(record) = config.record() {
                    record(&Note::MessageLimitTooSmall { target, needed });
                }
            }
            return refuse_selected(
                out,
                // Capability takes precedence; a short limit is still recorded.
                if capability_short {
                    Reason::FrameVersionUnsupported
                } else {
                    Reason::Range
                },
            );
        }
        self.session = Some(session);
        step
    }
}

fn refuse_selected(out: &mut Vec<Vec<u8>>, reason: Reason) -> Step {
    let frame = Frame::Attached(AttachOutcome::Refused(reason));
    let mut msg = vec![0; frame.encoded_len()];
    frame.encode(&mut msg).expect("an attached frame fits");
    out.push(msg);
    Step::Close(CloseCode::NORMAL.0)
}

/// The member of `table` whose messages need the most bytes, with that
/// number, when it is above `limit`: the largest header and envelope of a
/// message that carries a payload, plus the member's limit. A `fixed` needs
/// at most one block message, 4,130 bytes, because a value that one `publish`
/// does not carry crosses in blocks: the smaller of 36 bytes plus its limit
/// and 4,130. The code clamps the limit of a `fixed` before it adds the header
/// and adds with `saturating_add`; no test on a 64-bit host checks the order
/// of these steps. On a tie, the first member in key order.
fn limit_short(table: &MemberTable, limit: usize) -> Option<(Target, usize)> {
    let header = largest_header();
    let block_cap = block_message() - header;
    let largest = table
        .entries()
        .fold(None, |largest, (target, member_limit)| {
            let member_limit = member_limit.map_or(default_ceiling(target.kind), |l| l as usize);
            let carried = if target.kind == Kind::Fixed {
                member_limit.min(block_cap)
            } else {
                member_limit
            };
            let needed = header.saturating_add(carried);
            match largest {
                Some((_, most)) if most >= needed => largest,
                _ => Some((target, needed)),
            }
        });
    largest.filter(|(_, needed)| *needed > limit)
}

/// The size of the largest header with its envelope of a message that carries
/// a payload: a `response` under `reply` with an empty payload, 36 bytes.
fn largest_header() -> usize {
    let header = Frame::Response {
        target: Target {
            kind: Kind::Query,
            iface: InterfaceNo(1),
            ord: Ordinal(1),
        },
        envelope: Envelope {
            stamp: Timestamp(0),
            seq: 0,
        },
        correlation: 0,
        outcome: ResponseOutcome::Reply(&[]),
    }
    .encoded_len();
    debug_assert_eq!(header, ENVELOPE_BYTES + 20);
    header
}

/// The size of one `block` message with a full block: the 34-byte header
/// and [`BLOCK_BYTES`] bytes, 4,130 bytes. `serve` checks the message limit
/// against it, and the session uses it to decide whether it can send blocks;
/// the session splits a value by [`BLOCK_BYTES`].
fn block_message() -> usize {
    let header = Frame::Block {
        target: Target {
            kind: Kind::Fixed,
            iface: InterfaceNo(1),
            ord: Ordinal(1),
        },
        envelope: Envelope {
            stamp: Timestamp(0),
            seq: 0,
        },
        total: 0,
        index: 0,
        payload: &[],
    }
    .encoded_len();
    header + BLOCK_BYTES
}

fn run<C, B, S>(
    conn: &mut C,
    session: &mut Selection<B, S>,
    config: &Config,
    max_version: u32,
) -> Ended
where
    C: Connection,
    B: Backend,
    S: for<'a> FnOnce(RequestedCatalog<'a>) -> Option<B>,
{
    if let Err(e) = conn.set_write_timeout(Some(config.liveness_timeout)) {
        return Ended::Io(e.kind());
    }
    let deadline = Instant::now() + config.liveness_timeout;
    let accept_origin = |origin: Option<&str>| config.origin_accepted(origin);
    match ws::handshake(
        &mut Deadline { conn, at: deadline },
        max_version,
        accept_origin,
    ) {
        Ok(Upgrade::Accepted(frame_version)) => {
            session.frame_version = frame_version;
            if let Some(inner) = &mut session.session {
                inner.upgraded(frame_version);
            }
        }
        Ok(Upgrade::Rejected | Upgrade::Forbidden) => {
            linger(conn);
            return Ended::Rejected;
        }
        Err(e) if timed_out(&e) => return Ended::LivenessTimeout,
        Err(e) => return Ended::Io(e.kind()),
    }
    run_upgraded(conn, session, config)
}

/// The shared WebSocket loop delegates only frame and tick dispositions.
trait Driver {
    fn on_message(
        &mut self,
        msg: &[u8],
        now: Instant,
        out: &mut Vec<Vec<u8>>,
        config: &Config,
    ) -> Step;
    fn tick(&mut self, now: Instant, out: &mut Vec<Vec<u8>>) -> Step;
    fn config_refused(&self) -> bool {
        false
    }
}

impl<B, S> Driver for Selection<B, S>
where
    B: Backend,
    S: for<'a> FnOnce(RequestedCatalog<'a>) -> Option<B>,
{
    fn on_message(
        &mut self,
        msg: &[u8],
        now: Instant,
        out: &mut Vec<Vec<u8>>,
        config: &Config,
    ) -> Step {
        Selection::on_message(self, msg, now, out, config)
    }
    fn tick(&mut self, now: Instant, out: &mut Vec<Vec<u8>>) -> Step {
        self.session
            .as_mut()
            .map_or(Step::Continue, |inner| inner.tick(now, out))
    }
    fn config_refused(&self) -> bool {
        self.config_refused
    }
}

fn run_upgraded<C: Connection, D: Driver>(conn: &mut C, session: &mut D, config: &Config) -> Ended {
    if let Err(e) = conn.set_read_timeout(Some(config.poll_interval)) {
        return Ended::Io(e.kind());
    }

    let mut parser = Parser::new(config.max_message_bytes());
    let mut buf = vec![0u8; READ_BUFFER];
    let mut out = Vec::new();
    let mut writer = FrameWriter::default();
    let opened = Instant::now();
    // The clock measures time since the client's last message, excluding
    // time spent in `flush`, because the server cannot read while it writes.
    // The write timeout ends a stalled reader.
    let mut heard = opened;
    let mut pinged = opened;
    let mut ticked = opened;
    loop {
        match conn.read(&mut buf) {
            Ok(0) => return Ended::Io(io::ErrorKind::UnexpectedEof),
            Ok(n) => parser.feed(&buf[..n]),
            Err(e) if timed_out(&e) || e.kind() == io::ErrorKind::Interrupted => {}
            Err(e) => return Ended::Io(e.kind()),
        }

        while let Some(event) = parser.next() {
            let now = Instant::now();
            heard = now;
            let step = match event {
                Ok(Event::Binary(msg)) => session.on_message(&msg, now, &mut out, config),
                Ok(Event::Ping(payload)) => match writer.pong(conn, &payload) {
                    Ok(()) => continue,
                    Err(e) => return write_failed(&e),
                },
                Ok(Event::Pong) => continue,
                Ok(Event::Close(code)) => {
                    // The session has ended whether or not the answer is
                    // written, so an error writing it is not reported.
                    let _ = writer.close(conn, code.map_or(CloseCode::NORMAL, CloseCode));
                    linger(conn);
                    return Ended::ClientClosed(code);
                }
                Err(code) => Step::Close(code.0),
            };
            let wrote = Instant::now();
            let ended = flush(conn, &mut writer, &mut out, step);
            heard += wrote.elapsed();
            if let Some(ended) = ended {
                return if session.config_refused() && ended == Ended::ServerClosed(1000) {
                    Ended::ConfigRefused
                } else {
                    ended
                };
            }
        }

        // A message is handled as soon as it is read, but the backend is
        // polled at most once per poll interval, whatever the client's rate.
        let now = Instant::now();
        if now.duration_since(ticked) >= config.poll_interval {
            ticked = now;
            let step = session.tick(now, &mut out);
            let wrote = Instant::now();
            let ended = flush(conn, &mut writer, &mut out, step);
            heard += wrote.elapsed();
            if let Some(ended) = ended {
                return ended;
            }
        }
        // The next read waits only until the next poll is due, so that a
        // message read just before it does not push the poll a whole
        // interval back: a change is still sent within one poll interval.
        let wait = (ticked + config.poll_interval)
            .saturating_duration_since(Instant::now())
            .max(MIN_READ_TIMEOUT);
        if let Err(e) = conn.set_read_timeout(Some(wait)) {
            return Ended::Io(e.kind());
        }
        if now.duration_since(heard) >= config.liveness_timeout {
            return Ended::LivenessTimeout;
        }
        if now.duration_since(pinged) >= config.ping_interval {
            if let Err(e) = writer.ping(conn) {
                return write_failed(&e);
            }
            pinged = now;
        }
    }
}

/// Writes each frame of `out` as one binary message, then, when `step` is a
/// close, a close frame followed by [`linger`]. Returns why the session
/// ended, when it did.
fn flush<C: Connection>(
    conn: &mut C,
    writer: &mut FrameWriter,
    out: &mut Vec<Vec<u8>>,
    step: Step,
) -> Option<Ended> {
    for msg in out.drain(..) {
        if let Err(e) = writer.binary(conn, &msg) {
            return Some(write_failed(&e));
        }
    }
    match step {
        Step::Continue => None,
        Step::Close(code) => {
            // As for the answer to a client's close: the session has ended.
            let _ = writer.close(conn, CloseCode(code));
            linger(conn);
            Some(Ended::ServerClosed(code))
        }
    }
}

/// Why the session ended when a write failed with `e`: a write that waited
/// its whole write timeout is a client that stopped reading.
fn write_failed(e: &io::Error) -> Ended {
    if timed_out(e) {
        Ended::LivenessTimeout
    } else {
        Ended::Io(e.kind())
    }
}

/// Whether `e` is a read or a write that waited its whole timeout.
fn timed_out(e: &io::Error) -> bool {
    matches!(
        e.kind(),
        io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
    )
}

/// After a handshake `400` or `403`, or a close frame the server sent: ends the write
/// side, then reads and drops what the client still sends, until the client
/// closes or [`LINGER`] passes. Closing a connection with unread bytes in its
/// receive buffer resets it, and a client may then lose the `400` or the
/// close frame it has not read yet.
fn linger<C: Connection>(conn: &mut C) {
    let _ = conn.shutdown_write();
    let until = Instant::now() + LINGER;
    let mut buf = [0u8; 4096];
    loop {
        let left = until.saturating_duration_since(Instant::now());
        if left.is_zero() || conn.set_read_timeout(Some(left)).is_err() {
            return;
        }
        match conn.read(&mut buf) {
            Ok(0) | Err(_) => return,
            Ok(_) => {}
        }
    }
}

/// A connection whose reads fail with `TimedOut` once `at` has passed: each
/// read waits at most until `at`, so that the handshake as a whole, and not
/// each byte of it, is bounded in time.
struct Deadline<'a, C> {
    conn: &'a mut C,
    at: Instant,
}

impl<C: Connection> Read for Deadline<'_, C> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let left = self.at.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return Err(io::ErrorKind::TimedOut.into());
        }
        self.conn.set_read_timeout(Some(left))?;
        self.conn.read(buf)
    }
}

impl<C: Connection> Write for Deadline<'_, C> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.conn.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.conn.flush()
    }
}
