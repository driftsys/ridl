# Remote units and `ridl vendor` — design

Issue: driftsys/ridl#48 (roadmap E4.6), lane D of release 0.10. Approved in
conversation with Sebastien on 2026-10-12, section by section.

## 1. Problem

ADR-0002 says that an air-gapped build is a manifest rewrite and lists a vendor
directory as an open question. Issue #48 asks for `ridl vendor`. Reading the
code at `da4a0cb9` showed that the premise is incomplete:

- Fetch, the cache (`~/.ridl/cache`), `ridl.lock` (URL to SHA-256 of the fetched
  tar) and `--frozen` are implemented (`crates/ridl-core/src/fetch.rs`,
  `cache.rs`, `lock.rs`).
- The compile path never reads a fetched package. `materialize_and_lock`
  discards the resolved paths (`crates/ridlc/src/lib.rs`, `_resolved`), and the
  resolver turns a remote import into an uncoded Info, "remote import … is not
  yet available" (`crates/ridl-sem/src/resolve.rs`). No remote type resolves.
- The wiring is already assigned to E4.6: item I3 of the debt record #135 is
  marked "CARRY → E4.6".

So #48 covers two things: the read path that makes a remote import resolve, and
`ridl vendor`, which copies the remote units into the repository for a build
without a network.

The issue gates two frozen-surface rows of release 0.10: `manifest` (it amends
ADR-0002) and `cli` (it adds a subcommand).

## 2. Delivery

One spec, one plan, two pull requests in order, both under #48:

1. **PR 1 — the read path.** Remote units load, resolve and generate code.
   ADR-0002 is amended for the read path.
2. **PR 2 — `ridl vendor`.** The vendor directory, the loader reservation, the
   command and its `--check` mode. ADR-0002 is amended for vendoring.

Shipped docs describe what is built, so each pull request documents only its own
half.

## 3. Decisions

| Id  | Decision                                                                                                                                                                                                                                                                                         |
| --- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| V-1 | **Flat, explicit dependencies.** The consumer's `[imports]` lists every remote unit the build needs, indirect ones included. A remote unit's own `[imports]` are read only to check that the consumer covers each key. When the two URLs differ, the consumer's URL is used, with no diagnostic. |
| V-2 | **A fetched artifact is a unit.** Its top level holds a `[package]` `ridl.toml` whose `name` equals the `[imports]` key.                                                                                                                                                                         |
| V-3 | **A remote unit is a read-only dependency.** Its Errors are reported and fail the command; its Warnings and Infos are dropped; the consumer's `[lints]` do not apply. It is emitted into codegen, gets no catalog in the consumer's output, and `ridl fmt` never touches it.                     |
| V-4 | **The vendor directory holds source trees**, not tar files.                                                                                                                                                                                                                                      |
| V-5 | **`ridl.lock` pins each tree.** Each entry gains `tree`, a SHA-256 over the unpacked tree, beside the existing `sha256` of the fetched tar.                                                                                                                                                      |
| V-6 | **The directory is `vendor/` at the manifest root**, beside `ridl.toml` and `ridl.lock` (the Cargo and Go convention). In workspace mode that is the workspace root.                                                                                                                             |
| V-7 | **The name `vendor` is reserved at the manifest root only.** Deeper directories named `vendor` are ordinary source packages.                                                                                                                                                                     |
| V-8 | **The command is `ridl vendor [PATH]` and `ridl vendor --check [PATH]`.**                                                                                                                                                                                                                        |
| V-9 | **`ridl.lock` and `vendor/` join the frozen `manifest` surface** from 0.10.0. The tar format (ADR-0007 decision 12) stays provisional.                                                                                                                                                           |

## 4. Design — PR 1, the read path

### 4.1 Where a remote unit is read from

For each URL of the consumer's `[imports]` (the union over the root manifest and
every member, as `materialize_and_lock` computes it today), the loader takes the
first of:

1. `<manifest root>/vendor/<key>/`, when that directory exists (from PR 2 on; PR
   1 reads only the cache);
2. the cache entry for the pin in `ridl.lock`;
3. outside `--frozen`, a fetch, which stores the entry in the cache and writes
   both hashes into `ridl.lock`.

Under `--frozen` nothing is fetched and `ridl.lock` is never rewritten. A URL
with no lock entry is MANI-103 and a pin that is not cached is MANI-104, as
today. Reading `vendor/` before the cache adds behaviour and changes none of the
`--frozen` contract that ADR-0008 decision 9 and ADR-0010 decision 5 make
plumbing-grade.

