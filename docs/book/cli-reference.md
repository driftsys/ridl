# CLI reference

Two command-line binaries ship from this repository: **`ridl`**, the porcelain
facade, and **`ridlc`**, the plumbing compiler underneath it. `ridl` hosts the
language server as the [`ridl lsp`](#ridl-lsp) subcommand and the Model
Context Protocol server as [`ridl mcp`](#ridl-mcp), both over stdio (see
[What is built](introduction.md#what-is-built)). The VS Code extension spawns
`ridl lsp`. Build both with:

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
  fmt       Reformat `.typl`, `.ridl` and `.rsdl` files in place (defaults to the current directory)
  diff      Compare two IR snapshots or source trees and classify the change: exit 0 compatible or identical, 1 breaking, 2 error
  lock      Allocate a number to every interface that has none and write each package's `interfaces.lock`; with `--rename` or `--retire`, rewrite one package's entries in place instead. Exit 0 when the file is written or nothing changes, 1 on a diagnostic error, 2 on a bad flag or a path or I/O failure. `ridl lock merge` is the git merge driver for the file
  lsp       Run the language server over stdio: exit 0 on a clean shutdown, 2 on a transport error. Editors spawn this; it takes no flag of its own
  mcp       Run the MCP server over stdio for an agent host: exit 0 on a clean shutdown, 2 on a transport error. It takes no flag of its own
  help      Print this message or the help of the given subcommand(s)

Options:
  -h, --help     Print help
  -V, --version  Print version
```

Running `ridl` with no subcommand at all prints this same text to **stderr**
and exits 2; `ridl --help` prints it to **stdout** and exits 0 — the two
routes carry identical text but are not interchangeable in a script that
checks the exit code or reads the right stream.

`ridl lsp` and `ridl mcp` are the two stdio servers this binary hosts — the
language server an editor drives, and the Model Context Protocol server an
agent drives. Neither takes an argument or a flag; each is documented in its
own section below, [`ridl lsp`](#ridl-lsp) and [`ridl mcp`](#ridl-mcp), and
each exits 0 when the client shuts it down and 2 when the transport ends
before the handshake or otherwise fails.

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
      --baseline <DIR|FILE>  Compare the checked workspace against a published baseline — a directory of `.ir.json` snapshots or one snapshot file — and warn (RIDL-407) on every interaction whose ordinal moved and every struct field or union arm that was reordered. Without the flag, `.ridl/baseline/` at the workspace root is used when it exists
      --format <FORMAT>      Output format for the report: text renders to stderr (the default); json goes to stdout instead — see the CLI reference (docs/book/cli-reference.md) for its schema [default: text] [possible values: text, json]
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

The same run with `--format json` prints the JSON diagnostic contract, also
produced by `ridl_core::diag::to_json`, as a bare array:

```sh
ridl check --format json
```

```text
[
  {
    "code": "TYPL-104",
    "severity": "error",
    "message": "range minimum 250 is greater than maximum 0",
    "span": {
      "path": "./demo.ridl",
      "start": {
        "line": 3,
        "column": 19
      },
      "end": {
        "line": 3,
        "column": 40
      }
    },
    "labels": [],
    "fixes": []
  }
]
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

A baseline directory that holds IR artifacts but no `.ir.json` snapshot —
named with `--baseline` or auto-discovered at `.ridl/baseline/` — is also
exit 2, with a message naming an artifact it found: a baseline stays
`.ir.json` ([ADR-0014][adr-0014] decision 5), so such a directory is a
baseline in a refused encoding, not the silently skipped "no baseline
published yet" state. An *empty* directory is still that silent state when
auto-discovered; named explicitly with `--baseline`, it is a different
refusal (below). A snapshot-named entry inside the directory that cannot be
stat'ed — a symlink to a file that is gone — is exit 2 under either, naming
the entry, because skipping it would read the directory as one snapshot
short.

So is a directory whose `.ir.json` snapshots sit one level *below* it rather
than inside it — `--baseline .ridl` where `.ridl/baseline` was meant. The
message names the subdirectory it found and gives the path to pass instead.
Snapshots are read from one directory and never from the directories below
it, so a directory of this shape is a path aimed one level too high, not a
layout to descend into.

An explicit `--baseline` that yields no snapshot at all — an empty directory,
or one whose snapshots sit two or more levels down rather than one — is the
same exit 2: the message names the directory, says to point `--baseline` at
the directory that holds the snapshots (`ridl baseline` publishes them to
`.ridl/baseline/` at the workspace root), and only then offers
`ridl baseline --out` for publishing a first baseline there. Auto-discovery
asserts nothing about a baseline being present, so a `.ridl/baseline/`
directory found this way, empty or absent, keeps the silent skip the two
paragraphs above do not touch.

**The baseline desk check.** With `.ridl/baseline/` present at the workspace
root — written by [`ridl baseline`](#ridl-baseline) — `ridl check` compares
the workspace against it and warns (RIDL-407) on every interaction whose
declaration order moved, and every struct field or union arm that was
reordered, without moving the exit code. Reordering two events in a published
interface:

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
clean. The desk check runs only after a compile with no error diagnostic other
than RIDL-409 — a live `interfaces.lock` entry with no declaration, which
leaves nothing out of the IR the desk check compares. A workspace with any
other error draws no RIDL-407 warning in addition to that error: it exits 1,
exactly as it would with no baseline present. A workspace whose only errors
are RIDL-409 still exits 1, and the desk check runs over it: when exactly one
declaration without an entry has the published shape of the orphan entry's
interface, the desk check adds a label to that RIDL-409 naming the
[`ridl lock --rename`](#ridl-lock) command to run.

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
touched. The workspace's interface numbers must be recorded first: a
provisional number is refused (RIDL-411, under the publication gate below), so
a package with interfaces runs plain [`ridl lock`](#ridl-lock) before its
first publication. A two-member workspace with no `[imports]`, its locks
written, publishes two files and no `ridl.lock`:

```sh
ridl baseline && find .ridl/baseline -type f | sort
```

```text
.ridl/baseline/veh.cluster.ir.json
.ridl/baseline/veh.common.ir.json
```

**The publication gate.** Before replacing the published snapshots,
`ridl baseline` compares the snapshot it is about to publish against the one
already there, and refuses to replace it when, in an interface body both
declare, the replacement breaks the tombstone rule of ridl §11 — RIDL-408,
exit 1, the published snapshots left as they were (the compile has already
run, so `ridl.lock` is written as on any other run). The refusal covers four
shapes: an interaction is gone from the source with no `reserved` line at
all; the source retires it with a `reserved` line, but at an ordinal other
than the one the interaction held; the baseline already retired the
interaction with a `reserved` line and the source has dropped that line; or
the source declares a live interaction under a name the baseline retires.
The interface level of the gate is the lock's: an interface whose number is
provisional — a declaration with no entry in the package's `interfaces.lock`
— is refused (RIDL-411, exit 1, nothing published), on a first publication as
on a replacement, until plain `ridl lock` records the number; and a number the
published baseline holds that the fresh snapshot neither carries nor retires
is refused too (RIDL-412) — a lock line deleted by hand, since a live entry
with no declaration already fails the build with RIDL-409. A whole service
removed from the source is reported by `ridl diff` as breaking but is not
refused here (ridl §17.14), and a named-form service's list is a set the gate
does not read. Deleting `doorClosed` outright, with `doorOpened` and `doorLocked`
still declared:

```sh
ridl baseline
```

```text
error[RIDL-408]: `doorClosed` is gone from the source but the baseline being replaced still declares it in `VehicleStatus` (ordinal 2). Publishing would free its ordinal for a later interaction to reuse, with nothing left to record that it was ever taken. Retire it in place with `reserved doorClosed`.
  ┌─ ./demo.ridl:3:11
  │
3 │ interface VehicleStatus {
  │           ^^^^^^^^^^^^^

```

A first publication — the output directory absent, or present and holding no
`.ir.json` snapshot yet — has no published snapshot to compare against, so
RIDL-408 and RIDL-412 never refuse it. This is the same empty-directory shape
[`ridl check --baseline` refuses](#ridl-check) when it is named explicitly:
`ridl check --baseline` refuses it because the user named the directory as a
baseline to compare against, while `ridl baseline --out` treats it as a first
publication because it is the directory being written. RIDL-411 still refuses
a first publication that holds a provisional interface number, because
RIDL-411 reads the fresh snapshot alone.

**Exit codes.** 0 on a clean publish. 1 when a diagnostic is an error, or when
the publication gate above refuses the replacement — in both cases the
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

2 when the workspace itself cannot be found, the same as `ridl check`, when
the output directory cannot be read or written, when a published `.ir.json`
snapshot fails to parse, when two published snapshots declare one package, or
when an entry of the output directory named like a snapshot cannot be
stat'ed:

```sh
ridl baseline /nonexistent/ridl/workspace
```

```text
error: `/nonexistent/ridl/workspace` does not exist
```

A published snapshot that cannot be parsed is left untouched rather than
silently overwritten: replacing it would destroy whatever ordinal record it
held without any report. `ridl` cannot tell a damaged file from a snapshot
that a toolchain with a different IR schema wrote — the reader rejects an
unknown field ([ADR-0014][adr-0014] decision 14) and a snapshot carries no
schema marker — so the message names a remedy for each cause, and neither
remedy discards the record unread:

```sh
ridl baseline
```

```text
error: ./.ridl/baseline/veh.cluster.ir.json: the IR snapshot is not valid IR v2 JSON: expected value at line 1 column 1; the file is left as it is, because a record that cannot be read cannot be shown safe to replace. If the file is damaged, restore it from version control or resolve the merge conflict left in it. If a toolchain with a different IR schema wrote it, check the source against it with that toolchain (`ridl check --baseline`), then remove the file and run `ridl baseline` with this one
```

The same parse error on a `ridl diff` input, or on a file or directory named
with `ridl check --baseline`, is reported without that remedy: only the
published baseline that `ridl baseline` is about to replace is the record the
remedy protects.

Two published snapshots that declare one package are two records of the same
ordinals, and the gate has no rule for choosing between them, so it refuses
rather than compare against either. Both files are left as they are; the
message names the package and both files. Here `zz-copy.ir.json` is a copy of
the published snapshot left beside it:

```sh
ridl baseline
```

```text
error: two published snapshots declare the package `veh.cluster`: `./.ridl/baseline/veh.cluster.ir.json` and `./.ridl/baseline/zz-copy.ir.json`; both files are left as they are, because the gate cannot tell which one is the published record. Remove the copy that is not the published record (restore the directory from version control if unsure), then run `ridl baseline` again
```

This refusal is `ridl baseline`'s alone: `ridl check --baseline` and
`ridl diff` read such a directory as a snapshot set, matching packages by
name.

An entry of the output directory named like a snapshot whose metadata cannot
be read — a symlink to a file that is gone — is also exit 2, naming the entry.
Skipping it would read the directory as one snapshot short, which is a first
publication to the gate, and the publication would then replace the entry
with a snapshot that had dropped the ordinal record. The entry is left as it
is. `ridl check` reports the same entry the same way when it finds it under
`.ridl/baseline/` by auto-discovery, and so does `ridl diff` over a directory
side:

```sh
ridl baseline
```

```text
error: cannot read the snapshot ./.ridl/baseline/veh.cluster.ir.json: No such file or directory (os error 2)
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
          - rust:          Idiomatic Rust source, written to `<base>.rs`
          - ir-json:       The lowered IR v2 as exact-decimal JSON, written to `<base>.ir.json`, and the lowered system to `<pkg.Name>.system.json`
          - ir-text:       The lowered IR v2 as prototext, written to `<base>.ir.txtpb`, and the lowered system to `<pkg.Name>.system.txtpb`
          - ir-binary:     The lowered IR v2 as protobuf binary, written to `<base>.ir.binpb`, and the lowered system to `<pkg.Name>.system.binpb`
          - typescript:    Idiomatic TypeScript source, written to `<base>.ts`
          - proto:         The proto3 schema, written to `<base>.proto`
          - flatbuffers:   The FlatBuffers schema, written to `<base>.fbs`
          - codegen-model: The lowered codegen model (`ridl.codegen.v1`) as canonical protobuf JSON, written to `<base>.codegen.json`
          
          [default: rust]

      --plugin <LANGUAGE[=PATH]>
          A codegen plugin to run beside the emits, once per package: `<LANGUAGE>` runs `ridlc-gen-<LANGUAGE>` from PATH, `<LANGUAGE>=<PATH>` runs the executable at PATH. Repeatable

      --plugin-timeout <SECONDS>
          Seconds a plugin may run per package before it is killed
          
          [default: 60]

      --frozen
          Verify remote imports against `ridl.lock` without fetching or regenerating it (CI mode, ADR-0002 §7)

  -h, --help
          Print help (see a summary with '-h')
```

`--emit` accepts a comma-separated list, so one invocation can write several
targets. `--frozen` is the same flag as on `ridl check`: it is
[`ridlc build --frozen`](#ridlc-build), documented word for word since
[ADR-0010][adr-0010].

**`rust` is a language backend**, and it writes the whole generated surface of
a package in one file: the domain types (a struct, an enum, an enum set, a
union and a named scalar, each with its typl constraints enforced at
construction), the FlatBuffers codec — `encode`, `verify`,
`decode` and a `MAX_SIZE` bound — the interaction descriptors, and the
interaction face. Each interface gets the parts its own interactions need: a
`Client` for the consumer side when it carries any interaction at all; a
`Publisher` for the producer side when it carries a signal or an event; and a
`Provider` trait the application implements plus a `dispatch` function that
settles the claims waiting on a port, when it carries a command or a query. So
a signal-only interface gets a `Client` and a `Publisher` and nothing to settle
with, a command-only interface gets a `Client`, a `Provider` and a `dispatch`
and no `Publisher`, and an interface carrying only `fixed` declarations gets no
face module at all. Beside the per-package files it writes a `lib.rs` crate
root and a `Cargo.toml` naming `ridl-rt` with the encoding's feature.

**Two things it may leave out, each with a note in the source it writes.** A
type whose size the compiler cannot bound — one that reaches itself, or a
stream — carries no codec, and gets a `__RIDL_FB_NO_CODEC_<NAME>` constant
whose documentation names the member and the reason. An interface the face
cannot carry — a call that does not take exactly one named parameter, a query
whose reply is not a named type, or a contract clause outside the form the
translator accepts — is skipped along with its descriptors, and gets a
`__RIDL_NO_FACE_<NAME>` constant naming the interface, the reason and the
story that removes the limit. Neither is an error: the rest of the package is
emitted, and the build succeeds.

The face names the `ridl-rt` port traits and nothing else: the crate carries no
runtime and opens no socket, so an application supplies the ports. The one
runtime in this workspace is `ridl-loopback`, which runs in process.
`examples/cabin/` is a worked example — a schema, and a consumer program
against the crate built from it — and `just demo` generates that crate and runs
the program, which prints one round trip per interaction kind.

There is **no flag for the payload encoding**. A package emits the FlatBuffers
codec, which is the only one built; the emitted `pub type Wire` names it in one
line. A `--wire` flag belongs to the story that adds the second codec, and
[ADR-0010][adr-0010] binds its spelling then rather than now.

**`proto` is a wire backend** (ADR-0013 decision 2): it emits the typl
surface — structs, enums, enum sets and unions, projected to proto3 messages
and enums, with named-scalar constraints carried as comments — plus the
interaction identity table, one enum per interface giving each signal, event,
command, query and fixed its ordinal. It emits no `service` block, no call
face, and no value store; store and dispatcher generation is roadmap stories
E11.2 and E11.4 (ADR-0018 decision 16), not this emit.

**`flatbuffers` is the second wire backend**, with the same two tiers and the
same ceiling. Its projection rules differ from proto3's where the targets
differ — a union is isolated in a wrapper table, every struct is a `table`, a
map is a vector of generated entry tables with no `(key)`, and enum values are
not prefixed because FlatBuffers scopes them inside their enum — and are
recorded in ADR-0019.

**`codegen-model` is not a backend at all**: it writes the lowered codegen
model — one package, resolved over the scope the build read, with the scalar
classes, the pinned name transforms, the resolved type references, the typl
init and closure rules, the tombstones in their slots, the FlatBuffers
projection and the interaction facts computed once — as canonical protobuf
JSON, in the schema `ridl.codegen.v1`. It is the payload a codegen request
carries (ADR-0020 decisions 8 and 9), written byte for byte as the request
would carry it, so a plugin's fixture is a file `ridlc` wrote. No in-tree
backend reads it yet: the four backends above still read the IR, and a drift
test in each of them asserts that what the backend derives and what the model
states are the same fact.

**`--plugin` runs a codegen plugin beside the emits** — a backend that is an
executable rather than part of `ridl`, over the contract
`generate(CodegenRequest) → CodegenResponse` ([ADR-0020][adr-0020] decisions 9
and 10; the as-built record is
[`docs/design/codegen-plugins.md`](https://github.com/driftsys/ridl/blob/main/docs/design/codegen-plugins.md)).
The value is a language, or a language and a path:

- `--plugin kotlin` runs `ridlc-gen-kotlin`, found by walking `PATH` in order
  and taking the first directory that holds a file of that name (on Windows,
  `ridlc-gen-kotlin.exe` is also tried). No directory holding it is an error
  naming `ridlc-gen-kotlin`, and — like a compile error — a build that writes
  nothing.
- `--plugin kotlin=/opt/gen/bin/kt-gen` runs the executable at that path and
  skips the lookup. `ridl` still reports it as `ridlc-gen-kotlin`, with the
  path beside the name.

The flag repeats, once per plugin. Each plugin runs once per package the code
emits are written for — `ridl.std` included, under the same rule as above — and
receives on its standard input one `ridl.codegen.v1.CodegenRequest` in canonical
protobuf JSON: `schema` (`"ridl.codegen.v1"`) and `toolchain` (this `ridl`'s
version) first, then `model`, byte for byte the package's `codegen-model`
artifact one indentation level deeper, `options` (empty from this command
line; no flag sets one yet) and `artifactBase`, the `<base>` of the emit list
above. It answers on its standard output with one `CodegenResponse`: `files`,
each a `path` relative to `--out-dir` with a `text` or `binary` content, and
`diagnostics`, each a `severity` and a `message`. `ridl` writes the files; the
plugin never touches the filesystem, so `--out-dir` means for a plugin exactly
what it means for `rust`. A path that is absolute, has a `..` or `.` or empty
component, or contains `\` is refused, and no file of that response is written.
A diagnostic is reported prefixed with the plugin's name, at its severity; an
error-severity one is a build that exits 1 with none of that response's files
written, as an in-tree backend's failure is. A non-zero exit, a response that
does not parse, a plugin that cannot be started, and a plugin still running
after `--plugin-timeout` seconds (60 by default; the plugin is killed) are each
one error naming the plugin, and the build exits 1. The plugin's standard error
is passed through.

The one plugin in this repository is `ridlc-gen-model`, built for the test
suite and not installed by any release: it is `--emit codegen-model` as a
process, and the test that runs it through this path proves the host, not a
language. The Rust backend and the other three still run in process only; the
Rust backend's own plugin follows its port onto the model (roadmap story
E4.5b).

**It writes** one file per package per `--emit` target, under `--out-dir`
(`out` by default), and — exactly like [`ridl check`](#ridl-check) —
`ridl.lock` at the workspace root when the manifest declares `[imports]`,
non-frozen. `<base>` in the `--emit` list above is the package name when
`PATH` is a package directory or a workspace root, and the input file's stem
in single-file mode.

When a package names a type from `ridl.std`, the standard package is written
beside your own as one more file per `--emit` target — `ridl.std.rs`,
`ridl.std.ts`, `ridl.std.codegen.json` — because generated code refers to
standard types by package path and does not compile without it, and the model
is lowered over the same scope. The three IR targets —
`ir-json`, `ir-text`, `ir-binary` — get no such file: a direct IR dump
records the packages the workspace declares, and `ridl.std` ships with the
compiler rather than with the workspace ([ADR-0007][adr-0007] decision 15).

When the workspace declares a `system` (rsdl reference §3.1), each of the three
IR targets also writes the lowered system — the closure, and every deployment
with its placements, links, routes and surface set (rsdl reference §13) — as
one more file named after the system's qualified name:
`veh.topology.Vehicle.system.json` for `system Vehicle` in package
`veh.topology` under `--emit ir-json`, `.system.txtpb` and `.system.binpb` for
the other two.

Building a workspace of two packages that name no standard type, with no
`[imports]`:

```sh
ridl build --out-dir out && find out -type f | sort
```

```text
out/Cargo.toml
out/lib.rs
out/veh.cluster.rs
out/veh.common.rs
```

The `Cargo.toml` and the `lib.rs` are the crate root: the per-package files
are flat, and the crate root is what gives them the module paths the generated
code refers to each other by. Single-file mode writes neither, because one
file is not a crate.

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
package that fails to compile. An RSDL-7xx error is the one exception: it
blocks only its own deployment (rsdl reference §13), so the build writes every
artifact, leaves that deployment out of the lowered system, and still exits 1:

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
Reformat `.typl`, `.ridl` and `.rsdl` files in place (defaults to the current directory)

Usage: ridl fmt [OPTIONS] [PATH]

Arguments:
  [PATH]  [default: .]

Options:
      --check  Do not write; exit 1 if any file would change
  -h, --help   Print help
```

**It writes** every `.typl`/`.ridl`/`.rsdl` file under `PATH` back to itself in
canonical form, unless `--check` is given or the file fails to parse. An `.rsdl`
file takes the file layout (the header, one blank line between declarations).
The formatter has no layout rules for the rsdl declarations, so it keeps each
one as written, as it does a ridl `interface` or `service`.

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

Of the nine subcommands that take a path, [ADR-0010][adr-0010] decision 6
found `ridl fmt` is the only one that reliably names the actual unreadable
path this way in every case it was tested against. `ridl check`, `ridl build`,
`ridl baseline`, `ridl lock`, `ridlc check`, and `ridlc build` still exit 2 on
the same inputs, but with the wrong cause or none: an unreadable *workspace root*
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
directory of them (the shape `.ridl/baseline/` takes), or source — a file, a
package directory, or a workspace root — compiled in process. Only IR
artifacts are recognised by name, and three inputs are refused rather than
compiled (exit 2): an `.ir.txtpb` or `.ir.binpb` file, because diffs and
baselines read `.ir.json` only ([ADR-0014][adr-0014] decision 5); a directory
that holds IR artifacts but no `.ir.json` snapshot and no source — no
`ridl.toml` and no `.typl`/`.ridl` file — which is a snapshot directory in a
refused encoding, not a source tree; and a directory with no source whose
`.ir.json` snapshots sit one level below it, which is a path aimed one level
too high. A directory is also exit 2, naming the entry, when an entry inside
it named like a snapshot cannot be stat'ed — a symlink to a file that is gone
— because skipping the entry would compare against a set one snapshot short;
this is checked before the directory is read as a source tree, so a source
directory holding such an entry is refused too. Every other input reaches
the source compiler. Omit both and pass `--explain <CATEGORY>` instead to print that
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

A named-form service's list is a set of interfaces: an interface joining it
is `service_interface_added`, one leaving it `service_interface_removed`, a
reorder no change, and both are compatible, because an interface's number
comes from its package's `interfaces.lock` and the routing key does not
contain the service. A removal is still visible in source — the
`service.member` addresses of that interface stop resolving under the service
— so the text report lists it under a heading of its own, printed once as a
line ending in a colon, after every change that has no heading. The JSON
report carries the category word and no heading field. With `interface K`
added and `J` dropped from `service veh.cluster.dash : I, J`:

```sh
ridl diff dash-old.ridl dash-new.ridl
```

```text
compatible
  [compatible] decl_added veh.cluster/K: (absent) -> interface
compatible on the wire, visible in source:
  [compatible] service_interface_removed veh.cluster/veh.cluster.dash/J: J -> (removed)
```

An interface is matched by its number from the package's `interfaces.lock`,
not by its name — a declared `interface` and a service's inline shape alike. A
rename that keeps its number, recorded with `ridl lock <pkg> --rename Old=New`,
is `interface_renamed`: compatible on the wire, because the number is the
routing identity, and visible in source, because the generated identity-table
names change, so it shares the heading above; the path carries the new name,
and a change inside the renamed interface is reported under the new name too.
A number gone from the new side is `interface_retired` when that side's lock
retires it, and `decl_removed`, breaking, otherwise. A declaration with no lock
entry carries a provisional number, which is no identity: it is always
`decl_added`, and it is never matched to an old interface, so a rename the lock
does not record is `decl_removed` plus `decl_added`. Two sides with no lock
file — two bare source trees, or a snapshot published before the lock existed —
are matched by name. With `J` renamed to `Jay` on its number, the lock beside
each file recording it, and `Jay` no longer listed in
`service veh.cluster.dash : I, J`:

```sh
ridl diff old/dash.ridl new/dash.ridl
```

```text
compatible
compatible on the wire, visible in source:
  [compatible] interface_renamed veh.cluster/Jay: J -> Jay
  [compatible] service_interface_removed veh.cluster/veh.cluster.dash/J: J -> (removed)
```

The breaking comparison above with `--format json`:

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

**At the system.** When both sides are source whose workspace declares a
`system`, `ridl diff` also lists the changes to the lowered system that are not
contract changes, under two headings and with no verdict (rsdl reference §14):
**placement changed** for a deployment or a machine added or removed, a machine
made `external`, or an instance moved to another machine; **composition
changed** for a component added to or removed from the system, an `offers` or
`requires` line added or removed, `instances` changed, or a component made
`external`. The verdict and the exit code are the contracts' alone.

Moving `Panel` from machine `Front` to machine `Rear` in deployment `Desk`:

```sh
ridl diff old new
```

```text
identical
placement changed
  Desk/Rear: (absent) -> machine
  Desk/Front: machine -> (removed)
  Desk/veh.demo.Panel.Unit: Front -> Rear
```

With `--format json` the headings are the keys `placement_changed` and
`composition_changed`, each a list of `{"path", "before", "after"}`, present
only when they hold a change. An `.ir.json` snapshot, or a directory of them,
carries no system — `ridl baseline` publishes package snapshots only — so a
diff against `.ridl/baseline/` lists no system change.

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

and 2 for an `--explain` category that is not one of the categories the tool
knows, which the error lists in full:

```sh
ridl diff --explain not_a_real_category
```

```text
error: unknown change category `not_a_real_category`
the categories `ridl diff` reports are:
  decl_added
  decl_removed
  interface_renamed
  interface_retired
  member_reordered
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
  service_interface_added
  service_interface_removed
  doc_only
  visibility_changed
```

### `ridl lock`

```sh
ridl lock --help
```

```text
Allocate a number to every interface that has none and write each package's `interfaces.lock`; with `--rename` or `--retire`, rewrite one package's entries in place instead. Exit 0 when the file is written or nothing changes, 1 on a diagnostic error, 2 on a bad flag or a path or I/O failure. `ridl lock merge` is the git merge driver for the file

Usage: ridl lock [OPTIONS] [PATH]
       ridl lock <COMMAND>

Commands:
  merge  The git merge driver for `interfaces.lock`: a three-way merge over entries matched by number, written to OURS. Exit 0 when the merge is clean, 1 when entries disagree (they are left between conflict markers of MARKER_SIZE, and the file is RIDL-410 until resolved), 2 when an input cannot be read or does not parse (OURS is left as it was). Register it with `.gitattributes` and `git config` as the CLI reference documents
  help   Print this message or the help of the given subcommand(s)

Arguments:
  [PATH]  A package directory, a workspace root, or a file. A directory named `merge` is spelled `./merge`, since the bare word is the subcommand [default: .]

Options:
      --rename <OLD=NEW>  Rewrite the live entry OLD to hold the key NEW, keeping its number (repeatable). NEW must be a declaration without an entry
      --retire <NAME>     Mark the live entry NAME retired, keeping its line and its number (repeatable). NAME must no longer be declared
  -h, --help              Print help
```

**It writes** `interfaces.lock` in the package directory, beside the `.ridl`
sources — the line table that gives every interface of the package its
number (ridl §11): a `#` header, `next N`, then one entry per interface,
`Name N`, with the word `retired` after the number when the interface is
gone. A service's inline shape is an interface too and is keyed `service:`
followed by the service's dotted name. Only `ridl lock` writes the file: the
compiler reads it beside the sources, and `ridl fmt` never touches it.

Plain `ridl lock` is the only form that allocates. Every declared interface
whose name has no live entry gets the next free number, in byte order of the
name, and the file is written; a declaration that already has its entry is
left as it is. Over a package holding `interface Zone` and `interface Cabin`
and no lock file yet:

```sh
ridl lock . ; echo "exit: $?"
```

```text
allocated Cabin 1
allocated Zone 2
exit: 0
```

```text
# interfaces.lock — written by ridl lock; do not edit by hand.
next 3
Cabin 1
Zone 2
```

Run again with nothing to allocate, it prints nothing, writes nothing and
exits 0. Over a workspace it writes each package's own file, and each output
line is prefixed with the package directory relative to `PATH` and a colon:
`hvac: allocated Cabin 1`. Until `ridl lock` has run, a declaration with no
entry compiles with a provisional number, which carries no identity.

The reverse case — a live entry whose interface is gone from the source — is
RIDL-409 from the compiler, and plain `ridl lock` refuses to allocate over it:
nothing is written, exit 1. The two flags are that diagnostic's fix, and they
run with RIDL-409 present:

- `--rename OLD=NEW` rewrites the entry `OLD` to hold the key `NEW` in place,
  keeping its number, when `NEW` is a declaration without an entry — the same
  interface under a new name. Printed as `renamed Old New N`.
- `--retire NAME` marks the entry retired, keeping its line and its number,
  when nothing declares `NAME` any more. Printed as `retired Name N`.

Both are repeatable, neither allocates, and `PATH` must resolve to exactly one
package. A rename keeps the number because the number, not the name, is the
interface's wire identity; the old name is then free for a later, unrelated
interface. Starting from the file above with `interface Zone` renamed to
`interface Lane` in the source:

```sh
ridl check . ; echo "exit: $?"
```

```text
error[RIDL-409]: `Zone` is a live entry of `interfaces.lock` with no declaration in the package: run `ridl lock . --rename Zone=New` when a declaration without an entry, `New`, is this interface under a new name, or `ridl lock . --retire Zone` when the interface is gone
  ┌─ ./interfaces.lock:4:1
  │
4 │ Zone 2
  │ ^^^^^^

exit: 1
```

```sh
ridl lock . --rename Zone=Lane ; echo "exit: $?"
```

```text
renamed Zone Lane 2
exit: 0
```

**The merge driver.** Two branches that each edited `interfaces.lock` merge
through `ridl lock merge`, a three-way merge over entries matched by number
rather than over lines. git runs it when the repository registers it: one
versioned line in `.gitattributes`, and one `git config` line per clone,
which git does not version.

```text
interfaces.lock merge=ridl-lock
```

```sh
git config merge.ridl-lock.driver "ridl lock merge %O %A %B %L"
```

```sh
ridl lock merge --help
```

```text
The git merge driver for `interfaces.lock`: a three-way merge over entries matched by number, written to OURS. Exit 0 when the merge is clean, 1 when entries disagree (they are left between conflict markers of MARKER_SIZE, and the file is RIDL-410 until resolved), 2 when an input cannot be read or does not parse (OURS is left as it was). Register it with `.gitattributes` and `git config` as the CLI reference documents

Usage: ridl lock merge <BASE> <OURS> <THEIRS> <MARKER_SIZE>

Arguments:
  <BASE>         The common ancestor's file (`%O`); an empty file reads as `next 1`
  <OURS>         The current branch's file (`%A`); the result is written here
  <THEIRS>       The other branch's file (`%B`)
  <MARKER_SIZE>  The length of a conflict marker line (`%L`, 7 by default)

Options:
  -h, --help  Print help
```

At each number the driver applies git's own three-way rule: a change on one
side is taken, the same change on both sides is kept once, and two different
changes to one entry — a retire against a rename, or two renames — are left
between conflict markers, with every other entry written plain. One rule is
the driver's own: when both sides allocated one number to two different
interfaces, ours keeps the number and theirs is renumbered to the next free
one, which is safe because a branch never allocates. A live name on two
numbers — each side allocated the same interface on its own number — is a
conflict too. `next` is the maximum of the three sides plus one per
renumbered entry, so it is never lowered. With `A 1, B 2` as the base, ours
retiring `B` and theirs renaming it to `Bee`:

```text
# interfaces.lock — written by ridl lock; do not edit by hand.
next 3
A 1
<<<<<<< ours
B 2 retired
=======
Bee 2
>>>>>>> theirs
```

The file is malformed (RIDL-410) until an author keeps one side and deletes
the markers, and `ridl check` refuses it until then. git's `union` driver is
not a substitute: it discards the base, and resurrects a renamed or retired
entry silently. An empty BASE — what git passes when both branches created
the file — reads as `next 1` with no entries. A package directory named
`merge` is spelled `./merge`: the bare word is the subcommand.

**Exit codes.** 0 when the file is written or there is nothing to change. 1
on a diagnostic error over the source, with nothing written: a live entry with
no declaration when plain `ridl lock` is asked to allocate (RIDL-409), a
malformed lock file — a git conflict left in it included — (RIDL-410), or any
other compile error, in any package of the workspace. 2 when the path is
missing or unreadable, on a bad flag — `--rename` naming no live entry or a
`NEW` that is not a declaration without an entry, `--retire` naming an
interface that is still declared, either flag over more than one package — or
on an I/O failure writing the file. Every cell is confirmed against the built
binary by `crates/ridl/tests/lock_cli.rs`. `ridl lock merge` exits 0 when the
three sides merge clean and OURS is written; 1 when entries disagree — OURS is
written with the conflict markers and is malformed until resolved; 2 when an
input cannot be read or does not parse (OURS is left as it was), when
`MARKER_SIZE` is not a number from 1 up, or on an I/O failure writing OURS.
Every cell is confirmed by `crates/ridl/tests/lock_merge.rs`, which also
registers the driver in a temporary repository and merges two branches
through git.

### `ridl lsp`

```sh
ridl lsp --help
```

```text
Run the language server over stdio: exit 0 on a clean shutdown, 2 on a transport error. Editors spawn this; it takes no flag of its own

Usage: ridl lsp

Options:
  -h, --help  Print help
```

`ridl lsp` hosts the language server: behavior lives in `crates/ridl-lsp`, and
this subcommand only wires the stdio transport. An editor spawns it and speaks
the Language Server Protocol over its stdin and stdout.

**Exit codes.** 0 on a clean shutdown — the client sends `shutdown` then
`exit`. 2 when the transport ends before the `initialize` handshake, or fails
for any other reason. There is no exit 1: `ridl lsp` answers no question that
can come back negative. Both outcomes are confirmed directly against the built
binary by `crates/ridl/tests/servers.rs`.

### `ridl mcp`

```sh
ridl mcp --help
```

```text
Run the MCP server over stdio for an agent host: exit 0 on a clean shutdown, 2 on a transport error. It takes no flag of its own

Usage: ridl mcp

Options:
  -h, --help  Print help
```

`ridl mcp` serves the Model Context Protocol over stdio with one tool,
`ridl_check`: behavior lives in `crates/ridl-mcp`, and this subcommand only
builds the Tokio runtime the server needs — the only asynchronous code in this
workspace — and wires the stdio transport. An agent host spawns it and speaks
MCP over its stdin and stdout. The tool's input and output are documented in
`crates/ridl-mcp/README.md`.

**Exit codes.** 0 on a clean shutdown: the host closes the server's stdin,
which is how a stdio host ends an MCP session. 2 when the Tokio runtime fails
to build, when the transport ends before the `initialize` handshake, or when a
task the SDK runs for the session fails after it. A read failure on stdin
after the handshake is not distinguishable from the host closing stdin — the
SDK logs it and ends the session the same way — so it exits 0. There is no
exit 1: `ridl mcp` answers no question that can come back negative. The clean
shutdown and the transport ending before the handshake are confirmed directly
against the built binary by `crates/ridl/tests/servers.rs`; a session-task
failure cannot be provoked through the server, so its mapping is tested on its
own in `crates/ridl-mcp`.

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
          The artifacts to emit: `rust` (default), `ir-json`, `ir-text`, `ir-binary`, `typescript`, `proto`, `flatbuffers`, `codegen-model`

          Possible values:
          - rust:          Idiomatic Rust source, written to `<base>.rs`
          - ir-json:       The lowered IR v2 as exact-decimal JSON, written to `<base>.ir.json`, and the lowered system to `<pkg.Name>.system.json`
          - ir-text:       The lowered IR v2 as prototext, written to `<base>.ir.txtpb`, and the lowered system to `<pkg.Name>.system.txtpb`
          - ir-binary:     The lowered IR v2 as protobuf binary, written to `<base>.ir.binpb`, and the lowered system to `<pkg.Name>.system.binpb`
          - typescript:    Idiomatic TypeScript source, written to `<base>.ts`
          - proto:         The proto3 schema, written to `<base>.proto`
          - flatbuffers:   The FlatBuffers schema, written to `<base>.fbs`
          - codegen-model: The lowered codegen model (`ridl.codegen.v1`) as canonical protobuf JSON, written to `<base>.codegen.json`
          
          [default: rust]

      --plugin <LANGUAGE[=PATH]>
          A codegen plugin to run beside the emits, once per package: `<LANGUAGE>` runs `ridlc-gen-<LANGUAGE>` from PATH, `<LANGUAGE>=<PATH>` runs the executable at PATH. Repeatable

      --plugin-timeout <SECONDS>
          Seconds a plugin may run per package before it is killed
          
          [default: 60]

      --frozen
          Verify remote imports against `ridl.lock` without fetching or regenerating it (CI mode, ADR-0002 §7)

  -h, --help
          Print help (see a summary with '-h')
```

**It writes** the same artifacts as `ridl build` — and `ridl.lock` under the
same `[imports]` condition — under the `--out-dir` you must now name
explicitly. `--plugin` and `--plugin-timeout` are the same two flags as on
[`ridl build`](#ridl-build), spelled and documented identically:

```sh
ridlc build . --out-dir out && find out -type f | sort
```

```text
out/Cargo.toml
out/cli.demo.rs
out/lib.rs
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

`ridl baseline`, `ridl test`, `ridl fmt`, `ridl diff`, and `ridl lock` have no
`ridlc` counterpart at all — `ridlc`'s surface is `check` and `build`, full stop, as
its own `--help` shows. Reach for `ridl` unless you are scripting the
compiler directly and want its stable, default-free flags.

## Exit codes across the toolchain

| Command | 0 | 1 | 2 |
| --- | --- | --- | --- |
| `ridl check` / `ridlc check` | clean (warnings included) | a diagnostic is an error | the workspace cannot be found, or — for `ridl check` only — a `--baseline` problem: absent, wrongly encoded (not `.ir.json`), unreadable, a snapshot-named entry inside it that cannot be stat'ed, its snapshots nested one level too deep, empty when named explicitly, or a snapshot that fails to parse |
| `ridl build` / `ridlc build` | clean, every requested artifact written | a diagnostic is an error, nothing written — except for an RSDL-7xx error, which leaves only its deployment out of the lowered system | the workspace cannot be found, or (for `ridlc build`) a missing `--out-dir` |
| `ridl baseline` | clean, snapshot(s) published | a diagnostic is an error, or the publication gate refuses: the replacement under the tombstone rule (RIDL-408), a provisional interface number (RIDL-411), or a published number the fresh snapshot neither carries nor retires (RIDL-412); the existing baseline is left untouched | the workspace cannot be found, the output directory cannot be read or written, a published `.ir.json` snapshot fails to parse, two published snapshots declare one package, or a snapshot-named entry in the output directory cannot be stat'ed |
| `ridl test` | every range self-corpus and sampled `require` passed | a self-corpus failure, or a clause raised an evaluation error | the workspace fails to compile, cannot be found, or `--samples 0` |
| `ridl fmt` | nothing under `--check` would change, or the rewrite succeeded | a file under `--check` would change, or has a parse error | the path does not exist, or a directory the walk reaches is unreadable — named in the message, unlike six of the other eight, which name no path at all |
| `ridl diff` | the change is compatible, or the two sides are identical | the change is breaking | a side fails to compile, an input is missing, or neither `--explain` nor both inputs were given |
| `ridl lock` | the file is written, or there is nothing to change | a diagnostic error over the source, nothing written: a live entry with no declaration under plain `ridl lock` (RIDL-409), a malformed lock file (RIDL-410), or any other compile error | the path is missing or unreadable; a bad flag — `--rename` naming no live entry or a `NEW` that is not a declaration without an entry, `--retire` naming a still-declared interface, either flag over more than one package; an I/O failure writing |
| `ridl lock merge` | the three sides merge clean, and the result is written to OURS | entries disagree: OURS is written with conflict markers around only the disagreeing entries, and is malformed (RIDL-410) until resolved | an input cannot be read or does not parse (OURS is left as it was), `MARKER_SIZE` is not a number from 1 up, or an I/O failure writing OURS |

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

Seven of the nine subcommands that take a path also share a lesser-known gap:
[issue driftsys/ridl#196][issue-196] records that when the *workspace root
itself* is unreadable, `ridl check`, `ridl build`, `ridl baseline`,
`ridl lock`, `ridlc check`, and `ridlc build` all report
`` error: no `ridl.toml` found at or above `<path>` `` — exit 2 is right, the
cause is wrong, confirmed directly against this build — and when a
*subdirectory nested inside* an otherwise-readable workspace is unreadable,
the same six report a bare `error: Permission denied (os error 13)`, naming
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
[adr-0020]: https://github.com/driftsys/ridl/blob/main/docs/decisions/ADR-0020-third-encoding-runtime-layering-and-plugin-system.md
[issue-196]: https://github.com/driftsys/ridl/issues/196
[clig]: https://clig.dev
