# Fuzz targets

Coverage-guided fuzz targets for the decoders that read bytes from a peer
(driftsys/ridl#1144). Each target gives arbitrary bytes to one decoder. The
decoder may refuse the bytes with a returned error. A panic, an abort, an
out-of-memory failure or a hang is a finding.

This directory is its own Cargo workspace and needs a nightly toolchain, because
cargo-fuzz instruments the build with flags that stable Rust rejects. The
workspace of the repository keeps its stable pin in `rust-toolchain.toml`; no
member of `just build` and no gating CI job builds or runs anything here.

## Targets

| Target               | Decoder under test                                                                |
| -------------------- | --------------------------------------------------------------------------------- |
| `proto3_verify`      | `Ref::verify` and `decode` of the generated proto3 codec                          |
| `flatbuffers_verify` | `Ref::verify` and `decode` of the generated FlatBuffers codec                     |
| `podbuf_verify`      | `Ref::verify` and `decode` of the generated podbuf codec (input copied 8-aligned) |
| `descriptor_verify`  | `ridl_descriptor::verify` and the conversion to the owned `Catalog`               |
| `frame_decode`       | `ridl_transport_ws::frame::decode`                                                |

The three codec targets decode into code generated at build time from the
fixtures that `crates/ridl-backend-rust/tests/fixtures/` holds
(`proto3_roundtrip.ridl`, `flatbuffers_roundtrip.ridl` and
`podbuf_roundtrip.ridl`), so they read the same types as the codec tests of that
crate. See `build.rs`.

Beyond the refusal, each target checks what the totality tests of
driftsys/ridl#1287 check. A buffer that verifies must decode, and the decoded
value must encode again to bytes that verify. For `frame_decode` the encoded
bytes must equal the input.

## Setup

The nightly is pinned for this directory only in `rust-toolchain.toml`.

    rustup toolchain install nightly-2026-10-08 --profile minimal
    cargo install cargo-fuzz --locked

## Run a target

From the repository root, run one target for a number of seconds (60 by
default):

    just fuzz frame_decode
    just fuzz podbuf_verify 300

Or from this directory:

    cargo fuzz run proto3_verify corpus/proto3_verify seeds/proto3_verify -- -max_total_time=120

Set `CARGO_TARGET_DIR` to keep the build output out of `fuzz/target`.

`seeds/<target>/` holds a few small, valid encodings, checked in. The corpus
that a run grows goes to `corpus/<target>/`, and a crashing input goes to
`artifacts/<target>/`. Neither directory is in git.

## From a crash to a regression test

A crash file stays out of git. When a target finds one:

1. Minimize it: `cargo fuzz tmin <target> artifacts/<target>/crash-<hash>`.
2. Print the bytes: `xxd -i artifacts/<target>/<minimized file>`.
3. File an issue with the label `debt`: the target, the bytes, and the stack
   that the fuzzer printed.
4. Add a normal `#[test]` to the stable suite of the crate that owns the
   decoder, with the bytes written inline in the test. It calls the decoder the
   way the target does and expects a returned error, or a successful round trip,
   and no panic. Until the decoder is fixed, mark it
   `#[ignore = "driftsys/ridl#<issue>"]`.
5. Fix the decoder in a separate change and remove the `#[ignore]` in it.

The regression test runs on stable with `cargo test`, so the fuzz run is not
needed to keep the fix covered.

## Not covered

- No target feeds structured input. Every target takes a flat byte string, so a
  buffer that needs a valid checksum or length to reach deep code is reached
  only through the corpus and the instrumentation.
- The codec targets cover the fixtures' root types, not every type that a schema
  can declare.
- Nothing runs these targets on a schedule yet.
