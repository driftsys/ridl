# WebSocket blocks Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** Frame version 2 of the WebSocket binding: the Rust provider sends a
`fixed` whose payload does not fit one message as `block` frames of 4,096 bytes,
and the TypeScript consumer reassembles them into a buffer bounded by the
catalog limit (driftsys/ridl#1129).

**Architecture:** Two lanes on separate branches and pull requests. Lane rust
(tasks R1 to R3) changes `crates/ridl-transport-ws`: the codec gains the `block`
form, the handshake selects `ridl-ws-v2` and refuses version 1, and the session
splits a large `fixed`. Lane ts (tasks T1 to T3) changes
`packages/ridl-transport-ws`: the codec gains the same form, the client offers
`ridl-ws-v2, ridl-ws-v1`, and the session reassembles. The lanes share nothing
but the byte vectors of the section "Shared byte vectors", which both test
against. The end-to-end test and the records are a third pull request after both
lanes merge.

**Tech Stack:** Rust 2021 (rust-version 1.88; `frame.rs` is `core` only),
`tungstenite` 0.29 as the test client; TypeScript on Deno with `node:assert`.

**Spec:** [`2026-10-30-ws-blocks-design.md`](2026-10-30-ws-blocks-design.md)
(merged as #1131). Read B-1 to B-6 and §3 before a task. Where this plan and the
note differ, the plan's section "Decisions this plan takes" says why.

## Global Constraints

- **Repository rules.** Read `AGENTS.md` in the worktree. Prose (comments,
  rustdoc, TSDoc, README, commit messages) is plain, literal English, with no
  idioms. Technical terms stay as they are.
- **No plan names in code.** No task id (`R1`, `T3`), no "lane", no story id and
  no `docs/wip/` path in `crates/`, `packages/`, `docs/book/`, `docs/design/` or
  `docs/technotes/` (`just story-id-check`, `just doc-path-check`). A code
  comment that needs the version 2 layout cites `driftsys/ridl#1129`; the frame
  specification gains that layout only in the third pull request.
- **Commits.** Conventional Commits with a scope listed in `.git-std.toml`:
  `ridl-transport-ws` for lane rust, `ts-transport-ws` for lane ts. R2 and T2
  are breaking and use `feat(<scope>)!:`. Never skip git hooks (the pre-commit
  hook runs `prim .` and restages). Never force-push. End every commit message
  with exactly this line, and no other trailer (no `BREAKING CHANGE:` footer):

  ```text
  Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
  ```
- **One worktree per lane.** Each lane works only in its own worktree and
  branch. Lane rust edits nothing under `packages/`; lane ts edits nothing under
  `crates/`. Neither lane edits `docs/specification/`, `docs/design/`,
  `docs/technotes/`, `docs/decisions/`, `xtask/` or `crates/ridl-ir/`: those are
  the third pull request.
- **Focused tests only.** Implementers run the tests of the crate or package
  they touch, and the narrower targets each task names; CI runs the full gate.
  The lane commands:

  ```bash
  # lane rust, from the worktree root
  cargo test --locked -p ridl-transport-ws
  cargo test --locked -p ridl-transport-ws --no-default-features
  cargo test --locked -p ridl-transport-ws --features descriptor --test descriptor
  cargo clippy --locked -p ridl-transport-ws --all-targets -- -D warnings
  cargo fmt --all --check
  # lane ts, from packages/ridl-transport-ws
  deno test --allow-read tests/
  deno fmt --check && deno lint
  deno check mod.ts && deno check --config browser.deno.json mod.ts
  ```
- **A red test fails on an assertion.** When a test names something new, first
  add the declaration with a body that keeps today's behaviour (for example
  `decode` still answering `UnknownCode` for form 11), then write the test, and
  see it fail on its assertion, not on a compile or type error and not on a
  panic of a placeholder.
- **`core` only.** `crates/ridl-transport-ws/src/frame.rs` uses no `alloc` and
  no `std` (`tests/constraints.rs` checks it), and no API newer than Rust 1.88
  (`just compat-check` builds the crate at 1.88). `u32::div_ceil` is allowed.
- **Frozen surfaces.** A change of a frame-version constant is a change of
  frozen surfaces: `wire` (`freeze/wire.txt`) and `runtime-crates` (the public
  API of `ridl-transport-ws`). The implementer of R3, after its last commit,
  runs `cargo xtask freeze update`, then `cargo xtask freeze check`, and reports
  every line that `git diff freeze/` shows removed and every break the check
  reports. No row of `freeze/surfaces.toml` is edited: moving a baseline is the
  owner's decision. Measured on this plan's base: `wire.txt` takes its
  `frame-vector` lines from the table of the frame specification §11.1
  (`xtask/src/freeze/wire.rs`, `frame_vectors`), not from the crate, so this
  lane is expected to change no line of it; `runtime-crates` has no dump file
  and is a `cargo semver-checks` comparison, which (0.51.0) has no lint for a
  changed `const` value or for a variant added to a `#[non_exhaustive]` enum, so
  no break is expected. A result that differs is reported, not worked around.
- **Merge order: lane ts first.** `just ts-e2e`, `just ts-browser-e2e`,
  `just cabin-ws-demo` and `just cabin-browser-demo` connect the repository's
  TypeScript client to the repository's Rust server. Lane ts alone passes them
  (its client offers `ridl-ws-v2, ridl-ws-v1`, the version 1 server selects
  `ridl-ws-v1`, and the client attaches with frame version 1). Lane rust alone
  fails them (the version 1 client is refused `frame_version_unsupported`). So
  lane rust rebases onto a `main` that holds lane ts before its close-out runs
  the gates, and its pull request merges after lane ts.
- **The numbers.** Frame version 2; subprotocols `ridl-ws-v2` and `ridl-ws-v1`;
  `form` code `block` 11; block header 34 bytes; `BLOCK_BYTES` 4,096; one block
  message at most 4,130 bytes; the provider sends a `fixed` whole when the
  payload is at most `max_message_bytes − 27` (16,137 at the default 16,164),
  otherwise in blocks; `FIXED_CEILING` 65,280.

## Decisions this plan takes

Where the design note is silent, or the code has moved since it was written.

- **P-1. A `ridl-ws-v1` connection refuses every `attach`.** The frame
  specification §11.1 says "On a `ridl-ws-v1` connection, `attach.frame_version`
  is 1 or the `attach` is refused". The provider serves frame version 2 only, so
  on `ridl-ws-v1` it answers `attached(refused(frame_version_unsupported))` to
  an `attach` of frame 1 and also of frame 2. The handshake tells the session
  which subprotocol it selected (R2).
- **P-2. The codes of the new failures.** A single-block error (B-6, last but
  three row) is `ProtocolViolation::BadBlock` in Rust and `"bad-block"` in
  TypeScript; a `kind` outside 1 to 5 stays `UnknownCode`/`"unknown-code"` and
  an `interface` or `ordinal` 0 stays `ZeroNumber`/`"zero-number"`, as for every
  form. A sequence error of B-6 is the `SessionViolation` `"block-sequence"`. A
  payload refused for its size is recorded as the `Note`
  `{ note: "discarded", reason: "over-limit", frame }`, once per payload.
- **P-3. The TypeScript consumer has no member table.** Its `interfaces` option
  gives limits only, and the generated module leaves out a member whose payload
  has no finite FlatBuffers bound, so it cannot say that an identity is not a
  `fixed`. It is the "consumer with no member table" of B-6: it reassembles any
  identity within that identity's limit and hands the result to the existing
  `publish` path, which discards it as `not-subscribed` when no `read` waits.
- **P-4. The `Config` check counts a `fixed` as the smaller of 36 + its limit
  and 4,130.** 36 is the largest data-plane header with its envelope, which the
  check already uses for every member; 4,130 is one block message. The check is
  `limit_short` in `crates/ridl-transport-ws/src/server.rs`, which `serve` runs
  before the opening handshake (not at attach).
- **P-5. The provider's read cap.** When `max_message_bytes` is at least 4,130,
  `read_fixed` reads up to the member's limit (the table's, or 65,280 without a
  table, which `Session::limit` already returns). Below 4,130, which only a
  backend without a member table can reach under `serve`, it keeps today's cap
  and answer: the smaller of the one-publish payload and the limit, and above it
  `Note::ValueTooLarge` with `contract(UnknownInteraction)`.
