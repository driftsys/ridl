# Transcode between payload encodings — design

Status: draft, 2026-10-23. Not yet approved.

## Intent

A generated type that is built with two payload encodings already implements
`ridl_rt::payload::Payload<E>` once for each encoding. A program that receives a
payload in one encoding and forwards it in another (a bridge, for example a
WebSocket server between a FlatBuffers peer and a proto3 peer) must today write
the conversion by hand: verify under the source encoding, decode, and encode
under the target encoding.

This work gives that conversion one name in each generated crate, so that a
later version of the Rust backend can replace its body with a faster generated
conversion (a streaming transcoder, ADR-0018 decision 13) without a change at
any call site.

## Goals

- One reference implementation of "verify under S, decode to `T`, encode under
  D", written once in `ridl-rt`, generic over `T`, S and D.
- Generated functions, one for each ordered pair of encodings, on each type that
  implements `Payload` under both encodings of the pair. Their bodies call the
  reference implementation today.
- Generation is requested in the manifest, not on the command line.
- Tests that will later serve as the equivalence check between a generated fast
  body and the reference implementation.

## Non-goals

- Streaming transcoders. The generated bodies call the reference implementation
  only. A later design replaces the bodies.
- A `Transcode` trait. No caller that is generic over the payload type is known;
  a trait and its generated impls can be added later as an additive change.
- Any change to the `Payload` trait.
- TypeScript, and every backend other than `ridl-backend-rust`.
- Selecting individual pairs or directions. All ordered pairs of the listed
  encodings are generated.

## Anti-goals

- A call site that has to change when the body changes from the reference path
  to a streaming body.
- Code emitted in a crate that did not ask for it (code size matters on the
  targets with no standard library that `just demo` checks).
- A heap allocation inside `ridl-rt`. `T`'s own decode may allocate when `T`
  owns a `String` or a `Vec`; that is unchanged.

## Decided design

### `ridl-rt`

In `crates/ridl-rt/src/payload.rs`, behind no feature (it uses only `payload`
and the encoding markers, which are not feature-gated):

```rust
/// Why a transcode failed.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TranscodeError {
    /// The input bytes did not verify under the source encoding.
    Verify(VerifyError),
    /// The decoded value did not encode under the target encoding.
    Encode(EncodeError),
}
// From<VerifyError> and From<EncodeError> for TranscodeError.

pub fn transcode<'o, T, S, D>(
    bytes: &[u8],
    out: &'o mut [u8],
) -> Result<Ref<'o, T, D>, TranscodeError>
where
    T: Payload<S> + Payload<D>,
    S: Encoding,
    D: Encoding,
{
    let value: T = Ref::<T, S>::verify(bytes)?.decode();
    Ok(Ref::<T, D>::encode(&value, out)?)
}
```

The result follows the conventions of `Ref::encode`: the encoded bytes are a
subslice of `out` (a suffix of `out` for FlatBuffers), and too small an `out` is
`EncodeError::Capacity`. After the input verifies under S, an
`EncodeError::Contract` on the encode under D means the two codecs disagree on a
typl constraint; the function reports it, it does not hide it.

The type parameters are not named `From` and `To`, which would shadow
`core::convert::From`. S is never inferable, so a caller writes at least
`transcode::<_, Proto3, _>`.

`ridl-rt`'s public API is frozen at v0.12.0 under
`cargo semver-checks --release-type minor` (`freeze/surfaces.toml`, surface
`runtime-crates`). A new function and a new `#[non_exhaustive]` enum are
additive changes. The `runtime-crates` dump under `freeze/` is updated with
`cargo xtask freeze update`.

### The manifest

A new boolean key `transcode` in `[backend.rust]`, defined in
`crates/ridl-core/src/manifest.rs` beside `WASM_KEY` and carried to the backend
as a request option in the same way as `wasm`:

```toml
[backend.rust]
encodings = ["flatbuffers", "proto3"]
transcode = true
```

- Absent or `false`: nothing is generated. The default stays off, because
  switching it on later only adds code, and switching it off later would remove
  functions that callers use.
- A value that is not a boolean is MANI-015, as for `wasm`.
- `transcode = true` with fewer than two entries in `encodings` is a new
  manifest error, the next free MANI code (MANI-020 at the time of writing),
  following the MANI-019 check for `codec = true`.

### `ridl-backend-rust`

When the request carries `transcode = true`, for each generated type `T` and for
each ordered pair (S, D) of distinct listed encodings such that `T` implements
both `Payload<S>` and `Payload<D>`, the backend emits one inherent function on
`T`:

```rust
impl Speed {
    pub fn transcode_proto3_to_flatbuffers<'o>(
        bytes: &[u8],
        out: &'o mut [u8],
    ) -> Result<
        ::ridl_rt::payload::Ref<'o, Self, ::ridl_rt::encoding::FlatBuffers>,
        ::ridl_rt::payload::TranscodeError,
    > {
        ::ridl_rt::payload::transcode::<Self, ::ridl_rt::encoding::Proto3, _>(bytes, out)
    }
}
```

