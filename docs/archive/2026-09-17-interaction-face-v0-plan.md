# Generated Interaction Face v0 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> `superpowers:subagent-driven-development` (recommended) or
> `superpowers:executing-plans` to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** Generate and compile an in-process Rust interaction face for the
approved Lane M fixture, including descriptors, `Client`, `Publisher`,
`Provider`, and total `dispatch` behavior over `ridl-rt`.

**Architecture:** Extend `ridl-backend-rust` with descriptor and
interaction-face emitters. The generated source remains generic over `ridl-rt`
payload and port traits; the test tree supplies a disposable loopback port
implementation and hand-written `Payload<ReprC>` implementations. The generated
fixture is checked in, included by the integration test, and protected by a
byte-equality regeneration test.

**Tech Stack:** Rust, `proc_macro2`, `quote`, `syn`, `prettyplease`, `ridl-ir`,
`ridl-rt`, Cargo integration tests, and the repository `just` gates.

---

## Files and responsibilities

The implementation is deliberately confined to the Rust backend and its tests.
No `ridl-rt` API or ADR changes are part of M3.

- Modify `crates/ridl-backend-rust/src/lib.rs`: register the two new emitter
  modules and add the `generate_face` companion entry point that appends their
  generated items to the existing package output. `generate` itself is
  unchanged; see "Where the face is emitted from" below.
- Create `crates/ridl-backend-rust/src/descriptors.rs`: emit package catalog
  data, one `ridl_rt::contract::Interface` implementation per interface, and one
  interaction descriptor per member. It also emits the generated maximum buffer
  constants.
- Create `crates/ridl-backend-rust/src/face.rs`: emit `Client`, `Publisher`,
  `Provider`, interaction methods, and total `dispatch` code.
- Create `crates/ridl-backend-rust/tests/fixtures/interaction_face.ridl`: the
  single-catalog fixture with two interfaces and one-parameter calls.
- Create `crates/ridl-backend-rust/tests/generated/interaction_face.rs`: the
  checked-in output of `generate` for the fixture. It is never hand-edited.
- Create `crates/ridl-backend-rust/tests/support/loopback.rs`: the disposable,
  test-only implementation of the required `ridl-rt` ports. It must not
  implement `ScannableSignals` or `CoherentSignals`.
- Create `crates/ridl-backend-rust/tests/interaction_face.rs`: include the
  generated module with hand-written outer lint allows, define the throwaway
  `Payload<ReprC>` implementations, exercise the round trip, and test the
  regeneration guard.
- Modify `crates/ridl-backend-rust/Cargo.toml`: add the in-tree `ridl-rt`,
  `ridlc`, and `ridl-core` dev-dependencies needed to compile the generated
  integration test and turn the `.ridl` fixture into IR. Follow the sibling
  proto backend's documented dev-dependency-cycle exception.

The fixture intentionally does not add a `fixed` interaction. The generated
emitter still covers the `Fixed` descriptor and `Publisher`/`FixedReader`
surface, but adding a fixed member would require more placeholder-port and
payload plumbing without increasing the MVP's round-trip evidence. This decision
keeps the fixture within the spec's fixed-size scalar, enum, and struct
stand-in.

The plan originally left the remaining three settlement rows without dedicated
tests: the `require` and `ensure` rows were tested because they exercise
generated contract clauses, while unknown routing, malformed bytes, and invalid
payload verification were left to the runtime/codec tests, covered by the
`ridl-rt` contracts. That scope was later revisited: every row of the settlement
table now has a runtime test through `dispatch`, in
`crates/ridl-backend-rust/tests/interaction_face.rs`. The dispatch unit tests
still assert the explicit `Capacity` and `SettleError` decisions below.

Multi-parameter calls and induced argument structs are a recorded follow-up, not
M3 work. M3 emits no induced argument type. Every command and query in the
fixture has exactly one declared argument type and every query has one declared
reply type.

## Settled M2 decisions

### Capacity during dispatch

