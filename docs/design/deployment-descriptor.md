# The deployment descriptor

The FlatBuffers file per deployment that a runtime reads without decoding the
IR, as built. It is the second artifact of the runtime descriptors design (the
first is [the catalog descriptor](catalog-descriptor.md)) and the second emitter
over the lowered `System` of
[ADR-0022](../decisions/ADR-0022-rsdl-system-in-the-ir.md) (decision 12). The
tracking issue is driftsys/ridl#1176.

The reasoning behind the artifact is the archived design note
[`../archive/2026-09-13-runtime-descriptors-design.md`](../archive/2026-09-13-runtime-descriptors-design.md)
(decisions D-1, D-5 and D-9). This record states the code. Where the code
differs from that note, the section "Differences from the design" lists the
difference.

## Why a deployment descriptor

A runtime that keeps its state in shared memory has a node and clients. At
attach, the node must know which client may write an interface (it offers the
interface's service) and which catalogs a client may map (a `requires` line of
its component reaches them). Those facts belong to a deployment: one component
can have several instances, and the same catalog serves every system that uses
its interfaces. Neither the catalog descriptor nor the generated interface code
carries them, because either would tie a unit to one system and would change the
catalog hash when the wiring changes. The deployment descriptor carries them in
one file per deployment.

## Where the code is

| What                                                                                                         | Where                                                        |
| ------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------ |
| The schema                                                                                                   | `crates/ridl-descriptor/schema/deployment.fbs`               |
| The generated accessors (committed)                                                                          | `crates/ridl-descriptor/src/deployment/generated.rs`         |
| The generator and its drift test (both schemas)                                                              | `xtask/src/descriptor.rs` (`cargo xtask descriptor-codegen`) |
| `finish`, `verify`, `VerifyError`, `SCHEMA_VERSION`, `FILE_IDENTIFIER`, `FILE_SUFFIX`, `MAX_ATTRIBUTE_DEPTH` | `crates/ridl-descriptor/src/deployment.rs`                   |
| The lowering from the system IR (`lower`, `DeploymentLowerError`)                                            | `crates/ridl-descriptor/src/deployment/lower.rs`             |
| The JSON view (`to_json`)                                                                                    | `crates/ridl-descriptor/src/deployment/describe.rs`          |
| `offers` and `maps` of an instance (`lower_deployment`)                                                      | `crates/ridl-ir/src/codegen.rs`                              |
| `Emit::Deployment`, `write_deployments`                                                                      | `crates/ridlc/src/lib.rs`                                    |
| `ridl describe` (`run_describe`)                                                                             | `crates/ridl/src/main.rs`                                    |
| The placement's backend keys in the codegen deployment section (`Instance.attributes`)                       | `crates/ridl-ir/proto/ridl/codegen/v1/deployment.proto`      |
| The instance lock: format, allocation, retire, rename, merge                                                 | `crates/ridl-core/src/instance_lock.rs`                      |
| The numbering step, and the `RSDL-314` and `RSDL-315` diagnostics                                            | `crates/ridl-sem/src/rsdl/numbering.rs`                      |
| `ridl lock` for instances, and the merge driver                                                              | `crates/ridl/src/lock.rs`                                    |
| The `instances` module of the generated Rust crate                                                           | `crates/ridl-backend-rust/src/instances.rs`                  |
| The `instance number changed` heading of `ridl diff`                                                         | `crates/ridl-diff/src/system.rs`                             |

`finish`, `verify` and the generated accessors need no standard library. `lower`
and `to_json` are under the `std` feature of `ridl-descriptor`.

## The artifact

One file per lowered deployment, named
`<deployment package>.<DeploymentName>.deployment.binfb`. The FlatBuffers file
identifier is `RDLD`, in bytes 4 to 8. The schema version is 1
(`SCHEMA_VERSION`). The deployment's qualified name is unique in a workspace,
because rsdl rejects a duplicate declaration name in a package.

