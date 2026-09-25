//! The interaction-face round trip (Lane M stage M3, Task 5,
//! `docs/design/interaction-face.md`; the approved design is
//! `docs/archive/2026-09-16-interaction-face-v0-design.md` §7).
//!
//! It brings in the checked-in generated face of `tests/generated/` and runs
//! the round trip over `ridl-loopback`, the in-process reference runtime: a
//! signal publish and read, an event raise and receive, a command's
//! acknowledgment, a query's reply, a failing `require`, a failing `ensure`,
//! the settlement count when one claim's settlement fails and a later one
//! succeeds, and dispatch's short-buffer behavior.
//!
//! **Every payload here is encoded by the generated FlatBuffers codec**
//! (stage K9b, design note D-11). The hand-written `Payload<ReprC>` module
//! this file carried until then is gone, and with it the `PAD` constant that
//! made its `encode` return a subslice rather than a prefix. That guard is
//! not lost: a FlatBuffers builder fills a buffer from its end, so what the
//! generated `encode` returns is a suffix of the output buffer and a caller
//! that re-sliced `&buf[..len]` would send leading bytes the encoder never
//! wrote. `the_encoded_bytes_are_not_a_prefix_of_the_buffer` states that
//! property directly, and every round trip below goes red if the emitter
//! re-slices, because each one sends through the generated `Client`.
//!
//! The ports the round trip runs over are the aggregate handle of
//! `ridl-loopback` (story E11.15, driftsys/ridl#445). Until that story landed
//! they were a disposable double at `tests/support/loopback.rs`, exercised
//! ahead of the face by a set of `support_*` tests; both the double and those
//! tests are gone, the tests having moved to `crates/ridl-loopback/tests/`
//! as tests of the runtime itself.

mod support;

use ridl_loopback::Loopback;
use ridl_rt::contract::{CatalogHash, CatalogRef, InterfaceNo, Ordinal};
use ridl_rt::port::Caller;
use ridl_rt::sample::Provenance;

/// The catalog the fixture's package declares, with the all-zero placeholder
/// hash the descriptor emitter writes until story E16.2 (driftsys/ridl#378)
/// computes a real one. The generated constructor performs no catalog check
/// (driftsys/ridl#448), and the loopback checks nothing against it either.
const CATALOG: CatalogRef = CatalogRef {
    name: "face.demo",
    hash: CatalogHash([0u8; 32]),
};

fn loopback() -> Loopback {
    Loopback::new(CATALOG)
}

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
        self.set_level_calls.push(level.get());
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
    let mut port = loopback();
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

/// Before any publication, a signal reads its init value under
/// `Provenance::Init` (ridl §4.4), not as a detected invalid state. The port
/// already reports this correctly (`ridl-loopback`'s
/// `a_signal_with_no_publication_reads_as_init_and_copies_nothing`); this test
/// pins it through the generated face, over `ridl-loopback`, driftsys/ridl#517.
#[test]
fn round_trip_signal_reads_as_init_before_any_publication() {
    let mut port = loopback();
    let client = generated::cabin::Client::new(&mut port);
    let sample = client.temperature().expect("read");
    assert_eq!(
        sample.value,
        <generated::CabinTemperature as ridl_rt::contract::Signal>::init()
    );
    assert_eq!(sample.provenance, Provenance::Init);
}

/// A channel invalidated with no prior publication has no last good value to
/// keep (ridl §4.5: "the last good value, or the init value when there is
/// none"), so it reads as the init value under `Invalid(Declared)`, not as a
/// detected invalid state — the same distinction as the never-published case,
/// driftsys/ridl#517.
#[test]
fn round_trip_signal_reads_as_init_when_invalidated_before_any_publication() {
    let mut port = loopback();
    {
        let mut publisher = generated::cabin::Publisher::new(&mut port);
        publisher.invalidate_temperature().expect("invalidate");
        publisher.commit();
    }

    let client = generated::cabin::Client::new(&mut port);
    let sample = client.temperature().expect("read");
    assert_eq!(
        sample.value,
        <generated::CabinTemperature as ridl_rt::contract::Signal>::init()
    );
    assert_eq!(
        sample.provenance,
        Provenance::Invalid(ridl_rt::sample::Cause::Declared)
    );
}

