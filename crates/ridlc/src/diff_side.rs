//! Loads a diff side without filesystem writes or remote-import fetches.
use crate::Emit;
use ridl_core::{Diagnostic, Overlay, RidlDatabase, SourceMap};
use std::io;
use std::path::{Path, PathBuf};

/// Resolved packages and the lowered system carried by a diff side.
#[derive(Debug)]
pub struct DiffSide {
    pub packages: Vec<ridl_ir::v2::Package>,
    pub system: Option<ridl_ir::v2::System>,
}

/// A refusal to load or compile one side of a diff.
pub enum DiffSideError {
    NonJsonIr(PathBuf),
    ArtifactDirectory {
        path: PathBuf,
        witness: PathBuf,
    },
    NestedDirectory {
        path: PathBuf,
        nested: PathBuf,
    },
    SnapshotDirectory {
        path: PathBuf,
        error: io::Error,
    },
    SnapshotMetadata {
        path: PathBuf,
        error: io::Error,
    },
    NestedRead {
        path: PathBuf,
        error: io::Error,
    },
    SnapshotRead {
        path: PathBuf,
        error: io::Error,
    },
    SnapshotParse {
        path: PathBuf,
        error: String,
    },
    SourceIo {
        path: PathBuf,
        error: io::Error,
    },
    OverlayOnSnapshot(PathBuf),
    Load(ridl_core::LoadError),
    Compile {
        diagnostics: Vec<Diagnostic>,
        sources: SourceMap,
    },
}

impl std::fmt::Display for DiffSideError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NonJsonIr(entry) => write!(
                f,
                "{}: `ridl diff` compares `.ir.json` snapshots only (ADR-0014 decision 5); \
             emit the package with `--emit ir-json` to compare it",
                entry.display()
            ),
            Self::ArtifactDirectory { path: dir, witness } => {
                let expectation = "`ridl diff` compares `.ir.json` snapshots only (ADR-0014 decision 5); emit \
                     the packages with `--emit ir-json` to compare them";
                write!(
                    f,
                    "{}: the directory holds IR artifacts (`{}`) but no `.ir.json` snapshot; \
         {expectation}",
                    dir.display(),
                    witness
                        .file_name()
                        .unwrap_or(witness.as_os_str())
                        .to_string_lossy()
                )
            }
            Self::NestedDirectory { path: dir, nested } => {
                let remedy = format!("compare `{}` instead", nested.display());
                write!(
                    f,
                    "{}: no `.ir.json` snapshot directly inside, but the subdirectory `{}` holds one; \
         snapshots are read from one directory, never from the directories below it; {remedy}",
                    dir.display(),
                    nested
                        .file_name()
                        .unwrap_or(nested.as_os_str())
                        .to_string_lossy()
                )
            }
            Self::SnapshotDirectory { path, error } => write!(
                f,
                "cannot read the snapshot directory {}: {error}",
                path.display()
            ),
            Self::SnapshotMetadata { path, error } => {
                write!(f, "cannot read the snapshot {}: {error}", path.display())
            }
            Self::NestedRead { path, error } => {
                write!(f, "cannot read {}: {error}", path.display())
            }
            Self::SnapshotRead { path, error } => write!(
                f,
                "{}: {}",
                path.display(),
                ridl_diff::LoadError::Io(io::Error::new(error.kind(), error.to_string()))
            ),
            Self::SnapshotParse { path, error } => write!(
                f,
                "{}: {}",
                path.display(),
                ridl_diff::LoadError::Parse(error.clone())
            ),
            Self::SourceIo { path, error } => write!(f, "{}: {error}", path.display()),
            Self::OverlayOnSnapshot(path) => write!(
                f,
                "overlays cannot be applied to snapshot side `{}`; pass a source path instead",
                path.display()
            ),
            Self::Load(error) => write!(f, "{error}"),
            Self::Compile { .. } => write!(f, "the source side does not compile"),
        }
    }
}
impl std::error::Error for DiffSideError {}

