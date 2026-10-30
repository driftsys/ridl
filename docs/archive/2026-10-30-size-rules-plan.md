# Size rules of ADR-0030 — implementation plan

Status: plan, 2026-10-30. Covers driftsys/ridl#1108 and driftsys/ridl#1128: the
code of the ADR-0030 amendments of 2026-10-28 and 2026-10-30. Nothing here is
implemented yet. One branch (`feat/size-rules`), one task per implementer, one
review per task before the next implementer starts, and one pull request at the
end, released in the next version marked breaking.

Read first: [ADR-0030](../decisions/ADR-0030-transfer-and-bound-limits.md) (all
of it, the Status block lists every record to update), ADR-0031 decision 10 and
open item 2, ADR-0024 decisions 7 and 11, ADR-0029 and `freeze/README.md`, the
frame specification §9 and §11.1, and the two issues with their comments
(`gh issue view 1108 --comments`, `gh issue view 1128 --comments`). The owner's
comments of 2026-10-30 on both issues withdraw the codec guard: there is no
`EncodeError::TooLarge` and no `PAYLOAD_CEILING` in `ridl-rt`.

## Goal

When the branch lands:

- `ridl_core::limits` holds the three default ceilings (3,840 / 16,128 /
  65,280), the largest raise, and the TYPL-116 thresholds.
- Every interaction whose payload, in an enabled encoding, is above its kind's
  default ceiling or has no finite bound, with no entry that gives its limit,
  draws the error RIDL-157. RIDL-150 is retired; `payload-above-ceiling` is a
  reserved name and a `[lints]` key naming it draws MANI-010.
- RIDL-151 checks against the resolved limit, with `default` clamped to each
  kind's ceiling. MANI-022 follows decisions 12 and 18.
- TYPL-116 warns on a `string` bound above 16,256 characters and a `bytes` bound
  above 65,280 bytes.
- The language server publishes RIDL-151, RIDL-152, RIDL-157 and the MANI-022 of
  the limits pass.
- The generated FlatBuffers and podbuf `verify` check structure only.
- The transport owns size: `ridl-rt` exports the three default ceilings; a
  sending port refuses a payload above its member's limit with
  `TooLarge { cap }`; every receiving endpoint refuses a payload above its
  member's limit before `verify`; the WebSocket message limit is 16,164 bytes,
  and a `Config` that cannot carry an allowed payload is refused.
- The corpora and fixtures fit the new rules; the records listed in ADR-0030's
  Status are updated; the freeze dumps are regenerated.

## Order and dependencies

| Task | Title                                                    | Tier   | Depends on |
| ---- | -------------------------------------------------------- | ------ | ---------- |
| 1    | Bound the corpora and fixtures                           | sonnet | —          |
| 2    | `ridl-core` limits, MANI-022 ranges, TYPL-116            | sonnet | —          |
| 3    | The `ridlc` size pass: RIDL-157, RIDL-150 retired        | fable  | 1, 2       |
| 4    | The language server runs the size pass                   | sonnet | 3          |
| 5    | `verify` checks structure only                           | fable  | —          |
| 6    | `ridl-rt` default ceilings, send-side refusal (loopback) | fable  | 2          |
| 7    | `ridl-transport-ws` (Rust): limits, Config, both sides   | fable  | 6          |
| 8    | Generated TS limits and the TS client's receive checks   | fable  | 3, 6       |
| 9    | Close-out: freeze, records, ADR-0030 Status              | sonnet | 1-8        |

Suggested run order: 1, 2, 5, 3, 4, 6, 7, 8, 9. Tasks 1, 2 and 5 are independent
of each other. Each task leaves `cargo test --workspace` green for the crates it
touches (implementers run focused crate tests; CI runs the full gate).

Rules for every task:

- Red first, and red is an **assertion failure**, never a compile error: write
  the first test against behaviour that already compiles (a diagnostic code
  compared as a string, an `Ok`/`Err` that today has the other value), and add
  tests that name new items only after the item exists.
- The reviewer receives the diff from the base recorded before the task's
  implementer started, and gives two verdicts: the task meets its brief, and the
  code is sound. The reviewer applies the listed semantic mutation in its own
  worktree with its own `CARGO_TARGET_DIR`, and confirms a test fails.
- Prose is plain, literal English. No story ids in shipped docs or rustdoc
  (`just story-id-check`). Shipped docs describe the system as built, so each
  task updates its docs in the same commit as its code.
- Do not edit `freeze/surfaces.toml`. Regenerating dumps with
  `cargo xtask freeze update` is fine.

## Task 1 — Bound the corpora and fixtures (sonnet)

**Why first.** Tasks 3 and 8 turn oversized payloads into errors; every shipped
source must already fit, so that task 3's diff carries only rule changes.

**Files.**

- `evals/corpus/ros2/**` and its frozen copy
  `xtask/tests/freeze-fixtures/sources/evals/corpus/ros2/**` (byte-identical),
  `evals/corpus/ros2/PROVENANCE.md`.
- `crates/ridlc/tests/corpus/veh-cluster/cluster/appendix-a.ridl`
  (`FaultPage.faults : [FaultEvent; 0..64]`), its `NOTES`, and the frozen copy
  `xtask/tests/freeze-fixtures/generated/veh-cluster`; the snapshots and tests
  that read the corpus (`crates/ridlc/tests/codegen_model.rs`,
  `rust_crate_emit.rs`, `corpus.rs`,
  `crates/ridl-backend-proto/tests/corpus.rs`,
  `crates/ridl-backend-podbuf/tests/corpus.rs`, the conformance tests in
  `crates/ridl-backend-rust/tests/`); `freeze/generated/veh-cluster` and
  `freeze/wire.txt` through `cargo xtask freeze update`.
