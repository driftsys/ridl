# Layout inputs for backend plugins — the design (E17.0)

Status: approved in conversation on 2026-10-05; under review as a document. Lane
S, stage S1. Stories: E17.0 to E17.5, driftsys/ridl#715 to driftsys/ridl#720.
Satisfies: driftsys/ridl#715. Working memory under the
[lifecycle rule](../wip/README.md); the durable records are the amended ADRs,
the rsdl reference, `docs/design/codegen-plugins.md` and
`docs/design/catalog-descriptor.md`.

## 1. Goal

A backend plugin has enough information to compute and generate a deployment's
shared-memory layouts and socket message layouts from its `CodegenRequest`
alone. The toolchain supplies the inputs; the plugin computes the layout. Two
plugins given the same request, implementing the same transport binding, compute
the same sizes.

Exit test of the epic: a test plugin, given only the request for
`examples/cabin`'s deployment, computes each region's byte layout and each
socket channel's maximum message size, and the values match a fixture checked by
hand (§9).

## 2. Decisions taken before this design

These were taken by Sebastien on 2026-10-05 and recorded in the roadmap's
Epic 17. This design does not reopen them.

1. The sizing inputs are rsdl deployment attributes: a queue depth override, a
   slot count and a call budget. They are not `ridl` syntax and not backend
   options.
2. The request does not choose between shared memory and socket (rsdl reference
   §1.4). It carries each link's crossing; the plugin decides.
3. Envelope and frame header sizes are set per transport binding (frame
   specification §10 item 2). E17.3 waits on driftsys/ridl#265, the WebSocket
   binding; nothing else in the lane does.
4. E17.1 adds an rsdl system with one deployment to `examples/cabin`.
5. The engine stays parked as `ridl-engine`. Only the inputs move into the
   toolchain.

Three choices that change the rsdl surface were put to Sebastien during this
design and are recorded as taken (§8, items S-1 to S-3): what the slot count is,
where the keys attach, and the key spellings with the depth rule.

## 3. Vocabulary

- **Channel**: one interaction member of one interface, produced by one instance
  in one deployment. A channel has one producer and zero or more consumers. Two
  instances of a redundant provider set (RSDL-409) give two channels for the
  same member.
- **Consumer link**: one consumer instance of a channel, with the crossing of
  the link that reaches it (rsdl §10).
- **Depth**: the number of occurrences an event channel's ring holds. Signals
  have no depth (ADR-0018 decision 12: they are the store).
- **Slots**: the number of entries of a consumer's call table for a call
  channel, `N` of `ridl_rt::correlate::Table<N>` (ADR-0021 decision 15). It
  bounds the calls one consumer has in flight.
- **Budget**: the optional byte budget of that table, debited at insert from
  `Member::reservation` (ADR-0021 decisions 15 and 17). `docs/design/ridl-rt.md`
  states that no specification defines it; this design does.
- **Reservation**: a call member's request bound plus its reply bound for one
  encoding; **table budget**: the sum of the reservations of an interface's call
  members (`ridl_rt::contract::table_budget`).

## 4. Decisions

Each decision states what is decided, why, and the alternatives considered.

### D-1. The deployment section is a field of the request, beside the model

`CodegenRequest` gains `optional Deployment deployment = 6`. `Deployment` is a
new message of package `ridl.codegen.v1`, in a new file
`crates/ridl-ir/proto/ridl/codegen/v1/deployment.proto`, imported by
`plugin.proto`. The change is additive under the IR stability design D-5 and the
IR specification §6: a new field with a never-used number, whose absence means
what today's request means (no deployment). A request written with no deployment
is byte for byte today's request. The toolchain's own generated reader stays
strict and rejects an unknown key (ADR-0014); the in-tree plugins are rebuilt
with it, and a plugin outside this workspace reads leniently, as the IR
specification §8 and the model's own field comments already require.

`Model` is unchanged by this decision. It stays one package lowered over a
stated scope, so `ridl build --emit codegen-model`, `ridl baseline` and every
parity test keep their artifact.

Every request of one build carries the same deployment section, whichever
package the request is for. A plugin generating package P finds P's regions and
P's messages in the section, and their payload sizes in P's model (D-7).

**Alternatives considered.**

- A `Deployment` field inside `Model`. Rejected: the per-package model would
  then vary with a command-line selection, and `--emit codegen-model` would no
  longer be a function of the package and its scope.
- A second plugin call, `generate_deployment`, carrying the system. Rejected:
  ADR-0020 decision 9 fixes one contract, `generate(CodegenRequest)`, and a
  plugin that needs both the model and the deployment would correlate two calls.
- A request per deployment with no model. Rejected for the same reason, and
  because a region is a catalog is a package: the layout of a region needs the
  package's sizes, which are in its model.

### D-2. What the deployment section carries

The section is an emitter over the lowered system IR (ADR-0022 decision 1: a
later emitter, not a second lowering). It carries, in a fixed order so that two
runs write the same bytes:

1. **Identity**: the system's qualified name and the deployment's name.
2. **Regions**, in catalog name order: catalog name, the 32-byte catalog hash,
   and the interfaces closure services list, in interface number order, each
   with name, number, `inline` and `provisional`.
3. **Instances**, in the system IR's placement order: component, instance,
   machine, `external`; the interfaces the instance offers (the regions whose
   slots it writes); the catalogs it requires into (the regions it maps, from
   the permission list).
4. **Channels**, in (catalog, interface number, member ordinal, producer
   component, producer instance) order: the key and the names beside it, the
   member's kind, the producer endpoint, and the consumer links in (component,
   instance) order. Each consumer link carries its crossing, its encoding (D-4),
   and the sizing values of the channel's kind: `depth` for an event (D-5),
   `slots` and `budget` for a command or a query (D-6). An event channel also
   carries its ring depth, the maximum over its consumer links.
5. **Bindings** (D-9), in binding name order.

Producer and consumer counts are the lengths of these lists; the section does
not repeat them as numbers.

**What it does not carry.** Payload sizes (they are in the model, D-7). The
transport over a crossing, a process, an address space, or whether a region is
mapped or framed: rsdl declares none of these (§1.4, §10), and the plugin
decides them. Backend keys of the attribute map: a plugin that reads
`someip.serviceId` reads it from the system IR dump, as today, until a need to
carry it here is recorded.

**Alternatives considered.** Copying each channel's payload size states into the
section, so that a plugin for package P could lay out another package's region.
Rejected: it duplicates the model, and the request for the other package is
where that region is generated.

