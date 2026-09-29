# Generated-name collisions in the Rust backend: design note

**Status:** a design note for the maintainer's approval. It settles where each
generated-name collision is removed and how each fix is proven. It implements
nothing. Every "this collides" and "this compiles" statement below is an
experiment in the appendix, run with `ridl build --emit rust` from this branch
and `cargo check` of the emitted crate at the toolchain pin, 1.98.1.

**Date:** 2026-09-29.

**Trace:** driftsys/ridl#583, #587, #588, #423, #449, #453, #455 (the class);
driftsys/ridl#416 and #424 (related, in scope, §8);
[ADR-0016](../decisions/ADR-0016-schema-projection-and-the-name-transform.md)
decisions 3 to 5 and its consequences (the checked namespaces);
[ADR-0017](../decisions/ADR-0017-proto3-projection-rules.md) decision 4 (name
totality is a backend obligation);
[ADR-0023](../decisions/ADR-0023-interaction-face-generation.md) decision 7 and
the design note it came from,
[`docs/archive/2026-09-28-face-fixed-methods-traits-design.md`](../archive/2026-09-28-face-fixed-methods-traits-design.md)
(driftsys/ridl#580, the template of this note);
[ADR-0013](../decisions/ADR-0013-codegen-backend-scope.md) and
[ADR-0020](../decisions/ADR-0020-third-encoding-runtime-layering-and-plugin-system.md)
decision 8 (one codegen model for every backend);
[the interaction-face design record](../design/interaction-face.md), E11.14
decision 5 (the `Wire` refusal).

## 1. The problem

`ridl check` accepts a source, and the Rust backend then emits two items of one
name in one Rust namespace, or an item whose name hides a name the backend
writes unqualified. rustc then fails in the consumer's build. The seven issues
are samples of one class, and the inventory in §2 finds thirteen more cases.

The experiments correct four statements in the issues:

- driftsys/ridl#583 names rustc E0201 for `enumset W { self = 0  self_ = 1 }`.
  The error is E0592 (X-1a). The same escape also loses a whole package without
  any error: packages `p.self` and `p.self_` both map to the module `self_`, and
  the crate links only one of them (X-1f).
- driftsys/ridl#587 names a struct field `bytes`. `bytes` is a typl keyword and
  cannot be a field name (FORM-102). A field named `Bytes` or `BYTES` reaches
  the accessor `bytes` through `snake_case` and gives E0592 (X-2a).
- driftsys/ridl#588 expects E0428 from a type named `Wire`. The backend has
  refused that package at `ridl build` since driftsys/ridl#476 (E11.14 decision
  5), so the consumer sees a build error, not a rustc error (X-3).
- driftsys/ridl#455 says `ridl build` does not reach the face. Since E11.14 it
  does, and members `XY` and `x_y` give E0428 from `ridl build --emit rust`
  (X-8).

## 2. Namespace inventory

Read from `crates/ridl-backend-rust/src/lib.rs` (`ident`, `emit_*`,
`wire_alias`, `refuse_wire_collision`, `tuple_collision`), `src/codec.rs` (the
views and the `__ridl_fb_*` functions), `src/descriptors.rs`, `src/defaults.rs`,
`src/face.rs` and `src/face/*.rs`, and `crates/ridlc/src/lib.rs`
(`render_lib_rs`, the crate module tree). The transforms `snake_case`,
`camel_case` and `pascal_case` are in `crates/ridl-ir/src/name.rs`; the codegen
model carries all of them for every name (`Spellings` in
`crates/ridl-ir/proto/ridl/codegen/v1/model.proto`), and the induced tuple name
as `InducedName.rust`. `ident()` passes a name through, writes a Rust keyword as
a raw identifier (`r#type`), and appends `_` to the four keywords that cannot be
raw (`crate`, `self`, `Self`, `super`).

Two facts decide many rows. A ridl identifier starts with a letter
(`[A-Za-z][A-Za-z0-9_]*`), so a name that starts with `_` is out of reach of
every ridl name. And a Rust trait has its own namespace, so a trait item
collides with nothing a ridl name derives.

| Rust namespace                                                   | ridl names that reach it                                                                                                                                                    | Transform                                                                                                       | Fixed names the backend puts there, or writes unqualified in its scope                                                                                | Issues and experiments                                                                                            | Checked today?                                                                                                                                            |
| ---------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Package module, types                                            | declaration; interface; interface and member (descriptor); declaration (view); field path (induced tuple); interface (face module)                                          | verbatim; verbatim; `<Iface>` + `camel_case(member)`; `<Decl>FbView`; `camel_case` of the path; `snake_case`    | `Wire`; written unqualified: `Default`, `String`, `Vec`, `Option`, and the primitives `bool`, `f32`, `f64`, `i64`, `str`, `u8`, `u16`, `u64`, `usize` | #588 (X-3), #423 (X-4), #453 (X-7b), #455 (X-8), #583 (X-1c); new: X-5, X-9, X-10, X-11, X-12, X-13, X-14a, X-14b | TYPL-009 refuses a declaration and an interface of one name (X-19); the lowering refuses two tuples of one name; the backend refuses `Wire`; nothing else |
| Package module, values                                           | constant; newtype and enum set (tuple-struct constructor); interface and descriptor (unit-struct constructor); declaration (codec functions); interface (skipped-face note) | verbatim; verbatim; as above; `__ridl_fb_{encode,verify,decode}_` + `snake_case`; `__RIDL_NO_FACE_` + screaming | the `__` names on the left; written unqualified: `Some`, `None`                                                                                       | #423 (X-4); new: X-14c, X-15, X-17                                                                                | no                                                                                                                                                        |
| Crate module tree (`lib.rs`)                                     | package segment                                                                                                                                                             | verbatim through `module_segment`, which is `ident()`                                                           | `__ridl_package`                                                                                                                                      | #416 (X-18), #583 (X-1f)                                                                                          | no                                                                                                                                                        |
| One struct's fields, and its view's accessors                    | field                                                                                                                                                                       | `snake_case`                                                                                                    | view: `bytes`                                                                                                                                         | #583 (X-1b), #587 (X-2a)                                                                                          | RIDL-149 (`snake_case`), TYPL-215 (exact repeat)                                                                                                          |
| One induced tuple's fields, and its view's accessors             | tuple field                                                                                                                                                                 | `snake_case`                                                                                                    | view: `bytes`                                                                                                                                         | #449 (X-6), #583 (X-1e), #587 (X-2b)                                                                              | no                                                                                                                                                        |
| One enum's variants                                              | enum value                                                                                                                                                                  | `pascal_case`                                                                                                   | none                                                                                                                                                  | none                                                                                                              | RIDL-149 (`pascal_case`), TYPL-216                                                                                                                        |
| One union's variants                                             | arm                                                                                                                                                                         | `camel_case`                                                                                                    | none                                                                                                                                                  | none                                                                                                              | RIDL-149 (`snake_case` and `camel_case`), TYPL-217                                                                                                        |
| One enum set's inherent constants                                | bit                                                                                                                                                                         | verbatim                                                                                                        | none since driftsys/ridl#562; the path call `T::default()` resolves here first                                                                        | #583 (X-1a); new: X-16                                                                                            | TYPL-218 (exact repeat)                                                                                                                                   |
| A newtype's inherent items                                       | none                                                                                                                                                                        | none                                                                                                            | `new`, `check`, `new_unchecked`, `get`, `into_inner`                                                                                                  | none                                                                                                              | not needed                                                                                                                                                |
| `<Iface>`'s inherent items                                       | none                                                                                                                                                                        | none                                                                                                            | `MAX_BUFFER_SIZE`, `EVENT_SOURCE_BUFFER_SIZE`                                                                                                         | none                                                                                                              | not needed                                                                                                                                                |
| A scalar, enum or union view's inherent items                    | none                                                                                                                                                                        | none                                                                                                            | `bytes`, `value`                                                                                                                                      | none                                                                                                              | not needed                                                                                                                                                |
| Face module `<iface>`, types                                     | member                                                                                                                                                                      | `camel_case` + `Call`, `Phase`, `Correlation`                                                                   | `Client`, `Publisher`, `Provider`, `Event`, `NextEvent`, `Serve`, `ServeState`, `Subscribe`, `Invalidate`, `prelude`, `blocking`                      | #455 (X-8)                                                                                                        | no                                                                                                                                                        |
| Face module `<iface>`, values                                    | member                                                                                                                                                                      | `send_`, `poll_…_ack`, `poll_…_reply` around `snake_case`                                                       | `serve`, `dispatch`, `poll_next_event`                                                                                                                | none                                                                                                              | RIDL-149 (`snake_case`)                                                                                                                                   |
| `<iface>::Event`'s variants                                      | event member                                                                                                                                                                | `camel_case`                                                                                                    | none                                                                                                                                                  | #455                                                                                                              | no                                                                                                                                                        |
| `Client`, `blocking::Client`, `Publisher` inherent methods       | member                                                                                                                                                                      | `snake_case`                                                                                                    | none since driftsys/ridl#580 moved the fixed ones to traits                                                                                           | #583 (X-1d)                                                                                                       | RIDL-149 (`snake_case`)                                                                                                                                   |
| `Provider`, `Subscribe`, `Invalidate` methods; method parameters | member; parameter                                                                                                                                                           | `snake_case`, with `subscribe_` or `invalidate_`                                                                | the `__arg`-style locals of driftsys/ridl#581                                                                                                         | none                                                                                                              | RIDL-149, RIDL-413                                                                                                                                        |
| Trait items, derive macros                                       | none                                                                                                                                                                        | none                                                                                                            | `Payload`, `TryFrom`, `From`, `Default` items; `Debug`, `Clone`, `Copy`, `PartialEq`, `Eq`, `Hash`, `PartialOrd`, `Ord`                               | none: a declaration named like any of them compiles (X-4)                                                         | not needed                                                                                                                                                |

