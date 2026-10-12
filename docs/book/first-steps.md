# First steps

This chapter takes you from nothing to a checked package and a generated Rust
crate, in five steps. It states each step once and links to the chapter that
owns the detail. Read [Getting started](getting-started.md) next for the
language itself.

The commands below do not run a contract. `ridl` checks your declarations and
generates code from them. The only runtime in this repository is
`ridl-loopback`, which runs inside one program; no command here starts it.

## 1. Install the toolchain

On Linux and macOS:

```sh
curl -fsSL https://raw.githubusercontent.com/driftsys/ridl/main/install.sh | bash
ridl --version
```

On Windows, run `install.ps1` from PowerShell instead. The environment variables
that choose the install directory and the release, and the option of building
from a clone, are in
[Installing the toolchain](getting-started.md#installing-the-toolchain). The
editor and the agent tools are in
[Editor and agent setup](getting-started.md#editor-and-agent-setup).

## 2. Create a package

```sh
ridl new demo
```

`ridl new` creates the directory `demo` and writes two files into it. Use
`ridl init` to write the same files into a directory that already exists, and
`--workspace` to write a workspace root with one member instead of a standalone
package. The flags and the cases where the command refuses are in the
[CLI reference](cli-reference.md#ridl-init).

`demo/ridl.toml` is the manifest. It names the package and its version, and it
ends with a commented-out `[backend.rust]` table that step 5 uses.

`demo/demo.ridl` is the source file:

```ridl
package demo

/// Road speed.
type Speed: km/h [0.0..250.0 step 0.5]

/// Reports speed.
interface Hello {
  /// Current speed, every 10 ms.
  signal speed: Speed @10ms
}
```

The first line names the package. A package name must equal the directory path
below the workspace root, and every file in the directory declares the same
name. `Speed` is a scalar type with a unit and a range, and `Hello` is an
interface with one signal whose contract requires the provider to publish it
every 10 ms. The [typl reference](reference/typl.md) and the [ridl reference](reference/ridl.md)
define every form.

## 3. Check it

```sh
ridl check demo
```

`ridl check` reads the package and prints the diagnostics it draws. It exits 0
when none is an error and 1 when one is. To see an error, change `Speed` to
`Sped` in the signal and run the command again: it prints `TYPL-011` (unknown
type name) with the file, the line and the column. The [`ridl check`
reference](cli-reference.md#ridl-check) lists the flags and the exit codes, and
the [lints chapter](lints.md) describes the warnings that you can raise or
lower.

## 4. Format it

```sh
ridl fmt --check demo
```

`ridl fmt` rewrites the sources into the canonical layout, and `--check` writes
nothing and exits 1 when a file differs. The scaffold already conforms.

## 5. Build it

```sh
ridl build demo
```

`ridl build` writes the Rust crate under `out` in the current directory:
`Cargo.toml`, `lib.rs` and one file for the package. With no `[backend.rust]`
table in the manifest, the crate holds the domain types only. To also generate
the FlatBuffers codec, the descriptors and the interaction face, uncomment the
table in `demo/ridl.toml`:

```toml
[backend.rust]
encodings = ["flatbuffers"]
```

and build again. Other targets, such as TypeScript and the IR, are chosen with
`--emit`; see [`ridl build`](cli-reference.md#ridl-build).

## Where to go next

- To learn the language, work through [Getting started](getting-started.md).
- To use the crate that step 5 wrote, read
  [Using the generated Rust code](generated-code.md).
- To describe components, wiring and deployments, read
  [Describing a system](rsdl.md).
- To change an interface after you published it, read
  [Evolving an interface](evolving-an-interface.md).
- To see what stays fixed across the 0.10.x releases, read
  [Stability](stability.md).
