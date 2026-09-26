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
(`ridl-loopback`, story E11.15 since the split of 2026-09-20; and
`ridl-transport-ws`, story E11.9), and the ridl reference finalization pass
(story E14.2).

Written from lane A of the 2026-09-13 step-1 coordination (driftsys/ridl#328),
which built `crates/ridl-rt`. The reasoning trail, including the "Alternatives
considered" table below, is
[`docs/archive/2026-09-13-ridl-rt-v0.1-design.md`](../archive/2026-09-13-ridl-rt-v0.1-design.md)
(sections R-1 to R-12); the crate's architecture — its six unconditional modules
(seven once story E11.18 lands `correlate`, the 2026-09-26 amendment's decision
15), the one the `flatbuffers` feature adds since 2026-09-20, the one the `std`
feature adds since 2026-09-25, and their full type and trait surface, as built —
is [the `ridl-rt` design record](../design/ridl-rt.md).

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

**Amendment (2026-09-26) — decisions 13 to 18, decision 8 folded, decision 11
corrected, open questions 5 and 6.** Lane F's design note,
[`2026-09-25-async-face-design.md`](../wip/2026-09-25-async-face-design.md),
designed the substrate the generated async client of
[ADR-0023](ADR-0023-interaction-face-generation.md) decision 6 polls: a keyed
wake source on a port, a correlation table, and the errors a call and a `serve`
return. Sebastien disposed of its fifteen decisions on its pull request on
2026-09-26; decisions 13 to 18 record F-1, F-5, F-6, F-8, F-9 and F-13 of that
disposition, and the items the reviews of driftsys/ridl#519, #522 and #523 left
for this amendment on driftsys/ridl#509. Decisions 13, 14, 15 and 16 land in
stories E11.16 and E11.18; until they merge this record describes items
`crates/ridl-rt/src/port.rs`, `error.rs` and a `correlate.rs` do not yet
contain. Decision 17 ratifies what E11.19 built.

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

   **Amendment (2026-09-21) — the check is not emitted yet, and waits on E16.2
   (driftsys/ridl#378).** No generated constructor compares `port.catalog()`
   against its interface's `CATALOG`, and none has since the face first landed:
   `.catalog()` is called nowhere under `crates/ridl-backend-rust/`. The
   sentence above described a check the codegen never wrote, which
   driftsys/ridl#448 recorded. It is deferred rather than written now because
   the descriptor emitter still writes `CatalogHash([0u8; 32])` as a
   placeholder, so the comparison would hold two zero hashes against each other
   and pass for every port, of every catalog — a check with no power, in a place
   where its presence would read as a guarantee. E16.2 is what gives a catalog a
   computed hash; the check lands with it or after it, and **the story that
   emits it also takes the decision this record leaves open: what `new` does on
   a mismatch.** Decision 3's substance is unchanged — the binding is one
   catalog per port, checked once at construction and not per call — and what
   the amendment changes is only the tense: this is what a generated client will
   do, not what it does. Until then a face built over a port bound to another
   catalog reads and writes the wrong interface's slots with no error, which is
   sound only because `ridl-loopback` is in-process and single-catalog.

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

   **Note (2026-09-26, driftsys/ridl#544).** The ridl reference no longer waits
   on E14.2 for this sentence: ridl §3.1 and §6.1 now match
   [frame specification](../specification/frame-specification.md) §7, which
   gives a caller one counter over every request it sends and keys duplicate
   suppression on the caller's identity plus the request's `seq`. A counter
   unique per caller is also unique per (caller instance, channel). The `seq` 0
   convention is stated in frame specification §7 and not yet in the reference.

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

   **Note (2026-09-26, driftsys/ridl#544).** The ridl reference no longer waits
   on E14.2 for this sentence: ridl §5.1 now states it, matching
   [frame specification](../specification/frame-specification.md) §9.2, and
   quarantine is not chosen.

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

   **Amended 2026-09-20 (story E11.7, stage K3): `Encoded.bytes` is a subslice
   of the output buffer, not a prefix of it.** `Payload::encode` is handed one
   `&mut [u8]` and no allocator, and a FlatBuffers buffer is built back to front
   — the root offset is written last and sits at the low end of the finished
   buffer — so the encoder fills the slice from its end and the finished buffer
   ends where the slice ends. The alternative was one `copy_within` per encode
   to restore the prefix, a memmove of up to `MAX_SIZE` bytes on the path that
   exists to avoid copies. No signature changes: `Encoded` already carries
   `bytes: &[u8]`. **One consumer in the tree does assume a prefix**, and it is
   named here rather than left to be found: `ridl-backend-rust`'s `encode_into`
   returns `encoded.bytes().len()`, and the face it emits sends `&buf[..len]`.
   That is correct today, because that code names `ReprC`, whose encoder writes
   a prefix; it stops being correct the moment the face names an encoding that
   does not, so it changes with the face, in the stage that takes D-11. Passing
   `encoded.bytes()` itself, rather than a length the caller re-slices from the
   front of its own buffer, is what makes that code encoding-independent. Under
   decision 10 this is a 0.x minor, because the documented contract narrowed for
   a caller that assumed the bytes started at `out[0]`. The port methods of
   `port::*`, which copy a stored payload "into the front of `out`", are a
   different contract and are unchanged: a FlatBuffers buffer is
   position-independent, so a runtime may place one anywhere in the caller's
   slice.

   **Addendum 2026-09-21 (stage K7, driftsys/ridl#471): the one consumer named
   above no longer assumes a prefix, and it was not the face that changed it.**
   `encode_into` now evaluates to `encoded.bytes()` and the emitted code passes
   that subslice on unchanged, at all four sites — a command's and a query's
   `send`, a signal's `set`, an event's `raise`, and a query reply's `settle`.
   The paragraph above expected this to land with D-11, in the stage that moves
   the face onto `Wire`; it landed a stage early instead, because D-11 was then
   blocked: a payload type that mints no root table has no FlatBuffers root at
   all (driftsys/ridl#470). At the time the emitted code still named `ReprC`, so
   nothing observable changed — what changed is that it became
   encoding-independent, which is the property this amendment asked for. **D-11
   has since landed** (stage K9b): the emitted code names the package's own
   `Wire` alias, which is `::ridl_rt::encoding::FlatBuffers`, and the subslice
   the face passes on is now a suffix rather than a prefix at every send site.
   Restoring `&buf[..len]` in the emitter turns seven of the round-trip tests in
   `crates/ridl-backend-rust/tests/interaction_face.rs` red — applied and run,
   not inferred.

   The same amendment settles what `EncodeError::Capacity`'s `needed` means,
   which the FlatBuffers encoder is the first to make a question. An encoder
   that sizes its output before writing reports the whole encoding; one that
   builds incrementally reports what it needed when it gave up, which is a lower
   bound on the whole. `needed` is documented as that lower bound. The
   alternative, making it always the whole requirement, would have the
   FlatBuffers encoder compute a size it cannot know without encoding, or report
   `MAX_SIZE` and tell every caller to allocate the worst case for a value that
   may be far smaller.

8. **The three cargo features are declared and carry no dependency in 0.1.**
   `flatbuffers`, `proto3` and `repr-c` exist so a runtime can name every
   encoding without linking a codec, and the crate has no dependency in any
   feature combination. The `flatbuffers` dependency, its version, and the
   obligation it brings — every final binary that enables the feature must
   provide a global allocator, because the `flatbuffers` crate uses `alloc` even
   without `std` — arrive with story E11.7 as an additive change.

   **Amended 2026-09-20 (story E11.7, stage K3): the dependency did not arrive,
   and the sentence above is what changed.** E11.7's design note takes D-12
   without one: the `flatbuffers` feature now gates the crate's own reading and
   writing helpers, written here, and `ridl-rt` still has no dependency in any
   feature combination. So the allocator obligation this paragraph anticipated
   does not exist — the helpers allocate nothing, and a generated package over
   types that own neither a `String` nor a `Vec` is `no_std` with no allocator.
   The FlatBuffers runtime crate that ADR-0020 decision 5 permits under this
   feature stays permitted and unused; a later story that takes it would restore
   this paragraph's obligation with it.

   **Amended 2026-09-21 (story E11.7, stage K5): the `flatbuffers` feature gains
   one public item, `Builder::push_offset_vector`.** It writes a vector of
   `uoffset_t`s naming objects already written, which is what a vector of
   strings and a vector of tables encode to; every offset in it is relative to
   its own position inside the vector, and a `Pos` carries no arithmetic outside
   the module, so a caller cannot compute them. Stage K3's own module
   documentation said a helper of this kind would arrive with the emitter that
   needs it, and stage K5 is that emitter.

   It is recorded here rather than only in the design record because a new
   public item in `ridl-rt` is a decision of this ADR, not an implementation
   detail — the lane K driver's K-2 says so, and stage K3's amendment above is
   the precedent. The item is additive: no signature changes, nothing is
   removed, and it is reachable only under a feature that is off by default, so
   under decision 10 it is a 0.x minor rather than a break. It takes no
   dependency, allocates nothing, and is `no_std`, so the three constraints that
   bind this crate — `no_std`, the `wasm32` build with `--no-default-features`,
   and the edition 2021 build at rust-version 1.83 — are unaffected;
   `just wasm-check` and `just compat-check` both cover it through the
   `-p ridl-rt --all-features` invocations stage K3 added.

   No helper for a union arrived with it, and none is needed: a union's wrapper
   table and a non-table arm's box are both ordinary tables (ADR-0019 decisions
   1 and 2), which `Builder::push_table` already writes.

   **Amended (2026-09-26, lane F) — a fourth cargo feature, `std`, is part of
   this decision.** It is off by default, it links the standard library, and it
   enables the `task` module and its two public functions,
   `task::block_on(fut, deadline: Option<Instant>) -> Option<F::Output>` and
   `task::noop_waker() -> Waker`, both written over `std::task::Wake` on an
   `Arc` with no `unsafe` (story E11.17, driftsys/ridl#511, merged 2026-09-25 in
   #522). It adds no dependency, so the sentence "the crate has no dependency in
   any feature combination" still holds; the `no_std` build with the feature off
   is unchanged, and `just wasm-check` and `just compat-check` cover the feature
   through their `--all-features` invocations. The feature is not an encoding,
   so "one feature per encoding" describes the other three and not this one. It
   is for two callers: the generated blocking client, which is `block_on` over
   the async call's future (ADR-0023 decision 6), and a frame loop that polls a
   future once per frame with `noop_waker`. It compiles for
   `wasm32-unknown-unknown` but `block_on` is not usable there. Under decision
   10 the change is additive, so it ships in the 0.x minor of decision 18.
   `task` links `std`, and `std` brings `alloc` with it, which is the one place
   the crate reaches an allocator; decision 11's correction of the same date
   says what that does and does not change.

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

    **Amended (2026-09-21).** The independent-version sentence above no longer
    holds: ADR-0007 decision 14's 2026-09-21 amendment retires it. `ridl-rt` now
    shares the workspace's single version and travels on the same `v<version>`
    tag as every other published crate, publishing through
    `.github/workflows/crates-io-release.yml` rather than its own
    `ridl-rt@<version>` tag and a maintainer's by-hand `cargo publish`. What
    this decision still owns, unchanged: what counts as a breaking `ridl-rt`
    change (a 0.x minor release), the fifteen named-field structs and seven
    tuple structs a public field cannot be added to, and the edition/MSRV matrix
    below. The version _number_ moves with the rest of the workspace; the _rule_
    for when it must move stays this decision's.

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
    `impl<P: T + ?Sized> T for &P`. The crate itself implements no port trait,
    so until these impls land a face can be built only over a type that
    implements the traits by hand — a runtime's own, or an application's wrapper
    — and never over a reference to one. That is the whole of what they buy: a
    wrapper that adds tracing and a test double are accepted by the face's trait
    bounds already, with or without them. The impls are additive — every
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

    **Note (2026-09-25, story E11.17, driftsys/ridl#511).** The sentence above
    no longer holds under the `std` feature, which decision 8's amendment of
    2026-09-26 records: `task.rs` declares `extern crate std`, and `std` brings
    `alloc` with it. Whether the `Box<P>` forwarding impls are added under that
    feature is left to lane F's amendment of this record.

    **Amended (2026-09-26, lane F).** The `Box<P>` impls are not added under
    `std`. The reason is corrected rather than the deferral: `alloc` is now
    reachable under that feature, so "no feature combination brings it in" is
    false, but nothing needs a boxed port — a face holds its port by value or by
    `&mut` (ADR-0023 decision 5), and no runtime or consumer in the tree boxes
    one — and an impl nothing uses is not added. Open question 5 is reworded
    with this.

12. **Amendment (2026-09-20) — a runtime presents one handle per port role, it
    may also offer an aggregate handle per face, and `ridl-rt` adds no `Send` or
    `Sync` bound to any port trait.** A **port role** is one port trait. A
    runtime crate exposes one handle type per port role it implements, rather
    than one type implementing them all. In a runtime whose handles are used
    from more than one thread, a handle whose port traits all take `&self` —
    `Attached`, `Clock`, `SignalReader`, `FixedReader` and the two signal
    extensions — is `Send + Sync`, because several threads may read one store at
    once, and a handle carrying a trait with a `&mut self` method —
    `SignalWriter`, `EventSource`, `EventSink`, `Caller` and `Handler` — is
    `Send` and need not be `Sync`, because one thread drives each. Deriving the
    split from the receiver rather than from a list classifies every one of the
    eleven port traits. A single-threaded runtime is held to neither, for the
    reason the third paragraph gives.

    A face is built over one value implementing exactly the port traits its
    interface needs, which [ADR-0023](ADR-0023-interaction-face-generation.md)
    decision 5 leaves unchanged; the value implements at least those traits, and
    may implement more. When a face needs exactly one port trait, that value is
    the role handle itself. When it needs more than one — the common case,
    because a generated `Client` may be bound over
    `SignalReader + EventSource + Caller` at once — it is an **aggregate
    handle**: one the runtime offers for that port set, or one the application
    writes over role handles, implementing each port trait by delegating to the
    handle that has it. An aggregate is `Send` or `Sync` exactly when the
    handles it holds are, which the compiler derives. Either value reaches
    `Client::new` by value, or as a `&mut` borrow of itself under decision 11's
    forwarding impls. A handle for one role does not satisfy a multi-trait
    bound, so a face that needs more than one port trait must be given an
    aggregate (the second 2026-09-20 amendment; driftsys/ridl#429).

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
    **Since 2026-09-20 that story is E11.15** (driftsys/ridl#445): E11.9 was
    split, its loopback half became E11.15, and the `Done when` clause this
    paragraph points at moved to E11.15's row unchanged. E11.9 keeps
    `ridl-transport-ws`, which builds no runtime.

13. **Amendment (2026-09-26) — the `Wakeable` port extension and its key set
    (story E11.16).** `port` gains

    ```rust,ignore
    /// Extension: a port that can wake a task. A runtime that serves a
    /// generated async client implements it.
    pub trait Wakeable {
        fn wake_on(&self, what: Interest, waker: &core::task::Waker);
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum Interest {
        Outcome(Correlation),   // the outcome of one call is known
        Slot,                   // a slot for a new call is free
        Event(InterfaceNo),     // an occurrence of one of the interface's events is waiting
        Claim(InterfaceNo),     // a claim on one of the interface's members is waiting
    }
    ```

    `Wakeable` has no supertrait, like `Clock`, and is forwarded through `&P`
    and `&mut P` under decision 11. Its contract: a handle stores one waker per
    kind of key — `Slot`, `Event`, `Claim` — and an `Outcome` waker with its
    call; a change to any key of that kind that the handle observes wakes the
    stored waker, so a task that registers `Event(a)` and then `Event(b)` is
    woken by an occurrence of either, and a spurious wake is allowed; a
    `wake_on` whose waker `will_wake` the stored one is a refresh, which
    replaces the stored waker without waking it, and a waker of another task
    displaces the stored one and wakes it; a stored waker is woken at most once,
    after every change of its kind becomes visible, and is cleared when woken;
    the caller registers on every poll and registers before it reads the port.
    (The per-kind storage and the refresh rule replace "one waker per key" and
    an unconditional wake of the displaced waker, decided on driftsys/ridl#546
    and landed by driftsys/ridl#551.) A second task waiting for the same events
    holds a second handle, and each handle's waiter is woken. `Event` and
    `Claim` are keyed per interface, because `EventSource::next` and
    `Handler::next_claim` drain one queue whatever the ordinal and the
    subscription and the served set already filter by member. `Interest` is
    exhaustive, because a runtime must handle every key and an unknown key has
    no safe default; a new key is a 0.x minor under decision 10. A runtime with
    one unkeyed "something changed" source may wake every waiter it holds on any
    change; the contract is never that a waiter is woken only for its key. Every
    runtime that serves a generated async client implements the trait; a runtime
    with no wake source of its own has none to offer. The name is `Interest`,
    not `Wake`, because `task` already imports `std::task::Wake`. This closes
    open question 6. Notes F-5 and F-6.

14. **Amendment (2026-09-26) — `Transport::Busy` crosses the frame (story
    E11.16).** `Transport` gains `Busy`: the providing runtime refused the call
    at admission — no slot, no budget, or a call faster than the member's `min`
    — and the caller may retry later. It crosses as a `response` outcome: `busy`
    joins `corrupt` as a bare value of the frame's `outcome` field (frame
    specification §5.3, §5.4 and §5.6), the caller's runtime maps it to
    `Err(CallError::Transport(Busy))` (§5.3), §9.6 states it as the second of
    exactly two `Transport` variants that cross, and §8's cell on refusing a
    faster call says the answer is `busy`. `Transport` is `#[non_exhaustive]`
    (decision 9), so the variant is additive. `SendError::Busy`, the local case,
    is unchanged. Note F-13.

15. **Amendment (2026-09-26) — the `correlate` module: `Table<const N: usize>`
    and `Waiters` (story E11.18).** A seventh unconditional module,
    [ADR-0020](ADR-0020-third-encoding-runtime-layering-and-plugin-system.md)
    decision 5 amended in place. `Table<N>` is the caller-side table every
    runtime with asynchronous replies needs: `N` slots, each with a generation
    counter, the outcome's status and error, and one waker; the correlation is
    `(generation << 16) | slot`, so `N ≤ 65536` and 48 bits of generation
    remain; an optional byte budget debited at insert from `Member::reservation`
    and credited when the slot is reclaimed; `&mut self` throughout, so each
    runtime puts it behind its own lock. A slot is reclaimed by `forget` alone:
    at once for a settled call, and at the settlement for a call in flight,
    whose slot `forget` marks; `Caller::ack` and `Caller::reply` do not consume.
    On a reclaim every registered `Slot` waiter is woken. The table stores no
    reply bytes: a runtime keeps them in storage of its own indexed by slot,
    sized from `table_budget` where it has a descriptor. Every operation that
    would wake returns the waker instead, and the runtime wakes it after
    releasing its lock, so no waker runs under a runtime's mutex. `Waiters` is
    the bounded registry a handle keeps behind `Wakeable`: one `Option<Waker>`
    per kind of key, woken by a change to any key of that kind, `register`
    returning the displaced waker of another task and nothing on a refresh
    (decision 13), `take` clearing. Both allocate nothing and hold no lock. The
    exact method signatures are the plan's and the design record's.
    `ridl-loopback` moves its caller side onto both in the same story, with
    sixteen slots and no byte budget until E16.2 gives it a descriptor. Notes
    F-5, F-8 and F-9.

16. **Amendment (2026-09-26) — `ClientError` and `ProviderError`.** `error`
    gains `ClientError { Send(SendError), Call(CallError), Read(ReadError) }`,
    the error of a generated client call, and
    `ProviderError { Serve(ServeError), Claim(ReadError) }`, the error a
    generated `serve` resolves to; both `#[non_exhaustive]` under decision 9,
    `Copy`, with a `From` impl for each inner type. `CallError` stays exhaustive
    and gains no `Send` variant: a send failure is not a settlement outcome.
    `ClientError::Read` exists because the wait polls `Caller::reply`, whose
    outer `ReadError` reports the port itself; only `Detached` is reachable
    there, and it is kept as what it is rather than mapped onto a `Transport`
    variant, which would give a local failure a frame-level meaning. The types
    are this crate's and not the face's for the reason `Violation` is: a type
    every generated crate would emit identically belongs where a helper crate
    can name it. Note F-1.

17. **Amendment (2026-09-26) — the helpers of story E11.19 and
    `Encoding::max_size`, where they live, and what that means against ADR-0020
    decision 6.** E11.19 (driftsys/ridl#513, merged in #523) added, behind no
    feature:
    `Freshness::of(&Envelope, now, timing: Option<Timing>) -> Freshness` and
    `EventSeqTracker<const N: usize>` with
    `observe(interface, ordinal, seq)
    -> Result<Continuity, TrackerFull>`
    and `forget`, in `sample`; `Member::call_deadline() -> Option<Duration>`,
    `Member::reservation<E>() -> Result<u64, Unsized>` and
    `table_budget<E>(&[Member]) -> Result<u64, Unsized>` in `contract`; and
    `Encoding::max_size(&EncodedSizes) -> Option<u32>` as a required item of the
    sealed `Encoding` trait, which is not a breaking change because no crate
    outside this one implements the trait. `Continuity` is exhaustive: its four
    variants are the outcomes of comparing two sequence numbers, and a fifth is
    not a runtime's to add. `Unsized` is `#[non_exhaustive]`; `TrackerFull` is a
    unit struct. Each helper lives beside the type it reads, which this decision
    ratifies: moving one after the release of decision 18 would be a breaking
    change for no gain.

    The sentence that reconciles the helpers, and decisions 13, 15 and 16, with
    ADR-0020 decision 6: `ridl-rt` carries, beside the vocabulary generated code
    and a runtime agree on, the runtime-side helpers every runtime would
    otherwise write alone — pure data structures and pure functions, `no_std`,
    allocation-free, behind no feature, holding no lock, no thread, no clock and
    no I/O. A runtime remains its own crate: it owns the storage, the lock, the
    clock and the transport, and drives the helpers from them. The dependency
    graph of ADR-0020 decision 6, emitter output → `ridl-rt` ← runtime, is
    unchanged. Note F-8.

    One rule the generated face relies on is recorded here because the reviews
    of driftsys/ridl#519 found `SignalReader::read`'s documentation did not
    state it: a sample whose provenance is `Init` or `Invalid(Declared)` may
    carry zero bytes, because the runtime has no value to copy, and the
    generated face then returns the channel's init value under that provenance.
    The doc comment says so since this amendment.

18. **Amendment (2026-09-26) — E11.16 to E11.19 ship as one 0.x minor.** The
    four stories are one release under decision 10, tagged by a maintainer after
    E11.21's first half has merged, so that a generated face has exercised every
    item before it is published; nothing in lane F pushes a tag. The release
    carries decisions 8's `std` feature and 13 to 17. E11.21's first half, the
    one breaking step for a consumer of generated code, precedes the release, so
    a face has exercised the API before the tag; the second half follows the
    release and links the released crate.

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
| Forwarding (decision 11)   | no forwarding impls; runtimes hand out short-lived ports   | moves the cost into every runtime rather than removing it, and still admits no face over a reference to a port                                                                                                                             |
| Forwarding (decision 11)   | `impl<P: T + ?Sized> T for Box<P>` in 0.1                  | needs `alloc`, which only the `std` feature brings in since 2026-09-25 (decision 8), and nothing needs a boxed port; deferred rather than rejected                                                                                         |
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
  older wording until E14.2 adds these sentences. **Note (2026-09-26):** the two
  corrections are made — driftsys/ridl#544 aligned ridl §3.1, §5.1 and §6.1 with
  the frame specification for #308 and #309.
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
   story E11.1 triggers or not once the frame header is fixed. **Closed
   2026-09-22 by story E11.1**: the frame specification places no bound on a
   header's size, so the fallback is not triggered. A binding whose transport
   carries one of these numbers in a narrower native field range-checks the
   catalog at attach and refuses the session, rather than narrowing the type
   ([the frame specification](../specification/frame-specification.md) §4).
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

   **Reworded 2026-09-26 (lane F, decision 11's amendment).** `alloc` is
   reachable under `std` since 2026-09-25, so the condition is now only the
   second half: something that needs a boxed port.
6. **A wake hook** — whether a port gains a way to register interest in the
   arrival of a reply, an occurrence or a claim, or whether waiting stays a
   runtime's own loop. Every port method returns immediately, which this record
   keeps; what an adapter that presents a reply as a future does instead of
   polling is a frame and transport question, left to stories E11.1 and E11.9.
   Tracked as driftsys/ridl#350 item 17; items 15 and 16 of that issue are the
   two the 2026-09-20 amendment settles.

   **Closed 2026-09-26 by decision 13**, the `Wakeable` port extension: a port
   gains `wake_on(what: Interest, waker)`, every port method still returns at
   once, and the waiting is a future's, in generated code (ADR-0023 decision 6)
   or in `task::block_on`.

## Documents amended

| Document                                                                                         | Change                                                                                                                                                                                                                                                                                                                           |
| ------------------------------------------------------------------------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| [ADR-0007](ADR-0007-e1-execution.md)                                                             | decision 14's 2026-09-14 amendment now points at this record and [the `ridl-rt` design record](../design/ridl-rt.md) rather than at the working `docs/wip/` spec, which this pull request archives, and now also points at R-12 of that archived spec, noting that this record's decision 10 replaces R-12's `rust-version` item |
| [ADR-0020](ADR-0020-third-encoding-runtime-layering-and-plugin-system.md) decision 5             | its 2026-09-13 amendment (the `strata` → `error` rename) now points at [the archived spec](../archive/2026-09-13-ridl-rt-v0.1-design.md) rather than at the working `docs/wip/` spec, which this pull request archives                                                                                                           |
| [ADR-0006](ADR-0006-walking-skeleton-execution.md) decision 1                                    | a 2026-09-14 amendment records that `ridl-rt` is the one workspace crate on edition 2021, tested as both editions under this record's decision 10                                                                                                                                                                                |
| [ADR-0009](ADR-0009-toolchain-and-gate-parity.md) decision 4                                     | a 2026-09-14 amendment records that `cargo fmt --all`'s style edition now follows each crate's own edition rather than one workspace-wide value, because `ridl-rt` is edition 2021 and every other crate is edition 2024                                                                                                         |
| [the `ridl-rt` design record](../design/ridl-rt.md), "The ports"                                 | two paragraphs record the forwarding impls of decision 11 and the handle model of decision 12                                                                                                                                                                                                                                    |
| [the `ridl-rt` design record](../design/ridl-rt.md), the API table and the feature paragraph     | `Builder::push_offset_vector` joins the `flatbuffers` row, and the paragraph's claim that a field's inline offset and a table's size are the projection's facts is corrected: they are the codec emitter's, because no other emitter can observe them (2026-09-21, story E11.7, stage K5)                                        |
| [the roadmap](../ROADMAP.md), story E11.9, then E11.15                                           | its `Done when` gains the handle model of decision 12: the loopback exposes one handle per port role, its reader handle is `Sync`, and it offers the aggregate the generated face is built over. E11.9 was split on 2026-09-20 (driftsys/ridl#445) and that clause moved to story E11.15's row unchanged                         |
| [ADR-0020](ADR-0020-third-encoding-runtime-layering-and-plugin-system.md) decision 5             | a 2026-09-26 amendment records `correlate` as the seventh unconditional module (decision 15)                                                                                                                                                                                                                                     |
| [ADR-0023](ADR-0023-interaction-face-generation.md)                                              | its 2026-09-26 amendment, decision 6, is the face built over decisions 13 to 16; the two records were amended together                                                                                                                                                                                                           |
| [ADR-0018](ADR-0018-runtime-core-and-generated-surface.md) open question 5                       | a 2026-09-26 note answers it for the generated face (note F-15)                                                                                                                                                                                                                                                                  |
| [the frame specification](../specification/frame-specification.md) §5.3, §5.4, §5.6, §8 and §9.6 | `Busy` crosses as a `response` outcome (decision 14), written in story E11.16's pull request                                                                                                                                                                                                                                     |
| [the `ridl-rt` design record](../design/ridl-rt.md)                                              | the module table, the ports, the errors and the helpers sections follow decisions 13 to 17 as each story lands; its "seven unconditional modules" sentence lands with E11.18                                                                                                                                                     |
| [the roadmap](../ROADMAP.md), stories E11.16 and E11.18                                          | `Wake` is `Interest`, and "FIFO slot waiters" is "every `Slot` waiter woken on a reclaim" (decisions 13 and 15)                                                                                                                                                                                                                  |
| `crates/ridl-rt/src/port.rs`                                                                     | `SignalReader::read`'s zero-bytes rule (decision 17) and the `Box<P>` comment (decision 11) are doc-comment changes made with this amendment                                                                                                                                                                                     |

## References

- [`docs/archive/2026-09-13-ridl-rt-v0.1-design.md`](../archive/2026-09-13-ridl-rt-v0.1-design.md)
  — the full reasoning trail (sections R-1 to R-12), archived once this record
  and [the design record](../design/ridl-rt.md) carried its content forward
- [the `ridl-rt` design record](../design/ridl-rt.md) — the crate's modules and
  full type and trait surface, as built
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
- [`2026-09-25-async-face-design.md`](../wip/2026-09-25-async-face-design.md) —
  lane F's design note, F-1 to F-15, and the disposition that ratifies decisions
  13 to 18
- [ADR-0023](ADR-0023-interaction-face-generation.md) decision 6 — the generated
  clients and `serve` built over decisions 13 to 16
- `crates/ridl-rt/src/task.rs`, `sample.rs`, `contract.rs` — the `std` feature
  and the E11.19 helpers as built; driftsys/ridl#509 — the amendments issue
