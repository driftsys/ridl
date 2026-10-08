# One catalog per unit — design

Status: draft, for review. Date: 2026-10-08.

## Problem

Today `ridl build --emit catalog` writes one catalog for each source package
that declares an interface shape (an `interface`, or a `service` with an inline
body). A project that splits its interfaces over several source packages, for
example `com.example.hmi.cluster` and `com.example.hmi.climate`, gets one
catalog per package. An engine must then load and track several catalogs, and a
component that uses interfaces from those packages needs one attached port per
catalog.

The goal is one catalog for one unit of distribution: one manifest and the tree
of source packages below it, in the same way that one Rust crate is one unit.

## Terms

- **Unit**: one `ridl.toml` with a `[package]` table, plus every source package
  in its directory tree. A workspace member is a unit. A standalone manifest is
  a unit.
- **Source package**: one directory, named by the `package` line of each file in
  it (ADR-0002 §1). This meaning does not change.
- **Unit name**: the `name` of the `[package]` table. It is also the name of the
  root source package and the name of the unit's catalog.

In prose, "the package manifest" is the unit and "a source package" is a
directory namespace. A diagnostic or a document that says "package" alone means
a source package.

## Design

### The manifest

The manifest does not change shape. No new table and no new field.

```toml
# workspace root
[workspace]
members = ["common", "hmi", "vehicle"]

# hmi/ridl.toml
[package]
name = "com.example.hmi"    # the root source package and the catalog name
version = "1.0.0"
```

### Rules

1. **A unit is one `[package]` manifest and its directory tree.** The manifest's
   directory is the source package `name`, and each subdirectory is the source
   package `name.<path>`, as ADR-0002 §1 states today. A file whose `package`
   line does not match its directory is a compile error, as today. Every source
   package of a unit therefore starts with the unit name.
2. **No nested manifests.** A `ridl.toml` in a subdirectory of a unit's tree is
   an error. Today the loader skips such a directory without a diagnostic
   (`crates/ridl-core/src/workspace.rs`, `load_package_tree`). This needs a new
   `MANI-` code (MANI-013 is free). Two current users of nesting must change:
   the `evals/corpus/vss` workspace nests members on purpose (decision 5), and
   the book harness writes one `ridl.toml` per book package at `root/<a/b/c>/`
   (`crates/ridl/tests/book_examples.rs`), which would nest if the book declared
   both `x` and `x.y`. No such pair exists today; the harness is changed to
   write a flat layout so that the case cannot arise.
3. **One source package, one unit.** Across every unit that one build loads,
   remote imports included, a source package belongs to exactly one unit. A
   second unit that declares the same source package is an error. This needs a
   new `MANI-` code. Rule 1 prevents this inside one workspace when unit names
   do not overlap. The check is still needed for a remote import, and for two
   unit names where one is a prefix of the other (`com.example` and
   `com.example.hmi`). This needs a new `MANI-` code (MANI-014 is free). The
   loader does not load remote imports yet (`materialize_imports` in
   `crates/ridl-core/src/fetch.rs` is called only by its tests), so for now the
   check covers the workspace. When remote imports are wired in, a fetched
   import is one unit: its own `ridl.toml` and its tree, with rule 2 inside it.
