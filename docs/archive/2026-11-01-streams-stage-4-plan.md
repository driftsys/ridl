# Streams stage 4 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** Carry a query's streamed reply over the WebSocket binding: a new frame
version with the forms `chunk`, `stream_end` and `stream_credit`, a server in
`ridl-transport-ws` that pulls the reply stream from the backend's stream port
under a per-stream credit, and `Session.queryStream` in the TypeScript client.

**Architecture:** Frame version 2 already exists and holds the blocks of a
`fixed` (`block` = form 11, `ridl-ws-v2`, unreleased). The stream forms
therefore arrive as **frame version 3** under `ridl-ws-v3`, which is version 2
plus the three forms, as the frame specification §11.1 "Later versions" and §13
and the technote's "Streams arrive as frame version 3" already state. The server
selects version 3 only when its backend has the stream port and a member table.
On a version 3 session, a request for a member whose reply is a stream is
answered only by `chunk` frames and one `stream_end`, never by a `response`.

**Tech Stack:** Rust (`ridl-transport-ws`, edition 2021, `rust-version` 1.88),
`ridl-rt::port::StreamCaller`, `ridl-loopback` as the test backend,
`tungstenite` in the tests, Deno for `packages/ridl-transport-ws`,
`cargo xtask freeze`.

**Spec:** `docs/wip/2026-10-31-streams-design.md` — section 5 (5.1 to 5.4 and
5.6), section 6 (the reply stream only), R-12 to R-19, R-26 to R-29, R-32, and
the stage 4 entry of section 9. That design was written before the blocks work
took frame version 2; every place it says "frame version 2" or `ridl-ws-v2` for
streams reads "frame version 3" or `ridl-ws-v3` here (S4-1). Ruling numbers R-n
are the design's; S4-n are this plan's.

## Rulings taken in this plan

- **S4-1. Streams are frame version 3, `ridl-ws-v3`; form codes `chunk` 12,
  `stream_end` 13, `stream_credit` 14.** The design's codes 11 to 13 collide
  with `block` 11. Version 3 is version 2 plus the three forms: every layout,
  code and rule of version 2 holds, blocks included. Outcome codes `end` 10 and
  `interrupted` 11 are as in the design, and are valid only in a `stream_end`.
  The records already say streams arrive as version 3; this ruling follows them.
- **S4-2. The server selects `ridl-ws-v3` when the client offers it, the backend
  answers `streams()` with `Some`, and `members()` is `Some`; otherwise it
  selects as today (`ridl-ws-v2`, then `ridl-ws-v1`).** This amends R-18, which
  fell back to version 1: the fallback is now the newest version the server
  serves without streams. "The member table carries the stream entries" is read
  as "there is a member table"; a table with no streamed member makes version 3
  harmless.
- **S4-3. The public constants `FRAME_VERSION` and `SUBPROTOCOL` keep meaning
  version 2; version 3 adds `FRAME_VERSION_3` and `SUBPROTOCOL_V3`** in Rust and
  in TypeScript. Reason: `ridl-transport-ws` is in the `runtime-crates` freeze
  row, and changing a constant's value would be a break and would need the row
  removed and re-added, as #1156 needed. New items are additive.
- **S4-4. `Rule::Utf8` (R-15) is not added in stage 4.** No `<string>` stream
  exists before stage 5. If version 3 has not been released when stage 5 starts,
  stage 5 adds `detail` code 7 to version 3; otherwise it adds it in version 4.
  Reason: it keeps `ridl-rt` unchanged in this stage.
- **S4-5. In TypeScript, `queryStream` on a version 1 or 2 session sends nothing
  and returns a stream that errors with `SessionError` of the new category
  `"unsupported"`.** This replaces R-19's `ProtocolError("streams-unsupported")`
  and follows the technote: a missing capability is not a peer defect, so it
  must not end the session. The categories `"interrupted"` and `"unsupported"`
  join `Failure`.
- **S4-6. A streamed reply's abandonment clock restarts at each `chunk` sent and
  each `stream_credit` received.** When `call_abandon_after` passes with
  neither, the server sends `stream_end interrupted`, forgets the call and
  records `Note::CallAbandoned`. The design states the bound only for the
  consumer.
