# Schema Projection — from ridl identity to proto and FlatBuffers

| Field     | Value                                                                                                                                  |
| --------- | -------------------------------------------------------------------------------------------------------------------------------------- |
| Status    | design, for review — nothing ratified                                                                                                  |
| Date      | 2026-08-03                                                                                                                             |
| Origin    | generating a signal store and an event/command/query dispatcher from the IR, for targets whose schemas carry their own field numbering |
| Scope     | the identity chain, the projection contract, one projection rule per target, and the gaps each exposes                                 |
| Companion | `2026-08-03-rpc-response-bound-design.md`, whose §4.4 establishes the interface as the generation unit — this note projects that unit  |

A bare section reference — §11, §14.5, Appendix B — is to the **ridl Language
Reference**. References to this document are marked _above_ or _below_; the
companion note is named in full.

**Alignment with ADR-0012.** The boundary model gives an interaction a `family`
and an operation `shape` alongside its kind, with the spelling↔(kind, family,
shape) mapping bijective (d6). Nothing here changes: **numbering keys on kind**,
so a store holds every interaction of kind `signal` whatever its family, and
`family` selects backend behaviour rather than field numbers — which is what d7
means by "codegen selects on `family`". The IR change lands clear of this note
too: `Decl` earmarks fields 8 and 9 as open beside `ordinal = 7`, which is
exactly where E3.3's two closed enums go, leaving the per-kind messages
untouched.

ADR-0012 open 4 — interaction citation paths, "the projection from a declaration
to the stable identifier that specifications, journeys, tests, and telemetry
cite" — is the string-valued sibling of §2 below. Both project the same identity
chain; only the codomain differs.

## 1. The question

The companion note settles that a **provided interface** is the generation unit:
its signals become one store, its interactions become one dispatcher. Emitting
either against proto or FlatBuffers means mapping ridl's identities onto a
target's field numbering.

That raises two questions this note answers. What identity does ridl actually
own? And what must a projection from it guarantee, so that a contract change
ridl calls compatible does not silently renumber a deployed schema?

## 2. The identity chain

| Identity            | Scope                           | Owner      | Status                       |
| ------------------- | ------------------------------- | ---------- | ---------------------------- |
| interaction ordinal | local to an interface           | ridl       | specified — §11              |
| interface id        | local to a service              | ridl       | proposed here — §2.1 below   |
| service number      | global to the addressing domain | deployment | **unspecified** — §2.2 below |

The chain has one governing property: **everything derives, nothing is
allocated.** Ordinals derive from declaration order (§11), transport IDs derive
from ordinals (rsdl §8), wire widths derive from ranges (typl §4.2). No stage
holds allocation state, which is what makes the whole pipeline reproducible from
source. The exception is the last row, and §2.2 below is about why.

### 2.1 Interface ids, assigned per service

§11 numbers interactions within an interface and stops. An interface itself has
no number, which is invisible to a nominal-identity target such as proto and
fatal to a tag-based one.

The proposal is to number interfaces within their service, by exactly the model
§11 already uses one level down:

- **1-based**, matching §11's interaction ordinals, and matching proto's field
  numbers, which start at 1.
- **By declaration order**, append-only. Adding an interface appends; reordering
  shifts ids and is breaking; removing one requires a tombstone to hold its
  slot.
- **An inline shape is slot 1.** A service with an inline shape has exactly one
  interface, so making it the first slot turns the inline form into a degenerate
  case of the general one rather than a separate construct.

The scope argument is what admits this into ridl at all. An interface's slot
within its own service is exactly as local as a member's ordinal within its
interface — it does not depend on anything outside the declaration. That is the
same test that keeps interaction ordinals in ridl, and the same test that pushes
the service number out (§2.2 below).

One consequence follows: `reserved` is needed at **service** level, not only
inside an interface body, so a retired interface can hold its slot.

**A correction, recorded because an earlier draft of this note got it wrong.**
Making the inline shape slot 1 does _not_ make "extract the inline shape into a
named interface" a compatible refactor. Numbering survives the extraction, but
identity does not: ADR-0008 decision 4 derives a fallible return's transport
identity from the enclosing **interface name** plus the ordinal plus both arms —
`fallible_transport_identity` renders it `I#1:CalReport|CalError` — and an
inline shape has no interface name, so it uses the service's dotted name
instead. Extraction therefore rewrites the identity of every fallible query in
the shape, and `ridl-diff` is right to classify a switch between the two forms
as breaking. Making it compatible would mean changing how that identity is
derived, which is a wire-identity decision of its own and not a side effect of
this one.