- **P-6. The codecs stay version-agnostic.** Both decoders read form 11 on any
  session. The TypeScript session refuses a `block` on a frame-version-1 session
  as the violation `"unknown-code"` (B-1: "an unknown form"). The Rust provider
  never accepts a `block` (B-6, last row).
- **P-7. No splitting helper in the public API.** The encoder writes the fields
  of a `Frame::Block` as given; the session's private `push_blocks` builds valid
  blocks.
- **P-8. Task mapping.** Design task 4 (Rust tests) is folded into R1 to R3, and
  design task 7 (TypeScript tests) into T1 to T3, because each task writes its
  tests first. Design task 6a is dropped: #1132 made
  `crates/ridl-backend-ts/src/codec.rs` emit `limit: N` on each member entry the
  catalog gives a limit, and `packages/ridl-transport-ws/src/session.ts` already
  reads it (`ConnectOptions.interfaces`, `recordedLimits`, `#limitOf`).
- **P-9. Where the version 2 vectors live until the third pull request.** As
  constants in `crates/ridl-transport-ws/tests/vectors.rs` and
  `packages/ridl-transport-ws/tests/vectors_test.ts`, copied from the section
  below. The existing spec-reading tests stay as they are.

## Review Focus

- A `fixed` whose length is an exact multiple of 4,096 (32,768 bytes): the last
  block is a full 4,096 bytes, and neither side expects a ninth block. R3 and T3
  test it.
- A provider whose `max_message_bytes` is lowered to between 4,130 and 16,164:
  the whole-or-blocks threshold is that limit minus 27, not 16,137, and every
  message stays within the limit. R3 tests 5,000 and 4,130.
- A consumer whose generated table lowers a `fixed` to 20,000 bytes: a `total`
  of 20,000 is reassembled and 20,001 is refused at the first block, before any
  copy, and the session goes on. T3 tests it.
- Two `read` calls of one `fixed` before the answer: the provider answers twice,
  the first payload settles both reads, and the second block sequence is a new
  payload that is discarded `not-subscribed`, not a sequence error. T3 tests it.
- Blocks that arrive after `Session.close()` or after the connection ended: they
  are ignored, nothing is delivered and nothing throws. T3 tests it.

---

## Shared byte vectors (both lanes)

Computed with an independent script and checked against the layout of B-3 and
the single-block rules of B-6. Every hex string is one whole message; spaces are
for reading only. Common values: the stamp is `0x0123456789AB`
(`ab89674523010000`), `seq` 0, `kind` `Fixed` (5), interface 1, ordinal 5. The
hash in `attach` is the bytes `00` to `1f`. **The pattern** `P(n)` is the `n`
bytes whose byte `i` is `i mod 251`, so a block at a wrong offset changes the
bytes.

### Vectors that decode

| Id               | Frame                                                     | Hex                                                                                                     | Bytes |
| ---------------- | --------------------------------------------------------- | ------------------------------------------------------------------------------------------------------- | ----- |
| `attach-2`       | `attach` frame 2, FlatBuffers, `veh.cabin`                | `01 02000000 01 000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f 09 7665682e636162696e` | 48    |
| `block-last`     | `block`, total 4,098, index 1, payload `abcd`             | `0b 05 01000000 05000000 ab89674523010000 0000000000000000 02100000 01000000 abcd`                      | 36    |
| `block-one`      | `block`, total 1, index 0, payload `ab`                   | `0b 05 01000000 05000000 ab89674523010000 0000000000000000 01000000 00000000 ab`                        | 35    |
| `block-first`    | `block`, total 4,098, index 0, payload `P(4096)`          | header `0b 05 01000000 05000000 ab89674523010000 0000000000000000 02100000 00000000`, then `P(4096)`    | 4,130 |
| `block-full`     | `block`, total 4,096, index 0, 4,096 bytes of `00`        | header `0b 05 01000000 05000000 ab89674523010000 0000000000000000 00100000 00000000`, then 4,096 × `00` | 4,130 |
| `block-max-last` | `block`, total 4,294,967,295, index 1,048,575, 4,095 `00` | header `0b 05 01000000 05000000 ab89674523010000 0000000000000000 ffffffff ffff0f00`, then 4,095 × `00` | 4,129 |

`attach-2` and `block-last` are the two vectors of the design note §3, byte for
byte. `block-first` followed by `block-last` is one payload of 4,098 bytes:
`P(4096)` then `ab cd`, whose SHA-256 is
`8830a559eb9dfd3795c81b96aaa42875e12ce7fb6348fccf47757b55cfefc7fe`.
`block-max-last` decodes because its length is `total − 4,096 × index` = 4,095;
`4,096 × (index + 1)` is 2^32, so a computation in `u32` that adds 1 to `index`
first overflows.

### Vectors that are protocol violations

Each is `block-last` with one change, unless the row says otherwise.

| Id                  | Change                                    | Hex                                                                                  | Rust          | TypeScript       |
| ------------------- | ----------------------------------------- | ------------------------------------------------------------------------------------ | ------------- | ---------------- |
| `bad-kind-signal`   | `kind` 1                                  | `0b 01 01000000 05000000 ab89674523010000 0000000000000000 02100000 01000000 abcd`   | `BadBlock`    | `"bad-block"`    |
| `bad-kind-6`        | `kind` 6                                  | `0b 06 01000000 05000000 ab89674523010000 0000000000000000 02100000 01000000 abcd`   | `UnknownCode` | `"unknown-code"` |
| `bad-interface-0`   | `interface` 0                             | `0b 05 00000000 05000000 ab89674523010000 0000000000000000 02100000 01000000 abcd`   | `ZeroNumber`  | `"zero-number"`  |
| `bad-total-0`       | `total` 0, `index` 0                      | `0b 05 01000000 05000000 ab89674523010000 0000000000000000 00000000 00000000 abcd`   | `BadBlock`    | `"bad-block"`    |
| `bad-no-bytes`      | no bytes after the header                 | `0b 05 01000000 05000000 ab89674523010000 0000000000000000 02100000 01000000`        | `BadBlock`    | `"bad-block"`    |
| `bad-cut-33`        | the first 33 bytes                        | `0b 05 01000000 05000000 ab89674523010000 0000000000000000 02100000 010000`          | `TooShort`    | `"too-short"`    |
| `bad-index-2`       | `index` 2 (n is 2)                        | `0b 05 01000000 05000000 ab89674523010000 0000000000000000 02100000 02000000 abcd`   | `BadBlock`    | `"bad-block"`    |
| `bad-index-max`     | `index` 4,294,967,295                     | `0b 05 01000000 05000000 ab89674523010000 0000000000000000 02100000 ffffffff abcd`   | `BadBlock`    | `"bad-block"`    |
| `bad-short-first`   | `index` 0 with 2 bytes                    | `0b 05 01000000 05000000 ab89674523010000 0000000000000000 02100000 00000000 abcd`   | `BadBlock`    | `"bad-block"`    |
| `bad-last-long`     | payload `abcdef`                          | `0b 05 01000000 05000000 ab89674523010000 0000000000000000 02100000 01000000 abcdef` | `BadBlock`    | `"bad-block"`    |
| `bad-last-short`    | payload `ab`                              | `0b 05 01000000 05000000 ab89674523010000 0000000000000000 02100000 01000000 ab`     | `BadBlock`    | `"bad-block"`    |
| `bad-first-long`    | total 8,193, index 0, 4,097 bytes of `00` | header `… 01200000 00000000`, then 4,097 × `00` (4,131 bytes)                        | `BadBlock`    | `"bad-block"`    |
| `bad-max-last-full` | `block-max-last` with 4,096 bytes of `00` | `block-max-last`'s header, then 4,096 × `00` (4,130 bytes)                           | `BadBlock`    | `"bad-block"`    |

