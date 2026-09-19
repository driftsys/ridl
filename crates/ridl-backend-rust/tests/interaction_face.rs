//! The interaction-face round trip (Lane M stage M3, Tasks 4 and 5,
//! `docs/design/interaction-face.md`; the approved design is
//! `docs/archive/2026-09-16-interaction-face-v0-design.md` §7).
//!
//! Task 4 exercises the disposable loopback ports of `tests/support/loopback.rs`
//! directly, against `ridl-rt`'s trait contracts, ahead of the checked-in
//! generated face that Task 5 adds. A test name containing `support` isolates
//! this task's tests: `cargo test -p ridl-backend-rust --test interaction_face
//! support`.
//!
//! Task 5 brings in the checked-in generated face of `tests/generated/`, adds
//! the hand-written `Payload<ReprC>` implementations the fixture's types need
//! to compile (design §2 — a throwaway stand-in; E11.7/E11.8/E11.12 replace
//! it), and runs the round trip: a signal publish and read, an event raise and
//! receive, a command's acknowledgment, a query's reply, a failing `require`,
//! a failing `ensure`, the settlement count when one claim's settlement fails
//! and a later one succeeds, and dispatch's short-buffer behavior.

mod support;

use ridl_rt::contract::{InterfaceNo, Ordinal};
use ridl_rt::port::{Caller, Clock, EventSink, EventSource, Handler, SignalReader, SignalWriter};
use ridl_rt::sample::{Duration, Provenance};

use generated::Inner;
use support::loopback::Loopback;

const IFACE: InterfaceNo = InterfaceNo(1);
const ORD: Ordinal = Ordinal(1);

#[test]
fn support_command_is_delivered_and_acknowledged() {
    let mut lb = Loopback::new("face.demo");
    let correlation = lb.command(IFACE, ORD, &[1, 2, 3]).expect("command sent");

    assert_eq!(lb.ack(correlation), None, "not yet settled");

    let mut buf = [0u8; 8];
    let claim = lb
        .next_claim(&mut buf)
        .expect("next_claim")
        .expect("a claim is waiting");
    assert_eq!(claim.iface, IFACE);
    assert_eq!(claim.ord, ORD);
    assert_eq!(&buf[..claim.len], &[1, 2, 3]);

    lb.settle(claim.id, Ok(&[])).expect("settle succeeds");
    assert_eq!(
        lb.ack(correlation),
        Some(Ok(())),
        "settlement is observable through ack"
    );
}

#[test]
fn support_query_is_delivered_and_replied() {
    let mut lb = Loopback::new("face.demo");
    let correlation = lb.query(IFACE, ORD, &[9]).expect("query sent");

    let mut buf = [0u8; 8];
    let claim = lb
        .next_claim(&mut buf)
        .expect("next_claim")
        .expect("a claim is waiting");
    assert_eq!(&buf[..claim.len], &[9]);

    lb.settle(claim.id, Ok(&[7, 7])).expect("settle succeeds");

    let mut out = [0u8; 8];
    let reply = lb.reply(correlation, &mut out).expect("reply read");
    let Some(Ok(len)) = reply else {
        panic!("expected a successful reply, got {reply:?}");
    };
    assert_eq!(&out[..len], &[7, 7]);
    // A query's correlation always answers `None` from `ack` (`Caller::ack`'s
    // own documentation).
    assert_eq!(lb.ack(correlation), None);
}

#[test]
fn support_settle_can_be_made_to_fail_once_then_succeed() {
    // Task 5 asserts that `dispatch`'s returned count only advances past a
    // settlement the handler accepted; this proves the loopback can inject
    // that one failure ahead of a later, successful claim.
    let mut lb = Loopback::new("face.demo");
    let correlation = lb.command(IFACE, ORD, &[1]).expect("command sent");
    let mut buf = [0u8; 8];
    let claim = lb
        .next_claim(&mut buf)
        .expect("next_claim")
        .expect("a claim is waiting");

    lb.fail_next_settle();
    assert!(
        lb.settle(claim.id, Ok(&[])).is_err(),
        "the injected failure surfaces from settle"
    );
    assert_eq!(
        lb.ack(correlation),
        None,
        "a failed settle records no outcome"
    );

    lb.settle(claim.id, Ok(&[]))
        .expect("the next settle succeeds");
    assert_eq!(lb.ack(correlation), Some(Ok(())));
}

#[test]
fn support_signal_publish_and_read_round_trips() {
    let mut lb = Loopback::new("face.demo");
    lb.set(IFACE, ORD, &[42]).expect("set staged");
    lb.commit();

    let mut out = [0u8; 8];
    let raw = lb.read(IFACE, ORD, &mut out).expect("read");
    assert_eq!(raw.provenance, Provenance::Live);
    assert_eq!(&out[..raw.len], &[42]);
}