Headline. Twenty-one cases reproduce: the seven issues, driftsys/ridl#416, and
thirteen found by the inventory (X-5, X-9, X-10, X-11, X-12, X-13, X-14a to c,
X-15, X-16, X-17, and the silent package loss of X-1f). Three rows that look
exposed are not: a declaration named `Result`, `Ok`, `Err`, `From`, `TryFrom`,
`Box`, `Into`, `Sized`, `Send`, `Sync` or a derive name compiles (X-4); an
interface named `Bool`, `I64` or `Usize`, whose face module is `bool`, `i64` or
`usize`, compiles, because a module does not hide a primitive type (X-5b); and
the face module has no glob import of its parent, so a declaration named `Some`
or `Option` does not reach the code inside it (X-4d).

## 3. Options

1. **The checker refuses the colliding names.** New diagnostics in `ridl-sem`. A
   name that collides with a Rust fixed name, a Rust keyword escape or a Rust
   prelude name becomes a reserved word of the language.
2. **The backend avoids the collision itself.** The backend changes the name it
   chose: it qualifies a name it writes (`::core::…`, as driftsys/ridl#420 did
   for five names), moves a fixed method behind a trait (as driftsys/ridl#580
   did), renames a fixed item to a name no ridl identifier can spell, makes its
   keyword escape injective, or spells an internal name from the declared name
   instead of from a lossy transform. Where two names are both derived from ridl
   names and the Rust namespace cannot hold both, no rename is available, and
   the backend refuses the package with a message that names both sources.
3. **Move each derived shape into a namespace of its own.** Descriptors, views
   and induced tuples leave the package module for per-shape modules, so that
   concatenated names cannot meet declared ones.
4. **A mix, with a stated rule for which namespace gets which.**

## 4. Recommendation

Option 4, with the rule that ADR-0016 decision 3 and ADR-0017 decision 4 already
state between them, applied to the Rust backend, which has not yet applied the
second.

### 4.1 The rule

1. **A collision is the language's only when every backend that projects the
   name meets it.** Two names of one ridl scope that collide under a pinned
   transform every projecting backend applies in that scope are a property of
   the package: RIDL-149, in `ridl-sem` (ADR-0016 decisions 3 and 5). A name
   spelled twice in one scope is met by every backend that projects that name:
   TYPL-215 and its siblings. This note adds one language rule of that kind
   (TYPL-215 over a tuple's fields, §4.2) and no RIDL-149 scope.
2. **Every other collision belongs to one target's namespaces, and that target's
   backend owns it** (ADR-0017 decision 4: a backend models its target's name
   scopes and refuses a collision rather than emitting code the target rejects).
   The Rust backend applies it in two steps.
   1. **A name the backend chose never refuses a package.** When one of the two
      names is fixed by the backend, written unqualified by it, produced by its
      keyword escape, or derived by it through a lossy transform for its own
      internal use, the backend changes that name so that no ridl name reaches
      it. This is the rule of driftsys/ridl#580, stated for every namespace.
   2. **Two ridl-derived names that one Rust namespace cannot hold are refused
      at `ridl build`,** with one message that names both sources, from one
      claim table per Rust namespace. It generalizes the backend's
      `tuple_collision` and the claim tables the proto and FlatBuffers backends
      already keep (X-11). Each case it refuses either fails rustc today or is
      already refused at `ridl build`, so it rejects nothing that builds.

The constraint that an asymmetry between backends justifies a backend strategy
and not a language rule decides the line in step 1. Three of the issues proposed
RIDL-149 extensions (driftsys/ridl#449 for a tuple's fields, driftsys/ridl#453
for a struct field under `camel_case`, driftsys/ridl#455 for a member under
`camel_case`). Of the in-tree backends, each transform is applied in that scope
by the Rust backend alone: the proto and FlatBuffers backends name a tuple's
fields by position (`field_1`, `field_2`, X-6a) and name an induced tuple from
`InducedName.wire`, and the TypeScript backend keeps every name as declared and
compiles `(minSpeed : Speed, min_speed : Speed)` (X-6a). So the three go to the
Rust backend's claim table. An exact repeat, `(a : Speed, a : Speed)`, fails in
Rust (E0124) and in TypeScript (TS2300, X-6b), so it is the language's.

The rule leaves one inconsistency in place, and this note states it rather than
changing it. RIDL-149 checks a union's arms under `camel_case` (2026-09-20
amendment) and an enum's values under `pascal_case` (2026-09-26 amendment), and
in both scopes only the Rust backend applies that transform. Under the rule
those two would be the Rust backend's. Moving them would loosen the language and
is not needed to close any issue here, so the note leaves them and proposes that
the ADR-0016 amendment record the question as open (§4.4; §10, decision 2).

