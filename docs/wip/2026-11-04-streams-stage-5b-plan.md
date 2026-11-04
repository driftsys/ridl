# Streams stage 5b Implementation Plan

> **For agentic workers:** This plan is built by Codex, one task per run, and
> each task is reviewed by Claude, with mutation testing, before the next one
> starts. Read "How to work" before Task 1. Steps use checkbox (`- [ ]`) syntax
> for tracking.

**Goal:** Carry the upload direction of a stream over the WebSocket binding: the
server in `ridl-transport-ws` receives `chunk` frames on a parameter stream,
pushes them to the backend under a per-stream credit, and reports a provider's
refusal on the stream; the TypeScript client gains `commandStream` and
`queryWithUpload`; `Rule::Utf8` gets its wire code 7 in frame version 3.

**Architecture:** Frame version 3 and its three stream forms exist (stage 4).
The server today sends stream 0 and refuses every client `chunk`. This plan
gives the member table the count of parameter streams of each member, gives each
call in flight one `Upload` per parameter stream (a queue of at most 4 chunks,
the credit, the inbound `seq`), and changes the rule that forgets a call: a call
is forgotten when its outcome is settled and each of its uploads has ended or
stopped. The TypeScript `Calls` table gains an upload call whose
`WritableStream` sink sends one `chunk` per `write` while credit lasts.

**Tech Stack:** Rust (`ridl-transport-ws`, edition 2021, `rust-version` 1.88),
`ridl_rt::port::StreamCaller`, `ridl-loopback` as the test backend,
`tungstenite` in the TCP tests, Deno for `packages/ridl-transport-ws`,
`cargo xtask freeze`.

