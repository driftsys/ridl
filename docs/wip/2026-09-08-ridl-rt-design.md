# `ridl-rt` — the runtime library generated code is written against

Status: working note, 2026-09-08. Proposes the library every generated package
links: the trait vocabulary for interactions, payload encoding, verification and
provenance, and the ports an implementation provides. It answers the shape
ADR-0018 decision 15 left to phase 2 — "a concrete client generic over a
transport and a server trait the provider implements" — and it is the only part
of that record's runtime story that stays in scope.

Scope, and the descope it records: ridl owns **types, interactions and system**
— typl, ridl, rsdl, the IR, `ridl diff`, the emitters and this library. It does
**not** own a store, a seqlock, a frame protocol, a subscription table, platform
traits or a sans-IO session. Those are deployment decisions rather than language
ones, they are implemented in external packages, and the scheduling and
step-machine questions behind them are revisited when rmdl lands — because
execution is rmdl's subject and deciding it before that language exists is
deciding it blind.

Nothing here is implemented. The first implementation is the first runtime,
built by the first consumer, which implements these ports over its own store and
transport.

## 0. What this descopes, and what survives

    reasoning trail  docs/archive/2026-09-08-roadmap-simplification.md,
                     which reaches the same place from the plan's side: S-18
                     fixes the language order typl -> ridl -> rsdl ->
                     rmdl, S-19 parks the engine block as a block, S-22
                     recasts the codecs as backend stories, and S-23 adds
                     SPIKE-1 — the activation model — as the gate that
                     reopens the park by observation rather than by
                     argument. Its own sentence is the summary: *ridl
                     ships the contract and the traits; the first runtime
                     is the consumer's runtime.* This note is what "the traits" are.

**A name is reassigned, and both documents must say the same thing.** That
roadmap note uses `ridl-rt` for the **engine** — the store, the seqlock, the
sans-IO core and their platform traits — which is what S-19 parks. This note
takes `ridl-rt` for the **library**, because that is what every code generator
calls its companion and because the engine no longer needs the name it is parked
under. So the parked block wants a name of its own — `ridl-engine`, or
`ridl-session`, which is accurate since ADR-0018's four calls are a session —
and S-19, S-21 and the unscheduled table should carry it. Left as it stands,
"park `ridl-rt`" and "build `ridl-rt` first" appear in two working notes dated
the same day.

ADR-0018 bundled what the toolchain emits with what a runtime does. That record
is still _Proposed_, so the split is cheap to make now and expensive after Epic
11's stories are written against it.

    survives untouched   3  two encodings          4  generated codec
                         5  encoding vs represent. 6  language scope
                        13  bridge transcoders    15  the restored face
                        17  deployment schema     18  the access service

    out of scope         2  the sans-IO core       7  the frame protocol
                         8  the store layout      11  privacy and mapping
                        12  ring depth            16  ridl-rt as an engine

    splits              10  WHICH checks are emitted is ridl's; HOW a
                            mapping is protected is the runtime's

The name moves with the split. **`ridl-rt` is this library** — the runtime every
codegen ecosystem has, the thing generated code links: `prost`, `serde`,
`antlr4-runtime`, the `flatbuffers` runtime. What ADR-0018 called `ridl-rt` was
a state machine, which is an _engine_ rather than a runtime library, and it is
now out of scope. If it returns with rmdl it needs a name of its own —
`ridl-session` is accurate, since ADR-0018's four calls are exactly a session.

