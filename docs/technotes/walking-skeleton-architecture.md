# The Walking-Skeleton Architecture, as Built

This note is informative — it binds nothing. For the decisions behind these
choices, see [ADR-0006](../decisions/ADR-0006-walking-skeleton-execution.md) and
[ADR-0004](../decisions/ADR-0004-implementation-sequencing-and-stack.md); for
the requirements, see [`docs/ROADMAP.md`](../ROADMAP.md) epic E0. This note
exists for whoever picks up Epic 1: it records the E0 crate map, the end-to-end
pipeline, and the seams a newcomer or the E1 implementer needs to know about —
as they actually landed in the merged code, not as planned. Where it disagrees
with an ADR or the roadmap, the ADR/roadmap is normative and this note is stale.

## The crate map

One Cargo workspace, six crates (`Cargo.toml`,
`members = ["crates/*",
"backends/*"]`):

- **`crates/ridl-syntax`** — logos lexer, hand-written recursive-descent parser,
  lossless rowan CST, thin hand-written AST accessors. Public surface:
  `lex(&str) -> Vec<Token>`; `parse(&str) -> Parse`
  (`.syntax() ->
  SyntaxNode`, `.errors() -> &[SyntaxError]`,
  `SyntaxError { message,
  offset }`); `SourceFile` (`type_decls()`,
  `const_decls()`), `TypeDecl` (`name()`, `unit()`, `range()`), `ConstDecl`
  (`name()`, `type_name()`, `value()`) — AST types re-exported at the crate root
  from a private `ast` module, so the public paths carry no `ast::` prefix —
  `RangeSpec { min, max, step }`; `SyntaxKind`, `SyntaxNode`, `SyntaxToken`,
  `RidlLanguage`.
- **`crates/ridl-core`** — the salsa incremental database, the resolver, and the
  checker. All three deliberately live here until a `ridl-sem` crate exists
  (ADR-0006 decision 2). Public surface: `db::SourceFile` (a `#[salsa::input]` —
  distinct from `ridl_syntax::SourceFile`, see cross-seam fact 1 below),
  `RidlDatabase`, `parse_file(&db, file) -> Parse`;
  `resolve::{Resolution, ResolveError, SymbolKind, resolve}`;
  `check::{CheckError, check}`.
- **`crates/ridl-ir`** — the IR v0 protobuf schema
  (`proto/ridl/ir/v0/ir.proto`), compiled by `build.rs` with `protox` +
  `prost-build` (no system `protoc` — ADR-0006 decision 3). Public surface:
  `Module { name, types, consts }`, `TypeDef { name, unit, range }`,
  `Range { min, max, step }`, `ConstDef { name, type_name, value }` — prost
  types with `serde::{Serialize, Deserialize}` added via `type_attribute`.
- **`backends/rust`** (crate `ridl-backend-rust`) — IR to Rust source text.
  Public surface: `generate(&Module) -> Result<String, GenerateError>`, built as
  a `quote` `TokenStream` and formatted with `prettyplease`.
- **`crates/ridlc`** — the compiler CLI and its library face. Public surface:
  `ridlc::compile(path: &str, text: &str) -> CompileOutput {
  rust_source, module, diagnostics: Vec<String> }`;
  the binary's `build` subcommand (`ridlc build <INPUT.typl> --out-dir <DIR>`,
  clap-based), exit 0 on no diagnostics, 1 on diagnostics, 2 on an I/O error.
- **`crates/ridl`** — a stub binary only (`publish = false`); porcelain
  subcommands are E1 scope (ADR-0006 decision 6).

## The end-to-end pipeline contract

```text
walking_skeleton.typl
  -> lex        Vec<Token>                          ridl-syntax
  -> parse      Parse { GreenNode, errors }         ridl-syntax, salsa-memoized in ridl-core
  -> resolve    Resolution { symbols, diagnostics } ridl-core
  -> check      ir::Module + Vec<CheckError>        ridl-core -> ridl-ir types
  -> generate   String (Rust source)                backends/rust
  -> ridlc build fixture.typl --out-dir out/        ridlc, insta golden test
```

`ridlc::compile` wires these five stages in order and concatenates diagnostics
in that order: parser messages, then resolver, then checker. The golden test
(`crates/ridlc/tests/golden.rs`) pins the generated Rust and the lowered IR
against committed `insta` snapshots — the epic's exit criterion — and, as of
this gardening pass, is green on `main`.

## Three cross-seam facts E1 must handle deliberately

1. **Two types named `SourceFile`.** `ridl_syntax::SourceFile` (the AST root)
   and `ridl_core::db::SourceFile` (the salsa input) share a name across the
   crate boundary; `crates/ridlc/src/lib.rs` already aliases them on import
   (`SourceFile as InputFile`, `SourceFile as AstFile`). When the semantic
   passes move to a future `ridl-sem` crate (ADR-0006 decision 2), resolve this
   collision deliberately — rename one of them, or keep the alias convention and
   document it as the crate boundary's contract.

2. **The resolver and the checker disagree on the duplicate-declaration
   tiebreak.** `ridl_core::resolve::resolve` declares types before consts
   regardless of source order (see the module comment in `resolve.rs`) and
   reports a `duplicate declaration` diagnostic for the losing name, keeping
   only the winner in `Resolution::symbols`. `ridl_core::check::check` consults
   `Resolution::symbols` only to skip consts whose type reference did not
   resolve; it lowers every `type_decl` (and every const with a resolvable type)
   directly from the AST — so a duplicate declaration still produces two
   same-named entries in the emitted IR `Module`, alongside the resolver's
   diagnostic. The two passes need to agree on one canonical declaration once
   the real, source-position-order resolver lands in E1.

3. **Parser error offsets are flattened at the `compile()` seam.**
   `ridl_syntax::SyntaxError` carries a byte `offset` alongside its `message`,
   but `ridlc::compile` immediately maps every diagnostic (parser, resolver, and
   checker alike) to a bare `String`, discarding the offset. Reclaim it when the
   coded `Diagnostic` model (ADR-0004 §5, ADR-0006 decision 7) replaces these
   per-crate error structs — a span is exactly the kind of thing that model
   exists to carry.

## Closed by the E0 fix wave

Two findings from the whole-epic review were closed by the E0 fix wave (pull
request 104):

- `backends/rust/src/lib.rs`'s `generate` used to panic (`.expect(...)`) if the
  emitted tokens failed to parse as a well-formed Rust file; it now validates
  emitted names up front and returns a `GenerateError`, which `ridlc::compile`
  folds into its diagnostics.
- the input file stem used to be derived independently in
  `crates/ridlc/src/lib.rs` and `crates/ridlc/src/main.rs`; the public
  `module_name_from_path` helper is now the single source.

A consolidated set of remaining minor findings from the review is tracked as
issue #102.

## References

- [ADR-0006](../decisions/ADR-0006-walking-skeleton-execution.md) — the E0
  execution decisions this note assumes.
- [ADR-0004](../decisions/ADR-0004-implementation-sequencing-and-stack.md) — the
  stack these choices refine.
- [`docs/ROADMAP.md`](../ROADMAP.md), Epic 0 — the requirements this
  architecture satisfies.
