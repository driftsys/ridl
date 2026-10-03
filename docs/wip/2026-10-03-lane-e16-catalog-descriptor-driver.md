# Lane E16 driver — the catalog descriptor

Status: driver, 2026-10-03. One lane, stages D0 to D8. Run each stage in a fresh
session. This document is written for an agent with no prior context. It names
every record it relies on and carries the facts a session would otherwise need
from a conversation. Set the `THIS SESSION RUNS` line below before you start a
session, and do only that stage. Where this document says "stop", stop and
report to Sebastien. Do not guess past it.

Where this document and an ADR disagree, the ADR wins. This document summarizes.
It does not decide.

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
kinds, the catalog hash, and a maximum encoded size per payload for proto3 and
FlatBuffers. `ridlc build --emit catalog` writes it, a verifier checks it before
any access, and `ridl describe` prints it as strict JSON. The system descriptor
(per deployment) is out of scope. It follows the rsdl lowering.

This is the release's critical path (`docs/BACKLOG.md`, "Next delivery
sequence", items 1 and 2).

**Records.**

- Design: `docs/wip/2026-09-13-runtime-descriptors-design.md`, decisions D-1 to
  D-10. Sebastien approved it on 2026-09-13 (PR #323).
- Plan: `docs/wip/2026-09-13-catalog-descriptor-plan.md`, 12 tasks with code (PR
  #324, 2026-09-13).
- Identity: `docs/wip/2026-09-12-rsdl-rewrite-decisions.md` D-7 and D-8.
- Debt on the plan: #326 (the book census for a new subcommand and a new emit
  value). Resolve its first three items before Task 11 and the rest before
  Task 12.
- ADR-0010 (CLI), ADR-0014 decision 4 (artifact naming), ADR-0016 (the name
  transform), ADR-0017 (proto3 projection), ADR-0019 (FlatBuffers projection).

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
coordinate it with #378. See §4.

## 2. Why the plan must be re-baselined first

The plan was written on 2026-09-13. These changes have landed since, and each
one can make a task's code or a disposition wrong:

- **E15, the lock, landed.** The IR carries `Interface.number` and
  `Interface.provisional`. `interfaces.lock` has a parser and a writer, and
  `ridl lock` exists. The plan's disposition "every interface is provisional,
  numbered 1.. in `Package::shapes()` order" and its `number_interfaces` (Task
  3) were written for a time with no lock. They are now at least partly replaced
  by what the checker already computes.
- **planus is a workspace dependency.** `planus`, `planus-codegen` and
  `planus-translation` 1.3.0 are in the root `Cargo.toml` (the FlatBuffers
  codec, E11.7, #473). Task 1 adds them as if they were new.
- **The FlatBuffers projection changed.** #474 roots every declaration in a
  table. #604 changed how an omitted default reads and gave an optional scalar
  `= null`. ADR-0019 has amendments, decision 8 among them. Task 7's upper bound
  follows the projection, so check it against the ADR as it stands.
- **Part of the catalog already exists in other code.** Lane P built the lowered
  codegen model, `ridl.codegen.v1`
  (`crates/ridl-ir/proto/ridl/codegen/v1/model.proto`). It has a `Catalog`
  message whose `hash` field is "all zero until E16.2 (driftsys/ridl#378)", with
  the retired interfaces. It also has a per-scope FlatBuffers bound, which comes
  from `max_size` in `crates/ridl-ir/src/projection/flatbuffers.rs`. Task 4 must
  fill that hash field, and the descriptor and the codegen model must agree on
  the hash. Task 7 must reuse the existing FlatBuffers bound or state why it
  differs. No proto3 bound exists yet.
- **The frame specification (E11.1) landed.** The plan's disposition on "what a
  payload is on the wire" (induced single-field message, request as the induced
  message of the parameters, fallible response as a union over `ok` and `err`)
  says "E11.1 must adopt or amend this". Check what E11.1 decided in
  `docs/specification/frame-specification.md` (#257).
- **`match` semantics.** The plan narrows a string's byte capacity only for an
  anchored ASCII-only pattern, because `ridl-sem` applied `match` as a substring
  search. #597 (portable match patterns,
  `docs/wip/2026-10-01-portable-match-patterns-design.md`) and #603 (pattern
  cache) changed this area. Check how the checker applies a pattern now.
