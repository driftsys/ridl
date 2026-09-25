# Lane F driver — the async face and the runtime substrate

Status: driver, 2026-09-25. One lane, stages F0 to F5, each stage a fresh
session. This document is written for an agent with no prior context: it names
every record it relies on and carries every fact a session would otherwise need
from a conversation. Set the `THIS SESSION RUNS` line below before starting a
session, and do only that stage. Where this document says "stop", stop and
report to Sebastien; do not guess past it.

Where this document and an ADR disagree, the ADR wins. This document summarizes;
it does not decide. The plan it carries was reviewed and validated by Sebastien
on 2026-09-25, in the conversation that produced this document; the decisions
listed in §4 are not taken yet, and F2 takes them.

**THIS SESSION RUNS: F1** — F0 is the pull request that added this document.

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
  `examples/cabin` through the generated face over `ridl-loopback` — the one
  check in the gate that exercises what F5 changes.
- Review before merge, per the lanes plan §7
  ([`2026-09-13-step1-lanes-plan.md`](2026-09-13-step1-lanes-plan.md)): open the
  pull request, run `/review <PR>` (the docs-only lane when no executable line
  changes), post the ledger as a comment, fix what is kept, run pass 2, run
  `just verify`. After that the driver may squash-merge; Sebastien has given
  that permission for reviewed and verified work. One pull request open at a
  time, except where §3 says a stage's pull requests are independent.
- Never push a tag, publish to a registry, add a secret, or push to `main`.
- A new crate adds its own scope to `.git-std.toml`, which is an explicit list.

## 1. What this lane is

The generated face exposes one poll-based surface. A consumer-side send returns
a per-call correlation newtype, and the application calls `*_ack`, `*_reply` and
`next_event` in a loop of its own; a provider runs `dispatch` repeatedly. Every
application that wants to wait for a reply writes the same loop, and every
application handles correlations it has no use for. driftsys/ridl#485 records
two related findings on the call shapes.

The lane replaces that public surface with two clients per interface, and puts
underneath them the substrate every runtime with asynchronous replies needs:

- **The face.** `<iface>::Client`, async, `no_std`; `<iface>::blocking::Client`
  behind a `std` cargo feature, implemented as `block_on` over the async call so
  the two cannot diverge; `serve(provider)` in both forms in place of the public
  `dispatch`. The correlation newtypes, the `*_ack` and `*_reply` methods,
  `next_event` and `dispatch` become `pub(crate)`.
- **The `ridl-rt` substrate.** A keyed `Wakeable` port extension (the wake
  source no port may wait for), `Transport::Busy`, a `std` feature with
  `block_on` and a no-op waker, a correlation table with a keyed waker registry,
  and the helpers the language reference defines but every runtime computes on
  its own today: freshness, event loss, the in-flight byte budget, the call
  deadline. One `ridl-rt` 0.x minor release carries all of it.
- **A conformance crate.** `crates/ridl-loopback/tests/ports.rs` states what a
  runtime must do behind each port and only `ridl-loopback` runs it. The suite
  becomes `ridl-rt-conformance`, generic over a runtime factory, and the
  loopback becomes its first user.
- **Three findings from the Kotlin port**, driftsys/ridlc-gen-kotlin#3, which
  ported the face and the port tests to Kotlin by hand: the Rust face reports a
  never-published signal as `Invalid(Detected(Corrupt))` instead of `Init`, the
  FlatBuffers codec emitter writes code that does not compile for a `[bool]`
  field, and the AIDL transaction codes the frame specification fixes collide
  with the control methods.
- **The Binder statement.** ridl specifies no Binder layout: on Android, a
  runtime binds the ports over its own binder contract, which may be one
  generic, versioned AIDL serving every catalog. This reverses lane P's decision
  D-P5 and replaces `frame-specification.md` §11.2; §3 says how.

The Kotlin side mirrors the lane in driftsys/ridlc-gen-kotlin, one issue per
Rust item, one wave behind. §6 states what each Rust stage owes it.