- **S4-7. The client side of stage 4's unused directions.** A client `chunk` on
  stream 0 is `Note::UnexpectedDirection`, and close 1002. A client `chunk` on
  any other stream is `Note::StreamCreditExceeded`, and close 1002, because no
  stage 4 member has a parameter stream, so the server granted no credit for it.
  A client `stream_end` with outcome `end` on stream 0 is
  `Note::Violation(ProtocolViolation::OutcomeNotAllowed)`, and close 1002,
  because a receiver cancels a stream and does not end it. A `stream_credit` for
  a correlation the server no longer streams is ignored and records no note. A
  credit can arrive just after the end, so this is a normal race.
- **S4-8. A kept streamed reply is a `stream_end interrupted` with `seq` 1.**
  The frame is kept in the kept-response set of R-17 under the request's `seq`.
  It is stamped when the stream ends and retained until
  `min(stamp, receipt) + max`.
- **S4-9. A request for a streamed-reply member on a version 1 or 2 session is
  answered `response contract(UnknownInteraction)` with
  `Note::UnknownInteraction`.** Version 2 has no outcome that names a missing
  capability. A conforming client never sends this request, because S4-5 stops
  it locally.
- **S4-10. When the session ends, the provider reads `Interrupted`, not
  `Down`.** The server forgets every call, and the port's `forget` stops every
  open stream with `Transport::Interrupted` (`ridl_rt::error::Transport`). The
  design's "`Transport::Down` on the backend" is not reachable through the port,
  so it is not built.
- **S4-11. Review:** a per-task gate, then `/review` passes 1 and 2 over the
  branch. There is no separate final whole-branch review. `/review`'s deep seat
  runs on Codex. The task reviewers run Opus high, and mutation runs only in the
  reviews of Tasks 3 and 4. Implementers: Sonnet for Tasks 1, 2 and 5, Opus for
  Tasks 3 and 4.

## Global Constraints

- Version 1 and version 2 layouts, codes, vectors and dump lines are not edited.
  The `wire` dump only gains lines.
- `ridl-transport-ws` changes are additive (`runtime-crates` row): `Frame`,
  `Note`, `ProtocolViolation` are `#[non_exhaustive]`, `Backend::streams` has a
  default, constants are new items. Run `cargo xtask freeze check`.
- `ridl-rt`, the IR, the catalog descriptor and every catalog hash are
  unchanged. No diagnostic is added or changed.
- Header sizes: `chunk` 35, `stream_end` 37, `stream_credit` 23.
  `MAX_MESSAGE_LIMIT` stays 16,164; `chunk` is 35 + at most 16,128.
- Initial credit is 4 chunks per stream (R-16, R-27):
  `pub const INITIAL_CREDIT:
  u32 = 4` in `frame.rs`, `INITIAL_CREDIT = 4` in
  `frame.ts`.
- A stream frame's `kind` is `Command` or `Query`; any other kind is
  `ProtocolViolation::BadStream`.
- Every vector stamp follows the version 1 table: `0x0123456789AB` on
  consumer-sent frames, `0x0123456789AC` on provider-sent ones.
- No story id, epic or plan name in shipped docs or rustdoc
  (`just story-id-check`). Plain, literal prose.
- Implementers run focused crate tests (`cargo test -p ridl-transport-ws`,
  `deno task test` in the package); CI runs the full gate.

## Review Focus

1. **A slow consumer.** If a client grants no credit, the server sends exactly 4
   chunks and then stops. Other traffic on the same session, such as a signal
   publish or another call's response, still flows while the stream waits.
   Pinned in Task 3 (`credit_stall_does_not_block_a_signal`).
2. **Cancel during a stalled stream.** A client `stream_end interrupted` while
   the provider is `Pending` or out of credit forgets the call on the backend.
   The provider's next push then answers `Failed`. Pinned in Task 3.
3. **A seq gap or a stream frame the TS client did not ask for.** A `chunk` with
   a skipped `seq` errors the `ReadableStream` with `"interrupted"` and sends
   `stream_end interrupted`. A `chunk` for a plain query's correlation is a
   protocol violation. Pinned in Task 4.
