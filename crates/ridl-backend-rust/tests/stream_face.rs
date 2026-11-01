//! The caller side of a streamed reply of named elements, over
//! `ridl-loopback` (`docs/design/interaction-face.md`).
//!
//! It brings in the checked-in generated face of `tests/fixtures/stream_face.ridl`
//! and drives the `Client` of `Log`, whose `history` replies with a stream of
//! `Reading`. The tests of the caller side drive the provider by hand through
//! the `StreamHandler` port of a loopback handler handle, so that each test
//! chooses every push: it takes the claim, pushes encoded elements, and ends
//! or aborts the stream. What they pin is the stream value's contract: the
//! order of the elements and the end, the first chunk kept by the call's
//! future, the registration of the waker, the refusal of an element that
//! fails its check, and the one `forget` a drop or the end makes. The tests of
//! the provider side, at the end of the file, run the generated `serve` over a
//! `Provider` whose sources follow a script: the order of the elements and
//! the end, a source that is not ready, one source poll per poll of `serve`
//! and its place in the budget, the element kept while the window is full,
//! the interruption, the dropped stream, the second call of a running member,
//! the check before the push, and the member's limit. The tests after them
//! pin the forgets of a call that ends or fails before any element and of a
//! stream the provider interrupts, a malformed chunk, a call that waits for a
//! slot, what `serve` checks before it starts a source, the held element under
//! Proto3, the drop of `Serve` while a stream runs, and an interface with two
//! streamed queries, `Twin`.
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

use std::cell::Cell;
use std::future::Future;
use std::pin::{Pin, pin};
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::task::{Context, Poll, Wake, Waker};

use ridl_loopback::{HandlerHandle, Loopback};
use ridl_rt::contract::{CatalogRef, Interaction, Interface, InterfaceNo, Ordinal, Sizes};
use ridl_rt::encoding::{FlatBuffers, Proto3};
use ridl_rt::error::{CallError, ClientError, Contract, Transport};
use ridl_rt::face::{Interrupted, Source};
use ridl_rt::payload::{Ref, Rule, Violation};
use ridl_rt::port::{
    Attached, Caller, ClaimId, Clock, Correlation, Encoded, Handler, Interest, Pull, PushError,
    REPLY_STREAM, ReadError, SendError, StreamCaller, StreamHandler, Wakeable,
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
/// a handler handle, so that a test chooses each push, including pushes the
/// generated `serve` never makes. It serves `history` alone, takes the one
/// claim a test sends, and pushes, ends or aborts the reply stream.
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

// ---------------------------------------------------------------------------
// The provider side: the generated `serve` drives a `Source`.
// ---------------------------------------------------------------------------

/// What a `Scripted` source yields.
#[derive(Clone, Default)]
struct Script {
    /// The elements, in order, before the end.
    elements: Vec<Reading>,
    /// Yield readings without end instead of `elements`.
    endless: bool,
    /// How many `Pending` answers come before each element.
    pending_before_each: usize,
    /// Stop with `Interrupted` after this many elements instead of the end.
    interrupt_after: Option<usize>,
}

/// What the tests read from the sources and the provider while `serve`
/// holds the provider.
#[derive(Default)]
struct Counters {
    /// Calls of `Source::poll_next`, every source summed.
    polls: Cell<usize>,
    /// Sources the provider returned.
    started: Cell<usize>,
    /// Sources dropped.
    dropped: Cell<usize>,
    /// The `limit` the provider's last `history` call received.
    limit: Cell<Option<i64>>,
}

/// A source that follows its `Script`. It stores no waker on `Pending`: the
/// tests poll `serve` by hand.
struct Scripted {
    script: Script,
    yielded: usize,
    pending_left: usize,
    counters: Rc<Counters>,
}

impl Source for Scripted {
    type Item = Reading;

    fn poll_next(&mut self, _cx: &mut Context<'_>) -> Poll<Option<Result<Reading, Interrupted>>> {
        self.counters.polls.set(self.counters.polls.get() + 1);
        if self.pending_left > 0 {
            self.pending_left -= 1;
            return Poll::Pending;
        }
        if self.script.interrupt_after == Some(self.yielded) {
            return Poll::Ready(Some(Err(Interrupted)));
        }
        let element = if self.script.endless {
            reading(i64::try_from(self.yielded % 100).expect("below 100"), false)
        } else {
            match self.script.elements.get(self.yielded) {
                Some(element) => *element,
                None => return Poll::Ready(None),
            }
        };
        self.yielded += 1;
        self.pending_left = self.script.pending_before_each;
        Poll::Ready(Some(Ok(element)))
    }
}

impl Drop for Scripted {
    fn drop(&mut self) {
        self.counters.dropped.set(self.counters.dropped.get() + 1);
    }
}

/// The generated `Provider` of `Log`: every `history` call gets a new source
/// that follows `script`.
struct Feed {
    script: Script,
    counters: Rc<Counters>,
}

impl Feed {
    fn new(script: Script) -> (Self, Rc<Counters>) {
        let counters = Rc::new(Counters::default());
        (
            Feed {
                script,
                counters: Rc::clone(&counters),
            },
            counters,
        )
    }
}

impl generated::log::Provider for Feed {
    type History = Scripted;

    fn clear(&mut self, _level: &Level) {}

    fn history(&mut self, limit: &Level) -> Scripted {
        self.counters.started.set(self.counters.started.get() + 1);
        self.counters.limit.set(Some(limit.get()));
        Scripted {
            pending_left: self.script.pending_before_each,
            script: self.script.clone(),
            yielded: 0,
            counters: Rc::clone(&self.counters),
        }
    }
}

fn elements(count: i64) -> Vec<Reading> {
    (1..=count).map(|i| reading(i, i % 2 == 0)).collect()
}

/// `history(3)` sent and its claim taken by `serve`, which starts the
/// provider's source and polls it on no later poll than the next.
fn sent<'a, P>(
    client: &'a mut generated::log::Client<P>,
    serve: &mut (impl Future + Unpin),
) -> generated::log::HistoryCall<'a, P>
where
    P: Encoded<Encoding = FlatBuffers> + ridl_rt::port::StreamCaller + Clock + Wakeable,
{
    let mut call = client.history(Level::new_unchecked(3));
    assert!(poll_once(&mut call).is_pending(), "the query was sent");
    assert!(poll_once(serve).is_pending(), "serve took the claim");
    call
}

