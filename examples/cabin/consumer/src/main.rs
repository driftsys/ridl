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
//! The consumer side is the generated client, in both of its forms. Round trips
//! 1 to 4 use the async `Client`: the signal read of round trip 1 returns at
//! once, and `next_event` (round trip 2), the command (3) and the query (4)
//! return a named future, polled here by hand with `ridl_rt::task::noop_waker`,
//! the way a frame loop polls; the provider side of 3 and 4 is the generated
//! `serve`, polled once per step. No executor is involved: each round trip is a
//! fixed sequence of polls, and the result of each poll is asserted, so a
//! future that resolved on the wrong poll fails the proof the way a wrong value
//! does. Round trips 5 and 6 make the command and the query again through
//! `blocking::Client`, which is `ridl_rt::task::block_on` over the async client
//! and parks this thread until the outcome or its timeout; the provider side is
//! `blocking::serve` on a second thread, called in a loop with a timeout, the
//! way a thread that also does other work serves. The generated crate's `std`
//! feature, on by default, is what carries the `blocking` module.
//!
//! One runtime for all six round trips, and every face over it held for the
//! whole program, which is the shape an application has (driftsys/ridl#488).
//! The `Publisher`, the async `Client` and the `blocking::Client` each own an
//! aggregate from `Loopback::attach`, and the provider side serves over one
//! handler from `Loopback::handler`. The store is shared, so what one round
//! trip leaves in it is still there for the next: the temperature of round
//! trip 1 stays published, and the async client stays subscribed to
//! `warning` after round trip 2. So the round trips are checked against what
//! each one did, not only against what arrived. Round trip 2 checks that the
//! occurrence it raised is the only one waiting. Each call is settled by the
//! `serve` of its own round trip, so no call is left for a later one to
//! find. The provider, which also lives for the whole program, keeps the
//! level of every command, and the blocking round trips send and reply with
//! values of their own (43 and 9, not the 42 and 7 of round trips 3 and 4),
//! so a value left over from an earlier round trip is not mistaken for this
//! one's.
//!
//! Round trips 1 to 4 are blocks, and 5 and 6 are `thread::scope` closures,
//! so that each one's futures and borrows end with it: a call's future
//! borrows its client, and a `serve` future borrows the handler and the
//! provider, until it is dropped.
//!
//! Every value the program sends or replies with is built with `new`, which
//! checks the type's typl constraints, never with `new_unchecked`, which is
//! for a value already known to satisfy them (driftsys/ridl#484). The
//! provider holds its reply as an `Average`, so the value a query replies
//! with was checked when it was built.

use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::task::{Context, Poll};
use std::time::Duration;

use api::cabin;
use api::cabin::prelude::*;
use ridl_loopback::Loopback;
use ridl_rt::contract::{CatalogRef, Interface};
use ridl_rt::sample::Provenance;
use veh_cabin::veh::cabin as api;

/// The catalog the runtime is attached to: the one the face was generated
/// from. The face's `Bind::new` and `serve` compare the port's catalog with
/// it and panic on a mismatch (ADR-0023 decision 8).
const CATALOG: CatalogRef = *<api::Cabin as Interface>::CATALOG;

struct Cabin {
    levels: Vec<i64>,
    average: api::Average,
}

