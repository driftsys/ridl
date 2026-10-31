# `ridl-loopback` — the in-process reference runtime

`ridl-loopback` is the first runtime in this workspace. It implements the
fourteen port traits of `ridl-rt`'s `port` module over one in-memory store, so a
generated interaction face can be built over a runtime rather than over a test
double.
[ADR-0020](../decisions/ADR-0020-third-encoding-runtime-layering-and-plugin-system.md)
decision 6 names the crate and fixes `ridl-rt` as its only dependency;
[ADR-0021](../decisions/ADR-0021-ridl-rt-0.1-api-and-release.md) decisions 11
and 12 fix the forwarding impls it relies on and the handle model it presents;
[ADR-0023](../decisions/ADR-0023-interaction-face-generation.md) decision 5
fixes that a face holds its port by value, which is what a handle is passed to.

This is the architecture as built. The code and its tests are the source of
truth; where this record and the code differ, this record is corrected. Read it
together with [the `ridl-rt` design record](ridl-rt.md), whose "The ports"
section states the contract every section below implements.

## Scope

Three things this crate is not.

- **Not a transport.** It carries no frame, opens no socket and has no wire
  format. A value published through it is read through it, in the same process.
  `ridl-transport-ws` is the transport, and it does not link this crate.
- **Not the engine.** The store here is a map and a queue behind one lock. The
  seqlock store, the sans-IO session, the platform traits and the scheduler are
  outside this repository (the 2026-09-12 re-scope, section 3.7).
- **Not a checker.** Payload bytes are opaque to it.

What it is for: the round trips of the generated face, an example a reader can
run, and a single-process application. It is a reference in the sense that a
second runtime can be read against it, not in the sense that it is the runtime a
product ships.

## The crate and its dependencies

**`crates/ridl-loopback/` is a `std` crate depending on `ridl-rt` alone, and it
declares no cargo feature.** It is a runtime: it owns a store, hands out
handles, and runs on a host, so `no_std` buys it nothing that a consumer of it
could use. A feature would have nothing to gate — the crate implements the port
traits and nothing else, and every port trait is implemented, so there is no
subset for a consumer to turn off.

The alternative rejected is a `no_std`, single-threaded loopback, over `Cell`
and `RefCell` rather than a mutex. ADR-0021 decision 12 keeps that shape
supported — no port trait carries `Send` or `Sync` as a supertrait, exactly so
such a runtime remains possible — but nothing asks for it yet, and writing it
now would fix a second shape before there is a consumer to fix it against. It
stays available as a later variant, and it is a variant rather than a
replacement, because a single-threaded runtime cannot present the `Send + Sync`
reader handle this one does.

`just wasm-check`'s package list does not gain this crate. That list is the
packages a wasm consumer compiles — the compiler crates and `ridl-rt`, which
generated code links — and a host runtime is not among them.

**The loopback's default encoding is FlatBuffers.** `ridl-rt`'s `Encoded` trait
has each port type state the encoding of the bytes it carries, and leaves the
default to each transport, chosen from how coupled the transport's two ends are.
The loopback's two ends are one process, and FlatBuffers is the codec that reads
a payload in place, so `Loopback::new(catalog)` builds a runtime whose ports
carry `FlatBuffers`. `Loopback::<E>::with_encoding(catalog)` names another
encoding. The crate declares no cargo feature for the default: a feature must be
additive, and a default is a choice between alternatives. The encoding is
carried as a type and never applied, because the bytes are opaque to this
runtime.

## One store, one lock

**Every handle of one runtime holds the same `Arc<Mutex<Store>>`.** The store is
`crates/ridl-loopback/src/store.rs`: the signal map, the per interface
generation counters, the provisioned `fixed` values, one queue per event source,
the call table, the clock, and every stored waker. Each queued occurrence keeps
its interface, ordinal, payload bytes, envelope and the trace context it was
raised with.

A port method that reaches the store takes the lock, does its work and returns.
None waits for data while holding it, which is what `ridl_rt::port` requires of
every port method: `EventSource::next` returns `Ok(None)` when nothing is
queued, `Caller::reply` returns `Ok(None)` while the outcome is unknown, and
`Handler::next_claim` returns `Ok(None)` when no call is waiting. A critical
section that reads or writes a map and returns is not waiting.

**`RwLock` is rejected for 0.1.** The property ADR-0021 decision 12 protects is
that a signal read does not block on a publication, and the decision's own
rejected alternative — one runtime struct implementing every port, shared behind
a mutex — fails it for a different reason than lock kind: there, every read
waits behind every commit **for the lifetime of the borrow a face holds**, not
for the length of a critical section. Here the handles are separate values, and
a read waits only for the length of a commit's own critical section, which
writes the staged entries and increments the generation of one interface. In an
in-process reference that is not the bottleneck, and a `Mutex` is the lock whose
cost matches what this store does: every path that is not a plain read —
`commit`, `raise`, `send`, `next_claim`, `settle` — writes, so a lock that let
readers in together would be taken for writing on most calls anyway.

A reader that must not block at all is the seqlock store, which is the engine's
and outside this repository.

`ridl-loopback` recovers a poisoned lock rather than propagating the poison
(`handle::lock`). Every critical section leaves the maps well formed, so a panic
in one test would otherwise turn every later port call over the same runtime
into a second panic that hides the first.

The maps are `BTreeMap`s, not hash maps, so that a commit applies its staged
changes and a scan reports its changes in one order on every run. A reference
runtime a test compares output against has no reason to be nondeterministic.

## The handles

**One handle type per port role rather than one type implementing them all, plus
an aggregate**, which is ADR-0021 decision 12's shape and this story's
`Done when`. The six here group eleven roles the way that decision derives the
threading split: a handle each for the five roles with a `&mut self` method, and
one handle for the six whose methods all take `&self`, which are exactly the
roles several threads may hold at once. The twelfth port trait, `Wakeable`, is
implemented on every handle rather than grouped, because each handle wakes its
own waiters ("Waking", below). The two stream extensions, `StreamCaller` and
`StreamHandler`, are on the caller and the handler handle, beside the role they
extend.

Every handle implements `Attached`, which every port trait but `Clock` and
`Wakeable` carries as a supertrait, and `Encoded`. The table lists what each
handle adds to them.

| Handle          | Port roles beside `Attached` and `Encoded`                                                | Threading     |
| --------------- | ----------------------------------------------------------------------------------------- | ------------- |
| `ReaderHandle`  | `Clock`, `SignalReader`, `FixedReader`, `ScannableSignals`, `CoherentSignals`, `Wakeable` | `Send + Sync` |
| `WriterHandle`  | `SignalWriter`, `Wakeable`                                                                | `Send`        |
| `SourceHandle`  | `EventSource`, `Wakeable`                                                                 | `Send`        |
| `SinkHandle`    | `EventSink`, `Wakeable`                                                                   | `Send`        |
| `CallerHandle`  | `Clock`, `Caller`, `StreamCaller`, `Wakeable`                                             | `Send`        |
| `HandlerHandle` | `Handler`, `StreamHandler`, `Wakeable`                                                    | `Send`        |