**Records that bind this lane.** Read them before F2, and read the ones a stage
names before that stage:

- [ADR-0020](../decisions/ADR-0020-third-encoding-runtime-layering-and-plugin-system.md)
  decisions 5 and 6 — what `ridl-rt` is, and that every runtime is its own crate
  outside it.
- [ADR-0021](../decisions/ADR-0021-ridl-rt-0.1-api-and-release.md) — the
  `ridl-rt` 0.1 API. Decision 8 (three cargo features, no dependency), 9
  (`Contract` and `CallError` exhaustive, every other error enum
  `#[non_exhaustive]`), 10 (a breaking change is a 0.x minor), 11 (the
  forwarding impls) and 12 (one handle per port role). A new public item in
  `ridl-rt` is an ADR-0021 decision, not an implementation detail.
- [ADR-0023](../decisions/ADR-0023-interaction-face-generation.md) — the face.
  Decision 4 and its 2026-09-20 amendment (a send returns
  `Result<Correlation, SendError>`, the correlation newtype per call) and
  decision 5 (the face holds its port by value) are what F2 amends or reverses.
- [The frame specification](../specification/frame-specification.md) — §5 (what
  crosses per kind), §6 (the control plane), §8 (timing; its table says how a
  faster call is refused "is not fixed here"), §9.6 (which failures cross),
  §11.2 (the AIDL binding F1c replaces).
- [The ridl language reference](../specification/ridl-language-reference.md) —
  §4.4 (the init value), §5 (events), §9 (timing), §10.3 (transport errors).
- The design records [`ridl-rt.md`](../design/ridl-rt.md),
  [`interaction-face.md`](../design/interaction-face.md) and
  [`ridl-loopback.md`](../design/ridl-loopback.md).
- [The lane P driver](2026-09-22-lane-p-driver.md) §3, decisions D-P4 and D-P5,
  and driftsys/ridlc-gen-kotlin#3, the Kotlin pull request whose sections K3b
  and K5 the Binder statement and the `Wakeable` design answer.

**Code the lane has to fit.** `crates/ridl-rt/src/port.rs` (the port traits,
`Correlation`, `SendError`, `ScannableSignals` — the shape a port extension
takes), `crates/ridl-rt/src/error.rs` (`Transport`, `CallError`),
`crates/ridl-rt/src/contract.rs` (`Timing`, `PayloadInfo`, `EncodedSizes`),
`crates/ridl-loopback/src/` and `crates/ridl-loopback/tests/ports.rs`,
`crates/ridl-backend-rust/src/face.rs` (the face emitter; `ridlc::run_build`
reaches it through `generate_pipeline`), `crates/ridl-backend-rust/src/lib.rs`
(the FlatBuffers codec emitter), and `examples/cabin/` (the consumer the demo
runs).

## 2. The stories and the issues

Six stories join Epic 11 in `docs/ROADMAP.md`, under the identifiers below. Four
more items are issues and not stories: an ADR amendment, a specification change
and two defects. The Kotlin items are issues in driftsys/ridlc-gen-kotlin.

