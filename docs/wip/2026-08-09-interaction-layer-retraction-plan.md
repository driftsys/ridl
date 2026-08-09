# Interaction Layer Retraction Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remove the generated interaction layer and the extern-C header from
both language backends, so the surface that E10 and E11 build on is the typl
type layer alone. Nothing is replaced in this work — phase 2 restores an
interaction face as a client and a server, over a runtime that exists.

**Architecture:** The interaction layer is
`crates/ridl-backend-rust/src/interact.rs` and
`crates/ridl-backend-ts/src/interact.rs` plus `interact/`, both emitting a
per-package vocabulary (`Provenance`, `SignalHandle`, `EventHandle`,
`TimingConst`, `ContractStub`) and consumer/provider trait pairs over it. The
header is `crates/ridl-backend-rust/src/c_header.rs`. All three are emitters;
none of them is upstream of anything. The IR, the checker, `ridl-sem`'s RIDL-149
collision rule, `ridl-diff` and the pinned name transform in
`crates/ridl-ir/src/name.rs` are all untouched — the transform is shared and
outlives its first caller.

**Tech Stack:** Rust 2024, `proc-macro2`/`quote`/`prettyplease` (Rust emitter),
plain string emission (TypeScript emitter), `insta` snapshots, `clap` (CLI).

**Spec:**
[`docs/decisions/ADR-0018-runtime-core-and-generated-surface.md`](../decisions/ADR-0018-runtime-core-and-generated-surface.md),
decisions 6 and 15. Where this plan and the record disagree, the record is
authoritative.

## Global Constraints

- **This is a deletion, not a refactor.** No behaviour is preserved and nothing
  is ported. A reviewer should be able to read the diff as removal plus the
  consequences of removal.
- **The ADR lands first.** ADR-0018 is Proposed; this work cites it. If the
  record is rejected or amended, this plan changes with it.
- **Conventional Commits**, linted by git-std against `.git-std.toml`. Scopes
  used here: `ridl-backend-rust`, `ridl-backend-ts`, `ridlc`, `ridl`, `docs`,
  `roadmap`.
- **Never push to `main`.** This work lands on its own branch via PR.
- **Run `just verify` before opening the PR** (`lint-commits`, then the full
  `build` gate). Individual tasks run `just test` and `just lint`.
- **Snapshots are `insta`.** Delete the obsolete ones rather than accepting
  empty rewrites; review the rest with `cargo insta review` and never hand-edit
  a `.snap` file.
- **Prose is plain and literal** — comments, commit messages, docs. No idioms.
- **`just check` needs `prim` on `PATH`.** `./bootstrap` installs it; a cargo
  install lands it in `~/.cargo/bin`, which is not always on the default `PATH`.

---

## Task 0 — Survey the blast radius

- [ ] Grep for every reference to the emitted vocabulary and to the header
      across `crates/`, `docs/`, and `editors/`: `SignalHandle`, `EventHandle`,
      `Provenance`, `TimingConst`, `ContractStub`, `RidlStream`, `c-header`,
      `CHeader`.
- [ ] Record which of `ridl-lsp`'s features depend on the emitted layer as
      opposed to the IR. E2.10 shipped interaction hovers and timing display;
      those read the IR and should survive, but confirm rather than assume.
- [ ] Check whether any verified fence in `docs/book/` names `c-header` or shows
      generated interaction code. Fences are compiled by
      `crates/ridl/tests/book_examples.rs`, which compiles `.ridl` source rather
      than inspecting generated output, so the expected answer is that only
      prose and the emit table mention it.
- [ ] Write the findings into the PR description before deleting anything.

**Verify:** the survey names every file the later tasks touch.

## Task 1 — Remove the Rust interaction layer

- [ ] Delete `crates/ridl-backend-rust/src/interact.rs` and its module
      declaration.
- [ ] Delete the interaction snapshots under
      `crates/ridl-backend-rust/src/snapshots/` — the per-kind ones
      (`signal_carries_init_and_provenance`, `event_is_a_subscribe_only_handle`,
      `command_returns_unit_and_records_its_require`, the fallible-query,
      stream, `fixed`, services, RPC-bounds and visibility cases) and the
      interaction half of the Appendix A and Appendix B snapshots.
- [ ] Delete the corresponding tests in `crates/ridl-backend-rust/src/tests.rs`,
      including the helpers that exist only for them (`interaction_package`,
      `rust_for_interaction`).
- [ ] Confirm `InducedTuple` discovery still works for tuples reached through
      declarations. Tuples induced only by interaction positions disappear with
      the layer; tuples in declarations must not.

**Verify:** `just test` and `just lint` pass; `--emit rust` on the corpus emits
types and constants and nothing else.

## Task 2 — Remove the TypeScript interaction layer

- [ ] Delete `crates/ridl-backend-ts/src/interact.rs` and `src/interact/`,
      including its snapshots and tests.
- [ ] Remove the interaction half of the Appendix B TypeScript snapshot.
- [ ] Confirm the type-layer snapshots are unchanged — a diff in those means
      something was shared that should not have been.

**Verify:** `just test` passes; the TypeScript type snapshots show no diff.

## Task 3 — Remove the extern-C header

- [ ] Delete `crates/ridl-backend-rust/src/c_header.rs` and its snapshots.
- [ ] Remove `Emit::CHeader` from `crates/ridlc/src/lib.rs`, the `c-header` CLI
      value, and the branch in `run_build` that writes `<base>.h`.
- [ ] Confirm `crates/ridl-ir/src/name.rs` is untouched and still used. ADR-0016
      decision 1 pinned the transform to this file's algorithm and decision 2
      moved it to `ridl-ir`; it is shared and must outlive the header.
- [ ] Update the emit tables in `docs/book/getting-started.md` and the `--emit`
      help text.

**Verify:** `just test`, `just lint` and `just book-check` pass;
`ridlc build
--emit c-header` reports an unknown value rather than a panic.

## Task 4 — Record the consequences

- [ ] Add a note to `docs/book/` wherever the generated interaction API is
      described as shipped, stating that the face is retracted and phase 2
      restores it. The book describes the system as built (AGENTS.md).
- [ ] Close driftsys/ridl#236 as won't-fix, citing ADR-0018 decision 6 — the C
      header's type-name collision cannot occur once the header is gone.
- [ ] Leave driftsys/ridl#237 open. The union-arm camel-caser is in
      `crates/ridl-backend-rust/src/lib.rs`, not in the interaction layer, and
      E10's variant naming still walks into it.
- [ ] Check whether any remaining diagnostic code became unreachable. RIDL-149
      stays — it is a `ridl-sem` rule about a package, not about a backend.

**Verify:** `just build` passes end to end.

## Task 5 — Open the PR

- [ ] `just verify`.
- [ ] Open a draft PR citing ADR-0018 decisions 6 and 15, with Task 0's survey
      in the body and an explicit list of what was removed against what was
      deliberately kept.
- [ ] State plainly that this removes shipped surface and that E2's exit
      criterion is retracted, so a reviewer does not have to infer it.

---

## What this plan does not do

- **It does not add the compile gate.** That is E10.7, which makes `--emit rust`
  write a crate that compiles. Sequencing it there rather than here keeps this
  change a pure deletion.
- **It does not touch the IR, the checker, `ridl-diff` or the LSP.** If any of
  those needs a change, Task 0 found something this plan did not anticipate and
  the plan should be amended before proceeding.
- **It does not begin phase 2.** The client and server face is Epic 11's work,
  over a runtime that does not exist yet.
