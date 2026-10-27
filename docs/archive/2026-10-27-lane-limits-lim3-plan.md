# Payload limits in the transport (LIM3, #1059 and #1067) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** a consumer records the same limits as the owner of a remote unit, the
catalog descriptor carries each member's limit, and `ridl-transport-ws` limits a
message to 16,292 bytes, answers a request above its member's limit `corrupt`,
and stops raising the retained-bytes cap to the message limit.

**Architecture:** `ridl-core` stores the `[limits]` table of each materialized
remote unit in `LoadedWorkspace.unit_limits`, so the LIM2 pass `annotate_limits`
records the owner's limits on the remote unit's interactions and the two catalog
hashes agree; the RIDL-151 and RIDL-152 checks already skip remote packages
(Task 1). The descriptor's `Member` table gains an optional `limit` field,
appended last, filled from `Decl.limit` (Task 2). The transport's `MemberTable`
gains a limit per entry, read by `from_descriptor` (Task 3), which the session's
`request` handler compares with the length of the arguments (Task 4). `Config`
gains the upper bound and the fixed retained minimum (Task 5). Task 6 approves
the API break in `freeze/surfaces.toml`.

**Tech Stack:** Rust; planus 1.3.0 for the descriptor accessors
(`cargo xtask descriptor-codegen`); insta snapshots in `crates/ridl/tests`.

**Spec:** `docs/decisions/ADR-0030-transfer-and-bound-limits.md` decisions 10,
13 and 15; driftsys/ridl#1059 (body and the owner comment of 2026-10-24);
driftsys/ridl#1067; `docs/wip/2026-10-24-lane-limits-driver.md` rulings R-16,
R-21, R-24 and R-25.

## Global Constraints

- One pull request. Its body has `Closes #1059`, `Closes #1067`, and a
  `BREAKING CHANGE:` footer that names the four breaks the owner accepted on
  #1059 plus the new field of the descriptor's generated `Member` struct.
- 16,292 = 36 + 16,256: the largest data-plane header with its envelope (frame
  specification §11.1 "The sizes") plus the payload ceiling (ADR-0030 decision
  2). The 294-byte floor (`MIN_MESSAGE_LIMIT`) stays.
- The retained-bytes cap's smallest valid value is 16,292 whatever the message
  limit. Its default stays 16 MiB.
- A `request` is checked against its member's limit only when the backend has a
  member table and that member has a limit. A publish is never checked. An unset
  limit means "no `[limits]` entry applies"; the message limit alone then bounds
  the arguments (a `request` within 16,292 bytes carries at most 16,292 − 42 =
  16,250 bytes of arguments, below the ceiling, so no ceiling check is needed).
- The descriptor field is appended at the end of `Member` and is an optional
  scalar (`= null`), so a descriptor written before it reads as absent.
  `SCHEMA_VERSION` stays 1, as it did when `compatible` was appended.
- A remote unit's limits are recorded, not checked by the consumer (R-24): no
  RIDL-151 and no RIDL-152 for a remote unit.
- The TypeScript client `packages/ridl-transport-ws` changes nothing: it sets no
  message size limit (only the constant `MIN_MESSAGE_LIMIT` in `src/frame.ts`),
  holds no member table, and already decodes a `corrupt` response. ADR-0030's
  Status states it does not change here.
- `ridl-descriptor` must still pass
  `cargo test -p ridl-descriptor
  --no-default-features` and
  `--no-default-features --features dedup`, and
  `cargo check --target thumbv7em-none-eabihf -p ridl-descriptor
  --no-default-features`
  (the `just test` and `just wasm-check` lines).
- Implementers run the focused crate tests named in each task; CI runs the full
  gates. Plain, literal English. No story ids or plan names in code or in
  shipped docs (`just story-id-check`).

## Review Focus

1. A remote unit whose `[limits]` table names no interaction, or gives a payload
   a limit it exceeds, draws no RIDL-151 or RIDL-152 in the consumer, and
   `ridl check` stays silent. Test in Task 1.
