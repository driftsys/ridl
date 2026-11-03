//! Parameter stream commands over the generated face and loopback ports.
#[allow(dead_code, unused_imports)]
mod generated {
    include!("generated/stream_face.rs");
}
use core::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};
use ridl_loopback::Loopback;
use ridl_rt::{
    contract::Interface,
    encoding::{FlatBuffers, Proto3},
    error::{CallError, ClientError, Transport},
    face::{Bind, ByteConsumer, Consumer},
    payload::{Payload, Ref},
    port::StreamCaller,
};
use std::{cell::RefCell, rc::Rc};
#[derive(Default)]
struct State {
    bytes: Vec<u8>,
    lengths: Vec<usize>,
    readings: Vec<generated::Reading>,
    ends: usize,
    failed: Vec<CallError>,
    partial: usize,
}
#[derive(Clone)]
struct Receiver(Rc<RefCell<State>>);
impl ByteConsumer for Receiver {
    fn poll_write(&mut self, _: &mut Context<'_>, bytes: &[u8]) -> Poll<usize> {
        let mut state = self.0.borrow_mut();
        let n = if state.partial == 0 {
            bytes.len()
        } else {
            bytes.len().min(state.partial)
        };
        state.lengths.push(bytes.len());
        state.bytes.extend_from_slice(&bytes[..n]);
        Poll::Ready(n)
    }
    fn end(&mut self) {
        self.0.borrow_mut().ends += 1;
    }
    fn failed(&mut self, e: CallError) {
        self.0.borrow_mut().failed.push(e);
    }
}
impl Consumer for Receiver {
    type Item = generated::Reading;
    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<()> {
        Poll::Ready(())
    }
    fn accept(&mut self, item: Self::Item) {
        self.0.borrow_mut().readings.push(item);
    }
    fn end(&mut self) {
        self.0.borrow_mut().ends += 1;
    }
    fn failed(&mut self, e: CallError) {
        self.0.borrow_mut().failed.push(e);
    }
}
struct Provider(Receiver);
struct Empty;
impl ridl_rt::face::ByteSource for Empty {
    fn poll_read(
        &mut self,
        _: &mut Context<'_>,
        _: &mut [u8],
    ) -> Poll<Result<usize, ridl_rt::face::Interrupted>> {
        Poll::Ready(Ok(0))
    }
}
impl generated::docs::Provider for Provider {
    type Fetch = Empty;
    type Text = Empty;
    type Put = Receiver;
    type Note = Receiver;
    fn fetch(&mut self, _: &generated::Id) -> Empty {
        Empty
    }
    fn text(&mut self, _: &generated::Id) -> Empty {
        Empty
    }
    fn put(&mut self, _: &generated::Info) -> Receiver {
        self.0.clone()
    }
    fn note(&mut self, _: &generated::DocsNoteParams) -> Receiver {
        self.0.clone()
    }
}
struct Readings;
impl ridl_rt::face::Source for Readings {
    type Item = generated::Reading;
    fn poll_next(
        &mut self,
        _: &mut Context<'_>,
    ) -> Poll<Option<Result<Self::Item, ridl_rt::face::Interrupted>>> {
        Poll::Ready(None)
    }
}
impl generated::log::Provider for Provider {
    type History = Readings;
    type Upload = Receiver;
    fn history(&mut self, _: &generated::Level) -> Readings {
        Readings
    }
    fn clear(&mut self, _: &generated::Level) {}
    fn upload(&mut self, _: &generated::LogUploadParams) -> Receiver {
        self.0.clone()
    }
}
fn poll<F: Future + Unpin>(f: &mut F) -> Poll<F::Output> {
    Pin::new(f).poll(&mut Context::from_waker(&ridl_rt::task::noop_waker()))
}
fn reading(n: i64) -> generated::Reading {
    generated::Reading {
        level: generated::Level::new_unchecked(n),
        flag: false,
    }
}
#[test]
fn a_200_kb_put_reaches_the_byte_consumer_in_chunks_of_at_most_the_limit() {
    let rt = Loopback::<FlatBuffers>::new(*<generated::Docs as Interface>::CATALOG);
    let state = Rc::new(RefCell::new(State::default()));
    let mut provider = Provider(Receiver(state.clone()));
    let mut serve = generated::docs::serve(rt.handler(), &mut provider);
    let mut client = generated::docs::Client::new(rt.caller());
    let (mut call, mut sink) = client.put(generated::Info {
        id: generated::Id::new_unchecked(1),
    });
    assert!(poll(&mut serve).is_pending());
    assert_eq!(poll(&mut call), Poll::Ready(Ok(())));
    let input: Vec<_> = (0..204800).map(|n| (n % 251) as u8).collect();
    let mut offset = 0;
    for _ in 0..1000 {
        if offset == input.len() {
            break;
        }
        let w = ridl_rt::task::noop_waker();
        let mut cx = Context::from_waker(&w);
        if let Poll::Ready(Ok(n)) = sink.poll_write(&mut cx, &input[offset..]) {
            offset += n;
        }
        assert!(poll(&mut serve).is_pending());
    }
    assert_eq!(offset, input.len());
    assert_eq!(poll(&mut Box::pin(sink.end())), Poll::Ready(Ok(())));
    for _ in 0..20 {
        assert!(poll(&mut serve).is_pending());
    }
    let s = state.borrow();
    assert_eq!(s.bytes, input);
    assert_eq!(s.ends, 1);
    assert!(
        s.lengths
            .iter()
            .all(|n| *n <= generated::docs::PutSink::<ridl_loopback::CallerHandle>::LIMIT)
    );
}
#[test]
fn a_zero_byte_proto3_element_is_delivered_once() {
    let value = reading(0);
    let mut storage = <generated::Reading as Payload<Proto3>>::BUFFER;
    assert_eq!(
        Ref::<generated::Reading, Proto3>::encode(&value, &mut storage)
            .unwrap()
            .bytes()
            .len(),
        0,
        "the test exercises a zero-byte encoding"
    );
    let rt = Loopback::<Proto3>::with_encoding(*<generated::Log as Interface>::CATALOG);
    let state = Rc::new(RefCell::new(State::default()));
    let mut provider = Provider(Receiver(state.clone()));
    let mut serve = generated::log::serve(rt.handler(), &mut provider);
    let mut client = generated::log::Client::new(rt.caller());
    let (mut call, mut sink) = client.upload(generated::LogUploadParams {});
    assert!(poll(&mut serve).is_pending());
    assert_eq!(poll(&mut call), Poll::Ready(Ok(())));
    assert_eq!(sink.send(&reading(0)), Ok(()));
    for _ in 0..8 {
        assert_eq!(poll(&mut Box::pin(sink.ready())), Poll::Ready(Ok(())));
        assert!(poll(&mut serve).is_pending());
    }
    assert_eq!(poll(&mut Box::pin(sink.end())), Poll::Ready(Ok(())));
    assert!(poll(&mut serve).is_pending());
    assert_eq!(state.borrow().readings, vec![reading(0)]);
    assert_eq!(state.borrow().ends, 1);
}

