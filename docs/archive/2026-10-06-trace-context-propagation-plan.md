# Trace context propagation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** `ridl-rt` carries an optional W3C trace context from a call or an
event's sender to the `Claim` or `RawOccurrence` its receiver reads, and
`ridl-loopback` delivers it.

**Architecture:** A plain-data `TraceContext` type in a new `ridl_rt::trace`
module. A last argument `trace: Option<TraceContext>` on `Caller::command`,
`Caller::query` and `EventSink::raise`, and a `trace` field on `Claim` and
`RawOccurrence`. `Envelope` and the frame are unchanged. The generated face
passes `None`.

**Tech Stack:** Rust (`no_std` `ridl-rt`), `ridl-loopback`,
`ridl-rt-conformance`, `ridl-backend-rust` (quote-based emitter).

**Spec:**
[`docs/wip/2026-10-06-trace-context-propagation-design.md`](2026-10-06-trace-context-propagation-design.md)
— read it before any task. Issue: driftsys/ridl#752.

## Global Constraints

- `ridl-rt` takes no dependency and stays `no_std`; `just wasm-check` and
  `just compat-check` (Rust 1.83, editions 2021 and 2024) keep passing.
- `TraceContext` is exactly
  `{ trace_id: [u8; 16], span_id: [u8; 8], flags: u8 }`, all fields `pub`,
  derives `Clone, Copy, Debug, PartialEq, Eq, Hash`, no feature gate, no
  validation.
- The new argument is named `trace`, has type `Option<TraceContext>`, and is the
  **last** parameter. The new field is named `trace`, is `pub`, and sits after
  `envelope`.
- `Envelope` is not changed. The frame specification gains no field.
- Delivery contract (spec §3.4): a carrying runtime delivers the sender's value
  unchanged on each claim and on each subscriber's occurrence; a non-carrying
  runtime or transport delivers `None`; `None` is delivered as `None`.
- Shipped docs and rustdoc name no story id or plan name (`just story-id-check`)
  and are plain, literal English.