**Epic 11 collapses** to three stories it already contained: this library
(E11.10's component binding contract, promoted), the two codecs (E11.7, E11.8),
and the deployment-facts schema (E11.6). E11.1 to E11.5 and E11.9 move to an
rmdl-era epic.

**The consequence worth stating: the IR becomes the product.** ridl compiles
source to IR and anyone writes an emitter over it. An external runtime's own
descriptor blob, region map and typed layer are emitters over that IR living
outside ridl — which is already how the first consumer works, since its store
ABI is an agreement between it and whatever generated its blob, not a ridl
document.

## 1. Why a separate library

ADR-0018 found that the shipped interaction layer emitted its vocabulary once
per package — every package declared its own `Provenance`, its own
`SignalHandle` — so no runtime could ship `impl SignalHandle for …` for a trait
that exists once per package, and cross-package generic code was impossible.
Decision 15 retracted the layer and promised a phase 2 shaped as "a concrete
client generic over a transport and a server trait the provider implements".

That sentence names the missing thing. A client generic over a transport is
generic over **traits the transport implements**, and those traits have to be
defined exactly once, in a library that both the emitter's output and an
implementation depend on, and that depends on neither. That library is `ridl-rt`
— the companion every code generator ships, and the only thing standing between
a generated package and a runtime that can serve it.

**There are already two implementations in prospect**, which is why it cannot
live inside either: the first runtime, from the first consumer, and whatever
ridl grows when rmdl arrives. Put the traits in one and the other depends on a
runtime it does not use, purely for the vocabulary — the `slf4j` lesson, that a
facade must not live inside an implementation.

    application               hand-written, calls generated code
    generated bindings        client, provider trait, publisher,
                              payload types, descriptors
                │ written against
    ─────────  ridl-rt  ──────────────────  ridl's scope ends here  ──
                │ implemented by
    a runtime                 the first runtime · a browser shim · an rmdl host
    its platform              whatever that runtime needs

Three things follow from its position and decide its contents.

**It has no state machine.** Generated payload types must exist without a
runtime — most consumers only map and read — and `Inline::SIZE` must agree with
the region map's widths. So the codec traits live here and not in `ridl-rt`, and
`ridl-rt` is one implementation of the ports rather than their definition.

**It is `no_std`, no alloc, no clock, no IO.** Anything with a thread, a future,
a socket or a timer is a face over a port, not a port. This is ADR-0018's
asymmetry applied to the trait layer: a pulled port can be wrapped blocking or
async; an `async fn` in a trait cannot be unwrapped.

**It is the third versioned contract.** Beside the IR schema and the frame,
these traits are what an emitter's output and a runtime agree on. A change here
is an ABI event for every generated package and every runtime, which is why the
crate is small and its rules are numbered.

### 1.1 Module layout

Several of these names are ordinary English words that would collide in any
application crate — `Handler`, `Caller`, `Sample`, `Member`, `Interface`, `Kind`
— and one is worse than a collision: `Duration` beside `core::time::Duration` in
the same file is a bug waiting to be written. So the crate is modules with short
names inside them, the `io::Error` idiom:

    ridl_rt::
        encoding::   Encoding · FlatBuffers · Proto3
        payload::    Payload · Inline · Ref · Constrained
                     EncodeError · VerifyError · Violation · Rule
        sample::     Provenance · Cause · Freshness · Sample · Envelope
                     Timestamp · Duration
        contract::   Interaction · Signal · Event · Command · Query · Fixed
                     Interface · Member · Access · Ordinal · InterfaceId
                     Kind · Family
        port::       SignalReader · SignalWriter · EventSource · EventSink
                     Caller · Handler · FixedReader · Clock
        strata::     Contract · Transport · CallError · Ack · Outcome

    fn verify(buf: &[u8]) -> Result<payload::Ref<'_, Self, E>, payload::VerifyError>;
    pub struct Client<'a, P: port::SignalReader + port::Caller> { .. }

The rule that makes it hold: **the emitter always qualifies and never imports a
bare name.** Generated code is the overwhelming majority of the usage, so that
is a property of the emitter rather than a discipline anyone has to remember;
hand-written code may `use` what it likes locally and rustc errors on a real
conflict rather than choosing silently.

    RA-01  MUST   `ridl-rt` is `no_std`, with no alloc, no clock, no
                  IO and no `async`. Its only dependency is the
                  FlatBuffers runtime, for the `Verifiable` structural
                  pass (§4.1) — nothing else, ever.
    RA-02  MUST   Every trait here is defined exactly once. An emitter
                  never re-emits any of them; a runtime never redefines
                  them.
    RA-03  MUST   A runtime implements ports; generated code calls ports
                  and never a runtime type. The dependency graph is
                  emitter output → ridl-rt ← runtime, and nothing else.
    RA-26  MUST   Nothing enters this crate that is not implemented by
                  generated code or by a runtime. Helpers, conveniences
                  and utilities are what turns a shared crate into a
                  dumping ground.
    RA-27  MUST   Every public item lives in one of the modules of §1.1,
                  and the emitter qualifies rather than importing bare
                  names.

## 2. Identity and the envelope

Straight from the language reference, so the trait layer adds no vocabulary the
contract does not already have.

    pub struct Ordinal(pub u16);        // §11: 1-based, per interface, append-only
    pub struct InterfaceId(pub u16);    // §14.5: 1-based, per service
    pub struct ServiceId(pub u16);      // the deployment table's

    #[repr(u8)] pub enum Kind   { Signal = 1, Event = 2, Command = 3, Query = 4, Fixed = 5 }
    #[repr(u8)] pub enum Family { Dispatch = 0, Presentation, Intent, Acquisition, Control }

    pub struct Timestamp(pub i64);      // §3.1: µs since the PTP epoch, TAI
    pub struct Duration(pub i64);       // µs
    pub struct Envelope { pub stamp: Timestamp, pub seq: u64 }

`Kind` has exactly the five values the language has. A domain that needs another
one — the first consumer's catalog surface — is a domain extension per ADR-0012
decision 7: a payload type plus an attribute, never a sixth kind. A surface's
record is a signal; its pixels move outside the interaction fabric altogether.

The envelope is sender-stamped and never re-stamped (§3.1). The runtime supplies
it; generated code exposes it beside the value and never declares it. Where the
platform's synchronized time base is a mapped clock region rather than PTP on a
NIC, the runtime's `Clock` port is that region, and the domain is still §3.1's.

    RA-04  MUST   `Kind` has five values. Domain kinds are attributes on a
                  payload type, resolved by an emitter, never a variant.
    RA-05  MUST   `Envelope` is stamped by the producing binding from the
                  runtime's `Clock`, in the platform time base, and is
                  carried unchanged by every relay.

## 3. Provenance, freshness, and the sample

§4.5 is normative about what a subscriber sees: **value + provenance**, with
provenance `init | live | invalid` and the last good value still accessible in
the invalid state. Freshness (§9.2) is a second axis the consumer computes
against its own clock, and it must not be folded into provenance because a stale
live value and a fresh invalid one are different facts needing different
reactions.

    #[repr(u8)] pub enum Provenance { Init = 0, Live = 1, Invalid = 2 }

    /// Why a channel is invalid. `Declared` is the provider's own transition
    /// (§4.5); the other three are detections a consumer's binding makes when
    /// the producer's trust level is below its own (§7).
    #[repr(u8)] pub enum Cause { Declared = 0, OutOfRange, Corrupt, Implausible }

    pub enum Freshness { Fresh, Stale { by: Duration } }

    pub struct Sample<T> {
        /// The latest published value, or the init value under `Init`, or the
        /// last good value under `Invalid`. Never absent: the channel is never
        /// empty (§4.4).
        pub value: T,
        pub provenance: Provenance,
        pub cause: Option<Cause>,       // Some iff Invalid
        pub freshness: Freshness,
        pub envelope: Envelope,
    }
    impl<T> Sample<T> {
        /// Live and fresh. The one question a renderer asks.
        pub fn usable(&self) -> bool;
    }

What the application does on `!usable()` — hold last good, fail safe, degrade —
is failure management (§10.4). The trait layer's job is that the fact is
visible, typed and timely, which is why `Sample` carries both axes and the cause
rather than a boolean.

    RA-06  MUST   A signal read returns `Sample<T>` with the value never
                  absent: init before the first publication, last good
                  under `Invalid`.
    RA-07  MUST   Provenance and freshness are separate fields. A binding
                  never reports staleness as invalidity or the reverse.

## 4. Encoding, verification, decoding

ADR-0018 decision 3: proto3 on the network, FlatBuffers in memory, and the codec
is generated. So one codec trait, parameterised by encoding, and every generated
payload type implements it for the encodings its package was built with. The
parameter is not ceremony: Rust allows one impl of a trait per type, so it is
the only way `Speed` can carry two codecs under one trait.

    mod sealed { pub trait Sealed {} }

    /// Decision 3 closes the set at two, so this is sealed: a third encoding
    /// is an ADR amendment, not a downstream `impl`.
    pub trait Encoding: sealed::Sealed {
        const NAME: &'static str;
        /// The value a frame header carries to say how to decode its payload.
        /// The one definition of that mapping.
        const FORMAT: u16;
    }
    pub struct FlatBuffers;   // mapped, read in place
    pub struct Proto3;        // parsed to an owned value

    /// A proof, not a view. Private field, no public constructor: the only
    /// two ways to obtain one are `verify` (by checking) and `encode` (by
    /// provenance — you just produced these bytes). So `decode` is
    /// infallible by construction.
    pub struct Ref<'a, T, E: Encoding> { buf: &'a [u8], _t: PhantomData<(T, E)> }

    pub trait Payload<E: Encoding>: Sized {
        /// Largest encoding on this encoding. On FlatBuffers it equals the
        /// slot capacity the store projection assigned (RA-10).
        const MAX_SIZE: usize;
        fn encode<'o>(&self, out: &'o mut [u8]) -> Result<Ref<'o, Self, E>, EncodeError>;
        /// One pass: structure, then the typl contract. No allocation, no
        /// clock, on a private copy — never on a live slot.
        fn verify(buf: &[u8]) -> Result<Ref<'_, Self, E>, VerifyError>;
        fn decode(r: Ref<'_, Self, E>) -> Self;
    }

    /// One-size encoding: a FlatBuffers struct. Named `Inline` and not
    /// `Fixed` because `fixed` is an interaction kind (§8) and both appear
    /// in the same systems.
    pub trait Inline: Payload<FlatBuffers> {
        const SIZE: usize;
        fn load(buf: &[u8; Self::SIZE]) -> Self;
        fn store(&self, out: &mut [u8; Self::SIZE]);
    }

    /// The typl contract of a constructed value (Epic 10's checked
    /// constructor). What `verify` calls after the structural pass.
    pub trait Constrained { fn check(&self) -> Result<(), Violation>; }

