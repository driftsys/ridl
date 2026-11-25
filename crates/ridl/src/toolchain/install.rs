use super::{
    paths::{binary_name, release_target},
    selection::inspect,
};
use flate2::read::GzDecoder;
use ridl_core::release::{MIN_PINNABLE_RELEASE, Release};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    fs::{self, File, OpenOptions},
    io::{self, Read},
    path::{Path, PathBuf},
    time::Duration,
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

const ARCHIVE_LIMIT: u64 = 256 * 1024 * 1024;
const CHECKSUM_LIMIT: u64 = 4 * 1024;
const SIGNATURE_LIMIT: u64 = 4 * 1024;

fn read_download(reader: impl Read, url: &str, limit: u64) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    reader
        .take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| io::Error::new(e.kind(), format!("download {url}: {e}")))?;
    if bytes.len() as u64 > limit {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("download {url} exceeds {limit} bytes"),
        ));
    }
    Ok(bytes)
}

fn download_config(url: &str) -> ureq::config::Config {
    ureq::Agent::config_builder()
        .https_only(url.starts_with("https://"))
        .timeout_connect(Some(Duration::from_secs(30)))
        .timeout_global(Some(Duration::from_secs(10 * 60)))
        .build()
}

fn download(base_url: &str, url: &str, limit: u64) -> io::Result<Vec<u8>> {
    if let Some(path) = url.strip_prefix("file://") {
        if !Path::new(path).is_absolute() {
            return Err(io::Error::other("file source must be absolute"));
        }
        let file =
            File::open(path).map_err(|e| io::Error::new(e.kind(), format!("{path}: {e}")))?;
        return read_download(file, url, limit);
    }
    let config = download_config(base_url);
    let mut response = ureq::Agent::new_with_config(config)
        .get(url)
        .call()
        .map_err(|e| io::Error::other(format!("download {url}: {e}")))?;
    read_download(response.body_mut().as_reader(), url, limit)
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

fn verify_signature(
    bytes: &[u8],
    minisig: &[u8],
    public_key: &str,
    expected_comment: &str,
) -> io::Result<()> {
    let key = minisign_verify::PublicKey::from_base64(public_key).map_err(|e| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("invalid release public key: {e}"),
        )
    })?;
    let bad = |reason: String| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("invalid release signature: {reason}"),
        )
    };
    let text = std::str::from_utf8(minisig).map_err(|e| bad(e.to_string()))?;
    let signature = minisign_verify::Signature::decode(text).map_err(|e| bad(e.to_string()))?;
    key.verify(bytes, &signature, false)
        .map_err(|e| bad(e.to_string()))?;
    if signature.trusted_comment() != expected_comment {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "release signature is for `{}`, expected `{expected_comment}`",
                signature.trusted_comment()
            ),
        ));
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

/// Removes the stage directories that earlier runs of CLI installs left for
/// `release`. The caller holds the version's lock, which excludes every
/// cooperating owner of those stages.
pub(super) fn remove_stale_stages(versions: &Path, release: Release) -> io::Result<()> {
    let stage_prefix = format!(".tmp-{release}-");
    for entry in fs::read_dir(versions)? {
        let entry = entry?;
        if entry
            .file_name()
            .to_str()
            .and_then(|name| name.strip_prefix(&stage_prefix))
            .is_some_and(|pid| !pid.is_empty() && pid.bytes().all(|byte| byte.is_ascii_digit()))
            && entry.file_type()?.is_dir()
        {
            fs::remove_dir_all(entry.path())?;
        }
    }
    Ok(())
}