`ridl lsp` and `ridl mcp` never fetch. They read `vendor/`, then the cache; a
miss is MANI-104 with a message that names `ridl check` and `ridl vendor` as the
fix.

### 4.2 Checks on a remote unit

| Code     | Severity | Raised when                                                                                                                                    |
| -------- | -------- | ---------------------------------------------------------------------------------------------------------------------------------------------- |
| MANI-105 | Error    | the tree holds no `ridl.toml` at its top level, the manifest is a `[workspace]` manifest, or its `[package]` `name` is not the `[imports]` key |
| MANI-106 | Error    | a key of the remote unit's `[imports]` is not a key of the consumer's `[imports]`; the message names the key and the URL the remote unit gave  |
| MANI-107 | Error    | the tree's hash does not equal the `tree` of its `ridl.lock` entry                                                                             |
| MANI-108 | Error    | under `--frozen`, the `ridl.lock` entry has no `tree`                                                                                          |

The numbers are the next free ones of the distribution family (MANI-101 to
MANI-104 exist). Like those, the four are detached diagnostics that name the URL
or the unit, not a source span. Each is added to `MANI_CATALOG` and to the
family overview's section 7 table, which `diag.rs` checks against each other.

The ordinary rules apply to a remote unit unchanged: MANI-014 (two units claim
one source package), the directory-equals-package rule, and the cross-package
cycle check across the boundary between the consumer and the remote unit.

### 4.3 Resolution

ADR-0002 §5 keeps its order. Steps 2 and 3 still find a key by the longest
dot-separated prefix, so `import veh.common.types.Speed` finds the key
`veh.common`. `PackageSource::Remote(url)` now binds to the declarations of the
remote unit loaded for that URL; the uncoded Info is removed. `internal`
declarations of a remote unit are not visible to the consumer.

### 4.4 Diagnostics and output

- A remote unit's Errors are reported with the unit name and the file path in
  `vendor/` or the cache. They fail `ridl check`, `ridl build` and
  `ridl baseline` with exit 1. Its Warnings and Infos are dropped, before lint
  levels are applied.
- Codegen: remote packages are part of the model the backends receive, and are
  emitted like member packages.
- Catalogs: no catalog is written for a remote unit. The note printed by
  `crates/ridl/src/catalogs.rs` is updated to say that a remote unit's catalog
  is published by its owner.
- `ridl fmt` walks the consumer's sources only.

### 4.5 The `tree` hash

Defined once, in `ridl-core`, and used by the cache, `vendor/` and
`ridl vendor --check`:

- the input is every regular file under the unit's directory, in ascending byte
  order of its relative path written with `/` separators;
- for each file, the hash is fed the path bytes, one NUL byte, the file length
  as a u64 in little-endian order, and the file bytes;
- a symlink or any other non-regular file in the tree is an error (MANI-107,
  with a message that names the file), so no tree that holds one is ever loaded;
- the result is the SHA-256, in lowercase hex.

A fetch computes `tree` after unpacking. A lock entry written before this change
has no `tree`; the next run without `--frozen` adds it, and a run with
`--frozen` raises MANI-108.

### 4.6 Breaking change

A manifest with a remote import used to compile with an Info. It can now fail
with MANI-105 to MANI-108, and codegen plugins receive packages they did not
receive before. PR 1 carries `!` in its title and gets a heads-up issue on
driftsys/ridlc-gen-kotlin. Recorded as a delegated ruling in the PR body.

## 5. Design — PR 2, `ridl vendor`

### 5.1 The loader reservation

The workspace walk skips exactly `<manifest root>/vendor/`.

| Code     | Severity | Raised when                                                                                            |
| -------- | -------- | ------------------------------------------------------------------------------------------------------ |
| MANI-017 | Error    | in workspace mode, a `members` entry is `vendor`                                                       |
| MANI-018 | Warning  | in standalone mode, a `.ridl`, `.typl` or `.rsdl` file is under the root's `vendor/` and is not loaded |

MANI-018 is a Warning, so it needs a lint name in the catalogue (ADR-0024):
`vendor-source-not-loaded`. A directory named `vendor` deeper in a unit's tree
is an ordinary source package.

### 5.2 `ridl vendor [PATH]`

1. Find the root by ADR-0002 §4 root discovery.
2. Take the union of the root's and the members' `[imports]`.
3. Materialize each unit as PR 1 does; this can fetch and can rewrite
   `ridl.lock`.
4. Bring `vendor/` to exactly that set: write each unit to a temporary sibling
   directory and rename it into place, and remove the directory of every unit no
   longer imported. Nothing in `vendor/` changes when the run has an error.
