//! The synchronous server loop and its state (docs/ROADMAP.md epic E1.15a,
//! ADR-0004 §6).
//!
//! [`run`] follows the rust-analyzer `lsp-server` pattern: an initialize
//! handshake, then a plain loop that receives one message at a time and
//! dispatches it — no async runtime. Because the loop is strictly
//! sequential, a `$/cancelRequest` is dequeued only after older requests
//! were already answered; the cancelled-set check before each dispatch is
//! the hook the later, longer-running handlers (E1.15b–d) extend, and
//! salsa's own cancellation applies once queries run off-thread.
//!
//! The state model is the incremental overlay design described in the crate
//! docs: one workspace load, then `set_text` on the existing salsa
//! [`InputFile`]s per edit, with every recompute going through the memoized
//! `parse_file` / `resolve_package` / `check_package` / `service_catalog` /
//! `check_system` queries. The load runs at initialize from the client's
//! root. When that load fails, the server shows the error with
//! `window/showMessage`; when it fails or the client sent no root, the server
//! loads instead from the first opened file that has a `ridl.toml` at or above
//! it (issue #384). The nearest `ridl.toml` wins, so a file inside a member of
//! a `[workspace]` loads that member's package only.
//!
//! Two scope limits of this task, both by design:
//!
//! - The loader's own findings (manifest diagnostics and the
//!   package↔directory law, e.g. TYPL-002) are computed once at load time —
//!   the loader does not re-run per edit. They are published for files whose
//!   buffer still matches the loaded text and dropped once a file is edited.
//! - A load diagnostic whose file the load-time [`SourceMap`] cannot resolve
//!   back to a path (only a [`FileId::DETACHED`] span) is not published;
//!   `ridl check` still renders it.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::io;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use lsp_server::{Connection, ErrorCode, Message, Notification, Request, RequestId, Response};
use lsp_types as lt;
use lsp_types::notification::Notification as _;
use lsp_types::request::Request as _;
use ridl_core::db::{InputFile, RidlDatabase, parse_file};
use ridl_core::diag::{
    DiagCode, Diagnostic, FileId, Severity, SourceMap, Span, house_style_message, remap_diagnostics,
};
use ridl_core::package::{Package, PackageOrigin, Workspace};
use ridl_core::{
    LoadedWorkspace, find_manifest_root, load_workspace, profile_of_path, std_package,
};
use ridl_sem::{
    CheckedWorkspace, check_package, check_workspace, resolve_package, unclaimed_backend_keys,
};
use ridl_syntax::Profile;
use ridl_syntax::ast::{AstNode as _, SourceFile};
use rowan::TextRange;
use salsa::Setter as _;

use crate::convert::{self, LineIndex};
use crate::{complete, hover, inlay, nav, rename, rsdl};

type Error = Box<dyn std::error::Error + Send + Sync>;

/// Runs the server over `connection` until the client shuts it down: the
/// initialize handshake (including the `initialized` notification), the
/// workspace load from the client's root, the initial diagnostics publish,
/// then the message loop.
pub fn run(connection: Connection) -> Result<(), Error> {
    run_with_version(connection, None)
}

/// [`run`], reporting `version` in the initialize result's `serverInfo` when
/// one is given — the `ridl` binary passes its own build version here.
///
/// This drives the handshake through `initialize_start`/`initialize_finish`
/// rather than the single-call `Connection::initialize`, because that method
/// hardcodes the result to `{"capabilities": ...}` with no room for
/// `serverInfo`. `server_capabilities()` is still serialized through an
/// explicit, fallible `serde_json::to_value` call rather than embedded
/// directly in the `json!` macro below, so a serialization failure is
/// propagated as an error, matching what the single-call form did, rather
/// than reaching the macro's internal `.unwrap()` for interpolated values.
pub fn run_with_version(connection: Connection, version: Option<&str>) -> Result<(), Error> {
    let (id, params) = connection.initialize_start()?;
    let params: lt::InitializeParams = serde_json::from_value(params)?;
    let capabilities = serde_json::to_value(server_capabilities())?;
    let mut server_info = serde_json::json!({ "name": "ridl-lsp" });
    if let Some(version) = version {
        server_info["version"] = serde_json::Value::String(version.to_string());
    }
    let result = serde_json::json!({
        "capabilities": capabilities,
        "serverInfo": server_info,
    });
    connection.initialize_finish(id, result)?;
    let mut state = ServerState::new();
    if let Some(root) = workspace_root(&params)
        && let Err(err) = state.load(&root)
    {
        show_message(
            &connection,
            lt::MessageType::WARNING,
            format!(
                "ridl-lsp loaded no workspace from `{}`: {err}. It loads the workspace of \
                 the first opened file that has a `ridl.toml` at or above it; until then, \
                 each open file is checked on its own.",
                root.display()
            ),
        )?;
    }
    state.publish_all(&connection)?;
    main_loop(connection, state)
}

