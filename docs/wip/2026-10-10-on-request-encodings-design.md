# On-request encodings — design (stage 1 of the WebSocket and payload formats steering)

Date: 2026-10-10. Status: reviewed with Sebastien on 2026-10-10; every choice
the steering note did not fix is listed in section 15 with its confirmation or
its revision.

Satisfies: driftsys/ridl#801. Implements decisions A1, A2, A3, A4 and A5 of
`docs/wip/2026-10-06-ws-and-payload-formats-steering.md` (section 2), which are
fixed and not reopened here. Source prompt:
`docs/wip/ws-and-payload-formats-handoff.md`, "Decided by Sebastien before this
session".

## 1. Intent

A generated Rust package carries a payload codec only when its manifest asks for
that codec. Today the Rust backend always emits the FlatBuffers codec and a face
written for it, selected by a plugin option (`wire-encoding`) that nothing sets.
After this stage:

- a build that names no encoding emits the domain types and nothing else (A1);
- the manifest names the encodings per backend, `[backend.rust]` with
  `encodings = [...]` (A2);
- the generated face is generic over the encoding, so one build can carry
  several codecs; the encoding is a property of the port type, named once where
  the runtime or the transport is built (A3, refined on review);
- `ridl build` warns when a selected deployment has a link whose encoding the
  list lacks (A4);
- each emitted codec emits one size table per interface, derived from its
  `MAX_SIZE`, and the reservation and the table budget read that table instead
  of the `EncodedSizes` row (A5).

Success: `examples/cabin` builds and runs under `just demo` with one manifest
line added; a build of the same package with no manifest line compiles as a
types-only crate against `ridl-rt` with no encoding feature; the Rust backend
refuses `wire-encoding`; `ridl-rt`'s public API has no `EncodedSizes`; and the
tests of section 11 fail on the wrong behaviour, not only on a compile error.

## 2. Fixed inputs and what this spec adds

| Fixed by | Content                                                                   | This spec adds                                                                            |
| -------- | ------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------- |
| A1       | Types only by default: no codec, no face                                  | What "nothing else" covers (section 5.1), the generated manifest's features               |
| A2       | `[backend.rust] encodings = [...]`; no command-line flag; option replaced | Schema, validation, diagnostics, the request mapping (sections 3 and 4)                   |
| A3       | `Bind<E>`, `serve::<E>`; every named codec emitted; the program picks `E` | The exact signatures and the bound discipline (section 7): `E` is read from the port type |
| A4       | A warning under ADR-0024, lowerable in `[lints]`                          | Code, name, level, command, location (section 8)                                          |
| A5       | One size table per emitted codec; `ridl-rt` breaking release              | The trait, its items, what replaces each removed item (section 6)                         |

Two facts found while reading the code shape the design and are stated here
once:

1. **A stack buffer cannot be sized by a generic encoding.** The face sizes its
   buffers as
   `[0u8; <T as Payload<::ridl_rt::encoding::FlatBuffers>>::MAX_SIZE]`
   (`crates/ridl-backend-rust/src/face.rs:1075`, `face/serve.rs:94`,
   `face/poll.rs:188`). With `E` a type parameter, rustc on the pinned stable
   toolchain rejects `[0u8; <T as Payload<E>>::MAX_SIZE]` with "constant
   expression depends on a generic parameter" (checked on 2026-10-10 with a
   ten-line program). An array _type_ stated by a trait implementation for a
   concrete `E` is accepted. So every buffer the generic face holds is a type
   stated by an implementation: a payload's own buffer is an item of its
   `Payload<E>` implementation (`type Buffer = [u8; MAX_SIZE]`, section 6.1),
   and the two buffers that must hold any payload of an interface — the server's
   claim buffer and the client's next-event buffer — are items of the
   interface's `Sizes<E>` table. The shape was compiled and run on 2026-10-10
   with `rustup run 1.83 rustc --edition 2021`, the floor of the codegen build
   matrix: the trait, a generated-style implementation with rows over
   `MAX_SIZE`, `Interaction::ROW`, both `const fn`s, a generic function holding
   a buffer and a `static` table sized from `table_budget` gave reservation 32,
   budget 40, static table 40 and buffer 24; a per-payload buffer type gave
   payloads of 4 and 4096 bytes their own buffer size through one generic
   function; and a port type carrying its encoding let `Loopback::new()` and
   `Client::new(rt.attach())` compile with no type annotation (section 7).
2. **`ridl-rt` shares the workspace version.** ADR-0021 decision 10, amended
   2026-09-21, says `ridl-rt` "shares the workspace's single version"; the
   workspace and the published `ridl-rt` are at 0.7.0. The steering's "`ridl-rt`
   0.2" therefore means the next breaking 0.x minor of the workspace: **0.8.0**
   (DD-S1-1).

## 3. The manifest: `[backend.<name>] encodings`

### 3.1 Schema

```toml
[package]
name = "veh.cabin"
version = "1.0.0"

[backend.rust]
encodings = ["flatbuffers"]
```

- `[backend.<name>]` is a table per backend. `<name>` is the backend's language
  name as `ridl build` knows it: `rust` for the built-in Rust backend, and the
  `<language>` of a `--plugin <language>=<path>` for an external plugin
  (`kotlin`, for the Kotlin plugin). The manifest accepts any `<name>`: a
  manifest is shared by every build of the package, and a table for a backend
  the current build does not emit is read, validated and otherwise unused
  (DD-S1-2).
- `encodings` is the one key this stage defines. Its value is an array of
  strings, each one of the encoding names the toolchain knows, spelled as
  `ridl_rt::encoding::Encoding::NAME` spells them: `flatbuffers`, `proto3`,
  `repr-c`. The order is the manifest's order and carries no meaning beyond the
  order of the emitted items. An absent table, an absent key, or an empty array
  (`encodings = []`) means types only; the empty array is accepted so that a
  manifest can state the choice.
- Any other key under `[backend.<name>]` draws MANI-005 `unknown-manifest-key`,
  as an unknown key under `[codegen]` does (DD-S1-2).
- Only the workspace root's manifest, or a standalone package's manifest, may
  hold a `[backend.<name>]` table, the rule `[codegen] header-file` already
  follows (ADR-0002 §4, MANI-012): a workspace build emits one crate, so the
  choice is one per build (DD-S1-3).

This table is not the rsdl `backend.key` attribute namespace of RSDL-804
(`docs/specification/rsdl-language-reference.md`, "backend keys"), which
annotates rsdl declarations; the two share the word and nothing else. The
records say so where each is defined.

### 3.2 Validation and diagnostics

`ridl-core`'s manifest parser (`crates/ridl-core/src/manifest.rs`) reads the
table into a new `Manifest` field, `backends: BTreeMap<String, BackendTable>`
with `BackendTable { encodings: Vec<(String, Range<usize>)> }` (the span serves
the diagnostics). It validates the `encodings` value against the closed set of
names, which is the toolchain's (the IR's `Encoding` enum, the sealed
`ridl_rt::encoding::Encoding`), not a backend's (DD-S1-4). Two new codes, both
Error (an Error is never a lint, ADR-0024 decision 1):

| Code     | Severity | Summary                                                                                                                                    | Raised when                                                                                                                                                                                                                                           |
| -------- | -------- | ------------------------------------------------------------------------------------------------------------------------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| MANI-015 | Error    | `[backend.<name>] encodings` is not an array of encoding names, names an encoding the toolchain does not know, or names one encoding twice | `backend` is not a table; `[backend.<name>]` is not a table; `encodings` is not an array of strings; an element is not one of `flatbuffers`, `proto3`, `repr-c`; an element repeats. The message names the offending value and lists the known names. |
| MANI-016 | Error    | `[backend.<name>]` is set in a workspace member                                                                                            | A workspace member's manifest holds any `[backend.<name>]` table. Mirrors MANI-012.                                                                                                                                                                   |

An Error from the manifest stops the build before any file is written, as
MANI-011 does. The package is still checked: `ridl check` reports MANI-015 and
MANI-016 like every manifest diagnostic, because the loader reads the manifest
for every command.

The registered codes are added to `docs/specification/ridl-family-overview.md`
§7 (the MANI table) and to the diagnostic catalogue in
`crates/ridl-core/src/diag.rs`.

### 3.3 Builds with no manifest