The dispatch buffer is caller-owned and must be at least the generated maximum
of every argument and reply `<Payload as Payload<ReprC>>::MAX_SIZE` used by the
interface. Dispatch checks that precondition before asking the handler for a
claim. With a short caller buffer it returns `0` without consuming a claim; the
caller can retry with the generated `MAX_BUFFER_SIZE`. A legal provider value
cannot produce `EncodeError::Capacity` once this precondition holds. If it does,
the provider has violated the generated type's value invariant or supplied a
manually written `Payload` implementation that does not honor `MAX_SIZE`.

`dispatch` treats this as an internal provider/programming error, not as one of
the five protocol outcomes. It must not manufacture a `Contract::InvalidValue`:
`EncodeError::Capacity` has no received bytes and no `Violation` rule to report,
and the caller did not send an invalid value. The generated branch calls
`unreachable!` with the type name, needed size, and available size. This
preserves the distinction between a provider-side implementation defect and
`Transport::Corrupt` or a contract error. A source-text assertion in
`dispatch_generation.rs` proves the branch is explicit — the generated code
names the type, the needed size, and the available size and calls `unreachable!`
rather than falling through; the normal round trip proves the generated
constants make the branch unreachable for legal values. A runtime proof that
drives the branch itself is not implemented; see "Follow-ups recorded, not
implemented".

The hand-written `Payload<ReprC>::verify` implementations return
`VerifyError::Contract(Violation { type_name, rule })` for constraint failures,
as settled by #404. They never return a generated domain error type.

### `SettleError` and the returned count

`dispatch` increments its return value only after `Handler::settle` returns
`Ok(())`. A `SettleError` is not converted into another `CallError`, because the
handler already owns settlement of that claim and the function has no error
channel. Dispatch records no false success, continues to the next claim, and
returns the count of successful settlements. The test-support handler injects
one settlement failure followed by one successful claim and asserts that the
result is `1`.

### Buffer constants

Each generated interface has a public argument/reply buffer constant whose value
is the maximum of all argument and reply `MAX_SIZE` values. Replies are encoded
into the same dispatch buffer, so considering arguments alone is incorrect. Each
generated interface also has a separate public event-source constant, computed
as the maximum `MAX_SIZE` across event payloads only; this is the size required
by `EventSource::next`, whose payload type is discovered from
`RawOccurrence::ord`.

### Include and lint policy

The generated file must contain no inner attributes. In particular, the emitter
must never emit `#![allow(...)]`; it parses a bare `syn::File` and continues to
do so after this change. The handwritten integration-test wrapper uses outer
module attributes such as
`#[allow(clippy::...)] mod generated { include!(...); }`. Any lint allow is
therefore visible at the test call site and cannot silently affect consumers of
generated code. If the generated face requires broad or numerous allows, the
emitter is fixed instead of expanding the wrapper.

### Contract clause bodies — a narrow, total translator

Settled during M3, on delegated authority, because neither M1 nor M2 named a
mechanism for it and the work cannot proceed without one.

Design §6 says the emitter writes `ensure` for every query, "returning `Ok(())`
when the query declares no `ensure` clause and the translated clauses when it
does"; it says nothing about `require`'s body. This plan's own Task 2 states the
analogous no-clause rule for `require`: "`require`/`ensure` implementations
returning `Ok(())` when no clause exists". Executing that showed there is
nothing to translate from. The IR carries a clause as canonical **ridl** text in
`Contract.source` — `window > 0ms`,
`position != GearPosition.PARK || currentSpeed == 0.0` — and not as an
expression tree; `E5.1` is the story that replaces the text with one. No backend
translates an expression, and no backend depends on `ridl-sem`, so
`ridl_sem::expr_eval::parse_contract_expr` is out of reach: a backend consumes
IR (ADR-0020 decision 7). The text is also not Rust. A named scalar is a
`#[repr(transparent)]` newtype with no comparison implementations, so
`speed <= 100` does not compile, and `0ms` and `GearPosition.PARK` are not Rust
at any layer.