5. Files are copied with their bytes unchanged.

| Exit | Meaning                                                                                        |
| ---- | ---------------------------------------------------------------------------------------------- |
| 0    | `vendor/` was written, or there was nothing to change                                          |
| 1    | a diagnostic error (MANI-102 to MANI-108, or a source error in a remote unit); nothing written |
| 2    | a missing or unreadable path, a bad flag, an I/O failure, or a fetch failure (MANI-101)        |

### 5.3 `ridl vendor --check [PATH]`

No network, no writes. It compares `vendor/` with the set from step 2 and with
the `tree` of each lock entry, and raises one diagnostic per unit:

| Code     | Severity | Raised when                                                        |
| -------- | -------- | ------------------------------------------------------------------ |
| MANI-109 | Error    | an imported unit has no directory in `vendor/`                     |
| MANI-110 | Error    | `vendor/` holds a directory that no `[imports]` key names          |
| MANI-107 | Error    | a vendored tree does not match its `tree` hash (as in section 4.2) |

| Exit | Meaning                             |
| ---- | ----------------------------------- |
| 0    | `vendor/` matches                   |
| 1    | drift: any of the diagnostics above |
| 2    | a missing or unreadable path        |

### 5.4 Out of scope

A `--frozen` mode for `ridl vendor`, a cache eviction or `ridl cache` command,
transitive fetching, and the diamond-conflict policy (still an ADR-0002 open
question).

## 6. Records

- **ADR-0002**, amended in place with a dated Status note in each PR:
  - PR 1: §5 states the remote-unit rules (V-1 to V-3); §7 gains the `tree`
    hash.
  - PR 2: the `[imports]` rationale in §4 and the Positive consequence "air-gap
    is a manifest rewrite" say that air-gap is `ridl vendor`, and that rewriting
    the manifest is still possible; §7 gains `vendor/`, its read order, and
    `ridl vendor --check`; the "Vendor directory" open question is marked
    resolved; the alternatives of section 7 below are recorded.
- **ADR-0029**: the Manifest row adds `ridl.lock` (PR 1) and `vendor/` (PR 2),
  and the manifest dump in `xtask/src/freeze/` gains those lines
  (`cargo xtask freeze update`). The row in `freeze/surfaces.toml` is not added
  here: a one-line pull request adds it after the last pending break.
- **ADR-0010**: decision 1 gains a dated `ridl vendor` row, with one test per
  clause (PR 2).
- **Book**: the CLI reference and the manifest chapter.
- **Family overview §7**: the new MANI codes.
- **#135**: item I3 is closed by PR 1.

## 7. Alternatives considered

- **Vendor without the read path**, or the read path as a separate issue: the
  first would freeze a command whose output nothing reads; the second leaves
  work no lane owns in front of two gate rows. I3 was already carried to E4.6.
- **Transitive fetch** of a remote unit's `[imports]`: needs the
  diamond-conflict policy that ADR-0002 defers to its own ADR, and makes
  `ridl vendor` walk a graph.
- **Ignore a remote unit's `[imports]`**: a missing dependency then fails inside
  the remote unit's source with a worse message.
- **A bare source tree, or a unit matched by prefix**: the first has no manifest
  to read the coverage check from; the second lets one key stand for several
  units, which makes coverage and vendoring ambiguous.
- **Treat a remote unit as a member**, or **check types only**: the first shows
  warnings the consumer cannot fix and lets two parties publish one catalog; the
  second turns a broken remote unit into errors in generated code.
- **Vendor the tar files**, or **both tars and trees**: exact verification
  against today's `sha256`, but a review shows a binary file, or two copies can
  drift. Sebastien chose source trees.
- **Replace `sha256` with the tree hash**, or **a separate pin file under
  `vendor/`**: the first loses verification of a download before it is unpacked;
  the second is a second pin file that can disagree with `ridl.lock`.
- **A hidden `.ridl/vendor/`**: needs no loader change, but is not the
  convention users expect. **A `vendor` reserved at every depth**, or **detected
  by content**: the first takes a package segment away from every unit; the
  second makes the meaning of a directory depend on its contents and the lock.
- **A `--frozen` mode for `ridl vendor`**, or **no `--check`**: the first adds a
  frozen flag with no stated need; the second leaves CI no way to check
  `vendor/` without a full compile.
- **Leave `ridl.lock` and `vendor/` unfrozen**, or **freeze only part of the
  lock**: either lets a 0.10.x release break a repository that vendored under
  0.10.0, which is what the freeze exists to prevent.