- The breaking commit is `feat(ridl-rt)!: …`. Commits end with
  `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- Work in `../ridl-752` (branch `docs/752-trace-context-spec`); run
  `./bootstrap` there once before Task 1.

## Review Focus

1. A claim reported as `ReadError::ShortClaim` and presented again must still
   carry its context — Task 2,
   `an_oversized_claims_context_survives_its_second_presentation`.
2. An occurrence left in the queue by `ReadError::Short` must still carry its
   context on the next `next` — Task 2,
   `a_short_buffer_keeps_the_occurrences_context`.
3. A call slot reused after `forget` must not keep the previous call's context —
   Task 2, `a_reused_call_slot_does_not_keep_the_previous_context`.
4. Two callers on one provider with different contexts each get their own — Task
   2, `two_calls_in_flight_each_keep_their_own_context` (use two caller handles,
   not one).
5. A borrowed port (`&P`) must forward the context, not drop it — Task 1,
   `port_forwarding.rs` assertions.

---

### Task 1: The API change and its mechanical fallout (one breaking commit)

**Files:**

- Create: `crates/ridl-rt/src/trace.rs`
- Modify: `crates/ridl-rt/src/lib.rs` (add `pub mod trace;` in the module list),
  `crates/ridl-rt/src/port.rs` (`EventSink::raise`, `Caller::command`,
  `Caller::query`, `RawOccurrence`, `Claim`, the `&P` impls near line 696)
- Modify (fallout — every implementer, call site and literal): `ridl-loopback`
  (`src/lib.rs`, `src/handle.rs`, `src/store.rs`, `tests/ports.rs`),
  `ridl-rt-conformance/src/*.rs`, `ridl-rt/tests/*.rs`, `ridl-rt/examples/*.rs`,
  `ridl-backend-rust/src/face.rs` (emitter, line ~862 and the command/query
  emission), `ridl-backend-rust/tests/support/doubles.rs`,
  `ridl-backend-rust/tests/interaction_face.rs`,
  `ridl-backend-rust/tests/face_generation.rs`, `xtask/tests/calibrate_cli.rs`,
  and any other hit of the grep in Step 6
- Regenerate: `crates/ridl-backend-rust/tests/generated/interaction_face.rs`
- Test: `crates/ridl-rt/tests/port_forwarding.rs`, a new
  `crates/ridl-rt/tests/trace.rs`

**Interfaces:**

- Produces: `ridl_rt::trace::TraceContext`;
  `Caller::command(&mut self, iface: InterfaceNo, ord: Ordinal, args: &[u8], trace: Option<TraceContext>) -> Result<Correlation, SendError>`;
  `Caller::query(…same…, trace: Option<TraceContext>) -> Result<Correlation, SendError>`;
  `EventSink::raise(&mut self, iface: InterfaceNo, ord: Ordinal, bytes: &[u8], trace: Option<TraceContext>) -> Result<(), RaiseError>`;
  `Claim::trace: Option<TraceContext>`;
  `RawOccurrence::trace: Option<TraceContext>`.
- After this task `ridl-loopback` accepts the argument and delivers `None`
  everywhere (rule 3). Task 2 makes it carry the value.

- [ ] **Step 1: Write the failing tests**

`crates/ridl-rt/tests/trace.rs`:

```rust
use ridl_rt::trace::TraceContext;

#[test]
fn a_trace_context_is_25_bytes_and_its_option_26() {
    assert_eq!(core::mem::size_of::<TraceContext>(), 25);
    assert_eq!(core::mem::size_of::<Option<TraceContext>>(), 26);
}
```

In `crates/ridl-rt/tests/port_forwarding.rs`: make the test double record the
`trace` it receives in `raise`, `command` and `query` (a
`Cell<Option<TraceContext>>` field), and extend
`event_sink_is_reached_through_a_borrow` and
`caller_is_reached_through_a_borrow` so each sends
`Some(TraceContext { trace_id: [1; 16], span_id: [2; 8], flags: 1 })` through
`&mut &mut double` (or the borrow form the test already uses) and asserts the
double recorded exactly that value. Cover `command` and `query` separately.

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p ridl-rt --test trace --test port_forwarding` Expected:
compile error — `ridl_rt::trace` does not exist / wrong number of arguments.

- [ ] **Step 3: Implement the API in `ridl-rt`**

Add `trace.rs` with the struct from Global Constraints and rustdoc that says it
is the W3C `traceparent` layout without the version byte, carried unvalidated.
Change the three trait methods and the two structs as in Interfaces. Forward
`trace` in the `&P` impls. State the four delivery rules of spec §3.4 in the
rustdoc of `Caller`, `EventSink`, `Handler` and `EventSource` (rules 1/3/4 on
calls, 2/3/4 on events). Update `ridl-rt`'s own tests and examples.

- [ ] **Step 4: Run `ridl-rt` tests**

Run: `cargo test -p ridl-rt --all-features` Expected: PASS.

- [ ] **Step 5: Change the emitter**

In `crates/ridl-backend-rust/src/face.rs`, the emitted `port.command(…)`,
`port.query(…)` and `port.raise(…)` calls gain a last argument `None`. No
generated method signature changes. Then regenerate the fixture:
`RIDL_UPDATE_GENERATED=1 cargo test -p ridl-backend-rust --test interaction_face_regeneration`
and check the fixture diff contains only the added `, None` arguments.

- [ ] **Step 6: Fix every remaining implementer, call site and literal**

Find them with
`grep -rnE "fn (command|query|raise)\(|\.(command|query|raise)\(|\b(Claim|RawOccurrence) \{" crates xtask examples --include=*.rs`.
Senders pass `None`; literals add `trace: None`; implementers take the argument
(`ridl-loopback` names it `_trace` for now). Do not change behaviour.

- [ ] **Step 7: Run the full gate**

Run:
`just compile && just test && just lint && just wasm-check && just compat-check && just demo`
Expected: all pass. Report the test count before and after: it must rise by
exactly 1 (the new `trace.rs` test).

- [ ] **Step 8: Commit**

```bash
git add -A crates xtask
git commit -m "feat(ridl-rt)!: carry an optional trace context on calls and events

<body: the type, the argument, the fields; loopback delivers None until the next commit; refs #752>"
```

---

### Task 2: Loopback carries the context, pinned by conformance cases

**Files:**

- Modify: `crates/ridl-rt-conformance/src/calls.rs`,
  `crates/ridl-rt-conformance/src/events.rs`,
  `crates/ridl-rt-conformance/src/lib.rs` (the `suite!` list — the crate's own
  test fails if a `pub fn` case is missing from it)
- Modify: `crates/ridl-loopback/src/store.rs` (`CallEntry`, `QueuedEvent`, the
  `Claim` literal near line 956, the `RawOccurrence` literal near line 526),
  `crates/ridl-loopback/src/lib.rs` and `src/handle.rs` (pass `trace` into the
  store)
- Test: `crates/ridl-loopback/tests/conformance.rs` (runs the suite; no edit
  expected)

**Interfaces:**

- Consumes: everything Task 1 produces.
- Produces: nine conformance cases, all `pub fn …<F: Factory>()`, named exactly:
  - `calls::a_commands_context_arrives_on_its_claim`
  - `calls::a_querys_context_arrives_on_its_claim`
  - `calls::a_call_sent_without_a_context_arrives_without_one`
  - `calls::two_calls_in_flight_each_keep_their_own_context`
  - `calls::an_oversized_claims_context_survives_its_second_presentation`
  - `calls::a_reused_call_slot_does_not_keep_the_previous_context`
  - `events::a_raised_events_context_arrives_on_every_subscribers_occurrence`
  - `events::an_event_raised_without_a_context_arrives_without_one`
  - `events::a_short_buffer_keeps_the_occurrences_context`

- [ ] **Step 1: Write the failing cases**

Model each on its neighbour (`a_command_is_delivered_and_acknowledged`,
`two_sources_each_receive_their_own_copy_of_one_occurrence`,
`an_oversized_claim_is_reported_with_its_id_and_is_not_consumed`,
`a_reclaimed_slots_old_correlation_answers_none`,
`a_short_buffer_leaves_the_occurrence_for_the_next_call`). Use two distinct
contexts,
`A = TraceContext { trace_id: [0xA1; 16], span_id: [0xA2; 8], flags: 1 }` and
`B = TraceContext { trace_id: [0xB1; 16], span_id: [0xB2; 8], flags: 0 }`.
Assertions:

- command / query: `assert_eq!(claim.trace, Some(A))`.
- without a context: command, query and raise each with `None` →
  `assert_eq!(…trace, None)`.
- two in flight: two caller handles (`F::caller`) send A and B before any claim
  is read; the claim whose `envelope.seq` / bytes identify the first call has
  `Some(A)`, the other `Some(B)`.
- oversized: send with A and args larger than the buffer; the first `next_claim`
  is `Err(ShortClaim)`; with a large enough buffer the claim has `Some(A)`.
- reused slot: fill to the reuse condition used by
  `a_reclaimed_slots_old_correlation_answers_none`; the old call carries A, the
  new call in the same slot is sent with `None`; its claim has `None`.
- event to two subscribers: raise with A; both sources' occurrences have
  `Some(A)`.
- short buffer: raise with A; first `next` with a 0-byte buffer is `Err(Short)`;
  the second `next` returns `trace == Some(A)`.

Add all nine to `suite!` in `lib.rs`.

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p ridl-loopback --test conformance` Expected: the cases that
send `Some` FAIL with `left: None, right: Some(…)`; the `None` cases pass.

- [ ] **Step 3: Carry the context in `ridl-loopback`**

Add `trace: Option<TraceContext>` to `CallEntry` and `QueuedEvent`, store the
argument on send and on raise (copied into each subscribed source's queue), and
put it on the `Claim` and `RawOccurrence` the store builds. Replace `_trace`
from Task 1.

- [ ] **Step 4: Run to verify they pass, then the gate**

Run: `cargo test -p ridl-loopback --test conformance -p ridl-rt-conformance`
then `just test && just lint`. Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/ridl-rt-conformance crates/ridl-loopback
git commit -m "feat(ridl-loopback): deliver the sender's trace context on claims and occurrences"
```

The task reviewer mutates `store.rs` in its own worktree: always `None` on the
claim; always `None` on the occurrence; the context of the oldest call put on
every claim. Each mutation must fail at least one new case.

---

### Task 3: The records

**Files:**

- Modify: `docs/decisions/ADR-0021-ridl-rt-0.1-api-and-release.md`,
  `docs/decisions/ADR-0023-interaction-face-generation.md`,
  `docs/decisions/README.md` (only if its ADR summary lists decisions),
  `docs/design/ridl-rt.md`, `docs/technotes/ridl-rt-by-example.md`,
  `docs/specification/frame-specification.md`

**Interfaces:**

- Consumes: the final API from Tasks 1 and 2.

- [ ] **Step 1: ADR-0021**

Add decision 21, "Amendment (2026-10-06) — `trace`: an optional trace context on
calls and events", in the numbered list after decision 20, in the form of
decisions 19 and 20: the type, the argument, the fields, the four delivery
rules, and why `Envelope` is not changed (decision 5). Add a matching
`**Amendment (2026-10-06) — decision 21 …**` paragraph to `## Status`, after the
2026-09-29 one, stating it is a breaking change under decision 10, released with
the workspace as 0.6.0, and citing driftsys/ridl#752. Do not cite the
`docs/wip/` design note from the ADR: gardening adds the link to its archived
path when it moves the note.

- [ ] **Step 2: ADR-0023**

Add an amendment: the generated `Client` methods and event raise pass
`trace: None`; generated method signatures are unchanged; driftsys/ridl#754 owns
generated spans.

- [ ] **Step 3: Design, technote, frame spec**

- `docs/design/ridl-rt.md`: the three signatures, the two fields, the delivery
  rules, and the `trace` module in the module list.
- `docs/technotes/ridl-rt-by-example.md`: update every `command`, `query`,
  `raise` call and every `Claim`/`RawOccurrence` shown.
- `docs/specification/frame-specification.md` §2: add `TraceContext` to the
  names that deliberately do not appear on the frame, with one sentence: a frame
  transport delivers `None`.

- [ ] **Step 4: Run the doc gates**

Run:
`just check && just book-check && just link-check && just doc-path-check && just story-id-check`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add docs
git commit -m "docs(adr): record the trace context on calls and events"
```

---

After Task 3: the whole-branch review (`/review` passes 1 and 2), then
`just verify`, then gardening of this plan and its spec before the PR.
