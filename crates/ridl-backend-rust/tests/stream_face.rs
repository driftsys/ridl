//! The caller side of a streamed reply of named elements, over
//! `ridl-loopback` (`docs/design/interaction-face.md`).
//!
//! It brings in the checked-in generated face of `tests/fixtures/stream_face.ridl`
//! and drives the `Client` of `Log`, whose `history` replies with a stream of
//! `Reading`. The provider side of such a query is not generated yet, so each
//! test drives it by hand through the `StreamHandler` port of a loopback
//! handler handle: it takes the claim, pushes encoded elements, and ends or
//! aborts the stream. What the tests pin is the stream value's contract: the
//! order of the elements and the end, the first chunk kept by the call's
//! future, the registration of the waker, the refusal of an element that
//! fails its check, and the one `forget` a drop or the end makes.
//!
//! The futures and the stream are polled by hand with
//! `ridl_rt::task::noop_waker`, the way a frame loop polls, and the tests of
//! what the face does at the port use `RecordingPorts`, the double of
//! `tests/support/doubles.rs`, whose log shows each call the face made.

#[allow(
    dead_code,
    reason = "the doubles are shared with tests/interaction_face.rs, which uses the handler \
              doubles and the injected faults this file does not"
)]
mod support;

use std::future::Future;
use std::pin::{Pin, pin};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::task::{Context, Poll, Wake, Waker};

use ridl_loopback::{HandlerHandle, Loopback};
use ridl_rt::contract::{CatalogRef, Interaction, Interface, InterfaceNo, Ordinal, Sizes};
use ridl_rt::encoding::FlatBuffers;
use ridl_rt::error::{CallError, ClientError, Contract, Transport};
use ridl_rt::payload::{Ref, Rule, Violation};
use ridl_rt::port::{
    Attached, Caller, ClaimId, Clock, Correlation, Encoded, Handler, Interest, PushError,
    REPLY_STREAM, ReadError, SendError, StreamHandler, Wakeable,
};
use ridl_rt::sample::{Duration, Timestamp};
use ridl_rt::trace::TraceContext;

use support::doubles::{self, Op, RecordingPorts};

/// The checked-in output of `generate_face_with` over
/// `tests/fixtures/stream_face.ridl`. It is `include!`d, never hand-edited;
/// `tests/interaction_face_regeneration.rs` compares it byte-for-byte
/// against a fresh `generate_face_with` call.
#[allow(
    dead_code,
    reason = "not every generated item is named from this file's own tests; \
              the byte-equality guard is what pins the emitter's output"
)]
#[allow(
    unused_imports,
    reason = "each interface module's `prelude` is a set of `pub use` re-exports, which \
              rustc reports as unused inside this private module when this file's tests \
              glob-import no prelude of that interface"
)]
mod generated {
    include!("generated/stream_face.rs");
}

use generated::log::prelude::*;
use generated::{Level, Reading};

/// The catalog the runtime is built with: the generated `CATALOG` of the
/// fixture's package.
const CATALOG: CatalogRef = *<generated::Log as Interface>::CATALOG;
const LOG: InterfaceNo = <generated::Log as Interface>::NUMBER;
const HISTORY: Ordinal = <generated::LogHistory as Interaction>::MEMBER.ordinal;
/// `history`'s response bound is `@[..200ms]`, in microseconds.
const HISTORY_MAX: Duration = Duration(200_000);
/// The interface's claim-buffer maximum, read from its size table.
const CLAIM_MAX: usize = <generated::Log as Sizes<FlatBuffers>>::MAX_BUFFER_SIZE;
/// Room for one encoded element, from the element's own bound.
const ELEMENT_MAX: usize = <Reading as ridl_rt::payload::Payload<FlatBuffers>>::MAX_SIZE;

fn loopback() -> Loopback {
    Loopback::new(CATALOG)
}

/// Polls `future` once with the no-op waker, the way a frame loop does.
fn poll_once<F: Future + Unpin>(future: &mut F) -> Poll<F::Output> {
    let waker = ridl_rt::task::noop_waker();
    let mut cx = Context::from_waker(&waker);
    Pin::new(future).poll(&mut cx)
}