#[test]
fn serve_sends_a_sources_elements_in_order_then_the_end() {
    let rt = loopback();
    let script = Script {
        elements: elements(3),
        ..Script::default()
    };
    let (mut feed, counters) = Feed::new(script.clone());
    let mut serve = generated::log::serve(rt.handler(), &mut feed);
    let mut client = generated::log::Client::new(rt.caller());

    let mut call = sent(&mut client, &mut serve);
    assert_eq!(counters.started.get(), 1, "the provider returned a source");
    assert_eq!(
        counters.polls.get(),
        0,
        "the poll that started the stream polled no source"
    );
    for _ in 0..4 {
        assert!(poll_once(&mut serve).is_pending());
    }
    let mut stream = resolved(poll_once(&mut call), "the first element");
    for element in &script.elements {
        assert_eq!(next_once(&mut stream), Poll::Ready(Some(Ok(*element))));
    }
    assert_eq!(
        next_once(&mut stream),
        Poll::Ready(None),
        "Ready(None) is the end"
    );
    assert_eq!(counters.dropped.get(), 1, "the ended source was dropped");
}

#[test]
fn serve_polls_a_pending_source_again_and_sends_each_element_once_it_is_ready() {
    let rt = loopback();
    let script = Script {
        elements: elements(3),
        pending_before_each: 2,
        ..Script::default()
    };
    let (mut feed, counters) = Feed::new(script.clone());
    let mut serve = generated::log::serve(rt.handler(), &mut feed);
    let mut client = generated::log::Client::new(rt.caller());

    let mut call = sent(&mut client, &mut serve);
    // Two polls of `serve` answer `Pending` from the source, the third sends
    // the element.
    assert!(poll_once(&mut serve).is_pending());
    assert!(poll_once(&mut serve).is_pending());
    assert!(
        poll_once(&mut call).is_pending(),
        "the source has given nothing yet"
    );
    assert!(poll_once(&mut serve).is_pending());
    let mut stream = resolved(poll_once(&mut call), "the first element");
    assert_eq!(
        next_once(&mut stream),
        Poll::Ready(Some(Ok(script.elements[0])))
    );
    for element in &script.elements[1..] {
        assert!(poll_once(&mut serve).is_pending());
        assert!(poll_once(&mut serve).is_pending());
        assert!(
            next_once(&mut stream).is_pending(),
            "the source answered Pending"
        );
        assert!(poll_once(&mut serve).is_pending());
        assert_eq!(next_once(&mut stream), Poll::Ready(Some(Ok(*element))));
    }
    assert!(poll_once(&mut serve).is_pending());
    assert!(poll_once(&mut serve).is_pending());
    assert!(poll_once(&mut serve).is_pending());
    assert_eq!(next_once(&mut stream), Poll::Ready(None));
    assert_eq!(
        counters.polls.get(),
        12,
        "three Pending before each element and the end"
    );
}

#[test]
fn serve_polls_the_source_once_per_poll_of_its_own() {
    let rt = loopback();
    let (mut feed, counters) = Feed::new(Script {
        endless: true,
        ..Script::default()
    });
    let mut serve = generated::log::serve(rt.handler(), &mut feed);
    let mut client = generated::log::Client::new(rt.caller());

    let _call = sent(&mut client, &mut serve);
    for polls in 1..=3 {
        assert!(poll_once(&mut serve).is_pending());
        assert_eq!(
            counters.polls.get(),
            polls,
            "one poll of serve polls the source once, although it has more ready"
        );
    }
}

/// A loopback handler with claims of its own added behind the runtime's:
/// once the runtime has no claim waiting, it presents `extra` claims of
/// ordinal 99, which no member of `Log` has, so `dispatch` settles each
/// `Contract::UnknownInteraction`. The loopback's call table holds 16 calls,
/// fewer than one poll of `serve` takes, so the budget is reached through
/// these.
///
/// It stores no `Interest::Claim` waker: the loopback wakes a `Claim` waker
/// that registers while a claim is already waiting, which would hide whether
/// `serve` wakes itself. Every other registration reaches the loopback.
struct Crowded {
    inner: HandlerHandle,
    extra: Rc<Cell<usize>>,
    taken: Rc<Cell<usize>>,
}

/// The ids of the added claims, far above any id the loopback gives.
const CROWD: u64 = 1 << 40;

impl Attached for Crowded {
    fn catalog(&self) -> &CatalogRef {
        self.inner.catalog()
    }
}

impl Encoded for Crowded {
    type Encoding = FlatBuffers;
}

impl Handler for Crowded {
    fn serve(
        &mut self,
        iface: InterfaceNo,
        ords: &[Ordinal],
    ) -> Result<(), ridl_rt::port::ServeError> {
        self.inner.serve(iface, ords)
    }

    fn next_claim(&mut self, out: &mut [u8]) -> Result<Option<ridl_rt::port::Claim>, ReadError> {
        if let Some(claim) = self.inner.next_claim(out)? {
            return Ok(Some(claim));
        }
        if self.extra.get() == 0 {
            return Ok(None);
        }
        self.extra.set(self.extra.get() - 1);
        self.taken.set(self.taken.get() + 1);
        Ok(Some(ridl_rt::port::Claim {
            id: ClaimId(CROWD + self.taken.get() as u64),
            iface: LOG,
            ord: Ordinal(99),
            envelope: ridl_rt::sample::Envelope {
                stamp: Timestamp(0),
                seq: 0,
            },
            trace: None,
            remaining: None,
            len: 0,
        }))
    }