**Spec:** `docs/wip/2026-10-31-streams-design.md` — section 5 (5.1 to 5.6),
section 6 (the server and the TypeScript client, R-18, R-19), section 9 stage 5
(its Tests list is the acceptance list; the parts stage 5a delivered are in "As
built (stage 5a)"), section 10, and the sections "As built (stage 4)" and "As
built (stage 5a)", which take precedence over the earlier sections where they
differ. R-n, S4-n, S5-n and A-n are that document's rulings; B-n are this
plan's.

## Questions for the driver

Each question is answered below with a recommendation, and the plan is written
as if the recommendation is accepted. A different answer changes the tasks
named.

**Q1. Does the server carry a query with an upload and a reply stream (the 5a
`pipe` form) in 5b, and does the TypeScript client get a method for it?**
Recommendation: the server carries it (Task 3); the TypeScript client gets no
method in 5b, and a tracking issue is filed at PR time. Reason: on the server
the duplex call is the composition of the stage 4 reply stream and this plan's
upload, with one extra rule (B-6: when the reply stream ends while the upload is
open, the server sends `stream_end interrupted` on the upload); leaving it out
would need its own refusal path. The design names only two TypeScript methods,
and a third needs a return shape the design does not give
(`{ upload, reply: ReadableStream }` and its cancel rules).

**Q2. How do the loopback_server example and its test provider gain
parameter-stream members?** Recommendation: store and fetch, four new ordinals
in `tests/support/provider.rs` (Task 3), mirrored in `e2e/catalog.ts` (Task 5):

| Ordinal | Name           | Kind    | Streams | Behaviour                                                                                                                                                                                                                                                                                                                                   |
| ------- | -------------- | ------- | ------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 11      | `UPLOAD`       | command | 1       | argument `[key, mode]`; settles `Ok(&[])` at the claim, then pulls stream 1 into `store[key]` (replaced at the claim). `mode` 0 stores; `MODE_REFUSE_RANGE` 1 and `MODE_REFUSE_UTF8` 2 abort stream 1 when the third chunk is pulled, with `Contract(InvalidValue(Violation { type_name: Some("Block"), rule }))`, `rule` `Range` or `Utf8` |
| 12      | `UPLOAD_QUERY` | query   | 1       | argument `[mode]`; pulls stream 1 to its end and settles a 12-byte reply: the byte count as `u64` little-endian, then the wrapping sum of the bytes as `u32` little-endian. Modes 1 and 2 abort stream 1 at the third chunk as `UPLOAD` does, then settle `Err` with the same error                                                         |
| 13      | `FETCH`        | query   | 0       | argument `[key]`; the reply stream is `store[key]` in chunks of `FETCH_CHUNK` = 16,128 bytes (the last one shorter), then the end; an empty or unknown key ends at once                                                                                                                                                                     |
| 14      | `DUPLEX`       | query   | 0 and 1 | pushes each chunk pulled from stream 1 to stream 0 unchanged; ends stream 0 when stream 1 ends                                                                                                                                                                                                                                              |

Reason: the design's acceptance test reads an upload back "through `fetch`", and
a reply stream is already proved by stage 4, so the read-back adds no new path;
an echo would need the duplex form in TypeScript (Q1). The store is keyed by one
byte so that tests sharing one provider in `loopback_server` do not collide.

**Q3. Does the browser end-to-end test change?** Recommendation: yes, one upload
of 200,000 bytes through `commandStream` and its read-back through `FETCH` in
`e2e/page.ts`, with one more field in the result that `browser_test.ts` checks
(Task 5). Reason: stage 4 added `queryStream` to the page, and the browser test
exists to catch differences in the platform's streams; `WritableStream`
backpressure (`desiredSize`, the timing of a sink's `write` promise) is such a
difference. The browser test is CI-only; the driving session runs it locally
only if a Chromium-family browser is installed.

**Q4. Where the design and the as-built code disagree.**

- (a) Design §6 says a client `stream_end` "on a stream it receives" aborts the
  backend stream; as built (S4-7) a client `stream_end end` closes 1002 on every
  stream. Decided: direction-aware (B-5).
- (b) Design §6 lists three `Note` values; `StreamCreditExceeded` and
  `StreamStopped` exist, and `Note::StreamCreditExceeded`'s rustdoc says no
  member has a stream parameter. Decided: add `ChunkTooLarge`; recommendation:
  also rewrite that rustdoc (Task 2).
- (c) R-33 step 3 says the provider "drops every later `chunk` of that stream
  without a note until the consumer's own `stream_end` or the end of the bound".
  The server keeps no state for a forgotten call. Recommendation: B-4's rule "a
  `chunk` whose `correlation` names no call in flight is dropped and records no
  note" gives the same behaviour with no timer; a refused upload of a call still
  in flight is marked `Stopped` and its chunks are dropped (B-6).
- (d) R-17 says the retention clock "runs from the receipt of the consumer's
  `stream_end end`", where today's clock is
  `min(request.stamp, receipt) +
  max`. Recommendation:
  `min(end.stamp, end receipt) + max` (B-8), the same formula over the
  `stream_end`'s envelope, so a client stamp ahead of the server's clock is
  still capped.
- (e) Decoding code 7. `frame::decode` is stateless, and today 7 is an unknown
  `detail` in every version (`tests/decode_errors.rs` asserts it). The design
  says the code arrives with version 3. Recommendation: the codecs decode 7 as
  `Utf8` in every version, and the sender gates it: the server's
  `carried(c,
  frame_version)` sends `Utf8` as `corrupt` with
  `Note::RuleNotCarried` on a session below version 3 (B-11). Reason: version
  gating in the decoder would need a version parameter on `decode` (a break of
  the frozen crate), and a server below version 3 never has a text stream to
  refuse.
- (f) R-33 step 5 says `close` rejects after a refusal. After the client's
  `stream_end end`, the server pushes the end and has nothing more to push, so
  it never learns of a refusal of the last chunks; and no frame confirms an
  upload's end. Recommendation: `close` resolves once `stream_end end` is sent
  (B-13); a refusal that the server sends before it receives the end still
  rejects `close`. State the limit in D-28 and D-21 (ADR-0032 note (a)).

**Q5. How does `write` size a chunk?** The design is silent. Recommendation: one
`write` is one `chunk`; a `write` longer than the member's limit rejects with
`SessionError` `corrupt`, errors the upload, and sends `stream_end interrupted`
on stream 1 (B-12). Reason: the transport does not know whether the stream
carries named elements, and splitting would break an element across chunks; the
codec of stage 6 writes one element per `write`, and a byte writer of stage 6
can cut at the limit. This matches `queryStream`'s "arguments longer than the
limit are `corrupt`".

## How to work

- Work in the worktree `../ridl-streams-5b` (branch `feat/streams-stage-5b`). Do
  not push. Do not open a pull request.
- Do **one task**, then stop. Leave the changes uncommitted: the sandbox cannot
  commit, and the driving session commits with the message the task gives.
- Before stopping, write `.superpowers/codex/task-<N>-report.md` (gitignored):
  the files changed, each test added with one line on what it pins, the commands
  run and their result, the tests you wrote but could not run (TCP), and every
  choice this plan did not decide, each on a line starting with `Ruling:`.
- Test first: write each test, run it, and see it fail on an assertion (not on a
  compile error) before writing the code that makes it pass. Where a test needs
  a new type to compile, add the type with a body of `todo!()` first.
- The sandbox cannot bind TCP. Tests in
  `crates/ridl-transport-ws/tests/serve.rs` and under
  `packages/ridl-transport-ws/e2e/` bind sockets: write them, compile them
  (`cargo test -p ridl-transport-ws --no-run`, `deno check e2e/*.ts`), and list
  them in the report; the driving session runs them. Session unit tests in
  `src/server/` and the Deno tests under `tests/` use no socket; run them.
- Run only focused tests:
  `cargo test -p ridl-transport-ws --all-features
  --locked --lib`,
  `cargo clippy -p ridl-transport-ws --all-targets
  --all-features -- -D warnings`,
  and in `packages/ridl-transport-ws` `deno task test` (or `deno test tests/`),
  `deno check mod.ts`, `deno lint`, `deno fmt --check`. CI runs the full gate.
- Every test below names the **mutation** it must catch. The reviewer applies
  each mutation and expects that test to fail; a test that stays green fails the
  gate.
- Read `AGENTS.md` once. Prose is plain and literal; shipped docs, rustdoc and
  TSDoc name no story id, stage or epic (`just story-id-check`).

## Global Constraints

The decisions already taken, verbatim:

- Frame version 3 is unreleased (latest tag v0.14.0 predates it), so `Utf8` is
  rule code 7 of version 3 and the vector is additive to the `wire` freeze
  surface; no new frame version.
- The design's
  `commandStream(iface, ord, args, { bound }): { upload: WritableStream<Uint8Array>; accepted: Promise<void> }`
  and
  `queryWithUpload(iface, ord, args, { bound }): { upload: WritableStream<Uint8Array>; reply: Promise<Uint8Array> }`
  are the TS surface. The WritableStream's write waits while credit is spent;
  close sends stream_end end; abort sends stream_end interrupted.
- Server: credit 4 initial, one per accepted chunk (R-16); a client chunk beyond
  credit is StreamCreditExceeded 1002; a chunk longer than the member's limit is
  answered stream_end corrupt and the call forgotten (Note::ChunkTooLarge,
  added); after a provider refusal (R-33 step 3) the server sends stream_end
  <outcome> on stream k and drops later chunks of k until the client's
  stream_end or the bound; a command with an upload stays open after its
  acknowledgment while the upload is open; a query with an upload and a plain
  reply measures bound, retention and abandon from the client's stream_end end
  (R-17); the client's stream_end end on stream k >= 1 is the legitimate close
  (S4-7: make the stream_end check direction-aware).
- The 5a face rulings bind the provider side the server drives (see As built
  (stage 5a)).

And the constraints of the binding:

- Version 1 and version 2 layouts, codes and vectors are unchanged; the `wire`
  dump gains exactly one line (`stream-end-utf8`).
- `ridl-transport-ws` changes are additive (`runtime-crates` row): `Note` is
  `#[non_exhaustive]`; `MemberTable` gains methods; no public signature changes.
  `cargo xtask freeze check` passes.
- `ridl-rt`, `ridl-loopback`, the IR, the catalog descriptor and every catalog
  hash are unchanged, except the rustdoc of `ridl_rt::payload::Rule` (Task 1).
- Header sizes are stage 4's: `chunk` 35, `stream_end` 37, `stream_credit` 23.
  `MAX_MESSAGE_LIMIT` stays 16,164. `INITIAL_CREDIT` stays 4.
- A parameter stream's number `k` is 1 to the member's count of stream
  parameters; the TypeScript methods use stream 1 only (S5-3).
- A `stream_end` the server sends on stream `k >= 1` takes `seq` 1, because the
  server sends no chunk in that direction.
- Every vector stamp follows the version 1 table: `0x0123456789AC` on a
  provider-sent frame.

## Rulings taken in this plan

- **B-4. A client `chunk` on stream `k >= 1`.** Its `correlation` names no call
  in flight: dropped, no note. The call's member declares fewer than `k` stream
  parameters: `Note::StreamCreditExceeded`, close 1002 (no credit was granted).
  The upload's credit is 0: `Note::StreamCreditExceeded`, close 1002. The upload
  is `Stopped`: dropped, no note. Its `seq` is not the next one: the upload is
  interrupted (`abort(c, k, Interrupted)`, `stream_end interrupted` on `k`), no
  note. Its payload is longer than the member's limit: `Note::ChunkTooLarge`,
  `stream_end corrupt` on `k`, the call forgotten. A `chunk` on stream 0 stays
  `UnexpectedDirection`.
- **B-5. A client `stream_end` is direction-aware.** On stream 0 the stage 4
  rule holds. On stream `k >= 1` of an open upload, `end` ends it (pushed after
  the queued chunks) and `interrupted` aborts it with `Transport::Interrupted`;
  `contract`, `corrupt` and `busy` close 1002 with `OutcomeNotAllowed`, because
  the producer of a stream ends or interrupts it and gives no verdict on it. A
  `stream_end` whose `seq` is not the next one interrupts the upload as B-4. On
  a stream the call does not have, or for a call not in flight, it is ignored. A
  client `stream_credit` on stream `k >= 1` is ignored (S4-7 unchanged).
- **B-6. When a call is forgotten.** A call is forgotten when its outcome is
  settled (a `response` sent, or its reply stream ended) and each upload is
  `Ended` or `Stopped`. A refusing `response` (`contract`, `corrupt`, `busy`)
  forgets the call at once and sends no `stream_end` on its uploads. Any other
  forget while an upload is `Open` (a reply or a reply stream's end before the
  upload's end, abandonment, the client's stop of stream 0) sends
  `stream_end interrupted` on that upload; `ChunkTooLarge` sends
  `stream_end corrupt`. A provider refusal (`PushError::Failed(e)` on a push or
  an end) sends `stream_end <stream_outcome(e)>` on `k`, drops the queue, and
  marks the upload `Stopped`.
- **B-7. Abandonment of an open upload.** Its clock restarts at each `chunk`
  received and at each drain that the backend took a chunk from. When
  `call_abandon_after` passes, the server sends `stream_end interrupted` on the
  upload, aborts it on the backend, forgets the call and records
  `Note::CallAbandoned`. A command whose upload has ended and whose
  acknowledgment is pending is abandoned from `sent`, as today.
- **B-8. R-17 on the server.** When the client's `stream_end end` of a query
  with an upload arrives, the call's `sent` becomes that `Instant`, its `stamp`
  the `stream_end`'s envelope stamp and its `receipt` the backend's clock, so
  `retention` and the abandon bound run from the end with the existing formulas.
- **B-9. Credit.** Each drain of an upload's queue into the backend that took
  `n > 0` chunks sends one `stream_credit` of `n` on `k`. A chunk is drained at
  once when it arrives, and again on each tick. An empty chunk counts as one
  chunk.
- **B-10. A member with a parameter stream on a session below version 3** is
  answered `response contract(UnknownInteraction)` with
  `Note::UnknownInteraction`, as S4-9 does for a streamed reply.
- **B-11. `Utf8` on the wire.** `rule_tag(Utf8) = Some(7)` and
  `rule_from(7) = Utf8` in every version; `carried(c, frame_version)` is false
  for `InvalidValue(Utf8)` below version 3. TypeScript decodes 7 as `"utf8"`.
- **B-12. One `write` is one `chunk`** (Q5).
- **B-13. `close` resolves once `stream_end end` is sent** (Q4 (f)). For a
  command the call is then forgotten on the client if `accepted` has settled;
  otherwise it waits for its `response`.
- **B-14. TypeScript failure rules of an upload.** A server `stream_end` on
  stream 1 errors the upload with its failure (`contract` with the contract,
  `corrupt`, `interrupted`), rejects `accepted` or `reply` with the same failure
  when still pending, and forgets the call; a resolved `accepted` stays
  resolved. A `write` waiting for credit rejects at once. A refusing `response`
  rejects `accepted` or `reply` and errors the upload with the same failure.
  `abort` of a `queryWithUpload` rejects `reply` with `interrupted` and forgets
  the call; `abort` of a `commandStream` leaves `accepted` to its `response`.
  The session's end errors the upload and rejects with `down`.
- **B-15. TypeScript bounds.** `commandStream`'s `accepted` rejects
  `undelivered` when `bound` passes after the `request` with no `response`.
  `queryWithUpload` starts its timer when `close` sends `stream_end end`, and
  `reply` rejects `timeout` when `bound` passes after that with no `response`;
  before `close` it has no timer (the server's abandonment bounds an idle
  upload).
- **B-16. Review:** the per-task gate with mutation by the reviewer, then
  `/review` passes 1 and 2 over the branch.

## Review Focus

1. **A client that writes faster than the provider reads.** The server holds at
   most 4 chunks per upload, grants no credit beyond them, and a signal and
   another call's response on the same session still flow. Pinned in Task 2
   (`the_server_grants_one_credit_per_chunk_the_backend_takes`) and Task 3
   (`a_stalled_upload_does_not_block_a_signal`).
2. **The end arrives while chunks are still queued at the server.** The end is
   pushed only after the queue is drained, and the provider reads every byte,
   then `End`. Pinned in Task 2 (`the_end_is_pushed_after_the_queued_chunks`)
   and by the 200,000-byte read-back of Tasks 3 and 5.
3. **The session ends during an upload.** The provider reads
   `Failed(Interrupted)`; the TypeScript upload errors `down`. Pinned in Task 2
   (`the_end_of_a_session_interrupts_an_open_upload`) and Task 4.
4. **A refusal while a `write` waits for credit.** The pending `write` rejects
   with the refusal; it does not wait for ever. Pinned in Task 4
   (`a_refusal_rejects_a_write_waiting_for_credit`).
5. **A retransmitted `request` during an upload.** It is `DuplicateDropped` and
   the upload continues. Pinned in Task 2
   (`a_retransmitted_upload_request_is_dropped_and_the_upload_continues`).

---

### Task 1: `Utf8` as rule code 7, its vector, and the sender's version gate

**Files:**

- Modify: `crates/ridl-transport-ws/src/frame.rs` (`rule_tag`, `rule_from`,
  their rustdoc), `src/server/session.rs` (`carried`, its three call sites)
- Modify: `crates/ridl-rt/src/payload.rs` (rustdoc of `Rule` and `Rule::Utf8`
  only)
- Modify: `crates/ridl-transport-ws/tests/vectors.rs`, `tests/decode_errors.rs`,
  `tests/codes.rs`
- Modify: `packages/ridl-transport-ws/src/frame.ts` (`Rule`, `RULES`),
  `tests/vectors_test.ts`, `tests/decode_errors_test.ts`, `tests/codes_test.ts`
- Modify: `docs/specification/frame-specification.md` §9.6 ("The `Rule` codes")
  and §11.1 "Version 3 test vectors"
- Modify: `freeze/wire.txt` (by `cargo xtask freeze update`)

**Interfaces:**

- Produces: `frame::rule_tag(Rule::Utf8) == Some(7)`; `Contract::InvalidValue`
  with `Rule::Utf8` decodes from `detail` 7; TS `type Rule = ... | "utf8"`, with
  `RULES[6] === "utf8"`.
- Produces: `fn carried(c: Contract, frame_version: u32) -> bool` in
  `session.rs` (private); every caller passes `self.frame_version`.

- [ ] **Step 1: Write the specification rows.** In §9.6, the `Rule` codes become
      "`Range` 1, `Step` 2, `Length` 3, `Pattern` 4, `Variant` 5, `Unique` 6,
      and, from frame version 3, `Utf8` 7, for a chunk of a `<string>` stream
      that is not valid UTF-8 (ADR-0032 decision 12). A sender does not send 7
      on a session below version 3." Keep the sentence on why a rule receives
      its code with a version, and add that invalid UTF-8 inside a payload stays
      `corrupt` (design §12). Add the row after `stream-credit`:

  | Id                | Vector                                                                                                       | Hex                                                                                   |
  | ----------------- | ------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------- |
  | `stream-end-utf8` | `stream_end` `contract(InvalidValue(Utf8))`, command, interface 1, ordinal 3, seq 1, correlation 2, stream 1 | `0d 03 01000000 03000000 ac89674523010000 0100000000000000 0200000000000000 01 03 07` |

- [ ] **Step 2: Write the failing Rust tests.** In `tests/vectors.rs`, add
      `"stream-end-utf8"` to `KNOWN_3` and its frame to `expected_3`:
      `Frame::StreamEnd { target: Target { kind: Command, iface: 1, ord: 3 },
      envelope: { stamp: 0x0123456789AC, seq: 1 }, correlation: 2, stream: 1,
      outcome: StreamOutcome::Contract(Contract::InvalidValue(Violation {
      type_name: None, rule: Rule::Utf8 })) }`.
      In `tests/decode_errors.rs`, change the two `[0, 7]` loops
      (`an_unknown_rule_is_an_unknown_code`,
      `stream_end_interrupted_with_nonzero_detail_is_refused`) to `[0, 8]`. In
      `tests/codes.rs`, add `the_rule_codes_are_1_to_7` asserting `rule_tag` of
      each of the seven rules is 1 to 7 in declaration order and that a response
      with `detail` 7 decodes to `Rule::Utf8`. In `session.rs` tests, add
      `a_utf8_violation_is_carried_from_version_3_only`: a query whose provider
      settles
      `Err(Contract(InvalidValue(Violation { type_name:
      Some("Text"), rule: Utf8 })))`
      is answered `response contract` with `detail` 7 on a version 3 session,
      and `response corrupt` with `Note::RuleNotCarried` on a version 2 session.
      Mutation: drop the version check in `carried` (the version 2 case fails);
      map `Utf8` to 6 (the vector fails).
- [ ] **Step 3: Run them and confirm they fail.**
      `cargo test -p ridl-transport-ws --all-features --test vectors --test decode_errors --test codes --lib`.
      Expected: the vector, code and session tests fail on assertions.
- [ ] **Step 4: Implement** `rule_tag`, `rule_from` and `carried` as in
      Interfaces. Rewrite the rustdoc that says `Rule::Utf8` has no wire code,
      in `frame.rs` and `ridl-rt`'s `payload.rs`: `Utf8` has code 7 from frame
      version 3.
- [ ] **Step 5: Run the Rust tests and confirm they pass**, with the same
      command, then `cargo xtask freeze update` and check that
      `git diff freeze/wire.txt` adds exactly one `frame-vector` line.
- [ ] **Step 6: TypeScript.** Add `"utf8"` to `Rule` and `RULES`. Mirror Step 2
      in `vectors_test.ts` (`KNOWN_3` and the switch), `decode_errors_test.ts`
      (`[0, 8]`) and `codes_test.ts`. Run `deno test tests/` and
      `deno check
      mod.ts`; they pass after the change and the vector test
      fails before it.
- [ ] **Step 7: Commit** (driver):
      `feat(ridl-transport-ws): carry the UTF-8 rule as detail 7 of frame version 3`.

### Task 2: The server receives an upload of a command

**Files:**

- Create: `crates/ridl-transport-ws/src/server/upload.rs` (the state of one
  upload and its drain into the backend, with its unit tests)
- Create: `crates/ridl-transport-ws/src/server/session/tests/upload.rs` (session
  tests, declared as `mod upload;` inside `mod tests` of `session.rs`)
- Modify: `src/server/session.rs` (`handle`'s `Chunk` and `StreamEnd` arms,
  `request`, `poll_calls`, `stop_stream`, `close`), `src/server/calls.rs`
  (`InFlight`), `src/server/members.rs`, `src/server/descriptor.rs`,
  `src/server/config.rs` (`Note`), `src/server.rs` (module list)
- Modify: `crates/ridl-transport-ws/tests/descriptor.rs`

**Interfaces:**

- Consumes: `INITIAL_CREDIT`, `Frame::{Chunk, StreamEnd, StreamCredit}`,
  `Backend::streams()`, `StreamCaller::{push, end, abort}`.
- Produces (public, additive):
  - `MemberTable::set_parameter_streams(&mut self, iface: InterfaceNo, ord: Ordinal, count: u8)`
    — stores nothing for a key the table does not hold; a later `insert` clears
    it. `MemberTable::parameter_streams(&self, iface, ord) -> u8`, 0 for none or
    an unknown key. `from_descriptor` sets the count to the largest stream `no`
    above 0.
  - `Note::ChunkTooLarge { seq: u64, stream: u8, len: usize }` — `seq` is the
    request's, `len` the payload's length. `Note::StreamCreditExceeded`'s
    rustdoc: a `chunk` beyond the credit granted on a stream parameter, or on a
    stream the member does not declare.
- Produces (crate-private, `server/upload.rs`):

  ```rust
  pub(super) enum UploadState { Open, Ended, Stopped }
  pub(super) struct Upload {
      pub(super) no: StreamNo,
      pub(super) seq_in: u64,          // seq of the last frame received, 0 before
      pub(super) credit: u32,          // chunks the client may still send
      pub(super) queue: VecDeque<Vec<u8>>, // received, not taken by the backend
      pub(super) end_received: bool,   // pushed as `end` once `queue` is empty
      pub(super) state: UploadState,
      pub(super) last_activity: Instant,
  }
  pub(super) enum Receive { Queued, Gap, CreditExceeded, Dropped }
  pub(super) struct Drain { pub(super) taken: u32, pub(super) end: Option<DrainEnd> }
  pub(super) enum DrainEnd { Ended, Refused(CallError), Detached }
  impl Upload {
      pub(super) fn new(no: StreamNo, now: Instant) -> Self;
      pub(super) fn receive_chunk(&mut self, seq: u64, payload: &[u8], now: Instant) -> Receive;
      pub(super) fn receive_end(&mut self, seq: u64, now: Instant) -> Receive;
      pub(super) fn drain(&mut self, c: Correlation, ports: &mut dyn StreamCaller, now: Instant) -> Drain;
  }
  ```

  `drain` pushes from the front of `queue` until `push` answers 0 or the queue
  is empty, adds `taken` to `credit`, then calls `end` when `end_received` and
  the queue is empty. `PushError::Failed(e)` is `Refused(e)`,
  `PushError::Contract(c)` is `Refused(CallError::Contract(c))`,
  `PushError::TooLarge { .. }` is `Refused(Transport(Corrupt))`,
  `PushError::Ended` is `Refused(Transport(Interrupted))`, `Detached` is
  `Detached` (the session closes 1001).
- `InFlight` gains `uploads: Vec<Upload>` (one per stream 1 to the member's
  count, made in `request`) and `settled: bool`.

- [ ] **Step 1: Write the failing unit tests of `upload.rs`** (over a `Loopback`
      with one command claimed and not pulled; window 4):
  - `a_fifth_chunk_without_credit_is_credit_exceeded` — four `Queued`, then
    `CreditExceeded`. Mutation: initial credit 5.
  - `a_chunk_taken_by_the_backend_returns_one_credit` — after four chunks and a
    drain, `taken == 4` and `credit == 4`. Mutation: add credit on receipt.
  - `a_full_backend_window_keeps_the_chunk_queued` — with four chunks in the
    window, a fifth received after a drain stays in `queue`, `taken == 0`.
    Mutation: drop a chunk that `push` answered 0 for.
  - `a_seq_gap_is_reported` — `seq` 1 then 3 is `Gap`; a `stream_end` with `seq`
    3 after chunk 1 is `Gap`. Mutation: accept any `seq`.
  - `the_end_is_pushed_after_the_queued_chunks` — six chunks with the provider
    pulling two between drains and `end_received` set after the sixth: the
    provider pulls the six chunks in order, then `Pull::End`; `end` is reached
    only on the drain that empties the queue. Mutation: push `end` while the
    queue holds chunks.
  - `a_receiver_refusal_is_refused_with_its_error` — the provider aborts with
    `Contract(InvalidValue(Range))`; the next drain ends
    `Refused(Contract(InvalidValue(..Range..)))`. Mutation: map `Failed(e)` to
    `Interrupted`.
- [ ] **Step 2: Write the failing session tests** in `session/tests/upload.rs`,
      over `Described<Streaming<Scanning<Loopback>>>` with a table holding
      `UPLOAD` (command, bound 1 s, `set_parameter_streams(.., 1)`) and a plain
      `COMMAND`; the provider handle claims and settles by hand:
  - `a_command_upload_is_acknowledged_and_stays_open` — request, the provider
    settles `Ok(&[])`, tick: `response accepted`; three chunks of `[1]`, `[2]`,
    `[3]` with `seq` 1 to 3, then `stream_end end` with `seq` 4: the provider
    pulls the three chunks and then `End`. Mutation: forget the call at the
    acknowledgment (the provider's pull answers `Failed(Interrupted)`).
  - `the_server_grants_one_credit_per_chunk_the_backend_takes` — the provider
    pulls nothing: four chunks give `stream_credit` frames on stream 1 whose
    credits sum to 4; four more give none; a ninth closes 1002 with
    `StreamCreditExceeded { seq: 1 }`. Variant: the provider pulls one chunk and
    a tick sends one `stream_credit` of 1. Mutation: grant on receipt.
  - `a_chunk_over_the_member_limit_is_stream_end_corrupt_and_forgotten` — limit
    4, a chunk of 5 bytes: one `stream_end corrupt` on stream 1 with `seq` 1,
    `Note::ChunkTooLarge { seq: 1, stream: 1, len: 5 }`; the provider's pull
    reads `Failed(Interrupted)`; a later chunk sends nothing and records
    nothing. A chunk of exactly 4 bytes is queued. Mutation: `>=` for `>`; send
    the `stream_end` on stream 0.
  - `a_provider_refusal_after_the_acknowledgment_is_stream_end_contract_on_the_upload`
    (R-33 step 3) — after `accepted` and two chunks pulled, the provider aborts
    stream 1 with `Contract(InvalidValue(Range))`; chunk 3 is answered
    `stream_end contract(InvalidValue(Range))` on stream 1, `seq` 1; chunks 4 to
    6 and a `stream_end end` send nothing, record nothing and do not close.
    Mutation: close 1002 on the later chunks; send no `stream_end`.
  - `a_client_stream_end_on_an_upload_is_direction_aware` — on stream 1, `end`
    continues and the provider reads `End`; `interrupted` continues and the
    provider reads `Failed(Interrupted)`; `contract`, `corrupt` and `busy` each
    close 1002 with `Violation(OutcomeNotAllowed)`. The stage 4 tests of stream
    0 pass unchanged. Mutation: keep the stage 4 rule that `end` closes on every
    stream.
  - `a_chunk_for_a_call_not_in_flight_is_dropped_without_a_note` — correlation
    99, and a chunk after a refusing response: nothing sent, no note, no close.
    Mutation: `StreamCreditExceeded` for an unknown correlation.
  - `a_chunk_on_a_stream_the_member_does_not_declare_closes_1002` — `UPLOAD`
    with stream 2, and `COMMAND` with stream 1: `StreamCreditExceeded`, close
    1002. Mutation: create uploads for every stream number.
  - `a_chunk_seq_gap_interrupts_the_upload` — `seq` 1 then 3: one
    `stream_end interrupted` on stream 1; the provider reads
    `Failed(Interrupted)`; no note. Mutation: queue the chunk after a gap.
  - `an_idle_open_upload_is_abandoned_interrupted` — after `accepted`, a chunk
    at `t0 + 50 s` and a tick at `t0 + 60 s` (`call_abandon_after` 60 s) send
    nothing; a tick at `t0 + 110 s` sends `stream_end interrupted` on stream 1
    and records `CallAbandoned { seq: 1 }`. Mutation: measure from `sent`.
  - `a_refusing_acknowledgment_forgets_the_call_and_sends_no_stream_end` — the
    provider settles `Err(Contract(PreconditionFailed))`: exactly one
    `response contract`, and no `stream_end`. Mutation: B-6's interrupted
    `stream_end` on every forget.
  - `the_end_of_a_session_interrupts_an_open_upload` — `close()` during an open
    upload: the provider reads `Failed(Interrupted)`. Mutation: `close` skips
    calls with uploads.
  - `a_retransmitted_upload_request_is_dropped_and_the_upload_continues` — the
    same `seq` again while open: `DuplicateDropped`; the next chunk is queued.
    Mutation: reset the upload on a repeat.
  - `a_v2_session_answers_an_upload_member_unknown_interaction` (B-10) —
    `response contract(UnknownInteraction)`, `Note::UnknownInteraction`.
    Mutation: send the request to the backend.
- [ ] **Step 3: Add `the_table_reads_a_parameter_stream`** to
      `tests/descriptor.rs`: a member with stream entries 0 and 1 gives
      `reply_stream == true` and `parameter_streams == 1`; one with entries 1
      and 2 gives 2. Mutation: count entries instead of taking the largest `no`.
- [ ] **Step 4: Run them and confirm they fail** on assertions:
      `cargo test -p ridl-transport-ws --all-features --lib --test descriptor`.
- [ ] **Step 5: Implement** the items of Interfaces and B-4 to B-7, B-9 and
      B-10. `stop_stream` dispatches on `stream`: 0 is today's body, `k >= 1`
      follows B-5. `poll_calls` drains every open upload of every call before it
      reads the call's outcome, applies B-6 to decide whether to forget, and
      checks B-7. Keep `CHUNKS_PER_TICK` for the reply stream; a drain is
      bounded by the queue (at most 4).
- [ ] **Step 6: Run the tests and confirm they pass**, then
      `cargo clippy -p ridl-transport-ws --all-targets --all-features -- -D warnings`
      and `cargo xtask freeze check`.
- [ ] **Step 7: Commit** (driver):
      `feat(ridl-transport-ws): receive the parameter stream of a command`.

### Task 3: Queries with an upload, the duplex query, and the test provider

**Files:**

- Modify: `crates/ridl-transport-ws/src/server/session.rs`,
  `src/server/session/tests/upload.rs`
- Modify: `crates/ridl-transport-ws/tests/support/provider.rs` (the members of
  Q2), `examples/loopback_server.rs` (its module rustdoc), `tests/serve.rs`

**Interfaces:**

- Consumes: Task 2's `Upload`, `InFlight::{uploads, settled}`,
  `MemberTable::{set_parameter_streams, parameter_streams}`.
- Produces (test support, `tests/support/provider.rs`):
  `UPLOAD: Ordinal =
  Ordinal(11)`, `UPLOAD_QUERY = Ordinal(12)`,
  `FETCH = Ordinal(13)`, `DUPLEX = Ordinal(14)`, `MODE_REFUSE_RANGE: u8 = 1`,
  `MODE_REFUSE_UTF8: u8 =
  2`, `FETCH_CHUNK: usize = 16_128`; `member_table()`
  holds the four with a bound of 1 s; `provider()` serves them; `Streams` gains
  the open uploads, the open fetches and `store: BTreeMap<u8, Vec<u8>>`. Task 5
  mirrors these values.

- [ ] **Step 1: Write the failing session tests** (table: `UPLOAD_QUERY` query,
      bound 1 s, stream 1; `DUPLEX` query, bound 1 s, streams 0 and 1):
  - `a_query_upload_replies_after_the_end` — chunks, `stream_end end`, the
    provider settles a reply: one `response reply` with its bytes, and the call
    is forgotten. Mutation: answer the reply while the upload is open with no
    `stream_end`.
  - `a_query_upload_retention_runs_from_the_end` — request at backend time `T`;
    `advance(5 s)`; `stream_end end` stamped `T + 5 s`; the provider replies; at
    `T + 5.5 s` a repeat of the `request` gets the kept response again; at
    `T + 6.1 s` it is `DuplicateDropped`. Mutation: keep the request's stamp and
    receipt (the repeat at `T + 5.5 s` is dropped).
  - `a_query_upload_abandon_runs_from_the_end` — `call_abandon_after` 60 s; a
    chunk every 50 s up to `t0 + 100 s`, the end at `t0 + 100 s`, no reply: a
    tick at `t0 + 159 s` sends nothing, at `t0 + 160 s` records `CallAbandoned`
    and sends nothing. Mutation: measure from `sent` of the request.
  - `a_query_upload_refusal_reaches_the_upload_and_the_reply` — the provider
    aborts stream 1 with
    `Contract(InvalidValue(Violation { type_name:
    Some("Text"), rule: Utf8 }))`
    and settles `Err` with the same error: the next chunk is answered
    `stream_end contract` with `detail` 7 on stream 1, and a tick sends
    `response contract` with `detail` 7; no other frame. Mutation: forget at the
    `stream_end` (no `response` is sent).
  - `a_query_upload_reply_before_the_end_interrupts_the_upload` — the provider
    replies `Ok` while the upload is open: `response reply`, then
    `stream_end interrupted` on stream 1. Mutation: B-6's no-`stream_end` rule
    applied to a `reply`.
  - `a_duplex_reply_starts_before_the_upload_ends` — `DUPLEX`: chunk `[9]`, the
    provider pushes `[9]` to stream 0, a tick sends `chunk` `[9]` on stream 0
    before any client `stream_end`. Mutation: poll the reply stream only after
    the upload ended.
  - `a_duplex_reply_end_with_the_upload_open_interrupts_the_upload` — the
    provider ends stream 0 while stream 1 is open: `stream_end end` on stream 0,
    then `stream_end interrupted` on stream 1, and the call is forgotten.
    Mutation: forget with no `stream_end` on stream 1.
- [ ] **Step 2: Run them and confirm they fail:**
      `cargo test -p ridl-transport-ws --all-features --lib`.
- [ ] **Step 3: Implement** B-8 in the `stream_end end` path of an upload of a
      query; the duplex call is a call with both `stream: Some(ReplyStream)` and
      an upload, and `poll_stream`'s end applies B-6.
- [ ] **Step 4: Run them and confirm they pass**, with the same command.
- [ ] **Step 5: Extend the test provider** with the members of Q2. `UPLOAD`
      stores into `store[key]`, replacing the value at the claim; the upload of
      `UPLOAD_QUERY` with a refusing mode aborts stream 1, then settles `Err`
      (A-4 and 5a's "both upload-query refusal paths abort stream 1 before
      settling"). Update `loopback_server.rs`'s rustdoc to name the members.
- [ ] **Step 6: Write the TCP tests in `tests/serve.rs`** (compile only; the
      driver runs them):
  - `an_upload_of_200_000_bytes_is_stored_and_fetched_back` — `UPLOAD` with
    `[1, 0]`, `pattern(200_000)` in chunks of 16,128 bytes honouring the credit,
    then `FETCH` `[1]`: the concatenated reply equals the input.
  - `a_refused_third_block_sends_stream_end_contract_on_stream_1` — `UPLOAD`
    `[2, MODE_REFUSE_RANGE]`, five chunks: `response accepted`, then
    `stream_end contract(InvalidValue(Range))` on stream 1; the session stays
    open (a following `QUERY` is answered).
  - `an_oversized_chunk_is_answered_stream_end_corrupt` — a chunk of 16,129
    bytes on `UPLOAD`: `stream_end corrupt` on stream 1, `Note::ChunkTooLarge`.
  - `a_query_upload_replies_the_count_and_the_sum` — `UPLOAD_QUERY` `[0]`,
    `pattern(200_000)`: the reply is `200_000u64.to_le_bytes()` followed by the
    wrapping `u32` sum of `pattern(200_000)`.
  - `a_stalled_upload_does_not_block_a_signal` — a held provider (the stage 4
    `Stepped::held`), eight chunks on `UPLOAD`, then a `subscribe` of `SIGNAL`
    is answered and a `publish` arrives.
- [ ] **Step 7: Compile** with
      `cargo test -p ridl-transport-ws --all-features --no-run` and clippy. The
      driver runs `cargo test -p ridl-transport-ws --all-features --test serve`.
- [ ] **Step 8: Commit** (driver):
      `feat(ridl-transport-ws): receive the parameter stream of a query`.

### Task 4: `commandStream` and `queryWithUpload` in the TypeScript client

**Files:**

- Modify: `packages/ridl-transport-ws/src/calls.ts` (the upload call,
  `onStream`, `settle`, `fail`), `src/session.ts` (the two methods, the `chunk`
  and `stream_end` senders), `mod.ts` (exports, if a new type is public)
- Modify: `packages/ridl-transport-ws/tests/calls_test.ts`,
  `tests/session_test.ts`

**Interfaces:**

- Consumes: Task 1's `"utf8"` rule; the frame forms of `frame.ts`;
  `INITIAL_CREDIT`.
- Produces (public, `Session`):

  ```ts
  commandStream(iface: number, ord: number, args: Uint8Array, opts: { bound: number }):
    { upload: WritableStream<Uint8Array>; accepted: Promise<void> };
  queryWithUpload(iface: number, ord: number, args: Uint8Array, opts: { bound: number }):
    { upload: WritableStream<Uint8Array>; reply: Promise<Uint8Array> };
  ```

  Before sending, on a session of frame version 1 or 2 both reject
  `unsupported`, and arguments longer than the member's limit reject `corrupt`,
  a bound or numbers `command` refuses throw a `RangeError` into the promise; in
  each case `upload` is errored with the same error and nothing is sent.
- Produces (`calls.ts`): `export const UPLOAD_STREAM = 1;`

  ```ts
  export interface UploadSend {
    request(seq: bigint): void;
    chunk(seq: bigint, streamSeq: bigint, payload: Uint8Array): void;
    end(seq: bigint, streamSeq: bigint, outcome: { kind: "end" } | { kind: "interrupted" }): void;
  }
  Calls.commandUpload(bound: number, limit: number, send: UploadSend):
    { upload: WritableStream<Uint8Array>; accepted: Promise<void> };
  Calls.queryUpload(bound: number, limit: number, send: UploadSend):
    { upload: WritableStream<Uint8Array>; reply: Promise<Uint8Array> };
  ```

  The upload call keeps `granted` (`INITIAL_CREDIT` plus every server
  `stream_credit` on stream 1), `sent`, the next stream `seq` (from 1), and at
  most one `write` waiting for credit. `onStream` on an upload call: a
  `stream_credit` on stream 1 adds credit and releases the waiting `write`; a
  `stream_end` on stream 1 with `seq` 1 applies B-14, with another `seq` errors
  the upload `interrupted`; a `chunk` on any stream, or any frame on stream 0,
  is `unexpected-form`.

- [ ] **Step 1: Write the failing tests** in `calls_test.ts` (FakeSocket, a
      version 3 session, `BOUND = { bound: 20 }`, the `UPLOAD` target command
      ordinal 11 and the query ordinal 12):
  - `commandStream_sends_the_request_then_chunks_on_stream_1_from_seq_1` — two
    writes give a `request` with `seq` 1, then two `chunk` frames on stream 1
    with correlation 1 and stream `seq` 1 and 2, payloads equal. Mutation:
    stream 0; stream `seq` from 0.
  - `commandStream_accepted_resolves_and_the_upload_stays_open` — `accepted`
    resolves on `response accepted`; a later `write` still sends a `chunk`.
    Mutation: forget the call at the response.
  - `a_write_waits_while_the_credit_is_spent_and_resumes_on_stream_credit` — the
    fifth `write` stays pending (no fifth `chunk` after a turn of the event
    loop) until the server's `stream_credit` of 1 on stream 1; then it sends and
    resolves. Mutation: initial credit 5; ignore `stream_credit`.
  - `close_sends_stream_end_end_with_the_next_seq` — after three chunks, `close`
    sends `stream_end end` on stream 1 with `seq` 4 and resolves. Mutation:
    `seq` 1.
  - `abort_sends_stream_end_interrupted` — on stream 1 with the next `seq`.
    Mutation: send `end`.
  - `a_stream_end_contract_on_stream_1_rejects_the_next_write_and_accepted_stays_resolved`
    (R-33 step 5) — `accepted` resolved, then the server's
    `stream_end
    contract(invalid-value range)` on stream 1: the next `write`
    rejects with a `SessionError` of category `contract` and that contract;
    `accepted` is still resolved. Mutation: reject only on `close`.
  - `a_refusal_rejects_a_write_waiting_for_credit` — the fifth `write` pending,
    then `stream_end corrupt` on stream 1: it rejects `corrupt`. Mutation: leave
    the waiting `write` unsettled.
  - `queryWithUpload_reply_rejects_with_the_upload_refusal` —
    `stream_end
    contract(invalid-value utf8)` on stream 1 rejects `reply`
    with rule `"utf8"`. Mutation: leave `reply` pending.
  - `queryWithUpload_bound_runs_from_close` — 60 ms with the upload open:
    `reply` still pending; `close`, then 60 ms: `reply` rejects `timeout`.
    Mutation: start the timer at the `request`.
  - `commandStream_bound_runs_from_the_request` — no `response` within 20 ms:
    `accepted` rejects `undelivered`. Mutation: no timer before `close`.
  - `a_write_longer_than_the_member_limit_rejects_corrupt_and_sends_stream_end_interrupted`
    — a `write` of 16,129 bytes. Mutation: send the chunk.
  - `commandStream_on_a_v2_session_is_unsupported_and_sends_nothing` — both the
    promise and the first `write` reject `unsupported`. Mutation: send the
    request.
  - `a_chunk_or_a_frame_on_stream_0_for_an_upload_is_unexpected_form` — a
    `chunk` on stream 1, and a `stream_credit` on stream 0, each end the session
    with `unexpected-form`. Mutation: discard instead.
  - `a_refusing_response_errors_the_upload` — `response busy`: `accepted`
    rejects `busy`, the next `write` rejects `busy`. Mutation: error only the
    promise.
  - `aborting_queryWithUpload_rejects_reply_interrupted` — and a later
    `response` is discarded as `unknown-correlation`. Mutation: keep the call.
  - `the_session_ending_errors_the_upload_and_rejects_with_down`. Mutation: skip
    upload calls in `fail`.
  - `a_stream_credit_on_stream_1_for_a_streamed_reply_is_unexpected_form` — the
    stage 4 rule still holds for `queryStream`. Mutation: route every
    `stream_credit` to an upload.
- [ ] **Step 2: Run them and confirm they fail:**
      `deno test tests/calls_test.ts`.
- [ ] **Step 3: Implement** the items of Interfaces and B-12 to B-15. The sink's
      `write` returns a promise that resolves when its `chunk` is sent; the
      `WritableStream` uses its default queuing strategy (high-water mark 1).
- [ ] **Step 4: Run** `deno test tests/`, `deno check mod.ts`, `deno lint`,
      `deno fmt --check`. All pass; the stage 4 tests are unchanged.
- [ ] **Step 5: Commit** (driver):
      `feat(ridl-transport-ws): upload a parameter stream from the TypeScript client`.

### Task 5: The end-to-end tests

**Files:**

- Modify: `packages/ridl-transport-ws/e2e/catalog.ts`, `e2e/deno_test.ts`,
  `e2e/page.ts`, `e2e/browser_test.ts`
- Modify: `crates/ridl-transport-ws/tests/ts_ceilings.rs`
  (`the_e2e_catalog_holds_the_ordinals_and_lengths_of_the_test_provider`)

**Interfaces:**

- Consumes: Task 3's provider members and values; Task 4's methods.
- Produces (`e2e/catalog.ts`): `UPLOAD = 11`, `UPLOAD_QUERY = 12`, `FETCH = 13`,
  `MODE_REFUSE_RANGE = 1`, `MODE_REFUSE_UTF8 = 2`, `FETCH_CHUNK = 16_128`,
  `UPLOAD_LEN = 200_000`, and
  `writeAll(upload: WritableStream<Uint8Array>, bytes: Uint8Array, chunk: number): Promise<void>`
  (writes `bytes` in pieces of `chunk`, then closes) and
  `readAll(stream: ReadableStream<Uint8Array>): Promise<Uint8Array>`.

- [ ] **Step 1: Extend `ts_ceilings.rs`** so that `UPLOAD`, `UPLOAD_QUERY`,
      `FETCH` and `FETCH_CHUNK` in `catalog.ts` equal the provider's. Run
      `cargo test -p ridl-transport-ws --test ts_ceilings` (no socket); it fails
      until Step 2. Mutation: a wrong ordinal in `catalog.ts`.
- [ ] **Step 2: Write the Deno tests** in `e2e/deno_test.ts` (the driver runs
      `just ts-e2e`):
  - `commandStream_uploads_200_000_bytes_and_fetch_reads_them_back` — key 1,
    `pattern(UPLOAD_LEN)` in pieces of 16,128; `accepted` resolves; then
    `readAll(queryStream(IFACE, FETCH, [1]))` equals the input byte for byte.
  - `queryWithUpload_replies_the_count_and_the_sum_of_200_000_bytes` — the reply
    is 12 bytes: 200,000 as `u64` little-endian, then the wrapping `u32` sum.
  - `a_refused_third_block_rejects_the_next_write_with_contract_and_accepted_stays_resolved`
    — key 2, `MODE_REFUSE_RANGE`, pieces of 1,024: some `write` after the third
    rejects with category `contract`, rule `"range"`; `accepted` resolved.
  - `a_utf8_refusal_crosses_with_rule_utf8` — `UPLOAD_QUERY` with
    `MODE_REFUSE_UTF8`: `reply` rejects with category `contract`, rule `"utf8"`.
- [ ] **Step 3: Extend the browser page.** In `page.ts`, after the `queryStream`
      read, upload `pattern(UPLOAD_LEN)` to key 3 with `commandStream` and read
      it back through `FETCH`; the result gains `upload: "<length>:<match>"`,
      and `browser_test.ts` expects `upload: "200000:true"`. The driver runs
      `just ts-browser-e2e` when a Chromium-family browser is installed, and CI
      runs it otherwise.
- [ ] **Step 4: Check** with `deno check e2e/*.ts`, `deno lint`,
      `deno fmt
      --check`.
- [ ] **Step 5: Commit** (driver):
      `test(ridl-transport-ws): upload 200,000 bytes end to end and read them back`.

### Task 6: The records

**Files:**

- Modify: `docs/specification/frame-specification.md` — §5.3 (replace "A command
  with a stream parameter is carried by no binding yet (§13)" with the rules of
  a command's parameter stream: the stream opens with the `request`, `chunk`
  frames on stream `k`, the client's `stream_end end` or `interrupted`, the
  acknowledgment covers the arguments and not the chunks, R-33 step 3); §5.4 (a
  query with a parameter stream: R-17's bound from the `stream_end end`, a
  refusal before the reply); §11.1 "Binding version 3" (the credit of a stream
  parameter: initial 4, one per chunk taken); §13 "Streams" (the parameter
  streams and the UTF-8 rule are carried; keep the remaining omissions).
- Modify: `docs/design/ridl-transport-ws.md` — new **D-28. A parameter stream
  over frame version 3**, recording B-4 to B-10, `Note::ChunkTooLarge`, the
  limit of Q4 (f), and the duplex call; update D-27's "What the client may send"
  and the "not built" list near the end of the record.
- Modify: `docs/design/ridl-transport-ws-ts.md` — new **D-21. An upload**,
  recording B-12 to B-16 and the limit of Q4 (f); update the "not built" list
  and the end-to-end test list.
- Modify: `docs/technotes/websocket-binding-rationale.md` (the version 3
  section: the upload direction, and why one `write` is one `chunk`).
- Modify: `docs/decisions/ADR-0032-byte-streams.md` `## Status` (an amendment
  dated the day the PR lands: the WebSocket upload direction, `Utf8` as code 7
  of frame version 3, and the two TypeScript methods are built; the duplex query
  has no TypeScript method, with its tracking issue).
- Modify: `docs/specification/ridl-family-overview.md` row 43,
  `docs/book/getting-started.md` (the paragraph that says the upload direction
  is not built), `packages/ridl-transport-ws/README.md` (the two methods),
  `crates/ridl-transport-ws` rustdoc where it says no member has a stream
  parameter.
- Modify: `docs/wip/2026-10-31-streams-design.md` — a section "As built (stage
  5b)" listing B-4 to B-16 and the answers to Q1 to Q5.

- [ ] **Step 1: Write the records** above. State each fact once and link to its
      record. No stage, story or epic name outside `docs/wip/` and the ADR.
- [ ] **Step 2: Check** with `just link-check`, `just doc-path-check`,
      `just story-id-check`, `just book-check` and `prim fmt --check` on the
      changed files. All pass.
- [ ] **Step 3: Commit** (driver):
      `docs: record the upload direction of the WebSocket binding`.

## Self-review

- **Spec coverage.** Design §6 server bullets: chunk received and pushed (Task
  2), credit per accepted chunk and at most 4 held (Task 2), `ChunkTooLarge`
  (Task 2), client `stream_end` (Task 2), `PushError::Failed` on a parameter
  stream (Tasks 2 and 3), session end (Task 2). §6 TS client: `commandStream`,
  `queryWithUpload`, `write` waits, `close`, `abort` (Task 4). R-15 and §5.2
  `Utf8` 7 (Task 1). R-17 for a query with an upload (Task 3 server, Task 4
  client). R-33 steps 3 and 5 (Tasks 2, 3, 4, 5). Stage 5 Tests list: the
  `ts-e2e` upload of 200 KB read back through `fetch` (Task 5); the oversized
  chunk answered `stream_end corrupt` (Tasks 2 and 3); the refused third block
  (Tasks 3 and 5); the bound from the upload's end (Tasks 3 and 4); "a chunk
  injected on the wire is refused on the receiving side" is the face's (5a) and
  crosses here as `detail` 7 (Tasks 1 and 3); the `pipe` reply before the upload
  ends (Task 3, server only, Q1). Records: Task 6. The remaining items of the
  stage 5 Tests list were delivered by 5a.
- **Step scan.** Every test names its assertions and its mutation; every code
  step names its signature or points to an Interfaces block. The one algorithm
  given in prose is `Upload::drain`, whose order (chunks before the end) the
  signature does not fix.
- **Type consistency.** `Upload`, `Receive`, `Drain`, `DrainEnd`,
  `InFlight::{uploads, settled}` are defined in Task 2 and used in Task 3.
  `MemberTable::set_parameter_streams` and `parameter_streams` take and give a
  count (`u8`) everywhere. `UPLOAD` 11, `UPLOAD_QUERY` 12, `FETCH` 13, `DUPLEX`
  14, `FETCH_CHUNK` 16,128 and the modes 1 and 2 match between Task 3, Task 5
  and Q2. `UPLOAD_STREAM = 1` in TS matches "stream 1" in the server tests.
  `Note::ChunkTooLarge { seq, stream, len }` is the same in Tasks 2, 3 and 6.
- **Review Focus.** Each of the five lines names its test and its task.
- **Proportion.** The plan carries signatures, values, test names and mutations;
  no function body.
