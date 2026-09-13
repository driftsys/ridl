# Release scope and the codegen plugin system — design note

Status: working note, 2026-09-12. Not ratified. This note records the decisions
of the 2026-09-12 re-scoping session. It supersedes the scope parts of
[`2026-09-08-roadmap-simplification.md`](../archive/2026-09-08-roadmap-simplification.md)
(that note's D-1, S-27, S-28, SR-X3 and SR-X9 are answered here) and it feeds
two later changes: the roadmap rewrite, and a new decision record plus in-place
amendments to
[ADR-0018](../decisions/ADR-0018-runtime-core-and-generated-surface.md).

The first consumer of the toolchain is a runtime in another repository. This
note never names it. Where the consumer's needs decided something, the need is
written as a generic requirement, and the consumer is the test of it, not the
citation.

## 1. The scope of this release

**Languages.** typl and ridl are finalized: every open question in their §17
receives a disposition — resolved, or deferred to a named version — and the
references drop "Draft". rsdl is rewritten as a language around
[`2026-09-08-topology-vocabulary.md`](2026-09-08-topology-vocabulary.md) and
lowered to the IR. rxdl stays a family member covering types, interfaces and
wiring; its rmdl half and its backend wait for rmdl. rmdl stays a Proposed draft
with no implementation.

**Backends.** Rust, with three payload encodings: FlatBuffers, proto3 and
repr(C). TypeScript, with generated types and the generated faces, the codec
reached through the wasm build of the runtime library, for Deno and the browser.

**Runtime material, per language.** A runtime library (`ridl-rt` in Rust, its
wasm build plus a TypeScript package), a frame specification, an in-process
loopback runtime for tests, and a separate WebSocket transport module that is
the default of the getting-started path and mandatory nowhere.

**The codegen plugin system.** A lowering step in the compiler, one backend
contract, and a process host for out-of-tree backends. The two in-tree backends
are ported onto the contract. See §3.8.

**Parked, with what reopens each.** The engine block — store, seqlock, sans-IO
core, platform traits, scheduler, ring depth, memory check — and any bus,
reopened by rmdl. The rmdl implementation, reopened by a consumer whose
behaviour has to be generated. rxdl's backend and domain spellings, reopened
with rmdl. The rest of the tooling plane and the gateway, reopened by a second
consumer. The plugin protocol's second host (wasm), reopened by the browser
playground.

**Next, immediately after this release.** Kotlin as the first external plugin,
with a hand-written Kotlin runtime library. See §3.9.

## 2. Ordering principle

The scope of §1 replaces the roadmap-simplification note's P-1 ("one consumer
orders the backlog") as the ordering principle. The first consumer remains the
validation of every runtime-facing decision, and its needs enter this note as
the generic requirements of §3.13.

## 3. Decisions

Each decision lists the alternatives that were considered and why they were not
taken.

### 3.1 rsdl is a language, rewritten directly

rsdl is rewritten around the topology-vocabulary note's nouns and lowered to the
IR the way ridl is. There is no separate deployment-descriptor schema step, and
no TOML descriptor as the user-facing surface.

Alternatives: (a) path O of the roadmap-simplification note's D-1 — a TOML
descriptor with no language surface — rejected because it gives the person
describing a system no syntax, no hover, no diagnostics, and it leaves the rsdl
reference as a document nothing implements; (b) ADR-0018 decision 17's sequence
— design the deployment-facts schema first, then rsdl as its authoring surface —
not taken, as a sequencing cost the author declined: the rewrite designs the
facts and the syntax together, and the IR is the schema.

Consequence: roadmap story E11.6 (the hand-written deployment-facts schema)
closes as superseded; the facts it listed become part of what rsdl lowers into
the IR (§3.13).

### 3.2 rxdl stays, trimmed

The rxdl reference stays in `docs/specification/` and keeps its place in the
family. Its unrestricted profile covers types, interfaces and wiring in one
file; the model layer of that profile, and the domain spellings and their
bindings (roadmap E7.7–E7.9), together with E7's ecosystem half (E7.2–E7.6),
wait for rmdl. The reference gains a status line saying so.

Alternative: archive the reference next to the retired uxdl reference — rejected
because rxdl is not retired, only partly deferred.

### 3.3 repr(C) is a third payload encoding

A third encoding joins FlatBuffers and proto3: a generated `#[repr(C)]` layout
struct per type in the C-representable subset, a C header emitted from the same
IR, and a generated codec between the layout struct and the domain type. It
serves two uses: a wire payload where FlatBuffers is not wanted, and a
C-readable layout for interoperation with a legacy consumer.

The domain types do **not** carry `#[repr(C)]`. They stay the value objects of
roadmap epic E10 — a private field and a checked constructor. A scalar newtype
may additionally carry `#[repr(transparent)]`, which constrains nothing.

Alternative: `#[repr(C)]` on the domain types themselves, so they cross a
foreign-function boundary as they are — rejected because every domain type would
then have to live in the C subset (no `String`, no `Vec`, no `Option<T>`, no
enum with data), every Rust consumer would pay that representation including the
ones that only speak proto3, and a typl §17.11 width change would move every
field after it in every consumer's struct. ADR-0007 decision 13 did this for
fixed-layout structs (scalar newtypes got `#[repr(transparent)]`) and ADR-0018
retired it because the header could not carry strings, optionals or collections;
ADR-0018 decision 6 rejects exposed layout at the FFI for the same reason.

This amends ADR-0018 decision 3 ("two encodings and no more"). The projection
rules — the fixed-capacity layout of a bounded string, optional and collection,
the string capacity and terminator, alignment and endianness — belong in a
projection record of the shape of ADR-0017 and ADR-0019, written when the
backend is.

### 3.4 One `ridl-rt` crate, encodings as features

`ridl-rt` is a single `no_std` crate with the modules the ridl-rt design note's
§1.1 lists — `contract`, `sample`, `payload`, `port`, `strata`, `encoding` — and
one cargo feature per encoding: `flatbuffers`, `proto3`, `repr-c`. With default
features off it has no dependency. It builds natively and for `wasm32`, and
joins the crates `just wasm-check` covers.

The runtimes live outside it, as the note's RA-03 requires:

| Crate or package                           | Holds                                                                              | Depends on                                      |
| ------------------------------------------ | ---------------------------------------------------------------------------------- | ----------------------------------------------- |
| `ridl-rt`                                  | the library above                                                                  | the FlatBuffers runtime, only under its feature |
| `ridl-loopback`                            | the in-process reference runtime: every port over a queue and a map, no IO         | `ridl-rt`                                       |
| `ridl-transport-ws`                        | the ports over a WebSocket, proto3-framed; the default of the getting-started path | `ridl-rt`, a WebSocket crate                    |
| the TypeScript runtime package (name open) | the `ridl-rt` wasm module plus the port interfaces spelled in TypeScript           | the wasm artifact                               |
| the TypeScript WebSocket package           | the same binding for Deno and the browser                                          | the runtime package                             |

Generated Rust links `ridl-rt`; generated TypeScript imports the runtime
package; neither ever names a transport.

Alternative: split the port traits into their own crate (`ridl-abi`) under
`ridl-rt` — rejected because the ports are not definable without `Sample<T>`,
`Envelope` and `Provenance`, so the two crates would always ship together at
matching versions, and what the split would buy — an engine author depending on
the traits without the codec machinery — the features already buy.

The roadmap-simplification note's S-03 (E11.0, `ridl-abi`) becomes E11.0
`ridl-rt`, the first story of the runtime-material epic. The archived `ridl-abi`
note is the first draft of the same design.

### 3.5 TypeScript: three modules, WebSocket optional

The TypeScript side is the same three layers as Rust: the runtime package (wasm
codec plus port interfaces), the generated faces, and a separate transport
package. The WebSocket transport is what the getting-started path uses and what
a remote server or an emulator links by default; nothing links it unless asked.

This confirms ADR-0018 decision 6 for TypeScript and un-parks the substance of
roadmap stories E12.1 (the TypeScript surface) and E12.4 (an emulator), re-homed
under the TypeScript work. Roadmap E11.9 becomes the transport crate and
package.

_Where it landed (2026-09-12, writing the record):_ "the wasm build of the
runtime library" in §1 and in this section's first sentence is wrong, and
ADR-0020 decision 7 corrects it. ADR-0018 decision 4 makes the codec a
serializer for known types, and the ridl-rt note emits the encoding impls and
the typl constraint checks per type into the generated package, so a wasm build
of `ridl-rt` alone carries no codec for any package's types. The codec is the
generated Rust for the package compiled to `wasm32` against `ridl-rt`; the
TypeScript runtime package carries the port interfaces and the loader. Decision
2's no-skew argument is unaffected — both artifacts still come from one `ridlc`
run over one IR.

Alternatives: (a) faces and frame only, no transport — rejected because then two
emulators written by two people share no transport, and the TypeScript backend
has no end-to-end test in this repository; (b) faces only, no frame
specification — rejected because two hand-written transports would not
interoperate.

### 3.6 The frame is a logical frame with one binding per encoding

The frame specification is written at the field level — ordinal, kind, envelope,
correlation, payload bytes — and names one binding per encoding: a proto3
binding for streams (the WebSocket transport uses it) and a FlatBuffers binding
for a mapped or within-node path. ADR-0018 decision 7 already says "one frame,
several bindings"; the specification has to be written at that level, or two
"same five kinds, different bytes" designs drift apart with no shared record.
The first consumer's local transport is one binding of this frame.

Roadmap E11.1 (frame and control-plane specification) stays, as a specification
with no implementation in this repository beyond the WebSocket binding.

### 3.7 The engine is outside this repository

The ridl-rt design note's §0 already moves the engine out and takes the name
`ridl-rt` for the library. This note confirms it and adds: the parked block
needs a name of its own on the roadmap (`ridl-engine` or `ridl-session`), and no
story in this release builds a store, a scheduler, a seqlock or a bus. The first
runtime is the consumer's.

### 3.8 The codegen plugin system

Generating a new language is hard today for a reason no plugin transport fixes:
each backend re-derives the same semantics from the raw IR. The TypeScript
backend's own header says it mirrors the Rust backend's init derivation; so do
the name transforms, the width derivation, the tombstone handling and the
width-to-`bigint` decisions. A third language written against the raw IR
re-implements all of it a third time.

The design is two changes, in this order:

1. **Lower once, in the compiler.** A codegen model between the IR and every
   backend: names already transformed per ADR-0016's pinned rules, widths
   already derived, inits already resolved, descriptors already tabulated,
   ordinals and constraint tables already flat. Every backend — in-tree or
   plugin — consumes that model and is mostly a printer.
2. **One backend contract, two hosts.**
   `generate(CodegenRequest) →
   CodegenResponse`. The request carries the
   lowered model in ADR-0014's canonical encoding plus the backend options; the
   response carries files (path, bytes) and diagnostics. The in-tree backends
   (Rust, TypeScript) implement the contract in-process. The process host
   implements it by running an executable named `ridlc-gen-<language>` found on
   `PATH` (or given with a flag), writing the request to its stdin and reading
   the response from its stdout. The plugin never touches the filesystem;
   `ridlc` writes the files, so output and dry-run options behave the same for
   every backend. A non-zero exit or a malformed response is a `ridlc` error
   naming the plugin.

A plugin is any executable, in any language: a Rust binary, a JVM launcher
script, a Deno script, a Python file.

**Proof without a second language.** The in-tree TypeScript backend is also
built as a `ridlc-gen-ts` binary; the test suite runs it through the process
host and asserts byte-identical output to the in-process path. That is the
parity test the protocol needs, at no cost in new languages.

**Prerequisite.** The contract is the IR encoding, so roadmap E4.5a (the IR
stability policy) lands first, and issue #231 (the canonical binary encoding
cannot round-trip the IR) is its first item.