### 4.1 Verification is two layers with two owners

**Structure belongs to the encoding's own runtime.** `flatc --rust` emits
`impl Verifiable` per table and `flatbuffers::root_with_opts` runs it — root
offset in bounds, vtable in bounds and aligned, each field inside the table,
strings NUL-terminated inside the buffer, union discriminants type-checked, with
depth and table limits we tighten per type from the catalog's bounds. ridl does
**not** generate a structural verifier where an audited one exists; it would be
rewriting, less well, code with far more eyes on it.

**The contract belongs to ridl.** Range, step, unit, pattern, bounds and the
interface's cross-field invariants — everything typl declares and nothing flatc
knows — emitted per type and run on the same pass.

    fn verify(buf: &[u8]) -> Result<Ref<'_, Self, FlatBuffers>, VerifyError> {
        let opts = VerifierOptions { max_apparent_size: Self::MAX_SIZE,
                                     max_depth: 2, max_tables: 1,
                                     ..Default::default() };
        let t = flatbuffers::root_with_opts::<fb::NavStatus>(&opts, buf)?;  // flatc
        typl::check_distance(t.dist())?;                                    // ridl
        typl::check_str::<64>(t.street())?;
        Ok(Ref::of(t))
    }

That gives `ridl-rt` its one dependency — the `flatbuffers` runtime, `no_std` —
which amends RA-01. The no-alloc `encode` needs its `Allocator` trait, added in
the 24.x series, so the version available on a target platform is a prerequisite
rather than a detail.

An `Inline` payload has no structural layer at all: a packed struct has no
offsets, so there is nothing to check and `load()` is the whole codec.

### 4.2 No `unsafe`, anywhere

An earlier draft had `unsafe fn from_trusted` as the escape hatch for a trusted
producer. That would put an `unsafe` block in every generated package — a
`#![deny(unsafe_code)]` no generated crate could hold, and one justification per
payload type in any safety argument. Three moves remove it.

**`encode` returns the proof.** Bytes you produced have known provenance; no
check is needed and none is skipped.

**The unchecked read is `Inline`-only.** `unsafe` was needed only because
skipping the verifier and then following vtable offsets in malformed bytes is
genuine UB. A struct has no offsets: `load` is an array reference, constant
offsets and `from_le_bytes`. Bad bytes give a wrong value — memory-safe, and
caught by the typl check when one is emitted.

**`from_trusted` is deleted.** Every path is then covered safely:

    you encoded the bytes              encode -> Ref -> decode
    trusted producer, Inline           load()
    trusted producer, variable         verify (off the frame path)
    untrusted producer                 verify

