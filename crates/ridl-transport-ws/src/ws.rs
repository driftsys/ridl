//! The server side of RFC 6455: the opening handshake, the reading of masked
//! client frames into messages, and the writing of unmasked server frames.
//!
//! Nothing here binds, listens or connects: every function works on a stream
//! or a buffer that the caller supplies.

mod base64;
mod sha1;

use std::io::{self, Read, Write};

use crate::frame;

/// The most bytes the request line and the headers may take, the empty line
/// that ends them included.
const MAX_HEAD_BYTES: usize = 8 * 1024;

/// The value RFC 6455 §1.3 appends to `Sec-WebSocket-Key` before hashing.
const ACCEPT_GUID: &str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";

/// The response to every request that is not upgraded, but for one whose
/// `Sec-WebSocket-Version` is not 13.
const BAD_REQUEST: &[u8] =
    b"HTTP/1.1 400 Bad Request\r\nConnection: close\r\nContent-Length: 0\r\n\r\n";

/// The response to a request whose `Sec-WebSocket-Version` is not 13: the
/// version the server speaks is named (RFC 6455 §4.2.2 item 4).
const BAD_VERSION: &[u8] = b"HTTP/1.1 400 Bad Request\r\nSec-WebSocket-Version: 13\r\n\
    Connection: close\r\nContent-Length: 0\r\n\r\n";

/// The response to an upgrade request whose `Origin` the check refused.
const FORBIDDEN: &[u8] =
    b"HTTP/1.1 403 Forbidden\r\nConnection: close\r\nContent-Length: 0\r\n\r\n";

/// The result of [`handshake`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Upgrade {
    /// The server answered `101 Switching Protocols` with `ridl-ws-v1`
    /// selected. The stream now carries WebSocket frames.
    Accepted,
    /// The server answered `400 Bad Request`. The caller closes the stream.
    Rejected,
    /// The server answered `403 Forbidden`: the origin check refused the
    /// request's `Origin`. The caller closes the stream.
    Forbidden,
}

/// Reads one HTTP/1.1 upgrade request from `stream` and answers it.
///
/// The request is upgraded when it is a `GET` with a `Host` header,
/// `Upgrade: websocket`, `Connection: Upgrade`, `Sec-WebSocket-Version: 13`,
/// one `Sec-WebSocket-Key` that decodes from base64 to 16 bytes, and
/// `ridl-ws-v1` in any position of `Sec-WebSocket-Protocol` (RFC 6455
/// §4.2.1), and when `accept_origin` accepts its `Origin` header, which is
/// `None` when the request has none. The path, the query and every other
/// header are ignored. A request head over 8 KiB, a malformed request, a
/// request with more than one `Origin` header, or a request that does not
/// offer `ridl-ws-v1` is answered `400 Bad Request` and returns
/// [`Upgrade::Rejected`]; when the version is not 13, the `400` names the
/// version the server speaks in a `Sec-WebSocket-Version: 13` header (RFC
/// 6455 §4.2.2 item 4). A request whose `Origin` `accept_origin` refuses is
/// answered `403 Forbidden` and returns [`Upgrade::Forbidden`].
///
/// The head is read one byte at a time, so that no byte after the empty line
/// that ends it is taken from the stream.
///
/// # Errors
///
/// An error of `stream`, or `UnexpectedEof` when the stream ends before the
/// head does.
pub(crate) fn handshake<S: Read + Write>(
    stream: &mut S,
    accept_origin: impl FnOnce(Option<&str>) -> bool,
) -> io::Result<Upgrade> {
    let Some(head) = read_head(stream)? else {
        return reject(stream, BAD_REQUEST);
    };
    let request = match upgrade_request(&head) {
        Ok(request) => request,
        Err(Reject::BadRequest) => return reject(stream, BAD_REQUEST),
        Err(Reject::Version) => return reject(stream, BAD_VERSION),
    };
    if !accept_origin(request.origin) {
        stream.write_all(FORBIDDEN)?;
        stream.flush()?;
        return Ok(Upgrade::Forbidden);
    }
    let response = format!(
        "HTTP/1.1 101 Switching Protocols\r\n\
         Upgrade: websocket\r\n\
         Connection: Upgrade\r\n\
         Sec-WebSocket-Accept: {}\r\n\
         Sec-WebSocket-Protocol: {}\r\n\
         \r\n",
        accept_key(request.key),
        frame::SUBPROTOCOL,
    );
    stream.write_all(response.as_bytes())?;
    stream.flush()?;
    Ok(Upgrade::Accepted)
}

fn reject<S: Write>(stream: &mut S, response: &[u8]) -> io::Result<Upgrade> {
    stream.write_all(response)?;
    stream.flush()?;
    Ok(Upgrade::Rejected)
}

/// Reads up to and including the empty line that ends the request head.
/// Returns `None` when the head passes [`MAX_HEAD_BYTES`].
fn read_head<S: Read>(stream: &mut S) -> io::Result<Option<Vec<u8>>> {
    let mut head = Vec::with_capacity(512);
    let mut byte = [0u8; 1];
    while !head.ends_with(b"\r\n\r\n") {
        if head.len() == MAX_HEAD_BYTES {
            return Ok(None);
        }
        match stream.read(&mut byte) {
            Ok(0) => return Err(io::ErrorKind::UnexpectedEof.into()),
            Ok(_) => head.push(byte[0]),
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(e) => return Err(e),
        }
    }
    Ok(Some(head))
}

/// The headers of an upgrade request that [`handshake`] uses after it has
/// checked them.
struct UpgradeRequest<'a> {
    /// The `Sec-WebSocket-Key`.
    key: &'a str,
    /// The `Origin`, when the request has one.
    origin: Option<&'a str>,
}

/// Why a request is not upgraded, which decides the `400` written.
enum Reject {
    /// The request is malformed, or it is not the upgrade this server
    /// accepts.
    BadRequest,
    /// The request is well formed and its `Sec-WebSocket-Version` is not 13.
    Version,
}

