# Lane E16 driver — the catalog descriptor

Status: driver, 2026-10-03. One lane, stages D0 to D8. Run each stage in a fresh
session. This document is written for an agent with no prior context. It names
every record it relies on and carries the facts a session would otherwise need
from a conversation. Set the `THIS SESSION RUNS` line below before you start a
session, and do only that stage. Where this document says "stop", stop and
report to Sebastien. Do not guess past it. Since 2026-10-04 Sebastien has
delegated the lane's open decisions: a stage takes a decision this document or
the plan reserves for him, and records it in §5 for his review, instead of
stopping. Irreversible actions (a tag, a publish, a secret, a push to `main`)
still stop.

Where this document and an ADR disagree, the ADR wins. This document summarizes.
The one exception is §4, which records Sebastien's answers of 2026-10-03. Where
an answer departs from an ADR (answer 4 and ADR-0014 decision 9; answer 8 and
the FlatBuffers runtime that ADR-0020 decision 5 permits `ridl-rt`), the stage
that applies it writes the decision record first.

**THIS SESSION RUNS: D4**. D0 is the pull request that added this document; D1
re-baselined the plan on 2026-10-03 (its "Re-baseline 2026-10" section lists
every change and the decisions it took beyond §4). D2 landed as PR #669 on
2026-10-03. Every buffer is finished with `ridl_descriptor::finish`, not with
planus's `Builder::finish(.., Some(id))`, which writes the header in the wrong
order in planus 1.3.0. The planus crates are pinned to `=1.3.0`. D3 landed as PR
#676 (742c0a3f) on 2026-10-04. Facts from D3 that the plan text does not have:

- The catalog hash is computed in `ridl-ir`, not in `ridl-descriptor`:
  `ridl_ir::catalog_hash::{catalog_hash, reduced_package, reachable_decls}`,
  re-exported as `ridl_descriptor::hash`. The codegen model's `Catalog.hash`
  carries it, and the Rust backend writes it into every `Interface::CATALOG`.
  ADR-0014 decision 15 is the record.
- `ridl-descriptor` depends on `ridl-ir` (no `sha2`, no `serde_json` yet), and
  `ridl_descriptor::number::numbered_shapes` copies the numbers. The retired
  list is copied by the lowering (Task 8).
- The reduced package sorts its interfaces by (number, name), not in
  `Package::shapes()` order. It follows names inside contract clauses and
  constant values. It blanks `labels` and `deprecated` as well as doc strings.
  Plan Task 4's text still shows the earlier forms; the code and ADR-0014
  decision 15 are current.
- The golden test is `crates/ridl-descriptor/tests/golden_hash.rs`. The plan's
  `the_hash_is_the_same_whatever_a_build_emits` was removed. #275's criterion is
  tested in `crates/ridl/tests/facade.rs`.
- `xtask/tests/shape_walk.rs` counts the non-comment lines in `catalog_hash.rs`
  that contain `.interfaces`, whatever the receiver. A new line that reads or
  edits a package's `.interfaces` field changes the count; a struct-literal
  field written `interfaces: vec![..]` does not.
- Debt from the review: #679.

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
for both encodings: bounded or unbounded for a payload that is one named type
(on the proto3 side, only a struct or a union payload is sized, §5 D1 item 5),
and absent for a request with more than one parameter, an inline `T | E` reply
and a stream payload, even a stream of a named type (§4 answers 5, 6 and 10).

### D6 — E16.5 (#381): the lowering and the emit

Plan Tasks 8 and 9. The `Emit` enum in `crates/ridlc/src/lib.rs` is a shared
file. Check open pull requests first. D3 gave this stage the port's catalog
check (§5, D3 item 6): emit the comparison of `port.catalog()` with the
interface's `CATALOG` in each generated constructor (ADR-0021 decision 3), and
record what the generated `new` does on a mismatch as an amendment to ADR-0023.
The runtimes built in `crates/ridl-backend-rust/tests/interaction_face.rs` and
in `examples/cabin/consumer` then take the generated `CATALOG`, not a zero
`CatalogHash`. `generate_face` and `generate_face_with` lower with no other
packages, so their hash differs from `ridl build`'s for a package that
references another. Done when the corpus package writes a descriptor that
verifies, two runs write the same bytes, and a face built over a port bound to
another catalog does what the amendment says.

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