| Item                                                                                    | Kind                     | Stage |
| --------------------------------------------------------------------------------------- | ------------------------ | ----- |
| The ADR-0023 and ADR-0021 amendments: two clients per interface, the poll face private  | ADR amendment, an issue  | F2    |
| E11.16 `ridl-rt`: a keyed `Wakeable` port extension and `Transport::Busy`               | story                    | F3    |
| E11.17 `ridl-rt`: a `std` feature with `block_on` and a no-op waker                     | story                    | F1d   |
| E11.18 `ridl-rt`: a correlation table and a keyed waker registry                        | story                    | F3    |
| E11.19 `ridl-rt`: helpers for freshness, event loss, the in-flight budget and deadlines | story                    | F1e   |
| E11.20 `ridl-rt-conformance`: the port contract tests as a reusable crate               | story                    | F4    |
| E11.21 Rust backend: the async and blocking clients and `serve`, the poll face private  | story                    | F5    |
| Frame specification §11.2 and the roadmap: ridl specifies no Binder layout              | specification change     | F1c   |
| The Rust face reports a never-published signal as `Invalid(Detected(Corrupt))`          | defect                   | F1a   |
| The FlatBuffers codec emitter does not compile for a vector of booleans                 | defect                   | F1b   |
| K1 remove the per-interface AIDL emission from driftsys/ridlc-gen-kotlin#3              | Kotlin, after F1c        | —     |
| K2 `ridl-rt-kt`: keyed `Wakeable`, `Transport.Busy` and a blocking wait                 | Kotlin, after F3 and F1d | —     |
| K3 `ridl-rt-kt`: the correlation table and the runtime helpers                          | Kotlin, after F3 and F1e | —     |
| K4 a blocking client and a `suspend` client per interface, the poll face `internal`     | Kotlin, after F5b        | —     |
| K5 the Kotlin twin of `ridl-rt-conformance`                                             | Kotlin, after F4         | —     |

F0 files the issues; their numbers are recorded in the roadmap's tracker section
under "Filed 2026-09-25" and in a comment on driftsys/ridl#328.

## 3. Stages

| Stage | Content                                                                                                          | Model  | Effort | Starts when                                       |
| ----- | ---------------------------------------------------------------------------------------------------------------- | ------ | ------ | ------------------------------------------------- |
| F0    | This document, the roadmap rows, the issues                                                                      | —      | —      | done in the pull request that added this file     |
| F1a   | The face returns the init value under `Init` before the first publication                                        | Sonnet | —      | now; independent of every other F1 item           |
| F1b   | The FlatBuffers codec emitter compiles for a `[bool]` field                                                      | Sonnet | —      | now; independent                                  |
| F1c   | The Binder statement: §11.2, the two roadmap paragraphs, the D-P5 note                                           | Opus   | high   | now; independent                                  |
| F1d   | E11.17, the `std` feature, with its ADR-0021 decision 8 note                                                     | Opus   | high   | now; independent                                  |
| F1e   | E11.19, the helpers                                                                                              | Opus   | high   | now; independent                                  |
| F2    | The design note, the disposition, then the two ADR amendments and the plan for F3 to F5                          | Fable  | —      | F0 merged; F1 need not be finished                |
| F3    | E11.16, then E11.18; `ridl-loopback` adopts both                                                                 | Opus   | high   | F2's amendments merged                            |
| F4    | E11.20, the conformance crate, over the current contract first                                                   | Opus   | high   | F0 merged; extended to E11.16 and E11.18 after F3 |
| F5a   | E11.21, first half: the async `Client` and `serve`, the poll face still public                                   | Opus   | high   | F3 and F1d merged                                 |
| —     | The `ridl-rt` 0.x minor release carrying E11.16 to E11.19 — a maintainer act, not a stage                        | —      | —      | F5a merged, so the face has exercised the API     |
| F5b   | E11.21, second half: the `blocking` module, the poll face `pub(crate)`, the records, `examples/cabin`, gardening | Opus   | high   | the release published                             |

**Sequential and parallel.** The chain is F0, F2, F3, F5a, the release, F5b:
each merged before the next branches, and F2's note stopped for its disposition
before its amendments. F1a to F1e are five independent pull requests with no
design decision among them; they run in any order, in parallel with each other
and with F2, and they are the exception to the one-open-pull-request rule. F4
starts after F0 and runs beside F2 and F3; it is finished only after F3, because
its `Done when` covers `Wakeable` and the table. The one thing every F1 item
must not do is decide a §4 question: an F1 item that needs one stops and leaves
it to F2.

### F0 — this document, the roadmap rows, the issues. Done.

