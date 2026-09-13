# Interface id study — carrier, tooling, and the refuting experiment

Status: study report, written 2026-09-12 in a fresh context against the crates
at `69def56`. Summarised in
[`2026-09-12-rsdl-rewrite-decisions.md`](2026-09-12-rsdl-rewrite-decisions.md)
D-7 and §6, which override its recommendation (the stamped attribute) on the
family-general-form §6.3 consistency argument. Kept verbatim apart from the
scratchpad paths.

## Claim

Keep the id in the source as `interface X [ id = N ]` with a package-level
`reserved N`, and let the tooling own the number: a package-aware stamper
(`ridl fmt` and an LSP quick fix) allocates a missing id, the compiler refuses
an unstamped or duplicated id, `ridl diff` matches interfaces by id, and
`ridl baseline` refuses to publish an untombstoned removal. No other carrier
survives a plain-editor rename and a move between files while failing loud on a
merge; the better-reading alternatives (a positional catalog list, a registry
file) each reopen one silent path the attribute closes.

## (a) Options against the criteria

| #  | Option                                                                                          | 1 Safety                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            | 2 DX                                                                                   | 3 Consistency                                                                                                    | 4 Cost                                                                                                                                                         |
| -- | ----------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1  | Hand-typed `[ id = N ]` + `reserved N`                                                          | Rename, move, file rename, insert, reorder: nothing positional. Delete without tombstone: `DeclRemoved`, Breaking. Merge: both branches take the same number; a duplicate is a compile error.                                                                                                                                                                                                                                                                                                                                                       | Types a number and a tombstone; forgetting is a compile error.                         | Attribute forms §4.2–4.4 fit; first explicit wire number in the family; `reserved <int>` already in the grammar. | Parser: attr block on the `InterfaceDef` header (FORM-101 today), package-level `ReservedEntry` (FORM-102 today); sem 4 codes; IR 2 fields; diff 3 categories. |
| 2  | Registry file keyed by name                                                                     | Plain-editor rename: unregistered name, loud, but recovery cannot tell rename from retire+add. Remembers retired ids by itself. Merge: both branches append the same number to one file.                                                                                                                                                                                                                                                                                                                                                            | Second file, a command.                                                                | Outside the surface; D-7 as written.                                                                             | Reader, writer, diff.                                                                                                                                          |
| 3  | `ridl.lock`                                                                                     | Regenerated, per workspace, does not ship.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          | —                                                                                      | Contradicts ADR-0002 §7.                                                                                         | —                                                                                                                                                              |
| 4  | Position across files                                                                           | Move and file rename break; today both are `identical`, so this creates wire identity out of file order.                                                                                                                                                                                                                                                                                                                                                                                                                                            | Nothing to type.                                                                       | Member model.                                                                                                    | Diff only.                                                                                                                                                     |
| 5  | Name hash                                                                                       | Rename breaks silently without a baseline; unreadable; 64 bits is not a small id.                                                                                                                                                                                                                                                                                                                                                                                                                                                                   | Nothing.                                                                               | Rejected, ADR-0016 d8.                                                                                           | —                                                                                                                                                              |
| 6  | `.ridl/baseline` as record                                                                      | Keyed by name; republished wholesale on accept (measured), so retired ids vanish.                                                                                                                                                                                                                                                                                                                                                                                                                                                                   | —                                                                                      | —                                                                                                                | —                                                                                                                                                              |
| 7  | Stamp the service                                                                               | Recomposition changes the id of an interface (breaks V-05 immunity); a service may list an interface of another package (ADR-0015 d24), so the id lands in the wrong catalog (V-16); an interface with no service has no id.                                                                                                                                                                                                                                                                                                                        | Saves #interfaces − #services stamps; zero when one-to-one.                            | Reintroduces the service number D-7 removed (V-X2).                                                              | Frame and lowering.                                                                                                                                            |
| 8  | Hybrid inferred/explicit                                                                        | An inferred id depends on file order and on the other interfaces, so a move or a merge renumbers it relative to a branch build; the stable sub-rules are all-inferred (= 4) or all-explicit (= 1).                                                                                                                                                                                                                                                                                                                                                  | Fewer stamps in toy packages.                                                          | Adds a mode.                                                                                                     | Stamper must read the baseline.                                                                                                                                |
| 9  | Registry + tool-owned rename/move                                                               | As 2, with rename safe through the tool; a bypass fails loud, recovery is a human judgement.                                                                                                                                                                                                                                                                                                                                                                                                                                                        | Commands for rename and move; the LSP rename resolves `SymbolKind::Interface` already. | As 2.                                                                                                            | 2 + commands + LSP hook.                                                                                                                                       |
| 10 | 1 + stamper, quick fix, baseline gate                                                           | As 1; the delete gap closed by `ridl baseline` refusing an untombstoned removal.                                                                                                                                                                                                                                                                                                                                                                                                                                                                    | Types nothing in the common cases; one error and one fix when forgotten.               | As 1, plus one stated `fmt` exception.                                                                           | 1 + package-aware stamper + baseline rule.                                                                                                                     |
| 11 | Prior art                                                                                       | proto3: hand-typed numbers and `reserved`. Cap’n Proto: `@N` dense from 0, gap or duplicate is a compile error; file ids from `capnp id`. FlatBuffers: `id:` all-or-nothing, dense, `deprecated` holds the slot. COM: `[uuid]` pasted from `uuidgen`, travelling with the declaration through a rename. AUTOSAR SOME/IP: integrator-allocated ids in the deployment model. FIDL: hashed, pinned through a rename by `@selector`. Pattern: rename-safe schemes co-locate the identity with the declaration; merge-loud schemes use explicit numbers. |                                                                                        |                                                                                                                  |                                                                                                                                                                |
| 12 | Catalog list block `catalog { A, reserved B, C }` (the service list one level up, in one place) | Rename: dangling name, loud. Move safe. Reorder in the block: Breaking (tidying hazard). Merge: both branches append at the same anchor; measured today at the service level: `compatible` against main, `breaking` only against the own build of the branch — the second addition is renumbered unreported.                                                                                                                                                                                                                                        | Add the name to the list, or the formatter appends it.                                 | The own model of the family, verbatim.                                                                           | New keyword and block; walk reused.                                                                                                                            |

