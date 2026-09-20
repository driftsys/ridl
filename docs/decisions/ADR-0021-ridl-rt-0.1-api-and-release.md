# ADR-0021 — The `ridl-rt` 0.1 API: identity, the proof type, the port dispositions, and the release policy

## Status

Accepted — 2026-09-14. Scope: the decisions that fixed `ridl-rt` 0.1.0's public
API where the earlier records left a choice open — the identity types, how a
port binds to a catalog, what a failed `require`/`ensure` clause carries, the
duplicate-suppression and invalid-event-payload dispositions of
driftsys/ridl#308 and #309, the proof type that keeps decoding safe with no
`unsafe`, the cargo features, the exhaustiveness split of the error enums, and
the release and versioning policy. It binds every consumer of `ridl-rt`: the
Rust codegen (roadmap epic E11's later stories), the two runtimes
(`ridl-loopback`, `ridl-transport-ws`, story E11.9), and the ridl reference
finalization pass (story E14.2).

Written from lane A of the 2026-09-13 step-1 coordination (driftsys/ridl#328),
which built `crates/ridl-rt`. The reasoning trail, including the "Alternatives
considered" table below, is
[`docs/archive/2026-09-13-ridl-rt-v0.1-design.md`](../archive/2026-09-13-ridl-rt-v0.1-design.md)
(sections R-1 to R-12); the crate's architecture — its six modules and their
full type and trait surface, as built — is
[the `ridl-rt` design record](../design/ridl-rt.md).

Sebastien approved the spec these decisions come from before it merged
(driftsys/ridl#332), and decided decision 4 himself during the review of
driftsys/ridl#348. The API revision driftsys/ridl#351 settled decision 9
(`#[non_exhaustive]` on `Transport` and the seven port error enums) and the
`CallError` argument of `Handler::settle`, both decided by Sebastien. Where the
lane's driver first took a choice under his delegation, the decision's own text
says so.

It does not restate
[ADR-0020](ADR-0020-third-encoding-runtime-layering-and-plugin-system.md)
decision 5, which already fixes the crate's module list and its rename of
`strata` to `error`, nor [ADR-0007](ADR-0007-e1-execution.md) decision 14, which
fixes the crate's version number, its tag, and that creating the tag and running
`cargo publish` are maintainer acts. This record adds the policy those mechanics
did not need to state: what counts as a breaking 0.x change.

**Amendment (2026-09-20) — decisions 11 and 12.** An assessment of the generated
interaction face and the ports, made on 2026-09-20 over `main` at 2bcbab8, found
two gaps this record had left: nothing except a runtime's own type implements a
port trait, so a face can be built over a runtime's port and over nothing else;
and nothing stated whether a port may be sent to another thread or shared
between threads, so the answer would have been fixed by whichever concrete type
story E11.9 shipped. Decisions 11 and 12 answer them. Sebastien took both on
2026-09-20; the working note they come from and his disposition of it are on
driftsys/ridl#429, as its items D-2 and D-3, and decision 12's aggregate handle
is a second amendment he took there the same day. Decision 12 changes no API in
this crate; decision 11's impls land in the `ridl-rt` change that follows this
record, which also gives the crate-level rustdoc its matching paragraph. Until
that change merges, this record describes impls `crates/ridl-rt/src/port.rs`
does not yet contain.

## Context

`ridl-rt` 0.1 is the first crate a generated ridl package links and a runtime
implements. Its earlier records fixed the shape of the problem — six modules,
three payload encodings, ports over bytes rather than payload types — but left
several questions for the crate that actually had to compile: what identifies an
interface once the service slot model is gone (driftsys/ridl's identity studies
retired `ServiceId`), whether a `require` or `ensure` failure carries a value
nothing yet has a type for, how two open issues about delivery semantics resolve
for a 0.1 that ships before the engine does, and how a decoded value can be
trusted with no `unsafe` and no second verification pass.

## Decision

1. **An interface number is scoped by its catalog, not by a service, and is
   named `InterfaceNo`.** The note this crate refines called it `InterfaceId`
   and scoped it "per service" — a scope the identity studies retired. Under the
   catalog model, the value is a number: frozen from the lock file's `next`
   counter, or provisional and ordered after the frozen ones deterministically.
   `ServiceId` is dropped — nothing produces it, because a service has no number
   of its own — and there is no catalog slot type: which connection a catalog is
   attached to is assigned outside generated code. A provisional number is
   marked on the interface's descriptor (`Interface::PROVISIONAL`), not on the
   type, because it routes exactly like a frozen one and a flag bit would take
   width the next decision fixes.

2. **`Ordinal` and `InterfaceNo` are `u32`; `CatalogHash` is `[u8; 32]`,
   SHA-256.** The first is gate GW's width, which lane L posted on
   driftsys/ridl#328 and Sebastien approved on 2026-09-13; the IR already
   carries an ordinal as `uint32`. The second is taken from the catalog
   descriptor plan (driftsys/ridl#324), decided by the driver under Sebastien's
   delegation, because that plan is the record that computes the hash and a
   generated `Interface::CATALOG` and a catalog descriptor must carry the same
   bytes. **Fallback, recorded for E11.1:** if the frame specification finds
   that a frame header must fit in 8 bytes or fewer, or must carry these numbers
   in an existing 16-bit transport field, `Ordinal` and `InterfaceNo` become
   `u16` with a range check at lowering, while the IR and the descriptor keep
   `uint32`; that change ships as a 0.x minor release under decision 10 below.
   `CatalogRef` equality compares the name and the hash together, because the
   hash is computed over the interfaces, their numbers and the types they reach,
   and may not cover the package name.

3. **A port is bound to one catalog, checked once at construction.** Every
   interaction port has the supertrait `Attached`, and port methods address an
   interaction by `InterfaceNo` and `Ordinal` scoped to that one catalog. A
   generated client compares `port.catalog()` against its interface's `CATALOG`
   once, when it is built, rather than carrying the full
   `(catalog, interface, ordinal)` key and checking it on every call. A provider
   that spans two catalogs holds one port set per catalog; nothing atomic is
   lost, because generation is already per interface.

4. **A failed `require` or `ensure` clause carries no value.** Both methods
   return `Result<(), ()>` (`#[allow(clippy::result_unit_err)]`, because the
   omission is deliberate): the method that fails already decides the contract
   error a provider reports — `Contract::PreconditionFailed` for `require`,
   `Contract::ContractBroken` for `ensure` — and `Handler::settle` takes both
   without a payload. typl v0.1 has no invariant constraint, so a `Violation`
   naming a declared constraint would name one that does not exist, and nothing
   would read it.

5. **driftsys/ridl#308: duplicate suppression is below the port.** `Envelope`
   keeps exactly the sender's timestamp and sequence number (ridl §3.1). A
   runtime keys duplicate suppression on the caller's transport identity plus
   `seq`, not on a field this crate adds to `Envelope`: `Handler::next_claim`
   presents each delivered call once, a retransmission of an already-presented
   call receives the cached acknowledgment rather than being presented again,
   two callers are never merged even under the same `seq`, and `ClaimId` is the
   key unique per channel. On a claim, `envelope.seq` is unique per caller, not
   per channel. The ridl reference finalization pass (story E14.2) receives the
   corrected sentence: on a call, the sequence number is scoped per (caller
   instance, channel), and duplicate suppression keys on the caller's identity
   plus the sequence number. Before a signal's first publication, its envelope
   reads `seq` 0, stamped when the channel is created — the convention E14.2
   confirms rather than changes.

6. **driftsys/ridl#309: an event that fails its check is delivered with an
   invalid marker, not withheld.** `EventSource::next` stays unvalidated — it
   returns bytes, and the generated binding runs the check — so the typed face
   returns `sample::Occurrence<T>`, whose `payload` field is `Err(Detection)`
   when the check fails rather than silently dropping the occurrence (ridl
   §10.3, §10.4: nothing fails silently) or holding it back in a ninth port this
   crate does not add. If story E14.2 chooses quarantine instead, the `Err` arm
   is never produced and nothing built against 0.1 breaks. E14.2 receives: an
   event payload that violates its typl constraints is delivered to the
   subscriber with an invalid marker and its envelope.

7. **A payload is decoded only from a proof, and the set of encodings is
   closed.** `payload::Ref` has private fields; the only functions that build
   one are the crate's own `Ref::verify` and `Ref::encode`, which call the
   generated `Payload::verify`/`Payload::encode` and wrap the result. So
   `Payload::decode` is reachable only with a proof that the bytes were checked
   or that the crate's own encoder wrote them, and code ridl emits needs no
   `unsafe` to decode (a flatc-generated accessor used as a `View` may still
   contain `unsafe` inside the accessor itself, which narrows the no-`unsafe`
   rule to code ridl emits, not to every `View`). `encoding::Encoding` is
   sealed, so `FlatBuffers`, `Proto3` and `ReprC` are the only encodings a
   payload can implement; adding a fourth is a decision recorded in an ADR,
   never an `impl` in a downstream crate.

8. **The three cargo features are declared and carry no dependency in 0.1.**
   `flatbuffers`, `proto3` and `repr-c` exist so a runtime can name every
   encoding without linking a codec, and the crate has no dependency in any
   feature combination. The `flatbuffers` dependency, its version, and the
   obligation it brings — every final binary that enables the feature must
   provide a global allocator, because the `flatbuffers` crate uses `alloc` even
   without `std` — arrive with story E11.7 as an additive change.

9. **`Contract` and `CallError` stay exhaustive; every other error enum stays
   `#[non_exhaustive]`.** `Contract`'s variants are ridl §10.2's fixed
   categories and `CallError` composes `Contract` with `Transport`, so a new
   variant in either is a language change, not something a runtime adds.
   `Transport` and the seven port error enums (`ReadError`, `WriteError`,
   `RaiseError`, `SendError`, `SubscribeError`, `ServeError`, `SettleError`)
   stay open, because ridl §10.3's detected infrastructure failures are
   open-ended and a runtime may need to report one this crate does not yet name.

10. **A breaking `ridl-rt` change is a 0.x minor release, and a public struct
    whose fields are all public and that carries no `#[non_exhaustive]` cannot
    gain a field without a breaking change, because code outside the crate can
    build it as a struct literal.** Fifteen named-field structs in 0.1 meet that
    condition: `CatalogRef`, `Member`, `Timing`, `PayloadInfo`, `EncodedSizes`,
    `Encoded`, `Violation`, `RawSample`, `RawOccurrence`, `Claim`, `Watermark`,
    `Changed`, `Envelope`, `Sample` and `Occurrence`. The public tuple structs —
    `Ordinal`, `InterfaceNo`, `CatalogHash`, `Correlation`, `ClaimId`,
    `Timestamp` and `Duration` — follow the same rule: each has one public field
    code outside the crate builds directly. The unit structs `FlatBuffers`,
    `Proto3` and `ReprC` (`crates/ridl-rt/src/encoding.rs`) follow it for a
    different reason: each is a marker type with no field, and code outside the
    crate uses it as a value or a pattern, so a field added to any of them
    breaks that code just as surely. A field added to any of these structs is
    such a change — recorded with no code change against driftsys/ridl#350
    item 4. The crate's own version is independent of the workspace's (ADR-0007
    decision 14); the tag and the maintainer acts that create it are that
    decision's, not this one's. The API questions that stayed open when 0.1
    shipped — driftsys/ridl#350 items 5, 12, 13 and 14, and the meaning of
    `Watermark::seq` — are tracked on that issue; the review debt of
    driftsys/ridl#348 is driftsys/ridl#349.

    `ridl-rt` supports Rust 1.83 or newer: `rust-version = "1.83"` in
    `crates/ridl-rt/Cargo.toml`. The crate's manifest compiles as edition 2021,
    because its 1.83 minimum predates edition 2024 — Rust cannot build that
    edition before 1.85 — and the crate is tested as both editions it supports:
    edition 2021 with the 1.83 toolchain and with the `rust-toolchain.toml` pin,
    and edition 2024 with the pin — edition 2024 did not exist before Rust 1.85,
    so the 1.83 minimum cannot build it. `just test` covers edition 2021 with
    the pin; `just compat-check` covers edition 2021 with 1.83 and edition 2024
    with the pin, against the crate `cargo package -p ridl-rt` produces, not a
    hand copy. The root `Cargo.toml` sets `resolver = "2"`, because
    `cargo package` writes the workspace's resolver into the packaged manifest,
    cargo 1.83 cannot read resolver 3 ("feature `edition2024` is required"), and
    resolver 3's MSRV-aware fallback would resolve every workspace dependency
    for `ridl-rt`'s minimum rather than only its own; decided by Sebastien on
    2026-09-14 in driftsys/ridl#352. The pin follows the latest stable release:
    a maintainer bumps it by hand under
    [ADR-0009](ADR-0009-toolchain-and-gate-parity.md) decision 3 when a new
    stable release appears (driftsys/ridl#353). Raising `rust-version` is a
    breaking change under this decision's rule, shipped in a 0.x minor release;
    a toolchain pin bump does not by itself change `rust-version`. This replaces
    R-12's `rust-version` item — not the rest of R-12 — in
    [the archived design record](../archive/2026-09-13-ridl-rt-v0.1-design.md),
    which set `rust-version` equal to the pin, decided by Sebastien on
    2026-09-14 during the review of driftsys/ridl#352, because a minimum tied to
    the pin would rise with every toolchain bump.

    `ridl-rt` compiles as edition 2021 and is tested as editions 2021 and 2024,
    with Rust 1.83 and with the pin, in the matrix above. The Rust codegen
    (roadmap epic E11) emits code that compiles under the same matrix — edition
    2021 with 1.83, edition 2021 with the pin, and edition 2024 with the pin —
    and the codegen stories test that; decided by Sebastien on 2026-09-14 during
    the review of driftsys/ridl#352, because Cargo compiles each crate under its
    own edition — an edition-2024 dependency builds under an edition-2021
    consumer once the toolchain is 1.85 or newer — but source ridl emits, copied
    or generated directly into a consuming crate, compiles as that crate's own
    edition, and a consumer may be edition 2021 or 2024.

11. **Amendment (2026-09-20) — every port trait is implemented for `&mut P`, and
    every port trait whose methods all take `&self` also for `&P`.** For each
    port trait `T` in `crates/ridl-rt/src/port.rs`, the crate provides
    `impl<P: T + ?Sized> T for &mut P`; for the traits whose methods all take
    `&self` — `Attached`, `Clock`, `SignalReader`, `FixedReader`,
    `ScannableSignals` and `CoherentSignals` — it also provides
    `impl<P: T + ?Sized> T for &P`. Until these impls land, nothing except a
    runtime's own type implements a port trait, so a face can be built over a
    runtime's port and over nothing else: not over a borrow of it, and not over
    a wrapper that adds tracing or a test double. The impls are additive — every
    existing signature and every existing implementation is unchanged — so this
    is not a breaking change under decision 10, and they need no `alloc`. They
    are what keep `Client::new(&mut h)` compiling once the face of
    [ADR-0023](ADR-0023-interaction-face-generation.md) decision 5 takes its
    port by value, with `P` inferred as `&mut H` for whatever handle `H`
    decision 12 has the runtime present; the by-value face then also accepts
    that handle owned, a `Clone` handle, or any wrapper that forwards the port
    traits. A face built over `&mut H` holds that borrow for as long as the face
    lives, exactly as a face does now — these impls widen what a face accepts,
    and it is decision 12's one handle per port role that lets two faces run
    over one runtime without contending for a single value.
    **`impl<P: T + ?Sized> T for Box<P>` is deferred**, because it needs
    `alloc`, which this crate brings in under no feature combination (decision
    8, and [the design record's](../design/ridl-rt.md) "Features, `no_std`,
    `alloc` and `wasm32`"); it is deferred until a cargo feature brings in
    `alloc` and something needs a boxed port.

12. **Amendment (2026-09-20) — a runtime presents one handle per port role, it
    may also offer an aggregate handle per face, and `ridl-rt` adds no `Send` or
    `Sync` bound to any port trait.** A **port role** is one port trait. A
    runtime crate exposes one handle type per port role it implements, rather
    than one type implementing them all. A handle whose port traits all take
    `&self` is `Send + Sync`, because several threads may read one store at
    once; a handle carrying a trait with a `&mut self` method is `Send` and is
    not required to be `Sync`, because one thread drives each. Stating the rule
    by receiver rather than as a list classifies every port trait, including
    `EventSink`, whose `raise` takes `&mut self`, and `Clock`, whose `now` does
    not.

    A face is built over one value implementing exactly the port traits its
    interface needs, which [ADR-0023](ADR-0023-interaction-face-generation.md)
    decision 5 leaves unchanged. When a face needs exactly one port trait, that
    value is the role handle itself. When it needs more than one — the common
    case, because a generated `Client` may be bound over
    `SignalReader + EventSource + Caller` at once — it is an **aggregate
    handle**: one the runtime offers for that port set, or one the application
    composes from role handles and forwards the port traits through, which is
    the wrapper decision 11's impls admit. An aggregate is `Send`, and is `Sync`
    only if every port trait it carries takes `&self` in all its methods, which
    is the same receiver rule applied to the traits it composes. Either value
    reaches `Client::new` by value, or as a `&mut` borrow of itself under
    decision 11's forwarding impls. A handle for one role does not satisfy a
    multi-trait bound, so a face that needs more than one port trait must be
    given an aggregate (the second 2026-09-20 amendment; driftsys/ridl#429).

    This record states the expectation; the crate does not enforce it. No port
    trait gains `Send` or `Sync` as a supertrait, because that would exclude a
    single-threaded `no_std` runtime whose handles use `Cell` or `RefCell`
    internally, which is a supported target on the platform ladder. A runtime
    crate checks its own handles with a compile-time assertion —
    `fn assert_sync<T: Sync>()` applied to a reader handle. The alternative this
    rejects is one runtime struct implementing every port and shared behind a
    mutex: every `SignalReader::read` would then wait behind every
    `SignalWriter::commit`, removing the property a signal read is specified to
    have, that a read does not block on a publication. Story E11.9 builds the
    first runtime to this shape, and its roadmap row states that obligation.

## Alternatives considered

| Question                   | Alternative                                                | Why it was not chosen                                                                                                                                                                                                                      |
| -------------------------- | ---------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Identity (decision 1)      | keep `ServiceId`                                           | the identity studies give a service no number; nothing would produce the type                                                                                                                                                              |
| Identity (decision 1)      | a provisional bit inside `InterfaceNo`                     | a provisional number routes identically to a frozen one, and the bit would take width decision 2 fixes                                                                                                                                     |
| Widths (decision 2)        | keep the note's `u16`                                      | the IR already carries an ordinal as `uint32`, and the catalog descriptor plan writes `uint32` for both                                                                                                                                    |
| Port binding (decision 3)  | the full `(catalog, interface, ordinal)` key on every call | a lookup and a pointer on every call, and a catalog field on every result struct                                                                                                                                                           |
| Port binding (decision 3)  | an address handle resolved once                            | a table per binding in the runtime; a handle is not `const`, so generated dispatch cannot `match` on it, and a handle from one port means nothing to another                                                                               |
| Clause result (decision 4) | `require`/`ensure` return `Result<(), Violation>`          | typl v0.1 has no invariant constraint, so the `Violation` would name one that does not exist, and `Handler::settle` reads neither variant's payload                                                                                        |
| Clause result (decision 4) | the index of the failing clause as the error               | neither `Contract` variant carries a payload, so reading the index needs one on both plus a wire form for it                                                                                                                               |
| #308 (decision 5)          | a caller field added to `Envelope`                         | ridl §3.1 defines exactly two fields; the field would expose transport identity above the port and decide E14.2's question in advance                                                                                                      |
| #309 (decision 6)          | a ninth port that records withheld occurrences             | a port every runtime must implement, for a reading the reference has not chosen, and it decides E14.2's question in advance                                                                                                                |
| Proof type (decision 7)    | a `#[doc(hidden)]` public `Ref` constructor                | a convention, not visibility — any crate could still forge a proof                                                                                                                                                                         |
| Proof type (decision 7)    | `Ref` over bytes only, with a required separate `check`    | decoding a flatc-style buffer then needs an unchecked root (`unsafe`) or a second verification pass                                                                                                                                        |
| Features (decision 8)      | let `flatbuffers` pull the dependency in 0.1               | pins a version before story E11.7 chooses one, and obliges every binary that enables the feature to provide an allocator for a codec that does not exist yet                                                                               |
| Error enums (decision 9)   | `#[non_exhaustive]` on `Contract` and `CallError` too      | their variants are ridl §10's fixed categories and strata; a new one there is a language change, not a runtime's to add                                                                                                                    |
| Rust version (decision 10) | `rust-version` equal to the `rust-toolchain.toml` pin      | it would rise with every toolchain bump, and repeats the pin that ADR-0009 decision 2 keeps in one file                                                                                                                                    |
| Rust version (decision 10) | no `rust-version` at all                                   | cargo's MSRV-aware resolver and crates.io get no minimum to build against                                                                                                                                                                  |
| Rust edition (decision 10) | keep `ridl-rt` on the workspace's edition 2024 only        | the 1.83 minimum cannot build edition 2024, so the crate would break its own `rust-version`; and source ridl emits, copied or generated into an edition-2021 consumer — which compiles as that consumer's own edition — would have no test |
| Forwarding (decision 11)   | no forwarding impls; runtimes hand out short-lived ports   | moves the cost into every runtime rather than removing it, and still admits no wrapper, no test double, and no face over a borrow of a runtime's own value                                                                                 |
| Forwarding (decision 11)   | `impl<P: T + ?Sized> T for Box<P>` in 0.1                  | needs `alloc`, which no feature combination of this crate brings in (decision 8); deferred rather than rejected                                                                                                                            |
| Threading (decision 12)    | one runtime struct implementing every port, behind a mutex | serialises every signal read behind every publication commit, removing the property a signal read is specified to have                                                                                                                     |
| Threading (decision 12)    | `Send + Sync` as supertraits on the port traits            | excludes a single-threaded `no_std` runtime whose handles use `Cell` or `RefCell` internally, a supported target on the platform ladder                                                                                                    |

## Consequences

- **Positive.** A generated client's catalog check happens once, not per call; a
  downstream crate cannot decode a payload without a proof or add a fourth
  payload encoding without an ADR; and the two issues #308 and #309 raised
  against the note have crate-level answers a runtime can build against today,
  with a stated fallback if the reference finalization pass (E14.2) chooses
  differently.
- **Negative — deferred to E14.2.** Two sentences of the ridl reference are now
  known to need correction (the sequence-number scope on a call, and the
  invalid-event-payload delivery) but stay uncorrected until that pass runs; the
  `Init` envelope convention needs a sentence that confirms it, not a
  correction. A reader of the reference alone, without this record, sees the
  older wording until E14.2 adds these sentences.
- **Positive — added 2026-09-20.** Once decision 11's impls land, a face can be
  built over a borrowed port, a wrapped port or a test double rather than only
  over a runtime's own value; and from now on the threading model a runtime
  presents is stated where a runtime author reads it, rather than left for story
  E11.9 to settle by implementation (decision 12). Neither costs a breaking
  release: decision 11 is additive and decision 12 adds no bound to any trait.
- **Neutral — the `u16` fallback and the review debt stay open.** Decision 2's
  fallback and the driftsys/ridl#350 items decision 10 leaves open are not
  blocking anything scheduled now, and are recorded here so a future change to
  either is judged against a known list rather than rediscovered.

## Open

1. **The `u16` fallback for `Ordinal` and `InterfaceNo`** (decision 2), which
   story E11.1 triggers or not once the frame header is fixed.
2. **driftsys/ridl#350 items 5, 12, 13 and 14, and `Watermark::seq`** (decision
   10) — the API questions the reviews of 0.1 raised and did not resolve,
   tracked on that issue. The review debt of driftsys/ridl#348 is
   driftsys/ridl#349.
3. **The view a proto3 codec chooses** (`&[u8]`, an accessor, or an owned parsed
   value) — story E11.8's, not fixed here.
4. **How a generated binding keeps a signal's last good value with no `alloc`**
   — the Rust codegen's question, once it starts consuming this crate.
5. **`Box<P>` forwarding** (decision 11), deferred until a cargo feature brings
   `alloc` into this crate and something needs a boxed port.
6. **driftsys/ridl#350 items 15, 16 and 17** — the two the 2026-09-20 amendment
   settles, recorded there against decisions 11 and 12, and the wake hook, which
   that amendment deliberately leaves open for stories E11.1 and E11.9.

## Documents amended

| Document                                                                             | Change                                                                                                                                                                                                                                                                                                                           |
| ------------------------------------------------------------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| [ADR-0007](ADR-0007-e1-execution.md)                                                 | decision 14's 2026-09-14 amendment now points at this record and [the `ridl-rt` design record](../design/ridl-rt.md) rather than at the working `docs/wip/` spec, which this pull request archives, and now also points at R-12 of that archived spec, noting that this record's decision 10 replaces R-12's `rust-version` item |
| [ADR-0020](ADR-0020-third-encoding-runtime-layering-and-plugin-system.md) decision 5 | its 2026-09-13 amendment (the `strata` → `error` rename) now points at [the archived spec](../archive/2026-09-13-ridl-rt-v0.1-design.md) rather than at the working `docs/wip/` spec, which this pull request archives                                                                                                           |
| [ADR-0006](ADR-0006-walking-skeleton-execution.md) decision 1                        | a 2026-09-14 amendment records that `ridl-rt` is the one workspace crate on edition 2021, tested as both editions under this record's decision 10                                                                                                                                                                                |
| [ADR-0009](ADR-0009-toolchain-and-gate-parity.md) decision 4                         | a 2026-09-14 amendment records that `cargo fmt --all`'s style edition now follows each crate's own edition rather than one workspace-wide value, because `ridl-rt` is edition 2021 and every other crate is edition 2024                                                                                                         |
| [the `ridl-rt` design record](../design/ridl-rt.md), "The ports"                     | two paragraphs record the forwarding impls of decision 11 and the handle model of decision 12                                                                                                                                                                                                                                    |
| [the roadmap](../ROADMAP.md), story E11.9                                            | its `Done when` gains the handle model of decision 12: the loopback exposes one handle per port role, its reader handle is `Sync`, and it offers the aggregate the generated face is built over                                                                                                                                  |

## References

- [`docs/archive/2026-09-13-ridl-rt-v0.1-design.md`](../archive/2026-09-13-ridl-rt-v0.1-design.md)
  — the full reasoning trail (sections R-1 to R-12), archived once this record
  and [the design record](../design/ridl-rt.md) carried its content forward
- [the `ridl-rt` design record](../design/ridl-rt.md) — the crate's six modules
  and full type and trait surface, as built
- [ADR-0007](ADR-0007-e1-execution.md) decision 14 — the crate's version number,
  its tag, and the maintainer acts that create and publish it
- [ADR-0020](ADR-0020-third-encoding-runtime-layering-and-plugin-system.md)
  decisions 1 and 5 — the three payload encodings and the crate's module list
- [ridl language reference](../specification/ridl-language-reference.md) — §3.1
  the envelope, §4.5 provenance, §5.2 and §10.3 event delivery and serialization
  failure, §10.2 the contract categories, §11 identity
- `crates/ridl-rt/src/contract.rs`, `crates/ridl-rt/src/payload.rs`,
  `crates/ridl-rt/src/encoding.rs` — the identity types, the proof type and the
  sealed `Encoding` trait as built
