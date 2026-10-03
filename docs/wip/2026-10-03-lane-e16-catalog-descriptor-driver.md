# Lane E16 driver — the catalog descriptor

Status: driver, 2026-10-03. One lane, stages D0 to D8. Run each stage in a fresh
session. This document is written for an agent with no prior context. It names
every record it relies on and carries the facts a session would otherwise need
from a conversation. Set the `THIS SESSION RUNS` line below before you start a
session, and do only that stage. Where this document says "stop", stop and
report to Sebastien. Do not guess past it.

Where this document and an ADR disagree, the ADR wins. This document summarizes.
The one exception is §4, which records Sebastien's answers of 2026-10-03. Where
an answer departs from an ADR (answer 4 and ADR-0014 decision 9; answer 8 and
the FlatBuffers runtime that ADR-0020 decision 5 permits `ridl-rt`), the stage
that applies it writes the decision record first.

**THIS SESSION RUNS: D1**. D0 is the pull request that added this document.

## 0. How to work in this repository

- Read `AGENTS.md` first. It names the records to read, the gate (`just build`),
  the conventions (Conventional Commits linted by git-std, prim over Markdown,
  plain literal prose with no idioms or figures of speech), and the rule that
  nothing is pushed to `main` directly.
- Work in a git worktree under `.claude/worktrees/` (the directory is
  gitignored). Run `./bootstrap` there and branch fresh from `origin/main`. Run
  `git branch --show-current` before every commit and push. Stage explicit
  paths, never `git add -A`. Run `cargo fmt --all` before a Rust commit and
  `just fmt` before a Markdown, TOML or YAML commit.
- Run `just verify` before every pull request.
- Review before merge: open the pull request, run `/review <PR>` (the docs-only
  lane when no executable line changes), post the ledger as a comment, fix what
  is kept, run pass 2, and run `just verify`. After that the driver may
  squash-merge. Sebastien has given that permission for reviewed and verified
  work. Grep the pull request body for closing keywords before merging: "does
  not close #N" still closes #N.
- Never push a tag, publish to a registry, add a secret, or push to `main`.
- A new crate adds its own scope to `.git-std.toml`, which is an explicit list.
- Implementation stages use `superpowers:subagent-driven-development` over the
  plan's tasks. Use the `fable` model for the size derivation (D4, D5) and the
  hash (D3). Use `opus` for the rest, and `sonnet` for mechanical sweeps.
- A stage agent that dispatches a review driver must wait for the review
  driver's ledger. If the review driver hands back before its seats report, wait
  for its next notification. If it then ends with no ledger, send it one message
  asking for its complete ledger. Do not hand back until the pull request merges
  or a real blocker stops you.
- Other sessions work in this repository at the same time. Before you start, run
  `gh pr list` and check for overlap with the shared files named in §3.

## 1. What this lane is

Epic E16 in `docs/ROADMAP.md`: one FlatBuffers catalog descriptor per package,
which an engine (a bus configurator, a runtime outside this repository) reads
without decoding the IR. It carries the interface numbers, the members and their
kinds, the catalog hash, and a size state per payload for proto3 and FlatBuffers
(§4 answer 5). `ridlc build --emit catalog` writes it, a verifier checks it
before any access, and `ridl describe` prints it as strict JSON. The system
descriptor (per deployment) is out of scope. It follows the rsdl lowering.

This is the release's critical path (`docs/BACKLOG.md`, "Next delivery
sequence", items 1 and 2).

**Records.**

- Design: `docs/wip/2026-09-13-runtime-descriptors-design.md`, decisions D-1 to
  D-10. Sebastien approved it in conversation on 2026-09-13, before PR #323
  merged it.
- Plan: `docs/wip/2026-09-13-catalog-descriptor-plan.md`, 12 tasks with code (PR
  #324, 2026-09-13).
- Identity: `docs/wip/2026-09-12-rsdl-rewrite-decisions.md` D-7 and D-8.
- Debt on the plan: #326 (the book census for a new subcommand and a new emit
  value). Resolve its first three items before Task 11 and the rest before
  Task 12.
