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

**Amended 2026-10-12 — the wire dump holds the frame vectors.** The `wire` dump
gains one `frame-vector <id> <hex>` line per row of the vector table of the
frame specification §11.1, so the frame envelope and the `attach` handshake are
checked by the dump and leave the policy-only list. The Context bullet on the
wire dump, decision 3 and decision 6's `Wire format` row are changed in place.
The layouts of the `unsubscribe`, `read` and `occurrence` frames, which no
vector shows, the payload-presence rule for a `publish` under `Init` or
`Invalid(Declared)` and for a `response` under `reply`, which no vector shows,
and the codes no vector carries, among them the `kind` values 2 and 5 and the
`attach` encodings 2 and 3, stay frozen by policy.

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

The check as built differs from the design in six ways. This record refines the
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
- The accepted-sources dump covers frozen copies of the packages under
  `examples/` and `evals/corpus/`; it does not include the fences of
  `docs/book/`, which a test already compiles.
- The wire dump holds the encoding names, the `.proto` and `.fbs` lines for the
  corpus, the catalog hash of the cabin example, and the test vectors of the
  WebSocket binding (the frame specification §11.1), which cover the frame
  envelope and the `attach` handshake.
- The minimum of Rust 1.83 and edition 2021 applies to `ridl-rt` and
  `ridl-rt-conformance` only. The `msrv` dump also records the edition of
  `ridl-loopback` and `ridl-descriptor`, which are edition 2024 today, so a
  change of their edition fails the check, but the minimum does not apply to
  them. The generated crate compiles as edition 2021 with Rust 1.83, which
  `just compat-check` checks and `just freeze-check` does not.

## Decision

1. **Ten surfaces are frozen from 0.10.0.** A change that removes or alters what
   a user depends on in one of them is a break.

   | Surface                 | What a user depends on                                                                                                                  |
   | ----------------------- | --------------------------------------------------------------------------------------------------------------------------------------- |
   | Language                | typl, ridl and rsdl syntax and keywords; the set of sources that compile without an error                                               |
   | Manifest                | `ridl.toml` keys and their value types, `interfaces.lock`, `.ridl/baseline`                                                             |
   | Diagnostics and lints   | diagnostic codes, lint names, lint default levels                                                                                       |
   | CLI                     | subcommands, flags, exit codes, the `--format json` and `--format sarif` output shapes                                                  |
   | Generated Rust crate    | the domain types and the interaction face (`Client`, `Provider`, `Publisher`, `Event`, `serve`, `Bind`)                                 |
   | Runtime crates          | the public API of `ridl-rt`, `ridl-rt-conformance`, `ridl-loopback` and `ridl-descriptor`, and their cargo feature names                |
   | Codegen plugin protocol | `ridl.ir.v2` and `ridl.codegen.v1`, the request and response of a plugin                                                                |
   | MCP tools               | tool names, input schemas and output schemas                                                                                            |
   | Wire format             | the frame envelope, the `attach` handshake, the inputs of the catalog hash, the emitted `.proto` and `.fbs` mapping, the encoding names |
   | Minimum Rust version    | Rust 1.83 and edition 2021 for `ridl-rt` and `ridl-rt-conformance`; the generated crate compiles with them (`just compat-check`)        |

   The frozen minimum of the last row is Rust 1.83 and edition 2021 for
   `ridl-rt` and `ridl-rt-conformance`, checked by the `msrv` dump. The
   generated crate compiles as edition 2021 with that Rust version, checked by
   `just compat-check`. The `msrv` dump also records the edition of
   `ridl-loopback` and `ridl-descriptor`, so a change of their edition fails the
   check, but the minimum of Rust 1.83 and edition 2021 does not apply to them.

   The formats of `interfaces.lock` and `.ridl/baseline` are frozen by policy
   and checked by no tool. Of the exit codes, the check records only those of
   `ridl check`; the exit codes of the other subcommands are frozen by policy.

2. **These are not frozen.**
   - Size and budget values, for example header sizes and size sums.
   - The text of a diagnostic message, of a doc comment and of generated docs.
   - Anything that is only added: a new subcommand, flag, key, lint, field,
     tool, cargo feature or transport default. On an existing subcommand or
     tool, a new flag, argument or MCP property passes only when it is optional:
     a new required CLI flag or argument, or a new required MCP input property,
     fails the check, as does one that becomes required. A new subcommand or
     tool may have required ones.
   - A new lint, which starts at the `allow` or `info` level.
   - The crates that are not published: `xtask`, `ridlc-gen-model` and
     `ridlc-gen-rust`.