/// A channel invalidated after a real publication is a third case, distinct
/// from both signals above: `ridl-loopback` keeps the last good value's bytes
/// under `Invalid(Declared)` (ridl §4.5), so the accessor's provenance-and-
/// length guard does not apply — there are real bytes to verify and decode —
/// and the value stays the last published one, not the init value. This is
/// the case a mutant that routed every non-`Live` read to the init value would
/// pass undetected if it were the only invalidate test.
#[test]
fn round_trip_signal_keeps_the_last_good_value_when_declared_invalid_after_publication() {
    let mut port = loopback();
    {
        let mut publisher = generated::cabin::Publisher::new(&mut port);
        publisher
            .temperature(generated::Temperature::new_unchecked(21))
            .expect("set");
        publisher.commit();
        publisher.invalidate_temperature().expect("invalidate");
        publisher.commit();
    }

    let client = generated::cabin::Client::new(&mut port);
    let sample = client.temperature().expect("read");
    assert_eq!(sample.value.get(), 21);
    assert_eq!(
        sample.provenance,
        Provenance::Invalid(ridl_rt::sample::Cause::Declared)
    );
}

/// A live publication whose bytes the accessor cannot decode is a separate
/// case from a never-published or never-set-then-invalidated channel: there is
/// a payload, and it fails its check, so the accessor reports the detected
/// cause and substitutes the init value, over real (malformed) bytes rather
/// than an empty buffer. Written directly through the raw `SignalWriter` port,
/// bypassing the generated `Publisher`'s encoder, which would refuse to send
/// bytes this short.
#[test]
fn round_trip_signal_with_malformed_bytes_settles_detected_corrupt() {
    use ridl_rt::contract::Interaction;
    use ridl_rt::port::SignalWriter;

    let mut port = loopback();
    let ordinal = <generated::CabinTemperature as Interaction>::MEMBER.ordinal;
    port.set(
        <generated::Cabin as ridl_rt::contract::Interface>::NUMBER,
        ordinal,
        // `Temperature`'s FlatBuffers encoding is a box table behind a root
        // offset (ADR-0019 decision 8); 3 bytes are too short even for that
        // offset, so `verify` refuses the buffer's structure.
        &[1, 2, 3],
    )
    .expect("set");
    port.commit();

    let client = generated::cabin::Client::new(&mut port);
    let sample = client.temperature().expect("read");
    assert_eq!(
        sample.value,
        <generated::CabinTemperature as ridl_rt::contract::Signal>::init()
    );
    assert_eq!(
        sample.provenance,
        Provenance::Invalid(ridl_rt::sample::Cause::Detected(
            ridl_rt::sample::Detection::Corrupt
        ))
    );
}

#[test]
fn round_trip_event_raise_and_receive() {
    let mut port = loopback();
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
    let mut port = loopback();
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
    assert_eq!(client.set_level_ack(correlation), Some(Ok(())));
}

