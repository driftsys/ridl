# The async face and the runtime substrate — design note

**Status: disposed of, 2026-09-26.** Sebastien disposed of the fifteen decisions
on this note's pull request (driftsys/ridl#530), decision by decision, and took
**F-1 to F-15 as written**, together with the one stage change the review
forced: the poll face becomes `pub(crate)` in F5a, not F5b (§6). The
alternatives he declined are listed in the disposition comment. This note takes
the fifteen decisions that [the lane F driver](2026-09-25-lane-f-driver.md) §4
lists, F-1 to F-15, the way D-1 to D-12 of the FlatBuffers codec note were
disposed. Each decision states what is decided, the reason, and the alternative
it rejects with the reason that alternative was rejected, so that a comment can
take, amend or reject one by its number. After the disposition, a second pull
request carries the two ADR amendments and the plan for stages F3 to F5; §5
lists every record the disposition moves, and §6 the stage each one moves in.
Where this note differs from the semantics list of driftsys/ridl#509 — its item
3 (F-11 bounds the blocking client per client, not per call), its item 5 (F-1
adds a third variant) — the note, once disposed, is what that issue's "Done
when" means by "the semantics above".

**Stories:** E11.16 (`Wakeable` and `Transport::Busy`, driftsys/ridl#510),
E11.18 (the correlation table, #512), the second half of E11.20 (the conformance
suite over the new contract, #514) and E11.21 (the two clients and `serve`,
#515); the amendments issue is #509, which closes #485. E11.17 (#511, the `std`
feature) and E11.19 (#513, the helpers) are built, in driftsys/ridl#522 and
#523, and F-8 ratifies where E11.19 put its items.

## 1. Context

**What exists.** The Rust backend emits, per interface, a poll face
([the interaction-face record](../design/interaction-face.md)): `Client<P>` with
one send method per command and query returning that call's own correlation
newtype, a `*_ack` and a `*_reply` poll per call, `next_event`, and a provider
side `dispatch` that makes one pass over the waiting claims and returns a count.
Every method returns without waiting. Under it, `ridl-rt` 0.2 defines the eleven
port traits, every one of whose methods returns at once
([the `ridl-rt` record](../design/ridl-rt.md), "The ports"), the `std` feature
with `task::block_on(fut, deadline)` and `task::noop_waker()` (E11.17), and the
four helpers of E11.19: `Freshness::of`, `EventSeqTracker`,
`Member::call_deadline`, and `Member::reservation` with `table_budget`. The one
runtime is `ridl-loopback`: one store behind one mutex, one handle per port
role, a call table that is a `BTreeMap` with a monotonic counter and no bound, a
hand-driven clock, and no descriptor, so nothing there is timed, bounded or
detached ([the loopback record](../design/ridl-loopback.md), "What it cannot
report").

**What this note designs.** Two clients per interface and a `serve` per
interface, and the substrate they poll: a keyed wake source on a port, a
correlation table every runtime with asynchronous replies needs, and the errors
a call and a `serve` return. The poll face becomes `pub(crate)` in stage F5a, in
the change that emits the async client, and the blocking client follows in F5b.

**What binds it.** `ridl-rt` is `no_std` and allocates nothing with the `std`
feature off, forbids `unsafe`, has `rust-version = "1.83"`, and takes no
dependency in any feature combination
([ADR-0021](../decisions/ADR-0021-ridl-rt-0.1-api-and-release.md) decisions 8
and 10). `Contract` and `CallError` are exhaustive and every other error enum is
`#[non_exhaustive]` (decision 9). A breaking change is a 0.x minor (decision
10). Every port trait forwards through `&mut P` (decision 11); a runtime
presents one handle per port role and `ridl-rt` adds no `Send` or `Sync` bound
(decision 12). A face holds its port by value
([ADR-0023](../decisions/ADR-0023-interaction-face-generation.md) decision 5). A
generated `Client` carries exactly the port bounds its interface needs (RA-19).
Generated code contains no thread, socket or timer (RA-20, restated by F-15).
`Correlation` is `pub struct Correlation(pub u64)`, so
`(generation << 16) | slot` leaves 48 bits of generation.

**The surface at a glance**, for the fixture interface `Cabin` (one signal
`temperature`, one command `setLevel`, one query `average`, events). What every
decision below adds up to; the exact bounds and types are F-10's:

```rust,ignore
// The async client, no_std. P: SignalReader + EventSource + Caller + Clock + Wakeable.
let mut client = cabin::Client::new(port);
let t = client.temperature()?;                       // unchanged: Result<Sample<Temperature>, ReadError>
client.set_level(Level::new(40)?).await?;            // Result<(), ClientError>
let avg = client.average(Window::new(10)?).await?;   // Result<Average, ClientError>
let ev = client.next_event().await?;                 // Result<Event, ReadError>

// A frame loop stores the call and polls it once per frame; the future is a named, Unpin type.
let mut call: cabin::AverageCall<'_, P> = client.average(w);
if let Poll::Ready(out) = Pin::new(&mut call).poll(&mut cx) { /* ... */ }

// The blocking client, behind the emitted crate's `std` feature.
let mut client = cabin::blocking::Client::new(port).with_timeout(Duration::from_secs(1));
let avg = client.average(Window::new(10)?)?;         // Result<Average, ClientError>

// The provider side.
cabin::serve(handler, &mut provider).await;          // Result<Infallible, ProviderError>: resolves only on failure
cabin::blocking::serve(handler, &mut provider, None); // Result<(), ProviderError>: Ok(()) only at a timeout
```

## 2. The decisions

### F-1 — one error type per call, and one per `serve`, both in `ridl_rt::error`

**Decision.** `ridl-rt`'s `error` module gains two enums:

```rust,ignore
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClientError {
    /// The call was not sent.
    Send(SendError),
    /// The call was sent and its outcome is a failure.
    Call(CallError),
    /// The port failed while the outcome was read; only `ReadError::Detached` is reachable.
    Read(ReadError),
}

#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProviderError {
    /// `Handler::serve` refused the interface's members.
    Serve(ServeError),
    /// The handler port failed while a claim was read; the claims already settled stay settled.
    Claim(ReadError),
}
```

with `From<SendError>`, `From<CallError>` and `From<ReadError>` for
`ClientError`, and `From<ServeError>` and `From<ReadError>` for `ProviderError`.
A client call returns `Result<T, ClientError>`; `serve`'s future resolves to
`Result<Infallible, ProviderError>` (F-7).

**Reason.** One type per side, so that `?` works with one type and an
application over two generated packages handles one error. The type is
`ridl-rt`'s and not the face's for the reason the value-objects plan gave when
it made the constraint error `ridl_rt::payload::Violation` rather than a
generated type: a type every generated crate would emit identically belongs in
the library, where a helper crate can name it and where the Kotlin runtime
mirrors it once. `CallError` is exhaustive by ADR-0021 decision 9, so a `Send`
variant in it is a breaking change and, worse, would put a send failure into the
settlement vocabulary a provider uses. The third variant exists because the wait
polls `Caller::reply`, whose outer `ReadError` reports the port itself; only
`Detached` is reachable there, because the reply buffer is sized from
`MAX_SIZE`, and it is kept as what it is rather than mapped onto a `Transport`
variant, because `Transport::Down` and its siblings are the caller's runtime's
findings about the peer (frame specification §9.6) and a local runtime being
gone is not one. `Copy` because every inner type is `Copy`; `#[non_exhaustive]`
by decision 9's rule for every error enum but the two named there.

**Rejected.** A `Send` variant in `CallError`: breaking under decision 9, and it
mixes the two sides' vocabularies. A per-package `ClientError` emitted by the
face: one type per generated crate, each with its own `From` impls, for a
composition that carries no payload-shaped fact. Mapping `ReadError::Detached`
to `Transport::Down`: invents a frame-level meaning for a local failure.

### F-2 — a member whose `max` is `None` waits without a bound

**Decision.** The bound of a call is `Member::call_deadline()`. When it is
`Some(max)`, the future's deadline is `now + max`, with `now` read from the
port's `Clock` when the method is called; the future compares `Clock::now()`
with that deadline on every poll. When it is `None`, the future has no deadline
and resolves only on the outcome, on a send failure other than `Busy`, or on a
port failure. The blocking client's timeout (F-11) is the bound an application
puts on such a call. A runtime that measures the bound itself, as `Caller::ack`
and `Caller::reply` document, reports `Undelivered` or `Timeout` through the
port; the future reports the first of the two findings, and both name the same
category.

**Reason.** ridl §9.1 gives an untimed `command` or `query` no default, and
`call_deadline` takes no position on `None`, so the face has nothing to bound
the call with; refusing the call would break every member without a timing
annotation, and a default would be a language decision the reference does not
take. The future measures the bound because the direction says the bound covers
the whole call from the application's call, including the wait for a slot, and
only the future sees that whole; and because the reference runtime measures
nothing, so without it a call over `ridl-loopback` has no bound at all and
E11.21's "a call waits for a free slot within its bound" cannot be tested.
Reading `Clock::now` on a poll is not a timer: nothing fires.

**Rejected.** Refusing a call with no `max`: breaks every untimed member. A
crate-wide default bound: a language decision in a library. Leaving the bound to
the runtime alone: unbounded on the loopback, and no runtime knows when the
application made the call or how long it waited for a slot.

### F-3 — no slot within `max` is `ClientError::Send(SendError::Busy)`

**Decision.** When the port answers `SendError::Busy`, the future keeps the
argument value, registers `Interest::Slot` (F-6), and tries the send again on
every poll. When the deadline passes with the call still unsent, the future
resolves to `Err(ClientError::Send(SendError::Busy))`. When the deadline passes
after the send, the future calls `Caller::forget` and resolves to
`Err(ClientError::Call(CallError::Transport(Transport::Undelivered)))` for a
command and `Transport::Timeout` for a query, the same two variants the port
itself uses for a bound it measured.

**Reason.** Nothing was sent, and the condition is retryable, which is what
`SendError::Busy` says; `Transport::Timeout` says "sent, no reply" and would be
false. The two sent-and-expired variants are the port's own, so an application
sees one vocabulary whether the runtime or the future measured the bound.

**A limit, stated.** The future has no timer, so nothing wakes it when its
deadline passes: the wake comes from `block_on`'s park timeout when the blocking
client has a timeout set (F-11), from the frame loop's own cadence when the
future is polled with the no-op waker, or from a runtime that measures bounds
and wakes `Interest::Outcome` when it settles the call as expired. A general
executor, or a blocking client with no timeout, driving a call over a runtime
that measures nothing — `ridl-loopback`, whose clock moves only under `advance`
— polls the future again only when a key wakes it. That is the loopback's limit,
not a defect of the face; stage F3 records it in the loopback's design record,
and whether the loopback ever measures a bound is not decided here (§3).

**Rejected.** No slot wait, `Busy` returned at once: leaves backpressure to
every application, which then writes the retry loop the client exists to remove;
the direction validated the wait. `Transport::Timeout` for the unsent case: says
"sent" when nothing was.

### F-4 — a call is sent when the method is called; a dropped future forgets

**Decision.** A send method is a plain function taking `&mut self` and its
argument by value. Its body evaluates `require`, encodes the argument into a
stack buffer, calls `Caller::command` or `Caller::query` once, and returns the
future with the result of that attempt inside it. A `require` failure, or a send
error other than `Busy`, is a future that is ready with `Err`; `Busy` is a
future in the slot-waiting phase (F-3). Dropping the future in the waiting phase
calls `Caller::forget` on its correlation; a command already sent is not taken
back, and a query already sent is still served and settled by the provider.
(Corrected 2026-09-26: that is what the face promises over a transport; a
runtime may withdraw a command no provider has taken, and the loopback does, per
the #553 and #557 decisions.) A future dropped in the slot-waiting phase sends
nothing. A future that has taken its outcome calls `Caller::forget` at once, so
`forget` is the one operation that reclaims a slot (F-9), and the port's `ack`
and `reply` stay non-consuming, as they are today.

**What the future holds.** A `&'a mut P` to the port, the argument value (kept
only until the send succeeds, for the retry), the phase — unsent with its
argument, waiting with its correlation, done — and the deadline as
`Option<Timestamp>`. It holds no buffer: the reply is read into a stack buffer
inside `poll` and decoded there. The future is `Unpin`, because every field is,
so a frame loop stores it and polls it with `Pin::new`. It is `Send` when `P`
and the argument type are `Send`; it needs no `Sync`, because it holds the port
by `&mut` and every `Caller` method takes `&mut self`. One call is in flight per
`Client` at a time, for the length of that borrow; two concurrent calls are two
`Client`s over two caller handles, which is ADR-0021 decision 12's model — the
loopback's `Loopback::caller()` hands out the second.

**Reason.** An `async fn` body runs nothing until polled, so a call that is
"sent when called" cannot be one. The argument is kept as a value and not as
encoded bytes because the value is smaller than its `MAX_SIZE` encoding and
`require` is evaluated over it once, at the call. The port is held by `&mut`
because that is the only receiver the `Caller` trait offers; a `&P` future would
need interior mutability the traits do not have and a `Sync` bound the crate
does not add.

**Rejected.** `async fn`: sends on first poll, is opaque, cannot be stored in a
frame loop's state without allocation, and makes the future's size a compiler
artifact, which is ADR-0018's reason for rejecting it at the engine's platform
layer. Taking the port by value into the future: moves the port out of the
client for the length of the call. Cancelling a sent command on drop: the port
has no such operation, and ridl §6.1 says a delivered command is the provider's.

### F-5 — one waiter per kind of key per port handle; a displaced waiter is woken

> **Amended on driftsys/ridl#546 (Sebastien, 2026-09-26).** "One waker per key"
> became one waker per kind of key, woken by a change to any key of that kind,
> and a registration of the same task's waker became a refresh. The text below
> is the amended decision; ADR-0021 decision 13 carries the same wording.

**Decision.** `Wakeable::wake_on(&self, what: Interest, waker: &Waker)` stores a
clone of `waker` on the handle it is called on, under the kind of `what`. A
handle holds one waker per kind of key — `Slot`, `Event`, `Claim` — and a change
to any key of that kind the handle observes wakes it, so a task that registers
`Event(a)` and then `Event(b)` is woken by an occurrence of either; a spurious
wake is allowed, and the future re-checks. A `wake_on` whose waker `will_wake`
the stored one is a refresh: it replaces the stored waker without waking it,
because a task registers on every poll and waking it for its own registration
would schedule the next poll from every poll. A waker of another task displaces
the stored one and wakes it, so no task waits on a registration that can no
longer fire. A stored waker is woken at most once and cleared when woken; a
caller registers on every poll, and registers before it reads the port, so a
wake caused by a change between the read and the return is delivered. "Exactly
one waiter is woken per key" means this: the one of that kind registered on that
handle. A second task waiting for the same interface's events holds a second
handle — one thread drives a handle with a `&mut self` method (decision 12), and
one task per handle follows from it — and each handle's waiter is woken. The
registry is bounded: one slot per kind of key the handle's roles admit, and
nothing is allocated. `Interest::Outcome` wakers are stored in the table's slot
(F-9); the handle's registry holds `Slot`, `Event` and `Claim`. When a slot is
reclaimed, every registered `Slot` waiter is woken: the first to re-poll takes
the slot and the others re-register.

**Reason.** This is the `AtomicWaker` model, which every executor already
assumes: one waker per waited thing, replaced on re-registration. Waking the
displaced waker costs one spurious poll and never strands a task; dropping it
silently strands one. Register-then-read is the order that makes a wake between
a read and a registration impossible to lose, and it needs no ordering rule from
the runtime beyond "wake after the change is visible".

**Rejected.** One waker per key, as this note first said: a task that registers
`Event(a)` and then `Event(b)` on one handle loses the first registration, and
an occurrence of `a` then wakes nothing. A list of waiters per key: needs
storage the crate does not allocate, and a use — several tasks on one handle —
decision 12 already rules out. Dropping the displaced waker without waking it:
strands the task that registered it. Read-then-register: the wake between the
two is lost unless the runtime keeps a wake token, which every runtime would
then have to implement. FIFO slot waiters woken one at a time, as the driver's
F3 text and the E11.18 roadmap row propose: a queue of waiters needs storage the
allocation-free table cannot hold, a waiter without a slot has no slot to keep
it in, and waking every waiter costs one extra poll per contender.

### F-6 — the key set is `Interest::{Outcome, Slot, Event, Claim}`, keyed per interface

**Decision.**

```rust,ignore
/// Extension: a port that can wake a task. A runtime that serves a generated
/// async client implements it.
pub trait Wakeable {
    fn wake_on(&self, what: Interest, waker: &core::task::Waker);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Interest {
    /// The outcome of one call is known.
    Outcome(Correlation),
    /// A slot for a new call is free.
    Slot,
    /// An occurrence of one of the interface's events is waiting.
    Event(InterfaceNo),
    /// A claim on one of the interface's members is waiting.
    Claim(InterfaceNo),
}
```

Both in `port`, beside `ScannableSignals`. `Wakeable` has no supertrait, like
`Clock`, and is forwarded through `&P` and `&mut P` under decision 11. `Event`
and `Claim` are keyed per interface, not per member. `Interest` is exhaustive.
The name is `Interest`, not the driver's `Wake`, because `task.rs` imports
`std::task::Wake` and any module of the crate that imports both would collide,
and because ADR-0021's open question 6 asked for "a way to register interest".

**A runtime that cannot key.** A runtime with one "something changed" source may
wake every waiter it holds on any change: a spurious wake costs one poll, a
missed wake hangs a task, so the contract is only that a registered waiter is
woken after every change of its key becomes visible, and never that it is woken
only then. A runtime with no wake source of its own has none to offer and does
not implement the trait; every runtime this repository builds has one (the
loopback's settle, raise and send are the sources; a transport's readable socket
is).

**Reason.** `Outcome` is one per correlation because one future waits on one
call. `Slot` is unkeyed because a free slot serves any call. `Event` and `Claim`
follow the port methods they wake: `EventSource::next` and `Handler::next_claim`
drain one queue whatever the ordinal, and the subscription and the served set
already filter by member, so a per-member key would multiply the registry by the
member count for no observable difference. Exhaustive, because a runtime must
handle every key and an unknown key has no safe default: ignoring it hangs the
waiter, and waking it at once makes a busy loop; adding a key is a 0.x minor
under decision 10.

**Binds E11.9.** The WebSocket runtime maps each frame it receives to a key — a
`response` to `Outcome`, an `event` to `Event`, a `call` to `Claim` — or wakes
every waiter; either satisfies the contract.

**Rejected.** Per-member keys: the reason above. A `Signal(InterfaceNo)` key so
a task can await a signal change: no face method waits on a signal, and
`ScannableSignals` already reports change; nothing needs it, so it is not added.
`#[non_exhaustive]`: a runtime's `match` would need a wildcard arm with no
correct body.

### F-7 — `serve` registers its members, runs until the handler port fails, and returns that failure

**Decision.** `serve(h, &mut provider)` returns a named future `Serve<'a, H, P>`
over `H: Handler + Wakeable` and `P: Provider`. When the function is called it
calls `Handler::serve(iface, ORDINALS)` with the interface's command and query
ordinals; a refusal is a future ready with `Err(ProviderError::Serve(e))`. Each
poll registers `Interest::Claim(iface)`, then drains `Handler::next_claim`,
routing and settling every claim it takes exactly as `dispatch` does today (the
settlement table of the interaction-face record is unchanged); `Ok(None)` is
`Pending`; `Err(e)` resolves the future to `Err(ProviderError::Claim(e))`, with
every claim settled before the failure staying settled. The output type is
`Result<core::convert::Infallible, ProviderError>`: the future never resolves to
`Ok`. The future holds the handle by value, `&'a mut P`, and the claim buffer of
`MAX_BUFFER_SIZE` bytes inline. `dispatch` becomes the `pub(crate)` one-pass
step `serve` calls, and returns the `ReadError` instead of swallowing it; both
in stage F5a.

**The two dispositions of driftsys/ridl#485.** Item 1, the three-deep reply
shape, is closed by the poll face becoming `pub(crate)` in F5a: no public method
returns it, and F5 may reshape the internal methods freely. Item 2, `dispatch`
hiding a handler failure, is this decision: the failure is the value `serve`
resolves to.

**Reason.** A detached runtime looks idle to today's `dispatch`, so an async
`serve` over it would wait forever on a claim that cannot come. Registering the
served set is what `Handler::serve` is for, and the generated face is the one
caller that knows the set; today nothing calls it, which is why the loopback
deviates from the trait's contract for an empty set (its record, "An empty
served set is no filter"). `Infallible` states in the type that `serve` is a
loop, not a pass.

**Rejected.** Resolving to `Ok(())` when the runtime detaches: hides the failure
again. Returning the count of settled claims: the count is the internal step's
and has no consumer once the loop is the face's. A `ReadError` output with
`Handler::serve`'s refusal mapped into it: `ServeError::NotOwner` has no home
there.

### F-8 — where every item lives, and the sentence that reconciles it with ADR-0020 decision 6

**Decision.** E11.19's placement is ratified as built: `Freshness::of`,
`EventSeqTracker`, `Continuity` and `TrackerFull` in `sample`;
`Member::call_deadline`, `Member::reservation`, `table_budget` and `Unsized` in
`contract`; `Encoding::max_size` on the sealed trait. A helper lives beside the
type it reads. The new items: `Wakeable` and `Interest` in `port`;
`Transport::Busy`, `ClientError` and `ProviderError` in `error`; and one new
unconditional module, `correlate`, holding `Table<const N: usize>` (E11.18) and
`Waiters`, the keyed registry a handle keeps behind `Wakeable`. `task` stays the
one module behind `std`. The crate then has seven unconditional modules and two
behind features, and ADR-0020 decision 5's module list gains a dated note.

The `Box<P>` forwarding impls of ADR-0021 open question 5 are not added under
`std`: nothing needs a boxed port, and the question stays open with its reason
corrected — `alloc` is now reachable under `std`, and the impls wait for
something that needs them.

**The sentence for the ADR-0021 amendment.** "`ridl-rt` carries, beside the
vocabulary generated code and a runtime agree on, the runtime-side helpers every
runtime would otherwise write alone: pure data structures and pure functions,
`no_std`, allocation-free, behind no feature, holding no lock, no thread, no
clock and no I/O. A runtime remains its own crate: it owns the storage, the
lock, the clock and the transport, and drives the helpers from them. The
dependency graph of ADR-0020 decision 6 — emitter output → `ridl-rt` ← runtime —
is unchanged."

**Reason.** Beside-the-type placement is what E11.19 chose and what a reader of
`Member` or `Freshness` finds first; moving it would be a breaking change once
the 0.x minor is tagged, for no gain. `correlate` is a module of its own because
`Table` and `Waiters` read no existing type's documentation and are the first
items in the crate that hold a `Waker`. `Busy` joins `Transport` because it
crosses the frame (F-13) and `Transport` is `#[non_exhaustive]`.

**Rejected.** A separate helpers crate: ADR-0020 decision 5 rejected splitting
`ridl-rt` for the ports on the ground that the parts always ship together at
matching versions, and the same holds here. One `runtime` module gathering every
helper: hides `call_deadline` from the reader of `Member`, and E11.19 already
shipped the other placement.

### F-9 — `ridl-loopback` adopts `correlate::Table`, in stage F3

**Decision.** Yes. The loopback's caller-side call table — the `BTreeMap` with
the monotonic counter — is replaced by `correlate::Table<{ Loopback::SLOTS }>`,
with `SLOTS` a documented constant of 16 and no byte budget, because the
loopback holds no descriptor until E16.2. The claim side stays the loopback's:
`ClaimId`, the presented-once rule, the handler map, the served sets and
`fail_next_settle`. Reply bytes are stored by the loopback in its own map,
indexed by slot; a `no_std` runtime stores them in an arena of `table_budget`
bytes instead, which is what E11.19's reservation is for.

**What the table is.** `N` slots, each with a generation counter, the outcome's
status and error, and one waker; the correlation is `(generation << 16) | slot`,
so `N ≤ 65536`; an optional byte budget debited at insert from the member's
reservation and credited when the slot is reclaimed; `&mut self` throughout, so
each runtime puts it behind its own lock. A slot is reclaimed by `forget` alone:
at once for a settled call, and at the settlement for a call still in flight,
whose slot `forget` marks. The future forgets after taking its outcome and on
drop while waiting (F-4); `ack` and `reply` do not consume, as today. `settle`
and the reclaiming operations return the wakers to wake, and the runtime wakes
them after releasing its lock, so no waker runs under a runtime's mutex. The
plan fixes the method signatures.

**Amended (2026-09-26).** In the loopback, a forgotten call that no handler has
claimed is withdrawn: it leaves the waiting calls, and its slot is reclaimed at
the `forget`, so a withdrawn command is never delivered. Under the store's lock
the loopback settles the call and then forgets it through the existing `Table`
API, which reaches `Forgotten::Reclaimed`; `ridl-rt` gains no item. A claimed
call keeps its slot until its settlement, as above. The withdrawal is the
loopback's behaviour, not a port contract. Decision 1 of the
[pass-1 dispositions on driftsys/ridl#553](https://github.com/driftsys/ridl/pull/553#issuecomment-5848559640),
with its
[confirmed details](https://github.com/driftsys/ridl/pull/553#issuecomment-5848618786).

**What the loopback's two hooks need from it.** Nothing. `fail_next_settle`
fails `Handler::settle` before the table is touched and records no outcome, so
the claim stays settleable, as today. The hand-driven clock stamps envelopes and
is read by the future for its deadline (F-2); the table holds no time.

**What changes in the loopback's record.** "Nothing is bounded, so `Busy` and
`TooLarge` never appear outside the one injected failure" becomes "the caller
side holds 16 slots, and a send with all of them in flight is `Busy`"; "a
settled outcome is kept until the caller releases it" stays true, and the two
sentences after it — that nothing the Rust backend emits calls `forget`, and
that a program's call table grows with its unforgotten calls — are retired by
F5a and by the 16-slot bound; and the `CallerHandle` gains `Clock` and
`Wakeable`, because a `Client` over an interface with a call is bound on both
(F-10) and a role handle that cannot build one is not a handle for that role.

**Reason.** The table is untested by a runtime until one uses it, and the
reference runtime is the one whose tests are the contract. E11.21's "a call
waits for a free slot within its bound" needs a bounded runtime, and the
loopback is the only one.

**Rejected.** Leaving the loopback on its own table: the table ships untested
and the slot-wait test has no runtime to run on. A byte budget on the loopback
now: it has no descriptor to derive one from, and guessing a budget is what
`Unsized` exists to forbid.

### F-10 — the concrete signatures, and the build matrix

**Decision.** For interface `Cabin`, the emitted module holds:

```rust,ignore
pub struct Client<P: SignalReader + EventSource + Caller + Clock + Wakeable> { port: P }

impl<P: ...> Client<P> {
    pub fn new(port: P) -> Self;                                         // unchanged
    pub fn temperature(&self) -> Result<Sample<Temperature>, ReadError>;  // unchanged
    pub fn subscribe_warning(&mut self) -> Result<(), SubscribeError>;       // unchanged
    pub fn next_event(&mut self) -> NextEvent<'_, P>;                      // Output = Result<Event, ReadError>
    pub fn set_level(&mut self, level: Level) -> SetLevelCall<'_, P>;     // Output = Result<(), ClientError>
    pub fn average(&mut self, window: Window) -> AverageCall<'_, P>;      // Output = Result<Average, ClientError>
}

pub struct SetLevelCall<'a, P: Caller + Clock + Wakeable> { /* F-4 */ }
impl<'a, P: Caller + Clock + Wakeable> Future for SetLevelCall<'a, P> { type Output = Result<(), ClientError>; ... }
impl<'a, P: Caller + Clock + Wakeable> Drop for SetLevelCall<'a, P> { ... } // forget, F-4; the bounds equal the struct's
// AverageCall<'a, P> and NextEvent<'a, P> in the same shape.

pub fn serve<H: Handler + Wakeable, P: Provider>(h: H, p: &mut P) -> Serve<'_, H, P>;
impl<'a, H, P> Future for Serve<'a, H, P> { type Output = Result<Infallible, ProviderError>; ... }

#[cfg(feature = "std")]
pub mod blocking {
    pub struct Client<P: ...> { inner: super::Client<P>, timeout: Option<std::time::Duration> }
    impl<P: ...> Client<P> {
        pub fn new(port: P) -> Self;
        pub fn with_timeout(self, timeout: std::time::Duration) -> Self;
        pub fn set_timeout(&mut self, timeout: Option<std::time::Duration>);
        pub fn temperature(&self) -> Result<Sample<Temperature>, ReadError>;
        pub fn subscribe_warning(&mut self) -> Result<(), SubscribeError>;
        pub fn next_event(&mut self) -> Result<Option<Event>, ReadError>;   // None at the timeout
        pub fn set_level(&mut self, level: Level) -> Result<(), ClientError>;
        pub fn average(&mut self, window: Window) -> Result<Average, ClientError>;
    }
    pub fn serve<H, P>(h: H, p: &mut P, timeout: Option<std::time::Duration>) -> Result<(), ProviderError>;
}
```

The bounds follow RA-19: `Clock` and `Wakeable` are added only when the
interface declares a command or a query (`Clock` for the deadline, `Wakeable`
for the wait), and `Wakeable` alone when it declares an event; a signal-only
interface's `Client` is unchanged. `Publisher` is unchanged. The future types
are named, one per call and one for `next_event`, `Unpin`, with the fields F-4
gives; every future's `poll` registers its interest, then reads the port, then
returns. `NextEvent` has no deadline: an event has no response bound, and its
`max` is the time to live the runtime applies inside `EventSource::next`. The
`blocking` module is gated by the emitted crate's own `std` feature, which
enables `ridl-rt/std`. The poll face becomes `pub(crate)` in F5a, in the same
change, because an async `set_level` and the poll face's `set_level` cannot both
be methods of one `Client`: the internal send, acknowledgment, reply and event
methods take names of their own — `send_set_level`, `poll_set_level_ack`,
`send_average`, `poll_average_reply`, `poll_next_event` — and `dispatch` becomes
the internal one-pass step. F5b adds the `blocking` module.

**The build matrix.** Everything the async client uses — `Future`, `Poll`,
`Context`, `Waker`, `Pin`, `Infallible` — is in `core` and stable before 1.83,
and no `impl Trait` in return position and no `async` block is emitted, so the
emitted source compiles as edition 2021 at 1.83 when copied into a crate of that
edition, which is the first of the three cells ADR-0021 decision 10 sets for the
codegen — edition 2021 at 1.83, edition 2021 at the pin, edition 2024 at the pin
— and the other two follow from it. The manifest `ridl build` emits
(`crates/ridlc/src/lib.rs`, the `Cargo.toml` template) is edition 2024 and
already declares a `std` feature, on by default and empty; F5b makes that
feature forward to `ridl-rt/std` and gates the `blocking` module on it, so a
build with default features off has no `blocking` module and builds for
`wasm32-unknown-unknown`. F5a adds the emitted crate to `just compat-check` if
nothing compiles its source as edition 2021 at 1.83 yet.

**Reason.** A named future type is what a `no_std` frame loop needs: a value it
can store in its own state struct between frames, which `impl Future` cannot be
without allocation. The blocking client needs it too (F-11): after `block_on`
gives up it asks the future whether the call was sent. `Pin::new` on an `Unpin`
future is what makes by-hand polling one line. A sketch with these bounds, the
`Drop` impl, the by-hand poll and an `Unpin` assertion compiled at Rust 1.83 as
edition 2021, and for `wasm32-unknown-unknown` at the pinned toolchain, while
this note was written (the sketch is not kept in the tree); the struct declares
the full bound set and every impl repeats it, because a `Drop` impl's bounds
must equal the struct's (rustc E0367).

**Rejected.** `impl Future` in return position: cannot be named, so cannot be
stored, and cannot be asked anything. One generic future type in `ridl-rt`,
`Call<'a, P, C>` over the descriptor traits: it would move payload decoding and
the `require` evaluation into the library, and generated code is the one place
that knows the payload types; the state machine is small and is pinned by
`face_generation.rs` where it is emitted.

### F-11 — the blocking client's timeout is per client, and a shorter one is accepted

**Decision.** `blocking::Client` holds `timeout: Option<std::time::Duration>`,
`None` by default, set by `with_timeout` or `set_timeout`. Every call runs
`block_on(call, timeout.map(|t| Instant::now() + t))` over the async call's
future, pinned locally. The member's `max` still bounds the call inside the
future on the runtime's clock (F-2); the two bounds are on two clocks and each
holds, so the earlier one ends the call, and a caller's timeout shorter than
`max` is accepted. When `block_on` returns `None`, the client asks the future
whether the call was sent: unsent is `Err(ClientError::Send(SendError::Busy))`,
sent is `Err(ClientError::Call(CallError::Transport(Transport::Undelivered)))`
for a command and `Transport::Timeout` for a query, the same answers the future
gives at its own deadline (F-3); dropping the future then forgets the call.
`blocking::next_event` returns `Ok(None)` at the timeout, and
`blocking::serve(h, p, timeout)` returns `Ok(())` at it, so a loop that also
does other work can call either repeatedly; with `None` neither returns except
on failure.

**Reason.** The blocking client is `block_on` over the async call, so the two
cannot diverge; a timeout parameter on every call would make the two clients'
signatures differ in more than the future, and an absolute `Instant` is computed
from a duration by every caller anyway. A caller that wants one call bounded
differently sets the timeout before it. `None` by default because, on a runtime
whose clock advances, a member with a `max` is bounded by the future, and the
direction says an untimed member waits. Over `ridl-loopback`, whose clock moves
only under `advance`, a blocking call whose provider never serves returns only
at a timeout the caller set, so a test over the loopback sets one.

**Rejected.** A `deadline: Option<Instant>` parameter on each call: the reason
above. Refusing a caller's timeout shorter than `max`: the runtime's bound is a
ceiling the reference sets on the provider, not a floor on the caller's
patience. A default timeout: a bound the reference does not state.

### F-12 — decision 4 is superseded for the public surface and kept for the internal methods

**Decision.** The ADR-0023 amendment supersedes decision 4 and its 2026-09-20
amendment for the public surface: a public `Client` call is an async call
returning `Result<T, ClientError>`. It keeps them for the internal methods: the
send methods still return `Result<<Name>Correlation, SendError>` and become
`pub(crate)` in F5a, the correlation newtypes stay the face's internal
vocabulary and become `pub(crate)` with them, and `SendError` remains the send
methods' error. `ClientError::Send` carries it unchanged, so a client-side
`require` failure is
`ClientError::Send(SendError::Contract(Contract::PreconditionFailed))`, which is
decision 4's reasoning — no invented conversion between the two sides'
vocabularies — carried one level up.

**Reason.** The reasoning of decision 4 is about which error a send reports and
is still right; what changes is who calls the send. The newtypes still keep a
query's correlation out of an acknowledgment inside the face, which is the
amendment's whole argument, and cost nothing when private.

**Rejected.** Reversing decision 4 outright: loses the reasoning that keeps the
mapping lossless. Deleting the newtypes: the internal `*_ack` and `*_reply`
would again accept the wrong kind of correlation.

### F-13 — `Busy` crosses the frame, as a `response` outcome

**Decision.** A providing runtime that cannot admit a call — no slot, no budget,
or a call faster than the member's `min` — answers with a `response` whose
outcome is `busy`, and the caller's runtime presents it as `Transport::Busy`
from `Caller::ack` or `Caller::reply`. The frame specification changes wherever
it names the outcome values or counts the crossing variants: `busy` joins
`corrupt` as a bare value of the `outcome` field in §5.3's and §5.4's rows and
in §5.6's field table; §5.3's "What the caller does with the response" maps
`busy` to `Err(CallError::Transport(Busy))`; §9.6's "exactly one crosses"
becomes "exactly two cross: `Corrupt`, because the provider detected it, and
`Busy`, because the provider refused"; and §8's cell "the providing runtime may
refuse a faster call at admission; how it answers one is not fixed here" becomes
"... and answers with `busy`"; and every sentence of §5.3, §5.4 and §9.6 that
counts the outcome values or the `Transport` variants — §9.6 opens with "Of the
four `Transport` variants" — counts one more. All of it lands in stage F3, in
E11.16's pull request, with the variant. `Busy` is `Copy` and carries nothing.

**Reason.** A provider that knows at admission that it will not serve a call
should say so; silence makes the caller wait the whole bound for a refusal that
was known at once, and the caller's remedy — retry later — is the same for every
cause, so one variant is enough. `Transport` is `#[non_exhaustive]`, so the
variant is additive.

**Binds E11.9.** The WebSocket binding carries the outcome; a `response` already
carries the bare outcome value `corrupt`, so `busy` is one more value of the
same field and no new frame is needed.

**Rejected.** A `Contract` category: it is not a contract error, and `Contract`
is exhaustive, so it would be a language change. A `retry_after` field: the
reference states no such value, and a duration in a frame outcome would be a
specification the runtime cannot honor.

### F-14 — what proves it

**Decision.** The conformance suite — `ridl-rt-conformance`, which stage F4 is
building beside this note over the contract as it stands — gains, after F3,
these contract tests, each generic over the runtime factory:

- `Interest::Outcome`: a waker registered before the settlement is woken exactly
  once by it; a waker registered after the settlement is woken at once or finds
  the outcome on the read that follows registration.
- `Interest::Slot`: with every slot taken, a waker registered under `Slot` is
  woken when one is reclaimed, and the send that follows succeeds.
- `Interest::Event`: a raise wakes the waiter of a subscribed handle and not of
  an unsubscribed one.
- `Interest::Claim`: a send wakes the waiter of the handler that serves the
  member.
- A second registration under a key of the same kind, by another task, wakes the
  displaced waker, and the new one is the one woken by the change; a
  registration of the same task's waker wakes nothing, and a change to any key
  of the kind wakes the stored waker.
- Register, then read: a change made after the registration and before the read
  is seen by the read and also wakes the waker.
- Through the ports, over the slot count the factory states: a send with every
  slot in flight is refused with `SendError::Busy`, and a correlation whose slot
  was reclaimed answers `None` to the old generation.

**The mutation that must turn the loopback red.** Removing the wake from the
loopback's `settle` path: the `Outcome` test then fails on its "woken exactly
once" assertion. The F3 pull request names it and shows the failure.

**The face tests**, in `crates/ridl-backend-rust/tests/interaction_face.rs` over
a recording port double and over `ridl-loopback`:

- a future dropped while waiting calls `forget` once with its correlation; one
  that took its outcome called `forget` once at that moment and calls nothing on
  drop;
- a call over a runtime with every slot taken is `Pending`, is woken by a
  reclaimed slot, and resolves; with the clock advanced past `max` first, it
  resolves to `Send(Busy)`;
- a sent call whose clock passes `max` resolves to `Undelivered` or `Timeout`
  and forgets;
- the blocking client over an untimed member returns `Busy`, `Undelivered` or
  `Timeout` at its own timeout, by phase and by kind;
- `serve` over a handler whose `next_claim` fails resolves to
  `ProviderError::Claim`, with the claims settled before the failure counted by
  the provider double;
- the cabin round trip passes through both clients on `ridl-loopback`.

**Reason.** Each test names one sentence of F-3 to F-7 and fails when that
sentence is false; the mutation is the one that leaves every other test green.

### F-15 — RA-20 restated, ADR-0018 untouched, and its open question 5 answered

**Decision.** RA-20 is restated as: "generated code contains no thread, socket
or timer, and no port waits; a face may return a future, and that future never
blocks." ADR-0018's two rejections — `async fn` at the platform layer, blocking
calls at the platform layer — are about `ridl-engine`'s layers 1 and 2, as that
record's 2026-09-12 correction says, and bind neither the ports library nor
generated code, so they stand untouched. RA-19's provider clause, "a `dispatch`
over `Handler`", becomes "a `serve` over `Handler`"; its client clause is
unchanged, and RA-14's sentence that generated code never contains a blocking or
async face is superseded with RA-20's. ADR-0018's open question 5 is answered
for the face: a `command`'s future resolves on the delivery acknowledgment,
which is the runtime's finding and not the application's, and its output
`Result<(), ClientError>` carries exactly the runtime delivery result the
question asked for, never an acceptance value — which is the second of the two
readings the question left open, taken.

**Where each of the four places is amended, and when.**

| Place                                                            | Change                                                  | Stage                           |
| ---------------------------------------------------------------- | ------------------------------------------------------- | ------------------------------- |
| `docs/wip/2026-09-08-ridl-rt-design.md`, RA-20                   | a dated note under §8 with the restated rule            | F2, the amendments pull request |
| ADR-0018, open question 5                                        | a dated note closing it for the face                    | F2, the amendments pull request |
| `crates/ridl-backend-rust/src/face.rs`, the module documentation | the restated rule, when the face first returns a future | F5a                             |
| `docs/design/interaction-face.md`, "Nothing here waits (RA-20)"  | rewritten with the two clients                          | F5b                             |

**Reason.** The rule's purpose — a runtime, not generated code, owns every
mechanism that waits — is kept whole: the future polls ports that return at
once, and the only things that wait are `block_on`, which is `ridl-rt`'s, and
the application's executor. The letter changes because a future is now returned.

**Rejected.** Leaving RA-20 as written and calling the future an exception: a
rule with an unwritten exception is not a rule. Amending ADR-0018's alternatives
table: it would suggest those rejections bound the ports, which its own
correction denies.

## 3. What this note does not decide

- **The engine.** Outside this repository (the 2026-09-12 re-scope §3.7).
- **The transport.** E11.9 is a separate story; F-6 and F-13 bind it and say why
  they generalize.
- **The Kotlin runtime's internals.** `ridl-rt-kt` mirrors the shapes here and
  records each in its correspondence test; what each Rust stage owes it is the
  driver's §6.
- **The catalog check.** E16.2 (driftsys/ridl#378) owns it; `Client::new` is
  unchanged.
- **A timed settlement in `ridl-loopback`.** It needs the descriptor E16.2
  brings; until then a call the provider never serves is bounded only by the
  future's deadline (F-3) or the blocking client's timeout (F-11).
- **The plugin protocol.** Lane P landed it; this lane changes what the face
  emitter writes, not how it is invoked.

## 4. Open items

1. **`Loopback::SLOTS = 16`** (F-9) is a number chosen so that a test fills the
   table in a loop; a use that needs more is a constructor argument, and nothing
   asks for one yet.
2. **Whether `Interest::Slot` should carry the reservation** the waiting call
   needs, so a runtime with a byte budget wakes only waiters that would fit. Not
   taken: a spurious wake costs one poll, and the loopback has no budget to
   observe the difference with.

## 5. Records this changes, if the disposition takes it

None of these moves in this note's own pull request.

| Record                                                                    | What changes                                                                                                                                                                                                                                                      | Decision                    |
| ------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------- |
| ADR-0023, a dated amendment                                               | decision 4 superseded for the public surface and kept inside; the semantics of F-2, F-3, F-4, F-7, F-11; decision 5 unchanged                                                                                                                                     | F-1 to F-4, F-7, F-11, F-12 |
| ADR-0021, a dated amendment                                               | `Wakeable`, `Interest`, `Transport::Busy`, `correlate`, `ClientError`, `ProviderError`, the `std` feature folded in, the helpers, the reconciling sentence, one 0.x minor; open question 6 closed, open question 5 reworded; the items noted on driftsys/ridl#509 | F-1, F-5, F-6, F-8, F-13    |
| ADR-0020 decision 5                                                       | a dated note: `correlate` is the seventh unconditional module                                                                                                                                                                                                     | F-8                         |
| ADR-0018 open question 5                                                  | a dated note closing it for the face                                                                                                                                                                                                                              | F-15                        |
| `docs/wip/2026-09-08-ridl-rt-design.md` §8                                | a dated note restating RA-20 and RA-19's provider clause                                                                                                                                                                                                          | F-15                        |
| `docs/specification/frame-specification.md` §5.3, §5.4, §5.6, §8 and §9.6 | `busy` crosses                                                                                                                                                                                                                                                    | F-13                        |
| `docs/design/ridl-rt.md`                                                  | the module table, the ports, the errors, the helpers section, `Box<P>`                                                                                                                                                                                            | F-8                         |
| `docs/design/ridl-loopback.md`                                            | the table, the bound, the reclaim rule, the call-table growth sentence, the wake limit of F-3, `CallerHandle`'s two new roles, the served-set deviation                                                                                                           | F-7, F-9                    |
| `docs/design/interaction-face.md`                                         | rewritten from this note in F5b                                                                                                                                                                                                                                   | all                         |
| `crates/ridl-backend-rust/src/face.rs`, module documentation              | RA-20 restated                                                                                                                                                                                                                                                    | F-15                        |
| `docs/ROADMAP.md`, E11.16 and E11.18 rows                                 | `Wake` is `Interest`; "FIFO slot waiters" becomes "every `Slot` waiter is woken"                                                                                                                                                                                  | F-5, F-6                    |
| `docs/wip/2026-09-25-lane-f-driver.md`, §3                                | amended by the plan's pull request: the `pub(crate)` step and the texts that describe it move from F5b to F5a                                                                                                                                                     | —                           |

## 6. Execution

The stages are the driver's; the plan that follows the disposition writes one
task per landable change, with the files, the test and what it must not break.

1. **F3, first pull request — E11.16.** `Wakeable`, `Interest`,
   `Transport::Busy`, the frame-specification changes of F-13, the wake limit of
   F-3 recorded in the loopback record; the loopback implements `Wakeable` on
   every handle and `Clock` on `CallerHandle`; the mutation of F-14 named.
2. **F3, second pull request — E11.18.** `correlate::Table` and
   `correlate::Waiters`; the loopback moves onto both (F-9); `ClientError` and
   `ProviderError` (F-1), so the release carries every public item the face
   needs.
3. **F4, second half — E11.20.** The conformance suite gains the F-14 contract
   tests; the loopback runs them.
4. **F5a — E11.21, first half.** The async `Client`, the named futures, `serve`;
   the poll face `pub(crate)` under internal names, `dispatch` the internal
   step; `face.rs`'s module documentation restated (F-15); the cabin round trip
   and the round-trip tests through the async client; the interaction-face
   record gains a dated section for what F5a emits, and its rewrite stays F5b's.
5. **The release.** The 0.x minor carrying E11.16 to E11.19, a maintainer act.
6. **F5b — E11.21, second half.** The `blocking` module, and the emitted
   manifest's `std` feature forwarding to `ridl-rt/std`; the records,
   `examples/cabin/consumer` through both clients, the book; gardening. The
   driver's stage table is amended by the plan's pull request: the `pub(crate)`
   step moves from F5b to F5a.

**What binds E11.9**, for the driftsys/ridl#328 comment: F-5 and F-6 (the
`Wakeable` contract and the key a frame maps to), F-8 (the helpers a runtime
drives from its own lock), F-13 (`Busy` as a `response` outcome), and F-1's
error types, which a runtime never constructs but a consumer over it matches.

## 7. Trace

- Roadmap: `docs/ROADMAP.md` — E11.16, E11.18, E11.20, E11.21; the driver's §2
  table.
- Binds nothing on its own; it proposes.
- Reads: [ADR-0018](../decisions/ADR-0018-runtime-core-and-generated-surface.md)
  (the correction, the alternatives table, open question 5),
  [ADR-0020](../decisions/ADR-0020-third-encoding-runtime-layering-and-plugin-system.md)
  decisions 5 and 6,
  [ADR-0021](../decisions/ADR-0021-ridl-rt-0.1-api-and-release.md) decisions 8
  to 12 and open questions 5 and 6,
  [ADR-0023](../decisions/ADR-0023-interaction-face-generation.md) decisions 4
  and 5.
- Specifications:
  [the frame specification](../specification/frame-specification.md) §5, §6, §8,
  §9.6; [the ridl reference](../specification/ridl-language-reference.md) §4.4,
  §5, §6.1, §9, §10.3.
- Design records: [`ridl-rt.md`](../design/ridl-rt.md),
  [`interaction-face.md`](../design/interaction-face.md),
  [`ridl-loopback.md`](../design/ridl-loopback.md); the working note
  [`2026-09-08-ridl-rt-design.md`](2026-09-08-ridl-rt-design.md) §8 (RA-19,
  RA-20).
- Issues: driftsys/ridl#509 (the amendments; its two comments list what the F1
  reviews left for them), #485, #510, #512, #514, #515, #526 (the F1 review
  debt), driftsys/ridl#350 item 17 (the wake hook), driftsys/ridlc-gen-kotlin#3
  (K3b and K5).
- Code: `crates/ridl-rt/src/{port,error,contract,sample,task}.rs`,
  `crates/ridl-loopback/src/{lib,store,handle}.rs`,
  `crates/ridl-backend-rust/src/face.rs`,
  `crates/ridl-backend-rust/tests/generated/interaction_face.rs`,
  `examples/cabin/consumer/src/main.rs`.