#[test]
fn support_event_raise_and_receive_round_trips() {
    let mut lb = Loopback::new("face.demo");
    lb.subscribe(IFACE, &[ORD]).expect("subscribe");
    lb.raise(IFACE, ORD, &[5, 6]).expect("raise");

    let mut out = [0u8; 8];
    let occurrence = lb
        .next(&mut out)
        .expect("next")
        .expect("an occurrence is waiting");
    assert_eq!(occurrence.iface, IFACE);
    assert_eq!(occurrence.ord, ORD);
    assert_eq!(&out[..occurrence.len], &[5, 6]);
}

#[test]
fn support_clock_is_hand_driven_not_wall_clock() {
    // Two independently constructed ports must start at the same logical time
    // regardless of when, in real time, each was constructed — a clock that
    // reads wall-clock time could not guarantee that.
    let first = Loopback::new("face.demo");
    std::thread::sleep(std::time::Duration::from_millis(5));
    let second = Loopback::new("face.demo");
    assert_eq!(
        first.now(),
        second.now(),
        "the clock must not read wall-clock time"
    );

    let mut lb = second;
    let before = lb.now();
    lb.advance(Duration(1_000));
    let after = lb.now();
    assert_eq!(
        after.0,
        before.0 + 1_000,
        "advance moves the hand-driven clock by exactly the given amount"
    );
}

// ---------------------------------------------------------------------------
// Task 5: the checked-in generated face, the hand-written payloads, and the
// round trip.
// ---------------------------------------------------------------------------

/// The checked-in output of `generate_face` over `tests/fixtures/interaction_face.ridl`.
/// It is `include!`d, never hand-edited; the regeneration guard below compares
/// it byte-for-byte against a fresh `generate_face` call. Lint allows live on
/// this wrapper module, as outer attributes, because the generated file itself
/// carries no inner attribute (design §7): an inner attribute inside an
/// `include!`d file is a hard error.
#[allow(
    dead_code,
    reason = "not every generated item is named from this file's own tests; \
              the byte-equality guard is what pins the emitter's output"
)]
#[allow(
    clippy::derivable_impls,
    reason = "the domain-type Default emission (crate::defaults, predating M3) writes a manual \
              impl rather than #[derive(Default)]; this is the first place that output is \
              compiled in-tree, so it is the first place this lint sees it. Fixing the emitter \
              is outside Lane M stage M3's scope: it is baseline domain-type emission every \
              backend consumer shares, not face- or descriptor-specific."
)]
#[allow(
    clippy::upper_case_acronyms,
    reason = "an enum variant keeps its typl SCREAMING_SNAKE spelling by design \
              (crate::emit_enum's own doc comment), predating M3; this file is the first place \
              that spelling is compiled in-tree"
)]
mod generated {
    include!("generated/interaction_face.rs");

    /// Reads an `i64`-backed scalar through a shared reference.
    ///
    /// The generated `get(self)` takes the value by move, and the generated
    /// type carries no `Copy` derive until Task 6 of the typl value objects
    /// plan lands, so a `Payload::encode(&self)` or a `Provider` method that
    /// receives `&Level` cannot call it. This module is the type's defining
    /// module, where the private field is visible, which is why the bridge
    /// lives here. Task 6 replaces every `inner()` call with `get()` and
    /// removes this trait.
    pub trait Inner {
        fn inner(&self) -> i64;
    }

    macro_rules! inner_i64 {
        ($($ty:ident),* $(,)?) => {
            $(impl Inner for $ty {
                fn inner(&self) -> i64 {
                    self.0
                }
            })*
        };
    }

    inner_i64!(Temperature, Level, Window, Average);
}

/// Hand-written `Payload<ReprC>` implementations for the fixture's restricted
/// fixed-size scalar, enum, and struct types (design §2). This is a throwaway
/// stand-in with no schema and no dependency: E11.7, E11.8, or E11.12 replaces
/// it with a generated codec. The wire layout is 8 little-endian bytes per
/// `i64`-backed scalar or enum, and a struct's fields back to back in
/// declaration order.
mod payloads {
    use ridl_rt::encoding::ReprC;
    use ridl_rt::payload::{
        EncodeError, Encoded, Malformed, Payload, Ref, Rule, VerifyError, Violation,
    };

    use super::generated::{Average, Health, Inner, Level, Temperature, Warning, Window};

