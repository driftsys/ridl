# ADR-0029 — Stability policy

## Status

Accepted — 2026-10-11. Scope: which surfaces of RIDL are frozen from release
0.10.0, which are not, how a frozen surface is checked, how a break of a frozen
surface is approved, and which gate covers which surface. It binds every change
to a frozen surface and every contributor, and it is the one record that owns
stability: the earlier records that state a stability rule link here and do not
restate it.

Written from the release 0.10 design (2026-10-11), whose decisions R-2, R-8 and
R-9 Sebastien took in the brainstorming session of the same day. The design's
section 4.2 describes the check as first imagined; decision 3 below describes it
as built, and where the two differ this record is the authority.

It adds a dated note to each record that held a stability statement before: the
IR specification §6 and §7, [ADR-0005](ADR-0005-agent-enablement.md) §7,
[ADR-0008](ADR-0008-e2-execution.md) decision 9,
[ADR-0010](ADR-0010-cli-conventions.md),
[ADR-0021](ADR-0021-ridl-rt-0.1-api-and-release.md) decision 10 and
[ADR-0025](ADR-0025-workspace-aware-mcp-tools.md). Each statement stays as
written; this record adds the check and the approval rule.

## Context

Until 0.9.x, a breaking change could land in any release. 0.10.0 is the first
release that people adopt for real work, so a user who builds on 0.10.0 must be
able to move to any 0.10.x release without changing a source file, a manifest, a
script that calls the CLI, a codegen plugin, a program that uses the generated
Rust crate, or a deployed peer.

A written promise is not enough. The workspace holds many insta snapshots, and a
pull request can rewrite any of them with `cargo insta accept` in the same
change that breaks the surface. A freeze check must therefore compare against a
version that the pull request cannot edit: a git ref.

The check as built differs from the design in five ways. This record refines the
design in each:

- The generated-crate inputs are frozen copies of `examples/cabin` and of the
  `veh-cluster` corpus under `xtask/tests/freeze-fixtures/generated/`, so an
  edit to `examples/` or to the corpus is not a break of that surface.
- The fixture manifests name only `flatbuffers`, the one codec the Rust backend
  emits today.
- The dumps are committed under `freeze/`, and the baseline copy of a dump is
  read with `git show <baseline>:freeze/<surface>.txt`. A new row therefore
  starts in a follow-up pull request that names the merge commit of the pull
  request which added the dump as its baseline.
- The accepted-sources dump covers the packages under `examples/` and
  `evals/corpus/`; it does not include the fences of `docs/book/`, which a test
  already compiles.
- The wire dump holds the encoding names, the `.proto` and `.fbs` lines for the
  corpus and the catalog hash of the cabin example. It holds no frame vectors
  and no `attach` handshake vectors, so the frame envelope and the handshake are
  frozen by policy with no automated check until the wire dump gains them.
- The minimum Rust version is checked for `ridl-rt` and `ridl-rt-conformance`
  only. `ridl-loopback` and `ridl-descriptor` are edition 2024 today, so the
  minimum does not cover them yet. The generated crate compiles as edition 2021
  with Rust 1.83, which `just compat-check` checks and `just freeze-check` does
  not.

## Decision

1. **Ten surfaces are frozen from 0.10.0.** A change that removes or alters what
   a user depends on in one of them is a break.

   | Surface                 | Check (row name in `surfaces.toml`)                                                              | Switched on by                       | Baseline |
   | ----------------------- | ------------------------------------------------------------------------------------------------ | ------------------------------------ | -------- |
   | Language                | `keywords`, `accepted-sources` dumps                                                             | the language lane                    |          |
   | Manifest                | `manifest` dump                                                                                  | the language lane                    |          |
   | Diagnostics and lints   | `diagnostics` dump                                                                               | the language lane                    |          |
   | CLI                     | `cli` dump                                                                                       | the developer-experience lane        |          |
   | Generated Rust crate    | `generated-crate`, `cargo-semver-checks`                                                         | the codegen lane                     |          |
   | Runtime crates          | `runtime-crates`, `cargo-semver-checks`                                                          | the wire and runtime lane            |          |
   | Codegen plugin protocol | `codegen-protocol` dump                                                                          | the codegen lane                     |          |
   | MCP tools               | `mcp` dump                                                                                       | the release and policy lane, at once |          |
   | Wire format             | `wire` dump: encoding names, `.proto` and `.fbs` lines, catalog hash; frames are policy only now | the wire and runtime lane            |          |
   | Minimum Rust version    | `msrv` dump                                                                                      | the wire and runtime lane            |          |

## Alternatives considered

- **Compare the insta snapshots.** Rejected: a pull request can rewrite a
  snapshot in the same change that breaks the surface, so the comparison has no
  fixed reference.
- **One freeze on one date.** Rejected: the lanes run in parallel and finish at
  different times. A gate per surface switches on when the last pending break
  for that surface has landed.
- **A line dump for the Rust crates.** Rejected: it cannot tell a compatible
  addition from a break, for example a new required method on a public trait.
- **Read the generated crate's sources from `examples/` and the corpus.**
  Rejected: an edit to an example would then count as a break of the generated
  crate. Frozen fixture copies make the break an explicit change.
- **Freeze size and budget values.** Rejected: they are tuned as the encodings
  mature, and no user program depends on an exact value.

## Consequences

- A pull request that breaks a frozen surface fails `just freeze-check` unless
  it also moves the baseline of that surface, so the approval is visible in the
  diff.
- A break needs two pull requests: the first removes the row and makes the
  break, and the second adds the row again at the first one's merge commit.
- Adding a row for a new surface needs two pull requests: the first adds the
  dump, and the second adds the row with the first one's merge commit as its
  baseline.
- The check reads the cargo cache offline, so a build of the workspace must run
  before it on a clean machine.
- ADR-0021 decision 10 and the stability notes of the records listed in the
  Status keep their text. This record owns the policy and the check.

## References

- `freeze/README.md` and `freeze/surfaces.toml`
- `xtask/src/freeze.rs` and `xtask/src/freeze/`
- [ADR-0009](ADR-0009-toolchain-and-gate-parity.md) — the gate recipes
- [Stability, in the book](../book/stability.md)
- `CONTRIBUTING.md`, "Changing a frozen surface"
