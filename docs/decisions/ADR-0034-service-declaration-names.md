# ADR-0034 — Service declaration names

## Status

_Accepted_ — 2026-11-18 (proposed 2026-11-14). Written from the owner's proposal
on driftsys/ridl#1390 and its two comments, and from the decisions the owner
delegated for that issue. The documents in "Documents amended" are changed with
the acceptance of this record, in the pull request that builds it.

Decision 6 was amended on 2026-11-18 by the owner's ruling on the review of the
pull request that builds this record: two directories of one unit that give one
package name (`a.b/` and `a/b/`) are refused when the unit loads (MANI-023), so
two packages of one name cannot load and the premise of the retirement of
RIDL-140 holds.

Scope: the name a `service` declaration carries, the namespace that name lives
in, how every other declaration refers to a service, the diagnostic for the
dotted form, the `interfaces.lock` key of an inline shape, and every generated
name and descriptor value derived from a service name. It binds the ridl and
rsdl references, the parser, the checker, the IR lowering, `ridl lock`, every
backend, the descriptors, the language server and the MCP server.

Amends [ADR-0015](ADR-0015-qos-absorption-and-rpc-bounds.md) decisions 12 and 14
(the grammar of `ServiceDef`) and decisions 15 and 16 (the `service:` key and
the member address), ridl reference §14.5 and Appendix C, and records the code
that [ADR-0008](ADR-0008-e2-execution.md) decision 21 reserved.

This is a breaking change of the language. It ships in a pre-1.0 minor release
(decision 10).

## Context

Today a service is declared with a dotted, reverse-domain global name, and every
segment of that name must be lowercase:

```ridl,ignore
package com.example.safety.alerts

service com.example.safety.alerts.telltales : SafetyTelltaleStatus
```

The checker reports a segment that is not lowercase with an error that has no
code (`check_service_name` in `crates/ridl-sem/src/check.rs`):

```text
error: service name segment `SafetyAlertRendering` is not lowercase — a service has a dotted, reverse-domain global name
```

The issue records five problems with this rule:

- Interfaces and components have `CamelCase` names, and users expect a service
  to have one too. `service SafetyAlertRendering : …` is the form they write
  first.
- The reference does not say why a service name must be lowercase. The keyword
  `service` and the keyword `offers` already say that the name is a service, so
  case is not needed to tell a service from an interface or a component.
- The error has no code, so it cannot be looked up, documented or configured.
  ADR-0008 decision 21 reserved RIDL-142 for it on 2026-07-25, and the code was
  never minted.
- A lowercase service `widgets` and an interface `Widgets` can be declared side
  by side. Two names that differ only in case are a hazard for tools and for
  file systems that ignore case.
- The grammar production is `dotted_name = camelCase_id { "." camelCase_id }`,
  with a comment that says every segment is lowercase. The production's name and
  its comment disagree.

The source does not need a dotted name for any other reason. The dotted name is
the only key of the workspace service catalog (`service_catalog` in
`crates/ridl-core/src/package.rs`), and the catalog exists because the dotted
name is not in any package's namespace: it is the only place where two equal
service names can be detected, which is RIDL-140. A name scoped by its package
is unique when package names are unique (a source package belongs to one unit,
MANI-014), and a second declaration of the same name in one package is already
TYPL-009.

## Decision

1. **A service is declared with one `CamelCase` name.** The name after `service`
   is one `CamelCase_id` (typl Appendix E: an upper-case ASCII letter, then
   ASCII letters and digits), the rule the grammar states for an interface name.
   The service's **global name** is its package followed by the declared name,
   joined by `.`. Both forms use it:

   ```ridl,ignore
   package com.example.safety.alerts

   /// Safety telltale state provided to the alert monitor.
   service SafetyTelltales : SafetyTelltaleStatus
   ```

   ```ridl,ignore
   package com.example.hmi.climate

   service CabinClimate {
     signal  temperature : Temperature @[1s..10s]
     command setTarget(t: Temperature)
   }
   ```

   The global names are `com.example.safety.alerts.SafetyTelltales` and
   `com.example.hmi.climate.CabinClimate`. A declaration uses either an
   interface list or an inline shape, never both (ADR-0015 decision 14,
   unchanged).

   The checker enforces `CamelCase_id` for a service name, which it does not do
   for an interface name. The reason is that rsdl reads the role of a reference
   from its case (rsdl §4): a service whose name is not `CamelCase_id` could be
   declared but could not be named in an `offers` line.