    /// A named scalar backed by `i64`, with its declared closed range
    /// (typl §5.5: both bounds inclusive) checked by `verify`.
    macro_rules! scalar_payload {
        ($ty:ty, $name:literal, $min:expr, $max:expr) => {
            impl Payload<ReprC> for $ty {
                const MAX_SIZE: usize = 8;
                type View<'a> = &'a [u8];

                fn encode<'o>(
                    &self,
                    out: &'o mut [u8],
                ) -> Result<Encoded<'o, &'o [u8]>, EncodeError> {
                    if out.len() < 8 {
                        return Err(EncodeError::Capacity {
                            needed: 8,
                            available: out.len(),
                        });
                    }
                    out[..8].copy_from_slice(&self.inner().to_le_bytes());
                    let bytes = &out[..8];
                    Ok(Encoded { bytes, view: bytes })
                }

                fn verify(buf: &[u8]) -> Result<&[u8], VerifyError> {
                    if buf.len() != 8 {
                        return Err(VerifyError::Structure(Malformed::OutOfBounds));
                    }
                    let value = i64::from_le_bytes(buf.try_into().expect("checked length"));
                    if !($min..=$max).contains(&value) {
                        return Err(VerifyError::Contract(Violation {
                            type_name: $name,
                            rule: Rule::Range,
                        }));
                    }
                    Ok(buf)
                }

                fn decode(r: Ref<'_, Self, ReprC>) -> Self {
                    let bytes = r.bytes();
                    Self::new_unchecked(i64::from_le_bytes(
                        bytes.try_into().expect("verified length"),
                    ))
                }
            }
        };
    }

    scalar_payload!(Temperature, "Temperature", -40, 85);
    scalar_payload!(Level, "Level", 0, 100);
    scalar_payload!(Window, "Window", 0, 100_000);
    scalar_payload!(Average, "Average", 0, 1000);

    fn health_discriminant(value: &Health) -> i64 {
        match value {
            Health::OK => 0,
            Health::WARN => 1,
            Health::FAIL => 2,
        }
    }

    fn health_from_discriminant(value: i64) -> Health {
        match value {
            0 => Health::OK,
            1 => Health::WARN,
            _ => Health::FAIL,
        }
    }

    impl Payload<ReprC> for Health {
        const MAX_SIZE: usize = 8;
        type View<'a> = &'a [u8];

        fn encode<'o>(&self, out: &'o mut [u8]) -> Result<Encoded<'o, &'o [u8]>, EncodeError> {
            if out.len() < 8 {
                return Err(EncodeError::Capacity {
                    needed: 8,
                    available: out.len(),
                });
            }
            out[..8].copy_from_slice(&health_discriminant(self).to_le_bytes());
            let bytes = &out[..8];
            Ok(Encoded { bytes, view: bytes })
        }

        fn verify(buf: &[u8]) -> Result<&[u8], VerifyError> {
            if buf.len() != 8 {
                return Err(VerifyError::Structure(Malformed::OutOfBounds));
            }
            let value = i64::from_le_bytes(buf.try_into().expect("checked length"));
            if !(0..=2).contains(&value) {
                return Err(VerifyError::Contract(Violation {
                    type_name: "Health",
                    rule: Rule::Variant,
                }));
            }
            Ok(buf)
        }

        fn decode(r: Ref<'_, Self, ReprC>) -> Self {
            let bytes = r.bytes();
            health_from_discriminant(i64::from_le_bytes(
                bytes.try_into().expect("verified length"),
            ))
        }
    }

    impl Payload<ReprC> for Warning {
        const MAX_SIZE: usize = 16;
        type View<'a> = &'a [u8];

        fn encode<'o>(&self, out: &'o mut [u8]) -> Result<Encoded<'o, &'o [u8]>, EncodeError> {
            if out.len() < 16 {
                return Err(EncodeError::Capacity {
                    needed: 16,
                    available: out.len(),
                });
            }
            out[..8].copy_from_slice(&self.code.inner().to_le_bytes());
            out[8..16].copy_from_slice(&health_discriminant(&self.health).to_le_bytes());
            let bytes = &out[..16];
            Ok(Encoded { bytes, view: bytes })
        }

        fn verify(buf: &[u8]) -> Result<&[u8], VerifyError> {
            if buf.len() != 16 {
                return Err(VerifyError::Structure(Malformed::OutOfBounds));
            }
            // Each field's own check, so a violation names the field's own
            // type — `Level` or `Health` — not `Warning` (`Violation` names
            // "the typl type whose constraint failed").
            Level::verify(&buf[..8])?;
            Health::verify(&buf[8..16])?;
            Ok(buf)
        }

        fn decode(r: Ref<'_, Self, ReprC>) -> Self {
            let bytes = r.bytes();
            let code = Level::new_unchecked(i64::from_le_bytes(
                bytes[..8].try_into().expect("verified length"),
            ));
            let health = health_from_discriminant(i64::from_le_bytes(
                bytes[8..16].try_into().expect("verified length"),
            ));
            Warning { code, health }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn an_out_of_range_level_verifies_as_a_contract_violation() {
            let mut buf = [0u8; 8];
            buf.copy_from_slice(&150i64.to_le_bytes());
            let error = <Level as Payload<ReprC>>::verify(&buf).expect_err("150 is out of range");
            assert!(matches!(
                error,
                VerifyError::Contract(Violation {
                    type_name: "Level",
                    rule: Rule::Range,
                })
            ));
        }

        #[test]
        fn malformed_bytes_verify_as_a_structure_error() {
            let short = [0u8; 3];
            let error =
                <Level as Payload<ReprC>>::verify(&short).expect_err("too short to be a Level");
            assert!(matches!(
                error,
                VerifyError::Structure(Malformed::OutOfBounds)
            ));
        }
    }
}

/// A `Provider` whose `average` reply is test-controlled, so a test can make
/// it return a value that breaks the query's `ensure` clause.
struct TestProvider {
    set_level_calls: Vec<i64>,
    next_average: i64,
}

impl TestProvider {
    fn new(next_average: i64) -> Self {
        TestProvider {
            set_level_calls: Vec::new(),
            next_average,
        }
    }
}

