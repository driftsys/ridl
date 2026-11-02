//! Baseline context validation and recoverable managed-file publication.
use ridlc::baseline_context::{BaselineContext, read_context as read, write_context};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

pub(crate) fn read_context(
    dir: &Path,
    own: &[ridl_ir::v2::Package],
) -> Result<Option<BaselineContext>, ExitCode> {
    read(dir, own).map_err(|err| {
        eprintln!("error: {err}");
        ExitCode::from(2)
    })
}

/// Bind the staged local inventory to evidence from the compile that emitted it.
pub(crate) fn stage_context(
    staging: &Path,
    own: &[ridl_ir::v2::Package],
    context: &BaselineContext,
) -> Result<(), ExitCode> {
    write_context(staging, context).map_err(|err| {
        eprintln!("error: {err}");
        ExitCode::from(2)
    })?;
    read_context(staging, own)?;
    Ok(())
}

/// Reserve a sibling staging directory without deleting any retained recovery
/// evidence from an earlier invocation, including a symlink occupying a name.
pub(crate) fn reserve_staging(out: &Path) -> std::io::Result<PathBuf> {
    let parent = out
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    std::fs::create_dir_all(parent)?;
    let name = out
        .file_name()
        .ok_or_else(|| std::io::Error::other("baseline path has no final name"))?;
    for index in 0u64.. {
        let mut candidate = std::ffi::OsString::from(".");
        candidate.push(name);
        candidate.push(".staging");
        if index != 0 {
            candidate.push(format!(".{index}"));
        }
        let candidate = parent.join(candidate);
        match std::fs::create_dir(&candidate) {
            Ok(()) => return Ok(candidate),
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(err) => return Err(err),
        }
    }
    Err(std::io::Error::other("no unused baseline staging name"))
}

/// Replace only managed entries, retaining the prior artifacts in an unused
/// sibling directory. Completed renames are undone on a commit failure. The
/// context is installed last; there is no crash-atomicity guarantee.
pub(crate) fn replace_preserving(staging: &Path, out: &Path) -> std::io::Result<PathBuf> {
    replace_with(staging, out, |from, to| std::fs::rename(from, to))
}

fn managed_name(name: &std::ffi::OsStr) -> bool {
    name.to_str().is_some_and(|name| {
        name.ends_with(".ir.json")
            || name.ends_with(".catalogs")
            || name == ridlc::baseline_context::CONTEXT_FILE
    })
}

fn managed_entries(dir: &Path) -> std::io::Result<Vec<std::ffi::OsString>> {
    let metadata = std::fs::symlink_metadata(dir)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(std::io::Error::other(format!(
            "{} is not a regular directory",
            dir.display()
        )));
    }
    let mut names = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        if managed_name(&entry.file_name()) {
            let kind = entry.file_type()?;
            if !kind.is_file() && !kind.is_symlink() {
                return Err(std::io::Error::other(format!(
                    "managed baseline path {} is not a file or symlink",
                    entry.path().display()
                )));
            }
            names.push(entry.file_name());
        }
    }
    names.sort();
    Ok(names)
}

fn reserve_backup(out: &Path) -> std::io::Result<PathBuf> {
    let name = out
        .file_name()
        .ok_or_else(|| std::io::Error::other("baseline path has no final name"))?;
    for index in 1u64.. {
        let mut candidate = name.to_os_string();
        candidate.push(format!(".previous.{index}"));
        let candidate = out.with_file_name(candidate);
        match std::fs::create_dir(&candidate) {
            Ok(()) => return Ok(candidate),
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(err) => return Err(err),
        }
    }
    Err(std::io::Error::other("no unused baseline backup name"))
}

