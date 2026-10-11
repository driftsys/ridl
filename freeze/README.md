# freeze

This directory holds the dumps of the frozen surfaces and the table that says
which release each surface is frozen against (ADR-0029).

- `<surface>.txt` is the sorted, deduplicated dump of one surface, one line per
  fact. A dump is generated, never edited by hand.
- `surfaces.toml` has one `[[surface]]` table per frozen surface, with a `name`
  and a `baseline` (a git ref). The check fails when a line of the dump at the
  baseline is missing from the dump at HEAD. An added line passes.

## The frozen source copies

The `accepted-sources` and `wire` dumps read frozen copies of the packages under
`examples/` and `evals/corpus/`, not the packages themselves. The copies are in
`xtask/tests/freeze-fixtures/sources/`, each at the path of its original (for
example `xtask/tests/freeze-fixtures/sources/examples/cabin`), and each holds
the package's `ridl.toml` and its `.typl`, `.ridl` and `.rsdl` files. A dump
line names the path of the original. The catalog hash of the `wire` dump is read
from `xtask/tests/freeze-fixtures/generated/cabin`, the copy of `examples/cabin`
that the `generated-crate` surface reads. An edit to `examples/` or to
`evals/corpus/` therefore changes no dump.

To refresh a copy on purpose, copy the package's files over it and run
`cargo xtask freeze update`. A new copy adds lines and passes. A replaced copy
that removes a line is a break of the surface (see "Approve a break").

## The Rust API surfaces

Two surfaces are Rust APIs, compared with `cargo semver-checks` and not as
lines. Neither has a `<surface>.txt`.

- `runtime-crates`: `ridl-rt`, `ridl-rt-conformance`, `ridl-loopback` and
  `ridl-descriptor`, compared with the same crates at the baseline.
- `generated-crate`: each directory `generated/<name>/` holds a crate that
  `ridl build --emit rust` writes, from the source packages under
  `xtask/tests/freeze-fixtures/generated/`. Those are frozen copies of
  `examples/cabin` and of `crates/ridlc/tests/corpus/veh-cluster`, so an edit to
  `examples/` or to the corpus is not a break of this surface. The check fails
  when a crate differs from a regeneration. With a row, each crate is compared
  with the same crate in the baseline's `generated/`, on copies outside the
  repository that read `ridl-rt` from `crates/ridl-rt`.

cargo-semver-checks reports some lints that require a major version only as
warnings. The check denies each of them (`DENIED_LINTS` in
`xtask/src/freeze/rust_api.rs`, and
`[workspace.metadata.cargo-semver-checks.lints]` in the root `Cargo.toml`), so
the break they find fails the check. The list was read from cargo-semver-checks
0.51.0, and must be checked again when the version pinned in `bootstrap` and in
`.github/workflows/ci.yml` changes.

The comparison runs without the network, so the dependencies must already be in
the local cargo cache. A build of the workspace puts them there.

## Update a dump

Run `cargo xtask freeze update` and commit the changed files. The recipe
`just freeze-check` runs `cargo xtask freeze check`, which fails when a
committed dump differs from the regenerated one.

## Approve a break

A removed or changed line, or a Rust API break, is a break of the frozen
surface. The baseline copy is read with
`git show <baseline>:freeze/<surface>.txt`, so a new baseline must be a commit
that already holds the changed dump, and a branch commit disappears after a
squash merge. To accept a break:

1. In the pull request that makes the break, delete that surface's row from
   `surfaces.toml`, and state the approval and the reason in the pull request
   description. The deleted row is the visible record of the approval.
2. After that pull request merges, add the row again in a follow-up pull
   request, with the merge commit of the first as the `baseline`.

The surface is unchecked between the two pull requests.