/// Polls the stream once with the no-op waker.
fn next_once<P>(
    stream: &mut generated::log::HistoryStream<'_, P>,
) -> Poll<Option<Result<Reading, ClientError>>>
where
    P: Encoded<Encoding = FlatBuffers> + ridl_rt::port::StreamCaller + Clock + Wakeable,
{
    let waker = ridl_rt::task::noop_waker();
    let mut cx = Context::from_waker(&waker);
    stream.poll_next(&mut cx)
}

fn reading(level: i64, flag: bool) -> Reading {
    Reading {
        level: Level::new_unchecked(level),
        flag,
    }
}

/// The provider of `history`, driven by hand over the `StreamHandler` port of
/// a handler handle: the generated `serve` has no arm for a streamed reply
/// yet. It serves `history` alone, takes the one claim a test sends, and
/// pushes, ends or aborts the reply stream.
struct Provider {
    handler: HandlerHandle,
    claim: Option<ClaimId>,
}

impl Provider {
    fn new(rt: &Loopback) -> Self {
        let mut handler = rt.handler();
        handler.serve(LOG, &[HISTORY]).expect("serve history");
        Provider {
            handler,
            claim: None,
        }
    }

    /// Takes the waiting claim, once; a later call is the same claim.
    fn claim(&mut self) -> ClaimId {
        if let Some(claim) = self.claim {
            return claim;
        }
        let mut buf = [0u8; CLAIM_MAX];
        let claim = self
            .handler
            .next_claim(&mut buf)
            .expect("next_claim")
            .expect("a call is waiting")
            .id;
        self.claim = Some(claim);
        claim
    }

    /// Pushes one element, encoded by the generated FlatBuffers codec. The
    /// encoder does not check a named scalar's range, so an out-of-range
    /// `Level` built with `new_unchecked` is pushed as a well-formed table.
    fn push(&mut self, element: &Reading) -> Result<usize, PushError> {
        let mut buf = [0u8; ELEMENT_MAX];
        let bytes = Ref::<Reading, FlatBuffers>::encode(element, &mut buf)
            .expect("encode")
            .bytes();
        let claim = self.claim();
        self.handler.push_claim(claim, REPLY_STREAM, bytes)
    }

    fn end(&mut self) -> Result<(), PushError> {
        let claim = self.claim();
        self.handler.end_claim(claim, REPLY_STREAM)
    }

    fn abort(&mut self, error: CallError) -> Result<(), PushError> {
        let claim = self.claim();
        self.handler.abort_claim(claim, REPLY_STREAM, error)
    }
}

/// `history(3)` sent through `client`, with the claim taken by `provider`
/// and `first` pushed, polled to its stream: the way most tests start.
fn started<'a, P>(
    client: &'a mut generated::log::Client<P>,
    provider: &mut Provider,
    first: &Reading,
) -> generated::log::HistoryStream<'a, P>
where
    P: Encoded<Encoding = FlatBuffers> + ridl_rt::port::StreamCaller + Clock + Wakeable,
{
    let mut call = client.history(Level::new_unchecked(3));
    assert!(
        poll_once(&mut call).is_pending(),
        "the query was sent, and no element arrived"
    );
    assert_eq!(
        provider.push(first),
        Ok(element_len()),
        "the first element is taken"
    );
    resolved(poll_once(&mut call), "the first element")
}

/// The stream a resolved call gives, or a failure naming `what` should have
/// resolved it; the stream has no `Debug`, so the failure is spelled out.
fn resolved<'a, P>(
    polled: Poll<Result<generated::log::HistoryStream<'a, P>, ClientError>>,
    what: &str,
) -> generated::log::HistoryStream<'a, P>
where
    P: Encoded<Encoding = FlatBuffers> + ridl_rt::port::StreamCaller + Clock + Wakeable,
{
    match polled {
        Poll::Ready(Ok(stream)) => stream,
        Poll::Ready(Err(error)) => panic!("{what} resolves the call to its stream, not {error:?}"),
        Poll::Pending => panic!("{what} resolves the call to its stream; it is pending"),
    }
}

