# Debug Socket Implementation Plan

> **For agentic workers:** Use `superpowers:executing-plans` and the task/report
> protocol in [the handoff](2026-11-03-debug-socket-codex-handoff.md). Implement
> one task, report, then stop for the owner's Claude reviewer. Do not dispatch
> an implementer or substitute a self-review for that gate.

**Goal:** Give a loopback machine descriptor-based attachments, a debug
listener, and a twin that can provide signals, events and call responses over a
separate WebSocket connection.

**Architecture:** A feature-gated node owns one loopback store per machine
catalog and returns restricted attachment wrappers. Existing consuming WebSocket
sessions gain backend selection; a separate version-2 session consumes a remote
provider. The gateway is example/test support over these APIs, not a CLI
service.

**Tech stack:** Rust, existing port traits and descriptor accessors, the
blocking WebSocket server, Deno/browser TypeScript, existing tungstenite tests.
No new external dependencies.

**Spec:** [Debug socket design](2026-11-03-debug-socket-design.md), including
owner answers Q1–Q3; [handoff](2026-11-03-debug-socket-codex-handoff.md).

## Global constraints

- Phase 1 creates only this plan. The design header still says draft; this plan
  does not change its status or constitute approval to implement it.
- PR A, B and C are sequential deliveries, each starting from a clean base
  containing its predecessors. Do not implement C concurrently with WebSocket
  parameter streams. Re-read the transport interfaces on C's actual base.
- `debug` is off by default. `ridl-rt/debug` implies nothing else and works
  without `std`. Generated instance constants remain positive `i32` constants.
- Rust 1.88 remains the minimum; runtime crates remain edition 2021, descriptor
  edition 2024. Preserve every existing API, existing `serve` behavior and all
  frame layouts.
- The listening and injection listeners bind separate ports on `127.0.0.1`. No
  injection listener means no injection. Only the gateway interprets deployment
  URL paths.
- The reversed subprotocol is `ridl-ws-v2-provider`; no reversed streams. The
  WebSocket server still sends pings.
- No production per-interface write ownership or redundant-provider arbitration
  is added. Tests of injection use one instance per component.
- No merge, push, PR creation, broad refactor, dependency installation or
  `just build`. Stop for an impossible design, compatibility break, an unrelated
  test failure, or any dependency not explicitly required by the design.
- Preserve the existing worktree and other work. Do not use `git stash`. The
  phase-1 base is `f0b6847b` on `docs/debug-socket-design`; record a fresh full
  base hash before each implementation task.

## Names and interfaces

These are the names chosen here; subsequent tasks refer to this section.

- `ridl_rt::contract::InstanceNo(i32)` has a private field, derives `Clone`,
  `Copy`, `Debug`, `PartialEq`, `Eq`, `PartialOrd`, `Ord`, `Hash`, and exposes
  `pub const fn new(i32) -> Option<Self>` and `pub const fn get(self) -> i32`.
  With `debug`: `pub const fn twin(self) -> Option<Self>`,
  `pub const LISTENER: Self`, `pub const fn is_twin(self) -> bool`,
  `pub const fn is_listener(self) -> bool`.
- `ridl-loopback` features: `node = ["dep:ridl-descriptor"]`,
  `debug = ["node", "ridl-rt/debug"]`. The optional descriptor dependency uses
  the workspace's defaults-off entry. Default loopback dependencies remain as
  today.
- `ridl_loopback::Node`, `Attachment`, `NodeError`, `AttachError` are exported
  only with `node`.
  `Node::from_descriptor(bytes: &'static [u8], machine: &str) -> Result<Self, NodeError>`,
  `Node::attach(&self, who: InstanceNo, catalog: &CatalogRef) -> Result<Attachment, AttachError>`,
  `Node::catalogs(&self) -> impl Iterator<Item = &CatalogRef>`. The node uses
  FlatBuffers, the loopback default. Other encodings remain available on the
  existing `Loopback` API.
- `NodeError` and `AttachError` are new non-exhaustive error enums implementing
  `Display` and `Error`. Construction distinguishes invalid descriptor and
  unknown machine. Attachment distinguishes unknown instance, unknown catalog,
  catalog not granted and identity conflict. Preserve descriptor verification as
  an error source.
- `Attachment` implements `Attached`, `Encoded`, `Clock`, `SignalReader`,
  `SignalWriter`, `EventSource`, `EventSink`, `Caller`, `Handler`,
  `FixedReader`, `ScannableSignals`, `CoherentSignals` and `Wakeable` by checked
  delegation. It exposes no raw loopback, split, clone or role-handle escape.
  The wrapper itself is the identity suite's runtime type.