4. **The browser cancels a reader.** `reader.cancel()` sends exactly one
   `stream_end interrupted` with `seq` 1. A frame that arrives after the cancel
   is discarded as `unknown-correlation` and does not error the session. Pinned
   in Task 4.
5. **An empty stream.** A provider that ends at once gives `stream_end end` with
   `seq` 1, and the TS stream closes with zero chunks. Pinned in Tasks 3 and 4.

---

### Task 1: The version 3 codec in Rust and TypeScript, its vectors and the wire dump

**Files:**

- Modify: `crates/ridl-transport-ws/src/frame.rs`
- Modify: `crates/ridl-transport-ws/tests/vectors.rs`, `tests/decode_errors.rs`,
  `tests/codes.rs`
- Modify: `packages/ridl-transport-ws/src/frame.ts`, `tests/vectors_test.ts`,
  `tests/decode_errors_test.ts`, `tests/codes_test.ts`
- Modify: `docs/specification/frame-specification.md` §4 (form table, codes, the
  three form paragraphs, "Frame versions") and §11.1 (a new
  `#### Binding version 3` after "Binding version 2", with its vector table)
- Modify: `xtask/src/freeze/wire.rs` (`frame_vectors`, `check_vector`),
  `freeze/wire.txt`

**Interfaces:**

- Produces (Rust, `ridl_transport_ws::frame`):
  - `pub const FRAME_VERSION_3: u32 = 3;`,
    `pub const SUBPROTOCOL_V3: &str =
    "ridl-ws-v3";`,
    `pub const INITIAL_CREDIT: u32 = 4;`
  - `Frame::Chunk { target: Target, envelope: Envelope, correlation: u64,
    stream: u8, payload: &'a [u8] }`
  - `Frame::StreamEnd { target: Target, envelope: Envelope, correlation: u64,
    stream: u8, outcome: StreamOutcome }`
  - `Frame::StreamCredit { target: Target, correlation: u64, stream: u8,
    credit: u32 }`
  - `#[non_exhaustive] pub enum StreamOutcome { End, Contract(Contract), Corrupt,
    Busy, Interrupted }`.
    Its codes are `end` 10, 3 to 6 with `detail` = the `Rule` code for 3 and 0
    otherwise, `corrupt` 7, `busy` 8 and `interrupted` 11.
  - `ProtocolViolation::BadStream`, for a stream frame whose kind is not
    `Command` or `Query`. An outcome outside the set is the existing
    `OutcomeNotAllowed`, and a `stream_credit` of 0 is the existing
    `ZeroNumber`.
- Produces (TS, `frame.ts`): `FRAME_VERSION_3`, `SUBPROTOCOL_V3`,
  `INITIAL_CREDIT`, `FORM_CHUNK = 12`, `FORM_STREAM_END = 13`,
  `FORM_STREAM_CREDIT = 14`. The `Frame` union also gains
  `{ form: "chunk"; target; envelope; correlation: bigint; stream: number;
  payload: Uint8Array }`,
  `{ form: "stream_end"; target; envelope; correlation;
  stream; outcome: StreamOutcome }`
  and
  `{ form: "stream_credit"; target;
  correlation; stream; credit: number }`,
  with
  `type StreamOutcome = { kind: "end" } | { kind: "interrupted" } | { kind:
  "corrupt" } | { kind: "busy" } | { kind: "contract"; contract: Contract }`.
  `Violation` gains `"bad-stream"`.

- [ ] **Step 1: Write the version 3 section of the specification with its four
      vectors.** Add `chunk`, `stream_end` and `stream_credit` to the §4 form
      table, its codes and its field table, with the layouts in Global
      Constraints and design §5.1. Add `#### Binding version 3`, modelled on
      "Binding version 2", stating S4-1, the subprotocol order
      `ridl-ws-v3, ridl-ws-v2, ridl-ws-v1` and the initial credit 4. Its table,
      `**Version 3 test vectors.**`, holds exactly these rows:

  | Id               | Vector                                                                                | Hex                                                                                                     |
  | ---------------- | ------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------- |
  | `attach-3`       | `attach` frame 3, FlatBuffers, `veh.cabin`                                            | `01 03000000 01 000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f 09 7665682e636162696e` |
  | `chunk-reply`    | `chunk` query, interface 1, ordinal 4, seq 1, correlation 2, stream 0, payload `ff01` | `0c 04 01000000 04000000 ac89674523010000 0100000000000000 0200000000000000 00 ff01`                    |
  | `stream-end-end` | `stream_end` `end`, query, interface 1, ordinal 4, seq 2, correlation 2, stream 0     | `0d 04 01000000 04000000 ac89674523010000 0200000000000000 0200000000000000 00 0a 00`                   |
  | `stream-credit`  | `stream_credit` 4, query, interface 1, ordinal 4, correlation 2, stream 0             | `0e 04 01000000 04000000 0200000000000000 00 04000000`                                                  |

