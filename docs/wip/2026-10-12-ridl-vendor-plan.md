# Remote units and `ridl vendor` Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make a remote `[imports]` unit load, resolve and generate code (PR 1),
then add `vendor/` and `ridl vendor` so the same build runs without a network
(PR 2).

**Architecture:** The loader in `ridl-core` (`workspace.rs`) gains a second
entry point, `load_workspace_with_remotes`, which loads the consumer as today,
takes the union of its `[imports]`, materializes each URL (vendor directory,
then cache, then fetch) through `materialize_imports`, verifies the `tree` hash
of each unpacked tree, and loads each remote unit's tree into the same salsa
`Workspace` as packages whose origin is `PackageOrigin::Remote { url }`. The
resolver in `ridl-sem` binds `PackageSource::Remote` to the package loaded for
that URL. `ridlc` runs the materialization before the check instead of after it,
drops a remote unit's Warnings and Infos once per run, and keeps remote packages
out of IR dumps, catalogs and the baseline chain while giving them to the code
backends. PR 2 reserves `<manifest root>/vendor/`, reads it before the cache,
and adds `ridl vendor` and `ridl vendor --check` over a new `ridl-core`
`vendor.rs`.

**Tech Stack:** Rust 2024 (the pinned toolchain in `rust-toolchain.toml`), salsa
inputs (`Package`, `Workspace`), `sha2`, `tar`, `ureq` behind the `ridl-core`
`fetch` feature, `clap` for the CLI, `insta` not needed.

**Spec:** `docs/wip/2026-10-12-ridl-vendor-design.md`

## Global Constraints

- Decisions V-1 to V-9 of the spec are settled; this plan implements them and
  changes none.
- Gate: `just build` locally before each PR; the members an implementer runs per
  task are `just fmt-check`, `just lint`, `just test`, `just doc-path-check`,
  `just story-id-check`, `just book-check`, `just link-check` and
  `just freeze-check`.
- No story id (`E4.6`) and no plan name (`epic E4`, `lane D`) in a tracked file
  under `crates/`, `xtask/`, `docs/book/`, `docs/design/`, `docs/technotes/`,
  nor in rustdoc; `just story-id-check` enforces it.
- Prose — comments, commit messages, docs, PR descriptions — is plain and
  literal English: no idioms, no figures of speech.
- Conventional Commits, scopes from `.git-std.toml`: `ridl-core`, `ridl-sem`,
  `ridlc`, `ridl`, `ridl-lsp`, `ridl-mcp`, `xtask`, `adr`, `docs`, `family`.
- Every new `MANI-` code goes into both `MANI_CATALOG`
  (`crates/ridl-core/src/diag.rs`) and the table of
  `docs/specification/ridl-family-overview.md` section 7.2; the test
  `the_overview_form_and_mani_tables_mirror_the_catalogues` fails otherwise.
- Every Warning code carries a lint name (ADR-0024); the test
  `form_and_mani_severities_follow_their_namespace_rule` lists each MANI Warning
  by hand and must be edited for MANI-018.
- `cargo xtask freeze update` after every change that alters a dump
  (`freeze/diagnostics.txt`, `freeze/manifest.txt`, `freeze/cli.txt`), and the
  regenerated dumps are committed. Do NOT add a row to `freeze/surfaces.toml`.
- Each PR is breaking: `!` in the PR title, a heads-up issue on
  driftsys/ridlc-gen-kotlin, and the delegated ruling recorded in the PR body.
- Never force-push.
- Every commit ends with
  `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- Shipped docs describe what is built: PR 1 documents the read path only, PR 2
  documents `vendor/`; nothing in `docs/book/` or the ADRs describes PR 2 before
  it lands.
- No `Id:` is hand-written anywhere; no MarkSpec entries are involved.

## Review Focus

Five input classes the spec implies and no existing test exercises, most likely
to bite first:

1. **A symbolic link inside a remote tree** (cache entry or `vendor/<key>/`).
   Expected: MANI-107 naming the link, and the unit is not loaded. Pinned in
   Task 1 (`tree_hash` refuses a symlink) and Task 9 (the read path reports it
   as MANI-107).
2. **Two `[imports]` keys that name one URL.** Expected: the unit is fetched and
   loaded once; the key that does not equal the unit's `[package] name` draws
   MANI-105 and binds nothing; the other key resolves. Pinned in Task 3.
3. **A remote unit that imports one of the consumer's own members.** Expected:
   the import binds (ADR-0002 §5 step 1), and when the member imports the remote
   unit back, TYPL-004 is reported as the remote unit's error and fails the
   check. Pinned in Task 4.
4. **A file whose bytes differ only by line ends** (a checkout with
   `core.autocrlf`, or an editor that rewrote a vendored file with CRLF).
   Expected: the `tree` hash is over bytes, so the rewritten tree is MANI-107
   and the stale copy is never compiled silently. Pinned in Task 1 (the hash
   covers content bytes) and Task 11 (`ridl vendor --check` reports it).
5. **A `ridl vendor` interrupted between staging and rename**, leaving
   `vendor/.staging-*`. Expected: the next run removes the stale staging
   directory first, and `vendor/<key>/` is at every moment either the old tree
   or the new one, never a partial copy. Pinned in Task 10.

---

## Part 1 — PR 1, the read path (spec section 4)

Branch: `feat/ridl-vendor`, from `cea41fdc`. PR title:
`feat(ridl-core)!: load, resolve and generate remote units`.

### Task 1: The `tree` hash

**Files:**

- Create: `crates/ridl-core/src/tree_hash.rs`
- Modify: `crates/ridl-core/src/lib.rs:14-30` (module list) and `:32-42`
  (re-exports)

**Interfaces:**

- Consumes: `sha2::Sha256` (already an optional dependency under the `fetch`
  feature, `crates/ridl-core/Cargo.toml:49,58`).
- Produces:

  ```rust
  /// Why a tree could not be hashed.
  #[derive(Debug)]
  pub enum TreeHashError {
      /// A symbolic link or another non-regular file in the tree.
      NotRegular(PathBuf),
      /// A directory or file that could not be read.
      Io { path: PathBuf, error: std::io::Error },
  }
  impl std::fmt::Display for TreeHashError { /* see step 3 */ }
  impl std::error::Error for TreeHashError {}

  /// The `tree` hash of the unit at `dir` (spec section 4.5).
  pub fn tree_hash(dir: &Path) -> Result<String, TreeHashError>;
  ```

- [ ] **Step 1: Write the failing tests**

Create `crates/ridl-core/src/tree_hash.rs` with only the test module for now:

```rust
//! The `tree` hash of a remote unit's directory (ADR-0002 §7): one SHA-256
//! over every regular file under the directory, so that `ridl.lock` pins the
//! unpacked tree beside the fetched tar, and `vendor/` can be verified
//! without the tar.
//!
//! This module sits behind the `fetch` feature: it hashes with `sha2`, which
//! only the fetch feature pulls in.

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// A unique directory under the system temp dir, removed on drop.
    struct TempDir(PathBuf);

    impl TempDir {
        fn new(label: &str) -> Self {
            static COUNTER: AtomicUsize = AtomicUsize::new(0);
            let mut path = std::env::temp_dir();
            path.push(format!(
                "ridl-core-tree-hash-{label}-{}-{}",
                std::process::id(),
                COUNTER.fetch_add(1, Ordering::SeqCst),
            ));
            fs::create_dir_all(&path).expect("create the temp dir");
            Self(path)
        }
        fn path(&self) -> &Path {
            &self.0
        }
        fn write(&self, relative: &str, bytes: &[u8]) {
            let path = self.0.join(relative);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, bytes).unwrap();
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    /// An empty directory hashes to the SHA-256 of the empty input, the
    /// known NIST value, so the construction feeds nothing but the files.
    #[test]
    fn an_empty_tree_hashes_to_the_empty_digest() {
        let dir = TempDir::new("empty");
        assert_eq!(
            tree_hash(dir.path()).unwrap(),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
        );
    }

    /// The hash is the SHA-256 of, per file in byte order of its `/`
    /// separated relative path: the path bytes, one NUL, the length as a
    /// little-endian u64, the bytes. Computed by hand here, so the function
    /// is checked against the definition and not against itself.
    #[test]
    fn the_hash_follows_the_definition() {
        use sha2::{Digest, Sha256};
        let dir = TempDir::new("definition");
        dir.write("ridl.toml", b"[package]\nname = \"r.u\"\n");
        dir.write("sub/b.typl", b"package r.u.sub\n");
        dir.write("a.typl", b"package r.u\n");

        let mut hasher = Sha256::new();
        for (path, bytes) in [
            ("a.typl", &b"package r.u\n"[..]),
            ("ridl.toml", &b"[package]\nname = \"r.u\"\n"[..]),
            ("sub/b.typl", &b"package r.u.sub\n"[..]),
        ] {
            hasher.update(path.as_bytes());
            hasher.update([0u8]);
            hasher.update((bytes.len() as u64).to_le_bytes());
            hasher.update(bytes);
        }
        let expected: String = hasher
            .finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        assert_eq!(tree_hash(dir.path()).unwrap(), expected);
    }

    /// Creation order does not matter: two trees with the same files hash
    /// the same.
    #[test]
    fn creation_order_does_not_change_the_hash() {
        let first = TempDir::new("order-1");
        first.write("z.typl", b"z");
        first.write("a.typl", b"a");
        let second = TempDir::new("order-2");
        second.write("a.typl", b"a");
        second.write("z.typl", b"z");
        assert_eq!(
            tree_hash(first.path()).unwrap(),
            tree_hash(second.path()).unwrap()
        );
    }

    /// The hash covers the content bytes: a file rewritten with CRLF line
    /// ends is a different tree.
    #[test]
    fn line_ends_change_the_hash() {
        let lf = TempDir::new("lf");
        lf.write("a.typl", b"package r.u\ntype A: m\n");
        let crlf = TempDir::new("crlf");
        crlf.write("a.typl", b"package r.u\r\ntype A: m\r\n");
        assert_ne!(
            tree_hash(lf.path()).unwrap(),
            tree_hash(crlf.path()).unwrap()
        );
    }

    /// A symbolic link anywhere in the tree is refused, naming the link.
    #[cfg(unix)]
    #[test]
    fn a_symlink_is_refused_by_name() {
        let dir = TempDir::new("symlink");
        dir.write("a.typl", b"package r.u\n");
        std::os::unix::fs::symlink(dir.path().join("a.typl"), dir.path().join("sub-link"))
            .unwrap();
        match tree_hash(dir.path()) {
            Err(TreeHashError::NotRegular(path)) => {
                assert_eq!(path, dir.path().join("sub-link"));
            }
            other => panic!("a symlink is NotRegular, got {other:?}"),
        }
    }

    /// A directory that does not exist is an `Io` error naming it.
    #[test]
    fn a_missing_directory_is_an_io_error() {
        let dir = TempDir::new("missing");
        let gone = dir.path().join("gone");
        match tree_hash(&gone) {
            Err(TreeHashError::Io { path, .. }) => assert_eq!(path, gone),
            other => panic!("a missing directory is Io, got {other:?}"),
        }
    }
}
```

Add to `crates/ridl-core/src/lib.rs` after `pub mod std_lib;`:

```rust
#[cfg(feature = "fetch")]
pub mod tree_hash;
```

and after the `fetch` re-export:

```rust
#[cfg(feature = "fetch")]
pub use tree_hash::{TreeHashError, tree_hash};
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p ridl-core --lib tree_hash::` Expected: compile error,
`cannot find function tree_hash` and `TreeHashError` not found.

- [ ] **Step 3: Implement `tree_hash`**

Add above the test module:

```rust
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

/// Why a tree could not be hashed.
#[derive(Debug)]
pub enum TreeHashError {
    /// A symbolic link or another non-regular file in the tree. A remote
    /// unit holds regular files only, so the tree is not loaded.
    NotRegular(PathBuf),
    /// A directory or file that could not be read.
    Io { path: PathBuf, error: io::Error },
}

impl fmt::Display for TreeHashError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotRegular(path) => write!(
                f,
                "`{}` is not a regular file; a remote unit holds regular files only",
                path.display()
            ),
            Self::Io { path, error } => write!(f, "cannot read `{}`: {error}", path.display()),
        }
    }
}

impl std::error::Error for TreeHashError {}