What this transfers rather than removes is one claim: `encode -> Ref` is sound
only if the encoder is correct. That is the same class of argument as trusting
`Vec::push`, and unlike most it is mechanically dischargeable — the emitter
writes the test, and `encode ⊆ verify` is the property:

    #[test] fn round_trip_verifies() {
        for v in Speed::sample_values() {            // from the typl bounds
            let mut buf = [0u8; Speed::MAX_SIZE];
            let r = v.encode(&mut buf).unwrap();
            assert!(Speed::verify(r.bytes()).is_ok());
            assert_eq!(Speed::decode(r), v);
        }
    }

    RA-08  MUST   `Ref<T, E>` has a private field, no public constructor
                  and no `unsafe` constructor. `verify` and `encode` are
                  its only sources, and no API takes a boolean deciding
                  whether to verify.
    RA-09  MUST   `verify` is one pass over a private copy, allocates
                  nothing, reads no clock, and checks structure and typl
                  constraints together. Structure is the encoding
                  runtime's where it ships a verifier; ridl emits only
                  the contract checks.
    RA-10  MUST   `MAX_SIZE` on FlatBuffers equals the slot capacity the
                  store projection assigned, and `SIZE` equals it too
                  when the type is `Inline`. The emitter asserts it; a
                  runtime checks it at attach.
    RA-21  MUST   Generated packages contain no `unsafe`. The unchecked
                  read path is `Inline::load` only; a variable-length
                  payload is verified regardless of trust.
    RA-22  MUST   Every generated codec emits the round-trip property
                  test. It is what makes `encode`'s safe constructor
                  honest.
    RA-23  MUST   `Encoding::FORMAT` is the only definition of an
                  encoding's wire tag.

## 5. Interaction descriptors — what the emitter writes per member

One trait per kind, implemented by a zero-sized marker type the emitter writes
per declared interaction. Everything in them is a `const`, so a binding generic
over a marker monomorphises to the same instructions a hand-written accessor
would.

    pub trait Interaction {
        const ORDINAL: Ordinal;
        const KIND: Kind;
        const FAMILY: Family;
        const NAME: &'static str;
    }
    pub trait Signal: Interaction {
        type Payload: Encode<Memory>;
        /// §4.3, §9: debounce and refresh ceiling, from `@[min..max]` or the
        /// configurable default. Never 0: an untimed signal gets the default.
        const MIN: Duration;
        const MAX: Duration;
        /// §4.4: the channel-level init, layered over the type's.
        fn init() -> Self::Payload;
    }
    pub trait Event: Interaction {
        type Payload: Encode<Memory>;
        const MIN: Duration;            // throttle on the provider
        const TTL: Duration;            // §5.2
    }
    pub trait Command: Interaction {
        type Args: Encode<Memory>;
        const MIN: Duration;            // call throttle on the caller (§9.3)
        const MAX: Duration;            // response bound ON ACCEPTANCE
        fn require(args: &Self::Args) -> Result<(), Violation>;   // §13, or Ok
    }
    pub trait Query: Interaction {
        type Args: Encode<Memory>;
        /// `T`, a named tuple, or `Result<T, E>` for an inline `T | E` (§10.1).
        type Reply: Encode<Memory>;
        const MIN: Duration;
        const MAX: Duration;            // response bound on the reply
        fn require(args: &Self::Args) -> Result<(), Violation>;
        fn ensure(args: &Self::Args, reply: &Self::Reply) -> Result<(), Violation>;
    }
    pub trait Fixed: Interaction {
        type Payload: Encode<Memory>;
    }

    pub trait Interface {
        const ID: InterfaceId;
        const NAME: &'static str;
        /// Over the IR of this interface: what a peer compares to detect
        /// UNKNOWN_INTERACTION (§10.2) and what a store header records.
        const HASH: u64;
        /// ADR-0018 decision 8: one region per provided interface; coherence derived by the emitter.
        const COHERENT: bool;
        const MEMBERS: &'static [Member];
    }
    pub struct Member { pub ordinal: Ordinal, pub kind: Kind, pub name: &'static str,
                        pub max_size: u32, pub min: Duration, pub max: Duration }

`require` and `ensure` are emitted from the contract terms (§13) as plain
functions over decoded values, so a provider binding evaluates them before and
after application code (§6.2, §10.2) and a caller binding may evaluate `require`
before sending. They live on the descriptor rather than on a separate trait so a
dispatcher generic over `C: Command` has them.

Streams (§12) are deferred: a `Stream<T>` parameter or reply needs a port of its
own with flow control, and nothing in the first consumer needs one.

    RA-11  MUST   Every timing const comes from the IR. A descriptor with
                  a bound the source did not declare and the default did
                  not supply does not exist.
    RA-12  MUST   `require`/`ensure` are on the descriptor and are pure.
                  A binding evaluates them; application code never does.

## 6. Ports — what an implementation provides

The contract's five kinds become seven ports, because a signal has a reading
side and a writing side, an event a source and a sink, and a call a caller and a
handler, while `fixed` has only a reader. Every port is **pulled**: a method
returns at once with a value, a correlation or `None`, and something outside
this crate — the runtime's loop, a blocking face, an async face — decides how to
wait. That is ADR-0018 decision 2 at the trait layer.

