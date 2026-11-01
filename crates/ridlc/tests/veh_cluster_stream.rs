//! The round trip of a streamed reply over the crate that `ridl build` writes
//! for the `veh-cluster` corpus.
//!
//! `FaultLog::faults` replies with a stream of `FaultResult`. The test builds
//! the corpus, compiles the emitted crate with plain `rustc` the way
//! `cabin_example.rs` does, and compiles a program against it that serves 64
//! elements through the generated `serve` over `ridl-loopback` and reads them
//! back through the generated `Client`. The elements carry more bytes in
//! FlatBuffers than any one call may carry, so the reply can only arrive as a
//! stream. The program checks the order of the elements and the end of the
//! stream, and it prints the encoded size, which the test reads.

use std::path::Path;

use ridlc::Emit;

mod support;

/// The corpus the build compiles.
fn corpus_dir() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/corpus/veh-cluster")
}

/// The program that serves and reads the stream. It names the generated face
/// and the loopback and nothing of the compiler that produced the crate.
const PROGRAM: &str = r#"
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use ridl_loopback::Loopback;
use ridl_rt::contract::{CatalogRef, Interface, Sizes};
use ridl_rt::encoding::FlatBuffers;
use ridl_rt::face::{Interrupted, Source};
use ridl_rt::payload::Payload;
use ridl_rt::task::noop_waker;
use veh_cluster::ridl::std::{Message, Timestamp};
use veh_cluster::veh::cluster::fault_log::prelude::*;
use veh_cluster::veh::cluster::{self as api, DiagFilter, FaultEvent, FaultPage, FaultResult};

const CATALOG: CatalogRef = *<api::FaultLog as Interface>::CATALOG;
const COUNT: usize = 64;

fn event(index: usize, line: usize) -> FaultEvent {
    FaultEvent {
        code: i64::try_from(index * 100 + line).expect("a small number"),
        message: Message::new(format!("fault {index}.{line}: {}", "x".repeat(37)))
            .expect("a legal message"),
        timestamp: Timestamp::new(i64::try_from(index * 1000 + line).expect("a small number"))
            .expect("a legal timestamp"),
    }
}

/// Every fourth element is a single event; the others are a full page.
fn element(index: usize) -> FaultResult {
    if index % 4 == 3 {
        FaultResult::Single(event(index, 0))
    } else {
        FaultResult::Page(FaultPage {
            faults: (0..14).map(|line| event(index, line)).collect(),
        })
    }
}

struct Replay {
    next: usize,
}

impl Source for Replay {
    type Item = FaultResult;

    fn poll_next(&mut self, _cx: &mut Context<'_>) -> Poll<Option<Result<FaultResult, Interrupted>>> {
        if self.next == COUNT {
            return Poll::Ready(None);
        }
        self.next += 1;
        Poll::Ready(Some(Ok(element(self.next - 1))))
    }
}

struct Log;

impl api::fault_log::Provider for Log {
    type Faults = Replay;

    fn faults(&mut self, _filter: &DiagFilter) -> Replay {
        Replay { next: 0 }
    }
}