- `ridl_rt_conformance::identity::IdentityFactory: Factory` adds associated
  `Node`, `Attachment` (the identity group's base-port bounds), and
  `Error: Debug`; `fn node(bytes: &'static [u8], machine: &str) -> Self::Node`,
  `fn attach(node: &Self::Node, who: InstanceNo, catalog: &CatalogRef) -> Result<Self::Attachment, Self::Error>`.
  Existing `Factory` does not change. New `suite!` extension: `identity`; debug
  cases are emitted through a separate `identity_debug` arm enabled by the
  conformance crate's `debug = ["ridl-rt/debug"]` feature.
- `server::serve_selected<C, B, S>(conn: C, max_version: u32, select: S, config: &Config) -> Ended`,
  where `C: Connection`, `B: Backend`, `S: FnOnce(&CatalogRef) -> Option<B>`.
  `max_version` supports 2 or 3; selected version 1 retains the current explicit
  attach refusal. Unsupported capability input returns `ConfigRefused` before
  the handshake. Selection occurs once after parsing attach, before normal
  catalog/encoding checks.
- `server::ProvidedBackend` has
  `type Ports: Attached + Encoded + Clock + Handler`,
  `fn ports(&mut self) -> &mut Self::Ports`, and
  `fn members(&self) -> &MemberTable`. Its table contains only the offered
  members of this instance in this catalog. Under R-DS-3 its delivery methods
  are `sample(target: Target, sample: Sample<Option<&[u8]>>, config: &Config)`,
  `occurrence(target: Target, occurrence: Occurrence<&[u8]>, gap: Option<u64>, config: &Config)`
  and `commit(remote_stamp: Timestamp)`, each with `&mut self` and no return
  value. These reuse the runtime's sample, provenance, detection, freshness and
  envelope types. A sample's `None` means the catalog init value; `Some` holds
  the last good bytes. `gap` counts missing event occurrences.
  `server::Providing<P>::new(ports: P, members: MemberTable) -> Result<Self, TableMismatch>`
  is the standard adapter, validating table identity.
- `server::serve_provided<C: Connection, B: ProvidedBackend>(conn: C, backend: B, config: &Config) -> Ended`;
  `server::serve_provided_selected<C, B, S>(conn: C, select: S, config: &Config) -> Ended`,
  where `S: FnOnce(&str) -> Result<B, SelectionError>`.
  `SelectionError::{NotFound, Conflict}` maps to HTTP 404/409 before upgrade.
  Fixed-backend form ignores path. Both accept only the reversed subprotocol.
- Add
  `MemberTable::iter(&self) -> impl Iterator<Item = (InterfaceNo, Ordinal, Kind)>`
  for deterministic member discovery. Keep existing entry metadata and accessors
  intact.
- TypeScript:
  `provide(url: string, options: ProvideOptions): Promise<Provision>` in
  `src/provision.ts`, exported by `mod.ts`. `ProvideOptions` contains
  `catalog: ConnectOptions["catalog"]`, `encoding: EncodingName`,
  `members: readonly MemberEntry[]`, and
  `onRequest(request: ProvidedRequest): Promise<ProvidedResult> | ProvidedResult`,
  and optional `webSocket: ConnectOptions["webSocket"]` for socket injection.
  Reuse existing types from `session.ts` and `frame.ts`, with explicit type
  imports and re-exports rather than duplicate definitions.
- `Provision` exposes
  `publish(iface: number, ord: number, payload: Uint8Array, stamp: bigint): void`,
  `invalidate(iface: number, ord: number, stamp: bigint): void`,
  `raise(iface: number, ord: number, payload: Uint8Array, stamp: bigint): void`,
  `close(): void`, and `closed: Promise<void>`. `ProvidedRequest` carries
  target, encoding-independent argument bytes, envelope and correlation;
  `ProvidedResult` is `{ outcome: ResponseOutcome; payload: Uint8Array }`,
  reusing the frame's outcome representation. Responses preserve the request
  correlation and use a provider response sequence counter.

## Rulings

Ruling: `twin` returns `Option`; positive values negate safely, while twins and
the listener return `None`. No public unchecked constructor or deserializer
creates arbitrary negative values.

Ruling: `node` makes descriptor support optional; `debug` implies `node`. The
descriptor edge explicitly required by design §4.1 is the only authorized
dependency addition. Any further dependency requires the handoff's stop/report
process.

Ruling: Attachments use private checked delegation, not flags on the public
`Loopback`. This preserves existing behavior and prevents bypassing a listener's
permissions. One attachment owns one reservation; it cannot manufacture
uncounted handles.

Ruling: Node identity counts are under one mutex across all catalogs. Check
conflict and increment atomically; decrement on drop. Failed selections and
failed handshakes must release their attachment. Dropping `Node` does not
invalidate live attachments or their reservation state.

Ruling: Catalog enumeration is deterministic by name and hash, with duplicates
removed. A grant derives from component offers/maps, joined through placed
instances and embedded catalog identity; a name match with a different hash is
not a grant.

Ruling: The identity fixture is a committed descriptor plus its RIDL workspace
sources in the conformance crate. It is regenerated by the existing compiler,
not by a new conformance dependency. The fixture has two local instances, two
local catalogs, one mapped catalog, and a second machine with an
instance/catalog to prove placement filtering.

Ruling: The gateway accepts
`debug_gateway <descriptor> <machine> <listen-port> [inject-port]`; port 0
requests an ephemeral port. PR B accepts only the listening arguments; PR C adds
the optional injection argument. Shared implementation lives in
`examples/support/debug_gateway.rs`, imported by tests using `#[path]`, and is
not exported from the library.

Ruling: Injection paths have exactly three nonempty literal segments; reject
query strings, percent escapes, trailing slashes and additional segments
with 404. Qualified names use their descriptor spelling. This avoids inventing a
second name normalization scheme. Unknown/off-machine/not-offered selections
give 404; a live positive counterpart gives 409.

Ruling: Same-stamp signal publications stage in one writer transaction. Flush on
a stamp change before processing the new publication, or one
`Config::poll_interval` tick after the last publication in the group. Equal
stamps after a flush begin a new transaction. An event does not flush signals.
Drop an incomplete staged group on session termination; do not publish after
disconnect. Node clock, not client stamp, supplies committed envelopes.

Ruling: The injection end-to-end acceptance test uses the actual TypeScript
`provide` API against a Rust fixture server. Rust tungstenite tests additionally
isolate transport failures. This proves interoperability rather than only two
local implementations agreeing internally.

Ruling: A reversed provider supports signal/event subscriptions and
command/query responses in this MVP. Reject `fixed` reads with the existing
unknown-interaction result and reject stream forms with 1002. The offered member
table may include fixed metadata, but no new fixed provisioning API is invented.

## Review focus

1. A live twin conflicts across catalogs until its last attachment drops; A3
   tests both orders, duplicates, failed grants and concurrent acquisition.
2. Listener methods and wrapper forwarding cannot leak write access or claim
   calls; A3 tests every writer operation and call isolation.
3. A selector cannot let an incompatible catalog, encoding, frame version or
   member limit reach the session; B1 tests each mismatch, including cleanup.
4. Bad reversed input cannot publish before subscription, replay an event, leave
   half a commit, or retain a call indefinitely; C3/C4 cover these cases.
5. A refused or disconnected provider cannot reserve a twin permanently, and a
   listening socket cannot accept injection; C1/C6 exercise handshake and
   teardown paths.

## Task protocol and common checks

For every task, in order:

- [ ] Record full `git rev-parse HEAD`; write the named tests and keep their
      failing output. Compile-only red is accepted only for a new API. For
      existing behavior require an assertion failure.
- [ ] Implement the specified behavior with the minimum change. Keep non-test
      changes below roughly 600 lines; stop and split the task before exceeding
      this. The size figures below are estimates, not permission to skip a gate.
- [ ] Run the task's listed checks. The reviewer runs each test's mutation
      separately and restores the original bytes afterwards.
- [ ] Commit one logical change with an allowed Conventional Commit scope, no
      breaking marker and no trailer; write
      `.superpowers/codex/debug-socket-task-<N>-report.md` with base, commits,
      red output, behavior/test mapping, mutation mapping, commands/results, and
      rulings.
- [ ] Stop. Await both Claude verdicts before the next task. At the last task of
      each PR, report that the implementation is ready for branch review and
      gardening, then stop.

`R(crate, features)` below means these exact commands at the worktree root,
using the specified feature arguments on test and clippy:

```sh
RUSTC_WRAPPER= cargo test -p <crate> --locked <feature-arguments>
RUSTC_WRAPPER= cargo clippy -p <crate> --all-targets --locked <feature-arguments> -- -D warnings
cargo fmt --all --check
```

Use `--offline` only when the existing cache suffices. Report environment
blockers; do not alter socket tests to avoid a bind failure. `T` means, in
`packages/ridl-transport-ws`, `deno fmt --check`, `deno lint`, and
`deno test --allow-read tests/`. Individual new socket e2e tests additionally
receive the permissions stated below. Formatting/linting is part of each
implementation diff. Full CI remains the PR owner's responsibility.

## PR A — identities and descriptor attachments

Start from a clean base containing the approved design. This PR delivers design
parts 1–2; B/C code is absent.

### Task 1: Represent instance identities and prove production exclusion

**Files:** modify `crates/ridl-rt/src/contract.rs`, `crates/ridl-rt/Cargo.toml`,
`justfile`; add `crates/ridl-rt/tests/instance_no.rs`. Include the gated
compile-fail examples in `contract.rs`.

**Behaviour:** Implement the identity API from Names. Add a separate
default-feature `ridl-rt` test invocation to the existing `test` recipe; retain
its all-features invocation. No new gate recipe or workflow command is needed.

**Tests first / reviewer mutations:**

| Test                                           | Assertion / expected red                                                                              | Mutation it must catch                   |
| ---------------------------------------------- | ----------------------------------------------------------------------------------------------------- | ---------------------------------------- |
| `positive_instance_bounds`                     | 1 and MAX round-trip; 0, -1 and MIN return None; new API may initially fail compilation               | Accept `n >= 0`                          |
| `debug_identity_partition`                     | Twins of 1/MAX are -1/-MAX, listener is MIN; predicates are disjoint; twin/listener cannot twin again | Return a twin for every nonzero value    |
| `listener_is_absent_without_debug` doctest     | Naming LISTENER fails without debug                                                                   | Remove cfg from LISTENER                 |
| `twin_is_absent_without_debug` doctest         | Calling twin fails without debug                                                                      | Remove cfg from twin                     |
| `predicates_are_absent_without_debug` doctests | Each predicate is unavailable without debug                                                           | Remove cfg from the predicate under test |

**Review focus:** Private field; no negative construction through trait
implementations; const usability; no feature unification hides the negative
controls. Reviewer must observe compile-fail examples fail their test when the
item becomes available.

**Checks:** `R(ridl-rt, --no-default-features)`, `R(ridl-rt, --features debug)`;
explicit `cargo test -p ridl-rt --no-default-features --doc --locked` reports
the named compile-fail tests executed;
`cargo check -p ridl-rt --no-default-features --features debug --target thumbv7em-none-eabihf --locked`;
`just gate-parity`. Read CI's existing `just test` invocation to confirm this
isolated run is reached.

**Size:** 100–180 non-test lines including rustdoc and recipe.

### Task 2: Forward the generated debug feature

**Files:** modify `crates/ridlc/src/lib.rs` (`render_cargo_toml`),
`crates/ridlc/tests/rust_crate_emit.rs`, `freeze/generated/cabin/Cargo.toml`,
`freeze/generated/veh-cluster/Cargo.toml` through regeneration. No backend
emitter change is required: `ridlc` owns the manifest.

**Behaviour:** Every generated Rust crate has `debug = ["ridl-rt/debug"]`;
default features and instance constants remain as before.

**Tests first / reviewer mutations:**
`generated_debug_forwards_only_runtime_debug` parses the manifest and asserts
that exact one-element list, and that default omits debug (mutation: add debug
to defaults or std to its list). `generated_instances_remain_positive_i32`
asserts existing generated constant declarations and positive values (mutation:
render an InstanceNo constant). Add a generated consumer compile probe naming
LISTENER and twin: default fails, debug succeeds, with no-default-features plus
debug also succeeds (mutation: emit empty debug forwarding). New API compile red
is acceptable for this probe, but manifest test must fail an assertion first.

**Review focus:** Test actual generated artifacts, not only a string template.
Keep all existing generated public items. Regenerate frozen manifests with
`cargo xtask freeze update`, never edit dumps to pass.

**Checks:** `R(ridlc, no extra features)`; run the three generated consumer
probes; `just freeze-check` after regeneration. Review all generated diffs and
require only additive feature changes.

**Size:** under 80 non-test lines plus generated manifest additions.

### Task 3: Build the node and enforce grants and lifetime

**Files:** add `crates/ridl-loopback/src/node.rs`,
`crates/ridl-loopback/src/node/attachment.rs`,
`crates/ridl-loopback/tests/node.rs`; modify `crates/ridl-loopback/src/lib.rs`,
`crates/ridl-loopback/Cargo.toml`, `Cargo.lock` if Cargo requires it. Add the
fixture sources and descriptor under
`crates/ridl-rt-conformance/fixtures/identity/` for reuse in Task 4. Document
its regeneration command in a fixture README.

**Behaviour:** Verify before reading; select the machine, build deduplicated
catalog stores and grants, expose the wrapper from Names. Listener
set/invalidate/raise/serve return the specified NotOwner errors and mutate
nothing; commit is harmless. Listener `next_claim` never exposes a claim even
though an unconfigured raw loopback handler sees all calls. Read, scan,
subscribe, fixed read and caller delegation remain usable. Reserve/release
identities atomically across catalogs. No general write ownership check.

**Tests first / reviewer mutations:**

| Test                                                | Assertion / expected red                                                                                                    | Mutation it must catch                              |
| --------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------- |
| `node_verifies_descriptor_and_machine`              | Corrupt embedded catalog/descriptor and absent machine are errors                                                           | Skip verifier or accept absent machine              |
| `node_catalogs_follow_local_offers_and_maps`        | Exact local union, deduplicated; remote-only catalog excluded                                                               | Build stores from all embedded catalogs             |
| `positive_grants_use_name_and_hash`                 | Local offered/mapped catalogs attach; unrelated catalog, foreign instance and altered hash fail                             | Grant by catalog name alone or skip placement check |
| `twins_receive_the_same_grants`                     | Twin can read/write in each granted catalog, unrelated catalog still refused                                                | Give twin every machine catalog                     |
| `opposite_identities_exclude_in_both_orders`        | N then -N, and -N then N, fail across different catalogs                                                                    | Check counts only for requested catalog             |
| `last_attachment_releases_identity`                 | Two equal identities allowed; dropping one still excludes opposite; dropping last permits it; failed attach leaves no count | Remove identity entry on first drop                 |
| `simultaneous_opposites_have_one_winner`            | Barrier-started opposite attaches yield exactly one success, keeping winner live until both results collected               | Split conflict check from increment                 |
| `listener_refuses_every_write_without_side_effects` | Set, invalidate, raise and serve each return NotOwner; existing signal/event/call state unchanged after commit              | Remove each individual method's listener check      |
| `listener_reads_and_calls_but_never_claims`         | Listener observes both catalogs, subscriptions/scan work; command/query reach a positive provider; listener cannot claim it | Delegate listener next_claim to raw handler         |
| `attachments_survive_node_drop`                     | Ports continue to read/write after Node is dropped                                                                          | Store borrowed node state in Attachment             |

Tests for the new node API may first fail compilation. Once the API exists,
preserve assertion-red evidence for the permission, exclusion and lifetime
controls by disabling the corresponding check before implementing it. Use a
separate case per refusal operation so one early error cannot hide the rest.

**Review focus:** No raw handle escape, no unsafe code, poison handling matches
existing mutex style, exact wrapper forwarding signatures and error precedence,
no lock held while delegated ports wake tasks. A disabled feature must remove
the descriptor dependency. Fixture binary must independently verify and match
its committed sources.

**Checks:** `R(ridl-loopback, --features debug)`,
`R(ridl-loopback, --features node)`;
`cargo test -p ridl-loopback --no-default-features --locked`;
`cargo tree -p ridl-loopback --no-default-features` contains no descriptor;
`cargo tree -p ridl-loopback --features node` shows descriptor without compiler
features; `just freeze-check`.

**Size:** about 450–580 non-test lines; keep descriptor/grant logic in node.rs
and delegation in attachment.rs. If above 600, split construction/grants from
debug restrictions with a report/review gate between them.

### Task 4: Publish the opt-in identity conformance group

**Files:** add `crates/ridl-rt-conformance/src/identity.rs`; modify
`crates/ridl-rt-conformance/src/lib.rs`, its macro coverage test and manifest,
`crates/ridl-loopback/tests/conformance.rs`, loopback dev-dependency feature
configuration, and both crates' API documentation.

**Behaviour:** The extension factory shares one node between attachments.
Existing factories/macros compile unchanged. The descriptor fixture is included
from the conformance package, with no compiler dependency. Identity-debug tests
are selected in the conformance crate, not using an external caller's unrelated
`debug` cfg. Loopback's integration test enables them deliberately.

**Tests first / reviewer mutations:** `identity_positive_grants` refuses
unknown/off-machine numbers and unrelated catalogs (mutation: adapter returns
unrestricted loopback). `identity_twin_grants` compares positive/twin grant sets
(mutation: grant all). `identity_exclusion_both_orders` and `identity_last_drop`
repeat cross-catalog controls through the extension hook (mutations: skip one
conflict direction; release on first drop). `identity_listener_all_catalogs`
reads a distinct published byte from each catalog (mutation: fixture adapter
always returns first catalog). `identity_listener_write_refusals` asserts each
NotOwner independently (mutation: permit each writer method in turn).
`identity_listener_can_call` completes a command and query while refusing
listener serving (mutation: deny Caller to listener). Existing macro inventory
test must include both new arms (mutation: remove any one test from an arm).
`existing_factory_needs_no_identity_hook` is a minimal old-style compile
consumer (mutation: add an identity requirement to Factory or default suite
expansion).

**Review focus:** An external runtime can implement the hook without depending
on loopback or loopback errors. The test factory translates refusal into Result
but suite assertions test observable permissions, not concrete error enum
spelling. Debug tests actually execute, with a test-name inventory in the
report.

**Checks:** `R(ridl-rt-conformance, --features debug)`,
`R(ridl-loopback, --features debug)`; default-feature conformance tests and old
factory consumer; `cargo package -p ridl-rt-conformance --list` includes fixture
bytes; `just freeze-check`.

**Size:** under 180 non-test lines; most additions are tests.

**PR A gardening:** `docs/design/ridl-rt.md` (identity/features),
`docs/design/deployment-descriptor.md` (node/grants),
`docs/design/ridl-loopback.md` (wrapper and remaining ownership limits),
conformance README. Record partial delivery in `docs/ROADMAP.md`; do not mark
all of E19.7 landed yet. Claude owns branch review and gardening under handoff
§4.

## PR B — listening through the gateway

Start from clean merged PR A. No reversed-role implementation is required for
this PR.

### Task 5: Select the consuming client's backend after attach

**Files:** modify `crates/ridl-transport-ws/src/server.rs`,
`src/server/session.rs`, `tests/serve.rs`, `tests/scenario.rs`; add
`tests/selected.rs` with a server-required test target in the manifest.

**Behaviour:** Implement serve_selected with explicit pre-handshake capability.
Parse one attach, invoke selector once, preserve D-11 checks. Return
unknown_catalog on None. Refuse a selected backend lacking negotiated capability
or sufficient message capacity as ConfigRefused before delivering session data.
Existing serve retains pre-handshake capacity checking and its signature; share
internal code only where this preserves the timing.

**Tests first / reviewer mutations:** `selected_routes_two_catalogs` uses
different stored bytes and observes correct routing (mutation: always choose
first). `unknown_catalog_calls_selector_once_and_refuses` counts selection and
asserts refusal (mutation: fall back to a backend).
`selection_preserves_attach_checks` independently tests changed hash,
unsupported encoding and mismatched frame version (mutation: bypass
corresponding check). `selected_capability_mismatch_is_config_refused`
negotiates 3 then selects backend without streams/table (mutation: start v3
session anyway). `selected_limit_failure_releases_backend` asserts ConfigRefused
and drop count (mutation: skip selected limit check).
`serve_preflight_still_reads_nothing` uses a counting connection and invalid
config (mutation: move old serve preflight after handshake).
`selected_preserves_bytes_after_attach` coalesces attach and subscribe in one
read and receives a sample (mutation: discard buffered tail).

**Review focus:** Transient attach names are safely owned/interned by the
existing decode path before exposing CatalogRef. Do not leak one allocation per
requested unknown catalog. Version 1 refusal and existing origin/liveness
semantics stay intact.

**Checks:** `R(ridl-transport-ws, --features descriptor)`;
`cargo test -p ridl-transport-ws --no-default-features --locked`;
`just freeze-check`.

**Size:** about 200–350 non-test lines.

### Task 6: Run the listening gateway over node attachments

**Files:** add `crates/ridl-transport-ws/examples/debug_gateway.rs`,
`examples/support/debug_gateway.rs`, `tests/debug_gateway.rs`; modify manifest
with required example/test features and loopback's debug dev-dependency. Reuse
conformance fixture sources/bytes without publishing them as transport API.

**Behaviour:** Parse descriptor/machine/listening port, leak the input buffer
once, build Node, print actual bound port, serve each connection through a
listener attachment using selected catalog. Support module owns the shared
gateway construction/backend selection. Bind loopback only. No injection socket
exists yet.

**Tests first / reviewer mutations:**
`gateway_observes_every_interface_across_catalogs` publishes distinct signal
values and events on every offered interface, then verifies all through
independent tungstenite clients, including initial read and later update
(mutation: filter out one offered interface or mapped catalog).
`gateway_listener_sends_a_command` completes a command on a positive provider
through listening connection (mutation: reject listener Caller).
`gateway_rejects_client_publication` sends publish and expects 1002
UnexpectedDirection (mutation: allow publish dispatch).
`gateway_disconnect_drops_listener_attachment` reconnects and asserts clean
subscriptions and no retained connection state (mutation: retain backend on
EOF). `gateway_argument_and_bind_policy` asserts missing arguments fail,
ephemeral port is printed and address is 127.0.0.1 (mutation: bind 0.0.0.0).

**Review focus:** Test support and executable use the same node/selection
implementation. Independent expected interface inventory must be derived from
the fixture specification, not the gateway's filtered table. Document no access
policy beyond loopback binding and no reachable external runtime.

**Checks:** `R(ridl-transport-ws, --features descriptor)`;
`cargo check -p ridl-transport-ws --example debug_gateway --features descriptor --locked`;
explicitly run
`cargo test -p ridl-transport-ws --features descriptor --test debug_gateway --locked`;
`just freeze-check`.

**Size:** about 200–350 non-test lines.

**PR B gardening:** transport record's selected-backend behavior and D-11
capability timing; add `docs/book/debugging.md` and its SUMMARY entry describing
listening and commands, the in-process limitation and local access policy. Do
not describe injection as built yet. Update the roadmap's partial status.

## PR C — reversed provider sessions and injection

Start from clean merged PR B and confirm that WebSocket parameter streams are
not concurrently editing these files. Use the actual base's stream-aware
contracts without enabling reversed streams.

### Task 7: Specify the additive subprotocol and consume all vectors

**Files:** modify `docs/specification/frame-specification.md` §11.1,
`crates/ridl-transport-ws/tests/vectors.rs`,
`packages/ridl-transport-ws/tests/vectors_test.ts`, `xtask/src/freeze/wire.rs`,
regenerated `freeze/wire.txt`; add provider subprotocol constants to
Rust/TypeScript frame modules. Read family overview and ADR-0029 before editing
the normative record.

**Behaviour:** Add the reversed variant, first-message order, server ping rule
and no-mixed-role offer rule without changing existing rows or layouts. Add a
separate Reversed role test vectors table containing `provider-attach-2`,
`provider-attached-accepted`, `provider-publish-live`, explicitly with frame
version 2. Readers and freeze dumper require and read the fourth table. Use the
same catalog/envelope/payload as existing corresponding v2 vectors so role
reversal changes no bytes.

**Tests first / reviewer mutations:** `reversed_vectors_round_trip` in Rust and
TS checks all three exact hex rows decode/re-encode (mutation: change provider
attach's version byte). `all_reversed_rows_are_read` independently asserts those
IDs (mutation: reader skips fourth table).
`reversed_vector_table_is_required_and_unique` in xtask rejects missing table,
duplicate IDs across tables and stream/version-3 forms in the reversed table
(mutation: parse it as version 3 or skip uniqueness). Existing frozen-vector
test retains every old row (mutation: remove an old vector). To establish
assertion-red for table discovery, add valid table rows then show old readers
omit them before modifying readers.

**Review focus:** Separate roles from frame versions. No edit to old bytes,
envelope sizes or catalog hash inputs. Keep §11.1 vectors within its heading
scope; no broad Markdown parser rewrite.

**Checks:** `R(ridl-transport-ws, --features descriptor)`,
`R(xtask, no extra features)`, `T`; `cargo xtask freeze update`, then
`just freeze-check`. Confirm freeze diff consists only of new vector rows.

**Size:** under 180 non-test lines plus prose.

### Task 8: Negotiate role and select the provided backend before upgrade

**Files:** modify `crates/ridl-transport-ws/src/ws.rs`, `src/server.rs`; add
`src/server/provided.rs`, `tests/provided.rs` and manifest test target. Add
ProvidedBackend/Providing in a focused backend module and export from server.rs.

**Behaviour:** Role-aware internal handshake table replaces version-only
selection. Existing serve/serve_selected accept consuming names only. New
functions accept provider name only, validate origin before invoking path
selector, select backend and check capacity before sending 101. Send server
attach immediately after upgrade; require accepted attached with correct
framing. Rejection writes 404/409 and releases backend on every failure. This
task delivers negotiation/attach only; no data forwarding yet.

**Tests first / reviewer mutations:** `listeners_accept_only_their_role` asserts
both cross-role handshakes fail (mutation: use union of protocol names).
`mixed_role_offer_is_rejected` asserts 400 in both entry points (mutation:
choose one role). `provider_selector_gets_exact_path` records path and HTTP
404/409 cases (mutation: drop path or collapse statuses).
`origin_refusal_precedes_selection` asserts 403 and zero selector invocations
(mutation: invoke selector first). `provider_server_attaches_first` checks
selected name, exact attach catalog/encoding/version and next-frame expectations
(mutation: wait for client attach).
`provider_refusal_or_bad_attached_releases_backend` tests refused/malformed
attached and EOF with drop counters (mutation: retain selected backend).
`provider_backend_checks_table_and_limits` checks constructor mismatch and
pre-upgrade capacity refusal (mutation: omit catalog/hash or limit check).
`existing_consuming_handshake_vectors_unchanged` reuses old handshake assertions
(mutation: replace selected old name with provider name).

**Review focus:** No public Config field or Connection method becomes required.
Ended::Rejected can cover new HTTP statuses without altering existing variants.
Avoid touching RFC masking direction: browser frames remain masked even though
session roles reverse.

**Checks:** `R(ridl-transport-ws, --features descriptor)`;
`cargo test -p ridl-transport-ws --no-default-features --locked`;
`just freeze-check`.

**Size:** about 300–450 non-test lines.

### Task 9: Consume remote signals and events with bounded commits

**Files:** extend `src/server/provided.rs`; add
`src/server/provided/signals.rs`, `tests/provided_signals.rs`, manifest test
target; add deterministic iteration to `src/server/members.rs`.

**Behaviour:** After accepted attached, subscribe to every offered signal/event,
no mapped-only interface. Validate target/kind/subscription/provenance/sequence,
using the existing frame rules with roles swapped. Init/Update with payload
stage set; absent/invalid value stages invalidate. Group by client stamp per
Ruling and stamp commits with backend clock. Apply event duplicate/loss and
payload dispositions consistent with existing consumer. Enforce v2 frame bounds
and reject stream frames. Drive timeout using wall-clock tick even when no
socket bytes arrive; backend time comes only from Clock.

**Tests first / reviewer mutations:**
`provider_subscribes_to_exact_offered_members` compares full set and
order-independent IDs (mutation: include mapped interfaces).
`publication_before_subscription_is_rejected` verifies no node state mutation
(mutation: bypass subscription state). `same_stamp_values_commit_together` sees
neither staged value until flush and then equal local stamps (mutation: commit
after every publish). `stamp_change_and_idle_tick_flush` independently checks
both triggers, with client stamp deliberately unlike local clock (mutations:
skip each trigger; forward client stamp). `invalidate_preserves_frame_rules`
tests initial absent, update, invalidation and no payload on invalid form
(mutation: ignore invalidation). `duplicate_and_old_frames_do_not_repeat_writes`
tests signal and event counters plus wraparound (mutation: accept duplicate
seq). `event_payload_and_sequence_dispositions` tests exact limit, over-limit
and loss indication (mutation: silently accept oversized bytes).
`disconnect_discards_pending_group` proves no late commit (mutation: commit in
Drop). `reversed_stream_forms_close_1002` covers each stream form (mutation:
permit chunk).

**Review focus:** Every test observes node ports, not only outgoing frame
counts. Use an injected clock/step time in private session tests; socket tests
use bounded deadlines. Sequence checking precedes mutation and timestamp
grouping. Do not turn opaque loopback bytes into payload schema verification.

**Checks:** `R(ridl-transport-ws, --features descriptor)` and explicit new
integration test target; `just freeze-check`.

**Size:** about 350–500 non-test lines. Keep scheduling glue separate from frame
disposition helpers.

### Task 10: Serve local calls through the remote provider and clean up

**Files:** add `src/server/provided/calls.rs`, `tests/provided_calls.rs`; extend
provided.rs and manifest. Reuse existing correlation helpers where contracts
match, without changing consuming call behavior.

**Behaviour:** Serve only offered command/query ordinals, send claims as
requests and settle their exact response. Preserve correlation, outcomes,
payload ceilings, deadlines and bounded in-flight capacity. Retire/settle every
outstanding claim with Transport::Down on connection loss; deadline uses
existing Timeout result. Server remains responsible for ping/liveness. Refuse
wrong-direction messages and unknown response identities without settling an
unrelated call.

**Tests first / reviewer mutations:** `provider_answers_command_and_query`
verifies argument bytes, matching response and error outcome at Caller
(mutation: settle wrong correlation). `provider_serves_only_offered_ordinals`
leaves another provider's call available (mutation: serve all catalog commands).
`provider_response_duplicate_is_not_resettled` keeps two calls in flight and
replays one response (mutation: correlate by most recent claim).
`provider_call_timeout_and_disconnect_settle` separately checks Timeout and
Down, then reuses all loopback slots (mutation: forget teardown settlement).
`provider_calls_respect_limits_and_capacity` checks payload boundary and more
claims than capacity (mutation: remove bounds).
`provider_server_pings_and_closes_unresponsive_client` uses deterministic core
timing and one bounded wire test (mutation: disable server pings).
`provider_fixed_read_is_unknown_and_wrong_direction_closes` tests fixed read
result and client request rejection (mutation: allow remote request as a
response).

**Review focus:** Claim ownership and cleanup on every Ended path; no fabricated
correlation or silently dropped call. Existing consuming-side call and stream
tests still pass. Do not add reversed streaming support.

**Checks:** `R(ridl-transport-ws, --features descriptor)` and explicit
provided_calls target; `just freeze-check`.

**Size:** about 300–450 non-test lines.

### Task 11: Expose the TypeScript providing API

**Files:** add `packages/ridl-transport-ws/src/provision.ts`,
`src/provision_calls.ts`, `tests/provision_test.ts`; modify `mod.ts`,
`tests/fake_socket.ts` only as needed for reusable socket setup. Keep `connect`
unchanged.

**Behaviour:** Implement the Names API. Offer only provider protocol; wait for
server attach and validate version/catalog/encoding, answer accepted/refused
before resolving. Track subscriptions; publish/raise/invalidate refuse targets
before accepted subscription and after close. Initial subscribe receives one
current value or Init, updates carry increasing per-channel sequence, and
subscribe/unsubscribe update delivery. Caller-supplied stamp enables signal
grouping. Dispatch request through onRequest, map its explicit outcome to
response; an unexpected callback rejection becomes Transport::Down response and
is observed, never an unhandled rejection. Maintain bounded request replay state
using existing session limits. Remote fixed read receives unknown interaction.
Reuse encoding, counters, close/errors and validation rules; do not copy the
entire consuming session.

**Tests first / reviewer mutations:** `provide_waits_for_and_validates_attach`
checks no initial client attach, each refusal, correct response, and resolution
ordering (mutation: resolve at socket open).
`provide_requires_selected_provider_protocol` refuses missing/old selected
protocol (mutation: skip protocol check).
`provide_subscription_and_initial_state` checks exact initial sample, refusal
before subscription and silence after unsubscribe (mutation: publish
unconditionally). `provide_encodes_values_events_and_invalidation` checks exact
decoded fields, stamps and wraparound (mutation: reuse a single sequence across
channels). `provide_dispatches_and_replays_requests_once` checks one callback on
repeated request and correct command/query/error response (mutation: invoke
callback twice). `provide_bounds_and_bad_direction` checks limits, stream forms,
fixed read and unexpected attach/request targets (mutation: skip kind check).
`provide_close_settles_pending_operations` checks close/EOF/callback rejection
without further sends or unhandled promises (mutation: allow callback to send
after close).

**Review focus:** Use the actual frame types when assigning fields; do not
invent a second envelope format. Fake socket asserts both send order and exact
bytes. Browser-compatible module imports, no Deno globals in src, no change to
existing exports' shapes.

**Checks:** `T`; `deno check mod.ts`,
`deno check --config browser.deno.json mod.ts`, `deno publish --dry-run`. Run
existing consuming session tests too.

**Size:** about 450–580 non-test lines across the two focused modules. Split
API/session from request replay if larger.

### Task 12: Route gateway injection and prove cross-language replacement

**Files:** extend `crates/ridl-transport-ws/examples/debug_gateway.rs`,
`examples/support/debug_gateway.rs`, `tests/debug_gateway.rs`; add
`examples/debug_gateway_fixture.rs` and manifest target for an e2e fixture host;
add `packages/ridl-transport-ws/e2e/provide_test.ts`. Fixture host initializes
the same Node used by gateway support and local consumer/provider controls; no
test-only production API.

**Behaviour:** Add optional injection listener. Resolve path to a local placed
instance and an offered catalog, attach as twin, build filtered member table
from embedded descriptor and run provided session. Independent fixture control
exposes local consumer observations, not gateway-private buffers. Listening can
remain active while injection provides. Reject positive attachment throughout
twin lifetime and permit it after teardown.

**Tests first / reviewer mutations:** `gateway_injection_path_statuses` covers
unknown component/instance/catalog, off-machine instance, malformed path and
active N with exact 404/409 (mutation: attach without checking placement or map
conflict to 404). `gateway_path_selects_only_offered_members` rejects a
mapped-only catalog and injects nothing on non-offered interface (mutation: use
whole catalog table). `gateway_refused_handshake_releases_twin` tries bad
origin, negotiation failure, refused attached and disconnect, then attaches N
(mutation: retain attachment).
`gateway_listening_port_refuses_provider_protocol` asserts no 101 and no local
writes (mutation: share role allowlist across sockets).
`gateway_injection_is_absent_by_default` starts listening-only fixture and
asserts no injection address/socket advertised (mutation: always bind
injection). TypeScript `provide_replaces_detached_instance_end_to_end` uses
provide to inject signal 42, invalidate it, raise event 7, answer a local
command and query, while a listening connection receives updates and can send a
command. Local consumer verifies each byte/outcome, N attach fails while
provider lives and succeeds after close. Mutation: replace twin attachment with
an unrestricted Loopback, or skip any one forwarding leg; each assertion must
fail independently.

**Review focus:** Use real TypeScript provide and a Rust process sharing one
node; no mock socket in acceptance test. Concurrent listener and injection prove
separate connection roles. Keep redundant-provider behavior documented, not
arbitrated. Bound startup/observation/teardown waits and stop the child process
in finally.

**Checks:** `R(ridl-transport-ws, --features descriptor)`;
`cargo build -p ridl-transport-ws --features descriptor --example debug_gateway_fixture --locked`;
`T`; from the package,
`deno test --allow-read --allow-net --allow-run e2e/provide_test.ts` with
fixture binary path supplied through its test argument/config (add
`--allow-env=RIDL_DEBUG_GATEWAY_FIXTURE` only if that environment variable is
used); `just freeze-check`. Reuse the existing ts-e2e recipe for execution
wiring if adding this acceptance test to CI, keeping command ownership in
justfile; inspect gate parity and workflow when changing the recipe. Do not
create a second shell build command in CI.

**Size:** about 250–400 non-test lines, excluding fixture tests.

**PR C gardening:** transport design D-20 role table, provided ports, selected
entry points, and replace the stand-in section with as-built behavior;
deployment record's twin path and lifetime; frame §11.1 remains normative as
changed in Task 7; update `freeze/README.md` to mention the fourth vector table;
package README for provide; debugging book page for both listeners, injection
and calls, local-only policy, no generated debug constants, and redundant sets
needing other instances stopped. Update family overview references if the
normative record's reference map changes. Mark E19.7 landed and close #988 only
after complete acceptance/branch review; preserve the distinctions from
E19.9/E19.12. Claude handles branch review, gardening, PR and CI; this
implementer stops after Task 12's report.

## Plan amendments (plan check, 2026-11-03)

The plan was checked against the code at f0b6847b. These amendments override the
task text above where they differ. Each task brief includes the amendments that
name it.

- **P-1 (Names, Task 5; was the "Stopped" section).** The selector receives a
  borrowed request, not a `CatalogRef`:
  `pub struct RequestedCatalog<'a> { pub name: &'a str, pub hash: CatalogHash }`
  in `server`, and `S: for<'a> FnOnce(RequestedCatalog<'a>) -> Option<B>`. The
  selector compares name and hash; the gateway maps them to the `'static`
  `CatalogRef` from `Node::catalogs()`. No interning and no leaked client
  string. Task 5's review focus on interning is replaced by this.
- **P-2 (Task 11).** `ResponseOutcome` has no "down" outcome
  (`crates/ridl-transport-ws/src/frame.rs:243-254`,
  `packages/ridl-transport-ws/src/frame.ts:169-175`). When the TypeScript
  handler throws or rejects, `provide` sends no `response` for that request, and
  the server settles the call locally as Task 10 does for a missing response.
  `ProvidedResult` is the `ResponseOutcome` itself, with no separate `payload`
  field.
- **P-3 (Task 1).** Add `xtask/src/main.rs` to the files and add `"debug"` to
  `NON_ENCODING_FEATURES` (`xtask/src/main.rs:74`); its test at :101 fails on
  any other `ridl-rt` feature.
- **P-4 (Tasks 3 and 4).** `compat-check`: give the packaged `ridl-loopback`
  cell (`justfile:645-669`) the same patching (`ridl-descriptor`, `ridl-ir`) and
  `resolve_for` and fetch steps as the transport cell (`justfile:681-686`), with
  the new features on. Add `tests/node.rs` to the `exclude` list of
  `ridl-loopback` when it includes a fixture outside the package. No
  `gate-parity` change.
- **P-5 (Tasks 3 and 4).** Add
  `cargo test --locked -p ridl-loopback --features
  debug` and
  `cargo test --locked -p ridl-rt-conformance --features debug` to the `test`
  recipe, and the matching clippy lines to `lint`. Gate `tests/node.rs` with
  `required-features`, so that `cargo test --workspace` still compiles.
- **P-6 (Task 3).** The `node` feature makes `ridl-loopback` a second runtime
  crate that reaches planus. Amend ADR-0020 decision 6 and its 2026-10-24
  amendment in place, and the comment at the root `Cargo.toml:88-97`, in PR A.
  Add an `xtask/tests/oracle_boundary.rs` case: `ridl-loopback` without `node`
  reaches no planus.
- **P-7 (Task 8).** Each entry point ignores the names of the other role, as
  `ws.rs:221-226` ignores unknown names today. A mixed offer to `serve` selects
  `ridl-ws-v2` (assert it); `400` only when no name of the server's own role is
  offered. `mixed_role_offer_is_rejected` is replaced by these two tests.
- **P-8 (Task 12).** The TypeScript acceptance test is required. Extend the
  `ts-e2e` recipe (`justfile:2947`) to build `debug_gateway_fixture` and run
  `e2e/provide_test.ts`. `ts-e2e` is already in `build` and in CI.
- **P-9 (Task 2).** Use a cargo probe for the generated `debug` feature (the
  generated crate, `[patch.crates-io] ridl-rt = { path = ... }`, `--offline`,
  `--no-default-features`), and list `crates/ridlc/tests/support/mod.rs` if it
  changes; the rustc helper `support::ridl_rt_rlib` never reads the manifest.
  Update the assertion at `rust_crate_emit.rs:567-572` that pins three features.
  PR A's gardening adds the `debug` row to `docs/book/generated-code.md:314-317`
  and the sentence at `docs/book/cli-reference.md:898-899`.
- **P-10 (Tasks 3 and 4).** The fixture catalogs' `CatalogRef` values (name and
  32-byte hash) are committed as constants in the conformance crate's
  `identity.rs`. A test in `ridl-loopback/tests/node.rs` checks the constants
  against the committed descriptor, and a test in `crates/ridl/tests` checks the
  committed descriptor against a regeneration from its sources.
- **P-11 (Names).** `MemberTable::iter(&self) -> impl Iterator<...> + '_`.
  `SelectionError` is `#[non_exhaustive]`.
- **P-12 (Task 4).** `suite!` expands in the caller's crate, so the debug arms
  are defined under `#[cfg(feature = "debug")]` in the conformance crate; with
  that feature off, `identity_debug` expands to `compile_error!`. Place and
  count the new arms to match the inventory test (`lib.rs:594-623`).
  `existing_factory_needs_no_identity_hook` lives in
  `crates/ridl-loopback/tests/conformance.rs`, which expands `suite!`.
- **P-13 (Task 5).** Under `serve_selected`, a capability refusal comes after
  `attach`: the client receives `attached` with a refused outcome and close code
  1000, as for the D-11 refusals. Update the doc of `Ended::ConfigRefused`
  (`server.rs:146-156`).
- **P-14 (Names).** The node serves FlatBuffers only, and TypeScript `connect`
  defaults to proto3 (`session.ts:161`). Tests set the encoding; the book page
  says so.
- **P-15 (gardening).** Add: `docs/design/ridl-transport-ws-ts.md` for `provide`
  (PR C); `docs/book/writing-a-port.md:153-183` for the new `suite!` flags (PR
  A); the `NotOwner` sentences at `ridl-loopback/src/lib.rs:73-74` and
  `docs/design/ridl-loopback.md:854-855` change in Task 3's code. The book page
  names no story id (link driftsys/ridl#1196), and any rsdl `system` fence on it
  is marked `ignore` (RSDL-601).

## Phase-1 self-review

- Design parts 1–2 map to Tasks 1–4, listening to Tasks 5–6, reversed
  binding/Rust/TypeScript/path/acceptance to Tasks 7–12. Owner Q2's listening
  command is covered in Task 6; Q3's non-arbitration is explicit in constraints
  and gardening.
- Every task has file ownership, observable behavior, named tests, mutations,
  focused checks and an estimated size. Non-test line estimates require an
  execution-time split if exceeded.
- New APIs permit compilation-red; existing behavior changes require
  assertion-red. Negative-feature doctests run outside workspace feature
  unification. Mutations are review work, not assumed evidence.
- No implementation or test execution is claimed by this plan. Live merge/CI
  state and PR C's integration ordering must be checked by the owner when each
  PR starts.
- Stop here for owner and Claude plan review. No task begins in phase 1.

## Delegated rulings (the owner delegated decisions on 2026-11-03)

- **R-DS-1.** The design, this plan and the handoff stay in `docs/wip/` until PR
  C, because PRs B and C still build from them. PR A and PR B garden only their
  own content into the durable records; PR C archives the three files. Each PR
  body states this.
- **R-DS-5 (PR B review, 2026-11-04).** Corrects P-1's "the selector compares
  name and hash": a selector chooses the backend by catalog name only, and the
  session's unchanged D-11 checks then compare the hash, so a known name with
  another hash is refused `catalog_mismatch` (frame §6.1), not
  `unknown_catalog`. `None` means the name is unknown.
- **R-DS-5, amended (PR B pass 2, 2026-11-04).** A deployment descriptor may
  embed two catalogs with one name and different hashes, so the gateway's
  selector prefers the catalog whose name and hash both match and otherwise
  chooses by name, where D-11 then answers `catalog_mismatch`. `None` still
  means that no catalog of the machine has the name.
- **R-DS-2.** Streams stage 5a is open as driftsys/ridl#1222 and stage 5b has
  not started, so PR C is built after PR B without waiting for stage 5b. Of PR C
  and stage 5b, the one that lands second rebases onto the other.
- **R-DS-3 (Task 9 stop, 2026-11-04).** Writing a remote provider's frames
  straight into `SignalWriter` and `EventSink` cannot follow the consumer rules
  of §5 to §9 that the "Reversed role" section applies: the ports cannot restore
  an `Init`, cannot carry a detected-invalid sample, and cannot carry a corrupt
  occurrence or the remote envelope and sequence. So:
  - The provided session is a full consumer. It applies §5 to §9 itself (the
    `Init` rule, the length refusal before verification, `Invalid(Detected)`
    samples that keep the last good value, `Err(Corrupt)` occurrences that are
    delivered, sequence gaps) exactly as a consuming client does.
  - `ProvidedBackend` receives what a consumer's application receives, not port
    writes: one call per delivered sample (interface, ordinal, provenance,
    validity with its cause, payload, the remote stamp), one per delivered
    occurrence (interface, ordinal, the remote sequence and envelope, `Ok` with
    the payload or `Err` with the cause, and a gap when one was detected), and
    one at the end of each commit group (consecutive frames with the same remote
    stamp). Its `Ports` keep `Attached + Encoded + Clock + Handler` for the
    calls. The builder names the types; the plan's Names section records them.
  - `Providing<P>` maps those deliveries onto local ports, and its rustdoc lists
    what the ports cannot carry: `Init` writes nothing (the twin has not
    published; the last value on the machine stays); a live value is `set`, and
    `commit` runs at the end of each group, stamped by the node; a declared
    invalid value is `invalidate`; a detected-invalid value is `invalidate` too
    (the machine reports `Declared`) and records a `Note`; a corrupt occurrence
    is not raised and records a `Note`; a gap records a `Note`. The loss is
    confined to the debug path and is stated in the book page.
  - No frozen port trait changes.
- **R-DS-4 (Task 9 review, 2026-11-04).** Clarifies R-DS-3: only signal frames
  form commit groups. An occurrence never opens, extends or closes a group
  (frame §5.2: "There is no staging and no commit"), so an event interleaved in
  a signal group does not split it, and no group holds only occurrences. A group
  also closes before a second accepted frame for a channel already in it, so one
  local `commit` carries at most one frame per channel (§5.1) and a client that
  keeps one stamp still commits.
- **R-DS-6 (Task 10 review, 2026-11-04).** On the provided session, a `response`
  whose outcome is outside its call kind's set (`accepted` for a query, `reply`
  for a command; frame §5.3 and §5.4) settles the local claim as
  `Transport::Corrupt`, so the call ends and no claim waits for its deadline.
  The TypeScript consumer returns `unexpected-outcome` and leaves its call
  unsettled; the two consumers differ, and the design record states both.
- **R-DS-7 (Task 11 stop, 2026-11-04).** A browser's or Deno's `WebSocket`
  cannot close with 1002 or 1003 and does not show pings to the page. The
  TypeScript providing side therefore follows the TypeScript consumer's D-10 and
  D-11 (`docs/design/ridl-transport-ws-ts.md`) exactly: on a violation it calls
  `close()` with no code and records the violation through `onNote`; it has no
  client liveness timer, and the server's liveness check is the one that runs.
  driftsys/ridl#991 already asks the frame specification to state what such a
  client does; it covers the reversed role too.
