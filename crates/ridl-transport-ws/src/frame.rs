//! The header codec of the WebSocket binding (frame specification §11.1).
//!
//! One frame is one WebSocket message. [`decode`] reads one message without
//! any session context and [`Frame::encode`] writes one. Every integer is
//! fixed-width little-endian, a payload is every byte after the header, and
//! whether a frame carries a payload is decided from its header alone. The
//! rules of the payload are in the types: only [`PublishState::Live`] and
//! [`ResponseOutcome::Reply`] hold payload bytes among the states that can
//! lack them.
//!
//! The module needs `core` only.

use core::str;

use ridl_rt::contract::{CatalogHash, InterfaceNo, Kind, Ordinal};
use ridl_rt::error::Contract;
use ridl_rt::payload::{Rule, Violation};
use ridl_rt::sample::{Envelope, Timestamp};

/// The frame version this binding carries.
pub const FRAME_VERSION: u32 = 1;
/// The WebSocket subprotocol name of binding version 1.
pub const SUBPROTOCOL: &str = "ridl-ws-v1";
/// The smallest message size limit a receiver may configure: the largest
/// header of any message, an `attach` with a 255-byte name.
pub const MIN_MESSAGE_LIMIT: usize = 294;
/// The size of an encoded [`Envelope`] in bytes.
pub const ENVELOPE_BYTES: usize = 16;

const FORM_ATTACH: u8 = 1;
const FORM_ATTACHED: u8 = 2;
const FORM_SUBSCRIBE: u8 = 3;
const FORM_ANSWER: u8 = 4;
const FORM_UNSUBSCRIBE: u8 = 5;
const FORM_READ: u8 = 6;
const FORM_PUBLISH: u8 = 7;
const FORM_OCCURRENCE: u8 = 8;
const FORM_REQUEST: u8 = 9;
const FORM_RESPONSE: u8 = 10;

const OUT_ACCEPTED: u8 = 1;
const OUT_REPLY: u8 = 2;
const OUT_INVALID_VALUE: u8 = 3;
const OUT_PRECONDITION_FAILED: u8 = 4;
const OUT_CONTRACT_BROKEN: u8 = 5;
const OUT_UNKNOWN_INTERACTION: u8 = 6;
const OUT_CORRUPT: u8 = 7;
const OUT_BUSY: u8 = 8;
const OUT_REFUSED: u8 = 9;

const PROV_INIT: u8 = 1;
const PROV_LIVE: u8 = 2;
const PROV_INVALID_DECLARED: u8 = 3;

/// The size of the header fields common to every form but `attach` and
/// `attached`: `form`, `kind`, `interface`, `ordinal`.
const COMMON: usize = 10;
const ATTACHED_LEN: usize = 3;
const ANSWER_LEN: usize = COMMON + 2;
const PUBLISH_LEN: usize = COMMON + ENVELOPE_BYTES + 1;
const DATA_LEN: usize = COMMON + ENVELOPE_BYTES;
const RESPONSE_LEN: usize = COMMON + ENVELOPE_BYTES + 8 + 2;
/// `form`, `frame_version`, `encoding`, `hash` and `name_len`.
const ATTACH_FIXED: usize = 1 + 4 + 1 + 32 + 1;

/// The interaction a frame is about.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Target {
    /// The interaction kind.
    pub kind: Kind,
    /// The interface number.
    pub iface: InterfaceNo,
    /// The member ordinal.
    pub ord: Ordinal,
}

/// The reason an [`Attach`] cannot be built.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AttachError {
    /// The name has no bytes.
    Empty,
    /// The name is longer than 255 bytes of UTF-8.
    TooLong {
        /// The length of the name in bytes.
        len: usize,
    },
    /// The encoding tag is not 1, 2 or 3.
    UnknownEncoding(u8),
}

/// An `attach` frame: a consumer's request to open a session.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Attach<'a> {
    frame_version: u32,
    encoding: u8,
    hash: CatalogHash,
    name: &'a str,
}

