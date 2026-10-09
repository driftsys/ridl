# On-request encodings — design (stage 1 of the WebSocket and payload formats steering)

Date: 2026-10-10. Status: draft for review, written while the maintainer was
away; every choice the steering note did not fix is listed in section 15.

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
  several codecs and the program picks one per port (A3);
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

| Fixed by | Content                                                                   | This spec adds                                                              |
| -------- | ------------------------------------------------------------------------- | --------------------------------------------------------------------------- |
| A1       | Types only by default: no codec, no face                                  | What "nothing else" covers (section 5.1), the generated manifest's features |
| A2       | `[backend.rust] encodings = [...]`; no command-line flag; option replaced | Schema, validation, diagnostics, the request mapping (sections 3 and 4)     |
| A3       | `Bind<E>`, `serve::<E>`; every named codec emitted; the program picks `E` | The exact signatures and the bound discipline (section 7)                   |
| A4       | A warning under ADR-0024, lowerable in `[lints]`                          | Code, name, level, command, location (section 8)                            |
| A5       | One size table per emitted codec; `ridl-rt` breaking release              | The trait, its items, what replaces each removed item (section 6)           |

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
   concrete `E` is accepted. So the per-codec size table of A5 is also where the
   generic face of A3 takes its buffer types from: the two decisions are one
   trait (section 6.1).
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
face are emitted once, generic over `E` (section 7). The `Cargo.toml` enables
one feature per name.

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

### 6.1 The `Sizes<E>` trait

In `ridl_rt::contract`:

```rust
/// The size table of an interface in encoding `E`: what a face and a runtime
/// need to hold one payload or one table of calls in flight in that encoding.
/// Generated code implements it once per interface per codec the package
/// carries; every number derives from `<T as Payload<E>>::MAX_SIZE`, which is
/// the one source of a payload's bound.
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
    /// `[u8; Self::MAX_BUFFER_SIZE]`, stated as a type so that code generic
    /// over `E` can hold one on the stack (a generic array length is not
    /// expressible on stable Rust).
    type CallBuffer: AsRef<[u8]> + AsMut<[u8]> + Copy;
    /// `[u8; Self::EVENT_SOURCE_BUFFER_SIZE]`, likewise.
    type EventBuffer: AsRef<[u8]> + AsMut<[u8]> + Copy;
    /// A zeroed `CallBuffer`.
    const CALL_BUFFER: Self::CallBuffer;
    /// A zeroed `EventBuffer`.
    const EVENT_BUFFER: Self::EventBuffer;
}
```

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
    type CallBuffer = [u8; Self::MAX_BUFFER_SIZE];
    type EventBuffer = [u8; Self::EVENT_SOURCE_BUFFER_SIZE];
    const CALL_BUFFER: Self::CallBuffer = [0u8; Self::MAX_BUFFER_SIZE];
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
`const fn` so that a runtime may size a static table (DD-S1-9). The trait, the
generated implementation, both `const fn`s and a generic function holding a
`CALL_BUFFER` were compiled together on the pinned toolchain on 2026-10-10 (a
forty-line program): `reservation` of a query row with payloads of 8 and 24
bytes gave 32, the budget over two rows gave 40, and the buffer's length gave
the interface maximum, 24.

### 6.3 Breaking changes to `ridl-rt` (the full list)

All in the 0.8.0 release (ADR-0021 decision 10: a breaking `ridl-rt` change is a
0.x minor):

