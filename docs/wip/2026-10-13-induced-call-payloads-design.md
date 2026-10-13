# Induced call payloads — design

Date: 2026-10-13 Issue: driftsys/ridl#713 Ruling: R-C-14 (release 0.10, lane C),
with both of its open questions settled below.

## The problem

The generated interaction face carries a call only when the interaction declares
exactly one parameter whose type is a named type, and a query only when its
reply is one named declared type. Every other shape is refused by the descriptor
emitter, and because the face is built on the descriptors, the whole interface
is skipped: the Rust backend emits the package's domain types and its codec, and
in place of the interface it writes a private
`const __RIDL_NO_FACE_<Interface>: () = ()` whose doc comment names the reason.

### What is actually skipped, corrected

The issue, and ruling R-C-14's table, attribute two of the four skipped
interfaces in the checked-in output to a call with more than one parameter. That
is not what the sources say. Both are calls with **no** parameters:

```text
crates/ridlc/tests/corpus/veh-cluster/cluster/internal-shape.ridl:58
  query readTicks(): RawTickCount @[..50ms]

crates/ridlc/tests/corpus/services-workspace/contracts/shapes.ridl:40
  query fuelLevel(): Percent [ ... ]
```

The refusal is `params.len() != 1` in `no_single_param`
(`crates/ridl-backend-rust/src/descriptors.rs`), and zero trips it exactly as
two does. The frozen dump records the message against `readTicks`, a
zero-parameter query.

Across the corpus, a call with two or more parameters appears only in
`evals/corpus/ros2` (`isPathValid`, `saveMap`, `getPlan`, `setMap`) and in the
two name-collision fixtures of `ridl-diag-showcase`, which exist to provoke a
diagnostic rather than to be built. Zero-parameter calls are everywhere.

So the work that clears the two named markers is the zero-parameter request, not
the multi-parameter one. This design covers every parameter count under one
rule, which is both of them and the ros2 shapes as well.

## Decisions

### D-1: the carrier is `InducedTuple`

The induced type reuses `InducedTuple` with an `InteractionPath` origin, rather
than a sibling message.

`InducedTuple` already carries an `InteractionPath` origin, documented as "a
parameter, a reply or a fixed payload of an interaction", with segments such as
`["param", "window"]` and `["reply"]`. Its `fields` are `Field`, which carry
names, so a named parameter list is not a misfit.

The cost argument that might have favoured a sibling does not apply. The request
payload is a `Payload`, whose `type` is a `TypeRef` — an index into
`Model.declarations` — while an induced tuple is reached by `TupleRef`, an index
into `Model.tuples`. Neither carrier can be referenced through the existing
field, so a protocol addition is required either way.

Reusing `InducedTuple` means the lowering's collision rule (`TupleCollision`,
codegen model design note D-3), the Rust backend's claim table, and the existing
struct, codec, defaults and derives emitters can be reused for these types. The
wire printers require an adapter to supply the interaction tuples and their
lowering-assigned names to the existing positional emitters.

### D-2: the type is created in lowering, never in a backend

As ruled in R-C-14. ADR-0016 makes a projection a pure function from IR identity
to a target namespace, so a name that exists only in one emitter has no identity
to project from, and the Rust and the wire backends would disagree. Lowering
creates the type, names it in `InducedName`, and every backend reads it — the
path a tuple-typed struct field already takes.

### D-3: the name is `<Interface><Interaction><Role>`

Induced structs are emitted at the package module root, not inside the interface
module, so a name must be unique across the package.

`crates/ridl-ir/src/codegen/lower.rs` gains a naming rule for
`Path::interaction`: the Rust name starts at
`camel_case(interface) + camel_case(interaction)`, the wire name at their joined
form, and `named` becomes true. The interface name is the one
`Interface.identity` carries — the declared spelling, or the owning service's
dotted name for an inline shape. The role word `Params` or `Reply` is the next
segment. A tuple written directly in a parameter instead uses `Param` and then
the parameter name. This distinguishes the parameter named `params` from the
request wrapper.

