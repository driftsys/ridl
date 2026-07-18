# ADR-0007: Epic E1 (typl + Tooling Spine) Execution Decisions

## Status

Accepted — 2026-07-18. Scope: epic E1 only (docs/ROADMAP.md). Decisions here
refine ADR-0002 and ADR-0004 for the typl implementation; none of them changes a
prior ADR choice. Recorded so the delegating maintainer can review them after
the fact, following the ADR-0006 pattern.

## Context

Epic E1 turns the E0 walking skeleton into the typl compiler and tooling spine:
full lexer and parser, generated typed AST, module system, manifest and
distribution, the type system with exact ranges and units, coded diagnostics, IR
v1, the Rust + extern-C backend, `ridl fmt`, the LSP, and a VS Code extension.
ADR-0004 fixes the stack and ADR-0002 the module semantics, but a set of
execution-level choices was still open. The execution plan lives at
`docs/wip/2026-07-18-e1-typl-tooling-spine-plan.md` (archived to docs/archive
when the epic closes) and cites these decisions by number.

## Decision

1. **Typed-AST generation: the `ungrammar` crate plus an in-repo generator.**
   ADR-0004 left "adopt rust-analyzer's ungrammar toolchain wholesale, or
   hand-roll a lighter generator" open. E1 adopts the `ungrammar` crate for the
   grammar description (`crates/ridl-syntax/typl.ungram`) and hand-rolls a small
   generator in a new `xtask` workspace member (`cargo xtask codegen`). The
   generated accessor file is committed, and a drift test regenerates it and
   fails on divergence. Rationale: the `ungrammar` format is proven and tiny;
   the rust-analyzer codegen around it is entangled with rust-analyzer
   internals, so the generator itself is easier to own than to vendor.

2. **Diagnostic namespaces beyond the specs.** The typl reference fixes `TYPL-…`
   for typl semantic rules but codes nothing for the shared surface grammar or
   the manifest layer. E1 allocates two namespaces: `FORM-…` for family-grammar
   diagnostics (lexical 0xx, parse 1xx — named after the general form's own
   "shared form namespace" phrasing), and `MANI-…` for manifest, lockfile,
   cache, and fetch diagnostics (manifest 0xx, distribution 1xx). Same
   discipline as TYPL codes: grouped by hundreds, never renumbered, never
   reused. The `Diagnostic` model itself lives in `ridl_core::diag` — no new
   crate, keeping the concept note §8.1 crate map intact; `ridl-syntax` keeps a
   plain span-carrying `SyntaxError` (now with a code tag) that `ridl-core` maps
   into the model.

3. **Test layout, and `insta` settled.** Syntax corpora live at
   `crates/ridl-syntax/test_data/{lexer,parser/ok,parser/err}/*.typl`, driven by
   `insta::glob!` snapshot tests; full-pipeline corpora are real packages under
   `crates/ridlc/tests/corpus/` snapshotting rendered diagnostics, IR JSON, and
   generated code. The ADR-0004 open question "insta vs expect-test" is settled
   as `insta` — E0 already committed to it and the corpus style benefits from
   external snapshot files.

4. **`ridl-sem` is carved at the start of E1.** The resolver and checker move
   verbatim from `ridl-core` to the new `crates/ridl-sem` (concept note §8.1)
   before the semantic stories begin, and the salsa input is renamed
   `SourceFile` → `InputFile`, resolving the E0 cross-seam name collision with
   `ridl_syntax::ast::SourceFile` by rename, not by alias convention.

