//! Remote import fetch and materialization (ADR-0002
//! §5, §7; ADR-0004 §9).
//!
//! [`fetch`] downloads one artifact over HTTP with `ureq` (synchronous, minimal
//! — ADR-0004 §9). [`materialize_imports`] drives the whole resolution: for
//! every remote import URL it serves a vendored tree under
//! `<manifest root>/vendor/` when the lockfile pins its `tree`, else reuses a
//! cache hit when the lockfile-pinned hash is already unpacked, fetches and
//! caches otherwise, and regenerates the lockfile from the hashes it resolved.
//! Under [`Frozen::Yes`] it never fetches and fails on any import that is
//! missing from the lockfile or absent from both `vendor/` and the cache
//! (ADR-0002 §7).
//!
//! The fetched artifact is an uncompressed tar archive of one package directory
//! (ADR-0007 decision 12, provisional until the registry spec is written);
//! [`Cache::store`] unpacks it.
//!
//! This module sits behind the `fetch` feature: it pulls `ureq` for the network
//! and drives the `fetch`-gated [`Cache`].

use std::path::{Path, PathBuf};
use std::time::Duration;

use rowan::TextRange;

use crate::cache::Cache;
use crate::diag::{DiagCode, Diagnostic, FileId, Severity, Span};
use crate::lock::{LockEntry, Lockfile};
use crate::tree_hash::{TreeHashError, tree_hash};
use crate::workspace::ImportUnion;
pub use crate::workspace::MaterializedUnit;

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
/// `FETCH_TIMEOUT` (30 seconds): a stalled or trickling server is a
/// [`FetchError`], never a hang. A non-2xx HTTP status, a transport failure, or
/// a body over the 10 MB read limit is also a [`FetchError`]. Callers validate
/// the URL first (see `is_fetchable_url`); `ureq` will still reject a
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
/// Full RFC 3986 and version-suffix validation is deferred to the registry spec.
/// This only rejects values that plainly cannot be fetched.
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