/// Loads a source path or a flat JSON snapshot set. Overlays apply only to source.
pub fn load_diff_side(
    db: &mut RidlDatabase,
    entry: &Path,
    overlays: &[Overlay],
) -> Result<DiffSide, DiffSideError> {
    if is_ir_json(entry) {
        refuse_snapshot_overlay(entry, overlays)?;
        return Ok(DiffSide {
            packages: load_snapshots(&[entry.to_path_buf()])?,
            system: None,
        });
    }
    if is_non_json_ir(entry) {
        return Err(DiffSideError::NonJsonIr(entry.to_path_buf()));
    }
    if entry.is_dir() {
        let snapshots = snapshot_files(entry)?;
        if !snapshots.is_empty() {
            refuse_snapshot_overlay(entry, overlays)?;
            return Ok(DiffSide {
                packages: load_snapshots(&snapshots)?,
                system: None,
            });
        }
        if !is_source_dir(entry) {
            if let Some(witness) = first_non_json_ir_in(entry) {
                return Err(DiffSideError::ArtifactDirectory {
                    path: entry.to_path_buf(),
                    witness,
                });
            }
            if let Some(nested) = first_nested_snapshot_dir(entry)? {
                return Err(DiffSideError::NestedDirectory {
                    path: entry.to_path_buf(),
                    nested,
                });
            }
        }
    }
    let output =
        crate::compile_workspace_with(db, entry, overlays).map_err(|error| match error {
            ridl_core::LoadError::Io(error) => DiffSideError::SourceIo {
                path: entry.to_path_buf(),
                error,
            },
            error => DiffSideError::Load(error),
        })?;
    if output
        .diagnostics
        .iter()
        .any(|d| d.severity == ridl_core::Severity::Error)
    {
        return Err(DiffSideError::Compile {
            diagnostics: output.diagnostics,
            sources: output.sources,
        });
    }
    Ok(DiffSide {
        packages: output.checked.into_iter().map(|c| c.ir).collect(),
        system: output.system,
    })
}

fn refuse_snapshot_overlay(path: &Path, overlays: &[Overlay]) -> Result<(), DiffSideError> {
    if overlays.is_empty() {
        Ok(())
    } else {
        Err(DiffSideError::OverlayOnSnapshot(path.to_path_buf()))
    }
}
impl std::fmt::Debug for DiffSideError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(self, f)
    }
}
const IR_JSON_SUFFIX: &str = match Emit::IrJson.ir_dump_suffix() {
    Some(suffix) => suffix,
    None => panic!("`ir-json` is an IR dump"),
};

/// Whether `path` is an `.ir.json` snapshot (a file whose name ends
/// [`IR_JSON_SUFFIX`]) rather than a source input.
///
/// `Path::is_file` is `false` on any metadata error as well as on a
/// directory, so an entry named like a snapshot that cannot be read — a
/// symlink whose target is gone — is not a snapshot to this test. That is
/// acceptable for a single named input, which then falls through to the
/// compiler and is reported there, but not for a directory listing, where a
/// skipped entry would read as an absent snapshot: [`snapshot_files`] tells
/// the two apart and reports the entry it cannot read (driftsys/ridl#339).
fn is_ir_json(path: &Path) -> bool {
    path.is_file() && has_ir_json_name(path)
}

/// Whether `path`'s file name ends [`IR_JSON_SUFFIX`], whatever the entry
/// behind it is.
fn has_ir_json_name(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.ends_with(IR_JSON_SUFFIX))
}

/// Whether `path` is an IR artifact in an encoding the snapshot surface must
/// refuse: every suffix the emit table names except `.ir.json` — prototext
/// (`.ir.txtpb`) and binary (`.ir.binpb`) today. Baselines and diffs stay
/// `.ir.json` (ADR-0014 decision 5) — a committed baseline must be
/// reviewable in a pull request. The suffixes are iterated from the table
/// rather than spelled here, so an encoding added to `ridlc` is refused by
/// name with no edit on this side (issue #218 item 4).
fn is_non_json_ir(path: &Path) -> bool {
    path.is_file()
        && path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| {
                Emit::ir_dump_suffixes()
                    .any(|(emit, suffix)| emit != Emit::IrJson && name.ends_with(suffix))
            })
}

