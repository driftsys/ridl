# `ridl-transport-ws` Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** A new crate `crates/ridl-transport-ws`: a `no_std` codec for the frame
specification §11.1 WebSocket binding, and a std server that serves a `ridl-rt`
backend's observe-and-call ports over one WebSocket connection.

**Architecture:** `frame` (core only) encodes and decodes the ten forms. `ws`
(private, std) is the server side of RFC 6455, written here with no dependency.
`server` holds a private sans-IO `Session` (frames in, frames out, over a
`Backend`) and the public blocking `serve` that drives it over a `Connection`
the embedder accepted.

**Tech Stack:** Rust, edition 2021, `rust-version = "1.83"`; `ridl-rt` as the
only normal dependency; dev-dependencies `ridl-loopback`, `ridl-ir`,
`tungstenite` (default features).

**Spec:** `docs/wip/2026-10-19-ridl-transport-ws-design.md` (decisions D-1 to
D-25, tests T-1 to T-6). The wire rules are the frame specification
`docs/specification/frame-specification.md` §4 to §9 and §11.1. Read both before
a task; the plan cites decisions by number and does not restate them.

## Global Constraints

- Normal dependencies: `ridl-rt` only, with no feature turned on (D-2).
- `frame` builds with `#![no_std]` and no `alloc`; the `server` feature
  (default) enables `std` (D-1).
- Edition `"2021"`, `rust-version = "1.83"` as literals, as in
  `crates/ridl-loopback/Cargo.toml`. No `[package.metadata.cargo-semver-checks]`
  block (design §10).
- The crate never calls `bind`, `listen`, `spawn` or `connect` outside tests
  (C-2).
- The server never decodes, verifies or re-encodes a payload.
- Prose (comments, rustdoc, commits) is plain literal English; no story ids
  (`E11.9`) in `crates/` (`just story-id-check`); issue links as
  `driftsys/ridl#N`.
- Commits: Conventional Commits, scope `ridl-transport-ws` (added in Task 1),
  trailer `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- Implementers run the crate's own tests (`cargo test -p ridl-transport-ws`),
  `cargo clippy -p ridl-transport-ws --all-targets -- -D warnings` and
  `cargo fmt --all --check`. The full `just build` is run once, at the end.

## Review Focus

1. A client that sends `attach` split across several WebSocket fragments, or a
   TCP read that ends mid-frame: the message is reassembled, not rejected (Task
   3 test `a_message_split_across_reads_and_fragments_is_reassembled`).
2. A backend value larger than the server's read buffer: the read is retried
   with `needed` bytes, not reported as an error (Task 4 test
   `a_signal_larger_than_the_first_buffer_is_read_again`).
3. A backend whose channel `seq` restarts at 1 (a replaced writer): the client
   keeps receiving updates with increasing session `seq` (Task 4 test
   `a_backend_seq_restart_still_reaches_the_client`).
4. A client that disconnects with calls in flight: the backend calls are
   forgotten and the call slots are free again (Task 5 test
   `the_end_of_a_session_forgets_its_calls_in_flight`).
5. A client that stops answering pings without closing the TCP connection:
   `serve` returns within `liveness_timeout` plus one `poll_interval` (Task 6
   test `a_silent_client_ends_the_session_at_the_liveness_timeout`).

---

### Task 1: The crate and the frame codec

Model: sonnet.

**Files:**

- Create: `crates/ridl-transport-ws/Cargo.toml`,
  `crates/ridl-transport-ws/src/lib.rs`,
  `crates/ridl-transport-ws/src/frame.rs`,
  `crates/ridl-transport-ws/tests/vectors.rs`,
  `crates/ridl-transport-ws/tests/decode_errors.rs`,
  `crates/ridl-transport-ws/LICENSE` (copy of `crates/ridl-loopback`'s license
  file layout, if it has one; otherwise follow `ridl-loopback`)
- Modify: `.git-std.toml` (add scope `ridl-transport-ws` after `ridl-lsp`'s
  neighbours), `justfile` `wasm-check` (two lines, below)

**Interfaces:**

- Produces (all in `ridl_transport_ws::frame`):

```rust
pub const FRAME_VERSION: u32 = 1;
pub const SUBPROTOCOL: &str = "ridl-ws-v1";
pub const MIN_MESSAGE_LIMIT: usize = 294;
pub const ENVELOPE_BYTES: usize = 16;