The pull request that added this document also added the six story rows to Epic
11 of `docs/ROADMAP.md`, with the sequencing above, and filed the ten
driftsys/ridl issues and the five driftsys/ridlc-gen-kotlin issues from drafts
Sebastien validated. It did not change the Kotlin paragraph of the roadmap or
`frame-specification.md` §11.2: that is F1c, so the reversal of D-P5 is one
reviewed pull request and not a side effect of a planning one.

### F1 — five independent items

- **F1a — the init value.** The port is right: `ports.rs`'s
  `a_signal_with_no_publication_reads_as_init_and_copies_nothing` passes over
  `ridl-loopback`. The face is wrong: the `_ => Detection::Corrupt` arm in
  `crates/ridl-backend-rust/src/face.rs` (the signal read path, near line 284)
  reaches the never-published case. ridl §4.4: before the first publication a
  signal reads its init value under `Provenance::Init`. Fix the emitter, add a
  face test over `ridl-loopback` that reads before any publication, regenerate
  the fixture. Cite the Kotlin face, which does this correctly, in the pull
  request.
- **F1b — `[bool]` in the FlatBuffers codec.** Add a corpus package with a
  `[bool]` field, make it compile and round-trip, pin the emitted code in a
  snapshot. The other two codec findings of driftsys/ridlc-gen-kotlin#3 are
  already tracked, `step` in driftsys/ridl#469 and NaN in a range check in
  driftsys/ridl#421; do not fold them in.
- **F1c — the Binder statement.** Three texts and one note. Replace
  `frame-specification.md` §11.2 with the statement that on Android a runtime
  binds the ports over its own binder contract, which may be one generic,
  versioned AIDL serving every catalog, and that ridl specifies no Binder layout
  and no transaction code; keep the ridl reference's Appendix B AIDL column,
  because an ordinal stays stable whether it travels as a code or as a field.
  Reword the two roadmap paragraphs that say the opposite: the E11.1 paragraph
  in Epic 11 ("the Kotlin backend's AIDL over Binder") and "The Kotlin backend
  owns its IPC binding" under "After step 2". Leave the platform ladder row and
  Appendix B's target list alone: they name the platform, not a layout. Add a
  dated note under D-P5 in [the lane P driver](2026-09-22-lane-p-driver.md),
  naming the issue and this driver, because D-P5 was taken by Sebastien on
  2026-09-22 and a reader of that driver must see it was reversed and where.
  State the reasons in the pull request: a generated binding per interface ties
  the AIDL version to every interface change, reads signals over IPC, gives a
  command no path to report "busy", and its fixed codes collide with the control
  methods (K3b's finding). D-P4, the lowered model's content, is unchanged. O-P3
  is unchanged.
- **F1d — E11.17.** A `std` cargo feature, off by default, adding
  `block_on(fut, deadline: Option<Instant>) -> Option<F::Output>` — a waker that
  unparks the current thread through `std::task::Wake` on an `Arc`,
  `thread::park_timeout` until the deadline, a poll on every wake, a spurious
  wake harmless — and `noop_waker() -> Waker` as `Waker::from(Arc<Noop>)`,
  created once per loop and cloned. `Waker::noop()` needs Rust 1.85 and the
  crate is at 1.83; a raw waker needs `unsafe`, which the crate forbids
  (`#![forbid(unsafe_code)]`). No dependency and no `unsafe`. Tests park and
  wake across threads and time out at the deadline. The three checks that bind
  the crate: `just wasm-check` (`--no-default-features`, unchanged),
  `just compat-check` (edition 2021 at 1.83 with `--all-features`, so the
  feature must build there), and `just test`. The feature is a fourth cargo
  feature where ADR-0021 decision 8 declares three: the pull request adds a
  dated note under decision 8 saying so, that it enables `block_on` and
  `noop_waker` and no dependency, and that F2's amendment records the rest.
- **F1e — E11.19.** Four helpers, each with tests citing the section it
  implements: `Freshness::of(stamp, now, timing)` returning `Fresh`,
  `Stale { by }` or `Unbounded` from the envelope and the member's `max` (ridl
  §4, §9); an event `seq` tracker holding the last `seq` per interface and
  reporting a gap as a loss (ridl §5, frame §7); the in-flight budget from the
  descriptors — a member's reservation is its `args` plus its `reply`
  `PayloadInfo::max_size` for the package's encoding, and a table budget sums
  `Interface::MEMBERS`; a size that is `None` is reported, never guessed; and a
  member's call deadline from its `Timing`, which is `None` when `max` is. The
  last helper takes no position on what a caller does with `None`; that is F-2.