/// The encoded length of a `Reading`, which every element here has: a
/// table of two fixed-width fields, the same length whatever the values. A
/// push that took the chunk answers it.
fn element_len() -> usize {
    let mut buf = [0u8; ELEMENT_MAX];
    Ref::<Reading, FlatBuffers>::encode(&reading(1, true), &mut buf)
        .expect("encode")
        .bytes()
        .len()
}

// ---------------------------------------------------------------------------
// The elements, the end and the failures, over the loopback aggregate.
// ---------------------------------------------------------------------------

#[test]
fn round_trip_the_elements_arrive_in_order_and_the_stream_ends() {
    let mut rt = loopback();
    let mut provider = Provider::new(&rt);
    let mut client = generated::log::Client::new(&mut rt);
    let elements = [reading(1, false), reading(2, true), reading(3, false)];

    let mut stream = started(&mut client, &mut provider, &elements[0]);
    assert_eq!(provider.push(&elements[1]), Ok(element_len()));
    assert_eq!(provider.push(&elements[2]), Ok(element_len()));
    assert_eq!(provider.end(), Ok(()));

    for element in &elements {
        assert_eq!(next_once(&mut stream), Poll::Ready(Some(Ok(*element))));
    }
    assert_eq!(next_once(&mut stream), Poll::Ready(None), "the end");
    assert_eq!(
        next_once(&mut stream),
        Poll::Ready(None),
        "after the end every poll is None"
    );
}

#[test]
fn the_first_element_is_kept_and_handed_out_by_the_first_poll_next() {
    let mut rt = loopback();
    let mut provider = Provider::new(&rt);
    let mut client = generated::log::Client::new(&mut rt);
    let first = reading(7, true);

    let mut stream = started(&mut client, &mut provider, &first);
    // Nothing else was pushed: the only element the stream can give is the
    // one the call's future pulled.
    assert_eq!(
        next_once(&mut stream),
        Poll::Ready(Some(Ok(first))),
        "the chunk the future pulled is the first element"
    );
    assert!(
        next_once(&mut stream).is_pending(),
        "no second element arrived"
    );
}

#[test]
fn more_elements_than_the_window_arrive_in_order() {
    let mut rt = loopback();
    let mut provider = Provider::new(&rt);
    let mut client = generated::log::Client::new(&mut rt);
    let elements: Vec<Reading> = (1..=6).map(|i| reading(i, i % 2 == 0)).collect();

    let mut stream = started(&mut client, &mut provider, &elements[0]);
    // The loopback's window holds four chunks; the fifth push is not taken
    // until the consumer pulls one.
    for element in &elements[1..=4] {
        assert_eq!(provider.push(element), Ok(element_len()));
    }
    assert_eq!(provider.push(&elements[5]), Ok(0), "the window is full");
    assert_eq!(next_once(&mut stream), Poll::Ready(Some(Ok(elements[0]))));
    assert_eq!(next_once(&mut stream), Poll::Ready(Some(Ok(elements[1]))));
    assert_eq!(
        provider.push(&elements[5]),
        Ok(element_len()),
        "a pull freed the window"
    );
    assert_eq!(provider.end(), Ok(()));
    for element in &elements[2..] {
        assert_eq!(next_once(&mut stream), Poll::Ready(Some(Ok(*element))));
    }
    assert_eq!(next_once(&mut stream), Poll::Ready(None));
}

#[test]
fn an_end_before_any_element_resolves_the_call_to_an_empty_stream() {
    let mut rt = loopback();
    let mut provider = Provider::new(&rt);
    let mut client = generated::log::Client::new(&mut rt);

    let mut call = client.history(Level::new_unchecked(3));
    assert!(poll_once(&mut call).is_pending());
    assert_eq!(provider.end(), Ok(()));
    let mut stream = resolved(poll_once(&mut call), "the end");
    assert_eq!(next_once(&mut stream), Poll::Ready(None));
}

