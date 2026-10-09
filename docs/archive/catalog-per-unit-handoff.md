# Catalog per unit: Kotlin heads-up and release note

Texts for the driver to post and release. The driver files the issue on
driftsys/ridlc-gen-kotlin and runs the release; nothing here is posted yet.

## 1. Kotlin plugin heads-up (issue draft)

Title: One catalog per unit: region lookup by `Catalog.package`, catalog names
in regions

Body:

ridl `<version>` writes one catalog per unit instead of one catalog per source
package. A unit is the tree of source packages under one `ridl.toml`. Two things
change for a codegen plugin.

1. **Find the region by `Catalog.package`.** The deployment section of the
   system artifact has one region per catalog. Find the region of a unit by
   matching `Catalog.package` (the unit name), not `Model.name`. One unit can
   hold several source packages, so `Model.name` of one source package is not
   the unit name.
2. **Region interface names are catalog names.** An interface name in a region
   is the source package path relative to the unit, followed by the interface
   name, for example `cluster.SpeedDisplay`. `Model.interfaces[i].name` stays
   the short name (`SpeedDisplay`). Match a region interface to a model
   interface by the catalog name of the model interface (its source package path
   relative to the unit, a dot, then its short name).

What does not change:

- Generated packages stay per source package.
- `Catalog.package` keeps its name and its number.
- `Catalog.retired` now carries the whole retired list of the unit, with entries
  under catalog names (`cluster.Speed`), not short names.
- `NUMBER` is the number of the interface within the unit.

Breaking effects:

- Every catalog hash changes, because the hash covers the reduced unit.
- Interface numbers are reassigned per unit, so a number emitted by an earlier
  version does not match the new number of the same interface.
- The IR field `Package.retired[].name` holds the lock key (the catalog name),
  not the short name.

Requested action: update the region lookup and the interface name matching, then
regenerate against ridl `<version>`.

## 2. Release note

BREAKING CHANGE: ridl writes one catalog per unit. The reduced unit, not the
reduced package, is the input of the catalog hash.

- `ridl build --emit catalog` writes `<unit>.catalog.binfb`, one file per unit.
  Interface names in the descriptor are qualified by the source package path
  relative to the unit (`cluster.Speed`).
- `ridl lock` writes one `interfaces.lock` per unit, beside `ridl.toml`. Lock
  keys are qualified by the source package path relative to the unit. An
  `interfaces.lock` in a subdirectory of a unit is not read; it draws the
  warning RIDL-416 `lock-in-subdirectory`.
- Interface numbers are one space per unit and are reassigned. Every catalog
  hash changes.
- A region of the system artifact is one unit. Region interface names are
  catalog names, and a plugin finds its region by `Catalog.package`.
- `ridl diff` groups baseline snapshots by unit and matches interface numbers
  within the unit. A move between sibling packages is `InterfaceRenamed`, with
  catalog names on both sides.
- The IR's retired entries (`Package.retired[].name`) carry the lock key, which
  is the catalog name, not the short name. `Catalog.retired` in the codegen
  model lists the entries of the unit under catalog names.
- The baseline gate refuses with RIDL-412 a package deleted, without retiring
  its numbers, from a unit that still has other packages. Before this release
  the baseline gate did not see the interfaces of a deleted package, because
  `ridl diff` reported the package as one removal. It now reads the package
  declaration by declaration when its unit still has other packages, so an
  unretired number is refused. A package whose whole unit is deleted is still
  reported as one removal and is not refused.
- Two new manifest errors: MANI-013 (a `ridl.toml` inside a unit's tree) and
  MANI-014 (a source package declared by two units).
- Baseline snapshots written before this release carry no `unit`. Each is
  treated as its own unit until the project publishes a new baseline.

Migration, in three steps:

1. Delete the per-package `interfaces.lock` files.
2. Run `ridl lock` to number the unit.
3. Publish a new baseline with `ridl baseline`.

The `evals/corpus/vss` corpus was re-cut into flat members, and the book harness
stages one flat member per package, so neither nests a `ridl.toml`.

## 3. Gardening pointers

For the `sdd-gardening` pass that archives the spec and this plan. The records
below were amended by the change; check them against the code, do not rewrite
them.

- Decisions: ADR-0002 section 1 and section 4; ADR-0014 decision 15; ADR-0015
  (the lock sentences, the decision 17 key, a new `ridl diff` paragraph);
  ADR-0016; ADR-0022 decisions 6 and 7; `docs/decisions/README.md`.
- Design: `docs/design/catalog-descriptor.md`,
  `docs/design/interaction-face.md`, `docs/design/codegen-plugins.md`,
  `docs/design/README.md`.
- Specification: `docs/specification/ridl-language-reference.md` section 11 and
  section 14.5; `docs/specification/rsdl-language-reference.md` section 13, V-16
  and Appendix A; `docs/specification/ridl-family-overview.md` (decision
  ledger); `docs/specification/frame-specification.md`.
- Technotes: `docs/technotes/rsdl-implementation.md`,
  `docs/technotes/walking-skeleton-architecture.md`.
- Book: `rsdl.md`, `getting-started.md`, `codegen-plugins.md` and
  `cli-reference.md` and `lints.md` (the RIDL-416 `lock-in-subdirectory` row)
  under `docs/book/`.

Archive to `docs/archive/`:

- `docs/wip/2026-10-08-catalog-per-unit-design.md` (the spec)
- `docs/wip/2026-10-08-catalog-per-unit-plan.md` (the plan)
- `docs/wip/2026-10-08-catalog-per-unit-handoff.md` (the execution handoff)
- this file, after the issue is posted and the release is cut