A single-file build (`ridl build foo.ridl --emit rust`) has no manifest and so
no `encodings`: it is a types-only build. A command-line flag that names
encodings for such a build can be added later under ADR-0010, additively
(DD-S1-6). No test in this repository builds a face from a single file; the
tests that do build a face use a directory with a manifest.

## 4. The request: how the list reaches a backend

### 4.1 The option

The codegen request (`crates/ridl-ir/proto/ridl/codegen/v1/plugin.proto`,
`CodegenRequest.options`, `repeated BackendOption { key, value }`, sorted by
key, each key once) carries the list as one option:

```text
key   = "encodings"
value = "flatbuffers"              # one encoding
value = "flatbuffers,proto3"       # several: joined by "," in manifest order, no spaces
```

An absent `[backend.<name>]` table or `encodings` key sends no option: the
request's `options` is empty, as it is today. The `wire-encoding` key is
retired: the Rust backend refuses it the way it refuses every key it does not
know ("takes one option, `encodings`"), and no host sends it. Keys unique and a
joined value keep `BackendOption` unchanged; the schema stays `ridl.codegen.v1`
and the change is additive under IR specification §6 (DD-S1-5).

### 4.2 One request per backend

Today `write_emits` (`crates/ridlc/src/lib.rs`, around line 2070) builds one
request per package and hands the same request to every built-in emit and every
plugin, with `options` always `Vec::new()`. A per-backend option needs a request
per (package, backend): `write_emits` builds the request once without options
and, for each emit and each plugin, clones it with that backend's `options` set.
The built-in Rust backend receives `[backend.rust]`'s option; a plugin started
as `--plugin kotlin=...` receives `[backend.kotlin]`'s; the TypeScript, proto,
FlatBuffers and codegen-model emits receive the options of
`[backend.typescript]`, `[backend.proto]`, `[backend.flatbuffers]` and
`[backend.codegen-model]` when such tables exist, which they do not today, and
nothing otherwise (DD-S1-5). `run_build` reads `backends` from the loaded
`Manifest` beside `codegen_header_file` and passes it to `write_emits`.

A backend that receives an `encodings` option it cannot honour refuses it with
an error diagnostic, as the Rust backend does for an unknown key; that is the
backend's own contract (`docs/design/codegen-plugins.md`, "Options"). The
reference plugins `ridlc-gen-rust` and `ridlc-gen-model` read no option today;
`ridlc-gen-rust` wraps the Rust backend and inherits its reading of `encodings`.

### 4.3 The Kotlin plugin

driftsys/ridlc-gen-kotlin reads `wire-encoding` (its `Options.kt`, default
`flatbuffers`, any other value refused) and `kotlin-package`; it does not read
the model's `PayloadSizes`, and its `PayloadInfo` takes its size from
`Payload.flatbuffers_max_size`. Nothing in this stage changes what the host
sends it unless a manifest gains a `[backend.kotlin]` table, in which case it
receives `encodings=...` and, as written today, would refuse the unknown key.
The heads-up (section 9.5) states that, and recommends the plugin read
`encodings` under the same rule. The codegen model keeps
`Payload.flatbuffers_max_size` and `PayloadSizes`; the catalog descriptor keeps
every size column (A5) (DD-S1-17).

## 5. What the Rust backend emits

### 5.1 Types only (no `encodings`)

The domain types with their constructors, constraint checks, `Default`s and
derives, as `generate` emits them today, minus the FlatBuffers codec: no
`impl Payload<_>`, no `*FbView` type, no `__ridl_fb_*` helper. Also no
interaction surface: no interface descriptor (`pub struct Cabin;`,
`impl Interface`), no interaction descriptor, no size table, no `mod cabin`
face, no `blocking` module, no `prelude`. The descriptors go with the face
rather than with the types because they exist to be read by a face and a
runtime, and a types-only crate attaches to no port (DD-S1-7).

The generated `Cargo.toml` depends on `ridl-rt` with no encoding feature:
`ridl-rt = { version = "0.8" }` plus the existing `std` mapping. A types-only
crate still needs `ridl-rt` for `payload::Violation` and `payload::Rule`
(`crates/ridl-backend-rust/src/lib.rs:62-74`).

### 5.2 One encoding (`encodings = ["flatbuffers"]`)

Everything a build emits today, with the size table of section 6 in place of the
`EncodedSizes` rows and the buffer constants: the types; the FlatBuffers codec
(`impl Payload<::ridl_rt::encoding::FlatBuffers> for T` for every payload type,
`MAX_SIZE` a literal from `ridl_ir::projection::flatbuffers::max_size`); the
interface and interaction descriptors; `impl Sizes<FlatBuffers> for Cabin` per
interface; and the face. The generated `Cargo.toml` enables the `ridl-rt`
feature of each named encoding: `features = ["flatbuffers"]`. The feature names
are the encoding names (`flatbuffers`, `proto3`, `repr-c`), as ADR-0020 decision
5 fixed them.

### 5.3 Several encodings

For each named encoding the backend emits that codec's `Payload<E>`
implementations and its `Sizes<E>` table per interface; the descriptors and the
face are emitted once, generic over the port's encoding (section 7). The
`Cargo.toml` enables one feature per name.