Ports carry **ordinals and bytes**, never payload types: a runtime routes and
stores; it never resolves a type. The generated binding is what turns a
`Sample<&[u8]>` into a `Sample<Speed>`, and it is the only place that knows
`Speed` exists.

    /// The clock the envelope is stamped from (RA-05). One per runtime.
    pub trait Clock { fn now(&self) -> Timestamp; }

    /// Signals, consumer side. A read is a snapshot of a slot: copy out under
    /// the runtime's discipline (a seqlock, a double buffer — its business),
    /// clamp to the slot capacity, never to the length read.
    pub trait SignalReader {
        /// Copies the slot into `out`; returns provenance, envelope and length.
        /// `Err(Short)` reports the capacity so a caller sizes once.
        /// No `now`: provenance comes from the slot header and the
        /// envelope from the slot. Freshness is computed above, from
        /// `envelope.stamp` and the descriptor's `MAX` (RA-28).
        fn read(&self, iface: InterfaceId, ord: Ordinal, out: &mut [u8])
            -> Result<RawSample, ReadError>;
        // RawSample { provenance, cause, freshness, envelope, len } — the
        // runtime resolves the timing, not the binding (RA-33).
        /// Interface generation for a coherent read (ADR-0018 decision 8): read gen,
        /// read slots, read gen, retry on change. Only meaningful when the
        /// interface declares `COHERENT`.
        fn generation(&self, iface: InterfaceId) -> u64;
        /// Everything that moved past `marks`, ordinals only; updates `marks`.
        fn scan(&self, marks: &mut [Watermark], out: &mut [Changed]) -> usize;
    }
    pub struct RawSample { pub provenance: Provenance, pub cause: Option<Cause>,
                           pub envelope: Envelope, pub len: usize }
    pub struct Watermark { pub iface: InterfaceId, pub gen: u64, pub seq: u64 }
    pub struct Changed   { pub iface: InterfaceId, pub ord: Ordinal, pub seq: u64 }
    pub enum ReadError { Short { needed: usize }, UnknownInteraction, Detached }

    /// Signals, provider side. `set` stages a value; `invalidate` stages the
    /// §4.5 transition; `touch` re-affirms without a value (the provider's side
    /// of `MAX`); `commit` publishes everything staged, per interface under
    /// one generation increment and one envelope stamp, and wakes
    /// consumers once for the whole batch. `commit` cannot fail: a
    /// last-value store has no queue to overflow. A provider that owns
    /// several interfaces uses one writer; the generated `Publisher` is
    /// per interface and passes its id.
    pub trait SignalWriter {
        fn set(&mut self, iface: InterfaceId, ord: Ordinal, bytes: &[u8]) -> Result<(), WriteError>;
        fn invalidate(&mut self, iface: InterfaceId, ord: Ordinal, cause: Cause);
        fn touch(&mut self, iface: InterfaceId, ord: Ordinal);
        fn commit(&mut self, now: Timestamp);
    }
    pub enum WriteError { TooLarge { cap: usize }, NotOwner, Detached }

    /// Events, consumer side. Occurrences are pulled in order; a gap in `seq`
    /// is a loss and the only evidence of one (§3.1). Never coalesced.
    pub trait EventSource {
        fn subscribe(&mut self, iface: InterfaceId, ords: &[Ordinal]) -> Result<(), SubscribeError>;
        fn unsubscribe(&mut self, iface: InterfaceId, ords: &[Ordinal]);
        /// The next occurrence, copied into `out`. `None` when the queue is empty.
        fn next(&mut self, out: &mut [u8]) -> Option<Occurrence>;
    }
    pub struct Occurrence { pub iface: InterfaceId, pub ord: Ordinal, pub envelope: Envelope, pub len: usize }

    /// Events, provider side. A real call with backpressure: `Busy` is the
    /// one retryable error, because a queued occurrence has somewhere to
    /// overflow.
    pub trait EventSink {
        fn raise(&mut self, iface: InterfaceId, ord: Ordinal, bytes: &[u8]) -> Result<(), RaiseError>;
    }
    pub enum RaiseError { Busy, TooLarge { cap: usize }, NotOwner, Detached }

    /// Calls, consumer side. `command` and `query` are separate because their
    /// semantics diverge on identical fields (§6, §7): what the reply means,
    /// what `MAX` bounds, and what an unknown outcome permits. Both return a
    /// correlation the runtime minted; the outcome is polled, never pushed.
    pub trait Caller {
        fn command(&mut self, iface: InterfaceId, ord: Ordinal, args: &[u8]) -> Result<Correlation, CallError>;
        fn query  (&mut self, iface: InterfaceId, ord: Ordinal, args: &[u8]) -> Result<Correlation, CallError>;
        /// §6.1: the delivery acknowledgment, visible to the runtime and to a
        /// caller that chooses to supervise it. Never a contract value.
        fn ack(&mut self, c: Correlation) -> Option<Ack>;
        /// §7: the reply, copied into `out`, or the Stratum 2/3 failure.
        fn reply(&mut self, c: Correlation, out: &mut [u8]) -> Option<Result<usize, CallError>>;
        fn forget(&mut self, c: Correlation);
    }
    pub struct Correlation(pub u64);
    pub enum Ack { Accepted, Rejected(Contract), Undelivered(Transport) }

    /// Calls, provider side. One claim in flight per interface; every claim is
    /// settled, including rejections — silence is a lapse and a lapse tells
    /// the caller `Undelivered`.
    pub trait Handler {
        fn serve(&mut self, iface: InterfaceId, ords: &[Ordinal]) -> Result<(), ServeError>;
        fn next_claim(&mut self, out: &mut [u8]) -> Option<Claim>;
        fn settle(&mut self, claim: ClaimId, outcome: Outcome, reply: &[u8]) -> Result<(), SettleError>;
    }
    pub struct Claim { pub id: ClaimId, pub iface: InterfaceId, pub ord: Ordinal, pub kind: Kind,
                       pub envelope: Envelope, pub remaining: Duration, pub len: usize }
    pub struct ClaimId(pub u64);
    pub enum Outcome { Ok, Rejected(Contract), Failed }

    /// `fixed`, reader only (§8): a plain accessor populated at binding
    /// initialisation. Safe to cache unconditionally.
    pub trait FixedReader {
        fn read(&self, iface: InterfaceId, ord: Ordinal) -> Result<&[u8], ReadError>;
    }

