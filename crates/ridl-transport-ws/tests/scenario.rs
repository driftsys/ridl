//! One scenario, run twice: in process over `ridl-loopback` with no socket,
//! and over WebSocket to a provider in a second process.
//!
//! The second process is this test executable again, run with only the
//! ignored test `provider_process` and the environment variable
//! `RIDL_TRANSPORT_WS_PROVIDER=1`, which makes that test serve the provider
//! on a TCP port it prints. The child ends on its own when its stdin reaches
//! the end, which happens when the parent ends without killing it, and when
//! no connection arrives within [`ACCEPT_LIMIT`].

mod support;

use std::io::{self, BufRead, BufReader, ErrorKind, Write};
use std::net::{SocketAddr, TcpListener};
use std::process::{Child, Command, Stdio};
use std::sync::{mpsc, Arc};
use std::thread;
use std::time::{Duration, Instant};

use ridl_loopback::Loopback;
use ridl_transport_ws::{serve, Config, Described, Scanning};

use support::client::WsClient;
use support::provider::{member_table, provider, provider_step, CATALOG};
use support::scenario::{run, Rig};

/// The environment variable that makes `provider_process` serve.
const PROVIDER_ENV: &str = "RIDL_TRANSPORT_WS_PROVIDER";

/// How long the provider's process waits for its one connection.
const ACCEPT_LIMIT: Duration = Duration::from_secs(10);

/// How long the parent waits for the provider's process to start and print
/// its port: a process start on a loaded runner takes longer than one step
/// of the scenario, which `support::scenario::LIMIT` bounds at 2 s.
const START_LIMIT: Duration = Duration::from_secs(10);

/// The in-process rig: the consumer's ports and the provider are two
/// aggregates on one loopback store, and a step is one pass of the provider.
struct LoopbackRig {
    provider: Loopback,
    ports: Loopback,
}

impl Rig for LoopbackRig {
    type Ports = Loopback;

    fn ports(&mut self) -> &mut Loopback {
        &mut self.ports
    }

    fn step(&mut self) {
        provider_step(&mut self.provider);
    }
}

#[test]
fn the_scenario_runs_over_the_loopback_with_no_socket() {
    let provider = provider();
    let ports = provider.attach();
    run(&mut LoopbackRig { provider, ports });
}

/// The child process. Dropping it kills it and waits for it, so that a test
/// that fails leaves no process behind.
struct ChildGuard(Child);

impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// The cross-process rig: the consumer's ports are the WebSocket client, and
/// a step reads what the provider's process sent.
struct WsRig {
    client: WsClient,
    // Declared after `client`, so that it is dropped after it.
    _child: ChildGuard,
}

impl Rig for WsRig {
    type Ports = WsClient;

    fn ports(&mut self) -> &mut WsClient {
        &mut self.client
    }

    fn step(&mut self) {
        self.client.pump();
    }
}

/// Runs this executable as the provider's process and returns it with the
/// address it serves on.
fn spawn_provider() -> (ChildGuard, SocketAddr) {
    let exe = std::env::current_exe().expect("the test executable");
    let mut child = ChildGuard(
        Command::new(exe)
            .args(["--exact", "provider_process", "--nocapture", "--ignored"])
            .env(PROVIDER_ENV, "1")
            // A pipe the parent never writes: the child reads its end when
            // the parent's side closes, which happens however the parent ends.
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .expect("the provider's process starts"),
    );
    let stdout = child.0.stdout.take().expect("a piped stdout");

    // The test harness writes its own lines to stdout too, so the reader looks
    // for the line that carries `port <n>`. It reads on its own thread, so that
    // the wait is bounded, and goes on reading until the child exits, so that
    // the child never blocks on a full pipe.
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            let Ok(line) = line else { break };
            if let Some((_, port)) = line.split_once("port ") {
                if let Ok(port) = port.trim().parse::<u16>() {
                    let _ = tx.send(port);
                }
            }
        }
    });
    let port = rx
        .recv_timeout(START_LIMIT)
        .expect("the provider's process prints its port within the limit");
    (child, SocketAddr::from(([127, 0, 0, 1], port)))
}

#[test]
fn the_scenario_runs_over_websocket_to_a_second_process() {
    let (child, addr) = spawn_provider();
    let client = WsClient::connect(addr, CATALOG, 1);
    run(&mut WsRig {
        client,
        _child: child,
    });
}

/// The provider's process. It returns at once unless
/// `RIDL_TRANSPORT_WS_PROVIDER=1`; then it prints `port <n>`, runs the
/// provider on a thread every 5 ms, and serves the first connection on that
/// port. It exits when its stdin reaches the end, and fails when no
/// connection arrives within [`ACCEPT_LIMIT`].
#[test]
#[ignore = "run as the second process of the WebSocket scenario"]
fn provider_process() {
    if std::env::var(PROVIDER_ENV).as_deref() != Ok("1") {
        return;
    }
    thread::spawn(|| {
        // The parent writes nothing, so this returns when the parent's end of
        // the pipe closes.
        let _ = io::copy(&mut io::stdin(), &mut io::sink());
        std::process::exit(0);
    });
    let mut rt = provider();
    let backend = Described::new(Scanning(rt.attach()), Arc::new(member_table()))
        .expect("the table matches the provider's catalog");
    let listener = TcpListener::bind("127.0.0.1:0").expect("bound");
    let port = listener.local_addr().expect("an address").port();
    println!("port {port}");
    std::io::stdout().flush().expect("flushed");
    thread::spawn(move || loop {
        provider_step(&mut rt);
        thread::sleep(Duration::from_millis(5));
    });
    listener
        .set_nonblocking(true)
        .expect("a nonblocking listener");
    let start = Instant::now();
    let conn = loop {
        match listener.accept() {
            Ok((conn, _)) => break conn,
            Err(e) if e.kind() == ErrorKind::WouldBlock => {
                assert!(
                    start.elapsed() < ACCEPT_LIMIT,
                    "a connection within {ACCEPT_LIMIT:?}"
                );
                thread::sleep(Duration::from_millis(5));
            }
            Err(e) => panic!("accept failed: {e}"),
        }
    };
    // On some platforms the accepted stream inherits the listener's mode.
    conn.set_nonblocking(false).expect("a blocking connection");
    serve(conn, backend, &Config::default());
}