    fn settle(
        &mut self,
        claim: ClaimId,
        outcome: Result<&[u8], CallError>,
    ) -> Result<(), ridl_rt::port::SettleError> {
        if claim.0 > CROWD {
            return Ok(());
        }
        self.inner.settle(claim, outcome)
    }
}

impl StreamHandler for Crowded {
    fn pull_claim(
        &mut self,
        claim: ClaimId,
        s: ridl_rt::contract::StreamNo,
        out: &mut [u8],
    ) -> Result<Pull, ReadError> {
        self.inner.pull_claim(claim, s, out)
    }

    fn push_claim(
        &mut self,
        claim: ClaimId,
        s: ridl_rt::contract::StreamNo,
        chunk: &[u8],
    ) -> Result<usize, PushError> {
        self.inner.push_claim(claim, s, chunk)
    }

    fn end_claim(
        &mut self,
        claim: ClaimId,
        s: ridl_rt::contract::StreamNo,
    ) -> Result<(), PushError> {
        self.inner.end_claim(claim, s)
    }

    fn abort_claim(
        &mut self,
        claim: ClaimId,
        s: ridl_rt::contract::StreamNo,
        e: CallError,
    ) -> Result<(), PushError> {
        self.inner.abort_claim(claim, s, e)
    }
}

impl Wakeable for Crowded {
    fn wake_on(&self, what: Interest, waker: &Waker) {
        if !matches!(what, Interest::Claim(_)) {
            self.inner.wake_on(what, waker);
        }
    }
}

impl Crowded {
    /// A handler over `rt` with no added claims.
    fn quiet(rt: &Loopback) -> Self {
        Crowded {
            inner: rt.handler(),
            extra: Rc::new(Cell::new(0)),
            taken: Rc::new(Cell::new(0)),
        }
    }
}

#[test]
fn a_source_poll_counts_against_the_claims_one_poll_of_serve_takes() {
    let rt = loopback();
    let (mut feed, counters) = Feed::new(Script {
        endless: true,
        ..Script::default()
    });
    let extra = Rc::new(Cell::new(0));
    let taken = Rc::new(Cell::new(0));
    let handler = Crowded {
        inner: rt.handler(),
        extra: Rc::clone(&extra),
        taken: Rc::clone(&taken),
    };
    let mut serve = generated::log::serve(handler, &mut feed);
    let mut client = generated::log::Client::new(rt.caller());
    let _call = sent(&mut client, &mut serve);

    // 32 claims wait, the most one poll takes.
    extra.set(32);
    assert!(poll_once(&mut serve).is_pending());
    assert_eq!(counters.polls.get(), 1, "the source was polled");
    assert_eq!(
        taken.get(),
        31,
        "the source poll counted as one of the 32 claims of the poll, so one claim waits"
    );
    assert!(poll_once(&mut serve).is_pending());
    assert_eq!(taken.get(), 32, "the next poll takes it");
}

#[test]
fn an_element_the_window_had_no_room_for_is_sent_on_a_later_poll_and_not_asked_for_again() {
    let rt = loopback();
    let script = Script {
        elements: elements(10),
        ..Script::default()
    };
    let (mut feed, counters) = Feed::new(script.clone());
    let mut serve = generated::log::serve(rt.handler(), &mut feed);
    let mut client = generated::log::Client::new(rt.caller());

    let mut call = sent(&mut client, &mut serve);
    assert!(poll_once(&mut serve).is_pending());
    let mut stream = resolved(poll_once(&mut call), "the first element");
    // The future pulled the first element; the window then takes four more,
    // and the sixth finds it full and is held. Later polls push it again
    // and do not poll the source.
    for _ in 0..10 {
        assert!(poll_once(&mut serve).is_pending());
    }
    assert_eq!(
        counters.polls.get(),
        6,
        "the source is not polled while its element waits for room in the window"
    );
    let mut received = Vec::new();
    // Bounded, so that a stream that never ends fails the test instead of
    // holding it.
    for _ in 0..100 {
        match next_once(&mut stream) {
            Poll::Ready(Some(Ok(element))) => received.push(element),
            Poll::Ready(None) => break,
            Poll::Ready(Some(Err(error))) => panic!("the stream failed: {error:?}"),
            Poll::Pending => assert!(poll_once(&mut serve).is_pending()),
        }
    }
    assert_eq!(
        next_once(&mut stream),
        Poll::Ready(None),
        "the stream ended"
    );
    assert_eq!(received, script.elements, "every element, once, in order");
    assert_eq!(
        counters.polls.get(),
        11,
        "each element and the end were asked for once"
    );
}

#[test]
fn a_source_that_interrupts_after_three_elements_gives_them_then_the_interruption() {
    let rt = loopback();
    let script = Script {
        elements: elements(5),
        interrupt_after: Some(3),
        ..Script::default()
    };
    let (mut feed, counters) = Feed::new(script.clone());
    let mut serve = generated::log::serve(rt.handler(), &mut feed);
    let mut client = generated::log::Client::new(rt.caller());

    let mut call = sent(&mut client, &mut serve);
    for _ in 0..4 {
        assert!(poll_once(&mut serve).is_pending());
    }
    let mut stream = resolved(poll_once(&mut call), "the first element");
    for element in &script.elements[..3] {
        assert_eq!(next_once(&mut stream), Poll::Ready(Some(Ok(*element))));
    }
    assert_eq!(
        next_once(&mut stream),
        Poll::Ready(Some(Err(ClientError::Call(CallError::Transport(
            Transport::Interrupted
        ))))),
        "the source's Interrupted reaches the consumer as Transport::Interrupted"
    );
    assert_eq!(next_once(&mut stream), Poll::Ready(None));
    assert_eq!(
        counters.dropped.get(),
        1,
        "the interrupted source was dropped"
    );
}