impl<'a> Attach<'a> {
    /// Builds an `attach`. The name must hold 1 to 255 bytes and the encoding
    /// tag must be one of the table of [`encoding_tag`].
    pub fn new(
        frame_version: u32,
        encoding: u8,
        hash: CatalogHash,
        name: &'a str,
    ) -> Result<Self, AttachError> {
        if !(1..=3).contains(&encoding) {
            return Err(AttachError::UnknownEncoding(encoding));
        }
        match name.len() {
            0 => Err(AttachError::Empty),
            1..=255 => Ok(Attach {
                frame_version,
                encoding,
                hash,
                name,
            }),
            len => Err(AttachError::TooLong { len }),
        }
    }

    /// The frame version the consumer speaks.
    pub fn frame_version(&self) -> u32 {
        self.frame_version
    }

    /// The encoding tag the consumer asks for.
    pub fn encoding(&self) -> u8 {
        self.encoding
    }

    /// The catalog hash.
    pub fn hash(&self) -> &CatalogHash {
        &self.hash
    }

    /// The unit name.
    pub fn name(&self) -> &'a str {
        self.name
    }
}

/// Why a provider refused an `attach`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reason {
    /// The provider serves no catalog of that name.
    UnknownCatalog,
    /// The catalog hash differs from the provider's.
    CatalogMismatch,
    /// The provider does not serve the encoding.
    EncodingUnsupported,
    /// The provider does not speak the frame version.
    FrameVersionUnsupported,
    /// A catalog number does not fit the binding's range.
    Range,
}

/// The outcome carried by an `attached` frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AttachOutcome {
    /// The session is open.
    Accepted,
    /// The provider refused the `attach`.
    Refused(Reason),
}

/// The outcome carried by an `answer` frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnswerOutcome {
    /// The provider accepted a `subscribe`.
    Accepted,
    /// The provider refused a `subscribe` or a `read`.
    Contract(Contract),
}

/// The provenance of a `publish`, with the payload that only `Live` carries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PublishState<'a> {
    /// No publication yet.
    Init,
    /// The latest publication, with its payload (which may be empty).
    Live(&'a [u8]),
    /// The channel is invalid for its declared cause.
    InvalidDeclared,
}

/// The outcome of a `response`, with the payload that only `Reply` carries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResponseOutcome<'a> {
    /// A command was accepted.
    Accepted,
    /// A query's reply, with its payload (which may be empty).
    Reply(&'a [u8]),
    /// The call failed with a contract error.
    Contract(Contract),
    /// The provider found the request's payload malformed.
    Corrupt,
    /// The provider refused the call at admission.
    Busy,
}

/// One decoded frame. Payloads and the unit name borrow from the message.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Frame<'a> {
    /// Consumer to provider: opens the session.
    Attach(Attach<'a>),
    /// Provider to consumer: accepts or refuses the `attach`.
    Attached(AttachOutcome),
    /// Consumer to provider: starts delivery of a signal or an event.
    Subscribe(Target),
    /// Consumer to provider: stops delivery.
    Unsubscribe(Target),
    /// Consumer to provider: asks for a signal's state or a `fixed`'s value.
    Read(Target),
    /// Provider to consumer: accepts or refuses a `subscribe` or a `read`.
    Answer {
        /// The interaction answered.
        target: Target,
        /// The outcome.
        outcome: AnswerOutcome,
    },
    /// Provider to consumer: one publication of a signal or a `fixed`.
    Publish {
        /// The interaction published.
        target: Target,
        /// The sender's envelope.
        envelope: Envelope,
        /// The provenance and, for `Live`, the payload.
        state: PublishState<'a>,
    },
    /// Provider to consumer: one occurrence of an event.
    Occurrence {
        /// The event.
        target: Target,
        /// The sender's envelope.
        envelope: Envelope,
        /// The payload, possibly empty.
        payload: &'a [u8],
    },
    /// Consumer to provider: one call.
    Request {
        /// The command or query.
        target: Target,
        /// The caller's envelope.
        envelope: Envelope,
        /// The arguments, possibly empty.
        payload: &'a [u8],
    },
    /// Provider to consumer: the acknowledgment or the reply of a call.
    Response {
        /// The command or query.
        target: Target,
        /// The provider's envelope.
        envelope: Envelope,
        /// The `seq` of the request answered.
        correlation: u64,
        /// The outcome and, for `Reply`, the payload.
        outcome: ResponseOutcome<'a>,
    },
}

