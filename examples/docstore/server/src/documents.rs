//! The generated provider used by the WebSocket server and Rust consumer.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;
use std::task::{Context, Poll, Waker};

use docstore_api::docstore::api;
use ridl_rt::error::CallError;
use ridl_rt::face::{ByteConsumer, ByteSource, Consumer, Interrupted, Source};

type Shared = Rc<RefCell<Document>>;

struct Document {
    info: api::DocInfo,
    bytes: Option<Vec<u8>>,
    pending: bool,
    readers: Vec<Waker>,
}

impl Document {
    fn wake(&mut self) {
        for waker in self.readers.drain(..) {
            waker.wake();
        }
    }
}

#[derive(Clone, Default)]
pub struct Documents(Rc<RefCell<BTreeMap<String, Shared>>>);

pub struct Upload {
    document: Shared,
    bytes: Vec<u8>,
}

impl ByteConsumer for Upload {
    fn poll_write(&mut self, _: &mut Context<'_>, bytes: &[u8]) -> Poll<usize> {
        self.bytes.extend_from_slice(bytes);
        Poll::Ready(bytes.len())
    }

    fn end(&mut self) {
        let mut doc = self.document.borrow_mut();
        // Only the provider's explicit end makes the uploaded bytes visible.
        doc.bytes = (doc.info.size.get() as u64 == self.bytes.len() as u64)
            .then(|| std::mem::take(&mut self.bytes));
        doc.pending = false;
        doc.wake();
    }

    fn failed(&mut self, _: CallError) {
        let mut doc = self.document.borrow_mut();
        doc.pending = false;
        doc.wake();
    }
}

pub struct Download {
    document: Option<Shared>,
    bytes: Option<Vec<u8>>,
    position: usize,
}

impl Download {
    fn snapshot(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Interrupted>> {
        if self.bytes.is_some() {
            return Poll::Ready(Ok(()));
        }
        let Some(document) = &self.document else {
            return Poll::Ready(Err(Interrupted));
        };
        let mut doc = document.borrow_mut();
        if doc.pending {
            if !doc.readers.iter().any(|w| w.will_wake(cx.waker())) {
                doc.readers.push(cx.waker().clone());
            }
            return Poll::Pending;
        }
        self.bytes = doc.bytes.clone();
        Poll::Ready(self.bytes.as_ref().map(|_| ()).ok_or(Interrupted))
    }
}

impl ByteSource for Download {
    fn poll_read(
        &mut self,
        cx: &mut Context<'_>,
        out: &mut [u8],
    ) -> Poll<Result<usize, Interrupted>> {
        std::task::ready!(self.snapshot(cx))?;
        let bytes = self.bytes.as_ref().expect("the committed snapshot exists");
        let n = out.len().min(bytes.len() - self.position);
        out[..n].copy_from_slice(&bytes[self.position..self.position + n]);
        self.position += n;
        Poll::Ready(Ok(n))
    }
}

impl Source for Download {
    type Item = api::DocChunk;

    fn poll_next(&mut self, cx: &mut Context<'_>) -> Poll<Option<Result<Self::Item, Interrupted>>> {
        if let Err(error) = std::task::ready!(self.snapshot(cx)) {
            return Poll::Ready(Some(Err(error)));
        }
        let bytes = self.bytes.as_ref().expect("the committed snapshot exists");
        if self.position == bytes.len() {
            return Poll::Ready(None);
        }
        let end = (self.position + 3200).min(bytes.len());
        let Ok(index) = api::ChunkIndex::new((self.position / 3200) as i64) else {
            return Poll::Ready(Some(Err(Interrupted)));
        };
        let data = api::DocBlock::new(bytes[self.position..end].to_vec())
            .expect("a nonempty block of at most 3200 bytes");
        self.position = end;
        Poll::Ready(Some(Ok(api::DocChunk { index, data })))
    }
}

/// Holds at most one echo while the reply is waiting for credit.
#[derive(Default)]
pub struct Exchange {
    next: Option<api::DocChunk>,
    ended: bool,
    failed: bool,
    reader: Option<Waker>,
    writer: Option<Waker>,
}

impl Consumer for Exchange {
    type Item = api::DocChunk;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<()> {
        if self.next.is_some() {
            self.writer = Some(cx.waker().clone());
            Poll::Pending
        } else {
            Poll::Ready(())
        }
    }

    fn accept(&mut self, item: Self::Item) {
        self.next = Some(item);
        if let Some(waker) = self.reader.take() {
            waker.wake();
        }
    }

    fn end(&mut self) {
        self.ended = true;
        if let Some(waker) = self.reader.take() {
            waker.wake();
        }
    }

    fn failed(&mut self, _: CallError) {
        self.failed = true;
        self.next = None;
        if let Some(waker) = self.reader.take() {
            waker.wake();
        }
    }
}

impl Source for Exchange {
    type Item = api::DocChunk;

    fn poll_next(&mut self, cx: &mut Context<'_>) -> Poll<Option<Result<Self::Item, Interrupted>>> {
        if self.failed {
            return Poll::Ready(Some(Err(Interrupted)));
        }
        if let Some(item) = self.next.take() {
            if let Some(waker) = self.writer.take() {
                waker.wake();
            }
            return Poll::Ready(Some(Ok(item)));
        }
        if self.ended {
            return Poll::Ready(None);
        }
        self.reader = Some(cx.waker().clone());
        Poll::Pending
    }
}

impl api::doc_store::Provider for Documents {
    type Put = Upload;
    type Fetch = Download;
    type Chunks = Download;
    type Exchange = Exchange;

    fn put(&mut self, info: &api::DocInfo) -> Self::Put {
        let document = Rc::new(RefCell::new(Document {
            info: info.clone(),
            bytes: None,
            pending: true,
            readers: Vec::new(),
        }));
        self.0
            .borrow_mut()
            .insert(info.name.get().to_owned(), document.clone());
        Upload {
            document,
            bytes: Vec::new(),
        }
    }

    fn fetch(&mut self, id: &api::DocId) -> Self::Fetch {
        Download {
            document: self.0.borrow().get(id.get()).cloned(),
            bytes: None,
            position: 0,
        }
    }

    fn chunks(&mut self, id: &api::DocId) -> Self::Chunks {
        self.fetch(id)
    }

    fn exchange(&mut self, _: &api::DocId) -> Self::Exchange {
        Exchange::default()
    }

    fn describe(&mut self, id: &api::DocId) -> api::DescribeResult {
        match self.0.borrow().get(id.get()) {
            Some(doc) if doc.borrow().bytes.is_some() => {
                api::DescribeResult::Info(doc.borrow().info.clone())
            }
            _ => api::DescribeResult::Failure(api::DocError::Missing),
        }
    }
}
