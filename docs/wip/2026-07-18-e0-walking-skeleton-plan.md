# Epic 0 — Walking Skeleton Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** one trivial `.typl` file compiles end to end — lex → parse → resolve →
check → IR → generated Rust — with a green `insta` snapshot (docs/ROADMAP.md,
epic E0).

**Architecture:** a Cargo workspace per concept note §8.1: `ridl-syntax` (logos
lexer + hand-written parser + rowan lossless CST + thin typed-AST accessors),
`ridl-core` (salsa database, resolver, checker — the semantic passes live here
until `ridl-sem` exists), `ridl-ir` (protobuf IR v0 compiled with prost),
`backends/rust` (IR → Rust source via quote + prettyplease), `ridlc` (compiler
CLI wiring the pipeline), `ridl` (facade stub). Every stage is deliberately
shallow — the epic proves the IR shape and the query graph, not feature depth.

**Tech Stack:** Rust edition 2024 · logos · rowan · salsa (pinned exact) ·
prost + protox (no system protoc) · serde/serde_json · quote + prettyplease ·
clap · insta. All fixed by ADR-0004; E0-specific choices recorded in ADR-0006.

## Global Constraints

- The exit criterion is the fixture below compiling end to end; **no feature
  depth anywhere** (docs/ROADMAP.md, epic E0).
- Library choices are fixed by ADR-0004 §2–§9 and are not renegotiable inside a
  task: logos, hand-written recursive descent (no combinator or generator
  frameworks), rowan, salsa, prost, quote + prettyplease, clap, insta.
- E0-scoped decisions are fixed by ADR-0006: crates under `crates/`, backend
  under `backends/rust`, resolver + checker in `ridl-core`, protox instead of
  system protoc, hand-written typed-AST accessors, crates.io reservation
  deferred.
- The lexer must emit trivia (whitespace, comments) as real tokens — rowan
  losslessness depends on it (ADR-0004 §2).
- Diagnostics are accumulated values, never a hard error return (ADR-0004 §5);
  E0 uses plain per-crate error structs, not the full coded `Diagnostic` model.
- Commit messages are Conventional Commits linted by git-std against
  `.git-std.toml`; use the type and scope named in each task.
- Every task ends with `cargo test --workspace`, `cargo fmt --all --check`, and
  `cargo clippy --workspace --all-targets -- -D warnings` green.
- Prose in comments and docs follows plain, literal English.

## The fixture

The single input every stage is built against, and the golden input of E0.9.
Created in E0.1 at `crates/ridl-syntax/fixtures/walking_skeleton.typl`:

```ridl
// Walking-skeleton fixture — docs/ROADMAP.md epic E0.
type Speed: km/h [0.0..250.0 step 0.5]
const MAX_SPEED: Speed = 250.0
```

Syntax authority: typl language reference §2 (lexical), §5.1 (unit types), §6.1
(value constants); general form Shape 1 (docs/wip/family-general-form.md §2).

## The end-to-end contract

```text
walking_skeleton.typl
  → lex        Vec<Token>                          (ridl-syntax)
  → parse      Parse { GreenNode, errors }         (ridl-syntax, salsa-memoized in ridl-core)
  → resolve    Resolution { symbols, diagnostics } (ridl-core)
  → check      ir::Module + Vec<CheckError>        (ridl-core → ridl-ir types)
  → generate   String (Rust source)                (backends/rust)
  → ridlc build fixture.typl --out-dir out/        (ridlc, insta golden test)
```

Expected generated Rust (semantic contract; exact formatting is whatever
prettyplease emits and the E0.9 snapshot records):

```rust
pub struct Speed(pub f64);

pub const MAX_SPEED: Speed = Speed(250.0);
```

---

### Task 1: E0.1 — Cargo workspace scaffold

Branch `e0-1-workspace-scaffold` · commits `chore(repo): …` / `ci: …` ·
implementer model: sonnet.

**Files:**

