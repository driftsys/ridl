# The async face and the runtime substrate — implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give every generated interface an async `Client` and a blocking
`Client`, and a `serve` in both forms, over a `ridl-rt` substrate — a keyed wake
source, a correlation table, two composed error types — that every runtime with
asynchronous replies shares; then make the poll face `pub(crate)`.

**Architecture:** Every decision is in
[the design note](2026-09-25-async-face-design.md), F-1 to F-15, as disposed on
its pull request; the two ADR amendments that record them landed with this plan.
Where this plan and the note disagree, the note is authoritative. The plan adds
nothing the note did not decide: it names the files, the tests and the order.

**Tech stack:** Rust, `ridl-rt` (`no_std`, 1.83, edition 2021, no `unsafe`),
`proc-macro2`/`quote`/`prettyplease` in the Rust backend, the checked-in fixture
`crates/ridl-backend-rust/tests/generated/interaction_face.rs` regenerated with
`RIDL_UPDATE_GENERATED=1 cargo test -p ridl-backend-rust --test interaction_face`.

**Stages:** F3 is Tasks 1 and 2, F4's second half is Task 3, F5a is Task 4, the
release is between, F5b is Task 5, as the driver's §3 table has them, with one
change this plan's pull request writes into the driver: the poll face becomes
`pub(crate)` in F5a, not F5b, because an async method and a poll method of one
name cannot share a `Client`. Each task is one pull request, reviewed per the
driver's §0.

## Currency

Written 2026-09-25, refreshed 2026-09-26 against `origin/main` at e4507db. Two
things to know:

- **E11.20's first half** (stage F4) merged as 83214a1 (driftsys/ridl#531):
  `crates/ridl-rt-conformance` holds the port contract suite as functions
  generic over a factory, and `crates/ridl-loopback/tests/ports.rs` keeps the
  eleven tests only the loopback can express. Task 3 extends that crate's
  factory; the hook names below are the loopback's (`advance`,
  `fail_next_settle`) and are to be read as "the factory's hook for this".
