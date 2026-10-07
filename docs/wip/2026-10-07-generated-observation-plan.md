# Observation from the generated Rust face, phase 1 — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** The generated Rust face emits `tracing` spans for calls, claims,
raises and occurrences, emits warning and error events for failures inside
`dispatch`, and calls an application trace-context hook in `ridl_rt::trace`.
Each of these is behind a Cargo feature that is off by default.

**Architecture:** `ridl-rt` gains the `Propagation` hook (std only, no
dependency). The generated crate gains a private module, `__observe`, emitted
inline in `lib.rs`. It is the only code that names `tracing` or the hook. The
emitted face invokes only `__observe` items. With the features off, each item is
a zero-sized type or a macro that expands to nothing.

**Tech Stack:** Rust; `tracing` 0.1 (optional dependency of the generated
crate); `tracing-subscriber`, `opentelemetry`, `opentelemetry_sdk` and
`tracing-opentelemetry` as dev-dependencies of `ridl-backend-rust` only.

**Spec:** `docs/wip/2026-10-07-generated-observation-design.md` (D-1 to D-9).
Phase 2 (metrics) is out of scope.

## Order of work and the ridl-rt 0.6.0 release

The work is two pull requests with a release between them.

| Step | What                                                     | Who              |
| ---- | -------------------------------------------------------- | ---------------- |
| 0    | PR #766 (the spec and this plan) is merged               | Sebastien        |
| A    | Tasks 1-2 on `feat/754-propagation-hook`, one PR, merged | executor         |
| R    | Release 0.6.0 from `main` (see below)                    | Sebastien        |
| B    | Tasks 3-9 on `feat/754-observe-face`, one PR             | executor, from R |

**Why the release sits between A and B.** The code emitted in Task 7 calls
`ridl_rt::trace::propagation`, which exists only from the release that ships
Task 2. The generated manifest's `ridl-rt` requirement is a literal that the
test `ridl_rt_version_requirement_matches_the_crate`
(`crates/ridlc/tests/rust_crate_emit.rs`) holds equal to the workspace version.
So the requirement can move from `"0.5"` to `"0.6"` only in the release commit,
and a branch that emits a call to the hook before then would pin a `ridl-rt`
that does not have it. Task 2 must therefore be on `main` before the release,
and the 0.6.0 release also ships the trace context of driftsys/ridl#752.

**Step R** follows the release procedure: `git std bump` on `main`, and in the
same commit move the `ridl-rt` literal in `render_cargo_toml` to `"0.6"`, the
`ridl-rt` requirement in `examples/cabin/consumer/Cargo.toml` and the comment in
`examples/cabin/Cargo.toml`, the `ridl-core` literal in
`crates/ridl-sem/Cargo.toml`, regenerate `examples/cabin/generated`, run
`cargo update -w --offline` for both lockfiles, run `just build`, then push with
`--follow-tags`. Branch B starts from the release commit.

## Global Constraints

- `ridl-rt` keeps no dependency in any feature combination, and no `unsafe`
  (`#![forbid(unsafe_code)]`, `crates/ridl-rt/src/lib.rs:113`).
- The hook is behind `ridl-rt`'s `std` feature, held in a `std::sync::OnceLock`.
  `OnceLock` is stable since 1.70; `ridl-rt`'s `rust-version` is 1.83.
- The generated crate stays `no_std` capable and allocates nothing on the hot
  path. With every observation feature off, its compiled code is the same as
  today.
- Generated manifest, exactly: `tracing = ["dep:tracing"]`,
  `trace-context = ["std"]`, `std = ["ridl-rt/std", "tracing?/std"]`, and
  `tracing = { version = "0.1", default-features = false, optional = true }`.
  `default` stays `["validate-pattern", "std"]`.
- Minimum Rust version of `tracing`: `tracing` 0.1.44 and `tracing-core` 0.1.36
  declare 1.65, below `ridl-rt`'s 1.83 (checked for D-8). The bare-`rustc` cell
  of `just compat-check` gets no `tracing` cell.
- All spans at level `trace`. Target of every call site: `ridl::<catalog>`.
- Span names: `<Interface>/<member>` (call, claim);
  `publish <Interface>.<member>` (raise); `process <Interface>.<member>`
  (occurrence).