#[test]
fn a_dropped_stream_fails_the_next_push_and_serve_drops_the_source() {
    let rt = loopback();
    let (mut feed, counters) = Feed::new(Script {
        endless: true,
        pending_before_each: 1,
        ..Script::default()
    });
    let mut serve = generated::log::serve(rt.handler(), &mut feed);
    let mut client = generated::log::Client::new(rt.caller());

    // Every round sends a call, reads its first element, and drops the
    // stream; `serve` reads `Failed(Transport(Interrupted))` from its next
    // push, drops the source and settles the claim. One more round than the
    // caller's call table holds shows that the claim of each dropped stream
    // was settled, so its slot was given back, and that each source was
    // dropped, so the member's slot in `serve` was free for the next call.
    for round in 1..=Loopback::SLOTS + 1 {
        let mut call = sent(&mut client, &mut serve);
        assert!(poll_once(&mut serve).is_pending(), "Pending");
        assert!(poll_once(&mut serve).is_pending(), "the first element");
        let mut stream = resolved(poll_once(&mut call), "the first element");
        assert!(next_once(&mut stream).is_ready());
        drop(stream);
        assert!(poll_once(&mut serve).is_pending(), "Pending");
        assert_eq!(
            counters.dropped.get(),
            round - 1,
            "the source is kept until a push fails"
        );
        assert!(poll_once(&mut serve).is_pending(), "the push that fails");
        assert_eq!(counters.dropped.get(), round, "serve dropped the source");
    }
    assert_eq!(counters.started.get(), Loopback::SLOTS + 1);
}

#[test]
fn a_second_call_while_the_members_stream_runs_is_settled_busy() {
    let rt = loopback();
    let (mut feed, counters) = Feed::new(Script {
        endless: true,
        pending_before_each: 1,
        ..Script::default()
    });
    let mut serve = generated::log::serve(rt.handler(), &mut feed);
    let mut first = generated::log::Client::new(rt.caller());
    let mut second = generated::log::Client::new(rt.caller());

    let _running = sent(&mut first, &mut serve);
    let mut refused = sent(&mut second, &mut serve);
    assert_eq!(
        poll_once(&mut refused).map(|outcome| outcome.map(|_| ())),
        Poll::Ready(Err(ClientError::Call(CallError::Transport(
            Transport::Busy
        )))),
    );
    assert_eq!(
        counters.started.get(),
        1,
        "the provider was not called for it"
    );
}

#[test]
fn an_element_outside_its_range_stops_the_stream_before_it_is_sent() {
    let rt = loopback();
    let (mut feed, counters) = Feed::new(Script {
        elements: vec![reading(1, false), reading(200, true), reading(2, false)],
        ..Script::default()
    });
    let mut serve = generated::log::serve(rt.handler(), &mut feed);
    let invalid = CallError::Contract(Contract::InvalidValue(Violation {
        type_name: Some("Level"),
        rule: Rule::Range,
    }));

    // The reply stream is read through the bare port, so a chunk that
    // crossed would be seen as a chunk, before any check of the consumer's.
    let mut caller = rt.caller();
    let correlation = generated::log::send_history(&mut caller, &Level::new_unchecked(3))
        .expect("the query is sent")
        .0;
    for _ in 0..4 {
        assert!(poll_once(&mut serve).is_pending());
    }
    let mut buf = [0u8; ELEMENT_MAX];
    assert_eq!(
        caller.pull(correlation, REPLY_STREAM, &mut buf),
        Ok(Pull::Chunk(element_len())),
        "the element in range was sent"
    );
    assert_eq!(
        caller.pull(correlation, REPLY_STREAM, &mut buf),
        Ok(Pull::Failed(invalid)),
        "the element outside its range was not sent, and stopped the stream"
    );
    assert_eq!(counters.dropped.get(), 1, "the source was dropped");
    assert_eq!(
        counters.polls.get(),
        2,
        "the source was not polled after the refusal"
    );
}

#[test]
fn an_element_above_the_members_lowered_limit_is_refused_and_the_consumer_reads_interrupted() {
    let mut rt = loopback();
    let (mut feed, counters) = Feed::new(Script {
        elements: elements(2),
        ..Script::default()
    });
    let mut serve = generated::log::serve(rt.handler(), &mut feed);
    let mut client = generated::log::Client::new(rt.caller());
    let mut call = sent(&mut client, &mut serve);
    // The limit a `[limits.interaction]` entry would give `history`, one
    // byte below one element's encoding. The argument and one element both
    // encode to the same length in FlatBuffers, so no limit admits the call
    // and refuses the element; the loopback applies the member's limit at
    // each send, so the limit is lowered once the call was admitted.
    rt.set_limit(LOG, HISTORY, element_len() - 1);
    assert!(poll_once(&mut serve).is_pending());
    assert_eq!(
        poll_once(&mut call).map(|outcome| outcome.map(|_| ())),
        Poll::Ready(Err(ClientError::Call(CallError::Transport(
            Transport::Interrupted
        )))),
        "the push was refused TooLarge, and the stream stopped as interrupted"
    );
    assert_eq!(counters.dropped.get(), 1, "the source was dropped");
}

/// Polls `future` once with `waker`.
fn poll_with<F: Future + Unpin>(future: &mut F, waker: &Waker) -> Poll<F::Output> {
    let mut cx = Context::from_waker(waker);
    Pin::new(future).poll(&mut cx)
}