The header of `bad-first-long` is
`0b 05 01000000 05000000 ab89674523010000 0000000000000000 01200000 00000000`.
The decoder's order of checks, which these rows pin: the header length
(`TooShort`), then `kind` in 1 to 5 (`UnknownCode`), then `interface` and
`ordinal` not 0 (`ZeroNumber`), then `kind` `Fixed`, `total` at least 1, `index`
below `n = ceil(total / 4,096)`, and the bytes after the header equal to 4,096
for `index < n − 1` and to `total − 4,096 × index` for the last (`BadBlock`).

### The split of a payload

| `total` | Blocks | Message lengths        | Bytes on the wire |
| ------- | ------ | ---------------------- | ----------------- |
| 16,138  | 4      | 3 × 4,130, then 3,884  | 16,274            |
| 20,000  | 5      | 4 × 4,130, then 3,650  | 20,170            |
| 32,768  | 8      | 8 × 4,130              | 33,040            |
| 65,280  | 16     | 15 × 4,130, then 3,874 | 65,824            |
| 4,974   | 2      | 4,130, then 912        | 5,042             |
| 4,104   | 2      | 4,130, then 42         | 4,172             |

For `total` 65,280 with payload `P(65280)`, the header of block 0 is
`0b 05 01000000 05000000 ab89674523010000 0000000000000000 00ff0000 00000000`,
the header of block 15 is the same with `0f000000` as `index`, and block 15's
payload starts `c4 c5 c6 c7`. The SHA-256 of the 16 messages joined in order is
`4738f9ca3db820c9597bcd0785a9b194a2b21eb0096f31cd94641b6934c26a58`, and of
`P(65280)` alone
`218da22426f9259c2bd8bad291c6f70c5ca96a5f402c503f5700e2f7ba2a0ba7`.

---

## Lane rust

Branch from `origin/main`; scope `ridl-transport-ws`. Close-out: rebase onto a
`main` that holds lane ts, run the gates, open the pull request with "Refs
driftsys/ridl#1129" (no closing keyword, not even inside a negation), and state
in its body that the records follow in the third pull request.

### Task R1: The `block` form in the Rust codec

**Model:** sonnet.

**Files:**

- Modify: `crates/ridl-transport-ws/src/frame.rs` (constants at 20–44, `Frame`
  at 210–269, `ProtocolViolation` at 280–300, `decode` at 505–618, `encoded_len`
  and `encode` at 669–804)
- Modify: `crates/ridl-transport-ws/src/server/session.rs:264-271` (the arm of
  provider-to-consumer forms in `handle`)
- Test: `crates/ridl-transport-ws/tests/vectors.rs`,
  `crates/ridl-transport-ws/tests/decode_errors.rs`,
  `crates/ridl-transport-ws/tests/sizes.rs`, the `tests` module of
  `src/server/session.rs`

**Interfaces:**

