# freeze

This directory holds the dumps of the frozen surfaces and the table that says
which release each surface is frozen against (ADR-0029).

- `<surface>.txt` is the sorted, deduplicated dump of one surface, one line per
  fact. A dump is generated, never edited by hand.
- `surfaces.toml` has one `[[surface]]` table per frozen surface, with a `name`
  and a `baseline` (a git ref). The check fails when a line of the dump at the
  baseline is missing from the dump at HEAD. An added line passes.

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