**Decision. The Rust backend gets a clause translator that accepts one
expression form and refuses every other, and the fixture's clauses stay inside
that form.**

- The accepted form is `<subject> <comparison> <numeric literal>`, where
  `<subject>` is the interaction's single declared parameter, or `result` on an
  `ensure` clause, and `<comparison>` is one of `<`, `<=`, `>`, `>=`, `==`,
  `!=`. The subject's type must be a named scalar over an integer or a float.
- It emits `args.0 <op> <literal>` for a parameter and `reply.0 <op> <literal>`
  for `result`, reaching the newtype's public field. Several clauses of one kind
  are conjoined: every clause must hold.
- A command or a query that declares no clause of a kind emits a body of
  `Ok(())`, as the plan already said.
- **Any other clause form is refused with a `GenerateError`.** It is never
  silently dropped and never emitted as `Ok(())`. Dropping a clause would
  generate a provider that accepts arguments its contract forbids, which is a
  worse failure than refusing to generate. `generate` is already total over
  errors this way, so the refusal costs no new mechanism.

This is deliberately the smallest thing that makes the two clause settlement
rows real. It is not a first instalment of E5 and must not grow into one: a
clause the form cannot carry is a reason to simplify the fixture, exactly as §2
says about the payload stand-in. E5.1 replaces the translator with one driven by
the structured expression tree, and the generated doc comment says so.

### Where the face is emitted from — a companion entry point, not the pipeline

Settled during M3, on delegated authority, after Task 1 found that the
integration the design sketches cannot leave `just build` green.

Design §7 says `lib.rs` gains "two module declarations and the call that appends
the new items to what `generate` already returns", and this plan's Task 5 says
the byte guard "runs `generate` over the fixture". Executing that showed
`generate` is the shared pipeline entry point, and two of `crates/ridlc`'s tests
bind what it may emit:

1. `corpus_entries_compile_to_reviewed_snapshots` calls
   `ridl_backend_rust::generate(ir).expect(...)` over every clean corpus entry.
   The corpus interfaces carry contract clauses the narrow translator must
   refuse — `window > 0ms`, `result >= 0.0`, `level < HANDLE_MAX`,
   `position != GearPosition.PARK || currentSpeed == 0.0`. Refusing them is the
   translator's whole point, so folding it into `generate` turns every one of
   those entries into a `GenerateError` and panics the corpus test.
2. `rustc_accepts` compiles generated corpus output with `rustc` and passes no
   `--extern`. Emitting `::ridl_rt::…` from `generate` fails it with `E0433`.

The second is not M3's to change. Lane C's Epic 10 Task 3 owns it: its plan
states "Task 3 is where generated code first names the runtime, and where the
compile proofs first pass `--extern ridl_rt=<path>`", and it deliberately
withholds the flag until then so a proof whose generated source wrongly names
`ridl_rt` is still detected. M3 taking that flag would pre-empt an in-flight
lane and remove its detector.

**Decision. The face is emitted from a companion entry point. `generate` is left
byte-identical.**

- `generate(package)` keeps today's contract exactly: the domain types, naming
  no runtime.
- `generate_face(package)` emits what `generate` emits plus the descriptor and
  face items. It is what Task 5's checked-in fixture and byte guard run, and the
  only caller of the clause translator. The domain types must come from the same
  call, because the checked-in file is brought in with one `include!`: the face
  names those types, and the orphan rule needs them local to the test crate for
  the hand-written `Payload<ReprC>` implementations.
- The doc comment on both says why there are two, and that the pipeline keeps
  the narrow one until Lane C's Epic 10 Task 3 lands the runtime naming and the
  `--extern ridl_rt` proof.