/// The capability set: incremental text sync with open/close notifications,
/// quick-fix code actions (E1.15a), hover, goto-definition, and find-references
/// (E1.15b), completion and rename (E1.15c), and inlay hints (E1.16). Rename
/// advertises `prepareProvider` so the client validates the cursor and the new
/// name before applying an edit. Inlay hints close the E1 LSP feature set.
fn server_capabilities() -> lt::ServerCapabilities {
    lt::ServerCapabilities {
        text_document_sync: Some(lt::TextDocumentSyncCapability::Options(
            lt::TextDocumentSyncOptions {
                open_close: Some(true),
                change: Some(lt::TextDocumentSyncKind::INCREMENTAL),
                ..Default::default()
            },
        )),
        code_action_provider: Some(lt::CodeActionProviderCapability::Options(
            lt::CodeActionOptions {
                code_action_kinds: Some(vec![lt::CodeActionKind::QUICKFIX]),
                ..Default::default()
            },
        )),
        hover_provider: Some(lt::HoverProviderCapability::Simple(true)),
        definition_provider: Some(lt::OneOf::Left(true)),
        references_provider: Some(lt::OneOf::Left(true)),
        completion_provider: Some(lt::CompletionOptions {
            // `.` completes an import path; `:` a type position. Identifier
            // characters need not be listed — the client triggers on those.
            trigger_characters: Some(vec![":".to_string(), ".".to_string()]),
            ..Default::default()
        }),
        rename_provider: Some(lt::OneOf::Right(lt::RenameOptions {
            prepare_provider: Some(true),
            work_done_progress_options: Default::default(),
        })),
        inlay_hint_provider: Some(lt::OneOf::Left(true)),
        ..Default::default()
    }
}

/// The workspace root directory: the first workspace folder, falling back to
/// the deprecated `rootUri` for clients that send only that.
fn workspace_root(params: &lt::InitializeParams) -> Option<PathBuf> {
    if let Some(folder) = params.workspace_folders.as_ref().and_then(|f| f.first()) {
        return convert::uri_to_path(&folder.uri).map(PathBuf::from);
    }
    #[allow(deprecated)]
    params
        .root_uri
        .as_ref()
        .and_then(convert::uri_to_path)
        .map(PathBuf::from)
}

/// Receives and dispatches messages until the client shuts the server down
/// or the connection closes.
fn main_loop(connection: Connection, mut state: ServerState) -> Result<(), Error> {
    for message in &connection.receiver {
        match message {
            Message::Request(request) => {
                if connection.handle_shutdown(&request)? {
                    return Ok(());
                }
                let response = if state.cancelled.remove(&request.id) {
                    Response::new_err(
                        request.id,
                        ErrorCode::RequestCanceled as i32,
                        "the request was cancelled".to_string(),
                    )
                } else {
                    state.dispatch_request(request)
                };
                connection.sender.send(response.into())?;
            }
            Message::Notification(notification) => {
                state.dispatch_notification(notification, &connection)?;
            }
            // The server sends no requests of its own yet, so no responses
            // arrive.
            Message::Response(_) => {}
        }
    }
    Ok(())
}

/// One quick fix from the latest analysis: the range of the diagnostic it
/// fixes (for the code-action range filter) plus the ready-made action.
struct QuickFix {
    range: lt::Range,
    action: lt::CodeAction,
}

/// A batch conversion result: LSP diagnostics and quick fixes, grouped by
/// the primary span's file path.
#[derive(Default)]
struct Batch {
    diagnostics: BTreeMap<String, Vec<lt::Diagnostic>>,
    fixes: HashMap<String, Vec<QuickFix>>,
}

/// The server's whole state: the salsa database with the one loaded
/// workspace, the overlay bookkeeping, and the latest publish results.
struct ServerState {
    db: RidlDatabase,
    /// The embedded `ridl.std` package, threaded into every resolve/check.
    std: Package,
    /// The one `Workspace` input: loaded at initialize from the client's
    /// root, or, when the client sent no root or that load failed, at the
    /// first `didOpen` of a file that has a `ridl.toml` at or above it. Empty
    /// until then.
    workspace: Workspace,
    /// Whether [`ServerState::load`] has succeeded. Once it has, the
    /// workspace is not loaded again; a file outside it is an overlay.
    loaded: bool,
    /// The manifest directory and reason of the last load error a `didOpen`
    /// showed, so the same error is not shown again on every later
    /// `didOpen`.
    shown_load_error: Option<String>,
    /// Every loaded workspace file, keyed by its load-time path string —
    /// the inputs `didOpen`/`didChange` overlay via `set_text`.
    files: HashMap<String, InputFile>,
    /// The package each loaded workspace file belongs to, keyed by the same
    /// path — the map hover and navigation use to find a file's package.
    file_package: HashMap<String, Package>,
    /// Open files outside the loaded workspace: a fresh overlay input
    /// wrapped in a synthetic single-file package.
    overlays: HashMap<String, (InputFile, Package)>,
    /// Per-file line tables, keyed by path, for the position arithmetic hover
    /// and navigation need. Built on first use and invalidated on `didChange`
    /// (the only event that changes a file's text).
    line_indexes: HashMap<String, Rc<LineIndex>>,
    /// The load-time loader findings per path, converted once; dropped per
    /// file when its buffer diverges from the loaded text (see module docs).
    loader_diagnostics: BTreeMap<String, Vec<lt::Diagnostic>>,
    /// Paths whose buffer text diverged from the loaded text.
    edited: HashSet<String>,
    /// Paths the last publish sent a non-empty list for — the set that gets
    /// an explicit empty publish once a file turns clean.
    published: HashSet<String>,
    /// Quick fixes per path from the latest analysis.
    fixes: HashMap<String, Vec<QuickFix>>,
    /// Requests cancelled by `$/cancelRequest` and not yet dispatched.
    cancelled: HashSet<RequestId>,
}