impl generated::cabin::Provider for TestProvider {
    fn set_level(&mut self, level: &generated::Level) {
        self.set_level_calls.push(level.inner());
    }

    fn average(&mut self, _window: &generated::Window) -> generated::Average {
        generated::Average::new_unchecked(self.next_average)
    }
}

/// Regenerates the fixture through `generate_face`, using the crate's own
/// compile helper rather than a new cargo subprocess, and compares it
/// byte-for-byte against the checked-in file. `RIDL_UPDATE_GENERATED=1`
/// writes the fresh output instead of comparing.
#[test]
fn generated_interaction_face_matches_the_emitter() {
    let package = support::ir::compile_fixture("interaction_face.ridl");
    let face = ridl_backend_rust::generate_face(&package)
        .expect("generate_face")
        .rust_source;

    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/generated/interaction_face.rs");

    if std::env::var_os("RIDL_UPDATE_GENERATED").is_some() {
        std::fs::write(&path, &face)
            .unwrap_or_else(|error| panic!("write {}: {error}", path.display()));
        return;
    }

    let checked_in = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
    assert_eq!(
        face, checked_in,
        "generated interaction_face.rs is stale; regenerate it with \
         RIDL_UPDATE_GENERATED=1 cargo test -p ridl-backend-rust --test interaction_face"
    );
}

#[test]
fn round_trip_signal_publish_and_read() {
    let mut port = Loopback::new("face.demo");
    {
        let mut publisher = generated::cabin::Publisher::new(&mut port);
        publisher
            .temperature(generated::Temperature::new_unchecked(21))
            .expect("set");
        publisher.commit();
    }

    let client = generated::cabin::Client::new(&mut port);
    let sample = client.temperature().expect("read");
    assert_eq!(sample.value.get(), 21);
    assert_eq!(sample.provenance, Provenance::Live);
}

#[test]
fn round_trip_event_raise_and_receive() {
    let mut port = Loopback::new("face.demo");
    {
        let mut client = generated::cabin::Client::new(&mut port);
        client.subscribe_warning().expect("subscribe");
    }
    {
        let mut publisher = generated::cabin::Publisher::new(&mut port);
        publisher
            .warning(generated::Warning {
                code: generated::Level::new_unchecked(5),
                health: generated::Health::WARN,
            })
            .expect("raise");
    }

    let mut client = generated::cabin::Client::new(&mut port);
    let event = client
        .next_event()
        .expect("next_event")
        .expect("an occurrence is waiting");
    match event {
        generated::cabin::Event::Warning(occurrence) => {
            let warning = occurrence.payload.expect("payload verifies");
            assert_eq!(warning.code.get(), 5);
            assert!(matches!(warning.health, generated::Health::WARN));
        }
    }
}

#[test]
fn round_trip_command_is_acknowledged() {
    let mut port = Loopback::new("face.demo");
    let correlation = {
        let mut client = generated::cabin::Client::new(&mut port);
        client
            .set_level(generated::Level::new_unchecked(42))
            .expect("send")
    };

    let mut provider = TestProvider::new(0);
    let mut buf = [0u8; generated::Cabin::MAX_BUFFER_SIZE];
    let settled = generated::cabin::dispatch(&mut port, &mut provider, &mut buf);
    assert_eq!(settled, 1);
    assert_eq!(provider.set_level_calls, vec![42]);

    let mut client = generated::cabin::Client::new(&mut port);
    assert_eq!(client.ack(correlation), Some(Ok(())));
}

#[test]
fn round_trip_query_reply_is_delivered() {
    let mut port = Loopback::new("face.demo");
    let correlation = {
        let mut client = generated::cabin::Client::new(&mut port);
        client
            .average(generated::Window::new_unchecked(10))
            .expect("send")
    };

    let mut provider = TestProvider::new(7);
    let mut buf = [0u8; generated::Cabin::MAX_BUFFER_SIZE];
    let settled = generated::cabin::dispatch(&mut port, &mut provider, &mut buf);
    assert_eq!(settled, 1);

    let mut client = generated::cabin::Client::new(&mut port);
    let reply = client
        .average_reply(correlation)
        .expect("reply read")
        .expect("reply is known")
        .expect("no call error");
    assert_eq!(reply.get(), 7);
}

