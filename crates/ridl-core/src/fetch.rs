//! Remote import fetch and materialization (docs/ROADMAP.md epic E1.6, ADR-0002
//! §5, §7; ADR-0004 §9).
//!
//! [`fetch`] downloads one artifact over HTTP with `ureq` (synchronous, minimal
//! — ADR-0004 §9). [`materialize_imports`] drives the whole resolution: for
//! every remote import URL it reuses a cache hit when the lockfile-pinned hash
//! is already unpacked, fetches and caches otherwise, and regenerates the
//! lockfile from the hashes it resolved. Under [`Frozen::Yes`] it never fetches
//! and fails on any import that is missing from the lockfile or absent from the
//! cache (ADR-0002 §7).
//!
//! The fetched artifact is an uncompressed tar archive of one package directory
//! (ADR-0007 decision 12, provisional until the registry spec E7.4);
//! [`Cache::store`] unpacks it.
//!
//! This module sits behind the `fetch` feature: it pulls `ureq` for the network
//! and drives the `fetch`-gated [`Cache`].

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;

use rowan::TextRange;

use crate::cache::Cache;
use crate::diag::{DiagCode, Diagnostic, FileId, Severity, Span};
use crate::lock::{LockEntry, Lockfile};

/// A remote fetch failure: the human-readable reason. It carries no source span
/// because it concerns a URL, not a byte range.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FetchError {
    pub message: String,
}

/// The bounded end-to-end timeout applied to every fetch. `ureq`'s default
/// agent leaves connect, receive-headers, and receive-body unbounded, so a
/// server that accepts the connection then stalls — or trickles bytes slowly
/// under the 10 MB cap — would hang the build forever. This caps the whole
/// call.
const FETCH_TIMEOUT: Duration = Duration::from_secs(30);

/// Downloads the artifact at `url` and returns its bytes.
///
/// The entire call — connect, send, and receive — is bounded by
/// [`FETCH_TIMEOUT`] (30 seconds): a stalled or trickling server is a
/// [`FetchError`], never a hang. A non-2xx HTTP status, a transport failure, or
/// a body over the 10 MB read limit is also a [`FetchError`]. Callers validate
/// the URL first (see [`is_fetchable_url`]); `ureq` will still reject a
/// malformed URL here.
pub fn fetch(url: &str) -> Result<Vec<u8>, FetchError> {
    fetch_with_timeout(url, FETCH_TIMEOUT)
}

/// [`fetch`] with an explicit `timeout`, so a test can drive the timeout path
/// with a short bound instead of the production 30 seconds. Builds a one-off
/// agent whose global timeout caps the whole call.
fn fetch_with_timeout(url: &str, timeout: Duration) -> Result<Vec<u8>, FetchError> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(timeout))
        .build()
        .into();
    let mut response = agent.get(url).call().map_err(|err| FetchError {
        message: err.to_string(),
    })?;
    response.body_mut().read_to_vec().map_err(|err| FetchError {
        message: err.to_string(),
    })
}

/// Whether `url` is a value this fetch layer will attempt: an `http(s)` URL with
/// a non-empty host and no whitespace or control characters. The manifest
/// records every `[imports]` value verbatim, including ones that failed
/// `MANI-007`, so a recorded URL is re-validated here before it reaches `ureq`.
/// Full RFC 3986 and version-suffix validation is deferred to the registry spec
/// (E7.4); this only rejects values that plainly cannot be fetched.
fn is_fetchable_url(url: &str) -> bool {
    if url.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return false;
    }
    let Some(rest) = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
    else {
        return false;
    };
    let host_end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    !rest[..host_end].is_empty()
}

/// Whether a materialize run verifies against the lockfile strictly and never
/// fetches ([`Frozen::Yes`], `ridlc --frozen`) or fetches and regenerates the
/// lockfile as needed ([`Frozen::No`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Frozen {
    Yes,
    No,
}

impl From<bool> for Frozen {
    fn from(frozen: bool) -> Self {
        if frozen { Frozen::Yes } else { Frozen::No }
    }
}