`ridlc build --emit deployment` (and `ridl build --emit deployment`) writes one
file for each deployment in `System.deployments`. It writes nothing in
single-file mode, in a workspace with no `system`, or for a system with no
deployment. A deployment that an RSDL-7xx error blocked is absent from the
lowered system and gets no file (ADR-0022 decision 8). A lowering failure is an
I/O error of the command, with exit code 2. The catalog descriptors of the
regions are lowered once per build and shared by the catalog files and the
deployment files; a region that names a unit from a dependency is lowered too.

The file is self-contained. For every region of the system's closure it embeds
the catalog descriptor bytes that `--emit catalog` writes for that unit,
unchanged, with the region's 32-byte hash beside them. The lowering checks that
the embedded catalog verifies and that its hash is the region's hash; a failure
is a `DeploymentLowerError`, not a bad file. A reader configures a deployment
from this one file and compares one hash with the client's catalog.

The toolchain assigns no number to a component or a service. It numbers an
instance from `instances.lock`. See "Identifying an instance".

## What a deployment descriptor contains

The root table `Deployment` carries these fields, in field-id order. Each list
is a direct projection of a field of the system IR, in the order the IR holds
it.

| Field                                  | Content                                                                                                                                                                     |
| -------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `version`, `system`, `name`, `package` | The schema version (1), the qualified name of the system, and the name and package of the deployment.                                                                       |
| `catalogs`                             | One `EmbeddedCatalog { name, hash, bytes }` per region, in region order (catalog name order). `bytes` is the catalog descriptor.                                            |
| `interfaces`                           | One `DeployedInterface { catalog, number, name, inline, provisional, service }` per interface of a region: the region map flattened, with the owning service.               |
| `machines`                             | `Machine { name, external, labels, attributes }`, in declaration order.                                                                                                     |
| `components`                           | `Component { name, implicit, external, instances, labels, attributes }`, in closure order.                                                                                  |
| `instances`                            | One `Instance { component, instance, machine, external, attributes, offers, maps, number, provisional }` per placement, in placement order.                                 |
| `links`                                | `Link { catalog, number, service, consumer, producer, crossing, attributes }`, in IR link order. `consumer` and `producer` are `Endpoint { component, instance, machine }`. |
| `routes`                               | `Route { catalog, number, ordinal, interface, member, service, producers }`, in the IR's (catalog, number, ordinal) order.                                                  |
| `surface`                              | `Surface { link, direction }`. `link` is the index of the link in `links`; `direction` is `ExternalConsumes` or `ExternalOffers`.                                           |
| `system_attributes`, `attributes`      | The backend keys of the system and of the deployment.                                                                                                                       |

An instance's `attributes` are the backend keys of its placement line. A link's
`attributes` are the backend keys of the `requires` line that the link comes
from. `crossing` is `SameMachine`, `DifferentMachine` or `OffBoard`.

An `Attribute { ns, key, value }` is a backend key as the IR carries it. The
`value` table is absent for a flag; otherwise it is an
`AttributeValue { scalar, list }` with exactly one of the two set, and a list
nests other values.

`offers` lists the interface keys (`InterfaceKey { catalog, number }`) whose
slots the instance writes, read from the producers of the closure. `maps` lists
the names of the catalogs that the instance's component reaches through its
`requires` lines, read from the grant. The lowering takes both lists from
`ridl_ir::codegen::lower_deployment`, the function that builds the codegen
deployment section, so that the descriptor and the section cannot disagree.

Not contained: the transport chosen for a crossing, the fabric, envelope and
framing overhead, doc comments, `@since`, `deprecated`, distributions, and the
sizing values (`depth`, `slots`, `budget` and the derived values). The codegen
deployment section resolves the precedence of the sizing values; a later version
appends them when a runtime reads them.