### D-3. The tabulation boundary

The toolchain tabulates every value that is a function of the IR, the lock, the
deployment and the rsdl attributes alone. The plugin computes every value that
depends on a transport binding or on a layout policy of its own.

| The toolchain tabulates                                         | The plugin computes                                         |
| --------------------------------------------------------------- | ----------------------------------------------------------- |
| regions, their hash and their interfaces                        | the byte offset of each slot in a region                    |
| instances, their machine, what they offer and what they map     | which process maps which region                             |
| channels, their producer and their consumer links               | whether a same-machine channel is mapped or framed          |
| the crossing and the encoding of each consumer link (D-4)       | alignment and padding                                       |
| the depth of each event channel and consumer link (D-5)         | the slot size: payload bound plus per-slot overhead         |
| the slots and the budget of each call consumer link (D-6)       | the table storage: slots times reservation, or the budget   |
| per-encoding size states of every payload shape (D-7)           | the maximum message size: header plus envelope plus payload |
| the reservation of each call member and the table budget (D-8)  |                                                             |
| the frame header and envelope sizes of each known binding (D-9) |                                                             |

This is ADR-0020 decision 8 applied to the deployment: lower once, so that the
plugin is a printer of layouts and does not re-derive timing, fan-out or the
encoding matrix. The slot size is on the plugin's side because the per-slot
overhead (generation counter, provenance, envelope placement) is what the frame
specification §10 item 8 leaves to the binding.

**Alternatives considered.** Tabulating a slot size per channel in the
toolchain. Rejected: it needs a binding's per-slot overhead and an alignment
rule, neither of which the toolchain owns; carrying the binding's overheads as
inputs (D-9) gives the plugin the same determinism without the toolchain
choosing a layout. Tabulating nothing beyond the raw system IR. Rejected: every
plugin would re-derive depth and the encoding matrix, the failure ADR-0020
decision 8 names.

### D-4. The encoding of a consumer link derives from its crossing

ADR-0020 decision 2's matrix, applied to rsdl §10's crossing kinds:

| Crossing            | Encoding    | Matrix row                                              |
| ------------------- | ----------- | ------------------------------------------------------- |
| `same machine`      | FlatBuffers | mapped or passed within a node, a local socket included |
| `different machine` | proto3      | serialized into a stream                                |
| `off-board`         | proto3      | serialized into a stream                                |

No consumer link gets `repr(C)` in this epic: that encoding has no layout until
driftsys/ridl#317, and no rsdl fact selects it. When it lands, a backend key
selects it per link and the matrix row is added here.

A plugin honours the tabulated encoding. The same-machine choice between a
shared-memory store and a local socket stays the plugin's (decision 2 of §2);
both carry FlatBuffers, so the choice does not change the sizes.

**Alternatives considered.** Leaving the encoding to the plugin. Rejected: two
plugins would then size the same channel differently, against the goal of §1.

### D-5. The depth rule: the contract bound is the derived depth, the rsdl key replaces it

The two recorded rules bound different things.

- ADR-0015 decision 21, `depth = ceil(max / min)` over the event's resolved
  timing, bounds the occurrences alive at once: the one after the bound cannot
  exist before the first has expired. It needs only the contract.
- ADR-0018 decision 12, `depth ≈ ceil((service_period + jitter) / rate_floor)`,
  bounds the occurrences one consumer leaves unserviced between two services. It
  needs a service period from rsdl and a jitter from the target, and neither has
  a source: RSDL-801 is reserved until timing feasibility reopens (rsdl §12),
  and the IR has no field for either.

When a consumer is feasible (`service_period + jitter ≤ max`), the second value
is at most the first. The toolchain therefore derives the contract bound as
every event channel's depth: it is the worst case for any feasible consumer and
needs no deployment input. The consumer rule is not evaluated in this release;
it is the justification of the override, which is how a deployment states a
consumer-side result today.

The rule, per consumer link of an event channel:

1. If the link has a declared `depth` (D-6 precedence), that is the link's
   depth, with source `DECLARED`.
2. Otherwise, if the event's resolved timing has both bounds, the depth is
   `ceil(max_us / min_us)` with source `DERIVED`. Resolved timing includes the
   §9.1 defaults, so an event written with no timing has both bounds.
3. Otherwise (an explicit half-open range), the depth is absent with source
   `UNDERIVABLE`, and the build draws the warning `depth-underivable` (D-6).

The channel's ring depth is the maximum over its consumer links' depths; it is
absent when any link's depth is absent. A declared value below the contract
bound draws the warning `depth-below-bound`, naming the event, the declared
value and the bound, because live occurrences can then be dropped. A declared
value above the bound is accepted silently.

`ceil(max_us / min_us)` is computed on the exact-decimal microsecond strings of
`Timing`, as integers after scaling, never in floating point. A `min` of zero
cannot occur: the checker refuses it.

**Alternatives considered.**

- The declared value as a floor, `max(derived, declared)`. Rejected (Sebastien,
  2026-10-05): a deployment could not trade depth for memory, and a value below
  the bound would be silently ineffective.
- Replacing with no diagnostic. Rejected: a value below the bound is a decision
  worth seeing.
- Evaluating ADR-0018 decision 12 with a default service period and jitter.
  Rejected: the numbers would be invented, and the rule's own text defers them
  to measurement.
- Choosing ADR-0018 decision 12 as the only rule and marking the depth absent
  until rsdl carries a service period. Rejected: every event channel would be
  unsized in this release, while the contract bound is derivable today.

### D-6. The three attributes

**Keys and meaning.** Three rsdl-owned keys, bare words in the style of
`instances` and `tier`:

| Key      | Applies to                 | Value                  | Range           | Default                                       |
| -------- | -------------------------- | ---------------------- | --------------- | --------------------------------------------- |
| `depth`  | event channels             | integer, occurrences   | 1 to 4294967295 | derived (D-5)                                 |
| `slots`  | command and query channels | integer, table entries | 1 to 65536      | 16                                            |
| `budget` | command and query channels | integer, bytes         | 1 to 2^64 − 1   | none: only `slots` bounds the calls in flight |

The `slots` ceiling is ADR-0021 decision 15's correlation bound,
`(generation << 16) | slot`. The `slots` default is the value `ridl-loopback`
runs with (ADR-0021 decision 15). The `budget` default is ridl-rt's meaning of a
table built with `new(None)`.