This follows the convention the repository already pinned for exactly this
situation. ADR-0017 decision 1 gave the proto backend
`generate_with(package,
others)` with "`generate(package)` retained as
`generate_with(package, &[])`", and recorded that later backends inherit the
API. A companion entry point is also the less committed choice against ADR-0020
decision 7, which replaces a backend's entry point with
`generate(CodegenRequest) -> CodegenResponse`: less is wired into a signature
that record already schedules for replacement.

The cost is that `ridl --emit rust` does not yet emit the face. That is correct
for this stage rather than a shortfall — ADR-0018 decision 15 makes the face
phase 2, and nothing in M3 ships a runtime for a pipeline consumer to link
against.

### The `Provider` argument is taken by reference

Settled during M3, on delegated authority, because the design note's §6 cannot
be implemented as literally written.

§6's paragraph "The `Provider` method signatures, settled here" argues about the
**return** type: `Rejected` is not a type `ridl-rt` has, and a command has no
failure the application reports, so a command returns nothing and a query
returns its declared reply. That argument is kept exactly. The by-value
parameters in the paragraph's two example signatures were incidental to it and
were never reasoned about.

By value does not work. §6's own settlement table requires `dispatch` to call a
query's `ensure` **after** the provider returns, and the M3 clause translator
emits `args.0 <op> <literal>` for a clause over a parameter, so `dispatch` must
still hold the arguments at that point. A provider that took them by value would
have moved them, and the generated payload types implement neither `Copy` nor
`Clone`.

**Decision. A `Provider` method takes its argument by reference:
`fn set_target(&mut self, desired: &Speed);` and
`fn average_speed(&mut self, window: &Duration) -> Speed;`.** The return types
are what §6 settled. §6's example signatures are superseded on the parameter
only, and gardening reconciles the note.

### The consumer-side call reports `SendError`, not `CallError`

Settled during M3, on delegated authority, because §6 left it open.

§6 fixes the `Client` shape — one method per consumer-side interaction, a query
returning a `Correlation` polled by a separate `*_reply` method — but never
names the error type a client method returns. The `CallError` vocabulary in §6
belongs to the settlement table, which is the provider side: it is what
`dispatch` settles, not what a caller's send returns.

**Decision. A consumer-side call returns `Result<Correlation, SendError>`.**
`SendError` is what the `Caller` port itself returns and it already carries
`Contract`, so a failed client-side `require` is reported as
`SendError::Contract(Contract::PreconditionFailed)` with no lossy mapping
between two error types. Mapping it into `CallError` would invent a conversion
no port performs.

This closes a gap rather than overriding a decision.

### Epic 10 risk

`crates/ridl-backend-rust/src/lib.rs` is shared with Lane C's Epic 10. The
expected order remains C4 then M3. Starting M3 before C4 is allowed only when no
open Lane C pull request holds the file; it incurs accepted rework. Epic 10 Task
3 or Task 6 may require a touch-up to generated domain types and the
hand-written payload implementations. This is a risk in M3, not a blocker or an
ADR change. If Epic 10 lands first, rebase before implementation and expect no
design change.

## Task 1: Add failing descriptor and size-constant tests

**Files:**

- Modify: `crates/ridl-backend-rust/src/lib.rs`
- Create: `crates/ridl-backend-rust/src/descriptors.rs`
- Create: `crates/ridl-backend-rust/src/clauses.rs`
- Create: `crates/ridl-backend-rust/tests/fixtures/interaction_face.ridl`
- Create: `crates/ridl-backend-rust/tests/descriptor_generation.rs`
- Create: `crates/ridl-backend-rust/tests/support/ir.rs`
- Modify: `crates/ridl-backend-rust/Cargo.toml`

**Implementer:** Sonnet for emitter plumbing; Opus reviews the generated
descriptor shape.

- [ ] **Step 1: Write the failing test.** Add the source-to-IR helper in
      `tests/support/ir.rs` using `ridlc::compile`, then parse the fixture
      through that helper and assert that generated text contains an `Interface`
      implementation with `CATALOG`, `NUMBER`, `PROVISIONAL`, `NAME`, `MEMBERS`,
      `Interaction::Iface`, `Interaction::MEMBER`, and all five interaction-kind
      implementations. Assert that the interface constant is the maximum
      argument/reply size and that the event-source constant is the maximum
      event size.
