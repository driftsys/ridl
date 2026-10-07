# ridl-rt

The library that code generated from [ridl](https://github.com/driftsys/ridl)
links, and a runtime implements.

`ridl-rt` defines, once, the vocabulary that generated code and a runtime agree
on: identity and the interaction descriptors (`contract`), the payload encodings
(`encoding`), the contract and transport errors and the two errors a generated
client call and a generated `serve` return (`error`), encoding and decoding a
payload (`payload`), the ports a runtime implements and generated code calls
(`port`), and time, the envelope, and the values a read returns (`sample`). It
contains no runtime: a runtime is a separate crate that implements the `port`
module's traits. It also carries the caller-side call table and the waker
registry every runtime would otherwise write alone (`correlate`), and the traits
a generated face implements for its fixed methods — `Bind::new`,
`Events::next_event`, `Publish::commit`, and under `std` `Timeout` — which a
consumer brings into scope with the generated `prelude` of each interface whose
face it uses (`face`). One more module holds the optional trace context that a
call or an event carries across a port (`trace`); with the `std` feature it also
holds the application's hook for that context (`Propagation`, `AlreadySet`,
`set_propagation` and `propagation`).

With its default features the crate is `no_std` and allocates nothing; it
contains no `unsafe` code and has no dependency in any feature combination. It
declares one cargo feature per payload encoding — `flatbuffers`, which enables
the FlatBuffers reading and writing helpers, and `proto3` and `repr-c`, which
enable nothing in this version — and a `std` feature, off by default, that links
the standard library and enables `task::block_on`, `task::noop_waker` and
`task::flag_waker`, and the propagation hook of the `trace` module.

## Versioning

`ridl-rt` is a 0.x crate. A breaking change bumps the minor version (0.1 → 0.2),
not the major version. A public struct whose fields are all public and that
carries no `#[non_exhaustive]` cannot gain a field without a breaking change,
because code outside the crate can build it as a struct literal: `CatalogRef`,
`Member`, `Timing`, `PayloadInfo`, `EncodedSizes`, `Encoded`, `Violation`,
`RawSample`, `RawOccurrence`, `Claim`, `Watermark`, `Changed`, `Envelope`,
`Sample`, `Occurrence`, `TraceContext`. The public tuple structs — `Ordinal`,
`InterfaceNo`, `CatalogHash`, `Correlation`, `ClaimId`, `Timestamp`, `Duration`
— follow the same rule. So do the unit structs `FlatBuffers`, `Proto3` and
`ReprC` (`src/encoding.rs`), `TrackerFull` (`src/sample.rs`) and `AlreadySet`
(`src/trace.rs`): each is a unit struct with no field that code outside the
crate uses as a value or a pattern, so a field added to any of them breaks that
code.

The open API questions are tracked at
<https://github.com/driftsys/ridl/issues/350>.

`ridl-rt` supports Rust 1.83 or newer (`rust-version = "1.83"`). Raising that
minimum is a breaking change, shipped in a 0.x minor release like any other.

The crate builds as edition 2021 and edition 2024: edition 2021 is tested with
Rust 1.83 and with the pinned toolchain, and edition 2024 is tested with the
pinned toolchain (edition 2024 did not exist before Rust 1.85, so the minimum
cannot build it).

## What 0.1 leaves out

- streams
- `Inline`
- the three payload codecs (proto3, FlatBuffers, `repr(C)`)
- `Encoding::FORMAT`
- `Family`
- `Access`
- `Constrained`
- the runtimes (`ridl-loopback`, `ridl-transport-ws`)
- the engine

## License

MIT. See [LICENSE](https://github.com/driftsys/ridl/blob/main/LICENSE).