#[test]
fn a_failure_before_any_element_resolves_the_call_to_that_error() {
    let mut rt = loopback();
    let mut provider = Provider::new(&rt);
    let mut client = generated::log::Client::new(&mut rt);
    let refusal = CallError::Contract(Contract::PreconditionFailed);

    let mut call = client.history(Level::new_unchecked(3));
    assert!(poll_once(&mut call).is_pending());
    assert_eq!(provider.abort(refusal), Ok(()));
    assert_eq!(
        poll_once(&mut call).map(|outcome| outcome.map(|_| ())),
        Poll::Ready(Err(ClientError::Call(refusal))),
        "a failure before the first element is the call's outcome"
    );
}

#[test]
fn an_interruption_after_elements_yields_them_then_the_interruption_then_none() {
    let mut rt = loopback();
    let mut provider = Provider::new(&rt);
    let mut client = generated::log::Client::new(&mut rt);
    let elements = [reading(1, false), reading(2, true), reading(3, false)];
    let interrupted = CallError::Transport(Transport::Interrupted);

    let mut stream = started(&mut client, &mut provider, &elements[0]);
    assert_eq!(provider.push(&elements[1]), Ok(element_len()));
    assert_eq!(provider.push(&elements[2]), Ok(element_len()));
    assert_eq!(provider.abort(interrupted), Ok(()));

    for element in &elements {
        assert_eq!(next_once(&mut stream), Poll::Ready(Some(Ok(*element))));
    }
    assert_eq!(
        next_once(&mut stream),
        Poll::Ready(Some(Err(ClientError::Call(interrupted)))),
        "the interruption follows the elements pushed before it"
    );
    assert_eq!(next_once(&mut stream), Poll::Ready(None));
}

#[test]
fn an_element_that_breaks_its_range_refuses_the_stream() {
    let mut rt = loopback();
    let mut provider = Provider::new(&rt);
    let mut client = generated::log::Client::new(&mut rt);
    let invalid = CallError::Contract(Contract::InvalidValue(Violation {
        type_name: Some("Level"),
        rule: Rule::Range,
    }));

    // 200 is outside `Level`'s range; the encoder writes it all the same.
    let mut stream = started(&mut client, &mut provider, &reading(200, false));
    assert_eq!(
        next_once(&mut stream),
        Poll::Ready(Some(Err(ClientError::Call(invalid)))),
        "the element fails its check and the stream is refused with it"
    );
    assert_eq!(next_once(&mut stream), Poll::Ready(None));
    // The refusal aborts the stream with the error and keeps the call until
    // the stream is dropped: a forget would turn the provider's answer into
    // `Interrupted` (`StreamHandler`'s forget rules), and the provider is to
    // read why the stream stopped.
    assert_eq!(
        provider.push(&reading(1, false)),
        Err(PushError::Failed(invalid)),
        "the provider reads the refusal from its next push"
    );
    drop(stream);
    assert_eq!(
        provider.push(&reading(1, false)),
        Err(PushError::Failed(CallError::Transport(
            Transport::Interrupted
        ))),
        "once the stream is dropped the call is forgotten"
    );
}

/// A chunk that fits no legal element: 256 bytes, which the loopback takes
/// (the member's limit is far above) and the element's buffer cannot hold.
const OVERSIZED: [u8; 256] = [0; 256];
const CORRUPT: CallError = CallError::Transport(Transport::Corrupt);

