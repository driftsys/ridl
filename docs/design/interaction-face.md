# The generated interaction face

The Rust backend's generated face over `ridl-rt`: per interface, an async
`Client`, a `Publisher`, a `Provider` trait, `serve`, and a `blocking` module
holding the same client, and `serve` where there is one, as blocking calls. The
first form of it was an in-process-only MVP, ahead of the frame specification
and the transport, so the team had a face to write against; ADR-0018 decision 15
restores the face as the runtime layer's "phase 2". `ridl build --emit rust`
then emitted it, and the call surface was reshaped: the poll face the MVP made
public — a send that returns a correlation, a poll per outcome, a one-pass
`dispatch` — became internal, and the two clients and `serve` took its place.
This is the architecture as built. The generation decisions that bind future
work on it are [ADR-0023](../decisions/ADR-0023-interaction-face-generation.md),
whose decision 6 records the call surface, and the reasoning behind that surface
is the archived design note
[`2026-09-25-async-face-design.md`](../archive/2026-09-25-async-face-design.md),
cited below by decision number (F-1 to F-15). Read this record together with
[the `ridl-rt` design record](ridl-rt.md), which the face binds against, and the
[ridl language reference](../specification/ridl-language-reference.md) §6 and §7
(command and query), which the settlement table implements.

## Scope and the two entry points

`crates/ridl-backend-rust` holds the face in three modules and one public
function beside the domain-type emission:

- `src/descriptors.rs` — one `ridl_rt::contract::Interface` implementation per
  named interface, one `Interaction` implementation per member, and the
  generated buffer-size constants.
- `src/face.rs`, with its submodules under `src/face/` — `Client`, `Publisher`,
  `Provider` and the `Event` enum in `face.rs`; the named futures in
  `futures.rs`; the internal poll face in `poll.rs`; the internal one-pass step
  in `dispatch.rs`; `serve` and its future in `serve.rs`; and the `blocking`
  module in `blocking.rs`.
- `src/clauses.rs` — the contract-clause translator. Only `src/descriptors.rs`
  calls it, when it emits a `Command`'s or a `Query`'s `require` and `ensure`
  bodies. `src/face/dispatch.rs` names those generated methods from the step it
  writes, but does not translate a clause itself.

`generate(package)` keeps its original output: the domain types and the codec,
naming no port. `generate_face(package)` is a companion entry point that emits
what `generate` emits, plus the descriptor and face items. It is the only caller
of the clause translator, and the only entry point whose output names
`::ridl_rt::port`. `ridl build --emit rust` calls `generate_pipeline`, which
walks interfaces one at a time and emits the face of each it can carry; the "The
face is emitted by `ridl build`" section below is the record. Why a companion
entry point rather than folding the face into `generate` is
[ADR-0023](../decisions/ADR-0023-interaction-face-generation.md) decision 2.

## The descriptors

For each named interface reached through `Package::shapes()` (a service's inline
shape is skipped — its identity name is a dotted service name, not a single Rust
identifier, and descriptors for one are a follow-up), the emitter writes:

- a unit struct with an `Interface` impl carrying `CATALOG`, `NUMBER`,
  `PROVISIONAL`, `NAME` and `MEMBERS`, plus two associated constants,
  `MAX_BUFFER_SIZE` and `EVENT_SOURCE_BUFFER_SIZE`;
- a unit struct per interaction with an `Interaction` impl (`type Iface`,
  `const MEMBER`) and the kind's trait — `Signal`, `Event`, `Command`, `Query`
  or `Fixed`.