The JSON view that `ridl describe` prints uses the schema's field names as keys
(`ns` for the namespace), except that an embedded catalog prints no `bytes`: it
prints `bytes_len`, the length of the buffer, and `catalog`, the catalog's own
JSON view (`null` for a buffer that fails the catalog verifier). It names enum
members, prints a hash as an array of bytes and prints an absent field as
`null`. Keys are in alphabetical order, as in the catalog's view.

## Identifying an instance

An instance has a number from 1 to 2147483647, assigned by `ridl lock` in the
root `instances.lock` and stable across builds (rsdl reference section 7). The
lowered system carries it in `Component.numbers`, one entry of
`InstanceNumber { instance, number, provisional }` per instance. The codegen
deployment section copies it into `Instance.number` and `Instance.provisional`,
and the deployment descriptor copies it from the section into the `number` and
`provisional` fields of its `Instance`. A runtime that identifies an instance by
an integer reads `number` from the descriptor, and a plugin reads the same value
from the section.

`ridl build` also writes each instance's number as a constant in the `instances`
module of the generated Rust crate. A component is a module and an instance is a
constant, so the number of instance `primary` of component `veh.adas.Cruise` is
`instances::veh::adas::cruise::PRIMARY`. The module is not per deployment: the
numbers belong to the system, and every deployment of the system shares them. A
constant of a provisional number says so in its documentation.

`provisional` is true when the instance has no live entry in `instances.lock`.
Its number is then the next free one at the time of the build, and a later
`ridl lock` may assign another. A runtime that stores the number outside the
build treats a provisional number as unstable.

0 is not an instance number. Negative numbers are reserved for the runtime: the
toolchain neither assigns nor generates one, so no descriptor and no generated
constant holds one. A client sends its instance number when it attaches only; no
data frame carries it, so instances stay indistinguishable on the wire (rsdl
reference section 7).

Instance numbers are unique within a deployment. The verifier refuses an
instance whose number is 0 or negative (`VerifyError::InstanceNumber`), and a
descriptor in which two instances have the same number
(`VerifyError::DuplicateInstanceNumber`, which names the number and both
instances). The lowering reports a system that holds a number that is not
positive as `DeploymentLowerError::Internal`. A backend key on the placement
line stays what it was: the toolchain carries it as declared in `attributes` and
does not interpret it (rsdl section 5).

## Attaching to a node

The loopback runtime reads a descriptor to decide what an attach may do
([the loopback record](ridl-loopback.md) describes the types). This section
states the rules applied by the in-process node.

- **Construction.** `Node::from_descriptor(bytes, machine)` takes a `&'static`
  descriptor, because the catalog names borrow its bytes, and a machine name. It
  verifies the descriptor first, then refuses a machine the descriptor does not
  hold (`UnknownMachine`) and an offer or map that names a catalog the
  descriptor does not embed (`MissingCatalog`).
- **The grant.** The grant of an instance placed on the machine is the set of
  catalogs it offers plus the catalogs it maps. The node creates one store for
  each distinct catalog name and hash that some local instance is granted.
  Attaching an instance that is not placed on the machine fails with
  `UnknownInstance`, a catalog with no local store fails with `UnknownCatalog`,
  and a catalog outside the grant fails with `CatalogNotGranted`.
- **The twin.** With the `debug` feature, the twin `-N` of a placed instance `N`
  receives exactly the grant of `N`. Without the feature, a negative number is
  an unknown instance.
- **The exclusion.** `N` and `-N` are never attached at the same time, in any
  catalog of the node: the node counts the live attachments of each identity
  across catalogs, and an attach whose opposite identity has a live attachment
  fails with `IdentityConflict`. Several attachments of the same identity may
  coexist. Dropping the last attachment of an identity releases it.
- **The listener.** With the `debug` feature, the listener `i32::MIN` has no
  grant and attaches to every catalog of the machine. It can read, subscribe and
  send a call. It writes nothing: setting, invalidating or touching a signal,
  raising an event and serving fail with `NotOwner`, and it takes no claim.

