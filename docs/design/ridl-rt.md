# `ridl-rt` — the runtime library

`ridl-rt` 0.1.0 is the `no_std` library that defines what a package generated
from ridl will link and a runtime will implement: identity, time and the
envelope, samples, the payload traits, the interaction descriptors, the ports,
and the contract and transport errors (ADR-0020 decision 5). No backend emits
code against it yet and no runtime exists. It contains no runtime — nothing in
the crate performs I/O — and no engine: the store, the sans-IO session, the
scheduler and the platform traits are `ridl-engine`'s, parked outside this
repository and reopened by rmdl (ADR-0018's 2026-09-12 amendment). A runtime is
a separate crate that implements the traits of the `port` module; generated code
calls those traits without naming the runtime.

The crate carries `#![no_std]` and `#![forbid(unsafe_code)]`, has no dependency
in any feature combination, and allocates nothing. This is the architecture as
built; the decisions behind the choices that had more than one reasonable answer
are [ADR-0021](../decisions/ADR-0021-ridl-rt-0.1-api-and-release.md). Read this
record together with the
[ridl language reference](../specification/ridl-language-reference.md), which
every section below cites for the contract it implements.

## Module layout

Six modules, each public item living in exactly one (ADR-0020 decision 5;
`error` renamed from that decision's original `strata`, amended in place
2026-09-13):

| Module        | Contents                                                                                                                                                                                                                                                             |
| ------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `encoding`    | `Encoding` (sealed), `FlatBuffers`, `Proto3`, `ReprC`                                                                                                                                                                                                                |
| `payload`     | `Payload<E>`, `Ref`, `Encoded`, `EncodeError`, `VerifyError`, `Malformed`, `Violation`, `Rule`                                                                                                                                                                       |
| `sample`      | `Timestamp`, `Duration`, `Envelope`, `Provenance`, `Cause`, `Detection`, `Freshness`, `Sample`, `Occurrence`                                                                                                                                                         |
| `contract`    | `Ordinal`, `InterfaceNo`, `CatalogHash`, `CatalogRef`, `Kind`, `Interface`, `Interaction`, `Signal`, `Event`, `Command`, `Query`, `Fixed`, `Member`, `Timing`, `TimingMode`, `PayloadInfo`, `EncodedSizes`                                                           |
| `port`        | `Attached`, `Clock`, `SignalReader`, `SignalWriter`, `EventSource`, `EventSink`, `Caller`, `Handler`, `FixedReader`, `ScannableSignals`, `CoherentSignals`, `RawSample`, `RawOccurrence`, `Claim`, `ClaimId`, `Correlation`, `Watermark`, `Changed`, the port errors |
| `error`       | `Contract`, `Transport`, `CallError`                                                                                                                                                                                                                                 |
| `flatbuffers` | under the feature of the same name, since 2026-09-20: `Builder`, `Pos`, `Field`, `TableField`, `Vector`, the `read_*` scalar reads, `root`, `follow`, `field`, `string`, `vector`                                                                                    |

Generated code names every item by its full path and imports none, because
several names here — `Duration`, `Handler`, `Kind` — are also names in `core` or
in application code. A type with more than one codec qualifies its constant too:
`<T as Payload<E>>::MAX_SIZE`, because `Self::MAX_SIZE` is ambiguous once a type
implements `Payload` for two encodings.

## Identity

```rust
pub struct Ordinal(pub u32);          // a member's position in the interface body, from 1 (ridl §11)
pub struct InterfaceNo(pub u32);      // an interface's number in its catalog: frozen, or provisional
pub struct CatalogHash(pub [u8; 32]); // SHA-256 over the catalog's interfaces, their numbers, and the types they reach
pub struct CatalogRef { pub name: &'static str, pub hash: CatalogHash }
#[repr(u8)] pub enum Kind { Signal = 1, Event = 2, Command = 3, Query = 4, Fixed = 5 }
```

Neither `Ordinal` nor `InterfaceNo` is ever 0. `CatalogRef` equality compares
the name and the hash together, because two packages with identical contents
could share a hash. An interface number is scoped by its catalog rather than by
a service — there is no `ServiceId` and no catalog slot type — and a provisional
number is marked on the interface's descriptor (`Interface::PROVISIONAL`), not
folded into the type. The widths, the rename from the note's `InterfaceId`, and
why the number is catalog-scoped are
[ADR-0021](../decisions/ADR-0021-ridl-rt-0.1-api-and-release.md) decisions 1
and 2.

## A port is bound to one catalog

```rust
pub trait Attached { fn catalog(&self) -> &CatalogRef; }
```

Every interaction port carries `Attached` as a supertrait. Port methods keep
addressing an interaction by `InterfaceNo` and `Ordinal`, both scoped by the
port's catalog: a generated client compares `port.catalog()` against its
interface's `CATALOG` once, when the client is built, rather than checking a
full `(catalog, interface, ordinal)` key on every call. A provider that spans
two catalogs holds one port set per catalog and commits each; nothing atomic is
lost, because generation is already per interface. The rejected alternatives are
in ADR-0021 decision 3.

## The interaction descriptors

A descriptor is a zero-sized marker type with constants — the per-member form of
the ordinal table ADR-0013 decision 3 requires, extended with the interface
number and the catalog hash:

```rust
pub trait Interface {
    const CATALOG: &'static CatalogRef;
    const NUMBER: InterfaceNo;
    const PROVISIONAL: bool;
    const NAME: &'static str;
    const MEMBERS: &'static [Member]; // ordinal order; a reserved ordinal has no row
}
pub trait Interaction { type Iface: Interface; const MEMBER: &'static Member; }
pub trait Signal:  Interaction { type Payload; fn init() -> Self::Payload; }
pub trait Event:   Interaction { type Payload; }
pub trait Fixed:   Interaction { type Payload; }
pub trait Command: Interaction { type Args; fn require(args: &Self::Args) -> Result<(), ()>; }
pub trait Query: Interaction {
    type Args;
    type Reply;
    fn require(args: &Self::Args) -> Result<(), ()>;
    fn ensure(args: &Self::Args, reply: &Self::Reply) -> Result<(), ()>;
}

pub struct Member {
    pub ordinal: Ordinal,
    pub kind: Kind,
    pub name: &'static str,
    pub timing: Option<Timing>,           // None: no Timing in the IR (a bare call, a fixed)
    pub payloads: &'static [PayloadInfo], // one per payload; two for a query (request, then reply)
}
pub enum TimingMode { StrictPeriodic, Range }
pub struct Timing { pub mode: TimingMode, pub min: Option<Duration>, pub max: Option<Duration> }
pub struct PayloadInfo { pub type_name: &'static str, pub max_size: EncodedSizes }
pub struct EncodedSizes { pub proto3: Option<u32>, pub flatbuffers: Option<u32>, pub repr_c: Option<u32> }
```

`min` and `max` are each independently optional, because the IR leaves a
half-open side unset (`@[..500ms]` gives only a `max`, `@[20ms..]` only a
`min`); a strict period (`@Xms`) is its own `TimingMode` because both bounds
then hold the same value. `max` is a signal's staleness bound, an event's time
to live, and a call's response bound, depending on the member's `kind`. `Member`
and `PayloadInfo` are not `#[non_exhaustive]`, because generated code in another
crate builds both as struct literals — which is why a field added to either is a
breaking change (ADR-0021 decision 10). `Command` and `Query` return
`Result<(), ()>` from `require`/`ensure`: the value is decided in ADR-0021
decision 4.

**An `EncodedSizes` field is `None` when the toolchain cannot size the payload
for that encoding**, which covers both an encoding that cannot carry the payload
and one whose size is not derivable yet. The `repr(C)` column is `None` for
every payload until E11.12 defines the C-representable layout
(`docs/wip/2026-09-13-catalog-descriptor-plan.md` §3), and a backend that emits
descriptors before its codec exists writes `None` for that codec's column as
well — which is what E11.13's MVP does for all three. The field's own
documentation first read `None` as "that encoding cannot carry the payload"
alone; that is the narrower of the two cases and it made a backend with no codec
yet unable to say anything true. A consumer that needs to know which encodings a
payload has asks the catalog descriptor, not this field.

Not carried by a descriptor: the toolchain version (not identity), retired
interfaces and reserved ordinals (a runtime answers
`Contract::UnknownInteraction` for either), the stream element type and flag
(streams are out of 0.1), and `default_applied` (no runtime behaviour depends on
it).

## Time and the envelope

```rust
pub struct Timestamp(pub i64);  // microseconds since the PTP epoch, TAI (ridl §3.1)
pub struct Duration(pub i64);   // microseconds
pub struct Envelope { pub stamp: Timestamp, pub seq: u64 }
```

`Envelope` carries exactly the sender's timestamp and sequence number, stamped
once at origin and never changed by a relay. Before a signal's first publication
its envelope reads `seq` 0, stamped when the channel was created. On a call,
`seq` is unique per caller, not per channel — how a runtime keys duplicate
suppression from that fact, and the driftsys/ridl#308 disposition behind it, is
ADR-0021 decision 5.

## Provenance, freshness, the sample and the occurrence

```rust
pub enum Provenance { Init, Live, Invalid(Cause) }
pub enum Cause      { Declared, Detected(Detection) }
pub enum Detection  { InvalidValue(Violation), Corrupt }
pub enum Freshness  { Fresh, Stale { by: Duration }, Unbounded }

pub struct Sample<T> {
    pub value: T,
    pub provenance: Provenance,
    pub freshness: Freshness,
    pub envelope: Envelope,
}
impl<T> Sample<T> {
    /// `true` when the provenance is `Live` and the freshness is not `Stale`.
    pub fn usable(&self) -> bool;
}

pub struct Occurrence<T> { pub payload: Result<T, Detection>, pub envelope: Envelope }
```

A sample's `value` is never absent: the init value under `Init`, and the last
good value (or the init value, absent one) under `Invalid`. `Cause` lives inside
`Provenance::Invalid` rather than as a separate optional field, so an invalid
sample with no cause, or a live one with a cause, cannot be built.
`Detection::InvalidValue` is a consumer-side binding's detection of a typl
constraint violation (ridl §10.2); `Detection::Corrupt` is a serialization
failure (§10.3). There is no `Implausible` variant: ridl §3.3 states that
failure to correspond is not invalidity, so `SignalWriter::invalidate` takes no
cause: an invalidation the provider declares arrives as `Cause::Declared`, and
`Cause::Detected` is what the consumer's own binding reports, when its check
finds the payload invalid. `Freshness::Unbounded` is the freshness of a member
whose timing carries no `max` (`@[1s..]` is legal and receives no default upper
bound); a signal with no `@` annotation at all is not unbounded, because it
receives the default range.

`Occurrence<T>` is what a consumer receives from an event: `EventSource::next`
(below) stays unvalidated and returns bytes, and the generated binding runs the
check, so `payload` is `Err(Detection)` when the check fails rather than the
occurrence being silently dropped. The driftsys/ridl#309 disposition behind this
is ADR-0021 decision 6.

## The payload encodings and the proof type

```rust
mod sealed { pub trait Sealed {} }
pub trait Encoding: sealed::Sealed + 'static { const NAME: &'static str; }
pub struct FlatBuffers; // NAME = "flatbuffers"
pub struct Proto3;      // NAME = "proto3"
pub struct ReprC;        // NAME = "repr-c"

pub trait Payload<E: Encoding>: Sized {
    const MAX_SIZE: usize;
    type View<'a>; // what a successful check leaves behind: bytes, an accessor, or a parsed value
    fn encode<'o>(&self, out: &'o mut [u8]) -> Result<Encoded<'o, Self::View<'o>>, EncodeError>;
    fn verify(buf: &[u8]) -> Result<Self::View<'_>, VerifyError>;
    fn decode(r: Ref<'_, Self, E>) -> Self;
}

pub struct Encoded<'a, V> { pub bytes: &'a [u8], pub view: V } // data, not a proof; bytes is a subslice of out

pub struct Ref<'a, T: Payload<E>, E: Encoding> { /* private fields */ }
impl<'a, T: Payload<E>, E: Encoding> Ref<'a, T, E> {
    pub fn verify(buf: &'a [u8]) -> Result<Self, VerifyError>;    // calls T::verify
    pub fn encode(value: &T, out: &'a mut [u8]) -> Result<Self, EncodeError>; // calls T::encode
    pub fn bytes(&self) -> &'a [u8];
    pub fn view(&self) -> &T::View<'a>;
    pub fn into_view(self) -> T::View<'a>;
    pub fn decode(self) -> T;                                     // calls T::decode
}

#[non_exhaustive] pub enum EncodeError { Capacity { needed: usize, available: usize } }
#[non_exhaustive] pub enum VerifyError { Structure(Malformed), Contract(Violation) }
#[non_exhaustive] pub enum Malformed {
    OutOfBounds, Unaligned, MissingRequired, Utf8, Union, TooDeep, TooManyTables, TooLarge,
}
pub struct Violation { pub type_name: &'static str, pub rule: Rule }
#[non_exhaustive] pub enum Rule { Range, Step, Length, Pattern, Variant }
```

`Encoding` is sealed: `FlatBuffers`, `Proto3` and `ReprC` are the only encodings
a payload type can implement, and the marker types carry no cargo feature of
their own, so a runtime can name every encoding without linking a codec. `Ref`'s
fields are private, and `Ref::verify` and `Ref::encode` are the only functions
that build one — each calls the generated `Payload::verify` or `Payload::encode`
and wraps the result — so `Payload::decode` is reachable only with a proof that
the bytes were checked or that the crate's own encoder wrote them, and code ridl
emits needs no `unsafe` to decode. `Ref::encode` does not check the value's typl
constraints: a value is trusted to be valid when constructed, and the receiver's
`verify` reports one that is not. The closed encoding set and the proof-type
design are ADR-0021 decision 7.

`View` carries what the check produced — the bytes, an accessor over them, or an
owned parsed value — and has no `Copy` bound, because a parsed value holding a
`String` or a `Vec` is not `Copy`; `Ref` is therefore not `Copy` either, and
`Ref::view` lends the view while `Ref::into_view` moves it out.

## Features, `no_std`, `alloc` and `wasm32`

The crate root carries `#![no_std]` and `#![forbid(unsafe_code)]`, with no
`extern crate alloc`. The three features `flatbuffers`, `proto3` and `repr-c`
name the payload encodings, and the crate has no dependency in any feature
combination — decided in ADR-0021 decision 8. `just wasm-check` covers
`-p ridl-rt`, because a package's generated Rust links this crate and is
compiled to `wasm32` (ADR-0020 decisions 6 and 7).

**Since 2026-09-20 (story E11.7, stage K3) `flatbuffers` is no longer empty.**
It gates the `flatbuffers` module: the byte-order reads, the vtable walk, and
the tail builder that a generated `Payload<FlatBuffers>` implementation calls.
The module decides no layout — which slot a field takes, its offset in the table
and the table's size are the projection's facts, handed to the builder — which
is what keeps `Payload::MAX_SIZE` and the encoder from disagreeing. It still
takes no dependency: the FlatBuffers runtime crate ADR-0020 decision 5 permits
under this feature is not used. `proto3` and `repr-c` remain empty.

Because the workspace resolves this crate with default features, `just test`,
`just lint` and `just wasm-check` each carry a second invocation with
`--all-features` for `-p ridl-rt`; without them the feature-gated modules are
compiled only by `just compat-check`, at rust-version 1.83 and against the
packaged crate.

## The ports

Every port method returns without waiting, names no payload type, and takes no
`now` — a runtime reads its own `Clock`. A method that names one interaction
takes an `InterfaceNo` and an `Ordinal`, plus bytes where it carries a payload.

```rust
pub trait Clock { fn now(&self) -> Timestamp; }   // one per runtime; not Attached

pub trait SignalReader: Attached {
    fn read(&self, iface: InterfaceNo, ord: Ordinal, out: &mut [u8]) -> Result<RawSample, ReadError>;
}
pub struct RawSample { pub provenance: Provenance, pub freshness: Freshness, pub envelope: Envelope, pub len: usize }

pub trait SignalWriter: Attached {
    fn set(&mut self, iface: InterfaceNo, ord: Ordinal, bytes: &[u8]) -> Result<(), WriteError>;
    fn invalidate(&mut self, iface: InterfaceNo, ord: Ordinal) -> Result<(), WriteError>; // Cause::Declared
    fn touch(&mut self, iface: InterfaceNo, ord: Ordinal) -> Result<(), WriteError>;
    fn commit(&mut self); // cannot fail
}

pub trait EventSource: Attached {
    fn subscribe(&mut self, iface: InterfaceNo, ords: &[Ordinal]) -> Result<(), SubscribeError>;
    fn unsubscribe(&mut self, iface: InterfaceNo, ords: &[Ordinal]);
    fn next(&mut self, out: &mut [u8]) -> Result<Option<RawOccurrence>, ReadError>;
}
pub struct RawOccurrence { pub iface: InterfaceNo, pub ord: Ordinal, pub envelope: Envelope, pub len: usize }

pub trait EventSink: Attached {
    fn raise(&mut self, iface: InterfaceNo, ord: Ordinal, bytes: &[u8]) -> Result<(), RaiseError>;
}

pub trait Caller: Attached {
    fn command(&mut self, iface: InterfaceNo, ord: Ordinal, args: &[u8]) -> Result<Correlation, SendError>;
    fn query(&mut self, iface: InterfaceNo, ord: Ordinal, args: &[u8]) -> Result<Correlation, SendError>;
    fn ack(&mut self, c: Correlation) -> Option<Result<(), CallError>>;
    fn reply(&mut self, c: Correlation, out: &mut [u8]) -> Result<Option<Result<usize, CallError>>, ReadError>;
    fn forget(&mut self, c: Correlation);
}
pub struct Correlation(pub u64);

pub trait Handler: Attached {
    fn serve(&mut self, iface: InterfaceNo, ords: &[Ordinal]) -> Result<(), ServeError>;
    fn next_claim(&mut self, out: &mut [u8]) -> Result<Option<Claim>, ReadError>;
    fn settle(&mut self, claim: ClaimId, outcome: Result<&[u8], CallError>) -> Result<(), SettleError>;
}
pub struct Claim {
    pub id: ClaimId, pub iface: InterfaceNo, pub ord: Ordinal,
    pub envelope: Envelope, pub remaining: Option<Duration>, pub len: usize,
}
pub struct ClaimId(pub u64);

pub trait FixedReader: Attached {
    fn read_fixed(&self, iface: InterfaceNo, ord: Ordinal, out: &mut [u8]) -> Result<usize, ReadError>;
}

/// Extension: signals in a store a consumer can walk. A runtime may omit it.
pub trait ScannableSignals: SignalReader {
    fn generation(&self, iface: InterfaceNo) -> u64;
    fn scan(&self, marks: &mut [Watermark], out: &mut [Changed]) -> usize;
}
pub struct Watermark { pub iface: InterfaceNo, pub generation: u64, pub seq: u64 }
pub struct Changed { pub iface: InterfaceNo, pub ord: Ordinal, pub seq: u64 }

/// Extension: reads of several signals of one interface from one publication.
pub trait CoherentSignals: SignalReader {
    fn read_coherent(&self, iface: InterfaceNo, ords: &[Ordinal],
                     out: &mut [u8], samples: &mut [RawSample]) -> Result<usize, ReadError>;
}
```

`Watermark`'s generation counter field is named `generation`, not `gen`: `gen`
is a reserved keyword in the 2024 edition, so the field name the crate's earlier
working spec used does not compile, and `generation` also matches the name of
`ScannableSignals::generation`, the method that returns the same counter.

Semantics each implementation presents:

- **`SignalReader::read`** copies the slot into `out` and returns the
  provenance, the freshness and the envelope; the runtime resolves both.
- **`SignalWriter`**: `set` stages a value, `invalidate` stages the ridl §4.5
  invalid transition with `Cause::Declared`, `touch` re-affirms without a new
  value, and `commit` publishes everything staged under one generation increment
  and one timestamp per interface, taken from the runtime's `Clock`. `set`,
  `invalidate` and `touch` fail with `WriteError::Contract` when the ordinal
  names no member and `WriteError::NotOwner` when it names a member this
  provider does not own.
- **`EventSource::next`** returns occurrences in order; a gap in `seq` is a
  loss. An occurrence older than its time to live is discarded inside `next`.
- **`Caller::ack`** reports a command's delivery acknowledgment: `Ok(())`
  accepted, `Err(CallError::Contract(_))` a negative acknowledgment,
  `Err(CallError::Transport(Transport::Corrupt))` when the provider could not
  read the command's argument bytes, and
  `Err(CallError::Transport(Transport::Undelivered))` no acknowledgment within
  the bound. It returns `None` for a query's correlation; a query's outcome
  comes from `reply`.
- **`Caller::reply`**'s outer `ReadError` reports the port call itself (`Short`,
  `Detached`, never `Contract`); the inner `CallError` is the outcome from the
  peer or the transport.