2. Arguments exactly at the member's limit reach the backend; one byte more is
   answered `corrupt` with no backend call; a limit of 0 (R-3) lets an empty
   argument through. Tests in Task 4.
3. A repeat of a request that was answered `corrupt` for its length gets the
   same `response` again from the kept responses, and the backend still sees no
   call. Test in Task 4.
4. A request naming a member the table does not hold, or holds under another
   kind, is answered `contract(UnknownInteraction)` whatever its length, not
   `corrupt`. Test in Task 4.
5. An embedder that set a message limit above 16,292 or a retained cap below it
   gets an `Err` at configuration time whose text names the allowed bound, not a
   session that fails later. Tests in Task 5.

---

### Task 1: Load the `[limits]` table of a remote unit (#1067)

**Files:**

- Modify: `crates/ridl-core/src/workspace.rs` — `read_remote_unit` (around line
  1530): after the MANI-014 check and before `self.remote_units.insert(..)`,
  insert `UnitLimits { manifest: file_id,
  limits: manifest.limits }` into
  `self.unit_limits` under `name` when the table is not empty. Update the doc
  comment of `LoadedWorkspace.unit_limits` (line 89): the map also holds each
  loaded remote unit's table.
- Modify: `docs/book/cli-reference.md` (the `[limits]` paragraphs near line
  945): one sentence — the table of a remote unit that `[imports]` names is read
  too, so its interactions record the owner's limits and the consumer computes
  the owner's catalog hash; the consumer reports no RIDL-151 or RIDL-152 for it.
- Test: `crates/ridl-core/src/workspace.rs` `mod tests::remote`;
  `crates/ridl/tests/remote_units.rs`.

**Interfaces:**

- Produces: `LoadedWorkspace.unit_limits` has an entry for a remote unit that
  has a non-empty `[limits]` table; `manifest` is the remote unit's `ridl.toml`.

- [ ] **Step 1: Write the failing tests**
  - `workspace.rs` `mod remote`: `a_remote_units_limits_table_is_loaded` — seed
    the cache with `COMMON_MANIFEST` plus `"[limits]\ndefault = 64\n"`, load
    frozen, then
    `assert_eq!(loaded.unit_limits.get("veh.common").and_then(|u| u.limits.default.as_ref().map(|d| *d.as_ref())), Some(64))`
    and assert that `loaded.sources.path(entry.manifest)` is
    `Some(path_string(&loaded.remote_units["veh.common"].dir.join("ridl.toml")))`.
    `a_remote_unit_with_no_limits_table_has_no_entry` — the existing
    `COMMON_MANIFEST`; `assert!(!loaded.unit_limits.contains_key("veh.common"))`
    (passes before and after; it guards the "no table, no entry" rule).
  - `remote_units.rs`: `a_remote_units_limits_reach_its_catalog_hash` — the unit
    of
    `build_generates_the_remote_package_but_writes_no_snapshot_or_catalog_for_it`
    (interface `Probe` with `signal s: Speed @10ms`) with
    `"[limits]\ndefault =
    64\n"` appended to its `ridl.toml`. Write the same
    files to an owner directory and run `ridl build --emit rust --out-dir <a>`
    there; serve them from the stub and run the consumer's
    `ridl build --emit rust --out-dir <b>`. Assert that the text between
    `CatalogHash([` and `])` in `<a>/veh.common.rs` equals the one in
    `<b>/veh.common.rs`.
  - `remote_units.rs`: `a_remote_units_limits_are_not_checked_by_the_consumer` —
    the same unit with
    `"[limits.kind]\nsignal = 0\n\n[limits.interaction]\n\"Nope.x\" = 8\n"`; the
    consumer's `ridl check` exits 0 with empty stderr. (Passes before the change
    too; it pins R-24's "not checked again" once the table is loaded.)
- [ ] **Step 2: Run** `cargo test -p ridl-core remote` and
      `cargo test -p ridl --test remote_units` — the two new loading and hash
      tests fail on their `assert_eq!` (no entry; hashes differ).
- [ ] **Step 3: Implement** the insert in `read_remote_unit`.
- [ ] **Step 4: Run** `cargo test -p ridl-core`,
      `cargo test -p ridl --test
      remote_units`,
      `cargo test -p ridlc --test limit_in_ir` — pass.