Today the Rust backend has one codec. An `encodings` entry it has no codec for
is refused with an error diagnostic from the backend naming the entry and the
issue that builds it: "the Rust backend has no `proto3` codec
(driftsys/ridl#264)", "... no `repr-c` codec (driftsys/ridl#317)". The list is
validated whole: one unknown entry fails the build and nothing is emitted, the
way an unknown option key fails it today (DD-S1-8). When the proto3 codec lands
it removes that arm; the generic face and the `Sizes` trait need no change for
it.

### 5.4 The `--emit flatbuffers` and `--emit proto` schema flags

Unchanged. A `.fbs` or `.proto` schema is for another toolchain and is
independent of which codecs the Rust package carries; both flags were already
opt-in. `encodings` does not imply a schema emit and a schema emit does not
imply a codec (DD-S1-15).

### 5.5 The backend's library API

`crates/ridl-backend-rust`:

- `WireEncoding` keeps its name and its `#[non_exhaustive]` one-variant shape
  (`FlatBuffers`), loses `Default`, and gains
  `fn parse(name: &str) -> Option<Self>` over the encoding names.
  `check_emitted` stays the compile-time guard that a new variant trips
  (DD-S1-16).
- `WIRE_ENCODING_OPTION` is removed; `ENCODINGS_OPTION: &str = "encodings"`
  replaces it. `Backend::generate` reads that one key, splits its value on `,`,
  refuses an empty element, an unknown name and a name twice, and refuses any
  other key.
- `generate(&Package)` and `generate_with(&Package, &[&Package])` emit types
  only. `generate_with` keeps the signature ADR-0017 decision 1 fixed for every
  wire backend.
- `generate_face(&Package)` is removed;
  `generate_face_with(&Package, &[WireEncoding])` and
  `generate_pipeline(&Package, &[WireEncoding], &[&Package])` take the list. An
  empty list is types only; both refuse nothing on an empty list.
- `generate_pipeline_over(&v1::Model, &[WireEncoding])` likewise.

## 6. `ridl-rt` 0.8: the size table per codec

### 6.1 The payload's buffer and the `Sizes<E>` trait

**`Payload<E>` gains its buffer** (`ridl_rt::payload`, DD-S1-10 revised):

```rust
pub trait Payload<E: Encoding>: Sized {
    const MAX_SIZE: usize;
    /// `[u8; Self::MAX_SIZE]`, stated as a type so that code generic over the
    /// encoding can hold one on the stack (a generic array length is not
    /// expressible on stable Rust).
    type Buffer: AsRef<[u8]> + AsMut<[u8]> + Copy;
    /// A zeroed `Buffer`.
    const BUFFER: Self::Buffer;
    // View, encode, verify, decode: unchanged
}
```

The codec emits
`type Buffer = [u8; <literal>]; const BUFFER: Self::Buffer = [0u8; <literal>];`
beside `MAX_SIZE`, the three from the same literal. A face site that knows the
payload type — a signal read, a call argument, a reply, a publisher's send —
holds `<T as Payload<E>>::BUFFER`, so a call's stack use stays the payload's own
bound, as today.

**The size table** (`ridl_rt::contract`):

```rust
/// The size table of an interface in encoding `E`: what a face and a runtime
/// need to hold one table of calls in flight, or one payload whose type is
/// not known before it is read, in that encoding. Generated code implements it
/// once per interface per codec the package carries; every number derives
/// from `<T as Payload<E>>::MAX_SIZE`, which is the one source of a payload's
/// bound.
pub trait Sizes<E: Encoding>: Interface {
    /// Row `i` is the bytes one in-flight instance of `MEMBERS[i]` reserves:
    /// the sum of `MAX_SIZE` over the member's payloads — one payload for most
    /// kinds, the request and then the reply for a `query`, none for a fixed.
    const RESERVATIONS: &'static [u64];
    /// The largest `MAX_SIZE` over the argument and reply payloads of the
    /// interface's commands and queries; `0` when it has none.
    const MAX_BUFFER_SIZE: usize;
    /// The largest `MAX_SIZE` over the interface's event payloads; `0` when it
    /// has none.
    const EVENT_SOURCE_BUFFER_SIZE: usize;
    /// `[u8; Self::MAX_BUFFER_SIZE]`: the server's claim buffer, which
    /// receives a call before its member is known.
    type ClaimBuffer: AsRef<[u8]> + AsMut<[u8]> + Copy;
    /// `[u8; Self::EVENT_SOURCE_BUFFER_SIZE]`: the client's next-event
    /// buffer, which `EventSource::next` fills before the event is known.
    type EventBuffer: AsRef<[u8]> + AsMut<[u8]> + Copy;
    /// A zeroed `ClaimBuffer`.
    const CLAIM_BUFFER: Self::ClaimBuffer;
    /// A zeroed `EventBuffer`.
    const EVENT_BUFFER: Self::EventBuffer;
}
```

Two buffers stay interface-wide because the face fills them before it knows the
payload type: `dispatch` reads a claim into one buffer and then matches the
ordinal, and `EventSource::next(&mut self, out: &mut [u8])` writes one
occurrence of any subscribed event into `out` and reports its ordinal afterwards
(`crates/ridl-rt/src/port.rs`, `crates/ridl-backend-rust/src/face/poll.rs`,
`poll_next_event`). Each needs a type that generic code can name, and one
associated type per buffer in `Sizes<E>` is the simplest shape that compiles on
Rust 1.83 (it is the shape of the 2026-10-10 trial). The review asked for a
`Sizes<E>` with the three constants only; the event buffer is the one item kept
beyond that, for the reason above.

The generated implementation for interface `Cabin` and codec `FlatBuffers`
writes every item as a const expression over `MAX_SIZE` paths, never as a
number, so a table cannot drift from its codec (A5's "one source"):

```rust
impl ::ridl_rt::contract::Sizes<::ridl_rt::encoding::FlatBuffers> for Cabin {
    const RESERVATIONS: &'static [u64] = &[
        <Temperature as Payload<FlatBuffers>>::MAX_SIZE as u64,                 // signal
        <Warning as Payload<FlatBuffers>>::MAX_SIZE as u64,                     // event
        <Level as Payload<FlatBuffers>>::MAX_SIZE as u64,                       // command
        <Window as Payload<FlatBuffers>>::MAX_SIZE as u64
            + <Average as Payload<FlatBuffers>>::MAX_SIZE as u64,               // query
    ];
    const MAX_BUFFER_SIZE: usize = { /* today's max_size_const block */ };
    const EVENT_SOURCE_BUFFER_SIZE: usize = { /* likewise over events */ };
    type ClaimBuffer = [u8; Self::MAX_BUFFER_SIZE];
    type EventBuffer = [u8; Self::EVENT_SOURCE_BUFFER_SIZE];
    const CLAIM_BUFFER: Self::ClaimBuffer = [0u8; Self::MAX_BUFFER_SIZE];
    const EVENT_BUFFER: Self::EventBuffer = [0u8; Self::EVENT_SOURCE_BUFFER_SIZE];
}
```

(Paths are written in full in the emitted code, as the codec writes them.) The
`MAX_BUFFER_SIZE` and `EVENT_SOURCE_BUFFER_SIZE` inherent constants of the
interface descriptor move into this trait, because their value depends on the
codec (DD-S1-21). The row order is the order of `Interface::MEMBERS`, and
`Interaction` gains `const ROW: usize`, the index of `MEMBER` in
`Iface::MEMBERS`, so that a row is reached without a scan (DD-S1-9).

### 6.2 The reservation and the table budget

Two free functions in `ridl_rt::contract` replace `Member::reservation::<E>()`
and `table_budget::<E>(&[Member])`:

```rust
/// The bytes one in-flight instance of interaction `X` reserves in encoding
/// `E`: its row of the interface's size table.
pub const fn reservation<X, E>() -> u64
where
    X: Interaction,
    X::Iface: Sizes<E>,
    E: Encoding,
{
    <X::Iface as Sizes<E>>::RESERVATIONS[X::ROW]
}

/// The in-flight byte budget of interface `I`'s table in encoding `E`: the
/// sum of its rows, saturating. A table serving several interfaces adds the
/// budget of each.
pub const fn table_budget<I, E>() -> u64
where
    I: Sizes<E>,
    E: Encoding,
{ /* saturating sum over I::RESERVATIONS */ }
```

Both are infallible: a row exists for every member because a codec is emitted
for every payload type of the package or the build fails
(`crates/ridl-backend-rust/src/codec.rs`, "has no finite FlatBuffers bound"). An
encoding the package does not carry is a compile error at the call site
(`Cabin: Sizes<Proto3>` is not satisfied), which is the gain the handoff named:
"an encoding not asked for is a compile error instead of a run-time `Unsized`".
`const fn` so that a runtime may size a static table (DD-S1-9).

**How a runtime registers sizes.** A runtime driven by data takes
`<I as Sizes<E>>::RESERVATIONS` — a `&'static [u64]`, one row per member in
`MEMBERS` order — at the one point where the interface type is known, for
example
`runtime.register(Cabin::MEMBERS, <Cabin as Sizes<FlatBuffers>>::RESERVATIONS)`,
and is data-driven from then on: the two slices are parallel, and a row is
reached by the member's position. A runtime whose set of interfaces is known
statically sums `table_budget::<I, E>()` terms in a `const` and sizes a `static`
table from it at compile time, with no allocator.

**The MSRV trial.** On 2026-10-10 the whole shape — the trait, a generated-style
implementation with rows over `MAX_SIZE`, `Interaction::ROW`, both `const fn`s,
a generic function holding a buffer, and a `static` table sized from
`table_budget` — compiled and ran with `rustup run 1.83 rustc --edition 2021`:
reservation 32 for a query row with payloads of 8 and 24 bytes, budget 40 over
two rows, a static table of 40 bytes, and a buffer of 24 bytes, the interface
maximum. A per-payload buffer type (`Payload::Buffer`) was compiled the same
way: payloads of 4 and 4096 bytes got their own buffer size through one generic
function.

### 6.3 Breaking changes to `ridl-rt` (the full list)

All in the 0.8.0 release (ADR-0021 decision 10: a breaking `ridl-rt` change is a
0.x minor):

| Item                                                                         | Change                                                                                                                                                                                  | Replacement                                 |
| ---------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------- |
| `contract::EncodedSizes`                                                     | removed                                                                                                                                                                                 | `Sizes<E>::RESERVATIONS`                    |
| `contract::PayloadInfo::max_size`                                            | field removed; `PayloadInfo { type_name }` stays                                                                                                                                        | the size table                              |
| `contract::Member::reservation::<E>()`                                       | removed                                                                                                                                                                                 | `contract::reservation::<X, E>()`           |
| `contract::table_budget::<E>(&[Member])`                                     | removed                                                                                                                                                                                 | `contract::table_budget::<I, E>()`          |
| `contract::Unsized`                                                          | removed                                                                                                                                                                                 | none: a missing size is a compile error     |
| `contract::Interaction`                                                      | gains `const ROW: usize` (breaking for implementors; only generated code implements it)                                                                                                 |                                             |
| `contract::Sizes<E>`                                                         | added                                                                                                                                                                                   |                                             |
| `encoding::Encoding::max_size`                                               | removed; the trait keeps `NAME` and stays sealed                                                                                                                                        |                                             |
| `payload::Payload<E>`                                                        | gains `type Buffer` and `const BUFFER` (breaking for implementors: generated codecs, and the `Raw` placeholder in `payload.rs`'s tests)                                                 | `[u8; MAX_SIZE]` emitted beside `MAX_SIZE`  |
| `port::Encoded`                                                              | added: `pub trait Encoded { type Encoding: Encoding; }`, with `impl<P: Encoded + ?Sized> Encoded for &mut P`; every port type implements it (section 7.1)                               |                                             |
| Generated interface descriptor `MAX_BUFFER_SIZE`, `EVENT_SOURCE_BUFFER_SIZE` | inherent constants removed from the generated crate                                                                                                                                     | `<Iface as Sizes<E>>::MAX_BUFFER_SIZE` etc. |
| Generated face types                                                         | take the encoding from the port (`P: Encoded`, `P::Encoding`); no new type parameter (section 7; sub-stage 1b)                                                                          |                                             |
| Generated `Cargo.toml`                                                       | `ridl-rt = "0.8"`; features follow `encodings`                                                                                                                                          |                                             |
| `ridl-loopback`                                                              | `Loopback<E: Encoding = FlatBuffers>` and `HandlerHandle<E>`, both `Encoded`; `Loopback::new(catalog)` is FlatBuffers, `Loopback::<E>::with_encoding(catalog)` any other (sub-stage 1b) |                                             |

Every `ridl-rt` row above lands in sub-stage 1a, so that 0.8.0's `ridl-rt` break
is complete in one sub-stage; the three generated-code rows and the
`ridl-loopback` row land in 1b.

Not changed: `face::Bind`, `face::Events`, `face::Timeout`, `face::Publish`, the
existing `port` traits (`Encoded` is a separate trait, not a supertrait of
`Attached`), `MAX_SIZE`, `View`, `encode`, `verify` and `decode` on
`Payload<E>`, the three marker types, the cargo features,
`rust-version = "1.83"`, the codegen build matrix of ADR-0021 decision 10 (every
new construct here — associated array types, `const fn` with a loop, `as u64`
casts in a const, a defaulted type parameter — is stable in Rust 1.83 and
edition 2021), `ridl-rt-conformance` (its `Factory` bounds its `Runtime` by the
port traits only and never encodes a payload, so it needs no `Encoded` bound and
is unaffected).

`docs/design/ridl-rt.md`'s "Versioning" section still says the crate "carries
its own version, independent of the workspace's", which ADR-0021 decision 10's
2026-09-21 amendment superseded; this stage corrects it (DD-S1-22).

## 7. The generic face (sub-stage 1b)

### 7.1 The encoding comes from the port

A port type states the encoding of the bytes it carries (DD-S1-11 revised).
`ridl_rt::port` gains:

```rust
/// The payload encoding of the bytes a port carries. One session has one
/// encoding (frame specification); a transport that serves several encodings
/// hands out one port type per session, each typed with its own.
pub trait Encoded {
    type Encoding: Encoding;
}
impl<P: Encoded + ?Sized> Encoded for &mut P { type Encoding = P::Encoding; }
```

It is a separate trait, not a supertrait of `Attached`, so a port that never
meets a face (a conformance fixture) is not forced to name one. Every port type
a program binds a face to implements it: `ridl-loopback`'s `Loopback<E>` and
`HandlerHandle<E>`, a WebSocket port, a shared-memory port.

The generated face takes the encoding from its port type parameter and gains no
type parameter of its own. `Bind` in `ridl_rt::face` is unchanged. Per interface
module (`cabin`):

```rust
pub struct Client<P>
where
    P: ::ridl_rt::port::Encoded
        + SignalReader + EventSource + Caller + Clock + Wakeable,   // as today, plus Encoded
{ port: P }

impl<P> ::ridl_rt::face::Bind for Client<P>
where P: Encoded + /* as today */,
      super::Cabin: ::ridl_rt::contract::Sizes<P::Encoding>,
      Temperature: Payload<P::Encoding>, Warning: Payload<P::Encoding>,
      Level: Payload<P::Encoding>, Window: Payload<P::Encoding>,
      Average: Payload<P::Encoding>,                           // every payload of the interface
{ type Port = P; fn new(port: P) -> Self }

pub struct Publisher<W> where W: Encoded + SignalWriter + EventSink { ... }

pub fn serve<H, P>(h: H, p: &mut P) -> Serve<'_, H, P>
where
    H: Encoded + Handler + Wakeable,
    P: Provider,
    super::Cabin: Sizes<H::Encoding>,
    /* every payload: Payload<H::Encoding> */;

pub struct Serve<'a, H, P> where H: Encoded + ... { handler: H, provider: &'a mut P,
    buf: <super::Cabin as Sizes<H::Encoding>>::ClaimBuffer, ... }

pub mod blocking {
    pub struct Client<P> where P: Encoded + ...;
    pub fn serve<H, P>(h: H, p: &mut P, timeout: Option<Duration>)
        -> Result<(), ProviderError> where /* as above */;
}
```

The futures (`SetLevelCall<'a, P>`, `AverageCall<'a, P>`, `NextEvent<'a, P>`)
keep their parameters and gain the `Encoded` bound. `Provider`, `Subscribe`,
`Invalidate`, the descriptors and `prelude` do not change: they name domain
types only. Application code names no encoding and is unchanged:

```rust
let rt = Loopback::new(CATALOG);                       // FlatBuffers: the loopback's default
let mut publisher = cabin::Publisher::new(rt.attach());
let mut client = cabin::Client::new(rt.attach());
let serve = cabin::serve(&mut handler, &mut provider);
```

The encoding is named once, where the runtime or the transport is built. Each
transport picks a default from how coupled its two ends are, and the
construction can override it:

| Transport                    | Default     | Override                                                   |
| ---------------------------- | ----------- | ---------------------------------------------------------- |
| `ridl-loopback` (in process) | FlatBuffers | `Loopback::<E>::with_encoding(catalog)`                    |
| WebSocket (#265)             | proto3      | `ws::connect_with::<E>(url)`; `ws::connect(url)` is proto3 |
| shared memory (later)        | `repr(C)`   | likewise                                                   |

A cargo feature for the default was rejected: features must be additive, and a
default is a choice. The manifest says which codecs the package carries, not
which one a port uses; a port whose encoding the package does not carry is a
compile error at the binding (`Cabin: Sizes<Proto3>` is not satisfied). One
session has one encoding (frame specification rule); a transport that serves
several encodings hands out one port per session, each typed with its own.

Consequence for #265: the WebSocket transport lands with
`connect_with::<FlatBuffers>` only, the codec that exists; plain `connect` (the
proto3 default) is added additively when #264 lands.

### 7.2 Bounds

Generic code that encodes or decodes a payload `T` needs
`T: Payload<P::Encoding>`, and a trait's `where` clause is not assumed by its
users on stable Rust, so the bounds are written out: each `impl` and each free
function of the face carries, as a generated `where` clause,
`Iface: Sizes<P::Encoding>` and one `T: Payload<P::Encoding>` per payload type
the interface uses (DD-S1-12). The consumer never writes them: its port's
encoding is one the emitted codecs satisfy. A consumer who writes a helper
generic over the port restates them; that is the cost
`docs/design/interaction-face.md` ("The face gains no type parameter") named,
and it is paid by generated code, not by the program.

### 7.3 Buffers

A face site that knows the payload type holds the payload's own buffer,
`<T as Payload<P::Encoding>>::BUFFER`: a signal read (`face.rs`, the
`payload_buffer` call at line 365), a publisher's send (818, 846), a call
argument (`face/poll.rs:70`) and a reply (`face/poll.rs:142`). The stack use of
a call is the payload's own bound, as today (DD-S1-10 revised). The two sites
that read before they know the type hold a `Sizes<P::Encoding>` buffer: the
server's claim buffer (`face/serve.rs:94,104`, `face/dispatch.rs:170`) is
`CLAIM_BUFFER`, and the next-event buffer (`face/poll.rs:188`) is `EVENT_BUFFER`
(section 6.1).

### 7.4 Sub-stage 1a's face

Sub-stage 1a (section 13) keeps the face monomorphic over the one codec the Rust
backend has, but every size it reads already comes from
`<T as Payload<::ridl_rt::encoding::FlatBuffers>>::BUFFER` or from
`<Iface as Sizes<::ridl_rt::encoding::FlatBuffers>>`, so that 1b replaces the
concrete path by `P::Encoding`, adds the `Encoded` bounds, types
`ridl-loopback`, and nothing else.

## 8. The lint: RSDL-807 `link-encoding-not-emitted`

| Field         | Value                                                                                                                                                                                                                                                                                                                                                                             |
| ------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Code          | RSDL-807 (the next free code of the RSDL-80x family, the deployment checks; RSDL-801 to 803 are reserved or rejected)                                                                                                                                                                                                                                                             |
| Name          | `link-encoding-not-emitted`                                                                                                                                                                                                                                                                                                                                                       |
| Default level | warn                                                                                                                                                                                                                                                                                                                                                                              |
| Summary       | a link of the selected deployment carries an encoding the emitted backend's `encodings` list does not name                                                                                                                                                                                                                                                                        |
| Raised by     | `ridl build` and `ridlc build`, in `run_build` after `select_deployment`, for each backend being emitted (a built-in emit or a plugin) — the place and the shape of RSDL-804 `unclaimed-backend-key`. Not by `ridl check`, the language server or `ridl_check`: without an emit there is no backend list to compare against (DD-S1-13).                                           |
| Location      | the consumer link's `requires` declaration in the rsdl source, located through the checked `system` that `run_build` already holds for `unclaimed_backend_keys` (RSDL-804); the lowered `v1::Deployment` carries no span, so the check walks the checked system's links and derives each link's encoding the way `encoding_of` in `crates/ridl-ir/src/codegen/deployment.rs` does |
| Message       | "link `<consumer instance> -> <provider instance>` of deployment `<name>` carries `proto3`, which `[backend.rust] encodings` does not name"                                                                                                                                                                                                                                       |
| Granularity   | one diagnostic per consumer link whose derived encoding (codegen-plugins.md, "The encoding rule": same machine FlatBuffers, different machine or off-board proto3) is not in the list; a link with an unspecified crossing draws nothing                                                                                                                                          |
| Types-only    | fires: a build with no `encodings` that selects a deployment warns for every link, which is the intended signal                                                                                                                                                                                                                                                                   |
| Lowering      | never blocks lowering, like RSDL-409 and RSDL-804 to 806; the rsdl reference's sentence listing them gains RSDL-807                                                                                                                                                                                                                                                               |
| `[lints]`     | `link-encoding-not-emitted = "allow"` silences it; `"deny"` makes it an error that stops the build                                                                                                                                                                                                                                                                                |

Registration follows ADR-0024: a `diag_codes!` row with
`lint = "link-encoding-not-emitted"` in `crates/ridl-core/src/diag.rs`, the name
map entry, a row in the table of `docs/book/lints.md`, and a row in the rsdl
reference's code table.

**Open question, recorded.** The lint derives a link's encoding from "The
encoding rule" of `docs/design/codegen-plugins.md` (same machine FlatBuffers,
other crossings proto3), while section 7.1 gives each transport its own default
(shared memory `repr(C)`). The two are aligned when `repr(C)` or a shared-memory
transport lands; until then the rule stands and the lint uses it.

## 9. Migration

### 9.1 `examples/cabin`

- `examples/cabin/ridl.toml` gains `[backend.rust]` with
  `encodings = ["flatbuffers"]` (the one line A1 priced).
- `examples/cabin/consumer/src/main.rs` needs no edit in either sub-stage. Its
  five face calls (lines 153-158: `Loopback::new(CATALOG)`, two `attach()`
  bindings, a `blocking::Client::new(rt.attach())`, `rt.handler()`) and its two
  `serve` calls name no encoding today and keep compiling when the encoding
  comes from the port: `Loopback::new` yields the FlatBuffers-typed loopback,
  and `Client::new(rt.attach())` infers the rest (section 2, fact 1, the 1.83
  trial).
- `examples/cabin/Cargo.toml`'s comment and the generated-manifest guard test in
  `crates/ridlc/tests/` move from `"0.7"` to `"0.8"` at release time, with the
  other pins the release procedure bumps.

### 9.2 `just demo`, `just compat-check`

Both run `ridl build examples/cabin --emit rust`; neither recipe changes. The
second build of `demo`, over `crates/ridlc/tests/corpus/veh-cluster`, checks a
generated crate for a target with no standard library; that corpus's root
`ridl.toml` gains the same table so the check keeps covering the codec. Tests
under `crates/ridlc/tests/` that snapshot a build of that corpus are re-recorded
once.

### 9.3 Tests and fixtures in the workspace

`crates/ridl-backend-rust/tests/generated/interaction_face.rs` is regenerated
(1a: the size table and the moved constants; 1b: the generic face). Tests that
call `generate_pipeline(.., WireEncoding::FlatBuffers, ..)` pass
`&[WireEncoding::FlatBuffers]`. `the_default_wire_encoding_is_flatbuffers`
becomes `no_encodings_emits_types_only`. `crates/ridl-rt/tests/budget.rs` and
`descriptors.rs` move to the size table. The hand-written fake ports of
`crates/ridl-backend-rust/tests/interaction_face.rs`
(`ReportsInitOverRealBytes`, `MinimalSignalOnlyPort`, `DistinctiveInitPort`)
gain `impl Encoded { type Encoding = FlatBuffers; }` in 1b, and the `Raw`
placeholder of `crates/ridl-rt/src/payload.rs` gains `Buffer = [u8; 0]` in 1a.

### 9.4 The book

Chapters that state the old behaviour: `docs/book/cli-reference.md` (lines
around 618 and 730-734: "no flag for the payload encoding"), `introduction.md`
(42-44), `generated-code.md` (56 and 229, the `flatbuffers` feature line, and 38
for `Bind`), `lints.md` (the new row), and the manifest chapter's table list. A
`toml` fence is not compiled by `book_examples.rs`; the `ridl` fences are
unchanged.

### 9.5 The Kotlin heads-up

An issue on driftsys/ridlc-gen-kotlin, filed by the plan's last task, stating:
(1) the host now sends `encodings=<names joined by ",">` from `[backend.kotlin]`
and never `wire-encoding`; today's `Options.kt` would refuse the new key, so the
plugin should read `encodings` and may keep `flatbuffers` as its only accepted
value; (2) the rule A1 (types only when nothing is named) is the family's and
the plugin's fixtures gain `[backend.kotlin] encodings = ["flatbuffers"]` if the
plugin adopts it; (3) `ridl-rt` 0.8.0 replaces `EncodedSizes`,
`PayloadInfo.maxSize` and `Encoding.maxSize` by a per-codec `Sizes<E>` table and
infallible `reservation`/`tableBudget`, gives each `Payload<E>` its own buffer,
and makes the encoding a property of the port type (`Encoded`), for `ridl-rt-kt`
to mirror; (4) the codegen model and the catalog descriptor are unchanged, so
`Payload.flatbuffersMaxSize` stays.

## 10. Records amended

| Record                                                                           | Change                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| -------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| ADR-0018 (Proposed)                                                              | Status: dated amendment line. Decision 4 and 5: the Rust codec is selected by `[backend.rust] encodings`, not by `--wire`; `--wire proto` is no longer the form a codec request takes; the schema emits stay `--emit`. Decision 15: the face is generic over the encoding as well as over its ports.                                                                                                                                          |
| ADR-0020 (Proposed)                                                              | Decision 5: `contract` loses `EncodedSizes` and `Unsized`, gains `Sizes<E>`; a generated package enables one `ridl-rt` feature per named encoding and none when types only. Decision 9: the `encodings` option convention. "Documents amended" table: rows for ADR-0021, ADR-0023, `interaction-face.md`, `ridl-rt.md`.                                                                                                                       |
| ADR-0021 (Accepted)                                                              | Decision 10: a dated note that this change ships as 0.8.0 and lists section 6.3. Decision 17: `Encoding::max_size`, `Member::reservation` and `table_budget(&[Member])` replaced. The port contract: the `port` module gains `Encoded`, implemented by every port a face binds to, with the `&mut` forwarding impl; `Payload<E>` gains `Buffer`/`BUFFER`. Decision 19: `Bind` unchanged; the generated face reads the encoding from its port. |
| ADR-0023 (Accepted)                                                              | Decision 2's consequence note: `ridl build --emit rust` emits the face only when an encoding is named. Decision 6: `Client<P>` with `P: Encoded`, the encoding from `P::Encoding`, the bound discipline; application code names no encoding. Lines 584-589: the encoding is the port's, not one path. Decision 8 unchanged.                                                                                                                   |
| ADR-0002 (Accepted)                                                              | Status: amendment line. §4: a "`[backend.<name>]` tables" paragraph in the form of the `[codegen]` one; MANI-015, MANI-016.                                                                                                                                                                                                                                                                                                                   |
| ADR-0024 (Accepted)                                                              | No decision changes. The lint table gains RSDL-807.                                                                                                                                                                                                                                                                                                                                                                                           |
| `docs/design/ridl-rt.md`                                                         | `contract`, `payload` and `port` item lists; the `PayloadInfo`/`EncodedSizes` passage; the reservation and budget section, with the registration pattern of section 6.2; `Payload::Buffer`; `Encoded` and the transport-default table of section 7.1; `Encoding` without `max_size`; "Versioning" corrected to the shared version.                                                                                                            |
| `docs/design/interaction-face.md`                                                | "The face gains no type parameter" rewritten: the face still gains none, and reads the encoding from `P::Encoding`; the `PayloadInfo.max_size` passage; rule 3 ("No flag selects the encoding, yet") replaced by the manifest rule and the port rule; the provisional table's size rows; buffer sizing through `Payload::BUFFER` and `Sizes<E>`.                                                                                              |
| `ridl-loopback` rustdoc and `docs/book/writing-a-port.md`                        | `Loopback<E = FlatBuffers>`, `with_encoding`, the `Encoded` impl a port writes (1b).                                                                                                                                                                                                                                                                                                                                                          |
| `docs/design/flatbuffers-codec.md`                                               | "Every decision is built" paragraph: the codec is emitted on request; the `WireEncoding` row; the "one alias" section retitled to the generic face.                                                                                                                                                                                                                                                                                           |
| `docs/design/codegen-plugins.md`                                                 | "Options": `encodings` replaces `wire-encoding`; one request per (package, backend). "The encoding rule": unchanged, cited by the lint.                                                                                                                                                                                                                                                                                                       |
| `docs/design/catalog-descriptor.md`                                              | Line citing `EncodedSizes` for `bytes`; the two-sums passage now cites `Sizes<E>` and the two functions, and drops the "`Unsized` for every member" sentence.                                                                                                                                                                                                                                                                                 |
| `docs/specification/ridl-family-overview.md`                                     | §7 MANI table: MANI-015, MANI-016.                                                                                                                                                                                                                                                                                                                                                                                                            |
| `docs/specification/rsdl-language-reference.md`                                  | Lint sentence and code table: RSDL-807; a sentence under "backend keys" separating the manifest table from the attribute namespace.                                                                                                                                                                                                                                                                                                           |
| `docs/book/lints.md`, `cli-reference.md`, `introduction.md`, `generated-code.md` | Section 9.4.                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| `docs/technotes/walking-skeleton-architecture.md`                                | `ridl-core` row: the manifest's `[backend.<name>]` table; `ridl-backend-rust` row: emits the codecs the request names.                                                                                                                                                                                                                                                                                                                        |
| `docs/ROADMAP.md`                                                                | The sentence "`proto3` stays `None` because the Rust backend emits no proto3 codec" (size rows) reworded; the proto3 and `repr(C)` codec rows note that each arrives as a `Sizes<E>` table.                                                                                                                                                                                                                                                   |

## 11. Testing

Every test below names the wrong behaviour it fails on; a test that only fails
to compile is not enough.

**Manifest (`crates/ridl-core`).** One test per MANI-015 trigger (not an array,
an unknown name, a duplicate, `backend` not a table) asserting the code, the
message text and the span; a mutation that drops the duplicate check fails
`duplicate_encoding_is_mani_015`. MANI-016 for a member manifest, and no
MANI-016 for the root. `[backend.rust] foo = 1` is MANI-005. A valid table
parses to `["flatbuffers", "proto3"]` in order (a mutation that sorts fails).

**Request (`crates/ridlc`).** `codegen_request` carries
`encodings=flatbuffers,proto3` for the `rust` request and no option for a
`kotlin` plugin's request from the same manifest; two parity tests
(`ridlc-gen-rust`, `ridlc-gen-model`) keep passing with an empty vector. A build
with `[backend.kotlin]` only sends the Rust backend nothing.

**Backend (`crates/ridl-backend-rust`).** Snapshot of a types-only build
contains no `Payload<`, no `impl ::ridl_rt::contract::Interface`, no
`pub mod cabin`; snapshot with `["flatbuffers"]` equals today's output plus the
size table minus the removed constants; `wire-encoding` is refused with the
message naming `encodings`; `proto3` is refused with the message naming
driftsys/ridl#264; `flatbuffers,flatbuffers` and `flatbuffers,` are refused; the
generated `Cargo.toml` has `features = ["flatbuffers"]` for one name and no
`features` for none (a mutation that always writes the feature fails the
second).

**Size table.** On the checked-in fixture,
`<Cabin as Sizes<FlatBuffers>>::RESERVATIONS` is compared row by row with values
the test computes itself from the planus oracle's bounds
(`flatbuffers_conformance.rs` already verifies `MAX_SIZE` against planus): the
query row equals argument plus reply, with two payloads of different sizes, so a
mutation that emits `max` or the first payload fails;
`table_budget::<Cabin, FlatBuffers>()` equals the sum of rows over at least
three members of distinct sizes, so a mutation returning a row or a maximum
fails; `reservation::<CabinAverage, FlatBuffers>()` equals its row and differs
from its neighbours' rows; `CLAIM_BUFFER.len() == MAX_BUFFER_SIZE` and
`MAX_BUFFER_SIZE` equals the largest call payload's `MAX_SIZE` and is not the
largest event's (fixture with a larger event payload);
`EVENT_BUFFER.len() == EVENT_SOURCE_BUFFER_SIZE`. `Interaction::ROW` of each
descriptor indexes its own `MEMBER` (compare ordinals).

**Payload buffers.** For every payload type of the fixture,
`<T as Payload<FlatBuffers>>::BUFFER.len() == T::MAX_SIZE` and every byte is 0;
the fixture has two payloads of different `MAX_SIZE`, so an emitter that writes
one size for all fails; the smallest payload's buffer is smaller than
`MAX_BUFFER_SIZE`, so a face that took the interface maximum fails.

**Face buffers.** The largest call payload round-trips through `serve`; a
provider-side buffer of `MAX_BUFFER_SIZE - 1` bytes is refused by `dispatch`
(the existing `buf.len() < MAX_BUFFER_SIZE` branch), so a mutation that sizes
the claim buffer from the wrong constant fails. The fixture's emitted source
contains no `MAX_SIZE]` array length.

**Generic face (1b).** A compile-and-run test hand-implements `Payload<Proto3>`
(with `Buffer`) and `Sizes<Proto3>` for the fixture's types with a codec whose
bytes differ from FlatBuffers (for example the FlatBuffers bytes reversed),
builds `Loopback::<Proto3>::with_encoding(CATALOG)` and binds
`cabin::Client::new(rt.attach())` and
`cabin::serve(&mut handler, &mut provider)` with no type annotation: the round
trip succeeds and the bytes observed on the port are the reversed ones, so a
face that still encodes with FlatBuffers somewhere fails. The same test binds a
client over `Loopback::new(CATALOG)` beside it, so both codecs coexist in one
crate and the default is FlatBuffers. A client over
`Loopback::<ReprC>::with_encoding(..)` fails to compile (a `compile_fail`
doctest on `Cabin: Sizes<ReprC>`). The cabin program compiles with no edit
(`just demo`).

**Lint.** A system with one different-machine link and `["flatbuffers"]` draws
exactly one RSDL-807 at the link's span with the message of section 8; the same
system with a same-machine link draws none;
`[lints] link-encoding-not-emitted = "allow"` draws none and `"deny"` fails the
build with exit code 1; `ridl check` on the first system draws none; a
types-only build that selects the deployment draws one per link. A mutation that
compares against the wrong backend's list fails the test that gives
`[backend.kotlin]` the encoding and emits `rust`.

**Gate.** `just build` members that apply to every task: `compile`, `test`,
`lint`, `fmt-check`, `wasm-check`; `compat-check` and `demo` for the backend,
manifest and cabin tasks; `book-check`, `link-check`, `doc-path-check`,
`story-id-check`, `check` for every docs task.

## 12. Ordering against concurrent work

Checked on 2026-10-10 against origin/main (376aa927):

| Work                                           | Touches                                                                                                                                                    | Ordering                                                                                                                                                                                                        |
| ---------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Lane H3, PR #787 (catalog compat)              | `crates/ridlc/src/lib.rs` (a `ridl_descriptor::lower` call), and uncommitted edits to `ridl-ir`'s codegen model, its proto and `ridlc`'s request snapshots | The request task of this stage (section 4.2) edits `write_emits` and `codegen_request` in the same file and re-records request snapshots: it starts after #787 merges, or rebases on it. Nothing else overlaps. |
| Lane G3, PR #800                               | `docs/book/lints.md` (one RIDL-417 row), `ridl-core` `diag.rs`                                                                                             | The lint task adds one RSDL-807 row and one catalogue entry; a textual merge. No ordering constraint.                                                                                                           |
| Lane G5 (gates)                                | `justfile` `gate-parity` member list                                                                                                                       | This stage adds no `build` member; no constraint.                                                                                                                                                               |
| #754 observation (`feat/754-propagation-hook`) | merged (#766, #769); both worktrees are ancestors of main                                                                                                  | None; the face this stage parameterises is the one on main.                                                                                                                                                     |
| PR #803 (steering note)                        | `docs/ROADMAP.md`, two wip files                                                                                                                           | This spec cites the steering note at its wip path; no file overlap.                                                                                                                                             |
| Stale branch `docs/e17-0-layout-inputs-design` | ADR-0018, ADR-0020 text                                                                                                                                    | Its content is on main (lane S); not rebased; ignored.                                                                                                                                                          |

No open work touches `crates/ridl-rt`, `crates/ridl-backend-rust`,
`crates/ridl-loopback`, `crates/ridl-core/src/manifest.rs`, `examples/cabin` or
the design records this stage amends. Moving every `ridl-rt` change into
sub-stage 1a changes no ordering: 1b now touches `ridl-loopback`, which no open
work edits either.

## 13. Scope verdict and sub-stages

The stage is too large for one plan under the per-task review gate: the generic
face alone touches the 58 sites that name FlatBuffers in the fixture, every
future, the blocking module, `serve`, the cabin program and the book. It is
split into two ordered sub-stages (DD-S1-14):

- **Sub-stage 1a — on-request emission, the size table and the whole `ridl-rt`
  break.** Sections 3, 4, 5, 6 (every `ridl-rt` row of 6.3, `Payload::Buffer`
  and `port::Encoded` included), 8, 9.1 to 9.5 (except the 1b items), 10 (except
  the ADR-0023 decision 6 amendment and the "no type parameter" rewrite, which
  describe 1b). The face stays monomorphic over the one codec, reading every
  buffer through `Payload<FlatBuffers>::BUFFER` or `Sizes<FlatBuffers>` (section
  7.4). It is releasable alone as 0.8.0 with the complete `ridl-rt` breaking
  list; `Encoded` ships with no implementor in the workspace until 1b.
- **Sub-stage 1b — the face reads the encoding from the port.** Section 7: the
  generated face (`Encoded` bounds, `P::Encoding` in place of the FlatBuffers
  path), `ridl-loopback` (`Loopback<E = FlatBuffers>`, `HandlerHandle<E>`,
  `with_encoding`, the `Encoded` impls), the fake ports of the face tests, the
  remaining records. No `ridl-rt` change. `ridl-rt-conformance` is unaffected
  (section 6.3).

Target: both in the 0.8.0 release. If 1b slips past the release, 0.8.0 ships
`Encoded` with no port implementing it, and the generated face's `Encoded` bound
becomes a second pre-1.0 break of the generated API when 1b lands; the cabin
program needs no edit in either case. The plan written with this spec covers
sub-stage 1a only; 1b gets its own plan from section 7 when 1a has merged.

## 14. Alternatives considered

- **One face per encoding** (a `cabin::flatbuffers::Client` module per named
  codec instead of a type parameter): no generic-const problem and no buffer
  types, but n copies of the futures, the dispatch and the blocking module, and
  a path that changes shape between one and two encodings. Rejected by A3, which
  fixes the generic face; its generic-const cost is paid once in
  `Payload::Buffer` and `Sizes<E>`.
- **Naming `E` at each binding** (`Client::<FlatBuffers, _>::new(port)`,
  `serve::<FlatBuffers, _, _>(h, p)`, the first draft of this spec): one
  turbofish per binding, and application code would name the wire encoding,
  which is the transport's property. Rejected on review (DD-S1-11 revised).
- **Passing the encoding as a value** (`Client::new(port, FlatBuffers)`,
  `serve(h, p, FlatBuffers)`, with `Bind<E>` a generic trait): inference without
  a turbofish, but a marker argument on every binding, a change to `face::Bind`,
  and application code still names the encoding. Rejected.
- **Default type parameters on `Client`** (`Client<P, E = FlatBuffers>`): do not
  help; Rust ignores a type parameter's default during expression inference, so
  `Client::new(port)` would still leave `E` unresolved. Rejected. (A default on
  `Loopback<E = FlatBuffers>` works because `new` is defined in the
  `impl Loopback<FlatBuffers>` block, which fixes `E`.)
- **A build-wide default encoding in the manifest** (a `DefaultEncoding` alias
  emitted by the package): a transport crate, which is where the encoding is
  chosen, cannot read a package's manifest. Rejected.
- **A cargo feature for a transport's default encoding**: features must be
  additive, and a default is a choice between alternatives. Rejected.
- **`impl Trait` arguments on `serve`** so that `serve::<E>` names `E` alone:
  requires `Serve` to become an unnameable `impl Future`, which an embedded
  program that stores the future cannot hold. Rejected; moot once the port
  carries the encoding.
- **Interface-maximum buffers on the client side** (every face buffer a
  `Sizes<E>` buffer, the first draft of this spec): fewer items, but a call's
  stack use grows to the interface's largest payload. Rejected on review for
  `Payload::Buffer` (DD-S1-10 revised); the two buffers that must hold any
  payload stay in `Sizes<E>` (section 6.1).
- **A side table of sizes behind a feature flag** (a second, data-driven copy of
  the sizes for a runtime with no generated code): rejected now. `RESERVATIONS`
  is already the data form (a `&'static [u64]`), a second copy reintroduces the
  drift #350 item 13 closed, no consumer exists, and a feature doubles the build
  matrix. If a consumer appears, an additive generated
  `fn reservations(encoding: &str) -> Option<&'static [u64]>` per interface
  returns the same slices by name.
- **Keeping `EncodedSizes` beside the table** so that a consumer reads every
  encoding from one row: the row's `None` would keep two meanings and the
  descriptor already keeps every column (A5). Rejected.
- **Validating `encodings` in the backend only**: the name set is the
  toolchain's and the lint needs the parsed list before any backend runs.
  Rejected; both layers validate, each its own concern (DD-S1-4).
- **A repeated `encodings` key in the request** instead of a joined value: the
  request's keys are unique by contract. Rejected.
- **A new request field** (`repeated string encodings`): additive but
  backend-agnostic, where A2 asks per backend. Rejected.
- **Passing every key under `[backend.<name>]` through as an option**: would let
  a plugin's own keys (`kotlin-package`) be set from the manifest, but is not
  asked for and makes the manifest validate nothing. Not done now; adding it
  later is additive (DD-S1-2).
- **Descriptors in a types-only build**: possible once sizes leave `MEMBERS`,
  but A1 says "nothing else", and a descriptor without a face has no reader.
  Rejected (DD-S1-7).
- **A command-line `--encodings` flag beside the manifest**: A2 says none now;
  additive later under ADR-0010 (DD-S1-6).

## 15. Decisions taken in this session, for Sebastien to confirm

Reviewed with Sebastien on 2026-10-10: DD-S1-1 to DD-S1-9, DD-S1-12, DD-S1-13
and DD-S1-14 to DD-S1-22 are confirmed as written; DD-S1-10 and DD-S1-11 are
revised as the table states.

| Id       | Decision                                                                                                                                                                                                                                                                                                                                                                                                                                                                             | Alternatives                                                                                            | Cost if wrong                                                                                                                  |
| -------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------ |
| DD-S1-1  | Confirmed by Sebastien 2026-10-10. "`ridl-rt` 0.2" is read as the next breaking workspace minor, 0.8.0; `ridl-rt` keeps sharing the workspace version                                                                                                                                                                                                                                                                                                                                | Give `ridl-rt` its own version line again                                                               | A release-procedure note; no code                                                                                              |
| DD-S1-2  | Confirmed by Sebastien 2026-10-10. `[backend.<name>]` accepts any name; the one key is `encodings`; other keys draw MANI-005; a table for a backend not emitted is silent                                                                                                                                                                                                                                                                                                            | Only known names; pass every key through as an option                                                   | A typo in `<name>` is silent until a build emits that backend; adding pass-through later is additive                           |
| DD-S1-3  | Confirmed by Sebastien 2026-10-10. `[backend.<name>]` is root-only (MANI-016), like `[codegen]`                                                                                                                                                                                                                                                                                                                                                                                      | Allow per member, last wins                                                                             | A member that wants its own codec set cannot have it; lifting the rule is additive                                             |
| DD-S1-4  | Confirmed by Sebastien 2026-10-10. `ridl-core` validates `encodings` against the toolchain's closed set (MANI-015); the backend refuses what it has no codec for                                                                                                                                                                                                                                                                                                                     | Backend-only validation                                                                                 | One error path duplicated in two layers                                                                                        |
| DD-S1-5  | Confirmed by Sebastien 2026-10-10. One option `encodings` with a `,`-joined value, in the request of that backend only; one request per (package, backend); `wire-encoding` retired                                                                                                                                                                                                                                                                                                  | Repeated keys; a new request field; one shared request                                                  | A plugin parses a joined string; a request clone per backend                                                                   |
| DD-S1-6  | Confirmed by Sebastien 2026-10-10. A single-file build is types only; a flag comes later under ADR-0010                                                                                                                                                                                                                                                                                                                                                                              | Add the flag now                                                                                        | Single-file builds cannot carry a codec until then                                                                             |
| DD-S1-7  | Confirmed by Sebastien 2026-10-10. Types only emits no descriptors, no size table and no face                                                                                                                                                                                                                                                                                                                                                                                        | Emit the (now size-free) descriptors always                                                             | A consumer wanting `Interface::MEMBERS` without a codec must name one                                                          |
| DD-S1-8  | Confirmed by Sebastien 2026-10-10. The Rust backend refuses `proto3` and `repr-c` with an error naming #264 / #317; the list is validated whole                                                                                                                                                                                                                                                                                                                                      | Emit what it can and warn                                                                               | A build naming a future codec fails instead of degrading                                                                       |
| DD-S1-9  | Confirmed by Sebastien 2026-10-10. `Sizes<E>` shape of section 6.1; `Interaction::ROW`; `reservation::<X, E>()` and `table_budget::<I, E>()` as infallible `const fn`; `Unsized` removed; `PayloadInfo` keeps `type_name`                                                                                                                                                                                                                                                            | Keep `Member::reservation` with a scan by ordinal; a `MemberSize` row struct                            | A runtime holding `&Member` dynamically cannot size it without the interface type                                              |
| DD-S1-10 | Revised by Sebastien 2026-10-10. `Payload<E>` gains `type Buffer` (`[u8; MAX_SIZE]`) and `const BUFFER`; every typed face site holds the payload's own buffer; `Sizes<E>` keeps `RESERVATIONS`, `MAX_BUFFER_SIZE`, `EVENT_SOURCE_BUFFER_SIZE` and the two buffers that are filled before the type is known, `ClaimBuffer`/`CLAIM_BUFFER` (server claim) and `EventBuffer`/`EVENT_BUFFER` (next event)                                                                                | Interface-maximum buffers everywhere (the first draft); one buffer type per payload inside `Sizes<E>`   | Two trait items more than the review asked for, kept because `dispatch` and `EventSource::next` read before they know the type |
| DD-S1-11 | Revised by Sebastien 2026-10-10. The encoding comes from the port: `ridl_rt::port::Encoded { type Encoding }`, implemented by every port; the face takes `P::Encoding` and gains no type parameter; application code names no encoding; each transport has a default from the coupling of its ends (loopback FlatBuffers, WebSocket proto3, shared memory `repr(C)`), overridable at construction; #265 lands with `connect_with::<FlatBuffers>` and gains `connect` when #264 lands | `E` named at each binding (the first draft); a value argument; a manifest-wide default; a cargo feature | A port type implementing `Encoded` per transport; `ridl-loopback` typed by `E`                                                 |
| DD-S1-12 | Confirmed by Sebastien 2026-10-10. Payload bounds are generated `where` clauses on every impl and function of the face                                                                                                                                                                                                                                                                                                                                                               | A per-interface codec trait that dispatches every payload                                               | Long generated signatures; a consumer generic over `E` restates them                                                           |
| DD-S1-13 | Confirmed by Sebastien 2026-10-10. RSDL-807 `link-encoding-not-emitted`, warn, raised by `ridl build`/`ridlc build` only, one per consumer link, at the link; fires for types-only builds; unspecified crossing draws nothing                                                                                                                                                                                                                                                        | A MANI code at the manifest; one diagnostic per encoding; also raised by `ridl check`                   | A noisy warning set to `allow` (A4's own cost)                                                                                 |
| DD-S1-14 | Confirmed by Sebastien 2026-10-10, with the scope moved as the review asked. Two sub-stages, 1a (emission, size table, lint, the complete `ridl-rt` 0.8 break including `Payload::Buffer` and `Encoded`) then 1b (the face reads the encoding from the port; `ridl-loopback` typed); plan 1a now; both aimed at 0.8.0                                                                                                                                                                | One plan; 1b first                                                                                      | If 1b slips, a second generated-API break after 0.8.0                                                                          |
| DD-S1-15 | Confirmed by Sebastien 2026-10-10. `--emit flatbuffers` and `--emit proto` unchanged                                                                                                                                                                                                                                                                                                                                                                                                 | Tie a schema emit to `encodings`                                                                        | None                                                                                                                           |
| DD-S1-16 | Confirmed by Sebastien 2026-10-10. `WireEncoding` keeps its name, loses `Default`, gains `parse`; the pipeline takes `&[WireEncoding]`; `generate_face` removed                                                                                                                                                                                                                                                                                                                      | Rename to `Codec`; a set type                                                                           | A rename later                                                                                                                 |
| DD-S1-17 | Confirmed by Sebastien 2026-10-10. The codegen model and the catalog descriptor are unchanged (`PayloadSizes`, `flatbuffers_max_size`, every `max_sizes` column)                                                                                                                                                                                                                                                                                                                     | Drop `flatbuffers_max_size`                                                                             | None now; the Kotlin plugin depends on the field                                                                               |
| DD-S1-18 | Confirmed by Sebastien 2026-10-10. The Kotlin heads-up (section 9.5) is filed by the plan's last task, not by this session                                                                                                                                                                                                                                                                                                                                                           | File it now                                                                                             | One day of notice                                                                                                              |
| DD-S1-19 | Confirmed by Sebastien 2026-10-10. The lint compares against the table of each backend the build emits (built-in or plugin)                                                                                                                                                                                                                                                                                                                                                          | Only `[backend.rust]`                                                                                   | None                                                                                                                           |
| DD-S1-20 | Confirmed by Sebastien 2026-10-10. The request task starts after PR #787 merges or rebases on it; the lint row merges textually with #800                                                                                                                                                                                                                                                                                                                                            | Start now and resolve conflicts                                                                         | A rebase                                                                                                                       |
| DD-S1-21 | Confirmed by Sebastien 2026-10-10. `MAX_BUFFER_SIZE` and `EVENT_SOURCE_BUFFER_SIZE` move from inherent constants to `Sizes<E>`                                                                                                                                                                                                                                                                                                                                                       | Keep inherent aliases for the one-codec case                                                            | A consumer that named them writes `<Cabin as Sizes<FlatBuffers>>::MAX_BUFFER_SIZE`                                             |
| DD-S1-22 | Confirmed by Sebastien 2026-10-10. `docs/design/ridl-rt.md`'s stale "own version" sentence is corrected in this stage                                                                                                                                                                                                                                                                                                                                                                | Leave it                                                                                                | None                                                                                                                           |