- `crates/ridl-backend-ts/tests/fixtures/codec/shape.ridl` (signals
  `Sensors.note` and `Sensors.extras`, event `Sensors.bounds`) and the
  expectations in `crates/ridl-backend-ts/tests/codec.rs`.

**Behaviour.** No source changes meaning except its bounds:

- ros2: the 15 commands and queries with no finite bound (`clearEntireCostmap`,
  `followPath`, `followWaypoints`, `getCostmap`, `isPathValid`, `loadMap`,
  `navigateToPose`, `saveMap` of `nav2msgs`; `getMap`, `getPlan`, `loadMap`,
  `setMap` of `navmsgs`; `setCameraInfo` of `sensormsgs`; `setBool`, `trigger`
  of `stdsrvs`) fit 16,128 bytes, and the two feedback events
  (`followPathFeedback`, `navigateToPoseFeedback`) fit 3,840 bytes, under
  FlatBuffers (ros2 names no backend table, so FlatBuffers is the checked
  encoding). See R-14.
- veh-cluster: `getFaultPage`'s reply fits 16,128 bytes in FlatBuffers, proto3
  and podbuf. Lower `FaultPage.faults` to the largest `N` that fits (measured
  near 14 from 70,649 / 67,008 / 67,152 bytes at 64; measure, do not compute).
  R-15.
- TS fixture: the three members fit 3,840 bytes in FlatBuffers. R-16.
- Any other fixture that `cargo test --workspace` shows drawing RIDL-157 after
  task 3 is fixed in task 3, not here (R-17), but list the candidates you find
  now (`crates/ridl-backend-rust/tests/fixtures/proto3_unbounded_call.ridl`,
  `proto3_free.ridl`) in the task report.

**Failing test first.** Add a test (in `crates/ridl/tests/eval_corpus.rs` for
ros2, in `crates/ridlc/tests/corpus.rs` for veh-cluster, in
`crates/ridl-backend-ts/tests/codec.rs` for the fixture) that copies the
workspace to a temporary directory, adds to every unit manifest

```toml
[limits.kind]
signal = 3840
event = 3840
command = 16128
query = 16128
```

and asserts `ridl check` reports no RIDL-151. Every value is within today's
MANI-022 range (16,256) and within task 2's ranges, so the test stays valid
after tasks 2 and 3. Today it fails with RIDL-151 on the listed members.

**Semantic mutation.** Restore one bound (for example `SetBoolResponse.message`
back to `RosString`, or `faults` back to `[0..64]`): the test fails.

**Watch.** `evals/calibration/expected-counts.toml` pins the design-lint counts
on ros2 (`design_lint_counts_on_the_corpus_are_pinned`); a new bounded struct
can add a `duplicate-shape` finding. Keep the counts, or update the file
deliberately with the reason in the commit message. The calibration records name
byte offsets in ros2 files (`evals/calibration/*.toml` "Sampled:" lines); check
whether `xtask` tests re-resolve them (`xtask/src/calibrate.rs`,
`xtask/tests/calibrate_cli.rs`) and report it. Keep the six `RosString`
declarations unchanged: ADR-0030 Consequences expect them to keep their TYPL-116
warning.

**Docs in this task.** `PROVENANCE.md` (each modification), veh-cluster `NOTES`.
No shipped doc names the old bound.

## Task 2 — `ridl-core` limits, MANI-022 ranges, TYPL-116 (sonnet)

**Files.** `crates/ridl-core/src/limits.rs`, `crates/ridl-core/src/manifest.rs`
(`read_limit` and its callers; `LimitEntry`), `crates/ridl-core/src/diag.rs`
(the MANI-022 catalogue text), `crates/ridl-sem/src/check.rs` (TYPL-116 at
~2129), `crates/ridlc/tests/payload_ceiling.rs` (TYPL-116 tests).

**Behaviour.**

- `limits.rs`: the default ceiling of each kind (3,840 for signal and event,
  16,128 for command and query, 65,280 for fixed) through one function over
  `manifest::LimitKind`; `RAISE_MAX = 16_128` (largest limit an interaction
  entry may give a non-fixed); the TYPL-116 thresholds
  `STRING_BOUND_MAX_CHARS = 16_256` and `BYTES_BOUND_MAX = 65_280`.
  `PAYLOAD_CEILING` becomes 65,280, the largest payload of any kind (R-18), and
  the old `ADVISED_*` names go. The unit test relates them: 3,840 = 1 × 4,096 −
  256, 16,128 = 4 × 4,096 − 256, 65,280 = 16 × 4,096 − 256; 16,256 = 254 × 64; 4
  × 16,256 + 58 ≤ 65,280.
- Manifest (decision 12): a `[limits.kind]` entry above its kind's default is
  MANI-022; `default` is valid up to 65,280 and MANI-022 above it; a
  `[limits.interaction]` entry above 65,280 is MANI-022. Message: "… is N, above
  what the entry may give for its kind (M bytes)". `LimitEntry` gains the byte
  range of its value, which task 3 needs to report MANI-022 on the value.
- TYPL-116: the new thresholds; the message names 65,280 bytes as the largest
  payload of any kind and no longer names 16,256 or "the codecs do not yet
  enforce it" (decision 7 as amended).

