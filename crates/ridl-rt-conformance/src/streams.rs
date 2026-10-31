//! `StreamCaller` and `StreamHandler`: the streams of a call.
//!
//! The contract is the documentation of the two traits, of [`Pull`] and of
//! [`PushError`]: a stream is keyed by its call and a [`StreamNo`], where
//! [`REPLY_STREAM`] is the reply stream and `StreamNo(k)` from 1 is the
//! `k`-th stream parameter; the unit is the chunk, which a port never splits
//! or merges; a push takes the whole chunk or nothing, and answers 0 for a
//! non-empty chunk the full window did not take; an empty chunk is always
//! taken and is pulled as `Pull::Chunk(0)`; the reply stream ends with the
//! call's settlement; a stop on purpose is an abort, and a forget stops every
//! stream of the call that its producer has not ended with
//! `Transport::Interrupted`, and keeps the ended ones.
//!
//! Every test asks for `StreamCaller`, `StreamHandler` and `Wakeable` on
//! `F::Runtime`, so these tests are in their own arm of
//! [`suite!`](crate::suite). The tests send each call through the runtime
//! and take its claim through the same runtime, so one value holds both
//! sides of every stream.
//!
//! The suite attaches to a catalog with no descriptor, so a runtime under
//! test holds no member table and does not know which streams a member
//! declares. Such a runtime must accept [`REPLY_STREAM`] on a query, and
//! `StreamNo(1)` and `StreamNo(2)` on any call; the tests use no other
//! stream number, and a stream number beyond those is not tested.
//!
//! The window is the runtime's own choice. A test that needs a full window
//! pushes until a push answers 0, and does at most 1,024 pushes; a
//! runtime whose window never fills in that many pushes skips the part of
//! the test that needs a full window.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::task::{Wake, Waker};

use ridl_rt::contract::{default_ceiling, Kind};
use ridl_rt::error::{CallError, Contract, Transport};
use ridl_rt::payload::{Rule, Violation};
use ridl_rt::port::{
    Caller, ClaimId, Correlation, Handler, Interest, Pull, PushError, ReadError, SettleError,
    StreamCaller, StreamHandler, StreamNo, Wakeable, REPLY_STREAM,
};

use crate::{runtime, Factory, IFACE, ORD};

/// The first stream parameter.
const PARAM_1: StreamNo = StreamNo(1);
/// The second stream parameter.
const PARAM_2: StreamNo = StreamNo(2);

/// The most pushes a test makes to fill a window.
const MAX_PUSHES: usize = 1_024;

/// A waker that counts its wakes. Each one stands for a task.
struct Count(AtomicUsize);