#[test]
fn a_first_chunk_longer_than_any_element_refuses_the_stream_as_corrupt() {
    let mut rt = loopback();
    let mut provider = Provider::new(&rt);
    let mut client = generated::log::Client::new(&mut rt);
    assert!(
        ELEMENT_MAX < OVERSIZED.len(),
        "the chunk cannot fit the element buffer"
    );

    let mut call = client.history(Level::new_unchecked(3));
    assert!(poll_once(&mut call).is_pending());
    let claim = provider.claim();
    assert_eq!(
        provider.handler.push_claim(claim, REPLY_STREAM, &OVERSIZED),
        Ok(OVERSIZED.len())
    );
    // `ReadError::Short` does not reach the application: the future refuses
    // the chunk and hands the refusal to the stream.
    let mut stream = resolved(poll_once(&mut call), "the refused first chunk");
    assert_eq!(
        next_once(&mut stream),
        Poll::Ready(Some(Err(ClientError::Call(CORRUPT))))
    );
    assert_eq!(next_once(&mut stream), Poll::Ready(None));
    assert_eq!(
        provider.push(&reading(1, false)),
        Err(PushError::Failed(CORRUPT)),
        "the provider reads the refusal from its next push"
    );
}

#[test]
fn a_later_chunk_longer_than_any_element_refuses_the_stream_as_corrupt() {
    let mut rt = loopback();
    let mut provider = Provider::new(&rt);
    let mut client = generated::log::Client::new(&mut rt);
    let first = reading(1, false);

    let mut stream = started(&mut client, &mut provider, &first);
    let claim = provider.claim();
    assert_eq!(
        provider.handler.push_claim(claim, REPLY_STREAM, &OVERSIZED),
        Ok(OVERSIZED.len())
    );
    assert_eq!(next_once(&mut stream), Poll::Ready(Some(Ok(first))));
    assert_eq!(
        next_once(&mut stream),
        Poll::Ready(Some(Err(ClientError::Call(CORRUPT)))),
        "the chunk that does not fit is refused, not reported as a short read"
    );
    assert_eq!(next_once(&mut stream), Poll::Ready(None));
    assert_eq!(
        provider.push(&reading(2, false)),
        Err(PushError::Failed(CORRUPT))
    );
}

#[test]
fn a_dropped_stream_interrupts_the_providers_next_push() {
    let mut rt = loopback();
    let mut provider = Provider::new(&rt);
    let mut client = generated::log::Client::new(&mut rt);

    let stream = started(&mut client, &mut provider, &reading(1, false));
    drop(stream);
    assert_eq!(
        provider.push(&reading(2, false)),
        Err(PushError::Failed(CallError::Transport(
            Transport::Interrupted
        ))),
        "dropping the stream forgets the call, which interrupts the stream"
    );
}

#[test]
fn a_failing_require_sends_nothing() {
    let mut rt = loopback();
    let mut provider = Provider::new(&rt);
    let mut client = generated::log::Client::new(&mut rt);

    // `history` requires `limit > 0`.
    let mut call = client.history(Level::new_unchecked(0));
    assert_eq!(
        poll_once(&mut call).map(|outcome| outcome.map(|_| ())),
        Poll::Ready(Err(ClientError::Send(SendError::Contract(
            Contract::PreconditionFailed
        )))),
    );
    let mut buf = [0u8; CLAIM_MAX];
    assert!(
        provider
            .handler
            .next_claim(&mut buf)
            .expect("next_claim")
            .is_none(),
        "nothing was sent"
    );
}

#[test]
fn next_awaits_the_element_poll_next_would_give() {
    let mut rt = loopback();
    let mut provider = Provider::new(&rt);
    let mut client = generated::log::Client::new(&mut rt);
    let elements = [reading(1, false), reading(2, true)];

    let mut stream = started(&mut client, &mut provider, &elements[0]);
    assert_eq!(
        poll_once(&mut pin!(stream.next())),
        Poll::Ready(Some(Ok(elements[0])))
    );
    {
        let mut next = pin!(stream.next());
        assert!(poll_once(&mut next).is_pending(), "no second element yet");
        assert_eq!(provider.push(&elements[1]), Ok(element_len()));
        assert_eq!(poll_once(&mut next), Poll::Ready(Some(Ok(elements[1]))));
    }
    assert_eq!(provider.end(), Ok(()));
    assert_eq!(poll_once(&mut pin!(stream.next())), Poll::Ready(None));
}