- [ ] **Step 5: Commit**
      `fix(ridl-core): load the [limits] table of a remote
      unit`.

### Task 2: The limit column of the descriptor's member table

**Files:**

- Modify: `crates/ridl-descriptor/schema/catalog.fbs` — append to
  `table Member`, after `timing`: `limit: uint32 = null;` with a comment: the
  payload limit in bytes that a `[limits]` entry of the unit's manifest gives
  the member (ADR-0030 decisions 12 and 13); absent when no entry applies, and
  in a file written before the field existed.
- Regenerate: `crates/ridl-descriptor/src/generated.rs` with
  `cargo xtask descriptor-codegen` (never by hand).
- Modify: `crates/ridl-descriptor/src/lower.rs` `member_of`:
  `limit: decl.limit`.
- Modify: `crates/ridl-descriptor/src/describe.rs`: the member object gains
  `"limit": member.limit()?` (a number, or `null` when absent); update the
  module and `to_json` doc comments that list the `null` fields.
- Modify: every `ridl_descriptor::Member { .. }` literal the compiler reports
  (`crates/ridl-descriptor/tests/*.rs`, `src/describe.rs` tests,
  `crates/ridl-transport-ws/tests/descriptor.rs`): add `limit: None`.
- Modify: `crates/ridl/tests/snapshots/describe_cli__corpus_catalog.snap` and
  any other `describe` snapshot that changes (every member gains
  `"limit": null`).
- Modify: `docs/design/catalog-descriptor.md` — the "Members" bullet (line ~181:
  the member carries `limit` when a `[limits]` entry applies; replace "the
  descriptor does not carry it yet"), the D-4 row of the decisions table (line
  ~592: drop "is not part of the descriptor yet"), the JSON rules (an absent
  `limit` is `null`), and the corpus excerpt (`"limit": null` in key order).
- Modify: `docs/book/catalog-descriptor.md` — the "Each member" bullet (its
  `limit`, absent when no entry applies), the "absent … is `null`" sentence, and
  the `average` JSON excerpt (`"limit": null` after `"kind"`).
- Test: `crates/ridl-descriptor/tests/lower.rs`, `src/describe.rs` `mod tests`.

**Interfaces:**

- Consumes: `ridl_ir::v2::Decl.limit: Option<u32>` (LIM2).
- Produces: `ridl_descriptor::Member.limit: Option<u32>` (owned) and
  `MemberRef::limit(&self) -> planus::Result<Option<u32>>` (reader).

- [ ] **Step 1: Make the field exist** — edit the schema, regenerate, add
      `limit: None` to the literals the compiler reports. Nothing reads or
      writes the field yet, so the crate compiles.
- [ ] **Step 2: Write the failing tests**
  - `tests/lower.rs`: `a_declared_limit_reaches_the_member` — an IR whose
    command has `limit: Some(64)` lowers to a member whose reader returns
    `Ok(Some(64))`; `an_unset_limit_is_absent` — `limit: None` reads `Ok(None)`.
  - `describe.rs`: `the_view_writes_the_limit_or_null` — one member with
    `limit: Some(64)` renders `"limit": 64`, one with `None` renders
    `"limit": null`.
- [ ] **Step 3: Run** `cargo test -p ridl-descriptor` — the three tests fail on
      their assertions.
- [ ] **Step 4: Implement** `member_of` and `to_json`.
- [ ] **Step 5: Run** `cargo test -p ridl-descriptor`,
      `cargo test -p ridl-descriptor --no-default-features`,
      `cargo test -p ridl-descriptor --no-default-features --features dedup`,
      `cargo check --target thumbv7em-none-eabihf -p ridl-descriptor --no-default-features`,
      `cargo test -p ridl --test describe_cli` (accept the changed snapshots
      with `cargo insta review` or `INSTA_UPDATE=always`, then read the diff:
      only `"limit": null` lines are added), and
      `cargo test -p ridl-transport-ws --features descriptor --test descriptor`
      — pass.
