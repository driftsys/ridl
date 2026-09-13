# The baseline tombstone gate, the explicit-baseline read, and the composite reorder category

Status: working note, 2026-09-13, design approved in conversation the same day.
Nothing here is ratified. It disposes of three filed defects against shipped
crates: driftsys/ridl#315, #235 (with #234) and #314.

Read after
[`2026-09-12-rsdl-rewrite-decisions.md`](2026-09-12-rsdl-rewrite-decisions.md)
D-7, which is where #315 and #314 were found and which fixes what the interface
level of the same rule will become. ADR-0010 decision 1 (the exit-code taxonomy)
and decision 6 (the fail-closed change to `ridl fmt`) bind the command surface;
the ridl language reference §11 states the tombstone rule this note enforces.

Why it exists: `ridl baseline` publishes the record that `ridl check` and
`ridl diff` compare against, and it publishes unconditionally. An interaction
removed with no `reserved` tombstone is published as if it had never been
declared, a later append reuses the freed ordinal, and `ridl diff` reports the
append compatible. That is a wrong answer from the compatibility guarantee, on
shipped code. Two smaller defects sit beside it: a baseline path aimed two or
more levels above the snapshots reads as empty with no diagnostic, and a
composite body reorder is classified breaking only through the
`constraint_changed` fallback, which names no rule a reader can act on.

## 1. The three defects, reproduced at `b1c43fa`

All three reproduce at the current head of `main`. The reproduction fixtures are
throwaway and are not checked in.

- **#315.** Publish a baseline for an interface with three events. Delete the
  second, add no `reserved` line. `ridl check --baseline <original>` emits
  `warning[RIDL-407]` and exits 0. Publish again: exit 0 with no output at all,
  and the third event now holds the second's ordinal. Append a fourth event.
  Against the original baseline `ridl diff` exits 1 and reports
  `interaction_removed` plus `interaction_appended`. Against the second baseline
  it exits 0 and reports the append alone. The second baseline is what CI
  compares against once the republish has happened.
- **#235.** With snapshots published to `ws/.ridl/baseline/`,
  `ridl check ws --baseline ws` produces empty stdout and empty stderr and exits
  0. `first_nested_snapshot_dir` descends exactly one level, which its own doc
  comment records as deliberate and scoped to driftsys/ridl#230, and
  `load_baseline` returns an empty snapshot list that `desk_check` treats as "no
  baseline published yet" and short-circuits on.
- **#314.** Swap two fields of a struct and `ridl diff` exits 1 with the single
  line `[breaking] constraint_changed <path>`. The verdict is right and the
  category is not: no per-field detail survives, and a reader acting on
  `constraint_changed` looks for a constraint edit that does not exist.

## 2. Decisions

### D-1 The gate sits between the clean compile and the publish

**Decision.** `run_baseline` compiles the source to a staging directory and, on
a clean compile, calls `publish_baseline`, which deletes the stale `.ir.json`
files in the output directory and moves the fresh ones in. The gate runs between
those two steps. It loads the snapshots already in the output directory, diffs
them against the freshly built ones, and refuses to publish when the diff
carries an untombstoned interaction removal.

The output directory is the right side to compare against because it is exactly
what `publish_baseline` is about to destroy. No discovery, no flag, and no
ambiguity about which record is being replaced.

**Rejected.** Calling the existing `desk_check` from `run_baseline`. It is the
smaller edit and it reuses the loader, but it also inherits the one-level
discovery bound that D-3 leaves in place and the silent short-circuit on an
empty snapshot list, which would make the gate pass silently in exactly the
cases D-3 describes. A gate that can be defeated by where a directory sits is
not a gate.

### D-2 The refusal is a new code, RIDL-408; RIDL-407 does not change

**Decision.** The refusal is a new diagnostic, **RIDL-408**, severity Error,
emitted only by `ridl baseline`. Its message names the interaction that was
removed and the `reserved <name>` line that would sanction the removal.
`ridl baseline` exits 1, which is the row ADR-0010 decision 1 already assigns it
for a negative answer over the checked source. The staging directory is removed
and the existing baseline is left byte-identical, which is the behaviour the
command already has for a compile that carried an error.