- [ ] **Step 2: Extend the wire dump reader.** In `xtask/src/freeze/wire.rs`,
      `frame_vectors` also splits at `\n#### Binding version 3\n` and reads that
      table with version 3. `check_vector` admits forms 12 to 14 only at version
      3 or later and gives their header sizes. Add a unit test that a version 2
      table holding form 12 is refused, next to the existing form 11 test. Run
      `cargo xtask freeze update` and confirm that
      `git diff
      freeze/wire.txt` only adds the four `frame-vector` lines.
- [ ] **Step 3: Write the failing Rust vector test.** In `tests/vectors.rs`, add
      `every_version_3_vector_decodes_and_reencodes_byte_for_byte`. It reads the
      `**Version 3 test vectors.**` table and has
      `KNOWN_3 = ["attach-3",
      "chunk-reply", "stream-end-end", "stream-credit"]`.
      Its `expected_3` builds the frames of the table above, and it uses
      `assert_round_trip`. Add decode-error tests to `tests/decode_errors.rs`: -
      `stream_frame_of_kind_signal_is_bad_stream`; -
      `stream_end_with_outcome_reply_is_not_allowed` (outcome 2); -
      `stream_end_interrupted_with_nonzero_detail_is_refused`; -
      `stream_credit_of_zero_is_zero_number`; -
      `chunk_shorter_than_its_header_is_too_short` (34 bytes).

      Add a test to `tests/codes.rs` that maps every `StreamOutcome` to its
      code and back.
- [ ] **Step 4: Run them and confirm they fail.** Run
      `cargo test -p
      ridl-transport-ws --test vectors --test decode_errors --test codes`.
      The expected result is a compile failure on `Frame::Chunk`.
- [ ] **Step 5: Implement the Rust codec.** Add the form constants
      `FORM_CHUNK =
      12`, `FORM_STREAM_END = 13` and
      `FORM_STREAM_CREDIT = 14`, the header lengths `CHUNK_LEN = 35`,
      `STREAM_END_LEN = 37` and `STREAM_CREDIT_LEN =
      23`, the outcome
      codes `OUT_END = 10` and `OUT_INTERRUPTED = 11`, and the `decode`,
      `encoded_len` and `encode` arms. `decode` stays stateless. The session
      gates versions (Tasks 2 and 3). `outcome_known` does not change, because
      10 and 11 are known only to `stream_end`. Keep `FRAME_VERSION` and
      `SUBPROTOCOL` as they are (S4-3).
- [ ] **Step 6: Run the Rust tests and confirm they pass.** Run the same
      command. All of them pass.
- [ ] **Step 7: Write the failing TypeScript tests and then the TS codec.** In
      `vectors_test.ts`, read the version 3 table the way the version 2 table is
      read, and add the decode-error and code tests of Step 3 in TS form. Then
      implement the `frame.ts` arms. Run `deno task test` in
      `packages/ridl-transport-ws` and `deno check`. Both pass.
- [ ] **Step 8: Commit** with the message
      `feat(ridl-transport-ws): add the
      stream forms of frame version 3`.

### Task 2: Negotiate version 3 and give the server the stream port and the stream table

**Files:**

- Modify: `crates/ridl-transport-ws/src/server/backend.rs`, `server/members.rs`,
  `server/descriptor.rs`, `src/ws.rs`, `src/server.rs`, `server/session.rs` (the
  version gate only)