**The wasm host** is a second host for the same contract — `ridlc` loads a
module and calls its export with the same request bytes. It is not in this
release: it adds a wasm runtime to `ridlc`, and it does not make "write the
plugin in TypeScript" easier, since TypeScript to wasm means bundling a
JavaScript engine. A TypeScript plugin author writes a Deno process speaking the
protocol, which the process host already covers.

Alternatives: (a) plugins over the raw IR with no lowering step — rejected
because every plugin re-derives the semantics above; (b) template packs (the IR
through a template engine) — rejected for faces and codecs, where the existing
backends each carry around a thousand lines of logic that a template language
cannot type-check; a template pack remains a reasonable way to write a
types-only printer over the lowered model; (c) backends as Rust crates loaded
dynamically — rejected because the plugin is then Rust-only and the
dynamic-library ABI is not stable, which reduces to the in-tree feature-gated
crate that exists today; (d) a wasm host only — deferred as above.

This restores the roadmap-simplification note's S-27 as written: E4.5b returns
to the critical path, and roadmap issue #47 (E4.5) stays whole — E4.5a and E4.5b
are both in scope.

### 3.9 Kotlin is the first external plugin, after this release

The first consumer delivers in Kotlin and needs Kotlin types with validation,
deserialization, and a Kotlin runtime library to read and write signals. That is
the full shape of a language: a hand-written runtime library (every codegen
ecosystem writes its runtime once) plus a generated layer — value classes with
checked constructors, a codec (FlatBuffers readers through the official Kotlin
runtime plus a generated verifier for payloads that are not `Inline`, the
ridl-rt design note's §9 third option, and a generated proto3 decoder), and the
faces.

The generated layer is a `ridlc-gen-kotlin` executable over the contract of
§3.8, most likely written in Kotlin by the people who maintain the Kotlin side.
It is sequenced immediately after this release, in a visible "next" band of the
roadmap, not parked. No JNI binding is planned; the ridl-rt note's RA-30 stands.

Alternatives: (a) no Kotlin at all, the consumer running `flatc --kotlin` on the
emitted schema — rejected by the author because the consumer needs validation
and a runtime library, which the stock generator does not give; (b) an in-tree
Kotlin backend — rejected as S-27 already rejected it: the plugin system exists
so that a third language does not enter this workspace.

This answers the roadmap-simplification note's SR-X3 (TypeScript stays a
first-party backend) and reopens its SR-X9 (what the Kotlin plugin proves) as a
real test of the protocol from outside, which the in-tree round trip of §3.8
cannot be.

