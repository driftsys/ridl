//! Transfers a document through the generated Rust face over ridl-loopback.
//! The compiler integration test builds and runs this program with plain rustc.

use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use api::doc_store::{self, prelude::*};
use docstore_api::docstore::api;
use ridl_loopback::Loopback;
use ridl_rt::contract::{CatalogRef, Interface};

const CATALOG: CatalogRef = *<api::DocStore as Interface>::CATALOG;

#[path = "../../server/src/documents.rs"]
mod documents;
use documents::Documents;

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
    let mut provider = Documents::default();
    let mut observer = provider.clone();
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
            if matches!(
                doc_store::Provider::describe(&mut observer, &id),
                api::DescribeResult::Info(_)
            ) {
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