/// Materializes every remote import from `vendor/` or the cache and returns
/// the verified [`MaterializedUnit`]s, the regenerated [`Lockfile`], and any
/// diagnostics.
///
/// For each URL, in sorted order (a URL named by two keys is materialized
/// once):
///
/// - If `vendor` is given, holds a directory named by a key of the URL, and
///   the lockfile pins a `tree` for the URL, the vendored directory is
///   verified against that `tree` and serves the unit: no cache lookup and
///   no fetch, frozen or not (`MANI-107` naming `ridl vendor` on a mismatch,
///   and the cache is not consulted instead). The lock entry is kept as it
///   is. Without a pinned `tree`, the steps below run first so that a pin
///   exists, and the vendored directory is then verified against it and
///   serves the unit in place of the cache entry.
/// - If the lockfile pins a hash and that hash is already unpacked in the
///   cache, the cache hit is used and no fetch happens. The pinned `tree` is
///   then verified against the unpacked tree (`MANI-107` on a mismatch or a
///   file that is not a regular file). An entry with no `tree` gets one
///   computed and pinned under [`Frozen::No`], and is `MANI-108` under
///   [`Frozen::Yes`].
/// - Otherwise, under [`Frozen::No`], the artifact is fetched and cached. If the
///   lockfile pinned a different hash, the fetch is a `MANI-102` mismatch and
///   the import is dropped. If the entry pins a `tree`, the unpacked tree is
///   verified against it as for a cache hit (`MANI-107` on a mismatch) and the
///   pinned value is kept; an entry with no `tree` gets the newly computed
///   `tree` pinned. The `sha256` is the newly computed one.
/// - Under [`Frozen::Yes`] nothing is ever fetched: a URL with no lockfile entry
///   is `MANI-103`, and a pinned URL that is not in the cache is `MANI-104`,
///   whose message names `ridl check` and `ridl vendor` as the fix.
///
/// A URL that is not a fetchable `http(s)` URL is a `MANI-101` fetch failure.
/// Every diagnostic is detached ([`FileId::DETACHED`]): it names the URL rather
/// than a source span, so the caller renders it as a bare coded message.
pub fn materialize_imports(
    imports: &ImportUnion,
    lock: Option<&Lockfile>,
    cache: &Cache,
    frozen: Frozen,
    vendor: Option<&Path>,
) -> (Vec<MaterializedUnit>, Lockfile, Vec<Diagnostic>) {
    let mut units = Vec::new();
    let mut regenerated = Lockfile::default();
    let mut diagnostics = Vec::new();

    for (url, keys) in imports {
        if !is_fetchable_url(url) {
            diagnostics.push(detached(
                DiagCode::MANI_101,
                format!("cannot fetch `{url}`: not a fetchable `http(s)` URL"),
            ));
            continue;
        }
        let vendored = vendor.and_then(|vendor| {
            keys.iter()
                .map(|key| vendor.join(key))
                .find(|dir| dir.is_dir())
        });
        let pinned = lock.and_then(|lock| lock.entries.get(url));
        // A vendored tree with a pinned `tree` is verified and used as it
        // is, with no cache and no fetch: the air-gapped build. Without a
        // usable pin, the cache-or-fetch step runs first so that the pin
        // exists, and the vendored tree is then verified against it.
        let resolved = match (&vendored, pinned.and_then(|entry| entry.tree.as_deref())) {
            (Some(dir), Some(tree)) => {
                verify_tree(url, dir, tree, TreeOrigin::Vendor, &mut diagnostics).map(|()| {
                    (
                        LockEntry {
                            sha256: pinned
                                .expect("a pinned tree implies a lock entry")
                                .sha256
                                .clone(),
                            tree: Some(tree.to_string()),
                        },
                        MaterializedUnit {
                            url: url.clone(),
                            dir: dir.clone(),
                            tree: tree.to_string(),
                        },
                    )
                })
            }
            _ => from_cache_or_fetch(url, pinned, cache, frozen, &mut diagnostics).and_then(
                |(entry, unit)| match &vendored {
                    Some(dir) => {
                        verify_tree(url, dir, &unit.tree, TreeOrigin::Vendor, &mut diagnostics).map(
                            |()| {
                                (
                                    entry,
                                    MaterializedUnit {
                                        dir: dir.clone(),
                                        ..unit
                                    },
                                )
                            },
                        )
                    }
                    None => Some((entry, unit)),
                },
            ),
        };
        if let Some((entry, unit)) = resolved {
            regenerated.entries.insert(url.clone(), entry);
            units.push(unit);
        }
    }
    (units, regenerated, diagnostics)
}

