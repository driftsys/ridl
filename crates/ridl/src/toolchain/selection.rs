use super::paths::{binary_name, release_target};
use ridl_core::{
    RootRelease, read_root_release,
    release::{Release, compatible, parse_release, pin_value},
};
use serde::Serialize;
use std::{
    fs, io,
    path::{Path, PathBuf},
};

#[derive(Debug)]
pub struct InstalledRelease {
    pub release: Release,
    pub directory: PathBuf,
    /// Whether an empty regular completion marker is present.
    pub complete: bool,
    /// A missing marker or binary makes an existing directory unusable.
    pub damaged: bool,
}

fn regular(path: &Path) -> io::Result<Option<fs::Metadata>> {
    match fs::symlink_metadata(path) {
        Ok(m) => Ok(Some(m)),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(io::Error::new(e.kind(), format!("{}: {e}", path.display()))),
    }
}

pub(super) fn inspect(
    directory: &Path,
    release: Release,
    binary_name: &str,
) -> io::Result<Option<InstalledRelease>> {
    let Some(metadata) = regular(directory)? else {
        return Ok(None);
    };
    let complete = metadata.is_dir()
        && regular(&directory.join(".complete"))?.is_some_and(|m| m.is_file() && m.len() == 0);
    let binary =
        metadata.is_dir() && regular(&directory.join(binary_name))?.is_some_and(|m| m.is_file());
    Ok(Some(InstalledRelease {
        release,
        directory: directory.to_path_buf(),
        complete,
        damaged: !complete || !binary,
    }))
}

/// Lists canonical version directories numerically, retaining damaged entries.
pub fn list(root: &Path) -> io::Result<Vec<InstalledRelease>> {
    list_for_target(root, env!("RIDL_BUILD_TARGET"))
}

fn list_for_target(root: &Path, target: &str) -> io::Result<Vec<InstalledRelease>> {
    let versions = root.join("versions");
    let entries = match fs::read_dir(&versions) {
        Ok(entries) => entries,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => {
            return Err(io::Error::new(
                e.kind(),
                format!("{}: {e}", versions.display()),
            ));
        }
    };
    let mut out = Vec::new();
    for entry in entries {
        let entry = entry?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        let Some(release) = parse_release(name).filter(|v| v.to_string() == name) else {
            continue;
        };
        if !entry.file_type()?.is_dir() {
            continue;
        }
        if let Some(mut installed) = inspect(&entry.path(), release, binary_name(target))? {
            installed.directory = installed.directory.canonicalize()?;
            out.push(installed);
        }
    }
    out.sort_by_key(|installed| installed.release);
    Ok(out)
}

/// The target triple this binary was built for.
pub const BUILD_TARGET: &str = env!("RIDL_BUILD_TARGET");

/// Selects the highest complete installed release that is compatible with every pin.
///
/// Damaged entries are skipped. The result is the release and the canonical path
/// of its binary. No pins select nothing.
pub fn select_installed(
    root: &Path,
    pins: &[Release],
    target: &str,
) -> io::Result<Option<(Release, PathBuf)>> {
    if pins.is_empty() {
        return Ok(None);
    }
    let selected = list_for_target(root, target)?
        .into_iter()
        .rev()
        .find(|installed| {
            !installed.damaged && pins.iter().all(|pin| compatible(installed.release, *pin))
        });
    Ok(selected.map(|installed| {
        let binary = installed.directory.join(binary_name(target));
        (installed.release, binary)
    }))
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum WhichStatus {
    Compatible,
    Missing,
    Unpinned,
    Invalid,
}
#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum WhichSource {
    Installed,
    Running,
}
#[derive(Debug, Serialize)]
pub struct WhichDiagnostic {
    pub code: String,
    pub message: String,
}
#[derive(Debug, Serialize)]
pub struct WhichReport {
    pub schema: u32,
    pub manifest: Option<PathBuf>,
    pub pin: Option<String>,
    pub running: String,
    pub status: WhichStatus,
    pub path: Option<PathBuf>,
    pub source: Option<WhichSource>,
    pub diagnostic: Option<WhichDiagnostic>,
}
impl WhichReport {
    pub fn exit_code(&self) -> u8 {
        match self.status {
            WhichStatus::Compatible | WhichStatus::Unpinned => 0,
            WhichStatus::Missing | WhichStatus::Invalid => 1,
        }
    }
}

/// Reads only the root pin. Syntax and mode-table type errors are input errors.
pub(super) fn read_pin(entry: &Path) -> io::Result<RootRelease> {
    let mut root = read_root_release(entry, None)?;
    if let Some(text) = &root.text {
        let document: toml::Table =
            toml::from_str(text).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        for key in ["workspace", "package"] {
            if document.get(key).is_some_and(|v| !v.is_table()) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("manifest {key} must be a table"),
                ));
            }
        }
    }
    root.manifest = root.manifest.map(fs::canonicalize).transpose()?;
    Ok(root)
}

/// Resolves an install request from the current root pin without comparing releases.
pub fn workspace_pin(entry: &Path) -> io::Result<Release> {
    let root = read_pin(entry)?;
    if let Some(diagnostic) = root.diagnostics.first() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{}: {}", diagnostic.code.0, diagnostic.message),
        ));
    }
    root.pin.map(|pin| pin.release).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "no root ridl-version pin; pass an explicit release",
        )
    })
}

/// Selects the highest compatible installed or running release, preferring running ties.
pub fn which(
    entry: &Path,
    root: &Path,
    running: Release,
    running_exe: &Path,
    target: &str,
) -> io::Result<WhichReport> {
    let manifest = read_pin(entry)?;
    let mut report = WhichReport {
        schema: 1,
        manifest: manifest.manifest,
        pin: manifest.pin.as_ref().map(|p| p.release.to_string()),
        running: running.to_string(),
        status: WhichStatus::Unpinned,
        path: None,
        source: None,
        diagnostic: None,
    };
    if let Some(diagnostic) = manifest.diagnostics.first() {
        report.pin = manifest
            .text
            .as_deref()
            .and_then(pin_value)
            .and_then(|v| v.text)
            .and_then(|s| parse_release(&s))
            .map(|v| v.to_string());
        report.status = WhichStatus::Invalid;
        report.diagnostic = Some(WhichDiagnostic {
            code: diagnostic.code.0.to_string(),
            message: diagnostic.message.clone(),
        });
        return Ok(report);
    }
    let Some(pin) = manifest.pin else {
        report.path = Some(running_exe.canonicalize()?);
        report.source = Some(WhichSource::Running);
        return Ok(report);
    };
    report.status = WhichStatus::Missing;
    let mut best = None;
    if compatible(running, pin.release) {
        best = Some(running);
        report.path = Some(running_exe.canonicalize()?);
        report.source = Some(WhichSource::Running);
    }
    for installed in list_for_target(root, target)? {
        if installed.damaged || !compatible(installed.release, pin.release) {
            continue;
        }
        if let Err(error) = release_target(target) {
            if best.is_some() {
                continue;
            }
            return Err(error);
        }
        if best.is_none_or(|v| installed.release > v) {
            best = Some(installed.release);
            report.path = Some(
                installed
                    .directory
                    .join(binary_name(target))
                    .canonicalize()?,
            );
            report.source = Some(WhichSource::Installed);
        }
    }
    if best.is_some() {
        report.status = WhichStatus::Compatible;
    }
    Ok(report)
}