Not built: a check that only the instance that offers an interface writes it
(the grant stops at the catalog and does not name interfaces), and the
arbitration of a redundant provider set, which is driftsys/ridl#1196.

## Verification before access

`ridl_descriptor::deployment::verify` is the reader's first call. It checks the
length, the file identifier (`RDLD`) and the version, walks every field, and
verifies each embedded catalog with the catalog verifier, then compares the hash
stored beside it with the hash inside it (`CatalogHashMismatch`). Attribute
lists are verified to a depth of 16 (`MAX_ATTRIBUTE_DEPTH`), and the verifier
visits at most `bytes.len() / 4` attribute values, counting every visit of a
table that several lists share. It refuses an `AttributeValue` that sets both
`scalar` and `list`, or neither (`MalformedAttributeValue`); a flag has no value
table at all. It refuses a surface entry whose `link` is not an index into
`links` (`SurfaceLink`). It has its own
`ridl_descriptor::deployment::VerifyError` (`TooShort`, `WrongIdentifier`,
`WrongVersion`, `Invalid`, `EmbeddedCatalog`, `CatalogHashMismatch`, `TooDeep`,
`TooManyAttributeValues`, `MalformedAttributeValue`, `InstanceNumber`,
`DuplicateInstanceNumber`, `SurfaceLink`); the catalog `VerifyError` is
unchanged.

`ridl describe <file>` reads either descriptor and chooses by the file
identifier. For a deployment descriptor it verifies the file and every embedded
catalog, then prints the JSON view. A refusal exits 2 and names the embedded
catalog, including the case of a stored hash that differs from the catalog's
own. The lowering checks the same hash when it writes the file.

`DeploymentLowerError` has these variants: `UnknownDeployment`,
`MissingCatalog`, `HashMismatch`, `InvalidCatalog`, `AttributeTooDeep` and
`Internal` (a value in the system IR that the schema cannot express, a link with
no interface, no consumer or no producer, a link whose interface no region of
its catalog holds, or a buffer that the lowering wrote and the verifier then
refused). The lowering counts the depth of a backend key's value as the verifier
does: the value is at depth 1 and each list wrapper adds 1, so a scalar inside
15 lists is the deepest value accepted. A deeper value draws `AttributeTooDeep`,
which names the namespace and the key, at every site that carries backend keys.
The lowering verifies the buffer it has written before it returns it.

## Evolution

The schema follows the catalog schema's rule. A field is only appended at the
end of its table and never removed, reordered or retyped, and an enum member is
never renumbered. A reader built against an older schema ignores fields it does
not know. Regenerate the accessors after every edit with
`cargo xtask descriptor-codegen`; the staleness test covers both schemas.

## Decisions and alternatives considered

The choices below were taken under delegation from the owner, each with its
reason. They bind later changes to the artifact.

- **One file per deployment, with its own name and identifier.** The file is
  `<package>.<Name>.deployment.binfb` with identifier `RDLD`. The lowered IR
  already uses `<pkg.Name>.system.{json,txtpb,binpb}` for the whole `System`, so
  a `.system.binfb` would differ from the IR artifact by its last extension only
  and name a different thing. Rejected: the identifier `RDLS` with the name
  `<pkg.System>.<deployment>.system.binfb`, because its suffix collides in form
  with the IR artifact; one file per system that holds every deployment, because
  a node reads one deployment.
- **The file embeds the catalogs it reaches.** A node configures a deployment
  from one file, and the embedded bytes are identical to the standalone
  `.catalog.binfb` that a client includes, so both sides compare one hash. The
  catalogs and the deployment come from one build, so a hash that differs is a
  defect of the build and not of the input.
- **The number comes from `instances.lock`, not from a backend key.** A runtime
  that needs an integer per instance reads it from the descriptor, and the
  toolchain assigns it in a lock file as `interfaces.lock` numbers interfaces,
  so that the number is stable and the same in every artifact. This replaces an
  earlier decision that left the number to a backend key. The owner rejected
  that decision on 2026-11-02: a key typed by hand in each deployment can differ
  between deployments, and no check compares it with another instance's key.
  Rejected: a key owned by rsdl (`id = 3`), because a number written in source
  is not unique across deployments; a number derived from placement order,
  because it changes when a source is reordered.
