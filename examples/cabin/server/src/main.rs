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
//! then starts again at -40, where it stays for 50 ms so that a client always
//! receives the lowest value. The process exits with status 0 when its standard
//! input reaches the end.
//!
//! The Deno program of `examples/cabin/web` passes only if every value is
//! inside [-40..85], a decrease lands on -40 at least once, and 85 is seen; it
//! also fails on two decreases in a row or on an increase larger than 20.

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
use ridl_transport_ws::{Config, Scanning, serve};
use veh_cabin::veh::cabin as api;

/// The catalog the runtime is attached to: the one the face was generated
/// from.
const CATALOG: CatalogRef = *<api::Cabin as Interface>::CATALOG;

/// The lowest and the highest value of `Temperature`.
const LOWEST: i64 = -40;
const HIGHEST: i64 = 85;

/// The time between two publications.
const PERIOD: Duration = Duration::from_millis(10);

/// The time the lowest value stays published. The transport sends the newest
/// value once per tick of 20 ms and drops the ones before it, so a value
/// published for one period can be skipped. The lowest value stays for more
/// than two ticks, so a client always receives it after a restart.
const LOWEST_DWELL: Duration = Duration::from_millis(50);

fn main() {
    let rt = Arc::new(Mutex::new(Loopback::new(CATALOG)));
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
                thread::sleep(if value == LOWEST {
                    LOWEST_DWELL
                } else {
                    PERIOD
                });
                value = if value == HIGHEST { LOWEST } else { value + 1 };
            }
        }));
        if publishing.is_err() {
            std::process::exit(1);
        }
    });

    for conn in listener.incoming() {
        let Ok(conn) = conn else { continue };
        let backend = Scanning(rt.lock().expect("an unpoisoned runtime").attach());
        thread::spawn(move || serve(conn, backend, &Config::default()));
    }
}
