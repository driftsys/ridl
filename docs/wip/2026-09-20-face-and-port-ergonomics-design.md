# The generated face and the ports — six decisions before E11.9

Status: working note, 2026-09-20, drafted for decision. Nothing here is
ratified; each decision below is taken when the maintainer says so, and the
records named in §5 change then. Read after
[the `ridl-rt` design record](../design/ridl-rt.md),
[the interaction-face design record](../design/interaction-face.md),
[ADR-0021](../decisions/ADR-0021-ridl-rt-0.1-api-and-release.md) and
[ADR-0023](../decisions/ADR-0023-interaction-face-generation.md).

Why it exists: E11.13 gave the team a generated face to write against, and E11.9
builds the first runtime against that face. An assessment of the face and the
ports on 2026-09-20, made against `main` at 2bcbab8, found four shapes that an
application pays for on every use and one decision nobody has taken. Every one
of them is cheap to change now: the face is provisional by its own record, and a
breaking change to `ridl-rt` is a 0.x minor release (ADR-0021 decision 10).
Every one of them is expensive to change after E11.9, because the runtime, the
transport and the TypeScript face inherit whatever shape E11.9 builds against.

Binds, when ratified: the Rust backend's `generate_face`, `ridl-rt`, the runtime
crate story E11.9 builds, and Epic 10 task 6.

## 1. What the assessment found

The evidence is in the tree; each item names where.

**F-1. A face borrows its port mutably for its whole life.** `Client<'a, P>` and
`Publisher<'a, W>` each hold `&'a mut P`
(`crates/ridl-backend-rust/src/face.rs`; the emitted form is
`crates/ridl-backend-rust/tests/generated/interaction_face.rs`, module `cabin`).
A runtime that implements every port on one value, as the test-only loopback
does, can therefore be held by one face at a time, and not at all while
`dispatch` runs over it. The round-trip tests show the cost: each one builds a
face inside a block, drops it, and builds another for the next step
(`crates/ridl-backend-rust/tests/interaction_face.rs`,
`round_trip_event_raise_and_receive`). A component that reads a signal, sends a
command and later polls the reply rebuilds its `Client` at each step.

**F-2. A port trait is implemented by nothing but a runtime's own type.**
`ridl-rt` provides no `impl<P: SignalReader + ?Sized> SignalReader for &mut P`,
nor the same for any other port trait (`crates/ridl-rt/src/port.rs` holds trait
definitions only). So a face cannot be built over a borrowed port, and a value
that wraps a port cannot forward to it without writing every method by hand.

**F-3. A correlation does not know what it correlates.** `Correlation` is a
`u64` newtype for every kind of call. `Caller::ack` returns `None` for a query's
correlation forever, and `Caller::reply` never answers for a command's; both are
documented as a caller's mistake to avoid rather than prevented
(`crates/ridl-rt/src/port.rs`, `Caller::ack`). The generated `ack` and `*_reply`
methods inherit the hole.

**F-4. A generated domain type derives nothing.** No emitted type carries
`Debug`, `Clone`, `Copy`, `PartialEq` or `Eq`
(`crates/ridl-backend-rust/tests/generated/interaction_face.rs` holds no
`derive`). An application cannot log a value, compare one in a test, or keep one
in a struct it clones. Since driftsys/ridl#420 a constrained scalar reads its
inner value through `get(self)`, which moves a value that cannot be copied, so
`sample.value.get()` consumes the sample's value and a second read does not
compile. This is also why a `Provider` method takes its argument by reference
(ADR-0023 decision 3): `dispatch` has to read the argument again after the
provider returns, and it cannot clone it.

**F-5. Nothing says whether a port may cross a thread.** No type in `ridl-rt`
mentions `Send` or `Sync`, no port trait carries either as a bound, and every
plain type in the crate is auto-trait neutral. A generated `Client<'a, P>` is
`Send` exactly when `P` is. That is right for a library, and it also means the
question is answered by whatever concrete type E11.9 ships. If that type is one
struct implementing every port, sharing it across threads means a mutex around
the whole port, and every `SignalReader::read` then waits behind every
`SignalWriter::commit`, which is the case reading a signal was designed to
avoid.

**F-6. Nothing lets a caller wait.** Every port method returns at once, which is
RA-20 and is right. But no port offers a way to be told that a reply, an
occurrence or a claim has arrived, so an adapter that wanted to present
`average_reply` as a future would have to poll. This note records it and does
not decide it (§4).

Two properties the assessment confirmed and this note keeps: a value is decoded
only from checked bytes through `payload::Ref`, and the decoded value owns its
data, so no lifetime from a port reaches application code; and a `Client`
carries exactly the port bounds its interface needs (RA-19).

## 2. Decisions proposed

Each decision names what changes, which crate changes, and the alternative it
rejects.

### D-1. A face takes its port by value

