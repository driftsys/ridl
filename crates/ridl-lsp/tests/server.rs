//! Integration tests driving the server over an in-memory
//! `lsp_server::Connection` (docs/ROADMAP.md epic E1.15a): the full
//! initialize → didOpen → publishDiagnostics → didChange → shutdown
//! conversation, with the client side scripted by the test.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use lsp_server::{Connection, Message, Notification, Request, RequestId, Response};
use lsp_types as lt;
use lsp_types::notification::Notification as _;
use ridl_lsp::convert::path_to_uri;

/// A file with a duration literal inside a range constraint: the parser
/// reports FORM-101 (`expected `]``) and TYPL-302 (duration in a typl
/// context) on the `10ms` token — line 1, UTF-16 columns 24..28.
const BROKEN: &str = "package demo\ntype Speed: integer [0..10ms]\n";

/// A unique directory under the system temp dir, removed on drop.
struct TempDir(PathBuf);

impl TempDir {
    fn new(label: &str) -> Self {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let mut path = std::env::temp_dir();
        path.push(format!(
            "ridl-lsp-{label}-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::SeqCst),
        ));
        std::fs::create_dir_all(&path).expect("create the temp dir");
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }

    fn write(&self, relative: &str, text: &str) -> PathBuf {
        let path = self.0.join(relative);
        std::fs::write(&path, text).expect("write the fixture file");
        path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn uri_of(path: &Path) -> lt::Uri {
    path_to_uri(path.to_str().expect("fixture paths are UTF-8"))
        .expect("fixture paths are absolute")
}

fn recv(client: &Connection) -> Message {
    client
        .receiver
        .recv_timeout(Duration::from_secs(10))
        .expect("the server answers within the timeout")
}

/// Receives until the next `textDocument/publishDiagnostics` for `uri`.
fn next_publish(client: &Connection, uri: &lt::Uri) -> lt::PublishDiagnosticsParams {
    loop {
        if let Message::Notification(notification) = recv(client)
            && notification.method == lt::notification::PublishDiagnostics::METHOD
        {
            let params: lt::PublishDiagnosticsParams =
                serde_json::from_value(notification.params).expect("valid publish params");
            if params.uri.as_str() == uri.as_str() {
                return params;
            }
        }
    }
}

/// Receives until the next response, skipping notifications.
fn next_response(client: &Connection) -> Response {
    loop {
        if let Message::Response(response) = recv(client) {
            return response;
        }
    }
}

fn notify<N: lt::notification::Notification>(client: &Connection, params: N::Params) {
    client
        .sender
        .send(Notification::new(N::METHOD.to_string(), params).into())
        .expect("the server side is connected");
}

fn request<R: lt::request::Request>(client: &Connection, id: i32, params: R::Params) {
    client
        .sender
        .send(Request::new(RequestId::from(id), R::METHOD.to_string(), params).into())
        .expect("the server side is connected");
}

/// Performs the initialize handshake and returns the advertised capabilities.
fn initialize(client: &Connection, root: Option<lt::Uri>) -> lt::ServerCapabilities {
    let params = lt::InitializeParams {
        workspace_folders: root.map(|uri| {
            vec![lt::WorkspaceFolder {
                uri,
                name: "fixture".to_string(),
            }]
        }),
        ..Default::default()
    };
    request::<lt::request::Initialize>(client, 1, params);
    notify::<lt::notification::Initialized>(client, lt::InitializedParams {});
    let response = next_response(client);
    let result: lt::InitializeResult =
        serde_json::from_value(response.response_result.expect("initialize succeeds"))
            .expect("a valid InitializeResult");
    result.capabilities
}

fn shut_down(client: &Connection, id: i32) {
    request::<lt::request::Shutdown>(client, id, ());
    let response = next_response(client);
    assert!(response.response_result.is_ok(), "shutdown succeeds");
    notify::<lt::notification::Exit>(client, ());
}

fn codes(diagnostics: &[lt::Diagnostic]) -> Vec<&str> {
    diagnostics
        .iter()
        .map(|diagnostic| match &diagnostic.code {
            Some(lt::NumberOrString::String(code)) => code.as_str(),
            other => panic!("expected a string code, got {other:?}"),
        })
        .collect()
}

fn range(start: (u32, u32), end: (u32, u32)) -> lt::Range {
    lt::Range {
        start: lt::Position {
            line: start.0,
            character: start.1,
        },
        end: lt::Position {
            line: end.0,
            character: end.1,
        },
    }
}

fn pos(line: u32, character: u32) -> lt::Position {
    lt::Position { line, character }
}

fn text_position(uri: lt::Uri, position: lt::Position) -> lt::TextDocumentPositionParams {
    lt::TextDocumentPositionParams {
        text_document: lt::TextDocumentIdentifier { uri },
        position,
    }
}

/// Sends a hover request and returns the parsed result.
fn hover_at(
    client: &Connection,
    id: i32,
    uri: lt::Uri,
    position: lt::Position,
) -> Option<lt::Hover> {
    request::<lt::request::HoverRequest>(
        client,
        id,
        lt::HoverParams {
            text_document_position_params: text_position(uri, position),
            work_done_progress_params: Default::default(),
        },
    );
    let response = next_response(client);
    serde_json::from_value(response.response_result.expect("hover succeeds"))
        .expect("a valid hover result")
}

/// Sends a goto-definition request and returns the parsed result.
fn definition_at(
    client: &Connection,
    id: i32,
    uri: lt::Uri,
    position: lt::Position,
) -> Option<lt::GotoDefinitionResponse> {
    request::<lt::request::GotoDefinition>(
        client,
        id,
        lt::GotoDefinitionParams {
            text_document_position_params: text_position(uri, position),
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        },
    );
    let response = next_response(client);
    serde_json::from_value(response.response_result.expect("definition succeeds"))
        .expect("a valid definition result")
}

/// Sends a references request and returns the parsed result.
fn references_at(
    client: &Connection,
    id: i32,
    uri: lt::Uri,
    position: lt::Position,
    include_declaration: bool,
) -> Option<Vec<lt::Location>> {
    request::<lt::request::References>(
        client,
        id,
        lt::ReferenceParams {
            text_document_position: text_position(uri, position),
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
            context: lt::ReferenceContext {
                include_declaration,
            },
        },
    );
    let response = next_response(client);
    serde_json::from_value(response.response_result.expect("references succeed"))
        .expect("a valid references result")
}

/// The two-member fixture workspace: `veh.common` declares `Speed` (a
/// unit-typed, constrained type with a doc comment), the constant `MAX_SPEED`
/// used as a range bound in `Cruise`, and a `Gearbox` struct with a reserved
/// tombstone between its two fields; `app` imports and uses `veh.common.Speed`.
const VEH_COMMON: &str = "package veh.common\n\
/// Vehicle speed over ground\n\
type Speed: km/h [0.0..250.0 step 0.5]\n\
const MAX_SPEED: float = 250.0\n\
type Cruise: float [0.0..MAX_SPEED step 0.5]\n\
struct Gearbox { primary: Speed, reserved oldRatio, secondary: Speed }\n";

const APP: &str = "package app\n\
import veh.common.Speed\n\
struct Cabin { primary: Speed }\n";

/// Writes the two-member workspace to `dir` and returns the two file URIs.
fn write_workspace(dir: &TempDir) -> (lt::Uri, lt::Uri) {
    dir.write(
        "ridl.toml",
        "[workspace]\nmembers = [\"veh-common\", \"app\"]\n",
    );
    std::fs::create_dir_all(dir.path().join("veh-common")).expect("create veh-common");
    std::fs::create_dir_all(dir.path().join("app")).expect("create app");
    dir.write(
        "veh-common/ridl.toml",
        "[package]\nname = \"veh.common\"\nversion = \"1.0.0\"\n",
    );
    let veh = dir.write("veh-common/lib.typl", VEH_COMMON);
    dir.write(
        "app/ridl.toml",
        "[package]\nname = \"app\"\nversion = \"1.0.0\"\n",
    );
    let app = dir.write("app/lib.typl", APP);
    (uri_of(&veh), uri_of(&app))
}

/// The server thread's join handle: it returns the server loop's result.
type ServerThread = std::thread::JoinHandle<Result<(), Box<dyn std::error::Error + Send + Sync>>>;

/// Spawns the server over an in-memory connection loaded at `root`.
fn start(root: lt::Uri) -> (Connection, ServerThread) {
    let (server_side, client) = Connection::memory();
    let server = std::thread::spawn(move || ridl_lsp::server::run(server_side));
    initialize(&client, Some(root));
    (client, server)
}

/// Hover on `Speed` shows its unit, range, derived width, and doc comment,
/// read across the package boundary from `veh.common`'s IR.
#[test]
fn hover_on_a_type_reference_shows_unit_range_width_and_doc() {
    let dir = TempDir::new("hover");
    let (_veh, app) = write_workspace(&dir);
    let root = uri_of(dir.path());
    let (client, server) = start(root);

    // `primary: Speed` on line 2 of `app/lib.typl`; the `Speed` reference
    // starts at UTF-16 column 24.
    let hover = hover_at(&client, 10, app, pos(2, 26)).expect("Speed has hover content");
    let value = match hover.contents {
        lt::HoverContents::Markup(markup) => {
            assert_eq!(markup.kind, lt::MarkupKind::Markdown);
            markup.value
        }
        other => panic!("expected markdown hover, got {other:?}"),
    };
    assert!(
        value.contains("veh.common.Speed"),
        "qualified name: {value}"
    );
    assert!(value.contains("km/h"), "canonical unit: {value}");
    // The checker stores canonical decimals, so `0.0..250.0` reads `0..250`.
    assert!(value.contains("[0..250 step 0.5]"), "constraint: {value}");
    assert!(value.contains("f32"), "derived wire width: {value}");
    assert!(
        value.contains("Vehicle speed over ground"),
        "doc comment: {value}",
    );

    shut_down(&client, 11);
    server.join().expect("thread joins").expect("clean exit");
}

/// Goto-definition on the `Speed` reference in `app` jumps to its declaration
/// in `veh.common` — resolved across the package boundary through the import.
#[test]
fn goto_definition_crosses_the_package_boundary() {
    let dir = TempDir::new("goto");
    let (veh, app) = write_workspace(&dir);
    let root = uri_of(dir.path());
    let (client, server) = start(root);

    let response =
        definition_at(&client, 10, app, pos(2, 26)).expect("Speed resolves to a definition");
    let location = match response {
        lt::GotoDefinitionResponse::Scalar(location) => location,
        other => panic!("expected a single location, got {other:?}"),
    };
    assert_eq!(
        location.uri.as_str(),
        veh.as_str(),
        "definition is in veh.common's file",
    );
    // `type Speed` on line 2 of `veh-common/lib.typl`: the name spans columns
    // 5..10.
    assert_eq!(
        location.range,
        range((2, 5), (2, 10)),
        "the `Speed` name span"
    );

    shut_down(&client, 11);
    server.join().expect("thread joins").expect("clean exit");
}

/// Find-references on the constant `MAX_SPEED` finds its use inside the
/// `Cruise` range constraint — a resolved reference, not a textual match.
#[test]
fn find_references_finds_a_constant_bound_reference() {
    let dir = TempDir::new("refs");
    let (veh, _app) = write_workspace(&dir);
    let root = uri_of(dir.path());
    let (client, server) = start(root);

    // `const MAX_SPEED` on line 3 of `veh-common/lib.typl`: the name starts at
    // column 6.
    let locations = references_at(&client, 10, veh.clone(), pos(3, 8), false)
        .expect("MAX_SPEED resolves to a symbol");
    assert_eq!(locations.len(), 1, "one reference, got: {locations:?}");
    assert_eq!(locations[0].uri.as_str(), veh.as_str());
    // `[0.0..MAX_SPEED step 0.5]` on line 4: `MAX_SPEED` spans columns 25..34.
    assert_eq!(
        locations[0].range,
        range((4, 25), (4, 34)),
        "the reference inside the constraint",
    );

    shut_down(&client, 11);
    server.join().expect("thread joins").expect("clean exit");
}

/// Hover on a struct field shows its derived ordinal (general form §6.3).
#[test]
fn hover_on_a_struct_field_shows_its_ordinal() {
    let dir = TempDir::new("ordinal");
    let (veh, _app) = write_workspace(&dir);
    let root = uri_of(dir.path());
    let (client, server) = start(root);

    // `struct Gearbox { primary: Speed, reserved oldRatio, secondary: Speed }`
    // on line 5: `secondary` starts at column 52. It is the third slot — the
    // reserved tombstone between the two fields keeps ordinal 2, so `secondary`
    // is ordinal 3. This pins that hover counts reserved slots.
    let hover = hover_at(&client, 10, veh, pos(5, 54)).expect("the field has hover content");
    let value = match hover.contents {
        lt::HoverContents::Markup(markup) => markup.value,
        other => panic!("expected markdown hover, got {other:?}"),
    };
    assert!(value.contains("secondary"), "names the field: {value}");
    assert!(
        value.contains("#3"),
        "shows the tombstone-counted ordinal: {value}"
    );

    shut_down(&client, 11);
    server.join().expect("thread joins").expect("clean exit");
}

/// Opens a standalone overlay from `text` and returns the hover markdown at
/// `position`, or `None`.
fn open_and_hover(
    client: &Connection,
    id: i32,
    uri: &lt::Uri,
    text: &str,
    position: lt::Position,
) -> Option<String> {
    notify::<lt::notification::DidOpenTextDocument>(
        client,
        lt::DidOpenTextDocumentParams {
            text_document: lt::TextDocumentItem {
                uri: uri.clone(),
                language_id: "ridl".to_string(),
                version: 0,
                text: text.to_string(),
            },
        },
    );
    hover_at(client, id, uri.clone(), position).map(|hover| match hover.contents {
        lt::HoverContents::Markup(markup) => markup.value,
        other => panic!("expected markdown hover, got {other:?}"),
    })
}

/// Closing and reopening an overlay with shifted text still hovers at the new
/// position: the cached line table is dropped on close, so the reopened
/// document's positions map through its own text (IMPORTANT-1 regression).
#[test]
fn reopening_a_shifted_overlay_hovers_at_the_new_position() {
    let uri = path_to_uri("/ridl-lsp-reopen/solo.typl").expect("an absolute synthetic path");

    let (server_side, client) = Connection::memory();
    let server = std::thread::spawn(move || ridl_lsp::server::run(server_side));
    initialize(&client, None);

    // First open: `type Widget` is on line 1; hover lands on the declaration.
    let first = "package solo\ntype Widget: integer [0..10]\n";
    let before = open_and_hover(&client, 10, &uri, first, pos(1, 7)).expect("Widget hovers");
    assert!(
        before.contains("Widget"),
        "first hover names Widget: {before}"
    );

    notify::<lt::notification::DidCloseTextDocument>(
        &client,
        lt::DidCloseTextDocumentParams {
            text_document: lt::TextDocumentIdentifier { uri: uri.clone() },
        },
    );

    // Reopen the same path with a blank leading line, so `type Widget` is now
    // on line 2. Hovering there must still land — a stale line table would map
    // this position through the old text and miss.
    let shifted = "\npackage solo\ntype Widget: integer [0..10]\n";
    let after = open_and_hover(&client, 11, &uri, shifted, pos(2, 7))
        .expect("Widget still hovers after the shift");
    assert!(
        after.contains("Widget"),
        "reopened hover names Widget: {after}"
    );

    shut_down(&client, 12);
    server.join().expect("thread joins").expect("clean exit");
}

/// Hover renders an open-start (`[..100]`) and an open-end (`[0..]`) range —
/// the checker lowers each with one bound absent, and hover must still show the
/// constraint (IMPORTANT-2 regression; ADR-0004 §10).
#[test]
fn hover_renders_open_ended_ranges() {
    let uri = path_to_uri("/ridl-lsp-edge/edge.typl").expect("an absolute synthetic path");

    let (server_side, client) = Connection::memory();
    let server = std::thread::spawn(move || ridl_lsp::server::run(server_side));
    initialize(&client, None);

    let text = "package edge\ntype Partial: integer [..100]\ntype Lower: integer [0..]\n";
    notify::<lt::notification::DidOpenTextDocument>(
        &client,
        lt::DidOpenTextDocumentParams {
            text_document: lt::TextDocumentItem {
                uri: uri.clone(),
                language_id: "ridl".to_string(),
                version: 0,
                text: text.to_string(),
            },
        },
    );

    // `type Partial` on line 1: the name starts at column 5.
    let partial = hover_at(&client, 10, uri.clone(), pos(1, 7))
        .expect("Partial hovers")
        .contents;
    let partial = match partial {
        lt::HoverContents::Markup(markup) => markup.value,
        other => panic!("expected markdown hover, got {other:?}"),
    };
    assert!(
        partial.contains("[..100]"),
        "open-start range is shown: {partial}",
    );

    // `type Lower` on line 2: the name starts at column 5.
    let lower = hover_at(&client, 11, uri.clone(), pos(2, 7))
        .expect("Lower hovers")
        .contents;
    let lower = match lower {
        lt::HoverContents::Markup(markup) => markup.value,
        other => panic!("expected markdown hover, got {other:?}"),
    };
    assert!(lower.contains("[0..]"), "open-end range is shown: {lower}",);

    shut_down(&client, 12);
    server.join().expect("thread joins").expect("clean exit");
}

/// Two packages each declare `Speed`; find-references on one package's `Speed`
/// returns only that package's own reference, never the other's — the
/// name-resolution property T24 rename depends on (MINOR-5).
#[test]
fn find_references_does_not_cross_a_name_collision() {
    let dir = TempDir::new("collision");
    dir.write(
        "ridl.toml",
        "[workspace]\nmembers = [\"alpha\", \"beta\"]\n",
    );
    std::fs::create_dir_all(dir.path().join("alpha")).expect("create alpha");
    std::fs::create_dir_all(dir.path().join("beta")).expect("create beta");
    dir.write(
        "alpha/ridl.toml",
        "[package]\nname = \"alpha\"\nversion = \"1.0.0\"\n",
    );
    let alpha = dir.write(
        "alpha/lib.typl",
        "package alpha\ntype Speed: km/h\nstruct A { primary: Speed }\n",
    );
    dir.write(
        "beta/ridl.toml",
        "[package]\nname = \"beta\"\nversion = \"1.0.0\"\n",
    );
    let beta = dir.write(
        "beta/lib.typl",
        "package beta\ntype Speed: m/s\nstruct B { primary: Speed }\n",
    );
    let alpha_uri = uri_of(&alpha);
    let beta_uri = uri_of(&beta);
    let (client, server) = start(uri_of(dir.path()));

    // `type Speed` on line 1 of `alpha/lib.typl`: the name starts at column 5.
    let locations = references_at(&client, 10, alpha_uri.clone(), pos(1, 7), false)
        .expect("alpha's Speed resolves");
    assert_eq!(
        locations.len(),
        1,
        "only alpha's reference, got: {locations:?}"
    );
    assert_eq!(
        locations[0].uri.as_str(),
        alpha_uri.as_str(),
        "the reference is in alpha, not beta",
    );
    assert_ne!(
        locations[0].uri.as_str(),
        beta_uri.as_str(),
        "beta's identically named Speed is not a reference to alpha's",
    );
    // `struct A { primary: Speed }` on line 2: `Speed` spans columns 20..25.
    assert_eq!(locations[0].range, range((2, 20), (2, 25)));

    shut_down(&client, 11);
    server.join().expect("thread joins").expect("clean exit");
}

/// The full conversation over a loaded workspace: initialize advertises
/// incremental sync and quick-fix code actions; opening the broken file
/// publishes the two parser codes with correct ranges; an incremental edit
/// that removes the `ms` suffix publishes an empty list; a code-action
/// request answers (no fix-it-carrying diagnostic exists yet, so the list is
/// empty); shutdown ends the loop.
#[test]
fn diagnostics_and_code_actions_over_an_in_memory_connection() {
    let dir = TempDir::new("workspace");
    dir.write(
        "ridl.toml",
        "[package]\nname = \"demo\"\nversion = \"1.0.0\"\n",
    );
    let file = dir.write("demo.typl", BROKEN);
    let root_uri = uri_of(dir.path());
    let file_uri = uri_of(&file);

    let (server_side, client) = Connection::memory();
    let server = std::thread::spawn(move || ridl_lsp::server::run(server_side));

    let capabilities = initialize(&client, Some(root_uri));
    assert_eq!(
        capabilities.text_document_sync,
        Some(lt::TextDocumentSyncCapability::Options(
            lt::TextDocumentSyncOptions {
                open_close: Some(true),
                change: Some(lt::TextDocumentSyncKind::INCREMENTAL),
                ..Default::default()
            }
        )),
        "the server advertises incremental text sync",
    );
    match &capabilities.code_action_provider {
        Some(lt::CodeActionProviderCapability::Options(options)) => assert_eq!(
            options.code_action_kinds.as_deref(),
            Some([lt::CodeActionKind::QUICKFIX].as_slice()),
            "the server advertises quick-fix code actions",
        ),
        other => panic!("expected code action options, got {other:?}"),
    }

    // The initial load already analyzed the workspace: the broken file's
    // diagnostics arrive without the file being open.
    let initial = next_publish(&client, &file_uri);
    assert_eq!(codes(&initial.diagnostics), vec!["FORM-101", "TYPL-302"]);

    // Opening the file (same text as disk) republishes the same findings.
    notify::<lt::notification::DidOpenTextDocument>(
        &client,
        lt::DidOpenTextDocumentParams {
            text_document: lt::TextDocumentItem {
                uri: file_uri.clone(),
                language_id: "ridl".to_string(),
                version: 0,
                text: BROKEN.to_string(),
            },
        },
    );
    let opened = next_publish(&client, &file_uri);
    assert_eq!(codes(&opened.diagnostics), vec!["FORM-101", "TYPL-302"]);
    for diagnostic in &opened.diagnostics {
        assert_eq!(
            diagnostic.range,
            range((1, 24), (1, 28)),
            "both diagnostics point at the `10ms` token",
        );
        assert_eq!(diagnostic.severity, Some(lt::DiagnosticSeverity::ERROR));
        assert_eq!(diagnostic.source.as_deref(), Some("ridl"));
    }

    // A code-action request answers; no current diagnostic carries a fix-it,
    // so the list is empty (the conversion itself is unit-tested against a
    // synthetic fix-it in `convert`).
    request::<lt::request::CodeActionRequest>(
        &client,
        2,
        lt::CodeActionParams {
            text_document: lt::TextDocumentIdentifier {
                uri: file_uri.clone(),
            },
            range: range((1, 24), (1, 28)),
            context: lt::CodeActionContext::default(),
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        },
    );
    let actions = next_response(&client);
    assert_eq!(
        actions.response_result.expect("code actions succeed"),
        serde_json::json!([]),
        "no fix-it-carrying diagnostic exists, so no quick fix is offered",
    );

    // An incremental change deleting `ms` fixes the file: the next publish
    // for it carries an empty list.
    notify::<lt::notification::DidChangeTextDocument>(
        &client,
        lt::DidChangeTextDocumentParams {
            text_document: lt::VersionedTextDocumentIdentifier {
                uri: file_uri.clone(),
                version: 1,
            },
            content_changes: vec![lt::TextDocumentContentChangeEvent {
                range: Some(range((1, 26), (1, 28))),
                range_length: None,
                text: String::new(),
            }],
        },
    );
    let fixed = next_publish(&client, &file_uri);
    assert_eq!(
        fixed.diagnostics,
        Vec::new(),
        "the fixed file publishes an empty diagnostic list",
    );

    shut_down(&client, 3);
    server
        .join()
        .expect("the server thread joins")
        .expect("the server loop exits cleanly");
}

/// A file opened outside any workspace (no root, nothing on disk) becomes a
/// standalone overlay: its unsaved buffer is analyzed, and closing it clears
/// its diagnostics.
#[test]
fn a_standalone_overlay_file_is_analyzed_from_its_buffer() {
    let file_uri =
        path_to_uri("/ridl-lsp-nowhere/lonely.typl").expect("an absolute synthetic path");

    let (server_side, client) = Connection::memory();
    let server = std::thread::spawn(move || ridl_lsp::server::run(server_side));

    initialize(&client, None);

    notify::<lt::notification::DidOpenTextDocument>(
        &client,
        lt::DidOpenTextDocumentParams {
            text_document: lt::TextDocumentItem {
                uri: file_uri.clone(),
                language_id: "ridl".to_string(),
                version: 0,
                text: BROKEN.to_string(),
            },
        },
    );
    let opened = next_publish(&client, &file_uri);
    assert_eq!(
        codes(&opened.diagnostics),
        vec!["FORM-101", "TYPL-302"],
        "the unsaved buffer is analyzed without touching the filesystem",
    );

    notify::<lt::notification::DidCloseTextDocument>(
        &client,
        lt::DidCloseTextDocumentParams {
            text_document: lt::TextDocumentIdentifier {
                uri: file_uri.clone(),
            },
        },
    );
    let closed = next_publish(&client, &file_uri);
    assert_eq!(
        closed.diagnostics,
        Vec::new(),
        "closing a standalone overlay clears its diagnostics",
    );

    shut_down(&client, 2);
    server
        .join()
        .expect("the server thread joins")
        .expect("the server loop exits cleanly");
}

// ==========================================================================
// E1.15c — completion and rename (task 24)
// ==========================================================================

/// Opens `text` as the buffer for `uri` (a workspace file or a standalone
/// overlay).
fn did_open(client: &Connection, uri: &lt::Uri, text: &str) {
    notify::<lt::notification::DidOpenTextDocument>(
        client,
        lt::DidOpenTextDocumentParams {
            text_document: lt::TextDocumentItem {
                uri: uri.clone(),
                language_id: "ridl".to_string(),
                version: 0,
                text: text.to_string(),
            },
        },
    );
}

/// Sends a completion request and returns the offered items.
fn complete_at(
    client: &Connection,
    id: i32,
    uri: lt::Uri,
    position: lt::Position,
) -> Vec<lt::CompletionItem> {
    request::<lt::request::Completion>(
        client,
        id,
        lt::CompletionParams {
            text_document_position: text_position(uri, position),
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
            context: None,
        },
    );
    let response = next_response(client);
    let parsed: Option<lt::CompletionResponse> =
        serde_json::from_value(response.response_result.expect("completion succeeds"))
            .expect("a valid completion result");
    match parsed {
        Some(lt::CompletionResponse::Array(items)) => items,
        Some(lt::CompletionResponse::List(list)) => list.items,
        None => Vec::new(),
    }
}

/// The labels of a set of completion items.
fn labels(items: &[lt::CompletionItem]) -> Vec<&str> {
    items.iter().map(|item| item.label.as_str()).collect()
}

/// The kind of the item labelled `label`, if present.
fn kind_of(items: &[lt::CompletionItem], label: &str) -> Option<lt::CompletionItemKind> {
    items
        .iter()
        .find(|item| item.label == label)
        .and_then(|item| item.kind)
}

/// Sends a rename request and returns the raw response (so a rejection can be
/// inspected as an error).
fn rename_raw(
    client: &Connection,
    id: i32,
    uri: lt::Uri,
    position: lt::Position,
    new_name: &str,
) -> Response {
    request::<lt::request::Rename>(
        client,
        id,
        lt::RenameParams {
            text_document_position: text_position(uri, position),
            new_name: new_name.to_string(),
            work_done_progress_params: Default::default(),
        },
    );
    next_response(client)
}

/// Sends a rename request that is expected to succeed and returns the edit.
fn rename_at(
    client: &Connection,
    id: i32,
    uri: lt::Uri,
    position: lt::Position,
    new_name: &str,
) -> lt::WorkspaceEdit {
    let response = rename_raw(client, id, uri, position, new_name);
    let parsed: Option<lt::WorkspaceEdit> =
        serde_json::from_value(response.response_result.expect("rename succeeds"))
            .expect("a valid rename result");
    parsed.expect("rename produces a workspace edit")
}

/// The text edits a workspace edit carries for `uri`, sorted by start position.
fn edits_for(edit: &lt::WorkspaceEdit, uri: &lt::Uri) -> Vec<lt::TextEdit> {
    // `WorkspaceEdit.changes` is keyed by `lsp_types::Uri`, whose inner cache
    // cell trips `mutable_key_type`; the key's identity never mutates.
    #[allow(clippy::mutable_key_type)]
    let changes = edit.changes.as_ref().expect("the edit carries changes");
    let mut edits = changes
        .iter()
        .find(|(key, _)| key.as_str() == uri.as_str())
        .map(|(_, edits)| edits.clone())
        .unwrap_or_default();
    edits.sort_by_key(|edit| (edit.range.start.line, edit.range.start.character));
    edits
}

/// Sends a prepareRename request and returns the parsed response.
fn prepare_rename_at(
    client: &Connection,
    id: i32,
    uri: lt::Uri,
    position: lt::Position,
) -> Option<lt::PrepareRenameResponse> {
    request::<lt::request::PrepareRenameRequest>(client, id, text_position(uri, position));
    let response = next_response(client);
    serde_json::from_value(response.response_result.expect("prepareRename succeeds"))
        .expect("a valid prepareRename result")
}

/// (a) Completion after `:` in a field type position offers the visible named
/// types (locals + `ridl.std`) and the primitives, each kind-annotated.
#[test]
fn completion_after_colon_offers_types_and_primitives() {
    let uri = path_to_uri("/ridl-lsp-complete/types.typl").expect("an absolute synthetic path");
    let (server_side, client) = Connection::memory();
    let server = std::thread::spawn(move || ridl_lsp::server::run(server_side));
    initialize(&client, None);

    // `struct Holder { item: }` on line 2; the colon is at column 20, so a
    // cursor at column 22 sits just past `: `.
    let text = "package demo\ntype Speed: integer [0..10]\nstruct Holder { item: }\n";
    did_open(&client, &uri, text);
    let items = complete_at(&client, 10, uri.clone(), pos(2, 22));
    let offered = labels(&items);

    assert!(
        offered.contains(&"Speed"),
        "local type offered: {offered:?}"
    );
    assert!(
        offered.contains(&"Timestamp"),
        "a ridl.std type offered: {offered:?}",
    );
    assert!(
        offered.contains(&"integer"),
        "a primitive offered: {offered:?}",
    );
    assert_eq!(
        kind_of(&items, "Speed"),
        Some(lt::CompletionItemKind::CLASS),
        "a named type is kind-annotated as a type",
    );
    assert_eq!(
        kind_of(&items, "integer"),
        Some(lt::CompletionItemKind::KEYWORD),
        "a primitive is kind-annotated as a keyword",
    );

    shut_down(&client, 11);
    server.join().expect("thread joins").expect("clean exit");
}

/// (b) Completion after `import` offers the known package names, and after a
/// completed package path offers that package's public symbols.
#[test]
fn completion_after_import_offers_packages_and_symbols() {
    let dir = TempDir::new("complete-import");
    let (_veh, app) = write_workspace(&dir);
    let (client, server) = start(uri_of(dir.path()));

    // `import ` on line 1, cursor just past the keyword and its space.
    did_open(&client, &app, "package app\nimport \n");
    let packages = complete_at(&client, 10, app.clone(), pos(1, 7));
    let package_labels = labels(&packages);
    assert!(
        package_labels.contains(&"veh.common"),
        "a workspace package is offered: {package_labels:?}",
    );
    assert_eq!(
        kind_of(&packages, "veh.common"),
        Some(lt::CompletionItemKind::MODULE),
        "a package name is kind-annotated as a module",
    );

    // `import veh.common.` on line 1, cursor just past the final dot.
    did_open(&client, &app, "package app\nimport veh.common.\n");
    let symbols = complete_at(&client, 12, app.clone(), pos(1, 18));
    let symbol_labels = labels(&symbols);
    assert!(
        symbol_labels.contains(&"Speed"),
        "the package's public symbol is offered: {symbol_labels:?}",
    );

    shut_down(&client, 13);
    server.join().expect("thread joins").expect("clean exit");
}

/// (c) Completion at a definition-start position offers the definition
/// keywords and the modifiers.
#[test]
fn completion_at_definition_start_offers_keywords() {
    let uri = path_to_uri("/ridl-lsp-complete/keywords.typl").expect("an absolute synthetic path");
    let (server_side, client) = Connection::memory();
    let server = std::thread::spawn(move || ridl_lsp::server::run(server_side));
    initialize(&client, None);

    // A blank top-level line after the package declaration.
    let text = "package demo\n\n";
    did_open(&client, &uri, text);
    let items = complete_at(&client, 10, uri.clone(), pos(1, 0));
    let offered = labels(&items);

    for keyword in [
        "type", "const", "struct", "enum", "enumset", "union", "internal", "error",
    ] {
        assert!(
            offered.contains(&keyword),
            "definition keyword `{keyword}` offered: {offered:?}",
        );
    }
    assert_eq!(
        kind_of(&items, "struct"),
        Some(lt::CompletionItemKind::KEYWORD),
        "a definition keyword is kind-annotated as a keyword",
    );

    shut_down(&client, 11);
    server.join().expect("thread joins").expect("clean exit");
}

/// (c') Completion inside a `match` constraint offers the regex constants in
/// scope (a local one and a `ridl.std` pattern).
#[test]
fn completion_inside_match_offers_regex_consts() {
    let uri = path_to_uri("/ridl-lsp-complete/match.typl").expect("an absolute synthetic path");
    let (server_side, client) = Connection::memory();
    let server = std::thread::spawn(move || ridl_lsp::server::run(server_side));
    initialize(&client, None);

    // `type Code: string [1..8 match ]` on line 2: `match ` ends at column 30,
    // so a cursor there sits just past the keyword and its space.
    let text = "package demo\nconst CODE_PATTERN = /^[A-Z]+$/\ntype Code: string [1..8 match ]\n";
    did_open(&client, &uri, text);
    let items = complete_at(&client, 10, uri.clone(), pos(2, 30));
    let offered = labels(&items);

    assert!(
        offered.contains(&"CODE_PATTERN"),
        "a local regex const offered: {offered:?}",
    );
    assert!(
        offered.contains(&"UUID_PATTERN"),
        "a ridl.std regex const offered: {offered:?}",
    );
    // A non-regex const is not offered as a pattern.
    assert!(
        !offered.contains(&"MAX_SPEED"),
        "only regex consts are offered: {offered:?}",
    );

    shut_down(&client, 11);
    server.join().expect("thread joins").expect("clean exit");
}

/// (d) Renaming `Speed` across the two-package workspace also rewrites the
/// `import veh.common.Speed` line in `app` — the load-bearing case.
#[test]
fn rename_across_packages_updates_the_import_line() {
    let dir = TempDir::new("rename-import");
    let (veh, app) = write_workspace(&dir);
    let (client, server) = start(uri_of(dir.path()));

    // `type Speed` on line 2 of `veh-common/lib.typl`: the name spans 5..10.
    let edit = rename_at(&client, 10, veh.clone(), pos(2, 7), "Velocity");

    // The declaration and both struct references in veh.common are rewritten.
    let veh_edits = edits_for(&edit, &veh);
    assert_eq!(
        veh_edits.len(),
        3,
        "declaration + two references: {veh_edits:?}"
    );
    assert!(
        veh_edits.iter().all(|edit| edit.new_text == "Velocity"),
        "every veh edit inserts the new name: {veh_edits:?}",
    );
    assert_eq!(
        veh_edits[0].range,
        range((2, 5), (2, 10)),
        "the declaration"
    );

    // The import line AND the struct reference in app are rewritten.
    let app_edits = edits_for(&edit, &app);
    assert_eq!(
        app_edits.len(),
        2,
        "import line + one reference: {app_edits:?}"
    );
    assert!(
        app_edits.iter().all(|edit| edit.new_text == "Velocity"),
        "every app edit inserts the new name: {app_edits:?}",
    );
    // `import veh.common.Speed` on line 1: `Speed` spans columns 18..23.
    assert_eq!(
        app_edits[0].range,
        range((1, 18), (1, 23)),
        "the import line's imported-name segment is rewritten",
    );

    shut_down(&client, 11);
    server.join().expect("thread joins").expect("clean exit");
}

/// (e) Rename invoked from the import line itself works: the cursor sits on the
/// imported name in `import veh.common.Speed`, where `symbol_at` finds nothing,
/// so the import-path entry point resolves the target.
#[test]
fn rename_from_the_import_line_works() {
    let dir = TempDir::new("rename-from-import");
    let (veh, app) = write_workspace(&dir);
    let (client, server) = start(uri_of(dir.path()));

    // `import veh.common.Speed` on line 1 of `app/lib.typl`: `Speed` is at 18.
    let edit = rename_at(&client, 10, app.clone(), pos(1, 20), "Velocity");

    let veh_edits = edits_for(&edit, &veh);
    assert_eq!(
        veh_edits.len(),
        3,
        "the declaration and its two references are found from the import: {veh_edits:?}",
    );
    let app_edits = edits_for(&edit, &app);
    assert_eq!(
        app_edits.len(),
        2,
        "the import and its reference: {app_edits:?}"
    );

    shut_down(&client, 11);
    server.join().expect("thread joins").expect("clean exit");
}

/// (f) Renaming a qualified reference `veh.common.Speed` rewrites only the
/// final `Speed` segment, never the whole qualified name.
#[test]
fn rename_of_a_qualified_reference_rewrites_only_the_last_segment() {
    let dir = TempDir::new("rename-qualified");
    dir.write(
        "ridl.toml",
        "[workspace]\nmembers = [\"veh-common\", \"app\"]\n",
    );
    std::fs::create_dir_all(dir.path().join("veh-common")).expect("create veh-common");
    std::fs::create_dir_all(dir.path().join("app")).expect("create app");
    dir.write(
        "veh-common/ridl.toml",
        "[package]\nname = \"veh.common\"\nversion = \"1.0.0\"\n",
    );
    let veh = dir.write(
        "veh-common/lib.typl",
        "package veh.common\ntype Speed: km/h\n",
    );
    dir.write(
        "app/ridl.toml",
        "[package]\nname = \"app\"\nversion = \"1.0.0\"\n",
    );
    // A qualified reference, no import statement.
    let app = dir.write(
        "app/lib.typl",
        "package app\nstruct Cabin { primary: veh.common.Speed }\n",
    );
    let veh_uri = uri_of(&veh);
    let app_uri = uri_of(&app);
    let (client, server) = start(uri_of(dir.path()));

    let edit = rename_at(&client, 10, veh_uri.clone(), pos(1, 7), "Velocity");

    let app_edits = edits_for(&edit, &app_uri);
    assert_eq!(app_edits.len(), 1, "one qualified reference: {app_edits:?}");
    let edited = &app_edits[0];
    assert_eq!(edited.new_text, "Velocity");
    assert_eq!(
        edited.range.end.character - edited.range.start.character,
        5,
        "only the 5-character `Speed` segment is edited, not the whole path",
    );

    shut_down(&client, 11);
    server.join().expect("thread joins").expect("clean exit");
}

/// (g) Renaming into the reserved word `signal` is rejected.
#[test]
fn rename_into_a_reserved_word_is_rejected() {
    let dir = TempDir::new("rename-reserved");
    let (veh, _app) = write_workspace(&dir);
    let (client, server) = start(uri_of(dir.path()));

    let response = rename_raw(&client, 10, veh.clone(), pos(2, 7), "signal");
    assert!(
        response.response_result.is_err(),
        "renaming to a reserved word fails: {response:?}",
    );

    shut_down(&client, 11);
    server.join().expect("thread joins").expect("clean exit");
}

/// (h) Renaming `Speed` to `Cruise`, already declared in the same package, is
/// rejected as a collision.
#[test]
fn rename_introducing_a_duplicate_is_rejected() {
    let dir = TempDir::new("rename-dup");
    let (veh, _app) = write_workspace(&dir);
    let (client, server) = start(uri_of(dir.path()));

    let response = rename_raw(&client, 10, veh.clone(), pos(2, 7), "Cruise");
    assert!(
        response.response_result.is_err(),
        "renaming onto an existing declaration fails: {response:?}",
    );

    shut_down(&client, 11);
    server.join().expect("thread joins").expect("clean exit");
}

/// (i) Renaming a type to a name that violates the CamelCase convention (R7)
/// is rejected.
#[test]
fn rename_violating_the_case_convention_is_rejected() {
    let dir = TempDir::new("rename-case");
    let (veh, _app) = write_workspace(&dir);
    let (client, server) = start(uri_of(dir.path()));

    // A type must stay CamelCase; `velocity` is lowercase.
    let response = rename_raw(&client, 10, veh.clone(), pos(2, 7), "velocity");
    assert!(
        response.response_result.is_err(),
        "renaming a type to a lowercase name fails: {response:?}",
    );

    shut_down(&client, 11);
    server.join().expect("thread joins").expect("clean exit");
}

/// prepareRename returns the name span on a renameable symbol and nothing on a
/// non-symbol position, so the client rejects an invalid rename early.
#[test]
fn prepare_rename_validates_the_cursor() {
    let dir = TempDir::new("prepare-rename");
    let (veh, _app) = write_workspace(&dir);
    let (client, server) = start(uri_of(dir.path()));

    let on_name = prepare_rename_at(&client, 10, veh.clone(), pos(2, 7));
    match on_name {
        Some(lt::PrepareRenameResponse::Range(returned)) => {
            assert_eq!(returned, range((2, 5), (2, 10)), "the `Speed` name span");
        }
        other => panic!("expected a rename range, got {other:?}"),
    }

    // The `package` keyword is not a renameable symbol.
    let on_keyword = prepare_rename_at(&client, 11, veh.clone(), pos(0, 2));
    assert!(on_keyword.is_none(), "a keyword position is not renameable");

    shut_down(&client, 12);
    server.join().expect("thread joins").expect("clean exit");
}

/// (CRITICAL-1 regression) Renaming a symbol imported under an alias rewrites
/// the import line's imported-name segment but leaves the alias usage intact —
/// the alias binds the local name, so rewriting the usage would unbind it.
#[test]
fn rename_leaves_an_alias_usage_intact() {
    let dir = TempDir::new("rename-alias");
    dir.write(
        "ridl.toml",
        "[workspace]\nmembers = [\"veh-common\", \"app\"]\n",
    );
    std::fs::create_dir_all(dir.path().join("veh-common")).expect("create veh-common");
    std::fs::create_dir_all(dir.path().join("app")).expect("create app");
    dir.write(
        "veh-common/ridl.toml",
        "[package]\nname = \"veh.common\"\nversion = \"1.0.0\"\n",
    );
    let veh = dir.write(
        "veh-common/lib.typl",
        "package veh.common\ntype Speed: km/h\n",
    );
    dir.write(
        "app/ridl.toml",
        "[package]\nname = \"app\"\nversion = \"1.0.0\"\n",
    );
    // `Speed` is imported under the alias `Velocity`, then used as `Velocity`.
    let app = dir.write(
        "app/lib.typl",
        "package app\nimport veh.common.Speed as Velocity\nstruct Cabin { primary: Velocity }\n",
    );
    let veh_uri = uri_of(&veh);
    let app_uri = uri_of(&app);
    let (client, server) = start(uri_of(dir.path()));

    let edit = rename_at(&client, 10, veh_uri.clone(), pos(1, 7), "Rapidity");

    // The declaration in veh.common is rewritten.
    let veh_edits = edits_for(&edit, &veh_uri);
    assert_eq!(veh_edits.len(), 1, "the declaration only: {veh_edits:?}");
    assert_eq!(veh_edits[0].new_text, "Rapidity");

    // In app, ONLY the import line's imported-name segment is rewritten. The
    // `Velocity` usage on line 2 is left untouched — rewriting it would leave
    // `Rapidity` unbound.
    let app_edits = edits_for(&edit, &app_uri);
    assert_eq!(
        app_edits.len(),
        1,
        "only the import segment, not the alias usage: {app_edits:?}",
    );
    // `import veh.common.Speed as Velocity` on line 1: `Speed` spans 18..23.
    assert_eq!(
        app_edits[0].range,
        range((1, 18), (1, 23)),
        "the import segment"
    );
    assert_eq!(app_edits[0].new_text, "Rapidity");
    assert!(
        app_edits.iter().all(|edit| edit.range.start.line != 2),
        "no edit touches the `Velocity` usage on line 2: {app_edits:?}",
    );

    shut_down(&client, 11);
    server.join().expect("thread joins").expect("clean exit");
}

/// (CRITICAL-2 regression) A `ridl.std` symbol is not renameable: prepareRename
/// returns null and rename fails, so no partial edit that drops the built-in
/// declaration while rewriting user references is ever produced.
#[test]
fn a_std_symbol_is_not_renameable() {
    let uri = path_to_uri("/ridl-lsp-std/std.typl").expect("an absolute synthetic path");
    let (server_side, client) = Connection::memory();
    let server = std::thread::spawn(move || ridl_lsp::server::run(server_side));
    initialize(&client, None);

    // A user struct referencing the built-in `ridl.std` type `Timestamp`.
    did_open(&client, &uri, "package demo\nstruct S { at: Timestamp }\n");

    // `Timestamp` on line 1 starts at column 15.
    let prepared = prepare_rename_at(&client, 10, uri.clone(), pos(1, 16));
    assert!(
        prepared.is_none(),
        "a ridl.std symbol cannot be renamed: {prepared:?}",
    );

    // Rename itself fails rather than emitting a partial edit.
    let response = rename_raw(&client, 11, uri.clone(), pos(1, 16), "Instant");
    assert!(
        response.response_result.is_err(),
        "renaming a std symbol fails: {response:?}",
    );

    shut_down(&client, 12);
    server.join().expect("thread joins").expect("clean exit");
}

// --- inlay hints (E1.16) -------------------------------------------------

/// Sends an inlay-hint range request and returns the parsed hints.
fn inlay_hints_at(
    client: &Connection,
    id: i32,
    uri: lt::Uri,
    range: lt::Range,
) -> Vec<lt::InlayHint> {
    request::<lt::request::InlayHintRequest>(
        client,
        id,
        lt::InlayHintParams {
            work_done_progress_params: Default::default(),
            text_document: lt::TextDocumentIdentifier { uri },
            range,
        },
    );
    let response = next_response(client);
    let parsed: Option<Vec<lt::InlayHint>> =
        serde_json::from_value(response.response_result.expect("inlay hints succeed"))
            .expect("a valid inlay-hint result");
    parsed.unwrap_or_default()
}

/// The string label of an inlay hint (every hint the server emits is a plain
/// string label).
fn label(hint: &lt::InlayHint) -> &str {
    match &hint.label {
        lt::InlayHintLabel::String(text) => text.as_str(),
        other => panic!("expected a string label, got {other:?}"),
    }
}

/// The `(position, label)` pairs of the hints of the given kind, in order.
fn hint_pairs(hints: &[lt::InlayHint], kind: lt::InlayHintKind) -> Vec<(lt::Position, String)> {
    hints
        .iter()
        .filter(|hint| hint.kind == Some(kind))
        .map(|hint| (hint.position, label(hint).to_string()))
        .collect()
}

/// A whole-file range: line 0 through a line past the end, which the server's
/// `LineIndex` clamps to the end of the text.
fn whole_file() -> lt::Range {
    range((0, 0), (1_000, 0))
}

/// The server advertises the inlay-hint capability, closing the E1.16 LSP
/// feature set.
#[test]
fn advertises_the_inlay_hint_capability() {
    let (server_side, client) = Connection::memory();
    let server = std::thread::spawn(move || ridl_lsp::server::run(server_side));
    let capabilities = initialize(&client, None);
    assert!(
        matches!(
            capabilities.inlay_hint_provider,
            Some(lt::OneOf::Left(true))
        ),
        "inlay hints advertised: {:?}",
        capabilities.inlay_hint_provider,
    );
    shut_down(&client, 1);
    server.join().expect("thread joins").expect("clean exit");
}

/// The §7.4 tombstone struct: the two live fields render their derived
/// ordinals, and the reserved slot between them is counted, so `speed` is `#3`,
/// not `#2` — a reorder is visibly a renumbering (general form §6.3).
#[test]
fn inlay_hints_render_struct_field_ordinals_counting_the_tombstone() {
    let uri = path_to_uri("/ridl-lsp-inlay-struct/solo.typl").expect("an absolute synthetic path");
    let (server_side, client) = Connection::memory();
    let server = std::thread::spawn(move || ridl_lsp::server::run(server_side));
    initialize(&client, None);

    // `struct DriverProfile { name / reserved legacyChecksum / speed }` — the
    // §7.4 example. `Name` and `Speed` are primitive-backed, so no unit hints.
    did_open(
        &client,
        &uri,
        "package demo\n\
         type Name: string [1..64]\n\
         type Speed: integer [0..250]\n\
         struct DriverProfile {\n\
        \x20 name: Name\n\
        \x20 reserved legacyChecksum\n\
        \x20 speed: Speed\n\
         }\n",
    );

    let hints = inlay_hints_at(&client, 10, uri.clone(), whole_file());
    // `name` on line 4 (cols 2..6) is #1; `speed` on line 6 (cols 2..7) is #3 —
    // the tombstone on line 5 occupies slot 2.
    assert_eq!(
        hint_pairs(&hints, lt::InlayHintKind::PARAMETER),
        vec![(pos(4, 6), "#1".to_string()), (pos(6, 7), "#3".to_string()),],
        "field ordinals count the reserved tombstone",
    );

    shut_down(&client, 11);
    server.join().expect("thread joins").expect("clean exit");
}

/// A unit-typed `type` renders the unit's human reading after the UCUM code,
/// from `UcumExpr::display_name` over the canonical unit the checker stored.
#[test]
fn inlay_hint_expands_a_unit_typed_declaration() {
    let uri = path_to_uri("/ridl-lsp-inlay-unit/solo.typl").expect("an absolute synthetic path");
    let (server_side, client) = Connection::memory();
    let server = std::thread::spawn(move || ridl_lsp::server::run(server_side));
    initialize(&client, None);

    did_open(
        &client,
        &uri,
        "package demo\ntype Speed: km/h [0.0..250.0 step 0.5]\n",
    );

    let hints = inlay_hints_at(&client, 10, uri.clone(), whole_file());
    // `km/h` on line 1 spans cols 12..16; the reading is anchored after it.
    assert_eq!(
        hint_pairs(&hints, lt::InlayHintKind::TYPE),
        vec![(pos(1, 16), "\u{27e8}kilometer per hour\u{27e9}".to_string())],
        "the unit expansion reads the canonical UCUM unit",
    );

    shut_down(&client, 11);
    server.join().expect("thread joins").expect("clean exit");
}

/// An enum's values render the number that is their wire identity — the
/// explicit integer value, read from the IR, not a positional ordinal. `PARK`
/// is the first value yet renders `#0`, because its transport identity is `0`.
#[test]
fn inlay_hints_render_enum_value_wire_numbers() {
    let uri = path_to_uri("/ridl-lsp-inlay-enum/solo.typl").expect("an absolute synthetic path");
    let (server_side, client) = Connection::memory();
    let server = std::thread::spawn(move || ridl_lsp::server::run(server_side));
    initialize(&client, None);

    did_open(
        &client,
        &uri,
        "package demo\n\
         enum GearPosition {\n\
        \x20 PARK = 0\n\
        \x20 DRIVE = 1\n\
        \x20 REVERSE = 2\n\
         }\n",
    );

    let hints = inlay_hints_at(&client, 10, uri.clone(), whole_file());
    assert_eq!(
        hint_pairs(&hints, lt::InlayHintKind::PARAMETER),
        vec![
            (pos(2, 6), "#0".to_string()), // PARK, cols 2..6
            (pos(3, 7), "#1".to_string()), // DRIVE, cols 2..7
            (pos(4, 9), "#2".to_string()), // REVERSE, cols 2..9
        ],
        "enum values render their wire value, not a declaration-order position",
    );

    shut_down(&client, 11);
    server.join().expect("thread joins").expect("clean exit");
}

/// A union's arms render their declaration-order ordinal. The request is a
/// range request scoped to the union body, so the fields of the two structs on
/// earlier lines fall outside the window and are excluded.
#[test]
fn inlay_hint_renders_union_arm_ordinals_within_the_requested_range() {
    let uri = path_to_uri("/ridl-lsp-inlay-union/solo.typl").expect("an absolute synthetic path");
    let (server_side, client) = Connection::memory();
    let server = std::thread::spawn(move || ridl_lsp::server::run(server_side));
    initialize(&client, None);

    did_open(
        &client,
        &uri,
        "package demo\n\
         struct Reading { value: integer [0..100] }\n\
         struct Fault { code: integer [0..255] }\n\
         union SensorResult {\n\
        \x20 ok: Reading\n\
        \x20 err: Fault\n\
         }\n",
    );

    // Lines 4..5 are the union arms; the struct fields sit on lines 1..2, out
    // of the requested window.
    let hints = inlay_hints_at(&client, 10, uri.clone(), range((4, 0), (6, 0)));
    assert_eq!(
        hint_pairs(&hints, lt::InlayHintKind::PARAMETER),
        vec![
            (pos(4, 4), "#1".to_string()), // `ok`, cols 2..4
            (pos(5, 5), "#2".to_string()), // `err`, cols 2..5
        ],
        "only the in-range union arm ordinals are returned",
    );

    shut_down(&client, 11);
    server.join().expect("thread joins").expect("clean exit");
}

// --- the ridl interaction layer (E2.10b) ---------------------------------

/// The typl vocabulary the ridl fixture contract imports.
const RIDL_VOCAB: &str = "package veh.common\n\
/// Vehicle speed over ground\n\
type Speed: km/h [0.0..250.0 step 0.5]\n\
type Version: string [0..32]\n";

/// The ridl fixture contract: one interface holding all five interaction
/// kinds plus a `reserved` tombstone at ordinal 6, a named-shape service, and
/// an inline-shape service.
///
/// `engineTemp`'s payload sits behind a block comment holding a two-byte `°`,
/// so its UTF-16 column and its byte offset differ — the fixture that pins the
/// `convert` bridge on the ridl side.
const RIDL_CONTRACT: &str = "package veh.cluster\n\
\n\
import veh.common.Speed\n\
import veh.common.Version\n\
\n\
struct DoorPayload {\n\
\x20 sensorId : integer [0..15]\n\
\x20 isOpen   : boolean\n\
}\n\
\n\
struct CalReport {\n\
\x20 offset : integer [0..100]\n\
}\n\
\n\
error enum CalError {\n\
\x20 SENSOR_UNAVAILABLE = 0\n\
}\n\
\n\
/// Main vehicle status contract shape.\n\
interface VehicleStatus {\n\
\x20 /// Current vehicle speed\n\
\x20 signal currentSpeed : Speed @10ms\n\
\x20 signal cabinLoad : Speed\n\
\x20 signal engineTemp : /* \u{b0}C */ Speed @[20ms..100ms]\n\
\x20 event doorOpened : DoorPayload @[50ms..500ms]\n\
\x20 command setGear(position: Speed)\n\
\x20 reserved resetCounters\n\
\x20 query calibrate(axle: Speed): CalReport | CalError\n\
\x20 final softwareVersion : Version\n\
}\n\
\n\
service veh.adas.cruise : VehicleStatus\n\
\n\
service veh.hvac.cabin {\n\
\x20 signal temperature : Speed @[1s..10s]\n\
\x20 command setTarget(t: Speed)\n\
}\n";

/// Writes the two-member ridl fixture workspace to `dir` and returns the
/// `(vocabulary, contract)` file URIs.
fn write_ridl_workspace(dir: &TempDir) -> (lt::Uri, lt::Uri) {
    dir.write(
        "ridl.toml",
        "[workspace]\nmembers = [\"veh-common\", \"cluster\"]\n",
    );
    std::fs::create_dir_all(dir.path().join("veh-common")).expect("create veh-common");
    std::fs::create_dir_all(dir.path().join("cluster")).expect("create cluster");
    dir.write(
        "veh-common/ridl.toml",
        "[package]\nname = \"veh.common\"\nversion = \"1.0.0\"\n",
    );
    let vocab = dir.write("veh-common/lib.typl", RIDL_VOCAB);
    dir.write(
        "cluster/ridl.toml",
        "[package]\nname = \"veh.cluster\"\nversion = \"1.0.0\"\n",
    );
    let contract = dir.write("cluster/contract.ridl", RIDL_CONTRACT);
    (uri_of(&vocab), uri_of(&contract))
}

/// The UTF-16 LSP position of the start of the `occurrence`-th (0-based)
/// match of `needle` in `text`. Keeps the ridl tests readable without hand
/// counting columns; the UTF-16 test below pins one column literally as well,
/// so the helper itself cannot mask a conversion bug.
fn find_pos(text: &str, needle: &str, occurrence: usize) -> lt::Position {
    let byte = text
        .match_indices(needle)
        .nth(occurrence)
        .unwrap_or_else(|| panic!("`{needle}` occurs at least {} times", occurrence + 1))
        .0;
    let line = text[..byte].matches('\n').count() as u32;
    let line_start = text[..byte].rfind('\n').map_or(0, |index| index + 1);
    let character = text[line_start..byte].encode_utf16().count() as u32;
    lt::Position { line, character }
}

/// The hover markdown at `position`, or a panic naming what was missing.
fn hover_markdown(client: &Connection, id: i32, uri: lt::Uri, position: lt::Position) -> String {
    let hover = hover_at(client, id, uri, position).expect("the position has hover content");
    match hover.contents {
        lt::HoverContents::Markup(markup) => markup.value,
        other => panic!("expected markdown hover, got {other:?}"),
    }
}

/// Hover on a signal renders its kind, resolved payload, ordinal, and the
/// resolved strict-periodic timing with the per-kind reading general form
/// §6.2 derives for a state interaction.
#[test]
fn hover_on_a_signal_shows_kind_payload_ordinal_and_the_signal_reading() {
    let dir = TempDir::new("ridl-hover-signal");
    let (_vocab, contract) = write_ridl_workspace(&dir);
    let root = uri_of(dir.path());
    let (client, server) = start(root);

    let value = hover_markdown(
        &client,
        10,
        contract,
        find_pos(RIDL_CONTRACT, "currentSpeed", 0),
    );
    assert!(value.contains("signal"), "kind: {value}");
    assert!(value.contains("veh.common.Speed"), "payload: {value}");
    assert!(value.contains("km/h"), "payload unit: {value}");
    assert!(
        value.contains("[0..250 step 0.5]"),
        "payload range: {value}"
    );
    assert!(value.contains("#1"), "ordinal: {value}");
    assert!(value.contains("strict periodic"), "timing mode: {value}");
    assert!(value.contains("10ms"), "timing bound: {value}");
    assert!(
        value.contains("min = rate floor (debounce), max = staleness bound (refresh ceiling)"),
        "the general form §6.2 signal reading: {value}",
    );

    shut_down(&client, 11);
    server.join().expect("thread joins").expect("clean exit");
}

/// Hover on an event renders the occurrence reading — throttle and TTL —
/// which general form §6.2 derives from the declaring keyword, not from the
/// annotation.
#[test]
fn hover_on_an_event_shows_the_throttle_and_ttl_reading() {
    let dir = TempDir::new("ridl-hover-event");
    let (_vocab, contract) = write_ridl_workspace(&dir);
    let root = uri_of(dir.path());
    let (client, server) = start(root);

    let value = hover_markdown(
        &client,
        10,
        contract,
        find_pos(RIDL_CONTRACT, "doorOpened", 0),
    );
    assert!(value.contains("event"), "kind: {value}");
    assert!(value.contains("DoorPayload"), "payload: {value}");
    assert!(value.contains("#4"), "ordinal: {value}");
    assert!(value.contains("[50ms..500ms]"), "timing bounds: {value}");
    assert!(
        value.contains(
            "min = rate floor (throttle), max = staleness bound \
             (TTL: stale occurrences discarded)"
        ),
        "the general form §6.2 event reading: {value}",
    );

    shut_down(&client, 11);
    server.join().expect("thread joins").expect("clean exit");
}

/// An untimed signal is not untimed in the IR: the configured default is
/// resolved at compile time, and hover says so (ridl §9.1).
#[test]
fn hover_on_an_untimed_signal_shows_the_applied_default() {
    let dir = TempDir::new("ridl-hover-default");
    let (_vocab, contract) = write_ridl_workspace(&dir);
    let root = uri_of(dir.path());
    let (client, server) = start(root);

    let value = hover_markdown(
        &client,
        10,
        contract,
        find_pos(RIDL_CONTRACT, "cabinLoad", 0),
    );
    assert!(value.contains("#2"), "ordinal: {value}");
    assert!(
        value.contains("default [100ms..1000ms] applied"),
        "the applied default: {value}",
    );

    shut_down(&client, 11);
    server.join().expect("thread joins").expect("clean exit");
}

/// Hover on a command renders its kind, ordinal, and resolved parameters.
#[test]
fn hover_on_a_command_shows_its_ordinal_and_parameters() {
    let dir = TempDir::new("ridl-hover-command");
    let (_vocab, contract) = write_ridl_workspace(&dir);
    let root = uri_of(dir.path());
    let (client, server) = start(root);

    let value = hover_markdown(&client, 10, contract, find_pos(RIDL_CONTRACT, "setGear", 0));
    assert!(value.contains("command"), "kind: {value}");
    assert!(
        value.contains("position: veh.common.Speed"),
        "parameter: {value}",
    );
    assert!(value.contains("#5"), "ordinal: {value}");

    shut_down(&client, 11);
    server.join().expect("thread joins").expect("clean exit");
}

/// Hover on a `final` renders its kind, payload, and ordinal — the tombstone
/// before it is counted, so it is `#8`, not `#7`.
#[test]
fn hover_on_a_final_shows_its_payload_and_tombstone_counted_ordinal() {
    let dir = TempDir::new("ridl-hover-final");
    let (_vocab, contract) = write_ridl_workspace(&dir);
    let root = uri_of(dir.path());
    let (client, server) = start(root);

    let value = hover_markdown(
        &client,
        10,
        contract,
        find_pos(RIDL_CONTRACT, "softwareVersion", 0),
    );
    assert!(value.contains("final"), "kind: {value}");
    assert!(value.contains("veh.common.Version"), "payload: {value}");
    assert!(
        value.contains("#8"),
        "the tombstone-counted ordinal: {value}"
    );

    shut_down(&client, 11);
    server.join().expect("thread joins").expect("clean exit");
}

/// Hover on a fallible query names both arms and closes with the general form
/// §6.4 wording — Stratum 3 is detected, not undefined.
#[test]
fn hover_on_a_fallible_query_names_both_arms_and_the_stratum_three_wording() {
    let dir = TempDir::new("ridl-hover-query");
    let (_vocab, contract) = write_ridl_workspace(&dir);
    let root = uri_of(dir.path());
    let (client, server) = start(root);

    let value = hover_markdown(
        &client,
        10,
        contract,
        find_pos(RIDL_CONTRACT, "calibrate", 0),
    );
    assert!(value.contains("query"), "kind: {value}");
    assert!(value.contains("#7"), "ordinal: {value}");
    assert!(value.contains("CalReport"), "the ok arm: {value}");
    assert!(value.contains("CalError"), "the error arm: {value}");
    assert!(
        value.contains("infrastructure failure — detected, undeclared"),
        "the verbatim general form §6.4 sentence: {value}",
    );
    assert!(
        !value.contains("undefined behavior") && !value.contains("undefined behaviour"),
        "Stratum 3 is never described as undefined behavior: {value}",
    );

    shut_down(&client, 11);
    server.join().expect("thread joins").expect("clean exit");
}

/// Hover on the `|` of an inline fallible return renders the same two-arm
/// reading and the same §6.4 wording as the query hover.
#[test]
fn hover_on_the_fallible_pipe_shows_both_arms_and_the_strata_note() {
    let dir = TempDir::new("ridl-hover-pipe");
    let (_vocab, contract) = write_ridl_workspace(&dir);
    let root = uri_of(dir.path());
    let (client, server) = start(root);

    let value = hover_markdown(
        &client,
        10,
        contract,
        find_pos(RIDL_CONTRACT, "| CalError", 0),
    );
    assert!(value.contains("CalReport"), "the ok arm: {value}");
    assert!(value.contains("CalError"), "the error arm: {value}");
    assert!(
        value.contains("infrastructure failure — detected, undeclared"),
        "the verbatim general form §6.4 sentence: {value}",
    );

    shut_down(&client, 11);
    server.join().expect("thread joins").expect("clean exit");
}

/// Hover on a service names its interface shape and states the ridl §14.5
/// posture neutrality — a service says nothing about its wire realization.
#[test]
fn hover_on_a_service_shows_its_interface_and_the_posture_note() {
    let dir = TempDir::new("ridl-hover-service");
    let (_vocab, contract) = write_ridl_workspace(&dir);
    let root = uri_of(dir.path());
    let (client, server) = start(root);

    let value = hover_markdown(
        &client,
        10,
        contract.clone(),
        find_pos(RIDL_CONTRACT, "veh.adas.cruise", 0),
    );
    assert!(value.contains("service"), "kind: {value}");
    assert!(value.contains("veh.adas.cruise"), "service name: {value}");
    assert!(value.contains("VehicleStatus"), "interface shape: {value}");
    assert!(value.contains("Posture-neutral"), "the §14.5 note: {value}");

    // The inline-shape form reports its own member count instead of a name.
    let inline = hover_markdown(
        &client,
        12,
        contract,
        find_pos(RIDL_CONTRACT, "veh.hvac.cabin", 0),
    );
    assert!(inline.contains("inline"), "inline shape: {inline}");
    assert!(
        inline.contains("Posture-neutral"),
        "the §14.5 note: {inline}"
    );

    shut_down(&client, 13);
    server.join().expect("thread joins").expect("clean exit");
}

/// Ordinal inlay hints number every interaction of an interface body and the
/// `reserved` tombstone among them — the editor half of the general form §6.3
/// mitigation.
#[test]
fn inlay_hints_number_every_interaction_and_the_reserved_tombstone() {
    let dir = TempDir::new("ridl-inlay-interface");
    let (_vocab, contract) = write_ridl_workspace(&dir);
    let root = uri_of(dir.path());
    let (client, server) = start(root);

    let hints = inlay_hints_at(&client, 10, contract, whole_file());
    let ordinals = hint_pairs(&hints, lt::InlayHintKind::PARAMETER);
    let labels: Vec<&str> = ordinals
        .iter()
        .map(|(_, label)| label.as_str())
        .filter(|label| label.starts_with('#'))
        .collect();
    // The struct and enum bodies contribute their own numbers first; the eight
    // interface slots follow in declaration order, tombstone included.
    assert!(
        labels
            .windows(8)
            .any(|window| window == ["#1", "#2", "#3", "#4", "#5", "#6", "#7", "#8"]),
        "eight contiguous interface ordinals: {labels:?}",
    );

    let at = |needle: &str| {
        let start = find_pos(RIDL_CONTRACT, needle, 0);
        pos(
            start.line,
            start.character + needle.encode_utf16().count() as u32,
        )
    };
    for (needle, expected) in [
        ("currentSpeed", "#1"),
        ("cabinLoad", "#2"),
        ("engineTemp", "#3"),
        ("doorOpened", "#4"),
        ("setGear", "#5"),
        ("resetCounters", "#6"),
        ("calibrate", "#7"),
        ("softwareVersion", "#8"),
    ] {
        assert!(
            ordinals.contains(&(at(needle), expected.to_string())),
            "`{needle}` carries {expected}: {ordinals:?}",
        );
    }

    shut_down(&client, 11);
    server.join().expect("thread joins").expect("clean exit");
}

/// A service's inline shape carries its own ordinal sequence, and the inlay
/// hints render it — an inline shape is an interface body in every way that
/// matters to wire identity.
#[test]
fn inlay_hints_number_a_service_inline_shape() {
    let dir = TempDir::new("ridl-inlay-service");
    let (_vocab, contract) = write_ridl_workspace(&dir);
    let root = uri_of(dir.path());
    let (client, server) = start(root);

    let hints = inlay_hints_at(&client, 10, contract, whole_file());
    let ordinals = hint_pairs(&hints, lt::InlayHintKind::PARAMETER);
    let at = |needle: &str| {
        let start = find_pos(RIDL_CONTRACT, needle, 0);
        pos(
            start.line,
            start.character + needle.encode_utf16().count() as u32,
        )
    };
    assert!(
        ordinals.contains(&(at("temperature"), "#1".to_string())),
        "the inline shape's first member: {ordinals:?}",
    );
    assert!(
        ordinals.contains(&(at("setTarget"), "#2".to_string())),
        "the inline shape's second member: {ordinals:?}",
    );

    shut_down(&client, 11);
    server.join().expect("thread joins").expect("clean exit");
}

/// Goto-definition on an interaction's payload reference crosses the package
/// boundary to the typl declaration.
#[test]
fn goto_definition_on_an_interaction_payload_crosses_packages() {
    let dir = TempDir::new("ridl-goto-payload");
    let (vocab, contract) = write_ridl_workspace(&dir);
    let root = uri_of(dir.path());
    let (client, server) = start(root);

    // The `Speed` payload of `signal currentSpeed`.
    let position = find_pos(RIDL_CONTRACT, "currentSpeed : Speed", 0);
    let position = pos(position.line, position.character + 16);
    let response = definition_at(&client, 10, contract, position).expect("the payload resolves");
    let location = match response {
        lt::GotoDefinitionResponse::Scalar(location) => location,
        other => panic!("expected a single location, got {other:?}"),
    };
    assert_eq!(
        location.uri.as_str(),
        vocab.as_str(),
        "declared in veh.common"
    );
    assert_eq!(
        location.range,
        range((2, 5), (2, 10)),
        "the `Speed` name span",
    );

    shut_down(&client, 11);
    server.join().expect("thread joins").expect("clean exit");
}

/// Goto-definition on a service's interface reference jumps to the interface
/// declaration, and find-references from the interface finds that reference.
#[test]
fn goto_definition_and_references_work_on_a_service_interface_reference() {
    let dir = TempDir::new("ridl-goto-service");
    let (_vocab, contract) = write_ridl_workspace(&dir);
    let root = uri_of(dir.path());
    let (client, server) = start(root);

    let reference = find_pos(RIDL_CONTRACT, "VehicleStatus", 1);
    let declaration = find_pos(RIDL_CONTRACT, "VehicleStatus", 0);
    let response = definition_at(
        &client,
        10,
        contract.clone(),
        pos(reference.line, reference.character + 2),
    )
    .expect("the interface reference resolves");
    let location = match response {
        lt::GotoDefinitionResponse::Scalar(location) => location,
        other => panic!("expected a single location, got {other:?}"),
    };
    assert_eq!(location.uri.as_str(), contract.as_str());
    assert_eq!(
        location.range,
        range(
            (declaration.line, declaration.character),
            (declaration.line, declaration.character + 13),
        ),
        "the `VehicleStatus` name span",
    );

    let locations = references_at(
        &client,
        12,
        contract.clone(),
        pos(declaration.line, declaration.character + 2),
        false,
    )
    .expect("the interface is a symbol");
    assert_eq!(
        locations.len(),
        1,
        "the service's shape reference, got: {locations:?}",
    );
    assert_eq!(
        locations[0].range.start, reference,
        "the reference inside the service declaration",
    );

    shut_down(&client, 13);
    server.join().expect("thread joins").expect("clean exit");
}

/// Completion after a `:` in payload position offers the visible named types,
/// and completion at an interaction-start position offers the five kind
/// keywords plus `reserved`.
#[test]
fn completion_inside_an_interface_body_covers_both_contexts() {
    let dir = TempDir::new("ridl-completion");
    let (_vocab, contract) = write_ridl_workspace(&dir);
    let root = uri_of(dir.path());
    let (client, server) = start(root);

    // Just after the `: ` of `signal currentSpeed : Speed`.
    let payload = find_pos(RIDL_CONTRACT, "currentSpeed : Speed", 0);
    let items = complete_at(
        &client,
        10,
        contract.clone(),
        pos(payload.line, payload.character + 15),
    );
    let types = labels(&items);
    assert!(types.contains(&"Speed"), "named types: {types:?}");
    assert!(
        types.contains(&"DoorPayload"),
        "local named types: {types:?}",
    );

    // The start of the `reserved resetCounters` line — an interaction-start
    // position inside the interface body.
    let start = find_pos(RIDL_CONTRACT, "reserved resetCounters", 0);
    let items = complete_at(&client, 12, contract, pos(start.line, start.character));
    let keywords = labels(&items);
    for keyword in ["signal", "event", "command", "query", "final", "reserved"] {
        assert!(
            keywords.contains(&keyword),
            "`{keyword}` offered at an interaction start: {keywords:?}",
        );
    }

    shut_down(&client, 13);
    server.join().expect("thread joins").expect("clean exit");
}

/// A payload reference preceded on its own line by a two-byte `°` resolves at
/// its UTF-16 column, not its byte offset — the `convert` bridge is the only
/// place positions cross, on the ridl side too.
#[test]
fn hover_on_a_payload_after_a_multibyte_comment_uses_utf16_columns() {
    let dir = TempDir::new("ridl-utf16");
    let (_vocab, contract) = write_ridl_workspace(&dir);
    let root = uri_of(dir.path());
    let (client, server) = start(root);

    // `  signal engineTemp : /* °C */ Speed @[20ms..100ms]` — `Speed` starts
    // at UTF-16 column 31 but byte offset 32 within the line.
    let payload = find_pos(RIDL_CONTRACT, "*/ Speed", 0);
    let payload = pos(payload.line, payload.character + 3);
    assert_eq!(payload.character, 31, "the UTF-16 column of `Speed`");

    let hover = hover_at(
        &client,
        10,
        contract,
        pos(payload.line, payload.character + 2),
    )
    .expect("the payload reference has hover content");
    let value = match hover.contents {
        lt::HoverContents::Markup(markup) => markup.value,
        other => panic!("expected markdown hover, got {other:?}"),
    };
    assert!(
        value.contains("veh.common.Speed"),
        "resolved payload: {value}"
    );
    assert_eq!(
        hover.range.expect("hover reports its range").start,
        payload,
        "the reference span starts at the UTF-16 column",
    );

    shut_down(&client, 11);
    server.join().expect("thread joins").expect("clean exit");
}