/// Returns the key and the origin of `head` when `head` is an upgrade request
/// this server accepts, and why not otherwise.
fn upgrade_request(head: &[u8]) -> Result<UpgradeRequest<'_>, Reject> {
    let head = std::str::from_utf8(head).map_err(|_| Reject::BadRequest)?;
    let head = head.strip_suffix("\r\n\r\n").ok_or(Reject::BadRequest)?;
    let mut lines = head.split("\r\n");

    let mut request_line = lines.next().ok_or(Reject::BadRequest)?.split(' ');
    let (method, target, version) = (
        request_line.next().ok_or(Reject::BadRequest)?,
        request_line.next().ok_or(Reject::BadRequest)?,
        request_line.next().ok_or(Reject::BadRequest)?,
    );
    if request_line.next().is_some()
        || method != "GET"
        || target.is_empty()
        || version != "HTTP/1.1"
    {
        return Err(Reject::BadRequest);
    }

    let mut host = false;
    let mut upgrade = false;
    let mut connection = false;
    let mut version_13 = false;
    let mut subprotocol = false;
    let mut key = None;
    let mut origin = None;
    for line in lines {
        // A line that starts with white space continues the previous header
        // (obsolete line folding), which RFC 9112 §5.2 lets a server reject.
        if line.starts_with([' ', '\t']) {
            return Err(Reject::BadRequest);
        }
        let (name, value) = line.split_once(':').ok_or(Reject::BadRequest)?;
        let value = value.trim_matches([' ', '\t']);
        if name.eq_ignore_ascii_case("Host") {
            host = true;
        } else if name.eq_ignore_ascii_case("Upgrade") {
            upgrade |= has_token(value, "websocket");
        } else if name.eq_ignore_ascii_case("Connection") {
            connection |= has_token(value, "upgrade");
        } else if name.eq_ignore_ascii_case("Sec-WebSocket-Version") {
            version_13 |= value == "13";
        } else if name.eq_ignore_ascii_case("Sec-WebSocket-Protocol") {
            subprotocol |= value
                .split(',')
                .any(|p| p.trim_matches([' ', '\t']) == frame::SUBPROTOCOL);
        } else if name.eq_ignore_ascii_case("Sec-WebSocket-Key") {
            // One key, of 16 bytes in base64 (RFC 6455 §4.2.1 item 5).
            if key.is_some() || !base64::decode(value).is_some_and(|k| k.len() == 16) {
                return Err(Reject::BadRequest);
            }
            key = Some(value);
        } else if name.eq_ignore_ascii_case("Origin") {
            // One request has one origin (RFC 6454 §7.3); with two, the
            // check could see one and a proxy the other.
            if origin.is_some() {
                return Err(Reject::BadRequest);
            }
            origin = Some(value);
        }
    }
    if !version_13 {
        return Err(Reject::Version);
    }
    if !(host && upgrade && connection && subprotocol) {
        return Err(Reject::BadRequest);
    }
    let key = key.ok_or(Reject::BadRequest)?;
    Ok(UpgradeRequest { key, origin })
}

/// Whether the comma-separated list `value` holds `token`, compared without
/// regard to ASCII case.
fn has_token(value: &str, token: &str) -> bool {
    value
        .split(',')
        .any(|t| t.trim_matches([' ', '\t']).eq_ignore_ascii_case(token))
}

/// The `Sec-WebSocket-Accept` value for `key` (RFC 6455 §4.2.2 item 5.4).
fn accept_key(key: &str) -> String {
    let mut input = String::with_capacity(key.len() + ACCEPT_GUID.len());
    input.push_str(key);
    input.push_str(ACCEPT_GUID);
    base64::encode(&sha1::digest(input.as_bytes()))
}

/// A WebSocket close code (RFC 6455 §7.4), limited to those frame
/// specification §11.1 item 4 gives the server.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CloseCode(pub u16);

impl CloseCode {
    /// The session ends on purpose.
    pub(crate) const NORMAL: Self = Self(1000);
    /// The backend detached.
    pub(crate) const GOING_AWAY: Self = Self(1001);
    /// The peer broke RFC 6455 or the frame specification.
    pub(crate) const PROTOCOL: Self = Self(1002);
    /// The peer sent a text message.
    pub(crate) const UNSUPPORTED: Self = Self(1003);
    /// The peer sent a message over the size limit.
    pub(crate) const TOO_BIG: Self = Self(1009);
}

/// One thing a [`Parser`] read from the client.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Event {
    /// A whole binary message, its fragments joined.
    Binary(Vec<u8>),
    /// A ping, with its payload, which the pong carries back.
    Ping(Vec<u8>),
    /// A pong.
    Pong,
    /// A close frame, with its status code when it carries one.
    Close(Option<u16>),
}

const OP_CONTINUATION: u8 = 0x0;
const OP_TEXT: u8 = 0x1;
const OP_BINARY: u8 = 0x2;
const OP_CLOSE: u8 = 0x8;
const OP_PING: u8 = 0x9;
const OP_PONG: u8 = 0xA;

/// The largest payload of a control frame (RFC 6455 §5.5).
const MAX_CONTROL_PAYLOAD: u64 = 125;

/// Reads client frames from bytes fed in any pieces, and returns whole
/// messages and control frames.
///
/// A read may end inside a frame, and a message may be split into fragments,
/// with control frames between them (RFC 6455 §5.4). After [`Parser::next`]
/// returns an error or [`Event::Close`], it returns `None` from then on.
#[derive(Debug)]
pub(crate) struct Parser {
    max_message_bytes: usize,
    /// Bytes fed. The bytes before `pos` are consumed, and `pos` is at a
    /// frame boundary.
    buf: Vec<u8>,
    pos: usize,
    /// The fragments of a binary message whose final frame has not arrived.
    message: Option<Vec<u8>>,
    finished: bool,
}

/// A frame header that [`Parser`] has read whole.
struct Header {
    fin: bool,
    opcode: u8,
    mask: [u8; 4],
    payload_len: u64,
    header_len: usize,
}

impl Parser {
    /// A parser that closes with 1009 a message whose size passes
    /// `max_message_bytes`.
    pub(crate) fn new(max_message_bytes: usize) -> Self {
        Self {
            max_message_bytes,
            buf: Vec::new(),
            pos: 0,
            message: None,
            finished: false,
        }
    }

    /// Adds bytes read from the client.
    pub(crate) fn feed(&mut self, bytes: &[u8]) {
        if !self.finished {
            // The consumed bytes are removed once per call, not once per
            // frame, so that a read holding many frames costs linear time.
            self.buf.drain(..self.pos);
            self.pos = 0;
            self.buf.extend_from_slice(bytes);
        }
    }

    /// Returns the next event, or `None` when the bytes fed so far do not hold
    /// one. An error is the close code the server sends before it closes.
    ///
    /// The size limit is checked from a frame's length field, before its
    /// payload is buffered.
    pub(crate) fn next(&mut self) -> Option<Result<Event, CloseCode>> {
        if self.finished {
            return None;
        }
        let result = self.step();
        if matches!(result, Some(Err(_)) | Some(Ok(Event::Close(_)))) {
            self.finished = true;
            self.buf = Vec::new();
            self.pos = 0;
            self.message = None;
        }
        result
    }