2. **A service is a symbol of its package.** The declared name is in the
   package's declaration namespace, beside the package's types, constants,
   interfaces and rsdl declarations. The resolver binds it as a new
   `SymbolKind::Service`. A second declaration of the same name in the package,
   of any kind and in any profile, is **TYPL-009**; there is no new code for the
   clash. A service and an interface of one package therefore cannot share a
   name: a service that publishes an interface `Cabin` is named differently, for
   example `CabinService` or `FrontCabin`.

   A service is not a type. Written in a type position, it draws the error an
   interface draws there (ridl §14.0).

3. **A service is referred to as an interface is.** A reference to a service is
   a path: the bare name in its own package, the bare name after an `import`
   (`import com.example.safety.alerts.SafetyTelltales`), or the fully qualified
   global name. The places that take a service name are:

   | Place                                                     | What it takes                                                                      |
   | --------------------------------------------------------- | ---------------------------------------------------------------------------------- |
   | rsdl `offers` line (rsdl §3.2)                            | one service reference                                                              |
   | rsdl `requires` line (rsdl §3.2)                          | an interface, or a service with an inline shape                                    |
   | rsdl member line of `system`, `distribution` (§3.1, §3.3) | a component, or a lone service (rsdl §6)                                           |
   | rsdl placement line of `machine` (rsdl §3.5, §9)          | a component, one instance, or a lone service                                       |
   | `import` in any profile                                   | a service, like any declaration                                                    |
   | a doc link and an `@see` target (typl §14.5)              | a service, or a member of one: `[SafetyTelltales]`, `[SafetyTelltales.snapshot]`   |
   | `ridl lock --rename`, `--retire`                          | the lock key `service:<global name>`, and an implicit component by its global name |
   | `ridl describe`                                           | prints the service strings of a descriptor; takes a file path, not a name          |
   | MCP `ridl_resolve`                                        | `name`: a service as `Name` or `pkg.Name`, resolved to kind `service`              |
   | MCP `ridl_list_interactions`                              | `interface`: an inline-shape service as `Name` or `pkg.Name`                       |
   | MCP `ridl_describe_type`                                  | a service is refused, as an interface is                                           |
   | language server                                           | hover, go to definition, references, rename and completion on every place above    |

   MCP `ridl_references` is not changed by this record: given a service, it
   reports no new kind of use, and the components that offer a service are not
   among its results. Reporting them is left to a later change.

   In a doc link, the members of a service are the interactions of its inline
   shape, or the interactions of every interface of its list (flat addressing,
   ADR-0015 decision 16).

4. **A member is addressed `<package>.<Name>.<member>`.** Inside the package and
   after an import the address is `<Name>.<member>`. Addressing stays flat
   (ADR-0015 decision 16): no interface name is written between the service and
   the member, and RIDL-144 still refuses a member name that two interfaces of
   one service share.

   The IR keeps the **global name** in `Service.name`, as it keeps the dotted
   name today, so the catalog, the lock key, the descriptors and the codegen
   model keep one key for a service. A consumer that needs the declared name
   compares the global name with the package name joined to a declared name; it
   never splits an address at its last `.`.

