//! The one scenario both rigs run: read the `fixed` and the signal, subscribe
//! the event, send the command, observe its effects, and send the query.

use std::time::{Duration, Instant};

use ridl_rt::port::{Caller, EventSource, FixedReader, SignalReader};
use ridl_rt::sample::Provenance;

use super::provider::{COMMAND, EVENT, FIXED, FIXED_VALUE, IFACE, QUERY, SIGNAL};

/// How long the scenario waits for one condition before it fails.
pub const LIMIT: Duration = Duration::from_secs(2);

/// The command's argument.
const X: &[u8] = &[0x2a, 0x07];
/// The query's argument, and the reply the provider computes from it.
const Y: &[u8] = &[0x01, 0x80, 0xff];
const Y_REPLY: &[u8] = &[0x02, 0x81, 0x00];

/// What the scenario runs over: the consumer's ports, and a way to let time
/// pass so that the provider and the transport make progress.
pub trait Rig {
    /// The consumer's ports.
    type Ports: SignalReader + EventSource + Caller + FixedReader;
    /// The consumer's ports.
    fn ports(&mut self) -> &mut Self::Ports;
    /// Lets the provider and the transport make progress.
    fn step(&mut self);
}

/// Calls `check` until it returns `Some`, with one `step` between two calls.
/// Fails the test when `LIMIT` of wall time passes first.
fn wait<R: Rig, T>(
    rig: &mut R,
    what: &str,
    mut check: impl FnMut(&mut R::Ports) -> Option<T>,
) -> T {
    let start = Instant::now();
    loop {
        if let Some(v) = check(rig.ports()) {
            return v;
        }
        assert!(start.elapsed() < LIMIT, "{what} within {LIMIT:?}");
        rig.step();
    }
}

/// Runs the scenario over `rig`.
pub fn run<R: Rig>(rig: &mut R) {
    let mut out = [0u8; 64];

    // 1. The `fixed` is the provisioned value.
    let n = rig
        .ports()
        .read_fixed(IFACE, FIXED, &mut out)
        .expect("the fixed is read");
    assert_eq!(&out[..n], FIXED_VALUE);

    // 2. The signal is `Init`, with no bytes.
    let sample = rig.ports().read(IFACE, SIGNAL, &mut out).expect("read");
    assert_eq!(sample.provenance, Provenance::Init);
    assert_eq!(sample.len, 0);
    assert_eq!(sample.envelope.seq, 0);

    // 3. Subscribe the event.
    rig.ports()
        .subscribe(IFACE, &[EVENT])
        .expect("the event is subscribed");

    // 4. The command is accepted.
    let c = rig
        .ports()
        .command(IFACE, COMMAND, X, None)
        .expect("the command is sent");
    let ack = wait(rig, "the command's acknowledgment", |p| p.ack(c));
    assert_eq!(ack, Ok(()));
    rig.ports().forget(c);

    // 5. The provider set the signal to `X` and raised the event with `X`.
    let sample = wait(rig, "the signal is Live", |p| {
        let s = p.read(IFACE, SIGNAL, &mut out).expect("read");
        (s.provenance == Provenance::Live).then_some(s)
    });
    assert_eq!(&out[..sample.len], X);
    assert!(sample.envelope.seq >= 1);
    let occurrence = wait(rig, "the occurrence", |p| p.next(&mut out).expect("next"));
    assert_eq!((occurrence.iface, occurrence.ord), (IFACE, EVENT));
    assert_eq!(&out[..occurrence.len], X);
    rig.step();
    assert_eq!(rig.ports().next(&mut out).expect("next"), None);

    // 6. The query's reply is the provider's function of `Y`.
    let c = rig
        .ports()
        .query(IFACE, QUERY, Y, None)
        .expect("the query is sent");
    let reply = wait(rig, "the query's reply", |p| {
        p.reply(c, &mut out).expect("reply")
    });
    let n = reply.expect("the query is answered");
    assert_eq!(&out[..n], Y_REPLY);
    rig.ports().forget(c);
    // 7. A rejected payload keeps X, and a later good payload restores Live.
    fn reject_ff(bytes: &[u8]) -> Result<(), ridl_rt::sample::Detection> {
        if bytes.first() == Some(&0xFF) {
            Err(ridl_rt::sample::Detection::InvalidValue(
                ridl_rt::payload::Violation {
                    type_name: Some("Conformance"),
                    rule: ridl_rt::payload::Rule::Range,
                },
            ))
        } else {
            Ok(())
        }
    }
    rig.ports().check_with(IFACE, SIGNAL, reject_ff);
    let c = rig
        .ports()
        .command(IFACE, COMMAND, &[0xFF], None)
        .expect("command sent");
    assert_eq!(
        wait(rig, "the bad command's acknowledgment", |p| p.ack(c)),
        Ok(())
    );
    rig.ports().forget(c);
    let detection = Provenance::Invalid(ridl_rt::sample::Cause::Detected(
        reject_ff(&[0xFF]).unwrap_err(),
    ));
    let sample = wait(rig, "the rejected signal is Invalid(Detected)", |p| {
        let s = p.read(IFACE, SIGNAL, &mut out).expect("read");
        (s.provenance == detection).then_some(s)
    });
    assert_eq!(&out[..sample.len], X);
    let c = rig
        .ports()
        .command(IFACE, COMMAND, X, None)
        .expect("command sent");
    assert_eq!(
        wait(rig, "the good command's acknowledgment", |p| p.ack(c)),
        Ok(())
    );
    rig.ports().forget(c);
    let sample = wait(rig, "the signal recovers to Live", |p| {
        let s = p.read(IFACE, SIGNAL, &mut out).expect("read");
        (s.provenance == Provenance::Live).then_some(s)
    });
    assert_eq!(&out[..sample.len], X);
}
