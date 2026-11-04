//! Reversed-role negotiation and attach.

use ridl_rt::encoding::Encoding;
use ridl_rt::port::{Attached, Encoded};

use std::time::Instant;

use super::session::Step;
use super::{
    limit_short, linger, run_upgraded, timed_out, write_failed, Config, Connection, Deadline,
    Driver, Ended, Note, ProvidedBackend,
};
use crate::frame::{self, Attach};
use crate::frame::{AttachOutcome, Frame, FRAME_VERSION};
use crate::ws::{self, FrameWriter, Upgrade};
use crate::ws::{HttpRefusal, Role};

/// Why a providing-session selector refused the request target.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectionError {
    /// The target is not served; answers HTTP 404.
    NotFound,
    /// The target conflicts with an active attachment; answers HTTP 409.
    Conflict,
}

/// Consumes a remote provider over a connection the embedder accepted.
///
/// Accepts only `ridl-ws-v2-provider`, ignores the URL path, and sends
/// `attach` first. The peer must answer `attached`. The blocking, timeout,
/// framing and cleanup rules of [`serve`](super::serve) apply. This entry point currently
/// negotiates and attaches only; it forwards no session data.
pub fn serve_provided<C: Connection, B: ProvidedBackend>(
    conn: C,
    backend: B,
    config: &Config,
) -> Ended {
    serve_provided_selected(conn, |_| Ok(backend), config)
}

/// Selects a local providing backend from the unchanged HTTP request target.
///
/// Origin and role checks precede the one selector invocation. Selection
/// errors answer 404 or 409; insufficient message capacity answers 400 and
/// returns [`Ended::ConfigRefused`], before upgrade. The selected backend is
/// dropped on every exit. The client checks the catalog and encoding in the
/// server's `attach`; refusal ends the session with close 1000.
pub fn serve_provided_selected<C, B, S>(mut conn: C, select: S, config: &Config) -> Ended
where
    C: Connection,
    B: ProvidedBackend,
    S: FnOnce(&str) -> Result<B, SelectionError>,
{
    if let Err(e) = conn.set_write_timeout(Some(config.liveness_timeout)) {
        return Ended::Io(e.kind());
    }
    let mut backend = None;
    let mut attach = None;
    let mut config_refused = false;
    let upgraded = ws::handshake_selected(
        &mut Deadline {
            conn: &mut conn,
            at: Instant::now() + config.liveness_timeout,
        },
        Role::Providing,
        FRAME_VERSION,
        |origin| config.origin_accepted(origin),
        |path| {
            let mut selected = select(path).map_err(|error| match error {
                SelectionError::NotFound => HttpRefusal::NotFound,
                SelectionError::Conflict => HttpRefusal::Conflict,
            })?;
            if let Some((target, needed)) =
                limit_short(selected.members(), config.max_message_bytes())
            {
                if let Some(record) = config.record() {
                    record(&Note::MessageLimitTooSmall { target, needed });
                }
                config_refused = true;
                return Err(HttpRefusal::Capacity);
            }
            let catalog = *selected.ports().catalog();
            let encoding = frame::encoding_tag(<B::Ports as Encoded>::Encoding::NAME);
            attach = Some(configured_attach(catalog, encoding).inspect_err(|_| {
                config_refused = true;
            })?);
            backend = Some(selected);
            Ok(())
        },
    );
    match upgraded {
        Ok(Upgrade::Accepted(_)) => {}
        Ok(Upgrade::Rejected | Upgrade::Forbidden) => {
            linger(&mut conn);
            return if config_refused {
                Ended::ConfigRefused
            } else {
                Ended::Rejected
            };
        }
        Err(e) if timed_out(&e) => return Ended::LivenessTimeout,
        Err(e) => return Ended::Io(e.kind()),
    }
    let frame = Frame::Attach(attach.expect("upgrade has an attach"));
    let mut bytes = vec![0; frame.encoded_len()];
    frame.encode(&mut bytes).expect("attach fits");
    if let Err(e) = FrameWriter::default().binary(&mut conn, &bytes) {
        return write_failed(&e);
    }
    let mut session = Provided {
        _backend: backend.expect("upgrade selected a backend"),
        attached: false,
    };
    run_upgraded(&mut conn, &mut session, config)
}

fn configured_attach(
    catalog: ridl_rt::contract::CatalogRef,
    encoding: Option<u8>,
) -> Result<Attach<'static>, HttpRefusal> {
    encoding
        .and_then(|encoding| Attach::new(FRAME_VERSION, encoding, catalog.hash, catalog.name).ok())
        .ok_or(HttpRefusal::Config)
}

struct Provided<B> {
    _backend: B,
    attached: bool,
}

impl<B: ProvidedBackend> Driver for Provided<B> {
    fn on_message(
        &mut self,
        msg: &[u8],
        _: Instant,
        _: &mut Vec<Vec<u8>>,
        config: &Config,
    ) -> Step {
        match frame::decode(msg) {
            Ok(Frame::Attached(AttachOutcome::Accepted)) if !self.attached => {
                self.attached = true;
                Step::Continue
            }
            Ok(Frame::Attached(AttachOutcome::Refused(_))) if !self.attached => Step::Close(1000),
            other => {
                if let Some(record) = config.record() {
                    record(&match other {
                        Err(frame::DecodeError::Violation(v)) => Note::Violation(v),
                        _ => Note::UnexpectedDirection,
                    });
                }
                Step::Close(1002)
            }
        }
    }
    fn tick(&mut self, _: Instant, _: &mut Vec<Vec<u8>>) -> Step {
        Step::Continue
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ridl_rt::contract::{CatalogHash, CatalogRef};

    #[test]
    fn unencodable_attach_is_a_configuration_refusal() {
        let hash = CatalogHash([7; 32]);
        for (name, encoding) in [("", Some(1)), ("valid", None)] {
            assert!(matches!(
                configured_attach(CatalogRef { name, hash }, encoding),
                Err(HttpRefusal::Config)
            ));
        }
        assert!(configured_attach(
            CatalogRef {
                name: "valid",
                hash
            },
            Some(1)
        )
        .is_ok());
    }
}
