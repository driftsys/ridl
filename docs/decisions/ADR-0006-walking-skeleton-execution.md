# ADR-0006: Walking-Skeleton (Epic E0) Execution Decisions

## Status

Accepted — 2026-07-18. Scope: epic E0 only (docs/ROADMAP.md). Decisions here
refine ADR-0004 for the walking skeleton; none of them changes an ADR-0004
choice. Recorded so the delegating maintainer can review them after the fact.

## Context

Epic E0 builds the end-to-end skeleton: one trivial `.typl` file through lex →
parse → resolve → check → IR → generated Rust, snapshot-tested. ADR-0004 fixes
the stack (logos, hand-written parser, rowan, salsa, prost, quote +
prettyplease, insta) and the concept note §8.1 fixes the monorepo layout, but
several execution-level choices were still open. The execution plan lived in
docs/wip and is archived verbatim at
docs/archive/2026-07-18-e0-walking-skeleton-plan.md now that the epic has
landed.

## Decision

1. **Workspace layout.** A root Cargo workspace with members `crates/*` plus
   `backends/*`. E0 creates the five crates the roadmap names — `ridl-syntax`,
   `ridl-core`, `ridl-ir`, `ridlc`, `ridl` — and one backend crate,
   `ridl-backend-rust`, at `backends/rust` per the concept note §8.1 layout.
   Rust edition 2024; `version = "0.0.0"` until E1 cuts a preview.

2. **Semantic passes live in `ridl-core` for now.** The concept note places
   per-profile semantic passes in a `ridl-sem` crate, which E0 does not create.
   The E0 resolver and checker (and the salsa database) live in `ridl-core` and
   move to `ridl-sem` when that crate exists. The crate boundary is deferred,
   not dissolved.

3. **`protox` instead of a system `protoc`.** ADR-0004 §4 fixes protobuf via
   `prost`; the schema compiler was unstated. `ridl-ir/build.rs` compiles the
   schema with `protox` (a pure-Rust protobuf front end feeding `prost-build`),
   so builds are hermetic: no `protoc` on contributor machines or CI images.
   Revisit only if protox lags a proto feature the IR needs.

4. **Hand-written typed-AST accessors in E0.** ADR-0004 §2 plans
   ungrammar-generated accessors; generation infrastructure is not worth
   building for two declarations. E0 hand-writes the thin accessor layer; the
   ungrammar generator lands with E1's full typl grammar.

5. **crates.io name reservation is deferred.** E0.1's "done when" includes
   reserving the family crate names (concept note §10). Publishing requires the
   owner's crates.io credentials and is an outward-facing act the agent does not
   perform autonomously. Deferred with a tracked debt issue (#92); the concept
   note's own deadline (before V1) still binds. Resolved 2026-07-18: the
   maintainer supplied credentials and authorized the run, and all ten names
   (`typl`, `rmdl`, `rsdl`, `uxdl`, `rxdl`, `ridlc`, `ridl-syntax`, `ridl-core`,
   `ridl-ir`, `ridl-lsp`) are reserved as 0.0.0 placeholders (#92 closed).

6. **`ridl` facade is a stub.** The `ridl` crate builds a binary that only
   points at the roadmap. Porcelain subcommands are E1 scope; the crate exists
   now so the workspace shape is final from the first commit. The crate sets
   `publish = false`: the `ridl` name is taken on crates.io, and the naming
   ledger (concept note §10) prescribes shipping the `ridl` binary from the
   `ridlc` crate when publishing. That tension between §8.1 (a `ridl` workspace
   crate) and §10 (publishable names) is resolved with the reservation debt
   issue, before anything is published.

7. **Error surfaces in E0 are plain per-crate structs.** The coded `Diagnostic`
   model with `codespan-reporting` rendering is E1 scope (ADR-0004 §5, §10). E0
   accumulates simple error values and never converts a diagnostic into a hard
   error return, so the E1 model can replace the structs without changing
   control flow.

8. **Merge gate while CI is stuck.** The `rust` CI job (build, test, fmt,
   clippy) is added in E0.1 so it runs when CI returns. Until then the gate for
   every E0 PR is run locally: `just verify` (commit lint, build, test, lint)
   plus `cargo fmt --all --check` and
   `cargo clippy --workspace --all-targets -- -D warnings`. This is a process
   decision for this execution window, not a standing rule.

## Consequences

- Positive: hermetic builds (no protoc), a workspace whose shape already matches
  the concept note, and zero throwaway infrastructure (accessor generator,
  diagnostic renderer) built before the grammar that justifies it.
- Negative / accepted: `ridl-core` temporarily hosts passes that belong in
  `ridl-sem`, a known cut-and-paste move later; the crates.io squatting-risk
  window closed on 2026-07-18 when the maintainer-authorized reservation ran
  (decision 5).
- Review hook: each numbered decision is reversible before E1 at the cost of a
  small refactor; the maintainer can veto any of them by reopening this ADR.

## References

- ADR-0004 — implementation sequencing and stack (the frame this refines).
- docs/wip/ridl-family-concept.md §8.1 (layout), §10 (naming ledger).
- docs/ROADMAP.md — epic E0 stories and exit criteria.