**Failing test first.** In `manifest.rs` tests: `[limits.kind] signal = 3841`
draws MANI-022 (today nothing: 3,841 ≤ 16,256). In
`crates/ridlc/tests/payload_ceiling.rs`: `string [0..16256]` and
`bytes [0..65280]` draw no TYPL-116 (today both warn). Then: MANI-022 for
`event = 3841`, `command = 16129`, `query = 16129`, `fixed = 65281`,
`default = 65281`, an interaction entry of 65,281; none at each default and at
`default = 65280`; TYPL-116 at 16,257 characters and 65,281 bytes, and its
message contains "65280" and not "16256".

**Semantic mutation.** Compare a `[limits.kind] signal` entry against 16,128
instead of 3,840; or use `>=` for TYPL-116; or swap the string and bytes
thresholds. A test fails.

**Docs.** typl reference §4.4, §4.5 and §16.2 (the TYPL-116 threshold and
wording); `docs/book/lints.md` (TYPL-116 row and paragraph); ADR-0002 (a dated
note: the range of a `[limits]` value and MANI-022); family overview §7 (the
MANI-022 row); `docs/book/cli-reference.md` (`[limits]`: the value ranges and
the clamp of `default`).

## Task 3 — The `ridlc` size pass: RIDL-157, RIDL-150 retired (fable)

**Files.** `crates/ridl-core/src/diag.rs` (catalogue: remove the RIDL-150 row,
add RIDL-157 as an Error with no lint name, `RETIRED_RIDL_CODES` gains 150, a
reserved lint-name list with its guard, the expected lint-name list near line
2442), `crates/ridlc/src/design_lints/{ceiling,limits,mod}.rs`,
`crates/ridlc/src/lib.rs` (`check_loaded`, `check_unit_limits`,
`payload_ceiling_blocked`), `crates/ridlc/tests/payload_ceiling.rs`,
`crates/ridlc/tests/payload_limits.rs`, `crates/ridlc/tests/corpus.rs` (profile
codes), `crates/ridl-backend-ts/tests/codec.rs` (allowed codes),
`freeze/diagnostics.txt`.

**Behaviour.**

- One pass sizes every interaction and raises RIDL-151, RIDL-152, RIDL-157 and
  the ridlc MANI-022, with one skip rule: the one RIDL-151 uses today
  (`payload_ceiling_blocked(.., by_manifest = true)`, remote packages skipped).
  It runs whether or not any unit has a `[limits]` table (today
  `check_unit_limits` returns early when none has).
- Resolution of a limit (decision 12): the `[limits.interaction]` entry; else
  the `[limits.kind]` entry; else `default` clamped to the kind's ceiling; else
  none. With an entry: RIDL-151 against that limit. With none: RIDL-157 against
  the kind's default ceiling, for every kind (fixed included, sized as
  `fixed_entry` does today). One interaction draws at most one of the two (R-1).
  A call checks its request and its reply; a fallible reply is checked through
  both types, as `fallible_entry` does.
- MANI-022 from this pass for a `[limits.interaction]` value between 16,129 and
  65,280 that names a member other than a fixed, on the value span (task 2).
  Such an entry gives no limit (RIDL-151 is not also raised for it).
- `annotate_limits` records the resolved limit when an entry applies (with the
  clamp; R-2) and nothing when none applies.
- RIDL-150 is gone: catalogue row, `ceiling::check`, `check_payload_ceiling` and
  its call in `check_loaded`. `payload-above-ceiling` is reserved: add
  `RETIRED_LINT_NAMES` (R-3) and a guard test that no catalogue row uses a name
  in it. A `[lints]` key naming it draws MANI-010 and is ignored, through the
  existing unknown-name path.