#[test]
fn serve_wakes_itself_after_it_starts_a_stream() {
    let rt = loopback();
    let (mut feed, counters) = Feed::new(Script {
        endless: true,
        ..Script::default()
    });
    // A handler that stores no `Claim` waker, so the only wake is the one
    // `serve` makes itself.
    let mut serve = generated::log::serve(Crowded::quiet(&rt), &mut feed);
    let mut client = generated::log::Client::new(rt.caller());
    let mut call = client.history(Level::new_unchecked(3));
    assert!(poll_once(&mut call).is_pending());

    let wakes = CountWakes::new();
    let waker = Waker::from(Arc::clone(&wakes));
    assert!(poll_with(&mut serve, &waker).is_pending());
    assert_eq!(counters.started.get(), 1, "the poll started the stream");
    assert_eq!(counters.polls.get(), 0, "and did not poll its source");
    assert_eq!(
        wakes.count(),
        1,
        "the future woke itself, so that the new source is polled"
    );
}

#[test]
fn serve_wakes_itself_after_the_window_took_an_element() {
    let rt = loopback();
    let (mut feed, counters) = Feed::new(Script {
        endless: true,
        ..Script::default()
    });
    let mut serve = generated::log::serve(Crowded::quiet(&rt), &mut feed);
    let mut client = generated::log::Client::new(rt.caller());
    let _call = sent(&mut client, &mut serve);

    let wakes = CountWakes::new();
    let waker = Waker::from(Arc::clone(&wakes));
    assert!(poll_with(&mut serve, &waker).is_pending());
    assert_eq!(counters.polls.get(), 1, "the source gave one element");
    assert_eq!(
        wakes.count(),
        1,
        "the future woke itself: the source stored no waker and may have the next element"
    );
}

#[test]
fn serve_is_woken_when_room_frees_after_its_element_found_the_window_full() {
    let rt = loopback();
    let (mut feed, counters) = Feed::new(Script {
        endless: true,
        ..Script::default()
    });
    let mut serve = generated::log::serve(Crowded::quiet(&rt), &mut feed);
    let mut client = generated::log::Client::new(rt.caller());
    let mut call = sent(&mut client, &mut serve);
    // Four elements fill the window.
    for _ in 0..4 {
        assert!(poll_once(&mut serve).is_pending());
    }
    // The fifth element is pushed for the first time and finds the window
    // full; nothing wakes the future yet.
    let wakes = CountWakes::new();
    let waker = Waker::from(Arc::clone(&wakes));
    assert!(poll_with(&mut serve, &waker).is_pending());
    assert_eq!(counters.polls.get(), 5);
    assert_eq!(wakes.count(), 0, "a full window is not a wake");
    // The call's future pulls the first element, which frees room.
    let _stream = resolved(poll_once(&mut call), "the first element");
    assert_eq!(
        wakes.count(),
        1,
        "the pull that freed room woke the waker registered before the push"
    );
}

#[test]
fn serve_is_woken_when_room_frees_while_it_pushes_a_held_element_again() {
    let rt = loopback();
    let (mut feed, counters) = Feed::new(Script {
        endless: true,
        ..Script::default()
    });
    let mut serve = generated::log::serve(Crowded::quiet(&rt), &mut feed);
    let mut client = generated::log::Client::new(rt.caller());
    let mut call = sent(&mut client, &mut serve);
    // Four elements fill the window and the fifth is held.
    for _ in 0..5 {
        assert!(poll_once(&mut serve).is_pending());
    }
    assert_eq!(counters.polls.get(), 5);
    // A later poll pushes the held element again, still into a full window.
    let wakes = CountWakes::new();
    let waker = Waker::from(Arc::clone(&wakes));
    assert!(poll_with(&mut serve, &waker).is_pending());
    assert_eq!(counters.polls.get(), 5, "the source is not polled");
    assert_eq!(wakes.count(), 0);
    let mut stream = resolved(poll_once(&mut call), "the first element");
    assert_eq!(
        wakes.count(),
        1,
        "the waker registered before the second push is the one woken"
    );
    // The first `poll_next` hands out the kept chunk and pulls nothing; the
    // second pulls from the port and frees room again.
    assert!(next_once(&mut stream).is_ready());
    assert_eq!(wakes.count(), 1);
    assert!(next_once(&mut stream).is_ready());
    assert_eq!(wakes.count(), 1, "already woken; the waker was taken");
}

#[test]
fn a_call_dropped_while_its_element_is_held_is_settled_and_its_source_dropped() {
    let rt = loopback();
    let (mut feed, counters) = Feed::new(Script {
        endless: true,
        ..Script::default()
    });
    let mut serve = generated::log::serve(rt.handler(), &mut feed);
    let mut client = generated::log::Client::new(rt.caller());

    // Every round fills the window, holds the fifth element, and drops the
    // call. `serve`'s next push of the held element reads the forget; it must
    // settle the claim, which gives the caller's call slot back, and drop
    // the source, which frees the member. One round more than the call table
    // holds shows both.
    for round in 1..=Loopback::SLOTS + 1 {
        let call = sent(&mut client, &mut serve);
        for _ in 0..5 {
            assert!(poll_once(&mut serve).is_pending());
        }
        assert_eq!(counters.polls.get(), round * 5, "the fifth element is held");
        drop(call);
        assert!(poll_once(&mut serve).is_pending());
        assert_eq!(counters.dropped.get(), round, "serve dropped the source");
        assert_eq!(counters.polls.get(), round * 5, "and did not poll it again");
    }
    let mut call = sent(&mut client, &mut serve);
    assert!(poll_once(&mut serve).is_pending());
    let mut stream = resolved(poll_once(&mut call), "the first element of a fresh call");
    assert!(matches!(next_once(&mut stream), Poll::Ready(Some(Ok(_)))));
}

