# ridl-mcp v0 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give the `ridl` CLI two new subcommands — `ridl lsp` (the existing
language server) and `ridl mcp` (a new MCP server with one tool, `ridl_check`) —
plus the JSON diagnostic contract both `ridl mcp` and a new
`ridl check --format json` emit.

**Architecture:** One new library crate, `crates/ridl-mcp`, built on the `rmcp`
SDK, calls a new `ridlc::check_source` (the front half of the existing
`ridlc::compile`, without the backend) and returns diagnostics through a new
`ridl_core::diag::to_json`. The `ridl` binary hosts both servers as subcommands;
`crates/ridl-lsp` keeps its own binary for now (the VS Code extension still
spawns `ridl-lsp` until the distribution plan switches it).

**Tech Stack:** Rust 2024 edition workspace, `clap` 4 (derive), `rmcp` (server +
macros + stdio transport; client + child-process transport in dev-dependencies),
`schemars`, `serde`/`serde_json`, `tokio`, `lsp-server` 0.10.

**Spec:** `docs/wip/2026-09-13-ridl-mcp-v0-design.md` (read it first; the plan
argues from it).

## Global Constraints

- Work on a branch in a git worktree, never on `main`; open a PR at the end
  (AGENTS.md).
- Conventional Commits, scopes from `.git-std.toml`: `ridl-core`, `ridlc`,
  `ridl`, `ridl-mcp` (added in Task 4), `repo`, `docs`.
- Every gate must stay green: `just build` runs toolchain-check, gate-parity,
  fmt-check, book-check, link-check, compile (`--locked`), test (`--locked`),
  lint (`clippy --all-targets -D warnings`), wasm-check, check. Run
  `cargo fmt --all` before every commit.
- `wasm-check` builds `ridl-syntax ridl-core ridl-sem ridl-ir ridl-backend-*`;
  nothing in this plan adds a dependency to those crates except pure-Rust code
  in `ridl-core` that uses only `serde` (already a dependency).
- `Cargo.lock` changes are committed with the task that causes them (`--locked`
  gates).
- Prose in code comments, commit messages, and docs is plain and literal: no
  idioms (AGENTS.md).
- `ridl_check` accepts `profile: "typl" | "ridl"` only — `rxdl` is not a
  compiler profile (spec §2).
- Tool output is the JSON contract of spec §2: `code`, `severity`
  (`error`/`warning`/`info`), `message`, `span` (`path`, 1-based `start`/`end`
  `line`/`column`), `fixes` (`label`, `replacement`, `span`).
- Do not delete `crates/ridl-lsp/src/main.rs` in this plan: the extension spawns
  `ridl-lsp` until the distribution plan changes it.

---

### Task 1: The JSON diagnostic contract in `ridl-core`

**Files:**

- Modify: `crates/ridl-core/src/diag.rs` (append after `impl SourceMap`, around
  line 930; the `remap_diagnostics` function follows it)
- Test: `crates/ridl-core/src/diag.rs` (a `#[cfg(test)] mod json_tests` at the
  end of the file)

**Interfaces:**

- Consumes:
  `Diagnostic { code: DiagCode(pub &'static str), severity: Severity, message: String, primary: Span { file: FileId, range: TextRange }, labels, fixits: Vec<FixIt { span, replacement, label }> }`,
  `SourceMap::file_id(&mut self, path, text) -> FileId`,
  `SourceMap::path(&self, FileId) -> Option<&str>` — all already in `diag.rs`.
- Produces: `SourceMap::text(&self, id: FileId) -> Option<&str>`;
  `pub struct LineCol { pub line: u32, pub column: u32 }`;
  `pub fn line_col(text: &str, offset: TextSize) -> LineCol`;
  `pub struct JsonSpan { pub path: String, pub start: LineCol, pub end: LineCol }`;
  `pub struct JsonFixIt { pub label: String, pub replacement: String, pub span: JsonSpan }`;
  `pub struct JsonDiagnostic { pub code: String, pub severity: String, pub message: String, pub span: JsonSpan, pub fixes: Vec<JsonFixIt> }`;
  `pub fn to_json(diagnostics: &[Diagnostic], sources: &SourceMap) -> Vec<JsonDiagnostic>`.
  All `Serialize + Debug + Clone + PartialEq + Eq`.

- [ ] **Step 1: Write the failing tests**

Append to the end of `crates/ridl-core/src/diag.rs`:

```rust
#[cfg(test)]
mod json_tests {
    use super::*;

    #[test]
    fn line_col_is_one_based_and_counts_chars() {
        let text = "ab\ncé\n";
        assert_eq!(line_col(text, TextSize::from(0)), LineCol { line: 1, column: 1 });
        assert_eq!(line_col(text, TextSize::from(2)), LineCol { line: 1, column: 3 });
        assert_eq!(line_col(text, TextSize::from(3)), LineCol { line: 2, column: 1 });
        // `é` is two bytes; the column after it is the third character.
        assert_eq!(line_col(text, TextSize::from(6)), LineCol { line: 2, column: 3 });
        // An offset past the end clamps to the end of the text.
        assert_eq!(line_col(text, TextSize::from(99)), LineCol { line: 3, column: 1 });
    }

    #[test]
    fn to_json_projects_spans_and_fixits_onto_lines_and_columns() {
        let mut sources = SourceMap::new();
        let file = sources.file_id("a.typl", "package p\ntype X:\n");
        let span = Span {
            file,
            range: TextRange::new(TextSize::from(10), TextSize::from(17)),
        };
        let diagnostic = Diagnostic {
            code: DiagCode("TYPL-100"),
            severity: Severity::Error,
            message: "expected a type".to_string(),
            primary: span,
            labels: Vec::new(),
            fixits: vec![FixIt {
                span,
                replacement: "type X: integer".to_string(),
                label: "give `X` a backing type".to_string(),
            }],
        };

        let json = to_json(&[diagnostic], &sources);

        assert_eq!(json.len(), 1);
        let first = &json[0];
        assert_eq!(first.code, "TYPL-100");
        assert_eq!(first.severity, "error");
        assert_eq!(first.message, "expected a type");
        assert_eq!(first.span.path, "a.typl");
        assert_eq!(first.span.start, LineCol { line: 2, column: 1 });
        assert_eq!(first.span.end, LineCol { line: 2, column: 8 });
        assert_eq!(first.fixes.len(), 1);
        assert_eq!(first.fixes[0].label, "give `X` a backing type");
        assert_eq!(first.fixes[0].replacement, "type X: integer");
        assert_eq!(first.fixes[0].span, first.span);
    }

    #[test]
    fn to_json_tolerates_a_span_on_an_unknown_file() {
        let sources = SourceMap::new();
        let diagnostic = Diagnostic {
            code: DiagCode("MANI-101"),
            severity: Severity::Warning,
            message: "detached".to_string(),
            primary: Span {
                file: FileId::DETACHED,
                range: TextRange::new(TextSize::from(0), TextSize::from(0)),
            },
            labels: Vec::new(),
            fixits: Vec::new(),
        };
        let json = to_json(&[diagnostic], &sources);
        assert_eq!(json[0].severity, "warning");
        assert_eq!(json[0].span.path, "");
        assert_eq!(json[0].span.start, LineCol { line: 1, column: 1 });
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p ridl-core json_tests` Expected: compile error — `line_col`,
`LineCol`, `to_json` not found.