## 5. Decisions taken under delegation

On 2026-10-04 Sebastien delegated the lane's open decisions to the session that
drives it, and asked for every decision to be recorded here for his review. Each
stage appends the decisions it takes that neither the plan nor §4 settles, in
the follow-up pull request that moves the `THIS SESSION RUNS` line. Each entry
gives the decision, the reason, and what it costs if it is wrong. A decision
Sebastien overturns is struck through here, and the stage that reverses it is
named.

### D1 — the plan's "Re-baseline 2026-10" items 1 to 8, accepted

1. **The hash does not cover `Package.retired`.** Reason: the hash is the
   identity of the interfaces, their numbers and the types they reach (rsdl note
   D-8), and the retired list is carried beside it. Retiring an interface
   removes it from those interfaces, so the hash still changes. Cost if wrong: a
   hash-input change in D3's decision record and its golden value.
2. **A reduced interface takes `InterfaceShape::visibility()`.** Reason: a
   declared interface and an inline service shape with the same content must
   hash alike. Cost if wrong: one field of the reduced package.
3. **A stream payload's `type_name` is the spelled `<T>`.** Reason: it names the
   element type without a `stream` flag, which answer 10 defers to #336. Cost if
   wrong: a string format that #336 can change with the flag.
4. **A request with zero parameters has absent sizes.** Reason: like a request
   with more than one, it is not one named type, and answer 6 sizes only one
   named type. Cost if wrong: one more payload shape for a later codec story.
5. **A proto3 size is computed only for a struct or a union payload.** A named
   scalar, an enum or an enum-set payload has an absent proto3 state. Reason:
   ADR-0017 decision 1 inlines a named scalar and an enum set into their field,
   decision 2 rejects a wrapper message per named scalar, and an enum is a
   declared `enum`, not a message, so none of the three has a proto3 root form.
   Giving one would need an induced message, a wire shape the descriptor would
   define, which answer 6 forbids. This narrows §3 D5's done criterion on the
   proto3 side (amended there), and the plan's re-baseline item 5 and Task 6 no
   longer wait for Sebastien: D5 may start Task 6. Cost if wrong: the proto3
   column is empty for those payloads until a codec story defines their root
   form.
6. **The FlatBuffers cause comes from `ridl_ir::codegen::fb_unbounded`.**
   Reason: one implementation of the cause, as with `max_size`. Cost if wrong:
   one function made public in `ridl-ir`.
7. **`lower` returns a `Result`, and `ridlc` reports interface number 0 as an
   internal error with exit 2.** Reason: answer 3 and ADR-0010 decision 1. Cost
   if wrong: an exit code.
8. **Cargo commands for `crates/ridl` say `-p ridl-cli`.** Reason: that is the
   package name (AGENTS.md). Cost if wrong: none; it is a fact.

### D2 — PR #669 (004ca063)

1. **A public `ridl_descriptor::finish` writes every descriptor buffer.** planus
   1.3.0's `Builder::finish(_, Some(id))` writes the identifier at bytes 0..4
   and the root offset at 4..8, the reverse of the FlatBuffers layout, and
   planus's own reader then rejects the buffer. A test fails when a planus
   release changes this. Cost if wrong: the helper becomes one call.
2. **The three planus crates are pinned to `=1.3.0`.** Reason: the generated
   accessors call `check_version_compatibility("planus-1.3.0")`, so a caret
   range breaks the published crate for a consumer who resolves without the lock
   file. Cost if wrong: a pin bump with each planus upgrade, which the
   regeneration needs anyway.
3. **`serde` is a dependency of `ridl-descriptor`.** Reason: the code planus
   generates always derives serde. Cost if wrong: one dependency.
4. **The generator formats with rustfmt, not prettyplease, and writes the schema
   path relative to the repository.** Reason: `cargo fmt --check` and the drift
   test must pass in every checkout. Cost if wrong: the drift test needs
   rustfmt, which the pinned toolchain carries.
5. **The ADR-0020 decision 5 amendment narrows the permission instead of
   withdrawing it.** planus is barred from `ridl-rt` and every generated
   package, and "must not" covers every dependency kind. Another FlatBuffers
   runtime crate stays permitted under the `flatbuffers` feature, and nothing
   uses it (E11.7 D-12). Reason: answer 8 names planus only. Cost if wrong: one
   sentence of the amendment.
