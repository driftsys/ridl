//! Filesystem discovery: from an entry path to a loaded [`Workspace`]
//! (ADR-0002 §1, §4–5).
//!
//! This is the only module in the crate that touches the filesystem, and it
//! sits behind the default-on `fs` feature (ADR-0007 decision 5) so the crate
//! still builds for `wasm32-unknown-unknown` with `--no-default-features`.
//!
//! [`load_workspace`] walks from an entry path — a source file, a package
//! directory, or a workspace root — reads the `ridl.toml` manifests, loads
//! every source file (`.typl` and `.ridl` alike — a package may mix both)
//! into [`InputFile`] inputs, and enforces the package↔directory law (typl
//! reference §3.1): every file in a package directory must declare that
//! directory's package name (TYPL-002), and more than one `package`
//! declaration in a file is TYPL-001. A bare `.typl` or `.ridl` file with no
//! manifest anywhere up the tree loads in **single-file mode**: one synthetic
//! package named from the file's declared package, exempt from TYPL-002 (the
//! task 20 CLI contract). A unit's manifest directory is read for an
//! `interfaces.lock`, which rides on every [`Package`] of the unit as its
//! [`PackageLock`]; the bare file's directory is read the same way. A lock in
//! any other directory is not read (RIDL-416), and a malformed one is
//! RIDL-410 on the file's own line (lock design §2, §8).
//!
//! Problems in loaded content — manifest diagnostics, the law violations, a
//! nested workspace (MANI-004), a broken member (MANI-008), a file that is
//! not valid UTF-8 — are accumulated [`Diagnostic`]s, never an error return
//! (ADR-0004 §5). `std::io::Error` is reserved for real filesystem failures.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

use ridl_ir::codegen::{header_control_character, normalise_header};
use ridl_syntax::ast::{AstNode as _, SourceFile};
use rowan::{TextRange, TextSize};

use crate::db::{InputFile, RidlDatabase, parse_file};
use crate::diag::{DiagCode, Diagnostic, FileId, Severity, SourceMap, Span};
use crate::interface_lock;
use crate::lint::{LintLevels, LintScopes};
use crate::manifest::{Manifest, ManifestKind, TimingDefaults, parse_manifest};
use crate::package::{Package, PackageLock, PackageOrigin, Workspace, package_declarations};

/// The result of [`load_workspace`]: the salsa [`Workspace`] input, the
/// diagnostics the load accumulated, the interned path+text table the
/// diagnostics' [`Span`]s point into (what the caller hands to
/// [`render`](crate::diag::render())), and the effective lint levels by
/// directory (ADR-0024 decision 10): one scope for the workspace root,
/// one per member, one for a standalone package, none in single-file mode.
/// The scope keys are the directories in the same path form as the file
/// paths in `sources`, so [`LintScopes::for_path`] resolves a recorded path.
pub struct LoadedWorkspace {
    pub workspace: Workspace,
    pub diagnostics: Vec<Diagnostic>,
    pub sources: SourceMap,
    pub lints: LintScopes,
    /// The text of the file named by the root manifest's `[codegen]
    /// header-file`, normalised by [`ridl_ir::codegen::normalise_header`];
    /// `None` when no file is named, when the file holds no text, and in
    /// single-file mode.
    pub codegen_header: Option<String>,
    /// The member directory the entry lies in, when the entry is inside a
    /// member of the loaded workspace ([`find_root`] walked from the member to
    /// its workspace, or the entry named a path below a member); `None` for
    /// an entry at the workspace root, a standalone package, or single-file
    /// mode. The whole workspace is loaded and checked either way; a command
    /// that reports diagnostics reports only those under this directory
    /// (ADR-0024 decision 9). The path is in the same form as the file paths
    /// in `sources`.
    pub report_scope: Option<PathBuf>,
    /// The loaded units: each `[package]` manifest's `name` mapped to its
    /// directory, in the path form of the file paths in `sources`. A
    /// workspace root is not a unit; its members are. In single-file mode the
    /// one unit is the file's package, mapped to the file's directory.
    pub units: BTreeMap<String, PathBuf>,
}

/// Unsaved source text for a file in a loaded package directory.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Overlay {
    pub path: PathBuf,
    pub text: String,
}

/// A filesystem failure or an overlay that cannot belong to the workspace.
#[derive(Debug)]
pub enum LoadError {
    Io(io::Error),
    OverlayNotSource(PathBuf),
    OverlayOutsideWorkspace {
        path: PathBuf,
        missing_directory: bool,
    },
}

impl std::fmt::Display for LoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "{e}"),
            Self::OverlayNotSource(p) => write!(
                f,
                "overlay `{}` is not a `.typl`, `.ridl` or `.rsdl` file",
                p.display()
            ),
            Self::OverlayOutsideWorkspace {
                path,
                missing_directory: true,
            } => write!(
                f,
                "overlay `{}` is in a directory that does not exist; create the directory first",
                path.display()
            ),
            Self::OverlayOutsideWorkspace {
                path,
                missing_directory: false,
            } => write!(
                f,
                "overlay `{}` is not in a package directory of the workspace loaded from this path",
                path.display()
            ),
        }
    }
}

impl std::error::Error for LoadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<io::Error> for LoadError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}

/// The comparison key for a path: its parent directory canonicalised, joined
/// with its file name. `None` when the parent directory does not exist.
fn overlay_key(path: &Path) -> Option<PathBuf> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir().ok()?.join(path)
    };
    Some(
        absolute
            .parent()?
            .canonicalize()
            .ok()?
            .join(absolute.file_name()?),
    )
}

/// Loads the workspace reachable from `entry` into `db`.
///
/// `entry` may be:
///
/// - a `.typl` or `.ridl` file — [`find_root`] from the file's directory is
///   the root; with no manifest anywhere up the tree the file loads in
///   single-file mode;
/// - a package directory or workspace root — [`find_root`] from the
///   directory is the root; a `[package]` manifest loads that package's
///   directory tree, a `[workspace]` manifest loads every member.
///
/// An entry inside a workspace member loads the whole workspace, so the
/// root's `[lints]`, `[defaults]` and `[imports]` apply to the member
/// and its imports of sibling members resolve;
/// [`LoadedWorkspace::report_scope`] records the member.
///
/// `[imports]` maps stay scoped per ADR-0002 §5: each [`Package`] carries the
/// `[imports]` of the manifest governing its directory tree (step 2), and
/// [`Workspace::imports`] holds only the workspace root's `[imports]` (step
/// 3, the shared default). Nothing is merged — a member's pin never leaks to
/// a sibling member; the task 9 resolver walks the order itself. In a
/// standalone package load the manifest's `[imports]` ride on its packages
/// and [`Workspace::imports`] is empty.
pub fn load_workspace(db: &mut RidlDatabase, entry: &Path) -> io::Result<LoadedWorkspace> {
    load_workspace_with(db, entry, &[]).map_err(|error| match error {
        LoadError::Io(e) => e,
        _ => unreachable!("no overlay error is possible without overlays"),
    })
}

/// Loads a workspace with unsaved source text substituted before parsing.
pub fn load_workspace_with(
    db: &mut RidlDatabase,
    entry: &Path,
    overlays: &[Overlay],
) -> Result<LoadedWorkspace, LoadError> {
    let mut loader = Loader::default();
    for overlay in overlays {
        if !overlay
            .path
            .extension()
            .is_some_and(|ext| ext == "typl" || ext == "ridl" || ext == "rsdl")
        {
            return Err(LoadError::OverlayNotSource(overlay.path.clone()));
        }
        let key = overlay_key(&overlay.path).ok_or_else(|| LoadError::OverlayOutsideWorkspace {
            path: overlay.path.clone(),
            missing_directory: true,
        })?;
        loader.overlays.push((key, overlay.clone(), false));
    }

    if entry.is_file() {
        match entry.parent().and_then(find_root) {
            Some(root) => loader.load_root(db, &root)?,
            None => loader.load_single_file(db, entry)?,
        }
    } else if entry.is_dir() {
        match find_root(entry) {
            Some(root) => loader.load_root(db, &root)?,
            None => {
                return Err(io::Error::new(
                    io::ErrorKind::NotFound,
                    format!("no `ridl.toml` found at or above `{}`", entry.display()),
                )
                .into());
            }
        }
    } else {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("`{}` does not exist", entry.display()),
        )
        .into());
    }

    if let Some((_, overlay, _)) = loader.overlays.iter().find(|(_, _, consumed)| !consumed) {
        return Err(LoadError::OverlayOutsideWorkspace {
            path: overlay.path.clone(),
            missing_directory: false,
        });
    }
    let report_scope = absolute(entry).and_then(|entry| {
        loader
            .member_dirs
            .iter()
            .find(|member| absolute(member).is_some_and(|member| entry.starts_with(member)))
            .cloned()
    });
    let workspace = Workspace::new(&*db, loader.packages, loader.workspace_imports);
    Ok(LoadedWorkspace {
        workspace,
        diagnostics: loader.diagnostics,
        sources: loader.sources,
        lints: loader.lints,
        codegen_header: loader.codegen_header,
        report_scope,
        units: loader.units,
    })
}

/// The directory [`load_workspace`] loads from for an entry at or below
/// `dir` (ADR-0002 §4). It starts at the nearest directory at or above `dir`
/// that contains a `ridl.toml`. When that manifest is a `[package]`, the walk
/// continues upward:
///
/// - at the first `[workspace]` manifest it stops; that workspace is the root
///   when its `members` names the package directory, and the package is the
///   root otherwise;
/// - a `[package]` manifest above does not stop the walk;
/// - a manifest that cannot be read or parsed stops the walk and is the root,
///   so the loader reports why it failed;
/// - a directory that holds `.git` stops the walk after its own `ridl.toml`
///   is checked, and so does the filesystem root; the package is then the
///   root.
///
/// A relative `dir` gives a root in the same relative form, built with `..`
/// when the root is above the current directory. `None` means there is no
/// `ridl.toml` at or above `dir`. The command line, the language server and
/// the MCP server all call this, so every entry point loads the same root.
pub fn find_root(dir: &Path) -> Option<PathBuf> {
    let package = dir
        .ancestors()
        .find(|candidate| candidate.join("ridl.toml").is_file())?
        .to_path_buf();
    if !matches!(
        read_manifest_kind(&package),
        Some(ManifestKind::Package { .. })
    ) {
        return Some(package);
    }
    let Some(absolute_package) = absolute(&package) else {
        return Some(package);
    };
    for (levels, parent) in absolute_package.ancestors().skip(1).enumerate() {
        if parent.join("ridl.toml").is_file() {
            match read_manifest_kind(parent) {
                None => return Some(up(&package, levels + 1)),
                Some(ManifestKind::Workspace { members }) => {
                    let listed = members
                        .iter()
                        .any(|member| normalize(&parent.join(member)) == absolute_package);
                    return Some(if listed {
                        up(&package, levels + 1)
                    } else {
                        package
                    });
                }
                Some(ManifestKind::Package { .. }) => {}
            }
        }
        if parent.join(".git").exists() {
            break;
        }
    }
    Some(package)
}

/// The manifest kind of `dir/ridl.toml`, or `None` when the file cannot be
/// read as UTF-8 or does not parse as a manifest.
fn read_manifest_kind(dir: &Path) -> Option<ManifestKind> {
    let text = fs::read_to_string(dir.join("ridl.toml")).ok()?;
    parse_manifest(FileId::DETACHED, &text)
        .0
        .map(|manifest| manifest.kind)
}

/// `path` made absolute against the current directory and normalised
/// lexically; `None` when the current directory cannot be read.
fn absolute(path: &Path) -> Option<PathBuf> {
    if path.is_absolute() {
        Some(normalize(path))
    } else {
        Some(normalize(&std::env::current_dir().ok()?.join(path)))
    }
}

/// `path` with every `.` removed and every `..` applied to the component
/// before it, without reading the filesystem. A trailing `/` is dropped.
fn normalize(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if matches!(
                    normalized.components().next_back(),
                    Some(Component::Normal(_))
                ) {
                    normalized.pop();
                } else if !normalized.has_root() {
                    normalized.push("..");
                }
            }
            other => normalized.push(other),
        }
    }
    normalized
}

/// The directory `levels` levels above `path`, in the path form of `path`:
/// a trailing name is removed, and `..` is added once no name is left to
/// remove. Removing the last name of a relative path yields `.`, not the
/// empty path [`Path::ancestors`] yields: the empty path joins like the
/// current directory, but `read_dir`, `is_dir` and `exists` fail on it, so
/// a caller that walks the root's files would read nothing.
fn up(path: &Path, levels: usize) -> PathBuf {
    let mut result = path.to_path_buf();
    for _ in 0..levels {
        match result.components().next_back() {
            Some(Component::Normal(_)) => {
                result.pop();
                if result.as_os_str().is_empty() {
                    result = PathBuf::from(".");
                }
            }
            Some(Component::RootDir | Component::Prefix(_)) => {}
            Some(Component::CurDir) | None => result = PathBuf::from(".."),
            Some(Component::ParentDir) => result.push(".."),
        }
    }
    result
}

/// One loaded file plus its `package` declarations (dotted name, source
/// range), as [`Loader::load_file`] returns them.
type LoadedFile = (InputFile, Vec<(String, TextRange)>);