- [ ] **Step 2: Run the focused test and verify failure.** Run:
      `cargo test -p ridl-backend-rust --test descriptor_generation` Expected:
      compilation or assertion failure because the descriptor module and
      generated interaction items do not exist.
- [ ] **Step 3: Add the fixture and dependencies, then implement the minimum
      descriptor emitter.** Add the two-interface, one-parameter fixture at the
      path above and add the `ridlc`/`ridl-core` dev-dependencies documented by
      the sibling proto backend. Then add helpers that map IR names through the
      existing identifier escaping helper, emit the package `CatalogRef` with
      `CatalogHash([0; 32])`, emit the provisional number from the IR without
      inventing a number, and emit `EncodedSizes` with `None` in all three
      columns. Add generated doc comments stating that the zero catalog hash is
      an E16.2 placeholder and that `None` means the toolchain cannot size the
      payload yet, rather than asserting that the encoding cannot carry it. Emit
      one member row per declared interaction, in ordinal order, with two
      payload rows for a query and one for every other member. Emit
      `MAX_BUFFER_SIZE` from argument and reply maxima and
      `EVENT_SOURCE_BUFFER_SIZE` from event maxima. Do not emit an inner
      attribute. Add `clauses.rs`, the narrow clause translator settled above,
      and emit `require` and `ensure` bodies from it: `Ok(())` when the
      interaction declares no clause of that kind, the conjunction of the
      translated clauses when it does, and a `GenerateError` when a clause is
      outside the accepted form. Unit-test the translator's accepted form, its
      conjunction, and its refusal directly.
- [ ] **Step 4: Run the focused test and verify it passes.** Run:
      `cargo test -p ridl-backend-rust --test descriptor_generation` Expected:
      PASS.
- [ ] **Step 5: Run the full gate before committing.** Run: `just build`
      Expected: the repository gate is green.
- [ ] **Step 6: Commit after checking the branch.** Run:
      `git branch --show-current` Expected: `docs/interaction-face-plan` only
      while this plan is being implemented by a future M3 branch; the
      implementer must use its assigned implementation branch. Then commit the
      task with a Conventional Commit.

## Task 2: Add failing tests for generated `Client`, `Publisher`, and provider traits

**Files:**

- Create: `crates/ridl-backend-rust/src/face.rs`
- Modify: `crates/ridl-backend-rust/src/lib.rs`
- Create: `crates/ridl-backend-rust/tests/face_generation.rs`

**Implementer:** Opus.

- [ ] **Step 1: Write the failing tests.** Assert that the first fixture
      interface generates:
  - a `Client<'a, P>` bound only by the ports required by its declared signal,
    event, command, and query;
  - signal reads that call `SignalReader::read`, event subscription and polling
    that call `EventSource`, command and query methods that call `Caller`;
  - a `Publisher<'a, W>` over `SignalWriter` and `EventSink`, including
    `invalidate_*` and `commit`;
  - a `Provider` method for the command returning `()`, a query method returning
    its declared reply, and generated `require`/`ensure` methods. Add a minimal
    signal-only port type implementing only `Attached` and `SignalReader`;
    compile a signal-only client with it to prove no unnecessary `Caller` or
    `EventSource` bound is emitted.
- [ ] **Step 2: Run the focused test and verify failure.** Run:
      `cargo test -p ridl-backend-rust --test face_generation` Expected: FAIL
      because no face items are emitted.
- [ ] **Step 3: Register and implement the generated face surface.** Declare
      `mod face;` in `lib.rs`, append its items after the descriptor items, and
      emit one client method per consumer interaction, one publisher method per
      provider-side signal/event, `Provider` methods with the settled
      signatures, and `require`/`ensure` implementations returning `Ok(())` when
      no clause exists. Compute generic bounds from the interface's actual
      kinds, not from a common superset. Keep every method non-blocking and
      avoid threads, futures, sockets, timers, or `CoherentSignals`.
