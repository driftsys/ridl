//! Filesystem discovery: from an entry path to a loaded [`Workspace`]
//! (docs/ROADMAP.md epic E1.3, ADR-0002 §1, §4–5).
//!
//! This is the only module in the crate that touches the filesystem, and it
//! sits behind the default-on `fs` feature (ADR-0007 decision 5) so the crate
//! still builds for `wasm32-unknown-unknown` with `--no-default-features`.
//!
//! [`load_workspace`] walks from an entry path — a `.typl` file, a package
//! directory, or a workspace root — reads the `ridl.toml` manifests, loads
//! every `.typl` file into [`InputFile`] inputs, and enforces the
//! package↔directory law (typl reference §3.1): every file in a package
//! directory must declare that directory's package name (TYPL-002), and more
//! than one `package` declaration in a file is TYPL-001. A bare `.typl` file
//! with no manifest anywhere up the tree loads in **single-file mode**: one
//! synthetic package named from the file's declared package, exempt from
//! TYPL-002 (the task 20 CLI contract).
//!
//! Problems in loaded content — manifest diagnostics, the law violations, a
//! nested workspace (MANI-004), a broken member (MANI-008), a file that is
//! not valid UTF-8 — are accumulated [`Diagnostic`]s, never an error return
//! (ADR-0004 §5). `std::io::Error` is reserved for real filesystem failures.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use ridl_syntax::ast::{AstNode as _, SourceFile};
use rowan::{TextRange, TextSize};

use crate::db::{InputFile, RidlDatabase, parse_file};
use crate::diag::{DiagCode, Diagnostic, FileId, Severity, SourceMap, Span};
use crate::manifest::{Manifest, ManifestKind, parse_manifest};
use crate::package::{Package, PackageOrigin, Workspace, package_declarations};

/// The result of [`load_workspace`]: the salsa [`Workspace`] input, the
/// diagnostics the load accumulated, and the interned path+text table the
/// diagnostics' [`Span`]s point into (what the caller hands to
/// [`render`](crate::diag::render)).
pub struct LoadedWorkspace {
    pub workspace: Workspace,
    pub diagnostics: Vec<Diagnostic>,
    pub sources: SourceMap,
}

/// Loads the workspace reachable from `entry` into `db`.
///
/// `entry` may be:
///
/// - a `.typl` file — the nearest `ridl.toml` up the tree is the root; with no
///   manifest anywhere up the tree the file loads in single-file mode;
/// - a package directory or workspace root — the nearest `ridl.toml` at or
///   above the directory is the root; a `[package]` manifest loads that
///   package's directory tree, a `[workspace]` manifest loads every member.
///
/// `[imports]` maps stay scoped per ADR-0002 §5: each [`Package`] carries the
/// `[imports]` of the manifest governing its directory tree (step 2), and
/// [`Workspace::imports`] holds only the workspace root's `[imports]` (step
/// 3, the shared default). Nothing is merged — a member's pin never leaks to
/// a sibling member; the task 9 resolver walks the order itself. In a
/// standalone package load the manifest's `[imports]` ride on its packages
/// and [`Workspace::imports`] is empty.
pub fn load_workspace(db: &mut RidlDatabase, entry: &Path) -> io::Result<LoadedWorkspace> {
    let mut loader = Loader::default();

    if entry.is_file() {
        match entry.parent().and_then(find_manifest_root) {
            Some(root) => loader.load_root(db, &root)?,
            None => loader.load_single_file(db, entry)?,
        }
    } else if entry.is_dir() {
        match find_manifest_root(entry) {
            Some(root) => loader.load_root(db, &root)?,
            None => {
                return Err(io::Error::new(
                    io::ErrorKind::NotFound,
                    format!("no `ridl.toml` found at or above `{}`", entry.display()),
                ));
            }
        }
    } else {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("`{}` does not exist", entry.display()),
        ));
    }

    let workspace = Workspace::new(&*db, loader.packages, loader.workspace_imports);
    Ok(LoadedWorkspace {
        workspace,
        diagnostics: loader.diagnostics,
        sources: loader.sources,
    })
}

/// The nearest directory at or above `dir` that contains a `ridl.toml`.
fn find_manifest_root(dir: &Path) -> Option<PathBuf> {
    dir.ancestors()
        .find(|candidate| candidate.join("ridl.toml").is_file())
        .map(Path::to_path_buf)
}