- Produces (public, in `ridl_transport_ws::frame`):

  ```rust
  /// The bytes of payload in every `block` but the last (binding version 2).
  pub const BLOCK_BYTES: usize = 4_096;

  // in `enum Frame<'a>` (already `#[non_exhaustive]`):
  /// Provider to consumer: one part of a `publish` of a `fixed` whose
  /// payload does not fit one message (frame version 2).
  Block {
      target: Target,
      envelope: Envelope,
      /// The length of the whole payload, at least 1.
      total: u32,
      /// The position of this block, from 0.
      index: u32,
      /// The bytes of this block.
      payload: &'a [u8],
  },

  // in `enum ProtocolViolation` (already `#[non_exhaustive]`):
  /// A `block` that is wrong by itself (frame version 2): `kind` not
  /// `Fixed`, `total` 0, `index` at or past the block count, or bytes after
  /// the header other than 4,096 (every block but the last) or
  /// `total − 4,096 × index` (the last).
  BadBlock,
  ```
- Private: `FORM_BLOCK = 11`, `BLOCK_LEN = COMMON + ENVELOPE_BYTES + 8` (34).
- `FRAME_VERSION` and `SUBPROTOCOL` do not change in this task (R2 changes
  them).

- [ ] **Step 1: Scaffold.** Add the constant, the variant and `BadBlock`.
      `decode` keeps answering `UnknownCode` for form 11; `encoded_len` returns
      `BLOCK_LEN + payload.len()`; `encode` writes only the `form` byte for a
      block. Add `Frame::Block { .. }` to the arm of `handle` that records
      `Note::UnexpectedDirection` and closes with 1002 (B-6, last row). The
      crate compiles and every existing test passes.

- [ ] **Step 2: Write the failing tests.**

  `tests/vectors.rs` (a `hex` helper and `target`/`envelope` builders exist):

  ```rust
  /// The version 2 vectors of driftsys/ridl#1129, until the frame
  /// specification holds them.
  #[test]
  fn every_version_2_vector_decodes_and_reencodes_byte_for_byte()
  // attach-2 → Frame::Attach(Attach::new(2, 1, CatalogHash(00..1f), "veh.cabin"))
  // block-last → Frame::Block { target: target(Kind::Fixed, 1, 5),
  //   envelope: envelope(0x0123_4567_89AB, 0), total: 4_098, index: 1,
  //   payload: &[0xab, 0xcd] }
  // block-one → total 1, index 0, payload &[0xab]
  // for each: decode == expected, encoded_len == bytes.len(), encode == bytes

  #[test]
  fn the_first_block_of_a_4098_byte_payload_carries_4096_bytes_of_the_pattern()
  // block-first: decode → Block { total: 4_098, index: 0, payload == P(4096) };
  // re-encodes to the same 4,130 bytes

  #[test]
  fn a_last_block_of_the_largest_total_decodes_without_overflow()
  // block-max-last decodes, total u32::MAX, index 1_048_575, 4,095 bytes;
  // block-full decodes; bad-max-last-full is Err(Violation(BadBlock))
  ```

  `tests/decode_errors.rs`: replace `form_0_and_form_11_are_unknown_codes` (line
  54), which now fails because form 11 is a block, with

  ```rust
  #[test]
  fn form_0_and_form_12_are_unknown_codes_and_a_lone_form_11_is_too_short()
  // [0] → UnknownCode, [12] → UnknownCode, [11] → TooShort

  #[test]
  fn every_single_block_error_is_a_protocol_violation()
  // each row of "Vectors that are protocol violations" → its Rust column
  ```

  and extend `every_form_cut_short_of_its_header_is_too_short` with a loop of
  its own over `block-one` (not the `whole` list, which asserts that the whole
  header decodes): each cut from 1 to 33 bytes is `TooShort`, and the cut to 34
  bytes, the header with no payload, is `BadBlock`.

  `tests/sizes.rs`: add a `Frame::Block` of `target(Kind::Fixed)` with
  `total: len.max(1) as u32`, `index: 0` to the `frames` array of
  `every_other_data_plane_form_is_no_larger`, and

  ```rust
  #[test]
  fn a_block_message_is_its_34_byte_header_and_at_most_4096_bytes()
  // BLOCK_BYTES == 4_096; a Block with a 4,096-byte payload has encoded_len
  // and written length 4_130 == ENVELOPE_BYTES + 18 + BLOCK_BYTES
  ```

  `src/server/session.rs` tests:

  ```rust
  #[test]
  fn a_block_from_the_client_closes_with_1002_and_records_unexpected_direction()
  // open(); send the block-last frame; Step::Close(1002), out empty,
  // notes == [Note::UnexpectedDirection]

  #[test]
  fn a_block_before_attach_closes_with_1002_as_a_message_before_attach()
  // notes == [Note::MessageBeforeAttach]
  ```

- [ ] **Step 3: Run the tests and see them fail on their assertions.**

  ```bash
  cargo test --locked -p ridl-transport-ws --test vectors --test decode_errors --test sizes
  cargo test --locked -p ridl-transport-ws --lib server::session::tests::a_block
  ```

  Expected: FAIL; decode answers `Err(Violation(UnknownCode))` where a block or
  `BadBlock` is expected, and the session records `Violation(UnknownCode)` in
  place of the expected note.

- [ ] **Step 4: Implement the decoder and the encoder in `frame.rs`.** The
      decoder checks in the order the shared section states; compute the
      expected length after the `index < n` check (or in `u64`) so that no `u32`
      overflows. The encoder writes the common 10 bytes, `stamp`, `seq`,
      `total`, `index` (little-endian) and the payload. Update the module doc so
      that it names the `block` form.

- [ ] **Step 5: Run the tests and see them pass.** Run:
      `cargo test --locked -p ridl-transport-ws` and
      `cargo test --locked -p ridl-transport-ws --no-default-features`, then the
      clippy and fmt commands of the Global Constraints. Expected: PASS, no
      warning.

- [ ] **Step 6: Commit.**

  ```bash
  git add crates/ridl-transport-ws
  git commit -m "feat(ridl-transport-ws): decode and encode the block form of frame version 2"
  ```

### Task R2: Frame version 2 at the handshake and at attach

**Model:** opus.

**Files:**

- Modify: `crates/ridl-transport-ws/src/frame.rs:20-23` (constants), the doc of
  `rule_tag` (it says "frame version 1")
- Modify: `crates/ridl-transport-ws/src/ws.rs` (`Upgrade` at 35–46, `handshake`
  at 49–102, `UpgradeRequest` and `upgrade_request` at 130–226, tests at 529 and
  following)
- Modify: `crates/ridl-transport-ws/src/server.rs` (`run`, the match on
  `ws::handshake` at about line 302; the doc of `Ended::Rejected` at 127–133)
- Modify: `crates/ridl-transport-ws/src/server/session.rs` (`Session` fields at
  89–128, `new` at 132–188, `attach` at 506–536, the doc of `carried`)
- Test: the `tests` modules of `ws.rs` and `session.rs`,
  `crates/ridl-transport-ws/tests/serve.rs`

**Interfaces:**

- Consumes: `Frame::Block` (R1).
- Produces:

  ```rust
  pub const FRAME_VERSION: u32 = 2;
  /// The WebSocket subprotocol name of binding version 2.
  pub const SUBPROTOCOL: &str = "ridl-ws-v2";
  /// The subprotocol name of binding version 1, which the provider selects
  /// only to refuse its `attach` (driftsys/ridl#1129).
  pub const SUBPROTOCOL_V1: &str = "ridl-ws-v1";

  // ws.rs
  pub(crate) enum Upgrade {
      /// `101 Switching Protocols`; the value is the frame version of the
      /// subprotocol selected: `FRAME_VERSION` for `ridl-ws-v2`, 1 for
      /// `ridl-ws-v1`.
      Accepted(u32),
      Rejected,
      Forbidden,
  }

  // session.rs
  impl<B: Backend> Session<B> {
      /// Records the frame version of the subprotocol the handshake selected.
      /// A new session assumes `FRAME_VERSION`.
      pub(crate) fn upgraded(&mut self, frame_version: u32);
  }
  ```

**Behaviour.** The handshake selects `ridl-ws-v2` when any
`Sec-WebSocket-Protocol` header offers it, in any position, else `ridl-ws-v1`
when offered, else answers `400 Bad Request` as today; the client's order does
not matter (§11.1: "the server selects the newest it serves"). The `101` names
the selected subprotocol. `run` calls `session.upgraded(v)` after
`Upgrade::Accepted(v)`. `Session::attach` refuses
`Reason::FrameVersionUnsupported`, then closes with 1000, when
`attach.frame_version() != FRAME_VERSION` or the connection's frame version is
not `FRAME_VERSION` (P-1); the order of the other checks is unchanged.

- [ ] **Step 1: Scaffold.** Add `SUBPROTOCOL_V1`; change `Upgrade::Accepted` to
      carry a `u32` and have `handshake` return `Accepted(FRAME_VERSION)`; add
      `upgraded` storing the value; leave `FRAME_VERSION` at 1 and `SUBPROTOCOL`
      at `ridl-ws-v1` for now. The existing assertions of `Upgrade::Accepted`
      read `Upgrade::Accepted(1)`, and every existing test passes. (Step 2 makes
      `GOOD_HEAD` offer `ridl-ws-v2`, and its assertions
      `Upgrade::Accepted(2)`.)

- [ ] **Step 2: Write the failing tests.**

  `ws.rs` tests (keep `pair`, `connect`, `handshake_on`; make `GOOD_HEAD` offer
  `ridl-ws-v2`, and add `V1_HEAD`, the same head offering `ridl-ws-v1`):

  ```rust
  fn a_client_offering_ridl_ws_v2_is_upgraded_with_it_selected()
  // tungstenite offers "ridl-ws-v2": Upgrade::Accepted(2), client sees Some("ridl-ws-v2")
  fn a_client_offering_both_versions_gets_ridl_ws_v2_in_either_order()
  // "ridl-ws-v2, ridl-ws-v1" and "ridl-ws-v1, ridl-ws-v2": Accepted(2), "ridl-ws-v2"
  fn a_client_offering_only_ridl_ws_v1_is_upgraded_with_it_selected()
  // replaces a_client_offering_ridl_ws_v1_is_upgraded_with_it_selected: Accepted(1), "ridl-ws-v1"
  fn the_101_response_carries_the_accept_key_and_the_subprotocol()
  // GOOD_HEAD → "...Sec-WebSocket-Protocol: ridl-ws-v2\r\n\r\n";
  // V1_HEAD → the same with ridl-ws-v1 and Accepted(1)
  ```

  Delete `a_client_offering_ridl_ws_v1_second_is_still_upgraded` (the case is in
  the either-order test). In
  `a_client_offering_only_another_subprotocol_gets_400`,
  `a_malformed_request_gets_400_before_the_origin_check` and the last lines of
  `a_version_other_than_13_gets_400_naming_version_13`, the subprotocol that no
  server serves becomes `ridl-ws-v3`.

  `session.rs` tests: `open()` (line 1285) and every `attach_msg(1, …)` that
  expects acceptance or a later reason use `FRAME_VERSION`. Replace
  `frame_version_2_is_refused` (line 1369) with

  ```rust
  fn frame_versions_1_and_3_are_refused()
  // attach_msg(1, …) and attach_msg(3, …): Attached(Refused(FrameVersionUnsupported)), Close(1000)
  fn an_attach_on_a_version_1_connection_is_refused_whatever_its_frame_version()
  // s.upgraded(1); attach_msg(FRAME_VERSION, …) → refused FrameVersionUnsupported, Close(1000);
  // a fresh session, upgraded(1), attach_msg(1, …) → the same
  fn an_attach_on_a_version_2_connection_is_accepted()
  // s.upgraded(2); open(&mut s) succeeds
  ```

  and in `the_attach_checks_run_in_the_design_order` (line 2250) the first row's
  version 2 becomes 1, and rows 2 to 4, version 1 today, use `FRAME_VERSION`.

  `tests/serve.rs`: every `"ridl-ws-v1"` in `connect(…)` and in
  `UPGRADE_REQUEST` becomes `SUBPROTOCOL`'s value, and every `Attach::new(1, …)`
  becomes `Attach::new(FRAME_VERSION, …)` (import both from
  `ridl_transport_ws::frame`). New:

  ```rust
  fn a_client_offering_both_versions_is_upgraded_with_v2_and_attached()
  // connect "ridl-ws-v2, ridl-ws-v1"; send Attach::new(2, 1, CATALOG.hash, CATALOG.name)
  // (the literal 2, not FRAME_VERSION); recv → Attached(Accepted); close; ClientClosed(None)
  fn a_version_1_client_is_upgraded_with_ridl_ws_v1_and_its_attach_refused()
  // connect "ridl-ws-v1"; send Attach::new(1, 1, CATALOG.hash, CATALOG.name);
  // recv → Attached(Refused(FrameVersionUnsupported)); recv_close → Some(CloseCode::Normal);
  // handle.join().0 == Ended::ServerClosed(1000)
  ```

- [ ] **Step 3: Run the tests and see them fail on their assertions.** Run:
      `cargo test --locked -p ridl-transport-ws --lib ws::tests` and
      `cargo test --locked -p ridl-transport-ws --lib server::session::tests`
      and `cargo test --locked -p ridl-transport-ws --test serve`. Expected:
      FAIL; a client offering only `ridl-ws-v2` gets `Rejected`, an `attach` of
      frame 1 is accepted, an `attach` of frame 2 is refused, and `upgraded(1)`
      changes nothing.
      `a_client_offering_only_ridl_ws_v1_is_upgraded_with_it_selected` passes
      already: it guards today's version 1 selection through the change.

- [ ] **Step 4: Implement.** Set `FRAME_VERSION` to 2 and `SUBPROTOCOL` to
      `ridl-ws-v2`; the selection in `upgrade_request`, the selected name in the
      `101`, `Accepted(version)`, the call of `upgraded` in `run`, and the check
      in `attach`. Update every doc that names `ridl-ws-v1` as the one
      subprotocol: `Upgrade`, `handshake`, `Ended::Rejected` ("did not offer
      `ridl-ws-v2` or `ridl-ws-v1`"), and the mentions of "frame version 1" in
      `rule_tag` and `carried` (frame version 2 carries the same `Rule` codes).

- [ ] **Step 5: Run the tests and see them pass.** Run:
      `cargo test --locked -p ridl-transport-ws`,
      `cargo test --locked -p ridl-transport-ws --features descriptor --test descriptor`,
      clippy and fmt. Expected: PASS.

- [ ] **Step 6: Commit.**

  ```bash
  git add crates/ridl-transport-ws
  git commit -m "feat(ridl-transport-ws)!: serve frame version 2 under ridl-ws-v2 and refuse version 1"
  ```

### Task R3: The provider sends a large `fixed` in blocks

**Model:** opus.

**Files:**

- Modify: `crates/ridl-transport-ws/src/server/session.rs` (`Session` fields and
  `new`; `read_fixed` and its doc at 712–760; a new private `push_blocks` beside
  `push` at 1145; tests: the test at 4016–4045, the `Fail` enum at 3396–3426 and
  `FaultyBackend::read_fixed` at 3449–3460)
- Modify: `crates/ridl-transport-ws/src/server.rs` (`limit_short` at 247–271 and
  its doc, the `serve` doc paragraph at 220–227, `Ended::ConfigRefused` at
  141–150)
- Modify: `crates/ridl-transport-ws/src/server/config.rs` (docs of
  `Note::ValueTooLarge` at 33–45 and `Note::MessageLimitTooSmall` at 75–90)
- Test: `session.rs` tests, `crates/ridl-transport-ws/tests/serve.rs`

**Interfaces:**

- Consumes: `Frame::Block`, `BLOCK_BYTES` (R1); `Session::upgraded`,
  `FRAME_VERSION` 2 (R2).
- Produces: no public item. Private:

  ```rust
  /// Pushes `payload` as the `block` frames of `target` under `envelope`:
  /// `ceil(len / BLOCK_BYTES)` frames, `index` from 0, each with `total`
  /// equal to `payload.len()`, every one but the last with `BLOCK_BYTES`
  /// bytes. The caller holds `payload.len()` within a `u32` limit.
  fn push_blocks(out: &mut Vec<Vec<u8>>, target: Target, envelope: Envelope, payload: &[u8]);
  ```

  and a `Session` field holding the size of one block message, computed in `new`
  as `Frame::Block { …, payload: &[] }.encoded_len() + BLOCK_BYTES` (4,130), the
  way `new` already derives `payload_cap`.

**Behaviour.** `read_fixed`: the read cap is `self.limit(target)` when the
message limit is at least one block message, else
`self.payload_cap.min(self.limit(target))` (P-5); a value over the cap is
recorded `Note::ValueTooLarge` and answered `contract(UnknownInteraction)`, as
today. A value of at most `payload_cap` bytes is one `publish` (`Live`, `seq` 0,
the backend's clock), as today. A larger one is `push_blocks` with that same
envelope in every block, pushed one after the other with no frame between them.
`limit_short`: a `fixed` needs
`min(largest_header() + member_limit, block message)`, where `member_limit` is
the table's limit or the default ceiling of the kind (P-4); every other kind is
unchanged.

- [ ] **Step 1: Write the failing tests.** In the `session.rs` tests, add
      `fn pattern(len: usize) -> Vec<u8>` (byte `i` is `i % 251`) and a helper
      that decodes each message of `out` as a `Frame::Block` of `FIXED`, asserts
      `index` 0 to n − 1 in order, `total`, the envelope and the length of each,
      and returns the payloads joined.

  ```rust
  fn a_fixed_of_16_137_bytes_is_sent_whole_and_one_of_16_138_bytes_in_4_blocks()
  // scanning(), default config: P(16137) → one message of 16_164 bytes,
  // publish(FIXED, 0, 0, Live(&P(16137))); P(16138) → lengths [4_130, 4_130, 4_130, 3_884],
  // joined == P(16138); notes empty
  fn the_16_blocks_of_a_65_280_byte_fixed_match_the_shared_vectors()
  // p.advance(Duration(0x0123_4567_89AB)); P(65280); lengths 15 × 4_130 then 3_874;
  // message k == msg(Frame::Block { target: FIXED, envelope (stamp 0x0123_4567_89AB, seq 0),
  //   total: 65_280, index: k, payload: &P(65280)[4096k..min(4096(k+1), 65_280)] });
  // the first 34 bytes of messages 0 and 15 are the shared headers
  fn a_fixed_of_an_exact_multiple_of_4096_bytes_ends_with_a_full_block()
  // P(32768) → 8 messages, each 4_130 bytes; joined == P(32768)
  fn a_lowered_message_limit_moves_the_split_to_the_limit_minus_27()
  // config 5_000 (no table): P(4973) → one publish of 5_000 bytes; P(4974) → [4_130, 912];
  // config 4_130: P(4103) → one publish of 4_130 bytes; P(4104) → [4_130, 42];
  // every message ≤ the configured limit
  fn a_fixed_above_the_default_ceiling_without_a_table_is_answered_unknown_interaction()
  // new Fail::FixedOverCeiling: FaultyBackend::read_fixed reports
  // Short { needed: FIXED_CEILING + 1 } when out is shorter, else fills
  // FIXED_CEILING + 1 bytes; faulty(Fail::FixedOverCeiling), read FIXED →
  // out == [unknown(FIXED)]
  ```

  Rewrite
  `a_read_of_a_fixed_above_one_message_is_answered_unknown_interaction_and_recorded`
  (line 4021) as
  `a_read_of_a_fixed_above_one_message_is_answered_in_blocks_and_the_session_goes_on`:
  20,000 bytes → lengths `[4_130; 4]` then 3,650, joined equal to the value, no
  note, and the signal subscribe that follows still answers. Keep
  `a_fixed_of_exactly_the_cap_is_sent_and_one_byte_more_is_not` (line 3316,
  message limit 4,096), `a_fixed_is_capped_after_the_buffer_grew_on_occurrences`
  (3198, message limit 1,024) and
  `a_fixed_above_the_limit_its_table_entry_gives_is_answered_unknown_interaction`
  (4047) as they are; they must pass unchanged. Add one sentence to the doc
  comment of each of the first two: it pins the rule for a message limit below
  one block message (P-5).

  `tests/serve.rs` (add a `FIXED` target, ordinal 5):

  ```rust
  fn a_config_below_one_block_message_is_refused_for_a_fixed_of_default_limit()
  // max 4_129; table(|t| (t.kind != Kind::Fixed).then_some(900)) → Ended::ConfigRefused,
  // notes == [MessageLimitTooSmall { target: FIXED, needed: 4_130 }]
  fn a_config_of_one_block_message_is_accepted_for_a_fixed_of_default_limit()
  // max 4_130, same table → attach accepted, no note
  fn a_fixed_that_fits_whole_needs_36_bytes_plus_its_limit()
  // fixed limit 3_000, others 900: max 3_035 → refused, needed 3_036; max 3_036 → accepted
  fn a_fixed_of_65_280_bytes_crosses_as_16_block_messages()
  // Loopback with P(65280) provisioned at ordinal 5; tungstenite read of FIXED →
  // 16 binary messages, lengths 15 × 4_130 then 3_874, each a Block with index k,
  // payloads joined == P(65280)
  ```

  and correct the doc of
  `the_default_config_is_accepted_for_a_catalog_with_a_fixed_of_default_limit`
  (line 1328): a `fixed` counts at most one block message, 4,130 bytes.

- [ ] **Step 2: Run the tests and see them fail on their assertions.** Run:
      `cargo test --locked -p ridl-transport-ws --lib server::session::tests`
      and `cargo test --locked -p ridl-transport-ws --test serve`. Expected:
      FAIL; the large reads are answered `unknown(FIXED)` with `ValueTooLarge`,
      and a `Config` of 4,130 is refused with `needed` 16,164.

- [ ] **Step 3: Implement** the cap, the split and `push_blocks` in
      `session.rs`, and the `fixed` rule in `limit_short`. Update the docs this
      changes: `read_fixed` (no "until the binding carries a fixed in blocks"),
      `Note::ValueTooLarge` (a `fixed` is not sent only above its limit, or when
      the message limit is below one block message),
      `Note::MessageLimitTooSmall` and `Ended::ConfigRefused` (for a `fixed`,
      the smaller of 36 + its limit and 4,130), and the `serve` paragraph on the
      check. Search the crate for `1129` and correct every sentence that says
      blocks are not built.

- [ ] **Step 4: Run the tests and see them pass.** Run the lane's commands of
      the Global Constraints. Expected: PASS.

- [ ] **Step 5: Commit.**

  ```bash
  git add crates/ridl-transport-ws
  git commit -m "feat(ridl-transport-ws): send a fixed larger than one message in blocks"
  ```

- [ ] **Step 6: The frozen surfaces.** Run `cargo xtask freeze update`, then
      `git diff --stat freeze/` and `cargo xtask freeze check`. Report every
      removed line and every break (expected: none, see the Global Constraints).
      Commit any changed dump as
      `test(ridl-transport-ws): refresh the frozen dumps`; edit no row of
      `freeze/surfaces.toml`.

---

## Lane ts

Branch from `origin/main`; scope `ts-transport-ws`; work in
`packages/ridl-transport-ws`. Close-out: rebase onto `main`, run the package's
commands and `just ts-e2e` (against the version 1 Rust server of `main`, it
proves that the version 2 client attaches with frame version 1), open the pull
request with "Refs driftsys/ridl#1129" and no closing keyword. This lane merges
first.

### Task T1: The `block` form in the TypeScript codec

**Model:** sonnet.

**Files:**

- Modify: `packages/ridl-transport-ws/src/frame.ts` (constants at 12–22, `Frame`
  at 142–182, `Violation` at 190–198, `decodeFrame` at 339–472, `encodedLength`
  at 553–575, `encode` at 584–688)
- Test: `packages/ridl-transport-ws/tests/vectors_test.ts`,
  `tests/decode_errors_test.ts`, `tests/codes_test.ts`

**Interfaces:**

- Produces (exported through `mod.ts`):

  ```ts
  /** The bytes of payload in every `block` but the last (binding version 2). */
  export const BLOCK_BYTES = 4_096;
  // a new member of `Frame`:
  | {
    form: "block";
    target: Target;
    envelope: Envelope;
    /** The length of the whole payload in bytes, 1 to 4294967295. */
    total: number;
    /** The position of the block in the payload, from 0. */
    index: number;
    /** The bytes of the block: a view of the message. */
    payload: Uint8Array;
  }
  // a new member of `Violation`:
  | "bad-block"
  ```
- `FRAME_VERSION` and `SUBPROTOCOL` do not change in this task (T2 changes
  them).

- [ ] **Step 1: Scaffold.** Add the constant and the two union members;
      `decodeFrame` still fails form 11 with `"unknown-code"`; `encodedLength`
      returns 34 plus the payload; `encode` writes only the `form` byte. Until
      T3, `session.ts` handles a decoded `block` in its `default` arm
      (`"unexpected-form"`), so it needs no change to type-check.
      `deno check mod.ts` passes.

- [ ] **Step 2: Write the failing tests.**

  `tests/vectors_test.ts` (a `hex` helper exists):

  ```ts
  Deno.test("every version 2 vector decodes and re-encodes byte for byte", ...)
  // attach-2 → { form: "attach", frameVersion: 2, encoding: 1, hash: 00..1f, name: "veh.cabin" }
  // block-last → { form: "block", target: { kind: Kind.Fixed, iface: 1, ord: 5 },
  //   envelope: { stamp: 0x0123456789ABn, seq: 0n }, total: 4098, index: 1,
  //   payload: Uint8Array.of(0xab, 0xcd) }
  // block-one → total 1, index 0, payload [0xab]
  Deno.test("the first block of a 4,098-byte payload carries 4,096 bytes of the pattern", ...)
  Deno.test("a last block of the largest total decodes, and with 4,096 bytes is a bad block", ...)
  // block-max-last ok; block-full ok; bad-max-last-full → "bad-block"
  Deno.test("the 16 blocks of a 65,280-byte payload encode to the shared sequence", async () => ...)
  // encode block k of P(65280) for k in 0..15; lengths 15 × 4130 then 3874;
  // SHA-256 (crypto.subtle.digest) of the joined bytes ==
  // "4738f9ca3db820c9597bcd0785a9b194a2b21eb0096f31cd94641b6934c26a58"
  ```

  `tests/decode_errors_test.ts`: replace "form 0 and form 11 are unknown codes"
  (line 145) with "form 0 and form 12 are unknown codes and a lone form 11 is
  too short" (`[0]` and `[12]` → `"unknown-code"`, `[11]` → `"too-short"`), and
  add "every single-block error is a violation" over the rows of the violation
  table, TypeScript column.

  `tests/codes_test.ts`: in "each data form code maps to its form", a
  `roundTrip` of `[11, ...T(5), ...E, ...u32(1), ...u32(0), 0xab]` to
  `{ form: "block", target: { kind: Kind.Fixed, iface: 1, ord: 2 }, envelope,
  total: 1, index: 0, payload: Uint8Array.of(0xab) }`.

- [ ] **Step 3: Run the tests and see them fail on their assertions.**

  ```bash
  deno test --allow-read tests/vectors_test.ts tests/decode_errors_test.ts tests/codes_test.ts
  ```

  Expected: FAIL; a block decodes to
  `{ ok: false, class: "violation", violation: "unknown-code" }`.

- [ ] **Step 4: Implement the decoder and the encoder in `frame.ts`**, with the
      checks in the order of the shared section (`targetAt` already gives
      `"unknown-code"` and `"zero-number"`). The decoded `payload` is a view of
      the message, as for the other forms. Update the module doc.

- [ ] **Step 5: Run the package's commands** (Global Constraints). Expected:
      PASS.

- [ ] **Step 6: Commit.**

  ```bash
  git add packages/ridl-transport-ws
  git commit -m "feat(ts-transport-ws): decode and encode the block form of frame version 2"
  ```

### Task T2: Offer `ridl-ws-v2, ridl-ws-v1` and attach with the selected version

**Model:** opus.

**Files:**

- Modify: `packages/ridl-transport-ws/src/frame.ts:12-15` (constants)
- Modify: `packages/ridl-transport-ws/src/session.ts` (`connect` at 175–222,
  `#onOpen` at 571–578, `#onMessage` at 580–621, a new private field for the
  session's frame version)
