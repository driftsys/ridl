# `ridl-rt` by example

An introduction to the `ridl-rt` traits, built one interaction at a time from
the code the Rust backend actually generates.

This note is informative. Nothing depends on it. The binding records are
[ADR-0018](../decisions/ADR-0018-runtime-core-and-generated-surface.md),
[ADR-0020](../decisions/ADR-0020-third-encoding-runtime-layering-and-plugin-system.md)
and [ADR-0021](../decisions/ADR-0021-ridl-rt-0.1-api-and-release.md); the
as-built descriptions are [`ridl-rt.md`](../design/ridl-rt.md) for the library
and [`interaction-face.md`](../design/interaction-face.md) for the generated
face.

## Who this is for, and what it assumes

You are about to write an application against a RIDL interface in Rust. You have
seen `crates/ridl-rt/src/port.rs`, counted about a dozen traits and seven error
enums, and want to know which ones you need and in what order.

The answer is that you need very few of them directly. Generated code calls the
ports; you call the generated code. This note walks through one small interface
and introduces each trait at the point where the generated code first needs it.

It assumes Rust and the ridl language reference
([`ridl-language-reference.md`](../specification/ridl-language-reference.md)),
and nothing about the runtime.

## One thing to know before the examples

**`ridl-rt` is a library, not a runtime.** It defines the vocabulary and the
port traits and contains no implementation of them (ADR-0020 decision 6). The
one runtime in this workspace is `ridl-loopback` (story E11.15,
[its design record](../design/ridl-loopback.md)), which the examples below run
against: it holds every value in a map behind one lock, in one process, and its
clock is a counter a test advances by hand. It carries no frame and opens no
socket, so everything in this note that describes delivery over a transport,
timing or staleness is still describing what the specification requires of a
runtime rather than behaviour `ridl-loopback` produces — it measures nothing
against a staleness bound and discards nothing for age, because it holds no
member table to read a bound from. The other implementations of the ports in the
tree are test doubles, not runtimes: `MinimalSignalOnlyPort` in
`crates/ridl-backend-rust/tests/interaction_face.rs`, and `Stub` and `Memory`
inside `ridl-rt`'s own `tests/ports.rs` and `examples/read_sample.rs`.