- [ ] **Step 3: Implement the contract**

In `crates/ridl-core/src/diag.rs`, inside the existing `impl SourceMap` block
(after `pub fn path`), add:

```rust
pub fn text(&self, id: FileId) -> Option<&str> {
    self.files
        .get(id.0 as usize)
        .map(|entry| entry.text.as_str())
}
```

Then, after the `impl SourceMap` block and before `pub fn remap_diagnostics`,
add:

```rust
/// A 1-based line and column. The column counts Unicode scalar values, not
/// bytes, so a caller who supplied the source text can index into it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct LineCol {
    pub line: u32,
    pub column: u32,
}

/// The line and column of a byte `offset` into `text`. An offset past the end
/// of the text, or inside a multi-byte character, is moved back to the nearest
/// character boundary at or before it.
pub fn line_col(text: &str, offset: TextSize) -> LineCol {
    let mut offset = usize::from(offset).min(text.len());
    while !text.is_char_boundary(offset) {
        offset -= 1;
    }
    let before = &text[..offset];
    let line = before.matches('\n').count() as u32 + 1;
    let column = before
        .rsplit('\n')
        .next()
        .map_or(0, |last_line| last_line.chars().count()) as u32
        + 1;
    LineCol { line, column }
}

/// A span in the JSON diagnostic contract: the file's path as registered in the
/// [`SourceMap`] and 1-based start and end positions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct JsonSpan {
    pub path: String,
    pub start: LineCol,
    pub end: LineCol,
}

/// A fix-it in the JSON diagnostic contract, verbatim from the compiler.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct JsonFixIt {
    pub label: String,
    pub replacement: String,
    pub span: JsonSpan,
}

/// One diagnostic in the JSON contract `ridl check --format json` and the MCP
/// `ridl_check` tool emit. This is the first agent-facing diagnostic contract
/// (ADR-0005 §7): a change to its shape is a change to an external contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct JsonDiagnostic {
    pub code: String,
    pub severity: String,
    pub message: String,
    pub span: JsonSpan,
    pub fixes: Vec<JsonFixIt>,
}

fn json_span(span: Span, sources: &SourceMap) -> JsonSpan {
    let path = sources.path(span.file).unwrap_or("").to_string();
    let text = sources.text(span.file).unwrap_or("");
    JsonSpan {
        path,
        start: line_col(text, span.range.start()),
        end: line_col(text, span.range.end()),
    }
}

fn severity_name(severity: Severity) -> &'static str {
    match severity {
        Severity::Error => "error",
        Severity::Warning => "warning",
        Severity::Info => "info",
    }
}

/// Projects `diagnostics` onto the JSON contract, resolving every span through
/// `sources`.
pub fn to_json(diagnostics: &[Diagnostic], sources: &SourceMap) -> Vec<JsonDiagnostic> {
    diagnostics
        .iter()
        .map(|diagnostic| JsonDiagnostic {
            code: diagnostic.code.0.to_string(),
            severity: severity_name(diagnostic.severity).to_string(),
            message: diagnostic.message.clone(),
            span: json_span(diagnostic.primary, sources),
            fixes: diagnostic
                .fixits
                .iter()
                .map(|fixit| JsonFixIt {
                    label: fixit.label.clone(),
                    replacement: fixit.replacement.clone(),
                    span: json_span(fixit.span, sources),
                })
                .collect(),
        })
        .collect()
}
```

If `TextSize` and `TextRange` are not already imported at the top of `diag.rs`,
add `use rowan::{TextRange, TextSize};` (the file already serializes a
`TextRange`, so `TextRange` is in scope; check `TextSize`).

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p ridl-core json_tests` Expected: 3 passed.

- [ ] **Step 5: Gate and commit**

Run:
`cargo fmt --all && cargo clippy -p ridl-core --all-targets -- -D warnings && just wasm-check`
Expected: no warnings; wasm-check passes.

```bash
git add crates/ridl-core/src/diag.rs
git commit -m "feat(ridl-core): add the JSON diagnostic contract with line and column spans

The MCP ridl_check tool and ridl check --format json emit one shape, defined
once here. This is the first agent-facing diagnostic contract (ADR-0005 §7).

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 2: `ridlc::check_source` — the single-file check without the backend

**Files:**

- Modify: `crates/ridlc/src/lib.rs:64-130` (the `compile` function)
- Test: `crates/ridlc/tests/check_source.rs` (new)

**Interfaces:**