| Item                                                                         | Change                                                                                  | Replacement                                 |
| ---------------------------------------------------------------------------- | --------------------------------------------------------------------------------------- | ------------------------------------------- |
| `contract::EncodedSizes`                                                     | removed                                                                                 | `Sizes<E>::RESERVATIONS`                    |
| `contract::PayloadInfo::max_size`                                            | field removed; `PayloadInfo { type_name }` stays                                        | the size table                              |
| `contract::Member::reservation::<E>()`                                       | removed                                                                                 | `contract::reservation::<X, E>()`           |
| `contract::table_budget::<E>(&[Member])`                                     | removed                                                                                 | `contract::table_budget::<I, E>()`          |
| `contract::Unsized`                                                          | removed                                                                                 | none: a missing size is a compile error     |
| `contract::Interaction`                                                      | gains `const ROW: usize` (breaking for implementors; only generated code implements it) |                                             |
| `contract::Sizes<E>`                                                         | added                                                                                   |                                             |
| `encoding::Encoding::max_size`                                               | removed; the trait keeps `NAME` and stays sealed                                        |                                             |
| Generated interface descriptor `MAX_BUFFER_SIZE`, `EVENT_SOURCE_BUFFER_SIZE` | inherent constants removed from the generated crate                                     | `<Iface as Sizes<E>>::MAX_BUFFER_SIZE` etc. |
| Generated face types                                                         | gain the encoding as their first type parameter (section 7; sub-stage 1b)               |                                             |
| Generated `Cargo.toml`                                                       | `ridl-rt = "0.8"`; features follow `encodings`                                          |                                             |

Not changed: `face::Bind`, `face::Events`, `face::Timeout`, `face::Publish`, the
`port` traits, `payload::Payload<E>` and `MAX_SIZE`, the three marker types, the
cargo features, `rust-version = "1.83"`, the codegen build matrix of ADR-0021
decision 10 (every new construct here — associated array types, `const fn` with
a loop, `as u64` casts in a const — is stable in Rust 1.83 and edition 2021),
`ridl-rt-conformance` (reads no size), `ridl-loopback` (calls neither budget
function).

`docs/design/ridl-rt.md`'s "Versioning" section still says the crate "carries
its own version, independent of the workspace's", which ADR-0021 decision 10's
2026-09-21 amendment superseded; this stage corrects it (DD-S1-22).

## 7. The generic face (sub-stage 1b)

### 7.1 Signatures

The encoding is the first type parameter of every generic face item, so that a
program names it once and the port type is inferred (DD-S1-11). `Bind` in
`ridl_rt::face` is unchanged: `E` lives on the bound type, and `Bind::new` keeps
`fn new(port: Self::Port) -> Self`. Per interface module (`cabin`):

```rust
pub struct Client<E, P>
where
    E: ::ridl_rt::encoding::Encoding,
    P: SignalReader + EventSource + Caller + Clock + Wakeable,   // as today
{ port: P, encoding: ::core::marker::PhantomData<E> }

impl<E, P> ::ridl_rt::face::Bind for Client<E, P>
where E: Encoding, P: /* as today */,
      super::Cabin: ::ridl_rt::contract::Sizes<E>,
      Temperature: Payload<E>, Warning: Payload<E>, Level: Payload<E>,
      Window: Payload<E>, Average: Payload<E>,                      // every payload of the interface
{ type Port = P; fn new(port: P) -> Self }

pub struct Publisher<E, W> where E: Encoding, W: SignalWriter + EventSink { ... }

pub fn serve<E, H, P>(h: H, p: &mut P) -> Serve<'_, E, H, P>
where
    E: Encoding,
    H: Handler + Wakeable,
    P: Provider,
    super::Cabin: Sizes<E>,
    /* every payload: Payload<E> */;

pub struct Serve<'a, E, H, P> { handler: H, provider: &'a mut P,
    buf: <super::Cabin as Sizes<E>>::CallBuffer, ... }

pub mod blocking {
    pub struct Client<E, P> ...;
    pub fn serve<E, H, P>(h: H, p: &mut P, timeout: Option<Duration>)
        -> Result<(), ProviderError> where /* as above */;
}
```

The futures (`SetLevelCall<'a, E, P>`, `AverageCall<'a, E, P>`,
`NextEvent<'a, E, P>`) gain `E` first too. `Provider`, `Subscribe`,
`Invalidate`, the descriptors and `prelude` do not change: they name domain
types only. A program writes:

```rust
let mut publisher = cabin::Publisher::<FlatBuffers, _>::new(rt.attach());
let mut client = cabin::Client::<FlatBuffers, _>::new(rt.attach());
let serve = cabin::serve::<FlatBuffers, _, _>(&mut handler, &mut provider);
```

