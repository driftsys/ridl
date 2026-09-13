# Interface id study 2 — the per-package generated lock

Status: study report, written 2026-09-13 in a fresh context against the crates
at `69def56`, after the first study. Summarised in
[`2026-09-12-rsdl-rewrite-decisions.md`](2026-09-12-rsdl-rewrite-decisions.md)
D-7 and §6. Kept verbatim apart from the scratchpad paths.

## Claim

The lock model holds on the diff and on the keyboard, but two of its stated
mechanisms fail silently as written: the union merge driver plus "the entry
already on main keeps its number" turns a rename into retire-plus-add in one
merge direction with a Compatible verdict, and "next free number" lets a
hand-deleted line free a number that the by-id diff then reads as a rename. Both
close with a three-way driver run by `ridl lock` and a `next N` line the tool
never lowers. Bodies stay positional.

## 1. Diff

| Change                 | Verdict                                                                                                                                               | Current structure                                                                                                                                                                                                                 |
| ---------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Appended interface     | `DeclAdded`, Compatible                                                                                                                               | Works: `added()` returns Compatible for a two-segment path.                                                                                                                                                                       |
| Renamed, same number   | `InterfaceRenamed`, Compatible (ruling below)                                                                                                         | Key `diff_interfaces` on the id, not `name`. Paths must carry the id: `classify` re-finds the container by name on both sides, so every interaction change inside a renamed interface hits the Breaking fallback of `appended()`. |
| Retired with entry     | `InterfaceRetired`, Compatible (as `InteractionRetired`)                                                                                              | Needs retired entries in the IR (a repeated field on `Package`); an id field alone cannot express a tombstone.                                                                                                                    |
| Removed, no entry      | Never reaches the diff: an entry without declaration fails the build; a hand-deleted line arrives as `DeclRemoved`, Breaking, and publish refuses.    | Works.                                                                                                                                                                                                                            |
| Number changed by hand | `DeclRemoved` + `DeclAdded`, Breaking; a by-name second pass can name it `InterfaceIdChanged`.                                                        | Small addition.                                                                                                                                                                                                                   |
| Interaction cases      | Unchanged                                                                                                                                             | `diff_interface` receives the matched pair.                                                                                                                                                                                       |
| Service list as a set  | `ServiceShapeAdded` Compatible; `ServiceShapeRemoved` Breaking (`service.member` addresses change; ruling if V-05 immunity should make it Compatible) | Retire the slot walk (`diff_service_shapes`, `keyed_by_name`, the `ServiceShape*` categories, `ServiceShape.id`, service-level `Reserved`, RIDL-146/147/148, ADR-0015 d15/d19/d24); keep RIDL-144/145.                            |

Rename on the bindings ADR-0015 d17 keys by name: a ruling is required — amend
d17 and the Appendix B eventgroup mapping to key ordinal spaces on (package,
number). Then Compatible holds everywhere: proto3 and FlatBuffers carry the name
only in generated identifiers, and classify.rs says `ridl diff` judges wire
identity, not API surface. Without the amendment the verdict must be Breaking
and the number buys nothing.

## 2. Merge

Git, `interfaces.lock merge=union`, base `A 1 / B 2`, new interfaces in their
own files so the source merges cleanly:

- Both add: lock `A 1 / B 2 / C 3 / D 3`. Renumbering the later line converges;
  union orders ours first, so the rule is deterministic.
- Rename + add: `A 1 / Bee 2 / B 2 / D 3` — union resurrected the base line
  (adjacent hunks); reverse direction `A 1 / B 2 / D 3 / Bee 2`. Under "an
  orphan entry becomes retired" and "first keeps the number", one direction
  yields `Bee 2, B retired`, the other `B 2 retired, Bee 4`: the rename became
  retire-plus-add, the diff against main says retired plus added, both
  Compatible, and consumers of B reach nothing at 2. Silent.
- Retire vs rename, same file: source conflict, loud.
- Retire vs rename-and-move: source clean; lock `A 1 / B 2 retired / Bee 2` —
  the state the previous case also produces, so after a union merge the tool
  cannot tell a resurrection from a true conflict. Plain three-way merge
  conflicts here, correctly.
- Both retire and re-add: same file, source conflict; different files, source
  clean, lock identical, `ridl check` reports TYPL-009 duplicate declaration.
  Loud.
- Retire vs add: `A 1 / B 2 retired / B 2 / D 3`, resurrection again.

