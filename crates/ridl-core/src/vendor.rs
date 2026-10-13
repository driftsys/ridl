//! The vendor directory (ADR-0002 §7): `<manifest root>/vendor/<key>/` holds
//! the source tree of each remote unit the consumer imports, so a `--frozen`
//! build reads it before the cache and needs no network. [`write_vendor`]
//! brings the directory to exactly one set of units.
//!
//! This module sits behind the `fetch` feature: it reads the `tree` hash.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::Path;

use crate::tree_hash::tree_hash;
use crate::workspace::{MaterializedUnit, VENDOR_DIR};

/// What [`write_vendor`] changed, each list in key order: the units it
/// copied, the unit directories it removed, and the units whose directory
/// already matched.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct VendorWrite {
    pub written: Vec<String>,
    pub removed: Vec<String>,
    pub unchanged: Vec<String>,
}

/// The prefix of the directory a unit is copied into before it is renamed
/// to `vendor/<key>/`.
const STAGING_PREFIX: &str = ".staging-";

/// The prefix of the name an old `vendor/<key>/` is renamed to while the new
/// copy is renamed into its place.
const REPLACED_PREFIX: &str = ".replaced-";

/// Brings `<root>/vendor/` to exactly `units`, keyed by `[imports]` key.
///
/// - A `.staging-*` or `.replaced-*` directory, which only an interrupted
///   run leaves, is removed first. No other hidden entry is touched.
/// - A unit whose `vendor/<key>/` already hashes to its `tree` is left as it
///   is.
/// - Any other unit is copied byte for byte from its `dir` into
///   `vendor/.staging-<key>-<pid>/`, and the copy is checked against the
///   unit's `tree`. Then the old `vendor/<key>/`, if any, is renamed to
///   `vendor/.replaced-<key>-<pid>`, the copy is renamed to
///   `vendor/<key>/`, and the old entry is removed. A regular file at
///   `vendor/<key>` is replaced the same way as an old tree.
/// - The directory of every key not in `units` is removed. An entry that is
///   not a directory, and a hidden directory, is left alone.
///
/// So `vendor/<key>/` never holds a partial copy: it holds the old tree or
/// the new one, and it is absent only between the two renames. A run that
/// stops there leaves the old tree in `.replaced-*`, and the next run
/// removes that directory and writes `vendor/<key>/` again. A copy that fails,
/// or that does not match its `tree`, is an error that leaves
/// `vendor/<key>/` as it was and removes the staging directory.
///
/// With no unit and no `vendor/`, nothing is created.
pub fn write_vendor(
    root: &Path,
    units: &BTreeMap<String, MaterializedUnit>,
) -> io::Result<VendorWrite> {
    let vendor = root.join(VENDOR_DIR);
    let mut report = VendorWrite::default();
    if units.is_empty() && !vendor.exists() {
        return Ok(report);
    }
    fs::create_dir_all(&vendor)?;
    for entry in fs::read_dir(&vendor)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let leftover = name.starts_with(STAGING_PREFIX) || name.starts_with(REPLACED_PREFIX);
        if leftover && entry.file_type()?.is_dir() {
            fs::remove_dir_all(entry.path())?;
        }
    }
    let pid = std::process::id();
    for (key, unit) in units {
        let dest = vendor.join(key);
        if dest.is_dir() && tree_hash(&dest).is_ok_and(|tree| tree == unit.tree) {
            report.unchanged.push(key.clone());
            continue;
        }
        let staging = vendor.join(format!("{STAGING_PREFIX}{key}-{pid}"));
        if let Err(err) = copy_checked(unit, &staging) {
            let _ = fs::remove_dir_all(&staging);
            return Err(err);
        }
        if dest.exists() {
            let replaced = vendor.join(format!("{REPLACED_PREFIX}{key}-{pid}"));
            fs::rename(&dest, &replaced)?;
            if let Err(err) = fs::rename(&staging, &dest) {
                let _ = fs::rename(&replaced, &dest);
                let _ = fs::remove_dir_all(&staging);
                return Err(err);
            }
            // A regular file at the key path is replaced like an old tree.
            if replaced.is_dir() {
                fs::remove_dir_all(&replaced)?;
            } else {
                fs::remove_file(&replaced)?;
            }
        } else {
            fs::rename(&staging, &dest)?;
        }
        report.written.push(key.clone());
    }
    let mut removed = Vec::new();
    for entry in fs::read_dir(&vendor)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if entry.file_type()?.is_dir() && !name.starts_with('.') && !units.contains_key(&name) {
            fs::remove_dir_all(entry.path())?;
            removed.push(name);
        }
    }
    removed.sort();
    report.removed = removed;
    Ok(report)
}