/// The `tree` hash of the unit at `dir` (ADR-0002 §7): the SHA-256, in
/// lowercase hex, fed for every regular file under `dir`, in ascending byte
/// order of its relative path written with `/` separators, with the path
/// bytes, one NUL byte, the file length as a little-endian u64, and the file
/// bytes. A symbolic link or any other non-regular entry is
/// [`TreeHashError::NotRegular`]; a directory is entered, never hashed itself.
pub fn tree_hash(dir: &Path) -> Result<String, TreeHashError> {
    let mut files: Vec<(Vec<u8>, PathBuf)> = Vec::new();
    collect(dir, dir, &mut files)?;
    files.sort_by(|a, b| a.0.cmp(&b.0));
    let mut hasher = Sha256::new();
    for (relative, path) in files {
        let bytes = fs::read(&path).map_err(|error| TreeHashError::Io {
            path: path.clone(),
            error,
        })?;
        hasher.update(&relative);
        hasher.update([0u8]);
        hasher.update((bytes.len() as u64).to_le_bytes());
        hasher.update(&bytes);
    }
    Ok(hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

/// Appends every regular file under `dir` to `files` as its relative path
/// bytes (`/` separated) and its full path. `symlink_metadata` is read so a
/// link is seen as a link and never followed.
fn collect(root: &Path, dir: &Path, files: &mut Vec<(Vec<u8>, PathBuf)>) -> Result<(), TreeHashError> {
    let entries = fs::read_dir(dir).map_err(|error| TreeHashError::Io {
        path: dir.to_path_buf(),
        error,
    })?;
    for entry in entries {
        let entry = entry.map_err(|error| TreeHashError::Io {
            path: dir.to_path_buf(),
            error,
        })?;
        let path = entry.path();
        let kind = fs::symlink_metadata(&path)
            .map_err(|error| TreeHashError::Io {
                path: path.clone(),
                error,
            })?
            .file_type();
        if kind.is_dir() {
            collect(root, &path, files)?;
        } else if kind.is_file() {
            let relative = path
                .strip_prefix(root)
                .expect("every entry is under the root")
                .components()
                .map(|component| component.as_os_str().as_encoded_bytes().to_vec())
                .collect::<Vec<Vec<u8>>>()
                .join(&b'/');
            files.push((relative, path));
        } else {
            return Err(TreeHashError::NotRegular(path));
        }
    }
    Ok(())
}
```

Note: `OsStr::as_encoded_bytes` gives the path bytes on Unix and WTF-8 on
Windows, so a file name that is not UTF-8 is hashed by its bytes and never by a
lossy string.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p ridl-core --lib tree_hash::` Expected: 6 passed (5 on
Windows, where the symlink test is skipped).

- [ ] **Step 5: Commit**

```bash
git add crates/ridl-core/src/tree_hash.rs crates/ridl-core/src/lib.rs
git commit -m "feat(ridl-core): hash a remote unit's tree

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

### Task 2: `ridl.lock` pins the tree, and the four new codes

**Files:**

- Modify: `crates/ridl-core/src/lock.rs:11-16` (module doc), `:50-55`
  (`LockEntry`), `:149-153` (test helper)
- Modify: `crates/ridl-core/src/fetch.rs:106-225` (`materialize_imports`),
  `:455-457` (`imports_of`), `:496-653` (tests)
- Modify: `crates/ridl-core/src/workspace.rs:34` (imports) and after `:86` (the
  `ImportUnion` type)
- Modify: `crates/ridl-core/src/diag.rs:1405-1410` (`MANI_CATALOG`)
- Modify: `docs/specification/ridl-family-overview.md:358` (the MANI table)
- Modify: `crates/ridlc/src/lib.rs:1984-2003` (the one external caller of
  `materialize_imports`; rewritten again in Task 5, patched here so the
  workspace compiles)

**Interfaces:**

- Consumes: `tree_hash` (Task 1).
- Produces:

  ```rust
  // crates/ridl-core/src/workspace.rs
  /// The union of a consumer's `[imports]`: every URL to the keys that name it.
  pub type ImportUnion = BTreeMap<String, BTreeSet<String>>;

  // crates/ridl-core/src/lock.rs
  pub struct LockEntry {
      pub sha256: String,
      /// The `tree` hash of the unpacked tree (Task 1). `None` in a lockfile
      /// written before the hash existed.
      #[serde(default, skip_serializing_if = "Option::is_none")]
      pub tree: Option<String>,
  }
  /// `<key> <type>` lines of `ridl.lock`, for the manifest freeze dump.
  pub const LOCK_KEYS: &[(&str, &str)] = &[
      ("entries.*.sha256", "string"),
      ("entries.*.tree", "string"),
  ];

  // crates/ridl-core/src/fetch.rs
  /// One remote unit the read path found: where its tree is and its hash.
  #[derive(Debug, Clone, PartialEq, Eq)]
  pub struct MaterializedUnit { pub url: String, pub dir: PathBuf, pub tree: String }
  pub fn materialize_imports(
      imports: &ImportUnion,
      lock: Option<&Lockfile>,
      cache: &Cache,
      frozen: Frozen,
  ) -> (Vec<MaterializedUnit>, Lockfile, Vec<Diagnostic>);
  ```

  New codes: `DiagCode::MANI_105`, `MANI_106`, `MANI_107`, `MANI_108`, all
  `Error`.

- [ ] **Step 1: Write the failing tests**

In `crates/ridl-core/src/fetch.rs` tests, replace `imports_of` and add tests:

```rust
    fn imports_of(url: &str) -> ImportUnion {
        BTreeMap::from([(
            url.to_string(),
            BTreeSet::from(["remote.pkg".to_string()]),
        )])
    }

    /// A fetch pins both hashes: the tar's `sha256` and the unpacked `tree`,
    /// and the returned unit carries the same `tree`.
    #[test]
    fn a_fetch_pins_the_tree_hash() {
        let stub = Stub::serving(make_tar("remote.typl", b"package remote.pkg\n"));
        let dir = TempDir::new("tree-pin");
        let cache = cache(&dir);
        let (units, lock, diags) =
            materialize_imports(&imports_of(stub.url()), None, &cache, Frozen::No);
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
        let (_, lock, _) = materialize_imports(&imports_of(stub.url()), None, &cache, Frozen::No);
        let mut old = lock.clone();
        old.entries.get_mut(stub.url()).unwrap().tree = None;
        stub.reset_hits();
        let (_, regenerated, diags) =
            materialize_imports(&imports_of(stub.url()), Some(&old), &cache, Frozen::No);
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
        let (_, lock, _) = materialize_imports(&imports_of(stub.url()), None, &cache, Frozen::No);
        let mut old = lock;
        old.entries.get_mut(stub.url()).unwrap().tree = None;
        let (units, _, diags) =
            materialize_imports(&imports_of(stub.url()), Some(&old), &cache, Frozen::Yes);
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
            materialize_imports(&imports_of(stub.url()), None, &cache, Frozen::No);
        std::fs::write(units[0].dir.join("extra.typl"), b"package remote.pkg\n").unwrap();
        for frozen in [Frozen::Yes, Frozen::No] {
            let (units, _, diags) =
                materialize_imports(&imports_of(stub.url()), Some(&lock), &cache, frozen);
            assert_eq!(codes(&diags), vec!["MANI-107"], "{frozen:?}");
            assert!(units.is_empty());
        }
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
            materialize_imports(&imports_of(stub.url()), None, &cache, Frozen::No);
        std::os::unix::fs::symlink("remote.typl", units[0].dir.join("link.typl")).unwrap();
        let (units, _, diags) =
            materialize_imports(&imports_of(stub.url()), Some(&lock), &cache, Frozen::Yes);
        assert_eq!(codes(&diags), vec!["MANI-107"]);
        assert!(diags[0].message.contains("link.typl"), "{}", diags[0].message);
        assert!(units.is_empty());
    }
```

Update the existing tests for the new return type: in
`materialize_unpacks_a_remote_package_into_the_cache` replace
`let (resolved_url, path) = &resolved[0]; assert_eq!(resolved_url, stub.url());`
with `assert_eq!(resolved[0].url, stub.url()); let path = &resolved[0].dir;`. In
`a_hash_mismatch_is_mani_102` and `frozen_pinned_but_uncached_is_mani_104` build
the entry as `LockEntry { sha256: "0".repeat(64), tree: None }`. In
`an_invalid_url_is_mani_101_without_fetching` build the union as
`BTreeMap::from([("not-a-url".to_string(), BTreeSet::from(["bad.dep".to_string()]))])`.
In `crates/ridl-core/src/lock.rs` tests, `entry()` builds
`LockEntry { sha256: sha.to_string(), tree: None }` and add:

```rust
    /// A lockfile written before `tree` existed reads with `tree: None`, and
    /// an entry with a tree round-trips it.
    #[test]
    fn tree_is_optional_on_read_and_kept_on_write() {
        let dir = TempDir::new("tree");
        let path = dir.path().join("ridl.lock");
        fs::write(
            &path,
            "[entries.\"https://r.example.com/a@v1\"]\nsha256 = \"aa\"\n",
        )
        .unwrap();
        let (lock, diags) = read_lockfile(&path);
        assert!(diags.is_empty());
        let lock = lock.unwrap();
        assert_eq!(lock.entries["https://r.example.com/a@v1"].tree, None);

        let mut lock = lock;
        lock.entries.get_mut("https://r.example.com/a@v1").unwrap().tree = Some("bb".into());
        write_lockfile(&path, &lock).unwrap();
        assert!(fs::read_to_string(&path).unwrap().contains("tree = \"bb\""));
        assert_eq!(read_lockfile(&path).0, Some(lock));
    }
```

In `crates/ridl-core/src/diag.rs` tests, the catalogue mirror test needs no
edit; the overview table gets its rows in step 3.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p ridl-core --lib fetch:: lock::` Expected: compile errors —
`ImportUnion` not found, `LockEntry` has no field `tree`, `MaterializedUnit` not
found.

- [ ] **Step 3: Implement**

`crates/ridl-core/src/workspace.rs`: add `use std::collections::BTreeSet;` to
the `std::collections` import and, after the `LoadedWorkspace` struct:

```rust
/// The union of a consumer's `[imports]` (ADR-0002 §5): every URL, mapped to
/// the keys that name it across the root manifest and every member. Keyed by
/// URL so that one key pinned to two URLs by two members keeps both URLs, and
/// two keys naming one URL fetch it once.
pub type ImportUnion = BTreeMap<String, BTreeSet<String>>;
```

`crates/ridl-core/src/lock.rs`: the `LockEntry` and `LOCK_KEYS` from Interfaces;
extend the module doc's TOML example with `tree = "…"`.

`crates/ridl-core/src/diag.rs`, after `MANI_104`:

```rust
        /// A remote unit's tree holds no `ridl.toml` at its top level, the
        /// manifest is a `[workspace]` manifest, or its `[package] name` is
        /// not the `[imports]` key that names its URL (ADR-0002 §5).
        MANI_105 = "MANI-105", Error,
            "a remote unit has no top-level `[package]` manifest, or its name is not the `[imports]` key";

        /// A key of a remote unit's `[imports]` is not a key of the
        /// consumer's `[imports]`; the consumer lists every remote unit the
        /// build needs (ADR-0002 §5).
        MANI_106 = "MANI-106", Error,
            "a remote unit imports a key the consumer's `[imports]` does not name";

        /// The tree of a remote unit does not hash to the `tree` its
        /// `ridl.lock` entry pins, or holds a file that is not a regular file
        /// (ADR-0002 §7).
        MANI_107 = "MANI-107", Error,
            "a remote unit's tree does not match the `tree` hash in `ridl.lock`";

        /// `--frozen` was requested and the `ridl.lock` entry has no `tree`;
        /// a frozen run never rewrites the lockfile (ADR-0002 §7).
        MANI_108 = "MANI-108", Error,
            "`--frozen`: a `ridl.lock` entry has no `tree` hash";
```

`docs/specification/ridl-family-overview.md`, after the MANI-104 row:

```markdown
| MANI-105 | a remote unit has no top-level `[package]` manifest, or its name is not the `[imports]` key | error |
| MANI-106 | a remote unit imports a key the consumer's `[imports]` does not name | error |
| MANI-107 | a remote unit's tree does not match the `tree` hash in `ridl.lock` | error |
| MANI-108 | `--frozen`: a `ridl.lock` entry has no `tree` hash | error |
```

`crates/ridl-core/src/fetch.rs`: replace `materialize_imports` with:

```rust
use crate::tree_hash::{TreeHashError, tree_hash};
use crate::workspace::ImportUnion;

/// One remote unit the read path found: its URL, the directory that holds its
/// tree, and the `tree` hash the directory was verified against.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaterializedUnit {
    pub url: String,
    pub dir: PathBuf,
    pub tree: String,
}

pub fn materialize_imports(
    imports: &ImportUnion,
    lock: Option<&Lockfile>,
    cache: &Cache,
    frozen: Frozen,
) -> (Vec<MaterializedUnit>, Lockfile, Vec<Diagnostic>) {
    let mut units = Vec::new();
    let mut regenerated = Lockfile::default();
    let mut diagnostics = Vec::new();

    // The union is keyed by URL, so a URL named by two keys is materialized
    // once, in URL order.
    for url in imports.keys() {
        if !is_fetchable_url(url) {
            diagnostics.push(detached(
                DiagCode::MANI_101,
                format!("cannot fetch `{url}`: not a fetchable `http(s)` URL"),
            ));
            continue;
        }
        let pinned = lock.and_then(|lock| lock.entries.get(url));
        if let Some((entry, unit)) = from_cache_or_fetch(url, pinned, cache, frozen, &mut diagnostics) {
            regenerated.entries.insert(url.clone(), entry);
            units.push(unit);
        }
    }
    (units, regenerated, diagnostics)
}

/// The cache-or-fetch step for one URL: the verified unit and the lock entry
/// to write, or `None` after a diagnostic was pushed.
///
/// A cache hit against the pinned `sha256` needs no fetch. Its `tree` is then
/// verified (MANI-107) when the entry pins one; when the entry has none, a
/// non-frozen run computes and pins it, and a frozen run is MANI-108. A miss
/// fetches under [`Frozen::No`] and pins both hashes; under [`Frozen::Yes`] a
/// URL with no entry is MANI-103 and a pinned URL not in the cache is
/// MANI-104.
fn from_cache_or_fetch(
    url: &str,
    pinned: Option<&LockEntry>,
    cache: &Cache,
    frozen: Frozen,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<(LockEntry, MaterializedUnit)> {
    if let Some(entry) = pinned
        && let Some(dir) = cache.lookup(url, &entry.sha256)
    {
        let tree = match &entry.tree {
            Some(tree) => {
                verify_tree(url, &dir, tree, diagnostics)?;
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
        return Some((
            LockEntry {
                sha256: entry.sha256.clone(),
                tree: Some(tree.clone()),
            },
            MaterializedUnit {
                url: url.to_string(),
                dir,
                tree,
            },
        ));
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
                        "`{url}` is pinned in `ridl.lock` but not in the cache; run `ridl check` without `--frozen` to fetch it"
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
            // hash, never in the pinned entry's directory.
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
            let tree = hash_tree(url, &dir, diagnostics)?;
            Some((
                LockEntry {
                    sha256: sha,
                    tree: Some(tree.clone()),
                },
                MaterializedUnit {
                    url: url.to_string(),
                    dir,
                    tree,
                },
            ))
        }
    }
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

/// `Some(())` when `dir` hashes to `expected`; `None` after a MANI-107.
fn verify_tree(
    url: &str,
    dir: &Path,
    expected: &str,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<()> {
    let actual = hash_tree(url, dir, diagnostics)?;
    if actual == expected {
        return Some(());
    }
    diagnostics.push(detached(
        DiagCode::MANI_107,
        format!(
            "the tree of `{}` does not match the `tree` hash `ridl.lock` pins for `{url}`",
            dir.display()
        ),
    ));
    None
}
```

Add `use std::path::Path;` to the module imports. Update the function's doc
comment to the rules above (cache hit, tree verification, MANI-107/108). Keep
the `--frozen` wording out of the MANI-103 and MANI-104 messages' head so the
language server and the MCP server, which run the same frozen path, read
correctly; the fix the message names is `ridl check`.

`crates/ridlc/src/lib.rs:1984-2003` (`materialize_and_lock`): build the union as
`ImportUnion` (`imports.entry(url.clone()).or_default().insert(name.clone())`
over `workspace.imports(db)` and each package's `imports(db)`), bind the first
element of the result as `_units`, and leave the rest unchanged. Task 5 deletes
this function.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p ridl-core --lib fetch:: lock:: diag::` and
`cargo test -p ridlc --test cli frozen` Expected: all pass, including
`the_overview_form_and_mani_tables_mirror_the_catalogues`.

- [ ] **Step 5: Regenerate the diagnostics dump and commit**

Run: `cargo xtask freeze update` and check that `git diff --stat` touches
`freeze/diagnostics.txt` only (four added `MANI-10x error` lines).

```bash
git add crates/ridl-core/src crates/ridlc/src/lib.rs docs/specification/ridl-family-overview.md freeze/diagnostics.txt
git commit -m "feat(ridl-core)!: pin the unpacked tree in ridl.lock

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

### Task 3: The loader loads a remote unit

**Files:**

- Modify: `crates/ridl-core/src/package.rs:27-37` (`PackageOrigin`)
- Modify: `crates/ridl-core/src/workspace.rs:58-86` (`LoadedWorkspace`),
  `:206-286` (`load_workspace_with`), `:503-558` (`TreeScope`, `Loader`),
  `:623-671` (`load_root`), `:753-797` (`load_member`), `:823-930`
  (`load_package_tree`), and new functions
- Modify: `crates/ridl-core/src/lib.rs:50-53` (re-exports)
- Modify: `crates/ridlc/src/lib.rs:133-146` (the `LoadedWorkspace` literal in
  `front_end`) and `crates/ridl-lsp/src/server.rs:332-341` (the destructuring
  pattern) — both gain `remote_units: _` / `remote_units: BTreeMap::new()` and
  `imports: _` / `imports: BTreeMap::new()`
- Test: `crates/ridl-core/src/workspace.rs` test module (new tests)

**Interfaces:**

- Consumes: `materialize_imports`, `MaterializedUnit`, `ImportUnion` (Task 2),
  `Cache`, `Frozen`, `read_lockfile`, `write_lockfile`.
- Produces:

  ```rust
  // package.rs
  #[derive(Debug, Clone, PartialEq, Eq, Hash)]   // Copy is removed
  pub enum PackageOrigin {
      WorkspaceMember,
      /// Fetched from a remote URL, or read from the cache or `vendor/` for it.
      Remote { url: String },
      Std,
  }

  // workspace.rs
  /// How a load reaches remote units. Behind the `fetch` feature.
  #[derive(Clone, Copy)]
  pub struct RemoteAccess<'a> {
      /// `Frozen::Yes` never fetches and never writes `ridl.lock`.
      pub frozen: Frozen,
      pub cache: &'a Cache,
  }
  /// One remote unit a load took: its URL, its directory, its `tree` hash.
  #[derive(Debug, Clone, PartialEq, Eq)]
  pub struct RemoteUnit { pub url: String, pub dir: PathBuf, pub tree: String }

  pub struct LoadedWorkspace {
      /* existing fields */
      /// The union of the consumer's `[imports]` (ADR-0002 §5).
      pub imports: ImportUnion,
      /// The remote units loaded, keyed by unit name. Empty when the load
      /// took no `RemoteAccess`, in single-file mode, and for a consumer
      /// with no `[imports]`.
      pub remote_units: BTreeMap<String, RemoteUnit>,
  }

  #[cfg(feature = "fetch")]
  pub fn load_workspace_with_remotes(
      db: &mut RidlDatabase,
      entry: &Path,
      overlays: &[Overlay],
      access: RemoteAccess<'_>,
  ) -> Result<LoadedWorkspace, LoadError>;
  ```

  `load_workspace` and `load_workspace_with` keep their signatures and load no
  remote unit (they fill `imports` and leave `remote_units` empty).

- [ ] **Step 1: Write the failing tests**

Append to the test module of `crates/ridl-core/src/workspace.rs` (the module
already has a `TempDir`-style fixture; reuse it, named `dir` below as
`fixture.path()` — adapt the helper names to the ones the module defines):

```rust
    #[cfg(feature = "fetch")]
    mod remote {
        use super::*;
        use crate::cache::Cache;
        use crate::fetch::Frozen;
        use crate::lock::{LockEntry, Lockfile, write_lockfile};
        use crate::package::PackageOrigin;
        use crate::tree_hash::tree_hash;
        use std::collections::BTreeSet;

        /// An uncompressed tar holding `files` at the paths given, the
        /// provisional fetch artifact.
        fn unit_tar(files: &[(&str, &str)]) -> Vec<u8> {
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

        /// Stores `files` as the unit at `url` in `cache` and writes a
        /// `ridl.lock` at `root` that pins both hashes, so a frozen load
        /// reads it with no network.
        fn seed(cache: &Cache, root: &Path, url: &str, files: &[(&str, &str)]) {
            let (sha, dir) = cache.store(url, &unit_tar(files)).unwrap();
            let mut lock = Lockfile::default();
            lock.entries.insert(
                url.to_string(),
                LockEntry {
                    sha256: sha,
                    tree: Some(tree_hash(&dir).unwrap()),
                },
            );
            write_lockfile(&root.join("ridl.lock"), &lock).unwrap();
        }

        const URL: &str = "https://registry.example.com/veh/common@v1.0.0";

        fn consumer(fixture: &TempDir) -> PathBuf {
            fixture.write(
                "app/ridl.toml",
                &format!(
                    "[package]\nname = \"app\"\nversion = \"1.0.0\"\n\n[imports]\n\"veh.common\" = \"{URL}\"\n"
                ),
            );
            fixture.write("app/app.ridl", "package app\nimport veh.common.Speed\n");
            fixture.path().join("app")
        }

        fn load(fixture: &TempDir, root: &Path) -> (RidlDatabase, LoadedWorkspace) {
            let cache = Cache { root: fixture.path().join("cache") };
            let mut db = RidlDatabase::default();
            let loaded = load_workspace_with_remotes(
                &mut db,
                root,
                &[],
                RemoteAccess { frozen: Frozen::Yes, cache: &cache },
            )
            .unwrap();
            (db, loaded)
        }

        fn codes(loaded: &LoadedWorkspace) -> Vec<&str> {
            loaded.diagnostics.iter().map(|d| d.code.as_str()).collect()
        }

        /// A cached unit loads as packages whose origin names the URL, under
        /// the unit's name, with the unit recorded in `remote_units` and not
        /// in `units`.
        #[test]
        fn a_remote_unit_loads_from_the_cache() {
            let fixture = TempDir::new("remote-load");
            let root = consumer(&fixture);
            let cache = Cache { root: fixture.path().join("cache") };
            seed(&cache, &root, URL, &[
                ("ridl.toml", "[package]\nname = \"veh.common\"\nversion = \"1.0.0\"\n"),
                ("common.typl", "package veh.common\ntype Speed: km/h\n"),
                ("types/types.typl", "package veh.common.types\ntype Temp: degC\n"),
            ]);
            let (db, loaded) = load(&fixture, &root);
            assert_eq!(codes(&loaded), Vec::<&str>::new());
            let names: Vec<String> = loaded
                .workspace
                .packages(&db)
                .iter()
                .map(|p| p.name(&db).clone())
                .collect();
            assert_eq!(names, ["app", "veh.common", "veh.common.types"]);
            let remote = loaded.workspace.packages(&db)[1];
            assert_eq!(*remote.origin(&db), PackageOrigin::Remote { url: URL.to_string() });
            assert_eq!(remote.unit(&db), "veh.common");
            assert!(loaded.remote_units.contains_key("veh.common"));
            assert_eq!(loaded.units.keys().collect::<Vec<_>>(), ["app"]);
            assert_eq!(
                loaded.imports,
                BTreeMap::from([(URL.to_string(), BTreeSet::from(["veh.common".to_string()]))])
            );
        }

        /// A tree with no top-level `ridl.toml` is MANI-105 and loads nothing.
        #[test]
        fn a_tree_without_a_manifest_is_mani_105() {
            let fixture = TempDir::new("remote-105-none");
            let root = consumer(&fixture);
            let cache = Cache { root: fixture.path().join("cache") };
            seed(&cache, &root, URL, &[("common.typl", "package veh.common\n")]);
            let (db, loaded) = load(&fixture, &root);
            assert_eq!(codes(&loaded), ["MANI-105"]);
            assert_eq!(loaded.workspace.packages(&db).len(), 1);
            assert!(loaded.remote_units.is_empty());
        }

        /// A `[workspace]` manifest at the top is MANI-105.
        #[test]
        fn a_workspace_manifest_is_mani_105() {
            let fixture = TempDir::new("remote-105-ws");
            let root = consumer(&fixture);
            let cache = Cache { root: fixture.path().join("cache") };
            seed(&cache, &root, URL, &[("ridl.toml", "[workspace]\nmembers = []\n")]);
            let (_, loaded) = load(&fixture, &root);
            assert_eq!(codes(&loaded), ["MANI-105"]);
        }

        /// A name that is not the key is MANI-105, and the message names both.
        #[test]
        fn a_name_that_is_not_the_key_is_mani_105() {
            let fixture = TempDir::new("remote-105-name");
            let root = consumer(&fixture);
            let cache = Cache { root: fixture.path().join("cache") };
            seed(&cache, &root, URL, &[
                ("ridl.toml", "[package]\nname = \"veh.other\"\nversion = \"1.0.0\"\n"),
                ("other.typl", "package veh.other\n"),
            ]);
            let (_, loaded) = load(&fixture, &root);
            assert_eq!(codes(&loaded), ["MANI-105"]);
            let message = &loaded.diagnostics[0].message;
            assert!(message.contains("veh.other") && message.contains("veh.common"), "{message}");
        }

        /// Two keys for one URL: the unit loads once under its own name; the
        /// other key is MANI-105 (Review Focus 2).
        #[test]
        fn two_keys_for_one_url_load_the_unit_once_and_flag_the_other_key() {
            let fixture = TempDir::new("remote-two-keys");
            fixture.write(
                "app/ridl.toml",
                &format!(
                    "[package]\nname = \"app\"\nversion = \"1.0.0\"\n\n[imports]\n\"veh.common\" = \"{URL}\"\n\"veh.alias\" = \"{URL}\"\n"
                ),
            );
            fixture.write("app/app.ridl", "package app\n");
            let root = fixture.path().join("app");
            let cache = Cache { root: fixture.path().join("cache") };
            seed(&cache, &root, URL, &[
                ("ridl.toml", "[package]\nname = \"veh.common\"\nversion = \"1.0.0\"\n"),
                ("common.typl", "package veh.common\n"),
            ]);
            let (db, loaded) = load(&fixture, &root);
            assert_eq!(codes(&loaded), ["MANI-105"]);
            assert!(loaded.diagnostics[0].message.contains("veh.alias"));
            assert_eq!(loaded.workspace.packages(&db).len(), 2, "the unit still loads");
            assert_eq!(loaded.remote_units.len(), 1);
        }

        /// A key of the remote unit's `[imports]` the consumer does not name
        /// is MANI-106, naming the key and the remote unit's URL for it.
        #[test]
        fn an_uncovered_import_is_mani_106() {
            let fixture = TempDir::new("remote-106");
            let root = consumer(&fixture);
            let cache = Cache { root: fixture.path().join("cache") };
            seed(&cache, &root, URL, &[
                (
                    "ridl.toml",
                    "[package]\nname = \"veh.common\"\nversion = \"1.0.0\"\n\n[imports]\n\"veh.units\" = \"https://registry.example.com/veh/units@v2\"\n",
                ),
                ("common.typl", "package veh.common\n"),
            ]);
            let (_, loaded) = load(&fixture, &root);
            assert_eq!(codes(&loaded), ["MANI-106"]);
            let message = &loaded.diagnostics[0].message;
            assert!(message.contains("veh.units") && message.contains("veh/units@v2"), "{message}");
        }

        /// A remote unit's own `[imports]` key is read with the consumer's
        /// URL for that key (V-1), on every package of the unit.
        #[test]
        fn a_remote_unit_reads_its_imports_through_the_consumers_urls() {
            let fixture = TempDir::new("remote-v1");
            const UNITS_CONSUMER: &str = "https://registry.example.com/veh/units@v1";
            const UNITS_REMOTE: &str = "https://registry.example.com/veh/units@v2";
            fixture.write(
                "app/ridl.toml",
                &format!(
                    "[package]\nname = \"app\"\nversion = \"1.0.0\"\n\n[imports]\n\"veh.common\" = \"{URL}\"\n\"veh.units\" = \"{UNITS_CONSUMER}\"\n"
                ),
            );
            fixture.write("app/app.ridl", "package app\n");
            let root = fixture.path().join("app");
            let cache = Cache { root: fixture.path().join("cache") };
            let (sha_common, dir_common) = cache
                .store(URL, &unit_tar(&[
                    ("ridl.toml", &format!("[package]\nname = \"veh.common\"\nversion = \"1.0.0\"\n\n[imports]\n\"veh.units\" = \"{UNITS_REMOTE}\"\n")),
                    ("common.typl", "package veh.common\n"),
                ]))
                .unwrap();
            let (sha_units, dir_units) = cache
                .store(UNITS_CONSUMER, &unit_tar(&[
                    ("ridl.toml", "[package]\nname = \"veh.units\"\nversion = \"1.0.0\"\n"),
                    ("units.typl", "package veh.units\n"),
                ]))
                .unwrap();
            let mut lock = Lockfile::default();
            lock.entries.insert(URL.into(), LockEntry { sha256: sha_common, tree: Some(tree_hash(&dir_common).unwrap()) });
            lock.entries.insert(UNITS_CONSUMER.into(), LockEntry { sha256: sha_units, tree: Some(tree_hash(&dir_units).unwrap()) });
            write_lockfile(&root.join("ridl.lock"), &lock).unwrap();

            let (db, loaded) = load(&fixture, &root);
            assert_eq!(codes(&loaded), Vec::<&str>::new());
            let common = loaded.workspace.packages(&db)[1];
            assert_eq!(common.name(&db), "veh.common");
            assert_eq!(common.imports(&db).get("veh.units").map(String::as_str), Some(UNITS_CONSUMER));
        }

        /// A remote tree's Warnings are dropped and its Errors are kept with
        /// the unit named in the message (spec 4.4).
        #[test]
        fn a_remote_tree_keeps_errors_and_drops_warnings() {
            let fixture = TempDir::new("remote-severities");
            let root = consumer(&fixture);
            let cache = Cache { root: fixture.path().join("cache") };
            seed(&cache, &root, URL, &[
                ("ridl.toml", "[package]\nname = \"veh.common\"\nversion = \"1.0.0\"\nbogus = 1\n"),
                ("common.typl", "package veh.wrong\n"),
                ("sketch.rmdl", "package veh.common\n"),
            ]);
            let (_, loaded) = load(&fixture, &root);
            // MANI-005 (warning) and RIDL-417 (warning) are dropped; TYPL-002
            // (error) is kept.
            assert_eq!(codes(&loaded), ["TYPL-002"]);
            assert!(loaded.diagnostics[0].message.starts_with("remote unit `veh.common`: "));
        }

        /// A remote unit that declares a consumer's source package is
        /// MANI-014, as a second member would be.
        #[test]
        fn a_remote_unit_claiming_a_consumer_package_is_mani_014() {
            let fixture = TempDir::new("remote-014");
            fixture.write(
                "app/ridl.toml",
                &format!(
                    "[package]\nname = \"veh.common\"\nversion = \"1.0.0\"\n\n[imports]\n\"veh.common\" = \"{URL}\"\n"
                ),
            );
            fixture.write("app/c.typl", "package veh.common\n");
            let root = fixture.path().join("app");
            let cache = Cache { root: fixture.path().join("cache") };
            seed(&cache, &root, URL, &[
                ("ridl.toml", "[package]\nname = \"veh.common\"\nversion = \"1.0.0\"\n"),
                ("common.typl", "package veh.common\n"),
            ]);
            let (_, loaded) = load(&fixture, &root);
            assert_eq!(codes(&loaded), ["MANI-014"]);
        }

        /// A frozen load with no lock entry is MANI-103 and loads no remote
        /// unit; `ridl.lock` is not written.
        #[test]
        fn a_frozen_load_without_a_lock_entry_is_mani_103() {
            let fixture = TempDir::new("remote-103");
            let root = consumer(&fixture);
            let (_, loaded) = load(&fixture, &root);
            assert_eq!(codes(&loaded), ["MANI-103"]);
            assert!(!root.join("ridl.lock").exists());
        }

        /// The plain loader leaves the remote units alone: `load_workspace`
        /// fills `imports` and nothing else.
        #[test]
        fn load_workspace_takes_no_remote_unit() {
            let fixture = TempDir::new("remote-plain");
            let root = consumer(&fixture);
            let mut db = RidlDatabase::default();
            let loaded = load_workspace(&mut db, &root).unwrap();
            assert!(loaded.remote_units.is_empty());
            assert_eq!(loaded.imports.len(), 1);
            assert_eq!(loaded.workspace.packages(&db).len(), 1);
        }
    }
```

Add `tar` to `[dev-dependencies]` of `crates/ridl-core/Cargo.toml`
(`tar.workspace = true`) if the `fetch` tests' `make_tar` is not already visible
from this module — it is not (it lives in another test module), so add the
dev-dependency.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p ridl-core --lib workspace::tests::remote` Expected: compile
errors — `load_workspace_with_remotes`, `RemoteAccess`, `remote_units`,
`imports` and `PackageOrigin::Remote { url }` do not exist.

- [ ] **Step 3: Implement**

`crates/ridl-core/src/package.rs`: the `PackageOrigin` from Interfaces (drop
`Copy`, keep the other derives; the doc of `Remote` says "Fetched from a remote
URL, or read from the cache or `vendor/` for it"). No call site uses
`PackageOrigin` by copy; `*package.origin(&db)` comparisons keep compiling.

`crates/ridl-core/src/workspace.rs`:

1. Imports: add `#[cfg(feature = "fetch")] use crate::cache::Cache;`,
   `#[cfg(feature = "fetch")] use crate::fetch::{Frozen, MaterializedUnit, materialize_imports};`,
   `#[cfg(feature = "fetch")] use crate::lock::{read_lockfile, write_lockfile};`.
2. `LoadedWorkspace` gains `pub imports: ImportUnion` and
   `pub remote_units: BTreeMap<String, RemoteUnit>` (Interfaces). Define
   `RemoteUnit` and, under `#[cfg(feature = "fetch")]`, `RemoteAccess`.
3. `Loader` gains `imports: ImportUnion` and
   `remote_units: BTreeMap<String, RemoteUnit>`.
4. `TreeScope` gains `origin: &'a PackageOrigin`; `load_root`'s and
   `load_member`'s scopes pass `origin: &PackageOrigin::WorkspaceMember`;
   `load_package_tree` passes `origin.clone()` to `Package::new` instead of
   `PackageOrigin::WorkspaceMember`.
5. `load_root` records the union: in the `Package` branch
   `self.record_imports(&imports)`; in the `Workspace` branch
   `self.record_imports(&imports)` before the member loop. `load_member` records
   the member's `imports` after the MANI-014 check (so a member that is not
   loaded still contributes its pins — that is what `materialize_and_lock` did
   through `workspace.packages`; keep it before `return Ok(())` of the MANI-014
   branch by moving the call above the `if let Some(first_dir)`).

   ```rust
   /// Adds `imports` to the union: each URL gains the key that names it.
   fn record_imports(&mut self, imports: &BTreeMap<String, String>) {
       for (key, url) in imports {
           self.imports.entry(url.clone()).or_default().insert(key.clone());
       }
   }
   ```

6. Split `load_workspace_with` into a public wrapper and
   `load_inner(db, entry, overlays, access: Option<RemoteAccess<'_>>)`. The
   wrapper passes `None`. After the `is_file`/`is_dir` match, when `access` is
   `Some` and a root was loaded (not single-file mode), call
   `loader.load_remotes(db, &root, access)?`. Keep the root in a local: the
   match arms that call `load_root` already have it, so restructure as
   `let root = ...; loader.load_root(db, &root)?; if let Some(access) = access { loader.load_remotes(db, &root, access)?; }`.
   Under `#[cfg(not(feature = "fetch"))]` the `access` parameter type is a
   zero-variant placeholder; simplest: define
   `#[cfg(not(feature = "fetch"))] pub struct RemoteAccess<'a>(std::marker::PhantomData<&'a ()>);`
   with no constructor, so the signature is one and the branch is dead.
7. The public entry point:

   ```rust
   /// [`load_workspace_with`], then every remote unit of the consumer's
   /// `[imports]` (ADR-0002 §5, §7): the union of the root's and every
   /// member's pins is materialized through
   /// [`materialize_imports`](crate::fetch::materialize_imports) — a cache hit
   /// against `ridl.lock`, else a fetch under [`Frozen::No`] — and each unit's
   /// tree is loaded as packages of origin [`PackageOrigin::Remote`]. Under
   /// [`Frozen::No`] and with no materialization error, `ridl.lock` at the
   /// manifest root is rewritten with both hashes; under [`Frozen::Yes`]
   /// nothing is fetched and the lockfile is never written. The language
   /// server and the MCP server pass [`Frozen::Yes`]. A remote unit's
   /// Warnings and Infos are dropped here; its Errors are kept with the unit
   /// named in the message. Single-file mode has no manifest and loads no
   /// remote unit.
   #[cfg(feature = "fetch")]
   pub fn load_workspace_with_remotes(
       db: &mut RidlDatabase,
       entry: &Path,
       overlays: &[Overlay],
       access: RemoteAccess<'_>,
   ) -> Result<LoadedWorkspace, LoadError> {
       load_inner(db, entry, overlays, Some(access))
   }
   ```

8. `Loader::load_remotes`:

   ```rust
   #[cfg(feature = "fetch")]
   fn load_remotes(
       &mut self,
       db: &mut RidlDatabase,
       root: &Path,
       access: RemoteAccess<'_>,
   ) -> io::Result<()> {
       if self.imports.is_empty() {
           return Ok(());
       }
       let lock_path = root.join("ridl.lock");
       let (lock, diagnostics) = read_lockfile(&lock_path);
       self.diagnostics.extend(diagnostics);
       let (units, regenerated, diagnostics) =
           materialize_imports(&self.imports, lock.as_ref(), access.cache, access.frozen);
       let had_error = diagnostics.iter().any(|d| d.severity == Severity::Error);
       self.diagnostics.extend(diagnostics);
       // A frozen run never regenerates the lockfile (ADR-0002 §7); a failed
       // run must not overwrite the pins it could not verify.
       if access.frozen == Frozen::No
           && !had_error
           && let Err(err) = write_lockfile(&lock_path, &regenerated)
       {
           self.diagnostics.push(detached_warning(format!(
               "cannot write `{}`: {err}",
               lock_path.display()
           )));
       }
       let union = self.imports.clone();
       for unit in units {
           let keys = &union[&unit.url];
           self.load_remote_unit(db, &unit, keys, &union)?;
       }
       Ok(())
   }
   ```

   `detached_warning` is the `DiagCode::NONE` Warning builder; move it from
   `crates/ridlc/src/lib.rs:2567-2579` into this module (Task 5 removes it
   there).

9. `Loader::load_remote_unit`:

   ```rust
   /// Loads one materialized remote unit (spec section 4.2): its top-level
   /// `ridl.toml` must be a `[package]` manifest whose `name` is one of the
   /// `keys` that name its URL (MANI-105), every key of its `[imports]` must
   /// be a key of the consumer's union (MANI-106), and its tree then loads
   /// as `load_package_tree` loads a member's, with the consumer's URL for
   /// each key it imports (V-1), its own `[defaults]`, and its own
   /// `interfaces.lock`. The ordinary rules apply unchanged: MANI-014 when
   /// a consumer unit or an earlier remote unit already claims a package.
   /// Every Warning and Info the tree draws is dropped; every Error keeps
   /// its span and is prefixed with the unit name.
   #[cfg(feature = "fetch")]
   fn load_remote_unit(
       &mut self,
       db: &mut RidlDatabase,
       unit: &MaterializedUnit,
       keys: &BTreeSet<String>,
       union: &ImportUnion,
   ) -> io::Result<()> {
       let start = self.diagnostics.len();
       let manifest_path = unit.dir.join("ridl.toml");
       if !manifest_path.is_file() {
           self.diagnostics.push(detached_error(
               DiagCode::MANI_105,
               format!(
                   "the remote unit at `{}` has no `ridl.toml` at its top level (`{}`)",
                   unit.url,
                   unit.dir.display()
               ),
           ));
           return Ok(());
       }
       let text = fs::read_to_string(&manifest_path).map_err(|e| cannot_read(&manifest_path, e))?;
       let file_id = self.sources.file_id(&path_string(&manifest_path), &text);
       let (manifest, diags) = parse_manifest(file_id, &text);
       self.diagnostics.extend(diags);
       let Some(manifest) = manifest else {
           self.keep_remote_errors(start, keys.iter().next().map_or("", String::as_str));
           return Ok(());
       };
       let (name, name_span) = match manifest.kind {
           ManifestKind::Workspace { .. } => {
               self.diagnostics.push(error(
                   DiagCode::MANI_105,
                   file_id,
                   workspace_section_range(&text),
                   format!(
                       "the remote unit at `{}` is a `[workspace]` manifest; a remote unit is one `[package]` manifest and its tree",
                       unit.url
                   ),
               ));
               return Ok(());
           }
           ManifestKind::Package { name, name_span, .. } => (name, name_span),
       };
       let name_range = byte_range(name_span.start, name_span.end);
       // Every key that names the URL but is not the unit's name binds
       // nothing; the unit still loads under its own name.
       for key in keys {
           if *key != name {
               self.diagnostics.push(error(
                   DiagCode::MANI_105,
                   file_id,
                   name_range,
                   format!(
                       "the `[imports]` key `{key}` names `{}`, whose `[package] name` is `{name}`; the key must equal the name",
                       unit.url
                   ),
               ));
           }
       }
       if !keys.contains(&name) {
           return Ok(());
       }
       let covered: BTreeSet<&String> = union.values().flatten().collect();
       for (key, remote_url) in &manifest.imports {
           if !covered.contains(key) {
               self.diagnostics.push(error(
                   DiagCode::MANI_106,
                   file_id,
                   member_entry_range(&text, key),
                   format!(
                       "remote unit `{name}` imports `{key}` (`{remote_url}`), which the consumer's `[imports]` does not name; add `{key}` to the consumer's `[imports]`"
                   ),
               ));
           }
       }
       // V-1: the consumer's URL for each key the unit imports. A key the
       // consumer does not name was MANI-106 above and resolves nowhere.
       let imports: BTreeMap<String, String> = manifest
           .imports
           .keys()
           .filter_map(|key| {
               union
                   .iter()
                   .find(|(_, keys)| keys.contains(key))
                   .map(|(url, _)| (key.clone(), url.clone()))
           })
           .collect();
       if self.units.contains_key(&name) || self.remote_units.contains_key(&name) {
           let first_dir = self
               .units
               .get(&name)
               .cloned()
               .unwrap_or_else(|| self.remote_units[&name].dir.clone());
           self.diagnostics.push(error(
               DiagCode::MANI_014,
               file_id,
               name_range,
               format!(
                   "source package `{name}` is already declared by the unit in `{}`; the remote unit at `{}` declares it too. A source package belongs to one unit",
                   first_dir.display(),
                   unit.url
               ),
           ));
           return Ok(());
       }
       self.unit_name_spans.insert(name.clone(), (file_id, name_range));
       let lock = self.read_lock(&unit.dir)?;
       let origin = PackageOrigin::Remote { url: unit.url.clone() };
       let scope = TreeScope {
           unit_dir: &unit.dir,
           unit: &name,
           imports: &imports,
           defaults: &manifest.defaults,
           lock: &lock,
           origin: &origin,
       };
       self.load_package_tree(db, &unit.dir, &name, &scope)?;
       self.keep_remote_errors(start, &name);
       self.remote_units.insert(
           name,
           RemoteUnit {
               url: unit.url.clone(),
               dir: unit.dir.clone(),
               tree: unit.tree.clone(),
           },
       );
       Ok(())
   }

   /// Keeps only the Errors among the diagnostics pushed since `start` —
   /// a remote unit's Warnings and Infos are not the consumer's to fix
   /// (V-3) — and prefixes each with the unit name, so a reader sees which
   /// unit the file path belongs to.
   fn keep_remote_errors(&mut self, start: usize, unit: &str) {
       let tail: Vec<Diagnostic> = self.diagnostics.drain(start..).collect();
       self.diagnostics.extend(
           tail.into_iter()
               .filter(|d| d.severity == Severity::Error)
               .map(|mut d| {
                   d.message = format!("remote unit `{unit}`: {}", d.message);
                   d
               }),
       );
   }
   ```

   `member_entry_range` already finds a quoted key in a manifest text; it serves
   the `[imports]` key too. Add `detached_error(code, message)` beside
   `detached_warning`. No lint scope is inserted for a remote directory. Every
   return after `parse_manifest` goes through `keep_remote_errors` (write the
   body as an inner function and call `keep_remote_errors(start, label)` on its
   result, with `label` the unit's name once known and the first key before
   that), so a MANI-005 of a remote manifest never reaches the consumer
   whichever branch returns.

10. `load_inner` fills `imports: loader.imports` and
    `remote_units: loader.remote_units` into `LoadedWorkspace`.

`crates/ridl-core/src/lib.rs`: re-export `ImportUnion`, `RemoteUnit`, and under
`fetch`, `RemoteAccess` and `load_workspace_with_remotes`.

`crates/ridlc/src/lib.rs:133-146` and `crates/ridl-lsp/src/server.rs:332-341`:
add the two new fields as described in Files.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p ridl-core --lib workspace::` then
`cargo test --workspace --locked` Expected: the eleven `remote::` tests pass;
the rest of the workspace is green (nothing else observes the new fields yet).

- [ ] **Step 5: Commit**

```bash
git add crates/ridl-core crates/ridlc/src/lib.rs crates/ridl-lsp/src/server.rs
git commit -m "feat(ridl-core)!: load a remote unit from the cache into the workspace

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

### Task 4: The resolver binds a remote import

**Files:**

- Modify: `crates/ridl-sem/src/resolve.rs:169-190` (`ImportRecord`), `:233-244`
  (the `resolve_source` match in `collect_imports`), `:264-267` and `:299-326`
  (`apply_imports`), `:474-500` (`PackageSource`, `resolve_source`)
- Test: `crates/ridl-sem/src/resolve.rs` test module (new tests beside the
  existing `Package::new` fixtures at `:1130-1170`)

**Interfaces:**

- Consumes: `PackageOrigin::Remote { url }` (Task 3), `package_of`.
- Produces: no public signature changes. The uncoded Info "remote import … is
  not yet available" is removed. A remote import whose package the loaded unit
  does not hold is the uncoded Error "unresolved import: `{path}` is not a
  package of the remote unit at `{url}`".

- [ ] **Step 1: Write the failing tests**

Append to the test module of `crates/ridl-sem/src/resolve.rs`, using the
module's existing helpers (`file`, a `Package::new` fixture, `std_package`):

```rust
    fn remote_package(db: &RidlDatabase, name: &str, url: &str, text: &str) -> Package {
        Package::new(
            db,
            name.to_string(),
            name.split('.').take(2).collect::<Vec<_>>().join("."),
            vec![file(db, &format!("cache/{}.typl", name.replace('.', "/")), text)],
            PackageOrigin::Remote { url: url.to_string() },
            BTreeMap::new(),
            TimingDefaults::default(),
            None,
        )
    }

    const URL: &str = "https://registry.example.com/veh/common@v1";

    /// `import veh.common.Speed` binds to the declaration of the remote
    /// package loaded for the key's URL, through ADR-0002 §5 step 2.
    #[test]
    fn a_remote_import_binds_to_the_loaded_unit() {
        let mut db = RidlDatabase::default();
        let std = std_package(&mut db);
        let common = remote_package(&db, "veh.common", URL, "package veh.common\ntype Speed: km/h\ninternal type Raw: m\n");
        let app = Package::new(
            &db,
            "app".into(),
            "app".into(),
            vec![file(&db, "app/app.ridl", "package app\nimport veh.common.Speed\nimport veh.common.Raw\ntype Fast: Speed\n")],
            PackageOrigin::WorkspaceMember,
            BTreeMap::from([("veh.common".to_string(), URL.to_string())]),
            TimingDefaults::default(),
            None,
        );
        let ws = Workspace::new(&db, vec![app, common], BTreeMap::new());
        let resolution = resolve_package(&db, ws, app, std);
        let speed = &resolution.symbols["Speed"];
        assert_eq!(speed.package, "veh.common");
        assert_eq!(speed.kind, SymbolKind::Type);
        assert!(!resolution.symbols.contains_key("Raw"), "internal is not visible");
        assert!(
            resolution.diagnostics.iter().all(|d| d.severity != Severity::Info),
            "the uncoded Info is gone: {:?}",
            resolution.diagnostics
        );
    }

    /// A key's URL in the package's own `[imports]` wins over the
    /// workspace's URL for the same key (ADR-0002 §5 steps 2 and 3): the
    /// import binds to the unit loaded for the package's URL.
    #[test]
    fn the_packages_own_pin_shadows_the_workspace_pin() {
        let mut db = RidlDatabase::default();
        let std = std_package(&mut db);
        let mine = remote_package(&db, "veh.common", "https://r.example.com/a@v2", "package veh.common\ntype Speed: km/h\n");
        let app = Package::new(
            &db,
            "app".into(),
            "app".into(),
            vec![file(&db, "app/app.ridl", "package app\nimport veh.common.Speed\ntype Fast: Speed\n")],
            PackageOrigin::WorkspaceMember,
            BTreeMap::from([("veh.common".to_string(), "https://r.example.com/a@v2".to_string())]),
            TimingDefaults::default(),
            None,
        );
        let ws = Workspace::new(
            &db,
            vec![app, mine],
            BTreeMap::from([("veh.common".to_string(), URL.to_string())]),
        );
        let resolution = resolve_package(&db, ws, app, std);
        assert_eq!(resolution.symbols["Speed"].package, "veh.common");
    }

    /// A remote package loaded for a sibling's pin is not visible to a
    /// member that pins nothing (ADR-0002 §5 step 4 stays an error).
    #[test]
    fn a_siblings_pin_is_not_visible() {
        let mut db = RidlDatabase::default();
        let std = std_package(&mut db);
        let common = remote_package(&db, "veh.common", URL, "package veh.common\ntype Speed: km/h\n");
        let app = Package::new(
            &db,
            "app".into(),
            "app".into(),
            vec![file(&db, "app/app.ridl", "package app\nimport veh.common.Speed\n")],
            PackageOrigin::WorkspaceMember,
            BTreeMap::new(),
            TimingDefaults::default(),
            None,
        );
        let ws = Workspace::new(&db, vec![app, common], BTreeMap::new());
        let resolution = resolve_package(&db, ws, app, std);
        assert!(!resolution.symbols.contains_key("Speed"));
        assert!(resolution.diagnostics.iter().any(|d| d.message.starts_with("unresolved import")));
    }

    /// A path under a pinned key that names no package of the unit is an
    /// error that names the unit's URL.
    #[test]
    fn a_missing_package_of_a_remote_unit_is_an_error_naming_the_url() {
        let mut db = RidlDatabase::default();
        let std = std_package(&mut db);
        let common = remote_package(&db, "veh.common", URL, "package veh.common\n");
        let app = Package::new(
            &db,
            "app".into(),
            "app".into(),
            vec![file(&db, "app/app.ridl", "package app\nimport veh.common.nope.Speed\n")],
            PackageOrigin::WorkspaceMember,
            BTreeMap::from([("veh.common".to_string(), URL.to_string())]),
            TimingDefaults::default(),
            None,
        );
        let ws = Workspace::new(&db, vec![app, common], BTreeMap::new());
        let resolution = resolve_package(&db, ws, app, std);
        let error = resolution.diagnostics.iter().find(|d| d.severity == Severity::Error).unwrap();
        assert!(error.message.contains("veh.common.nope") && error.message.contains(URL), "{}", error.message);
    }

    /// A remote unit may import one of the consumer's members (step 1), and
    /// a cycle across the boundary is TYPL-004 on the remote unit's import
    /// (Review Focus 3).
    #[test]
    fn a_cycle_across_the_remote_boundary_is_typl_004() {
        let mut db = RidlDatabase::default();
        let std = std_package(&mut db);
        let common = remote_package(&db, "veh.common", URL, "package veh.common\nimport app.Id\ntype Speed: km/h\ntype Tag: Id\n");
        let app = Package::new(
            &db,
            "app".into(),
            "app".into(),
            vec![file(&db, "app/app.ridl", "package app\nimport veh.common.Speed\ntype Id: integer[0..9]\ntype Fast: Speed\n")],
            PackageOrigin::WorkspaceMember,
            BTreeMap::from([("veh.common".to_string(), URL.to_string())]),
            TimingDefaults::default(),
            None,
        );
        let ws = Workspace::new(&db, vec![app, common], BTreeMap::new());
        let remote = resolve_package(&db, ws, common, std);
        assert_eq!(remote.symbols["Id"].package, "app", "the member binds in the remote unit");
        assert!(remote.diagnostics.iter().any(|d| d.code == DiagCode::TYPL_004), "{:?}", remote.diagnostics);
        let consumer = resolve_package(&db, ws, app, std);
        assert!(consumer.diagnostics.iter().any(|d| d.code == DiagCode::TYPL_004));
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run:
`cargo test -p ridl-sem --lib resolve::tests::a_remote_import_binds_to_the_loaded_unit resolve::tests::the_packages_own_pin resolve::tests::a_siblings_pin resolve::tests::a_missing_package_of_a_remote resolve::tests::a_cycle_across`
Expected: `a_remote_import_binds_to_the_loaded_unit` fails on `symbols["Speed"]`
(no entry) and the Info assertion; `a_missing_package…` finds no error;
`a_cycle_across…` finds no `Id` symbol; the other two pass already.

- [ ] **Step 3: Implement**

`PackageSource` and `resolve_source`:

```rust
/// Where a package reference resolves, following ADR-0002 §5 in order.
enum PackageSource {
    /// A workspace member (step 1).
    Member(Package),
    /// A key of the package's own or the workspace's `[imports]` (steps 2
    /// and 3): the URL, and the package of the remote unit loaded for that
    /// URL whose name is the path, when the unit holds one.
    Remote { url: String, package: Option<Package> },
    Unknown,
}

fn resolve_source(
    db: &dyn salsa::Database,
    ws: Workspace,
    pkg: Package,
    package_path: &str,
) -> PackageSource {
    // Step 1 sees members only: a remote package is in `ws.packages` so
    // that the checker and the cycle walk find it by name, but it is
    // reached here only through the key that pins it, so a sibling's pin
    // never leaks (ADR-0002 §5).
    if let Some(member) = package_of(db, ws, package_path.to_string())
        && !matches!(member.origin(db), PackageOrigin::Remote { .. })
    {
        return PackageSource::Member(member);
    }
    let url = longest_import_prefix(pkg.imports(db), package_path)
        .or_else(|| longest_import_prefix(ws.imports(db), package_path));
    match url {
        Some(url) => {
            let package = ws
                .packages(db)
                .iter()
                .copied()
                .find(|candidate| {
                    candidate.name(db) == package_path
                        && *candidate.origin(db) == PackageOrigin::Remote { url: url.clone() }
                });
            PackageSource::Remote { url, package }
        }
        None => PackageSource::Unknown,
    }
}
```

Add `use ridl_core::package::PackageOrigin;`. In `ImportRecord` replace
`remote_url: Option<String>` with:

```rust
/// The URL of the remote unit the path's key pins, when that unit holds
/// no package named by the path.
missing_remote: Option<String>,
```

and rename `member` to `target` with the doc "The loaded package the path
resolves to, member or remote (for cycle detection)". In `collect_imports`:

```rust
let (bound, target, missing_remote, unknown) =
    match resolve_source(db, ws, pkg, &package_path) {
        PackageSource::Member(member) => {
            (exposed(db, member, &symbol_name), Some(member), None, false)
        }
        PackageSource::Remote { package: Some(package), .. } => {
            (exposed(db, package, &symbol_name), Some(package), None, false)
        }
        PackageSource::Remote { url, package: None } => (None, None, Some(url), false),
        PackageSource::Unknown => (None, None, None, true),
    };
```

with

```rust
/// The non-`internal` declaration `name` of `package`, if any.
fn exposed(db: &dyn salsa::Database, package: Package, name: &str) -> Option<Symbol> {
    declared_symbols(db, package)
        .get(name)
        .filter(|symbol| !symbol.internal)
        .cloned()
}
```

In `apply_imports`, replace the
`if let Some(url) = &record.remote_url { … Info … }` block with:

```rust
if let Some(url) = &record.missing_remote {
    diagnostics.push(diagnostic(
        DiagCode::NONE,
        Severity::Error,
        record.file_id,
        record.range,
        format!(
            "unresolved import: `{}` is not a package of the remote unit at `{url}`",
            record.package_path
        ),
    ));
}
```

Update the `apply_imports` doc comment ("unresolved-remote" becomes "a path the
pinned remote unit does not hold") and the module doc's description of the
order. `detect_cycles` reads `record.target`; the "Importing the package's own
name" guard stays. `member_edges` is unchanged: `package_of` finds remote
packages by name, which is what the reachability walk needs.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p ridl-sem` then `cargo test -p ridlc --test cli` and
`cargo test -p ridl-cli` Expected: all pass. The `catalog_chain` and `cli` tests
that pin a remote import under `--frozen` still see MANI-103, because no lock
exists there.

- [ ] **Step 5: Commit**

```bash
git add crates/ridl-sem/src/resolve.rs
git commit -m "feat(ridl-sem)!: bind a remote import to the loaded unit

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

### Task 5: The `ridlc` drivers read remote units before the check

**Files:**

- Modify: `crates/ridlc/src/lib.rs:60-70` (imports), `:298-355`
  (`WorkspaceOutput`), `:382-429` (`compile_workspace`,
  `compile_workspace_with`), `:431-468` (`retain_in_report_scope`,
  `in_report_scope`), `:754-776` (`run_check`), `:942-1018` and `:1250-1323`
  (`run_build_inner`), `:1805-1964` (`Compiled`, `load_and_check`,
  `check_loaded`), `:1966-2021` (`materialize_and_lock`, deleted), `:2246-2277`
  (`write_catalogs`), `:2565-2579` (`detached_warning`, deleted)
- Modify: `crates/ridl-mcp/src/snapshot.rs:60-64` and
  `crates/ridl/src/lock.rs:97-101` (the two external `retain_in_report_scope`
  callers)
- Modify: `crates/ridl/Cargo.toml:46-66` (`[dev-dependencies]`: add
  `tar.workspace = true`)
- Create: `crates/ridl/tests/remote_support/mod.rs`,
  `crates/ridl/tests/remote_units.rs`

**Interfaces:**

- Consumes: `load_workspace_with_remotes`, `RemoteAccess`, `RemoteUnit`,
  `Cache::user_default()`, `Frozen` (Tasks 2–3).
- Produces:

  ```rust
  pub struct WorkspaceOutput {
      /* existing fields */
      /// The remote units the load took (ADR-0002 §5), keyed by unit name;
      /// `checked` holds their packages beside the consumer's. A consumer
      /// that publishes, locks, tests or diffs the workspace's own packages
      /// tells them apart with `ridl_ir::v2::unit_of`.
      pub remote_units: BTreeMap<String, RemoteUnit>,
  }
  /// `scope` as before; `remote_dirs` are the directories of the loaded
  /// remote units, whose diagnostics are kept whatever the scope.
  pub fn retain_in_report_scope(
      diagnostics: &mut Vec<Diagnostic>,
      sources: &SourceMap,
      scope: Option<&Path>,
      remote_dirs: &[PathBuf],
  );
  /// Whether `package` belongs to a loaded remote unit.
  pub fn is_remote(package: &ridl_ir::v2::Package, remote_units: &BTreeMap<String, RemoteUnit>) -> bool;
  ```

  `compile_workspace` and `compile_workspace_with` keep their signatures and
  load remote units with `Frozen::Yes` (no fetch, no lock write).

- [ ] **Step 1: Write the failing tests**

Create `crates/ridl/tests/remote_support/mod.rs` — the HTTP stub of
`crates/ridl-core/src/fetch.rs:295-394` (copy `Stub`, `serve_one`, verbatim,
`pub`), plus:

```rust
//! Support for the tests that exercise a remote `[imports]` unit against
//! the built `ridl` binary: a local HTTP stub that serves one tar, a tar
//! builder, and a runner that points `~/.ridl/cache` at a temp directory.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

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
        builder.append_data(&mut header, name, contents.as_bytes()).unwrap();
    }
    builder.into_inner().unwrap()
}