/// The output buffer given to [`Frame::encode`] is too small.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Short {
    /// The number of bytes the frame needs.
    pub needed: usize,
}

/// A message the receiver cannot read: the receiver records the frame and
/// closes the connection with code 1002.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProtocolViolation {
    /// The message is empty or shorter than its header.
    TooShort,
    /// A control-plane message other than `attach` is longer than its header.
    ControlTooLong,
    /// A `form`, `kind`, `provenance`, `outcome` or `detail` is outside its
    /// table.
    UnknownCode,
    /// The `outcome` is in the table but not allowed on that form.
    OutcomeNotAllowed,
    /// A `detail` that must be 0 is not.
    NonzeroDetail,
    /// An `interface` or an `ordinal` is 0.
    ZeroNumber,
    /// An `attach`'s `name_len` is 0 or does not match the message.
    BadNameLength,
    /// An `attach`'s `name` is not valid UTF-8.
    BadName,
}

/// A frame the receiver records and drops; the session continues.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Discarded {
    /// A `publish` or a `response` whose header says it carries no payload
    /// has bytes after its header.
    UnexpectedPayload,
}

/// The reason [`decode`] returned no frame.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DecodeError {
    /// A protocol violation: close the connection with 1002.
    Violation(ProtocolViolation),
    /// A frame to record and drop.
    Discard(Discarded),
}

impl From<ProtocolViolation> for DecodeError {
    fn from(v: ProtocolViolation) -> Self {
        DecodeError::Violation(v)
    }
}

/// The encoding tag of an encoding name: `flatbuffers` 1, `proto3` 2,
/// `podbuf` 3.
pub fn encoding_tag(name: &str) -> Option<u8> {
    match name {
        "flatbuffers" => Some(1),
        "proto3" => Some(2),
        "podbuf" => Some(3),
        _ => None,
    }
}

fn kind_code(kind: Kind) -> u8 {
    kind as u8
}

fn kind_from(code: u8) -> Result<Kind, ProtocolViolation> {
    match code {
        1 => Ok(Kind::Signal),
        2 => Ok(Kind::Event),
        3 => Ok(Kind::Command),
        4 => Ok(Kind::Query),
        5 => Ok(Kind::Fixed),
        _ => Err(ProtocolViolation::UnknownCode),
    }
}

/// The `detail` code of a violation's rule: `Range` 1, `Step` 2, `Length` 3,
/// `Pattern` 4, `Variant` 5, `Unique` 6 (frame specification §9.6), or
/// `None` for a `Rule` variant frame version 1 has no code for. `Rule` is
/// `#[non_exhaustive]`, and a new variant comes only with a new frame
/// version, so a sender built against a newer `ridl-rt` checks here before
/// it builds a `contract(InvalidValue)` outcome: the server sends such a
/// violation as `corrupt` on a `response` and as `contract(ContractBroken)`
/// on an `answer`, which has no `corrupt` outcome.
pub fn rule_tag(rule: Rule) -> Option<u8> {
    match rule {
        Rule::Range => Some(1),
        Rule::Step => Some(2),
        Rule::Length => Some(3),
        Rule::Pattern => Some(4),
        Rule::Variant => Some(5),
        Rule::Unique => Some(6),
        _ => None,
    }
}

/// The `detail` byte written for a rule: its tag, or 0 for a rule with no
/// tag, which no receiver accepts (`detail` 0 under `contract(InvalidValue)`
/// is an unknown code). The codec does not panic on it; the sender keeps
/// such a rule off the wire with [`rule_tag`].
fn rule_code(rule: Rule) -> u8 {
    rule_tag(rule).unwrap_or(0)
}

