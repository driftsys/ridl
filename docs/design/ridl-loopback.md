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
`Done when`. The six here group the eleven roles the way that decision derives
the threading split: a handle each for the five roles with a `&mut self` method,
and one handle for the six whose methods all take `&self`, which are exactly the
roles several threads may hold at once.

Every handle implements `Attached`, which every port trait but `Clock` carries
as a supertrait. The table lists what each handle adds to it.

| Handle          | Port roles beside `Attached`                                                  | Threading     |
| --------------- | ----------------------------------------------------------------------------- | ------------- |
| `ReaderHandle`  | `Clock`, `SignalReader`, `FixedReader`, `ScannableSignals`, `CoherentSignals` | `Send + Sync` |
| `WriterHandle`  | `SignalWriter`                                                                | `Send`        |
| `SourceHandle`  | `EventSource`                                                                 | `Send`        |
| `SinkHandle`    | `EventSink`                                                                   | `Send`        |
| `CallerHandle`  | `Caller`                                                                      | `Send`        |
| `HandlerHandle` | `Handler`                                                                     | `Send`        |

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
  `a_touch_does_not_discard_an_invalidation_staged_before_it` are the two cases.

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
length before running `Payload::verify`, and runs it only when there are bytes
to decode (driftsys/ridl#517). The generated `Publisher` does emit
`invalidate_<name>`, so a provider that invalidates before its first `set`
reaches this path.

## A claim is not a correlation

The call table holds one entry per call sent, from the caller's send to the
provider's settlement. Two identities address it, and they are separate:

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
`a_claim_is_presented_once_and_settled_once` are the three cases. The
alternative rejected is one identity for both ends, which is what the deleted
double had and what this crate had before its review: with it,
`settle(ClaimId(correlation.0))` before any presentation recorded an outcome the
caller could read as an acknowledgment.

`Loopback::fail_next_settle` is not scoped this way: it is the runtime's, so it
fails whichever handler settles next.

**`Caller::forget` releases the caller's interest, and cancels nothing.** A call
whose outcome is already recorded has nothing left to happen to it, so its entry
goes. A call still in flight keeps its entry, marked forgotten: the provider is
still presented it and still settles it, because `Handler`'s contract is that
every claim is settled and a caller losing interest is not the provider's
business. Either way the correlation answers `None` from `ack` and `reply`
afterwards, which is what `Caller::forget` tells a caller to expect. The
alternative rejected is removing the entry outright: it revokes a claim the
provider may already hold, so the provider's `settle` fails and the generated
`dispatch` does not count it — and, in the shape this crate first had, it left
the call's identity in the waiting queue with no entry behind it, which made
every later `next_claim` on that runtime panic.

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
`two_callers_on_one_provider_are_two_claims_under_one_seq` runs exactly that
case and shows two claims under one `seq`, which is the rule ADR-0021 decision 5
fixes: two callers are never merged even under the same `seq`. What tells them
apart here is the claim, not the number. A runtime over a real transport keys
duplicate suppression on the caller's transport identity plus `seq`, below the
port, for the same reason.

**The loopback deduplicates nothing.** It presents each call once because it
delivers each call once, not because it recognises a retransmission — nothing
retransmits in a process.

A sink's counters are per channel for a reason a single counter per handle would
break: a consumer subscribed to some of a sink's events would see the numbers of
the events it did not subscribe to as gaps, and `EventSource::next` states that
a gap in `seq` is a loss.
`a_sink_sequence_number_counts_one_channel_publications` is that case.

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
this crate. Nothing here would change if E14.2 chose differently, because this
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
  variant is returned except `FixedReader::read_fixed`'s. That one is not an
  exception to the rule but a case the rule does not reach: a `fixed` is
  provisioned into the runtime rather than published through a port, so an
  ordinal with no provisioned value is a read the store cannot serve at all,
  where an unpublished signal is one it serves as `Init`.
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
with, unexamined. ADR-0021 decision 3 places the check of it against an
interface's own `CATALOG` in the generated face's constructor, once, when the
face is built; the constructor the Rust backend emits today performs no such
check, which driftsys/ridl#448 is open on. Either way the check is the face's
and not the runtime's.

Three more that are the runtime's own shape rather than the descriptor's:

- **A settled outcome is kept until the caller releases it.** The call table
  holds one entry per call sent, and the only thing that reclaims a settled one
  is `Caller::forget`, which nothing the Rust backend emits calls. A program
  that makes calls over this runtime and never forgets a correlation therefore
  grows its call table with them. A runtime with a session, or one that bounded
  the outcome table, would reclaim; this one holds the outcome because nothing
  else can know the caller has read it.
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
  handler's calls stay waiting for that handler. The deviation is taken with its
  eyes open: the generated `dispatch` never calls `serve`
  (`crates/ridl-backend-rust/src/face.rs`), so a handler that always filtered
  would be presented nothing at all by it, and a handler that never filtered
  would take a second component's calls and settle them `UnknownInteraction` —
  two components providing different interfaces in one process is the plainest
  use of an in-process runtime.
  `two_handlers_each_receive_only_what_they_served` is that case, and
  `a_handler_that_served_nothing_is_presented_every_call` is the other side of
  the rule. The alternative rejected is recording the set without acting on it,
  which loses a call whenever more than one handler exists.
  `HandlerHandle::served` reads the set back.
- **`Loopback::fail_next_settle` is the one fault this runtime injects.** The
  generated `dispatch` counts a claim only once the handler has accepted its
  settlement, and in an in-process runtime nothing else can make that path fail,
  so the count would be untestable without it. It is the one place a
  `SettleError` other than `UnknownClaim` comes from. The claim is looked up
  before the injected failure is consumed, so arming it and then settling a
  claim that does not exist answers `UnknownClaim` and leaves the injection
  armed for the next real settlement.

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
