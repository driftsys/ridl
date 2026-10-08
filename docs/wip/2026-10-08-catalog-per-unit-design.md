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
   `MANI-` code.
3. **One source package, one unit.** Across every unit that one build loads,
   remote imports included, a source package belongs to exactly one unit. A
   second unit that declares the same source package is an error. This needs a
   new `MANI-` code. Rule 1 prevents this inside one workspace when unit names
   do not overlap. The check is still needed for a remote import, and for two
   unit names where one is a prefix of the other (`com.example` and
   `com.example.hmi`).
4. **What a unit produces is inferred from what it holds.** One unit can produce
   both artifacts:
   - interface shapes in any of its source packages give **one catalog**, named
     after the unit;
   - the `system` gives **the system artifact**;
   - a unit with only types produces no artifact of its own. Other units import
     its types, and their catalog hashes include the types they reach.
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
- **Interface names**: qualified by the source package relative to the unit
  name, for example `cluster.SpeedDisplay`. An interface in the root source
  package keeps its short name (`Session`). Today the name is always the short
  name, which is ambiguous when two source packages of one unit declare the same
  short name.
- **Numbers**: one numbering space per unit (see the next section).
- **Hash**: SHA-256 over the reduced unit: every interface shape of the unit
  with its number, and every declaration those shapes reach in any unit of the
  build. This amends ADR-0014 decision 15, which hashes a reduced package.

### `interfaces.lock`

- One `interfaces.lock` per unit, in the manifest directory. Today there is one
  per source package directory (ridl reference §11).
- Its keys are the qualified interface names of the catalog
  (`cluster.SpeedDisplay`, `service:climate.control` for an inline shape).
- `ridl lock [PATH]` writes the lock of the unit that `PATH` is in.
- The number stays 1-based and is now scoped per unit, not per source package.

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

| Area                     | Change                                                                                                                                                                                                                            |
| ------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `ridl-core` loader       | rule 2 and rule 3 diagnostics; a unit boundary visible to later stages                                                                                                                                                            |
| `ridl-sem`               | read one lock per unit; number per unit                                                                                                                                                                                           |
| `ridl-ir`                | the reduced unit for the catalog hash; ADR-0014 decision 15                                                                                                                                                                       |
| `ridl-descriptor`        | `lower` takes a unit, not a package; qualified interface names                                                                                                                                                                    |
| `ridlc` / `ridl`         | one catalog file per unit; `ridl lock` writes the unit's lock; `ridl diff` groups the per-package snapshots by unit (decision 1)                                                                                                  |
| `ridl-backend-rust`      | `CATALOG.name` is the unit name; `NUMBER` is the unit's number. Generated modules stay per source package                                                                                                                         |
| `ridl-rt`                | no API change. The doc comment of `CatalogRef` ("one package's interfaces") changes to the unit                                                                                                                                   |
| system artifact          | regions are per catalog, so there are fewer regions. No format change                                                                                                                                                             |
| codegen plugins (Kotlin) | no structural change: generated packages stay per source package. `Catalog.package` in the codegen model carries the unit name, and the scope of each number changes (decision 2). A heads-up issue is filed on the Kotlin plugin |
| `ridl-mcp`, `ridl-lsp`   | none expected beyond the new diagnostics                                                                                                                                                                                          |

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
   wire. If the Kotlin plugin uses `Catalog.package`, it needs only a new test
   snapshot. The heads-up issue asks its maintainers to confirm this.
3. **The root source package can be empty.** A unit's manifest directory can
   hold only `ridl.toml` and `interfaces.lock`, with every source file in a
   subdirectory. The unit name is still the prefix of every source package and
   the catalog name, and interface names are qualified from it.

## Related

- driftsys/ridl#700 (`ridl diff` misses a lock change) touches the same lock and
  diff code.