- The function name is `transcode_<s>_to_<d>`, where `<s>` and `<d>` are the
  manifest encoding names with `-` replaced by `_` (`flatbuffers`, `proto3`,
  `repr_c`).
- A type that implements `Payload` under only one encoding of a pair gets no
  function for that pair. For example, proto3 roots only a struct, a union, a
  named scalar, an enum or an enum set, while FlatBuffers roots every named
  type.
- No function is emitted when S and D are the same encoding.
- The function carries a doc comment that states the conversion it performs and
  links to `ridl_rt::payload::transcode`.

The frozen generated crates under `freeze/generated/` do not set
`transcode = true` and do not change.

## Tests

- `ridl-rt` unit tests of `transcode`, with a test type that implements two
  encodings: S to D gives the same bytes as encoding the value directly under D,
  and the result decodes back to the value; D to S likewise; S equal to D; a
  verify failure; a contract failure; a capacity failure. `Ref` has no `Debug`,
  so tests compare `.map(|r| r.bytes())` and `.err()`. A test compares error
  variants, not `Capacity.needed`, which is documented as a lower bound.
- `ridl-core` manifest tests: the key parses as a boolean, a non-boolean is
  MANI-015, `true` with zero or one encoding is the new MANI code, `true` with
  two encodings has no diagnostic.
- `ridl-backend-rust` tests: with `transcode = true` and two encodings, the
  expected functions are emitted for a type with both impls and not for a type
  with one; with the key absent, none are emitted.
- An end-to-end test that builds a crate with `flatbuffers` and `proto3` and
  `transcode = true`, and checks over a set of values that each generated
  function returns the same bytes as encoding directly under the target
  encoding, in both directions. This also checks that the generated encoders are
  deterministic, which nothing checks today. This test is the equivalence check
  a later streaming body must pass.

## Records to update

- ADR-0018 decision 13: a dated note that the domain-mediated path ships as
  `ridl_rt::payload::transcode`, with generated `transcode_<s>_to_<d>` functions
  whose bodies a later streaming transcoder replaces.
- `docs/design/ridl-rt.md`: the function and the error type.
- `docs/book/generated-code.md`: the generated functions and the manifest key.
- The manifest chapter of the book that lists the `[backend.rust]` keys, and the
  MANI catalogue entry for the new code.

## Alternatives considered

- **`From`/`Into` between the encoded forms.** Not expressible: inside `ridl-rt`
  a generic impl conflicts with `impl<T> From<T> for T` (E0119); in a generated
  crate it breaks the orphan rule (E0117); and `From::from` has no output
  buffer, so it would need an allocation.
- **A `Transcode<S, D>` trait with a blanket impl in `ridl-rt`.** A blanket impl
  prevents a per-type override, so a faster body could not replace it.
- **A `Transcode<S, D>` trait with generated impls.** Allows the override and
  supports callers that are generic over the payload type, but adds a trait to
  the frozen `runtime-crates` surface for callers that do not exist yet.
  Rejected for now; it is an additive change later.
- **The free function only, with no generated functions.** Every call site would
  change when faster generated conversions arrive.
- **Command-line flags (`--transcode-from`, `--transcode-to`).** The encodings
  are already listed in the manifest, and ADR-0010 keeps build configuration
  there.
- **Generate automatically whenever two encodings are listed.** Adds code to
  every crate that lists two encodings, most of which do not transcode.
- **`transcode` takes a verified `Ref<T, S>` and returns `EncodeError`.** No new
  error type, but a later streaming transcoder takes bytes and reports both
  error classes, so the equivalence test would compare two different signatures.

## Settled after the builder check

These were settled by the author from the code, not by the owner:

- **Feature gates.** The generated crate gates its `Payload<E>` impls by no
  Cargo feature; the encodings are fixed by the manifest at generation time. The
  transcode functions carry no `cfg` either, and no feature-matrix cell is
  added.
- **`repr-c`.** `ridl-backend-rust` has no codec for `repr-c` and emits no
  `Payload<ReprC>` impl, so in practice only the `flatbuffers`/`proto3` pair
  produces functions. The rule "both impls exist" covers `repr-c` when a codec
  for it lands; the `repr_c` name rule is kept for that case.
- **Contract failure after a successful verify.** No case is known where the
  proto3 and FlatBuffers codecs disagree on a typl constraint. The `ridl-rt`
  test covers it with a hand-written test type. The end-to-end test uses valid
  values only and expects equal bytes; a difference there is a defect to report,
  not an expected result.
- **Absent `encodings`.** An absent key and `encodings = []` both mean no
  encodings, so `transcode = true` with either gets the new MANI code. The key
  is independent of `wasm` and `codec`.
- **Freeze.** The `runtime-crates` baseline stays `v0.12.0`;
  `just
  freeze-check` must pass in the same PR as the code. The `manifest`
  surface dump gains the new key and is regenerated in the same PR.

## Left to the builder

- Where in `ridl-backend-rust` the emission lives (a new module, or beside the
  codec emitters).
- How the backend knows that a type implements `Payload` under an encoding (the
  same predicate the codec emitters already use).
- The exact wording of the new MANI message.