5. **The dotted form is reported as RIDL-142.** The parser still accepts a
   dotted name after `service`, so the checker can report it. A declared name
   that is not one `CamelCase_id` is **RIDL-142**, an error: a name of two or
   more segments, and a name of one segment that is not `CamelCase_id`
   (`service doors`, `service cabin_2`). RIDL-142 is the code ADR-0008 decision
   21 reserved for the uncoded lowercase error, and it replaces that error. The
   one-segment case is included because a one-segment lowercase name was a valid
   global name before this change, and reading it as a declared name would move
   its global name without a diagnostic.

   The diagnostic suggests a name, and carries it as a fix-it, which the
   language server offers as a quick fix. The suggested name is computed from
   the written segments and the package name:

   1. Drop the longest run of leading segments that equal the leading segments
      of the package name. When nothing is left, keep the last segment.
   2. Split each remaining segment at `_`, write each part with its first letter
      in upper case, and join every part. When the result does not start with an
      upper-case letter (a part starts with a digit, or nothing is left), put
      `Service` before it.
   3. The result is taken when the package already binds it, or when the
      suggestion for an earlier RIDL-142 declaration of the same package has it.
      The package binds the names of its declarations, the names its imports
      bind (the alias, or the imported name when there is no alias), and the
      `ridl.std` names every package sees without an import (`Message`). So
      `service veh.hvac.cabin : Cabin` beside `import veh.common.Cabin` is told
      to become `CabinService`, not `Cabin`, which would refer to itself.
      Earlier means before in the package's file order, the order the checker
      reads its files, and in source order within a file. A declaration that
      draws RIDL-142 binds nothing, so without the second clause two old-form
      services, such as `veh.cabin.control` and `control` in package
      `veh.cabin`, would both be told to become `Control`.
   4. While the result is taken, try in turn the result followed by `Service`,
      `Service2`, `Service3` and so on, and keep the first that is not taken.
      The rule always gives a name, so every RIDL-142 carries a fix-it.

   | Package         | Written                                      | Suggested                        | Global name                                     |
   | --------------- | -------------------------------------------- | -------------------------------- | ----------------------------------------------- |
   | `fleet.vehicle` | `fleet.vehicle.doors.front`                  | `DoorsFront`                     | `fleet.vehicle.DoorsFront`                      |
   | `veh.cabin`     | `veh.cabin.control`                          | `Control`                        | `veh.cabin.Control`                             |
   | `veh.cluster`   | `veh.hvac.cabin`                             | `HvacCabin`                      | `veh.cluster.HvacCabin`                         |
   | `p`             | `doors`                                      | `Doors`                          | `p.Doors`                                       |
   | `alpha`         | `alpha.face`, with `interface Face` declared | `FaceService`                    | `alpha.FaceService`                             |
   | `veh.cabin`     | `veh.cabin.control`, then `control`          | `Control`, then `ControlService` | `veh.cabin.Control`, `veh.cabin.ControlService` |

   A declaration that draws RIDL-142 binds nothing: no symbol, no catalog entry
   and no IR service. A reference that still uses the old dotted name — a
   reference of lowercase segments only — draws the unknown-name code of its
   slot (RSDL-310, RSDL-312, RSDL-602, RSDL-702 or RSDL-903), and the message
   says that a service is named by its `CamelCase` name. There is no alias and
   no migration command; the fix-it and these messages are the migration aid.

6. **RIDL-140 is retired.** With decision 2, two services can have one global
   name only if two packages have one name. Two packages of one name cannot
   load: MANI-014 refuses two units that declare one source package, and
   MANI-023 refuses two directories of one unit that give one package name
   (`a.b/` and `a/b/`), for every declaration kind; the second directory is not
   loaded. Both are raised when the unit loads, before checking. Within one
   package the clash is TYPL-009. RIDL-140 can no longer be raised, so it leaves
   the catalogue and its number is kept reserved, as RIDL-146 to RIDL-148,
   RIDL-150 and RIDL-158 are (ADR-0008 decision 13). `service_catalog` keeps the
   global names as its keys and reports nothing.