- Create: `Cargo.toml` (workspace root), `crates/ridl-syntax/`,
  `crates/ridl-core/`, `crates/ridl-ir/`, `crates/ridlc/`, `crates/ridl/` (each
  with `Cargo.toml` + minimal `src/lib.rs` or `src/main.rs`),
  `crates/ridl-syntax/fixtures/walking_skeleton.typl`, `Cargo.lock`
- Modify: `.github/workflows/ci.yml` (add `rust` job), `justfile` (add `test`
  recipe, extend `build`), `.git-std.toml` (add crate scopes), `AGENTS.md`
  (commands section), `.gitignore` (`/target`)

**Interfaces:**

- Produces: the workspace every later task adds code to. Crate names:
  `ridl-syntax`, `ridl-core`, `ridl-ir`, `ridlc` (binary `ridlc`), `ridl`
  (binary `ridl`, stub main). `ridl-core` depends on `ridl-syntax`; `ridlc`
  depends on `ridl-syntax` + `ridl-core` + `ridl-ir`.

**Steps:**

- [ ] Root `Cargo.toml`: `[workspace]` with `resolver = "3"`,
      `members = ["crates/*"]` (E0.8 adds `backends/*`), and
      `[workspace.package]` `version = "0.0.0"`, `edition = "2024"`,
      `license = "MIT"`, `repository = "https://github.com/driftsys/ridl"`.
      Crates inherit via `version.workspace = true` etc. Add an empty
      `[workspace.dependencies]` table (later tasks fill it).
- [ ] Five crates: lib crates expose one placeholder
      `pub fn crate_name() -> &'static str` with one test each;
      `ridlc/src/main.rs` and `ridl/src/main.rs` print a one-line "lands in epic
      E0/E1" pointer to docs/ROADMAP.md and exit 0.
- [ ] Commit the fixture file exactly as given above.
- [ ] `ci.yml`: add `rust` job — `actions/checkout@v4`,
      `dtolnay/rust-toolchain@stable` with `components: rustfmt, clippy`,
      `Swatinem/rust-cache@v2`, then `cargo build --workspace --locked`,
      `cargo test --workspace --locked`, `cargo fmt --all --check`,
      `cargo clippy --workspace --all-targets -- -D warnings`.
- [ ] `justfile`: add `test:` recipe running `cargo test --workspace` guarded
      like `compile` (Cargo.toml existence), and change `build` to
      `build: compile test check`.
- [ ] `.git-std.toml`: add scopes `ridl-syntax`, `ridl-core`, `ridl-ir`,
      `ridlc`, `backends` (comment says to keep the list in sync as E0 lands).
- [ ] `AGENTS.md`: update the `just compile` line (the workspace now exists) and
      add the `just test` line to the command table.
- [ ] Commit `Cargo.lock` (the workspace contains binaries).
- [ ] Verify: `cargo test --workspace` green, `just build` green.

**Done when:** all five crates build and test green locally; the `rust` CI job
exists (CI is stuck — local run is the gate). Crates.io reservation is deferred
per ADR-0006 (debt issue, needs owner credentials).

---

### Task 2: E0.2 — logos lexer

Branch `e0-2-lexer` · commits `feat(ridl-syntax): …` · implementer model:
sonnet.

**Files:**

- Create: `crates/ridl-syntax/src/syntax_kind.rs`,
  `crates/ridl-syntax/src/lexer.rs`
- Modify: `crates/ridl-syntax/src/lib.rs`, `crates/ridl-syntax/Cargo.toml` (+
  `logos` via workspace.dependencies)

**Interfaces:**

- Produces (consumed by Task 3):

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u16)]
pub enum SyntaxKind {
    // Tokens — E0 subset of the family lexer.
    TypeKw, ConstKw, Ident, IntNumber, FloatNumber,
    Colon, Eq, LBracket, RBracket, DotDot, Dot, Slash, Comma,
    Whitespace, LineComment, Error,
    // Nodes are appended by Task 3.
}

pub struct Token<'a> { pub kind: SyntaxKind, pub text: &'a str }