- [ ] **Step 6: Commit**
      `feat(ridl-descriptor)!: carry each member's payload limit`, body ending
      with
      `BREAKING CHANGE: the generated Member struct has a new public field,
      limit.`

### Task 3: The limit in the transport's member table

**Files:**

- Modify: `crates/ridl-transport-ws/src/server/members.rs` — each entry also
  holds `limit: Option<u32>`; `insert` keeps its signature and return type and
  sets the entry's limit to `None`; add `set_limit` and `limit` below. Update
  the type's doc comment (an entry also holds the member's payload limit).
- Modify: `crates/ridl-transport-ws/src/server/descriptor.rs` `from_descriptor`:
  after `table.insert(..)`, when `member.limit()` is `Some(n)`, call
  `table.set_limit(iface, ord, n)`; update its doc comment.
- Modify: `docs/design/ridl-transport-ws.md` D-10 (the `MemberTable` paragraph,
  line ~273): the entry's payload limit, `set_limit`, and that `from_descriptor`
  reads the descriptor's `limit` (absent → no limit).
- Test: `members.rs` `mod tests`;
  `crates/ridl-transport-ws/tests/descriptor.rs`.

**Interfaces:**

- Consumes: `MemberRef::limit()` (Task 2).
- Produces:
  `pub fn set_limit(&mut self, iface: InterfaceNo, ord: Ordinal, limit: u32) -> bool`
  (false, and nothing stored, when the table holds no entry under the key) and
  `pub fn limit(&self, iface: InterfaceNo, ord: Ordinal) -> Option<u32>`.

- [ ] **Step 1: Write the failing tests**
  - `members.rs`: `a_limit_is_set_on_an_existing_entry_only` — `set_limit` on an
    absent key returns `false` and `limit` stays `None`; after `insert`,
    `set_limit(.., 64)` returns `true` and `limit` is `Some(64)`;
    `insert_under_the_same_key_clears_the_limit`.
  - `tests/descriptor.rs`:
    `each_members_limit_is_read_and_an_absent_one_is_none` — a descriptor whose
    member 1 has `limit: Some(64)` and member 2 `None` gives
    `limit(.., Ordinal(1)) == Some(64)` and `limit(.., Ordinal(2)) == None`.
- [ ] **Step 2: Run** `cargo test -p ridl-transport-ws --lib members` and
      `cargo test -p ridl-transport-ws --features descriptor --test descriptor`
      — fail on assertions (add `set_limit` and `limit` as stubs that store and
      return nothing first, so the tests compile).
- [ ] **Step 3: Implement** the entry field, the two methods and the read in
      `from_descriptor`.
- [ ] **Step 4: Run** the two commands of Step 2 and
      `cargo test -p ridl-transport-ws` — pass.
- [ ] **Step 5: Commit**
      `feat(ridl-transport-ws): read each member's payload
      limit into the member table`.

### Task 4: Answer a request above its member's limit `corrupt`

**Files:**

- Modify: `crates/ridl-transport-ws/src/server/session.rs` `request` (line
  ~742): after the `known` check and before the backend call, when
  `self.backend.members()` gives the member a limit and `payload.len()` is above
  it, return
  `self.respond(target, seq, ResponseOutcome::Corrupt, stamp,
  receipt, out)`.
  Update the method's doc comment.
- Modify: `docs/design/ridl-transport-ws.md` D-16 — a new bullet: a request
  whose arguments are longer than the member's limit in the member table is
  answered `corrupt` without a backend call (ADR-0030 decision 15); with no
  table or no limit, the message limit alone applies; a publish is not checked.
- Modify: `docs/specification/frame-specification.md` §11.1 item 4 — the
  sentence "A frame whose header says it carries a payload is never answered or
  discarded for the payload's length" gains the exception: a provider that holds
  its catalog's member limits answers a `request` whose payload is longer than
  its member's limit `corrupt` (ADR-0030 decision 15); and the "As built" note
  at the top of the document: with a member table, the crate also answers such a
  request `corrupt`.
- Test: `session.rs` `mod tests`, in the member-table section (near `table()`,
  line ~3654).

**Interfaces:**

