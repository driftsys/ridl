# `ridl-transport-ws` reads the catalog descriptor — design note

Working memory for driftsys/ridl#985, which is also the planned fix for
driftsys/ridl#984 and settles or scopes driftsys/ridl#986. Gardened into
[`../design/ridl-transport-ws.md`](../design/ridl-transport-ws.md) and the frame
specification when the work lands.

## The problem

The server holds only the `CatalogRef` its backend's `Attached::catalog()`
returns: a unit name and a hash. Three rules of the frame specification need the
member table of the catalog, and the server cannot apply them:

- §6.1 and conformance item 1 of §12: accept an `attach` naming an earlier hash
  the descriptor's `compatible` list holds. Today `Session::attach`
  (`crates/ridl-transport-ws/src/server/session.rs`) compares the hash for
  equality only.
- §4 and §6.4: refuse a `subscribe`, `read` or `request` naming an
  `(interface, ordinal)` the catalog does not hold, or holding it under another
  `kind`, with `contract(UnknownInteraction)`. Today the server checks only that
  the form admits the kind (design record D-12) and forwards the rest to the
  backend; `ridl-loopback` answers any key with `Init`, so a client grows the
  session's `signals` map without limit (driftsys/ridl#984).
- §7 and §11.1 item 6: keep a settled response until the request's response
  bound has passed. Today `Calls::settle` (`server/calls.rs`) keeps the last 64
  (`RETAINED`), a stated deviation (driftsys/ridl#986).

The descriptor holds everything needed: per interface its `number`, per member
its `ordinal`, `kind` and `timing` (`max_us` is the response bound of a command
or a query, ridl reference §9.3), the `retired` list, `reserved_ordinals` and
`compatible` (`crates/ridl-descriptor/schema/catalog.fbs`).

## 1. How the embedder supplies the descriptor

**Recommendation: a provided method on `Backend`, with an adapter.**

- A new public owned type `MemberTable` in `ridl-transport-ws` (name from §6.4's
  "member table"): the unit name, the catalog hash, the `compatible` hashes, and
  a map from `(InterfaceNo, Ordinal)` to `(Kind, Option<Duration>)`, the
  duration being the member's `max`. Built by hand through a constructor and an
  insert method, or from descriptor bytes (section 2). A retired interface and a
  reserved ordinal have no entry, which is what §6.4 asks.
- `Backend` gains a provided method `fn members(&self) -> Option<&MemberTable>`
  that returns `None`. `Backend` is a trait with provided methods already
  (`scannable`, `read_fixed`); adding one is not a breaking change for an
  implementer, and `Polling` and `Scanning` keep returning `None`.
- A third adapter, `Described<B: Backend>`, wraps any backend with an
  `Arc<MemberTable>` and forwards `ports`, `scannable` and `read_fixed`. Its
  constructor compares the table's name and hash with `ports().catalog()` and
  returns an error on a mismatch, so a wrong table is found in the embedder's
  thread before `serve` runs, and no new `Ended` variant is needed. The example
  `crates/ridl-transport-ws/examples/loopback_server.rs` and
  `examples/cabin/server/src/main.rs` become
  `Described::new(Scanning(rt.attach()), table)`.
- **Optional.** Without a table the server behaves exactly as today, and the
  documented limits stay: `serve`'s rustdoc, D-11 to D-13 and D-16, the frame
  specification's as-built note. The rustdoc of `serve` gains one paragraph:
  with a table, per-session signal and event state is bounded by the catalog's
  member count; without one it is not.

**Alternatives considered.**

| Alternative                                             | Verdict  | Why                                                                                                                                                                                                                                     |
| ------------------------------------------------------- | -------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| a `Config::with_member_table` setting                   | rejected | `Config` is per server and knows no backend, so the name-and-hash check against `ports().catalog()` could run only inside `serve`, where the only ways to report a mismatch are a panic on a connection thread or a new `Ended` variant |
| a third parameter of `serve`                            | rejected | changes a frozen signature (`runtime-crates` surface, ADR-0029) for what a trait method gives without a break                                                                                                                           |
| required: `Backend::members` with no default            | rejected | every embedder would have to deploy a descriptor file, including `ridl-loopback` tests; the issues ask for the limits to stay documented, not for a break                                                                               |
| the table on `ridl-rt` (`Attached` returns the members) | rejected | `ridl-rt` has no dependency and its descriptors are `'static` generated constants (`contract::Member`); an owned table built at run time does not fit, and the change would reach every runtime                                         |

## 2. The dependency cost

**Recommendation: an optional feature `descriptor`, off by default, that adds
`ridl-descriptor` with `default-features = false`.**

- The feature is `descriptor = ["server", "dep:ridl-descriptor"]`. It adds
  `MemberTable::from_descriptor(bytes: &[u8]) -> Result<MemberTable, DescriptorError>`,
  which calls `ridl_descriptor::verify`, then walks `interfaces`, `members`,
  `timing` and `compatible` once into the owned table. `DescriptorError` wraps
  `VerifyError`, a `max_us` string that is not a decimal `u64`, and a `number`
  or `ordinal` of 0. The crate does no I/O: the embedder reads
  `<unit>.catalog.binfb` (written by `ridl build --emit catalog`) or embeds it
  with `include_bytes!`.
- With `default-features = false`, `ridl-descriptor` is `no_std` with `alloc`:
  planus and serde (`alloc`, `derive`) only, no `ridl-ir`, no `serde_json`, no
  `hashbrown`/`foldhash` (the `dedup` caches, and their Zlib licence, are a
  builder's concern). That is the smallest tree that reads a descriptor.
- MSRV: `ridl-descriptor` pins `planus = 1.3.0`, whose minimum is 1.88, while
  `ridl-transport-ws` states 1.83. The manifest keeps `rust-version = "1.83"`;
  the feature needs 1.88 and the rustdoc and the design record say so. An
  optional dependency is resolved but not compiled when its feature is off, so
  the crate still builds at 1.83 with default features and with none. The msrv
  freeze dump (`freeze/msrv.txt`) does not change.
- Gates to extend: `just compat-check` gets one more cell, the packaged
  `ridl-transport-ws` with `--features descriptor` at 1.88 against the packaged
  `ridl-descriptor` the recipe already extracts; its list of allowed workspace
  dependencies gains `ridl-descriptor`. `tests/constraints.rs` changes from "the
  only dependency is `ridl-rt`" to "the only normal dependency is `ridl-rt`; the
  one optional dependency is `ridl-descriptor` with default features off and
  under the `descriptor` feature". `just wasm-check` is unchanged: the feature
  implies `server`, which needs `std`.

**Alternatives considered.**

| Alternative                                                   | Verdict  | Why                                                                                                                                                                                      |
| ------------------------------------------------------------- | -------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `ridl-descriptor` as an unconditional dependency              | rejected | raises the crate's minimum to 1.88 and puts planus in every embedder's tree, also one that passes no table; fails `compat-check` as written                                              |
| no dependency: the embedder converts the descriptor itself    | rejected | every embedder rewrites the same forty-line planus walk against the schema; the hand-built constructor stays public, so this path remains open for an embedder that must not link planus |
| the conversion in `ridl-descriptor` (`impl From<CatalogRef>`) | rejected | makes `ridl-descriptor` depend on the transport's type, the wrong direction; the conversion is the transport's                                                                           |

## 3. Behaviour with a table

**Attach (§6.1).** After the name check, the hash is accepted when it equals the
table's hash or one of its `compatible` hashes; otherwise `catalog_mismatch`.
Order of refusal reasons unchanged. Without a table the exact-match rule stays.

**Routing (§4, §6.4).** Before the form check of D-12, every `subscribe`,
`read`, `request` and `unsubscribe` looks the frame's `(interface, ordinal)` up
in the table. Absent, or present under another `kind`: a `subscribe` or `read`
is answered `answer` with `contract(UnknownInteraction)`, a `request` with a
`response` carrying it and the request's `seq` as `correlation`, an
`unsubscribe` is dropped and recorded as a new `Note::UnknownInteraction`. The
reply echoes the identity as received (D-25). Nothing reaches the backend and no
session state is created, so `signals` and `events` hold at most one entry per
signal or event member of the catalog. The D-12 form check follows, unchanged.

**Retention (§7, §11.1 item 6).** `Calls::settle` records, with each response,
the instant it stops being kept: `min(request.stamp, receipt) + max`, where
`receipt` is `Clock::now()` of the backend when the request arrived and `max`
the member's bound from the table. The `min` keeps the spec's "measured from the
request's `stamp`" when the client's clock is behind or equal, and caps the
retention at `receipt + max` when the client's stamp is ahead, so a client
cannot hold responses by stamping requests in the future (a stamp ahead of the
provider's clock is a loss of synchronization, §8). Expired responses are
removed on each tick. A repeated `seq` whose response has expired is
`DuplicateDropped`, as today. The `RETAINED` constant goes; when the table has
no bound for the member (a catalog built before the default response bound,
`Member::call_deadline` rationale in `crates/ridl-rt/src/contract.rs`), or there
is no table, the server keeps the last 64 as today and the record says so.

**What bounds per-session state afterwards.** Signal and event state: the member
count. Calls in flight: the backend's call table (`Busy` beyond it) and
`call_abandon_after`. Settled responses: the client's request rate times the
response bound, each response at most `max_message_bytes`. The last is bounded
in time but not in size: a client that sends 1000 requests per second under a 60
s bound has 60 000 responses kept. **Recommendation: yes, a cap as defense in
depth** — `Config::with_max_retained_bytes` (default 16 MiB, at least one
`max_message_bytes`), counting the retained response bytes of the session; when
a `settle` would pass it the session closes with 1008 and `serve` returns
`Ended::ServerClosed(1008)`. Closing is observable and keeps §11.1 item 6
honest; silently dropping the oldest would not. The cap is a server setting, not
a frame rule, so the frame does not change; §11.1 item 4 gains one sentence ("a
provider may close with 1008 when a session passes a resource limit it states").
Ruled yes (D-1 below).

## 4. Spec impact

- Frame specification, as-built note (lines 23 to 45): "applies the exact-match
  rule of §6.1 at `attach`, and keeps the last 64 settled responses" and the
  second paragraph ("no runtime in this workspace reads it at `attach`.
  `ridl-transport-ws` holds no descriptor ...") become: with a member table the
  crate accepts a listed earlier hash, refuses an unknown interaction and keeps
  a response until its bound; without one it applies the exact-match rule and
  keeps the last 64.
- §6.4 last sentences: add that `ridl-transport-ws` with a member table reports
  `UnknownInteraction`; the `ridl-loopback` sentence stays.
- §7 last paragraph ("`ridl-transport-ws` suppresses a duplicate `request`: it
  keeps the last 64 ..."): rewrite to the bound-based retention, the fallback
  for a member with no bound or a server with no table, and the cap if decided.
- §11.1 item 4: the 1008 sentence, if decided. Item 6: unchanged if #986 is
  ruled "stated deviation" (below).
- §12 item 1's parenthesis ("no runtime in this workspace accepts a listed hash,
  per the As-built note"): drop once the server does.
- Design record `docs/design/ridl-transport-ws.md`: "What it does not do" ("It
  reads no catalog descriptor"), constraints, D-1 table (a `descriptor` feature
  row), D-2 dependency table and gates, D-10 (`members`, `Described`), D-11
  (compatible hashes), D-12 (the table lookup), D-13 last paragraph (the bound,
  closing driftsys/ridl#984), D-16 duplicates paragraph, D-18 `Note` list and
  `Ended` table (1008), the `Config` section, "What is not built" (two bullets
  go), "Alternatives considered" (the deferred row becomes built).
- **#986.** With the default response bound (ridl reference §9.1) every command
  and query in a catalog built by this toolchain has a `max`, so the no-bound
  case is a descriptor from an older toolchain or no table at all.
  Recommendation: §11.1 item 6 stays as written, and §7's last paragraph states
  the fallback as a deviation of this crate, not as a rule of the binding. Ruled
  so (D-2 below).

## 5. Test plan

All on the sans-IO `Session` over `ridl-loopback` unless stated, with a
hand-built `MemberTable` of the scenario catalog (one signal, one event, one
command, one query, one `fixed`, one reserved ordinal). Each line names the
behaviour and the mutation that turns it red.

1. `attach` with a listed earlier hash is `accepted`; with an unlisted one,
   `catalog_mismatch`. Mutation: compare the own hash only.
2. `subscribe` of an ordinal the table lacks is answered
   `contract(UnknownInteraction)`, and a later loopback write to that key sends
   no `publish`. Mutation: skip the lookup (the loopback answers `Init`).
3. `subscribe` of the event's ordinal under `kind` `Signal` is refused the same
   way. Mutation: look the ordinal up without the kind.
4. `read` of the reserved ordinal, and `request` of the signal's ordinal under
   `Command`, are refused with the identity echoed. Mutation: route by form
   first.
5. `unsubscribe` of an unknown key sends nothing and records
   `Note::UnknownInteraction`. Mutation: drop without recording.
6. Over `Polling` and `Scanning` alike, after 1000 unknown subscribes the
   session holds no signal state (observed through a tick that reads nothing: a
   counting `SignalReader` wrapper). Mutation: insert before the lookup.
7. A response is sent again for a repeat at `stamp + max/2`, and
   `DuplicateDropped` for one at `stamp + max + 1` after the clock advanced.
   Mutations: never expire; expire at once.
8. 65 settled calls, then a repeat of the first within its bound, gets its
   response: the count cap is gone. Mutation: keep `RETAINED`.
9. A request stamped one hour ahead expires at `receipt + max` by the server's
   clock. Mutation: measure from the stamp only.
10. A member with no `max` in the table falls back to the last 64. Mutation:
    drop such responses at once.
11. The cap: responses whose bytes pass `max_retained_bytes` close the session
    with 1008, in `tests/serve.rs` over `tungstenite` as `ServerClosed(1008)`
    and the client's close code. Mutation: count frames instead of bytes.
12. `Described::new` with a table whose name or hash differs from the ports'
    catalog returns the error.
13. `from_descriptor` over a buffer `ridl_descriptor::finish` built in the test:
    a retired interface and a reserved ordinal have no entry; the kinds map 1:1;
    `max_us` becomes the bound; a malformed buffer returns `VerifyError`; a
    non-decimal `max_us` is refused. Mutation per field.
14. `tests/constraints.rs` asserts the new dependency shape; `compat-check`'s
    new cell builds the feature at 1.88. `tests/scenario.rs` runs with a table
    too, so the done-when holds under the checks.

## 6. Pull-request steps

1. **The member table and the §6.4 refusals.** `MemberTable` (hand-built only),
   `Backend::members`, `Described`, the lookup in `subscribe`/`read`/`request`/
   `unsubscribe`, `Note::UnknownInteraction`, tests 2 to 6 and 12, the `serve`
   rustdoc bound statement, D-10, D-12, D-13. Closes driftsys/ridl#984.
2. **Compatible hashes at `attach`.** Test 1; as-built note, §12 item 1, D-11.
3. **Retention by bound.** `Calls` with expiry, `RETAINED` as fallback only,
   tests 7 to 10; §7, D-16; the #986 disposition. Then the cap and test 11 in
   the same PR if D-1 is yes, with §11.1 item 4, D-18, `Config`.
4. **The `descriptor` feature.** `from_descriptor`, the manifest, test 13,
   `tests/constraints.rs`, the `compat-check` cell, ADR-0020 decision 6
   amendment, D-1 and D-2 tables. Closes driftsys/ridl#985.
5. **The examples read the file.** `loopback_server.rs` builds its table by hand
   (the scenario catalog has no descriptor); `examples/cabin/server` reads
   `cabin.catalog.binfb` emitted by `ridl build --emit catalog` in
   `just cabin-ws-demo`; test 14's scenario run; the design record's "What is
   not built" and alternatives rows.

Steps 1 to 3 need no new dependency, so each is reviewable at 1.83; step 4 is
independent of 2 and 3 and can follow 1 directly.

## Decisions

Ruled by Sebastien on 2026-10-23, each following the recommendation above. The
implementation plan is
[`2026-10-23-transport-ws-descriptor-plan.md`](2026-10-23-transport-ws-descriptor-plan.md).

- **D-1. The cap: yes.** A retained-bytes cap as a `Config` setting,
  `Config::with_max_retained_bytes`, default 16 MiB, at least one
  `max_message_bytes`. When a settled response would take the session's retained
  bytes past the cap, the session closes with 1008 and `serve` returns
  `Ended::ServerClosed(1008)`. §11.1 item 4 gains the one sentence of section 3.
  Built in step 3 with test 11.
- **D-2. #986: a stated deviation of this crate.** §11.1 item 6 stays as
  written. §7's last paragraph states the fallback (the last 64 settled
  responses, for a member with no bound or a server with no table) as a
  deviation of `ridl-transport-ws`, not as a rule of the binding. Step 3 writes
  the text; driftsys/ridl#986 closes with it.
- **D-3. ADR-0020 decision 6: amended in step 4.** The `ridl-transport-ws` row
  becomes "`ridl-rt`; `ridl-descriptor` with its default features off, under the
  `descriptor` feature", off by default, Rust 1.88 with the feature on. The
  amendment lands in the same pull request as the feature. `ridl-loopback` is
  unchanged by this work.
