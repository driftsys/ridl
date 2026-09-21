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
//! It is also `just demo`'s program. `examples/cabin/Cargo.toml` is a
//! two-member cargo workspace — this crate and the generated one — outside
//! the repository's own workspace, which excludes `examples`. So this file is
//! built two ways from one source: by `cargo` for the demo, and by a bare
//! `rustc` for the test. The crate name is `veh_cabin` in both, because that
//! is the package name `ridlc` writes into the generated `Cargo.toml`.
//!
//! One `Loopback` per round trip, rather than one for all four: the loopback
//! holds every value in one map, and a fresh port is what keeps each round
//! trip's assertions about what is waiting true independently of the order
//! they run in.

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

fn main() {
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
    println!("signal ok");

    // 2 — event
    let mut port = Loopback::new(CATALOG);
    cabin::Client::new(&mut port)
        .subscribe_warning()
        .expect("subscribe");
    cabin::Publisher::new(&mut port)
        .warning(api::Warning {
            code: api::Level::new_unchecked(5),
            health: api::Health::WARN,
        })
        .expect("raise warning");
    let event = cabin::Client::new(&mut port)
        .next_event()
        .expect("next_event")
        .expect("an occurrence is waiting");
    match event {
        cabin::Event::Warning(occurrence) => {
            let warning = occurrence.payload.expect("payload verifies");
            assert_eq!(warning.code.get(), 5);
            assert!(matches!(warning.health, api::Health::WARN));
        }
    }
    println!("event ok");

    // 3 — command
    let mut port = Loopback::new(CATALOG);
    let correlation = cabin::Client::new(&mut port)
        .set_level(api::Level::new_unchecked(42))
        .expect("send setLevel");
    let mut provider = Cabin {
        levels: Vec::new(),
        average: 0,
    };
    let mut buf = [0u8; api::Cabin::MAX_BUFFER_SIZE];
    assert_eq!(cabin::dispatch(&mut port, &mut provider, &mut buf), 1);
    assert_eq!(provider.levels, vec![42]);
    assert_eq!(
        cabin::Client::new(&mut port).set_level_ack(correlation),
        Some(Ok(()))
    );
    println!("command ok");

    // 4 — query
    let mut port = Loopback::new(CATALOG);
    let correlation = cabin::Client::new(&mut port)
        .average(api::Window::new_unchecked(10))
        .expect("send average");
    let mut provider = Cabin {
        levels: Vec::new(),
        average: 7,
    };
    let mut buf = [0u8; api::Cabin::MAX_BUFFER_SIZE];
    assert_eq!(cabin::dispatch(&mut port, &mut provider, &mut buf), 1);
    let reply = cabin::Client::new(&mut port)
        .average_reply(correlation)
        .expect("reply read")
        .expect("reply is known")
        .expect("no call error");
    assert_eq!(reply.get(), 7);
    println!("query ok");
}
