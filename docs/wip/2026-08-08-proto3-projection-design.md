# The proto3 Projection — the first wire backend

Roadmap story E9.8. Written 2026-08-08, after E9.7 pinned the name transform and
ADR-0016 fixed the projection contract.

This note designs the first **wire backend** in the sense ADR-0013 decision 1
gives the term: a backend whose target describes bytes in transit rather than a
general-purpose language. It emits the two tiers ADR-0013 decision 2 admits and
nothing above them. It is the first time a ridl name reaches a deployed
contract, which is why E9.7 had to land first.

Throughout, "the note" is
[`2026-08-03-schema-projection-design.md`](2026-08-03-schema-projection-design.md),
and a reference of the form note §4.1 is to it. Language-reference sections are
named in full (ridl §11, typl §7.4, typl Appendix D).

## 1. What is already decided, and by what

Most of this story is specified before it starts. Collecting the bindings in one
place is what keeps the implementation from re-deciding them.

| Question                       | Answer                                                            | Fixed by                     |
| ------------------------------ | ----------------------------------------------------------------- | ---------------------------- |
| What may a wire backend emit?  | two tiers — the typl surface, and the interaction identity table  | ADR-0013 decision 2          |
| Field number of an interaction | the ordinal, identity mapping                                     | note §4.1                    |
| Field number of a struct field | the typl §7.4 ordinal                                             | typl Appendix D              |
| Retired slots                  | `reserved N;`                                                     | typl Appendix D, note §4.1   |
| Integer and float widths       | no `uint8`/`uint16`; negatives take `sint32`/`sint64`             | typl Appendix D, ADR-0013 d4 |
| Quantized floats               | keep their native `float`/`double` form                           | ADR-0013 decision 4          |
| `const`                        | not emitted by a wire backend                                     | ADR-0013 decision 5          |
| Optionality                    | proto3 represents absence structurally, so it does                | ADR-0013 decision 7          |
| Name transform                 | `ridl_ir::name::snake_case`, the pinned algorithm                 | ADR-0016 decisions 1 and 2   |
| Injectivity                    | discharged by RIDL-149 on the package, not by the transform       | ADR-0016 decision 3          |
| Identity table shape           | interface-wide, kind-blind, `UNSPECIFIED = 0`, retired `reserved` | ADR-0013 decision 3          |

Four decisions were taken for this story and are recorded in §3, §4 and §6
below: the emit ceiling for E9.8 specifically, where constraint information
goes, the emitter's internal form, and how "valid proto3" is established.

## 2. Scope — tier 1 and tier 2, and no service block

ADR-0013 decision 2 admits two tiers and says a wire backend emits "no `service`
block, no call face, no value store". Note §4.5, ratified as ADR-0016 decision
10, describes a dispatcher as "one service definition per provided interface" —
which in proto3 is a `service` block. The two readings differ, and the roadmap
settles which one E9.8 takes: **E9.11 is "store and dispatcher generation"**, so
emitting either here would leave that story with nothing to do.

**E9.8 emits tier 1 and tier 2 only.** The tension between ADR-0013 decision 2
and ADR-0016 decision 10 is real but belongs to E9.11, which is where a proto
`service` block would first be written. It is recorded in §8 as an open question
for that story rather than resolved here as a side effect.

One consequence to expect rather than discover: a package declaring only named
scalars and interfaces — which is what the baseline corpus holds today — emits
**no messages at all**, only the identity-table enums. Named scalars inline at
their use sites (§3), and in E9.8 there are no use sites, because interaction
payloads are reached through the store and the dispatcher. Exercising tier 1
therefore needs a fixture that declares structs, enums and unions (§6).

## 3. Tier 1 — the typl surface

### 3.1 The mapping

| typl                     | proto3                                                                                       |
| ------------------------ | -------------------------------------------------------------------------------------------- |
| `TypeDef` (named scalar) | inlined to its backing scalar; the name, unit, range and step become a comment at each use   |
| `StructDef`              | `message`; field numbers are typl §7.4 ordinals; tombstones emit `reserved N;`               |
| `EnumDef`                | `enum`; values keep their explicitly assigned numbers; tombstones emit `reserved N;`         |
| `EnumSetDef`             | the resolved integer scalar; bit names and positions become a comment                        |
| `UnionDef`               | `message` wrapping a `oneof`; arm field numbers are arm ordinals; retired arms emit reserved |
| `ConstDef`               | not emitted (ADR-0013 decision 5)                                                            |
| array                    | `repeated`                                                                                   |
| map                      | `map<K, V>`                                                                                  |
| tuple                    | a generated message with positional fields                                                   |
| `?` field                | the proto3 `optional` keyword                                                                |