/// The `veh.common` unit every test imports: one type at the unit root.
pub const COMMON_UNIT: &[(&str, &str)] = &[
    ("ridl.toml", "[package]\nname = \"veh.common\"\nversion = \"1.0.0\"\n"),
    ("common.typl", "package veh.common\n\ntype Speed : km/h [0.0..250.0 step 0.5]\n"),
];

/// The same unit with an interface whose signal has no timing: RIDL-100
/// (`missing-timing`), a Warning the consumer must not see.
pub const WARNING_UNIT: &[(&str, &str)] = &[
    ("ridl.toml", "[package]\nname = \"veh.common\"\nversion = \"1.0.0\"\n"),
    ("common.ridl", "package veh.common\n\ntype Speed : km/h [0.0..250.0 step 0.5]\n\ninterface Probe {\n  signal s: Speed\n}\n"),
];

/// The same unit with a range whose minimum exceeds its maximum: TYPL-104,
/// an Error the consumer must see.
pub const BROKEN_UNIT: &[(&str, &str)] = &[
    ("ridl.toml", "[package]\nname = \"veh.common\"\nversion = \"1.0.0\"\n"),
    ("common.typl", "package veh.common\n\ntype Speed : km/h [250.0..0.0 step 0.5]\n"),
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
        "package app\n\nimport veh.common.Speed\n\ninterface Dash {\n  signal speed: Speed @10ms\n}\n",
    );
    dir.path().join("app")
}
```

Create `crates/ridl/tests/remote_units.rs`:

```rust
//! The read path for a remote `[imports]` unit, against the built `ridl`
//! binary: a unit is fetched once, read from the cache after that, resolved,
//! checked as a read-only dependency and generated, and never cataloged or
//! published as the consumer's own.