- Span fields: call and claim `rpc.system = "ridl"`, `rpc.service`,
  `rpc.method`, `ridl.catalog`; raise and occurrence
  `messaging.system = "ridl"`, `messaging.destination.name`,
  `messaging.operation.type` (`send` or `process`), `ridl.catalog`; every span
  `outcome`.
- `outcome` is a `&'static str`: `ok`, `cancelled`, or `<Enum>::<Variant>` (for
  example `Transport::Timeout`, `Contract::PreconditionFailed`). The ridl-rt
  error enums are `#[non_exhaustive]`, so each match has a wildcard arm that
  records `<Enum>::Other`.
- No payload byte is ever recorded.
- Received trace context is recorded as `trace.trace_id`,
  `trace.parent_span_id`, `trace.flags`, in lowercase hexadecimal, through a
  `Display` wrapper that does not allocate. The fields are absent when the
  received context is `None`.
- RIDL never creates W3C ids. No OpenTelemetry crate outside
  `[dev-dependencies]`.
- Signals get no span (D-7). `enter`/`leave` are not called for an occurrence
  (D-6).
- Shipped docs and rustdoc name no story id or plan name
  (`just story-id-check`). Prose is plain and literal. Commit scopes come from
  `.git-std.toml`.

**One plan-level decision:** D-1 calls the module `observe`. A manifest package
name matches `[a-z][a-z0-9]*` (MANI-006), so a package named `observe` would
emit `pub mod observe` at the crate root and collide. The module is named
`__observe`, which no package segment can spell, and is emitted inline in
`lib.rs` so that the overwrite gate of `crate_file_refusals` does not gain a
third file.

## Review Focus

1. **A call retried on `SendError::Busy`.** `send_<member>` runs again on each
   poll while the port is busy. Expected: one call span per call, not one per
   send attempt. Test in Task 4.
2. **A claim handler that panics.** Expected: `leave` runs exactly once, the
   claim span closes, and the panic still reaches the caller of `serve`. Test in
   Task 7.
3. **A failure in `dispatch` with no member.** An unknown ordinal, a claim for
   another interface, and `ReadError::ShortClaim` have no `MEMBER`. Expected:
   the event carries the interface and the ordinal number, and nothing indexes
   `MEMBERS`. Test in Task 5.
4. **An application span is current around `serve`.** Expected: the claim's
   OpenTelemetry parent is still the received remote context, not the
   application's span. Test in Task 1 and again in Task 7.
5. **A call future dropped before its first poll, or polled after it resolved.**
   Expected: `outcome = "cancelled"` once in the first case, and no second
   outcome in the second. Test in Task 4.

---

## Pull request A

### Task 1: Spike — fix the hook's call order against `tracing-opentelemetry`

The spec's D-6 signature is provisional. `tracing-opentelemetry`'s `set_parent`
returns `AlreadyStarted` once the OpenTelemetry span has started. This task
finds the order in which `dispatch` must open the claim span and call `enter`,
and changes the signature only if no order works.

**Files:**

- Modify: `crates/ridl-backend-rust/Cargo.toml` (`[dev-dependencies]`)
- Create: `crates/ridl-backend-rust/tests/otel_propagation.rs`
- Modify: `docs/wip/2026-10-07-generated-observation-design.md` (D-6)

**Interfaces:**

- Produces: the `Propagation` signature and the call order that Task 2 and Task
  7 implement, written in D-6 under a new paragraph "Call order".

- [ ] **Step 1: Add the dev-dependencies.** `tracing`, `tracing-subscriber`
      (feature `registry`), `opentelemetry`, `opentelemetry_sdk` (feature
      `testing`, for `InMemorySpanExporter`) and `tracing-opentelemetry`, at the
      newest set that `tracing-opentelemetry`'s own version table declares
      compatible, with `cargo add --dev`. Commit the updated `Cargo.lock`,
      because `just test` runs with `--locked`. Run:
      `cargo build -p ridl-backend-rust --tests --locked`. Expected: builds.