// ---------------------------------------------------------------------------
// What the face does at the port, over `RecordingPorts`.
// ---------------------------------------------------------------------------

/// A waker that counts its wakes, for the test that asserts a wake happened.
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

/// The correlation of the one send the log holds, taken with everything
/// logged before it.
fn the_one_send(log: &doubles::Log) -> Correlation {
    match doubles::take(log).as_slice() {
        [Op::Query(Ok(c))] => *c,
        ops => panic!("the method sends once when it is called, not {ops:?}"),
    }
}

/// The correlation of the send the log holds, among the polls `started`
/// made; everything logged so far is taken.
fn the_send_so_far(log: &doubles::Log) -> Correlation {
    let ops = doubles::take(log);
    let sends: Vec<Correlation> = ops
        .iter()
        .filter_map(|op| match op {
            Op::Query(Ok(c)) => Some(*c),
            _ => None,
        })
        .collect();
    match sends.as_slice() {
        [c] => *c,
        _ => panic!("the method sends once when it is called, not {ops:?}"),
    }
}

#[test]
fn a_waiting_call_registers_the_stream_interest_then_pulls() {
    let rt = loopback();
    let mut provider = Provider::new(&rt);
    let ports = RecordingPorts::new(&rt);
    let log = ports.log();
    let mut client = generated::log::Client::new(ports);

    let mut call = client.history(Level::new_unchecked(3));
    let c = the_one_send(&log);
    assert!(poll_once(&mut call).is_pending());
    assert_eq!(
        doubles::take(&log),
        vec![
            Op::WakeOn(Interest::Stream(c, REPLY_STREAM)),
            Op::Pull(c, REPLY_STREAM)
        ],
        "one poll registers its interest in the reply stream, then pulls, and returns"
    );
    assert_eq!(provider.push(&reading(1, false)), Ok(element_len()));
    let mut stream = resolved(poll_once(&mut call), "the first element");
    assert_eq!(
        doubles::take(&log),
        vec![
            Op::WakeOn(Interest::Stream(c, REPLY_STREAM)),
            Op::Pull(c, REPLY_STREAM)
        ],
        "the resolving poll pulled the first chunk and forgot nothing"
    );
    assert_eq!(
        next_once(&mut stream),
        Poll::Ready(Some(Ok(reading(1, false))))
    );
    assert!(
        doubles::take(&log).is_empty(),
        "the first element is handed out from the buffer, with no pull"
    );
    assert!(next_once(&mut stream).is_pending());
    assert_eq!(
        doubles::take(&log),
        vec![
            Op::WakeOn(Interest::Stream(c, REPLY_STREAM)),
            Op::Pull(c, REPLY_STREAM)
        ],
        "a poll of the stream registers its interest, then pulls"
    );
}

#[test]
fn a_pending_stream_is_woken_by_the_providers_push() {
    let rt = loopback();
    let mut provider = Provider::new(&rt);
    let ports = RecordingPorts::new(&rt);
    let mut client = generated::log::Client::new(ports);
    let wakes = CountWakes::new();
    let waker = Waker::from(Arc::clone(&wakes));
    let mut cx = Context::from_waker(&waker);

    let mut stream = started(&mut client, &mut provider, &reading(1, false));
    assert!(
        stream.poll_next(&mut cx).is_ready(),
        "the kept first element"
    );
    assert!(stream.poll_next(&mut cx).is_pending());
    assert_eq!(wakes.count(), 0);
    assert_eq!(provider.push(&reading(2, false)), Ok(element_len()));
    assert_eq!(wakes.count(), 1, "the push wakes the registered waker");
    assert_eq!(
        stream.poll_next(&mut cx),
        Poll::Ready(Some(Ok(reading(2, false))))
    );
}