or names the type on the `let`
(`let client: cabin::Client<FlatBuffers, _> = Client::new(..)`). The encoding
cannot be inferred from the port, because a port carries bytes (ADR-0020: the
transport is encoding-agnostic), so one annotation per binding is the floor.

### 7.2 Bounds

Generic code that encodes or decodes a payload `T` needs `T: Payload<E>`, and a
trait's `where` clause is not assumed by its users on stable Rust, so the bounds
are written out: each `impl` and each free function of the face carries, as a
generated `where` clause, `Iface: Sizes<E>` and one `T: Payload<E>` per payload
type the interface uses (DD-S1-12). The consumer never writes them: it names a
concrete `E`, for which the emitted codec satisfies every bound. A consumer who
writes a helper generic over `E` restates them; that is the cost
`docs/design/interaction-face.md` ("The face gains no type parameter") named,
and it is paid by generated code, not by the program.

### 7.3 Buffers

Every buffer of the face is a `Sizes<E>` buffer: a call argument, a reply and
the server's claim buffer are `<Iface as Sizes<E>>::CALL_BUFFER`; an event
buffer is `EVENT_BUFFER`. A client-side buffer is therefore sized by the
interface's largest call payload, not by the payload's own `MAX_SIZE` as today
(DD-S1-10). The reason is fact 1 of section 2. The cost is stack bytes per
in-flight call up to the interface's largest payload; the server side already
pays it (`Serve::buf`).

### 7.4 Sub-stage 1a's face

Sub-stage 1a (section 13) keeps the face monomorphic over the one codec the Rust
backend has, but every size it reads already comes from
`<Iface as Sizes<::ridl_rt::encoding::FlatBuffers>>`, so that 1b replaces the
concrete path by `E` and adds the parameters and bounds, and nothing else.

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

## 9. Migration

### 9.1 `examples/cabin`

- `examples/cabin/ridl.toml` gains `[backend.rust]` with
  `encodings = ["flatbuffers"]` (the one line A1 priced).
- `examples/cabin/consumer/src/main.rs` (sub-stage 1b) names the encoding at its
  three bindings and two `serve` calls (section 7.1). Sub-stage 1a leaves it
  unchanged.
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
`descriptors.rs` move to the size table.

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
infallible `reservation`/`tableBudget`, for `ridl-rt-kt` to mirror; (4) the
codegen model and the catalog descriptor are unchanged, so
`Payload.flatbuffersMaxSize` stays.

## 10. Records amended

