# Encoding bench

Measures the three payload encodings (FlatBuffers, proto3, podbuf) on the types
of the veh-cluster corpus: encoded size and heap allocations of encode, verify
and decode. It is its own cargo workspace, outside the repository's.

    just bench-host

The recipe runs `cargo xtask bench gen`, which writes the generated crate to
`bench/encodings/generated/veh-cluster` (not in git), then runs the tests of
`harness/`. The recipe is not a member of `build`.