mod remote_support;

use remote_support::*;

/// `ridl check` fetches the unit once, pins both hashes, and the import
/// resolves: a clean run prints nothing.
#[test]
fn check_fetches_once_and_resolves_the_import() {
    let dir = TempDir::new("check");
    let stub = Stub::serving(unit_tar(COMMON_UNIT));
    let root = consumer(&dir, stub.url());
    let (code, _, stderr) = ridl(&root, dir.path(), &["check"]);
    assert_eq!(code, 0, "{stderr}");
    // `missing-docs` may print an Info for the consumer's own items; the
    // import itself draws nothing.
    assert!(!stderr.contains("unresolved") && !stderr.contains("Speed"), "{stderr}");
    assert_eq!(stub.hits(), 1);
    let lock = std::fs::read_to_string(root.join("ridl.lock")).unwrap();
    assert!(lock.contains("sha256 = ") && lock.contains("tree = "), "{lock}");

    stub.reset_hits();
    let (code, _, stderr) = ridl(&root, dir.path(), &["check", "--frozen"]);
    assert_eq!(code, 0, "{stderr}");
    assert_eq!(stub.hits(), 0, "a frozen check reads the cache");
}

/// A type the unit does not declare is an error on the consumer's import.
#[test]
fn a_missing_declaration_is_the_consumers_error() {
    let dir = TempDir::new("missing-decl");
    let stub = Stub::serving(unit_tar(COMMON_UNIT));
    let root = consumer(&dir, stub.url());
    dir.write("app/app.ridl", "package app\nimport veh.common.Nope\n");
    let (code, _, stderr) = ridl(&root, dir.path(), &["check"]);
    assert_eq!(code, 1, "{stderr}");
    assert!(stderr.contains("app.ridl"), "{stderr}");
}

/// A Warning inside the remote unit is dropped; an Error is reported with
/// the unit name and the cache path, and fails the check (spec 4.4).
#[test]
fn a_remote_error_fails_and_a_remote_warning_is_silent() {
    let dir = TempDir::new("severity");
    let stub = Stub::serving(unit_tar(WARNING_UNIT));
    let root = consumer(&dir, stub.url());
    let (code, _, stderr) = ridl(&root, dir.path(), &["check"]);
    assert_eq!(code, 0, "{stderr}");
    assert!(!stderr.contains("RIDL-100"), "the untimed signal of the unit is not shown: {stderr}");

    let dir = TempDir::new("severity-error");
    let stub = Stub::serving(unit_tar(BROKEN_UNIT));
    let root = consumer(&dir, stub.url());
    let (code, _, stderr) = ridl(&root, dir.path(), &["check"]);
    assert_eq!(code, 1, "{stderr}");
    assert!(stderr.contains("TYPL-104"), "{stderr}");
    assert!(stderr.contains("remote unit `veh.common`"), "{stderr}");
    assert!(stderr.contains(".ridl/cache"), "the path is in the cache: {stderr}");
}

/// The consumer's `[lints]` do not apply to the remote unit: `deny` on a
/// lint the unit draws changes nothing.
#[test]
fn the_consumers_lints_do_not_reach_the_remote_unit() {
    let dir = TempDir::new("lints");
    let stub = Stub::serving(unit_tar(WARNING_UNIT));
    let root = consumer(&dir, stub.url());
    dir.write(
        "app/ridl.toml",
        &format!(
            "[package]\nname = \"app\"\nversion = \"1.0.0\"\n\n[imports]\n\"veh.common\" = \"{}\"\n\n[lints]\nmissing-timing = \"deny\"\n",
            stub.url()
        ),
    );
    let (code, _, stderr) = ridl(&root, dir.path(), &["check"]);
    assert_eq!(code, 0, "{stderr}");
}

/// `ridl build --emit rust,ir-json,catalog` writes the remote package's Rust
/// module and names it in `lib.rs`, but writes no IR snapshot and no catalog
/// for it (spec 4.4).
#[test]
fn build_generates_the_remote_package_but_writes_no_snapshot_or_catalog_for_it() {
    let dir = TempDir::new("build");
    let out = TempDir::new("build-out");
    let stub = Stub::serving(unit_tar(&[
        COMMON_UNIT[0],
        ("common.ridl", "package veh.common\n\ntype Speed : km/h [0.0..250.0 step 0.5]\n\ninterface Probe {\n  signal s: Speed @10ms\n}\n"),
    ]));
    let root = consumer(&dir, stub.url());
    let (code, _, stderr) = ridl(
        &root,
        dir.path(),
        &["build", "--out-dir", out.path().to_str().unwrap(), "--emit", "rust,ir-json,catalog"],
    );
    assert_eq!(code, 0, "{stderr}");
    assert!(out.path().join("veh.common.rs").is_file());
    assert!(out.path().join("app.rs").is_file());
    assert!(std::fs::read_to_string(out.path().join("lib.rs")).unwrap().contains("common"));
    assert!(out.path().join("app.ir.json").is_file());
    assert!(!out.path().join("veh.common.ir.json").exists(), "no snapshot for a remote package");
    assert!(out.path().join("app.catalog.binfb").is_file());
    assert!(!out.path().join("veh.common.catalog.binfb").exists(), "no catalog for a remote unit");
}

/// A lockfile written before `tree` existed fails a frozen check with
/// MANI-108 and is repaired by the next non-frozen check without a fetch.
#[test]
fn an_old_lock_is_mani_108_frozen_and_repaired_otherwise() {
    let dir = TempDir::new("old-lock");
    let stub = Stub::serving(unit_tar(COMMON_UNIT));
    let root = consumer(&dir, stub.url());
    let (code, _, stderr) = ridl(&root, dir.path(), &["check"]);
    assert_eq!(code, 0, "{stderr}");
    let lock = std::fs::read_to_string(root.join("ridl.lock")).unwrap();
    let old: String = lock.lines().filter(|l| !l.starts_with("tree = ")).map(|l| format!("{l}\n")).collect();
    std::fs::write(root.join("ridl.lock"), &old).unwrap();

    let (code, _, stderr) = ridl(&root, dir.path(), &["check", "--frozen"]);
    assert_eq!(code, 1, "{stderr}");
    assert!(stderr.contains("MANI-108"), "{stderr}");

    stub.reset_hits();
    let (code, _, stderr) = ridl(&root, dir.path(), &["check"]);
    assert_eq!(code, 0, "{stderr}");
    assert_eq!(stub.hits(), 0, "the cache hit needs no fetch");
    assert!(std::fs::read_to_string(root.join("ridl.lock")).unwrap().contains("tree = "));
}