## (b) Ranking, recommendation, developer story

Ranking: 10, 1, 12, 9, 8, 2, 7, 6, 4, 3, 5. Recommendation: 10. The number is
attached to the declaration, so a plain editor cannot separate them; explicit
numbers collide on a merge instead of shifting; the tooling removes the typing.

- New interface: write `interface CruiseControl { … }`, save. The LSP reports
  "interface has no id" (error) with the quick fix "allocate id 4", which writes
  `[ id = 4 ]` into the header; `ridl fmt` does the same on the CLI — a missing
  id only, never a change to one, next = max(ids ∪ reserved) + 1 over the
  package. Without the stamp `ridlc` fails with one error.
- Rename: change the name; the attribute stays; `ridl diff` matches by id,
  reports `interface_renamed`.
- Move between files, file rename: cut and paste the declaration; `ridl diff`:
  `identical`.
- Delete: delete the declaration, write `reserved 4` anywhere in the package.
  Forgotten: `ridl check --baseline` reports it with a quick fix,
  `ridl baseline` refuses to publish, `ridl diff`: `decl_removed`, Breaking.
  With the tombstone: `interface_retired`, Compatible.
- Insert, reorder: no effect.
- Merge: both branches stamped 4; compile error "duplicate interface id 4"; the
  quick fix renumbers the interface absent from the baseline.

## (c) Amendments implied

- `docs/wip/2026-09-12-rsdl-rewrite-decisions.md` D-7: the record is the
  attribute, not a checked-in file; never-reuse, rename-keeps-id, explicit
  allocation and build-fails-when-unstamped stay; "reads the registry" becomes
  "reads the ids in the IR".