- Modify: `crates/ridl-transport-ws/tests/serve.rs`, `tests/descriptor.rs`,
  `src/ws.rs` handshake tests

**Interfaces:**

- Consumes: Task 1's constants.
- Produces:
  - `Backend::streams(&mut self) -> Option<&mut dyn ridl_rt::port::StreamCaller>`,
    with a default of `None` (R-32). `Polling`, `Scanning` and `Described`
    forward it to their inner ports when those implement `StreamCaller`. They do
    it through a helper trait or by `Described` forwarding to the inner backend.
    If `dyn StreamCaller` is not object safe, record the substitute signature as
    a ruling in the ledger before continuing.
  - `MemberTable::set_reply_stream(&mut self, iface: InterfaceNo, ord: Ordinal)`
    and `MemberTable::reply_stream(&self, iface, ord) -> bool`.
    `from_descriptor` sets it when `member.streams()` holds an entry with
    `no == 0`.
  - `ws::handshake` takes `max_version: u32` (3, or 2 when S4-2's condition
    fails) and selects the newest offered version up to it.
  - `Session::frame_version` can be 3. On a session below 3, a received form 12,
    13 or 14 is `Note::Violation(ProtocolViolation::UnknownCode)` and close
    1002.

- [ ] **Step 1: Write the failing tests.** In `ws.rs`, add
      `offers_v3_v2_v1_with_max_3_selects_v3` and
      `offers_v3_v2_v1_with_max_2_selects_v2`. In `tests/serve.rs`: -
      `a_v2_client_against_a_streams_server_is_served_v2`; -
      `a_v3_client_against_a_backend_without_streams_is_selected_v2`, using
      `Scanning` with a loopback ports type that is not stream-capable, or
      `Described` over a test backend whose `streams()` is `None`; -
      `a_v3_client_against_a_backend_without_a_table_is_selected_v2`; -
      `a_v3_client_against_a_streams_server_is_selected_v3`; -
      `a_stream_form_on_a_v2_session_closes_1002`.

      In `tests/descriptor.rs`, add `the_table_reads_a_reply_stream` from a
      catalog fixture whose member has `streams: Some(..)` with entry 0.
- [ ] **Step 2: Run them and confirm they fail.** Run
      `cargo test -p
      ridl-transport-ws`. The expected result is a compile
      failure or the assertion failures.
- [ ] **Step 3: Implement.** In `serve()`, compute `max_version` from S4-2
      before the handshake. The backend is already owned there, because the D-26
      check runs before the handshake. Add the session gate. Implement the
      `Backend` and `MemberTable` items above.
- [ ] **Step 4: Run them and confirm they pass.** Run
      `cargo test -p
      ridl-transport-ws --all-features`. Every version 1
      and version 2 test still passes.
- [ ] **Step 5: Commit** with the message
      `feat(ridl-transport-ws): select frame
      version 3 when the backend serves streams`.

### Task 3: Serve a streamed reply on a version 3 session

**Files:**

- Modify: `crates/ridl-transport-ws/src/server/session.rs`, `server/calls.rs`,
  `server/config.rs` (`Note`)
- Modify: `crates/ridl-transport-ws/tests/support/provider.rs` (a streamed
  member), `examples/loopback_server.rs`, `tests/serve.rs`

**Interfaces:**

- Consumes: Task 1's frames and `INITIAL_CREDIT`; Task 2's `Backend::streams`,
  `MemberTable::reply_stream` and `frame_version`.
- Produces:
  - `Note::StreamCreditExceeded { seq: u64 }` (a violation; close 1002) and
    `Note::StreamStopped { seq: u64 }` (the client cancelled). The design's
    `ChunkTooLarge` waits for stage 5, because no client chunk is accepted in
    stage 4.
  - In `tests/support/provider.rs`: `pub const STREAM_QUERY: u32 = 9;` on
    interface 1. The catalog member has a reply stream of named elements. The
    argument is one byte `n`. The provider pushes `n` elements, where element
    `i` is the 1,024 bytes `[i; 1024]` (`i` as `u8`), then calls
    `end_claim(REPLY_STREAM)`. An argument of `0xff` instead pushes 3 elements
    and then `abort_claim(.., Transport(Interrupted))`. `provider_step` keeps
    per-claim progress across steps, because a push that answers 0 means the
    window is full. Add the member to `member_table()`. The example server
    serves it unchanged.

