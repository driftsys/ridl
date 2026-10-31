//! The synchronous server loop and its state (ADR-0004 §6).
//!
//! [`run`] follows the rust-analyzer `lsp-server` pattern: an initialize
//! handshake, then a plain loop that receives one message at a time and
//! dispatches it — no async runtime. Because the loop is strictly
//! sequential, a `$/cancelRequest` is dequeued only after older requests
//! were already answered; the cancelled-set check before each dispatch is
//! the hook the later, longer-running handlers extend, and
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
//! it (issue #384). Both loads find the root with [`find_root`]; every path
//! the server passes it comes from a `file://` URI and is absolute, so the
//! root does not depend on the server's current directory. A file
//! inside a member of a `[workspace]` loads the whole workspace and its
//! imports of sibling members resolve (ADR-0002 §4). The server publishes the
//! diagnostics of every loaded member; it does not narrow them to the member
//! the editor was opened at. One load failure is shown once, whether the
//! initialize load or the first `didOpen` meets it first.
//!
//! The `[lints]` levels the load resolved apply to every published
//! diagnostic (ADR-0024 decision 6): to the loader's findings once, at
//! load time, and to the analysis results on every recompute, both before
//! conversion. A lint at `allow` is not published; one at `deny` is published
//! as an error.
//!
//! The size pass of `ridlc` (RIDL-151, RIDL-152, RIDL-157 and the MANI-022 of
//! the `[limits]` table) runs on every recompute, through the same function
//! `ridl check` calls. The workspace is sized under the encodings of the root
//! manifest's `[backend.<name>]` tables and the `[limits]` table of each unit;
//! a standalone overlay has neither, so it is sized for FlatBuffers against
//! the default ceilings. Each source set is passed only its own errors, so an
//! error in one set does not skip the others. The pass also reads the loader's
//! errors, other than RSDL-7xx, as `ridl check` does, so a manifest error
//! skips the packages beneath that manifest. Its diagnostics on a manifest
//! are published for that `ridl.toml`.
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
use ridl_core::TimingDefaults;
use ridl_core::db::{InputFile, RidlDatabase, parse_file};
use ridl_core::diag::{
    DiagCode, Diagnostic, FileId, LineCol, Severity, SourceMap, Span, house_style_message,
    line_col, remap_diagnostics,
};
use ridl_core::lint::{LintScopes, apply_lint_levels, report_lint_directives};
use ridl_core::package::{Package, PackageOrigin, Workspace};
use ridl_core::{
    Cache, Frozen, LoadedWorkspace, RemoteAccess, find_root, load_workspace_with_remotes,
    profile_of_path, std_package,
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
pub fn run(connection: Connection) -> Result<SessionEnd, Error> {
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
pub fn run_with_version(
    connection: Connection,
    version: Option<&str>,
) -> Result<SessionEnd, Error> {
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
        // The first `didOpen` under the same manifest meets the same error;
        // recording it here keeps that `didOpen` from showing it again.
        if let Some(manifest_dir) = find_root(&root) {
            state.shown_load_error = Some(load_error_key(&manifest_dir, &err));
        }
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
/// quick-fix code actions, hover, goto-definition, and find-references,
/// completion and rename, and inlay hints. Rename
/// advertises `prepareProvider` so the client validates the cursor and the new
/// name before applying an edit. Inlay hints close the E1 LSP feature set.
/// Whole-document formatting runs the `ridl fmt` engine.
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
            // `.` completes an import path or a doc link's path; `:` a type
            // position; `[` opens a doc link and `@` a doc tag (ADR-0026).
            // Identifier characters need not be listed — the client triggers
            // on those.
            trigger_characters: Some(vec![
                ":".to_string(),
                ".".to_string(),
                "[".to_string(),
                "@".to_string(),
            ]),
            ..Default::default()
        }),
        rename_provider: Some(lt::OneOf::Right(lt::RenameOptions {
            prepare_provider: Some(true),
            work_done_progress_options: Default::default(),
        })),
        inlay_hint_provider: Some(lt::OneOf::Left(true)),
        document_formatting_provider: Some(lt::OneOf::Left(true)),
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

/// How a session that reached the message loop ended. The LSP specification
/// (3.17, "Exit Notification") ties the process exit code to this: 0 only
/// after `shutdown` then `exit`, 1 otherwise.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SessionEnd {
    /// The client sent `shutdown`, then `exit`.
    Shutdown,
    /// The client sent `exit` with no `shutdown` before it, or the
    /// connection closed with no `shutdown`.
    WithoutShutdown,
}