fn rule_from(code: u8) -> Result<Rule, ProtocolViolation> {
    match code {
        1 => Ok(Rule::Range),
        2 => Ok(Rule::Step),
        3 => Ok(Rule::Length),
        4 => Ok(Rule::Pattern),
        5 => Ok(Rule::Variant),
        6 => Ok(Rule::Unique),
        _ => Err(ProtocolViolation::UnknownCode),
    }
}

fn reason_code(reason: Reason) -> u8 {
    match reason {
        Reason::UnknownCatalog => 1,
        Reason::CatalogMismatch => 2,
        Reason::EncodingUnsupported => 3,
        Reason::FrameVersionUnsupported => 4,
        Reason::Range => 5,
    }
}

fn reason_from(code: u8) -> Result<Reason, ProtocolViolation> {
    match code {
        1 => Ok(Reason::UnknownCatalog),
        2 => Ok(Reason::CatalogMismatch),
        3 => Ok(Reason::EncodingUnsupported),
        4 => Ok(Reason::FrameVersionUnsupported),
        5 => Ok(Reason::Range),
        _ => Err(ProtocolViolation::UnknownCode),
    }
}

/// The `(outcome, detail)` pair of a contract error.
fn contract_codes(contract: Contract) -> (u8, u8) {
    match contract {
        Contract::InvalidValue(v) => (OUT_INVALID_VALUE, rule_code(v.rule)),
        Contract::PreconditionFailed => (OUT_PRECONDITION_FAILED, 0),
        Contract::ContractBroken => (OUT_CONTRACT_BROKEN, 0),
        Contract::UnknownInteraction => (OUT_UNKNOWN_INTERACTION, 0),
    }
}

/// A contract error from an `outcome` code in 3 to 6 and its `detail`.
fn contract_from(outcome: u8, detail: u8) -> Result<Contract, ProtocolViolation> {
    if outcome == OUT_INVALID_VALUE {
        return Ok(Contract::InvalidValue(Violation {
            type_name: None,
            rule: rule_from(detail)?,
        }));
    }
    if detail != 0 {
        return Err(ProtocolViolation::NonzeroDetail);
    }
    match outcome {
        OUT_PRECONDITION_FAILED => Ok(Contract::PreconditionFailed),
        OUT_CONTRACT_BROKEN => Ok(Contract::ContractBroken),
        _ => Ok(Contract::UnknownInteraction),
    }
}

/// Checks the `outcome` code against the table and returns an error that
/// tells a code outside the table from one that the form does not allow.
fn outcome_known(outcome: u8) -> Result<(), ProtocolViolation> {
    if (OUT_ACCEPTED..=OUT_REFUSED).contains(&outcome) {
        Ok(())
    } else {
        Err(ProtocolViolation::UnknownCode)
    }
}

fn u32_at(msg: &[u8], at: usize) -> u32 {
    let mut b = [0u8; 4];
    b.copy_from_slice(&msg[at..at + 4]);
    u32::from_le_bytes(b)
}

fn u64_at(msg: &[u8], at: usize) -> u64 {
    let mut b = [0u8; 8];
    b.copy_from_slice(&msg[at..at + 8]);
    u64::from_le_bytes(b)
}

/// Reads the common 10 bytes. The caller has checked the length.
fn target_at(msg: &[u8]) -> Result<Target, ProtocolViolation> {
    let kind = kind_from(msg[1])?;
    let iface = u32_at(msg, 2);
    let ord = u32_at(msg, 6);
    if iface == 0 || ord == 0 {
        return Err(ProtocolViolation::ZeroNumber);
    }
    Ok(Target {
        kind,
        iface: InterfaceNo(iface),
        ord: Ordinal(ord),
    })
}

fn envelope_at(msg: &[u8], at: usize) -> Envelope {
    Envelope {
        stamp: Timestamp(u64_at(msg, at) as i64),
        seq: u64_at(msg, at + 8),
    }
}