`Client<P>` and `Publisher<W>` hold `P` and `W`, and `new` takes the port by
value. With D-2, `Client::new(&mut port)` still compiles, with `P` inferred as
`&mut Loopback`, so the by-value form accepts everything the borrowed form did
and also accepts an owned handle, a `Clone` handle, or any wrapper that forwards
the port traits. The lifetime parameter goes.

- Changes: `crates/ridl-backend-rust/src/face.rs`, the checked-in fixture, and
  the interaction-face design record's `Client` and `Publisher` paragraphs. The
  round-trip tests keep their shape.
- Rejected: a stateless face whose every method takes the port as an argument.
  It removes the borrow but gives every generated method an extra parameter, and
  it leaves nowhere for the once-at-construction catalog check ADR-0021 decision
  3 places in the face.
- Rejected: keeping `&'a mut P` and asking runtimes to hand out short-lived
  ports. It moves the cost into every runtime rather than removing it.

### D-2. `ridl-rt` forwards every port trait through a reference

For each port trait `T` in `crates/ridl-rt/src/port.rs`, `ridl-rt` adds
`impl<P: T + ?Sized> T for &mut P`, and for the traits whose methods all take
`&self` (`Attached`, `Clock`, `SignalReader`, `FixedReader`, `ScannableSignals`,
`CoherentSignals`) also `impl<P: T + ?Sized> T for &P`. The impls are additive,
so this is not a breaking change under ADR-0021 decision 10, and it needs no
`alloc`.

- Changes: `crates/ridl-rt/src/port.rs`, one test per trait that builds a face
  over a borrowed port, the `ridl-rt` design record's "The ports" section.
- Deferred: `impl<P: T + ?Sized> T for Box<P>`. It needs `alloc`, which the
  crate does not depend on, so it waits for a cargo feature and a reason.

### D-3. A runtime hands out one handle per port role