### 3.10 typl: a unit type may be integer-backed

A unit type's backing is the literal type of its range: integer literals give an
integer backing, float literals give a float backing, and a unit type without a
range keeps today's float default, so every existing declaration is unchanged.
`Latency : ms [0..5000]` is then an integer type with a unit, and
`Timestamp : us`, `Duration : ms` become declarable in typl as the integers the
runtime library already makes them.

The grammar already distinguishes `int_lit` from `float_lit`, and TYPL-105
already rejects a `step` whose literal type differs from the range's, so a
backing cannot change by editing one number; when it is changed deliberately,
`ridl-diff` sees the backing change in the IR. The domain type, the width and
every encoding follow the rules that plain `integer` already has. The unit
algebra is an rmdl-era open item and is unaffected.

Alternative: an explicit backing keyword on unit types — rejected because
TYPL-105 already makes the flip explicit, and the rule is the one every language
uses for `1` versus `1.0`. The wire question ("can a float unit be sent as an
integer") was already answered by the width derivation from range and step (typl
reference §5.6, ADR-0013 decision 6).

New row for typl §17; disposed of in the finalization pass as a v0.2 syntax
change. Corpus check when implemented: any unit type written with integer
literals today changes backing and is either a wanted integer or a missing `.0`.

### 3.11 typl: a string is Unicode scalar values, UTF-8 in every encoding