- Modify: `packages/ridl-transport-ws/README.md` (the first paragraph names
  `ridl-ws-v1`)
- Test: `packages/ridl-transport-ws/tests/session_test.ts`

**Interfaces:**

- Consumes: the `block` member of `Frame` (T1).
- Produces:

  ```ts
  /** The frame version this binding carries. */
  export const FRAME_VERSION = 2;
  /** The WebSocket subprotocol name of binding version 2. */
  export const SUBPROTOCOL = "ridl-ws-v2";
  /** The subprotocol name of binding version 1, which a server of frame version 1 selects. */
  export const SUBPROTOCOL_V1 = "ridl-ws-v1";
  ```

**Behaviour.** `connect` validates the `attach` as today (before any socket),
then creates the socket with `[SUBPROTOCOL, SUBPROTOCOL_V1]`. On open: the
server selected `SUBPROTOCOL`, so the session sends the `attach` with
`frameVersion` 2; or `SUBPROTOCOL_V1`, so it sends it with `frameVersion` 1 and
the session is a frame-version-1 session; any other value, the empty string
included, is the violation `"no-subprotocol"` as today. On a frame-version-1
session, a decoded `block` is the violation `"unknown-code"` (P-6). The TSDoc of
`connect` and the README name both subprotocols.

- [ ] **Step 1: Write the failing tests** in `tests/session_test.ts` (import
      `SUBPROTOCOL_V1`; until Step 3 it is a type error, so add the export first
      with the value `"ridl-ws-v1"`, leaving the other two constants as they
      are):

  ```ts
  Deno.test("connect offers ridl-ws-v2 then ridl-ws-v1 and attaches with frame version 2 on ridl-ws-v2", ...)
  // replaces "connect sends attach with the catalog, frame version 1 and proto3" (line 137):
  // socket.protocols deepEqual ["ridl-ws-v2", "ridl-ws-v1"]; open("ridl-ws-v2");
  // frames(socket) deepEqual [{ form: "attach", frameVersion: 2, encoding: 2, hash: HASH, name: "veh.cabin" }]
  Deno.test("the session attaches with frame version 1 when the server selects ridl-ws-v1", ...)
  // open("ridl-ws-v1"); the attach has frameVersion 1; attached(accepted) resolves connect; notes []
  Deno.test("a block on a frame version 1 session is an unknown-code violation", ...)
  // version 1 session, read(FIXED) waiting; deliver the block-one frame →
  // socket.closes [{}], notes [{ note: "violation", violation: "unknown-code" }], the read rejects down
  Deno.test("a server that selects ridl-ws-v3 is a no-subprotocol violation", ...)
  ```

  Every other test keeps `socket.open(SUBPROTOCOL)`.