/// The accumulating state of one [`load_workspace`] run.
#[derive(Default)]
struct Loader {
    /// Source package name to the unit that claimed it and that unit's
    /// manifest directory.
    claims: BTreeMap<String, (String, PathBuf)>,
    overlays: Vec<(PathBuf, Overlay, bool)>,
    sources: SourceMap,
    diagnostics: Vec<Diagnostic>,
    packages: Vec<Package>,
    /// The workspace root's own `[imports]` (ADR-0002 §5 step 3). Stays empty
    /// in a standalone package load and in single-file mode.
    workspace_imports: BTreeMap<String, String>,
    /// The workspace root's `[defaults]` (ridl §9.1). A member's own
    /// `[defaults]` shadows it per key; a key the member leaves unset rides on
    /// the member's packages. Stays empty in a standalone package load and in
    /// single-file mode.
    workspace_defaults: TimingDefaults,
    /// The workspace root's effective lint levels: the registry defaults
    /// overlaid with the root `[lints]` (ADR-0002 §4).
    /// Each member's own table is overlaid on a clone. Stays at the defaults
    /// in a standalone package load and in single-file mode.
    workspace_lints: LintLevels,
    /// The effective lint levels by directory: one scope for the root, one per
    /// member directory, one for a standalone package, none in single-file mode
    /// (ADR-0024 decision 10). Each key is the directory in the path form the
    /// loader records for the files under it.
    lints: LintScopes,
    /// The workspace root's (or standalone package's) normalised header text.
    codegen_header: Option<String>,
    /// Every member directory of a loaded workspace, in the path form of the
    /// files recorded under it. Empty outside workspace mode.
    member_dirs: Vec<PathBuf>,
    /// Unit name to manifest directory, for [`LoadedWorkspace::units`].
    units: BTreeMap<String, PathBuf>,
}

impl Loader {
    /// Loads from a directory known to contain a `ridl.toml`, in whichever
    /// mode its manifest declares.
    fn load_root(&mut self, db: &mut RidlDatabase, root: &Path) -> io::Result<()> {
        let manifest_path = root.join("ridl.toml");
        // The error names the manifest: the root may be a workspace above the
        // entry (`find_root`), so the reader cannot assume which file failed.
        let text = fs::read_to_string(&manifest_path).map_err(|e| {
            io::Error::new(
                e.kind(),
                format!("cannot read `{}`: {e}", manifest_path.display()),
            )
        })?;
        let file_id = self.sources.file_id(&path_string(&manifest_path), &text);
        let (manifest, diags) = parse_manifest(file_id, &text);
        self.diagnostics.extend(diags);
        let Some(Manifest {
            kind,
            imports,
            defaults,
            lints,
            codegen_header_file,
        }) = manifest
        else {
            return Ok(());
        };
        if let Some((relative, range)) = codegen_header_file {
            let path = root.join(&relative);
            match fs::read_to_string(&path) {
                Ok(header) => match header_control_character(&header) {
                    None => self.codegen_header = normalise_header(&header),
                    Some(c) => self.diagnostics.push(error(
                        DiagCode::MANI_011,
                        file_id,
                        byte_range(range.start, range.end),
                        format!(
                            "`[codegen] header-file` contains a control character \
                             (U+{:04X}): `{}`",
                            u32::from(c),
                            path.display()
                        ),
                    )),
                },
                Err(e) => self.diagnostics.push(error(
                    DiagCode::MANI_011,
                    file_id,
                    byte_range(range.start, range.end),
                    format!(
                        "`[codegen] header-file` cannot be read: `{}`: {e}",
                        path.display()
                    ),
                )),
            }
        }
        // The root directory's scope: the registry defaults overlaid with the
        // root `[lints]`. In workspace mode it is also the base every member
        // overlays its own table on (ADR-0002 §4).
        let mut root_lints = LintLevels::default();
        root_lints.overlay(&lints);
        self.lints.insert(root.to_path_buf(), root_lints.clone());
        match kind {
            ManifestKind::Package { name, .. } => {
                // A standalone package: the manifest's `[imports]` and
                // `[defaults]` ride on its packages; the workspace maps
                // stay empty.
                self.units.insert(name.clone(), root.to_path_buf());
                let lock = self.read_lock(root)?;
                self.load_package_tree(db, root, &name, &name, &imports, &defaults, &lock)?;
            }
            ManifestKind::Workspace { members } => {
                // ADR-0002 §5 step 3: the workspace root's `[imports]` and
                // `[defaults]` are the shared defaults. Member maps are
                // never merged into them.
                self.workspace_imports = imports;
                self.workspace_defaults = defaults;
                self.workspace_lints = root_lints;
                for member in &members {
                    self.member_dirs.push(root.join(member));
                    self.load_member(db, root, member, file_id, &text)?;
                }
            }
        }
        Ok(())
    }

    /// Loads one workspace member directory: its manifest, then its package
    /// tree. A member manifest that declares `[workspace]` is a nested
    /// workspace — MANI-004 — and loads nothing.
    fn load_member(
        &mut self,
        db: &mut RidlDatabase,
        workspace_root: &Path,
        member: &str,
        workspace_file: FileId,
        workspace_text: &str,
    ) -> io::Result<()> {
        let manifest_path = workspace_root.join(member).join("ridl.toml");
        if !manifest_path.is_file() {
            // T7 records member paths unvalidated; the loader validates them
            // against the filesystem (MANI-008).
            self.diagnostics.push(error(
                DiagCode::MANI_008,
                workspace_file,
                member_entry_range(workspace_text, member),
                format!("workspace member `{member}` has no `ridl.toml`"),
            ));
            return Ok(());
        }
        let text = fs::read_to_string(&manifest_path)?;
        let file_id = self.sources.file_id(&path_string(&manifest_path), &text);
        let (manifest, diags) = parse_manifest(file_id, &text);
        self.diagnostics.extend(diags);
        let Some(Manifest {
            kind,
            imports,
            defaults,
            lints,
            codegen_header_file,
        }) = manifest
        else {
            return Ok(());
        };
        if let Some((_, range)) = codegen_header_file {
            self.diagnostics.push(error(
                DiagCode::MANI_012,
                file_id,
                byte_range(range.start, range.end),
                format!(
                    "`[codegen] header-file` is set in workspace member `{member}`; set it in the workspace root's `ridl.toml`"
                ),
            ));
        }
        // The member directory's scope: the root levels overlaid with the
        // member's own `[lints]` (ADR-0002 §4). The key
        // is the member directory in the same path form as the file paths
        // recorded under it, so `for_path` finds them by prefix.
        let mut member_lints = self.workspace_lints.clone();
        member_lints.overlay(&lints);
        self.lints.insert(workspace_root.join(member), member_lints);
        match kind {
            ManifestKind::Workspace { .. } => {
                self.diagnostics.push(error(
                    DiagCode::MANI_004,
                    file_id,
                    workspace_section_range(&text),
                    format!(
                        "workspace member `{member}` declares `[workspace]`; nested workspaces are forbidden"
                    ),
                ));
            }
            ManifestKind::Package { name, .. } => {
                // ADR-0002 §5 step 2: the member's `[imports]` ride on the
                // member's packages only — never merged into the workspace
                // map, never visible to a sibling member. Its
                // `[defaults]` shadow the workspace defaults per key (ridl §9.1);
                // a key the member leaves unset takes the workspace value.
                let member_defaults = defaults.or(&self.workspace_defaults);
                self.units.insert(name.clone(), workspace_root.join(member));
                let lock = self.read_lock(&workspace_root.join(member))?;
                self.load_package_tree(
                    db,
                    &workspace_root.join(member),
                    &name,
                    &name,
                    &imports,
                    &member_defaults,
                    &lock,
                )?;
            }
        }
        Ok(())
    }

    /// Loads the package rooted at `dir` under the package name `name` and the
    /// unit name `unit` (the manifest's `name`), then
    /// every subdirectory as its own package named by its path — the
    /// package↔directory law's "the name mirrors the directory path relative
    /// to the manifest root" (ADR-0002 §1). Every package in the tree carries
    /// `imports`, the governing manifest's `[imports]`. Directories are
    /// visited in name order; hidden directories, symlinked directories
    /// (following them could revisit the tree in a cycle), and directories
    /// with their own `ridl.toml` (separate package roots) are skipped. Every
    /// package of the tree carries `lock`, the unit's `interfaces.lock` read
    /// from the manifest directory; a lock in any other directory is not read
    /// and is RIDL-416.
    #[allow(clippy::too_many_arguments)]
    fn load_package_tree(
        &mut self,
        db: &mut RidlDatabase,
        dir: &Path,
        unit: &str,
        name: &str,
        imports: &BTreeMap<String, String>,
        defaults: &TimingDefaults,
        lock: &Option<PackageLock>,
    ) -> io::Result<()> {
        let mut source_files = Vec::new();
        let mut subdirs = Vec::new();
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            let is_symlink = entry.file_type()?.is_symlink();
            if path.is_dir() {
                if !is_symlink {
                    subdirs.push(path);
                }
            } else if path
                .extension()
                .is_some_and(|ext| ext == "typl" || ext == "ridl" || ext == "rsdl")
            {
                source_files.push(path);
            }
        }
        if !self.overlays.is_empty() {
            let directory_key = dir.canonicalize()?;
            for (key, _, _) in &self.overlays {
                if key.parent() == Some(directory_key.as_path())
                    && !source_files
                        .iter()
                        .any(|p| overlay_key(p).as_ref() == Some(key))
                {
                    let added = dir.join(key.file_name().expect("overlay keys have a file name"));
                    if !source_files.contains(&added) {
                        source_files.push(added);
                    }
                }
            }
        }
        source_files.sort();
        subdirs.sort();

        let mut claimed_elsewhere = false;
        if !source_files.is_empty() {
            let unit_dir = self.units.get(unit).cloned().unwrap_or_default();
            match self.claims.get(name) {
                Some((first, first_dir)) if first != unit => {
                    claimed_elsewhere = true;
                    let manifest = unit_dir.join("ridl.toml");
                    let text = fs::read_to_string(&manifest)?;
                    let file = self.sources.file_id(&path_string(&manifest), &text);
                    self.diagnostics.push(error(
                        DiagCode::MANI_014,
                        file,
                        package_name_range(&text),
                        format!(
                            "source package `{name}` is already declared by unit `{first}` (`{}`); unit `{unit}` declares it too, in `{}`. A source package belongs to one unit",
                            first_dir.display(),
                            unit_dir.display()
                        ),
                    ));
                }
                _ => {
                    self.claims
                        .insert(name.to_string(), (unit.to_string(), unit_dir));
                }
            }
        }
        if !source_files.is_empty() && !claimed_elsewhere {
            let mut files = Vec::new();
            for path in &source_files {
                if let Some((input, _)) = self.load_file(db, path, Some(name))? {
                    files.push(input);
                }
            }
            self.packages.push(Package::new(
                &*db,
                name.to_string(),
                unit.to_string(),
                files,
                PackageOrigin::WorkspaceMember,
                imports.clone(),
                defaults.clone(),
                lock.clone(),
            ));
        }

