# The lowered codegen model — content, home, encoding, and the drift test

Status: pre-ADR design note, written 2026-09-22 as stage P2a of the
[lane P driver](2026-09-22-lane-p-driver.md) (§4, "P2 — the lowered codegen
model"). It proposes the disposition of the driver's open item O-P2 and the
model the driver's P2a bullets ask for. **The disposition of O-P2 belongs to
Sebastien**; this note is a recommendation he can overturn on the pull request,
written so that stage P2b can write the proto and the lowering from it as it
stands. Nothing here is implemented, and no ADR is amended by this note.

The note inherits the [IR stability note](2026-09-22-ir-stability-design.md) in
full: its D-1 makes canonical protobuf JSON the canonical encoding of
`ridl.codegen.v1`, its D-3 asks this note to restate the nesting bound for that
schema from its own shape (§6), and its D-5 and D-6 fix the compatibility and
versioning rules the model lives under. It is bound by the driver's decisions
D-P2 (the request carries the model, never the raw IR — a raw-IR fact a backend
still needs is a gap in the model), D-P3 (the plugin never touches the
filesystem) and D-P4 (the floor: what an IPC binding needs), and by
[ADR-0020](../decisions/ADR-0020-third-encoding-runtime-layering-and-plugin-system.md)
decisions 8 to 12.

Every fact about the code below was read from the tree on 2026-09-22, at commit
164e3be, and cites the file it comes from.

## 1. What the note decides

Five things, in the order the driver lists them:

1. **The model's content**, message by message (§3), with the source of each
   fact in the tree and with D-P4's floor checked item by item (§3.6).
2. **Its home** — open item O-P2 (§5). The recommendation is `crates/ridl-ir`,
   as the module `ridl_ir::codegen`, for the ground stage K2 stated when it
   placed the projection facts there.
3. **Its encoding** (§6): canonical protobuf JSON in the proto package
   `ridl.codegen.v1`, with the nesting bound restated from the schema below.
4. **What the model does not carry** (§7): nothing a printer can compute from
   the model alone, with the list of what that excludes.
5. **How drift is caught** (§8): one test per in-tree backend, in two forms — a
   fact-level form stage P2b lands while every backend still reads the raw IR,
   and the byte-level form stage P4 makes total for the Rust backend and then
   deletes for it.

The decisions are D-1 to D-14 (§4), each with its reason and the alternative it
rejects. §9 lists every place where a record and the code disagree.

## 2. What the backends derive from the raw IR today

The model's content is not invented: it is the set of facts the four backends
re-derive from `ridl.ir.v2` today, each in its own copy, plus the two facts they
already share (`ridl_ir::name`, `ridl_ir::projection::flatbuffers`). The table
is the evidence for §3. "R" is `ridl-backend-rust`, "P" `ridl-backend-proto`,
"F" `ridl-backend-flatbuffers`, "T" `ridl-backend-ts`.

| Fact re-derived from the IR                                                                             | Where                                                                                                                                                                                   | By      |
| ------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------- |
| The scalar class of a backing (float, integer, boolean, string, bytes; unit → float; absent → float)    | `backing_scalar` in `ridl-backend-rust/src/lib.rs` and, copied, in `ridl-backend-ts/src/lib.rs`; the `match` on `td.width` and `td.backing` in `proto_scalar` and `fbs_scalar`          | R P F T |
| Whether a constraint is vacuous (no bound, no pattern; `step` excluded)                                 | `ridl_ir::v2::constraint_is_vacuous`, read by R's `emit_type_def` and the codec's `NamedScalar::ctor`                                                                                   | R       |
| The regex body of a pattern, delimiters stripped                                                        | `strip_regex_delimiters`, one copy in R and one in T                                                                                                                                    | R T     |
| A constant's type: same-package scalar class, primitive keyword, or skipped when foreign                | `emit_const` in R; `emit_const` in T; both through `same_package_scalar_backing` and `primitive_keyword`                                                                                | R T     |
| The pinned name transforms                                                                              | `ridl_ir::name::{snake_case, camel_case}`, shared; `screaming_snake_case` composed in P and F; a private `snake_case` in `ridl-backend-rust/src/face.rs` (driftsys/ridl#450)            | R P F   |
| The CamelCase of a dotted name (`corpus.baseline.hvac` → `CorpusBaselineHvac`)                          | `type_name` in P and, copied, in F; not one of ADR-0016's pinned transforms                                                                                                             | P F     |
| The name of an induced tuple type                                                                       | R: `camel_case(parent) + camel_case(field)`, nested by `camel_case(field)`; P and F: `owner + type_name(field)`, nested by `Field<N>`; both add `Element`, `Value` (F and R also `Key`) | R P F   |
| Tombstones held in their ordinal slots                                                                  | R skips them; P emits `reserved N;` where the slot sits; F emits a `deprecated` placeholder at the slot's id; `projection::struct_table` gives the slot                                 | R P F   |
| Resolution of a type reference into its declaration, local or foreign                                   | R: `Ctx::lookup` (same package) and the codec's `resolve` (over `others`); P and F: `resolve_reference`/`named_field_type` over `others`; T: same package only                          | R P F T |
| Transitive init derivability (typl §5.8, the leaf-recursion rule over the one-level IR flag)            | `ridl-backend-rust/src/defaults.rs`; mirrored in T's `init_function` family                                                                                                             | R T     |
| The transitive closure's shape (reaches a float, an allocation, a collection, a foreign name, a cycle)  | `ridl-backend-rust/src/derives.rs`                                                                                                                                                      | R       |
| An enum's zero member, and its init member (zero, else lowest)                                          | R `enum_default`, T `enum_init`; P synthesizes `UNSPECIFIED` and leads with the zero; F's `= null` through `projection::enum_field_needs_null_default`                                  | R P F T |
| An enum set's declared mask                                                                             | R `emit_enum_set` (the fold over bits in `0..=63`); T computes one mask per bit                                                                                                         | R T     |
| The FlatBuffers table layouts, the union discriminant, the root table, the size bound                   | `ridl_ir::projection::flatbuffers`, shared by F's schema and R's codec                                                                                                                  | R F     |
| The attribution of a missing FlatBuffers bound (member, untyped, layout, aggregate, exempt)             | `unbounded_member` and `unjudgeable_members` in `ridl-backend-rust/src/lib.rs`, called with `others: &[]`                                                                               | R       |
| The FlatBuffers wire kind of a position (scalar type and width, text, bytes, table, union, vector, map) | The codec's `Wire`, `Prim`, `Repr` in `ridl-backend-rust/src/codec.rs`; the keyword table `fbs_scalar` in F                                                                             | R F     |
| Interface number, provisional flag, member rows with ordinal, kind, name, timing, payloads              | `ridl-backend-rust/src/descriptors.rs`; the identity tables in P and F                                                                                                                  | R P F   |
| The one-parameter call shape, the named reply, the named `fixed` payload                                | `single_param_type`, `query_reply_type`, `fixed_payload_type` in `descriptors.rs`                                                                                                       | R       |
| The contract clause in the accepted form (`<subject> <comparison> <numeric literal>`)                   | `ridl-backend-rust/src/clauses.rs`: `parse`, then the subject's scalar class through `same_package_scalar_backing`                                                                      | R       |
| The catalog reference with the placeholder hash                                                         | `descriptors.rs`, `CatalogHash([0u8; 32])`                                                                                                                                              | R       |

Two things the survey shows that the driver's summary does not say:

- **The backends do not agree with each other on two names.** The induced name
  of a tuple nested in a tuple is `OuterInner` (by field name) in Rust and
  `OuterField2` (by position) in proto3 and FlatBuffers; and the CamelCase of a
  field name is `camel_case` in Rust and `type_name` in the wire backends, which
  differ on a name carrying an underscore (`foo_bar` → `FooBar` versus
  `Foo_bar`). The model cannot carry "the" induced name; it carries both
  spellings (D-2) and records the divergence for a later story (§9).
- **The resolution scope is part of the output.** `crates/ridlc/tests/corpus.rs`
  snapshots `ridl_backend_rust::generate(ir)` — no other package in scope —
  while `crates/ridlc/src/lib.rs`'s `write_emits` calls
  `generate_pipeline(ir, wire, others)` with every sibling and `ridl.std`. The
  codec's withheld-codec note names different causes in the two cases
  (`withheld_note`, `if self.ctx.others.is_empty()`), so byte identity against
  both the corpus snapshots and the CLI's output needs the scope in the model
  (D-1).

## 3. The model, message by message

The schema is `crates/ridl-ir/proto/ridl/codegen/v1/model.proto`, package
`ridl.codegen.v1`. Every listing below is the field set P2b writes, with the
numbers it assigns; a comment names where the value comes from. The listings
follow the IR's own numbering scheme (envelope and data fields from 1, `oneof`
members from 10) so the two schemas read alike. The enums the schema restates
rather than imports are in §3.8.

### 3.1 The root and the scope

    message Model {
      DottedName name = 1;                       // the package name (Package.name)
      Scope scope = 2;                           // what the lowering was handed
      repeated Declaration declarations = 3;     // Package.decls, same order, same count
      repeated InducedTuple tuples = 4;          // every tuple type reached, discovery order
      repeated Interface interfaces = 5;         // Package::shapes() order: declared, then inline
      repeated Service services = 6;             // Package.services, same order
      Catalog catalog = 7;
      FlatBuffersProjection flatbuffers = 8;
      repeated ForeignDeclaration foreign = 9;   // facts of the foreign declarations this package reaches
      repeated TupleCollision tuple_collisions = 10;
    }

    message Scope {
      string package = 1;                        // the package lowered
      repeated string others = 2;                // the other packages handed in, in the order given
    }

    message ForeignDeclaration {
      string package = 1;
      Declaration declaration = 2;               // the same message a local declaration is
    }

    message TupleCollision {                     // two paths spelled one induced name with different shapes
      string name = 1;
      repeated string first_fields = 2;
      repeated string second_fields = 3;
    }

`Model` is one package, lowered by
`lower(package: &v2::Package, others: &[&v2::Package]) -> Model` over the scope
`others`. The lowering is total: it returns a model for every IR the checker
accepts and, for malformed IR, carries the defect as a fact (a `TupleCollision`,
an `FbUnbounded` with the layout message, a `Type` with no kind) rather than
failing, because every printer refuses those today with its own message and must
keep doing so byte for byte.

**Positional correspondence** is an invariant P2b tests: `declarations[i]` is
lowered from `Package.decls[i]`, `interfaces[i]` from the `i`-th shape of
`Package::shapes()`, a `Struct.slots[j]` from `StructDef.members[j]`, an
`Interface.slots[j]` from `Interface.interactions[j]`. It is what lets a backend
read the model and the IR side by side during stage P4 (§8.3).

`foreign` holds a copy of each foreign declaration the package reaches,
transitively, through a type reference, a union arm, a constant's type, an enum
set's backing enum or an interaction payload — the closure
`projection::max_size` and the codec's `resolve` follow today. A `TypeRef` into
a foreign declaration indexes this list (§3.4).

### 3.2 Names

    message Spellings {                          // one per identifier the source declared
      string declared = 1;                       // as the IR carries it
      string snake = 2;                          // ridl_ir::name::snake_case (ADR-0016 decision 1)
      string camel = 3;                          // ridl_ir::name::camel_case (ADR-0016, 2026-09-20 amendment)
      string screaming = 4;                      // snake, upper-cased — what P and F call screaming_snake_case
    }

    message DottedName {                         // a package name or a service name
      string dotted = 1;                         // "veh.adas.cruise"
      repeated string segments = 2;              // ["veh", "adas", "cruise"]
      string joined_camel = 3;                   // "VehAdasCruise" — the wire backends' type_name
      string underscored = 4;                    // "veh_adas_cruise" — the TypeScript import alias
    }

    message InducedName {                        // the name of a type no source declared
      string rust = 1;                           // camel_case(parent) + camel_case(field), nested by camel_case(field), …Element/…Key/…Value
      string wire = 2;                           // owner + type_name(field), nested by Field<N>, …Element/…Key/…Value — proto3 and FlatBuffers agree
    }

A `Spellings` carries the three pinned outputs and the composition every wire
backend makes of one of them. A printer whose namespace is snake_case reads
`snake`, one whose namespace is CamelCase reads `camel`, and a printer that
keeps the source spelling — TypeScript, for a field — reads `declared`. The
target set is open (a plugin is any executable), which is why the fields are
named after the transform and not after a language (D-2).

`DottedName.joined_camel` pins the wire backends' `type_name` rule, which
ADR-0016 does not cover (§9, item 4): `camel_case` splits on underscores, so
over a dotted name it gives `Veh.adas.cruise`, not `VehAdasCruise`.

### 3.3 Declarations

    message Declaration {
      Spellings name = 1;
      Visibility visibility = 2;                 // resolved: PUBLIC or INTERNAL, never unspecified
      bool is_error = 3;
      string doc = 4;
      repeated string labels = 5;
      optional string deprecated = 6;            // present-and-empty is still #[deprecated] (typl §14.2)
      Closure closure = 7;                       // §3.5; absent on a constant
      Init init = 8;                             // §3.5; absent on a constant
      oneof kind {
        Scalar scalar = 10;
        Constant constant = 11;
        Struct struct = 12;
        Enum enum = 13;
        EnumSet enum_set = 14;
        Union union = 15;
      }
    }

    message Scalar {                             // a named scalar, and the shape of an inline one
      ScalarClass class = 1;                     // backing_scalar's total classification
      optional string unit = 2;                  // the canonical UCUM expression when the backing is a unit
      oneof width {                              // as the IR derives it; unset for boolean, string, bytes
        IntWidth int_width = 10;
        FloatWidth float_width = 11;
      }
      Constraint constraint = 3;                 // absent when the IR carries none
      bool vacuous = 4;                          // ridl_ir::v2::constraint_is_vacuous
      optional string declared_init = 5;         // unset on an inline scalar: the field's is authoritative
    }

    message Constraint {                         // the flat table; every bound is canonical decimal text
      optional string min = 1;
      optional string max = 2;
      optional string step = 3;
      optional uint64 len_min = 4;
      optional uint64 len_max = 5;
      optional string pattern = 6;               // the regex body, `/…/` stripped as strip_regex_delimiters does
      optional string pattern_const = 7;         // the canonical reference to the regex constant
    }

    message Constant {
      string value = 1;                          // canonical text; empty for a regex constant
      oneof typed {
        TypeRef named = 10;                      // a named scalar; its class is on the referenced declaration
        PrimitiveType primitive = 11;            // the keyword as written
        string regex_body = 12;                  // a regex constant, delimiters stripped
        string unresolved = 13;                  // the declared type the scope did not resolve
      }
    }

    message Struct {
      repeated Slot slots = 1;                   // StructDef.members, same order: a field or a tombstone
      bool fixed_layout = 2;                     // read by R's emit_struct today (§9, item 2)
    }

    message Slot {
      uint32 ordinal = 1;                        // typl §7.4
      oneof occupant {
        Field field = 10;
        Retired retired = 11;
      }
    }

    message Retired {                            // a tombstone
      optional Spellings name = 1;               // `reserved legacyChecksum`
      optional int64 value = 2;                  // `reserved 3` in an enum body
    }

    message Field {
      Spellings name = 1;
      Type type = 2;                             // absent when the IR carries none
      optional string declared_init = 3;
      Init init = 4;                             // §3.5
      string doc = 5;
      repeated string labels = 6;
      optional string deprecated = 7;
    }

    message Enum {
      repeated EnumValue values = 1;             // declaration order
      repeated Retired retired = 2;
      optional uint32 zero_member = 3;           // index into values of the member whose value is 0
      optional uint32 init_member = 4;           // index: the zero member, else the lowest value (typl §5.8)
    }

    message EnumValue {
      Spellings name = 1;
      int64 value = 2;
      string doc = 3;
    }

    message EnumSet {
      optional TypeRef backing_enum = 1;
      repeated EnumValue bits = 2;
      IntWidth width = 3;
      int64 declared_mask = 4;                   // OR of 1 << bit over the bits in 0..=63, as R's fold computes it
    }

    message Union {
      repeated Arm arms = 1;                     // declaration order
      repeated Retired retired = 2;
      bool is_result = 3;
    }

    message Arm {
      Spellings name = 1;
      uint32 ordinal = 2;
      TypeRef type = 3;
      string doc = 4;
      uint32 discriminant = 5;                   // projection::union_arm_discriminant: the ordinal
      optional uint32 flatbuffers_box = 6;       // index into FlatBuffersProjection.tables when the arm is boxed (ADR-0019 decision 2)
    }

The `Scalar` message carries the class every backend classifies for itself, the
width the IR already derives, the constraint as a flat table and the one derived
boolean (`vacuous`) on which a named scalar's constructor shape turns (`new` or
`new_unchecked`, `check` present or not). The Rust inner type (`i64`, `f64`,
`bool`, `String`, `Vec<u8>`), the proto3 scalar (`uint32`, `sint64`, …), the
FlatBuffers keyword and the TypeScript base (`number`, `bigint`) are each a
fixed table over (`class`, `width`) and stay with their printers (§7), except
that the FlatBuffers keyword and byte width are also carried in the projection
(§3.7), because two emitters must agree on them byte for byte.