3. **The gate is one recipe, `just freeze-check`, with two kinds of check.** The
   recipe runs `cargo xtask freeze check`. It is a member of `just build`, of
   the `expected` list of `gate-parity` and of the CI workflow (ADR-0009).
   - _Line dumps._ `freeze/<surface>.txt` is the sorted, deduplicated dump of
     one surface, one fact per line. The dumps are: keywords with the whole
     reserved set, accepted sources, manifest keys with their value types,
     diagnostic codes with lint names and default levels, the CLI (subcommands,
     flags with value kinds, whether each flag and positional argument is
     required, the exit codes of `ridl check`, the key paths of the JSON and
     SARIF reports of `ridl check`), the codegen protocol fields with the
     reserved numbers and names and the whole value set of each enum, the MCP
     tool and schema paths with the type of each property, whether it is
     required, the required set of each schema object that has `properties` or
     `required`, each enum value and const, and each `additionalProperties`,
     `minimum` and `format`, the wire mapping (encoding names, the `.proto` and
     `.fbs` lines for the corpus, the catalog hash of the cabin example, the
     test vectors of the frame specification §11.1) and the minimum Rust version
     and edition. The check fails when a committed dump differs from a
     regeneration. For a surface with a row in `freeze/surfaces.toml`, it also
     fails when a line of the dump at the row's `baseline` ref is missing from
     the dump at `HEAD`. An added line passes. A changed line is a removed line
     plus an added line, so it fails. The accepted-sources and wire dumps read
     frozen copies of the packages under `examples/` and `evals/corpus/`, kept
     under `xtask/tests/freeze-fixtures/sources/` at the path of each original,
     and the catalog hash is read from the generated-crate copy of
     `examples/cabin`. An edit to an example or to the corpus therefore changes
     no dump. A copy is refreshed on purpose: adding a copy adds lines, and
     replacing a copy so that a line is removed is a break. The frame vectors
     are read from the frame specification itself, so an edit to a vector
     changes the dump.

     Some lines hold a whole set or a requiredness, so a change that adds to a
     set, or that makes a requirement looser, also removes a line and fails. A
     codegen-protocol enum gains a value only in a minor release, such as 0.11,
     and not in a 0.10.x release, because a plugin rejects an enum value that it
     does not know. In the same way, a new reserved word, an MCP property or a
     CLI flag or argument that becomes optional, and any change of a value in
     the `msrv` dump each need the approval of decision 4. In the other
     direction, a new required CLI flag or argument or MCP property, or one that
     becomes required, fails the check; a new optional one passes.
   - _Rust API comparison._ The runtime crates and the generated crate are
     compared with `cargo semver-checks`, because a line dump cannot classify a
     Rust API change correctly, for example a new required method on a public
     trait. The runtime crates are compared with the same crates at the baseline
     ref. The generated crates are committed under `freeze/generated/` from the
     frozen fixtures named in the Context, and each is compared with the same
     crate in the baseline's `freeze/generated/`. The check first runs
     `cargo fetch --locked` once with the network allowed, then runs the
     comparison offline and with the release type `minor`. Ten lints of
     `cargo semver-checks` that require a major version but report only a
     warning are raised to `deny`, so the break they find fails the check.

   `freeze/surfaces.toml` is the authoritative baseline record: one row per
   surface with a `name` and a `baseline`. Without a row, a line-dump surface is
   checked only against its committed dump, `generated-crate` is checked only
   for equality with a regeneration, and `runtime-crates` is not checked. The
   output of `ridl diff` and the `ridl-diff` categories, the exit codes of every
   subcommand other than `ridl check`, the formats of `interfaces.lock` and
   `.ridl/baseline`, the layouts of the `unsubscribe`, `read` and `occurrence`
   frames, which no test vector of the frame specification §11.1 shows, the
   payload-presence rule of §11.1 for a `publish` under `Init` or
   `Invalid(Declared)` and for a `response` under `reply`, which no vector
   shows, and the codes of §4 and §9.6 that no vector carries, among them the
   `kind` values 2 and 5 and the `attach` encodings 2 and 3, are in no dump;
   they are frozen by policy only. This record is the one that states the status
   of `ridl diff` and of the `ridl-diff` categories.

4. **A break of a frozen surface is approved by removing its row, then adding
   the row again.** `git show <baseline>:freeze/<surface>.txt` must read a dump
   that already holds the change, and a commit of a branch disappears after a
   squash merge, so the new baseline is the merge commit of the pull request
   that made the break.
   - Before 0.10.0, the pull request that breaks a surface deletes that
     surface's row from `freeze/surfaces.toml`, carries `!` in its title, states
     the approval in its description, and merges only on the maintainer's
     explicit approval. The name of the surface stays in the `required` list of
     `freeze/surfaces.toml`. A follow-up pull request adds the row again with
     that merge commit as its `baseline`. The surface is unchecked between the
     two pull requests. While its name is in `required` and it has no row,
     `just freeze-check` prints a notice, and `just release` fails. The check
     guards only a forgotten re-add: removing a name from `required` is an edit
     of `freeze/surfaces.toml` that review must catch.
   - At 0.10.0, the release pull request moves every baseline to `v0.10.0`.
   - During 0.10.x, no break lands. The one exception is a security fix; its
     reason is recorded in this record, in the same pull request that makes the
     fix.
   - 0.11 starts with a pull request that breaks surfaces on purpose, with the
     same two steps.