- **One instance lock for the workspace, at the root.** An interface number is
  scoped by its catalog, so the interface lock is one per unit. An instance
  number is scoped by nothing: a node compares it with every instance of a
  deployment, and a deployment can place components of several units, so one
  file numbers them all. The root workspace numbers every instance, including
  those of a component declared in a dependency unit, because the workspace that
  builds a system is the one that deploys it and nothing may be written into a
  remote unit (ADR-0002). Plain `ridl lock` at the root is the only command that
  allocates. The format, the key, the allocation and the retire and rename rules
  are specified in the rsdl reference section 7 and the CLI reference, not
  repeated here. Rejected: a lock per unit, because two units would both start
  at 1 and scoping the number by unit would put the unit on the attach as a
  second identity; the dependency's own lock, because its numbers can collide
  with the root's and the dependency's workspace may deploy the component
  elsewhere; a number keyed per system or per deployment, because one instance
  has one number in every build and its constant must not depend on a
  deployment.
- **The numbering is one workspace step before lowering.** `number_instances`
  takes the checked systems of the root workspace and the lock and returns the
  map from key to number and provisional flag. A provisional number depends on
  every other provisional key, so it is computed once for the workspace and not
  per system. `lower_system` takes the map, so the system IR, the codegen
  section and the descriptor read one source and cannot disagree.
- **`ridl diff` reports a changed locked number, and not a rename.** When the
  old key is absent on the new side, a diff cannot tell a rename from a reuse.
  The lock never reuses a number, and a client built before a rename sends the
  same number to the same instance, so the diff does not report it (rsdl
  reference section 14). A provisional old number was never a promise and is
  never reported.
- **The constants are an integer, from the system artifact, with no constants
  for a bare file.** The type is `i32`, the type of the attach field; a newtype
  has since been added in `ridl-rt` as `InstanceNo` (see "Attaching to a node");
  the generated constants stay `i32`. A single-file build writes no crate
  `lib.rs`, so it gets no `instances` module; its numbers still reach the IR and
  the codegen section. A single-file build writes no deployment descriptor (see
  "The artifact"). A workspace with no `system` gets no module.
- **The codegen deployment section carries the placement's backend keys**
  (`Instance.attributes`) and the instance's number and provisional flag
  (`Instance.number`, `Instance.provisional`), so that a plugin generates a
  client's bring-up code from the values the node reads and no program writes
  the number by hand. The fields are additive under the IR specification
  section 6.
- **No deployment module in the Rust backend.** A deployment module in a
  generated crate would bind the crate to one deployment, which is the problem
  that the catalog hash exists to avoid. A runtime that wants generated identity
  code writes a plugin over the deployment section. Rejected: a `deployment`
  module per deployment with each instance's names and attributes as constants;
  it can return as an opt-in option when a second runtime asks for it.
- **The descriptor and the deployment section share one rule** for `offers` and
  `maps`, by calling `lower_deployment`; a test compares the complete `offers`
  and `maps` lists of every instance with the codegen section, over several
  catalogs and interfaces.
- **The verifier is bounded, and the writer never emits what the reader
  refuses.** The attribute walk is limited in depth and in visits because a
  hostile buffer can share one subtree many times. The lowering applies the same
  depth limit, returns `AttributeTooDeep` for a deeper value, and verifies the
  buffer it wrote before it returns it. A stored hash that differs from the
  embedded catalog's hash is refused by `verify`, because a node that reads the
  file needs the check and not only the `describe` command.
- **The lowering fails instead of choosing a default.** A value that the schema
  cannot express (an unspecified crossing or direction, a surface entry with no
  equal link, an attribute value with neither scalar nor list) is an `Internal`
  error, because a default would write a wrong descriptor that a node trusts.
