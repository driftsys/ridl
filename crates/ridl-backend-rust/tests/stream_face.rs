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
    type Checked = IgnoreUpload;
    fn checked(&mut self, _: &generated::LogCheckedParams) -> IgnoreUpload {
        IgnoreUpload
    }
    fn checked_reply(&mut self, _: IgnoreUpload) -> Level {
        Level::new_unchecked(0)
    }
    type Summarize = IgnoreUpload;
    type Pipe = IgnoreUpload;
    fn summarize(&mut self, _: &generated::LogSummarizeParams) -> IgnoreUpload {
        IgnoreUpload
    }
    fn summarize_reply(&mut self, _: IgnoreUpload) -> generated::Summary {
        generated::Summary { count: 0, total: 0 }
    }
    fn pipe(&mut self, _: &generated::LogPipeParams) -> IgnoreUpload {
        IgnoreUpload
    }
    type History = Scripted;
    type Upload = IgnoreUpload;
    fn upload(&mut self, _: &generated::LogUploadParams) -> IgnoreUpload {
        IgnoreUpload
    }

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
    fn claim_forget(&self, claim: ClaimId) -> ridl_rt::port::ClaimForget {
        self.inner.claim_forget(claim)
    }
    fn wake_on_claim_forget(&self, claim: ClaimId, waker: &Waker) {
        self.inner.wake_on_claim_forget(claim, waker);
    }

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
fn a_handler_with_the_provided_defaults_keeps_a_pending_source_until_the_next_push() {
    let rt = loopback();
    let (mut feed, counters) = Feed::new(Script {
        endless: true,
        pending_before_each: 1,
        ..Script::default()
    });
    let mut serve = generated::log::serve(LegacyHandler(rt.handler()), &mut feed);
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
fn serve_does_not_wake_itself_on_the_poll_after_one_that_started_a_stream_and_used_the_budget() {
    let rt = loopback();
    let (mut feed, counters) = Feed::new(Script {
        endless: true,
        pending_before_each: 5,
        ..Script::default()
    });
    let extra = Rc::new(Cell::new(0));
    let handler = Crowded {
        inner: rt.handler(),
        extra: Rc::clone(&extra),
        taken: Rc::new(Cell::new(0)),
    };
    let mut serve = generated::log::serve(handler, &mut feed);
    let mut client = generated::log::Client::new(rt.caller());
    let mut call = client.history(Level::new_unchecked(3));
    assert!(poll_once(&mut call).is_pending());

    // The first poll starts the stream and takes 32 claims, so it uses the
    // whole budget and wakes itself for two reasons at once.
    extra.set(31);
    assert!(poll_once(&mut serve).is_pending());
    assert_eq!(counters.started.get(), 1);

    // The second poll has no claim to take, and the source answers `Pending`:
    // the start of the stream was already answered by the first wake.
    let wakes = CountWakes::new();
    let waker = Waker::from(Arc::clone(&wakes));
    assert!(poll_with(&mut serve, &waker).is_pending());
    assert_eq!(
        counters.polls.get(),
        1,
        "the source was polled and was not ready"
    );
    assert_eq!(
        wakes.count(),
        0,
        "the start of a stream is answered once, even when the budget was used up"
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
    // call. The next serve poll queries the forget before a push; it must
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
fn forgetting_a_call_wakes_serve_while_its_source_ignores_the_waker() {
    let rt = loopback();
    let (mut feed, counters) = Feed::new(Script {
        pending_before_each: usize::MAX,
        ..Script::default()
    });
    let mut serve = generated::log::serve(rt.handler(), &mut feed);
    let mut client = generated::log::Client::new(rt.caller());
    let call = sent(&mut client, &mut serve);
    let wakes = CountWakes::new();
    let waker = Waker::from(wakes.clone());
    assert!(
        Pin::new(&mut serve)
            .poll(&mut Context::from_waker(&waker))
            .is_pending()
    );
    assert_eq!(counters.polls.get(), 1, "the source returned Pending");
    let before = wakes.count();
    drop(call);
    assert!(
        wakes.count() > before,
        "forget wakes serve even though the source stored no waker"
    );
    assert!(poll_once(&mut serve).is_pending());
    assert_eq!(counters.dropped.get(), 1);
    assert_eq!(counters.polls.get(), 1, "forget does not poll the source");
}

#[test]
fn a_call_dropped_while_its_source_is_pending_releases_without_polling_the_source() {
    let rt = loopback();
    let (mut feed, counters) = Feed::new(Script {
        endless: true,
        pending_before_each: 3,
        ..Script::default()
    });
    let mut serve = generated::log::serve(rt.handler(), &mut feed);
    let mut client = generated::log::Client::new(rt.caller());

    for round in 1..=Loopback::SLOTS + 1 {
        let call = sent(&mut client, &mut serve);
        assert!(poll_once(&mut serve).is_pending());
        let polls = counters.polls.get();
        drop(call);
        assert!(poll_once(&mut serve).is_pending());
        assert_eq!(
            counters.dropped.get(),
            round,
            "the pending source was dropped"
        );
        assert_eq!(
            counters.polls.get(),
            polls,
            "forget must not poll the source"
        );
    }
    let mut call = sent(&mut client, &mut serve);
    for _ in 0..4 {
        assert!(poll_once(&mut serve).is_pending());
    }
    let mut stream = resolved(poll_once(&mut call), "the next call succeeds");
    assert!(matches!(next_once(&mut stream), Poll::Ready(Some(Ok(_)))));
    assert_eq!(counters.started.get(), Loopback::SLOTS + 2);
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
    assert_eq!(
        next_once(&mut stream),
        Poll::Ready(None),
        "the interrupted stream is over: the drop of `Serve` released the old claim"
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

/// Two calls, one per streamed query of `Twin`, with `serve` polled until
/// both streams run. `left_limit` and `right_limit` are the values of the
/// sources.
macro_rules! twin_streams {
    ($rt:ident, $handler:ident, $serve:ident, $left:ident, $right:ident, $left_limit:expr, $right_limit:expr) => {
        let mut provider = Pair;
        // The handler is lent, so it outlives `serve`: a stream is interrupted
        // only if the drop of `serve` aborts it.
        let mut $handler = $rt.handler();
        let mut $serve = generated::twin::serve(&mut $handler, &mut provider);
        let mut left_client = generated::twin::Client::new($rt.caller());
        let mut right_client = generated::twin::Client::new($rt.caller());
        let mut $left = left_client.left(Level::new_unchecked($left_limit));
        let mut $right = right_client.right(Level::new_unchecked($right_limit));
        assert!(poll_once(&mut $left).is_pending());
        assert!(poll_once(&mut $right).is_pending());
    };
}

#[test]
fn dropping_serve_interrupts_the_running_stream_of_every_member() {
    let rt = loopback();
    twin_streams!(rt, handler, serve, left_call, right_call, 100, 100);
    for _ in 0..3 {
        assert!(poll_once(&mut serve).is_pending());
    }
    // Both streams run and neither has ended: the sources hold 100 values.
    drop(serve);

    let waker = ridl_rt::task::noop_waker();
    let mut cx = Context::from_waker(&waker);
    let interrupted = ClientError::Call(CallError::Transport(Transport::Interrupted));

    let mut left = match poll_once(&mut left_call) {
        Poll::Ready(Ok(stream)) => stream,
        _ => panic!("`left` has its first element"),
    };
    let mut outcome = None;
    for _ in 0..100 {
        match left.poll_next(&mut cx) {
            Poll::Ready(Some(Ok(_))) => {}
            other => {
                outcome = Some(other);
                break;
            }
        }
    }
    assert_eq!(
        outcome,
        Some(Poll::Ready(Some(Err(interrupted)))),
        "`left` reads Interrupted"
    );

    let mut right = match poll_once(&mut right_call) {
        Poll::Ready(Ok(stream)) => stream,
        _ => panic!("`right` has its first element"),
    };
    let mut outcome = None;
    for _ in 0..100 {
        match right.poll_next(&mut cx) {
            Poll::Ready(Some(Ok(_))) => {}
            other => {
                outcome = Some(other);
                break;
            }
        }
    }
    assert_eq!(
        outcome,
        Some(Poll::Ready(Some(Err(interrupted)))),
        "`right` reads Interrupted"
    );
}

#[test]
fn serve_wakes_itself_when_only_the_second_member_pushed() {
    let rt = loopback();
    // `left` has no value and ends at its first drive; `right` has many.
    twin_streams!(rt, handler, serve, left_call, right_call, 0, 100);
    // The first poll starts both streams; the second ends `left`'s and
    // pushes the first element of `right`.
    for _ in 0..2 {
        assert!(poll_once(&mut serve).is_pending());
    }
    let wakes = CountWakes::new();
    let waker = Waker::from(Arc::clone(&wakes));
    assert!(poll_with(&mut serve, &waker).is_pending());
    assert_eq!(
        wakes.count(),
        1,
        "only `right` took an element, and that alone wakes the future"
    );
    drop((left_call, right_call));
}

// Byte and text replies use raw chunks rather than encoded named elements.
const DOCS: InterfaceNo = <generated::Docs as Interface>::NUMBER;
const FETCH: Ordinal = <generated::DocsFetch as Interaction>::MEMBER.ordinal;
const TEXT: Ordinal = <generated::DocsText as Interaction>::MEMBER.ordinal;
const BYTE_LIMIT: usize = generated::docs::FetchStream::<ridl_loopback::CallerHandle>::LIMIT;
const TEXT_LIMIT: usize = generated::docs::TextStream::<ridl_loopback::CallerHandle>::LIMIT;

fn utf8_failure() -> CallError {
    CallError::Contract(Contract::InvalidValue(Violation {
        type_name: Some("string"),
        rule: Rule::Utf8,
    }))
}

struct DocumentSource {
    bytes: Vec<u8>,
    position: usize,
    step: usize,
    pending_at_end: bool,
}

impl ridl_rt::face::ByteSource for DocumentSource {
    fn poll_read(
        &mut self,
        _cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<Result<usize, Interrupted>> {
        if self.position == self.bytes.len() && self.pending_at_end {
            return Poll::Pending;
        }
        let n = buf
            .len()
            .min(self.step)
            .min(self.bytes.len() - self.position);
        buf[..n].copy_from_slice(&self.bytes[self.position..self.position + n]);
        self.position += n;
        Poll::Ready(Ok(n))
    }
}

struct Documents {
    bytes: Vec<u8>,
    step: usize,
    pending_at_end: bool,
}

impl Documents {
    fn source(&self) -> DocumentSource {
        DocumentSource {
            bytes: self.bytes.clone(),
            position: 0,
            step: self.step,
            pending_at_end: self.pending_at_end,
        }
    }
}

impl generated::docs::Provider for Documents {
    type Put = IgnoreUpload;
    type Note = IgnoreUpload;
    fn put(&mut self, _: &generated::Info) -> IgnoreUpload {
        IgnoreUpload
    }
    fn note(&mut self, _: &generated::DocsNoteParams) -> IgnoreUpload {
        IgnoreUpload
    }

    type Fetch = DocumentSource;
    type Text = DocumentSource;
    fn fetch(&mut self, _id: &generated::Id) -> Self::Fetch {
        self.source()
    }
    fn text(&mut self, _id: &generated::Id) -> Self::Text {
        self.source()
    }
}

fn document_handler(rt: &Loopback, ordinal: Ordinal) -> HandlerHandle {
    let mut handler = rt.handler();
    handler
        .serve(DOCS, &[ordinal])
        .expect("serve document query");
    handler
}

#[test]
fn a_byte_reply_read_with_a_short_buffer_is_byte_equal_to_what_was_pushed() {
    let rt = loopback();
    let mut handler = document_handler(&rt, FETCH);
    let mut client = generated::docs::Client::new(rt.caller());
    let mut call = client.fetch(generated::Id::new_unchecked(1));
    let mut args = [0; 64];
    let claim = handler.next_claim(&mut args).unwrap().unwrap().id;
    let bytes: Vec<u8> = (0..BYTE_LIMIT * 2 + 5).map(|n| (n % 251) as u8).collect();
    for chunk in bytes.chunks(BYTE_LIMIT) {
        assert_eq!(
            handler.push_claim(claim, REPLY_STREAM, chunk),
            Ok(chunk.len())
        );
    }
    handler.end_claim(claim, REPLY_STREAM).unwrap();
    let mut stream = document_resolved(poll_once(&mut call), "first byte chunk");
    let mut got = Vec::new();
    let mut buf = [0; 7];
    for _ in 0..bytes.len() + 1 {
        let result = {
            let mut read = pin!(stream.read(&mut buf));
            poll_once(&mut read)
        };
        match result {
            Poll::Ready(Ok(0)) => break,
            Poll::Ready(Ok(n)) => got.extend_from_slice(&buf[..n]),
            other => panic!("unexpected read: {other:?}"),
        }
    }
    assert_eq!(got, bytes);
    assert_eq!(
        poll_once(&mut pin!(stream.read(&mut buf))),
        Poll::Ready(Ok(0))
    );
}

#[test]
fn a_text_reply_whose_code_points_straddle_every_boundary_arrives_unsplit() {
    let text = "€🦀🦀".repeat(TEXT_LIMIT / 2 + 3);
    for step in [1, TEXT_LIMIT] {
        let rt = loopback();
        let mut documents = Documents {
            bytes: text.as_bytes().to_vec(),
            step,
            pending_at_end: false,
        };
        let mut serve = generated::docs::serve(rt.handler(), &mut documents);
        let mut client = generated::docs::Client::new(rt.caller());
        let mut call = client.text(generated::Id::new_unchecked(1));
        assert!(poll_once(&mut serve).is_pending());
        let mut remaining = text.len() + 10;
        let mut stream = loop {
            assert!(remaining > 0, "the text stream must start");
            remaining -= 1;
            assert!(poll_once(&mut serve).is_pending());
            if let Poll::Ready(result) = poll_once(&mut call) {
                break result.unwrap();
            }
        };
        // With full reads this fills the port window and holds the next
        // chunk before the caller starts draining it.
        for _ in 0..8 {
            assert!(poll_once(&mut serve).is_pending());
        }
        let mut got = String::new();
        let mut chunks = 0;
        for _ in 0..text.len() + 10 {
            assert!(poll_once(&mut serve).is_pending());
            match poll_once(&mut pin!(stream.next())) {
                Poll::Ready(Some(Ok(chunk))) => {
                    assert!(chunk.len() <= TEXT_LIMIT);
                    got.push_str(chunk);
                    chunks += 1;
                }
                Poll::Ready(None) => break,
                Poll::Pending => {}
                Poll::Ready(Some(Err(error))) => panic!("text failed: {error:?}"),
            }
        }
        assert!(
            chunks >= 6,
            "the source crosses the four-chunk loopback window"
        );
        assert_eq!(got, text, "source read size {step}");
        let waker = Waker::noop();
        assert_eq!(
            stream.poll_next(&mut Context::from_waker(waker)),
            Poll::Ready(None)
        );
    }
}

#[test]
fn a_text_source_that_ends_inside_a_code_point_is_refused_with_utf8() {
    let rt = loopback();
    let expected = "a".repeat(TEXT_LIMIT + 5);
    let mut bytes = expected.as_bytes().to_vec();
    bytes.extend_from_slice(&[0xf0, 0x9f]);
    let mut documents = Documents {
        bytes,
        step: TEXT_LIMIT,
        pending_at_end: false,
    };
    let mut serve = generated::docs::serve(rt.handler(), &mut documents);
    let mut client = generated::docs::Client::new(rt.caller());
    let mut call = client.text(generated::Id::new_unchecked(1));
    for _ in 0..4 {
        assert!(poll_once(&mut serve).is_pending());
    }
    let mut stream = document_resolved(poll_once(&mut call), "whole text before incomplete tail");
    let mut got = String::new();
    let mut refused = false;
    for _ in 0..12 {
        assert!(poll_once(&mut serve).is_pending());
        match poll_once(&mut pin!(stream.next())) {
            Poll::Ready(Some(Ok(chunk))) => got.push_str(chunk),
            Poll::Ready(Some(Err(error))) => {
                assert_eq!(error, ClientError::Call(utf8_failure()));
                refused = true;
                break;
            }
            other => panic!("expected text or UTF-8 refusal: {other:?}"),
        }
    }
    assert!(refused);
    assert_eq!(got, expected);
    assert_eq!(poll_once(&mut pin!(stream.next())), Poll::Ready(None));
}

#[test]
fn a_text_source_with_invalid_bytes_is_refused_before_they_are_pushed() {
    let rt = loopback();
    let expected = vec![b'b'; TEXT_LIMIT + 7];
    let mut bytes = expected.clone();
    bytes.extend_from_slice(b"\xfftail");
    let mut documents = Documents {
        bytes,
        step: TEXT_LIMIT,
        pending_at_end: false,
    };
    let mut serve = generated::docs::serve(rt.handler(), &mut documents);
    let mut caller = rt.caller();
    let correlation = generated::docs::send_text(&mut caller, &generated::Id::new_unchecked(1))
        .unwrap()
        .0;
    let mut got = Vec::new();
    let mut refused = false;
    let mut buf = [0; TEXT_LIMIT];
    for _ in 0..20 {
        assert!(poll_once(&mut serve).is_pending());
        match caller.pull(correlation, REPLY_STREAM, &mut buf).unwrap() {
            Pull::Chunk(n) => {
                assert!(!buf[..n].contains(&0xff));
                got.extend_from_slice(&buf[..n]);
            }
            Pull::Pending => {}
            Pull::Failed(error) => {
                assert_eq!(error, utf8_failure());
                refused = true;
                break;
            }
            other => panic!("expected chunks and refusal: {other:?}"),
        }
    }
    assert!(refused);
    assert_eq!(got, expected);
    caller.forget(correlation);
}

#[test]
fn an_invalid_text_chunk_pushed_by_hand_is_refused_before_the_application() {
    let rt = loopback();
    let mut handler = document_handler(&rt, TEXT);
    let mut client = generated::docs::Client::new(rt.caller());
    let mut call = client.text(generated::Id::new_unchecked(1));
    let mut args = [0; 64];
    let claim = handler.next_claim(&mut args).unwrap().unwrap().id;
    assert_eq!(handler.push_claim(claim, REPLY_STREAM, b"a\xffb"), Ok(3));
    let mut stream = document_resolved(
        poll_once(&mut call),
        "the stream with an invalid first chunk",
    );
    let mut cx = Context::from_waker(Waker::noop());
    assert_eq!(
        stream.poll_next(&mut cx),
        Poll::Ready(Some(Err(ClientError::Call(utf8_failure()))))
    );
    assert_eq!(stream.poll_next(&mut cx), Poll::Ready(None));
    assert_eq!(
        handler.push_claim(claim, REPLY_STREAM, b"next"),
        Err(PushError::Failed(utf8_failure()))
    );
}

#[test]
fn a_byte_source_that_is_pending_sends_its_partial_chunk() {
    let rt = loopback();
    let expected = vec![42; 10];
    let mut documents = Documents {
        bytes: expected.clone(),
        step: 10,
        pending_at_end: true,
    };
    let mut serve = generated::docs::serve(rt.handler(), &mut documents);
    let mut caller = rt.caller();
    let correlation = generated::docs::send_fetch(&mut caller, &generated::Id::new_unchecked(1))
        .unwrap()
        .0;
    for _ in 0..3 {
        assert!(poll_once(&mut serve).is_pending());
    }
    let mut buf = [0; BYTE_LIMIT];
    assert_eq!(
        caller.pull(correlation, REPLY_STREAM, &mut buf),
        Ok(Pull::Chunk(10))
    );
    assert_eq!(&buf[..10], expected);
    assert_eq!(
        caller.pull(correlation, REPLY_STREAM, &mut buf),
        Ok(Pull::Pending)
    );
    caller.forget(correlation);
}

fn document_resolved<T>(polled: Poll<Result<T, ClientError>>, what: &str) -> T {
    match polled {
        Poll::Ready(Ok(value)) => value,
        Poll::Ready(Err(error)) => panic!("{what}: {error:?}"),
        Poll::Pending => panic!("{what}: still pending"),
    }
}

struct InterruptedTextSource(bool);

impl ridl_rt::face::ByteSource for InterruptedTextSource {
    fn poll_read(
        &mut self,
        _cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<Result<usize, Interrupted>> {
        if self.0 {
            Poll::Ready(Err(Interrupted))
        } else {
            self.0 = true;
            buf[0] = 0xf0;
            Poll::Ready(Ok(1))
        }
    }
}

struct InterruptingDocuments;

impl generated::docs::Provider for InterruptingDocuments {
    type Put = IgnoreUpload;
    type Note = IgnoreUpload;
    fn put(&mut self, _: &generated::Info) -> IgnoreUpload {
        IgnoreUpload
    }
    fn note(&mut self, _: &generated::DocsNoteParams) -> IgnoreUpload {
        IgnoreUpload
    }

    type Fetch = InterruptedTextSource;
    type Text = InterruptedTextSource;
    fn fetch(&mut self, _id: &generated::Id) -> Self::Fetch {
        InterruptedTextSource(false)
    }
    fn text(&mut self, _id: &generated::Id) -> Self::Text {
        InterruptedTextSource(false)
    }
}

#[test]
fn a_text_source_interruption_with_an_incomplete_suffix_stays_interrupted() {
    let rt = loopback();
    let mut documents = InterruptingDocuments;
    let mut serve = generated::docs::serve(rt.handler(), &mut documents);
    let mut caller = rt.caller();
    let correlation = generated::docs::send_text(&mut caller, &generated::Id::new_unchecked(1))
        .unwrap()
        .0;
    for _ in 0..3 {
        assert!(poll_once(&mut serve).is_pending());
    }
    let mut buf = [0; TEXT_LIMIT];
    assert_eq!(
        caller.pull(correlation, REPLY_STREAM, &mut buf),
        Ok(Pull::Failed(CallError::Transport(Transport::Interrupted)))
    );
    caller.forget(correlation);
}

struct CountedByteSource(Rc<Cell<usize>>);
impl ridl_rt::face::ByteSource for CountedByteSource {
    fn poll_read(
        &mut self,
        _cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<Result<usize, Interrupted>> {
        self.0.set(self.0.get() + 1);
        buf[0] = b'a';
        Poll::Ready(Ok(1))
    }
}

struct CountedDocuments {
    fetch: Rc<Cell<usize>>,
    text: Rc<Cell<usize>>,
}
impl generated::docs::Provider for CountedDocuments {
    type Put = IgnoreUpload;
    type Note = IgnoreUpload;
    fn put(&mut self, _: &generated::Info) -> IgnoreUpload {
        IgnoreUpload
    }
    fn note(&mut self, _: &generated::DocsNoteParams) -> IgnoreUpload {
        IgnoreUpload
    }

    type Fetch = CountedByteSource;
    type Text = CountedByteSource;
    fn fetch(&mut self, _id: &generated::Id) -> Self::Fetch {
        CountedByteSource(self.fetch.clone())
    }
    fn text(&mut self, _id: &generated::Id) -> Self::Text {
        CountedByteSource(self.text.clone())
    }
}

#[test]
fn serve_polls_each_byte_source_once_without_starving_the_next_member() {
    let rt = loopback();
    let fetch = Rc::new(Cell::new(0));
    let text = Rc::new(Cell::new(0));
    let mut documents = CountedDocuments {
        fetch: fetch.clone(),
        text: text.clone(),
    };
    let mut serve = generated::docs::serve(rt.handler(), &mut documents);
    let mut fetch_client = generated::docs::Client::new(rt.caller());
    let mut text_client = generated::docs::Client::new(rt.caller());
    let _fetch = fetch_client.fetch(generated::Id::new_unchecked(1));
    let _text = text_client.text(generated::Id::new_unchecked(1));
    assert!(poll_once(&mut serve).is_pending());
    assert_eq!((fetch.get(), text.get()), (0, 0));
    assert!(poll_once(&mut serve).is_pending());
    assert_eq!((fetch.get(), text.get()), (1, 1));
}

// Controls for the byte/text paths, separate from named elements.
fn document_claim(handler: &mut HandlerHandle) -> ClaimId {
    handler.next_claim(&mut [0; 64]).unwrap().unwrap().id
}

/// Reuses the crowded handler's counters, but presents valid byte-query
/// claims behind the two real calls. Those extra claims settle Busy.
struct ByteClaims {
    inner: Crowded,
    args: Vec<u8>,
}
impl Attached for ByteClaims {
    fn catalog(&self) -> &CatalogRef {
        self.inner.catalog()
    }
}
impl Encoded for ByteClaims {
    type Encoding = FlatBuffers;
}
impl Handler for ByteClaims {
    fn serve(
        &mut self,
        iface: InterfaceNo,
        ords: &[Ordinal],
    ) -> Result<(), ridl_rt::port::ServeError> {
        self.inner.serve(iface, ords)
    }
    fn next_claim(&mut self, out: &mut [u8]) -> Result<Option<ridl_rt::port::Claim>, ReadError> {
        let Some(mut claim) = self.inner.next_claim(out)? else {
            return Ok(None);
        };
        if claim.id.0 > CROWD {
            out[..self.args.len()].copy_from_slice(&self.args);
            claim.iface = DOCS;
            claim.ord = FETCH;
            claim.len = self.args.len();
        }
        Ok(Some(claim))
    }
    fn settle(
        &mut self,
        claim: ClaimId,
        outcome: Result<&[u8], CallError>,
    ) -> Result<(), ridl_rt::port::SettleError> {
        if claim.0 > CROWD {
            assert_eq!(outcome, Err(CallError::Transport(Transport::Busy)));
        }
        self.inner.settle(claim, outcome)
    }
}
impl StreamHandler for ByteClaims {
    fn claim_forget(&self, claim: ClaimId) -> ridl_rt::port::ClaimForget {
        self.inner.claim_forget(claim)
    }
    fn wake_on_claim_forget(&self, claim: ClaimId, waker: &Waker) {
        self.inner.wake_on_claim_forget(claim, waker);
    }

    fn pull_claim(
        &mut self,
        c: ClaimId,
        s: ridl_rt::contract::StreamNo,
        out: &mut [u8],
    ) -> Result<Pull, ReadError> {
        self.inner.pull_claim(c, s, out)
    }
    fn push_claim(
        &mut self,
        c: ClaimId,
        s: ridl_rt::contract::StreamNo,
        bytes: &[u8],
    ) -> Result<usize, PushError> {
        self.inner.push_claim(c, s, bytes)
    }
    fn end_claim(&mut self, c: ClaimId, s: ridl_rt::contract::StreamNo) -> Result<(), PushError> {
        self.inner.end_claim(c, s)
    }
    fn abort_claim(
        &mut self,
        c: ClaimId,
        s: ridl_rt::contract::StreamNo,
        e: CallError,
    ) -> Result<(), PushError> {
        self.inner.abort_claim(c, s, e)
    }
}
impl Wakeable for ByteClaims {
    fn wake_on(&self, interest: Interest, waker: &Waker) {
        self.inner.wake_on(interest, waker);
    }
}

#[test]
fn byte_source_polls_debit_the_shared_budget_before_ready_byte_claims() {
    let rt = loopback();
    let fetch = Rc::new(Cell::new(0));
    let text = Rc::new(Cell::new(0));
    let mut provider = CountedDocuments {
        fetch: fetch.clone(),
        text: text.clone(),
    };
    let extra = Rc::new(Cell::new(0));
    let taken = Rc::new(Cell::new(0));
    let mut buf = <generated::Id as ridl_rt::payload::Payload<FlatBuffers>>::BUFFER;
    let args =
        Ref::<generated::Id, FlatBuffers>::encode(&generated::Id::new_unchecked(1), &mut buf)
            .unwrap()
            .bytes()
            .to_vec();
    let handler = ByteClaims {
        inner: Crowded {
            inner: rt.handler(),
            extra: extra.clone(),
            taken: taken.clone(),
        },
        args,
    };
    let mut serve = generated::docs::serve(handler, &mut provider);
    let mut fetch_client = generated::docs::Client::new(rt.caller());
    let mut text_client = generated::docs::Client::new(rt.caller());
    let _fetch = fetch_client.fetch(generated::Id::new_unchecked(1));
    let _text = text_client.text(generated::Id::new_unchecked(1));
    assert!(poll_once(&mut serve).is_pending());
    extra.set(36);
    assert!(poll_once(&mut serve).is_pending());
    assert_eq!((fetch.get(), text.get()), (1, 1));
    assert_eq!(taken.get(), 30, "two source polls leave thirty claim units");
    assert_eq!(extra.get(), 6);
    assert!(poll_once(&mut serve).is_pending());
    assert_eq!(taken.get(), 36);
}

#[test]
fn binary_bytes_round_trip_through_generated_serve_and_end() {
    let rt = loopback();
    let expected: Vec<u8> = (0..BYTE_LIMIT * 3 + 19)
        .map(|n| [0xff, 0x80, 0xc3, 0, 0x7f][n % 5])
        .collect();
    let mut provider = Documents {
        bytes: expected.clone(),
        step: BYTE_LIMIT,
        pending_at_end: false,
    };
    let mut serve = generated::docs::serve(rt.handler(), &mut provider);
    let mut client = generated::docs::Client::new(rt.caller());
    let mut call = client.fetch(generated::Id::new_unchecked(1));
    for _ in 0..3 {
        assert!(poll_once(&mut serve).is_pending());
    }
    let mut stream = document_resolved(poll_once(&mut call), "the binary reply");
    let mut got = Vec::new();
    let mut ended = false;
    let mut cx = Context::from_waker(Waker::noop());
    let mut buf = [0; 37];
    for _ in 0..expected.len() + 8 {
        assert!(poll_once(&mut serve).is_pending());
        match stream.poll_read(&mut cx, &mut buf) {
            Poll::Ready(Ok(0)) => {
                ended = true;
                break;
            }
            Poll::Ready(Ok(n)) => got.extend_from_slice(&buf[..n]),
            Poll::Pending => {}
            Poll::Ready(Err(error)) => panic!("binary data was refused: {error:?}"),
        }
    }
    assert!(ended, "the byte source ends normally");
    assert_eq!(got, expected);
}

#[test]
fn a_pending_byte_stream_registers_a_waker_for_the_next_push() {
    let rt = loopback();
    let mut handler = document_handler(&rt, FETCH);
    let ports = RecordingPorts::new(&rt);
    let log = ports.log();
    let mut client = generated::docs::Client::new(ports);
    let mut call = client.fetch(generated::Id::new_unchecked(1));
    let c = the_one_send(&log);
    let claim = document_claim(&mut handler);
    handler.push_claim(claim, REPLY_STREAM, b"x").unwrap();
    let mut stream = document_resolved(poll_once(&mut call), "first byte chunk");
    let wakes = CountWakes::new();
    let waker = Waker::from(wakes.clone());
    let mut cx = Context::from_waker(&waker);
    let mut buf = [0; 8];
    assert_eq!(stream.poll_read(&mut cx, &mut buf), Poll::Ready(Ok(1)));
    doubles::take(&log);
    assert!(stream.poll_read(&mut cx, &mut buf).is_pending());
    assert_eq!(
        doubles::take(&log),
        vec![
            Op::WakeOn(Interest::Stream(c, REPLY_STREAM)),
            Op::Pull(c, REPLY_STREAM)
        ]
    );
    assert_eq!(wakes.count(), 0);
    handler.push_claim(claim, REPLY_STREAM, b"y").unwrap();
    assert_eq!(wakes.count(), 1);
    assert_eq!(stream.poll_read(&mut cx, &mut buf), Poll::Ready(Ok(1)));
    assert_eq!(buf[0], b'y');
}

#[test]
fn a_pending_text_stream_registers_a_waker_for_the_next_push() {
    let rt = loopback();
    let mut handler = document_handler(&rt, TEXT);
    let ports = RecordingPorts::new(&rt);
    let log = ports.log();
    let mut client = generated::docs::Client::new(ports);
    let mut call = client.text(generated::Id::new_unchecked(1));
    let c = the_one_send(&log);
    let claim = document_claim(&mut handler);
    handler.push_claim(claim, REPLY_STREAM, b"x").unwrap();
    let mut stream = document_resolved(poll_once(&mut call), "first text chunk");
    let wakes = CountWakes::new();
    let waker = Waker::from(wakes.clone());
    let mut cx = Context::from_waker(&waker);
    assert_eq!(stream.poll_next(&mut cx), Poll::Ready(Some(Ok("x"))));
    doubles::take(&log);
    assert!(stream.poll_next(&mut cx).is_pending());
    assert_eq!(
        doubles::take(&log),
        vec![
            Op::WakeOn(Interest::Stream(c, REPLY_STREAM)),
            Op::Pull(c, REPLY_STREAM)
        ]
    );
    assert_eq!(wakes.count(), 0);
    handler.push_claim(claim, REPLY_STREAM, b"y").unwrap();
    assert_eq!(wakes.count(), 1);
    assert_eq!(stream.poll_next(&mut cx), Poll::Ready(Some(Ok("y"))));
}

macro_rules! check_raw_first {
    (bytes, $stream:ident, $cx:expr) => {{
        let mut buf = [0; 8];
        assert_eq!($stream.poll_read($cx, &mut buf), Poll::Ready(Ok(1)));
        assert_eq!(buf[0], b'x');
    }};
    (text, $stream:ident, $cx:expr) => {
        assert_eq!($stream.poll_next($cx), Poll::Ready(Some(Ok("x"))));
    };
}
macro_rules! check_raw_end {
    (bytes, $stream:ident, $cx:expr) => {
        assert_eq!($stream.poll_read($cx, &mut [0; 8]), Poll::Ready(Ok(0)));
    };
    (text, $stream:ident, $cx:expr) => {
        assert_eq!($stream.poll_next($cx), Poll::Ready(None));
    };
}
macro_rules! check_raw_failure {
    (bytes, $stream:ident, $cx:expr, $error:expr) => {
        assert_eq!(
            $stream.poll_read($cx, &mut [0; 8]),
            Poll::Ready(Err(ClientError::Call($error)))
        );
    };
    (text, $stream:ident, $cx:expr, $error:expr) => {
        assert_eq!(
            $stream.poll_next($cx),
            Poll::Ready(Some(Err(ClientError::Call($error))))
        );
    };
}
macro_rules! raw_release_tests {
    ($module:ident, $method:ident, $ordinal:ident, $form:ident) => {
        mod $module {
            use super::*;
            #[test]
            fn end_forgets_once_before_drop() {
                let rt = loopback();
                let mut handler = document_handler(&rt, $ordinal);
                let ports = RecordingPorts::new(&rt);
                let log = ports.log();
                let mut client = generated::docs::Client::new(ports);
                let mut call = client.$method(generated::Id::new_unchecked(1));
                let c = the_one_send(&log);
                let claim = document_claim(&mut handler);
                handler.push_claim(claim, REPLY_STREAM, b"x").unwrap();
                let mut stream = document_resolved(poll_once(&mut call), "first chunk");
                let mut cx = Context::from_waker(Waker::noop());
                check_raw_first!($form, stream, &mut cx);
                assert!(doubles::forgets(&log).is_empty());
                handler.end_claim(claim, REPLY_STREAM).unwrap();
                check_raw_end!($form, stream, &mut cx);
                assert_eq!(doubles::forgets(&log), vec![c]);
                drop(stream);
                drop(call);
                assert_eq!(doubles::forgets(&log), vec![c]);
            }
            #[test]
            fn provider_failure_forgets_once_before_drop() {
                let rt = loopback();
                let mut handler = document_handler(&rt, $ordinal);
                let ports = RecordingPorts::new(&rt);
                let log = ports.log();
                let mut client = generated::docs::Client::new(ports);
                let mut call = client.$method(generated::Id::new_unchecked(1));
                let c = the_one_send(&log);
                let claim = document_claim(&mut handler);
                handler.push_claim(claim, REPLY_STREAM, b"x").unwrap();
                let mut stream = document_resolved(poll_once(&mut call), "first chunk");
                let mut cx = Context::from_waker(Waker::noop());
                check_raw_first!($form, stream, &mut cx);
                let error = CallError::Transport(Transport::Interrupted);
                handler.abort_claim(claim, REPLY_STREAM, error).unwrap();
                check_raw_failure!($form, stream, &mut cx, error);
                assert_eq!(doubles::forgets(&log), vec![c]);
                drop(stream);
                drop(call);
                assert_eq!(doubles::forgets(&log), vec![c]);
            }
            #[test]
            fn live_stream_drop_forgets_once_and_interrupts() {
                let rt = loopback();
                let mut handler = document_handler(&rt, $ordinal);
                let ports = RecordingPorts::new(&rt);
                let log = ports.log();
                let mut client = generated::docs::Client::new(ports);
                let mut call = client.$method(generated::Id::new_unchecked(1));
                let c = the_one_send(&log);
                let claim = document_claim(&mut handler);
                handler.push_claim(claim, REPLY_STREAM, b"x").unwrap();
                let stream = document_resolved(poll_once(&mut call), "live stream");
                assert!(doubles::forgets(&log).is_empty());
                drop(stream);
                drop(call);
                assert_eq!(doubles::forgets(&log), vec![c]);
                assert_eq!(
                    handler.push_claim(claim, REPLY_STREAM, b"y"),
                    Err(PushError::Failed(CallError::Transport(
                        Transport::Interrupted
                    )))
                );
            }
            #[test]
            fn failure_before_a_chunk_forgets_once() {
                let rt = loopback();
                let mut handler = document_handler(&rt, $ordinal);
                let ports = RecordingPorts::new(&rt);
                let log = ports.log();
                let mut client = generated::docs::Client::new(ports);
                let mut call = client.$method(generated::Id::new_unchecked(1));
                let c = the_one_send(&log);
                let claim = document_claim(&mut handler);
                let error = CallError::Transport(Transport::Interrupted);
                handler.abort_claim(claim, REPLY_STREAM, error).unwrap();
                assert_eq!(
                    poll_once(&mut call).map(|result| result.map(|_| ())),
                    Poll::Ready(Err(ClientError::Call(error)))
                );
                assert_eq!(doubles::forgets(&log), vec![c]);
                drop(call);
                assert_eq!(doubles::forgets(&log), vec![c]);
            }
            #[test]
            fn end_before_a_chunk_forgets_once() {
                let rt = loopback();
                let mut handler = document_handler(&rt, $ordinal);
                let ports = RecordingPorts::new(&rt);
                let log = ports.log();
                let mut client = generated::docs::Client::new(ports);
                let mut call = client.$method(generated::Id::new_unchecked(1));
                let c = the_one_send(&log);
                let claim = document_claim(&mut handler);
                handler.end_claim(claim, REPLY_STREAM).unwrap();
                let mut stream = document_resolved(poll_once(&mut call), "empty stream");
                assert_eq!(doubles::forgets(&log), vec![c]);
                let mut cx = Context::from_waker(Waker::noop());
                check_raw_end!($form, stream, &mut cx);
                drop(stream);
                drop(call);
                assert_eq!(doubles::forgets(&log), vec![c]);
            }
            #[test]
            fn waiting_call_drop_forgets_once_and_interrupts() {
                let rt = loopback();
                let mut handler = document_handler(&rt, $ordinal);
                let ports = RecordingPorts::new(&rt);
                let log = ports.log();
                let mut client = generated::docs::Client::new(ports);
                let mut call = client.$method(generated::Id::new_unchecked(1));
                let c = the_one_send(&log);
                let claim = document_claim(&mut handler);
                assert!(poll_once(&mut call).is_pending());
                assert!(doubles::forgets(&log).is_empty());
                drop(call);
                assert_eq!(doubles::forgets(&log), vec![c]);
                assert_eq!(
                    handler.push_claim(claim, REPLY_STREAM, b"y"),
                    Err(PushError::Failed(CallError::Transport(
                        Transport::Interrupted
                    )))
                );
            }
        }
    };
}
raw_release_tests!(fix_byte_release, fetch, FETCH, bytes);
raw_release_tests!(fix_text_release, text, TEXT, text);

#[test]
fn text_streams_skip_initial_and_later_empty_chunks() {
    let rt = loopback();
    let mut handler = document_handler(&rt, TEXT);
    let mut client = generated::docs::Client::new(rt.caller());
    let mut call = client.text(generated::Id::new_unchecked(1));
    let claim = document_claim(&mut handler);
    handler.push_claim(claim, REPLY_STREAM, b"").unwrap();
    handler.push_claim(claim, REPLY_STREAM, b"x").unwrap();
    let mut stream = document_resolved(poll_once(&mut call), "empty first chunk");
    let mut cx = Context::from_waker(Waker::noop());
    assert_eq!(stream.poll_next(&mut cx), Poll::Ready(Some(Ok("x"))));
    handler.push_claim(claim, REPLY_STREAM, b"").unwrap();
    assert!(stream.poll_next(&mut cx).is_pending());
    handler.push_claim(claim, REPLY_STREAM, b"").unwrap();
    handler.end_claim(claim, REPLY_STREAM).unwrap();
    assert_eq!(poll_once(&mut pin!(stream.next())), Poll::Ready(None));
}

struct IgnoreUpload;
impl ridl_rt::face::Consumer for IgnoreUpload {
    type Item = Reading;
    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<()> {
        Poll::Ready(())
    }
    fn accept(&mut self, _: Reading) {}
    fn end(&mut self) {}
    fn failed(&mut self, _: CallError) {}
}
impl ridl_rt::face::ByteConsumer for IgnoreUpload {
    fn poll_write(&mut self, _: &mut Context<'_>, bytes: &[u8]) -> Poll<usize> {
        Poll::Ready(bytes.len())
    }
    fn end(&mut self) {}
    fn failed(&mut self, _: CallError) {}
}

#[test]
fn serve_delivers_a_zero_byte_proto3_reply_once() {
    let rt = Loopback::<Proto3>::with_encoding(CATALOG);
    let script = Script {
        elements: vec![reading(0, false)],
        ..Script::default()
    };
    let (mut feed, _) = Feed::new(script);
    let mut serve = generated::log::serve(rt.handler(), &mut feed);
    let mut client = generated::log::Client::new(rt.caller());
    let mut call = client.history(Level::new_unchecked(3));
    assert!(poll_once(&mut serve).is_pending());
    assert!(poll_once(&mut serve).is_pending());
    let mut stream = match poll_once(&mut call) {
        Poll::Ready(Ok(s)) => s,
        _ => panic!("first element missing"),
    };
    let w = ridl_rt::task::noop_waker();
    let mut cx = Context::from_waker(&w);
    assert_eq!(
        stream.poll_next(&mut cx),
        Poll::Ready(Some(Ok(reading(0, false))))
    );
    assert!(poll_once(&mut serve).is_pending());
    assert_eq!(stream.poll_next(&mut cx), Poll::Ready(None));
}

impl ridl_rt::face::Source for IgnoreUpload {
    type Item = Reading;
    fn poll_next(
        &mut self,
        _: &mut Context<'_>,
    ) -> Poll<Option<Result<Reading, ridl_rt::face::Interrupted>>> {
        Poll::Ready(None)
    }
}

/// A runtime adapter that uses the provided forget defaults.
struct LegacyHandler(HandlerHandle);
impl Attached for LegacyHandler {
    fn catalog(&self) -> &CatalogRef {
        self.0.catalog()
    }
}
impl Encoded for LegacyHandler {
    type Encoding = FlatBuffers;
}
impl Handler for LegacyHandler {
    fn serve(
        &mut self,
        iface: InterfaceNo,
        ords: &[Ordinal],
    ) -> Result<(), ridl_rt::port::ServeError> {
        self.0.serve(iface, ords)
    }
    fn next_claim(&mut self, out: &mut [u8]) -> Result<Option<ridl_rt::port::Claim>, ReadError> {
        self.0.next_claim(out)
    }
    fn settle(
        &mut self,
        claim: ClaimId,
        outcome: Result<&[u8], CallError>,
    ) -> Result<(), ridl_rt::port::SettleError> {
        self.0.settle(claim, outcome)
    }
}
impl StreamHandler for LegacyHandler {
    fn pull_claim(
        &mut self,
        claim: ClaimId,
        stream: ridl_rt::contract::StreamNo,
        out: &mut [u8],
    ) -> Result<Pull, ReadError> {
        self.0.pull_claim(claim, stream, out)
    }
    fn push_claim(
        &mut self,
        claim: ClaimId,
        stream: ridl_rt::contract::StreamNo,
        chunk: &[u8],
    ) -> Result<usize, PushError> {
        self.0.push_claim(claim, stream, chunk)
    }
    fn end_claim(
        &mut self,
        claim: ClaimId,
        stream: ridl_rt::contract::StreamNo,
    ) -> Result<(), PushError> {
        self.0.end_claim(claim, stream)
    }
    fn abort_claim(
        &mut self,
        claim: ClaimId,
        stream: ridl_rt::contract::StreamNo,
        error: CallError,
    ) -> Result<(), PushError> {
        self.0.abort_claim(claim, stream, error)
    }
}
impl Wakeable for LegacyHandler {
    fn wake_on(&self, what: Interest, waker: &Waker) {
        self.0.wake_on(what, waker);
    }
}

struct PendingByteSource(Rc<Counters>);
impl ridl_rt::face::ByteSource for PendingByteSource {
    fn poll_read(&mut self, _: &mut Context<'_>, _: &mut [u8]) -> Poll<Result<usize, Interrupted>> {
        self.0.polls.set(self.0.polls.get() + 1);
        Poll::Pending
    }
}
impl Drop for PendingByteSource {
    fn drop(&mut self) {
        self.0.dropped.set(self.0.dropped.get() + 1);
    }
}
struct PendingDocuments(Rc<Counters>);
impl generated::docs::Provider for PendingDocuments {
    type Put = IgnoreUpload;
    type Note = IgnoreUpload;
    type Fetch = PendingByteSource;
    type Text = PendingByteSource;
    fn put(&mut self, _: &generated::Info) -> IgnoreUpload {
        IgnoreUpload
    }
    fn note(&mut self, _: &generated::DocsNoteParams) -> IgnoreUpload {
        IgnoreUpload
    }
    fn fetch(&mut self, _: &generated::Id) -> Self::Fetch {
        self.0.started.set(self.0.started.get() + 1);
        PendingByteSource(self.0.clone())
    }
    fn text(&mut self, _: &generated::Id) -> Self::Text {
        self.fetch(&generated::Id::new_unchecked(1))
    }
}

#[test]
fn forgetting_pending_byte_and_text_replies_drops_without_another_source_poll() {
    for text in [false, true] {
        let rt = loopback();
        let counters = Rc::new(Counters::default());
        let mut provider = PendingDocuments(counters.clone());
        let mut serve = generated::docs::serve(rt.handler(), &mut provider);
        let mut caller = rt.caller();
        for round in 1..=Loopback::SLOTS + 1 {
            let id = generated::Id::new_unchecked(1);
            let correlation = if text {
                generated::docs::send_text(&mut caller, &id).unwrap().0
            } else {
                generated::docs::send_fetch(&mut caller, &id).unwrap().0
            };
            assert!(poll_once(&mut serve).is_pending());
            assert_eq!(counters.started.get(), round);
            assert!(poll_once(&mut serve).is_pending());
            assert_eq!(counters.polls.get(), round);
            caller.forget(correlation);
            assert!(poll_once(&mut serve).is_pending());
            assert_eq!(counters.dropped.get(), round);
            assert_eq!(
                counters.polls.get(),
                round,
                "release does not poll the source"
            );
        }
    }
}

#[test]
fn a_forget_before_the_first_source_poll_admits_the_next_call_in_the_same_poll() {
    let rt = loopback();
    let (mut feed, counters) = Feed::new(Script {
        endless: true,
        ..Script::default()
    });
    let mut serve = generated::log::serve(rt.handler(), &mut feed);
    let mut client = generated::log::Client::new(rt.caller());
    let call = sent(&mut client, &mut serve);
    drop(call);
    let mut next = client.history(Level::new_unchecked(3));
    assert!(poll_once(&mut next).is_pending());
    assert!(poll_once(&mut serve).is_pending());
    assert_eq!(counters.polls.get(), 0, "forgotten source was never polled");
    assert_eq!(counters.dropped.get(), 1);
    assert_eq!(
        counters.started.get(),
        2,
        "next claim started in the release poll"
    );
    assert!(poll_once(&mut serve).is_pending());
    let mut stream = resolved(poll_once(&mut next), "the next call succeeds");
    assert!(matches!(next_once(&mut stream), Poll::Ready(Some(Ok(_)))));
}