The typl reference says `string` is "a character sequence" and bounds it in
characters, and never says what a character is or how a value is encoded. The
rule to record in typl §5:

- a `string` is a sequence of Unicode scalar values, and the bound counts scalar
  values — not graphemes, which are unbounded, and not bytes, which `bytes`
  already is;
- every encoding carries it as UTF-8; the in-memory representation is the
  language's own (UTF-8 in Rust; UTF-16 in TypeScript, Kotlin and C#), and the
  codec converts at the buffer, as every proto and FlatBuffers runtime on those
  platforms already does;
- the byte capacity of `string [N]` is 4·N. UTF-16 has the same worst case, so
  UTF-8 costs nothing at the maximum.

Alternative: UTF-16 on the wire — rejected because proto3 `string` must be UTF-8
and FlatBuffers `string` is UTF-8, so a UTF-16 wire means emitting `bytes` and
`[ushort]` in the schemas, and a consumer generated from those schemas sees an
opaque byte field where the contract says text; this breaks ADR-0018 decision
4's "interoperability is at the bytes, mediated by the emitted schema".

Per encoding: one sentence each in ADR-0017 and ADR-0019 ("typl `string`
projects to the target's `string`"); the repr(C) rules (fixed capacity 4·N plus
a length field, and whether a terminator is guaranteed for C readers) in the
repr(C) projection record. One rule for the TypeScript runtime package, because
the wasm codec cannot see the violation: a JavaScript string may hold a lone
surrogate, which `TextEncoder` silently replaces, so the binding checks
well-formedness before encoding and rejects.

New row for typl §17; disposed of in the finalization pass as normative text in
**§4** — §5 above is wrong. The sentences this rule replaces are the primitives
table's "character sequence" and §4.4's "encoding (ASCII, UTF-8, UTF-16) is a
codegen concern per target", both in §4; §4.4 is also the standing statement
this rule contradicts, and it now points at §17.13.

### 3.12 Amendments to the ridl-rt design note

Two corrections the first consumer's sketch exposed:

- **A ninth port, `Snapshot`.** An interface read under the coherence rule
  (ADR-0015 decision 9, ridl §14.5 — coherence is implicit, never declared)
  needs pinned-generation multi-reads. The note alludes to it in RA-28 and never
  defines it; §6 gains it.

  _Where it landed (2026-09-12, writing the amendment):_ as the extension
  `CoherentSignals::read_coherent` in that note's §6.1, not as a ninth core
  port. ADR-0015 **decision 10** separates production coherence, which decision
  9 makes implicit, from delivery coherence, which depends on the binding — ridl
  §14.5's table gives per-field only on SOME/IP, and ridl Appendix B's
  per-target matrix gives per-message only on proto3/gRPC. A runtime over such a
  binding cannot pin, so by §6.1's own test a core port would be one every such
  runtime has to fake. The portable answer decision 10 already names is the
  struct idiom of ridl §17.3.
- **`commit` takes no `now`.** §6 and §8 write
  `commit(&mut self, now:
  Timestamp)`; the note's own RA-28 says no port takes
  `now`. The publisher stamps through the `Clock` port, so the signature is
  `commit(&mut self)`.

### 3.13 Requirements derived from the first consumer

Written generically; the consumer is the test of each.

- **The system IR carries what a runtime derives its node descriptor from:** the
  region map, the link set, the routing table, the permission list, the surface
  set and the catalog hash. Process-local facts — file paths, ports, tuning —
  stay in the runtime's own configuration. A runtime's descriptor is then an
  emitter over the IR, and this is the acceptance test of the rsdl rewrite
  (§3.1).
- **A language without a ridl backend reads payloads through the emitted schema
  and its own generator**, restricted to `Inline` or trusted reads (the ridl-rt
  note's RA-29), because such a reader has no verifier and no typl constraint
  checks. This is the path a Kotlin consumer takes until §3.9 lands.
- **A string read per frame allocates on every UTF-16 platform.** The
  mitigations need nothing from typl — compare the raw bytes with the previous
  read and decode on change, or keep strings out of frame-rate signals, where
  ridl's contract already puts them as event and query payloads.

## 4. Open items carried

- **The encoding at the codec-in-wasm boundary.** ADR-0018 decision 3 puts
  proto3 on "the wasm guest boundary" because a guest updates independently of
  its host. A codec compiled to wasm and its TypeScript host are generated
  together and have no version skew, and decision 3's own rule classifies that
  boundary as within a node, which is FlatBuffers. If FlatBuffers is right
  there, TypeScript reads the buffer in place through generated accessors and no
  object materialization crosses the boundary. To be decided in the ADR-0018
  amendment.
- **`Inline` versus repr(C).** The ridl-rt note's `Inline` payload is a
  FlatBuffers struct read in place; a repr(C) struct read in place is the same
  operation with a simpler rule and no runtime dependency. Whether `Inline` is
  the repr(C) encoding, with FlatBuffers keeping only its tables, touches
  decision 3's reasoning and is decided with the repr(C) projection record.
- **The repr(C) layout rules** for a bounded string, optional and collection;
  string terminator; alignment; endianness.
- **The frame's control-plane subset** the WebSocket binding implements:
  `subscribe` with immediate current value, `call`, reply, and what of `attach`
  and `read` a framed transport needs.
- **rsdl's noun set.** The topology-vocabulary note's §7 keeps `component`; the
  roadmap-simplification note's S-36 dropped it. The vocabulary note is later
  and wins; the rewrite confirms it against the first system.
- **A name for the parked engine block** on the roadmap.
- **Where the TypeScript packages live** in this workspace.
- **The wasm host** for the plugin contract, when the browser playground
  returns.

## 5. What happens to the records

- **A new decision record** for the genuinely new topics: the third encoding and
  the encoding matrix (§3.3); the library / faces / transport layering per
  language (§3.4, §3.5); the codegen plugin system (§3.8).
- **In-place amendments**: ADR-0018 decisions 3, 6, 15, 16, 17 and its open item
  on the wasm boundary; ADR-0013's target list; the family overview's map,
  decision ledger and open-question index; typl §17 (two new rows); the ridl-rt
  design note (§3.12); the rxdl reference's status line.
- **The roadmap**, rewritten as a forward plan plus a landed record (the
  roadmap-simplification note's S-15), with a "next" band holding Kotlin.

## 6. Execution order

1. This pull request: archive the superseded working notes, land the 8 September
   notes, this note.
2. Close the out-of-scope issues (61) and the two draft pull requests (#304,
   #305) whose subject is the parked engine; retitle the rescoped ones (18).
3. The roadmap rewrite; archives the roadmap-simplification note.
4. The decision record and the amendments of §5.
5. Then the plan's first stories: E4.5a with #231, E11.0 `ridl-rt`, the lowering
   step, the language finalization passes.
