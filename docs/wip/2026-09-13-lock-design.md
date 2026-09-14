# The lock file and `ridl lock`

Status: working note, 2026-09-13, design agreed in conversation with Sebastien
section by section. Nothing here is ratified. Stage L1 of lane L in
[`2026-09-13-step1-lanes-plan.md`](2026-09-13-step1-lanes-plan.md). Read after
[`2026-09-12-rsdl-rewrite-decisions.md`](2026-09-12-rsdl-rewrite-decisions.md)
D-7 and the two identity studies
([`2026-09-12-interface-id-study.md`](2026-09-12-interface-id-study.md),
[`2026-09-12-interface-id-study-2.md`](2026-09-12-interface-id-study-2.md)).

Satisfies: rsdl decisions D-7 (numbers live outside the source, at every level)
and the §4 amendment items about identity and the lock; driftsys/ridl#315 at the
interface level. Coordinated on driftsys/ridl#328.

## 1. Identity widths

Approved by Sebastien on 2026-09-13, before the rest of this note was written
(gate GW of the lanes plan, §5).

| Identity         | Width | Range and scope                                                                                                                                                                                               |
| ---------------- | ----- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| member ordinal   | `u32` | 1-based, per interface, from position in the body (ridl §11). 0 is never an ordinal: the IR writes 0 for a package-level declaration, which has none (`crates/ridl-ir/proto/ridl/ir/v2/ir.proto:84-87`).      |
| interface number | `u32` | 1-based, per catalog, which is one package (D-7). 0 is never allocated.                                                                                                                                       |
| service number   | none  | A service is identified by its published dotted name and is not in the routing key (D-7, V-X2). A transport that needs a service id gets it as that backend's own attribute (D-6), not as a runtime identity. |

The routing key is (catalog slot, interface number, member ordinal). The catalog
slot is assigned per connection and is outside this note
([`2026-09-08-topology-vocabulary.md`](2026-09-08-topology-vocabulary.md) §6
gives it as `u8`).

**Why `u32`.** Every consumer that exists already carries 32 bits: the checker's
ordinal counter, the IR's `uint32 ordinal`, the proto3 backend's
`<Interface>Ordinal` enum (int32 values, bounded by `check_field_number`,
defined at `crates/ridl-backend-proto/src/lib.rs:196` and applied to each value
at `:1106`), the FlatBuffers backend's `enum <Interface>Ordinal : uint`
(`crates/ridl-backend-flatbuffers/src/lib.rs:1223`), and the catalog descriptor
plan's schema and hash input
([`2026-09-13-catalog-descriptor-plan.md`](2026-09-13-catalog-descriptor-plan.md)
`:296`, `:306`, `:319`, and `entry.number.to_le_bytes()` at `:1327`). A narrower
width would save at most 4 bytes in a frame header, and would add a range check
at lowering, a diagnostic, and a second place where the bound can drift from the
records.

**Bounds narrower than `u32` belong to the backend that has them.** A target
whose identifier is narrower checks the bound where it projects, as the proto3
backend checks its field-number range today. For example, the SOME/IP protocol
specification gives a 16-bit method id whose top bit is the event flag, so a
SOME/IP binding accepts an ordinal below 32768. No record states that bound, and
no such backend exists yet; the check is that backend's when it is written.

**What the widths change.** Only the identity types of
[`2026-09-08-ridl-rt-design.md`](2026-09-08-ridl-rt-design.md) §2 (`:205-207`):
`Ordinal(pub u32)`, `InterfaceId(pub u32)` per catalog instead of per service,
and no `ServiceId`. Lane A's `ridl-rt` spec (stage A1) states the types, and it
supersedes those lines. The widths change nothing in the IR, in either wire
backend, or in Tasks 1, 3 and 4 of the catalog descriptor plan, which already
use 32 bits. The lock itself changes the IR, Task 3 and Task 4 for other reasons
(§3, §9, §11).

**Not decided here.**

- How a frame writes the two numbers, fixed width or variable length, is E11.1's
  (driftsys/ridl#257).
- The catalog hash. The vocabulary note §6 gives a `u64`, the catalog descriptor
  plan uses SHA-256 (`:84`, `:327`), and the `ridl-rt` note gives each interface
  its own hash (`:464-469`). D-8 and the runtime descriptors design own that
  question.

**This decision is wrong if** E11.1 finds that a frame header must fit in 8
bytes or fewer in total, or must place the numbers in an existing 16-bit field
of a transport's own header. Then the runtime types become `u16` with a range
check at lowering, and the IR and the descriptor keep `uint32`.

### Alternatives considered

- **`u16` for both, no service number.** Rejected. The IR has no 16-bit scalar,
  so it would stay `uint32` and lowering would add a range check and a new
  diagnostic. The catalog descriptor plan's Tasks 1, 3 and 4 would change (the
  schema, the stated bound, and the hash input). The saving is at most 4 bytes
  per frame header.
- **`uint32` in the IR and the descriptor, `u16` in the runtime.** Rejected for
  now, and kept as the fallback named above. Two widths, a diagnostic at
  lowering, and a descriptor reader that must range-check what it loads.
- **A service number in the runtime identity** (the `ridl-rt` note's
  `ServiceId`). Rejected by D-7: recomposing a service would change it, a
  service may list another package's interface, and the routing key does not
  contain it.

## 2. The lock file

**Decision.** One file per package, `interfaces.lock`, in the package directory
beside the `.ridl` sources — the directory ADR-0002 §1 makes the package
(`docs/decisions/ADR-0002-module-system.md:53-55`), not beside `ridl.toml`: a
subdirectory package under one manifest has no manifest of its own
(`crates/ridl-core/src/package.rs:43-44`), and the file is per package. The
format is D-7's line table (`2026-09-12-rsdl-rewrite-decisions.md:309-312`):

```text
# interfaces.lock — written by ridl lock; do not edit by hand.
next 6
CruiseControl 1
LaneAssist 2 retired
LaneKeeping 3
DoorControl 4
service:veh.hvac.cabin 5
```

- **Line 1** is a `#` header naming `ridl lock` as the writer. The parser
  ignores it and the merge driver writes the same line (§6). The `ridl.lock`
  writer already does the same (`crates/ridl-core/src/lock.rs:34-38`).
- **Line 2** is `next N`. N is greater than every number in the file, live or
  retired, and is never lowered: a retire keeps its number and a rename keeps
  its number (§4). `ridl lock` allocates from N upward and writes the new N.
- **Every later line** is one entry, `Key number`, with the word `retired` after
  the number for a retired entry. Entries are in number order. Fields are
  separated by one space and columns are not aligned: aligning would rewrite
  every line when a longer name arrives, and every such rewrite is a merge
  conflict.