- ADR-0007 decision 14 (publishing), ADR-0010 (CLI), ADR-0014 decisions 4 and 9
  (artifact naming, canonical encoding), ADR-0017 (proto3 projection), ADR-0019
  (FlatBuffers projection), ADR-0021 decision 3 and ADR-0023 (the port's catalog
  check).

**Issues.**

| Stage | Issue | Story                                              | Plan tasks | Size |
| ----- | ----- | -------------------------------------------------- | ---------- | ---- |
| D2    | #377  | E16.1 — crate, `catalog.fbs`, accessors, verifier  | 1, 2       | L    |
| D3    | #378  | E16.2 — numbering in the descriptor, catalog hash  | 3, 4       | M    |
| D4    | #379  | E16.3 — size context, type leaves, string capacity | 5          | M    |
| D5    | #380  | E16.4 — proto3 and FlatBuffers upper bound         | 6, 7       | M    |
| D6    | #381  | E16.5 — lowering, `ridlc build --emit catalog`     | 8, 9       | M    |
| D7    | #382  | E16.6 — JSON view, `ridl describe`, records        | 10, 11, 12 | M    |
| D8    | #367  | E6.17 — the catalog hash in each rsdl region       | —          | S    |

Related: #275 (E9.10, the schema hash over the IR). The backlog says to
coordinate it with #378. §4 answer 11 closes it with #378.

## 2. Why the plan must be re-baselined first