- Consumes: the body of `ridlc::compile(path, text) -> CompileOutput`;
  `CliRun { pub diagnostics: Vec<Diagnostic>, pub sources: SourceMap }` (already
  public in `lib.rs:380`); `ridl_core::diag::to_json` from Task 1.
- Produces: `pub fn check_source(path: &str, text: &str) -> CliRun` — parse,
  resolve, and check `text` as a single-file package under the profile `path`'s
  extension selects, against the embedded `ridl.std`; no backend runs.

- [ ] **Step 1: Write the failing tests**

Create `crates/ridlc/tests/check_source.rs`:

```rust
//! `ridlc::check_source`: the single-file front end the MCP tool and
//! `ridl check --format json` share.

use ridl_core::diag::to_json;

#[test]
fn a_bare_package_declaration_checks_clean() {
    let run = ridlc::check_source("only_package.typl", "package p\n");
    assert!(!run.has_error(), "{:?}", run.diagnostics);
}

#[test]
fn a_broken_declaration_reports_an_error_on_line_two() {
    let run = ridlc::check_source("broken.typl", "package p\ntype X:\n");
    assert!(run.has_error());

    let json = to_json(&run.diagnostics, &run.sources);
    let error = json
        .iter()
        .find(|diagnostic| diagnostic.severity == "error")
        .expect("an error diagnostic");
    assert!(!error.code.is_empty());
    assert_eq!(error.span.path, "broken.typl");
    assert_eq!(error.span.start.line, 2);
}

#[test]
fn the_profile_follows_the_path_extension() {
    // A `.ridl` file may declare an interface; a `.typl` file may not.
    let source = "package p\ninterface I {}\n";
    assert!(!ridlc::check_source("ok.ridl", source).has_error());
    assert!(ridlc::check_source("wrong.typl", source).has_error());
}

#[test]
fn check_source_and_compile_agree_on_diagnostics() {
    let source = "package p\ntype X:\n";
    let checked = ridlc::check_source("same.typl", source);
    let compiled = ridlc::compile("same.typl", source);
    assert_eq!(
        to_json(&checked.diagnostics, &checked.sources),
        to_json(&compiled.diagnostics, &compiled.sources)
    );
}
```

If `crates/ridlc/Cargo.toml` has no `[dev-dependencies]` entry for `ridl-core`,
none is needed: `ridl-core` is a normal dependency and its `diag` module is
public.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p ridlc --test check_source` Expected: compile error —
`check_source` not found.

- [ ] **Step 3: Factor the front end out of `compile`**

In `crates/ridlc/src/lib.rs`, replace the `compile` function with a private
`front_end` plus two public entry points. Move the existing body verbatim:
everything from `let mut db = RidlDatabase::default();` through the two
`diagnostics.extend(remap_diagnostics(...))` lines goes into `front_end`; the
backend `match ridl_backend_rust::generate(&checked.ir)` and the construction of
`CompileOutput` stay in `compile`.

```rust
/// The checked front end of one source text: parser, resolver, and checker
/// diagnostics, the source map they are remapped onto, and the IR the checker
/// produced.
struct FrontEnd {
    diagnostics: Vec<Diagnostic>,
    sources: SourceMap,
    /// The one file the source was interned as — the backend's error arm
    /// points its diagnostic at it.
    file: FileId,
    ir: ridl_ir::v2::Package,
}

/// Parses, resolves, and checks `text` (registered under `path`) as a
/// single-file synthetic package named from its `package` declaration, falling
/// back to the path's file stem — the loader's single-file rule (E1.3). The
/// profile follows `path`'s extension. Diagnostics are concatenated parser →
/// resolver → checker, with the package-scoped spans remapped onto this
/// function's own [`SourceMap`].
fn front_end(path: &str, text: &str) -> FrontEnd {
    // --- moved verbatim from `compile`: from `let mut db = ...` through the
    // --- second `diagnostics.extend(remap_diagnostics(...))`.
    let mut db = RidlDatabase::default();
    let std = std_package(&mut db);
    let input = InputFile::new(&db, path.to_string(), text.to_string());
    let parse = parse_file(&db, input);

    let mut sources = SourceMap::new();
    let file = sources.file_id(path, text);

    let mut diagnostics: Vec<Diagnostic> = parse
        .errors()
        .iter()
        .map(|error| {
            error_diagnostic(
                error.code,
                ridl_core::diag::house_style_message(&error.message),
                file,
                error.range,
            )
        })
        .collect();

    let ast = SourceFile::cast(parse.syntax()).expect("parser roots every tree in a SourceFile");
    let package_name = declared_package_name(&ast).unwrap_or_else(|| module_name_from_path(path));
    let pkg = Package::new(
        &db,
        package_name,
        vec![input],
        PackageOrigin::WorkspaceMember,
        BTreeMap::new(),
        None,
    );
    let ws = Workspace::new(&db, vec![pkg], BTreeMap::new());

    let resolution = resolve_package(&db, ws, pkg, std);
    let checked = check_package(&db, ws, pkg, std);

    let render_ids = vec![file];
    diagnostics.extend(remap_diagnostics(resolution.diagnostics, &render_ids));
    diagnostics.extend(remap_diagnostics(checked.diagnostics, &render_ids));
    // --- end of the moved body.

    FrontEnd {
        diagnostics,
        sources,
        file,
        ir: checked.ir,
    }
}

/// Checks `text` (registered under `path`) without running any backend: the
/// single-file oracle `ridl mcp`'s `ridl_check` and `ridl check --format json`
/// share.
pub fn check_source(path: &str, text: &str) -> CliRun {
    let front = front_end(path, text);
    CliRun {
        diagnostics: front.diagnostics,
        sources: front.sources,
    }
}

