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
//! property directly, and every round trip that sends through the generated
//! `Client` goes red if the emitter re-slices; the settlement-table tests send
//! through the raw port, with the encoder's own returned slice.
//!
//! The ports the round trip runs over are the aggregate handle of
//! `ridl-loopback` (story E11.15, driftsys/ridl#445). Until that story landed
//! they were a disposable double at `tests/support/loopback.rs`, exercised
//! ahead of the face by a set of `support_*` tests; both the double and those
//! tests are gone, the tests having moved to `crates/ridl-loopback/tests/`
//! as tests of the runtime itself.
//!
//! **Since story E11.21 (first half) the consumer side is the async client.**
//! A command, a query and `next_event` return a named future, polled here by
//! hand with `ridl_rt::task::noop_waker`, the way a frame loop polls; the
//! provider side is `serve`, polled once per step. The poll face those futures
//! are built over is `pub(crate)` in the generated module, and this file is
//! inside the crate that `include!`s it, so the tests of the internal one-pass
//! step `dispatch` — its count, its short-buffer rule, and the settlement
//! rows a `serve` over the loopback can never be presented, because the
//! handler serves only the interface's own members — call it directly. The
//! face tests of the async face design's note F-14 are in their own section
//! below.

mod support;

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::task::{Context, Poll, Wake, Waker};

use ridl_loopback::Loopback;
use ridl_rt::contract::{CatalogHash, CatalogRef, InterfaceNo, Ordinal};
use ridl_rt::error::{CallError, ClientError, Contract, ProviderError, Transport};
use ridl_rt::port::{Caller, Correlation, Interest, ReadError, SendError, ServeError};
use ridl_rt::sample::{Duration, Provenance};

use support::doubles::{self, FailingHandler, Op, RecordingPorts};

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

/// Polls `future` once with the no-op waker, the way a frame loop does: the
/// future makes progress only when it is polled again.
fn poll_once<F: Future + Unpin>(future: &mut F) -> Poll<F::Output> {
    let waker = ridl_rt::task::noop_waker();
    let mut cx = Context::from_waker(&waker);
    Pin::new(future).poll(&mut cx)
}

/// The fixture's `Cabin` interface number, as its descriptor declares it.
const CABIN: InterfaceNo = <generated::Cabin as ridl_rt::contract::Interface>::NUMBER;

/// Sends `setLevel(level)` through the raw port, bypassing the generated
/// client, for a test of the provider side alone. The client's future holds
/// the port for as long as it lives, and dropping it forgets the call, which
/// the loopback then withdraws, so a claim meant to wait for `dispatch` is
/// sent this way.
fn send_level_raw(port: &mut Loopback, level: i64) -> Correlation {
    use ridl_rt::contract::Interaction;
    use ridl_rt::payload::Ref;

    let level = generated::Level::new_unchecked(level);
    let mut buf = [0u8; generated::Cabin::MAX_BUFFER_SIZE];
    // `Encoded.bytes` is a subslice of the buffer, not a prefix of it
    // (`crates/ridl-rt/src/payload.rs`), and the generated FlatBuffers
    // encoder builds at the tail, so the bytes to send are the ones the
    // encoder returned and never `&buf[..len]`.
    let bytes = Ref::<generated::Level, generated::Wire>::encode(&level, &mut buf)
        .expect("encode")
        .bytes();
    port.command(
        CABIN,
        <generated::CabinSetLevel as Interaction>::MEMBER.ordinal,
        bytes,
    )
    .expect("send")
}

/// The checked-in output of `generate_face` over `tests/fixtures/interaction_face.ridl`.
/// It is `include!`d, never hand-edited; the regeneration guard in
/// `tests/interaction_face_regeneration.rs` compares it byte-for-byte against a
/// fresh `generate_face` call. Lint allows live on
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

/// The same provider serves `Valve`, the fixture's calls-only interface with
/// no response bound, whose reply is the same test-controlled value.
impl generated::valve::Provider for TestProvider {
    fn open(&mut self, level: &generated::Level) {
        self.set_level_calls.push(level.get());
    }

    fn pressure(&mut self, _window: &generated::Window) -> generated::Average {
        generated::Average::new_unchecked(self.next_average)
    }
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
/// already reports this correctly (`ridl-rt-conformance`'s
/// `a_signal_with_no_publication_reads_as_init_and_copies_nothing`, which
/// `ridl-loopback` runs); this test
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
/// keep (`docs/specification/frame-specification.md` §5.1 receive rule 5:
/// "the last good value, or the init value when there is none"), so it reads
/// as the init value under `Invalid(Declared)`, not as a detected invalid
/// state — the same distinction as the never-published case,
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
/// bypassing the generated `Publisher`'s encoder, which takes a typed
/// `Temperature` and always encodes it, so it cannot produce malformed bytes.
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

/// A live publication of zero bytes is a fourth, narrower case than the
/// malformed-bytes one above: `SignalWriter::set` accepts an empty payload,
/// and `ridl-loopback`'s `commit` publishes it as `Provenance::Live`, not
/// `Provenance::Init` or `Invalid(Declared)` — a channel with no publication
/// at all is a different state, reported as `Provenance::Init` directly
/// (`round_trip_signal_reads_as_init_before_any_publication` above). The
/// accessor's `Init`-or-`Invalid(Declared)`-and-`len == 0` guard therefore
/// does not match here on provenance, so `verify` runs over the empty buffer
/// and reports it corrupt, the same as any other malformed payload. This
/// pins the guard's provenance condition: a guard that matched on length
/// alone, regardless of provenance, would route this zero-length `Live`
/// sample to the init value under `Provenance::Live` instead, and this test
/// would fail on the provenance assertion below (driftsys/ridl#519).
#[test]
fn round_trip_signal_with_zero_length_live_bytes_settles_detected_corrupt() {
    use ridl_rt::contract::Interaction;
    use ridl_rt::port::SignalWriter;

    let mut port = loopback();
    let ordinal = <generated::CabinTemperature as Interaction>::MEMBER.ordinal;
    port.set(
        <generated::Cabin as ridl_rt::contract::Interface>::NUMBER,
        ordinal,
        &[],
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

/// A port that reports `Provenance::Init` over a channel that carries a real,
/// well-formed encoding — a shape `ridl-loopback` itself never produces (its
/// `Init` sample is always zero-length, `unpublished` in `store.rs`), but one
/// the accessor's `Init`-and-`len == 0` guard makes no promise about once the
/// length is nonzero. It wraps a real `Loopback`, whose bytes come from a
/// genuine FlatBuffers encoding written through the generated `Publisher`,
/// and reports every read as `Provenance::Init` regardless of what the inner
/// runtime recorded.
struct ReportsInitOverRealBytes {
    inner: Loopback,
}

impl ridl_rt::port::Attached for ReportsInitOverRealBytes {
    fn catalog(&self) -> &ridl_rt::contract::CatalogRef {
        self.inner.catalog()
    }
}

impl ridl_rt::port::SignalReader for ReportsInitOverRealBytes {
    fn read(
        &self,
        iface: InterfaceNo,
        ord: Ordinal,
        out: &mut [u8],
    ) -> Result<ridl_rt::port::RawSample, ridl_rt::port::ReadError> {
        let raw = self.inner.read(iface, ord, out)?;
        Ok(ridl_rt::port::RawSample {
            provenance: Provenance::Init,
            ..raw
        })
    }
}

/// At a nonzero length, the `Init` arm's guard does not match, so the
/// accessor verifies and decodes the port's bytes instead of substituting the
/// descriptor's init value — the case the guard's `if raw.len == 0` condition
/// exists to exclude. Runs over the signal-only `horn` interface, so the
/// wrapper needs to implement only `SignalReader`, the same minimal shape as
/// `DistinctiveInitPort` above. `Health::default()` (the descriptor's init
/// value) is `Health::Ok`, so publishing `Health::Warn` gives a decoded value
/// the init value cannot be confused with. A guard that took the `Init` arm
/// at any length would substitute `Health::Ok` here instead of decoding, and
/// the `sample.value` assertion below would fail (driftsys/ridl#519).
#[test]
fn round_trip_signal_reported_as_init_with_real_bytes_decodes_them() {
    let mut port = ReportsInitOverRealBytes { inner: loopback() };
    {
        let mut publisher = generated::horn::Publisher::new(&mut port.inner);
        publisher.active(generated::Health::Warn).expect("set");
        publisher.commit();
    }

    let client = generated::horn::Client::new(&mut port);
    let sample = client.active().expect("read");
    assert_eq!(sample.value, generated::Health::Warn);
    assert_eq!(sample.provenance, Provenance::Init);
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
                health: generated::Health::Warn,
            })
            .expect("raise");
    }