```text
WheelDiagnosticsReadTicksParams
ClimateControlGetBoundsReply
```

`Path::field`, `Path::step` and `Path::tuple_field` extend that prefix as they
already do, so a tuple nested inside a parameter keeps extending one path and
needs no second rule.

### D-4: one request rule for every parameter count

The face's request is always a named type.

| Parameters                | Today                         | After                                   |
| ------------------------- | ----------------------------- | --------------------------------------- |
| exactly one, a named type | request payload; face carried | unchanged — no induced type             |
| zero                      | refused, interface skipped    | induced struct with no fields           |
| two or more               | refused                       | induced struct, one field per parameter |
| one, not a named type     | refused                       | induced struct with one field           |

The one-named-parameter case stays the direct payload it is today. Changing it
would rename a type in every existing face for no gain.

The induced type has origin `InteractionPath` with segments `["Params"]`.

This is the lowering rule over the IR. The source checker still admits only
named types and streams at a parameter position (`FORM-102` otherwise). The
one-unnamed-type and tuple-parameter cases are therefore totality tests over IR
supplied directly, not newly admitted source forms.

### D-5: the reply mirrors it for the two product forms

A query's reply induces `<Interface><Interaction>Reply`, segments `["Reply"]`,
in two cases:

- a tuple reply, such as `getBounds(): (min: Temperature, max: Temperature)`;
- no declared return type, such as `nothingBack(): ()`, which induces a struct
  with no fields — the reply-side twin of the zero-parameter request.

A fallible reply `T | E`, such as `readDoor(): DoorReport | DoorFault`, is a
sum, not a product. It needs an induced enum, a different construction, and it
touches how `ridl-rt` carries a reply and ADR-0023 decision 6. It is out of
scope here and gets its own issue. `DoorControl` therefore keeps its marker.

### D-6: two shapes keep refusing

Neither can be a struct field, so neither has an induced form:

- a parameter or a reply with no declared type;
- a stream in either position, such as
  `command replayTrace(samples: <FilterState>)`.

Their refusal messages change to name the actual cause instead of the parameter
count.

## What this touches

**Codegen protocol — three additive fields.** `CommandShape` and `QueryShape`
each gain `optional TupleRef request_tuple`; `QueryShape` gains
`optional TupleRef reply_tuple`. `request` and `reply_payload` keep their exact
meaning, so a reader that ignores the new fields sees today's model.
`codegen-protocol` is frozen at `a3a0ac21` and the freeze check is a removal
check, so the addition passes.

**FlatBuffers projection — a root table per induced payload.** The codec emits a
table only for a tuple reachable from an `FbRoot` (`Codec::reachable_tuples`),
and an argument or reply struct is a wire root by definition.
`InducedTuple.flatbuffers_table` already exists; the roots list is what grows.
This is the one piece of new projection work.

The `.fbs` and `.proto` printers also emit these induced product types as
positional tables and messages. Their contract entry points read the tuples from
`request.model`; the direct `generate_with` APIs first run lowering. Existing
declaration emission still reads raw IR. A shared model adapter adds resolution
entries for interaction tuples so that fields can reference nested tuples by the
assigned name, without a backend naming them again. These product types are
payload shapes, not RPC envelopes or faces.

An induced root without a finite FlatBuffers bound has no `Payload` codec. The
descriptor emitter refuses its interface before accepting that root's name, and
the pipeline leaves the existing interface refusal marker.

**Sizes and reservations become real.** The Rust descriptor writes a reservation
as a `<Type as Sizes<FlatBuffers>>::MAX_SIZE` path, never a literal
(`max_size_path`), so the Rust side follows the induced struct once that struct
has a codec. In the IR, `request_sizes` and `reply_sizes` stop being
`ABSENT_CAUSE_ENCODING_UNDEFINED` for these shapes, and
`Interaction.reservation` stops being unsized. Three `model.proto` doc comments
state the old behaviour in prose and are part of the change, including
`ReservationState.unsized`, which enumerates the parameter-list form `(a, b)`.

