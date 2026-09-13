# Runtime descriptors — the catalog descriptor and the system descriptor

Status: working note, 2026-09-13, design agreed in conversation the same day.
Nothing here is ratified. Read after
[`2026-09-12-release-scope-and-plugin-system-design.md`](2026-09-12-release-scope-and-plugin-system-design.md)
§3.8 and §3.13,
[`2026-09-08-topology-vocabulary.md`](2026-09-08-topology-vocabulary.md) §6, and
[`2026-09-12-rsdl-rewrite-decisions.md`](2026-09-12-rsdl-rewrite-decisions.md)
D-6 and D-7.

Why it exists: the records fix the IR for the toolchain (ADR-0014) and list what
the system IR carries (design note §3.13), but no record defines a file that an
engine or an integrator outside the toolchain reads. A bus configurator, a
SOME/IP stack, a broker that hosts many catalogs, or a gateway is not a codegen
backend and does not receive the plugin contract's request. Generated Rust
serves a component that links it; an engine that is not written in Rust, or that
loads catalogs it was not compiled against, needs the same facts as data: what
is addressed, who produces it, where it runs, and how large each payload can be.
This note defines those two files, their encoding, their contents, and where
they are emitted.

Satisfies: design note §3.13 ("a runtime's descriptor is then an emitter over
the IR") and §3.8; vocabulary note §6 and V-15 to V-17; rsdl decisions D-6
(attribute map per node) and D-7 (interface numbers folded into the IR).

## 1. The shape

Two artifacts, lowered from the IR, read by engines:

    catalog descriptor   per package     what a catalog contains and how it is
                                         numbered; the unit a broker loads
    system descriptor    per deployment  who produces what, on which machine,
                                         over which crossing; self-contained,
                                         embeds the catalog descriptors of the
                                         system's closure

    source (.typl, .ridl, .rsdl)
      │  ridlc / the rsdl lowering
      ▼
    IR — protobuf, ADR-0014            ──▶  codegen backends, in-process or
      │                                     plugins, through the codegen model
      │  the lowering: numbers folded         of design note §3.8
      │  from the lock, sizes derived,
      │  system facts derived
      ▼
    descriptors — FlatBuffers          ──▶  engines and integrators: ridl-rt's
                                            node descriptor, a bus configurator,
                                            a broker, a gateway, a C engine

The IR is the toolchain's artifact and stays as ADR-0014 fixed it. A descriptor
is the engine's artifact: a flat, indexed lowering of the same facts, not a
second encoding of the IR.

## 2. Decisions

### D-1 Two descriptors; the system descriptor is self-contained

**Decision.** The compiler emits a catalog descriptor for every package that
declares at least one interface, and a system descriptor for every `deployment`
of a `system`. The system descriptor embeds the catalog descriptors of the
system's closure, so an engine that configures a deployment reads one file. The
standalone catalog descriptor is still emitted, for a consumer that hosts
catalogs without a deployment: a broker, a test harness, a schema registry.

**Rejected.** (a) The system descriptor references catalogs by hash only, and
the engine loads the package files: two file lookups and a resolution rule on
the engine's side for what the compiler already has in hand; the hash stays in
the system descriptor as the identity, embedding adds the content. (b) One file
per machine: a per-machine node view is a filter over the system descriptor
(`machine` on every placement), and a second schema for it would drift.

### D-2 Descriptors are FlatBuffers; the IR stays protobuf

**Decision.** Both descriptors are FlatBuffers binaries. A reader maps the file
and reads tables in place: no parse step, no allocation, no decode into a heap
structure. ADR-0018 decision 3's rule — proto3 on the network, FlatBuffers in
memory — places an artifact that is loaded into memory and read there on the
FlatBuffers side. Readers exist with a buffer verifier in C (`flatcc`), C++,
Rust and Swift; C and C++ also carry JSON and reflection. ADR-0014 is not
amended: the IR that backends, plugins, `ridl diff` and the baseline consume
remains canonical protobuf binary with its JSON and prototext forms.

**Rejected.** (a) Protobuf descriptors: one encoding for everything, but a
protobuf reader decodes into an owned structure before the first field is read,
which is the cost an engine on a constrained node pays at startup and a broker
pays per catalog. (b) JSON or TOML as the artifact: a text format is an
inspection form, not a zero-copy one; design note §3.1 already retired the TOML
descriptor as a user-facing surface. (c) Both encodings: a second schema of the
same content to keep in step, with no consumer asking for it.

### D-3 The schemas are hand-written, versioned, and append-only

**Decision.** Two `.fbs` schema files, hand-written, held in the workspace
beside the IR's `.proto` (the crate is an open item, §7). Each root table
carries a `version` field and each file a FlatBuffers `file_identifier`, so a
reader rejects the wrong kind of file before verifying it. Evolution is by
appending fields and tables, never by renumbering or removing: the same
discipline the frame specification (roadmap E11.1) holds, because a descriptor
written by one toolchain version is read by an engine built against another. The
schemas stay within the subset both `flatc` and `flatcc` accept; when a C engine
exists, CI compiles the schemas with both.

ADR-0019's projection rules bind the FlatBuffers backend's output, not these
schemas; two of them are kept as practice anyway: every composite is a `table`,
and a union is isolated in a wrapper table.

**Rejected.** Generating the descriptor schema from the IR schema: the
descriptor is a lowering with a different shape (flat, indexed by interface and
member, sizes and producers resolved), and a generated schema would carry the
IR's shape into every engine.

### D-4 What the catalog descriptor contains

- **Identity of the catalog**: name (the package name, V-16); the catalog hash
  (D-8 of the rsdl note: over the interfaces, their numbers, and the types they
  reach); the toolchain version that wrote it.
- **Interfaces**: name; the frozen number from the lock file; a `provisional`
  flag when the number is not yet frozen (rsdl note D-7); the retired entries as
  name and number, so an engine can refuse a peer that still speaks a retired
  interface.
- **Members, per interface**: name; ordinal (position in the body, ridl §11);
  kind (`signal`, `event`, `command`, `query`); the payload type name per
  payload; the bounds and QoS terms the IR carries for the member (ADR-0015);
  and the **max-size table** of D-6.
- **Reserved ordinals** per interface, so the ordinal space is complete.

Not contained: type layouts, field lists, constraints beyond the bounds that
size a payload. A consumer that needs a payload's shape reads the projected
schema the wire backend emits (D-7).

### D-5 What the system descriptor contains

- **The embedded catalog descriptors** of the closure, and the catalog hashes as
  the identity the routing key's slot resolves to (vocabulary note §6).
- **Machines**: name; the `external` flag; `labels`.
- **Components and instances**: component name; the `external` flag; instance
  names, `Unit` for the implicit one (rsdl note D-4); the machine each instance
  is placed on in this deployment.
- **Producers**: for every service in the closure, the offering component and
  its instances; a redundant provider set is visible as more than one instance
  and carries the not-yet-realizable marker the lowering reports (D-4).
- **Links**: for every `requires`, the consumer instance, the producer instance
  reached through the owning service, and the crossing kind — same machine,
  different machine, off-board (rsdl note D-2, D-5).
- **The routing table**: (catalog, interface number, member ordinal) to
  producer, the flat form of the IR's routing table (design note §3.13).
- **Grants**: per component, the set of catalog regions its requirements reach
  (rsdl note D-9); the surface set, the system's external boundary.
- **The attribute map per node** (rsdl note D-6): the namespaced backend keys as
  declared, uninterpreted, on every node that may carry them — machine,
  component, instance, placement, link. A tag-based transport's service number
  (`someip.service_id`) reaches its stack through this map, which discharges the
  registry ADR-0016 decision 8 deferred.

Not contained: transport choice per crossing, network fabric, process-local
facts (file paths, ports, tuning), envelope and framing overhead. Design note
§3.13 keeps those in the runtime's own configuration; rsdl note D-5 keeps the
fabric out of rsdl.

### D-6 Every payload carries its maximum encoded size, per encoding

**Decision.** For every interaction, the catalog descriptor carries one row per
payload the kind has, and each row carries one number per core encoding:

    kind      payloads
    signal    1   the value
    event     1   the occurrence
    command   1   the request; the ack is empty and lives in the envelope
    query     2   the request, the response

    row       proto3 · FlatBuffers · repr(C)      max encoded size, bytes

The crossing kind decides which column an engine reads: a signal's last-value
slot in a shared-memory store is sized from the in-memory column, the same
signal on a bus from the network column. A new core encoding appends a column.

**Derivation.** One new derivation in the lowering, from typl bounds, in bytes.
Every payload is finite: variable-size collections require explicit bounds (typl
§12, TYPL-201/202), `string` and `bytes` default to `[0..256]` (typl §4), and
recursion is rejected because it makes the wire size unbounded (typl §7.3). A
`string [min..max]` bound counts characters; its byte bound is four bytes per
character under UTF-8 unless a `match` constraint restricts the character set to
one whose encoding is narrower, in which case the narrower bound applies. Each
encoding's overhead — proto3 tags and varint widths at their maximum,
FlatBuffers vtables, offsets and alignment padding, `repr(C)` layout — is part
of the number. Nothing in the crates computes an encoded size today; design note
§3.8's "widths derived" covers scalar widths only.

**Refutation.** For every core encoding, a conformance test encodes the largest
legal value of every payload in a fixture package and asserts the encoded length
is at most the descriptor's number. The proto3 and FlatBuffers codec stories
(roadmap E11.8, E11.7) carry the conformance harness the test extends.

**Rejected.** (a) One number per payload, the largest across encodings: simpler,
and it oversizes every in-memory slot to the proto3 bound. (b) Per type rather
than per interaction: the descriptor is looked up by (interface, member), and
two interactions sharing a type cost one repeated row.

### D-7 Not in version 1

- **Payload layouts.** A FlatBuffers `.bfbs` reflection schema of the catalog's
  types would let an engine read FlatBuffers payload fields by name with no
  generated code, zero-copy as well. It is a third decode path beside the
  generated codecs and ADR-0018's ridl-less reader, it covers only FlatBuffers
  payloads, and it adds `flatc` to the toolchain. Version 1 names the payload
  type; the layout arrives with the first engine that reads payloads through the
  descriptor.
- **Transport per crossing and the fabric** (rsdl note D-5).
- **Envelope and framing overhead**: the frame specification's (E11.1) and the
  transport's; an engine adds it to the payload size.

### D-8 Readers verify before access

**Decision.** A descriptor is a file loaded from storage. Every reader —
`ridl
describe`, `ridl-rt`, an engine — runs the FlatBuffers verifier on the
buffer before the first zero-copy read, and checks the `file_identifier` and
`version` first. A buffer that fails is rejected as a whole. The toolchain's own
reader reports the rejection under ADR-0010's exit-code taxonomy with the cause
named, the way an unreadable input is reported today; an engine's policy on a
provisional interface number (D-4) is its own.

### D-9 Emission and inspection

- **Catalog descriptor**: `ridlc build --emit catalog`, one file per package,
  beside the IR emits of ADR-0014 decision 4. It depends on the lock file of
  rsdl note D-7 for frozen numbers; before the lock exists, every number is
  provisional and flagged so.
- **System descriptor**: emitted by the rsdl lowering, one file per
  `deployment`. It is Epic 6's exit artifact: the roadmap's "the IR carries the
  region map, the link set, the routing table, the permission list, the surface
  set and the catalog hash" becomes "the lowering emits the system descriptor
  carrying them".
- **Inspection**: `ridl describe <file>` prints the descriptor as strict JSON,
  walking the buffer with the generated accessors; no JSON emit exists, and
  `flatc --json --strict-json` against the schema gives the same view outside
  the toolchain. `ridl describe` follows ADR-0010 and earns its exit-code row
  when added.
- **File names**: open (§7); the `file_identifier` is the authority on kind, the
  extension a convenience.

### D-10 Generated code keeps its identity table; the descriptor is the engine's

**Decision.** A component that links generated code keeps the ordinal table
ADR-0013 decision 3 requires, extended with the interface number and the catalog
hash so generated code and descriptor agree on identity. The descriptor is the
alternative for an engine, not a replacement for generated code. `ridl-rt`'s
node descriptor (roadmap E11.0, design note §3.13) reads the system descriptor;
whether it embeds the bytes at build time or loads them at start is an
implementation choice (§7).

## 3. Error handling

- **Lowering errors precede emission.** A service with no offering component, an
  instance placed twice or not at all, an unresolved `requires`, an unclaimed
  backend namespace: the rsdl rules report these (rsdl note D-3, D-6) and no
  descriptor is written for that deployment.
- **A provisional number is data, not an error**: flagged in the descriptor;
  `ridl baseline` keeps its refusal (rsdl note D-7); an engine decides.
- **A corrupt or foreign file** fails the identifier check or the verifier and
  is rejected whole (D-8).
- **A descriptor and its embedded catalogs disagree** (a system descriptor
  assembled from stale catalog descriptors): impossible by construction, because
  one lowering writes both from one IR; the catalog hash in the system
  descriptor equals the hash inside the embedded catalog, and `ridl describe`
  checks it.

## 4. Testing

- **Schema round trip**: for each schema, build a descriptor with the Rust
  builder, verify it, read every field back, in the workspace test suite.
- **Golden files**: a fixture system lowered to both descriptors, checked in,
  byte-stable across runs; a change to the schema or the lowering shows as a
  diff of the golden file.
- **Max-size conformance** per encoding (D-6).
- **Verifier rejection**: a truncated and a bit-flipped golden file are rejected
  by `ridl describe` with the exit code ADR-0010 assigns.
- **Cross-compiler subset**: `flatc` and `flatcc` both compile the schemas;
  deferred until a C engine exists (D-3).
- **Book examples**: a `describe` transcript in the book, under the
  book-examples harness where a fence is compiled.

## 5. Alternatives considered

Recorded per decision above; collected here so the rationale survives gardening:
protobuf as the descriptor encoding (D-2); a text artifact (D-2); a generated
descriptor schema (D-3); catalogs by reference only, and per-machine files
(D-1); a single maximum size, and sizes per type (D-6); payload layouts in
version 1 (D-7).

## 6. Amendments implied for existing records

- **ADR-0018**: the lowering emits the two descriptors; `ridl-rt`'s node
  descriptor reads the system descriptor; decision 3's encoding rule gains the
  descriptor as an in-memory artifact on the FlatBuffers side.
- **ADR-0013 decision 3**: the generated ordinal table carries the interface
  number and the catalog hash.
- **ADR-0016 decision 8**: the registry for tag-based service numbers is
  discharged by the attribute map in the system descriptor (D-5).
- **ADR-0014**: unchanged; cited as the boundary between the IR's encodings and
  the descriptors'.
- **ADR-0010**: `ridl describe` and the `catalog` emit value earn their rows.
- **Design note** §3.13: the content list becomes the system descriptor's; §3.8
  gains the second consumer class beside codegen backends.
- **Roadmap**: Epic 6's exit criteria name the system descriptor; a story for
  the catalog descriptor and the size derivation lands with the ridl
  finalization (Epic 14), after the lock file; E11.0's "interaction descriptors"
  are reconciled with D-10.
- **rsdl note** D-6: "every extract a backend reads carries [the attribute map]"
  names the system descriptor as that extract.

## 7. Open

- Where the schemas and the generated accessors live: a `ridl-descriptor` crate
  beside `ridl-ir`, or inside `ridl-ir`.
- File extensions and the two `file_identifier` values, under ADR-0010's naming.
- Whether the first bus configurator needs QoS terms the IR does not yet carry;
  decide against the first consumer, not in advance.
- Whether `ridl-rt` embeds the system descriptor at build time or loads it at
  start (D-10).
- When payload layouts enter (D-7).