The three strata of §10 appear exactly once, here, and every port's error is
built from them:

    /// Stratum 2 — contract errors, derived, never declared (§10.2).
    pub enum Contract { InvalidValue(Violation), PreconditionFailed, ContractBroken, UnknownInteraction }
    /// Stratum 3 — infrastructure failure, detected, undeclared (§10.3).
    /// Supplied by the runtime, never by the source.
    pub enum Transport { Timeout, Undelivered, Down, Busy, Detached }
    pub enum CallError { Contract(Contract), Transport(Transport) }

    RA-13  MUST   Ports carry `InterfaceId`, `Ordinal` and bytes. No port
                  method names a payload type; the binding resolves types.
    RA-14  MUST   Every port method returns without waiting. Waiting is a
                  face over a port — blocking, async, or a driving loop —
                  and no face is defined in this crate.
    RA-15  MUST   `command` and `query` are separate methods on `Caller`
                  and separate claims on `Handler`; a runtime never
                  collapses them into one call with a kind flag.
    RA-16  MUST   `SignalWriter::commit` has no error return. Backpressure
                  exists on `EventSink` and `Caller` only.
    RA-17  MUST   The delivery acknowledgment (§6.1) reaches the caller
                  only through `Caller::ack`, never as a return value of
                  a generated command method.
    RA-35  MUST   A port method expresses an interaction semantic that
                  every implementation must present, never a mechanism
                  one implementation happens to use. `scan` and
                  `generation` fail that test as written (§6.1) and are
                  an extension, not the core.
    RA-33  MUST   The runtime resolves provenance AND freshness and
                  returns both from a read. It already resolves the
                  member descriptor to find the slot, already holds the
                  `Clock` and already reads the slot header, so the
                  comparison costs nothing there and would otherwise be
                  written once per language binding.
    RA-34  MUST   A generated accessor does three things and no more:
                  decode, evaluate the typl constraints, substitute the
                  declared fallback. Everything timing-related, and
                  everything about the slot's own state, is the
                  runtime's.
    RA-28  MUST   No port and no generated accessor takes `now`. The
                  runtime holds the `Clock` port, whose implementation
                  knows the platform time base; asking a caller to
                  supply a `Timestamp` is where a value from the wrong
                  clock domain enters. A caller wanting several reads
                  pinned to one instant takes a snapshot, which is the
                  object a `coherent` interface needs anyway.

### 6.1 Two of these are not language semantics

With the engine descoped, the ports have to be re-read with one question:
**would every implementation have this, or only a mapped-store one?**

`read`, `raise`, `subscribe`, `command`, `query`, `serve`, `settle` and the
`fixed` accessor are interaction semantics — ridl §4.4's never-empty channel,
§4.5's invalid state, §6.1's acknowledgment, §7's mandatory reply. A browser
shim, an rmdl host and a shared-memory cockpit all must present them, however
differently they are built.

Two are not:

    SignalReader::generation(iface) -> u64      presumes a per-interface
                                                counter in a shared region
    SignalReader::scan(marks, out)              presumes watermarks over
                                                slots a consumer can walk

Both come from one implementation's coherence and change-detection mechanism. A
browser runtime has neither, and would have to fake them. They are useful — the
first consumer's frame path is built on `scan` — so the answer is not to delete
them but to place them honestly:

    /// Extension: implementations whose signals live in a walkable store.
    /// Not part of the interaction semantics; a runtime may omit it.
    pub trait ScannableSignals: SignalReader {
        fn generation(&self, iface: InterfaceId) -> u64;
        fn scan(&self, marks: &mut [Watermark], out: &mut [Changed]) -> usize;
    }

A generated client bounds on `SignalReader`; a generated _frame-path_ client,
where rsdl says the consumer is periodic, bounds on `ScannableSignals` and gets
`scan`. The bound is then a statement about what the deployment provides rather
than an assumption every runtime must satisfy.

The general form of the test, applied to anything added later: **if a port
method could not be implemented by a runtime with no shared memory, it is an
extension.** That is the line the descope draws, expressed as something
checkable.

## 7. Trust: verify by default, skip by measurement

ADR-0018 decision 10 derives read verification from the trust relationship,
decided at generation time. The knob stays; its default inverts.

    /// Emitted per (interaction, reading machine). No runtime flag, no
    /// branch, no caller judgment.
    pub trait Access<S: Signal> { const CHECKED: bool; }

**The cost of always checking is small and mostly does not run.** flatc's
verifier is O(fields), not O(bytes) — a vector of 250 structs verifies in O(1),
because structs are inline and offset-free. The expensive shape is a wide table:
250 fields is ~250 vtable slots, order 0.5–1 µs, against a 2.5 KB copy the
reader pays anyway. And a reader that scans watermarks first skips both the copy
and the check for every cell that did not move — which, for variable-length
payloads changing at human rates, is nearly all of them.

**What always-checking removes is worth more than the microseconds.** No
`unsafe` (§4.2). No verifier gap in languages whose FlatBuffers runtime ships
none (§9). And no checked/unchecked matrix — the per-deployment build artifact
enumerating interaction × reading machine × emitted checks, which a person signs
off every time a service moves machines between stages. That is a recurring
review traded against a fraction of a frame.

