# On-request encodings, sub-stage 1b — implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** The generated Rust face reads its payload encoding from its port type
(`P: Encoded`, `P::Encoding`) and names no encoding itself; `ridl-loopback` is
typed by the encoding it carries, with FlatBuffers as its default; application
code is unchanged.

**Architecture:** `ridl_rt::port::Encoded` exists since sub-stage 1a and no port
implements it. This plan makes `Loopback<E = FlatBuffers>`, `HandlerHandle<E>`
and `Handles<E>` implement it (Task 1); gives every hand-written port of the
test suite an `Encoded` implementation and every generic consumer of the face
the bounds the generic face needs (Task 2); then replaces the thirteen
`::ridl_rt::encoding::FlatBuffers` paths of the face emitter by `P::Encoding`
and adds the generated `where` clauses (Task 3), proved by a second codec over
`Loopback::<Proto3>` beside the FlatBuffers default. The rest is proof (Task 4),
records (Tasks 5, 7), the Kotlin heads-up (Task 6) and gardening (Task 8).
`ridl-rt`'s API does not change; two rustdoc sentences in it do.

**Tech Stack:** Rust workspace (pinned toolchain in `rust-toolchain.toml`),
`quote` + `prettyplease` in the backend, `insta` snapshots, bare `rustc` compile
proofs, `cargo xtask freeze update`, `just` recipes as the gate.

**Spec:** `docs/wip/2026-10-10-on-request-encodings-1b-design.md` (sections 7,
6.3, 9.3, 10, 11, 13 and 15; DD-S1-11 and DD-S1-12 are decided). Context:
`docs/archive/2026-10-10-on-request-encodings-design.md` sections 2, 6.1 and
6.3. The release frame is `docs/wip/2026-10-11-release-0.10-design.md` and the
lane C brief in `docs/wip/2026-10-11-release-0.10-lanes-plan.md`.

## Global Constraints

- Every change to the emitted code compiles under the codegen build matrix of
  ADR-0021 decision 10: edition 2021 with Rust 1.83, edition 2021 with the pin,
  edition 2024 with the pin. `just compat-check` runs in Tasks 1, 3 and 4. Every
  construct used here (a defaulted type parameter, an associated type projection
  in a `where` clause, `PhantomData`) is stable in Rust 1.83.