- **`Handler`** presents each delivered call once through `next_claim`: a
  retransmission of an already-presented call is not presented again and
  receives the cached acknowledgment, and two callers are never merged even
  under the same `seq`. Every claim is settled — a command settles `Ok(&[])`
  after its arguments and `require` pass and before application code runs; a
  query settles with the reply bytes or the `CallError` outcome. A provider
  settles `CallError::Transport(Transport::Corrupt)` when the argument bytes
  fail the structure check.
- **`ScannableSignals::scan`** writes each interface's changes into `out` all
  together or not at all: when an interface's changes do not fit in the rest of
  `out`, none of them is written, that interface's mark is not updated, and
  `scan` returns the number of entries written so far.
- **A `Short` error does not consume.** `next`, `next_claim` and `reply` return
  `Result<Option<_>, ReadError>`, so a caller that receives
  `ReadError::Short { needed }` resizes and reads the same item again.

`ScannableSignals` and `CoherentSignals` are extensions: they describe
mechanisms some runtimes have, not interaction semantics every runtime must
present, so a runtime may omit either.

**Every port trait is implemented for `&mut P`, and the `&self`-only traits also
for `&P`.** For each port trait `T`, the crate provides
`impl<P: T + ?Sized> T for &mut P`; for the traits whose methods all take
`&self` — `Attached`, `Clock`, `SignalReader`, `FixedReader`, `ScannableSignals`
and `CoherentSignals` — it also provides `impl<P: T + ?Sized> T for &P`. A
generated face holds its port by value
([ADR-0023](../decisions/ADR-0023-interaction-face-generation.md) decision 5),
so these impls are what let it be built over a borrow of a handle. An owned
handle, a `Clone` handle and a wrapper that adds tracing or a test double are
accepted by the face's trait bounds with or without them, because such a type
implements the port traits itself rather than borrowing through them. The impls
are additive and need no `alloc`; `Box<P>` is not forwarded, because that would
need `alloc`, which no feature combination of this crate brings in.
[ADR-0021](../decisions/ADR-0021-ridl-rt-0.1-api-and-release.md) decision 11
records them and the reasoning.