### 2.2 The service number has no derivation, anywhere

rsdl §8 is emphatic that no source layer names a transport identity — "no
SOME/IP IDs, IP addresses, eventgroup IDs, frame layouts, or serialization
formats in any layer" — and that "transport IDs derive from ordinals (ridl §11):
a service's SOME/IP method/event IDs come from interaction ordinals". That is
airtight for method and event IDs, because §11 supplies the ordinals.

It says nothing about where a service's own number comes from, and neither does
ridl §17 nor rsdl §13. Both derivations available to the family are closed off:

- **Hashing the name** is the obvious one, and Appendix E records that it was
  studied and rejected — "renames silently break wire, IDs unreadable from
  source" — in favour of positional ordinals plus `reserved`.
- **Counting declaration order** does not exist to be counted. The service
  catalog is a flat global namespace spanning packages (§14.5, RIDL-140), so
  there is no sequence, and adding a service anywhere would renumber others.

What remains is allocation-and-record: a registry, pinned in a lockfile-shaped
artifact. That is a different kind of mechanism from everything else in the
family, which is why it deserves an explicit decision rather than being
discovered by the first tag-based binding.

Scope note: this only binds tag-based transports. proto and gRPC never need it,
because identity there is nominal.

**Recommendation:** record it as an open question in rsdl §13, where it belongs
— the number is a deployment fact — and leave it out of ridl entirely.

## 3. The projection contract

A projection maps ridl identity onto one target's numbering. Every projection
must satisfy four properties.

1. **Deterministic.** The same IR yields the same numbers on every run, in every
   backend. No allocation state, no counters persisted between builds.
2. **Total.** Defined for every interaction, or the backend fails with a
   diagnostic rather than emitting a number the target rejects. A target's valid
   range is not the whole integer range (§4.2 below).
3. **Stable under compatible change.** If `ridl-diff` returns compatible, no
   number already assigned may move. This is the load-bearing property.
4. **Injective in scope.** No two interactions collide within one message or
   table.

Property 3 is the one that needs a test rather than an argument, and it can be
driven rather than hand-written: `ridl-diff` already classifies which deltas are
compatible, so a property test can generate a compatible delta, re-run the
projection, and assert that no previously assigned number changed. Hard-coding
example deltas would test the examples; driving it from the classifier tests the
rule.

**Projections are part of the stability guarantee, not backend implementation
detail.** Once anything is deployed, changing a projection renumbers every
existing field of every existing schema. That puts these rules in the same
bucket as the IR schema itself — E4.5's IR stability policy — rather than in
"whatever the backend does today". Recording that now is cheap; discovering it
after a fleet has a schema stamped on it is not.

## 4. The generated shapes — store and dispatcher

The companion note's §4.4 makes the provided interface the generation unit and
the service the addressing unit: one interface yields one store and one
dispatcher. They project differently, and §4.5 below is where they part company.

**The store nests**, because its members are read together:

```text
Store        (the service)      — a table per provided interface
  └─ Interface state            — one coherent block, one generation counter
       └─ signal fields         — numbered by projection (§4.1, §4.2 below)
```

The generation counter belongs on the **inner** table, because coherence is a
property of a provided interface, not of a service (companion note §4.1). The
outer level is a container that supplies addressing; it is not a coherence unit
and must not carry a counter, or two interfaces with independent update rates
would be forced to share one.

The counter itself is emitted, never declared. It is platform vocabulary in the
same sense as the §3.1 envelope: generated into every backend, not expressible
in source, so no contract can redefine it.

### 4.1 proto — field number is the ordinal

proto field numbers run 1 to 536,870,911, are not required to be contiguous, and
are retired with `reserved`. Every one of those properties matches ridl, so the
projection is the identity function:

| ridl                        | proto                       |
| --------------------------- | --------------------------- |
| interaction ordinal _n_     | field number _n_            |
| `reserved` tombstone at _n_ | `reserved n;`               |
| gap where a non-signal sits | absent field number — legal |