        for subdir in subdirs {
            let Some(dir_name) = subdir.file_name().map(|n| n.to_string_lossy().into_owned())
            else {
                continue;
            };
            if dir_name.starts_with('.') {
                continue;
            }
            let nested_manifest = subdir.join("ridl.toml");
            if nested_manifest.is_file() {
                let nested_text = fs::read_to_string(&nested_manifest)?;
                let nested_id = self
                    .sources
                    .file_id(&path_string(&nested_manifest), &nested_text);
                self.diagnostics.push(error(
                    DiagCode::MANI_013,
                    nested_id,
                    byte_range(0, 0),
                    format!(
                        "`{}` is a `ridl.toml` inside the tree of unit `{unit}`; a unit holds one manifest. Move the directory beside the unit, or delete the manifest",
                        nested_manifest.display()
                    ),
                ));
                continue;
            }
            let ignored = subdir.join(interface_lock::FILE_NAME);
            if ignored.is_file() {
                let ignored_text = fs::read_to_string(&ignored).unwrap_or_default();
                let ignored_id = self.sources.file_id(&path_string(&ignored), &ignored_text);
                self.diagnostics.push(warning(
                    DiagCode::RIDL_416,
                    ignored_id,
                    byte_range(0, 0),
                    format!(
                        "`{}` is an `interfaces.lock` inside the tree of unit `{unit}`; only the `interfaces.lock` beside the unit's `ridl.toml` is read, so this file is ignored",
                        ignored.display()
                    ),
                ));
            }
            self.load_package_tree(
                db,
                &subdir,
                unit,
                &format!("{name}.{dir_name}"),
                imports,
                defaults,
                lock,
            )?;
        }
        Ok(())
    }

    /// Loads one bare source file as a synthetic package named from its
    /// declared package — single-file mode, exempt from TYPL-002 (TYPL-001
    /// still applies). With no usable declaration the file stem names the
    /// package; the parser's FORM-104 for the missing declaration lives on
    /// `parse_file(..).errors()`, like every parse error — loader diagnostics
    /// carry only the manifest and law findings.
    fn load_single_file(&mut self, db: &mut RidlDatabase, path: &Path) -> io::Result<()> {
        let Some((input, decls)) = self.load_file(db, path, None)? else {
            // A non-UTF8 file: the diagnostic is recorded, nothing loads.
            return Ok(());
        };
        let name = decls
            .first()
            .map(|(name, _)| name.clone())
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| {
                path.file_stem()
                    .map(|stem| stem.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "package".to_string())
            });
        // The file's directory is the package directory, so the lock is read
        // there too (plan decision PD-8).
        let lock = match path.parent() {
            Some(dir) => self.read_lock(dir)?,
            None => None,
        };
        self.packages.push(Package::new(
            &*db,
            name.clone(),
            name.clone(),
            vec![input],
            PackageOrigin::WorkspaceMember,
            BTreeMap::new(),
            TimingDefaults::default(),
            lock,
        ));
        if let Some(dir) = path.parent() {
            self.units.insert(name, dir.to_path_buf());
        }
        Ok(())
    }

    /// Reads `dir/interfaces.lock` for the unit whose manifest directory is
    /// `dir`, or for the directory of a bare source file (lock design §2). An absent file is `None`. A malformed file — one that is
    /// not valid UTF-8 included — is RIDL-410 on the offending line of the
    /// lock file itself, through this loader's source map, at the empty range
    /// 0..0 when there is no line to point at (plan decision PD-3); the
    /// package then carries no lock. Any other I/O failure is the error.
    fn read_lock(&mut self, dir: &Path) -> io::Result<Option<PackageLock>> {
        let path = path_string(&dir.join(interface_lock::FILE_NAME));
        let text = match interface_lock::read(dir) {
            Ok(Some(text)) => text,
            Ok(None) => return Ok(None),
            Err(err) if err.kind() == io::ErrorKind::InvalidData => {
                let file_id = self.sources.file_id(&path, "");
                self.diagnostics.push(error(
                    DiagCode::RIDL_410,
                    file_id,
                    byte_range(0, 0),
                    malformed_lock_message("the file is not valid UTF-8"),
                ));
                return Ok(None);
            }
            Err(err) => return Err(err),
        };
        match interface_lock::parse(&text) {
            Ok(lock) => Ok(Some(PackageLock { path, text, lock })),
            Err(malformed) => {
                let file_id = self.sources.file_id(&path, &text);
                self.diagnostics.push(error(
                    DiagCode::RIDL_410,
                    file_id,
                    malformed.range,
                    malformed_lock_message(&malformed.message),
                ));
                Ok(None)
            }
        }
    }

    /// Reads one source file into an [`InputFile`], parses it through the
    /// salsa query, and enforces the package↔directory law: every `package`
    /// declaration after the first is TYPL-001; when `expected` is given and
    /// the first declared name differs, TYPL-002 with the declaration line as
    /// the primary span. Returns the input plus the file's declarations, or
    /// `None` for a file that is not valid UTF-8 — recorded as a diagnostic
    /// and skipped, never an abort of the whole load (ADR-0004 §5).
    fn load_file(
        &mut self,
        db: &mut RidlDatabase,
        path: &Path,
        expected: Option<&str>,
    ) -> io::Result<Option<LoadedFile>> {
        let path_str = path_string(path);
        let replacement = self
            .overlays
            .iter_mut()
            .filter(|(key, _, _)| overlay_key(path).as_ref() == Some(key))
            .map(|(_, overlay, consumed)| {
                *consumed = true;
                overlay.text.clone()
            })
            .last();
        let text = match replacement
            .map(Ok)
            .unwrap_or_else(|| fs::read_to_string(path))
        {
            Ok(text) => text,
            Err(err) if err.kind() == io::ErrorKind::InvalidData => {
                // No text means no spans; the diagnostic points at the start
                // of the interned (empty) file. No code is cataloged for a
                // broken source encoding, so it carries the `NONE` sentinel.
                let file_id = self.sources.file_id(&path_str, "");
                self.diagnostics.push(error(
                    DiagCode::NONE,
                    file_id,
                    byte_range(0, 0),
                    format!("`{path_str}` is not valid UTF-8; the file is skipped"),
                ));
                return Ok(None);
            }
            Err(err) => return Err(err),
        };
        let file_id = self.sources.file_id(&path_str, &text);
        let input = InputFile::new(&*db, path_str, text);

        let parse = parse_file(&*db, input);
        let source =
            SourceFile::cast(parse.syntax()).expect("parser roots every tree in a SourceFile");
        let decls = package_declarations(&source);

        for (_, range) in decls.iter().skip(1) {
            self.diagnostics.push(error(
                DiagCode::TYPL_001,
                file_id,
                *range,
                "more than one `package` declaration in this file".to_string(),
            ));
        }
        if let Some((declared, range)) = decls.first()
            && crate::std_lib::is_reserved_package_name(declared)
        {
            // Reported on the declaration rather than on the manifest or the
            // directory, because that is the one place both paths meet: a
            // workspace member and single-file mode both arrive here, and the
            // issue this closes (driftsys/ridl#203) names both.
            //
            // The message states only the unreachability, which always holds.
            // The artifact overwrite that issue reports is a consequence in
            // package and workspace mode, where the output base is the package
            // name; in single-file mode the base is the file stem, so it
            // collides only when that stem is itself `ridl.std`, whatever the
            // extension. That distinction belongs in the catalogue entry, not
            // in a message that would then be false for some of the inputs it
            // greets.
            self.diagnostics.push(error(
                DiagCode::TYPL_010,
                file_id,
                *range,
                format!(
                    "`{declared}` is provided by the compiler, so a package cannot declare it; every package already imports all of `{declared}` implicitly (typl §3.2), which leaves these declarations unreachable under their own name. Rename the package"
                ),
            ));
        }
        if let (Some(expected), Some((declared, range))) = (expected, decls.first())
            && !declared.is_empty()
            && declared != expected
        {
            self.diagnostics.push(error(
                DiagCode::TYPL_002,
                file_id,
                *range,
                format!(
                    "package name `{declared}` does not mirror the directory path; every file in this directory must declare `package {expected}`"
                ),
            ));
        }
        Ok(Some((input, decls)))
    }
}