/// Total: concatenating token texts reproduces the input byte for byte;
/// unrecognized input becomes `Error` tokens.
pub fn lex(input: &str) -> Vec<Token<'_>>
```

**Rules (typl reference §2):** identifiers start with a letter, digits after,
underscore only in SCREAMING_SNAKE — one `Ident` token kind covers all three
conventions (the checker distinguishes them later). Integers: decimal, optional
leading `-`. Floats: must contain a decimal point, optional leading `-`, no
scientific notation. `//` line comments and whitespace (space, tab, CR, LF) are
trivia tokens. `step` is contextual — lexed as `Ident`, never a keyword.

**Steps:**

- [ ] Write failing tests first: (a) fixture lexes with zero `Error` tokens and
      round-trips (`tokens.concat() == input`); (b) `0.0..250.0` lexes as
      `FloatNumber DotDot FloatNumber` — the float regex must not swallow the
      range dots; (c) `km/h` lexes as `Ident Slash Ident`; (d) `// x` is one
      `LineComment`; (e) an unknown byte (`$`) yields `Error` and still
      round-trips.
- [ ] Implement with a `logos`-derived enum mapped into `SyntaxKind`.
- [ ] `cargo test -p ridl-syntax` green; commit.

**Done when:** token stream includes whitespace/comments; all listed tests
green.

---

### Task 3: E0.3 — hand-written parser → rowan CST

Branch `e0-3-parser` · commits `feat(ridl-syntax): …` · implementer model: opus.

**Files:**

- Create: `crates/ridl-syntax/src/parser.rs`, `crates/ridl-syntax/src/ast.rs`
- Modify: `crates/ridl-syntax/src/syntax_kind.rs` (append node kinds),
  `crates/ridl-syntax/src/lib.rs`, `crates/ridl-syntax/Cargo.toml` (+ `rowan`)

**Interfaces:**

- Consumes: `lex`, `SyntaxKind`, `Token` from Task 2.
- Produces (consumed by Tasks 4, 5, 7):

```rust
// syntax_kind.rs — appended node kinds:
//   SourceFile, TypeDecl, ConstDecl, Name, Backing, UnitExpr, Range, Literal

pub type SyntaxNode = rowan::SyntaxNode<RidlLanguage>; // rowan::Language impl

#[derive(Clone)]
pub struct Parse { /* GreenNode + Vec<SyntaxError> */ }
impl Parse {
    pub fn syntax(&self) -> SyntaxNode;
    pub fn errors(&self) -> &[SyntaxError];
}
impl PartialEq for Parse { /* green-node pointer equality */ }
impl Eq for Parse {}

pub fn parse(input: &str) -> Parse

// ast.rs — thin hand-written typed accessors (ungrammar generation is E1):
pub struct SourceFile(SyntaxNode);
impl SourceFile {
    pub fn cast(node: SyntaxNode) -> Option<Self>;
    pub fn type_decls(&self) -> impl Iterator<Item = TypeDecl>;
    pub fn const_decls(&self) -> impl Iterator<Item = ConstDecl>;
}
pub struct TypeDecl(SyntaxNode);
impl TypeDecl {
    pub fn name(&self) -> Option<String>;        // "Speed"
    pub fn unit(&self) -> Option<String>;        // "km/h" (UnitExpr text)
    pub fn range(&self) -> Option<RangeSpec>;    // { min, max, step: Option }
}
pub struct ConstDecl(SyntaxNode);
impl ConstDecl {
    pub fn name(&self) -> Option<String>;        // "MAX_SPEED"
    pub fn type_name(&self) -> Option<String>;   // "Speed"
    pub fn value(&self) -> Option<f64>;          // 250.0
}
#[derive(Debug, Clone, PartialEq)]
pub struct RangeSpec { pub min: f64, pub max: f64, pub step: Option<f64> }
```

**Grammar (E0 subset, general form Shape 1):**