The store message is therefore sparse, because §11 runs one ordinal sequence
across all five kinds while a store holds only signals. proto is untroubled by
that.

One property worth recording rather than engineering: proto field numbers 1 to
15 encode their tag in a single byte. Since the field number is the ordinal and
ordinals follow declaration order, an interface's first fifteen interactions get
the cheap tags automatically, and those are usually the core ones — later
additions being the accreted extras. The mapping puts the saving in the right
place with no rule needed.

### 4.2 proto — the one totality case

Field numbers 19,000 to 19,999 are reserved by protobuf itself, and the range
ends at 536,870,911. An ordinal landing in either place would produce a schema
`protoc` rejects.

Neither is reachable in practice: it would take one interface accumulating
nineteen thousand interactions and tombstones, when an interface with a hundred
members is already a design problem. This is recorded to make property 2 of §3
above true as stated — the codomain is not the whole integer range — and not
because it needs design attention. The check costs nothing, since both backends
already return a generation error rather than emit something wrong.

### 4.3 FlatBuffers — field id is the ordinal, minus one

FlatBuffers is stricter. The `id:` attribute, if used at all, "must be applied
to all fields of a table, with numbers forming a contiguous range from 0", and a
field may never be deleted — it is marked `(deprecated)`, which suppresses
accessor generation while the slot stays occupied.

Contiguity means ridl's sparse ordinals cannot be used directly. The resolution
is to keep the identity mapping and **fill the gaps**: every interaction gets
its slot, and the slots of non-signal interactions carry a placeholder.

```text
table VehicleStatusState {
  current_speed:       Speed;                  // id 0  — ordinal 1, signal
  engine_temp:         Temperature;            // id 1  — ordinal 2, signal
  slot_door_opened:    ubyte (deprecated);     // id 2  — ordinal 3, event
  warnings:            WarningFlags;           // id 3  — ordinal 4, signal
  slot_set_gear:       ubyte (deprecated);     // id 4  — ordinal 5, command
  retired_legacy_temp: ubyte (deprecated);     // id 5  — ordinal 6, tombstone
}
```

The projection is `id = ordinal − 1`, and the rules are:

- **Explicit `id:` on every field, always.** Without it FlatBuffers assigns ids
  by declaration order, which makes the generator's text ordering
  wire-significant. With it, text order is free and the numbering is auditable.
- **A non-signal interaction's slot carries a placeholder** named for its owner
  and marked `(deprecated)`. The declared filler type is inert, because a
  deprecated field generates no accessor.
- **A ridl `reserved` tombstone becomes a `(deprecated)` field** holding its
  slot. The two evolution models coincide: `ridl-diff` already guarantees that
  "a tombstone occupies its ordinal exactly so the slot is never reused", which
  is precisely what FlatBuffers requires to keep its range contiguous under
  removal.
- **Names say which interaction owns each slot**, rather than an opaque counter.
  A reader can set the schema beside the interface and check it by eye. In
  particular the placeholder prefix must not be `reserved_`, which in ridl means
  a retired interaction and would invert the meaning.

Two properties make this safe. A slot **never changes hands**: converting an
event to a signal at the same ordinal is `KindChanged`, already breaking. And
the alternative — a dense rank over signals only — was rejected because it turns
the projection into a computed mapping that has to be recorded and kept from
drifting, where this one can be verified by inspection.

### 4.4 FlatBuffers — union payloads must not become unions

The FlatBuffers documentation notes that "unions implicitly add two fields,
requiring careful ID management". A union-typed field consumes two id slots, so
a signal whose payload is a ridl `union` would eat its neighbour's slot and
break `id = ordinal − 1` everywhere after it.

**The projection therefore maps a union payload to a FlatBuffers table carrying
a discriminant and the arms, never to a FlatBuffers union.** One slot, identity
mapping preserved. This is a rule of the projection, not a backend preference,
and it must be stated as one — a backend that reached for the native union would
silently corrupt the numbering of every later field.

### 4.5 The dispatcher does not nest