- Consumes: `MemberTable::set_limit`, `MemberTable::limit` (Task 3).

- [ ] **Step 1: Write the failing tests**, each over `described_with(t, |c| c)`
      where `t` is `(*table()).clone()` with `set_limit(IFACE, COMMAND.ord, 4)`
      (and `QUERY.ord` where named):
  - `a_request_above_its_members_limit_is_answered_corrupt` —
    `request(COMMAND,
    1, &[1; 5])` gives exactly one frame, decoding to
    `response(COMMAND, 0, 1, 1, ResponseOutcome::Corrupt)`; then
    `p.serve(IFACE, &[COMMAND.ord])` and `p.next_claim(&mut buf)` yields no
    claim (the backend saw no call).
  - `a_request_at_its_members_limit_reaches_the_backend` — `&[1; 4]` sends no
    frame, and `claim(&mut p, COMMAND)` returns a claim with 4 argument bytes.
  - `a_query_above_its_members_limit_is_answered_corrupt` — the same as the
    first test for `QUERY`.
  - `a_limit_of_zero_admits_only_empty_arguments` — limit 0: `&[]` reaches the
    backend, `&[1]` is answered `corrupt`.
  - `a_member_with_no_limit_checks_no_argument_length` — `COMMAND_UNBOUNDED` (no
    limit set) with `&[1; 5]` reaches the backend. Keep arguments small in every
    test: the loopback answers arguments above its own capacity with
    `SendError::TooLarge`, which is also `corrupt`, so a long argument would not
    show which check answered.
  - `a_repeat_of_a_request_answered_corrupt_for_its_length_gets_the_same_response`
    — send `seq` 1 above the limit, send `seq` 1 again: the second answer equals
    the first, and the backend saw no call (Review Focus 3).
  - `an_unknown_member_is_answered_unknown_interaction_whatever_its_length` — a
    request under `Kind::Command` naming ordinal 1 (a signal in `table()`) with
    `&[1; 5]` decodes to
    `ResponseOutcome::Contract(Contract::UnknownInteraction)` (Review Focus 4).
- [ ] **Step 2: Run** `cargo test -p ridl-transport-ws --lib session` — the
      `corrupt` tests fail on their assertions (the request reaches the
      backend), and the zero-limit test fails on `&[1]`.
- [ ] **Step 3: Implement** the check in `request`.
- [ ] **Step 4: Run** `cargo test -p ridl-transport-ws` — pass.
- [ ] **Step 5: Commit**
      `feat(ridl-transport-ws)!: answer a request above its
      member's payload limit corrupt`,
      body ending with
      `BREAKING CHANGE: a
      request whose arguments are longer than its member's limit in the member
      table is answered corrupt and does not reach the backend.`

### Task 5: The 16,292-byte message limit and the fixed retained minimum

**Files:**

- Modify: `crates/ridl-transport-ws/src/frame.rs` — beside `MIN_MESSAGE_LIMIT`:
  `pub const MAX_MESSAGE_LIMIT: usize = 16_292;` with a doc comment: the largest
  message limit, the largest data-plane message: 36 bytes of header and envelope
  plus the 16,256-byte payload ceiling (ADR-0030 decisions 2 and 10).
- Modify: `crates/ridl-transport-ws/src/server/config.rs`:
  - The default `max_message_bytes` is `MAX_MESSAGE_LIMIT`; the `Default` doc
    comment says 16,292 bytes.
  - `LimitTooSmall` is replaced by
    `pub struct LimitOutOfRange { pub given:
    usize }`, whose `Display` is
    `a message limit of {given} bytes is outside
    the allowed range, {MIN_MESSAGE_LIMIT} to {MAX_MESSAGE_LIMIT} bytes`.
    `with_max_message_bytes(n) -> Result<Self, LimitOutOfRange>` refuses
    `n < MIN_MESSAGE_LIMIT` and `n > MAX_MESSAGE_LIMIT`; its doc comment says
    the limit can only be lowered from the default.
  - `RetainedTooSmall` keeps `pub given: usize` and loses `message_limit`;
    `Display`:
    `a retained-bytes cap of {given} bytes is below the smallest
    allowed, {MAX_MESSAGE_LIMIT} bytes`.
    `with_max_retained_bytes` refuses `n < MAX_MESSAGE_LIMIT`, whatever the
    message limit. Rewrite the doc comments of `max_retained_bytes` and
    `with_max_retained_bytes`: no mention of "the larger of" the cap and the
    message limit.
