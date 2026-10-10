# Evolving an interface

An interface changes after it ships: a signal gains a sibling, a bound is
tightened, a payload type is replaced. This chapter shows how to change one so
that the toolchain can tell a consumer built against the old version from one
built against the new version, and what it records about each change. It covers
the three commands that carry the workflow — [`ridl lock`](cli-reference.md#ridl-lock),
[`ridl baseline`](cli-reference.md#ridl-baseline) and
[`ridl diff`](cli-reference.md#ridl-diff) — a table of the verdicts, and the
list of compatible catalogs that the toolchain writes into the catalog
descriptor.

## What identifies a version

Two identities are involved, and they answer different questions.

- **The interface number** says _which interface_. It is allocated once, by
  `ridl lock`, and recorded in the unit's `interfaces.lock`. A rename keeps it
  and a retirement keeps it forever, so a number is never given to a different
  interface. The ridl reference,
  [section 11](reference/ridl.md#11-interaction-identity-and-evolution), is the
  rule.
- **The catalog hash** says _which version of the unit_. It is a SHA-256 hash
  over the unit's interfaces, their numbers, and every declaration they reach.
  [The catalog descriptor](catalog-descriptor.md#the-catalog-and-the-face)
  describes it.

A unit is the package or workspace member that owns one `ridl.toml`, and it has
one `interfaces.lock`. The workflow below keeps the numbers stable and records
each published version's hash.

## The workflow

The workflow has four steps. Steps 1 and 2 run once for a new unit; steps 3 and
4 repeat for every release.

1. **Record the numbers.** `ridl lock` gives every interface that has no entry
   the next free number and writes `interfaces.lock`. Until it runs, an
   interface has a provisional number, which is no identity: `ridl baseline`
   refuses to publish a provisional number (RIDL-411).
2. **Publish a baseline.** `ridl baseline` writes one `<package>.ir.json`
   snapshot per source package, and one `<unit>.catalogs` file per unit that has
   an interface, under `.ridl/baseline/` at the workspace root. Commit the
   directory: it is the record of what was released.
3. **Change the source, then compare.** `ridl diff` compares the published
   baseline with the working tree and prints a verdict. `ridl check` also reads
   `.ridl/baseline/` when it exists, and warns (RIDL-407) on a change that moves
   an interaction's position.
4. **Publish again** when the release is ready. `ridl baseline` replaces the
   snapshots and extends the `<unit>.catalogs` file.

For a unit with one package, the first publication is:

```sh
ridl lock . && ridl baseline && find .ridl/baseline -type f | sort
```

```text
allocated Cabin 1
.ridl/baseline/demo.cabin.catalogs
.ridl/baseline/demo.cabin.ir.json
```

### Comparing with `ridl diff`

`ridl diff` takes the old and the new side as snapshots, a directory of
snapshots, or source. To compare the published baseline with the working tree,
pass the baseline directory and the tree:

```sh
ridl diff .ridl/baseline .
```

The report lists each change with its category and its verdict, then prints the
overall verdict as its first line. The exit code is 0 for `identical` and
`compatible`, 1 for `breaking`, and 2 when a side does not compile or an input
is missing. A pipeline that must not ship a breaking change can run the command
and stop on exit 1. Appending an event to the interface:

```text
compatible
  [compatible] interaction_appended demo.cabin/Cabin/doorLocked: (absent) -> event doorLocked
```

`ridl diff --explain <category>` prints the rule for one category, and the
[CLI reference](cli-reference.md#ridl-diff) lists every category.

### Renaming and retiring

A rename and a removal are not edits of the source alone, because the number
records the identity.

- To rename an interface, change its name in the source and run
  `ridl lock . --rename Old=New`. The number is kept, and `ridl diff` reports
  `interface_renamed`, which is compatible on the wire.
- To remove an interface, delete it from the source and run
  `ridl lock . --retire Name`. The entry keeps its number with the word
  `retired`, and `ridl diff` reports `interface_retired`. Without the
  retirement, the build fails with RIDL-409 and `ridl baseline` publishes
  nothing.
- To remove an interaction, replace it with a `reserved` line that holds its
  position. A bare deletion is breaking, and `ridl baseline` refuses it
  (RIDL-408).

## The verdicts

`ridl diff` gives each change one of two verdicts, compatible or breaking, and
the report's verdict is the worst of them. `identical` means there is no change
at all. A verdict says whether a consumer built against the old version still
works with the new one. It does not say whether the catalog hash changed, and
the two are separate questions: the hash identifies an exact version. A
compatible change that reaches the hash makes a new version, and a change that
leaves the hash unchanged makes none.

No runtime in this workspace performs the check at `attach`. The last column
states what the frame specification
([section 6.1](reference/frame.md#61-attach-and-attached)) specifies for an
older consumer, built against the version before the change.

| Kind of change                                                                     | Verdict    | The catalog hash | The list in the descriptor and in the history file                                          | At `attach`, by §6.1                                       |
| ---------------------------------------------------------------------------------- | ---------- | ---------------- | ------------------------------------------------------------------------------------------- | ---------------------------------------------------------- |
| None                                                                               | identical  | unchanged        | unchanged                                                                                   | accepted: the hash is the provider's own                   |
| A doc comment, a label or `deprecated` changes                                     | compatible | unchanged        | unchanged                                                                                   | accepted: the hash is the provider's own                   |
| A declaration is added that no interface reaches                                   | compatible | unchanged        | unchanged                                                                                   | accepted: the hash is the provider's own                   |
| A compatible change to an interface, or to a declaration that an interface reaches | compatible | changes          | the earlier hashes stay, and the previous one joins                                         | accepted: the earlier hash is in the list                  |
| A breaking change to an interface, or to a declaration that an interface reaches   | breaking   | changes          | the descriptor's list is empty; the history file restarts with the new hash alone           | refused with `catalog_mismatch`: the earlier hash is not listed |
| A breaking change to a declaration of the unit that no interface reaches           | breaking   | unchanged        | unchanged: the hash did not change                                                          | accepted: the hash is the provider's own                   |

The "compatible" and "breaking" rows cover the categories that `ridl diff`
prints. The rules are in the
[CLI reference](cli-reference.md#ridl-diff), and
`ridl diff --explain <category>` prints the rule for one. These are the
common cases:

- **Compatible**: an interaction appended after every slot that was used before
  (`interaction_appended`); an interaction retired to a `reserved` line in its
  own slot (`interaction_retired`); an interface added, renamed with its number
  kept, or retired (`decl_added`, `interface_renamed`, `interface_retired`); a
  scalar constraint widened (`constraint_changed`); a signal or event bound
  tightened (`timing_changed`); an interface joining or leaving a service's list
  (`service_interface_added`, `service_interface_removed`).
- **Compatible, and the hash changes**: an enum body reordered with no number
  changed (`enum_reordered`), and a provisional interface number frozen by
  `ridl lock` (`interface_frozen`).
- **Breaking**: an interaction inserted, removed without a tombstone or
  reordered (`interaction_inserted`, `interaction_removed`,
  `interaction_reordered`); a payload, a parameter list or a return shape
  changed (`payload_changed`, `params_changed`, `return_changed`); a scalar
  constraint narrowed or a width changed (`constraint_changed`,
  `width_changed`); a signal bound loosened or removed; a struct field, a union
  arm or an enum value removed, an enum value renumbered, or an interface
  number changed (`decl_removed`, `member_reordered`,
  `interface_number_changed`).

An interface that joins the list of a service with a named interface list, or
leaves it, is not part of the hash input, because the list is not hashed. Such
a change is compatible, and it leaves the hash as it was.

The verdict that decides the chain is the verdict **of one unit**, not the
report's overall verdict. It is the worst verdict among the changes that
concern the unit: a change in one of its source packages, or in a declaration
that an interface of the unit reaches, in the unit or in another one. A breaking
change in a declaration of another unit that the unit does not reach leaves the
unit's chain as it was, although `ridl diff` still exits 1 for the report as a
whole. A breaking change in a declaration of the unit's own packages that no
interface reaches is breaking for the unit, although the hash stays as it was. The chain is then
carried, because the catalog did not change and every listed hash stays
compatible.

## The compatible catalogs

The catalog descriptor of a unit carries, beside the unit's own `hash`, a
`compatible` list: the hashes of earlier versions of the unit that the
toolchain judged compatible with this one, newest first. The current hash is
never in it. The frame specification uses the list at `attach`: a provider may
accept an `attach` that names one of those hashes
([section 6.1](reference/frame.md#61-attach-and-attached) states the rule).
**No runtime in this repository accepts an `attach` on the strength of the
list.** The toolchain writes it, and a runtime that reads it is outside this
repository. The generated Rust face compares its own `CATALOG` with its own
port only, and does not read the list. This section describes what the toolchain
writes.

### The chain file

`ridl baseline` keeps the history in `.ridl/baseline/<unit>.catalogs`, one file
per unit that has an interface, or a service with an inline body. The file is text: a first line that starts with
`#`, then one catalog hash per line as 64 lowercase hexadecimal characters,
newest first.

```text
# catalogs of this unit's published baselines, newest first, back to the last breaking change
a5973898ecf39cce4b375906dfdbcafc795108832593b70dd49f3e75a90ca841
5ca96801a44963f58aca2dd05a73736c51fa199a2070cdf0a773360294a071db
```

The first hash is the catalog hash of the baseline being published, the same
hash that `ridl build --emit catalog` computes. The hashes after it are copied
from the replaced file when the change is compatible or the hash is unchanged,
and a breaking change that changes the hash restarts the file with the new hash
alone; the rules are in the
[`ridl baseline` reference](cli-reference.md#ridl-baseline).

For a unit with a published snapshot, a file that cannot be read, or that has a
line that is neither a comment nor a hash, or that lists a hash twice, stops
`ridl baseline` with exit 2. `ridl build` also exits 2 when it writes a catalog
descriptor or generates code and the workspace compiles without an error
diagnostic. The message
names the file and the line, and says to repair the file or remove it to start
the unit's chain again.

### The build writes the list

`ridl build` reads the baseline when it writes a catalog descriptor or generates
code. For each unit that has an interface and a package in the baseline, it
lists the hashes of the unit's `<unit>.catalogs` file, less the current catalog
hash, when the unit's verdict from the baseline to the working tree is
compatible or identical, or when the current hash equals the first hash of the
file. It writes the list into the descriptor, which
`ridl describe` prints as `compatible`, and into the `catalog.compatible` field
of the codegen model that a plugin receives. The recorded file is not changed by
a build; only `ridl baseline` changes it.

The list is empty in these cases:

- the workspace has no `.ridl/baseline/` directory, or the directory holds no
  `.ir.json` snapshot;
- the baseline holds no package of the unit;
- the unit has no `<unit>.catalogs` file;
- the unit's verdict from the baseline to the working tree is breaking and the
  current hash differs from the first hash of the file.

A build with an error diagnostic writes no list. A build whose only errors are
`RSDL-7xx` still writes its artifacts, with an empty list.

`ridlc build` reads no baseline, so the descriptors and the codegen requests it
writes carry an empty list. `ridl build` is the command that fills it.

So a working tree that adds a signal to a published interface is built with a
list that holds the published hash. After the next `ridl baseline`, the file
holds the new hash first and the published hash after it, and a later build of
the same tree lists the published hash and no other.

### An example

The unit `demo.cabin` declares an interface `Cabin` with a signal
`currentSpeed` and an event `doorChanged`. After `ridl lock` and the first
`ridl baseline`, the file holds the one hash of that baseline:

```text
# catalogs of this unit's published baselines, newest first, back to the last breaking change
5ca96801a44963f58aca2dd05a73736c51fa199a2070cdf0a773360294a071db
```

An event `doorLocked`, with a doc comment, is appended to `Cabin`. The change is compatible, and a
build of the tree writes a descriptor whose `compatible` list holds one hash,
the published `5ca96801...`. Publishing again puts the new hash first:

```sh
ridl diff .ridl/baseline . ; echo "exit: $?"
ridl baseline && cat .ridl/baseline/demo.cabin.catalogs
```

```text
compatible
  [compatible] interaction_appended demo.cabin/Cabin/doorLocked: (absent) -> event doorLocked
exit: 0
# catalogs of this unit's published baselines, newest first, back to the last breaking change
a5973898ecf39cce4b375906dfdbcafc795108832593b70dd49f3e75a90ca841
5ca96801a44963f58aca2dd05a73736c51fa199a2070cdf0a773360294a071db
```

Next, `currentSpeed` changes its payload type from `Speed` to `DoorState`. The
change is breaking, so a build of the tree writes an empty `compatible` list.
`ridl baseline` does not refuse the publication, because it enforces the
numbers and the tombstones, not the verdict. It restarts the chain:

```sh
ridl diff .ridl/baseline . ; echo "exit: $?"
ridl baseline && cat .ridl/baseline/demo.cabin.catalogs
```

```text
breaking
  [breaking] payload_changed demo.cabin/Cabin/currentSpeed: Speed -> DoorState
exit: 1
# catalogs of this unit's published baselines, newest first, back to the last breaking change
4a83686ad26f656d469694a346b1eebb0f851becf05c95e173a4a29bc1496269
```

A consumer built against either of the two earlier versions is not on the new
list, so the rule of [section 6.1](reference/frame.md#61-attach-and-attached)
would not accept it.

## Further reading

- [`ridl lock`](cli-reference.md#ridl-lock), [`ridl baseline`](cli-reference.md#ridl-baseline)
  and [`ridl diff`](cli-reference.md#ridl-diff) in the CLI reference, with the
  exit codes and the publication gate.
- [The catalog descriptor](catalog-descriptor.md) — the file that carries the
  hash and the `compatible` list.
- [The ridl reference, section 11](reference/ridl.md#11-interaction-identity-and-evolution)
  — the rule for numbers, retirement and tombstones.
- [The catalog descriptor design record][design] — how the hash and the chain
  are computed.

[design]: https://github.com/driftsys/ridl/blob/main/docs/design/catalog-descriptor.md