/// A `ridl check` run from inside a workspace member still reports a remote
/// unit's error, which no member scope hides.
#[test]
fn a_member_entry_still_sees_a_remote_error() {
    let dir = TempDir::new("member-scope");
    let stub = Stub::serving(unit_tar(BROKEN_UNIT));
    dir.write(
        "ws/ridl.toml",
        &format!("[workspace]\nmembers = [\"app\"]\n\n[imports]\n\"veh.common\" = \"{}\"\n", stub.url()),
    );
    dir.write("ws/app/ridl.toml", "[package]\nname = \"app\"\nversion = \"1.0.0\"\n");
    dir.write("ws/app/app.ridl", "package app\nimport veh.common.Speed\ntype Fast: Speed\n");
    let (code, _, stderr) = ridl(&dir.path().join("ws/app"), dir.path(), &["check"]);
    assert_eq!(code, 1, "{stderr}");
    assert!(stderr.contains("TYPL-104") && stderr.contains("remote unit `veh.common`"), "{stderr}");
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p ridl-cli --test remote_units` Expected:
`check_fetches_once_and_resolves_the_import` fails — stderr holds the uncoded
"unresolved import" error from Task 4 or an unresolved `Speed`, because
`load_and_check` still loads no remote unit. The others fail on the same missing
resolution.

- [ ] **Step 3: Implement**

1. `crates/ridlc/src/lib.rs` imports: replace `materialize_imports` and
   `read_lockfile`/`write_lockfile` in the `use ridl_core::{…}` list with
   `RemoteAccess, RemoteUnit, load_workspace_with_remotes`.
2. `Compiled` gains `remote_units: BTreeMap<String, RemoteUnit>`;
   `load_and_check` takes `access: RemoteAccess<'_>` and calls
   `load_workspace_with_remotes(db, entry, overlays, access)`. Every caller
   builds the access as `RemoteAccess { frozen, cache: &Cache::user_default() }`
   (Task 9 turns that into `RemoteAccess::new(frozen, &cache)`). `check_loaded`
   destructures `remote_units` and `imports: _` from `LoadedWorkspace` and,
   after `check_design_lints`, calls:

   ```rust
   /// A remote unit is a read-only dependency (ADR-0002 §5): every Warning
   /// and Info whose primary span is in a file of a remote unit is dropped,
   /// before any lint level is applied, and every Error there is prefixed
   /// with the unit name when the loader has not done so already.
   fn drop_remote_non_errors(
       diagnostics: &mut Vec<Diagnostic>,
       sources: &SourceMap,
       remote_units: &BTreeMap<String, RemoteUnit>,
   ) {
       diagnostics.retain_mut(|diagnostic| {
           let Some(path) = sources.path(diagnostic.primary.file) else {
               return true;
           };
           let path = Path::new(path);
           let Some((unit, _)) = remote_units.iter().find(|(_, unit)| path.starts_with(&unit.dir)) else {
               return true;
           };
           if diagnostic.severity != Severity::Error {
               return false;
           }
           let prefix = format!("remote unit `{unit}`: ");
           if !diagnostic.message.starts_with(&prefix) {
               diagnostic.message.insert_str(0, &prefix);
           }
           true
       });
   }
   ```

3. `compile_workspace_with` passes `Frozen::Yes` in its access; its doc says the
   load reads `vendor/` (from PR 2) and the cache and never fetches, so the
   language server and the MCP server never touch the network. `WorkspaceOutput`
   gains `remote_units` (filled from `Compiled`).
4. `run_check`:
   `load_and_check(&mut db, entry, &[], RemoteAccess { frozen, cache: &Cache::user_default() })`;
   delete the `materialize_and_lock` call;
   `retain_in_report_scope(&mut diagnostics, &sources, report_scope.as_deref(), &remote_dirs(&remote_units))`
   with

   ```rust
   /// The directories of the loaded remote units.
   fn remote_dirs(remote_units: &BTreeMap<String, RemoteUnit>) -> Vec<PathBuf> {
       remote_units.values().map(|unit| unit.dir.clone()).collect()
   }
   ```

5. `retain_in_report_scope` and `in_report_scope` take `remote_dirs: &[PathBuf]`
   and keep a diagnostic whose path starts with one of them. Update the two
   external callers (`crates/ridl-mcp/src/snapshot.rs`,
   `crates/ridl/src/lock.rs`) to pass `&remote_dirs(&output.remote_units)` —
   expose `remote_dirs` as `pub fn remote_dirs`.
6. `run_build_inner`: same load; delete the `materialize_and_lock` call and its
   comment (keep the comment's statement that a MANI-1xx error joins the compile
   diagnostics, now because the loader raised it). The `compute` closure
   receives the consumer's packages only:

   ```rust
   let packages: Vec<&ridl_ir::v2::Package> = checked
       .iter()
       .map(|package| &package.ir)
       .filter(|ir| !is_remote(ir, &remote_units))
       .collect();
   ```

   and `names_imports` stays as it is. In the emit section, `packages` (used for
   `references_std`, `others`, `lower_system`, `write_catalogs`) is every
   checked package; add
   `let own: Vec<&ridl_ir::v2::Package> = packages.iter().copied().filter(|ir| !is_remote(ir, &remote_units)).collect();`
   and pass `&own` to `write_catalogs`. In the per-package loop:

   ```rust
   // A remote package is generated like a member's (spec 4.4) but is not
   // part of the consumer's contract snapshot: no IR dump is written for it.
   let package_emits: Vec<Emit> = if is_remote(&package.ir, &remote_units) {
       emits.iter().copied().filter(|emit| !emit.is_ir_dump()).collect()
   } else {
       emits.to_vec()
   };
   write_emits(out_dir, &base, &package.ir, &others, unit_compatible, &package_emits, …)
   ```

   `write_crate_files` keeps every package name (the generated code references
   `crate::veh::common::…`). Define:

   ```rust
   /// Whether `package` belongs to a loaded remote unit.
   pub fn is_remote(package: &ridl_ir::v2::Package, remote_units: &BTreeMap<String, RemoteUnit>) -> bool {
       remote_units.contains_key(ridl_ir::v2::unit_of(package))
   }
   ```

7. Delete `materialize_and_lock` and `detached_warning` (moved to ridl-core in
   Task 3). `front_end`'s `LoadedWorkspace` literal gets
   `remote_units: BTreeMap::new()` (added in Task 3) and `Compiled.remote_units`
   is filled from it.
8. Update the doc comment of `CompatibleCatalogsFn` ("The packages of an import
   are not among the checked packages" becomes "The packages of a remote unit
   are not among the packages given").

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p ridl-cli --test remote_units` then
`cargo test --workspace --locked` Expected: the seven tests pass;
`crates/ridl/tests/servers.rs` still asserts that `ridl mcp` writes no
`ridl.lock` (the frozen load writes none).

- [ ] **Step 5: Commit**

```bash
git add crates/ridlc/src/lib.rs crates/ridl-mcp/src/snapshot.rs crates/ridl/src/lock.rs crates/ridl/Cargo.toml crates/ridl/tests/remote_support crates/ridl/tests/remote_units.rs Cargo.lock
git commit -m "feat(ridlc)!: read remote units before the check and generate them

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

### Task 6: The facade, the language server and the MCP server

**Files:**

- Modify: `crates/ridl/src/lock.rs:114-118` (the `irs` list), `:175-230`
  (`locked_units`)
- Modify: `crates/ridl/src/property.rs:224-240` (the report loop)
- Modify: `crates/ridlc/src/diff_side.rs:210-214` (`DiffSide::packages`)
- Modify: `crates/ridl/src/catalogs.rs:96-101` (the note)
- Modify: `crates/ridl-lsp/src/server.rs:60` (imports), `:239-310`
  (`ServerState`), `:330-392` (`load`), `:672-800` (`analyze`), `:803-825`
  (`publish_all`)
- Test: `crates/ridl/tests/remote_units.rs` (append),
  `crates/ridl/tests/servers.rs` (append)

**Interfaces:**

- Consumes: `WorkspaceOutput.remote_units`, `is_remote`, `remote_dirs` (Task 5);
  `load_workspace_with_remotes`, `RemoteAccess` (Task 3).
- Produces: no new public API. `ServerState` gains
  `remote_files: HashSet<String>` and `remote_notice: Option<String>`.

- [ ] **Step 1: Write the failing tests**

Append to `crates/ridl/tests/remote_units.rs`:

```rust
/// `ridl baseline` publishes the consumer's snapshots only.
#[test]
fn baseline_publishes_the_consumers_packages_only() {
    let dir = TempDir::new("baseline");
    let stub = Stub::serving(unit_tar(COMMON_UNIT));
    let root = consumer(&dir, stub.url());
    let (code, _, stderr) = ridl(&root, dir.path(), &["lock"]);
    assert_eq!(code, 0, "{stderr}");
    let (code, _, stderr) = ridl(&root, dir.path(), &["baseline"]);
    assert_eq!(code, 0, "{stderr}");
    let published: Vec<String> = std::fs::read_dir(root.join(".ridl/baseline"))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert!(published.iter().any(|f| f == "app.ir.json"), "{published:?}");
    assert!(!published.iter().any(|f| f.starts_with("veh.common")), "{published:?}");
}

/// `ridl lock` allocates numbers in the consumer's units only; the remote
/// unit's `interfaces.lock` is its owner's.
#[test]
fn lock_writes_the_consumers_units_only() {
    let dir = TempDir::new("lock");
    let stub = Stub::serving(unit_tar(&[
        COMMON_UNIT[0],
        ("common.ridl", "package veh.common\n\ntype Speed : km/h [0.0..250.0 step 0.5]\n\ninterface Probe {\n  signal s: Speed @10ms\n}\n"),
    ]));
    let root = consumer(&dir, stub.url());
    let (code, _, stderr) = ridl(&root, dir.path(), &["lock"]);
    assert_eq!(code, 0, "{stderr}");
    assert!(root.join("interfaces.lock").is_file());
    let cache = dir.path().join(".ridl/cache");
    let mut stack = vec![cache];
    while let Some(next) = stack.pop() {
        for entry in std::fs::read_dir(&next).unwrap() {
            let path = entry.unwrap().path();
            assert_ne!(path.file_name().unwrap(), "interfaces.lock", "the cache is never written");
            if path.is_dir() {
                stack.push(path);
            }
        }
    }
}

/// `ridl test` samples the consumer's clauses only.
#[test]
fn test_runs_the_consumers_packages_only() {
    let dir = TempDir::new("test");
    // The clause faults on every sample (division by zero), as the
    // `ridl test` exit-1 cell of ADR-0010 decision 1 describes; the clause
    // form follows `crates/ridl/tests/property_cli.rs`.
    let stub = Stub::serving(unit_tar(&[
        COMMON_UNIT[0],
        ("common.ridl", "package veh.common\n\ntype Speed : km/h [0.0..250.0 step 0.5]\n\ninterface Probe {\n  command half(s: Speed) [\n    require 100.0 / (s - s) > 0.0\n  ] @[..50ms]\n}\n"),
    ]));
    let root = consumer(&dir, stub.url());
    let (code, stdout, stderr) = ridl(&root, dir.path(), &["test"]);
    assert_eq!(code, 0, "the faulting clause of the remote unit is not sampled: {stdout}{stderr}");
    assert!(!stdout.contains("veh.common"), "{stdout}");
}

/// `ridl diff` over two source trees compares the consumer's packages only:
/// a dependency whose type changed between the two sides is not the
/// consumer's breaking change.
#[test]
fn diff_compares_the_consumers_packages_only() {
    let dir = TempDir::new("diff");
    let old_stub = Stub::serving(unit_tar(COMMON_UNIT));
    let new_stub = Stub::serving(unit_tar(&[
        COMMON_UNIT[0],
        ("common.typl", "package veh.common\n\ntype Speed : m/s [0.0..70.0 step 0.5]\n"),
    ]));
    let old = consumer(&dir, old_stub.url());
    let new = TempDir::new("diff-new");
    let new_root = consumer(&new, new_stub.url());
    for root in [&old, &new_root] {
        let (code, _, stderr) = ridl(root, dir.path(), &["check"]);
        assert_eq!(code, 0, "{stderr}");
    }
    let (code, stdout, stderr) = ridl(
        &old,
        dir.path(),
        &["diff", "--format", "json", old.to_str().unwrap(), new_root.to_str().unwrap()],
    );
    assert_eq!(code, 0, "the consumer did not change: {stderr}");
    assert!(stdout.contains("\"identical\""), "{stdout}");
    assert!(!stdout.contains("veh.common"), "{stdout}");
}

/// The note `ridl build` prints beside a compatible list says a remote
/// unit's catalog is its owner's.
#[test]
fn the_catalog_note_names_the_owner() {
    let dir = TempDir::new("note");
    let stub = Stub::serving(unit_tar(COMMON_UNIT));
    let root = consumer(&dir, stub.url());
    let out = TempDir::new("note-out");
    for _ in 0..2 {
        let (code, _, stderr) = ridl(&root, dir.path(), &["lock"]);
        assert_eq!(code, 0, "{stderr}");
        let (code, _, stderr) = ridl(&root, dir.path(), &["baseline"]);
        assert_eq!(code, 0, "{stderr}");
    }
    let (code, _, stderr) = ridl(
        &root,
        dir.path(),
        &["build", "--out-dir", out.path().to_str().unwrap(), "--emit", "catalog"],
    );
    assert_eq!(code, 0, "{stderr}");
    assert!(stderr.contains("note: the workspace names `[imports]`"), "{stderr}");
    assert!(stderr.contains("published by its owner"), "{stderr}");
}
```

Append to `crates/ridl/tests/servers.rs`, next to the existing read-only tool
test (reuse its `connect`, `call_workspace_tool`, `TIMEOUT`, and the LSP helpers
`spawn_lsp_in`, `read_messages`, the `initialize` exchange the file already
writes), with `mod remote_support;` at the top:

```rust
/// `ridl mcp` resolves a cached remote unit and never fetches: a hit count
/// of zero on the stub, and no `ridl.lock` write.
#[tokio::test]
async fn mcp_reads_the_cache_and_never_fetches() {
    use remote_support::*;
    let dir = TempDir::new("mcp");
    let stub = Stub::serving(unit_tar(COMMON_UNIT));
    let root = consumer(&dir, stub.url());
    let (code, _, stderr) = ridl(&root, dir.path(), &["check"]);
    assert_eq!(code, 0, "{stderr}");
    let lock_before = std::fs::read_to_string(root.join("ridl.lock")).unwrap();
    stub.reset_hits();
    tokio::time::timeout(TIMEOUT, async {
        let client = connect_with_home(dir.path()).await;
        let result = call_workspace_tool(
            &client,
            "ridl_resolve",
            json!({"path": root.to_str().unwrap(), "name": "veh.common.Speed"}),
        )
        .await;
        assert_eq!(result.is_error, None, "{result:?}");
        let check = call_workspace_tool(&client, "ridl_check", json!({"path": root.to_str().unwrap()})).await;
        assert_eq!(check.structured_content.unwrap()["diagnostics"].as_array().unwrap().len(), 0);
        client.cancel().await.unwrap();
    })
    .await
    .expect("mcp remote timeout");
    assert_eq!(stub.hits(), 0, "the MCP server never fetches");
    assert_eq!(std::fs::read_to_string(root.join("ridl.lock")).unwrap(), lock_before);
}

/// Without a cache entry, `ridl_check` reports MANI-104 whose message names
/// `ridl check` as the fix, and still never fetches.
#[tokio::test]
async fn mcp_reports_a_cache_miss_as_mani_104() {
    use remote_support::*;
    let dir = TempDir::new("mcp-miss");
    let stub = Stub::serving(unit_tar(COMMON_UNIT));
    let root = consumer(&dir, stub.url());
    let (code, _, _) = ridl(&root, dir.path(), &["check"]);
    assert_eq!(code, 0);
    std::fs::remove_dir_all(dir.path().join(".ridl")).unwrap();
    stub.reset_hits();
    tokio::time::timeout(TIMEOUT, async {
        let client = connect_with_home(dir.path()).await;
        let check = call_workspace_tool(&client, "ridl_check", json!({"path": root.to_str().unwrap()})).await;
        let diagnostics = check.structured_content.unwrap()["diagnostics"].clone();
        let text = diagnostics.to_string();
        assert!(text.contains("MANI-104") && text.contains("ridl check"), "{text}");
        client.cancel().await.unwrap();
    })
    .await
    .expect("mcp miss timeout");
    assert_eq!(stub.hits(), 0);
}

/// `ridl lsp` publishes the consumer's diagnostics with the remote import
/// resolved, publishes nothing for a file of the remote unit, and never
/// fetches.
#[test]
fn lsp_resolves_a_cached_remote_unit_and_publishes_nothing_for_it() {
    use remote_support::*;
    let dir = TempDir::new("lsp");
    let stub = Stub::serving(unit_tar(WARNING_UNIT));
    let root = consumer(&dir, stub.url());
    let (code, _, stderr) = ridl(&root, dir.path(), &["check"]);
    assert_eq!(code, 0, "{stderr}");
    stub.reset_hits();
    let mut child = spawn_lsp_in_with_home(&[], Some(&root), dir.path());
    let messages = read_messages(child.stdout.take().unwrap());
    initialize(&mut child, &root);   // the file's existing initialize exchange
    let published = collect_publishes(&messages, std::time::Duration::from_secs(10));
    assert!(
        published.iter().all(|(uri, _)| !uri.contains(".ridl/cache")),
        "no diagnostic is published for a remote file: {published:?}"
    );
    let app = published.iter().find(|(uri, _)| uri.ends_with("app.ridl"));
    assert!(app.is_none_or(|(_, list)| list.is_empty()), "the import resolves: {app:?}");
    shutdown(&mut child);
    assert_eq!(stub.hits(), 0, "the language server never fetches");
}
```

`connect_with_home(home)` is `connect()` with `.env("HOME", home)` on the
spawned command; `spawn_lsp_in_with_home` likewise; `collect_publishes` drains
`textDocument/publishDiagnostics` notifications for the given duration into
`Vec<(String, Vec<Value>)>`. Add each beside the helper it extends.

- [ ] **Step 2: Run the tests to verify they fail**

Run:
`cargo test -p ridl-cli --test remote_units baseline_publishes lock_writes test_runs diff_compares the_catalog_note`
and `cargo test -p ridl-cli --test servers mcp_reads mcp_reports lsp_resolves`
Expected: `baseline_publishes…` and `lock_writes…` exit 2 with "no directory is
recorded for unit `veh.common`"; `test_runs…` exits 1 on the remote clause;
`diff_compares…` exits 1 (the remote type change reads as breaking); the note
test lacks "published by its owner"; the LSP test publishes RIDL-100 under a
cache URI (the server still loads with `load_workspace`), and `mcp_reads…`
passes already.

- [ ] **Step 3: Implement**

1. `crates/ridl/src/lock.rs`: filter `irs` with
   `.filter(|checked| !ridlc::is_remote(&checked.ir, &output.remote_units))`
   before `.map(|checked| checked.ir)`; the `retain_in_report_scope` call passes
   `&ridlc::remote_dirs(&output.remote_units)`. `locked_units` loads with
   `load_workspace` (no remotes) and is otherwise unchanged.
2. `crates/ridl/src/property.rs`:
   `.zip(&output.resolutions).filter(|(checked, _)| !ridlc::is_remote(&checked.ir, &output.remote_units))`.
3. `crates/ridlc/src/diff_side.rs`:
   `packages: output.checked.into_iter().filter(|c| !crate::is_remote(&c.ir, &output.remote_units)).map(|c| c.ir).collect()`,
   with a comment: a remote unit is not part of the consumer's contract; a
   change in a dependency is the dependency's diff.
4. `crates/ridl/src/catalogs.rs:96-101`: the note becomes
   `"note: the workspace names`[imports]`; a remote unit's catalog is published by its owner, so a unit's list covers the workspace's own packages only"`.
5. `crates/ridl-lsp/src/server.rs`:
   - import
     `ridl_core::{Cache, Frozen, RemoteAccess, load_workspace_with_remotes}` and
     drop `load_workspace` from the import list;
   - `ServerState` gains `remote_files: HashSet<String>` (the paths of every
     file of a remote package, recorded in `load` from `workspace.packages`
     whose `origin` is `Remote`) and `remote_notice: Option<String>`;
   - `load` calls
     `load_workspace_with_remotes(&mut self.db, dir, &[], RemoteAccess { frozen: Frozen::Yes, cache: &Cache::user_default() })`,
     destructures `remote_units` and `imports: _`, and before converting the
     loader diagnostics takes the detached Errors out of `diagnostics` into
     `self.remote_notice` as one string joined with `"\n"` ("`MANI-104`: …"),
     because `convert_loader_diagnostics` drops a span with no path;
   - `analyze` leaves the loop as it is (a remote package is checked, so the
     consumer's references resolve) and `batch` is called over
     `all.retain(|d| sources.path(d.primary.file).is_none_or(|p| !self.remote_files.contains(p)))`;
   - `publish_all` shows `self.remote_notice.take()` once with
     `show_message(connection, lt::MessageType::ERROR, notice)` before
     publishing, and never publishes a path in `remote_files` from
     `loader_diagnostics` either (skip it in the `for (path, loader)` loop).
6. `crates/ridl-mcp`: no change; `ridl_check` already includes detached
   diagnostics through `to_json`.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p ridl-cli --test remote_units --test servers` then
`cargo test --workspace --locked` and `just lint` Expected: all green.

- [ ] **Step 5: Commit**

```bash
git add crates/ridl/src crates/ridlc/src/diff_side.rs crates/ridl-lsp/src/server.rs crates/ridl/tests
git commit -m "feat(ridl)!: keep remote units out of lock, test, diff and the catalog chain

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

### Task 7: Records of PR 1 and the gate

**Files:**

- Modify: `docs/decisions/ADR-0002-module-system.md:3-30` (Status), `:337-354`
  (§5), `:369-383` (§7)
- Modify: `docs/decisions/ADR-0029-stability-policy.md:94` (the Manifest row)
- Modify: `xtask/src/freeze/manifest.rs:10-22` (the dump) and `:24-43` (its
  test); `freeze/manifest.txt` (regenerated)
- Modify: `docs/book/cli-reference.md:144-170` (the `ridl check` paragraph on
  `~/.ridl/cache` and `ridl.lock`)
- Modify: `docs/book/getting-started.md:165-200` ("A workspace": one paragraph
  on `[imports]`)
- Modify: `docs/technotes/walking-skeleton-architecture.md` (the sentence that
  says remote imports resolve nothing, if present —
  `grep -n "remote" docs/technotes/walking-skeleton-architecture.md`)

**Interfaces:** none.

- [ ] **Step 1: Write the failing test**

In `xtask/src/freeze/manifest.rs` tests, add to the expected lines list:

```rust
"ridl.lock.entries.*.sha256 string",
"ridl.lock.entries.*.tree string",
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p xtask manifest_dump_lists_each_key_with_its_value_type`
Expected: FAIL, `missing "ridl.lock.entries.*.sha256 string"`.

- [ ] **Step 3: Implement the dump and write the records**

`xtask/src/freeze/manifest.rs`: after the `KNOWN_KEYS` loop, add

```rust
for (key, value_type) in ridl_core::lock::LOCK_KEYS {
    lines.push(format!("ridl.lock.{key} {value_type}"));
}
```

and extend the module doc: "and the keys of `ridl.lock`, from
`ridl_core::lock::LOCK_KEYS`, prefixed `ridl.lock.`". Run
`cargo xtask freeze update`; `freeze/manifest.txt` gains the two lines and
nothing else changes.

`docs/decisions/ADR-0002-module-system.md`:

- Status, after the 2026-10-10 note:

  ```markdown
  Amended 2026-10-12 by the remote units design (driftsys/ridl#48, PR 1): §5
  states how a remote unit is read and checked (a flat, explicit dependency
  list; a fetched artifact is a unit; a remote unit is a read-only dependency),
  and §7 gains the `tree` hash of the unpacked tree beside the `sha256` of the
  fetched artifact, with MANI-105 to MANI-108.
  ```

- §5, after the rationale paragraph, a new sub-heading
  `**Remote units**
  (amended 2026-10-12)` with three paragraphs, one per
  decision V-1, V-2, V-3, in this wording:

  ```markdown
  **Remote units** (amended 2026-10-12, driftsys/ridl#48). The consumer's
  `[imports]` — the union of the root manifest's table and every member's —
  lists every remote unit the build needs, indirect ones included. A remote
  unit's own `[imports]` are read only to check that the consumer names each
  key; a key the consumer does not name is MANI-106 (Error), and when the two
  URLs for a key differ the consumer's URL is used, with no diagnostic.

  A fetched artifact is a unit (§1): its top level holds a `[package]`
  `ridl.toml` whose `name` equals the `[imports]` key that names its URL.
  Otherwise — no manifest at the top, a `[workspace]` manifest, or another
  name — the unit is MANI-105 (Error) and is not loaded; when two keys name
  one URL, the key that is not the name is MANI-105 and binds nothing. The
  ordinary rules apply to a remote unit unchanged: MANI-014, the
  directory-equals-package rule, and the cross-package cycle check across the
  boundary between the consumer and the unit.

  A remote unit is a read-only dependency. Its Errors are reported with the
  unit name and the file path, and fail the command; its Warnings and Infos
  are dropped before any `[lints]` level is applied, and the consumer's
  `[lints]` do not apply to it. Its packages are given to the code backends
  and generated like a member's; no IR snapshot, catalog, `interfaces.lock`,
  property run or diff side is produced for it, and `ridl fmt` never touches
  it. `internal` declarations of a remote unit are not visible to the
  consumer. Step 1 of the order above sees members only: a remote package is
  reached through the key that pins it, so a sibling member's pin is never
  visible.
  ```

- §7, after the second paragraph:

  ```markdown
  **The `tree` hash** (amended 2026-10-12, driftsys/ridl#48). Each `ridl.lock`
  entry pins two hashes: `sha256`, over the fetched artifact, and `tree`, over
  the unpacked tree. The `tree` hash is defined once, in `ridl-core`: the
  input is every regular file under the unit's directory, in ascending byte
  order of its relative path written with `/` separators; for each file the
  hash is fed the path bytes, one NUL byte, the file length as a
  little-endian u64, and the file bytes; a symbolic link or any other
  non-regular file is an error (MANI-107, naming the file) and no tree that
  holds one is loaded; the result is the SHA-256 in lowercase hex. A fetch
  computes `tree` after unpacking; a cache hit is verified against it
  (MANI-107 on a mismatch). An entry written before this amendment has no
  `tree`: the next run without `--frozen` adds it, and a run with `--frozen`
  raises MANI-108. `ridl lsp` and `ridl mcp` never fetch: they read the cache
  as a frozen run does, and a miss is MANI-103 or MANI-104 whose message names
  `ridl check` as the fix. The tar format (ADR-0007 decision 12) stays
  provisional.
  ```

`docs/decisions/ADR-0029-stability-policy.md:94`: the Manifest row's second cell
lists `ridl.toml` keys and their value types, then `ridl.lock` entries and their
keys, then `interfaces.lock` and `.ridl/baseline`, with a dated note under the
table: "Amended 2026-10-12 (driftsys/ridl#48): the Manifest row gains
`ridl.lock`."

`docs/book/cli-reference.md:144-170`: rewrite the paragraph so that it says what
is built now — `ridl check` materializes each unit (cache hit, else fetch),
verifies its `tree`, loads it as a read-only dependency whose errors fail the
check and whose warnings are not shown, and regenerates `ridl.lock` with both
hashes; update the quoted `ridl.lock` transcript to show a `tree = "…"` line
under `sha256`; keep the `--frozen` sentence and add that MANI-108 is the frozen
outcome of a lockfile that predates `tree`.

`docs/book/getting-started.md`, after the paragraph that ends "(TYPL-002)." in
"A workspace":

````markdown
A package that lives in another repository is named in an `[imports]` table of
the root manifest, under the key its `ridl.toml` declares as its `name`:

```toml
[imports]
"veh.common" = "https://registry.example.com/veh/common@v1.0.0"
```

`ridl check` fetches it once into `~/.ridl/cache`, pins it in `ridl.lock`, and
from then on reads the cache; `import veh.common.Speed` then resolves as it
does for a member. A remote unit is a read-only dependency: its errors fail the
check, its warnings are not shown, and `ridl build` generates it beside your own
packages. The [CLI reference](cli-reference.md#ridl-check) says what each
command writes.
````

The fence is a `toml` fence, so `book_examples.rs` does not compile it.

- [ ] **Step 4: Run the gate**

Run: `cargo test -p xtask`, then `just build`. Expected: green.
`just freeze-check` passes with the regenerated `freeze/manifest.txt` and
`freeze/diagnostics.txt`; `just book-check`, `just link-check`,
`just doc-path-check`, `just story-id-check` pass.

- [ ] **Step 5: Commit, then open PR 1**

```bash
git add docs/decisions/ADR-0002-module-system.md docs/decisions/ADR-0029-stability-policy.md xtask/src/freeze/manifest.rs freeze/manifest.txt docs/book/cli-reference.md docs/book/getting-started.md docs/technotes/walking-skeleton-architecture.md
git commit -m "docs(adr): record the remote unit read path and the tree hash

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

PR 1 body: the breaking change (spec 4.6) as a delegated ruling; "Closes item I3
of driftsys/ridl#135; part of driftsys/ridl#48"; the heads-up issue filed on
driftsys/ridlc-gen-kotlin ("a plugin now receives the packages of every remote
unit beside the consumer's; `ridl_ir::v2::unit_of` tells them apart, and a
remote unit never gets a catalog").

---

## Part 2 — PR 2, `ridl vendor` (spec section 5)

Branch: `feat/ridl-vendor-command`, from the merge commit of PR 1. PR title:
`feat(ridl)!: add ridl vendor and the vendor directory`.

### Task 8: The loader reserves `<manifest root>/vendor/`

**Files:**

- Modify: `crates/ridl-core/src/diag.rs:1389-1392` (`MANI_CATALOG`, after
  `MANI_016`), `:2176-2178` (the lint-name list), `:2226-2231` (the severity
  rule)
- Modify: `docs/specification/ridl-family-overview.md:354` (after MANI-016)
- Modify: `docs/book/lints.md:162` (after the `unknown-lint` row)
- Modify: `crates/ridl-core/src/workspace.rs:520-558` (`Loader`: a `root`
  field), `:563-673` (`load_root`), `:652-669` (the member loop), `:932-973`
  (the subdirectory loop of `load_package_tree`)
- Modify: `crates/ridl/src/main.rs:2857-2906` (`run_fmt`), `:2917-2945`
  (`collect_source_files`)
- Test: `crates/ridl-core/src/workspace.rs` tests;
  `crates/ridl/tests/vendor_cli.rs` (new, the fmt test)

**Interfaces:**

- Produces: `DiagCode::MANI_017` (Error), `DiagCode::MANI_018` (Warning, lint
  `vendor-source-not-loaded`, default `warn`). `Loader.root: PathBuf`.
  `collect_source_files(path: &Path, skip: Option<&Path>)` in `main.rs`.

- [ ] **Step 1: Write the failing tests**

`crates/ridl-core/src/workspace.rs` tests:

```rust
    /// In standalone mode the walk skips `<root>/vendor/`, and each source
    /// file under it is MANI-018 on the file, not loaded.
    #[test]
    fn a_source_under_the_roots_vendor_is_mani_018_and_not_loaded() {
        let fixture = TempDir::new("vendor-018");
        fixture.write("app/ridl.toml", "[package]\nname = \"app\"\nversion = \"1.0.0\"\n");
        fixture.write("app/app.typl", "package app\n");
        fixture.write("app/vendor/veh.common/ridl.toml", "[package]\nname = \"veh.common\"\nversion = \"1.0.0\"\n");
        fixture.write("app/vendor/veh.common/common.typl", "package veh.common\n");
        fixture.write("app/vendor/veh.common/sub/s.ridl", "package veh.common.sub\n");
        let mut db = RidlDatabase::default();
        let loaded = load_workspace(&mut db, &fixture.path().join("app")).unwrap();
        let codes: Vec<&str> = loaded.diagnostics.iter().map(|d| d.code.as_str()).collect();
        assert_eq!(codes, ["MANI-018", "MANI-018"], "one per file, no MANI-013: {:?}", loaded.diagnostics);
        assert!(loaded.diagnostics.iter().all(|d| d.severity == Severity::Warning));
        assert_eq!(loaded.workspace.packages(&db).len(), 1);
    }

    /// A directory named `vendor` deeper in the tree is an ordinary package.
    #[test]
    fn a_deeper_vendor_directory_is_a_package() {
        let fixture = TempDir::new("vendor-deep");
        fixture.write("app/ridl.toml", "[package]\nname = \"app\"\nversion = \"1.0.0\"\n");
        fixture.write("app/parts/vendor/v.typl", "package app.parts.vendor\n");
        let mut db = RidlDatabase::default();
        let loaded = load_workspace(&mut db, &fixture.path().join("app")).unwrap();
        assert!(loaded.diagnostics.is_empty(), "{:?}", loaded.diagnostics);
        let names: Vec<String> = loaded.workspace.packages(&db).iter().map(|p| p.name(&db).clone()).collect();
        assert_eq!(names, ["app.parts.vendor"]);
    }

    /// In workspace mode a `members` entry of `vendor`, or under it, is
    /// MANI-017 and the member is not loaded.
    #[test]
    fn a_vendor_member_is_mani_017() {
        let fixture = TempDir::new("vendor-017");
        fixture.write("ws/ridl.toml", "[workspace]\nmembers = [\"app\", \"vendor\", \"vendor/x\"]\n");
        fixture.write("ws/app/ridl.toml", "[package]\nname = \"app\"\nversion = \"1.0.0\"\n");
        fixture.write("ws/vendor/ridl.toml", "[package]\nname = \"vendor\"\nversion = \"1.0.0\"\n");
        fixture.write("ws/vendor/x/ridl.toml", "[package]\nname = \"x\"\nversion = \"1.0.0\"\n");
        fixture.write("ws/vendor/x/x.typl", "package x\n");
        let mut db = RidlDatabase::default();
        let loaded = load_workspace(&mut db, &fixture.path().join("ws")).unwrap();
        let codes: Vec<&str> = loaded.diagnostics.iter().map(|d| d.code.as_str()).collect();
        assert_eq!(codes, ["MANI-017", "MANI-017"]);
        assert!(loaded.workspace.packages(&db).is_empty(), "no package of a vendor member loads");
        assert_eq!(loaded.units.len(), 0);
    }
```

In `crates/ridl-core/src/diag.rs` tests: add
`("MANI-018", "vendor-source-not-loaded")` to the expected lint-name list, and
make the severity rule read
`entry.code == DiagCode::MANI_005 || entry.code == DiagCode::MANI_010 || entry.code == DiagCode::MANI_018`
with its doc comment saying "the unknown-key, unknown-lint and
vendor-source-not-loaded warnings".

Create `crates/ridl/tests/vendor_cli.rs` with `mod remote_support;` and:

```rust
//! `vendor/` and `ridl vendor`, against the built `ridl` binary.

mod remote_support;

use remote_support::*;

/// `ridl fmt` over the manifest root walks the consumer's sources only: a
/// file under `vendor/` is neither rewritten nor reported.
#[test]
fn fmt_never_touches_vendor() {
    let dir = TempDir::new("fmt-vendor");
    dir.write("app/ridl.toml", "[package]\nname = \"app\"\nversion = \"1.0.0\"\n");
    dir.write("app/app.typl", "package app\n");
    let unformatted = "package   veh.common\ntype   Speed :  km/h\n";
    dir.write("app/vendor/veh.common/ridl.toml", "[package]\nname = \"veh.common\"\nversion = \"1.0.0\"\n");
    let vendored = dir.write("app/vendor/veh.common/common.typl", unformatted);
    let root = dir.path().join("app");
    let (code, _, stderr) = ridl(&root, dir.path(), &["fmt", "--check"]);
    assert_eq!(code, 0, "{stderr}");
    let (code, _, _) = ridl(&root, dir.path(), &["fmt"]);
    assert_eq!(code, 0);
    assert_eq!(std::fs::read_to_string(&vendored).unwrap(), unformatted);
    // An explicit path under `vendor/` is refused.
    let (code, _, stderr) = ridl(&root, dir.path(), &["fmt", "vendor/veh.common"]);
    assert_eq!(code, 2, "{stderr}");
    assert!(stderr.contains("vendor/"), "{stderr}");
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run:
`cargo test -p ridl-core --lib workspace::tests::a_source_under_the_roots_vendor workspace::tests::a_deeper_vendor workspace::tests::a_vendor_member diag::`
and `cargo test -p ridl-cli --test vendor_cli fmt_never_touches_vendor`
Expected: the loader tests see `MANI-013` and loaded packages instead of
MANI-018/017; `diag::` fails on the missing constants; the fmt test rewrites the
vendored file and exits 0 on the explicit path.

- [ ] **Step 3: Implement**

`crates/ridl-core/src/diag.rs`, after `MANI_016`:

```rust
        /// In workspace mode, a `members` entry names `vendor` or a path
        /// under it. `<manifest root>/vendor/` holds the remote units
        /// `ridl vendor` writes and is never a member (ADR-0002 §7).
        MANI_017 = "MANI-017", Error,
            "a workspace member is `vendor` or a path under it";

        /// In standalone mode, a `.typl`, `.ridl` or `.rsdl` file under the
        /// root's `vendor/` is not loaded: the directory is reserved for
        /// `ridl vendor` (ADR-0002 §7). One warning per file.
        MANI_018 = "MANI-018", Warning,
            "a source file under the root's `vendor/` is not loaded", lint = "vendor-source-not-loaded";
```

`docs/specification/ridl-family-overview.md` after the MANI-016 row:

```markdown
| MANI-017 | a workspace member is `vendor` or a path under it | error |
| MANI-018 | a source file under the root's `vendor/` is not loaded | warning |
```

`docs/book/lints.md` after the `unknown-lint` row:

```markdown
| `vendor-source-not-loaded` | MANI-018 | warn | a source file under the root's `vendor/` is not loaded |
```

`crates/ridl-core/src/workspace.rs`:

- `Loader` gains `root: PathBuf`, set at the top of `load_root`
  (`self.root = root.to_path_buf()`).
- `pub const VENDOR_DIR: &str = "vendor";` beside `ImportUnion`.
- Member loop in `load_root`: before `self.member_dirs.push`, when
  `normalize(&member_dir).starts_with(normalize(&root.join(VENDOR_DIR)))` push
  `error(DiagCode::MANI_017, file_id, member_entry_range(&text, member), format!("workspace member`{member}`is`vendor`or a path under it;`vendor/`at the manifest root holds the remote units`ridl
  vendor`writes and is never a member"))` and `continue`.
- Subdirectory loop of `load_package_tree`: before the `nested_manifest` check,
  when `normalize(&subdir) == normalize(&self.root.join(VENDOR_DIR))`: call
  `self.report_vendor_sources(&subdir)` and `continue`:

  ```rust
  /// Reports MANI-018 on every `.typl`, `.ridl` and `.rsdl` file under the
  /// root's `vendor/`, which the walk does not load. The span is the empty
  /// range of the file, interned with no text, as RIDL-417 is.
  fn report_vendor_sources(&mut self, vendor: &Path) -> io::Result<()> {
      let mut pending = vec![vendor.to_path_buf()];
      let mut files = Vec::new();
      while let Some(dir) = pending.pop() {
          for entry in fs::read_dir(&dir).map_err(|e| cannot_read(&dir, e))? {
              let entry = entry.map_err(|e| cannot_read(&dir, e))?;
              let path = entry.path();
              if path.is_dir() && !entry.file_type().map_err(|e| cannot_read(&path, e))?.is_symlink() {
                  pending.push(path);
              } else if path.extension().is_some_and(|ext| ext == "typl" || ext == "ridl" || ext == "rsdl") {
                  files.push(path);
              }
          }
      }
      files.sort();
      for path in files {
          let file_id = self.sources.file_id(&path_string(&path), "");
          self.diagnostics.push(warning(
              DiagCode::MANI_018,
              file_id,
              byte_range(0, 0),
              format!(
                  "`{}` is under the manifest root's `vendor/`, which holds the remote units `ridl vendor` writes; it is not loaded as a source of this unit",
                  path.display()
              ),
          ));
      }
      Ok(())
  }
  ```

  This runs only when the walk reaches `<root>/vendor` as a subdirectory, which
  is standalone mode (a workspace never walks its root). The remote-unit load of
  Task 9 reads `vendor/<key>/` through `load_remote_unit`, not through this
  walk.

`crates/ridl/src/main.rs`: `collect_source_files(path, skip: Option<&Path>)`
skips a directory whose normalized path equals `skip`; `run_fmt` computes
`let vendor = ridl_core::find_root(if path.is_file() { path.parent().unwrap_or(path) } else { path }).map(|root| root.join("vendor"))`
(`index_root` at `:2718` wraps `find_root` the same way but falls back to the
entry; `fmt` needs the `None`), and refuses an explicit `path` under `vendor`
with `eprintln!("error:`{}`is under`vendor/`, which`ridl
fmt`never formats: a vendored unit is its owner's", path.display())` and exit 2;
otherwise passes `vendor.as_deref()` as `skip`.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p ridl-core --lib workspace:: diag::`,
`cargo test -p ridl-cli --test vendor_cli --test book_lints`,
`cargo xtask freeze update` (diagnostics dump gains two lines). Expected: pass.

- [ ] **Step 5: Commit**

```bash
git add crates/ridl-core/src crates/ridl/src/main.rs crates/ridl/tests/vendor_cli.rs docs/specification/ridl-family-overview.md docs/book/lints.md freeze/diagnostics.txt
git commit -m "feat(ridl-core)!: reserve vendor/ at the manifest root

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

### Task 9: The read path reads `vendor/` before the cache

**Files:**

- Modify: `crates/ridl-core/src/fetch.rs` (`materialize_imports` and
  `from_cache_or_fetch` from Task 2; tests)
- Modify: `crates/ridl-core/src/workspace.rs` (`RemoteAccess`, `load_remotes`
  from Task 3)
- Modify: `crates/ridlc/src/lib.rs` (`load_and_check`: builds the access with
  `VendorRead::Read`); `crates/ridl-lsp/src/server.rs` (same)
- Test: `crates/ridl-core/src/fetch.rs`, `crates/ridl/tests/vendor_cli.rs`

**Interfaces:**

- Produces:

  ```rust
  // workspace.rs
  /// Whether a load reads `<manifest root>/vendor/<key>/` before the cache.
  #[derive(Clone, Copy, PartialEq, Eq, Debug)]
  pub enum VendorRead { Read, Skip }
  pub struct RemoteAccess<'a> { pub frozen: Frozen, pub cache: &'a Cache, pub vendor: VendorRead }
  impl<'a> RemoteAccess<'a> {
      /// The ordinary access: `vendor/`, then the cache, then (not frozen) a fetch.
      pub fn new(frozen: Frozen, cache: &'a Cache) -> Self;
      /// `ridl vendor`: the cache, then a fetch; `vendor/` is what it writes, not what it reads.
      pub fn refresh(cache: &'a Cache) -> Self;
  }
  // fetch.rs
  pub fn materialize_imports(
      imports: &ImportUnion,
      lock: Option<&Lockfile>,
      cache: &Cache,
      frozen: Frozen,
      vendor: Option<&Path>,
  ) -> (Vec<MaterializedUnit>, Lockfile, Vec<Diagnostic>);
  ```

- [ ] **Step 1: Write the failing tests**

`crates/ridl-core/src/fetch.rs` tests (every existing call gains a trailing
`None`):

```rust
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
        let (_, lock, _) = materialize_imports(&imports_of(stub.url()), None, &cache, Frozen::No, None);
        let vendor = vendored(&dir, "remote.pkg", &[("remote.typl", b"package remote.pkg\n")]);
        std::fs::remove_dir_all(&cache.root).unwrap();
        stub.reset_hits();
        for frozen in [Frozen::Yes, Frozen::No] {
            let (units, regenerated, diags) =
                materialize_imports(&imports_of(stub.url()), Some(&lock), &cache, frozen, Some(&vendor));
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
        let (_, lock, _) = materialize_imports(&imports_of(stub.url()), None, &cache, Frozen::No, None);
        let vendor = vendored(&dir, "remote.pkg", &[("remote.typl", b"package remote.pkg\r\n")]);
        let (units, _, diags) =
            materialize_imports(&imports_of(stub.url()), Some(&lock), &cache, Frozen::Yes, Some(&vendor));
        assert_eq!(codes(&diags), vec!["MANI-107"]);
        assert!(diags[0].message.contains("ridl vendor"), "{}", diags[0].message);
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
        let vendor = vendored(&dir, "remote.pkg", &[("remote.typl", b"package remote.pkg\n")]);
        let (units, _, diags) =
            materialize_imports(&imports_of(stub.url()), None, &cache, Frozen::Yes, Some(&vendor));
        assert_eq!(codes(&diags), vec!["MANI-103"]);
        assert!(units.is_empty());
        let (units, lock, diags) =
            materialize_imports(&imports_of(stub.url()), None, &cache, Frozen::No, Some(&vendor));
        assert!(diags.is_empty(), "{diags:?}");
        assert_eq!(stub.hits(), 1);
        assert_eq!(units[0].dir, vendor.join("remote.pkg"));
        assert!(lock.entries[stub.url()].tree.is_some());
    }

    /// A symbolic link in `vendor/<key>/` is MANI-107 naming the link
    /// (Review Focus 1).
    #[cfg(unix)]
    #[test]
    fn a_symlink_in_the_vendor_tree_is_mani_107() {
        let stub = Stub::serving(make_tar("remote.typl", b"package remote.pkg\n"));
        let dir = TempDir::new("vendor-link");
        let cache = cache(&dir);
        let (_, lock, _) = materialize_imports(&imports_of(stub.url()), None, &cache, Frozen::No, None);
        let vendor = vendored(&dir, "remote.pkg", &[("remote.typl", b"package remote.pkg\n")]);
        std::os::unix::fs::symlink("remote.typl", vendor.join("remote.pkg/alias.typl")).unwrap();
        let (units, _, diags) =
            materialize_imports(&imports_of(stub.url()), Some(&lock), &cache, Frozen::Yes, Some(&vendor));
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
        let vendor = vendored(&dir, "remote.pkg", &[("remote.typl", b"package remote.pkg\n")]);
        std::fs::remove_dir_all(&cache.root).unwrap();
        let (units, _, diags) = materialize_imports(&imports, Some(&lock), &cache, Frozen::Yes, Some(&vendor));
        assert!(diags.is_empty(), "{diags:?}");
        assert_eq!(units[0].dir, vendor.join("remote.pkg"));
    }
```

Append to `crates/ridl/tests/vendor_cli.rs`:

```rust
/// Writes the `COMMON_UNIT` files as `<root>/vendor/veh.common/`.
fn vendor_common(root: &std::path::Path) {
    for (name, text) in COMMON_UNIT {
        let path = root.join("vendor/veh.common").join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }
}

/// A frozen `ridl check` with an empty cache reads `vendor/`: the
/// air-gapped build.
#[test]
fn frozen_check_reads_vendor_with_an_empty_cache() {
    let dir = TempDir::new("frozen-vendor");
    let stub = Stub::serving(unit_tar(COMMON_UNIT));
    let root = consumer(&dir, stub.url());
    let (code, _, stderr) = ridl(&root, dir.path(), &["check"]);
    assert_eq!(code, 0, "{stderr}");
    vendor_common(&root);
    std::fs::remove_dir_all(dir.path().join(".ridl")).unwrap();
    stub.reset_hits();
    let (code, _, stderr) = ridl(&root, dir.path(), &["check", "--frozen"]);
    assert_eq!(code, 0, "{stderr}");
    assert_eq!(stub.hits(), 0);
    // An error inside the vendored tree names the vendor path.
    std::fs::write(root.join("vendor/veh.common/common.typl"), BROKEN_UNIT[1].1).unwrap();
    let (code, _, stderr) = ridl(&root, dir.path(), &["check", "--frozen"]);
    assert_eq!(code, 1, "{stderr}");
    assert!(stderr.contains("MANI-107"), "the edited tree no longer matches: {stderr}");
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p ridl-core --lib fetch::` and
`cargo test -p ridl-cli --test vendor_cli frozen_check_reads_vendor` Expected:
compile error on the fifth argument; the CLI test exits 1 with MANI-104 (the
cache is empty and `vendor/` is not read).

- [ ] **Step 3: Implement**

`crates/ridl-core/src/fetch.rs`: `materialize_imports` gains
`vendor: Option<&Path>`. Per URL, after the `is_fetchable_url` check:

```rust
let keys = &imports[url];
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
        verify_vendored(url, dir, tree, &mut diagnostics).map(|()| {
            (
                LockEntry {
                    sha256: pinned.expect("a tree implies an entry").sha256.clone(),
                    tree: Some(tree.to_string()),
                },
                MaterializedUnit { url: url.clone(), dir: dir.clone(), tree: tree.to_string() },
            )
        })
    }
    _ => from_cache_or_fetch(url, pinned, cache, frozen, &mut diagnostics).and_then(
        |(entry, unit)| match &vendored {
            Some(dir) => verify_vendored(url, dir, &unit.tree, &mut diagnostics)
                .map(|()| (entry, MaterializedUnit { dir: dir.clone(), ..unit })),
            None => Some((entry, unit)),
        },
    ),
};
if let Some((entry, unit)) = resolved {
    regenerated.entries.insert(url.clone(), entry);
    units.push(unit);
}
```

with

```rust
/// `Some(())` when the vendored `dir` hashes to `expected`; `None` after a
/// MANI-107 that names `ridl vendor` as the fix.
fn verify_vendored(url: &str, dir: &Path, expected: &str, diagnostics: &mut Vec<Diagnostic>) -> Option<()> {
    let actual = hash_tree(url, dir, diagnostics)?;
    if actual == expected {
        return Some(());
    }
    diagnostics.push(detached(
        DiagCode::MANI_107,
        format!(
            "the vendored tree `{}` does not match the `tree` hash `ridl.lock` pins for `{url}`; run `ridl vendor` to rewrite it",
            dir.display()
        ),
    ));
    None
}
```

The MANI-104 message gains ", or `ridl vendor` to vendor it" at its end (spec
4.1: the language server's and the MCP server's fix).

`crates/ridl-core/src/workspace.rs`: `VendorRead`, the `vendor` field and the
two constructors from Interfaces; `load_remotes` passes
`match access.vendor { VendorRead::Read => Some(root.join(VENDOR_DIR)), VendorRead::Skip => None }.as_deref()`.
The Task 3 tests that build `RemoteAccess { frozen, cache }` switch to
`RemoteAccess::new(frozen, &cache)`; so do `crates/ridlc/src/lib.rs` and
`crates/ridl-lsp/src/server.rs`.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p ridl-core --lib fetch:: workspace::` and
`cargo test -p ridl-cli --test vendor_cli --test remote_units` Expected: pass.

- [ ] **Step 5: Commit**

```bash
git add crates/ridl-core/src crates/ridlc/src/lib.rs crates/ridl-lsp/src/server.rs crates/ridl/tests/vendor_cli.rs
git commit -m "feat(ridl-core)!: read a remote unit from vendor/ before the cache

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

### Task 10: `ridl vendor [PATH]`

**Files:**

- Create: `crates/ridl-core/src/vendor.rs` (`write_vendor`, behind `fetch`)
- Modify: `crates/ridl-core/src/lib.rs` (module and re-exports)
- Modify: `crates/ridlc/src/lib.rs:382-429` (a `compile_workspace_with_access`
  beside `compile_workspace_with`)
- Create: `crates/ridl/src/vendor.rs`
- Modify: `crates/ridl/src/main.rs:79-250` (the `Vendor` variant, after
  `Describe`), `:300-395` (dispatch)
- Test: `crates/ridl-core/src/vendor.rs` tests;
  `crates/ridl/tests/vendor_cli.rs`

**Interfaces:**

- Produces:

  ```rust
  // ridl-core vendor.rs
  /// What `write_vendor` changed.
  #[derive(Debug, Default, PartialEq, Eq)]
  pub struct VendorWrite { pub written: Vec<String>, pub removed: Vec<String>, pub unchanged: Vec<String> }
  /// Brings `<root>/vendor/` to exactly `units` (spec 5.2 step 4).
  pub fn write_vendor(root: &Path, units: &BTreeMap<String, RemoteUnit>) -> io::Result<VendorWrite>;

  // ridlc
  /// `compile_workspace_with` with the remote access the caller chooses.
  pub fn compile_workspace_with_access(db: &mut RidlDatabase, entry: &Path, overlays: &[Overlay], access: RemoteAccess<'_>) -> Result<WorkspaceOutput, LoadError>;

  // ridl main.rs
  /// Copy every remote unit of the workspace's `[imports]` into `vendor/`
  /// at the manifest root, so a `--frozen` build needs no network and no
  /// cache. Exit 0 when `vendor/` was written or there was nothing to
  /// change, 1 on a diagnostic error (nothing written), 2 on a missing or
  /// unreadable path, a fetch failure, or an I/O failure.
  Vendor {
      #[arg(default_value = ".")]
      path: PathBuf,
  },
  ```

  Task 11 adds the `--check` flag and extends the doc comment.

- [ ] **Step 1: Write the failing tests**

`crates/ridl-core/src/vendor.rs` tests (own `TempDir` as in `tree_hash.rs`):

```rust
    fn unit(dir: &Path, files: &[(&str, &str)]) -> RemoteUnit {
        for (name, text) in files {
            let path = dir.join(name);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, text).unwrap();
        }
        RemoteUnit { url: "https://r.example.com/u@v1".into(), dir: dir.to_path_buf(), tree: tree_hash(dir).unwrap() }
    }

    /// The first write copies each unit byte for byte under its name.
    #[test]
    fn writes_each_unit_under_its_name() {
        let fixture = TempDir::new("write");
        let source = unit(&fixture.path().join("src/a"), &[("ridl.toml", "[package]\nname = \"a\"\n"), ("sub/s.typl", "package a.sub\r\n")]);
        let root = fixture.path().join("root");
        fs::create_dir_all(&root).unwrap();
        let report = write_vendor(&root, &BTreeMap::from([("a".to_string(), source.clone())])).unwrap();
        assert_eq!(report.written, ["a"]);
        assert_eq!(tree_hash(&root.join("vendor/a")).unwrap(), source.tree);
        assert_eq!(fs::read(root.join("vendor/a/sub/s.typl")).unwrap(), b"package a.sub\r\n");
    }

    /// A second write with the same set changes nothing and reports it.
    #[test]
    fn an_unchanged_unit_is_left_alone() {
        let fixture = TempDir::new("unchanged");
        let source = unit(&fixture.path().join("src/a"), &[("a.typl", "package a\n")]);
        let root = fixture.path().join("root");
        fs::create_dir_all(&root).unwrap();
        let units = BTreeMap::from([("a".to_string(), source)]);
        write_vendor(&root, &units).unwrap();
        let before = fs::metadata(root.join("vendor/a/a.typl")).unwrap().modified().unwrap();
        let report = write_vendor(&root, &units).unwrap();
        assert_eq!(report.unchanged, ["a"]);
        assert!(report.written.is_empty());
        assert_eq!(fs::metadata(root.join("vendor/a/a.typl")).unwrap().modified().unwrap(), before);
    }

    /// A unit no longer imported is removed; a hidden entry and a file are
    /// left alone.
    #[test]
    fn a_unit_no_longer_imported_is_removed() {
        let fixture = TempDir::new("remove");
        let a = unit(&fixture.path().join("src/a"), &[("a.typl", "package a\n")]);
        let b = unit(&fixture.path().join("src/b"), &[("b.typl", "package b\n")]);
        let root = fixture.path().join("root");
        fs::create_dir_all(&root).unwrap();
        write_vendor(&root, &BTreeMap::from([("a".to_string(), a.clone()), ("b".to_string(), b)])).unwrap();
        fs::write(root.join("vendor/README.md"), "kept").unwrap();
        let report = write_vendor(&root, &BTreeMap::from([("a".to_string(), a)])).unwrap();
        assert_eq!(report.removed, ["b"]);
        assert!(!root.join("vendor/b").exists());
        assert!(root.join("vendor/README.md").is_file());
    }

    /// A staging directory left by an interrupted run is removed first, and
    /// a changed unit is swapped in whole (Review Focus 5).
    #[test]
    fn a_stale_staging_directory_is_cleared_and_the_swap_is_whole() {
        let fixture = TempDir::new("staging");
        let old = unit(&fixture.path().join("src/old"), &[("a.typl", "package a\n"), ("gone.typl", "package a\n")]);
        let root = fixture.path().join("root");
        fs::create_dir_all(&root).unwrap();
        write_vendor(&root, &BTreeMap::from([("a".to_string(), old)])).unwrap();
        fs::create_dir_all(root.join("vendor/.staging-a-999")).unwrap();
        fs::write(root.join("vendor/.staging-a-999/partial.typl"), "x").unwrap();
        let new = unit(&fixture.path().join("src/new"), &[("a.typl", "package a\ntype T: m\n")]);
        let report = write_vendor(&root, &BTreeMap::from([("a".to_string(), new.clone())])).unwrap();
        assert_eq!(report.written, ["a"]);
        assert_eq!(tree_hash(&root.join("vendor/a")).unwrap(), new.tree);
        assert!(!root.join("vendor/a/gone.typl").exists(), "the old tree is replaced whole");
        let leftovers: Vec<String> = fs::read_dir(root.join("vendor")).unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|n| n.starts_with('.'))
            .collect();
        assert!(leftovers.is_empty(), "{leftovers:?}");
    }
```

Append to `crates/ridl/tests/vendor_cli.rs`:

```rust
/// `ridl vendor` writes each imported unit to `vendor/<key>/`, pins it, and
/// prints what it wrote; a second run reports nothing to change.
#[test]
fn vendor_writes_the_units_and_is_idempotent() {
    let dir = TempDir::new("vendor");
    let stub = Stub::serving(unit_tar(COMMON_UNIT));
    let root = consumer(&dir, stub.url());
    let (code, stdout, stderr) = ridl(&root, dir.path(), &["vendor"]);
    assert_eq!(code, 0, "{stderr}");
    assert!(stdout.contains("veh.common"), "{stdout}");
    assert!(root.join("vendor/veh.common/common.typl").is_file());
    assert!(root.join("ridl.lock").is_file());
    let (code, stdout, _) = ridl(&root, dir.path(), &["vendor"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("nothing to change"), "{stdout}");
    assert_eq!(stub.hits(), 1, "the second run reads the cache");
}

/// A URL change in `[imports]` after a vendor run is fetched and the
/// directory replaced: `ridl vendor` never reads `vendor/` as a source.
#[test]
fn vendor_replaces_a_unit_whose_url_changed() {
    let dir = TempDir::new("vendor-url-change");
    let stub = Stub::serving(unit_tar(COMMON_UNIT));
    let root = consumer(&dir, stub.url());
    assert_eq!(ridl(&root, dir.path(), &["vendor"]).0, 0);
    let newer = Stub::serving(unit_tar(&[
        COMMON_UNIT[0],
        ("common.typl", "package veh.common\n\ntype Speed : km/h [0.0..400.0 step 0.5]\n"),
    ]));
    consumer(&dir, newer.url());
    let (code, _, stderr) = ridl(&root, dir.path(), &["vendor"]);
    assert_eq!(code, 0, "{stderr}");
    assert!(std::fs::read_to_string(root.join("vendor/veh.common/common.typl")).unwrap().contains("400"));
    let (code, _, stderr) = ridl(&root, dir.path(), &["check", "--frozen"]);
    assert_eq!(code, 0, "{stderr}");
}

/// A unit no longer imported is removed from `vendor/`.
#[test]
fn vendor_removes_a_unit_no_longer_imported() {
    let dir = TempDir::new("vendor-remove");
    let stub = Stub::serving(unit_tar(COMMON_UNIT));
    let root = consumer(&dir, stub.url());
    assert_eq!(ridl(&root, dir.path(), &["vendor"]).0, 0);
    dir.write("app/ridl.toml", "[package]\nname = \"app\"\nversion = \"1.0.0\"\n");
    dir.write("app/app.ridl", "package app\n");
    let (code, stdout, _) = ridl(&root, dir.path(), &["vendor"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("removed veh.common"), "{stdout}");
    assert!(!root.join("vendor/veh.common").exists());
}

/// A diagnostic error leaves `vendor/` as it was and exits 1; a fetch
/// failure exits 2.
#[test]
fn vendor_writes_nothing_on_an_error() {
    let dir = TempDir::new("vendor-error");
    let stub = Stub::serving(unit_tar(COMMON_UNIT));
    let root = consumer(&dir, stub.url());
    assert_eq!(ridl(&root, dir.path(), &["vendor"]).0, 0);
    let before = std::fs::read_to_string(root.join("vendor/veh.common/common.typl")).unwrap();

    let broken = Stub::serving(unit_tar(BROKEN_UNIT));
    consumer(&dir, broken.url());
    let (code, _, stderr) = ridl(&root, dir.path(), &["vendor"]);
    assert_eq!(code, 1, "{stderr}");
    assert!(stderr.contains("TYPL-104"), "{stderr}");
    assert_eq!(std::fs::read_to_string(root.join("vendor/veh.common/common.typl")).unwrap(), before);

    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let dead = format!("http://{}/package.tar", listener.local_addr().unwrap());
    drop(listener);
    consumer(&dir, &dead);
    let (code, _, stderr) = ridl(&root, dir.path(), &["vendor"]);
    assert_eq!(code, 2, "{stderr}");
    assert!(stderr.contains("MANI-101"), "{stderr}");
    assert_eq!(std::fs::read_to_string(root.join("vendor/veh.common/common.typl")).unwrap(), before);
}

/// A missing path exits 2 and prints nothing to stdout.
#[test]
fn vendor_on_a_missing_path_exits_2() {
    let dir = TempDir::new("vendor-missing");
    let (code, stdout, stderr) = ridl(dir.path(), dir.path(), &["vendor", "nowhere"]);
    assert_eq!(code, 2, "{stderr}");
    assert_eq!(stdout, "");
}

/// In workspace mode `vendor/` is at the workspace root (V-6).
#[test]
fn vendor_writes_at_the_workspace_root() {
    let dir = TempDir::new("vendor-ws");
    let stub = Stub::serving(unit_tar(COMMON_UNIT));
    dir.write("ws/ridl.toml", "[workspace]\nmembers = [\"app\"]\n");
    dir.write("ws/app/ridl.toml", &format!("[package]\nname = \"app\"\nversion = \"1.0.0\"\n\n[imports]\n\"veh.common\" = \"{}\"\n", stub.url()));
    dir.write("ws/app/app.ridl", "package app\nimport veh.common.Speed\ntype Fast: Speed\n");
    let (code, _, stderr) = ridl(&dir.path().join("ws/app"), dir.path(), &["vendor"]);
    assert_eq!(code, 0, "{stderr}");
    assert!(dir.path().join("ws/vendor/veh.common/common.typl").is_file());
    assert!(!dir.path().join("ws/app/vendor").exists());
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p ridl-core --lib vendor::` and
`cargo test -p ridl-cli --test vendor_cli vendor_` Expected: compile error
(`write_vendor` missing); the CLI tests get clap's "unrecognized subcommand
'vendor'" with exit 2.

- [ ] **Step 3: Implement**

`crates/ridl-core/src/vendor.rs`:

```rust
//! The vendor directory (ADR-0002 §7): `<manifest root>/vendor/<unit>/`
//! holds the source tree of each remote unit the consumer imports, so a
//! `--frozen` build reads it before the cache and needs no network.
//! [`write_vendor`] brings the directory to exactly one set of units;
//! `ridl vendor --check` compares it (`check_vendor`, added with the
//! `--check` flag).
//!
//! Behind the `fetch` feature: it reads the `tree` hash.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::Path;

use crate::tree_hash::tree_hash;
use crate::workspace::{RemoteUnit, VENDOR_DIR};

#[derive(Debug, Default, PartialEq, Eq)]
pub struct VendorWrite {
    pub written: Vec<String>,
    pub removed: Vec<String>,
    pub unchanged: Vec<String>,
}

/// Brings `<root>/vendor/` to exactly `units`: each unit whose directory
/// does not already hash to its `tree` is copied, byte for byte, into a
/// staging directory beside its destination and renamed into place; the
/// directory of every unit not in `units` is removed. A hidden entry
/// (`.staging-*` of an interrupted run included) is removed first; any
/// other entry that is not a directory is left alone. A copy that fails
/// leaves the destination as it was and removes its staging directory.
pub fn write_vendor(root: &Path, units: &BTreeMap<String, RemoteUnit>) -> io::Result<VendorWrite> {
    let vendor = root.join(VENDOR_DIR);
    fs::create_dir_all(&vendor)?;
    let mut report = VendorWrite::default();
    // Stale staging and replaced-tree directories from an interrupted run.
    for entry in fs::read_dir(&vendor)? {
        let entry = entry?;
        if entry.file_name().to_string_lossy().starts_with('.') && entry.path().is_dir() {
            fs::remove_dir_all(entry.path())?;
        }
    }
    for (name, unit) in units {
        let dest = vendor.join(name);
        if dest.is_dir() && tree_hash(&dest).ok().as_deref() == Some(unit.tree.as_str()) {
            report.unchanged.push(name.clone());
            continue;
        }
        let staging = vendor.join(format!(".staging-{name}-{}", std::process::id()));
        if let Err(err) = copy_tree(&unit.dir, &staging) {
            let _ = fs::remove_dir_all(&staging);
            return Err(err);
        }
        if dest.exists() {
            let replaced = vendor.join(format!(".replaced-{name}-{}", std::process::id()));
            fs::rename(&dest, &replaced)?;
            if let Err(err) = fs::rename(&staging, &dest) {
                let _ = fs::rename(&replaced, &dest);
                let _ = fs::remove_dir_all(&staging);
                return Err(err);
            }
            fs::remove_dir_all(&replaced)?;
        } else {
            fs::rename(&staging, &dest)?;
        }
        report.written.push(name.clone());
    }
    for entry in fs::read_dir(&vendor)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if entry.path().is_dir() && !name.starts_with('.') && !units.contains_key(&name) {
            fs::remove_dir_all(entry.path())?;
            report.removed.push(name);
        }
    }
    Ok(report)
}

/// Copies every regular file under `from` to the same relative path under
/// `to`, creating directories as needed. A non-regular entry is an
/// `InvalidData` error, which cannot happen for a tree whose hash was
/// verified.
fn copy_tree(from: &Path, to: &Path) -> io::Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let kind = fs::symlink_metadata(entry.path())?.file_type();
        let target = to.join(entry.file_name());
        if kind.is_dir() {
            copy_tree(&entry.path(), &target)?;
        } else if kind.is_file() {
            fs::copy(entry.path(), target)?;
        } else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("`{}` is not a regular file", entry.path().display()),
            ));
        }
    }
    Ok(())
}
```

Register `#[cfg(feature = "fetch")] pub mod vendor;` and re-export
`vendor::{VendorWrite, write_vendor}`.

`crates/ridlc/src/lib.rs`: `compile_workspace_with_access` as in Interfaces;
`compile_workspace_with` becomes a call to it with
`RemoteAccess::new(Frozen::Yes, &Cache::user_default())`.

`crates/ridl/src/vendor.rs`:

```rust
//! `ridl vendor` — copy every remote unit into `vendor/` at the manifest
//! root, or compare the directory with `[imports]` and `ridl.lock`
//! (`--check`). Exit codes follow ADR-0010 decision 1: 0 when `vendor/`
//! was written or matches, 1 on a diagnostic error or drift, 2 when the
//! command could not answer.

use std::path::Path;
use std::process::ExitCode;

use ridl_core::diag::{Severity, render};
use ridl_core::{Cache, DiagCode, RemoteAccess, RidlDatabase, write_vendor};

pub fn run(path: &Path) -> ExitCode {
    let Some(root) = manifest_root(path) else {
        eprintln!("error: no `ridl.toml` found at or above `{}`", path.display());
        return ExitCode::from(2);
    };
    let cache = Cache::user_default();
    let mut db = RidlDatabase::default();
    // `vendor/` is what this command writes, so the load reads the cache
    // and fetches; a stale vendored tree is replaced, never compared.
    let output = match ridlc::compile_workspace_with_access(&mut db, &root, &[], RemoteAccess::refresh(&cache)) {
        Ok(output) => output,
        Err(err) => {
            eprintln!("error: {err}");
            return ExitCode::from(2);
        }
    };
    let diagnostics = output.diagnostics;
    eprint!("{}", render(&diagnostics, &output.sources));
    // A fetch failure means the tool could not answer (exit 2); any other
    // error — a lockfile, a remote unit's manifest, a source — is a
    // negative answer (exit 1). Nothing is written in either case.
    if diagnostics.iter().any(|d| d.code == DiagCode::MANI_101) {
        return ExitCode::from(2);
    }
    if diagnostics.iter().any(|d| d.severity == Severity::Error) {
        return ExitCode::from(1);
    }
    match write_vendor(&root, &output.remote_units) {
        Ok(report) => {
            for name in &report.written {
                println!("vendored {name}");
            }
            for name in &report.removed {
                println!("removed {name}");
            }
            if report.written.is_empty() && report.removed.is_empty() {
                println!("vendor/ is up to date; nothing to change");
            }
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("error: cannot write `{}`: {err}", root.join("vendor").display());
            ExitCode::from(2)
        }
    }
}

/// The manifest root of `path` (ADR-0002 §4 root discovery), or `None`.
fn manifest_root(path: &Path) -> Option<std::path::PathBuf> {
    if path.is_file() {
        path.parent().and_then(ridl_core::find_root)
    } else {
        ridl_core::find_root(path)
    }
}
```

`crates/ridl/src/main.rs`: `mod vendor;`, the `Vendor` variant from Interfaces,
and `Command::Vendor { path } => vendor::run(&path)`. `ridl vendor` applies no
`[lints]` level (ADR-0010 decision 1's lint paragraph: only `check`, `build`,
`lsp`, `mcp` do); it reports the errors the load drew.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p ridl-core --lib vendor::` and
`cargo test -p ridl-cli --test vendor_cli` Expected: the four unit tests and the
six CLI tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/ridl-core crates/ridlc/src/lib.rs crates/ridl/src crates/ridl/tests/vendor_cli.rs
git commit -m "feat(ridl)!: add ridl vendor

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

### Task 11: `ridl vendor --check [PATH]`

**Files:**

- Modify: `crates/ridl-core/src/diag.rs` (`MANI_CATALOG`, after `MANI_108`),
  `docs/specification/ridl-family-overview.md` (after MANI-108)
- Modify: `crates/ridl-core/src/vendor.rs` (`check_vendor`),
  `crates/ridl-core/src/lib.rs`
- Modify: `crates/ridl/src/vendor.rs` (`run_check`), `crates/ridl/src/main.rs`
  (the `check` flag of `Vendor` and its dispatch)
- Test: `crates/ridl-core/src/vendor.rs`, `crates/ridl/tests/vendor_cli.rs`

**Interfaces:**

- Produces: `DiagCode::MANI_109`, `MANI_110` (Error);

  ```rust
  /// Compares `<root>/vendor/` with the imports and the lock (spec 5.3):
  /// no network, no write. One detached diagnostic per unit.
  pub fn check_vendor(root: &Path, imports: &ImportUnion, lock: Option<&Lockfile>) -> Vec<Diagnostic>;
  ```

- [ ] **Step 1: Write the failing tests**

`crates/ridl-core/src/vendor.rs` tests:

```rust
    fn union(url: &str, key: &str) -> ImportUnion {
        BTreeMap::from([(url.to_string(), BTreeSet::from([key.to_string()]))])
    }

    fn lock_for(url: &str, tree: &str) -> Lockfile {
        let mut lock = Lockfile::default();
        lock.entries.insert(url.into(), LockEntry { sha256: "a".repeat(64), tree: Some(tree.into()) });
        lock
    }

    fn codes(diags: &[Diagnostic]) -> Vec<&str> {
        diags.iter().map(|d| d.code.as_str()).collect()
    }

    /// A matching directory draws nothing.
    #[test]
    fn a_matching_vendor_checks_clean() {
        let fixture = TempDir::new("check-clean");
        let root = fixture.path().join("root");
        let unit = unit(&root.join("vendor/a"), &[("a.typl", "package a\n")]);
        let diags = check_vendor(&root, &union(&unit.url, "a"), Some(&lock_for(&unit.url, &unit.tree)));
        assert!(diags.is_empty(), "{diags:?}");
    }

    /// An imported unit with no directory is MANI-109; a directory no key
    /// names is MANI-110; a changed tree is MANI-107; a unit with no lock
    /// entry is MANI-103, and one whose entry has no `tree` is MANI-108.
    #[test]
    fn drift_is_one_diagnostic_per_unit() {
        let fixture = TempDir::new("check-drift");
        let root = fixture.path().join("root");
        let a = unit(&root.join("vendor/a"), &[("a.typl", "package a\n")]);
        unit(&root.join("vendor/stray"), &[("s.typl", "package stray\n")]);
        let mut imports = union(&a.url, "a");
        imports.insert("https://r.example.com/b@v1".into(), BTreeSet::from(["b".to_string()]));
        imports.insert("https://r.example.com/c@v1".into(), BTreeSet::from(["c".to_string()]));
        unit(&root.join("vendor/c"), &[("c.typl", "package c\n")]);
        let mut lock = lock_for(&a.url, "0000");
        lock.entries.insert("https://r.example.com/b@v1".into(), LockEntry { sha256: "b".repeat(64), tree: Some("1111".into()) });
        let mut diags = check_vendor(&root, &imports, Some(&lock));
        diags.sort_by(|x, y| x.code.as_str().cmp(y.code.as_str()));
        assert_eq!(codes(&diags), ["MANI-103", "MANI-107", "MANI-109", "MANI-110"]);
        assert!(diags[0].message.contains("c@v1"));
        assert!(diags[1].message.contains("vendor/a") || diags[1].message.contains("vendor\\a"));
        assert!(diags[2].message.contains("b@v1"));
        assert!(diags[3].message.contains("stray"));
    }

    /// A file rewritten with CRLF line ends is drift (Review Focus 4).
    #[test]
    fn a_crlf_rewrite_is_drift() {
        let fixture = TempDir::new("check-crlf");
        let root = fixture.path().join("root");
        let unit = unit(&root.join("vendor/a"), &[("a.typl", "package a\n")]);
        fs::write(root.join("vendor/a/a.typl"), "package a\r\n").unwrap();
        let diags = check_vendor(&root, &union(&unit.url, "a"), Some(&lock_for(&unit.url, &unit.tree)));
        assert_eq!(codes(&diags), ["MANI-107"]);
    }
```

Append to `crates/ridl/tests/vendor_cli.rs`:

```rust
/// `ridl vendor --check` exits 0 on a match, 1 on drift of any kind, and
/// never fetches or writes.
#[test]
fn vendor_check_reports_drift_without_network_or_writes() {
    let dir = TempDir::new("vendor-check");
    let stub = Stub::serving(unit_tar(COMMON_UNIT));
    let root = consumer(&dir, stub.url());
    assert_eq!(ridl(&root, dir.path(), &["vendor"]).0, 0);
    stub.reset_hits();
    let (code, stdout, stderr) = ridl(&root, dir.path(), &["vendor", "--check"]);
    assert_eq!(code, 0, "{stderr}");
    assert_eq!(stdout, "");

    std::fs::write(root.join("vendor/veh.common/common.typl"), COMMON_UNIT[1].1.replace('\n', "\r\n")).unwrap();
    let (code, _, stderr) = ridl(&root, dir.path(), &["vendor", "--check"]);
    assert_eq!(code, 1, "{stderr}");
    assert!(stderr.contains("MANI-107"), "{stderr}");

    std::fs::create_dir_all(root.join("vendor/stray")).unwrap();
    std::fs::remove_dir_all(root.join("vendor/veh.common")).unwrap();
    let (code, _, stderr) = ridl(&root, dir.path(), &["vendor", "--check"]);
    assert_eq!(code, 1, "{stderr}");
    assert!(stderr.contains("MANI-109") && stderr.contains("MANI-110"), "{stderr}");
    assert!(root.join("vendor/stray").is_dir(), "--check writes nothing");
    assert_eq!(stub.hits(), 0, "--check never fetches");

    let (code, _, _) = ridl(dir.path(), dir.path(), &["vendor", "--check", "nowhere"]);
    assert_eq!(code, 2);
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p ridl-core --lib vendor:: diag::` and
`cargo test -p ridl-cli --test vendor_cli vendor_check` Expected: compile errors
on `check_vendor`, `MANI_109`, `MANI_110`; the CLI test gets clap's "unexpected
argument '--check'" with exit 2.

- [ ] **Step 3: Implement**

`MANI_CATALOG` after `MANI_108`:

```rust
        /// `ridl vendor --check`: an imported unit has no directory under
        /// `<manifest root>/vendor/` (ADR-0002 §7).
        MANI_109 = "MANI-109", Error,
            "an imported unit has no directory in `vendor/`";

        /// `ridl vendor --check`: `vendor/` holds a directory that no
        /// `[imports]` key names (ADR-0002 §7).
        MANI_110 = "MANI-110", Error,
            "`vendor/` holds a directory that no `[imports]` key names";
```

Overview rows:

```markdown
| MANI-109 | an imported unit has no directory in `vendor/` | error |
| MANI-110 | `vendor/` holds a directory that no `[imports]` key names | error |
```

`crates/ridl-core/src/vendor.rs`:

```rust
pub fn check_vendor(root: &Path, imports: &ImportUnion, lock: Option<&Lockfile>) -> Vec<Diagnostic> {
    let vendor = root.join(VENDOR_DIR);
    let mut diagnostics = Vec::new();
    let mut named: BTreeSet<String> = BTreeSet::new();
    for (url, keys) in imports {
        let Some(dir) = keys.iter().map(|key| vendor.join(key)).find(|dir| dir.is_dir()) else {
            diagnostics.push(detached(
                DiagCode::MANI_109,
                format!(
                    "the imported unit `{}` (`{url}`) has no directory in `{}`; run `ridl vendor`",
                    keys.iter().cloned().collect::<Vec<_>>().join("`, `"),
                    vendor.display()
                ),
            ));
            continue;
        };
        named.insert(dir.file_name().unwrap().to_string_lossy().into_owned());
        match lock.and_then(|lock| lock.entries.get(url)) {
            None => diagnostics.push(detached(
                DiagCode::MANI_103,
                format!("no `ridl.lock` entry for `{url}`; run `ridl vendor` to pin and vendor it"),
            )),
            Some(LockEntry { tree: None, .. }) => diagnostics.push(detached(
                DiagCode::MANI_108,
                format!("the `ridl.lock` entry for `{url}` has no `tree` hash; run `ridl vendor` to add it"),
            )),
            Some(LockEntry { tree: Some(expected), .. }) => match tree_hash(&dir) {
                Ok(actual) if actual == *expected => {}
                Ok(_) => diagnostics.push(detached(
                    DiagCode::MANI_107,
                    format!(
                        "the vendored tree `{}` does not match the `tree` hash `ridl.lock` pins for `{url}`; run `ridl vendor` to rewrite it",
                        dir.display()
                    ),
                )),
                Err(err) => diagnostics.push(detached(DiagCode::MANI_107, format!("cannot hash `{}`: {err}", dir.display()))),
            },
        }
    }
    if let Ok(entries) = fs::read_dir(&vendor) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if entry.path().is_dir() && !name.starts_with('.') && !named.contains(&name) {
                diagnostics.push(detached(
                    DiagCode::MANI_110,
                    format!("`{}` is in `vendor/` but no `[imports]` key names `{name}`; run `ridl vendor` to remove it", entry.path().display()),
                ));
            }
        }
    }
    diagnostics
}
```

`detached` is the builder of `fetch.rs`; make it `pub(crate)` there and import
it. A missing `vendor/` directory draws MANI-109 for every import and nothing
else.

`crates/ridl/src/main.rs`: the `Vendor` variant gains

```rust
/// Compare `vendor/` with `[imports]` and `ridl.lock` instead of
/// writing: no network, no write. Exit 0 when `vendor/` matches, 1 on
/// drift (MANI-107, MANI-109, MANI-110), 2 on a missing or unreadable
/// path.
#[arg(long)]
check: bool,
```

and the dispatch becomes
`Command::Vendor { path, check } => if check { vendor::run_check(&path) } else { vendor::run(&path) }`;
the variant's doc comment gains "with `--check`, compare `vendor/` with the
imports and `ridl.lock` instead, writing nothing".

`crates/ridl/src/vendor.rs` `run_check`:

```rust
pub fn run_check(path: &Path) -> ExitCode {
    let Some(root) = manifest_root(path) else {
        eprintln!("error: no `ridl.toml` found at or above `{}`", path.display());
        return ExitCode::from(2);
    };
    let mut db = RidlDatabase::default();
    let loaded = match ridl_core::load_workspace(&mut db, &root) {
        Ok(loaded) => loaded,
        Err(err) => {
            eprintln!("error: {err}");
            return ExitCode::from(2);
        }
    };
    let (lock, mut diagnostics) = ridl_core::read_lockfile(&root.join("ridl.lock"));
    diagnostics.extend(ridl_core::check_vendor(&root, &loaded.imports, lock.as_ref()));
    eprint!("{}", render(&diagnostics, &loaded.sources));
    if diagnostics.iter().any(|d| d.severity == Severity::Error) {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}
```

The plain `load_workspace` reads no remote unit and never fetches; its own
MANI-0xx findings (a bad manifest) are rendered and count as errors, so a
`--check` over a broken manifest exits 1 rather than reporting a clean
`vendor/`.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p ridl-core`, `cargo test -p ridl-cli --test vendor_cli`,
`cargo xtask freeze update` Expected: pass; `freeze/diagnostics.txt` gains
`MANI-109 error` and `MANI-110 error`.

- [ ] **Step 5: Commit**

```bash
git add crates/ridl-core crates/ridl/src/vendor.rs crates/ridl/tests/vendor_cli.rs docs/specification/ridl-family-overview.md freeze/diagnostics.txt
git commit -m "feat(ridl)!: add ridl vendor --check

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

### Task 12: Records of PR 2 and the gate

**Files:**

- Modify: `docs/decisions/ADR-0002-module-system.md` — Status; §4 "Rationale —
  `[imports]` aliases logical package names" (`:318-324`); §7 (after the `tree`
  paragraph of Task 7); Consequences, Positive, "Air-gap is a manifest rewrite"
  (`:421`); Open questions, "Vendor directory" (`:464-467`)
- Modify: `docs/decisions/ADR-0010-cli-conventions.md:111-131` (decision 1's
  table: a `ridl vendor` row) and the dated prose block after the `ridl init` /
  `ridl new` block (`:241-262`)
- Modify: `docs/decisions/ADR-0029-stability-policy.md:94` (the Manifest row:
  add `vendor/`)
- Modify: `xtask/src/freeze/manifest.rs` (one `vendor` line);
  `freeze/manifest.txt`, `freeze/cli.txt` (regenerated)
- Modify: `docs/book/cli-reference.md` — a `### ridl vendor` section after
  `### ridl describe` (`:2026-2124`), the `ridl check` paragraph of Task 7
  (`vendor/` read first), the exit-code table (`:2311-2322`)
- Modify: `docs/book/getting-started.md` (the `[imports]` paragraph of Task 7:
  one sentence on `ridl vendor`)

- [ ] **Step 1: Write the failing tests**

`xtask/src/freeze/manifest.rs` test: add
`"vendor <manifest root>/vendor/<unit>/"` to the expected lines. The book's CLI
reference tests (`crates/ridl/tests/cli_reference.rs`:
`every_subcommand_has_a_transcript`,
`the_exit_code_table_lists_the_subcommands`,
`help_transcripts_equal_the_binary`) already fail once `ridl vendor` exists
without a transcript and a table row — run them first to see the three failures.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p xtask manifest_dump` and
`cargo test -p ridl-cli --test cli_reference` Expected: the dump test misses the
`vendor` line; `every_subcommand_has_a_transcript` names `vendor`; the exit
table test names `vendor`.

- [ ] **Step 3: Write the records**

`xtask/src/freeze/manifest.rs`: push
`"vendor <manifest root>/vendor/<unit>/".to_string()` after the lock lines, with
the doc "and the layout of the vendor directory". Run
`cargo xtask freeze update`: `freeze/manifest.txt` gains the line and
`freeze/cli.txt` gains the `ridl vendor` lines (`ridl vendor --check optional`,
`ridl vendor [PATH] optional`, `ridl vendor requires` and the rest clap prints);
no line is removed.

`docs/book/cli-reference.md`:

- `### ridl vendor` with the `ridl vendor --help` transcript in a `text` fence
  (copy the binary's output exactly; the test compares it), then prose: `PATH`
  and root discovery; the three steps (the union of the root's and every
  member's `[imports]`; each unit materialized as `ridl check` does, from the
  cache or a fetch, with `ridl.lock` rewritten; `vendor/` brought to exactly
  that set, each unit written whole then renamed into place, a unit no longer
  imported removed, nothing written on an error); `--check` (no network, no
  write; MANI-109, MANI-110, MANI-107, and MANI-103 or MANI-108 for a unit the
  lock does not pin); the exit codes of spec 5.2 and 5.3 as two short tables;
  and the sentence that `ridl vendor` reads the cache, never `vendor/`, so a URL
  change is fetched and the directory replaced.
- The `ridl check` paragraph: the read order `vendor/<key>/`, then the cache,
  then (not frozen) a fetch; a stale vendored tree is MANI-107 naming
  `ridl vendor`.
- Exit-code table row:

  ```markdown
  | `ridl vendor` | `vendor/` was written, or there was nothing to change; with `--check`, `vendor/` matches | a diagnostic error (MANI-102 to MANI-108, or a source error in a remote unit), nothing written; with `--check`, drift: MANI-107, MANI-109 or MANI-110 | a missing or unreadable path, a bad flag, an I/O failure, or a fetch failure (MANI-101) |
  ```

`docs/book/getting-started.md`, at the end of the `[imports]` paragraph:
"`ridl vendor` copies each unit into `vendor/` beside `ridl.toml`, so a
`ridl check --frozen` on a machine with no network and no cache still builds."

`docs/decisions/ADR-0002-module-system.md`:

- Status: "Amended 2026-10-12 by the remote units design (driftsys/ridl#48, PR
  2): §4 and the Positive consequence say that an air-gapped build is
  `ridl vendor`; §7 gains `vendor/`, its read order and `ridl vendor --check`,
  with MANI-017, MANI-018, MANI-109 and MANI-110; the vendor directory open
  question is resolved."
- §4 rationale, replace "This is also what makes air-gapping work as a pure
  manifest rewrite." with "An air-gapped build is `ridl vendor` (§7); rewriting
  the manifest to point at a mirror is still possible, because only the manifest
  changes."
- §7, after the `tree` paragraph:

  ```markdown
  **The vendor directory** (amended 2026-10-12, driftsys/ridl#48).
  `<manifest root>/vendor/` — beside `ridl.toml` and `ridl.lock`; the
  workspace root in workspace mode — holds the source tree of each remote
  unit under its `[imports]` key, written by `ridl vendor`. For each URL of
  the consumer's `[imports]` the loader takes the first of: `vendor/<key>/`
  when that directory exists and `ridl.lock` pins a `tree` for the URL (the
  tree must match, MANI-107); the cache entry for the pin; outside `--frozen`,
  a fetch. A vendored directory with no usable pin is read only after the
  fetch has pinned the URL, and is verified against that pin. The name
  `vendor` is reserved at the manifest root only: a `members` entry that is
  `vendor` or a path under it is MANI-017 (Error), a source file under a
  standalone root's `vendor/` is MANI-018 (Warning, `vendor-source-not-loaded`)
  and is not loaded, and a deeper directory named `vendor` is an ordinary
  source package. `ridl vendor [PATH]` brings the directory to exactly the
  imported set — each unit written whole into a staging directory and renamed
  into place, a unit no longer imported removed, nothing written when the run
  has an error — and reads the cache, never `vendor/`, so a changed URL
  replaces the directory. `ridl vendor --check [PATH]` compares the directory
  with the imports and the lock without network or writes: MANI-109 (Error)
  for an imported unit with no directory, MANI-110 (Error) for a directory no
  key names, MANI-107 for a tree that does not match. `ridl.lock` and
  `vendor/` are part of the frozen manifest surface from 0.10.0
  ([ADR-0029](ADR-0029-stability-policy.md)).

  Alternatives considered: a vendor directory holding the tar files, or both
  tars and trees (a review would show a binary file, and two copies can
  drift); replacing `sha256` with the tree hash, or a second pin file under
  `vendor/` (the first loses the verification of a download before it is
  unpacked, the second can disagree with `ridl.lock`); a hidden
  `.ridl/vendor/` (no loader change, but not the convention users expect); a
  `vendor` reserved at every depth, or detected by content (the first takes a
  package segment from every unit, the second makes a directory's meaning
  depend on its contents); a `--frozen` mode for `ridl vendor` (no stated
  need), and no `--check` (CI would have no way to check `vendor/` without a
  full compile); and leaving `ridl.lock` and `vendor/` unfrozen (a 0.10.x
  release could then break a repository that vendored under 0.10.0).
  ```

- Positive consequence: "Air-gap is `ridl vendor`: the vendored trees are
  verified against `ridl.lock` and read before the cache. Rewriting the manifest
  to point at a mirror stays possible."
- Open questions: mark the vendor item "**Resolved 2026-10-12** (§7,
  driftsys/ridl#48)" and keep its text.

`docs/decisions/ADR-0010-cli-conventions.md`: a row in decision 1's table,

```markdown
| `ridl vendor`            | `vendor/` was written, or there was nothing to change; with `--check`, `vendor/` matches | a diagnostic error (MANI-102 to MANI-108, or a source error in a remote unit), nothing written; with `--check`, drift (MANI-107, MANI-109, MANI-110) | a missing or unreadable path, a bad flag, an I/O failure, or a fetch failure (MANI-101) (added 2026-10-12) |
```

and after the `ridl init` / `ridl new` block:

```markdown
**`ridl vendor` earned its row on 2026-10-12, when it was added and
checked** by the tests in `crates/ridl/tests/vendor_cli.rs` against the
built `ridl` binary. The 0 cell is `vendor_writes_the_units_and_is_idempotent`
(written, then nothing to change) and the first half of
`vendor_check_reports_drift_without_network_or_writes`. The 1 cell is
`vendor_writes_nothing_on_an_error` (a source error in the remote unit)
and the drift halves of the `--check` test (MANI-107, MANI-109,
MANI-110). The 2 cell is `vendor_on_a_missing_path_exits_2`, the last
assertion of the `--check` test (a missing path under `--check`), and
the fetch-failure half of `vendor_writes_nothing_on_an_error` (MANI-101).
```

Status note: "Amended 2026-10-12: decision 1 gained a row for `ridl vendor`."

`docs/decisions/ADR-0029-stability-policy.md:94`: the Manifest row's cell adds
`vendor/` and its layout after `ridl.lock`, and the dated note gains "and
`vendor/`".

- [ ] **Step 4: Run the gate**

Run: `cargo test -p xtask`, `cargo test -p ridl-cli --test cli_reference`, then
`just build`. Expected: green, including `just freeze-check` with the
regenerated `freeze/cli.txt`, `freeze/manifest.txt` and
`freeze/diagnostics.txt`.

- [ ] **Step 5: Commit, then open PR 2**

```bash
git add docs/decisions xtask/src/freeze/manifest.rs freeze docs/book
git commit -m "docs(adr): record vendor/ and ridl vendor

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

PR 2 body: the breaking change (a `members` entry of `vendor` is now an error; a
source under a standalone root's `vendor/` is no longer loaded) as a delegated
ruling; "Closes driftsys/ridl#48"; the heads-up issue on
driftsys/ridlc-gen-kotlin ("a consumer may now hold `vendor/`; a plugin that
walks the manifest root must skip it").

### Task 13: Gardening hand-off

After PR 2 merges, run the `sdd-gardening` skill: the spec and this plan move to
`docs/archive/`, and the durable records are those Tasks 7 and 12 wrote. No task
of this plan writes under `docs/design/` or `docs/technotes/`; if the gardener
finds that `docs/technotes/walking-skeleton-architecture.md` still says a remote
import resolves nothing, that sentence is fixed in PR 1 (Task 7's last file).

---

## Open questions raised while reading the code

Each is answered in the task that meets it; the answer is a plan decision, not a
spec change, and is listed here so the maintainer can overrule it.

1. **A vendored directory with no usable pin** (spec 4.1 says `vendor/` is read
   first). Task 9 reads `vendor/<key>/` first only when `ridl.lock` pins a
   `tree` for its URL; otherwise the cache-or-fetch step runs first and the
   vendored tree is verified against the new pin. A directory the lock cannot
   verify is never compiled.
2. **`ridl vendor` and the read order** (spec 5.2 step 3, "as PR 1 does"). Task
   10 gives `ridl vendor` `RemoteAccess::refresh`, which skips `vendor/`;
   otherwise a changed URL would be MANI-107 against the old vendored tree and
   the command could never update a dependency.
3. **What a remote package is kept out of** (spec 4.4 names codegen, catalogs
   and `ridl fmt`). Tasks 5 and 6 also keep it out of IR dumps, `ridl baseline`,
   the compatible-catalog chain, `ridl lock`, `ridl test` and a `ridl diff`
   source side, because each of those publishes or edits the consumer's own
   contract. The code backends receive it (spec 4.4).
4. **Two keys for one URL** (V-2 says the name equals the key). Task 3 loads the
   unit once under its name and raises MANI-105 for the other key, which binds
   nothing.
5. **One key pinned to two URLs by two members** (ADR-0002 §5 lets a member pin
   its own version). Under V-1 both units load and the second is MANI-014. This
   is a behaviour change the ADR amendment of Task 7 states.
6. **MANI-103 in the servers** (spec 4.1 names MANI-104 for a miss). A URL with
   no lock entry is MANI-103 there, as under `--frozen`; a pinned URL not in the
   cache is MANI-104. Both messages name `ridl check` (and, from PR 2,
   `ridl vendor`).
7. **Diagnostics the language server shows for a remote file.** Task 6 publishes
   none for a file of a remote unit and shows the detached MANI-1xx errors once
   as a window message, because `convert_loader_diagnostics` drops a span with
   no path.
8. **MANI-017 scope** (spec 5.1 says "a `members` entry is `vendor`"). Task 8
   also raises it for a path under `vendor/`, so a member can never sit inside
   the reserved directory.
9. **`ridl fmt` given a path under `vendor/`.** Task 8 refuses it with exit 2; a
   walk from the root skips the directory silently.
10. **Which errors stop `ridl vendor`** (spec 5.2 lists MANI-102 to MANI-108 and
    a remote source error). Task 10 stops on any error diagnostic, consumer
    errors included, so a manifest that fails to parse (and therefore names no
    import) cannot empty `vendor/`.
