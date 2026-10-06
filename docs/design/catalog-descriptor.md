# The catalog descriptor

The FlatBuffers file per package that an engine reads without decoding the IR,
as built. The binding choices are
[ADR-0014](../decisions/ADR-0014-ir-encodings.md) decision 15 (the catalog hash:
its input, its determinism rule, where it is computed, and its golden test),
[ADR-0020](../decisions/ADR-0020-third-encoding-runtime-layering-and-plugin-system.md)
decision 5 as amended 2026-10-03 (the toolchain may depend on planus; `ridl-rt`
and every generated package must not),
[ADR-0023](../decisions/ADR-0023-interaction-face-generation.md) decision 8 (the
generated face checks the port's catalog and panics on a mismatch),
[ADR-0010](../decisions/ADR-0010-cli-conventions.md) (the `ridl describe` row of
the exit-code table) and
[ADR-0022](../decisions/ADR-0022-rsdl-system-in-the-ir.md) decision 7 and its
notes (the catalog hash in each region of the lowered system). The projections
the size states are computed through are
[ADR-0017](../decisions/ADR-0017-proto3-projection-rules.md) (proto3) and
[ADR-0019](../decisions/ADR-0019-flatbuffers-projection-rules.md) decision 8
(FlatBuffers).

The reasoning behind each decision is the archived design note
[`../archive/2026-09-13-runtime-descriptors-design.md`](../archive/2026-09-13-runtime-descriptors-design.md)
(decisions D-1 to D-10). Its plan is
[`../archive/2026-09-13-catalog-descriptor-plan.md`](../archive/2026-09-13-catalog-descriptor-plan.md),
whose "Re-baseline 2026-10" section lists what changed from the plan of
2026-09-13. The lane driver is
[`../archive/2026-10-03-lane-e16-catalog-descriptor-driver.md`](../archive/2026-10-03-lane-e16-catalog-descriptor-driver.md):
its §4 holds Sebastien's answers of 2026-10-03, which this record follows.

**Only the catalog half of the design is built.** The design also defines a
system descriptor per deployment. That half, and the other items listed under
"Not built" below, is not built. For it, the archived design note is the record
of the agreed direction.

Sebastien reviews the lane's delegated decisions in the driver's
[§5 "Decisions taken under delegation"](../archive/2026-10-03-lane-e16-catalog-descriptor-driver.md#5-decisions-taken-under-delegation).

## Where the code is

| What                                                                             | Where                                                        |
| -------------------------------------------------------------------------------- | ------------------------------------------------------------ |
| The schema                                                                       | `crates/ridl-descriptor/schema/catalog.fbs`                  |
| The generated accessors (committed)                                              | `crates/ridl-descriptor/src/generated.rs`                    |
| The generator and its drift test                                                 | `xtask/src/descriptor.rs` (`cargo xtask descriptor-codegen`) |
| `finish`, `verify`, `SCHEMA_VERSION`, `FILE_IDENTIFIER`, `FILE_SUFFIX`           | `crates/ridl-descriptor/src/lib.rs`                          |
| The lowering from the IR (`lower`, `LowerError`, the `type_name` spelling)       | `crates/ridl-descriptor/src/lower.rs`                        |
| The interface numbers, copied from the IR                                        | `crates/ridl-descriptor/src/number.rs`                       |
| The size states, the size context, the leaf model, the string byte capacity      | `crates/ridl-ir/src/projection/size.rs`                      |
| The proto3 state                                                                 | `crates/ridl-ir/src/projection/size/proto3.rs`               |
| The FlatBuffers state                                                            | `crates/ridl-ir/src/projection/size/flatbuffers.rs`          |
| The JSON view                                                                    | `crates/ridl-descriptor/src/describe.rs`                     |
| The catalog hash (re-exported as `ridl_descriptor::hash`)                        | `crates/ridl-ir/src/catalog_hash.rs`                         |
| The proto3 scalar table and field-number limits both sides read                  | `crates/ridl-ir/src/projection/proto3.rs`                    |
| `Emit::Catalog`, `write_emits`, `lower_workspace_system`, `embed_catalog_hashes` | `crates/ridlc/src/lib.rs`                                    |
| `ridl describe` (`run_describe`)                                                 | `crates/ridl/src/main.rs`                                    |
| The port's catalog check in the generated face                                   | `crates/ridl-backend-rust/src/face.rs`                       |
| The planus boundary                                                              | `xtask/tests/oracle_boundary.rs`                             |

## The artifact

`ridlc build --emit catalog`, and `ridl build --emit catalog`, write one file
per package, `<base>.catalog.binfb`, beside the IR dumps and under the same base
name. `catalog` is the ninth `--emit` value. A file is written only for a
package that has at least one interface shape: a declared `interface`, or a
`service` with an inline body (`Package::shapes()`). A package with no shape
gets no file.

The file is a FlatBuffers buffer with the file identifier `RDLC` at bytes 4..8
(`FILE_IDENTIFIER`). Its root table `Catalog` carries a `version` field, which
is `SCHEMA_VERSION`, now 1. The schema `catalog.fbs` is hand-written, and its
header states the evolution rule: a field is only ever appended at the end of
its table, and a field is never removed, reordered or retyped, and an enum
member is never renumbered. A reader built against an older schema ignores the
fields it does not know.

**The accessors are generated and committed.** `cargo xtask descriptor-codegen`
runs planus (`planus-translation` and `planus-codegen`, which `xtask` depends
on; `ridl-backend-rust` carries both as test-only dependencies, and
`ridl-backend-flatbuffers` carries `planus-translation` as one) over the schema
and formats the output with `rustfmt` at the workspace edition. No `flatc`
binary is involved. The test `committed_generated_accessors_match_the_schema` in
`xtask/src/descriptor.rs` fails when `generated.rs` is stale. The three planus
crates are pinned to `=1.3.0` in the root `Cargo.toml`, because the generated
code calls `check_version_compatibility("planus-1.3.0")`, and a caret range
would break the published crate for a consumer who resolves without the lock
file.

**Every buffer is finished by `ridl_descriptor::finish`.** planus 1.3.0's
`Builder::finish(root, Some(id))` writes the identifier at bytes 0..4 and the
root offset at bytes 4..8, which is the reverse of the FlatBuffers layout, and
planus's own reader then rejects the buffer. `finish` calls planus and then
swaps the two header fields, adding 4 to the root offset. The test
`planus_writes_the_identifier_before_the_root_offset` in `tests/round_trip.rs`
fails when a planus release changes that behaviour, which is when `finish` can
call planus directly.

**planus stays in the toolchain.** `ridl-descriptor` depends on the `planus`
runtime, and `ridlc` and `ridl` depend on `ridl-descriptor`. `ridl-rt` and every
generated package must not depend on any planus crate, through any dependency
kind (ADR-0020 decision 5, as amended). `xtask/tests/oracle_boundary.rs` checks
both: `the_runtime_reaches_no_planus_crate` resolves `ridl-rt` with every
feature on, and `the_generated_crate_reaches_no_planus_crate` reads the resolved
graph of the crate `ridl build` generates for `examples/cabin`, which runs under
`just demo`.

`ridl-descriptor` is published to crates.io, after `ridl-ir` and before `ridlc`
(`.github/workflows/crates-io-release.yml`).

## What a catalog contains

`ridl_descriptor::lower(package, others)` writes the whole file in one pass. It
returns `Result<Vec<u8>, LowerError>`.

- **The catalog.** `version`; `name`, the package name; `hash`, the 32-byte
  catalog hash; `toolchain`, the version of the crate that wrote the file;
  `interfaces`; and `retired`.
- **The numbers are the IR's.** Each `Interface` carries its `name`, `number`
  and `provisional`, copied from the IR's `Interface.number` and
  `Interface.provisional`, which `ridl-sem` folds from `interfaces.lock`. The
  descriptor computes no numbering of its own (`number.rs`). A provisional
  number is written and flagged: it is data, and an engine decides what to do
  with it. A number of 0 is `LowerError::ZeroNumber`, an internal error, which
  `ridlc` returns as an I/O error so that the build stops with exit code 2.
- **An inline service shape** is an interface under the service's dotted name.
- **The retired list** is `Package.retired`, copied as name and number, so an
  engine can refuse a peer that still speaks a retired interface.
- **Members.** One `Member` per interaction, in body order: `name`, `ordinal`
  (the position in the body, ridl §11), `kind` (`Signal`, `Event`, `Command`,
  `Query`, `Fixed`), `payloads`, and `timing` (`mode`, `min_us`, `max_us`) when
  the interaction declares one. A fixed member never has a timing. Timing is the
  only quality-of-service term the IR carries for an interaction, and contract
  clauses and a signal's initial value are not carried.
- **Payloads.** A signal has one payload with the role `value`, an event one
  with `occurrence`, a command one with `request`, a fixed one with `value`. A
  query has a `request`, and a `response` when the IR carries a return type.
- **Reserved ordinals.** Each interface lists the ordinals its `reserved`
  tombstones hold, in `reserved_ordinals`, so the ordinal space is complete.

**`type_name`** is the canonical name of a payload that is one named type, and
otherwise a spelling in the typl syntax over the IR's canonical values (which
are not always the source's literals): a tuple or a request of several
parameters `(a: T, b: U)`, an exact-length array `[T; N]`, a bounded array
`[T; min..max]`, a map `[K: V; min..max]`, an inline scalar such as
`km/h [0..250 step 0.5]`, a stream `<T>`, and a fallible reply `T | E`. An
optional type carries `?`. No reader parses these strings.

Not contained: type layouts, field lists, and constraints other than the bounds
that size a payload. A consumer that needs a payload's shape reads the schema
the wire backend emits.

## The catalog hash

The hash is SHA-256 over `ridl_ir::v2::to_binary` of the package that
`ridl_ir::catalog_hash::reduced_package` returns: the package name, the
interface shapes in (number, name) order with their numbers, and every
declaration they reach in any package of the build, under canonical names, with
doc strings, `labels` and `deprecated` blanked, and no services and no retired
entries. ADR-0014 decision 15 is the full rule, with the determinism rule for
the binary and the reason the canonical JSON is not the input. There is one
identity: the schema hash driftsys/ridl#275 asked for is this hash, so it does
not depend on which wire schema a build emits.

It is computed in `ridl-ir`, not in `ridl-descriptor`, because three artifacts
carry it and `ridl-ir` is below all of them:

- the catalog descriptor, through `ridl_descriptor::hash`, which re-exports
  `catalog_hash`, `reduced_package` and `reachable_decls`;
- the codegen model's `Catalog.hash`, which the Rust backend writes into the
  `CATALOG` of every generated interface descriptor type. The Rust backend
  refuses a model whose hash is missing or is not 32 bytes long;
- each `Region` of the lowered system (`bytes hash = 3`).
  `ridl_sem::lower_system` leaves it empty and does not depend on
  `ridl-descriptor`. `ridlc` fills it in `embed_catalog_hashes`, called by
  `lower_workspace_system` (which `compile_workspace`, and so `ridl diff`, uses)
  and by `ridl build` whenever that build lowers the system — for a code emit,
  for a plugin, for `--deployment`, or for a system dump.

**One scope for every hash.** `catalog_scope` gives the descriptor and the
regions the same packages: every checked package of the workspace, then
`ridl.std` when a package references it. `Emit::Catalog` is classed as a code
emit for that reason, so a build with `--emit catalog` alone keeps `ridl.std` in
`others`, and its hash equals the hash the Rust face carries. The system write
checks `ridl.std` itself when a package references it and no code emit checked
it. An entry of `others` named like the hashed package is skipped, so the hash
is the same whether or not a caller includes the package there.

The golden test is `crates/ridl-descriptor/tests/golden_hash.rs`: it hashes the
corpus package's checked-in IR snapshot, its shapes numbered 1.. by the test,
and compares the result with a pinned value. The codegen-model corpus snapshots
in `crates/ridlc/tests/snapshots/` carry the hash of each corpus package as the
compiler builds it, which guards the compiler-driven changes the frozen snapshot
cannot see.

## The size states

Each payload carries a `max_sizes` list with one `MaxSize` row per encoding that
has a state. A row has `encoding` (`Proto3`, `FlatBuffers`, `ReprC`), `state`
(`Bounded` or `Unbounded`), `bytes` (a `uint32`, as in
`ridl_rt::contract::EncodedSizes` and the codegen model) and `cause` (the
codegen model's `FbUnboundedCause`, member for member). That gives three states
per payload and encoding:

- **absent**: no row, because this toolchain computed no state;
- **bounded**: `bytes` is the maximum encoded size of the payload, envelope and
  framing excluded;
- **unbounded**: `bytes` is 0 and `cause` says why.

The states are `ridl_ir::projection::size`'s. The sizer is a projection, read by
this descriptor and by every other consumer of the same bound. It answers
`Bounded`, `Unbounded` with a cause, or `Absent` with a cause of its own, and
this descriptor writes no row for an absent state.

**The descriptor defines no wire shape.** Only a payload that is one named type
is sized, through the projection the wire backend emits:

- **FlatBuffers** is `ridl_ir::projection::flatbuffers::max_size`, the one
  implementation of the bound, which the Rust codec's `MAX_SIZE` is emitted from
  too ([`flatbuffers-codec.md`](flatbuffers-codec.md)). It is called over the
  projection's `Packages` rooted at the declaring package (`Ctx::packages_for`),
  so a bare name inside an imported declaration resolves in that package. When
  `max_size` answers `None`, the row is `Unbounded` with the cause from
  `ridl_ir::codegen::fb_unbounded`, the function the codegen model uses. The row
  is absent for a name that does not resolve or a declaration with no root
  table.
- **proto3** follows ADR-0017. A row is present only for a struct or a union
  payload, and it is always `Bounded`: typl bounds every collection, so proto3
  has no unbounded state. A named scalar and an enum set are inlined into their
  field (ADR-0017 decision 1, with decision 2 rejecting a wrapper message), and
  an enum is a declared `enum`, so none of the three has a message of its own,
  and their proto3 state is absent. The row is also absent when a member is one
  the proto backend refuses (a map key outside the integral and string scalars,
  a map value that is an array or a map, an optional array or map, an enum value
  outside int32, a field number protobuf reserves or exceeds), when a member has
  no proto3 bound (a `string` or `bytes` with no length bound, a type def with
  no width), when the nesting passes `MAX_DEPTH`, or when the bound overflows
  `u64` or exceeds `u32::MAX`. The rustdoc of `state` in
  `crates/ridl-ir/src/projection/size/proto3.rs` has the full list. The proto3
  sizer counts tags and varint widths at their maximum, and remembers each named
  struct and union it has sized within one call, so a type that several paths
  share is walked once.
- **repr(C)** has no row until driftsys/ridl#317 defines the layout.

**Absent by shape.** A stream payload (`<T>`), a request of zero or of several
parameters, and an inline `T | E` reply have no rows in any column, until a
codec defines their encoding. A payload `T?` is sized as `T`.

**Strings.** A `string [min..max]` bound counts Unicode scalar values, and its
byte capacity is four bytes per scalar value (`string_max_bytes`), in both
columns. A `match` pattern does not narrow it: the checker and the generated
Rust code do not yet agree on what a pattern matches, so an ASCII-only verdict
is not safe for a size bound (driftsys/ridl#665). A `string` or `bytes` with no
`len_max` has no bound in either column, as in `max_size`. A compiled package
always carries `len_max`.

The Rust backend's `PayloadInfo.max_size.flatbuffers` is the codegen model's
`Payload.flatbuffers_max_size`, the same `max_size` value. Its `proto3` stays
`None`, because that backend emits no proto3 codec.

**The codegen model carries the same states.** The model (`ridl.codegen.v1`)
writes a `PayloadSizes` message, one `SizeState` per encoding, on every payload
and on the request of every command and query and the reply of every query. It
differs from the descriptor in one respect: the descriptor writes no row for an
absent state, and the model writes the state, `absent` with an `AbsentCause`
(the sizer's own causes, unchanged), so that a plugin can tell a shape that no
codec defines from a toolchain too old to report a size. A query whose reply has
no return type takes the same absent state, with the cause
`ABSENT_CAUSE_ENCODING_UNDEFINED`. The model sums these states into two totals,
both per encoding:

- **`Interaction.reservation`** is the saturating `u64` sum, in bytes, of the
  bounded sizes of the member's payloads, in order: a signal, an event or a
  fixed member has its one payload, a command has its request, and a query has
  its request and then its reply. The first payload whose state is not bounded
  makes the reservation `unsized`, naming `<member>.<role>: <payload>`, where
  `<payload>` is the type's reference when the payload is one named type and
  otherwise what the shape is — a kind word, a parameter list or `()`. A member
  of a kind this toolchain does not know is `unsized` naming
  `<member>: no known payload shape`, never summed as zero. The field's own
  comment in `ridl/codegen/v1/model.proto` lists every shape.
- **`Interface.table_budget`** is the saturating sum of the reservations of the
  live members in `MEMBERS` order. It is `unsized`, naming the first member
  whose reservation is unsized. A tombstone does not count, and an interface
  with no live member has a budget of zero bytes.

These are the same two sums the runtime computes:
`ridl_rt::contract::Member::reservation` over a member's payloads, and the free
function `ridl_rt::contract::table_budget` over `Interface::MEMBERS`. The model
states them per wire encoding, so a plugin reads them without walking the
members itself.

The two sides agree only where the runtime holds the same per-payload sizes the
model holds. Today the Rust backend writes no proto3 size into the runtime
member table it generates: the `PayloadInfo::max_size` of that table carries
`proto3: None` for every payload (`descriptors.rs` in `ridl-backend-rust`), so
the runtime's proto3 reservation is `Unsized` for every member of a generated
table, while the model's proto3 column can carry a byte count. The catalog
descriptor this record describes is a different artifact, and it does write a
proto3 row (`lower.rs` in `ridl-descriptor`). The FlatBuffers column is the one
where the runtime table and the model read the same number.

## Verification before access, and `ridl describe`

`ridl_descriptor::verify(bytes)` is the only entry to a read. It checks, in this
order: at least 8 bytes (`TooShort`), the identifier at bytes 4..8
(`WrongIdentifier`), the root table and its `version` (`WrongVersion` for any
version but `SCHEMA_VERSION`), and then a walk that reads every field of every
table, vector and string once through the checked accessors (`Invalid`). A
buffer that fails any step is rejected as a whole. A view `verify` returns
cannot fail a later read on a malformed offset.

`ridl describe <PATH>` reads the file, verifies it, and prints the JSON that
`ridl_descriptor::describe::to_json` builds, pretty-printed. Every failure exits
2 with `error: <path>: <cause>` (ADR-0010 decision 1): a missing or unreadable
path, a file that is not a catalog descriptor, a version this toolchain does not
read, a malformed buffer, and a failed write to stdout, such as a pipe whose
reader has gone.

The JSON uses the schema's field names as keys, enum members by name, `hash` as
an array of bytes, and every field of a `MaxSize` row, defaults included. It is
**not** the view `flatc --json --strict-json --defaults-json` prints, in two
ways:

- the keys are in alphabetical order, because the workspace's `serde_json` has
  no `preserve_order` feature, and turning it on would change the key order of
  every other JSON writer in the workspace through feature unification;
- an absent `timing`, `min_us` or `max_us` is `null`, where `flatc` omits it.

An excerpt of the corpus snapshot
(`crates/ridl/tests/snapshots/describe_cli__corpus_catalog.snap`):

```json
{
  "kind": "Signal",
  "name": "currentSpeed",
  "ordinal": 1,
  "payloads": [
    {
      "max_sizes": [
        {
          "bytes": 46,
          "cause": "Unspecified",
          "encoding": "FlatBuffers",
          "state": "Bounded"
        }
      ],
      "role": "value",
      "type_name": "Speed"
    }
  ],
  "timing": {
    "max_us": "10000",
    "min_us": "10000",
    "mode": "StrictPeriodic"
  }
}
```

`Speed` is a named scalar, so it has no proto3 row.

## The port's catalog check

The generated face compares the port's catalog with the interface's `CATALOG`
once per binding (ADR-0023 decision 8). `Bind::new` of `Client<P>` and of
`Publisher<W>` makes the comparison before it stores the port, and `serve` makes
it on its handler port before it calls `Handler::serve`. The blocking client and
the blocking `serve` make it through the async ones. The comparison is
`CatalogRef` equality: the package name and the catalog hash. A mismatch panics
with a message that names the interface, the catalog the face was generated
from, and the catalog the port is attached to. `check_catalog`, both `Bind::new`
methods and `serve` are `#[track_caller]`, so the panic reports the program's
binding call. A program that must not panic compares
`port.catalog() == <Iface as Interface>::CATALOG` itself before it binds.

The Rust backend emits no interface descriptor type and no face for a service's
inline shape, although the catalog descriptor carries that shape
(`crates/ridl-backend-rust/src/descriptors.rs`).

## Tests

| What                                                                                | Where                                                                            |
| ----------------------------------------------------------------------------------- | -------------------------------------------------------------------------------- |
| The schema round trip, and the planus header behaviour `finish` works around        | `crates/ridl-descriptor/tests/round_trip.rs`                                     |
| The verifier: short, foreign, wrong version, truncated, flipped, every walked field | `crates/ridl-descriptor/tests/verify.rs`                                         |
| What the lowering writes, per kind and shape; stable bytes across runs              | `crates/ridl-descriptor/tests/lower.rs`                                          |
| The pinned corpus hash                                                              | `crates/ridl-descriptor/tests/golden_hash.rs`                                    |
| Every `MAX_SIZE` the Rust backend writes equals the descriptor's FlatBuffers bound  | `crates/ridl-descriptor/tests/codec_agreement.rs`                                |
| The two size states, the leaf model, the string capacity                            | unit tests in `crates/ridl-ir/src/projection/size.rs` and `size/`                |
| The JSON view                                                                       | unit tests in `crates/ridl-descriptor/src/describe.rs`                           |
| The emit, the hash equal to the face's, and `ridl describe` (exit codes, snapshot)  | `crates/ridl/tests/describe_cli.rs`                                              |
| The hash does not depend on which wire schema a build emits (driftsys/ridl#275)     | `crates/ridl/tests/facade.rs`                                                    |
| Each region carries its catalog's hash, `ridl.std` included                         | `crates/ridlc/tests/cli.rs`                                                      |
| The catalog check in the emitted face, and its panics                               | `crates/ridl-backend-rust/tests/face_generation.rs`, `tests/interaction_face.rs` |
| The planus boundary                                                                 | `xtask/tests/oracle_boundary.rs`                                                 |
| The accessors match the schema                                                      | `xtask/src/descriptor.rs`                                                        |

The FlatBuffers bound is also checked against an encoder: the round trip in
`crates/ridl-backend-rust/tests/flatbuffers_roundtrip.rs` encodes the largest
legal value within `MAX_SIZE`, and `codec_agreement.rs` ties `MAX_SIZE` to the
descriptor. No proto3 bound is checked against an encoder, because no proto3
codec exists yet.

## The design's decisions, as built

The archived design note keeps each decision's full reasoning. The table gives
the status in the code and the rejected alternatives in short form.

| Decision                                                     | As built                                                                                                                                                                                                                                               | Rejected                                                                                                                                                                      |
| ------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| D-1 Two descriptors; the system descriptor is self-contained | Half built: a catalog descriptor for every package with an interface shape. The system descriptor is not built.                                                                                                                                        | A system descriptor that references catalogs by hash only; one file per machine.                                                                                              |
| D-2 Descriptors are FlatBuffers; the IR stays protobuf       | Built. ADR-0014 is unchanged for the IR.                                                                                                                                                                                                               | Protobuf descriptors; JSON or TOML as the artifact; both encodings.                                                                                                           |
| D-3 Hand-written, versioned, append-only schemas             | Built for the one schema, `catalog.fbs`. The CI check that compiles the schemas with both `flatc` and `flatcc` is not built; it waits for a C engine.                                                                                                  | A descriptor schema generated from the IR schema.                                                                                                                             |
| D-4 What the catalog descriptor contains                     | Built, as "What a catalog contains" states. Of the member's bounds and quality-of-service terms, only the timing is carried, because it is the only one the IR has for an interaction. No `stream` flag (driftsys/ridl#336).                           | —                                                                                                                                                                             |
| D-5 What the system descriptor contains                      | Not built.                                                                                                                                                                                                                                             | —                                                                                                                                                                             |
| D-6 A size state per payload and encoding                    | Built for proto3 and FlatBuffers, as "The size states" states. `repr(C)` has no rows (driftsys/ridl#317). The max-size conformance test is built for FlatBuffers only.                                                                                 | One number per payload; a number per type rather than per interaction; an induced message, request message or `ok`/`err` union for a payload that is not a struct or a union. |
| D-7 Not in version 1                                         | Holds: no `.bfbs` payload layouts, no transport per crossing, no envelope or framing overhead.                                                                                                                                                         | —                                                                                                                                                                             |
| D-8 Readers verify before access                             | Built: `verify`, and `ridl describe` exits 2 with the cause named.                                                                                                                                                                                     | —                                                                                                                                                                             |
| D-9 Emission and inspection                                  | The catalog emit and `ridl describe` are built. The system descriptor's emission is not built. `ridl describe`'s view differs from `flatc`'s in key order and in `null` for an absent field, where the design said the two views are the same.         | —                                                                                                                                                                             |
| D-10 Generated code keeps its identity table                 | Built: each generated interface descriptor type carries `CATALOG` (name and hash), `NUMBER` and `PROVISIONAL`, and the face checks the catalog. A runtime's node descriptor derived from a system descriptor is outside this repository and not built. | —                                                                                                                                                                             |

### Not built

- **The system descriptor**: the second half of D-1, all of D-5, the second
  bullet of D-9, and the checks of the design's §3 that apply to it (a lowering
  error that stops a deployment's descriptor, and `ridl describe` comparing a
  system descriptor's catalog hashes with its embedded catalogs). Its file
  identifier and extension are not chosen. Decided on 2026-10-05
  (driftsys/ridl#715): it stays unbuilt. The per-deployment facts a backend
  needs reach it through the codegen request's deployment section, which is
  built ([ADR-0022](../decisions/ADR-0022-rsdl-system-in-the-ir.md) decision 11,
  [`codegen-plugins.md`](codegen-plugins.md)); only the transport bindings'
  overheads are missing from it (driftsys/ridl#718). A descriptor file would be
  a second emitter over the same system IR, built when a runtime that reads one
  exists.
- **Payload layouts** as a `.bfbs` reflection schema (D-7).
- **The `stream` flag** and the per-element bound of a stream payload
  (driftsys/ridl#336), to be appended to the schema.
- **The cross-compiler check** of the schemas with `flatc` and `flatcc` (D-3).

For these, the archived design note is the record of the agreed direction.

## Open items and debt

| Item                                                                                                       | Issue             |
| ---------------------------------------------------------------------------------------------------------- | ----------------- |
| Narrow a string's byte capacity from an ASCII-only match pattern, after #597                               | driftsys/ridl#665 |
| ridl-rt: streams (ridl §12) have no port; the stream flag and per-element bound follow it                  | driftsys/ridl#336 |
| The repr(C) payload codec, which gives the `ReprC` column its rows                                         | driftsys/ridl#317 |
| debt(ridl-descriptor): Windows schema paths and an unpinned guard in the schema tooling                    | driftsys/ridl#670 |
| debt(ridl-ir): catalog hash follow-ups deferred from the review of #676                                    | driftsys/ridl#679 |
| debt: review follow-ups — one name resolver, one proto3 width table                                        | driftsys/ridl#684 |
| debt: review follow-ups — shared proto3 refusal rules, agreement tests                                     | driftsys/ridl#690 |
| debt: deferred review items from PR #692 (the lowering and the catalog check)                              | driftsys/ridl#693 |
| debt: deferred review items from PR #696 (ridl describe)                                                   | driftsys/ridl#697 |
| ridl diff reports identical when ridl lock freezes a provisional number, although the catalog hash changes | driftsys/ridl#700 |
| debt: deferred review items from PR #699 (the catalog hash per region)                                     | driftsys/ridl#701 |

## Trace

- Roadmap: [`../ROADMAP.md`](../ROADMAP.md)
- Decisions: [ADR-0014](../decisions/ADR-0014-ir-encodings.md) decisions 4 and
  15,
  [ADR-0020](../decisions/ADR-0020-third-encoding-runtime-layering-and-plugin-system.md)
  decision 5, [ADR-0023](../decisions/ADR-0023-interaction-face-generation.md)
  decision 8, [ADR-0010](../decisions/ADR-0010-cli-conventions.md),
  [ADR-0022](../decisions/ADR-0022-rsdl-system-in-the-ir.md) decision 7,
  [ADR-0017](../decisions/ADR-0017-proto3-projection-rules.md),
  [ADR-0019](../decisions/ADR-0019-flatbuffers-projection-rules.md) decision 8
- Design records: [`flatbuffers-codec.md`](flatbuffers-codec.md),
  [`ridl-rt.md`](ridl-rt.md), [`interaction-face.md`](interaction-face.md)
- The reasoning, the plan and the lane:
  [`../archive/2026-09-13-runtime-descriptors-design.md`](../archive/2026-09-13-runtime-descriptors-design.md),
  [`../archive/2026-09-13-catalog-descriptor-plan.md`](../archive/2026-09-13-catalog-descriptor-plan.md)
  and
  [`../archive/2026-10-03-lane-e16-catalog-descriptor-driver.md`](../archive/2026-10-03-lane-e16-catalog-descriptor-driver.md)