| Record                                                                           | Change                                                                                                                                                                                                                                                                                                                  |
| -------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| ADR-0018 (Proposed)                                                              | Status: dated amendment line. Decision 4 and 5: the Rust codec is selected by `[backend.rust] encodings`, not by `--wire`; `--wire proto` is no longer the form a codec request takes; the schema emits stay `--emit`. Decision 15: the face is generic over the encoding as well as over its ports.                    |
| ADR-0020 (Proposed)                                                              | Decision 5: `contract` loses `EncodedSizes` and `Unsized`, gains `Sizes<E>`; a generated package enables one `ridl-rt` feature per named encoding and none when types only. Decision 9: the `encodings` option convention. "Documents amended" table: rows for ADR-0021, ADR-0023, `interaction-face.md`, `ridl-rt.md`. |
| ADR-0021 (Accepted)                                                              | Decision 10: a dated note that this change ships as 0.8.0 and lists section 6.3. Decision 17: `Encoding::max_size`, `Member::reservation` and `table_budget(&[Member])` replaced. Decision 19: generated face types gain `E`; `Bind` unchanged.                                                                         |
| ADR-0023 (Accepted)                                                              | Decision 2's consequence note: `ridl build --emit rust` emits the face only when an encoding is named. Decision 6: `Client<E, P>`, `serve::<E, _, _>`, the bound discipline. Lines 584-589: the encoding is named by `E`, not by one path. Decision 8 unchanged.                                                        |
| ADR-0002 (Accepted)                                                              | Status: amendment line. §4: a "`[backend.<name>]` tables" paragraph in the form of the `[codegen]` one; MANI-015, MANI-016.                                                                                                                                                                                             |
| ADR-0024 (Accepted)                                                              | No decision changes. The lint table gains RSDL-807.                                                                                                                                                                                                                                                                     |
| `docs/design/ridl-rt.md`                                                         | `contract` and `encoding` item lists; the `PayloadInfo`/`EncodedSizes` passage; the reservation and budget section; `Encoding` without `max_size`; "Versioning" corrected to the shared version.                                                                                                                        |
| `docs/design/interaction-face.md`                                                | "The face gains no type parameter" rewritten to the generic face and its bounds; the `PayloadInfo.max_size` passage; rule 3 ("No flag selects the encoding, yet") replaced by the manifest rule; the provisional table's size rows; buffer sizing through `Sizes<E>`.                                                   |
| `docs/design/flatbuffers-codec.md`                                               | "Every decision is built" paragraph: the codec is emitted on request; the `WireEncoding` row; the "one alias" section retitled to the generic face.                                                                                                                                                                     |
| `docs/design/codegen-plugins.md`                                                 | "Options": `encodings` replaces `wire-encoding`; one request per (package, backend). "The encoding rule": unchanged, cited by the lint.                                                                                                                                                                                 |
| `docs/design/catalog-descriptor.md`                                              | Line citing `EncodedSizes` for `bytes`; the two-sums passage now cites `Sizes<E>` and the two functions, and drops the "`Unsized` for every member" sentence.                                                                                                                                                           |
| `docs/specification/ridl-family-overview.md`                                     | §7 MANI table: MANI-015, MANI-016.                                                                                                                                                                                                                                                                                      |
| `docs/specification/rsdl-language-reference.md`                                  | Lint sentence and code table: RSDL-807; a sentence under "backend keys" separating the manifest table from the attribute namespace.                                                                                                                                                                                     |
| `docs/book/lints.md`, `cli-reference.md`, `introduction.md`, `generated-code.md` | Section 9.4.                                                                                                                                                                                                                                                                                                            |
| `docs/technotes/walking-skeleton-architecture.md`                                | `ridl-core` row: the manifest's `[backend.<name>]` table; `ridl-backend-rust` row: emits the codecs the request names.                                                                                                                                                                                                  |
| `docs/ROADMAP.md`                                                                | The sentence "`proto3` stays `None` because the Rust backend emits no proto3 codec" (size rows) reworded; the proto3 and `repr(C)` codec rows note that each arrives as a `Sizes<E>` table.                                                                                                                             |

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
from its neighbours' rows; `CALL_BUFFER.len() == MAX_BUFFER_SIZE` and
`MAX_BUFFER_SIZE` equals the largest call payload's `MAX_SIZE` and is not the
largest event's (fixture with a larger event payload). `Interaction::ROW` of
each descriptor indexes its own `MEMBER` (compare ordinals).

**Face buffers.** The largest call payload round-trips through `serve`; a
provider-side buffer of `MAX_BUFFER_SIZE - 1` bytes is refused by `dispatch`
(the existing `buf.len() < MAX_BUFFER_SIZE` branch), so a mutation that sizes
the buffer from the wrong constant fails.

**Generic face (1b).** A compile-and-run test hand-implements `Payload<Proto3>`
and `Sizes<Proto3>` for the fixture's types with a codec whose bytes differ from
FlatBuffers (for example the FlatBuffers bytes reversed) and binds
`Client<Proto3, _>` and `serve::<Proto3, _, _>` over the loopback: the round
trip succeeds and the bytes observed on the port are the reversed ones, so a
face that still encodes with FlatBuffers somewhere fails. The same test binds
`Client<FlatBuffers, _>` beside it, so both codecs coexist in one crate.
`Client<ReprC, _>` fails to compile (a `compile_fail` doctest on
`Cabin: Sizes<ReprC>`).

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
`crates/ridl-core/src/manifest.rs`, `examples/cabin` or the design records this
stage amends.

## 13. Scope verdict and sub-stages