    fn step(&mut self) -> Option<Result<Event, CloseCode>> {
        loop {
            let header = match self.header()? {
                Ok(header) => header,
                Err(code) => return Some(Err(code)),
            };
            let total = header.header_len + header.payload_len as usize;
            let frame = self.buf.get(self.pos..self.pos + total)?;
            let payload: Vec<u8> = frame[header.header_len..]
                .iter()
                .enumerate()
                .map(|(i, byte)| byte ^ header.mask[i % 4])
                .collect();
            self.pos += total;
            match header.opcode {
                OP_PING => return Some(Ok(Event::Ping(payload))),
                OP_PONG => return Some(Ok(Event::Pong)),
                OP_CLOSE => return Some(close_event(&payload)),
                OP_BINARY if header.fin => return Some(Ok(Event::Binary(payload))),
                OP_BINARY => self.message = Some(payload),
                _ => {
                    // A continuation: `header` checked that a message is open.
                    let mut message = self.message.take().unwrap_or_default();
                    message.extend_from_slice(&payload);
                    if header.fin {
                        return Some(Ok(Event::Binary(message)));
                    }
                    self.message = Some(message);
                }
            }
        }
    }

    /// Reads the header at `pos` in `buf`. Returns `None` when `buf` does not
    /// hold all of it, and an error when the frame is one this server closes
    /// on. Each check runs as soon as the bytes it reads are in `buf`, so that
    /// an unmasked frame, which has no mask key, is not waited on.
    fn header(&self) -> Option<Result<Header, CloseCode>> {
        let buf = &self.buf[self.pos..];
        let b0 = *buf.first()?;
        let b1 = *buf.get(1)?;
        let fin = b0 & 0x80 != 0;
        let opcode = b0 & 0x0F;
        let len7 = b1 & 0x7F;

        // No extension is negotiated, so every reserved bit is 0 (§5.2), and
        // a client masks every frame (§5.1).
        if b0 & 0x70 != 0 || b1 & 0x80 == 0 {
            return Some(Err(CloseCode::PROTOCOL));
        }
        match opcode {
            OP_CLOSE | OP_PING | OP_PONG => {
                if !fin || u64::from(len7) > MAX_CONTROL_PAYLOAD {
                    return Some(Err(CloseCode::PROTOCOL));
                }
            }
            OP_TEXT if self.message.is_none() => return Some(Err(CloseCode::UNSUPPORTED)),
            OP_BINARY if self.message.is_none() => {}
            OP_CONTINUATION if self.message.is_some() => {}
            _ => return Some(Err(CloseCode::PROTOCOL)),
        }

        // A 16-bit or 64-bit length that would fit a shorter form is
        // accepted: RFC 6455 §5.2 puts the minimal-encoding rule on the
        // sender, and the size limit below applies to the value either way.
        let (payload_len, len_bytes) = match len7 {
            126 => {
                let b = buf.get(2..4)?;
                (u64::from(u16::from_be_bytes([b[0], b[1]])), 2)
            }
            127 => {
                let b = buf.get(2..10)?;
                (u64::from_be_bytes(b.try_into().ok()?), 8)
            }
            n => (u64::from(n), 0),
        };
        if payload_len >> 63 != 0 {
            return Some(Err(CloseCode::PROTOCOL));
        }
        // The limit is on a message, so it counts data frames only. A control
        // frame between fragments is not part of the message, and its size is
        // limited to 125 bytes above.
        let control = matches!(opcode, OP_CLOSE | OP_PING | OP_PONG);
        let open = self.message.as_ref().map_or(0, Vec::len);
        if !control && payload_len > (self.max_message_bytes - open) as u64 {
            return Some(Err(CloseCode::TOO_BIG));
        }

        let header_len = 2 + len_bytes + 4;
        let mask: [u8; 4] = buf.get(2 + len_bytes..header_len)?.try_into().ok()?;
        Some(Ok(Header {
            fin,
            opcode,
            mask,
            payload_len,
            header_len,
        }))
    }
}

/// The event for a close frame's payload (RFC 6455 §5.5.1, §7.4).
fn close_event(payload: &[u8]) -> Result<Event, CloseCode> {
    match payload {
        [] => Ok(Event::Close(None)),
        [hi, lo, reason @ ..] => {
            let code = u16::from_be_bytes([*hi, *lo]);
            // The codes a peer may send: those §7.4.1 defines for use in a
            // close frame, the ones registered after it (1012 to 1014), and
            // the ranges §7.4.2 leaves to libraries and applications.
            let valid = matches!(code, 1000..=1003 | 1007..=1014 | 3000..=4999);
            if valid && std::str::from_utf8(reason).is_ok() {
                Ok(Event::Close(Some(code)))
            } else {
                Err(CloseCode::PROTOCOL)
            }
        }
        [_] => Err(CloseCode::PROTOCOL),
    }
}

/// Writes unmasked, final server frames. Each frame is assembled, header
/// and payload, in one buffer the writer keeps from frame to frame, and
/// written with one `write_all`: a frame written as two writes could wait
/// for the peer's delayed acknowledgement of the first (Nagle's algorithm)
/// before the second is sent.
#[derive(Debug, Default)]
pub(crate) struct FrameWriter {
    buf: Vec<u8>,
}

impl FrameWriter {
    fn frame(&mut self, w: &mut impl Write, opcode: u8, payload: &[u8]) -> io::Result<()> {
        self.buf.clear();
        self.buf.push(0x80 | opcode);
        match payload.len() {
            n @ 0..=125 => self.buf.push(n as u8),
            n @ 126..=0xFFFF => {
                self.buf.push(126);
                self.buf.extend_from_slice(&(n as u16).to_be_bytes());
            }
            n => {
                self.buf.push(127);
                self.buf.extend_from_slice(&(n as u64).to_be_bytes());
            }
        }
        self.buf.extend_from_slice(payload);
        w.write_all(&self.buf)?;
        w.flush()
    }

    /// Writes `msg` as one binary message in one frame.
    pub(crate) fn binary(&mut self, w: &mut impl Write, msg: &[u8]) -> io::Result<()> {
        self.frame(w, OP_BINARY, msg)
    }

    /// Writes a ping with an empty payload.
    pub(crate) fn ping(&mut self, w: &mut impl Write) -> io::Result<()> {
        self.frame(w, OP_PING, &[])
    }

    /// Writes a pong carrying `payload`, the payload of the ping it answers,
    /// which [`Parser`] limits to 125 bytes.
    pub(crate) fn pong(&mut self, w: &mut impl Write, payload: &[u8]) -> io::Result<()> {
        self.frame(w, OP_PONG, payload)
    }

    /// Writes a close frame carrying `code` and no reason.
    pub(crate) fn close(&mut self, w: &mut impl Write, code: CloseCode) -> io::Result<()> {
        self.frame(w, OP_CLOSE, &code.0.to_be_bytes())
    }
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::thread;