- [ ] **Step 2: Run the tests and see them fail on their assertions.** Run:
      `deno test --allow-read tests/session_test.ts`. Expected: FAIL; the socket
      is offered `["ridl-ws-v1"]`, `open("ridl-ws-v2")` is a `"no-subprotocol"`
      violation, and a block on a version 1 session is `"unexpected-form"`. The
      `ridl-ws-v1` and `ridl-ws-v3` tests pass already: they guard today's
      behaviour through the change.

- [ ] **Step 3: Implement** the constants (`FRAME_VERSION` 2, `SUBPROTOCOL`
      `ridl-ws-v2`), the offer, the choice in `#onOpen` and the `block` check in
      `#onMessage`; update the TSDoc of `connect` and the README.

- [ ] **Step 4: Run the package's commands.** Expected: PASS.

- [ ] **Step 5: Commit.**

  ```bash
  git add packages/ridl-transport-ws
  git commit -m "feat(ts-transport-ws)!: offer ridl-ws-v2 and ridl-ws-v1 and attach with the selected frame version"
  ```

### Task T3: Reassemble the blocks of a `fixed`

**Model:** opus.

**Files:**

- Create: `packages/ridl-transport-ws/src/reassembly.ts` (not exported from
  `mod.ts`)
- Modify: `packages/ridl-transport-ws/src/session.ts` (`Note` at 106–131,
  `#onMessage`, `#onPublish` at 677–734, `#fail` at 804–814)