A runtime crate (E11.9's loopback first, then the transport) exposes one handle
type per port trait it implements, not one type implementing them all. The
reader handles (`SignalReader`, `FixedReader`, and the two extensions) are
`Send + Sync`, because a read takes `&self` and several threads may read one
store. The writer, caller, event-source and handler handles are `Send` and are
not required to be `Sync`, because their methods take `&mut self` and one thread
drives each. A face is built over one handle. `ridl-rt` adds no `Send` or `Sync`
bound to any trait; the expectation is stated in the design record and pinned in
the runtime crate by a compile-time assertion (`fn assert_sync<T: Sync>()` over
the reader handle).

- Changes: the `ridl-rt` design record's "The ports" section and the crate-level
  rustdoc, one paragraph each; the E11.9 story's `Done when` gains "the loopback
  exposes one handle per port role and its reader handle is `Sync`"; the roadmap
  row changes with it.
- Rejected: one runtime struct implementing every port, shared behind a mutex.
  It serialises reads behind writes (F-5).
- Rejected: `Send + Sync` as supertrait bounds on the port traits. It would
  exclude a single-threaded `no_std` runtime whose handles use `Cell` or
  `RefCell` internally, which is a legitimate target on the platform ladder.

### D-4. The face types its correlations; the port does not

The generated face emits one newtype per call:
`SetLevelCorrelation(Correlation)` for a command,
`AverageCorrelation(Correlation)` for a query, each `Copy`. A send method
returns its own newtype, `average_reply` takes `AverageCorrelation`, and `ack`
becomes one method per command, `set_level_ack(SetLevelCorrelation)`, so a
query's correlation cannot reach `ack` and a command's cannot reach a `*_reply`.
The port keeps the untyped `Correlation`, because a port carries interface
numbers, ordinals and bytes and never a payload type (the crate-level rustdoc
states this as a property of every port), and a typed correlation is a
payload-shaped fact.

This corrects the 2026-09-20 assessment's first reading, which placed the typed
correlation in `ridl-rt`. Placing it in the face keeps the port's contract
unchanged and makes the fix codegen-only.

- Changes: `crates/ridl-backend-rust/src/face.rs`, the fixture, the
  interaction-face design record; ADR-0023 decision 4 is amended to name the
  newtype as the return type, keeping `SendError` as the error type.
- Rejected: `Correlation<K>` in `ridl-rt` with a marker `K`. It types the port,
  which contradicts the port's own rule above, and it makes every runtime carry
  a type parameter it never reads.

### D-5. The derive set is design decision 7 of the value-objects design, and lands in task 6

`docs/wip/typl-value-objects-design.md` decision 7 already fixes the derives:
`Debug`, `Clone` and `PartialEq` on every generated type; `Copy` when the
transitive closure is `f64`, `i64` or `bool` only; `Eq` and `Hash` when no `f64`
appears in the closure; `PartialOrd` and `Ord` on numeric named scalars only.
Decision 8 keeps `Default` built from the typl init value, and decision 9 emits
nothing for `Send` and `Sync`. Epic 10 task 6 implements decision 7. This note
adds no derive and changes no rule there; it records two things the face depends
on:

- `Copy` on every named scalar is what makes `get(self)` a copy rather than a
  move (F-4), so the face's usability rests on task 6 landing, not on a change
  to the face.
- The `Provider`-by-reference rule (ADR-0023 decision 3) stays as it is after
  task 6. By reference costs nothing, and the rule no longer needs "the payload
  types implement neither `Copy` nor `Clone`" as its reason; the ADR-0023
  amendment of §5 restates the reason as "one rule describes the whole trait".

- Changes: none to the design or the plan. The ADR-0023 amendment names decision
  7 as the reason the by-reference rule survives.
- Rejected: a separate derives pull request ahead of task 6. It would change the
  same emitter and every snapshot twice.

### D-6. The order these land in

1. This note, ratified: the ADR amendments and the `ridl-rt` design record
   paragraph of §5, in one docs-only pull request.
2. D-1 and D-4 together, as one face pull request, before Epic 10 task 4
   branches: tasks 4 and 5 regenerate the same fixture, and landing the face
   change first keeps that file from conflicting on every lane C pull request.
3. D-2, in `ridl-rt`, any time after step 1; it is additive and touches no file
   lane C touches. Whether it ships as 0.1.1 or 0.2.0 is the maintainer's call
   under ADR-0007 decision 14; nothing here requires a breaking release.
4. Epic 10 task 6, which carries the derives D-5 depends on.
5. E11.9 starts against the face of step 2 and the impls of step 3, and builds
   D-3.

## 3. What does not change

- `Provider` methods take their argument by reference (ADR-0023 decision 3).
- A `Client` send returns `SendError` (ADR-0023 decision 4, error half).
- No port method waits, no port names a payload type, no port takes the current
  time (the `ridl-rt` design record, "The ports").
- The port method receivers: `&self` for a read, `&mut self` for a write.
- `Correlation` and `ClaimId` in `ridl-rt` stay untyped `u64` newtypes.

## 4. Open, and deliberately not decided here

1. **A wake hook (F-6).** Whether a port gains a way to register interest in an
   arrival, or whether waiting is entirely the runtime's own loop, is a frame
   and transport question and belongs to E11.1 and E11.9. Recorded on
   driftsys/ridl#350 as a new row; not decided by this note.
2. **Stack buffers sized from `MAX_SIZE`.** Every generated method reserves
   `[0u8; <T as Payload<ReprC>>::MAX_SIZE]` on the stack. It is exact for the
   fixed-width fixture and grows with the first string or sequence payload. The
   codecs (E11.7, E11.8, E11.12) decide what `MAX_SIZE` is for those, so the
   sizing rule waits for them.
3. **`Box<P>` forwarding** (D-2, deferred).

## 5. Records this note changes when ratified

| Record                                        | Change                                                                                                                                                          |
| --------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| ADR-0021                                      | a 2026-09-20 amendment recording D-2 and D-3: the forwarding impls, and that no port trait carries a `Send` or `Sync` bound because the handle model carries it |
| ADR-0023 decision 4                           | amended: a send returns the call's own correlation newtype (D-4); `SendError` unchanged                                                                         |
| ADR-0023                                      | a new decision recording D-1: the face takes its port by value                                                                                                  |
| `docs/design/ridl-rt.md`, "The ports"         | one paragraph: the forwarding impls, and the handle model a runtime is expected to present                                                                      |
| `docs/design/interaction-face.md`             | the `Client`, `Publisher` and correlation paragraphs rewritten to the shape of D-1 and D-4                                                                      |
| `docs/wip/typl-value-objects-plan.md`, task 6 | no change; D-5 confirms design decision 7                                                                                                                       |
| `docs/ROADMAP.md`, E11.9                      | `Done when` gains the handle model of D-3                                                                                                                       |
| driftsys/ridl#350                             | three new rows: the handle model, the forwarding impls, the wake hook                                                                                           |

## 6. Trace

- Assessment: this session, 2026-09-20, over `main` at 2bcbab8; the two crates'
  test suites passed at b6544fe
  (`cargo test -p ridl-rt -p ridl-backend-rust
  --locked`).
- Evidence: `crates/ridl-rt/src/port.rs`,
  `crates/ridl-backend-rust/src/face.rs`,
  `crates/ridl-backend-rust/tests/generated/interaction_face.rs`,
  `crates/ridl-backend-rust/tests/interaction_face.rs`, driftsys/ridl#420
- Binds against: ADR-0021 decisions 3 and 10; ADR-0023 decisions 3 and 4;
  ADR-0020 decision 6 (a runtime implements the ports)
- Related: driftsys/ridl#350 (the open API questions), E11.9
  (driftsys/ridl#265), E11.1 (driftsys/ridl#257), Epic 10 task 6
  (driftsys/ridl#251)