**The interface number is the IR's own, never invented.** The example package
carries no `interfaces.lock`, so `Interface::NUMBER` and
`Interface::PROVISIONAL` come straight from the IR's provisional assignment
(`Interface.number`, `Interface.provisional`, added by the lock design's L4).

**The two buffer constants are computed, not counted.** `MAX_BUFFER_SIZE` is the
maximum over every argument and reply
`<T as Payload<::ridl_rt::encoding::FlatBuffers>>::MAX_SIZE` the interface's
calls use — arguments alone would under-size the claim buffer whenever a query's
reply is larger than its argument, because a reply is encoded into the same
buffer. `EVENT_SOURCE_BUFFER_SIZE` is the maximum over event payload sizes only,
because `EventSource::next`'s payload type is not known until the occurrence's
ordinal is read. Both are emitted as a const-evaluable block (a `while` loop
over an array), not as a folded literal, so the maximum is computed by the same
code a consumer compiles, not precomputed and trusted by the emitter.

**The catalog hash, and one placeholder until its replacing story lands:**

- `CATALOG.hash` is the computed catalog hash, copied from the codegen model's
  `Catalog.hash` (`crates/ridl-backend-rust/src/descriptors.rs`): SHA-256 over
  the protobuf binary of a reduced package — the package's interfaces, their
  numbers and the types they reach
  ([ADR-0014](../decisions/ADR-0014-ir-encodings.md) decision 15). The catalog
  hash (driftsys/ridl#378) replaced the `CatalogHash([0u8; 32])` placeholder the
  emitter wrote before it. Since driftsys/ridl#381 the generated `Bind::new` and
  `serve` compare the port's catalog with `CATALOG`, name and hash (see "The
  catalog check" below).
- `PayloadInfo.max_size` (`EncodedSizes { proto3, flatbuffers, repr_c }`) is
  filled one column at a time. Since driftsys/ridl#380 the `flatbuffers` column
  is the codegen model's `Payload.flatbuffers_max_size`, the bound
  `ridl_ir::projection::flatbuffers::max_size` computed — the backend, like any
  plugin, reads the model and recomputes nothing — and `None` only when the
  projection has no bound. The `proto3` column is `None` because this backend
  emits no proto3 codec, and the `repr_c` column is `None` until the
  C-representable layout is defined (driftsys/ridl#317). `ridl-rt`'s reading of
  `None` is the broad one — "no size is available here", never "this payload
  cannot be encoded this way" (`crates/ridl-rt/src/contract.rs`) — and the doc
  comment the emitter writes into every generated file
  (`crates/ridl-backend-rust/src/descriptors.rs`) states the same three facts.
  Nothing in the face reads these fields — it sizes buffers from
  `<T as Payload<::ridl_rt::encoding::FlatBuffers>>::MAX_SIZE` directly — so the
  columns serve a catalog consumer, not the face.

## The contract-clause translator

The IR carries a `require`/`ensure` clause only as canonical ridl source text
(`Contract.source`, e.g. `"window > 0"`), not as an expression tree;
driftsys/ridl#704 tracks adding the tree. `src/clauses.rs` is a narrow, total
translator built to make the two clause-driven settlement rows real without
pre-empting the tree:

- **Accepted form:** `<subject> <comparison> <numeric literal>`, where
  `<subject>` is the interaction's single declared parameter (or `result` on a
  query's `ensure`) and `<comparison>` is one of `< <= > >= == !=`. The
  subject's named type must be an integer- or float-backed scalar.
- It emits `args.0 <op> <literal>` for a parameter and `reply.0 <op> <literal>`
  for `result`; several clauses of one kind conjoin with `&&`.
- **Every other clause form is refused with a `GenerateError`**, never dropped
  and never silently emitted as `Ok(())`. A dropped clause would generate a
  provider that accepts arguments its own contract forbids.
- A command or query declaring no clause of a kind emits `Ok(())`.

Why the translator refuses rather than widens, and why it lives behind
`generate_face` rather than `generate`, is
[ADR-0023](../decisions/ADR-0023-interaction-face-generation.md) decision 1.

## The consumer face

`src/face.rs` emits, per named interface with at least one member the face
covers, one `pub mod` named after the interface. Its consumer side is:

- **`Client<P: ...>`** — one read method per signal, returning
  `Result<Sample<T>, ReadError>` at once; a `subscribe_<event>` per event and
  one `next_event` per interface, returning the `NextEvent` future; and one
  method per command and per query, returning that call's own named future. The
  reads and the calls are inherent methods; `new` is `ridl_rt::face::Bind`'s,
  `next_event` is `ridl_rt::face::Events`'s, and `subscribe_<event>` is a method
  of the module's generated `Subscribe` trait (the paragraph "The fixed methods
  are trait methods" below). The trait bounds on `P` are computed from the
  interface's interaction kinds and no others: `SignalReader` when it declares a
  signal, `EventSource` when it declares an event, `Caller` and `Clock` when it
  declares a command or a query (`Clock` for the call's deadline), and
  `Wakeable` when it declares an event, a command or a query (for the wait).
  This is RA-19: a `Client` never carries a bound its own interface does not
  need. It is proven, not merely asserted, by
  `ra19_a_minimal_signal_only_port_constructs_the_signal_only_client` in
  `tests/interaction_face.rs`, which constructs the fixture's signal-only `Horn`
  interface's `Client` with a port implementing only `SignalReader` and
  `Attached` — a bound the emitter should not have added would fail this to
  compile — and the three other bound sets are pinned by
  `tests/face_generation.rs`. (The converse — that a bound the interface does
  need is never missing — is not separately proven: under-bounding cannot
  compile at all.)
- **`Event`** — one enum per interface that declares an event, with one variant
  per event carrying `Occurrence<T>`, which `next_event` routes into by ordinal.
- **One future type per command and per query, `<Name>Call<'a, P>`**, and
  **`NextEvent<'a, P>`** when the interface declares an event.
- **`blocking::Client<P>`**, under the emitted crate's `std` feature — the
  section "The blocking module" below.
- **`prelude`** — the module a consumer glob-imports,
  `use <crate>::<iface>::prelude::*;`, for each interface whose face it uses
  (rustc reports a prelude as an unused import when the other imported preludes
  already bring every item it would add): it re-exports by name the
  `ridl_rt::face` traits the module's types implement (`Bind`; `Events` with an
  event; `Publish` with a signal; `Timeout` under `cfg(feature = "std")` where
  the `blocking` module is emitted) and the module's own `Subscribe` and
  `Invalidate` as `_`, so two interfaces' preludes share one scope without
  conflict.

**A face holds its port by value and has no lifetime parameter.** `Client<P>`
and `Publisher<W>` hold `P` and `W`, and `new` takes the port by value. A
borrowed port still works — `Client::new(&mut port)` infers `P` as `&mut Port`
(with the prelude in scope, since `new` is `Bind`'s), under the forwarding impls
of [ADR-0021](../decisions/ADR-0021-ridl-rt-0.1-api-and-release.md) decision 11
— and so does an owned handle, a `Clone` handle, or any wrapper that forwards
the port traits. A face built over a borrow holds that borrow for as long as the
face lives, and a call's future holds the client's port for as long as the
future lives; so the provider side runs over a handler handle taken from the
runtime before the client is built, which is what the round-trip tests do. A
program that holds its faces for its whole run gives each face an aggregate of
its own from `Loopback::attach` instead, and takes the handler from
`Loopback::handler`, which is what `examples/cabin/consumer` does
(driftsys/ridl#488). The constructor is where the catalog check of
[ADR-0021](../decisions/ADR-0021-ridl-rt-0.1-api-and-release.md) decision 3 is
made: `Bind::new` compares the port's catalog with the interface's `CATALOG`
before it stores the port, and panics on a mismatch (see "The catalog check"
below). This supersedes the M1 design's `Client<'a, P>` and `Publisher<'a, W>`;
[ADR-0023](../decisions/ADR-0023-interaction-face-generation.md) decision 5
records it.

**A command and a query return a named future.**
`set_level(&mut self, level: Level) -> SetLevelCall<'_, P>` and
`average(&mut self, window: Window) -> AverageCall<'_, P>`, with the future
bound on `P: Caller + Clock + Wakeable`, `Output = Result<(), ClientError>` for
the command and `Result<Average, ClientError>` for the query. `ClientError` is
`ridl-rt`'s (F-1): `Send(SendError)` when nothing was sent, `Call(CallError)`
for the outcome the provider or the transport settled, `Read(ReadError)` when
the reply could not be read. The call is sent when the method runs, not when the
future is first polled (F-4). The future holds `&'a mut P`, its phase —
`Unsent(arg)` while the port answers `SendError::Busy`, `Waiting(c)`,
`Failed(SendError)` for a `require` failure or a send failure other than `Busy`,
which the first poll reports, and `Done` — and the deadline,
`Option<Timestamp>`, computed as the port's `now` plus `Member::call_deadline()`
when the method runs (F-2); a member with no `max` has no deadline. Every poll
registers its phase's interest — `Interest::Slot` in `Unsent`,
`Interest::Outcome(c)` in `Waiting` — then reads the port once, then returns
(F-5). At the deadline, an unsent call resolves to
`Err(ClientError::Send(SendError::Busy))`; a sent call calls `Caller::forget`
and resolves to `Transport::Undelivered` for a command and `Transport::Timeout`
for a query (F-3). A call at exactly `max` is still within its bound: the
deadline has passed when `now > deadline`, as `Freshness::of` counts an age
equal to `max` as fresh. Leaving `Waiting` inside `poll` forgets the call there,
so `Drop` forgets only a future that is still waiting, and a future that took
its outcome forgets nothing on drop. A resolved call future panics when it is
polled again.

A `require` clause is evaluated before anything is sent, so a failure costs no
round trip and is `ClientError::Send(SendError::Contract(PreconditionFailed))`:
the `SendError` the internal send returns, carried up unchanged, with no
conversion between the two sides' vocabularies (F-12; ADR-0023 decision 4 as
amended).

**`next_event(&mut self) -> NextEvent<'_, P>`** (`ridl_rt::face::Events`, with
`Next<'a> = NextEvent<'a, P>`), with `P: EventSource + Wakeable` and
`Output = Result<Event, ReadError>`, registers `Interest::Event`, reads the
queue once, and returns; it has no deadline — an event has no response bound,
and its `max` is the time to live the runtime applies inside `EventSource::next`
— and no `Drop`, and it can be polled again for the next occurrence. The
interface number is checked before the ordinal, for the reason `serve` checks
it: a port is attached to a whole catalog, ordinals restart at 1 in each
interface, and an occurrence of a sibling interface at the same ordinal would
otherwise be decoded as this interface's payload; such an occurrence is
`Contract::UnknownInteraction`.

**The futures are named, `Unpin`, and store nothing they cannot forget.** A
named type is what a `no_std` frame loop needs: a value it can store in its own
state between frames, which `impl Future` cannot be without allocation, and one
the blocking client can ask whether the call was sent (F-10). The call futures
and `NextEvent` are `Unpin` by their fields — `&'a mut P`, the phase and the
deadline — and declare nothing; `Serve` declares it, because it holds the
handler by value. Nothing emitted uses `async` or `impl Trait` in return
position, so the emitted source compiles as edition 2021 at Rust 1.83, the first
cell of the codegen build matrix
([ADR-0021](../decisions/ADR-0021-ridl-rt-0.1-api-and-release.md) decision 10),
which `just compat-check` builds for the emitted cabin crate with the `std` cfg
on.

**The ridl-named argument is rebound to an emitter-owned name.** The call
method, its future's `poll` and the internal send rebind the argument to `__arg`
first, `dispatch` binds a claim's decoded argument as `__arg`, and the internal
send names its port parameter `__port`; the blocking methods' locals are
`__deadline` and `__call`, and the blocking module's deadline helper is the
function `__deadline_after`, because a parameter is bound under its own snake
case in the method body and would shadow a function of that name. A ridl
identifier cannot start with an underscore, so no parameter can be shadowed by,
or shadow, a local or a function the emitter owns. `dispatch`'s own parameters
and locals keep unprefixed names, and are safe because no ridl parameter name is
bound in its body. `tests/face_compile.rs` compiles an interface whose members
are named `port`, `deadline`, `deadlineAfter`, `this` and `cx`, and, for
`dispatch`, commands and queries whose parameter is named `claim`, `h`, `p`,
`buf`, `accepted` or `reply`, the names in `dispatch`'s body that collided
before driftsys/ridl#570.

**The fixed methods are trait methods, and the member methods are inherent
(ADR-0023 decision 7, driftsys/ridl#580).** Until 2026-09-28 a member whose
snake case was a fixed method name of the face was rustc E0592 in the emitted
crate, and `ridl check` accepted the source: `new` and `next_event` on both
clients, `with_timeout` and `set_timeout` on the blocking one, `new` and
`commit` on `Publisher`, and the derived names `subscribe_<event>` and
`invalidate_<signal>` against a member spelled that way. Rust gives one type one
inherent namespace and a trait its own, and a trait method stays reachable
through the trait's path; so the fixed methods are now methods of the four
traits of `ridl_rt::face` — `Bind::new` on all three faces, `Events::next_event`
on both clients, `Timeout::{with_timeout, set_timeout}` on the blocking client,
`Publish::commit` on `Publisher` — and the derived ones are methods of two
traits generated inside the interface module, `Subscribe` (both clients) and
`Invalidate` (`Publisher`), while every member method stays inherent. The
`prelude` module puts them in scope; the emitter's own calls into the async
client are written through the traits' paths
(`<super::Client<P> as ::ridl_rt::face::Bind>::new(port)`,
`::ridl_rt::face::Events::next_event(&mut self.inner)`,
`super::Subscribe::subscribe_<event>(&mut self.inner)`), so a member of that
name cannot capture them. On a real collision nothing is refused and nothing is
renamed: with a member named `new`, `Client::new(port)` is the member, because a
path call finds an inherent item first, and the consumer writes
`<Client<_> as Bind>::new(port)` or `let c: Client<_> = Bind::new(port)`. A dot
call follows Rust's method probe — the receiver by value, then by `&`, then by
`&mut`, an inherent method before a trait method at each step — and a member
takes `&self` (a signal read) or `&mut self`; `Bind::new` takes no receiver and
is reached by a path call, and every other trait method takes `&mut self` except
`Timeout::with_timeout`, which takes `self` by value. So the member keeps the
dot call for every fixed and derived method but `with_timeout`: with a member
named `commit`, `publisher.commit()` is the member and
`Publish::commit(&mut publisher)` the face's; with a member named `withTimeout`,
`client.with_timeout(x)` on a blocking client held by value is
`Timeout::with_timeout`, found at the by-value step, and the member is reached
through the inherent path `blocking::Client::with_timeout(&mut client, x)`
(`&client` for a signal read). `tests/face_compile.rs` compiles one interface
per colliding name, with the `std` cfg on and off, each with a consumer that
dot-calls the member and reaches the face's method through the trait's path
(and, for `withTimeout`, the reverse), one consumer of two interfaces' preludes
that calls both interfaces' `subscribe_<event>` and `invalidate_<signal>` and
writes the qualified forms, and a proof that `Timeout` is under `ridl-rt`'s
`std` feature; `tests/face_generation.rs` pins the trait impls, the preludes,
and that no `Subscribe` is emitted without an event and no `Invalidate` without
a signal. The `ridl-rt` side is ADR-0021 decision 19, and the design note with
the inventory of every generated item and the compile experiments is
[`2026-09-28-face-fixed-methods-traits-design.md`](../archive/2026-09-28-face-fixed-methods-traits-design.md).

**Nothing here waits (RA-20, as F-15 restates it).** Generated code contains no
thread, socket or timer, and no port waits; a face may return a future, and that
future never blocks. Each future's `poll` registers its interest with the port,
then reads the port — a call or event future once, `Serve` until the handler has
no claim waiting, it has taken 32 claims in that poll, or it refused the
settlement of an oversized claim (driftsys/ridl#569) — and returns. What waits
is the executor or the frame loop that polls the future, or
`ridl_rt::task::block_on` under the `blocking` module, which is the library's
and not generated. ADR-0018's two rejections — `async fn` at the platform layer,
blocking calls at the platform layer — are about the engine's layers and bind
neither the ports library nor generated code; its open question 5 is answered
for the face: a command's future resolves on the delivery acknowledgment, the
runtime's finding, and carries no acceptance value.

## The provider face

- **`Publisher<W: ...>`** — over `SignalWriter` when the interface declares a
  signal and `EventSink` when it declares an event, with one `set` and one
  `invalidate_*` per signal, `commit`, and one `raise` per event. Unchanged by
  the async clients. The sets and the raises are inherent; `new` is
  `ridl_rt::face::Bind`'s, `commit` is `ridl_rt::face::Publish`'s, and
  `invalidate_<signal>` is a method of the module's generated `Invalidate`
  trait, emitted with `Publish`'s impl only when the interface declares a signal
  (ADR-0023 decision 7).
- **`trait Provider`** — one method per command and query, generated only when
  the interface declares one. **A method takes its argument by reference**
  (`fn set_level(&mut self, level: &Level)`), because the internal step reads
  the argument again when it evaluates a query's `ensure` clause after the
  provider returns, and the generated payload types implement neither `Copy` nor
  `Clone`. A command method returns nothing — a command has no failure the
  application reports (ridl §6.1) — and a query method returns its declared
  reply type.
- **`serve`** — the next section.

## `serve`, the internal step, and the settlement table

**`serve<H: Handler + Wakeable, P: Provider>(h: H, p: &mut P) -> Serve<'_, H, P>`**
calls `Handler::serve` with the interface's command and query ordinals when the
function runs; a refusal is a future ready with `ProviderError::Serve`. Each
poll registers `Interest::Claim`, then settles claims through the internal
one-pass step, at most 32 in one poll (`SERVE_BUDGET`, a private constant of
each interface module that emits `serve`; ADR-0023's 2026-09-28 amendment). A
poll that took 32 claims wakes its own waker and is `Pending`, so the executor
polls it again after other tasks have run, because a future that holds one poll
for an unbounded time blocks every other task on a single-threaded executor; a
poll that found no claim left before 32 is `Pending` without waking itself. The
32 are counted as claims are taken, not as their settlements are accepted; the
one exception is a refused settlement of an oversized claim, which ends the poll
with the budget unspent and no self-wake (below). The self-wake is discarded
under `ridl_rt::task::noop_waker`, so a frame loop that polls `Serve` once per
frame with it settles at most 32 claims per frame; a loop that polls with
`ridl_rt::task::flag_waker` polls again while the flag was set, up to its own
limit of polls per frame. The handler port's failure resolves the future to
`ProviderError::Claim`, with every claim settled before it staying settled.
`Output` is `Result<Infallible, ProviderError>`: the future never resolves to
`Ok` (F-7). It holds the handler by value, the provider by `&mut`, and the claim
buffer of `MAX_BUFFER_SIZE` bytes inline, so the application supplies no buffer
and reads no count. A resolved `Serve` panics when it is polled again. Over
`ridl-loopback`, registering the served set means a handler under `serve` is
presented only the interface's own members; the settlement table's unknown-route
rows are reachable only through the internal step directly, which is how
`tests/interaction_face.rs` still exercises them — the test file is inside the
crate that `include!`s the fixture, so `pub(crate)` reaches it.

**The internal step, `dispatch`**, is `pub(crate)` and returns
`Result<usize, ReadError>` — the count of claims `Handler::settle` accepted, or
the failure `serve` resolves to — and `Ok(0)` on a buffer shorter than
`MAX_BUFFER_SIZE`, without consuming a claim. It takes a `budget: &mut usize`,
which it decreases by one for each claim it takes and at 0 stops without asking
for another claim; `serve` passes 32 and reads the budget left to know whether
the pass stopped at the bound. It loops over `Handler::next_claim`, routing and
settling every claim it takes — including one this interface does not recognise,
because `Handler`'s own contract requires every claim to be settled:

| Cause                                       | Settled as                                                          |
| ------------------------------------------- | ------------------------------------------------------------------- |
| `claim.ord` or `claim.iface` matches no arm | `Contract::UnknownInteraction`                                      |
| `VerifyError::Structure(_)`                 | `Transport::Corrupt`                                                |
| `VerifyError::Contract(v)`                  | `Contract::InvalidValue(v)`                                         |
| `require` returns `Err(())`                 | `Contract::PreconditionFailed`                                      |
| `ensure` returns `Err(())` (query only)     | `Contract::ContractBroken`                                          |
| `ReadError::ShortClaim` from `next_claim`   | `Transport::Corrupt`, by the claim's id, unread (driftsys/ridl#569) |

The two `VerifyError` rows are kept separate rather than both settled as
`Transport::Corrupt`, because `Payload::verify` reports a structural failure and
a typl-constraint violation as two distinct variants, and collapsing them would
report a range or enum-variant violation in the wrong error stratum (ridl §10.2
vs §10.3).

**An oversized claim is settled, not fatal (2026-09-28, driftsys/ridl#569).**
`Handler::next_claim` reports a claim whose argument bytes do not fit `buf` as
`ReadError::ShortClaim { claim, needed, trace }` and does not consume it
([ADR-0021](../decisions/ADR-0021-ridl-rt-0.1-api-and-release.md) decision 5,
amended the same day). The step settles `claim` `Transport::Corrupt` without
reading it — an argument that does not fit `MAX_BUFFER_SIZE`, the interface's
largest argument or reply payload, is larger than any valid encoding of this
interface's members and so not a well-formed encoding of one, the rule the
`VerifyError::Structure` row already applies; a claim naming another interface
may be validly larger, and is settled `Corrupt` too, because the step cannot
read it — counts it toward the budget like a claim taken, and continues with the
next claim. When the handler refuses that settlement, the step ends the pass at
once and returns the count so far with the budget unspent, as if no claim were
waiting: the runtime keeps the unsettled claim the next one, so taking it again
would spend the whole budget on it on every poll and wake `serve` forever.
`serve` therefore does not wake itself, and waits for the next claim wake; the
claims behind such a claim wait until the handler can settle it. Every other
`ReadError` from `next_claim`, including a `Short` from a runtime older than the
variant, still resolves `serve` to `ProviderError::Claim`. Only a raw `Caller`
or a network runtime can send such a claim; a generated client sizes its
arguments from the same descriptor. Before this change the step resolved to
`ProviderError::Claim(Short)`, and every claim queued behind the oversized one
stayed unserved.

**A command settles before the provider method runs; a query settles after.** A
command's acknowledgment is a delivery acknowledgment, not a completion one
(ridl §6.1), so once its arguments and `require` pass, the step calls
`Handler::settle(claim.id, Ok(&[]))` and only then calls the provider's method.
A query settles after the provider returns and `ensure` is evaluated, because
its settlement carries the reply. **This ordering is pinned only by an
exact-text assertion** in `tests/dispatch_generation.rs`; no behavioural test
exercises it, because neither `Handler::settle` nor a provider call had an
observable side effect a reordering would change when the round trip ran over a
test double. `ridl-loopback` makes it observable: a provider can hold a caller
handle on the same store and read the acknowledgment from inside its own method.
That test is not written, which is a gap in the tests rather than a defect in
the generated code.

**`EncodeError::Capacity` is a provider-side invariant violation, never a
manufactured contract error.** Every buffer the face encodes into is sized from
`<T as Payload<::ridl_rt::encoding::FlatBuffers>>::MAX_SIZE`, the largest
encoded size of any legal value, so a legal value cannot exceed it. If
`Ref::encode` still returns `Capacity`, the provider returned a value outside
its own type's range, or a `Payload` implementation does not honor `MAX_SIZE` —
a defect the generated code has no vocabulary to describe as one of the five
settlement outcomes, because `EncodeError::Capacity` carries no received bytes
and no violated rule. The generated branch is an explicit `unreachable!` naming
the type, the needed size and the available size. No runtime test drives this
branch: doing so needs a `Payload` implementation that lies about `MAX_SIZE`
without corrupting the buffers the round trip's other assertions share, and
nothing in the fixture isolates one type enough to do that safely under the test
binary's parallel execution.

**A `SettleError` is left to the handler** — which already owns that claim's
settlement — and is not turned into a different `CallError`; the step continues
to the next claim. `round_trip_dispatch_counts_only_accepted_settlements` in
`tests/interaction_face.rs` injects one settlement failure followed by one
successful claim and asserts the counts are `0` then `1`.

**The poll face the step and the futures are built over is `pub(crate)`**
(F-12): the `Copy` correlation newtypes, `<Name>Correlation`;
`send_<name>(port: &mut P, arg: &T) -> Result<<Name>Correlation, SendError>`;
`poll_<name>_ack`, `poll_<name>_reply` and `poll_next_event`. They are
module-level functions over a bare port rather than methods of `Client<P>`,
because a future holds `&'a mut P` under the narrower bound F-10 fixes, and an
inherent method of `Client<P>`, whose struct bounds also name `SignalReader` and
`EventSource`, cannot be called on it. The newtypes stay because inside the face
they still keep a query's correlation out of an acknowledgment — `Caller::ack`
returns `None` for a query's correlation always, with no part of the type saying
so — and cost nothing when private. They are the face's, not the port's:
`Correlation` and `ClaimId` in `ridl-rt` stay untyped, because a port carries
identity and bytes and never a payload type, while which interaction a
correlation belongs to is a payload-shaped fact
([ADR-0023](../decisions/ADR-0023-interaction-face-generation.md) decisions 3
and 4). Making the face private closed both items of driftsys/ridl#485: no
public method returns the three-deep reply shape, and a handler failure is the
value `serve` resolves to.

## The blocking module

Under `#[cfg(feature = "std")]`, and only when the interface declares an event,
a command or a query — a signal-only interface's `Client` never waits, so it has
no blocking form — the face module holds `pub mod blocking`:

```rust,ignore
pub struct Client<P: /* the async Client's bounds */> {
    inner: super::Client<P>,
    timeout: Option<std::time::Duration>,
}
impl<P: ...> ridl_rt::face::Bind for Client<P> {
    type Port = P;
    fn new(port: P) -> Self;                                          // timeout None
}
impl<P: ...> ridl_rt::face::Timeout for Client<P> {
    fn with_timeout(self, timeout: Duration) -> Self;
    fn set_timeout(&mut self, timeout: Option<Duration>);
}
impl<P: ...> ridl_rt::face::Events for Client<P> {
    type Next<'a> = Result<Option<Event>, ReadError> where Self: 'a;
    fn next_event(&mut self) -> Result<Option<Event>, ReadError>;     // None at the timeout
}
impl<P: ...> super::Subscribe for Client<P> {
    fn subscribe_warning(&mut self) -> Result<(), SubscribeError>;    // unchanged
}
impl<P: ...> Client<P> {
    pub fn temperature(&self) -> Result<Sample<Temperature>, ReadError>;  // unchanged
    pub fn set_level(&mut self, level: Level) -> Result<(), ClientError>;
    pub fn average(&mut self, window: Window) -> Result<Average, ClientError>;
}
pub fn serve<H: Handler + Wakeable, P: Provider>(h: H, p: &mut P, timeout: Option<Duration>)
    -> Result<(), ProviderError>;   // only when the interface declares a command or a query
```

**Every waiting method is `ridl_rt::task::block_on` over the async method's
future**, polled through `&mut` because every face future is `Unpin`, so the two
clients cannot diverge: what a call does is the future's, and the blocking
module adds the thread's wait and the timeout (F-10, F-11); a signal read and a
`subscribe_*` delegate unchanged. The deadline is the timeout added to
`Instant::now()` with `checked_add`; a timeout so large that the instant cannot
be represented is a wait with no bound, as the call future's own deadline
saturates rather than panics. The timeout is per client, `None` by default, and
a timeout shorter than a member's `max` is accepted and ends the call first.
**What `max` bounds under `block_on`, stated.** The member's `max` is measured
by the future on the port's clock, and the future reads that clock only when it
is polled, which under `block_on` is when the port wakes it. On a runtime that
measures the bound and wakes the `Outcome` waiter when it passes, the woken poll
finds the bound passed and the call ends at `max`; `ridl-loopback` measures no
bound and `advance` wakes nobody, so over it an unserved call returns only at
the client's timeout (F-3 "A limit, stated", F-11's reason). The face is not
what would change that: it is the runtime's clock and wake. When `block_on`
returns `None`, the client asks the future whether the call was sent — a private
`sent()` on each call future, itself under `std` — and answers as the future
would at its own deadline: unsent is `Err(ClientError::Send(SendError::Busy))`,
sent is `Transport::Undelivered` for a command and `Transport::Timeout` for a
query; dropping the future then forgets a sent call. `next_event` returns
`Ok(None)` at the timeout, and `serve` returns `Ok(())` at it, so a loop that
also does other work can call either repeatedly; with no timeout, a call returns
only with its outcome, `next_event` only with an occurrence or a read failure,
and `serve` only with a failure. `blocking::serve` holds `h` by value, as
`serve` does, and drops it when it returns; a loop passes `&mut handler`, under
the forwarding impls of ADR-0021 decision 11, so the handle outlives each pass.

**Why per client and not per call.** A timeout parameter on every call would
make the two clients' signatures differ in more than the future, and an absolute
`Instant` is computed from a duration by every caller anyway; a caller that
wants one call bounded differently sets the timeout before it. `None` by default
because, on a runtime that measures the bound and wakes at it, a member with a
`max` is bounded by the future, and a member with no `max` waits. Over
`ridl-loopback`, whose clock moves only under `advance`, a blocking call whose
provider never serves returns only at a timeout the caller set, so a test over
the loopback sets one (F-11).

**The feature.** The manifest `ridl build` emits declares
`std = ["ridl-rt/std"]`, on by default, and the `blocking` module is under the
emitted crate's own `std`, so a build with default features off has no
`blocking` module, links `ridl-rt` as `no_std`, and builds for
`wasm32-unknown-unknown`. `block_on` is what `ridl-rt`'s `std` feature carries
([the `ridl-rt` design record](ridl-rt.md), the `task` module); it is not usable
on wasm, where `Instant::now()` panics, and a frame loop there polls with
`noop_waker` or `flag_waker` instead: poll, then, under `flag_waker`, poll again
while the flag was set, up to the loop's own limit of polls per frame, so that
`Serve`'s self-wake at 32 claims is not lost until the next frame. A `no_std`
frame loop with an allocator writes the same small waker over
`alloc::task::Wake` on an `Arc`; one without an allocator needs a hand-written
`RawWaker`, which needs `unsafe`. Inside this repository the fixture is
`include!`d by the backend crate's own tests, and `cfg(feature = "std")` is
evaluated against the including crate, so `crates/ridl-backend-rust` declares a
`std` feature of its own, on by default and read by nothing in the library, for
that purpose alone.

## The encoding and the ports

**The face runs over the generated FlatBuffers codec**
([its design record](flatbuffers-codec.md)). It names that encoding by its full
path, `::ridl_rt::encoding::FlatBuffers`, at every buffer it sizes and every
`Ref` it builds, the way the codec's own `Payload` implementations name it and
the way the prelude names are written (driftsys/ridl#420). The face gains no
type parameter: a `Client<E: Encoding, ...>` would put `E` on every descriptor,
every provider trait and every caller, and a `where` bound per payload type on
every impl, for a choice made once per generated package.

The encoding is stated by a backend option. `ridl_backend_rust::WireEncoding`
defaults to `FlatBuffers` and reaches the entry points through
`generate_face_with(package, wire)`, of which `generate_face(package)` is the
defaulted form, and `generate_pipeline`. It has one variant, because this
backend emits a `Payload` implementation for one encoding, and every emitter
writes that encoding's path; `repr(C)` and proto3 join it when the Rust backend
emits theirs, and the entry points check the option against the one variant so
that a second one is a compile error there rather than a face over
implementations that do not exist. A consumer names the encoding from
`ridl_rt::encoding`.

`generate_face` appends the face to the items `generate` emits, the codec
included. There is one codec emitter and one call to it, so the face compiles
over the same implementations a consumer of `generate` gets rather than over a
second set written for it, and the checked-in fixture stays a single `include!`.

**Until 2026-09-29 the face named the encoding through one alias**,
`pub type Wire = ::ridl_rt::encoding::FlatBuffers;`, emitted once per package at
package scope. An unprefixed item at package scope is a name a declaration or an
interface can carry, and `Wire` collided with both (driftsys/ridl#476, #588);
interaction-face design rule 5 refused such a package. The
[generated-name collision design](../technotes/rust-backend-name-collisions.md)
removed the alias instead (its decision 5), because a name the backend chose
never refuses a package: every site that named `Wire` writes the full path,
`refuse_wire_collision` is deleted, and no name is reserved. The codec's free
functions carry a `__ridl_fb_` prefix, which no typl name can spell.

This was once a placeholder instead: `ReprC`, with `tests/interaction_face.rs`
hand-writing `Payload<ReprC>` for the fixture's types, marked throwaway in its
own module documentation. Both are gone.

**The ports are `ridl-loopback`'s**, the in-process reference runtime (ADR-0020
decision 6, [its design record](ridl-loopback.md)). The round-trip tests build
their faces over that crate's aggregate handle, which implements every port
trait by delegating to one handle per port role, or over the role handles
themselves: `tests/support/doubles.rs` builds the consumer ports from the
loopback's reader, source and caller handles — with a log of every `Caller` and
`Wakeable` call — so that a test can `advance` the clock, take a handler for the
provider side, and fill the call table while a future is alive. Earlier the
ports were instead a disposable double at `tests/support/loopback.rs`; it was
deleted and its tests moved into `crates/ridl-loopback/tests/ports.rs`. The
runtime is in-process, with a clock the test advances by hand and no I/O, by
design and not as a placeholder; its limit is that it measures no bound of its
own, so a call over it whose provider never serves is bounded by the future's
deadline or the blocking client's timeout and by nothing else.

## The fixture and the round trip

`crates/ridl-backend-rust/tests/fixtures/interaction_face.ridl` is a
single-file, single-catalog package (`face.demo`) cut to what the `ReprC`
stand-in could carry when it was written — fixed-width named scalars, one enum,
one all-fixed-size struct, no optional field, sequence, map, union, string or
bytes. The stand-in is gone and the generated codec carries every typl shape, so
the cut is now a limit on what this fixture exercises rather than on what the
face can carry; the codec's own suites (`tests/flatbuffers_roundtrip.rs`,
`tests/flatbuffers_conformance.rs`) run the wider corpus. It declares four
interfaces:

- **`Cabin`** — one signal, one event, one command (`setLevel`, with a `require`
  clause and `@[..50ms]`) and one query (`average`, with a `require` and an
  `ensure` clause and `@[..200ms]`). This is the interface the round trips run
  against, through both clients. Every call declares exactly one parameter of a
  declared named type, and every query replies with one declared named type —
  the face emits no induced argument struct, so a call needing more than one
  parameter is refused (`descriptors::single_param_type`), and a multi-parameter
  call is a recorded follow-up.
- **`Horn`** — one signal and nothing else, existing only to make RA-19's claim
  testable in the direction described above: a minimal port cannot construct a
  `Client` unless the emitted bounds are exactly what the interface needs. It
  has no `blocking` module.
- **`Siren`** — one event and nothing else, so the event-only bound set
  (`EventSource + Wakeable`, no `Clock`) is pinned.
- **`Valve`** — one command and one query that declare no response bound, so
  they carry the built-in default (1 s and 3 s) and the blocking client's
  timeout on a defaulted member (F-11) is testable. The no-`max` path (F-2) is
  covered at the `Member` level by `crates/ridl-rt/tests/call_deadline.rs`.

The fixture declares no `fixed` interaction; the descriptor emitter's `Fixed`
path is covered instead by a hand-built-IR unit test in `src/descriptors.rs`.

**The generated face is checked in, not regenerated at test time.**
`tests/generated/interaction_face.rs` is `generate_face`'s output for the
fixture, brought into `tests/interaction_face.rs` with `include!` under a
handwritten module carrying only outer `#[allow(...)]` lint attributes — the
emitter itself must never emit an inner attribute, because one inside an
`include!`d file is a hard compile error. A test in its own target,
`tests/interaction_face_regeneration.rs`, regenerates the fixture and compares
the result byte-for-byte against the checked-in file, failing with the
instruction to regenerate
(`RIDL_UPDATE_GENERATED=1 cargo test -p
ridl-backend-rust --test interaction_face_regeneration`)
when it drifts. This is what makes the generated face genuinely compiled and
linked by `cargo test`, rather than only snapshotted — the defect ADR-0018's
Alternatives-considered table records against the retracted interaction layer
("it cannot be connected to a runtime at all, and has never been compiled"). The
regeneration test is kept apart from the tests that consume the fixture, so that
it still compiles, and can rewrite the fixture, when a change to the face's
shape leaves those tests unable to compile against the old fixture.

**What proves it** (F-14), in `tests/interaction_face.rs`: the round trips of a
signal, an event, a command and a query through the async client, polled with
`ridl_rt::task::noop_waker`, and of the command and the query through the
blocking client, with `blocking::serve` on a second thread; the dropped future
forgets once and a poll registers before it reads; a future that took its
outcome forgot in that poll and forgets nothing on drop; a future dropped while
waiting for a slot forgets nothing; a call with every slot taken is `Pending`,
is woken by a reclaim, sends on the next poll, and resolves; an unsent call at
its deadline is `Send(Busy)` and is not sent when a slot frees later; a sent
command and a sent query at their deadline are `Undelivered` and `Timeout` and
forget once; a settled call delivers its outcome after its deadline passed; a
send failure other than `Busy` on the retry resolves either kind of call;
`next_event` waits for a raise and resolves with a read failure; `serve`
registers the served set, resolves to `ProviderError::Claim` after settling the
claims before the failure, is ready with a refusal, is woken by a send, and
panics when polled again after either; the blocking client over `Valve` answers
`Send(Busy)`, `Undelivered` or `Timeout` at its own timeout, by phase and by
kind, forgetting a sent call once; a client timeout shorter than `max` is
accepted; `blocking::next_event` returns `Ok(None)` at the timeout and the
occurrence when one is raised from another thread; and `blocking::serve` returns
`Ok(())` at its timeout and a refusal at once. Four test files exercise the
layers before the round trip: `tests/descriptor_generation.rs`,
`tests/face_generation.rs`, `tests/dispatch_generation.rs` (source-text
assertions on the generated code) and `tests/face_compile.rs` (the emitted face
compiled with a bare `rustc`, the `blocking` module included, over shapes the
fixture does not hold); the byte-equality guard is
`tests/interaction_face_regeneration.rs`. `examples/cabin/consumer` runs the
four round trips through the async client and the command and the query through
the blocking client, under `just demo` and
`crates/ridlc/tests/cabin_example.rs`. The test gaps the reviews of the async
and blocking clients left are on driftsys/ridl#571.

## The catalog check

Each interface module carries a private `check_catalog(found: &CatalogRef)`,
which compares `found` with the interface's `CATALOG`
(`<super::Iface as ridl_rt::contract::Interface>::CATALOG`) as a whole
`CatalogRef`, name and hash, and calls `::core::panic!` when they differ, so it
compiles under `no_std`. The panic message names the interface, the catalog the
face was generated from and the catalog the port is attached to, each by
`CatalogRef`'s `Debug`.

Three places call it, each once, through `ridl_rt::port::Attached::catalog`'s
path so that a member named `catalog` cannot capture the call:

- `Bind::new` of `Client` and of `Publisher`, before the port is stored. Every
  port bound of either face — `SignalReader`, `EventSource`, `Caller`,
  `SignalWriter`, `EventSink` — has `Attached` as its supertrait, and every
  emitted `Client` or `Publisher` has at least one of them, so the bound the
  face already carries is enough.
- `serve`, on the handler port, before `Handler::serve` is called, so a handler
  of another catalog never registers the members.
- None in the `blocking` module: its `Client::new` builds the async client
  through `Bind::new`, and its `serve` calls the async `serve`, so each makes
  the comparison once, through that call.

`check_catalog`, both `Bind::new` methods and `serve` are `#[track_caller]`, so
a mismatch panic reports the program's binding call, not a line of the generated
face. `tests/interaction_face.rs` pins this in `mismatch_message`, which every
one of the `*_panics` tests listed below runs: it asserts that the panic's file
is the test file.

No member method compares anything: the comparison is made once per binding. The
rustdoc of each `new` and each `serve` states the panic under `# Panics` and
names the comparison a program makes first to avoid it,
`port.catalog() == <Iface as Interface>::CATALOG`.
[ADR-0023](../decisions/ADR-0023-interaction-face-generation.md) decision 8
records why a mismatch panics rather than returning an error.

`tests/interaction_face.rs` binds a `Client`, a blocking `Client`, a
`Publisher`, `serve` and `blocking::serve` over a runtime attached to a catalog
whose hash differs from the face's, and a `Client` over one whose package name
differs, and each panics with that message; every other round trip runs over a
runtime attached to the face's own `CATALOG`. `tests/face_generation.rs` pins
the comparison in the emitted `Bind::new` and `serve`, and its order before
`Handler::serve`. `examples/cabin/consumer` attaches its runtime to the
generated `CATALOG`.

**History.** Until driftsys/ridl#381 no generated constructor compared anything,
which driftsys/ridl#448 recorded on 2026-09-21. The check waited first for a
computed catalog hash, driftsys/ridl#378, because a comparison of two
`CatalogHash([0u8; 32])` placeholders passes for every port, and then for the
decision on what a mismatch does, which ADR-0023 decision 8 took on 2026-10-04.

## The face is emitted by `ridl build` (2026-09-21)

driftsys/ridl#444 closed the gap the first form left: the face was generated by
a companion entry point no CLI called, so there was nothing to link against.

**The six design rules, numbered.** Every `interaction-face design rule N`
citation in the tree resolves against this list, which is why it is numbered
rather than written as prose.

1. **`write_emits` calls `generate_pipeline`, not `generate`.** The pipeline
   entry point emits the face and the descriptors beside the payload types, so
   `ridl build --emit rust` produces a package a consumer can link against
   without a second tool. `generate` is unchanged in what it emits.

2. **An interface the face cannot carry is skipped, and the rest of the package
   is emitted.** `generate_pipeline` walks interfaces one at a time rather than
   refusing the package, and leaves a note naming the interface, the reason and
   the story that removes the limit. The skip takes that interface's
   **descriptors** with it: `single_param_type`, `query_reply_type` and the
   contract-clause translator all live in the descriptor emitter and serve both
   halves, so an interface whose face cannot be built cannot have its
   descriptors built either. The decision as first written assumed the
   descriptors survive a skip; they cannot, and the note says what a consumer
   actually gets. No other interface, and no payload type, is affected.

   The note's owner line is chosen by the refusal that was raised, not by
   re-reading the interface. An interface can have a call-shape gap on one
   interaction and an untranslatable clause on another — the corpus's
   `veh.cluster.VehicleStatus` does — and reading the interface named the wrong
   story for it. A refusal neither owner claims, such as a `fixed` whose payload
   is not a named type, names no story rather than the nearest one.

3. **No flag selects the encoding, yet.** `--emit rust` writes the FlatBuffers
   codec because it is the only one built, and the emitted code names it by its
   path, `::ridl_rt::encoding::FlatBuffers`, at each site (until 2026-09-29
   through a `pub type Wire` alias; see "The encoding and the ports"). A
   `--wire` flag belongs with the proto3 codec (driftsys/ridl#264), when a
   second codec exists to choose between; ADR-0010 binds its spelling then.
   Adding one now would be a CLI surface with one legal value, which the next
   story would have to change rather than fill in.

4. **A cross-package reference resolves, and the codec is no longer withheld for
   it** (driftsys/ridl#467). `Ctx` carries the other packages of the build and
   the codec resolves a reference through them. The half the gap did not state:
   a foreign owner is written as a path through the module tree `ridlc` writes
   rather than as a bare identifier, and the items such a path names — `check`,
   every `__ridl_fb_*` function, and the generated view struct's fields — are
   `pub(crate)` rather than private, because a generated caller may now sit in
   another module of the emitted crate. The emitted crate is one crate per
   build, so `pub(crate)` reaches every generated caller and adds nothing to the
   crate's public surface. This deliberately does **not** settle open item 2 of
   the 2026-09-20 codec design note — whether `check` is `pub`, for a consumer
   validating a value it did not build, is a surface commitment the public
   surface would take, and `pub(crate)` does not take it.

   Resolution depends on what the caller hands the codec. `generate` is
   `generate_with(package, &[])` and resolves none, so a type reaching another
   package is still withheld a codec there and the withheld note says so.
   `ridl build --emit rust` hands the build, so nothing is withheld for that
   reason. `docs/design/flatbuffers-codec.md` carries the detail.

5. **A package whose declaration or interface is named `Wire` is refused**
   (driftsys/ridl#476). Both land at package scope — a declaration as its own
   item, an interface as the descriptor emitter's identity struct — so either
   collides with the emitted `pub type Wire = …`. `generate_face_with` and
   `generate_pipeline` both refuse rather than emitting the collision. The walk
   is over `shapes()`, the complete set of interface bodies, and skips a
   service's inline shape, for which no identity struct is emitted.

   _Superseded on 2026-09-29 by the
   [generated-name collision design](../technotes/rust-backend-name-collisions.md),
   decision 5 (driftsys/ridl#588): the alias is removed, every site that named
   it writes `::ridl_rt::encoding::FlatBuffers`, `refuse_wire_collision` is
   deleted, and a package whose declaration or interface is named `Wire` builds
   and compiles. The rule is that a name the backend chose never refuses a
   package._

6. **The proof runs what the CLI wrote.** `crates/ridlc/tests/cabin_example.rs`
   builds `examples/cabin/`, compiles the emitted crate and
   `examples/cabin/consumer/src/main.rs` against it with plain `rustc`, runs the
   program, and requires one round trip of each of a signal, an event, a command
   and a query through the generated face over `ridl-loopback` — since the two
   clients, the command and the query through both clients. What is new is the
   path rather than the running: other proofs run generated code, but each runs
   the backend's own output inside this workspace. This one runs what the CLI
   wrote, linked as a separate crate into a separate process. It does not
   establish identity — the round trip is symmetric, so a wrong `InterfaceNo`,
   ordinal or catalog hash would be written and read back consistently;
   `descriptor_generation.rs` pins those.

## The two clients and `serve` (2026-09-27 and 2026-09-28)

The two clients and `serve` landed in two changes to `src/face.rs`, with the
`ridl-rt` 0.3.0 release between them, because the second needed `block_on` from
a published crate. The first (2026-09-27) emitted the async `Client`, the named
futures and `serve`, made the poll face `pub(crate)` under names of its own,
restated RA-20 in the module documentation, added `Siren` and `Valve` to the
fixture, and rewrote `examples/cabin/consumer` onto the async client; it was the
one breaking step for a consumer of generated code. The second (2026-09-28)
emitted the `blocking` module, made the emitted manifest's `std` feature forward
to `ridl-rt/std`, added the blocking round trips to the consumer, rewrote this
record from the design note, and archived the note, the plan and the lane
driver. The sections above describe the face as both left it, with the later
fixes that cite their own issues; nothing in them describes a face the fixture
does not hold.

## driftsys/ridl#580: the fixed methods behind traits (2026-09-28)

One change to `src/face.rs` and `src/face/blocking.rs`, with `ridl-rt`'s `face`
module in the same pull request, before the 0.4.0 release that carries both: the
fixed methods of the three faces became methods of
`ridl_rt::face::{Bind, Events, Timeout, Publish}`, the derived
`subscribe_<event>` and `invalidate_<signal>` became methods of a `Subscribe`
and an `Invalidate` trait generated inside each interface module, each interface
module gained a `prelude`, and the blocking client's calls into the async one
were rewritten through the traits' paths. `tests/face_compile.rs` gained one
case per colliding name and one consumer of two preludes;
`tests/face_generation.rs` moved its exact-text assertions onto the trait impls;
`tests/interaction_face.rs` and `examples/cabin/consumer` gained their `use`
line. It is the one breaking step for a consumer of generated code since the two
clients: a consumer with no `use` of a prelude gets E0599 on every fixed method.
ADR-0023 decision 7 and ADR-0021 decision 19 record it; the design note is
archived as
[`2026-09-28-face-fixed-methods-traits-design.md`](../archive/2026-09-28-face-fixed-methods-traits-design.md).

## What is provisional

| Placeholder                                                                                                                    | Replaced by                                  |
| ------------------------------------------------------------------------------------------------------------------------------ | -------------------------------------------- |
| The `None` `EncodedSizes.repr_c` column (`flatbuffers` is filled; `proto3` is `None` because this backend has no proto3 codec) | driftsys/ridl#317                            |
| The narrow contract-clause translator (`src/clauses.rs`)                                                                       | driftsys/ridl#704                            |
| One declared parameter per call, no induced argument struct                                                                    | a recorded follow-up story                   |
| The command-settled-before / query-settled-after ordering, pinned only by exact-text assertion                                 | a test over `ridl-loopback`, not yet written |

**The hand-written payload row was retired on 2026-09-21**, by design note D-11
of [the FlatBuffers codec record](flatbuffers-codec.md). It read "the
hand-written `Payload<ReprC>` implementations", and it was blocked twice over:
the codec wrote a `Payload<FlatBuffers>` only for a declaration the projection
minted a root table for, which was a `struct` or a `union`, while this face's
payloads are four named scalars and one enum beside one struct. ADR-0019
decision 8 gave every declaration a root, and D-11 made the face name `Wire`.
The hand-written module is deleted, and the round trips run over the generated
codec and `ridl-loopback`.

**The `dispatch` binding row was retired on 2026-09-28**, by driftsys/ridl#570.
It read "`dispatch` binding the ridl parameter name beside its own locals";
`dispatch` now binds a claim's decoded argument as `__arg`.

**A member named like a fixed method of the face was never a row here**, and
compiled to rustc E0592 until 2026-09-28, when driftsys/ridl#580 moved the fixed
and derived methods behind traits (ADR-0023 decision 7, the section "The
consumer face" above). Two collisions of the same class outside the face were
closed on 2026-09-29 by the
[generated-name collision design](../technotes/rust-backend-name-collisions.md):
`<Struct>FbView::bytes` against a field whose accessor is `bytes`
(driftsys/ridl#587) — `bytes` is a method of `ridl_rt::payload::View`, ADR-0021
decision 20 — and the package module's `Wire` against a type or an interface
named `Wire` (driftsys/ridl#588) — the alias is removed, "The encoding and the
ports" above.

## Trace

- Roadmap: `docs/ROADMAP.md`
- Tracking issues: driftsys/ridl#393, driftsys/ridl#444, driftsys/ridl#515,
  driftsys/ridl#485 (the call-shape findings the two clients close)
- Binds: [ADR-0018](../decisions/ADR-0018-runtime-core-and-generated-surface.md)
  decision 15;
  [ADR-0020](../decisions/ADR-0020-third-encoding-runtime-layering-and-plugin-system.md)
  decisions 1, 5, 6 and 7;
  [ADR-0021](../decisions/ADR-0021-ridl-rt-0.1-api-and-release.md), decisions 13
  to 18 for the items the futures poll;
  [ADR-0023](../decisions/ADR-0023-interaction-face-generation.md) — the
  generation decisions specific to this face, decision 6 for the call surface
  and decision 8 for the catalog check
- Depends on: `crates/ridl-rt` 0.4.0 (`Wakeable`, `Interest`, `ClientError`,
  `ProviderError`, the `std` feature's `block_on`, and since 0.4.0 the `face`
  traits, ADR-0021 decision 19); the IR's provisional interface numbering (the
  lock design's L4, driftsys/ridl#391)
- Replaced later by: driftsys/ridl#317 (the `repr_c` size column) and
  driftsys/ridl#704 (the clause translator). The payload stand-in, the test-only
  ports, the zero catalog hash, the `None` `flatbuffers` size column and the
  unemitted catalog check are already replaced
- Reasoning trail (archived):
  [`2026-09-15-lane-m-driver.md`](../archive/2026-09-15-lane-m-driver.md),
  [`2026-09-16-interaction-face-v0-design.md`](../archive/2026-09-16-interaction-face-v0-design.md),
  [`2026-09-17-interaction-face-v0-plan.md`](../archive/2026-09-17-interaction-face-v0-plan.md);
  [`2026-09-25-lane-f-driver.md`](../archive/2026-09-25-lane-f-driver.md),
  [`2026-09-25-async-face-design.md`](../archive/2026-09-25-async-face-design.md),
  [`2026-09-25-async-face-plan.md`](../archive/2026-09-25-async-face-plan.md)
- `crates/ridl-backend-rust/src/descriptors.rs`, `src/face.rs` and its
  submodules under `src/face/`, `src/clauses.rs`, `src/lib.rs` (`generate_face`,
  `generate_pipeline`) — the emitter as built
- `crates/ridl-backend-rust/tests/fixtures/interaction_face.ridl`,
  `tests/generated/interaction_face.rs`, `tests/interaction_face.rs`,
  `tests/interaction_face_regeneration.rs`, `tests/support/doubles.rs` — the
  fixture, the checked-in output, the round trips, the regeneration guard and
  the port doubles; the ports the round trips run over are
  `crates/ridl-loopback/`
- `examples/cabin/consumer/src/main.rs`, `crates/ridlc/tests/cabin_example.rs`,
  the `demo` and `compat-check` recipes of `justfile` — the emitted crate built,
  linked and run outside this workspace's own crates
