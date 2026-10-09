# One catalog per unit — implementation handoff

Date: 2026-10-08. For the session that executes the plan.

## Where things are

- Worktree `../ridl-catalog-unit`, branch `docs/catalog-per-unit`, head
  `f4ffbe31`, not pushed.
- Spec (approved): `docs/wip/2026-10-08-catalog-per-unit-design.md`.
- Plan (approved, reviewed twice):
  `docs/wip/2026-10-08-catalog-per-unit-plan.md`, 17 tasks. Its "Answers from
  the author" section is settled.
- Run `./bootstrap` in the worktree before the first build if it has not run.

## How to execute

1. Rename the branch to `feat/catalog-per-unit` before the first code commit,
   because it will hold code.
2. Use `superpowers:subagent-driven-development`: one implementer subagent per
   task, then one reviewer per task, before the next task starts. Record the
   base commit before each implementer starts and give the reviewer the diff
   from that base as a file. The reviewer gives two verdicts: the task meets its
   brief, and the code is sound. Reviewers mutate the code to check that the
   tests catch a wrong implementation, each in its own worktree with its own
   `CARGO_TARGET_DIR`.
3. Order and parallel groups (from the plan):
   - {T1, T2, T3, T8} in parallel;
   - T4 → T5 → T6 in sequence (all edit `workspace.rs`); T7 beside T6;
   - T9, then T10;
   - {T11 → T12 → T13} beside T14;
   - {T15, T16, T17} in parallel.
4. Models: Fable for T9 (the `check_package` split), T11 (the reduced-unit hash)
   and T14 (`ridl diff` matching across packages). Sonnet for the other
   implementers. The driver keeps the rulings.
5. T13 and T17 run the full `just build`. Every other task runs the recipes it
   names.
6. After T17: run `/review` (passes 1 and 2) over the whole branch, then the
   `sdd-gardening` skill so that `docs/wip/` holds none of this work, then open
   one PR to `main`.

## Things to do by hand

- Post the Kotlin plugin heads-up that T17 drafts (on
  driftsys/ridlc-gen-kotlin). It must say that a plugin finds its region by
  `Catalog.package`, not `Model.name`, and that region interface names become
  catalog names.
- The change is breaking: the release notes say to delete per-package
  `interfaces.lock` files, run `ridl lock`, and run `ridl baseline` again.
- Never write the consumer's real package names anywhere. Use `com.example.*` or
  `veh.*`.

## Rulings already made

- Keep `[package]`; the unit name is the root source package and the catalog
  name.
- No nested manifests (MANI-013). A source package claimed by two units is an
  error (MANI-014). A lock in a subdirectory is a warning (RIDL-415).
- `unit` field on IR `Package`; baselines stay per source package; `ridl diff`
  groups by unit and reports a move to a sibling package as `InterfaceRenamed`.
- Retired entries with no existing source package go to the anchor package
  (root, else first by name).
- One hash per unit. A unit whose parts change at different rates is split.
- `.rxdl` is future work (E7.1, #68); #770 tracks the warning for a skipped
  `.rxdl` file.
