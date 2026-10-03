//! A stateless checked workspace and the locations its symbols refer to.
use crate::types::{Location, OverlayInput, Position, WorkspaceStatus};
use ridl_core::{InputFile, ManifestKind, RidlDatabase, Severity};
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
    let root = if entry.is_file() {
        entry.parent().and_then(ridl_core::find_manifest_root)
    } else {
        ridl_core::find_manifest_root(entry)
    }
    .unwrap_or_else(|| entry.to_path_buf());
    let mut notes = Vec::new();
    fn kind(path: &Path) -> Option<ManifestKind> {
        let text = std::fs::read_to_string(path.join("ridl.toml")).ok()?;
        ridl_core::parse_manifest(ridl_core::FileId::DETACHED, &text)
            .0
            .map(|m| m.kind)
    }
    if matches!(kind(&root), Some(ManifestKind::Package { .. })) {
        let absolute = root
            .canonicalize()
            .map_err(|e| ToolError::Request(e.to_string()))?;
        for ancestor in absolute.ancestors().skip(1) {
            if matches!(kind(ancestor), Some(ManifestKind::Workspace { .. })) {
                notes.push(format!("loaded the package at `{}` alone, so imports of its sibling workspace members do not resolve (driftsys/ridl#529); pass the workspace root `{}` as `path` instead", root.display(), ancestor.display()));
                break;
            }
        }
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
    pub struct TempWorkspace(pub PathBuf);
    impl TempWorkspace {
        pub fn copy() -> Self {
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
            copy(Path::new(&fixture("ws")), &path);
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
        let snap = snapshot(&fixture("ws"), &[]).unwrap();
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
    #[test]
    fn a_member_path_draws_the_529_note() {
        let snap = snapshot(&format!("{}/b", fixture("ws")), &[]).unwrap();
        assert_eq!(snap.notes.len(), 1);
        assert!(snap.notes[0].contains("driftsys/ridl#529"));
        assert!(snap.notes[0].contains(&fixture("ws")));
        assert!(snap.status().errors > 0);
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
        let copy = TempWorkspace::copy();
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
    }
    #[test]
    fn workspace_status_counts_errors_and_warnings_separately() {
        let snapshot = snapshot(&fixture("ws-diag"), &[]).unwrap();
        assert_eq!(snapshot.status().errors, 1);
        assert_eq!(snapshot.status().warnings, 1);
    }
    #[tokio::test]
    async fn a_remote_import_is_reported_and_not_fetched() {
        let copy = TempWorkspace::copy();
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
}