#[allow(dead_code)]
mod support;
use support::doubles::{self, Op, RecordingPorts};
fn cx_write<F>(f: impl FnOnce(&mut Context<'_>) -> F) -> F {
    let w = ridl_rt::task::noop_waker();
    f(&mut Context::from_waker(&w))
}
fn pending<F: Future + Unpin>(f: &mut F) {
    assert!(poll(f).is_pending());
}
fn correlation(log: &doubles::Log) -> ridl_rt::port::Correlation {
    log.borrow()
        .iter()
        .find_map(|op| {
            if let Op::Command(Ok(c)) = op {
                Some(*c)
            } else {
                None
            }
        })
        .expect("sent command")
}
fn docs() -> Loopback {
    Loopback::new(*<generated::Docs as Interface>::CATALOG)
}
fn log_rt() -> Loopback {
    Loopback::new(*<generated::Log as Interface>::CATALOG)
}
fn state() -> (Rc<RefCell<State>>, Provider) {
    let s = Rc::new(RefCell::new(State::default()));
    (s.clone(), Provider(Receiver(s)))
}
fn info() -> generated::Info {
    generated::Info {
        id: generated::Id::new_unchecked(1),
    }
}
#[test]
fn a_byte_consumer_that_takes_part_of_a_chunk_is_offered_the_rest_first() {
    let rt = docs();
    let (s, mut p) = state();
    s.borrow_mut().partial = 2;
    let mut serve = generated::docs::serve(rt.handler(), &mut p);
    let mut client = generated::docs::Client::new(rt.caller());
    let (mut call, mut sink) = client.put(info());
    pending(&mut serve);
    assert_eq!(poll(&mut call), Poll::Ready(Ok(())));
    assert_eq!(
        cx_write(|cx| sink.poll_write(cx, b"abcde")),
        Poll::Ready(Ok(5))
    );
    assert_eq!(
        cx_write(|cx| sink.poll_write(cx, b"FG")),
        Poll::Ready(Ok(2))
    );
    assert_eq!(cx_write(|cx| sink.poll_end(cx)), Poll::Ready(Ok(())));
    for _ in 0..12 {
        pending(&mut serve);
    }
    let s = s.borrow();
    assert_eq!(s.bytes, b"abcdeFG");
    assert_eq!(s.lengths, vec![5, 3, 1, 2]);
    assert_eq!(s.ends, 1);
}
#[test]
fn the_providers_refusal_of_the_third_element_reaches_the_next_send() {
    let rt = log_rt();
    let (s, mut p) = state();
    let mut raw = rt.caller();
    let mut serve = generated::log::serve(rt.handler(), &mut p);
    let ports = RecordingPorts::new(&rt);
    let ops = ports.log();
    let mut client = generated::log::Client::new(ports);
    let (mut call, mut sink) = client.upload(generated::LogUploadParams {});
    pending(&mut serve);
    assert_eq!(poll(&mut call), Poll::Ready(Ok(())));
    sink.send(&reading(1)).unwrap();
    sink.send(&reading(2)).unwrap();
    let mut buf = <generated::Reading as Payload<FlatBuffers>>::BUFFER;
    let encoded = Ref::<generated::Reading, FlatBuffers>::encode(&reading(200), &mut buf).unwrap();
    raw.push(
        correlation(&ops),
        ridl_rt::contract::StreamNo(1),
        encoded.bytes(),
    )
    .unwrap();
    for _ in 0..5 {
        pending(&mut serve);
    }
    let failures = s.borrow().failed.clone();
    assert_eq!(failures.len(), 1);
    assert!(matches!(
        failures[0],
        CallError::Contract(ridl_rt::error::Contract::InvalidValue(_))
    ));
    assert_eq!(sink.send(&reading(4)), Err(ClientError::Call(failures[0])));
    assert_eq!(s.borrow().readings, vec![reading(1), reading(2)]);
    assert_eq!(
        ops.borrow()
            .iter()
            .filter(|op| matches!(op, Op::Ack(_)))
            .count(),
        1,
        "no second acknowledgment read"
    );
}
#[test]
fn a_send_that_fails_its_check_aborts_the_stream_and_sends_nothing() {
    let rt = log_rt();
    let (s, mut p) = state();
    let ports = RecordingPorts::new(&rt);
    let ops = ports.log();
    let mut client = generated::log::Client::new(ports);
    let mut serve = generated::log::serve(rt.handler(), &mut p);
    let (mut call, mut sink) = client.upload(generated::LogUploadParams {});
    pending(&mut serve);
    assert!(matches!(
        sink.send(&reading(200)),
        Err(ClientError::Call(CallError::Contract(
            ridl_rt::error::Contract::InvalidValue(_)
        )))
    ));
    assert_eq!(
        poll(&mut call),
        Poll::Ready(Ok(())),
        "local stream failure preserves delivery ack"
    );
    for _ in 0..4 {
        pending(&mut serve);
    }
    assert!(s.borrow().readings.is_empty());
    assert_eq!(s.borrow().failed.len(), 1);
    assert!(!ops.borrow().iter().any(|op| matches!(op, Op::Push(..))));
    assert_eq!(
        ops.borrow()
            .iter()
            .filter(|op| matches!(op, Op::Abort(..)))
            .count(),
        1
    );
}
#[test]
fn a_send_while_an_element_is_held_is_busy_and_drops_nothing() {
    let rt = log_rt();
    let (s, mut p) = state();
    let mut serve = generated::log::serve(rt.handler(), &mut p);
    let mut client = generated::log::Client::new(rt.caller());
    let (mut call, mut sink) = client.upload(generated::LogUploadParams {});
    pending(&mut serve);
    assert_eq!(poll(&mut call), Poll::Ready(Ok(())));
    for n in 1..=5 {
        sink.send(&reading(n)).unwrap();
    }
    assert_eq!(
        sink.send(&reading(99)),
        Err(ClientError::Call(CallError::Transport(Transport::Busy)))
    );
    assert!(cx_write(|cx| sink.poll_ready(cx)).is_pending());
    pending(&mut serve);
    assert_eq!(cx_write(|cx| sink.poll_ready(cx)), Poll::Ready(Ok(())));
    assert_eq!(cx_write(|cx| sink.poll_end(cx)), Poll::Ready(Ok(())));
    for _ in 0..10 {
        pending(&mut serve);
    }
    assert_eq!(
        s.borrow().readings,
        (1..=5).map(reading).collect::<Vec<_>>()
    );
    assert_eq!(s.borrow().ends, 1);
}
#[test]
fn a_text_sink_never_cuts_a_code_point() {
    let rt = docs();
    let (s, mut p) = state();
    let mut serve = generated::docs::serve(rt.handler(), &mut p);
    let mut client = generated::docs::Client::new(rt.caller());
    let (mut call, mut sink) = client.note(generated::DocsNoteParams {});
    pending(&mut serve);
    assert_eq!(poll(&mut call), Poll::Ready(Ok(())));
    let input = "a".to_string() + &"🦀".repeat(10000);
    let mut offset = 0;
    for _ in 0..100 {
        if offset == input.len() {
            break;
        }
        if let Poll::Ready(Ok(n)) = cx_write(|cx| sink.poll_write_str(cx, &input[offset..])) {
            assert!(n <= 16128);
            offset += n;
            assert!(input.is_char_boundary(offset));
        }
        pending(&mut serve);
    }
    assert_eq!(offset, input.len());
    assert_eq!(cx_write(|cx| sink.poll_end(cx)), Poll::Ready(Ok(())));
    for _ in 0..20 {
        pending(&mut serve);
    }
    assert_eq!(s.borrow().bytes, input.as_bytes());
    assert_eq!(s.borrow().ends, 1);
    assert!(s.borrow().failed.is_empty());
}
#[test]
fn a_sink_dropped_before_its_end_interrupts_the_consumer() {
    let rt = docs();
    let (s, mut p) = state();
    let mut serve = generated::docs::serve(rt.handler(), &mut p);
    let ports = RecordingPorts::new(&rt);
    let ops = ports.log();
    let mut client = generated::docs::Client::new(ports);
    let (mut call, sink) = client.put(info());
    pending(&mut serve);
    assert_eq!(poll(&mut call), Poll::Ready(Ok(())));
    drop(sink);
    assert_eq!(
        ops.borrow()
            .iter()
            .filter(|op| matches!(op, Op::Abort(..)))
            .count(),
        1
    );
    for _ in 0..4 {
        pending(&mut serve);
    }
    assert_eq!(
        s.borrow().failed,
        vec![CallError::Transport(Transport::Interrupted)]
    );
    assert_eq!(s.borrow().ends, 0);
    assert_eq!(doubles::forgets(&ops).len(), 1);
}
#[test]
fn a_sink_dropped_after_its_end_forgets_nothing_more() {
    let rt = docs();
    let (s, mut p) = state();
    let ports = RecordingPorts::new(&rt);
    let ops = ports.log();
    let mut client = generated::docs::Client::new(ports);
    let (mut call, mut sink) = client.put(info());
    assert_eq!(
        cx_write(|cx| sink.poll_write(cx, b"saved")),
        Poll::Ready(Ok(5))
    );
    assert_eq!(cx_write(|cx| sink.poll_end(cx)), Poll::Ready(Ok(())));
    drop(sink);
    assert!(
        doubles::forgets(&ops).is_empty(),
        "the unclaimed call stays visible"
    );
    let mut serve = generated::docs::serve(rt.handler(), &mut p);
    pending(&mut serve);
    assert_eq!(poll(&mut call), Poll::Ready(Ok(())));
    drop(call);
    for _ in 0..6 {
        pending(&mut serve);
    }
    assert_eq!(s.borrow().bytes, b"saved");
    assert_eq!(s.borrow().ends, 1);
    assert_eq!(doubles::forgets(&ops).len(), 1);
}
#[test]
fn a_second_put_while_one_runs_is_busy() {
    let rt = docs();
    let (s, mut p) = state();
    let mut serve = generated::docs::serve(rt.handler(), &mut p);
    let mut a = generated::docs::Client::new(rt.caller());
    let (mut first, _sink) = a.put(info());
    pending(&mut serve);
    assert_eq!(poll(&mut first), Poll::Ready(Ok(())));
    let mut b = generated::docs::Client::new(rt.caller());
    let (mut second, _sink2) = b.put(info());
    pending(&mut serve);
    assert_eq!(
        poll(&mut second),
        Poll::Ready(Err(ClientError::Call(CallError::Transport(
            Transport::Busy
        ))))
    );
    assert_eq!(s.borrow().ends, 0);
}
#[test]
fn empty_byte_and_text_writes_make_no_port_call() {
    let rt = docs();
    let ports = RecordingPorts::new(&rt);
    let ops = ports.log();
    let mut client = generated::docs::Client::new(ports);
    {
        let (_call, mut sink) = client.put(info());
        doubles::take(&ops);
        assert_eq!(cx_write(|cx| sink.poll_write(cx, b"")), Poll::Ready(Ok(0)));
        assert_eq!(cx_write(|cx| sink.poll_ready(cx)), Poll::Ready(Ok(())));
        assert!(doubles::take(&ops).is_empty());
    }
    let (_call, mut sink) = client.note(generated::DocsNoteParams {});
    doubles::take(&ops);
    assert_eq!(
        cx_write(|cx| sink.poll_write_str(cx, "")),
        Poll::Ready(Ok(0))
    );
    assert!(doubles::take(&ops).is_empty());
}

const CATALOG: ridl_rt::contract::CatalogRef = *<generated::Log as Interface>::CATALOG;

#[test]
fn an_unsent_upload_does_not_retry_after_its_bound() {
    let mut rt = docs();
    let ports = RecordingPorts::new(&rt);
    let ops = ports.log();
    let faults = ports.faults();
    faults.send.set(Some(ridl_rt::port::SendError::Busy));
    let mut client = generated::docs::Client::new(ports);
    let (mut call, _sink) = client.put(info());
    rt.advance(ridl_rt::sample::Duration(1_000_001));
    assert_eq!(
        poll(&mut call),
        Poll::Ready(Err(ClientError::Send(ridl_rt::port::SendError::Busy)))
    );
    assert_eq!(
        ops.borrow()
            .iter()
            .filter(|op| matches!(op, Op::Command(_)))
            .count(),
        1,
        "no retry after expiry"
    );
}
#[test]
fn dropping_serve_aborts_the_upload_stream() {
    let rt = docs();
    let (s, mut p) = state();
    let mut client = generated::docs::Client::new(rt.caller());
    let (mut call, mut sink) = client.put(info());
    let mut handler = rt.handler();
    let mut serve = generated::docs::serve(&mut handler, &mut p);
    pending(&mut serve);
    assert_eq!(poll(&mut call), Poll::Ready(Ok(())));
    drop(serve);
    assert_eq!(
        s.borrow().failed,
        vec![CallError::Transport(Transport::Interrupted)]
    );
    assert_eq!(
        cx_write(|cx| sink.poll_write(cx, b"next")),
        Poll::Ready(Err(ClientError::Call(CallError::Transport(
            Transport::Interrupted
        ))))
    );
}
#[test]
fn dropping_the_call_before_acknowledgment_cancels_the_sink_once() {
    let rt = docs();
    let ports = RecordingPorts::new(&rt);
    let ops = ports.log();
    let mut client = generated::docs::Client::new(ports);
    let (call, mut sink) = client.put(info());
    drop(call);
    assert_eq!(
        doubles::forgets(&ops).len(),
        1,
        "the call cleans up immediately"
    );
    assert_eq!(
        cx_write(|cx| sink.poll_write(cx, b"next")),
        Poll::Ready(Err(ClientError::Call(CallError::Transport(
            Transport::Interrupted
        ))))
    );
    drop(sink);
    assert_eq!(doubles::forgets(&ops).len(), 1);
}
#[test]
fn a_full_byte_window_registers_a_stream_waker() {
    let rt = docs();
    let ports = RecordingPorts::new(&rt);
    let ops = ports.log();
    let mut client = generated::docs::Client::new(ports);
    let (_call, mut sink) = client.put(info());
    for _ in 0..4 {
        assert_eq!(cx_write(|cx| sink.poll_write(cx, b"x")), Poll::Ready(Ok(1)));
    }
    doubles::take(&ops);
    assert!(cx_write(|cx| sink.poll_write(cx, b"y")).is_pending());
    let ops = doubles::take(&ops);
    assert!(matches!(
        ops.as_slice(),
        [
            Op::WakeOn(ridl_rt::port::Interest::Stream(
                _,
                ridl_rt::contract::StreamNo(1)
            )),
            Op::Push(_, ridl_rt::contract::StreamNo(1))
        ]
    ));
}

#[test]
fn dropping_a_locally_failed_sink_does_not_abort_again() {
    let rt = log_rt();
    let ports = RecordingPorts::new(&rt);
    let ops = ports.log();
    let mut client = generated::log::Client::new(ports);
    let (_call, mut sink) = client.upload(generated::LogUploadParams {});
    assert!(sink.send(&reading(200)).is_err());
    drop(sink);
    assert_eq!(
        ops.borrow()
            .iter()
            .filter(|op| matches!(op, Op::Abort(..)))
            .count(),
        1
    );
    assert_eq!(doubles::forgets(&ops).len(), 1);
}

macro_rules! cancelled_before_admission {
    ($name:ident, $module:ident, $method:ident, $args:expr) => {
        #[test]
        fn $name() {
            let rt = docs();
            let ports = RecordingPorts::new(&rt);
            let ops = ports.log();
            let faults = ports.faults();
            faults.send.set(Some(ridl_rt::port::SendError::Busy));
            let mut client = generated::$module::Client::new(ports);
            let (mut call, sink) = client.$method($args);
            assert!(matches!(
                ops.borrow().as_slice(),
                [Op::Command(Err(ridl_rt::port::SendError::Busy))]
            ));
            drop(sink);
            faults.send.set(None);
            assert_eq!(
                poll(&mut call),
                Poll::Ready(Err(ClientError::Call(CallError::Transport(
                    Transport::Interrupted
                ))))
            );
            assert_eq!(
                ops.borrow()
                    .iter()
                    .filter(|op| matches!(op, Op::Command(_)))
                    .count(),
                1,
                "a cancelled request must never be retried"
            );
            drop(call);
            assert!(
                doubles::forgets(&ops).is_empty(),
                "an unadmitted call has no correlation to forget"
            );
        }
    };
}
cancelled_before_admission!(
    a_sink_dropped_before_admission_cancels_the_pending_call,
    docs,
    put,
    info()
);
cancelled_before_admission!(
    a_named_sink_dropped_before_admission_cancels_the_pending_call,
    log,
    upload,
    generated::LogUploadParams {}
);
cancelled_before_admission!(
    a_text_sink_dropped_before_admission_cancels_the_pending_call,
    docs,
    note,
    generated::DocsNoteParams {}
);