fn main() {
    let waker = noop_waker();
    let mut cx = Context::from_waker(&waker);
    let runtime = Loopback::new(CATALOG);
    let mut log = Log;
    let mut serve = api::fault_log::serve(runtime.handler(), &mut log);
    let mut client = api::fault_log::Client::new(runtime.caller());

    // The size of the elements in FlatBuffers, summed, against the largest
    // buffer one call of the interface may carry.
    let mut total = 0usize;
    for index in 0..COUNT {
        let mut buffer = vec![0u8; <FaultResult as Payload<FlatBuffers>>::MAX_SIZE];
        total += <FaultResult as Payload<FlatBuffers>>::encode(&element(index), &mut buffer)
            .expect("an element encodes")
            .bytes
            .len();
    }
    let call_limit = <api::FaultLog as Sizes<FlatBuffers>>::MAX_BUFFER_SIZE;
    assert!(total > call_limit, "the elements ({total} bytes) must exceed one call ({call_limit} bytes)");
    println!("encoded bytes {total}");

    let mut call = client.faults(DiagFilter { severity: 0, category: None });
    let mut stream = None;
    let mut received: Vec<FaultResult> = Vec::new();
    let mut ended = false;
    for _ in 0..10_000 {
        let _ = Pin::new(&mut serve).poll(&mut cx);
        match stream.as_mut() {
            None => {
                if let Poll::Ready(outcome) = Pin::new(&mut call).poll(&mut cx) {
                    stream = Some(outcome.expect("the call resolves to the stream"));
                }
            }
            Some(stream) => loop {
                match stream.poll_next(&mut cx) {
                    Poll::Ready(Some(item)) => received.push(item.expect("an element")),
                    Poll::Ready(None) => {
                        ended = true;
                        break;
                    }
                    Poll::Pending => break,
                }
            },
        }
        if ended {
            break;
        }
    }
    assert!(ended, "the stream must end");
    assert_eq!(received.len(), COUNT, "every element arrives");
    for (index, item) in received.iter().enumerate() {
        assert_eq!(item, &element(index), "element {index} arrives in order and intact");
    }
    println!("faults ok {}", received.len());
}
"#;

#[test]
fn a_streamed_reply_above_one_call_limit_arrives_in_order_over_the_loopback() {
    let out_dir = tempfile::tempdir().expect("a temp dir is created");
    let libs_dir = tempfile::tempdir().expect("a temp dir is created");
    let out = out_dir.path();
    let libs = libs_dir.path();

    let run =
        ridlc::run_build(&corpus_dir(), out, &[Emit::Rust], false.into()).expect("the build runs");
    assert!(!run.has_error(), "the corpus must compile without an error");

    let ridl_rt = support::ridl_rt_rlib(libs);
    let ridl_loopback = support::ridl_loopback_rlib(libs, &ridl_rt);
    let dependency = format!("dependency={}", libs.display());

    let veh_cluster = libs.join("libveh_cluster.rlib");
    let output = std::process::Command::new("rustc")
        .args(["--edition", "2024", "--crate-type", "rlib"])
        .args(["--crate-name", "veh_cluster", "--cfg", r#"feature="std""#])
        .arg("--extern")
        .arg(format!("ridl_rt={}", ridl_rt.display()))
        .args(["-L", &dependency])
        .arg(out.join("lib.rs"))
        .arg("-o")
        .arg(&veh_cluster)
        .output()
        .expect("rustc runs");
    assert!(
        output.status.success(),
        "the emitted crate must compile, rustc said:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let source = libs.join("faults.rs");
    std::fs::write(&source, PROGRAM).expect("the program is written");
    let program = libs.join("faults");
    let output = std::process::Command::new("rustc")
        .args(["--edition", "2024", "--crate-name", "faults"])
        .arg("--extern")
        .arg(format!("veh_cluster={}", veh_cluster.display()))
        .arg("--extern")
        .arg(format!("ridl_rt={}", ridl_rt.display()))
        .arg("--extern")
        .arg(format!("ridl_loopback={}", ridl_loopback.display()))
        .args(["-L", &dependency])
        .arg(&source)
        .arg("-o")
        .arg(&program)
        .output()
        .expect("rustc runs");
    assert!(
        output.status.success(),
        "the program must compile, rustc said:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let run = std::process::Command::new(&program)
        .output()
        .expect("the program runs");
    let stdout = String::from_utf8_lossy(&run.stdout);
    assert!(
        run.status.success(),
        "the round trip must hold, it said:\n{stdout}{}",
        String::from_utf8_lossy(&run.stderr)
    );
    assert!(
        stdout.lines().any(|line| line == "faults ok 64"),
        "the program must report 64 elements, it said:\n{stdout}"
    );
    let bytes: usize = stdout
        .lines()
        .find_map(|line| line.strip_prefix("encoded bytes "))
        .expect("the program reports the size")
        .parse()
        .expect("a number");
    assert!(
        (66_000..=68_000).contains(&bytes),
        "the 64 elements encode to about 67 KB, they encode to {bytes} bytes"
    );
}
