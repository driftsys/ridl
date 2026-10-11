# Stability

From release 0.10.0, you can move from one 0.10.x release to the next without
changing a source file, a manifest, a script that calls the CLI, a codegen
plugin, a program that uses the generated Rust crate, or a deployed peer. This
chapter lists what that promise covers and what it leaves out. The policy is in
effect now; release 0.10.0 is not published yet.

## What will not change in 0.10.x

- **The language.** The syntax and the keywords of typl, ridl and rsdl, and the
  set of sources that compile without an error.
- **The manifest.** The keys of `ridl.toml`, their value types, and the formats
  of `interfaces.lock` and `.ridl/baseline`.
- **Diagnostics and lints.** The diagnostic codes, the lint names and the
  default level of each lint.
- **The command line.** The subcommands, the flags, the exit codes, and the
  shape of the `--format json` and `--format sarif` output.
- **The generated Rust crate.** The domain types and the interaction face
  (`Client`, `Provider`, `Publisher`, `Event`, `serve` and `Bind`).
- **The runtime crates.** The public API of `ridl-rt`, `ridl-rt-conformance`,
  `ridl-loopback` and `ridl-descriptor`, and the names of their cargo features.
- **The codegen plugin protocol.** The messages `ridl.ir.v2` and
  `ridl.codegen.v1`, as a plugin reads and writes them. An enum of the protocol
  gains a value only in a minor release, such as 0.11, because a plugin rejects
  an enum value that it does not know.
- **The MCP tools.** The tool names and the input and output schemas.
- **The wire format.** The frame envelope, the `attach` handshake, the inputs of
  the catalog hash, the emitted `.proto` and `.fbs` mapping and the encoding
  names.
- **The minimum Rust version.** Rust 1.83 and edition 2021 for `ridl-rt` and
  `ridl-rt-conformance`, and the generated crate compiles as edition 2021 with
  that Rust version. A 0.10.x release does not raise either.

## What may change in 0.10.x

- **Additions.** A new subcommand, flag, manifest key, field, tool, cargo
  feature or transport default. A program that does not use an addition is not
  affected by it. A new flag, command-line argument or MCP tool input is always
  optional: a 0.10.x release does not add a required one, and does not make an
  optional one required.
- **New lints.** A new lint starts at the `allow` or `info` level, so it does not
  fail a build that passed before.
- **Message text.** The text of a diagnostic message, of a doc comment and of
  generated docs.
- **Size and budget values.** For example the size of a header or the sum that a
  budget limits.
- **Unpublished crates.** `xtask`, `ridlc-gen-model` and `ridlc-gen-rust`.

A security fix is the one reason to break a frozen surface in a 0.10.x release.
Its reason is recorded in the stability policy.

## When a break can happen

A break can only land in 0.11 or a later release, and that release states it.
The releases of 0.9.x and earlier are not covered by this promise.

## How the promise is checked

The repository checks each frozen surface that has a recorded baseline against
that baseline, and a change that removes or alters something a user depends on
fails the build. Some parts are promised by policy and not yet checked by a
tool: the frame envelope, the `attach` handshake, the formats of
`interfaces.lock` and `.ridl/baseline`, the exit codes of every subcommand
other than `ridl check`, the output of `ridl diff`, and the categories that
`ridl diff` reports. A contributor who must change a frozen
surface follows the steps in `CONTRIBUTING.md`, "Changing a frozen surface". The
full policy, with the gate that checks each surface and the rules for approving
a break, is [ADR-0029](../decisions/ADR-0029-stability-policy.md).
