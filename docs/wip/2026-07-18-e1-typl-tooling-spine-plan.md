# Epic 1 — typl + Tooling Spine Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** typl ships as a standalone units-aware schema language with a real
editor experience — arbitrary typl packages compile with full coded diagnostics,
format with `ridl fmt`, generate Rust + extern-C, and edit live in VS Code; IR
stabilized at v1 for the typl subset (docs/ROADMAP.md, epic E1).

**Architecture:** the E0 walking skeleton grows into the concept note §8.1 crate
map. `ridl-syntax` gains the full family lexer, the full typl parser with error
recovery, and an ungrammar-generated typed AST. A new `ridl-sem` crate takes the
resolver, checker, type system, exact range/width arithmetic, UCUM units, and
init derivation. `ridl-core` keeps the salsa database and gains the ns core:
manifest, package model, lockfile, cache, fetch — plus the coded `Diagnostic`
model every pass emits. `ridl-ir` moves to IR v1 (typl surface, exact decimal
values). `backends/rust` regrows over IR v1 with an extern-C face. `ridlc` gains
stable flags; the `ridl` facade gains `check`/`build`/`fmt`; `tools/fmt` and
`crates/ridl-lsp` are new; an `editors/vscode` extension closes the loop.

**Tech Stack:** Rust edition 2024 · logos · rowan · ungrammar · salsa (pinned
`=0.28.0`) · prost + protox · num-bigint + num-rational · codespan-reporting ·
toml + serde · sha2 · ureq · regress · lsp-server + lsp-types · quote +
prettyplease · minijinja · clap · insta · proptest. All fixed by ADR-0004;
E1-specific choices recorded in ADR-0007.

## Global Constraints

- Specification authority: `docs/specification/typl-language-reference.md` (typl
  v0.1.0) is normative for everything E1 implements; where the roadmap row or
  the wip general form disagrees with it, the typl reference wins (ADR-0007
  decisions 7 and 11). The `wire` clause is **deferred** (typl §17.11) despite
  the E1.8 roadmap row mentioning it.
- Module semantics are ADR-0002; execution decisions are ADR-0007; stack choices
  are ADR-0004 and are not renegotiable inside a task.
- Diagnostics are accumulated `Diagnostic` values, never a hard error return
  (ADR-0004 §5). After task 6 lands, every new pass emits coded diagnostics
  (`TYPL-…` per typl §16, `FORM-…` per ADR-0007 decision 2). Codes are never
  renumbered or reused.
- The lexer emits trivia (whitespace, comments) as real tokens — rowan
  losslessness (ADR-0004 §2). `parse(text).syntax().text() == text` holds for
  every input, valid or broken.
- Commit messages are Conventional Commits linted by git-std against
  `.git-std.toml`; use the type and scope named in each task. New scopes are
  added in the task that introduces the crate or directory.
- Every task ends with the local gate green: `cargo test --workspace`,
  `cargo fmt --all --check`,
  `cargo clippy --workspace --all-targets -- -D warnings`, and `just check` for
  tasks that touch Markdown. CI is stuck (ADR-0006 decision 8 carries over); the
  local gate is the merge gate.
- Prose in comments and docs follows plain, literal English.
- One PR per task, squash-merged after a recorded review; never push to `main`
  directly; sync local `main` after every merge.

## Debt folding (issue #102)

Each #102 item is absorbed by a named task; the issue closes when the last
lands. Map: lexer/parser test gaps → tasks 1 and 4 corpus; accessor-layer items
(`unit()` returning `Some("")`, Eq/Hash latency, `RangeSpec` overreach) → tasks
2–4 (generated AST replaces the accessors); salsa observability (`database_key`
capture, `Clone` on the counter Arc) → task 8; diagnostics polish (message
shape, empty-file resolver test, discarded parser offsets) → task 6;
checker/backend test gaps → tasks 13, 14 and 17; duplicate-declaration tiebreak
→ task 9 (first declaration wins, ADR-0007 decision 6); `SourceFile` name
collision → task 5 (salsa input renamed `InputFile`); cosmetic Cargo.toml items
→ task 5.

## Dependency waves

```text
wave 0   PR 0 (plan + ADR-0007, this document)
wave 1   T1 lexer → T2 ungrammar AST → T3 parser → T4 recovery+migration
         → T5 sem split → T6 diagnostics
wave 2a  T7 manifest → T8 packages+salsa → T9 resolver → T16 lockfile/fetch → T19 wasm
wave 2b  T10 ranges/widths ∥ T11 ucum ∥ T12 IR v1        (parallel, after T6)
wave 2c  T13 composites checker → T14 nominal/consts/docs → T15 init derivation
         (T13 needs T9 + T10 + T11 + T12)
wave 3   T17 backend → T18 test spine ; T21 fmt (after T4, parallel)
wave 4   T20 CLI+facade (after T17, T21) → T22 LSP core → T23 hover/defs
         → T24 completion/rename → T25 inlay hints ; T26 VS Code (after T22)
close    whole-epic review → fix wave → gardening + docs sync
```

Model tiers: fable = T2, T3, T8, T12, T13, epic review; sonnet = T5, T19, T26;
opus = everything else.

---

### Task 1: E1.1 — full family lexer

Branch `e1-1-lexer` · commits `feat(ridl-syntax): …` · model: opus.

**Read first:** typl reference §1.4 (keyword registry), §2 (lexical
conventions), §2.8 (tokens lexed but rejected); ADR-0007 decision 2.

**Files:**

- Create: `crates/ridl-syntax/src/keywords.rs`
- Modify: `crates/ridl-syntax/src/syntax_kind.rs`,
  `crates/ridl-syntax/src/lexer.rs`, `crates/ridl-syntax/src/lib.rs`
- Test: `crates/ridl-syntax/src/lexer.rs` unit tests +
  `crates/ridl-syntax/test_data/lexer/*.typl` corpus with insta snapshots

**Interfaces:**

- Keeps: `lex(&str) -> Vec<Token>`; `Token { kind: SyntaxKind, text: &str }`;
  totality (concatenating token texts reproduces the input byte for byte).
- Produces (consumed by tasks 2–3):
  - `SyntaxKind` token variants: one variant per **used** typl keyword
    (`PackageKw`, `ImportKw`, `AsKw`, `InternalKw`, `TypeKw`, `ConstKw`,
    `StructKw`, `EnumKw`, `EnumsetKw`, `UnionKw`, `BooleanKw`, `IntegerKw`,
    `FloatKw`, `StringKw`, `BytesKw`, `TrueKw`, `FalseKw`, `StepKw`, `MatchKw`,
    `ReservedKw`, `ErrorKw`), one `ReservedWord` variant covering every
    family-registry word typl does not use (typl §1.4 list), and the non-keyword
    tokens: `Ident`, `IntNumber`, `FloatNumber`, `String`, `Regex`, `Duration`,
    `Colon`, `Eq`, `LBracket`, `RBracket`, `LBrace`, `RBrace`, `LParen`,
    `RParen`, `DotDot`, `Dot`, `Slash`, `Comma`, `Question`, `Semicolon`, `At`,
    `Pipe`, `Percent`, `Minus`, `Whitespace`, `LineComment`, `BlockComment`,
    `DocComment`, `Error`.
  - `keywords::FAMILY_RESERVED: &[&str]` — the full §1.4 registry (used +
    reserved), the SSOT the E4.7 governance test will later read.
  - `keywords::typl_keyword(text: &str) -> Option<SyntaxKind>` and
    `keywords::is_reserved(text: &str) -> bool`.

**Rules:** identifiers per §2.3 (one `Ident` kind covers all three case
conventions). Integers per §2.4: decimal, unary minus, leading zeros lexed but
flagged later by the parser (FORM-005). Floats per §2.5: decimal point
mandatory, no scientific notation. Strings per §2.6 with RFC 8259 escapes;
unterminated string → `Error` token to end of line. Durations per §2.8: integer
or float immediately followed by a UCUM time atom (`us`, `ms`, `s`, `min`, `h`)
lex as one `Duration` token (rejected later by the typl profile, TYPL-302).
Block comments `/* … */` without nesting; `/** … */` and `///` are `DocComment`.
Regex literals: a `Slash` whose previous non-trivia token is `Eq` starts a regex
literal — scan to the first unescaped `/`, emitting one `Regex` token; an
unterminated regex is an `Error` token to end of line (regex literals cannot
contain a raw newline). Everything else about `/` stays `Slash` (UCUM `km/h`).

**Steps:**

- [ ] Write failing tests first: (a) every used-keyword string lexes to its
      variant and every reserved word to `ReservedWord`; (b) round-trip totality
      on a corpus file containing every token kind; (c) `0.0..250.0` lexes
      float–dotdot–float; (d) `km/h` lexes ident–slash– ident while
      `const V = /a\/b/` lexes one `Regex` token; (e) `10ms`, `500us`, `1s` lex
      as `Duration`, while a bare `min` lexes as `Ident` (not in the registry; a
      duration needs the number immediately before the atom); (f) `"a\nb"` and
      an unterminated string; (g) `/* block */` and nested `/* /* */` ends at
      the first `*/`; (h) `042` lexes as `IntNumber` (flagging is the parser's
      job).