impl ServerState {
    /// A state with no workspace loaded yet: the embedded `ridl.std` package
    /// and an empty `Workspace` input, which [`ServerState::load`] replaces.
    fn new() -> ServerState {
        let mut db = RidlDatabase::default();
        let std = std_package(&mut db);
        let workspace = Workspace::new(&db, Vec::new(), BTreeMap::new());
        ServerState {
            db,
            std,
            workspace,
            loaded: false,
            shown_load_error: None,
            files: HashMap::new(),
            file_package: HashMap::new(),
            overlays: HashMap::new(),
            line_indexes: HashMap::new(),
            loader_diagnostics: BTreeMap::new(),
            edited: HashSet::new(),
            published: HashSet::new(),
            fixes: HashMap::new(),
            cancelled: HashSet::new(),
        }
    }

    /// Loads the workspace whose `ridl.toml` is at or above `dir` — the only
    /// cold, from-disk load in the server's lifetime; every later recompute
    /// reuses these inputs. On an error no field of the state changes; the
    /// salsa inputs the failed load created stay in the database, unused.
    ///
    /// An open overlay whose path the loaded workspace contains (a file
    /// opened before its `ridl.toml` existed) moves its buffer onto the
    /// loaded input and stops being an overlay, so the file is analyzed once,
    /// as a member of its package.
    fn load(&mut self, dir: &Path) -> io::Result<()> {
        let LoadedWorkspace {
            workspace,
            diagnostics,
            sources,
        } = load_workspace(&mut self.db, dir)?;
        for package in workspace.packages(&self.db) {
            for file in package.files(&self.db) {
                self.files.insert(file.path(&self.db).clone(), *file);
                self.file_package
                    .insert(file.path(&self.db).clone(), *package);
            }
        }
        self.loader_diagnostics =
            convert_loader_diagnostics(&self.db, &self.files, diagnostics, &sources);
        self.workspace = workspace;
        self.loaded = true;

        let joined: Vec<String> = self
            .overlays
            .keys()
            .filter(|path| self.files.contains_key(*path))
            .cloned()
            .collect();
        for path in joined {
            let Some((overlay, _)) = self.overlays.remove(&path) else {
                continue;
            };
            let text = overlay.text(&self.db).clone();
            let input = self.files[&path];
            // The cached line table was built for the overlay input.
            self.line_indexes.remove(&path);
            self.set_text(path, input, text);
        }
        Ok(())
    }

    /// Before a `didOpen` with no workspace loaded yet: loads the workspace
    /// of the nearest `ridl.toml` at or above the opened file (issue #384).
    /// No manifest there is not an error — the file becomes a standalone
    /// overlay, and the next `didOpen` tries again. A load from a manifest
    /// that fails is shown to the user. A manifest that stays unreadable
    /// fails again on each `didOpen`; its error is shown only when it differs
    /// from the last one shown, by manifest directory or by reason.
    fn load_for_opened_file(&mut self, path: &str, connection: &Connection) -> Result<(), Error> {
        if self.loaded {
            return Ok(());
        }
        let Some(dir) = Path::new(path).parent() else {
            return Ok(());
        };
        let Some(root) = find_manifest_root(dir) else {
            return Ok(());
        };
        let Err(err) = self.load(&root) else {
            return Ok(());
        };
        let shown = format!("{}: {err}", root.display());
        if self.shown_load_error.as_ref() == Some(&shown) {
            return Ok(());
        }
        let message = format!("ridl-lsp could not load the workspace of `{path}`: {err}");
        self.shown_load_error = Some(shown);
        show_message(connection, lt::MessageType::ERROR, message)
    }

    /// Handles one request; shutdown and cancellation were already handled
    /// by the loop.
    fn dispatch_request(&mut self, request: Request) -> Response {
        match request.method.as_str() {
            lt::request::CodeActionRequest::METHOD => {
                match serde_json::from_value::<lt::CodeActionParams>(request.params) {
                    Ok(params) => Response::new_ok(request.id, self.code_actions(&params)),
                    Err(err) => Response::new_err(
                        request.id,
                        ErrorCode::InvalidParams as i32,
                        err.to_string(),
                    ),
                }
            }
            lt::request::HoverRequest::METHOD => {
                match serde_json::from_value::<lt::HoverParams>(request.params) {
                    Ok(params) => Response::new_ok(request.id, self.hover(&params)),
                    Err(err) => Response::new_err(
                        request.id,
                        ErrorCode::InvalidParams as i32,
                        err.to_string(),
                    ),
                }
            }
            lt::request::GotoDefinition::METHOD => {
                match serde_json::from_value::<lt::GotoDefinitionParams>(request.params) {
                    Ok(params) => Response::new_ok(request.id, self.goto_definition(&params)),
                    Err(err) => Response::new_err(
                        request.id,
                        ErrorCode::InvalidParams as i32,
                        err.to_string(),
                    ),
                }
            }
            lt::request::References::METHOD => {
                match serde_json::from_value::<lt::ReferenceParams>(request.params) {
                    Ok(params) => Response::new_ok(request.id, self.references(&params)),
                    Err(err) => Response::new_err(
                        request.id,
                        ErrorCode::InvalidParams as i32,
                        err.to_string(),
                    ),
                }
            }
            lt::request::Completion::METHOD => {
                match serde_json::from_value::<lt::CompletionParams>(request.params) {
                    Ok(params) => Response::new_ok(request.id, self.completion(&params)),
                    Err(err) => Response::new_err(
                        request.id,
                        ErrorCode::InvalidParams as i32,
                        err.to_string(),
                    ),
                }
            }
            lt::request::PrepareRenameRequest::METHOD => {
                match serde_json::from_value::<lt::TextDocumentPositionParams>(request.params) {
                    Ok(params) => Response::new_ok(request.id, self.prepare_rename(&params)),
                    Err(err) => Response::new_err(
                        request.id,
                        ErrorCode::InvalidParams as i32,
                        err.to_string(),
                    ),
                }
            }
            lt::request::Rename::METHOD => {
                match serde_json::from_value::<lt::RenameParams>(request.params) {
                    Ok(params) => match self.rename(&params) {
                        Ok(edit) => Response::new_ok(request.id, edit),
                        Err(err) => Response::new_err(
                            request.id,
                            ErrorCode::RequestFailed as i32,
                            err.message(),
                        ),
                    },
                    Err(err) => Response::new_err(
                        request.id,
                        ErrorCode::InvalidParams as i32,
                        err.to_string(),
                    ),
                }
            }
            lt::request::InlayHintRequest::METHOD => {
                match serde_json::from_value::<lt::InlayHintParams>(request.params) {
                    Ok(params) => Response::new_ok(request.id, self.inlay_hints(&params)),
                    Err(err) => Response::new_err(
                        request.id,
                        ErrorCode::InvalidParams as i32,
                        err.to_string(),
                    ),
                }
            }
            method => Response::new_err(
                request.id,
                ErrorCode::MethodNotFound as i32,
                format!("unsupported method `{method}`"),
            ),
        }
    }