- **E16.2** (driftsys/ridl#378) is not started. Nothing here waits on it; the
  loopback's byte budget and timed settlement wait on it (note F-9, §3).

## Global constraints

- **`ridl-rt` is `no_std`, allocates nothing, forbids `unsafe`, takes no
  dependency in any feature combination, and builds at 1.83 as edition 2021**
  (ADR-0021 decisions 8 and 10). Every item Tasks 1 and 2 add is checked by
  `just wasm-check` and `just compat-check`, which carry
  `-p ridl-rt
  --all-features` invocations.
- **A new public item in `ridl-rt` is an ADR-0021 decision.** The amendment that
  landed with this plan records every item Tasks 1 and 2 add; a task that needs
  one it does not record stops and amends the record first.
- **E11.16 to E11.19 ship as one 0.x minor**, tagged by Sebastien after Task 4
  merges. No task pushes a tag or publishes.
- **The generated face is pinned by exact text.**
  `tests/interaction_face.rs::generated_interaction_face_matches_the_emitter`
  compares the emitter's output with the checked-in fixture byte for byte;
  `face_generation.rs`, `dispatch_generation.rs` and `descriptor_generation.rs`
  assert whitespace-stripped substrings. A face change regenerates the fixture
  in the same commit and updates the substrings it breaks.
- **`just demo` is in `just build`**, and it builds and runs
  `examples/cabin/consumer` against the emitted crate with cargo. Task 4 makes
  the poll face `pub(crate)` and rewrites that program onto the async client in
  the same pull request; Task 5 adds the blocking client to it.
- **Conventional Commits**, scopes: `ridl-rt`, `ridl-loopback`,
  `ridl-backend-rust`, `ridlc`, `ridl` (the frame specification took `ridl` in
  ddd56fd), `docs`, `adr`, `roadmap`.
- **Never edit another lane's worktree**, and check for an open pull request
  touching `crates/ridl-backend-rust/src/face.rs` or the fixture before
  branching Task 4 or 5 (driver §8).
- **Run `just verify` before opening each pull request.** Individual steps run
  `just test` and `just lint`.
- **Prose is plain and literal.**

---

### Task 1: E11.16 — `Wakeable`, `Interest`, `Transport::Busy`; the loopback wakes

**Model:** Opus, effort high (driver §3, stage F3).

**Files:**

- Modify: `crates/ridl-rt/src/port.rs` — `Wakeable`, `Interest`, the two
  forwarding impls; `crates/ridl-rt/src/error.rs` — `Transport::Busy`;
  `crates/ridl-rt/src/lib.rs` — the crate rustdoc's port paragraph.
- Modify: `crates/ridl-loopback/src/lib.rs`, `handle.rs`, `store.rs` — every
  handle implements `Wakeable`; `CallerHandle` implements `Clock`; the store
  keeps one `Option<Waker>` per key kind per handle and wakes after releasing
  its lock.
- Modify: `docs/specification/frame-specification.md` §5.3, §5.4 and §5.6
  (`busy` as an outcome value, and the caller's mapping), §8 (the `min` row's
  command/query cell) and §9.6 (`busy` crosses) — note F-13, verbatim.
- Modify: `docs/design/ridl-rt.md` ("The ports", the `error` module section) and
  `docs/design/ridl-loopback.md` (the handles table gains the two roles; a
  "Waking" section).
- Test: `crates/ridl-rt/tests/ports.rs` (the forwarding of `Wakeable` through
  `&P` and `&mut P`, beside `port_forwarding.rs`'s pattern);
  `crates/ridl-loopback/tests/ports.rs`.

**Interfaces:**

- Produces `ridl_rt::port::Wakeable`, `ridl_rt::port::Interest` exactly as note
  F-6 writes them, and `ridl_rt::error::Transport::Busy` with the doc comment
  "The providing runtime refused the call at admission and the caller may retry
  later. Crosses the frame as a `response` outcome (frame specification §9.6)."
- Consumes nothing new.

- [ ] **Step 1: Write the failing loopback tests**

In `crates/ridl-loopback/tests/ports.rs`, with a test waker that counts wakes
(build it over `std::task::Wake` on an `Arc<AtomicUsize>`; the crate is `std`):

- `a_waiter_on_an_outcome_is_woken_exactly_once_by_its_settlement`: register
  `Interest::Outcome(c)` on the caller handle, settle through a handler handle,
  assert one wake; settle nothing more and assert the count stays one.
- `a_waiter_registered_after_the_settlement_is_woken_at_once`.
- `a_second_registration_under_a_key_wakes_the_displaced_waker`: two wakers,
  register both under the same key; the first counts one wake at the second
  registration; the settlement wakes only the second.
- `a_raise_wakes_a_subscribed_source_and_not_an_unsubscribed_one`.
- `a_send_wakes_the_handler_that_serves_the_member`.
- `a_caller_handle_reads_the_clock`: `Clock::now` on `CallerHandle` equals the
  aggregate's after `advance`.

Run: `cargo test -p ridl-loopback --locked` — expected: fail to compile
(`Wakeable`, `Interest` do not exist).

- [ ] **Step 2: Add the `ridl-rt` items**

`Interest` and `Wakeable` in `port.rs` after `CoherentSignals`, with F-6's doc
comments and the contract of F-5 in the trait's rustdoc: one waker per key per
handle, the displaced waker woken, register before reading.
`impl<P: Wakeable +
?Sized> Wakeable for &P` and `for &mut P`. `Transport::Busy`
in `error.rs`.

Run: `cargo test -p ridl-rt --locked --all-features` — expected: PASS (nothing
tests the new items yet).

- [ ] **Step 3: Write the `ridl-rt` forwarding test, then the loopback**

Add the two forwarding cases to `crates/ridl-rt/tests/ports.rs`. Then the
loopback: per-handle waiter storage in the store, keyed by the handle's id;
`settle`, `raise`, `send` and the reclaiming of a call collect the wakers to
wake into a local `Vec` inside the critical section and wake them after the lock
is dropped; `CallerHandle` gains `Clock` by reading `Store::now`.

Run: `cargo test -p ridl-loopback --locked` — expected: PASS, and the eleven
tests that stayed in `ports.rs` and the conformance suite over the loopback
still pass.

- [ ] **Step 4: Name the mutation**

Remove the wake from the settle path and run
`a_waiter_on_an_outcome_is_woken_exactly_once_by_its_settlement`; expected:
FAIL. Restore it. Record the mutation and the failing assertion in the pull
request body, as note F-14 asks.

- [ ] **Step 5: The frame specification and the records**

The frame-specification sentences as F-13 writes them; the design records as the
Files list says. `just fmt`, `just check`, `just link-check`.

- [ ] **Step 6: Commit and open the pull request**

```bash
git add crates/ridl-rt/src/port.rs crates/ridl-rt/src/error.rs crates/ridl-rt/src/lib.rs crates/ridl-rt/tests/ports.rs
git commit -m "feat(ridl-rt): add the Wakeable port extension and Transport::Busy (E11.16)"
git add crates/ridl-loopback docs/design/ridl-loopback.md
git commit -m "feat(ridl-loopback): implement Wakeable on every handle and Clock on the caller handle"
git add docs/specification/frame-specification.md docs/design/ridl-rt.md
git commit -m "docs(ridl): state that Busy crosses the frame as a response outcome"
```

The pull request closes driftsys/ridl#510. **Done when** the six loopback tests
and the forwarding tests pass, the mutation is named, and `just verify` is
green.

**Must not break:** the conformance suite over the loopback and the eleven
loopback-only tests; `just wasm-check`; `just compat-check`; the generated face
(untouched here).

---

### Task 2: E11.18 — `correlate::Table`, `correlate::Waiters`, the two error types; the loopback adopts them

**Model:** Opus, effort high.

**Files:**

- Create: `crates/ridl-rt/src/correlate.rs`; declare `pub mod correlate;` in
  `lib.rs`.
- Modify: `crates/ridl-rt/src/error.rs` — `ClientError`, `ProviderError` and
  their `From` impls (note F-1).
- Modify: `crates/ridl-loopback/src/store.rs` — the caller-side call table
  becomes `Table<{ Loopback::SLOTS }>` with reply bytes in a slot-indexed map;
  the per-handle waiter storage of Task 1 becomes `Waiters`;
  `crates/ridl-loopback/src/lib.rs` — `pub const SLOTS: usize = 16;` with its
  reason.
- Modify: `docs/design/ridl-rt.md` (the module table, a "The correlation table"
  section, the errors), `docs/design/ridl-loopback.md` ("A claim is not a
  correlation", "What it cannot report" — note F-9's three changes).
- Test: `crates/ridl-rt/tests/correlate.rs`;
  `crates/ridl-loopback/tests/ports.rs`.

**Interfaces:**

- Produces, in `ridl_rt::correlate`:

  ```rust,ignore
  pub struct Table<const N: usize> { /* private */ }
  impl<const N: usize> Table<N> {
      pub const fn new(budget: Option<u64>) -> Self;              // N <= 65536, checked at compile time
      pub fn insert(&mut self, reservation: u64) -> Option<Correlation>;   // None: no slot or budget
      pub fn settle(&mut self, c: Correlation, outcome: Result<(), CallError>) -> Settled; // the waker to wake, and whether the slot was reclaimed
      pub fn outcome(&self, c: Correlation) -> Option<Result<(), CallError>>;
      pub fn forget(&mut self, c: Correlation) -> Forgotten; // the one reclaim: at once, or at the settlement of a call in flight
      pub fn wake_on(&mut self, c: Correlation, waker: &Waker) -> Option<Waker>;  // the displaced waker
      pub fn slot(c: Correlation) -> usize;                       // for the runtime's byte storage
  }
  pub struct Waiters { /* Slot, Event and Claim: one Option<Waker> per kind, with its key */ }
  impl Waiters {
      pub const fn new() -> Self;
      pub fn register(&mut self, what: Interest, waker: &Waker) -> Option<Waker>; // the displaced waker
      pub fn take(&mut self, what: Interest) -> Option<Waker>;
      pub fn take_all(&mut self) -> impl Iterator<Item = Waker> + '_;             // for an unkeyed runtime
  }
  ```

  The plan's executor fixes the exact return types (`Settled`, `Forgotten`) and
  records them in the `ridl-rt` design record; the note fixes the
  responsibilities (F-9, "What the table is").

- Produces `ridl_rt::error::ClientError` and `ProviderError` as note F-1 writes
  them.

- [ ] **Step 1: Write the failing table tests**

`crates/ridl-rt/tests/correlate.rs`, each named for the sentence it pins: slot
reuse after `forget`; a generation mismatch answers `None` to the old
correlation; `insert` is `None` at `N` calls in flight and `Some` after one is
forgotten; `outcome` does not reclaim; the byte budget refuses a reservation
that does not fit and accepts it after a reclaim; `forget` of a settled call
reclaims, of a call in flight marks, and the settlement then reclaims; `wake_on`
returns the displaced waker; `settle` returns the stored waker once. `Waiters`:
one per kind, displaced returned, `take` clears.

- [ ] **Step 2: Implement `correlate` and the error types**

`Table` over `[Slot; N]` with
`Slot { generation: u64, state, waker:
Option<Waker> }`; the correlation is
`(generation << 16) | slot`; the compile-time bound on `N` is a `const`
assertion. `Waiters` over three `Option`s — `Outcome` wakers live in the table's
slot. `#![forbid(unsafe_code)]` holds; nothing allocates.

Run: `cargo test -p ridl-rt --locked --all-features`; `just wasm-check`;
`just compat-check`.

- [ ] **Step 3: Move the loopback onto both**

Replace `Store::calls`/`pending`/`next_call_id` for the caller side with the
table and a `BTreeMap<usize, Vec<u8>>` of reply bytes by slot; the claim side is
unchanged. Add `the_seventeenth_in_flight_call_is_busy` and
`a_reclaimed_slots_old_correlation_answers_none` to the loopback tests.

Run: `cargo test -p ridl-loopback --locked` — expected: PASS, including the six
tests of Task 1 unchanged.

- [ ] **Step 4: The records**

`docs/design/ridl-rt.md`: `correlate` joins the module table as the seventh
unconditional module; the errors section gains the two enums. The loopback
record: F-9's bound and reclaim sentences; the `forget` sentence is Task 4's.

- [ ] **Step 5: Commit and open the pull request**

```bash
git commit -m "feat(ridl-rt): add the correlation table, the waiter registry and the two composed errors (E11.18)"
git commit -m "feat(ridl-loopback): move the caller side onto correlate::Table and Waiters"
```

Closes driftsys/ridl#512. **Done when** the table tests, the loopback tests and
the gate pass.

**Must not break:** the Task 1 tests; the generated face's round trips in
`crates/ridl-backend-rust/tests/interaction_face.rs` (they run over the loopback
and must see the same outcomes); `just demo`.

---

### Task 3: E11.20, second half — the conformance suite covers `Wakeable` and the table

**Model:** Opus, effort high. Starts after Task 2; E11.20's first half is
merged.

**Files:**

- Modify: `crates/ridl-rt-conformance/src/` — the F-14 contract tests as
  functions generic over the factory; the factory trait gains what they need (a
  way to obtain a second caller handle, if the first half's factory has none).
- Modify: `crates/ridl-loopback/tests/` — the loopback runs the new functions.
- Modify: `docs/design/ridl-loopback.md`, the conformance crate's rustdoc.

- [ ] **Step 1: Write the seven F-14 contract tests** as generic functions, each
      failing against a runtime that does not wake (prove it with a runtime
      double in the crate's own tests, or by the Task 1 mutation).
- [ ] **Step 2: Run them over the loopback**; expected PASS.
- [ ] **Step 3: Move the Task 1 and Task 2 loopback tests** that the suite now
      covers out of `crates/ridl-loopback/tests/ports.rs`, and list in the pull
      request every test that stays there with its reason (driver §3, F4).
- [ ] **Step 4: Commit and open the pull request.** Closes driftsys/ridl#514.

**Must not break:** the first half's suite over the loopback; the mutation of
Task 1 must still turn the suite red (re-run it, and say so).

---

### Task 4: E11.21, first half — the async `Client`, the named futures and `serve`; the poll face `pub(crate)`

**Model:** Opus, effort high. Starts after Tasks 1 and 2.

**Files:**

- Modify: `crates/ridl-backend-rust/src/face.rs` — `client()` emits the new
  bounds and, per command and query, a named future type and the plain method
  that sends and returns it; `next_event` as a future; `serve` and its future;
  the correlation newtypes, the send, acknowledgment, reply and event poll
  methods become `pub(crate)` under the names `send_<call>`, `poll_<call>_ack`,
  `poll_<call>_reply` and `poll_next_event`; `dispatch` becomes the `pub(crate)`
  one-pass step returning `Result<usize, ReadError>`; the module documentation
  restates RA-20 (note F-15).
- Modify: `crates/ridl-backend-rust/tests/generated/interaction_face.rs`
  (regenerated), `tests/face_generation.rs`, `tests/dispatch_generation.rs` (the
  substrings), `tests/interaction_face.rs` (the new round trips and the F-14
  face tests), `tests/support/` (a recording port double implementing
  `Caller + Clock + Wakeable`).
- Modify: `examples/cabin/consumer/src/main.rs` — every round trip goes through
  the async client, polled by hand with `ridl_rt::task::noop_waker` (enable
  `ridl-rt/std` in the consumer's manifest); the provider side runs `serve`
  polled once per step.
- Modify: `docs/design/interaction-face.md` — a dated section "E11.21, first
  half" describing what is emitted, so the record never describes a face the
  fixture does not contain (the rewrite is Task 5's);
  `docs/design/ridl-loopback.md` — the sentence that nothing the Rust backend
  emits calls `forget`, and the call-table growth sentence, are retired.

**Interfaces:** exactly note F-10's; the poll face is `pub(crate)` under the
internal names above.

- [ ] **Step 1: Write the failing face-generation assertions** for the bounds,
      the future types and `serve`'s signature (`face_generation.rs`,
      whitespace-stripped substrings).
- [ ] **Step 2: Write the failing round trips and F-14 face tests** in
      `interaction_face.rs`: a command and a query through the async client over
      the loopback, polled with `noop_waker`; the dropped future forgets once
      (recording double), and a future that took its outcome called `forget`
      once at that moment and nothing on drop; the slot wait over a loopback
      with 16 calls in flight, woken by a reclaim, and `Send(Busy)` when
      `advance` passes `max` first; a sent call whose clock passes `max`
      resolves to `Undelivered` or `Timeout` and forgets; `serve` over a handler
      double whose `next_claim` fails resolves to `ProviderError::Claim` after
      settling the claims before it.
- [ ] **Step 3: Emit the futures and `serve`.** The state machine per note F-4:
      phases `Unsent(args)`, `Waiting(c)`, `Done`; `poll` registers, then reads,
      then returns; `Drop` forgets in `Waiting`. `Serve` registers
      `Interest::Claim`, then drains through the internal one-pass step.
      `cargo fmt --all`; regenerate the fixture.
- [ ] **Step 4: Run the whole backend suite and the emitted-crate matrix.**
      `cargo test -p ridl-backend-rust --locked`; `just demo`; if
      `just compat-check` does not compile the emitted cabin crate at 1.83 as
      edition 2021, add it there (note F-10, "The build matrix").
- [ ] **Step 5: Commit and open the pull request.** Part of driftsys/ridl#515
      (not closed until Task 5). This is the breaking step for a consumer of
      generated code.

**Must not break:** `just demo` (the consumer is rewritten in this task); the
two `crates/ridlc` tests ADR-0023 decision 2 names; every existing round trip,
moved onto the async client; the descriptor snapshots.

---

### The release, between Task 4 and Task 5

Not a task. Sebastien tags the workspace version that carries E11.16 to E11.19,
under ADR-0021 decision 10 and ADR-0007 decision 14; the face of Task 4 has
exercised every item by then. Task 5 waits for it, so the crate the blocking
client links is the released one; the breaking step for a consumer of generated
code is Task 4, before the release.

---

### Task 5: E11.21, second half — the `blocking` module, the records, gardening

**Model:** Opus, effort high.

**Files:**

- Modify: `crates/ridl-backend-rust/src/face.rs` — the `blocking` module under
  `#[cfg(feature = "std")]` (note F-10 and F-11). The emitted `Cargo.toml`
  (`crates/ridlc/src/lib.rs`, the template) already declares `std = []`, on by
  default; it becomes `std = ["ridl-rt/std"]`.
- Modify: the fixture and the three substring test files; `interaction_face.rs`
  — the blocking client's round trips and the by-phase timeout test (F-14).
- Modify: `examples/cabin/consumer/src/main.rs` — the four round trips through
  both clients and `blocking::serve` with a timeout; its manifest enables the
  emitted crate's `std`.
- Modify: `docs/design/interaction-face.md` (rewritten from the note),
  `docs/design/ridl-loopback.md` (the served-set deviation retired or kept,
  stated), `docs/book/introduction.md` and `docs/book/cli-reference.md` where
  they show the poll face, `docs/technotes/ridl-rt-by-example.md`'s client
  sketch (driftsys/ridl#526 notes it is stale already).
- Gardening (`sdd-gardening`): the note, this plan and the driver archive to
  `docs/archive/`; the decisions live in ADR-0021, ADR-0023 and the design
  records; `docs/wip/` holds nothing of lane F afterwards.

- [ ] **Step 1: Write the failing blocking round trips and the by-phase timeout
      test.**
- [ ] **Step 2: Emit the `blocking` module**; `block_on` over the pinned future,
      `sent()` asked when it gives up (F-11).
- [ ] **Step 3: Regenerate the fixture** and move `examples/cabin/consumer`'s
      command and query round trips onto the blocking client, keeping the async
      ones beside them.
- [ ] **Step 4: The records and the book**; `just book-check`,
      `just
      link-check`, `just doc-path-check`.
- [ ] **Step 5: Garden**, then `just verify`, then the pull request. Closes
      driftsys/ridl#515 and #509's implementation half; the #328 comment; the §6
      comment on driftsys/ridlc-gen-kotlin (K4).

**Must not break:** `just demo`; the emitted crate's build matrix; the
conformance suite.
