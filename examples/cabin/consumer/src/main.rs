//! An application against the crate `ridl build --emit rust` writes for
//! `cabin.ridl`, written the way one outside this workspace would be: it
//! names the generated face and `ridl-loopback`, and nothing of the compiler
//! that produced the crate.
//!
//! `crates/ridlc/tests/cabin_example.rs` compiles this with plain `rustc`
//! against the emitted crate and runs it. Every `assert` below is part of
//! that proof: the program exits non-zero if a round trip does not hold, and
//! the test fails with its output.
//!
//! Each line it prints carries the value that round trip carried, rather than
//! the bare word `ok`. That is what `just demo` matches, so the gate checks
//! the value that travelled and not merely that a line was printed: a codec
//! or a face returning a wrong value fails the match even if the `assert`
//! above it were weakened. Keep the value in the line when editing one of
//! these — a line rewritten to a bare literal would still match, and would
//! be checking nothing.
//!
//! It is also `just demo`'s program. `examples/cabin/Cargo.toml` is a
//! two-member cargo workspace — this crate and the generated one — outside
//! the repository's own workspace, which excludes `examples`. So this file is
//! built two ways from one source: by `cargo` for the demo, and by a bare
//! `rustc` for the test. The crate name is `veh_cabin` in both, because that
//! is the package name `ridlc` writes into the generated `Cargo.toml`.
//!
//! The consumer side is the generated client, in both of its forms (story
//! E11.21). Round trips 3 and 4 use the async `Client`: a command, a query
//! and `next_event` return a named future, polled here by hand with
//! `ridl_rt::task::noop_waker`, the way a frame loop polls, and the provider
//! side is the generated `serve`, polled once per step. No executor is
//! involved: each round trip is a fixed sequence of polls, and the result of
//! each poll is asserted, so a future that resolved on the wrong poll fails
//! the proof the way a wrong value does. Round trips 5 and 6 make the same
//! command and query through `blocking::Client`, which is
//! `ridl_rt::task::block_on` over the async client and parks this thread
//! until the outcome or its timeout; the provider side is `blocking::serve`
//! on a second thread, called in a loop with a timeout, the way a thread that
//! also does other work serves. The generated crate's `std` feature, on by
//! default, is what carries the `blocking` module.
//!
//! One `Loopback` per round trip, rather than one for all six: the loopback
//! holds every value in one map, and a fresh port is what keeps each round
//! trip's assertions about what is waiting true independently of the order
//! they run in. The handler `serve` runs over is taken from the runtime
//! before the client borrows it, because a call's future holds the client's
//! port for as long as it lives.

use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::task::{Context, Poll};
use std::time::Duration;

use api::cabin;
use ridl_loopback::Loopback;
use ridl_rt::contract::{CatalogHash, CatalogRef};
use ridl_rt::sample::Provenance;
use veh_cabin::veh::cabin as api;

const CATALOG: CatalogRef = CatalogRef {
    name: "veh.cabin",
    hash: CatalogHash([0u8; 32]),
};

struct Cabin {
    levels: Vec<i64>,
    average: i64,
}

impl cabin::Provider for Cabin {
    fn set_level(&mut self, level: &api::Level) {
        self.levels.push(level.get());
    }
    fn average(&mut self, _window: &api::Window) -> api::Average {
        api::Average::new_unchecked(self.average)
    }
}

/// Polls `future` once, the way a frame loop does.
fn poll_once<F: Future + Unpin>(future: &mut F, cx: &mut Context<'_>) -> Poll<F::Output> {
    Pin::new(future).poll(cx)
}

/// How long a blocking call may wait for the serving thread. It is reached
/// only when the round trip is broken, so it is long enough that a loaded
/// machine cannot reach it by delay alone.
const CLIENT_TIMEOUT: Duration = Duration::from_secs(10);

/// Each pass of the serving loop returns after this long with nothing
/// served, so the loop reads its `done` flag between passes.
const SERVE_PASS: Duration = Duration::from_millis(50);

/// Serves `provider` over `handler` on the calling thread until `done` is
/// set: `blocking::serve` returns `Ok(())` at each pass's timeout, and the
/// loop calls it again. A failure of the handler port ends the loop with it.
fn serve_until_done(
    handler: &mut ridl_loopback::HandlerHandle,
    provider: &mut Cabin,
    done: &AtomicBool,
) -> Result<(), ridl_rt::error::ProviderError> {
    while !done.load(Ordering::Acquire) {
        cabin::blocking::serve(&mut *handler, &mut *provider, Some(SERVE_PASS))?;
    }
    Ok(())
}