5. **The minimum supported Rust version is a frozen surface.** Raising the
   `rust-version` of `ridl-rt` or `ridl-rt-conformance`, or dropping edition
   2021 for them, is a break, and the `msrv` dump checks both crates. The
   generated crate must compile as edition 2021 with that Rust version, which
   `just compat-check` checks. The `msrv` dump also records the edition of
   `ridl-loopback` and `ridl-descriptor`, so a change of their edition fails the
   check, but the minimum does not apply to them. This restates ADR-0021
   decision 10 as a frozen surface and adds the `msrv` check.

6. **The gate table says which gate covers which surface.** The `baseline`
   column is empty on the day this record is accepted. `freeze/surfaces.toml` is
   the authoritative record of every baseline; this table lists the coverage and
   the work that switches each gate on, and a gate is on when its surface has a
   row there.

   | Surface                 | Row names in `surfaces.toml`                                                                | Switched on by                       | Baseline   |
   | ----------------------- | ------------------------------------------------------------------------------------------- | ------------------------------------ | ---------- |
   | Language                | `keywords`, `accepted-sources`                                                              | the language lane                    |            |
   | Manifest                | `manifest`                                                                                  | the language lane                    |            |
   | Diagnostics and lints   | `diagnostics`                                                                               | the language lane                    |            |
   | CLI                     | `cli`                                                                                       | the developer-experience lane        |            |
   | Generated Rust crate    | `generated-crate` (`cargo semver-checks`)                                                   | the codegen lane                     |            |
   | Runtime crates          | `runtime-crates` (`cargo semver-checks`)                                                    | the wire and runtime lane            |            |
   | Codegen plugin protocol | `codegen-protocol`                                                                          | the codegen lane                     |            |
   | MCP tools               | `mcp`                                                                                       | the release and policy lane, at once | `3c3301df` |
   | Wire format             | `wire`: encoding names, `.proto` and `.fbs` lines, catalog hash; frames are policy only now | the wire and runtime lane            |            |
   | Minimum Rust version    | `msrv`                                                                                      | the wire and runtime lane            |            |

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
- **Index `anyOf` branches of an MCP schema under their own paths.** Rejected:
  every `anyOf` in the tool schemas wraps one value branch and a `null` branch,
  which is how a nullable value is written. Indexing them would move every
  nullable property under `/anyOf/0` and make a reordering of `[T, null]` a
  break, with no gain. Only `oneOf`, which lists alternative shapes, is indexed.
- **Fail every `just freeze-check` run while a required surface has no row.**
  Rejected: `main` would be red between the two pull requests of an approved
  break, and unrelated pull requests would be blocked. The risk is a release
  that ships a frozen surface without a baseline, and the release check covers
  exactly that.
- **A CI job that counts merges since a row was removed.** Rejected: it needs
  state outside the tree and is harder to test than a list in `surfaces.toml`.

## Consequences

- A pull request that breaks a frozen surface fails `just freeze-check` while
  the surface has a row. The pull request deletes the row, and the deleted row
  is the visible approval, and the name stays in the `required` list. A
  follow-up pull request adds the row again at the merge commit of the first.
  `just release` fails while a name in `required` has no row.
- Adding a row for a new surface needs two pull requests: the first adds the
  dump, and the second adds the row with the first one's merge commit as its
  baseline. The pull request that adds a surface's row also adds its name to
  `required` in `freeze/surfaces.toml`, and the check fails when a row's name is
  not in `required`.
- Before the Rust API comparison, the check runs `cargo fetch --locked` with the
  network allowed, so its first run on a machine with an empty cargo cache needs
  the network. The comparison itself runs offline.
- ADR-0021 decision 10 and the stability notes of the records listed in the
  Status keep their text. This record owns the policy and the check.

## References

- `freeze/README.md` and `freeze/surfaces.toml`
- `xtask/src/freeze.rs` and `xtask/src/freeze/`
- [ADR-0009](ADR-0009-toolchain-and-gate-parity.md) — the gate recipes
- [Stability, in the book](../book/stability.md)
- `CONTRIBUTING.md`, "Changing a frozen surface"
