//! Byte and text query uploads through the checked-in generated face.
#[allow(dead_code, unused_imports)]
mod generated {
    include!("generated/stream_face.rs");
}
#[allow(dead_code)]
mod support;
use core::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};
use ridl_loopback::Loopback;
use ridl_rt::{
    contract::{Interface, StreamNo},
    error::{CallError, ClientError, Contract},
    face::{Bind, ByteConsumer, Interrupted, Source},
    payload::{Rule, Violation},
    port::StreamCaller,
};
use std::{cell::RefCell, rc::Rc};
use support::doubles::{Op, RecordingPorts};
const CATALOG: ridl_rt::contract::CatalogRef = *<generated::RawQueries as Interface>::CATALOG;
#[derive(Default)]
struct State {
    bytes: Vec<u8>,
    offers: Vec<Vec<u8>>,
    ends: usize,
    failures: Vec<CallError>,
    blocked: bool,
}
#[derive(Clone)]
struct Upload(Rc<RefCell<State>>);
impl ByteConsumer for Upload {
    fn poll_write(&mut self, _: &mut Context<'_>, bytes: &[u8]) -> Poll<usize> {
        let mut s = self.0.borrow_mut();
        s.offers.push(bytes.to_vec());
        if s.blocked {
            return Poll::Pending;
        }
        let n = bytes.len().min(2);
        s.bytes.extend_from_slice(&bytes[..n]);
        Poll::Ready(n)
    }
    fn end(&mut self) {
        self.0.borrow_mut().ends += 1;
    }
    fn failed(&mut self, e: CallError) {
        self.0.borrow_mut().failures.push(e);
    }
}
impl Source for Upload {
    type Item = generated::Reading;
    fn poll_next(&mut self, _: &mut Context<'_>) -> Poll<Option<Result<Self::Item, Interrupted>>> {
        if self.0.borrow().ends > 0 {
            Poll::Ready(None)
        } else {
            Poll::Pending
        }
    }
}
struct Provider(Upload);
impl generated::raw_queries::Provider for Provider {
    type Sum = Upload;
    type Pipe = Upload;
    fn sum(&mut self, _: &generated::RawQueriesSumParams) -> Upload {
        self.0.clone()
    }
    fn sum_reply(&mut self, upload: Upload) -> generated::Summary {
        generated::Summary {
            count: upload.0.borrow().bytes.len() as i64,
            total: 0,
        }
    }
    fn pipe(&mut self, _: &generated::RawQueriesPipeParams) -> Upload {
        self.0.clone()
    }
}
fn poll<F: Future + Unpin>(f: &mut F) -> Poll<F::Output> {
    Pin::new(f).poll(&mut Context::from_waker(core::task::Waker::noop()))
}
fn cx<R>(f: impl FnOnce(&mut Context<'_>) -> R) -> R {
    f(&mut Context::from_waker(core::task::Waker::noop()))
}
#[test]
fn a_byte_query_holds_the_chunk_and_offers_each_suffix_before_end() {
    let rt = Loopback::new(*<generated::RawQueries as Interface>::CATALOG);
    let s = Rc::new(RefCell::new(State {
        blocked: true,
        ..State::default()
    }));
    let mut p = Provider(Upload(s.clone()));
    let mut serve = generated::raw_queries::serve(rt.handler(), &mut p);
    let mut client = generated::raw_queries::Client::new(rt.caller());
    let (mut call, mut sink) = client.sum(generated::RawQueriesSumParams {});
    assert!(poll(&mut serve).is_pending());
    assert_eq!(cx(|cx| sink.poll_write(cx, b"abcde")), Poll::Ready(Ok(5)));
    assert_eq!(cx(|cx| sink.poll_end(cx)), Poll::Ready(Ok(())));
    for _ in 0..3 {
        assert!(poll(&mut serve).is_pending());
    }
    assert!(!s.borrow().offers.is_empty());
    assert_eq!(s.borrow().ends, 0);
    s.borrow_mut().blocked = false;
    for _ in 0..8 {
        assert!(poll(&mut serve).is_pending());
    }
    assert_eq!(s.borrow().bytes, b"abcde");
    let offers = s.borrow().offers.clone();
    assert!(offers.ends_with(&[b"abcde".to_vec(), b"cde".to_vec(), b"e".to_vec()]));
    assert_eq!(s.borrow().ends, 1);
    assert!(s.borrow().failures.is_empty());
    assert_eq!(
        poll(&mut call),
        Poll::Ready(Ok(generated::Summary { count: 5, total: 0 }))
    );
}
#[test]
fn a_text_query_refuses_invalid_utf8_before_offering_any_prefix() {
    let rt = Loopback::new(*<generated::RawQueries as Interface>::CATALOG);
    let s = Rc::new(RefCell::new(State::default()));
    let mut p = Provider(Upload(s.clone()));
    let mut serve = generated::raw_queries::serve(rt.handler(), &mut p);
    let mut raw = rt.caller();
    let ports = RecordingPorts::new(&rt);
    let ops = ports.log();
    let mut client = generated::raw_queries::Client::new(ports);
    let (mut call, _sink) = client.pipe(generated::RawQueriesPipeParams {});
    assert!(poll(&mut serve).is_pending());
    let c = ops
        .borrow()
        .iter()
        .find_map(|op| {
            if let Op::Query(Ok(c)) = op {
                Some(*c)
            } else {
                None
            }
        })
        .unwrap();
    assert_eq!(raw.push(c, StreamNo(1), b"a\xff"), Ok(2));
    for _ in 0..4 {
        assert!(poll(&mut serve).is_pending());
    }
    let error = CallError::Contract(Contract::InvalidValue(Violation {
        type_name: Some("string"),
        rule: Rule::Utf8,
    }));
    assert_eq!(s.borrow().failures, vec![error]);
    assert!(s.borrow().offers.is_empty());
    assert!(s.borrow().bytes.is_empty());
    assert_eq!(s.borrow().ends, 0);
    let mut reply = match poll(&mut call) {
        Poll::Ready(Ok(reply)) => reply,
        _ => panic!("terminal reply"),
    };
    assert_eq!(
        cx(|cx| reply.poll_next(cx)),
        Poll::Ready(Some(Err(ClientError::Call(error))))
    );
    assert_eq!(cx(|cx| reply.poll_next(cx)), Poll::Ready(None));
}