- Modify: `packages/ridl-transport-ws/src/errors.ts:16-20` (`SessionViolation`)
- Modify: `packages/ridl-transport-ws/README.md` (a section on a large `fixed`)
- Test: Create `packages/ridl-transport-ws/tests/reassembly_test.ts`; modify
  `tests/session_test.ts`

**Interfaces:**

- Consumes: the `block` frame and `BLOCK_BYTES` (T1); the frame-version-2
  session (T2).
- Produces:

  ```ts
  // src/reassembly.ts
  export type BlockFrame = Extract<Frame, { form: "block" }>;
  /** What one block did to the slot. */
  export type BlockStep =
    | { step: "collecting" }                     // copied; more blocks follow
    | { step: "complete"; payload: Uint8Array }  // the last block copied; the slot is empty
    | { step: "over-limit" }                     // index 0 with total above the limit; nothing copied
    | { step: "skipping" }                       // a block of a refused payload, sequence checked
    | { step: "sequence" };                      // a sequence error of B-6; the slot is empty
  /** The one reassembly of a session (B-6). */
  export class Reassembly {
    /** Applies one decoded block. `limit` is the member's limit; it is read at index 0 only. */
    accept(block: BlockFrame, limit: number): BlockStep;
    /** Whether a payload of `(iface, ord)` is being collected or skipped. */
    holds(iface: number, ord: number): boolean;
    /** Drops the payload in progress. */
    clear(): void;
  }

  // errors.ts: SessionViolation gains "block-sequence".
  // session.ts: the `reason` of a "discarded" Note gains "over-limit".
  ```

**Behaviour of `Reassembly`.** With no payload in progress, a block with `index`
0 starts one: above `limit` it is `over-limit` and the slot skips the remaining
`n − 1` blocks (none when `n` is 1); otherwise it allocates `total` bytes,
copies the block at offset 0, and is `complete` when `n` is 1, else
`collecting`. With no payload in progress, `index` above 0 is `sequence`. With a
payload in progress, a block is `sequence` when its `index` is 0, is not the
next expected, or its `iface`, `ord`, `stamp`, `seq` or `total` differ; else it
is copied at `BLOCK_BYTES × index` (or not, when skipping) and the last one ends
the payload (`complete`, or `skipping` with the slot empty).

**Behaviour of the session** (frame-version-2 session, after `attached`):

- a `block` goes to `Reassembly.accept` with `this.#limitOf(frame.target)`;
  `complete` hands
  `{ form: "publish", target, envelope, state: { provenance: "live", payload } }`
  to `#onPublish`, which settles the waiting reads or records `not-subscribed`
  as for a `publish` that arrived whole (P-3);
- `over-limit` rejects every `read` at the head of the identity's queue with
  `SessionError` `corrupt`, as `#onPublish` does for an oversized `publish`, and
  records `{ note: "discarded", reason: "over-limit", frame }` with the first
  block as `frame`;
- `sequence` is `#violate("block-sequence")`;
- a whole `publish` whose `(iface, ord)` the slot `holds` is
  `#violate("block-sequence")`;
- `#fail` calls `clear()`; a block after the session ended is ignored by the
  existing `#ended` check.

- [ ] **Step 1: Scaffold.** Create `src/reassembly.ts` with the types and a
      `Reassembly` whose `accept` returns `{ step: "sequence" }`, `holds`
      returns `false` and `clear` does nothing; add `"block-sequence"` and
      `"over-limit"` to the unions. The session does not call it yet.
      `deno check mod.ts` passes.