7. **The lock keys follow the global name.** The `interfaces.lock` entry of an
   inline shape is keyed `service:` followed by the global name:
   `service:com.example.hmi.climate.CabinClimate`. The `instances.lock` entry of
   a lone service's implicit component is keyed by the global name followed by
   `/Unit`. Both are migrated with the commands `ridl lock` already has, after
   the source is changed:

   ```sh
   ridl lock <path> --rename service:veh.hvac.cabin=service:veh.cluster.HvacCabin
   ridl lock <path> --rename veh.ego.status=veh.ego.Status
   ```

   The first renames an interface entry; the second renames every instance of an
   implicit component. Each keeps its number, so no interface number and no
   instance number changes. No new migration command is added.

   `ridl lock` places two conditions on these edits (`crates/ridl/src/lock.rs`,
   `edit`), and the migration follows them: a `service:` rename is an interface
   edit, so `<path>` must resolve to exactly one unit — run it once per unit,
   with that unit's directory; an implicit component rename is an instance edit,
   so `<path>` must be the workspace root.

8. **Generated names and descriptor values derive from the global name.** Every
   output that derives from a service name today derives from the global name
   with the same transform, so a value changes exactly when the global name
   changes the transform's result:

   | Output                                                                  | Derived as                                        | Before (`veh.hvac.cabin` in `veh.cluster`) | After (`HvacCabin` in `veh.cluster`)            |
   | ----------------------------------------------------------------------- | ------------------------------------------------- | ------------------------------------------ | ----------------------------------------------- |
   | IR `Service.name`                                                       | the global name                                   | `veh.hvac.cabin`                           | `veh.cluster.HvacCabin`                         |
   | codegen model `Service.name`, inline `Interface.identity`               | `DottedName` of the global name                   | segments `veh`, `hvac`, `cabin`            | segments `veh`, `cluster`, `HvacCabin`          |
   | inline-shape identity name (`InterfaceShape.name`)                      | the global name                                   | `veh.hvac.cabin`                           | `veh.cluster.HvacCabin`                         |
   | fallible-return transport identity (ADR-0008 decision 4)                | `<identity name>#<ordinal>:<ok>\|<err>`           | `veh.hvac.cabin#14:…`                      | `veh.cluster.HvacCabin#14:…`                    |
   | proto3 ordinal enum of an inline shape                                  | `joined_camel` + `Ordinal`                        | `VehHvacCabinOrdinal`                      | `VehClusterHvacCabinOrdinal`                    |
   | FlatBuffers ordinal enum of an inline shape                             | `joined_camel` + `Ordinal`                        | `VehHvacCabinOrdinal`                      | `VehClusterHvacCabinOrdinal`                    |
   | induced parameter and reply types (Rust, proto3, FlatBuffers)           | `joined_camel` + member + role                    | `VehHvacCabinGetClimateParams`             | `VehClusterHvacCabinGetClimateParams`           |
   | podbuf names of those induced types (Rust codec, C header)              | the induced wire name + `PodbufHead`, `_head`, …  | `VehHvacCabinGetClimateParamsPodbufHead`   | `VehClusterHvacCabinGetClimateParamsPodbufHead` |
   | TypeScript doc comment of an induced tuple                              | the global name + `.` + member                    | `veh.hvac.cabin.getClimate`                | `veh.cluster.HvacCabin.getClimate`              |
   | catalog descriptor `Interface.name` of an inline shape                  | the global name                                   | `veh.hvac.cabin`                           | `veh.cluster.HvacCabin`                         |
   | catalog hash of a unit with an inline shape (ADR-0014 decision 15)      | SHA-256 over the shapes under their catalog names | the hash over `veh.hvac.cabin`             | a different hash over `veh.cluster.HvacCabin`   |
   | deployment descriptor `service` of a deployed interface, link and route | the global name                                   | `veh.cabin.control`                        | `veh.cabin.Control`                             |
   | implicit component name in the lowered system                           | the global name                                   | `veh.diag.access`                          | `veh.diag.Access`                               |
   | `interfaces.lock` key of an inline shape                                | `service:` + the global name                      | `service:veh.hvac.cabin`                   | `service:veh.cluster.HvacCabin`                 |
   | `ridl diff` path of a service                                           | `<package>/<global name>`                         | `veh.cluster/veh.hvac.cabin`               | `veh.cluster/veh.cluster.HvacCabin`             |

   `joined_camel` writes the first letter of each segment in upper case and
   joins the segments, so a service whose old dotted name began with its own
   package keeps every generated identifier: `veh.cluster.wheels` and
   `veh.cluster.Wheels` both give `VehClusterWheelsOrdinal`. That condition
   covers the generated identifiers only. Every renamed service changes its
   descriptor strings and its lock key, and every renamed inline shape changes
   the catalog hash input of its unit and its fallible-return identities,
   because they hold the global name as written: `veh.cluster.wheels` and
   `veh.cluster.Wheels` are different strings.

   Outputs that do not derive from a service name do not change: the Rust
   interaction face, the Rust descriptors and the TypeScript client classes
   (`CabinClient`) are emitted for declared interfaces only. The catalog hash of
   a unit without an inline shape does not change: it reads the declared
   interfaces under their catalog names and no service (ADR-0014 decision 15).

   The change of the fallible-return identity and of the generated names of an
   inline shape is breaking by design: the identity derives from the name the
   source gives the shape (ADR-0008 decision 4, ADR-0015 decision 15), and that
   name changes. The frozen surfaces this breaks are approved with the procedure
   of ADR-0029 decision 4: `diagnostics` (RIDL-140 leaves, RIDL-142 arrives),
   `accepted-sources` (the frozen copy of `examples/cabin`, in the dotted form,
   is no longer accepted, so its line `examples/cabin ok` is removed), `wire`
   and `generated-crate` (the inline shapes of the frozen `veh-cluster` copy).
   The step that applies is the two-step procedure of decision 4, which its
   bullet "0.11 starts with a pull request that breaks surfaces on purpose, with
   the same two steps" extends to a later pre-1.0 minor: the pull request that
   makes the break deletes each surface's row from `freeze/surfaces.toml`, keeps
   the name in `required`, carries `!` in its title, states the approval in its
   description and merges only on the maintainer's explicit approval; a
   follow-up pull request adds each row again with that merge commit as its
   `baseline`. The break is shown before the copies are refreshed, and the
   refreshed copies are what the follow-up baseline records.

