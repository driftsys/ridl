# Streams stage 3 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give the generated Rust face a method for a query whose reply is a
stream of named elements: a caller-side `<Name>Stream` with a pull API, a
`Provider` associated type that returns a `ridl_rt::face::Source`, and a `serve`
arm that drives that source over the stream port of `ridl-rt`.

**Architecture:** `ridl-rt` gains `face::Source` and `face::Interrupted` (core
only, no `std`). The Rust backend stops refusing a query whose reply is
`StreamType.named` (driftsys/ridl#943 for that form only); the face emits
`Client::<name>` returning `<Name>Call`, which resolves to `<Name>Stream` over
`StreamCaller`; `Provider` gains
`type <Name>: Source<Item = Result<T, Interrupted>>` and `serve` gains an arm
that pushes each element through `StreamHandler`. The
`StreamCaller`/`StreamHandler` bounds appear only on an interface that has a
stream member (R-32). The veh-cluster corpus gains `interface FaultLog` and the
loopback round-trip tests run over it.

**Tech Stack:** Rust (edition 2021, `rust-version` 1.88 for `ridl-rt`), the Rust
backend's text emitters in `crates/ridl-backend-rust/src/face.rs` and
`src/face/`, `cargo xtask freeze`.

**Spec:** `docs/wip/2026-10-31-streams-design.md` — section 7 (first row of
7.3), R-20, R-22 as amended below, R-24, R-32, and the stage 3 entry of section
9. Ruling numbers (R-n) are that document's; S3-n are this plan's.

## Rulings taken in this plan

- **S3-1. Scope cut (owner, 2026-11-01): no `blocking` stream method and no
  `futures-io` / `futures-core` features.** This amends R-22 and R-34: stage 3
  ships the `no_std` pull API (`poll_next`) and one `std` `async fn next`. The
  `blocking` module gains nothing for a stream member and the emitted manifest
  gains no feature. Deferred until a consumer asks. The design's tests "the
  `blocking` client reads the same stream" and "`face_compile.rs` compiles the
  crate with and without the two features" are dropped.
- **S3-2. `Source::poll_next` yields
  `Poll<Option<Result<Self::Item, Interrupted>>>`.** The design's section 7.1
  writes `Poll<Option<Self::Item>>`, which gives a provider no way to interrupt
  a stream, while the stage 3 tests require "a provider that interrupts after 3
  elements gives the consumer 3 elements then `Transport::Interrupted`".
  `ByteSource` already returns `Result<_, Interrupted>`; the named form follows
  it. `serve` answers `Err(Interrupted)` with
  `abort_claim(.., CallError::Transport(Interrupted))`. `Consumer`, `ByteSource`
  and `ByteConsumer` stay in stage 5.
- **S3-3. Review:** per-task gate, then `/review` passes 1 and 2 over the
  branch; no separate final whole-branch review. `/review`'s deep seat runs on
  Codex (`~/.claude/commands/review.md` step 4).

## Global Constraints

- `ridl-rt` stays sans-IO and `no_std`; `face::Source` uses `core::task` only.
  `ridl-rt` keeps edition 2021 and `rust-version = "1.88"`. The `ridl-rt` change
  is additive (`runtime-crates` freeze row adds lines only).
- The IR, the catalog descriptor and every catalog hash are unchanged.
- No diagnostic is added or changed (ADR-0032 decision 14).
- Only `query f(..): <T>` with `T` named is accepted. Every other stream form
  (byte, text, stream parameter) stays refused exactly as today, with today's
  message.
- `StreamCaller` / `StreamHandler` bounds are emitted only for an interface that
  declares a stream member; a signal-only or call-only `Client` still builds
  over a port without the stream extensions (R-32).
- `ensure` is not evaluated on a streamed reply; `require` is evaluated as for
  any query. Each element is verified against its type on the sending side
  before `push_claim`; a failure settles `Contract(InvalidValue)`, which ends
  the stream (ridl §12.4).
- At most one `Source` poll per claim per `serve` poll counts against
  `SERVE_BUDGET`.
- Dropping a `<Name>Stream` calls `forget`, which interrupts (R-5). The call
  future resolves `Ok(stream)` on the first chunk or end, `Err(Timeout)` at the
  bound, `Err(e)` on `Failed(e)` before any chunk; the first chunk is kept in
  the stream's buffer and returned by the first `poll_next`.
- The generated crate allocates nothing for a stream: the element buffer is
  inline (`[u8; <T>::BUFFER]` or the member's limit, whichever the existing
  reply path uses).
- Shipped docs and rustdoc name no story id, stage or epic. Commits follow
  `.git-std.toml`; scopes used here: `ridl-rt`, `ridl-backend-rust`, `ridlc`,
  `ridl-loopback`, `xtask`, `docs`.
- Implementers run focused crate tests only; the full gate runs in CI.
- When a task changes generated output, run `cargo xtask freeze update` and
  commit the refreshed dumps in the same task; `cargo xtask freeze check` must
  pass at the end of each task. Every dump change must add lines only.

## Review Focus

- The caller future and stream: the first chunk is not lost, `Pending` registers
  the waker through `Interest`, drop interrupts exactly once, a `Short` read
  never reaches the application.
- `serve`: budget accounting, the order verify → push, the window-full path
  (`PushError` that means "full" leaves the element for the next poll and does
  not drop it), `Ready(None)` → `end_claim`, `Err(Interrupted)` → `abort_claim`.
- R-32 bounds: a fixture interface without a stream member must not gain a
  stream bound.
- Mutation in the reviews of Tasks 1 and 2: delete the kept-first-chunk path,
  the drop-forget, the verify before push, the window-full retention; each must
  fail a test.

### Task 1: `face::Source`, `face::Interrupted`, and the caller-side `<Name>Stream`

Model: Fable. Files: `crates/ridl-rt/src/face.rs`,
`crates/ridl-backend-rust/src/descriptors.rs`,
`crates/ridl-backend-rust/src/face.rs`,
`crates/ridl-backend-rust/src/face/futures.rs` (and `poll.rs` if the caller poll
helpers live there), `crates/ridl-backend-rust/tests/face_generation.rs`, a new
test fixture under `crates/ridl-backend-rust/tests/fixtures/` with one interface
with a streamed reply of a named struct and one interface without a stream.

- [ ] Write a failing `face_generation.rs` test: the fixture's face emits
      `Client::<name>(&mut self, ..) -> <Name>Call<'_, P>`, a `<Name>Stream`
      with `poll_next` and `async fn next`, and the `StreamCaller` bound only on
      the streaming interface's `Client`.
- [ ] Add `face::Source` (S3-2 signature) and `face::Interrupted` to `ridl-rt`
      with rustdoc; a unit test that a hand-written `Source` is object-safe
      enough to be driven by a generic function in `no_std` code.
- [ ] `descriptors::query_reply_type` (and the face's member filter) accept
      `StreamType.named`; every other stream form keeps today's refusal and
      message.
- [ ] Emit `<Name>Call` (future) and `<Name>Stream` per section 7.2 and the
      Global Constraints above. The `blocking` module emits nothing for the
      member (S3-1).
- [ ] Caller-side behaviour tests against `ridl-loopback` with a hand-driven
      provider written against the port (the `serve` arm is Task 2): order of
      elements, end, first chunk kept, drop interrupts, `Failed` before any
      chunk resolves the call `Err`.
- [ ] `cargo test -p ridl-rt -p ridl-backend-rust`, `cargo xtask freeze update`
      if generated output changed (the cabin face in the frozen veh-cluster copy
      gains `stream_filter_history`), `cargo xtask freeze check`.
- [ ] Commit: `feat(ridl-rt): add face::Source and face::Interrupted` and
      `feat(ridl-backend-rust): emit the caller side of a streamed reply`.

### Task 2: the `Provider` associated type and the `serve` arm

Model: Fable. Files: `crates/ridl-backend-rust/src/face.rs`,
`crates/ridl-backend-rust/src/face/serve.rs`, `face/dispatch.rs` if the dispatch
table lists members, `tests/face_generation.rs`, `tests/interaction_face.rs` (or
a new test file if the existing one is built over a fixed fixture).

- [ ] Failing `face_generation.rs` test: `Provider` gains
      `type <Name>: ridl_rt::face::Source<Item = Result<T, ridl_rt::face::Interrupted>>;`
      and `fn <name>(&mut self, ..) -> Self::<Name>;`, and the `StreamHandler`
      bound appears only on the streaming interface's `serve`.
- [ ] Emit the associated type, the method and the `serve` arm per section 7.1
      and the Global Constraints.
- [ ] Behaviour tests over `ridl-loopback`, generated provider and generated
      client end to end: a source returning `Pending` twice before each element;
      a source that interrupts after 3 elements gives the consumer 3 elements
      then `Transport::Interrupted`; a dropped `<Name>Stream` makes the
      provider's next push fail with `Transport(Interrupted)` and `serve` drops
      the source; an element outside its range ends the stream with
      `Contract(InvalidValue)` before it is sent; an element whose encoding is
      above a lowered `[limits.interaction]` entry is refused `TooLarge` and the
      consumer reads `Interrupted` (R-11); more elements than the loopback
      window (4) arrive in order.
- [ ] `cargo test -p ridl-backend-rust -p ridl-loopback`, freeze update and
      check as in Task 1.
- [ ] Commit:
      `feat(ridl-backend-rust): serve a streamed reply from a Provider source`.

### Task 3: the veh-cluster `FaultLog` corpus, the freeze refresh, the round trip, the records

Model: Sonnet. Files:
`crates/ridlc/tests/corpus/veh-cluster/cluster/services.ridl` the frozen copy
`xtask/tests/freeze-fixtures/generated/veh-cluster/cluster/services.ridl`,
`freeze/`, `crates/ridl-backend-rust/tests/interaction_face.rs`,
`docs/design/interaction-face.md`, `docs/design/ridl-rt.md`,
`docs/wip/2026-10-31-streams-design.md` (its "As built" section).

- [ ] Add `union FaultResult` and
      `interface FaultLog { query faults(filter: DiagFilter): <FaultResult> @[..1s] }`
      to `services.ridl` (R-24); `getFaultPage` and `appendix-a.ridl` stay
      unchanged. `ridl check` on the corpus draws exactly the diagnostics it
      drew before.
- [ ] Refresh the frozen corpus copy and run `cargo xtask freeze update`; the
      `generated-crate` and `wire` rows add lines only (`FaultLog`,
      `FaultResult`); the cabin catalog hash line is unchanged.
- [ ] Loopback round trip over the generated veh-cluster crate: 64 `FaultResult`
      elements, about 68 KB in FlatBuffers (above one call limit), order and end
      checked.
- [ ] Records: `docs/design/interaction-face.md` (a streamed reply of named
      elements is now on the face; other stream forms stay refused,
      driftsys/ridl#943), `docs/design/ridl-rt.md` (`face::Source`,
      `face::Interrupted`), and the design's "As built" section (S3-1, S3-2 and
      every ruling an implementer recorded).
- [ ] `cargo test -p ridlc -p ridl-backend-rust`, `cargo xtask freeze check`,
      `just doc-path-check`, `just story-id-check`.
- [ ] Commits:
      `test(ridlc): add the FaultLog streamed reply to the veh-cluster corpus`,
      `docs(docs): record the streamed reply on the Rust face`.

## After the last task

`/review` pass 1 then pass 2 over the branch (Codex deep seat), one fix wave per
pass, then the owner's word on the pull request. `ridl-rt` is additive, so the
change can ship in any release after the breaking one that carries stages 1 and
2 (R-35).