4. **What a unit produces is inferred from what it holds.** One unit can produce
   both artifacts:
   - interface shapes in any of its source packages give **one catalog**, named
     after the unit;
   - the `system` gives **the system artifact**;
   - a unit with only types produces no artifact of its own. Other units import
     its types, and their catalog hashes include the types they reach.

   Rule 4 works per declaration, not per file extension, so a future `.rxdl`
   file (roadmap E7.1, driftsys/ridl#68) fits a unit without a change to this
   design.
5. **Components are not in a catalog.** Components, instances and deployments
   belong to the system artifact, which refers to catalogs by name and hash. A
   deployment change therefore never changes a catalog hash.
6. **Reuse is by explicit import**, from a sibling member or from `[imports]`.
   No wildcards and no re-exports (ADR-0002 §2, unchanged).
7. **One `system` per workspace** (RSDL-601, unchanged).

### The catalog

- **File**: `<unit name>.catalog.binfb`. One file per unit that has at least one
  interface shape.
- **`name`**: the unit name. Today it is the source package name
  (`crates/ridl-descriptor/src/lower.rs`).
- **Interface names**: a declared `interface` is qualified by its source package
  relative to the unit name, for example `cluster.SpeedDisplay`. An interface in
  the root source package keeps its short name (`Session`). Today the name is
  always the short name, which is ambiguous when two source packages of one unit
  declare the same short name.
- **Inline-shape services** keep their full global dotted name, with no
  unit-relative prefix. A service's dotted name is one global namespace, checked
  workspace-wide (RIDL-140, ridl reference §14.5), and it does not have to start
  with the unit name. The two forms cannot collide: a declared interface name is
  relative and an inline shape is marked as inline.
- **Numbers**: one numbering space per unit (see the next section).
- **Hash**: SHA-256 over the reduced unit. The reduced unit is one IR `Package`
  named after the unit. It holds every interface shape of the unit under its
  catalog name (above), sorted by (number, catalog name), and every declaration
  those shapes reach in any unit of the build, each under its full canonical
  name (`com.example.hmi.cluster.Foo`). Today `reduced_package`
  (`crates/ridl-ir/src/catalog_hash.rs`) writes the hashed package's own
  declarations under bare names; that cannot work when two source packages of
  one unit both declare `Foo`. Doc strings, `labels` and `deprecated` stay
  blanked. This amends ADR-0014 decision 15. The golden hash test and the corpus
  snapshots change.
- **Scope of a hash change**: a change to any source package of a unit changes
  the hash of the unit's one catalog, so every port bound to any interface of
  the unit fails its catalog check until it is rebuilt. Today a change to one
  source package leaves the catalogs of its sibling packages unchanged. This is
  the intended semantics (decision 6).

### `interfaces.lock`

- One `interfaces.lock` per unit, in the manifest directory. Today there is one
  per source package directory (ridl reference §11).
- Its keys are the catalog names: `cluster.SpeedDisplay` for a declared
  interface, `Session` in the root source package, and
  `service:<full dotted
  name>` for an inline shape, as today.
- The key grammar changes. Today an interface key is one identifier
  (`crates/ridl-core/src/interface_lock.rs`), so `cluster.SpeedDisplay` is
  rejected. The parser, `ridl lock --rename` and `--retire`
  (`crates/ridl/src/lock.rs`), and the merge driver (`ridl lock merge`) accept a
  dotted relative name.
- `ridl lock [PATH]` writes the lock of the unit that `PATH` is in.
- The number stays 1-based and is now scoped per unit, not per source package.
- **Numbering runs once per unit.** Today `number_interfaces`
  (`crates/ridl-sem/src/check.rs`) runs inside the per-package query, starting
  provisional numbers at the lock's `next` or 1 for each package, so two source
  packages of one unit would get the same provisional number. A unit-level step
  reads the unit's lock, gives each interface without an entry a provisional
  number from `next` upward in byte order of its lock key, and then folds the
  result into each source package's `Interface.number` and `Package.retired`.
- A retired entry belongs to the source package that its relative name names.
  `Package.retired` in the IR keeps short names, as today. The catalog
  descriptor's `retired` list and the codegen model's `Catalog.retired` carry
  the unit's whole list under catalog names.

### The unit in the IR

The hash, the system regions and `ridl diff` work on IR packages, and none of
them can derive the unit from a package name: a prefix is not enough when
`com.example` and `com.example.hmi` are two units, and `ridl diff` reads
snapshots without a manifest. So the IR records the unit.

- `v2::Package` gets a new field, `unit`: the unit name. The compiler fills it
  for every package it loads. A new field follows ADR-0014's rule for adding a
  field.
- The baseline snapshots (`.ridl/baseline/<pkg>.ir.json`) carry the field, so
  the old side of `ridl diff` knows its units without a manifest. A snapshot
  written before this change has no `unit`, which is one more reason for the
  re-baseline in the migration.
- The catalog hash groups packages by `unit`. The rsdl lowering keys regions on
  the interface's `unit`, not on its package name
  (`crates/ridl-sem/src/rsdl/lower.rs`).

### Single-file mode

`ridl check x.ridl` on a file outside any manifest, and the LSP and the MCP
server on such a file, load a synthetic package
(`crates/ridl-core/src/workspace.rs`). That package is its own unit: the unit
name is the file's `package` name, and the `interfaces.lock` beside the file is
read as the unit's lock, as today. A file inside a manifest's tree belongs to
that manifest's unit, whatever the entry.

### The runtime identity

`CatalogRef { name, hash }` keeps its shape. `name` becomes the unit name. Each
generated interface's `CATALOG` carries the unit name and the unit's catalog
hash, and its `NUMBER` is the unit's number. The catalog check
(`docs/design/interaction-face.md`, "The catalog check") is unchanged in logic.

### Example

```text
cockpit/                                  ← repository
├── ridl.toml                             [workspace] members = ["common", "hmi", "vehicle"]
├── common/                               unit com.example.common (types only, no catalog)
│   ├── ridl.toml                         [package] name = "com.example.common"
│   ├── ids.typl                          package com.example.common
│   └── units/units.typl                  package com.example.common.units
├── hmi/                                  unit com.example.hmi → one catalog
│   ├── ridl.toml                         [package] name = "com.example.hmi"
│   ├── interfaces.lock
│   ├── session.ridl                      package com.example.hmi
│   ├── cluster/speed.ridl                package com.example.hmi.cluster
│   └── climate/climate.ridl              package com.example.hmi.climate
└── vehicle/                              unit com.example.vehicle → catalog and system artifact
    ├── ridl.toml                         [package] name = "com.example.vehicle"
    ├── interfaces.lock
    ├── diag.ridl                         package com.example.vehicle
    ├── system.rsdl                       package com.example.vehicle
    └── components/cockpit.rsdl           package com.example.vehicle.components
```

The build output:

```text
out/com.example.hmi.catalog.binfb        Session, cluster.SpeedDisplay, climate.Climate
out/com.example.vehicle.catalog.binfb    Diag
```

These are errors:

- `hmi/cluster/x.ridl` that declares `package com.example.vehicle.x` (rule 1).
- `hmi/cluster/ridl.toml` (rule 2).
- A second loaded unit that declares `com.example.hmi.cluster` (rule 3).

## Migration

This is a breaking change. Interface numbers are reassigned per unit, and
catalog names and hashes change. There is no automatic merge of the old
per-package lock files. A project deletes its per-package `interfaces.lock`
files, runs `ridl lock` to number the unit, and publishes a new baseline with
`ridl baseline`. The release notes state this.

## Impact

| Area                     | Change                                                                                                                                                                                                                                                                                                            |
| ------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `ridl-core` loader       | rule 2 and rule 3 diagnostics; a unit boundary visible to later stages                                                                                                                                                                                                                                            |
| `ridl-sem`               | read one lock per unit; number per unit                                                                                                                                                                                                                                                                           |
| `ridl-ir`                | the reduced unit for the catalog hash; ADR-0014 decision 15                                                                                                                                                                                                                                                       |
| `ridl-descriptor`        | `lower` takes a unit, not a package; qualified interface names                                                                                                                                                                                                                                                    |
| `ridlc` / `ridl`         | one catalog file per unit; `ridl lock` writes the unit's lock; `ridl diff` groups the per-package snapshots by unit (decision 1)                                                                                                                                                                                  |
| `ridl-backend-rust`      | `CATALOG.name` is the unit name; `NUMBER` is the unit's number. Generated modules stay per source package                                                                                                                                                                                                         |
| `ridl-rt`                | no API change. The doc comment of `CatalogRef` ("one package's interfaces") changes to the unit                                                                                                                                                                                                                   |
| system artifact          | regions are per catalog, so there are fewer regions. No format change                                                                                                                                                                                                                                             |
| codegen plugins (Kotlin) | generated packages stay per source package. A plugin finds its region in the deployment section by `Catalog.package` (the unit name), not by `Model.name`. Region interface names become catalog names, while `Model.interfaces[i].name` stays short (decision 2). A heads-up issue is filed on the Kotlin plugin |
| `ridl-mcp`, `ridl-lsp`   | the new diagnostics; single-file mode as above                                                                                                                                                                                                                                                                    |

Fixtures and corpora that change:

- `evals/corpus/vss` and its calibration records (decision 5).
- `crates/ridlc/tests/corpus/rsdl-appendix-a`: one `[package]` with an empty
  root and four source packages. It has two catalogs and two regions today and
  one of each after the change; its snapshots change.
- `crates/ridl-mcp/tests/fixtures/{ws,ws-v2,ws-diag}/a`, which hold a
  subpackage.
- The book harness (rule 2).
- Every snapshot that carries a catalog hash.

Records to amend when this lands:

- ADR-0002 §1 (a unit holds a tree of source packages; rule 3) and §4 (rule 2).
- ADR-0014 decision 15 (the reduced unit).
- `docs/design/catalog-descriptor.md`: D-1, D-4 (catalog contents) and D-10 (the
  identity table).
- `docs/specification/ridl-language-reference.md` §11 (the lock and the number
  scope).
- `docs/design/interaction-face.md` ("The catalog check": the name is the unit
  name).
- The book's CLI reference for `ridl lock`, `ridl diff` and the catalog emit.
- `docs/specification/rsdl-language-reference.md` §13 and vocabulary V-16 ("its
  package name").
- ADR-0022 and the comment in `crates/ridl-ir/proto/ridl/ir/v2/system.proto` for
  regions ("its package name").
- ADR-0015 and ADR-0016 where they say the lock is per package.
- `docs/design/codegen-plugins.md` (the deployment section: region lookup).
- The book: `rsdl.md`, `getting-started.md`, `codegen-plugins.md`, and the
  `cli-reference.md` entries for `ridl lock`, `ridl baseline` and `ridl diff`.
- The comment of `Catalog.package` in
  `crates/ridl-ir/proto/ridl/codegen/v1/model.proto`.

## Alternatives considered

- **Load catalogs by prefix in the engine** (keep one catalog per source
  package; the engine loads every catalog whose name starts with a prefix).
  Rejected: it works without a change, but the engine still tracks N catalogs
  and N ports, which is the problem this design solves.
- **Wildcard imports** (`import com.example.hmi.*`) to pull the subpackages into
  one catalog. Rejected: ADR-0002 bans wildcards, and an import only brings
  names into scope. It does not move interfaces into another catalog.
- **A new `[module]` table** for the unit, so that "package" keeps one meaning.
  Rejected: the existing `[package]` table already names the unit, as the
  `package` attribute of an Android manifest does. The cost is that "package"
  has two meanings, which the Terms section resolves.
- **A free unit name with a `root` field** for the root source package.
  Rejected: rule 1 with the unit name as the root makes the clash in rule 3
  impossible inside one workspace, and needs no new field.
- **The Kotlin rule** (the `package` line decides, the directory is a convention
  checked by a lint). Rejected: it gives up ADR-0002's "one place a package can
  live", and it allows two units to claim one source package.
- **Exclusive unit kinds** (a unit is either a contract unit or a system unit).
  Rejected: one unit can hold both, as a Rust package can hold a library and a
  binary. `examples/cabin` stays one unit.
- **Merge existing locks without renumbering.** Rejected for now: a re-baseline
  is simpler, and there is no published catalog that must keep its numbers.

## Decisions on baselines, plugins and the root package

1. **Baselines stay per source package.** `ridl baseline` keeps writing one
   `.ridl/baseline/<pkg>.ir.json` snapshot per source package, because the IR
   stays per source package. `ridl diff` groups the snapshots by unit and
   matches interface numbers within a unit. A retired entry in the unit's lock
   carries its qualified name (`cluster.Old`), so it maps back to its source
   package. A snapshot per unit was rejected: it needs a new IR shape and gives
   no other benefit.
2. **Codegen plugins read the catalog from the codegen model.** A plugin does
   not compute the catalog name or hash. It reads
   `Catalog { package, hash, retired }` from the codegen model
   (`crates/ridl-ir/proto/ridl/codegen/v1/model.proto`). The field `package`
   keeps its name and its number, and its comment changes: it carries the unit
   name, which is `CatalogRef.name`. Renaming the field to `name` was rejected:
   it breaks the generated accessors of every plugin and changes nothing on the
   wire. Two things do change for a plugin. It finds its region in the
   deployment section by `Catalog.package`, not by `Model.name`. And region
   interface names become catalog names (`cluster.SpeedDisplay`), while
   `Model.interfaces[i].name` stays short. The heads-up issue on the Kotlin
   plugin states both.
3. **The root source package can be empty.** A unit's manifest directory can
   hold only `ridl.toml` and `interfaces.lock`, with every source file in a
   subdirectory. The unit name is still the prefix of every source package and
   the catalog name, and interface names are qualified from it.

4. **The IR records the unit.** `v2::Package` gets a `unit` field, filled by the
   compiler and carried in the baseline snapshots (see "The unit in the IR").
   Deriving the unit from a name prefix was rejected: it fails when one unit
   name is a prefix of another, and `ridl diff` has no manifest for its old
   side.
5. **The vss corpus is re-cut.** `evals/corpus/vss` nests workspace members on
   purpose, to mirror the VSS tree (for example member `Station` inside
   `Vehicle/Cabin/HVAC`). Under rules 1 and 2 that is an error. Each member
   moves to its own top-level directory and keeps its package name. The
   calibration records are regenerated, and `evals/corpus/vss/PROVENANCE.md`
   records the new layout. Relaxing rule 2 for workspace members was rejected:
   `vehicle.cabin.hvac.station` would then belong to two units, which is an
   error under rule 3.
6. **A unit has one identity.** A change in any source package of a unit changes
   the unit's catalog hash. This matches the release model: a unit is versioned
   and released as a whole, and releasing one unit never changes another unit's
   hash. Types that a unit reaches in another unit are part of its hash, as
   today, because they are part of its wire contract. A unit whose parts change
   at different rates is split into two units.

## Related

- driftsys/ridl#700 (`ridl diff` misses a lock change) touches the same lock and
  diff code.