/// One loaded file plus its `package` declarations (dotted name, source
/// range), as [`Loader::load_file`] returns them.
type LoadedFile = (InputFile, Vec<(String, TextRange)>);

/// The accumulating state of one [`load_workspace`] run.
#[derive(Default)]
struct Loader {
    sources: SourceMap,
    diagnostics: Vec<Diagnostic>,
    packages: Vec<Package>,
    /// The workspace root's own `[imports]` (ADR-0002 §5 step 3). Stays empty
    /// in a standalone package load and in single-file mode.
    workspace_imports: BTreeMap<String, String>,
}

impl Loader {
    /// Loads from a directory known to contain a `ridl.toml`, in whichever
    /// mode its manifest declares.
    fn load_root(&mut self, db: &mut RidlDatabase, root: &Path) -> io::Result<()> {
        let manifest_path = root.join("ridl.toml");
        let text = fs::read_to_string(&manifest_path)?;
        let file_id = self.sources.file_id(&path_string(&manifest_path), &text);
        let (manifest, diags) = parse_manifest(file_id, &text);
        self.diagnostics.extend(diags);
        let Some(Manifest { kind, imports }) = manifest else {
            return Ok(());
        };
        match kind {
            ManifestKind::Package { name, .. } => {
                // A standalone package: the manifest's `[imports]` ride on its
                // packages; the workspace map stays empty.
                self.load_package_tree(db, root, &name, &imports)?;
            }
            ManifestKind::Workspace { members } => {
                // ADR-0002 §5 step 3: the workspace root's `[imports]` is the
                // shared default. Member maps are never merged into it.
                self.workspace_imports = imports;
                for member in &members {
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
        let Some(Manifest { kind, imports }) = manifest else {
            return Ok(());
        };
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
                // map, never visible to a sibling member.
                self.load_package_tree(db, &workspace_root.join(member), &name, &imports)?;
            }
        }
        Ok(())
    }

    /// Loads the package rooted at `dir` under the package name `name`, then
    /// every subdirectory as its own package named by its path — the
    /// package↔directory law's "the name mirrors the directory path relative
    /// to the manifest root" (ADR-0002 §1). Every package in the tree carries
    /// `imports`, the governing manifest's `[imports]`. Directories are
    /// visited in name order; hidden directories, symlinked directories
    /// (following them could revisit the tree in a cycle), and directories
    /// with their own `ridl.toml` (separate package roots) are skipped.
    fn load_package_tree(
        &mut self,
        db: &mut RidlDatabase,
        dir: &Path,
        name: &str,
        imports: &BTreeMap<String, String>,
    ) -> io::Result<()> {
        let mut typl_files = Vec::new();
        let mut subdirs = Vec::new();
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            let is_symlink = entry.file_type()?.is_symlink();
            if path.is_dir() {
                if !is_symlink {
                    subdirs.push(path);
                }
            } else if path.extension().is_some_and(|ext| ext == "typl") {
                typl_files.push(path);
            }
        }
        typl_files.sort();
        subdirs.sort();

        if !typl_files.is_empty() {
            let mut files = Vec::new();
            for path in &typl_files {
                if let Some((input, _)) = self.load_file(db, path, Some(name))? {
                    files.push(input);
                }
            }
            self.packages.push(Package::new(
                &*db,
                name.to_string(),
                files,
                PackageOrigin::WorkspaceMember,
                imports.clone(),
            ));
        }

        for subdir in subdirs {
            let Some(dir_name) = subdir.file_name().map(|n| n.to_string_lossy().into_owned())
            else {
                continue;
            };
            if dir_name.starts_with('.') || subdir.join("ridl.toml").is_file() {
                continue;
            }
            self.load_package_tree(db, &subdir, &format!("{name}.{dir_name}"), imports)?;
        }
        Ok(())
    }

    /// Loads one bare `.typl` file as a synthetic package named from its
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
        self.packages.push(Package::new(
            &*db,
            name,
            vec![input],
            PackageOrigin::WorkspaceMember,
            BTreeMap::new(),
        ));
        Ok(())
    }

    /// Reads one `.typl` file into an [`InputFile`], parses it through the
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
        let text = match fs::read_to_string(path) {
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

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use salsa::Setter;
    use salsa::plumbing::AsId;

    use super::*;

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

    const PACKAGE_MANIFEST: &str = "[package]\nname = \"veh.common\"\nversion = \"1.0.0\"\n";

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
}