RIDL-407 keeps its severity, its wording and its single call site. The reference
defines it as the desk-time warning that an ordinal moved at all, emitted by the
`ridl check` facade, and states that it neither classifies nor gates. Promoting
it under `ridl baseline` would contradict that sentence, and it states a
different fact in any case: an ordinal moving is the consequence of the removal,
not the removal.

**Rejected.** Promoting RIDL-407 to an error for `ridl baseline` alone, which is
the disposition issue #315 proposes. See the paragraph above for why the
reference forbids it.

### D-3 An explicit baseline that holds no snapshots is an input error

**Decision.** The search depth does not change. What changes is what an empty
result means:

- An explicit `--baseline <path>` that yields zero snapshots is an input error,
  **exit 2**, with the cause named: the path exists but holds no `.ir.json`
  snapshot at the depth the loader reads.
- Auto-discovery keeps its silent skip. With no flag and no published baseline,
  `ridl check` behaves as it did before the baseline command existed, which is
  the property `baseline_location` already documents.

ADR-0010 decision 6 made this same fail-closed change for `ridl fmt`, where an
unreadable directory reached mid-walk let `ridl fmt --check` flip from failing
to passing silently in CI. This is the same failure with a different cause.

**Rejected.** Descending arbitrarily deep from the given path. It closes the
same case, but `first_nested_snapshot_dir` records its one-level bound as a
deliberate decision scoped to driftsys/ridl#230, and an unbounded walk makes any
directory above a workspace a plausible baseline. Reopening a recorded bound is
a larger change than the defect needs.

### D-4 The composite reorder category is `MemberReordered`

**Decision.** A new `Category::MemberReordered`, classified breaking, for a
composite body whose member names match on both sides but whose order differs.
In `diff_composite`, inside the branch that today emits one whole-container
`ConstraintChanged`, the two name lists are compared as sequences. When they
differ, one `MemberReordered` is emitted per member whose index moved, carrying
the old and new names, in place of the single `ConstraintChanged`. When the
order matches and the bodies still differ, `ConstraintChanged` is emitted
exactly as it is today.

The name is `MemberReordered` rather than `DeclReordered` because `DeclAdded`
and `DeclRemoved` are already how `diff_composite` spells a composite member,
and a `Decl*` reorder would read as a reorder of top-level declarations.

**This closes nothing that driftsys/ridl#302 guards.** The carried-debt comment
above `diff_composite` records that the name-keyed comparison never reads the
body's `reserved` list, and instructs that it must not be closed alone because
of a FlatBuffers union-discriminant coupling. The sequence comparison reads no
`reserved` list and changes no removal matching, so that debt and its comment
stay exactly as they are.

### D-5 The gate covers the interaction level only

**Decision.** The gate refuses an untombstoned interaction removal. It does not
refuse anything at the interface level: not a removed interface without a
service-level tombstone, not a provisional interface number.

The ridl reference states the tombstone rule at two levels, interactions within
an interface and interfaces within a service's shape list. D-7 of the rsdl
decisions retires the second one — a service's interface list becomes a set,
RIDL-146 to RIDL-148 retire with it, and interface identity moves to a
per-package lock file. Building the service-level half of the gate now would be
building something the lock block deletes. The interface-level refusal that D-7
describes arrives with the lock, against the lock, which is the only place a
provisional number or a retired entry exists to check.

### D-6 Two changes, two pull requests

**Decision.** D-1 to D-3 land together in `crates/ridl`, because they are one
command surface and one loader. D-4 lands separately in `crates/ridl-diff`. The
crates are disjoint and neither change needs the other.

## 3. Error handling

- **A compile that carries an error** publishes nothing and leaves the baseline
  untouched, as it does today. The gate does not run, because there is nothing
  fresh to compare.
- **A refused publish** removes the staging directory and leaves every published
  file byte-identical. The gate reports every untombstoned removal it found, not
  the first, so one run tells the author the whole correction.
