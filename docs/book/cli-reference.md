# CLI reference

Two command-line binaries ship from this repository: **`ridl`**, the porcelain
facade, and **`ridlc`**, the plumbing compiler underneath it. A third binary,
`ridl-lsp`, builds too — the language server an editor drives over stdio (see
[What is built](introduction.md#what-is-built)) — but it takes no subcommand
and no flag of the kind this page documents, so it has no place here. Build
all three with:

```sh
cargo build --release
```

as [Getting started](getting-started.md#building-the-toolchain) describes.
This page assumes `target/release/ridl` and `target/release/ridlc` are on your
`PATH`, or called by path.

Only typl and ridl have a toolchain — see
[What is built](introduction.md#what-is-built). Every command and transcript
below runs against a workspace built from those two layers; rxdl, rmdl, and
rsdl accept no command here.

Every synopsis and transcript on this page is the literal output of the
binaries built from this repository, run against constructed fixtures. The
fixtures themselves are not reproduced here — this page documents command
behavior, not how to write a `.ridl` file, and a fixture built for one
transcript is often a one-line edit of the fixture built for the transcript
before it. [Getting started](getting-started.md) is the tutorial that shows
complete, worked files; treat a transcript below as a faithful record of what
the tool printed against *some* input shaped as described, not as a listing you
can paste in whole. Every exit code stated below was observed by running the
command and reading its exit status, not read out of a help string or a
specification.

`ridl --version` and its short form `ridl -V` (and `ridlc --version`) print the
binary's own name and version and exit 0:

```sh
ridl --version
```

```text
ridl 0.0.0
```

The version string is `0.0.0` on every build until a maintainer cuts a release
([ADR-0007][adr-0007] decision 14 pins it there), so it cannot yet answer
"which commit is this" — recorded as a known gap in [ADR-0010][adr-0010]
decision 8, which also records a deferred fix.

## `ridl`

```sh
ridl --help
```

```text
The RIDL toolchain

Usage: ridl <COMMAND>

Commands:
  check     Type-check a file, package, or workspace (defaults to the current directory)
  baseline  Publish the current workspace as a baseline: one `<pkg-name>.ir.json` snapshot per package, written to `.ridl/baseline/` at the workspace root
  build     Compile to the selected artifacts (defaults to the current directory)
  test      Run the property suite over a workspace: the range self-corpora and the contract-clause sampling (ridl §13). Exit 0 when every run passes, 1 on a self-corpus failure or an evaluation error, 2 on a compile error
  fmt       Reformat `.typl` and `.ridl` files in place (defaults to the current directory)
  diff      Compare two IR snapshots or source trees and classify the change: exit 0 compatible or identical, 1 breaking, 2 error
  help      Print this message or the help of the given subcommand(s)

Options:
  -h, --help     Print help
  -V, --version  Print version
```

Running `ridl` with no subcommand at all prints this same text to **stderr**
and exits 2; `ridl --help` prints it to **stdout** and exits 0 — the two
routes carry identical text but are not interchangeable in a script that
checks the exit code or reads the right stream.

### `ridl check`

```sh
ridl check --help
```

```text
Type-check a file, package, or workspace (defaults to the current directory)

Usage: ridl check [OPTIONS] [PATH]

Arguments:
  [PATH]  [default: .]

Options:
      --frozen               Verify remote imports against `ridl.lock` without fetching or regenerating it (CI mode, ADR-0002 §7)
      --baseline <DIR|FILE>  Compare the checked workspace against a published baseline — a directory of `.ir.json` snapshots or one snapshot file — and warn (RIDL-407) on every interaction whose ordinal moved. Without the flag, `.ridl/baseline/` at the workspace root is used when it exists
  -h, --help                 Print help
```

`PATH` is a `.typl`/`.ridl` file, a package directory, or a workspace root, and
defaults to the current directory. `--frozen` is the same flag
[`ridlc check --frozen`](#ridlc-check) documents, forwarded unchanged; the two
commands share one implementation and render byte-identical diagnostics on the
same input. It now carries this description word for word under `ridl` too —
before [ADR-0010][adr-0010], `ridl check --help` rendered it with a blank line
where the description belongs.

**It writes `~/.ridl/cache` and `ridl.lock`, but only when the manifest
declares `[imports]`.** `ridl check` loads, resolves, and checks the
workspace, then — non-frozen, and only when the checked-out manifest (or a
package in it) has an `[imports]` table — materializes every remote import
into `~/.ridl/cache` (ADR-0002 §7, content-addressed by URL and by the
fetched artifact's SHA-256) and regenerates `ridl.lock` at the workspace root
on a clean run. Every other fixture on this page has no `[imports]`, so none
of them reach either write; over a manifest pointed at a package served from
a local HTTP stub:

```sh
ridl check && cat ridl.lock
```

```text
# ridl.lock — generated by ridlc; do not edit by hand.
# Regenerated on every successful resolution (ADR-0002 §7).

[entries."http://127.0.0.1:8934/remotepkg.tar"]
sha256 = "f25092bff94094dca3eccf057a66014df4d0df9869ce81e556d08a60e34714a5"
```

`--frozen` verifies against the existing `ridl.lock` instead of fetching or
regenerating it, exactly as its description says: with no `ridl.lock` on disk
yet, `ridl check --frozen` over the same workspace fails closed (`MANI-103`,
exit 1) rather than writing one. `ridl test` and `ridl diff`, checked against
the same import-bearing workspace, write nothing at all — neither calls the
materialization step this paragraph describes, only the plain compile.

**Exit codes.** 0 when nothing is wrong — a clean run prints nothing at all,
to either stream. 1 when a diagnostic is an error, for example a range whose
minimum exceeds its maximum:

```sh
ridl check
```

```text
error[TYPL-104]: range minimum 250 is greater than maximum 0
  ┌─ ./demo.ridl:3:19
  │
3 │ type Speed : km/h [250.0..0.0 step 0.5]
  │                   ^^^^^^^^^^^^^^^^^^^^^

```

2 when the workspace itself cannot be found:

```sh
ridl check
```

```text
error: no `ridl.toml` found at or above `.`
```

An explicit `--baseline` naming a path that does not exist is the same exit
2, for the same reason — asking for a baseline that is not there is a mistake
worth reporting, while a missing *default* baseline is silently skipped:

```sh
ridl check --baseline ./nope
```

```text
error: the baseline `./nope` does not exist
```

A `--baseline` directory that holds IR artifacts but no `.ir.json` snapshot
is also exit 2, with a message naming an artifact it found: a baseline stays
`.ir.json` ([ADR-0014][adr-0014] decision 5), so such a directory is a
baseline in a refused encoding, not the silently skipped "no baseline
published yet" state — which an *empty* directory still is.

**The baseline desk check.** With `.ridl/baseline/` present at the workspace
root — written by [`ridl baseline`](#ridl-baseline) — `ridl check` compares
the workspace against it and warns (RIDL-407) on every interaction whose
declaration order moved, without moving the exit code. Reordering two events
in a published interface:

```sh
ridl check
```

```text
warning[RIDL-407]: `doorOpened` has moved in `VehicleStatus` since the published baseline (position 1 there, position 2 here). Declaration order is the wire identity of an interaction (ridl §11), so a consumer built against the baseline would now bind this slot to a different interaction — put the declarations back in the baseline's order and add new ones at the end
  ┌─ ./demo.ridl:7:3
  │
7 │   event doorOpened : DoorState @[100ms..1s]
  │   ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

warning[RIDL-407]: `doorClosed` has moved in `VehicleStatus` since the published baseline (position 2 there, position 1 here). Declaration order is the wire identity of an interaction (ridl §11), so a consumer built against the baseline would now bind this slot to a different interaction — put the declarations back in the baseline's order and add new ones at the end
  ┌─ ./demo.ridl:6:3
  │
6 │   event doorClosed : DoorState @[100ms..1s]
  │   ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

```

That run exits 0: two RIDL-407 warnings and an otherwise clean compile stay
clean. The desk check runs only after a compile with no error diagnostic, so
a workspace that fails to check at all draws no RIDL-407 warning on top of its
real problem — it just exits 1, exactly as it would with no baseline present.

### `ridl baseline`

```sh
ridl baseline --help
```

```text
Publish the current workspace as a baseline: one `<pkg-name>.ir.json` snapshot per package, written to `.ridl/baseline/` at the workspace root

Usage: ridl baseline [OPTIONS] [PATH]

Arguments:
  [PATH]  [default: .]

Options:
      --out <DIR>  Write the snapshots here instead of `.ridl/baseline/`
  -h, --help       Print help
```

**It writes** one `<package-name>.ir.json` file per package in the workspace,
under `.ridl/baseline/` at the workspace root or under `--out` when given —
and, exactly like [`ridl check`](#ridl-check), `ridl.lock` at the workspace
root when the manifest declares `[imports]` (`ridl baseline` builds through
`ridlc build`, non-frozen, so the same materialization step runs). Publishing
the snapshots is wholesale: the target directory ends up holding exactly the
snapshots the workspace declares now, and nothing else in that directory is
touched. A two-member workspace with no `[imports]` writes two files and no
lockfile:

```sh
ridl baseline && find .ridl/baseline -type f | sort
```

```text
.ridl/baseline/veh.cluster.ir.json
.ridl/baseline/veh.common.ir.json
```

**Exit codes.** 0 on a clean publish. 1 when a diagnostic is an error — the
existing baseline is left exactly as it was, because a re-publish must never
destroy a good baseline with a broken one:

```sh
ridl baseline
```

```text
error: unknown type name `Nope`
  ┌─ ./demo.ridl:4:22
  │
4 │   event doorClosed : Nope @[100ms..1s]
  │                      ^^^^

```

2 when the workspace itself cannot be found, the same as `ridl check`:

```sh
ridl baseline /nonexistent/ridl/workspace
```

```text
error: `/nonexistent/ridl/workspace` does not exist
```

### `ridl build`

```sh
ridl build --help
```

```text
Compile to the selected artifacts (defaults to the current directory)

Usage: ridl build [OPTIONS] [PATH]

Arguments:
  [PATH]
          [default: .]

Options:
      --out-dir <OUT_DIR>
          [default: out]

      --emit <EMIT>
          Possible values:
          - rust:       Idiomatic Rust source, written to `<base>.rs`
          - c-header:   The extern-C header, written to `<base>.h`
          - ir-json:    The lowered IR v2 as exact-decimal JSON, written to `<base>.ir.json`
          - ir-text:    The lowered IR v2 as prototext, written to `<base>.ir.txtpb`
          - ir-binary:  The lowered IR v2 as protobuf binary, written to `<base>.ir.binpb`
          - typescript: Idiomatic TypeScript source, written to `<base>.ts`
          
          [default: rust]

      --frozen
          Verify remote imports against `ridl.lock` without fetching or regenerating it (CI mode, ADR-0002 §7)

  -h, --help
          Print help (see a summary with '-h')
```

`--emit` accepts a comma-separated list, so one invocation can write several
targets. `--frozen` is the same flag as on `ridl check`: it is
[`ridlc build --frozen`](#ridlc-build), documented word for word since
[ADR-0010][adr-0010].

**It writes** one file per package per `--emit` target, under `--out-dir`
(`out` by default), and — exactly like [`ridl check`](#ridl-check) —
`ridl.lock` at the workspace root when the manifest declares `[imports]`,
non-frozen. `<base>` in the `--emit` list above is the package name when
`PATH` is a package directory or a workspace root, and the input file's stem
in single-file mode.

When a package names a type from `ridl.std`, the standard package is written
beside your own as one more file per `--emit` target — `ridl.std.rs`,
`ridl.std.h`, `ridl.std.ts` — because generated code refers to standard types
by package path and does not compile without it. The three IR targets —
`ir-json`, `ir-text`, `ir-binary` — get no such file: a direct IR dump
records the packages the workspace declares, and `ridl.std` ships with the
compiler rather than with the workspace ([ADR-0007][adr-0007] decision 15).

Building a workspace of two packages that name no standard type, with no
`[imports]`:

```sh
ridl build --out-dir out && find out -type f | sort
```

```text
out/veh.cluster.rs
out/veh.common.rs
```

and a single `.typl` file with `--emit rust,ir-json`:

```sh
ridl build speed.typl --emit rust,ir-json --out-dir out && find out -type f | sort
```

```text
out/speed.ir.json
out/speed.rs
```

**Exit codes.** 0 when the build compiles clean and every requested artifact
is written. 1 when a diagnostic is an error — nothing is written for a
package that fails to compile:

```sh
ridl build --out-dir out
```

```text
error: unknown type name `Bogus`
  ┌─ ./demo.ridl:4:25
  │
4 │   signal currentSpeed : Bogus @10ms
  │                         ^^^^^

```

2 when the workspace itself cannot be found:

```sh
ridl build /nonexistent/ridl/workspace --out-dir out
```

```text
error: `/nonexistent/ridl/workspace` does not exist
```

### `ridl test`

```sh
ridl test --help
```

```text
Run the property suite over a workspace: the range self-corpora and the contract-clause sampling (ridl §13). Exit 0 when every run passes, 1 on a self-corpus failure or an evaluation error, 2 on a compile error

Usage: ridl test [OPTIONS] [PATH]

Arguments:
  [PATH]  [default: .]

Options:
      --samples <SAMPLES>  Random parameter tuples drawn per `require` clause (minimum 1). Each clause also runs its parameters' boundary corpus, which is drawn first and is not counted here, so the total per clause is larger [default: 256]
      --format <FORMAT>    Output format for the report [default: text] [possible values: text, json]
  -h, --help               Print help
```

`ridl test` runs two checks per package: every constrained named type's
boundary and violation corpus against the constraint validator (a
self-consistency check of the toolchain, not of the model), and every
`require` clause's satisfiability, sampled over each parameter's boundary
values plus `--samples` random draws. `ensure` clauses are listed as observer
stubs and never evaluated — there is no runtime to produce the `result` they
need. The report goes to **stdout**; a compile error's diagnostics go to
stderr instead, and no report prints at all.

**It writes nothing, ever.** The report is printed, never saved. `ridl test`
runs the plain compile only — never the materialization step that makes
[`ridl check`](#ridl-check) write `ridl.lock` — so pointing the same
`[imports]`-declaring workspace at `ridl test` produces no lockfile.

A clean run over an interface with two `require` clauses:

```sh
ridl test --samples 8
```

```text
package cli.demo
  ranges
    Speed  ok — 4 boundary accepted, 2 violations rejected
  requires
    Cruise.setTargetSpeed.require[0]  ok — 3 boundary + 8 random of 12 satisfied  (speed > 0.0)
    Cruise.setTargetSpeed.require[1]  ok — 4 boundary + 8 random of 12 satisfied  (speed <= 250.0)
  ensures
    (none)
  summary — requires: 2 total, 2 evaluated; ensures: 0 listed
```

The same run with `--format json` prints one JSON object per package, on one
line:

```sh
ridl test --samples 8 --format json
```

```text
[{"contracts":[{"boundary_samples":4,"detail":null,"discarded_samples":0,"id":"Cruise.setTargetSpeed.require[0]","random_samples":8,"samples":12,"satisfied":11,"source":"speed > 0.0","status":"ok"},{"boundary_samples":4,"detail":null,"discarded_samples":0,"id":"Cruise.setTargetSpeed.require[1]","random_samples":8,"samples":12,"satisfied":12,"source":"speed <= 250.0","status":"ok"}],"package":"cli.demo","ranges":[{"boundary":4,"status":"ok","type":"Speed","violations":2}],"summary":{"ensures_listed":0,"nothing_evaluated":false,"requires_constant_false":0,"requires_errored":0,"requires_evaluated":2,"requires_skipped":0,"requires_suspect":0,"requires_total":2}}]
```

**Exit codes.** 0 above. 1 when a range self-corpus fails, or — the case a
`require` clause reaches in practice — evaluating one raises an error, such as
a division by zero every sampled input hits:

```sh
ridl test
```

```text
package cli.demo
  ranges
    Divisor  ok — 4 boundary accepted, 2 violations rejected
  requires
    I.c.require[0]  ERROR — division by zero while evaluating `100 / (d - d) > 1`
  ensures
    (none)
  summary — requires: 1 total, 0 evaluated, 1 errored; ensures: 0 listed
  WARNING: no require clause was evaluated — this run tested no precondition (0 skipped of 1)
```

2 when the workspace fails to compile, when the workspace cannot be found, or
when `--samples 0` is given — sampling zero values is refused rather than
silently clamped, because it would report every clause as unsatisfiable and
call that a finding:

```sh
ridl test
```

```text
error[TYPL-104]: range minimum 250 is greater than maximum 0
  ┌─ ./demo.ridl:2:20
  │
2 │ type Broken : km/h [250.0..0.0]
  │                    ^^^^^^^^^^^^

warning[TYPL-102]: `float` without both a range and a `step`
  ┌─ ./demo.ridl:2:15
  │
2 │ type Broken : km/h [250.0..0.0]
  │               ^^^^

```

```sh
ridl test --samples 0
```

```text
error: `--samples` must be at least 1
```

Note the overloaded meaning: exit 1 here is a **test failure** (the toolchain
found a problem worth failing the run over), not the *breaking* verdict
`ridl diff` uses the same code for, and not the *diagnostic error* `ridl
check` uses it for. A `suspect` finding — no sampled input satisfies a
precondition — is reported in the same output but does **not** fail the run;
only a self-corpus failure or an evaluation error does.

### `ridl fmt`

```sh
ridl fmt --help
```

```text
Reformat `.typl` and `.ridl` files in place (defaults to the current directory)

Usage: ridl fmt [OPTIONS] [PATH]

Arguments:
  [PATH]  [default: .]

Options:
      --check  Do not write; exit 1 if any file would change
  -h, --help   Print help
```

**It writes** every `.typl`/`.ridl` file under `PATH` back to itself in
canonical form, unless `--check` is given or the file fails to parse.

**Exit codes.** 0 when nothing needed rewriting, or the rewrite (without
`--check`) succeeded. 1 under `--check` when a file would change, without
writing it. Starting from `speed.typl` holding
`type Speed  :  km/h [0.0..250.0 step 0.5]` on one line, with no blank line
after the package declaration:

```sh
ridl fmt --check speed.typl; echo "exit: $?"
```

```text
exit: 1
```

Reformatting it for real, then checking again:

```sh
ridl fmt speed.typl && cat speed.typl
```

```text
package veh.common

type Speed: km/h [0.0..250.0 step 0.5]
```

`ridl fmt` inserted the blank line and tightened the colon; the shown text is
`cat`'s, since `ridl fmt` itself printed nothing. A second `--check` pass is
now a fixed point:

```sh
ridl fmt --check speed.typl; echo "exit: $?"
```

```text
exit: 0
```

and 1 — with or without `--check` — when a file has a parse error; the file
is never rewritten, and the parse diagnostics render to stderr:

```sh
ridl fmt broken.typl
```

```text
error[FORM-101]: expected `]`
  ┌─ broken.typl:2:22
  │
2 │ type X : integer [0..10ms]
  │                      ^^^^

error[TYPL-302]: duration literal in typl context
  ┌─ broken.typl:2:22
  │
2 │ type X : integer [0..10ms]
  │                      ^^^^

```

2 when an existing file cannot be read or written — a permissions error, for
example:

```sh
ridl fmt unreadable.typl
```

```text
error: cannot read unreadable.typl: Permission denied (os error 13)
```

**A missing `PATH`, or a directory the walk cannot read, is the same exit 2 —
naming the cause.** This is a fix, recorded as [ADR-0010][adr-0010] decision 6
and decision 1: before it, `ridl fmt /nonexistent` walked the missing path,
found nothing, and reported success, and an unreadable directory reached
partway through a larger walk was silently treated the same way — the one
subcommand on this page that did not fail closed on a bad path. Both now
report the directory `read_dir` failed on and the OS error:

```sh
ridl fmt /nonexistent/ridl/workspace
```

```text
error: cannot read /nonexistent/ridl/workspace: No such file or directory (os error 2)
```

and, for a tree holding one ordinary file next to a subdirectory the walk
cannot read into:

```sh
ridl fmt --check .
```

```text
error: cannot read ./sub: Permission denied (os error 13)
```

Of the eight subcommands this page documents, [ADR-0010][adr-0010] decision 6
found `ridl fmt` is the only one that reliably names the actual unreadable
path this way in every case it was tested against. `ridl check`, `ridl build`,
`ridl baseline`, `ridlc check`, and `ridlc build` still exit 2 on the same
inputs, but with the wrong cause or none: an unreadable *workspace root*
reports `` error: no `ridl.toml` found at or above `<path>` `` — confirmed
directly against this build — and an unreadable subdirectory nested inside an
otherwise-readable workspace reports a bare `error: Permission denied (os
error 13)`, naming no path at all — also confirmed directly. Tracked as
[issue driftsys/ridl#196][issue-196], not fixed as of this page.

### `ridl diff`

```sh
ridl diff --help
```

```text
Compare two IR snapshots or source trees and classify the change: exit 0 compatible or identical, 1 breaking, 2 error

Usage: ridl diff [OPTIONS] [OLD] [NEW]

Arguments:
  [OLD]  The baseline: an `.ir.json` snapshot, a `.typl`/`.ridl` file, a package directory, or a workspace root
  [NEW]  The candidate, in the same forms as the baseline

Options:
      --format <FORMAT>     Output format for the report [default: text] [possible values: text, json]
      --explain <CATEGORY>  Print the classification rule for one change category and exit, instead of comparing snapshots. Takes a category exactly as the report prints it, e.g. `timing_changed`
  -h, --help                Print help
```

`OLD` and `NEW` each take one of three forms: an `.ir.json` snapshot, a
directory of them (the shape `.ridl/baseline/` takes), or source — a
`.typl`/`.ridl` file, a package directory, or a workspace root — compiled in
process. Anything else is refused by name rather than compiled (exit 2): an
`.ir.txtpb` or `.ir.binpb` input, because diffs and baselines read `.ir.json`
only ([ADR-0014][adr-0014] decision 5); any other file, which is neither
source nor a snapshot; and a directory holding IR artifacts but no `.ir.json`
snapshot. Omit both and pass `--explain <CATEGORY>` instead to print that
category's classification rule without comparing anything.

**It writes nothing.** The report goes to stdout; a compile error's
diagnostics go to stderr instead and no report prints.

Two identical sources:

```sh
ridl diff old.ridl old.ridl
```

```text
identical
```

An appended interaction — compatible:

```sh
ridl diff old.ridl compatible.ridl
```

```text
compatible
  [compatible] interaction_appended veh.cluster/VehicleStatus/doorClosed: (absent) -> event doorClosed
```

A signal's payload type changed — breaking:

```sh
ridl diff old.ridl breaking.ridl
```

```text
breaking
  [compatible] decl_added veh.cluster/Speed2: (absent) -> type
  [breaking] payload_changed veh.cluster/VehicleStatus/currentSpeed: Speed -> Speed2
```

The same comparison with `--format json`:

```sh
ridl diff old.ridl breaking.ridl --format json
```

```text
{
  "verdict": "breaking",
  "changes": [
    {
      "path": "veh.cluster/Speed2",
      "category": "decl_added",
      "verdict": "compatible",
      "before": null,
      "after": "type"
    },
    {
      "path": "veh.cluster/VehicleStatus/currentSpeed",
      "category": "payload_changed",
      "verdict": "breaking",
      "before": "Speed",
      "after": "Speed2"
    }
  ]
}
```

`--explain` for one category:

```sh
ridl diff --explain payload_changed
```

```text
payload_changed
A signal, event, or fixed payload type changed.
  breaking    any direction, a stream added or removed included
```

**Exit codes.** 0 for `identical` and `compatible` above. 1 for `breaking`
above — the verdict, not a tool failure. 2 when a side fails to compile:

```sh
ridl diff old.ridl broken.ridl
```

```text
error: unknown type name `NoSuchType`
  ┌─ broken.ridl:4:25
  │
4 │   signal currentSpeed : NoSuchType @10ms
  │                         ^^^^^^^^^^

```

2 when an input path does not exist:

```sh
ridl diff old.ridl does-not-exist.ridl
```

```text
error: does-not-exist.ridl: `does-not-exist.ridl` does not exist
```

2 when neither both inputs nor `--explain` are given:

```sh
ridl diff old.ridl
```

```text
error: `ridl diff` needs both an old and a new input, or `--explain <CATEGORY>`
```

and 2 for an `--explain` category that is not one of the twenty the tool
knows, which the error lists in full:

```sh
ridl diff --explain not_a_real_category
```

```text
error: unknown change category `not_a_real_category`
the categories `ridl diff` reports are:
  decl_added
  decl_removed
  interaction_appended
  interaction_inserted
  interaction_reordered
  interaction_removed
  interaction_retired
  kind_changed
  payload_changed
  return_changed
  params_changed
  timing_changed
  rpc_bound_changed
  contract_changed
  width_changed
  constraint_changed
  init_changed
  reserved_name_redeclared
  service_changed
  service_shape_appended
  service_shape_inserted
  service_shape_reordered
  service_shape_removed
  service_shape_retired
  doc_only
  visibility_changed
```

## `ridlc`

```sh
ridlc --help
```

```text
The RIDL family compiler (plumbing)

Usage: ridlc <COMMAND>

Commands:
  check  Type-check a file, package directory, or workspace
  build  Compile to the selected artifacts
  help   Print this message or the help of the given subcommand(s)

Options:
  -h, --help     Print help
  -V, --version  Print version
```

`ridlc` has no humane defaults: `PATH` is required on both subcommands, and
`build`'s `--out-dir` is required rather than defaulted to `out`. Both are
usage errors — exit 2 — courtesy of `clap`, not application code:

```sh
ridlc check
```

```text
error: the following required arguments were not provided:
  <PATH>

Usage: ridlc check <PATH>

For more information, try '--help'.
```

### `ridlc check`

```sh
ridlc check --help
```

```text
Type-check a file, package directory, or workspace

Usage: ridlc check [OPTIONS] <PATH>

Arguments:
  <PATH>  The `.typl` file, package directory, or workspace root

Options:
      --frozen  Verify remote imports against `ridl.lock` without fetching or regenerating it (CI mode, ADR-0002 §7)
  -h, --help    Print help
```

**It writes `ridl.lock` under the same condition `ridl check` does** — the
manifest declares `[imports]`, the run is non-frozen, and materialization
found no error — because `ridl check` calls this very function
(`ridlc::run_check`) directly, forwarding its own `PATH` and `--frozen`.
Otherwise it reads the workspace, renders diagnostics to stderr, and touches
no file.

**Exit codes.** 0 clean, 1 a diagnostic is an error, 2 an input/output or
usage error — identical in shape to `ridl check`, and, on the same input,
identical in rendered text:

```sh
ridlc check .
```

```text
error[TYPL-104]: range minimum 250 is greater than maximum 0
  ┌─ ./demo.ridl:3:19
  │
3 │ type Speed : km/h [250.0..0.0 step 0.5]
  │                   ^^^^^^^^^^^^^^^^^^^^^

```

run over the same directory as the `type Speed : km/h [250.0..0.0 step 0.5]`
example under [`ridl check`](#ridl-check) above — byte for byte the same
diagnostic, confirmed by diffing the two runs' stderr. A workspace `ridlc`
cannot find is the same exit 2 as `ridl check`:

```sh
ridlc check /nonexistent/ridl/workspace
```

```text
error: `/nonexistent/ridl/workspace` does not exist
```

### `ridlc build`

```sh
ridlc build --help
```

```text
Compile to the selected artifacts

Usage: ridlc build [OPTIONS] --out-dir <OUT_DIR> <PATH>

Arguments:
  <PATH>
          The `.typl` file, package directory, or workspace root

Options:
      --out-dir <OUT_DIR>
          The directory to write generated artifacts into

      --emit <EMIT>
          The artifacts to emit: `rust` (default), `c-header`, `ir-json`, `ir-text`, `ir-binary`, `typescript`

          Possible values:
          - rust:       Idiomatic Rust source, written to `<base>.rs`
          - c-header:   The extern-C header, written to `<base>.h`
          - ir-json:    The lowered IR v2 as exact-decimal JSON, written to `<base>.ir.json`
          - ir-text:    The lowered IR v2 as prototext, written to `<base>.ir.txtpb`
          - ir-binary:  The lowered IR v2 as protobuf binary, written to `<base>.ir.binpb`
          - typescript: Idiomatic TypeScript source, written to `<base>.ts`
          
          [default: rust]

      --frozen
          Verify remote imports against `ridl.lock` without fetching or regenerating it (CI mode, ADR-0002 §7)

  -h, --help
          Print help (see a summary with '-h')
```

**It writes** the same artifacts as `ridl build` — and `ridl.lock` under the
same `[imports]` condition — under the `--out-dir` you must now name
explicitly:

```sh
ridlc build . --out-dir out && find out -type f | sort
```

```text
out/cli.demo.rs
```

**Exit codes.** 0/1/2 in the same shape as `ridl build` — clean, a diagnostic
is an error, or an input/output error such as a missing `--out-dir` (usage,
exit 2) or a workspace that cannot be found (exit 2).

## How `ridl` and `ridlc` relate

`ridlc` is the plumbing: two subcommands, `check` and `build`, both with a
required `PATH` and no defaults — stable flags meant for CI and build
scripts. `ridl check` and `ridl build` call straight into `ridlc`'s own
library functions and add nothing to the compile itself; what they add is
humane defaults (`PATH` defaults to `.`, `ridl build`'s `--out-dir` defaults
to `out`) and, on `check` only, the baseline desk check described
[above](#ridl-check), which has no `ridlc` equivalent. On identical input the
two render byte-identical diagnostics, confirmed earlier on this page.

`ridl baseline`, `ridl test`, `ridl fmt`, and `ridl diff` have no `ridlc`
counterpart at all — `ridlc`'s surface is `check` and `build`, full stop, as
its own `--help` shows. Reach for `ridl` unless you are scripting the
compiler directly and want its stable, default-free flags.

## Exit codes across the toolchain

| Command | 0 | 1 | 2 |
| --- | --- | --- | --- |
| `ridl check` / `ridlc check` | clean (warnings included) | a diagnostic is an error | the workspace cannot be found, or a named `--baseline` is absent |
| `ridl build` / `ridlc build` | clean, every requested artifact written | a diagnostic is an error, nothing written | the workspace cannot be found, or (for `ridlc build`) a missing `--out-dir` |
| `ridl baseline` | clean, snapshot(s) published | a diagnostic is an error; the existing baseline is left untouched | the workspace cannot be found |
| `ridl test` | every range self-corpus and sampled `require` passed | a self-corpus failure, or a clause raised an evaluation error | the workspace fails to compile, cannot be found, or `--samples 0` |
| `ridl fmt` | nothing under `--check` would change, or the rewrite succeeded | a file under `--check` would change, or has a parse error | the path does not exist, or a directory the walk reaches is unreadable — named in the message, unlike five of the other seven, which name no path at all |
| `ridl diff` | the change is compatible, or the two sides are identical | the change is breaking | a side fails to compile, an input is missing, or neither `--explain` nor both inputs were given |

This table is this repository's own taxonomy, recorded in
[ADR-0010][adr-0010]: **0** succeeded, or the verdict is affirmative; **1** a
real answer that happens to be negative; **2** the tool could not answer at
all. [clig.dev][clig], which the ADR quotes rather than paraphrases, says
nothing about a multi-valued exit code — it treats exit status as a binary
success/failure signal. `ridl diff`'s breaking-change exit 1,
`ridl fmt --check`'s would-reformat exit 1, and `ridl test`'s evaluation-fault
exit 1 rest on the external `diff(1)`/`grep(1)` convention of a specific
non-zero code being part of the answer, not on any clig endorsement of it —
clig is silent here, not permissive.

Exit code 1 means something different in almost every row: for `ridl diff` it
is the *breaking* verdict, a statement about the change, not a failure of the
tool; for `ridl test` it is a *test failure* — a self-corpus problem or an
evaluation error, where a `suspect` finding is not enough to fail the run; for
`ridl check`, `ridl build`, and `ridl baseline` it is an ordinary *diagnostic
error*; for `ridl fmt` it is *would change* or *unparsable*. Reading a bare
`$?` of 1 without knowing which subcommand produced it says nothing on its
own.

Exit code 2 is the one genuinely shared meaning — an input the command could
not use at all, including `ridl fmt`'s missing or unreadable path since
[ADR-0010][adr-0010] closed the gap documented under
[`ridl fmt`](#ridl-fmt) above. A `clap`-level usage error — a missing
required argument, an unrecognized flag, or no subcommand at all — is also
exit 2 on both binaries, which happens to agree with the applications' own
convention rather than being one of their exit paths:

```sh
ridl check --bogus-flag
```

```text
error: unexpected argument '--bogus-flag' found

  tip: to pass '--bogus-flag' as a value, use '-- --bogus-flag'

Usage: ridl check [OPTIONS] [PATH]

For more information, try '--help'.
```

`--help` itself, on any subcommand of either binary, always exits 0 — and so
does `--version`/`-V`, covered [above](#ridl).

Six of these eight subcommands also share a lesser-known gap:
[issue driftsys/ridl#196][issue-196] records that when the *workspace root
itself* is unreadable, `ridl check`, `ridl build`, `ridl baseline`,
`ridlc check`, and `ridlc build` all report
`` error: no `ridl.toml` found at or above `<path>` `` — exit 2 is right, the
cause is wrong, confirmed directly against this build — and when a
*subdirectory nested inside* an otherwise-readable workspace is unreadable,
the same five report a bare `error: Permission denied (os error 13)`, naming
no path at all, also confirmed directly. `ridl test` reaches the same code
path and wraps it with the top-level path it was given
(`error: <path>: no ridl.toml found…` in the first case,
`error: <path>: Permission denied…` in the second — also confirmed directly),
so it names *a* path but the wrong one in the second case: the workspace
root, not the subdirectory that actually failed. `ridl fmt` is the one
subcommand that names the actual cause in both cases (see
[above](#ridl-fmt)); `ridl diff` names it only when the unreadable directory
is the argument passed to it directly, not one nested inside a readable
argument.

[adr-0007]: https://github.com/driftsys/ridl/blob/main/docs/decisions/ADR-0007-e1-execution.md
[adr-0010]: https://github.com/driftsys/ridl/blob/main/docs/decisions/ADR-0010-cli-conventions.md
[adr-0014]: https://github.com/driftsys/ridl/blob/main/docs/decisions/ADR-0014-ir-encodings.md
[issue-196]: https://github.com/driftsys/ridl/issues/196
[clig]: https://clig.dev