- `docs/wip/2026-09-08-topology-vocabulary.md` V-17: "recorded in source as the
  `id` attribute of the interface". ADR-0016 d8: unchanged.
- ridl reference §11: a third level, package-level `reserved <int>`, the
  baseline gate, the new codes. §14.5 and ADR-0015 d17: a binding keys ordinal
  spaces on (package, id), not the name — required by the rename constraint;
  RIDL-147 then needs a ruling.
- `docs/wip/family-general-form.md` §4.3: row
  `id | assignment | interface | routing table, rsdl lowering, diff`; §5: `fmt`
  stamps a missing id, the one place it assigns identity.
- `crates/ridl-ir/proto/ridl/ir/v2/ir.proto`: `Interface.id` (uint32, field 7),
  package-level reserved ids; additive per ADR-0014. `crates/ridl-diff`:
  `InterfaceRenamed` (verdict: owner ruling), `InterfaceIdChanged` (Breaking),
  `InterfaceRetired` (Compatible). `crates/ridl/src/main.rs` `run_baseline`:
  refuse an untombstoned removal at both levels. ADR-0002 §7: untouched.

## What would refute this

Build package P with `Alpha [id=1]`, `Beta [id=2]`, `Gamma [id=3]`; run
`ridl baseline .`; build a consumer from it. With a plain editor delete `Gamma`,
no tombstone. `ridl baseline .` must exit 1 naming `reserved 3`. If it exits 0:
add `Delta`, run the stamper, run `ridl diff .ridl/baseline .`. Refuted if
`Delta` receives id 3 and the diff exits 0: the consumer routes the messages of
`Gamma` to `Delta` and no gate said so. The same sequence one level down is open
in the current crates today (below), so the recommendation is not worse than the
model it extends, and the baseline rule closes both levels.

## What I actually ran

Detached worktree (removed afterwards), `cargo build --locked -p ridl`; fixtures
in the session scratchpad, not kept (package `veh.cat`).

- Rename Beta→Gamma: `breaking` (`decl_removed`, `decl_added`). Beta moved to a
  second file, or reordered before Alpha: `identical`.
- Service list merge: `Alpha, Beta` → `Alpha, Beta, Gamma, Delta`: `compatible`
  (two `service_shape_appended`); `Alpha, Beta, Delta` → the same list:
  `breaking` (`service_shape_inserted …/Gamma`).
- `interface Alpha [ id = 1 ] {`: `FORM-101`. Top-level `reserved 2`:
  `FORM-102`.
- `ridl fmt` on a `.ridl` file with interfaces: exit 0, interface lines
  untouched.
- Reuse path: `ridl baseline .` on `Alpha { a, x }`; removed `x`;
  `ridl check --baseline`: `warning[RIDL-407]`; `ridl baseline .`: exit 0;
  appended `y`; `ridl diff .ridl/baseline .`: `compatible`,
  `interaction_appended veh.cat/Alpha/y`.

## What I could not check, and why

- The interface id width in the frame — the vocabulary note gives slot u8 and
  hash u64 only. It bounds the legal range and decides whether a
  coordination-free random id (COM, Cap’n Proto) is possible; the ABI record of
  the first consumer is outside this repository.
- Whether a rename is wire-compatible on the name-keyed bindings of ADR-0015
  d17, and the verdict for `InterfaceRenamed`: owner rulings. Without the d17
  amendment the id buys order-independence, not rename safety.
- Where the stamper lives (`ridl fmt` or a new command) and whether the
  missing-id error fires always or only on lowering to the runtime: rulings; I
  recommend `fmt` and always.
- LSP rename of an interface end to end: not run.

## Confidence

1. The co-location argument is structural and the merge and reuse behaviours
   were measured on the current crates; the residual is a ruling that rename
   stays Breaking on name-keyed bindings, which narrows the benefit without
   changing the carrier.