So `CHECKED` is `true` everywhere until a measurement on target hardware says
otherwise for a named member. Because it is a generation-time const, flipping it
later regenerates a crate and changes no layout, no ordinal and no hash — which
is exactly why it is a const and not a runtime flag.

    RA-18  MUST   Whether an accessor verifies is a generation-time
                  const, never a runtime flag or a caller's argument.
    RA-24  MUST   `Access::CHECKED` is `true` for every member until a
                  measurement justifies otherwise for that member. A
                  trust level alone does not justify it.
    RA-25  SHOULD Where the matrix is emitted at all, it is a build
                  artifact so the delta between deployments is a diff
                  someone signs rather than a consequence nobody looked
                  at.

## 8. What the emitter produces, per interface

With the descriptors and ports above, phase 2's face is mechanical. For

    interface Drivetrain {
      signal  speed : Speed @[20ms..500ms]
      signal  gear  : Gear
      event   shiftDone : ShiftInfo @[50ms..500ms]
      command setGear(g: Gear) @[..50ms] [ require g != Gear.PARK || speed < 3 ]
      query   averageSpeed(window: Duration): Speed @[..200ms]
    }

the Rust emitter writes, in one file per package:

    pub mod drivetrain {
        pub struct Iface;              impl Interface for Iface { .. }
        pub struct Speed_;             impl Signal  for Speed_  { type Payload = Speed; .. }
        pub struct Gear_;              impl Signal  for Gear_   { .. }
        pub struct ShiftDone_;         impl Event   for ShiftDone_ { .. }
        pub struct SetGear_;           impl Command for SetGear_ { type Args = SetGearArgs; .. }
        pub struct AverageSpeed_;      impl Query   for AverageSpeed_ { .. }

        /// Consumer face: generic over the ports it needs and nothing else.
        pub struct Client<'a, P: SignalReader + EventSource + Caller> { p: &'a P, buf: .. }
        impl<'a, P: ..> Client<'a, P> {
            pub fn speed(&self, now: Timestamp) -> Sample<Speed>;          // read + verify|trust + decode
            pub fn gear(&self, now: Timestamp) -> Sample<Gear>;
            pub fn next_shift_done(&mut self) -> Option<(ShiftInfo, Envelope)>;
            pub fn set_gear(&mut self, g: Gear) -> Result<Correlation, CallError>;   // require, encode, Caller::command
            pub fn average_speed(&mut self, window: Duration) -> Result<Correlation, CallError>;
            pub fn average_speed_reply(&mut self, c: Correlation) -> Option<Result<Speed, CallError>>;
        }

        /// Provider face: what the application implements. Contract-violating
        /// input never reaches it (§6.2); `ensure` is checked on its way out.
        pub trait Provider {
            fn set_gear(&mut self, g: Gear) -> Result<(), Rejected>;
            fn average_speed(&mut self, window: Duration) -> Speed;
        }
        /// Routes claims by ordinal, verifies args, evaluates require/ensure,
        /// settles. Generic over the handler port.
        pub fn dispatch<H: Handler, P: Provider>(h: &mut H, p: &mut P, buf: &mut [u8]) -> usize;

        /// Publisher face, provider side.
        pub struct Publisher<'a, W: SignalWriter + EventSink> { w: &'a mut W }
        impl<'a, W: ..> Publisher<'a, W> {
            pub fn speed(&mut self, v: Speed);          // check, encode, SignalWriter::set
            pub fn gear(&mut self, v: Gear);
            pub fn invalidate_speed(&mut self, cause: Cause);
            pub fn shift_done(&mut self, e: ShiftInfo) -> Result<(), RaiseError>;
            pub fn commit(&mut self, now: Timestamp);
        }
    }

Blocking and async faces are wrappers a runtime crate ships over the same
`Correlation` (a condvar or a oneshot per correlation); the generated code never
contains one (RA-14). Kotlin and TypeScript emitters produce the same faces over
the same ports rendered in their language, with the codec reached as ADR-0018
decision 6 says.

    RA-19  MUST   A generated client is generic over exactly the ports its
                  interface needs, named as trait bounds; a generated
                  provider is a trait the application implements plus a
                  `dispatch` over `Handler`.
    RA-20  MUST   Generated code contains no thread, future, socket or
                  timer. Faces are the runtime's.

## 9. Kotlin, and an amendment to decision 6

ADR-0018 decision 6 says TypeScript and Kotlin get generated types and
interfaces only, and that the codec is the Rust one reached via wasm and JNI
respectively. **That cannot hold for a mapped store.** A signal read at frame
rate is a load from a mapped region; a JNI transition per read is two orders of
magnitude more expensive than the read it wraps, and the reason is not specific
to any one platform — it is true for any Kotlin consumer of any ridl store.

So decision 6 wants narrowing: **JNI where the payload path is framed and off
the frame budget; a native codec where the store is mapped.** And ridl owes a
**Kotlin codec emitter**, not only a types emitter. That is the real cost of the
Kotlin story and belongs on the roadmap explicitly rather than being discovered
by the first consumer.

Four things the language decides rather than the design:

**Value classes wherever Rust emits a newtype** — `Timestamp`, `Duration`,
`Ordinal`, `Correlation`, and every named typl scalar. Same erasure to a
primitive, same type safety, and the unit lives in the type rather than as a
suffix on the identifier (`now: Timestamp`, not `nowUs: Long`).

**Descriptors stay raw.** A value-class property reached through an interface
boxes, because the override's signature is the boxed one. So a descriptor is a
plain object of `const val` primitives, folded at every use site.