**Sites.** The keys are legal on the `deployment` declaration and on a placement
line (an instance on a machine). A value on the deployment applies to every
consumer link of that deployment. A value on a placement line applies to every
link that instance consumes, and takes precedence over the deployment's value.
Precedence, per consumer link: placement line, then `deployment` declaration,
then the default. The keys are deployment-scoped because the values depend on
the deployment, not on the interface (§2 item 1); the placement line is the
finest deployment-scoped grain that exists. This amends rsdl §5's rule that
lines take backend keys only: a placement line takes these three rsdl keys too.

**Grain.** No per-link and no per-member grain in this release. A per-member
override would need an attribute value form keyed by a member name, which the
family's general form does not have; a per-link, per-deployment site would be a
new declaration shape inside `deployment`. Either reopens when a deployment
records a need the two sites cannot express.

**Parsing and checking.** The value is an integer literal (the parser's
`is_value_token` already refuses a duration there; `50ms` draws FORM-101). A key
on any other declaration or line is FORM-107; a key twice in one block is
FORM-108. A value that is not an integer within the key's range is a new error,
RSDL-709, naming the key, the written value and the range. It is a 7xx code
because the sites are deployment-scoped: it blocks only its deployment (ADR-0022
decision 8). A `depth`, `slots` or `budget` declared where no channel of the
matching kind exists draws nothing.

**Diagnostics.** Three new codes, numbers taken from the free list of the RSDL
catalogue (rsdl §16: 709 and 805, 806 are unused, reserved by nothing, retired
by nothing):

| Code     | Severity | Lint name           | When                                                                                                                                                                                                                                                                                                                  |
| -------- | -------- | ------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| RSDL-709 | error    | —                   | a `depth`, `slots` or `budget` value is not an integer within its range                                                                                                                                                                                                                                               |
| RSDL-805 | warning  | `depth-below-bound` | a declared `depth` is below `ceil(max / min)` for an event a covered link consumes                                                                                                                                                                                                                                    |
| RSDL-806 | warning  | `depth-underivable` | an event with an explicit half-open range is consumed by a link with no declared `depth` **Superseded by DD-42** (§8): the code fires for every contract bound that cannot be derived, not only an explicit half-open range, and only for a link with no declared `depth` on its placement line or on its deployment. |

The RSDL-806 row above keeps its original wording; DD-42 in §8 widens its
condition and is the as-built rule.

The two warnings are lints under ADR-0024: a catalogue row with a name, a row in
`docs/book/lints.md`, default level `warn`.

**Lowering.** The declared values enter the system IR as dedicated fields, as
`instances` and `external` do, not as entries of the attribute map (rsdl §13
item 9 carries backend keys only). `ridl.ir.v2.Deployment` gains
`optional Sizing sizing`, and `ridl.ir.v2.Placement` gains the same; `Sizing`
holds the three values as `optional` fields. Additive under D-5. The
deployment-section emitter (D-2) resolves the precedence and the derivation and
writes each consumer link's values with their source (`DECLARED`, `DERIVED`,
`DEFAULT`, `UNDERIVABLE`).

**Alternatives considered.**

- `queueDepth`, `callSlots`, `callBudget`: rejected (Sebastien, 2026-10-05) as a
  style no rsdl-owned key uses.
- `ridl.depth` and the like, a toolchain namespace: rejected, because §5 says a
  namespaced key is carried uninterpreted, and a carve-out would blur the rule.
- The `requires` line as the site: rejected, because it is closure-level and the
  same in every deployment.
- A `link` line inside `deployment`: rejected for this release as a new
  declaration shape.
- Deriving `slots` from the call's timing, `ceil(max / min)` when both call
  bounds are declared: rejected, because a call's `min` is undeclared in the
  normal case (ADR-0015 decision 4) and one rule with one default is simpler to
  read in a request.
- A backend option carrying the three values: rejected before this design (§2
  item 1).

### D-7. Size states in the model, for every payload shape and both encodings

The model today carries one bound, `Payload.flatbuffers_max_size`, for a payload
that is one named type. The catalog descriptor record ("The size states") has
the richer model: per payload and encoding, bounded, unbounded with a cause, or
absent. The codegen model adopts it, with the absence made explicit and given a
cause, because a plugin must be able to tell "no bound exists" from "this
toolchain did not compute one".

`SizeState` is one of:

- `bounded`: the maximum encoded size in bytes, envelope and framing excluded, a
  `uint32`;