- **Errors are separate types.** The deployment verifier has its own
  `VerifyError`, because adding a variant to the published, exhaustive catalog
  enum is a semver break that the catalog reader does not need.

## Differences from the design

The working design for this artifact differed from the code in these points. The
code is the record.

- The namespace field of `Attribute` is `ns`, because `namespace` is a keyword
  of the FlatBuffers grammar.
- `EmbeddedCatalog.bytes` is a plain `[ubyte]`. The design gave it the
  `nested_flatbuffer` attribute, which names a type of the same schema, and the
  deployment schema does not include `catalog.fbs`. The verifier checks the
  embedded buffer instead.
- The deployment verifier has its own `VerifyError`, and it bounds the attribute
  lists (depth 16, `bytes.len() / 4` values). The design reused the catalog
  verifier's error and set no bound.
- The design called a hash mismatch at lowering an internal error. It is the
  `HashMismatch` variant, beside `InvalidCatalog` for a catalog that fails the
  catalog verifier, and the CLI reports both with exit code 2.
- The JSON view has alphabetical key order, as the catalog's view has.

## Tests

| What                                                                                                                                                                                        | Where                                                   |
| ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------- |
| The round trip, and the identifier at bytes 4 to 8                                                                                                                                          | `crates/ridl-descriptor/tests/deployment_round_trip.rs` |
| The verifier: short, foreign, wrong version, truncated, a bad embedded catalog, attribute depth and shared-child bounds                                                                     | `crates/ridl-descriptor/tests/deployment_verify.rs`     |
| The lowering: offers, maps, placement keys, links, surface, the embedded bytes, each `DeploymentLowerError`, stable bytes, the JSON view                                                    | `crates/ridl-descriptor/tests/deployment_lower.rs`      |
| The builders the tests above share                                                                                                                                                          | `crates/ridl-descriptor/tests/deployment_common.rs`     |
| `--emit deployment`: one file per deployment, the embedded catalog equal to the standalone file, none for a blocked deployment, a system with no deployment, no system, or single-file mode | `crates/ridlc/tests/deployment_descriptor.rs`           |
| `ridl describe` of a deployment descriptor, and its refusal of a damaged embedded catalog                                                                                                   | `crates/ridl/tests/describe_cli.rs`                     |
| A catalog of a dependency unit is embedded in the deployment                                                                                                                                | `crates/ridl/tests/remote_units.rs`                     |
| The accessors match the schema                                                                                                                                                              | `xtask/src/descriptor.rs`                               |

## Open items

| Item                                                                                                   | Where                                                |
| ------------------------------------------------------------------------------------------------------ | ---------------------------------------------------- |
| A manifest claim of a backend namespace for a runtime outside the toolchain (RSDL-804 stays a warning) | ADR-0022, open item 3                                |
| The sizing values (`depth`, `slots`, `budget`) in the descriptor, when a runtime reads them            | driftsys/ridl#1176                                   |
| A deployment module in the generated Rust crate, as an opt-in option, when a second runtime asks       | driftsys/ridl#1176                                   |
| The cross-compiler check of the schemas with `flatc` and `flatcc` (D-3 of the design)                  | [catalog record](catalog-descriptor.md), "Not built" |

## Trace

- Tracking issue: driftsys/ridl#1176
- Decisions: [ADR-0022](../decisions/ADR-0022-rsdl-system-in-the-ir.md)
  decisions 11 and 12,
  [ADR-0020](../decisions/ADR-0020-third-encoding-runtime-layering-and-plugin-system.md)
  decision 9
- Design records: [`catalog-descriptor.md`](catalog-descriptor.md),
  [`codegen-plugins.md`](codegen-plugins.md)
- The reasoning:
  [`../archive/2026-09-13-runtime-descriptors-design.md`](../archive/2026-09-13-runtime-descriptors-design.md)