### F2 — the design note, the disposition, the amendments, the plan

Write `docs/wip/2026-09-2x-async-face-design.md`: the design note taking the
fourteen decisions of §4. Open it as a pull request. **The gate is Sebastien's
disposition comment on that pull request**, the way lane K's K-1 to K-12 were
disposed. Write each decision so it can be disposed of by reading: the decision,
the reason, the alternative it rejects and why. Number them F-1 to F-14 so a
comment can name one.

Then, once the disposition is in hand and in the same session if context allows,
open the second pull request: the amendments and the plan.

- **The ADR-0023 amendment**, dated, reversing decision 4 and its 2026-09-20
  amendment for the public surface: a `Client` call is an async call returning
  the error type F-1 names; the correlation newtypes stay as the face's internal
  vocabulary and become `pub(crate)`; decision 5 is unchanged. Record the
  semantics F2 fixed. Close driftsys/ridl#485 from this amendment with the two
  dispositions F-7 states.
- **The ADR-0021 amendment**, dated: `Wakeable`, `Transport::Busy`, the `std`
  feature (already noted by F1d), `correlate`, the helpers, and the error type
  if it lives in `ridl-rt`. It states, against ADR-0020 decision 6, that
  `ridl-rt` carries runtime-side helpers — `no_std`, allocation-free, behind no
  feature — and that a runtime remains its own crate. It also states that the
  release carrying E11.16 to E11.19 is one 0.x minor under decision 10.
- **The plan**, `docs/wip/2026-09-2x-async-face-plan.md`, in the shape of
  `typl-value-objects-plan.md`: one task per landable change for F3, F4 and F5,
  each with the files it touches, the test that proves it and what it must not
  break. The plan's pull request amends this document's stage table if the split
  above changes.

Open `docs/design/interaction-face.md` and `docs/design/ridl-rt.md` while
writing the note; the decisions must be stated so that F5b can rewrite those
records from them.

### F3 — E11.16, then E11.18

Two pull requests in order. E11.16 adds `port::Wakeable` — an extension trait in
the shape of `ScannableSignals`, `fn wake_on(&self, what: Wake, waker: &Waker)`
with `Wake` as F-6 fixes it — and `Transport::Busy`, and makes `ridl-loopback`
implement `Wakeable`; its tests wake exactly one waiter per key. E11.18 adds
`correlate::Table<const N: usize>` — `N` slots, a generation counter per slot, a
stored outcome and one waker, the correlation `(generation << 16) | slot`, a
byte budget per F1e's reservation, `SendError::Busy` when no slot or not enough
budget is free, FIFO slot waiters woken one at a time through `Wake::Slot` — and
the keyed waker registry behind `Wakeable`; `&mut self`, caller-supplied
storage, no allocation, each runtime putting it behind its own lock. Its unit
tests cover slot reuse, a generation mismatch, FIFO slot waiters, the byte
budget, and `forget` freeing a slot. Whether `ridl-loopback` moves its call
table onto `correlate::Table` is F-9; if it does, this stage does it.

### F4 — E11.20, the conformance crate

