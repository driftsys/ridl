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

The toolchain assigns no number to a component, an instance or a service. See
"Identifying an instance".

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
| `instances`                            | One `Instance { component, instance, machine, external, attributes, offers, maps }` per placement, in placement order.                                                      |
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

The toolchain assigns no instance number. A runtime that identifies an instance
by an integer declares it as a backend key of its own namespace on the placement
line, and reads it from the instance's `attributes` in the descriptor. The
toolchain carries the key as declared and does not interpret it (rsdl section
5).

```rsdl,ignore
deployment Vehicle for VehicleSystem {
  machine Hpc {
    VehicleStateProvider [ shm.service = 3 ]
    UiModel [ shm.service = 4 ]
  }
}
```

A namespace that no configured backend claims draws RSDL-804, a warning, and the
key is carried. That warning is accepted. A way for a manifest to claim a
namespace for a runtime outside the toolchain is the third open item of ADR-0022
and is not built.

The codegen deployment section carries the same keys in `Instance.attributes`,
so that a plugin which generates a client's bring-up code can emit the number
from the key the node reads. The Rust backend emits no deployment module: a
generated interface crate stays independent of every deployment.

## Verification before access

`ridl_descriptor::deployment::verify` is the reader's first call. It checks the
length, the file identifier (`RDLD`) and the version, walks every field, and
verifies each embedded catalog with the catalog verifier, then compares the hash
stored beside it with the hash inside it (`CatalogHashMismatch`). Attribute
lists are verified to a depth of 16 (`MAX_ATTRIBUTE_DEPTH`), and the verifier
visits at most `bytes.len() / 4` attribute values, counting every visit of a
table that several lists share. It refuses an `AttributeValue` that sets both
`scalar` and `list`, or neither (`MalformedAttributeValue`); a flag has no value
table at all. It has its own `ridl_descriptor::deployment::VerifyError`
(`TooShort`, `WrongIdentifier`, `WrongVersion`, `Invalid`, `EmbeddedCatalog`,
`CatalogHashMismatch`, `TooDeep`, `TooManyAttributeValues`,
`MalformedAttributeValue`); the catalog `VerifyError` is unchanged.

`ridl describe <file>` reads either descriptor and chooses by the file
identifier. For a deployment descriptor it verifies the file and every embedded
catalog, then prints the JSON view. A refusal exits 2 and names the embedded
catalog, including the case of a stored hash that differs from the catalog's
own. The lowering checks the same hash when it writes the file.

`DeploymentLowerError` has these variants: `UnknownDeployment`,
`MissingCatalog`, `HashMismatch`, `InvalidCatalog`, `AttributeTooDeep` and
`Internal` (a value in the system IR that the schema cannot express, or a buffer
that the lowering wrote and the verifier then refused). The lowering counts the
depth of a backend key's value as the verifier does: the value is at depth 1 and
each list wrapper adds 1, so a scalar inside 15 lists is the deepest value
accepted. A deeper value draws `AttributeTooDeep`, which names the namespace and
the key, at every site that carries backend keys. The lowering verifies the
buffer it has written before it returns it.

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
- **No number for an instance in ridl.** A runtime that needs an integer
  declares it as a backend key on the placement line. The language already has
  the site and the carrier, and a number is a fact of one runtime's wire, which
  rsdl section 1.3 keeps out of the toolchain's identities. Rejected: a lock
  file of instance numbers, as `interfaces.lock` numbers interfaces, because it
  adds a second lock with its resolver and baseline rules for one runtime's
  integer, and a backend key written once in source is as stable; a key owned by
  rsdl (`id = 3`), because it changes the language for one runtime's need,
  against the family rule that a backend's fact is a backend key; a number
  derived from placement order, because it changes when a source is reordered.
- **The codegen deployment section carries the placement's backend keys**
  (`Instance.attributes`), so that a plugin generates a client's bring-up code
  from the same key the node reads and no program writes the number by hand. The
  field is additive under the IR specification section 6.
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