- **The key** is a declared interface's name, or `service:` followed by the
  dotted name of the service whose inline shape the entry numbers (§3), as one
  token that `ridl lock`'s flags spell the same way (§5). The prefix is needed
  because a service name may have one segment and an interface may have the same
  spelling: `interface cabin` and `service cabin` check clean together in one
  package today.
- **Malformed** means: no `next` line, `next` less than or equal to an entry's
  number, one number on two entries, one live key on two entries, or a line that
  does not parse — git conflict markers included. The compiler reports RIDL-410
  and stops (§8).
- **Checked in.** Only `ridl lock` writes it (D-7 `:306-307`); `ridl fmt` never
  touches it (`:328`). prim leaves a `.lock` file alone: verified with prim
  0.9.0 on 2026-09-13, where `prim fmt --check` and `prim lint` over a directory
  holding this repository's `.editorconfig` and an `interfaces.lock` with
  trailing whitespace and no final newline report nothing. No `.primignore`
  entry is needed.
- **A package input.** The loader in `ridl-core` reads it beside the sources,
  behind the `fs` feature as `ridl.lock` is (`lock.rs:18-20`), and the compiler
  folds the numbers into the IR (D-7 `:340`; §8). A package with interfaces and
  no lock file compiles with every number provisional (§3); the first plain
  `ridl lock` creates the file. A package with no interfaces never gets one.
- **Ships with a fetched package.** A remote package's lock travels inside the
  fetched artifact; the fetch path (`crates/ridl-core/src/fetch.rs:106-127`,
  which stores an artifact in the cache) copies it and never writes it, so a
  foreign interface composed into a local service keeps the number its own
  package froze (D-7 (g) `:386-387`). `ridl.lock` could not give this, which is
  why D-7 (e) rejected it: it "does not ship with a package" (`:383-384`).

### Alternatives considered

- **`ridl.lock`.** Taken: ADR-0002 §7 (`:216-219`) and `lock.rs:1-16` make it
  the per-workspace TOML pin of remote imports; D-7 (e) rejects reusing it.
- **Beside the manifest.** In the subdirectory-package case one file would hold
  several packages' entries and need a package column, in the reader and in the
  merge driver.
- **TOML.** `serde` for free, but prim reformats TOML (`justfile:32-34`,
  `.editorconfig:21-22`), so every consumer repository would need a
  `.primignore` line, and a multi-line entry spreads one merge hunk over several
  entries.
- **Aligned columns.** Rewrites every line when a longer name arrives.

## 3. Provisional numbers and the inline shape

**Decision — the order.** A declared interface with no lock entry compiles with
a provisional number: the numbers after the highest one the lock holds (from
`next`), assigned in byte order of the name — the order a `BTreeMap<&str, _>`
gives, the one `ridl-diff` already keys by (`crates/ridl-diff/src/walk.rs:266`)
— and, for an interface and a service's inline shape spelled the same, the
interface first. The IR marks each one `provisional` (§9). A package with no
lock file numbers every interface this way from 1. A file rename or move changes
no provisional number. The catalog hash, which includes each number and its flag
(`2026-09-13-catalog-descriptor-plan.md:1324-1329`), also hashes the reduced
package, whose interfaces the plan takes in `Package::shapes()` order, which is
file-path order (`:1305-1312`); the hash is free of file order only when Task 4
takes them in number order instead (§11). A provisional number carries no
identity: `ridl diff` never matches on it (§7) and `ridl baseline` refuses to
publish it (§8). Branches never allocate (D-7 `:320-321`); only plain
`ridl lock` turns a provisional number into a frozen one (§5).

**Decision — the inline shape.** The inline form of a service (ADR-0015
decisions 12 and 14,
`docs/decisions/ADR-0015-qos-absorption-and-rpc-bounds.md:275-308`; ridl §14.5
`:1323-1332`) stays: D-7 retires the slot model of the named list, not the
inline body. The inline shape is an interface (an IR `Interface` with
`name == ""`, `crates/ridl-ir/proto/ridl/ir/v2/ir.proto:401-402`) and gets a
lock entry keyed by `service:` and the service's dotted name —
`service:veh.hvac.cabin 5` above. The number belongs to the inline interface,
not to the service: the service still has no number and is not in the routing
key (§1). A service rename is an entry rename (§4). `Package::shapes()` already
yields the inline shape under the service's name
(`crates/ridl-ir/src/lib.rs:437-470`), and the descriptor plan numbers it that
way (`:830-831`). The name alone is not a key: a one-segment service name may be
spelled like an interface (`DottedName = 'ident' ('.' 'ident')*`,
`crates/ridl-syntax/family.ungram:305-306`), so the lock writes `service:`
before it (§2), and Task 3's `Numbered` carries the same distinction (§11).

### Alternatives considered

- **Source order** — the descriptor plan's `Package::shapes()` order (`:78-80`,
  test `:898-909`): declared interfaces in file-path order
  (`crates/ridl-core/src/workspace.rs:263`), then declaration order. D-7 rejects
  file order as identity (`:381-382`), and a file rename would change the
  provisional numbers, and with them the hash, of a package whose contract had
  not changed.
- **No entry for the inline shape.** An interface with no number cannot be
  routed to; D-7 `:373` gives every interface exactly one number from its
  catalog.

## 4. Renames, removals and the protocol

**Decision — an entry's number never changes; no entry is ever removed.** A
rename rewrites the entry's name in place: `LaneKeeping 3` becomes
`LaneCentering 3`. The old name is free for a later, unrelated interface, which
gets its own number. A retire adds the word `retired` and keeps the line. This
is this note's reading of D-7 `:314` ("An entry is never changed or removed;
`next` is never lowered; a retired entry holds its number forever"), the one the
same decision's rename paragraph needs (`:333-339`, "the entry follows"); the
decisions note is not edited. The merge driver needs no history: it matches by
number (§6), so a rename on one side and a retire on the other is still a
conflict.

**Decision — protocol A, record on the branch.** Every departure from the lock
is recorded on the branch that makes it; only allocation waits for `main`.

- `ridl check` and `ridl build` — and `ridlc` (§8) — fail with RIDL-409 on every
  live lock entry that has no declaration. The compiler names
  `ridl lock <pkg> --retire Old` when the package has no declaration without an
  entry, and both `ridl lock <pkg> --rename Old=New` and `--retire Old`
  otherwise; it reads no baseline, so it cannot tell a rename from a new
  interface.
