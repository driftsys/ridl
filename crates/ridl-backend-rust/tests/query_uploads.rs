//! Upload queries over the generated face and loopback ports.
#[allow(dead_code, unused_imports)]
mod generated {
    include!("generated/stream_face.rs");
}
#[allow(dead_code)]
mod support;
use ridl_loopback::Loopback;
use ridl_rt::{
    contract::{CatalogRef, Interface, StreamNo},
    encoding::{FlatBuffers, Proto3},
    error::{CallError, ClientError, Contract, Transport},
    face::{Bind, Consumer, Interrupted, Source},
    payload::{Payload, Ref},
    port::{Correlation, SendError, StreamCaller},
    sample::Duration,
};
use std::{
    cell::RefCell,
    collections::VecDeque,
    future::Future,
    pin::Pin,
    rc::Rc,
    task::{Context, Poll, Waker},
};
use support::doubles::{self, Op, RecordingPorts};
const CATALOG: CatalogRef = *<generated::Log as Interface>::CATALOG;
#[derive(Default)]
struct State {
    accepted: Vec<generated::Reading>,
    queued: VecDeque<generated::Reading>,
    ends: usize,
    failures: Vec<CallError>,
    replies: usize,
    finished: bool,
    starts: usize,
    blocked: bool,
    reply_blocked: bool,
    ready_polls: usize,
    source_polls: usize,
    drops: usize,
    callbacks: Vec<&'static str>,
    waker: Option<Waker>,
}
#[derive(Clone)]
struct Upload(Rc<RefCell<State>>);
impl Consumer for Upload {
    type Item = generated::Reading;
    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<()> {
        self.0.borrow_mut().ready_polls += 1;
        if self.0.borrow().blocked {
            Poll::Pending
        } else {
            Poll::Ready(())
        }
    }
    fn accept(&mut self, item: Self::Item) {
        let mut s = self.0.borrow_mut();
        s.callbacks.push("accept");
        s.accepted.push(item);
        s.queued.push_back(item);
        if let Some(w) = s.waker.take() {
            w.wake();
        }
    }
    fn end(&mut self) {
        let mut s = self.0.borrow_mut();
        s.callbacks.push("end");
        s.ends += 1;
        s.finished = true;
        if let Some(w) = s.waker.take() {
            w.wake();
        }
    }
    fn failed(&mut self, e: CallError) {
        let mut s = self.0.borrow_mut();
        s.callbacks.push("failed");
        s.failures.push(e);
        s.finished = true;
    }
}
impl Source for Upload {
    type Item = generated::Reading;
    fn poll_next(&mut self, cx: &mut Context<'_>) -> Poll<Option<Result<Self::Item, Interrupted>>> {
        let mut s = self.0.borrow_mut();
        s.source_polls += 1;
        if s.reply_blocked {
            return Poll::Pending;
        }
        if let Some(item) = s.queued.pop_front() {
            Poll::Ready(Some(Ok(item)))
        } else if s.finished {
            Poll::Ready(None)
        } else {
            s.waker = Some(cx.waker().clone());
            Poll::Pending
        }
    }
}
impl Drop for Upload {
    fn drop(&mut self) {
        self.0.borrow_mut().drops += 1;
    }
}
struct Provider(Upload);
struct Empty;
impl Source for Empty {
    type Item = generated::Reading;
    fn poll_next(&mut self, _: &mut Context<'_>) -> Poll<Option<Result<Self::Item, Interrupted>>> {
        Poll::Ready(None)
    }
}
impl generated::log::Provider for Provider {
    type History = Empty;
    type Upload = Upload;
    type Summarize = Upload;
    type Pipe = Upload;
    type Checked = Upload;
    fn checked(&mut self, _: &generated::LogCheckedParams) -> Upload {
        self.0.clone()
    }
    fn checked_reply(&mut self, upload: Upload) -> generated::Level {
        let mut s = upload.0.borrow_mut();
        s.replies += 1;
        generated::Level::new_unchecked(60)
    }
    fn history(&mut self, _: &generated::Level) -> Empty {
        Empty
    }
    fn clear(&mut self, _: &generated::Level) {}
    fn upload(&mut self, _: &generated::LogUploadParams) -> Upload {
        self.0.clone()
    }
    fn summarize(&mut self, _: &generated::LogSummarizeParams) -> Upload {
        self.0.0.borrow_mut().starts += 1;
        self.0.clone()
    }
    fn summarize_reply(&mut self, upload: Upload) -> generated::Summary {
        let mut s = upload.0.borrow_mut();
        s.replies += 1;
        generated::Summary {
            count: s.accepted.len() as i64,
            total: s.accepted.iter().map(|r| r.level.get()).sum(),
        }
    }
    fn pipe(&mut self, _: &generated::LogPipeParams) -> Upload {
        self.0.0.borrow_mut().starts += 1;
        self.0.clone()
    }
}
fn setup() -> (Loopback, Rc<RefCell<State>>, Provider) {
    let s = Rc::new(RefCell::new(State::default()));
    (Loopback::new(CATALOG), s.clone(), Provider(Upload(s)))
}
fn poll<F: Future + Unpin>(f: &mut F) -> Poll<F::Output> {
    Pin::new(f).poll(&mut Context::from_waker(&ridl_rt::task::noop_waker()))
}
fn with_cx<R>(f: impl FnOnce(&mut Context<'_>) -> R) -> R {
    f(&mut Context::from_waker(&ridl_rt::task::noop_waker()))
}
fn pending<F: Future + Unpin>(f: &mut F) {
    assert!(poll(f).is_pending());
}
fn reading(n: i64) -> generated::Reading {
    generated::Reading {
        level: generated::Level::new_unchecked(n),
        flag: false,
    }
}
fn correlation(ops: &doubles::Log) -> Correlation {
    ops.borrow()
        .iter()
        .find_map(|op| {
            if let Op::Query(Ok(c)) = op {
                Some(*c)
            } else {
                None
            }
        })
        .unwrap()
}
#[test]
fn a_query_with_an_upload_replies_after_the_end() {
    let (rt, s, mut provider) = setup();
    let mut serve = generated::log::serve(rt.handler(), &mut provider);
    let mut client = generated::log::Client::new(rt.caller());
    let (mut call, mut sink) = client.summarize(generated::LogSummarizeParams {});
    pending(&mut call);
    pending(&mut serve);
    for n in 1..=3 {
        sink.send(&reading(n)).unwrap();
        pending(&mut serve);
    }
    pending(&mut call);
    assert_eq!(s.borrow().replies, 0);
    assert_eq!(s.borrow().accepted.len(), 3);
    assert_eq!(with_cx(|cx| sink.poll_end(cx)), Poll::Ready(Ok(())));
    pending(&mut serve);
    assert_eq!(
        poll(&mut call),
        Poll::Ready(Ok(generated::Summary { count: 3, total: 6 }))
    );
    assert_eq!(s.borrow().ends, 1);
    assert_eq!(s.borrow().replies, 1);
}
#[test]
fn the_reply_bound_starts_at_the_end_of_the_upload() {
    let (mut rt, _, _) = setup();
    let mut client = generated::log::Client::new(rt.caller());
    let (mut call, mut sink) = client.summarize(generated::LogSummarizeParams {});
    rt.advance(Duration(100_001));
    pending(&mut call);
    assert_eq!(with_cx(|cx| sink.poll_end(cx)), Poll::Ready(Ok(())));
    rt.advance(Duration(99_999));
    pending(&mut call);
    rt.advance(Duration(2));
    assert_eq!(
        poll(&mut call),
        Poll::Ready(Err(ClientError::Call(CallError::Transport(
            Transport::Timeout
        ))))
    );
}
#[test]
fn a_pipe_reply_bound_also_starts_at_the_end_of_the_upload() {
    let (mut rt, _, _) = setup();
    let mut client = generated::log::Client::new(rt.caller());
    let (mut call, mut sink) = client.pipe(generated::LogPipeParams {});
    rt.advance(Duration(1_000_001));
    pending(&mut call);
    assert_eq!(with_cx(|cx| sink.poll_end(cx)), Poll::Ready(Ok(())));
    rt.advance(Duration(999_999));
    pending(&mut call);
    rt.advance(Duration(2));
    assert!(matches!(
        poll(&mut call),
        Poll::Ready(Err(ClientError::Call(CallError::Transport(
            Transport::Timeout
        ))))
    ));
}
#[test]
fn a_refusal_of_the_upload_settles_the_query_with_the_error() {
    let (rt, s, mut provider) = setup();
    let operations = Rc::new(RefCell::new(Vec::new()));
    let handler = ObservedHandler {
        inner: rt.handler(),
        operations: operations.clone(),
    };
    let mut serve = generated::log::serve(handler, &mut provider);
    let ports = RecordingPorts::new(&rt);
    let ops = ports.log();
    let mut raw = rt.caller();
    let mut client = generated::log::Client::new(ports);
    let (mut call, mut sink) = client.summarize(generated::LogSummarizeParams {});
    pending(&mut serve);
    let mut buffer = <generated::Reading as Payload<FlatBuffers>>::BUFFER;
    let bad = reading(200);
    let encoded = Ref::<generated::Reading, FlatBuffers>::encode(&bad, &mut buffer).unwrap();
    raw.push(correlation(&ops), StreamNo(1), encoded.bytes())
        .unwrap();
    for _ in 0..4 {
        pending(&mut serve);
    }
    let error = s.borrow().failures[0];
    assert!(matches!(
        error,
        CallError::Contract(Contract::InvalidValue(_))
    ));
    assert_eq!(poll(&mut call), Poll::Ready(Err(ClientError::Call(error))));
    assert_eq!(s.borrow().failures.len(), 1);
    assert_eq!(s.borrow().replies, 0);
    assert!(s.borrow().accepted.is_empty());
    assert_eq!(sink.send(&reading(4)), Err(ClientError::Call(error)));
    assert_eq!(
        *operations.borrow(),
        vec!["abort_upload", "settle_error"],
        "refusal aborts the upload before settling the query"
    );
}
#[test]
fn a_pipe_reply_starts_before_the_upload_ends() {
    let (rt, s, mut provider) = setup();
    let mut serve = generated::log::serve(rt.handler(), &mut provider);
    let mut client = generated::log::Client::new(rt.caller());
    let (mut call, mut sink) = client.pipe(generated::LogPipeParams {});
    pending(&mut serve);
    sink.send(&reading(42)).unwrap();
    pending(&mut serve);
    let mut stream = match poll(&mut call) {
        Poll::Ready(Ok(stream)) => stream,
        _ => panic!("reply must start during upload"),
    };
    assert_eq!(
        with_cx(|cx| stream.poll_next(cx)),
        Poll::Ready(Some(Ok(reading(42))))
    );
    assert_eq!(s.borrow().ends, 0);
    assert_eq!(with_cx(|cx| sink.poll_end(cx)), Poll::Ready(Ok(())));
    pending(&mut serve);
    assert_eq!(with_cx(|cx| stream.poll_next(cx)), Poll::Ready(None));
    assert_eq!(s.borrow().ends, 1);
    assert_eq!(
        s.borrow().failures,
        Vec::<CallError>::new(),
        "a completed input must not also fail on source drop"
    );
}
#[test]
fn a_pipe_whose_sink_and_stream_are_dropped_forgets_once() {
    for sink_first in [true, false] {
        let (rt, _, mut provider) = setup();
        let mut serve = generated::log::serve(rt.handler(), &mut provider);
        let ports = RecordingPorts::new(&rt);
        let ops = ports.log();
        let mut client = generated::log::Client::new(ports);
        let (mut call, mut sink) = client.pipe(generated::LogPipeParams {});
        pending(&mut serve);
        sink.send(&reading(1)).unwrap();
        pending(&mut serve);
        let stream = match poll(&mut call) {
            Poll::Ready(Ok(s)) => s,
            _ => panic!("reply stream"),
        };
        drop(call);
        assert!(doubles::forgets(&ops).is_empty());
        if sink_first {
            drop(sink);
            drop(stream);
        } else {
            drop(stream);
            drop(sink);
        }
        assert_eq!(doubles::forgets(&ops).len(), 1);
    }
}
macro_rules! cancellation {
    ($test:ident,$method:ident,$args:expr) => {
        #[test]
        fn $test() {
            let (rt, _, _) = setup();
            let ports = RecordingPorts::new(&rt);
            let ops = ports.log();
            let faults = ports.faults();
            faults.send.set(Some(SendError::Busy));
            let mut client = generated::log::Client::new(ports);
            let (mut call, sink) = client.$method($args);
            drop(sink);
            faults.send.set(None);
            assert!(matches!(
                poll(&mut call),
                Poll::Ready(Err(ClientError::Call(CallError::Transport(
                    Transport::Interrupted
                ))))
            ));
            assert_eq!(
                ops.borrow()
                    .iter()
                    .filter(|op| matches!(op, Op::Query(_)))
                    .count(),
                1
            );
        }
    };
}
cancellation!(
    a_pending_summary_is_cancelled_when_its_sink_is_dropped,
    summarize,
    generated::LogSummarizeParams {}
);
cancellation!(
    a_pending_pipe_is_cancelled_when_its_sink_is_dropped,
    pipe,
    generated::LogPipeParams {}
);
#[test]
fn an_ended_summary_sink_dropped_before_the_claim_preserves_the_query() {
    let (rt, s, mut provider) = setup();
    let ports = RecordingPorts::new(&rt);
    let ops = ports.log();
    let mut client = generated::log::Client::new(ports);
    let (mut call, mut sink) = client.summarize(generated::LogSummarizeParams {});
    sink.send(&reading(7)).unwrap();
    assert_eq!(with_cx(|cx| sink.poll_end(cx)), Poll::Ready(Ok(())));
    drop(sink);
    assert!(doubles::forgets(&ops).is_empty());
    let mut serve = generated::log::serve(rt.handler(), &mut provider);
    for _ in 0..5 {
        pending(&mut serve);
    }
    assert_eq!(
        poll(&mut call),
        Poll::Ready(Ok(generated::Summary { count: 1, total: 7 }))
    );
    assert_eq!(doubles::forgets(&ops).len(), 1);
    assert_eq!(s.borrow().ends, 1);
}

#[test]
fn a_pipe_upload_refusal_is_a_terminal_query_error() {
    let (rt, s, mut provider) = setup();
    let operations = Rc::new(RefCell::new(Vec::new()));
    let handler = ObservedHandler {
        inner: rt.handler(),
        operations: operations.clone(),
    };
    let mut serve = generated::log::serve(handler, &mut provider);
    let ports = RecordingPorts::new(&rt);
    let ops = ports.log();
    let mut raw = rt.caller();
    let mut client = generated::log::Client::new(ports);
    let (mut call, mut sink) = client.pipe(generated::LogPipeParams {});
    pending(&mut serve);
    let bad = reading(200);
    let mut buf = <generated::Reading as Payload<FlatBuffers>>::BUFFER;
    let encoded = Ref::<generated::Reading, FlatBuffers>::encode(&bad, &mut buf).unwrap();
    raw.push(correlation(&ops), StreamNo(1), encoded.bytes())
        .unwrap();
    for _ in 0..4 {
        pending(&mut serve);
    }
    let e = s.borrow().failures[0];
    assert_eq!(s.borrow().failures.len(), 1);
    let mut stream = match poll(&mut call) {
        Poll::Ready(Ok(stream)) => stream,
        _ => panic!("the terminal reply must be available"),
    };
    assert_eq!(
        with_cx(|cx| stream.poll_next(cx)),
        Poll::Ready(Some(Err(ClientError::Call(e))))
    );

    assert_eq!(with_cx(|cx| stream.poll_next(cx)), Poll::Ready(None));
    assert_eq!(
        *operations.borrow(),
        vec!["abort_upload", "settle_error"],
        "refusal aborts the upload before settling the query"
    );
    assert_eq!(sink.send(&reading(1)), Err(ClientError::Call(e)));
}
#[test]
fn an_upload_query_is_busy_until_its_consumer_ends() {
    let (rt, s, mut provider) = setup();
    let mut serve = generated::log::serve(rt.handler(), &mut provider);
    let mut first = generated::log::Client::new(rt.caller());
    let (mut first_call, mut first_sink) = first.summarize(generated::LogSummarizeParams {});
    pending(&mut serve);
    let mut second = generated::log::Client::new(rt.caller());
    let (mut second_call, second_sink) = second.summarize(generated::LogSummarizeParams {});
    pending(&mut serve);
    assert_eq!(
        poll(&mut second_call),
        Poll::Ready(Err(ClientError::Call(CallError::Transport(
            Transport::Busy
        ))))
    );
    assert_eq!(s.borrow().starts, 1);
    drop(second_sink);
    drop(second_call);
    assert_eq!(with_cx(|cx| first_sink.poll_end(cx)), Poll::Ready(Ok(())));
    pending(&mut serve);
    assert!(matches!(poll(&mut first_call), Poll::Ready(Ok(_))));
    drop(first_sink);
    drop(first_call);
    let (mut next, mut sink) = second.summarize(generated::LogSummarizeParams {});
    pending(&mut serve);
    assert_eq!(s.borrow().starts, 2);
    assert_eq!(with_cx(|cx| sink.poll_end(cx)), Poll::Ready(Ok(())));
    pending(&mut serve);
    assert!(matches!(poll(&mut next), Poll::Ready(Ok(_))));
}
#[test]
fn dropping_serve_settles_an_upload_query_as_interrupted() {
    let (rt, s, mut provider) = setup();
    let mut handler = rt.handler();
    let operations = Rc::new(RefCell::new(Vec::new()));
    let observed = ObservedHandler {
        inner: &mut handler,
        operations: operations.clone(),
    };
    let mut serve = generated::log::serve(observed, &mut provider);
    let mut client = generated::log::Client::new(rt.caller());
    let (mut call, _sink) = client.summarize(generated::LogSummarizeParams {});
    pending(&mut serve);
    drop(serve);
    assert_eq!(
        poll(&mut call),
        Poll::Ready(Err(ClientError::Call(CallError::Transport(
            Transport::Interrupted
        ))))
    );
    assert_eq!(
        s.borrow().failures,
        vec![CallError::Transport(Transport::Interrupted)]
    );
    assert_eq!(*operations.borrow(), vec!["abort_upload", "settle_error"]);
}

/// A reply the handler refuses as too large is settled again as
/// `Transport::Corrupt` (driftsys/ridl#1260), as for a query without an upload.
#[test]
fn a_refused_reply_of_an_upload_query_is_settled_again_as_corrupt() {
    let (mut rt, _s, mut provider) = setup();
    let mut serve = generated::log::serve(rt.handler(), &mut provider);
    let mut client = generated::log::Client::new(rt.caller());
    let (mut call, mut sink) = client.summarize(generated::LogSummarizeParams {});
    pending(&mut call);
    pending(&mut serve);
    sink.send(&reading(1)).unwrap();
    pending(&mut serve);
    assert_eq!(with_cx(|cx| sink.poll_end(cx)), Poll::Ready(Ok(())));
    rt.fail_next_settle();
    pending(&mut serve);
    assert_eq!(
        poll(&mut call),
        Poll::Ready(Err(ClientError::Call(CallError::Transport(
            Transport::Corrupt
        ))))
    );
}
#[test]
fn a_broken_ensure_on_an_upload_query_settles_contract_broken() {
    let (rt, s, mut provider) = setup();
    let operations = Rc::new(RefCell::new(Vec::new()));
    let handler = ObservedHandler {
        inner: rt.handler(),
        operations: operations.clone(),
    };
    let mut serve = generated::log::serve(handler, &mut provider);
    let mut client = generated::log::Client::new(rt.caller());
    let (mut call, mut sink) = client.checked(generated::LogCheckedParams {});
    pending(&mut serve);
    sink.send(&reading(7)).unwrap();
    pending(&mut serve);
    assert_eq!(with_cx(|cx| sink.poll_end(cx)), Poll::Ready(Ok(())));
    pending(&mut serve);
    assert_eq!(
        poll(&mut call),
        Poll::Ready(Err(ClientError::Call(CallError::Contract(
            Contract::ContractBroken
        ))))
    );
    assert_eq!(s.borrow().accepted, vec![reading(7)]);
    assert_eq!(s.borrow().ends, 1);
    assert_eq!(s.borrow().replies, 1);
    assert_eq!(*operations.borrow(), vec!["settle_error"]);
}

#[test]
fn a_pipe_is_busy_until_its_consumer_ends() {
    let (rt, s, mut provider) = setup();
    let mut serve = generated::log::serve(rt.handler(), &mut provider);
    let mut first = generated::log::Client::new(rt.caller());
    let (mut first_call, mut first_sink) = first.pipe(generated::LogPipeParams {});
    pending(&mut serve);
    let mut second = generated::log::Client::new(rt.caller());
    let (mut second_call, second_sink) = second.pipe(generated::LogPipeParams {});
    pending(&mut serve);
    // Busy is a terminal reply-stream error, available before any element.
    let mut refused = {
        let reply = poll(&mut second_call);
        assert!(matches!(reply, Poll::Ready(Ok(_))));
        match reply {
            Poll::Ready(Ok(stream)) => stream,
            _ => unreachable!(),
        }
    };
    assert_eq!(
        with_cx(|cx| refused.poll_next(cx)),
        Poll::Ready(Some(Err(ClientError::Call(CallError::Transport(
            Transport::Busy
        )))))
    );
    assert_eq!(s.borrow().starts, 1);
    drop(refused);
    drop(second_sink);
    drop(second_call);
    assert_eq!(with_cx(|cx| first_sink.poll_end(cx)), Poll::Ready(Ok(())));
    pending(&mut serve);
    let mut stream = match poll(&mut first_call) {
        Poll::Ready(Ok(stream)) => stream,
        _ => panic!("the first pipe must end"),
    };
    assert_eq!(with_cx(|cx| stream.poll_next(cx)), Poll::Ready(None));
    drop(stream);
    drop(first_sink);
    drop(first_call);
    let (mut next, mut sink) = second.pipe(generated::LogPipeParams {});
    pending(&mut serve);
    assert_eq!(s.borrow().starts, 2);
    assert_eq!(with_cx(|cx| sink.poll_end(cx)), Poll::Ready(Ok(())));
    pending(&mut serve);
    let mut stream = match poll(&mut next) {
        Poll::Ready(Ok(stream)) => stream,
        _ => panic!("the next pipe must end"),
    };
    assert_eq!(with_cx(|cx| stream.poll_next(cx)), Poll::Ready(None));
}

#[test]
fn dropping_serve_aborts_both_streams_of_a_running_pipe() {
    let (rt, s, mut provider) = setup();
    let mut handler = rt.handler();
    let operations = Rc::new(RefCell::new(Vec::new()));
    let observed = ObservedHandler {
        inner: &mut handler,
        operations: operations.clone(),
    };
    let mut serve = generated::log::serve(observed, &mut provider);
    let mut client = generated::log::Client::new(rt.caller());
    let (mut call, mut sink) = client.pipe(generated::LogPipeParams {});
    pending(&mut serve);
    sink.send(&reading(8)).unwrap();
    pending(&mut serve);
    let mut stream = match poll(&mut call) {
        Poll::Ready(Ok(stream)) => stream,
        _ => panic!("reply stream"),
    };
    assert_eq!(
        with_cx(|cx| stream.poll_next(cx)),
        Poll::Ready(Some(Ok(reading(8))))
    );
    drop(serve);
    assert_eq!(
        s.borrow().failures,
        vec![CallError::Transport(Transport::Interrupted)]
    );
    assert_eq!(*operations.borrow(), vec!["abort_upload", "abort_reply"]);
    assert_eq!(
        with_cx(|cx| stream.poll_next(cx)),
        Poll::Ready(Some(Err(ClientError::Call(CallError::Transport(
            Transport::Interrupted
        )))))
    );
    assert_eq!(with_cx(|cx| stream.poll_next(cx)), Poll::Ready(None));
}

#[test]
fn dropping_an_ended_pipe_sink_preserves_its_live_reply_stream() {
    let (rt, _, mut provider) = setup();
    let mut serve = generated::log::serve(rt.handler(), &mut provider);
    let ports = RecordingPorts::new(&rt);
    let ops = ports.log();
    let mut client = generated::log::Client::new(ports);
    let (mut call, mut sink) = client.pipe(generated::LogPipeParams {});
    pending(&mut serve);
    sink.send(&reading(9)).unwrap();
    pending(&mut serve);
    let mut stream = match poll(&mut call) {
        Poll::Ready(Ok(stream)) => stream,
        _ => panic!("reply stream"),
    };
    drop(call);
    assert_eq!(with_cx(|cx| sink.poll_end(cx)), Poll::Ready(Ok(())));
    drop(sink);
    assert!(doubles::forgets(&ops).is_empty());
    assert_eq!(
        with_cx(|cx| stream.poll_next(cx)),
        Poll::Ready(Some(Ok(reading(9))))
    );
    pending(&mut serve);
    assert_eq!(with_cx(|cx| stream.poll_next(cx)), Poll::Ready(None));
    assert_eq!(doubles::forgets(&ops).len(), 1);
    drop(stream);
    assert_eq!(doubles::forgets(&ops).len(), 1);
}

macro_rules! ended_sink_call_drop {
    ($test:ident, $method:ident, $args:expr) => {
        #[test]
        fn $test() {
            let (rt, s, mut provider) = setup();
            let ports = RecordingPorts::new(&rt);
            let ops = ports.log();
            let mut client = generated::log::Client::new(ports);
            let (mut call, mut sink) = client.$method($args);
            pending(&mut call);
            assert_eq!(with_cx(|cx| sink.poll_end(cx)), Poll::Ready(Ok(())));
            drop(sink);
            assert!(doubles::forgets(&ops).is_empty());
            drop(call);
            assert_eq!(doubles::forgets(&ops).len(), 1);
            // Forgetting before the claim withdraws the call from the provider.
            let mut serve = generated::log::serve(rt.handler(), &mut provider);
            pending(&mut serve);
            assert_eq!(s.borrow().starts, 0);
            assert_eq!(doubles::forgets(&ops).len(), 1);
        }
    };
}
ended_sink_call_drop!(
    dropping_an_unresolved_summary_after_its_ended_sink_forgets_once,
    summarize,
    generated::LogSummarizeParams {}
);
ended_sink_call_drop!(
    dropping_an_unresolved_pipe_after_its_ended_sink_forgets_once,
    pipe,
    generated::LogPipeParams {}
);

#[test]
fn dropping_an_unended_pipe_sink_immediately_interrupts_the_call() {
    let (rt, s, mut provider) = setup();
    let mut serve = generated::log::serve(rt.handler(), &mut provider);
    let ports = RecordingPorts::new(&rt);
    let ops = ports.log();
    let mut client = generated::log::Client::new(ports);
    let (mut call, mut sink) = client.pipe(generated::LogPipeParams {});
    pending(&mut serve);
    sink.send(&reading(10)).unwrap();
    pending(&mut serve);
    let mut stream = match poll(&mut call) {
        Poll::Ready(Ok(stream)) => stream,
        _ => panic!("reply stream"),
    };
    drop(call);
    assert!(doubles::forgets(&ops).is_empty());
    drop(sink);
    assert_eq!(doubles::forgets(&ops).len(), 1);
    // The first reply element is already buffered in the stream value.
    assert_eq!(
        with_cx(|cx| stream.poll_next(cx)),
        Poll::Ready(Some(Ok(reading(10))))
    );
    assert_eq!(
        with_cx(|cx| stream.poll_next(cx)),
        Poll::Ready(Some(Err(ClientError::Call(CallError::Transport(
            Transport::Interrupted
        )))))
    );
    assert_eq!(with_cx(|cx| stream.poll_next(cx)), Poll::Ready(None));
    pending(&mut serve);
    assert_eq!(
        s.borrow().failures,
        vec![CallError::Transport(Transport::Interrupted)]
    );
    drop(stream);
    assert_eq!(doubles::forgets(&ops).len(), 1);
}

#[test]
fn an_unresolved_call_whose_unended_sink_is_dropped_settles_interrupted() {
    let interrupted = ClientError::Call(CallError::Transport(Transport::Interrupted));
    let (rt, _, mut provider) = setup();
    let mut serve = generated::log::serve(rt.handler(), &mut provider);
    let ports = RecordingPorts::new(&rt);
    let ops = ports.log();
    let mut client = generated::log::Client::new(ports);
    let (mut call, mut sink) = client.summarize(generated::LogSummarizeParams {});
    pending(&mut call);
    pending(&mut serve);
    sink.send(&reading(1)).unwrap();
    drop(sink);
    assert_eq!(doubles::forgets(&ops).len(), 1);
    assert_eq!(poll(&mut call), Poll::Ready(Err(interrupted)));
    drop(call);
    assert_eq!(doubles::forgets(&ops).len(), 1);

    let (mut call, sink) = client.pipe(generated::LogPipeParams {});
    pending(&mut call);
    pending(&mut serve);
    drop(sink);
    assert_eq!(doubles::forgets(&ops).len(), 2);
    assert!(matches!(poll(&mut call), Poll::Ready(Err(e)) if e == interrupted));
    drop(call);
    assert_eq!(doubles::forgets(&ops).len(), 2);
}

struct ObservedHandler<H> {
    inner: H,
    operations: Rc<RefCell<Vec<&'static str>>>,
}
impl<H: ridl_rt::port::Encoded> ridl_rt::port::Encoded for ObservedHandler<H> {
    type Encoding = H::Encoding;
}
impl<H: ridl_rt::port::Attached> ridl_rt::port::Attached for ObservedHandler<H> {
    fn catalog(&self) -> &CatalogRef {
        self.inner.catalog()
    }
}
impl<H: ridl_rt::port::Handler> ridl_rt::port::Handler for ObservedHandler<H> {
    fn serve(
        &mut self,
        i: ridl_rt::contract::InterfaceNo,
        o: &[ridl_rt::contract::Ordinal],
    ) -> Result<(), ridl_rt::port::ServeError> {
        self.inner.serve(i, o)
    }
    fn next_claim(
        &mut self,
        out: &mut [u8],
    ) -> Result<Option<ridl_rt::port::Claim>, ridl_rt::port::ReadError> {
        self.inner.next_claim(out)
    }
    fn settle(
        &mut self,
        c: ridl_rt::port::ClaimId,
        r: Result<&[u8], CallError>,
    ) -> Result<(), ridl_rt::port::SettleError> {
        self.operations.borrow_mut().push(if r.is_err() {
            "settle_error"
        } else {
            "settle_ok"
        });
        self.inner.settle(c, r)
    }
}
impl<H: ridl_rt::port::StreamHandler> ridl_rt::port::StreamHandler for ObservedHandler<H> {
    fn claim_forget(&self, c: ridl_rt::port::ClaimId) -> ridl_rt::port::ClaimForget {
        self.inner.claim_forget(c)
    }
    fn wake_on_claim_forget(&self, c: ridl_rt::port::ClaimId, waker: &Waker) {
        self.inner.wake_on_claim_forget(c, waker);
    }
    fn pull_claim(
        &mut self,
        c: ridl_rt::port::ClaimId,
        s: StreamNo,
        b: &mut [u8],
    ) -> Result<ridl_rt::port::Pull, ridl_rt::port::ReadError> {
        self.inner.pull_claim(c, s, b)
    }
    fn push_claim(
        &mut self,
        c: ridl_rt::port::ClaimId,
        s: StreamNo,
        b: &[u8],
    ) -> Result<usize, ridl_rt::port::PushError> {
        self.inner.push_claim(c, s, b)
    }
    fn end_claim(
        &mut self,
        c: ridl_rt::port::ClaimId,
        s: StreamNo,
    ) -> Result<(), ridl_rt::port::PushError> {
        self.inner.end_claim(c, s)
    }
    fn abort_claim(
        &mut self,
        c: ridl_rt::port::ClaimId,
        s: StreamNo,
        e: CallError,
    ) -> Result<(), ridl_rt::port::PushError> {
        self.operations.borrow_mut().push(if s == StreamNo(1) {
            "abort_upload"
        } else {
            "abort_reply"
        });
        self.inner.abort_claim(c, s, e)
    }
}
impl<H: ridl_rt::port::Wakeable> ridl_rt::port::Wakeable for ObservedHandler<H> {
    fn wake_on(&self, i: ridl_rt::port::Interest, w: &Waker) {
        self.inner.wake_on(i, w)
    }
}

#[test]
fn a_duplex_proto3_zero_byte_element_is_delivered_once_in_each_direction() {
    let (_, s, mut provider) = setup();
    let rt = Loopback::<Proto3>::with_encoding(CATALOG);
    let zero = reading(0);
    let mut buf = <generated::Reading as Payload<Proto3>>::BUFFER;
    assert_eq!(
        Ref::<generated::Reading, Proto3>::encode(&zero, &mut buf)
            .unwrap()
            .bytes()
            .len(),
        0
    );
    let mut serve = generated::log::serve(rt.handler(), &mut provider);
    let mut client = generated::log::Client::new(rt.caller());
    let (mut call, mut sink) = client.pipe(generated::LogPipeParams {});
    pending(&mut serve);
    sink.send(&zero).unwrap();
    pending(&mut serve);
    let mut stream = match poll(&mut call) {
        Poll::Ready(Ok(s)) => s,
        _ => panic!("first zero-byte reply"),
    };
    assert_eq!(
        with_cx(|cx| stream.poll_next(cx)),
        Poll::Ready(Some(Ok(zero)))
    );
    assert_eq!(with_cx(|cx| sink.poll_end(cx)), Poll::Ready(Ok(())));
    for _ in 0..6 {
        pending(&mut serve);
    }
    assert_eq!(with_cx(|cx| stream.poll_next(cx)), Poll::Ready(None));
    assert_eq!(s.borrow().accepted, vec![zero]);
    assert_eq!(s.borrow().ends, 1);
}

#[test]
fn validation_before_admission_cancels_the_query_and_wakes_once() {
    let (rt, _, _) = setup();
    let ports = RecordingPorts::new(&rt);
    let ops = ports.log();
    let faults = ports.faults();
    faults.send.set(Some(SendError::Busy));
    let mut client = generated::log::Client::new(ports);
    let (mut call, mut sink) = client.summarize(generated::LogSummarizeParams {});
    let count = std::sync::Arc::new(WakeCount::default());
    let waker = Waker::from(count.clone());
    faults.send.set(Some(SendError::Busy));
    assert!(
        Pin::new(&mut call)
            .poll(&mut Context::from_waker(&waker))
            .is_pending()
    );
    count.0.store(0, std::sync::atomic::Ordering::SeqCst);
    let error = sink.send(&reading(200)).unwrap_err();
    drop(sink);
    assert_eq!(count.0.load(std::sync::atomic::Ordering::SeqCst), 1);
    assert_eq!(poll(&mut call), Poll::Ready(Err(error)));
    assert_eq!(
        ops.borrow()
            .iter()
            .filter(|op| matches!(op, Op::Query(_)))
            .count(),
        2
    );
    drop(call);
    assert_eq!(count.0.load(std::sync::atomic::Ordering::SeqCst), 1);
}
#[derive(Default)]
struct WakeCount(std::sync::atomic::AtomicUsize);
impl std::task::Wake for WakeCount {
    fn wake(self: std::sync::Arc<Self>) {
        self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }
}

#[test]
fn a_blocked_query_consumer_receives_terminal_outcomes_and_releases_busy() {
    for abort in [true, false] {
        let (rt, s, mut provider) = setup();
        s.borrow_mut().blocked = true;
        let mut serve = generated::log::serve(rt.handler(), &mut provider);
        let mut client = generated::log::Client::new(rt.caller());
        let (call, mut sink) = client.summarize(generated::LogSummarizeParams {});
        pending(&mut serve);
        if abort {
            drop(sink);
        } else {
            assert_eq!(with_cx(|cx| sink.poll_end(cx)), Poll::Ready(Ok(())));
            drop(sink);
        }
        for _ in 0..3 {
            pending(&mut serve);
        }
        assert_eq!(s.borrow().ends, usize::from(!abort));
        assert_eq!(
            s.borrow().failures,
            if abort {
                vec![CallError::Transport(Transport::Interrupted)]
            } else {
                vec![]
            }
        );
        drop(call);
        let (mut next, mut sink) = client.summarize(generated::LogSummarizeParams {});
        pending(&mut serve);
        assert_eq!(s.borrow().starts, 2);
        assert_eq!(with_cx(|cx| sink.poll_end(cx)), Poll::Ready(Ok(())));
        pending(&mut serve);
        assert!(matches!(poll(&mut next), Poll::Ready(Ok(_))));
    }
}

#[test]
fn a_held_query_element_precedes_end() {
    let (rt, s, mut provider) = setup();
    s.borrow_mut().blocked = true;
    let mut serve = generated::log::serve(rt.handler(), &mut provider);
    let mut client = generated::log::Client::new(rt.caller());
    let (mut call, mut sink) = client.summarize(generated::LogSummarizeParams {});
    pending(&mut serve);
    sink.send(&reading(7)).unwrap();
    assert_eq!(with_cx(|cx| sink.poll_end(cx)), Poll::Ready(Ok(())));
    for _ in 0..3 {
        pending(&mut serve);
        assert_eq!(s.borrow().ends, 0);
        assert!(s.borrow().accepted.is_empty());
        assert!(s.borrow().callbacks.is_empty());
    }
    s.borrow_mut().blocked = false;
    for _ in 0..4 {
        pending(&mut serve);
    }
    assert_eq!(s.borrow().accepted, vec![reading(7)]);
    assert_eq!(s.borrow().ends, 1);
    assert_eq!(s.borrow().callbacks, vec!["accept", "end"]);
    assert_eq!(
        poll(&mut call),
        Poll::Ready(Ok(generated::Summary { count: 1, total: 7 }))
    );
}

#[test]
fn dropping_either_query_value_wakes_its_pending_sibling_once() {
    for sink_first in [true, false] {
        let (rt, _, _) = setup();
        let ports = RecordingPorts::new(&rt);
        let faults = ports.faults();
        faults.send.set(Some(SendError::Busy));
        let mut client = generated::log::Client::new(ports);
        let (mut call, mut sink) = client.summarize(generated::LogSummarizeParams {});
        let count = std::sync::Arc::new(WakeCount::default());
        let waker = Waker::from(count.clone());
        let mut cx = Context::from_waker(&waker);
        if sink_first {
            faults.send.set(Some(SendError::Busy));
            assert!(Pin::new(&mut call).poll(&mut cx).is_pending());
            count.0.store(0, std::sync::atomic::Ordering::SeqCst);
            drop(sink);
            assert_eq!(count.0.load(std::sync::atomic::Ordering::SeqCst), 1);
            assert!(matches!(poll(&mut call), Poll::Ready(Err(_))));
            drop(call);
        } else {
            assert!(sink.poll_end(&mut cx).is_pending());
            count.0.store(0, std::sync::atomic::Ordering::SeqCst);
            drop(call);
            assert_eq!(count.0.load(std::sync::atomic::Ordering::SeqCst), 1);
            assert!(matches!(sink.poll_end(&mut cx), Poll::Ready(Err(_))));
            drop(sink);
        }
        assert_eq!(count.0.load(std::sync::atomic::Ordering::SeqCst), 1);
    }
}

#[test]
fn dropping_a_locally_refused_admitted_sink_preserves_the_query_validation_error() {
    let (rt, _, mut provider) = setup();
    let mut serve = generated::log::serve(rt.handler(), &mut provider);
    let mut client = generated::log::Client::new(rt.caller());
    let (mut call, mut sink) = client.summarize(generated::LogSummarizeParams {});
    pending(&mut serve);
    let error = sink.send(&reading(200)).unwrap_err();
    assert!(matches!(error, ClientError::Call(CallError::Contract(_))));
    drop(sink);
    assert_eq!(poll(&mut call), Poll::Ready(Err(error)));
}

#[test]
fn cancelling_a_query_wakes_a_named_sink_holding_an_element_before_admission() {
    let (rt, _, _) = setup();
    let ports = RecordingPorts::new(&rt);
    ports.faults().send.set(Some(SendError::Busy));
    let mut client = generated::log::Client::new(ports);
    let (call, mut sink) = client.summarize(generated::LogSummarizeParams {});
    sink.send(&reading(7)).unwrap();
    let count = std::sync::Arc::new(WakeCount::default());
    let waker = Waker::from(count.clone());
    let mut cx = Context::from_waker(&waker);
    assert!(sink.poll_ready(&mut cx).is_pending());
    count.0.store(0, std::sync::atomic::Ordering::SeqCst);
    drop(call);
    assert_eq!(count.0.load(std::sync::atomic::Ordering::SeqCst), 1);
    assert_eq!(
        sink.poll_ready(&mut cx),
        Poll::Ready(Err(ClientError::Call(CallError::Transport(
            Transport::Interrupted
        ))))
    );
}

#[test]
fn forgetting_a_query_drops_its_pending_consumer_without_polling_it_again() {
    let (rt, state, mut provider) = setup();
    state.borrow_mut().blocked = true;
    let mut serve = generated::log::serve(rt.handler(), &mut provider);
    let mut client = generated::log::Client::new(rt.caller());
    for round in 1..=Loopback::SLOTS + 1 {
        let (call, mut sink) = client.summarize(generated::LogSummarizeParams {});
        pending(&mut serve);
        sink.send(&reading(7)).unwrap();
        pending(&mut serve);
        let polls = state.borrow().ready_polls;
        if round % 2 == 0 {
            assert_eq!(with_cx(|cx| sink.poll_end(cx)), Poll::Ready(Ok(())));
        }
        drop(call);
        pending(&mut serve);
        assert_eq!(
            state.borrow().drops,
            round,
            "the forgotten consumer is dropped"
        );
        assert_eq!(
            state.borrow().ready_polls,
            polls,
            "release does not poll the consumer"
        );
        drop(sink);
    }
    assert_eq!(state.borrow().starts, Loopback::SLOTS + 1);
}

#[test]
fn dropping_an_acknowledged_commands_sink_drops_its_pending_consumer() {
    let (rt, state, mut provider) = setup();
    state.borrow_mut().blocked = true;
    let mut serve = generated::log::serve(rt.handler(), &mut provider);
    let mut client = generated::log::Client::new(rt.caller());
    let (mut call, mut sink) = client.upload(generated::LogUploadParams {});
    pending(&mut serve);
    assert_eq!(poll(&mut call), Poll::Ready(Ok(())));
    sink.send(&reading(7)).unwrap();
    pending(&mut serve);
    let polls = state.borrow().ready_polls;
    drop(sink);
    pending(&mut serve);
    assert_eq!(state.borrow().drops, 1);
    assert_eq!(state.borrow().ready_polls, polls);
    assert_eq!(
        state.borrow().failures,
        vec![CallError::Transport(Transport::Interrupted)]
    );
}

#[test]
fn a_normally_forgotten_command_keeps_its_ended_pending_consumer_until_completion() {
    let (rt, state, mut provider) = setup();
    state.borrow_mut().blocked = true;
    let mut serve = generated::log::serve(rt.handler(), &mut provider);
    let ports = RecordingPorts::new(&rt);
    let log = ports.log();
    let mut client = generated::log::Client::new(ports);
    let (mut call, mut sink) = client.upload(generated::LogUploadParams {});
    pending(&mut serve);
    assert_eq!(poll(&mut call), Poll::Ready(Ok(())));
    sink.send(&reading(7)).unwrap();
    pending(&mut serve);
    sink.send(&reading(8)).unwrap();
    assert_eq!(with_cx(|cx| sink.poll_end(cx)), Poll::Ready(Ok(())));
    assert_eq!(
        doubles::forgets(&log).len(),
        1,
        "end after acknowledgment normally forgets the call"
    );
    drop(call);
    drop(sink);
    for _ in 0..3 {
        pending(&mut serve);
    }
    assert_eq!(
        state.borrow().drops,
        0,
        "normal forget preserves the consumer"
    );
    assert!(state.borrow().failures.is_empty());
    state.borrow_mut().blocked = false;
    for _ in 0..4 {
        pending(&mut serve);
    }
    assert_eq!(state.borrow().accepted, vec![reading(7), reading(8)]);
    assert_eq!(state.borrow().ends, 1);
    assert_eq!(state.borrow().drops, 1);
}

#[test]
fn forgetting_a_query_after_parameter_end_wakes_serve_and_drops_its_pending_reply() {
    let (rt, state, mut provider) = setup();
    state.borrow_mut().reply_blocked = true;
    let mut serve = generated::log::serve(rt.handler(), &mut provider);
    let mut client = generated::log::Client::new(rt.caller());
    let (call, mut sink) = client.pipe(generated::LogPipeParams {});
    assert_eq!(with_cx(|cx| sink.poll_end(cx)), Poll::Ready(Ok(())));
    pending(&mut serve);
    pending(&mut serve);
    assert_eq!(state.borrow().ends, 1);
    let count = std::sync::Arc::new(WakeCount::default());
    let waker = Waker::from(count.clone());
    assert!(
        Pin::new(&mut serve)
            .poll(&mut Context::from_waker(&waker))
            .is_pending()
    );
    assert_eq!(
        count.0.load(std::sync::atomic::Ordering::SeqCst),
        0,
        "an ended parameter does not wake the forget registration at once"
    );
    let polls = state.borrow().source_polls;
    drop(call);
    assert_eq!(
        count.0.load(std::sync::atomic::Ordering::SeqCst),
        1,
        "forget wakes serve after the parameter ended"
    );
    pending(&mut serve);
    assert_eq!(state.borrow().drops, 1);
    assert_eq!(
        state.borrow().source_polls,
        polls,
        "release does not poll the pending reply"
    );
    assert_eq!(state.borrow().ends, 1);
    assert!(
        state.borrow().failures.is_empty(),
        "an ended consumer is not failed again"
    );
    drop(sink);
}