#[test]
fn round_trip_failing_require_settles_precondition_failed() {
    use ridl_rt::contract::Interaction;
    use ridl_rt::encoding::ReprC;
    use ridl_rt::payload::Ref;

    let mut port = Loopback::new("face.demo");

    // 100 is a legal `Level` value ([0, 100] inclusive) but fails the
    // command's own `require level < 100` clause. The generated `Client`
    // evaluates `require` itself before sending (face.rs's `send`), so
    // sending through the raw port bypasses that and exercises `dispatch`'s
    // own check instead.
    let level = generated::Level::new_unchecked(100);
    let mut encode_buf = [0u8; generated::Cabin::MAX_BUFFER_SIZE];
    let len = Ref::<generated::Level, ReprC>::encode(&level, &mut encode_buf)
        .expect("encode")
        .bytes()
        .len();
    let ordinal = <generated::CabinSetLevel as Interaction>::MEMBER.ordinal;
    let correlation = port
        .command(
            <generated::Cabin as ridl_rt::contract::Interface>::NUMBER,
            ordinal,
            &encode_buf[..len],
        )
        .expect("send");

    let mut provider = TestProvider::new(0);
    let mut buf = [0u8; generated::Cabin::MAX_BUFFER_SIZE];
    let settled = generated::cabin::dispatch(&mut port, &mut provider, &mut buf);
    assert_eq!(settled, 1, "an outcome was still settled");
    assert!(
        provider.set_level_calls.is_empty(),
        "the provider must not be called when require fails"
    );

    let outcome = port.ack(correlation).expect("settled");
    assert_eq!(
        outcome,
        Err(ridl_rt::error::CallError::Contract(
            ridl_rt::error::Contract::PreconditionFailed
        )),
    );
}

#[test]
fn round_trip_client_set_level_short_circuits_on_failing_require() {
    // 100 is a legal `Level` value ([0, 100] inclusive) but fails the
    // command's own `require level < 100` clause. Unlike
    // `round_trip_failing_require_settles_precondition_failed` above, this
    // sends through the generated `Client::set_level` itself, which is
    // `send()`'s own short circuit (face.rs), not `dispatch`'s: nothing must
    // reach the port.
    let mut port = Loopback::new("face.demo");
    let result = {
        let mut client = generated::cabin::Client::new(&mut port);
        client.set_level(generated::Level::new_unchecked(100))
    };
    assert_eq!(
        result,
        Err(ridl_rt::port::SendError::Contract(
            ridl_rt::error::Contract::PreconditionFailed
        )),
        "a failing require is reported before anything is sent",
    );

    // Nothing was sent: an otherwise-empty port has no claim for dispatch to
    // find.
    let mut provider = TestProvider::new(0);
    let mut buf = [0u8; generated::Cabin::MAX_BUFFER_SIZE];
    let settled = generated::cabin::dispatch(&mut port, &mut provider, &mut buf);
    assert_eq!(
        settled, 0,
        "nothing was sent to the port, so dispatch settles nothing"
    );
    assert!(provider.set_level_calls.is_empty());
}

#[test]
fn round_trip_client_average_short_circuits_on_failing_require() {
    // 0 is a legal `Window` value ([0, 100000] inclusive) but fails the
    // query's own `require window > 0` clause. Sent through the generated
    // `Client::average` itself, so this pins `send()`'s own short circuit
    // (face.rs), not `dispatch`'s.
    let mut port = Loopback::new("face.demo");
    let result = {
        let mut client = generated::cabin::Client::new(&mut port);
        client.average(generated::Window::new_unchecked(0))
    };
    assert_eq!(
        result,
        Err(ridl_rt::port::SendError::Contract(
            ridl_rt::error::Contract::PreconditionFailed
        )),
        "a failing require is reported before anything is sent",
    );

    // Nothing was sent: an otherwise-empty port has no claim for dispatch to
    // find.
    let mut provider = TestProvider::new(0);
    let mut buf = [0u8; generated::Cabin::MAX_BUFFER_SIZE];
    let settled = generated::cabin::dispatch(&mut port, &mut provider, &mut buf);
    assert_eq!(
        settled, 0,
        "nothing was sent to the port, so dispatch settles nothing"
    );
}

#[test]
fn round_trip_failing_ensure_settles_contract_broken() {
    let mut port = Loopback::new("face.demo");
    let correlation = {
        let mut client = generated::cabin::Client::new(&mut port);
        client
            .average(generated::Window::new_unchecked(1))
            .expect("send")
    };

    // The provider misbehaves: it returns a reply whose declared
    // `ensure result >= 0` clause is false. `Average`'s own declared range
    // [0, 1000] would never let a legally constructed value violate this —
    // Rust's newtype does not enforce it at construction — so this proves
    // `dispatch` checks `ensure` independently of the reply type's own range.
    let mut provider = TestProvider::new(-1);
    let mut buf = [0u8; generated::Cabin::MAX_BUFFER_SIZE];
    let settled = generated::cabin::dispatch(&mut port, &mut provider, &mut buf);
    assert_eq!(settled, 1, "an outcome was still settled");

    let mut client = generated::cabin::Client::new(&mut port);
    let reply = client
        .average_reply(correlation)
        .expect("reply read")
        .expect("reply is known");
    assert!(matches!(
        reply,
        Err(ridl_rt::error::CallError::Contract(
            ridl_rt::error::Contract::ContractBroken
        ))
    ));
}