```text
source_file = (type_decl | const_decl | trivia)*
type_decl   = 'type' Name ':' unit_expr range?
unit_expr   = Ident (('/' | '.') Ident)*        // "km/h", "N.m"
range       = '[' number '..' number ('step' number)? ']'
const_decl  = 'const' Name ':' Ident '=' number
```

`step` is recognized by token text `Ident("step")`. Recovery: on an unexpected
token, emit an error node/token and advance — never panic, never drop text.

**Steps:**

- [ ] Failing tests first: (a) losslessness —
      `parse(fixture).syntax().text() == fixture` including comments and
      whitespace; (b) same for a mangled input (`type 123 :: [`) with `errors()`
      non-empty; (c) AST accessors return the fixture values shown above; (d)
      `Parse` equality is green-node identity (needed by salsa in Task 4).
- [ ] Implement: token source over `lex` output, `rowan::GreenNodeBuilder`, one
      function per grammar production.
- [ ] `cargo test -p ridl-syntax` green; commit.

**Done when:** lossless tree round-trips to source for valid and broken input.

---

### Task 4: E0.4 — salsa spike (memoized parse-of-file)

Branch `e0-4-salsa-spike` · commits `feat(ridl-core): …` · implementer model:
opus.

**Files:**

- Create: `crates/ridl-core/src/db.rs`
- Modify: `crates/ridl-core/src/lib.rs`, `crates/ridl-core/Cargo.toml` (+
  `salsa` pinned exact `=0.x.y`, + `ridl-syntax`)

**Interfaces:**

- Consumes: `parse`, `Parse` from Task 3.
- Produces (consumed by Task 9):

```rust
#[salsa::input]
pub struct SourceFile { /* path: String, #[return_ref] text: String */ }

#[salsa::tracked]
pub fn parse_file(db: &dyn salsa::Database, file: SourceFile) -> Parse

#[salsa::db] // concrete database type usable from ridlc
#[derive(Default, Clone)]
pub struct RidlDatabase { /* storage */ }
```

**Steps:**

- [ ] Failing test: create two `SourceFile` inputs A and B; call `parse_file` on
      both; record executions (salsa event callback or an execution counter);
      call again — zero re-executions (memo hit); `set_text` on A only; call
      both — exactly one re-execution (A), B stays memoized.
- [ ] Implement input + tracked query over `ridl_syntax::parse`. Pin the exact
      salsa version in `workspace.dependencies` (`=` requirement) per ADR-0004
      §3. Use context7 for current salsa 0.x API if needed.
- [ ] `cargo test -p ridl-core` green; commit.

**Done when:** the invalidation test proves an edit re-parses only the edited
file.

---

### Task 5: E0.5 — trivial resolver

Branch `e0-5-resolver` · commits `feat(ridl-core): …` · implementer model:
sonnet.

**Files:**

- Create: `crates/ridl-core/src/resolve.rs`
- Modify: `crates/ridl-core/src/lib.rs`

**Interfaces:**

- Consumes: `ast::SourceFile`, `ast::TypeDecl`, `ast::ConstDecl` from Task 3.
- Produces (consumed by Task 7):

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum SymbolKind { Type, Const }

#[derive(Debug, Clone, PartialEq)]
pub struct ResolveError { pub message: String }   // e.g. unknown type name

pub struct Resolution {
    pub symbols: std::collections::HashMap<String, SymbolKind>,
    pub diagnostics: Vec<ResolveError>,
}