**A runtime presents one handle per port role, and this crate adds no `Send` or
`Sync` bound.** A port role is one port trait. A runtime crate exposes one
handle type per port role it implements rather than one type implementing them
all, and may also offer an aggregate handle covering the port set one
interface's face needs. In a runtime whose handles are used from more than one
thread, a handle whose port traits all take `&self` is `Send + Sync`, because
several threads may read one store at once, and a handle carrying a trait with a
`&mut self` method is `Send` and need not be `Sync`, because one thread drives
each. An aggregate is `Send` or `Sync` exactly when the handles it holds are. A
face is built over one value implementing at least the port traits its interface
needs: the role handle itself when the face needs exactly one, and an aggregate
— the runtime's, or one the application writes over role handles — when it needs
more, because a generated `Client` may be bound over
`SignalReader + EventSource + Caller` at once. Either value reaches
`Client::new` by value, or as a `&mut` borrow of itself under the forwarding
impls above. No port trait in this crate carries `Send` or `Sync` as a
supertrait, so a single-threaded `no_std` runtime whose handles use `Cell` or
`RefCell` internally remains supported; a runtime checks its own handles with a
compile-time assertion, `fn assert_sync<T: Sync>()` applied to a reader handle.
[ADR-0021](../decisions/ADR-0021-ridl-rt-0.1-api-and-release.md) decision 12
records the reasoning and the alternative it rejects, one runtime struct behind
a mutex. No runtime exists in this workspace yet; story E11.15 builds the first
one to this shape.