Union always converges because it discards the base. A custom driver
(`ridl lock merge %O %A %B`) has the base: both-add resolves (ours keeps, theirs
renumbered), rename-plus-add and retire-plus-add resolve (one side unchanged),
retire-vs-rename refuses. Record that instead of union.

## 3. Developer experience

| Operation            | Types            | Runs                                                                                    | Mistake                                                                             |
| -------------------- | ---------------- | --------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------- |
| New                  | the declaration  | `ridl lock`; one line appended                                                          | build: "`X` has no entry; run `ridl lock`"                                          |
| Rename, LSP          | the new name     | rename edits declaration and entry                                                      | none                                                                                |
| Rename, plain editor | the new name     | build fails naming the orphan entry and the unregistered declaration; two fixes offered | choosing "retire and allocate" cuts consumers off silently (a retire is Compatible) |
| Move between files   | cut, paste       | nothing                                                                                 | none                                                                                |
| Reorder              | nothing          | nothing                                                                                 | none                                                                                |
| Delete               | delete the block | `ridl lock` marks the entry retired                                                     | forgotten: build fails on the orphan entry                                          |
| Merge                | nothing          | driver, then `ridl lock`                                                                | union: §2                                                                           |
| Publish              | `ridl baseline`  | refuses a removed interface with no retired entry                                       | —                                                                                   |

| Axis            | Lock (this model)                                       | Option 12 catalog list                     | Option 10 attribute                         |
| --------------- | ------------------------------------------------------- | ------------------------------------------ | ------------------------------------------- |
| Typing          | none; a tool run                                        | a name into the list                       | none; a stamp                               |
| Numbers visible | in the lock only                                        | by position                                | on the declaration                          |
| Rename          | LSP, or a two-choice build error                        | edit the list entry                        | none                                        |
| Reorder         | free                                                    | breaking                                   | free                                        |
| Merge           | duplicate numbers; needs a three-way driver             | conflict at one anchor, or silent renumber | duplicate ids, compile error                |
| Infrastructure  | file format, `ridl lock`, merge driver, IR retired list | a keyword and a block                      | an attribute and a package-level `reserved` |

## 4. Bodies

Measured: swapping two struct fields diffs `breaking` as
`constraint_changed veh.cat/S` — caught only by the fallback in `constraint()`,
with no reorder category. Recommendation: bodies stay positional; add a
`MemberReordered` category. Reasons: a body gives one order, and the lock exists
exactly where no body does; a per-field lock is touched by every field addition,
so feature branches conflict in it on every merge; FlatBuffers ids must be dense
(`member_id` is ordinal minus one, `reserved` becomes a `deprecated`
placeholder), so a sparse lock needs a placeholder per gap and the proto3
identity mapping becomes a lookup; the tidying hazard is already covered by
inlay ordinals, RIDL-407 and the Breaking verdict.

## What would refute this

Lock `A 1 / B 2 retired / C 3`. Delete the retired line by hand, or let "next
free" mean the lowest unused number. Add `interface E` with the member names B
had. `ridl lock` gives E 2; build; `ridl diff .ridl/baseline .`. Refuted if the
diff exits 0 with `interface_renamed B -> E`: every consumer built earlier now
routes B traffic to E. Closed if the lock carries `next 4`, the tool allocates
from it, and the diff reports a lowered `next` as Breaking. On the current
crates the same reuse one level up is caught only because the walk keys by name:
`ridl diff sv-base sv-reuse` (`service : A, B` → `A, C`) reports
`service_shape_removed B` and `service_shape_appended C`, both breaking.

## What I actually ran

Fixtures and the git simulation in the session scratchpad, not kept (`scn/`,
`mergesim/`). `ridl diff st-base st-reord`: `breaking`,
`constraint_changed veh.cat/S`. `ridl diff sv-base sv-reuse`: as above. Seven
merges with the union driver, plus `git merge-file` with and without `--union`
in both directions.

## What I could not check, and why

- The tool does not exist: how rules (b) and (c) interact after a union merge,
  and what "next free" means, are read literally from the model.
- The d17 and `ServiceShapeRemoved` rulings: owner decisions.
- The LSP rename writing the lock, and the interface id width in the frame: not
  run, not in this repository.

## Confidence

1. The merge and reuse findings are measured, the diff findings are read from
   the code; the residual is whether the owner accepts a three-way driver and a
   `next` line, without which the model is not equally safe.