The stage is too large for one plan under the per-task review gate: the generic
face alone touches the 58 sites that name FlatBuffers in the fixture, every
future, the blocking module, `serve`, the cabin program and the book. It is
split into two ordered sub-stages (DD-S1-14):

- **Sub-stage 1a — on-request emission and the size table.** Sections 3, 4, 5,
  6, 8, 9.1 to 9.5 (except the 1b items), 10 (except the ADR-0023 decision 6
  amendment and the "no type parameter" rewrite, which say "generic over `E`"
  only once 1b lands). The face stays monomorphic over the one codec, reading
  every size through `Sizes<FlatBuffers>` (section 7.4). It is releasable alone
  as 0.8.0 with the full `ridl-rt` breaking list of section 6.3 except the
  face-type row.
- **Sub-stage 1b — the generic face.** Section 7, the cabin program, the
  remaining records. No `ridl-rt` API change: `Sizes<E>` already carries what
  the generic code needs.

Target: both in the 0.8.0 release. If 1b slips past the release, the face's type
parameters become a second pre-1.0 break of the generated API, and the cost is
one more Kotlin heads-up and one more cabin edit. The plan written with this
spec covers sub-stage 1a only; 1b gets its own plan from section 7 when 1a has
merged.

## 14. Alternatives considered

- **One face per encoding** (a `cabin::flatbuffers::Client` module per named
  codec instead of a type parameter): no generic-const problem and no `Sizes<E>`
  buffer types, but n copies of the futures, the dispatch and the blocking
  module, and a path that changes shape between one and two encodings. Rejected
  by A3, which fixes the generic face; its generic-const cost is paid once in
  `Sizes<E>`.
- **Passing the encoding as a value** (`Client::new(port, FlatBuffers)`,
  `serve(h, p, FlatBuffers)`, with `Bind<E>` a generic trait): inference without
  a turbofish, but a marker argument on every binding and a change to
  `face::Bind` that the runtime's turbofish style (`Payload<E>`, `Ref<T, E>`)
  does not need. Rejected for consistency with the steering's `serve::<E>`.
- **`impl Trait` arguments on `serve`** so that `serve::<E>` names `E` alone:
  requires `Serve` to become an unnameable `impl Future`, which an embedded
  program that stores the future cannot hold. Rejected.