9. **rsdl reads a `CamelCase` reference as a declaration of any kind.** The rsdl
   case table (rsdl §2) and the general form's rule R7 list `service` among the
   `CamelCase` roles; `lowercase.dotted` is a package only. In a reference, the
   leading lowercase segments are the package path and the `CamelCase` segment
   names a declaration of that package — a component, a service, or the system
   after `for`. Which one is decided by lookup, which is unambiguous because
   components and services share their package's namespace (decision 2). In
   `offers` the declaration must be a service; in a member or placement line, a
   component or a lone service; in `requires`, an interface or an inline-shape
   service. A bare name binds the declaration of the reference's own package
   first, whatever its kind, and an import only when the package declares no
   such name; a local service therefore shadows an imported component of the
   same name, as a local component shadows an import today.

10. **Release.** The change ships in a pre-1.0 minor release, never a patch. Its
    commit carries `!` and a `BREAKING CHANGE:` footer that states the
    migration: rename each service declaration as RIDL-142 suggests, update each
    `offers` line, each lone service in a `system`, `distribution` or `machine`
    body, each `requires` line naming an inline-shape service, and each doc link
    that names a service, then run the two `ridl lock --rename` commands of
    decision 7 — each `service:` rename once per unit with the unit's directory,
    each implicit component rename at the workspace root. It also states that
    `ridl diff` keys a service by `<package>/<global name>`, so a comparison
    against a baseline published before the migration reports every renamed
    service as removed and added, which is breaking, and that the workspace
    publishes a new baseline with `ridl baseline` after the migration. The
    book's chapter on stability states the same steps.

## Alternatives considered

- **Keep the lowercase dotted name, give the error a code and document the
  reason.** This is the smallest of the issue's steps. It leaves the name out of
  every package's namespace, so the catalog keeps its own uniqueness check and
  the language server and the MCP server keep special cases for a name that is
  not a symbol. It does not answer the first problem of the context, which is
  the one users report.
- **Make the lowercase rule a lint.** A lint lets a project choose its
  convention, but the dotted name would still be a global name outside the
  package namespace, so every problem except the missing code would remain.