#[test]
fn a_call_dropped_while_its_source_is_pending_keeps_the_member_busy_until_the_next_element() {
    let rt = loopback();
    let (mut feed, counters) = Feed::new(Script {
        endless: true,
        pending_before_each: 3,
        ..Script::default()
    });
    let mut serve = generated::log::serve(rt.handler(), &mut feed);
    let mut client = generated::log::Client::new(rt.caller());

    let call = sent(&mut client, &mut serve);
    drop(call);
    // The port tells `serve` nothing of the forget until a push: the member
    // stays held while its source answers `Pending`.
    let mut refused = sent(&mut client, &mut serve);
    assert_eq!(counters.polls.get(), 1, "the source answered Pending");
    assert_eq!(
        poll_once(&mut refused).map(|outcome| outcome.map(|_| ())),
        Poll::Ready(Err(ClientError::Call(CallError::Transport(
            Transport::Busy
        )))),
        "a new call of the member is settled Busy"
    );
    drop(refused);
    assert_eq!(counters.dropped.get(), 0, "the source is kept");
    // Two more `Pending`, then the element, whose push reads the forget.
    for _ in 0..3 {
        assert!(poll_once(&mut serve).is_pending());
    }
    assert_eq!(counters.polls.get(), 4);
    assert_eq!(
        counters.dropped.get(),
        1,
        "the push failed and serve dropped the source"
    );
    // The member is free again.
    let mut call = sent(&mut client, &mut serve);
    for _ in 0..4 {
        assert!(poll_once(&mut serve).is_pending());
    }
    let mut stream = resolved(poll_once(&mut call), "the first element");
    assert!(matches!(next_once(&mut stream), Poll::Ready(Some(Ok(_)))));
    assert_eq!(counters.started.get(), 2);
}

// ---------------------------------------------------------------------------
// The forgets of the call future and of a stopped stream.
// ---------------------------------------------------------------------------

#[test]
fn a_call_that_ends_before_any_element_forgets_its_call_once() {
    let rt = loopback();
    let mut provider = Provider::new(&rt);
    let ports = RecordingPorts::new(&rt);
    let log = ports.log();
    let mut client = generated::log::Client::new(ports);

    let mut call = client.history(Level::new_unchecked(3));
    let c = the_one_send(&log);
    assert!(poll_once(&mut call).is_pending());
    assert_eq!(provider.end(), Ok(()));
    let mut stream = resolved(poll_once(&mut call), "the end");
    assert_eq!(
        doubles::forgets(&log),
        vec![c],
        "the future that read the end forgot the call"
    );
    assert_eq!(next_once(&mut stream), Poll::Ready(None));
    drop(stream);
    drop(call);
    assert_eq!(
        doubles::forgets(&log),
        vec![c],
        "and nothing forgets it again"
    );
}

#[test]
fn a_call_that_fails_before_any_element_forgets_its_call_once() {
    let rt = loopback();
    let mut provider = Provider::new(&rt);
    let ports = RecordingPorts::new(&rt);
    let log = ports.log();
    let mut client = generated::log::Client::new(ports);
    let refusal = CallError::Contract(Contract::PreconditionFailed);

    let mut call = client.history(Level::new_unchecked(3));
    let c = the_one_send(&log);
    assert!(poll_once(&mut call).is_pending());
    assert_eq!(provider.abort(refusal), Ok(()));
    assert_eq!(
        poll_once(&mut call).map(|outcome| outcome.map(|_| ())),
        Poll::Ready(Err(ClientError::Call(refusal)))
    );
    assert_eq!(
        doubles::forgets(&log),
        vec![c],
        "the future that read the failure forgot the call"
    );
    drop(call);
    assert_eq!(doubles::forgets(&log), vec![c], "and the drop does not");
}

#[test]
fn a_stream_the_provider_interrupts_forgets_its_call_at_once() {
    let rt = loopback();
    let mut provider = Provider::new(&rt);
    let ports = RecordingPorts::new(&rt);
    let log = ports.log();
    let mut client = generated::log::Client::new(ports);
    let interrupted = CallError::Transport(Transport::Interrupted);

    let mut stream = started(&mut client, &mut provider, &reading(1, false));
    let c = the_send_so_far(&log);
    assert_eq!(provider.abort(interrupted), Ok(()));
    assert!(matches!(next_once(&mut stream), Poll::Ready(Some(Ok(_)))));
    assert_eq!(
        next_once(&mut stream),
        Poll::Ready(Some(Err(ClientError::Call(interrupted))))
    );
    assert_eq!(
        doubles::forgets(&log),
        vec![c],
        "the poll that read the interruption forgot the call, before any drop"
    );
    drop(stream);
    assert_eq!(
        doubles::forgets(&log),
        vec![c],
        "the drop forgets nothing more"
    );
}

#[test]
fn a_malformed_first_chunk_refuses_the_stream_as_corrupt() {
    let mut rt = loopback();
    let mut provider = Provider::new(&rt);
    let mut client = generated::log::Client::new(&mut rt);

    let mut call = client.history(Level::new_unchecked(3));
    assert!(poll_once(&mut call).is_pending());
    let claim = provider.claim();
    // Three bytes that fit the element buffer and are no FlatBuffers table.
    let malformed = [0xff_u8; 3];
    assert_eq!(
        provider.handler.push_claim(claim, REPLY_STREAM, &malformed),
        Ok(malformed.len())
    );
    let mut stream = resolved(poll_once(&mut call), "the malformed first chunk");
    assert_eq!(
        next_once(&mut stream),
        Poll::Ready(Some(Err(ClientError::Call(CORRUPT)))),
        "a chunk that is no well-formed element is refused as corrupt"
    );
    assert_eq!(next_once(&mut stream), Poll::Ready(None));
    assert_eq!(
        provider.push(&reading(1, false)),
        Err(PushError::Failed(CORRUPT)),
        "the provider reads the refusal from its next push"
    );
}