/// Materializes every remote import into the cache and returns the resolved
/// `(url, unpacked-package-directory)` pairs, the regenerated [`Lockfile`], and
/// any diagnostics.
///
/// For each URL, in sorted order:
///
/// - If the lockfile pins a hash and that hash is already unpacked in the
///   cache, the cache hit is used and no fetch happens — the regenerated
///   lockfile keeps the pin.
/// - Otherwise, under [`Frozen::No`], the artifact is fetched and cached. If the
///   lockfile pinned a different hash, the fetch is a `MANI-102` mismatch and
///   the import is dropped; otherwise the newly computed hash is pinned.
/// - Under [`Frozen::Yes`] nothing is ever fetched: a URL with no lockfile entry
///   is `MANI-103`, and a pinned URL that is not in the cache is `MANI-104`.
///
/// A URL that is not a fetchable `http(s)` URL is a `MANI-101` fetch failure.
/// Every diagnostic is detached ([`FileId::DETACHED`]): it names the URL rather
/// than a source span, so the caller renders it as a bare coded message.
pub fn materialize_imports(
    imports: &BTreeMap<String, String>,
    lock: Option<&Lockfile>,
    cache: &Cache,
    frozen: Frozen,
) -> (Vec<(String, PathBuf)>, Lockfile, Vec<Diagnostic>) {
    let mut resolved = Vec::new();
    let mut regenerated = Lockfile::default();
    let mut diagnostics = Vec::new();

    // Iterate by URL so the run is deterministic even when two logical names
    // alias the same URL; a URL is materialized once.
    let mut urls: Vec<&String> = imports.values().collect();
    urls.sort();
    urls.dedup();

    for url in urls {
        if !is_fetchable_url(url) {
            diagnostics.push(detached(
                DiagCode::MANI_101,
                format!("cannot fetch `{url}`: not a fetchable `http(s)` URL"),
            ));
            continue;
        }

        let pinned = lock
            .and_then(|lock| lock.entries.get(url))
            .map(|entry| entry.sha256.clone());

        // A cache hit against the pinned hash skips the fetch entirely — the
        // one path that must never touch the network (proven by the request
        // counter in the tests).
        if let Some(sha) = &pinned
            && let Some(path) = cache.lookup(url, sha)
        {
            regenerated.entries.insert(
                url.clone(),
                LockEntry {
                    sha256: sha.clone(),
                },
            );
            resolved.push((url.clone(), path));
            continue;
        }

        match frozen {
            Frozen::Yes => match pinned {
                None => diagnostics.push(detached(
                    DiagCode::MANI_103,
                    format!("`--frozen`: no lockfile entry for `{url}`"),
                )),
                Some(_) => diagnostics.push(detached(
                    DiagCode::MANI_104,
                    format!("`--frozen`: `{url}` is pinned but not in the cache"),
                )),
            },
            Frozen::No => {
                let bytes = match fetch(url) {
                    Ok(bytes) => bytes,
                    Err(err) => {
                        diagnostics.push(detached(
                            DiagCode::MANI_101,
                            format!("failed to fetch `{url}`: {}", err.message),
                        ));
                        continue;
                    }
                };
                // Store before verifying against the pin: the cache is
                // content-addressed, so a mismatched artifact lands at its own
                // hash, never in the pinned entry's directory — it can never be
                // served as the pin. The mismatch below then drops it.
                let (sha, path) = match cache.store(url, &bytes) {
                    Ok(stored) => stored,
                    Err(err) => {
                        diagnostics.push(detached(
                            DiagCode::MANI_101,
                            format!("failed to cache `{url}`: {err}"),
                        ));
                        continue;
                    }
                };
                if let Some(expected) = &pinned
                    && expected != &sha
                {
                    diagnostics.push(detached(
                        DiagCode::MANI_102,
                        format!(
                            "hash mismatch for `{url}`: the lockfile pins `{expected}` but the fetched content hashes to `{sha}`"
                        ),
                    ));
                    continue;
                }
                regenerated
                    .entries
                    .insert(url.clone(), LockEntry { sha256: sha });
                resolved.push((url.clone(), path));
            }
        }
    }

    (resolved, regenerated, diagnostics)
}