pub struct Target { pub kind: Kind, pub iface: InterfaceNo, pub ord: Ordinal }
pub struct Attach<'a> { /* private */ }
impl<'a> Attach<'a> {
    pub fn new(frame_version: u32, encoding: u8, hash: CatalogHash, name: &'a str) -> Result<Self, NameError>;
    pub fn frame_version(&self) -> u32; pub fn encoding(&self) -> u8;
    pub fn hash(&self) -> &CatalogHash; pub fn name(&self) -> &'a str;
}
pub enum NameError { Empty, TooLong { len: usize } }
pub enum Reason { UnknownCatalog, CatalogMismatch, EncodingUnsupported, FrameVersionUnsupported, Range }
pub enum AttachOutcome { Accepted, Refused(Reason) }
pub enum AnswerOutcome { Accepted, Contract(Contract) }
pub enum PublishState<'a> { Init, Live(&'a [u8]), InvalidDeclared }
pub enum ResponseOutcome<'a> { Accepted, Reply(&'a [u8]), Contract(Contract), Corrupt, Busy }
pub enum Frame<'a> {
    Attach(Attach<'a>), Attached(AttachOutcome),
    Subscribe(Target), Unsubscribe(Target), Read(Target),
    Answer { target: Target, outcome: AnswerOutcome },
    Publish { target: Target, envelope: Envelope, state: PublishState<'a> },
    Occurrence { target: Target, envelope: Envelope, payload: &'a [u8] },
    Request { target: Target, envelope: Envelope, payload: &'a [u8] },
    Response { target: Target, envelope: Envelope, correlation: u64, outcome: ResponseOutcome<'a> },
}
impl Frame<'_> {
    pub fn encoded_len(&self) -> usize;
    pub fn encode(&self, out: &mut [u8]) -> Result<usize, Short>;
}
pub struct Short { pub needed: usize }
pub fn decode(msg: &[u8]) -> Result<Frame<'_>, DecodeError>;
pub enum DecodeError { Violation(ProtocolViolation), Discard(Discarded) }
pub enum ProtocolViolation { TooShort, ControlTooLong, UnknownCode, OutcomeNotAllowed,
    NonzeroDetail, ZeroNumber, BadNameLength, BadName }
pub enum Discarded { UnexpectedPayload }
pub fn encoding_tag(name: &str) -> Option<u8>; // "flatbuffers" 1, "proto3" 2, "repr-c" 3
```

Derive `Clone, Copy, Debug, PartialEq, Eq` where the fields allow it; mark the
error enums and `Frame` `#[non_exhaustive]`. A `Contract::InvalidValue` decodes
with `type_name: None` (D-3). `Contract`, `Rule` and `Kind` come from `ridl_rt`;
`Rule` and `Contract` are `#[non_exhaustive]` upstream, so `encode` maps every
variant known to `ridl-rt` 0.11 by its §4 or §9.6 code and treats any other as a
programming error with `unreachable!` documented on the match arm. A
`ProtocolViolation` must not be raised for a valid message.

- [ ] **Step 1: Scaffold.** `Cargo.toml` mirrors
      `crates/ridl-loopback/Cargo.toml` (version, license and repository from
      the workspace; description "The WebSocket binding of the ridl frame: a
      no_std codec and a server for ridl-rt ports."), with
      `[features] default = ["server"]`, `server = []`, and
      `exclude = ["tests/"]` with a comment saying why (path dev-dependencies
      carry no version, as in `ridl-loopback`). `lib.rs` is
      `#![cfg_attr(not(feature = "server"), no_std)]`, `pub mod frame;`. Add the
      scope to `.git-std.toml`.

- [ ] **Step 2: Write the failing test `tests/vectors.rs`.** Test
      `every_spec_vector_decodes_and_reencodes_byte_for_byte`: read
      `../../docs/specification/frame-specification.md` (via
      `env!("CARGO_MANIFEST_DIR")`), take the rows of the table under "**Test
      vectors.**" in §11.1, strip spaces and backticks from the `Hex` cell,
      decode with `frame::decode`, assert the frame equals the expected value
      for that `Id`, and assert `encode` gives the same bytes. Expected values,
      from the `Vector` cells: `attach-1` (version 1, encoding 1, hash `00..1f`,
      name `veh.cabin`); `attached-accepted`; `attached-mismatch`
      (`Refused(CatalogMismatch)`); `subscribe-signal` (Signal, 1, 2);
      `answer-accepted`; `publish-live` (stamp `0x0123456789AB`, seq 7,
      `Live(&[0xff, 0x01])`); `request-command` (Command, 1, 3, seq 1, payload
      `[0x01]`); `response-accepted` (stamp `0x0123456789AC`, seq 1, correlation
      1); `response-violation` (Query, 1, 4, seq 2, correlation 2,
      `Contract(InvalidValue(Violation { type_name: None, rule: Range }))`). The
      test also fails when the table has a row whose `Id` it does not know, so a
      new vector cannot be skipped.

- [ ] **Step 3: Run it; expect FAIL**
      (`cargo test -p ridl-transport-ws --test vectors`: `decode` not defined).

- [ ] **Step 4: Implement `frame.rs`** per §11.1 item 2's layout table and the
      codes of §4 and §9.6. Little-endian fixed width; payload is every byte
      after the header; presence decided from the header only.

- [ ] **Step 5: Run; expect PASS.**

- [ ] **Step 6: Write `tests/decode_errors.rs`**, one test per row, each
      asserting the exact `DecodeError`: `an_empty_message_is_too_short`,
      `a_header_cut_short_is_too_short` (the `publish-live` vector minus its
      last header byte), `a_subscribe_with_trailing_bytes_is_control_too_long`,
      `form_0_and_form_11_are_unknown_codes`, `kind_6_is_an_unknown_code`,
      `provenance_4_is_an_unknown_code`,
      `a_refused_response_is_an_outcome_not_allowed`,
      `an_accepted_attached_with_detail_1_is_nonzero_detail`,
      `interface_0_is_a_zero_number`, `ordinal_0_is_a_zero_number`,
      `name_len_0_is_a_bad_name_length`,
      `name_len_longer_than_the_message_is_a_bad_name_length`,
      `a_name_that_is_not_utf8_is_a_bad_name`,
      `an_init_publish_with_bytes_after_its_header_is_discarded`,
      `an_accepted_response_with_bytes_after_its_header_is_discarded`,
      `a_live_publish_with_an_empty_payload_decodes` (zero-byte payload is
      valid), `attach_new_refuses_an_empty_name_and_a_256_byte_name`,
      `encode_into_a_short_buffer_reports_the_needed_length`. Run; fix until
      PASS.

- [ ] **Step 7: Add to `justfile` `wasm-check`**, after the
      `ridl-rt
  --all-features` line, with a comment that this is the proof the
      codec needs no standard library (design C-3):
      `cargo check --target wasm32-unknown-unknown -p ridl-transport-ws --no-default-features`
      and
      `cargo check --target thumbv7em-none-eabihf -p ridl-transport-ws --no-default-features`.
      Run `just wasm-check`; expect exit 0.

- [ ] **Step 8: Commit**
      `feat(ridl-transport-ws): add the crate and the frame codec`.

### Task 2: The binding sizes test (driftsys/ridl#718)

Model: sonnet.

**Files:** Create `crates/ridl-transport-ws/tests/sizes.rs`; possibly modify
`crates/ridl-ir/src/codegen.rs` (see Step 1).

**Interfaces:** Consumes Task 1's `Frame`, `Attach`, `ENVELOPE_BYTES`.

- [ ] **Step 1: Find the public route to the `websocket` row.** Look for how
      `crates/ridl-ir` tests or `crates/ridlc` tests build a deployment section
      whose `bindings` field holds the rendered `v1::Binding` rows. If a public
      route reads the row in about 40 lines of setup or fewer, use it. Otherwise
      add to `crates/ridl-ir/src/codegen.rs`:
      `pub fn known_bindings() -> Vec<v1::Binding> { bindings::bindings() }`
      with rustdoc "The transport bindings the toolchain knows, as the
      deployment section renders them." (design T-2). Record which route was
      taken in the commit body.

- [ ] **Step 2: Write the failing tests:**
      `the_largest_data_plane_message_is_header_plus_envelope_plus_payload`: for
      `len` in `[0, 1, 255]`, a `Response` with
      `ResponseOutcome::Reply(&vec![0; len])` has
      `encoded_len() == frame_header_max_bytes + envelope_bytes + len`, where
      both numbers come from the row named `"websocket"`, and the row's
      `version` is `"1"`; `every_other_data_plane_form_is_no_larger`: `Publish`
      `Live`, `Occurrence` and `Request` with the same payload encode to at most
      that sum; `the_largest_attach_is_294_bytes`: an `Attach` with a 255-byte
      name encodes to `MIN_MESSAGE_LIMIT`. Run; expect PASS once the route
      compiles (these tests pin the row against the codec; a red step here means
      the row or the codec is wrong — stop and report which).

- [ ] **Step 3: Commit**
      `test(ridl-transport-ws): check the websocket row against written frames`
      with `Refs: #718` in the body.

### Task 3: The WebSocket server side (`ws`)

Model: opus.

**Files:** Create `crates/ridl-transport-ws/src/ws.rs`,
`crates/ridl-transport-ws/src/ws/sha1.rs`,
`crates/ridl-transport-ws/src/ws/base64.rs`; test
`crates/ridl-transport-ws/tests/ws.rs` plus unit tests in the two hash files.
Add `tungstenite` (default features; newest version that resolves under
`--locked` with the pinned toolchain) as a dev-dependency.

**Interfaces:** Produces (crate-private, used by Task 6; expose through a
`#[doc(hidden)] pub mod __test` only if the integration tests need it — prefer
driving `ws` through unit tests in `src/ws.rs` with a `TcpStream` pair):

```rust
pub(crate) enum Upgrade { Accepted, Rejected }
pub(crate) fn handshake<S: Read + Write>(stream: &mut S) -> io::Result<Upgrade>;
pub(crate) struct Parser { /* buffered, incremental */ }
impl Parser {
    pub(crate) fn new(max_message_bytes: usize) -> Self;
    pub(crate) fn feed(&mut self, bytes: &[u8]);
    pub(crate) fn next(&mut self) -> Option<Result<Event, CloseCode>>;
}
pub(crate) enum Event { Binary(Vec<u8>), Ping(Vec<u8>), Pong, Close(Option<u16>) }
pub(crate) struct CloseCode(pub u16); // NORMAL 1000, GOING_AWAY 1001, PROTOCOL 1002, UNSUPPORTED 1003, TOO_BIG 1009
pub(crate) fn write_binary(w: &mut impl Write, msg: &[u8]) -> io::Result<()>;
pub(crate) fn write_ping(w: &mut impl Write) -> io::Result<()>;
pub(crate) fn write_pong(w: &mut impl Write, payload: &[u8]) -> io::Result<()>;
pub(crate) fn write_close(w: &mut impl Write, code: CloseCode) -> io::Result<()>;
```

`handshake` reads at most 8 KiB of request head (D-19), answers `101` with
`Sec-WebSocket-Protocol: ridl-ws-v1` when offered (D-20), and `400 Bad Request`
on anything else, returning `Rejected`.

- [ ] **Step 1: SHA-1 and base64 unit tests first:** SHA-1 of `""`, `"abc"` and
      the 448-bit FIPS 180 message against their published digests; base64 of
      `""`, `"f"`, `"fo"`, `"foo"`, `"foob"`, `"fooba"`, `"foobar"` (RFC 4648
      §10);
      `accept_key("dGhlIHNhbXBsZSBub25jZQ==") == "s3pPLMBiTxaQ9kYGJfbf+GxwOkk="`
      (RFC 6455 §1.3). Fail, implement, pass.
- [ ] **Step 2: Tests against `tungstenite`** (client on a thread, server side
      driven over the accepted `TcpStream`; the crate's own code never binds —
      the test binds `127.0.0.1:0`):
      `a_client_offering_ridl_ws_v1_is_upgraded_with_it_selected`,
      `a_client_offering_ridl_ws_v1_second_is_still_upgraded`,
      `a_client_offering_no_subprotocol_gets_400`,
      `a_request_head_over_8_kib_gets_400`, `a_binary_message_round_trips`,
      `a_message_split_across_reads_and_fragments_is_reassembled` (Review Focus
      1: the client sends a fragmented message and the parser is fed one byte at
      a time), `a_text_message_closes_with_1003`,
      `an_unmasked_client_frame_closes_with_1002` (raw bytes written to the
      socket after a real handshake),
      `a_message_over_the_limit_closes_with_1009_before_it_is_buffered` (the
      close is decided from the header's length field),
      `a_ping_is_answered_with_a_pong_carrying_its_payload`,
      `a_close_frame_is_answered_with_a_close_frame`. Fail, implement, pass.
- [ ] **Step 3: Commit**
      `feat(ridl-transport-ws): add the server side of RFC 6455`.

### Task 4: The session core — attach, routing, signals, `fixed`

Model: fable.

**Files:** Create `crates/ridl-transport-ws/src/server.rs`,
`crates/ridl-transport-ws/src/server/backend.rs`,
`crates/ridl-transport-ws/src/server/session.rs`,
`crates/ridl-transport-ws/src/server/config.rs`; unit tests in `session.rs`
(`#[cfg(test)] mod tests`) over `ridl-loopback` (add it as a dev-dependency,
path only).

**Interfaces:** Produces:

```rust
// public, in ridl_transport_ws::server (re-exported at the crate root)
pub trait Backend { /* exactly design D-10 */ }
pub struct Polling<P>(pub P);
pub struct Scanning<P>(pub P);
pub struct Config { pub poll_interval: Duration /* 20 ms */, pub ping_interval: Duration /* 5 s */,
    pub liveness_timeout: Duration /* 15 s */, pub call_abandon_after: Duration /* 60 s */, /* private: */
    max_message_bytes: usize /* 1 MiB */, record: Option<Arc<dyn Fn(&Note) + Send + Sync>> }
impl Config {
    pub fn max_message_bytes(&self) -> usize;
    pub fn with_max_message_bytes(self, n: usize) -> Result<Self, LimitTooSmall>; // n < 294 refused
    pub fn with_record(self, f: impl Fn(&Note) + Send + Sync + 'static) -> Self;
}
#[non_exhaustive] pub enum Note { Violation(ProtocolViolation), Discarded(Discarded),
    ValueTooLarge { target: Target, needed: usize }, DuplicateDropped { seq: u64 },
    UnexpectedDirection, CallAbandoned { seq: u64 } }
// crate-private
pub(crate) struct Session<B: Backend> { /* … */ }
pub(crate) enum Step { Continue, Close(u16) }
impl<B: Backend> Session<B> {
    pub(crate) fn new(backend: B, config: &Config) -> Self;
    pub(crate) fn on_message(&mut self, msg: &[u8], now: Instant, out: &mut Vec<Vec<u8>>) -> Step;
    pub(crate) fn tick(&mut self, now: Instant, out: &mut Vec<Vec<u8>>) -> Step;
}
```

Each `out` entry is one encoded frame (one WebSocket binary message). `now` is
wall time for `call_abandon_after` (Task 5); the frame `stamp` comes from the
backend's `Clock`.

- [ ] **Step 1: Failing tests** (decode each `out` entry with `frame::decode`
      and assert on the `Frame`; a test helper builds a `Session` over
      `Scanning(Loopback::new(catalog))` or `Polling(...)` and a second
      `Loopback::attach()` as the provider):
  - D-11: `the_first_message_must_be_attach` (a `subscribe` first →
    `Close(1002)`), `a_second_attach_is_a_violation`,
    `frame_version_2_is_refused`,
    `another_unit_name_is_refused_unknown_catalog`,
    `the_same_name_with_another_hash_is_refused_catalog_mismatch`,
    `another_encoding_tag_is_refused_encoding_unsupported`,
    `a_refusal_is_followed_by_close_1000`, `a_matching_attach_is_accepted`.
  - D-12/D-25:
    `a_subscribe_of_a_command_is_answered_unknown_interaction_with_its_own_identity`,
    `a_read_of_an_event_is_answered_unknown_interaction`,
    `a_request_of_a_signal_kind_is_answered_unknown_interaction_with_kind_signal`,
    `a_publish_from_the_client_closes_with_1002`.
  - D-13:
    `subscribing_a_signal_with_no_publication_answers_then_publishes_init_seq_0`,
    `subscribing_a_published_signal_publishes_live_with_its_bytes_and_stamp`,
    `a_second_subscribe_is_accepted_and_publishes_nothing_more`,
    `a_read_publishes_once_and_does_not_subscribe`,
    `an_invalidation_is_published_as_invalid_declared_with_no_payload`,
    `a_signal_larger_than_the_first_buffer_is_read_again` (Review Focus 2; a 4
    KiB value),
    `a_value_larger_than_the_message_limit_is_recorded_and_not_sent`.
  - D-14 and D-24, each run in both modes (`Scanning` and `Polling`):
    `a_commit_after_subscribe_is_published_on_the_next_tick`,
    `two_commits_between_ticks_are_coalesced_into_the_newest`,
    `an_unsubscribed_signal_is_not_published`,
    `session_seq_counts_from_1_per_channel` (backend seq 5 then 6 → session 1,
    2), `a_backend_seq_restart_still_reaches_the_client` (Review Focus 3:
    publish through one writer, drop it, publish through a new
    `Loopback::attach()` writer; the session `seq` keeps increasing),
    `a_read_of_an_unchanged_signal_reuses_its_session_seq`.
  - D-17: `a_read_of_a_fixed_publishes_kind_fixed_seq_0_live` (via
    `Loopback::provision_fixed`),
    `a_read_of_a_fixed_through_polling_is_unknown_interaction`.
- [ ] **Step 2: Run; expect FAIL.** **Step 3: Implement.** **Step 4: PASS.**
- [ ] **Step 5: Commit**
      `feat(ridl-transport-ws): serve signals and fixed values over the session core`.

### Task 5: The session core — events, calls, the end of a session

Model: fable.

**Files:** Modify `crates/ridl-transport-ws/src/server/session.rs` (split calls
into `server/calls.rs` if `session.rs` passes about 600 lines).

**Interfaces:** Consumes Task 4's `Session`, `Config`, `Note`. Adds
`pub(crate) fn Session::close(self)` — forgets every backend call in flight;
called by `serve` on any end.

- [ ] **Step 1: Failing tests:**
  - D-15: `an_event_subscribe_is_answered_accepted`,
    `an_occurrence_raised_after_subscribe_is_sent_on_the_next_tick`,
    `an_occurrence_raised_before_subscribe_is_never_sent`,
    `occurrences_keep_their_order_and_count_session_seq_from_1`,
    `unsubscribe_stops_delivery`, `a_large_occurrence_is_read_again`.
  - D-16:
    `a_command_request_is_answered_accepted_with_the_client_seq_as_correlation`,
    `a_query_request_is_answered_reply_with_the_reply_bytes`,
    `a_contract_settlement_crosses_as_contract_with_its_rule`,
    `a_corrupt_settlement_crosses_as_corrupt`,
    `a_busy_settlement_crosses_as_busy`,
    `a_full_call_table_answers_busy_at_once` (17 calls on a 16-slot loopback),
    `response_seq_counts_from_1_per_session`,
    `a_duplicate_seq_in_flight_is_not_sent_again`,
    `a_duplicate_seq_already_settled_gets_the_same_response_again`,
    `an_older_unknown_seq_is_recorded_and_dropped`,
    `a_call_unsettled_after_call_abandon_after_is_forgotten_and_answered_nothing`
    (advance `now` past the bound; the loopback slot is free again).
  - D-18: `the_end_of_a_session_forgets_its_calls_in_flight` (Review Focus 4:
    after `close`, a second caller on the same loopback can fill all 16 slots).
- [ ] **Step 2: FAIL. Step 3: Implement. Step 4: PASS.**
- [ ] **Step 5: Commit**
      `feat(ridl-transport-ws): serve events and calls over the session core`.

### Task 6: `serve`, `Connection` and liveness

Model: opus.

**Files:** Modify `crates/ridl-transport-ws/src/server.rs`,
`crates/ridl-transport-ws/src/lib.rs` (crate rustdoc: what the crate is, the
embedding example below, what it does not do — no TLS, no authentication, binds
nothing, no descriptor); test `crates/ridl-transport-ws/tests/serve.rs`.

**Interfaces:** Produces:

```rust
pub trait Connection: Read + Write { fn set_read_timeout(&self, t: Option<Duration>) -> io::Result<()>; }
// impls for std::net::TcpStream and, #[cfg(unix)], std::os::unix::net::UnixStream
#[non_exhaustive] pub enum Ended { ClientClosed(Option<u16>), ServerClosed(u16), Rejected,
    LivenessTimeout, Io(io::ErrorKind) }
pub fn serve<C: Connection, B: Backend>(conn: C, backend: B, config: &Config) -> Ended;
```

The loop is D-8; the rustdoc example (`no_run`) shows an embedder's accept loop:
bind `127.0.0.1:9000`, accept, spawn,
`serve(stream, Scanning(rt.attach()), &config)`.

- [ ] **Step 1: Failing tests** (tungstenite client, loopback backend):
      `serve_answers_attach_and_streams_a_signal_change`,
      `a_rejected_handshake_returns_rejected`,
      `a_client_close_returns_client_closed_with_its_code`,
      `the_server_pings_every_ping_interval` (short intervals in `Config`),
      `a_silent_client_ends_the_session_at_the_liveness_timeout` (Review Focus
      5: a raw TCP client that completes the handshake and then reads nothing
      and sends nothing; `serve` returns `LivenessTimeout` within
      `liveness_timeout + poll_interval + 100 ms`),
      `a_protocol_violation_closes_1002_and_records_a_note` (via `with_record`),
      `a_backend_that_detaches_closes_1001` (a test backend whose
      `EventSource::next` returns `ReadError::Detached`).
- [ ] **Step 2: FAIL. Step 3: Implement. Step 4: PASS.**
- [ ] **Step 5: Commit**
      `feat(ridl-transport-ws): serve a session over a connection`.

### Task 7: The done-when — one scenario, in process and across processes

Model: opus.

**Files:** Create `crates/ridl-transport-ws/tests/support/mod.rs`,
`tests/support/client.rs`, `tests/support/provider.rs`,
`tests/support/scenario.rs`, `tests/scenario.rs`.

**Interfaces:**

```rust
// support/provider.rs — the test catalog, unit "test.ws", hash [7; 32], interface 1:
pub const SIGNAL: Ordinal = Ordinal(1); pub const EVENT: Ordinal = Ordinal(2);
pub const COMMAND: Ordinal = Ordinal(3); pub const QUERY: Ordinal = Ordinal(4);
pub const FIXED: Ordinal = Ordinal(5); pub const FIXED_VALUE: &[u8] = &[0xf0, 0x0d];
pub fn provider_step(rt: &mut Loopback); // one pass over waiting claims:
// COMMAND: settle Ok(&[]), then set SIGNAL to the argument, commit, raise EVENT with the argument
// QUERY: settle Ok(reply) where reply[i] = arg[i].wrapping_add(1)
// support/scenario.rs
pub trait Rig { type Ports: SignalReader + EventSource + Caller + FixedReader;
    fn ports(&mut self) -> &mut Self::Ports; fn step(&mut self); }
pub fn run<R: Rig>(rig: &mut R); // design T-6 steps 1–6; waits by calling step() until a
                                 // condition holds, failing after 2 s of wall time
// support/client.rs — the test-only consuming runtime over tungstenite + frame
pub struct WsClient { /* … */ }
impl WsClient { pub fn connect(addr: SocketAddr, catalog: CatalogRef, encoding: u8) -> Self;
    pub fn pump(&mut self); } // reads every waiting message, applies §5's consumer rules
// implements Attached, SignalReader, EventSource, Caller, FixedReader
```

The client subscribes `SIGNAL` at attach (§6.2 permits it) so `read` is local;
`read_fixed` sends a `read` and pumps until the `publish` arrives (interior
mutability, because `read_fixed` takes `&self`).

- [ ] **Step 1: Write `tests/scenario.rs`** with
      `the_scenario_runs_over_the_loopback_with_no_socket` (rig: one `Loopback`;
      ports = `Loopback::attach()`; `step` = `provider_step`) and
      `the_scenario_runs_over_websocket_to_a_second_process` (rig: spawn
      `std::env::current_exe()` with args
      `["--exact", "provider_process", "--nocapture", "--ignored"]` and env
      `RIDL_TRANSPORT_WS_PROVIDER=1`; read the port from the child's first
      stdout line `port <n>`; `WsClient::connect`; `step` = `pump`; kill the
      child at the end), plus `#[test] #[ignore] fn provider_process()` which
      returns at once unless the env var is set, and otherwise builds the
      loopback, provisions `FIXED`, binds `127.0.0.1:0`, prints `port <n>`, runs
      `provider_step` on a thread every 5 ms, and `serve`s the first connection
      with `Scanning(rt.attach())`.
- [ ] **Step 2: Run; expect the loopback test to PASS and the WebSocket test to
      FAIL** until `client.rs` is written. **Step 3: Write `client.rs`.** **Step
      4: Both PASS**, and `cargo test -p ridl-transport-ws` runs
      `provider_process` as ignored (not failed).
- [ ] **Step 5: Commit**
      `test(ridl-transport-ws): run one scenario in process and across processes`.

### Task 8: Gates and records

Model: sonnet.

**Files:** Modify `justfile` (`compat-check`), `AGENTS.md`,
`docs/technotes/walking-skeleton-architecture.md`,
`docs/specification/frame-specification.md`,
`docs/decisions/ADR-0020-third-encoding-runtime-layering-and-plugin-system.md`,
`docs/ROADMAP.md`.

- [ ] **Step 1: `compat-check`.** Following how the recipe builds the packaged
      `ridl-loopback` at the `ridl-rt` rust-version, add the same for
      `ridl-transport-ws`, once with default features and once with
      `--no-default-features`. Run `just compat-check`; expect exit 0.
- [ ] **Step 2: Records** (design §10), each a minimal edit:
  - frame specification: the "As built" note and §11's "its transport crate is
    not built yet" now say the crate exists, serves the observe-and-call ports,
    applies the exact-match rule, and keeps the last 64 settled responses
    instead of retaining until the bound (D-16); D-24's reading of the relay
    rule as one sentence in §11.1 is **not** added — it is the design record's.
  - ADR-0020 decision 6: the `ridl-transport-ws` row's "Depends on" becomes
    "`ridl-rt`", with a dated "**Amended 2026-10-19**" note at the top of the
    amendments, giving D-2's reason.
  - ROADMAP: an "**E11.9 landed**" paragraph is written at merge time, not now;
    add nothing to the ROADMAP in this task.
  - `AGENTS.md`: crate count twenty-one and the crate in the list;
    walking-skeleton technote: the crate's row.
- [ ] **Step 3: Run** `just link-check`, `just doc-path-check`,
      `just story-id-check`, `just check`; expect exit 0.
- [ ] **Step 4: Commit** `docs(ridl-transport-ws): record the transport crate`
      (scope may be `docs`; `git std lint` decides).
