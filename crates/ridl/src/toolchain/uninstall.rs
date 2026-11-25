use super::install::remove_stale_stages;
use ridl_core::release::Release;
use std::{
    fs::{self, OpenOptions},
    io,
    path::{Path, PathBuf},
};

/// Removes the version directory of `release`, whether it is complete or damaged.
///
/// Returns the removed path under the canonical `versions` directory, or `None`
/// when `<root>/versions` or the entry does not exist. The entry is renamed to a
/// stage name first, so a reader never sees a half-removed directory, and a
/// symbolic link is removed without following it. The release that `running_exe`
/// belongs to is refused.
pub fn uninstall(root: &Path, release: Release, running_exe: &Path) -> io::Result<Option<PathBuf>> {
    let versions = match root.join("versions").canonicalize() {
        Ok(versions) => versions,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    let entry = versions.join(release.to_string());
    if let (Ok(running), Ok(entry)) = (running_exe.canonicalize(), entry.canonicalize())
        && running.starts_with(&entry)
    {
        return Err(io::Error::other(format!(
            "ridl {release} at {} is the running release; run another ridl to remove it",
            entry.display()
        )));
    }
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(versions.join(format!(".{release}.lock")))?;
    lock.lock()?;
    remove_stale_stages(&versions, release)?;
    match fs::symlink_metadata(&entry) {
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    }
    let stage = versions.join(format!(".tmp-{release}-{}", std::process::id()));
    fs::rename(&entry, &stage)?;
    if fs::symlink_metadata(&stage)?.is_dir() {
        fs::remove_dir_all(&stage)?;
    } else {
        fs::remove_file(&stage)?;
    }
    drop(lock);
    Ok(Some(entry))
}