### 4.2 Per namespace

| Namespace                                            | Collision                                                                                                                                                                                                      | Home                   | Mechanism                                                                                                                                                                                                                                    | Experiments                  |
| ---------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------- |
| One tuple's fields                                   | the same name twice                                                                                                                                                                                            | language               | TYPL-215 also covers a tuple's fields (a tuple is an anonymous struct, typl §11)                                                                                                                                                             | X-6b                         |
| Package module, types and values                     | a declaration named like a prelude name the backend writes (`Default`, `String`, `Vec`, `Option`, `Some`, `None`)                                                                                              | backend, rename        | write `::core::default::Default`, `::std::string::String`, `::std::vec::Vec`, `::core::option::Option` and its `Some` and `None` in package scope, as driftsys/ridl#420 did for five names                                                   | X-4, X-4c                    |
| Package module, types                                | a declaration named like a primitive the backend writes (`bool`, `f32`, `f64`, `i64`, `str`, `u8`, `u16`, `u64`, `usize`)                                                                                      | backend, rename        | write `::core::primitive::<name>` in package scope; a `#[repr(i64)]` keeps the bare name                                                                                                                                                     | X-5, X-5c                    |
| Enum set constants                                   | a bit named `default` captures `T::default()`                                                                                                                                                                  | backend, rename        | write `<T as ::core::default::Default>::default()` (`defaults.rs`, `descriptors.rs`)                                                                                                                                                         | X-16                         |
| Struct and tuple views                               | a field whose `snake_case` is `bytes` against the fixed `bytes`                                                                                                                                                | backend, trait         | `bytes` moves from every view's inherent impl to a trait in `ridl-rt`, `ridl_rt::payload::View<'a>` with `fn bytes(&self) -> &'a [u8]`; the accessor stays inherent and wins the dot call, the trait path reaches the bytes                  | X-2                          |
| Package module, types                                | a declaration or an interface named `Wire`                                                                                                                                                                     | backend, rename        | the alias becomes `pub type __Wire`; `refuse_wire_collision` and E11.14 decision 5 are retired                                                                                                                                               | X-3                          |
| Every namespace `ident()` writes, and the crate tree | a name `k` and a name `k_`, where `k` is `crate`, `self`, `Self` or `super`                                                                                                                                    | backend, rename        | the escape becomes injective: a name that is one of the four followed by zero or more `_` gets one more `_`, so `self` is `self_` as today and `self_` is `self__`                                                                           | X-1a, X-1b, X-1c, X-1e, X-1f |
| Package module, values                               | two declarations or two skipped interfaces whose `snake_case` agrees (`HTTPServer`, `HttpServer`)                                                                                                              | backend, rename        | spell `__ridl_fb_{encode,verify,decode}_*`, `__RIDL_FB_NO_CODEC_*` and `__RIDL_NO_FACE_*` from the declared name, with `#[allow(non_snake_case)]` or `#[allow(non_upper_case_globals)]` on the item                                          | X-15, X-14c                  |
| Crate module tree                                    | a type `veh.common` hidden by the child package module `veh::common`                                                                                                                                           | backend, rename (path) | a reference to such a type is written through the package's own module, `crate::veh::__ridl_package::common`, and `__ridl_package` becomes `#[doc(hidden)] pub`; `ridlc` knows the child packages, and the model's `Scope.others` lists them | X-18                         |
| Package module, types                                | descriptor against declaration, view against declaration, tuple against declaration, descriptor against interface, two descriptors across interfaces, face module against declaration or interface, two tuples | backend, claim         | one claim table over the package module's type namespace; the second claim is refused with both sources named                                                                                                                                | X-7, X-9 to X-14b            |
| Package module, values                               | a constant against a unit or tuple-struct constructor                                                                                                                                                          | backend, claim         | the same table over the value namespace                                                                                                                                                                                                      | X-17                         |
| One tuple's fields                                   | two field names whose `snake_case` agrees                                                                                                                                                                      | backend, claim         | a claim table per induced tuple                                                                                                                                                                                                              | X-6a, X-1e                   |
| Face module types, `Event` variants                  | two members whose `camel_case` agrees                                                                                                                                                                          | backend, claim         | the package-scope descriptors `<Iface><Member>` already carry the same collision, so the package table refuses it first; the face module and `Event` tables are kept for totality                                                            | X-8, X-1d                    |

The claim table runs before the codec. Today a tuple named like a declaration
reaches the FlatBuffers projection first and is refused with "the FlatBuffers
projection describes no table for `ReadingBounds`", which names neither source
(X-11). Its message follows `tuple_collision` and the proto backend's `claim`:
the generated name, then each source (declaration, interface, member, field
path, suffix), then "rename one of them".

### 4.3 What does not change

RIDL-149's scopes. The face's "nothing is refused and nothing is renamed" rule
for a member against a fixed method (ADR-0023 decision 7) holds and is the first
half of step 2. The per-interface skip of E11.14 decision 2 is not used for a
name collision (§10, decision 4).

### 4.4 Records to amend

The implementer applies these; this stage touches neither `docs/decisions/` nor
`docs/design/`.

**ADR-0016, a dated amendment in `## Status`, proposed wording:**

