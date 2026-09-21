# `ridl-loopback` — the in-process reference runtime

`ridl-loopback` is the first runtime in this workspace. It implements the eleven
port traits of `ridl-rt`'s `port` module over one in-memory store, so a
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
  `ridl-transport-ws` (story E11.9) is the transport, and it does not link this
  crate.
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

## One store, one lock

**Every handle of one runtime holds the same `Arc<Mutex<Store>>`.** The store is
`crates/ridl-loopback/src/store.rs`: the signal map, the per interface
generation counters, the provisioned `fixed` values, one queue per event source,
the call table, and the clock.

Every port method takes the lock, does its work and returns. None waits for data
while holding it, which is what `ridl_rt::port` requires of every port method:
`EventSource::next` returns `Ok(None)` when nothing is queued, `Caller::reply`
returns `Ok(None)` while the outcome is unknown, and `Handler::next_claim`
returns `Ok(None)` when no call is waiting. A critical section that reads or
writes a map and returns is not waiting.

**`RwLock` is rejected for 0.1.** The property ADR-0021 decision 12 protects is
that a signal read does not block on a publication, and the decision's own
rejected alternative — one runtime struct implementing every port, shared behind
a mutex — fails it for a different reason than lock kind: there, every read
waits behind every commit **for the lifetime of the borrow a face holds**, not
for the length of a critical section. Here the handles are separate values, and
a read waits only for the microseconds a commit spends writing its maps. In an
in-process reference that is not the bottleneck, and one lock keeps the
invariants — a commit's generation increment, its one timestamp, and the entries
it writes — in one place rather than spread across a read path and a write path
that must agree.

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

**One handle type per port role, plus an aggregate**, which is ADR-0021 decision
12's shape and this story's `Done when`.

| Handle          | Port roles                                                                                | Threading     |
| --------------- | ----------------------------------------------------------------------------------------- | ------------- |
| `ReaderHandle`  | `Attached`, `Clock`, `SignalReader`, `FixedReader`, `ScannableSignals`, `CoherentSignals` | `Send + Sync` |
| `WriterHandle`  | `SignalWriter`                                                                            | `Send`        |
| `SourceHandle`  | `EventSource`                                                                             | `Send`        |
| `SinkHandle`    | `EventSink`                                                                               | `Send`        |
| `CallerHandle`  | `Caller`                                                                                  | `Send`        |
| `HandlerHandle` | `Handler`                                                                                 | `Send`        |

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
eleven port traits by delegating to the one that has each, and is what
`Loopback::new(catalog)` returns. It exists because a generated `Client` is
commonly bound over `SignalReader + EventSource + Caller` at once and no single
role handle satisfies a multi-trait bound (ADR-0021 decision 12, second
amendment). It reaches a face by value, or as `&mut` under the forwarding impls
of ADR-0021 decision 11.

Three ways to obtain handles, each with one purpose:

- `Loopback::new(catalog)` — the aggregate, with its six handles inside.
- `Loopback::split(self)` — consumes the aggregate and hands out the six handles
  it holds, as a `Handles` struct with one named field per role. A struct rather
  than a six-element tuple, because a tuple of six values of six distinct types
  is read by position.
- `Loopback::reader()`, `writer()`, `source()`, `sink()`, `caller()`,
  `handler()` — an **additional** handle of that role on the same store, each
  with its own per handle state. This is how a test builds two callers on one
  provider, or two event sources of one event.

`advance`, `provision_fixed` and `fail_next_settle` are the aggregate's, not any
role handle's: each acts on the runtime as a whole rather than through a port.

## State on a handle, state in the store

One rule decides where a piece of state lives: state two handles must agree on
is in the store, and state belonging to one handle alone is on that handle.

On a handle: a writer's staged changes and its per channel sequence counters, a
sink's and a caller's sequence counters, a source's identity, and a handler's
served set. In the store: the published signals, the generations, the `fixed`
values, the event queues, the call table, and the clock.