- `ridl check` adds a note naming the single `ridl lock <pkg> --rename Old=New`
  when exactly one declaration without an entry has `Old`'s baseline shape,
  however many such declarations there are. With no baseline, or with none or
  several of that shape, RIDL-409 stands alone.
- `--rename` and `--retire` edit one line each, in place, and never allocate.
- Plain `ridl lock` is the only form that allocates. The release recipe runs it
  before the version bump, or a merge queue runs it on `main`, on a clean build
  (D-7 `:327-331`).
- A declaration with no entry builds with a provisional number (§3);
  `ridl baseline` refuses it with RIDL-411 (§8).
- An LSP rename may run `ridl lock --rename` as a code action. D-7 `:333-335`
  leaves that an implementation choice; the build-side rule is the guarantee.

**The shape comparison** runs in the `ridl` facade's desk check, `desk_check`,
where RIDL-407 is emitted (`crates/ridl/src/main.rs:615`, `:642`), against the
published snapshot `.ridl/baseline/<pkg>.ir.json` (`:69-71`): ADR-0008 decision
9 keeps a workspace-local baseline read outside `ridlc`
(`docs/decisions/ADR-0008-e2-execution.md:495-501`). Two shapes are the same
when the `interactions` lists of the baseline's `Interface` for `Old` and of the
candidate's lowered `Interface` compare equal once each interaction's `doc`,
`labels` and `deprecated` are blanked on both sides. Every other field of the
`Interface` — its name, visibility, doc, labels, deprecated, number and
provisional flag — is not a member and is not compared: the baseline's `Old` is
frozen and the candidate is provisional, so a comparison of whole `Interface`
values would never match. Every interaction's name, kind, ordinal, payload,
timing, parameters, return, contracts and visibility, and every `reserved`
tombstone, must match. The IR types derive `PartialEq`, and the descriptor plan
already clones IR values and blanks their doc strings before hashing them
(`:1305-1320`). **The desk check runs with RIDL-409 present.** `run_check` runs
the desk check only when the compile produced no error
(`crates/ridl/src/main.rs:444-453`); L4 changes that condition so the desk check
also runs when every error is RIDL-409. RIDL-409 stops nothing in lowering,
since an entry with no declaration has nothing to lower, so the candidate's
`Interface` exists.

| Situation                                                                                                  | `ridl check` / `ridl build`                                                                                    | `ridl lock`                                                                                                     | `ridl baseline`                                                                                        |
| ---------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------ |
| A declaration with no entry                                                                                | provisional number, no diagnostic                                                                              | plain: allocates it, `allocated Name N`                                                                         | RIDL-411, exit 1, nothing written                                                                      |
| An entry with no declaration, no candidate                                                                 | RIDL-409, exit 1, naming `--retire Old`                                                                        | `--retire Old`: marks the line retired; plain: RIDL-409, exit 1, nothing written                                | does not run: the compile failed                                                                       |
| An entry with no declaration; declarations without an entry, exactly one of them with the baseline's shape | RIDL-409, exit 1, naming both commands; `ridl check` adds a note naming `--rename Old=New`                     | `--rename Old=New`: rewrites the line                                                                           | does not run                                                                                           |
| The unclear case: declarations without an entry, none or several of them with the baseline's shape         | RIDL-409, exit 1, naming both commands                                                                         | the author chooses `--rename` or `--retire`                                                                     | does not run                                                                                           |
| No baseline published yet, with declarations without an entry                                              | RIDL-409, exit 1, naming both commands: there is no shape to compare                                           | as the unclear case                                                                                             | does not run until the fix; then the first publication, with nothing to compare against                |
| A lock line deleted by hand                                                                                | the build cannot see it: the declaration, if kept, is provisional; if it was deleted too, the package is clean | plain: allocates a new number to a kept declaration (`next` was never lowered, so the old number is not reused) | RIDL-411 while provisional; RIDL-412 once the old number is absent from the fresh side and not retired |

**Four departures from D-7, stated.**

1. D-7 `:335-338`: "In a plain editor the build sees one entry without a
   declaration and one declaration without an entry: with the same shape as the
   baseline's, member for member, it is a rename and the entry follows;
   otherwise the build asks once". Here a same-shape rename is not automatic:
   RIDL-409 names the commands from the lock and the source alone,
   `ridl check`'s shape test only adds a note naming the single `--rename`
   command, and the author runs it. "Asks once" is a diagnostic plus two
   explicit flags, because the build runs where nobody can answer a prompt — CI
   and a merge queue — and ADR-0010 decision 1 already gives a refusal its form:
   a diagnostic error, exit 1
   (`docs/decisions/ADR-0010-cli-conventions.md:59-73`).
2. D-7 `:329-332`: "A repository whose `main` is consumed directly runs the same
   command in its merge queue; only then does the file change in parallel". Here
   it reads "only then is a number allocated in parallel": a branch edits lines
   in place for a rename or a retire, and the merge driver (§6) is needed for
   those edits in every repository.
3. D-7 `:319-326` places the refusal of "a removed interface that has no retired
   entry" at publication. Here the build refuses it first, with RIDL-409 (a live
   entry with no declaration), so the author records the retire on the branch;
   `ridl baseline`'s refusal (RIDL-412) stays for a lock line deleted by hand.
4. D-7 `:346-363` gives a rename "its own heading", "source-breaking for a
   consumer of the generated identity table". Here the rename shares one heading
   with `ServiceInterfaceRemoved`, "compatible on the wire, visible in source"
   (§7): both are wire-compatible changes a consumer sees in source, and one
   heading keeps the report's groups few.

**The reason is the merge.** Under D-7 as written, branch X removes an interface
and branch Y adds an unrelated one; each branch is clean on its own (X holds an
orphan entry with no candidate, an unambiguous delete; Y holds a provisional
declaration). `main` after both merges holds one orphan entry and one
declaration of a different shape with no entry: the unclear case, so `main`
fails to build although every branch was clean. Under inferred renames, two
unrecorded same-shape renames on two branches, or a rename followed by a shape
change, produce the same failure on `main`. Under protocol A each branch records
its own departure, and merging two clean branches never produces an unclear
`main`.

### Alternatives considered

- **B — infer renames.** A same-shape rename binds the old number in memory and
  warns; the next `ridl lock` records it. Rejected: the lock on disk disagrees
  with the IR until then, and the merge case above.
- **C — D-7 as written.** The build fails only on the unclear case. Rejected:
  the merge case above.
- **Holding the old name after a rename**, as `reserved name` holds a member
  name. Rejected: identity is the number (D-7 `:340-344`); a reused name changes
  only generated identifiers, which a rename changes in any case (`:345-363`).