> **Amendment (2026-09-29), from the generated-name collision design
> (driftsys/ridl#583, #587, #588, #423, #449, #453, #455).** The line between
> decision 3 and ADR-0017 decision 4 is stated for every backend. A collision is
> the language's, and `ridl-sem` refuses it, only when every backend that
> projects the names meets it: two names of one scope under a pinned transform
> every projecting backend applies in that scope (RIDL-149), or one name spelled
> twice in one scope (TYPL-215 and its siblings). Every other collision belongs
> to one target's namespaces and its backend removes it: by changing a name the
> backend chose, when one of the two names is the backend's own, or, when both
> are derived from ridl names and the target cannot hold both, by refusing the
> package with a message naming both sources.
>
> Applied here: TYPL-215 covers a tuple's fields, because a repeated tuple field
> fails in Rust and in TypeScript. A tuple field's `snake_case`, a struct
> field's and a member's `camel_case`, and the interface and declaration names
> of one package are not RIDL-149 scopes: each is applied by the Rust backend
> alone (the wire backends name a tuple's fields by position), so the Rust
> backend refuses those collisions at `ridl build`. The consequences entries on
> driftsys/ridl#449, #453 and #455 are resolved by this, not by extending
> RIDL-149.
>
> Open: RIDL-149 checks a union's arms under `camel_case` and an enum's values
> under `pascal_case`, and only the Rust backend applies either transform. By
> this amendment's line those two checks are the Rust backend's. They stay where
> they are until a change needs to move them.

**The interaction-face design record,** the E11.14 decisions list: decision 5
(the `Wire` refusal) is superseded; the alias is `__Wire`; the paragraph "The
alias is an unprefixed item at package scope, and one name can collide" is
rewritten; the closing paragraph that lists driftsys/ridl#587 and #588 as open
is updated.

**ADR-0023, a dated consequence note:** the face's `camel_case` names of a
member are refused by the claim table when two members collide
(driftsys/ridl#455), and decision 7's rule for fixed names is unchanged.

**ADR-0021, a new decision:** `ridl_rt::payload::View<'a>`, ungated and
`no_std`, and the release that carries it.

**`docs/design/flatbuffers-codec.md`:** the view's `bytes` is a trait method;
the `__ridl_fb_*` functions are spelled from the declared name.

**typl reference §16 and the TYPL-215 row:** "field name declared twice in one
struct or one tuple".

**`crates/ridl-backend-rust/src/lib.rs` doc comments** that cite the retired
decision (`refuse_wire_collision`, the `ident` docstring's escape rule, the
`emit_field` and `emit_tuple_struct` notes that cite driftsys/ridl#449 and #453
as open).

## 5. Breaking or not, and the Kotlin mirror

**The generated API breaks.** Three changes rename or move a public item:

- `bytes` leaves every view's inherent impl. A consumer that calls
  `view.bytes()` adds `use ridl_rt::payload::View;`. With a field whose accessor
  is `bytes`, `view.bytes()` is the field and `View::bytes(&view)` is the buffer
  (X-2a).
- `Wire` becomes `__Wire`. No consumer in the tree names it
  (`examples/cabin/consumer`, the tests outside `generated/`, the book).
- A source name `self_`, `Self_`, `super_` or `crate_` (and a package segment of
  that form) now emits one more `_`. Only a package that declared both forms
  failed before; a package that declared `self_` alone changes spelling.

The rest adds nothing a consumer can name: absolute paths name the same items,
the `__ridl_fb_*` and `__RIDL_*` items are `pub(crate)` or private, and
`__ridl_package` becoming `#[doc(hidden)] pub` is an addition. Every claim-table
refusal and the TYPL-215 extension reject only sources that fail rustc today,
and the TYPL-215 case also fails TypeScript (X-6b).

The backend commit is `feat(ridl-backend-rust)!:`. At 0.x a breaking commit is a
minor bump, so the next release is **0.5.0**, as 0.4.0 followed
driftsys/ridl#582. `ridl-rt` gains `payload::View`, an addition under ADR-0021
decision 10; it is released as 0.5.0 with the workspace, the manifest literal in
`crates/ridlc/src/lib.rs` moves to `"0.5"` in the release commit (its guard in
`crates/ridlc/tests/rust_crate_emit.rs` follows the workspace version), and
`ridl-rt` 0.5.0 is published at the tag, the order 0.4.0 used.

**The Kotlin mirror** (driftsys/ridlc-gen-kotlin) gets one heads-up issue, as #9
and #10 were written: one issue per ridl change. It carries:

- the rule of §4.1, and that the only language change is TYPL-215 over a tuple's
  fields, which Kotlin receives from `ridl check` with no work;
- that every `camel_case` collision of §4.2 (a member, a struct field, a tuple
  field, cross-interface concatenation, a descriptor or tuple named like a
  declaration) is a backend obligation, so the Kotlin backend models its own
  namespaces (package-level classes, nested classes, members, and JVM signature
  clashes) and refuses a collision with both sources named, rather than
  expecting `ridl check` to;
- the rule that a name the backend chose never refuses a package: each fixed
  name the Kotlin backend emits into a namespace a ridl name reaches is
  qualified, moved to an interface, or renamed out of reach, decided there;
- the keyword note: Kotlin escapes a keyword with backticks, which is injective,
  so the `k` against `k_` class of driftsys/ridl#583 has no Kotlin counterpart
  unless that backend uses a suffix escape;
- the appendix ids that concern names Kotlin also derives (X-6a, X-7, X-8, X-9,
  X-12, X-13, X-14a, X-14b, X-17) as the cases its own tests cover.

`payload::View`, `__Wire` and the `__ridl_fb_*` spelling are Rust codec details
and are named in the issue for information only.

## 6. Proof

Each experiment in the appendix becomes a test. Three homes, following the
existing harnesses.

**`crates/ridl-backend-rust/tests/name_collision_compile.rs`, new,** the shape
of `face_compile.rs`: `ridlc::compile` an inline source with no error, emit it
with `generate_pipeline(&output.package, WireEncoding::FlatBuffers, &[])` (the
entry point `ridl build --emit rust` calls), and check it with bare `rustc`
(`--edition 2024 --crate-type lib --emit=metadata -D warnings`) against the
`ridl-rt` rlib of `tests/support/rustc.rs`, once with the `std` cfg and once
without. Sources whose names are off-convention on purpose (a lowercase type, a
lowercase bit, `Self`) add `-A non_camel_case_types -A non_upper_case_globals`:
a naming lint is not a collision, and the lint is emitted for those names today
(X-5, X-1c). One test per namespace:

| Test                                             | Source                                                                                                                                               | Asserts                                                                                          |
| ------------------------------------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------ |
| `declarations_named_like_prelude_names_compile`  | X-4c: `Default`, `String`, `Vec`, `Option`, `Some`, `None`, `Result`, `Ok`, `Err`, `From`, `TryFrom` beside an interface with `require` and `ensure` | compiles; this is driftsys/ridl#424's recipe, over the pipeline entry point                      |
| `declarations_named_like_primitives_compile`     | X-5c: the nine primitive names                                                                                                                       | compiles                                                                                         |
| `an_enum_set_bit_named_default_compiles`         | X-16                                                                                                                                                 | compiles                                                                                         |
| `a_field_whose_accessor_is_bytes_compiles`       | X-2a and X-2b, with a consumer module                                                                                                                | compiles; the consumer's `view.bytes()` has the field's type and `View::bytes(&view)` is `&[u8]` |
| `a_declaration_or_interface_named_wire_compiles` | X-3a and X-3b                                                                                                                                        | compiles                                                                                         |
| `a_keyword_and_its_escape_compile`               | X-1a, X-1b, X-1c, X-1e in one package                                                                                                                | compiles                                                                                         |
| `names_equal_under_snake_case_compile`           | X-15 and X-14c                                                                                                                                       | compiles                                                                                         |

**`crates/ridl-backend-rust/src/tests.rs` or a sibling integration test,** the
claim table: for each of X-1d, X-6a, X-7a, X-7b, X-8, X-9, X-10, X-11, X-12,
X-13, X-14a, X-14b and X-17, `ridlc::compile` reports no error and
`generate_pipeline` returns a `GenerateError` whose message names the generated
name and both sources. X-11's test also asserts the message does not mention the
FlatBuffers projection.

**`crates/ridlc/tests/rust_crate_emit.rs`,** the crate module tree, since the
tree is `ridlc`'s: X-18 (a type named like its child package, referenced from
the child) and X-1f (packages `p.self` and `p.self_`) are emitted with the CLI
path and compiled, with a consumer that names both packages' types. The two
existing `Wire` refusal tests (near line 1210 and 1252) invert: the source now
compiles.

**`ridl-sem`:** a TYPL-215 test for X-6b, beside the struct-field one.
`crates/ridl-sem/src/check.rs` is owned by a peer session at the time of
writing, so the implementer coordinates that edit.

**`ridl-rt`:** a doc test on `payload::View`.

`just compat-check` covers the `ridl-rt` addition at 1.83 in both editions with
no recipe change. `just demo` covers `examples/cabin` after the rename of `Wire`
and the move of `bytes`, neither of which the consumer names.

## 7. Alternatives considered

| Alternative                                                                                                                                                                                         | Verdict  | Reason                                                                                                                                                                                                                                                                                                                                                                     |
| --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Extend RIDL-149 to a tuple's fields (`snake_case`, `camel_case`), a struct field's and a member's `camel_case`, and the declarations of one package (`snake_case`), as #449, #453 and #455 proposed | rejected | Each transform is applied in that scope by the Rust backend alone (X-6a), so the check would put one backend's namespaces into the language, which the constraint forbids. It would also refuse `HTTPServer` beside `HttpServer`, which every other backend accepts.                                                                                                       |
| A reserved-word list in `ridl-sem` (Rust keywords, `Wire`, prelude and primitive names, `bytes`)                                                                                                    | rejected | The same objection, and the list grows with every fixed name the backend adds, which driftsys/ridl#580 already rejected.                                                                                                                                                                                                                                                   |
| Keep E11.14 decision 5's refusal of `Wire` and close driftsys/ridl#588 as intended                                                                                                                  | rejected | The backend refuses a package over a name it chose, which the #580 rule excludes, and `__Wire` costs one rename in generated code no consumer names.                                                                                                                                                                                                                       |
| Move `Wire` into each face module                                                                                                                                                                   | rejected | The descriptors at package scope also name it (`<T as Payload<Wire>>::MAX_SIZE`), and an interface whose members are all `fixed` has no face module.                                                                                                                                                                                                                       |
| Delete the views' `bytes` instead of moving it                                                                                                                                                      | rejected | It duplicates `Ref::bytes`, but a function that holds only a nested view would lose the buffer. The trait keeps it at the cost of one `use` line.                                                                                                                                                                                                                          |
| A leading-underscore escape (`_self`, `_Self`)                                                                                                                                                      | rejected | It is out of reach of every ridl name, but it renames every existing keyword-named item, where the injective suffix renames only `k_` names, which are rarer.                                                                                                                                                                                                              |
| Refuse a declaration named like a primitive                                                                                                                                                         | rejected | The backend chose to write the primitive unqualified; `::core::primitive` has been stable since Rust 1.43 and costs only length.                                                                                                                                                                                                                                           |
| Refuse `HTTPServer` beside `HttpServer` in the claim table                                                                                                                                          | rejected | The only colliding names are internal (`__ridl_fb_*`, `__RIDL_*`) and derived through a lossy transform the backend chose; the declared name is unique by TYPL-009.                                                                                                                                                                                                        |
| Skip the interface's face and descriptors on a member collision, as E11.14 decision 2 does for a call the face cannot carry                                                                         | rejected | A skipped face builds and is found missing at the consumer's call site. Decision 2 covers gaps that named stories remove; a name collision is permanent until the source changes.                                                                                                                                                                                          |
| Move descriptors, views and induced tuples into per-shape modules (option 3)                                                                                                                        | rejected | It removes the concatenation collisions by construction, but it renames every descriptor, view and tuple path a consumer or the face names, which is larger than the class it removes. The most likely case, `type CabinTemperature` beside `interface Cabin { signal temperature }` (X-9), is refused with a message; if it proves common, this option can be taken then. |
| A new TYPL code for a repeated tuple field                                                                                                                                                          | rejected | The rule and its remedy are TYPL-215's; a tuple is an anonymous struct (typl §11).                                                                                                                                                                                                                                                                                         |
| Leave driftsys/ridl#416 as a naming rule in the language reference, or refuse the combination                                                                                                       | rejected | A path through `__ridl_package` compiles (X-18), so the backend removes the collision without refusing.                                                                                                                                                                                                                                                                    |

## 8. Scope decisions on #416 and #424

**driftsys/ridl#416 is in scope.** It is the same class: a name the crate layout
derives from a ridl name (the child package module `common`) hides a declared
name (the type `common` of package `veh`), and rustc reports E0573 in the
consumer's build (X-18). The fix is step 2.1 of the rule, with a path rather
than a rename: the reference is written as `crate::veh::__ridl_package::common`,
and the module becomes `#[doc(hidden)] pub` so that a consumer can name the type
the same way. The type stays unreachable as `veh::common`, which names the
module; that is a property of Rust's single type namespace and is documented at
the reference.

**driftsys/ridl#424 is in scope.** It is the test gap under the class: no rustc
proof compiled face output against a package declaring prelude names. The first
test of §6 is its recipe (the five names of driftsys/ridl#420 plus the six of
driftsys/ridl#423, and an interface with `require` and `ensure` so the clause
translator runs), over `generate_pipeline`, which emits the face as
`generate_face` does and is the CLI's path. X-4c shows the source fails today
(E0404) and compiles with the fix.

## 9. Issues

The issues this design would close are driftsys/ridl#583, #587, #588, #423,
driftsys/ridl#449, #453, #455, #416 and #424. None of the nine is left open.

Two observations are outside the class and are not filed by this note: a
lowercase type name or a lowercase enum set bit draws `non_camel_case_types` or
`non_upper_case_globals` in the consumer's build, and the escape of `Self` gives
the view `Self_FbView`, which draws `non_camel_case_types`. They are naming
lints on off-convention names, not collisions.

One derivation is stated without an experiment: `__RIDL_FB_NO_CODEC_*` is
spelled from `snake_case` like the codec functions, so two declarations with one
`snake_case` and no FlatBuffers bound would give it twice. It is not reproduced,
because it needs two unbounded declarations; §4.2 changes its spelling with the
others.

## 10. Decisions taken on Sebastien's behalf

1. **The rule of §4.1**: the language refuses a collision only when every
   projecting backend meets it, and the Rust backend owns the rest. Rejected:
   extending RIDL-149 to the scopes #449, #453 and #455 proposed, because only
   the Rust backend applies those transforms there (X-6a) and the constraint
   puts such a rule in the backend.
2. **RIDL-149's arm and value scopes stay**, and the ADR-0016 amendment records
   that they are Rust-only transforms as an open question. Rejected: moving them
   into the claim table now, which loosens the language and is needed by no
   issue in scope.
3. **TYPL-215 covers a tuple's fields.** Rejected: a new TYPL code, because the
   rule and its remedy are the same as for a struct's fields.
4. **A collision of two ridl-derived names is a build error of the package**,
   from one claim table per Rust namespace. Rejected: skipping the interface
   (E11.14 decision 2), because a skipped face is found only at the consumer's
   call site; and per-shape modules, because they rename every descriptor, view
   and tuple path.
5. **`Wire` becomes `__Wire`, and E11.14 decision 5 is retired.** This reverses
   a recorded decision. Rejected: keeping the refusal, because the backend would
   refuse a package over a name it chose.
6. **`bytes` moves to a trait named `ridl_rt::payload::View<'a>`.** Rejected:
   deleting `bytes`; and the trait name `Bytes`, because `View` names what
   implements it. The name `View` is not exported by `ridl-rt` today; the
   associated type `Payload::View` is a trait item and does not conflict.
7. **The keyword escape becomes injective by one more `_`.** Rejected: a
   leading-underscore escape, which renames every existing keyword-named item.
8. **Primitive names are written `::core::primitive::<name>` in package scope.**
   The face module is left as it is (X-4d). Rejected: refusing a declaration
   named like a primitive.
9. **Internal names are spelled from the declared name** with the naming lint
   allowed. Rejected: refusing two declarations whose `snake_case` agrees.
10. **driftsys/ridl#416 and #424 are in scope** (§8).
11. **The release is 0.5.0**, with `ridl-rt` 0.5.0 carrying `payload::View`,
    published at the tag.
12. **Two pull requests**: one for TYPL-215 over tuples in `ridl-sem`, where a
    peer session owns `check.rs`; one for everything else (the backend, the
    `ridlc` tree, the `ridl-rt` trait, the records of §4.4, the tests of §6).
    Rejected: one pull request, which would wait on the peer-owned file.

## Appendix: experiments

**Method.** Each experiment is a workspace in the scratchpad: a `ridl.toml`, the
source below, `ridl check .`, `ridl build . --emit rust --out-dir gen` with the
binary built from this branch (`cargo build -p ridl-cli`, commit d09c520), and
`cargo check --offline` of the emitted crate with `ridl-rt` patched to
`crates/ridl-rt` of this worktree. Every result below was obtained at
`RUSTUP_TOOLCHAIN=1.98.1` (the pin); most were first run at 1.97.1, the default
toolchain of the scratch directory, and no result differs. The first error line
of `cargo check --message-format short` is shown; the rest of the output is cut.

**How a fix is proven.** A fix changes what the emitter writes, and this stage
changes no crate. So each fix is the emitted crate edited to what the fixed
emitter would write, then checked. Where the edit cannot be told apart from the
colliding name (a declaration named `String` beside the `String` the backend
writes), the source is first built with a placeholder name (`Qqq`, `qqq`,
`QQQ`), the qualification is applied to that crate, where every prelude name is
the prelude's, and the placeholder is then renamed to the colliding name
(including inside derived names such as `QqqFbView` and `__ridl_fb_encode_qqq`).
The result is what the fixed emitter would write for the colliding source. The
qualification scripts are two `perl` substitutions: the prelude one writes
`impl ::core::default::Default for`, `::std::string::String`, `::std::vec::Vec`,
`::core::option::Option`, `::core::option::Option::Some(`,
`::core::option::Option::None` and `<T as ::core::default::Default>::default()`;
the primitive one writes `::core::primitive::<name>` everywhere except on a
`#[repr(..)]` line.

### X-0: baseline

`examples/cabin` built and checked the same way compiles with no error and no
warning. It is the control for the harness.

### X-1: the keyword escape (driftsys/ridl#583)

X-1a:

```ridl
package probe.x01a

enumset W {
  self = 0
  self_ = 1
}
```

Emitted: `pub const self_: W = W(1 << 0);` and
`pub const self_: W = W(1 << 1);`. `ridl check` exit 0. Result:
`` error[E0592]: duplicate definitions with name `self_` ``. Fix: the second is
`self__`: compiles.

X-1b, struct fields `self : Level` and `self_ : Level`: `pub self_: Level,`
twice; ``error[E0124]: field `self_` is already declared``. Fix: compiles.

X-1c, `type Self : integer [0..100]` and `type Self_ : integer [0..100]`:
`pub struct Self_(i64);` twice and `pub struct Self_FbView<'a>` twice;
``error[E0428]: the name `Self_` is defined multiple times``. Fix (the second is
`Self__`, `Self__FbView`): compiles, with the `non_camel_case_types` warning on
`Self_FbView` that the first declaration draws today.

X-1d,
`interface Cabin { signal self : Level @10ms  signal self_ : Level @10ms }`:
`pub struct CabinSelf;` twice (both names are `Self` under `camel_case`) and
`pub fn self_(` twice in `Client` and in `Publisher`;
``error[E0428]: the name `CabinSelf` is defined multiple times``. Fix: the claim
table refuses the descriptor pair; no compile fix.

X-1e, `struct S { t : (self : Level, self_ : Level) }`: `pub self_: Level,`
twice in `ST`; ``error[E0124]: field `self_` is already declared``. Fix:
compiles.

X-1f, two packages in one workspace, `p.self` (`type A`) in `self/` and
`p.self_` (`type B`) in `self_/`. `ridl build` writes `p.self.rs` and
`p.self_.rs`, and `lib.rs` holds one module:

```rust,ignore
pub mod p {
    #[path = "p.self_.rs"]
    pub mod self_;
}
```

The crate compiles and package `p.self` is not in it. Fix: `lib.rs` holds
`#[path = "p.self.rs"] pub mod self_;` and
`#[path = "p.self_.rs"] pub mod self__;`, and a consumer builds `p::self_::A`
and `p::self__::B`: compiles.

X-1 combined (the test of §6): one package with `type Self`, `type Self_`,
`enumset W { self = 0  self_ = 1  super = 2  super_ = 3 }` and
`struct S { self, self_, crate, crate_ : Level  t : (self : Level, self_ : Level) }`,
fixed: compiles, with naming-lint warnings only.

### X-2: a field whose accessor is `bytes` (driftsys/ridl#587)

`struct Packet { bytes : Level }` is refused by `ridl check` with FORM-102:
`bytes` is a typl keyword.

X-2a:

```ridl
package probe.x02

type Level : integer [0..100]

struct Packet {
  Bytes : Level
}
```

Emitted in `impl<'a> PacketFbView<'a>`: `pub fn bytes(&self) -> &'a [u8]` and
`pub fn bytes(&self) -> Level`. Result:
`` error[E0592]: duplicate definitions with name `bytes` ``.

X-2b, `struct Packet { t : (Bytes : Level, other : Level) }`: the same pair in
`PacketTFbView`; E0592.

Fix: a copy of `ridl-rt` gains

```rust,ignore
pub trait View<'a> {
    fn bytes(&self) -> &'a [u8];
}
```

in `payload`; every view's inherent `bytes` is deleted and
`impl<'a> ::ridl_rt::payload::View<'a> for <X>FbView<'a> { fn bytes(&self) -> &'a [u8] { self.buf } }`
is added; X-2a also gets this consumer:

```rust,ignore
pub fn consumer(view: PacketFbView<'_>) -> (Level, usize) {
    let field: Level = view.bytes();
    let raw: &[u8] = ::ridl_rt::payload::View::bytes(&view);
    (field, raw.len())
}
```

Both crates compile against the copy, with no warning.

### X-3: a declaration or an interface named `Wire` (driftsys/ridl#588)

X-3a, `type Wire : integer [0..100]`: `ridl check` exit 0; `ridl build` exit 1,
"`probe.x03a.Wire` collides with the `Wire` encoding alias the interaction face
emits at package scope; rename the declaration". X-3b,
`interface Wire { signal level : Level @10ms }`: the same, "rename the
interface".

Fix: built with `Qqq` for `Wire`, then every `Wire` of the alias and its uses
(`pub type Wire`, `Payload<Wire>`, `super::Wire`) renamed `__Wire`, then `Qqq`
renamed `Wire`. Both crates hold `pub type __Wire = …;` beside `pub struct Wire`
(X-3a) or `pub struct Wire;` and `pub mod wire` (X-3b), and compile with no
warning: `__Wire` draws no naming lint.

### X-4: declarations named like prelude names (driftsys/ridl#423, #424)

The base source is
`crates/ridl-backend-rust/tests/fixtures/flatbuffers_roundtrip.ridl` under a new
package line, plus:

```ridl
interface Cabin {
  signal speed : Speed @10ms
  event  warn  : Inner @[100ms..1s]
  command set(level: Speed) @[..50ms]
  query  avg(window: Count): Speed @[..50ms]
}

type <Name> : integer [0..100]
```

One package per name. `ridl check` exits 0 for every one.

| Name                                                                                                                                                       | Result                                                                                                                     |
| ---------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------- |
| `Default`                                                                                                                                                  | `` error[E0404]: expected trait, found struct `Default` `` at `impl Default for Speed {`                                   |
| `String`                                                                                                                                                   | `` error[E0308]: mismatched types: expected `&str`, found `&String` `` in `Label::new`                                     |
| `Vec`                                                                                                                                                      | `error[E0107]: struct takes 0 generic arguments but 1 generic argument was supplied` at `pub struct Blob(Vec<u8>);`        |
| `Option`                                                                                                                                                   | E0107 at `pub pair: Option<ReportPair>,`                                                                                   |
| `Some`                                                                                                                                                     | `` error[E0308]: mismatched types: expected `i64`, found `Duration` `` at `min: Some(::ridl_rt::sample::Duration(10000)),` |
| `None`                                                                                                                                                     | ``error[E0308]: mismatched types: expected `Option<ReportPair>`, found struct constructor`` at `pair: None,`               |
| `Box`, `Clone`, `Copy`, `Debug`, `PartialEq`, `Eq`, `Hash`, `PartialOrd`, `Ord`, `Result`, `Ok`, `Err`, `From`, `TryFrom`, `Into`, `Sized`, `Send`, `Sync` | compile                                                                                                                    |

A newtype is a tuple struct, so `Some` and `None` reach the value namespace as
well as the type one.

Fix, one package per failing name, by placeholder: compiles, no warning, for
each of the six.

X-4c, the test of §6: the base with the calls given clauses,

```ridl
command set(level: Speed) @[..50ms] [
  require level < 100
]
query  avg(window: Count): Speed @[..50ms] [
  require window > 0
  ensure  result >= 0
]
```

and the eleven declarations `Default`, `String`, `Vec`, `Option`, `Some`,
`None`, `Result`, `Ok`, `Err`, `From`, `TryFrom`. Unfixed: E0404, 47 errors.
Fixed by placeholder: compiles, no warning; the face is emitted (no
`__RIDL_NO_FACE_` note) and holds both `fn require`.

X-4d: the same package with the nine primitive names of X-5 added (twenty
declarations), with the prelude and primitive qualifications applied **only
above the first face module** (`pub mod cabin {`), so that the face module is as
emitted today: compiles, with the eighteen naming-lint warnings of the nine
lowercase types. The face module needs no change.

### X-5: declarations and interfaces named like primitives

X-5a, the X-4 base plus `type <name> : integer [0..100]` for a lowercase name.
`ridl check` exits 0 for every one.

| Name               | Result                                                                                                                            |
| ------------------ | --------------------------------------------------------------------------------------------------------------------------------- |
| `bool`             | `` error[E0326]: implemented const `PROVISIONAL` has an incompatible type for trait: expected `bool`, found a different `bool` `` |
| `f32`              | `` error[E0308]: mismatched types: expected `f32`, found `qf32::f32` ``                                                           |
| `f64`              | ``error[E0308]: mismatched types: expected `f64`, found floating-point number``                                                   |
| `i64`              | ``error[E0072]: recursive type `qi64::i64` has infinite size`` (`pub struct i64(i64);`)                                           |
| `str`              | `` error[E0326]: implemented const `NAME` has an incompatible type for trait: expected `str`, found `qstr::str` ``                |
| `u8`               | `` error[E0053]: method `encode` has an incompatible type for trait: expected `u8`, found `qu8::u8` ``                            |
| `u16`              | `` error[E0308]: mismatched types: expected `u16`, found `qu16::u16` ``                                                           |
| `u64`              | ``error[E0308]: mismatched types: expected `u64`, found integer``                                                                 |
| `usize`            | `` error[E0326]: implemented const `MAX_SIZE` has an incompatible type for trait: expected `usize`, found `qusize::usize` ``      |
| `i8`, `i32`, `u32` | compile (the package-scope code does not write them)                                                                              |

X-5b, the X-4 base plus `interface <Name> { signal on : Engaged @10ms }` for
`Bool`, `F64`, `I64`, `Str`, `U8`, `U16`, `U32`, `Usize`: each emits
`pub mod bool {` (and so on) beside the package-scope code and compiles.

Fix, one package per failing name, by placeholder: compiles, with the two
`non_camel_case_types` warnings the lowercase type and its view draw today.
X-5c, the nine names in one package: compiles, eighteen such warnings.

### X-6: a tuple's field names (driftsys/ridl#449)

X-6a:

```ridl
package probe.x06

type Speed : integer [0..250]

struct Reading {
  bounds : (minSpeed : Speed, min_speed : Speed)
}
```

Rust: `pub min_speed: Speed,` twice in `ReadingBounds`;
``error[E0124]: field `min_speed` is already declared``. The same source with
`--emit proto` and `--emit flatbuffers` emits `ReadingBounds` with the fields
`field_1` and `field_2`; with `--emit typescript` it emits
`bounds: { minSpeed: Speed; min_speed: Speed }`, and `tsc --noEmit --strict`
accepts it. Fix: the claim table refuses the pair.

X-6b, `bounds : (a : Speed, a : Speed)`: Rust `pub a: Speed,` twice, E0124;
TypeScript `bounds: { a: Speed; a: Speed }`, and `tsc` reports
`error TS2300: Duplicate identifier 'a'.` Fix: TYPL-215 refuses it at check.

### X-7: induced tuple names (driftsys/ridl#449 point 1, #453)

X-7a, `struct S { t : (XY : (a : Counter), x_y : (b : Counter)) }`: `ridl check`
exit 0; `ridl build` exit 1, "the generated name STXY is claimed by two
different tuple types, (a) and (b); …". X-7b,
`struct S { XY : (a : Counter), x_y : (b : Counter) }`: the same for `SXY`. Both
are refused today by the lowering's `TupleCollision`; the claim table takes that
refusal over unchanged.

### X-8: two members equal under `camel_case` (driftsys/ridl#455)

```ridl
package probe.x08

type Level : integer [0..100]

interface Cabin {
  signal XY : Level @10ms
  signal x_y : Level @10ms
}
```

`ridl check` exit 0; `ridl build` exit 0. Emitted: `pub struct CabinXY;` twice.
Result: ``error[E0428]: the name `CabinXY` is defined multiple times``, with two
E0119 on its trait impls. Fix: the claim table refuses the pair.

### X-9: a descriptor against a declaration

```ridl
package probe.x09

type CabinTemperature : integer [-40..85]

interface Cabin {
  signal temperature : CabinTemperature @10ms
}
```

Emitted: `pub struct CabinTemperature(i64);` and the descriptor
`pub struct CabinTemperature;`. Result:
``error[E0428]: the name `CabinTemperature` is defined multiple times``. Fix:
the claim table refuses the pair.

### X-10: a view against a declaration

`type Level : integer [0..100]` and `struct LevelFbView { level : Level }`:
`pub struct LevelFbView {` and the view `pub struct LevelFbView<'a> {`;
``error[E0428]: the name `LevelFbView` is defined multiple times``. Fix: claim.

### X-11: an induced tuple against a declaration

```ridl
package probe.x11

type Speed : integer [0..250]

struct Reading {
  bounds : (low : Speed, high : Speed)
}

struct ReadingBounds {
  low : Speed
}
```

`ridl check` exit 0. `ridl build --emit rust` exit 1: "the FlatBuffers
projection describes no table for `ReadingBounds`". `--emit proto`:
"`ReadingBounds` is claimed twice in package `probe.x11`: once by struct
`ReadingBounds`, and again by a message generated for a tuple, named for the
field path that reaches it. …"; `--emit flatbuffers`: the same for a table. Fix:
the claim table, run before the codec, refuses with both sources named.

### X-12: two descriptors across interfaces

`interface A { signal bC : Level @10ms }` and
`interface AB { signal c : Level @10ms }`: `pub struct ABC;` twice;
``error[E0428]: the name `ABC` is defined multiple times``. Fix: claim.

### X-13: a descriptor against an interface

`interface Horn { signal active : Level @10ms }` and
`interface HornActive { signal level : Level @10ms }`: `pub struct HornActive;`
twice; ``error[E0428]: the name `HornActive` is defined multiple times``. Fix:
claim.

### X-14: face modules and the skipped-face note

X-14a, `interface HTTPServer` and `interface HttpServer`, each
`{ signal level : Level @10ms }`: `pub mod http_server {` twice;
``error[E0428]: the name `http_server` is defined multiple times``. Fix: claim.

X-14b, `type cabin : integer [0..100]` and
`interface Cabin { signal level : cabin @10ms }`: `pub struct cabin(i64);` and
`pub mod cabin {`; ``error[E0428]: the name `cabin` is defined multiple times``.
Fix: claim.

X-14c, `interface HTTPServer` and `interface HttpServer`, each
`{ command set(a: Level, b: Level) @[..50ms] }`, a call shape the face skips:
`const __RIDL_NO_FACE_HTTP_SERVER: () = ();` twice;
``error[E0428]: the name `__RIDL_NO_FACE_HTTP_SERVER` is defined multiple times``.
Fix: `#[allow(non_upper_case_globals)] const __RIDL_NO_FACE_HTTPServer` and
`… __RIDL_NO_FACE_HttpServer`: compiles, no warning.

### X-15: codec functions of two declarations equal under `snake_case`

`type HTTPServer : integer [0..100]` and `type HttpServer : integer [0..100]`:
`pub(crate) fn __ridl_fb_encode_http_server(` twice, and the same for `verify`
and `decode`;
``error[E0428]: the name `__ridl_fb_encode_http_server` is defined
multiple times``.
Fix, by placeholder (`Qqa`, `Qqb`, and a `struct Both` naming both so a
cross-type call is exercised): the functions are
`__ridl_fb_{encode,verify,decode}_HTTPServer` and `…_HttpServer`, each under
`#[allow(non_snake_case)]`: compiles, no warning.

### X-16: an enum set bit named `default`

An enum set with the bits `try_from`, `new`, `MAX_SIZE`, `encode`, `decode`,
`verify`, `check`, `default`, `View`, `Error`, `from`, `into`, `value`,
`bytes_`, `clone`, `eq`, `fmt`, `hash`, used as a struct field and as a signal
payload. Only `default` fails: `pub const default: Flags = …;` captures
`Holder { flags: Flags::default() }` and the signal's
`fn init() -> Self::Payload { Flags::default() }`;
`` error[E0618]: expected function, found `Flags` ``, twice. Fix:
`<Flags as ::core::default::Default>::default()` at both sites: compiles, with
the `non_upper_case_globals` warnings the lowercase bits draw today.

### X-17: a constant against a descriptor constructor

```ridl
package probe.x17

type Level : integer [0..100]

const CabinLevel : Level = 5

interface Cabin {
  signal level : Level @10ms
}
```

`pub const CabinLevel: Level = …;` and the unit struct `pub struct CabinLevel;`,
which is a value as well as a type;
``error[E0428]: the name `CabinLevel` is
defined multiple times``. Fix: claim
(value namespace).

### X-18: a type named like its child package (driftsys/ridl#416)

Package `veh` (`veh.ridl`) declares `type common : integer [0..100]`; package
`veh.common` (`common/common.ridl`) declares `import veh.common as Common` and
`struct Uses { c : Common }`. `ridl check` exit 0 (TYPL-008 warns that the alias
is not needed). `lib.rs`:

```rust,ignore
pub mod veh {
    #[path = "veh.rs"]
    mod __ridl_package;
    pub use __ridl_package::*;
    #[path = "veh.common.rs"]
    pub mod common;
}
```

Result: `` error[E0573]: expected type, found module `crate::veh::common` ``.
Fix: `#[doc(hidden)] pub mod __ridl_package;`, and every `crate::veh::common`
that names the type in `veh.common.rs` written
`crate::veh::__ridl_package::common` (six sites): compiles, with the naming-lint
warnings of the lowercase type.

### X-19: control, a declaration and an interface of one name

`type Cabin : integer [0..100]` and
`interface Cabin { signal level : Cabin @10ms }`: `ridl check` exit 1,
`` error[TYPL-009]: duplicate declaration of `Cabin` ``. The language already
refuses this pair, so the claim table never sees it.