Behaviour on a version 3 session, for a request whose `reply_stream` is true.
The rulings are R-13, R-16, R-17 and S4-6 to S4-9:

1. Dispatch with `ports.query` as today. A `SendError` is answered with
   `stream_end` carrying the mapped outcome, `seq` 1 and stream 0, not with a
   `response`.
2. Each `poll`, for each streamed call with credit left,
   `streams().pull(corr,
   REPLY_STREAM, buf)`. Grow `buf` on `Short` up to the
   member's limit, then handle the result:
   - `Chunk(n)` sends a `chunk` with the next per-stream `seq`, starting at 1,
     and lowers the credit by one;
   - `End` sends `stream_end end` with the next `seq`;
   - `Failed(e)` sends `stream_end` with `e`'s outcome and the next `seq`;
   - `Pending` stops the loop for this call.

   After `End` or `Failed`, run `forget(corr)` and settle the kept
   `stream_end interrupted` (S4-8). A `response` is never sent for this call.
3. A `stream_credit` for `(correlation, 0)` of a streamed call adds to its
   credit with a saturating add.
4. A client `stream_end` on stream 0 with `interrupted`, `contract` or `corrupt`
   calls `abort(corr, REPLY_STREAM, e)` and then `forget(corr)`, records
   `Note::StreamStopped`, and keeps the S4-8 frame. With `end`, it is a
   violation (S4-7).
5. A retransmitted `request` of an ended streamed call re-sends the kept frame.
   One still in flight is `Note::DuplicateDropped`, as today.

- [ ] **Step 1: Write the failing tests in `tests/serve.rs`.** Use the
      `tungstenite` pattern of
      `serve_answers_attach_and_streams_a_signal_change`, with a client that
      offers `SUBPROTOCOL_V3`:
  - `streams_64_elements_then_ends_with_the_next_seq`: the client grants 1
    credit after each chunk it reads. It receives 64 `chunk`s with `seq` 1 to
    64, payload `[i; 1024]` and correlation equal to the request `seq`, then
    `stream_end end` with `seq` 65. No `response` arrives.
  - `an_empty_stream_ends_with_seq_1` (argument `0`).
  - `no_credit_means_four_chunks_and_no_more`: no fifth `chunk` arrives within
    200 ms. After `stream_credit 2`, exactly 2 more arrive.
  - `credit_stall_does_not_block_a_signal`: while the stream is stalled at 4, a
    subscribed signal change still arrives as a `publish`.
  - `client_interrupt_forgets_the_call_on_the_backend`: after 2 chunks, the
    client sends `stream_end interrupted`. The provider's next push answers
    `Failed`, `Note::StreamStopped` is recorded, and no further frame of the
    stream arrives.
  - `provider_interrupt_reaches_the_client`: with argument `0xff`, the client
    receives 3 chunks and then `stream_end interrupted` with `seq` 4.
  - `retransmit_after_the_end_is_answered_stream_end_interrupted`.
  - `a_busy_admission_is_a_stream_end_busy_with_seq_1`, using the loopback's
    admission limit or a test backend whose `query` answers `Busy`.
  - `a_client_chunk_on_stream_0_closes_1002` (`UnexpectedDirection`) and
    `a_client_chunk_on_stream_1_closes_1002` (`StreamCreditExceeded`).
  - `a_client_stream_end_end_closes_1002`.
  - `a_plain_query_on_a_v3_session_is_answered_by_a_response`.
  - `a_streamed_member_on_a_v2_session_is_unknown_interaction` (S4-9).
  - `an_abandoned_stream_ends_interrupted`: a short `call_abandon_after` and no
    credit give `stream_end interrupted` and `Note::CallAbandoned`.
- [ ] **Step 2: Run them and confirm they fail.** Run
      `cargo test -p
      ridl-transport-ws --test serve`. The assertions fail:
      today the server answers with a `response`, or never answers.
- [ ] **Step 3: Implement** the behaviour above in `session.rs` and `calls.rs`.
      Store the per-call stream state (`seq_out: u64`, `credit: u32`,
      `last_activity: Timestamp`) on the in-flight entry, next to the existing
      `InFlight` fields.