    let mut client = generated::cabin::Client::new(&mut port);
    let mut next = client.next_event();
    let Poll::Ready(event) = poll_once(&mut next) else {
        panic!("an occurrence is waiting, so the future is ready on its first poll");
    };
    match event.expect("next_event") {
        generated::cabin::Event::Warning(occurrence) => {
            let warning = occurrence.payload.expect("payload verifies");
            assert_eq!(warning.code.get(), 5);
            assert!(matches!(warning.health, generated::Health::Warn));
        }
    }
}

/// The command round trip through the async client and `serve`, as a frame
/// loop drives them: the call is sent when the method runs, one poll of the
/// provider side settles it, and the next poll of the call takes the
/// acknowledgment. The handler is taken from the runtime before the client
/// borrows it, because the call's future holds the client's port for as long
/// as it lives (the async face design, note F-4).
#[test]
fn round_trip_command_is_acknowledged() {
    let mut rt = loopback();
    let handler = rt.handler();
    let mut provider = TestProvider::new(0);
    let mut serve = generated::cabin::serve(handler, &mut provider);

    let mut client = generated::cabin::Client::new(&mut rt);
    let mut call = client.set_level(generated::Level::new_unchecked(42));
    assert!(
        poll_once(&mut call).is_pending(),
        "the call was sent, and nothing has served it"
    );
    assert!(
        poll_once(&mut serve).is_pending(),
        "serve settles the claim and keeps serving"
    );
    assert_eq!(poll_once(&mut call), Poll::Ready(Ok(())));

    drop(serve);
    assert_eq!(provider.set_level_calls, vec![42]);
}

#[test]
fn round_trip_query_reply_is_delivered() {
    let mut rt = loopback();
    let handler = rt.handler();
    let mut provider = TestProvider::new(7);
    let mut serve = generated::cabin::serve(handler, &mut provider);

    let mut client = generated::cabin::Client::new(&mut rt);
    let mut call = client.average(generated::Window::new_unchecked(10));
    assert!(
        poll_once(&mut call).is_pending(),
        "the query was sent, and nothing has served it"
    );
    assert!(poll_once(&mut serve).is_pending());
    assert_eq!(
        poll_once(&mut call),
        Poll::Ready(Ok(generated::Average::new_unchecked(7)))
    );
}

#[test]
fn round_trip_failing_require_settles_precondition_failed() {
    use ridl_rt::contract::Interaction;
    use ridl_rt::payload::Ref;

    let mut port = loopback();

    // 100 is a legal `Level` value ([0, 100] inclusive) but fails the
    // command's own `require level < 100` clause. The generated `Client`
    // evaluates `require` itself before sending (face/poll.rs's `send`), so
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
    assert_eq!(settled, Ok(1), "an outcome was still settled");
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
    // sends through the generated `Client::set_level` itself, which is the
    // internal send's own short circuit (face/poll.rs), not `dispatch`'s: nothing
    // must reach the port, and the future is ready with the send's error on
    // its first poll (the async face design, note F-4).
    let mut port = loopback();
    {
        let mut client = generated::cabin::Client::new(&mut port);
        let mut call = client.set_level(generated::Level::new_unchecked(100));
        assert_eq!(
            poll_once(&mut call),
            Poll::Ready(Err(ClientError::Send(SendError::Contract(
                Contract::PreconditionFailed
            )))),
            "a failing require is reported before anything is sent",
        );
    }

    // Nothing was sent: an otherwise-empty port has no claim for dispatch to
    // find.
    let mut provider = TestProvider::new(0);
    let mut buf = [0u8; generated::Cabin::MAX_BUFFER_SIZE];
    let settled = generated::cabin::dispatch(&mut port, &mut provider, &mut buf);
    assert_eq!(
        settled,
        Ok(0),
        "nothing was sent to the port, so dispatch settles nothing"
    );
    assert!(provider.set_level_calls.is_empty());
}