## The `error` module and the port errors

```rust
// error::
pub enum Contract  { InvalidValue(Violation), PreconditionFailed, ContractBroken, UnknownInteraction }
#[non_exhaustive] pub enum Transport { Timeout, Undelivered, Down, Corrupt }
pub enum CallError { Contract(Contract), Transport(Transport) }

// port::
#[non_exhaustive] pub enum ReadError      { Short { needed: usize }, TooFewSamples { needed: usize }, Contract(Contract), Detached }
#[non_exhaustive] pub enum WriteError     { TooLarge { cap: usize }, NotOwner, Contract(Contract), Detached }
#[non_exhaustive] pub enum RaiseError     { Busy, TooLarge { cap: usize }, NotOwner, Contract(Contract), Detached }
#[non_exhaustive] pub enum SendError      { Busy, TooLarge { cap: usize }, Contract(Contract), Detached }
#[non_exhaustive] pub enum SubscribeError { Contract(Contract), Detached }
#[non_exhaustive] pub enum ServeError     { Contract(Contract), NotOwner, Detached }
#[non_exhaustive] pub enum SettleError    { UnknownClaim, TooLarge { cap: usize }, Detached }
```

`Contract` is ridl §10.2's four categories; `Transport` is §10.3's detected
infrastructure failures. `Detached` — the runtime behind the port is gone — is
local to the port, not a stratum. Why `Contract` and `CallError` stay exhaustive
while `Transport` and the seven port error enums stay `#[non_exhaustive]` is
ADR-0021 decision 9. Every error type here is `Copy` and owns nothing.