Nesting exists for coherence — the store's members are read together and must be
observed as one sample. Calls are not read together; each is independent, and no
two of them are ever a joint observation. So the dispatcher is a **routing
table**, not a nested message: one service definition per provided interface,
its members routed by the interface's single ordinal sequence (§11, one sequence
across all kinds).

For proto the mapping is already normative. Appendix B fixes it row by row: a
`command` is a "unary RPC → `Empty` (= ack)", a `query` is a "unary/streaming
RPC", an `event` is a "server-streaming RPC", and a `fixed` is a "unary getter
RPC (cacheable)". This note adds nothing there.

What Appendix B has no column for is **FlatBuffers**, and that is the actual gap
this section fills. Its `rpc_service` declaration carries the same routing role;
§4.6 and §4.7 below supply the two rules Appendix B's proto column gets from
gRPC's own conventions.

### 4.6 Request messages are numbered positionally

A parameter has no identity in ridl. `Param` carries a name and a type and no
ordinal, and `ParamsChanged` classifies as breaking "any direction — a parameter
added, removed, renamed, retyped, or a stream added or removed on one". **There
is no compatible evolution of a parameter list.**

That removes the numbering problem rather than solving it. Since no parameter
list survives a change, nothing has to stay stable across one, so:

> A request message numbers its fields by **parameter position** — 1-based for
> proto, 0-based for FlatBuffers.

Two consequences. Positions are dense by construction, so **FlatBuffers'
contiguity constraint never bites here** — the sparse-ordinal problem of §4.3
above is specific to the store, whose numbering spans all five kinds while its
membership is one. And no tombstone mechanism is needed, because a retired
parameter is not a thing that can exist.

One property worth stating plainly, because it inverts an expectation: this
makes a generated schema **stricter than its own target**. proto's canonical
compatible change is adding an optional field, and ridl forbids the
corresponding edit outright. That is the contract being the authority rather
than the wire format, and it is deliberate — but a reader who knows proto will
expect the looser rule.

### 4.7 Reply carriers differ by whether the transport has an error channel

The inline `T | E` needs no new identity: ADR-0008 decision 4 already derives
one "from the enclosing interface name + the interaction ordinal + both arm
references — stable under compatible evolution", and the IR records it on
`FallibleType`. What differs per target is the **carrier**.

| Target                          | Reply carries                                                                                                                         |
| ------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------- |
| proto / gRPC                    | the success arm only — the error arm goes to the native error channel, which Appendix B fixes as "`google.rpc.Status` + typed detail" |
| FlatBuffers over a byte channel | one table carrying a **discriminant and both arms**, because no native error channel exists                                           |
| any target, `command`           | nothing — the reply is the empty acknowledgment of §6.1, per Appendix B's "unary RPC → `Empty`"                                       |

The second row is the one place §4.4 above's union rule actually bites, and the
reason it had to be normative rather than advisory. A reply table is exactly
where a native FlatBuffers union is most tempting — it is a tagged choice of two
types, which is what the construct is for — and taking it would consume two id
slots for one logical field and shift every field after it.

## 5. Names become wire-significant, on one target

Appendix B records that proto RPC identity is **nominal** — "RPC name (identity
is nominal)" — where SOME/IP derives method and event IDs from ordinals. The
ordinal never reaches a proto wire; the method name does.

That changes the status of the name transform. Turning `currentSpeed` into
`CurrentSpeed` or `current_speed` is cosmetic for the Rust and C headers, where
the generated identifier is a local convenience. For a proto service it is part
of the contract, and changing the transform later is a wire break.

The repository currently has **two different `snake_case` implementations with
different algorithms** — one in `crates/ridl-backend-rust/src/interact.rs` and
one in `crates/ridl-backend-rust/src/c_header.rs`. Today that is a cosmetic
inconsistency between two generated artifacts. For a nominal-identity target it
would be a correctness defect.

**One pinned, tested transform is a prerequisite for any nominal-identity
backend.** It needs a specified algorithm rather than an incidental one,
including the case the two implementations are most likely to disagree on —
consecutive capitals, where `getVIN` may become `get_v_i_n` or `get_vin` and
only one of those can be right. It also needs to be injective over the names
ridl admits, so that two distinct interactions cannot collide after
transformation.

## 6. The schema hash is over the IR