#[test]
fn round_trip_dispatch_counts_only_accepted_settlements() {
    // Sending and dispatching one claim at a time, instead of queuing both
    // commands ahead of a single `dispatch` call, makes each call's own
    // returned count unambiguous: with one success and one failure queued
    // together, the aggregate is 1 whichever way `dispatch` decides what
    // counts as accepted, so an aggregate-only assertion cannot tell a
    // correct count from one that counts the wrong polarity. Splitting the
    // calls means the first call's count is pinned to 0 (its only claim's
    // settlement fails) and the second call's count is pinned to 1 (its only
    // claim's settlement succeeds), which a polarity flip in `dispatch`'s
    // counting condition cannot pass unnoticed.
    let mut port = Loopback::new("face.demo");
    let first = {
        let mut client = generated::cabin::Client::new(&mut port);
        client
            .set_level(generated::Level::new_unchecked(1))
            .expect("send first")
    };
    port.fail_next_settle();

    let mut provider = TestProvider::new(0);
    let mut buf = [0u8; generated::Cabin::MAX_BUFFER_SIZE];
    let settled_first = generated::cabin::dispatch(&mut port, &mut provider, &mut buf);
    assert_eq!(
        settled_first, 0,
        "the first claim's settlement fails and dispatch does not count it"
    );
    assert_eq!(
        port.ack(first),
        None,
        "a failed settle records no outcome for the first claim's correlation"
    );

    let second = {
        let mut client = generated::cabin::Client::new(&mut port);
        client
            .set_level(generated::Level::new_unchecked(2))
            .expect("send second")
    };
    let settled_second = generated::cabin::dispatch(&mut port, &mut provider, &mut buf);
    assert_eq!(
        settled_second, 1,
        "the second claim's settlement succeeds and dispatch counts it"
    );
    assert_eq!(
        port.ack(second),
        Some(Ok(())),
        "the successful settlement is observable as accepted through ack"
    );

    // The original claim's shape: exactly one of the two settlements is
    // counted, across both dispatch calls.
    assert_eq!(settled_first + settled_second, 1);
    // The provider still runs for both claims: `dispatch` settles a command
    // before calling the provider, unconditionally of whether the handler
    // accepted that settlement.
    assert_eq!(provider.set_level_calls, vec![1, 2]);
}

#[test]
fn round_trip_short_caller_buffer_returns_zero_without_consuming_a_claim() {
    let mut port = Loopback::new("face.demo");
    {
        let mut client = generated::cabin::Client::new(&mut port);
        client
            .set_level(generated::Level::new_unchecked(1))
            .expect("send");
    }

    let mut provider = TestProvider::new(0);

    // Too small: dispatch must return 0 and must not consume the claim.
    let mut short = [0u8; 1];
    let settled = generated::cabin::dispatch(&mut port, &mut provider, &mut short);
    assert_eq!(settled, 0, "a short buffer settles nothing");
    assert!(provider.set_level_calls.is_empty());

    // Retrying with a correctly sized buffer still finds the claim the short
    // buffer left untouched.
    let mut buf = [0u8; generated::Cabin::MAX_BUFFER_SIZE];
    let settled = generated::cabin::dispatch(&mut port, &mut provider, &mut buf);
    assert_eq!(settled, 1);
    assert_eq!(provider.set_level_calls, vec![1]);
}

// ---------------------------------------------------------------------------
// Design §6's settlement table: the three rows with no prior runtime proof.
// Each test sends raw bytes through the loopback port, bypassing the
// generated `Client`, so the claim `dispatch` receives has exactly the shape
// the row names.
// ---------------------------------------------------------------------------

#[test]
fn round_trip_unrecognized_ordinal_settles_unknown_interaction() {
    // Ordinal 99 names no member of `Cabin`, so `dispatch`'s match on
    // `claim.ord` falls to its fallback arm regardless of the argument
    // bytes, which is why an empty argument slice is enough here.
    let mut port = Loopback::new("face.demo");
    let correlation = port
        .command(
            <generated::Cabin as ridl_rt::contract::Interface>::NUMBER,
            ridl_rt::contract::Ordinal(99),
            &[],
        )
        .expect("send");

    let mut provider = TestProvider::new(0);
    let mut buf = [0u8; generated::Cabin::MAX_BUFFER_SIZE];
    let settled = generated::cabin::dispatch(&mut port, &mut provider, &mut buf);
    assert_eq!(settled, 1, "an outcome was still settled");
    assert!(
        provider.set_level_calls.is_empty(),
        "the provider must not be called for an unrecognized ordinal"
    );

    let outcome = port.ack(correlation).expect("settled");
    assert_eq!(
        outcome,
        Err(ridl_rt::error::CallError::Contract(
            ridl_rt::error::Contract::UnknownInteraction
        )),
    );
}