- [ ] **Step 4: Run the focused test and verify it passes.** Run:
      `cargo test -p ridl-backend-rust --test face_generation` Expected: PASS.
- [ ] **Step 5: Run `just build`.** Expected: all repository gates pass.
- [ ] **Step 6: Commit after `git branch --show-current`.**

## Task 3: Add failing dispatch settlement tests

**Files:**

- Modify: `crates/ridl-backend-rust/src/face.rs`
- Create: `crates/ridl-backend-rust/tests/dispatch_generation.rs`

**Implementer:** Opus.

- [ ] **Step 1: Write failing dispatch-generation tests.** Assert in the
      generated source that
      `dispatch<H, P>(h: &mut H, p: &mut P, buf: &mut
      [u8]) -> usize`
      checks the caller-owned buffer before polling, matches unknown interface
      and ordinal values, maps the two `VerifyError` variants, evaluates
      `require`, calls the provider, evaluates query `ensure`, maps
      `EncodeError::Capacity` to the explicit invariant branch, and increments
      its result only after successful settlement. Assert that it uses the
      argument/reply maximum and that the event source uses its separate
      event-only constant. Runtime behavior is tested after the include and
      loopback harness exist in Task 5.
- [ ] **Step 2: Run the focused test and verify failure.** Run:
      `cargo test -p ridl-backend-rust --test dispatch_generation` Expected:
      FAIL because `dispatch` is not generated.
- [ ] **Step 3: Implement total dispatch.** Match on catalog interface number
      and ordinal, with an explicit unknown-interaction arm. Return `0` before
      polling when `buf.len() < MAX_BUFFER_SIZE`, leaving the claim available
      for a retry with a correctly sized buffer. Then call
      `Handler::next_claim`; map `VerifyError::Structure` to
      `CallError::Transport(Transport::Corrupt)` and `VerifyError::Contract(v)`
      to `CallError::Contract(Contract::InvalidValue(v))`; map failed `require`
      to `PreconditionFailed`; call the provider; call query `ensure`; map
      failed `ensure` to `ContractBroken`; and settle each claim. Use the
      generated maximum buffer. Treat `EncodeError::Capacity` as the explicit
      `unreachable!` provider invariant failure described above. Increment the
      return count only after successful settlement and continue after a
      `SettleError`.
- [ ] **Step 4: Run the focused test and verify it passes.** Run:
      `cargo test -p ridl-backend-rust --test dispatch_generation` Expected:
      PASS.
- [ ] **Step 5: Run `just build`.** Expected: all repository gates pass.
- [ ] **Step 6: Commit after `git branch --show-current`.**

## Task 4: Add the disposable in-process ports test first

**Files:**

- Create: `crates/ridl-backend-rust/tests/support/loopback.rs`
- Create: `crates/ridl-backend-rust/tests/support/mod.rs`
- Create: `crates/ridl-backend-rust/tests/interaction_face.rs`
- Modify: `crates/ridl-backend-rust/Cargo.toml`

**Implementer:** Sonnet.

- [ ] **Step 1: Write the failing support test and test target.** Create the
      integration target with `mod support;`, then add a support-module test
      showing that a command sent through `Caller` is returned by
      `Handler::next_claim`, and that a successful settlement is observable by
      `Caller::ack`; add the analogous query/reply assertion. Add signal and
      event queue assertions. Add an assertion that the support port advances
      its hand-driven clock and does not require wall-clock time.
- [ ] **Step 2: Run the focused support test and verify failure.** Run:
      `cargo test -p ridl-backend-rust --test interaction_face support`
      Expected: FAIL in the support assertions because the loopback traits are
      not implemented; the test target itself must be discovered.