/// Requires a message of at least `len` bytes.
fn need(msg: &[u8], len: usize) -> Result<(), ProtocolViolation> {
    if msg.len() < len {
        Err(ProtocolViolation::TooShort)
    } else {
        Ok(())
    }
}

/// Requires a control-plane message of exactly `len` bytes.
fn control(msg: &[u8], len: usize) -> Result<(), ProtocolViolation> {
    need(msg, len)?;
    if msg.len() > len {
        Err(ProtocolViolation::ControlTooLong)
    } else {
        Ok(())
    }
}

/// Decodes one message. The function reads the message without session
/// context: the order of frames within a session is the session's to check.
pub fn decode(msg: &[u8]) -> Result<Frame<'_>, DecodeError> {
    let Some(&form) = msg.first() else {
        return Err(ProtocolViolation::TooShort.into());
    };
    Ok(match form {
        FORM_ATTACH => decode_attach(msg)?,
        FORM_ATTACHED => {
            control(msg, ATTACHED_LEN)?;
            outcome_known(msg[1])?;
            let outcome = match msg[1] {
                OUT_ACCEPTED if msg[2] == 0 => AttachOutcome::Accepted,
                OUT_ACCEPTED => return Err(ProtocolViolation::NonzeroDetail.into()),
                OUT_REFUSED => AttachOutcome::Refused(reason_from(msg[2])?),
                _ => return Err(ProtocolViolation::OutcomeNotAllowed.into()),
            };
            Frame::Attached(outcome)
        }
        FORM_SUBSCRIBE | FORM_UNSUBSCRIBE | FORM_READ => {
            control(msg, COMMON)?;
            let target = target_at(msg)?;
            match form {
                FORM_SUBSCRIBE => Frame::Subscribe(target),
                FORM_UNSUBSCRIBE => Frame::Unsubscribe(target),
                _ => Frame::Read(target),
            }
        }
        FORM_ANSWER => {
            control(msg, ANSWER_LEN)?;
            let target = target_at(msg)?;
            let (code, detail) = (msg[10], msg[11]);
            outcome_known(code)?;
            let outcome = match code {
                OUT_ACCEPTED if detail == 0 => AnswerOutcome::Accepted,
                OUT_ACCEPTED => return Err(ProtocolViolation::NonzeroDetail.into()),
                OUT_INVALID_VALUE..=OUT_UNKNOWN_INTERACTION => {
                    AnswerOutcome::Contract(contract_from(code, detail)?)
                }
                _ => return Err(ProtocolViolation::OutcomeNotAllowed.into()),
            };
            Frame::Answer { target, outcome }
        }
        FORM_PUBLISH => {
            need(msg, PUBLISH_LEN)?;
            let target = target_at(msg)?;
            let envelope = envelope_at(msg, COMMON);
            let rest = &msg[PUBLISH_LEN..];
            let state = match msg[PUBLISH_LEN - 1] {
                PROV_INIT => PublishState::Init,
                PROV_LIVE => PublishState::Live(rest),
                PROV_INVALID_DECLARED => PublishState::InvalidDeclared,
                _ => return Err(ProtocolViolation::UnknownCode.into()),
            };
            if !matches!(state, PublishState::Live(_)) && !rest.is_empty() {
                return Err(DecodeError::Discard(Discarded::UnexpectedPayload));
            }
            Frame::Publish {
                target,
                envelope,
                state,
            }
        }
        FORM_OCCURRENCE | FORM_REQUEST => {
            need(msg, DATA_LEN)?;
            let target = target_at(msg)?;
            let envelope = envelope_at(msg, COMMON);
            let payload = &msg[DATA_LEN..];
            if form == FORM_OCCURRENCE {
                Frame::Occurrence {
                    target,
                    envelope,
                    payload,
                }
            } else {
                Frame::Request {
                    target,
                    envelope,
                    payload,
                }
            }
        }
        FORM_RESPONSE => {
            need(msg, RESPONSE_LEN)?;
            let target = target_at(msg)?;
            let envelope = envelope_at(msg, COMMON);
            let correlation = u64_at(msg, COMMON + ENVELOPE_BYTES);
            let (code, detail) = (msg[RESPONSE_LEN - 2], msg[RESPONSE_LEN - 1]);
            let rest = &msg[RESPONSE_LEN..];
            outcome_known(code)?;
            let outcome = match code {
                OUT_REFUSED => return Err(ProtocolViolation::OutcomeNotAllowed.into()),
                OUT_INVALID_VALUE..=OUT_UNKNOWN_INTERACTION => {
                    ResponseOutcome::Contract(contract_from(code, detail)?)
                }
                _ if detail != 0 => return Err(ProtocolViolation::NonzeroDetail.into()),
                OUT_ACCEPTED => ResponseOutcome::Accepted,
                OUT_REPLY => ResponseOutcome::Reply(rest),
                OUT_CORRUPT => ResponseOutcome::Corrupt,
                _ => ResponseOutcome::Busy,
            };
            if !matches!(outcome, ResponseOutcome::Reply(_)) && !rest.is_empty() {
                return Err(DecodeError::Discard(Discarded::UnexpectedPayload));
            }
            Frame::Response {
                target,
                envelope,
                correlation,
                outcome,
            }
        }
        _ => return Err(ProtocolViolation::UnknownCode.into()),
    })
}