/// Compiles `text` (registered under `path`) end to end: the front end, then
/// the Rust backend.
pub fn compile(path: &str, text: &str) -> CompileOutput {
    let FrontEnd {
        mut diagnostics,
        sources,
        file,
        ir,
    } = front_end(path, text);

    // Unchanged from the old `compile` except `checked.ir` → `ir`.
    let rust_source = match ridl_backend_rust::generate(&ir) {
        // The E1.12 backend returns Rust plus a C header; this pre-CLI plumbing
        // path keeps only the Rust source. Task 20 wires the C header emit.
        Ok(generated) => generated.rust_source,
        Err(err) => {
            // The backend does not carry source ranges yet, so its diagnostic
            // has no code and points at the file start.
            diagnostics.push(error_diagnostic(
                "",
                err.message,
                file,
                TextRange::default(),
            ));
            String::new()
        }
    };

    CompileOutput {
        rust_source,
        package: ir,
        diagnostics,
        sources,
    }
}
```

Keep the existing doc comments' content on `compile` where it still applies.
`checked.ir`'s type is whatever `CheckedPackage::ir` is; if it is not
`ridl_ir::v2::Package`, use that type for `FrontEnd::ir` (it is the type
`CompileOutput::package` already has).

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p ridlc` Expected: the four new tests pass and every existing
`ridlc` test (`golden`, `totality`, …) still passes — `compile`'s behavior is
unchanged.

- [ ] **Step 5: Gate and commit**

Run: `cargo fmt --all && cargo clippy -p ridlc --all-targets -- -D warnings`

```bash
git add crates/ridlc/src/lib.rs crates/ridlc/tests/check_source.rs
git commit -m "feat(ridlc): expose check_source, the single-file front end without a backend

The MCP tool checks a source string an agent supplies; it needs the parser,
resolver, and checker but no code generation. compile keeps its behavior and
now calls the same front end.

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 3: `ridl check --format json`

**Files:**

- Modify: `crates/ridl/src/main.rs:54-71` (the `Check` variant), `:146-150` (its
  dispatch), `:444-465` (`run_check`), and `:1214` (`finish`, unchanged — a new
  `finish_check` sits next to it)
- Test: `crates/ridl/tests/check_json.rs` (new)

**Interfaces:**

- Consumes: `ridl_core::diag::to_json` (Task 1);
  `ridlc::run_check(path, frozen) -> io::Result<CliRun>`; `CliRun::has_error()`.
- Produces: the CLI flag `ridl check --format json`, printing a JSON array of
  `JsonDiagnostic` to **stdout** (text mode keeps rendering to stderr), exit
  code 1 when any diagnostic is an error, 0 otherwise, 2 on an I/O failure —
  unchanged from text mode.

- [ ] **Step 1: Write the failing test**

Create `crates/ridl/tests/check_json.rs`:

```rust
//! `ridl check --format json` prints the JSON diagnostic contract to stdout.

use std::path::PathBuf;
use std::process::Command;

fn scratch_file(name: &str, text: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ridl-check-json-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("scratch dir");
    let path = dir.join(name);
    std::fs::write(&path, text).expect("scratch file");
    path
}

#[test]
fn json_format_prints_the_contract_and_keeps_the_exit_code() {
    let path = scratch_file("broken.typl", "package p\ntype X:\n");

    let output = Command::new(env!("CARGO_BIN_EXE_ridl"))
        .args(["check", "--format", "json"])
        .arg(&path)
        .output()
        .expect("run ridl check");

    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let stdout = String::from_utf8(output.stdout).expect("utf-8 stdout");
    let diagnostics: serde_json::Value = serde_json::from_str(&stdout).expect("stdout is JSON");
    let diagnostics = diagnostics.as_array().expect("a JSON array");
    let error = diagnostics
        .iter()
        .find(|diagnostic| diagnostic["severity"] == "error")
        .expect("an error diagnostic");
    assert!(error["code"].as_str().is_some_and(|code| !code.is_empty()));
    assert_eq!(error["span"]["start"]["line"], 2);
    assert!(error["fixes"].is_array());
}

#[test]
fn json_format_prints_an_empty_array_for_a_clean_file() {
    let path = scratch_file("clean.typl", "package p\n");

    let output = Command::new(env!("CARGO_BIN_EXE_ridl"))
        .args(["check", "--format", "json"])
        .arg(&path)
        .output()
        .expect("run ridl check");

    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "[]");
}
```

`serde_json` is already a dependency of `crates/ridl`; integration tests can use
a crate's normal dependencies.

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p ridl --test check_json` Expected: both fail — `--format` is
an unknown argument (exit code 2, stdout empty).

- [ ] **Step 3: Add the flag**

In `crates/ridl/src/main.rs`, add to the `Check` variant after `baseline`:

```rust
/// Output format: human-readable text on stderr (the default), or a
/// JSON array of diagnostics on stdout with a stable schema
/// (`ridl_core::diag::JsonDiagnostic`).
#[arg(long, value_enum, default_value_t = CheckFormat::Text)]
format: CheckFormat,
```

Next to the existing `DiffFormat` enum, add:

```rust
/// The `ridl check` output format.
#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
enum CheckFormat {
    Text,
    Json,
}
```

Change the dispatch:

```rust
Command::Check {
    path,
    frozen,
    baseline,
    format,
} => run_check(&path, frozen, baseline.as_deref(), format),
```

Change `run_check`'s signature to
`fn run_check(path: &Path, frozen: bool, baseline: Option<&Path>, format: CheckFormat) -> ExitCode`
and its last line from `finish(Ok(run))` to `finish_check(run, format)`. Then
add, directly above `fn finish`:

```rust
/// Ends `ridl check`: text renders to stderr through [`finish`]; JSON prints
/// the contract to stdout and keeps the same exit code.
fn finish_check(run: CliRun, format: CheckFormat) -> ExitCode {
    match format {
        CheckFormat::Text => finish(Ok(run)),
        CheckFormat::Json => {
            let json = ridl_core::diag::to_json(&run.diagnostics, &run.sources);
            println!(
                "{}",
                serde_json::to_string_pretty(&json).expect("diagnostics serialize")
            );
            if run.has_error() {
                ExitCode::FAILURE
            } else {
                ExitCode::SUCCESS
            }
        }
    }
}
```