/// The cache-or-fetch step for one URL: the verified unit and the lock entry
/// to write, or `None` after a diagnostic was pushed. The rules are those of
/// [`materialize_imports`].
fn from_cache_or_fetch(
    url: &str,
    pinned: Option<&LockEntry>,
    cache: &Cache,
    frozen: Frozen,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<(LockEntry, MaterializedUnit)> {
    // A cache hit against the pinned hash skips the fetch entirely, the one
    // path that must never touch the network (proven by the request counter in
    // the tests).
    if let Some(entry) = pinned
        && let Some(dir) = cache.lookup(url, &entry.sha256)
    {
        let tree = match &entry.tree {
            Some(tree) => {
                verify_tree(url, &dir, tree, TreeOrigin::Cache, diagnostics)?;
                tree.clone()
            }
            None => match frozen {
                Frozen::Yes => {
                    diagnostics.push(detached(
                        DiagCode::MANI_108,
                        format!(
                            "the `ridl.lock` entry for `{url}` has no `tree` hash; run `ridl check` without `--frozen` to add it"
                        ),
                    ));
                    return None;
                }
                Frozen::No => hash_tree(url, &dir, diagnostics)?,
            },
        };
        return Some(pinned_unit(url, entry.sha256.clone(), dir, tree));
    }

    match frozen {
        Frozen::Yes => {
            diagnostics.push(match pinned {
                None => detached(
                    DiagCode::MANI_103,
                    format!(
                        "no `ridl.lock` entry for `{url}`; run `ridl check` without `--frozen` to fetch and pin it"
                    ),
                ),
                Some(_) => detached(
                    DiagCode::MANI_104,
                    format!(
                        "`{url}` is pinned in `ridl.lock` but not in the cache; run `ridl check` without `--frozen` to fetch it, or `ridl vendor` to vendor it"
                    ),
                ),
            });
            None
        }
        Frozen::No => {
            let bytes = match fetch(url) {
                Ok(bytes) => bytes,
                Err(err) => {
                    diagnostics.push(detached(
                        DiagCode::MANI_101,
                        format!("failed to fetch `{url}`: {}", err.message),
                    ));
                    return None;
                }
            };
            // Store before verifying against the pin: the cache is
            // content-addressed, so a mismatched artifact lands at its own
            // hash, never in the pinned entry's directory. It can never be
            // served as the pin; the mismatch below then drops it.
            let (sha, dir) = match cache.store(url, &bytes) {
                Ok(stored) => stored,
                Err(err) => {
                    diagnostics.push(detached(
                        DiagCode::MANI_101,
                        format!("failed to cache `{url}`: {err}"),
                    ));
                    return None;
                }
            };
            if let Some(entry) = pinned
                && entry.sha256 != sha
            {
                diagnostics.push(detached(
                    DiagCode::MANI_102,
                    format!(
                        "hash mismatch for `{url}`: the lockfile pins `{}` but the fetched content hashes to `{sha}`",
                        entry.sha256
                    ),
                ));
                return None;
            }
            // A pinned `tree` is verified after a fetch as after a cache hit:
            // the artifact is the pinned one, but the pin says what its
            // unpacked tree must be.
            let tree = match pinned.and_then(|entry| entry.tree.as_deref()) {
                Some(expected) => {
                    verify_tree(url, &dir, expected, TreeOrigin::Cache, diagnostics)?;
                    expected.to_string()
                }
                None => hash_tree(url, &dir, diagnostics)?,
            };
            Some(pinned_unit(url, sha, dir, tree))
        }
    }
}

/// The lock entry and the unit for a verified `dir`.
fn pinned_unit(
    url: &str,
    sha256: String,
    dir: PathBuf,
    tree: String,
) -> (LockEntry, MaterializedUnit) {
    (
        LockEntry {
            sha256,
            tree: Some(tree.clone()),
        },
        MaterializedUnit {
            url: url.to_string(),
            dir,
            tree,
        },
    )
}

/// The `tree` hash of `dir`, or `None` after a MANI-107 that names the
/// non-regular file or the read failure.
fn hash_tree(url: &str, dir: &Path, diagnostics: &mut Vec<Diagnostic>) -> Option<String> {
    match tree_hash(dir) {
        Ok(tree) => Some(tree),
        Err(TreeHashError::NotRegular(path)) => {
            diagnostics.push(detached(
                DiagCode::MANI_107,
                format!(
                    "`{}` in the tree of `{url}` is not a regular file; a remote unit holds regular files only",
                    path.display()
                ),
            ));
            None
        }
        Err(err) => {
            diagnostics.push(detached(
                DiagCode::MANI_107,
                format!("cannot hash the tree of `{url}`: {err}"),
            ));
            None
        }
    }
}

/// Where a tree [`verify_tree`] checks was read from. It chooses the wording
/// of the MANI-107 message and the fix it names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TreeOrigin {
    /// A cache entry (`<cache root>/<url hash>/<sha256>/`).
    Cache,
    /// A vendored directory (`<manifest root>/vendor/<key>/`), which
    /// `ridl vendor` rewrites.
    Vendor,
}