#[test]
fn a_call_with_no_free_slot_waits_then_expires_unsent_as_busy() {
    let mut rt = loopback();
    let _provider = Provider::new(&rt);
    // Every slot of the call table is held by a call nobody serves.
    let mut crowd = rt.caller();
    for _ in 0..Loopback::SLOTS {
        generated::log::send_history(&mut crowd, &Level::new_unchecked(3)).expect("a slot is free");
    }
    let mut client = generated::log::Client::new(rt.caller());

    let mut call = client.history(Level::new_unchecked(3));
    assert!(
        poll_once(&mut call).is_pending(),
        "a send the port answers Busy waits for a slot"
    );
    rt.advance(Duration(HISTORY_MAX.0 + 1));
    assert_eq!(
        poll_once(&mut call).map(|outcome| outcome.map(|_| ())),
        Poll::Ready(Err(ClientError::Send(SendError::Busy))),
        "a call still unsent at its bound is Send(Busy)"
    );
}

// ---------------------------------------------------------------------------
// What `serve` checks before it starts a stream.
// ---------------------------------------------------------------------------

/// Sends `history(limit)` through the bare caller port, with no `require`
/// evaluated on the caller's side, and answers the correlation.
fn send_unchecked(caller: &mut ridl_loopback::CallerHandle, limit: i64) -> Correlation {
    let mut buf = [0u8; CLAIM_MAX];
    let bytes = Ref::<Level, FlatBuffers>::encode(&Level::new_unchecked(limit), &mut buf)
        .expect("encode")
        .bytes();
    caller
        .query(LOG, HISTORY, bytes, None)
        .expect("the query is sent")
}

#[test]
fn serve_refuses_a_streamed_call_that_breaks_require_without_starting_a_source() {
    let rt = loopback();
    let (mut feed, counters) = Feed::new(Script {
        elements: elements(2),
        ..Script::default()
    });
    let mut serve = generated::log::serve(rt.handler(), &mut feed);
    let mut caller = rt.caller();

    // `history` requires `limit > 0`; 0 is inside `Level`'s range.
    let c = send_unchecked(&mut caller, 0);
    assert!(poll_once(&mut serve).is_pending());
    let mut buf = [0u8; ELEMENT_MAX];
    assert_eq!(
        caller.pull(c, REPLY_STREAM, &mut buf),
        Ok(Pull::Failed(CallError::Contract(
            Contract::PreconditionFailed
        ))),
        "serve evaluates require on its own side"
    );
    assert_eq!(counters.started.get(), 0, "the provider was not called");
}

#[test]
fn serve_evaluates_require_before_it_reads_the_members_slot() {
    let rt = loopback();
    let (mut feed, counters) = Feed::new(Script {
        endless: true,
        pending_before_each: 1,
        ..Script::default()
    });
    let mut serve = generated::log::serve(rt.handler(), &mut feed);
    let mut client = generated::log::Client::new(rt.caller());
    let _running = sent(&mut client, &mut serve);

    let mut caller = rt.caller();
    let c = send_unchecked(&mut caller, 0);
    assert!(poll_once(&mut serve).is_pending());
    let mut buf = [0u8; ELEMENT_MAX];
    assert_eq!(
        caller.pull(c, REPLY_STREAM, &mut buf),
        Ok(Pull::Failed(CallError::Contract(
            Contract::PreconditionFailed
        ))),
        "a call that breaks require reads PreconditionFailed, not Busy, while the member runs"
    );
    assert_eq!(counters.started.get(), 1);
}

#[test]
fn serve_settles_a_streamed_call_whose_arguments_do_not_decode_as_corrupt() {
    let rt = loopback();
    let (mut feed, counters) = Feed::new(Script::default());
    let mut serve = generated::log::serve(rt.handler(), &mut feed);
    let mut caller = rt.caller();

    let c = caller
        .query(LOG, HISTORY, &[0xff; 3], None)
        .expect("the query is sent");
    assert!(poll_once(&mut serve).is_pending());
    let mut buf = [0u8; ELEMENT_MAX];
    assert_eq!(
        caller.pull(c, REPLY_STREAM, &mut buf),
        Ok(Pull::Failed(CORRUPT)),
        "argument bytes that are no well-formed `Level` settle the claim as corrupt"
    );
    assert_eq!(counters.started.get(), 0, "the provider was not called");
}

#[test]
fn the_provider_receives_the_callers_argument() {
    let rt = loopback();
    let (mut feed, counters) = Feed::new(Script::default());
    let mut serve = generated::log::serve(rt.handler(), &mut feed);
    let mut client = generated::log::Client::new(rt.caller());

    let mut call = client.history(Level::new_unchecked(42));
    assert!(poll_once(&mut call).is_pending());
    assert!(poll_once(&mut serve).is_pending());
    assert_eq!(counters.limit.get(), Some(42));
}

// ---------------------------------------------------------------------------
// The held element under Proto3, whose encoder fills the buffer from its
// start, where FlatBuffers fills it from its end.
// ---------------------------------------------------------------------------

#[test]
fn under_proto3_an_element_the_window_had_no_room_for_is_sent_on_a_later_poll() {
    let rt = Loopback::<Proto3>::with_encoding(CATALOG);
    let script = Script {
        elements: elements(10),
        ..Script::default()
    };
    let (mut feed, counters) = Feed::new(script.clone());
    let mut serve = generated::log::serve(rt.handler(), &mut feed);
    let mut client = generated::log::Client::new(rt.caller());
    let waker = ridl_rt::task::noop_waker();
    let mut cx = Context::from_waker(&waker);

    let mut call = client.history(Level::new_unchecked(3));
    assert!(Pin::new(&mut call).poll(&mut cx).is_pending());
    for _ in 0..12 {
        assert!(poll_once(&mut serve).is_pending());
    }
    assert_eq!(
        counters.polls.get(),
        5,
        "four elements fill the window and the fifth is held"
    );
    let mut stream = match Pin::new(&mut call).poll(&mut cx) {
        Poll::Ready(Ok(stream)) => stream,
        Poll::Ready(Err(error)) => panic!("the call failed: {error:?}"),
        Poll::Pending => panic!("the first element resolves the call"),
    };
    let mut received = Vec::new();
    for _ in 0..100 {
        match stream.poll_next(&mut cx) {
            Poll::Ready(Some(Ok(element))) => received.push(element),
            Poll::Ready(None) => break,
            Poll::Ready(Some(Err(error))) => panic!("the stream failed: {error:?}"),
            Poll::Pending => assert!(poll_once(&mut serve).is_pending()),
        }
    }
    assert_eq!(received, script.elements, "every element, once, in order");
}