The baseline desk check inside `run_check` is unchanged: its RIDL-407 warnings
are appended to `run.diagnostics` before `finish_check` runs, so they appear in
the JSON too.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p ridl --test check_json` Expected: 2 passed. Then
`cargo test -p ridl` — every existing CLI test still passes (text mode is
untouched).

- [ ] **Step 5: Gate and commit**

Run: `cargo fmt --all && cargo clippy -p ridl --all-targets -- -D warnings`

```bash
git add crates/ridl/src/main.rs crates/ridl/tests/check_json.rs
git commit -m "feat(ridl): add check --format json, the CLI face of the diagnostic contract

Prints ridl_core::diag::JsonDiagnostic to stdout; exit codes are unchanged.
It is the oracle the MCP ridl_check tool is compared against.

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 4: The `ridl-mcp` library crate

**Files:**

- Modify: `.git-std.toml` (add `"ridl-mcp",` after `"ridl-lsp",` in `scopes`)
- Modify: `Cargo.toml` (root — `[workspace] members` if members are listed
  explicitly; `[workspace.dependencies]` gains `rmcp`, `schemars`, `tokio`)
- Create: `crates/ridl-mcp/Cargo.toml`, `crates/ridl-mcp/src/lib.rs`
- Test: `crates/ridl-mcp/src/lib.rs` (`#[cfg(test)] mod tests`)

**Interfaces:**

- Consumes: `ridlc::check_source(path, text) -> CliRun` (Task 2);
  `ridl_core::diag::{to_json, JsonDiagnostic}` (Task 1).
- Produces: `pub enum Profile { Typl, Ridl }` (serde `lowercase`);
  `pub struct CheckParams { pub source: String, pub profile: Profile }`;
  `pub struct CheckOutput { pub diagnostics: Vec<JsonDiagnostic> }`;
  `pub fn check(params: &CheckParams) -> CheckOutput`; `pub struct RidlMcp` with
  `RidlMcp::new()`;
  `pub async fn serve_stdio() -> Result<(), Box<dyn std::error::Error + Send + Sync>>`.

- [ ] **Step 1: Register the scope and add the dependencies**

Edit `.git-std.toml`: after the line `"ridl-lsp",` add `"ridl-mcp",`. Commit
that alone first, so the later commits' scope lints:

```bash
git add .git-std.toml
git commit -m "chore(repo): add the ridl-mcp commit scope

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

Create the crate and add the dependencies with `cargo add`, which records the
current versions (do not hand-write a version):

```bash
cargo new --lib crates/ridl-mcp --name ridl-mcp --vcs none
cd crates/ridl-mcp
cargo add rmcp --features server,macros,transport-io
cargo add schemars
cargo add serde --features derive
cargo add serde_json
cargo add --path ../ridlc
cargo add --path ../ridl-core
cd ../..
```

Then edit `crates/ridl-mcp/Cargo.toml` so its `[package]` matches the sibling
crates:

```toml
[package]
name = "ridl-mcp"
# The MCP server for RIDL (ADR-0005 Layer B, docs/ROADMAP.md epic E8.6): a
# thin consumer of the shared compiler crates behind `ridl mcp`.
version.workspace = true
edition.workspace = true
license.workspace = true
repository.workspace = true
publish = false
```

and move `rmcp`, `schemars`, `serde`, `serde_json` to `.workspace = true`
entries, adding `rmcp` and `schemars` to the root `[workspace.dependencies]`
with the versions `cargo add` picked (`serde` and `serde_json` are already
there). If the root `Cargo.toml` lists `members` explicitly, add
`"crates/ridl-mcp"`.

Run: `cargo metadata --format-version 1 --no-deps | grep -c '"name":"ridl-mcp"'`
Expected: `1`.

- [ ] **Step 2: Write the failing tests**

Replace the generated `crates/ridl-mcp/src/lib.rs` with a file containing only
the tests for now:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_reports_an_error_for_a_broken_typl_source() {
        let output = check(&CheckParams {
            source: "package p\ntype X:\n".to_string(),
            profile: Profile::Typl,
        });
        let error = output
            .diagnostics
            .iter()
            .find(|diagnostic| diagnostic.severity == "error")
            .expect("an error diagnostic");
        assert_eq!(error.span.path, "input.typl");
        assert_eq!(error.span.start.line, 2);
    }

    #[test]
    fn check_selects_the_profile_from_the_parameter() {
        let source = "package p\ninterface I {}\n".to_string();
        let ridl = check(&CheckParams {
            source: source.clone(),
            profile: Profile::Ridl,
        });
        let typl = check(&CheckParams {
            source,
            profile: Profile::Typl,
        });
        assert!(ridl.diagnostics.iter().all(|d| d.severity != "error"));
        assert!(typl.diagnostics.iter().any(|d| d.severity == "error"));
    }

    #[test]
    fn profile_deserializes_lowercase_only() {
        assert!(serde_json::from_str::<Profile>("\"typl\"").is_ok());
        assert!(serde_json::from_str::<Profile>("\"ridl\"").is_ok());
        assert!(serde_json::from_str::<Profile>("\"rxdl\"").is_err());
        assert!(serde_json::from_str::<Profile>("\"Typl\"").is_err());
    }

    #[test]
    fn the_server_advertises_exactly_one_tool_named_ridl_check() {
        let server = RidlMcp::new();
        let names: Vec<String> = server
            .tool_router
            .list_all()
            .into_iter()
            .map(|tool| tool.name.to_string())
            .collect();
        assert_eq!(names, vec!["ridl_check".to_string()]);
    }
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test -p ridl-mcp` Expected: compile error — `check`, `CheckParams`,
`Profile`, `RidlMcp` not found.

- [ ] **Step 4: Implement the crate**

Put this above the `#[cfg(test)]` module in `crates/ridl-mcp/src/lib.rs`:

```rust
//! The RIDL MCP server (ADR-0005 Layer B; docs/ROADMAP.md epic E8.6).
//!
//! One tool, `ridl_check`: the compiler's single-file check
//! (`ridlc::check_source`) over a source string an agent supplies, returned
//! as the JSON diagnostic contract (`ridl_core::diag::JsonDiagnostic`). The
//! server is a thin consumer of the shared compiler crates — no parser, no
//! checker of its own — and runs over stdio behind `ridl mcp`.

use ridl_core::diag::{JsonDiagnostic, to_json};
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, Content, ServerCapabilities, ServerInfo};
use rmcp::transport::stdio;
use rmcp::{ErrorData, ServerHandler, ServiceExt, tool, tool_handler, tool_router};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Which language a source text is parsed as. These are the two profiles the
/// compiler has; the `.rxdl` form does not exist yet (epic E3.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Profile {
    Typl,
    Ridl,
}

impl Profile {
    /// The path the source is registered under: `ridlc` selects the profile
    /// from the extension, and this path is what spans report.
    fn synthetic_path(self) -> &'static str {
        match self {
            Profile::Typl => "input.typl",
            Profile::Ridl => "input.ridl",
        }
    }
}

/// The `ridl_check` tool's input.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct CheckParams {
    /// The full text of one `.typl` or `.ridl` file.
    pub source: String,
    /// Which language `source` is parsed as.
    pub profile: Profile,
}

/// The `ridl_check` tool's output.
#[derive(Debug, Serialize)]
pub struct CheckOutput {
    pub diagnostics: Vec<JsonDiagnostic>,
}

/// Checks `params.source` under `params.profile` against the embedded
/// `ridl.std`. Pure: no transport, no I/O.
pub fn check(params: &CheckParams) -> CheckOutput {
    let run = ridlc::check_source(params.profile.synthetic_path(), &params.source);
    CheckOutput {
        diagnostics: to_json(&run.diagnostics, &run.sources),
    }
}

/// The MCP server: the tool router and nothing else.
#[derive(Clone)]
pub struct RidlMcp {
    tool_router: ToolRouter<Self>,
}

impl Default for RidlMcp {
    fn default() -> Self {
        Self::new()
    }
}

#[tool_router]
impl RidlMcp {
    pub fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }

    #[tool(
        name = "ridl_check",
        description = "Type-check one typl or ridl source text against the embedded ridl.std. Returns the compiler's coded diagnostics with their spans and fix-its, verbatim."
    )]
    fn ridl_check(
        &self,
        Parameters(params): Parameters<CheckParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let output = check(&params);
        let text = serde_json::to_string(&output)
            .map_err(|error| ErrorData::internal_error(error.to_string(), None))?;
        Ok(CallToolResult::success(vec![Content::text(text)]))
    }
}

#[tool_handler]
impl ServerHandler for RidlMcp {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            instructions: Some(
                "RIDL compiler tools. Call ridl_check with a source text and a profile \
                 (typl or ridl) to get coded diagnostics with fix-its."
                    .to_string(),
            ),
            ..Default::default()
        }
    }
}

/// Serves the MCP protocol over this process's stdin and stdout until the
/// client disconnects.
pub async fn serve_stdio() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let service = RidlMcp::new().serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
```

If the `rmcp` version `cargo add` picked names things differently (`rmcp::Error`
instead of `rmcp::ErrorData`, `list_all` on `ToolRouter` under another name),
follow `cargo doc -p rmcp --open` for that version and adjust the import, not
the design. `#[tool_router]` generates `Self::tool_router()`; `#[tool_handler]`
generates `list_tools`/`call_tool` over the `tool_router` field.

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p ridl-mcp` Expected: 4 passed.

- [ ] **Step 6: Gate and commit**

Run:
`cargo fmt --all && cargo clippy -p ridl-mcp --all-targets -- -D warnings && just wasm-check`
Expected: clean; wasm-check unaffected (`ridl-mcp` is not in its crate list).

```bash
git add Cargo.toml Cargo.lock crates/ridl-mcp
git commit -m "feat(ridl-mcp): add the MCP server library with the ridl_check tool

A thin consumer of ridlc::check_source over rmcp's stdio transport: one tool,
returning the ridl-core JSON diagnostic contract. ADR-0005 Layer B, epic E8.6.

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 5: `ridl lsp` and `ridl mcp` subcommands

**Files:**

- Modify: `crates/ridl/Cargo.toml` (`[dependencies]` and `[dev-dependencies]`),
  `Cargo.toml` (root `[workspace.dependencies]`: `tokio`)
- Modify: `crates/ridl/src/main.rs:50-142` (the `Command` enum) and `:143-170`
  (dispatch); new `run_lsp` / `run_mcp` next to `run_check`
- Test: `crates/ridl/tests/servers.rs` (new)

**Interfaces:**

- Consumes:
  `ridl_lsp::server::run(connection: lsp_server::Connection) -> Result<(), E>`
  where `E: std::error::Error` (the existing `crates/ridl-lsp/src/main.rs` calls
  it exactly this way); `ridl_mcp::serve_stdio()` (Task 4);
  `ridl check --format json` (Task 3).
- Produces: `ridl lsp` and `ridl mcp` — both stdio servers; exit 0 on a clean
  shutdown, 2 on a transport or runtime error.

- [ ] **Step 1: Add the dependencies**

```bash
cd crates/ridl
cargo add --path ../ridl-lsp
cargo add --path ../ridl-mcp
cargo add lsp-server
cargo add tokio --features rt-multi-thread
cargo add --dev rmcp --features client,transport-child-process
cargo add --dev tokio --features rt-multi-thread,macros,process
cd ../..
```

Then in `crates/ridl/Cargo.toml` change `lsp-server = "..."` to
`lsp-server.workspace = true`, and make `tokio` a `[workspace.dependencies]`
entry (`tokio = { version = "<picked>", features = ["rt-multi-thread"] }`)
referenced as `tokio.workspace = true` from `[dependencies]`; the dev-dependency
keeps its extra features via
`tokio = { workspace = true, features = ["macros", "process"] }`. `rmcp` in
dev-dependencies:
`rmcp = { workspace = true, features = ["client", "transport-child-process"] }`.