The caller handle carries `Clock` as well as the reader handle, because a
generated async `Client` over an interface with a call is bound on `Caller`,
`Clock` and `Wakeable` together
([ADR-0023](../decisions/ADR-0023-interaction-face-generation.md) decision 6),
and a role handle that cannot build one is not a handle for that role. The Rust
backend emits that client; the roles were here first, so that it could be built
over this handle. Both handles read the one clock in the store.

The split follows the receiver, as ADR-0021 decision 12 derives it: every method
on the reader handle takes `&self`, so several threads may read one store at
once; every other handle carries a trait with a `&mut self` method, so one
thread drives each. Neither property is declared. Both follow from the fields,
and `crates/ridl-loopback/src/lib.rs` asserts them at compile time with
`assert_sync::<ReaderHandle>()` and `assert_send` on each of the six, which is
the assertion ADR-0021 decision 12 asks a runtime crate to carry.
`a_writer_handle_publishes_on_one_thread_while_a_reader_reads_on_another` and
`a_reader_handle_is_shared_between_threads`, in
`crates/ridl-loopback/tests/ports.rs`, run the two properties rather than only
asserting them.

**`Loopback` is the aggregate.** It holds one of each handle, implements all
fourteen port traits by delegating to the one that has each, and is what
`Loopback::new(catalog)` returns. It exists because a generated `Client` is
commonly bound over `SignalReader + EventSource + Caller` at once and no single
role handle satisfies a multi-trait bound (ADR-0021 decision 12, second
amendment). It reaches a face by value, or as `&mut` under the forwarding impls
of ADR-0021 decision 11.

**Every handle is typed by the encoding its ports carry.** `Loopback<E>`,
`Handles<E>` and each of the six role handles take one type parameter
`E: Encoding`, defaulting to `FlatBuffers`, and implement `Encoded` with
`type Encoding = E`. The parameter is a `PhantomData` field on each handle and
takes part in no method. All six role handles carry it, not only the aggregate,
so that ADR-0021 decision 12's rule that a face may be built over a role handle
stays true when the face reads its encoding from its port: a face that needs
only `SignalReader` is built over a `ReaderHandle<E>` and reads `E` from it.
Every way of obtaining a handle below preserves `E`: `split` hands out
`Handles<E>`, `attach` returns `Loopback<E>`, and `reader()` to `handler()`
return the `E`-typed handle, so one store has one encoding. `new` is defined in
an `impl Loopback<FlatBuffers>` block, which is what lets
`Loopback::new(catalog)` and `Loopback::SLOTS` resolve with no type argument;
Rust ignores a type parameter's default in expression position, so a constructor
in the generic block would need a turbofish on every call. `SLOTS` sits in that
block for the same reason: the count is the store's and the same under every
encoding. A program that names the types bare — `Loopback`, `HandlerHandle` —
names the FlatBuffers runtime, and is unchanged by the parameter.

Four ways to obtain handles, each with one purpose:

- `Loopback::new(catalog)` — the aggregate, with its six handles inside, typed
  FlatBuffers; `Loopback::<E>::with_encoding(catalog)` is the same aggregate
  typed `E`.
- `Loopback::split(self)` — consumes the aggregate and hands out the six handles
  it holds, as a `Handles` struct with one named field per role. A struct rather
  than a six-element tuple, because a tuple of six values of six distinct types
  is read by position.
- `Loopback::reader()`, `writer()`, `source()`, `sink()`, `caller()`,
  `handler()` — an **additional** handle of that role on the same store, each
  with its own per handle state. This is how a test builds two callers on one
  provider, or two event sources of one event.