#[test]
fn round_trip_query_reply_is_delivered() {
    let mut port = loopback();
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
    use ridl_rt::payload::Ref;

    let mut port = loopback();

    // 100 is a legal `Level` value ([0, 100] inclusive) but fails the
    // command's own `require level < 100` clause. The generated `Client`
    // evaluates `require` itself before sending (face.rs's `send`), so
    // sending through the raw port bypasses that and exercises `dispatch`'s
    // own check instead.
    let level = generated::Level::new_unchecked(100);
    let mut encode_buf = [0u8; generated::Cabin::MAX_BUFFER_SIZE];
    // `Encoded.bytes` is a subslice of the buffer, not a prefix of it
    // (`crates/ridl-rt/src/payload.rs`), and the generated FlatBuffers
    // encoder builds at the tail, so the bytes to send are the ones the
    // encoder returned and never `&encode_buf[..len]`.
    let bytes = Ref::<generated::Level, generated::Wire>::encode(&level, &mut encode_buf)
        .expect("encode")
        .bytes();
    let ordinal = <generated::CabinSetLevel as Interaction>::MEMBER.ordinal;
    let correlation = port
        .command(
            <generated::Cabin as ridl_rt::contract::Interface>::NUMBER,
            ordinal,
            bytes,
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
    let mut port = loopback();
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
    let mut port = loopback();
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
    let mut port = loopback();
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
    let mut port = loopback();
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
        port.ack(first.0),
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
        port.ack(second.0),
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
    let mut port = loopback();
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
// The property the deleted `payloads` module's `PAD` constant used to guard.
// ---------------------------------------------------------------------------

/// `Encoded.bytes` is a subslice of the output buffer and not necessarily a
/// prefix of it (`crates/ridl-rt/src/payload.rs`, ADR-0021 decision 7's
/// 2026-09-20 amendment). Until stage K9b this file's own throwaway
/// `Payload<ReprC>` implementations wrote after four leading bytes so that
/// the property held of them too; the generated FlatBuffers codec needs no
/// such arrangement, because a FlatBuffers builder fills a buffer from its
/// end.
///
/// This states the property over the real codec: the bytes `encode` returns
/// start past the front of the buffer, and the equally long prefix of that
/// same buffer is not a payload. A face that reconstructed `&buf[..len]`
/// would send exactly that prefix.
#[test]
fn the_encoded_bytes_are_not_a_prefix_of_the_buffer() {
    use ridl_rt::payload::{Payload, Ref};

    let level = generated::Level::new_unchecked(42);
    let mut buf = [0u8; <generated::Level as Payload<generated::Wire>>::MAX_SIZE];
    let encoded = Ref::<generated::Level, generated::Wire>::encode(&level, &mut buf)
        .expect("encode")
        .bytes()
        .to_vec();

    assert!(
        encoded.len() < buf.len(),
        "the bound leaves room ahead of the value, which is what makes the \
         prefix and the subslice different slices"
    );
    assert_ne!(
        encoded.as_slice(),
        &buf[..encoded.len()],
        "the encoder built at the tail, so the equally long prefix is not \
         the value"
    );
    <generated::Level as Payload<generated::Wire>>::verify(&buf[..encoded.len()])
        .expect_err("the prefix of the buffer is not a payload");
    let checked =
        Ref::<generated::Level, generated::Wire>::verify(&encoded).expect("the subslice is");
    assert_eq!(checked.decode().get(), 42);
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
    let mut port = loopback();
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
    let mut port = loopback();
    let level = generated::Level::new_unchecked(50);
    let mut encode_buf = [0u8; generated::Cabin::MAX_BUFFER_SIZE];
    // `Encoded.bytes` is a subslice of the buffer, not a prefix of it
    // (`crates/ridl-rt/src/payload.rs`), and the generated FlatBuffers
    // encoder builds at the tail, so the bytes to send are the ones the
    // encoder returned and never `&encode_buf[..len]`.
    let bytes = Ref::<generated::Level, generated::Wire>::encode(&level, &mut encode_buf)
        .expect("encode")
        .bytes();
    let ordinal = <generated::CabinSetLevel as Interaction>::MEMBER.ordinal;
    let correlation = port
        .command(
            <generated::Horn as ridl_rt::contract::Interface>::NUMBER,
            ordinal,
            bytes,
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

    // `Level`'s FlatBuffers encoding is a box table behind a root offset
    // (ADR-0019 decision 8); 3 bytes are too short even for that offset, so
    // the generated `verify` refuses the buffer's structure before any value
    // is read. This exercises `VerifyError::Structure`, not
    // `VerifyError::Contract`.
    let mut port = loopback();
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
    use ridl_rt::payload::Ref;

    // 200 is out of `Level`'s declared range [0, 100]. `new` would refuse
    // it; `new_unchecked` does not, so this value encodes to a structurally
    // well-formed box table and fails `verify`'s own range check — the
    // `check` the codec calls beside the structure walk — rather than its
    // structure check.
    let mut port = loopback();
    let level = generated::Level::new_unchecked(200);
    let mut encode_buf = [0u8; generated::Cabin::MAX_BUFFER_SIZE];
    // `Encoded.bytes` is a subslice of the buffer, not a prefix of it
    // (`crates/ridl-rt/src/payload.rs`), and the generated FlatBuffers
    // encoder builds at the tail, so the bytes to send are the ones the
    // encoder returned and never `&encode_buf[..len]`.
    let bytes = Ref::<generated::Level, generated::Wire>::encode(&level, &mut encode_buf)
        .expect("encode")
        .bytes();
    let ordinal = <generated::CabinSetLevel as Interaction>::MEMBER.ordinal;
    let correlation = port
        .command(
            <generated::Cabin as ridl_rt::contract::Interface>::NUMBER,
            ordinal,
            bytes,
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
/// `MinimalSignalOnlyPort::read` reports `Provenance::Init` with a
/// zero-length sample, the shape of a channel with no publication (ridl
/// §4.4). `active()` reports this as `Provenance::Init` directly, without
/// attempting to decode the empty buffer (driftsys/ridl#517): the assertion
/// below confirms the call completed through that path, not that the port
/// served real data, which is outside this test's purpose.
#[test]
fn ra19_a_minimal_signal_only_port_constructs_the_signal_only_client() {
    let mut port = MinimalSignalOnlyPort::new("face.demo");
    let client = generated::horn::Client::new(&mut port);
    let sample = client.active().expect("read");
    assert_eq!(
        sample.provenance,
        Provenance::Init,
        "a never-published signal reads as Init, not a detected invalid state",
    );
}

/// A port whose `Init` sample carries a distinctive freshness and envelope,
/// unlike every other port in this file, which reports `Freshness::Unbounded`
/// and a zero envelope under `Init`. Its purpose is narrow: proving that the
/// accessor's `Init`/`Invalid(Declared)` branch passes the port's own
/// freshness and envelope through to the `Sample` unchanged, rather than
/// fabricating `Freshness::Fresh` or a zero envelope — values indistinguishable
/// from the common case in every other test.
struct DistinctiveInitPort {
    catalog: ridl_rt::contract::CatalogRef,
}

impl DistinctiveInitPort {
    fn new(package_name: &'static str) -> Self {
        DistinctiveInitPort {
            catalog: ridl_rt::contract::CatalogRef {
                name: package_name,
                hash: ridl_rt::contract::CatalogHash([0u8; 32]),
            },
        }
    }
}

impl ridl_rt::port::Attached for DistinctiveInitPort {
    fn catalog(&self) -> &ridl_rt::contract::CatalogRef {
        &self.catalog
    }
}

impl ridl_rt::port::SignalReader for DistinctiveInitPort {
    fn read(
        &self,
        _iface: InterfaceNo,
        _ord: Ordinal,
        _out: &mut [u8],
    ) -> Result<ridl_rt::port::RawSample, ridl_rt::port::ReadError> {
        Ok(ridl_rt::port::RawSample {
            provenance: Provenance::Init,
            freshness: ridl_rt::sample::Freshness::Stale {
                by: ridl_rt::sample::Duration(5),
            },
            envelope: ridl_rt::sample::Envelope {
                stamp: ridl_rt::sample::Timestamp(7),
                seq: 3,
            },
            len: 0,
        })
    }
}

#[test]
fn the_init_branch_passes_the_ports_freshness_and_envelope_through_unchanged() {
    let mut port = DistinctiveInitPort::new("face.demo");
    let client = generated::horn::Client::new(&mut port);
    let sample = client.active().expect("read");
    assert_eq!(
        sample.freshness,
        ridl_rt::sample::Freshness::Stale {
            by: ridl_rt::sample::Duration(5)
        }
    );
    assert_eq!(
        sample.envelope,
        ridl_rt::sample::Envelope {
            stamp: ridl_rt::sample::Timestamp(7),
            seq: 3
        }
    );
}

// ---------------------------------------------------------------------------
// The generated constructor (typl value objects, Task 3), run rather than read.
// The constructor's text is also checked, but only by comparison against a
// regenerable fixture: `cargo insta test --accept` does not touch this file,
// and `RIDL_UPDATE_GENERATED=1` rewrites it wholesale. Either way a text check
// states what the constructor says, and these four state what it does.
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