    /// Handles one notification. Document mutations re-analyze and republish;
    /// everything the server does not track is ignored.
    fn dispatch_notification(
        &mut self,
        notification: Notification,
        connection: &Connection,
    ) -> Result<(), Error> {
        match notification.method.as_str() {
            lt::notification::DidOpenTextDocument::METHOD => {
                let params: lt::DidOpenTextDocumentParams =
                    serde_json::from_value(notification.params)?;
                let Some(path) = convert::uri_to_path(&params.text_document.uri) else {
                    return Ok(());
                };
                self.load_for_opened_file(&path, connection)?;
                self.open(path, params.text_document.text);
                self.publish_all(connection)
            }
            lt::notification::DidChangeTextDocument::METHOD => {
                let params: lt::DidChangeTextDocumentParams =
                    serde_json::from_value(notification.params)?;
                let Some(path) = convert::uri_to_path(&params.text_document.uri) else {
                    return Ok(());
                };
                self.change(&path, &params.content_changes);
                self.publish_all(connection)
            }
            lt::notification::DidCloseTextDocument::METHOD => {
                let params: lt::DidCloseTextDocumentParams =
                    serde_json::from_value(notification.params)?;
                let Some(path) = convert::uri_to_path(&params.text_document.uri) else {
                    return Ok(());
                };
                self.close(&path);
                self.publish_all(connection)
            }
            lt::notification::Cancel::METHOD => {
                let params: lt::CancelParams = serde_json::from_value(notification.params)?;
                self.cancelled.insert(request_id(params.id));
                Ok(())
            }
            _ => Ok(()),
        }
    }

    /// `didOpen`: overlay the editor buffer over the loaded input, or create
    /// a standalone overlay for a file outside the workspace.
    fn open(&mut self, path: String, text: String) {
        if let Some(input) = self.files.get(&path).copied() {
            self.set_text(path, input, text);
        } else if let Some((input, _)) = self.overlays.get(&path).copied() {
            self.set_text(path, input, text);
        } else {
            let input = InputFile::new(&self.db, path.clone(), text);
            let package = Package::new(
                &self.db,
                overlay_package_name(&self.db, input, &path),
                vec![input],
                PackageOrigin::WorkspaceMember,
                BTreeMap::new(),
                None,
                None,
            );
            self.overlays.insert(path, (input, package));
        }
    }

    /// `didChange`: applies the incremental content changes in order — each
    /// range is relative to the text after the previous change — then drives
    /// `set_text` once.
    fn change(&mut self, path: &str, changes: &[lt::TextDocumentContentChangeEvent]) {
        let input = match self.files.get(path).copied() {
            Some(input) => input,
            None => match self.overlays.get(path) {
                Some((input, _)) => *input,
                None => return,
            },
        };
        let mut text = input.text(&self.db).clone();
        for change in changes {
            match change.range {
                Some(range) => {
                    let lines = convert::line_index(&text);
                    let range = lines.text_range(range);
                    text.replace_range(
                        usize::from(range.start())..usize::from(range.end()),
                        &change.text,
                    );
                }
                None => text.clone_from(&change.text),
            }
        }
        self.set_text(path.to_string(), input, text);
    }

    /// `didClose`: a standalone overlay is dropped (its diagnostics clear on
    /// the next publish); a workspace file reverts to its on-disk text.
    fn close(&mut self, path: &str) {
        if self.overlays.remove(path).is_some() {
            // Dropping the overlay drops its input; a later reopen mints a
            // fresh one, so the cached line table for this path is stale.
            self.line_indexes.remove(path);
            return;
        }
        if let Some(input) = self.files.get(path).copied()
            && let Ok(text) = std::fs::read_to_string(path)
        {
            self.set_text(path.to_string(), input, text);
        }
    }

