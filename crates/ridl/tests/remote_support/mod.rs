//! Support for the tests that exercise a remote `[imports]` unit against
//! the built `ridl` binary: a local HTTP stub that serves one tar, a tar
//! builder, and a runner that points `~/.ridl/cache` at a temp directory.

// Each test binary that declares this module uses a subset of it.
#![allow(dead_code)]

use std::io::{BufRead as _, BufReader, Write as _};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::thread::JoinHandle;

/// A unique directory under the system temp dir, removed on drop.
pub struct TempDir(pub PathBuf);

impl TempDir {
    pub fn new(label: &str) -> Self {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let mut path = std::env::temp_dir();
        path.push(format!(
            "ridl-remote-{label}-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::SeqCst),
        ));
        std::fs::create_dir_all(&path).expect("create the temp dir");
        Self(path)
    }

    pub fn path(&self) -> &Path {
        &self.0
    }

    pub fn write(&self, relative: &str, text: &str) -> PathBuf {
        let path = self.0.join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, text).unwrap();
        path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// An uncompressed tar of `files`, each at its path under the unit root.
pub fn unit_tar(files: &[(&str, &str)]) -> Vec<u8> {
    let mut builder = tar::Builder::new(Vec::new());
    for (name, contents) in files {
        let mut header = tar::Header::new_gnu();
        header.set_size(contents.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        builder
            .append_data(&mut header, name, contents.as_bytes())
            .unwrap();
    }
    builder.into_inner().unwrap()
}

/// The `veh.common` unit every test imports: one type at the unit root.
pub const COMMON_UNIT: &[(&str, &str)] = &[
    (
        "ridl.toml",
        "[package]\nname = \"veh.common\"\nversion = \"1.0.0\"\n",
    ),
    (
        "common.typl",
        "package veh.common\n\ntype Speed : km/h [0.0..250.0 step 0.5]\n",
    ),
];

/// The same unit with an interface whose signal has no timing: RIDL-100
/// (`missing-timing`), a Warning the consumer must not see.
pub const WARNING_UNIT: &[(&str, &str)] = &[
    (
        "ridl.toml",
        "[package]\nname = \"veh.common\"\nversion = \"1.0.0\"\n",
    ),
    (
        "common.ridl",
        "package veh.common\n\ntype Speed : km/h [0.0..250.0 step 0.5]\n\ninterface Probe {\n  signal s: Speed\n}\n",
    ),
];

/// The same unit with a used and a malformed lint directive: the consumer
/// must see neither a TYPL-411 nor a TYPL-412 for the unit's file.
pub const DIRECTIVE_UNIT: &[(&str, &str)] = &[
    (
        "ridl.toml",
        "[package]\nname = \"veh.common\"\nversion = \"1.0.0\"\n",
    ),
    (
        "common.ridl",
        "package veh.common\n\ntype Speed : km/h [0.0..250.0 step 0.5]\n\ninterface Probe {\n  // lint: allow missing-timing -- remote\n  signal s: Speed\n  // lint: allow missing-timing\n  signal t: Speed @10ms\n}\n",
    ),
];

/// The same unit with a range whose minimum exceeds its maximum: TYPL-104,
/// an Error the consumer must see.
pub const BROKEN_UNIT: &[(&str, &str)] = &[
    (
        "ridl.toml",
        "[package]\nname = \"veh.common\"\nversion = \"1.0.0\"\n",
    ),
    (
        "common.typl",
        "package veh.common\n\ntype Speed : km/h [250.0..0.0 step 0.5]\n",
    ),
];

/// Runs `ridl` with `args` in `cwd`, with `HOME` set to `home` so the cache
/// is `home/.ridl/cache`. Returns `(exit_code, stdout, stderr)`.
pub fn ridl(cwd: &Path, home: &Path, args: &[&str]) -> (i32, String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_ridl"))
        .current_dir(cwd)
        .env("HOME", home)
        .args(args)
        .output()
        .expect("the ridl binary must run");
    (
        output.status.code().expect("the process exits with a code"),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

/// A consumer package `app` that imports `veh.common.Speed` from `url`.
pub fn consumer(dir: &TempDir, url: &str) -> PathBuf {
    dir.write(
        "app/ridl.toml",
        &format!(
            "[package]\nname = \"app\"\nversion = \"1.0.0\"\n\n[imports]\n\"veh.common\" = \"{url}\"\n"
        ),
    );
    dir.write(
        "app/app.ridl",
        "package app\n\nimport veh.common.Speed\n\n/// The dashboard.\ninterface Dash {\n  /// The speed.\n  signal speed: Speed @10ms\n}\n",
    );
    dir.path().join("app")
}

/// A consumer package `app` whose root manifest lists `podbuf`, with a struct
/// `Reading` whose head reaches `veh.common.Speed`, held inline by `Bounds`,
/// the payload of the interface's signal. `appended` is written after the
/// fields of `Reading`: `""`, or one more field such as `"  quality: Wide\n"`
/// (eight bytes, which grow the head) or `"  quality: Level\n"` (one byte,
/// which fits in the pad of the head).
pub fn inline_consumer(dir: &TempDir, url: &str, appended: &str) -> PathBuf {
    dir.write(
        "app/ridl.toml",
        &format!(
            "[package]\nname = \"app\"\nversion = \"1.0.0\"\n\n[imports]\n\"veh.common\" = \"{url}\"\n\n[backend.rust]\nencodings = [\"podbuf\"]\n"
        ),
    );
    dir.write(
        "app/app.ridl",
        &format!(
            "package app\n\nimport veh.common.Speed\n\ntype Level: integer [0..100]\ntype Wide: integer [0..9223372036854775807]\n\nstruct Reading {{\n  speed: Speed\n  level: Level\n{appended}}}\n\nstruct Bounds {{\n  range: Reading\n  limit: Level\n}}\n\ninterface Dash {{\n  signal bounds: Bounds @10ms\n}}\n"
        ),
    );
    dir.path().join("app")
}

/// A workspace `ws` whose root pins `veh.common` to `url`, with one member
/// `app` that imports `veh.common.Speed`. Returns the member directory.
pub fn member_consumer(dir: &TempDir, url: &str) -> PathBuf {
    dir.write(
        "ws/ridl.toml",
        &format!("[workspace]\nmembers = [\"app\"]\n\n[imports]\n\"veh.common\" = \"{url}\"\n"),
    );
    dir.write(
        "ws/app/ridl.toml",
        "[package]\nname = \"app\"\nversion = \"1.0.0\"\n",
    );
    dir.write(
        "ws/app/app.ridl",
        "package app\nimport veh.common.Speed\ntype Fast: Speed\n",
    );
    dir.path().join("ws/app")
}

/// Writes the `COMMON_UNIT` files as `<root>/vendor/veh.common/`.
pub fn vendor_common(root: &Path) {
    for (name, text) in COMMON_UNIT {
        let path = root.join("vendor/veh.common").join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }
}

/// A local HTTP stub over `std::net::TcpListener`, serving a fixed body for
/// every request and counting the requests it receives — no real network is
/// ever used. The request counter is what proves a cache hit skips the
/// fetch.
pub struct Stub {
    url: String,
    addr: std::net::SocketAddr,
    hits: Arc<AtomicUsize>,
    shutdown: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl Stub {
    /// Starts a stub serving `body` for every GET.
    pub fn serving(body: Vec<u8>) -> Stub {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind the stub");
        let addr = listener.local_addr().expect("stub address");
        let hits = Arc::new(AtomicUsize::new(0));
        let shutdown = Arc::new(AtomicBool::new(false));

        let handle = {
            let hits = hits.clone();
            let shutdown = shutdown.clone();
            std::thread::spawn(move || {
                for incoming in listener.incoming() {
                    if shutdown.load(Ordering::SeqCst) {
                        break;
                    }
                    let stream = match incoming {
                        Ok(stream) => stream,
                        Err(_) => break,
                    };
                    // The shutdown wake-up connection (see `Drop`) arrives
                    // with the flag already set: skip it, do not count it.
                    if shutdown.load(Ordering::SeqCst) {
                        break;
                    }
                    serve_one(stream, &body, &hits);
                }
            })
        };

        Stub {
            url: format!("http://{addr}/package.tar"),
            addr,
            hits,
            shutdown,
            handle: Some(handle),
        }
    }

    pub fn url(&self) -> &str {
        &self.url
    }

    pub fn hits(&self) -> usize {
        self.hits.load(Ordering::SeqCst)
    }

    pub fn reset_hits(&self) {
        self.hits.store(0, Ordering::SeqCst);
    }
}

impl Drop for Stub {
    fn drop(&mut self) {
        // Set the flag first, then make a throwaway connection to unblock
        // the accept loop so the thread can observe the flag and exit.
        self.shutdown.store(true, Ordering::SeqCst);
        let _ = TcpStream::connect(self.addr);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

/// Reads and discards one HTTP request, counts it, and writes the fixed
/// body back with a `Content-Length` and `Connection: close` so `ureq`
/// does not pool the connection (one request per fetch).
pub fn serve_one(mut stream: TcpStream, body: &[u8], hits: &AtomicUsize) {
    let mut reader = BufReader::new(stream.try_clone().expect("clone the stub stream"));
    let mut line = String::new();
    loop {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) => return, // client closed without a request; do not count
            Ok(_) if line == "\r\n" || line == "\n" => break,
            Ok(_) => {}
            Err(_) => return,
        }
    }
    hits.fetch_add(1, Ordering::SeqCst);
    let header = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/x-tar\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len(),
    );
    let _ = stream.write_all(header.as_bytes());
    let _ = stream.write_all(body);
    let _ = stream.flush();
}
