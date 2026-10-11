# freeze

This directory holds the dumps of the frozen surfaces and the table that says
which release each surface is frozen against (ADR-0029).

- `<surface>.txt` is the sorted, deduplicated dump of one surface, one line per
  fact. A dump is generated, never edited by hand.
- `surfaces.toml` has one `[[surface]]` table per frozen surface, with a `name`
  and a `baseline` (a git ref). The check fails when a line of the dump at the
  baseline is missing from the dump at HEAD. An added line passes.

## The Rust API surfaces

Two surfaces are Rust APIs, compared with `cargo semver-checks` and not as
lines. Neither has a `<surface>.txt`.

- `runtime-crates`: `ridl-rt`, `ridl-rt-conformance`, `ridl-loopback` and
  `ridl-descriptor`, compared with the same crates at the baseline.
- `generated-crate`: `generated/<name>/` is the crate that
  `ridl build --emit
  rust` writes for `examples/cabin` and for
  `crates/ridlc/tests/corpus/veh-cluster`. The check fails when it differs from
  a regeneration. With a row, each crate is compared with the same crate in the
  baseline's `generated/`, on copies outside the repository that read `ridl-rt`
  from `crates/ridl-rt`.

The comparison runs without the network, so the dependencies must already be in
the local cargo cache. A build of the workspace puts them there.

## Update a dump

Run `cargo xtask freeze update` and commit the changed files.
`just
freeze-check` runs `cargo xtask freeze check`, which fails when a
committed dump differs from the regenerated one.

## Approve a break

A removed or changed line is a break of the frozen surface. To accept it, change
the `baseline` of that surface in `surfaces.toml` to a newer ref, in the same
pull request and with the reason in the pull request description. The change to
`surfaces.toml` is the visible record of the approval.