impl Wake for Count {
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

/// A new task: the counter and a waker that increments it.
fn task() -> (Arc<Count>, Waker) {
    let count = Arc::new(Count(AtomicUsize::new(0)));
    (Arc::clone(&count), Waker::from(count))
}

fn wakes(count: &Count) -> usize {
    count.0.load(Ordering::SeqCst)
}

/// A runtime that serves `IFACE`/`ORD`, with one query sent through it and
/// claimed by it: the call's correlation and its claim.
fn claimed_query<F: Factory>() -> (F::Runtime, Correlation, ClaimId) {
    let mut rt = runtime::<F>();
    rt.serve(IFACE, &[ORD]).expect("serve");
    let c = rt.query(IFACE, ORD, &[1], None).expect("query sent");
    let claim = claim_next(&mut rt);
    (rt, c, claim)
}

/// The next claim `rt` presents.
fn claim_next(rt: &mut impl Handler) -> ClaimId {
    let mut buf = [0u8; 8];
    rt.next_claim(&mut buf)
        .expect("next_claim")
        .expect("the call just sent is waiting")
        .id
}

/// Pushes `&[1]` to the reply stream of `claim` until a push answers 0, at
/// most [`MAX_PUSHES`] times. Answers the number of chunks taken when a push
/// answered 0, and `None` when every push was taken.
fn fill_reply_window(rt: &mut impl StreamHandler, claim: ClaimId) -> Option<usize> {
    for taken in 0..MAX_PUSHES {
        match rt.push_claim(claim, REPLY_STREAM, &[1]) {
            Ok(1) => {}
            Ok(0) => return Some(taken),
            other => panic!("a push into an open stream answers 1 or 0, not {other:?}"),
        }
    }
    None
}

/// Pushes `&[1]` to the first parameter stream of `c` until a push answers
/// 0, at most [`MAX_PUSHES`] times, as [`fill_reply_window`] does for the
/// reply stream.
fn fill_parameter_window(rt: &mut impl StreamCaller, c: Correlation) -> Option<usize> {
    for taken in 0..MAX_PUSHES {
        match rt.push(c, PARAM_1, &[1]) {
            Ok(1) => {}
            Ok(0) => return Some(taken),
            other => panic!("a push into an open stream answers 1 or 0, not {other:?}"),
        }
    }
    None
}

/// The provider pushes three chunks to a query's reply stream and ends it;
/// the caller pulls the three in the order pushed, each with its bytes, and
/// then `End`, and `End` again on the next pull. Each chunk is pulled before
/// the next one is pushed, so a window of one chunk is enough.
pub fn a_reply_stream_is_pulled_in_order_then_ends<F>()
where
    F: Factory,
    F::Runtime: StreamCaller + StreamHandler + Wakeable,
{
    let (mut rt, c, claim) = claimed_query::<F>();
    let chunks: [&[u8]; 3] = [&[1], &[2, 2], &[3, 3, 3]];
    let mut out = [0u8; 8];
    for chunk in chunks {
        assert_eq!(
            rt.push_claim(claim, REPLY_STREAM, chunk),
            Ok(chunk.len()),
            "a push takes the whole chunk"
        );
        assert_eq!(
            rt.pull(c, REPLY_STREAM, &mut out),
            Ok(Pull::Chunk(chunk.len())),
            "the chunk is pulled whole, with its own length"
        );
        assert_eq!(&out[..chunk.len()], chunk);
    }
    rt.end_claim(claim, REPLY_STREAM).expect("end_claim");
    assert_eq!(rt.pull(c, REPLY_STREAM, &mut out), Ok(Pull::End));
    assert_eq!(
        rt.pull(c, REPLY_STREAM, &mut out),
        Ok(Pull::End),
        "pulling an ended stream again answers End again"
    );
}

/// A pull before the provider pushed anything answers `Pending`, before the
/// call is claimed and after.
pub fn a_pull_before_the_first_chunk_is_pending<F>()
where
    F: Factory,
    F::Runtime: StreamCaller + StreamHandler + Wakeable,
{
    let mut rt = runtime::<F>();
    rt.serve(IFACE, &[ORD]).expect("serve");
    let c = rt.query(IFACE, ORD, &[1], None).expect("query sent");
    let mut out = [0u8; 8];
    assert_eq!(
        rt.pull(c, REPLY_STREAM, &mut out),
        Ok(Pull::Pending),
        "nothing was pushed before the claim"
    );
    let _claim = claim_next(&mut rt);
    assert_eq!(
        rt.pull(c, REPLY_STREAM, &mut out),
        Ok(Pull::Pending),
        "nothing was pushed after the claim"
    );
}

/// An empty chunk is a chunk: its push answers 0 and it is taken, and the
/// receiver pulls `Chunk(0)`, not `End`. It is taken into a full window too,
/// because the window bounds the non-empty chunks only, and it is pulled
/// after the chunks that filled the window.
pub fn a_zero_length_chunk_is_a_chunk<F>()
where
    F: Factory,
    F::Runtime: StreamCaller + StreamHandler + Wakeable,
{
    let (mut rt, c, claim) = claimed_query::<F>();
    let mut out = [0u8; 8];
    assert_eq!(rt.push_claim(claim, REPLY_STREAM, &[]), Ok(0));
    assert_eq!(
        rt.pull(c, REPLY_STREAM, &mut out),
        Ok(Pull::Chunk(0)),
        "an empty chunk is pulled as a chunk of length 0, not as the end"
    );
    assert_eq!(rt.pull(c, REPLY_STREAM, &mut out), Ok(Pull::Pending));

    let taken = fill_reply_window(&mut rt, claim).unwrap_or(MAX_PUSHES);
    assert_eq!(
        rt.push_claim(claim, REPLY_STREAM, &[]),
        Ok(0),
        "an empty chunk is taken whatever the window"
    );
    for _ in 0..taken {
        assert_eq!(rt.pull(c, REPLY_STREAM, &mut out), Ok(Pull::Chunk(1)));
    }
    assert_eq!(
        rt.pull(c, REPLY_STREAM, &mut out),
        Ok(Pull::Chunk(0)),
        "the empty chunk pushed into the full window was taken"
    );
    assert_eq!(rt.pull(c, REPLY_STREAM, &mut out), Ok(Pull::Pending));
}

/// A pull into a buffer shorter than the next chunk reports the length it
/// needs and does not consume the chunk: a pull into a buffer long enough
/// reads it.
pub fn a_short_buffer_reports_needed_and_does_not_consume<F>()
where
    F: Factory,
    F::Runtime: StreamCaller + StreamHandler + Wakeable,
{
    let (mut rt, c, claim) = claimed_query::<F>();
    assert_eq!(rt.push_claim(claim, REPLY_STREAM, &[3, 3, 3]), Ok(3));
    let mut short = [0u8; 2];
    assert_eq!(
        rt.pull(c, REPLY_STREAM, &mut short),
        Err(ReadError::Short { needed: 3 })
    );
    let mut out = [0u8; 8];
    assert_eq!(
        rt.pull(c, REPLY_STREAM, &mut out),
        Ok(Pull::Chunk(3)),
        "the short pull did not consume the chunk"
    );
    assert_eq!(&out[..3], &[3, 3, 3]);
}

/// `end_claim` on the reply stream settles the claim `Ok` with no reply
/// bytes: `reply` reports it as a reply of length 0, the stream reads `End`,
/// and a `settle` after it finds no claim.
pub fn end_claim_on_the_reply_stream_settles_the_claim<F>()
where
    F: Factory,
    F::Runtime: StreamCaller + StreamHandler + Wakeable,
{
    let (mut rt, c, claim) = claimed_query::<F>();
    rt.end_claim(claim, REPLY_STREAM).expect("end_claim");
    let mut out = [0u8; 8];
    assert_eq!(
        rt.reply(c, &mut out),
        Ok(Some(Ok(0))),
        "the end of the reply stream is the call's settlement"
    );
    assert_eq!(rt.pull(c, REPLY_STREAM, &mut out), Ok(Pull::End));
    assert_eq!(
        rt.settle(claim, Ok(&[])),
        Err(SettleError::UnknownClaim),
        "the claim was settled by the end, so no settle follows"
    );
}

/// A settlement with an error ends the reply stream with that error, after
/// the chunks already pushed. The first chunk is pulled before the second is
/// pushed, so a window of one chunk is enough.
pub fn a_settle_err_after_chunks_fails_the_stream_after_them<F>()
where
    F: Factory,
    F::Runtime: StreamCaller + StreamHandler + Wakeable,
{
    let (mut rt, c, claim) = claimed_query::<F>();
    let mut out = [0u8; 8];
    assert_eq!(rt.push_claim(claim, REPLY_STREAM, &[1]), Ok(1));
    assert_eq!(rt.pull(c, REPLY_STREAM, &mut out), Ok(Pull::Chunk(1)));
    assert_eq!(&out[..1], &[1]);
    assert_eq!(rt.push_claim(claim, REPLY_STREAM, &[2, 2]), Ok(2));
    let error = CallError::Contract(Contract::PreconditionFailed);
    rt.settle(claim, Err(error)).expect("settle");

    assert_eq!(rt.pull(c, REPLY_STREAM, &mut out), Ok(Pull::Chunk(2)));
    assert_eq!(&out[..2], &[2, 2]);
    assert_eq!(
        rt.pull(c, REPLY_STREAM, &mut out),
        Ok(Pull::Failed(error)),
        "the settlement's error follows the chunks pushed before it"
    );
}

/// A push into a full window answers 0 and takes nothing; one pull frees
/// the window, and the same push is then taken. A runtime whose window does
/// not fill in 1,024 pushes checks only that every push was taken.
pub fn a_push_into_a_full_window_answers_zero_and_a_pull_frees_it<F>()
where
    F: Factory,
    F::Runtime: StreamCaller + StreamHandler + Wakeable,
{
    let (mut rt, c, claim) = claimed_query::<F>();
    let Some(taken) = fill_reply_window(&mut rt, claim) else {
        return;
    };
    assert!(taken > 0, "a window holds at least one chunk");
    let mut out = [0u8; 8];
    assert_eq!(rt.pull(c, REPLY_STREAM, &mut out), Ok(Pull::Chunk(1)));
    assert_eq!(
        rt.push_claim(claim, REPLY_STREAM, &[1]),
        Ok(1),
        "the pull freed room for one chunk"
    );
    for _ in 0..taken {
        assert_eq!(
            rt.pull(c, REPLY_STREAM, &mut out),
            Ok(Pull::Chunk(1)),
            "the push that answered 0 took nothing"
        );
    }
    assert_eq!(rt.pull(c, REPLY_STREAM, &mut out), Ok(Pull::Pending));
}

/// A `Stream` waker on the reply stream is woken by a chunk, and a fresh one
/// by the end of the stream. A `ClaimStream` waker on the same stream,
/// registered by the producer while the window is full, is woken by the pull
/// that frees it.
pub fn a_stream_waker_is_woken_by_a_chunk_an_end_and_a_freed_window<F>()
where
    F: Factory,
    F::Runtime: StreamCaller + StreamHandler + Wakeable,
{
    let (mut rt, c, claim) = claimed_query::<F>();
    let mut out = [0u8; 8];

    let (chunk, chunk_waker) = task();
    rt.wake_on(Interest::Stream(c, REPLY_STREAM), &chunk_waker);
    assert_eq!(wakes(&chunk), 0, "nothing changed since the registration");
    assert_eq!(rt.push_claim(claim, REPLY_STREAM, &[1]), Ok(1));
    assert_eq!(wakes(&chunk), 1, "a chunk wakes the receiver's waker");
    assert_eq!(rt.pull(c, REPLY_STREAM, &mut out), Ok(Pull::Chunk(1)));

    if let Some(taken) = fill_reply_window(&mut rt, claim) {
        let (window, window_waker) = task();
        rt.wake_on(Interest::ClaimStream(claim, REPLY_STREAM), &window_waker);
        assert_eq!(wakes(&window), 0, "nothing changed since the registration");
        assert_eq!(rt.pull(c, REPLY_STREAM, &mut out), Ok(Pull::Chunk(1)));
        assert_eq!(
            wakes(&window),
            1,
            "the pull that frees a full window wakes the producer's waker"
        );
        for _ in 1..taken {
            assert_eq!(rt.pull(c, REPLY_STREAM, &mut out), Ok(Pull::Chunk(1)));
        }
    }

    let (end, end_waker) = task();
    rt.wake_on(Interest::Stream(c, REPLY_STREAM), &end_waker);
    assert_eq!(wakes(&end), 0, "nothing changed since the registration");
    rt.end_claim(claim, REPLY_STREAM).expect("end_claim");
    assert_eq!(wakes(&end), 1, "the end wakes the receiver's waker");
}

/// The wakers of a parameter stream: a `ClaimStream` waker on the provider's
/// side is woken by a chunk the caller pushes, and a `Stream` waker of the
/// caller, registered while the parameter window is full, is woken by the
/// provider's pull that frees it. A runtime whose window does not fill in
/// 1,024 pushes checks only the first.
pub fn a_parameter_streams_wakers_are_woken_by_a_chunk_and_a_freed_window<F>()
where
    F: Factory,
    F::Runtime: StreamCaller + StreamHandler + Wakeable,
{
    let (mut rt, c, claim) = claimed_query::<F>();
    let mut out = [0u8; 8];

    let (chunk, chunk_waker) = task();
    rt.wake_on(Interest::ClaimStream(claim, PARAM_1), &chunk_waker);
    assert_eq!(wakes(&chunk), 0, "nothing changed since the registration");
    assert_eq!(rt.push(c, PARAM_1, &[1]), Ok(1));
    assert_eq!(wakes(&chunk), 1, "a chunk wakes the provider's waker");
    assert_eq!(rt.pull_claim(claim, PARAM_1, &mut out), Ok(Pull::Chunk(1)));

    let Some(taken) = fill_parameter_window(&mut rt, c) else {
        return;
    };
    let (window, window_waker) = task();
    rt.wake_on(Interest::Stream(c, PARAM_1), &window_waker);
    assert_eq!(wakes(&window), 0, "nothing changed since the registration");
    assert_eq!(rt.pull_claim(claim, PARAM_1, &mut out), Ok(Pull::Chunk(1)));
    assert_eq!(
        wakes(&window),
        1,
        "the pull that frees a full window wakes the caller's waker"
    );
    for _ in 1..taken {
        assert_eq!(rt.pull_claim(claim, PARAM_1, &mut out), Ok(Pull::Chunk(1)));
    }
}

/// A `Stream` waker of a call is woken when the call is settled and when the
/// call is forgotten, so no task waits on a stream that can no longer change.
/// A `ClaimStream` waker of the provider is woken when the caller forgets
/// the call. A runtime passes this only if it takes and wakes the stream
/// wakers of a call at each settlement and each forget.
///
/// The settlement is an error settlement of a query, which ends its reply
/// stream. The forget is of a query that is claimed and not settled, so the
/// call is in flight when it is forgotten.
pub fn a_settlement_and_a_forget_wake_the_stream_wakers<F>()
where
    F: Factory,
    F::Runtime: StreamCaller + StreamHandler + Wakeable,
{
    // The settlement path.
    let (mut rt, c, claim) = claimed_query::<F>();
    let (settled, settled_waker) = task();
    rt.wake_on(Interest::Stream(c, REPLY_STREAM), &settled_waker);
    assert_eq!(wakes(&settled), 0, "nothing changed since the registration");
    rt.settle(
        claim,
        Err(CallError::Contract(Contract::PreconditionFailed)),
    )
    .expect("settle");
    assert_eq!(
        wakes(&settled),
        1,
        "the settlement wakes the caller's stream waker"
    );
    rt.forget(c);

    // The forget path, on a call in flight.
    let c = rt.query(IFACE, ORD, &[2], None).expect("query sent");
    let claim = claim_next(&mut rt);
    let (reply, reply_waker) = task();
    let (param, param_waker) = task();
    let (provider, provider_waker) = task();
    rt.wake_on(Interest::Stream(c, REPLY_STREAM), &reply_waker);
    rt.wake_on(Interest::Stream(c, PARAM_1), &param_waker);
    rt.wake_on(Interest::ClaimStream(claim, PARAM_1), &provider_waker);
    // The parameter window is empty, so the producer's key already holds and
    // a runtime may wake that waker at once: each count is measured from
    // what it is before the forget.
    let before = (wakes(&reply), wakes(&param), wakes(&provider));
    rt.forget(c);
    assert!(
        wakes(&reply) > before.0,
        "the forget wakes the caller's waker on the reply stream"
    );
    assert!(
        wakes(&param) > before.1,
        "the forget wakes the caller's waker on a parameter stream"
    );
    assert!(
        wakes(&provider) > before.2,
        "the forget wakes the provider's waker on the claim's stream"
    );
}

/// After the caller forgets a claimed call, the provider's pull answers
/// `Failed(Transport(Interrupted))` and its push fails with the same outcome.
/// Its `end_claim` on the reply stream fails the same way and still settles
/// the claim, so a `settle` after it finds no claim.
pub fn forget_fails_the_providers_pull_and_push_with_interrupted<F>()
where
    F: Factory,
    F::Runtime: StreamCaller + StreamHandler + Wakeable,
{
    let (mut rt, c, claim) = claimed_query::<F>();
    rt.forget(c);
    let interrupted = CallError::Transport(Transport::Interrupted);
    let mut out = [0u8; 8];
    assert_eq!(
        rt.pull_claim(claim, PARAM_1, &mut out),
        Ok(Pull::Failed(interrupted))
    );
    assert_eq!(
        rt.push_claim(claim, REPLY_STREAM, &[1]),
        Err(PushError::Failed(interrupted))
    );
    assert_eq!(
        rt.end_claim(claim, REPLY_STREAM),
        Err(PushError::Failed(interrupted)),
        "the end of the reply stream after the forget is interrupted"
    );
    assert_eq!(
        rt.settle(claim, Ok(&[])),
        Err(SettleError::UnknownClaim),
        "the end settled the claim all the same"
    );
}

/// A producer's abort reaches the receiver after the chunks pushed before
/// it: the receiver pulls them, then `Failed` with the abort's outcome.
pub fn an_abort_by_the_producer_fails_the_receivers_pull_after_the_chunks_before_it<F>()
where
    F: Factory,
    F::Runtime: StreamCaller + StreamHandler + Wakeable,
{
    let (mut rt, c, claim) = claimed_query::<F>();
    let interrupted = CallError::Transport(Transport::Interrupted);
    assert_eq!(rt.push_claim(claim, REPLY_STREAM, &[1]), Ok(1));
    rt.abort_claim(claim, REPLY_STREAM, interrupted)
        .expect("abort_claim");
    let mut out = [0u8; 8];
    assert_eq!(rt.pull(c, REPLY_STREAM, &mut out), Ok(Pull::Chunk(1)));
    assert_eq!(&out[..1], &[1]);
    assert_eq!(
        rt.pull(c, REPLY_STREAM, &mut out),
        Ok(Pull::Failed(interrupted))
    );
}

/// After the producer ends a stream, an abort and a push answer `Ended`, and
/// the receiver still reads the chunks and then `End`: a late failure does
/// not turn a complete transfer into an interrupted one.
pub fn an_abort_or_a_push_after_the_end_is_ended_and_the_end_stands<F>()
where
    F: Factory,
    F::Runtime: StreamCaller + StreamHandler + Wakeable,
{
    let (mut rt, c, claim) = claimed_query::<F>();
    assert_eq!(rt.push_claim(claim, REPLY_STREAM, &[1]), Ok(1));
    rt.end_claim(claim, REPLY_STREAM).expect("end_claim");
    assert_eq!(
        rt.abort_claim(
            claim,
            REPLY_STREAM,
            CallError::Transport(Transport::Interrupted)
        ),
        Err(PushError::Ended)
    );
    assert_eq!(
        rt.push_claim(claim, REPLY_STREAM, &[2, 2]),
        Err(PushError::Ended)
    );
    let mut out = [0u8; 8];
    assert_eq!(rt.pull(c, REPLY_STREAM, &mut out), Ok(Pull::Chunk(1)));
    assert_eq!(
        rt.pull(c, REPLY_STREAM, &mut out),
        Ok(Pull::End),
        "the end stands"
    );
}

/// A command acknowledged at its claim, whose parameter stream the caller
/// pushes, and whose provider refuses the first chunk with `abort_claim`:
/// the runtime, the command's correlation and the refusal's outcome.
fn refused_parameter_stream<F>() -> (F::Runtime, Correlation, CallError)
where
    F: Factory,
    F::Runtime: StreamCaller + StreamHandler + Wakeable,
{
    let mut rt = runtime::<F>();
    rt.serve(IFACE, &[ORD]).expect("serve");
    let c = rt.command(IFACE, ORD, &[1], None).expect("command sent");
    let claim = claim_next(&mut rt);
    rt.settle(claim, Ok(&[])).expect("the acknowledgment");
    assert_eq!(rt.push(c, PARAM_1, &[1]), Ok(1));
    let refusal = CallError::Contract(Contract::InvalidValue(Violation {
        type_name: None,
        rule: Rule::Range,
    }));
    rt.abort_claim(claim, PARAM_1, refusal)
        .expect("the provider refuses the chunk");
    (rt, c, refusal)
}

/// The provider's refusal of a parameter stream reaches the producer through
/// its next push, as `PushError::Failed` with the refusal's outcome.
pub fn a_receivers_abort_of_a_parameter_stream_fails_the_producers_next_push<F>()
where
    F: Factory,
    F::Runtime: StreamCaller + StreamHandler + Wakeable,
{
    let (mut rt, c, refusal) = refused_parameter_stream::<F>();
    assert_eq!(
        rt.push(c, PARAM_1, &[2, 2]),
        Err(PushError::Failed(refusal))
    );
}

/// A refusal of a parameter stream after the command's acknowledgment does
/// not change the acknowledgment.
pub fn a_refusal_after_the_acknowledgment_leaves_the_acknowledgment<F>()
where
    F: Factory,
    F::Runtime: StreamCaller + StreamHandler + Wakeable,
{
    let (mut rt, c, refusal) = refused_parameter_stream::<F>();
    assert_eq!(
        rt.push(c, PARAM_1, &[2, 2]),
        Err(PushError::Failed(refusal))
    );
    assert_eq!(
        rt.ack(c),
        Some(Ok(())),
        "the acknowledgment stands after the refusal"
    );
}

/// A chunk of a parameter stream pushed before the provider takes the claim
/// is kept, and the provider pulls it after the claim.
pub fn a_parameter_stream_pushed_before_the_claim_is_pulled_after_it<F>()
where
    F: Factory,
    F::Runtime: StreamCaller + StreamHandler + Wakeable,
{
    let mut rt = runtime::<F>();
    rt.serve(IFACE, &[ORD]).expect("serve");
    let c = rt.query(IFACE, ORD, &[1], None).expect("query sent");
    assert_eq!(rt.push(c, PARAM_1, &[1]), Ok(1));
    let claim = claim_next(&mut rt);
    let mut out = [0u8; 8];
    assert_eq!(rt.pull_claim(claim, PARAM_1, &mut out), Ok(Pull::Chunk(1)));
    assert_eq!(&out[..1], &[1]);
}

/// The caller ends a parameter stream with `end`; a second `end` answers
/// `Ended`, and the provider pulls `End`.
pub fn a_parameter_stream_ends_explicitly_and_a_second_end_is_ended<F>()
where
    F: Factory,
    F::Runtime: StreamCaller + StreamHandler + Wakeable,
{
    let (mut rt, c, claim) = claimed_query::<F>();
    assert_eq!(rt.end(c, PARAM_1), Ok(()));
    assert_eq!(rt.end(c, PARAM_1), Err(PushError::Ended));
    let mut out = [0u8; 8];
    assert_eq!(rt.pull_claim(claim, PARAM_1, &mut out), Ok(Pull::End));
}

/// A chunk longer than the member's limit is refused with the limit, and
/// nothing is pushed, on the reply stream and on a parameter stream. The suite attaches to a catalog with no descriptor, so
/// the limit of the query is the default ceiling of its kind (ADR-0030
/// decision 16); a chunk of exactly that length is taken.
pub fn a_chunk_longer_than_the_limit_is_too_large_and_not_sent<F>()
where
    F: Factory,
    F::Runtime: StreamCaller + StreamHandler + Wakeable,
{
    let cap = default_ceiling(Kind::Query);
    let (mut rt, c, claim) = claimed_query::<F>();
    assert_eq!(
        rt.push_claim(claim, REPLY_STREAM, &vec![0u8; cap + 1]),
        Err(PushError::TooLarge { cap }),
        "one byte above the limit is refused"
    );
    let mut out = vec![0u8; cap + 1];
    assert_eq!(
        rt.pull(c, REPLY_STREAM, &mut out),
        Ok(Pull::Pending),
        "the refused chunk was not pushed"
    );
    assert_eq!(
        rt.push_claim(claim, REPLY_STREAM, &vec![7u8; cap]),
        Ok(cap),
        "a chunk of exactly the limit is taken"
    );
    assert_eq!(rt.pull(c, REPLY_STREAM, &mut out), Ok(Pull::Chunk(cap)));

    // The limit bounds every chunk in both directions: the caller's chunk to
    // a parameter stream too.
    assert_eq!(
        rt.push(c, PARAM_1, &vec![0u8; cap + 1]),
        Err(PushError::TooLarge { cap }),
        "one byte above the limit is refused on a parameter stream"
    );
    assert_eq!(
        rt.pull_claim(claim, PARAM_1, &mut out),
        Ok(Pull::Pending),
        "the refused parameter chunk was not pushed"
    );
    assert_eq!(
        rt.push(c, PARAM_1, &vec![7u8; cap]),
        Ok(cap),
        "a parameter chunk of exactly the limit is taken"
    );
    assert_eq!(
        rt.pull_claim(claim, PARAM_1, &mut out),
        Ok(Pull::Chunk(cap))
    );
}

/// Two stream parameters of one call are two streams: each holds its own
/// chunks, and pulling one does not read the other's.
pub fn two_stream_parameters_are_two_streams<F>()
where
    F: Factory,
    F::Runtime: StreamCaller + StreamHandler + Wakeable,
{
    let (mut rt, c, claim) = claimed_query::<F>();
    assert_eq!(rt.push(c, PARAM_1, &[1]), Ok(1));
    assert_eq!(rt.push(c, PARAM_2, &[2, 2]), Ok(2));
    let mut out = [0u8; 8];
    assert_eq!(rt.pull_claim(claim, PARAM_2, &mut out), Ok(Pull::Chunk(2)));
    assert_eq!(&out[..2], &[2, 2]);
    assert_eq!(rt.pull_claim(claim, PARAM_1, &mut out), Ok(Pull::Chunk(1)));
    assert_eq!(&out[..1], &[1]);
    assert_eq!(rt.pull_claim(claim, PARAM_1, &mut out), Ok(Pull::Pending));
    assert_eq!(rt.pull_claim(claim, PARAM_2, &mut out), Ok(Pull::Pending));
}

/// A call with a parameter stream and a reply stream carries both at once:
/// the provider pulls the caller's chunk, and the caller pulls the
/// provider's.
pub fn a_bidirectional_call_carries_both_streams<F>()
where
    F: Factory,
    F::Runtime: StreamCaller + StreamHandler + Wakeable,
{
    let (mut rt, c, claim) = claimed_query::<F>();
    assert_eq!(rt.push(c, PARAM_1, &[1]), Ok(1));
    assert_eq!(rt.push_claim(claim, REPLY_STREAM, &[2, 2]), Ok(2));
    let mut out = [0u8; 8];
    assert_eq!(rt.pull_claim(claim, PARAM_1, &mut out), Ok(Pull::Chunk(1)));
    assert_eq!(&out[..1], &[1]);
    assert_eq!(rt.pull(c, REPLY_STREAM, &mut out), Ok(Pull::Chunk(2)));
    assert_eq!(&out[..2], &[2, 2]);
    assert_eq!(rt.pull_claim(claim, PARAM_1, &mut out), Ok(Pull::Pending));
    assert_eq!(rt.pull(c, REPLY_STREAM, &mut out), Ok(Pull::Pending));
}

/// A slot reclaimed and taken by a new call does not answer to the
/// correlation it had before: a pull or a push on the old correlation is an
/// unknown interaction, and does not read or write the new call's streams.
///
/// The old call is a query the test claims itself, so it holds the claim:
/// after the forget the provider pulls the old call's parameter stream to its
/// end, the chunk and then `Failed(Interrupted)`, because the slot of a
/// forgotten call is not reclaimed while the provider has a stream to read,
/// and then settles the claim. Every other slot holds a settled command, so
/// the new call can only take the slot that was reclaimed.
pub fn a_reclaimed_slots_stream_answers_unknown<F>()
where
    F: Factory,
    F::Runtime: StreamCaller + StreamHandler + Wakeable,
{
    let (mut rt, old, old_claim) = claimed_query::<F>();
    for _ in 1..F::SLOTS {
        rt.command(IFACE, ORD, &[1], None).expect("a slot is free");
        let claim = claim_next(&mut rt);
        rt.settle(claim, Ok(&[])).expect("settle");
    }
    // A chunk on the old call's parameter stream, which the call that takes
    // the slot must not inherit.
    assert_eq!(rt.push(old, PARAM_1, &[9]), Ok(1));
    rt.forget(old);
    let mut out = [0u8; 8];
    assert_eq!(
        rt.pull_claim(old_claim, PARAM_1, &mut out),
        Ok(Pull::Chunk(1)),
        "the chunk pushed before the forget is still read"
    );
    assert_eq!(
        rt.pull_claim(old_claim, PARAM_1, &mut out),
        Ok(Pull::Failed(CallError::Transport(Transport::Interrupted))),
        "the stream was not ended, so the forget interrupted it"
    );
    rt.settle(old_claim, Ok(&[]))
        .expect("a claim is settled after its call is forgotten");
    let new = rt
        .query(IFACE, ORD, &[2], None)
        .expect("the reclaimed slot");
    assert_ne!(new, old, "the slot is taken under a new correlation");
    let claim = claim_next(&mut rt);
    assert_eq!(rt.push_claim(claim, REPLY_STREAM, &[1]), Ok(1));

    let unknown = Contract::UnknownInteraction;
    assert_eq!(
        rt.pull(old, REPLY_STREAM, &mut out),
        Err(ReadError::Contract(unknown)),
        "the old correlation does not read the new call's stream"
    );
    assert_eq!(
        rt.push(old, PARAM_1, &[3]),
        Err(PushError::Contract(unknown)),
        "the old correlation does not push to the new call's stream"
    );
    assert_eq!(rt.pull(new, REPLY_STREAM, &mut out), Ok(Pull::Chunk(1)));
    assert_eq!(&out[..1], &[1]);
    assert_eq!(
        rt.pull_claim(claim, PARAM_1, &mut out),
        Ok(Pull::Pending),
        "the new call holds neither the old call's chunk nor the push on the \
         old correlation"
    );
}

/// A forget after a command's acknowledgment keeps a stream its producer
/// ended, which the provider still reads to its end, and interrupts a stream
/// that was not ended, which the provider reads to `Failed(Interrupted)`
/// after the chunk pushed before the forget. A command has no reply stream,
/// so the provider's push to the reply stream after a forget is checked on a
/// query, by `forget_fails_the_providers_pull_and_push_with_interrupted`.
pub fn a_forget_after_a_commands_acknowledgment_keeps_an_ended_stream_and_interrupts_an_open_one<
    F,
>()
where
    F: Factory,
    F::Runtime: StreamCaller + StreamHandler + Wakeable,
{
    let mut rt = runtime::<F>();
    rt.serve(IFACE, &[ORD]).expect("serve");
    let c = rt.command(IFACE, ORD, &[1], None).expect("command sent");
    let claim = claim_next(&mut rt);
    rt.settle(claim, Ok(&[])).expect("the acknowledgment");
    assert_eq!(rt.push(c, PARAM_1, &[7]), Ok(1));
    assert_eq!(rt.end(c, PARAM_1), Ok(()));
    assert_eq!(rt.push(c, PARAM_2, &[8]), Ok(1));
    rt.forget(c);

    let interrupted = CallError::Transport(Transport::Interrupted);
    let mut out = [0u8; 8];
    assert_eq!(rt.pull_claim(claim, PARAM_1, &mut out), Ok(Pull::Chunk(1)));
    assert_eq!(&out[..1], &[7]);
    assert_eq!(
        rt.pull_claim(claim, PARAM_1, &mut out),
        Ok(Pull::End),
        "a stream its producer ended is kept to its end"
    );
    assert_eq!(rt.pull_claim(claim, PARAM_2, &mut out), Ok(Pull::Chunk(1)));
    assert_eq!(&out[..1], &[8]);
    assert_eq!(
        rt.pull_claim(claim, PARAM_2, &mut out),
        Ok(Pull::Failed(interrupted)),
        "a stream that was not ended is interrupted after its chunks"
    );
}

/// The caller's abort of a parameter stream it produces reaches the provider
/// after the chunk pushed before it: the provider pulls the chunk, then
/// `Failed` with the abort's outcome. After its own abort, the caller's push
/// and end answer `Ended`.
pub fn a_callers_abort_of_a_parameter_stream_fails_the_providers_pull_after_the_chunks_before_it<
    F,
>()
where
    F: Factory,
    F::Runtime: StreamCaller + StreamHandler + Wakeable,
{
    let (mut rt, c, claim) = claimed_query::<F>();
    let interrupted = CallError::Transport(Transport::Interrupted);
    assert_eq!(rt.push(c, PARAM_1, &[1]), Ok(1));
    rt.abort(c, PARAM_1, interrupted).expect("abort");
    let mut out = [0u8; 8];
    assert_eq!(rt.pull_claim(claim, PARAM_1, &mut out), Ok(Pull::Chunk(1)));
    assert_eq!(&out[..1], &[1]);
    assert_eq!(
        rt.pull_claim(claim, PARAM_1, &mut out),
        Ok(Pull::Failed(interrupted))
    );
    assert_eq!(
        rt.push(c, PARAM_1, &[2]),
        Err(PushError::Ended),
        "a push after the producer's own abort is Ended"
    );
    assert_eq!(
        rt.end(c, PARAM_1),
        Err(PushError::Ended),
        "an end after the producer's own abort is Ended"
    );
}

/// The caller's refusal of a query's reply stream: the caller reads none of
/// what it refused, the provider's next push and its `end_claim` answer
/// `Failed` with the refusal, `end_claim` settles the claim with the
/// refusal, and a second abort by the caller answers `Ended`.
pub fn a_callers_refusal_of_the_reply_stream_fails_the_providers_push_and_end<F>()
where
    F: Factory,
    F::Runtime: StreamCaller + StreamHandler + Wakeable,
{
    let (mut rt, c, claim) = claimed_query::<F>();
    let refusal = CallError::Contract(Contract::PreconditionFailed);
    assert_eq!(rt.push_claim(claim, REPLY_STREAM, &[1]), Ok(1));
    rt.abort(c, REPLY_STREAM, refusal)
        .expect("the caller refuses");
    let mut out = [0u8; 8];
    assert_eq!(
        rt.pull(c, REPLY_STREAM, &mut out),
        Ok(Pull::Failed(refusal)),
        "the refusing side reads none of what it refused"
    );
    assert_eq!(
        rt.push_claim(claim, REPLY_STREAM, &[2]),
        Err(PushError::Failed(refusal))
    );
    assert_eq!(
        rt.end_claim(claim, REPLY_STREAM),
        Err(PushError::Failed(refusal))
    );
    assert_eq!(
        rt.reply(c, &mut out),
        Ok(Some(Err(refusal))),
        "the end after the refusal settles the claim with the refusal"
    );
    assert_eq!(
        rt.abort(c, REPLY_STREAM, refusal),
        Err(PushError::Ended),
        "an abort of a stream already stopped is Ended"
    );
}

/// After the provider refused a parameter stream, the caller's end answers
/// `Failed` with the refusal, and its abort answers `Ended`.
pub fn after_the_receivers_refusal_the_producers_end_fails_and_its_abort_is_ended<F>()
where
    F: Factory,
    F::Runtime: StreamCaller + StreamHandler + Wakeable,
{
    let (mut rt, c, refusal) = refused_parameter_stream::<F>();
    assert_eq!(rt.end(c, PARAM_1), Err(PushError::Failed(refusal)));
    assert_eq!(
        rt.abort(c, PARAM_1, CallError::Transport(Transport::Interrupted)),
        Err(PushError::Ended)
    );
}

/// The provider's abort of a stream parameter its producer already ended
/// answers `Ended`, and the provider is done with the stream all the same:
/// after the forget of the acknowledged command, its slot comes back. The
/// abort comes before the forget in one round and after it in the next, so
/// a call whose slot stayed taken makes a later send answer `Busy`.
pub fn a_providers_abort_of_an_ended_parameter_stream_lets_the_forget_free_the_slot<F>()
where
    F: Factory,
    F::Runtime: StreamCaller + StreamHandler + Wakeable,
{
    let mut rt = runtime::<F>();
    rt.serve(IFACE, &[ORD]).expect("serve");
    let refusal = CallError::Contract(Contract::PreconditionFailed);
    let mut out = [0u8; 8];
    for round in 0..F::SLOTS * 2 {
        let c = rt
            .command(IFACE, ORD, &[1], None)
            .unwrap_or_else(|e| panic!("round {round}: the send answered {e:?}"));
        let claim = claim_next(&mut rt);
        rt.settle(claim, Ok(&[])).expect("the acknowledgment");
        assert_eq!(rt.push(c, PARAM_1, &[1]), Ok(1));
        rt.end(c, PARAM_1).expect("end");
        if round % 2 == 0 {
            assert_eq!(
                rt.abort_claim(claim, PARAM_1, refusal),
                Err(PushError::Ended),
                "an abort of an ended stream is Ended"
            );
            rt.forget(c);
        } else {
            rt.forget(c);
            assert_eq!(rt.pull_claim(claim, PARAM_1, &mut out), Ok(Pull::Chunk(1)));
            assert_eq!(
                rt.abort_claim(claim, PARAM_1, refusal),
                Err(PushError::Ended),
                "an abort of an ended stream is Ended"
            );
        }
    }
}

/// The settlement of a query releases its stream parameters, with `Ok` and
/// with an error, and so does a command's settlement with an error, which
/// refuses the whole call: the caller's next push and end on a stream
/// parameter answer `Failed(Interrupted)`, and the call's outcome carries the
/// settlement.
pub fn a_querys_settlement_and_a_commands_refusal_release_the_parameter_streams<F>()
where
    F: Factory,
    F::Runtime: StreamCaller + StreamHandler + Wakeable,
{
    let interrupted = PushError::Failed(CallError::Transport(Transport::Interrupted));
    let error = CallError::Contract(Contract::PreconditionFailed);
    let mut out = [0u8; 8];

    for outcome in [Ok(&[][..]), Err(error)] {
        let (mut rt, c, claim) = claimed_query::<F>();
        assert_eq!(rt.push(c, PARAM_1, &[1]), Ok(1));
        rt.settle(claim, outcome).expect("settle");
        assert_eq!(rt.push(c, PARAM_1, &[2]), Err(interrupted), "{outcome:?}");
        assert_eq!(rt.end(c, PARAM_1), Err(interrupted), "{outcome:?}");
        assert_eq!(
            rt.reply(c, &mut out),
            Ok(Some(outcome.map(<[u8]>::len))),
            "the reply carries the settlement"
        );
    }

    let mut rt = runtime::<F>();
    rt.serve(IFACE, &[ORD]).expect("serve");
    let c = rt.command(IFACE, ORD, &[1], None).expect("command sent");
    assert_eq!(rt.push(c, PARAM_1, &[1]), Ok(1));
    let claim = claim_next(&mut rt);
    rt.settle(claim, Err(error)).expect("the refusal");
    assert_eq!(rt.push(c, PARAM_1, &[2]), Err(interrupted));
    assert_eq!(rt.end(c, PARAM_1), Err(interrupted));
    assert_eq!(rt.ack(c), Some(Err(error)), "the ack carries the refusal");
}

/// Each stream has one producer and one receiver: a pull by the producer,
/// and a push or an end by the receiver, are unknown interactions, on the
/// reply stream and on a parameter stream alike.
pub fn a_pull_by_the_producer_and_a_push_by_the_receiver_are_unknown<F>()
where
    F: Factory,
    F::Runtime: StreamCaller + StreamHandler + Wakeable,
{
    let (mut rt, c, claim) = claimed_query::<F>();
    let unknown = Contract::UnknownInteraction;
    let mut out = [0u8; 8];
    assert_eq!(
        rt.pull(c, PARAM_1, &mut out),
        Err(ReadError::Contract(unknown)),
        "the caller produces a parameter stream"
    );
    assert_eq!(
        rt.pull_claim(claim, REPLY_STREAM, &mut out),
        Err(ReadError::Contract(unknown)),
        "the provider produces the reply stream"
    );
    assert_eq!(
        rt.push(c, REPLY_STREAM, &[1]),
        Err(PushError::Contract(unknown))
    );
    assert_eq!(rt.end(c, REPLY_STREAM), Err(PushError::Contract(unknown)));
    assert_eq!(
        rt.push_claim(claim, PARAM_1, &[1]),
        Err(PushError::Contract(unknown))
    );
    assert_eq!(
        rt.end_claim(claim, PARAM_1),
        Err(PushError::Contract(unknown))
    );
}

/// A claim offered through `ReadError::ShortClaim` and not yet taken has no
/// streams: a pull, a push and an end on it are unknown interactions, and a
/// `ClaimStream` registration on it is woken at once. Once the provider takes
/// the claim with a large enough buffer, under the same id, it reads the
/// parameter stream from its first chunk: the refused operations consumed
/// nothing.
pub fn a_claim_only_offered_has_no_streams_until_it_is_taken<F>()
where
    F: Factory,
    F::Runtime: StreamCaller + StreamHandler + Wakeable,
{
    let mut rt = runtime::<F>();
    rt.serve(IFACE, &[ORD]).expect("serve");
    let c = rt.query(IFACE, ORD, &[1, 2, 3], None).expect("query sent");
    assert_eq!(rt.push(c, PARAM_1, &[1]), Ok(1));
    let mut short = [0u8; 1];
    let Err(ReadError::ShortClaim { claim, .. }) = rt.next_claim(&mut short) else {
        panic!("a buffer shorter than the arguments reports ShortClaim");
    };
    let unknown = Contract::UnknownInteraction;
    let mut out = [0u8; 8];
    assert_eq!(
        rt.pull_claim(claim, PARAM_1, &mut out),
        Err(ReadError::Contract(unknown)),
        "an offered claim has no stream to pull"
    );
    assert_eq!(
        rt.push_claim(claim, REPLY_STREAM, &[7]),
        Err(PushError::Contract(unknown)),
        "an offered claim has no stream to push to"
    );
    assert_eq!(
        rt.end_claim(claim, REPLY_STREAM),
        Err(PushError::Contract(unknown)),
        "an offered claim has no stream to end"
    );
    let (count, waker) = task();
    rt.wake_on(Interest::ClaimStream(claim, PARAM_1), &waker);
    assert!(
        wakes(&count) > 0,
        "a registration on a stream of an offered claim is woken at once"
    );
    let taken = claim_next(&mut rt);
    assert_eq!(taken, claim, "the call is presented under the same id");
    assert_eq!(
        rt.pull_claim(taken, PARAM_1, &mut out),
        Ok(Pull::Chunk(1)),
        "the provider reads the stream from its first chunk once it took the claim"
    );
    assert_eq!(&out[..1], &[1]);
}

/// The settlement of a query drops the unread chunks of its stream
/// parameters: the provider's pull after the settlement answers `End` for a
/// stream the caller ended, and `Failed(Interrupted)` for one it did not,
/// never the chunk.
pub fn a_querys_settlement_drops_the_unread_chunks_of_its_parameter_streams<F>()
where
    F: Factory,
    F::Runtime: StreamCaller + StreamHandler + Wakeable,
{
    let interrupted = CallError::Transport(Transport::Interrupted);
    for ended in [false, true] {
        let (mut rt, c, claim) = claimed_query::<F>();
        assert_eq!(rt.push(c, PARAM_1, &[1]), Ok(1));
        if ended {
            rt.end(c, PARAM_1).expect("end");
        }
        rt.settle(claim, Ok(&[])).expect("settle");
        let expected = if ended {
            Pull::End
        } else {
            Pull::Failed(interrupted)
        };
        let mut out = [0u8; 8];
        assert_eq!(
            rt.pull_claim(claim, PARAM_1, &mut out),
            Ok(expected),
            "ended {ended}: the unread chunk was dropped at the settlement"
        );
    }
}

/// A settlement with an error drops the unread chunks of the stream
/// parameters, for a query and for a command, which the error refuses as a
/// whole: the provider's pull after the settlement answers `End` for a stream
/// the caller ended, and `Failed(Interrupted)` for one it did not, never the
/// chunk.
pub fn a_settlement_with_an_error_drops_the_unread_chunks_of_the_parameter_streams<F>()
where
    F: Factory,
    F::Runtime: StreamCaller + StreamHandler + Wakeable,
{
    let interrupted = CallError::Transport(Transport::Interrupted);
    let error = CallError::Contract(Contract::PreconditionFailed);
    for command in [false, true] {
        for ended in [false, true] {
            let mut rt = runtime::<F>();
            rt.serve(IFACE, &[ORD]).expect("serve");
            let c = if command {
                rt.command(IFACE, ORD, &[1], None).expect("command sent")
            } else {
                rt.query(IFACE, ORD, &[1], None).expect("query sent")
            };
            let claim = claim_next(&mut rt);
            assert_eq!(rt.push(c, PARAM_1, &[1]), Ok(1));
            if ended {
                rt.end(c, PARAM_1).expect("end");
            }
            rt.settle(claim, Err(error)).expect("the refusal");
            let expected = if ended {
                Pull::End
            } else {
                Pull::Failed(interrupted)
            };
            let mut out = [0u8; 8];
            assert_eq!(
                rt.pull_claim(claim, PARAM_1, &mut out),
                Ok(expected),
                "command {command}, ended {ended}: the unread chunk was dropped at the settlement"
            );
        }
    }
}

/// `settle(Ok(&[]))` after the caller refused the reply stream records `Ok`
/// and leaves the refusal on the stream: the caller's pull of the reply
/// stream answers `Failed` with its own refusal, not `End`.
pub fn a_settle_ok_after_the_callers_refusal_leaves_the_refusal_on_the_reply_stream<F>()
where
    F: Factory,
    F::Runtime: StreamCaller + StreamHandler + Wakeable,
{
    let (mut rt, c, claim) = claimed_query::<F>();
    let refusal = CallError::Contract(Contract::PreconditionFailed);
    rt.abort(c, REPLY_STREAM, refusal)
        .expect("the caller refuses");
    rt.settle(claim, Ok(&[])).expect("settle");
    let mut out = [0u8; 8];
    assert_eq!(
        rt.pull(c, REPLY_STREAM, &mut out),
        Ok(Pull::Failed(refusal)),
        "the settlement does not turn the refused stream into an ended one"
    );
    assert_eq!(
        rt.reply(c, &mut out),
        Ok(Some(Ok(0))),
        "settle records Ok, unlike end_claim"
    );
}

/// A receiver's abort of a stream that already ended answers `Ended` and
/// drops the receiver's unread chunks: its later pulls answer `End`. The
/// provider aborts a stream parameter the caller ended, and the caller
/// aborts the reply stream the provider ended.
pub fn a_receivers_abort_of_an_ended_stream_drops_its_unread_chunks<F>()
where
    F: Factory,
    F::Runtime: StreamCaller + StreamHandler + Wakeable,
{
    let (mut rt, c, claim) = claimed_query::<F>();
    let refusal = CallError::Contract(Contract::PreconditionFailed);
    let mut out = [0u8; 8];
    assert_eq!(rt.push(c, PARAM_1, &[1]), Ok(1));
    rt.end(c, PARAM_1).expect("end");
    assert_eq!(
        rt.abort_claim(claim, PARAM_1, refusal),
        Err(PushError::Ended)
    );
    assert_eq!(
        rt.pull_claim(claim, PARAM_1, &mut out),
        Ok(Pull::End),
        "the provider's abort dropped the unread chunk of the ended stream"
    );
    assert_eq!(rt.push_claim(claim, REPLY_STREAM, &[1]), Ok(1));
    rt.end_claim(claim, REPLY_STREAM).expect("end_claim");
    assert_eq!(rt.abort(c, REPLY_STREAM, refusal), Err(PushError::Ended));
    assert_eq!(
        rt.pull(c, REPLY_STREAM, &mut out),
        Ok(Pull::End),
        "the caller's abort dropped the unread chunk of the ended stream"
    );
}
