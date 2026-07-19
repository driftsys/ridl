//! The content-addressed package cache (docs/ROADMAP.md epic E1.6, ADR-0002
//! §7).
//!
//! The cache lives at `~/.ridl/cache` (ADR-0002 §7), indexed by URL and by the
//! SHA-256 content hash of the fetched artifact. A cached entry is never
//! re-fetched as long as the hash on record matches. The layout is
//!
//! ```text
//! <root>/<sha256(url)>/<sha256(artifact)>/…unpacked package files…
//! ```
//!
//! The URL is hashed into the first path segment so the cache is indexed by URL
//! (ADR-0002 §7); the artifact hash is the second segment so the same URL can
//! hold more than one pinned version. The fetched artifact is an uncompressed
//! tar archive of one package directory (ADR-0007 decision 12, provisional
//! until the registry spec E7.4); [`Cache::store`] unpacks it into the entry
//! directory.
//!
//! This module sits behind the `fetch` feature: it hashes with `sha2` and
//! unpacks with `tar`, and the content-addressed cache only exists when remote
//! fetch is compiled in.

use std::fmt::Write as _;
use std::fs;
use std::io;
use std::path::PathBuf;

use sha2::{Digest, Sha256};

/// The on-disk package cache, rooted at [`Cache::root`].
#[derive(Debug, Clone)]
pub struct Cache {
    pub root: PathBuf,
}

impl Cache {
    /// The cache rooted at `~/.ridl/cache` (ADR-0002 §7). When the home
    /// directory is unknown it falls back to `.ridl/cache` relative to the
    /// current directory.
    pub fn user_default() -> Cache {
        let root = std::env::home_dir()
            .unwrap_or_default()
            .join(".ridl")
            .join("cache");
        Cache { root }
    }

    /// The unpacked package directory for `url` pinned to `sha256`, if it is
    /// present in the cache. A hit means the artifact with that content hash is
    /// already unpacked, so no fetch is needed.
    pub fn lookup(&self, url: &str, sha256: &str) -> Option<PathBuf> {
        let path = self.entry_dir(url, sha256);
        path.is_dir().then_some(path)
    }

    /// Stores the artifact `bytes` fetched from `url`, unpacking the tar into
    /// the content-addressed entry directory. Returns the artifact's SHA-256
    /// content hash (lowercase hex) and the unpacked directory.
    ///
    /// The tar is unpacked into a temporary sibling directory and then renamed
    /// into place, so a fetch interrupted mid-unpack never leaves a partial
    /// directory that a later [`lookup`](Cache::lookup) would treat as a hit. A
    /// re-store of already-cached content is a no-op that returns the existing
    /// directory.
    pub fn store(&self, url: &str, bytes: &[u8]) -> io::Result<(String, PathBuf)> {
        let sha256 = sha256_hex(bytes);
        let dest = self.entry_dir(url, &sha256);
        if dest.is_dir() {
            return Ok((sha256, dest));
        }

        let parent = self.url_dir(url);
        fs::create_dir_all(&parent)?;

        // The staging name keys on the process id and the content hash, not a
        // per-call token, so `store` is not safe for two concurrent
        // same-process calls storing the *same* content — they would share this
        // directory. That is fine today: `materialize_imports` is sequential and
        // deduplicates URLs, so no two concurrent stores race here.
        let staging = parent.join(format!(".staging-{sha256}-{}", std::process::id()));
        // A stale staging directory from a crashed run would make `create_dir`
        // fail; clear it first.
        let _ = fs::remove_dir_all(&staging);
        fs::create_dir_all(&staging)?;

        let unpack_result = tar::Archive::new(bytes).unpack(&staging);
        if let Err(err) = unpack_result {
            let _ = fs::remove_dir_all(&staging);
            return Err(err);
        }

        match fs::rename(&staging, &dest) {
            Ok(()) => Ok((sha256, dest)),
            // A concurrent store may have created `dest` between the `is_dir`
            // check and the rename; the content is identical (same hash), so
            // adopt it and drop the staging copy.
            Err(_) if dest.is_dir() => {
                let _ = fs::remove_dir_all(&staging);
                Ok((sha256, dest))
            }
            Err(err) => {
                let _ = fs::remove_dir_all(&staging);
                Err(err)
            }
        }
    }

