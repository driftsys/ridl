# Encoding bench

Measures the three payload encodings (FlatBuffers, proto3, podbuf) on every
signal payload of the veh-cluster corpus and of `examples/cabin`, and on the
stress payloads of `stress/` (a long string, a list of structs, nested
optionals, a map): encoded size and heap allocations of encode, verify and
decode. A payload that an encoding refuses gets a `Refused` row with the reason.
It is its own cargo workspace, outside the repository's.

    just bench-host

The recipe runs `cargo xtask bench host`, which writes the three generated
crates and `signals.txt` to `bench/encodings/generated/` (not in git), runs the
tests of `harness/`, and prints the table.