/// Single package, no imports: collect declared names, then verify every
/// const's type reference names a declared `type` (or is unresolved).
pub fn resolve(file: &ridl_syntax::ast::SourceFile) -> Resolution
```

**Steps:**

- [ ] Failing tests: (a) fixture resolves — `Speed` and `MAX_SPEED` in
      `symbols`, no diagnostics; (b) `const X: Missing = 1.0` yields one
      `ResolveError` naming `Missing`; (c) duplicate declaration of the same
      name yields one `ResolveError`.
- [ ] Implement two passes: declare, then reference-check.
- [ ] `cargo test -p ridl-core` green; commit.

**Done when:** names resolve within one file; the three tests are green.

---

### Task 6: E0.6 — IR v0 proto schema + prost build

Branch `e0-6-ir-proto` · commits `feat(ridl-ir): …` · implementer model: opus.

**Files:**

- Create: `crates/ridl-ir/proto/ridl/ir/v0/ir.proto`, `crates/ridl-ir/build.rs`
- Modify: `crates/ridl-ir/src/lib.rs`, `crates/ridl-ir/Cargo.toml` (+ `prost`;
  build-deps `prost-build` + `protox`; + `serde`, `serde_json` dev-dep)

**Interfaces:**

- Produces (consumed by Tasks 7, 8, 9) — the schema, verbatim:

```proto
syntax = "proto3";

package ridl.ir.v0;

// IR v0 — walking-skeleton subset: named scalar types and value constants.
message Module {
  string name = 1;
  repeated TypeDef types = 2;
  repeated ConstDef consts = 3;
}

message TypeDef {
  string name = 1;   // "Speed"
  string unit = 2;   // UCUM expression, e.g. "km/h"; empty when unitless
  Range range = 3;
}

message Range {
  double min = 1;
  double max = 2;
  double step = 3;   // 0 = unstated
}

message ConstDef {
  string name = 1;        // "MAX_SPEED"
  string type_name = 2;   // resolved named-type reference
  double value = 3;
}
```

- Rust surface: `ridl_ir::{Module, TypeDef, Range, ConstDef}` (prost types,
  re-exported at crate root), with serde derives added via `prost-build`
  `type_attribute` so the JSON debug rendering exists (ADR-0004 §4).

**Steps:**

- [ ] `build.rs`: compile the proto with `protox::compile` +
      `prost_build::Config` `compile_fds` — no system `protoc` (ADR-0006). Add
      `#[derive(serde::Serialize, serde::Deserialize)]` via
      `type_attribute(".", …)`.
- [ ] Failing test, then green: construct the fixture's `Module` in Rust,
      round-trip through `prost::Message::encode`/`decode`, assert equality;
      serialize to JSON with `serde_json` and assert it contains
      `"name":"Speed"`.
- [ ] `cargo test -p ridl-ir` green; commit.

**Done when:** the `.proto` compiles in `build.rs` and generated Rust IR types
round-trip.

---

### Task 7: E0.7 — minimal checker (AST → IR)

Branch `e0-7-checker` · commits `feat(ridl-core): …` · implementer model: opus.

**Files:**

- Create: `crates/ridl-core/src/check.rs`
- Modify: `crates/ridl-core/src/lib.rs`, `crates/ridl-core/Cargo.toml` (+
  `ridl-ir`)

**Interfaces:**

- Consumes: AST accessors (Task 3), `Resolution` (Task 5), IR types (Task 6).
- Produces (consumed by Task 9):

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct CheckError { pub message: String }