- [ ] **Step 2: Write the failing tests.** `tests/reassembly_test.ts`, with a
      `block(index, total, payload, change?)` builder of a `BlockFrame` of
      interface 1, ordinal 5, stamp `0x0123456789ABn`, `seq` 0n, and `P(n)`:

  ```ts
  Deno.test("the blocks of a payload complete in order, each at 4,096 times its index", ...)
  // total 8193 = [4096, 4096, 1] of P(8193): collecting, collecting, complete with payload == P(8193)
  Deno.test("a payload of one block completes at once", ...)          // total 1, and total 4096
  Deno.test("an exact multiple of 4,096 completes after its last full block", ...) // P(32768), 8 blocks
  Deno.test("a total equal to the limit is collected and one byte more is over-limit", ...)
  // limit 8192: total 8192 → complete after 2; total 8193 → over-limit, skipping, skipping;
  // holds(1, 5) is true until the last skipped block and false after it
  Deno.test("a missing, a repeated and a reordered block are sequence errors", ...)  // n = 3: 0,2 / 0,1,1 / 0,2,1
  Deno.test("an index above 0 with nothing in progress is a sequence error", ...)
  Deno.test("an index 0 while a payload is in progress is a sequence error, for the same identity too", ...)
  Deno.test("a block whose interface, ordinal, stamp, seq or total differs is a sequence error", ...)
  Deno.test("the sequence of a skipped payload is still checked", ...) // over-limit, then index 2 → sequence
  Deno.test("after a payload completes, the next index 0 starts a new payload", ...)
  Deno.test("clear drops the payload in progress", ...) // 0, clear, 1 → sequence; 0 → collecting
  ```

  `tests/session_test.ts` (the harness of `attached`/`attachedWith`; a helper
  that delivers the blocks of `P(n)` for `FIXED`):

  ```ts
  Deno.test("a read of a fixed is settled by the shared block-first and block-last messages", ...)
  // deliver the exact bytes; payload length 4098, P(4096) then ab cd; SHA-256 8830a559…; notes []
  Deno.test("a read of a 65,280-byte fixed is settled by its 16 blocks", ...) // payload deepEqual P(65280)
  Deno.test("frames of other forms between two blocks are applied and the payload is still delivered", ...)
  // block 0; an occurrence of a subscribed EVENT (delivered); a whole publish of another fixed
  // (ord 6) that settles its own read; block 1 → the first read resolves
  Deno.test("a total above the lowered limit of a fixed rejects the read with corrupt before any copy and the session goes on", ...)
  // interfaces with { iface: 1, ord: 5, kind: "fixed", limit: 20000 }: total 20001 → read rejects
  // corrupt; notes [{ note: "discarded", reason: "over-limit", frame: block 0 }]; the other 4
  // blocks add nothing; a new read answered by a whole publish resolves; total 20000 in blocks resolves
  Deno.test("a total above the default ceiling of a fixed is over-limit without tables", ...) // 65281
  Deno.test("each block sequence error ends the session as block-sequence", ...)
  // for each case of B-6 (missing, repeated, reordered, index above 0 with none in progress,
  // two payloads interleaved, identity, envelope or total differing, a whole publish of the
  // identity in progress): socket.closes [{}], the read rejects down, closed.violation "block-sequence"
  Deno.test("a block the codec refuses ends the session as bad-block", ...) // bad-last-long bytes
  Deno.test("the end of the session drops a payload in progress and the read rejects with down", ...)
  Deno.test("blocks delivered after close are ignored", ...)            // no note, no throw
  Deno.test("two reads of one fixed are settled by the first payload and the second is discarded not-subscribed", ...)
  Deno.test("a block payload no read waits for is reassembled and discarded not-subscribed", ...)
  ```

- [ ] **Step 3: Run the tests and see them fail on their assertions.** Run:
      `deno test --allow-read tests/reassembly_test.ts tests/session_test.ts`.
      Expected: FAIL; `accept` answers `sequence` everywhere, and a block on a
      version 2 session is an `"unexpected-form"` violation. The `bad-block` and
      "after close" tests pass already: they guard today's behaviour. The
      end-of-session test asserts that `notes` holds no violation, so that it
      fails until a block is collected.

- [ ] **Step 4: Implement** `Reassembly`, the session wiring and the docs: the
      TSDoc of `Note` (`over-limit`), of `SessionViolation` (`block-sequence`),
      of `Session.read` (a `fixed` may arrive in blocks), and a README section
      that states the limit, the `corrupt` rejection with the `over-limit` note,
      and the end of the session on a sequence error.

- [ ] **Step 5: Run the package's commands.** Expected: PASS.

- [ ] **Step 6: Commit.**

  ```bash
  git add packages/ridl-transport-ws
  git commit -m "feat(ts-transport-ws): reassemble the blocks of a fixed within its limit"
  ```

---

## After both lanes (third pull request, no briefs yet)

- **A1, end-to-end (design task 8).** The three `fixed` values of the design
  note §3 (65,280 bytes of `P(65280)`, 16,137 and 16,138 bytes) in
  `crates/ridl-transport-ws/tests/support/provider.rs` (which
  `examples/loopback_server.rs` includes) and `e2e/catalog.ts`, read by
  `e2e/deno_test.ts` and `e2e/browser_test.ts` with `onNote` recording no
  discard; and `crates/ridl-transport-ws/tests/ts_ceilings.rs` extended so that
  `BLOCK_BYTES`, `FRAME_VERSION`, `SUBPROTOCOL` and `SUBPROTOCOL_V1` are equal
  on both sides.
- **A2, records (design task 9).** The frame specification (§4, §5.5, §5.6,
  §6.3, §9.4, §10, §11.1 with a binding version 2 section and its vector table,
  §12 items 17 to 24, §13 streams at version 3; the §11.1 "The sizes" sentence
  "Until the binding carries a `fixed` in blocks"); `xtask/src/freeze/wire.rs`
  (`check_attach` requires frame version 1, `check_vector` knows forms 1 to 10,
  `frame_vectors` reads the first Id/Vector/Hex table only, and the header-table
  test lists ten forms), then `cargo xtask freeze update`, which adds the
  `frame-vector` lines of the version 2 table; `tests/vectors.rs` and
  `tests/vectors_test.ts` read the version 2 table in place of their constants;
  the `websocket` row of `crates/ridl-ir/src/codegen/bindings.rs` to version
  `"2"`, with the tests that assert `"1"`
  (`crates/ridl-ir/src/codegen/deployment.rs:769`,
  `crates/ridl-transport-ws/tests/sizes.rs:19`); ADR-0030 decision 15's
  threshold sentence and its Status sentence; `docs/design/ridl-transport-ws.md`
  (constants at 131, the `Config` check at 665–679, D-20 at 708),
  `docs/design/ridl-transport-ws-ts.md` (7, 136, 194, D-19 at 395–398),
  `docs/technotes/websocket-binding-rationale.md` (194, and streams at version
  3), `docs/technotes/walking-skeleton-architecture.md:376`; replace the
  `driftsys/ridl#1129` citations in `crates/` and `packages/` with the
  specification section; garden the design note and this plan with
  `sdd-gardening`.

## Design note items found out of date

Checked against `main` at 94b52730.

1. Task 6a is already done by #1132 (P-8).
2. §6's dependency on #1128, and §4's on #1108 and #1128: both issues are
   closed. `ridl-loopback` refuses a provisioned `fixed` above its member's
   limit, `ridl-rt` exports the three ceilings and `default_ceiling`, and the
   TypeScript session checks each received payload against its limit.
3. Task 3 places "the `Config` check at attach of decision 10" in the session;
   it is `limit_short` in `server.rs`, which `serve` runs before the opening
   handshake, and it counts a `fixed` as 36 + min(limit, 16,128) today (P-4).
4. Task 4 says the `wire` dump gains the version 2 rows in the Rust lane; the
   dump reads its vectors from the frame specification, so it changes only in
   A2, and `xtask/src/freeze/wire.rs` refuses an `attach` of frame 2 and a form
   11 until A2 changes it. `runtime-crates` has no dump file; it is a
   `cargo semver-checks` comparison, and `cargo xtask freeze update` does not
   write it.
5. Task 7 puts the subprotocol tests in `tests/version_test.ts`; that file
   checks the package version against the workspace version. They go in
   `tests/session_test.ts` (T2).
6. Task 5 names `tests/vectors_test.ts` for the version 2 vectors; that test
   reads the specification's table and asserts its id list, so the version 2
   vectors are constants until A2 (P-9).
7. The note does not name the existing tests the change breaks:
   `form_0_and_form_11_are_unknown_codes` (Rust and TypeScript),
   `frame_version_2_is_refused`, `the_attach_checks_run_in_the_design_order`,
   `a_read_of_a_fixed_above_one_message_is_answered_unknown_interaction_and_recorded`,
   the `ws.rs` handshake tests that use `ridl-ws-v2` as the subprotocol no
   server serves, the `ridl-ws-v1` literals on 23 lines and three
   `Attach::new(1, …)` of `tests/serve.rs`, the version 1 `open()` of the
   session tests, and the TypeScript test that expects `[SUBPROTOCOL]` as the
   offer.
8. The note says the TypeScript side can be written in parallel, which holds,
   but not that the merges are ordered: the cabin demos and the TypeScript
   end-to-end tests fail on lane rust alone (Global Constraints).
9. B-1 does not say what the provider does with an `attach` of frame 2 on a
   `ridl-ws-v1` connection (P-1), and B-6 names no code for the new failures
   (P-2).
10. B-6 assumes a consumer member table that the TypeScript package does not
    have (P-3).
11. B-3 and task 9 name only `bindings.rs` for the `websocket` row's version;
    `deployment.rs:769` and `tests/sizes.rs:19` assert `"1"` too (A2).