- Modify: `crates/ridl-transport-ws/src/server/session.rs` — `Session::new`:
  `max_retained_bytes: config.max_retained_bytes()` (the clamp and its comment
  go); the doc comment of `respond` (line ~823) no longer says "the larger of";
  `crates/ridl-transport-ws/src/server.rs` line ~119 the same.
- Modify: `crates/ridl-transport-ws/src/lib.rs` — re-export `LimitOutOfRange`
  instead of `LimitTooSmall`.
- Modify tests that the new bounds refuse or that tested the clamp:
  - `config.rs`: `the_default_limit_is_one_mebibyte` becomes
    `the_default_limit_is_the_largest_allowed` (`== MAX_MESSAGE_LIMIT`);
    `a_limit_below_the_minimum_is_refused` expects `LimitOutOfRange`.
  - `session.rs`:
    `a_server_with_no_table_closes_1008_when_large_replies_pass_the_cap`
    (message limit 4,096 and cap 4 × 4,096 are now refused: use the default
    limit and the cap 16,292, with four replies of 4,037 bytes, each 4,073 bytes
    on the wire, reaching the cap and a fifth passing it);
    `retained_bytes_equal_to_the_cap_do_not_close_and_one_more_byte_closes_1008`
    (the cap is 16,292: four `response`s of 4,073 bytes reach it exactly and do
    not close; the next response closes with 1008);
    `a_response_above_the_default_cap_within_a_larger_limit_does_not_close` is
    deleted (it tested the clamp, and a 32 MiB limit is now refused);
    `a_retained_cap_below_the_message_limit_is_refused` becomes the new minimum
    test below.
  - `tests/serve.rs`
    `responses_past_the_retained_bytes_cap_close_1008_and_the_client_sees_it`:
    default limit, cap 16,292; 452 responses of 36 bytes (16,272 bytes) do not
    close, the 453rd (16,308) closes with 1008; update its mutation comment. The
    tests at lines ~654 and ~1143 set a 1,024-byte limit, which stays valid.
- Modify: `docs/design/ridl-transport-ws.md` — the `Config` section (line ~586:
  16,292 bytes, the range 294 to 16,292, `LimitOutOfRange`, the retained minimum
  16,292 and no clamp); D-16's retained paragraph (line ~541: drop "a session
  never applies a cap below `max_message_bytes`" and the 1 MiB arithmetic; 64
  replies at the default limit are at most 64 × 16,292 = 1,042,688 bytes, below
  the 16 MiB default cap); D-21 (line ~648: the reused buffer is bounded by
  16,292 bytes by default); D-1's export list (line ~94).
- Modify: `docs/specification/frame-specification.md` §11.1 "The sizes": the
  largest data-plane message is 36 + 16,256 = 16,292 bytes, and the Rust
  provider's message limit is at most that and 16,292 by default (ADR-0030
  decision 10).
- Modify: `docs/decisions/README.md`, the ADR-0030 entry (line ~530): the limits
  in the hash, the descriptor column, the 16,292-byte limit and the `corrupt`
  answer are built (replace "not yet built" and "will be").
- Test: `config.rs` `mod tests`; `session.rs` `mod tests`; `tests/sizes.rs`;
  `tests/serve.rs`.

**Interfaces:**

- Produces: `ridl_transport_ws::frame::MAX_MESSAGE_LIMIT: usize = 16_292`;
  `ridl_transport_ws::LimitOutOfRange { pub given: usize }`;
  `RetainedTooSmall { pub given: usize }`.

- [ ] **Step 1: Make the names exist** — add `MAX_MESSAGE_LIMIT`, rename
      `LimitTooSmall` to `LimitOutOfRange`, drop
      `RetainedTooSmall::message_limit` (its `Display` uses
      `MAX_MESSAGE_LIMIT`), and fix the callers, with the old behaviour
      unchanged.