## 5. `ridl lock`

**Synopsis.**

```text
ridl lock [PATH]                                  allocate every provisional interface
ridl lock [PATH] --rename OLD=NEW   (repeatable)  rewrite entries in place; allocates nothing
ridl lock [PATH] --retire NAME      (repeatable)  mark entries retired; allocates nothing
ridl lock merge BASE OURS THEIRS MARKER_SIZE      the git merge driver (§6)
```

**Scope.** `PATH` is as for the other subcommands: a package directory, a
workspace root, or a file. Over a workspace, plain `ridl lock` writes each
package's own file. With `--rename` or `--retire`, `PATH` must resolve to
exactly one package, and names are spelled exactly as in that package's lock
file. The command compiles the package first. It lives in the `ridl` facade
(`crates/ridl/src/main.rs`), beside `ridl baseline`; `ridlc` gains no `lock`
subcommand.

**Output.** One line per change to stdout — `allocated Name N`,
`renamed Old New N`, `retired Name N` — prefixed with the package path over a
workspace; nothing when nothing changes. Diagnostics go to stderr (ADR-0010
decision 2, `:119-129`).

**Exit codes**, in ADR-0010 decision 1's row form. The row is dated and each
cell has one test (#330's within-cell dating rule; §12).

| Subcommand  | 0                                                  | 1                                                                                                                                                                                                         | 2                                                                                                                                                                                                                                                    |
| ----------- | -------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `ridl lock` | the file is written, or there is nothing to change | a diagnostic error over the source, nothing written: a live entry with no declaration when plain `ridl lock` is asked to allocate (RIDL-409); a malformed lock file, conflict markers included (RIDL-410) | the path is missing or unreadable; a bad flag — `--rename` naming no live entry, or a `NEW` that is not a declaration without an entry; `--retire` naming a still-declared interface; either flag over more than one package; an I/O failure writing |

`--rename` and `--retire` run with RIDL-409 present, since they are its fix; any
other compile error still exits 1. There is no `--check` mode: `ridl baseline`
already refuses a provisional number, and the build already fails on an
unrecorded departure, so a CI gate has nothing further to ask.

### Alternatives considered

- **A `--check` mode** like `ridl fmt --check` (ADR-0010 `:84`). Not added: the
  two gates above cover CI.
- **Retiring an orphan entry from plain `ridl lock`** when it has no candidate.
  Not taken: a retire is a recorded departure (§4), and the diagnostic names the
  flag.

## 6. `ridl lock merge`

**Decision.** A three-way merge over entries, not lines. The driver parses BASE,
OURS and THEIRS, matches entries by number, writes the result to OURS and
exits 0. On a conflict it writes git conflict markers of MARKER_SIZE around only
the disagreeing entries, exits 1, and the file is malformed (RIDL-410) until an
author resolves it. An input that cannot be read is exit 2. An empty BASE is
read as `next 1` with no entries: when both sides created the file, git passes
the driver an existing empty file as `%O` (checked with a custom driver in a
throwaway repository on 2026-09-13), and only the driver accepts an empty file.
The header line is written fresh; `next` is the maximum of the three sides plus
any renumbering.

| Case (study 2 §2, `:37-63`)   | BASE       | OURS                 | THEIRS               | Result                                                                                 |
| ----------------------------- | ---------- | -------------------- | -------------------- | -------------------------------------------------------------------------------------- |
| Both add, one number          | `A 1, B 2` | `+ C 3`              | `+ D 3`              | `C 3, D 4`, `next 5`: ours keeps the number, theirs is renumbered                      |
| Rename + add                  | `A 1, B 2` | `B 2` → `Bee 2`      | `+ D 3`              | `Bee 2, D 3`: each entry changed on one side only                                      |
| Retire vs rename              | `A 1, B 2` | `B 2 retired`        | `B 2` → `Bee 2`      | conflict on entry 2, whichever source files the two edits sit in                       |
| Two different renames         | `A 1, B 2` | `B 2` → `Bee 2`      | `B 2` → `Bea 2`      | conflict on entry 2                                                                    |
| The same change on both sides | `A 1, B 2` | `B 2 retired`        | `B 2 retired`        | kept once                                                                              |
| Both retire and both re-add   | `A 1, B 2` | `B 2 retired`, `+ B` | `B 2 retired`, `+ B` | the lock merges clean; the two provisional `B` declarations are the checker's TYPL-009 |
| Retire vs add                 | `A 1, B 2` | `B 2 retired`        | `+ D 3`              | `B 2 retired, D 3`                                                                     |
| A live name on two numbers    | `A 1`      | `+ B 2`              | `+ B 3`              | conflict: one name, two numbers                                                        |

Study 2 measured the both-add, rename-plus-add, retire-vs-rename, both-retire
and retire-vs-add rows under git's `union` driver (`:42-58`) and found the base
line resurrected in the rename and retire cases, silently, with a Compatible
verdict; a driver that reads BASE resolves them (`:60-63`).

**Renumbering THEIRS is safe** under protocol A: only plain `ridl lock`
allocates, on `main` or in a merge queue (§4), so two allocations of one number
are two unpublished allocations, and the renumbered one was never in a baseline
or in generated code. This note records that assumption. A published number
changed this way surfaces as `DeclRemoved`, breaking, in the next `ridl diff`
against the baseline (§7).

**Registration.** In the repository that uses RIDL: `.gitattributes` with
`interfaces.lock merge=ridl-lock`, versioned; and per clone,
`git config merge.ridl-lock.driver "ridl lock merge %O %A %B %L"`, which git
does not version. Both lines are documented in the book's CLI reference; there
is no helper command. This repository's `./bootstrap` is unchanged: no lock file
is merged in parallel here.

### Alternatives considered

- **git's `union` driver.** Measured in study 2 §2: it resurrects a renamed or
  retired base line silently.
- **A helper command that writes the two registration lines.** Not added; two
  documented lines.

## 7. `ridl diff` by number

**Decision — matching.** `diff_interfaces` keys on the number instead of the
name (`crates/ridl-diff/src/walk.rs:266-267` today), inline shapes included. A
matched pair goes to `diff_interface` (`:313`) as today, so every
interaction-level verdict is unchanged. The classifier re-finds a container by
name on both sides today (`find_interface`,
`crates/ridl-diff/src/classify.rs:776`; study 2 `:23`); it re-finds the old side
by number, so a renamed interface's interaction changes classify as they do for
any other.

| Old side                                       | New side                         | Category                     | Verdict                                               |
| ---------------------------------------------- | -------------------------------- | ---------------------------- | ----------------------------------------------------- |
| number N, name X                               | number N, name X                 | matched; interactions diffed | as today                                              |
| number N, name X                               | number N, name Y                 | `InterfaceRenamed`           | Compatible — under the heading below                  |
| —                                              | number N, frozen                 | `DeclAdded`                  | Compatible                                            |
| —                                              | provisional                      | `DeclAdded`                  | Compatible — never matched: a rename keeps its number |
| number N                                       | absent; N in the retired list    | `InterfaceRetired`           | Compatible (D-7 `:343`)                               |
| number N                                       | absent; N not retired            | `DeclRemoved`                | Breaking (`ridl baseline` refuses it too, RIDL-412)   |
| number N, name X                               | number M, name X (a hand change) | `DeclRemoved` + `DeclAdded`  | Breaking (D-7 `:344`); no extra category              |
| `number` 0 (published before the lock existed) | any                              | matched by name              | as today — the one transition case                    |
| an interface in a service's set                | not in it                        | `ServiceInterfaceRemoved`    | Compatible — under the heading below                  |
| not in a service's set                         | in it                            | `ServiceInterfaceAdded`      | Compatible                                            |
| a named list                                   | an inline body, or the reverse   | `ServiceChanged`             | Breaking, unchanged (ADR-0015 decisions 15 and 19)    |

A published snapshot never holds a provisional number, because `ridl baseline`
refuses one (§8), and `number` 0 is never allocated (§1), so a 0 in a snapshot
means the snapshot predates the lock.

**Decision — the heading.** `Verdict` keeps its three values
(`crates/ridl-diff/src/lib.rs:41-47`). "Own heading" is D-11's sense
(`2026-09-12-rsdl-rewrite-decisions.md:447-450`): a separate group in the text
report (`render_text`, `:432`). `InterfaceRenamed` and `ServiceInterfaceRemoved`
exit 0 and are listed under one heading, "compatible on the wire, visible in
source": a rename changes the generated identity-table names in both wire
backends (D-7 `:345-363`), and a removal from a service's set stops the
`service.member` addresses of that interface resolving under that service. Each
category's `--explain` text states its own consequence. The JSON report carries
the category word and no heading field.

**Decision — ADR-0015 decision 17.** A binding keys each per-interface ordinal
space on (package, interface number), not on the interface name as
`docs/decisions/ADR-0015-qos-absorption-and-rpc-bounds.md:333-340` says today.
Appendix B's SOME/IP row, "eventgroup = interface"
(`docs/specification/ridl-language-reference.md:1860`), is keyed on the number
with it. A transport with a narrower eventgroup id checks the bound in its
backend (§1). This is the ruling D-7 `:340-344` and §5 `:507` name; with it a
same-number rename is compatible on the wire.

**Decision — a service's set.** Adding an interface to a service is compatible;
removing one is compatible too, under the heading above. The routing key does
not contain the service (§1), and V-05 promises immunity to recomposition
(`2026-09-08-topology-vocabulary.md:68-79`): a split is a removal plus an
addition, and if removal were breaking every split would fail `ridl diff`. A
consumer that loses its only provider is a wiring error that rsdl's lowering
reports (rsdl decisions D-3 `:192-194`, D-9 `:424-426`), not a package diff.

**Not widened.** #314 (a composite reorder category; PR #331) and #302 (the
FlatBuffers union-discriminant coupling) stay as they are: the carried-debt
comment above `diff_composite` (`crates/ridl-diff/src/walk.rs:196-207`) is about
struct and union tombstones, which the lock does not touch.