/// The interned string form of a filesystem path.
fn path_string(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

/// The RIDL-410 message: what is wrong with the lock file, then the fix the
/// lock design §8 names.
fn malformed_lock_message(reason: &str) -> String {
    format!(
        "`{}` is malformed: {reason} — resolve the conflict or restore the file from version \
         control, then run `ridl lock`",
        interface_lock::FILE_NAME
    )
}

/// The byte range of the quoted `member` entry inside a workspace manifest's
/// text, or the whole file when it cannot be found (T7 does not retain member
/// spans).
fn member_entry_range(text: &str, member: &str) -> TextRange {
    let quoted = format!("\"{member}\"");
    match text.find(&quoted) {
        Some(start) => byte_range(start, start + quoted.len()),
        None => byte_range(0, text.len()),
    }
}

/// The byte range of the quoted `name` value of a manifest's `[package]`
/// table, or the whole file as a fallback.
fn package_name_range(text: &str) -> TextRange {
    let mut offset = 0;
    for line in text.split_inclusive('\n') {
        let value = line
            .trim_start()
            .strip_prefix("name")
            .and_then(|rest| rest.trim_start().strip_prefix('='))
            .map(str::trim_start)
            .and_then(|value| value.strip_prefix('"').map(|inner| (value, inner)));
        if let Some((value, inner)) = value
            && let Some(len) = inner.find('"')
        {
            let start = offset + (line.len() - value.len());
            return byte_range(start, start + len + 2);
        }
        offset += line.len();
    }
    byte_range(0, text.len())
}

/// The byte range of the `[workspace]` section header inside a manifest's
/// text, or the whole file as a fallback.
fn workspace_section_range(text: &str) -> TextRange {
    const HEADER: &str = "[workspace]";
    match text.find(HEADER) {
        Some(start) => byte_range(start, start + HEADER.len()),
        None => byte_range(0, text.len()),
    }
}

/// A `rowan::TextRange` over byte offsets.
fn byte_range(start: usize, end: usize) -> TextRange {
    TextRange::new(TextSize::from(start as u32), TextSize::from(end as u32))
}

/// Builds an error [`Diagnostic`]; loader diagnostics carry no secondary
/// labels or fix-its.
fn error(code: DiagCode, file: FileId, range: TextRange, message: String) -> Diagnostic {
    Diagnostic {
        code,
        severity: Severity::Error,
        message,
        primary: Span { file, range },
        labels: Vec::new(),
        fixits: Vec::new(),
    }
}

fn warning(code: DiagCode, file: FileId, range: TextRange, message: String) -> Diagnostic {
    Diagnostic {
        severity: Severity::Warning,
        ..error(code, file, range, message)
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use salsa::Setter;
    use salsa::plumbing::AsId;

    use super::*;
    use crate::lint::{LintLevel, apply_lint_levels, lint_by_name};

    /// A unique directory under the system temp dir, removed on drop.
    struct TempDir(PathBuf);

    impl TempDir {
        fn new(label: &str) -> Self {
            static COUNTER: AtomicUsize = AtomicUsize::new(0);
            let mut path = std::env::temp_dir();
            path.push(format!(
                "ridl-core-workspace-{label}-{}-{}",
                std::process::id(),
                COUNTER.fetch_add(1, Ordering::SeqCst),
            ));
            fs::create_dir_all(&path).expect("create the temp dir");
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }

        /// Writes `text` at `relative`, creating parent directories.
        fn write(&self, relative: &str, text: &str) -> PathBuf {
            let path = self.0.join(relative);
            fs::create_dir_all(path.parent().expect("relative paths have a parent"))
                .expect("create parent directories");
            fs::write(&path, text).expect("write the fixture file");
            path
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn codes(diags: &[Diagnostic]) -> Vec<&str> {
        diags.iter().map(|d| d.code.as_str()).collect()
    }

    fn overlay_fixture() -> (TempDir, PathBuf) {
        let dir = TempDir::new("overlay");
        dir.write(
            "p/ridl.toml",
            "[package]\nname = \"p\"\nversion = \"1.0.0\"\n",
        );
        let path = dir.write("p/a.typl", "package p\ntype A: integer [0..1]\n");
        (dir, path)
    }

    fn overlay(path: PathBuf, text: &str) -> Overlay {
        Overlay {
            path,
            text: text.to_string(),
        }
    }

    #[test]
    fn overlay_replaces_the_text_of_a_file_on_disk() {
        let (dir, path) = overlay_fixture();
        let text = "package p\n\ntype Other: integer [0..1]\n";
        let mut db = RidlDatabase::default();
        let loaded = load_workspace_with(
            &mut db,
            &dir.path().join("p"),
            &[overlay(path.clone(), text)],
        )
        .unwrap();
        let files = loaded.workspace.packages(&db)[0].files(&db);
        assert_eq!(files.len(), 1);
        let file = files[0];
        assert_eq!(file.text(&db), text);
        let entries: Vec<_> = loaded.sources.iter_files().collect();
        assert!(entries.contains(&(path_string(&path).as_str(), text)));
    }

    #[test]
    fn an_added_file_sorts_with_the_disk_files() {
        let (dir, _) = overlay_fixture();
        let mut db = RidlDatabase::default();
        let loaded = load_workspace_with(
            &mut db,
            &dir.path().join("p"),
            &[overlay(dir.path().join("p/0.typl"), "package p\n")],
        )
        .unwrap();
        let files = loaded.workspace.packages(&db)[0].files(&db);
        assert_eq!(files.len(), 2);
        assert!(files[0].path(&db).ends_with("0.typl"));
        assert!(files[1].path(&db).ends_with("a.typl"));
    }
    #[test]
    fn an_overlay_replaces_rather_than_adds() {
        let (dir, path) = overlay_fixture();
        let mut db = RidlDatabase::default();
        let loaded = load_workspace_with(
            &mut db,
            &dir.path().join("p"),
            &[overlay(
                path,
                "package p\ntype Replacement: integer [0..1]\n",
            )],
        )
        .unwrap();
        assert_eq!(loaded.workspace.packages(&db)[0].files(&db).len(), 1);
    }

    #[test]
    fn overlay_adds_a_file_to_the_package_of_its_directory() {
        let (dir, _) = overlay_fixture();
        let mut db = RidlDatabase::default();
        let loaded = load_workspace_with(
            &mut db,
            &dir.path().join("p"),
            &[overlay(dir.path().join("p/b.typl"), "package p\n")],
        )
        .unwrap();
        let files = loaded.workspace.packages(&db)[0].files(&db);
        assert_eq!(files.len(), 2);
        assert!(files[0].path(&db).ends_with("a.typl"));
        assert!(files[1].path(&db).ends_with("b.typl"));
    }

    #[test]
    fn overlay_in_an_existing_subdirectory_joins_its_package() {
        let (dir, _) = overlay_fixture();
        dir.write("p/sub/c.typl", "package p.sub\n");
        let mut db = RidlDatabase::default();
        let loaded = load_workspace_with(
            &mut db,
            &dir.path().join("p"),
            &[overlay(dir.path().join("p/sub/d.typl"), "package p.sub\n")],
        )
        .unwrap();
        let package = loaded
            .workspace
            .packages(&db)
            .iter()
            .find(|p| p.name(&db) == "p.sub")
            .unwrap();
        let files = package.files(&db);
        assert_eq!(files.len(), 2);
        assert!(files[0].path(&db).ends_with("c.typl"));
        assert!(files[1].path(&db).ends_with("d.typl"));
    }

    #[test]
    fn an_added_file_with_the_wrong_package_name_draws_typl_002() {
        let (dir, _) = overlay_fixture();
        let mut db = RidlDatabase::default();
        let loaded = load_workspace_with(
            &mut db,
            &dir.path().join("p"),
            &[overlay(dir.path().join("p/b.typl"), "package q\n")],
        )
        .unwrap();
        let diag = loaded
            .diagnostics
            .iter()
            .find(|d| d.code == DiagCode::TYPL_002)
            .unwrap();
        assert!(
            loaded
                .sources
                .path(diag.primary.file)
                .unwrap()
                .ends_with("b.typl")
        );
    }

    #[test]
    fn an_overlay_in_a_missing_directory_is_refused() {
        let (dir, _) = overlay_fixture();
        let result = load_workspace_with(
            &mut RidlDatabase::default(),
            &dir.path().join("p"),
            &[overlay(
                dir.path().join("p/nope/e.typl"),
                "package p.nope\n",
            )],
        );
        assert!(matches!(
            result,
            Err(LoadError::OverlayOutsideWorkspace {
                missing_directory: true,
                ..
            })
        ));
    }

    #[test]
    fn an_overlay_outside_the_workspace_is_refused() {
        let (dir, _) = overlay_fixture();
        let path = dir.write("sibling/a.typl", "package sibling\n");
        let result = load_workspace_with(
            &mut RidlDatabase::default(),
            &dir.path().join("p"),
            &[overlay(path, "package sibling\n")],
        );
        assert!(matches!(
            result,
            Err(LoadError::OverlayOutsideWorkspace {
                missing_directory: false,
                ..
            })
        ));
    }

    #[test]
    fn an_overlay_under_a_hidden_directory_is_refused() {
        let (dir, _) = overlay_fixture();
        fs::create_dir(dir.path().join("p/.hidden")).unwrap();
        let result = load_workspace_with(
            &mut RidlDatabase::default(),
            &dir.path().join("p"),
            &[overlay(
                dir.path().join("p/.hidden/f.typl"),
                "package p.hidden\n",
            )],
        );
        assert!(matches!(
            result,
            Err(LoadError::OverlayOutsideWorkspace {
                missing_directory: false,
                ..
            })
        ));
    }

    #[test]
    fn a_non_source_overlay_is_refused() {
        let (dir, _) = overlay_fixture();
        let result = load_workspace_with(
            &mut RidlDatabase::default(),
            &dir.path().join("p"),
            &[overlay(dir.path().join("p/ridl.toml"), "")],
        );
        assert!(matches!(result, Err(LoadError::OverlayNotSource(_))));
    }

    #[test]
    fn single_file_mode_takes_an_overlay_for_the_entry() {
        let dir = TempDir::new("overlay-single");
        let path = dir.write("x.typl", "package x\n");
        let text = "package x\ntype Other: integer [0..1]\n";
        let mut db = RidlDatabase::default();
        let loaded = load_workspace_with(&mut db, &path, &[overlay(path.clone(), text)]).unwrap();
        assert_eq!(
            loaded.workspace.packages(&db)[0].files(&db)[0].text(&db),
            text
        );
        let result = load_workspace_with(
            &mut db,
            &path,
            &[overlay(dir.path().join("y.typl"), "package y\n")],
        );
        assert!(matches!(
            result,
            Err(LoadError::OverlayOutsideWorkspace {
                missing_directory: false,
                ..
            })
        ));
    }

    #[cfg(unix)]
    #[test]
    fn overlay_matches_a_file_named_by_a_relative_entry() {
        let (dir, path) = overlay_fixture();
        let cwd = std::env::current_dir().unwrap();
        let mut relative = PathBuf::new();
        for part in cwd.components() {
            if matches!(part, std::path::Component::Normal(_)) {
                relative.push("..");
            }
        }
        relative.push(path.strip_prefix("/").unwrap());
        for (entry, overlay_path) in [(&relative, &path), (&path, &relative)] {
            let mut db = RidlDatabase::default();
            let text = "package p\ntype Other: integer [0..1]\n";
            let loaded =
                load_workspace_with(&mut db, entry, &[overlay(overlay_path.clone(), text)])
                    .unwrap();
            assert_eq!(
                loaded.workspace.packages(&db)[0].files(&db)[0].text(&db),
                text
            );
        }
        drop(dir);
    }

    #[test]
    fn load_workspace_without_overlays_is_unchanged() {
        let (dir, _) = overlay_fixture();
        let mut db = RidlDatabase::default();
        let first = load_workspace_with(&mut db, &dir.path().join("p"), &[]).unwrap();
        let describe = |db: &RidlDatabase, loaded: &LoadedWorkspace| {
            loaded
                .workspace
                .packages(db)
                .iter()
                .map(|p| {
                    (
                        p.name(db).clone(),
                        p.files(db)
                            .iter()
                            .map(|f| f.path(db).clone())
                            .collect::<Vec<_>>(),
                    )
                })
                .collect::<Vec<_>>()
        };
        let expected = describe(&db, &first);
        let mut other = RidlDatabase::default();
        let second = load_workspace(&mut other, &dir.path().join("p")).unwrap();
        assert_eq!(expected, describe(&other, &second));
    }

    const PACKAGE_MANIFEST: &str = "[package]\nname = \"veh.common\"\nversion = \"1.0.0\"\n";

    /// TYPL-010: a workspace member cannot declare a name the compiler
    /// provides (driftsys/ridl#203). Before this check the member compiled
    /// clean, its declarations were unreachable because every package already
    /// imports all of `ridl.std` implicitly, and its generated artifact was
    /// overwritten by the standard package's own.
    #[test]
    fn a_package_declaring_a_reserved_name_is_refused() {
        const SOURCE: &str = "package ridl.std\ntype MyOwnType: m\n";
        let dir = TempDir::new("reserved-name");
        dir.write(
            "ridl.toml",
            "[package]\nname = \"ridl.std\"\nversion = \"1.0.0\"\n",
        );
        dir.write("own.typl", SOURCE);

        let mut db = RidlDatabase::default();
        let loaded = load_workspace(&mut db, dir.path()).expect("the package loads");
        assert_eq!(
            codes(&loaded.diagnostics),
            vec!["TYPL-010"],
            "got: {:?}",
            loaded.diagnostics
        );
        let diagnostic = &loaded.diagnostics[0];
        assert!(
            diagnostic.message.contains("ridl.std"),
            "the message names the package: {}",
            diagnostic.message
        );
        // The message states only what holds for every input that draws it.
        // The artifact collision does not: in single-file mode the output base
        // is the file stem. Asserting the absence keeps that clause from
        // returning to the message without the test noticing.
        assert!(
            diagnostic.message.contains("unreachable"),
            "the message states the consequence that always holds: {}",
            diagnostic.message
        );
        for conditional in ["artifact", "overwrit"] {
            assert!(
                !diagnostic.message.contains(conditional),
                "`{conditional}` is true only where the output base is the package name, \
                 so it belongs in the catalogue entry, not the message: {}",
                diagnostic.message
            );
        }
        // The declaration is what is pointed at, not the manifest or the
        // directory: it is the one place a workspace member and single-file
        // mode both pass through. Both ends are asserted, against the source
        // itself, so an over-wide span covering the whole file cannot pass.
        let declaration = SOURCE
            .lines()
            .next()
            .expect("the declaration is the first line");
        assert_eq!(
            (
                usize::from(diagnostic.primary.range.start()),
                usize::from(diagnostic.primary.range.end()),
            ),
            (0, declaration.len()),
            "reported on `{declaration}` exactly"
        );
    }

    /// The same refusal in single-file mode — `ridlc build ridl_std.typl`,
    /// the second form driftsys/ridl#203 names. Single-file mode is exempt
    /// from TYPL-002, so nothing else would have caught it.
    #[test]
    fn a_single_file_declaring_a_reserved_name_is_refused() {
        let dir = TempDir::new("reserved-name-single");
        let file = dir.write("ridl_std.typl", "package ridl.std\ntype MyOwnType: m\n");

        let mut db = RidlDatabase::default();
        let loaded = load_workspace(&mut db, &file).expect("single-file mode loads");
        assert_eq!(
            codes(&loaded.diagnostics),
            vec!["TYPL-010"],
            "got: {:?}",
            loaded.diagnostics
        );
    }

    /// A name that merely starts with `ridl.` is not reserved: the reservation
    /// is the set of packages the compiler provides, not a namespace policy.
    #[test]
    fn only_a_compiler_provided_name_is_reserved() {
        let dir = TempDir::new("near-reserved");
        dir.write(
            "ridl.toml",
            "[package]\nname = \"ridl.stdlib\"\nversion = \"1.0.0\"\n",
        );
        dir.write("own.typl", "package ridl.stdlib\ntype MyOwnType: m\n");

        let mut db = RidlDatabase::default();
        let loaded = load_workspace(&mut db, dir.path()).expect("the package loads");
        assert_eq!(
            loaded.diagnostics,
            Vec::new(),
            "`ridl.stdlib` is not a package the compiler provides"
        );
    }

    /// (a) A two-file package loads, both files parse, and editing one
    /// re-parses only it — asserted by the re-executed query's `database_key`
    /// (issue #102).
    #[test]
    fn two_file_package_loads_and_edit_reparses_only_the_edited_file() {
        let dir = TempDir::new("two-file");
        dir.write("ridl.toml", PACKAGE_MANIFEST);
        dir.write("a.typl", "package veh.common\ntype A: m\n");
        dir.write("b.typl", "package veh.common\ntype B: s\n");

        let mut db = RidlDatabase::default();
        let loaded = load_workspace(&mut db, dir.path()).expect("the package loads");
        assert_eq!(
            loaded.diagnostics,
            Vec::new(),
            "a clean package, no diagnostics"
        );

        let packages = loaded.workspace.packages(&db).clone();
        assert_eq!(packages.len(), 1, "one package directory, one package");
        assert_eq!(packages[0].name(&db).as_str(), "veh.common");
        assert_eq!(*packages[0].origin(&db), PackageOrigin::WorkspaceMember);
        assert_eq!(
            packages[0].imports(&db),
            &BTreeMap::new(),
            "a manifest without `[imports]` yields an empty package map",
        );
        assert_eq!(
            loaded.workspace.imports(&db),
            &BTreeMap::new(),
            "a standalone load leaves the workspace map empty",
        );

        let files = packages[0].files(&db).clone();
        assert_eq!(files.len(), 2, "both .typl files load");
        for file in &files {
            assert_eq!(
                parse_file(&db, *file).errors(),
                &[],
                "both files parse clean"
            );
        }
        let a = files
            .iter()
            .copied()
            .find(|f| f.path(&db).ends_with("a.typl"))
            .expect("a.typl is loaded");
        let b = files
            .iter()
            .copied()
            .find(|f| f.path(&db).ends_with("b.typl"))
            .expect("b.typl is loaded");

        // Drain the executions the load itself ran; unchanged inputs are then
        // pure memo hits.
        db.take_executed_queries();
        let _ = parse_file(&db, a);
        let _ = parse_file(&db, b);
        assert_eq!(
            db.take_executed_queries(),
            Vec::new(),
            "re-querying unchanged inputs must run no executions",
        );

        // Edit A's text only: exactly one re-execution, and it is A's parse.
        a.set_text(&mut db)
            .to("package veh.common\ntype A: kg\n".to_string());
        let _ = parse_file(&db, a);
        let _ = parse_file(&db, b);
        let executed = db.take_executed_queries();
        assert_eq!(
            executed.len(),
            1,
            "editing one file re-parses exactly one file"
        );
        assert_eq!(
            salsa::attach(&db, || format!("{:?}", executed[0])),
            format!("parse_file({:?})", a.as_id()),
            "the re-executed query is the parse of the edited file",
        );
    }

    /// (b) A file that declares a different package than its directory
    /// requires is TYPL-002, primary span on the `package` line.
    #[test]
    fn typl_002_on_a_mismatching_file() {
        let dir = TempDir::new("mismatch");
        dir.write("ridl.toml", PACKAGE_MANIFEST);
        let bad_text = "package veh.wrong\ntype B: s\n";
        let bad_path = dir.write("bad.typl", bad_text);

        let mut db = RidlDatabase::default();
        let mut loaded = load_workspace(&mut db, dir.path()).expect("the package loads");
        assert_eq!(codes(&loaded.diagnostics), vec!["TYPL-002"]);

        let diag = &loaded.diagnostics[0];
        assert_eq!(diag.severity, Severity::Error);
        assert_eq!(
            diag.primary.range,
            byte_range(0, "package veh.wrong".len()),
            "the primary span is the mismatching `package` line",
        );
        assert_eq!(
            diag.primary.file,
            loaded.sources.file_id(&path_string(&bad_path), bad_text),
            "the span points into the mismatching file",
        );

        // The law is a diagnostic, not an exclusion: the file stays loaded.
        let packages = loaded.workspace.packages(&db).clone();
        assert_eq!(packages.len(), 1);
        assert_eq!(packages[0].files(&db).len(), 1);
    }

    /// (c) More than one `package` declaration in a file is TYPL-001 on each
    /// declaration after the first.
    #[test]
    fn typl_001_on_a_double_package_declaration() {
        let dir = TempDir::new("double-decl");
        dir.write("ridl.toml", PACKAGE_MANIFEST);
        dir.write(
            "dup.typl",
            "package veh.common\npackage veh.extra\ntype A: m\n",
        );

        let mut db = RidlDatabase::default();
        let loaded = load_workspace(&mut db, dir.path()).expect("the package loads");
        assert_eq!(codes(&loaded.diagnostics), vec!["TYPL-001"]);
        assert_eq!(
            loaded.diagnostics[0].primary.range,
            byte_range(19, 36),
            "the primary span is the second `package` declaration",
        );
    }

    /// (d) Workspace mode loads every member and keeps `[imports]` scoped per
    /// ADR-0002 §5: each member package carries only its own manifest's map
    /// (step 2 — a member's pin never leaks to a sibling), and the workspace
    /// map holds only the root's `[imports]` (step 3), never a merge.
    #[test]
    fn workspace_mode_scopes_imports_per_package() {
        let dir = TempDir::new("workspace");
        dir.write(
            "ridl.toml",
            "[workspace]\nmembers = [\"m-one\", \"m-two\"]\n\n[imports]\n\"third.dep\" = \"https://registry.example.com/third/dep@v1.0.0\"\n\"shared.util\" = \"https://registry.example.com/shared/util@v1.0.0\"\n",
        );
        dir.write(
            "m-one/ridl.toml",
            "[package]\nname = \"veh.one\"\nversion = \"1.0.0\"\n\n[imports]\n\"third.dep\" = \"https://mirror.example.com/third/dep@v2.0.0\"\n\"member.only\" = \"https://registry.example.com/member/only@v1.0.0\"\n",
        );
        dir.write("m-one/one.typl", "package veh.one\ntype A: m\n");
        dir.write(
            "m-two/ridl.toml",
            "[package]\nname = \"veh.two\"\nversion = \"1.0.0\"\n\n[imports]\n\"two.only\" = \"https://registry.example.com/two/only@v1.0.0\"\n",
        );
        dir.write("m-two/two.typl", "package veh.two\ntype B: s\n");

        let mut db = RidlDatabase::default();
        let loaded = load_workspace(&mut db, dir.path()).expect("the workspace loads");
        assert_eq!(loaded.diagnostics, Vec::new(), "a clean workspace");

        let packages = loaded.workspace.packages(&db).clone();
        let names: Vec<String> = packages.iter().map(|p| p.name(&db).clone()).collect();
        assert_eq!(
            names,
            vec!["veh.one", "veh.two"],
            "both members load, in member order"
        );

        // Step 3: the workspace map is the root's `[imports]`, un-merged —
        // the member pin for `third.dep` must NOT overwrite the root's.
        let workspace_imports = loaded.workspace.imports(&db).clone();
        assert_eq!(
            workspace_imports.get("third.dep").map(String::as_str),
            Some("https://registry.example.com/third/dep@v1.0.0"),
            "the workspace map keeps the root pin, not the member pin",
        );
        assert_eq!(
            workspace_imports.get("shared.util").map(String::as_str),
            Some("https://registry.example.com/shared/util@v1.0.0"),
        );
        assert_eq!(workspace_imports.len(), 2, "no member entry leaks upward");

        // Step 2: each member package carries its own manifest's map only.
        let one_imports = packages[0].imports(&db).clone();
        assert_eq!(
            one_imports.get("third.dep").map(String::as_str),
            Some("https://mirror.example.com/third/dep@v2.0.0"),
            "the member's own pin shadows the workspace default for it alone",
        );
        assert_eq!(
            one_imports.get("member.only").map(String::as_str),
            Some("https://registry.example.com/member/only@v1.0.0"),
        );
        assert!(
            !one_imports.contains_key("two.only"),
            "a sibling's pin never leaks into another member",
        );
        assert_eq!(one_imports.len(), 2, "no workspace entry is merged in");

        let two_imports = packages[1].imports(&db).clone();
        assert_eq!(
            two_imports.get("two.only").map(String::as_str),
            Some("https://registry.example.com/two/only@v1.0.0"),
        );
        assert!(
            !two_imports.contains_key("member.only"),
            "the sibling's pin never leaks into this member",
        );
        assert!(
            !two_imports.contains_key("third.dep"),
            "neither the root default nor the sibling's pin is merged in",
        );
        assert_eq!(two_imports.len(), 1);
    }

    /// `[defaults].timing` follows the ADR-0002 §5 precedence merged at load:
    /// a member's own `[defaults]` shadows the workspace `[defaults]`; a member
    /// without one inherits the workspace default (ridl §9.1).
    #[test]
    fn defaults_timing_precedence_package_shadows_workspace() {
        let dir = TempDir::new("defaults-timing");
        dir.write(
            "ridl.toml",
            "[workspace]\nmembers = [\"m-own\", \"m-inherit\"]\n\n[defaults]\ntiming = \"[100ms..1000ms]\"\n",
        );
        // m-own configures its own default — it shadows the workspace default.
        dir.write(
            "m-own/ridl.toml",
            "[package]\nname = \"veh.own\"\nversion = \"1.0.0\"\n\n[defaults]\ntiming = \"[50ms..2s]\"\n",
        );
        dir.write("m-own/own.typl", "package veh.own\ntype A: m\n");
        // m-inherit configures none — it inherits the workspace default.
        dir.write(
            "m-inherit/ridl.toml",
            "[package]\nname = \"veh.inherit\"\nversion = \"1.0.0\"\n",
        );
        dir.write("m-inherit/inherit.typl", "package veh.inherit\ntype B: s\n");

        let mut db = RidlDatabase::default();
        let loaded = load_workspace(&mut db, dir.path()).expect("the workspace loads");
        assert_eq!(loaded.diagnostics, Vec::new(), "a clean workspace");

        let packages = loaded.workspace.packages(&db).clone();
        let own = packages
            .iter()
            .find(|p| p.name(&db) == "veh.own")
            .expect("m-own loads");
        assert_eq!(
            own.defaults(&db).timing.as_deref(),
            Some("[50ms..2s]"),
            "the member's own `[defaults]` shadows the workspace default",
        );
        let inherit = packages
            .iter()
            .find(|p| p.name(&db) == "veh.inherit")
            .expect("m-inherit loads");
        assert_eq!(
            inherit.defaults(&db).timing.as_deref(),
            Some("[100ms..1000ms]"),
            "a member without `[defaults]` inherits the workspace default",
        );
    }

    /// Each `[defaults]` key resolves on its own: a member's value shadows the
    /// workspace's for that key only.
    #[test]
    fn defaults_precedence_is_per_key() {
        let dir = TempDir::new("defaults-per-key");
        dir.write(
            "ridl.toml",
            "[workspace]\nmembers = [\"m\"]\n\n[defaults]\ncommand_timing = \"[..2s]\"\nquery_timing = \"[..4s]\"\n",
        );
        dir.write(
            "m/ridl.toml",
            "[package]\nname = \"veh.m\"\nversion = \"1.0.0\"\n\n[defaults]\nquery_timing = \"[..5s]\"\n",
        );
        dir.write("m/m.typl", "package veh.m\ntype A: m\n");

        let mut db = RidlDatabase::default();
        let loaded = load_workspace(&mut db, dir.path()).expect("the workspace loads");
        assert_eq!(loaded.diagnostics, Vec::new(), "a clean workspace");
        let packages = loaded.workspace.packages(&db).clone();
        let member = packages
            .iter()
            .find(|p| p.name(&db) == "veh.m")
            .expect("the member loads");
        let defaults = member.defaults(&db);
        assert_eq!(defaults.command_timing.as_deref(), Some("[..2s]"));
        assert_eq!(defaults.query_timing.as_deref(), Some("[..5s]"));
        assert_eq!(defaults.timing, None);
    }

    /// Per-key precedence in the other direction: a member's `command_timing`
    /// is not overridden by the workspace's, and a member that leaves
    /// `query_timing` unset inherits the workspace's.
    #[test]
    fn defaults_precedence_is_per_key_in_both_directions() {
        let dir = TempDir::new("defaults-per-key-reverse");
        dir.write(
            "ridl.toml",
            "[workspace]\nmembers = [\"m\"]\n\n[defaults]\ncommand_timing = \"[..2s]\"\nquery_timing = \"[..4s]\"\n",
        );
        dir.write(
            "m/ridl.toml",
            "[package]\nname = \"veh.m\"\nversion = \"1.0.0\"\n\n[defaults]\ncommand_timing = \"[..7s]\"\n",
        );
        dir.write("m/m.typl", "package veh.m\ntype A: m\n");

        let mut db = RidlDatabase::default();
        let loaded = load_workspace(&mut db, dir.path()).expect("the workspace loads");
        assert_eq!(loaded.diagnostics, Vec::new(), "a clean workspace");
        let packages = loaded.workspace.packages(&db).clone();
        let member = packages
            .iter()
            .find(|p| p.name(&db) == "veh.m")
            .expect("the member loads");
        let defaults = member.defaults(&db);
        assert_eq!(
            defaults.command_timing.as_deref(),
            Some("[..7s]"),
            "the member's own `command_timing` wins over the workspace's",
        );
        assert_eq!(
            defaults.query_timing.as_deref(),
            Some("[..4s]"),
            "a member without `query_timing` inherits the workspace's",
        );
        assert_eq!(defaults.timing, None);
    }

    /// A standalone package's `command_timing` and `query_timing` ride on the
    /// root package and on a nested package directory alike (ridl §9.3).
    #[test]
    fn standalone_rpc_defaults_ride_on_the_tree() {
        let dir = TempDir::new("standalone-rpc-defaults");
        dir.write(
            "ridl.toml",
            "[package]\nname = \"veh.common\"\nversion = \"1.0.0\"\n\n[defaults]\ncommand_timing = \"[..2s]\"\nquery_timing = \"[5ms..4s]\"\n",
        );
        dir.write("a.typl", "package veh.common\ntype A: m\n");
        dir.write("sub/s.typl", "package veh.common.sub\ntype S: s\n");

        let mut db = RidlDatabase::default();
        let loaded = load_workspace(&mut db, dir.path()).expect("the package tree loads");
        assert_eq!(loaded.diagnostics, Vec::new());
        let packages = loaded.workspace.packages(&db).clone();
        let names: Vec<_> = packages.iter().map(|p| p.name(&db).clone()).collect();
        assert!(
            names.iter().any(|name| name == "veh.common")
                && names.iter().any(|name| name == "veh.common.sub"),
            "the root and the nested package both load: {names:?}",
        );
        for package in &packages {
            let defaults = package.defaults(&db);
            assert_eq!(
                defaults.command_timing.as_deref(),
                Some("[..2s]"),
                "{}",
                package.name(&db),
            );
            assert_eq!(
                defaults.query_timing.as_deref(),
                Some("[5ms..4s]"),
                "{}",
                package.name(&db),
            );
        }
    }

    /// A standalone package's `[defaults].timing` rides on every package in its
    /// directory tree, and single-file mode carries none (ridl §9.1).
    #[test]
    fn standalone_defaults_timing_rides_on_the_tree() {
        let dir = TempDir::new("standalone-defaults");
        dir.write(
            "ridl.toml",
            "[package]\nname = \"veh.common\"\nversion = \"1.0.0\"\n\n[defaults]\ntiming = \"[20ms..200ms]\"\n",
        );
        dir.write("a.typl", "package veh.common\ntype A: m\n");
        dir.write("sub/s.typl", "package veh.common.sub\ntype S: s\n");

        let mut db = RidlDatabase::default();
        let loaded = load_workspace(&mut db, dir.path()).expect("the package tree loads");
        assert_eq!(loaded.diagnostics, Vec::new());
        for package in loaded.workspace.packages(&db) {
            assert_eq!(
                package.defaults(&db).timing.as_deref(),
                Some("[20ms..200ms]"),
                "every package in the tree carries the manifest default",
            );
        }

        // Single-file mode: a bare file with no manifest anywhere up the tree.
        let bare = TempDir::new("standalone-defaults-bare");
        let single = bare.write("iface.ridl", "package solo\ntype A: m\n");
        let mut single_db = RidlDatabase::default();
        let single_loaded =
            load_workspace(&mut single_db, &single).expect("single-file mode loads");
        assert_eq!(
            single_loaded.workspace.packages(&single_db)[0].defaults(&single_db),
            &TimingDefaults::default(),
            "single-file mode carries no configured default",
        );
    }

    /// The lint scopes the loader builds in workspace mode: the root
    /// directory gets the defaults overlaid with the root `[lints]`, and
    /// each member directory gets the root levels overlaid with the member's
    /// own table (ADR-0002 §4, ADR-0024 decision 10). The scope keys share the
    /// path form of the file paths the loader records, so the path recorded
    /// for a member file resolves to the member's scope.
    #[test]
    fn lint_scopes_follow_root_then_member() {
        let dir = TempDir::new("lint-scopes");
        dir.write(
            "ridl.toml",
            "[workspace]\nmembers = [\"a\", \"b\"]\n\n[lints]\nmissing-timing = \"deny\"\nshared-error-type = \"allow\"\n",
        );
        dir.write(
            "a/ridl.toml",
            "[package]\nname = \"a\"\nversion = \"1.0.0\"\n\n[lints]\nmissing-timing = \"warn\"\n",
        );
        let a_file = dir.write("a/a.typl", "package a\ntype A: m\n");
        dir.write(
            "b/ridl.toml",
            "[package]\nname = \"b\"\nversion = \"1.0.0\"\n",
        );
        let b_file = dir.write("b/b.typl", "package b\ntype B: s\n");

        let mut db = RidlDatabase::default();
        let loaded = load_workspace(&mut db, dir.path()).expect("the workspace loads");
        assert_eq!(loaded.diagnostics, Vec::new(), "a clean workspace");

        let missing_timing = lint_by_name("missing-timing").expect("missing-timing is a lint");
        let shared_error_type =
            lint_by_name("shared-error-type").expect("shared-error-type is a lint");
        let levels = |path: &Path| {
            loaded
                .lints
                .for_path(path)
                .unwrap_or_else(|| panic!("`{}` is in a scope", path.display()))
        };

        let in_a = levels(&a_file);
        assert_eq!(in_a.level(missing_timing), Some(LintLevel::Warn));
        assert_eq!(in_a.level(shared_error_type), Some(LintLevel::Allow));
        assert_eq!(levels(&b_file).level(missing_timing), Some(LintLevel::Deny));
        let root = levels(&dir.path().join("ridl.toml"));
        assert_eq!(root.level(missing_timing), Some(LintLevel::Deny));
        assert_eq!(root.level(shared_error_type), Some(LintLevel::Allow));
        assert_eq!(
            levels(&dir.path().join("a/ridl.toml")).level(missing_timing),
            Some(LintLevel::Warn),
        );

        // The path the loader recorded for the member file, not one built by
        // hand, is in the member's scope.
        let packages = loaded.workspace.packages(&db).clone();
        let a = packages
            .iter()
            .find(|p| p.name(&db) == "a")
            .expect("member a loads");
        let recorded = a.files(&db)[0].path(&db).clone();
        assert_eq!(
            levels(Path::new(&recorded)).level(missing_timing),
            Some(LintLevel::Warn),
            "the recorded path `{recorded}` resolves to the member's scope",
        );
    }

    /// A manifest diagnostic's file path resolves to the scope of the
    /// manifest's own directory, so a member's `[lints]` table sets the level
    /// of the MANI-010 it causes (ADR-0024 decision 11).
    #[test]
    fn a_member_manifest_diagnostic_is_in_the_member_scope() {
        let dir = TempDir::new("lint-scope-manifest");
        dir.write(
            "ridl.toml",
            "[workspace]\nmembers = [\"a\"]\n\n[lints]\nunknown-lint = \"deny\"\n",
        );
        dir.write(
            "a/ridl.toml",
            "[package]\nname = \"a\"\nversion = \"1.0.0\"\n\n[lints]\nunknown-lint = \"allow\"\nnope = \"warn\"\n",
        );
        dir.write("a/a.typl", "package a\ntype A: m\n");

        let mut db = RidlDatabase::default();
        let loaded = load_workspace(&mut db, dir.path()).expect("the workspace loads");
        assert_eq!(codes(&loaded.diagnostics), vec!["MANI-010"]);
        let unknown_lint = lint_by_name("unknown-lint").expect("unknown-lint is a lint");
        let path = loaded
            .sources
            .path(loaded.diagnostics[0].primary.file)
            .expect("the manifest has a path");
        assert_eq!(
            loaded
                .lints
                .for_path(Path::new(path))
                .expect("the member manifest is in a scope")
                .level(unknown_lint),
            Some(LintLevel::Allow),
            "the recorded manifest path `{path}` resolves to the member's scope",
        );

        let mut diagnostics = loaded.diagnostics.clone();
        apply_lint_levels(&mut diagnostics, &loaded.sources, &loaded.lints);
        assert_eq!(
            diagnostics,
            Vec::new(),
            "`unknown-lint = \"allow\"` silences the MANI-010"
        );
    }

    /// A standalone package gets one scope for its directory, which covers
    /// every package in its tree; single-file mode gets none.
    #[test]
    fn standalone_lint_scope_covers_the_tree_and_single_file_has_none() {
        let dir = TempDir::new("lint-scope-standalone");
        dir.write(
            "ridl.toml",
            "[package]\nname = \"veh.common\"\nversion = \"1.0.0\"\n\n[lints]\nmissing-timing = \"deny\"\n",
        );
        let top = dir.write("a.typl", "package veh.common\ntype A: m\n");
        let nested = dir.write("sub/s.typl", "package veh.common.sub\ntype S: s\n");

        let mut db = RidlDatabase::default();
        let loaded = load_workspace(&mut db, dir.path()).expect("the package tree loads");
        assert_eq!(loaded.diagnostics, Vec::new());
        let missing_timing = lint_by_name("missing-timing").expect("missing-timing is a lint");
        for path in [&top, &nested] {
            assert_eq!(
                loaded
                    .lints
                    .for_path(path)
                    .unwrap_or_else(|| panic!("`{}` is in a scope", path.display()))
                    .level(missing_timing),
                Some(LintLevel::Deny),
            );
        }

        let bare = TempDir::new("lint-scope-bare");
        let single = bare.write("iface.ridl", "package solo\ntype A: m\n");
        let mut single_db = RidlDatabase::default();
        let single_loaded =
            load_workspace(&mut single_db, &single).expect("single-file mode loads");
        assert!(
            single_loaded.lints.for_path(&single).is_none(),
            "single-file mode has no lint scope",
        );
    }

    /// (e) A member manifest that declares `[workspace]` is a nested
    /// workspace: MANI-004, and the member loads nothing.
    #[test]
    fn mani_004_on_a_nested_workspace_member() {
        let dir = TempDir::new("nested");
        dir.write(
            "ridl.toml",
            "[workspace]\nmembers = [\"m-bad\", \"m-good\"]\n",
        );
        let bad_text = "[workspace]\nmembers = []\n";
        let bad_path = dir.write("m-bad/ridl.toml", bad_text);
        dir.write(
            "m-good/ridl.toml",
            "[package]\nname = \"veh.good\"\nversion = \"1.0.0\"\n",
        );
        dir.write("m-good/good.typl", "package veh.good\ntype A: m\n");

        let mut db = RidlDatabase::default();
        let mut loaded = load_workspace(&mut db, dir.path()).expect("the workspace loads");
        assert_eq!(codes(&loaded.diagnostics), vec!["MANI-004"]);

        let diag = &loaded.diagnostics[0];
        assert_eq!(diag.severity, Severity::Error);
        assert_eq!(
            diag.primary.file,
            loaded.sources.file_id(&path_string(&bad_path), bad_text),
            "the span points into the member's own manifest",
        );
        assert_eq!(
            diag.primary.range,
            byte_range(0, "[workspace]".len()),
            "the span is the `[workspace]` section header",
        );

        let packages = loaded.workspace.packages(&db).clone();
        let names: Vec<String> = packages.iter().map(|p| p.name(&db).clone()).collect();
        assert_eq!(names, vec!["veh.good"], "the nested member loads nothing");
    }

    /// A workspace member path with no manifest is MANI-008 and skipped; the
    /// rest of the workspace still loads.
    #[test]
    fn mani_008_on_a_missing_member_directory() {
        let dir = TempDir::new("missing-member");
        dir.write(
            "ridl.toml",
            "[workspace]\nmembers = [\"m-gone\", \"m-good\"]\n",
        );
        dir.write(
            "m-good/ridl.toml",
            "[package]\nname = \"veh.good\"\nversion = \"1.0.0\"\n",
        );
        dir.write("m-good/good.typl", "package veh.good\ntype A: m\n");

        let mut db = RidlDatabase::default();
        let loaded = load_workspace(&mut db, dir.path()).expect("the workspace loads");
        assert_eq!(codes(&loaded.diagnostics), vec!["MANI-008"]);
        assert_eq!(loaded.diagnostics[0].severity, Severity::Error);
        assert!(loaded.diagnostics[0].message.contains("m-gone"));

        let packages = loaded.workspace.packages(&db).clone();
        assert_eq!(packages.len(), 1);
        assert_eq!(packages[0].name(&db).as_str(), "veh.good");
    }

    /// A subdirectory of a package root is its own package, named by its
    /// directory path relative to the manifest root (ADR-0002 §1); every
    /// package in the tree carries the governing manifest's `[imports]`.
    #[test]
    fn a_subdirectory_is_its_own_package_named_by_its_path() {
        let dir = TempDir::new("subdir");
        dir.write(
            "ridl.toml",
            "[package]\nname = \"veh.common\"\nversion = \"1.0.0\"\n\n[imports]\n\"some.dep\" = \"https://registry.example.com/some/dep@v1.0.0\"\n",
        );
        dir.write("a.typl", "package veh.common\ntype A: m\n");
        dir.write("types/t.typl", "package veh.common.types\ntype T: s\n");

        let mut db = RidlDatabase::default();
        let loaded = load_workspace(&mut db, dir.path()).expect("the package tree loads");
        assert_eq!(loaded.diagnostics, Vec::new());

        let packages = loaded.workspace.packages(&db).clone();
        let names: Vec<String> = packages.iter().map(|p| p.name(&db).clone()).collect();
        assert_eq!(names, vec!["veh.common", "veh.common.types"]);
        for package in &packages {
            assert_eq!(
                package.imports(&db).get("some.dep").map(String::as_str),
                Some("https://registry.example.com/some/dep@v1.0.0"),
                "every package in the manifest's tree carries its `[imports]`",
            );
        }
        assert_eq!(
            loaded.workspace.imports(&db),
            &BTreeMap::new(),
            "a standalone load leaves the workspace map empty",
        );
    }

    /// A package directory may mix `.typl` and `.ridl`
    /// files — `.ridl` is accepted everywhere `.typl` is, under the same
    /// package↔directory law.
    #[test]
    fn a_package_directory_mixes_typl_and_ridl_files() {
        let dir = TempDir::new("mixed");
        dir.write("ridl.toml", PACKAGE_MANIFEST);
        dir.write("a.typl", "package veh.common\ntype A: m\n");
        dir.write("b.ridl", "package veh.common\ntype B: s\n");

        let mut db = RidlDatabase::default();
        let loaded = load_workspace(&mut db, dir.path()).expect("the package loads");
        assert_eq!(loaded.diagnostics, Vec::new(), "a clean mixed package");

        let packages = loaded.workspace.packages(&db).clone();
        assert_eq!(packages.len(), 1, "one package directory, one package");
        let files = packages[0].files(&db).clone();
        assert_eq!(files.len(), 2, "both the .typl and the .ridl file load");
        for file in &files {
            assert_eq!(
                parse_file(&db, *file).errors(),
                &[],
                "both files parse clean"
            );
        }
        assert!(files.iter().any(|f| f.path(&db).ends_with("a.typl")));
        assert!(files.iter().any(|f| f.path(&db).ends_with("b.ridl")));
    }

    /// A package directory loads its `.rsdl` files beside its `.typl` and
    /// `.ridl` files (rsdl reference §2: a package may hold all three), each
    /// parsed under the profile its extension selects.
    #[test]
    fn a_package_directory_loads_its_rsdl_files() {
        let dir = TempDir::new("rsdl");
        dir.write("ridl.toml", PACKAGE_MANIFEST);
        dir.write("a.typl", "package veh.common\ntype A: m\n");
        dir.write("b.ridl", "package veh.common\ntype B: s\n");
        dir.write("c.rsdl", "package veh.common\n");

        let mut db = RidlDatabase::default();
        let loaded = load_workspace(&mut db, dir.path()).expect("the package loads");
        assert_eq!(loaded.diagnostics, Vec::new(), "a clean mixed package");

        let packages = loaded.workspace.packages(&db).clone();
        assert_eq!(packages.len(), 1, "one package directory, one package");
        let files = packages[0].files(&db).clone();
        let paths: Vec<&str> = files.iter().map(|f| f.path(&db).as_str()).collect();
        assert_eq!(files.len(), 3, "the .rsdl file loads too: {paths:?}");
        let rsdl = files
            .iter()
            .find(|f| f.path(&db).ends_with("c.rsdl"))
            .expect("the .rsdl file is a package file");
        assert_eq!(
            crate::db::profile_of_path(rsdl.path(&db)),
            ridl_syntax::Profile::Rsdl
        );
        assert_eq!(
            parse_file(&db, *rsdl).errors(),
            &[],
            "the .rsdl file parses clean"
        );
    }

    /// Single-file mode accepts a bare `.ridl` entry, like a bare `.typl`.
    #[test]
    fn single_file_mode_accepts_a_bare_ridl_entry() {
        let dir = TempDir::new("single-ridl");
        let path = dir.write("iface.ridl", "package veh.iface\ntype A: m\n");

        let mut db = RidlDatabase::default();
        let loaded = load_workspace(&mut db, &path).expect("single-file mode loads");
        assert_eq!(loaded.diagnostics, Vec::new(), "exempt from TYPL-002");

        let packages = loaded.workspace.packages(&db).clone();
        assert_eq!(packages.len(), 1, "one synthetic package");
        assert_eq!(
            packages[0].name(&db).as_str(),
            "veh.iface",
            "named from the file's declared package",
        );
    }

    #[test]
    fn every_package_of_a_tree_carries_the_manifest_name_as_its_unit() {
        let dir = TempDir::new("unit-tree");
        dir.write(
            "ridl.toml",
            "[package]\nname = \"veh.hmi\"\nversion = \"1.0.0\"\n",
        );
        dir.write("hmi.ridl", "package veh.hmi\ntype A: m\n");
        dir.write("cluster/speed.ridl", "package veh.hmi.cluster\ntype S: m\n");

        let mut db = RidlDatabase::default();
        let loaded = load_workspace(&mut db, dir.path()).expect("the package tree loads");
        assert_eq!(loaded.diagnostics, Vec::new());

        let packages = loaded.workspace.packages(&db).clone();
        assert_eq!(packages.len(), 2);
        for package in &packages {
            assert_eq!(package.unit(&db).as_str(), "veh.hmi");
        }
        assert_eq!(
            loaded.units,
            BTreeMap::from([("veh.hmi".to_string(), dir.path().to_path_buf())]),
        );
    }

    #[test]
    fn a_workspace_member_is_a_unit_and_the_root_is_not() {
        let dir = TempDir::new("unit-members");
        dir.write("ridl.toml", "[workspace]\nmembers = [\"a\", \"b\"]\n");
        dir.write(
            "a/ridl.toml",
            "[package]\nname = \"x.a\"\nversion = \"1.0.0\"\n",
        );
        dir.write("a/a.ridl", "package x.a\ntype A: m\n");
        dir.write(
            "b/ridl.toml",
            "[package]\nname = \"x.b\"\nversion = \"1.0.0\"\n",
        );
        dir.write("b/b.ridl", "package x.b\ntype B: m\n");

        let mut db = RidlDatabase::default();
        let loaded = load_workspace(&mut db, dir.path()).expect("the workspace loads");
        assert_eq!(loaded.diagnostics, Vec::new());

        assert_eq!(
            loaded.units,
            BTreeMap::from([
                ("x.a".to_string(), dir.path().join("a")),
                ("x.b".to_string(), dir.path().join("b")),
            ]),
        );
        let units: Vec<String> = loaded
            .workspace
            .packages(&db)
            .iter()
            .map(|p| p.unit(&db).clone())
            .collect();
        assert_eq!(units, vec!["x.a", "x.b"]);
    }

    #[test]
    fn a_single_file_is_its_own_unit() {
        let dir = TempDir::new("unit-single");
        let path = dir.write("p.ridl", "package p\ntype A: m\n");

        let mut db = RidlDatabase::default();
        let loaded = load_workspace(&mut db, &path).expect("single-file mode loads");
        assert_eq!(loaded.diagnostics, Vec::new());

        let packages = loaded.workspace.packages(&db).clone();
        assert_eq!(packages.len(), 1);
        assert_eq!(packages[0].unit(&db).as_str(), "p");
        assert_eq!(
            loaded.units,
            BTreeMap::from([("p".to_string(), dir.path().to_path_buf())]),
        );
    }

    /// A symlinked directory is not followed by the tree walk — following it
    /// could revisit the tree in a cycle and duplicate packages endlessly.
    #[cfg(unix)]
    #[test]
    fn a_symlinked_directory_is_not_followed() {
        let dir = TempDir::new("symlink");
        dir.write("ridl.toml", PACKAGE_MANIFEST);
        dir.write("a.typl", "package veh.common\ntype A: m\n");
        // A symlink pointing back at the package root: a cycle.
        std::os::unix::fs::symlink(dir.path(), dir.path().join("loop"))
            .expect("create the directory symlink");

        let mut db = RidlDatabase::default();
        let loaded = load_workspace(&mut db, dir.path()).expect("the package loads");
        assert_eq!(loaded.diagnostics, Vec::new());

        let packages = loaded.workspace.packages(&db).clone();
        assert_eq!(packages.len(), 1, "the symlink cycle adds no packages");
        assert_eq!(packages[0].name(&db).as_str(), "veh.common");
    }

    /// A `.typl` file that is not valid UTF-8 becomes a diagnostic and is
    /// skipped; the rest of the package still loads (ADR-0004 §5 — never a
    /// hard error for content problems).
    #[test]
    fn a_non_utf8_file_is_reported_and_skipped() {
        let dir = TempDir::new("non-utf8");
        dir.write("ridl.toml", PACKAGE_MANIFEST);
        dir.write("a.typl", "package veh.common\ntype A: m\n");
        fs::write(dir.path().join("bad.typl"), [0xFF, 0xFE, 0x00, 0x9F])
            .expect("write the non-UTF8 fixture");

        let mut db = RidlDatabase::default();
        let loaded = load_workspace(&mut db, dir.path()).expect("the load continues");
        assert_eq!(loaded.diagnostics.len(), 1);
        assert!(
            loaded.diagnostics[0].message.contains("UTF-8"),
            "the diagnostic names the encoding problem",
        );

        let packages = loaded.workspace.packages(&db).clone();
        assert_eq!(packages.len(), 1);
        let files = packages[0].files(&db).clone();
        assert_eq!(files.len(), 1, "only the valid file loads");
        assert!(files[0].path(&db).ends_with("a.typl"));
    }

    /// (g) Single-file mode: a bare `.typl` file with no manifest up the tree
    /// loads as one synthetic package named from its declared package, exempt
    /// from TYPL-002. The E0 walking-skeleton fixture is the contract input.
    #[test]
    fn single_file_mode_loads_the_e0_fixture() {
        let fixture = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../ridl-syntax/fixtures/walking_skeleton.typl",
        );
        let text = fs::read_to_string(fixture).expect("the E0 fixture exists");
        assert!(
            text.contains("package fixtures"),
            "the fixture declares `package fixtures`",
        );

        // Copied into an empty temp dir so no manifest exists up the tree —
        // and the directory name never matches the declared package, which
        // proves the TYPL-002 exemption.
        let dir = TempDir::new("single-file");
        let path = dir.write("walking_skeleton.typl", &text);

        let mut db = RidlDatabase::default();
        let loaded = load_workspace(&mut db, &path).expect("single-file mode loads");
        assert_eq!(loaded.diagnostics, Vec::new(), "exempt from TYPL-002");

        let packages = loaded.workspace.packages(&db).clone();
        assert_eq!(packages.len(), 1, "one synthetic package");
        assert_eq!(
            packages[0].name(&db).as_str(),
            "fixtures",
            "named from the file's declared package",
        );
        assert_eq!(*packages[0].origin(&db), PackageOrigin::WorkspaceMember);

        let files = packages[0].files(&db).clone();
        assert_eq!(files.len(), 1);
        assert_eq!(
            parse_file(&db, files[0]).errors(),
            &[],
            "the fixture parses clean"
        );
    }

    // --- the interface lock (lock design §2, §8) --------------------------

    const LOCK_TEXT: &str = "\
# interfaces.lock — written by ridl lock; do not edit by hand.
next 3
Cabin 1
service:veh.common.climate 2
";

    /// A well-formed `interfaces.lock` beside the sources rides on the
    /// package: its path, its text and the parsed table (the text and path
    /// travel so a later checker diagnostic can point into the file).
    #[test]
    fn a_lock_beside_the_sources_rides_on_the_package() {
        let dir = TempDir::new("lock");
        dir.write("ridl.toml", PACKAGE_MANIFEST);
        dir.write("a.ridl", "package veh.common\ntype A: m\n");
        let lock_path = dir.write("interfaces.lock", LOCK_TEXT);

        let mut db = RidlDatabase::default();
        let loaded = load_workspace(&mut db, dir.path()).expect("the package loads");
        assert_eq!(
            loaded.diagnostics,
            Vec::new(),
            "a well-formed lock draws nothing"
        );

        let packages = loaded.workspace.packages(&db).clone();
        let lock = packages[0]
            .lock(&db)
            .as_ref()
            .expect("the lock rides on the package");
        assert_eq!(lock.path, path_string(&lock_path));
        assert_eq!(lock.text, LOCK_TEXT);
        assert_eq!(lock.lock.next, 3);
        assert_eq!(lock.lock.entries.len(), 2);
        assert_eq!(
            lock.lock.entries[1].key,
            crate::interface_lock::LockKey::Service("veh.common.climate".to_string())
        );
    }

    #[test]
    fn a_package_with_no_lock_has_none() {
        let dir = TempDir::new("no-lock");
        dir.write("ridl.toml", PACKAGE_MANIFEST);
        dir.write("a.ridl", "package veh.common\ntype A: m\n");

        let mut db = RidlDatabase::default();
        let loaded = load_workspace(&mut db, dir.path()).expect("the package loads");
        assert_eq!(loaded.diagnostics, Vec::new());
        let packages = loaded.workspace.packages(&db).clone();
        assert_eq!(*packages[0].lock(&db), None);
    }

    /// RIDL-410 is reported on the offending line of the lock file itself,
    /// through the loader's source map (PD-3), and the package then carries
    /// no lock.
    #[test]
    fn a_malformed_lock_is_ridl_410_on_its_own_line() {
        let dir = TempDir::new("bad-lock");
        dir.write("ridl.toml", PACKAGE_MANIFEST);
        dir.write("a.ridl", "package veh.common\ntype A: m\n");
        let text = "# interfaces.lock — written by ridl lock; do not edit by hand.\n\
                    next 2\n\
                    Cabin 1\n\
                    Door 1\n";
        let lock_path = dir.write("interfaces.lock", text);

        let mut db = RidlDatabase::default();
        let loaded = load_workspace(&mut db, dir.path()).expect("the package loads");
        assert_eq!(codes(&loaded.diagnostics), ["RIDL-410"]);
        let diagnostic = &loaded.diagnostics[0];
        assert_eq!(diagnostic.severity, Severity::Error);
        assert_eq!(
            loaded.sources.path(diagnostic.primary.file),
            Some(path_string(&lock_path).as_str()),
            "the span is in the lock file"
        );
        assert_eq!(loaded.sources.text(diagnostic.primary.file), Some(text));
        let line_start = text
            .find("Door 1")
            .expect("the offending line is in the text");
        assert_eq!(
            diagnostic.primary.range,
            byte_range(line_start, line_start + "Door 1".len()),
            "the span is the offending line"
        );
        assert_eq!(
            diagnostic.message,
            "`interfaces.lock` is malformed: number 1 is on two entries: `Cabin` and `Door` — \
             resolve the conflict or restore the file from version control, then run `ridl lock`"
        );

        let packages = loaded.workspace.packages(&db).clone();
        assert_eq!(
            *packages[0].lock(&db),
            None,
            "a malformed lock does not ride on the package"
        );
    }

    /// PD-3: an empty lock file, which has no `next` line, is reported at
    /// 0..0 of the file.
    #[test]
    fn an_empty_lock_is_ridl_410_at_the_start_of_the_file() {
        let dir = TempDir::new("empty-lock");
        dir.write("ridl.toml", PACKAGE_MANIFEST);
        dir.write("a.ridl", "package veh.common\ntype A: m\n");
        dir.write("interfaces.lock", "");

        let mut db = RidlDatabase::default();
        let loaded = load_workspace(&mut db, dir.path()).expect("the package loads");
        assert_eq!(codes(&loaded.diagnostics), ["RIDL-410"]);
        assert_eq!(loaded.diagnostics[0].primary.range, byte_range(0, 0));
        assert!(
            loaded.diagnostics[0].message.contains("no `next` line"),
            "got: {}",
            loaded.diagnostics[0].message
        );
    }

    /// A lock file that is not valid UTF-8 is RIDL-410 at 0..0 too — the
    /// treatment the loader gives a source file that is not valid UTF-8.
    #[test]
    fn a_lock_that_is_not_utf8_is_ridl_410() {
        let dir = TempDir::new("binary-lock");
        dir.write("ridl.toml", PACKAGE_MANIFEST);
        dir.write("a.ridl", "package veh.common\ntype A: m\n");
        fs::write(dir.path().join("interfaces.lock"), [0xff, 0xfe, b'\n'])
            .expect("write the bytes");

        let mut db = RidlDatabase::default();
        let loaded = load_workspace(&mut db, dir.path()).expect("the package loads");
        assert_eq!(codes(&loaded.diagnostics), ["RIDL-410"]);
        assert!(
            loaded.diagnostics[0].message.contains("not valid UTF-8"),
            "got: {}",
            loaded.diagnostics[0].message
        );
        assert_eq!(loaded.diagnostics[0].primary.range, byte_range(0, 0));
        let packages = loaded.workspace.packages(&db).clone();
        assert_eq!(*packages[0].lock(&db), None);
    }

    /// PD-8: a bare `.ridl` file with no manifest reads `interfaces.lock`
    /// from the file's directory.
    #[test]
    fn single_file_mode_reads_the_lock_beside_the_file() {
        let dir = TempDir::new("single-lock");
        let path = dir.write("iface.ridl", "package veh.iface\ntype A: m\n");
        dir.write("interfaces.lock", LOCK_TEXT);

        let mut db = RidlDatabase::default();
        let loaded = load_workspace(&mut db, &path).expect("single-file mode loads");
        assert_eq!(loaded.diagnostics, Vec::new());
        let packages = loaded.workspace.packages(&db).clone();
        let lock = packages[0]
            .lock(&db)
            .as_ref()
            .expect("the lock rides on the synthetic package");
        assert_eq!(lock.lock.next, 3);
    }

    /// The file is per unit: every package of the unit carries the lock of the
    /// manifest directory.
    #[test]
    fn every_package_of_a_unit_carries_the_lock_of_the_manifest_directory() {
        let dir = TempDir::new("unit-lock");
        dir.write(
            "ridl.toml",
            "[package]\nname = \"veh.hmi\"\nversion = \"1.0.0\"\n",
        );
        dir.write("hmi.ridl", "package veh.hmi\ntype A: m\n");
        let lock_path = dir.write("interfaces.lock", "next 1\n");
        dir.write("cluster/speed.ridl", "package veh.hmi.cluster\ntype B: m\n");

        let mut db = RidlDatabase::default();
        let loaded = load_workspace(&mut db, dir.path()).expect("the tree loads");
        assert_eq!(loaded.diagnostics, Vec::new());
        let packages = loaded.workspace.packages(&db).clone();
        assert_eq!(packages.len(), 2);
        for package in &packages {
            let lock = package
                .lock(&db)
                .as_ref()
                .unwrap_or_else(|| panic!("`{}` has the unit's lock", package.name(&db)));
            assert_eq!(lock.path, path_string(&lock_path));
        }
    }

    /// Only the lock beside the manifest is read: a lock in a subdirectory is
    /// ignored, even a malformed one, and reported as a warning.
    #[test]
    fn a_lock_in_a_subdirectory_is_not_read() {
        let dir = TempDir::new("subdir-lock-ignored");
        dir.write(
            "ridl.toml",
            "[package]\nname = \"veh.hmi\"\nversion = \"1.0.0\"\n",
        );
        dir.write("hmi.ridl", "package veh.hmi\ntype A: m\n");
        dir.write("cluster/speed.ridl", "package veh.hmi.cluster\ntype B: m\n");
        let ignored = dir.write("cluster/interfaces.lock", "x");

        let mut db = RidlDatabase::default();
        let loaded = load_workspace(&mut db, dir.path()).expect("the tree loads");
        assert_eq!(codes(&loaded.diagnostics), vec!["RIDL-416"]);
        let diag = &loaded.diagnostics[0];
        assert_eq!(diag.severity, Severity::Warning);
        assert_eq!(
            loaded.sources.path(diag.primary.file),
            Some(path_string(&ignored).as_str())
        );
        assert_eq!(diag.primary.range, byte_range(0, 0));
        assert!(diag.message.contains("unit `veh.hmi`"), "{}", diag.message);
        let packages = loaded.workspace.packages(&db).clone();
        assert_eq!(packages.len(), 2);
        for package in &packages {
            assert_eq!(*package.lock(&db), None);
        }
    }

    // Root discovery from a member (ADR-0002 §4, issue #529).

    const PACKAGE_A: &str = "[package]\nname = \"a\"\nversion = \"1.0.0\"\n";
    const PACKAGE_B: &str = "[package]\nname = \"b\"\nversion = \"1.0.0\"\n";

    #[test]
    fn find_root_walks_from_a_member_to_its_workspace() {
        let dir = TempDir::new("find-root-member");
        dir.write("ridl.toml", "[workspace]\nmembers = [\"a\", \"b\"]\n");
        dir.write("a/ridl.toml", PACKAGE_A);
        dir.write("a/src/a.typl", "package a.src\n");
        dir.write("b/ridl.toml", PACKAGE_B);

        assert_eq!(
            find_root(&dir.path().join("a/src")),
            Some(dir.path().to_path_buf())
        );
        assert_eq!(
            find_root(&dir.path().join("a")),
            Some(dir.path().to_path_buf())
        );
    }

    /// A member listed as `./a/` still names the package directory `a`.
    #[test]
    fn find_root_normalises_the_member_path() {
        let dir = TempDir::new("find-root-normalise");
        dir.write("ridl.toml", "[workspace]\nmembers = [\"./a/\"]\n");
        dir.write("a/ridl.toml", PACKAGE_A);

        assert_eq!(
            find_root(&dir.path().join("a")),
            Some(dir.path().to_path_buf())
        );
    }

    #[test]
    fn find_root_keeps_an_unlisted_package_standalone() {
        let dir = TempDir::new("find-root-unlisted");
        dir.write("ridl.toml", "[workspace]\nmembers = [\"b\"]\n");
        dir.write("a/ridl.toml", PACKAGE_A);
        dir.write("b/ridl.toml", PACKAGE_B);

        assert_eq!(find_root(&dir.path().join("a")), Some(dir.path().join("a")));
    }

    #[test]
    fn find_root_stops_at_the_first_workspace() {
        let dir = TempDir::new("find-root-first-workspace");
        dir.write("ridl.toml", "[workspace]\nmembers = [\"inner/a\"]\n");
        dir.write("inner/ridl.toml", "[workspace]\nmembers = [\"b\"]\n");
        dir.write("inner/a/ridl.toml", PACKAGE_A);

        assert_eq!(
            find_root(&dir.path().join("inner/a")),
            Some(dir.path().join("inner/a"))
        );
    }

    #[test]
    fn find_root_stops_at_a_git_directory() {
        let dir = TempDir::new("find-root-git");
        dir.write("ridl.toml", "[workspace]\nmembers = [\"repo/a\"]\n");
        fs::create_dir_all(dir.path().join("repo/.git")).expect("create .git");
        dir.write("repo/a/ridl.toml", PACKAGE_A);

        assert_eq!(
            find_root(&dir.path().join("repo/a")),
            Some(dir.path().join("repo/a"))
        );
    }

    /// The directory that holds `.git` is still checked for its own
    /// `ridl.toml` before the walk stops.
    #[test]
    fn find_root_checks_the_manifest_beside_a_git_directory() {
        let dir = TempDir::new("find-root-git-root");
        dir.write("ridl.toml", "[workspace]\nmembers = [\"a\"]\n");
        fs::create_dir_all(dir.path().join(".git")).expect("create .git");
        dir.write("a/ridl.toml", PACKAGE_A);

        assert_eq!(
            find_root(&dir.path().join("a")),
            Some(dir.path().to_path_buf())
        );
    }

    /// A manifest above the package that cannot be read stops the walk and is
    /// returned, so the loader reports why.
    #[test]
    fn find_root_returns_an_unreadable_manifest_above_the_package() {
        let dir = TempDir::new("find-root-unreadable");
        fs::write(dir.path().join("ridl.toml"), [0xff, 0xfe]).expect("write the manifest");
        dir.write("a/ridl.toml", PACKAGE_A);

        assert_eq!(
            find_root(&dir.path().join("a")),
            Some(dir.path().to_path_buf())
        );
    }

    /// A relative entry inside the current directory's member reaches the
    /// workspace above the current directory, and the root keeps the
    /// relative form.
    #[test]
    fn up_keeps_the_relative_form_of_the_entry() {
        assert_eq!(up(Path::new("members/a"), 2), PathBuf::from("."));
        assert_eq!(up(Path::new("a"), 1), PathBuf::from("."));
        assert_eq!(up(Path::new("a"), 2), PathBuf::from(".."));
        assert_eq!(up(Path::new("."), 2), PathBuf::from("../.."));
        assert_eq!(up(Path::new(""), 1), PathBuf::from(".."));
        assert_eq!(up(Path::new("a/.."), 1), PathBuf::from("a/../.."));
        assert_eq!(up(Path::new("/w/a"), 1), PathBuf::from("/w"));
    }

    #[test]
    fn member_entry_sets_report_scope() {
        let dir = TempDir::new("report-scope");
        dir.write("ridl.toml", "[workspace]\nmembers = [\"a\", \"b\"]\n");
        dir.write("a/ridl.toml", PACKAGE_A);
        dir.write("a/a.typl", "package a\ntype A: integer [0..1]\n");
        dir.write("b/ridl.toml", PACKAGE_B);
        let b_file = dir.write("b/b.typl", "package b\ntype B: integer [0..1]\n");

        let mut db = RidlDatabase::default();
        let loaded = load_workspace(&mut db, &dir.path().join("a")).expect("the workspace loads");
        assert_eq!(loaded.report_scope, Some(dir.path().join("a")));
        let mut names: Vec<_> = loaded
            .workspace
            .packages(&db)
            .iter()
            .map(|p| p.name(&db).clone())
            .collect();
        names.sort();
        assert_eq!(names, vec!["a", "b"], "both members compile");

        let mut db = RidlDatabase::default();
        let loaded = load_workspace(&mut db, &b_file).expect("the workspace loads");
        assert_eq!(loaded.report_scope, Some(dir.path().join("b")));

        let mut db = RidlDatabase::default();
        let loaded = load_workspace(&mut db, dir.path()).expect("the workspace loads");
        assert_eq!(loaded.report_scope, None, "the root reports on everything");
    }

    #[test]
    fn codegen_header_is_read_and_normalised() {
        let dir = TempDir::new("codegen-header");
        dir.write(
            "ridl.toml",
            &format!("{PACKAGE_A}\n[codegen]\nheader-file = \"H.txt\"\n"),
        );
        dir.write("a.typl", "package a\ntype A: integer [0..1]\n");
        dir.write("H.txt", "SPDX-License-Identifier: MIT\r\n");
        let mut db = RidlDatabase::default();
        let loaded = load_workspace(&mut db, dir.path()).expect("the package loads");
        assert!(loaded.diagnostics.is_empty(), "{:?}", loaded.diagnostics);
        assert_eq!(
            loaded.codegen_header.as_deref(),
            Some("SPDX-License-Identifier: MIT")
        );
    }

    #[test]
    fn codegen_header_file_missing_is_mani_011() {
        let dir = TempDir::new("codegen-header-missing");
        let manifest = format!("{PACKAGE_A}\n[codegen]\nheader-file = \"nope.txt\"\n");
        dir.write("ridl.toml", &manifest);
        dir.write("a.typl", "package a\ntype A: integer [0..1]\n");
        let mut db = RidlDatabase::default();
        let loaded = load_workspace(&mut db, dir.path()).expect("the package loads");
        assert_eq!(codes(&loaded.diagnostics), vec!["MANI-011"]);
        let diag = &loaded.diagnostics[0];
        let start = usize::from(diag.primary.range.start());
        let end = usize::from(diag.primary.range.end());
        assert_eq!(&manifest[start..end], "\"nope.txt\"");
        let resolved = dir.path().join("nope.txt");
        assert!(
            diag.message.contains(&resolved.display().to_string()),
            "{}",
            diag.message
        );
        assert_eq!(loaded.codegen_header, None);
    }

    #[test]
    fn codegen_header_file_that_is_not_utf8_is_mani_011() {
        let dir = TempDir::new("codegen-header-not-utf8");
        dir.write(
            "ridl.toml",
            &format!("{PACKAGE_A}\n[codegen]\nheader-file = \"H.txt\"\n"),
        );
        dir.write("a.typl", "package a\ntype A: integer [0..1]\n");
        fs::write(dir.path().join("H.txt"), [0xffu8, 0xfe]).expect("write the header");
        let mut db = RidlDatabase::default();
        let loaded = load_workspace(&mut db, dir.path()).expect("the package loads");
        assert_eq!(codes(&loaded.diagnostics), vec!["MANI-011"]);
        assert!(
            loaded.diagnostics[0].message.contains("H.txt"),
            "{}",
            loaded.diagnostics[0].message
        );
        assert_eq!(loaded.codegen_header, None);
    }

    #[test]
    fn codegen_header_file_with_a_control_character_is_mani_011() {
        let dir = TempDir::new("codegen-header-control");
        dir.write(
            "ridl.toml",
            &format!("{PACKAGE_A}\n[codegen]\nheader-file = \"H.txt\"\n"),
        );
        dir.write("a.typl", "package a\ntype A: integer [0..1]\n");
        dir.write("H.txt", "A\u{2028}B\u{0}\n");
        let mut db = RidlDatabase::default();
        let loaded = load_workspace(&mut db, dir.path()).expect("the package loads");
        assert_eq!(codes(&loaded.diagnostics), vec!["MANI-011"]);
        let diag = &loaded.diagnostics[0];
        let manifest = fs::read_to_string(dir.path().join("ridl.toml")).unwrap();
        let start = usize::from(diag.primary.range.start());
        let end = usize::from(diag.primary.range.end());
        assert_eq!(&manifest[start..end], "\"H.txt\"");
        let message = &diag.message;
        assert!(message.contains("control character"), "{message}");
        assert!(message.contains("U+2028"), "{message}");
        assert!(
            message.contains(&dir.path().join("H.txt").display().to_string()),
            "{message}"
        );
        assert_eq!(loaded.codegen_header, None);
    }

    #[test]
    fn codegen_header_file_in_a_member_is_mani_012() {
        let dir = TempDir::new("codegen-header-member");
        dir.write(
            "ridl.toml",
            "[workspace]\nmembers = [\"a\"]\n\n[codegen]\nheader-file = \"H.txt\"\n",
        );
        dir.write("H.txt", "root header\n");
        let member = format!("{PACKAGE_A}\n[codegen]\nheader-file = \"M.txt\"\n");
        dir.write("a/ridl.toml", &member);
        dir.write("a/a.typl", "package a\ntype A: integer [0..1]\n");
        let mut db = RidlDatabase::default();
        let loaded = load_workspace(&mut db, dir.path()).expect("the workspace loads");
        assert_eq!(codes(&loaded.diagnostics), vec!["MANI-012"]);
        let diag = &loaded.diagnostics[0];
        let start = usize::from(diag.primary.range.start());
        let end = usize::from(diag.primary.range.end());
        assert_eq!(&member[start..end], "\"M.txt\"");
        assert_eq!(loaded.codegen_header.as_deref(), Some("root header"));
    }

    #[test]
    fn a_manifest_below_a_unit_is_mani_013_and_its_tree_is_not_loaded() {
        let dir = TempDir::new("nested-manifest");
        dir.write(
            "ridl.toml",
            "[package]\nname = \"veh.hmi\"\nversion = \"1.0.0\"\n",
        );
        dir.write("hmi.ridl", "package veh.hmi\n");
        let nested = dir.write(
            "cluster/ridl.toml",
            "[package]\nname = \"veh.hmi.cluster\"\nversion = \"1.0.0\"\n",
        );
        dir.write("cluster/x.ridl", "package veh.hmi.cluster\n");
        let mut db = RidlDatabase::default();
        let loaded = load_workspace(&mut db, dir.path()).expect("the unit loads");
        assert_eq!(codes(&loaded.diagnostics), vec!["MANI-013"]);
        let diag = &loaded.diagnostics[0];
        assert_eq!(
            loaded.sources.path(diag.primary.file),
            Some(path_string(&nested).as_str())
        );
        assert_eq!(diag.primary.range, byte_range(0, 0));
        assert!(diag.message.contains("unit `veh.hmi`"), "{}", diag.message);
        let packages = loaded.workspace.packages(&db);
        assert_eq!(packages.len(), 1);
        assert_eq!(packages[0].name(&db), "veh.hmi");
    }

    #[test]
    fn a_member_whose_tree_holds_another_member_is_mani_013() {
        let dir = TempDir::new("nested-member");
        dir.write("ridl.toml", "[workspace]\nmembers = [\"a\", \"a/b\"]\n");
        dir.write(
            "a/ridl.toml",
            "[package]\nname = \"a\"\nversion = \"1.0.0\"\n",
        );
        dir.write("a/a.typl", "package a\n");
        dir.write(
            "a/b/ridl.toml",
            "[package]\nname = \"b\"\nversion = \"1.0.0\"\n",
        );
        dir.write("a/b/b.typl", "package b\n");
        let mut db = RidlDatabase::default();
        let loaded = load_workspace(&mut db, dir.path()).expect("the workspace loads");
        let count = codes(&loaded.diagnostics)
            .iter()
            .filter(|c| **c == "MANI-013")
            .count();
        assert_eq!(count, 1, "{:?}", codes(&loaded.diagnostics));
    }

    #[test]
    fn a_root_package_already_claimed_by_a_sibling_tree_is_mani_014() {
        let dir = TempDir::new("claimed-twice");
        dir.write("ridl.toml", "[workspace]\nmembers = [\"base\", \"hmi\"]\n");
        dir.write(
            "base/ridl.toml",
            "[package]\nname = \"com.example\"\nversion = \"1.0.0\"\n",
        );
        dir.write("base/hmi/x.ridl", "package com.example.hmi\n");
        let second = "[package]\nname = \"com.example.hmi\"\nversion = \"1.0.0\"\n";
        let second_path = dir.write("hmi/ridl.toml", second);
        dir.write("hmi/y.ridl", "package com.example.hmi\n");
        let mut db = RidlDatabase::default();
        let loaded = load_workspace(&mut db, dir.path()).expect("the workspace loads");
        assert_eq!(codes(&loaded.diagnostics), vec!["MANI-014"]);
        let diag = &loaded.diagnostics[0];
        assert_eq!(
            loaded.sources.path(diag.primary.file),
            Some(path_string(&second_path).as_str())
        );
        let start = usize::from(diag.primary.range.start());
        let end = usize::from(diag.primary.range.end());
        assert_eq!(&second[start..end], "\"com.example.hmi\"");
        assert!(
            diag.message.contains("unit `com.example`")
                && diag.message.contains("unit `com.example.hmi`"),
            "{}",
            diag.message
        );
        let packages = loaded.workspace.packages(&db);
        let claimed: Vec<_> = packages
            .iter()
            .filter(|p| p.name(&db) == "com.example.hmi")
            .collect();
        assert_eq!(claimed.len(), 1);
        assert_eq!(claimed[0].unit(&db), "com.example");
    }

    #[test]
    fn two_units_with_a_shared_prefix_and_no_overlap_both_load() {
        let dir = TempDir::new("shared-prefix");
        dir.write("ridl.toml", "[workspace]\nmembers = [\"base\", \"hmi\"]\n");
        dir.write(
            "base/ridl.toml",
            "[package]\nname = \"com.example\"\nversion = \"1.0.0\"\n",
        );
        dir.write("base/ids.typl", "package com.example\n");
        dir.write(
            "hmi/ridl.toml",
            "[package]\nname = \"com.example.hmi\"\nversion = \"1.0.0\"\n",
        );
        dir.write("hmi/y.ridl", "package com.example.hmi\n");
        let mut db = RidlDatabase::default();
        let loaded = load_workspace(&mut db, dir.path()).expect("the workspace loads");
        assert!(
            loaded.diagnostics.is_empty(),
            "{:?}",
            codes(&loaded.diagnostics)
        );
        let units: std::collections::BTreeSet<String> = loaded
            .workspace
            .packages(&db)
            .iter()
            .map(|p| p.unit(&db).to_string())
            .collect();
        assert_eq!(
            units,
            ["com.example", "com.example.hmi"].map(String::from).into()
        );
    }
}
