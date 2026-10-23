//! A WebSocket provider for the cabin example: it publishes the `temperature`
//! signal of the crate `ridl build --emit rust` writes for `cabin.ridl`, and
//! serves it to clients of `ridl-transport-ws`.
//!
//! It binds `127.0.0.1:0` and prints two lines on standard output, each
//! flushed: `port <n>`, and `hash <64 lowercase hexadecimal digits>`, the
//! catalog hash of the generated crate, which a client passes to `attach`. It
//! serves every connection on its own thread with its own attachment to one
//! shared runtime. A publisher thread publishes a new temperature at a fixed
//! period, which steps through the whole range of the type, from -40 to 85, and
//! then starts again at -40, with no pause at any value. The process exits
//! with status 0 when its standard input reaches the end.
//!
//! The Deno program `main.ts` and the browser page `page.ts` of
//! `examples/cabin/web` pass only if every value is inside [-40..85], every
//! received value is 1 to 63 steps ahead of the previous one on the ring of
//! the 126 values (85 is followed by -40; a repeated value is ignored), 85 is
//! reached by such a step, and -40 is seen after 85 (the first sample counts
//! for neither bound).
//!
//! The server gives the transport the member table read from the catalog
//! descriptor that `ridl build --emit catalog` writes, so an `attach` that
//! names a compatible earlier hash is accepted and an unknown interaction is
//! refused.

use std::io::{self, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use api::Temperature;
use api::cabin;
use api::cabin::prelude::*;
use ridl_loopback::Loopback;
use ridl_rt::contract::{CatalogRef, Interface};
use ridl_transport_ws::{Config, Described, MemberTable, Scanning, serve};
use veh_cabin::veh::cabin as api;

/// The catalog the runtime is attached to: the one the face was generated
/// from.
const CATALOG: CatalogRef = *<api::Cabin as Interface>::CATALOG;

/// The catalog descriptor that `ridl build --emit catalog` wrote beside the
/// generated crate. It is embedded so that the programs which start this server
/// need no path argument.
const DESCRIPTOR: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../generated/veh.cabin.catalog.binfb"
));

/// The lowest and the highest value of `Temperature`.
const LOWEST: i64 = -40;
const HIGHEST: i64 = 85;

/// The time between two publications. The transport sends the newest value of
/// a signal once per poll tick (20 ms by default), so a client receives only
/// the values that the ticks sample. 7 ms is not a divisor of 20 ms, so the
/// sampled phase differs from one cycle of 126 values to the next, and the
/// bounds are sampled within a few cycles. The real period is longer than 7 ms
/// because a sleep overshoots, and that overshoot also varies between cycles.
const PERIOD: Duration = Duration::from_millis(7);

fn main() {
    let rt = Arc::new(Mutex::new(Loopback::new(CATALOG)));
    let table =
        Arc::new(MemberTable::from_descriptor(DESCRIPTOR).expect("the descriptor verifies"));
    let listener = TcpListener::bind("127.0.0.1:0").expect("bound");
    let port = listener.local_addr().expect("an address").port();
    let hash: String = CATALOG.hash.0.iter().map(|b| format!("{b:02x}")).collect();
    println!("port {port}");
    println!("hash {hash}");
    io::stdout().flush().expect("flushed");

    // The parent writes nothing, so this returns when the parent's end of the
    // pipe closes.
    thread::spawn(|| {
        let _ = io::copy(&mut io::stdin(), &mut io::sink());
        std::process::exit(0);
    });

    let mut publisher = cabin::Publisher::new(rt.lock().expect("an unpoisoned runtime").attach());
    thread::spawn(move || {
        // A panic here would leave the server serving stale samples, and a
        // client would fail on a timeout instead of on the cause, so it ends
        // the process. A panic on a connection's thread does not.
        let publishing = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut value = LOWEST;
            loop {
                let temperature =
                    Temperature::new(value).expect("the value is inside Temperature's range");
                publisher
                    .temperature(temperature)
                    .expect("publish temperature");
                publisher.commit();
                thread::sleep(PERIOD);
                value = if value == HIGHEST { LOWEST } else { value + 1 };
            }
        }));
        if publishing.is_err() {
            std::process::exit(1);
        }
    });

    for conn in listener.incoming() {
        let Ok(conn) = conn else { continue };
        let backend = Described::new(
            Scanning(rt.lock().expect("an unpoisoned runtime").attach()),
            Arc::clone(&table),
        )
        .expect("the descriptor is the runtime's catalog");
        thread::spawn(move || serve(conn, backend, &Config::default()));
    }
}
