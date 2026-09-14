# ridl-rt

The library that code generated from [ridl](https://github.com/driftsys/ridl)
links, and a runtime implements.

`ridl-rt` defines, once, the vocabulary that generated code and a runtime agree
on: identity and the interaction descriptors (`contract`), the payload encodings
(`encoding`), the contract and transport errors (`error`), encoding and decoding
a payload (`payload`), the ports a runtime implements and generated code calls
(`port`), and time, the envelope, and the values a read returns (`sample`). It
contains no runtime: a runtime is a separate crate that implements the `port`
module's traits.

The crate is `no_std`, allocates nothing, contains no `unsafe` code, and has no
dependency. It declares one cargo feature per payload encoding — `flatbuffers`,
`proto3`, `repr-c` — and in this version each feature enables nothing.

## Versioning

`ridl-rt` is a 0.x crate. A breaking change bumps the minor version (0.1 → 0.2),
not the major version. Adding a field to a struct that code outside the crate
can build as a struct literal — `Encoded`, `Envelope`, `RawSample`, `Claim`,
`Timing`, `EncodedSizes`, `Member`, `PayloadInfo` — is a breaking change.

The open API questions are tracked at
<https://github.com/driftsys/ridl/issues/350>.

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