fn main() {
    let waker = ridl_rt::task::noop_waker();
    let mut cx = Context::from_waker(&waker);

    // 1 — signal
    let mut port = Loopback::new(CATALOG);
    {
        let mut publisher = cabin::Publisher::new(&mut port);
        publisher
            .temperature(api::Temperature::new_unchecked(21))
            .expect("publish temperature");
        publisher.commit();
    }
    let sample = cabin::Client::new(&mut port)
        .temperature()
        .expect("read temperature");
    assert_eq!(sample.value.get(), 21);
    assert_eq!(sample.provenance, Provenance::Live);
    println!("signal ok {}", sample.value.get());

    // 2 — event
    let mut port = Loopback::new(CATALOG);
    cabin::Client::new(&mut port)
        .subscribe_warning()
        .expect("subscribe");
    cabin::Publisher::new(&mut port)
        .warning(api::Warning {
            code: api::Level::new_unchecked(5),
            health: api::Health::Warn,
        })
        .expect("raise warning");
    let mut client = cabin::Client::new(&mut port);
    let mut next = client.next_event();
    let Poll::Ready(event) = poll_once(&mut next, &mut cx) else {
        panic!("an occurrence is waiting, so next_event is ready on its first poll");
    };
    let code = match event.expect("next_event") {
        cabin::Event::Warning(occurrence) => {
            let warning = occurrence.payload.expect("payload verifies");
            assert_eq!(warning.code.get(), 5);
            assert!(matches!(warning.health, api::Health::Warn));
            warning.code.get()
        }
    };
    println!("event ok {}", code);

    // 3 — command
    let mut port = Loopback::new(CATALOG);
    let handler = port.handler();
    let mut provider = Cabin {
        levels: Vec::new(),
        average: 0,
    };
    let mut serve = cabin::serve(handler, &mut provider);
    let mut client = cabin::Client::new(&mut port);
    let mut call = client.set_level(api::Level::new_unchecked(42));
    // Step 1: the call was sent when `set_level` ran, and nothing has served
    // it yet.
    assert!(poll_once(&mut call, &mut cx).is_pending());
    // Step 2: one pass of the provider side settles it.
    assert!(poll_once(&mut serve, &mut cx).is_pending());
    // Step 3: the acknowledgment is taken.
    assert_eq!(poll_once(&mut call, &mut cx), Poll::Ready(Ok(())));
    drop(serve);
    assert_eq!(provider.levels, vec![42]);
    println!("command ok {}", provider.levels[0]);

    // 4 — query
    let mut port = Loopback::new(CATALOG);
    let handler = port.handler();
    let mut provider = Cabin {
        levels: Vec::new(),
        average: 7,
    };
    let mut serve = cabin::serve(handler, &mut provider);
    let mut client = cabin::Client::new(&mut port);
    let mut call = client.average(api::Window::new_unchecked(10));
    assert!(poll_once(&mut call, &mut cx).is_pending());
    assert!(poll_once(&mut serve, &mut cx).is_pending());
    let reply = match poll_once(&mut call, &mut cx) {
        Poll::Ready(Ok(average)) => average,
        other => panic!("the reply is known after one pass of serve, not {other:?}"),
    };
    assert_eq!(reply.get(), 7);
    println!("query ok {}", reply.get());

    // 5 — command, through the blocking client. `set_level` parks this
    // thread until the serving thread settles the call; `done` then ends
    // that thread's loop, and the scope joins it.
    let mut port = Loopback::new(CATALOG);
    let mut handler = port.handler();
    let mut provider = Cabin {
        levels: Vec::new(),
        average: 0,
    };
    let done = AtomicBool::new(false);
    let served = std::thread::scope(|scope| {
        let serving = scope.spawn(|| serve_until_done(&mut handler, &mut provider, &done));
        let mut client = cabin::blocking::Client::new(&mut port).with_timeout(CLIENT_TIMEOUT);
        let acknowledged = client.set_level(api::Level::new_unchecked(42));
        done.store(true, Ordering::Release);
        assert_eq!(acknowledged, Ok(()));
        serving.join().expect("the serving thread does not panic")
    });
    assert_eq!(served, Ok(()));
    assert_eq!(provider.levels, vec![42]);
    println!("blocking command ok {}", provider.levels[0]);

    // 6 — query, through the blocking client.
    let mut port = Loopback::new(CATALOG);
    let mut handler = port.handler();
    let mut provider = Cabin {
        levels: Vec::new(),
        average: 7,
    };
    let done = AtomicBool::new(false);
    let (reply, served) = std::thread::scope(|scope| {
        let serving = scope.spawn(|| serve_until_done(&mut handler, &mut provider, &done));
        let mut client = cabin::blocking::Client::new(&mut port).with_timeout(CLIENT_TIMEOUT);
        let reply = client.average(api::Window::new_unchecked(10));
        done.store(true, Ordering::Release);
        let served = serving.join().expect("the serving thread does not panic");
        (reply, served)
    });
    assert_eq!(served, Ok(()));
    let reply = reply.expect("the query is served within the timeout");
    assert_eq!(reply.get(), 7);
    println!("blocking query ok {}", reply.get());
}