- [ ] **Step 3: Add the `ridl-rt` dev-dependency and implement the throwaway
      loopback.** Implement exactly the required `Attached`, `Clock`,
      `SignalReader`, `SignalWriter`, `EventSource`, `EventSink`, `Caller`,
      `Handler`, and `FixedReader` traits over in-memory queues/maps. Do not
      implement `ScannableSignals` or `CoherentSignals`. Keep it in the test
      tree, document that E11.9 replaces it, and make no runtime or transport
      claim for it.
- [ ] **Step 4: Run the focused support test and verify it passes.** Expected:
      PASS.
- [ ] **Step 5: Run `just build`.** Expected: all repository gates pass.
- [ ] **Step 6: Commit after `git branch --show-current`.**

## Task 5: Add the compiled fixture, payload stand-in, and byte guard

**Files:**

- Create: `crates/ridl-backend-rust/tests/generated/interaction_face.rs`
- Modify: `crates/ridl-backend-rust/tests/interaction_face.rs`
- Modify: `crates/ridl-backend-rust/tests/support/mod.rs`
- Modify: `crates/ridl-backend-rust/Cargo.toml`

**Implementer:** Sonnet for checked-in output and test boilerplate; Opus reviews
payload verification and the generated face behavior.

- [ ] **Step 1: Write failing integration tests.** Extend the existing target
      and include the generated file under a handwritten module with only outer
      lint allows. Define `Payload<ReprC>` for the fixture's fixed-width scalar,
      enum, and struct types. Test signal publish/read, event raise/receive,
      command acknowledgment, query reply, failing `require`, and failing
      `ensure`. The verification implementation must return
      `VerifyError::Contract(Violation { type_name, rule })` for an invalid
      value, and `VerifyError::Structure(...)` for malformed bytes. Add a test
      that regenerates the fixture with the existing parser and compares bytes
      to `tests/generated/interaction_face.rs`, with failure text:
      `generated interaction_face.rs is stale; regenerate it with RIDL_UPDATE_GENERATED=1 cargo test -p ridl-backend-rust --test interaction_face`.
- [ ] **Step 2: Run the focused integration test and verify failure.** Run:
      `cargo test -p ridl-backend-rust --test interaction_face` Expected: FAIL
      because the generated file and payload implementations are absent.
- [ ] **Step 3: Generate and check in the fixture output.** Use the backend's
      existing test support, not a new cargo subprocess. When
      `RIDL_UPDATE_GENERATED=1` is set, write the exact `generate_face` output
      to the checked-in file; otherwise compare it byte-for-byte. Keep the
      generated file free of hand-written edits and inner attributes.
- [ ] **Step 4: Run the focused integration test and verify it passes.** Run:
      `cargo test -p ridl-backend-rust --test interaction_face` Expected: PASS,
      including compilation of the included generated code and the round-trip
      assertions.
- [ ] **Step 5: Run `cargo fmt --all` and the full gate.** Run:
      `cargo fmt --all && just build` Expected: formatting is clean and all
      gates pass.
- [ ] **Step 6: Commit after `git branch --show-current`.**

## Task 6: Final verification and handoff

**Files:**

- No new files. Review all files listed above.

**Implementer:** Opus.

- [ ] **Step 1: Review the emitted API against the approved design.** Confirm
      the generated code uses the IR's provisional interface number and `true`
      provisional flag, emits the zero catalog hash only as the documented E16.2
      placeholder, emits all descriptor payload rows with `None` sizes, and
      never emits an inner attribute.
- [ ] **Step 2: Run focused tests together.** Run:
      `cargo test -p ridl-backend-rust --test descriptor_generation --test face_generation --test dispatch_generation --test interaction_face`
      Expected: PASS.
- [ ] **Step 3: Run repository verification.** Run: `just verify` Expected:
      commit lint, formatting, documentation, compilation, tests, clippy, wasm,
      compatibility, and connective-tissue checks all pass.
- [ ] **Step 4: Inspect the diff for scope.** Run:
      `git diff --check && git diff --stat origin/main...HEAD` Expected: only
      the backend implementation, its test fixture/support/output, and the
      backend manifest changed; no ADR, `ridl-rt`, roadmap, or unrelated lane
      file changed.
