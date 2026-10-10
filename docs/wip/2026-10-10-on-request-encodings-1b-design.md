# On-request encodings, sub-stage 1b — the face reads the encoding from the port (residual design)

Satisfies: driftsys/ridl#801. This file holds the part of the on-request
encodings design that is not built yet. Sub-stage 1a (on-request emission, the
size table, RSDL-807 and the whole `ridl-rt` break) landed and was gardened into
the amendments dated 2026-10-10 of ADR-0002, ADR-0018, ADR-0020, ADR-0021 and
ADR-0023 and into the design records they name. The full design and the 1a plan
are archived verbatim as
`docs/archive/2026-10-10-on-request-encodings-design.md` and
`docs/archive/2026-10-10-on-request-encodings-plan.md`. The sections below are
copied verbatim from that design, with the design's own section numbers, so that
the 1b plan can be written from this file. Read the archived design for the
context they assume (sections 2, 6.1 and 6.3).

## What changed since the design was written

- **The release.** `ridl-rt` 0.8.0 was released before sub-stage 1a landed, so
  the 1a break ships in the next minor release, 0.9.0 (ADR-0021 decision 10,
  amendment of 2026-10-10). Where the text below says 0.8.0, read 0.9.0.
- **What 1a left for 1b.** `ridl_rt::port::Encoded` exists, with the forwarding
  impl for `&mut P`, and no port in the workspace implements it. The generated
  face is monomorphic over FlatBuffers and reads every buffer from
  `Payload<FlatBuffers>::BUFFER` or `Sizes<FlatBuffers>` (design section 7.4, as
  built). So 1b replaces the concrete encoding path by `P::Encoding`, adds the
  `Encoded` bounds, types `ridl-loopback`, and changes no `ridl-rt` item.
- **The records that state 1b as later work.** When 1b lands, these sentences
  are rewritten to describe it as built: ADR-0018 decision 15's amendment;
  ADR-0021's `port::Encoded` amendment and decision 19's amendment; ADR-0023
  decision 6's amendment; `docs/design/interaction-face.md` (the passage that
  calls the generic face later work, and "The face gains no type parameter");
  `docs/design/ridl-rt.md` (the `Encoded` passage, and the transport-default
  table of section 7.1 below); the `ridl-loopback` rustdoc and
  `docs/book/writing-a-port.md` (row below).

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

## 6.3 Breaking changes (the rows that land in 1b)

From the design's table of breaking changes (section 6.3):

| Item                 | Change                                                                                                                                                                                  | Replacement |
| -------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------- |
| Generated face types | take the encoding from the port (`P: Encoded`, `P::Encoding`); no new type parameter (section 7; sub-stage 1b)                                                                          |             |
| `ridl-loopback`      | `Loopback<E: Encoding = FlatBuffers>` and `HandlerHandle<E>`, both `Encoded`; `Loopback::new(catalog)` is FlatBuffers, `Loopback::<E>::with_encoding(catalog)` any other (sub-stage 1b) |             |

## 9.3 Tests and fixtures (the 1b part)

The design's section 9.3 says: the hand-written fake ports of
`crates/ridl-backend-rust/tests/interaction_face.rs`
(`ReportsInitOverRealBytes`, `MinimalSignalOnlyPort`, `DistinctiveInitPort`)
gain `impl Encoded { type Encoding = FlatBuffers; }` in 1b, and
`crates/ridl-backend-rust/tests/generated/interaction_face.rs` is regenerated
for the generic face.

## 10. Records amended (the 1b row)

| Record                                                    | Change                                                                               |
| --------------------------------------------------------- | ------------------------------------------------------------------------------------ |
| `ridl-loopback` rustdoc and `docs/book/writing-a-port.md` | `Loopback<E = FlatBuffers>`, `with_encoding`, the `Encoded` impl a port writes (1b). |

The design's section 13 also leaves to 1b the ADR-0023 decision 6 amendment and
the rewrite of "The face gains no type parameter" in
`docs/design/interaction-face.md`.

## 11. Testing (the 1b test)

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

## 13. Scope (the 1b sub-stage)

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

## 14. Alternatives considered (the ones that concern the encoding of a face)

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

## 15. Decisions (the 1b rows)

| Id       | Decision                                                                                                                                                                                                                                                                                                                                                                                                                                                                             | Alternatives                                                                                            | Cost if wrong                                                                  |
| -------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------ |
| DD-S1-11 | Revised by Sebastien 2026-10-10. The encoding comes from the port: `ridl_rt::port::Encoded { type Encoding }`, implemented by every port; the face takes `P::Encoding` and gains no type parameter; application code names no encoding; each transport has a default from the coupling of its ends (loopback FlatBuffers, WebSocket proto3, shared memory `repr(C)`), overridable at construction; #265 lands with `connect_with::<FlatBuffers>` and gains `connect` when #264 lands | `E` named at each binding (the first draft); a value argument; a manifest-wide default; a cargo feature | A port type implementing `Encoded` per transport; `ridl-loopback` typed by `E` |
| DD-S1-12 | Confirmed by Sebastien 2026-10-10. Payload bounds are generated `where` clauses on every impl and function of the face                                                                                                                                                                                                                                                                                                                                                               | A per-interface codec trait that dispatches every payload                                               | Long generated signatures; a consumer generic over `E` restates them           |