**Prose carried from the source.** A parameter carries a doc comment in the IR
and a tuple field does not, so the lowering copies a parameter's doc, its links,
its `@see` targets and its `@since` versions onto the matching field of the
induced argument struct. A generated item renders the doc its source item has
(ADR-0028).

**The catalog hash does not move.** It is taken over reduced IR
packages(`crates/ridl-ir/src/catalog_hash.rs`); an induced type exists only in
the codegen model, downstream of it. A parameter list that already hashed as a
parameter list still does. No catalog identity changes, and `ridl diff` reports
nothing.

**Freeze.** `generated-crate` has no baseline row in `freeze/surfaces.toml`, so
it is checked only for equality against a regeneration: the two
`__RIDL_NO_FACE_` lines disappearing is repaired with
`cargo xtask freeze update`. `wire` and `codegen-protocol` have baselines, and
both changes here are additions.

**Records to amend.**

| Record                                             | Change                                                             |
| -------------------------------------------------- | ------------------------------------------------------------------ |
| ADR-0016                                           | the naming expression for an interaction-origin induced type (D-3) |
| ADR-0023                                           | the one request-shape rule and the narrowed refusal set            |
| `docs/design/interaction-face.md`                  | the same, as the design reference                                  |
| `docs/book/generated-code.md`                      | what a consumer now sees for a call                                |
| `crates/ridl-ir/proto/ridl/codegen/v1/model.proto` | the three fields and the three doc comments                        |

## Tests

The corpus already carries every case; no new fixture is needed for the main
paths.

| Case                            | Fixture                                    |
| ------------------------------- | ------------------------------------------ |
| zero parameters                 | `readTicks`, `fuelLevel`                   |
| two or more parameters          | ros2 `isPathValid` (seven), `setMap` (two) |
| one parameter, not a named type | `takeMap(table: [Level : Sample; 0..8])`   |
| tuple reply                     | `getBounds`, `readSpan`                    |
| no declared return type         | `nothingBack`                              |
| stream, still refused           | `replayTrace`, `rebalance`                 |
| name collision, still refused   | `setIt`, `setBoth`                         |
| fallible reply, still refused   | `readDoor`                                 |

The checked-in Rust output loses the `__RIDL_NO_FACE_Telemetry` marker and gains
a `Client`, `Provider` and `dispatch` for that interface. Three markers remain:

| Interface          | Refusal                                                    | Tracked by                    |
| ------------------ | ---------------------------------------------------------- | ----------------------------- |
| `VehicleStatus`    | a contract clause outside the narrow translator            | driftsys/ridl#704             |
| `DoorControl`      | a fallible `T \| E` reply                                  | the induced-enum issue below  |
| `WheelDiagnostics` | `fixed tickBounds`'s payload is an array, not a named type | the fixed-payload issue below |

**`WheelDiagnostics` was expected to lose its marker and does not.** Its
`readTicks` call was the refusal the emitter reported, and this change fixes it;
the interface then fails on a second refusal the first one hid, from

```ridl
fixed tickBounds : [(low : RawTickCount, high : RawTickCount); 2]
```

which `descriptors::fixed_payload` refuses because a `fixed`'s payload must be
one named type. The emitter reports the first refusal it reaches, so a marker
names one reason and an interface may have several. Ruling R-C-14's table read
the reported reason as the only one, and this is the correction: the change
un-skips one interface of the four, not two.

## Out of scope

- The induced enum for a fallible `T | E` reply — its own issue.
- An induced payload for a `fixed` whose payload is an array — its own issue,
  and the second half of un-skipping `WheelDiagnostics`. A `fixed` is
  provisioned and never in flight, so it is the weakest of the three.
- The contract-clause translator — driftsys/ridl#704.