- [ ] Add corpus files under `test_data/lexer/` and an `insta::glob!`-driven
      token-dump snapshot test.
- [ ] Implement: extend the logos enum; add `keywords.rs`; add the regex
      post-pass over the raw logos stream.
- [ ] `cargo test -p ridl-syntax` green; full local gate; commit.

**Done when:** lexer corpus tests pass; every §1.4 registry word is either a
keyword variant or `ReservedWord`; totality holds on every corpus file.

---

### Task 2: E1.2a — ungrammar grammar + generated typed AST

Branch `e1-2a-ungrammar` · commits `feat(ridl-syntax): …` and `chore(repo): …`
(xtask + scope) · model: fable.

**Read first:** typl reference Appendix E (EBNF); general form §2 (three
shapes); ADR-0007 decision 1; rust-analyzer's `ungrammar` README (context7 if
needed).

**Files:**

- Create: `xtask/Cargo.toml`, `xtask/src/main.rs`, `xtask/src/codegen.rs`,
  `crates/ridl-syntax/typl.ungram`, `crates/ridl-syntax/src/ast.rs` (module:
  re-export + `AstNode` trait + `AstChildren`),
  `crates/ridl-syntax/src/ast/generated.rs`
- Modify: root `Cargo.toml` (member `xtask`), `.git-std.toml` (scope `xtask`),
  `crates/ridl-syntax/src/syntax_kind.rs` (node kinds)

**Interfaces:**

- Produces (consumed by tasks 3, 4 and every semantic task):
  - `SyntaxKind` node variants: `SourceFile`, `PackageDecl`, `Import`,
    `TypeDef`, `ConstDef`, `StructDef`, `FieldDef`, `ReservedEntry`, `EnumDef`,
    `EnumValue`, `EnumSetDef`, `EnumSetBit`, `UnionDef`, `UnionArm`,
    `TupleType`, `TupleField`, `ArrayType`, `MapType`, `OptionalType`,
    `PathType`, `PrimitiveType`, `UnitExpr`, `Constraint`, `Bound`, `Name`,
    `QualifiedName`, `Literal`, `InitValue`, `ErrorNode`.
  - `ast::AstNode` trait: `fn cast(SyntaxNode) -> Option<Self>` +
    `fn syntax(&self) -> &SyntaxNode`; `ast::AstChildren<N>` iterator.
  - Generated node structs named exactly after the node kinds, with accessors
    following rust-analyzer conventions — children by cast, tokens by kind. Key
    accessors later tasks rely on:
    `SourceFile::{package_decl(), imports(), definitions()}`;
    `TypeDef::{name(), backing(), constraint(), init_value()}` (backing returns
    an enum `ast::Backing { Primitive(PrimitiveType), Unit(UnitExpr) }`);
    `ConstDef::{name(), type_ref(), value(), regex()}`;
    `StructDef::{name(), members()}` where a member is
    `ast::StructMember { Field(FieldDef), Reserved(ReservedEntry) }`;
    `FieldDef::{name(), field_type(), init_value()}`;
    `EnumDef::{name(), values(), reserved()}`; `EnumValue::{name(), value()}`;
    `EnumSetDef::{name(), backing_ref(),
    bits()}`;
    `UnionDef::{name(), arms()}`; `UnionArm::{name(), type_ref()}`;
    `Constraint::{min(), max(), step(), len(), match_pattern()}`;
    `ast::Definition` enum over the six definition kinds with
    `{name(), is_internal(), is_error(), doc_comments()}` on each via a shared
    `ast::HasName` / `ast::HasModifiers` / `ast::HasDocComments` trait trio (doc
    comments read from preceding trivia).
  - `cargo xtask codegen` regenerates `ast/generated.rs` from `typl.ungram`; an
    xtask test fails when the committed file drifts from the grammar.

**Steps:**

- [ ] Write `typl.ungram` transliterating Appendix E (node names above; the
      grammar file is the node-inventory SSOT).
- [ ] Failing xtask drift test: regenerate into a buffer, compare with the
      committed `generated.rs`.
- [ ] Implement the generator (ungrammar crate → node structs + accessors + the
      `SyntaxKind` node list assertion). Keep the generator ~small: no
      token-shorthand cleverness the grammar does not need.
- [ ] Generate, commit the generated file, wire `ast.rs` (trait + manual
      `Definition`/`Backing`/`StructMember` enums + trait trio).
- [ ] Unit tests: `cast` round-trips on hand-built green nodes for `TypeDef` and
      `StructDef` (build via `GreenNodeBuilder` directly — the full parser does
      not exist yet).
- [ ] Full local gate; commit.

**Done when:** `cargo xtask codegen` is idempotent, the drift test guards it,
and the generated accessors compile with the unit tests green. The E0
hand-written accessors in the old `ast` module remain untouched until task 4.

---

### Task 3: E1.2b — full typl parser

Branch `e1-2b-parser` · commits `feat(ridl-syntax): …` · model: fable.

**Read first:** typl reference §2–§12, §15.2, Appendix E; general form §2–§3
(shapes and invariants); task 2's `typl.ungram` (the node contract).

**Files:**

- Modify: `crates/ridl-syntax/src/parser.rs` (rewrite),
  `crates/ridl-syntax/src/lib.rs`
- Test: `crates/ridl-syntax/test_data/parser/ok/*.typl` + insta snapshots

**Interfaces:**

- Keeps: `parse(&str) -> Parse`; `Parse::{syntax(), errors()}`; green-node
  identity equality (salsa relies on it).
- Changes (consumed by tasks 4, 6):
  `SyntaxError { message: String,
  range: rowan::TextRange }` — the E0
  `offset: usize` becomes a real range.
- Produces: a CST for the full typl grammar whose nodes match `typl.ungram`
  exactly, for every construct: package, imports with alias, all six definition
  kinds with `internal`/`error` modifiers, doc comments as trivia, constraints
  (range/step, length, `match` with regex or named constant), unit expressions,
  init values, optionality, arrays, maps, tuples, reserved tombstones (name form
  and integer form), newline/comma separators with trailing comma, qualified
  references.

**Rules:** recursive descent, one function per production; `'step'`, `'match'`,
`'reserved'` are their keyword tokens. Missing `package` emits a `SyntaxError`
but parsing continues. Separator discipline per §15.2. Leading zeros in an
integer literal emit FORM-005 wording (`integer literal has leading zeros`) as a
`SyntaxError` (the coded model arrives in task 6). The parser never panics and
never drops text.

**Steps:**

- [ ] Failing tests first: an `ok` corpus covering (a) the typl reference
      Appendix B full example verbatim; (b) the Appendix A `ridl.std` source
      verbatim; (c) one file per construct family (types+units, consts+regex,
      structs+reserved+init, enums+enumsets both forms, unions incl. `error` and
      result shape, tuples, collections, imports+alias+internal, comma
      separators and trailing commas). Snapshot = CST debug dump + errors; every
      file asserts losslessness and zero errors.
- [ ] Implement production by production, keeping the E0 subset tests green
      throughout.
- [ ] Update the E0 fixture `walking_skeleton.typl` to start with
      `package fixtures` (grammar now requires it) and adjust E0-era snapshots.
- [ ] Full local gate; commit per production group (package/import, type/ const,
      struct, enum/enumset, union, collections/tuples).

**Done when:** the whole `ok` corpus parses lossless with zero errors and CST
snapshots reviewed; Appendix B and `ridl.std` parse clean.

---

### Task 4: E1.2c — error recovery + corpus, retire the E0 accessors

Branch `e1-2c-recovery` · commits `feat(ridl-syntax): …` /
`refactor(ridl-core): …` · model: opus.

**Read first:** typl reference §2; the E0 accessor layer
(`crates/ridl-syntax/src/ast.rs` pre-task-2 module) and its consumers
(`crates/ridl-core/src/{resolve,check}.rs`, `crates/ridlc/src/lib.rs`).

**Files:**

- Modify: `crates/ridl-syntax/src/parser.rs` (recovery),
  `crates/ridl-syntax/src/lib.rs` (delete the hand-written accessor module and
  its root re-exports; the generated `ast` module is the only AST),
  `crates/ridl-core/src/resolve.rs`, `crates/ridl-core/src/check.rs`,
  `crates/ridlc/src/lib.rs` (port to `ridl_syntax::ast`)
- Test: `crates/ridl-syntax/test_data/parser/err/*.typl` + insta snapshots

**Interfaces:**

- Consumes: generated AST (task 2), parser (task 3).
- Produces: recovery contract every later consumer relies on — a broken
  declaration yields an `ErrorNode` covering the skipped tokens, parsing
  resynchronizes at the next top-level keyword or `}`, and every `SyntaxError`
  carries the narrowest honest `range`.

**Steps:**