#[test]
fn a_dropped_stream_forgets_its_call_once() {
    let rt = loopback();
    let mut provider = Provider::new(&rt);
    let ports = RecordingPorts::new(&rt);
    let log = ports.log();
    let mut client = generated::log::Client::new(ports);

    let stream = started(&mut client, &mut provider, &reading(1, false));
    let c = the_send_so_far(&log);
    drop(stream);
    assert_eq!(
        doubles::forgets(&log),
        vec![c],
        "dropping the stream before its end forgets the call, once"
    );
    drop(client);
    assert_eq!(doubles::forgets(&log), vec![c], "nothing else forgets it");
}

#[test]
fn a_stream_that_ended_forgets_at_once_and_nothing_more_on_drop() {
    let rt = loopback();
    let mut provider = Provider::new(&rt);
    let ports = RecordingPorts::new(&rt);
    let log = ports.log();
    let mut client = generated::log::Client::new(ports);

    let mut stream = started(&mut client, &mut provider, &reading(1, false));
    let c = the_send_so_far(&log);
    assert_eq!(provider.end(), Ok(()));
    assert!(next_once(&mut stream).is_ready());
    assert_eq!(
        doubles::forgets(&log),
        vec![],
        "not forgotten before the end"
    );
    assert_eq!(next_once(&mut stream), Poll::Ready(None));
    assert_eq!(doubles::forgets(&log), vec![c], "the end forgets the call");
    drop(stream);
    assert_eq!(
        doubles::forgets(&log),
        vec![c],
        "a done stream forgets nothing on drop"
    );
}

#[test]
fn a_refused_element_aborts_the_stream_then_forgets_once() {
    let rt = loopback();
    let mut provider = Provider::new(&rt);
    let ports = RecordingPorts::new(&rt);
    let log = ports.log();
    let mut client = generated::log::Client::new(ports);
    let invalid = CallError::Contract(Contract::InvalidValue(Violation {
        type_name: Some("Level"),
        rule: Rule::Range,
    }));

    let mut stream = started(&mut client, &mut provider, &reading(200, false));
    let c = the_send_so_far(&log);
    assert_eq!(
        next_once(&mut stream),
        Poll::Ready(Some(Err(ClientError::Call(invalid))))
    );
    assert_eq!(
        doubles::take(&log),
        vec![Op::Abort(c, REPLY_STREAM, invalid)],
        "the refusal aborts the stream with the error and keeps the call"
    );
    assert_eq!(next_once(&mut stream), Poll::Ready(None));
    assert!(
        doubles::take(&log).is_empty(),
        "a stopped stream touches the port no more"
    );
    drop(stream);
    assert_eq!(
        doubles::take(&log),
        vec![Op::Forget(c)],
        "the drop forgets the call the refusal kept, once"
    );
    drop(client);
    assert!(doubles::take(&log).is_empty(), "nothing else forgets it");
}

#[test]
fn a_waiting_call_is_woken_by_the_providers_first_push_once() {
    let rt = loopback();
    let mut provider = Provider::new(&rt);
    let ports = RecordingPorts::new(&rt);
    let mut client = generated::log::Client::new(ports);
    let wakes = CountWakes::new();
    let waker = Waker::from(Arc::clone(&wakes));
    let mut cx = Context::from_waker(&waker);

    let mut call = client.history(Level::new_unchecked(3));
    assert!(Pin::new(&mut call).poll(&mut cx).is_pending());
    assert_eq!(wakes.count(), 0);
    assert_eq!(provider.push(&reading(1, false)), Ok(element_len()));
    assert_eq!(
        wakes.count(),
        1,
        "the first push wakes the waker the call registered, once"
    );
    let mut stream = resolved(Pin::new(&mut call).poll(&mut cx), "the first element");
    assert_eq!(
        stream.poll_next(&mut cx),
        Poll::Ready(Some(Ok(reading(1, false))))
    );
}

#[test]
fn a_call_future_dropped_while_waiting_forgets_its_call_once() {
    let rt = loopback();
    let _provider = Provider::new(&rt);
    let ports = RecordingPorts::new(&rt);
    let log = ports.log();
    let mut client = generated::log::Client::new(ports);

    let mut call = client.history(Level::new_unchecked(3));
    let c = the_one_send(&log);
    assert!(poll_once(&mut call).is_pending());
    drop(call);
    assert_eq!(doubles::forgets(&log), vec![c]);
    drop(client);
    assert_eq!(doubles::forgets(&log), vec![c], "nothing else forgets it");
}

