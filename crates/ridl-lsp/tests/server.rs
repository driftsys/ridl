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