Staging on the writer handle is the visible consequence: `set`, `invalidate` and
`touch` take no lock at all, and `commit` takes it once. A test pins that a
staged value is not visible to a reader until the commit.

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
- **`Watermark::seq`** is set to the highest signal sequence number the scan
  reported for that interface, and left unchanged when the scan reported none.
  It is carried for the caller and is not what `scan` selects on. What that
  field means is an open question of ADR-0021 decision 10 (driftsys/ridl#350);
  this is one runtime's reading of it, offered as evidence, not as an answer.
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
start at the same logical time, which `the_clock_is_hand_driven_not_wall_clock`
pins.

A wall-clock variant is rejected for this story rather than forever: it would
make every envelope timestamp in every test a value the test cannot state, and
nothing needs real time until something measures a real bound. A runtime that
does needs a second clock source, which is a later option and not this story.

## Sequence numbers are per sender handle

**Each sender handle owns its counter** (driftsys/ridl#308). A caller handle
counts its own calls, a sink handle counts its own occurrences, and a writer
handle counts its own publications per channel. The first publication, the first
occurrence and the first call of a handle are `seq` 1, because `seq` 0 is the
envelope of a channel with no publication (ADR-0021 decision 5).

That is what keeps two callers on one provider from colliding, which is #308's
report: with one counter per channel, two callers each sending their first call
would both carry `seq` 1, and a provider deduplicating on the sequence number
alone would drop the second. `two_callers_on_one_provider_do_not_collide` runs
exactly that case and shows two claims, not one.

**The loopback deduplicates nothing.** It presents each call once because it
delivers each call once, not because it recognises a retransmission — nothing
retransmits in a process. A runtime over a real transport keys duplicate
suppression on the caller's transport identity plus `seq`, below the port, which
is ADR-0021 decision 5.

A writer's counters are per channel and on the handle, which is the one place
this reading needs care. ridl §3.1 scopes a signal's sequence number to the
channel, and a signal has one provider; a counter per channel, owned by the
writer handle that publishes it, satisfies both that scope and the rule that the
sender assigns the number. The visible consequence is that a writer handle
dropped and replaced restarts the channel's counter, because the counter went
with the handle. A runtime with a session would carry it; this one has no
session.

The alternative rejected is a counter per channel in the store, shared by every
writer handle. It survives a handle being replaced, but it moves the assignment
off the sender, and on a call — where there is no single sender per channel — it
is exactly what #308 reports as wrong.

## Payload bytes are opaque

**The runtime carries bytes and never verifies a payload** (driftsys/ridl#309).
`Payload::verify` belongs to the generated face, on both sides: the consumer
checks what it reads, and the provider's `dispatch` checks the argument bytes it
is handed.

So what happens to an invalid event payload is decided entirely by the generated
face, not here. Today that is what the round-trip tests pin: argument bytes that
fail the structure check settle `CallError::Transport(Transport::Corrupt)` on
the handler side, and a signal payload that fails it reads back as
`Provenance::Invalid(Cause::Detected(Detection::Corrupt))` on the consumer side.
Nothing in this crate would change if E14.2 chose differently, because this
crate never looks.

The alternative rejected is a runtime that verifies. It cannot: a port carries
interface numbers, ordinals and bytes and names no payload type, so the runtime
has no `Payload` implementation to call. Giving it one would put the generated
types below the port, which is the layering ADR-0020 decision 6 fixes.

## What it cannot report

The loopback holds no catalog descriptor. The descriptor and the real catalog
hash arrive with story E16.2 (driftsys/ridl#378), so there is no member table,
and therefore:

- **no unknown ordinal.** Nothing here can tell an ordinal that names no member
  from one that names a member with no value yet, so no port error's `Contract`
  variant is returned except `FixedReader::read_fixed`'s, where the store knows
  it was given no value to serve.
- **no unowned member.** `WriteError::NotOwner`, `RaiseError::NotOwner` and
  `ServeError::NotOwner` are never returned: a provider's ownership is a fact of
  the descriptor.
- **no freshness and no remaining time.** Every sample reads
  `Freshness::Unbounded` and every claim reads `remaining: None`, because both
  are measured against a member's timing annotation.
- **no time to live.** `EventSource::next` discards nothing: the age at which an
  occurrence is discarded is a member's, too.

Three more, for reasons other than the descriptor: nothing detaches, because
every handle holds the store alive, so `Detached` never appears; nothing is
bounded, so `Busy` and `TooLarge` never appear outside the one injected failure
below; and `Attached::catalog` returns the `CatalogRef` the runtime was built
with, unexamined, because the generated constructor performs no catalog check
either (driftsys/ridl#448, deferred to E16.2 by decision).

Two behaviours are recorded here because they are decisions rather than
absences:

- **`Handler::serve` records its members and `next_claim` presents every waiting
  call regardless.** The generated `dispatch` never calls `serve`
  (`crates/ridl-backend-rust/src/face.rs`), so a handler that presented only
  served members would present nothing to it. `HandlerHandle::served` reads the
  recorded set back, so an application that does call `serve` can see what it
  asked for. Dated 2026-09-21: this changes when a generated `dispatch` serves,
  or when an application drives a claim loop itself.
- **`Loopback::fail_next_settle` is the one fault this runtime injects.** The
  generated `dispatch` counts a claim only once the handler has accepted its
  settlement, and in an in-process runtime nothing else can make that path fail,
  so the count would be untestable without it. It is the one place a
  `SettleError` other than `UnknownClaim` comes from.

## What it replaced

`crates/ridl-backend-rust/tests/support/loopback.rs` was a disposable double
whose own module documentation said to read it out rather than build on it. It
is deleted. In its place:

- `ridl-backend-rust` gains `ridl-loopback` as a dev-dependency, and every
  `round_trip_*` test in `crates/ridl-backend-rust/tests/interaction_face.rs`
  builds its face over the aggregate handle. The tests' shape is unchanged and
  `dispatch` is unchanged; what changed is the `use` line, the constructor —
  `Loopback::new` takes a `CatalogRef` rather than a package name, because a
  runtime is attached to a catalog and not to a string — and the deletion of the
  `support_*` tests.
- Those `support_*` tests moved to `crates/ridl-loopback/tests/ports.rs`, where
  they are tests of the runtime rather than of the Rust backend, alongside the
  tests of what the double did not implement.

`MinimalSignalOnlyPort`, in the same test file, is not replaced. It is not a
double of a runtime: it is a port implementing `SignalReader` and `Attached` and
nothing else, and its purpose is to prove that the emitter writes no bound wider
than the interface needs. A runtime implementing every port could not prove
that.

## Observations for other stories

- **The generated face reports an unpublished signal as corrupt, not as
  `Init`.** `SignalReader::read` answers a channel with no publication with
  `Provenance::Init` and copies nothing, because the port cannot produce the
  init value — the init value has a payload type and a port names none. The
  generated client then runs `Payload::verify` over the zero bytes, which fails,
  and reports `Provenance::Invalid(Cause::Detected(Detection::Corrupt))`. The
  face reaches that state by ignoring the `Init` the port reported.
  `ra19_a_minimal_signal_only_port_constructs_the_signal_only_client` pins the
  current behaviour. This is a face question (E11.13's), recorded here because a
  runtime now makes it reproducible outside a hand-written stub.
- **The settlement ordering is now observable.** The interaction-face record
  lists the command-settled-before, query-settled-after ordering as pinned only
  by an exact-text assertion, because neither settle nor a provider call in the
  double had an observable side effect. With this crate a provider can hold a
  caller handle on the same store and read the acknowledgment from inside its
  own method, which makes the ordering a behavioural assertion. Writing that
  test is not part of this story.

## Trace

- Roadmap: [`docs/ROADMAP.md`](../ROADMAP.md) — E11.15
- Tracking issue: driftsys/ridl#445, split from E11.9 (driftsys/ridl#265)
- Coordination: driftsys/ridl#328
- Binds:
  [ADR-0020](../decisions/ADR-0020-third-encoding-runtime-layering-and-plugin-system.md)
  decision 6 (the runtimes live outside `ridl-rt`, and this crate's dependency);
  [ADR-0021](../decisions/ADR-0021-ridl-rt-0.1-api-and-release.md) decisions 5,
  11 and 12 (the duplicate-suppression disposition, the forwarding impls, the
  handle model);
  [ADR-0023](../decisions/ADR-0023-interaction-face-generation.md) decision 5 (a
  face holds its port by value)
- Depends on: `ridl-rt` 0.1 ([the design record](ridl-rt.md), "The ports")
- Issues this crate is evidence for: driftsys/ridl#308 (sequence-number scope),
  driftsys/ridl#309 (an invalid event payload). It changes no specification
  text; both are the ridl finalization pass's, story E14.2
- Open against it: driftsys/ridl#350's `Watermark::seq` question, on which this
  crate takes a reading; driftsys/ridl#378 (E16.2), which gives it a catalog
  descriptor and with it every report in "What it cannot report"
- `crates/ridl-loopback/src/lib.rs`, `src/handle.rs`, `src/store.rs` — the crate
  as built; `crates/ridl-loopback/tests/ports.rs` — its tests