    /// The first path segment for `url`: `<root>/<sha256(url)>`.
    fn url_dir(&self, url: &str) -> PathBuf {
        self.root.join(sha256_hex(url.as_bytes()))
    }

    /// The full entry directory for `url` pinned to `content_sha`.
    fn entry_dir(&self, url: &str, content_sha: &str) -> PathBuf {
        self.url_dir(url).join(content_sha)
    }
}

/// The lowercase-hex SHA-256 of `bytes`.
fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut hex = String::with_capacity(64);
    for byte in digest {
        // Writing to a String is infallible.
        let _ = write!(hex, "{byte:02x}");
    }
    hex
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;

    /// A unique directory under the system temp dir, removed on drop.
    struct TempDir(PathBuf);

    impl TempDir {
        fn new(label: &str) -> Self {
            static COUNTER: AtomicUsize = AtomicUsize::new(0);
            let mut path = std::env::temp_dir();
            path.push(format!(
                "ridl-core-cache-{label}-{}-{}",
                std::process::id(),
                COUNTER.fetch_add(1, Ordering::SeqCst),
            ));
            fs::create_dir_all(&path).expect("create the temp dir");
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    /// Builds an uncompressed tar archive holding one file at `name` with the
    /// given `contents` — the provisional fetch artifact (ADR-0007 decision 12).
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

    fn cache(dir: &TempDir) -> Cache {
        Cache {
            root: dir.path().join("cache"),
        }
    }

    /// `sha256_hex` matches the SHA-256 of the empty input — the known NIST
    /// value — so the hashing is byte-correct, not just self-consistent.
    #[test]
    fn sha256_hex_matches_the_known_empty_hash() {
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
        );
    }

    /// Storing an artifact unpacks the tar into the content-addressed entry
    /// directory and returns the artifact's hash; a subsequent lookup by that
    /// hash finds the unpacked package.
    #[test]
    fn store_unpacks_and_lookup_finds_it() {
        let dir = TempDir::new("store-lookup");
        let cache = cache(&dir);
        let url = "https://registry.example.com/veh/common@v1.0.0";
        let tar = make_tar("common.typl", b"package veh.common\ntype Speed: km/h\n");

        let (sha, path) = cache.store(url, &tar).expect("the artifact stores");
        assert_eq!(sha.len(), 64, "the hash is a 64-char hex string");
        assert!(path.is_dir(), "the entry directory exists");

        let unpacked = fs::read_to_string(path.join("common.typl")).expect("the file unpacked");
        assert_eq!(unpacked, "package veh.common\ntype Speed: km/h\n");

        // A lookup by the same URL and hash finds the same directory.
        assert_eq!(
            cache.lookup(url, &sha),
            Some(path),
            "lookup finds the stored entry",
        );
    }

    /// A lookup with a hash that was never stored is a miss.
    #[test]
    fn lookup_misses_on_an_unknown_hash() {
        let dir = TempDir::new("miss");
        let cache = cache(&dir);
        let url = "https://registry.example.com/veh/common@v1.0.0";
        cache
            .store(url, &make_tar("a.typl", b"package veh.common\n"))
            .expect("store");

        assert_eq!(
            cache.lookup(url, &"0".repeat(64)),
            None,
            "an unstored hash is a cache miss",
        );
        // A different URL with the right hash still misses: the URL is part of
        // the index (ADR-0002 §7).
        let sha = sha256_hex(&make_tar("a.typl", b"package veh.common\n"));
        assert_eq!(
            cache.lookup("https://elsewhere.example.com/veh/common@v1.0.0", &sha),
            None,
            "the cache is indexed by URL as well as content hash",
        );
    }

    /// A tar carrying a parent-directory-escaping entry (`../poison`) must not
    /// write anything outside the entry directory. The `tar` crate skips such
    /// entries on unpack today; this test locks the security property so a
    /// future swap of the unpack mechanism cannot silently reintroduce path
    /// traversal.
    #[test]
    fn store_does_not_let_a_tar_escape_the_entry_directory() {
        let dir = TempDir::new("traversal");
        let cache = cache(&dir);
        let url = "https://registry.example.com/veh/evil@v1.0.0";

        // Sanity: the hand-crafting is valid, so a benign raw-name tar unpacks.
        // This proves the escaping case below is stopped by the traversal guard,
        // not by a malformed archive the reader rejects outright.
        let (_, benign_path) = cache
            .store(
                "https://registry.example.com/veh/benign@v1.0.0",
                &tar_with_raw_name("benign.typl", b"package veh.benign\n"),
            )
            .expect("a benign hand-crafted tar stores");
        assert!(
            benign_path.join("benign.typl").is_file(),
            "the hand-crafted tar format is valid and unpacks",
        );

        // The tar builder refuses to *write* a `..` path, so the malicious
        // archive is hand-crafted to reach the unpack-side guard.
        let tar = tar_with_raw_name("../poison", b"pwned");

        // Storing may succeed (skipping the escaping entry) or error; neither
        // outcome may write outside the entry directory.
        let _ = cache.store(url, &tar);
        assert!(
            !contains_file_named(dir.path(), "poison"),
            "an escaping tar entry must never be written anywhere in the cache tree",
        );
    }

    /// Hand-builds a single-entry POSIX tar whose file name is written verbatim
    /// — bypassing the `tar` builder's refusal to emit `..` paths — so a
    /// path-traversal archive can be constructed for the unpack guard test.
    fn tar_with_raw_name(name: &str, contents: &[u8]) -> Vec<u8> {
        let mut header = [0u8; 512];
        let name_bytes = name.as_bytes();
        header[..name_bytes.len()].copy_from_slice(name_bytes);
        header[100..108].copy_from_slice(b"0000644\0"); // mode
        header[108..116].copy_from_slice(b"0000000\0"); // uid
        header[116..124].copy_from_slice(b"0000000\0"); // gid
        header[124..136].copy_from_slice(format!("{:011o}\0", contents.len()).as_bytes()); // size
        header[136..148].copy_from_slice(b"00000000000\0"); // mtime
        header[156] = b'0'; // typeflag: regular file
        header[257..263].copy_from_slice(b"ustar\0"); // magic
        header[263..265].copy_from_slice(b"00"); // version

        // Checksum: sum every header byte with the checksum field taken as
        // spaces, then write it as six octal digits, a NUL, and a space.
        for byte in &mut header[148..156] {
            *byte = b' ';
        }
        let checksum: u32 = header.iter().map(|&byte| u32::from(byte)).sum();
        header[148..156].copy_from_slice(format!("{checksum:06o}\0 ").as_bytes());

        let mut out = Vec::new();
        out.extend_from_slice(&header);
        out.extend_from_slice(contents);
        // Pad the data to a 512-byte boundary, then end with two zero blocks.
        out.resize(out.len() + (512 - contents.len() % 512) % 512, 0);
        out.resize(out.len() + 1024, 0);
        out
    }

    /// Whether any file named `name` exists anywhere under `root`.
    fn contains_file_named(root: &Path, name: &str) -> bool {
        let mut stack = vec![root.to_path_buf()];
        while let Some(dir) = stack.pop() {
            let Ok(entries) = fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.file_name().is_some_and(|found| found == name) {
                    return true;
                }
                if path.is_dir() {
                    stack.push(path);
                }
            }
        }
        false
    }

    /// Re-storing already-cached content is a no-op that returns the existing
    /// directory with the same hash.
    #[test]
    fn store_is_idempotent() {
        let dir = TempDir::new("idempotent");
        let cache = cache(&dir);
        let url = "https://registry.example.com/veh/common@v1.0.0";
        let tar = make_tar("a.typl", b"package veh.common\n");

        let (sha1, path1) = cache.store(url, &tar).expect("first store");
        let (sha2, path2) = cache.store(url, &tar).expect("second store");
        assert_eq!(sha1, sha2, "the same bytes hash the same");
        assert_eq!(path1, path2, "the same entry directory is returned");
    }
}
