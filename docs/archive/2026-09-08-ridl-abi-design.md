# `ridl-abi` — the traits generated code is written against

Status: working note, 2026-09-08. Proposes a crate that sits between ADR-0018's
layers 2 and 3, and answers the question that record's decision 15 left to phase
2: what shape the restored client and server face has, and what it is generic
over.

Scope: the trait vocabulary for interactions, payload encoding, verification and
provenance; what a runtime implements to serve it; what an emitter generates
against it. Not the runtime itself (Epic 11), not the codecs (E11.7, E11.8), not
the deployment schema (E11.6).

Nothing here is implemented. The first consumer is the first runtime, which is
implementing these ports against its own sans-IO core and will be the second
implementation ADR-0018's E11.1 exit criterion asks for.

## 1. Why a crate, and why this one

ADR-0018 found that the shipped interaction layer emitted its vocabulary once
per package — every package declared its own `Provenance`, its own
`SignalHandle` — so no runtime could ship `impl SignalHandle for …` for a trait
that exists once per package, and cross-package generic code was impossible.
Decision 15 retracted the layer and promised a phase 2 shaped as "a concrete
client generic over a transport and a server trait the provider implements".

That sentence names the missing thing. A client generic over a transport is
generic over **traits the transport implements**, and those traits have to be
defined exactly once, in a crate that both the emitter's output and the runtime
depend on, and that depends on neither. That crate is `ridl-abi`.

    layer 4   application               hand-written, calls generated code
    layer 3   generated bindings        client, provider trait, publisher,
                                        payload types, descriptors
                        │ written against
    ────────  ridl-abi  ────────────────────────────────────────────────
                        │ implemented by
    layer 2   ridl-rt · the first runtime · any runtime
    layer 1   Socket/Link · Region · Notify · Clock
    layer 0   the OS

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

    RA-01  MUST   `ridl-abi` depends on no other crate, is `no_std`, has
                  no alloc, no clock, no IO and no `async`.
    RA-02  MUST   Every trait here is defined exactly once. An emitter
                  never re-emits any of them; a runtime never redefines
                  them.
    RA-03  MUST   A runtime implements ports; generated code calls ports
                  and never a runtime type. The dependency graph is
                  emitter output → ridl-abi ← runtime, and nothing else.

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
is generated. The trait layer therefore has one codec trait parameterised by the
wire, and every generated payload type implements it for the wires its package
was emitted with.

    pub trait Wire { const NAME: &'static str; }
    pub struct Memory;      impl Wire for Memory  { .. }   // FlatBuffers: mapped, in place
    pub struct Network;     impl Wire for Network { .. }   // proto3: streamed

    /// A verified view. **Private constructor**: only `Encode::verify` or the
    /// `unsafe` escape hatch can make one, so `grep unsafe` is the audit of
    /// every skipped verification.
    pub struct Ref<'a, T, W: Wire> { buf: &'a [u8], _t: PhantomData<(T, W)> }
    impl<'a, T, W: Wire> Ref<'a, T, W> {
        /// Provenance not known at build time and trusted by policy (§7).
        /// # Safety: `buf` was produced by `Encode::<W>::encode` for `T` and is
        /// not mutated for `'a`.
        pub unsafe fn from_trusted(buf: &'a [u8]) -> Self;
        pub fn bytes(&self) -> &'a [u8];
    }

    pub trait Encode<W: Wire>: Sized {
        /// Largest encoding of any value of this type on this wire. The
        /// emitter asserts it equals the store's slot capacity for `Memory`.
        const MAX_SIZE: usize;
        fn encode(&self, out: &mut [u8]) -> Result<usize, EncodeError>;
        /// One pass. Structure AND typl constraints. No allocation, no clock,
        /// on a private copy — never on a live slot.
        fn verify(buf: &[u8]) -> Result<Ref<'_, Self, W>, VerifyError>;
        /// Infallible: the `Ref` is the proof.
        fn decode(r: Ref<'_, Self, W>) -> Self;
    }

    /// One-size encoding on the memory wire: the degenerate slot, min == max,
    /// no verify needed for structure. Named `Inline`, not `Fixed`, because
    /// `fixed` is an interaction kind (§8) and the two must not be confused.
    pub trait Inline: Encode<Memory> {
        const SIZE: usize;
        fn load(buf: &[u8; Self::SIZE]) -> Self;
        fn store(&self, out: &mut [u8; Self::SIZE]);
    }

    /// The typl contract of a value, checkable on a constructed value. Emitted
    /// with the checked constructor (Epic 10); what `verify` calls after the
    /// structural pass, and what a provider binding calls before a publish.
    pub trait Constrained { fn check(&self) -> Result<(), Violation>; }

    pub enum EncodeError { Short { needed: usize }, Invalid(Violation) }
    pub enum VerifyError { Truncated, Malformed, Invalid(Violation) }
    pub struct Violation { pub field: u16, pub rule: Rule }   // range | step | bounds | pattern | invariant

`verify` and `decode` are separate on purpose: verification happens once at a
named, auditable boundary; a verified buffer decodes many times under fan-out;
`verify` is a clean fuzz target (bytes in, `Result` out); and it matches
FlatBuffers' own `root` / `root_unchecked` rather than fighting it. The rule is
not "no unchecked decode". It is: **`Ref` is constructible only by `verify` or
by `unsafe`, and `verify` is called only at an ingress boundary.**

`Inline::load` is the frame-rate path: no verification of structure because
there is none to verify, and the typl `check` is emitted or not per the trust
rule (§7). Not every signal is `Inline` — a street name is a signal — which is
why `Encode<Memory>` exists for slots at all.

    RA-08  MUST   `Ref<T, W>` has a private constructor; `verify` and
                  `unsafe from_trusted` are the only two ways to obtain
                  one. No `verify: bool` parameter exists anywhere.
    RA-09  MUST   `verify` is one pass over a private copy, allocates
                  nothing, reads no clock, and checks structure and typl
                  constraints together.
    RA-10  MUST   The memory-wire `MAX_SIZE` of a payload type equals the
                  slot capacity the store projection assigned it (E11.2).
                  The emitter asserts it; a runtime checks it at attach.

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
        /// ADR-0018 §8: one region per provided interface; coherence declared.
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

## 6. Ports — what a runtime implements

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
        fn read(&self, iface: InterfaceId, ord: Ordinal, out: &mut [u8], now: Timestamp)
            -> Result<RawSample, ReadError>;
        /// Interface generation for a coherent read (ADR-0018 §8): read gen,
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

## 7. Trust, and where verification is emitted

ADR-0018 decision 10: write mode and read verification derive from the trust
relationship, decided at generation time. The trait layer gives it exactly one
knob, on the generated accessor:

    /// Emitted per (interaction, reading machine). `CHECKED` is true iff the
    /// producer's trust level is below the reader's in this deployment; the
    /// accessor then calls `verify`, otherwise `from_trusted`. No runtime
    /// flag, no branch, no caller judgment.
    pub trait Access<S: Signal> {
        const CHECKED: bool;
    }

An emitter also writes a checked/unchecked matrix per deployment (interaction ×
reading machine × which checks were generated) as a build artifact, so a service
that moves machines between stages produces a diff someone signs off rather than
a silent change.

    RA-18  MUST   Whether an accessor verifies is a generation-time const
                  derived from the deployment's trust levels, and the
                  matrix of those decisions is an emitted artifact.

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

## 9. What this changes for the first consumer

The first runtime sketched its client before these traits existed. Adopting them
costs five renames and one addition, all recorded in the consumer's own API
record §3:

    Validity (6 values)          -> Provenance + Cause + Freshness (RA-07)
    Fixed (trait)                -> Inline; `Fixed` is the kind
    Kind::Surface = 5            -> gone; surface record = signal + attribute (RA-04)
    Instant (ms, u64)            -> the loop keeps ms internally; the envelope
                                    stamp is Timestamp (µs, i64) from the
                                    frame clock (RA-05)
    Writer::invalidate           -> restored: §4.5 is normative
    Client::{Reader,Writer,..}   -> impl SignalReader / SignalWriter /
                                    EventSource / EventSink / Caller /
                                    Handler / FixedReader / Clock

Its structured binder contract is one binding of these ports —
`command(iface, member, args)` is `Caller::command` over IPC — which is what
makes the Kotlin client a second implementation of the same seven traits.

## 10. Open

**RA-X1 — streams.** §12's `<T>` parameters and replies need a flow-controlled
port. Not needed by the first consumer; must not be bolted onto `Caller`.

**RA-X2 — `Family` on the ports.** Descriptors carry it; no port reads it. The
obligations of §3.3 are the test plane's and the emitter's, so this is probably
right, but a presentation runtime may want to know.

**RA-X3 — where `Freshness` is computed.** The reader port returns the envelope;
the generated accessor computes staleness from `Signal::MAX` and `now`. That
puts the comparison in generated code (~30 lines, safety-relevant) rather than
in the runtime. The alternative — the runtime computes it from the descriptor
table it already loaded — moves it to one place per runtime.

**RA-X4 — `Cause` beyond `Declared`.** `OutOfRange`, `Corrupt`, `Implausible`
are consumer-side detections and arguably not §4.5's channel state at all. They
are here because the consumer needs one door out; whether they belong in
`Provenance::Invalid` or in a fourth field is a spec question for §4.5.

**RA-X5 — roadmap placement.** This is most of E11.10's "component binding
contract" and a prerequisite of E11.4, E11.7 and E11.8. Either E11.10 moves
first and grows, or this is a new story E11.0. The dependency is real either
way.