#[test]
fn round_trip_client_average_short_circuits_on_failing_require() {
    // 0 is a legal `Window` value ([0, 100000] inclusive) but fails the
    // query's own `require window > 0` clause. Sent through the generated
    // `Client::average` itself, so this pins the internal send's own short
    // circuit (face/poll.rs), not `dispatch`'s.
    let mut port = loopback();
    {
        let mut client = generated::cabin::Client::new(&mut port);
        let mut call = client.average(generated::Window::new_unchecked(0));
        assert_eq!(
            poll_once(&mut call),
            Poll::Ready(Err(ClientError::Send(SendError::Contract(
                Contract::PreconditionFailed
            )))),
            "a failing require is reported before anything is sent",
        );
    }

    // Nothing was sent: an otherwise-empty port has no claim for dispatch to
    // find.
    let mut provider = TestProvider::new(0);
    let mut buf = [0u8; generated::Cabin::MAX_BUFFER_SIZE];
    let settled = generated::cabin::dispatch(&mut port, &mut provider, &mut buf);
    assert_eq!(
        settled,
        Ok(0),
        "nothing was sent to the port, so dispatch settles nothing"
    );
}

#[test]
fn round_trip_failing_ensure_settles_contract_broken() {
    let mut rt = loopback();
    let handler = rt.handler();
    // The provider misbehaves: it returns a reply whose declared
    // `ensure result >= 0` clause is false. `Average`'s own declared range
    // [0, 1000] would never let a legally constructed value violate this —
    // Rust's newtype does not enforce it at construction — so this proves
    // the provider side checks `ensure` independently of the reply type's
    // own range.
    let mut provider = TestProvider::new(-1);
    let mut serve = generated::cabin::serve(handler, &mut provider);

    let mut client = generated::cabin::Client::new(&mut rt);
    let mut call = client.average(generated::Window::new_unchecked(1));
    assert!(poll_once(&mut call).is_pending());
    assert!(poll_once(&mut serve).is_pending());
    assert_eq!(
        poll_once(&mut call),
        Poll::Ready(Err(ClientError::Call(CallError::Contract(
            Contract::ContractBroken
        ))))
    );
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
    let first = send_level_raw(&mut port, 1);
    port.fail_next_settle();

    let mut provider = TestProvider::new(0);
    let mut buf = [0u8; generated::Cabin::MAX_BUFFER_SIZE];
    let settled_first = generated::cabin::dispatch(&mut port, &mut provider, &mut buf);
    assert_eq!(
        settled_first,
        Ok(0),
        "the first claim's settlement fails and dispatch does not count it"
    );
    assert_eq!(
        port.ack(first),
        None,
        "a failed settle records no outcome for the first claim's correlation"
    );

    let second = send_level_raw(&mut port, 2);
    let settled_second = generated::cabin::dispatch(&mut port, &mut provider, &mut buf);
    assert_eq!(
        settled_second,
        Ok(1),
        "the second claim's settlement succeeds and dispatch counts it"
    );
    assert_eq!(
        port.ack(second),
        Some(Ok(())),
        "the successful settlement is observable as accepted through ack"
    );

    // The provider still runs for both claims: `dispatch` settles a command
    // before calling the provider, unconditionally of whether the handler
    // accepted that settlement.
    assert_eq!(provider.set_level_calls, vec![1, 2]);
}