/// Whether `path` is a source file by the rule the workspace loader collects
/// with: a file whose extension is `typl`, `ridl` or `rsdl`. Used only to tell
/// a source tree from a snapshot directory ([`is_source_dir`]) — a diff
/// *argument* is never gated on this, because a source file's own name is
/// unconstrained ([`load_diff_side`]).
fn is_source_file(path: &Path) -> bool {
    path.is_file()
        && path.extension().is_some_and(|extension| {
            extension == "typl" || extension == "ridl" || extension == "rsdl"
        })
}

/// Whether `dir` is a source tree by its direct contents: it holds a
/// `ridl.toml` or at least one `.typl`, `.ridl` or `.rsdl` file. A snapshot
/// directory — `.ridl/baseline/`, or a build `--out-dir` — holds neither.
fn is_source_dir(dir: &Path) -> bool {
    dir.join("ridl.toml").is_file()
        || files_matching(dir, is_source_file).is_ok_and(|files| !files.is_empty())
}

fn files_matching(dir: &Path, keep: fn(&Path) -> bool) -> std::io::Result<Vec<PathBuf>> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)?
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| keep(path))
        .collect();
    files.sort();
    Ok(files)
}

fn ir_json_files(dir: &Path) -> std::io::Result<Vec<PathBuf>> {
    files_matching(dir, is_ir_json)
}

fn first_non_json_ir_in(dir: &Path) -> Option<PathBuf> {
    files_matching(dir, is_non_json_ir)
        .unwrap_or_default()
        .into_iter()
        .next()
}

fn first_nested_snapshot_dir(dir: &Path) -> Result<Option<PathBuf>, DiffSideError> {
    let unreadable = |path: &Path, error: io::Error| DiffSideError::NestedRead {
        path: path.to_path_buf(),
        error,
    };
    let mut subdirectories: Vec<PathBuf> = std::fs::read_dir(dir)
        .map_err(|e| unreadable(dir, e))?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    subdirectories.sort();
    for subdirectory in subdirectories {
        if !ir_json_files(&subdirectory)
            .map_err(|e| unreadable(&subdirectory, e))?
            .is_empty()
        {
            return Ok(Some(subdirectory));
        }
    }
    Ok(None)
}
fn snapshot_files(dir: &Path) -> Result<Vec<PathBuf>, DiffSideError> {
    let named = files_matching(dir, has_ir_json_name).map_err(|error| {
        DiffSideError::SnapshotDirectory {
            path: dir.to_path_buf(),
            error,
        }
    })?;
    let mut files = Vec::new();
    for path in named {
        let metadata =
            std::fs::metadata(&path).map_err(|error| DiffSideError::SnapshotMetadata {
                path: path.clone(),
                error,
            })?;
        if metadata.is_file() {
            files.push(path);
        }
    }
    Ok(files)
}
fn load_snapshots(files: &[PathBuf]) -> Result<Vec<ridl_ir::v2::Package>, DiffSideError> {
    files
        .iter()
        .map(|file| {
            ridl_diff::load_ir_json(file).map_err(|error| match error {
                ridl_diff::LoadError::Io(error) => DiffSideError::SnapshotRead {
                    path: file.clone(),
                    error,
                },
                ridl_diff::LoadError::Parse(error) => DiffSideError::SnapshotParse {
                    path: file.clone(),
                    error,
                },
            })
        })
        .collect()
}
#[cfg(test)]
mod tests {
    use super::*;
    use ridl_core::{Overlay, RidlDatabase, Severity};
    use std::fs;

