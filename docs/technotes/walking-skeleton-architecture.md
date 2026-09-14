# The RIDL Toolchain Architecture, as Built

This note is informative — it binds nothing. For the decisions behind these
choices, see [ADR-0008](../decisions/ADR-0008-e2-execution.md),
[ADR-0007](../decisions/ADR-0007-e1-execution.md),
[ADR-0004](../decisions/ADR-0004-implementation-sequencing-and-stack.md), and
[ADR-0002](../decisions/ADR-0002-module-system.md); for the requirements, see
[the landed record](../archive/roadmap-landed-record.md), epics E1 and E2. This
note exists for whoever picks up the next epic: it records the workspace map,
the end-to-end pipeline, and the seams a newcomer or the E3 implementer needs to
know about — as they actually landed in the merged code (E1 as PRs #107–#133, E2
as PRs #136–#181), not as planned. Where it disagrees with an ADR or the
roadmap, the ADR/roadmap is normative and this note is stale.

The file has been rewritten as-built at each epic close. The first version
described the epic E0 walking skeleton, which the filename still carries; E1
rebuilt every part that version named, and E2 added the interaction layer over
it. The earlier versions remain in git history, and the section
[What E1 closed from the E0 note](#what-e1-closed-from-the-e0-note) records what
happened to the E0 open items.

## The workspace map

One Cargo workspace (`Cargo.toml`, `members = ["crates/*", "xtask"]`): every
crate under `crates/` with its directory named after it, and the `xtask`
automation member at the root (issue #180). The VS Code extension
(`editors/vscode`) is TypeScript and is not a workspace member.

The crates below arrived in three waves: seven from the E1 spine, grown in place
through E2; two more from E2 — `ridl-backend-ts` and `ridl-diff`; and
`ridl-mcp`, most recently. `ridl-rt` landed after those three waves, from epic
E11's first story rather than from E1 or E2, and is listed with the others
because a newcomer will look for it here. This list is not a standing count of
every crate the workspace holds — see `AGENTS.md` for that.

- **`crates/ridl-syntax`** — the surface layer, and the one grammar. A `logos`
  lexer over the full family token set; a hand-written recursive-descent parser
  producing a lossless rowan CST with recovery on broken input; and a typed AST
  generated from the grammar description `family.ungram` by
  `cargo xtask codegen` into `src/ast/generated.rs` — committed, with a drift
  test that regenerates and fails on divergence (ADR-0007 decision 1).
  `SyntaxError` carries a message, a stable code tag, and a `TextRange`;
  `ridl-core` maps it into the coded diagnostic model.

  E2 made lexing and parsing take a `Profile` (`keywords.rs`): `Profile::Typl`
  behaves exactly as the E1 toolchain did, and `Profile::Ridl` additionally
  activates nine keywords — `interface`, `service`, `signal`, `event`,
  `command`, `query`, `fixed`, `require`, `ensure`. The grammar grew the
  interaction productions, the single general-form attribute block, and the
  guaranteed-subset expression grammar, all in the same `family.ungram` (renamed
  from `typl.ungram`) and all reaching the typed AST through the same generator.

  Two invariants hold for **both** profiles, and are what the next profile
  inherits: `parse(text, profile).syntax().text() == text` for every input,
  valid or broken; and the parser deliberately accepts more than the reference
  allows, leaving the narrowing to the checker.

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
  (ADR-0007 decision 15). Filesystem discovery sits behind the `fs` feature and
  network fetch behind `fetch` (which pulls `ureq`, `sha2`, `tar` and implies
  `fs`), so the compiler crates build for `wasm32-unknown-unknown` with
  `--no-default-features` (ADR-0007 decision 5).

- **`crates/ridl-sem`** — the semantic passes, carved out of `ridl-core` at the
  start of E1 (ADR-0007 decision 4). `resolve` implements the ADR-0002 §5
  reference order (workspace member → the package's own `[imports]` → the
  workspace `[imports]` → error) plus cycle detection, and applies the
  first-wins duplicate tiebreak — every later duplicate is TYPL-009 (ADR-0007
  decision 6). `check` lowers each resolved package to IR v2, lowering only the
  resolver's first-wins winner. Supporting modules: `scalar` (exact
  `BigRational` arithmetic for ranges, steps, and wire-width derivation — no
  `f32`/`f64` anywhere), `ucum` (the UCUM term grammar over a curated atom
  table, ADR-0007 decision 8), `init` (init-value derivation), `docs`
  (doc-comment handling), and `testgen` (proptest strategies and
  boundary/violation corpora derived from checked ranges, behind the `testgen`
  feature).

  E2 added four modules here and grew `check` and `resolve` to the interaction
  surface. `timing` resolves every signal and event to concrete microsecond
  bounds plus a mode and a default-applied flag, so "untimed" does not exist
  beyond the parser (ADR-0008 decision 12); its duration atoms are `us`, `ms`,
  `s`, `min`, `h` (ADR-0008 decision 16). `expr` type-checks a
  `require`/`ensure` clause against the guaranteed subset and rejects anything
  outside it with RIDL-306. `expr_eval` evaluates the same subset totally and
  exactly over arbitrary-precision rationals, with division by zero as the
  single defined fault — it is the engine behind `ridl test` today and the E5
  reference oracle later. `lint` is four advisory codes (RIDL-404, RIDL-405,
  RIDL-406, RIDL-308) run from `check_package` once lowering has finished, on
  the ordinary diagnostic channel, with no lint driver and no configuration
  surface.

- **`crates/ridl-ir`** — the IR schema, compiled by `build.rs` with `protox` +
  `prost-build` (no system `protoc`). `proto/ridl/ir/` holds **`v2` alone**: E2
  landed the v2 lowering and removed the v1 schema with it (ADR-0008 decision
  8), so the pre-cut field reservations v1 carried are visible only in v2's
  numbering. Range bounds, steps, init values, and timing bounds are canonical
  decimal strings — the schema has no float or double field — and derived wire
  widths ride alongside as enums (ADR-0007 decision 9). The crate renders three
  encodings of that schema — canonical protobuf JSON, prototext, and protobuf
  binary (ADR-0014). JSON goes through `pbjson`-generated serde impls; prototext
  goes through `prost-reflect` over a descriptor pool that `build.rs` writes to
  `OUT_DIR` and `lib.rs` embeds, from the same compilation that generates the
  types; binary needs neither. JSON is the form `ridl baseline` writes and
  `ridl diff` reads.

- **`crates/ridl-backend-rust`** — one IR v2 package to
  `Generated { rust_source }`. Rust is built as a `quote` token stream and
  formatted with `prettyplease`: named scalar types become
  `#[repr(transparent)]` newtypes, and structs whose IR `fixed_layout` flag
  holds become `#[repr(C)]`. Codegen is total — failures return `GenerateError`,
  never panic. E2's `interact.rs` and the extern-C header are gone:
  [ADR-0018](../decisions/ADR-0018-runtime-core-and-generated-surface.md)
  decisions 15 and 6 retract the interaction layer, restoring it in a second
  phase as a client and a server over `ridl-rt`, and drop C as a target.

- **`crates/ridl-backend-ts`** — the second backend, and the reason IR
  neutrality is a demonstrated property rather than a claim (ADR-0008 decision
  7). One IR v2 package to one TypeScript module: named scalars become branded
  types so nominal identity survives structural typing, U64/I64 widths brand
  `bigint` because a value past 2^53 has no exact `number`, structs become
  `export interface`s, unions become discriminated unions, and `internal`
  declarations become non-exported members. It depends on `ridl-ir` and nothing
  else.

- **`crates/ridl-diff`** — the `ridl diff` engine. It compares two resolved IR
  v2 snapshots and reads **only** the IR, never source, so the comparison is
  honest against exactly what a backend sees. Two halves: `walk` says what
  structurally differs, emitting one `Change` per difference with a `Category`;
  `classify` says which direction it moved and settles the `Verdict`, so one
  structural category can carry opposite verdicts depending on direction
  (ADR-0008 decisions 9 and 14). The exit contract is 0 compatible, 1 breaking,
  2 error. Three matches over `Category` deny `clippy::wildcard_enum_match_arm`
  and `clippy::match_wildcard_for_single_variants`, which is why `just build`
  runs `just lint`: a new variant swept into a wildcard arm compiles and passes
  the whole test suite.

- **`crates/ridlc`** — the pipeline as a library plus the plumbing binary.
  `compile` runs a single file end to end; `compile_workspace` runs the loaded
  package model (file, package directory, or workspace root) and is the library
  face the language server drives; `run_check` and `run_build` add the
  remote-import lockfile round trip (`--frozen` never fetches) and, for build,
  write the selected artifacts: `<base>.rs`, `<base>.h`, `<base>.ir.json`,
  `<base>.ir.txtpb`, `<base>.ir.binpb`, and `<base>.ts`. The binary exposes
  `ridlc check` and `ridlc build`. E2 widened `WorkspaceOutput` with the
  checker's `resolutions` and the lowered `std_ir`, so a workflow crate does not
  have to restate the compiler's load-and-resolve loop (ADR-0008 decision 15).
  Both backends are ordinary dependencies here: `ridlc build --emit` offers
  `rust`, the three IR encodings of ADR-0014 decision 4 (`ir-json`, `ir-text`,
  `ir-binary`), `typescript` and `proto`, so the second backend over one IR is
  reachable from the command line and not only from the corpus runner's
  snapshots. The two language emits are independent — each backend generates
  from the same IR on its own, and one that cannot render a package skips only
  its own artifact.

- **`crates/ridl`** — the porcelain facade: `ridl check`, `ridl baseline`,
  `ridl build`, `ridl test`, `ridl fmt`, `ridl diff`, `ridl lsp`, and
  `ridl mcp`, driving the `ridlc` command drivers, the `ridl-fmt` engine, the
  `ridl-diff` engine, and the `ridl-lsp`/`ridl-mcp` libraries (the
  plumbing/porcelain split of concept note §8.1). Everything E2 added to the CLI
  landed here rather than in `ridlc`, because `ridlc` stays a pure source→IR
  function — the minimal ISO 26262 tool-qualification boundary (ADR-0008
  decision 9). `ridl baseline` publishes one `<pkg-name>.ir.json` per package
  into `.ridl/baseline/`; `ridl check --baseline` is the desk-time ordinal-drift
  check over it; `property.rs` is the `ridl test` runner, which spends the E1.18
  range strategies on the contract plane.

- **`crates/ridl-fmt`** — the `ridl fmt` engine: CST-based and trivia-aware
  (comments are preserved and re-anchored), total (input with parse errors is
  returned untouched), and idempotent, implementing the tight `name: Type` style
  of general form §5. E2 extended it to `.ridl` files.

- **`crates/ridl-rt`** — the `no_std` runtime library that defines what a
  generated ridl package will link and a runtime will implement: identity, time
  and the envelope, samples, the payload traits, the interaction descriptors,
  the ports, and the contract and transport errors (epic E11 story E11.0,
  ADR-0020 decision 5). No backend emits code against it yet and no runtime
  exists. It has no dependency in any feature combination and links no runtime:
  the store and the sans-IO session are `ridl-engine`'s, parked outside this
  repository, and the three payload codecs (FlatBuffers, proto3, `repr(C)`) are
  later Epic 11 stories. See [the design record](../design/ridl-rt.md) and
  [ADR-0021](../decisions/ADR-0021-ridl-rt-0.1-api-and-release.md).

- **`crates/ridl-lsp`** — the language server; see the LSP section below.

- **`crates/ridl-mcp`** — the MCP server behind `ridl mcp` (ADR-0005 Layer B):
  one tool, `ridl_check`, over the same single-file front end
  (`ridlc::check_source`) that returns the JSON diagnostic contract
  `ridl check --format json` also produces. It is the workspace's first async
  code, built on `tokio` — see [below](#the-lsp-overlay-design) for what that
  does and does not reach.

- **`editors/vscode`** — the VS Code extension: an LSP client plus TextMate
  grammars for both `.typl` and `.ridl`, built with npm/tsc.

- **`xtask`** — `cargo xtask codegen`, the typed-AST generator over
  `family.ungram`.

## The end-to-end pipeline contract

```text
single .typl/.ridl file | package directory | workspace root
  -> load      Workspace { packages, imports }      ridl-core   manifest + package<->directory law
  -> parse     Parse { GreenNode, errors }          ridl-syntax salsa-memoized per InputFile, per Profile
  -> resolve   Resolution { symbols, diagnostics }  ridl-sem    ADR-0002 §5 order
  -> check     ir v2 Package + diagnostics          ridl-sem -> ridl-ir types   (lint runs at the end of check)
  -> generate  Rust source + C header               ridl-backend-rust
               TypeScript module                    ridl-backend-ts
```

Every stage emits coded `Diagnostic` values, concatenated in pipeline order
(load, parse, resolve, check, backend). The package-scoped passes stamp spans
with a `FileId` local to the package's file list; `remap_diagnostics` rewrites
them onto the caller's `SourceMap` before rendering. One diagnostic value maps
two ways: to the terminal via `codespan-reporting`, and to
`lsp_types::Diagnostic` via the LSP's `convert` module.

Two consumers hang off the IR rather than off the pipeline: `ridl-diff` reads
two `.ir.json` snapshots, and `ridl test` reads the checker's resolution plus
the lowered IR. Neither re-parses anything.

## The LSP overlay design

`ridl-lsp` is built on `lsp-server` — a synchronous loop, with no async runtime
of its own. Dispatch is strictly sequential; a `$/cancelRequest` is honored by a
cancelled-set check before each dispatch, and salsa's own cancellation applies
once queries run off-thread.

The workspace's first async runtime arrived later, with `crates/ridl-mcp` (built
on `tokio`, behind `ridl mcp` — see the workspace map above). It does not reach
`ridl-lsp` or any crate but `ridl` itself; `ridl` links `tokio` only to build
the one runtime `ridl mcp` blocks on, and `wasm-check` does not build the `ridl`
crate at all, so `tokio` never reaches the wasm32 target either.

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
support, and inlay hints (field ordinals and unit expansion). E2 taught all of
them `.ridl` — interaction hovers, resolved timing, and interaction ordinals in
the inlay hints.

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

- [ADR-0008](../decisions/ADR-0008-e2-execution.md) — the E2 execution decisions
  this note assumes.
- [ADR-0007](../decisions/ADR-0007-e1-execution.md) — the E1 execution decisions
  this note assumes.
- [ADR-0004](../decisions/ADR-0004-implementation-sequencing-and-stack.md) — the
  stack these choices refine.
- [ADR-0002](../decisions/ADR-0002-module-system.md) — the module semantics the
  resolver implements.
- [`docs/archive/roadmap-landed-record.md`](../archive/roadmap-landed-record.md),
  Epics 1 and 2 — the stories this architecture satisfies.
- `docs/archive/2026-07-18-e1-typl-tooling-spine-plan.md` and
  `docs/archive/2026-07-19-e2-ridl-interface-layer-plan.md` — the archived
  execution plans.