- [ ] **Step 2: Write the failing tests**

Create `crates/ridl/tests/servers.rs`:

```rust
//! `ridl lsp` and `ridl mcp`: the two stdio servers the one binary hosts.

use std::io::Write as _;
use std::process::{Command as StdCommand, Stdio};
use std::sync::mpsc;
use std::time::Duration;

use rmcp::ServiceExt;
use rmcp::model::CallToolRequestParams;
use rmcp::transport::TokioChildProcess;
use tokio::process::Command;

const BROKEN_TYPL: &str = "package p\ntype X:\n";

/// Runs `ridl mcp` as a child, performs the MCP handshake, and returns the
/// client.
async fn connect() -> rmcp::service::RunningService<rmcp::RoleClient, ()> {
    let transport = TokioChildProcess::new(Command::new(env!("CARGO_BIN_EXE_ridl")).arg("mcp"))
        .expect("spawn ridl mcp");
    ().serve(transport).await.expect("MCP initialize handshake")
}

fn tool_text(result: &rmcp::model::CallToolResult) -> String {
    result
        .content
        .iter()
        .filter_map(|content| content.as_text())
        .map(|text| text.text.clone())
        .collect()
}

#[tokio::test]
async fn ridl_mcp_advertises_ridl_check() {
    let client = connect().await;
    let tools = client.list_tools(Default::default()).await.expect("tools/list");
    let names: Vec<&str> = tools.tools.iter().map(|tool| tool.name.as_ref()).collect();
    assert_eq!(names, ["ridl_check"]);
    client.cancel().await.expect("shutdown");
}

#[tokio::test]
async fn ridl_check_returns_the_diagnostic_contract() {
    let client = connect().await;
    let result = client
        .call_tool(CallToolRequestParams {
            name: "ridl_check".into(),
            arguments: serde_json::json!({ "source": BROKEN_TYPL, "profile": "typl" })
                .as_object()
                .cloned(),
            ..Default::default()
        })
        .await
        .expect("tools/call");
    assert_ne!(result.is_error, Some(true), "{result:?}");

    let text = tool_text(&result);
    let output: serde_json::Value = serde_json::from_str(&text).expect("tool returns JSON");
    let diagnostics = output["diagnostics"].as_array().expect("diagnostics array");
    assert!(
        diagnostics
            .iter()
            .any(|d| d["severity"] == "error" && d["span"]["start"]["line"] == 2),
        "{text}"
    );
    client.cancel().await.expect("shutdown");
}

#[tokio::test]
async fn ridl_check_and_check_format_json_agree() {
    // The CLI side.
    let dir = std::env::temp_dir().join(format!("ridl-servers-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("scratch dir");
    let path = dir.join("agree.typl");
    std::fs::write(&path, BROKEN_TYPL).expect("scratch file");
    let cli = StdCommand::new(env!("CARGO_BIN_EXE_ridl"))
        .args(["check", "--format", "json"])
        .arg(&path)
        .output()
        .expect("run ridl check");
    let mut cli: serde_json::Value =
        serde_json::from_slice(&cli.stdout).expect("CLI stdout is JSON");

    // The MCP side.
    let client = connect().await;
    let result = client
        .call_tool(CallToolRequestParams {
            name: "ridl_check".into(),
            arguments: serde_json::json!({ "source": BROKEN_TYPL, "profile": "typl" })
                .as_object()
                .cloned(),
            ..Default::default()
        })
        .await
        .expect("tools/call");
    client.cancel().await.expect("shutdown");
    let mut mcp: serde_json::Value =
        serde_json::from_str(&tool_text(&result)).expect("tool returns JSON");
    let mut mcp = mcp["diagnostics"].take();

    // The only permitted difference is the path each side registered.
    for value in cli.as_array_mut().into_iter().flatten() {
        value["span"]["path"] = serde_json::Value::String(String::new());
        for fix in value["fixes"].as_array_mut().into_iter().flatten() {
            fix["span"]["path"] = serde_json::Value::String(String::new());
        }
    }
    for value in mcp.as_array_mut().into_iter().flatten() {
        value["span"]["path"] = serde_json::Value::String(String::new());
        for fix in value["fixes"].as_array_mut().into_iter().flatten() {
            fix["span"]["path"] = serde_json::Value::String(String::new());
        }
    }
    assert_eq!(cli, mcp);
}

#[test]
fn ridl_lsp_starts_and_exits_when_stdin_closes() {
    let mut child = StdCommand::new(env!("CARGO_BIN_EXE_ridl"))
        .arg("lsp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn ridl lsp");
    // Close stdin without sending a request: the server must notice the
    // transport ending and exit, not hang.
    let mut stdin = child.stdin.take().expect("piped stdin");
    stdin.flush().expect("flush");
    drop(stdin);

    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let status = child.wait();
        let _ = sender.send(status);
    });
    let status = receiver
        .recv_timeout(Duration::from_secs(20))
        .expect("ridl lsp exits within 20 s of stdin closing")
        .expect("wait");
    // Any exit is acceptable here; the property under test is termination.
    let _ = status;
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test -p ridl --test servers` Expected: the three MCP tests fail on
the handshake (`ridl mcp` is an unknown subcommand, the process exits at once);
the LSP test fails because `ridl lsp` exits with clap's usage error — it passes
vacuously only if termination is all it checks, so also confirm by hand:
`cargo run -p ridl -- lsp` prints `error: unrecognized subcommand 'lsp'`.

- [ ] **Step 4: Add the subcommands**

In `crates/ridl/src/main.rs`, add two variants at the end of `enum Command`:

```rust
/// Run the language server over stdio (the VS Code extension spawns this).
Lsp,
/// Run the MCP server over stdio (Claude Code, Copilot, and Codex spawn
/// this).
Mcp,
```

Add to the `match cli.command` in `main`:

```rust
Command::Lsp => run_lsp(),
Command::Mcp => run_mcp(),
```