### Alternatives considered

- **Keep decision 17's name key.** A rename would be Breaking and the number
  would buy order independence only (study 2 `:30-35`).
- **Removal from a service's set is Breaking.** Study 2's default (`:28`) and
  ADR-0015's `ServiceShapeRemoved` today; rejected for the recomposition reason
  above.
- **An `InterfaceNumberChanged` category** for the hand change (study 2 `:26`).
  Not added: the two existing categories already classify it breaking.

## 8. Diagnostics and `ridl baseline`

**Decision.** The compiler reads `interfaces.lock` as a package input, so
`ridlc check` and `ridlc build` fail on the lock's two errors as `ridl check`
and `ridl build` do; the shape note (§4) is the `ridl` desk check's alone.
`ridl baseline` refuses two interface-level shapes inside the gate PR #330 adds
between the clean compile and the publication (gate design D-1 and D-5 on branch
`baseline-tombstone-gate`,
`docs/archive/2026-09-13-baseline-gate-design.md:50-68` and `:159-190` there):
exit 1, the staging directory removed, every published file byte-identical.

| Code     | Severity | Emitted by                        | Trigger                                                                                                                                   | Fix text                                                                                                                                                                                                                                                                                 |
| -------- | -------- | --------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| RIDL-409 | error    | the compiler (`ridlc` and `ridl`) | a live lock entry has no declaration                                                                                                      | `ridl lock <pkg> --retire Old` when the package has no declaration without an entry; otherwise both `--rename Old=New` and `--retire Old`. `ridl check`, with a baseline and exactly one declaration without an entry of `Old`'s shape, adds a note naming the single `--rename` command |
| RIDL-410 | error    | the compiler                      | `interfaces.lock` is malformed: no `next` line, conflict markers, `next` ≤ an entry, a duplicate number, a duplicate live key, a bad line | resolve the conflict or restore the file from version control, then run `ridl lock`                                                                                                                                                                                                      |
| RIDL-411 | error    | `ridl baseline`                   | an interface in the fresh IR has a provisional number                                                                                     | run `ridl lock`, then publish                                                                                                                                                                                                                                                            |
| RIDL-412 | error    | `ridl baseline`                   | an interface number in the published baseline is absent from the fresh side and not in its retired list                                   | restore the entry's line in `interfaces.lock` from version control, with `retired` if the interface is gone                                                                                                                                                                              |

- **RIDL-411** reads the fresh IR alone. **RIDL-412** reads the same `diff_sets`
  report the RIDL-408 gate walks (`untombstoned_removals`,
  `crates/ridl/src/main.rs:551-573` on the branch), filtering interface-level
  `DeclRemoved` changes against the fresh IR's retired list. In practice
  RIDL-412 is a lock line deleted by hand (§4): the build refuses an orphan
  entry, so a removed interface with a live entry never reaches publication.
- **RIDL-408** is unchanged, at the interaction level (gate D-5); **RIDL-407**
  is unchanged.
- The four numbers are fixed once #330 has merged with RIDL-408; they move if
  #330's numbering changes.
- **§17.12 and §17.13** of the ridl reference, added by #330, stay with stage
  L5. For §17.13: the gate does not descend into a package present on one side
  only, so a removed package is still not refused, and this design does not
  change that.