- Expose one public entry point in `ridlc` that runs this pass over a checked
  workspace (packages, checked packages, resolutions, the diagnostics so far,
  the backends' encodings, the unit limits and the units), for task 4. `ridlc`
  calls it itself, so the two callers cannot diverge.

**Failing test first.** In `payload_ceiling.rs`, with no `[limits]` table: a
signal whose payload is `string [0..946]` (FlatBuffers) draws a diagnostic whose
code string is `"RIDL-157"` (today: nothing, the code is not catalogued, so the
assertion fails; the string-literal guard
`codes_written_as_string_literals_are_all_catalogued` also fails until the row
exists, which is an assertion too). Then the issue's tests:

- RIDL-157 on a signal and an event at 3,841 bytes, on a command and a query at
  16,129, on a fixed at 65,281, and on each kind with no finite bound; none at
  each ceiling. The measured largest `string [0..N]` in a 3,840-byte signal: 945
  in FlatBuffers, 959 in proto3, 955 in podbuf (each draws nothing; N + 1 draws
  RIDL-157 with only that encoding enabled).
- A fallible reply whose error type alone is oversized draws RIDL-157 naming
  that type.
- An interaction entry of 16,128 on a signal removes RIDL-157; a payload above
  it draws RIDL-151.
- MANI-022 from `ridlc` for an interaction entry of 16,129 on a command and on a
  signal, none on a fixed.
- `default = 65280` with a 3,841-byte signal draws RIDL-151 with "its limit is
  3840 bytes".
- `[lints] payload-above-ceiling = "deny"` draws MANI-010 and the check still
  runs.
- Catalog hash: a unit with no `[limits]` table keeps its hash; a raised limit
  changes it (`crates/ridlc/tests/payload_limits.rs` has the hash helpers).
- A package with an error, or that depends on one, draws none of the three.

**Semantic mutation.** Use 16,128 as the fixed ceiling; or let an interaction
entry not suppress RIDL-157; or drop the `default` clamp; or size only the `ok`
side of a fallible reply. A test fails.

**Docs.** ridl reference diagnostics table (RIDL-150 retired, RIDL-151 row no
longer names RIDL-150, RIDL-157 for every kind); `docs/book/lints.md` (the
`payload-above-ceiling` row and paragraphs removed or marked retired with the
reserved name, the `[limits]` paragraph, the sentence that a fixed is not
checked); `docs/book/cli-reference.md` (`[limits]`: the order that resolves a
limit, the raise, RIDL-151, RIDL-157); `docs/book/stability.md` and an ADR-0029
note (only TYPL-116 remains a lint that starts at `warn`); ADR-0024 note
(RIDL-150 retired, its name reserved, RIDL-157 for every kind);
`docs/design/catalog-descriptor.md` (an absent `limit` means the kind's default;
a raised limit of a signal or an event is above 3,840 bytes);
`freeze/diagnostics.txt` regenerated.

**Note.** ADR-0024 decision 18 (inline directives, driftsys/ridl#1124, not
built) uses RIDL-150 as an example of a lint the language server does not run.
When #1124 is built, `payload-above-ceiling` is not a lint name (TYPL-412).
State that in the ADR-0024 note; do not change decision 18's text.

## Task 4 — The language server runs the size pass (sonnet)

**Files.** `crates/ridl-lsp/src/server.rs` (`load`: keep `backends`, `units` and
`unit_limits`, which are discarded today; the recompute near line 850), LSP
tests.

**Behaviour.** After the analysis, the server calls task 3's entry point with
the workspace packages and the encodings of every `[backend.<name>]` table of
the loaded manifest, as `ridl check` does, before `apply_lint_levels`. A
standalone overlay (a one-package source set outside the workspace) has no
`[limits]` table and no backends: it gets RIDL-157 against the kind ceilings
under FlatBuffers. Diagnostics on `ridl.toml` (MANI-022 of the pass, RIDL-152)
are published for the manifest file. Remote packages publish nothing, as today.

**Failing test first.** An LSP test opens a workspace whose signal carries a
`string [0..1000]` and asserts the published diagnostics for that file contain
`RIDL-157` (today they do not). Then: a proto3-only workspace whose payload is
oversized only in proto3; an edit in the buffer that shrinks the bound clears
the diagnostic; RIDL-151 and RIDL-152 appear; a `[limits.interaction]` value of
16,129 on a signal shows MANI-022 on `ridl.toml`.

**Semantic mutation.** Pass an empty encoding list (FlatBuffers only): the
proto3 test fails. Or skip the pass for overlays: the edit test fails.

**Docs.** `docs/book/lints.md` (the sentence that the language server does not
run the size checks), the ADR-0024 note of task 3 (the server runs them), the
module comment of `server.rs`.

## Task 5 — `verify` checks structure only (fable)

**Files.** `crates/ridl-backend-rust/src/codec.rs` (~2925),
`crates/ridl-backend-rust/src/codec_podbuf.rs` (~514), their snapshots, the
tests `flatbuffers_roundtrip.rs`, `podbuf_conformance.rs`, `podbuf_verify.rs`,
`interaction_face.rs`, `generated/*`; `freeze/generated/*` through
`cargo xtask freeze update`.

**Behaviour (decision 19).** Remove the `buf.len() > MAX_SIZE` refusal from the
generated FlatBuffers and podbuf `verify`. `verify` reads only what the
structure references; bytes past it are ignored. `Malformed::TooLarge` stays in
`ridl-rt` and the generated code no longer returns it. proto3 is unchanged and
gains no check. `MAX_SIZE` stays the computed bound (decision 3 withdrawn); a
type with no finite bound keeps having no codec.

Audit, and state the result in the task report: no generated accessor, `decode`,
`root` or podbuf head or tail check relies on `buf.len() <= MAX_SIZE`. The work
of `verify` stays bounded by the reader's declared maxima (ADR-0031 decision 8)
and the FlatBuffers verifier's limits (`TooDeep`, `TooManyTables`).

Decision 5 (no `BUFFER` above 16,128 bytes on the stack): the face carries no
method for a fixed (`face.rs`, "A `fixed` is provisioned, not interacted with"),
and `MAX_BUFFER_SIZE` counts no fixed (a fixed reserves nothing,
`descriptors.rs`). Add a test that pins both: the face of an interface with a
fixed whose payload is 20,000 bytes names no `BUFFER` of that type, and its
`MAX_BUFFER_SIZE` is the largest call payload. With task 3, every other `BUFFER`
the face uses is at most 16,128 bytes.

**Failing test first.** For podbuf and for FlatBuffers: an older reader verifies
a newer writer's buffer that is longer than the older `MAX_SIZE` (v1
`Reading { value: f32 }`, v2 appends `note: string [0..64]`), as
`a_newer_writers_payload_above_max_size_verifies` does for proto3. Today
`verify` returns `Structure(TooLarge)`. Then: a valid buffer followed by
trailing bytes verifies and decodes to the same value; a malformed buffer longer
than `MAX_SIZE` still fails with its structure error, not `TooLarge`.

