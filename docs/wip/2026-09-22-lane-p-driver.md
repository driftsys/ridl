# Lane P driver — the codegen plugin system, toward a Kotlin backend

Status: driver, 2026-09-22. One lane, six stages, each stage a fresh session.
This document is written for an agent with no prior context, of any vendor: it
names every record it relies on and carries every fact a session would otherwise
need from a conversation. Set the `THIS SESSION RUNS` line below before starting
a session, and do only that stage. Where this document says "stop", stop and
report to Sebastien; do not guess past it.

Where this document and an ADR disagree, the ADR wins. This document summarizes;
it does not decide. The decisions it carries as taken were taken by Sebastien on
the pull request that added it.

**THIS SESSION RUNS: P0**

## 0. How to work in this repository

- Read `AGENTS.md` first. It names the records to read, the gate (`just build`),
  the conventions (Conventional Commits linted by git-std, prim over Markdown,
  plain literal prose with no idioms or figures of speech), and the rule that
  nothing is pushed to `main` directly.
- Work in a git worktree under `.claude/worktrees/` (the directory is
  gitignored), run `./bootstrap` there, branch fresh from `origin/main`. Run
  `git branch --show-current` before every commit and push. Stage explicit
  paths, never `git add -A`. `cargo fmt --all` before a Rust commit, `just fmt`
  before a Markdown, TOML or YAML commit.
- If `git std` is missing:
  `cargo install --git https://github.com/driftsys/git-std git-std --locked`.
- `just verify` before every pull request. It runs the commit lint and the full
  gate, including `just demo`, which builds the compiler and runs
  `examples/cabin`. Expect several minutes.
- One pull request open at a time. Open it ready, never as a draft. Merge it
  yourself by squash once the review below is done and CI is green, with a
  Conventional Commits subject; add `!` when the change breaks a consumer of a
  public function or of the emitted crate.
- Every GitHub comment you post ends with a blank line, a `---` line, and one
  italic line naming the tool that wrote it, the way every comment on
  driftsys/ridl#328 does. No model identifier in any repository file or in a
  pull request title or body; a commit trailer naming the tool is the
  repository's convention and stays.