/// Lower every type/const declaration to IR. One real check, to make the
/// stage honest: a const value must lie inside its resolved type's range.
/// Diagnostics accumulate; lowering continues past errors (ADR-0004 §5).
pub fn check(
    file: &ridl_syntax::ast::SourceFile,
    resolution: &Resolution,
    module_name: &str,
) -> (ridl_ir::Module, Vec<CheckError>)
```

**Steps:**

- [ ] Failing tests: (a) fixture lowers to the exact `Module` of Task 6's test
      (name `walking_skeleton`, one `TypeDef` `{Speed, km/h, [0,250,0.5]}`, one
      `ConstDef` `{MAX_SPEED, Speed, 250.0}`); (b)
      `const TOO_FAST: Speed = 300.0` yields one `CheckError` and the const is
      still lowered; (c) a const whose type did not resolve is skipped without
      panic.
- [ ] Implement the lowering walk.
- [ ] `cargo test -p ridl-core` green; commit.

**Done when:** IR emitted for the skeleton input.

---

### Task 8: E0.8 — trivial Rust backend

Branch `e0-8-backend-rust` · commits `feat(backends): …` · implementer model:
sonnet.

**Files:**

- Create: `backends/rust/Cargo.toml` (crate `ridl-backend-rust`),
  `backends/rust/src/lib.rs`
- Modify: root `Cargo.toml` (`members` gains `backends/*`)

**Interfaces:**

- Consumes: IR types (Task 6).
- Produces (consumed by Task 9):

```rust
/// IR module → Rust source text. Each TypeDef becomes
/// `pub struct Name(pub f64);`, each ConstDef becomes
/// `pub const NAME: Type = Type(value);`. Built as a quote TokenStream,
/// formatted with prettyplease.
pub fn generate(module: &ridl_ir::Module) -> String
```

**Steps:**

- [ ] Failing tests: (a) generating from the fixture `Module` contains
      `pub struct Speed(pub f64)` and `pub const MAX_SPEED: Speed`; (b) the
      emitted source **compiles**: write to a temp dir and run
      `rustc --edition 2024 --crate-type lib --emit metadata` on it via
      `std::process::Command`, assert exit success.
- [ ] Implement with `quote` + `prettyplease` (ADR-0004 §7); float literals via
      `proc_macro2::Literal::f64_suffixed`.
- [ ] `cargo test -p ridl-backend-rust` green; commit.

**Done when:** emitted Rust compiles.

---

### Task 9: E0.9 — ridlc wiring + insta golden test

Branch `e0-9-ridlc-wiring` · commits `feat(ridlc): …` · implementer model: opus.

**Files:**

- Create: `crates/ridlc/src/main.rs` (replace stub),
  `crates/ridlc/tests/golden.rs`, snapshot(s) under
  `crates/ridlc/tests/snapshots/`
- Modify: `crates/ridlc/Cargo.toml` (+ `clap`, `ridl-syntax`, `ridl-core`,
  `ridl-ir`, `ridl-backend-rust`; dev-deps `insta`, `serde_json`)

**Interfaces:**

- Consumes: everything above. Pipeline in order: `RidlDatabase` + `parse_file`
  (Task 4) → `resolve` (Task 5) → `check` (Task 7) → `generate` (Task 8).
- Produces the CLI contract:

```text
ridlc build <INPUT.typl> --out-dir <DIR>
  writes <DIR>/<input-stem>.rs
  prints each resolve/check diagnostic to stderr, one per line
  exit 0 on success (no diagnostics), exit 1 otherwise
```

- Expose the pipeline as a library function so the test does not shell out:
  `pub fn compile(input_path, source_text) -> Result<CompileOutput, …>` in
  `crates/ridlc/src/lib.rs` with
  `CompileOutput { rust_source: String,
  module: ridl_ir::Module, diagnostics: Vec<String> }`;
  `main.rs` is a thin clap wrapper over it.

**Steps:**

- [ ] Failing golden test: `compile` the fixture; `insta::assert_snapshot!` the
      generated Rust and `insta::assert_json_snapshot!` the IR `Module`; assert
      zero diagnostics.
- [ ] Implement `compile` + the clap `build` subcommand.
- [ ] One CLI integration test: run the binary with `std::process::Command`
      (`env!("CARGO_BIN_EXE_ridlc")`) on the fixture into a temp dir; assert
      exit 0 and the output file exists.
- [ ] Review and accept the snapshots (`cargo insta review` or accept on first
      run); commit snapshots with the test.
- [ ] `cargo test --workspace` green; commit.

**Done when:** one command, one snapshot, green — the epic's exit criterion.

---

## Out of scope (deferred, recorded)

- crates.io name reservation (E0.1 "done when" clause) — needs owner
  credentials; deferred per ADR-0006 with a debt issue.
- `ridl-sem`, `ridl-lsp` crates, the coded `Diagnostic` model,
  `codespan-reporting` rendering, ungrammar-generated AST — all E1+ (ADR-0004
  §10 rings).
- Package/import syntax, structs/enums, string/bytes/regex — E1+.
