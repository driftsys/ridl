# Streams stage 5a Implementation Plan

> **For agentic workers:** This plan is built by Codex, one task at a time, and
> each task is reviewed by Claude before the next one starts. Read the section
> "How to work" before Task 1. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give the generated Rust face every stream form that stage 3 left out —
a reply stream of bytes or of text, a parameter stream of named elements, of
bytes or of text, a query with a parameter stream and a plain reply, and a query
with a parameter stream and a reply stream — over the stream port of `ridl-rt`,
proved over `ridl-loopback` and by a new worked example `examples/docstore/`.

**Architecture:** `ridl-rt` gains `face::ByteSource`, `face::Consumer`,
`face::ByteConsumer` and `payload::Rule::Utf8` (additive). The Rust backend's
face stops leaving out the forms above: it emits a consumer value per form
(`<Name>Stream` for a reply stream, `<Name>Sink` for a parameter stream) and a
`Provider` associated type per streamed member, and `serve` drives the
provider's values over `StreamHandler`. The WebSocket upload direction and the
TypeScript client are stage 5b, not this plan.

**Tech Stack:** Rust (`ridl-rt`: edition 2021, `rust-version = "1.88"`,
`no_std`), the Rust backend's emitters in `crates/ridl-backend-rust/src/face.rs`
and `src/face/`, `ridl-loopback`, `cargo xtask freeze`.

**Spec:** `docs/wip/2026-10-31-streams-design.md` — section 3 (the port), 7.1 to
7.3 (the generated Rust surface), R-10, R-15, R-20, R-21, R-25, R-33, the stage
5 entry of section 9, and the sections "As built" and "As built (stage 4)",
which take precedence over the earlier sections where they differ. R-n are that
document's rulings; S5-n are this plan's.

## How to work

- Work in the worktree `../ridl-streams-5a` (branch `feat/streams-stage-5a`). Do
  not push. Do not open a pull request.
- Do **one task**, then stop. Before stopping, write
  `.superpowers/codex/task-<N>-report.md` (the directory is gitignored): the
  commits of the task (`git log --oneline <base>..HEAD`, where `<base>` is the
  head before the task started), the tests you added with one line each on what
  they pin, the commands you ran and their result, and every choice you made
  that this plan did not decide, each as a line starting with `Ruling:`. Claude
  reviews the task from its diff; the next task starts after that review.
- Test first: write each test, run it, and see it fail on an assertion (not on a
  compile error) before you write the code that makes it pass.
- Run only the focused tests of the crates you touch
  (`cargo test -p <crate> --locked`, plus
  `cargo clippy -p <crate> --all-targets
  -- -D warnings`). CI runs the full
  gate.
- Commits follow Conventional Commits, linted by `.git-std.toml`. Scopes used
  here: `ridl-rt`, `ridl-backend-rust`, `ridlc`, `examples`, `xtask`, `docs`.
  The `git commit` hook runs `prim` and the commit lint; fix what it reports and
  commit again, never `--no-verify`.
- Read `AGENTS.md` once. It binds every task, in particular: prose is plain and
  literal, shipped docs and rustdoc name no story id, stage or epic (write what
  the code does), and `just story-id-check` enforces that.

## Rulings taken in this plan

- **S5-1. Split (owner, 2026-11-02).** Stage 5 is two pull requests. 5a is this
  plan. 5b is the upload direction of `ridl-transport-ws` (the server receives
  `chunk`, grants credit, `ChunkTooLarge`, `rule_tag` code 7 for `Utf8` in frame
  version 3, which is unreleased) and `commandStream` / `queryWithUpload` in
  `packages/ridl-transport-ws`, with `just ts-e2e` uploading 200 KB.
  `ridl-transport-ws` is not touched in 5a: its `rule_tag` already answers
  `None` for a rule it does not know, so the new variant cannot reach a frame.