**Semantic mutation.** Re-add the length check for podbuf only; or make the
podbuf verifier compute its tail end from `buf.len()` instead of the
descriptors. A test fails.

**Docs.** `docs/design/flatbuffers-codec.md` and `docs/design/podbuf-codec.md`
(no length check in `verify`; an embedder that verifies bytes no binding
checked, such as the podbuf gate, checks the length against the interaction's
limit itself; the "Known gaps" row of driftsys/ridl#1099; the stack rule and the
fixed read through a view); `docs/design/proto3-codec.md` (what protects a
receiver is the transport's refusal against the interaction's limit);
`docs/design/interaction-face.md` and `docs/design/ridl-rt.md` (the stack rule).

**Concurrency.** PR #1127 (branch `fix/podbuf-debt-1100`, another session)
changes `podbuf_verify.rs` and `podbuf_conformance.rs`. If it lands first,
rebase before starting.

## Task 6 — `ridl-rt` default ceilings and the send-side refusal (fable)

**Files.** `crates/ridl-rt/src/contract.rs` (R-4), `crates/ridl-rt/src/port.rs`
(rustdoc of `TooLarge { cap }`),
`crates/ridl-loopback/src/{lib,store,handle}.rs`, `crates/ridl-rt-conformance`
(R-6), a parity test in `crates/ridl-backend-rust/tests/` (the one crate that
depends on both `ridl-rt` and `ridl-core`).

**Behaviour.**

- `ridl-rt` exports the three default ceilings as values of the catalog rule
  (R-4). They are not a codec guard: no `EncodeError::TooLarge`, no
  `PAYLOAD_CEILING`.
- The loopback's sending ports refuse a payload longer than the member's limit
  with `TooLarge { cap }`, `cap` = that limit: `SignalWriter::set` →
  `WriteError`, `EventSink::raise` → `RaiseError`, `Caller::command` and
  `Caller::query` arguments → `SendError`, `Handler::settle` reply →
  `SettleError`. The refused value is not stored, and nothing is sent. The limit
  is the one given to the loopback for that member (R-5), else the kind's
  default ceiling, the kind being the port's.
- A `fixed` is provisioned, not sent through a port: `Loopback::provision_fixed`
  refuses a value longer than the member's limit (65,280 by default, or the
  limit given), returns the refusal, and stores nothing (R-5). A `read_fixed` of
  a fixed with no stored value answers what it answers today for an
  unprovisioned fixed: `ReadError::Contract(Contract::UnknownInteraction)`. So a
  fixed above its catalog limit is never stored, and a consumer's read of it is
  answered, never left waiting (coordinator input, from the WebSocket blocks
  design note, PR #1131).
- The loopback is not a binding: its receiving side reads only what its sending
  side accepted, so it adds no receive check (R-5).
- The generated face already returns the port's error unchanged; a refused
  `settle` is answered `corrupt` as today (`interaction_face.rs`,
  `an_oversized_claim_is_settled_corrupt…`).

**Failing test first.** In `ridl-loopback`: `set` of 3,841 bytes on a signal
returns `Err(WriteError::TooLarge { cap: 3840 })` (today `Ok`). Then: 3,840
passes; an event, a command, a query's arguments and a reply at their ceiling
plus one; a member limit given to the loopback lowers and raises the cap;
`provision_fixed` above the limit is refused and a later `read_fixed` answers
`Contract(UnknownInteraction)`. After the constants exist: the parity test (each
`ridl-rt` value equals `ridl_core::limits`).

**Semantic mutation.** Compare with `>=`; or ignore a given member limit for
events; or store the refused fixed. A test fails.

**Docs.** `docs/design/ridl-rt.md` and `docs/design/interaction-face.md` (the
port's `TooLarge { cap }` against the member's limit is the one refusal on
send); ADR-0021 (the entry of ADR-0030's first list: `ridl-rt` gains the three
default ceilings, no `PAYLOAD_CEILING`, no `EncodeError::TooLarge`); the
`ridl-loopback` crate docs ("What it reports, and what it cannot": `TooLarge`
now comes from the limit, not only from `fail_next_settle`).

## Task 7 — `ridl-transport-ws` (Rust): limits, Config, both sides (fable)

**Files.** `crates/ridl-transport-ws/src/frame.rs` (`MAX_MESSAGE_LIMIT`),
`src/server/config.rs`, `src/server/session.rs`, `src/server.rs` (`serve`,
`Ended`, `Note`), `src/server/members.rs`, tests `serve.rs` and `sizes.rs`.

**Behaviour.**

- `MAX_MESSAGE_LIMIT` = 36 + 16,128 = 16,164: the default and the largest
  accepted value; `LimitOutOfRange` covers 294 to 16,164; the smallest retained
  cap is 16,164 (`RetainedTooSmall`).
- A member's limit is its member-table limit, else the default ceiling of its
  kind (`ridl-rt`, task 6). Without a member table, the kind is the frame's
  `kind` (R-10).
- Receive side (decision 19, provider): a `request` whose arguments are longer
  than the member's limit is answered `corrupt` without calling the backend, as
  today, now also for a member with no limit and for a backend with no table.
- Send side (R-9): a value the session would send that is longer than its
  member's limit — a signal or fixed `publish`, an `occurrence`, a `reply` — is
  refused as a value above the message cap is refused today: no `publish` or
  `occurrence`, `Note::ValueTooLarge`; a reply is sent `corrupt` and recorded.
  With a runtime that refuses on send (task 6), this path is reached only by a
  backend that does not.
- `Config` check (decision 10 as amended, R-7): with a member table, `serve`
  refuses a `Config` whose `max_message_bytes` is below 36 + the largest limit,
  over the members, of what one message carries — each member's limit, capped at
  16,128 for a fixed. The part of a fixed above 16,128 is carried by blocks
  (driftsys/ridl#1129), so it is not counted, and a catalog with a large fixed
  is never refused at the default `Config` (decision 20). No attach refusal for
  a fixed is added.
- A fixed above 16,128 bytes and within its limit, until driftsys/ridl#1129
  lands (R-8, amended): the session cannot carry it in one message. No `publish`
  is sent for it; the session records `Note::ValueTooLarge { target, needed }`
  and answers the `read` with an `answer` carrying
  `contract(UnknownInteraction)`, so the read ends. The catalog is still
  attached and every other member is served. A fixed above its limit is never
  stored by a runtime that refuses on send (task 6), and a read of a fixed with
  no value is answered `contract(UnknownInteraction)` through the existing
  `ReadError::Contract` path.

**Failing test first.** `Config::default().with_max_message_bytes(16_165)`
returns `Err` (today `Ok`). Then: 16,164 is accepted and is the default;
`with_max_retained_bytes(16_163)` is refused; a request of 16,129 bytes to a
member with no limit is answered `corrupt` with and without a table; a signal of
3,841 bytes from a backend that does not refuse on send is not published and is
noted; the `Config` check refuses a `Config` of 1,000 bytes for a catalog whose
command has no limit, accepts it for a catalog whose `[limits]` lower every
member to 900 bytes, and accepts the default `Config` for a catalog with a fixed
of default limit; a read of a 20,000-byte fixed records `ValueTooLarge` and the
session keeps serving a signal; a read of an unprovisioned fixed is answered
`contract(UnknownInteraction)`.

**Semantic mutation.** Count a fixed's full limit in the `Config` check (the
default `Config` with a fixed is refused); or leave the retained minimum at
16,292; or skip the request check when the table has no limit. A test fails.

**Docs.** `docs/design/ridl-transport-ws.md` (16,164, the retained minimum and
its arithmetic, the size checks of each side, the `Config` check, the fixed
above one message until #1129); the frame specification §9 (the refusal before
`verify`) and §11.1 "The sizes" and item 4 (the ceilings of decision 16, 16,164,
the request check against the member's limit or its kind's default; the numbers
16,256 and 16,292 are corrected here). Blocks themselves are not described: they
wait for #1129. `cargo xtask freeze update` (the `runtime-crates` comparison;
see "Freeze").