#[test]
fn round_trip_foreign_interface_number_settles_unknown_interaction() {
    use ridl_rt::contract::Interaction;
    use ridl_rt::encoding::ReprC;
    use ridl_rt::payload::Ref;

    // `dispatch`'s first branch checks `claim.iface != Cabin::NUMBER` before
    // it ever looks at the ordinal. To pin that branch specifically, the
    // ordinal carried here must be one Cabin's own match arms recognize —
    // `setLevel`'s ordinal, with a well-formed `Level` argument that also
    // passes `setLevel`'s `require` clause — so that if the interface-number
    // branch were removed, dispatch would instead decode the argument, pass
    // `require`, call the provider, and settle `Ok(&[])`: a different
    // observable outcome from the `UnknownInteraction` this test asserts.
    // (An unrecognized ordinal would not do this: Cabin's dispatch falls to
    // its own ordinal fallback arm regardless of the interface-number check,
    // so that claim would settle `UnknownInteraction` either way — see
    // `round_trip_unrecognized_ordinal_settles_unknown_interaction` above,
    // which pins that fallback arm instead.)
    let mut port = Loopback::new("face.demo");
    let level = generated::Level::new_unchecked(50);
    let mut encode_buf = [0u8; generated::Cabin::MAX_BUFFER_SIZE];
    let len = Ref::<generated::Level, ReprC>::encode(&level, &mut encode_buf)
        .expect("encode")
        .bytes()
        .len();
    let ordinal = <generated::CabinSetLevel as Interaction>::MEMBER.ordinal;
    let correlation = port
        .command(
            <generated::Horn as ridl_rt::contract::Interface>::NUMBER,
            ordinal,
            &encode_buf[..len],
        )
        .expect("send");

    let mut provider = TestProvider::new(0);
    let mut buf = [0u8; generated::Cabin::MAX_BUFFER_SIZE];
    let settled = generated::cabin::dispatch(&mut port, &mut provider, &mut buf);
    assert_eq!(settled, 1, "an outcome was still settled");
    assert!(
        provider.set_level_calls.is_empty(),
        "the provider must not be called for a foreign interface number"
    );

    let outcome = port.ack(correlation).expect("settled");
    assert_eq!(
        outcome,
        Err(ridl_rt::error::CallError::Contract(
            ridl_rt::error::Contract::UnknownInteraction
        )),
    );
}

#[test]
fn round_trip_malformed_argument_bytes_settle_transport_corrupt() {
    use ridl_rt::contract::Interaction;

    // `Level`'s `ReprC` encoding is exactly 8 bytes (`Payload::verify` in
    // `tests/interaction_face.rs`'s own `payloads` module); 3 bytes fail
    // that length check before the value is ever read, so this exercises
    // `VerifyError::Structure`, not `VerifyError::Contract`.
    let mut port = Loopback::new("face.demo");
    let ordinal = <generated::CabinSetLevel as Interaction>::MEMBER.ordinal;
    let correlation = port
        .command(
            <generated::Cabin as ridl_rt::contract::Interface>::NUMBER,
            ordinal,
            &[1, 2, 3],
        )
        .expect("send");

    let mut provider = TestProvider::new(0);
    let mut buf = [0u8; generated::Cabin::MAX_BUFFER_SIZE];
    let settled = generated::cabin::dispatch(&mut port, &mut provider, &mut buf);
    assert_eq!(settled, 1, "an outcome was still settled");
    assert!(
        provider.set_level_calls.is_empty(),
        "the provider must not be called when the argument bytes fail to verify"
    );

    let outcome = port.ack(correlation).expect("settled");
    assert_eq!(
        outcome,
        Err(ridl_rt::error::CallError::Transport(
            ridl_rt::error::Transport::Corrupt
        )),
    );
}

#[test]
fn round_trip_out_of_range_argument_settles_invalid_value() {
    use ridl_rt::contract::Interaction;
    use ridl_rt::encoding::ReprC;
    use ridl_rt::payload::Ref;

    // 200 is out of `Level`'s declared range [0, 100]. `new` would refuse
    // it; `new_unchecked` does not, so this value encodes to a well-formed
    // 8-byte `ReprC` payload and fails `verify`'s range check rather than
    // its length check.
    let mut port = Loopback::new("face.demo");
    let level = generated::Level::new_unchecked(200);
    let mut encode_buf = [0u8; generated::Cabin::MAX_BUFFER_SIZE];
    let len = Ref::<generated::Level, ReprC>::encode(&level, &mut encode_buf)
        .expect("encode")
        .bytes()
        .len();
    let ordinal = <generated::CabinSetLevel as Interaction>::MEMBER.ordinal;
    let correlation = port
        .command(
            <generated::Cabin as ridl_rt::contract::Interface>::NUMBER,
            ordinal,
            &encode_buf[..len],
        )
        .expect("send");

    let mut provider = TestProvider::new(0);
    let mut buf = [0u8; generated::Cabin::MAX_BUFFER_SIZE];
    let settled = generated::cabin::dispatch(&mut port, &mut provider, &mut buf);
    assert_eq!(settled, 1, "an outcome was still settled");
    assert!(
        provider.set_level_calls.is_empty(),
        "the provider must not be called when the argument value breaks its typl constraints"
    );

    let outcome = port.ack(correlation).expect("settled");
    assert_eq!(
        outcome,
        Err(ridl_rt::error::CallError::Contract(
            ridl_rt::error::Contract::InvalidValue(ridl_rt::payload::Violation {
                type_name: "Level",
                rule: ridl_rt::payload::Rule::Range,
            })
        )),
    );
}

// ---------------------------------------------------------------------------
// RA-19's compiled proof (design §6): a minimal port, not a bound.
// ---------------------------------------------------------------------------

