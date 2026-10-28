# From a package to a running program

[First steps](first-steps.md) ends at a generated crate. This chapter goes on:
it links that crate from a Rust program and runs the program. It states each
step once and links to the chapter that owns the detail.

**What runs here.** The program runs in one process. The only runtime in this
repository is `ridl-loopback`, which connects the faces of one program to each
other; no network runtime exists, so nothing below delivers a message between
processes, measures a timing bound, or enforces a contract on a remote provider.
[Using the generated Rust code](generated-code.md) says what a call does over
`ridl-loopback`.

## The schema

The scaffold of [First steps](first-steps.md) has one signal. A program that
shows every kind of interaction needs a schema with more, so this chapter uses
the worked example `examples/cabin` of the repository. Its schema declares a
signal, an event, a command and a query:

```ridl,ignore
{{#include ../../examples/cabin/cabin.ridl:schema}}
```

The manifest of that example names the encoding that the face needs, with the
`[backend.rust]` table that step 5 of First steps shows:

```toml
[package]
name = "veh.cabin"
version = "1.0.0"

[backend.rust]
encodings = ["flatbuffers", "proto3", "podbuf"]
```

The face needs one of the encodings; the example lists all three so that the
program can also encode a value with podbuf. Do the same in your own package:
uncomment the table in `ridl.toml`, and the build writes the codec, the
descriptors and the face.

## Generate the crate

```sh
ridl build examples/cabin --emit rust --out-dir generated
```

The crate is named after the package, with each `.` replaced by `_`, so this one
is `veh_cabin`. [Using the generated Rust code](generated-code.md#what-the-crate-contains)
lists what it holds.

## Link the crate

A Rust package names the generated crate by path, `ridl-loopback` for the
ports, and `ridl-rt` for the items the program uses from it. This `Cargo.toml`
fragment is an illustration and is not compiled; the program that `just demo`
builds has the same three dependencies in `examples/cabin/consumer/Cargo.toml`:

```toml
[dependencies]
veh_cabin = { path = "generated", features = ["std"] }
ridl-loopback = "0.14.0"
ridl-rt = { version = "0.14.0", features = ["std"] }
```

The program imports `CatalogRef`, `Interface` and `Provenance` from `ridl-rt`
and calls `ridl_rt::task::noop_waker`, which exists only under the `std` feature
of `ridl-rt`. The `std` feature of the generated crate is on by default and is
named here so the dependency is explicit. The features of each crate are in
[Cargo features](generated-code.md#cargo-features).

## Write the program

The face is a set of types that the program imports. `prelude` brings the traits
of the face into scope; without it, `Client::new` does not compile:

```rust,noplayground
{{#include ../../examples/cabin/consumer/src/main.rs:imports}}
```

A runtime gives out ports, and each face binds one. `Loopback::new` takes the
catalog that the face was generated from, which `CATALOG` holds in the program:

```rust,noplayground
{{#include ../../examples/cabin/consumer/src/main.rs:bind}}
```

The first interaction is the signal. The publisher side publishes a value and
commits it, and the client side reads it. `new` checks the typl range of
`Temperature` and returns an error for a value outside it:

```rust,noplayground
{{#include ../../examples/cabin/consumer/src/main.rs:signal}}
```

A command and a query need a provider. The program implements the trait that the
face generates, and the face calls it only with a request that passed its
checks:

```rust,noplayground
{{#include ../../examples/cabin/consumer/src/main.rs:provider}}
```

The rest of the program, which sends the command and the query through the async
client and through the blocking client, is explained in
[Using the generated Rust code](generated-code.md#async-calls).

## Run it

From a clone of the repository, `just demo` generates the crate, runs clippy,
the `no_std` checks and the documentation build on it, and then runs the
program. To run only the program, generate the crate as above and then:

```sh
cargo run --manifest-path examples/cabin/Cargo.toml -p consumer
```

The program prints one line per round trip. Each line carries the value that
the round trip moved, and the program exits with a non-zero status when a value
is not the expected one. The last line comes from a value that goes through the
generated podbuf codec alone, without the runtime: the program encodes it,
verifies the bytes and decodes it again:

```text
signal ok 21
event ok 5
command ok 42
query ok 7
blocking command ok 43
blocking query ok 9
podbuf round trip: 21.5
```

## Where to go next

- To call the interface in every form, and to implement a provider, read
  [Using the generated Rust code](generated-code.md).
- To connect a face to a transport of your own, read
  [Writing a port](writing-a-port.md).
- To change an interface after you published it, read
  [Evolving an interface](evolving-an-interface.md).
