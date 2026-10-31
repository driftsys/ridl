//! The server: the backend trait, the configuration, the session core that
//! turns decoded frames and backend samples into frames to send, and
//! [`serve`], the blocking loop that runs a session over one connection.

mod backend;
mod calls;
mod config;
#[cfg(feature = "descriptor")]
mod descriptor;
mod members;
mod session;

use std::io::{self, Read, Write};
use std::net::{Shutdown, TcpStream};
use std::time::{Duration, Instant};

use ridl_rt::contract::{default_ceiling, InterfaceNo, Kind, Ordinal, CALL_CEILING};
use ridl_rt::sample::{Envelope, Timestamp};

pub use backend::{Backend, Described, Polling, Scanning, TableMismatch};
pub use config::{Config, LimitOutOfRange, Note, RetainedTooSmall};
#[cfg(feature = "descriptor")]
pub use descriptor::DescriptorError;
pub use members::MemberTable;

use crate::frame::{Frame, ResponseOutcome, Target, ENVELOPE_BYTES};
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
    /// upgrade needs, its `Sec-WebSocket-Version` was not 13, or it did not
    /// offer `ridl-ws-v2` or `ridl-ws-v1`; or `403 Forbidden`, because the
    /// check set by [`Config::with_origin_check`] refused its `Origin`.
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
    /// of the catalog needs (frame specification §11.1 "The sizes"): the
    /// configuration cannot carry a payload the catalog allows, which is a
    /// configuration error, not a session event. [`serve`] returned before
    /// the opening handshake, without reading the connection, and recorded
    /// [`Note::MessageLimitTooSmall`]; the connection closes when it is
    /// dropped. Without a member table there is no check.
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
/// neither multiplies the polls nor delays them. A change on the backend is
/// sent at most one poll interval after it, plus the write. Each write call has a write timeout of
/// [`Config::liveness_timeout`]: a client that reads nothing for that long,
/// so that a write call makes no progress, ends the session with
/// [`Ended::LivenessTimeout`]. A client that keeps reading, however slowly,
/// does not.
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
/// the table gives it or the default ceiling of its kind, and at most
/// 16,128 bytes for a `fixed`, what one message carries. A limit below that
/// is a configuration error: `serve` records [`Note::MessageLimitTooSmall`]
/// and returns [`Ended::ConfigRefused`] before the opening handshake,
/// without reading the connection. The default limit carries every catalog.
///
/// A zero [`Config::poll_interval`] or [`Config::liveness_timeout`] is
/// refused by the stream's `set_read_timeout` or `set_write_timeout`, and
/// `serve` returns [`Ended::Io`] with `InvalidInput`.
pub fn serve<C: Connection, B: Backend>(mut conn: C, backend: B, config: &Config) -> Ended {
    if let Some((target, needed)) = backend
        .members()
        .and_then(|table| limit_short(table, config.max_message_bytes()))
    {
        if let Some(record) = config.record() {
            record(&Note::MessageLimitTooSmall { target, needed });
        }
        return Ended::ConfigRefused;
    }
    let mut session = Session::new(backend, config);
    let ended = run(&mut conn, &mut session, config);
    session.close();
    ended
}

/// The member of `table` whose messages need the most bytes, with that
/// number, when it is above `limit`: the largest data-plane header and
/// envelope plus the member's limit, at most [`CALL_CEILING`] for a `fixed`,
/// whose bytes above it one message does not carry. On a tie, the first
/// member in key order.
fn limit_short(table: &MemberTable, limit: usize) -> Option<(Target, usize)> {
    let largest = table
        .entries()
        .fold(None, |largest, (target, member_limit)| {
            let member_limit = member_limit.map_or(default_ceiling(target.kind), |l| l as usize);
            let carried = if target.kind == Kind::Fixed {
                member_limit.min(CALL_CEILING)
            } else {
                member_limit
            };
            let needed = largest_header().saturating_add(carried);
            match largest {
                Some((_, most)) if most >= needed => largest,
                _ => Some((target, needed)),
            }
        });
    largest.filter(|(_, needed)| *needed > limit)
}

/// The size of the largest data-plane header with its envelope: a
/// `response` under `reply` with an empty payload, 36 bytes.
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

fn run<C: Connection, B: Backend>(
    conn: &mut C,
    session: &mut Session<B>,
    config: &Config,
) -> Ended {
    if let Err(e) = conn.set_write_timeout(Some(config.liveness_timeout)) {
        return Ended::Io(e.kind());
    }
    let deadline = Instant::now() + config.liveness_timeout;
    let accept_origin = |origin: Option<&str>| config.origin_accepted(origin);
    match ws::handshake(&mut Deadline { conn, at: deadline }, accept_origin) {
        Ok(Upgrade::Accepted(frame_version)) => session.upgraded(frame_version),
        Ok(Upgrade::Rejected | Upgrade::Forbidden) => {
            linger(conn);
            return Ended::Rejected;
        }
        Err(e) if timed_out(&e) => return Ended::LivenessTimeout,
        Err(e) => return Ended::Io(e.kind()),
    }
    if let Err(e) = conn.set_read_timeout(Some(config.poll_interval)) {
        return Ended::Io(e.kind());
    }

    let mut parser = Parser::new(config.max_message_bytes());
    let mut buf = vec![0u8; READ_BUFFER];
    let mut out = Vec::new();
    let mut writer = FrameWriter::default();
    let opened = Instant::now();
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
                Ok(Event::Binary(msg)) => session.on_message(&msg, now, &mut out),
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
            if let Some(ended) = flush(conn, &mut writer, &mut out, step) {
                return ended;
            }
        }

        // A message is handled as soon as it is read, but the backend is
        // polled at most once per poll interval, whatever the client's rate.
        let now = Instant::now();
        if now.duration_since(ticked) >= config.poll_interval {
            ticked = now;
            let step = session.tick(now, &mut out);
            if let Some(ended) = flush(conn, &mut writer, &mut out, step) {
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