- **Generated names.** ADR-0016 amendments, #608 (renamed backend-chosen names)
  and the name-collision work (#611, #612) can change the names the plan's
  snapshots expect.
- **The CLI grew.** `ridl lock` was added, so the subcommand counts in #326 and
  in Task 11's book census are off by at least one.
- **The release is 0.5.1.** Version pins in the plan's `Cargo.toml` snippets are
  stale.

## 3. Stages

### D0 — this document

The pull request that adds this file. Docs-only review lane.

### D1 — re-baseline the plan and settle the dispositions

Model: `fable` for the audit. Docs only. No code.

1. Read the design, the plan, #326, and each change listed in §2. For each plan
   task, record what no longer holds on `main`: a type or function that was
   renamed or removed, a file that moved, a dependency that already exists, a
   disposition that a landed record decided differently.
2. Amend the plan in place on a branch. Apply #326's items, with the census
   recounted against today's CLI. Rewrite Task 3 so that the descriptor reads
   the numbers the checker already assigns from the lock, rather than computing
   its own. Keep the task numbering, so the issues' task references stay valid.
   Remove a task only if it is empty.
3. Write a short "Re-baseline 2026-10" section at the top of the plan. List
   every change, and the record that caused each one.
4. Open the pull request (docs-only lane), review it, fix it, and merge it.
5. **Stop.** Report the dispositions in §4 to Sebastien with the audit's
   evidence, and wait for the answer before D2. Close #326 if the plan now
   carries all of its items.

### D2 — E16.1 (#377): the crate, the schema, the verifier

Plan Tasks 1 and 2. A new crate `crates/ridl-descriptor/` and its
`.git-std.toml` scope. `cargo xtask descriptor-codegen` generates the accessors,
which are committed with a drift test. Nothing may depend on `flatc`.
`xtask/tests/oracle_boundary.rs` keeps schema compilers out of normal
dependencies. Done when a catalog round-trips through the builder and the
reader, and a buffer with a wrong identifier or version is rejected as a whole.

### D3 — E16.2 (#378): numbering and the catalog hash

Plan Tasks 3 and 4. Every walk over a package's interfaces goes through
`Package::shapes()`, so inline service shapes are included. Settle the #275
question (§4) before you write the hash. Done when two packages with the same
declarations and numbering hash alike, and changing one number changes the hash.

### D4 — E16.3 (#379): the size context

Plan Task 5. Model `fable`. Done when every type leaf yields a byte bound or is
reported as unsizable. D4 can run beside D3 if the re-baselined plan shows the
two touch disjoint files. Otherwise it runs after D3.

### D5 — E16.4 (#380): the two upper bounds

Plan Tasks 6 (proto3) and 7 (FlatBuffers). Model `fable`. Each bound follows its
projection ADR as amended today. Done when each payload carries a maximum
encoded size for both encodings.

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

## 4. Decisions for Sebastien (D1 reports them)

The plan takes these dispositions. Sebastien has not confirmed them. Items 6 and
7 are new design that the plan took, not answers to open items in the design's
§7. Say so when you report them.

1. The crate `ridl-descriptor` sits beside `ridl-ir`, not inside it.
2. The file identifier is `RDLC`. The artifact is `<base>.catalog.binfb`, and
   the emit value is `catalog`.
3. Interface numbers: written for a time with no lock. D1 replaces this with the
   numbers from the lock (E15). Confirm the replacement.
4. The catalog hash is SHA-256 over the canonical protobuf binary of a reduced
   package (every interface shape under its identity name, the reachable type
   declarations with canonical names, doc strings blanked), followed by the
   numbering. It is derived and never recorded.
5. The `repr(C)` column exists in the `Encoding` enum but has no entry in any
   payload's size list until E11.12 (#317) defines the layout.
6. What a payload is on the wire: a struct or union payload is that message or
   table. Any other payload is an induced single-field message or table. A
   request is the induced message of its parameters. A fallible response is a
   union over `ok` and `err`. D1 checks this against the frame specification.
7. `match` narrowing of a string's byte capacity applies only to a pattern
   anchored at both ends that admits ASCII only. D1 checks this against the
   checker's current pattern semantics.

Two design wordings came from the review, not from Sebastien, and are to be
raised before the size derivation (D4) is built:

- (b) A stream payload gets the maximum size of one element plus a `stream`
  flag, because the stream itself has no bound.
- (c) The `match` narrowing is kept, as an amendment to the design note's §3.11,
  which says 4·N bytes for every string.

And one scope question:

- **#275 (the schema hash over the IR).** The catalog hash already hashes a
  reduced IR. Either it satisfies #275 for interfaces, and #275 closes with
  #378, or the schema hash covers more (types outside any interface's closure),
  and both are needed. D1 proposes one answer with evidence.