`crates/ridl-rt-conformance`: the contract tests of
`crates/ridl-loopback/tests/ports.rs` as functions generic over a runtime
factory, so any runtime runs them from its own test suite. The factory needs two
hooks the current tests use through the loopback's own API, a hand-driven clock
(`Loopback::advance`) and a fault injected once (`Loopback::fail_next_settle`);
a test only the loopback can express stays in `crates/ridl-loopback/tests/` and
the pull request lists each one with the reason. `ridl-loopback` runs the suite
as a dev-dependency and passes, and a mutation in the loopback turns it red —
name the mutation in the pull request. The new crate adds a workspace member, a
`.git-std.toml` scope, a row in ADR-0020 decision 6's crate table, and moves
"eighteen crates" to nineteen in `AGENTS.md`. After F3, the suite covers
`Wakeable` (exactly one waiter woken per key) and the table's observable
behaviour through the ports.

### F5 — E11.21, in two pull requests

- **F5a — the async `Client` and `serve`.** The face emitter writes
  `<iface>::Client` with one async call per command and query, each sent when
  the function is called, awaited for its outcome, with `forget` on drop (F-4),
  bounded by the member's `max` measured from the call (F-2, F-3), over the
  `Wakeable` of E11.16 and the slot semantics of E11.18; and `serve(provider)`,
  which terminates as F-7 fixes. The poll face stays public in this pull request
  so the interaction-face round trips and `examples/cabin` keep compiling. The
  cabin round trip passes through the async client on `ridl-loopback`.
- **The release.** `ridl-rt`'s 0.x minor carrying E11.16 to E11.19, published by
  Sebastien after F5a, so the API has been exercised by a face before it is
  published. Not a stage; nothing in this lane publishes.
- **F5b — the `blocking` module and the private poll face.**
  `<iface>::blocking::Client` and `blocking::serve` under the emitted crate's
  `std` feature, as `block_on` over the async calls; the correlation newtypes,
  `*_ack`, `*_reply`, `next_event` and `dispatch` become `pub(crate)`. The cabin
  round trip passes through both clients; a dropped future calls `forget`; a
  call waits for a free slot within its bound. The records move with it:
  `docs/design/interaction-face.md`, `docs/design/ridl-loopback.md`,
  `docs/book/introduction.md` and `docs/book/cli-reference.md` where they show
  the poll face, and `examples/cabin/consumer`. This is the one breaking step of
  the lane for a consumer of generated code. F5b also runs `sdd-gardening`: the
  note and the plan archive to `docs/archive/`, this driver with them, and the
  decisions live in the two ADRs and the design records.

## 4. The fourteen decisions F2 must take

The proposals below are the lane's starting position, validated as a direction
on 2026-09-25 and not as decisions. F2 may reject one; the note then says why.

**F-1. The error type of a client call.** One type combines a failure before
sending (`SendError`) and a failed outcome (`CallError`). `CallError` is
exhaustive by ADR-0021 decision 9, so adding a `Send` variant to it is a
breaking change and is ruled out. Proposal:
`pub enum ClientError { Send(SendError),
Call(CallError) }` in `ridl-rt`,
`#[non_exhaustive]`, `Copy`, so a call returns `Result<T, ClientError>` and `?`
works with one type. Say whether it is `ridl-rt`'s or the face's, and why.

**F-2. A member whose `max` is `None`.** `Timing.max` is `Option<Duration>`.
Proposal: the async call waits without a bound, and the blocking client takes an
explicit `deadline: Option<Instant>` that covers it. The alternative, refusing
the call, breaks every member without a timing annotation.

**F-3. The error when no slot comes within `max`.** The bound covers the wait
for a free runtime slot. Proposal: `SendError::Busy`, because nothing was sent
and the condition is retryable; `Transport::Timeout` stays "sent, no reply".

**F-4. When a call is sent, and what a drop does.** The direction says a call is
sent when the function is called, not when its future is first polled. An
`async fn` body runs nothing until polled, so the method is a plain function
that sends and returns a future for the wait; a send failure is then a future
that is ready with `Err`, so the caller writes one `?`. Dropping the future
calls `forget`; a command already sent is not taken back. Say what the future
holds (the port by reference, so `Client<P>` needs `P: Sync` for a `Send`
future) and whether the futures are `Send`.