- A new crate adds its own scope to `.git-std.toml`, which is an explicit list
  (AGENTS.md, issue #180); the commit lint fails until it does. P2's
  `ridl-codegen`, if that home is chosen, and P3's reference plugin are new
  crates.
- Never push a tag, publish to a registry, or add a secret.

**Review, without subagents.** Each pull request gets a written review before
merge, posted as a ledger comment on the pull request with numbered findings and
a disposition per finding (fixed in commit X, or kept as is with the reason).
Run three passes yourself, in this order, and record what each did:

1. Correctness and records: read the diff against the ADRs and design records it
   cites; every claim in the pull request body checked against the tree, not
   against the body.
2. Tests by mutation: for every test the pull request adds, apply one mutation
   to the code it claims to pin and confirm the test goes red; restore it. A
   test that stays green under its mutation is a finding.
3. Refutation: take the three strongest claims in the body and try to falsify
   each by running something, not by reading.

Then a second pass over the fix commits alone. CI green is not review; the three
passes are.

## 1. Read first, in this order

- `AGENTS.md`, then `docs/ROADMAP.md` step 2 ("TypeScript and the plugin
  system") and the section "After step 2 — Kotlin, the first external plugin".
- `docs/wip/2026-09-12-release-scope-and-plugin-system-design.md` §3.8 (the
  plugin system, with the alternatives it rejected) and §3.9 (Kotlin as the
  first external plugin); then ADR-0020 decisions 8, 9 and 10
  (`docs/decisions/ADR-0020-third-encoding-runtime-layering-and-plugin-system.md`):
  lower once in the compiler; one contract
  `generate(CodegenRequest) -> CodegenResponse`; two hosts, in-process and a
  process host running `ridlc-gen-<language>` over stdin and stdout.
- ADR-0014 (`docs/decisions/ADR-0014-ir-encodings.md`): decision 9 makes binary
  the canonical IR encoding, JSON derived and conformance-obliged; decision 14
  (amendment) moved JSON onto `pbjson`. Issue #231 records that binary cannot
  round-trip IR between roughly 50 and 128 nesting levels because prost enforces
  a fixed recursion limit on decode only, while JSON round-trips.
- ADR-0016 (the pinned name transforms), ADR-0019 decision 8 (every declaration
  has a FlatBuffers root), ADR-0021 (the `ridl-rt` API), ADR-0023 (the generated
  face), and the three as-built records `docs/design/interaction-face.md`,
  `docs/design/flatbuffers-codec.md`, `docs/design/ridl-rt.md`.
- Issues #321 (E4.5a), #322 (E4.5b), #257 (E11.1), #231.
- The code: `crates/ridl-ir/proto/ridl/ir/v2/ir.proto` and
  `crates/ridl-ir/src/lib.rs` (the IR and its encodings);
  `crates/ridlc/src/lib.rs` (`Emit`, `run_build`, `write_emits`,
  `render_lib_rs`, `render_cargo_toml`); the four backends' entry points:
  `generate(&v2::Package)` in all four, `generate_with(package, others)` in
  `ridl-backend-proto`, `ridl-backend-flatbuffers` and `ridl-backend-rust` (not
  in `ridl-backend-ts`), and in `ridl-backend-rust` also `generate_face_with`
  and `generate_pipeline`; `crates/ridl-ir/src/name.rs` and
  `src/projection/flatbuffers.rs` (facts already shared between backends, the
  precedent for the lowered model); `crates/ridlc/tests/corpus.rs` and its
  snapshots (byte-identity is measured against these); `xtask/src/codegen.rs`
  (the pattern for generated code checked in with a staleness test).

## 2. Facts as of 2026-09-22

- `main` is at fa72f02 or later. The workspace carries one shared version,
  0.2.0, and every publishable crate is published to crates.io on a `v<version>`
  tag (ADR-0007 decision 14, amended 2026-09-21 and 2026-09-22); the emitted
  manifest requires `ridl-rt = "0.2"`. The Rust codegen is complete for one
  encoding: domain types (Epic 10), the FlatBuffers codec (E11.7), the
  interaction face (E11.13), all reached from `ridl build --emit rust` (E11.14),
  running over `ridl-loopback` (E11.15). `just demo` proves it end to end.
- Every backend reads the raw `v2` IR and re-derives the same semantics: name
  transforms, widths, init values, tombstones, ordinals, descriptors. The only
  shared derivations are `ridl_ir::name` and `ridl_ir::projection::flatbuffers`.
- No lowered model, no backend contract and no process host exist. `Emit` is an
  enum in `ridlc`; a language is added by editing it.
- The IR's JSON encoding round-trips every IR the front end admits; the binary
  one does not (#231). No consumer reads binary today.
- Kotlin is the first external plugin (release-scope note §3.9, roadmap "After
  step 2"). No merged record names its IPC binding; D-P5 below takes it. It
  needs the logical frame (E11.1) to know what crosses a boundary per
  interaction kind.

## 3. Decisions

**Taken by Sebastien for this lane** (record each in the stage that uses it,
with the alternative rejected):

- D-P1. Kotlin precedes TypeScript. E4.5b's exit test runs over the Rust backend
  through the process host, byte-identical to the in-process path, not over
  `ridlc-gen-ts`. This contradicts ADR-0020 decision 11 and the release-scope
  note §3.8's "Proof without a second language", both of which name the
  TypeScript backend as the parity test; P3 amends decision 11 in place with a
  dated note, and until then the contradiction is this line's.
- D-P2. The request carries the lowered model and the backend options, never the
  raw IR. A backend that still needs a raw-IR fact is a gap in the model, fixed
  in the model.
- D-P3. The plugin never touches the filesystem: `ridlc` writes the files
  (ADR-0020 decision 9, unchanged).
- D-P5. Kotlin's IPC binding is AIDL over Binder on Android, generated by the
  Kotlin backend from the same lowered model as its types and faces. The
  WebSocket transport (E11.9) is not on Kotlin's path. P0 writes this into the
  roadmap; no earlier record states it.
- D-P4. The lowered model must be sufficient to generate an IPC binding: per
  package, the interface numbers and ordinals, each interaction's kind, payload
  type and FlatBuffers `MAX_SIZE` (from the projection's bound), the timing
  bounds, the contract clauses in the form the translator accepts, and the
  catalog hash (all zeros until E16.2).

**Open, decided by a disposition comment from Sebastien on the pull request that
proposes them** (stop at each until it arrives):

- O-P1. The canonical IR encoding (#231): JSON becomes canonical and binary
  derived, or binary stays canonical with the nesting limit stated in the
  policy. Proposed in P1's design note.
- O-P2. Where the lowered model lives: `crates/ridl-ir` beside `name` and
  `projection`, or a new crate `crates/ridl-codegen`. Proposed in P2's design
  note, in the shape K2 used for the projection facts (the ground for the choice
  stated).
- O-P3. Whether a Rust Binder runtime (`ridl-transport-binder`, Android only) is
  in scope for the first Kotlin demo. Proposed in P0; not needed before the
  Kotlin side starts.

## 4. Stages

Each stage is one or more pull requests, each with the review above, each merged
before the next stage branches. After each stage, one comment on issue #328, the
step-1 lanes' coordination issue, which lane P joins as a new lane (its body is
never edited): the merge SHA, what landed, what another lane must know.

### P0 — the roadmap amendment (docs, S)

Files: `docs/ROADMAP.md`. This driver already exists; P0 is the roadmap change
it depends on.

- In `docs/ROADMAP.md`: move E4.5a and E4.5b ahead of E11.8, E11.12 and E12 in
  the sequence text; rewrite E4.5b's `Done when` per D-P1; rewrite the "After
  step 2 — Kotlin" section to say Kotlin depends on E4.5a, E4.5b and E11.1's
  logical frame, that the Kotlin backend owns its IPC binding (AIDL over
  Binder), and that E11.9 is not on its path; add O-P3 as an open item. Keep
  every story identifier; identifiers are identity.
- Done when: the roadmap says the above, `just check`, `just link-check` and
  `just doc-path-check` pass, and Sebastien has disposed of O-P3 on the pull
  request.

### P1 — E4.5a, the IR stability policy and the canonical encoding (M)

Two pull requests.

**P1a, the design note** `docs/wip/2026-09-2x-ir-stability-design.md`, decisions
numbered D-1 onward, each with the reason and the rejected alternative:

- The canonical encoding (O-P1), with #231's measurement reproduced: the nesting
  depth at which binary decode fails and JSON does not.
- The compatibility rule for the IR: which changes are additive (a new optional
  field, a new enum value), which are breaking, how a breaking change is
  versioned (a new `v3` package in the proto, never an edit to `v2`), and the
  version a request carries.
- What "canonical" fixes for JSON if JSON is chosen: field order, defaults
  emitted (ADR-0014 decision 2), 64-bit integers as strings (decision 8),
  byte-identical output across runs and hosts.
- Stop after opening the pull request; wait for the disposition.

**P1b, the code**, after the disposition: ADR-0014 amended in place with a dated
note on decision 9; the policy written into `docs/specification/` where the IR
encodings are specified (find the section; do not create a second one); a test
in `crates/ridl-ir` that round-trips every corpus package and a generated
package at the front end's maximum nesting depth through the canonical encoding;
#231 closed by the pull request or amended to say what remains. Must not break:
every `--emit ir-json`, `ir-text` and `ir-binary` snapshot, unless the
disposition changed the canonical form, in which case each moved snapshot is
read before re-accepting.

### P2 — the lowered codegen model (L)

Two pull requests.

**P2a, the design note** `docs/wip/2026-09-2x-codegen-model-design.md`:

- The model's content, message by message: packages, declarations with names
  already transformed per ADR-0016 for each target namespace the backends use
  today (Rust, proto3, FlatBuffers, TypeScript), widths derived, init values
  resolved, constraints as flat tables, tombstones resolved into slots, the
  FlatBuffers table layouts and bounds from `projection::flatbuffers`,
  interfaces with numbers, interactions with ordinals, kinds, payloads, timing,
  clauses, and the catalog reference (D-P4).
- Its home (O-P2) and its encoding: the canonical encoding of P1, in its own
  proto package `ridl.codegen.v1` beside `ridl.ir.v2`.
- What the model does not carry, and why: nothing a printer can compute from the
  model alone.
- How drift between the model and the four in-tree backends is caught: a test
  per backend that the backend's output over the model equals its output over
  the raw IR, which is the test P4 makes total.
- Stop after opening the pull request; wait for the disposition.

**P2b, the code**: the proto, the lowering function
`lower(&v2::Package, others) -> Model` in the chosen home, its canonical
encoding, a `ridl build --emit codegen-model` flag under ADR-0010's conventions
so the model is inspectable, corpus snapshots of the model, and one drift test
per backend as the note describes. Must not break: any existing emit or
snapshot; the model is additive until P4.

### P3 — E4.5b, the contract and the process host (L)

One pull request, or two if the host is large.

- `CodegenRequest { version, model, options }` and
  `CodegenResponse { files: [(path, bytes)], diagnostics }` in
  `ridl.codegen.v1`.
- The in-process host: a trait every in-tree backend implements over the
  request; `write_emits` in `crates/ridlc/src/lib.rs` calls it.
- The process host: `ridlc-gen-<language>` found on `PATH` or given by a flag
  (name the flag per ADR-0010), the request on stdin in the canonical encoding,
  the response on stdout, a non-zero exit or a malformed response reported as a
  `ridlc` error naming the plugin, with a timeout. The plugin never touches the
  filesystem (D-P3).
- A reference plugin in this repository used only by tests: a binary that wraps
  the in-process Rust backend, so the exit test is real: the Rust backend
  through the process host is byte-identical to the in-process path over every
  corpus package (D-P1). The test runs under `just test` with no network and no
  installation.
- `docs/book/cli-reference.md` gains the flag and the plugin lookup rule;
  `docs/design/` gains `codegen-plugins.md`, the as-built record; ADR-0020
  decision 11 is amended in place with a dated note per D-P1; E4.5b's row on the
  roadmap and #322 are corrected per D-P1 in both clauses, the `Done when` and
  "both in-tree backends ported onto it", since P4 ports Rust alone and the
  other backends follow in their own stories.
- Must not break: every existing emit; `just gate-parity`; `just wasm-check`
  (the model crate must build for wasm32 with `--no-default-features` if it is
  in `ridl-ir`).

### P4 — the Rust backend onto the model (L, three pull requests)

The backend stops reading raw IR, one layer at a time, each byte-identical
against the existing snapshots in `crates/ridl-backend-rust/src/snapshots/`,
`crates/ridlc/tests/snapshots/` and the checked-in generated face fixture:

1. Domain types (`lib.rs`'s `emit_decl` family, `defaults.rs`).
2. The FlatBuffers codec (`codec.rs`), which already reads the projection facts,
   so mostly the resolution of references through the model.
3. The descriptors and the face (`descriptors.rs`, `face.rs`, `clauses.rs`).

A fact the backend needs that the model lacks is a model change in the same pull
request, with the model snapshot moved and the reason stated. When the third
lands, the drift test of P2 is trivially total for Rust and is deleted for that
backend. The other three backends stay on the raw IR until their own stories;
note that in `codegen-plugins.md`.

### P5 — E11.1, the logical frame specification (M, docs; independent of P2 to P4)

`docs/specification/` gains the frame specification the roadmap describes: a
logical frame, binding-agnostic, stating per interaction kind what crosses a
boundary (ordinal, kind, envelope, provenance, correlation, payload), the
invalid-payload behaviour, and the rule that a binding is written from this
document alone. It names two bindings by their stories: WebSocket (E11.9) and
AIDL over Binder (the Kotlin backend). `docs/ROADMAP.md`'s E11.1 row is marked
landed; #257 closed by comment. Reuse `ridl-rt`'s `Envelope`, `Provenance` and
`Correlation` as the vocabulary; do not define a wire format.

## 5. After the lane, outside this repository

Not this plan's to execute, stated so the model and the frame are built for it:
a hand-written Kotlin runtime library (ports, envelope, provenance, FlatBuffers
reading through `flatc --kotlin` over the emitted `.fbs`), and
`ridlc-gen-kotlin`, a JVM launcher over the P3 contract emitting value classes
with checked constructors, the codec's typl checks, the faces and the AIDL
binding. The first cross-language demo is a Kotlin consumer against a Rust
provider over Binder, which needs O-P3.

## 6. Stop conditions

Stop and report, with the exact conflict: a decision above disagrees with
ADR-0014, ADR-0020 or ADR-0023 in a way those records do not already allow; a P4
layer cannot be made byte-identical without changing emitted output; the process
host needs a capability ADR-0020 decision 9 rules out (a plugin reading or
writing files); a gate member fails after two fix rounds; or a disposition on
O-P1 or O-P2 has not arrived and the next stage depends on it. Do not widen a
stage to get past a stop.