fn replace_with(
    staging: &Path,
    out: &Path,
    mut rename: impl FnMut(&Path, &Path) -> std::io::Result<()>,
) -> std::io::Result<PathBuf> {
    let old = managed_entries(out)?;
    let mut fresh = managed_entries(staging)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt as _;
        if std::fs::metadata(staging)?.dev() != std::fs::metadata(out)?.dev() {
            return Err(std::io::Error::other(
                "staging and baseline must use the same filesystem",
            ));
        }
    }
    fresh.sort_by_key(|name| (name == ridlc::baseline_context::CONTEXT_FILE, name.clone()));
    let backup = reserve_backup(out)?;
    let mut moved_old = Vec::new();
    let mut installed = Vec::new();
    let commit = (|| -> std::io::Result<()> {
        for name in &old {
            rename(&out.join(name), &backup.join(name))?;
            moved_old.push(name);
        }
        for name in &fresh {
            rename(&staging.join(name), &out.join(name))?;
            installed.push(name);
        }
        Ok(())
    })();
    if let Err(commit_error) = commit {
        let mut recovery_errors = Vec::new();
        let mut retained_new = std::collections::BTreeSet::new();
        for name in installed.into_iter().rev() {
            let from = out.join(name);
            let to = staging.join(name);
            if let Err(err) = rename(&from, &to) {
                retained_new.insert(name);
                recovery_errors.push(format!("{} -> {}: {err}", from.display(), to.display()));
            }
        }
        for name in moved_old.into_iter().rev() {
            let from = backup.join(name);
            let to = out.join(name);
            // A failed retraction leaves new bytes at the destination. A
            // same-name restore could overwrite them on some platforms.
            if retained_new.contains(name) {
                recovery_errors.push(format!(
                    "pending restore {} -> {}: first retract the retained new entry {} -> {}",
                    from.display(),
                    to.display(),
                    to.display(),
                    staging.join(name).display()
                ));
                continue;
            }
            if let Err(err) = rename(&from, &to) {
                recovery_errors.push(format!("{} -> {}: {err}", from.display(), to.display()));
            }
        }
        if recovery_errors.is_empty() {
            if let Err(err) = std::fs::remove_dir(&backup) {
                eprintln!(
                    "warning: prior baseline restored, but cannot remove empty recovery directory {}: {err}",
                    backup.display()
                );
            }
            return Err(std::io::Error::other(format!(
                "{commit_error}; restored the prior managed baseline; staged files remain at {}",
                staging.display()
            )));
        }
        return Err(std::io::Error::other(format!(
            "{commit_error}; rollback failed: {}; recovery artifacts remain in {} and {}",
            recovery_errors.join("; "),
            staging.display(),
            backup.display()
        )));
    }
    // The final installation rename commits publication. Cleanup cannot turn
    // a committed success into a reported failure that promises restoration.
    if let Err(err) = std::fs::remove_dir(staging) {
        eprintln!(
            "warning: baseline published, but cannot remove staging directory {}: {err}",
            staging.display()
        );
    }
    eprintln!(
        "note: preserved the prior managed baseline at {}",
        backup.display()
    );
    Ok(backup)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct Fixture {
        root: PathBuf,
        out: PathBuf,
        stage: PathBuf,
    }
    impl Fixture {
        fn new() -> Self {
            static NEXT: AtomicUsize = AtomicUsize::new(0);
            let root = std::env::temp_dir().join(format!(
                "ridl-managed-publication-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            let out = root.join("baseline");
            let stage = root.join("staging");
            std::fs::create_dir_all(out.join("notes")).unwrap();
            std::fs::create_dir(&stage).unwrap();
            for (name, bytes) in [
                ("context.json", "old context"),
                ("a.ir.json", "old a"),
                ("stale.ir.json", "old stale"),
                ("notes/keep", "unrelated"),
            ] {
                std::fs::write(out.join(name), bytes).unwrap();
            }
            for (name, bytes) in [
                ("context.json", "new context"),
                ("a.ir.json", "new a"),
                ("new.ir.json", "new unit"),
            ] {
                std::fs::write(stage.join(name), bytes).unwrap();
            }
            #[cfg(unix)]
            std::os::unix::fs::symlink("notes/keep", out.join("note-link")).unwrap();
            Self { root, out, stage }
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    fn inventory(dir: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
        fn visit(dir: &Path, relative: &Path, files: &mut BTreeMap<PathBuf, Vec<u8>>) {
            for entry in std::fs::read_dir(dir.join(relative)).unwrap() {
                let entry = entry.unwrap();
                let name = relative.join(entry.file_name());
                let kind = entry.file_type().unwrap();
                let bytes = if kind.is_symlink() {
                    std::fs::read_link(entry.path())
                        .unwrap()
                        .to_string_lossy()
                        .as_bytes()
                        .to_vec()
                } else if kind.is_dir() {
                    visit(dir, &name, files);
                    b"directory".to_vec()
                } else {
                    std::fs::read(entry.path()).unwrap()
                };
                files.insert(name, bytes);
            }
        }
        let mut files = BTreeMap::new();
        visit(dir, Path::new(""), &mut files);
        files
    }

    #[test]
    fn managed_publication_fault_matrix() {
        // Three old entries and three installations, including a new unit and
        // removal of a stale old snapshot. Each forward rename fails once.
        for fail_at in 0..6 {
            let fixture = Fixture::new();
            let prior = inventory(&fixture.out);
            let mut calls = 0;
            let result = replace_with(&fixture.stage, &fixture.out, |from, to| {
                let call = calls;
                calls += 1;
                if call == fail_at {
                    Err(std::io::Error::other("injected rename failure"))
                } else {
                    std::fs::rename(from, to)
                }
            });
            assert!(result.is_err(), "forward rename {fail_at}");
            assert_eq!(
                inventory(&fixture.out),
                prior,
                "exact restoration after rename {fail_at}"
            );
            assert!(!fixture.root.join("baseline.previous.1").exists());
        }
    }

    #[test]
    fn successful_publication_preserves_unrelated_entries_and_occupied_backups() {
        let fixture = Fixture::new();
        let occupied = fixture.root.join("baseline.previous.1");
        std::fs::create_dir(&occupied).unwrap();
        std::fs::write(occupied.join("keep"), "occupied").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(&occupied, fixture.root.join("baseline.previous.2")).unwrap();
        #[cfg(not(unix))]
        std::fs::write(fixture.root.join("baseline.previous.2"), "occupied file").unwrap();
        #[cfg(unix)]
        let unrelated_inode = {
            use std::os::unix::fs::MetadataExt as _;
            std::fs::metadata(fixture.out.join("notes")).unwrap().ino()
        };
        let mut installed = Vec::new();
        let backup = replace_with(&fixture.stage, &fixture.out, |from, to| {
            if from.starts_with(&fixture.stage) {
                installed.push(from.file_name().unwrap().to_os_string());
            }
            std::fs::rename(from, to)
        })
        .unwrap();
        assert_eq!(backup, fixture.root.join("baseline.previous.3"));
        assert_eq!(installed.last().unwrap(), "context.json");
        assert_eq!(std::fs::read(occupied.join("keep")).unwrap(), b"occupied");
        #[cfg(unix)]
        assert_eq!(
            std::fs::read_link(fixture.root.join("baseline.previous.2")).unwrap(),
            occupied
        );
        assert_eq!(
            std::fs::read(fixture.out.join("notes/keep")).unwrap(),
            b"unrelated"
        );
        assert_eq!(
            std::fs::read(backup.join("stale.ir.json")).unwrap(),
            b"old stale"
        );
        assert!(!backup.join("notes").exists());
        assert!(!fixture.out.join("stale.ir.json").exists());
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt as _;
            assert_eq!(
                std::fs::metadata(fixture.out.join("notes")).unwrap().ino(),
                unrelated_inode
            );
            assert_eq!(
                std::fs::read_link(fixture.out.join("note-link")).unwrap(),
                Path::new("notes/keep")
            );
        }
    }

    #[test]
    fn rollback_failure_retains_recovery_artifacts_and_paths() {
        let fixture = Fixture::new();
        let mut failed_commit = false;
        let error = replace_with(&fixture.stage, &fixture.out, |from, to| {
            if from == fixture.stage.join("new.ir.json") {
                failed_commit = true;
                return Err(std::io::Error::other("commit failure"));
            }
            if failed_commit
                && from.file_name().is_some_and(|name| name == "a.ir.json")
                && from.parent() == Some(fixture.root.join("baseline.previous.1").as_path())
            {
                return Err(std::io::Error::other("restore failure"));
            }
            std::fs::rename(from, to)
        })
        .unwrap_err();
        let text = error.to_string();
        assert!(
            text.contains("commit failure")
                && text.contains("rollback failed")
                && text.contains("restore failure"),
            "{text}"
        );
        assert!(
            text.contains(fixture.stage.to_str().unwrap()) && text.contains("baseline.previous.1"),
            "{text}"
        );
        assert_eq!(
            std::fs::read(fixture.root.join("baseline.previous.1/a.ir.json")).unwrap(),
            b"old a"
        );
        assert_eq!(
            std::fs::read(fixture.stage.join("a.ir.json")).unwrap(),
            b"new a"
        );
        let retained = inventory(&fixture.stage);
        let retry = reserve_staging(&fixture.out).unwrap();
        assert_ne!(retry, fixture.stage);
        assert_eq!(
            inventory(&fixture.stage),
            retained,
            "retry allocation preserves recovery evidence"
        );
    }

    #[test]
    fn failed_new_file_retraction_does_not_overwrite_its_recovery_bytes() {
        let fixture = Fixture::new();
        let backup = fixture.root.join("baseline.previous.1");
        let mut attempted_same_name_restore = false;
        let error = replace_with(&fixture.stage, &fixture.out, |from, to| {
            if from == fixture.stage.join("new.ir.json") {
                return Err(std::io::Error::other("commit failure"));
            }
            if from == fixture.out.join("a.ir.json") && to == fixture.stage.join("a.ir.json") {
                return Err(std::io::Error::other("retraction failure"));
            }
            if from == backup.join("a.ir.json") && to == fixture.out.join("a.ir.json") {
                attempted_same_name_restore = true;
            }
            std::fs::rename(from, to)
        })
        .unwrap_err();
        assert_eq!(
            std::fs::read(fixture.out.join("a.ir.json")).unwrap(),
            b"new a",
            "the still-installed new bytes must not be overwritten during restoration"
        );
        assert_eq!(
            std::fs::read(backup.join("a.ir.json")).unwrap(),
            b"old a",
            "the original bytes remain in the backup until the new entry is retracted"
        );
        assert!(
            !attempted_same_name_restore,
            "a pending new entry blocks its same-name old restore"
        );
        let message = error.to_string();
        assert!(
            message.contains("retraction failure") && message.contains("pending restore"),
            "{message}"
        );
        for path in [
            fixture.out.join("a.ir.json"),
            fixture.stage.join("a.ir.json"),
            backup.join("a.ir.json"),
        ] {
            assert!(
                message.contains(path.to_str().unwrap()),
                "the recovery moves name all retained paths: {message}"
            );
        }
        assert_eq!(
            std::fs::read(fixture.stage.join("new.ir.json")).unwrap(),
            b"new unit"
        );
        assert_eq!(
            std::fs::read(fixture.out.join("context.json")).unwrap(),
            b"old context"
        );
    }

    #[test]
    fn managed_directory_blocker_refuses_before_any_mutation() {
        let fixture = Fixture::new();
        std::fs::remove_file(fixture.out.join("a.ir.json")).unwrap();
        std::fs::create_dir(fixture.out.join("a.ir.json")).unwrap();
        let prior = inventory(&fixture.out);
        assert!(replace_preserving(&fixture.stage, &fixture.out).is_err());
        assert_eq!(inventory(&fixture.out), prior);
        assert!(!fixture.root.join("baseline.previous.1").exists());
    }

    #[test]
    fn postcommit_cleanup_failure_keeps_success_and_staging() {
        let fixture = Fixture::new();
        let result = replace_with(&fixture.stage, &fixture.out, |from, to| {
            std::fs::rename(from, to)?;
            if from == fixture.stage.join("context.json") {
                std::fs::write(fixture.stage.join("cleanup-blocker"), "keep for recovery")?;
            }
            Ok(())
        });
        assert!(
            result.is_ok(),
            "the last installation rename committed publication"
        );
        assert_eq!(
            std::fs::read(fixture.out.join("context.json")).unwrap(),
            b"new context"
        );
        assert!(fixture.stage.join("cleanup-blocker").exists());
    }
}