- **A first publication** — the output directory is absent, or holds no snapshot
  — has nothing to compare and publishes. This is not an error and draws no
  diagnostic.
- **An output directory that cannot be read** is exit 2 with the cause named,
  which is what the publish path already does for a write it cannot perform.
- **A published snapshot that cannot be parsed** refuses publication, exit 2,
  rather than being silently overwritten. A baseline that cannot be read cannot
  be shown safe to replace, and overwriting it would destroy whatever ordinal
  record it held with no one seeing it — the exact failure this gate exists to
  prevent. The message names a remedy: restore the file, for example from
  version control or by resolving a merge conflict left in it, or delete it and
  run `ridl baseline` again, which discards the record it held.

## 4. Testing

Every test below starts red and is written before the change it pins.

- **The gate refuses.** Publish a baseline, remove an interaction with no
  tombstone, publish again: exit 1, RIDL-408 naming the interaction, and the
  published files byte-identical to what they were before the refused run. The
  byte-identity assertion is the one that matters — an exit code alone does not
  prove the record survived.
- **The gate admits the sanctioned path.** The same removal with a `reserved`
  line republishes at exit 0.
- **The gate admits a first publication.** An absent output directory publishes
  at exit 0 with no diagnostic.
- **The gate admits an append.** Adding an interaction at the end republishes at
  exit 0.
- **An explicit baseline holding no snapshots** exits 2 with the cause named.
  This gives driftsys/ridl#234's control something to assert that is not
  vacuous.
- **Auto-discovery with no baseline** still exits 0 and emits nothing.
- **A composite reorder** reports `member_reordered` per moved member and exits
  1; an in-place constraint edit still reports `constraint_changed`.
- **The `--explain` coverage test** fails until `MemberReordered` has its
  `explain`, `category_word` and `classify` arms. This is compiler-enforced and
  needs no new test.

## 5. Alternatives considered

Recorded per decision above, collected here so the rationale survives gardening:
reusing `desk_check` from `run_baseline` (D-1); promoting RIDL-407 (D-2);
descending arbitrarily deep from an explicit baseline path (D-3); naming the
category `DeclReordered` (D-4); gating the interface level now (D-5).

## 6. Amendments implied for existing records

- **ridl language reference** — the diagnostics table gains a RIDL-408 row.
  Section 11 gains a sentence stating that publication refuses an interaction
  removed without a `reserved` tombstone. The RIDL-407 row and the sentence
  stating that it neither classifies nor gates are unchanged.
- **ADR-0010 decision 1** — the `ridl baseline` row's exit-1 column gains the
  refusal, and its exit-2 column gains an explicit `--baseline` path that holds
  no snapshot. The table's own scoping sentence says a row is earned by being
  added and checked, so both cells are verified by direct construction against
  the built binary, as the table's existing cells were.
- **`docs/book/cli-reference.md`** — the diff category list gains
  `member_reordered`. The `ridl baseline` section and the exit-code summary
  table also change: the gate adds RIDL-408 to exit 1, and adds the unreadable
  output directory and the unparseable published snapshot to exit 2.
- **rsdl decisions note D-7** — no change. Its defect paragraph describes what
  #315 is, and this note is its disposition, not a correction of it.

## 7. Open

- **Decided.** RIDL-408's message shape: `ridl_diff` emits `InteractionRemoved`
  for three distinct shapes — a bare removal, a tombstone at the wrong ordinal,
  and a dropped tombstone — and each gets its own wording rather than one
  message reused across all three. A tombstone at the wrong ordinal is told
  apart structurally, from the change's own `after` value; a bare removal is
  told apart from a dropped tombstone by asking the published IR whether it
  already reserved the name, never by reading the wording of the change's
  `before` value.
- Whether `MemberReordered` should also be emitted when a member is inserted
  mid-body and every later member shifts. Today that case emits `DeclAdded` and
  the classifier judges the direction by reading the body. This note does not
  change it, because #314 reports the pure swap and widening the case would
  reach the matching logic driftsys/ridl#302 guards.