impl cabin::Provider for Cabin {
    fn set_level(&mut self, level: &api::Level) {
        self.levels.push(level.get());
    }
    fn average(&mut self, _window: &api::Window) -> api::Average {
        self.average
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

/// Sets `done` when it is dropped, so the serving loop ends when the round
/// trip on the other thread panics before it sets the flag itself; without
/// it `thread::scope` would wait on a loop that never ends, and the panic
/// would never be reported.
struct DoneOnDrop<'a>(&'a AtomicBool);

impl Drop for DoneOnDrop<'_> {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Release);
    }
}

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

    let rt = Loopback::new(CATALOG);
    let mut publisher = cabin::Publisher::new(rt.attach());
    let mut client = cabin::Client::new(rt.attach());
    let mut blocking_client =
        cabin::blocking::Client::new(rt.attach()).with_timeout(CLIENT_TIMEOUT);
    let mut handler = rt.handler();
    let mut provider = Cabin {
        levels: Vec::new(),
        average: api::Average::new(7).expect("7 is inside Average's range"),
    };

    // 1 — signal
    {
        publisher
            .temperature(api::Temperature::new(21).expect("21 is inside Temperature's range"))
            .expect("publish temperature");
        publisher.commit();
        let sample = client.temperature().expect("read temperature");
        assert_eq!(sample.value.get(), 21);
        assert_eq!(sample.provenance, Provenance::Live);
        println!("signal ok {}", sample.value.get());
    }

    // 2 — event
    {
        client.subscribe_warning().expect("subscribe");
        publisher
            .warning(api::Warning {
                code: api::Level::new(5).expect("5 is inside Level's range"),
                health: api::Health::Warn,
            })
            .expect("raise warning");
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
        drop(next);
        // The occurrence taken above was the only one waiting.
        assert!(poll_once(&mut client.next_event(), &mut cx).is_pending());
        println!("event ok {}", code);
    }

    // 3 — command
    {
        let mut serve = cabin::serve(&mut handler, &mut provider);
        let mut call = client.set_level(api::Level::new(42).expect("42 is inside Level's range"));
        // Step 1: the call was sent when `set_level` ran, and nothing has
        // served it yet.
        assert!(poll_once(&mut call, &mut cx).is_pending());
        // Step 2: one pass of the provider side settles it.
        assert!(poll_once(&mut serve, &mut cx).is_pending());
        // Step 3: the acknowledgment is taken.
        assert_eq!(poll_once(&mut call, &mut cx), Poll::Ready(Ok(())));
    }
    assert_eq!(provider.levels, vec![42]);
    println!("command ok {}", provider.levels[0]);

    // 4 — query
    {
        let mut serve = cabin::serve(&mut handler, &mut provider);
        let mut call = client.average(api::Window::new(10).expect("10 is inside Window's range"));
        assert!(poll_once(&mut call, &mut cx).is_pending());
        assert!(poll_once(&mut serve, &mut cx).is_pending());
        let reply = match poll_once(&mut call, &mut cx) {
            Poll::Ready(Ok(average)) => average,
            other => panic!("the reply is known after one pass of serve, not {other:?}"),
        };
        assert_eq!(reply.get(), 7);
        println!("query ok {}", reply.get());
    }

    // 5 — command, through the blocking client. `set_level` parks this
    // thread until the serving thread settles the call; `done` then ends
    // that thread's loop, and the scope joins it.
    let done = AtomicBool::new(false);
    let served = std::thread::scope(|scope| {
        let serving = scope.spawn(|| serve_until_done(&mut handler, &mut provider, &done));
        let _ends_the_loop = DoneOnDrop(&done);
        let acknowledged =
            blocking_client.set_level(api::Level::new(43).expect("43 is inside Level's range"));
        done.store(true, Ordering::Release);
        assert_eq!(acknowledged, Ok(()));
        serving.join().expect("the serving thread does not panic")
    });
    assert_eq!(served, Ok(()));
    // The command of round trip 3 is the first entry; this one is the second.
    assert_eq!(provider.levels, vec![42, 43]);
    println!("blocking command ok {}", provider.levels[1]);

    // 6 — query, through the blocking client, with a reply other than
    // round trip 4's.
    provider.average = api::Average::new(9).expect("9 is inside Average's range");
    let done = AtomicBool::new(false);
    let (reply, served) = std::thread::scope(|scope| {
        let serving = scope.spawn(|| serve_until_done(&mut handler, &mut provider, &done));
        let _ends_the_loop = DoneOnDrop(&done);
        let reply =
            blocking_client.average(api::Window::new(10).expect("10 is inside Window's range"));
        done.store(true, Ordering::Release);
        let served = serving.join().expect("the serving thread does not panic");
        (reply, served)
    });
    assert_eq!(served, Ok(()));
    let reply = reply.expect("the query is served within the timeout");
    assert_eq!(reply.get(), 9);
    println!("blocking query ok {}", reply.get());
}