- [ ] **Step 4: Run them and confirm they pass.** Run
      `cargo test -p
      ridl-transport-ws --all-features`, then
      `cargo clippy -p
      ridl-transport-ws --all-targets -- -D warnings` and
      `cargo xtask freeze
      check`. All pass, and the freeze check passes
      with additions only.
- [ ] **Step 5: Commit** with the message
      `feat(ridl-transport-ws): serve a
      streamed reply over frame version 3`.

### Task 4: `Session.queryStream` in the TypeScript client, with the end-to-end tests

**Files:**

- Modify: `packages/ridl-transport-ws/src/session.ts`, `src/calls.ts`,
  `src/errors.ts`, `src/mod.ts` (or the package entry, if it lists exports)
- Modify: `packages/ridl-transport-ws/tests/session_test.ts`,
  `tests/version_test.ts`, `tests/calls_test.ts`
- Modify: `packages/ridl-transport-ws/e2e/deno_test.ts`, `e2e/page.ts`,
  `e2e/browser_test.ts`, `e2e/catalog.ts`

**Interfaces:**

- Consumes: Task 1's TS frames; Task 3's `STREAM_QUERY` member on the example
  server (interface 1, ordinal 9, argument `n`, elements `[i; 1024]`).
- Produces:
  - `Session.queryStream(iface: number, ord: number, args: Uint8Array, { bound }:
    { bound: number }): ReadableStream<Uint8Array>`
  - `Session.frameVersion` can be 3.
  - `Failure` gains `{ category: "interrupted" }` and
    `{ category: "unsupported" }` (S4-5).

Behaviour:

- `connect` offers `[SUBPROTOCOL_V3, SUBPROTOCOL, SUBPROTOCOL_V1]`. `#onOpen`
  maps `ridl-ws-v3` to 3, and the session sends `attach` with that version.
- Forms 12 to 14 on a session below 3 are `ProtocolError("unknown-code")`, as
  `block` on version 1 is.
- `queryStream` on version 3 sends the `request` and returns a stream with
  `highWaterMark: 4` (count strategy). Per call it keeps `granted = 4`,
  `received = 0` and `expected = 1`. Its `pull` callback sends
  `stream_credit(d)` and sets `granted += d`, where
  `d = controller.desiredSize - (granted - received)` and `d > 0` (R-19).
- A `chunk` for the call is handled in this order:
  - `seq !== expected` is handled as an interruption: send
    `stream_end
    interrupted` and error with `"interrupted"`;
  - `received === granted` is `ProtocolError("unexpected-form")`, because the
    server sent more than the credit;
  - a payload above the member's limit sends `stream_end corrupt` and errors
    with `"corrupt"`;
  - otherwise the chunk is enqueued.
- A `stream_end` closes the stream on `end`. On any other outcome it errors the
  stream with that category, and it forgets the call.
- A `response` for a stream correlation, or a `chunk` for a plain query's
  correlation, is `ProtocolError("unexpected-form")`. A stream frame for an
  unknown correlation is discarded as `"unknown-correlation"`.
- `cancel()` sends `stream_end interrupted`, stream 0, `seq` 1, and forgets the
  call.
- If no `chunk` and no `stream_end` arrives within `bound` ms of the request,
  the stream errors with `"timeout"` and the client sends
  `stream_end
  interrupted`. Session down errors it with `"down"`.

- [ ] **Step 1: Write the failing unit tests** against `tests/fake_socket.ts`:
  - `queryStream_reads_chunks_in_order_and_closes_on_end`;
  - `queryStream_grants_credit_as_the_reader_drains`: with no read, no
    `stream_credit` is sent. Each read sends credit, and the total granted is
    never more than 4 plus the number of reads;
  - `queryStream_seq_gap_errors_interrupted_and_sends_stream_end`;
  - `queryStream_chunk_beyond_credit_is_a_protocol_error`;
  - `queryStream_response_is_a_protocol_error`;
  - `a_chunk_for_a_plain_query_is_a_protocol_error`;
  - `cancel_sends_one_stream_end_interrupted_with_seq_1`;
  - `a_frame_after_cancel_is_discarded`;
  - `an_empty_stream_closes_with_no_chunk`;
  - `queryStream_times_out_without_a_first_frame`;
  - `queryStream_on_a_v2_session_errors_unsupported_and_sends_nothing`;
  - `stream_end_busy_before_a_chunk_errors_busy`.

  Add `ridl-ws-v3_is_offered_first_and_selected` to `version_test.ts`.