/// Receives and dispatches messages until the client shuts the server down
/// or the connection closes.
fn main_loop(connection: Connection, mut state: ServerState) -> Result<SessionEnd, Error> {
    for message in &connection.receiver {
        match message {
            Message::Request(request) => {
                if connection.handle_shutdown(&request)? {
                    return Ok(SessionEnd::Shutdown);
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
            // `handle_shutdown` consumes the `exit` that follows a `shutdown`,
            // so an `exit` seen here arrived with no `shutdown` before it.
            Message::Notification(notification) if notification.method == "exit" => {
                return Ok(SessionEnd::WithoutShutdown);
            }
            Message::Notification(notification) => {
                state.dispatch_notification(notification, &connection)?;
            }
            // The server sends no requests of its own yet, so no responses
            // arrive.
            Message::Response(_) => {}
        }
    }
    Ok(SessionEnd::WithoutShutdown)
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
    /// The Errors in a file of a remote unit, one line each
    /// ([`remote_notice_line`]), which are not published and join the
    /// window message of [`ServerState::remote_notice`] instead.
    remote_errors: Vec<String>,
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
    /// The `[lints]` levels by directory, as the loader built them (ADR-0024
    /// decision 10); replaced by each [`ServerState::load`], empty
    /// before one succeeds. The levels resolve by directory, not by file id,
    /// so a file opened after the load — an overlay with no load-time entry —
    /// still takes the levels of the member whose directory contains it. The
    /// scope keys are in the path form the loader read the workspace from,
    /// which is the form of the client's root URI; a `didOpen` path is in the
    /// form of its own URI. Neither side is canonicalised.
    lints: LintScopes,
    /// The manifest directory and reason of the last load error shown, by the
    /// initialize load or by a `didOpen`, so the same error is not shown
    /// again on every later `didOpen`.
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
    /// The remote units the load read, by name and directory.
    remote_units: Vec<(String, PathBuf)>,
    /// The `[backend.<name>]` tables of the loaded root manifest, whose
    /// encodings the size pass measures. Empty when no table is named.
    backends: BTreeMap<String, ridl_core::BackendTable>,
    /// The `[limits]` table of each loaded unit by unit name, with the path
    /// and text of the manifest it was read from. The load-time file ids of
    /// the tables are not valid in a recompute's source map, so each
    /// recompute interns the manifest again.
    unit_limits: BTreeMap<String, SizeManifest>,
    /// The path and text of the file of each load-time Error that blocks the
    /// size pass (any code but RSDL-7xx), so the pass skips the same
    /// packages as `ridl check`. Only the workspace call reads them; they
    /// are not published here.
    loader_errors: Vec<(String, String)>,
    /// Whether a load-time Error that blocks the size pass has no file (only
    /// a [`FileId::DETACHED`] span: a remote import that is not cached, for
    /// example). `ridl check` lets such an error block every package, so the
    /// size pass of the workspace is skipped while it stands.
    loader_error_detached: bool,
    /// The directories a remote unit is read from — each loaded unit's
    /// directory, the manifest root's `vendor/` and the cache root
    /// ([`ridlc::remote_dirs`]). A remote unit
    /// is a read-only dependency: it is checked so that the consumer's
    /// references resolve, and no diagnostic is published for a file under
    /// one of these, not even a loader finding at a unit that did not load.
    remote_dirs: Vec<PathBuf>,
    /// The Errors the load left that no publish carries, one line each: the
    /// detached ones — a remote unit that is pinned but not cached
    /// (MANI-104), or not pinned (MANI-103) — and those in a file under a
    /// remote directory. `Some` after a load, until the next
    /// [`ServerState::publish_all`] joins the analysis's remote Errors to
    /// them and shows the lot as one window message: once per load, not on
    /// every edit, because `ridl check` fails on the same Errors and the
    /// editor must not look clean beside a failing check.
    remote_notice: Option<Vec<String>>,
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

/// One unit's `[limits]` table with the manifest it was read from: the path
/// and the text the table's byte ranges point into.
struct SizeManifest {
    path: String,
    text: String,
    limits: ridl_core::manifest::Limits,
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
            lints: LintScopes::default(),
            shown_load_error: None,
            files: HashMap::new(),
            file_package: HashMap::new(),
            overlays: HashMap::new(),
            line_indexes: HashMap::new(),
            loader_diagnostics: BTreeMap::new(),
            remote_units: Vec::new(),
            backends: BTreeMap::new(),
            unit_limits: BTreeMap::new(),
            loader_errors: Vec::new(),
            loader_error_detached: false,
            remote_dirs: Vec::new(),
            remote_notice: None,
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
    /// A load that reads no package and whose root `ridl.toml` draws an
    /// error (a TOML syntax error, for example) is an error too, so it is
    /// shown like a manifest that cannot be read.
    ///
    /// An open overlay whose path the loaded workspace contains (a file
    /// opened before its `ridl.toml` existed) moves its buffer onto the
    /// loaded input and stops being an overlay, so the file is analyzed once,
    /// as a member of its package.
    ///
    /// The remote units of the manifest's `[imports]` are read from
    /// `vendor/`, then the cache, under [`Frozen::Yes`]: the server never
    /// fetches and never writes `ridl.lock`. A pin neither holds is
    /// MANI-104, kept as [`ServerState::remote_notice`].
    fn load(&mut self, dir: &Path) -> io::Result<()> {
        let cache = Cache::user_default();
        // `report_scope` is not used: the server publishes the diagnostics of
        // every loaded file of the consumer.
        let LoadedWorkspace {
            workspace,
            mut diagnostics,
            sources,
            lints,
            report_scope: _,
            units: _,
            root_package: _,
            codegen_header: _,
            backends,
            unit_limits,
            imports: _,
            remote_units,
        } = load_workspace_with_remotes(
            &mut self.db,
            dir,
            &[],
            RemoteAccess::new(Frozen::Yes, &cache),
        )
        .map_err(|error| match error {
            ridl_core::LoadError::Io(e) => e,
            _ => unreachable!("no overlay error is possible without overlays"),
        })?;
        if workspace.packages(&self.db).is_empty()
            && let Some(root) = find_root(dir)
        {
            let manifest = root.join("ridl.toml").to_string_lossy().into_owned();
            if let Some(error) = diagnostics.iter().find(|diagnostic| {
                diagnostic.severity == Severity::Error
                    && sources.path(diagnostic.primary.file) == Some(manifest.as_str())
            }) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("`{manifest}` is not a valid manifest: {}", error.message),
                ));
            }
        }
        for package in workspace.packages(&self.db) {
            for file in package.files(&self.db) {
                self.files.insert(file.path(&self.db).clone(), *file);
                self.file_package
                    .insert(file.path(&self.db).clone(), *package);
            }
        }
        self.remote_dirs = ridlc::remote_dirs(&remote_units, &cache, dir);
        let blocking = diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Error && !d.code.as_str().starts_with("RSDL-7"));
        self.loader_error_detached = blocking
            .clone()
            .any(|d| sources.path(d.primary.file).is_none());
        self.loader_errors = blocking
            .filter_map(|d| {
                let path = sources.path(d.primary.file)?.to_string();
                let text = sources.text(d.primary.file)?.to_string();
                Some((path, text))
            })
            .collect();
        self.backends = backends;
        self.unit_limits = unit_limits
            .into_iter()
            .filter_map(|(unit, table)| {
                let path = sources.path(table.manifest)?.to_string();
                let text = match sources.text(table.manifest) {
                    Some(text) => text.to_string(),
                    None => std::fs::read_to_string(&path).ok()?,
                };
                Some((
                    unit,
                    SizeManifest {
                        path,
                        text,
                        limits: table.limits,
                    },
                ))
            })
            .collect();
        self.remote_units = remote_units
            .into_iter()
            .map(|(name, unit)| (name, unit.dir))
            .collect();
        // A detached Error has no file to publish under, and one in a remote
        // file is not published either, so both go to the notice;
        // `convert_loader_diagnostics` would drop the first and
        // `publish_all` skips the second.
        let notice: Vec<String> = diagnostics
            .iter()
            .filter(|diagnostic| {
                diagnostic.severity == Severity::Error
                    && sources
                        .path(diagnostic.primary.file)
                        .is_none_or(|path| self.is_remote_path(path))
            })
            .map(|diagnostic| remote_notice_line(diagnostic, &sources, &self.remote_units))
            .collect();
        self.remote_notice = Some(notice);
        // `analyze` never sees the loader's findings, so their levels are
        // applied here, once, before the conversion (ADR-0024
        // decision 6). A manifest's MANI-010 is itself a lint, and its file is in
        // the scope of the manifest's own directory.
        apply_lint_levels(&mut diagnostics, &sources, &lints);
        self.loader_diagnostics =
            convert_loader_diagnostics(&self.db, &self.files, diagnostics, &sources);
        self.workspace = workspace;
        self.lints = lints;
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
    /// that [`find_root`] finds from the opened file (issue #384).
    /// No manifest there is not an error — the file becomes a standalone
    /// overlay, and the next `didOpen` tries again. A load from a manifest
    /// that fails is shown to the user. A manifest that stays unreadable
    /// fails again on each `didOpen`; its error is shown only when it differs
    /// from the last one shown, by the initialize load or by a `didOpen`, by
    /// manifest directory or by reason.
    fn load_for_opened_file(&mut self, path: &str, connection: &Connection) -> Result<(), Error> {
        if self.loaded {
            return Ok(());
        }
        let Some(dir) = Path::new(path).parent() else {
            return Ok(());
        };
        let Some(root) = find_root(dir) else {
            return Ok(());
        };
        let Err(err) = self.load(&root) else {
            return Ok(());
        };
        let shown = load_error_key(&root, &err);
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
            lt::request::Formatting::METHOD => {
                match serde_json::from_value::<lt::DocumentFormattingParams>(request.params) {
                    Ok(params) => Response::new_ok(request.id, self.formatting(&params)),
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
            let package_name = overlay_package_name(&self.db, input, &path);
            let package = Package::new(
                &self.db,
                package_name.clone(),
                package_name,
                vec![input],
                PackageOrigin::WorkspaceMember,
                BTreeMap::new(),
                TimingDefaults::default(),
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
        let packages: Vec<_> = self
            .workspace
            .packages(db)
            .iter()
            .copied()
            .chain(overlay_packages)
            .collect();
        let mut resolutions = Vec::with_capacity(packages.len());
        let mut checked = Vec::with_capacity(packages.len());
        for (index, package) in packages.iter().copied().enumerate() {
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
            all.extend(remap_diagnostics(
                resolution.diagnostics.clone(),
                &render_ids,
            ));
            resolutions.push(resolution);
            let checked_package = check_package(db, self.workspace, package, self.std);
            all.extend(remap_diagnostics(
                checked_package.diagnostics.clone(),
                &render_ids,
            ));
            checked.push(checked_package);
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
        all.extend(ridlc::check_design_lints(
            db,
            &packages[..workspace_packages],
            &checked[..workspace_packages],
            &resolutions[..workspace_packages],
            &mut sources,
        ));
        // Each standalone overlay is an independent one-package source set.
        // Workspace overlays already update their existing database inputs.
        for index in workspace_packages..packages.len() {
            all.extend(ridlc::check_design_lints(
                db,
                &packages[index..index + 1],
                &checked[index..index + 1],
                &resolutions[index..index + 1],
                &mut sources,
            ));
        }
        // The size pass (ADR-0030), as `ridl check` runs it: the workspace
        // under the loaded `[backend.<name>]` encodings and `[limits]`
        // tables, and each standalone overlay under neither, so it is sized
        // for FlatBuffers against the default ceilings. A manifest is interned
        // in this source map, so the diagnostics on it are published under
        // its path. The pass reads the errors so far, remote ones included,
        // as `ridlc` does, and runs before the remote units' diagnostics are
        // dropped.
        let mut limits = BTreeMap::new();
        for (unit, manifest) in &self.unit_limits {
            let file = sources.file_id(&manifest.path, &manifest.text);
            table
                .entry(file)
                .or_insert_with(|| (manifest.path.clone(), manifest.text.clone()));
            limits.insert(
                unit.clone(),
                ridl_core::UnitLimits {
                    manifest: file,
                    limits: manifest.limits.clone(),
                },
            );
        }
        // Each source set sees only its own errors: an error in one set must
        // not skip the packages of another. A diagnostic belongs to the
        // standalone overlay that holds its file, and otherwise to the
        // workspace. The loader's blocking errors go to the workspace call
        // only, as one synthetic error per file, so nothing extra is
        // published.
        let overlay_paths: Vec<BTreeSet<String>> = packages[workspace_packages..]
            .iter()
            .map(|package| {
                package
                    .files(db)
                    .iter()
                    .map(|file| file.path(db).to_string())
                    .collect()
            })
            .collect();
        let mut workspace_errors: Vec<Diagnostic> = Vec::new();
        let mut overlay_errors: Vec<Vec<Diagnostic>> = vec![Vec::new(); overlay_paths.len()];
        for diagnostic in &all {
            let owner = sources
                .path(diagnostic.primary.file)
                .and_then(|path| overlay_paths.iter().position(|set| set.contains(path)));
            match owner {
                Some(index) => overlay_errors[index].push(diagnostic.clone()),
                None => workspace_errors.push(diagnostic.clone()),
            }
        }
        // Only the file of a marker is read; its code stands for any loader
        // error.
        let marker = |file: FileId| Diagnostic {
            code: DiagCode::MANI_001,
            severity: Severity::Error,
            message: String::new(),
            primary: Span {
                file,
                range: TextRange::default(),
            },
            labels: Vec::new(),
            fixits: Vec::new(),
        };
        for (path, text) in &self.loader_errors {
            if self.edited.contains(path) {
                continue;
            }
            workspace_errors.push(marker(sources.file_id(path, text)));
        }
        // A detached error has no file an edit could clear, so it blocks the
        // workspace's size pass until the next load, as it blocks every
        // package of `ridl check`.
        if self.loader_error_detached {
            workspace_errors.push(marker(FileId::DETACHED));
        }
        let sized = ridlc::check_payload_sizes(
            db,
            self.workspace,
            self.std,
            &packages[..workspace_packages],
            &checked[..workspace_packages],
            &resolutions[..workspace_packages],
            &workspace_errors,
            &self.backends,
            &limits,
            &mut sources,
        );
        all.extend(sized);
        let no_backends = BTreeMap::new();
        let no_limits = BTreeMap::new();
        for index in workspace_packages..packages.len() {
            let own = &overlay_errors[index - workspace_packages];
            let sized = ridlc::check_payload_sizes(
                db,
                self.workspace,
                self.std,
                &packages[index..index + 1],
                &checked[index..index + 1],
                &resolutions[index..index + 1],
                own,
                &no_backends,
                &no_limits,
                &mut sources,
            );
            all.extend(sized);
        }
        // A remote unit's packages were checked so that the consumer's
        // references resolve; nothing is published for their files. Their
        // Errors go to the window message instead, and their Warnings and
        // Infos are dropped here, before the consumer's `[lints]` levels
        // apply (ADR-0002 §5, "Remote units"), as `ridlc` drops them: a
        // `deny` of the consumer must not raise a remote Warning.
        let mut remote_errors = Vec::new();
        all.retain(|diagnostic| {
            let remote = sources
                .path(diagnostic.primary.file)
                .is_some_and(|path| self.is_remote_path(path));
            if remote && diagnostic.severity == Severity::Error {
                remote_errors.push(remote_notice_line(diagnostic, &sources, &self.remote_units));
            }
            !remote
        });
        // The lint directives are reported once, over the list as it stands
        // after the last diagnostic is raised and before any level is applied
        // (ADR-0024 decision 19). The loader's findings are not in `all`, so a
        // directive that names a lint raised only by the loader would look
        // unused. `not_raised` lists the lints that this list never carries:
        //
        // - RIDL-407 (`ridl check`'s baseline desk check) and RIDL-156 (its
        //   podbuf padding check), the two `ridlc::NOT_RAISED_BY_COMPILE`
        //   names, because the server runs neither;
        // - MANI-018 (`vendor-source-not-loaded`), the one lint that
        //   `ridl_core::load_workspace` raises with a span in a `.typl`,
        //   `.ridl` or `.rsdl` file (`workspace.rs`, the `vendor/` scan).
        //   RIDL-416 is spanned at an `interfaces.lock`, RIDL-417 at a `.rxdl`
        //   or `.rmdl` file and MANI-010 at a manifest, none of which can hold
        //   a directive.
        let mut not_raised = ridlc::NOT_RAISED_BY_COMPILE.to_vec();
        not_raised.push(DiagCode::MANI_018);
        report_lint_directives(&mut all, &sources, &not_raised);
        // The `[lints]` levels, resolved through the paths this source map
        // recorded for every file, including an overlay's (ADR-0024
        // decision 6). `allow` removes a diagnostic before conversion, so it is
        // never published. The same step removes a diagnostic that a
        // directive allows.
        apply_lint_levels(&mut all, &sources, &self.lints);
        let mut out = batch(all, &table);
        out.remote_errors = remote_errors;
        out
    }

    /// Recomputes the diagnostics for every package, merges the still-valid
    /// loader findings, and publishes: one notification per path with
    /// findings, plus an explicit empty list for every path that had
    /// findings before and is clean now. On the first publish after a load,
    /// the remote Errors ([`ServerState::remote_notice`], plus the
    /// analysis's) are shown as one window message before the publishes.
    fn publish_all(&mut self, connection: &Connection) -> Result<(), Error> {
        let Batch {
            mut diagnostics,
            fixes,
            remote_errors,
        } = self.analyze();
        if let Some(mut notice) = self.remote_notice.take() {
            notice.extend(remote_errors);
            if !notice.is_empty() {
                show_message(connection, lt::MessageType::ERROR, notice.join("\n"))?;
            }
        }
        for (path, loader) in &self.loader_diagnostics {
            if self.edited.contains(path) || self.is_remote_path(path) {
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

    /// Whether `path` is under a directory a remote unit is read from.
    fn is_remote_path(&self, path: &str) -> bool {
        let path = Path::new(path);
        self.remote_dirs.iter().any(|dir| path.starts_with(dir))
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
    /// as markdown.
    ///
    /// In an `.rsdl` file, the declaration an rsdl reference names; a
    /// typl symbol lookup never runs there, because a component name is not a
    /// typl symbol.
    fn hover(&mut self, params: &lt::HoverParams) -> Option<lt::Hover> {
        let position = params.text_document_position_params.position;
        let path = convert::uri_to_path(&params.text_document_position_params.text_document.uri)?;
        let (file, package) = self.locate(&path)?;
        let offset = self.line_index_of(file).offset(position);
        let info = if profile_of_path(&path) == Profile::Rsdl {
            // An rsdl declaration is a doc carrier too: a doc link in its
            // doc shows the target's hover (ADR-0026).
            rsdl::hover(&self.db, self.workspace, self.std, package, file, offset).or_else(
                || {
                    hover::doc_link_hover_at(
                        &self.db,
                        self.workspace,
                        self.std,
                        package,
                        file,
                        offset,
                    )
                },
            )?
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
    /// names, resolved through imports and qualified references, or
    /// the target of the doc link under the cursor (ADR-0026).
    ///
    /// In an `.rsdl` file, the declaration an rsdl reference names.
    fn goto_definition(
        &mut self,
        params: &lt::GotoDefinitionParams,
    ) -> Option<lt::GotoDefinitionResponse> {
        let position = params.text_document_position_params.position;
        let path = convert::uri_to_path(&params.text_document_position_params.text_document.uri)?;
        let (file, package) = self.locate(&path)?;
        let offset = self.line_index_of(file).offset(position);
        let site = if profile_of_path(&path) == Profile::Rsdl {
            rsdl::definition(&self.db, self.workspace, self.std, file, offset)
        } else {
            nav::symbol_at(&self.db, self.workspace, self.std, package, file, offset)
                .map(|located| (located.symbol.file, located.symbol.range))
        };
        // In either profile, a doc link under the cursor leads to its target
        // (ADR-0026).
        let (target, range) = site.or_else(|| {
            let link = nav::resolve_doc_link_at(
                &self.db,
                self.workspace,
                self.std,
                package,
                file,
                offset,
            )?;
            nav::canonical_site(
                &self.db,
                self.workspace,
                self.std,
                package,
                &link.target.canonical(),
            )
        })?;
        let location = self.location(target, range)?;
        Some(lt::GotoDefinitionResponse::Scalar(location))
    }

    /// `textDocument/references`: every resolved reference to the symbol the
    /// cursor names, across every loaded package — the declaration itself
    /// included when the client asks for it — and every doc link
    /// that names the symbol (ADR-0026). The cursor may sit on a doc link to
    /// the symbol; a link to one of its members names no symbol of its own.
    ///
    /// In an `.rsdl` file, every rsdl reference the checker binds to the
    /// declaration an rsdl reference or declaration name names. On a declared
    /// interface the ridl navigation finds the references, the rsdl
    /// `requires` lines among them; a doc link takes the same path as in a
    /// typl file. No typl symbol is looked up at the cursor there, as in
    /// hover.
    fn references(&mut self, params: &lt::ReferenceParams) -> Option<Vec<lt::Location>> {
        let position = params.text_document_position.position;
        let path = convert::uri_to_path(&params.text_document_position.text_document.uri)?;
        let (file, package) = self.locate(&path)?;
        let offset = self.line_index_of(file).offset(position);
        let include_declaration = params.context.include_declaration;
        let located = if profile_of_path(&path) == Profile::Rsdl {
            match rsdl::references(&self.db, self.workspace, self.std, file, offset) {
                Some(rsdl::References::Rsdl {
                    declaration,
                    references,
                }) => {
                    let declaration = declaration.filter(|_| include_declaration);
                    return Some(self.locations(declaration.into_iter().chain(references)));
                }
                Some(rsdl::References::Interface(symbol)) => Some(symbol),
                None => None,
            }
        } else {
            nav::symbol_at(&self.db, self.workspace, self.std, package, file, offset)
                .map(|located| located.symbol)
        };
        let symbol = match located {
            Some(symbol) => symbol,
            None => {
                let link = nav::resolve_doc_link_at(
                    &self.db,
                    self.workspace,
                    self.std,
                    package,
                    file,
                    offset,
                )?;
                link.target.member.is_none().then_some(link.target.symbol)?
            }
        };

        let packages = self.search_packages();
        let mut references =
            nav::find_references(&self.db, self.workspace, self.std, &packages, &symbol);
        references.extend(nav::doc_link_references(
            &self.db,
            self.workspace,
            self.std,
            &packages,
            &symbol,
        ));

        let declaration = include_declaration.then_some((symbol.file, symbol.range));
        Some(self.locations(declaration.into_iter().chain(references)))
    }

    /// The LSP locations of `sites`, in order, dropping a site whose file has
    /// no `file://` URI.
    fn locations(
        &mut self,
        sites: impl IntoIterator<Item = (InputFile, TextRange)>,
    ) -> Vec<lt::Location> {
        sites
            .into_iter()
            .filter_map(|(file, range)| self.location(file, range))
            .collect()
    }

    /// `textDocument/completion`: the items offered for the cursor position,
    /// dispatched by the syntactic context the cursor sits in.
    ///
    /// In an `.rsdl` file, the rsdl contexts: an item that completes a
    /// reference replaces the part of it already written.
    fn completion(&mut self, params: &lt::CompletionParams) -> Option<lt::CompletionResponse> {
        let position = params.text_document_position.position;
        let path = convert::uri_to_path(&params.text_document_position.text_document.uri)?;
        let (file, package) = self.locate(&path)?;
        let index = self.line_index_of(file);
        let offset = index.offset(position);
        let packages = self.search_packages();
        let items = if profile_of_path(&path) == Profile::Rsdl {
            let (mut items, replaced) = complete::rsdl_completion(
                &self.db,
                self.workspace,
                self.std,
                package,
                file,
                offset,
                &packages,
            );
            if let Some(replaced) = replaced {
                let range = index.range(replaced);
                for item in &mut items {
                    // An own-package item of a reference written qualified
                    // stays qualified (see `complete::rsdl_completion`).
                    let new_text = item
                        .filter_text
                        .clone()
                        .unwrap_or_else(|| item.label.clone());
                    item.text_edit = Some(lt::CompletionTextEdit::Edit(lt::TextEdit {
                        range,
                        new_text,
                    }));
                }
            }
            items
        } else {
            complete::completion(
                &self.db,
                self.workspace,
                self.std,
                package,
                file,
                offset,
                &packages,
            )
        };
        Some(lt::CompletionResponse::Array(items))
    }

    /// `textDocument/inlayHint`: the ordinal and unit-expansion hints inside the
    /// requested range, converted to LSP positions through the file's line
    /// table. A range request — only hints in the visible window are
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
    /// renameable symbol, so the client can validate before applying.
    ///
    /// In an `.rsdl` file, the name of a system, component, instance or
    /// declared interface under the cursor; nothing on a service or on an rsdl
    /// reference the checker binds to nothing.
    fn prepare_rename(
        &mut self,
        params: &lt::TextDocumentPositionParams,
    ) -> Option<lt::PrepareRenameResponse> {
        let path = convert::uri_to_path(&params.text_document.uri)?;
        let (file, package) = self.locate(&path)?;
        let offset = self.line_index_of(file).offset(params.position);
        let rsdl = if profile_of_path(&path) == Profile::Rsdl {
            rsdl::rename_at(&self.db, self.workspace, self.std, file, offset)
        } else {
            rsdl::RenameAt::Typl
        };
        let range = match rsdl {
            rsdl::RenameAt::Rsdl { span, .. } | rsdl::RenameAt::Interface { span, .. } => span,
            rsdl::RenameAt::Refused => return None,
            rsdl::RenameAt::Typl => {
                rename::prepare(&self.db, self.workspace, self.std, package, file, offset)?
            }
        };
        let lsp_range = self.line_index_of(file).range(range);
        Some(lt::PrepareRenameResponse::Range(lsp_range))
    }

    /// `textDocument/rename`: the workspace edit renaming the symbol under the
    /// cursor, or a [`RenameError`](rename::RenameError) the caller turns into an
    /// LSP error response.
    ///
    /// In an `.rsdl` file, a system, component or instance is renamed with its
    /// rsdl references and import lines, and a declared interface by the ridl
    /// rename; a service, and an rsdl reference the checker binds to nothing,
    /// are not renamed.
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
        if profile_of_path(&path) == Profile::Rsdl {
            match rsdl::rename_at(&self.db, self.workspace, self.std, file, offset) {
                rsdl::RenameAt::Rsdl { target, .. } => {
                    let edits = rsdl::rename(
                        &self.db,
                        self.workspace,
                        self.std,
                        &packages,
                        &target,
                        &params.new_name,
                    )?;
                    return Ok(self.workspace_edit(edits, &params.new_name));
                }
                rsdl::RenameAt::Interface { symbol, .. } => {
                    let edits = rename::rename_symbol(
                        &self.db,
                        self.workspace,
                        self.std,
                        &packages,
                        symbol,
                        &params.new_name,
                    )?;
                    return Ok(self.workspace_edit(edits, &params.new_name));
                }
                rsdl::RenameAt::Refused => return Err(rename::RenameError::NotRenameable),
                rsdl::RenameAt::Typl => {}
            }
        }
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

    /// `textDocument/formatting`: the `ridl fmt` rendering of the buffer, as one
    /// edit that replaces the whole document. An empty list when the buffer is
    /// already in canonical form; `None` (a `null` result) when it has parse
    /// errors, which `ridl fmt` also leaves untouched. The client's formatting
    /// options are ignored: the style is canonical. EditorConfig contributes
    /// only the width, resolved from the document's path.
    fn formatting(&mut self, params: &lt::DocumentFormattingParams) -> Option<Vec<lt::TextEdit>> {
        let path = convert::uri_to_path(&params.text_document.uri)?;
        let (file, _) = self.locate(&path)?;
        let text = file.text(&self.db);
        let ridl_fmt::FormatOutcome::Formatted(formatted) = ridl_fmt::format(
            text,
            profile_of_path(&path),
            &ridl_fmt::FormatOptions::for_path(std::path::Path::new(&path)),
        ) else {
            return None;
        };
        if formatted == *text {
            return Some(Vec::new());
        }
        let whole = TextRange::up_to(rowan::TextSize::of(text.as_str()));
        let range = self.line_index_of(file).range(whole);
        Some(vec![lt::TextEdit {
            range,
            new_text: formatted,
        }])
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

/// The key [`ServerState::shown_load_error`] compares: the manifest directory
/// the load started from and the reason it failed.
fn load_error_key(manifest_dir: &Path, err: &io::Error) -> String {
    format!("{}: {err}", manifest_dir.display())
}

/// One line of the remote-unit window message for `diagnostic`, an Error no
/// publish carries: for a spanned one, the file path, the 1-based line and
/// column, the code and the message, with the unit named when the loader has
/// not named it already; for a detached one, the code and the message.
fn remote_notice_line(
    diagnostic: &Diagnostic,
    sources: &SourceMap,
    units: &[(String, PathBuf)],
) -> String {
    let code = if diagnostic.code == DiagCode::NONE {
        String::new()
    } else {
        format!("[{}]", diagnostic.code.as_str())
    };
    let message = &diagnostic.message;
    let Some(path) = sources.path(diagnostic.primary.file) else {
        return format!("error{code}: {message}");
    };
    let LineCol { line, column } = line_col(
        sources.text(diagnostic.primary.file).unwrap_or_default(),
        diagnostic.primary.range.start(),
    );
    let unit = units
        .iter()
        .find(|(_, dir)| Path::new(path).starts_with(dir))
        .map(|(name, _)| name.as_str());
    let prefix = match unit {
        Some(name) if !message.starts_with("remote unit `") => format!("remote unit `{name}`: "),
        _ => String::new(),
    };
    format!("{path}:{line}:{column}: error{code}: {prefix}{message}")
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
/// rule.
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
