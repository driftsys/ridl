//! Transfers a document through the generated Rust face over ridl-loopback.
//! The compiler integration test builds and runs this program with plain rustc.

use std::cell::RefCell;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::task::{Context, Poll};

use api::doc_store::{self, prelude::*};
use docstore::docstore as api;
use ridl_loopback::Loopback;
use ridl_rt::contract::{CatalogRef, Interface};
use ridl_rt::error::CallError;
use ridl_rt::face::{ByteConsumer, ByteSource, Interrupted};

const CATALOG: CatalogRef = *<api::DocStore as Interface>::CATALOG;

struct Document {
    info: api::DocInfo,
    bytes: Vec<u8>,
}

type Store = Rc<RefCell<Option<Document>>>;

struct Documents(Store);

/// Collects the upload separately and publishes it only after its explicit end.
struct Upload {
    store: Store,
    info: api::DocInfo,
    bytes: Vec<u8>,
}

impl ByteConsumer for Upload {
    fn poll_write(&mut self, _: &mut Context<'_>, bytes: &[u8]) -> Poll<usize> {
        self.bytes.extend_from_slice(bytes);
        Poll::Ready(bytes.len())
    }

    fn end(&mut self) {
        assert_eq!(self.info.size.get() as usize, self.bytes.len());
        *self.store.borrow_mut() = Some(Document {
            info: self.info.clone(),
            bytes: std::mem::take(&mut self.bytes),
        });
    }

    fn failed(&mut self, error: CallError) {
        panic!("document upload failed: {error:?}");
    }
}

struct Download {
    bytes: Vec<u8>,
    position: usize,
}

impl ByteSource for Download {
    fn poll_read(
        &mut self,
        _: &mut Context<'_>,
        out: &mut [u8],
    ) -> Poll<Result<usize, Interrupted>> {
        let n = out.len().min(self.bytes.len() - self.position);
        out[..n].copy_from_slice(&self.bytes[self.position..self.position + n]);
        self.position += n;
        Poll::Ready(Ok(n))
    }
}

impl doc_store::Provider for Documents {
    type Put = Upload;
    type Fetch = Download;

    fn put(&mut self, info: &api::DocInfo) -> Self::Put {
        Upload {
            store: self.0.clone(),
            info: info.clone(),
            bytes: Vec::new(),
        }
    }

    fn describe(&mut self, id: &api::DocId) -> api::DescribeResult {
        match self.0.borrow().as_ref().filter(|doc| &doc.info.name == id) {
            Some(doc) => api::DescribeResult::Info(doc.info.clone()),
            None => api::DescribeResult::Failure(api::DocError::Missing),
        }
    }

    fn fetch(&mut self, id: &api::DocId) -> Self::Fetch {
        let stored = self.0.borrow();
        let doc = stored
            .as_ref()
            .filter(|doc| &doc.info.name == id)
            .expect("the requested document exists");
        Download {
            bytes: doc.bytes.clone(),
            position: 0,
        }
    }
}

/// Drives the caller and provider together, with a finite limit on progress.
fn complete<T>(mut step: impl FnMut() -> Poll<T>) -> T {
    for _ in 0..100_000 {
        if let Poll::Ready(value) = step() {
            return value;
        }
    }
    panic!("document transfer did not complete");
}

fn main() {
    let rt = Loopback::new(CATALOG);
    let mut client = doc_store::Client::new(rt.attach());
    let mut handler = rt.handler();
    let store = Rc::new(RefCell::new(None));
    let mut provider = Documents(store.clone());
    let mut serve = doc_store::serve(&mut handler, &mut provider);
    let waker = std::task::Waker::noop();
    let mut cx = Context::from_waker(waker);
    let id = api::DocId::new("manual".into()).unwrap();
    let bytes: Vec<u8> = (0..204_800).map(|i| (i % 251) as u8).collect();
    let info = api::DocInfo {
        name: id.clone(),
        size: api::DocSize::new(bytes.len() as i64).unwrap(),
    };

    {
        let (mut call, mut sink) = client.put(info);
        complete(|| {
            assert!(Pin::new(&mut serve).poll(&mut cx).is_pending());
            Pin::new(&mut call).poll(&mut cx)
        })
        .unwrap();
        let mut offset = 0;
        while offset < bytes.len() {
            let n = complete(|| {
                assert!(Pin::new(&mut serve).poll(&mut cx).is_pending());
                sink.poll_write(&mut cx, &bytes[offset..])
            })
            .unwrap();
            assert!(n > 0);
            offset += n;
        }
        complete(|| {
            assert!(Pin::new(&mut serve).poll(&mut cx).is_pending());
            sink.poll_end(&mut cx)
        })
        .unwrap();
        // A command acknowledges admission before its upload has been consumed.
        complete(|| {
            assert!(Pin::new(&mut serve).poll(&mut cx).is_pending());
            if store.borrow().is_some() {
                Poll::Ready(())
            } else {
                Poll::Pending
            }
        });
        println!("docstore put: {offset} bytes");
    }

    {
        let mut call = client.describe(id.clone());
        let described = complete(|| {
            assert!(Pin::new(&mut serve).poll(&mut cx).is_pending());
            Pin::new(&mut call).poll(&mut cx)
        })
        .unwrap();
        let api::DescribeResult::Info(info) = described else {
            panic!("the uploaded document must exist");
        };
        assert_eq!(info.name, id);
        assert_eq!(info.size.get(), 204_800);
        println!("docstore describe: {} bytes", info.size.get());
    }

    {
        let mut call = client.fetch(id);
        let mut stream = complete(|| {
            assert!(Pin::new(&mut serve).poll(&mut cx).is_pending());
            Pin::new(&mut call).poll(&mut cx)
        })
        .unwrap();
        let mut fetched = Vec::new();
        let mut buffer = [0u8; 1_000];
        loop {
            let n = complete(|| {
                assert!(Pin::new(&mut serve).poll(&mut cx).is_pending());
                stream.poll_read(&mut cx, &mut buffer)
            })
            .unwrap();
            if n == 0 {
                break;
            }
            fetched.extend_from_slice(&buffer[..n]);
        }
        assert_eq!(fetched, bytes);
        println!("docstore round trip: {} bytes", fetched.len());
    }
}