A binding that verifies compatibility at attach time needs a hash. It can be
computed over the IR subtree or over the generated schema text, and the choice
matters more than it appears.

**Over the IR.** One hash per contract, agreed by every backend, with the
projections staying derived outputs. A proto consumer and a FlatBuffers consumer
of the same contract compute the same value.

Over generated schema text, each target hashes differently, formatting changes
perturb the hash, and the projection rules become inputs to identity rather than
consequences of it — which contradicts §3 above, where they are a stability
guarantee layered on top.

This also binds the hash to E4.5's IR stability policy, which is where a value
stamped on deployed artifacts belongs.

### 6.1 A hash answers "same contract", not "compatible contract"

Worth stating, because the two are easy to conflate and a binding that gates
attach on hash equality is making the stronger demand without saying so.

`ridl-diff` calls several changes **compatible** that still alter the IR:
appending an interaction into a never-occupied slot, retiring one to a tombstone
("compatible always — the sanctioned retirement"), and, under §2.1 above,
appending an interface to a service. Every one of those changes the IR, and
therefore the hash. So a peer running the newer contract and a peer running the
older one are compatible by the diff and **unequal by the hash**.

Two coherent positions, and a binding must pick one deliberately:

- **The hash is an identity check.** Exact match required at attach; compatible
  evolution still means redeploying both sides. Simple, cheap, and honest — and
  the right answer for a closed world that deploys in lockstep.
- **The hash is a compatibility-class check**, computed over only the fields
  whose change is breaking. Peers then interoperate across compatible versions.
  This is seductive and fragile: it re-implements the classifier inside a hash
  function, and the two drift the moment a diff rule changes.

**Recommendation: the first.** A hash answers "is this the same contract"; if
the question is "are these two contracts compatible", the answer needs the IR
and the diff, not a digest. Anything that gates on a hash is choosing lockstep,
and should say so rather than discover it.

## 7. Decisions, and the one thing they leave open

### 7.1 Decided — the service number is recorded as an rsdl question

§2.2 above establishes that no derivation is available. The number is a
deployment fact, so it belongs in rsdl §13's open questions rather than in ridl,
and the mechanism — a registry pinned in a lockfile-shaped artifact — is
deferred to E6 with the rest of deployment. Recording it now matters because the
first tag-based binding will need it, and nothing currently says so anywhere.

### 7.2 Decided — one pinned transform, specified with the projections

The transform is `crates/ridl-backend-rust/src/interact.rs`'s, and the divergent
implementation in `c_header.rs` is deleted in favour of it. Tracing the former
on the case they most obviously disagree on: `getVIN` yields `get_vin`, because
an upper-case character emits a separator only when the previous character was
lower-case or a digit, so a run of capitals stays one word. That is the wanted
behaviour, and it decides the question by inspection rather than by preference.

It is specified with the projections in the IR stability policy (E4.5), not in
the attribute registry, because it is a projection: a pure function from IR
identity to a target's namespace, carrying the same stability obligation as the
numbering rules in §3 above. It must also be **injective** over the names ridl
admits, so two interactions cannot collide after transformation.

### 7.3 Decided — a `fixed` interaction gets a real field, not a placeholder

A `fixed` occupies an ordinal like everything else, and the question was whether
a store table should hold it or fill its slot.

It gets a real field. A `fixed` is a value a consumer reads; §8 already says
bindings "expose it as a plain accessor, populated at binding initialization",
and a field in the store table _is_ a plain accessor. It is immutable for the
lifetime of the instance, so it costs the coherence machinery nothing — its
value cannot vary between two reads of the block. Filling its slot with a
placeholder would mean generating a second accessor mechanism for no gain.

### 7.4 Decided — the dispatcher is a routing table, not a nested message

§4.5 to §4.7 above settle it, and it turned out smaller than expected because
two things were already fixed. Appendix B already specifies the proto mapping
for every kind, so only the FlatBuffers column was missing. And parameters carry
no identity, with every parameter change breaking, so request numbering is
positional and needs no stability rule at all.

What remains genuinely open is narrower, and is a transport-binding question
rather than a schema-projection one: whether a byte-channel binding multiplexes
all of an interface's calls over one envelope keyed by ordinal, or gives each
interaction its own channel. Neither choice changes any rule above.