- `unbounded`: FlatBuffers only, with the existing `FbUnbounded` attribution;
- `absent`: no state, with an `AbsentCause`:
  - `ENCODING_UNDEFINED`: a stream payload, a request of zero or of several
    parameters, an inline `T | E` reply; no codec defines the shape's encoding
    yet (driftsys/ridl#336 for streams);
  - `NO_MESSAGE`: proto3 only, a named scalar, an enum or an enum set, inlined
    into their field by ADR-0017 decision 1;
  - `REFUSED_MEMBER`: proto3 only, a member the proto backend refuses;
  - `NO_BOUND`: a `string` or `bytes` with no length bound, a type def with no
    width;
  - `OVERFLOW`: the bound exceeds `u32::MAX` or the nesting passes the sizer's
    depth;
  - `UNRESOLVED`: a name that does not resolve, or a declaration with no root
    table.

The exact list is fixed by the implementation from the `size_state` rustdoc of
`crates/ridl-descriptor/src/size.rs`, which is the one sizer both the descriptor
and the model call. `Payload` gains `PayloadSizes sizes = 3` with a `SizeState`
for proto3 and one for FlatBuffers; field 2 keeps being written, equal to the
FlatBuffers bounded value, for a reader older than this field. `CommandShape`
and `QueryShape` gain request (and, for a query, reply) size states for the
shape whatever its parameter count, so that a multi-parameter request and an
inline `T | E` have a state (absent, `ENCODING_UNDEFINED`) rather than no field.
`SignalShape` and `EventShape` are covered through their `Payload`.

`repr(C)` has no state and no field in this epic (driftsys/ridl#317). When it
lands, `PayloadSizes` gains a third `SizeState`, additively.

driftsys/ridl#665, narrowing a string's byte capacity from a portable `match`
pattern, is optional in this story and lands only after the portable-pattern
implementation of driftsys/ridl#597; a string's capacity stays four bytes per
scalar value until then.

**Alternatives considered.** A `Payload` present for every shape, with `type`
absent for a non-named shape: rejected, because changing a field's presence
semantics is breaking under D-5. Leaving the shapes with no encoding out of the
model until a codec defines them: rejected, because a plugin could not tell an
old toolchain from an undefined shape.

### D-8. Reservation and table budget are tabulated in the model

For each live interaction, the model carries `Reservation`: per encoding, the
saturating sum of the bounded sizes of the member's payloads in bytes (a
`uint64`, as `Member::reservation` returns), or `unsized` naming the first
payload with no bounded state. The payloads are the ones the Rust backend's
descriptors list today: a signal's, an event's or a fixed member's one payload;
a command's request alone, because §6.1 promises an acknowledgment and no reply
payload; a query's request then its reply. For each interface, the model carries
the table budget: per encoding, the saturating sum of the reservations of every
live member, in `MEMBERS` order, or `unsized` naming the first member that is.
These are `ridl_rt::contract::Member::reservation` and `table_budget` over
`Interface::MEMBERS`, computed once, so that the plugin that sizes a call
table's storage and the runtime that debits the budget agree on the same
numbers.

**Alternatives considered.** Leaving the two sums to the plugin. Rejected: the
sums are a contract of ridl-rt, and two plugins must not disagree on them.

### D-9. Binding overheads are a toolchain-held table carried in the section

The toolchain holds a table of the transport bindings it knows, each with a
name, the version of the binding document the numbers come from, an optional
maximum frame header size and an optional envelope size, in bytes. The
deployment section carries the table as `bindings`. A size is absent when the
binding's document does not fix it; a binding the toolchain does not know has no
row, and a plugin that implements such a binding supplies its own numbers. The
table is empty until driftsys/ridl#265 lands the WebSocket binding and its
document states the header layout (frame specification §10 item 2); E17.3 adds
that row and the test that checks header plus envelope plus payload bound
against a frame the binding writes.

The maximum is what a message layout needs: a variable-length header still has a
largest encoding.

**Alternatives considered.** Carrying the overheads at the request level,
outside the deployment section: rejected, because they matter only for a message
layout, which exists only with a deployment. Selecting one binding by a backend
option: rejected as premature; carrying every known row costs a few bytes.

### D-10. The system descriptor file is not built in this epic

`ridlc build --emit system`, the second half of the catalog descriptor design's
D-1 and all of its D-5, stays "Not built". The request's deployment section now
carries the per-deployment facts a backend needs, and no runtime in this
workspace reads a descriptor file (`ridl-loopback` is given its catalog by its
caller). `docs/design/catalog-descriptor.md` records, under "Not built", that
the system facts reach a backend through the codegen request's deployment
section, and that a descriptor file would be a second emitter over the same
system IR when a runtime that reads one exists. E17.5 drops the descriptor and
`ridl describe` from its scope.

**Alternatives considered.** Building it here as the fixture format of the exit
test. Rejected: the exit test reads a request, and a FlatBuffers file nobody
reads is cost without a consumer.

### D-11. The command-line selection

`ridl build` gains `--deployment NAME`. The flag names a deployment of the
workspace's system (RSDL-601: there is one system). With the flag absent, the
request carries the deployment when the system has exactly one, and none when it
has several or the workspace has no system. A `NAME` that is not a declared
deployment exits 2 with a message naming the known deployments (ADR-0010
decision 1: a bad flag value). A deployment that an RSDL-7xx error dropped from
the system IR (ADR-0022 decision 8) cannot be selected: the build already exits
1 on that error.

The flag gets a doc comment and a row in `docs/book/cli-reference.md`;
`ridlc
build` gets the same flag (ADR-0010: the same flag names for the same
things).

**Alternatives considered.** Carrying every deployment in the request and
selecting by backend option: rejected, because a backend option is the backend's
contract, and the selection is the build's. Requiring the flag even for one
deployment: rejected, because one deployment is the normal case and the exit
test should not need a flag.

### D-12. No new `--emit` value in this epic

The request stays an in-memory artifact (`ridlc::codegen_request`), and a test
plugin builds it in process as the parity tests do. A `codegen-request` dump,
useful to a plugin author outside this workspace, is filed as a follow-up issue,
not built here.

## 5. The schema

Field numbers below are binding; comments and message docs are written by E17.1
and E17.2 in the schema's own style. The codegen package imports nothing from
`ridl.ir.v2` (design note D-14), so `Crossing` is restated with the IR's values.

### 5.1 `ridl/codegen/v1/deployment.proto` (E17.1, E17.3)

```proto
syntax = "proto3";
package ridl.codegen.v1;
import "ridl/codegen/v1/model.proto";   // Kind

message Deployment {
  string system = 1;                 // the system's qualified name
  string name = 2;                   // the deployment's bare name
  repeated Region regions = 3;       // catalog name order
  repeated Instance instances = 4;   // the system IR's placement order
  repeated Channel channels = 5;     // §4 D-2 order
  repeated Binding bindings = 6;     // binding name order (E17.3)
}

message Region {
  string catalog = 1;
  bytes hash = 2;                    // 32 bytes, equal to the catalog descriptor's
  repeated RegionInterface interfaces = 3;   // interface number order
}

message RegionInterface {
  string name = 1;
  uint32 number = 2;
  bool inline = 3;
  bool provisional = 4;
  string service = 5;                // the closure service that lists it
}

message InterfaceKey {
  string catalog = 1;
  uint32 number = 2;
  string name = 3;
  bool inline = 4;
}

message Instance {
  string component = 1;              // qualified
  string instance = 2;
  string machine = 3;
  bool external = 4;
  repeated InterfaceKey offers = 5;  // the interfaces it provides
  repeated string maps = 6;          // the catalogs it requires into
}

message Endpoint {
  string component = 1;
  string instance = 2;
  string machine = 3;
}

enum Crossing {
  CROSSING_UNSPECIFIED = 0;
  CROSSING_SAME_MACHINE = 1;
  CROSSING_DIFFERENT_MACHINE = 2;
  CROSSING_OFF_BOARD = 3;
}

enum Encoding {
  ENCODING_UNSPECIFIED = 0;
  ENCODING_PROTO3 = 1;
  ENCODING_FLATBUFFERS = 2;
  ENCODING_REPR_C = 3;               // no link carries it until driftsys/ridl#317
}

enum ValueSource {
  VALUE_SOURCE_UNSPECIFIED = 0;
  VALUE_SOURCE_DERIVED = 1;          // depth: ceil(max / min)
  VALUE_SOURCE_DECLARED = 2;         // an rsdl key, on the placement line or the deployment
  VALUE_SOURCE_DEFAULT = 3;          // slots: 16
  VALUE_SOURCE_UNDERIVABLE = 4;      // depth: half-open range, nothing declared; value absent
}

message Depth {
  optional uint32 value = 1;         // absent iff source is UNDERIVABLE
  ValueSource source = 2;
}

message Consumer {
  string component = 1;
  string instance = 2;
  string machine = 3;
  Crossing crossing = 4;
  Encoding encoding = 5;
  Depth depth = 6;                   // event channels only
  optional uint32 slots = 7;         // call channels only
  ValueSource slots_source = 8;
  optional uint64 budget = 9;        // call channels only; absent = none
  ValueSource budget_source = 10;    // UNSPECIFIED when budget is absent
}

message Channel {
  string catalog = 1;
  uint32 interface_number = 2;
  string interface = 3;
  bool inline = 4;
  uint32 member_ordinal = 5;
  string member = 6;
  Kind kind = 7;                     // model.proto's Kind
  Endpoint producer = 8;
  repeated Consumer consumers = 9;   // (component, instance) order
  Depth depth = 10;                  // event channels: the ring depth, max over consumers
}

message Binding {
  string name = 1;                   // "websocket"
  string version = 2;                // the binding document's version
  optional uint32 frame_header_max_bytes = 3;
  optional uint32 envelope_bytes = 4;
}
```

`plugin.proto`: `optional Deployment deployment = 6;` in `CodegenRequest`, after
`artifact_base`.

### 5.2 `ridl/codegen/v1/model.proto` additions (E17.2)

```proto
message SizeAbsent {
  AbsentCause cause = 1;
  optional string detail = 2;        // the member or the shape, for a reader
}

enum AbsentCause {
  ABSENT_CAUSE_UNSPECIFIED = 0;
  ABSENT_CAUSE_ENCODING_UNDEFINED = 1;
  ABSENT_CAUSE_NO_MESSAGE = 2;
  ABSENT_CAUSE_REFUSED_MEMBER = 3;
  ABSENT_CAUSE_NO_BOUND = 4;
  ABSENT_CAUSE_OVERFLOW = 5;
  ABSENT_CAUSE_UNRESOLVED = 6;
}

message SizeState {
  oneof state {
    uint32 bounded = 10;
    FbUnbounded unbounded = 11;      // FlatBuffers only
    SizeAbsent absent = 12;
  }
}

message PayloadSizes {
  SizeState proto3 = 1;
  SizeState flatbuffers = 2;
}

message ReservationState {
  oneof state {
    uint64 bytes = 10;
    string unsized = 11;             // the payload (role and type) with no bounded state
  }
}

message Reservation {
  ReservationState proto3 = 1;
  ReservationState flatbuffers = 2;
}

// Added fields:
//   Payload:       PayloadSizes sizes = 3;
//   Interaction:   Reservation reservation = 8;
//   CommandShape:  PayloadSizes request_sizes = 4;
//   QueryShape:    PayloadSizes request_sizes = 6;  PayloadSizes reply_sizes = 7;
//   Interface:     Reservation table_budget = 9;
```

### 5.3 `ridl/ir/v2/system.proto` additions (E17.4)

```proto
message Sizing {
  optional uint32 depth = 1;
  optional uint32 slots = 2;
  optional uint64 budget = 3;
}
// Added fields:
//   Deployment: Sizing sizing = 15;
//   Placement:  Sizing sizing = 5;
```

### 5.4 Where the code lives

- `ridl_ir::codegen::lower_deployment(system: &v2::System, name: &str, packages: &[&v2::Package]) -> Option<v1::Deployment>`
  beside `lower`: the emitter of D-2, with the derivation of D-5 and the
  encoding of D-4. It returns `None` when the name is not a deployment of the
  system.
- `ridlc::codegen_request` gains the deployment parameter; `ridlc` selects it
  (D-11) and passes the same value to every package's request.
- The binding table (D-9) is a `const` in `ridl_ir::codegen`, empty until E17.3.
- The size states (D-7, D-8) are computed in `ridl_ir::codegen::lower` through
  the sizer. The sizer lives in `crates/ridl-descriptor/src/size.rs` today and
  `ridl-descriptor` depends on `ridl-ir`, so it moves into `ridl-ir` as
  `ridl_ir::projection::size`, with its own `Encoding`, `SizeState` and
  `UnboundedCause` types, and `ridl-descriptor` maps them onto its generated
  FlatBuffers enums, as the catalog hash already moved (ADR-0022 decision 7,
  note of 2026-10-04). One sizer, two emitters.
- `ceil(max_us / min_us)` needs integer arithmetic on the exact-decimal strings
  of `Timing`, and `ridl-ir` has no decimal parser; the emitter gets one,
  `ridl_ir::codegen::depth::ceil_ratio(max_us: &str, min_us: &str) -> Option<u32>`,
  which scales both operands to a common number of fractional digits and divides
  in `u128`.
- The rsdl checks (D-6) live in `crates/ridl-sem/src/rsdl/attrs.rs` (the
  allow-list and RSDL-709) and `mod.rs` (RSDL-805 and RSDL-806, which read the
  package IR's timing); the lowering into `Sizing` in `lower.rs`.

## 6. The rsdl reference, as E17.4 lands it

- §3.4 `deployment`: a sentence that the declaration and its placement lines
  take the three sizing keys, with a link to §5.
- §5: three rows in the attribute table (key, sites, value, range, default,
  guard), the amended sentence "Lines take backend keys only, except the three
  sizing keys a placement line takes", and the precedence rule.
- §13: an item after the attribute map: "The sizing values — the declared
  `depth`, `slots` and `budget` of the deployment and of each placement, as
  written; their resolution per link, and the derived depth, are the codegen
  request's (docs/design/codegen-plugins.md)".
- §16.1: RSDL-709, RSDL-805 and RSDL-806 rows.

`docs/design/codegen-plugins.md` gains a section on the deployment section (D-1
to D-5, D-9, D-11), written by E17.1 and extended by E17.3 and E17.4.

## 7. Records amended by this pull request

| Record                                    | Amendment                                                                                                                                                                                                                                        |
| ----------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| ADR-0015 decision 21                      | the depth stays derived by the toolchain; it is declarable as an rsdl deployment override that replaces it, with a warning below the bound (D-5, D-6)                                                                                            |
| ADR-0018 decision 12                      | the toolchain evaluates the contract bound; the consumer rule waits for a service period; the override is `depth`; the slot count and the byte budget are `slots` and `budget`; "deploy-time error" becomes a build warning and a plugin refusal |
| ADR-0020 decision 9                       | the request carries the lowered model, the backend options and, for a selected deployment, the deployment section                                                                                                                                |
| ADR-0022                                  | new decision 11: the codegen request's deployment section is an emitter over `System`; the boundary with the parked engine                                                                                                                       |
| `docs/design/catalog-descriptor.md`       | "Not built": the system descriptor stays out; where the system facts reach a backend (D-10)                                                                                                                                                      |
| `docs/ROADMAP.md`                         | the E17.5 row drops the descriptor and `ridl describe`; the Epic 11 note names the layout inputs                                                                                                                                                 |
| `docs/wip/2026-09-13-step1-lanes-plan.md` | lane S's driver prompt line names this design and its plan                                                                                                                                                                                       |

The rsdl reference and `codegen-plugins.md` change with the implementation (§6),
because shipped docs describe the system as built.

## 8. Delegated decisions

Decisions Sebastien took in this session (S-n) and decisions taken on his behalf
(DD-n), for his review.

- **S-1.** The slot count is the call table's entry count (`Table<N>`), and the
  call budget is the byte budget. The handoff's phrase "maximum number of RPC
  calls in flight" described the slot count.
- **S-2.** The keys attach to the `deployment` declaration and to placement
  lines; a placement value takes precedence.
- **S-3.** The keys are `depth`, `slots`, `budget`; a declared `depth` replaces
  the derived value and warns when below the contract bound.
- **DD-1.** The deployment section is a request field beside the model (D-1), in
  a new file of the same package.
- **DD-2.** Payload sizes stay in the model; the section carries no sizes (D-2).
- **DD-3.** The tabulation boundary of D-3: the plugin computes slot size,
  region layout, message size and table storage.
- **DD-4.** The encoding derives from the crossing by ADR-0020 decision 2 (D-4).
- **DD-5.** The contract bound is the derived depth; the consumer rule is not
  evaluated until rsdl carries a service period (D-5).
- **DD-6.** Ranges and defaults: `slots` 1 to 65536 default 16; `budget` default
  none; `depth` 1 to `u32::MAX` (D-6).
- **DD-7.** Codes RSDL-709 (error), RSDL-805 and RSDL-806 (warning lints).
- **DD-8.** The declared values are dedicated IR fields (`Sizing`), not
  attribute-map entries (D-6).
- **DD-9.** Size states gain an explicit absent state with a cause; field 2 of
  `Payload` keeps being written (D-7).
- **DD-10.** Reservation and table budget are tabulated in the model (D-8).
- **DD-11.** Binding overheads are a toolchain-held table, empty until #265
  (D-9).
- **DD-12.** The system descriptor file is not built in this epic (D-10).
- **DD-13.** `--deployment NAME`, the single deployment selected implicitly,
  several deployments and no flag carry none (D-11).
- **DD-14.** No `codegen-request` emit; a follow-up issue (D-12).
- **DD-15.** E17.1 adds a `service` to `cabin.ridl`, because a component offers
  services and a service is a `ridl` declaration (rsdl §3, RSDL-604).
- **DD-16.** The cabin deployment has three instances on two machines (§9), so
  that the exit test covers both encodings.

Decisions taken while the deployment section was built (DD-17 onward).

- **DD-17.** A zero quotient from the depth arithmetic is not derivable. The
  depth range starts at 1, so `ceil_ratio` answers nothing for a zero quotient
  and the channel records an absent value with the underivable source, exactly
  as it does for a missing bound. The alternative would write a depth of 0 into
  the section and leave a consumer to allocate a zero-length ring.
- **DD-18.** A workspace with no system at all, built with `--deployment NAME`,
  is an error that exits 2. D-11 does not state this case. A name that cannot be
  found is not found, and carrying no deployment silently would hide a typo in
  the flag.
- **DD-19.** When the selection fails and the build has already accumulated an
  error, the build reports that error and exits 1, writing nothing; only an
  otherwise clean build reaches the exit-2 unknown-name path. This is what lets
  a deployment dropped by an RSDL-7xx error report that error rather than be
  called unknown, as D-11 requires, while keeping exit 2 for a genuinely unknown
  name.
- **DD-20.** An event channel with no consumer link takes the member's contract
  bound as its ring depth. The design does not state this case. Under D-5 every
  link's depth is that same contract bound, so the maximum over a non-empty set
  always equals it, and reporting an underivable depth instead would tell a
  plugin the channel cannot be sized when the bound is known.
- **DD-21.** A route's interface is matched on the catalog and the identity name
  alone, not on the interface number. The name is unique within a catalog, so
  the number adds no discrimination, and including it means a package set whose
  numbers differ from the lowered system's matches nothing — the channel would
  lose its kind and its sizing with no diagnostic.

Decisions taken while the size states were built (DD-30 onward; the numbering
leaves room for the deployment section's own decisions, which start at DD-17).

- **DD-30.** The new size fields made the generated `Interaction.shape` oneof
  trip clippy's `large_enum_variant`, so the query variant is boxed and
  `Shape::Query` holds a `Box<QueryShape>`. The protobuf and JSON encodings are
  unchanged, so a plugin that reads the request as JSON is unaffected; only a
  Rust crate that links `ridl-ir` and matches on `Shape::Query` is.
- **DD-31.** `QueryShape.reply_sizes` is written for every query, including one
  with no declared return type, where it carries an absent state with cause
  `ENCODING_UNDEFINED`. The ridl surface makes a reply mandatory, so that case
  is unreachable today, but these fields exist so that a plugin can tell an
  undefined shape from a toolchain too old to report one, and an unwritten field
  reintroduces exactly that ambiguity.
- **DD-32.** Three cases D-8 leaves open. A `fixed` member whose payload is not
  a named type — an array, a tuple, a stream — is unsized, because no codec
  defines a size for it; its name states the payload's kind. A member of an
  unknown kind is unsized rather than summed, so that a newer toolchain's member
  kind never yields a silently smaller number. An interface with no live member
  has a table budget of 0 bytes, which is the identity of the sum and what the
  runtime's own `table_budget` returns for an empty member list.
- **DD-33.** A `u64` sum or product overflow inside the proto3 walk is
  attributed `REFUSED_MEMBER` rather than `OVERFLOW`. Attributing it precisely
  would require the whole walk to carry a cause instead of an `Option`, which is
  larger than this work; the coarser attribution is stated in the sizer's own
  documentation, and no caller branches on the cause.

- **DD-34.** Task 13 of the plan, the three catalogue rows as a commit of their
  own, is not a green commit: `crates/ridlc/tests/corpus.rs` asserts that
  `RSDL_PROFILE_CODES` equals the `RSDL-` rows of `RSDL_CATALOG` and that the
  diagnostic showcase provokes exactly the listed codes, so a catalogue row
  cannot land before the check that raises it. RSDL-709's catalogue row, corpus
  row, showcase source and reference §16.1 row land with Task 14; RSDL-805's and
  RSDL-806's, with their two `docs/book/lints.md` rows, land with Task 17. This
  is the fallback the plan's Task 13 names.
- **DD-35.** How a collect-time RSDL-709 blocks only its deployment, which D-6
  requires without saying how. `deployment_decl` records the reporter's
  diagnostic count before reading its attribute blocks and sets a new
  `DeploymentDecl.has_errors` when any diagnostic appended since is an error
  whose code starts with `RSDL-7`, the same predicate `check_system` already
  uses for a closure; `placement::place` seeds `DeploymentCheck.has_errors` from
  it. This keeps the plan's `sizing_key` signature, which returns nothing, and
  covers the declaration's own block and every placement line in one place.
  Rejected: `sizing_key -> bool` with a flag threaded through `ReadAttrs`,
  `member_ref` and `MachineDecl`.
- **DD-36.** A placement line is a new FORM-107 site, so it takes its own
  message: "attribute `<key>` not valid on a placement line — a placement line
  takes backend keys and the sizing keys `depth`, `slots` and `budget` (rsdl
  reference §5)". The message for the other lines is unchanged.
- **DD-37.** Two shapes D-6's "not an integer" leaves open. A bare key, written
  with no `=`, is RSDL-709 and the message reads "a bare `<key>` is not one" in
  place of the written text. A key whose `=` carries a value the parser cannot
  read, such as `slots = 50ms`, draws no RSDL-709: the parser has already raised
  FORM-101, and a second diagnostic calling the key bare would misdescribe the
  source. FORM-101 is a parse error, so `ridlc` blocks every artifact of the
  build, which is stricter than blocking the one deployment.
- **DD-38.** A rejected value leaves its `Sizing` field absent rather than
  clamping it to an endpoint, and a key written twice in one block keeps the
  first value, which is what the existing FORM-108 mechanism already does.
- **DD-39.** The showcase provokes RSDL-709 with `slots = 0` on the `Road`
  deployment of `placement.rsdl`, which already carries placement errors, rather
  than on the one deployment of `attributes.rsdl`, which is the deployment that
  lowers. No lowering snapshot changes.
- **DD-40.** The `budget` range is written in the diagnostic message and in the
  rustdoc as decimal digits, `18446744073709551615`, not as `2^64 − 1`.

- **DD-41.** The ring depth's tie-break. D-5 fixes the ring depth as the maximum
  over the consumer links and `docs/design/codegen-plugins.md` gives it "the
  source of the link that supplies it", which left the source order-dependent
  when a declared link and a derived link carry the same value: the first link
  in emitted order supplied it. The maximum is taken over the rank
  `(value, source is
  declared)`, so on an equal value the declared link
  supplies the source whatever its position. The record's sentence needs "and,
  on an equal value, the declared source".
- **DD-42.** RSDL-806 covers every underivable contract bound, not only the
  explicit half-open range of D-6's table. An event whose `ceil(max / min)`
  exceeds 4294967295 is `UNDERIVABLE` in the emitter, so a link to it with no
  declared `depth` has the same unsized ring and draws the same warning. The
  catalogue summary, `docs/book/lints.md` and the rsdl reference §16.1 state the
  wider condition; D-6's table row is narrower than what is built and this entry
  supersedes it. The message states which of the two cases applies.
- **DD-43.** The grain of both warnings is once per consumer instance and event.
  The IR lowers one link per (consumer instance, producer instance), so a
  redundant provider set would otherwise print identical text twice at one site;
  the declared value and the bound belong to the consumer side and the event.
- **DD-44.** A deployment blocked by its own RSDL-7xx error is skipped by the
  two warnings, because its placement set is not one to read links from: an
  instance placed twice or not at all, a machine declared twice, or a sizing
  value out of range leaves that deployment's lines without a defined link set.
  The pass does run when the closure carries errors that are not RSDL-7xx,
  because suppressing a warning over an unrelated error elsewhere would be worse
  for `ridl check`.
- **DD-45.** A `requires` whose two ends are both external is skipped, mirroring
  the lowering, which emits no link for it. A pair with one external end is
  lowered and is warned about.
- **DD-46.** The sites. RSDL-805 points at the `depth` attribute of the
  placement line that declares the value, matching RSDL-709 for the same key,
  and at the deployment's name when the value is the deployment's. RSDL-806
  points at the consumer's placement line.
- **DD-47.** The declared-depth precedence is restated in the checker rather
  than shared with the emitter. `declared_sizing` is private to `ridl-ir` and
  reads the lowered `v2::Deployment`, while the checker runs before lowering on
  the model's `Sizing`, and `ridl-ir` cannot depend on `ridl-sem`. The rule is
  one expression, so a shared helper would have cost a model-to-IR conversion or
  a generic over two option sites. Both sides carry a cross-reference in their
  rustdoc naming the other and saying the two are edited together.
- **DD-48.** `ridl_ir::codegen::depth` becomes public so that `ridl-sem` can use
  `ceil_ratio`, which is the path the plan already named.
- **DD-49.** A consumer named in a message is the component's declared name,
  with `.instance` appended for a declared instance and nothing for the unit
  instance.
- **DD-50.** A consumer link with no matching placement, which only a hand-built
  system produces, takes the deployment-level values: the placement site is
  absent and the lookup falls through. The emitter applies no range check of its
  own and copies whatever the IR carries, because RSDL-709 is the range gate.
- **DD-51.** An open question deliberately not settled in this stage. A
  half-open event that no consumer link consumes draws neither warning, which
  follows D-5 literally, yet the request carries that channel's depth as absent
  with source `UNDERIVABLE` and nothing tells the author why a plugin cannot
  size it. D-5 and DD-20 cover the no-consumer case only for a derivable bound.
  Referred to a follow-up issue rather than widened here, because what a warning
  is for is a design question.

Decisions taken while the test plugin and the records were built (DD-52 onward,
stage S4). DD-52 to DD-56 are rules of the test plugin in
`crates/ridlc/tests/layout.rs`, not rules of the toolchain or of the request: a
plugin that lays out memory differently is as correct as this one.

- **DD-52.** Each message entry of the fixture carries a `proto3_bound` field
  beside `interface`, `member`, `consumer` and `max_message_bytes`. Section 9
  says the fixture states the payload bound until a binding row exists, and
  without the field every message entry is all null and proves nothing numeric.
  If wrong, the fixture's shape differs from the plan's by one field; removing
  the field leaves the message half of the proof without a number.
- **DD-53.** A call slot's count is the largest `slots` over the consumer links
  of the channel that state one; a link with no `slots` is skipped, and the
  plugin reports an error only when no link states one. The toolchain's ring
  depth of an event channel is also the largest value over the links, but it is
  absent when any link's depth is absent, so the two rules differ for a link
  that states no value. A sum over the links is the other reading. The fixture
  pins the choice: `setLevel` has 16 slots under the maximum and would have 32
  under the sum. If wrong, a call region is half the size another plugin
  computes.
- **DD-54.** A call slot's base is the FlatBuffers reservation of the member
  rounded up to a multiple of 8 bytes as one number. For a query whose request
  and reply are each bounded at 41 bytes, the base is round8(41 + 41) =
  round8(82) = 88; rounding each payload first would give round8(41) +
  round8(41) = 48 + 48 = 96. The cabin fixture cannot pin this choice: its one
  query, `average`, gives round8(46 + 44) = 96 and round8(46) + round8(44) =
  48 + 48 = 96, and its command `setLevel` has one payload. If wrong, the
  regions that hold a call member differ by the padding of one payload.
- **DD-55.** A slot's label is `Interface.member`, because one region holds two
  interfaces, and a message's consumer is `component.instance`. The labels
  affect only how the fixture reads. If wrong, a label is renamed in the fixture
  and the plugin, and no number moves.
- **DD-56.** A query's message bound is the larger of the request's and the
  reply's proto3 bound, and null when either is not bounded, because the message
  that crosses is the larger of the two. If wrong, a query's message size is
  understated or null where another plugin states a number.
- **DD-57.** The end-to-end test of declared sizing values runs over a workspace
  written to a temporary directory, not over a corpus entry and not over
  `examples/cabin`. Six test files enumerate every entry of
  `crates/ridlc/tests/corpus` (`corpus.rs`, `ir_canonical.rs`,
  `codegen_model.rs` and `totality.rs` in `crates/ridlc/tests`, and `parity.rs`
  in `crates/ridlc-gen-rust/tests` and in `crates/ridlc-gen-model/tests`), and a
  new entry would run in each of them for a test that needs none;
  `examples/cabin` is the hand-checked fixture and the `just demo` subject, and
  `rsdl-appendix-a` is held verbatim against the reference. If wrong, the
  declared path has no snapshot in the corpus, and a later change to the corpus
  does not exercise it.
- **DD-58.** `max_message_bytes` is null in the fixture, and the sum of frame
  header, envelope and payload bound is untested, until the WebSocket binding
  row lands (driftsys/ridl#718, which waits on driftsys/ridl#265). The table of
  binding overheads has no row, and a number invented for the test would state
  an overhead no binding document defines. If wrong, the proof lands with the
  one sum it names unexercised, and the fixture gains non-null values when the
  row lands. Note of 2026-10-06 (driftsys/ridl#736), which supersedes the
  statements above that the sum is untested and that the proof lands with the
  sum unexercised; the statements that the fixture's `max_message_bytes` is null
  and that the binding row is missing stay current. The review of the pull
  request asked for the sum to be evaluated. A test now adds a `websocket` row
  with a frame header of 14 bytes and an envelope of 2 bytes to cabin's request
  and checks the sum for `Cabin.warning`, 14 + 2 + 8 = 24. Those two values
  belong to the test, not to a binding document; the fixture's values stay null.
- **DD-59.** The ROADMAP keeps the WebSocket binding row open while the rest of
  Epic 17 moves to the landed record, and the BACKLOG marks P1 done except that
  item. Shipped records describe the system as built, and the row has not
  landed. If wrong, the ROADMAP keeps one row the plan meant to retire.

## 9. The exit test and the cabin system

E17.1 adds to `examples/cabin`:

- in `cabin.ridl`, a service listing the two interfaces, so that a component can
  offer them;
- a `.rsdl` file with one `system`, three components and one `deployment`:
  - a provider component offering the service, instance `main`;
  - a local consumer requiring `Cabin` and `Horn`, placed on the provider's
    machine (two same-machine links, FlatBuffers);
  - a remote consumer requiring `Cabin`, placed on a second machine, neither
    machine `external` (one different-machine link, proto3).

The request for the deployment then carries one region (`veh.cabin`), three
instances, five channels (`temperature`, `warning`, `setLevel`, `average`,
`active`), and per channel the consumer links above. The hand-checked fixture of
E17.5 states, from this request alone:

- the `warning` channel's depth: `@[100ms..1s]` gives
  `ceil(1000000 / 100000)
  = 10`, source `DERIVED`, for both consumers; the
  ring depth is 10;
- each call consumer link's `slots = 16` (`DEFAULT`) and no budget;
- the FlatBuffers bound of each payload (a boxed scalar for `temperature`,
  `active`, `setLevel`'s `level` and `average`'s reply; the `Warning` table for
  `warning`), as `projection::flatbuffers::max_size` computes it, each number
  written in the fixture with its derivation;
- the region's byte layout under the test plugin's own stated rule (slot order
  by ordinal, each slot the payload bound rounded to 8 bytes, events times the
  depth, calls times `slots` times the reservation);
- the remote consumer's socket messages: the proto3 bound of each `Cabin`
  payload, plus the WebSocket binding's header and envelope sizes once E17.3 has
  its row; until then the fixture states the payload bound and the test asserts
  the binding list is empty.

Two runs of `ridl build` write the same request bytes, and each channel's
consumer count matches the routes of the system IR dump.

## 10. Follow-up issues to file

- A `codegen-request` emit (D-12).
- A heads-up on driftsys/ridlc-gen-kotlin when E17.1 and E17.2 change the model
  (lane rule).
- `repr(C)` rows in `PayloadSizes` and a backend key selecting the encoding per
  link, when driftsys/ridl#317 lands (D-4, D-7).
- Per-member and per-link sizing grain, when a deployment records the need
  (D-6).