// ---------------------------------------------------------------------------
// Dropping `Serve` while a stream runs.
// ---------------------------------------------------------------------------

#[test]
fn a_blocking_serve_that_times_out_interrupts_its_running_stream_and_frees_the_member() {
    let rt = loopback();
    let (mut feed, counters) = Feed::new(Script {
        endless: true,
        ..Script::default()
    });
    let mut handler = rt.handler();
    let mut client = generated::log::Client::new(rt.caller());

    let mut call = client.history(Level::new_unchecked(3));
    assert!(poll_once(&mut call).is_pending());
    // The handler is lent, so it outlives the future `blocking::serve`
    // drops at its timeout.
    assert_eq!(
        generated::log::blocking::serve(
            &mut handler,
            &mut feed,
            Some(std::time::Duration::from_millis(20)),
        ),
        Ok(())
    );
    assert_eq!(counters.started.get(), 1);
    assert_eq!(
        counters.dropped.get(),
        1,
        "the drop of `Serve` dropped the source"
    );
    let mut stream = resolved(poll_once(&mut call), "the first element");
    let mut outcome = None;
    for _ in 0..100 {
        match next_once(&mut stream) {
            Poll::Ready(Some(Ok(_))) => {}
            other => {
                outcome = Some(other);
                break;
            }
        }
    }
    assert_eq!(
        outcome,
        Some(Poll::Ready(Some(Err(ClientError::Call(
            CallError::Transport(Transport::Interrupted)
        ))))),
        "the consumer reads the interruption after the elements already pushed"
    );
    drop(stream);
    drop(call);

    // The member is free: a new `serve` over the same handler starts a new
    // source for a new call.
    let mut serve = generated::log::serve(&mut handler, &mut feed);
    let mut call = client.history(Level::new_unchecked(3));
    assert!(poll_once(&mut call).is_pending());
    for _ in 0..2 {
        assert!(poll_once(&mut serve).is_pending());
    }
    let mut stream = resolved(poll_once(&mut call), "the first element of a fresh call");
    assert!(matches!(next_once(&mut stream), Poll::Ready(Some(Ok(_)))));
    assert_eq!(counters.started.get(), 2);
}

// ---------------------------------------------------------------------------
// Two streamed queries on one interface.
// ---------------------------------------------------------------------------

/// A source of `count` values built by `make`, then the end.
struct Counted<T> {
    next: i64,
    count: i64,
    make: fn(i64) -> T,
}

impl<T> Source for Counted<T> {
    type Item = T;

    fn poll_next(&mut self, _cx: &mut Context<'_>) -> Poll<Option<Result<T, Interrupted>>> {
        if self.next == self.count {
            return Poll::Ready(None);
        }
        self.next += 1;
        Poll::Ready(Some(Ok((self.make)(self.next))))
    }
}

/// The provider of `Twin`: `left` gives readings, `right` gives levels.
struct Pair;

impl generated::twin::Provider for Pair {
    type Left = Counted<Reading>;
    type Right = Counted<Level>;

    fn left(&mut self, limit: &Level) -> Counted<Reading> {
        Counted {
            next: 0,
            count: limit.get(),
            make: |i| reading(i, true),
        }
    }

    fn right(&mut self, limit: &Level) -> Counted<Level> {
        Counted {
            next: 0,
            count: limit.get(),
            make: Level::new_unchecked,
        }
    }
}

#[test]
fn serve_drives_the_source_of_each_streamed_query_of_an_interface() {
    let rt = loopback();
    let mut provider = Pair;
    let mut serve = generated::twin::serve(rt.handler(), &mut provider);
    let mut left_client = generated::twin::Client::new(rt.caller());
    let mut right_client = generated::twin::Client::new(rt.caller());

    let mut left = left_client.left(Level::new_unchecked(3));
    let mut right = right_client.right(Level::new_unchecked(2));
    assert!(poll_once(&mut left).is_pending());
    assert!(poll_once(&mut right).is_pending());
    for _ in 0..6 {
        assert!(poll_once(&mut serve).is_pending());
    }
    let waker = ridl_rt::task::noop_waker();
    let mut cx = Context::from_waker(&waker);

    let mut left = match poll_once(&mut left) {
        Poll::Ready(Ok(stream)) => stream,
        _ => panic!("`left`'s source was driven to its first element"),
    };
    let mut got_left = Vec::new();
    while let Poll::Ready(Some(element)) = left.poll_next(&mut cx) {
        got_left.push(element.expect("an element"));
    }
    assert_eq!(
        got_left,
        vec![reading(1, true), reading(2, true), reading(3, true)]
    );

    let mut right = match poll_once(&mut right) {
        Poll::Ready(Ok(stream)) => stream,
        _ => panic!("`right`'s source was driven to its first element"),
    };
    let mut got_right = Vec::new();
    while let Poll::Ready(Some(element)) = right.poll_next(&mut cx) {
        got_right.push(element.expect("an element").get());
    }
    assert_eq!(
        got_right,
        vec![1, 2],
        "the second member's source was driven too"
    );
}