- [ ] **Step 2: Run them and confirm they fail.** Run `deno task test` in
      `packages/ridl-transport-ws`.
- [ ] **Step 3: Implement** the behaviour in `session.ts` and `calls.ts`.
- [ ] **Step 4: Run them and confirm they pass.** Run `deno task test`,
      `deno
      check`, `deno lint` and `deno fmt --check`. All pass.
- [ ] **Step 5: Add the end-to-end tests.** In `e2e/deno_test.ts`, add
      `queryStream_receives_64_elements_over_loopback_server`, which reads the
      whole stream from interface 1, ordinal 9, with argument 64. It asserts
      that there are 64 chunks, that chunk `i` is 1,024 bytes of value `i`, and
      that the stream closes normally. Add the same check to `e2e/page.ts` and
      `browser_test.ts`. Run `just ts-e2e`, which passes. Run
      `just ts-browser-e2e` where a browser is installed, or say in the ledger
      that CI runs it.
- [ ] **Step 6: Commit** with the message
      `feat(ridl-transport-ws): read a
      streamed reply in the TypeScript client`.

### Task 5: The records and the binding row

**Files:**

- Modify: `crates/ridl-ir/src/codegen/bindings.rs` (the `websocket` row:
  `version: "3"`, `frame_header_max_bytes: Some(21)`,
  `envelope_bytes:
  Some(16)`), and the tests that print it:
  `crates/ridlc/tests/codegen_model.rs`, `crates/ridlc/tests/layout.rs`. Look
  for other readers with `grep -rn 'frame_header_max_bytes' crates`.
- Modify: `docs/specification/frame-specification.md` §5.3 and §5.4 (a query's
  streamed reply is answered by `chunk` and `stream_end`, R-13), §7 (R-17 and
  S4-8), §8 (ADR-0032 decision 13: a consumer times out on the first stream
  frame), §9.6 (`Interrupted` crosses as a `stream_end` outcome, a third
  `Transport` variant; the UTF-8 difference of design §12 is deferred to stage 5
  with S4-4), §13 (the Streams item now says version 3 holds reply streams, and
  parameter streams are not built yet)
- Modify: `docs/design/ridl-transport-ws.md`: Calls, The end of a session,
  Tests, What is not built, and a decision D-27 holding S4-2 and S4-6 to S4-10
- Modify: `docs/design/ridl-transport-ws-ts.md`: The session, the end-to-end
  tests, What is not built, S4-5
- Modify: `docs/technotes/websocket-binding-rationale.md`: "Streams arrive as
  frame version 3" becomes as-built for the reply stream, and states the credit
  of R-16 with its alternatives
- Modify: `docs/wip/2026-10-31-streams-design.md`: an "As built (stage 4)" list
  that names S4-1 to S4-10, and a note at the head of section 5 that "frame
  version 2" there is version 3
- Modify: `packages/ridl-transport-ws/README.md` and
  `crates/ridl-transport-ws/README.md` if they list the versions or the API

- [ ] **Step 1: Change the binding row and its tests.** Run
      `cargo test -p ridlc
      --test codegen_model --test layout` and
      `cargo xtask freeze check`. If a frozen dump prints the row, update it
      with `cargo xtask freeze update`, confirm that it only adds lines or
      changes the row's own line, and name the change in the commit.
- [ ] **Step 2: Edit the records listed above.** Each fact is stated once, in
      the record that owns it, and the other records link to it.
- [ ] **Step 3: Run the documentation gates.** Run `just link-check`,
      `just doc-path-check`, `just story-id-check`, `just book-check` and
      `prim fmt --check` (or `just check`). All pass.
- [ ] **Step 4: Commit** with the message
      `docs(docs): record frame version 3
      and the streamed reply over WebSocket`.