- `ridl-rt` keeps `rust-version = "1.83"`, no dependency, default features off,
  and gains no item, no signature change and no trait item (spec section 13: "No
  `ridl-rt` change"). Two rustdoc sentences in `crates/ridl-rt/src/port.rs` are
  rewritten by Task 5; `cargo semver-checks` sees no change.
- `ridl-rt-conformance` is not edited (spec section 6.3: its `Factory` never
  encodes a payload).
- `examples/cabin/consumer/src/main.rs` is not edited by any task; `just demo`
  and `crates/ridlc/tests/cabin_example.rs` prove it compiles and runs with no
  edit (spec section 11).
- Encoding names stay `flatbuffers`, `proto3`, `repr-c`; the three marker types
  of `ridl_rt::encoding` are the only encodings, and `Proto3` and `ReprC` are
  not feature-gated, so a test can name them with the `flatbuffers` feature
  alone.
- The break: the pull request title carries `!`
  (`feat!: the generated face reads its encoding from the port, and ridl-loopback is typed by it`),
  because the generated face gains a bound (`P: Encoded`) and `ridl-loopback`'s
  two public types gain a type parameter. Both surfaces are frozen from 0.10.0
  (ADR-0029). Neither `generated-crate` nor `runtime-crates` has a row in
  `freeze/surfaces.toml` on main today, so no baseline moves in this pull
  request; `freeze/generated/cabin/` and `freeze/generated/veh-cluster/` are
  regenerated with `cargo xtask freeze update` (Task 3), and `just freeze-check`
  fails until they are committed. The lane opens the `generated-crate` row
  later, when the last pending break of that surface merges (lanes plan section
  3).
- Prose (rustdoc, comments, commit messages, records) is plain literal English.
  No story id, stage or lane name in any file under `crates/`, `examples/`,
  `docs/book/`, `docs/design/` or `docs/technotes/` (`just story-id-check`);
  "sub-stage 1b" is such a name: shipped files say "the generic face" or "since
  2026-10-xx" instead.
- Commits follow `.git-std.toml`: `<type>(<scope>): <summary>`; scopes used here
  are `ridl-loopback`, `ridl-backend-rust`, `ridlc`, `ridl-rt`, `docs`, `adr`.
  End each commit message with the attribution line the harness gives the
  implementer.
- Each task's implementer runs `just compile`, `just test`, `just lint` and
  `just fmt-check` before handing back; the task lists the other gate members it
  touches. The final gate is `just build`. The reviewer gets the task's diff as
  a file, computed from the base recorded before the task started, and returns
  two verdicts (meets its brief; code is sound), per the task review gate.
- Model tier per task: **fable** for a task marked subtle, **sonnet** for a task
  marked mechanical.
- Ordering: Task 7 (the ADR-0021 rewrite) is gated on lane W's binding document
  being on `main` (lanes plan section 2 and section 5.8 of the release design).
  Tasks 1 to 6 run before it in order; Task 8 runs last.
- Review focus items (below) each name the task that pins them.

## Review Focus

1. A port passed by `&mut` carries its encoding through the forwarding impl: the
   cabin program serves over `&mut *handler` where `handler` is a
   `HandlerHandle`, and the Proto3 test serves over
   `&mut HandlerHandle<Proto3>`. A face that bounds `H: Encoded` but reads
   `<&mut H>::Encoding` wrongly fails to compile. Pinned in Task 1
   (`a_mut_borrow_of_a_handle_is_encoded`) and Task 3 (the Proto3 round trip
   serves over `&mut handler`).
2. An interface that has no event, no call or no signal (the fixture's `Horn`,
   `Siren`, `Valve`, `Wiper`, `Zone`) gets a `where` clause that names only the
   payload types it uses and never an `EVENT_BUFFER` or `CLAIM_BUFFER` it does
   not hold. Pinned in Task 3 (`a_signal_only_face_bounds_only_its_own_payload`
   and the fixture compiling).
3. Two encodings coexist in one crate: a `Client` over `Loopback::new` and a
   `Client` over `Loopback::<Proto3>::with_encoding`, from the same generated
   module, in one test binary, and the default is FlatBuffers. Pinned in Task 3.
4. A port whose encoding the package does not carry is refused at the binding
   with a diagnostic that names `Sizes<`, not somewhere inside a call. Pinned in
   Task 4 (`a_port_whose_encoding_the_package_lacks_is_refused_at_the_binding`).
5. A consumer that is generic over the port must restate the bounds (spec
   section 7.2) and then compiles; a consumer over a concrete port writes none.
   Pinned in Task 2 (the `face_compile.rs` consumers) and Task 4 (the cabin
   program, unedited).

---

### Task 1: `ridl-loopback` is typed by its encoding — fable

**Files:**

- Modify: `crates/ridl-loopback/src/lib.rs` (crate docs 22-55 "The handles";
  `Handles` 134-150; `Loopback` 151-163 and its `impl` 165-340; the twelve port
  impls 344-515 become `impl<E: Encoding> ... for Loopback<E>`; the assertions
  516-533)
- Modify: `crates/ridl-loopback/src/handle.rs` (`HandlerHandle` 541-547 and its
  five impls 548-635; the `ReaderHandle` doc at 112-116, whose sentence "a face
  that needs only `SignalReader` can be built over this handle directly" is no
  longer true, see "Deviations" D-4)
- Modify: `crates/ridl-loopback/tests/conformance.rs` (unchanged in text:
  `type Runtime = Loopback;` and `type Handler = HandlerHandle;` resolve through
  the defaults; the task proves it by running it)
- Test: `crates/ridl-loopback/tests/encoding.rs` (new)
- Modify: `docs/design/ridl-loopback.md` ("The handles" section, line 99 on: one
  paragraph for the type parameter and the default; "The crate and its
  dependencies", line 36: the loopback's default encoding and why FlatBuffers)

**Interfaces:**

- Consumes: `ridl_rt::port::Encoded`,
  `ridl_rt::encoding::{Encoding, FlatBuffers}`.
- Produces (spec sections 6.3 and 7.1):

```rust
pub struct Handles<E: Encoding = FlatBuffers> { pub reader: ReaderHandle, pub writer: WriterHandle,
    pub source: SourceHandle, pub sink: SinkHandle, pub caller: CallerHandle, pub handler: HandlerHandle<E> }

pub struct Loopback<E: Encoding = FlatBuffers> { shared: Shared, catalog: CatalogRef, handles: Handles<E> }

impl Loopback<FlatBuffers> {
    /// FlatBuffers, the loopback's default: both ends are one process.
    pub fn new(catalog: CatalogRef) -> Self;
}
impl<E: Encoding> Loopback<E> {
    pub const SLOTS: usize = 16;
    /// A runtime whose ports carry `E`.
    pub fn with_encoding(catalog: CatalogRef) -> Self;
    pub fn split(self) -> Handles<E>;
    pub fn attach(&self) -> Loopback<E>;
    pub fn handler(&self) -> HandlerHandle<E>;
    // reader, writer, source, sink, caller, advance, provision_fixed, fail_next_settle: unchanged signatures
}
impl<E: Encoding> Encoded for Loopback<E> { type Encoding = E; }

pub struct HandlerHandle<E: Encoding = FlatBuffers> { /* as today */ encoding: PhantomData<E> }
impl<E: Encoding> Encoded for HandlerHandle<E> { type Encoding = E; }
```

`new` is defined in the `impl Loopback<FlatBuffers>` block, which is what lets
`Loopback::new(catalog)` infer `E` with no annotation (spec section 14, the
default-type-parameter bullet). `with_encoding` is the one other constructor;
`new` calls `Loopback::<FlatBuffers>::with_encoding`. The five other role
handles are unchanged (design 6.3 names `Loopback<E>` and `HandlerHandle<E>`
only; see D-4). `Send` and `Sync` are as today: `PhantomData<E>` of a unit
marker type adds no restriction, and the assertions at the bottom of `lib.rs`
keep pinning them, with `assert_send::<Loopback<Proto3>>()` added.

- [ ] **Step 1: Write the failing tests** in
      `crates/ridl-loopback/tests/encoding.rs`, with
      `fn encoding_of<P: Encoded>(_: &P) -> &'static str { P::Encoding::NAME }`
      and the `catalog()` helper copied from `tests/ports.rs`:
  - `the_default_encoding_is_flatbuffers`:
    `encoding_of(&Loopback::new(catalog())) == "flatbuffers"`, and the same for
    `rt.attach()`, `rt.handler()` and `rt.split().handler`.
  - `with_encoding_types_every_handle_that_meets_a_face`:
    `let rt = Loopback::<Proto3>::with_encoding(catalog());` — `encoding_of` is
    `"proto3"` for `&rt`, `&rt.attach()`, `&rt.handler()` and
    `&rt.split().handler` (so an `attach` or a `handler` that drops `E` fails).
  - `a_mut_borrow_of_a_handle_is_encoded`: `encoding_of(&&mut rt)` and
    `encoding_of(&&mut rt.handler())` are `"proto3"` (review focus 1).
  - `the_encoding_is_a_type_and_not_a_value`: a `Loopback::<Proto3>` and a
    `Loopback::new` built over the same catalog publish and read independently
    of each other (each is its own store); `Loopback::<ReprC>::with_encoding`
    compiles and reports `"repr-c"`.
  - `the_conformance_factory_still_names_the_default`: no new code; the existing
    `tests/conformance.rs` is the test (Step 4 runs it).
- [ ] **Step 2: Run** `cargo test -p ridl-loopback --test encoding` — expected:
      fails to compile, `with_encoding` not found and `Loopback` takes no
      generic argument.
- [ ] **Step 3: Implement** per the Interfaces block. Rustdoc: on `Loopback`,
      the sentence that the type parameter is the encoding its ports carry, the
      default is FlatBuffers because both ends are one process, and
      `with_encoding` names another (spec section 7.1's table, stated as the
      loopback's own row; the WebSocket and shared-memory rows are not built and
      are not mentioned here). On `Encoded for Loopback<E>`: one session has one
      encoding; an aggregate from `attach` carries the same `E`. Update the
      crate docs' "The handles" paragraph (the aggregate "is what a generated
      `Client`, `Publisher` or `dispatch` is normally built over" — add: "and it
      states the encoding the face encodes with"). Rewrite the `ReaderHandle`
      doc sentence per D-4's chosen resolution.
- [ ] **Step 4: Run** `cargo test -p ridl-loopback` (including
      `tests/conformance.rs` and `tests/ports.rs`, both unedited) and
      `RUSTDOCFLAGS=-Dwarnings cargo doc -p ridl-loopback --no-deps` — expected:
      PASS, no warning.
- [ ] **Step 5: Records**: `docs/design/ridl-loopback.md`, the two passages
      named in Files; keep the record as-built (no "later" and no lane name).
- [ ] **Step 6: Gate**: `just compile`, `just test`, `just lint`,
      `just compat-check` (it packages `ridl-loopback`'s consumer crate),
      `just demo` (the cabin program names `Loopback` and `HandlerHandle` bare:
      the defaults must resolve), `just check`, `just link-check`.
- [ ] **Step 7: Commit**:
      `feat(ridl-loopback)!: type the runtime by the encoding its ports carry` —
      body: `Loopback<E = FlatBuffers>`, `HandlerHandle<E>`, `Handles<E>`,
      `with_encoding`; `new` stays FlatBuffers; a program that names the types
      bare is unchanged.

---

### Task 2: every hand-written port states its encoding, and every generic consumer restates the bounds — sonnet

Additive: `Encoded` exists and the face does not require it yet, so each change
here compiles against the face as built, and Task 3's emitter change then finds
every port and every consumer ready. A generic function may carry a bound its
callers do not need.

**Files:**

- Modify: `crates/ridl-backend-rust/tests/interaction_face.rs`
  (`ReportsInitOverRealBytes` 382-416, `MinimalSignalOnlyPort` 2589-2625,
  `DistinctiveInitPort` 2656-2700: each gains
  `impl ridl_rt::port::Encoded for X { type Encoding = ridl_rt::encoding::FlatBuffers; }`
  beside its `Attached` impl)
- Modify: `crates/ridl-backend-rust/tests/support/doubles.rs` (`RecordingPorts`
  133, `FailingHandler` 279, `QueuedClaims` 420: the same impl beside each
  `Attached` impl, with a comment that the double carries the encoding of the
  `Loopback` handles it wraps)
- Modify: `crates/ridl-backend-rust/tests/face_compile.rs` (`CLIENT_BOUNDS`
  531-535 and `PUBLISHER_BOUNDS` 538 gain `::ridl_rt::port::Encoded`; a new
  `const FACE_WHERE: &str` for `fixed_name_face`'s payload set; every generic
  consumer function — lines 554, 560, 566, 590, 614, 621, 651, 657, 690, 697,
  722, 750, 758, 785, 847, 862, 873, 887, 905, 959, 968 — moves its bounds to a
  `where` clause that carries the predicates below)
- Test: the files above are tests; the new assertion is in
  `interaction_face.rs`.

**Interfaces:**

- Produces, for a consumer generic over a port `P` of interface `Cabin` whose
  payloads are `Level`, `Temperature` and `Warning` (the `fixed_name_face`
  fixture), the `where` clause the generic face will require (spec section 7.2):

```rust
where
    P: ::ridl_rt::port::Encoded + /* the port traits as today */,
    super::Cabin: ::ridl_rt::contract::Sizes<P::Encoding>,
    super::Level: ::ridl_rt::payload::Payload<P::Encoding>,
    super::Temperature: ::ridl_rt::payload::Payload<P::Encoding>,
    super::Warning: ::ridl_rt::payload::Payload<P::Encoding>,
```

For the fixtures of `a_consumer_of_two_preludes_compiles` (847-905) and
`a_prelude_brings_only_its_own_interfaces_traits` (959-968) the interface and
the payload set are each fixture's own; the implementer reads them from the
fixture source in the same test.

- [ ] **Step 1: Write the failing test** in `interaction_face.rs`:
      `every_hand_written_port_states_its_encoding`: with
      `fn encoding_of<P: ridl_rt::port::Encoded>() -> &'static str { P::Encoding::NAME }`,
      assert `"flatbuffers"` for `ReportsInitOverRealBytes`,
      `MinimalSignalOnlyPort`, `DistinctiveInitPort`, `doubles::RecordingPorts`,
      `doubles::FailingHandler` and `doubles::QueuedClaims`.
- [ ] **Step 2: Run**
      `cargo test -p ridl-backend-rust --test interaction_face every_hand_written_port`
      — expected: fails to compile, `Encoded` not implemented for the six types.
- [ ] **Step 3: Implement** the six impls and the `face_compile.rs` clauses.
- [ ] **Step 4: Run**
      `cargo test -p ridl-backend-rust --test interaction_face --test face_compile`
      — expected: PASS (the face as built accepts a consumer with more bounds).
- [ ] **Step 5: Gate**: `just compile`, `just test`, `just lint`,
      `just fmt-check`.
- [ ] **Step 6: Commit**:
      `test(ridl-backend-rust): give every hand-written port an encoding and every generic consumer the face's bounds`.

---

### Task 3: the face reads its encoding from the port — fable

One atomic change across the emitter, the checked-in fixture, the `ridlc` corpus
snapshots and the freeze dumps: the fixture is `include!`d by the face tests, so
the emitter change and the regeneration land together.

**Files:**

- Modify: `crates/ridl-backend-rust/src/face.rs` (`client_bounds` 358-387;
  `client` 387-570, the `pub struct Client` at 550 and the `Bind` impl at 556,
  the `Events`, `Subscribe` and inherent impls at 509-530; `publisher` 860-998,
  the struct at 977 and the `Bind` impl at 985; the `verify` at 438;
  `payload_buffer` 1138-1145; `encode_into` 1164-1193, whose `unreachable!`
  message at 1171-1174 names `Payload<FlatBuffers>`; `decode_args` 1195-1222)
- Modify: `crates/ridl-backend-rust/src/face/poll.rs` (`send` 83, `poll_ack`
  110, `poll_reply` 132 and its `verify` at 148, `poll_next_event` 169-171 and
  180, 199)
- Modify: `crates/ridl-backend-rust/src/face/serve.rs` (the bounds at 33, the
  doc string at 64 "`<{iface_name} as Sizes<FlatBuffers>>::MAX_BUFFER_SIZE`",
  `serve` 86-104, `Serve` 107-113, the `Unpin` and `Future` impls 126-128)
- Modify: `crates/ridl-backend-rust/src/face/dispatch.rs` (comments 119, 125,
  138; `dispatch` 162-177)
- Modify: `crates/ridl-backend-rust/src/face/futures.rs` (`call_future` bounds
  52-54, struct 135, impls 148, 159, 214; `NextEvent` bounds 228, struct 240,
  impl 244)
- Modify: `crates/ridl-backend-rust/src/face/blocking.rs` (61, impls 201-284,
  `serve` 379-386)
- Modify: `crates/ridl-backend-rust/tests/generated/interaction_face.rs`
  (regenerated with
  `RIDL_UPDATE_GENERATED=1 cargo test -p ridl-backend-rust --test interaction_face_regeneration`)
- Modify: `crates/ridlc/tests/snapshots/corpus__rust@veh-cluster.snap` and
  `corpus__rust@rsdl-appendix-a.snap` (re-recorded with `cargo insta` and
  inspected: the diff is the bounds, the `where` clauses and the paths, and
  nothing else)
- Modify: `freeze/generated/cabin/{lib.rs,veh.cabin.rs}` and
  `freeze/generated/veh-cluster/{lib.rs,ridl.std.rs,veh.cluster.rs}`
  (regenerated with `cargo xtask freeze update`; `Cargo.toml` of each is
  unchanged)
- Test: `crates/ridl-backend-rust/tests/interaction_face.rs`,
  `crates/ridl-backend-rust/tests/interaction_face_regeneration.rs`,
  `crates/ridl-backend-rust/tests/dispatch_generation.rs` (its tests pass a
  `Loopback` handler: unchanged in text, run as proof)

**Interfaces:**

- Consumes: `Encoded` on `Loopback<E>` and `HandlerHandle<E>` (Task 1); the
  `Encoded` impls of the fake ports (Task 2).
- Produces, in `face.rs` (private helpers; the names are the ones every file of
  `face/` calls):

```rust
/// The payload types an interface's face encodes or decodes: the signal and
/// event payloads, then each call's argument and reply, in declaration order,
/// each once.
fn payload_types<'a>(signals: &[(Member<'a>, &'a str)], events: &[(Member<'a>, &'a str)],
    commands: &[Call<'a>], queries: &[Call<'a>]) -> Vec<&'a str>;

/// The predicates that tie the face to the encoding of port `port`:
/// `super::#iface: ::ridl_rt::contract::Sizes<#port::Encoding>` and one
/// `#T: ::ridl_rt::payload::Payload<#port::Encoding>` per entry of `payloads`.
/// Written as a `where` clause on every impl and every free function of the
/// face (DD-S1-12); a struct carries only the port bounds, except `Serve`,
/// whose claim-buffer field projects through `Sizes`.
fn encoding_bounds(port: &Ident, iface: &Ident, payloads: &[&str]) -> TokenStream;

fn payload_buffer(type_name: &str, port: &Ident) -> TokenStream;   // <#path as Payload<#port::Encoding>>::BUFFER
fn encode_into(type_name: &str, value: TokenStream, target: TokenStream, target_name: &str, port: &Ident) -> TokenStream;
fn decode_args(type_name: &str, port: &Ident) -> TokenStream;
```

`client_bounds` and `publisher`'s bound list start with
`::ridl_rt::port::Encoded`; `serve.rs`'s bounds become
`H: ::ridl_rt::port::Encoded + ::ridl_rt::port::Handler + ::ridl_rt::port::Wakeable, P: Provider`;
`dispatch`'s `H: ::ridl_rt::port::Encoded + ::ridl_rt::port::Handler`; the
`futures.rs` bounds `::ridl_rt::port::Encoded + ::ridl_rt::port::Caller + ...`
and
`::ridl_rt::port::Encoded + ::ridl_rt::port::EventSource + ::ridl_rt::port::Wakeable`;
the `poll.rs` functions
`<P: ::ridl_rt::port::Encoded + ::ridl_rt::port::Caller>` (and `EventSource` for
`poll_next_event`). Every `::ridl_rt::encoding::FlatBuffers` in these six files
becomes `P::Encoding`, `W::Encoding` or `H::Encoding`, the parameter of the item
it sits in. The emitted shape is spec section 7.1's listing: `Client<P>`,
`Publisher<W>`, `serve<H, P>`, `Serve<'a, H, P>`, `blocking::Client<P>`,
`blocking::serve<H, P>`, the three futures, each with the `where` clause;
`Provider`, `Subscribe`, `Invalidate`, the descriptors and the `prelude` do not
change. The doc strings and the `unreachable!` messages that named `FlatBuffers`
say "the port's encoding" instead (the generated docs of ADR-0028 are unchanged
in form; only these generated sentences change).

- [ ] **Step 1: Write the failing tests** in `interaction_face.rs`:
  - `the_face_names_no_concrete_encoding`: from the first line of the fixture
    that starts with `pub mod` to the end (the face modules follow the
    descriptors), the text contains no `encoding::FlatBuffers`, and contains
    `::ridl_rt::port::Encoded`, `P::Encoding`, `W::Encoding` and `H::Encoding`.
  - `a_signal_only_face_bounds_only_its_own_payload` (review focus 2): the text
    of `pub mod horn` contains
    `super::Horn: ::ridl_rt::contract::Sizes<P::Encoding>` and
    `super::Health: ::ridl_rt::payload::Payload<P::Encoding>`, and contains
    neither `EVENT_BUFFER` nor `CLAIM_BUFFER` nor `super::Level:`.
  - `a_second_codec_is_read_from_the_port_type` (spec section 11; review focus 1
    and 3). In the test crate, beside the `generated` module:
    `impl Payload<Proto3>` for each of the fixture's six payload types
    (`Temperature`, `Level`, `Window`, `Average`, `Health`, `Warning`), through
    one macro, with `MAX_SIZE`, `Buffer` and `BUFFER` equal to the FlatBuffers
    ones, `type View<'a> = &'a [u8]`, `encode` that encodes with
    `Ref::<T, FlatBuffers>::encode` into a temporary
    `<T as Payload<FlatBuffers>>::BUFFER` and writes the bytes **reversed** into
    the end of `out` (so `Encoded.bytes` is a subslice, like FlatBuffers),
    `verify` that reverses into a temporary buffer and runs
    `Ref::<T, FlatBuffers>::verify` on it, and `decode` that does the same and
    calls `.decode()` (model: the `Raw` placeholder in
    `crates/ridl-rt/src/payload.rs:277-295`); and
    `impl Sizes<Proto3> for generated::Cabin` whose seven items repeat
    `<Cabin as Sizes<FlatBuffers>>`'s values (the reversed bytes have the same
    length). The test then:
    1. `let rt = Loopback::<Proto3>::with_encoding(CATALOG);` and
       `let mut publisher = generated::cabin::Publisher::new(rt.attach());`,
       `let mut client = generated::cabin::Client::new(rt.attach());` with no
       type annotation; publishes `Temperature::new(21)` and reads it back
       through `client.temperature()`: the value is 21 under `Provenance::Live`.
    2. Reads the raw bytes with
       `rt.read(<Cabin as Interface>::NUMBER, Ordinal(1), &mut out)` through
       `SignalReader` and asserts they equal the FlatBuffers encoding of the
       same value **reversed** (so a face that still encodes a signal with
       FlatBuffers fails here), and are not equal to the FlatBuffers bytes.
    3. `let mut handler = rt.handler();` and
       `let mut serve = generated::cabin::serve(&mut handler, &mut provider);`
       with a `TestProvider`; sends `client.set_level(Level::new(3))` and
       `client.average(Window::new(5))` through the async client, polled the way
       `round_trip_query_reply_is_delivered` polls; the command is acknowledged,
       the reply is the provider's `Average`, and the provider saw level 3 (a
       dispatch that decodes with FlatBuffers settles `Transport::Corrupt` and
       fails).
    4. Beside it, `let rt_fb = Loopback::new(CATALOG);` with a `Publisher` and a
       `Client` over it: the same signal round trip, and its raw bytes are the
       FlatBuffers bytes unreversed (the default is FlatBuffers; both codecs
       coexist in one binary).
  - The existing tests are unchanged in text: the fixture's faces over
    `Loopback` and the doubles resolve `P::Encoding = FlatBuffers`.
- [ ] **Step 2: Run** `cargo test -p ridl-backend-rust --test interaction_face`
      — expected: the three new tests fail
      (`the_face_names_no_concrete_encoding` and
      `a_signal_only_face_bounds_only_its_own_payload` on their string
      assertions; `a_second_codec_is_read_from_the_port_type` on the raw-bytes
      assertion of step 2, because the face still encodes with FlatBuffers); no
      compile error, because Task 2 made every port `Encoded`.
- [ ] **Step 3: Implement** the emitter per the Interfaces block: collect
      `payload_types` once in `one_interface` (face.rs 203-260) and thread the
      list, the interface ident and the port ident to `client`, `publisher`,
      `blocking`, `serve`, `dispatch`, `plumbing` and the futures; regenerate
      the fixture; re-record the two `ridlc` snapshots; run
      `cargo xtask freeze update` and inspect the diff under
      `freeze/generated/`.
- [ ] **Step 4: Run** `cargo test -p ridl-backend-rust` and
      `cargo test -p ridlc` — expected: PASS, including
      `interaction_face_regeneration`, `face_compile` (the consumers of Task 2
      now exercise the clause), `dispatch_generation`, `otel_propagation` and
      `name_collision_compile`.
- [ ] **Step 5: Docs in the emitter**: the module doc of `face.rs` (line 38 on)
      states that the face takes its encoding from its port and names none; the
      `payload_buffer` and `encode_into` docs name `Payload<P::Encoding>`.
- [ ] **Step 6: Gate**: `just compile`, `just test`, `just lint`, `just demo`,
      `just compat-check`, `just freeze-check`, `just check`,
      `just story-id-check`.
- [ ] **Step 7: Commit**:
      `feat(ridl-backend-rust)!: read the face's encoding from its port` — body:
      `P: Encoded`; `Sizes<P::Encoding>` and one `Payload<P::Encoding>` bound
      per payload type as generated `where` clauses; no new type parameter; the
      freeze dumps of the generated crate regenerated.

---

### Task 4: a port whose encoding the package lacks is refused at the binding, and the cabin program is unedited — sonnet

**Files:**

- Modify: `crates/ridlc/tests/cabin_example.rs` (a second test beside
  `the_emitted_cabin_crate_runs_six_round_trips_against_a_consumer`, reusing its
  steps 1 to 3 through a shared helper that returns
  `(out, libs, ridl_rt, ridl_loopback, veh_cabin)`; the `rustc` helper at 80-95
  gains a sibling that returns the `Output` instead of asserting success)
- Test: the same file; `examples/cabin/consumer/src/main.rs` is read, not
  edited.

**Interfaces:**

- Consumes: `Loopback::<ReprC>::with_encoding` (Task 1); the generic face (Task
  3).

- [ ] **Step 1: Write the failing test**
      `a_port_whose_encoding_the_package_lacks_is_refused_at_the_binding` (spec
      section 11's `compile_fail`; see D-3): compile, with the same `rustc`
      invocation as the consumer program, a program written in the test as a
      string:

```rust
use veh_cabin::veh::cabin as api;
use ridl_loopback::Loopback;
use ridl_rt::contract::{CatalogRef, Interface};
use ridl_rt::encoding::ReprC;
const CATALOG: CatalogRef = *<api::Cabin as Interface>::CATALOG;
fn main() {
    let rt = Loopback::<ReprC>::with_encoding(CATALOG);
    let _client = api::cabin::Client::new(rt.attach());
}
```

- [ ] **Step 1b: The assertions** of that test: `!output.status.success()`;
      stderr contains `error[E0277]`, `Sizes<` and `ReprC`, and does not contain
      `Encoded` (the port is `Encoded`; what is missing is the package's table,
      review focus 4). Then the same program with `Loopback::new(CATALOG)`
      compiles (a positive twin, so the negative is not a broken harness).
- [ ] **Step 2: Run** `cargo test -p ridlc --test cabin_example` — expected: the
      new test fails only if Task 3 is not merged; on the branch it passes at
      once, which is the point: this task's own change is the proof, and the
      reviewer checks the negative twin by hand (change `ReprC` to `FlatBuffers`
      in the negative program and see the test fail).
- [ ] **Step 3: Run** `just demo` — expected: the cabin program compiles with no
      edit and prints its six round-trip lines with their values (spec section
      11), and the veh-cluster crate passes its `no_std`, clippy and rustdoc
      checks.
- [ ] **Step 4: Gate**: `just compile`, `just test`, `just lint`, `just demo`,
      `just compat-check` (the generated cabin crate at edition 2021 with Rust
      1.83: the `where` clauses and the projection are stable there).
- [ ] **Step 5: Commit**:
      `test(ridlc): refuse a port whose encoding the package lacks at the binding`.

---

### Task 5: the records describe the generic face as built — sonnet

The spec's "What changed" list names the sentences; ADR-0021 is Task 7 (gated).
The line numbers are those of main at c6cd8126.

**Files:**

- Modify: `docs/decisions/ADR-0018-runtime-core-and-generated-surface.md`,
  decision 15's amendment (445-450): a dated amendment under it saying the face
  reads its encoding from its port as built (`P: Encoded`, `P::Encoding`, the
  generated `where` clauses), the loopback's default is FlatBuffers, and the
  sentence "Until then the face stays monomorphic over FlatBuffers" is
  superseded; the Status block gains the amendment line in the record's form.
- Modify: `docs/decisions/ADR-0023-interaction-face-generation.md`, decision 6's
  amendment (482-486): a dated amendment that `Client<P>`, `blocking::Client<P>`
  and `Publisher<W>` carry `Encoded` first in their bound lists, every impl and
  free function of the face carries the `where` clause (spec section 7.2),
  `Serve<'a, H, P>` carries the `Sizes` predicate on the struct, and the face
  names no encoding; the Status block likewise. Decision 2's note (180-192) is
  unchanged.
- Modify: `docs/design/interaction-face.md`: the section "The encoding and the
  ports" (606-620) is rewritten as built: the face takes the encoding from the
  port, the `where` clause and its cost for a consumer generic over the port,
  the path a buffer is named by (`<T as Payload<P::Encoding>>::BUFFER`,
  `<Iface as Sizes<P::Encoding>>::CLAIM_BUFFER` and `EVENT_BUFFER`), and the
  transport-default rule with the loopback as the one built row and the
  WebSocket and shared-memory rows as the specification (the book rule of
  AGENTS.md: say so where it appears); the `Client<P: ...>` bullet (168-182,
  RA-19 keeps its claim: `Encoded` is a port trait the interface's interactions
  need, because every interaction encodes), the `Publisher<W: ...>` bullet
  (362), the `serve` signature (382) and the blocking listing (521-545) gain
  `Encoded` and the `where` clause.
- Modify: `docs/design/ridl-rt.md`: the `Encoded` paragraph (605-614): drop "In
  this version no port type in this repository implements it, and the generated
  face does not yet require it" for "`ridl-loopback` implements it on
  `Loopback<E>` and `HandlerHandle<E>`, and the generated face requires it"; add
  the transport-default table of spec section 7.1 after it, with the loopback
  row marked as built and the other two rows marked as the specification (#265,
  later).
- Modify: `crates/ridl-rt/src/port.rs`: the module doc (10-13) and the `Encoded`
  rustdoc (44-56): the two sentences that say no port implements it and the face
  does not read it are rewritten to what is built; the `driftsys/ridl#801`
  citation stays as the tracking issue of the design. Rustdoc only; no item
  changes.
- Modify: `docs/book/writing-a-port.md`: the port-traits table (33-60) gains the
  `Encoded` row ("states the encoding of the bytes the port carries; every port
  a face is bound to; a conformance fixture need not"); the sentence at 119 that
  lists what `Loopback` implements gains `Encoded`; a short paragraph under "The
  port traits" shows the impl a port writes
  (`impl Encoded for MyPort { type Encoding = FlatBuffers; }`) and
  `Loopback::<E>::with_encoding` as the loopback's override; the ninth item of
  "What a binding decides" (187, "which payload encoding the path carries") is
  unchanged.
- Modify: `docs/book/cli-reference.md` 832-835: the sentence "The face names
  `::ridl_rt::encoding::FlatBuffers` at each site, and a consumer names the
  encoding from `ridl_rt::encoding`" becomes: the face takes the encoding from
  its port type, and a consumer names an encoding only where it builds the
  runtime (`Loopback::<E>::with_encoding`); `docs/book/generated-code.md` 46-58:
  one sentence beside the `Loopback` mention that its default is FlatBuffers.
- Modify: `docs/decisions/README.md` (248, 298: the summaries of ADR-0020 and
  ADR-0021 stay; ADR-0018's and ADR-0023's summaries gain the 1b amendment
  date), `docs/technotes/walking-skeleton-architecture.md` (the `ridl-loopback`
  row 283-290: typed by its encoding, FlatBuffers by default; the
  `ridl-backend-rust` row 165: the face reads its encoding from the port).

- [ ] **Step 1: Edit** each record in its own amendment convention; keep the
      superseded text and mark it, as the records do; no lane or sub-stage name
      in any of these files.
- [ ] **Step 2: Gate**: `just check`, `just link-check`, `just doc-path-check`,
      `just story-id-check`, `just book-check`,
      `cargo test -p ridl-cli --test book_examples`,
      `RUSTDOCFLAGS=-Dwarnings cargo doc -p ridl-rt --no-deps`.
- [ ] **Step 3: Commit**: one
      `docs(adr): amend ADR-0018 and ADR-0023 for the face that reads its encoding from the port`
      and one
      `docs(docs): describe the generic face and the typed loopback as built`
      (the second carries the `ridl-rt` rustdoc change too, as `docs(ridl-rt)`
      if the reviewer prefers a third commit).

---

### Task 6: the Kotlin heads-up — sonnet

Runs after the pull request exists (lanes plan rule 6: the issue is linked from
the pull request).

**Files:** none in this repository.

- [ ] **Step 1: File** an issue on driftsys/ridlc-gen-kotlin with
      `gh issue create --repo driftsys/ridlc-gen-kotlin`, titled "Heads-up: ridl
      0.10 generates a face that reads its encoding from the port, and
      ridl-loopback is typed by it", in the form of
      driftsys/ridlc-gen-kotlin#83: (1) the generated Rust face gains
      `P: Encoded` and generated `where` clauses, no new type parameter,
      application code unchanged; (2) `ridl-loopback` is
      `Loopback<E = FlatBuffers>` with `with_encoding`; (3) `ridl-rt` is
      unchanged; (4) nothing changes for the plugin's own output unless
      `ridl-rt-kt` mirrors a port-typed face; the pull request number; "tracked
      by driftsys/ridl#801"; never a closing keyword.
- [ ] **Step 2: Record** the issue number in the pull request body and in a
      comment on driftsys/ridl#801.

---

### Task 7: ADR-0021 is amended — sonnet — gated: starts only after lane W's binding document is on main

Lane W's binding document amends ADR-0021 too (the port contract's encoding
rules, #350 items 5 and 12). This task rebases onto it and writes its amendments
beside lane W's, so the two do not conflict (lanes plan section 2; release
design section 5.8).

**Files:**

- Modify: `docs/decisions/ADR-0021-ridl-rt-0.1-api-and-release.md`: the
  `port::Encoded` amendment of decision 11 (657-666 on main at c6cd8126; find it
  by its heading after the rebase): the sentence "The generated face does not
  use `Encoded` yet; the part that makes the face read its encoding from the
  port is later work, and until it lands no port in this workspace implements
  the trait" becomes a dated amendment stating what is built (`Loopback<E>`,
  `HandlerHandle<E>`, the face's bound); decision 19's amendment (925-927): "and
  will read its encoding from its port through `port::Encoded` in later work"
  becomes "and reads its encoding from its port through `port::Encoded` since
  2026-10-xx"; decision 10's note (549-552): the generic face and the typed
  loopback are a second break of the generated face and of `ridl-loopback`,
  shipped in 0.10.0, the first frozen release (D-1); the Status block's
  2026-10-10 amendment paragraph (146-153) gains one sentence for it.

- [ ] **Step 1: Rebase** the branch onto `main` and confirm lane W's binding
      document is in `git log main -- docs/decisions/ADR-0021-*`; if it is not,
      stop: this task has not started.
- [ ] **Step 2: Edit** the four passages.
- [ ] **Step 3: Gate**: `just check`, `just link-check`, `just doc-path-check`,
      `just story-id-check`.
- [ ] **Step 4: Commit**:
      `docs(adr): amend ADR-0021 for the face that reads its encoding from the port`.

---

### Task 8: gardening — the sdd-gardening skill

Runs last, after Task 7, before the pull request leaves draft. If lane W's
binding document is late, the pull request waits as a draft with Tasks 1 to 6
done; see the question in "Deviations", Q-2.

- [ ] **Step 1: Run** the `sdd-gardening` skill over
      `docs/wip/2026-10-10-on-request-encodings-1b-design.md` and
      `docs/wip/2026-10-11-on-request-encodings-1b-plan.md`: both move verbatim
      to `docs/archive/`; the durable records are the amendments of Tasks 5 and
      7 and the design records they name, so the skill adds no new record and
      links to those; `docs/wip/README.md`'s paragraph on the on-request
      encodings archive gains the 1b sentence; the `docs/wip/` reference in
      `docs/design/interaction-face.md` (if Task 5 left one) is removed.
- [ ] **Step 2: Check citations**:
      `grep -rn 'on-request-encodings-1b' crates docs examples` finds only
      `docs/archive/` and `docs/wip/README.md` (the gardening lesson: an
      archived file leaves dead paths in source comments).
- [ ] **Step 3: Gate**: `just build` (the full gate); then `/review` passes 1
      and 2 (lanes plan rule 3), with the per-task reviews already recorded.
- [ ] **Step 4: Commit**:
      `docs(docs): archive the on-request encodings 1b design and plan`.

---

## Deviations from the design

Found while checking the design (written 2026-10-10, before 0.9.0 and spec 2b
merged) against main at c6cd8126. None reopens DD-S1-11 or DD-S1-12.

- **D-1 — the release.** The design targets "both in the 0.8.0 release" (read
  0.9.0) and says that if 1b slips, the face's `Encoded` bound "becomes a second
  pre-1.0 break". 0.9.0 was released on 2026-10-11 (#862) without 1b, so that
  second break is what this plan makes, and it ships in 0.10.0 (R-7). The
  records of Tasks 5 and 7 say 0.10.0; the generated face and `ridl-loopback`
  are frozen surfaces with no baseline row yet, so no row moves (Global
  Constraints).
- **D-2 — more ports and consumers than section 9.3 names.** Section 9.3 names
  three fake ports. The three doubles of `tests/support/doubles.rs` also bind
  faces, and about twenty generic consumer functions in `face_compile.rs` must
  restate the bounds (the cost section 7.2 names). Resolution: Task 2 covers all
  of them, additively, before the emitter changes.
- **D-3 — the `compile_fail` doctest has no home.** No library crate can hold a
  doctest that names both a generated face and `ridl-loopback`: the generated
  crate has no doctests, and `ridl-loopback` cannot name a face. Resolution: the
  same proof as a `rustc`-driven negative compile in
  `crates/ridlc/tests/cabin_example.rs`, which already builds `ridl-loopback`
  and the emitted cabin crate with `rustc`, over the real
  `Loopback::<ReprC>::with_encoding` (Task 4). The assertion is the design's:
  the error names `Cabin: Sizes<ReprC>`.
- **D-4 — which handles carry `E`.** Section 6.3 types `Loopback<E>` and
  `HandlerHandle<E>` only. `Handles` holds a `HandlerHandle`, so it is
  `Handles<E>` as well (implied). The five other role handles stay untyped, and
  then the `ReaderHandle` rustdoc sentence "a face that needs only
  `SignalReader` can be built over this handle directly" and ADR-0021 decision
  12's reading of it stop being true: a `Client` cannot be bound over a
  `ReaderHandle`, because it is not `Encoded`. No test or example in the
  workspace binds a face over a bare role handle (the doubles wrap them and are
  `Encoded` themselves). Two resolutions: (a) type all six role handles with `E`
  (one more `PhantomData` field and one `Encoded` impl each; `Handles<E>` holds
  them all), which keeps decision 12's "a face may be built over a role handle"
  true; or (b) keep the design's two and rewrite the `ReaderHandle` sentence to
  "a face is bound over the aggregate or over `HandlerHandle`; the other role
  handles carry no encoding". The plan is written for (b), the design as stated;
  (a) adds the five handles to Task 1's file list with the same shape. **Q-1
  below.**
- **D-5 — `ridl-rt` prose.** The design says "No `ridl-rt` change". The `port`
  module doc and the `Encoded` rustdoc say that no port implements the trait and
  the face does not read it. Resolution: Task 5 rewrites the two sentences; no
  item, signature or trait item changes, so `runtime-crates` under
  `cargo semver-checks` sees nothing.
- **D-6 — spec 2b's generated docs.** Since #860 the face carries generated doc
  comments and `unreachable!` messages, some of which name
  `Payload<FlatBuffers>` or `Sizes<FlatBuffers>` (`face.rs:1171-1174`,
  `serve.rs:64`, `dispatch.rs:119-138`). Resolution: Task 3 rewrites them to
  "the port's encoding"; the form of the generated docs (ADR-0028) is unchanged.
- **D-7 — one more book sentence.** `docs/book/cli-reference.md:834` says the
  face names `::ridl_rt::encoding::FlatBuffers` at each site and a consumer
  names the encoding from `ridl_rt::encoding`; the design's "What changed" list
  does not name it. Resolution: Task 5 rewrites it.
- **D-8 — the transport-default table.** The design's table (7.1) names
  `ws::connect_with::<E>` and a shared-memory default; neither is built (R-1:
  they ship as 0.10.x additions). Resolution: the records of Task 5 carry the
  loopback row as built and the other rows marked as the specification, per
  AGENTS.md's rule for prose about what the CLI does not emit.
- **D-9 — `Encoded for &P` (#855).** Lane W owns #855. If it lands before Task
  3, nothing here changes; if after, nothing here blocks it. No overlap.

## Questions for a human decision

- **Q-1 (D-4).** Type all six role handles with `E` (a), or only `Loopback`,
  `HandlerHandle` and `Handles` as the design states (b)? Recommendation: (a),
  because ADR-0021 decision 12 lets a face be built over a role handle and the
  loopback's own rustdoc says so; the cost is five `PhantomData` fields.
- **Q-2.** If lane W's binding document is not on `main` when Tasks 1 to 6 are
  done, does the 1b pull request wait as a draft (this plan's order), or does it
  merge with Task 7 moved to lane W's own ADR-0021 pull request? The lanes plan
  says only that the rewrite waits for the document.

## Self-review

- Spec coverage: section 7.1 (the trait, the face's signatures, the loopback,
  the defaults) → Tasks 1, 3, 5; 7.2 (bounds as `where` clauses) → Tasks 2, 3;
  6.3's two 1b rows → Tasks 1, 3; 9.3 → Task 2 (and D-2), Task 3 (the fixture);
  10 → Tasks 1, 5, 7; 11 (the generic-face test: reversed bytes, FlatBuffers
  beside it, `compile_fail` on `Sizes<ReprC>`, `just demo`) → Tasks 3, 4; 13 →
  the whole plan, with the `ridl-rt` constraint in Global Constraints; 15 → not
  reopened. The lanes plan's rules: `!` in the title, the Kotlin heads-up (Task
  6), the gated ADR-0021 rewrite (Task 7), gardening before the pull request
  (Task 8).
- Placeholders: none; every test names its assertion and every record task names
  the sentence.
- Type consistency: `Loopback<E = FlatBuffers>`, `HandlerHandle<E>`,
  `Handles<E>`, `with_encoding`, `encoding_bounds`, `payload_types`,
  `payload_buffer(type_name, port)`, `encode_into(.., port)`,
  `decode_args(.., port)` are named once each and reused.
- Review focus items 1 to 5 each name the task that pins them.
- Proportion: the plan carries signatures, test names and assertions, and one
  short program; no bodies.