`Enum.zero_member` serves four printers at once: the Rust and TypeScript init
(through `init_member`), the proto3 `UNSPECIFIED` synthesis and zero-lead
reorder, and the FlatBuffers `= null` (ADR-0019 decision 6). `Retired.value` is
what the proto backend writes as `reserved N;` and the name form as
`reserved "<PREFIX>_<NAME>";`.

### 3.4 Type positions, references and induced tuples

    message Type {                               // one type position: a field, a tuple field, a parameter, an element
      bool optional = 1;
      oneof kind {
        TypeRef named = 10;
        PrimitiveType primitive = 11;
        Scalar inline = 12;                      // an inline constrained scalar
        TupleRef tuple = 13;                     // lifted: see InducedTuple
        ArrayType array = 14;
        MapType map = 15;
        StreamType stream = 16;
      }
    }

    message TypeRef {                            // a resolved reference — flat, so it costs one nesting level
      string reference = 1;                      // the canonical IR string: bare Name or pkg.Name
      bool resolved = 2;                         // false: no package in scope declares it, or it names a constant
      string package = 3;                        // the declaring package, when resolved
      bool foreign = 4;                          // package != Scope.package
      uint32 index = 5;                          // into Model.declarations when local, into Model.foreign when foreign
      DeclKind kind = 6;                         // SCALAR, STRUCT, ENUM, ENUM_SET, UNION; CONSTANT when it names one
    }

    message TupleRef {
      uint32 index = 1;                          // into Model.tuples
    }

    message ArrayType {
      Type element = 1;
      uint64 min = 2;
      uint64 max = 3;
    }

    message MapType {
      Type key = 1;
      Type value = 2;
      uint64 min = 3;
      uint64 max = 4;
      optional uint32 flatbuffers_entry_table = 5;   // index into FlatBuffersProjection.tables
    }

    message StreamType {
      oneof element {
        TypeRef named = 10;
        PrimitiveType primitive = 11;
      }
    }

    message InducedTuple {
      InducedName name = 1;                      // both fields empty when no backend has a rule for this origin
      oneof origin {
        TuplePath declaration = 10;              // reached from a struct field, through arrays, maps and tuples
        InteractionPath interaction = 11;        // a parameter, a reply or a fixed payload of an interaction
      }
      repeated Field fields = 2;                 // typl §11: always named; Init computed per field
      Visibility visibility = 3;                 // of the declaration or interface that induced it
      Closure closure = 4;
      Init init = 5;                             // derivable iff every field is
      optional uint32 flatbuffers_table = 6;     // index into FlatBuffersProjection.tables
    }

    message TuplePath {
      uint32 declaration = 1;                    // index into Model.declarations
      repeated string segments = 2;              // field names and the positional words: ["span", "hi"], ["items", "Element"]
    }

    message InteractionPath {
      uint32 interface = 1;                      // index into Model.interfaces
      uint32 slot = 2;                           // index into Interface.slots
      repeated string segments = 3;              // ["reply"], ["param", "window"], ["payload"], then as TuplePath
    }