- **S5-2. Stage 3's scope cut stays (S3-1).** No `blocking` stream method and no
  `futures-io` / `futures-core` feature. Each consumer value has the `no_std`
  `poll_*` methods and `async fn` wrappers emitted outside
  `cfg(feature = "std")` over `core::future::poll_fn`, as `<Name>Stream::next`
  is today.
- **S5-3. The face carries a call with at most one stream parameter.** A call
  with two or more stream parameters stays left out of the face, as every stream
  member was before stage 3; its descriptors are still emitted. Reason: no
  source in the repository declares one, and each extra parameter multiplies the
  sink values of the call. Claude files the tracking issue at PR time; the
  rustdoc that states the omission links it.
- **S5-4. A text reply stream yields one `&str` per chunk.** `<Name>Stream` of
  `query f(..): <string>` has
  `poll_next(&mut self, cx) -> Poll<Option<Result<&str, ClientError>>>` and
  `async fn next(&mut self) -> Option<Result<&str, ClientError>>`, the `&str`
  borrowed from the stream's staging buffer, in place of the design table's
  `poll_read`. Reason: a chunk holds whole code points (ADR-0032 decision 3),
  but a `poll_read` into a short `buf` would split one, and then the bytes the
  application holds are not text.
- **S5-5. A text sink takes `&str`.** `<Name>Sink` of a `<string>` parameter has
  `poll_write_str(&mut self, cx, s: &str) -> Poll<Result<usize, ClientError>>`,
  which sends the longest prefix of `s` that is at most the member's limit and
  ends on a char boundary, and answers its length in bytes. The sending side's
  UTF-8 check of R-15 is the type. On the provider side the text source is a
  `ByteSource` (the design's table), and `serve` checks it (Task 2).
- **S5-6. A sink of named elements holds at most one encoded element.**
  `send(&mut self, item: &T) -> Result<(), ClientError>` encodes into the sink's
  buffer, verifies the value (the sending side's check of ridl §10.2), and
  pushes; when the push answers 0 the element stays held and `send` still
  answers `Ok(())`. `poll_ready(&mut self, cx) -> Poll<Result<(), ClientError>>`
  pushes a held element and is `Ready(Ok(()))` once nothing is held.
  `poll_end(&mut self, cx) -> Poll<Result<(), ClientError>>` pushes a held
  element, then calls `end`. A `send` while an element is held answers
  `Err(ClientError::Call(CallError::Transport(Transport::Busy)))` and drops
  nothing. A value that fails its check answers
  `Err(ClientError::Call(CallError::Contract(Contract::InvalidValue(v))))`, and
  the sink aborts the stream with that error. Async wrappers: `ready`, `feed`
  (ready, then send) and `end`.
- **S5-7. A byte sink pushes from the caller's buffer.**
  `poll_write(&mut self, cx, buf: &[u8]) -> Poll<Result<usize, ClientError>>`
  pushes `buf[..min(buf.len(), LIMIT)]` as one chunk and answers its length;
  when the push answers 0 it registers `Interest::Stream` and answers `Pending`
  with nothing taken. No staging buffer on this side. `poll_end` as in S5-6.
  Async wrappers: `write` and `end`.
- **S5-8. A query with a parameter stream and a plain reply has two provider
  methods.** `fn <name>(&mut self, <args>) -> Self::<Name>`, where
  `type <Name>: Consumer<Item = T>` (or `ByteConsumer`), and
  `fn <name>_reply(&mut self, upload: Self::<Name>) -> <R>`, where `<R>` is
  exactly what the provider method of a plain query with the same reply returns
  today. `serve` calls `<name>_reply` after the stream's end has been fed to the
  consumer, then settles the claim. Reason: `Consumer` carries no output, and a
  second method keeps every `Provider` method synchronous (R-20). The caller's
  `<Name>Call` starts the reply bound when `poll_end` sends the end, not at the
  send of the request.
- **S5-9. A query with a parameter stream and a reply stream has one provider
  value that is both.** `type <Name>: Consumer<Item = T> + Source<Item = U>`.
  `serve` feeds the consumer and drains the source in the same poll, so the
  reply can start before the upload ends.
- **S5-10. `docstore`'s `describe` replies with a named result union.** The face
  refuses an inline `T | E` reply, so the example declares
  `union DescribeResult { info: DocInfo, failure: DocError }` and
  `query describe(id: DocId): DescribeResult @[..1s]`.
- **S5-11. `serve` runs one call per streamed member at a time.** The rule stage
  3 built for a streamed reply extends to every member with a stream: a second
  concurrent call of the member settles `Transport::Busy`.

## Global Constraints

- `ridl-rt` stays sans-IO and `no_std`; the new `face` traits use `core::task`
  only. Every `ridl-rt` change is additive: the `runtime-crates` dump gains
  lines only.
- The IR, the catalog descriptor and every catalog hash are unchanged. No
  diagnostic is added or changed (ADR-0032 decision 14).
- A generated crate allocates nothing for a stream. The staging buffer of R-21
  is an inline `[u8; LIMIT]` field, `LIMIT` the member's limit constant the face
  already emits.
- The `StreamCaller` / `StreamHandler` bounds appear only on an interface that
  declares a member the face carries with a stream (R-32).
- A chunk is at most the member's limit in both directions (design, "As built").
  A text chunk holds whole code points and is valid UTF-8.
- A UTF-8 violation is
  `CallError::Contract(Contract::InvalidValue(Violation {
  type_name: Some("string"), rule: Rule::Utf8 }))`.
- R-33: a provider's refusal of a parameter stream after a command's
  acknowledgment reaches the producer through its next sink call and leaves the
  `<Name>Call` result as it was.
- Dropping any consumer value (`<Name>Stream` or `<Name>Sink`) of a call that
  has not ended forgets the call exactly once, which interrupts (R-5). When a
  call has both a sink and a stream, the call is forgotten when the last of the
  two is dropped.
- A fixture member that names a type (`Id`, `Info`, `Summary`) declares it in
  `stream_face.ridl` beside the existing `Reading`, with a constraint where a
  test needs one to fail.
- When a task changes generated output that a dump records, run
  `cargo xtask freeze update` and commit the refreshed dumps in the same task;
  `cargo xtask freeze check` passes at the end of each task, and every dump
  change adds lines only.

## Review Focus

- A text source whose code points straddle the limit: `serve` must carry the 1
  to 3 trailing bytes of an incomplete code point into the next chunk, never
  send them, and refuse with `Utf8` when the source ends with bytes still
  carried. Pinned in Task 2.
- A `ByteConsumer` that accepts part of a chunk: `serve` must keep the rest and
  offer it again before pulling the next chunk. Pinned in Task 3.
- A sink dropped while an element is held, and a sink dropped after `poll_end`:
  the first interrupts, the second forgets nothing more than the end already
  did. Pinned in Task 3.
- The reply bound of a query with an upload: it starts at the end, not at the
  request. A hand-driven clock advanced past the bound before the end must not
  time out. Pinned in Task 4.
- The generated crates of `crates/ridlc/tests/corpus/rust-shapes` (its
  `command replay(samples: <Inner>)` gains a face in Task 3) are linted by
  `just demo` under `-D warnings`, `dead_code` included: an emitted helper that
  no member uses fails CI. Each task that adds an emitted item checks that the
  corpus crate still builds without warnings
  (`cargo test -p ridl-backend-rust --test face_compile --locked`).

---

### Task 1: The `ridl-rt` face traits and `Rule::Utf8`

**Files:**

- Modify: `crates/ridl-rt/src/face.rs` (beside `Source` and `Interrupted`)
- Modify: `crates/ridl-rt/src/payload.rs` (`Rule`)
- Modify: `freeze/runtime-crates*` dumps through `cargo xtask freeze update`
- Modify: `docs/design/ridl-rt.md` (the `face` module and `Rule`)

**Interfaces:**

- Produces, in `ridl_rt::face`, exactly the signatures of design section 7.1:

  ```rust
  pub trait ByteSource {
      fn poll_read(&mut self, cx: &mut Context<'_>, buf: &mut [u8]) -> Poll<Result<usize, Interrupted>>;
  }
  pub trait Consumer {
      type Item;
      fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<()>;
      fn accept(&mut self, item: Self::Item);
      fn end(&mut self);
      fn failed(&mut self, e: CallError);
  }
  pub trait ByteConsumer {
      fn poll_write(&mut self, cx: &mut Context<'_>, buf: &[u8]) -> Poll<usize>;
      fn end(&mut self);
      fn failed(&mut self, e: CallError);
  }
  ```

  `ByteSource::poll_read` answers `Ok(0)` at the end. `ByteConsumer::poll_write`
  answers how many bytes of `buf` it took, at least 1 when `buf` is not empty.
- Produces `Rule::Utf8`, after `Unique`, documented as "a text chunk of a
  `<string>` stream is not valid UTF-8 (ADR-0032 decision 12)".

- [ ] **Step 1: Write the failing tests** in the `tests` module of `face.rs`, in
      the style of the two existing `Source` tests (a slice-backed double,
      driven through `&mut dyn Trait` with `Waker::noop()`):
      `a_byte_source_is_read_until_ok_zero` (a source over `b"abcdef"` read with
      a 4-byte buffer gives `4`, `2`, `0`, and the bytes in order);
      `a_consumer_is_fed_until_its_end` (3 items accepted in order, then `end`,
      recorded by the double);
      `a_byte_consumer_that_takes_part_of_a_buffer_is_offered_the_rest` (a
      consumer that takes at most 3 bytes per call is driven by a generic helper
      that loops until the whole buffer is taken; it receives the bytes in
      order).
- [ ] **Step 2: Run them and see them fail**
      (`cargo test -p ridl-rt --locked face::tests`).
- [ ] **Step 3: Add the three traits and `Rule::Utf8`** with rustdoc that states
      each method's contract (what `Pending` registers, what the end is, who
      calls `failed`), and a doc example for `ByteSource` like `Source`'s.
- [ ] **Step 4: Run** `cargo test -p ridl-rt --locked`,
      `cargo clippy -p ridl-rt --all-targets -- -D warnings`, and
      `cargo check -p ridl-rt --no-default-features --target thumbv7em-none-eabihf`.
      Expected: pass. Then `cargo test --workspace --locked --no-run` to find
      every exhaustive `match` on `Rule` outside `ridl-rt` that no longer
      compiles; fix each with the arm its neighbours imply and list each one as
      a `Ruling:`.
- [ ] **Step 5: Refresh the freeze dump** with `cargo xtask freeze update`;
      `git diff freeze/` shows added lines only; `cargo xtask freeze check`
      passes.
- [ ] **Step 6: Update `docs/design/ridl-rt.md`** where it lists the `face`
      items and the `Rule` variants.
- [ ] **Step 7: Commit**
      `feat(ridl-rt): add the byte source, consumer and byte consumer traits and Rule::Utf8`.

### Task 2: Reply streams of bytes and of text

**Files:**

- Modify: `crates/ridl-backend-rust/src/descriptors.rs` (`named_stream_reply`
  and its callers: the face's member filter accepts a reply stream of form
  `Bytes` or `Text` whose only stream is the reply)
- Modify: `crates/ridl-backend-rust/src/face.rs`,
  `crates/ridl-backend-rust/src/face/futures.rs` (consumer value),
  `crates/ridl-backend-rust/src/face/serve.rs` (provider side)
- Modify: `crates/ridl-backend-rust/tests/fixtures/stream_face.ridl` (add, to a
  new interface `Docs`, `query fetch(id: Id): <bytes> @[..1s]` and
  `query text(id: Id): <string> @[..1s]`, with a limit small enough that the
  tests cross several chunks — set it with the attribute the fixture's other
  members use, or the smallest limit the sizer accepts)
- Modify: `crates/ridl-backend-rust/tests/generated/stream_face.rs` (regenerate
  through the existing regeneration test's update path)
- Test: `crates/ridl-backend-rust/tests/stream_face.rs`,
  `crates/ridl-backend-rust/tests/face_generation.rs`

**Interfaces:**

- Consumes: Task 1's `ByteSource`, `Rule::Utf8`.
- Produces, for `query fetch(..): <bytes>`:
  `Client::fetch(&mut self, ..) ->
  FetchCall<'_, P>` resolving to
  `FetchStream<'_, P>`, with
  `poll_read(&mut self, cx, buf: &mut [u8]) -> Poll<Result<usize, ClientError>>`
  (`Ok(0)` is the end) and
  `async fn read(&mut self, buf: &mut [u8]) ->
  Result<usize, ClientError>`; a
  staging field `[u8; LIMIT]` serves a short `buf` (R-21). `Provider`:
  `type Fetch: ridl_rt::face::ByteSource;` and
  `fn fetch(&mut self, <args>) -> Self::Fetch;`.
- Produces, for `query text(..): <string>`: `TextStream::poll_next` and `next`
  as S5-4; `Provider`: `type Text: ridl_rt::face::ByteSource;`.
- The call future of both resolves as stage 3's does: `Ok(stream)` on the first
  chunk or the end, the first chunk kept in the staging buffer.

**`serve` for a byte or text reply (R-25):** on each poll with window left, read
from the source into the staging buffer after any carried bytes, up to `LIMIT`.
Push the staged bytes as one chunk when the buffer is full, or when the source
answers `Pending` with at least one byte staged. For text, push only the prefix
that ends on a char boundary (`core::str::from_utf8` and
`Utf8Error::valid_up_to` / `error_len`): an `error_len()` of `Some(_)` is
invalid input, refused with the `Utf8` violation by
`abort_claim(.., REPLY_STREAM, e)`; a `None` means an incomplete code point at
the end, whose 1 to 3 bytes are moved to the front and carried. `Ok(0)` with
nothing carried `end_claim`s; `Ok(0)` with bytes carried is refused with the
`Utf8` violation. `Err(Interrupted)` aborts with `Transport::Interrupted`. A
push that answers 0 keeps the staged chunk for the next poll.

**Consumer side for text:** each chunk is checked with `core::str::from_utf8`
before the application sees it; a chunk that fails is refused with the `Utf8`
violation, the call is aborted with it, and `poll_next` yields that error once,
then `None`.

- [ ] **Step 1: Write the failing generation test** in `face_generation.rs`: the
      `Docs` face emits `FetchStream` with `poll_read` and `read`, `TextStream`
      with `poll_next` returning `&str`, and the two `Provider` associated types
      bounded by `ByteSource`.
- [ ] **Step 2: Write the failing loopback tests** in `stream_face.rs`, in the
      style of the existing ones (provider driven by hand through
      `HandlerHandle`'s `StreamHandler` port for the caller-side tests, and the
      generated `serve` over a scripted `Provider` for the provider-side ones):
  - `a_byte_reply_read_with_a_short_buffer_is_byte_equal_to_what_was_pushed`
    (three pushed chunks of `LIMIT`, `LIMIT` and 5 bytes, read with a 7-byte
    `buf`, concatenate to the input; then `Ok(0)`);
  - `a_text_reply_whose_code_points_straddle_every_boundary_arrives_unsplit`
    (`serve` over a source that yields a text of 3-byte and 4-byte code points
    in reads of 1 byte, then of `LIMIT`; each item of `poll_next` is valid
    `&str`, each at most `LIMIT` bytes, and their concatenation equals the
    text);
  - `a_text_source_that_ends_inside_a_code_point_is_refused_with_utf8` (the
    caller's stream yields the `Utf8` violation after the whole chunks before
    it);
  - `a_text_source_with_invalid_bytes_is_refused_before_they_are_pushed` (`0xff`
    in the middle: the bytes before it arrive, the `0xff` never crosses — run
    the generated `serve` over `Loopback` and pull every chunk of the reply
    stream through the caller's `StreamCaller` port, below the face: no chunk
    holds `0xff`, the chunks before the refusal concatenate to the bytes before
    `0xff`, and the stream ends `Pull::Failed` with the `Utf8` violation.
    `Loopback` delivers every chunk the provider pushes, so this proves that no
    push carried the byte. `RecordingPorts` is caller-side only and is not used
    here);
  - `an_invalid_text_chunk_pushed_by_hand_is_refused_before_the_application` (a
    chunk `b"a\xffb"` pushed through `StreamHandler`: `poll_next` yields the
    `Utf8` violation, never `"a"`, and the provider's next push answers
    `PushError::Failed(Contract(InvalidValue(_)))`);
  - `a_byte_source_that_is_pending_sends_its_partial_chunk` (source yields 10
    bytes then `Pending`: one chunk of 10 bytes is pushed).
- [ ] **Step 3: Run them and see them fail.** The loopback tests cannot compile
      until the face emits the new items, so for them the red step is the
      generation test of Step 1 failing on its assertion; once the items exist,
      each loopback test must have been seen to fail by breaking the code path
      it pins (for example skipping the carry) and then restored. Name the path
      you broke for each test in the task report.
- [ ] **Step 4: Implement** the member filter, the two consumer values, the two
      `Provider` items and the `serve` arm. Regenerate
      `tests/generated/stream_face.rs`.
- [ ] **Step 5: Run** `cargo test -p ridl-backend-rust --locked` and its clippy.
      Expected: pass. Then `cargo xtask freeze check`; if a dump changed,
      `cargo xtask freeze update`, added lines only.
- [ ] **Step 6: Commit**
      `feat(ridl-backend-rust): emit the face for a reply stream of bytes or of text`.

### Task 3: Parameter streams on a command

**Files:**

- Modify: `crates/ridl-backend-rust/src/descriptors.rs` (accept a command with
  exactly one stream parameter, of any form; S5-3)
- Modify: `crates/ridl-backend-rust/src/face.rs`, `src/face/futures.rs`,
  `src/face/serve.rs`, and `src/face/dispatch.rs` if the claim's argument
  decoding lives there
- Modify: the fixture `stream_face.ridl` (add to `Docs`:
  `command put(info: Info, data: <bytes>) @[..1s]`,
  `command note(data: <string>) @[..1s]`, and to `Log`:
  `command upload(data: <Reading>) @[..1s]`), and the regenerated face
- Test: `stream_face.rs`, `face_generation.rs`

**Interfaces:**

- Consumes: Task 1's `Consumer`, `ByteConsumer`; Task 2's staging and UTF-8
  helpers in the emitter, if Task 2 made them shared.
- Produces:
  `Client::put(&mut self, info: &Info) -> (PutCall<'_, P>, PutSink<'_, P>)`. The
  two values borrow the client through one shared handle; choose the
  representation (for example both holding `&RefCell` or a split borrow) and
  record it as a `Ruling:`. `PutCall` resolves `Ok(())` at the acknowledgment.
  `PutSink` has S5-7's methods; `NoteSink` S5-5's; `UploadSink` S5-6's.
- `Provider`: `type Put: ridl_rt::face::ByteConsumer;`
  `fn put(&mut self, info: &Info) -> Self::Put;` — and `Note` likewise,
  `type Upload: ridl_rt::face::Consumer<Item = Reading>;`.

**`serve` for a command with a parameter stream:** take the claim, decode and
verify the arguments and `require` as today, call the provider method, settle
the claim `Ok(&[])` (the acknowledgment, design section 3.2), then on each poll:
call the consumer's `poll_ready` (or offer the rest of a partly taken chunk to
`poll_write`), and only when it is ready `pull_claim(claim, StreamNo(1), buf)`.
A chunk of named elements is verified (structure, then constraints) before
`accept`; text is checked with `from_utf8` before `poll_write`. A failure calls
`abort_claim(claim, StreamNo(1), e)` with `Contract(InvalidValue(_))` or
`Transport(Corrupt)` and `failed(e)` on the consumer; the claim is not settled
again (R-33 step 1). `Pull::End` calls `end()`; `Pull::Failed(e)` calls
`failed(e)`.

- [ ] **Step 1: Write the failing generation test**: `Client::put` returns the
      pair; the three `Provider` associated types and their bounds.
- [ ] **Step 2: Write the failing loopback tests:**
  - `a_200_kb_put_reaches_the_byte_consumer_in_chunks_of_at_most_the_limit` (the
    client writes 204,800 bytes of a known pattern through `PutSink::write`
    while the test polls `serve`; the provider's `ByteConsumer` records every
    `poll_write` length, each ≤ `LIMIT`, and the recorded bytes equal the input;
    `end` is called once);
  - `a_byte_consumer_that_takes_part_of_a_chunk_is_offered_the_rest_first`;
  - `the_providers_refusal_of_the_third_element_reaches_the_next_send` (R-33:
    the provider's check refuses the third `Reading` — give the fixture a range
    the third value breaks; `PutCall`/`UploadCall` resolved `Ok(())` stays
    `Ok(())`; the sink's next `send` or `poll_ready` answers
    `Err(ClientError::Call(CallError::Contract(Contract::InvalidValue(_))))`;
    the consumer's `failed` was called once);
  - `a_send_that_fails_its_check_aborts_the_stream_and_sends_nothing` (S5-6:
    verify on the sending side);
  - `a_send_while_an_element_is_held_is_busy_and_drops_nothing` (S5-6);
  - `a_text_sink_never_cuts_a_code_point` (S5-5: a `&str` of 4-byte code points
    longer than `LIMIT`; each answered length ends on a char boundary);
  - `a_sink_dropped_before_its_end_interrupts_the_consumer` and
    `a_sink_dropped_after_its_end_forgets_nothing_more`;
  - `a_second_put_while_one_runs_is_busy` (S5-11).
- [ ] **Step 3: Run them and see them fail,** as Task 2 Step 3 describes.
- [ ] **Step 4: Implement.** Regenerate the checked-in face.
- [ ] **Step 5: Run** `cargo test -p ridl-backend-rust --locked` and its clippy,
      `cargo test -p ridlc --locked`, `cargo xtask freeze check` (update and
      commit dumps if needed, added lines only).
- [ ] **Step 6: Commit**
      `feat(ridl-backend-rust): emit the face for a parameter stream on a command`.

### Task 4: Queries with a parameter stream

**Files:**

- Modify: `descriptors.rs` (accept a query with one stream parameter and either
  a plain reply or a reply stream of named elements; S5-3), `face.rs`,
  `src/face/futures.rs`, `src/face/serve.rs`
- Modify: the fixture (add to `Log`:
  `query summarize(data: <Reading>): Summary @[..100ms]` and
  `query pipe(data: <Reading>): <Reading> @[..1s]`), and the regenerated face
- Test: `stream_face.rs`, `face_generation.rs`

**Interfaces:**

- Consumes: Task 3's sink values and `serve` feeding; stage 3's reply stream.
- Produces:
  `Client::summarize(&mut self) -> (SummarizeCall<'_, P>,
  SummarizeSink<'_, P>)`,
  `SummarizeCall` resolving to the reply as a plain query's call does;
  `Provider`: `type Summarize: Consumer<Item = Reading>;`,
  `fn summarize(&mut self) -> Self::Summarize;`,
  `fn summarize_reply(&mut self, upload: Self::Summarize) -> <R>` (S5-8).
- Produces: `Client::pipe(&mut self) -> (PipeCall<'_, P>, PipeSink<'_, P>)`,
  `PipeCall` resolving to `PipeStream<'_, P>` as stage 3's call does;
  `Provider`: `type Pipe: Consumer<Item = Reading> + Source<Item = Reading>;`
  (S5-9).

- [ ] **Step 1: Write the failing generation test** for both members.
- [ ] **Step 2: Write the failing loopback tests:**
  - `a_query_with_an_upload_replies_after_the_end` (three elements, `end`, then
    the reply computed by `summarize_reply` from them);
  - `the_reply_bound_starts_at_the_end_of_the_upload` (hand-driven clock:
    advance past the member's `max` before `poll_end` — no timeout; then
    `poll_end`, advance past `max` again — `Err(Timeout)`);
  - `a_refusal_of_the_upload_settles_the_query_with_the_error` (design R-33 step
    1 for a query: `SummarizeCall` resolves
    `Err(ClientError::Call(Contract(InvalidValue(_))))`);
  - `a_pipe_reply_starts_before_the_upload_ends` (the provider's source yields
    an element after the first accepted one; the caller reads it from
    `PipeStream` before calling `poll_end`);
  - `a_pipe_whose_sink_and_stream_are_dropped_forgets_once`.
- [ ] **Step 3: Run them and see them fail,** as Task 2 Step 3 describes.
- [ ] **Step 4: Implement.** Regenerate the checked-in face.
- [ ] **Step 5: Run** `cargo test -p ridl-backend-rust --locked` and its clippy,
      `cargo test -p ridlc --locked`, `cargo xtask freeze check`.
- [ ] **Step 6: Commit**
      `feat(ridl-backend-rust): emit the face for a query with a parameter stream`.

### Task 5: The `docstore` example and the records

**Files:**

- Create: `examples/docstore/ridl.toml`, `examples/docstore/docstore.ridl`,
  `examples/docstore/consumer/src/main.rs` (and a `Cargo.toml` only if
  `examples/cabin/consumer` has one for `just demo`; this example is not added
  to `just demo`)
- Create: `crates/ridlc/tests/docstore_example.rs`, modelled on
  `crates/ridlc/tests/cabin_example.rs` (`ridl build --emit rust`, the emitted
  crate compiled with `rustc`, the consumer compiled against it and run)
- Modify: `docs/design/interaction-face.md` (every row of design table 7.3 is
  now carried; S5-3's omission with its issue link; S5-4 to S5-9's shapes)
- Modify: `docs/wip/2026-10-31-streams-design.md`: add a section
  `## As built (stage 5a)` after "As built (stage 4)", one line per S5-n ruling
  and per `Ruling:` line of the task reports that Claude accepted (Claude gives
  you that list when Task 5 starts)
- Modify: `examples/README.md` or the file that lists the examples, if one
  exists

**`docstore.ridl`:** one package with `DocId` (a bounded string), `DocInfo`
(name: bounded string, size: `u64`), `DocError` (an enum), the result union of
S5-10, and

```ridl,ignore
interface DocStore {
  query fetch(id: DocId): <bytes> @[..1s]
  command put(info: DocInfo, data: <bytes>) @[..1s]
  query describe(id: DocId): DescribeResult @[..1s]
}
```

`ridl check examples/docstore` must report no diagnostic.

**The consumer program** holds one document in memory behind a `Provider` over
`Loopback`, puts 204,800 bytes of a known pattern, describes it (`size` is
204,800), fetches it back with a 1,000-byte read buffer, and exits non-zero
unless the fetched bytes equal the put bytes. It prints one line per step; the
test requires exit 0 and the line `docstore round trip: 204800 bytes`.

- [ ] **Step 1: Write `crates/ridlc/tests/docstore_example.rs`** with the test
      `the_emitted_docstore_crate_round_trips_200_kb`, and see it fail (the
      example does not exist yet).
- [ ] **Step 2: Write the example and the consumer.** Run
      `cargo run -p ridl-cli -- check examples/docstore` (no diagnostic), then
      the test. Expected: pass.
- [ ] **Step 3: Update the records** listed above. Run `just link-check`,
      `just doc-path-check`, `just story-id-check` and `just book-check`.
      Expected: pass.
- [ ] **Step 4: Run** `cargo test -p ridlc --locked` and
      `cargo xtask freeze check`.
- [ ] **Step 5: Commit**
      `feat(examples): add the docstore example of byte streams` and
      `docs(docs): record the stream forms of the Rust face` (two commits).