fn decode_attach(msg: &[u8]) -> Result<Frame<'_>, DecodeError> {
    need(msg, ATTACH_FIXED)?;
    let name_len = msg[ATTACH_FIXED - 1] as usize;
    if name_len == 0 || msg.len() != ATTACH_FIXED + name_len {
        return Err(ProtocolViolation::BadNameLength.into());
    }
    let name = str::from_utf8(&msg[ATTACH_FIXED..]).map_err(|_| ProtocolViolation::BadName)?;
    if !(1..=3).contains(&msg[5]) {
        return Err(ProtocolViolation::UnknownCode.into());
    }
    let mut hash = [0u8; 32];
    hash.copy_from_slice(&msg[6..38]);
    Ok(Frame::Attach(Attach {
        frame_version: u32_at(msg, 1),
        encoding: msg[5],
        hash: CatalogHash(hash),
        name,
    }))
}

/// A cursor over the output buffer. The caller has checked the length.
struct Writer<'o> {
    out: &'o mut [u8],
    at: usize,
}

impl Writer<'_> {
    fn bytes(&mut self, b: &[u8]) {
        self.out[self.at..self.at + b.len()].copy_from_slice(b);
        self.at += b.len();
    }

    fn u8(&mut self, v: u8) {
        self.bytes(&[v]);
    }

    fn target(&mut self, form: u8, t: Target) {
        self.u8(form);
        self.u8(kind_code(t.kind));
        self.bytes(&t.iface.0.to_le_bytes());
        self.bytes(&t.ord.0.to_le_bytes());
    }

    fn envelope(&mut self, e: Envelope) {
        self.bytes(&e.stamp.0.to_le_bytes());
        self.bytes(&e.seq.to_le_bytes());
    }
}