A tuple is anonymous in source and every backend that emits one gives it a name:
Rust and the two wire backends generate a declaration for it, and TypeScript
inlines it. The model therefore lifts every tuple into `Model.tuples`, in the
discovery order the Rust and proto backends' worklists share (declaration order,
then the fields of each tuple as they are found), and a tuple position is a
`TupleRef`. Two consequences: the type graph no longer nests through tuples,
which is what moves the nesting bound (§6); and the collision rule the Rust and
proto backends apply — two different tuples spelling one name — is applied once,
in the lowering, with the finding carried as a `TupleCollision` for the printer
to refuse with its own message.

A tuple reached from an interaction position — the corpus has one,
`query readSpan(): (min : RawTickCount, max : RawTickCount)` in
`crates/ridlc/tests/corpus/veh-cluster/cluster/internal-shape.ridl` — is listed
with its `InteractionPath` and both names empty, because no backend generates a
type for it today: the Rust face refuses a call whose reply is not a named type,
and the wire backends do not walk interactions for types. The story that gives
calls an induced argument struct (lane M's parked follow-up) fills the name; the
model is where it will be filled, once.

`TypeRef` is flat on purpose: it costs one message level on the type chain, and
the nesting bound of §6 is measured with it. The referenced declaration's
spellings, class, width, values and layout are read through `index`, not
duplicated.

### 3.5 Inits and closures

    message Init {
      bool derivable = 1;                        // typl §5.8, transitively, over the scope
      optional string value = 2;                 // the canonical scalar init text, where the position is scalar-valued
      InitSource source = 3;                     // DECLARED, DERIVED or NONE
    }

    message Closure {                            // facts about everything a declaration transitively contains
      bool reaches_float = 1;                    // a float or unit backing anywhere in the closure
      bool reaches_text_or_bytes = 2;            // a string or bytes backing
      bool reaches_collection = 3;               // an array or a map
      bool reaches_foreign = 4;                  // a reference into another package, resolved or not
      bool reaches_unresolved = 5;               // a reference no package in scope declares, or one to a constant
      bool reaches_cycle = 6;                    // the declaration reaches itself
      bool reaches_stream_or_unspecified = 7;    // a position no backend can type
    }

`Init.derivable` is the leaf-recursion rule of `defaults.rs` and T's
`init_function` family applied once, in the lowering: a composite is derivable
when every field it transitively contains is, a same-package reference is
re-checked by recursion with a cycle guard, and the IR's one-level
`InitValue.derivable` flag is trusted only for a reference the scope does not
resolve and that carries no declared init — which is exactly the case both
backends trust it in today. A field with a declared init on a reference the
scope does not resolve is not derivable, as both backends answer. An enum is
derivable when it has a member, an enum set always, a union when its first arm's
type is, a tuple when every field is; a signal's `Init` is the IR's resolved
channel init.

`Closure` is the transitive walk of `derives.rs`, stated as facts rather than as
Rust traits, so that the Rust printer computes `Copy`, `Eq`, `Hash` and the
ordering pair from it and a Kotlin printer computes whatever `data class` or
`value class` needs from the same seven booleans. The Rust rule, for the record:
the conditional derives are all refused when `reaches_foreign`,
`reaches_unresolved`, `reaches_cycle` or `reaches_stream_or_unspecified`;
otherwise `Copy` needs neither `reaches_text_or_bytes` nor `reaches_collection`,
and `Eq`/`Hash` need not `reaches_float`.

Three places where the Rust and TypeScript printers are more conservative than
the fact the model states, because each resolves nothing across packages today:
a field typed by a foreign named scalar that carries a declared init gets no
`Default`; a union whose first arm is foreign gets none; a constant typed by a
foreign scalar is skipped. The model states the resolved fact, and
`TypeRef.foreign` lets a printer keep the conservative rule. Stage P4 keeps it,
because P4 is measured by byte identity; removing it is a separate change to the
generated crate, made on its own (§9, item 6).

### 3.6 Interfaces, interactions, timing, clauses, and the catalog

    message Interface {
      oneof identity {
        Spellings declared = 10;                 // an `interface` declaration
        DottedName service = 11;                 // a service's inline shape: the service's dotted name
      }
      uint32 number = 1;                         // Interface.number from interfaces.lock; 0 before the lock
      bool provisional = 2;
      Visibility visibility = 3;                 // InterfaceShape::visibility(): the service's for an inline shape
      string doc = 4;
      repeated string labels = 5;
      optional string deprecated = 6;
      repeated InteractionSlot slots = 7;        // Interface.interactions, same order
      optional uint32 inline_of_service = 8;     // index into Model.services
    }

    message InteractionSlot {
      uint32 ordinal = 1;                        // ridl §11: one sequence, tombstones counted
      oneof occupant {
        Interaction interaction = 10;
        Retired retired = 11;
      }
    }

    message Interaction {
      Spellings name = 1;
      Kind kind = 2;                             // SIGNAL = 1 … FIXED = 5, the values ridl_rt::contract::Kind holds
      uint32 row = 3;                            // index among the live interactions: the MEMBERS row
      string doc = 4;
      repeated string labels = 5;
      optional string deprecated = 6;
      optional Timing timing = 7;                // absent for a fixed, and for a call with no declared bound
      oneof shape {
        SignalShape signal = 10;
        EventShape event = 11;
        CommandShape command = 12;
        QueryShape query = 13;
        FixedShape fixed = 14;
      }
    }

    message Payload {                            // one thing that crosses, as a named type
      TypeRef type = 1;
      optional uint32 flatbuffers_max_size = 2;  // projection::flatbuffers::max_size over the scope; absent when none
    }

    message SignalShape {
      Payload payload = 1;
      optional string declared_init = 2;         // the `= value` override (ridl §4.4)
      Init init = 3;                             // the resolved channel init (RIDL-109)
    }

    message EventShape {
      Payload payload = 1;
    }

    message CommandShape {
      repeated Param params = 1;
      optional Payload request = 2;              // present when the call declares exactly one named parameter
      repeated Clause clauses = 3;               // require only (RIDL-302)
    }

    message QueryShape {
      repeated Param params = 1;
      optional Payload request = 2;              // as for a command
      Reply reply = 3;                           // as declared
      optional Payload reply_payload = 4;        // present when the reply is one named type
      repeated Clause clauses = 5;
    }

    message FixedShape {
      Type payload = 1;                          // named type or array (ridl §8)
      optional Payload named = 2;                // present when the payload is a named type
    }

    message Param {
      Spellings name = 1;
      Type type = 2;
    }

    message Reply {
      oneof kind {
        Type value = 10;
        Fallible fallible = 11;
      }
    }

    message Fallible {
      TypeRef ok = 1;
      TypeRef err = 2;
    }

    message Timing {                             // as the IR resolves it; bounds are exact-decimal microsecond strings
      TimingMode mode = 1;
      optional string min_us = 2;
      optional string max_us = 3;
      bool default_applied = 4;
    }

    message Clause {
      ContractKind kind = 1;
      string source = 2;                         // the canonical text, as the IR carries it
      repeated string signal_refs = 3;
      repeated string param_refs = 4;
      bool uses_result = 5;
      string observer_id = 6;
      oneof translation {
        Comparison comparison = 10;              // the one form the narrow translator accepts
        string refused = 11;                     // the translator's reason, verbatim
      }
    }

    message Comparison {
      oneof subject {
        uint32 param = 10;                       // index into the interaction's params
        bool result = 11;                        // `result`, on a query's ensure clause
      }
      ComparisonOp op = 1;                       // LT, LE, GT, GE, EQ, NE
      string literal = 2;                        // as written
      ScalarClass literal_class = 3;             // INTEGER or FLOAT: the subject's class, which the literal matched
    }

    message Service {
      DottedName name = 1;
      Visibility visibility = 2;
      string doc = 3;
      repeated string labels = 4;
      optional string deprecated = 5;
      repeated string interface_refs = 6;        // the named form, source order
      optional uint32 inline_shape = 7;          // index into Model.interfaces, the inline form
    }

    message Catalog {
      string package = 1;                        // CatalogRef.name
      bytes hash = 2;                            // 32 bytes; all zero until E16.2 (driftsys/ridl#378)
      repeated RetiredInterface retired = 3;     // Package.retired, number order
    }

    message RetiredInterface {
      string name = 1;
      uint32 number = 2;
    }

**The D-P4 floor, item by item.** Decision D-P4 says the model must be
sufficient to generate an IPC binding: per package —

| D-P4 item                                      | In the model                                                                                                                         | Comes from                                                                                                                                                                                               |
| ---------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| the interface numbers                          | `Interface.number`, `Interface.provisional`                                                                                          | `ridl.ir.v2.Interface.number` and `.provisional`, folded from `interfaces.lock`; `descriptors.rs` writes them as `NUMBER` and `PROVISIONAL`                                                              |
| the ordinals                                   | `InteractionSlot.ordinal`, including the retired slots                                                                               | `ridl.ir.v2.Decl.ordinal` on an interaction, `Reserved.ordinal` on a tombstone; `descriptors.rs` `member_row`, the identity tables in P and F                                                            |
| each interaction's kind                        | `Interaction.kind`, values 1 to 5                                                                                                    | the `Decl.kind` member; the values are `ridl_rt::contract::Kind`'s (`crates/ridl-rt/src/contract.rs`), which the frame specification §2 fixes                                                            |
| its payload type                               | `Payload.type` on `SignalShape`, `EventShape`, `CommandShape.request`, `QueryShape.request` and `.reply_payload`, `FixedShape.named` | `SignalDef.payload`, `EventDef.payload`, the single named parameter (`single_param_type`), the named reply (`query_reply_type`), the named fixed payload (`fixed_payload_type`), resolved over the scope |
| its FlatBuffers `MAX_SIZE`                     | `Payload.flatbuffers_max_size`                                                                                                       | `ridl_ir::projection::flatbuffers::max_size(Packages { package, others }, decl)` over the referenced declaration in its declaring package — the number the codec emits as `MAX_SIZE` (`payload_impl`)    |
| the timing bounds                              | `Interaction.timing`                                                                                                                 | `SignalDef.timing`, `EventDef.timing`, `CommandDef.timing`, `QueryDef.timing`; absent where the IR leaves it absent (ADR-0015 decision 4), as `descriptors.rs` `timing_tokens` reads it                  |
| the contract clauses, in the translator's form | `Clause.translation`                                                                                                                 | `clauses.rs`: `parse` for the shape, `scalar_backing` for the subject's class, `literal_tokens` for the literal check; a refusal carries `refuse`'s reason                                               |
| the catalog hash, all zeros until E16.2        | `Catalog.hash`                                                                                                                       | `descriptors.rs`: `CatalogHash([0u8; 32])`; the frame specification §6.1 states the placeholder                                                                                                          |

`Payload.flatbuffers_max_size` is a `uint32`, not the `u64` the projection
returns: the projection refuses any bound above `MAX_ENCODABLE`, which is
`u32::MAX`, and `ridl_rt::contract::EncodedSizes.flatbuffers` is `Option<u32>`,
so a `uint32` is exact and renders as a JSON number rather than a string.

The clause translator's accepted form depends on facts the plugin does not have
— which parameter the subject names and whether that parameter's type is an
integer or a float scalar — so "the contract clauses in the form the translator
accepts" can only mean the translation itself. The model carries it, and the
translator's parser moves from `ridl-backend-rust/src/clauses.rs` into the
lowering at stage P4's third layer; the Rust printer then renders
`args.0 <op> <literal>` from a `Comparison` and composes
`"cannot translate contract clause`<source>`: <refused>"` from a refusal, which
is what the skipped-interface note (`skipped_interface_note`) prints today.
Story E5.1, which replaces the text clause with an expression tree in the IR,
replaces `Comparison` with the tree's lowered form under D-5 of the preceding
note (a new `oneof` member is additive).

`CommandShape.request` and `QueryShape.request` are the request payload only
where the call has the shape the face carries — one named parameter. A call with
another shape has `params` and no `request`; the Rust face skips it with a note
today, and a Kotlin binding meets the same limit until the induced argument
struct lands, at which point `request` names that struct's `TupleRef`-like
induced type. Nothing here invents that rule.

### 3.7 The FlatBuffers projection

    message FlatBuffersProjection {
      repeated FbRoot roots = 1;                 // one per declaration root_table names, declaration order
      repeated FbTable tables = 2;               // every table the projection induces for this package
    }

    message FbRoot {
      uint32 declaration = 1;                    // index into Model.declarations
      FbRootKind root = 2;                       // OWN, UNION_WRAPPER, BOX (projection::root_table)
      oneof bound {
        uint32 max_size = 10;                    // projection::max_size over the scope
        FbUnbounded unbounded = 11;
      }
    }

    message FbUnbounded {                        // unbounded_member's attribution, computed as it is today
      FbUnboundedCause cause = 1;                // MEMBER, UNTYPED, LAYOUT, AGGREGATE, EXEMPT
      optional string member = 2;                // MEMBER, UNTYPED: the member named
      optional string layout_message = 3;        // LAYOUT: the projection's own message
      repeated string unjudgeable_members = 4;   // EXEMPT: what the withheld note lists
    }

    message FbTable {
      string name = 1;                           // the `.fbs` table name
      oneof source {
        uint32 struct_declaration = 10;          // its own table: the declaration's name
        uint32 union_wrapper = 11;               // `<Name>`, the wrapper table of a union declaration
        ArmBox arm_box = 12;                     // `<Union><Arm>Box` (ADR-0019 decision 2)
        uint32 root_box = 13;                    // `<Name>Box` (decision 8)
        uint32 tuple = 14;                       // index into Model.tuples: InducedName.wire
        MapEntry map_entry = 15;                 // `<path>Entry`
      }
      repeated FbSlot slots = 2;                 // projection::TableLayout.slots, same order
      uint32 vtable_slots = 3;                   // TableLayout::vtable_slots
    }

    message ArmBox {
      uint32 declaration = 1;
      uint32 arm = 2;
    }

    message MapEntry {
      repeated string path = 1;                  // the path that reached the map, as the induced name spells it
    }

    message FbSlot {
      uint32 id = 1;                             // FieldSlot.id: the typl ordinal minus one, or the fixed ids
      oneof source {                             // SlotSource
        uint32 field = 10;                       // index into the owner's slots (a live field)
        uint32 retired_ordinal = 11;
        uint32 tuple_position = 12;
        bool key = 13;
        bool value = 14;
        bool wrapped = 15;
      }
      FbWire wire = 2;                           // what the slot holds on the wire
      bool needs_null_default = 3;               // projection::enum_field_needs_null_default on an enum-typed slot
    }

    message FbWire {
      oneof kind {
        FbScalar scalar = 10;
        bool text = 11;                          // an out-of-line string
        bool bytes = 12;                         // an out-of-line [ubyte]
        uint32 table = 13;                       // index into tables: a nested table or a tuple's
        uint32 union_wrapper = 14;               // index into tables
        FbVector vector = 15;
        FbMap map = 16;
      }
    }

    message FbScalar {
      FbScalarType type = 1;                     // BOOL, BYTE, UBYTE, SHORT, USHORT, INT, UINT, LONG, ULONG, FLOAT, DOUBLE
      uint32 width_bytes = 2;                    // 1, 2, 4 or 8
    }

    message FbVector {
      FbWire element = 1;
      uint64 min = 2;
      uint64 max = 3;
    }

    message FbMap {
      uint32 entry_table = 1;                    // index into tables
      uint64 min = 2;
      uint64 max = 3;
    }

The projection module is the precedent for the whole model and is carried whole:
the slot ids, sources and vtable counts are `projection::TableLayout`
(`struct_table`, `tuple_table`, `map_entry_table`, `union_wrapper_table`,
`union_arm_box_table`, `root_box_table`), the roots are `root_table`, the bound
is `max_size`, the discriminant is on `Arm`, and the `= null` fact is
`enum_field_needs_null_default`. What is added beyond the module is what the
codec's `Wire` classification and the schema emitter's `fbs_scalar` both derive
from the IR and must agree on: the wire kind of every slot with the scalar's
FlatBuffers type and byte width, and the names of the induced tables
(`<Name>Box`, `<Union><Arm>Box`, `<path>Entry`, the tuple's wire name), which
the schema emitter spells and the codec repeats in `__ridl_fb_*` function names.

The attribution of a missing bound is carried as `unbounded_member` computes it
today — over the package alone, because `check_flatbuffers_bound` passes
`others: &[]` even when the pipeline handed the codec a scope. That is what byte
identity requires; it is also less precise than it could be with a scope, and §9
item 7 records it.

An `FbTable` for a tuple or a map entry is listed whether or not a bounded root
reaches it. Which tables the codec writes functions for (its `reachable_tuples`)
is a walk from the bounded roots over the model, which a printer does for itself
(§7).

The inline layout — which byte of a table a field starts at, the table's size
and alignment — is not in the model. `docs/design/flatbuffers-codec.md` ("The
shared facts live outside both backends") places it in `codec.rs` because only
the codec observes it, and a FlatBuffers reader in any language reads a field
through the vtable, never at a fixed offset.

### 3.8 The enums the schema restates

`ridl.codegen.v1` imports nothing from `ridl.ir.v2`. It restates the enums it
needs, with the IR's values where the IR has them:

    enum Visibility        { VISIBILITY_UNSPECIFIED = 0; VISIBILITY_PUBLIC = 1; VISIBILITY_INTERNAL = 2; }
    enum PrimitiveType     { PRIMITIVE_TYPE_UNSPECIFIED = 0; …_BOOLEAN = 1; …_INTEGER = 2; …_FLOAT = 3; …_STRING = 4; …_BYTES = 5; }
    enum IntWidth          { INT_WIDTH_UNSPECIFIED = 0; …_U8 = 1; …_I8 = 2; …_U16 = 3; …_I16 = 4; …_U32 = 5; …_I32 = 6; …_U64 = 7; …_I64 = 8; }
    enum FloatWidth        { FLOAT_WIDTH_UNSPECIFIED = 0; …_F32 = 1; …_F64 = 2; }
    enum TimingMode        { TIMING_MODE_UNSPECIFIED = 0; …_STRICT_PERIODIC = 1; …_RANGE = 2; }
    enum ContractKind      { CONTRACT_KIND_UNSPECIFIED = 0; …_REQUIRE = 1; …_ENSURE = 2; }
    enum Kind              { KIND_UNSPECIFIED = 0; KIND_SIGNAL = 1; KIND_EVENT = 2; KIND_COMMAND = 3; KIND_QUERY = 4; KIND_FIXED = 5; }
    enum ScalarClass       { SCALAR_CLASS_UNSPECIFIED = 0; …_FLOAT = 1; …_INTEGER = 2; …_BOOLEAN = 3; …_STRING = 4; …_BYTES = 5; }
    enum DeclKind          { DECL_KIND_UNSPECIFIED = 0; …_SCALAR = 1; …_CONSTANT = 2; …_STRUCT = 3; …_ENUM = 4; …_ENUM_SET = 5; …_UNION = 6; }
    enum InitSource        { INIT_SOURCE_UNSPECIFIED = 0; …_DECLARED = 1; …_DERIVED = 2; …_NONE = 3; }
    enum ComparisonOp      { COMPARISON_OP_UNSPECIFIED = 0; …_LT = 1; …_LE = 2; …_GT = 3; …_GE = 4; …_EQ = 5; …_NE = 6; }
    enum FbRootKind        { FB_ROOT_KIND_UNSPECIFIED = 0; …_OWN = 1; …_UNION_WRAPPER = 2; …_BOX = 3; }
    enum FbUnboundedCause  { FB_UNBOUNDED_CAUSE_UNSPECIFIED = 0; …_MEMBER = 1; …_UNTYPED = 2; …_LAYOUT = 3; …_AGGREGATE = 4; …_EXEMPT = 5; }
    enum FbScalarType      { FB_SCALAR_TYPE_UNSPECIFIED = 0; …_BOOL = 1; …_BYTE = 2; …_UBYTE = 3; …_SHORT = 4; …_USHORT = 5; …_INT = 6; …_UINT = 7; …_LONG = 8; …_ULONG = 9; …_FLOAT = 10; …_DOUBLE = 11; }

The `_UNSPECIFIED` zero member is proto3's rule, not a value the lowering
writes: every enum field in a lowered model holds a named value, and a reader
that meets a zero is reading a model a later schema wrote (D-5 of the preceding
note, the enum row).

## 4. Decisions

### D-1. One model per package, lowered over a stated scope, total, positionally aligned with the IR

`lower(&v2::Package, others) -> Model` produces one `Model` per package, records
the scope it was handed (`Scope`), inlines the foreign declarations it reached
(`Model.foreign`), never fails, and keeps every repeated field in the IR's own
order and count so that `declarations[i]` is `decls[i]` (§3.1).

**Reason.** `write_emits` generates one package at a time and the request of
ADR-0020 decision 9 carries one model, so the unit is the package. The scope is
in the model because the output depends on it (§2, second finding) and because a
plugin must be able to tell "this reference names a package I was not given"
from "this reference names nothing". Totality is the codegen contract (ADR-0004
§5) one layer down: a printer that must refuse malformed IR with its own message
needs the fact, not an error that replaced it. Positional correspondence is what
makes the transition of §8.3 possible.

**Rejected.** A whole-build model: the request is per package, a plugin writes
one file per package, and the facts a package needs from its siblings are a
small closure. A model that resolves nothing (references left as strings): it is
the raw IR with names attached, and every plugin resolves again, which is what
D-P2 forbids. A fallible `lower`: it would put a refusal in the compiler where
four printers already word one each.

### D-2. Names are carried by transform, not by language; induced names by rule

Every declared identifier carries `Spellings` (declared, snake, camel,
screaming); every dotted name carries `DottedName` (dotted, segments,
joined_camel, underscored); every induced tuple carries `InducedName` with the
Rust rule and the wire rule spelled out (§3.2).

**Reason.** ADR-0020 decision 8 asks for names already transformed by ADR-0016's
pinned rules, and the driver asks for them per target namespace. The namespaces
the four backends use are exactly the pinned transforms and two compositions of
them; a field named after the transform serves every current backend and every
plugin, which a field named after a language could not. The induced names differ
between Rust and the wire backends (§2), so the model states both rules rather
than one.

**Rejected.** Per-language fields (`rust`, `proto3`, `flatbuffers`,
`typescript`) on every name: a closed set on an open surface, and three of the
four would hold the same string. One spelling plus a rule the printer applies:
that is the raw IR, and it re-derives the transform decision 8 says to lower
once. Unifying the two induced-name rules here: it moves generated names in one
backend, a breaking change to a shipped API that a design note cannot take.

### D-3. Type positions resolve their references, and tuples are lifted out of the type graph

A `Type` holds a flat `TypeRef` (resolved, package, foreign, index, kind) for a
named position and a `TupleRef` for a tuple; every tuple is an `InducedTuple` in
`Model.tuples` with its origin path (§3.4).

**Reason.** Resolution is the one thing every backend does again and the one
thing a plugin cannot do without the sibling packages; a flat `TypeRef` costs
one nesting level. Lifting tuples matches what three backends already do — each
generates a declaration per tuple and names it — and it is the single change
that turns the IR's deepest shape (386 message levels) into a constant (§6).

**Rejected.** An inline recursive `TupleType`, as the IR has: it keeps the tuple
shape as the nesting bound's worst case and leaves the collision rule to each
printer. A `TypeRef` that carries the referenced declaration's facts inline: it
duplicates what `index` reaches and deepens the type chain by the size of
`Spellings` and `Scalar`.

### D-4. A scalar carries its class, unit, width, flat constraint and vacuity; a constant its resolved type

§3.3's `Scalar`, `Constraint` and `Constant`.

**Reason.** The class is the classification four backends make for themselves,
with the totality rules (`backing_scalar`) that a hand-built IR needs; the width
is what the IR derives and the driver names; the constraint is the flat table
ADR-0020 decision 8 names; `vacuous` is the one boolean the constructor's shape
turns on and is read from `ridl_ir::v2` today. A constant resolved in the
lowering keeps `emit_const` from resolving it twice.

**Rejected.** Carrying the Rust inner type, the proto3 scalar, the FlatBuffers
keyword and the TypeScript base on every scalar: each is a fixed table over
(class, width), owned by a printer, except the FlatBuffers one, which the
projection carries for the reason §3.7 gives. Carrying the constructor's name
(`new`/`new_unchecked`): a Rust idiom over `vacuous`.

### D-5. Inits are resolved once, transitively; closures are stated as facts

§3.5's `Init` on every declaration, field, tuple field and signal; `Closure` on
every declaration and tuple.

**Reason.** The leaf-recursion rule exists in two copies today and the
TypeScript one says so in its module header; the driver names "init values
resolved" as the model's job. The closure facts are the walk `derives.rs` makes,
and a plugin in a language with its own notion of value equality needs the same
seven answers. Both walks resolve across packages, which is what a plugin cannot
do.

**Rejected.** Passing the IR's one-level `InitValue.derivable` through: it is
wrong for a composite whose member is not derivable (T15), and every printer
would recurse again. Carrying Rust's derive list: a Rust idiom over the facts.
Carrying no closure and letting each printer walk: the walk crosses packages.

### D-6. Slots hold tombstones in place; enums and unions carry their derived members

§3.3's `Slot`, `InteractionSlot`, `Enum.zero_member`, `Enum.init_member`,
`EnumSet.declared_mask`, `Arm.discriminant`.

**Reason.** The driver says "tombstones resolved into slots", and the proto
backend needs the tombstone where it sits (`reserved N;` is written in member
order). The zero member is read four ways by four backends; the init member is
typl §5.8; the mask is the fold two backends make; the discriminant is
`projection::union_arm_discriminant`, carried so a reader over the model needs
no projection function.

**Rejected.** Fields and tombstones as two lists: the proto output order is
lost. A boolean `has_zero_member` instead of the index: the proto zero-lead
reorder and the Rust init both need the member, not the fact.

### D-7. The FlatBuffers projection is carried whole, plus the wire kind of every slot and the induced table names

§3.7.

**Reason.** The projection module is the precedent the driver names: facts two
emitters must agree on byte for byte, computed once. The schema emitter and the
codec additionally agree on the scalar keyword and width of each slot and on the
names of the induced tables, which the module does not compute and each of the
two derives; carrying them closes that gap, and it is what D-P4's `MAX_SIZE`
rests on. The unbounded attribution moves in with the bound, computed as it is
today.

**Rejected.** Carrying only `max_size` per declaration: the `.fbs` emitter and
the codec need the layouts, and a Kotlin codec reading a parcel needs the slot
ids. Carrying the inline offsets: the codec's own, per the design record.
Carrying `reachable_tuples`: a walk over the model (§7).

### D-8. Interfaces carry every shape, every slot, and the interaction facts the descriptors and the frame need

§3.6.

**Reason.** This is D-P4's floor, checked item by item in §3.6's table, and the
facts the descriptors emit today. The inline shape of a service is a shape
(`Package::shapes()`) and the wire backends emit its identity table, so it is
carried with the service's `DottedName`; the Rust descriptors skip it today and
still can.

**Rejected.** Carrying only the live interactions (`MEMBERS` rows): the identity
tables write the retired ordinals, and the frame treats a retired ordinal as
unknown, which a binding must know. Carrying the kind as a string: the frame
fixes the values 1 to 5 and `ridl-rt` holds them.

### D-9. The clause translation is a model fact; a refusal is carried with its reason

`Clause.translation` (§3.6).

**Reason.** D-P4 asks for the clauses in the form the translator accepts, and
that form is a function of facts only the lowering holds (the subject's scalar
class). The translator becomes part of the lowering at P4's third layer; the
refusal reasons are carried verbatim because they are printed into generated
source today.

**Rejected.** Carrying the source text only: every plugin writes a parser for a
form E5.1 retires. Refusing in the lowering: a printer that skips an interface
with a note needs the reason as data.

### D-10. The model does not carry what a printer computes from the model alone

§7 is the list.

**Reason.** The driver's rule, and the line between a semantic fact (resolution,
a typl rule, a projection rule, a transform) and a target idiom (a keyword
table, an escape, a literal form, a suffix). The first kind changes when the
language or a projection changes and must change in one place; the second
changes when a target's conventions change and belongs to that target.

**Rejected.** Carrying every string a backend emits: that is the output, not a
model. Carrying the per-target keyword tables: see D-4.

### D-11. The home is `crates/ridl-ir`, as `ridl_ir::codegen`

§5.

### D-12. The encoding is canonical protobuf JSON, in its own package, written by `ridl build --emit codegen-model`

§6.

### D-13. Drift is caught by one test per backend, in two forms, staged

§8.

### D-14. The model carries no version field; the request does, and the compatibility rule of the preceding note applies

`ridl.codegen.v1.Model` has no `schema` or `toolchain` field. The
`CodegenRequest` of stage P3 carries both as its first two fields, as D-6 of the
preceding note fixes, and the model artifact `ridl build --emit
codegen-model`
writes is the request's `model` field byte for byte.

**Reason.** The preceding note's reasoning for the IR artifacts holds here: the
reader's schema and the request's `schema` field tell versions apart, and a
plugin's fixture is then a file `ridlc` wrote. Every change P4 makes to the
model while porting the Rust backend is a new field or a new `oneof` member,
additive under D-5 of that note; a breaking change is `ridl.codegen.v2`.

**Rejected.** A version field on the model: two places to read one fact, and a
fixture file that differs from the request's payload.

## 5. O-P2 — the home

The driver asks for the proposal "in the shape K2 used for the projection facts
(the ground for the choice stated)". Stage K2 was given two candidates by its
plan — `crates/ridl-ir/src/projection.rs` or a new `crates/ridl-projection/`
([the K2 task](../archive/2026-09-20-flatbuffers-codec-plan.md), Task 1) — and
the design note's open item 1 closed the choice with its ground: the facts live
outside any backend, in `ridl-ir` or a small projection crate, "so that
`ridl-backend-rust` and later `ridl-descriptor` reach them without depending on
a backend. The ground is ADR-0020 decision 9 — a backend is not a library other
crates link"
([the codec design, §4](../archive/2026-09-20-flatbuffers-codec-design.md)). The
module then recorded why `ridl-ir` and not the small crate: "`ridl-ir` is the
crate every consumer of a projection already depends on"
(`crates/ridl-ir/src/projection.rs`, module header), which is also decision 2 of
ADR-0016 for `name.rs`. This section states the same three things for the model:
the candidates, the ground, and the choice with its reopen condition.

### 5.1 The candidates

**A — `crates/ridl-ir`.** A module `ridl_ir::codegen` holding the generated
types (`ridl_ir::codegen::v1`), the lowering (`ridl_ir::codegen::lower`) and the
encoding functions, with the schema at
`crates/ridl-ir/proto/ridl/codegen/v1/model.proto`, compiled by the existing
`build.rs` beside `ir.proto` and `system.proto`.

**B — `crates/ridl-codegen`.** A new crate holding the same, depending on
`ridl-ir` for the IR types, `name` and `projection`.

### 5.2 What each does to the gates and the graph

| Concern                                            | A — `ridl-ir`                                                                                                                                                        | B — `ridl-codegen`                                                                                                                                                                                                   |
| -------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `just wasm-check`                                  | `ridl-ir` is already in the `-p` list with `--no-default-features`; the lowering adds no dependency (std collections, the transforms, the projection, a hand parser) | the crate must be added to the `-p` list in `justfile`, or the model silently leaves the wasm gate                                                                                                                   |
| `.git-std.toml`                                    | the scope `ridl-ir` exists                                                                                                                                           | a new scope line, per AGENTS.md and issue #180                                                                                                                                                                       |
| `Cargo.toml` (workspace)                           | no change                                                                                                                                                            | a `[workspace.dependencies]` entry with `version = "0.2.0"`, a description, and a place in the crates.io publish set (ADR-0007 decision 14 as amended)                                                               |
| The four backends' dependency graph                | unchanged: each depends on `ridl-ir` and gains nothing                                                                                                               | each gains an edge to `ridl-codegen`; ADR-0020 decision 9 is untouched either way (a backend still links no backend)                                                                                                 |
| `ridlc`                                            | unchanged                                                                                                                                                            | gains an edge, for `lower` and the `--emit codegen-model` writer                                                                                                                                                     |
| The P3 process host and reference plugin           | depend on `ridl-ir`, which they need anyway for nothing else — the request messages live beside the model in either option                                           | depend on `ridl-codegen`, which depends on `ridl-ir`                                                                                                                                                                 |
| The canonical encoding (D-4 of the preceding note) | one `build.rs`, one `pbjson_build::Builder` call over both packages, one descriptor pool for prototext, one `render_json`, one strict reader with the nesting cap    | a second `build.rs` compiling one more `.proto` with the same three build dependencies, and either a second copy of `render_json` and the reader guards or a `pub` re-export of them from `ridl-ir`                  |
| A Rust plugin author's dependency                  | `ridl-ir` (published), which brings `prost`, `pbjson`, `prost-reflect`, `serde_json`                                                                                 | `ridl-codegen` (published), which brings `ridl-ir` and the same four                                                                                                                                                 |
| What `ridl-sem`, `ridl-diff`, `ridl-lsp` compile   | the lowering too, unused — on the order of the projection module's size, twice                                                                                       | nothing new                                                                                                                                                                                                          |
| Where `name` and `projection` live                 | where they are; the model is the third shared derivation beside the two, in one module tree                                                                          | `name` cannot move — `ridl-sem` reads it for RIDL-149 (`crates/ridl-sem/src/check.rs`), and a checker must not depend on a codegen crate — so the "one crate for codegen facts" the option promises is not reachable |
| The crate's description                            | widens: "the IR, its encodings, and the lowered codegen model"                                                                                                       | a crate whose description is exactly its content                                                                                                                                                                     |

### 5.3 The recommendation

**A — `crates/ridl-ir`, as `ridl_ir::codegen`.** The ground is K2's, applied to
the thing K2's facts are part of: the model is the projection facts generalized
— it carries what `name` produces and what `projection` produces and adds the
resolution and the typl rules the backends still derive — and `ridl-ir` is the
crate every consumer of the model already depends on, so no edge in the graph
moves and ADR-0020 decision 9 holds as it does today. The encoding machinery the
preceding note's D-4 makes a contract exists once, in that crate's `build.rs`
and `lib.rs`, and the model must be written in that exact dialect; a second
crate either copies it or reaches back for it. The wasm gate covers the new code
without a `justfile` edit, the commit lint needs no new scope, and no publish
entry is added.

The costs are the two the table shows: `ridl-ir`'s description widens, and three
crates compile a lowering they never call. Neither changes a gate or a consumer.

**Reopen condition.** If the lowering ever needs a dependency `ridl-ir` must not
carry under `--no-default-features` on `wasm32` — a regex engine to validate a
pattern at lowering time would be one — or if a consumer needs the model's types
without the IR's (a plugin SDK crate is the candidate), the module moves to
`crates/ridl-codegen` with its schema, and `name` and `projection` stay where
they are. The `.proto` file and the package name do not change with the move, so
a plugin notices nothing.

## 6. The encoding, and the nesting bound

### 6.1 The encoding

The canonical encoding of `ridl.codegen.v1.Model` is canonical protobuf JSON as
D-1 and D-4 of the preceding note fix it: the pbjson-generated `Serialize` impl,
pretty-printed by `render_json`, with the six properties D-4 lists (the mapping,
presence, order, layout, no host input, one form). P2b generates the impls with
the same `pbjson_build::Builder` call `build.rs` makes for `ridl.ir.v2`, adding
`".ridl.codegen.v1"` to its package list, and exposes
`ridl_ir::codegen::to_json_pretty(&Model)` and `from_json(&str)` with the same
`MAX_JSON_NESTING` cap and sized stack `v2::from_json` has (ADR-0014 decision
14). Binary and prototext are derived and available through the same generated
code and the same descriptor pool; no emit writes them for the model until a
consumer asks, and D-2 of the preceding note's binary bound applies unchanged if
one does (100 message levels below the root, which the model reaches from source
depth 48 for arrays and maps).

**The artifact.** `ridl build --emit codegen-model` writes `<base>.codegen.json`
beside the package's other artifacts, one per package, `ridl.std` included when
the build references it — the rule the code emits follow, because the model is
lowered over the same scope they read. In `crates/ridlc/src/lib.rs` the new
`Emit` member is classified with the code emits (`is_ir_dump()` false, so
`ridl.std` joins the scope), and its suffix joins the one table `ir_dump_suffix`
keeps for the dumps as a code-class suffix, so ADR-0014 decision 10's exhaustive
classification holds. The flag value is `codegen-model`, lower-case and
hyphenated like `ir-json`, under ADR-0010's consistency rule for `--emit`
between `ridl build` and `ridlc build`; `docs/book/cli-reference.md` gains the
value. `ridl diff` and `ridl check --baseline` do not read the model.

**What "canonical" fixes for a list the model adds.** Every repeated field is in
an order the schema comment names (§3): the IR's order for what mirrors the IR,
discovery order for `tuples`, `shapes()` order for `interfaces`, declaration
order for `roots`, and the order the lowering reaches them for `tables` and
`foreign` (declaration walk, then interfaces, depth first). No `map<>` field
exists (D-4 item 3 of the preceding note).

### 6.2 The nesting bound, restated for this schema

D-3 of the preceding note asks this note to say whether the model nests a type
at the IR's cost or flattens it, and to give the numbers. Counted from §3's
schema, with the same method as that note's §2 (message levels below the
artifact's root, and JSON bracket levels):

- **A struct field's type is five message levels below the root:** `Model` →
  `Declaration` (1) → `Struct` (2) → `Slot` (3) → `Field` (4) → `Type` (5). The
  IR's is five too (`Decl`, `StructDef`, `StructMember`, `Field`, `FieldType`).
- **An array or a map costs two levels per source level**, as in the IR: `Type`
  → `ArrayType` → `Type`, or `Type` → `MapType` → `Type`. A tuple costs none: a
  tuple position is a `TupleRef`, and the tuple's own fields are five levels
  below the root in their own `InducedTuple`.
- **The leaf.** A primitive leaf ends at its `Type`; a named leaf adds one
  (`TypeRef`); an inline scalar leaf adds two (`Scalar`, `Constraint`), and a
  map's `string` key is one, because the checker materializes `[0..256]`.

So the deepest IR the front end admits (source depth 127, the tuple shape at 386
message levels and 516 JSON levels in the IR) lowers to a model whose deepest
chain is the **map shape: 261 message levels below the root** (the 127th
`MapType` at 258, its key `Type` at 259, the key's `Scalar` at 260, its
`Constraint` at 261) and **264 JSON levels** (`Model` 1, `declarations` 2,
`Declaration` 3, `struct` 4, `slots` 5, `Slot` 6, `field` 7, `type` 8, then two
per map level to the 127th `map` at 261, its `key` at 262, `inline` at 263,
`constraint` at 264). The array shape with a named leaf reaches 260 and 263;
with a primitive leaf, 259 and 262 — the IR's array figures. The tuple shape is
a constant: a chain of 127 nested tuples is 127 `InducedTuple` entries, each
five levels deep.

The bound a reader of `ridl.codegen.v1` must provision is therefore **264 JSON
levels**, and one built on a schema-typed parser **261 message levels**; the
policy's recommendation of 1,000 stands, and leaves a factor of 3.8 rather than
the IR's 1.9. From the root of a `CodegenRequest` (stage P3), every number above
is one greater, because the request holds the model in a field. The
`MAX_TYPE_DEPTH` coupling of D-3 holds for the model as it does for the IR: the
three numbers move only with that constant or with the per-level cost of the
schema, and P2b's test (§10) pins them the way the preceding note's §6 test pins
the IR's — the three shapes generated at source depth 127, lowered, and their
depths asserted.

## 7. What the model does not carry, and why

The rule (D-10): nothing a printer can compute from the model alone, where
"compute" means a function of the model's own facts with no resolution, no typl
rule and no projection rule inside it. What that excludes, by printer:

- **Target keyword tables** over (`ScalarClass`, width): Rust's `i64`/`f64`/
  `bool`/`String`/`Vec<u8>`, proto3's `uint32`/`sint64`/…, TypeScript's
  `number`/`bigint`/`Uint8Array`. The FlatBuffers keyword is the exception
  (§3.7). The proto3 table is ADR-0017's and joins the model as a
  `projection::proto3` module when the proto backend is ported, not before.
- **Identifier escaping and literal forms**: `r#override`, the `_` suffix on
  `crate`/`self`, the `.0` on a float literal, the `n` on a bigint, the
  single-quoted TypeScript string, `strip_regex_delimiters`'s inverse.
- **Composed generated names**: `<Name>FbView`, `__ridl_fb_encode_<snake>`,
  `<Interface><Camel>` for a descriptor, `<Camel>Correlation`,
  `<joined_camel>Ordinal`, `<PREFIX>_UNSPECIFIED`, `<PREFIX>_<SCREAMING>`,
  `<Name>Bits`, `init<Name>`, `reserved_<N>`, `field_<N>`. Each is a fixed
  suffix or prefix over a `Spellings` field. The induced _table_ names of the
  FlatBuffers projection are carried, because two emitters must agree on them.
- **Rust derive lists, `#[repr(...)]`, `#[allow(deprecated)]`, doc attributes**;
  the `Wire` alias and its collision refusal; the ports a face needs (a function
  of which kinds an interface declares).
- **The FlatBuffers inline layout** (`place`), the buffer alignment, and
  `reachable_tuples`; the `MAX_BUFFER_SIZE` and `EVENT_SOURCE_BUFFER_SIZE`
  maxima, which are maxima over `Payload.flatbuffers_max_size`.
- **Target totality checks**: proto3's reserved field-number range and maximum,
  the `SymbolScope` and `Namespace` collision guards, TypeScript's
  `MAX_SAFE_INTEGER` refusals, `refuse_optional` on a vector element. Each is a
  property of a target, checked where the target is spelled.
- **The proto3 zero-lead reorder and `UNSPECIFIED` synthesis**, over
  `Enum.zero_member`; the FlatBuffers `deprecated` placeholder over a `Retired`
  slot.
- **`min == max`** for a fixed array, and **`is_wide`** for a 64-bit width.

Two things are deliberately carried although a printer could compute them, each
with its reason stated where it sits: `Spellings.screaming` (one composition
three printers make of a pinned transform; §3.2) and `EnumSet.declared_mask` (a
typl §9 fact two printers fold; §3.3).

## 8. How drift is caught

The driver asks for "a test per backend that the backend's output over the model
equals its output over the raw IR, which is the test P4 makes total". No backend
can produce output over the model before it is ported, so the test takes two
forms, and the driver's sentence describes the second.

### 8.1 The P2b form: a fact-level drift test per backend

The precedent is `drift` in `crates/ridl-backend-flatbuffers/src/tests.rs`
(stage K2): the emitted `.fbs` text is read back, the field ids it states are
extracted, and they are compared with what `projection::flatbuffers` says, over
the corpus fixtures. P2b generalizes it: for each backend, over every corpus
package, the backend's output over the raw IR is read back, the facts the model
also states are extracted, and each is asserted equal to the model's field. The
comparison is exact, per fact; what is compared is:

| Backend       | Reader over the output                                                                                      | Facts compared, and the model field each must equal                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            |
| ------------- | ----------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `rust`        | `syn::parse_file` (already a dependency) over `generate(ir)` and `generate_pipeline(ir, wire, others)`      | each item's name = `Spellings.declared`; each struct field's ident = `Field.name.snake`; each union variant = `Arm.name.camel`; each enum discriminant = `EnumValue.value`; `impl Default for X` present iff `Init.derivable`; the `#[derive]` list = the Rust rule of §3.5 over `Closure`; each induced tuple struct = `InducedName.rust`; `const MAX_SIZE: usize = N` = `FbRoot.max_size`; a `__RIDL_FB_NO_CODEC_*` note present iff `FbUnbounded` with `EXEMPT`; `NUMBER`, `PROVISIONAL`, each `Member { ordinal, kind, name, timing }` row = `Interface` and `Interaction` |
| `proto`       | `protox::compile` (already a dev-dependency) over `generate_with(ir, others)`, then the `FileDescriptorSet` | each message = `Spellings.declared` or `InducedName.wire`; each field's name = `Field.name.snake`, number = `Slot.ordinal`; each `reserved` = a `Retired` slot; each enum value = `<Spellings.screaming of the enum>_<Spellings.screaming of the value>` and its number; each `oneof` arm = `Arm.name.snake` at `Arm.ordinal`; each identity table = `<DottedName.joined_camel or declared>Ordinal` with `<screaming>` at `InteractionSlot.ordinal`; each `import` = a `foreign` package                                                                                       |
| `flatbuffers` | the existing `read_back` and `drift` reader, extended                                                       | every table, id and `= null` = `FlatBuffersProjection.tables` (`FbTable.name`, `FbSlot.id`, `FbSlot.needs_null_default`); each field's scalar keyword = `FbScalar.type`; each union member = `Arm.name.snake`; each identity table as for proto3; each `include` = a `foreign` package                                                                                                                                                                                                                                                                                         |
| `typescript`  | a line reader over `generate(ir)`, in the style the TypeScript backend's own tests use                      | each `export interface/type/enum/const` = `Spellings.declared` with `export` iff `visibility` is public; each property = `Field.name.declared` with `?` iff `Type.optional`; a `bigint` brand iff `IntWidth` is U64 or I64; `function init<Name>` present iff `Init.derivable`; each `@bounds` = `ArrayType`/`MapType` min and max; each `import * as x from './p'` = a `foreign` package with `DottedName.underscored`                                                                                                                                                        |

Each reader is test code, total over the corpus and partial over the output: it
checks every fact the model states and nothing the model does not. A fact the
backend derives differently from the lowering fails here, in the pull request
that introduces the divergence, naming the package, the declaration and the two
values. The Rust reader runs over both scopes (§2) so that the withheld-codec
note and the skipped-interface note are checked under each.

### 8.2 The P4 form: byte identity, made total for Rust

When a Rust backend layer stops reading the raw IR, the fact-level reader for
that layer has nothing left to check: the output is a function of the model by
construction. P4's measure is then the one the driver states — the existing
snapshots in `crates/ridl-backend-rust/src/snapshots/`,
`crates/ridlc/tests/snapshots/` and the checked-in
`crates/ridl-backend-rust/tests/generated/interaction_face.rs`, none of which
may move — and, inside each P4 pull request, the comparison
`generate_with(package, others)` before the port equals
`generate_with(package, others)` after it, which the snapshots pin because they
are the raw-IR output frozen. After the third layer lands, the Rust reader of
§8.1 is deleted for Rust, and the other three backends keep theirs until their
own stories.

### 8.3 How a backend reads both during the transition

Within `ridl-backend-rust`, every entry point (`generate`, `generate_with`,
`generate_face_with`, `generate_pipeline`) calls
`let model = ridl_ir::codegen::lower(package, others)` first and passes `&model`
beside `&v2::Package` into `Ctx`. A ported emitter takes the model's message for
its declaration; an unported one takes the IR's, and the two are paired by index
— the positional correspondence D-1 makes an invariant and P2b tests. So after
layer 1, `emit_decl` reads `model.declarations[i]` while `codec::package_items`
still reads `package.decls[i]` and `descriptors.rs` still walks
`package.shapes()`; after layer 3, `Ctx` holds only the model, and the `package`
parameter of the entry points is what `lower` consumes and nothing else. No
second entry point over the model is added: `generate_with` keeps its signature
throughout, which is what keeps every caller, every snapshot and the P3 host
unchanged.

## 9. Where the records and the code disagree

Each item is a fact this note had to work around; none is fixed here.

1. **ADR-0020 decision 3 and ADR-0007 decision 13 as amended retire `#[repr(C)]`
   on generated domain structs; `emit_struct` in
   `crates/ridl-backend-rust/src/lib.rs` still emits it when
   `StructDef.fixed_layout` is true.** The model carries `Struct.fixed_layout`
   so that stage P4 stays byte-identical; removing the attribute is the
   retirement's own change.
2. **ADR-0016 decision 2 pins `snake_case` in `ridl-ir`;
   `crates/ridl-backend-rust/src/face.rs` keeps a private `snake_case` that
   differs on an acronym followed by a word** (`HTTPServer` → `httpserver`
   against the pinned `http_server`; driftsys/ridl#450). The model carries the
   pinned spelling only. The one such name in the corpus, `parseHTTPResponse`,
   is in `ridl-diag-showcase`, which fails RIDL-149 and has no generated-code
   snapshot; the face fixture declares none. So P4's third layer closes #450 by
   construction with no snapshot moving.
3. **ADR-0016 pins two transforms; the wire backends apply a third**,
   `type_name` (`crates/ridl-backend-proto/src/lib.rs`,
   `crates/ridl-backend-flatbuffers/src/lib.rs`), over a service's dotted name
   and over a field name when it spells an induced tuple — and the Rust backend
   spells the same tuple with `camel_case` and nests it by field name where the
   wire backends nest by `Field<N>`. The model carries `DottedName.joined_camel`
   and both `InducedName` rules (D-2). Pinning one rule is an ADR-0016 amendment
   and a breaking change to one side's generated names, recorded here for the
   story that takes it.
4. **The driver's D-P4 asks for clauses "in the form the translator accepts";
   that form is not a property of the text alone** — it depends on the
   parameter's scalar class, which only a resolver knows. The model carries the
   translation (D-9), and the translator's parser moves into the lowering at
   P4's third layer.
5. **`ridl_rt::contract::PayloadInfo.max_size` is `None` in every generated
   descriptor** (`descriptors.rs`, `payload_info`) while the bound exists and is
   emitted as `MAX_SIZE` a few items lower; the interaction-face record says
   "the toolchain cannot size the payload yet". The model carries the bound on
   every `Payload`; P4 keeps `None` for byte identity, and E16.2 (or the story
   that reconciles `ridl-rt`'s two readings of `None`) fills it. The 2026-09-13
   runtime-descriptors note's D-6 says "nothing in the crates computes an
   encoded size today", which stage K2 made false.
6. **The Rust and TypeScript backends resolve nothing across packages for inits,
   derives and constants, while the codec and both wire backends do (ADR-0017
   decision 1).** The model resolves over the scope; three printer rules stay
   conservative in P4 (§3.5). Lifting them is a change to what the generated
   crate contains, not a port.
7. **`check_flatbuffers_bound` attributes a missing bound over the package alone
   (`others: &[]`) even when the pipeline handed the codec a scope.** The model
   reproduces that (§3.7). With the scope, a cross-package member could be
   judged and named; that is a change to the attribution messages, left to the
   story that wants it.
8. **The preceding note's D-3 asks whether the model nests a type at the IR's
   cost.** It nests arrays and maps at the IR's cost and flattens tuples: the
   bound is 261 message levels and 264 JSON levels, from the map shape (§6.2).
9. **ADR-0020 open item 5 asks whether the lowered model is a public artifact.**
   This note answers yes (`--emit codegen-model`, D-12); the item is closed by
   whichever of P2b or P3 amends the record.
10. **The corpus harness and the CLI generate over different scopes** (§2). Not
    a contradiction in a record, but a fact no record states: P4's byte identity
    is measured against snapshots taken with no scope and a demo run with one.
11. **The TypeScript backend's module header names an `interact` module** that
    no longer exists — ADR-0018 retracted the interaction layer the language
    backends shipped. A stale comment; the backend emits the typl surface only,
    which the model's drift reader checks.
12. **The runtime-descriptors note (2026-09-13) plans a second lowering of the
    interface facts** for the catalog descriptor. The facts are §3.6's; the
    descriptor's lowering should read the model rather than the IR when that
    plan starts, so that the two artifacts cannot disagree. A consequence for
    that plan, not a decision here.

## 10. What P2b implements from this note

- `crates/ridl-ir/proto/ridl/codegen/v1/model.proto` with §3's messages and
  §3.8's enums, field numbers as listed; `build.rs` compiles it beside the two
  IR files and registers `.ridl.codegen.v1` with `pbjson_build`.
- `ridl_ir::codegen::v1` (the generated types and serde impls),
  `ridl_ir::codegen::lower(&v2::Package, &[&v2::Package]) -> v1::Model`,
  `to_json_pretty`, `from_json` (same cap and stack as `v2::from_json`),
  `to_binary`, `from_binary`, `to_text_format`.
- `Emit::CodegenModel` in `crates/ridlc/src/lib.rs`, flag value `codegen-model`,
  suffix `.codegen.json`, classified with the code emits (§6.1);
  `docs/book/cli-reference.md` gains the value.
- Tests in `crates/ridl-ir`: positional correspondence over every corpus
  package; determinism (`lower` twice, byte-identical JSON); the nesting bound
  of §6.2 over the three generated shapes at source depth 127 (261 and 264
  asserted); the D-P4 floor over the face fixture
  (`crates/ridl-backend-rust/tests/fixtures/interaction_face.ridl`): number,
  ordinals, kinds, payload types, `flatbuffers_max_size` equal to the codec's
  `MAX_SIZE`, timing, the two clauses translated, the zero hash.
- Corpus snapshots of the model: `corpus__codegen@<entry>.snap` beside the IR
  snapshots in `crates/ridlc/tests/snapshots/`, over the harness's scope.
- The four fact-level drift tests of §8.1, one per backend crate, over the
  corpus fixtures each crate already compiles.
- The `docs/wip/README.md` entry moves this note to "implemented by P2b";
  ADR-0020 open item 5 is closed by P2b or P3 with a dated note.

## 11. Records touched by the recommendation, for the reviewer

| Record                                         | Effect                                                                                                                                                          |
| ---------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| ADR-0020 decisions 8, 9                        | unchanged; the model is decision 8's, and its encoding is decision 9's canonical encoding (JSON, per the preceding note)                                        |
| ADR-0020 open item 5                           | answered: the model is a public artifact, `--emit codegen-model`                                                                                                |
| ADR-0016 decisions 1, 2                        | unchanged; the model carries their outputs. Its consequences list gains nothing here; §9 items 2 and 3 name what a later amendment would take                   |
| ADR-0017 decision 1                            | unchanged; `lower(package, others)` takes the same `others` `generate_with` takes                                                                               |
| ADR-0019, all decisions                        | unchanged; §3.7 carries them                                                                                                                                    |
| ADR-0014 decision 10                           | the `Emit` classification gains one member (P2b)                                                                                                                |
| ADR-0010                                       | the `--emit` value `codegen-model`, consistent between `ridl build` and `ridlc build` (P2b)                                                                     |
| The IR stability note, D-1, D-3, D-4, D-5, D-6 | inherited; D-3 restated for this schema in §6.2                                                                                                                 |
| The lane P driver, D-P2, D-P3, D-P4, O-P2      | D-P4 discharged in §3.6; O-P2 recommended in §5; P3's `CodegenRequest.model` is `ridl.codegen.v1.Model`; P4's three layers are §3.3–§3.5, §3.7 and §3.6 in turn |
| `docs/design/flatbuffers-codec.md`             | unchanged; §3.7 keeps the inline layout in the codec as that record states                                                                                      |
| `docs/design/interaction-face.md`              | unchanged; §9 item 5 names the `None` it records                                                                                                                |
| The frame specification §11.2                  | unchanged; its list of what the Kotlin backend needs is §3.6's table                                                                                            |
