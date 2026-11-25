use super::{paths::release_target, selection::inspect};
use flate2::read::GzDecoder;
use ridl_core::release::{MIN_PINNABLE_RELEASE, Release};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    fs::{self, File, OpenOptions},
    io,
    path::{Path, PathBuf},
};

/// Removes only the stage exclusively claimed by this operation.
struct Stage(PathBuf);
impl Drop for Stage {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn existing(directory: &Path, release: Release, binary: &str) -> io::Result<Option<PathBuf>> {
    let Some(installed) = inspect(directory, release, binary)? else {
        return Ok(None);
    };
    if installed.damaged {
        return Err(io::Error::other(format!(
            "damaged toolchain directory {}; preserve or move it before installing again",
            directory.display()
        )));
    }
    Ok(Some(directory.join(binary).canonicalize()?))
}

fn download(url: &str) -> io::Result<Vec<u8>> {
    if let Some(path) = url.strip_prefix("file://") {
        if !Path::new(path).is_absolute() {
            return Err(io::Error::other("file source must be absolute"));
        }
        return fs::read(path).map_err(|e| io::Error::new(e.kind(), format!("{path}: {e}")));
    }
    let mut response = ureq::get(url)
        .call()
        .map_err(|e| io::Error::other(format!("download {url}: {e}")))?;
    let mut bytes = Vec::new();
    io::Read::read_to_end(&mut response.body_mut().as_reader(), &mut bytes)?;
    Ok(bytes)
}

fn verify(bytes: &[u8], record: &[u8], archive: &str) -> io::Result<()> {
    let bad = || {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("invalid SHA-256 checksum for {archive}"),
        )
    };
    let record = std::str::from_utf8(record).map_err(|_| bad())?;
    let line = record.strip_suffix('\n').unwrap_or(record);
    let line = line.strip_suffix('\r').unwrap_or(line);
    let Some(hash) = line
        .get(..64)
        .filter(|s| s.bytes().all(|b| b.is_ascii_hexdigit()))
    else {
        return Err(bad());
    };
    let Some(tail) = line.get(64..) else {
        return Err(bad());
    };
    if tail.strip_prefix("  ").or_else(|| tail.strip_prefix(" *")) != Some(archive) {
        return Err(bad());
    }
    if !format!("{:x}", Sha256::digest(bytes)).eq_ignore_ascii_case(hash) {
        return Err(bad());
    }
    Ok(())
}

fn extract(bytes: &[u8], stage: &Path, binary: &str) -> io::Result<()> {
    let required = [binary, "LICENSE", "THIRD-PARTY-NOTICES.txt"];
    let mut seen = BTreeSet::new();
    let mut archive = tar::Archive::new(GzDecoder::new(bytes));
    for entry in archive.entries()? {
        let mut entry = entry?;
        let path = entry.path_bytes();
        let name = std::str::from_utf8(&path)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?
            .to_string();
        if !entry.header().entry_type().is_file()
            || !required.contains(&name.as_str())
            || !seen.insert(name.clone())
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("unsafe or duplicate toolchain archive entry {name}"),
            ));
        }
        let mut output = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(stage.join(name))?;
        io::copy(&mut entry, &mut output)?;
    }
    // Consume the gzip trailer as well, so CRC/truncation errors cannot publish.
    io::copy(&mut archive.into_inner(), &mut io::sink())?;
    if seen.len() != required.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "toolchain archive is missing a required file",
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(stage.join(binary), fs::Permissions::from_mode(0o755))?;
    }
    Ok(())
}

/// Installs one verified release without modifying an existing version directory.
/// Production callers pass the fixed release origin; tests may pass a file source.
pub fn install(root: &Path, release: Release, target: &str, base_url: &str) -> io::Result<PathBuf> {
    if release < MIN_PINNABLE_RELEASE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("release {release} is below {MIN_PINNABLE_RELEASE}"),
        ));
    }
    let target = release_target(target)?;
    let binary = if target.ends_with("windows-msvc") {
        "ridl.exe"
    } else {
        "ridl"
    };
    let versions = root.join("versions");
    fs::create_dir_all(&versions)?;
    let versions = versions.canonicalize()?;
    let directory = versions.join(release.to_string());
    if let Some(binary) = existing(&directory, release, binary)? {
        return Ok(binary);
    }
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(versions.join(format!(".{release}.lock")))?;
    lock.lock()?;
    // The handle retains the OS lock until publication (or error cleanup) finishes.
    if let Some(binary) = existing(&directory, release, binary)? {
        return Ok(binary);
    }
    let stage_path = versions.join(format!(".tmp-{release}-{}", std::process::id()));
    fs::create_dir(&stage_path)?;
    let stage = Stage(stage_path);
    let archive = format!("ridl-{target}.tar.gz");
    let url = format!(
        "{}/editor-v{release}/{archive}",
        base_url.trim_end_matches('/')
    );
    let bytes = download(&url)?;
    let checksum = download(&format!("{url}.sha256"))?;
    verify(&bytes, &checksum, &archive)?;
    extract(&bytes, &stage.0, binary)?;
    File::create_new(stage.0.join(".complete"))?;
    // Cooperating installers use the same lock; never replace a directory found now.
    if fs::symlink_metadata(&directory).is_ok() {
        return Err(io::Error::other(format!(
            "toolchain directory appeared during installation: {}",
            directory.display()
        )));
    }
    fs::rename(&stage.0, &directory)?;
    let result = directory.join(binary).canonicalize()?;
    drop(stage);
    drop(lock);
    Ok(result)
}