    /// Drives `set_text` on an existing input — the edit that starts a new
    /// salsa revision. A text that did not change is a no-op, keeping every
    /// memo warm.
    fn set_text(&mut self, path: String, input: InputFile, text: String) {
        if input.text(&self.db) != &text {
            input.set_text(&mut self.db).to(text);
            // The cached line table described the old text; drop it so the
            // next hover or navigation rebuilds it.
            self.line_indexes.remove(&path);
            // The loader's law findings for this file described the loaded
            // text; they are stale from here on.
            self.edited.insert(path);
        }
    }

    /// Runs parse + resolve + check over every package (workspace members
    /// and overlays) through the memoized queries and converts the result.
    ///
    /// The per-package passes stamp their spans with a [`FileId`] indexing
    /// `pkg.files(db)` in order; like the `ridlc` driver, this interns each
    /// package's files into a fresh [`SourceMap`] (collecting the issued ids
    /// in the same file order) and rewrites the spans onto those ids with
    /// [`remap_diagnostics`] before conversion.
    ///
    /// The workspace-wide passes — the service catalog and the rsdl system
    /// query (rsdl reference v0.2) — run once over the workspace afterwards,
    /// through the `check_workspace` that `ridlc` calls too.
    fn analyze(&self) -> Batch {
        let db = &self.db;
        let mut sources = SourceMap::new();
        let mut table: HashMap<FileId, (String, String)> = HashMap::new();
        let mut all: Vec<Diagnostic> = Vec::new();

        let overlay_packages: Vec<Package> = self
            .overlays
            .values()
            .map(|(_, package)| *package)
            .collect();
        // The render ids of the workspace's source files in package-then-file
        // order, the order the workspace-wide diagnostics index them in. A
        // package's lock is not one of those files.
        let mut workspace_render_ids: Vec<FileId> = Vec::new();
        let workspace_packages = self.workspace.packages(db).len();
        let packages = self.workspace.packages(db).iter().copied();
        for (index, package) in packages.chain(overlay_packages).enumerate() {
            let files = package.files(db).clone();
            let mut render_ids = Vec::with_capacity(files.len() + 1);
            for file in &files {
                let path = file.path(db);
                let text = file.text(db);
                let id = sources.file_id(path, text);
                table
                    .entry(id)
                    .or_insert_with(|| (path.clone(), text.clone()));
                render_ids.push(id);
            }
            if index < workspace_packages {
                workspace_render_ids.extend(&render_ids);
            }
            // The checker stamps a lock diagnostic (RIDL-409) with the index
            // after the package's files, so the lock's own id goes last (plan
            // decision PD-12), and its text joins the table so the diagnostic
            // is published under the lock file's URI.
            if let Some(lock) = package.lock(db) {
                let id = sources.file_id(&lock.path, &lock.text);
                table
                    .entry(id)
                    .or_insert_with(|| (lock.path.clone(), lock.text.clone()));
                render_ids.push(id);
            }

            for (file, id) in files.iter().zip(&render_ids) {
                for error in parse_file(db, *file).errors() {
                    all.push(Diagnostic {
                        code: DiagCode(error.code),
                        severity: Severity::Error,
                        message: house_style_message(&error.message),
                        primary: Span {
                            file: *id,
                            range: error.range,
                        },
                        labels: Vec::new(),
                        fixits: Vec::new(),
                    });
                }
            }

            let resolution = resolve_package(db, self.workspace, package, self.std);
            all.extend(remap_diagnostics(resolution.diagnostics, &render_ids));
            let checked = check_package(db, self.workspace, package, self.std);
            all.extend(remap_diagnostics(checked.diagnostics.clone(), &render_ids));
        }
        // The workspace-wide passes — the service catalog and the rsdl system
        // query — run once over the workspace, as in `ridlc`; a standalone
        // overlay is outside it. RSDL-804 is raised by the driver, not the
        // query: the server configures no backend, as the command line does
        // not, so no namespace is claimed and every backend key draws the
        // warning `ridl check` shows.
        let CheckedWorkspace {
            diagnostics: workspace_diagnostics,
            system,
        } = check_workspace(db, self.workspace, self.std);
        all.extend(remap_diagnostics(
            workspace_diagnostics,
            &workspace_render_ids,
        ));
        all.extend(unclaimed_backend_keys(
            db,
            &system,
            &BTreeSet::new(),
            &mut sources,
        ));
        batch(all, &table)
    }

    /// Recomputes the diagnostics for every package, merges the still-valid
    /// loader findings, and publishes: one notification per path with
    /// findings, plus an explicit empty list for every path that had
    /// findings before and is clean now.
    fn publish_all(&mut self, connection: &Connection) -> Result<(), Error> {
        let Batch {
            mut diagnostics,
            fixes,
        } = self.analyze();
        for (path, loader) in &self.loader_diagnostics {
            if self.edited.contains(path) {
                continue;
            }
            let entry = diagnostics.entry(path.clone()).or_default();
            entry.splice(0..0, loader.iter().cloned());
        }

        let current: HashSet<String> = diagnostics.keys().cloned().collect();
        for stale in self.published.difference(&current) {
            publish(connection, stale, Vec::new())?;
        }
        for (path, list) in &diagnostics {
            publish(connection, path, list.clone())?;
        }
        self.published = current;
        self.fixes = fixes;
        Ok(())
    }