## 9. The retirement

**Decision.** With the slot model gone (D-7 `:369-373`):

- **RIDL-146, RIDL-147 and RIDL-148** are removed from the checker
  (`crates/ridl-sem/src/check.rs:3226`, `:3274`, `:3298`, `:3345`), from
  `crates/ridl-core/src/diag.rs:584-613`, and from the `ridl-diag-showcase`
  corpus fixtures and their snapshot. The numbers are never reused (ADR-0008
  decision 13, `:543-545`); ridl §16.4 keeps each row, marked "retired by the
  lock". RIDL-144 and RIDL-145 stay (`diag.rs:566-577`).
- **The five `ServiceShape*` categories** go with the slot walk —
  `diff_service_shapes` (`crates/ridl-diff/src/walk.rs:765`), `keyed_by_name`
  (`:1041`), `reserved_shapes` (`:1068`), `live_shapes` (`:1019`) — their
  verdict arms (`crates/ridl-diff/src/classify.rs:52-93`) and `--explain` texts
  (`:1091-1125`), the CLI reference rows (`docs/book/cli-reference.md:837-841`),
  both backends' `tests/stability.rs`, and the desk check's uses
  (`crates/ridl/src/main.rs:432-434`, `:707-719`). `ServiceInterfaceAdded` and
  `ServiceInterfaceRemoved` replace them (§7). `ServiceChanged` keeps only its
  list-form/inline-form half (`walk.rs:729-747`); the unkeyable-list fallback
  (`:779-790`) goes.
- **Grammar.** `ServiceShape = PathType | ReservedEntry`
  (`crates/ridl-syntax/family.ungram:299-301`) becomes `PathType` alone: a set
  holds no slots, so `reserved` in a service's list is a parse error. Nothing is
  published at 0.0.0; no migration. The change reaches the AST variant
  `ServiceShape::Reserved` (`crates/ridl-syntax/src/ast.rs:272`) and its match
  arm in `crates/ridl/src/main.rs:1074-1079`, and the parser fixture
  `crates/ridl-syntax/test_data/parser/ok/services.ridl:32` with its snapshot.
- **IR.** `ServiceShape.id = 1` and `ServiceShape.reserved = 12`
  (`crates/ridl-ir/proto/ridl/ir/v2/ir.proto:446-462`) become reserved field
  numbers, the treatment ADR-0015 decision 20 gave the `oneof` it replaced
  (`ADR-0015:386-395`; `ir.proto:428-435`). `Interface` gains
  `uint32 number = 7` and `bool provisional = 8` — its fields end at 6
  (`:400-415`) — set for inline shapes too. `Package` gains
  `repeated RetiredInterface retired = 5` (name, number); 5 to 15 are open
  (`:50`).

**References on origin/main** (`git grep -w`; matching lines per file, whose
sums equal the occurrence totals). Files under `docs/archive/` are historical
and are not edited. Files under `docs/wip/` describe this retirement and are not
edited either; the sweep covers the crates, the reference, the book, ADR-0015,
ADR-0016 and the decisions index.

| Identifier              | Total | Files outside `docs/archive/`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          | `docs/archive/` (not edited)                                                                                                                 |
| ----------------------- | ----- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------- |
| `RIDL-146`              | 40    | crates/ridl-core/src/diag.rs 2; crates/ridl-diff/src/classify.rs 1; crates/ridl-diff/src/classify/classify_tests.rs 2; crates/ridl-diff/src/walk.rs 2; crates/ridl-sem/src/check.rs 11; crates/ridlc/tests/corpus.rs 2; crates/ridlc/tests/corpus/ridl-diag-showcase/main/services.ridl 1; `crates/ridlc/tests/snapshots/corpus__diagnostics@ridl-diag-showcase.snap` 1; docs/decisions/ADR-0015-qos-absorption-and-rpc-bounds.md 4; docs/specification/ridl-language-reference.md 2; docs/wip/2026-09-12-interface-id-study-2.md 1; docs/wip/2026-09-12-rsdl-rewrite-decisions.md 3; docs/wip/2026-09-13-lane-b-rsdl-driver.md 1; docs/wip/2026-09-13-lane-l-lock-driver.md 1; docs/wip/2026-09-13-step1-lanes-plan.md 2                                                                                                                                              | 2026-08-03-multi-interface-services-design.md 2; 2026-08-04-e9-1-to-e9-6-execution-plan.md 1; 2026-08-05-projection-name-transform-plan.md 1 |
| `RIDL-147`              | 36    | crates/ridl-core/src/diag.rs 1; crates/ridl-diff/src/classify.rs 1; crates/ridl-diff/src/classify/classify_tests.rs 1; crates/ridl-diff/src/walk.rs 1; crates/ridl-sem/src/check.rs 11; crates/ridlc/tests/corpus.rs 2; crates/ridlc/tests/corpus/ridl-diag-showcase/NOTES 1; crates/ridlc/tests/corpus/ridl-diag-showcase/main/services.ridl 2; crates/ridlc/tests/corpus/ridl-diag-showcase/twin/twin.ridl 2; `crates/ridlc/tests/snapshots/corpus__diagnostics@ridl-diag-showcase.snap` 1; docs/decisions/ADR-0015-qos-absorption-and-rpc-bounds.md 2; docs/decisions/ADR-0016-schema-projection-and-the-name-transform.md 2; docs/specification/ridl-language-reference.md 2; docs/wip/2026-09-12-interface-id-study.md 1; docs/wip/2026-09-12-rsdl-rewrite-decisions.md 1; docs/wip/2026-09-13-lane-l-lock-driver.md 1; docs/wip/2026-09-13-step1-lanes-plan.md 1 | 2026-08-05-projection-name-transform-design.md 1; 2026-08-05-projection-name-transform-plan.md 2                                             |
| `RIDL-148`              | 35    | crates/ridl-core/src/diag.rs 2; crates/ridl-diff/src/classify.rs 1; crates/ridl-diff/src/classify/classify_tests.rs 1; crates/ridl-diff/src/walk.rs 3; crates/ridl-ir/proto/ridl/ir/v2/ir.proto 1; crates/ridl-sem/src/check.rs 6; crates/ridlc/tests/corpus.rs 2; crates/ridlc/tests/corpus/ridl-diag-showcase/main/services.ridl 2; `crates/ridlc/tests/snapshots/corpus__diagnostics@ridl-diag-showcase.snap` 1; docs/decisions/ADR-0015-qos-absorption-and-rpc-bounds.md 1; docs/decisions/README.md 1; docs/specification/ridl-language-reference.md 3; docs/wip/2026-09-12-rsdl-rewrite-decisions.md 2; docs/wip/2026-09-13-lane-b-rsdl-driver.md 1; docs/wip/2026-09-13-lane-l-lock-driver.md 1; docs/wip/2026-09-13-step1-lanes-plan.md 2                                                                                                                      | 2026-08-05-projection-name-transform-plan.md 5                                                                                               |
| `ServiceShapeAppended`  | 14    | crates/ridl-backend-flatbuffers/tests/stability.rs 1; crates/ridl-backend-proto/tests/stability.rs 1; crates/ridl-diff/src/classify.rs 2; crates/ridl-diff/src/classify/classify_tests.rs 4; crates/ridl-diff/src/lib.rs 2; crates/ridl-diff/src/walk.rs 2; docs/decisions/ADR-0015-qos-absorption-and-rpc-bounds.md 1                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 | 2026-08-03-multi-interface-services-design.md 1                                                                                              |
| `ServiceShapeInserted`  | 11    | crates/ridl-diff/src/classify.rs 2; crates/ridl-diff/src/classify/classify_tests.rs 1; crates/ridl-diff/src/lib.rs 2; crates/ridl-diff/src/walk.rs 2; crates/ridl/src/main.rs 2; docs/decisions/ADR-0015-qos-absorption-and-rpc-bounds.md 1                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            | 2026-08-03-multi-interface-services-design.md 1                                                                                              |
| `ServiceShapeReordered` | 11    | crates/ridl-diff/src/classify.rs 2; crates/ridl-diff/src/classify/classify_tests.rs 1; crates/ridl-diff/src/lib.rs 2; crates/ridl-diff/src/walk.rs 2; crates/ridl/src/main.rs 2; docs/decisions/ADR-0015-qos-absorption-and-rpc-bounds.md 1                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            | 2026-08-03-multi-interface-services-design.md 1                                                                                              |
| `ServiceShapeRemoved`   | 18    | crates/ridl-diff/src/classify.rs 2; crates/ridl-diff/src/classify/classify_tests.rs 4; crates/ridl-diff/src/lib.rs 2; crates/ridl-diff/src/walk.rs 4; crates/ridl/src/main.rs 2; docs/decisions/ADR-0015-qos-absorption-and-rpc-bounds.md 1; docs/wip/2026-09-12-interface-id-study-2.md 2                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             | 2026-08-03-multi-interface-services-design.md 1                                                                                              |
| `ServiceShapeRetired`   | 12    | crates/ridl-backend-flatbuffers/tests/stability.rs 1; crates/ridl-backend-proto/tests/stability.rs 1; crates/ridl-diff/src/classify.rs 3; crates/ridl-diff/src/classify/classify_tests.rs 1; crates/ridl-diff/src/lib.rs 2; crates/ridl-diff/src/walk.rs 1; docs/decisions/ADR-0015-qos-absorption-and-rpc-bounds.md 2                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 | 2026-08-03-multi-interface-services-design.md 1                                                                                              |