- **Accept both forms, with the dotted name as an alias.** Two spellings of one
  service would keep the confusion the issue describes, and every addressing
  rule would have to state both. Rejected by the owner.
- **Keep the declared name in IR `Service.name`.** An interface's IR name is
  package-relative, and storing a service the same way would be uniform. It
  would also change every consumer that reads `Service.name` as a key — the
  catalog, the lock key, the descriptors, the codegen model and `ridl diff` —
  and decision 8's identities would derive from the declared name, which is not
  unique outside the package. Keeping the global name in the IR changes the
  value of the key and not its meaning.
- **A `ridl migrate` command.** The rename of a declaration is one fix-it, and
  the lock keys move with commands that exist. A command that rewrites rsdl
  references would have to resolve the old names, which this change removes.
  Rejected by the owner.
- **Keep RIDL-140 for a clash across packages.** No such clash can be loaded
  (decision 6), so the code would have no fixture that provokes it, and the
  corpus guard requires one for every code.

## Consequences

- Positive: a service is named, imported, referenced, hovered, renamed and
  linked like every other declaration. The service catalog loses its own
  uniqueness check. The uncoded error is gone, and RIDL-142 can be explained by
  `ridl explain`.
- Positive: the names that differ only in case are refused (TYPL-009), because a
  service and an interface of one package now share one namespace.
- Negative / accepted: every catalog that declares a service needs a one-time
  migration of its sources and its lock files. A peer that was built against an
  inline shape's old generated names, or that compares descriptor strings, is
  rebuilt with the new ones.
- Negative / accepted: a service and the interface it publishes can no longer
  share a name, which was common (`service veh.hvac.cabin : Cabin`).
- Negative / accepted: the catalog hash of a unit that declares an inline shape
  changes, because the shape is hashed under its catalog name, which is the
  global name. A peer or a tool that compares catalog hashes sees a new catalog
  for such a unit.
- Negative / accepted: `ridl diff` against a baseline published before the
  migration reports each renamed service as removed and added, which is breaking
  (`DeclRemoved` for the old key). The interface numbers are kept, so
  `ridl baseline` refuses nothing for them; the workspace publishes a new
  baseline after migrating.
- Neutral: interface numbers and instance numbers do not change, because the
  lock renames keep them. The catalog hash of a unit without an inline shape
  does not change.

## Documents amended

| Document                                                                            | Change                                                                                                     |
| ----------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------- |
| ridl reference §11, §14, §14.5, §14.6, §16.4, Appendix C, Appendix G                | the declared name, the global name, the namespace, the references, RIDL-142, RIDL-140 retired, the grammar |
| rsdl reference §2, §3, §4, §6, §7, §9, §13, §15, §16.3, §17, Appendix A, Appendix B | services named as declarations; the case table; open question 8 answered                                   |
| general form §2 Shape 3, R7                                                         | the `service` examples; R7 lists `service` among the `CamelCase` roles                                     |
| family overview §5, §6                                                              | decision ledger row; rsdl §17 item 8 answered                                                              |
| ADR-0015 decisions 12, 14, 15, 16                                                   | dated amendment notes                                                                                      |
| ADR-0008 decisions 6, 13, 21, Consequences                                          | dated note: RIDL-142 minted, RIDL-140 retired                                                              |
| the book: getting started, rsdl, CLI reference, documenting, stability              | the new form in every example; a migration section in the stability chapter                                |

## References

- driftsys/ridl#1390 — the issue and its two comments.
- [ADR-0002](ADR-0002-module-system.md) §1 — packages, units, MANI-014,
  MANI-023.
- [ADR-0008](ADR-0008-e2-execution.md) decisions 4, 13 and 21.
- [ADR-0015](ADR-0015-qos-absorption-and-rpc-bounds.md) decisions 12 to 16.
- [ADR-0016](ADR-0016-schema-projection-and-the-name-transform.md) — the name
  transform, which `joined_camel` sits beside and which this record does not
  change.
- [ADR-0029](ADR-0029-stability-policy.md) decision 4 — approving a break of a
  frozen surface.
