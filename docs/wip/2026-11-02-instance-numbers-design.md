# Instance numbers — the design of MVP 0

Design for E19.0 (driftsys/ridl#1184) of Epic 19 in `docs/ROADMAP.md`. It builds
on the decisions recorded with the owner in
[the instance identity decisions](2026-11-02-instance-identity-decisions.md),
and it covers E19.2, E19.3, E19.5 and E19.6. The work lands on PR
driftsys/ridl#1179, after the deployment descriptor (E19.1) that the PR already
holds.

## Intent

A shared-memory runtime has one node per machine and many clients. At attach, a
client states which instance it is, and the node checks that identity against
the deployment descriptor. Today the descriptor carries no number for an
instance, and the design record tells a runtime to declare one as a backend key
on the placement line (`VehicleStateProvider [ shm.service = 3 ]`). The owner
rejected that on 2026-11-02: a number typed by hand is an agreement kept outside
the toolchain, and the toolchain must give the number.

MVP 0 makes ridl assign every declared instance a number, lock it, carry it in
the artifacts a runtime reads, and generate it as a constant that the client
program links.

## Goals

- Every instance of every component that a workspace's systems use has a
  positive number, from 1 to 2^31 − 1.
- The number is stable: reordering, moving or renaming files does not change it,
  and a removed instance's number is never given to another instance.
- The number is the same in every system and every deployment of the workspace.
- The deployment descriptor, the codegen deployment section and the generated
  Rust code carry the same number for the same instance.
- `ridl diff` reports a changed or a reused instance number as breaking.

## Non-goals

- Component numbers and service numbers (decisions 1 and 5 of the decisions
  note; E19.11 for services).
- A deployment descriptor per machine (E19.4).
- The debug twin −N and the listener `i32::MIN` in the runtime (E19.7). This
  design only reserves the negative range.
- Constants for Kotlin and TypeScript (E19.13).
- Any check in a runtime node (E19.9).

## Anti-goals

- A number written in rsdl source. rsdl stays free of runtime identities (rsdl
  §1.3), and instances stay named.
- A number derived from declaration order or from placement order.
- An instance number in a catalog, in a catalog hash, or in the generated
  interface code of a unit. The catalog is shared by every system.
- An instance number in a data frame. The number is sent at attach only, so
  instances stay indistinguishable on the wire (rsdl §7, V-02).

## Constraints

- The interface lock is the model: `crates/ridl-core/src/interface_lock.rs`
  (format, `allocate`, `retire`, `rename`, `merge`), `ridl lock`
  (`crates/ridl/src/lock.rs`), and the numbering in
  `crates/ridl-sem/src/check.rs` (`number_unit`). Its rules are reused, not
  re-invented: `next N` never lowered, `retired` entries never removed, only
  plain `ridl lock` allocates, an entry with no declaration is an error that
  names `--retire` or `--rename`, a missing lock is not an error and makes every
  number provisional.
- Field additions to the IR, the codegen section and the descriptor are
  additive: appended proto fields, and FlatBuffers fields appended at the end of
  the table (IR specification §6; `deployment.fbs` uses declaration order for
  its field ids).
- A remote (dependency) unit is read-only: `ridl lock` never writes into it
  (ADR-0002).

## The design

### 1. The instance lock

**One file per workspace, at the workspace root: `instances.lock`.** The
interface lock is one per unit, because an interface number is scoped by its
catalog. An instance number is not scoped by anything: a node compares it
against every instance of a deployment, and a deployment can place components of
several units. Numbers must therefore be unique across everything a workspace's
systems place, so one file numbers them all. It sits beside the root
`ridl.toml`, as `ridl.lock` does. A bare source file with no manifest reads and
writes `instances.lock` in the file's own directory, as it does
`interfaces.lock`.

**The root workspace numbers every instance, including the instances of a
component declared in a dependency unit.** The workspace that builds a system is
the one that deploys it; the dependency's own workspace may deploy the same
component in another product with other numbers. This keeps the read-only rule
for a remote unit: nothing is written into the dependency.

**Key.** `<component qualified name>/<instance name>`, one token:

```text
# instances.lock — written by ridl lock; do not edit by hand.
next 4
veh.adas.Cruise/primary 1
veh.adas.Cruise/backup 2
veh.ego.status/Unit 3
```

- The component's qualified name is `package.Name` for a declared component, or
  the dotted service name for the implicit component of a lone service, as the
  IR's `Component` already carries it.
- The instance name is the declared camelCase name, or `Unit` for the unit
  instance. `Unit` cannot collide with a declared name (RSDL-307).
- `/` appears in neither part, so the key splits without ambiguity.

**Which instances are numbered.** Every instance of every component in the
closure of every `system` in the root workspace, deployed or not. A component
that no system uses gets no number, because no runtime can see it.

**Allocation.** The same rules as the interface lock:

- an instance with a live entry has that number, not provisional;
- every other instance is provisional and takes a number from `next` upward,
  ordered by the key's bytes, so a file move does not change the order;
- plain `ridl lock` allocates the provisional numbers and writes the file;
- `ridl lock --retire KEY` marks an entry `retired`; the number is never given
  again;
- `ridl lock --rename OLD=NEW` keeps the number when a component or an instance
  is renamed;
- a live entry with no instance is an error (a new rsdl code, the analogue of
  RIDL-409 for interfaces), whose help names `--retire` and `--rename`; a
  malformed file is an error (the analogue of RIDL-410);
- `ridl lock merge`, the git merge driver, handles `instances.lock` with the
  same rules as `interfaces.lock`.

`ridl lock` already compiles the workspace before it writes; it writes the
interface locks of the local units and the instance lock of the root in one run.
`--retire` and `--rename` accept either kind of key; a key containing `/` is an
instance key.

**Range.** 1 to 2^31 − 1. 0 is not a valid number. Negative numbers are reserved
for instances that a runtime defines in a debug build (E19.7). An allocation
that would pass 2^31 − 1 is an error.

### 2. The number in the artifacts

**IR, the system artifact** (`system.proto`). `Component` gains:

```proto
message InstanceNumber {
  string instance = 1;   // the instance name, Unit for the unit instance
  int32 number = 2;      // 1 .. 2^31 − 1
  bool provisional = 3;  // no live entry in instances.lock
}
// on Component, appended:
repeated InstanceNumber numbers = <next free field number>;
```

One entry per name in `Component.instances`, in the same order. The lowering in
`crates/ridl-sem/src/rsdl/lower.rs` fills it from the numbering of section 1.

**Codegen deployment section** (`deployment.proto`, `Instance`): appended
`int32 number = 8; bool provisional = 9;`.

**Deployment descriptor** (`deployment.fbs`, `Instance`): appended
`number: int32; provisional: bool;` at the end of the table. The JSON view of
`ridl describe` prints both. `SCHEMA_VERSION` stays 1, because the artifact has
not been released; the descriptor's verifier refuses a number that is 0 or
negative.

`lower_deployment` and `ridl_descriptor::deployment::lower` both read the number
from the IR, so the two cannot disagree; the existing test that compares their
`offers` and `maps` also compares `number` and `provisional`.

**The placement key is removed.** The section "Identifying an instance" of
`docs/design/deployment-descriptor.md` is rewritten to describe the lock, and
the tests that build an `shm.service` key are changed to read the number. A
backend key on a placement line stays legal and is still carried as declared; it
is no longer the documented way to identify an instance.

### 3. The Rust constants

The Rust backend emits, in the crate that `ridl build` writes for the root
workspace, one constant per numbered instance:

```rust
pub mod instances {
    pub mod veh {
        pub mod adas {
            pub mod cruise {
                /// `veh.adas.Cruise/primary` in `instances.lock`.
                pub const PRIMARY: i32 = 1;
                pub const BACKUP: i32 = 2;
            }
        }
        pub mod ego {
            pub mod status {
                pub const UNIT: i32 = 3;
            }
        }
    }
}
```

- The module path is the component's qualified name, each segment through the
  pinned name transform (ADR-0016, `module_segment`); the constant name is the
  instance name in SCREAMING_SNAKE_CASE, `UNIT` for the unit instance.
- The constants depend on no deployment: they are the same in every build of the
  workspace, so a program links its own constant whichever deployment it is
  placed in.
- A provisional number is emitted with a doc line saying so, so that a reader of
  the generated code sees it.
- The interface modules of the crate are unchanged, and no catalog hash changes.
- The type is `i32`, the type of the attach field. A newtype is left to the
  runtime work of E19.7.

### 4. `ridl diff`

The system diff (`crates/ridl-diff/src/system.rs`) gains one heading, **instance
number changed**, for an instance whose number differs between the two sides,
and for a number that one side gives to a different instance than the other
side. It is the one system heading with a verdict: **breaking**, because a
client built against the old number is refused, or worse, accepted as another
instance. A provisional number that becomes locked with the same value is not
reported, as `InterfaceFrozen` is compatible on the contract path.

### 5. The records

- rsdl reference §7: "Instances are named, never numbered" becomes "named in
  source; the toolchain numbers them" with the lock rules; V-02 gains the
  sentence that the number is sent at attach only; the range and the reserved
  negative numbers are stated. §14 gains the new diff heading; §16.1 gains the
  new codes.
- ADR-0022 decision 12 is amended: the lock replaces the placement key.
- `docs/design/deployment-descriptor.md`: "Identifying an instance", the field
  table, and the decision "No number for an instance in ridl" (now an
  alternative considered and rejected by the owner).
- `docs/book/`: the CLI chapter entry for `ridl lock` names the instance lock.

## Settled after the builder check

A reviewer read this design as its builder. Each point below is a call of the
author, for the owner to check.

1. **The numbering is one workspace step, before lowering.** A function in
   `ridl-sem`, beside `number_unit`, takes the checked systems of the root
   workspace and the instance lock, and returns a map from key to
   `(number, provisional)`. Provisional numbers depend on every other
   provisional key, so they are computed once for the whole workspace and not
   per system. `lower_system` takes the map as a new parameter; `ridl lock` and
   `ridl build` call the step.
2. **`ridl diff` reports only a locked number.** "instance number changed" is
   reported when the old side's number is not provisional and the new side gives
   that key another number, or gives that number to another key. A provisional
   number on the old side is never reported, because it was never a promise.
3. **A component rename moves all its instances.** `--rename OLD=NEW` without
   `/` renames a component: every live and retired `OLD/*` entry becomes `NEW/*`
   with its number. With `/` it renames one instance. `--retire` with a
   component name retires all its instances. The error for a live entry with no
   instance is drawn once per component whose entries are all orphaned, with the
   component rename as its help, and once per entry otherwise.
4. **The constants come from the system artifact and are emitted by
   `ridl-backend-rust`.** The backend gains a function that renders the
   `instances` module from the numbering map (the union over every system of the
   workspace); `ridlc` passes it and adds the module to `lib.rs`. A workspace
   with no `system` gets no `instances` module. The codegen deployment section
   is not used for the constants, because it is per deployment.
5. **The key's grammar.** Exactly one `/`. Left: the component's qualified name,
   a dotted path whose last segment is the CamelCase component name, or for the
   implicit component of a lone service, the service's dotted name. Right: a
   camelCase instance name or `Unit`. The key is not scoped by system or by
   deployment, so two systems that use one component share its keys. A declared
   component's last segment is CamelCase and a service's segments are not, so
   the two kinds of key cannot collide; the builder confirms that rule against
   the rsdl reference, and the parser refuses any other form.
6. **The instances that are numbered** are those of every component that the
   composition of some `system` lists, explicitly or as the implicit component
   of a lone service. Wiring alone adds none, because every `requires` resolves
   to a component in the composition.
7. **A bare file** gets constants like a workspace, each with the provisional
   doc line until `ridl lock` writes its `instances.lock`.
8. **The merge driver** is registered for `instances.lock` wherever it is
   registered for `interfaces.lock`, in the same task as the lock.

## Alternatives considered

- **One instance lock per unit, beside `interfaces.lock`.** Rejected: two units
  would both start at 1, and a node that sees both in one deployment could not
  tell their instances apart. Scoping the number by unit would put the unit on
  the attach, which is a second identity.
- **The dependency's own lock numbers its components.** Rejected: the
  dependency's workspace may deploy the component elsewhere, its numbers can
  collide with the root's, and the root may not write into a remote unit.
- **A backend key typed on the placement line (R-4 of the descriptor design).**
  Rejected by the owner on 2026-11-02.
- **A number derived from declaration or placement order.** Rejected: it changes
  when a source is reordered.
- **The number keyed per system or per deployment.** Rejected by decision 6: one
  instance has one number in every build, so its constant does not depend on a
  deployment.
- **A `ridl_rt` newtype for the constant now.** Deferred to E19.7, which defines
  the debug twin on that type; MVP 0 needs only the integer.

## Left to the builder

- The exact field numbers of the appended proto fields, and the names of the two
  new diagnostic codes (taken from the next free rsdl codes).
- Whether `instances.lock` gets its own module in `ridl-core` or shares the
  interface lock's parser with a key type parameter.
- How the `instances` module is placed in `render_lib_rs` so that it never
  collides with a package module named `instances`.
- The test fixtures: a corpus workspace with a redundant component, a lone
  service and a component from a dependency unit.