The snake-case category words are separate matches the sweep must also find:
`service_shape_appended`, `_inserted`, `_reordered`, `_removed` and `_retired`
in `crates/ridl-diff/src/lib.rs` (one each),
`docs/book/cli-reference.md:837-841`, `service_shape_removed` in
`crates/ridl-diff/src/classify.rs`, and mentions in the two studies.

## 10. ADR-0015 amendments

**Decision.** Applied in L4 with the code, in ADR-0018's form: a dated note
under `## Status` and a dated "**Amendment (date) — …**" paragraph inside each
changed decision
(`docs/decisions/ADR-0018-runtime-core-and-generated-surface.md:35-42`).
ADR-0015's own earlier amendment took a different form, a new numbered decision
(decision 24, `:430`); an amendment in place keeps each changed rule beside the
text it changes.

- **Decision 12** (`:275-292`): `ServiceShape = PathType | ReservedEntry`
  becomes `ServiceShape = PathType`; the multi-interface list itself stands.

- **Decision 15** (`:309-323`): the slot model is retired; an interface's number
  comes from its catalog's lock; the inline shape keeps its number under the
  service's name. The extraction paragraph stands: it rests on ADR-0008 decision
  4, not on the slot.
- **Decision 17** (`:333-340`): the key is (package, interface number); Appendix
  B's eventgroup row with it (§7).
- **Decision 18** (`:342-359`): RIDL-146 retired; RIDL-144 and RIDL-145 stand.
- **Decision 19** (`:361-384`): the five categories replaced by
  `ServiceInterfaceAdded` and `ServiceInterfaceRemoved`, both compatible;
  `ServiceChanged` narrowed to the form switch.
- **Decision 20** (`:386-395`): `ServiceShape.id` and `ServiceShape.reserved`
  reserved. Not listed by D-7; it changes with the set model.
- **Decision 24** (`:430-484`): RIDL-147 and RIDL-148 retired with their rules;
  a retargeted slot no longer exists.
- **"Documents to amend"** (`:531-544`) gains ridl §11, §14.5, §16.4,
  `ir.proto`, and ADR-0016's two RIDL-147 mentions (`:163`, `:303`). **Open item
  1** (`:548-552`) is marked moot: a service's list holds no tombstone any more,
  and a retired interface entry is checked against the baseline (RIDL-412).
  Decisions 10, 13, 14 and 16 are unchanged.

## 11. What changes elsewhere

Per record, with the stage that applies it. The lanes plan's §6 order for shared
files binds L4 (`2026-09-13-step1-lanes-plan.md:209-219`).

