//! The server's configuration and the notes it records.

use std::fmt;
use std::sync::Arc;
use std::time::Duration;

use crate::frame::{ProtocolViolation, Target, MIN_MESSAGE_LIMIT};

/// What the server records about a session: frames it dropped and events
/// that no frame reports to the client. Recording is through
/// [`Config::with_record`]; without a hook nothing is recorded.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Note {
    /// A message the codec could not read. The session closes with 1002.
    Violation(ProtocolViolation),
    /// A message that is not an `attach` arrived before the `attach`. The
    /// session closes with 1002.
    MessageBeforeAttach,
    /// A second `attach` arrived on an open session. The session closes
    /// with 1002.
    SecondAttach,
    /// A provider-to-consumer form arrived from the client, with or without
    /// bytes after its header that a consumer would discard it for. The
    /// session closes with 1002.
    UnexpectedDirection,
    /// An `unsubscribe` named a kind the form does not admit. The frame is
    /// dropped.
    KindNotAdmitted {
        /// The interaction named.
        target: Target,
    },
    /// A value the backend holds does not fit one message under the limit of
    /// [`Config::max_message_bytes`]. Nothing is sent for it.
    ValueTooLarge {
        /// The interaction whose value did not fit.
        target: Target,
        /// The bytes the value needs.
        needed: usize,
    },
    /// A `request` repeated a `seq` the session has seen and is not answering
    /// again. The frame is dropped.
    DuplicateDropped {
        /// The repeated `seq`.
        seq: u64,
    },
    /// A call whose outcome stayed unknown for [`Config::call_abandon_after`]
    /// was forgotten on the backend. Nothing is sent for it.
    CallAbandoned {
        /// The `seq` of the abandoned request.
        seq: u64,
    },
    /// The backend reported a `contract(InvalidValue)` whose `Rule` the
    /// frame has no code for (frame specification §9.6), from a `ridl-rt`
    /// newer than this crate. A `response` is sent as `corrupt` and an
    /// `answer` as `contract(ContractBroken)` instead.
    RuleNotCarried {
        /// The interaction answered.
        target: Target,
    },
    /// A `subscribe`, `read`, `request` or `unsubscribe` named an interaction
    /// the member table does not hold, or holds under another kind (frame
    /// specification §6.4). An `unsubscribe` is dropped; the other three are
    /// answered `contract(UnknownInteraction)`.
    UnknownInteraction {
        /// The interaction named.
        target: Target,
    },
}

/// A `max_message_bytes` below [`MIN_MESSAGE_LIMIT`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LimitTooSmall {
    /// The limit asked for.
    pub given: usize,
}

impl fmt::Display for LimitTooSmall {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "a message limit of {} bytes is below the smallest allowed, {} bytes",
            self.given, MIN_MESSAGE_LIMIT
        )
    }
}

impl std::error::Error for LimitTooSmall {}

/// The hook the server calls with each [`Note`].
pub(crate) type Record = Arc<dyn Fn(&Note) + Send + Sync>;

/// The hook the handshake calls with the request's `Origin` header.
pub(crate) type OriginCheck = Arc<dyn Fn(Option<&str>) -> bool + Send + Sync>;

/// The settings of one server.
#[derive(Clone)]
pub struct Config {
    /// How often the backend is polled: once per interval, whatever the
    /// client's message rate. A socket read waits until the next poll is
    /// due, so a read waits at most this long.
    pub poll_interval: Duration,
    /// How long between two pings.
    ///
    /// Set it below [`Config::liveness_timeout`]. With a `ping_interval` at
    /// or above it, the session of an idle client ends at the liveness
    /// timeout before the first ping is sent, because a client that is not
    /// pinged sends nothing (a browser sends no ping of its own).
    pub ping_interval: Duration,
    /// How long without any message from the client before the session is
    /// closed.
    pub liveness_timeout: Duration,
    /// How long a call whose outcome stays unknown is kept on the backend
    /// before it is forgotten.
    ///
    /// A zero duration forgets every call whose outcome is not known on the
    /// first poll after the request, so that call gets no `response`.
    pub call_abandon_after: Duration,
    max_message_bytes: usize,
    record: Option<Record>,
    origin_check: Option<OriginCheck>,
}