    /// The quick fixes whose diagnostic touches the requested range.
    fn code_actions(&self, params: &lt::CodeActionParams) -> Vec<lt::CodeActionOrCommand> {
        let Some(path) = convert::uri_to_path(&params.text_document.uri) else {
            return Vec::new();
        };
        let Some(fixes) = self.fixes.get(&path) else {
            return Vec::new();
        };
        fixes
            .iter()
            .filter(|fix| ranges_touch(fix.range, params.range))
            .map(|fix| lt::CodeActionOrCommand::CodeAction(fix.action.clone()))
            .collect()
    }

    /// `textDocument/hover`: the declaration or field the cursor names, rendered
    /// as markdown (E1.15b).
    ///
    /// In an `.rsdl` file, the declaration an rsdl reference names (E6.15); a
    /// typl symbol lookup never runs there, because a component name is not a
    /// typl symbol.
    fn hover(&mut self, params: &lt::HoverParams) -> Option<lt::Hover> {
        let position = params.text_document_position_params.position;
        let path = convert::uri_to_path(&params.text_document_position_params.text_document.uri)?;
        let (file, package) = self.locate(&path)?;
        let offset = self.line_index_of(file).offset(position);
        let info = if profile_of_path(&path) == Profile::Rsdl {
            rsdl::hover(&self.db, self.workspace, self.std, package, file, offset)?
        } else {
            hover::hover(&self.db, self.workspace, self.std, package, file, offset)?
        };
        let range = self.line_index_of(file).range(info.range);
        Some(lt::Hover {
            contents: lt::HoverContents::Markup(lt::MarkupContent {
                kind: lt::MarkupKind::Markdown,
                value: info.markdown,
            }),
            range: Some(range),
        })
    }

    /// `textDocument/definition`: the declaration site of the symbol the cursor
    /// names, resolved through imports and qualified references (E1.15b).
    ///
    /// In an `.rsdl` file, the declaration an rsdl reference names (E6.15).
    fn goto_definition(
        &mut self,
        params: &lt::GotoDefinitionParams,
    ) -> Option<lt::GotoDefinitionResponse> {
        let position = params.text_document_position_params.position;
        let path = convert::uri_to_path(&params.text_document_position_params.text_document.uri)?;
        let (file, package) = self.locate(&path)?;
        let offset = self.line_index_of(file).offset(position);
        let (target, range) = if profile_of_path(&path) == Profile::Rsdl {
            rsdl::definition(&self.db, self.workspace, self.std, file, offset)?
        } else {
            let located =
                nav::symbol_at(&self.db, self.workspace, self.std, package, file, offset)?;
            (located.symbol.file, located.symbol.range)
        };
        let location = self.location(target, range)?;
        Some(lt::GotoDefinitionResponse::Scalar(location))
    }

    /// `textDocument/references`: every resolved reference to the symbol the
    /// cursor names, across every loaded package — the declaration itself
    /// included when the client asks for it (E1.15b).
    fn references(&mut self, params: &lt::ReferenceParams) -> Option<Vec<lt::Location>> {
        let position = params.text_document_position.position;
        let path = convert::uri_to_path(&params.text_document_position.text_document.uri)?;
        let (file, package) = self.locate(&path)?;
        let offset = self.line_index_of(file).offset(position);
        let located = nav::symbol_at(&self.db, self.workspace, self.std, package, file, offset)?;

        let packages = self.search_packages();
        let references = nav::find_references(
            &self.db,
            self.workspace,
            self.std,
            &packages,
            &located.symbol,
        );

        let mut locations = Vec::new();
        if params.context.include_declaration
            && let Some(location) = self.location(located.symbol.file, located.symbol.range)
        {
            locations.push(location);
        }
        for (file, range) in references {
            if let Some(location) = self.location(file, range) {
                locations.push(location);
            }
        }
        Some(locations)
    }

    /// `textDocument/completion`: the items offered for the cursor position,
    /// dispatched by the syntactic context the cursor sits in (E1.15c).
    fn completion(&mut self, params: &lt::CompletionParams) -> Option<lt::CompletionResponse> {
        let position = params.text_document_position.position;
        let path = convert::uri_to_path(&params.text_document_position.text_document.uri)?;
        let (file, package) = self.locate(&path)?;
        let offset = self.line_index_of(file).offset(position);
        let packages = self.search_packages();
        let items = complete::completion(
            &self.db,
            self.workspace,
            self.std,
            package,
            file,
            offset,
            &packages,
        );
        Some(lt::CompletionResponse::Array(items))
    }

    /// `textDocument/inlayHint`: the ordinal and unit-expansion hints inside the
    /// requested range, converted to LSP positions through the file's line
    /// table (E1.16). A range request — only hints in the visible window are
    /// returned.
    fn inlay_hints(&mut self, params: &lt::InlayHintParams) -> Option<Vec<lt::InlayHint>> {
        let path = convert::uri_to_path(&params.text_document.uri)?;
        let (file, package) = self.locate(&path)?;
        let index = self.line_index_of(file);
        let range = index.text_range(params.range);
        let hints = inlay::inlay_hints(&self.db, self.workspace, self.std, package, file, range);
        Some(
            hints
                .into_iter()
                .map(|hint| lt::InlayHint {
                    position: index.position(hint.offset),
                    label: lt::InlayHintLabel::String(hint.label),
                    kind: Some(match hint.kind {
                        inlay::HintKind::Ordinal => lt::InlayHintKind::PARAMETER,
                        inlay::HintKind::Unit => lt::InlayHintKind::TYPE,
                    }),
                    text_edits: None,
                    tooltip: None,
                    padding_left: Some(true),
                    padding_right: None,
                    data: None,
                })
                .collect(),
        )
    }