| Record                                                                         | Change                                                                                                                                                                                                                                                                                                                                                      | Stage                                                                            |
| ------------------------------------------------------------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------- |
| Catalog descriptor plan Task 3 (`:821-844`)                                    | `number_interfaces` no longer assigns: it reads `Interface.number` and `provisional` from the IR, which the compiler wrote; its test (`:898-909`) switches to name order (§3); the retired list (`:81-82`) comes from `Package.retired`; `Numbered` tells an inline shape from an interface spelled the same, as the lock's `service:` prefix does (§2, §3) | #324's executor, after G4                                                        |
| Catalog descriptor plan Task 4 (`:1287-1331`)                                  | number and flag stay in the hash and retired entries stay out (D-8 `:374-375`); the reduced package's interfaces are taken in number order, not `Package::shapes()` order (`:1305-1312`), so a file rename leaves the hash unchanged (§3)                                                                                                                   | #324's executor                                                                  |
| driftsys/ridl#326                                                              | `ridl lock` is one more subcommand for its census sites (`docs/book/cli-reference.md:57-74`, `:651`, `:1007`, `:1068`; ADR-0010 `:91-92`); noted on #326 at the end of L1                                                                                                                                                                                   | L4 for `ridl lock`; #324 for `describe`                                          |
| ridl reference §11 (`:1122-1130`)                                              | the one-level-up paragraph replaced by the lock model: numbers from the package's lock, provisional until `ridl lock`, RIDL-409 to RIDL-412                                                                                                                                                                                                                 | L4 (§6 order S2 → B1 → L4 → L5 → E14.3, `:215`)                                  |
| ridl reference §14.5                                                           | `:1301-1302` "comma-separated list" becomes a set; `:1336-1364` (ids, append-only, the tombstone example, the name-keyed spaces) rewritten; `:1375-1388` (RIDL-146, RIDL-147) removed; `:1372-1374` (RIDL-145) and `:1389-1396` (extraction) stand                                                                                                          | L4                                                                               |
| ridl reference §16.4                                                           | `:1603-1605` marked "retired by the lock"; four rows for RIDL-409 to RIDL-412 after #330's RIDL-408 row                                                                                                                                                                                                                                                     | L4                                                                               |
| ADR-0010 decision 1 (`:78-92`)                                                 | a dated `ridl lock` row (§5) and a `ridl lock merge` row, after #327's `lsp` and `mcp` rows and #330's cell edits                                                                                                                                                                                                                                           | L4 (order S1 and S2 → L4 → #324, `:219`)                                         |
| `docs/book/cli-reference.md`                                                   | a `ridl lock` section; the help transcript (`:57-74`); the category list (`:835-841`); the exit-code table (`:1012`); the subcommand counts (`:651`, `:1007`, `:1068`); the merge-driver registration lines (§6)                                                                                                                                            | L4 (order S1 and S2 → L4 → #324, `:218`)                                         |
| ADR-0016 (`:163`, `:303`)                                                      | the two RIDL-147 mentions reworded to cite the retirement                                                                                                                                                                                                                                                                                                   | L4, with ADR-0015                                                                |
| `docs/decisions/README.md:79`                                                  | ADR-0015's summary line ("five diagnostics (RIDL-144 to RIDL-148), and five diff categories") gains the amendment                                                                                                                                                                                                                                           | L4                                                                               |
| Runtime descriptors design D-4 (`:117-120`), D-8 (`:235`), D-9 (`:240-242`)    | unaffected: they already state the frozen number from the lock, the provisional flag, and the retired entries as name and number                                                                                                                                                                                                                            | none                                                                             |
| `crates/ridl-core/src/diag.rs`, `crates/ridl-diff/`, `crates/ridl/src/main.rs` | the four codes and the three removals; the categories and the number-keyed walk; `ridl lock`, `ridl lock merge`, the desk check's shape note and `run_check`'s condition for running it (§4), the two `ridl baseline` refusals                                                                                                                              | L4 (orders `:210-212`: S1 and S2 → L4 → C3 → B3; S2 → L4; S1 and S2 → L4 → #324) |
| rsdl decisions note D-7                                                        | not edited: §4's reading of `:314` and the four departures are recorded here; the family general form §6.3 item is D-7's §4 (`:470-472`) and outside this lane                                                                                                                                                                                              | none                                                                             |

## 12. Testing

What pins each behaviour in L4; the L2 plan expands it.

- **The lock reader** (`crates/ridl-core`): a round trip; each malformed shape
  of §2 reports RIDL-410; provisional numbers follow byte order from `next`; a
  file rename leaves every number unchanged; `interface cabin` and
  `service cabin` in one package get two entries, `cabin` and `service:cabin`.
- **The shape note**: `ridl check` with RIDL-409 as its only error still runs
  the desk check and names the single `--rename` command for a same-shape
  candidate, including when the baseline's `Old` is frozen and the candidate is
  provisional.
- **The protocol** (`crates/ridl/tests/`): one test per row of §4's table, each
  asserting the exit code, the diagnostic and the named command, and that the
  lock file is byte-identical when nothing may be written.
- **`ridl lock`**: one test per cell of §5's row, run against the built binary
  as ADR-0010's cells are; `ridlc check` reports RIDL-409 and RIDL-410 too.
- **`ridl lock merge`**: one fixture per row of §6's table, run in both
  directions (OURS and THEIRS swapped), asserting the output bytes and the exit
  code; a conflict fixture asserts the markers sit around the disagreeing
  entries only and that the result reports RIDL-410 until resolved.
- **`ridl diff`** (`crates/ridl-diff/src/tests.rs`,
  `crates/ridl/tests/diff_cli.rs`): one test per row of §7's table; the text
  report's heading; the `--explain` coverage test, which fails until every new
  category has its `explain`, `category_word` and `classify` arms.
- **Diagnostics**: RIDL-409 and RIDL-410 fixtures in the `ridl-diag-showcase`
  corpus (`crates/ridlc/tests/corpus/`) with the snapshot; the RIDL-146 to
  RIDL-148 fixtures removed from it; RIDL-411 and RIDL-412 in #330's
  `crates/ridl/tests/baseline_gate.rs`, each asserting the published files stay
  byte-identical.
- **The book**: a lock file shown in the book is a plain fence, which the
  examples harness does not compile.

## 13. Traceability

**Satisfies.** rsdl decisions D-7
(`2026-09-12-rsdl-rewrite-decisions.md:286-399`) and its §4 items on ADR-0010,
the ridl reference and ADR-0017/ADR-0019 (`:481-497`); driftsys/ridl#315 at the
interface level (RIDL-411, RIDL-412).

**Related.**

- The two identity studies: `2026-09-12-interface-id-study.md` and
  `2026-09-12-interface-id-study-2.md` (the merge measurements, §2; the diff
  table, §1).
- ADR-0015 decisions 12, 15, 17, 18, 19, 20 and 24 (§10); ADR-0010 decisions 1
  and 2 (§5).
- The baseline gate: PR #330's design D-5 and RIDL-408 (§8); §17.12 and §17.13
  to stage L5.
- The catalog descriptor plan (#324) Tasks 3 and 4, and #326 (§11).
- Issues #314 (PR #331) and #302: not widened. The carried-debt comment above
  `diff_composite` (`crates/ridl-diff/src/walk.rs:196-207`) is about struct and
  union tombstones and the FlatBuffers union-discriminant coupling; the lock
  changes interface identity and never touches a composite body.
- The runtime descriptors design (`2026-09-13-runtime-descriptors-design.md`)
  D-4, D-8 and D-9, which consume the frozen number, the flag and the retired
  list (§11).