impl Default for Config {
    /// 20 ms between polls, a ping every 5 s, a liveness timeout of 15 s, a
    /// call abandoned after 60 s, a message limit of 1 MiB, nothing
    /// recorded, and no origin check.
    fn default() -> Self {
        Config {
            poll_interval: Duration::from_millis(20),
            ping_interval: Duration::from_secs(5),
            liveness_timeout: Duration::from_secs(15),
            call_abandon_after: Duration::from_secs(60),
            max_message_bytes: 1024 * 1024,
            record: None,
            origin_check: None,
        }
    }
}

impl Config {
    /// The largest message, in bytes, the server reads or sends.
    pub fn max_message_bytes(&self) -> usize {
        self.max_message_bytes
    }

    /// Sets the largest message the server reads or sends. A limit below
    /// [`MIN_MESSAGE_LIMIT`] is refused: no `attach` could be read under it.
    pub fn with_max_message_bytes(mut self, n: usize) -> Result<Self, LimitTooSmall> {
        if n < MIN_MESSAGE_LIMIT {
            return Err(LimitTooSmall { given: n });
        }
        self.max_message_bytes = n;
        Ok(self)
    }

    /// Sets the hook the server calls with each [`Note`].
    pub fn with_record(mut self, f: impl Fn(&Note) + Send + Sync + 'static) -> Self {
        self.record = Some(Arc::new(f));
        self
    }

    /// The recording hook, for the session.
    pub(crate) fn record(&self) -> Option<Record> {
        self.record.clone()
    }

    /// Sets the check the opening handshake applies to the request's
    /// `Origin` header. The handshake calls `f` with the header's value, or
    /// with `None` when the request has no `Origin` header, once the request
    /// is otherwise one the server upgrades. When `f` returns `false`, the
    /// request is answered `403 Forbidden` and [`serve`](crate::serve)
    /// returns [`Ended::Rejected`](crate::Ended::Rejected).
    ///
    /// Without a check, every `Origin` is accepted. A browser sends the
    /// origin of the page that opens the WebSocket, and any web page can open
    /// one to a local address, so a server that a browser reaches should set
    /// a check. A client that is not a browser sends any `Origin` it chooses,
    /// or none: the check is not authentication.
    pub fn with_origin_check(
        mut self,
        f: impl Fn(Option<&str>) -> bool + Send + Sync + 'static,
    ) -> Self {
        self.origin_check = Some(Arc::new(f));
        self
    }

    /// Whether the origin check, when one is set, accepts `origin`.
    pub(crate) fn origin_accepted(&self, origin: Option<&str>) -> bool {
        self.origin_check.as_ref().is_none_or(|f| f(origin))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_durations_keep_the_ping_before_the_liveness_timeout() {
        let config = Config::default();
        assert_eq!(config.poll_interval, Duration::from_millis(20));
        assert_eq!(config.ping_interval, Duration::from_secs(5));
        assert_eq!(config.liveness_timeout, Duration::from_secs(15));
        assert_eq!(config.call_abandon_after, Duration::from_secs(60));
        assert!(config.ping_interval < config.liveness_timeout);
    }

    #[test]
    fn the_default_limit_is_one_mebibyte() {
        assert_eq!(Config::default().max_message_bytes(), 1024 * 1024);
    }

    #[test]
    fn a_limit_below_the_minimum_is_refused() {
        let err = Config::default()
            .with_max_message_bytes(MIN_MESSAGE_LIMIT - 1)
            .err();
        assert_eq!(
            err,
            Some(LimitTooSmall {
                given: MIN_MESSAGE_LIMIT - 1
            })
        );
    }

    #[test]
    fn the_minimum_limit_is_accepted() {
        let config = Config::default()
            .with_max_message_bytes(MIN_MESSAGE_LIMIT)
            .expect("accepted");
        assert_eq!(config.max_message_bytes(), MIN_MESSAGE_LIMIT);
    }
}
