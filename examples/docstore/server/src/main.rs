//! Serves the generated document store face through the WebSocket binding.
//!
//! Binds loopback port zero, announces port/hash, and exits when stdin ends.

use std::future::Future;
use std::io::{self, Write};
use std::net::TcpListener;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Waker};
use std::thread;
use std::time::Duration;

use docstore_api::docstore::api;
use ridl_loopback::Loopback;
use ridl_rt::contract::{CatalogRef, Interface};
use ridl_transport_ws::{Config, Described, MemberTable, Scanning, Streaming, serve};

mod documents;
use documents::Documents;

const CATALOG: CatalogRef = *<api::DocStore as Interface>::CATALOG;
const DESCRIPTOR: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../generated/docstore.api.catalog.binfb"
));

fn main() {
    let runtime = Loopback::new(CATALOG);
    let table =
        Arc::new(MemberTable::from_descriptor(DESCRIPTOR).expect("the descriptor verifies"));
    let mut handler = runtime.handler();
    thread::spawn(move || {
        let mut provider = Documents::default();
        let mut serving = api::doc_store::serve(&mut handler, &mut provider);
        let mut cx = Context::from_waker(Waker::noop());
        loop {
            assert!(Pin::new(&mut serving).poll(&mut cx).is_pending());
            thread::sleep(Duration::from_millis(1));
        }
    });
    let listener = TcpListener::bind("127.0.0.1:0").expect("bound");
    println!("port {}", listener.local_addr().expect("an address").port());
    let hash: String = CATALOG
        .hash
        .0
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    println!("hash {hash}");
    io::stdout().flush().expect("flushed");
    thread::spawn(|| {
        let _ = io::copy(&mut io::stdin(), &mut io::sink());
        std::process::exit(0);
    });
    for connection in listener.incoming() {
        let Ok(connection) = connection else { continue };
        let backend = Described::new(Streaming(Scanning(runtime.attach())), Arc::clone(&table))
            .expect("the descriptor matches the runtime's catalog");
        thread::spawn(move || serve(connection, backend, &Config::default()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use api::doc_store::Provider;
    use documents::Exchange;
    use ridl_rt::face::{ByteConsumer, ByteSource, Consumer, Interrupted, Source};
    use std::task::Poll;

    #[test]
    fn exchange_echoes_before_end_with_bounded_buffering() {
        let mut exchange = Exchange::default();
        let mut cx = Context::from_waker(std::task::Waker::noop());
        assert_eq!(exchange.poll_next(&mut cx), Poll::Pending);
        for index in 0..64 {
            let chunk = api::DocChunk {
                index: api::ChunkIndex::new(index).unwrap(),
                data: api::DocBlock::new(vec![index as u8; 3200]).unwrap(),
            };
            assert_eq!(exchange.poll_ready(&mut cx), Poll::Ready(()));
            exchange.accept(chunk.clone());
            assert_eq!(exchange.poll_ready(&mut cx), Poll::Pending);
            assert_eq!(exchange.poll_next(&mut cx), Poll::Ready(Some(Ok(chunk))));
            assert_eq!(exchange.poll_next(&mut cx), Poll::Pending);
        }
        exchange.end();
        assert_eq!(exchange.poll_next(&mut cx), Poll::Ready(None));
        let mut failed = Exchange::default();
        failed.failed(ridl_rt::error::CallError::Transport(
            ridl_rt::error::Transport::Interrupted,
        ));
        assert_eq!(
            failed.poll_next(&mut cx),
            Poll::Ready(Some(Err(Interrupted)))
        );
    }

    #[test]
    fn download_waits_for_upload_end() {
        let mut documents = Documents::default();
        let id = api::DocId::new("manual".into()).unwrap();
        let info = api::DocInfo {
            name: id.clone(),
            size: api::DocSize::new(6400).unwrap(),
        };
        let mut cx = Context::from_waker(std::task::Waker::noop());
        let mut upload = documents.put(&info);
        let bytes: Vec<u8> = (0..6400).map(|i| (i % 251) as u8).collect();
        assert_eq!(
            upload.poll_write(&mut cx, &bytes[..3200]),
            Poll::Ready(3200)
        );
        let mut fetched = documents.fetch(&id);
        let mut chunks = documents.chunks(&id);
        let mut out = [0; 1000];
        assert_eq!(fetched.poll_read(&mut cx, &mut out), Poll::Pending);
        assert_eq!(chunks.poll_next(&mut cx), Poll::Pending);
        assert_eq!(
            upload.poll_write(&mut cx, &bytes[3200..]),
            Poll::Ready(3200)
        );
        assert_eq!(fetched.poll_read(&mut cx, &mut out), Poll::Pending);
        assert_eq!(chunks.poll_next(&mut cx), Poll::Pending);
        upload.end();
        let mut received = Vec::new();
        loop {
            let Poll::Ready(Ok(n)) = fetched.poll_read(&mut cx, &mut out) else {
                panic!("committed bytes are ready")
            };
            if n == 0 {
                break;
            }
            received.extend_from_slice(&out[..n]);
        }
        assert_eq!(received, bytes);
        for index in 0..2 {
            let Poll::Ready(Some(Ok(chunk))) = chunks.poll_next(&mut cx) else {
                panic!("committed chunks are ready")
            };
            assert_eq!(chunk.index.get(), index as i64);
            assert_eq!(chunk.data.get(), &bytes[index * 3200..(index + 1) * 3200]);
        }
        assert_eq!(chunks.poll_next(&mut cx), Poll::Ready(None));
    }
}