    fn source() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("ridl.toml"),
            "[package]\nname = \"p\"\nversion = \"1.0.0\"\n",
        )
        .unwrap();
        let path = dir.path().join("p.typl");
        fs::write(&path, "package p\ntype A: integer [0..1]\n").unwrap();
        (dir, path)
    }
    fn snapshot(dir: &Path) -> PathBuf {
        let (source, _) = source();
        let output = crate::compile_workspace(&mut RidlDatabase::default(), source.path()).unwrap();
        let path = dir.join("p.ir.json");
        fs::write(
            &path,
            ridl_ir::v2::to_json_pretty(&output.checked[0].ir).unwrap(),
        )
        .unwrap();
        path
    }
    fn load(path: &Path, overlays: &[Overlay]) -> Result<DiffSide, DiffSideError> {
        load_diff_side(&mut RidlDatabase::default(), path, overlays)
    }
    #[test]
    fn a_snapshot_file_loads() {
        let dir = tempfile::tempdir().unwrap();
        let result = load(&snapshot(dir.path()), &[]).unwrap();
        assert_eq!(result.packages.len(), 1);
        assert_eq!(result.packages[0].name, "p");
    }
    #[test]
    fn a_source_side_with_an_error_is_compile() {
        let (dir, path) = source();
        fs::write(path, "package p\nstruct A {\n  x: Missing\n}\n").unwrap();
        let error = load(dir.path(), &[]).err().unwrap();
        assert_eq!(error.to_string(), "the source side does not compile");
        let DiffSideError::Compile { diagnostics, .. } = error else {
            panic!("expected compile error")
        };
        assert_eq!(
            diagnostics
                .iter()
                .filter(|d| d.severity == Severity::Error)
                .count(),
            1
        );
    }
    #[test]
    fn an_overlay_on_a_snapshot_side_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let path = snapshot(dir.path());
        let error = load(
            &path,
            &[Overlay {
                path: path.clone(),
                text: String::new(),
            }],
        )
        .err()
        .unwrap();
        assert!(matches!(error, DiffSideError::OverlayOnSnapshot(_)));
    }
    #[test]
    fn an_overlay_reaches_a_source_side() {
        let (dir, path) = source();
        let side = load(
            dir.path(),
            &[Overlay {
                path,
                text: "package p\ntype A: integer [0..1]\ntype B: integer [0..1]\n".into(),
            }],
        )
        .unwrap();
        assert_eq!(
            side.packages[0]
                .decls
                .iter()
                .map(|d| d.name.as_str())
                .collect::<Vec<_>>(),
            ["A", "B"]
        );
    }
    #[test]
    fn a_non_json_ir_file_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("x.ir.txtpb");
        fs::write(&path, "").unwrap();
        let error = load(&path, &[]).err().unwrap();
        assert!(matches!(error, DiffSideError::NonJsonIr(_)));
        assert_eq!(
            error.to_string(),
            format!(
                "{}: `ridl diff` compares `.ir.json` snapshots only (ADR-0014 decision 5); emit the package with `--emit ir-json` to compare it",
                path.display()
            )
        );
    }
    #[test]
    fn an_artifact_directory_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("x.ir.txtpb"), "").unwrap();
        let error = load(dir.path(), &[]).err().unwrap();
        assert!(matches!(error, DiffSideError::ArtifactDirectory { .. }));
        assert_eq!(
            error.to_string(),
            format!(
                "{}: the directory holds IR artifacts (`x.ir.txtpb`) but no `.ir.json` snapshot; `ridl diff` compares `.ir.json` snapshots only (ADR-0014 decision 5); emit the packages with `--emit ir-json` to compare them",
                dir.path().display()
            )
        );
    }
    #[test]
    fn a_nested_snapshot_directory_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let nested = dir.path().join("nested");
        fs::create_dir(&nested).unwrap();
        snapshot(&nested);
        let error = load(dir.path(), &[]).err().unwrap();
        assert!(matches!(error, DiffSideError::NestedDirectory { .. }));
        assert_eq!(
            error.to_string(),
            format!(
                "{}: no `.ir.json` snapshot directly inside, but the subdirectory `nested` holds one; snapshots are read from one directory, never from the directories below it; compare `{}` instead",
                dir.path().display(),
                nested.display()
            )
        );
    }
    #[test]
    fn an_invalid_snapshot_is_a_parse_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("x.ir.json");
        fs::write(&path, "{").unwrap();
        let error = load(&path, &[]).err().unwrap();
        assert!(matches!(error, DiffSideError::SnapshotParse { .. }));
        let parse = ridl_diff::load_ir_json(&path).err().unwrap();
        assert_eq!(error.to_string(), format!("{}: {parse}", path.display()));
    }
    #[test]
    fn a_refused_overlay_on_a_source_side_is_load() {
        let (dir, _) = source();
        let error = load(
            dir.path(),
            &[Overlay {
                path: dir.path().join("x.toml"),
                text: String::new(),
            }],
        )
        .err()
        .unwrap();
        assert!(matches!(
            error,
            DiffSideError::Load(ridl_core::LoadError::OverlayNotSource(_))
        ));
    }
    #[test]
    fn a_missing_path_is_an_error_whose_text_is_the_cli_text() {
        let path = Path::new("nope");
        assert!(!path.exists());
        let error = load(path, &[]).err().unwrap();
        assert!(matches!(error, DiffSideError::SourceIo { .. }));
        assert_eq!(error.to_string(), "nope: `nope` does not exist");
    }
    #[cfg(unix)]
    #[test]
    fn an_unreadable_snapshot_directory_is_an_error() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let original = fs::metadata(dir.path()).unwrap().permissions();
        fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o000)).unwrap();
        let result = load(dir.path(), &[]);
        let io = fs::read_dir(dir.path()).err().unwrap();
        fs::set_permissions(dir.path(), original).unwrap();
        let error = result.err().unwrap();
        assert!(matches!(error, DiffSideError::SnapshotDirectory { .. }));
        assert_eq!(
            error.to_string(),
            format!(
                "cannot read the snapshot directory {}: {io}",
                dir.path().display()
            )
        );
    }
    #[cfg(unix)]
    #[test]
    fn an_unreadable_snapshot_file_is_an_error() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let private = dir.path().join("private");
        fs::create_dir(&private).unwrap();
        let target = snapshot(&private);
        let link = dir.path().join("p.ir.json");
        std::os::unix::fs::symlink(target, &link).unwrap();
        let original = fs::metadata(&private).unwrap().permissions();
        fs::set_permissions(&private, fs::Permissions::from_mode(0o000)).unwrap();
        let result = load(dir.path(), &[]);
        let io = fs::metadata(&link).err().unwrap();
        fs::set_permissions(private, original).unwrap();
        let error = result.err().unwrap();
        assert!(matches!(error, DiffSideError::SnapshotMetadata { .. }));
        assert_eq!(
            error.to_string(),
            format!("cannot read the snapshot {}: {io}", link.display())
        );
    }
    #[cfg(unix)]
    #[test]
    fn an_unreadable_ir_json_file_is_a_read_error() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let path = snapshot(dir.path());
        let original = fs::metadata(&path).unwrap().permissions();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o000)).unwrap();
        let result = load(&path, &[]);
        let io = fs::read_to_string(&path).err().unwrap();
        fs::set_permissions(&path, original).unwrap();
        let error = result.err().unwrap();
        assert!(matches!(error, DiffSideError::SnapshotRead { .. }));
        assert_eq!(
            error.to_string(),
            format!("{}: cannot read the IR snapshot: {io}", path.display())
        );
    }
    #[cfg(unix)]
    #[test]
    fn an_unreadable_nested_directory_is_an_error() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let nested = dir.path().join("nested");
        fs::create_dir(&nested).unwrap();
        let original = fs::metadata(&nested).unwrap().permissions();
        fs::set_permissions(&nested, fs::Permissions::from_mode(0o000)).unwrap();
        let result = load(dir.path(), &[]);
        let io = fs::read_dir(&nested).err().unwrap();
        fs::set_permissions(&nested, original).unwrap();
        let error = result.err().unwrap();
        assert!(matches!(error, DiffSideError::NestedRead { .. }));
        assert_eq!(
            error.to_string(),
            format!("cannot read {}: {io}", nested.display())
        );
    }
}