The section [What is provisional](#what-is-provisional) lists every placeholder
the examples below stand on. Read it before you build on any of this.

## The interface everything below is generated from

This is `crates/ridl-backend-rust/tests/fixtures/interaction_face.ridl`, the
fixture the Rust backend's round-trip test compiles. Every generated signature
quoted in this note is copied from
`crates/ridl-backend-rust/tests/generated/interaction_face.rs`, which the
backend regenerates and compares byte for byte on every test run. Signatures are
abbreviated for reading rather than copied character for character: the fully
qualified paths the emitter writes are cut down, a `where` clause may be inlined
into the generic parameter list, a struct is shown with its module in front of
it and without its fields, and a method body is summarised where the point is
the shape rather than the code. Read the generated file itself for the exact
text. The declarations below are the fixture's, with its comments removed.

```ridl
package face.demo

type Temperature : integer [-40..85]
type Level       : integer [0..100]
type Window      : integer [0..100000]
type Average     : integer [0..1000]

enum Health {
  OK   = 0
  WARN = 1
  FAIL = 2
}

struct Warning {
  code   : Level
  health : Health
}

interface Cabin {
  signal  temperature : Temperature @10ms
  event   warning      : Warning @[100ms..1s]
  command setLevel(level: Level) @[..50ms] [
    require level < 100
  ]
  query   average(window: Window): Average @[..200ms] [
    require window > 0
    ensure  result >= 0
  ]
}

interface Horn {
  signal active : Health @10ms
}
```

`Cabin` has one of each interaction kind that carries a payload. `Horn` has a
signal and nothing else; it is here because a second, smaller interface is what
shows that the generated bounds are computed per interface rather than fixed.

The backend writes one Rust module per interface: `cabin` and `horn`.

## Step 1 — read one signal

The smallest useful thing is reading a value someone else publishes.

```rust
let client = cabin::Client::new(&mut port);
let sample = client.temperature()?;
```

### Why the return is not just `Temperature`

`temperature()` returns `Sample<Temperature>`, not `Temperature`:

```rust
pub struct Sample<T> {
    pub value: T,
    pub provenance: Provenance,
    pub freshness: Freshness,
    pub envelope: Envelope,
}
```

A signal is a channel with a current value, not a function call. At the moment
you read it, three questions are open that a bare value cannot answer.

**Has anyone published yet, and is the channel healthy?** That is `Provenance`
(ridl §4.5):

| `Provenance`     | Meaning                                                                    |
| ---------------- | -------------------------------------------------------------------------- |
| `Init`           | No publication yet. `value` is the channel's init value.                   |
| `Live`           | `value` is the latest publication.                                         |
| `Invalid(Cause)` | The channel is invalid. `value` is the last good value, or the init value. |

`value` is never absent. That is deliberate: a consumer that wants to proceed
with the init value can ignore the provenance, and one that must not proceed
reads the provenance. A `Cause` is either `Declared` — the provider called
`invalidate` — or `Detected(Detection)`, which is the consumer's own binding
reporting that the bytes it received did not pass their check.

**Is the value too old?** That is `Freshness`, measured against the staleness
bound the `@` annotation sets (ridl §9): `Fresh`, `Stale { by: Duration }`, or
`Unbounded` when the member's timing has no upper bound. A signal with no `@`
annotation is not unbounded — it receives the default range.

**When was it published, and how many publications have there been?** That is
`Envelope { stamp: Timestamp, seq: u64 }`, stamped by the sender's runtime and
unchanged by any relay.

For the common case there is one helper:

```rust
if sample.usable() { /* Live, and not Stale */ }
```

That is the whole of `Sample::usable`: provenance is `Live` and freshness is not
`Stale`. Anything more discriminating, you write yourself.

### The first port: `SignalReader`

For `Client::temperature` to exist, the port must be able to read a signal. That
is the first trait, and it is one method:

```rust
pub trait SignalReader: Attached {
    fn read(&self, iface: InterfaceNo, ord: Ordinal, out: &mut [u8])
        -> Result<RawSample, ReadError>;
}
```

Three things about it explain most of the rest of the port module.

**It takes bytes, not a type.** `out` is a byte buffer and `RawSample` reports
how many bytes were copied into it, alongside the provenance, the freshness and
the envelope. A port never names a payload type. That is what lets one runtime
serve every interface in a catalog without knowing any of their schemas — and it
is why there is a generated layer at all, because someone has to turn those
bytes into a `Temperature`.

**It identifies the member by number.** `InterfaceNo` and `Ordinal` are the
interface's number in its catalog and the member's position in the interface
body counted from 1 (ridl §11). Neither is ever 0. The generated code supplies
both as constants; you never write an ordinal by hand.

**It is `Attached`.** `Attached` has one method, `catalog()`, and its meaning is
that a port serves exactly one catalog — so the interface numbers and ordinals
its methods take are scoped by that catalog, and a number from another catalog
means nothing to it. Every port trait in the module has `Attached` as a
supertrait except `Clock`, which answers a question about the runtime rather
than about a member.

### What the generated method does with all that

This is `Client::temperature`, with the error handling summarised:

```rust
pub fn temperature(&self) -> Result<Sample<Temperature>, ReadError> {
    let mut buf = [0u8; <Temperature as Payload<Wire>>::MAX_SIZE];
    let raw = self.port.read(Cabin::NUMBER, Ordinal(1u32), &mut buf)?;
    match Ref::<Temperature, Wire>::verify(&buf[..raw.len]) {
        Ok(checked) => Ok(Sample { value: checked.decode(), /* raw's three fields */ }),
        Err(error)  => Ok(Sample {
            value: CabinTemperature::init(),
            provenance: Provenance::Invalid(Cause::Detected(/* from error */)),
            /* raw's freshness and envelope */
        }),
    }
}
```

Four points worth pausing on.

The buffer is a stack array sized from a constant, so a read allocates nothing.
`ridl-rt` itself is `no_std`, allocates nothing and contains no `unsafe` code,
and the generated face names only `core` paths and fixed-size arrays.

The ordinal and the interface number are written into the call site as
constants. There is no lookup at run time.

**A payload that fails its check is not an error.** The read succeeded; the
bytes were bad. Turning that into `Err` would force every consumer to handle a
case the `Provenance` already models, so the generated code reports it as
`Provenance::Invalid(Cause::Detected(..))` and gives you the init value to
proceed with. The `ReadError` in the signature covers the read itself — the
buffer was too short, or the runtime behind the port is gone.

The `Payload`, `Ref` and `Wire` names are the encoding machinery. Step 3 covers
them; for now, `verify` checks bytes and `decode` turns checked bytes into a
value.

## Step 2 — publish one signal

The provider side of a signal is `Publisher`:

```rust
let mut publisher = cabin::Publisher::new(&mut port);
publisher.temperature(Temperature(21))?;
publisher.commit();
```

The second line is not optional, and it is the reason `SignalWriter` has four
methods rather than one:

```rust
pub trait SignalWriter: Attached {
    fn set(&mut self, iface: InterfaceNo, ord: Ordinal, bytes: &[u8]) -> Result<(), WriteError>;
    fn invalidate(&mut self, iface: InterfaceNo, ord: Ordinal) -> Result<(), WriteError>;
    fn touch(&mut self, iface: InterfaceNo, ord: Ordinal) -> Result<(), WriteError>;
    fn commit(&mut self);
}
```

`set`, `invalidate` and `touch` **stage** a change. `commit` publishes
everything staged, with one generation increment and one timestamp for the whole
interface, taken from the runtime's clock. So a provider that updates four
signals of one interface and then commits produces one coherent publication, not
four.

The generated `Publisher` gives each of those a named method:

```rust
pub fn temperature(&mut self, value: Temperature) -> Result<(), WriteError>;
pub fn invalidate_temperature(&mut self) -> Result<(), WriteError>;
pub fn commit(&mut self);
```

`invalidate_temperature` stages the invalid state with `Cause::Declared` — this
is how a provider says "I know I cannot produce a value right now", as distinct
from the consumer-side `Cause::Detected` of step 1.

**`commit` returns nothing, and that is a deliberate gap.** It cannot fail, so
it reports nothing — including that the runtime behind the port is gone. You
learn about a detached runtime from `WriteError::Detached` on a _later_ `set`,
`invalidate` or `touch`, never from the `commit`. What becomes of changes staged
before a commit whose runtime has already detached is not fixed by the port
contract.

`touch` — re-affirming the current value without a new one, which keeps a
strictly periodic signal from going stale — is on the port but has no generated
method in this version.

## Step 3 — where the bytes come from

Steps 1 and 2 both crossed the same boundary: a `Temperature` on one side, a
`&[u8]` on the other. That boundary is the `payload` and `encoding` modules, and
it is worth understanding once because every interaction kind uses it.

**An encoding is a marker type.** `ReprC`, `Proto3` and `FlatBuffers` implement
the `Encoding` trait, which is sealed — the set is closed by ADR-0020 decision
1, so a fourth encoding is an ADR, not an `impl` in another crate. The markers
do not depend on a cargo feature, so a runtime can name an encoding without
linking a codec for it.

**A payload type implements `Payload<E>` once per encoding it supports.** So one
generated type can carry several codecs. The generated face reaches a payload
only through `Payload` and `Ref` — it calls `Ref::encode`, `Ref::verify` and
`Ref::decode`, and never sees a field, a tag or a layout. The face's own generic
parameter is the port, not the payload; a payload type is concrete at each call
site.

**`Ref` is a proof, not a pointer.** This is the part worth knowing:

```rust
pub fn decode(self) -> T;          // on Ref<'a, T, E>
pub fn verify(buf: &'a [u8]) -> Result<Self, VerifyError>;
pub fn encode(value: &T, out: &'a mut [u8]) -> Result<Self, EncodeError>;
```

`decode` consumes a `Ref`, and `verify` and `encode` are the only ways to build
one — its fields are private. So a value can only be decoded from bytes that
were checked, or that its own encoder wrote. There is no path from arbitrary
bytes to a decoded value that skips the check, and that is enforced by the type
system rather than by convention.

`verify` checks the structure of the bytes and the typl constraints of the value
**in one pass**, and reports the two separately:

| `VerifyError`          | Meaning                                                    | ridl stratum |
| ---------------------- | ---------------------------------------------------------- | ------------ |
| `Structure(Malformed)` | The bytes are not a well-formed encoding.                  | §10.3        |
| `Contract(Violation)`  | Well-formed bytes holding a value outside its constraints. | §10.2        |

Keeping them apart matters downstream: a `Temperature` of 200 is a contract
error the application caused, and a truncated buffer is an infrastructure
failure. A `Violation` names the typl type and which rule it broke — `Range`,
`Step`, `Length`, `Pattern` or `Variant` — so the report says `Temperature`
broke its range rather than that something somewhere was wrong.

Finally, `Payload<E>::MAX_SIZE` is the largest encoded size of any legal value.
It is what sized the stack buffer in step 1, and what the generated
per-interface buffer constants are computed from.

`encode` does **not** check the value's constraints. A value is trusted to be
valid when constructed, and the receiver's `verify` reports one that is not. The
generated encode sites treat a capacity failure as `unreachable!` with a message
naming the type, because a legal value cannot exceed its own `MAX_SIZE`.

> `Wire` is the alias the generated package carries for its payload encoding,
> `::ridl_rt::encoding::FlatBuffers` (E11.7's D-11). The `Payload`
> implementations behind it are generated, not hand-written: the round-trip test
> runs over the emitted codec.

## Step 4 — an event

An event is a stream of occurrences rather than a current value, so it needs two
ports rather than one: `EventSink` to raise, `EventSource` to receive.

Raising is the simpler half, and it is on the same `Publisher`:

```rust
publisher.warning(Warning { code: Level(5), health: Health::WARN })?;
```

There is no `commit`. `EventSink::raise` publishes one occurrence immediately —
an event is not staged, because there is no coherent set to assemble.

Receiving takes two calls:

```rust
client.subscribe_warning()?;
// ... later ...
match client.next_event()? {
    Some(cabin::Event::Warning(occurrence)) => { /* ... */ }
    None => { /* nothing waiting */ }
}
```

### Why there is one `next_event` for the whole interface

`subscribe_warning` is per event, but `next_event` is per interface. That
asymmetry comes straight from the port:

```rust
fn next(&mut self, out: &mut [u8]) -> Result<Option<RawOccurrence>, ReadError>;
```

`next` returns the next occurrence of _anything_ subscribed, and reports which
member it was in `RawOccurrence.ord`. The payload type is not known until that
ordinal has been read, so the generated code cannot offer a typed
`next_warning()`. Instead it reads the ordinal and routes to a generated enum:

```rust
pub enum Event {
    Warning(Occurrence<Warning>),
}
```

The buffer is sized by `Cabin::EVENT_SOURCE_BUFFER_SIZE`, the largest event
payload of the interface, because any of them may arrive.

**The generated code checks the interface number before the ordinal.** A port is
attached to a whole catalog and ordinals restart at 1 in each interface, so an
occurrence of a sibling interface at ordinal 2 would otherwise be decoded as a
`Warning`. Such an occurrence is reported as `Contract::UnknownInteraction`, and
it is lost — `EventSource::next` has already consumed it, and this face cannot
hand it back to the interface it belongs to. The practical rule is to subscribe
on a port the interface owns.

### Why an occurrence's payload is a `Result` and a sample's value is not

```rust
pub struct Occurrence<T> {
    pub payload: Result<T, Detection>,
    pub envelope: Envelope,
}
```

In step 1, a payload that failed its check still produced a value, because a
signal channel has a last good value or an init value to fall back on. An event
has neither. There is nothing to hand you, so the failure is in the payload
itself, carrying the same `Detection` — `InvalidValue(Violation)` or `Corrupt` —
the signal path puts inside its `Provenance`.

Two more properties of the port that generated code cannot give you and you
should know: occurrences arrive in order, and a gap in `envelope.seq` is a loss
(ridl §3.1); and an occurrence older than its time to live is discarded by the
runtime rather than delivered (ridl §5.2).

## Step 5 — a call, consumer side

Commands and queries are the first interactions with two ends and an outcome.
The generated client methods are:

```rust
pub fn set_level(&mut self, level: Level)   -> Result<SetLevelCorrelation, SendError>;
pub fn average(&mut self, window: Window)   -> Result<AverageCorrelation, SendError>;
```

Neither returns a reply. **Nothing in generated code waits.** There is no
thread, future, socket or timer anywhere in the emitted file, and no port method
blocks: every one of them returns immediately. Waiting — blocking, `async`, or a
loop driving a runtime — belongs to a layer above the port, and `ridl-rt`
defines no such layer.

So a call returns a correlation, which identifies that one sent call to its
caller, and you ask about the outcome separately:

```rust
let correlation = client.set_level(Level(42))?;
// ... later ...
match client.set_level_ack(correlation) {
    Some(Ok(()))   => { /* accepted */ }
    Some(Err(e))   => { /* rejected, or a transport failure */ }
    None           => { /* not known yet */ }
}
```

```rust
let correlation = client.average(Window(10))?;
// ... later ...
match client.average_reply(correlation)? {
    Some(Ok(average)) => { /* the reply */ }
    Some(Err(e))      => { /* a CallError from the peer or the transport */ }
    None              => { /* not known yet */ }
}
```

Each call has its own correlation type. `set_level` returns a
`SetLevelCorrelation` and `average` an `AverageCorrelation`, each a `Copy`
newtype around `ridl_rt::port::Correlation`, which is itself a `u64`. Only that
call's own outcome method accepts it: `set_level_ack` takes a
`SetLevelCorrelation` and `average_reply` an `AverageCorrelation`. The newtypes
are the generated face's. `Correlation` in `ridl-rt` stays untyped, because a
port carries identity and bytes and never a payload type, while which
interaction a correlation belongs to is a payload-shaped fact. Reach the `u64`
through the newtype's field — `correlation.0` — when you call a port method such
as `Caller::forget` yourself.

### Why a command and a query have separate methods

`Caller` has `command` and `query` as separate port methods, and correspondingly
a generated `*_ack` and a generated `*_reply`, because their outcomes are
different things (ridl §6, §7).

A command's acknowledgment is a **delivery** acknowledgment, not a completion
one (ridl §6.1). `Ok(())` means the provider accepted the command, not that it
finished doing anything. A command has no failure the application reports —
which is why, in step 6, the generated `Provider` method for a command returns
nothing at all.

A query's outcome is its reply. `Caller::ack` is always `None` for a query's
correlation, and that is a trap worth naming at the port: `ack` returns
`Option<...>` with no error case, so a caller that polls it for a query's
correlation waits forever. Through the generated face the trap is closed — a
`*_ack` takes its own command's newtype, so a query's correlation does not
compile there — but a caller that drives the `Caller` port itself must keep the
correlations `command` returned and ask only about those. Likewise, after
`Caller::forget` releases a correlation its outcome is no longer retrievable, so
do not ask about it.

### Why the error type is `SendError` and not `CallError`

`SendError` reports the failure to _send_: the runtime is busy, the arguments
exceed the call's capacity, a contract error, or the runtime is gone. It is what
the `Caller` port itself returns, so the generated method passes it through
without inventing a conversion.

`CallError` — `Contract` or `Transport` — is the outcome that comes back from
the peer, and it appears where outcomes appear: inside `ack` and inside
`average_reply`.

One case crosses that line deliberately. `Cabin.setLevel` declares
`require level < 100`, and the generated client evaluates it **before sending**:

```rust
CabinSetLevel::require(&level)
    .map_err(|()| SendError::Contract(Contract::PreconditionFailed))?;
```

A `require` that fails locally costs no round trip, and the caller sees the same
`Contract::PreconditionFailed` category it would have seen from the provider —
reported as a `SendError` because nothing was sent.

## Step 6 — the provider side, and `dispatch`

Serving `Cabin`'s calls means implementing one generated trait:

```rust
pub trait Provider {
    fn set_level(&mut self, level: &Level);
    fn average(&mut self, window: &Window) -> Average;
}
```

Three deliberate choices are visible in those two lines.

**A command returns nothing.** Not `Result`, not an error type. A command has no
failure the application reports (ridl §6.1); every outcome a caller can see as
an error is decided by the generated dispatch, before or around this call.

**A query returns its declared reply type, bare.** Same reason.

**Arguments are taken by reference.** This is not a style choice. The dispatch
must still hold a query's arguments _after_ the provider returns, because
`ensure result >= 0` is evaluated then, and the generated payload types
implement neither `Copy` nor `Clone`. Passing by value would move the argument
into the provider and leave nothing to evaluate `ensure` against. Note the
asymmetry with the client side of step 5, which takes its argument by value —
the client encodes it immediately and needs it no further.

### `Handler`: the port under the provider

```rust
pub trait Handler: Attached {
    fn serve(&mut self, iface: InterfaceNo, ords: &[Ordinal]) -> Result<(), ServeError>;
    fn next_claim(&mut self, out: &mut [u8]) -> Result<Option<Claim>, ReadError>;
    fn settle(&mut self, claim: ClaimId, outcome: Result<&[u8], CallError>)
        -> Result<(), SettleError>;
}
```

A `Claim` is one call presented to the provider: an id, the interface number and
ordinal, the caller's envelope, the argument length, and `remaining` — the time
left before the response bound passes, or `None` when the call has no response
bound (ridl §9.3).

The contract that shapes everything below is one sentence from the trait's own
documentation: **every claim is settled.**

### `dispatch`

You do not implement `Handler` and you do not call `next_claim`. The backend
generates the loop:

```rust
pub fn dispatch<H: Handler, P: Provider>(h: &mut H, p: &mut P, buf: &mut [u8]) -> usize
```

```rust
let mut buf = [0u8; Cabin::MAX_BUFFER_SIZE];
let settled = cabin::dispatch(&mut port, &mut provider, &mut buf);
```

It makes **one pass** over the claims the handler already has and returns how
many it settled. It does not wait. The loop that calls it belongs to the
application or to the runtime — which is the same property as step 5's
`Correlation`, seen from the other end.

`buf` is caller-owned and must be at least `Cabin::MAX_BUFFER_SIZE` bytes,
because a reply is encoded into the same buffer the arguments arrived in. A
shorter buffer returns `0` **without consuming a claim**, so the caller can
retry with a correctly sized one rather than losing a call to a programming
error.

The returned count is claims that `Handler::settle` **accepted**. A
`SettleError` is left to the handler, which already owns that claim's
settlement, and the pass continues with the next claim.

### The settlement table

The generated match is total over the claims that can arrive, because the
`Handler` contract requires it. The rows are in the order dispatch evaluates
them:

| Cause                                       | Settled as                     |
| ------------------------------------------- | ------------------------------ |
| `claim.iface` or `claim.ord` matches no arm | `Contract::UnknownInteraction` |
| `VerifyError::Structure(m)`                 | `Transport::Corrupt`           |
| `VerifyError::Contract(v)`                  | `Contract::InvalidValue(v)`    |
| `require` returns `Err(())`                 | `Contract::PreconditionFailed` |
| `ensure` returns `Err(())`, a query only    | `Contract::ContractBroken`     |

Reading it top to bottom: route the claim, check the bytes, check the
precondition, call the provider, check the postcondition, settle.

The first row is the fallback arm. Without it an unroutable claim would never be
settled, which breaks the `Handler` contract. `UnknownInteraction` is exactly
the category the language defines for peers disagreeing on an interface number
or an ordinal.

The two `VerifyError` rows are separate for the reason step 3 gave: collapsing
them would settle a range violation as a transport corruption, which reaches the
caller in the wrong stratum.

The last row is the only one evaluated **after** the provider runs, and the only
one that discards a reply the provider already produced. A query whose `ensure`
fails settles `ContractBroken`; the reply is not sent.

### When each kind is settled

A **command** is settled `Ok(&[])` once its arguments and its `require` clauses
pass and _before_ the provider's method runs — because the acknowledgment is a
delivery acknowledgment, not a completion one.

A **query** is settled _after_ the provider returns, because the settlement
carries the reply.

> This ordering is currently pinned by an assertion on the generated source
> text, not by a behavioural test. Nothing in the in-process test double has a
> side effect that reordering would change, so no test can observe the
> difference today. E11.9 is what would make it observable.

## Step 7 — the descriptors, and why the bounds are exact

One thing has been in every example without being introduced: `Cabin` itself.
The backend emits a zero-sized marker type per interface and per interaction,
carrying constants.

```rust
pub trait Interface {
    const CATALOG: &'static CatalogRef;
    const NUMBER: InterfaceNo;
    const PROVISIONAL: bool;
    const NAME: &'static str;
    const MEMBERS: &'static [Member];
}
```

`PROVISIONAL` is `true` when the interface number has not been frozen in a lock
file. `MEMBERS` is in ordinal order, and a reserved ordinal has no row — so the
index of a row is _not_ its ordinal minus one. Each `Member` carries its
ordinal, its `Kind` (`Signal`, `Event`, `Command`, `Query`, `Fixed`), its name,
its resolved `Timing`, and a `PayloadInfo` per payload: one for most kinds, two
for a query — the request, then the reply.

Per interaction there is `Interaction` (which interface, and which member row)
plus the kind's own trait. `Signal` adds `init()`, the channel's init value that
step 1 falls back to. `Command` adds `require`. `Query` adds `require` and
`ensure`. Those are the methods steps 5 and 6 were calling.

### Why `Client` is generic over a port

Both interfaces in the fixture produce a `Client`, and the bounds differ:

```rust
pub struct cabin::Client<P: SignalReader + EventSource + Caller>;
pub struct horn::Client<P: SignalReader>;
```

```rust
pub struct cabin::Publisher<W: SignalWriter + EventSink>;
pub struct horn::Publisher<W: SignalWriter>;
```

The bounds are computed from the interaction kinds the interface actually
declares. `Horn` has one signal, so its `Client` requires a port that can read a
signal and nothing more, and a port type implementing only `SignalReader` and
its `Attached` supertrait constructs it. `Horn` generates no `Provider` and no
`dispatch` at all, because it declares no call.

This is why the face is generic over a port rather than taking a single runtime
type: an application links exactly the port capabilities its interfaces use, and
a runtime that offers only some of them still serves the interfaces that fit.

A face holds its port by value and carries no lifetime parameter, so `P` is
whatever you hand `new`. `Client::new(&mut port)` infers `P` as `&mut Port`,
because `ridl-rt` implements every port trait for `&mut P`; an owned handle, a
`Clone` handle and a wrapper that forwards the port traits are accepted just as
well. A face built over a borrow holds that borrow for as long as the face
lives, so a runtime that implements every port on one value can be held by one
face at a time, and by none while `dispatch` runs over it. That is why the
examples above build a client, use it, and let it go before the next step.

## The ports that did not appear

Four of `port.rs`'s traits have had no step of their own, which is itself
informative.

**`Clock`** — `fn now(&self) -> Timestamp`. A runtime has one and stamps
envelopes from it. No port method takes the current time, and generated code
never calls `Clock`.

**`FixedReader`** — the consumer side of `fixed`, the provisioned constants of
ridl §8. The emitter writes a `fixed` member's descriptor but no method for it,
because a `fixed` is provisioned rather than interacted with. An application
that needs a provisioned value calls `read_fixed` on the port itself. The
fixture declares no `fixed` member.

**`ScannableSignals`** and **`CoherentSignals`** are **extensions**. They
describe mechanisms some runtimes have, not interaction semantics every runtime
must present, so **a runtime may omit them** — and the generated face emits no
method bounded on either.

- `ScannableSignals` is for walking a signal store: `generation(iface)` and a
  `scan` over watermarks. Its `scan` has a subtlety worth knowing before you use
  it — a return of `0` means either "nothing changed" or "the next interface's
  changes did not fit in `out`", and the two are told apart by comparing each
  mark against `generation(iface)` after the call.
- `CoherentSignals::read_coherent` answers several ordinals of one interface
  from **one publication**, which is what step 2's staging-then-commit produces.
  A runtime whose binding delivers each field separately cannot present this and
  omits it.

A generated coherent multi-read, bounded on `CoherentSignals`, is a follow-up.

## The error enums, in one table

There are seven port error enums rather than one, so that each method's
signature says what can actually go wrong at that method. They repeat a small
set of causes:

| Cause                | Appears in                                                                           |
| -------------------- | ------------------------------------------------------------------------------------ |
| `Detached`           | Every one. The runtime behind the port is gone.                                      |
| `Contract(Contract)` | `ReadError`, `WriteError`, `RaiseError`, `SendError`, `SubscribeError`, `ServeError` |
| `TooLarge { cap }`   | `WriteError`, `RaiseError`, `SendError`, `SettleError`                               |
| `Busy`               | `RaiseError`, `SendError`. Retryable.                                                |
| `NotOwner`           | `WriteError`, `RaiseError`, `ServeError`                                             |

`ReadError` serves every read on every port, so some of its variants are
unreachable for a given method — `TooFewSamples` belongs to `read_coherent`
alone, and `Caller::reply` never returns `ReadError::Contract`. Each method's
documentation says what it can return.

All seven are `#[non_exhaustive]`, so a match on one needs a `_` arm. `Contract`
and `CallError` are **not**: they are exhaustive, and a match naming every
variant compiles without a `_` arm. That split is pinned by doctests in
`crates/ridl-rt/src/lib.rs`, so it cannot change silently.

## What is provisional

Everything in this note is real code that compiles and runs. Several pieces of
what it stands on are placeholders with a named replacement.

| Placeholder                                                         | Replaced by               |
| ------------------------------------------------------------------- | ------------------------- |
| The transport under the in-process runtime the examples run against | E11.9                     |
| `CATALOG.hash`, an all-zero `CatalogHash`                           | E16.2 (driftsys/ridl#378) |
| Every `PayloadInfo.max_size` field, which is `None`                 | E16.2 (driftsys/ridl#378) |
| The contract-clause translator                                      | E5.1                      |

Two further limits are not placeholders but scope:

**`ridl --emit rust` does not emit the face.** The pipeline calls the backend's
`generate`, which is unchanged; the face comes from a separate `generate_face`
entry point. ADR-0018 decision 15 makes the face phase 2, and reaching the
command line is story E11.14's.

**Every command and query takes exactly one parameter.** Multi-parameter calls
need induced argument structs, which the Rust backend does not emit from an
interface. It is a follow-up.

The clause translator is worth one extra sentence, because it fails loudly. The
IR carries a contract clause only as canonical ridl text, not as an expression
tree, so the translator accepts exactly one form —
`<subject> <comparison> <numeric literal>`, where the subject is the
interaction's single declared parameter or `result` on an `ensure` — and
**refuses every other form with a `GenerateError` rather than dropping it**. A
dropped clause would generate a provider that accepts arguments its own contract
forbids, which is worse than refusing to generate. E5.1 replaces it with one
driven by the structured expression tree.

## Where to look next

| Question                                 | Where                                                                                                                                                           |
| ---------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| The traits themselves                    | `crates/ridl-rt/src/port.rs`                                                                                                                                    |
| The library as built                     | [`../design/ridl-rt.md`](../design/ridl-rt.md)                                                                                                                  |
| The generated face as built              | [`../design/interaction-face.md`](../design/interaction-face.md)                                                                                                |
| The concrete generated code quoted here  | `crates/ridl-backend-rust/tests/generated/interaction_face.rs`                                                                                                  |
| A worked round trip                      | `crates/ridl-backend-rust/tests/interaction_face.rs`                                                                                                            |
| The runtime the round trip runs over     | [`../design/ridl-loopback.md`](../design/ridl-loopback.md)                                                                                                      |
| What the language requires               | [`../specification/ridl-language-reference.md`](../specification/ridl-language-reference.md)                                                                    |
| Why the runtime is layered this way      | [ADR-0018](../decisions/ADR-0018-runtime-core-and-generated-surface.md), [ADR-0020](../decisions/ADR-0020-third-encoding-runtime-layering-and-plugin-system.md) |
| Why the `ridl-rt` API is shaped this way | [ADR-0021](../decisions/ADR-0021-ridl-rt-0.1-api-and-release.md)                                                                                                |