**`Sample<T>` boxes**, because generics box value classes. One read is nothing;
a few hundred signals at frame rate is thousands of small allocations a second,
and a young-generation collection mid-frame is exactly the stutter a real-time
consumer cannot have. So where a consumer reads per frame the emitter writes a
**non-generic sample per payload type**. Which shape it emits is a
per-deployment fact, not a language one.

**There is no Kotlin verifier.** The Java/Kotlin FlatBuffers runtime ships
readers and a builder and no `Verifier`, so a checked read of a _table_ is not
slow in Kotlin — it is unavailable. Three ways out, in order: model the payload
as a bounded fixed array so it becomes `Inline` and needs none; route it as an
event or query so it is decoded once on arrival rather than per frame; or emit a
Kotlin structural verifier for that payload alone, which is ridl writing what
§4.1 says it should not have to.

**The consequence for §7 is real.** RA-24 makes `CHECKED` true everywhere, and
in Kotlin that is only satisfiable for `Inline` payloads. So a deployment with a
Kotlin consumer must either keep that consumer's read set `Inline` or accept an
emitted verifier for the exceptions — and that is a question to answer before a
layout is generated, not after.

    RA-29  MUST   A language whose runtime cannot verify a payload's
                  encoding may read that payload only where verification
                  is unnecessary — which for FlatBuffers means `Inline`
                  — unless ridl emits a verifier for it.
    RA-30  MUST   No language binding crosses a foreign-function
                  boundary on a mapped read path.
    RA-31  MUST   A binding emits its language's zero-cost wrapper
                  wherever Rust emits a newtype, and raw constants
                  inside descriptors.
    RA-32  MUST   Every binding's accessor has the same branches in the
                  same order, so a differential test compares field for
                  field rather than being two suites that can both pass
                  while disagreeing.

## 10. What this changes for the first consumer

The first runtime sketched its client before these traits existed. Adopting them
is five renames and two deletions, recorded in the consumer's own payload
record:

    Validity (6 values)          -> Provenance + Cause + Freshness (RA-06/07)
    Fixed (the codec trait)      -> Inline; `Fixed` is the kind
    Kind::Surface = 5            -> gone; a surface record is a signal
                                    carrying a domain attribute (RA-04)
    Instant (ms, monotonic)      -> kept inside the loop; the envelope
                                    stamp is Timestamp (µs, PTP) from the
                                    platform clock (RA-05)
    Writer::invalidate           -> restored: ridl §4.5 is normative
    `now` on reads and accessors -> deleted (RA-28)
    `from_trusted`               -> deleted (RA-21)

Its structured binder contract is one binding of these ports —
`command(iface, ordinal, args)` is `Caller::command` over IPC — which is what
makes its Kotlin client a second implementation of the same traits and the
differential test of RA-32 meaningful.

## 11. Open

**RA-X1 — streams.** ridl §12's `<T>` parameters and replies need a
flow-controlled port. Not needed by the first consumer; must not be bolted onto
`Caller`.

**RA-X2 — `Family` on the ports.** Descriptors carry it; no port reads it. The
§3.3 obligations are the emitter's and the test plane's, so this is probably
right — but a presentation runtime may want to know.

**RA-X4 — `Cause` beyond `Declared`.** `OutOfRange`, `Corrupt` and `Implausible`
are consumer-side detections and arguably not §4.5's channel state at all. They
are here because a consumer needs one door out; whether they belong inside
`Provenance::Invalid` or in a fourth field is a question for §4.5 rather than
for this crate.

**RA-X5 — roadmap placement.** With §0's descope, Epic 11 is this library plus
the two codecs plus the deployment schema. This library is E11.10 promoted and
renamed, and it blocks the codecs rather than the reverse. The Kotlin emitter of
§9 is a new story. E11.1 to E11.5 and E11.9 need a home in an rmdl-era epic
rather than deletion — the reasoning in them is good, it is only the ownership
that was wrong.

**RA-X10 — what an ADR-0018 amendment must say.** §0 lists which decisions
survive, leave and split, but the record itself still reads as though ridl ships
an engine. It is _Proposed_, so amending is cheap; leaving it is a document that
contradicts the roadmap it drives. The amendment should also retire the name
collision explicitly, so that a reader meeting `ridl-rt` in either document
means this library by it.

**RA-X6 — the FlatBuffers runtime version.** The no-alloc `encode` needs the
`Allocator` trait, added in the 24.x series. On a platform carrying an older
vendored copy, either the encode path allocates or the platform is uprevved —
the second has its own lead time. Settle before the emitter is written.

**RA-X7 — `Inline` versus typl §17.11's width flips.** Widening a range can flip
the underlying width. A vtable absorbs that; a packed struct cannot. So **a
change `ridl-diff` calls compatible is a layout break for an `Inline` payload.**
Either `Inline` is restricted to types whose width is pinned in the source, or
widening an `Inline` field is breaking and the diff must say so. This is
ADR-0018's deferred `repr(C)`-store row arriving by another road, and it is the
one open item here that can silently corrupt a deployment.

**RA-X8 — an `Inline` payload is not a valid root buffer.** A FlatBuffers struct
exists only as a field inside a table; `root::<Vec3>()` does not exist. So an
`Inline` slot holds a packed record using the format's struct layout rules
rather than a self-describing buffer, and no standard reader parses that slot as
a root. Fast, and fine — but it belongs in the slot contract rather than being
discovered by whoever writes the first trace tool.

**RA-X9 — where `Proto3` is actually used.** No known consumer encodes a payload
as proto3 today: the first consumer's own `.proto` carries opaque frames whose
payloads are FlatBuffers. If none appears, the second `Encoding` is a facility
the parameter of §4 exists to permit rather than one anything exercises, and
decision 4's byte-level conformance test has no local motive.