#[test]
fn a_call_whose_bound_passes_before_the_first_element_resolves_timeout_and_forgets() {
    let mut rt = loopback();
    let _provider = Provider::new(&rt);
    let ports = RecordingPorts::new(&rt);
    let log = ports.log();
    let mut client = generated::log::Client::new(ports);

    let mut call = client.history(Level::new_unchecked(3));
    let c = the_one_send(&log);
    assert!(poll_once(&mut call).is_pending());
    rt.advance(Duration(HISTORY_MAX.0 + 1));
    assert_eq!(
        poll_once(&mut call).map(|outcome| outcome.map(|_| ())),
        Poll::Ready(Err(ClientError::Call(CallError::Transport(
            Transport::Timeout
        )))),
        "the bound covers the first element or the end"
    );
    assert_eq!(
        doubles::forgets(&log),
        vec![c],
        "the expired call is forgotten"
    );
    drop(call);
    assert_eq!(
        doubles::forgets(&log),
        vec![c],
        "and not forgotten again on drop"
    );
}

// ---------------------------------------------------------------------------
// The interface without a stream builds over a port without the extension.
// ---------------------------------------------------------------------------

/// A caller port with no `StreamCaller`: the loopback's caller handle behind
/// the four port traits a plain call needs, and no more. `Plain`'s `Client`
/// is built over it, which is the proof that an interface without a stream
/// member gains no stream bound.
struct PlainPorts(ridl_loopback::CallerHandle);

impl Attached for PlainPorts {
    fn catalog(&self) -> &CatalogRef {
        self.0.catalog()
    }
}

impl Encoded for PlainPorts {
    type Encoding = FlatBuffers;
}

impl Clock for PlainPorts {
    fn now(&self) -> Timestamp {
        self.0.now()
    }
}

impl Caller for PlainPorts {
    fn command(
        &mut self,
        iface: InterfaceNo,
        ord: Ordinal,
        args: &[u8],
        trace: Option<TraceContext>,
    ) -> Result<Correlation, SendError> {
        self.0.command(iface, ord, args, trace)
    }

    fn query(
        &mut self,
        iface: InterfaceNo,
        ord: Ordinal,
        args: &[u8],
        trace: Option<TraceContext>,
    ) -> Result<Correlation, SendError> {
        self.0.query(iface, ord, args, trace)
    }

    fn ack(&mut self, c: Correlation) -> Option<Result<(), CallError>> {
        self.0.ack(c)
    }

    fn reply(
        &mut self,
        c: Correlation,
        out: &mut [u8],
    ) -> Result<Option<Result<usize, CallError>>, ReadError> {
        self.0.reply(c, out)
    }

    fn forget(&mut self, c: Correlation) {
        self.0.forget(c);
    }
}

impl Wakeable for PlainPorts {
    fn wake_on(&self, what: Interest, waker: &Waker) {
        self.0.wake_on(what, waker);
    }
}

/// Serves `Plain::latest` with a fixed reading.
struct PlainProvider;

impl generated::plain::Provider for PlainProvider {
    fn latest(&mut self, limit: &Level) -> Reading {
        reading(limit.get(), true)
    }
}

#[test]
fn the_client_of_an_interface_without_a_stream_builds_over_a_port_without_the_extension() {
    let rt = loopback();
    let mut provider = PlainProvider;
    let mut serve = generated::plain::serve(rt.handler(), &mut provider);
    let mut client = generated::plain::Client::new(PlainPorts(rt.caller()));

    let mut call = client.latest(Level::new_unchecked(5));
    assert!(poll_once(&mut call).is_pending());
    assert!(poll_once(&mut serve).is_pending());
    assert_eq!(poll_once(&mut call), Poll::Ready(Ok(reading(5, true))));
}