/// Builds a detached error [`Diagnostic`] for a URL-scoped problem: no source
/// span, since a fetch or lockfile problem has no meaningful byte range.
fn detached(code: DiagCode, message: String) -> Diagnostic {
    Diagnostic {
        code,
        severity: Severity::Error,
        message,
        primary: Span {
            file: FileId::DETACHED,
            range: TextRange::default(),
        },
        labels: Vec::new(),
        fixits: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use std::io::{BufRead as _, BufReader, Write as _};
    use std::net::{TcpListener, TcpStream};
    use std::path::Path;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::thread::JoinHandle;

    use super::*;

    /// A unique directory under the system temp dir, removed on drop.
    struct TempDir(PathBuf);

    impl TempDir {
        fn new(label: &str) -> Self {
            static COUNTER: AtomicUsize = AtomicUsize::new(0);
            let mut path = std::env::temp_dir();
            path.push(format!(
                "ridl-core-fetch-{label}-{}-{}",
                std::process::id(),
                COUNTER.fetch_add(1, Ordering::SeqCst),
            ));
            std::fs::create_dir_all(&path).expect("create the temp dir");
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// Builds an uncompressed tar archive holding one file — the provisional
    /// fetch artifact (ADR-0007 decision 12).
    fn make_tar(name: &str, contents: &[u8]) -> Vec<u8> {
        let mut builder = tar::Builder::new(Vec::new());
        let mut header = tar::Header::new_gnu();
        header.set_size(contents.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        builder
            .append_data(&mut header, name, contents)
            .expect("append the file to the tar");
        builder.into_inner().expect("finish the tar")
    }

    /// A local HTTP stub over `std::net::TcpListener`, serving a fixed body for
    /// every request and counting the requests it receives — no real network is
    /// ever used. The request counter is what proves a cache hit skips the
    /// fetch.
    struct Stub {
        url: String,
        addr: std::net::SocketAddr,
        hits: Arc<AtomicUsize>,
        shutdown: Arc<AtomicBool>,
        handle: Option<JoinHandle<()>>,
    }

    impl Stub {
        /// Starts a stub serving `body` for every GET.
        fn serving(body: Vec<u8>) -> Stub {
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

        fn url(&self) -> &str {
            &self.url
        }

        fn hits(&self) -> usize {
            self.hits.load(Ordering::SeqCst)
        }

        fn reset_hits(&self) {
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
    fn serve_one(mut stream: TcpStream, body: &[u8], hits: &AtomicUsize) {
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

    /// A stub that accepts connections and never responds — it holds every
    /// accepted stream open so the client waits for a response that never
    /// comes. It proves the fetch timeout fires instead of hanging.
    struct StallingStub {
        addr: std::net::SocketAddr,
        shutdown: Arc<AtomicBool>,
        handle: Option<JoinHandle<()>>,
    }

    impl StallingStub {
        fn start() -> StallingStub {
            let listener = TcpListener::bind("127.0.0.1:0").expect("bind the stalling stub");
            let addr = listener.local_addr().expect("stub address");
            let shutdown = Arc::new(AtomicBool::new(false));
            let handle = {
                let shutdown = shutdown.clone();
                std::thread::spawn(move || {
                    let mut held = Vec::new();
                    for incoming in listener.incoming() {
                        if shutdown.load(Ordering::SeqCst) {
                            break;
                        }
                        match incoming {
                            // Hold the stream open, never write a response.
                            Ok(stream) => held.push(stream),
                            Err(_) => break,
                        }
                    }
                    drop(held);
                })
            };
            StallingStub {
                addr,
                shutdown,
                handle: Some(handle),
            }
        }

        fn url(&self) -> String {
            format!("http://{}/package.tar", self.addr)
        }
    }

    impl Drop for StallingStub {
        fn drop(&mut self) {
            self.shutdown.store(true, Ordering::SeqCst);
            let _ = TcpStream::connect(self.addr);
            if let Some(handle) = self.handle.take() {
                let _ = handle.join();
            }
        }
    }

    fn cache(dir: &TempDir) -> Cache {
        Cache {
            root: dir.path().join("cache"),
        }
    }

    fn imports_of(url: &str) -> BTreeMap<String, String> {
        BTreeMap::from([("remote.pkg".to_string(), url.to_string())])
    }

    /// `fetch` returns the served body over the local stub — one request.
    #[test]
    fn fetch_returns_the_served_bytes() {
        let body = make_tar("a.typl", b"package remote.pkg\n");
        let stub = Stub::serving(body.clone());
        let got = fetch(stub.url()).expect("the fetch succeeds");
        assert_eq!(got, body, "the fetched bytes are the served body");
        assert_eq!(stub.hits(), 1, "exactly one request reached the stub");
    }

    /// A server that accepts the connection then never responds must time out
    /// rather than hang: the bounded global timeout turns the stall into a
    /// `FetchError` promptly. Driven with a short timeout so the test is fast.
    #[test]
    fn fetch_times_out_on_a_stalled_server() {
        let stub = StallingStub::start();
        let start = std::time::Instant::now();
        let result = fetch_with_timeout(&stub.url(), Duration::from_millis(300));
        assert!(result.is_err(), "a stalled server must time out, not hang");
        assert!(
            start.elapsed() < Duration::from_secs(5),
            "the timeout must fire promptly (no hang), took {:?}",
            start.elapsed(),
        );
    }

    /// `fetch` against a refused address is a `FetchError`, the MANI-101 path.
    #[test]
    fn fetch_of_a_dead_address_errors() {
        // Bind then drop to obtain an address nothing is listening on.
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().expect("addr");
        drop(listener);
        let url = format!("http://{addr}/package.tar");
        assert!(fetch(&url).is_err(), "a refused connection is a FetchError");
    }

    /// End to end: the stub serves a tar with one `.typl` file, and
    /// `materialize_imports` lands it unpacked in the cache, returns the
    /// `(url, dir)` pair, and pins the content hash in the regenerated
    /// lockfile.
    #[test]
    fn materialize_unpacks_a_remote_package_into_the_cache() {
        let source = "package remote.pkg\ntype Speed: km/h\n";
        let stub = Stub::serving(make_tar("remote.typl", source.as_bytes()));
        let dir = TempDir::new("materialize");
        let cache = cache(&dir);
        let imports = imports_of(stub.url());

        let (resolved, lock, diags) = materialize_imports(&imports, None, &cache, Frozen::No);
        assert!(diags.is_empty(), "a clean fetch, no diagnostics: {diags:?}");
        assert_eq!(stub.hits(), 1, "the artifact is fetched exactly once");

        assert_eq!(resolved.len(), 1);
        let (resolved_url, path) = &resolved[0];
        assert_eq!(resolved_url, stub.url());
        let unpacked = std::fs::read_to_string(path.join("remote.typl"))
            .expect("the .typl file is unpacked into the cache");
        assert_eq!(unpacked, source);

        let entry = lock.entries.get(stub.url()).expect("the URL is pinned");
        assert_eq!(entry.sha256.len(), 64, "the pin is a SHA-256 hex string");
    }

    /// A cache hit against the lockfile-pinned hash skips the fetch — proven by
    /// the request counter reading zero on the second materialize.
    #[test]
    fn a_cache_hit_skips_the_fetch() {
        let stub = Stub::serving(make_tar("remote.typl", b"package remote.pkg\n"));
        let dir = TempDir::new("cache-hit");
        let cache = cache(&dir);
        let imports = imports_of(stub.url());

        // First run fetches once and produces the lockfile.
        let (_, lock, diags) = materialize_imports(&imports, None, &cache, Frozen::No);
        assert!(diags.is_empty());
        assert_eq!(stub.hits(), 1, "the first run fetches once");

        // Second run, with the lockfile from the first and the same cache: the
        // pinned hash is already unpacked, so no request is made.
        stub.reset_hits();
        let (resolved, lock2, diags2) =
            materialize_imports(&imports, Some(&lock), &cache, Frozen::No);
        assert!(diags2.is_empty());
        assert_eq!(stub.hits(), 0, "a cache hit makes no request");
        assert_eq!(
            resolved.len(),
            1,
            "the package still resolves from the cache"
        );
        assert_eq!(lock2, lock, "the regenerated lockfile keeps the same pin");
    }

    /// Fetched content whose hash differs from the lockfile pin is a MANI-102
    /// mismatch, and the import is dropped.
    #[test]
    fn a_hash_mismatch_is_mani_102() {
        let stub = Stub::serving(make_tar("remote.typl", b"package remote.pkg\n"));
        let dir = TempDir::new("mismatch");
        let cache = cache(&dir); // empty: the pinned hash is not cached, so we fetch
        let imports = imports_of(stub.url());

        // Pin a hash the fetched content will not match.
        let mut lock = Lockfile::default();
        lock.entries.insert(
            stub.url().to_string(),
            LockEntry {
                sha256: "0".repeat(64),
            },
        );

        let (resolved, regenerated, diags) =
            materialize_imports(&imports, Some(&lock), &cache, Frozen::No);
        assert_eq!(stub.hits(), 1, "the mismatch is found only after fetching");
        assert_eq!(codes(&diags), vec!["MANI-102"]);
        assert!(resolved.is_empty(), "a mismatched import does not resolve");
        assert!(
            regenerated.entries.is_empty(),
            "a mismatched hash is never pinned",
        );
    }

    /// Frozen behaviour 1 — success: the pinned hash is already cached, so the
    /// import resolves with no fetch and no diagnostic.
    #[test]
    fn frozen_success_uses_the_cache_without_fetching() {
        let stub = Stub::serving(make_tar("remote.typl", b"package remote.pkg\n"));
        let dir = TempDir::new("frozen-ok");
        let cache = cache(&dir);
        let imports = imports_of(stub.url());

        // Populate the cache and the lockfile with a non-frozen run.
        let (_, lock, _) = materialize_imports(&imports, None, &cache, Frozen::No);
        stub.reset_hits();

        let (resolved, _, diags) = materialize_imports(&imports, Some(&lock), &cache, Frozen::Yes);
        assert!(
            diags.is_empty(),
            "a cached pin resolves cleanly under --frozen"
        );
        assert_eq!(stub.hits(), 0, "--frozen never fetches");
        assert_eq!(resolved.len(), 1);
    }

    /// Frozen behaviour 2 — a URL with no lockfile entry is MANI-103, with no
    /// fetch.
    #[test]
    fn frozen_missing_lockfile_entry_is_mani_103() {
        let stub = Stub::serving(make_tar("remote.typl", b"package remote.pkg\n"));
        let dir = TempDir::new("frozen-103");
        let cache = cache(&dir);
        let imports = imports_of(stub.url());

        let (resolved, _, diags) = materialize_imports(&imports, None, &cache, Frozen::Yes);
        assert_eq!(codes(&diags), vec!["MANI-103"]);
        assert_eq!(stub.hits(), 0, "--frozen never fetches");
        assert!(resolved.is_empty());
    }

    /// Frozen behaviour 3 — a pinned URL that is not in the cache is MANI-104,
    /// with no fetch.
    #[test]
    fn frozen_pinned_but_uncached_is_mani_104() {
        let stub = Stub::serving(make_tar("remote.typl", b"package remote.pkg\n"));
        let dir = TempDir::new("frozen-104");
        let cache = cache(&dir); // empty cache
        let imports = imports_of(stub.url());

        let mut lock = Lockfile::default();
        lock.entries.insert(
            stub.url().to_string(),
            LockEntry {
                sha256: "a".repeat(64),
            },
        );

        let (resolved, _, diags) = materialize_imports(&imports, Some(&lock), &cache, Frozen::Yes);
        assert_eq!(codes(&diags), vec!["MANI-104"]);
        assert_eq!(stub.hits(), 0, "--frozen never fetches");
        assert!(resolved.is_empty());
    }

    /// A recorded import URL that is not a fetchable `http(s)` URL (it may have
    /// failed MANI-007 yet still be recorded verbatim) is a MANI-101 without
    /// touching the network.
    #[test]
    fn an_invalid_url_is_mani_101_without_fetching() {
        let dir = TempDir::new("bad-url");
        let cache = cache(&dir);
        let imports = BTreeMap::from([("bad.dep".to_string(), "not-a-url".to_string())]);

        let (resolved, _, diags) = materialize_imports(&imports, None, &cache, Frozen::No);
        assert_eq!(codes(&diags), vec!["MANI-101"]);
        assert!(resolved.is_empty());
    }

    fn codes(diags: &[Diagnostic]) -> Vec<&str> {
        diags.iter().map(|d| d.code.as_str()).collect()
    }
}