    /// `textDocument/prepareRename`: the name span the cursor is on when it is a
    /// renameable symbol, so the client can validate before applying (E1.15c).
    fn prepare_rename(
        &mut self,
        params: &lt::TextDocumentPositionParams,
    ) -> Option<lt::PrepareRenameResponse> {
        let path = convert::uri_to_path(&params.text_document.uri)?;
        let (file, package) = self.locate(&path)?;
        let offset = self.line_index_of(file).offset(params.position);
        let range = rename::prepare(&self.db, self.workspace, self.std, package, file, offset)?;
        let lsp_range = self.line_index_of(file).range(range);
        Some(lt::PrepareRenameResponse::Range(lsp_range))
    }

    /// `textDocument/rename`: the workspace edit renaming the symbol under the
    /// cursor, or a [`RenameError`](rename::RenameError) the caller turns into an
    /// LSP error response (E1.15c).
    fn rename(
        &mut self,
        params: &lt::RenameParams,
    ) -> Result<lt::WorkspaceEdit, rename::RenameError> {
        let position = params.text_document_position.position;
        let path = convert::uri_to_path(&params.text_document_position.text_document.uri)
            .ok_or(rename::RenameError::NotRenameable)?;
        let (file, package) = self
            .locate(&path)
            .ok_or(rename::RenameError::NotRenameable)?;
        let offset = self.line_index_of(file).offset(position);
        let packages = self.search_packages();
        let edits = rename::rename(
            &self.db,
            self.workspace,
            self.std,
            package,
            file,
            offset,
            &packages,
            &params.new_name,
        )?;
        Ok(self.workspace_edit(edits, &params.new_name))
    }

    /// Groups rename edits into a [`lt::WorkspaceEdit`], mapping each edited
    /// input to its `file://` URI and its byte range to an LSP range. An input
    /// with no `file://` URI (the embedded `ridl.std`) is dropped.
    fn workspace_edit(&mut self, edits: Vec<rename::Edit>, new_name: &str) -> lt::WorkspaceEdit {
        // `WorkspaceEdit.changes` is keyed by `lsp_types::Uri`, whose inner
        // cache cell trips `mutable_key_type`; the key's identity (the URI
        // string) never mutates.
        #[allow(clippy::mutable_key_type)]
        let mut changes: HashMap<lt::Uri, Vec<lt::TextEdit>> = HashMap::new();
        for edit in edits {
            let Some(uri) = convert::path_to_uri(edit.file.path(&self.db)) else {
                continue;
            };
            let range = self.line_index_of(edit.file).range(edit.range);
            changes.entry(uri).or_default().push(lt::TextEdit {
                range,
                new_text: new_name.to_string(),
            });
        }
        lt::WorkspaceEdit {
            changes: Some(changes),
            document_changes: None,
            change_annotations: None,
        }
    }

    /// The input and package for a document path: an overlay file, or a loaded
    /// workspace file with its package.
    fn locate(&self, path: &str) -> Option<(InputFile, Package)> {
        if let Some((input, package)) = self.overlays.get(path) {
            return Some((*input, *package));
        }
        let input = self.files.get(path).copied()?;
        let package = self.file_package.get(path).copied()?;
        Some((input, package))
    }

    /// The every-package universe find-references walks: the workspace members,
    /// the standalone overlays, and the embedded `ridl.std`.
    fn search_packages(&self) -> Vec<Package> {
        let mut packages = self.workspace.packages(&self.db).clone();
        packages.extend(self.overlays.values().map(|(_, package)| *package));
        packages.push(self.std);
        packages
    }

    /// The line table for `file`, cached by path and invalidated on `didChange`.
    fn line_index_of(&mut self, file: InputFile) -> Rc<LineIndex> {
        let path = file.path(&self.db).clone();
        if let Some(index) = self.line_indexes.get(&path) {
            return index.clone();
        }
        let index = Rc::new(convert::line_index(file.text(&self.db)));
        self.line_indexes.insert(path, index.clone());
        index
    }

    /// The LSP location of a byte range inside `file`, or `None` when the file's
    /// path is not an absolute `file://` URI.
    fn location(&mut self, file: InputFile, range: TextRange) -> Option<lt::Location> {
        let uri = convert::path_to_uri(file.path(&self.db))?;
        let index = self.line_index_of(file);
        Some(lt::Location {
            uri,
            range: index.range(range),
        })
    }
}

/// Sends a `window/showMessage` notification: the client shows `message` to
/// the user.
fn show_message(
    connection: &Connection,
    typ: lt::MessageType,
    message: String,
) -> Result<(), Error> {
    let notification = Notification::new(
        lt::notification::ShowMessage::METHOD.to_string(),
        lt::ShowMessageParams { typ, message },
    );
    connection.sender.send(notification.into())?;
    Ok(())
}

