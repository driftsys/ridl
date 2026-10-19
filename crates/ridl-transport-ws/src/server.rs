//! The server: the backend trait, the configuration, the session core that
//! turns decoded frames and backend samples into frames to send, and
//! [`serve`], the blocking loop that runs a session over one connection.

mod backend;
mod calls;
mod config;
mod session;

use std::io::{self, Read, Write};
use std::net::{Shutdown, TcpStream};
use std::time::{Duration, Instant};

pub use backend::{Backend, Polling, Scanning};
pub use config::{Config, LimitTooSmall, Note};

use crate::ws::{self, CloseCode, Event, Parser, Upgrade};
use session::{Session, Step};

/// A connected stream that [`serve`] runs one session over.
///
/// The embedder accepts the connection and applies its access policy before
/// it calls [`serve`]; the crate binds, listens and connects nothing.
pub trait Connection: Read + Write {
    /// Sets how long one read waits for bytes before it fails with
    /// `WouldBlock` or `TimedOut`. `None` waits without limit.
    ///
    /// # Errors
    ///
    /// The error of the underlying stream, such as `InvalidInput` for a zero
    /// duration.
    fn set_read_timeout(&self, t: Option<Duration>) -> io::Result<()>;

    /// Ends the write side of the stream, so that the peer reads the end of
    /// the stream after the bytes already written. [`serve`] calls it after
    /// it answers a handshake `400 Bad Request`. The default does nothing,
    /// for a stream that has no write side of its own to end.
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

    fn shutdown_write(&self) -> io::Result<()> {
        self.shutdown(Shutdown::Write)
    }
}

#[cfg(unix)]
impl Connection for std::os::unix::net::UnixStream {
    fn set_read_timeout(&self, t: Option<Duration>) -> io::Result<()> {
        std::os::unix::net::UnixStream::set_read_timeout(self, t)
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
    /// on a message over [`Config::max_message_bytes`]. A violation of the
    /// frame is also recorded as a [`Note`].
    ServerClosed(u16),
    /// The opening handshake was answered `400 Bad Request`: the request was
    /// malformed, its head passed 8 KiB, or it did not offer `ridl-ws-v1`.
    Rejected,
    /// The opening handshake did not finish within
    /// [`Config::liveness_timeout`], or no message of any kind, data or
    /// control, arrived for that long after it. No close frame is sent.
    LivenessTimeout,
    /// A read or a write of the connection failed with this kind of error.
    /// `UnexpectedEof` is a client that closed the connection without a
    /// close frame.
    Io(io::ErrorKind),
}

/// The size of the buffer each socket read fills.
const READ_BUFFER: usize = 16 * 1024;

/// How long the server goes on reading after it answers a handshake `400`,
/// so that the client reads the answer before the connection closes.
const REJECT_LINGER: Duration = Duration::from_millis(100);

/// Serves `backend` to the WebSocket client on `conn`, and returns when the
/// session ends.
///
/// `serve` blocks the calling thread for the whole session. It answers the
/// opening handshake, then loops: it reads `conn` with a read timeout of
/// [`Config::poll_interval`], handles each whole message read, polls the
/// backend once and sends what changed, and sends a ping each
/// [`Config::ping_interval`]. A change on the backend is sent at most one
/// poll interval after it, plus the write.
///
/// At every end, each backend call in flight is forgotten and `backend` is
/// dropped, with its event subscriptions; then `conn` is dropped. After a
/// close frame, sent or answered, `serve` returns without waiting for the
/// client to close the connection: the client waits for the server to close
/// it (RFC 6455 §7.1.1).
///
/// A zero [`Config::poll_interval`] is refused by the stream's
/// `set_read_timeout`, and `serve` returns [`Ended::Io`] with
/// `InvalidInput`.
pub fn serve<C: Connection, B: Backend>(mut conn: C, backend: B, config: &Config) -> Ended {
    let mut session = Session::new(backend, config);
    let ended = run(&mut conn, &mut session, config);
    session.close();
    ended
}

fn run<C: Connection, B: Backend>(
    conn: &mut C,
    session: &mut Session<B>,
    config: &Config,
) -> Ended {
    let deadline = Instant::now() + config.liveness_timeout;
    match ws::handshake(&mut Deadline { conn, at: deadline }) {
        Ok(Upgrade::Accepted) => {}
        Ok(Upgrade::Rejected) => {
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
    let opened = Instant::now();
    let mut heard = opened;
    let mut pinged = opened;
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
                Ok(Event::Ping(payload)) => match ws::write_pong(conn, &payload) {
                    Ok(()) => continue,
                    Err(e) => return Ended::Io(e.kind()),
                },
                Ok(Event::Pong) => continue,
                Ok(Event::Close(code)) => {
                    // The session has ended whether or not the answer is
                    // written, so an error writing it is not reported.
                    let _ = ws::write_close(conn, code.map_or(CloseCode::NORMAL, CloseCode));
                    return Ended::ClientClosed(code);
                }
                Err(code) => Step::Close(code.0),
            };
            if let Some(ended) = flush(conn, &mut out, step) {
                return ended;
            }
        }

        let now = Instant::now();
        let step = session.tick(now, &mut out);
        if let Some(ended) = flush(conn, &mut out, step) {
            return ended;
        }
        if now.duration_since(heard) >= config.liveness_timeout {
            return Ended::LivenessTimeout;
        }
        if now.duration_since(pinged) >= config.ping_interval {
            if let Err(e) = ws::write_ping(conn) {
                return Ended::Io(e.kind());
            }
            pinged = now;
        }
    }
}

/// Writes each frame of `out` as one binary message, then a close frame when
/// `step` is a close. Returns why the session ended, when it did.
fn flush<C: Write>(conn: &mut C, out: &mut Vec<Vec<u8>>, step: Step) -> Option<Ended> {
    for msg in out.drain(..) {
        if let Err(e) = ws::write_binary(conn, &msg) {
            return Some(Ended::Io(e.kind()));
        }
    }
    match step {
        Step::Continue => None,
        Step::Close(code) => {
            // As for the answer to a client's close: the session has ended.
            let _ = ws::write_close(conn, CloseCode(code));
            Some(Ended::ServerClosed(code))
        }
    }
}

/// Whether `e` is a read that waited its whole read timeout.
fn timed_out(e: &io::Error) -> bool {
    matches!(
        e.kind(),
        io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
    )
}

/// After a handshake `400`: ends the write side, then reads and drops what
/// the client still sends, until the client closes or [`REJECT_LINGER`]
/// passes. Closing a connection with unread bytes in its receive buffer
/// resets it, and a client may then lose the `400` it has not read yet.
fn linger<C: Connection>(conn: &mut C) {
    let _ = conn.shutdown_write();
    let until = Instant::now() + REJECT_LINGER;
    let mut buf = [0u8; 512];
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