/// `Some(())` when `dir` hashes to `expected`; `None` after a MANI-107 that
/// names the non-regular file, the read failure, or the mismatch. For a
/// mismatch of [`TreeOrigin::Vendor`] the message names `ridl vendor` as the
/// fix. The one compare of a tree against a pinned `tree`, whichever
/// directory the unit is read from.
pub fn verify_tree(
    url: &str,
    dir: &Path,
    expected: &str,
    origin: TreeOrigin,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<()> {
    let actual = hash_tree(url, dir, diagnostics)?;
    if actual == expected {
        return Some(());
    }
    let message = match origin {
        TreeOrigin::Cache => format!(
            "the tree of `{}` does not match the `tree` hash `ridl.lock` pins for `{url}`",
            dir.display()
        ),
        TreeOrigin::Vendor => format!(
            "the vendored tree `{}` does not match the `tree` hash `ridl.lock` pins for `{url}`; run `ridl vendor` to rewrite it",
            dir.display()
        ),
    };
    diagnostics.push(detached(DiagCode::MANI_107, message));
    None
}

/// Builds a detached error [`Diagnostic`] for a URL-scoped problem: no source
/// span, since a fetch, lockfile or remote-unit problem has no meaningful
/// byte range. The loader raises MANI-105 and MANI-106 through it.
pub(crate) fn detached(code: DiagCode, message: String) -> Diagnostic {
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
    use std::collections::{BTreeMap, BTreeSet};
    use std::io::{BufRead as _, BufReader, Write as _};
    use std::net::{TcpListener, TcpStream};
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

    fn imports_of(url: &str) -> ImportUnion {
        BTreeMap::from([(url.to_string(), BTreeSet::from(["remote.pkg".to_string()]))])
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

        let (resolved, lock, diags) = materialize_imports(&imports, None, &cache, Frozen::No, None);
        assert!(diags.is_empty(), "a clean fetch, no diagnostics: {diags:?}");
        assert_eq!(stub.hits(), 1, "the artifact is fetched exactly once");

        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].url, stub.url());
        let path = &resolved[0].dir;
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
        let (_, lock, diags) = materialize_imports(&imports, None, &cache, Frozen::No, None);
        assert!(diags.is_empty());
        assert_eq!(stub.hits(), 1, "the first run fetches once");

        // Second run, with the lockfile from the first and the same cache: the
        // pinned hash is already unpacked, so no request is made.
        stub.reset_hits();
        let (resolved, lock2, diags2) =
            materialize_imports(&imports, Some(&lock), &cache, Frozen::No, None);
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
                tree: None,
            },
        );

        let (resolved, regenerated, diags) =
            materialize_imports(&imports, Some(&lock), &cache, Frozen::No, None);
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
        let (_, lock, _) = materialize_imports(&imports, None, &cache, Frozen::No, None);
        stub.reset_hits();

        let (resolved, _, diags) =
            materialize_imports(&imports, Some(&lock), &cache, Frozen::Yes, None);
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

        let (resolved, _, diags) = materialize_imports(&imports, None, &cache, Frozen::Yes, None);
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
                tree: None,
            },
        );

        let (resolved, _, diags) =
            materialize_imports(&imports, Some(&lock), &cache, Frozen::Yes, None);
        assert_eq!(codes(&diags), vec!["MANI-104"]);
        assert!(
            diags[0].message.contains("`ridl check`") && diags[0].message.contains("`ridl vendor`"),
            "the message names both fixes: {}",
            diags[0].message
        );
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
        let imports = BTreeMap::from([(
            "not-a-url".to_string(),
            BTreeSet::from(["bad.dep".to_string()]),
        )]);

        let (resolved, _, diags) = materialize_imports(&imports, None, &cache, Frozen::No, None);
        assert_eq!(codes(&diags), vec!["MANI-101"]);
        assert!(resolved.is_empty());
    }

    /// A fetch pins both hashes: the tar's `sha256` and the unpacked `tree`,
    /// and the returned unit carries the same `tree`.
    #[test]
    fn a_fetch_pins_the_tree_hash() {
        let stub = Stub::serving(make_tar("remote.typl", b"package remote.pkg\n"));
        let dir = TempDir::new("tree-pin");
        let cache = cache(&dir);
        let (units, lock, diags) =
            materialize_imports(&imports_of(stub.url()), None, &cache, Frozen::No, None);
        assert!(diags.is_empty(), "{diags:?}");
        let entry = &lock.entries[stub.url()];
        let expected = crate::tree_hash::tree_hash(&units[0].dir).unwrap();
        assert_eq!(entry.tree.as_deref(), Some(expected.as_str()));
        assert_eq!(units[0].tree, expected);
    }

    /// A cache hit whose lock entry has no `tree` (a lockfile written before
    /// the hash existed) adds it on a non-frozen run, without a fetch.
    #[test]
    fn a_cache_hit_adds_a_missing_tree_without_fetching() {
        let stub = Stub::serving(make_tar("remote.typl", b"package remote.pkg\n"));
        let dir = TempDir::new("tree-add");
        let cache = cache(&dir);
        let (_, lock, _) =
            materialize_imports(&imports_of(stub.url()), None, &cache, Frozen::No, None);
        let mut old = lock.clone();
        old.entries.get_mut(stub.url()).unwrap().tree = None;
        stub.reset_hits();
        let (_, regenerated, diags) = materialize_imports(
            &imports_of(stub.url()),
            Some(&old),
            &cache,
            Frozen::No,
            None,
        );
        assert!(diags.is_empty(), "{diags:?}");
        assert_eq!(stub.hits(), 0, "the cache hit makes no request");
        assert_eq!(regenerated, lock, "the tree hash is added back");
    }

    /// Under `--frozen`, a lock entry with no `tree` is MANI-108.
    #[test]
    fn frozen_entry_without_tree_is_mani_108() {
        let stub = Stub::serving(make_tar("remote.typl", b"package remote.pkg\n"));
        let dir = TempDir::new("frozen-108");
        let cache = cache(&dir);
        let (_, lock, _) =
            materialize_imports(&imports_of(stub.url()), None, &cache, Frozen::No, None);
        let mut old = lock;
        old.entries.get_mut(stub.url()).unwrap().tree = None;
        let (units, _, diags) = materialize_imports(
            &imports_of(stub.url()),
            Some(&old),
            &cache,
            Frozen::Yes,
            None,
        );
        assert_eq!(codes(&diags), vec!["MANI-108"]);
        assert!(units.is_empty());
    }

    /// A cache entry whose tree does not hash to the pinned `tree` is
    /// MANI-107, frozen or not, and the unit is not returned.
    #[test]
    fn a_tree_mismatch_is_mani_107() {
        let stub = Stub::serving(make_tar("remote.typl", b"package remote.pkg\n"));
        let dir = TempDir::new("tree-107");
        let cache = cache(&dir);
        let (units, lock, _) =
            materialize_imports(&imports_of(stub.url()), None, &cache, Frozen::No, None);
        std::fs::write(units[0].dir.join("extra.typl"), b"package remote.pkg\n").unwrap();
        for frozen in [Frozen::Yes, Frozen::No] {
            let (units, _, diags) =
                materialize_imports(&imports_of(stub.url()), Some(&lock), &cache, frozen, None);
            assert_eq!(codes(&diags), vec!["MANI-107"], "{frozen:?}");
            assert!(units.is_empty());
        }
    }

    /// A fresh fetch is verified against a pinned `tree` as a cache hit is:
    /// when the unpacked tree does not hash to the pin, the fetch is
    /// MANI-107 and the unit is not returned.
    #[test]
    fn a_fetch_into_an_empty_cache_verifies_the_pinned_tree() {
        let stub = Stub::serving(make_tar("remote.typl", b"package remote.pkg\n"));
        let dir = TempDir::new("tree-fetch-107");
        let (_, lock, _) = materialize_imports(
            &imports_of(stub.url()),
            None,
            &cache(&dir),
            Frozen::No,
            None,
        );
        let mut pinned = lock;
        pinned.entries.get_mut(stub.url()).unwrap().tree = Some("0".repeat(64));
        let fresh = TempDir::new("tree-fetch-107-fresh");
        stub.reset_hits();
        let (units, _, diags) = materialize_imports(
            &imports_of(stub.url()),
            Some(&pinned),
            &cache(&fresh),
            Frozen::No,
            None,
        );
        assert_eq!(stub.hits(), 1, "the empty cache is a miss");
        assert_eq!(codes(&diags), vec!["MANI-107"]);
        assert!(units.is_empty());
    }

    /// A fresh fetch whose unpacked tree hashes to the pinned `tree` draws no
    /// diagnostic, returns the unit with the pinned value, and leaves the lock
    /// entry as it was.
    #[test]
    fn a_fetch_into_an_empty_cache_accepts_the_matching_pinned_tree() {
        let stub = Stub::serving(make_tar("remote.typl", b"package remote.pkg\n"));
        let dir = TempDir::new("tree-fetch-ok");
        let (_, lock, _) = materialize_imports(
            &imports_of(stub.url()),
            None,
            &cache(&dir),
            Frozen::No,
            None,
        );
        let pinned_tree = lock.entries[stub.url()].tree.clone().unwrap();
        let fresh = TempDir::new("tree-fetch-ok-fresh");
        stub.reset_hits();
        let (units, regenerated, diags) = materialize_imports(
            &imports_of(stub.url()),
            Some(&lock),
            &cache(&fresh),
            Frozen::No,
            None,
        );
        assert_eq!(stub.hits(), 1, "the empty cache is a miss");
        assert!(diags.is_empty(), "{diags:?}");
        assert_eq!(units.len(), 1);
        assert_eq!(units[0].tree, pinned_tree);
        assert_eq!(regenerated, lock);
    }

    /// A symbolic link in a cache entry is MANI-107 naming the link; the
    /// unit is not returned.
    #[cfg(unix)]
    #[test]
    fn a_symlink_in_the_tree_is_mani_107_naming_it() {
        let stub = Stub::serving(make_tar("remote.typl", b"package remote.pkg\n"));
        let dir = TempDir::new("tree-link");
        let cache = cache(&dir);
        let (units, lock, _) =
            materialize_imports(&imports_of(stub.url()), None, &cache, Frozen::No, None);
        std::os::unix::fs::symlink("remote.typl", units[0].dir.join("link.typl")).unwrap();
        let (units, _, diags) = materialize_imports(
            &imports_of(stub.url()),
            Some(&lock),
            &cache,
            Frozen::Yes,
            None,
        );
        assert_eq!(codes(&diags), vec!["MANI-107"]);
        assert!(
            diags[0].message.contains("link.typl"),
            "{}",
            diags[0].message
        );
        assert!(units.is_empty());
    }

    /// Writes `files` as `vendor/<key>/` and returns the vendor directory.
    fn vendored(dir: &TempDir, key: &str, files: &[(&str, &[u8])]) -> PathBuf {
        let vendor = dir.path().join("vendor");
        for (name, bytes) in files {
            let path = vendor.join(key).join(name);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, bytes).unwrap();
        }
        vendor
    }

    /// With a pinned `tree`, a matching `vendor/<key>/` serves the unit:
    /// no cache, no fetch, frozen or not.
    #[test]
    fn a_matching_vendor_tree_serves_the_unit_without_cache_or_fetch() {
        let stub = Stub::serving(make_tar("remote.typl", b"package remote.pkg\n"));
        let dir = TempDir::new("vendor-hit");
        let cache = cache(&dir);
        let (_, lock, _) =
            materialize_imports(&imports_of(stub.url()), None, &cache, Frozen::No, None);
        let vendor = vendored(
            &dir,
            "remote.pkg",
            &[("remote.typl", b"package remote.pkg\n")],
        );
        std::fs::remove_dir_all(&cache.root).unwrap();
        stub.reset_hits();
        for frozen in [Frozen::Yes, Frozen::No] {
            let (units, regenerated, diags) = materialize_imports(
                &imports_of(stub.url()),
                Some(&lock),
                &cache,
                frozen,
                Some(&vendor),
            );
            assert!(diags.is_empty(), "{frozen:?}: {diags:?}");
            assert_eq!(units[0].dir, vendor.join("remote.pkg"));
            assert_eq!(regenerated, lock);
        }
        assert_eq!(stub.hits(), 0);
    }

    /// A vendored tree that does not hash to the pinned `tree` is MANI-107
    /// naming `ridl vendor`, and the cache is not consulted instead.
    #[test]
    fn a_stale_vendor_tree_is_mani_107() {
        let stub = Stub::serving(make_tar("remote.typl", b"package remote.pkg\n"));
        let dir = TempDir::new("vendor-stale");
        let cache = cache(&dir);
        let (_, lock, _) =
            materialize_imports(&imports_of(stub.url()), None, &cache, Frozen::No, None);
        let vendor = vendored(
            &dir,
            "remote.pkg",
            &[("remote.typl", b"package remote.pkg\r\n")],
        );
        // With the cache gone, a fall-through to the cache would be
        // MANI-104, not MANI-107.
        std::fs::remove_dir_all(&cache.root).unwrap();
        let (units, _, diags) = materialize_imports(
            &imports_of(stub.url()),
            Some(&lock),
            &cache,
            Frozen::Yes,
            Some(&vendor),
        );
        assert_eq!(codes(&diags), vec!["MANI-107"]);
        assert!(
            diags[0].message.contains("ridl vendor"),
            "{}",
            diags[0].message
        );
        assert!(units.is_empty());
    }

    /// A vendored tree with no lock entry: frozen is MANI-103; not frozen
    /// fetches, pins, then verifies the vendored tree against the new pin
    /// and serves it.
    #[test]
    fn a_vendor_tree_without_a_pin_is_fetched_then_verified() {
        let stub = Stub::serving(make_tar("remote.typl", b"package remote.pkg\n"));
        let dir = TempDir::new("vendor-unpinned");
        let cache = cache(&dir);
        let vendor = vendored(
            &dir,
            "remote.pkg",
            &[("remote.typl", b"package remote.pkg\n")],
        );
        let (units, _, diags) = materialize_imports(
            &imports_of(stub.url()),
            None,
            &cache,
            Frozen::Yes,
            Some(&vendor),
        );
        assert_eq!(codes(&diags), vec!["MANI-103"]);
        assert!(units.is_empty());
        let (units, lock, diags) = materialize_imports(
            &imports_of(stub.url()),
            None,
            &cache,
            Frozen::No,
            Some(&vendor),
        );
        assert!(diags.is_empty(), "{diags:?}");
        assert_eq!(stub.hits(), 1);
        assert_eq!(units[0].dir, vendor.join("remote.pkg"));
        assert!(lock.entries[stub.url()].tree.is_some());
    }

    /// A symbolic link in `vendor/<key>/` is MANI-107 naming the link.
    #[cfg(unix)]
    #[test]
    fn a_symlink_in_the_vendor_tree_is_mani_107() {
        let stub = Stub::serving(make_tar("remote.typl", b"package remote.pkg\n"));
        let dir = TempDir::new("vendor-link");
        let cache = cache(&dir);
        let (_, lock, _) =
            materialize_imports(&imports_of(stub.url()), None, &cache, Frozen::No, None);
        let vendor = vendored(
            &dir,
            "remote.pkg",
            &[("remote.typl", b"package remote.pkg\n")],
        );
        std::os::unix::fs::symlink("remote.typl", vendor.join("remote.pkg/alias.typl")).unwrap();
        let (units, _, diags) = materialize_imports(
            &imports_of(stub.url()),
            Some(&lock),
            &cache,
            Frozen::Yes,
            Some(&vendor),
        );
        assert_eq!(codes(&diags), vec!["MANI-107"]);
        assert!(diags[0].message.contains("alias.typl"));
        assert!(units.is_empty());
    }

    /// With two keys for one URL the vendor directory of either key serves.
    #[test]
    fn any_key_directory_serves_a_url_named_twice() {
        let stub = Stub::serving(make_tar("remote.typl", b"package remote.pkg\n"));
        let dir = TempDir::new("vendor-two-keys");
        let cache = cache(&dir);
        let imports: ImportUnion = BTreeMap::from([(
            stub.url().to_string(),
            BTreeSet::from(["remote.alias".to_string(), "remote.pkg".to_string()]),
        )]);
        let (_, lock, _) = materialize_imports(&imports, None, &cache, Frozen::No, None);
        let vendor = vendored(
            &dir,
            "remote.pkg",
            &[("remote.typl", b"package remote.pkg\n")],
        );
        std::fs::remove_dir_all(&cache.root).unwrap();
        let (units, _, diags) =
            materialize_imports(&imports, Some(&lock), &cache, Frozen::Yes, Some(&vendor));
        assert!(diags.is_empty(), "{diags:?}");
        assert_eq!(units[0].dir, vendor.join("remote.pkg"));
    }

    fn codes(diags: &[Diagnostic]) -> Vec<&str> {
        diags.iter().map(|d| d.code.as_str()).collect()
    }
}