5. **The ns core lives in `ridl-core` behind capability features.** Manifest,
   package model, lockfile, cache, and fetch are modules of `ridl-core` (the
   concept note's "ns" core), not a new crate. Filesystem discovery sits behind
   an `fs` feature and network fetch behind a `fetch` feature (both default-on,
   `fetch` implies `fs`), so the E1.19 `wasm32-unknown-unknown` check builds the
   compiler crates with `--no-default-features`.

6. **Duplicate-declaration tiebreak: first wins.** The E0 resolver kept the
   first declaration while the E0 checker lowered the last. The real resolver
   declares in source-position order, reports `TYPL-009` on every later
   duplicate, and the checker lowers only the surviving first declaration — one
   canonical winner everywhere.

7. **The typl reference outranks the roadmap row; the `wire` floor stays
   deferred.** Roadmap row E1.8 mentions a "`wire` floor"; typl reference §17.11
   explicitly defers the `wire` clause from v0.1. The reference wins: E1
   implements no `wire` surface, and the roadmap row is annotated at epic
   close-out. Generally, where a roadmap row and the language reference
   disagree, the reference is normative for E1.

8. **UCUM scope: term grammar plus a curated atom table.** E1 implements the
   UCUM term syntax (prefixes, `.` multiplication, `/` division, integer
   exponents, leading `/`, `%`, case-sensitive) over a curated atom table: SI
   base units and prefixes, the common derived and accepted units, `Cel`, `%`,
   and the automotive set the reference uses. An unknown atom is TYPL-110.
   Adopting the full UCUM table is deferred until real contracts demand it.

9. **IR v1 carries exact values as canonical decimal strings.** Range bounds,
   steps, and init values in IR v1 are canonical decimal strings, never
   floating-point fields — `float64` cannot represent values like `0.1` exactly,
   and E1.8's exact arithmetic must survive the IR round-trip for `ridl-diff`
   (E2.8) to compare honestly. Derived widths ride alongside as enums.

10. **Semantic scope cuts, recorded as debt.** TYPL-107 (regex vs length bound)
    needs regex length analysis, TYPL-401 needs doc-markdown reference
    resolution, TYPL-402/403 need assurance profiles that do not exist in V1,
    and TYPL-205 (repeated tuple shape) is `ridl lint` territory (E2). Of the
    profile-boundary family, TYPL-301/303/304 need the E2 family grammar to
    parse the constructs they reject and are deferred with it; TYPL-302
    (duration or timing in typl context) ships in E1, emitted by the parser,
    because the family lexer already produces the tokens. All the deferred codes
    roll into the E1 debt issue. TYPL-106 regex validation is implemented with
    the `regress` crate (an ECMA-262 engine, matching the reference's §2.7
    syntax choice, where Rust's `regex` crate dialect would not).

11. **The general form's attribute promotion is E2 scope.** E1 parses the typl
    profile per typl reference Appendix E, with `@labels` and `@deprecated` as
    doc-comment tags per typl §14. The general form §4 attribute block (single
    `attr_block` production, labels/deprecated as attributes, contextual keys)
    lands with the family grammar work in E2, when a second profile makes it
    real. The general form is a pre-ADR working spec; the published typl
    reference outranks it for the E1 surface.

12. **Fetch artifact format (provisional): an uncompressed tar archive of one
    package directory,** unpacked into the content-addressed cache. ADR-0002 is
    silent on the artifact format and the registry spec (E7.4) will fix the real
    one; this is the smallest honest form for `ureq` fetch + SHA-256 pinning to
    be end-to-end testable.

13. **extern-C strategy.** Generated Rust scalar newtypes are
    `#[repr(transparent)]`; structs whose IR `fixed_layout` flag holds are
    `#[repr(C)]`. The C header is rendered from the same IR via a `minijinja`
    template (a text target, per ADR-0004 §7), covering scalar typedefs, enum
    constants, and fixed-layout structs; shapes with no C ABI representation are
    listed in a header comment rather than silently dropped.

14. **Release, tagging, and publishing are maintainer acts.** The E1 milestone
    is the v0.1 preview, but the agent does not run `just release`, push tags,
    or publish to crates.io or the VS Code marketplace (same principle as
    ADR-0006 decision 5). The workspace version stays `0.0.0`; the epic's exit
    criteria are met by capability, and the preview cut is handed to the
    maintainer at close-out.

15. **`ridl.std` ships embedded in the compiler.** The Appendix A source is
    committed verbatim as an asset of `ridl-core` and loaded via `include_str!`
    as a built-in, implicitly imported package — no filesystem or network
    lookup, version-locked to the compiler binary.

16. **The merge gate carries over.** CI is still stuck; ADR-0006 decision 8's
    local gate (`just verify`, `cargo test --workspace`,
    `cargo fmt --all --check`,
    `cargo clippy --workspace --all-targets -- -D warnings`) remains the merge
    gate for every E1 PR, unchanged.

## Consequences

- Positive: the generated AST removes the E0 accessor debt wholesale; a single
  diagnostics model with stable namespaces feeds terminal, LSP, and the future
  error index from one SSOT; exact decimal IR values make `ridl-diff` and the
  property-test generators trustworthy; feature-gated ns modules keep the wasm
  path open for the E4.4 playground.
- Negative / accepted: the curated UCUM table under-covers exotic units until
  extended; the provisional tar fetch format will change with the registry spec;
  `FORM-`/`MANI-` namespaces are an agent-taken vocabulary decision the
  error-index website (E4.2) inherits; five diagnostic codes from the typl
  reference ship unimplemented as recorded debt.
- Review hook: each numbered decision is reversible at the cost of a small
  refactor before E2 builds on it; the maintainer can veto any of them by
  reopening this ADR.

## References

- ADR-0002 — module system (the semantics E1.3–E1.6 implement).
- ADR-0004 — sequencing and stack (the frame this refines).
- ADR-0006 — E0 execution decisions (the pattern this follows).
- docs/ROADMAP.md — epic E1 stories and exit criteria.
- docs/specification/typl-language-reference.md — the language E1 builds.
- docs/wip/2026-07-18-e1-typl-tooling-spine-plan.md — the execution plan citing
  these decisions.
