//! The vendor directory (ADR-0002 §7): `<manifest root>/vendor/<key>/` holds
//! the source tree of each remote unit the consumer imports, so a `--frozen`
//! build reads it before the cache and needs no network. [`write_vendor`]
//! brings the directory to exactly one set of units, and [`check_vendor`]
//! compares it with the imports and `ridl.lock` without writing.
//!
//! This module sits behind the `fetch` feature: it reads the `tree` hash.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io;
use std::path::Path;

use crate::diag::{DiagCode, Diagnostic};
use crate::fetch::{TreeOrigin, detached, verify_tree};
use crate::lock::{LockEntry, Lockfile};
use crate::tree_hash::tree_hash;
use crate::workspace::{ImportUnion, MaterializedUnit, VENDOR_DIR};

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

/// Compares `<root>/vendor/` with the consumer's `imports` and with `lock`
/// for `ridl vendor --check`. It reads no cache, makes no network access, and
/// writes nothing. Each diagnostic is detached and is an Error:
///
/// - `MANI-109` for a URL none of whose keys names a directory in `vendor/`.
/// - `MANI-103` for a vendored URL with no `ridl.lock` entry, and
///   `MANI-108` for one whose entry has no `tree`: the check cannot verify
///   the directory.
/// - `MANI-107` for a directory named by a key of a URL whose tree does not
///   match the pinned `tree`, or holds a file that is not a regular file.
///   Each directory named by a key of the URL is verified.
/// - `MANI-110` for a directory in `vendor/` whose name is no key of
///   `imports`. A hidden directory (which only an interrupted `ridl vendor`
///   leaves) and an entry that is not a directory are not reported, as
///   [`write_vendor`] leaves them alone.
///
/// A URL draws at most one of `MANI-109`, `MANI-103` and `MANI-108`, in that
/// order. The `MANI-110` diagnostics follow in name order.
///
/// The two directory tests differ on a symbolic link. A key's directory is
/// found with [`Path::is_dir`], which follows a symbolic link, so a link to
/// a directory at `vendor/<key>` is that key's directory. The `MANI-110`
/// listing uses the entry's own file type, which does not follow a link, so
/// a symbolic link in `vendor/` is never a stray.
pub fn check_vendor(
    root: &Path,
    imports: &ImportUnion,
    lock: Option<&Lockfile>,
) -> Vec<Diagnostic> {
    let vendor = root.join(VENDOR_DIR);
    let mut diagnostics = Vec::new();
    for (url, keys) in imports {
        let dirs: Vec<_> = keys
            .iter()
            .map(|key| vendor.join(key))
            .filter(|dir| dir.is_dir())
            .collect();
        if dirs.is_empty() {
            diagnostics.push(detached(
                DiagCode::MANI_109,
                format!(
                    "the imported unit `{}` (`{url}`) has no directory in `{}`; run `ridl vendor` to write it",
                    keys.iter().cloned().collect::<Vec<_>>().join("`, `"),
                    vendor.display()
                ),
            ));
            continue;
        }
        match lock.and_then(|lock| lock.entries.get(url)) {
            None => diagnostics.push(detached(
                DiagCode::MANI_103,
                format!(
                    "no `ridl.lock` entry for `{url}`, so `{}` cannot be verified; run `ridl vendor` to pin and vendor it",
                    dirs[0].display()
                ),
            )),
            Some(LockEntry { tree: None, .. }) => diagnostics.push(detached(
                DiagCode::MANI_108,
                format!(
                    "the `ridl.lock` entry for `{url}` has no `tree` hash, so `{}` cannot be verified; run `ridl vendor` to add it",
                    dirs[0].display()
                ),
            )),
            Some(LockEntry {
                tree: Some(expected),
                ..
            }) => {
                for dir in &dirs {
                    verify_tree(url, dir, expected, TreeOrigin::Vendor, &mut diagnostics);
                }
            }
        }
    }
    let named: BTreeSet<&String> = imports.values().flatten().collect();
    let mut strays = Vec::new();
    if let Ok(entries) = fs::read_dir(&vendor) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let is_dir = entry.file_type().is_ok_and(|kind| kind.is_dir());
            if is_dir && !name.starts_with('.') && !named.contains(&name) {
                strays.push((name, entry.path()));
            }
        }
    }
    strays.sort();
    for (name, path) in strays {
        diagnostics.push(detached(
            DiagCode::MANI_110,
            format!(
                "`{}` is in `vendor/` but no `[imports]` key names `{name}`; run `ridl vendor` to remove it",
                path.display()
            ),
        ));
    }
    diagnostics
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

    /// A unit no longer imported is removed, and so is any other directory
    /// that no key names, whether or not `ridl vendor` wrote it; a hidden
    /// directory, a file and a symbolic link are left alone.
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
        fs::create_dir_all(root.join("vendor/github.com/x")).unwrap();
        fs::write(root.join("vendor/github.com/x/x.go"), "package x").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(fixture.path().join("src/b"), root.join("vendor/link")).unwrap();
        let report = write_vendor(&root, &BTreeMap::from([("a".to_string(), a)])).unwrap();
        assert_eq!(report.removed, ["b", "github.com"]);
        assert_eq!(report.unchanged, ["a"]);
        assert!(!root.join("vendor/b").exists());
        assert!(!root.join("vendor/github.com").exists());
        assert!(root.join("vendor/README.md").is_file());
        assert!(root.join("vendor/.keep/k.typl").is_file());
        #[cfg(unix)]
        assert!(root.join("vendor/link/b.typl").is_file());
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

    fn union(url: &str, key: &str) -> ImportUnion {
        BTreeMap::from([(url.to_string(), BTreeSet::from([key.to_string()]))])
    }

    fn lock_for(url: &str, tree: &str) -> Lockfile {
        let mut lock = Lockfile::default();
        lock.entries.insert(
            url.into(),
            LockEntry {
                sha256: "a".repeat(64),
                tree: Some(tree.into()),
            },
        );
        lock
    }

    fn codes(diags: &[Diagnostic]) -> Vec<&str> {
        diags.iter().map(|d| d.code.as_str()).collect()
    }

    /// A matching directory draws nothing, and the check writes nothing.
    #[test]
    fn a_matching_vendor_checks_clean() {
        let fixture = TempDir::new("check-clean");
        let root = fixture.path().join("root");
        let unit = unit(&root.join("vendor/a"), &[("a.typl", "package a\n")]);
        let diags = check_vendor(
            &root,
            &union(&unit.url, "a"),
            Some(&lock_for(&unit.url, &unit.tree)),
        );
        assert!(diags.is_empty(), "{diags:?}");
        assert_eq!(tree_hash(&root.join("vendor/a")).unwrap(), unit.tree);
    }

    /// An imported unit with no directory is MANI-109; a directory no key
    /// names is MANI-110; a changed tree is MANI-107; a unit with no lock
    /// entry is MANI-103, and one whose entry has no `tree` is MANI-108.
    /// None of the messages claims `--frozen`, and each says what to run.
    #[test]
    fn drift_is_one_diagnostic_per_unit() {
        let fixture = TempDir::new("check-drift");
        let root = fixture.path().join("root");
        let a = unit(&root.join("vendor/a"), &[("a.typl", "package a\n")]);
        unit(&root.join("vendor/stray"), &[("s.typl", "package stray\n")]);
        let mut imports = union(&a.url, "a");
        for key in ["b", "c", "d"] {
            imports.insert(
                format!("https://r.example.com/{key}@v1"),
                BTreeSet::from([key.to_string()]),
            );
        }
        unit(&root.join("vendor/c"), &[("c.typl", "package c\n")]);
        unit(&root.join("vendor/d"), &[("d.typl", "package d\n")]);
        let mut lock = lock_for(&a.url, "0000");
        lock.entries.insert(
            "https://r.example.com/b@v1".into(),
            LockEntry {
                sha256: "b".repeat(64),
                tree: Some("1111".into()),
            },
        );
        lock.entries.insert(
            "https://r.example.com/d@v1".into(),
            LockEntry {
                sha256: "d".repeat(64),
                tree: None,
            },
        );
        let mut diags = check_vendor(&root, &imports, Some(&lock));
        diags.sort_by(|x, y| x.code.as_str().cmp(y.code.as_str()));
        assert_eq!(
            codes(&diags),
            ["MANI-103", "MANI-107", "MANI-108", "MANI-109", "MANI-110"]
        );
        assert!(diags[0].message.contains("c@v1"), "{}", diags[0].message);
        assert!(
            diags[1].message.contains("vendor/a") || diags[1].message.contains("vendor\\a"),
            "{}",
            diags[1].message
        );
        assert!(diags[2].message.contains("d@v1"), "{}", diags[2].message);
        assert!(diags[3].message.contains("b@v1"), "{}", diags[3].message);
        assert!(diags[4].message.contains("stray"), "{}", diags[4].message);
        for diag in &diags {
            assert!(!diag.message.contains("--frozen"), "{}", diag.message);
            assert!(diag.message.contains("ridl vendor"), "{}", diag.message);
            assert_eq!(diag.severity, crate::diag::Severity::Error);
        }
        assert!(
            root.join("vendor/stray/s.typl").is_file(),
            "nothing removed"
        );
    }

    /// Several directories no key names are each MANI-110, in name order
    /// whatever order the file system lists them in.
    #[test]
    fn strays_are_reported_in_name_order() {
        let fixture = TempDir::new("check-stray-order");
        let root = fixture.path().join("root");
        let a = unit(&root.join("vendor/a"), &[("a.typl", "package a\n")]);
        unit(&root.join("vendor/zeta"), &[("z.typl", "package z\n")]);
        unit(&root.join("vendor/mid"), &[("m.typl", "package m\n")]);
        unit(&root.join("vendor/alpha"), &[("p.typl", "package p\n")]);
        let diags = check_vendor(&root, &union(&a.url, "a"), Some(&lock_for(&a.url, &a.tree)));
        assert_eq!(codes(&diags), ["MANI-110", "MANI-110", "MANI-110"]);
        for (diag, name) in diags.iter().zip(["alpha", "mid", "zeta"]) {
            assert!(
                diag.message.contains(&format!("`{name}`")),
                "{name}: {}",
                diag.message
            );
        }
    }

    /// With no lockfile at all, a vendored unit is MANI-103: the check
    /// cannot verify it.
    #[test]
    fn no_lockfile_is_mani_103() {
        let fixture = TempDir::new("check-nolock");
        let root = fixture.path().join("root");
        let unit = unit(&root.join("vendor/a"), &[("a.typl", "package a\n")]);
        let diags = check_vendor(&root, &union(&unit.url, "a"), None);
        assert_eq!(codes(&diags), ["MANI-103"]);
    }

    /// A file rewritten with CRLF line ends is drift.
    #[test]
    fn a_crlf_rewrite_is_drift() {
        let fixture = TempDir::new("check-crlf");
        let root = fixture.path().join("root");
        let unit = unit(&root.join("vendor/a"), &[("a.typl", "package a\n")]);
        fs::write(root.join("vendor/a/a.typl"), "package a\r\n").unwrap();
        let diags = check_vendor(
            &root,
            &union(&unit.url, "a"),
            Some(&lock_for(&unit.url, &unit.tree)),
        );
        assert_eq!(codes(&diags), ["MANI-107"]);
    }

    /// With no `vendor/`, every import is MANI-109 and nothing else, and
    /// no directory is created.
    #[test]
    fn a_missing_vendor_is_mani_109_for_each_import() {
        let fixture = TempDir::new("check-missing");
        let root = root_in(&fixture);
        let mut imports = union("https://r.example.com/a@v1", "a");
        imports.insert(
            "https://r.example.com/b@v1".into(),
            BTreeSet::from(["b".to_string()]),
        );
        let diags = check_vendor(&root, &imports, None);
        assert_eq!(codes(&diags), ["MANI-109", "MANI-109"]);
        assert!(!root.join("vendor").exists());
    }

    /// A regular file at a key path is not the unit's directory: MANI-109.
    #[test]
    fn a_file_at_a_key_path_is_mani_109() {
        let fixture = TempDir::new("check-file-at-key");
        let root = root_in(&fixture);
        fs::create_dir_all(root.join("vendor")).unwrap();
        fs::write(root.join("vendor/a"), "not a directory").unwrap();
        let diags = check_vendor(&root, &union("https://r.example.com/a@v1", "a"), None);
        assert_eq!(codes(&diags), ["MANI-109"]);
    }

    /// A hidden directory, which only an interrupted `ridl vendor` leaves,
    /// and a file in `vendor/` are not MANI-110.
    #[test]
    fn hidden_directories_and_files_are_not_drift() {
        let fixture = TempDir::new("check-hidden");
        let root = fixture.path().join("root");
        let unit = unit(&root.join("vendor/a"), &[("a.typl", "package a\n")]);
        fs::create_dir_all(root.join("vendor/.staging-a-999")).unwrap();
        fs::write(root.join("vendor/README.md"), "notes").unwrap();
        let diags = check_vendor(
            &root,
            &union(&unit.url, "a"),
            Some(&lock_for(&unit.url, &unit.tree)),
        );
        assert!(diags.is_empty(), "{diags:?}");
    }

    /// Every directory named by a key of a URL is that unit's tree: two
    /// keys of one URL with matching directories draw nothing, and a second
    /// directory that does not match is MANI-107 naming it, not MANI-110.
    #[test]
    fn each_key_directory_of_a_url_is_verified() {
        let fixture = TempDir::new("check-two-keys");
        let root = fixture.path().join("root");
        let unit = unit(&root.join("vendor/a"), &[("a.typl", "package a\n")]);
        let b = root.join("vendor/b");
        fs::create_dir_all(&b).unwrap();
        fs::write(b.join("a.typl"), "package a\n").unwrap();
        let imports = BTreeMap::from([(
            unit.url.clone(),
            BTreeSet::from(["a".to_string(), "b".to_string()]),
        )]);
        let lock = lock_for(&unit.url, &unit.tree);
        let diags = check_vendor(&root, &imports, Some(&lock));
        assert!(diags.is_empty(), "{diags:?}");
        fs::write(b.join("a.typl"), "package b\n").unwrap();
        let diags = check_vendor(&root, &imports, Some(&lock));
        assert_eq!(codes(&diags), ["MANI-107"]);
        assert!(
            diags[0].message.contains("vendor/b") || diags[0].message.contains("vendor\\b"),
            "{}",
            diags[0].message
        );
    }

    /// A symbolic link in a vendored tree is MANI-107 naming the link.
    #[cfg(unix)]
    #[test]
    fn a_symlink_in_a_vendored_tree_is_mani_107() {
        let fixture = TempDir::new("check-symlink");
        let root = fixture.path().join("root");
        let unit = unit(&root.join("vendor/a"), &[("a.typl", "package a\n")]);
        std::os::unix::fs::symlink(
            root.join("vendor/a/a.typl"),
            root.join("vendor/a/link.typl"),
        )
        .unwrap();
        let diags = check_vendor(
            &root,
            &union(&unit.url, "a"),
            Some(&lock_for(&unit.url, &unit.tree)),
        );
        assert_eq!(codes(&diags), ["MANI-107"]);
        assert!(
            diags[0].message.contains("link.typl"),
            "{}",
            diags[0].message
        );
        assert!(
            diags[0].message.contains("not a regular file"),
            "{}",
            diags[0].message
        );
    }
}