## What 0.1 leaves out

Streams (`stream`-typed fields) and `Inline` payloads; `Constrained` (nothing
calls it in 0.1); `Family` (the IR carries no family field yet); `ServiceId`
(dropped, ADR-0021 decision 1); `Encoding::FORMAT` (the frame specification's
wire tag values, added with story E11.1); `Access` (the trust constant the Rust
codegen generates once the rsdl lowering exists); the three payload codecs
(stories E11.7, E11.8 and E11.12); the runtimes `ridl-loopback` (story E11.15)
and `ridl-transport-ws` (story E11.9); and the engine. `Family` returns as a
`Member` field, which is a breaking change (ADR-0021 decision 10); streams have
no story yet.

## Versioning

`ridl-rt` carries its own version, independent of the workspace's, and its
release mechanics — the tag, and that creating it and running `cargo publish`
are maintainer acts — are [ADR-0007](../decisions/ADR-0007-e1-execution.md)
decision 14. What counts as a breaking 0.x change, and the crate's still-open
API questions, are
[ADR-0021](../decisions/ADR-0021-ridl-rt-0.1-api-and-release.md) decision 10.

## References

- [ADR-0021](../decisions/ADR-0021-ridl-rt-0.1-api-and-release.md) — the
  decisions behind the choices recorded here, and the alternatives considered
- [ADR-0020](../decisions/ADR-0020-third-encoding-runtime-layering-and-plugin-system.md)
  decisions 1 and 5 — the three payload encodings and the crate's module list
- [ADR-0018](../decisions/ADR-0018-runtime-core-and-generated-surface.md) — the
  layer this crate occupies, and what stays outside it as `ridl-engine`
- [ADR-0013](../decisions/ADR-0013-codegen-backend-scope.md) decision 3 — the
  ordinal table the interaction descriptors extend
- [ridl language reference](../specification/ridl-language-reference.md) — §3.1
  the envelope, §3.3 plausibility, §4.4 and §4.5 last-value and provenance, §5.2
  event time-to-live, §6 and §7 command and query, §8 `fixed`, §9 timing, §10
  the error strata, §11 identity
- `crates/ridl-rt/src/` — the crate as built; each module carries the same
  documentation this record summarises
- `docs/archive/2026-09-13-ridl-rt-v0.1-design.md` — the working spec this
  record and ADR-0021 were gardened from
