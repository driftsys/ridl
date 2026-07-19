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