### 3.2 Constraints are carried as comments, and only as comments

A named scalar inlines to its backing scalar. Given this declaration:

```typl
type Speed : km/h [0.0..250.0 step 0.5]
```

a field of type `Speed` reaches the schema as a bare `double`. The unit, the
range and the step have no proto3 construct to occupy.

**They are recorded as generated comments at each use site.** The alternative
considered was a published options extension file — extending
`google.protobuf.FieldOptions` so a runtime could read the constraint through
descriptor reflection. It is rejected for v0.1: it requires allocated extension
field numbers and a published schema of our own, it pulls `descriptor.proto`
into every emitted file, and it serves a consumer that does not exist. The IR is
already the machine-readable contract, and E9.10 hashes the IR rather than the
emitted schema, so nothing downstream needs the constraint in structured form.

The forcing case that would reopen this: a consumer that must validate payloads
against the contract without access to the IR.

### 3.3 An enum set does not become a proto enum

A proto3 `enum` field holds one value. A typl enum set is a combination of bits,
so a proto enum cannot represent it, and emitting one would imply a guarantee
proto3 does not make — which is exactly what ADR-0013 decision 2 forbids. An
enum set therefore projects as its resolved integer scalar, with the bit names
and positions in a comment.

### 3.4 Two proto3 rules force naming and numbering choices

**Enum values share the enclosing scope.** proto3 scopes an enum's values as
siblings of the enum itself, so two typl enums in one package that each declare
`OK` would emit a redefinition `protoc` rejects. Every emitted value is
therefore **prefixed with its enum's name**. This is a backend naming strategy
and needs no language surface: the collision exists only on this target, and no
other backend is affected by it.

**The first enum value must be zero.** Where a typl enum declares no zero-valued
member, `<ENUM>_UNSPECIFIED = 0` is synthesized. This is the same rule ADR-0013
decision 3 already states for the identity table, applied to declared enums as
well.

## 4. Tier 2 — the interaction identity table

One top-level `enum <Interface>Ordinal` per interface, **including a service's
inline shape**, which is an interface shape (ridl §14.5) and is treated as one
everywhere else. Members appear at their ordinals; retired ordinals emit
`reserved N;`; `<INTERFACE>_ORDINAL_UNSPECIFIED = 0` leads, because ridl
ordinals are 1-based.

The table is interface-wide and kind-blind, matching ridl §11's single sequence
across all five interaction kinds. It is therefore sparse in the sense that
matters later — a store built from it in E9.11 holds signals only — and proto is
untroubled by that (note §4.1).

**Totality.** proto reserves field numbers 19,000 to 19,999 and ends the range
at 536,870,911. An ordinal in either place fails with a diagnostic rather than
emitting a schema `protoc` rejects. Neither is reachable in practice; the check
exists to make ADR-0016 decision 6's totality property true as stated rather
than true by luck.

## 5. The semantic-layer change ADR-0016 decision 4 requires

Decision 4 excluded struct fields from the transform and from RIDL-149, and
bound this story to close that exclusion: struct fields "stay out until E9.8
extends both the transform and this check to them **in the commit that starts
projecting them**, so that the rule and its application change in the same
commit."

So E9.8 carries a change to the language surface, not only a new backend:

1. Struct field names project through `ridl_ir::name::snake_case` **in the proto
   backend**, which is the commit that starts projecting them.
2. **RIDL-149 extends to the fields of one struct** — two fields whose names
   collide after the transform are an error, the same fail-closed rule already
   applied to interface members and interaction parameters. The check runs in
   `ridl-sem`, per ADR-0016 decision 5, so it binds the package rather than one
   backend.

Both land in the same commit, which is what decision 4 requires.

**What this deliberately does not change: the Rust backend.**
`crates/ridl-backend-rust/src/lib.rs` emits a struct field name verbatim
(`ident(&field.name)`), so a typl `currentSpeed` reaches generated Rust as
`currentSpeed`. Adopting the pinned transform there would rename a field on
every generated Rust struct, which is a breaking change to a shipped generated
API. Decision 4 obligates the commit that _starts projecting_ struct fields to
carry the rule with it; it does not obligate a second backend to change how it
already emits them. Whether `ridl-backend-rust` should adopt the transform is a
real question with its own blast radius, and §8 records it as one rather than
answering it here.

