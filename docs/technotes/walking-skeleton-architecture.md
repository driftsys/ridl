# The typl Toolchain Architecture, as Built

This note is informative — it binds nothing. For the decisions behind these
choices, see [ADR-0007](../decisions/ADR-0007-e1-execution.md),
[ADR-0004](../decisions/ADR-0004-implementation-sequencing-and-stack.md), and
[ADR-0002](../decisions/ADR-0002-module-system.md); for the requirements, see
[`docs/ROADMAP.md`](../ROADMAP.md) epic E1. This note exists for whoever picks
up Epic 2: it records the workspace map, the end-to-end pipeline, and the seams
a newcomer or the E2 implementer needs to know about — as they actually landed
in the merged code (PRs #107–#133), not as planned. Where it disagrees with an
ADR or the roadmap, the ADR/roadmap is normative and this note is stale.

An earlier version of this file described the epic E0 walking skeleton. E1
rebuilt every part that version named, so the note was rewritten as-built at E1
close; the E0 version remains in git history and the section
[What E1 closed from the E0 note](#what-e1-closed-from-the-e0-note) records what
happened to its open items.

## The workspace map

One Cargo workspace (`Cargo.toml`,
`members = ["crates/*", "backends/*", "tools/*", "xtask"]`): seven crates under
`crates/`, one backend, one tool, and the `xtask` automation member. The VS Code
extension (`editors/vscode`) is TypeScript and is not a workspace member.

- **`crates/ridl-syntax`** — the surface layer. A `logos` lexer over the full
  typl token set; a hand-written recursive-descent parser producing a lossless
  rowan CST with recovery on broken input; and a typed AST generated from the
  grammar description `typl.ungram` by `cargo xtask codegen` into
  `src/ast/generated.rs` — committed, with a drift test that regenerates and
  fails on divergence (ADR-0007 decision 1). `SyntaxError` carries a message, a
  stable code tag, and a `TextRange`; `ridl-core` maps it into the coded
  diagnostic model.
- **`crates/ridl-core`** — three roles. (1) The salsa incremental database:
  `InputFile` (a `#[salsa::input]` of path + text) and the memoized `parse_file`
  query; editing a file's text through `set_text` re-parses that file alone. (2)
  The coded diagnostic model (`diag`): a `Diagnostic` value with a stable
  `DiagCode`, severity, message, primary `Span`, secondary labels, and optional
  fix-its, accumulated in a `Vec` and rendered to the terminal over
  `codespan-reporting`; the `TYPL-`/`FORM-`/`RIDL-`/`MANI-` namespaces live here
  as the single source of truth (ADR-0007 decision 2), one catalogue each,
  declared through the `diag_codes!` macro so a code and its catalogue entry
  come out of the same line (ADR-0008 decision 21). For the codes themselves,
  the `FORM-` and `MANI-` tables are in the family overview §7, the `TYPL-`
  tables in typl §16, and the `RIDL-` tables in ridl §16 — this note says where
  the namespaces live, not what is in them. (3) The "ns" core: `ridl.toml`
  manifest parsing (standalone + workspace modes), the package model with
  per-package `[imports]` (ADR-0007 decision 17), workspace loading, the
  lockfile, the content-addressed cache, and `ureq` fetch of uncompressed tar
  artifacts (ADR-0007 decision 12). `ridl.std` is embedded via `include_str!`
  (decision 15). Filesystem discovery sits behind the `fs` feature and network
  fetch behind `fetch` (which pulls `ureq`, `sha2`, `tar` and implies `fs`), so
  the compiler crates build for `wasm32-unknown-unknown` with
  `--no-default-features` (decision 5).
- **`crates/ridl-sem`** — the semantic passes, carved out of `ridl-core` at the
  start of E1 (ADR-0007 decision 4). `resolve` implements the ADR-0002 §5
  reference order (workspace member → the package's own `[imports]` → the
  workspace `[imports]` → error) plus cycle detection, and applies the
  first-wins duplicate tiebreak — every later duplicate is TYPL-009 (decision
  6). `check` lowers each resolved package to IR v1, lowering only the
  resolver's first-wins winner. Supporting modules: `scalar` (exact
  `BigRational` arithmetic for ranges, steps, and wire-width derivation — no
  `f32`/`f64` anywhere), `ucum` (the UCUM term grammar over a curated atom
  table, decision 8), `init` (init-value derivation), `docs` (doc-comment
  handling), and `testgen` (proptest strategies and boundary/violation corpora
  derived from checked ranges, behind the `testgen` feature).
- **`crates/ridl-ir`** — IR v1 (`proto/ridl/ir/v1/ir.proto`), compiled by
  `build.rs` with `protox` + `prost-build` (no system `protoc`). Range bounds,
  steps, and init values are canonical decimal strings — the schema has no float
  or double field — and derived wire widths ride alongside as enums (ADR-0007
  decision 9). The prost types carry `serde` for the exact-decimal JSON debug
  rendering.
- **`backends/rust`** (crate `ridl-backend-rust`) — one IR v1 package to
  `Generated { rust_source, c_header }`. Rust is built as a `quote` token stream
  and formatted with `prettyplease`: named scalar types become
  `#[repr(transparent)]` newtypes, and structs whose IR `fixed_layout` flag
  holds become `#[repr(C)]`. The C header is rendered from a `minijinja`
  template; shapes with no fixed C ABI are listed in a trailing header comment
  rather than mis-mapped (ADR-0007 decision 13). Codegen is total — failures
  return `GenerateError`, never panic.
- **`crates/ridlc`** — the pipeline as a library plus the plumbing binary.
  `compile` runs a single file end to end; `compile_workspace` runs the loaded
  package model (file, package directory, or workspace root) and is the library
  face the language server drives; `run_check` and `run_build` add the
  remote-import lockfile round trip (`--frozen` never fetches) and, for build,
  write the selected artifacts: `<base>.rs`, `<base>.h`, and `<base>.ir.json`.
  The binary exposes `ridlc check` and `ridlc build`.
- **`crates/ridl`** — the porcelain facade: `ridl check`, `ridl build`, and
  `ridl fmt`, driving the same `ridlc` command drivers and the `tools/fmt`
  engine (the plumbing/porcelain split of concept note §8.1).
- **`tools/fmt`** — the `ridl fmt` engine: CST-based and trivia-aware (comments
  are preserved and re-anchored), total (input with parse errors is returned
  untouched), and idempotent, implementing the tight `name: Type` style of
  general form §5.
- **`crates/ridl-lsp`** — the language server; see the next section.
- **`editors/vscode`** — the VS Code extension: an LSP client plus a TextMate
  grammar, built with npm/tsc.
- **`xtask`** — `cargo xtask codegen`, the typed-AST generator over
  `typl.ungram`.

## The end-to-end pipeline contract

```text
single .typl file | package directory | workspace root
  -> load      Workspace { packages, imports }      ridl-core   manifest + package<->directory law
  -> parse     Parse { GreenNode, errors }          ridl-syntax salsa-memoized per InputFile
  -> resolve   Resolution { symbols, diagnostics }  ridl-sem    ADR-0002 §5 order
  -> check     ir v1 Package + diagnostics          ridl-sem -> ridl-ir types
  -> generate  Rust source + C header               backends/rust
```

Every stage emits coded `Diagnostic` values, concatenated in pipeline order
(load, parse, resolve, check, backend). The package-scoped passes stamp spans
with a `FileId` local to the package's file list; `remap_diagnostics` rewrites
them onto the caller's `SourceMap` before rendering. One diagnostic value maps
two ways: to the terminal via `codespan-reporting`, and to
`lsp_types::Diagnostic` via the LSP's `convert` module.

## The LSP overlay design

`ridl-lsp` is built on `lsp-server` — a synchronous loop, no async runtime.
Dispatch is strictly sequential; a `$/cancelRequest` is honored by a
cancelled-set check before each dispatch, and salsa's own cancellation applies
once queries run off-thread.

`ridlc::compile_workspace` is a cold, from-disk compile, so the server does not
drive it per keystroke. Instead it loads the workspace once at `initialize`,
holds the `Workspace` handle plus a map of file path → `InputFile`, and on
`didOpen`/`didChange` calls `set_text` on the existing salsa input — the editor
buffer overlays the disk state. A file opened from outside the loaded workspace
becomes its own overlay input in a synthetic single-file package. Every
recompute then goes through the memoized `parse_file` / `resolve_package` /
`check_package` queries, so editing one file re-checks only the package that
file belongs to.

`convert.rs` is the single bridge between compiler coordinates (byte-offset
`TextRange` into UTF-8 text) and LSP coordinates (UTF-16 line/character
positions), via a per-file `LineIndex`. Features shipped: published diagnostics,
quick-fix code actions derived from diagnostic fix-its, hover (units and
ranges), goto-definition, find-references, completion, rename with prepare
support, and inlay hints (field ordinals and unit expansion).

## What E1 closed from the E0 note

The E0 version of this note named cross-seam facts for E1 to handle
deliberately. All are closed:

1. **The `ridl-sem` split is done.** The resolver and checker moved out of
   `ridl-core` into `crates/ridl-sem` (ADR-0007 decision 4).
2. **The `SourceFile` name collision is resolved by rename.** The salsa input is
   `InputFile`; the only public `SourceFile` is the AST root
   `ridl_syntax::ast::SourceFile` (ADR-0007 decision 4).
3. **The coded `Diagnostic` model replaced the plain error structs.** The E0
   `ResolveError`/`CheckError` types and the string-flattened offsets are gone;
   every diagnostic carries its code and span end to end (ADR-0007 decision 2).
4. **The duplicate-declaration tiebreak is first-wins everywhere.** The resolver
   flags every later duplicate with TYPL-009 and the checker lowers only the
   surviving first declaration (ADR-0007 decision 6).

## References

- [ADR-0007](../decisions/ADR-0007-e1-execution.md) — the E1 execution decisions
  this note assumes.
- [ADR-0004](../decisions/ADR-0004-implementation-sequencing-and-stack.md) — the
  stack these choices refine.
- [ADR-0002](../decisions/ADR-0002-module-system.md) — the module semantics the
  resolver implements.
- [`docs/ROADMAP.md`](../ROADMAP.md), Epic 1 — the stories this architecture
  satisfies.
- `docs/archive/2026-07-18-e1-typl-tooling-spine-plan.md` — the archived
  execution plan.