6. **The generated-crate planus check runs under `just demo`, not
   `cargo test`.** Reason: the crate must be generated first; `demo` fails
   unless exactly one test runs and passes. Cost if wrong: a contributor who
   runs `just test` without `just demo` does not run the check; CI runs both in
   its `rust` job.
7. **The planus checks resolve with every feature on.** The `ridl-rt` check and
   the generated-crate check run `cargo metadata --all-features`, and the
   `ridl-rt` closure takes every edge kind on its first step. Reason: a planus
   dependency behind a feature nothing turns on, or a dev-dependency of
   `ridl-rt`, must still fail the check. Cost if wrong: none found; optional
   features of `ridl-loopback` are not covered (#670).
8. **`ridl-descriptor` has no `ridl-ir`, `sha2` or `serde_json` dependency
   yet**, although plan Task 1 lists them. Reason: nothing in D2 uses them, and
   an unused dependency is a cost every consumer of a published crate pays.
   `.github/workflows/crates-io-release.yml` keeps the publish position answer 9
   sets, and its comment says the crate has no internal dependency. Cost if
   wrong: D3 adds `ridl-ir` back and restores that comment's sentence (#670).

### D3 — PR #676 (742c0a3f)

1. **The hash lives in `ridl-ir` (`ridl_ir::catalog_hash`), and
   `ridl-descriptor` re-exports it.** Reason: the codegen model is lowered in
   `ridl-ir` and must carry the hash, plugins and the Rust backend see only the
   model, and `ridl-ir` cannot depend on `ridl-descriptor`. Cost if wrong: a
   module move, and `sha2` stays a `ridl-ir` dependency.
2. **The decision record is an ADR-0014 amendment (decision 15), not a new
   ADR.** Reason: ADR-0014's scope is how the IR is encoded on every surface
   that writes it. Cost if wrong: moving the text to a new ADR.
3. **A bare reference inside another package's declaration resolves in that
   package, and the reduced package writes every type reference under its
   canonical name.** Reason: the IR writes a same-package reference bare, so
   resolving it in the root package reached the wrong declaration or none. Cost
   if wrong: none found; the corpus hash did not move.
4. **The doc tags `@labels` and `@deprecated` are blanked like doc strings.**
   Reason: they are metadata for generated code, not wire identity, and a
   deprecation must not make peers refuse each other. The lane delegate
   confirmed it. Cost if wrong: one field un-blanked, and the pinned values move
   once.
5. **The reduced interfaces are sorted by (number, name).** Reason: the lock
   makes the number the identity, and declarations are already sorted by
   canonical name. The lane delegate ruled it during review. Cost if wrong: one
   sort, and the pinned values move once.
6. **The port's catalog check is emitted by D6 (E16.5, #381), which also decides
   what `new` does on a mismatch.** Reason: the smaller scope for D3. ADR-0021
   decision 3 lets the check land with E16.2 or after it, and the mismatch
   behaviour changes every generated `new`. E16.5 writes the descriptor, which
   is the other artifact a pair is built from. Cost if wrong: until D6 merges, a
   face built over a port bound to another catalog reads and writes the wrong
   slots with no error, although the hash could already tell the two catalogs
   apart.
7. **Names inside expression strings are followed into the closure.** This
   covers contract clauses, and constant values for the cyclic case the lowering
   keeps as a name. A bare name the declaring package does not hold is looked up
   in every other package of the build, because the IR records no imports.
   Reason: a constant used only in a `require` clause changed without moving the
   hash. Cost if wrong: the hash covers some declarations it does not need, so
   it moves on a few unrelated changes. An import alias is still not followed
   (#679).
8. **`catalog_hash` skips an entry of `others` that has the root package's
   name.** Reason: `ridlc` passes the root among its own others. Cost if wrong:
   none observable.
9. **The Rust backend refuses a model whose hash is missing or is not 32 bytes
   long, for the whole emit.** Reason: a malformed model, and an error inside
   one interface only skips that interface. Cost if wrong: an older model with
   no catalog cannot be emitted.
10. **The codegen corpus snapshots are the guard for compiler-driven changes to
    the hash; the golden test pins only the reduction and the encoding.**
    Reason: the golden test reads a frozen IR snapshot. Cost if wrong: a pinned
    value from compiled source (#679).