**F-5. One waiter per key.** `wake_on` stores a clone of the waker in a bounded,
allocation-free registry. Proposal: one waiter per key, and a second `wake_on`
for the same key replaces the first — the `AtomicWaker` model, and the meaning
of "exactly one waiter is woken". State what a second task waiting on the same
interface's events observes.

**F-6. The key set.**
`Wake::{Outcome(Correlation), Slot, Event(InterfaceNo),
Claim(InterfaceNo)}` is
the proposal. Decide whether `Event` and `Claim` are keyed per interface or per
interface and ordinal, and what a runtime that cannot key (a single "something
changed" source) is allowed to do.

**F-7. `serve`'s termination, and driftsys/ridl#485.** `dispatch` today ends a
pass on a `ReadError` and returns the count so far, so a detached runtime looks
idle and an async `serve` over it would loop. Proposal: `serve` returns `Err`
when the runtime is detached, with the claims already settled staying settled.
The two dispositions of #485: item 1 (the three-deep reply shape) is closed by
the poll face becoming `pub(crate)`, and F5 may reshape the internal methods
freely; item 2 is this decision.

**F-8. Where the helpers live.** Module names in `ridl-rt` for the table, the
registry and the E11.19 helpers, and the sentence in the ADR-0021 amendment that
reconciles them with ADR-0020 decision 6's "each runtime is its own crate".

**F-9. Whether `ridl-loopback` adopts `correlate::Table`.** It is the reference
runtime, and the table is untested by a real runtime until one uses it.
Proposal: yes, in F3, replacing the loopback's own call table. Say what the
loopback's `fail_next_settle` and hand-driven clock need from the table.

**F-10. The `no_std` and 1.83 shape of the async client.** The client is a
struct, not a trait, so `impl Future` in return position is available. Decide
the concrete signature and check it builds at `rust-version = "1.83"`, on
`wasm32` with `--no-default-features`, and as edition 2021.

**F-11. The blocking client's deadline API.** How `blocking::Client` exposes
F1d's `deadline` against F-2, and whether a member with a `max` accepts a
caller's shorter deadline.

**F-12. What of decision 4 survives.** Whether the ADR-0023 amendment reverses
decision 4 or supersedes it for the public surface while keeping its reasoning
for the internal methods. The correlation newtypes become `pub(crate)`; say
whether `SendError` remains the internal send methods' error.

**F-13. Whether `Busy` crosses the frame.** A runtime relaying a remote "busy,
try later" as a call's outcome needs `Transport::Busy`, and the frame
specification §8 says how a faster call is refused "is not fixed here". Decide
whether `Busy` is a frame-level outcome, and if so which section of
`frame-specification.md` states it and in which stage it is written.

**F-14. What proves it.** The conformance suite's contract for `Wakeable`
(exactly one waiter woken per key, and a wake that arrives between a poll and a
registration is not lost), the mutation that must turn `ridl-loopback` red, and
the face test for a dropped future calling `forget`.

## 5. What lane F does not decide

- **The engine.** Outside this repository (the 2026-09-12 re-scope §3.7).
- **The transport.** E11.9, `ridl-transport-ws`, is a separate story. Where a
  decision above binds it — F-6, F-13 — say so and give the reason it
  generalizes, but do not design it.
- **The Kotlin runtime's internals.** `ridl-rt-kt` mirrors the Rust shapes and
  records each in its `CorrespondenceTest`; what the Kotlin side owes is §6.
  K4's reversal of driftsys/ridlc-gen-kotlin#3's O-K3 for generated code is that
  repository's decision.
- **The catalog check.** E16.2 (driftsys/ridl#378) still owns it; a client's
  constructor is unchanged by this lane in that respect.
- **The plugin protocol.** Lane P landed the backend contract; the face emitter
  is still `crates/ridl-backend-rust/src/face.rs`, reached from `ridlc` through
  `generate_pipeline`. This lane changes what the emitter writes, not how it is
  invoked.

## 6. What each stage owes the Kotlin side

Post these on the matching driftsys/ridlc-gen-kotlin issue when the Rust stage
merges, one comment each, with the Rust names and signatures verbatim:

- **F1c → K1.** The Binder statement, so K3b's AIDL emission is removed and its
  disposition item 5 (transaction code placement) needs no decision.
- **F3 and F1d → K2.** `Wake`'s variants, `Wakeable::wake_on`'s signature,
  `Transport::Busy`, and `block_on`'s deadline semantics.
- **F3 and F1e → K3.** `correlate::Table`'s test cases and their expected
  results, and the four helpers' signatures and section citations.
- **F4 → K5.** The factory's two hooks and the list of tests that stayed in the
  loopback with their reasons.
- **F5b → K4.** The public surface of both clients, `serve`'s termination, and
  the error type.

## 7. Facts about the tree, easy to get wrong

Verified on `main` at 36241a3, 2026-09-25.

- **`Correlation` is `pub struct Correlation(pub u64)`**
  (`crates/ridl-rt/src/port.rs`). `(generation << 16) | slot` leaves 48 bits of
  generation. Kotlin's signed `Long` carries it.
- **`Transport` is `#[non_exhaustive]`; `CallError` and `Contract` are
  exhaustive** (ADR-0021 decision 9, `crates/ridl-rt/src/error.rs`). Adding
  `Transport::Busy` is not a breaking change; adding a variant to `CallError`
  is.
- **`SendError` already has `Busy`** ("the runtime cannot accept a call now.
  Retryable."). E11.16's addition is `Transport::Busy`, the remote case, not a
  second local one.
- **`Timing.max` is `Option<Duration>`** (`crates/ridl-rt/src/contract.rs`), and
  it is the response bound of a call, the staleness bound of a signal and the
  time to live of an event.
- **`ridl-rt` is `no_std`, allocates nothing, forbids `unsafe`
  (`#![forbid(unsafe_code)]`), has `rust-version = "1.83"`, and its three cargo
  features carry no dependency.** `Waker::noop()` is 1.85. `std::task::Wake` is
  stable since 1.51.
- **`crates/ridl-loopback/tests/ports.rs` holds 48 tests**, including a
  hand-driven clock (`the_clock_is_hand_driven_not_wall_clock`) and one injected
  fault (`settle_can_be_made_to_fail_once_then_succeed`). A generic suite needs
  both as factory hooks.
- **The never-published signal is a face defect, not a port defect.** The port
  test `a_signal_with_no_publication_reads_as_init_and_copies_nothing` passes;
  the face's read path has a `_ => Detection::Corrupt` arm.
- **D-P5 is a decision Sebastien took**, on the pull request that added the lane
  P driver, and `frame-specification.md` §11.2 is written from it. F1c reverses
  a taken decision and must say so in every text it touches.
- **`just demo` is in `just build`.** Any change to the face's public surface
  that breaks `examples/cabin/consumer` fails the gate, which is why F5 is split
  and F5a keeps the poll face public.

## 8. Order against other lanes

No pull request was open on 2026-09-25. Two files are shared with any lane that
resumes: `crates/ridl-backend-rust/src/face.rs` and the interaction-face fixture
under `crates/ridl-backend-rust/tests/generated/`, which lane C's Epic 10 and
E16.2 both touch when they run. Before branching a stage that regenerates the
fixture, check for an open pull request touching it. Never enter another lane's
worktree.

## 9. After each stage

Post one comment on driftsys/ridl#328: the stage, the pull request, and what
another lane must know. After F2: the fourteen dispositions in one line each,
and which of them bind E11.9. After each Rust stage that a Kotlin issue waits
on, the §6 comment on that issue.

## 10. When to stop

Stop and ask Sebastien when a decision and a merged record disagree in a way the
note did not foresee, when an F1 item turns out to need a §4 decision, or when a
gate check does not hold. A session low on context stops at a stage boundary and
posts the handoff.

Plain, literal prose everywhere. Never name a private or consumer project.