- [ ] Failing `err` corpus first: valid→garbage→valid resynchronization (the
      #102 recovery gap); unclosed brace; unclosed bracket; missing name;
      missing `=` in enum value; broken constraint; stray reserved word as
      identifier (`type signal : …`); mangled import. Snapshots assert
      losslessness, the error list, and that declarations after the garbage
      still produce real nodes.
- [ ] Implement recovery points (top-level keywords, `}`, separator boundaries)
      in the parser.
- [ ] Port `resolve.rs`, `check.rs`, `ridlc` to the generated AST; delete the
      hand-written accessors; keep behavior identical (E0 tests stay green,
      including `unit()` now returning `None` for an absent unit — the #102
      `Some("")` item dies with the old accessor).
- [ ] Full local gate; commit.

**Done when:** `err` corpus snapshots reviewed; the hand-written accessor layer
is gone; E0 pipeline tests still green.

---

### Task 5: carve `ridl-sem`, rename the salsa input

Branch `e1-sem-split` · commits `refactor(ridl-core): …` / `chore(repo): …` ·
model: sonnet.

**Read first:** ADR-0006 decision 2; concept note §8.1; walking-skeleton
technote (cross-seam facts 1); ADR-0007 decision 4.

**Files:**

- Create: `crates/ridl-sem/Cargo.toml`, `crates/ridl-sem/src/lib.rs`,
  `crates/ridl-sem/src/resolve.rs` (moved), `crates/ridl-sem/src/check.rs`
  (moved)
- Modify: `crates/ridl-core/src/lib.rs`, `crates/ridl-core/src/db.rs`
  (`SourceFile` → `InputFile`), `crates/ridlc/Cargo.toml` +
  `crates/ridlc/src/lib.rs` (imports; drop the `SourceFile as …` aliases;
  alphabetize the path-dep list and unify `x.workspace = true` style — the #102
  cosmetic items), `.git-std.toml` (scope `ridl-sem`)

**Interfaces:**

- Produces: `ridl_sem::{resolve, check}` (contents unchanged);
  `ridl_core::db::InputFile` (fields unchanged: `path`, `text`); `ridl-sem`
  depends on `ridl-syntax` + `ridl-core` + `ridl-ir`.

**Steps:**

- [ ] Move the two modules verbatim; rename the input; fix imports; no behavior
      change (git should show renames).
- [ ] Full local gate (all E0 tests green unchanged); commit.

**Done when:** the workspace builds with `ridl-sem` in place and no public type
named `SourceFile` outside `ridl_syntax::ast`.

---

### Task 6: E1.10 — coded diagnostics framework

Branch `e1-10-diagnostics` · commits `feat(ridl-core): …` · model: opus.

**Read first:** ADR-0004 §5; ADR-0007 decision 2; typl reference §16 (catalogue
structure); the #102 diagnostics-polish items.

**Files:**

- Create: `crates/ridl-core/src/diag.rs` (model + catalogue constants),
  `crates/ridl-core/src/diag/render.rs` (codespan-reporting)
- Modify: `crates/ridl-core/Cargo.toml` (+ `codespan-reporting` via
  workspace.dependencies), `crates/ridl-sem/src/{resolve,check}.rs`,
  `crates/ridlc/src/lib.rs`, `crates/ridlc/src/main.rs`
- Test: unit tests in `diag.rs` + updated `ridlc` golden snapshots

**Interfaces:**

- Produces (consumed by every later task):

```rust
pub struct DiagCode(pub &'static str);          // "TYPL-108", "FORM-101"
pub enum Severity { Error, Warning, Info }
pub struct Span { pub file: FileId, pub range: rowan::TextRange }
pub struct Label { pub span: Span, pub message: String }
pub struct FixIt { pub span: Span, pub replacement: String, pub label: String }
pub struct Diagnostic {
    pub code: DiagCode,
    pub severity: Severity,
    pub message: String,
    pub primary: Span,
    pub labels: Vec<Label>,
    pub fixits: Vec<FixIt>,
}
pub struct SourceMap { /* FileId -> (path, text) for rendering */ }
pub fn render(diags: &[Diagnostic], sources: &SourceMap) -> String
```

- Message-shape rule (fixes the #102 inconsistency): every message is
  description-first with backticked names — `unknown type name \`X\``,`duplicate
  declaration of \`X\``.
- Initial FORM catalogue (ADR-0007 decision 2): FORM-001 invalid character,
  FORM-002 unterminated string, FORM-003 unterminated regex, FORM-004
  unterminated block comment, FORM-005 leading zeros in integer literal,
  FORM-101 expected token, FORM-102 unexpected token, FORM-103 unclosed
  delimiter, FORM-104 missing `package` declaration, FORM-105 reserved word used
  as identifier.
- `ridlc::compile` maps `SyntaxError { message, range }` to coded `Diagnostic`s
  (the parser tags each error with its FORM code string — add a
  `code: &'static str` field to `SyntaxError`), reclaiming the offsets E0
  discarded. `CompileOutput.diagnostics` becomes `Vec<Diagnostic>`; the binary
  renders with `render` to stderr; exit codes keep their E0 meaning (0 clean, 1
  diagnostics with at least one error, 2 I/O), warnings alone exit 0.

**Steps:**

- [ ] Failing tests: render snapshot for a two-diagnostic input (one parser
      FORM-101 with caret span, one resolver TYPL-styled message); a fix-it
      carrying diagnostic renders its suggestion; empty-file input produces zero
      diagnostics (the #102 empty-file resolver test).
- [ ] Implement the model + renderer; port the resolver/checker error structs to
      `Diagnostic`. Code assignment for the E0-era checks: duplicate declaration
      → TYPL-009, const value out of range → TYPL-108, parser items → their FORM
      codes. The unknown-type-name check keeps a message without a code until
      task 13 rehomes it (no §16 code exists for it; it surfaces through the
      resolver import rules and the checker's lowering skips).
- [ ] Update golden snapshots (diagnostics are now structured; JSON snapshot the
      `Vec<Diagnostic>` shape).
- [ ] Full local gate; commit.

**Done when:** one struct renders to the terminal via codespan-reporting with
correct spans from real parser offsets; all diagnostics in the pipeline carry
codes.

---

### Task 7: E1.5 — manifest (`ridl.toml`)

Branch `e1-5-manifest` · commits `feat(ridl-core): …` · model: opus.

**Read first:** ADR-0002 §4 (both modes, verbatim examples); typl reference
glossary (manifest/lockfile rows); ADR-0007 decision 5.

**Files:**

- Create: `crates/ridl-core/src/manifest.rs`
- Modify: `crates/ridl-core/Cargo.toml` (+ `toml`, `serde` already present)
- Test: unit tests in `manifest.rs`

**Interfaces:**

- Produces (consumed by tasks 8, 9, 16):

```rust
pub struct Manifest {
    pub kind: ManifestKind,
    pub imports: BTreeMap<String, String>,   // logical package -> URL
}
pub enum ManifestKind {
    Package { name: String, version: String },
    Workspace { members: Vec<String> },
}
/// Parse + validate one ridl.toml. Diagnostics, not Err, for content
/// problems (unknown keys, both sections present, neither present,
/// invalid member path); Err only for unreadable TOML syntax is also a
/// Diagnostic (FORM-free: manifest diagnostics use TYPL-0xx? No —
/// ADR-0007 decision 2 allocates MANI-001.., the manifest namespace).
pub fn parse_manifest(path: &str, text: &str) -> (Option<Manifest>, Vec<Diagnostic>)
```

- Manifest diagnostics (ADR-0007 decision 2): MANI-001 invalid TOML, MANI-002
  both `[package]` and `[workspace]`, MANI-003 neither section, MANI-004 nested
  workspace (a member's manifest declares `[workspace]` — detected in task 8
  when members load), MANI-005 unknown key (warning), MANI-006 invalid package
  name (not lowercase-dot), MANI-007 invalid import URL.

**Steps:**

- [ ] Failing tests: both ADR-0002 §4 examples parse to the exact structs; each
      MANI code has a test; unknown keys warn but parse.
- [ ] Implement with `toml` + serde into raw shape, then validate into
      `Manifest`.
- [ ] Full local gate; commit.

**Done when:** both modes parse; every listed diagnostic fires in a test.

---

### Task 8: E1.3 — package model, multi-file salsa, package↔directory law

Branch `e1-3-packages` · commits `feat(ridl-core): …` · model: fable.

**Read first:** ADR-0002 §1, §4–5; typl reference §3.1, Appendix A; ADR-0007
decisions 4, 5, 15 (ridl.std embedding); #102 salsa items.

**Files:**

- Create: `crates/ridl-core/src/workspace.rs` (fs discovery — the only
  fs-touching module, feature-gated `fs`, default on),
  `crates/ridl-core/src/package.rs` (salsa inputs + queries),
  `crates/ridl-core/src/std_lib.rs` (embedded `ridl.std` via `include_str!`),
  `crates/ridl-core/assets/ridl_std.typl` (Appendix A source, verbatim)
- Modify: `crates/ridl-core/src/db.rs`, `crates/ridl-core/src/lib.rs`
- Test: `workspace.rs` + `package.rs` tests over tempdir fixtures

**Interfaces:**

- Produces (consumed by tasks 9, 13, 16, 20, 22):

```rust
// salsa inputs
#[salsa::input] pub struct InputFile { path: String, #[return_ref] text: String }
#[salsa::input] pub struct Package {
    name: String,                       // "veh.common"
    #[return_ref] files: Vec<InputFile>,
    origin: PackageOrigin,              // WorkspaceMember | Remote | Std
}
#[salsa::input] pub struct Workspace {
    #[return_ref] packages: Vec<Package>,
    #[return_ref] imports: BTreeMap<String, String>,  // merged per ADR-0002 §5
}
// discovery (fs, outside salsa): walk from a path (file, package dir, or
// workspace root), read manifests, load .typl files, build the inputs.
pub struct LoadedWorkspace { pub workspace: Workspace, pub diagnostics: Vec<Diagnostic> }
pub fn load_workspace(db: &mut RidlDatabase, entry: &Path) -> std::io::Result<LoadedWorkspace>
// queries
#[salsa::tracked] pub fn parse_file(db, file: InputFile) -> Parse          // kept
#[salsa::tracked] pub fn package_of(db, ws: Workspace, name: String) -> Option<Package>
pub fn std_package(db: &mut RidlDatabase) -> Package    // memoized embed of ridl.std
```

- Package↔directory law: every file in a package directory must declare that
  package (TYPL-002 with the mismatching `package` line as primary span); more
  than one `package` declaration per file is TYPL-001; `load_workspace` on a
  bare `.typl` file with no manifest anywhere up the tree yields a single
  synthetic package named from the file's declared package, exempt from TYPL-002
  (single-file mode, task 20 CLI contract).
- Salsa hygiene (the #102 items): `RidlDatabase` stops deriving `Clone`; the
  invalidation test asserts the re-executed query's `database_key`.

**Steps:**

- [ ] Failing tests: (a) a two-file package loads, both files parse, and editing
      one re-parses only it (`database_key` asserted); (b) TYPL-002 fires on a
      mismatching file; (c) TYPL-001 on a double declaration; (d) workspace mode
      loads two members and merges `[imports]` per ADR-0002 §5 (member shadows
      workspace); (e) MANI-004 fires when a member declares `[workspace]`; (f)
      `std_package` parses clean and exposes `ridl.std`; (g) single-file mode
      compiles the E0 fixture.
- [ ] Implement discovery + inputs + queries.
- [ ] Full local gate; commit.

**Done when:** arbitrary multi-file packages and workspaces load into salsa with
the law enforced and `ridl.std` available.

---

### Task 9: E1.4 — resolver: order, imports, cycles, one tiebreak

Branch `e1-4-resolver` · commits `feat(ridl-sem): …` · model: opus.

**Read first:** ADR-0002 §2, §5–6; typl reference §3.2–3.4, §16.1; ADR-0007
decision 6; walking-skeleton technote cross-seam fact 2.

**Files:**

- Modify: `crates/ridl-sem/src/resolve.rs` (rewrite over packages)
- Test: resolver tests over in-memory workspaces

**Interfaces:**

- Consumes: task 8 inputs/queries.
- Produces (consumed by tasks 13, 14, 22, 23):

```rust
pub enum SymbolKind { Type, Const, Struct, Enum, EnumSet, Union }
pub struct Symbol {
    pub name: String, pub package: String, pub kind: SymbolKind,
    pub internal: bool, pub is_error: bool,
    pub file: InputFile, pub range: rowan::TextRange,   // declaration site
}
pub struct Resolution {
    pub symbols: HashMap<String, Symbol>,        // package-local view:
    // locals + ridl.std + imported names (alias-aware), first wins
    pub diagnostics: Vec<Diagnostic>,
}
#[salsa::tracked] pub fn resolve_package(db, ws: Workspace, pkg: Package) -> Resolution
```

- Resolution order per ADR-0002 §5: workspace member → package `[imports]` →
  workspace `[imports]` → error. In E1, a URL alias that is not materialized in
  the cache resolves to `unresolved remote import` until task 16 lands fetch
  (the diagnostic text names the URL).
- Rules with codes: duplicate in-package declaration → TYPL-009 on the **later**
  declaration, first wins everywhere (decision 6 — the checker consults the same
  winner); wildcard/relative import → TYPL-003; cross-package cycle → TYPL-004
  (DFS over package import edges); colliding imports without alias → TYPL-006;
  unused import → TYPL-007 (warning); alias without collision → TYPL-008
  (warning). An unknown type reference carries no TYPL code (§16 defines none
  for it) and stays checker territory (task 13, keeping the E0 behavior with the
  task 6 message shape). Qualified references (`pkg.Type`) resolve without an
  import per §3.2.

**Steps:**

- [ ] Failing tests: one per rule above, plus the ADR-0002 §5 order test (member
      shadows package imports shadows workspace imports) and a duplicate test
      proving first-wins (both resolver symbol and, once task 13 lands, lowered
      IR — leave a `// task 13 asserts the IR half` note).
- [ ] Implement two passes per package (declare, then imports/refs), plus the
      workspace-level cycle walk.
- [ ] Full local gate; commit.

**Done when:** resolver honors ADR-0002 §5–6 with every TYPL-0xx rule tested;
first-wins is the single tiebreak.

---

### Task 10: E1.8a — exact ranges, steps, and width derivation

Branch `e1-8a-ranges` · commits `feat(ridl-sem): …` · model: opus.

**Read first:** typl reference §4.2–§4.5, §5.5–§5.6, §9.3, §16.2 (TYPL-104, 105,
111); ADR-0004 §9 (num-bigint/num-rational); ADR-0007 decision 9.

**Files:**

- Create: `crates/ridl-sem/src/scalar.rs`
- Modify: `crates/ridl-sem/src/lib.rs`, workspace `Cargo.toml` (+ `num-bigint`,
  `num-rational`)
- Test: unit tests in `scalar.rs` + proptest boundary checks

**Interfaces:**

- Produces (consumed by tasks 13, 15, 17, 18):

```rust
/// Exact literal: parsed from source text, never through f64.
pub struct ExactValue(pub num_rational::BigRational);
impl ExactValue {
    pub fn parse(text: &str) -> Option<ExactValue>;       // int or decimal form
    pub fn to_decimal_string(&self) -> String;            // canonical, lossless
}
pub enum IntWidth { U8, I8, U16, I16, U32, I32, U64OrI64 } // §4.2 table order
pub enum FloatWidth { F32, F64 }
pub struct IntRange { pub min: ExactValue, pub max: ExactValue }
pub fn derive_int_width(r: &IntRange) -> Result<IntWidth, WidthError>   // TYPL-111 outside i64 domain
pub struct FloatRange { pub min: ExactValue, pub max: ExactValue, pub step: Option<ExactValue> }
pub fn derive_float_width(r: &FloatRange) -> FloatWidth   // count-based §4.3, binary32 representability
pub fn enumset_width(highest_bit: u32) -> IntWidth        // §9.3
pub fn validate_range(min: &ExactValue, max: &ExactValue) -> Option<DiagKind>  // TYPL-104
pub fn validate_step(r: &FloatRange) -> Option<DiagKind>  // TYPL-105: non-positive, > range, type mismatch
```

- Exactness invariant: no f64 anywhere in this module; §4.3's
  `N = (max − min)/step + 1` computed in rationals; binary32 representability =
  value equals its f32 round-trip _computed exactly_ (mantissa/exponent check on
  the rational, not via `as f32`).

**Steps:**

- [ ] Failing tests: every row of the §4.2 table; both §4.3 conditions (incl.
      the errata example `[0.0..1000000.0 step 0.001]` → F64 and
      `[0.0..250.0 step 0.5]` → F32); `0.1` is not binary32-representable
      exactly but `0.5` is; TYPL-111 on `[0..2^63]`; TYPL-104/105 cases; §9.3
      all four rows; `to_decimal_string` round-trips `0.1` and
      `9223372036854775807`.
- [ ] proptest: for random rational ranges with step, every value `min + n·step`
      within range fits the derived width.
- [ ] Implement.
- [ ] Full local gate; commit.

**Done when:** width/range math is exact, tested against every spec table, and
property-tested.

---

### Task 11: E1.8b — UCUM unit expressions

Branch `e1-8b-ucum` · commits `feat(ridl-sem): …` · model: opus.

**Read first:** typl reference §5.1, §16.2 (TYPL-110); ADR-0007 decision 8
(curated atom table); the UCUM spec's term grammar (ucum.org, WebFetch if
needed).

**Files:**

- Create: `crates/ridl-sem/src/ucum.rs`
- Test: unit tests in `ucum.rs`

**Interfaces:**

- Produces (consumed by tasks 13, 17, 23):

```rust
pub struct UcumExpr { pub canonical: String }   // normalized source form
pub fn parse_ucum(text: &str) -> Result<UcumExpr, UcumError>
pub enum UcumError { UnknownAtom(String), Malformed(String) }
pub fn known_atoms() -> &'static [&'static str]
```

- Grammar: UCUM term syntax — atoms with optional SI prefixes, `.`
  multiplication, `/` division, integer exponents (`m/s2`, `N.m`), leading `/`
  (`/min`), `%`, `10*` powers excluded (out of the curated set). Case-sensitive
  per §5.1.
- Curated atom table (decision 8): SI base (`m`, `g`, `s`, `A`, `K`, `mol`,
  `cd`) + all SI prefixes, derived (`N`, `Pa`, `bar`, `J`, `W`, `V`, `Ohm`, `F`,
  `Hz`, `T`, `lm`, `lx`, `C`), accepted (`min`, `h`, `d`, `L`, `t`, `eV`, `u`),
  special (`Cel`, `%`), and the reference's automotive set (`km/h` decomposes to
  `km` over `h`, `/min`, `m/s2`). Unknown atom → `UnknownAtom` → TYPL-110 at the
  call site.

**Steps:**

- [ ] Failing tests: every unit in typl §5.1's examples and Appendix A/B parses;
      `KM/H` fails (case); `furlong` fails; `m/s2`, `N.m`, `/min`, `%` parse;
      prefix `k` composes with `m` but not with `Cel` (special units take no
      prefix per UCUM).
- [ ] Implement the term parser + table.
- [ ] Full local gate; commit.

**Done when:** every spec-cited unit parses, junk is rejected with the offending
atom named.

---

### Task 12: E1.11 — IR v1 (typl surface, exact values)

Branch `e1-11-ir-v1` · commits `feat(ridl-ir): …` · model: fable.

**Read first:** typl reference §4–§12, §14.3, Appendix D (fixed_layout note);
concept note §8.2; ADR-0007 decision 9; ADR-0004 §4.

**Files:**

- Create: `crates/ridl-ir/proto/ridl/ir/v1/ir.proto`
- Modify: `crates/ridl-ir/build.rs`, `crates/ridl-ir/src/lib.rs` (module `v1`
  re-export; keep `v0` compiled until task 13 retires its last consumer, then
  remove v0 in task 13)
- Test: round-trip + JSON tests in `ridl-ir`

**Interfaces:**

- Produces (consumed by tasks 13, 15, 17, 20, 22) — proto package `ridl.ir.v1`,
  Rust path `ridl_ir::v1::*`. Message inventory (field numbering and internal
  layout are this task's design work; the names and semantics below are the
  contract):
  - `Package { name, decls: repeated Decl }`
  - `Decl { name, visibility: Visibility, is_error: bool, doc: string,
    labels: repeated string, deprecated: optional string,
    kind: oneof { type_def: TypeDef, const_def: ConstDef,
    struct_def: StructDef, enum_def: EnumDef, enum_set_def: EnumSetDef,
    union_def: UnionDef } }`
  - `TypeDef { backing: Backing, constraint: Constraint,
    declared_init: optional string, init: InitValue }`
  - `ConstDef { type_ref: optional string, value: string,
    regex: optional string }`
  - `StructDef { members: repeated StructMember }`;
    `StructMember { oneof { field: Field, reserved: Reserved } }`;
    `Field { name, ordinal: uint32, type: FieldType,
    declared_init: optional string, init: InitValue, doc, labels,
    deprecated }`;
    `Reserved { ordinal: uint32, name: optional string,
    value: optional int64 }`
  - `EnumDef { values: repeated EnumValue, reserved: repeated Reserved }`;
    `EnumValue { name, value: int64, doc }`
  - `EnumSetDef { backing_enum: optional string,
    bits: repeated EnumValue, width: IntWidth }`
  - `UnionDef { arms: repeated UnionArm, is_result: bool }`;
    `UnionArm { name, ordinal: uint32, type_ref: string }`
  - `FieldType { oneof { named: string, primitive: PrimitiveType,
    inline_scalar: TypeDef, tuple: TupleType, array: ArrayType,
    map: MapType } , optional: bool }`;
    `TupleType { fields: repeated
    TupleField }`;
    `ArrayType { element: FieldType, min: uint64,
    max: uint64 }`;
    `MapType { key: FieldType, value: FieldType,
    min: uint64, max: uint64 }`
  - `Backing { oneof { primitive: PrimitiveType, unit: string } }` (unit =
    canonical UCUM)
  - `Constraint { min: optional string, max: optional string,
    step: optional string, len_min: optional uint64,
    len_max: optional uint64, pattern: optional string,
    pattern_const: optional string }`
    — **every numeric value is a canonical decimal string** (decision 9), never
    a double
  - `InitValue { derivable: bool, value: optional string }` — the value is the
    canonical text form of a scalar init; composite inits are not materialized
    in the IR — consumers derive them recursively from member inits (document
    this rule in the proto comments)
  - `IntWidth` / `FloatWidth` / `PrimitiveType` / `Visibility` enums
  - `fixed_layout: bool` on `StructDef` (Appendix D FlatBuffers note)
- serde derives on all v1 types (JSON debug rendering, ADR-0004 §4); a
  `v1::to_json_pretty(&Package) -> String` helper.

**Steps:**

- [ ] Write the proto with doc comments citing the typl reference section for
      every message.
- [ ] Failing tests: construct the Appendix B example's `Package` (or a
      representative slice: one unit type, one const, one struct with reserved +
      optional + inline scalar, one enum + enumset, one result union) in Rust;
      prost round-trip; JSON round-trip; JSON contains `"min": "0.0"` as a
      string (exactness visible).
- [ ] Full local gate; commit.

**Done when:** IR v1 compiles, round-trips, and its JSON dump is exact and
readable. v0 stays until task 13 flips the pipeline.

---

### Task 13: E1.7a — checker I: composites, lowering to IR v1

Branch `e1-7a-composites` · commits `feat(ridl-sem): …` · model: fable.

**Read first:** typl reference §7–§12, §16.3 (all TYPL-2xx); tasks 9–12
interfaces above; walking-skeleton technote cross-seam fact 2.

**Files:**

- Modify: `crates/ridl-sem/src/check.rs` (rewrite over packages + IR v1),
  `crates/ridl-ir/src/lib.rs` + `build.rs` (remove v0; `ridlc` golden snapshots
  move to v1 JSON), `crates/ridlc/src/lib.rs` (pipeline calls `check_package`)
- Test: checker tests + updated golden snapshots

**Interfaces:**

- Consumes: `resolve_package` (task 9), `scalar` (task 10), `ucum` (task 11), IR
  v1 (task 12).
- Produces (consumed by tasks 14, 15, 17, 20, 22):

```rust
pub struct CheckedPackage {
    pub ir: ridl_ir::v1::Package,
    pub diagnostics: Vec<Diagnostic>,
}
#[salsa::tracked] pub fn check_package(db, ws: Workspace, pkg: Package) -> CheckedPackage
```

- Checks with codes (each lowers as far as honesty allows; first-wins duplicates
  lower once): TYPL-201/202 unbounded array/map; TYPL-203 enum values explicit +
  unique; TYPL-204 union arm must be a named type; TYPL-206 recursive composite
  reference (DFS over composite reference graph, direct or transitive); TYPL-207
  enumset bit uniqueness; TYPL-208 `string`/`bytes` direct field use; TYPL-209
  map key shape; TYPL-210 redeclared reserved name/value; TYPL-211
  duplicate/dangling reserved (warning); TYPL-212 `error` on a non-composite;
  TYPL-213 result-union shape; TYPL-214 `error union` arm not error-typed;
  TYPL-104/105/110/111 surfaced from tasks 10–11 at the declaring span;
  TYPL-101/102/103 missing-constraint warnings (§4, with the `[0..256]` default
  applied for string/bytes). Ordinals assigned 1-based per §7.4 counting
  reserved slots. Unknown type reference keeps the E0 message shape (task 6) —
  description-first.
- TYPL-205 (repeated tuple shape) is a linter concern — **skip in E1**, record
  in the debt roll-up.

**Steps:**

- [ ] Failing tests: one per code above (including the §7.4 tombstone example
      verbatim and a two-arm result union vs a TYPL-213 three-arm mix), plus:
      Appendix B lowers clean end to end and its IR JSON snapshot is reviewed;
      the duplicate-declaration test now asserts the IR carries exactly the
      first declaration (closing cross-seam fact 2).
- [ ] Implement lowering + checks; flip `ridlc` to v1; delete IR v0.
- [ ] Full local gate; commit.

**Done when:** all typl composites check and lower; Appendix B's IR snapshot is
the new golden; v0 is gone.

---

### Task 14: E1.7b — checker II: nominal typing, consts, visibility, docs

Branch `e1-7b-nominal` · commits `feat(ridl-sem): …` · model: opus.

**Read first:** typl reference §5.7, §6, §3.3, §14, §16.1–16.5 (TYPL-005, 106,
107, 108, 401–405); ADR-0007 decision 10 (regress; TYPL-107/401–403 scope cuts).

**Files:**

- Create: `crates/ridl-sem/src/docs.rs` (doc-comment tag scanner)
- Modify: `crates/ridl-sem/src/check.rs`, workspace `Cargo.toml` (+ `regress`)
- Test: checker tests

**Interfaces:**

- Consumes/extends task 13's `check_package`.
- Produces: const evaluation used by task 15 —

```rust
/// A const's value as an exact scalar, or a regex, resolved for use in
/// bounds and inits (constants usable as bounds — Appendix E scalar rule).
pub enum ConstValue { Number(ExactValue), Bool(bool), Text(String), Regex(String) }
pub fn const_value(res: &Resolution, name: &str) -> Option<ConstValue>
```

- Checks: nominal identity per §5.7 — a const of named type accepts a literal
  satisfying the constraints (TYPL-108 otherwise) and never a value of another
  named type; constants as range bounds resolve through `const_value` (a const
  bound referencing a non-numeric const → TYPL-105 type-mismatch arm); TYPL-106
  invalid regex via `regress::Regex::new`; TYPL-005 public declaration exposing
  an `internal` type (fields, arms, bounds constants, backing); doc tags per
  §14: `@see` (unvalidated), `@labels` (pass-through into IR `labels`),
  `@deprecated` reason → TYPL-405 warning when the reason string is missing;
  TYPL-404 blank line between doc comment and definition (warning). TYPL-107 and
  TYPL-401–403 are **deferred** (decision 10): 107 needs regex length analysis,
  401 needs markdown reference resolution, 402/403 need assurance profiles that
  do not exist in V1 yet — debt roll-up.

**Steps:**

- [ ] Failing tests: §5.7's `Speed`/`Torque` non-assignability (const of type
      Speed = const-of-Torque reference → TYPL-108 shape); regex const in a
      `match` bound; TYPL-106 on `/[/`; TYPL-005 on a public struct with an
      internal field type; `@deprecated` with and without reason; TYPL-404;
      labels land in IR JSON.
- [ ] Implement.
- [ ] Full local gate; commit.

**Done when:** nominal + const + visibility + doc semantics are checked with
every non-deferred code tested.

---

### Task 15: E1.9 — init-value derivation

Branch `e1-9-init` · commits `feat(ridl-sem): …` · model: opus.

**Read first:** typl reference §5.8 (whole table), §16.2 (TYPL-109, 115).

**Files:**

- Create: `crates/ridl-sem/src/init.rs`
- Modify: `crates/ridl-sem/src/check.rs`
- Test: `init.rs` unit tests, one per §5.8 table row

**Interfaces:**

- Consumes: `ExactValue`/`ConstValue` (tasks 10, 14), IR v1 `InitValue`.
- Produces: every lowered `TypeDef` and `Field` carries a populated `InitValue`
  — declared (validated by TYPL-109) or derived per the §5.8 table; a
  non-derivable init with no declaration marks `InitValue { derivable: false }`
  and emits TYPL-115 (info).

**Steps:**

- [ ] Failing tests: every §5.8 row — boolean false; numeric 0-in-range vs min
      when 0 outside; string/bytes empty vs non-derivable when min-length > 0;
      `match`-typed non-derivable; enum with 0 vs lowest; enumset empty; struct
      recursive with optional absent; union first arm; tuple; collection
      min-count. Plus TYPL-109 on `= 300.0` for `[0.0..250.0]` and TYPL-115 info
      on a pattern type without `= v`.
- [ ] Implement derivation over IR shapes (post-lowering pass in
      `check_package`).
- [ ] Full local gate; commit.

**Done when:** derived inits match the spec table exactly; IR carries them.

---

### Task 16: E1.6 — lockfile, cache, fetch, `--frozen`

Branch `e1-6-lockfile` · commits `feat(ridl-core): …` + `feat(ridlc): …` ·
model: opus.

**Read first:** ADR-0002 §5, §7 (verbatim); ADR-0004 §9 (`ureq`, `sha2`);
ADR-0007 decision 5 (feature gates).

**Files:**

- Create: `crates/ridl-core/src/lock.rs`, `crates/ridl-core/src/cache.rs`,
  `crates/ridl-core/src/fetch.rs` (feature `fetch`, default on, implies `fs`)
- Modify: `crates/ridl-core/Cargo.toml` (+ `sha2`, `ureq` behind the feature),
  `crates/ridl-core/src/workspace.rs` (remote materialization),
  `crates/ridlc/src/main.rs` (`--frozen`)
- Test: lock/cache unit tests; fetch tested against a local
  `std::net::TcpListener` HTTP stub (no network in tests)

**Interfaces:**

- Produces (consumed by tasks 19, 20):

```rust
pub struct Lockfile { pub entries: BTreeMap<String, LockEntry> }   // url -> entry
pub struct LockEntry { pub sha256: String }
pub fn read_lockfile(path: &Path) -> (Option<Lockfile>, Vec<Diagnostic>)
pub fn write_lockfile(path: &Path, lock: &Lockfile) -> std::io::Result<()>
pub struct Cache { pub root: PathBuf }                  // default ~/.ridl/cache
impl Cache { pub fn lookup(&self, url: &str, sha256: &str) -> Option<PathBuf>;
             pub fn store(&self, url: &str, bytes: &[u8]) -> std::io::Result<(String, PathBuf)>; }
pub enum Frozen { Yes, No }
/// Materialize every remote import: cache hit skips fetch; frozen mode
/// never fetches and fails on any missing/mismatching entry.
pub fn materialize_imports(ws: …, lock: …, cache: &Cache, frozen: Frozen)
    -> (Vec<(String, PathBuf)>, Lockfile, Vec<Diagnostic>)
```

- Diagnostics (decision 2): MANI-101 fetch failed, MANI-102 hash mismatch vs
  lockfile, MANI-103 `--frozen` with missing lockfile entry, MANI-104 `--frozen`
  with unfetched import. The lockfile lives at the workspace root (`ridl.lock`),
  regenerated on successful resolution per ADR-0002 §7. Fetched artifact format
  (ADR-0002 is silent; ADR-0007 decision 12): the URL names an **uncompressed
  tar archive** of one package directory, unpacked into the cache — provisional
  until the registry spec (E7.4).

**Steps:**

- [ ] Failing tests: hash pin round-trip; cache hit skips the stub server
      (request counter); mismatch → MANI-102; all four frozen behaviors;
      `resolve_package` sees a materialized remote package end to end
      (stub-served tar with one `.typl` file).
- [ ] Implement; wire `--frozen` into `ridlc` (flag exists, task 20 fixes the
      full CLI surface).
- [ ] Full local gate; commit.

**Done when:** frozen verifies strictly; cache hit skips fetch; both are proven
by the request-counting stub.

---

### Task 17: E1.12 — Rust + extern-C backend over IR v1

Branch `e1-12-backend` · commits `feat(backends): …` · model: opus.

**Read first:** typl reference Appendix D (language layer table), §5.7 (codegen
realization), §10.1; ADR-0004 §7; ADR-0007 decision 13.

**Files:**

- Modify: `backends/rust/src/lib.rs` (rewrite over v1),
  `backends/rust/Cargo.toml` (+ `minijinja`)
- Create: `backends/rust/templates/c_header.j2`
- Test: snapshot tests per construct + the rustc-compiles test + a C header
  snapshot

**Interfaces:**

- Consumes: IR v1 (task 12) with populated inits (task 15).
- Produces (consumed by task 20):

```rust
pub struct Generated { pub rust_source: String, pub c_header: String }
pub fn generate(pkg: &ridl_ir::v1::Package) -> Result<Generated, GenerateError>
```

- Mapping (language layer, Appendix D): named scalar → newtype
  `pub struct Speed(pub f64);` / `(pub i64)` / `(pub bool)` / `(pub String)` /
  `(pub Vec<u8>)` with `#[repr(transparent)]`; const →
  `pub const NAME: Type = …` (string consts → `&'static str` via a parallel type
  strategy: `String`-backed named types get a `pub const … : &str` — document
  the asymmetry in the generated header comment); struct → `#[repr(C)]` where
  every field is fixed-layout (`fixed_layout` flag), plain `pub struct`
  otherwise; optional → `Option<T>`; enum → `#[repr(i64)] pub enum`; enumset →
  newtype over `i64` with bit consts; union → `pub enum` with one variant per
  arm; tuple → generated nested struct named `ParentField` (CamelCase of path);
  array `[T; N]` fixed / `Vec<T>` bounded; map → `Vec<(K, V)>` (deterministic,
  no HashMap in contract types); init values → a `Default` impl per type from IR
  `InitValue`; `internal` → `pub(crate)`; deprecated →
  `#[deprecated(note = …)]`; docs → `///`.
- extern-C face: the minijinja template renders a C header with the scalar
  typedefs (`typedef double veh_common_speed;`), enum constants, and repr(C)
  struct declarations for `fixed_layout` structs only, with a header comment
  naming the source package and IR version. Non-C- representable shapes (bounded
  arrays, maps, unions, optionals, strings) are listed in a trailing comment
  block as `/* not representable in C ABI: … */` — honest, minimal.

**Steps:**

- [ ] Failing tests: per-construct snapshots (scalar+unit doc, const, struct
      with optional + reserved skip, enum, enumset both forms, result union,
      tuple field, fixed and bounded arrays, map); the generated Rust for
      Appendix B compiles via
      `rustc --edition 2024 --crate-type lib --emit metadata` in a tempdir (kept
      from E0, with the #102 temp-file leak fixed by `tempfile`); `Default`
      yields the §5.8 derived inits; C header snapshot for Appendix B.
- [ ] Implement with quote/prettyplease + the template.
- [ ] Full local gate; commit.

**Done when:** generated Rust for the full corpus compiles; the C header
renders; snapshots reviewed.

---

### Task 18: E1.18 — test spine: pipeline corpus + proptest generators

Branch `e1-18-test-spine` · commits `test(ridlc): …` / `feat(ridl-sem): …` ·
model: opus.

**Read first:** roadmap E1.18 row; ADR-0007 decision 3 (corpus layout); task
10's `scalar` interfaces.

**Files:**

- Create: `crates/ridlc/tests/corpus/` — packages exercising the full surface:
  `veh-common/` (Appendix B verbatim, as a real package with `ridl.toml`),
  `diag-showcase/` (a package whose compile emits one instance of every
  implemented TYPL/FORM/MANI diagnostic), `workspace-two-members/` (cross-member
  import); `crates/ridlc/tests/corpus.rs` (insta::glob runner snapshotting
  diagnostics render + IR JSON + generated Rust per corpus entry);
  `crates/ridl-sem/src/testgen.rs`
- Modify: workspace `Cargo.toml` (+ `proptest`)

**Interfaces:**

- Produces (the shipped feature seed for E2.11):

```rust
/// proptest strategies derived from a checked range — the "typl ranges
/// are generators" feature (ADR-0004 §9).
pub fn int_values(r: &IntRange) -> impl proptest::strategy::Strategy<Value = i64>
pub fn float_values(r: &FloatRange) -> impl Strategy<Value = f64>      // quantized: min + n·step
pub fn boundary_values(r: &IntRange) -> Vec<i64>                        // min, min+1, max-1, max
pub fn violations(r: &IntRange) -> Vec<i64>                             // min-1, max+1 (when representable)
```

**Steps:**

- [ ] Corpus packages + runner; snapshots reviewed (the diag-showcase snapshot
      is the de-facto diagnostic index until E4.2).
- [ ] Failing proptests: generated values always satisfy range+step;
      boundary/violation corpora hit the §4.2 width edges (e.g. 255/256 flips
      width in `derive_int_width`).
- [ ] Implement `testgen`.
- [ ] Full local gate; commit.

**Done when:** the corpus runs green under `insta::glob`; ranges generate
boundary/step-violation corpora (the E1.18 exit line).

---

### Task 19: E1.19 — wasm32 build check

Branch `e1-19-wasm-check` · commits `ci: …` / `chore(repo): …` · model: sonnet.

**Files:**

- Modify: `.github/workflows/ci.yml` (add `wasm` job:
  `rustup target add wasm32-unknown-unknown` +
  `cargo check --target wasm32-unknown-unknown -p ridl-syntax -p ridl-core -p ridl-sem -p ridl-ir --no-default-features`),
  `justfile` (recipe `wasm-check` running the same command), fix any feature
  leaks the check finds (fs/fetch code must sit behind the task 8 `fs` / task 16
  `fetch` features).

**Steps:**

- [ ] `rustup target add wasm32-unknown-unknown`; run the check; gate any
      offending code behind the features; recipe + CI job; local gate including
      `just wasm-check` green; commit.

**Done when:** the wasm check passes locally and the CI job exists for when CI
returns (E4.4 playground guard).

---

### Task 20: E1.13 — `ridlc` stable flags + `ridl` facade

Branch `e1-13-cli` · commits `feat(ridlc): …` / `feat(ridl): …` · model: opus.

**Read first:** concept note §8.1 (porcelain/plumbing); ADR-0006 decision 6;
roadmap E1.13; tasks 8, 16, 17 interfaces.

**Files:**

- Modify: `crates/ridlc/src/lib.rs` + `main.rs` (stable surface),
  `crates/ridl/src/main.rs` (real facade), `crates/ridl/Cargo.toml` (+ clap,
  path deps; stays `publish = false`)
- Test: CLI integration tests via `CARGO_BIN_EXE_*` for both binaries

**Interfaces:**

- `ridlc` (plumbing, stable): `ridlc check <PATH>` and
  `ridlc build <PATH> --out-dir <DIR> [--emit rust,c-header,ir-json]
  [--frozen]`
  where `<PATH>` is a `.typl` file (single-file mode), a package directory, or a
  workspace root; exit 0 clean / 1 diagnostics with ≥1 error / 2 I/O or usage
  error; diagnostics render to stderr via task 6; `--emit` defaults to `rust`;
  `ir-json` writes `<pkg-name>.ir.json`.
- `ridl` (porcelain): `ridl check [PATH]`, `ridl build [PATH]`,
  `ridl fmt [PATH]` (wired in task 21 — this task lands `check`/`build` and a
  `fmt` stub that exits 2 with "lands with tools/fmt" if task 21 has not merged
  first; whichever merges second deletes the stub), PATH defaulting to the
  current directory.
- Library face (consumed by task 22):
  `ridlc::compile_workspace(db, entry:
  &Path, frozen: Frozen) -> WorkspaceOutput { checked: Vec<CheckedPackage>,
  diagnostics: Vec<Diagnostic> }`.

**Steps:**

- [ ] Failing CLI tests: check/build on the corpus fixtures (file, package dir,
      workspace); exit codes; `--emit ir-json` writes exact-decimal JSON;
      `--frozen` failure without lockfile (MANI-103).
- [ ] Implement; keep `module_name_from_path` for single-file mode only.
- [ ] Full local gate; commit.

**Done when:** the plumbing/porcelain split is real — CI-grade flags on `ridlc`,
humane defaults on `ridl`.

---

### Task 21: E1.14 — `ridl fmt`

Branch `e1-14-fmt` · commits `feat(tools): …` (add scope `tools`) · model: opus.

**Read first:** general form §5 (tight colon, no alignment, the four "why"
points); ADR-0004 §10 (fmt ring); typl reference §15.2.

**Files:**

- Create: `tools/fmt/Cargo.toml` (crate `ridl-fmt`), `tools/fmt/src/lib.rs`
- Modify: root `Cargo.toml` (members + `tools/*`), `.git-std.toml` (scope
  `tools`), `crates/ridl/src/main.rs` (wire `ridl fmt` — or land the
  stub-deletion if task 20 merged first)
- Test: fmt corpus `tools/fmt/test_data/{input,formatted}/*.typl` + idempotence
  tests

**Interfaces:**

- Produces: `ridl_fmt::format(text: &str) -> FormatOutcome` where
  `FormatOutcome { Formatted(String), ParseErrors(Vec<Diagnostic>) }` — a file
  with parse errors is **never reformatted** (fmt must not eat broken code);
  `ridl fmt` walks `.typl` files, rewrites in place, `--check` mode exits 1 on
  any would-change file.
- Rules: tight `name: Type` everywhere (general form §5); one blank line between
  definitions, none at file start, one trailing newline; newline separators
  canonical inside braces (commas removed, trailing comma dropped);
  brace/bracket spacing per the reference examples (`[0.0..250.0 step 0.5]`,
  `{ … }` blocks with 2-space indent); comments and doc comments preserved
  verbatim in place; alignment never introduced. Formatting is CST-based
  (rowan), trivia-aware, and total: `format(format(x)) == format(x)`.

**Steps:**

- [ ] Failing corpus: the Appendix B example written in the old spaced/aligned
      style formats to the tight style; already-tight input is a fixed point
      (byte-identical); a file with a comment between fields keeps it; a broken
      file returns `ParseErrors`.
- [ ] Property test: idempotence over the whole parser `ok` corpus; formatted
      output re-parses to an identical CST shape (losslessness of meaning: same
      nodes, different trivia).
- [ ] Implement.
- [ ] Full local gate; commit.

**Done when:** idempotent; corpus reformats clean; wired as `ridl fmt`.

---

### Task 22: E1.15a — `ridl-lsp` server core + diagnostics

Branch `e1-15a-lsp-core` · commits `feat(ridl-lsp): …` (add scope) · model:
fable.

**Read first:** ADR-0004 §6 (lsp-server, sync loop, salsa cancellation); concept
note §8.1; tasks 8, 20 interfaces.

**Files:**

- Create: `crates/ridl-lsp/Cargo.toml` (binary),
  `crates/ridl-lsp/src/
  main.rs`, `crates/ridl-lsp/src/server.rs`,
  `crates/ridl-lsp/src/
  convert.rs` (Diagnostic ↔ lsp_types::Diagnostic,
  TextRange ↔ Range via a line-index)
- Modify: workspace `Cargo.toml` (+ `lsp-server`, `lsp-types`), `.git-std.toml`
  (scope `ridl-lsp`)
- Test: unit tests on `convert.rs`; an integration test driving the server over
  an in-memory `lsp_server::Connection` (initialize → didOpen a broken file →
  expect publishDiagnostics with the FORM code)

**Interfaces:**

- Produces (consumed by tasks 23–26): the server main loop — `initialize`
  advertising incremental sync + the capability set the later tasks fill;
  `didOpen`/`didChange`/`didClose` maintaining salsa `InputFile` overlays over
  the loaded workspace (fs snapshot at `initialize` via `load_workspace`,
  overlays override); `publishDiagnostics` per open file from
  `compile_workspace`, with `code` = DiagCode, severity mapped, related info
  from labels; fix-its surfaced as quick-fix code actions (the one capability
  beyond diagnostics this task lands, because it falls out of the fixit field).
- `convert::line_index(text) -> LineIndex` + offset↔position both ways (UTF-16
  code units per LSP).

**Steps:**

- [ ] Failing convert tests (UTF-16 positions on a multibyte line); failing
      integration test above.
- [ ] Implement the loop (lsp-server pattern: request dispatch + cancellation
      check between requests).
- [ ] Full local gate; commit.

**Done when:** the server compiles a real package on open and pushes coded
diagnostics with correct ranges; quick-fixes apply.

---

### Task 23: E1.15b — hover, goto-definition, find-references

Branch `e1-15b-lsp-nav` · commits `feat(ridl-lsp): …` · model: opus.

**Read first:** roadmap E1.15 row; ADR-0004 §10 (hover shows units/ranges); task
9's `Symbol` (declaration sites).

**Files:**

- Create: `crates/ridl-lsp/src/hover.rs`, `crates/ridl-lsp/src/nav.rs`
- Modify: `crates/ridl-lsp/src/server.rs` (dispatch + capabilities)
- Test: integration tests per feature over fixture packages

**Interfaces:**

- Hover on a type reference or declaration shows: qualified name, kind,
  backing + canonical UCUM unit, constraint (range/step/length/pattern), derived
  wire width (from IR), init value, doc comment markdown, labels, deprecation.
  Hover on a field shows its ordinal (§6.3 groundwork).
- Goto-def resolves through imports and qualified references to the `Symbol`
  declaration span; find-refs walks every file of every loaded package for
  resolved references to the same symbol (name-resolution based, not textual).

**Steps:**

- [ ] Failing tests: hover content for `Speed` (unit + range + width + doc);
      goto-def across packages via import; find-refs finds the const bound
      reference `MAX_SPEED` inside a constraint.
- [ ] Implement (a shared `symbol_at(db, ws, file, offset)` in `nav.rs` consumed
      by all three).
- [ ] Full local gate; commit.

**Done when:** all three work across package boundaries on a real workspace.

---

### Task 24: E1.15c — completion + rename

Branch `e1-15c-lsp-edit` · commits `feat(ridl-lsp): …` · model: opus.

**Files:**

- Create: `crates/ridl-lsp/src/complete.rs`, `crates/ridl-lsp/src/
  rename.rs`
- Modify: `crates/ridl-lsp/src/server.rs`
- Test: integration tests

**Interfaces:**

- Completion contexts: after `:` in a field/const/type position → visible named
  types (locals + ridl.std + imports, kind-annotated) + primitives; after
  `import` → known package names + their public symbols; inside a constraint
  after `[` → nothing (numbers) but after the `match` keyword → regex consts in
  scope; keyword completion at definition start (`type`, `const`, `struct`,
  `enum`, `enumset`, `union`, `internal`, `error`).
- Rename: on a declaration or reference — rewrites the declaration and every
  resolved reference across the workspace via `WorkspaceEdit`; rejects (LSP
  error) renaming into a reserved word, a case-convention violation (R7: type
  must stay CamelCase, const SCREAMING_SNAKE), or a name that collides in any
  affected scope.

**Steps:**

- [ ] Failing tests: each completion context; rename `Speed` across two packages
      updates the import line too; rename to `signal` rejected; rename
      introducing a duplicate rejected.
- [ ] Implement over `symbol_at` + `Resolution`.
- [ ] Full local gate; commit.

**Done when:** completion and rename behave on a multi-package workspace.

---

### Task 25: E1.16 — inlay hints: ordinals + unit expansion

Branch `e1-16-inlay` · commits `feat(ridl-lsp): …` · model: opus.

**Read first:** general form §6.3 (both mitigations); typl §7.4.

**Files:**

- Create: `crates/ridl-lsp/src/inlay.rs`
- Modify: `crates/ridl-lsp/src/server.rs`
- Test: integration test asserting hint positions + texts

**Interfaces:**

- Ordinal hints: every struct field, union arm, and enum/enumset value renders
  its derived ordinal (`#1`, `#2`, …) after the name, counting reserved
  tombstones so a reorder is visibly a renumbering.
- Unit expansion hints: a unit-typed declaration renders the unit's human
  reading after the UCUM code (`km/h ⟨kilometer per hour⟩`) from a small
  display-name table in `ucum.rs` (add
  `UcumExpr::display_name() -> Option<String>` for the curated atoms).

**Steps:**

- [ ] Failing test: the §7.4 tombstone struct renders `#1`/`#3` around the
      reserved slot; `type Speed : km/h …` gets the expansion hint.
- [ ] Implement + capability flag.
- [ ] Full local gate; commit.

**Done when:** ordinals render beside fields (the roadmap exit line) and unit
hints show.

---

### Task 26: E1.17 — VS Code extension

Branch `e1-17-vscode` · commits `feat(editors): …` (add scope `editors`) ·
model: sonnet.

**Files:**

- Create: `editors/vscode/package.json`,
  `editors/vscode/
  language-configuration.json`,
  `editors/vscode/syntaxes/
  typl.tmLanguage.json`,
  `editors/vscode/src/extension.ts`, `editors/vscode/tsconfig.json`,
  `editors/vscode/.vscodeignore`, `editors/vscode/README.md`
- Modify: `.git-std.toml` (scope `editors`), root `.gitignore`
  (`editors/vscode/node_modules`, `*.vsix`)

**Interfaces:**

- The extension: language id `typl` for `.typl` files; TextMate grammar covering
  keywords (from the task 1 registry — keep the list in a comment pointing at
  `keywords.rs` as SSOT), literals incl. durations + regex, comments/doc
  comments, attribute-free typl surface; LSP client (`vscode-languageclient`)
  launching `ridl-lsp` from `PATH` or the `ridl.serverPath` setting; activation
  on language `typl`.
- Build: `npm ci && npm run compile && npx @vscode/vsce package` produces a
  `.vsix` installable with `code --install-extension`. Marketplace publishing is
  **deferred** (maintainer act, like crates.io — debt roll-up).

**Steps:**

- [ ] Scaffold; grammar; client; build the vsix locally; document the install +
      `cargo install --path crates/ridl-lsp` flow in the extension README.
- [ ] Verify by launching VS Code with the extension against the corpus
      workspace (diagnostics + hover visible) — record the check in the PR
      description; `just check` for the Markdown; commit.

**Done when:** the vsix builds, installs, connects to `ridl-lsp`, and highlights
— the roadmap exit line.

---

## Close-out (after task 26)

- Whole-epic review on the most capable model; Critical/Important → one fix-wave
  PR; Minor → consolidated debt issue (also absorbing the deferred items
  recorded above: TYPL-107, 205, 401–403, marketplace publish, fetch artifact
  format).
- Close #102 (all items absorbed) with a comment mapping item → task.
- Gardening PR (sdd-gardening): archive this plan to `docs/archive/`; update the
  walking-skeleton technote (E1 replaced its seams — rewrite as the E1 as-built
  map or supersede it); sync AGENTS.md (crate map, commands), README,
  CONTRIBUTING, `.git-std.toml` comment; annotate the roadmap E1.8 row (`wire`
  floor deferred per typl §17.11 / ADR-0007); roadmap E1 rows checked off. The
  version stays `0.0.0`: cutting the v0.1 preview tag (`just release`) and any
  publishing are maintainer acts (ADR-0007 decision 14).

## Out of scope (deferred, recorded)

- `wire` clause (typl §17.11 — despite the stale roadmap row).
- Attribute blocks, `labels`/`deprecated` as attributes, and the single
  `attr_block` production (general form §4) — E2, with the family grammar work
  (ADR-0007 decision 11).
- TYPL-107, TYPL-205, TYPL-401/402/403 (decision 10 scope cuts — debt).
- crates.io publishing, marketplace publishing, release tagging — maintainer
  acts.
- Semantic tokens, `ridl doc`, `ridl diff`, lint — E2/E4 rings (ADR-0004 §10).