- [ ] **Step 2: Write the test with a local copy of the trait.** In
      `otel_propagation.rs`, copy the D-6 trait (`current`, `enter`, `leave`)
      and implement it with OpenTelemetry: `current` reads
      `tracing::Span::current().context()`'s span context; `enter` makes the
      received context the parent; `leave` undoes `enter`. Install a `Registry`
      with an `OpenTelemetryLayer` over an `InMemorySpanExporter` with
      `tracing::subscriber::with_default`. Simulate one call and one claim in
      one thread: open and enter a `trace_span!("Cabin/setLevel")` call span,
      take `sent = hook.current()`, exit it; then, inside an enclosing
      application span `trace_span!("serve loop")`, simulate `dispatch` with
      each candidate order below and a nested call inside the handler. Tests:

  - `claim_span_has_the_remote_parent` — the exported claim span's
    `parent_span_id == sent.span_id` and `trace_id == sent.trace_id`.
  - `nested_call_is_a_child_of_the_claim` — `current()` taken inside the handler
    has `trace_id == sent.trace_id` and `span_id ==` the exported claim span's
    span id.
  - `leave_restores_the_application_span` — after the handler, the current
    OpenTelemetry context is the `serve loop` span's again.

  Candidate orders, tried in this order:

  - **O1 (the spec as written):** create and enter the claim span, then
    `enter(received)`, which calls `set_parent` on `Span::current()`.
  - **O2:** `enter(received)` first, which attaches the received context; then
    create the claim span as an explicit root (`parent: None`) so that
    `tracing-opentelemetry` takes its parent from the attached context.
  - **O3:** if neither passes, change the signature (for example, `enter` before
    the span exists returns nothing and RIDL creates the span with
    `parent: None`, and a second hook method runs once the span is entered).
    Choose the smallest change that passes the three tests.

- [ ] **Step 3: Run the tests.** Run:
      `cargo test -p ridl-backend-rust --test otel_propagation --locked`
      Expected: the three tests pass for the kept order; leave the code for the
      rejected orders out of the file.

- [ ] **Step 4: Record the result in D-6.** Add a "Call order" paragraph: the
      kept order, the version of `tracing-opentelemetry` it was proven against,
      and why each rejected order failed (the observed error or parent). If the
      signature changed, replace the code block in D-6.

- [ ] **Step 5: Commit.**
      `test(ridl-backend-rust): fix the propagation hook's call order against tracing-opentelemetry`

### Task 2: `Propagation` in `ridl_rt::trace`

**Files:**

- Modify: `crates/ridl-rt/src/trace.rs`
- Create: `crates/ridl-rt/tests/propagation.rs`,
  `crates/ridl-rt/tests/propagation_unset.rs`
- Modify: `crates/ridl-backend-rust/tests/otel_propagation.rs` (use the real
  trait)
- Modify: `docs/decisions/ADR-0021-ridl-rt-0.1-api-and-release.md`,
  `docs/design/ridl-rt.md`

**Interfaces:**

- Produces, all under `#[cfg(feature = "std")]`, with the signature that Task 1
  recorded (the D-6 one unless Task 1 changed it):
  - `pub trait Propagation: Sync { fn current(&self) -> Option<TraceContext>; fn enter(&self, received: Option<TraceContext>); fn leave(&self); }`
  - `pub struct AlreadySet;` with the derives and the `Display` and `Error`
    implementations that `SendError` (`crates/ridl-rt/src/port.rs:611`) has.
  - `pub fn set_propagation(p: &'static dyn Propagation) -> Result<(), AlreadySet>`
  - `pub fn propagation() -> Option<&'static dyn Propagation>`