impl Frame<'_> {
    /// The number of bytes [`Frame::encode`] writes.
    pub fn encoded_len(&self) -> usize {
        match self {
            Frame::Attach(a) => ATTACH_FIXED + a.name.len(),
            Frame::Attached(_) => ATTACHED_LEN,
            Frame::Subscribe(_) | Frame::Unsubscribe(_) | Frame::Read(_) => COMMON,
            Frame::Answer { .. } => ANSWER_LEN,
            Frame::Publish { state, .. } => {
                PUBLISH_LEN
                    + match state {
                        PublishState::Live(p) => p.len(),
                        _ => 0,
                    }
            }
            Frame::Occurrence { payload, .. } | Frame::Request { payload, .. } => {
                DATA_LEN + payload.len()
            }
            Frame::Response { outcome, .. } => {
                RESPONSE_LEN
                    + match outcome {
                        ResponseOutcome::Reply(p) => p.len(),
                        _ => 0,
                    }
            }
        }
    }

    /// Writes the frame at the start of `out` and returns the number of bytes
    /// written. When `out` is too small, nothing is written and the error
    /// holds the length the frame needs.
    ///
    /// A `contract(InvalidValue)` outcome whose `Rule` has no tag
    /// ([`rule_tag`] is `None`) is written with `detail` 0, which a receiver
    /// refuses as an unknown code; check the rule before building the frame.
    pub fn encode(&self, out: &mut [u8]) -> Result<usize, Short> {
        let needed = self.encoded_len();
        if out.len() < needed {
            return Err(Short { needed });
        }
        let mut w = Writer { out, at: 0 };
        match *self {
            Frame::Attach(a) => {
                w.u8(FORM_ATTACH);
                w.bytes(&a.frame_version.to_le_bytes());
                w.u8(a.encoding);
                w.bytes(&a.hash.0);
                // The constructor and the decoder keep the length in 1..=255.
                w.u8(a.name.len() as u8);
                w.bytes(a.name.as_bytes());
            }
            Frame::Attached(outcome) => {
                w.u8(FORM_ATTACHED);
                match outcome {
                    AttachOutcome::Accepted => {
                        w.u8(OUT_ACCEPTED);
                        w.u8(0);
                    }
                    AttachOutcome::Refused(reason) => {
                        w.u8(OUT_REFUSED);
                        w.u8(reason_code(reason));
                    }
                }
            }
            Frame::Subscribe(t) => w.target(FORM_SUBSCRIBE, t),
            Frame::Unsubscribe(t) => w.target(FORM_UNSUBSCRIBE, t),
            Frame::Read(t) => w.target(FORM_READ, t),
            Frame::Answer { target, outcome } => {
                w.target(FORM_ANSWER, target);
                let (code, detail) = match outcome {
                    AnswerOutcome::Accepted => (OUT_ACCEPTED, 0),
                    AnswerOutcome::Contract(c) => contract_codes(c),
                };
                w.u8(code);
                w.u8(detail);
            }
            Frame::Publish {
                target,
                envelope,
                state,
            } => {
                w.target(FORM_PUBLISH, target);
                w.envelope(envelope);
                match state {
                    PublishState::Init => w.u8(PROV_INIT),
                    PublishState::Live(p) => {
                        w.u8(PROV_LIVE);
                        w.bytes(p);
                    }
                    PublishState::InvalidDeclared => w.u8(PROV_INVALID_DECLARED),
                }
            }
            Frame::Occurrence {
                target,
                envelope,
                payload,
            } => {
                w.target(FORM_OCCURRENCE, target);
                w.envelope(envelope);
                w.bytes(payload);
            }
            Frame::Request {
                target,
                envelope,
                payload,
            } => {
                w.target(FORM_REQUEST, target);
                w.envelope(envelope);
                w.bytes(payload);
            }
            Frame::Response {
                target,
                envelope,
                correlation,
                outcome,
            } => {
                w.target(FORM_RESPONSE, target);
                w.envelope(envelope);
                w.bytes(&correlation.to_le_bytes());
                let (code, detail) = match outcome {
                    ResponseOutcome::Accepted => (OUT_ACCEPTED, 0),
                    ResponseOutcome::Reply(_) => (OUT_REPLY, 0),
                    ResponseOutcome::Contract(c) => contract_codes(c),
                    ResponseOutcome::Corrupt => (OUT_CORRUPT, 0),
                    ResponseOutcome::Busy => (OUT_BUSY, 0),
                };
                w.u8(code);
                w.u8(detail);
                if let ResponseOutcome::Reply(p) = outcome {
                    w.bytes(p);
                }
            }
        }
        Ok(w.at)
    }
}

#[cfg(test)]
mod encoding_tag_tests {
    use super::encoding_tag;

    #[test]
    fn the_encoding_tag_of_podbuf_is_3() {
        assert_eq!(encoding_tag("podbuf"), Some(3));
        assert_eq!(encoding_tag("repr-c"), None);
    }
}
