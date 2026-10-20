//! A WebSocket provider over `ridl-loopback`, for the end-to-end tests of the
//! TypeScript client.
//!
//! It binds `127.0.0.1:0`, prints one line `port <n>` on standard output, and
//! serves every connection on its own thread with its own backend, which is a
//! new attachment to one shared provider. The provider is the test catalog of
//! `tests/support/provider.rs`, stepped every 5 ms. The process exits with
//! status 0 when its standard input reaches the end.

// The provider module is shared with the tests, and this example uses only
// part of it.
#[allow(dead_code)]
#[path = "../tests/support/provider.rs"]
mod provider;

use std::io::{self, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use ridl_transport_ws::{serve, Config, Scanning};

fn main() {
    let rt = Arc::new(Mutex::new(provider::provider()));
    let listener = TcpListener::bind("127.0.0.1:0").expect("bound");
    let port = listener.local_addr().expect("an address").port();
    println!("port {port}");
    io::stdout().flush().expect("flushed");

    // The parent writes nothing, so this returns when the parent's end of the
    // pipe closes.
    thread::spawn(|| {
        let _ = io::copy(&mut io::stdin(), &mut io::sink());
        std::process::exit(0);
    });

    let stepping = Arc::clone(&rt);
    thread::spawn(move || loop {
        provider::provider_step(&mut stepping.lock().expect("an unpoisoned provider"));
        thread::sleep(Duration::from_millis(5));
    });

    for conn in listener.incoming() {
        let Ok(conn) = conn else { continue };
        let backend = Scanning(rt.lock().expect("an unpoisoned provider").attach());
        thread::spawn(move || serve(conn, backend, &Config::default()));
    }
}
