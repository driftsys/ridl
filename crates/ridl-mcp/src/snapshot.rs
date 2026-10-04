//! A stateless checked workspace and the locations its symbols refer to.
use crate::types::{Location, OverlayInput, Position, WorkspaceStatus};
use ridl_core::{InputFile, RidlDatabase, Severity};
use rmcp::model::{CallToolResult, ContentBlock};
use rowan::TextRange;
use std::path::{Path, PathBuf};

pub struct Snapshot {
    pub db: RidlDatabase,
    pub output: ridlc::WorkspaceOutput,
    pub root: PathBuf,
    pub notes: Vec<String>,
}

#[derive(Debug)]
pub enum ToolError {
    Request(String),
    WithData {
        message: String,
        data: serde_json::Value,
    },
}
impl ToolError {
    pub fn into_result(self) -> CallToolResult {
        match self {
            Self::Request(message) => CallToolResult::error(vec![ContentBlock::text(message)]),
            Self::WithData { message, mut data } => {
                let object = data.as_object_mut().expect("tool error data is an object");
                object.insert("message".into(), message.into());
                CallToolResult::structured_error(data)
            }
        }
    }
}

pub fn snapshot(path: &str, overlays: &[OverlayInput]) -> Result<Snapshot, ToolError> {
    let entry = Path::new(path);
    if !entry.exists() {
        return Err(ToolError::Request(format!("`{path}` does not exist")));
    }
    let overlays: Vec<_> = overlays
        .iter()
        .map(|overlay| ridl_core::Overlay {
            path: PathBuf::from(&overlay.path),
            text: overlay.source.clone(),
        })
        .collect();
    let mut db = RidlDatabase::default();
    let output = ridlc::compile_workspace_with(&mut db, entry, &overlays).map_err(|error| {
        let mut message = error.to_string();
        if matches!(&error, ridl_core::LoadError::Io(e) if e.kind() == std::io::ErrorKind::NotFound)
            && entry.is_dir()
        {
            message.push_str("; pass the workspace root, the directory that holds its ridl.toml");
        }
        ToolError::Request(message)
    })?;
    // An entry inside a workspace member loads the whole workspace and
    // reports on the member only, as `ridl check` does (ADR-0024 decision 9,
    // as ADR-0026 amends it). The lookup tools still see every member.
    let mut output = output;
    ridlc::retain_in_report_scope(
        &mut output.diagnostics,
        &output.sources,
        output.report_scope.as_deref(),
    );
    let root = if entry.is_file() {
        entry.parent().and_then(ridl_core::find_root)
    } else {
        ridl_core::find_root(entry)
    }
    .unwrap_or_else(|| entry.to_path_buf());
    let mut notes = Vec::new();
    if output.system.is_none()
        && output
            .sources
            .iter_files()
            .any(|(path, _)| path.ends_with(".rsdl"))
    {
        notes.push("rsdl uses were not counted, because no system was lowered: the workspace declares no `system`, or an error in its closure blocked the lowering; run ridl_check on the same path to see which".into());
    }
    Ok(Snapshot {
        db,
        output,
        root,
        notes,
    })
}
impl Snapshot {
    pub fn status(&self) -> WorkspaceStatus {
        WorkspaceStatus {
            root: self.root.to_string_lossy().into_owned(),
            errors: self
                .output
                .diagnostics
                .iter()
                .filter(|d| d.severity == Severity::Error)
                .count(),
            warnings: self
                .output
                .diagnostics
                .iter()
                .filter(|d| d.severity == Severity::Warning)
                .count(),
            notes: self.notes.clone(),
        }
    }
    pub fn location(&self, file: InputFile, range: TextRange) -> Location {
        let position = |offset| {
            let position = ridl_core::diag::line_col(file.text(&self.db), offset);
            Position {
                line: position.line,
                column: position.column,
            }
        };
        Location {
            path: file.path(&self.db).clone(),
            start: position(range.start()),
            end: position(range.end()),
        }
    }
}
#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::types::OverlayInput;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicUsize, Ordering};

    pub fn fixture(name: &str) -> String {
        format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"))
    }
    /// `snap` without its TYPL-406 (`missing-docs`) diagnostics, for a test
    /// that counts warnings over a fixture that leaves items undocumented.
    pub fn without_missing_docs(mut snap: Snapshot) -> Snapshot {
        snap.output
            .diagnostics
            .retain(|diagnostic| diagnostic.code.as_str() != "TYPL-406");
        snap
    }
    pub struct TempWorkspace(pub PathBuf);
    impl TempWorkspace {
        pub fn copy(name: &str) -> Self {
            static NEXT: AtomicUsize = AtomicUsize::new(0);
            let path = std::env::temp_dir().join(format!(
                "ridl-mcp-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fn copy(from: &Path, to: &Path) {
                fs::create_dir_all(to).unwrap();
                for entry in fs::read_dir(from).unwrap() {
                    let entry = entry.unwrap();
                    let target = to.join(entry.file_name());
                    if entry.path().is_dir() {
                        copy(&entry.path(), &target);
                    } else {
                        fs::copy(entry.path(), target).unwrap();
                    }
                }
            }
            copy(Path::new(&fixture(name)), &path);
            Self(path)
        }
    }
    impl Drop for TempWorkspace {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }
    #[test]
    fn snapshot_of_the_fixture_is_clean() {
        let snap = without_missing_docs(snapshot(&fixture("ws"), &[]).unwrap());
        let status = snap.status();
        assert_eq!(status.errors, 0);
        assert_eq!(status.warnings, 0);
        assert!(status.notes.is_empty());
        assert_eq!(
            snap.output
                .checked
                .iter()
                .map(|c| c.ir.name.as_str())
                .collect::<Vec<_>>(),
            ["fx.a", "fx.a.sub", "fx.b"]
        );
    }
    /// A member path loads its workspace (ADR-0002 §4): `fx.b`'s imports of
    /// its sibling `fx.a` resolve, no note says the package loaded alone, and
    /// the status root is the workspace root.
    #[test]
    fn a_member_path_resolves_a_sibling_import() {
        let snap = snapshot(&format!("{}/b", fixture("ws")), &[]).unwrap();
        assert!(snap.notes.is_empty(), "{:?}", snap.notes);
        assert_eq!(snap.status().errors, 0);
        assert_eq!(snap.root, PathBuf::from(fixture("ws")));
        assert!(snap.output.checked.iter().any(|c| c.ir.name == "fx.a"));
    }
    /// A member path reports only the member's diagnostics: an error in the
    /// sibling `a` is not counted when the path is `b`.
    #[test]
    fn a_member_path_reports_only_the_member() {
        let copy = TempWorkspace::copy("ws");
        fs::write(
            copy.0.join("a/broken.ridl"),
            "package fx.a\ntype Broken: Missing\n",
        )
        .unwrap();
        let from_root = snapshot(copy.0.to_str().unwrap(), &[]).unwrap();
        assert!(from_root.status().errors > 0);
        let from_b = snapshot(copy.0.join("b").to_str().unwrap(), &[]).unwrap();
        assert_eq!(from_b.status().errors, 0);
    }
    #[test]
    fn an_overlay_error_is_reported_and_disk_is_unchanged() {
        let path = format!("{}/b/b.ridl", fixture("ws"));
        let disk = fs::read_to_string(&path).unwrap();
        let snap = snapshot(
            &fixture("ws"),
            &[OverlayInput {
                path: path.clone(),
                source: disk.replace("signal speed: Speed @10ms", "signal speed: Missing @10ms"),
            }],
        )
        .unwrap();
        assert_eq!(snap.status().errors, 1);
        assert_eq!(fs::read_to_string(&path).unwrap(), disk);
        assert!(disk.contains("signal speed: Speed"));
    }
    #[test]
    fn an_empty_overlay_is_checked_like_an_empty_file() {
        let snap = snapshot(
            &fixture("ws"),
            &[OverlayInput {
                path: format!("{}/a/a.ridl", fixture("ws")),
                source: String::new(),
            }],
        )
        .unwrap();
        let copy = TempWorkspace::copy("ws");
        fs::write(copy.0.join("a/a.ridl"), "").unwrap();
        let output =
            ridlc::compile_workspace(&mut ridl_core::RidlDatabase::default(), &copy.0).unwrap();
        assert_eq!(
            snap.output
                .diagnostics
                .iter()
                .map(|d| d.code)
                .collect::<Vec<_>>(),
            output
                .diagnostics
                .iter()
                .map(|d| d.code)
                .collect::<Vec<_>>()
        );
        assert!(
            snap.output
                .checked
                .iter()
                .any(|c| c.ir.name == "fx.a.sub" && !c.ir.decls.is_empty())
        );
        assert!(snap.status().errors > 0);
        let found = crate::query::find(&snap, "fx.a.sub.Gear", None).unwrap();
        assert_eq!(found.package, "fx.a.sub");
        assert_eq!(found.item.name(), "Gear");
    }
    #[test]
    fn workspace_status_counts_errors_and_warnings_separately() {
        let snapshot = without_missing_docs(snapshot(&fixture("ws-diag"), &[]).unwrap());
        assert_eq!(snapshot.status().errors, 1);
        assert_eq!(snapshot.status().warnings, 1);
    }
    // `snapshot` does not apply the `[lints]` levels (ADR-0024
    // decision 8): the lookup tools share it and keep the emitted severities. The
    // fixture's root manifest sets `missing-timing = "deny"`, which only
    // `ridl_check` applies, so here RIDL-100 is still the Warning it was
    // emitted as.
    #[test]
    fn snapshot_keeps_the_emitted_severities() {
        let snapshot = snapshot(&fixture("ws-lints"), &[]).unwrap();
        let ridl_100 = snapshot
            .output
            .diagnostics
            .iter()
            .filter(|d| d.code.as_str() == "RIDL-100")
            .collect::<Vec<_>>();
        assert_eq!(ridl_100.len(), 1, "{:?}", snapshot.output.diagnostics);
        assert_eq!(ridl_100[0].severity, Severity::Warning);
        assert_eq!(snapshot.status().errors, 0);
        assert_eq!(snapshot.status().warnings, 1);
    }
    #[tokio::test]
    async fn a_remote_import_is_reported_and_not_fetched() {
        let copy = TempWorkspace::copy("ws");
        let manifest = copy.0.join("b/ridl.toml");
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let address = listener.local_addr().unwrap();
        fs::write(
            &manifest,
            format!(
                "{}\n[imports]\next = \"https://{address}/ext.git\"\n",
                fs::read_to_string(&manifest).unwrap()
            ),
        )
        .unwrap();
        let file = copy.0.join("b/b.ridl");
        fs::write(
            &file,
            fs::read_to_string(&file)
                .unwrap()
                .replace("package fx.b", "package fx.b\nimport ext.Thing"),
        )
        .unwrap();
        let path = copy.0.to_string_lossy().into_owned();
        let snap = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            tokio::task::spawn_blocking(move || snapshot(&path, &[])),
        )
        .await
        .unwrap()
        .unwrap()
        .unwrap();
        assert!(snap.output.diagnostics.iter().any(
            |d| d.severity == ridl_core::Severity::Info && d.message.contains("remote import")
        ));
        assert_eq!(
            listener.accept().unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock,
            "the compiler must not connect to the remote endpoint"
        );
        assert!(!copy.0.join("ridl.lock").exists());
        assert!(!copy.0.join(".ridl").exists());
    }

    pub fn name_location(relative: &str, declaration: &str, name: &str) -> serde_json::Value {
        let path = format!("{}/{relative}", fixture("ws"));
        let text = fs::read_to_string(&path).unwrap();
        let start = text.find(declaration).unwrap() + declaration.len() - name.len();
        let before = &text[..start];
        let line = before.bytes().filter(|b| *b == b'\n').count() + 1;
        let column = before.rsplit('\n').next().unwrap().chars().count() + 1;
        serde_json::json!({"path": path,
            "start": {"line": line, "column": column},
            "end": {"line": line, "column": column + name.chars().count()}})
    }
    /// A file in a workspace member has the workspace root as its root
    /// (ADR-0002 §4).
    #[test]
    fn snapshot_root_of_a_file_path() {
        let snap = snapshot(&format!("{}/b/b.ridl", fixture("ws")), &[]).unwrap();
        assert_eq!(snap.root, PathBuf::from(fixture("ws")));
    }
}