Churn is measured over the corpus, the book, the tests and `docs/` before the
change and confirmed after, the way E9.7 did it. E9.7 found the equivalent
change free; this one is expected to be free for the same reason, but expected
is not measured.

## 6. Verification

**Validity is established by compiling the output.** `protox` — a pure-Rust
protobuf compiler already present as a build-dependency of `ridl-ir` — compiles
every emitted schema inside the test suite. This is the story's acceptance
criterion and it costs no new gate requirement: no `bootstrap` change, no CI
install step, and no new recipe subject to ADR-0009's gate-parity discipline.
`protoc` was considered and rejected on that cost; it answers a slightly
different question (what the reference implementation accepts) at the price of
an external toolchain dependency for every contributor.

**Golden snapshots** pin the emitted text, matching how both language backends
are tested.

**The stability property is driven from the classifier**, as ADR-0016 decision 6
requires: generate a delta, and where `ridl_diff::diff_packages` returns
compatible, assert that no already-assigned number moved. Hard-coding example
deltas would test the examples; driving it from the classifier tests the rule.
`proptest` is already a workspace dependency.

**A fixture package is needed.** The story's acceptance criterion names a
cruise-control package, and no such package exists in the repository. The
baseline corpus holds named scalars and interfaces only, which exercises tier 2
and almost none of tier 1 (§2). The fixture must declare structs, enums, enum
sets, unions, tuples, maps, arrays, optional fields and tombstones, so that
every row of §3.1 has a living example.

## 7. Blast radius

- **New crate** `crates/ridl-backend-proto/`, with the surface
  `ridl-backend-rust` already has:
  `generate(&v2::Package) -> Result<Generated,
  GenerateError>`. It adds its
  own scope to `.git-std.toml`, which is an explicit list rather than
  path-derived.
- **New emit value** `Emit::Proto`, flag `proto`, artifact `<base>.proto`. It is
  a code emit, so `ir_dump_suffix` returns `None`; the wildcard-free discipline
  around that `match` means the compiler names every site that must be updated.
- **`ridl-sem`** gains the extended RIDL-149 check of §5.
- **`ridl-backend-rust`** starts transforming struct field names (§5), which is
  where snapshot churn would appear if any exists.
- **`wasm-check`** must pass, so the new crate builds under
  `--no-default-features`.
- **`ridl.std`** gets no emitted file. `ridl.std.Duration` and
  `ridl.std.Timestamp` map onto `google.protobuf.Duration` and
  `google.protobuf.Timestamp` with the matching `import`. `ridl.std` is
  version-locked to the compiler binary and already excluded from IR dumps for
  that reason.
- **Cross-package references** emit `import "<package-name>.proto";`, which is
  the file naming the TypeScript backend already depends on in package and
  workspace mode.

## 8. Out of scope

- **`service` blocks, store messages and RPC mappings** — E9.11. The conflict
  between ADR-0013 decision 2 ("no `service` block") and ADR-0016 decision 10
  ("one service definition per provided interface") is recorded here and must be
  resolved by that story, as an ADR amendment rather than as a side effect of
  writing the emitter.
- **FlatBuffers** — E9.9, including note §4.3's `id = ordinal − 1` projection
  and §4.4's rule that a union payload must not become a FlatBuffers union.
- **The schema hash** — E9.10, computed over the IR rather than over any emitted
  schema.
- **Machine-readable constraints** — §3.2, with the forcing case that would
  reopen it.
- **Whether `ridl-backend-rust` should transform struct field names** — §5. It
  emits them verbatim today, so a typl `currentSpeed` becomes a Rust field
  `currentSpeed`. This is observable in the committed snapshots, which contain
  `pub sensorId: i64` and `pub isOpen: bool`, and the backend emits no
  `#[allow(non_snake_case)]`, so generated code carrying a multi-word field name
  draws that warning at every consumer. Adopting the pinned transform would fix
  it, at the cost of renaming a field on every generated struct. That is a
  breaking change to a shipped API and deserves its own decision, not a side
  effect of adding a backend. Worth filing as an issue independently of this
  story.
- **The two open collision defects** — driftsys/ridl#236 (the C header
  transforms package-level type names) and driftsys/ridl#237 (union arm names
  collide under `camel_case`). Both are in `ridl-backend-rust`, both predate
  this story, and ADR-0016's consequences already record that RIDL-149 does not
  make "the backend never emits non-compiling output on a name collision" true
  in general.