/// Sends one `textDocument/publishDiagnostics` notification for `path`.
fn publish(
    connection: &Connection,
    path: &str,
    diagnostics: Vec<lt::Diagnostic>,
) -> Result<(), Error> {
    let Some(uri) = convert::path_to_uri(path) else {
        return Ok(());
    };
    let params = lt::PublishDiagnosticsParams {
        uri,
        diagnostics,
        version: None,
    };
    let notification = Notification::new(
        lt::notification::PublishDiagnostics::METHOD.to_string(),
        params,
    );
    connection.sender.send(notification.into())?;
    Ok(())
}

/// Converts coded diagnostics whose spans point into `table` (file id →
/// path and text) into LSP diagnostics and quick fixes grouped by the
/// primary span's path. A diagnostic whose primary file is not in `table`
/// (a detached span) is dropped; labels and fix-its follow the same rule
/// individually.
fn batch(diagnostics: Vec<Diagnostic>, table: &HashMap<FileId, (String, String)>) -> Batch {
    // One URI and line table per file the diagnostics actually reference.
    let mut resolved: HashMap<FileId, (lt::Uri, LineIndex)> = HashMap::new();
    let referenced = diagnostics.iter().flat_map(|diagnostic| {
        std::iter::once(diagnostic.primary.file)
            .chain(diagnostic.labels.iter().map(|label| label.span.file))
            .chain(diagnostic.fixits.iter().map(|fixit| fixit.span.file))
    });
    for file in referenced {
        if resolved.contains_key(&file) {
            continue;
        }
        let Some((path, text)) = table.get(&file) else {
            continue;
        };
        let Some(uri) = convert::path_to_uri(path) else {
            continue;
        };
        resolved.insert(file, (uri, convert::line_index(text)));
    }
    let resolve = |file: FileId| resolved.get(&file).map(|(uri, lines)| (uri, lines));

    let mut out = Batch::default();
    for diagnostic in &diagnostics {
        let Some(lsp) = convert::diagnostic(diagnostic, resolve) else {
            continue;
        };
        let path = table[&diagnostic.primary.file].0.clone();
        if !diagnostic.fixits.is_empty() {
            let range = lsp.range;
            let actions = convert::quick_fixes(&lsp, &diagnostic.fixits, resolve);
            out.fixes
                .entry(path.clone())
                .or_default()
                .extend(actions.into_iter().map(|action| QuickFix { range, action }));
        }
        out.diagnostics.entry(path).or_default().push(lsp);
    }
    out
}

/// Recovers path and text for the load-time [`FileId`]s and converts the
/// loader's findings (manifest diagnostics, the package↔directory law) once,
/// grouped by path.
///
/// The load-time [`SourceMap`] is the authority: [`SourceMap::path`] reverses
/// each diagnostic's [`FileId`] to the file the loader interned it for. A
/// `.typl` file's current text comes from its [`InputFile`]; a manifest's text
/// is read back from disk (nothing edited it this early). A span the map cannot
/// resolve (only [`FileId::DETACHED`]) or a manifest that cannot be read drops
/// its diagnostics from publication — `ridl check` still renders them.
fn convert_loader_diagnostics(
    db: &RidlDatabase,
    files: &HashMap<String, InputFile>,
    diagnostics: Vec<Diagnostic>,
    sources: &SourceMap,
) -> BTreeMap<String, Vec<lt::Diagnostic>> {
    if diagnostics.is_empty() {
        return BTreeMap::new();
    }
    let referenced = diagnostics.iter().flat_map(|diagnostic| {
        std::iter::once(diagnostic.primary.file)
            .chain(diagnostic.labels.iter().map(|label| label.span.file))
            .chain(diagnostic.fixits.iter().map(|fixit| fixit.span.file))
    });

    let mut table: HashMap<FileId, (String, String)> = HashMap::new();
    for id in referenced {
        if table.contains_key(&id) {
            continue;
        }
        let Some(path) = sources.path(id) else {
            continue;
        };
        let path = path.to_string();
        let text = match files.get(&path) {
            Some(input) => input.text(db).clone(),
            None => match std::fs::read_to_string(&path) {
                Ok(text) => text,
                Err(_) => continue,
            },
        };
        table.insert(id, (path, text));
    }
    // The loader emits no fix-its, so the batch's fixes stay empty.
    batch(diagnostics, &table).diagnostics
}

/// The synthetic package name of a standalone overlay file: its declared
/// `package` name, falling back to the file stem — the loader's single-file
/// rule (E1.3).
fn overlay_package_name(db: &RidlDatabase, input: InputFile, path: &str) -> String {
    let parse = parse_file(db, input);
    let source = SourceFile::cast(parse.syntax()).expect("parser roots every tree in a SourceFile");
    source
        .package_decl()
        .and_then(|decl| decl.qualified_name())
        .map(|name| {
            name.syntax()
                .descendants_with_tokens()
                .filter_map(|element| element.into_token())
                .filter(|token| !token.kind().is_trivia())
                .map(|token| token.text().to_string())
                .collect::<String>()
        })
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| {
            Path::new(path)
                .file_stem()
                .map(|stem| stem.to_string_lossy().into_owned())
                .unwrap_or_else(|| "package".to_string())
        })
}

/// The `lsp_server` request id for an LSP cancel parameter.
fn request_id(id: lt::NumberOrString) -> RequestId {
    match id {
        lt::NumberOrString::Number(number) => number.into(),
        lt::NumberOrString::String(string) => string.into(),
    }
}

/// Whether two LSP ranges share at least one position.
fn ranges_touch(a: lt::Range, b: lt::Range) -> bool {
    a.start <= b.end && b.start <= a.end
}
