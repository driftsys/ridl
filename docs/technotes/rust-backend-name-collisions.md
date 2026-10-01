# Generated-name collisions in the Rust backend

How the Rust backend keeps the names it generates from colliding, as built.
Informative: the binding rules are the 2026-09-29 and 2026-09-30 amendments of
[ADR-0016](../decisions/ADR-0016-schema-projection-and-the-name-transform.md),
[ADR-0017](../decisions/ADR-0017-proto3-projection-rules.md) decision 4,
[ADR-0023](../decisions/ADR-0023-interaction-face-generation.md) decision 7 and
its 2026-09-30 consequence note, and
[ADR-0021](../decisions/ADR-0021-ridl-rt-0.1-api-and-release.md) decision 20.
This note holds the per-namespace map, the decisions the code comments cite by
number, and the alternatives that were rejected.

The reasoning and the experiments behind it are in the archived design note,
[`docs/archive/2026-09-29-generated-name-collisions-design.md`](../archive/2026-09-29-generated-name-collisions-design.md).
Its appendix holds the source of every experiment named below (X-n). Read it as
a design: its §5, §6, §9 and §10 plan the pull requests and the release, and the
pull requests have merged (driftsys/ridl#606, #608, #611 and #612). The issues
it closed are driftsys/ridl#583, #587, #588, #423, #449, #453, #455, #416 and
#424.

## The problem

`ridl check` accepted a source, and the Rust backend then emitted two items of
one name in one Rust namespace, or an item whose name hides a name the backend
writes unqualified, and rustc failed in the consumer's build. The class had
twenty-one reproducing cases. Four statements in the original issues were
corrected by experiment:

- driftsys/ridl#583: an enum set with bits `self` and `self_` gives E0592, not
  E0201 (X-1a). Packages `p.self` and `p.self_` mapped to one module, and the
  crate silently linked only one (X-1f).
- driftsys/ridl#587: a field named `bytes` is a typl keyword and cannot exist. A
  field named `Bytes` or `BYTES` reaches the view accessor `bytes` through
  `snake_case` (X-2a).
- driftsys/ridl#588: a type named `Wire` was refused by the backend at
  `ridl build`, so the consumer did not see a rustc error (X-3).
- driftsys/ridl#455: the interaction face is reached by `ridl build`, and
  members `XY` and `x_y` gave E0428 (X-8).

## The rule

1. **A collision is the language's only when every backend that projects the
   name meets it.** Two names of one scope that collide under a pinned transform
   that every projecting backend applies in that scope are RIDL-149, in
   `ridl-sem`. A name spelled twice in one scope is TYPL-215 and its siblings.
   The one language rule this work added is TYPL-215 over a tuple's fields: a
   tuple is an anonymous struct, and a repeated field fails in Rust (E0124) and
   in TypeScript (TS2300, X-6b).
2. **Every other collision belongs to one target's namespaces, and that target's
   backend owns it** (ADR-0017 decision 4). The Rust backend applies this in two
   steps.
   1. A name the backend chose never refuses a package. When one of the two
      names is fixed by the backend, written unqualified by it, produced by its
      keyword escape, or derived through a lossy transform for internal use, the
      backend changes that name so that no ridl name reaches it.
   2. Two ridl-derived names that one Rust namespace cannot hold are refused at
      `ridl build`, from one claim table per Rust namespace
      (`crates/ridl-backend-rust/src/claims.rs`). The message names the
      generated name and both sources. A table claims only what the backend
      emits.

The line in step 1 follows from one constraint: an asymmetry between backends
justifies a backend strategy, not a language rule. The wire backends name a
tuple's fields by position (`field_1`, `field_2`) and the TypeScript backend
keeps every name as declared, so `(minSpeed : Speed, min_speed : Speed)` is
legal there (X-6a). Only the Rust backend applies `snake_case` to a tuple's
fields, so that collision is the Rust backend's.

One inconsistency is left in place and recorded as open in ADR-0016: RIDL-149
checks a union's arms under `camel_case` and an enum's values under
`pascal_case`, and only the Rust backend applies either transform. Under the
rule those two checks are the Rust backend's. They stay in the language until a
change needs to move them.

## Per namespace

| Namespace                                            | Collision removed                                                                                                          | Home                    | Mechanism                                                                                                                                                                    |
| ---------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------- | ----------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| One tuple's fields                                   | the same name twice                                                                                                        | language                | TYPL-215 covers a tuple's fields                                                                                                                                             |
| Package module, types and values                     | a declaration named like a prelude name the backend writes (`Default`, `String`, `Vec`, `Option`, `Some`, `None`)          | backend, rename         | the backend writes `::core::default::Default`, `::std::string::String`, `::std::vec::Vec` and `::core::option::Option` with its `Some` and `None`, in package scope          |
| Package module, types                                | a declaration named like a primitive (`bool`, `i8` to `i64`, `u8` to `u64`, `f32`, `f64`, `usize`, `str`)                  | backend, rename         | the backend writes `::core::primitive::<name>`; a `#[repr(..)]` line keeps the bare name                                                                                     |
| Enum set constants                                   | a bit named `default` captures `T::default()`                                                                              | backend, rename         | the backend writes `<T as ::core::default::Default>::default()` (`defaults.rs`, `descriptors.rs`)                                                                            |
| Struct and tuple views                               | a field whose `snake_case` is `bytes`, against the view's fixed `bytes`                                                    | backend, trait          | `bytes` is a method of `ridl_rt::payload::View<'a>`; the field accessor stays inherent and wins the dot call, and `View::bytes(&view)` reaches the buffer                    |
| Package module, types                                | a declaration or an interface named `Wire`                                                                                 | backend, removal        | the alias `pub type Wire` does not exist; every site writes `::ridl_rt::encoding::FlatBuffers`; no name is reserved                                                          |
| Every namespace `ident()` writes, and the crate tree | a name `k` against `k_`, where `k` is `crate`, `self`, `Self` or `super`                                                   | backend, rename         | the keyword escape is injective: one of the four followed by zero or more `_` gets one more `_`, so `self` is `self_` and `self_` is `self__`                                |
| Package module, values                               | two declarations or two skipped interfaces whose `snake_case` agrees (`HTTPServer`, `HttpServer`)                          | backend, rename         | `__ridl_fb_{encode,verify,decode}_*`, `__RIDL_FB_NO_CODEC_*` and `__RIDL_NO_FACE_*` are spelled from the declared name, with a naming-lint allow on the item                 |
| Crate module tree                                    | a type `veh.common` hidden by the child package module `veh::common`                                                       | backend, rename of path | a reference to such a type is written `crate::veh::__ridl_package::common`; `ridlc` makes `__ridl_package` `#[doc(hidden)] pub`; the type stays unreachable as `veh::common` |
| Package module, types                                | descriptor, view or tuple against a declaration; descriptor against an interface; two descriptors; face module; two tuples | backend, claim          | one claim table over the package module's type namespace                                                                                                                     |
| Package module, values                               | a constant against a unit-struct or tuple-struct constructor                                                               | backend, claim          | the same table over the value namespace                                                                                                                                      |
| One induced tuple's fields                           | two field names whose `snake_case` agrees                                                                                  | backend, claim          | one table per induced tuple                                                                                                                                                  |
| Face module types and `Event` variants               | two members whose `camel_case` agrees                                                                                      | backend, claim          | the package-scope descriptor `<Iface><Member>` carries the same collision, so the package table refuses it first; the face module and `Event` tables are kept for totality   |

Two facts decide many rows. A ridl identifier starts with a letter, so a name
that starts with `_` is out of reach of every ridl name, and the backend's
internal items (`__ridl_fb_*`, `__RIDL_*`, `__ridl_package`) rely on it. And a
Rust trait has its own namespace, so a trait item never meets an inherent item,
a module item or an item of another trait.

Rows that look exposed and are not: a declaration named `Result`, `Ok`, `Err`,
`From`, `TryFrom`, `Box`, `Into`, `Sized`, `Send`, `Sync` or a derive name
compiles (X-4); an interface named `Bool`, `I64` or `Usize`, whose face module
is `bool`, `i64` or `usize`, compiles (X-5b); the face module has no glob import
of its parent, so a declaration named `Some` or `Option` does not reach the code
inside it (X-4d).

## The claim tables

The tables run before the codec, so a tuple named like a declaration is refused
with both sources named and not by the FlatBuffers projection, whose message
named neither (X-11). The lowering's own tuple collision (X-7a, X-7b) is
reported first, with its earlier message.

A table claims only the items the backend emits. `generate` and `generate_with`
emit no descriptors and no face, so no interface claims anything there;
`generate_face_with` claims every named interface; `generate_pipeline`, the
entry point `ridl build --emit rust` calls, claims the interfaces whose face it
does not skip. An interface whose face is skipped under E11.14 decision 2
therefore claims nothing, and a collision among its members is refused by the
change that makes the face emittable (decision 13 below; X-8b and X-8c). With
this, every case a table refuses either failed rustc before or was already
refused at `ridl build`, so the tables reject no package that built.

## Decisions

The code comments and tests cite these by number. They are the numbers of the
design note's §10, which the archive keeps. The records named at the top hold
the binding ones (ADR-0016, ADR-0021 decision 20, ADR-0023 decision 7's note);
the others are choices of the Rust backend.

1. The rule above: the language refuses a collision only when every projecting
   backend meets it, and the Rust backend owns the rest.
2. RIDL-149's arm and value scopes stay, recorded as an open question.
3. TYPL-215 covers a tuple's fields; no new code.
4. A collision of two ridl-derived names is a build error of the package, from
   one claim table per Rust namespace.
5. The `pub type Wire` alias is removed. This reversed E11.14 decision 5, which
   refused a package that used the name, and `refuse_wire_collision` is gone.
6. `bytes` moves to `ridl_rt::payload::View<'a>`.
7. The keyword escape is injective by one more `_`.
8. Primitive names are written `::core::primitive::<name>` in package scope. The
   face module is unchanged (X-4d).
9. Internal names are spelled from the declared name, with the naming lint
   allowed.
10. driftsys/ridl#416 and #424 are in scope: the first by the `__ridl_package`
    path, the second by the prelude-name compile test.
11. The next release is 0.5.0, with `ridl-rt` carrying `payload::View`; the
    workspace version is still 0.4.0 until it is cut.
12. Four pull requests: (a) TYPL-215 over a tuple's fields, driftsys/ridl#606;
    (b) the renames of names the backend chose, driftsys/ridl#608; (c) the claim
    tables, driftsys/ridl#612; (d) the `__ridl_package` path, driftsys/ridl#611.
13. The claim table claims only the items the backend emits.

## Alternatives considered

| Alternative                                                                         | Verdict          | Reason                                                                                                                                                                                                                                                                                                        |
| ----------------------------------------------------------------------------------- | ---------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Extend RIDL-149 to a tuple's fields, a struct field's and a member's `camel_case`   | rejected         | Each transform is applied in that scope by the Rust backend alone (X-6a), so the check would put one backend's namespaces into the language. It would also refuse `HTTPServer` beside `HttpServer`, which the proto, FlatBuffers and TypeScript backends accept (X-15).                                       |
| A reserved-word list in `ridl-sem`                                                  | rejected         | The same objection, and the list grows with every fixed name the backend adds, which driftsys/ridl#580 already rejected.                                                                                                                                                                                      |
| Keep E11.14 decision 5's refusal of `Wire`                                          | rejected         | The backend would refuse a package over a name it chose.                                                                                                                                                                                                                                                      |
| Move `Wire` into each face module                                                   | rejected         | The descriptors at package scope also name it, and an interface whose members are all `fixed` has no face module.                                                                                                                                                                                             |
| Rename the alias `__Wire`                                                           | rejected         | It cannot collide, but the alias was public API that tests and the book used, and a double underscore reads as internal.                                                                                                                                                                                      |
| An associated type on a runtime trait, `<Level as Encoded>::Wire`                   | rejected for now | It compiles beside a user `Wire`, but every use is longer than the full path. It is the candidate when a second encoding lands (E11.8's `--wire` flag), because a consumer then needs "the package's encoding" without naming it.                                                                             |
| Delete the views' `bytes`                                                           | rejected         | A function that holds only a nested view would lose the buffer.                                                                                                                                                                                                                                               |
| A leading-underscore keyword escape (`_self`)                                       | rejected         | It renames every existing keyword-named item, where the injective suffix renames only `k_` names.                                                                                                                                                                                                             |
| Refuse a declaration named like a primitive                                         | rejected         | The backend chose to write the primitive unqualified, and `::core::primitive` is stable since Rust 1.43.                                                                                                                                                                                                      |
| Refuse `HTTPServer` beside `HttpServer` in the claim table                          | rejected         | The only colliding names are internal and derived through a lossy transform the backend chose. The proto backend's refusal of two such enums (X-15b) is its own and stays.                                                                                                                                    |
| Skip the interface's face and descriptors on a member collision (E11.14 decision 2) | rejected         | A skipped face builds and is found missing at the consumer's call site. A name collision is permanent until the source changes.                                                                                                                                                                               |
| Per-shape modules for descriptors, views and induced tuples                         | rejected         | It removes the concatenation collisions by construction but renames every descriptor, view and tuple path a consumer names. The likeliest case, `type CabinTemperature` beside `interface Cabin { signal temperature }` (X-9), is refused with a message; if it proves common, this option can be taken then. |
| A new TYPL code for a repeated tuple field                                          | rejected         | The rule and its remedy are TYPL-215's.                                                                                                                                                                                                                                                                       |
| A naming rule or a refusal for driftsys/ridl#416                                    | rejected         | A path through `__ridl_package` compiles (X-18), so the backend removes the collision without refusing.                                                                                                                                                                                                       |

## Where the experiments are held

Tests named by the experiments the code comments cite. A test source is inline
in the file; the archived note's appendix has the same source as a `ridl build`
workspace.

| Experiment                                   | Test                                                                                                                                                                                                                                                                 |
| -------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| X-1d (members `self` and `self_`)            | `two_members_equal_under_camel_case_after_the_escape_are_refused` in `crates/ridl-backend-rust/tests/name_collision_claims.rs`                                                                                                                                       |
| X-1a to X-1c, X-1e, injective escape         | `a_keyword_and_its_escape_compile` in `crates/ridl-backend-rust/tests/name_collision_compile.rs`; `the_keyword_escape_is_injective` in `crates/ridl-backend-rust/src/tests.rs`                                                                                       |
| X-1f (packages `p.self` and `p.self_`)       | `packages_named_self_and_self_underscore_both_reach_the_crate_tree` in `crates/ridlc/tests/rust_crate_emit.rs`                                                                                                                                                       |
| X-2 (accessor `bytes`)                       | `a_field_whose_accessor_is_bytes_compiles` in `name_collision_compile.rs`; the doc test of `ridl_rt::payload::View`                                                                                                                                                  |
| X-3 (`Wire`)                                 | `a_declaration_or_interface_named_wire_compiles` in `name_collision_compile.rs`; `a_declaration_named_wire_generates_a_face_with_no_alias` in `crates/ridl-backend-rust/src/tests.rs`                                                                                |
| X-4, X-5 (prelude and primitive names)       | `declarations_named_like_prelude_names_compile`, `declarations_named_like_primitives_compile` in `name_collision_compile.rs`                                                                                                                                         |
| X-6a, X-7a, X-7b                             | `two_tuple_fields_equal_under_snake_case_are_refused`, `two_nested_tuples_equal_under_camel_case_are_refused`, `two_struct_fields_whose_tuples_are_equal_under_camel_case_are_refused`                                                                               |
| X-8, X-8b, X-8c                              | `two_members_equal_under_camel_case_are_refused`, `a_skipped_face_claims_nothing`, `an_emitted_face_claims_its_members` in `name_collision_claims.rs`; `a_skipped_face_with_members_equal_under_camel_case_compiles` in `name_collision_compile.rs`                  |
| X-9, X-10, X-11, X-12, X-13                  | `a_descriptor_named_like_a_declaration_is_refused`, `a_view_named_like_a_declaration_is_refused`, `a_tuple_named_like_a_declaration_is_refused_before_the_codec`, `two_descriptors_across_interfaces_are_refused`, `a_descriptor_named_like_an_interface_is_refused` |
| X-14a, X-14b, X-17                           | `two_face_modules_equal_under_snake_case_are_refused`, `a_face_module_named_like_a_declaration_is_refused`, `a_constant_named_like_a_descriptor_is_refused`                                                                                                          |
| X-14c, X-15 (names equal under `snake_case`) | `names_equal_under_snake_case_compile` in `name_collision_compile.rs`                                                                                                                                                                                                |
| X-16 (enum set bit `default`)                | `an_enum_set_bit_named_default_compiles` in `name_collision_compile.rs`                                                                                                                                                                                              |
| X-18 (type named like its child package)     | `a_type_named_like_its_child_package_is_named_through_ridl_package` in `crates/ridlc/tests/rust_crate_emit.rs`                                                                                                                                                       |
| TYPL-215 over a tuple's field (X-6b)         | the TYPL-215 check in `crates/ridl-sem/src/check.rs`                                                                                                                                                                                                                 |

## What a consumer sees

The generated API changed in three places when this landed. `bytes` is no longer
an inherent method of a view: a consumer adds `use ridl_rt::payload::View;`
(with a field whose accessor is `bytes`, `view.bytes()` is the field and
`View::bytes(&view)` is the buffer). The `Wire` alias is gone, and a consumer
writes `ridl_rt::encoding::FlatBuffers`. A source name `self_`, `Self_`,
`super_` or `crate_`, or a package segment of that form, emits one more `_`.
Everything else is an addition or an absolute path to the same item. See
[the interaction-face design record](../design/interaction-face.md) and
[the FlatBuffers codec design record](../design/flatbuffers-codec.md) for the
emitted shape.

## Not covered

- A lowercase type name or a lowercase enum set bit draws `non_camel_case_types`
  or `non_upper_case_globals` in the consumer's build, and the escape of `Self`
  gives the view `Self_FbView`. These are naming lints on off-convention names,
  not collisions.
- Another backend's namespaces. The Kotlin backend (driftsys/ridlc-gen-kotlin)
  models its own, under the same rule: a name it chose never refuses a package,
  and a collision of two ridl-derived names is refused with both sources named.
