//! The `tree` hash of a remote unit's directory (ADR-0002 §7): one SHA-256
//! over every regular file under the directory, so that `ridl.lock` pins the
//! unpacked tree beside the fetched tar.
//!
//! This module sits behind the `fetch` feature: it hashes with `sha2`, which
//! only the fetch feature pulls in.

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
fn collect(
    root: &Path,
    dir: &Path,
    files: &mut Vec<(Vec<u8>, PathBuf)>,
) -> Result<(), TreeHashError> {
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
    /// is checked against the definition and not against itself. The order
    /// is that of the whole path's bytes, not of its components: `-` (0x2d)
    /// sorts before `.` (0x2e), and `.` before `/` (0x2f), so `a-b/x.typl`
    /// comes before `a.typl`, and `a.typl` before `a/b.typl`.
    #[test]
    fn the_hash_follows_the_definition() {
        use sha2::{Digest, Sha256};
        let dir = TempDir::new("definition");
        dir.write("ridl.toml", b"[package]\nname = \"r.u\"\n");
        dir.write("sub/b.typl", b"package r.u.sub\n");
        dir.write("a/b.typl", b"package r.u.a\n");
        dir.write("a.typl", b"package r.u\n");
        dir.write("a-b/x.typl", b"package r.u.a_b\n");

        let mut hasher = Sha256::new();
        for (path, bytes) in [
            ("a-b/x.typl", &b"package r.u.a_b\n"[..]),
            ("a.typl", &b"package r.u\n"[..]),
            ("a/b.typl", &b"package r.u.a\n"[..]),
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
        std::os::unix::fs::symlink(dir.path().join("a.typl"), dir.path().join("sub-link")).unwrap();
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