#[test]
fn round_trip_short_caller_buffer_returns_zero_without_consuming_a_claim() {
    let mut port = loopback();
    send_level_raw(&mut port, 1);

    let mut provider = TestProvider::new(0);

    // Too small: dispatch must return 0 and must not consume the claim.
    let mut short = [0u8; 1];
    let settled = generated::cabin::dispatch(&mut port, &mut provider, &mut short);
    assert_eq!(settled, Ok(0), "a short buffer settles nothing");
    assert!(provider.set_level_calls.is_empty());

    // Retrying with a correctly sized buffer still finds the claim the short
    // buffer left untouched.
    let mut buf = [0u8; generated::Cabin::MAX_BUFFER_SIZE];
    let settled = generated::cabin::dispatch(&mut port, &mut provider, &mut buf);
    assert_eq!(settled, Ok(1));
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
    assert_eq!(settled, Ok(1), "an outcome was still settled");
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
    assert_eq!(settled, Ok(1), "an outcome was still settled");
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
    assert_eq!(settled, Ok(1), "an outcome was still settled");
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
    assert_eq!(settled, Ok(1), "an outcome was still settled");
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
// The async client and `serve` (story E11.21, first half; the async face
// design, notes F-3 to F-7 and F-14). The port is `RecordingPorts`: the
// loopback's consumer-side role handles with a log of every `Caller` and
// `Wakeable` call, so the `Loopback` stays free for `advance`, for the
// handler `serve` runs over, and for a second caller that fills the call
// table. Each test names the sentence of the design it fails without.
// ---------------------------------------------------------------------------

/// `setLevel`'s response bound is `@[..50ms]`, in microseconds.
const SET_LEVEL_MAX: Duration = Duration(50_000);
/// `average`'s response bound is `@[..200ms]`.
const AVERAGE_MAX: Duration = Duration(200_000);

/// Fills every slot of the call table with a call no `serve` over `Cabin` is
/// presented — ordinal 99 names no member, and a handler is presented only
/// the members it serves — and returns the correlations, so a test can
/// reclaim one slot with `forget`.
fn fill_the_call_table(filler: &mut ridl_loopback::CallerHandle) -> Vec<Correlation> {
    (0..Loopback::SLOTS)
        .map(|_| {
            filler
                .command(CABIN, Ordinal(99), &[])
                .expect("a free slot")
        })
        .collect()
}

/// The correlation of the one send the log holds, which is what the method
/// did when it was called.
fn the_one_send(log: &doubles::Log) -> Correlation {
    match doubles::take(log).as_slice() {
        [Op::Command(Ok(c))] | [Op::Query(Ok(c))] => *c,
        ops => panic!("the method sends once when it is called, not {ops:?}"),
    }
}

/// A waker that counts its wakes, for the test that asserts a wake happened
/// rather than polling on a schedule.
struct CountWakes(AtomicUsize);

impl CountWakes {
    fn new() -> Arc<Self> {
        Arc::new(CountWakes(AtomicUsize::new(0)))
    }

    fn count(&self) -> usize {
        self.0.load(Ordering::SeqCst)
    }
}

impl Wake for CountWakes {
    fn wake(self: Arc<Self>) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

/// Note F-4: dropping the future while it waits calls `forget` once with its
/// correlation. Note F-5: a poll registers its interest, then reads.
#[test]
fn a_future_dropped_while_waiting_forgets_its_call_once() {
    let rt = loopback();
    let ports = RecordingPorts::new(&rt);
    let log = ports.log();
    let mut client = generated::cabin::Client::new(ports);

    let mut call = client.set_level(generated::Level::new_unchecked(42));
    let c = the_one_send(&log);
    assert!(
        poll_once(&mut call).is_pending(),
        "no provider has served the call"
    );
    assert_eq!(
        doubles::take(&log),
        vec![Op::WakeOn(Interest::Outcome(c)), Op::Ack(c)],
        "one poll registers its interest, then reads the outcome, and returns"
    );

    drop(call);
    assert_eq!(
        doubles::take(&log),
        vec![Op::Forget(c)],
        "dropping the future while it waits forgets the call, once"
    );
    drop(client);
    assert!(doubles::take(&log).is_empty(), "nothing else forgets it");
}

/// Note F-4: a future that has taken its outcome calls `forget` at once, in
/// the poll that took it, and nothing on drop.
#[test]
fn a_future_that_took_its_outcome_forgets_at_once_and_nothing_more_on_drop() {
    let rt = loopback();
    let mut provider = TestProvider::new(0);
    let mut serve = generated::cabin::serve(rt.handler(), &mut provider);
    let ports = RecordingPorts::new(&rt);
    let log = ports.log();
    let mut client = generated::cabin::Client::new(ports);

    let mut call = client.set_level(generated::Level::new_unchecked(42));
    let c = the_one_send(&log);
    assert!(
        poll_once(&mut serve).is_pending(),
        "serve settles the claim"
    );
    assert_eq!(poll_once(&mut call), Poll::Ready(Ok(())));
    assert_eq!(
        doubles::take(&log),
        vec![Op::WakeOn(Interest::Outcome(c)), Op::Ack(c), Op::Forget(c)],
        "the outcome is taken and the call forgotten in the same poll"
    );

    drop(call);
    assert!(
        doubles::take(&log).is_empty(),
        "a future that already forgot its call forgets nothing on drop"
    );
}

/// Note F-4: a future dropped in the slot-waiting phase sends nothing, so
/// there is nothing to forget. Note F-3: each poll retries the send, after
/// registering `Interest::Slot`.
#[test]
fn a_future_dropped_while_waiting_for_a_slot_forgets_nothing() {
    let rt = loopback();
    let mut filler = rt.caller();
    let _held = fill_the_call_table(&mut filler);
    let ports = RecordingPorts::new(&rt);
    let log = ports.log();
    let mut client = generated::cabin::Client::new(ports);

    let mut call = client.set_level(generated::Level::new_unchecked(42));
    assert_eq!(
        doubles::take(&log),
        vec![Op::Command(Err(SendError::Busy))],
        "the send is attempted once when the method runs"
    );
    assert!(poll_once(&mut call).is_pending());
    assert_eq!(
        doubles::take(&log),
        vec![
            Op::WakeOn(Interest::Slot),
            Op::Command(Err(SendError::Busy))
        ],
        "a poll registers the slot interest, then retries the send"
    );

    drop(call);
    assert!(
        doubles::take(&log).is_empty(),
        "nothing was sent, so nothing is forgotten"
    );
}

/// Note F-3: a call over a runtime with every slot taken is `Pending`, is
/// woken by a reclaimed slot, sends on the poll that follows, and resolves.
#[test]
fn a_call_with_every_slot_taken_waits_and_is_woken_by_a_reclaim() {
    let rt = loopback();
    let mut filler = rt.caller();
    let held = fill_the_call_table(&mut filler);
    let mut provider = TestProvider::new(0);
    let mut serve = generated::cabin::serve(rt.handler(), &mut provider);
    let ports = RecordingPorts::new(&rt);
    let log = ports.log();
    let mut client = generated::cabin::Client::new(ports);

    let wakes = CountWakes::new();
    let waker = Waker::from(Arc::clone(&wakes));
    let mut cx = Context::from_waker(&waker);

    let mut call = client.set_level(generated::Level::new_unchecked(42));
    assert!(Pin::new(&mut call).poll(&mut cx).is_pending());
    assert_eq!(
        wakes.count(),
        0,
        "no slot is free, so the registration is stored and not woken"
    );

    filler.forget(held[0]);
    let after_the_reclaim = wakes.count();
    assert!(
        after_the_reclaim >= 1,
        "the reclaimed slot wakes the waiting call"
    );

    assert!(
        Pin::new(&mut call).poll(&mut cx).is_pending(),
        "the retry sends; no provider has served the call yet"
    );
    match doubles::take(&log).as_slice() {
        [
            Op::Command(Err(SendError::Busy)),
            Op::WakeOn(Interest::Slot),
            Op::Command(Err(SendError::Busy)),
            Op::WakeOn(Interest::Slot),
            Op::Command(Ok(sent)),
            Op::WakeOn(Interest::Outcome(registered)),
            Op::Ack(read),
        ] if sent == registered && registered == read => {}
        ops => panic!("the send, the two polls and the wait on the outcome, not {ops:?}"),
    }
    // How many times the loopback woke the slot registration along the way
    // is the loopback's own count (it wakes a registration at once while a
    // slot is free, a spurious wake the contract allows); what the face owes
    // is that the outcome registration it made is woken by the settlement.
    let before_the_settlement = wakes.count();

    assert!(poll_once(&mut serve).is_pending());
    assert!(
        wakes.count() > before_the_settlement,
        "the settlement wakes the waiting call"
    );
    assert_eq!(Pin::new(&mut call).poll(&mut cx), Poll::Ready(Ok(())));
}

/// Note F-3: when the deadline passes with the call still unsent, the future
/// resolves to `Send(Busy)`, and nothing is forgotten because nothing was
/// sent. Note F-2: the bound is `max` from the port's clock at the call, and a
/// call at exactly `max` is still within it.
#[test]
fn a_call_still_unsent_at_its_deadline_resolves_to_send_busy() {
    let mut rt = loopback();
    let mut filler = rt.caller();
    let _held = fill_the_call_table(&mut filler);
    let ports = RecordingPorts::new(&rt);
    let log = ports.log();
    let mut client = generated::cabin::Client::new(ports);

    let mut call = client.set_level(generated::Level::new_unchecked(42));
    assert!(poll_once(&mut call).is_pending());
    rt.advance(SET_LEVEL_MAX);
    assert!(
        poll_once(&mut call).is_pending(),
        "at exactly the bound the call is still within it"
    );
    rt.advance(Duration(1));
    assert_eq!(
        poll_once(&mut call),
        Poll::Ready(Err(ClientError::Send(SendError::Busy)))
    );
    assert!(
        doubles::forgets(&log).is_empty(),
        "nothing was sent, so nothing is forgotten"
    );
}

/// Note F-3: a call still unsent when its deadline passes resolves to
/// `Send(Busy)` and is not sent, even when a slot frees after the deadline. A
/// send past the bound would be reported as `Undelivered`, which says "sent"
/// of a call the bound already refused.
#[test]
fn a_call_still_unsent_at_its_deadline_is_not_sent_when_a_slot_frees_later() {
    let mut rt = loopback();
    let mut filler = rt.caller();
    let held = fill_the_call_table(&mut filler);
    let ports = RecordingPorts::new(&rt);
    let log = ports.log();
    let mut client = generated::cabin::Client::new(ports);

    let mut call = client.set_level(generated::Level::new_unchecked(42));
    assert!(poll_once(&mut call).is_pending());
    rt.advance(SET_LEVEL_MAX);
    rt.advance(Duration(1));
    filler.forget(held[0]);
    doubles::take(&log);

    assert_eq!(
        poll_once(&mut call),
        Poll::Ready(Err(ClientError::Send(SendError::Busy)))
    );
    let after_the_deadline = doubles::take(&log);
    assert!(
        !after_the_deadline
            .iter()
            .any(|op| matches!(op, Op::Command(Ok(_)))),
        "the freed slot must not be taken past the deadline, but the log holds \
         {after_the_deadline:?}"
    );
    assert!(
        doubles::forgets(&log).is_empty(),
        "nothing was sent, so nothing is forgotten"
    );
}

/// Note F-3: a sent command whose clock passes `max` resolves to
/// `Undelivered` and forgets, once.
#[test]
fn a_sent_command_resolves_to_undelivered_at_its_deadline_and_forgets() {
    let mut rt = loopback();
    let ports = RecordingPorts::new(&rt);
    let log = ports.log();
    let mut client = generated::cabin::Client::new(ports);

    let mut call = client.set_level(generated::Level::new_unchecked(42));
    let c = the_one_send(&log);
    assert!(poll_once(&mut call).is_pending());
    rt.advance(SET_LEVEL_MAX);
    rt.advance(Duration(1));
    assert_eq!(
        poll_once(&mut call),
        Poll::Ready(Err(ClientError::Call(CallError::Transport(
            Transport::Undelivered
        ))))
    );
    assert_eq!(doubles::forgets(&log), vec![c]);

    drop(call);
    assert_eq!(doubles::forgets(&log), vec![c], "and not again on drop");
}

/// Note F-3: a sent query whose clock passes `max` resolves to `Timeout` and
/// forgets, once.
#[test]
fn a_sent_query_resolves_to_timeout_at_its_deadline_and_forgets() {
    let mut rt = loopback();
    let ports = RecordingPorts::new(&rt);
    let log = ports.log();
    let mut client = generated::cabin::Client::new(ports);

    let mut call = client.average(generated::Window::new_unchecked(10));
    let c = the_one_send(&log);
    assert!(poll_once(&mut call).is_pending());
    rt.advance(AVERAGE_MAX);
    rt.advance(Duration(1));
    assert_eq!(
        poll_once(&mut call),
        Poll::Ready(Err(ClientError::Call(CallError::Transport(
            Transport::Timeout
        ))))
    );
    assert_eq!(doubles::forgets(&log), vec![c]);

    drop(call);
    assert_eq!(doubles::forgets(&log), vec![c], "and not again on drop");
}

/// Note F-10: `next_event` waits on the event queue and takes the occurrence
/// a raise delivers on the poll that follows it.
#[test]
fn round_trip_next_event_waits_for_a_raise() {
    let mut rt = loopback();
    let ports = RecordingPorts::new(&rt);
    let log = ports.log();
    let mut client = generated::cabin::Client::new(ports);
    client.subscribe_warning().expect("subscribe");

    let mut next = client.next_event();
    assert!(poll_once(&mut next).is_pending(), "nothing was raised");
    assert_eq!(
        doubles::take(&log),
        vec![Op::WakeOn(Interest::Event(CABIN))],
        "the poll registers the event interest before it reads the queue"
    );

    generated::cabin::Publisher::new(&mut rt)
        .warning(generated::Warning {
            code: generated::Level::new_unchecked(5),
            health: generated::Health::Warn,
        })
        .expect("raise");
    let Poll::Ready(Ok(generated::cabin::Event::Warning(occurrence))) = poll_once(&mut next) else {
        panic!("the raise is delivered on the next poll");
    };
    assert_eq!(occurrence.payload.expect("payload verifies").code.get(), 5);
}

/// Note F-7: `serve` calls `Handler::serve` with the interface's command and
/// query ordinals when the function is called.
#[test]
fn serve_registers_the_interfaces_commands_and_queries_with_the_handler() {
    use ridl_rt::contract::Interaction;

    let rt = loopback();
    let mut handler = rt.handler();
    let mut provider = TestProvider::new(0);
    {
        let mut serve = generated::cabin::serve(&mut handler, &mut provider);
        assert!(poll_once(&mut serve).is_pending(), "no claim is waiting");
    }
    assert_eq!(
        handler.served(),
        &[
            (
                CABIN,
                <generated::CabinSetLevel as Interaction>::MEMBER.ordinal
            ),
            (
                CABIN,
                <generated::CabinAverage as Interaction>::MEMBER.ordinal
            ),
        ]
    );
}

/// Note F-7: `serve` over a handler whose `next_claim` fails resolves to
/// `ProviderError::Claim`, and every claim settled before the failure stays
/// settled.
#[test]
fn serve_resolves_to_the_handler_ports_failure_after_settling_the_claims_before_it() {
    let mut rt = loopback();
    let first = send_level_raw(&mut rt, 1);
    let second = send_level_raw(&mut rt, 2);
    let handler = FailingHandler::failing_after(rt.handler(), 1);
    let mut provider = TestProvider::new(0);
    let mut serve = generated::cabin::serve(handler, &mut provider);

    assert_eq!(
        poll_once(&mut serve),
        Poll::Ready(Err(ProviderError::Claim(ReadError::Detached)))
    );

    drop(serve);
    assert_eq!(
        provider.set_level_calls,
        vec![1],
        "the claim presented before the failure was served"
    );
    assert_eq!(rt.ack(first), Some(Ok(())), "and settled");
    assert_eq!(
        rt.ack(second),
        None,
        "the claim behind the failure is still waiting"
    );
}

/// Note F-7: a refused `Handler::serve` is a future ready with the refusal.
#[test]
fn serve_is_ready_with_the_refusal_when_the_handler_refuses_the_members() {
    let rt = loopback();
    let handler = FailingHandler::refusing(rt.handler(), ServeError::NotOwner);
    let mut provider = TestProvider::new(0);
    let mut serve = generated::cabin::serve(handler, &mut provider);

    assert_eq!(
        poll_once(&mut serve),
        Poll::Ready(Err(ProviderError::Serve(ServeError::NotOwner)))
    );
}

/// Note F-2: a member with no `max` has no deadline. `Valve`'s calls declare
/// no response bound, so a call waits past any time and still delivers.
#[test]
fn a_call_on_a_member_with_no_max_waits_without_a_bound() {
    let mut rt = loopback();
    let mut provider = TestProvider::new(3);
    let mut serve = generated::valve::serve(rt.handler(), &mut provider);
    let ports = RecordingPorts::new(&rt);
    let log = ports.log();
    let mut client = generated::valve::Client::new(ports);

    let mut call = client.pressure(generated::Window::new_unchecked(10));
    let c = the_one_send(&log);
    assert!(poll_once(&mut call).is_pending());
    rt.advance(Duration(i64::MAX / 2));
    assert!(
        poll_once(&mut call).is_pending(),
        "no bound passes for a member with no max"
    );
    assert!(poll_once(&mut serve).is_pending());
    assert_eq!(
        poll_once(&mut call),
        Poll::Ready(Ok(generated::Average::new_unchecked(3)))
    );
    assert_eq!(doubles::forgets(&log), vec![c]);
}

/// Note F-2: the outcome wins over the deadline. A command settled within its
/// bound and polled only after the bound passed delivers its acknowledgment,
/// not `Undelivered`.
#[test]
fn a_settled_command_delivers_its_acknowledgment_after_its_deadline_passed() {
    let mut rt = loopback();
    let mut provider = TestProvider::new(0);
    let mut serve = generated::cabin::serve(rt.handler(), &mut provider);
    let ports = RecordingPorts::new(&rt);
    let log = ports.log();
    let mut client = generated::cabin::Client::new(ports);

    let mut call = client.set_level(generated::Level::new_unchecked(42));
    let c = the_one_send(&log);
    assert!(
        poll_once(&mut serve).is_pending(),
        "serve settles the claim"
    );
    rt.advance(SET_LEVEL_MAX);
    rt.advance(Duration(1));
    assert_eq!(poll_once(&mut call), Poll::Ready(Ok(())));
    assert_eq!(doubles::forgets(&log), vec![c]);
}

/// The same for a query: the reply, not `Timeout`.
#[test]
fn a_settled_query_delivers_its_reply_after_its_deadline_passed() {
    let mut rt = loopback();
    let mut provider = TestProvider::new(7);
    let mut serve = generated::cabin::serve(rt.handler(), &mut provider);
    let ports = RecordingPorts::new(&rt);
    let log = ports.log();
    let mut client = generated::cabin::Client::new(ports);

    let mut call = client.average(generated::Window::new_unchecked(10));
    let c = the_one_send(&log);
    assert!(
        poll_once(&mut serve).is_pending(),
        "serve settles the claim"
    );
    rt.advance(AVERAGE_MAX);
    rt.advance(Duration(1));
    assert_eq!(
        poll_once(&mut call),
        Poll::Ready(Ok(generated::Average::new_unchecked(7)))
    );
    assert_eq!(doubles::forgets(&log), vec![c]);
}

/// Note F-4: a send failure other than `Busy` on the slot-wait retry is a
/// future ready with that failure; nothing was sent, so nothing is forgotten.
#[test]
fn a_send_failure_other_than_busy_on_the_retry_resolves_the_call() {
    let rt = loopback();
    let mut filler = rt.caller();
    let _held = fill_the_call_table(&mut filler);
    let ports = RecordingPorts::new(&rt);
    let log = ports.log();
    let faults = ports.faults();
    let mut client = generated::cabin::Client::new(ports);

    let mut call = client.set_level(generated::Level::new_unchecked(42));
    assert_eq!(
        doubles::take(&log),
        vec![Op::Command(Err(SendError::Busy))],
        "the call waits for a slot"
    );
    faults.send.set(Some(SendError::Detached));
    assert_eq!(
        poll_once(&mut call),
        Poll::Ready(Err(ClientError::Send(SendError::Detached)))
    );
    assert!(
        doubles::forgets(&log).is_empty(),
        "nothing was sent, so nothing is forgotten"
    );
}

/// Note F-10: a read failure resolves `NextEvent` with that failure.
#[test]
fn next_event_resolves_with_the_read_failure() {
    let rt = loopback();
    let ports = RecordingPorts::new(&rt);
    let faults = ports.faults();
    let mut client = generated::cabin::Client::new(ports);
    client.subscribe_warning().expect("subscribe");

    let mut next = client.next_event();
    faults.next.set(Some(ReadError::Detached));
    assert!(
        matches!(poll_once(&mut next), Poll::Ready(Err(ReadError::Detached))),
        "the read's failure is the future's output"
    );
}

/// A resolved call future panics when it is polled again, as an `async fn`'s
/// future does.
#[test]
#[should_panic(expected = "polled after completion")]
fn a_resolved_call_future_panics_when_polled_again() {
    let mut rt = loopback();
    let mut client = generated::cabin::Client::new(&mut rt);
    // 100 fails `require level < 100`, so the future is ready on its first
    // poll without a provider.
    let mut call = client.set_level(generated::Level::new_unchecked(100));
    assert!(poll_once(&mut call).is_ready());
    let _ = poll_once(&mut call);
}

/// A resolved `Serve` panics when it is polled again.
#[test]
#[should_panic(expected = "polled after completion")]
fn a_resolved_serve_future_panics_when_polled_again() {
    let rt = loopback();
    let handler = FailingHandler::refusing(rt.handler(), ServeError::NotOwner);
    let mut provider = TestProvider::new(0);
    let mut serve = generated::cabin::serve(handler, &mut provider);
    assert!(poll_once(&mut serve).is_ready());
    let _ = poll_once(&mut serve);
}

/// Note F-7: each poll of `Serve` registers `Interest::Claim` before it drains
/// the handler, so a call sent while it is pending wakes it.
#[test]
fn a_send_wakes_a_pending_serve() {
    let mut rt = loopback();
    let mut provider = TestProvider::new(0);
    let mut serve = generated::cabin::serve(rt.handler(), &mut provider);
    let wakes = CountWakes::new();
    let waker = Waker::from(Arc::clone(&wakes));
    let mut cx = Context::from_waker(&waker);

    assert!(Pin::new(&mut serve).poll(&mut cx).is_pending());
    assert_eq!(
        wakes.count(),
        0,
        "no claim is waiting, so the registration is stored and not woken"
    );
    send_level_raw(&mut rt, 1);
    assert_eq!(
        wakes.count(),
        1,
        "the send wakes the handler that serves the member"
    );
    assert!(Pin::new(&mut serve).poll(&mut cx).is_pending());
    drop(serve);
    assert_eq!(provider.set_level_calls, vec![1]);
}

/// The same for a query: a send failure other than `Busy` on the slot-wait
/// retry resolves `AverageCall` with that failure (driftsys/ridl#571).
#[test]
fn a_send_failure_other_than_busy_on_the_retry_resolves_the_query() {
    let rt = loopback();
    let mut filler = rt.caller();
    let _held = fill_the_call_table(&mut filler);
    let ports = RecordingPorts::new(&rt);
    let log = ports.log();
    let faults = ports.faults();
    let mut client = generated::cabin::Client::new(ports);

    let mut call = client.average(generated::Window::new_unchecked(10));
    assert_eq!(
        doubles::take(&log),
        vec![Op::Query(Err(SendError::Busy))],
        "the query waits for a slot"
    );
    faults.send.set(Some(SendError::Detached));
    assert_eq!(
        poll_once(&mut call),
        Poll::Ready(Err(ClientError::Send(SendError::Detached)))
    );
    assert!(
        doubles::forgets(&log).is_empty(),
        "nothing was sent, so nothing is forgotten"
    );
}

/// A `Serve` that resolved with the handler port's failure panics when it is
/// polled again, as one that resolved with a refusal does
/// (driftsys/ridl#571).
#[test]
#[should_panic(expected = "polled after completion")]
fn a_serve_future_resolved_by_the_handlers_failure_panics_when_polled_again() {
    let rt = loopback();
    let handler = FailingHandler::failing_after(rt.handler(), 0);
    let mut provider = TestProvider::new(0);
    let mut serve = generated::cabin::serve(handler, &mut provider);
    assert_eq!(
        poll_once(&mut serve),
        Poll::Ready(Err(ProviderError::Claim(ReadError::Detached)))
    );
    let _ = poll_once(&mut serve);
}

/// Note F-4: a call future is `Send` when its port is, and `Unpin` by its
/// fields; `Serve` is both over a `Send` handler and provider. The blocking
/// client is `Send` when its port is, so a thread can own one.
const _: () = {
    const fn assert_send<T: Send>() {}
    const fn assert_unpin<T: Unpin>() {}
    assert_send::<generated::cabin::SetLevelCall<'static, Loopback>>();
    assert_send::<generated::cabin::AverageCall<'static, Loopback>>();
    assert_send::<generated::cabin::NextEvent<'static, Loopback>>();
    assert_send::<generated::cabin::Serve<'static, ridl_loopback::HandlerHandle, TestProvider>>();
    assert_send::<generated::cabin::blocking::Client<Loopback>>();
    assert_unpin::<generated::cabin::SetLevelCall<'static, Loopback>>();
    assert_unpin::<generated::cabin::AverageCall<'static, Loopback>>();
    assert_unpin::<generated::cabin::NextEvent<'static, Loopback>>();
    assert_unpin::<generated::cabin::Serve<'static, ridl_loopback::HandlerHandle, TestProvider>>();
};

// ---------------------------------------------------------------------------
// The blocking client and `blocking::serve` (story E11.21, second half; the
// async face design, notes F-10, F-11 and F-14). Each blocking call is
// `ridl_rt::task::block_on` over the async call's future, so the calling
// thread parks until a wake or the client's own timeout. The provider side
// therefore runs on a second thread where a call is served, and where it is
// not, the test reads what the client answers at its timeout. The loopback's
// clock is hand-driven and never advanced here, so a member's `max` never
// passes inside the future: every timed answer below comes from the client's
// timeout, which is what note F-11 says a timeout shorter than `max` does.
// ---------------------------------------------------------------------------

/// A timeout long enough that a served call never reaches it, and short
/// enough that a test which fails does not hang the suite.
const GENEROUS: std::time::Duration = std::time::Duration::from_secs(10);
/// The blocking client's timeout in the tests that reach it.
const SHORT: std::time::Duration = std::time::Duration::from_millis(20);

/// The correlation of the first send in `ops`, and every correlation
/// `forget` was called with, in order.
fn first_send_and_forgets(ops: &[Op]) -> (Correlation, Vec<Correlation>) {
    let sent = match ops.first() {
        Some(Op::Command(Ok(c))) | Some(Op::Query(Ok(c))) => *c,
        _ => panic!("the call is sent when the method runs, not {ops:?}"),
    };
    let forgets = ops
        .iter()
        .filter_map(|op| match op {
            Op::Forget(c) => Some(*c),
            _ => None,
        })
        .collect();
    (sent, forgets)
}

/// Note F-14: the cabin round trip passes through the blocking client. The
/// provider side is `blocking::serve` on a second thread, over a handler that
/// fails once two claims were presented, so that thread ends with the failure
/// after the two calls rather than at its timeout, and the join returns at
/// once. Note F-7 for the blocking form: the claims settled before the
/// failure stay settled, and the failure is what `blocking::serve` returns.
#[test]
fn blocking_round_trip_command_and_query_are_served_from_another_thread() {
    let mut rt = loopback();
    let handler = FailingHandler::failing_after(rt.handler(), 2);
    let mut provider = TestProvider::new(7);

    let (acknowledged, reply, served) = std::thread::scope(|scope| {
        let serving = scope
            .spawn(|| generated::cabin::blocking::serve(handler, &mut provider, Some(GENEROUS)));
        let mut client = generated::cabin::blocking::Client::new(&mut rt).with_timeout(GENEROUS);
        let acknowledged = client.set_level(generated::Level::new_unchecked(42));
        let reply = client.average(generated::Window::new_unchecked(10));
        let served = serving.join().expect("the serving thread does not panic");
        (acknowledged, reply, served)
    });

    assert_eq!(acknowledged, Ok(()));
    assert_eq!(reply, Ok(generated::Average::new_unchecked(7)));
    assert_eq!(
        served,
        Err(ProviderError::Claim(ReadError::Detached)),
        "the handler failed after the two claims, and serve returned that failure"
    );
    assert_eq!(provider.set_level_calls, vec![42]);
}

/// Note F-11, the unsent phase: with every slot taken, the blocking call
/// gives up at the client's timeout with `Send(Busy)`, the answer the future
/// gives at its own deadline; nothing was sent, so nothing is forgotten. The
/// member is `Valve::open`, which has no `max`, so the client's timeout is
/// the only bound. `set_timeout` is the setter here, `with_timeout` in the
/// other tests.
#[test]
fn a_blocking_call_still_unsent_at_its_timeout_returns_send_busy() {
    let rt = loopback();
    let mut filler = rt.caller();
    let _held = fill_the_call_table(&mut filler);
    let ports = RecordingPorts::new(&rt);
    let log = ports.log();
    let mut client = generated::valve::blocking::Client::new(ports);
    client.set_timeout(Some(SHORT));

    let started = std::time::Instant::now();
    assert_eq!(
        client.open(generated::Level::new_unchecked(42)),
        Err(ClientError::Send(SendError::Busy))
    );
    assert!(
        started.elapsed() >= SHORT,
        "the client waits its whole timeout before it gives up"
    );
    assert!(
        doubles::forgets(&log).is_empty(),
        "nothing was sent, so nothing is forgotten"
    );
}

/// Note F-11, the sent phase, a command: `Undelivered` at the client's
/// timeout, and the call is forgotten once, when the future is dropped.
#[test]
fn a_blocking_command_sent_but_unserved_returns_undelivered_at_its_timeout_and_forgets() {
    let rt = loopback();
    let ports = RecordingPorts::new(&rt);
    let log = ports.log();
    let mut client = generated::valve::blocking::Client::new(ports).with_timeout(SHORT);

    assert_eq!(
        client.open(generated::Level::new_unchecked(42)),
        Err(ClientError::Call(CallError::Transport(
            Transport::Undelivered
        )))
    );
    let (sent, forgets) = first_send_and_forgets(&doubles::take(&log));
    assert_eq!(
        forgets,
        vec![sent],
        "forgotten once, when the future is dropped"
    );
}

/// Note F-11, the sent phase, a query: `Timeout` at the client's timeout, and
/// the call is forgotten once.
#[test]
fn a_blocking_query_sent_but_unserved_returns_timeout_at_its_timeout_and_forgets() {
    let rt = loopback();
    let ports = RecordingPorts::new(&rt);
    let log = ports.log();
    let mut client = generated::valve::blocking::Client::new(ports).with_timeout(SHORT);

    assert_eq!(
        client.pressure(generated::Window::new_unchecked(10)),
        Err(ClientError::Call(CallError::Transport(Transport::Timeout)))
    );
    let (sent, forgets) = first_send_and_forgets(&doubles::take(&log));
    assert_eq!(
        forgets,
        vec![sent],
        "forgotten once, when the future is dropped"
    );
}

/// Note F-11: a client timeout shorter than the member's `max` is accepted.
/// `setLevel`'s `max` is 50 ms on the loopback's clock, which never advances
/// here, so the future's deadline cannot pass; the answer at 20 ms is the
/// client's.
#[test]
fn a_blocking_timeout_shorter_than_the_members_max_is_accepted() {
    let mut rt = loopback();
    let mut client = generated::cabin::blocking::Client::new(&mut rt).with_timeout(SHORT);

    let started = std::time::Instant::now();
    assert_eq!(
        client.set_level(generated::Level::new_unchecked(42)),
        Err(ClientError::Call(CallError::Transport(
            Transport::Undelivered
        )))
    );
    assert!(started.elapsed() >= SHORT);
}

/// Note F-11: `blocking::next_event` returns `Ok(None)` at the timeout when
/// no occurrence arrived, and `Ok(Some)` when one is raised, here from
/// another thread, which is what a wake across threads is for.
#[test]
fn blocking_next_event_returns_none_at_its_timeout_and_the_occurrence_when_raised() {
    let mut rt = loopback();
    // The client is built over the role handles, so the `Loopback` stays
    // free for the thread that raises.
    let ports = RecordingPorts::new(&rt);
    let mut client = generated::cabin::blocking::Client::new(ports).with_timeout(SHORT);
    client.subscribe_warning().expect("subscribe");

    assert!(
        matches!(client.next_event(), Ok(None)),
        "nothing was raised, so the timeout answers"
    );

    client.set_timeout(Some(GENEROUS));
    let event = std::thread::scope(|scope| {
        scope.spawn(|| {
            std::thread::sleep(SHORT);
            generated::cabin::Publisher::new(&mut rt)
                .warning(generated::Warning {
                    code: generated::Level::new_unchecked(5),
                    health: generated::Health::Warn,
                })
                .expect("raise");
        });
        client.next_event()
    });
    let Ok(Some(generated::cabin::Event::Warning(occurrence))) = event else {
        panic!("the raise wakes the waiting client, which answers the occurrence");
    };
    assert_eq!(
        occurrence.payload.expect("the payload verifies").code.get(),
        5
    );
}

/// Note F-11: `blocking::serve` returns `Ok(())` at its timeout, so a loop
/// that also does other work can call it repeatedly; a refused
/// `Handler::serve` is returned at once.
#[test]
fn blocking_serve_returns_ok_at_its_timeout_and_the_refusal_at_once() {
    let rt = loopback();
    let mut provider = TestProvider::new(0);

    let started = std::time::Instant::now();
    assert_eq!(
        generated::cabin::blocking::serve(rt.handler(), &mut provider, Some(SHORT)),
        Ok(())
    );
    assert!(started.elapsed() >= SHORT, "serve waits its whole timeout");

    let handler = FailingHandler::refusing(rt.handler(), ServeError::NotOwner);
    assert_eq!(
        generated::cabin::blocking::serve(handler, &mut provider, Some(GENEROUS)),
        Err(ProviderError::Serve(ServeError::NotOwner))
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