- **Per-payload buffer types in `Sizes<E>`** (one associated type per payload,
  keeping today's exact stack size per call): correct but puts one generated
  name per payload into a public trait. Rejected for the interface maximum
  (DD-S1-10); revisit if a consumer measures the stack cost.
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

| Id       | Decision                                                                                                                                                                                   | Alternatives                                                                          | Cost if wrong                                                                                        |
| -------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------- |
| DD-S1-1  | "`ridl-rt` 0.2" is read as the next breaking workspace minor, 0.8.0; `ridl-rt` keeps sharing the workspace version                                                                         | Give `ridl-rt` its own version line again                                             | A release-procedure note; no code                                                                    |
| DD-S1-2  | `[backend.<name>]` accepts any name; the one key is `encodings`; other keys draw MANI-005; a table for a backend not emitted is silent                                                     | Only known names; pass every key through as an option                                 | A typo in `<name>` is silent until a build emits that backend; adding pass-through later is additive |
| DD-S1-3  | `[backend.<name>]` is root-only (MANI-016), like `[codegen]`                                                                                                                               | Allow per member, last wins                                                           | A member that wants its own codec set cannot have it; lifting the rule is additive                   |
| DD-S1-4  | `ridl-core` validates `encodings` against the toolchain's closed set (MANI-015); the backend refuses what it has no codec for                                                              | Backend-only validation                                                               | One error path duplicated in two layers                                                              |
| DD-S1-5  | One option `encodings` with a `,`-joined value, in the request of that backend only; one request per (package, backend); `wire-encoding` retired                                           | Repeated keys; a new request field; one shared request                                | A plugin parses a joined string; a request clone per backend                                         |
| DD-S1-6  | A single-file build is types only; a flag comes later under ADR-0010                                                                                                                       | Add the flag now                                                                      | Single-file builds cannot carry a codec until then                                                   |
| DD-S1-7  | Types only emits no descriptors, no size table and no face                                                                                                                                 | Emit the (now size-free) descriptors always                                           | A consumer wanting `Interface::MEMBERS` without a codec must name one                                |
| DD-S1-8  | The Rust backend refuses `proto3` and `repr-c` with an error naming #264 / #317; the list is validated whole                                                                               | Emit what it can and warn                                                             | A build naming a future codec fails instead of degrading                                             |
| DD-S1-9  | `Sizes<E>` shape of section 6.1; `Interaction::ROW`; `reservation::<X, E>()` and `table_budget::<I, E>()` as infallible `const fn`; `Unsized` removed; `PayloadInfo` keeps `type_name`     | Keep `Member::reservation` with a scan by ordinal; a `MemberSize` row struct          | A runtime holding `&Member` dynamically cannot size it without the interface type                    |
| DD-S1-10 | Client-side buffers are sized by the interface maximum, not per payload                                                                                                                    | One associated buffer type per payload                                                | Stack bytes per in-flight call up to the largest call payload                                        |
| DD-S1-11 | `E` is the first type parameter of every generic face item; `face::Bind` unchanged; `serve::<FlatBuffers, _, _>`                                                                           | `E` last; a value argument; `Bind<E>` generic                                         | One turbofish per binding and per `serve`                                                            |
| DD-S1-12 | Payload bounds are generated `where` clauses on every impl and function of the face                                                                                                        | A per-interface codec trait that dispatches every payload                             | Long generated signatures; a consumer generic over `E` restates them                                 |
| DD-S1-13 | RSDL-807 `link-encoding-not-emitted`, warn, raised by `ridl build`/`ridlc build` only, one per consumer link, at the link; fires for types-only builds; unspecified crossing draws nothing | A MANI code at the manifest; one diagnostic per encoding; also raised by `ridl check` | A noisy warning set to `allow` (A4's own cost)                                                       |
| DD-S1-14 | Two sub-stages, 1a (emission, size table, lint, `ridl-rt` 0.8) then 1b (generic face); plan 1a now; both aimed at 0.8.0                                                                    | One plan; 1b first                                                                    | If 1b slips, a second generated-API break after 0.8.0                                                |
| DD-S1-15 | `--emit flatbuffers` and `--emit proto` unchanged                                                                                                                                          | Tie a schema emit to `encodings`                                                      | None                                                                                                 |
| DD-S1-16 | `WireEncoding` keeps its name, loses `Default`, gains `parse`; the pipeline takes `&[WireEncoding]`; `generate_face` removed                                                               | Rename to `Codec`; a set type                                                         | A rename later                                                                                       |
| DD-S1-17 | The codegen model and the catalog descriptor are unchanged (`PayloadSizes`, `flatbuffers_max_size`, every `max_sizes` column)                                                              | Drop `flatbuffers_max_size`                                                           | None now; the Kotlin plugin depends on the field                                                     |
| DD-S1-18 | The Kotlin heads-up (section 9.5) is filed by the plan's last task, not by this session                                                                                                    | File it now                                                                           | One day of notice                                                                                    |
| DD-S1-19 | The lint compares against the table of each backend the build emits (built-in or plugin)                                                                                                   | Only `[backend.rust]`                                                                 | None                                                                                                 |
| DD-S1-20 | The request task starts after PR #787 merges or rebases on it; the lint row merges textually with #800                                                                                     | Start now and resolve conflicts                                                       | A rebase                                                                                             |
| DD-S1-21 | `MAX_BUFFER_SIZE` and `EVENT_SOURCE_BUFFER_SIZE` move from inherent constants to `Sizes<E>`                                                                                                | Keep inherent aliases for the one-codec case                                          | A consumer that named them writes `<Cabin as Sizes<FlatBuffers>>::MAX_BUFFER_SIZE`                   |
| DD-S1-22 | `docs/design/ridl-rt.md`'s stale "own version" sentence is corrected in this stage                                                                                                         | Leave it                                                                              | None                                                                                                 |