/// Copies the tree of `unit` to `to`, then checks the copy against the
/// unit's `tree` hash. A mismatch, which a source changed after it was
/// verified would give, is an [`io::ErrorKind::InvalidData`] error naming
/// the unit's URL.
fn copy_checked(unit: &MaterializedUnit, to: &Path) -> io::Result<()> {
    copy_tree(&unit.dir, to)?;
    match tree_hash(to) {
        Ok(tree) if tree == unit.tree => Ok(()),
        Ok(_) => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "the copy of `{}` does not match the unit's `tree` hash",
                unit.url
            ),
        )),
        Err(err) => Err(io::Error::new(io::ErrorKind::InvalidData, err.to_string())),
    }
}

/// Copies every regular file under `from` to the same relative path under
/// `to`, creating directories as needed, with the bytes unchanged. A
/// symbolic link or any other non-regular entry is an
/// [`io::ErrorKind::InvalidData`] error; a verified unit holds none.
fn copy_tree(from: &Path, to: &Path) -> io::Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let kind = entry.file_type()?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// A unique directory under the system temp dir, removed on drop.
    struct TempDir(PathBuf);

    impl TempDir {
        fn new(label: &str) -> Self {
            static COUNTER: AtomicUsize = AtomicUsize::new(0);
            let mut path = std::env::temp_dir();
            path.push(format!(
                "ridl-core-vendor-{label}-{}-{}",
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

    /// Writes `files` under `dir` and returns the unit that reads them, with
    /// the `tree` hash of what was written.
    fn unit(dir: &Path, files: &[(&str, &str)]) -> MaterializedUnit {
        for (name, text) in files {
            let path = dir.join(name);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, text).unwrap();
        }
        MaterializedUnit {
            url: "https://r.example.com/u@v1".into(),
            dir: dir.to_path_buf(),
            tree: tree_hash(dir).unwrap(),
        }
    }

    /// The names of the hidden entries directly under `dir`.
    fn hidden_entries(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|name| name.starts_with('.'))
            .collect();
        names.sort();
        names
    }

    fn root_in(fixture: &TempDir) -> PathBuf {
        let root = fixture.path().join("root");
        fs::create_dir_all(&root).unwrap();
        root
    }

    /// The first write copies each unit byte for byte under its key, the
    /// unit's own `vendor/` included, since the `tree` hash covers it.
    #[test]
    fn writes_each_unit_under_its_name() {
        let fixture = TempDir::new("write");
        let source = unit(
            &fixture.path().join("src/a"),
            &[
                ("ridl.toml", "[package]\nname = \"a\"\n"),
                ("sub/s.typl", "package a.sub\r\n"),
                ("vendor/other/o.typl", "package other\n"),
            ],
        );
        let root = root_in(&fixture);
        let report =
            write_vendor(&root, &BTreeMap::from([("a".to_string(), source.clone())])).unwrap();
        assert_eq!(report.written, ["a"]);
        assert!(report.removed.is_empty() && report.unchanged.is_empty());
        assert_eq!(tree_hash(&root.join("vendor/a")).unwrap(), source.tree);
        assert_eq!(
            fs::read(root.join("vendor/a/sub/s.typl")).unwrap(),
            b"package a.sub\r\n"
        );
        assert!(root.join("vendor/a/vendor/other/o.typl").is_file());
        assert!(hidden_entries(&root.join("vendor")).is_empty());
    }

    /// A second write with the same set changes nothing and reports it.
    #[test]
    fn an_unchanged_unit_is_left_alone() {
        let fixture = TempDir::new("unchanged");
        let source = unit(&fixture.path().join("src/a"), &[("a.typl", "package a\n")]);
        let root = root_in(&fixture);
        let units = BTreeMap::from([("a".to_string(), source)]);
        write_vendor(&root, &units).unwrap();
        // An empty directory the source does not hold: the tree hash does
        // not see it, and a rewrite would remove it.
        fs::create_dir(root.join("vendor/a/marker")).unwrap();
        let report = write_vendor(&root, &units).unwrap();
        assert_eq!(report.unchanged, ["a"]);
        assert!(report.written.is_empty() && report.removed.is_empty());
        assert!(
            root.join("vendor/a/marker").is_dir(),
            "the tree was rewritten"
        );
    }

    /// A unit no longer imported is removed; a hidden directory and a file
    /// are left alone.
    #[test]
    fn a_unit_no_longer_imported_is_removed() {
        let fixture = TempDir::new("remove");
        let a = unit(&fixture.path().join("src/a"), &[("a.typl", "package a\n")]);
        let b = unit(&fixture.path().join("src/b"), &[("b.typl", "package b\n")]);
        let root = root_in(&fixture);
        write_vendor(
            &root,
            &BTreeMap::from([("a".to_string(), a.clone()), ("b".to_string(), b)]),
        )
        .unwrap();
        fs::write(root.join("vendor/README.md"), "kept").unwrap();
        fs::create_dir_all(root.join("vendor/.keep")).unwrap();
        fs::write(root.join("vendor/.keep/k.typl"), "kept").unwrap();
        let report = write_vendor(&root, &BTreeMap::from([("a".to_string(), a)])).unwrap();
        assert_eq!(report.removed, ["b"]);
        assert_eq!(report.unchanged, ["a"]);
        assert!(!root.join("vendor/b").exists());
        assert!(root.join("vendor/README.md").is_file());
        assert!(root.join("vendor/.keep/k.typl").is_file());
    }

    /// The staging and replaced-tree directories an interrupted run leaves
    /// are removed first, and a changed unit is swapped in whole. Another
    /// hidden directory is left alone.
    #[test]
    fn a_stale_staging_directory_is_cleared_and_the_swap_is_whole() {
        let fixture = TempDir::new("staging");
        let old = unit(
            &fixture.path().join("src/old"),
            &[("a.typl", "package a\n"), ("gone.typl", "package a\n")],
        );
        let root = root_in(&fixture);
        write_vendor(&root, &BTreeMap::from([("a".to_string(), old)])).unwrap();
        for stale in [".staging-a-999", ".replaced-a-999"] {
            fs::create_dir_all(root.join("vendor").join(stale)).unwrap();
            fs::write(root.join("vendor").join(stale).join("partial.typl"), "x").unwrap();
        }
        fs::create_dir_all(root.join("vendor/.keep")).unwrap();
        let new = unit(
            &fixture.path().join("src/new"),
            &[("a.typl", "package a\ntype T: m\n")],
        );
        let report =
            write_vendor(&root, &BTreeMap::from([("a".to_string(), new.clone())])).unwrap();
        assert_eq!(report.written, ["a"]);
        assert_eq!(tree_hash(&root.join("vendor/a")).unwrap(), new.tree);
        assert!(
            !root.join("vendor/a/gone.typl").exists(),
            "the old tree is replaced whole"
        );
        assert_eq!(hidden_entries(&root.join("vendor")), [".keep"]);
    }

    /// A unit whose `vendor/<key>/` is absent, as a run stopped between the
    /// two renames leaves it, is written again from its `dir`.
    #[test]
    fn a_unit_left_absent_by_an_interrupted_swap_is_written_again() {
        let fixture = TempDir::new("interrupted");
        let a = unit(&fixture.path().join("src/a"), &[("a.typl", "package a\n")]);
        let root = root_in(&fixture);
        let units = BTreeMap::from([("a".to_string(), a.clone())]);
        write_vendor(&root, &units).unwrap();
        fs::rename(root.join("vendor/a"), root.join("vendor/.replaced-a-999")).unwrap();
        let report = write_vendor(&root, &units).unwrap();
        assert_eq!(report.written, ["a"]);
        assert_eq!(tree_hash(&root.join("vendor/a")).unwrap(), a.tree);
        assert!(hidden_entries(&root.join("vendor")).is_empty());
    }

    /// A copy that does not match the unit's `tree` is an error: the old
    /// `vendor/<key>/` is kept and no staging directory is left.
    #[test]
    fn a_copy_that_does_not_match_its_tree_is_refused() {
        let fixture = TempDir::new("mismatch");
        let old = unit(
            &fixture.path().join("src/old"),
            &[("a.typl", "package a\n")],
        );
        let root = root_in(&fixture);
        write_vendor(&root, &BTreeMap::from([("a".to_string(), old.clone())])).unwrap();
        let mut new = unit(
            &fixture.path().join("src/new"),
            &[("a.typl", "package a\ntype T: m\n")],
        );
        // The source changed after the hash was taken.
        fs::write(new.dir.join("a.typl"), "package a\ntype U: m\n").unwrap();
        new.url = "https://r.example.com/u@v2".into();
        let err = write_vendor(&root, &BTreeMap::from([("a".to_string(), new)])).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
        assert!(err.to_string().contains("u@v2"), "{err}");
        assert_eq!(tree_hash(&root.join("vendor/a")).unwrap(), old.tree);
        assert!(hidden_entries(&root.join("vendor")).is_empty());
    }

    /// A regular file at a key path is replaced by the unit's tree, and no
    /// `.replaced-*` entry is left.
    #[test]
    fn a_file_at_a_key_path_is_replaced() {
        let fixture = TempDir::new("file-at-key");
        let a = unit(&fixture.path().join("src/a"), &[("a.typl", "package a\n")]);
        let root = root_in(&fixture);
        fs::create_dir_all(root.join("vendor")).unwrap();
        fs::write(root.join("vendor/a"), "not a directory").unwrap();
        let report = write_vendor(&root, &BTreeMap::from([("a".to_string(), a.clone())])).unwrap();
        assert_eq!(report.written, ["a"]);
        assert_eq!(tree_hash(&root.join("vendor/a")).unwrap(), a.tree);
        assert!(hidden_entries(&root.join("vendor")).is_empty());
    }

    /// With nothing to vendor and no `vendor/`, no directory is created.
    #[test]
    fn nothing_to_vendor_creates_no_directory() {
        let fixture = TempDir::new("empty");
        let root = root_in(&fixture);
        let report = write_vendor(&root, &BTreeMap::new()).unwrap();
        assert_eq!(report, VendorWrite::default());
        assert!(!root.join("vendor").exists());
    }
}
