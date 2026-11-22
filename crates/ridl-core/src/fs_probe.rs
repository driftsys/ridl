//! File-type probes that report a failure instead of hiding it.
//!
//! `Path::is_file`, `Path::is_dir` and `Path::exists` return `false` for every
//! metadata error, so a permission failure reads as "not there"
//! (driftsys/ridl#1320). The functions here return `Ok(false)` only for an
//! entry that is absent — `NotFound`, or `NotADirectory` for a path below a
//! regular file — and return any other error to the caller, which reports it
//! with the path and the cause (ADR-0010 decision 6). Like the `Path`
//! methods, they follow symbolic links.

use std::io;
use std::path::Path;

/// The metadata of `path`, or `None` when the entry is absent.
fn probe(path: &Path) -> io::Result<Option<std::fs::Metadata>> {
    match std::fs::metadata(path) {
        Ok(metadata) => Ok(Some(metadata)),
        Err(error)
            if matches!(
                error.kind(),
                io::ErrorKind::NotFound | io::ErrorKind::NotADirectory
            ) =>
        {
            Ok(None)
        }
        Err(error) => Err(error),
    }
}

/// Whether `path` is a regular file. An absent entry and an entry of another
/// kind give `Ok(false)`; any other error is returned.
pub fn is_file(path: &Path) -> io::Result<bool> {
    Ok(probe(path)?.is_some_and(|metadata| metadata.is_file()))
}

/// Whether `path` is a directory. An absent entry and an entry of another
/// kind give `Ok(false)`; any other error is returned.
pub fn is_dir(path: &Path) -> io::Result<bool> {
    Ok(probe(path)?.is_some_and(|metadata| metadata.is_dir()))
}

/// Whether an entry exists at `path`. An absent entry gives `Ok(false)`; any
/// other error is returned.
pub fn exists(path: &Path) -> io::Result<bool> {
    Ok(probe(path)?.is_some())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("ridl-fs-probe-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn a_file_a_directory_and_an_absent_entry_are_told_apart() {
        let dir = temp("kinds");
        let file = dir.join("f");
        std::fs::write(&file, "").unwrap();
        assert!(is_file(&file).unwrap());
        assert!(!is_dir(&file).unwrap());
        assert!(exists(&file).unwrap());
        assert!(is_dir(&dir).unwrap());
        assert!(!is_file(&dir).unwrap());
        let absent = dir.join("absent");
        assert!(!exists(&absent).unwrap());
        assert!(!is_file(&absent).unwrap());
        assert!(!is_dir(&absent).unwrap());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A directory named `ridl.toml` is present but is not a file.
    #[test]
    fn a_directory_named_like_a_manifest_is_not_a_file() {
        let dir = temp("manifest-dir");
        std::fs::create_dir(dir.join("ridl.toml")).unwrap();
        assert!(!is_file(&dir.join("ridl.toml")).unwrap());
        assert!(exists(&dir.join("ridl.toml")).unwrap());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn a_path_below_a_regular_file_is_absent() {
        let dir = temp("below-file");
        let file = dir.join("f");
        std::fs::write(&file, "").unwrap();
        assert!(!exists(&file.join("x")).unwrap());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn a_symbolic_link_loop_is_an_error() {
        let dir = temp("loop");
        std::os::unix::fs::symlink("l", dir.join("l")).unwrap();
        assert!(exists(&dir.join("l")).is_err());
        assert!(is_file(&dir.join("l")).is_err());
        assert!(is_dir(&dir.join("l")).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn an_entry_in_an_unsearchable_directory_is_a_permission_error() {
        use std::os::unix::fs::PermissionsExt;
        let dir = temp("unsearchable");
        let locked = dir.join("locked");
        std::fs::create_dir(&locked).unwrap();
        std::fs::write(locked.join("f"), "").unwrap();
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o000)).unwrap();
        // `chmod 000` does not bind root; detect that by trying to read.
        if std::fs::read_dir(&locked).is_err() {
            let error = exists(&locked.join("f")).unwrap_err();
            assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
            assert!(is_file(&locked.join("f")).is_err());
            assert!(is_dir(&locked.join("f")).is_err());
        }
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o755)).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }
}