/// Installs one verified release without modifying an existing version directory.
/// Production callers pass the fixed release origin; tests may pass a file source.
pub fn install(
    root: &Path,
    release: Release,
    target: &str,
    base_url: &str,
    public_key: &str,
) -> io::Result<PathBuf> {
    if release < MIN_PINNABLE_RELEASE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("release {release} is below {MIN_PINNABLE_RELEASE}"),
        ));
    }
    let target = release_target(target)?;
    let binary = binary_name(target);
    let versions = root.join("versions");
    fs::create_dir_all(&versions)?;
    let versions = versions.canonicalize()?;
    let directory = versions.join(release.to_string());
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
    minisign_verify::PublicKey::from_base64(public_key).map_err(|e| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("invalid release public key: {e}"),
        )
    })?;
    remove_stale_stages(&versions, release)?;
    let stage_path = versions.join(format!(".tmp-{release}-{}", std::process::id()));
    fs::create_dir(&stage_path)?;
    let stage = Stage(stage_path);
    let archive = format!("ridl-{target}.tar.gz");
    let url = format!(
        "{}/editor-v{release}/{archive}",
        base_url.trim_end_matches('/')
    );
    let bytes = download(base_url, &url, ARCHIVE_LIMIT)?;
    let checksum = download(base_url, &format!("{url}.sha256"), CHECKSUM_LIMIT)?;
    verify(&bytes, &checksum, &archive)?;
    let minisig = download(base_url, &format!("{url}.minisig"), SIGNATURE_LIMIT)?;
    verify_signature(
        &bytes,
        &minisig,
        public_key,
        &format!("ridl {release} {target}"),
    )
    .map_err(|e| io::Error::new(e.kind(), format!("{archive}: {e}")))?;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn https_download_refuses_plain_http_url_before_connecting() {
        let error = download(
            "https://github.com/driftsys/ridl/releases/download",
            "http://127.0.0.1:1/archive",
            ARCHIVE_LIMIT,
        )
        .unwrap_err();
        assert!(
            error.to_string().contains("configured for https only"),
            "{error:?}"
        );
        assert!(
            !download_config("http://127.0.0.1:1").https_only(),
            "HTTP fixture origins remain available"
        );
    }

    const REAL_KEY: &str = "RWQf6LRCGA9i53mlYecO4IzT51TGPpvWucNSCh1CBM0QTaLn73Y7GFO3";
    const REAL_SIGNATURE: &str = "untrusted comment: signature from minisign secret key
RUQf6LRCGA9i559r3g7V1qNyJDApGip8MfqcadIgT9CuhV3EMhHoN1mGTkUidF/z7SrlQgXdy8ofjb7bNJJylDOocrCo8KLzZwo=
trusted comment: timestamp:1633700835\tfile:test\tprehashed
wLMDjy9FLAuxZ3q4NlEvkgtyhrr0gtTu6KC4KBJdITbbOeAi1zBIYo0v4iTgt8jJpIidRJnp94ABQkJAgAooBQ==";
    const LEGACY_SIGNATURE: &str = "untrusted comment: signature from minisign secret key
RWQf6LRCGA9i59SLOFxz6NxvASXDJeRtuZykwQepbDEGt87ig1BNpWaVWuNrm73YiIiJbq71Wi+dP9eKL8OC351vwIasSSbXxwA=
trusted comment: timestamp:1555779966\tfile:test
QtKMXWyYcwdpZAlPF7tE2ENJkRd1ujvKjlj1m9RtHTBnZPa5WKU5uWRs5GoP5M/VqE81QFuMKI5k/SfNQUaOAA==";

    #[test]
    fn signature_verifies_a_real_minisign_signature() {
        let comment = "timestamp:1633700835\tfile:test\tprehashed";
        let result = verify_signature(b"test", REAL_SIGNATURE.as_bytes(), REAL_KEY, comment);
        assert!(result.is_ok(), "{result:?}");
        assert!(verify_signature(b"test", REAL_SIGNATURE.as_bytes(), REAL_KEY, "x").is_err());
    }

    #[test]
    fn signature_refuses_a_legacy_signature() {
        let comment = "timestamp:1555779966\tfile:test";
        let result = verify_signature(b"test", LEGACY_SIGNATURE.as_bytes(), REAL_KEY, comment);
        assert!(result.is_err(), "{result:?}");
    }
}