- [ ] **Step 2: Write the failing tests**
  - `tests/sizes.rs`:
    `the_largest_message_limit_is_the_largest_data_plane_message` —
    `MAX_MESSAGE_LIMIT == ENVELOPE_BYTES + 20 + 16_256`, and equals
    `written_len` of a `response` carrying 16,256 payload bytes.
  - `config.rs`: `the_default_limit_is_the_largest_allowed`;
    `a_limit_above_the_maximum_is_refused` (`MAX_MESSAGE_LIMIT + 1` →
    `Err(LimitOutOfRange { given: 16_293 })`, and its `to_string()` contains
    `294 to 16292`); `the_maximum_limit_is_accepted`;
    `a_retained_cap_below_the_largest_message_limit_is_refused` — with a message
    limit of 1,024, `with_max_retained_bytes(16_291)` is
    `Err(RetainedTooSmall { given: 16_291 })` whose text contains `16292`, and
    `with_max_retained_bytes(16_292)` is accepted (Review Focus 5).
  - The rewritten `session.rs` and `serve.rs` 1008 tests as listed in **Files**.
    They pass before Step 4 too: once a cap below 16,292 is refused, the clamp
    can never raise a cap, so its removal changes no behaviour and has no red of
    its own. The red of this task is the refusals and the default.
- [ ] **Step 3: Run** `cargo test -p ridl-transport-ws` — the new `config.rs`
      and `sizes.rs` tests fail on their assertions (default 1 MiB; 16,293
      accepted; 16,291 accepted when the message limit is 1,024).
- [ ] **Step 4: Implement** the default, the two refusals and the removal of the
      clamp.
- [ ] **Step 5: Run** `cargo test -p ridl-transport-ws`,
      `cargo test -p ridl-transport-ws --features descriptor`,
      `cargo test -p ridl-transport-ws --no-default-features` — pass. Then
      `grep -rn "with_max_message_bytes\|with_max_retained_bytes\|LimitTooSmall" --include=*.rs --include=*.md . | grep -v target`
      finds no caller outside the crate and no stale name (examples/cabin and
      the e2e server set neither).
- [ ] **Step 6: Commit**
      `feat(ridl-transport-ws)!: limit a message to 16,292
      bytes and drop the retained-cap clamp`,
      body ending with
      `BREAKING CHANGE: the default message limit is 16,292 bytes instead of
      1 MiB; with_max_message_bytes refuses a value above 16,292 with
      LimitOutOfRange, which replaces LimitTooSmall; with_max_retained_bytes
      refuses a value below 16,292, and RetainedTooSmall has no message_limit
      field.`

### Task 6: Approve the API break in the freeze table

**Files:**

- Modify: `freeze/surfaces.toml` — delete the
  `[[surface]] name =
  "runtime-crates"` row; keep `"runtime-crates"` in
  `required` (`freeze/README.md`, "Approve a break").
- Modify: `docs/wip/2026-10-24-lane-limits-driver.md` — record the deletion as a
  ruling and the follow-up pull request that restores the row with the merge
  commit as `baseline`.

- [ ] **Step 1: Run** `cargo xtask freeze check` — fails: the `runtime-crates`
      comparison reports the breaks of Tasks 2, 3 and 5 (the `Member` field,
      `LimitTooSmall` removed, `RetainedTooSmall::message_limit` removed). Every
      line-dump surface passes; if one does not, stop and report it (no dump is
      expected to change: the frame vectors are not edited, the describe JSON is
      not a frozen surface, and no freeze fixture has a `[limits]` table).
- [ ] **Step 2: Delete the row.**
- [ ] **Step 3: Run** `cargo xtask freeze check` — passes, printing the notice
      for `runtime-crates` without a row. Run `prim fmt` on every edited
      Markdown file, then `just link-check`, `just doc-path-check` and
      `just story-id-check` — pass.
- [ ] **Step 4: Commit**
      `chore(repo): approve the runtime-crates API break of
      the transport limits`.