## Task 8 — Generated TS limits and the TS client's receive checks (fable)

**Ruling adopted (R-11, the coordinator's R-B1).** The TypeScript consumer gets
each member's limit from the generated `<base>.codec.ts` module, because the
catalog is the one source of a limit. `ConnectOptions` gains no hand-written
limit.

**Files.** `crates/ridl-ir/proto/ridl/codegen/v1/model.proto` and the lowering
in `ridl_ir::codegen` (an `optional uint32 limit` on `Interaction`, copied from
the IR's `limit`; R-12), `crates/ridl-backend-ts/src/codec.rs`
(`emit_interface`: `limit: N` on a member with a recorded limit), its snapshots
and fixtures; `packages/ridl-transport-ws/src/{frame,session,calls}.ts`,
`mod.ts`, `tests/`, README; `freeze/codegen-protocol.txt`.

**Behaviour.**

- The generated member entry carries `limit` when the catalog records one;
  absent means the kind's default.
- `packages/ridl-transport-ws` exports the three default ceilings; a Rust test
  reads `packages/ridl-transport-ws/src/frame.ts` and compares them with
  `ridl-rt` (R-13).
- `connect` takes the generated interface tables (the objects the module already
  exports, now with `limit`) and resolves each received payload's limit by
  `(kind, iface, ord)`, else the kind's default.
- Receive side (decision 19), before the payload reaches the application: a
  `reply` longer than its member's limit rejects the call with
  `SessionError({ category: "corrupt" })`; a `read` of a fixed rejects the same
  way; a `publish` of a signal is delivered as a sample with provenance
  `invalid`, `payload: null` and a new optional field `detected: "corrupt"`,
  keeping the frame's envelope (§9.1); an `occurrence` is delivered with an
  empty payload and `detected: "corrupt"` (§9.2). R-19.
- Send side: no local check (R-20); the provider answers an oversized request
  `corrupt`.

**Failing test first.** A Deno unit test with a fake socket: a `reply` of 16,129
bytes to a query rejects with `corrupt` (today it resolves). This uses only the
kind default, so it compiles before any option exists. Then: the generated
module of a fixture with a raised signal carries `limit`; a publish above 3,840
bytes on a raised member within its limit is delivered `live`; one above the
raised limit is `detected: "corrupt"`; an occurrence; a fixed read.

**Semantic mutation.** Compare with `>=`; or check only subscribed signals and
not reads; or ignore the generated `limit`. A test fails.

**Docs.** `docs/design/ridl-transport-ws-ts.md` (the size checks; replace D-10's
"A message larger than a limit is not checked" with what is checked now),
`docs/design/codegen-plugins.md` and `docs/book/codegen-plugins.md` where the
model's interaction fields are listed, the package README, `examples/cabin/web`
if it calls `connect`.

## Task 9 — Close-out (sonnet)

- `cargo xtask freeze update`; run `just freeze-check` and report each surface
  that fails (see "Freeze").
- ADR-0030 Status: replace the "not built" sentences with "Built <date>" notes
  for both amendments, naming what is built and what waits for
  driftsys/ridl#1129 (blocks). `docs/decisions/README.md` summary of ADR-0030.
- Sweep the remaining records in ADR-0030's Status lists that tasks 2-8 did not
  touch; `just book-check`, `just link-check`, `just doc-path-check`,
  `just story-id-check`.
- Garden this plan: archive it verbatim to `docs/archive/` and update
  `docs/wip/README.md`. Check
  `docs/wip/2026-10-28-payload-ceilings-postponed.md` with the owner: its
  section 1 stays postponed, so it stays in `docs/wip/` unless the owner moves
  it.

## Freeze

The final pull request cannot merge until the owner deletes, or updates, the
rows in `freeze/surfaces.toml` of the surfaces it breaks (ADR-0029, "Approve a
break"). Agents do not edit `surfaces.toml`.

| Surface                                      | Effect                                                                                                                                                                                                               |
| -------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `diagnostics`                                | Break: the line `RIDL-150 warning lint=payload-above-ceiling default=warn` is removed. RIDL-157 is added.                                                                                                            |
| `accepted-sources`                           | Break in fact (ADR-0030 Consequences): the baseline's ros2 copy no longer compiles. The refreshed copy keeps its `ok` lines, so the line check may pass; the owner decides whether to delete the row.                |
| `runtime-crates`                             | Breaking in behaviour: `MAX_MESSAGE_LIMIT` 16,292 → 16,164, the retained minimum, the `Config` refusal, `provision_fixed`'s signature. `cargo semver-checks` may not report a const value change; the owner decides. |
| `generated-crate`                            | The veh-cluster crate changes (`MAX_SIZE` values, `verify` bodies). Expected to pass the API comparison; measure.                                                                                                    |
| `wire`                                       | Expected unchanged (bounds are not in `.fbs` or `.proto`, the frame vectors do not change); measure after the veh-cluster refresh.                                                                                   |
| `codegen-protocol`                           | Task 8 adds `Interaction.limit`: added lines only, which pass.                                                                                                                                                       |
| `manifest`, `cli`, `mcp`, `keywords`, `msrv` | Unchanged.                                                                                                                                                                                                           |

## Concurrency

PR #1127 (another session, branch `fix/podbuf-debt-1100`) changes
`crates/ridlc/src/lib.rs` (the podbuf and payload block of `check_loaded`),
`crates/ridlc/tests/payload_ceiling.rs`,
`crates/ridl-backend-rust/tests/podbuf_verify.rs` and `podbuf_conformance.rs`,
`docs/book/cli-reference.md`, ADR-0031, `docs/decisions/README.md` and the
family overview. If it lands first, rebase `feat/size-rules` before task 3 and
task 5 start. If it lands mid-task, the implementer rebases before the review.

## Open questions, with proposed rulings

- **R-1. RIDL-151 or RIDL-157 when a `[limits.kind]` entry or `default` gives
  the limit.** Decision 17 as amended says RIDL-157 applies where no
  `[limits.interaction]` entry gives the limit; decision 14 says RIDL-151
  applies where an entry gives it; both could fire on one interaction, which
  decision 14 forbids. Default: RIDL-151 whenever any entry resolves the limit
  (its value is at most the kind's ceiling, so it is the stricter check);
  RIDL-157 only when no entry applies.
- **R-2. What the IR records for a clamped `default`.** Default: the clamped
  value (for example 3,840 for a signal under `default = 4096`), because an
  entry applied (decision 13). Alternative: record nothing when the effective
  limit equals the kind's ceiling, so the hash covers only limits that differ
  from the platform's.
- **R-3. The reserved lint name.** Default: `RETIRED_LINT_NAMES: &[&str]` in
  `ridl-core/src/diag.rs` beside `RETIRED_RIDL_CODES`, with a guard test; the
  MANI-010 message is unchanged.
- **R-4. Names of the `ridl-rt` ceilings.** Default: in `ridl_rt::contract`,
  `SIGNAL_EVENT_CEILING = 3_840`, `CALL_CEILING = 16_128`,
  `FIXED_CEILING = 65_280` (all `usize`), and
  `pub const fn default_ceiling(kind: Kind) -> usize`.
- **R-5. How the loopback learns a member's limit.** It holds no member table
  and depends on `ridl-rt` only (ADR-0020 decision 6). Default:
  `Loopback::set_limit(iface, ord, limit)` on the shared store, used by every
  handle; without it, the kind's default from the port. `provision_fixed`
  returns `Result<(), WriteError>` (refusal `TooLarge { cap }`) and stores
  nothing on refusal. No receive check: the loopback is not a binding.
- **R-6. `ridl-rt-conformance`.** Default: add the kind-default cases (a signal
  of 3,841 bytes → `WriteError::TooLarge { cap: 3840 }`, and the same for each
  sending port). This binds runtimes outside this repository in the same
  breaking release.
- **R-7. Where the `Config` is refused.** The `attached` frame has no reason for
  a configuration error, and a new reason would change the frozen `wire`
  surface. Default: `serve` checks before the opening handshake, closes the
  connection, records a `Note` naming the member and the size needed, and
  returns a new `Ended` variant (`Ended` is `#[non_exhaustive]`). Without a
  member table there is no check.
- **R-8. A fixed above 16,128 bytes within its limit, until
  driftsys/ridl#1129.** The ADR says only that such a fixed cannot be served
  over WebSocket. R-8 (amended, 2026-10-30): the session records `ValueTooLarge`
  and sends no `publish`, nothing is refused at attach, and the consumer's
  `read` is answered with an `answer` carrying the read's identity and
  `contract(UnknownInteraction)`, so that the read ends with an error instead of
  waiting until the session ends. That is the one refusal §6.3 of the frame
  specification defines for a `read`, the consuming runtime already maps it to
  `ReadError::Contract(UnknownInteraction)`, and it is the same answer a
  consumer gets for a fixed with no stored value (the loopback reads a fixed
  refused above its limit as unprovisioned): no new wire value, no new code, no
  new reason. The earlier default, a read that stays pending, is withdrawn
  because the read must end.
- **R-9. The WebSocket send side.** `ridl-transport-ws` implements no sending
  port. Default: the session refuses a value longer than its member's limit as
  it refuses one above the message cap today (no frame and a note; a reply
  `corrupt`).
- **R-10. A request with no member table.** Default: the kind's default of the
  frame's `kind`, 16,128 for a command and a query, which equals what the
  default message limit carries.
- **R-11. The TS consumer's limits.** Adopted from the coordinator (R-B1): from
  the generated codec module, not from a hand-written option.
- **R-12. How the TS backend learns a limit.** `generate_codec` reads only the
  codegen `Model`, which has no limit. Default: add `optional uint32 limit` to
  `codegen.v1.Interaction` (an added field; it lands in this breaking release
  and gives out-of-tree plugins the same value). Alternative: pass a limit map
  from `ridlc` to `generate_codec`, with no protocol change.
- **R-13. Parity of the TS constants.** Default: a Rust test that reads
  `packages/ridl-transport-ws/src/frame.ts` and compares the three values with
  `ridl-rt`.
- **R-14. Bounding ros2.** Default: bounded types local to the affected request,
  response and feedback structs, the six `RosString` declarations unchanged,
  each bound the largest multiple of 64 (strings) or the largest count (arrays)
  that fits; each change recorded in `PROVENANCE.md`.
- **R-15. veh-cluster.** Default: the largest `N` for `FaultPage.faults` that
  fits 16,128 bytes in all three encodings.
- **R-16. The TS fixture.** Default: shrink the three members to fit 3,840
  bytes; use a `[limits.interaction]` raise instead only if a test needs a large
  payload.
- **R-17. Fixtures that draw RIDL-157 after task 3** (for example
  `proto3_unbounded_call.ridl`, which may test the generate refusal of a type
  with no finite bound). Default: keep each test's intent; where it needs an
  unbounded type, the type is carried by no interaction (decision 3 as amended:
  only such a type meets the refusal).
- **R-18. `ridl_core::limits::PAYLOAD_CEILING`.** Default: keep the name, value
  65,280, the largest payload of any kind, as a rule of the catalog (decision
  2). `ridl-core` is not a frozen crate.
- **R-19. How the TS client reports a refused publish or occurrence.** Default:
  an added optional field `detected: "corrupt"` on `Sample` and `Occurrence`,
  with `payload: null` on a sample and an empty payload on an occurrence.
- **R-20. TS send side.** Default: no local check in this lane.
- **R-21. Where the ros2 bounds were put.** Decided during Task 1, and it
  departs from R-14. The bounds are on the shared message types
  (`stdmsgs.Header.frameId`, `navmsgs.OccupancyGrid.data`, `navmsgs.Path.poses`,
  `sensormsgs.CameraInfo.distortionModel` and `.d`, `nav2msgs.Costmap.data`, and
  `CostmapMetaData.layer`), not on local copies, because local copies would
  duplicate `Header`, `PoseStamped` and `Path`. Every message that holds one of
  these types inherits the bound. `PROVENANCE.md` of the corpus records each
  change.
- **R-22. Appendix A of the language reference.** Decided during Task 9.
  Appendix A of the ridl language reference and the `ridl-syntax` test data
  (`appendix_a_full_example.ridl`) declare `[FaultEvent; 0..14]` in place of
  `[FaultEvent; 0..64]`, because 14 is the largest count that fits the
  16,128-byte default ceiling of a query in FlatBuffers, proto3 and podbuf. The
  veh-cluster corpus already used 14.
- **R-23. The MANI-022 message for `default` and `[limits.interaction]`.** The
  message reads "... above what the entry may give (65280 bytes)", without "for
  its kind", because 65,280 bytes is not the ceiling of a kind. It is the
  largest value any entry may give.
- **R-24. Commit trailers.** Every commit of this lane ends with the one line
  `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- **R-25. A `[limits.interaction]` value that is too large for its member.** A
  value from 16,129 to 65,280 bytes on a member that is not a fixed draws
  MANI-022 and gives no limit. Resolution continues to the `[limits.kind]`
  entry, then to the `default` entry clamped to the kind's ceiling, then to no
  limit, which RIDL-157 reports. This is the same drop-and-continue rule as the
  manifest reader.
- **R-26. `proto3_unbounded_call.ridl`.** The fixture
  `crates/ridl-backend-rust/tests/fixtures/proto3_unbounded_call.ridl` keeps an
  interaction with an unbounded payload. A test helper compiles it and requires
  RIDL-157 and no other error. The test guards the backend API against IR that
  the checker did not check or that a plugin produced. This departs from the
  default of R-17.