Add next to `run_check`:

```rust
/// `ridl lsp`: the language server over stdio. The behavior lives in
/// `ridl-lsp`; this wires the transport.
fn run_lsp() -> ExitCode {
    let (connection, io_threads) = lsp_server::Connection::stdio();
    if let Err(err) = ridl_lsp::server::run(connection) {
        eprintln!("error: {err}");
        return ExitCode::from(2);
    }
    if let Err(err) = io_threads.join() {
        eprintln!("error: {err}");
        return ExitCode::from(2);
    }
    ExitCode::SUCCESS
}

/// `ridl mcp`: the MCP server over stdio. `rmcp` is async, so this owns the
/// only Tokio runtime in the binary.
fn run_mcp() -> ExitCode {
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(err) => {
            eprintln!("error: {err}");
            return ExitCode::from(2);
        }
    };
    match runtime.block_on(ridl_mcp::serve_stdio()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("error: {err}");
            ExitCode::from(2)
        }
    }
}
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p ridl --test servers` Expected: 4 passed. If
`ridl_lsp_starts_and_exits_when_stdin_closes` times out, `ridl_lsp::server::run`
does not return on a closed transport before `initialize`; in that case make the
test send a minimal `initialize` request followed by `shutdown` and `exit`
notifications (Content-Length framed JSON-RPC) before closing stdin, and keep
the termination assertion.

- [ ] **Step 6: Full gate and commit**

Run: `cargo fmt --all && just build` Expected: every recipe passes
(`gate-parity` is unaffected — no `build` member changed).

```bash
git add Cargo.toml Cargo.lock crates/ridl/Cargo.toml crates/ridl/src/main.rs crates/ridl/tests/servers.rs
git commit -m "feat(ridl): host the language server and the MCP server as ridl lsp and ridl mcp

One binary, two stdio roles — the shape the extension bundles and the tarball
ships. ridl-lsp keeps its own binary until the extension switches to ridl lsp.

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 6: Documentation — the crate README with host config snippets

**Files:**

- Create: `crates/ridl-mcp/README.md`
- Modify: `crates/ridl-lsp/src/lib.rs:1-25` (the module doc: mention `ridl lsp`)
- Modify: `AGENTS.md` (the crate list: thirteen → fourteen crates, add
  `ridl-mcp`)

**Interfaces:**

- Consumes: the `ridl mcp` subcommand (Task 5); the JSON contract (Task 1).
- Produces: documentation only.

- [ ] **Step 1: Write the README**

Create `crates/ridl-mcp/README.md`:

````markdown
# ridl-mcp

The RIDL MCP server (ADR-0005 Layer B): `ridl mcp` serves the Model Context
Protocol over stdio with one tool, `ridl_check`. It is a thin consumer of the
shared compiler crates — the same parser, resolver, and checker `ridl check`
and `ridl lsp` use.

## Install

`ridl` must be on `PATH`:

```sh
cargo install --path crates/ridl
```

(A release tarball and an installer script are added by the distribution
plan, `docs/wip/2026-09-13-vscode-extension-distribution-design.md`.)

## Tools

### `ridl_check`

| Parameter | Type                 | Meaning                                  |
| --------- | -------------------- | ---------------------------------------- |
| `source`  | string               | the full text of one file                |
| `profile` | `"typl"` or `"ridl"` | which language `source` is parsed as     |

Returns `{ "diagnostics": [ … ] }`, where each element is:

| Field      | Content                                                          |
| ---------- | ---------------------------------------------------------------- |
| `code`     | the catalogue code, for example `RIDL-107`                       |
| `severity` | `error`, `warning`, or `info`                                    |
| `message`  | the primary message, verbatim                                    |
| `span`     | `path`, and 1-based `start`/`end` with `line` and `column`       |
| `fixes`    | fix-its, verbatim: `label`, `replacement`, `span`                |

`ridl check --format json <file>` prints the same structure. This is the
first agent-facing diagnostic contract; a change to its shape is a change to
an external contract (ADR-0005 §7).

The source is checked as a standalone file against the embedded `ridl.std`
only: no workspace, no other imports. The `.rxdl` form is not a profile yet
(epic E3.5).

## Host configuration

### Claude Code

Add to the project's `.mcp.json` (or run `claude mcp add ridl -- ridl mcp`):

```json
{
  "mcpServers": {
    "ridl": { "command": "ridl", "args": ["mcp"] }
  }
}
```

### Codex

Add to `~/.codex/config.toml`:

```toml
[mcp_servers.ridl]
command = "ridl"
args = ["mcp"]
```

### GitHub Copilot in VS Code

Nothing to configure: the RIDL VS Code extension registers `ridl mcp` with VS
Code (1.101 or newer) when it activates.
````

- [ ] **Step 2: Update the LSP crate doc and AGENTS.md**

In `crates/ridl-lsp/src/lib.rs`, in the module doc comment, where the binary is
described, add one sentence: "The server is also reachable as `ridl lsp` (the
`ridl` CLI), which the release artifacts ship." In `AGENTS.md`, change "thirteen
crates" to "fourteen crates" and add `ridl-mcp` to the list after `ridl-lsp`.

- [ ] **Step 3: Gate and commit**

Run: `just check && just link-check` Expected: prim, markdownlint, and the link
check pass (fix any table alignment `prim fmt` reports with `just fmt`).

```bash
git add crates/ridl-mcp/README.md crates/ridl-lsp/src/lib.rs AGENTS.md
git commit -m "docs(ridl-mcp): document the ridl_check contract and the host configuration

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 7: Finish the branch

- [ ] **Step 1: Run the full gate once more**

Run: `just verify` Expected: `lint-commits` accepts every commit; `build` is
green.

- [ ] **Step 2: Hand off**

Do not garden `docs/wip/` yet: the distribution plan
(`docs/wip/2026-09-13-vscode-extension-distribution-plan.md`) builds on this
branch and is gardened with it. Open the PR against `main` with the spec and
this plan linked in the description, and stop.