- `Loopback::attach(&self)` — an **additional** aggregate on the same store,
  whose six handles are opened as those six methods open one each. This is how
  an application holds several faces over one runtime, each owning its own
  aggregate: a `Client` and a `Publisher`, or two clients, one with a call in
  flight while the other waits on `next_event`. A face holds its port for as
  long as it lives, and a call's future and `next_event`'s future hold the
  client's port for as long as they live, so one aggregate serves one face at a
  time (driftsys/ridl#488).

`advance`, `provision_fixed`, `set_limit` and `fail_next_settle` are the
aggregate's, not any role handle's: each acts on the runtime as a whole rather
than through a port, so it has the same effect whichever aggregate of the store
it is called on.

What an attached aggregate shares follows from the next section's rule: what is
in the store is shared, and what is on a handle is not carried over. It starts
subscribed to nothing, with nothing staged, no call sent and nothing served, and
dropping it closes only its own six handles.

The alternative rejected is `Clone` on the aggregate and the role handles, which
driftsys/ridl#488 first proposed. A derived `Clone` copies the per handle
identity, and three handles close that identity in their `Drop`: dropping one
copy would close the other's subscriptions, forget its calls, or return its
claims, and two copies of one source would take each other's occurrences. A
hand-written `Clone` that opens new identities is `attach` under a name that
promises a copy it cannot make: a call's outcome belongs to the one caller that
sent it and a claim to the one handler it was presented to, so neither can be
carried to a second handle. A receiver type that holds a queue is commonly not
`Clone` for the same reason (`tokio::sync::broadcast::Receiver` offers
`resubscribe` instead). No code in this workspace bounds a port on `Clone`, and
ADR-0021 decision 11 names a `Clone` handle only as one of the shapes a face
accepts, not as one a runtime must provide.

## State on a handle, state in the store

One rule decides where a piece of state lives: state two handles must agree on
is in the store, and state belonging to one handle alone is on that handle.

On a handle: a writer's staged changes and its per channel sequence counters, a
sink's and a caller's sequence counters, and a source's, a caller's and a
handler's identity. In the store: the published signals, the generations, the
`fixed` values, the event queues, the call table, the handlers' served sets, the
stored wakers, and the clock. A served set is in the store because a caller's
send reads it to know which handler to wake; the handler handle keeps a copy for
`HandlerHandle::served`, which returns a slice, and `serve` is the one method
that changes both. A waker is in the store because another handle's operation
wakes it.

Staging on the writer handle is the visible consequence: `set`, `invalidate` and
`touch` take no lock at all, and `commit` takes it once. A test pins that a
staged value is not visible to a reader until the commit.

Staging is one entry per channel, and two rules decide what a second operation
on one channel does before the commit:

- **A `set` or an `invalidate` replaces whatever is staged**, because each is a
  newer decision about that channel. An `invalidate` staged over a `set`
  publishes the invalid state carrying the last good value, which is the value
  the channel last published, not the one this commit staged: the staged value
  was never published, so it was never good (ridl §4.5).
- **A `touch` stages only when nothing else is staged.** It re-affirms the
  current value without a new one, and a `set` or an `invalidate` already staged
  is itself a publication, so a touch adds nothing to it. Letting it replace one
  would discard a value this writer staged, which is not what `touch` means.
  `a_touch_does_not_discard_a_value_staged_before_it` and
  `a_touch_does_not_discard_an_invalidation_staged_before_it`, in the
  `ridl-rt-conformance` suite this runtime runs, are the two cases.

One staged operation is dropped rather than applied: a `touch` of a channel with
no publication. A channel that has never published has nothing to re-affirm, so
applying it would publish a zero-length value as `Provenance::Live` — which a
consumer's binding reads as a corrupt payload rather than as the init value ridl
§4.4 gives it. It is dropped before the generation is advanced, so a commit
whose every staged change is such a touch changes nothing at all.

**An `invalidate` of a channel with no publication is not dropped**, although it
publishes a zero-length value too. The two differ in what they assert: a touch
asserts nothing a consumer did not already have, while an invalidate is the ridl
§4.5 transition to the invalid state, and dropping it would lose a provider's
declared state change silently. The generated client reports it as the init
value under `Provenance::Invalid(Cause::Declared)`, the provider's own
provenance, the same way it reports an unpublished read as the init value under
`Provenance::Init`: the accessor checks the port's reported provenance and
length before running `Payload::verify`, and skips it only when the provenance
is `Init` or `Invalid(Declared)` and the length is zero (driftsys/ridl#517).
Under every other provenance, `verify` still runs, including over zero bytes: a
zero-length `Live` sample, for instance, runs `verify` and is reported as
`Invalid(Detected(Corrupt))`, not as the init value. The generated `Publisher`
does emit `invalidate_<name>`, so a provider that invalidates before its first
`set` reaches this path.

## Waking

**Every handle implements `Wakeable`, and a handle stores a waker only under a
kind of key one of its roles observes**. A caller handle observes `Outcome`,
`Slot` and `Stream`, a source handle `Event`, and a handler handle `Claim` and
`ClaimStream`. For `Slot`, `Event` and `Claim` the store keeps one waker per
kind of key per handle, as ADR-0021 decision 13 states the contract: one `Slot`
waker on a caller, one `Event` waker on a source and one `Claim` waker on a
handler, whatever interface each was registered under, and a change to any key
of the kind wakes the stored waker, so a task that registers `Event(a)` and then
`Event(b)` on one source is woken by an occurrence of either. Each handle's
three are a `ridl_rt::correlate::Waiters`. An `Outcome` waker is per call: it is
kept with its call in the slot of the call table, `ridl_rt::correlate::Table`,
because the outcome is the call's, and no change but that call's settlement, its
`forget`, or the drop of the caller handle that sent it wakes it (a displacement
by another task does, as for every kind). A `Stream` waker is per stream of a
call, and is kept with the call in the same slot, through
`Table::wake_on_stream`, one for each of up to
`ridl_rt::correlate::STREAM_WAKERS` streams. A `ClaimStream` waker is per stream
of a claim, and is kept with the stream itself on the call's entry in the store,
because `Waiters` and `Table` hold no provider-side key. The aggregate sends
each key to the handle that observes it.

What wakes a stored waker:

| Kind          | Woken by                                                                                                                                                                                                                                         |
| ------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `Outcome`     | the settlement that records the outcome of the call, or a `forget` of the call while it is in flight, or the drop of the caller handle that sent it                                                                                              |
| `Slot`        | a reclaimed slot of the call table: a `forget` of a settled call or of a call no handler has claimed, the settlement of a claimed call forgotten while in flight, the drop of the handler that held such a call, or another caller handle's drop |
| `Event`       | a raise that queues an occurrence for this source                                                                                                                                                                                                |
| `Claim`       | a send of a member this handler serves, a `serve` that admits a call already waiting, or another handler's drop that returns a claim this handler serves                                                                                         |
| `Stream`      | on the stream: a chunk or the end the provider pushes, an abort by the provider, or a pull by the provider that frees the window; on the call: its settlement, its `forget`, or the drop of the caller handle that sent it                       |
| `ClaimStream` | on the stream: a chunk or the end the caller pushes, an abort by the caller, or a pull by the caller that frees the window; on the call: the caller's `forget`, the drop of the caller handle, or the drop of this handler                       |

A `serve` takes the handler's `Claim` waker before it scans the waiting calls,
and puts it back, unwoken, when no call the handler now serves is waiting. A
`serve` with no waker registered therefore scans nothing, as it did before the
waker moved into a `ridl_rt::correlate::Waiters`, which has no query for an
empty kind. This is the one change the call-table move makes on the claim side,
and it changes nothing about what wakes the waker.

A settlement, a raise, a send, a `serve`, a `forget`, a caller's drop, a
handler's drop, and a stream's push, end, abort and pull collect the wakers to
wake inside their critical section, and wake them after the lock is released, so
no waker is woken while the store is locked: a waker runs code this runtime does
not control, and that code may call a port method on the same store. A waker is
cloned under the lock, and a refreshed one is dropped under it; neither wakes a
task. **A dropped source, caller or handler drops its stored wakers after the
lock is released**, not under it: the store may hold the last reference to a
waker whose task owns another handle of this runtime, and dropping that waker
drops the handle, whose own `Drop` takes the lock. Review pass 1 on
driftsys/ridl#553 reproduced that deadlock for all three handles, the source's
and the handler's predating the caller's; the `…_drops_without_deadlock` tests
pin the fix. A reclaimed call's stream wakers, the caller's from the table and
the provider's from the call's entry, are put on the list to wake for the same
reason, and so are a dropped handler's `ClaimStream` wakers;
`a_stream_waker_whose_task_holds_a_handle_drops_without_deadlock` pins it for a
`Stream` waker.

**A registration of the waker already stored does not wake it.** This is the
contract's refresh rule (ADR-0021 decision 13): a task registers on every poll,
so a waker for the same task (`Waker::will_wake`) replaces the stored one
without waking it. A waker for another task displaces the stored one, and the
displaced waker is woken.

Three choices the `Wakeable` contract leaves to a runtime, and this one's
answer:

- **A registration whose key already holds is woken at once**, rather than
  stored: an outcome already recorded, an occurrence already queued for the
  source, a call the handler serves already waiting. The same holds for an
  `Outcome` of a correlation that names no call in flight — unknown or forgotten
  — for which no outcome will ever be recorded, and for a `Slot` registration
  while a slot of the call table is free.
- **A reclaim wakes every caller's `Slot` waiter**, not one of them: the first
  to send again takes the slot, and the others find the table full, answer
  `SendError::Busy` again, and register again. A queue of waiters, woken one at
  a time, would need storage the allocation-free table cannot hold, and a waiter
  that has no slot yet has no slot to be kept in (note F-5).
- **A dropped caller forgets every call it sent and did not forget**, as
  `Caller::forget` would: a settled call's slot and the slot of a call no
  handler has claimed are reclaimed at the drop, which wakes the other callers'
  `Slot` waiters, and a claimed call in flight is marked, so its settlement
  reclaims the slot. No handle reads the outcome of a call whose caller is gone,
  and a slot kept for it would be lost to every other caller. The drop also
  removes the caller's own `Slot` waker.
- **A stream registration is stored, not checked.** A `Stream` or `ClaimStream`
  registration is woken at once only when no stream of that call can change
  again: the correlation is unknown or forgotten, or the claim is not the
  handler's. A chunk already waiting does not wake it, because the task reads
  the stream after it registers.
- **A kind no role of the handle observes is woken at once.** The reader, writer
  and sink handles observe no kind of key, the source handle observes one, and
  the caller and handler handles observe three and two, so a registration under
  any other kind has nothing that could change under it, and a stored waker
  would never be woken.

**A handler dropped while it holds an unsettled claim returns the claim to the
waiting calls**, in its place by send order, so another handler that serves the
member can take it, and every handler that serves the member is woken. The
loopback enforces no deadline on the returned call: what bounds the caller's
wait is the deadline the generated async client's future measures (ADR-0023
decision 6). This and the offered claim ("An offered claim", below) are the two
ways a call is presented again, and a call is presented once more for each
holder dropped. A returned claim keeps the id it was first presented under,
because the id lives on the call's entry (driftsys/ridl#569). A dropped
handler's offered claim is not returned, because it never left the waiting
calls, and no handler is woken for it. A claim whose call the caller forgot is
not returned: it is withdrawn, its slot is reclaimed, and no handler is woken
for it (see "`Caller::forget` releases the caller's interest" below). A taken
claim whose call has any stream state is not returned either, because another
handler would read partial streams: every unfinished stream of the call stops
with `Transport::Interrupted`, and the call settles `Err` with the same error,
which wakes its `Outcome` and `Stream` waiters
(`a_dropped_handler_interrupts_a_taken_call_whose_streams_moved`). The same
`Drop` removes the handler's state from the store, its stored `Claim` waker with
it, so no later send reaches a waker of a handler that is gone.

**The limit, stated.** This runtime measures no bound: its clock moves only
under `Loopback::advance`, and nothing here settles a call as `Undelivered` or
`Timeout` when its `max` passes, so nothing wakes a task when a call's deadline
passes. A call over this runtime is polled again when a key wakes it — its
settlement, its `forget`, or a reclaimed slot while it waits for one — or when
the executor polls it for its own reasons: a blocking client's park timeout, or
a frame loop's cadence. Under a general executor with no timeout, a call whose
provider never settles it waits for as long as that is true. That is this
runtime's limit, not a defect of the face, whose future has no timer of its own
(ADR-0023 decision 6, "The bound"); whether this runtime ever measures a bound
is not decided.

The contract cases are in the `ridl-rt-conformance` suite this runtime runs, in
its `wakeable` module and in the two slot cases of its `calls` module;
`crates/ridl-loopback/tests/ports.rs` keeps, under "Waking" and "The bounded
call table", the tests of what this runtime chooses where the contract leaves
the choice open. Removing the wake from the settlement path turns red
`an_outcome_waker_is_kept_with_its_call_and_woken_once_by_the_settlement` and
every other suite case that waits for a settlement;
`every_wake_runs_with_the_runtime_lock_released` in the suite and
`every_wake_is_run_with_the_lock_released` and
`every_wake_at_once_and_every_displacement_is_run_with_the_lock_released` in
`ports.rs` fail when a waker is woken with the lock held, on the wake paths of a
change, of a registration whose key already holds, and of a displacement;
removing the `Slot` wake from a reclaim fails
`every_slot_waker_is_woken_by_a_reclaim_and_the_send_that_follows_succeeds` and
three other suite cases. In `ports.rs`,
`a_dropped_handler_returns_its_claims_to_the_waiting_calls` fails without the
handler's `Drop`, and `a_dropped_handler_leaves_no_waiter_behind` fails when
that `Drop` returns the claims but leaves the handler's state in the store.

## A claim is not a correlation

The call table is `ridl_rt::correlate::Table`, with `Loopback::SLOTS` — sixteen
— slots and no byte budget, because the loopback has no catalog descriptor to
size one from (note F-9): the descriptor file is written by `--emit catalog`
(driftsys/ridl#381), and nothing yet wires the loopback to read one. A call
holds a slot from the caller's send until `forget`, or the drop of the caller
handle that sent it, reclaims it; a send with every slot taken answers
`SendError::Busy`, on every caller handle, because the table is the runtime's. A
correlation is `(generation << 16) | slot`, and a reclaim advances the slot's
generation, so the correlation a reused slot had before answers as unknown:
`ack` and `reply` answer `None`, and an `Outcome` registration under it is woken
at once. The table holds the outcome status and the `Outcome` waker; the
loopback keeps each call's arguments, its envelope, the trace context it was
sent with, its place in send order and its reply bytes beside it, by slot. A
returned claim goes back among the waiting calls by send order, not by
correlation, because a reused slot's correlation is larger than that of a call
sent later into a fresh slot. A refused send draws no sequence number, because
nothing was sent. Two identities address a call, and they are separate:

- a **`Correlation`**, returned by `command` and `query`, is the caller's name
  for the outcome it will read back;
- a **`ClaimId`**, minted by `next_claim`, is the provider's name for a call it
  has been presented.

A claim also records the handler it was presented to. Between them, those two
facts make `SettleError::UnknownClaim` mean what `ridl_rt::port` says it means —
"the claim was already settled, or was never issued":

- a settlement of a call that was never presented is refused, so a call cannot
  be acknowledged before a provider has seen it;
- a settlement of a claim another handler holds is refused, so two providers in
  one process settle their own calls and not each other's;
- a claim leaves the table when it settles, so a second settlement of it is
  refused too.

`a_claim_that_was_never_presented_cannot_be_settled`,
`a_handler_cannot_settle_another_handlers_claim` and
`a_claim_is_presented_once_and_settled_once`, in the `ridl-rt-conformance` suite
this runtime runs, are the three cases. The alternative rejected is one identity
for both ends, which is what the deleted double had and what this crate had
before its review: with it, `settle(ClaimId(correlation.0))` before any
presentation recorded an outcome the caller could read as an acknowledgment.

`Loopback::fail_next_settle` is not scoped this way: it is the runtime's, so it
fails whichever handler settles next.

**A call's streams are kept on its entry, by stream number.** Each stream holds
the chunks pushed and not yet pulled, whether its producer ended it, and the
outcome it stopped with and the side that stopped it; a stream has state from
the first operation that names it, and a pull of a stream nothing named yet
answers `Pull::Pending`. The loopback has no member table, so it accepts every
stream number on every call, and a stream number is unknown only when its call
is. A stream parameter that an operation named and that the provider never
reads, such as a stream number the member does not declare, holds the call's
slot after a forget until the handler drops. The loopback also accepts
`REPLY_STREAM` on a command's claim, and `end_claim(REPLY_STREAM)` there
acknowledges the command. Each stream has one producer: the provider pushes the
reply stream (`REPLY_STREAM`) and the caller pushes each stream parameter, and a
push, an end or a pull from the other side answers
`Contract::UnknownInteraction`; an abort is accepted from either side. A stream
holds `Loopback::STREAM_WINDOW` — four — non-empty chunks; a push of a non-empty
chunk into a full stream answers 0 and takes nothing, and an empty chunk is
always taken. Because an empty chunk is always taken, as the contract requires,
the window does not bound a producer that pushes empty chunks and never ends the
stream: each one is stored until it is pulled. A chunk longer than the member's
limit is refused with `PushError::TooLarge`, under the limit a settlement's
reply has.

The caller addresses a call's streams by its correlation while the table holds
it and the caller has not forgotten it. The provider addresses them by the claim
id while it holds the claim, and after it settled the claim too, until the call
is forgotten: a command is settled at its acknowledgment, before its parameter
streams are read, so the entry records the handler that settled it. A settlement
`Ok` ends the reply stream, and a settlement with an error stops it with that
error, after the chunks already pushed; `end_claim` on the reply stream of a
claim still held is a settlement `Ok` with no reply bytes, and `abort_claim` on
it a settlement with the error. A `forget` stops every stream of the call that
has not ended with `Transport::Interrupted`, keeps every stream its producer
ended, and a stream first named after it starts stopped the same way. After the
forget, the provider's push or end of the reply stream answers
`PushError::Failed(Interrupted)`. **A forget waits for the provider's streams**:
when a handler holds or settled the call and the provider has not yet pulled
every stream parameter to `End` or `Failed`, the store marks the call forgotten
on its entry and does not tell the table, so the slot stays taken; the caller's
correlation already answers as forgotten, and its `Outcome` and `Stream` waiters
are woken. The provider's pull that reads the last of those ends, a settlement
after it, or the handler's drop completes the forget through `Table::forget`. An
`abort_claim` of a stream parameter counts as the provider's end of it, and the
settlement of a query releases its stream parameters (their unread chunks are
dropped, and the caller's next push answers `Failed(Interrupted)`); the
settlement of a command does not, because its stream parameters go on after the
acknowledgment. The drop of a handler stops every unfinished stream of a claim
it settled with `Interrupted` and wakes the caller's `Stream` waiters, so a
caller still pushing to an acknowledged command learns the provider is gone.
After the caller refused the reply stream, `end_claim` settles the claim with
that refusal rather than `Ok`. This holds a slot no longer than the provider's
reading, and uses no storage beyond the slot, at the cost of a caller finding
the table full sooner. With no member table, the store counts only the stream
parameters an operation has already named: after the forget of a settled call
none of whose stream parameters was named, the forget completes at once, the
claim is no longer known, and a stream operation on it answers
`Contract::UnknownInteraction`. After every settlement and every forget that the
table does not answer `Unknown`, the store takes the slot's `Stream` wakers with
`Table::take_streams`, and a reclaim also takes the provider's wakers, before
the entry, and the streams with it, are dropped. The stream cases are the
`streams` module of the `ridl-rt-conformance` suite this runtime runs.

**An offered claim (2026-09-28, driftsys/ridl#569).** A call whose argument
bytes do not fit the buffer passed to `next_claim` is offered rather than taken:
the claim id is minted, stored on the call's entry and recorded as the
handler's, the call stays among the waiting calls, and the answer is
`ReadError::ShortClaim { claim, needed, trace }`, where the loopback fills
`trace` with the caller's context
([ADR-0021](../decisions/ADR-0021-ridl-rt-0.1-api-and-release.md) decision 5,
amended the same day). The id lives on the entry, so every later presentation of
the call — by the same handler with a larger buffer, or by another handler that
serves the member, which moves the claim to that handler — carries the same id,
and a claim id names one call for the life of its slot. `settle` accepts the id
whether or not the call was read, and takes the call out of the waiting calls,
so no handler is presented it afterwards. A dropped handler's offered claim is
not re-inserted, because it never left the waiting calls; its id stays on the
entry for the next presentation. A forgotten offered call is treated as a
claimed one, not withdrawn: `forget` marks it and leaves it among the waiting
calls, so the provider's retry with a larger buffer still presents it under the
same id, and its settlement by the handler that holds it, or that handler's
drop, reclaims the slot; the table below has a row for it. Each `ClaimOwner`
records whether its call was taken or only offered, which is what `forget`,
`settle` and the handler's drop read. The conformance suite's
`an_oversized_claim_is_reported_with_its_id_and_is_not_consumed`,
`an_unread_claim_is_settled_by_its_id`,
`the_calls_behind_an_oversized_claim_are_presented_once_it_is_settled` and
`forget_between_the_offer_and_the_settlement_leaves_the_settlement_valid` pin
the port contract; this runtime's `tests/ports.rs` pins the rest:
`a_forgotten_offered_call_is_presented_again_under_the_same_id_and_its_settlement_frees_the_slot`,
`a_dropped_handlers_offered_claim_is_taken_once_by_another_handler_under_the_same_id`,
`a_forgotten_offered_call_is_withdrawn_when_its_handler_drops` and
`an_offered_call_taken_by_another_handler_moves_the_claim_to_it`.

**`Caller::forget` releases the caller's interest, and withdraws only a call no
handler has claimed.** What it does depends on where the call is:

| Call at `forget`                                   | What happens                                                                                                                                                                                                  |
| -------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| settled                                            | its slot is reclaimed now                                                                                                                                                                                     |
| claimed by a handler, not settled                  | marked forgotten; its settlement reclaims the slot (ADR-0021 decision 15)                                                                                                                                     |
| waiting, claimed by no handler                     | withdrawn: it leaves the waiting calls, and its slot is reclaimed now                                                                                                                                         |
| waiting, offered to a handler through `ShortClaim` | marked forgotten and left among the waiting calls, so a retry still presents it under the same id; its settlement by the handler that holds it, or that handler's drop, reclaims the slot (driftsys/ridl#569) |

A claimed call that the caller forgot, and whose handler is then dropped without
settling it, is withdrawn at that drop instead of being returned to the waiting
calls (decision 2 of the
[pass-1 dispositions on driftsys/ridl#557](https://github.com/driftsys/ridl/pull/557#issuecomment-5848835004)).
So no forgotten call ever waits again, and no forgotten call holds a slot that
only a handler which may never come could free.

A claimed call is still settled, because `Handler`'s contract is that every
claim is settled and a caller losing interest is not the provider's business.
Without the withdrawal, a call to a member no handler serves is never settled,
and sixteen such forgotten calls leave every later send `SendError::Busy` for
the life of the runtime (decision 1 of the
[pass-1 dispositions on driftsys/ridl#553](https://github.com/driftsys/ridl/pull/553#issuecomment-5848559640),
with its
[confirmed details](https://github.com/driftsys/ridl/pull/553#issuecomment-5848618786)).
**A withdrawn command is never delivered**: no handler is presented it, so a
command whose caller forgot it before any handler claimed it is delivered at
most once. **`forget` and the handler side — `serve` and `next_claim` — run
under the store's one lock**, so a call is either claimed first, and then held
until it is settled, or withdrawn first, and then never presented. A dropped
caller's calls go through the same path.

The withdrawal is this runtime's behaviour, not a port contract: a transport
that has already sent a request cannot recall it. The `ridl-rt-conformance` case
`forget_before_the_claim_is_presented_withdraws_or_leaves_the_call` therefore
accepts either result — the call withdrawn, or presented and held until settled.

The withdrawal adds nothing to `ridl-rt`'s API. Under the store's lock, the
store takes a waiting call out of the waiting calls, settles it through
`Table::settle`, and then forgets it through `Table::forget`, which answers
`Forgotten::Reclaimed` for a settled call. A claimed call the caller forgot is
already marked in the table, which has no query for the mark, so the loopback
keeps its own mark on the call's entry; at the handler's drop it settles that
call through `Table::settle`, which answers `Settled::Reclaimed` for a forgotten
call. The outcome it settles with is discarded with the slot, and no reader can
see it in between; it is `Transport::Undelivered`, the outcome that describes a
call no provider settled. A withdrawal wakes the call's `Outcome` waiter and
every caller's `Slot` waiter, as any reclaim does, and no handler's `Claim`
waiter. Either way the correlation answers `None` from `ack` and `reply`
afterwards, which is what `Caller::forget` tells a caller to expect. The
alternative rejected is removing a claimed call's entry outright: it revokes a
claim the provider already holds, so the provider's `settle` fails and the
generated `dispatch` does not count it — and, in the shape this crate first had,
it left the call's identity in the waiting queue with no entry behind it, which
made every later `next_claim` on that runtime panic. The withdrawal takes the
call out of the waiting queue before it drops the entry for that reason.
`forgotten_calls_to_an_unserved_member_leave_room_for_another_send`,
`a_withdrawn_call_is_never_presented_to_a_handler_that_serves_the_member_later`,
`a_claimed_then_forgotten_call_holds_its_slot_until_its_settlement`,
`a_dropped_callers_unclaimed_calls_are_withdrawn`,
`a_withdrawal_from_the_middle_of_the_queue_leaves_the_calls_around_it_in_order`,
`a_stale_correlation_does_not_withdraw_the_call_now_in_its_slot`,
`a_call_forgotten_while_claimed_is_withdrawn_when_its_handler_is_dropped`,
`a_withdrawal_at_a_handlers_drop_wakes_no_claim_waiter`,
`a_forget_withdrawal_wakes_no_claim_waiter_of_a_handler_serving_another_member`,
`a_dropped_handler_withdraws_only_its_forgotten_claim_and_returns_the_others`
and `a_dropped_callers_claimed_call_is_withdrawn_when_its_handler_is_dropped`
pin the rule.

## The two signal extensions are implemented

**`ScannableSignals` and `CoherentSignals` are both implemented on the reader
handle.** `ridl_rt::port` permits a runtime to omit either, and the test double
this crate replaces omitted both. Implementing them is what makes this a
reference runtime rather than a second double: they are the two mechanisms a
consumer of a store-shaped runtime actually uses, and under one store and one
lock each is a few lines.

- **`generation(iface)`** is a counter per interface that each commit to that
  interface increments once, however many signals the commit carries.
- **`scan`** selects on the generation: each signal entry records the generation
  it last changed at, and a scan reports the entries of one interface whose
  recorded generation is above the caller's mark. An interface's changes are
  written all together or not at all; on the first interface whose changes do
  not fit in what is left of `out`, the scan stops with that interface's mark
  untouched and returns what it wrote. That is the reading of "returns the
  number of entries written so far" that keeps the order of `marks` a priority
  order and keeps the `ScannableSignals::scan` loop — grow `out` when the call
  returns 0 and a mark is still behind — the way to make progress.
- **`Watermark`** records the interface and the generation last scanned, with no
  sequence number; `scan` moves the mark to the interface's current generation
  when it writes that interface's changes.
- **`read_coherent`** answers every ordinal from the one publication state the
  lock is held over, which is what makes the read coherent. It reports
  `ReadError::Short` with the size of the whole set, not of the first value, and
  `ReadError::TooFewSamples` when `samples` is shorter than `ords`.

Neither turned out to be large, so neither is deferred.

## The clock is hand-driven

**`Clock::now` reads a counter that only `Loopback::advance(Duration)` moves.**
It never reads wall-clock time, so a round trip over this runtime produces the
same timestamps on every run and on every machine, and a test can place a
publication at an exact time. Two runtimes constructed at different real times
start at the same logical time, which
`two_runtimes_start_at_the_same_logical_time` in
`crates/ridl-loopback/tests/ports.rs` pins.

`advance` panics on a negative duration and saturates rather than overflowing: a
clock that ran backwards would put an envelope before one already stamped, and
the panic a debug build gives on overflow would be a panic inside a port call. A
panic is the failure mode because `advance` is the aggregate's own method and
not a port method, so no port contract has a way to report it.

A wall-clock variant is rejected for this story rather than forever: it would
make every envelope timestamp in every test a value the test cannot state, and
nothing needs real time until something measures a real bound. A runtime that
does needs a second clock source, which is a later option and not this story.

## Sequence numbers are per sender handle

**Each sender handle owns its counters** (driftsys/ridl#308), and what they are
scoped to follows the interaction:

- a **caller handle** keeps one counter for the whole handle, because on a call
  the scope is the caller instance — ADR-0021 decision 5 states it as
  "`envelope.seq` is unique per caller, not per channel";
- a **writer handle** and a **sink handle** keep one counter per channel,
  because on a signal or an event ridl §3.1 scopes the number to the channel,
  and the channel has one provider.

The first publication, the first occurrence and the first call are `seq` 1,
because `seq` 0 is the envelope of a channel with no publication (ADR-0021
decision 5).

**What #308 reports is not fixed by the counter, and this runtime shows why.**
Two callers each sending their first call both carry `seq` 1 — that is what a
per-caller counter means — and a provider deduplicating on the sequence number
alone would treat the second as a retransmission of the first.
`two_callers_on_one_provider_are_two_claims_under_one_seq`, in the
`ridl-rt-conformance` suite this runtime runs, runs exactly that case and shows
two claims under one `seq`, which is the rule ADR-0021 decision 5 fixes: two
callers are never merged even under the same `seq`. What tells them apart here
is the claim, not the number. A runtime over a real transport keys duplicate
suppression on the caller's transport identity plus `seq`, below the port, for
the same reason.

**The loopback deduplicates nothing.** It presents each call once because it
delivers each call once, not because it recognises a retransmission — nothing
retransmits in a process. Two calls are presented again: a claim a dropped
handler held and did not settle, which returns to the waiting calls for another
serving handler to take ("Waking", above), unless its caller forgot it — that
claim is withdrawn at the drop and is not presented again — and a claim offered
through `ReadError::ShortClaim`, which stays among the waiting calls under the
same id until it is read or settled ("An offered claim", above).

A sink's counters are per channel for a reason a single counter per handle would
break: a consumer subscribed to some of a sink's events would see the numbers of
the events it did not subscribe to as gaps, and `EventSource::next` states that
a gap in `seq` is a loss.
`a_sink_sequence_number_counts_one_channel_publications`, in the
`ridl-rt-conformance` suite this runtime runs, is that case.

The visible consequence of a counter living on the handle is that a writer or a
sink dropped and replaced restarts its channels' counters. A runtime with a
session would carry them; this one has no session. Two writer handles publishing
one signal, or two sinks raising one event, restart it the same way — a misuse
the loopback does not police, because a signal and an event channel each have
one provider (ridl §4, §5).

The alternative rejected is a counter per channel in the store, shared by every
handle. It survives a handle being replaced, but it moves the assignment off the
sender, which ridl §3.1 gives to the sender; and on a call it would make the
number unique per channel, which is the reading ADR-0021 decision 5 replaces.

## Payload bytes are opaque

**The runtime carries bytes and never verifies a payload** (driftsys/ridl#309).
`Payload::verify` belongs to the generated face, on both sides: the consumer
checks what it reads, and the provider's `dispatch` checks the argument bytes it
is handed.

So what happens to an invalid event payload is decided entirely by the generated
face, not here. On the provider side a round trip over this crate pins it:
argument bytes that fail the structure check settle
`CallError::Transport(Transport::Corrupt)`
(`round_trip_malformed_argument_bytes_settle_transport_corrupt`). On the
consumer side, a signal payload that fails its check reports
`Provenance::Invalid(Cause::Detected(Detection::Corrupt))`, which is pinned as
an assertion on the emitted text
(`crates/ridl-backend-rust/tests/face_generation.rs`) and by a round trip over
this crate (`round_trip_signal_with_malformed_bytes_settles_detected_corrupt`,
which writes the malformed bytes directly through `SignalWriter`, bypassing the
generated `Publisher`'s encoder). Two cases have no payload to check at all, and
the face returns the init value under the port's own provenance without running
`Payload::verify`, rather than treating the empty bytes as corrupt
(driftsys/ridl#517): a never-published signal, where the port reports
`Provenance::Init` with zero bytes, pinned by
`round_trip_signal_reads_as_init_before_any_publication` over this crate and by
`ra19_a_minimal_signal_only_port_constructs_the_signal_only_client` over a
hand-written port; and a signal invalidated with no prior publication, where the
port reports `Provenance::Invalid(Cause::Declared)` with zero bytes, pinned by
`round_trip_signal_reads_as_init_when_invalidated_before_any_publication` over
this crate. Nothing here depends on how the reference words the rule, because
this crate never looks; the ridl reference states the event rule (ridl §5.1)
since driftsys/ridl#544, matching frame specification §9.2.

The alternative rejected is a runtime that verifies. It cannot: a port carries
interface numbers, ordinals and bytes and names no payload type, so the runtime
has no `Payload` implementation to call. Giving it one would put the generated
types below the port, which is the layering ADR-0020 decision 6 fixes.

## Member limits

**A sending port refuses a payload longer than its member's limit, with the
limit** ([ADR-0030](../decisions/ADR-0030-transfer-and-bound-limits.md) decision
19). `SignalWriter::set` answers `WriteError::TooLarge { cap }`,
`EventSink::raise` `RaiseError::TooLarge { cap }`, `Caller::command` and
`Caller::query` `SendError::TooLarge { cap }` for the arguments, and
`Handler::settle` `SettleError::TooLarge { cap }` for the reply; `cap` is the
member's limit. A refused value is not staged, a refused occurrence reaches no
source and draws no sequence number, a refused call takes no slot, wakes no
handler and draws no sequence number, and a refused reply records no outcome and
leaves the claim settleable. A `fixed` is provisioned, not sent through a port,
so `Loopback::provision_fixed` makes the same refusal: it returns
`Result<(), WriteError>`, stores nothing on `TooLarge`, and a value provisioned
before stays as it was. A `fixed` whose only provision was refused reads as an
unprovisioned one, `ReadError::Contract(Contract::UnknownInteraction)`, so a
consumer's read of it is answered rather than left waiting.

The limit is the one `Loopback::set_limit(iface, ord, limit)` gave the member,
else the default ceiling of the port's kind
(`ridl_rt::contract::default_ceiling`: 3,840 bytes for a signal and an event,
16,128 for a command and a query, 65,280 for a fixed). The loopback holds no
descriptor, so `set_limit` is how a program tells it the limit a `[limits]`
entry of the manifest gives a member (ADR-0030 decision 12); the table lives in
the store, so every handle and every attached aggregate applies it. The
comparison is made under the lock, on the sending path, before anything changes:
`set` takes the lock once for it, where before it took none, and `raise` draws
the occurrence's sequence number inside the same critical section, after the
check.

The receiving side adds no check. The loopback is not a binding: its reader,
source, handler and caller read only what its sending side accepted, so a second
comparison on receive would never refuse anything.

## What it cannot report

The loopback holds no catalog descriptor. driftsys/ridl#381 writes the
descriptor file; giving the loopback one is not yet assigned to a story. So
there is no member table, and therefore:

- **no unknown ordinal.** Nothing here can tell an ordinal that names no member
  from one that names a member with no value yet, so no port error's `Contract`
  variant is returned except `FixedReader::read_fixed`'s and the stream ports'
  `Contract::UnknownInteraction` for a call or a claim the store does not hold.
  The `read_fixed` one is not an exception to the rule but a case the rule does
  not reach: a `fixed` is provisioned into the runtime rather than published
  through a port, so an ordinal with no provisioned value is a read the store
  cannot serve at all, where an unpublished signal is one it serves as `Init`.
- **no unowned member.** `WriteError::NotOwner`, `RaiseError::NotOwner` and
  `ServeError::NotOwner` are never returned: a provider's ownership is a fact of
  the descriptor.
- **no freshness and no remaining time.** Every sample reads
  `Freshness::Unbounded` and every claim reads `remaining: None`, because both
  are measured against a member's timing annotation.
- **no time to live.** `EventSource::next` discards nothing: the age at which an
  occurrence is discarded is a member's, too.

Three more, for reasons other than the descriptor: nothing detaches, because
every handle holds the store alive, so `Detached` never appears; nothing but the
call table and the size of a payload is bounded, so the one `Busy` this runtime
originates is `SendError::Busy` with every slot taken, and `TooLarge` comes from
a member's limit ("Member limits" above) and from the one injected failure
below, which reports it with `cap: 0`; and `Transport::Busy`, a provider's
refusal at admission, reaches a caller only when a provider settles a call with
it (`a_providers_busy_settlement_reaches_the_caller`); and `Attached::catalog`
returns the `CatalogRef` the runtime was built with, unexamined. ADR-0021
decision 3 places the check of it against an interface's own `CATALOG` in the
generated face's constructor, once, when the face is built; since
driftsys/ridl#381 the constructor the Rust backend emits makes that check and
panics on a mismatch (ADR-0023 decision 8). The check is the face's and not the
runtime's.

Three more that are the runtime's own shape rather than the descriptor's:

- **A settled outcome is kept until the caller releases it.** A call holds its
  slot of the call table until it is forgotten or its caller handle is dropped.
  The generated async client's future forgets its call when it leaves the
  waiting phase: in the poll that takes the outcome, at the call's deadline, and
  on drop while the call is still waiting. So a program that calls through the
  generated face holds one slot per call in flight, and a call future that is
  neither polled to its outcome nor dropped is the one way such a program keeps
  a slot. Before the async client nothing the Rust backend emitted called
  `Caller::forget`, and a program over one `Loopback` found every send
  `SendError::Busy` from its seventeenth call on. This runtime holds the outcome
  because nothing else can know the caller has read it.

  A program that sends through the raw `Caller` port, as some tests do, is the
  one caller that must forget a correlation itself once it has read the outcome,
  or this runtime's sixteen slots fill and it refuses the seventeenth call with
  `SendError::Busy` (decision 2 of the
  [pass-1 dispositions on driftsys/ridl#553](https://github.com/driftsys/ridl/pull/553#issuecomment-5848559640)).
  The generated face's poll methods are `pub(crate)` since the async client, so
  a program that calls through the generated face never holds a correlation.
- **An unpublished channel's envelope is stamped `Timestamp(0)`, not the time
  the channel was created.** `Envelope`'s own documentation gives the creation
  time; this runtime has no channel-creation event — a channel exists when
  something publishes to it — so 0 is the only answer it has.
- **`serve` with an empty slice records nothing**, so it leaves the handler
  unfiltered, the same as never having called it.

Two behaviours are recorded here because they are decisions rather than
absences:

- **An empty served set is no filter, not no members** — a deliberate deviation
  from `Handler::serve`, which says delivery starts at the members listed. A
  handler that has served nothing is presented every waiting call; once it has
  served anything, it is presented only the members it served, and another
  handler's calls stay waiting for that handler. The deviation was taken with
  its eyes open: until the async client nothing the Rust backend emitted called
  `serve`, so a handler that always filtered would have been presented nothing
  at all by the generated `dispatch`, and a handler that never filtered would
  take a second component's calls and settle them `UnknownInteraction` — two
  components providing different interfaces in one process is the plainest use
  of an in-process runtime. The generated `serve` now calls `Handler::serve`
  with the interface's command and query ordinals when it is called
  (`crates/ridl-backend-rust/src/face/serve.rs`), so a handler under it is
  filtered from its first poll, and `blocking::serve` is `block_on` over it.
  **The deviation is kept**, decided with the blocking client: a handler under
  either `serve` never has an empty served set, so the rule reaches only a
  handler driven through the port directly, which is what the tests of the
  settlement table's unknown-route rows do; retiring it would make those tests
  register a set first and change nothing a generated program observes.
  `two_handlers_each_receive_only_what_they_served`, in the
  `ridl-rt-conformance` suite this runtime runs, is that case, and
  `a_handler_that_served_nothing_is_presented_every_call`, in
  `crates/ridl-loopback/tests/ports.rs`, is the other side of the rule. The
  alternative rejected is recording the set without acting on it, which loses a
  call whenever more than one handler exists. `HandlerHandle::served` reads the
  set back.
- **`Loopback::fail_next_settle` is the one fault this runtime injects.** The
  generated `dispatch` counts a claim only once the handler has accepted its
  settlement, and in an in-process runtime nothing else can make that path fail,
  so the count would be untestable without it. It is the one place a
  `SettleError` other than `UnknownClaim` comes from. The claim is looked up
  before the injected failure is consumed, so arming it and then settling a
  claim that does not exist answers `UnknownClaim` and leaves the injection
  armed for the next real settlement.

## It runs the port contract suite

`crates/ridl-loopback/tests/conformance.rs` runs every test of
`ridl-rt-conformance` over this runtime, the tests of both signal extensions and
of `Wakeable` included (driftsys/ridl#514). The factory it writes for the suite
is the aggregate, the `SourceHandle`, `CallerHandle` and `HandlerHandle` that
`Loopback::source`, `Loopback::caller` and `Loopback::handler` hand out,
`Loopback::SLOTS` as the size of the call table the suite fills, and
`Loopback::advance` and `Loopback::fail_next_settle` as the suite's clock hook
and fault hook.

The suite states only what the port contract states. Its handlers call `serve`
before they take a claim, so the deviation from `Handler::serve` recorded under
"What it cannot report" is not exercised by it. It asserts no wake the contract
allows but does not require: because a spurious wake is allowed, it asserts that
a change leaves a waker unwoken only where the contract rules the wake out — a
refresh, or a waker already woken or displaced and so cleared — and it never
asserts that a registration whose key already holds is woken at once. It drops
no handle, and it accepts either result for a forgotten call no handler has
claimed. What the suite leaves out, and why, is listed once, in the crate
documentation of `crates/ridl-rt-conformance/src/lib.rs`.
`crates/ridl-loopback/tests/ports.rs` keeps the tests of this runtime's own
choices — its narrower wakes, its wakes at once, its `Drop`, its withdrawal, and
its own numbers — some of which fall under that list and some of which are this
runtime's alone, and its module documentation names each test with its reason.

## What it replaced

`crates/ridl-backend-rust/tests/support/loopback.rs` was a disposable double
whose own module documentation said to read it out rather than build on it. It
is deleted. In its place:

- `ridl-backend-rust` gains `ridl-loopback` as a dev-dependency, and the
  `round_trip_*` tests in `crates/ridl-backend-rust/tests/interaction_face.rs`
  build their faces over the aggregate handle or, since the async client, over
  the role handles `tests/support/doubles.rs` groups. When the loopback replaced
  the double, the tests' shape and `dispatch` were unchanged; what changed was
  the `use` line, the constructor — `Loopback::new` takes a `CatalogRef` rather
  than a package name, because a runtime is attached to a catalog and not to a
  string — and the deletion of the `support_*` tests.
- Those `support_*` tests moved to `crates/ridl-loopback/tests/ports.rs`, where
  they are tests of the runtime rather than of the Rust backend, alongside the
  tests of what the double did not implement. The ones any runtime can run were
  later moved into `ridl-rt-conformance`.

`MinimalSignalOnlyPort`, in the same test file, is not replaced. It is not a
double of a runtime: it is a port implementing `SignalReader` and `Attached` and
nothing else, and its purpose is to prove that the emitter writes no bound wider
than the interface needs. A runtime implementing every port could not prove
that.

## Observations for other stories

- **The settlement ordering is now observable.** The interaction-face record
  lists the command-settled-before, query-settled-after ordering as pinned only
  by an exact-text assertion, because neither settle nor a provider call in the
  double had an observable side effect. With this crate a provider can hold a
  caller handle on the same store and read the acknowledgment from inside its
  own method, which makes the ordering a behavioural assertion. Writing that
  test is not part of this story.

## Trace

- Roadmap: [`docs/ROADMAP.md`](../ROADMAP.md)
- Tracking issue: driftsys/ridl#445, split from driftsys/ridl#265; `Wakeable`
  and the caller handle's `Clock`: driftsys/ridl#510; the call table on
  `ridl_rt::correlate`: driftsys/ridl#512
- Coordination: driftsys/ridl#328
- Binds:
  [ADR-0020](../decisions/ADR-0020-third-encoding-runtime-layering-and-plugin-system.md)
  decision 6 (the runtimes live outside `ridl-rt`, and this crate's dependency);
  [ADR-0021](../decisions/ADR-0021-ridl-rt-0.1-api-and-release.md) decisions 5,
  11, 12, 13 and 15 (the duplicate-suppression disposition, the forwarding
  impls, the handle model, the `Wakeable` contract, the call table);
  [ADR-0023](../decisions/ADR-0023-interaction-face-generation.md) decision 5 (a
  face holds its port by value)
- Depends on: `ridl-rt` 0.1 ([the design record](ridl-rt.md), "The ports")
- Issues this crate is evidence for: driftsys/ridl#308 (sequence-number scope),
  driftsys/ridl#309 (an invalid event payload). It changes no specification
  text; driftsys/ridl#544 aligned the ridl reference with the frame
  specification on both
- Open against it: driftsys/ridl#381, which writes the catalog descriptor.
  Giving this crate one, and with it every report in "What it cannot report", is
  not yet assigned to a story
- `crates/ridl-loopback/src/lib.rs`, `src/handle.rs`, `src/store.rs` — the crate
  as built; `crates/ridl-loopback/tests/conformance.rs` — the port contract
  suite of `ridl-rt-conformance` run over it; `tests/ports.rs` — the tests only
  it can express