    use tungstenite::client::IntoClientRequest;
    use tungstenite::protocol::frame::coding::{CloseCode as WsCloseCode, Data, OpCode};
    use tungstenite::protocol::frame::{CloseFrame, Frame};
    use tungstenite::{Message, WebSocket};

    use super::*;

    /// A connected pair: the server's end, accepted, and the client's end.
    fn pair() -> (TcpStream, TcpStream) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let (server, _) = listener.accept().unwrap();
        // A test whose peer never answers fails instead of waiting forever.
        let limit = Some(std::time::Duration::from_secs(10));
        server.set_read_timeout(limit).unwrap();
        client.set_read_timeout(limit).unwrap();
        (server, client)
    }

    /// Runs the `tungstenite` client handshake on `stream`, offering
    /// `protocols` in `Sec-WebSocket-Protocol` when it is not empty.
    fn connect(
        stream: TcpStream,
        protocols: &str,
    ) -> Result<(WebSocket<TcpStream>, Option<String>), tungstenite::Error> {
        let mut request = "ws://localhost/any/path?q=1".into_client_request().unwrap();
        if !protocols.is_empty() {
            request
                .headers_mut()
                .insert("Sec-WebSocket-Protocol", protocols.parse().unwrap());
        }
        match tungstenite::client(request, stream) {
            Ok((ws, response)) => {
                let selected = response
                    .headers()
                    .get("Sec-WebSocket-Protocol")
                    .map(|v| v.to_str().unwrap().to_owned());
                Ok((ws, selected))
            }
            Err(tungstenite::HandshakeError::Failure(e)) => Err(e),
            Err(tungstenite::HandshakeError::Interrupted(_)) => unreachable!("blocking stream"),
        }
    }

    /// Connects a `tungstenite` client that offers `ridl-ws-v1`, runs `client`
    /// on its thread, and returns the server's end, upgraded, with the thread.
    fn upgraded<T: Send + 'static>(
        client: impl FnOnce(WebSocket<TcpStream>) -> T + Send + 'static,
    ) -> (TcpStream, thread::JoinHandle<T>) {
        let (mut server, client_end) = pair();
        let handle = thread::spawn(move || {
            let (ws, _) = connect(client_end, frame::SUBPROTOCOL).unwrap();
            client(ws)
        });
        assert_eq!(handshake(&mut server, |_| true).unwrap(), Upgrade::Accepted);
        (server, handle)
    }

    /// Reads from `stream` into `parser` until it returns an event.
    fn read_event(stream: &mut TcpStream, parser: &mut Parser) -> Result<Event, CloseCode> {
        let mut buf = [0u8; 4096];
        loop {
            if let Some(result) = parser.next() {
                return result;
            }
            let n = stream.read(&mut buf).unwrap();
            assert_ne!(n, 0, "the client closed before an event");
            parser.feed(&buf[..n]);
        }
    }

    /// Reads messages on the client until a close frame, and returns its code.
    fn client_close_code(mut ws: WebSocket<TcpStream>) -> Option<WsCloseCode> {
        loop {
            match ws.read().unwrap() {
                Message::Close(frame) => return frame.map(|f| f.code),
                _ => continue,
            }
        }
    }

    /// A masked client frame, written by hand.
    fn masked(first_byte: u8, payload: &[u8]) -> Vec<u8> {
        let mask = [0x37, 0xFA, 0x21, 0x3D];
        let mut bytes = vec![first_byte];
        match payload.len() {
            n @ 0..=125 => bytes.push(0x80 | n as u8),
            n @ 126..=0xFFFF => {
                bytes.push(0x80 | 126);
                bytes.extend_from_slice(&(n as u16).to_be_bytes());
            }
            n => {
                bytes.push(0x80 | 127);
                bytes.extend_from_slice(&(n as u64).to_be_bytes());
            }
        }
        bytes.extend_from_slice(&mask);
        bytes.extend(payload.iter().enumerate().map(|(i, b)| b ^ mask[i % 4]));
        bytes
    }

    fn parse_all(max: usize, bytes: &[u8]) -> Vec<Result<Event, CloseCode>> {
        let mut parser = Parser::new(max);
        parser.feed(bytes);
        std::iter::from_fn(|| parser.next()).collect()
    }

    /// Runs [`handshake`] over an in-memory stream that holds `head`, with
    /// no origin check, and returns the result and the response the server
    /// wrote.
    fn handshake_on(head: &str) -> (Upgrade, String) {
        handshake_checked(head, |_| true)
    }

    /// The same with `accept_origin` as the origin check.
    fn handshake_checked(
        head: &str,
        accept_origin: impl FnOnce(Option<&str>) -> bool,
    ) -> (Upgrade, String) {
        struct Mem<'a> {
            input: &'a [u8],
            output: Vec<u8>,
        }
        impl Read for Mem<'_> {
            fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
                self.input.read(buf)
            }
        }
        impl Write for Mem<'_> {
            fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
                self.output.write(buf)
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        let mut mem = Mem {
            input: head.as_bytes(),
            output: Vec::new(),
        };
        let upgrade = handshake(&mut mem, accept_origin).unwrap();
        (upgrade, String::from_utf8(mem.output).unwrap())
    }

    const GOOD_HEAD: &str = "GET /chat HTTP/1.1\r\n\
        Host: server.example.com\r\n\
        Upgrade: websocket\r\n\
        Connection: keep-alive, Upgrade\r\n\
        Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\
        Sec-WebSocket-Protocol: ridl-ws-v1\r\n\
        Sec-WebSocket-Version: 13\r\n\
        \r\n";

    // The opening handshake.

    #[test]
    fn the_accept_key_of_the_rfc_6455_sample_matches() {
        assert_eq!(
            accept_key("dGhlIHNhbXBsZSBub25jZQ=="),
            "s3pPLMBiTxaQ9kYGzzhZRbK+xOo="
        );
    }

    #[test]
    fn a_client_offering_ridl_ws_v1_is_upgraded_with_it_selected() {
        let (mut server, client_end) = pair();
        let client = thread::spawn(move || connect(client_end, "ridl-ws-v1").map(|(_, p)| p));
        assert_eq!(handshake(&mut server, |_| true).unwrap(), Upgrade::Accepted);
        assert_eq!(
            client.join().unwrap().unwrap().as_deref(),
            Some("ridl-ws-v1")
        );
    }

    #[test]
    fn a_client_offering_ridl_ws_v1_second_is_still_upgraded() {
        let (mut server, client_end) = pair();
        let client =
            thread::spawn(move || connect(client_end, "ridl-ws-v2, ridl-ws-v1").map(|(_, p)| p));
        assert_eq!(handshake(&mut server, |_| true).unwrap(), Upgrade::Accepted);
        assert_eq!(
            client.join().unwrap().unwrap().as_deref(),
            Some("ridl-ws-v1")
        );
    }

    #[test]
    fn a_client_offering_no_subprotocol_gets_400() {
        let (mut server, client_end) = pair();
        let client = thread::spawn(move || connect(client_end, "").map(|(_, p)| p));
        assert_eq!(handshake(&mut server, |_| true).unwrap(), Upgrade::Rejected);
        match client.join().unwrap() {
            Err(tungstenite::Error::Http(response)) => assert_eq!(response.status(), 400),
            other => panic!("expected a 400 response, got {other:?}"),
        }
    }

    #[test]
    fn the_origin_check_sees_the_origin_value() {
        let head = GOOD_HEAD.replace("Host:", "Origin:  http://a.example \r\nHost:");
        let mut seen = None;
        let (upgrade, _) = handshake_checked(&head, |o| {
            seen = o.map(str::to_owned);
            true
        });
        assert_eq!(upgrade, Upgrade::Accepted);
        assert_eq!(seen.as_deref(), Some("http://a.example"));
    }

    #[test]
    fn a_refused_origin_gets_403() {
        let head = GOOD_HEAD.replace("Host:", "Origin: http://a.example\r\nHost:");
        assert_eq!(
            handshake_checked(&head, |_| false),
            (
                Upgrade::Forbidden,
                String::from_utf8(FORBIDDEN.to_vec()).unwrap()
            )
        );
    }

    #[test]
    fn a_malformed_request_gets_400_before_the_origin_check() {
        let head = GOOD_HEAD.replace("ridl-ws-v1", "ridl-ws-v2");
        let (upgrade, _) = handshake_checked(&head, |_| panic!("the check is not called"));
        assert_eq!(upgrade, Upgrade::Rejected);
    }

    #[test]
    fn a_request_with_two_origins_gets_400() {
        let head = GOOD_HEAD.replace(
            "Host:",
            "Origin: http://a.example\r\nOrigin: http://b.example\r\nHost:",
        );
        assert_eq!(handshake_checked(&head, |_| true).0, Upgrade::Rejected);
    }

    #[test]
    fn a_client_offering_only_another_subprotocol_gets_400() {
        let head = GOOD_HEAD.replace("ridl-ws-v1", "ridl-ws-v2");
        assert_eq!(
            handshake_on(&head),
            (
                Upgrade::Rejected,
                String::from_utf8(BAD_REQUEST.to_vec()).unwrap()
            )
        );
    }

    #[test]
    fn a_request_head_over_8_kib_gets_400() {
        let (mut server, mut client) = pair();
        let filler = "a".repeat(MAX_HEAD_BYTES);
        let head = GOOD_HEAD.replace("Host:", &format!("X-Filler: {filler}\r\nHost:"));
        let writer = thread::spawn(move || {
            client.write_all(head.as_bytes()).unwrap();
            let mut response = vec![0u8; BAD_REQUEST.len()];
            client.read_exact(&mut response).unwrap();
            response
        });
        assert_eq!(handshake(&mut server, |_| true).unwrap(), Upgrade::Rejected);
        // The server end stays open until the client has read the response:
        // closing a socket with unread input resets the connection.
        assert_eq!(writer.join().unwrap(), BAD_REQUEST);
        drop(server);
    }

    #[test]
    fn a_request_head_of_exactly_8_kib_is_upgraded_and_one_byte_more_is_not() {
        // 8 KiB is the limit, written out so that the test does not read
        // it from the constant under test.
        let pad = 8192 - GOOD_HEAD.len() - "X: \r\n".len();
        let head = GOOD_HEAD.replace("Host:", &format!("X: {}\r\nHost:", "a".repeat(pad)));
        assert_eq!(head.len(), 8192);
        assert_eq!(handshake_on(&head).0, Upgrade::Accepted);
        let head = GOOD_HEAD.replace("Host:", &format!("X: {}\r\nHost:", "a".repeat(pad + 1)));
        assert_eq!(handshake_on(&head).0, Upgrade::Rejected);
    }

    #[test]
    fn the_101_response_carries_the_accept_key_and_the_subprotocol() {
        let (upgrade, response) = handshake_on(GOOD_HEAD);
        assert_eq!(upgrade, Upgrade::Accepted);
        assert_eq!(
            response,
            "HTTP/1.1 101 Switching Protocols\r\n\
             Upgrade: websocket\r\n\
             Connection: Upgrade\r\n\
             Sec-WebSocket-Accept: s3pPLMBiTxaQ9kYGzzhZRbK+xOo=\r\n\
             Sec-WebSocket-Protocol: ridl-ws-v1\r\n\
             \r\n"
        );
    }

    #[test]
    fn a_malformed_or_incomplete_upgrade_request_gets_400() {
        let cases = [
            GOOD_HEAD.replace("GET ", "POST "),
            GOOD_HEAD.replace("HTTP/1.1", "HTTP/1.0"),
            GOOD_HEAD.replace("Upgrade: websocket\r\n", ""),
            GOOD_HEAD.replace("keep-alive, Upgrade", "keep-alive"),
            GOOD_HEAD.replace("Version: 13", "Version: 8"),
            GOOD_HEAD.replace("Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n", ""),
            GOOD_HEAD.replace(
                "Host:",
                "Sec-WebSocket-Key: AAAAAAAAAAAAAAAAAAAAAA==\r\nHost:",
            ),
            GOOD_HEAD.replace("Host: server.example.com", "no colon here"),
            GOOD_HEAD.replace("Host: server.example.com", "Host: a\r\n x: y"),
            GOOD_HEAD.replace("HTTP/1.1\r\n", "HTTP/1.1 extra\r\n"),
            GOOD_HEAD.replace("GET /chat ", "GET  "),
            GOOD_HEAD.replace("Key: dGhlIHNhbXBsZSBub25jZQ==", "Key: "),
        ];
        for head in cases {
            assert_eq!(handshake_on(&head).0, Upgrade::Rejected, "head {head:?}");
        }
    }

    #[test]
    fn a_request_without_a_host_header_gets_400() {
        let head = GOOD_HEAD.replace("Host: server.example.com\r\n", "");
        assert_eq!(
            handshake_on(&head),
            (
                Upgrade::Rejected,
                String::from_utf8(BAD_REQUEST.to_vec()).unwrap()
            )
        );
    }

    #[test]
    fn a_key_that_does_not_decode_to_16_bytes_gets_400() {
        let keys = [
            "AAAAAAAAAAAAAAAAAAAA",         // 15 bytes
            "AAAAAAAAAAAAAAAAAAAAAAAAAAAA", // 21 bytes
            "dGhlIHNhbXBsZSBub25jZQ!=",     // outside the alphabet
            "dGhlIHNhbXBsZSBub25jZQ=",      // not a multiple of 4
        ];
        for key in keys {
            let head = GOOD_HEAD.replace("dGhlIHNhbXBsZSBub25jZQ==", key);
            assert_eq!(handshake_on(&head).0, Upgrade::Rejected, "key {key:?}");
        }
    }

    #[test]
    fn a_version_other_than_13_gets_400_naming_version_13() {
        let expected = "HTTP/1.1 400 Bad Request\r\n\
            Sec-WebSocket-Version: 13\r\n\
            Connection: close\r\nContent-Length: 0\r\n\r\n";
        for head in [
            GOOD_HEAD.replace("Version: 13", "Version: 8"),
            GOOD_HEAD.replace("Sec-WebSocket-Version: 13\r\n", ""),
        ] {
            assert_eq!(
                handshake_on(&head),
                (Upgrade::Rejected, expected.to_owned()),
                "head {head:?}"
            );
        }
        // Any other refusal does not name the version.
        let head = GOOD_HEAD.replace("ridl-ws-v1", "ridl-ws-v2");
        assert!(!handshake_on(&head).1.contains("Sec-WebSocket-Version"));
    }

    #[test]
    fn header_names_and_tokens_ignore_ascii_case() {
        let head = GOOD_HEAD
            .replace("Upgrade: websocket", "UPGRADE: WebSocket")
            .replace("Connection: keep-alive, Upgrade", "connection: upgrade")
            .replace("Sec-WebSocket-Version", "sec-websocket-version");
        assert_eq!(handshake_on(&head).0, Upgrade::Accepted);
    }

    #[test]
    fn a_read_interrupted_once_is_retried() {
        struct InterruptedOnce<'a> {
            interrupted: bool,
            input: &'a [u8],
            output: Vec<u8>,
        }
        impl Read for InterruptedOnce<'_> {
            fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
                if !self.interrupted {
                    self.interrupted = true;
                    return Err(io::ErrorKind::Interrupted.into());
                }
                self.input.read(buf)
            }
        }
        impl Write for InterruptedOnce<'_> {
            fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
                self.output.write(buf)
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        let mut stream = InterruptedOnce {
            interrupted: false,
            input: GOOD_HEAD.as_bytes(),
            output: Vec::new(),
        };
        assert_eq!(handshake(&mut stream, |_| true).unwrap(), Upgrade::Accepted);
        assert!(stream.interrupted);
    }

    // Messages, over a real connection.

    #[test]
    fn a_ping_between_fragments_of_a_message_at_the_limit_is_answered_and_the_message_delivered() {
        let max = frame::MIN_MESSAGE_LIMIT;
        let (mut server, client) = upgraded(move |mut ws| {
            let binary = OpCode::Data(Data::Binary);
            let continuation = OpCode::Data(Data::Continue);
            ws.write(Message::Frame(Frame::message(
                vec![1u8; max - 4],
                binary,
                false,
            )))
            .unwrap();
            ws.write(Message::Frame(Frame::ping(vec![9u8; 20])))
                .unwrap();
            ws.write(Message::Frame(Frame::message(
                vec![2u8; 4],
                continuation,
                true,
            )))
            .unwrap();
            ws.flush().unwrap();
            loop {
                if let Message::Pong(payload) = ws.read().unwrap() {
                    return payload.to_vec();
                }
            }
        });
        let mut parser = Parser::new(max);
        let Ok(Event::Ping(payload)) = read_event(&mut server, &mut parser) else {
            panic!("expected a ping");
        };
        FrameWriter::default().pong(&mut server, &payload).unwrap();
        let mut expected = vec![1u8; max - 4];
        expected.extend([2u8; 4]);
        assert_eq!(
            read_event(&mut server, &mut parser),
            Ok(Event::Binary(expected))
        );
        assert_eq!(client.join().unwrap(), vec![9u8; 20]);
    }

    #[test]
    fn a_binary_message_round_trips() {
        let msg: Vec<u8> = (0..=255).cycle().take(70_000).collect();
        let sent = msg.clone();
        let (mut server, client) = upgraded(move |mut ws| {
            ws.send(Message::Binary(sent.into())).unwrap();
            match ws.read().unwrap() {
                Message::Binary(echo) => echo.to_vec(),
                other => panic!("expected a binary message, got {other:?}"),
            }
        });
        let mut parser = Parser::new(1 << 20);
        let Ok(Event::Binary(received)) = read_event(&mut server, &mut parser) else {
            panic!("expected a binary message");
        };
        assert_eq!(received, msg);
        FrameWriter::default()
            .binary(&mut server, &received)
            .unwrap();
        assert_eq!(client.join().unwrap(), msg);
    }

    #[test]
    fn a_message_split_across_reads_and_fragments_is_reassembled() {
        let (mut server, client) = upgraded(|mut ws| {
            let binary = OpCode::Data(Data::Binary);
            let continuation = OpCode::Data(Data::Continue);
            ws.write(Message::Frame(Frame::message(&b"abc"[..], binary, false)))
                .unwrap();
            ws.write(Message::Frame(Frame::ping(&b"between"[..])))
                .unwrap();
            ws.write(Message::Frame(Frame::message(
                &b"defg"[..],
                continuation,
                false,
            )))
            .unwrap();
            ws.write(Message::Frame(Frame::message(
                &b"h"[..],
                continuation,
                true,
            )))
            .unwrap();
            ws.flush().unwrap();
            ws
        });
        let mut parser = Parser::new(frame::MIN_MESSAGE_LIMIT);
        let mut events = Vec::new();
        let mut buf = [0u8; 256];
        while events.len() < 2 {
            let n = server.read(&mut buf).unwrap();
            assert_ne!(n, 0);
            for &byte in &buf[..n] {
                parser.feed(&[byte]);
                while let Some(event) = parser.next() {
                    events.push(event);
                }
            }
        }
        assert_eq!(
            events,
            [
                Ok(Event::Ping(b"between".to_vec())),
                Ok(Event::Binary(b"abcdefgh".to_vec()))
            ]
        );
        drop(client.join().unwrap());
    }

    #[test]
    fn a_text_message_closes_with_1003() {
        let (mut server, client) = upgraded(|mut ws| {
            ws.send(Message::Text("hello".into())).unwrap();
            client_close_code(ws)
        });
        let mut parser = Parser::new(frame::MIN_MESSAGE_LIMIT);
        let code = read_event(&mut server, &mut parser).unwrap_err();
        assert_eq!(code, CloseCode::UNSUPPORTED);
        FrameWriter::default().close(&mut server, code).unwrap();
        assert_eq!(client.join().unwrap(), Some(WsCloseCode::Unsupported));
    }

    #[test]
    fn an_unmasked_client_frame_closes_with_1002() {
        let (mut server, client) = upgraded(|mut ws| {
            ws.get_mut().write_all(&[0x82, 0x03, 1, 2, 3]).unwrap();
            client_close_code(ws)
        });
        let mut parser = Parser::new(frame::MIN_MESSAGE_LIMIT);
        let code = read_event(&mut server, &mut parser).unwrap_err();
        assert_eq!(code, CloseCode::PROTOCOL);
        FrameWriter::default().close(&mut server, code).unwrap();
        assert_eq!(client.join().unwrap(), Some(WsCloseCode::Protocol));
    }

    #[test]
    fn a_message_over_the_limit_closes_with_1009_before_it_is_buffered() {
        let (mut server, client) = upgraded(|mut ws| {
            ws.send(Message::Binary(
                vec![7u8; frame::MIN_MESSAGE_LIMIT + 1].into(),
            ))
            .unwrap();
            client_close_code(ws)
        });
        let mut parser = Parser::new(frame::MIN_MESSAGE_LIMIT);
        // Two bytes, a 16-bit length and a 4-byte mask: the header only.
        let mut header = [0u8; 8];
        server.read_exact(&mut header).unwrap();
        parser.feed(&header);
        assert_eq!(parser.next(), Some(Err(CloseCode::TOO_BIG)));
        FrameWriter::default()
            .close(&mut server, CloseCode::TOO_BIG)
            .unwrap();
        assert_eq!(client.join().unwrap(), Some(WsCloseCode::Size));
    }

    #[test]
    fn a_ping_is_answered_with_a_pong_carrying_its_payload() {
        let (mut server, client) = upgraded(|mut ws| {
            ws.send(Message::Ping(b"are you there"[..].into())).unwrap();
            loop {
                if let Message::Pong(payload) = ws.read().unwrap() {
                    return payload.to_vec();
                }
            }
        });
        let mut parser = Parser::new(frame::MIN_MESSAGE_LIMIT);
        let Ok(Event::Ping(payload)) = read_event(&mut server, &mut parser) else {
            panic!("expected a ping");
        };
        FrameWriter::default().pong(&mut server, &payload).unwrap();
        assert_eq!(client.join().unwrap(), b"are you there");
    }

    #[test]
    fn a_close_frame_is_answered_with_a_close_frame() {
        let (mut server, client) = upgraded(|mut ws| {
            ws.close(Some(CloseFrame {
                code: WsCloseCode::Normal,
                reason: "done".into(),
            }))
            .unwrap();
            loop {
                match ws.read() {
                    Ok(_) => continue,
                    Err(e) => return e,
                }
            }
        });
        let mut parser = Parser::new(frame::MIN_MESSAGE_LIMIT);
        assert_eq!(
            read_event(&mut server, &mut parser),
            Ok(Event::Close(Some(1000)))
        );
        FrameWriter::default()
            .close(&mut server, CloseCode::NORMAL)
            .unwrap();
        assert_eq!(parser.next(), None);
        // The server closes the connection after the close handshake (RFC 6455
        // §7.1.1), and `tungstenite` returns `ConnectionClosed` only when the
        // close handshake is complete and the connection is closed.
        drop(server);
        assert!(matches!(
            client.join().unwrap(),
            tungstenite::Error::ConnectionClosed
        ));
    }

    #[test]
    fn a_server_ping_is_answered_by_the_client() {
        let (mut server, client) = upgraded(|mut ws| {
            // `tungstenite` queues the pong during a read and sends it on
            // the next write or flush.
            let _ = ws.read();
            ws.flush().unwrap();
            ws
        });
        FrameWriter::default().ping(&mut server).unwrap();
        // Unblock the client's read with a message after the ping.
        FrameWriter::default().binary(&mut server, b"x").unwrap();
        let mut parser = Parser::new(frame::MIN_MESSAGE_LIMIT);
        assert_eq!(read_event(&mut server, &mut parser), Ok(Event::Pong));
        drop(client.join().unwrap());
    }

    // Frames, from hand-written bytes.

    #[test]
    fn a_frame_fed_in_pieces_is_returned_only_when_whole() {
        let bytes = masked(0x82, b"hello");
        let mut parser = Parser::new(frame::MIN_MESSAGE_LIMIT);
        for &byte in &bytes[..bytes.len() - 1] {
            parser.feed(&[byte]);
            assert_eq!(parser.next(), None);
        }
        parser.feed(&bytes[bytes.len() - 1..]);
        assert_eq!(parser.next(), Some(Ok(Event::Binary(b"hello".to_vec()))));
        assert_eq!(parser.next(), None);
    }

    #[test]
    fn several_frames_in_one_read_are_returned_in_order() {
        let mut bytes = masked(0x82, b"one");
        bytes.extend(masked(0x89, b""));
        bytes.extend(masked(0x8A, b"x"));
        bytes.extend(masked(0x82, &[0u8; 300]));
        assert_eq!(
            parse_all(1000, &bytes),
            [
                Ok(Event::Binary(b"one".to_vec())),
                Ok(Event::Ping(Vec::new())),
                Ok(Event::Pong),
                Ok(Event::Binary(vec![0u8; 300])),
            ]
        );
    }

    #[test]
    fn a_64_bit_length_is_read() {
        let payload = vec![9u8; 70_000];
        assert_eq!(
            parse_all(1 << 20, &masked(0x82, &payload)),
            [Ok(Event::Binary(payload))]
        );
    }

    #[test]
    fn a_message_exactly_at_the_limit_is_accepted() {
        let max = frame::MIN_MESSAGE_LIMIT;
        let mut bytes = masked(0x02, &vec![1u8; max - 4]);
        bytes.extend(masked(0x80, &[2u8; 4]));
        let mut expected = vec![1u8; max - 4];
        expected.extend([2u8; 4]);
        assert_eq!(parse_all(max, &bytes), [Ok(Event::Binary(expected))]);
    }

    #[test]
    fn fragments_that_together_pass_the_limit_close_with_1009() {
        let max = frame::MIN_MESSAGE_LIMIT;
        let mut bytes = masked(0x02, &vec![1u8; max - 4]);
        // Only the header of the second fragment: the length decides.
        bytes.extend(&masked(0x80, &[2u8; 5])[..6]);
        assert_eq!(parse_all(max, &bytes), [Err(CloseCode::TOO_BIG)]);
    }

    #[test]
    fn frame_sequence_violations_close_with_1002() {
        let cases: [(&str, Vec<u8>); 11] = [
            ("a continuation with no message", masked(0x80, b"x")),
            (
                "a binary frame inside a message",
                [masked(0x02, b"a"), masked(0x82, b"b")].concat(),
            ),
            (
                "a text frame inside a message",
                [masked(0x02, b"a"), masked(0x81, b"b")].concat(),
            ),
            ("reserved bit 1 set", masked(0xC2, b"x")),
            ("reserved bit 2 set", masked(0xA2, b"x")),
            ("reserved bit 3 set", masked(0x92, b"x")),
            ("a reserved opcode", masked(0x83, b"x")),
            ("a reserved control opcode", masked(0x8B, b"")),
            ("a fragmented ping", masked(0x09, b"x")),
            ("a control frame over 125 bytes", masked(0x89, &[0u8; 126])),
            ("a length with its top bit set", {
                let mut b = vec![0x82, 0x80 | 127, 0x80, 0, 0, 0, 0, 0, 0, 1];
                b.extend([0u8; 4]);
                b
            }),
        ];
        for (name, bytes) in cases {
            assert_eq!(
                parse_all(1 << 20, &bytes),
                [Err(CloseCode::PROTOCOL)],
                "{name}"
            );
        }
    }

    #[test]
    fn close_codes_are_accepted_inside_each_range_and_refused_outside_it() {
        for code in [1000u16, 1003, 1007, 1014, 3000, 4999] {
            let bytes = masked(0x88, &code.to_be_bytes());
            assert_eq!(
                parse_all(1000, &bytes),
                [Ok(Event::Close(Some(code)))],
                "{code}"
            );
        }
        for code in [999u16, 1004, 1006, 1015, 2999, 5000] {
            let bytes = masked(0x88, &code.to_be_bytes());
            assert_eq!(
                parse_all(1000, &bytes),
                [Err(CloseCode::PROTOCOL)],
                "{code}"
            );
        }
    }

    /// A writer that counts its `write` calls and keeps what they wrote.
    #[derive(Default)]
    struct Counting {
        writes: usize,
        bytes: Vec<u8>,
    }

    impl Write for Counting {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.writes += 1;
            self.bytes.extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    /// Each frame is one `write` call, header and payload together, so that
    /// no frame waits on Nagle's algorithm between its two halves.
    #[test]
    fn each_frame_is_written_with_one_write_call() {
        // One writer for every frame: its buffer is reused, and a frame
        // after a larger one carries none of the larger one's bytes.
        let mut fw = FrameWriter::default();
        let payloads: [&[u8]; 3] = [&[5u8; 200], &[6u8; 0x1_0000], b""];
        for payload in payloads {
            let mut w = Counting::default();
            fw.binary(&mut w, payload).unwrap();
            assert_eq!(w.writes, 1, "a payload of {} bytes", payload.len());
            assert!(w.bytes.ends_with(payload));
        }
        let mut w = Counting::default();
        fw.ping(&mut w).unwrap();
        fw.pong(&mut w, b"p").unwrap();
        fw.close(&mut w, CloseCode::GOING_AWAY).unwrap();
        assert_eq!(w.writes, 3);
        assert_eq!(
            w.bytes,
            [0x89, 0x00, 0x8A, 0x01, b'p', 0x88, 0x02, 0x03, 0xE9]
        );
    }

    #[test]
    fn a_125_byte_ping_is_accepted() {
        let bytes = masked(0x89, &[4u8; 125]);
        assert_eq!(parse_all(1000, &bytes), [Ok(Event::Ping(vec![4u8; 125]))]);
    }

    #[test]
    fn a_close_after_a_large_first_fragment_is_a_close_not_1009() {
        let max = frame::MIN_MESSAGE_LIMIT;
        let mut bytes = masked(0x02, &vec![1u8; max - 1]);
        bytes.extend(masked(0x88, &1000u16.to_be_bytes()));
        assert_eq!(parse_all(max, &bytes), [Ok(Event::Close(Some(1000)))]);
    }

    #[test]
    fn close_payloads_are_checked() {
        assert_eq!(
            parse_all(1000, &masked(0x88, b"")),
            [Ok(Event::Close(None))]
        );
        assert_eq!(
            parse_all(1000, &masked(0x88, &[0x0F, 0xA0])),
            [Ok(Event::Close(Some(4000)))]
        );
        let invalid: [&[u8]; 4] = [&[0x03], &[0x03, 0xEE], &[0x03, 0xED], &[0x03, 0xE8, 0xFF]];
        for payload in invalid {
            assert_eq!(
                parse_all(1000, &masked(0x88, payload)),
                [Err(CloseCode::PROTOCOL)],
                "{payload:?}"
            );
        }
    }

    #[test]
    fn nothing_is_returned_after_an_error_or_a_close() {
        let mut bytes = masked(0x81, b"text");
        bytes.extend(masked(0x82, b"after"));
        assert_eq!(parse_all(1000, &bytes), [Err(CloseCode::UNSUPPORTED)]);
        let mut bytes = masked(0x88, b"");
        bytes.extend(masked(0x82, b"after"));
        assert_eq!(parse_all(1000, &bytes), [Ok(Event::Close(None))]);
    }

    #[test]
    fn a_finished_parser_releases_its_bytes_and_keeps_no_more() {
        let mut bytes = masked(0x02, b"open");
        bytes.extend(masked(0x88, b""));
        bytes.extend(masked(0x82, b"after"));
        let mut parser = Parser::new(1000);
        parser.feed(&bytes);
        assert_eq!(parser.next(), Some(Ok(Event::Close(None))));
        assert!(parser.buf.is_empty() && parser.message.is_none());
        parser.feed(&[0u8; 100]);
        assert!(parser.buf.is_empty());
    }

    #[test]
    fn server_frames_are_unmasked_and_use_the_shortest_length() {
        let mut out = Vec::new();
        FrameWriter::default()
            .binary(&mut out, &[5u8; 125])
            .unwrap();
        assert_eq!(out[..2], [0x82, 125]);
        out.clear();
        FrameWriter::default()
            .binary(&mut out, &[5u8; 126])
            .unwrap();
        assert_eq!(out[..4], [0x82, 126, 0, 126]);
        out.clear();
        FrameWriter::default()
            .binary(&mut out, &[5u8; 0x1_0000])
            .unwrap();
        assert_eq!(out[..10], [0x82, 127, 0, 0, 0, 0, 0, 1, 0, 0]);
        out.clear();
        FrameWriter::default().ping(&mut out).unwrap();
        FrameWriter::default().pong(&mut out, b"p").unwrap();
        FrameWriter::default()
            .close(&mut out, CloseCode::GOING_AWAY)
            .unwrap();
        assert_eq!(out, [0x89, 0, 0x8A, 1, b'p', 0x88, 2, 0x03, 0xE9]);
    }
}