- [ ] **Step 1: Write the failing tests.** The hook is global to the process, so
      each file is one test binary:

  - `propagation.rs::set_once_then_refuse` — one test function: `propagation()`
    is `None`; `set_propagation(&A)` is `Ok(())`; `propagation()` returns `A`
    (its `current()` returns A's marker context); `set_propagation(&B)` is
    `Err(AlreadySet)`; `propagation()` still returns `A`.
  - `propagation_unset.rs::no_hook_by_default` — `propagation().is_none()`.

- [ ] **Step 2: Run them.**
      `cargo test -p ridl-rt --features std --test propagation --test propagation_unset`
      Expected: FAIL to compile, `set_propagation` not found.

- [ ] **Step 3: Implement** the four items in `trace.rs` over a private
      `static HOOK: OnceLock<&'static dyn Propagation>`. Rustdoc: the call
      points from D-6, that the hook is disabled until set, one hook per
      process, and the call order recorded by Task 1.

- [ ] **Step 4: Run** the Step 2 command, then `just compat-check` and
      `just wasm-check`. Expected: PASS; `ridl-rt` still has no dependency.

- [ ] **Step 5: Use the real trait in `otel_propagation.rs`** (delete the local
      copy; the hook object is passed directly, not registered, so the test
      stays independent of the global). Run its tests. Expected: PASS.

- [ ] **Step 6: Records.** ADR-0021: a new decision for `Propagation`,
      `set_propagation`, `propagation` and `AlreadySet`, behind `std`, an
      addition to the API; its alternatives are the ones in the spec's
      "Alternatives considered" about W3C ids and per-crate hooks.
      `docs/design/ridl-rt.md`: list the items under `trace`.

- [ ] **Step 7: Commit.**
      `feat(ridl-rt): add the trace-context propagation hook`

**End of PR A.** Open the PR, run the review, merge. Then step R.

## Pull request B (from the release commit)

### Task 3: Manifest features and the `__observe` module

**Files:**

- Create: `crates/ridl-backend-rust/src/observe.rs`
- Modify: `crates/ridl-backend-rust/src/lib.rs` (export),
  `crates/ridl-backend-rust/Cargo.toml` (features, dev-dependency)
- Modify: `crates/ridlc/src/lib.rs` (`render_cargo_toml` at about line 1298 and
  its doc comment; `render_lib_rs` at about line 1348)
- Create: `crates/ridl-backend-rust/tests/generated/observe.rs`
- Modify: `crates/ridl-backend-rust/tests/interaction_face_regeneration.rs`,
  `crates/ridl-backend-rust/tests/interaction_face.rs`,
  `crates/ridl-backend-rust/tests/face_compile.rs`,
  `crates/ridlc/tests/rust_crate_emit.rs`

**Interfaces:**

- Produces: `pub fn observe_module() -> String` in `ridl-backend-rust`: the text
  of `mod __observe { ... }`, appended by `render_lib_rs` after the package
  modules. Later tasks add items to it. Every item is `pub(crate)`; only items
  under `#[cfg(feature = "tracing")]` name `::tracing`, and only items under
  `#[cfg(feature = "trace-context")]` name `::ridl_rt::trace::propagation`.
- Produces: in `ridl-backend-rust`'s `[features]`, `tracing = []` and
  `trace-context = []`, both added to `default`, with a comment like the one on
  `std`: the checked-in face is `include!`d and its cfgs are evaluated against
  this crate. Dev-dependency
  `tracing = { version = "0.1", default-features = false, features = ["std"] }`.
- Produces: the test crates `include!` `tests/generated/observe.rs` inside
  `mod __observe { }` at their root.

- [ ] **Step 1: Write the failing tests** in `rust_crate_emit.rs`:

  - extend `the_emitted_manifest_parses_and_carries_the_declared_structure`:
    `features.tracing == ["dep:tracing"]`, `features.trace-context == ["std"]`,
    `features.std == ["ridl-rt/std", "tracing?/std"]`,
    `features.default == ["validate-pattern", "std"]`, dependency `tracing` has
    `version == "0.1"`, `default-features == false`, `optional == true`.
  - `tracing_version_requirement_matches_the_lockfile` — like
    `regex_version_requirement_matches_the_checker`: the `tracing` entry of the
    workspace `Cargo.lock` matches the emitted requirement `0.1`.
  - `the_emitted_lib_declares_the_observe_module` — `lib.rs` contains
    `mod __observe`.
  - `a_package_named_observe_builds` — a package `observe` with one interface
    builds through the `rustc` harness of `cabin_example.rs`.
  - `the_face_names_no_tracing_item` (in `face_generation.rs`) — the emitted
    face of `tests/fixtures/interaction_face.ridl` contains no `tracing` token
    and no `propagation` token; every observation reference is a
    `crate::__observe::` path.
  - `the_observe_module_names_tracing_only_under_its_feature` — parse
    `observe_module()` with `syn`; every item that mentions `tracing` carries
    `#[cfg(feature = "tracing")]`, and every item that mentions `propagation`
    carries `#[cfg(feature = "trace-context")]`.

- [ ] **Step 2: Run** `cargo test -p ridlc --test rust_crate_emit` and
      `cargo test -p ridl-backend-rust --test face_generation`. Expected: FAIL.

- [ ] **Step 3: Implement.** Manifest per Global Constraints, doc comment
      updated (both features, independence, `default-features = false`).
      `observe_module()` returns `mod __observe {}` with no items yet; later
      tasks add each item when the face first uses it. `compile_face` in
      `face_compile.rs` prepends the module and checks each cfg combination:
      `{}`, `{std}`, `{std, trace-context}`. The `tracing`-on combinations are
      compiled by the `include!` test crates, which have the dev-dependency.

- [ ] **Step 4: Regenerate and run.**
      `RIDL_UPDATE_GENERATED=1 cargo test -p ridl-backend-rust --test interaction_face_regeneration`,
      then `cargo test -p ridlc -p ridl-backend-rust`. Expected: PASS.

- [ ] **Step 5: Commit.**
      `feat(ridlc): add the tracing and trace-context features to the generated crate`

### Task 4: Call spans for commands and queries

**Files:**

- Modify: `crates/ridl-backend-rust/src/observe.rs`,
  `crates/ridl-backend-rust/src/face.rs` (`call_method`, line 729),
  `crates/ridl-backend-rust/src/face/futures.rs` (`call_future`, line 43)
- Create: `crates/ridl-backend-rust/tests/observe_spans.rs`,
  `crates/ridl-backend-rust/tests/support/recorder.rs`

**Interfaces:**

- Produces in `__observe`:
  - `pub(crate) struct Span` — wraps `::tracing::Span` with `tracing` on; a
    zero-sized struct with it off.
  - `impl Span { fn enter(&self) -> Entered<'_>; fn outcome(&self, o: &'static str); }`
    — `Entered` is `::tracing::span::Entered` or a zero-sized struct.
  - `macro_rules! call_span` taking literal `target`, `name`, `service`,
    `method`, `catalog`, re-exported with `pub(crate) use call_span;`. It
    creates a `trace_span!` with the Global Constraints fields and
    `outcome = ::tracing::field::Empty`; off, it evaluates to `Span`.
  - `fn client_outcome<T>(r: &Result<T, ::ridl_rt::error::ClientError>) -> &'static str`
    — `ok`, `Send::<Variant>`, `Read::<Variant>`, `Transport::<Variant>`,
    `Contract::<Variant>`, wildcard `<Enum>::Other`.
- Produces in `tests/support/recorder.rs`: a `tracing::Subscriber` (built from
  `tracing-subscriber`'s `Registry` and a layer) that records, for each span,
  its name, target, level, fields and close order, and for each event its level,
  target and fields. Tests install it with `tracing::subscriber::with_default`,
  never globally.

- [ ] **Step 1: Write the failing tests** in `observe_spans.rs` (the fixture
      face is `include!`d as in `interaction_face.rs`; expected names come from
      the descriptors, `<Cabin as Interface>::NAME`,
      `<Interface>::CATALOG.name`, `MEMBER.name`):

  - `a_command_opens_one_call_span` — `Client::set_level` over loopback, served
    once: exactly one span named `"Cabin/setLevel"`, target `"ridl::<catalog>"`,
    level `TRACE`, `rpc.system = "ridl"`, `rpc.service = "Cabin"`,
    `rpc.method = "setLevel"`, `ridl.catalog = <catalog>`, `outcome = "ok"`.
  - `a_query_records_the_contract_outcome` — `average` with an argument that
    fails `require`: `outcome = "Contract::PreconditionFailed"`.
  - `the_send_runs_inside_the_call_span` — the provider's claim arrives while
    the recorder saw the call span entered at send time (record
    `Span::current()` inside a wrapping `Caller`).
  - `a_busy_port_still_opens_one_span` — fill the port's slots so the first poll
    gets `SendError::Busy`, then free one: one call span.
  - `a_dropped_future_records_cancelled` — drop before the first poll, and drop
    while waiting: each span has `outcome = "cancelled"` once.
  - `a_resolved_future_records_one_outcome` — poll after it resolved: the span
    has one outcome.
  - `the_blocking_client_opens_a_call_span` — `blocking` `set_level`: one span
    named `"Cabin/setLevel"`.

- [ ] **Step 2: Run** `cargo test -p ridl-backend-rust --test observe_spans`.
      Expected: FAIL (no span recorded).

- [ ] **Step 3: Implement.** `call_method` creates the span with
      `crate::__observe::call_span!` (literals from the IR) and moves it into
      the future. The future struct gains a `span: crate::__observe::Span`
      field; each `poll` holds `this.span.enter()` for that poll only, records
      `client_outcome` when it returns `Ready`, and the existing `Drop` records
      `cancelled` when no outcome was taken.

- [ ] **Step 4: Regenerate the checked-in face and run** the Step 2 command,
      `cargo test -p ridl-backend-rust`, and `cargo test -p ridlc`. Expected:
      PASS.

- [ ] **Step 5: Commit.**
      `feat(ridl-backend-rust): open a tracing span for each call`

### Task 5: Claim spans and dispatch events

**Files:**

- Modify: `crates/ridl-backend-rust/src/observe.rs`,
  `crates/ridl-backend-rust/src/face/dispatch.rs` (`dispatch`, line 17)
- Create: `crates/ridl-backend-rust/tests/observe_dispatch.rs`

**Interfaces:**

- Consumes: `Span`, `call_span!` pattern from Task 4.
- Produces in `__observe`: `macro_rules! claim_span` (same literals as
  `call_span!`, plus a `received: Option<TraceContext>` argument, unused until
  Task 7); `macro_rules! dispatch_warn` and `dispatch_error`, taking literal
  `target`, `service` and `catalog`, the ordinal, an optional member name, and
  the `&'static str` outcome;
  `fn settle_outcome(r: &Result<&[u8], CallError>) -> &'static str`.

- [ ] **Step 1: Write the failing tests** in `observe_dispatch.rs`, driving
      `dispatch` directly as the settlement-table tests of `interaction_face.rs`
      do:

  - `an_unknown_ordinal_is_a_warning` — one `WARN` event, target
    `ridl::<catalog>`, `rpc.service = "Cabin"`, the ordinal number, no
    `rpc.method`, `outcome = "Contract::UnknownInteraction"`.
  - `a_claim_for_another_interface_is_a_warning` — same, with the interface
    number.
  - `undecodable_arguments_are_a_warning` — `WARN`, `rpc.method = "setLevel"`,
    the decode error's outcome.
  - `a_short_claim_is_an_error` — `ERROR`, `outcome = "Transport::Corrupt"`.
  - `a_broken_ensure_is_an_error` — `ERROR`, `rpc.method = "average"`,
    `outcome = "Contract::ContractBroken"`.
  - `a_refused_settlement_is_a_warning` — `WARN`.
  - `a_failed_require_is_not_an_event` — no event; the claim span has
    `outcome = "Contract::PreconditionFailed"`.
  - `the_command_claim_span_covers_the_handler` — inside the provider's
    `set_level`, `Span::current()` is the `"Cabin/setLevel"` claim span; the
    span closes after the handler returns.

- [ ] **Step 2: Run** `cargo test -p ridl-backend-rust --test observe_dispatch`.
      Expected: FAIL.

- [ ] **Step 3: Implement** in `dispatch`: a claim span per member arm, opened
      when the claim is read and held until the provider method returns; the
      events at the points and levels of the D-5 table. The arms with no member
      (`_ =>`, other interface, `ShortClaim`) use only the interface's literals
      and the ordinal.

- [ ] **Step 4: Regenerate and run** `cargo test -p ridl-backend-rust`.
      Expected: PASS, including the existing `dispatch_generation.rs` text tests
      (update a text assertion only where the inserted calls change the text it
      matches, and say so in the commit body).

- [ ] **Step 5: Commit.**
      `feat(ridl-backend-rust): trace claims and report dispatch failures`

### Task 6: Raise and occurrence spans

**Files:**

- Modify: `crates/ridl-backend-rust/src/observe.rs`,
  `crates/ridl-backend-rust/src/face.rs` (`publisher`, line 797),
  `crates/ridl-backend-rust/src/face/poll.rs` (`poll_next_event`, line 187),
  `crates/ridl-backend-rust/src/face/futures.rs` (`next_event_future`, line 232)
- Modify: `crates/ridl-backend-rust/tests/observe_spans.rs`

**Interfaces:**

- Produces in `__observe`: `macro_rules! raise_span` and `occurrence_span`
  (literals `target`, `name`, `destination`, `catalog`; `occurrence_span!` also
  takes `received`); `fn raise_outcome(&Result<(), RaiseError>)` and
  `fn read_outcome<T>(&Result<T, ReadError>)`, each `-> &'static str`.

- [ ] **Step 1: Write the failing tests** in `observe_spans.rs`:

  - `a_raise_opens_a_publish_span` — `Publisher` raises `Cabin.warning`: one
    span `"publish Cabin.warning"`, `messaging.system = "ridl"`,
    `messaging.destination.name = "Cabin.warning"`,
    `messaging.operation.type = "send"`, `outcome = "ok"`.
  - `an_occurrence_opens_a_process_span` — `next_event` receives it: one span
    `"process Cabin.warning"`, `messaging.operation.type = "process"`,
    `outcome = "ok"`, closed before the future returns the event.
  - `an_unreadable_occurrence_records_its_read_error` — an occurrence that does
    not decode: the span's outcome is the error's name.
  - `a_signal_opens_no_span` — a signal write and read record no span.

- [ ] **Step 2: Run** the file. Expected: FAIL.
- [ ] **Step 3: Implement** around `EventSink::raise` in `publisher` and around
      the read and decode in `poll_next_event`.
- [ ] **Step 4: Regenerate and run** `cargo test -p ridl-backend-rust`.
      Expected: PASS.
- [ ] **Step 5: Commit.**
      `feat(ridl-backend-rust): trace raises and occurrences`

### Task 7: Trace-context propagation and recording

**Files:**

- Modify: `crates/ridl-backend-rust/src/observe.rs`,
  `crates/ridl-backend-rust/src/face/poll.rs` (`send`, line 104),
  `crates/ridl-backend-rust/src/face.rs` (`publisher`, line 862),
  `crates/ridl-backend-rust/src/face/dispatch.rs`
- Create: `crates/ridl-backend-rust/tests/observe_propagation.rs`,
  `crates/ridl-backend-rust/tests/observe_propagation_unset.rs`
- Modify: `crates/ridl-backend-rust/tests/otel_propagation.rs`

**Interfaces:**

- Consumes: `ridl_rt::trace::{Propagation, set_propagation, propagation}` (Task
  2), and the call order recorded in D-6 by Task 1.
- Produces in `__observe`:
  - `fn current_trace() -> Option<TraceContext>` — with `trace-context`,
    `propagation().and_then(|p| p.current())`; without, `None`.
  - `pub(crate) struct ClaimGuard` and
    `fn enter_claim(received: Option<TraceContext>) -> ClaimGuard` — calls
    `enter` in the order of D-6; `Drop` calls `leave` once, including during
    unwinding. Without `trace-context`, both are zero-sized and call nothing.
  - `struct Hex<'a>(&'a [u8])` with a `Display` that writes lowercase
    hexadecimal byte by byte, under `#[cfg(feature = "tracing")]`.
  - `claim_span!`, `occurrence_span!`, `dispatch_warn!` and `dispatch_error!`
    record `trace.trace_id`, `trace.parent_span_id`, `trace.flags` through `Hex`
    when `received` is `Some`.

- [ ] **Step 1: Write the failing tests.** `observe_propagation.rs` (its own
      binary, because the hook is global) registers a recording hook whose
      `current()` returns a context derived from a per-thread counter and which
      logs every call in order:

  - `the_context_travels_a_to_b_to_c` — client A calls `Cabin.setLevel` on
    provider B; B's handler calls `Valve.open` on provider C. Assert: the claim
    B reads has `trace == A's current()`; B's `enter` received it; the claim C
    reads has `trace ==` the value `current()` returned inside B's handler; the
    log ends with `leave` for C, then `leave` for B.
  - `a_raise_sends_the_current_context` — the occurrence read by the consumer
    has `trace == current()` at raise time; `enter` is not called.
  - `leave_runs_when_the_handler_panics` — a provider that panics in
    `set_level`; `serve` under `std::panic::catch_unwind`: the panic propagates,
    `leave` ran once.
  - `the_claim_span_records_the_received_context` — with the recorder:
    `trace.trace_id` is the 32-character lowercase hexadecimal of the sent id,
    `trace.parent_span_id` 16 characters, `trace.flags` 2 characters.

  `observe_propagation_unset.rs`:

  - `no_hook_sends_none` — no `set_propagation`: claims and occurrences carry
    `trace == None`, and the claim span has no `trace.*` field.

  `otel_propagation.rs`:

  - `generated_face_links_otel_parents` — drive the generated face over loopback
    with the OpenTelemetry hook of Task 1 and an application span current around
    `serve`: the exported claim span's parent is the call span.

- [ ] **Step 2: Run** the three binaries. Expected: FAIL (`None` is sent).
- [ ] **Step 3: Implement.** Replace the `None` at `poll.rs:104` and
      `face.rs:862` with `crate::__observe::current_trace()`, evaluated inside
      the entered call or raise span. In `dispatch`, hold
      `crate::__observe::enter_claim(claim.trace)` around the handler in each
      member arm, ordered with the claim span as D-6 records.
- [ ] **Step 4: Regenerate and run** `cargo test -p ridl-backend-rust`, then
      `cargo test -p ridl-backend-rust --test face_compile`. Expected: PASS in
      every cfg combination.
- [ ] **Step 5: Commit.**
      `feat(ridl-backend-rust): call the propagation hook from the generated face`

### Task 8: The demo with observation on

**Files:**

- Create: `examples/cabin/observer/Cargo.toml`,
  `examples/cabin/observer/src/main.rs`
- Modify: `examples/cabin/Cargo.toml` (members), `examples/cabin/Cargo.lock`,
  `justfile` (`demo`)

**Decision (D-8 leaves it to the plan):** a second consumer package, `observer`,
not a variant of `consumer`. `consumer` keeps proving the features-off build,
and `cargo run -p` builds only the selected package's graph, so the two feature
sets do not unify. `observer` depends on `veh_cabin` with
`features = ["std", "tracing", "trace-context"]`, on `tracing`, and on
`tracing-subscriber`.

- [ ] **Step 1: Write `observer`.** It installs a recording subscriber and a
      hook that returns a fixed context, runs one command and one query round
      trip over `ridl-loopback`, and asserts one call span and one claim span
      per round trip with the identity fields and `outcome = "ok"`, and that
      each claim carried the fixed context. It prints `span ok <name>` for each
      asserted span.
- [ ] **Step 2: Extend `just demo`** with `cargo fmt --check` and
      `cargo clippy -- -D warnings` on `observer`,
      `cargo run --manifest-path examples/cabin/Cargo.toml -p observer --locked`,
      and `grep -qxF` for each `span ok` line. A missing line or a non-zero exit
      fails the recipe.
- [ ] **Step 3: Run** `just demo` and `just compat-check`. Expected: both PASS;
      the compat cell still builds the generated crate with the features off.
- [ ] **Step 4: Commit.** `test(repo): run the cabin demo with observation on`
      (`.git-std.toml` has no `examples` scope).

### Task 9: Records and the book

**Files:**

- Modify: `docs/decisions/ADR-0023-interaction-face-generation.md` (the
  amendment at line 133 and decision 6),
  `docs/decisions/ADR-0021-ridl-rt-0.1-api-and-release.md` (decision 21, the
  bullet "Generated code" at line 933), `docs/design/interaction-face.md` (new
  section "Observation"; "The consumer face", "The provider face", the
  settlement table: the event and level per row)
- Create: `docs/book/observing.md`; Modify: `docs/book/SUMMARY.md`

- [ ] **Step 1: Write the records** as built: the two features, the D-3 field
      and diagnosis settings, the span and event tables, the `__observe` module
      and why it is not named `observe`, and the call order from D-6.
- [ ] **Step 2: Write the book chapter:** enabling the features, the two
      settings, the three use cases, an example `Propagation` implemented with
      OpenTelemetry (a `rust` fence, taken from `otel_propagation.rs`), and the
      limit for events (a call made in reaction to an event is not linked to its
      trace).
- [ ] **Step 3: Run**
      `just book-check link-check doc-path-check story-id-check check`.
      Expected: PASS.
- [ ] **Step 4: Commit.**
      `docs(docs): document observation from the generated face`

**End of PR B.** Run `just verify`, then garden `docs/wip/` with `sdd-gardening`
before the PR merges (the spec and this plan move to `docs/archive/`).