The plan was written on 2026-09-13. Three later pull requests edited it in place
(#355, #473 and #474), and other work has landed around it. Each item below can
make a task's code or a disposition wrong.

- **Task 7 is already replaced.** The plan's Task 7 now opens with "Amended
  2026-09-21, E11.7 stage K8. Do not implement the bound this task describes."
  The FlatBuffers bound has one implementation,
  `ridl_ir::projection::flatbuffers::max_size`
  (`crates/ridl-ir/src/projection/flatbuffers.rs`), and the descriptor must call
  it. A second implementation would be a defect (E11.7 design D-6, in
  `docs/archive/2026-09-20-flatbuffers-codec-design.md`; see also
  `docs/design/flatbuffers-codec.md`). That bound charges four bytes per
  declared character of a string, and §4 answer 7 defers any `match` narrowing
  to #665. No proto3 bound exists yet.
- **E15, the lock, landed.** The IR carries `Interface.number` and
  `Interface.provisional`. `interfaces.lock` has a parser and a writer, and
  `ridl lock` exists. The plan's disposition "every interface shape is
  provisional, numbered 1.. in `Package::shapes()` order", and its
  `number_interfaces` in Task 3, were written for a time with no lock.
- **ADR-0014 decision 9 was amended on 2026-09-22.** The canonical encoding of
  the IR is now canonical protobuf JSON, and the binary encoding is derived. The
  plan (Task 4 hashes `ridl_ir::v2::to_binary`) and #378 say the hash is taken
  over "the canonical protobuf binary". #275 also depends on the canonical JSON
  form. §4 answer 4 settles the hash input.
- **Code already waits for E16.2's hash and sizes.**
  - The codegen model, `ridl.codegen.v1`
    (`crates/ridl-ir/proto/ridl/codegen/v1/model.proto`), has a `Catalog`
    message whose `hash` field is "all zero until E16.2 (driftsys/ridl#378)". It
    carries the FlatBuffers bound per payload (`Payload.flatbuffers_max_size`)
    and per root (`FbRoot.max_size`).
  - The Rust backend's descriptors
    (`crates/ridl-backend-rust/src/descriptors.rs`) emit a zero `CatalogHash`
    and every `EncodedSizes` field as `None`. `docs/ROADMAP.md` lists both as
    E11.13 placeholders that E16.2 retires.
  - ADR-0021 decision 3 and ADR-0023 (amendments of 2026-09-21) say the port's
    catalog check lands with E16.2 or after it, and that the story that emits
    the check decides what `new` does on a mismatch, as an amendment to
    ADR-0023. The frame specification says the attach check and
    `ridl-loopback`'s `UnknownInteraction` wait on E16.2 as well.
  - The size types differ: `ridl_rt::contract::EncodedSizes` holds
    `Option<u32>`, the codegen model holds `uint32`, and the plan's
    `catalog.fbs` holds `uint64`. §4 answer 5 picks `uint32`.
- **The rule on planus is stated more widely than its decision.** The root
  `Cargo.toml` says planus and planus-codegen are dev-dependencies of
  `ridl-backend-rust` only, and that nothing the workspace ships depends on
  either (E11.7 design D-12). `xtask/tests/oracle_boundary.rs` checks only three
  backend crates (`ridl-backend-flatbuffers`, `ridl-backend-proto`,
  `ridl-backend-rust`), so no test would catch a new crate that breaks the rule.
  Plan Task 1 makes `planus` a normal dependency of `ridl-descriptor`, which
  `ridlc` and `ridl` then depend on. §4 answer 8 settles this.
- **Every crate is now published unless it opts out.** ADR-0007 decision 14
  (amended 2026-09-21): a `v<version>` tag publishes every crate without
  `publish = false`, so an internal dependency must be a workspace entry with a
  path and a version. The plan's manifest for `ridl-descriptor` uses a path-only
  `ridl-ir` dependency, which `cargo publish` refuses.
- **The frame specification (E11.1) landed.** The plan's disposition on what a
  payload is on the wire says "E11.1 must adopt or amend this". Check what
  `docs/specification/frame-specification.md` (#257) decided.
- **`match` semantics are unchanged in code.** The checker still applies a
  pattern as a substring search (`regress` `find` in
  `crates/ridl-sem/src/check.rs`), which is the plan's premise. #601 added the
  TYPL-220 compile check. The portable-patterns design
  (`docs/wip/2026-10-01-portable-match-patterns-design.md`, #597) is not
  implemented, but if it lands it can change the premise.
- **The CLI grew.** `ridl` gained `lock`, `lsp` and `mcp` after the plan was
  written, so every subcommand count in the plan's Task 11 census is stale;
  recount each against `docs/book/cli-reference.md`. `codegen-model` (lane P) is
  the eighth emit value, so `catalog` is the ninth. #326's title and body count
  both one too low.

## 3. Stages

### D0 — this document

The pull request that adds this file. Docs-only review lane.

### D1 — re-baseline the plan

Model: `fable` for the audit. Docs only. No code.

1. Read the design, the plan, #326, and each change listed in §2. For each plan
   task, record what no longer holds on `main`: a type or function that was
   renamed or removed, a file that moved, a dependency that already exists, a
   disposition that a landed record decided differently.
2. Amend the plan in place on a branch. Apply the answers in §4. Correct every
   other record that states a criterion a §4 answer overturns. Search for them;
   the review of this driver found at least these. In the same pull request:
   `docs/ROADMAP.md` (the Epic 16 exit criteria, and the E16.2 and E16.4 rows),
   and the runtime-descriptors design (D-4's stream flag, D-6's every-payload
   heading, its one-number-per-encoding row, its stream wording and its `match`
   narrowing, and the §6 bullet on §3.11). With `gh issue edit`: the bodies of
   #275, #378, #379 and #380. Apply #326's items, with the census recounted
   against today's CLI. Keep the task numbering, so the issues' task references
   stay valid. Remove a task only if it is empty.
3. Write a short "Re-baseline 2026-10" section at the top of the plan. List
   every change, and the record or the §4 answer that caused each one.
4. Open the pull request (docs-only lane), review it, fix it, and merge it.
   Close #326 if the plan now carries all of its items.
5. D2 can start after this merge. Stop before D2 only if the audit found a fact
   that contradicts a §4 answer.

### D2 — E16.1 (#377): the crate, the schema, the verifier

Plan Tasks 1 and 2. A new crate `crates/ridl-descriptor/` and its
`.git-std.toml` scope. `cargo xtask descriptor-codegen` generates the accessors,
which are committed with a drift test. Nothing may depend on `flatc`. Toolchain
crates may depend on planus, directly or through `ridl-descriptor`; `ridl-rt`
and every generated package must not (§4 answer 8). Done when a catalog
round-trips through the builder and the reader, and a buffer with a wrong
identifier or version is rejected as a whole.

### D3 — E16.2 (#378): numbering and the catalog hash

Plan Tasks 3 and 4. Every walk over a package's interfaces goes through
`Package::shapes()`, so inline service shapes are included. Apply §4 answers 3,
4 and 11 before you write the hash. Fill the codegen model's `Catalog.hash`, and
retire the Rust backend's zero `CatalogHash`. Decide which stage emits the
port's catalog check. That stage also takes the decision ADR-0021 decision 3 and
ADR-0023 leave open (what `new` does on a mismatch), as an ADR-0023 amendment.
Done when two packages with the same declarations and numbering hash alike,
changing one number changes the hash, the hash is the same whether a build emits
proto3, FlatBuffers or both (§4 answer 11), and a golden-hash test runs in the
gate (§4 answer 4).

### D4 — E16.3 (#379): the size context

Plan Task 5. Model `fable`. Done when every type leaf yields a byte bound or is
reported as unsizable. D4 can run beside D3 if the re-baselined plan shows the
two touch disjoint files. Otherwise it runs after D3.

### D5 — E16.4 (#380): the two upper bounds

Plan Tasks 6 (proto3) and 7 (FlatBuffers). Model `fable`. Task 6 derives the
proto3 bound under ADR-0017 as amended today. Task 7 calls
`ridl_ir::projection::flatbuffers::max_size` and computes nothing of its own
(§2). Fill the Rust backend's `EncodedSizes`, which are all `None` today. The
records give that placeholder to E16.2 (`docs/ROADMAP.md`,
`docs/design/interaction-face.md`), but the bounds exist only after this stage,
so amend those records to say E16.4. Done when each payload carries a size state
for both encodings: bounded or unbounded for a payload that is one named type,
and absent for a request with more than one parameter, an inline `T | E` reply
and a stream payload, even a stream of a named type (§4 answers 5, 6 and 10).

### D6 — E16.5 (#381): the lowering and the emit

Plan Tasks 8 and 9. The `Emit` enum in `crates/ridlc/src/lib.rs` is a shared
file. Check open pull requests first. Done when the corpus package writes a
descriptor that verifies, and two runs write the same bytes.

### D7 — E16.6 (#382): `ridl describe` and the records

Plan Tasks 10, 11 and 12. `crates/ridl/src/main.rs`,
`docs/book/cli-reference.md` and ADR-0010 are shared files. The devex track's
Spec 0 (lint foundation, brief in PR #656) also changes `ridl check`, the CLI
reference and ADR-0010. Coordinate with that session before you start. Done when
`ridl describe` prints a catalog's contents as JSON, and a rejected buffer exits
2 with its cause named.

### D8 — E6.17 (#367), then gardening

Put the catalog hash in each region of the lowered system
(`docs/technotes/rsdl-implementation.md`, "What is not built yet"). Then run
`sdd-gardening`: the design's decisions go to a `docs/design/` record (or an ADR
if Sebastien prefers), the plan, the design and this driver go to
`docs/archive/`, and the roadmap and the backlog mark E16 landed. Before moving
a wip file, read `docs/wip/README.md` and grep for citations of it by name in
`.rs`, `.ridl` and `.md` files. No gate catches a dead path in a source comment.

## 4. Decisions (answered by Sebastien on 2026-10-03)

Sebastien answered every question below on 2026-10-03, before D1 ran. D1 applies
these answers to the plan. It does not ask them again. If the audit finds a fact
that contradicts an answer, stop and report that fact.

1. **Crate.** `ridl-descriptor` is a separate crate beside `ridl-ir`, so only
   the crates that read or write a catalog take a FlatBuffers dependency. It
   uses `ridl-ir`'s IR types and its existing FlatBuffers bound, and does not
   copy them.
2. **Names.** The file identifier is `RDLC`, the artifact is
   `<base>.catalog.binfb`, and the emit value is `catalog`, as the plan says.
3. **Interface numbers.** The descriptor copies `Interface.number`,
   `Interface.provisional` and `Package.retired` from the IR. It computes no
   numbering of its own. `--emit catalog` writes a catalog when an interface is
   provisional, and the descriptor flags that interface (design D-4). A number
   of 0 is rejected as an internal error. The hash covers the numbers, so it
   changes when a provisional number changes.
4. **Hash input.** The hash covers the protobuf binary of the reduced package,
   as the plan says, not the canonical JSON. Record this in a decision record
   (an ADR-0014 amendment or a new ADR) with:
   - the determinism rule for the binary: fields in field-number order, list
     elements in the order the writer holds them, no `map<>` field, fields at
     their default omitted;
   - the reason: canonical JSON emits every non-`optional` field at its default,
     so each additive IR field would change every catalog hash at a toolchain
     upgrade, and the read-back bound behind ADR-0014 decision 9 does not apply
     to bytes that are only hashed;
   - a golden-hash test in CI, so a toolchain change that moves a hash fails the
     gate.
5. **Sizes.** The `repr(C)` column has no entries until E11.12 (#317). Each
   payload and encoding has one of three states: absent (no size is computed),
   bounded with a byte count, or unbounded with a cause (reuse the codegen
   model's `FbUnboundedCause`). Every size is a `uint32`, as in
   `ridl_rt::contract::EncodedSizes` and the codegen model.
6. **Payload shapes follow the codecs.** The descriptor does not define a wire
   shape. It sizes a payload that is one named type through the existing
   projections: ADR-0019 decision 8 for FlatBuffers, and ADR-0017's projection
   of the same type for proto3. A request with more than one parameter and an
   inline `T | E` reply have absent sizes until the frame specification and a
   codec define their encoding. The plan's induced request message and its
   `ok`/`err` union are removed.
7. **No `match` narrowing in E16.** A string counts 4 bytes per scalar value in
   both columns, as §3.11 of
   `docs/wip/2026-09-12-release-scope-and-plugin-system-design.md` and
   `max_size` do. The checker and the generated Rust code do not agree on what a
   pattern matches until the design of #597
   (`docs/wip/2026-10-01-portable-match-patterns-design.md`, approach A) is
   implemented, so an ASCII-only verdict is not safe for a size bound. #665,
   which depends on that implementation, records the narrowing, to be built in
   one function that both bounds call.
8. **planus.** The toolchain may depend on planus: `ridl-descriptor`, and
   through it `ridlc` and `ridl`. `ridl-rt` and every generated package must
   not. Narrow the root `Cargo.toml` comment to say that, and extend
   `xtask/tests/oracle_boundary.rs` so that it checks `ridl-rt` and the
   dependency closure of the generated crate. D2 confirms that `just wasm-check`
   still passes.
9. **Publishing.** `ridl-descriptor` is published to crates.io. It is a
   workspace dependency entry with a path and a version, and the release
   procedure publishes it after `ridl-ir` and before `ridlc`.
10. **Streams.** A stream payload has absent sizes, and the descriptor has no
    `stream` flag yet. The story that builds the stream port and its codec
    (#336) adds the per-element bound and the flag, as an append to the schema.
    This replaces the review-driven wording of PR #323 on streams.
11. **#275.** There is one identity. The catalog hash is the schema hash over
    the IR. #378 gains #275's criterion: the hash is the same whether a build
    emits proto3, FlatBuffers or both. #275 closes when #378 merges.

One question stays open for the stage that needs the answer: which stage emits
the port's catalog check, and what `new` does on a mismatch (D3).