- [ ] **Step 5: Prepare the implementation pull request notes.** State that
      E11.9 replaces the test-only loopback and that a late Epic 10 merge may
      require the accepted generated-type touch-up. These notes make the
      replacement and rework visible to the next lane.
- [ ] **Step 6: Commit after checking the branch.** Run:
      `git branch --show-current` Expected: the assigned M3 branch, not `main`
      and not another lane's branch.

## Follow-ups recorded, not implemented

- Multi-parameter commands and queries need induced argument structs and a
  corresponding emitter worklist. The MVP intentionally keeps one declared
  argument type per call.
- E11.9 replaces `tests/support/loopback.rs` with the real in-process runtime.
- E11.7, E11.8, or E11.12 replaces the hand-written `Payload<ReprC>` impls.
- E16.2 replaces the all-zero `CatalogHash` and the all-`None` `EncodedSizes`
  rows.
- E5.1 replaces the narrow clause translator of `src/clauses.rs` with one driven
  by the structured expression tree, and widens the clause forms a contract may
  use.
- Lane C's Epic 10 may require a small generated-domain-type and payload
  stand-in touch-up if it lands after M3.
- The generated domain types draw two clippy lints when they are compiled in
  this repository for the first time: `derivable_impls` on the `Default`
  emission of `crate::defaults`, and `upper_case_acronyms` on an enum variant
  that keeps its typl SCREAMING_SNAKE spelling. Both predate M3 and neither
  comes from the face or the descriptors. M3 allows them on the handwritten
  wrapper module, with a `reason` on each, rather than changing the domain-type
  emitter from this lane. Whether the emitter should avoid them is a question
  for the lane that owns `crate::defaults`.
- A runtime proof of the `EncodeError::Capacity` to `unreachable!` branch in
  `dispatch`: a `Payload<ReprC>` implementation whose `MAX_SIZE` understates
  what it actually encodes, driving `dispatch` into that branch.
  `Cabin::MAX_BUFFER_SIZE` is computed from the same concrete
  `Payload::MAX_SIZE` constants that every other round-trip test in
  `interaction_face.rs` depends on, so an implementation that lies about
  `MAX_SIZE` for `Level`, `Window`, or `Average` would also shrink or corrupt
  the buffer those other tests share, and a value-conditional lie inside the
  same trait implementation would race under the test binary's default parallel
  execution. Driving this branch cleanly needs either a dedicated interface or
  type not shared with the other round-trip tests, or a way to substitute a
  `Payload` implementation per test; neither exists yet.
- A behavioural test of the command-settled-before / query-settled-after
  ordering that design §6 states. No behavioural test can observe this ordering
  today, because neither `Handler::settle` nor the provider call in
  `tests/support/loopback.rs`'s loopback harness has a side effect through which
  reordering the two would change final state; the ordering rests entirely on an
  exact-text assertion in `dispatch_generation.rs`. This is a limitation of the
  test double, not a defect in this change. E11.9's real in-process runtime is
  what would make the ordering observable.

## Self-review against the approved spec

- Scope, exclusions, `ReprC` stand-in, provisional numbering, catalog and size
  placeholders, generated shape, test-only ports, fixture, include strategy,
  lint rule, Epic 10 risk, roadmap/tracking context, and alternatives are
  covered by the task and decision sections above.
- The seven M2 deferrals are settled: capacity is an internal invariant failure;
  settlement counts successful claims only; the fixture omits `fixed`; every row
  of the settlement table now has a runtime test through `dispatch`;
  multi-parameter calls are a follow-up; lint allows live on the outer include
  wrapper and the emitter emits no inner attribute; Epic 10 is an accepted
  rework risk.
- Every implementation task starts with a failing test, names files, names an
  implementer model, and requires `just build` before its commit.
- No task changes ADR-0018, ADR-0020, or the approved M1 design.