/// A port that implements only `Attached` and `SignalReader` — no `Caller`,
/// no `EventSource`, no `EventSink`, no `Handler`.
///
/// A trait bound is a lower bound, so a port type that implements more traits
/// than a bound requires satisfies it anyway. Constructing the signal-only
/// `horn` interface's `Client` from `Loopback`, which implements every port
/// trait, would prove nothing: that construction compiles whether or not the
/// emitter added a `Caller` or `EventSource` bound the interface does not
/// need. Design §6 states the property the other way round: the claim is
/// shown by a minimal port implementing `SignalReader` and its `Attached`
/// supertrait and nothing else, which must construct the client. Had the
/// emitter written a wider bound than the interface needs, this port would
/// not satisfy it and `generated::horn::Client::new(&mut port)` below would
/// fail to compile.
struct MinimalSignalOnlyPort {
    catalog: ridl_rt::contract::CatalogRef,
}

impl MinimalSignalOnlyPort {
    fn new(package_name: &'static str) -> Self {
        MinimalSignalOnlyPort {
            catalog: ridl_rt::contract::CatalogRef {
                name: package_name,
                hash: ridl_rt::contract::CatalogHash([0u8; 32]),
            },
        }
    }
}

impl ridl_rt::port::Attached for MinimalSignalOnlyPort {
    fn catalog(&self) -> &ridl_rt::contract::CatalogRef {
        &self.catalog
    }
}

impl ridl_rt::port::SignalReader for MinimalSignalOnlyPort {
    fn read(
        &self,
        _iface: InterfaceNo,
        _ord: Ordinal,
        _out: &mut [u8],
    ) -> Result<ridl_rt::port::RawSample, ridl_rt::port::ReadError> {
        Ok(ridl_rt::port::RawSample {
            provenance: Provenance::Init,
            freshness: ridl_rt::sample::Freshness::Unbounded,
            envelope: ridl_rt::sample::Envelope {
                stamp: ridl_rt::sample::Timestamp(0),
                seq: 0,
            },
            len: 0,
        })
    }
}

/// RA-19's compiled proof, in the direction design §6 gives: the minimal
/// `SignalReader`-and-`Attached`-only port constructs the signal-only `horn`
/// interface's `Client`, and calling `active()` uses it, rather than only
/// type-checking the construction. See `MinimalSignalOnlyPort`'s doc comment
/// for why this port, and not the full `Loopback`, is what proves the bound
/// is exact.
///
/// The zero-length `RawSample` `MinimalSignalOnlyPort::read` returns is too
/// short for `Health`'s 8-byte encoding, so `active()`'s own structure check
/// reports it as corrupt — the same client-side path
/// `the_client_reads_a_signal_through_the_signal_reader_port` in
/// `face_generation.rs` pins as `Detection::Corrupt`. The assertion below
/// confirms the call completed through that path, not that the port served
/// real data, which is outside this test's purpose.
#[test]
fn ra19_a_minimal_signal_only_port_constructs_the_signal_only_client() {
    let mut port = MinimalSignalOnlyPort::new("face.demo");
    let client = generated::horn::Client::new(&mut port);
    let sample = client.active().expect("read");
    assert_eq!(
        sample.provenance,
        Provenance::Invalid(ridl_rt::sample::Cause::Detected(
            ridl_rt::sample::Detection::Corrupt
        )),
        "a zero-length sample is too short for Health's 8-byte encoding",
    );
}

// ---------------------------------------------------------------------------
// The generated constructor (typl value objects, Task 3), run rather than read.
// The constructor's text is also snapshotted, but a snapshot passes with a
// deleted check as soon as `cargo insta test --accept` runs; these four fail.
// `Level` is declared `integer [0..100]` in tests/fixtures/interaction_face.ridl.
// ---------------------------------------------------------------------------

#[test]
fn generated_new_refuses_a_value_above_the_range() {
    use ridl_rt::payload::{Rule, Violation};
    assert_eq!(
        generated::Level::new(200).err(),
        Some(Violation {
            type_name: "Level",
            rule: Rule::Range,
        })
    );
}

#[test]
fn generated_new_refuses_a_value_below_the_range() {
    use ridl_rt::payload::{Rule, Violation};
    assert_eq!(
        generated::Level::new(-1).err(),
        Some(Violation {
            type_name: "Level",
            rule: Rule::Range,
        })
    );
}

#[test]
fn generated_new_accepts_a_value_in_the_range() {
    let Ok(level) = generated::Level::new(50) else {
        panic!("50 is inside [0, 100]");
    };
    assert_eq!(level.get(), 50);
    // Both bounds are inclusive (typl §5.5).
    assert_eq!(
        generated::Level::new(0).ok().map(generated::Level::get),
        Some(0)
    );
    assert_eq!(
        generated::Level::new(100).ok().map(generated::Level::get),
        Some(100)
    );
}

#[test]
fn generated_try_from_delegates_to_new() {
    use ridl_rt::payload::{Rule, Violation};
    assert_eq!(
        generated::Level::try_from(200).err(),
        Some(Violation {
            type_name: "Level",
            rule: Rule::Range,
        })
    );
    assert_eq!(
        generated::Level::try_from(50)
            .ok()
            .map(generated::Level::get),
        Some(50)
    );
}