## The pull request

Title:
`feat(ridl-transport-ws)!: payload limits in the descriptor and the
transport`.
Body: what each task did; the freeze approval ("the `runtime-crates` row is
removed; the owner accepted the breaks on driftsys/ridl#1059 on 2026-10-24");
`Closes #1059`, `Closes #1067`; and the `BREAKING CHANGE:` footer joining the
three footers above. Merge per R-25. A follow-up pull request restores the
`runtime-crates` row with the merge commit as `baseline`.

## Open questions / rulings needed

Rulings this plan takes that R-1..R-25 do not cover, and places it is unsure:

1. **Error type for a too-large message limit.** The plan replaces
   `LimitTooSmall { given }` with `LimitOutOfRange { given }`, one type for both
   ends of the range, because `LimitTooSmall` would be a false name for a value
   above 16,292. The alternative is to keep `LimitTooSmall` and add
   `LimitTooLarge`, with `with_max_message_bytes` returning an enum of the two.
   Both are breaking; the owner accepted the break of the setter, not a name.
2. **Name of the constant.** `frame::MAX_MESSAGE_LIMIT = 16_292`, beside
   `MIN_MESSAGE_LIMIT`. `ridl-rt` has no `PAYLOAD_CEILING` constant yet (it
   lives in `ridl-core::limits`), so the transport writes 16,292 as a literal
   with a test that derives it from 36 + 16,256 instead of computing it from a
   shared constant.
3. **`MemberTable` API.** The plan adds `set_limit` and `limit` and leaves
   `insert` and `member` unchanged, so the member table's existing API does not
   break, and `insert` on an existing key clears the limit. The alternative is a
   fourth tuple element on `insert` and `member` (a larger break, one call
   instead of two).
4. **No `Note` for an over-limit request.** The plan answers it `corrupt` and
   records nothing, as a payload that fails to decode is not recorded. A new
   `Note::ArgumentsTooLong { target, len, limit }` (additive: `Note` is
   `#[non_exhaustive]`) would let an embedder see it.
5. **Order of the checks in `request`.** The `seq` is placed first, then the
   member is looked up (`contract(UnknownInteraction)`), then the length. So a
   repeat is answered from the kept responses before any check, and an unknown
   member wins over a long argument. ADR-0030 decision 15 does not fix the
   order.
6. **`SCHEMA_VERSION` stays 1.** An optional field appended at the end reads as
   absent in an older file and is ignored by an older reader, as for
   `compatible`. A reader that must distinguish "no limit declared" from "a
   descriptor older than the column" cannot; ADR-0030 decision 15 treats both as
   "message limit alone", so the plan does not.
7. **A descriptor limit above 16,256.** `from_descriptor` stores any `u32` and
   does not refuse a value above the ceiling (the manifest cannot produce one).
   A refusal would be a new `DescriptorError` variant.
8. **The retained minimum is fixed, not the current message limit.** ADR-0030
   decision 15 says the minimum is "the largest message limit, 16,292"; the plan
   reads that as the constant, so a cap of 16,292 is valid with a message limit
   of 294. The issue text agrees; the earlier rule compared with the configured
   message limit.
9. **The frame specification's upper bound.** The plan states 16,292 in §11.1
   "The sizes" as the Rust provider's maximum and default only. Whether every
   receiver of the binding (the TypeScript client included) must also refuse a
   larger limit is not decided by ADR-0030; the plan does not make it a rule of
   the binding.
10. **Book pages.** `grep -rn "1 MiB" docs/book` finds no page that states the
    old default, so no book page changes for it; the book pages that change are
    `cli-reference.md` (remote limits, Task 1) and `catalog-descriptor.md` (the
    column, Task 2). `docs/decisions/README.md` changes (Task 5); ADR-0030
    itself is not edited. Whether ADR-0030's Status should gain a dated "built"
    note is the owner's call.
11. **Commit scope of this plan.** The brief named `docs(repo)`; the two earlier
    plan commits of this lane used `docs(roadmap)` ("the implementation backlog"
    in `.git-std.toml`), so this plan was committed under `docs(roadmap)`.
