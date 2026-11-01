# The deployment descriptor — the system half of the runtime descriptors

Working design for driftsys/ridl#1176. It builds the second artifact of
[the runtime descriptors design](../archive/2026-09-13-runtime-descriptors-design.md)
(D-1, D-5, D-9), which
[the catalog descriptor record](../design/catalog-descriptor.md) lists under
"Not built", and which driftsys/ridl#715 left unbuilt "until a runtime that
reads one exists". Such a runtime now exists outside this repository.

The owner asked on 2026-11-01 for the design and the implementation to start
without a round of questions, so every choice below is a ruling taken under
delegation, numbered R-1 to R-n, each with its reason, for the owner to check
while reading. "Alternatives considered" follows each ruling that had a credible
alternative.

## 1. The driving use case

A shared-memory runtime has a node (a daemon) and clients (the component
processes). A client attaches with one catalog descriptor and an integer that
identifies it in the deployment. The node must decide, at attach, whether that
client may write an interface (it offers the interface's service) and whether it
may map a catalog (one of its `requires` lines reaches it). Today the program
writes the integer by hand, and the node has no record of who offers and who
requires what: that knowledge is in the rsdl sources and in the lowered `System`
of the IR, which the runtime does not read. Its node builds its table of
"interface number to owning client" by hand, in code, and admits every client to
every catalog, because no released artifact carries either table.

The runtime asked for component and service ids in the catalog descriptor and in
the generated interface code. Neither is the right place:

- A catalog is the contract of one unit. Its hash covers the interface shapes
  and no service ([catalog-descriptor.md](../design/catalog-descriptor.md), "The
  catalog hash"). The same catalog and the same generated module are used by
  every system that uses those interfaces. A component id in either ties a unit
  to one system, and a change of wiring changes the catalog hash.
- The identity belongs to an instance placed in a deployment, not to a
  component: one component may have several instances.

The facts the node needs are the deployment's. This design emits them in one
file per deployment.

## 2. What already exists

- `ridl_ir::v2::System` (`crates/ridl-ir/proto/ridl/ir/v2/system.proto`,
  ADR-0022) carries every fact of rsdl §13: the closure (components with their
  instances, offers and requires, producers, grants, regions with their catalog
  hashes) and one `Deployment` per deployment (machines, placements, links,
  routes, surface). Each node carries its backend keys as declared.
- The codegen request's deployment section
  (`crates/ridl-ir/proto/ridl/codegen/v1/deployment.proto`,
  [codegen-plugins.md](../design/codegen-plugins.md) §7) is an emitter over
  `System` for a plugin. A runtime node does not run a plugin and cannot read
  it.
- `ridl-descriptor` writes and verifies the catalog descriptor, in FlatBuffers,
  with a hand-written append-only schema and generated planus accessors, with a
  `no_std` reader half.
- rsdl backend keys (rsdl §5): `backend.key` on any declaration and any line,
  the placement line included, carried as declared and never interpreted. The
  runtime descriptors design (D-5) already names this route for a transport's
  service number (`someip.serviceId`).

## 3. Rulings

### R-1 The artifact is per deployment and is called the deployment descriptor

One file per lowered deployment: `<package>.<Name>.deployment.binfb`, where
`package` and `Name` are the `deployment` declaration's package and name. The
FlatBuffers file identifier is `RDLD`. The emit value is `deployment`.

Reason: the runtime descriptors design left the identifier and the extension
open. The lowered IR already uses `<pkg.Name>.system.{json,txtpb,binpb}` for the
whole `System`; a `.system.binfb` would differ from the IR artifact by its last
extension only and name a different thing (one deployment, not the system). The
deployment's own qualified name is unique in a workspace, because rsdl rejects a
duplicate declaration name in a package.

Alternatives considered: `RDLS` and `<pkg.System>.<deployment>.system.binfb`
(rejected: the suffix collides in form with the IR artifact); one file per
system holding every deployment (rejected by the archived D-1: a node reads one
deployment).

### R-2 The descriptor is self-contained: it embeds the catalogs it reaches

As the archived D-1 decided: the descriptor embeds, for every region of the
closure, the catalog descriptor bytes that `ridl_descriptor::lower` writes for
that unit in the same build, as a nested buffer (`[ubyte]` with the
`nested_flatbuffer: "Catalog"` attribute). The region's 32-byte hash is stored
beside it. The lowering checks that the hash of each embedded catalog is the
region's hash; a mismatch is an internal error (exit 2), because both come from
one build.

Reason: a node configures a deployment from one file, and the embedded bytes are
byte-identical to the standalone `.catalog.binfb` a client `include_bytes!`es,
so the two sides compare one hash.

### R-3 Contents

The root table `Deployment` carries, in this order (field ids are append-only,
D-3 of the archived design):

1. `version` (`SCHEMA_VERSION` of this schema, 1), `system` (qualified name),
   `name` and `package` of the deployment.
2. `catalogs`: one `EmbeddedCatalog { name, hash, bytes }` per region of the
   closure, in catalog name order.
3. `interfaces`: one
   `DeployedInterface { catalog, number, name, inline,
   provisional, service }`
   per interface of a region, in (catalog, number) order — the region map
   flattened, with the owning service.
4. `machines`: `Machine { name, external, labels, attributes }`, in declaration
   order.
5. `components`:
   `Component { name, implicit, external, instances, labels,
   attributes }`,
   in closure order.
6. `instances`: one
   `Instance { component, instance, machine, external,
   attributes, offers, maps }`
   per placement, in placement order. `attributes` are the placement line's
   backend keys. `offers` lists the interface keys (catalog, number) whose slots
   the instance writes, read from the producers; `maps` lists the catalog names
   its component's `requires` lines reach, read from the grant. These are the
   two lists the codegen deployment section already computes, and the lowering
   uses the same rule.
7. `links`:
   `Link { catalog, number, service, consumer, producer, crossing,
   attributes }`,
   in IR link order; `consumer` and `producer` are
   `Endpoint { component, instance, machine }`, and `attributes` are the
   `requires` line's backend keys (the line is the link's attribute site, rsdl
   §13).
8. `routes`:
   `Route { catalog, number, ordinal, interface, member, service,
   producers }`,
   in the IR's (catalog, number, ordinal) order.
9. `surface`: `Surface { link, direction }`, where `link` is the index of the
   link in `links`.
10. `attributes`: the system's and the deployment's backend keys, as two lists
    `system_attributes` and `attributes`.

An attribute is `Attribute { namespace, key, value }`, where `value` is absent
for a flag and otherwise an `AttributeValue { scalar, list }` table with exactly
one of the two set, as in the IR.

Not contained, as in the archived D-5 and D-7: transport choice per crossing,
fabric, envelope and framing overhead, doc comments, `@since`, `deprecated`,
distributions, and the sizing values. The sizing values (declared `depth`,
`slots`, `budget` and the derived values) are left out of version 1 because the
codegen deployment section is the record that resolves their precedence; adding
them is an append when a runtime reads them (R-9).

Reason: these are the archived D-5's contents, restricted to what the IR already
lowers. Each list is a direct projection of a `System` field, so the lowering
adds no rule of its own beyond the two lists of item 6.

### R-4 No instance number in ridl; a runtime reads its own backend key

The toolchain assigns no number to a component, an instance or a service. A
runtime that identifies an instance by an integer declares it as a backend key
of its namespace on the placement line, for example

```rsdl,ignore
deployment Vehicle for VehicleSystem {
  machine Hpc {
    VehicleStateProvider [ shm.service = 3 ]
    UiModel [ shm.service = 4 ]
  }
}
```

and reads the key from the instance's `attributes` in the descriptor. The
toolchain carries the key as declared and does not interpret it (rsdl §5).

Reason: the language already has the site and the carrier, and the runtime
descriptors design chose this route for a transport's service number. A number
is a fact of one runtime's wire, which rsdl §1.3 keeps out of the toolchain's
identities.

Alternatives considered:

- A lock file of instance numbers, as `interfaces.lock` numbers interfaces.
  Rejected for version 1: it adds a second lock, its resolver and its baseline
  rules for one runtime's integer, and the backend key gives the same stability
  (the number is written once in source and never derived).
- An rsdl-owned key (`id = 3`). Rejected: a language surface change for one
  runtime's need, against the family rule that a backend's fact is a backend
  key.
- A number derived from placement order. Rejected: it changes when a source is
  reordered.

A runtime namespace that no configured backend claims draws RSDL-804, a warning,
and the key is carried. That warning is accepted for version 1; a way for a
manifest to claim a namespace for an external runtime is ADR-0022's open item 3
and is not built here.

### R-5 The codegen deployment section carries the placement's backend keys

`Instance` in `deployment.proto` gains `repeated Attribute attributes = 7`, the
placement line's backend keys, so that a plugin that generates a client's
bring-up code can emit the client's number from the same key the node reads. The
`Attribute` message is added to `deployment.proto` with the IR's shape. The
field is additive under the IR specification §6.

Reason: generated code should not hold a number the program writes by hand, and
a plugin is the place for a runtime's own code (ADR-0020).

### R-6 No deployment module in the Rust backend

The Rust backend emits nothing new. A generated interface crate stays
independent of every deployment.

Reason: a deployment module in the generated crate would bind the crate to one
deployment, which is the problem of section 1 in another place. A runtime that
wants generated identity code writes a plugin over the deployment section (R-5).

Alternatives considered: a `deployment` module per deployment in the generated
crate, with each instance's names and attributes as constants. Rejected for the
reason above; it can be reconsidered as an opt-in option when a second runtime
asks for it.

### R-7 Where the code lives

- Schema `crates/ridl-descriptor/schema/deployment.fbs`, its own namespace
  `ridl.deployment`, its own file identifier. `cargo xtask descriptor-codegen`
  generates its accessors into
  `crates/ridl-descriptor/src/deployment/generated.rs` beside the catalog's, and
  the staleness test covers both.
- `ridl_descriptor::deployment`: `finish`, `verify` (the `no_std` half) and,
  under `std`, `lower(system, deployment_name, packages, catalogs)`, which takes
  each instance's `offers` and `maps` from `ridl_ir::codegen::lower_deployment`
  so that the two emitters cannot disagree, and `to_json`.
- `ridlc`: `Emit::Deployment`, written from the lowered system after the
  catalogs, one file per deployment in `System.deployments`. A deployment that
  an RSDL-7xx error blocked is absent from the IR and gets no file (ADR-0022
  decision 8). In single-file mode, and in a build with no system, the emit
  writes nothing.
- `ridl describe` reads either descriptor, choosing by the file identifier, and
  for a deployment descriptor also verifies each embedded catalog and checks its
  hash against the stored one; a mismatch exits 2 with the catalog named.

Reason: the catalog descriptor's crate already owns the FlatBuffers dependency
and the planus workaround (`finish`), and the archived design placed both
schemas there.

### R-8 Verification

`verify` checks the identifier and the version and walks every field, as the
catalog verifier does, and verifies each embedded catalog with the catalog
verifier. A reader calls it before the first access.

### R-9 Evolution

The schema follows the catalog schema's rule: a field is only appended at the
end of its table, never removed, reordered or retyped; an enum member is never
renumbered. `SCHEMA_VERSION` of the deployment schema is 1.

## 4. Testing

- Round trip and the planus header behaviour, as for the catalog.
- The verifier: short, foreign identifier, wrong version, truncated, a flipped
  byte inside an embedded catalog.
- The lowering, over `examples/cabin` and
  `crates/ridlc/tests/corpus/rsdl-appendix-a` (each declares a system and
  deployments): the instances' `offers` and `maps` equal the deployment
  section's for the same deployment (one test asserts the two emitters agree),
  the embedded catalogs equal the standalone files byte for byte, a placement
  backend key is carried to the instance, and two builds write the same bytes.
- `ridlc` and `ridl build`: `--emit deployment` writes one file per deployment,
  none for a blocked deployment, none in single-file mode.
- `ridl describe` on a deployment descriptor, and its hash mismatch exit.
- The deployment section: the placement's backend keys reach
  `Instance.attributes`.

## 5. Records to amend

- ADR-0022: an amendment under decision 11 adding the deployment descriptor as
  the second emitter over `System`.
- `docs/design/catalog-descriptor.md`: the "Not built" entry becomes a pointer
  to the new record.
- A new record `docs/design/deployment-descriptor.md`, gardened from this file.
- `docs/design/codegen-plugins.md` §7: `Instance.attributes`.
- `docs/technotes/rsdl-implementation.md`, "What is not built yet".
- The book's CLI pages that list the `--emit` values and `ridl describe`.
- The freeze dumps the new emit value and the protocol field change
  (`cargo xtask freeze update`).

## 6. Stages

1. Schema, generated accessors, `finish`, `verify` (`ridl-descriptor`).
2. The lowering and `to_json`.
3. `--emit deployment` in `ridlc` and `ridl build`, and `ridl describe`.
4. `Instance.attributes` in the codegen deployment section.
5. Records and gardening.
