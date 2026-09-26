# Enum variants in PascalCase — design

Status: design for driftsys/ridl#506, option B. Written 2026-09-26. The code
change runs from the plan
[`2026-09-26-enum-variant-pascal-case-plan.md`](2026-09-26-enum-variant-pascal-case-plan.md).
The decision it applies is recorded in
[ADR-0016](../decisions/ADR-0016-schema-projection-and-the-name-transform.md)'s
amendment of 2026-09-26.

## 1. The problem and the decision

A typl `enum` value is declared in `SCREAMING_SNAKE` (typl §2.3, §15.1). The
Rust backend emits it verbatim as a Rust enum variant, so
`enum Warning { LOW_FUEL = 0, CHECK_ENGINE = 1 }` becomes
`pub enum Warning { LOW_FUEL = 0, CHECK_ENGINE = 1 }`, and rustc warns
`non_camel_case_types` on every multi-word variant wherever the generated code
is compiled as part of the consumer's own workspace (Cargo caps the lints of a
registry dependency). Struct fields had the same defect under driftsys/ridl#243
and moved to `snake_case` in #451. After that change, enum variants are the only
generated Rust identifier that does not follow the Rust naming convention.

The maintainer chose option B on 2026-09-26: project each variant through a
pinned PascalCase transform (`CHECK_ENGINE` → `CheckEngine`), with the collision
check ADR-0016 requires for every pinned transform, and amend ADR-0016. The
change renames every generated variant whose `pascal_case` spelling differs from
its typl spelling. For a `SCREAMING_SNAKE` value that is every value with two or
more letters — `PARK` becomes `Park` as well as `CHECK_ENGINE` becoming
`CheckEngine` — although only the multi-word ones draw the lint today; a value
with one letter (`A`, `X2`) keeps its spelling. The renaming breaks a consumer
that names a variant, which the 0.x rule allows (ADR-0021 decision 10 states the
rule for `ridl-rt`; generated code is released with the same toolchain and
follows it).

## 2. The state this design starts from

Measured on `main` at 6557cc4.

- **Four Rust-backend sites spell an enum value as a Rust identifier**, all with
  `ident(declared(value.name.as_ref()))` or the same string:
  - `emit_enum` in `crates/ridl-backend-rust/src/lib.rs` — the variant
    declarations and the `TryFrom<i64>` match arms. Its doc comment says
    "Variant names keep their typl `SCREAMING_SNAKE` spelling."
  - `enum_default` in `crates/ridl-backend-rust/src/defaults.rs` — the value of
    the `Default` impl (`Warning::LOW_FUEL`).
  - `crates/ridl-backend-rust/src/codec.rs` — the `Repr::Enum { first }` string
    stored when the codec is built, and read back as an identifier in the
    FlatBuffers decode fallback (`.unwrap_or(Warning::LOW_FUEL)`).
- **The codegen model already carries a `Spellings` for every enum value**
  (`crates/ridl-ir/src/codegen/lower.rs`, `enum_value`), with the four fields
  `declared`, `snake`, `camel` and `screaming`. The Rust backend reads
  `declared`.
- **`camel_case` is not a PascalCase transform for this input.** It upper-cases
  the first character of each underscore-separated segment and leaves the rest
  as written, so `CHECK_ENGINE` gives `CHECKENGINE`. That silences the rustc
  lint but does not give `CheckEngine`.
- **No check covers an enum's value names under any transform.** `lower_enum` in
  `crates/ridl-sem/src/check.rs` runs TYPL-203 over the integer values, TYPL-210
  over the tombstones, and RIDL-307 over an `error` enum's names. The proto
  backend refuses a colliding enum value at generate time through its own
  `names.claim` (ADR-0017 decisions 4 and 5).
- **An enum value name repeated verbatim is not rejected.**
  `enum E { A = 0, A = 1 }` passes `ridlc check` with exit 0 and the Rust
  backend emits E0428. Filed as driftsys/ridl#554; see §5.3.
- **Enumset bits are not affected.** `emit_enum_set` emits each bit as an
  associated constant (`pub const LOW_FUEL: WarningFlags`), and
  `SCREAMING_SNAKE` is the Rust convention for a constant. It does not name the
  paired enum's variants.
- **Seven rustc compile proofs compile generated Rust, and none denies
  `non_camel_case_types`.** Four are in `crates/ridl-backend-rust/src/tests.rs`
  — `a_tuple_under_an_internal_declaration_is_package_private`,
  `constructible_collections_compile`, `appendix_b_compiles_with_rustc` and
  `appendix_a_compiles_with_rustc` — and deny `non_snake_case`; three of their
  comments say an enum variant draws `non_camel_case_types` by design. Three are
  in `crates/ridlc/tests/corpus.rs`:
  `veh_cluster_generated_rust_compiles_with_rustc` goes through `rustc_accepts`,
  which denies `non_snake_case` and two visibility lints, while
  `veh_common_generated_rust_compiles_with_rustc` and
  `workspace_two_members_composed_compiles_with_rustc` build their own `rustc`
  command and deny no lint. The fixtures that hold a multi-word enum value are
  Appendix B, `veh-common` and `veh-cluster`. Two doc comments in `corpus.rs`,
  on `veh_common_generated_rust_compiles_with_rustc` and on `rustc_accepts`,
  give `non_camel_case_types` on a screaming-case variant as their example of a
  non-fatal lint. The checked-in generated module in
  `crates/ridl-backend-rust/tests/interaction_face.rs` carries
  `#[allow(clippy::upper_case_acronyms)]` for the same spelling.
- **Snapshots that hold a multi-word variant:** 18 variant lines across
  `ridl_backend_rust__tests__appendix_b_rust_snapshot.snap`,
  `ridl_backend_rust__tests__enumset_derived_form.snap`, and
  `corpus__rust@{veh-cluster,services-workspace,veh-common}.snap`. Single-word
  variants move too (`PARK` → `Park`), so every snapshot with an enum moves.
- **Checked-in generated code and hand-written tests name variants in the typl
  spelling:** `crates/ridl-backend-rust/tests/generated/interaction_face.rs`
  (regenerated with `RIDL_UPDATE_GENERATED=1`), and the tests that use it or
  build their own enums — `crates/ridl-backend-rust/tests/interaction_face.rs`,
  `crates/ridl-backend-rust/tests/flatbuffers_roundtrip.rs`,
  `crates/ridl-backend-rust/tests/flatbuffers_conformance.rs`, and assertions in
  `crates/ridl-backend-rust/src/tests.rs`.
- **`examples/cabin`** names `api::Health::WARN` twice in
  `examples/cabin/consumer/src/main.rs`.
- **One document names a generated variant:**
  `docs/technotes/ridl-rt-by-example.md` writes `health: Health::WARN` in a Rust
  fence. The book holds no Rust fence that names an enum variant, and no
  specification or design prose states the verbatim spelling.

## 3. The transform

```text
pascal_case(name) = camel_case(snake_case(name))
```

Both inner functions are the pinned transforms in `crates/ridl-ir/src/name.rs`
(ADR-0016 decisions 1 and 2, and the 2026-09-20 amendment). `snake_case`
lower-cases the name and separates words; `camel_case` then upper-cases the
first character of each underscore-separated segment, drops the underscores, and
drops empty segments.

| typl name      | `snake_case`   | `pascal_case` |
| -------------- | -------------- | ------------- |
| `CHECK_ENGINE` | `check_engine` | `CheckEngine` |
| `PARK`         | `park`         | `Park`        |
| `OK`           | `ok`           | `Ok`          |
| `A`            | `a`            | `A`           |
| `X2`           | `x2`           | `X2`          |
| `ABS_V2`       | `abs_v2`       | `AbsV2`       |
| `V2_ABS`       | `v2_abs`       | `V2Abs`       |
| `LEVEL_10`     | `level_10`     | `Level10`     |
| `HTTP_SERVER`  | `http_server`  | `HttpServer`  |
| `A__B`         | `a__b`         | `AB`          |
| `A_`           | `a_`           | `A`           |
| `checkEngine`  | `check_engine` | `CheckEngine` |
| `HTTPServer`   | `http_server`  | `HttpServer`  |
| `SELF`         | `self`         | `Self`        |

The last three rows are values that do not follow the typl convention. The lexer
admits them (`[A-Za-z][A-Za-z0-9_]*`, one `Ident` kind for every form), so the
transform must be defined for them.

**Properties, each pinned by a test in the plan.**

1. **Defined for every name the lexer admits.** No input panics and no input
   gives an empty string: the lexer requires a leading letter, so the first
   segment is never empty.
2. **The output satisfies rustc's `non_camel_case_types`.** It holds no
   underscore, and its first character is an upper-case letter. A later segment
   may start with a digit (`Level10`), which the lint accepts.
3. **Not injective**, like every case-folding transform (ADR-0016 correction 2).
4. **Its collision set contains `snake_case`'s.** If
   `snake_case(a) == snake_case(b)` then `pascal_case(a) == pascal_case(b)`,
   because `pascal_case` is a function of `snake_case`'s output. The converse
   fails: `CHECK_ENGINE` and `CHECK__ENGINE` differ under `snake_case` and
   collide under `pascal_case`. §5.2 uses this containment.

The transform is **not idempotent**, and nothing depends on it being so: a
one-letter segment leaves two adjacent capitals (`A_B` → `AB`), and
`pascal_case("AB")` is `Ab`. It is applied once, to the declared name.

**`Self` needs no new rule.** `ident()` in `crates/ridl-backend-rust/src/lib.rs`
already escapes `Self` to `Self_`, because `Self` cannot be a raw identifier.
`Self_` cannot collide with another variant, because no `pascal_case` output
holds an underscore. The other Rust keywords are lower-case, so no PascalCase
output reaches them.

**`Ok`, `Err`, `Some` and `None` are legal variant names.** The generated code
always qualifies a variant with its enum (`Self::Ok`, `Health::Ok`) and never
brings an enum's variants into scope with a `use`, so a bare `None` or `Some`
elsewhere in the generated code still resolves to the prelude's `Option`, and a
variant with one of these names shadows nothing.

## 4. Where the transform lives

**`pascal_case` becomes a third public function in
`crates/ridl-ir/src/name.rs`,** beside `snake_case` and `camel_case`. The reason
is the one ADR-0016 decision 2 gives for the other two: a projection is a pure
function from IR identity to a target's namespace, and `ridl-ir` is the only
crate that `ridl-sem` and every backend already depend on. The check in
`ridl-sem` and the emitter in the Rust backend call one definition.

**`Spellings` gains a fifth field, `pascal = 5`,** in
`crates/ridl-ir/proto/ridl/codegen/v1/model.proto`, filled by `spellings()` in
`crates/ridl-ir/src/codegen/names.rs`. The codegen model design's D-2 says a
name is carried once per transform, named after the transform and not after a
language, and says the namespaces the backends use are the pinned transforms and
compositions of them (`screaming` is `snake` upper-cased). `pascal` is one more
composition. Carrying it in the model means a codegen plugin gets the Rust
variant spelling without reimplementing the transform, which is what ADR-0020
decision 8 asks for.

**An empty `pascal` means "derive it from `snake`".** The IR stability design's
compatibility rule (D-5) makes a new field additive only if its default carries
the meaning an old writer implied by omitting it, and a plugin does not refuse a
model written by an older toolchain (`docs/design/codegen-plugins.md` §2). So a
newer Rust backend can read a model an older `ridlc` wrote, with `pascal` empty.
The `model.proto` comment on the field states that an empty `pascal` is
`camel_case(snake)`, and the Rust backend reads the field through one helper
that applies that fallback. With that meaning, the field is additive and the
schema package stays `ridl.codegen.v1`. The cost is snapshot growth: every
`Spellings` in the five `corpus__codegen@*.snap` files that carry a model gains
one line, about 270 lines in total. The other three `corpus__codegen@*.snap`
files are the placeholders of packages that fail to check, and do not move.

**The Rust backend reads `pascal` at the four sites of §2,** in place of
`declared`. The `Repr::Enum { first }` string in `codec.rs` stores the `pascal`
spelling, so the decode fallback names the variant `emit_enum` declared.

## 5. The collision rule

### 5.1 The rule

**RIDL-149 extends to the values of one enum, keyed on `pascal_case`.** Two
values of one `enum` whose names differ in source and share a `pascal_case`
output are an error, reported on the later value with a label on the earlier
one. The check runs in `lower_enum` in `crates/ridl-sem/src/check.rs`, in the
same shape as `check_arm_projection`. It is in `ridl-sem` and not in a backend
for the reason ADR-0016 decision 5 gives: the transform is fixed by the family,
so the collision is a property of the package.

**A `reserved` value is not in the namespace.** A tombstone emits no variant, so
it cannot collide with one. This matches a union's `reserved` arm (ADR-0016,
2026-09-20 amendment). TYPL-210 already rejects a value that re-declares a
reserved name verbatim.

**Enumset bits are not in the namespace.** Their Rust spelling is the declared
name (§2), so no transform applies and no transform collision is possible.

### 5.2 One key, not two

A union's arms are checked under two transforms because the two collision sets
are incomparable (ADR-0016, 2026-09-20 amendment). An enum value reaches two
transformed namespaces as well — the Rust variant (`pascal_case`) and proto's
prefixed value (`snake_case` upper-cased). Here the collision sets are nested,
by property 4 of §3: every pair that collides under `snake_case` also collides
under `pascal_case`. So, **within one enum**, checking `pascal_case` alone
rejects every pair that either transform would break, and a second key would
find no new pair.

**Proto claims names RIDL-149 does not compare, and those stay with its backend
check.** proto3 scopes an enum's values as siblings of the enum, so the proto
backend claims every value in the file's package-wide namespace, and it adds a
`<PREFIX>_UNSPECIFIED` value to an enum that declares no zero. Two collisions
come from that. `enum A_B { C }` beside `enum A { B_C }` both give `A_B_C`,
across two enums. And a declared `UNSPECIFIED` value in an enum with no zero
meets the synthesized one, inside one enum but against a name that is not a
declared value. RIDL-149 compares the declared values of one enum, which is the
whole of the Rust namespace — a Rust variant is scoped by its enum, and the Rust
backend synthesizes no value — and not the whole of proto's. Both collisions
stay with the proto backend's `names.claim` refusal (ADR-0017 decision 4), as
they are today.

The TypeScript and FlatBuffers backends emit the declared name (§6), whose
collision set is exact equality. That case is a verbatim repeat, which is
driftsys/ridl#554's (§5.3).

### 5.3 A verbatim repeat is not a transform collision

`enum E { A = 0, A = 1 }` declares one name twice. Both values have the same
`pascal_case` output, but the transform did not cause that. Following
`check_arm_projection`, the check holds a verbatim repeat out of the projection
map rather than report it as RIDL-149. The missing exact-duplicate rule is the
enum sibling of TYPL-215 (struct fields), RIDL-413 (parameters) and
driftsys/ridl#452 (union arms), and is filed as driftsys/ridl#554. This design
does not close it.

### 5.4 The diagnostic

`Collision` in `crates/ridl-sem/src/check.rs` gains a fourth form,
`Pascal(String)`, and `colliding_projected_name` gains its message:

```text
error[RIDL-149]: `CHECK__ENGINE` and `CHECK_ENGINE` both become `CheckEngine`
under the pascal_case name transform, so a target whose namespace is PascalCase
would carry one identifier twice. Rename one of them (ridl §11, §16.4;
ADR-0016 decision 3)
  label on the earlier value: `CHECK_ENGINE` becomes `CheckEngine` here
```

The message names `pascal_case` and nothing else, and it is true of every input
that draws it. A pair that also collides under `snake_case` is still correctly
described: both names do become the stated identifier. The message does not
claim that the pair is distinct under `snake_case`. The remedy is the same
either way: rename one value.

### 5.5 What the new error rejects

The new error rejects two values of one enum that share a `pascal_case` output.
Some of those pairs were already refused by the proto backend, because they
share a `snake_case` output too — `PARK` beside `Park`, `checkEngine` beside
`CHECK_ENGINE`. The pairs that compiled on every backend before this change and
are refused after it are the ones whose `snake_case` outputs differ and whose
`pascal_case` outputs do not — `CHECK_ENGINE` beside `CHECK__ENGINE`, `A` beside
`A_`, `LEVEL_10` beside `LEVEL10` or `Level10`. No shorter description fits:
`A_B` beside `AB` differ only in an underscore and are accepted (`AB` and `Ab`).

Measured by computing `pascal_case` over the values of every `enum` in every
tracked `.ridl` and `.typl` file and every Markdown file under `docs/book/`: 59
enums, 182 values, and no two values of one enum share an output. The check is
expected to reject nothing that exists today, and the plan verifies it by
running the full suite, which compiles the book's fences through
`crates/ridl/tests/book_examples.rs`.

## 6. Every backend and runtime

| Consumer                                                     | Change                                                                                                             |
| ------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------ |
| `ridl-backend-rust` — `emit_enum`, `enum_default`, the codec | reads `Spellings.pascal`; the doc comment on `emit_enum` states the projection                                     |
| `ridl-backend-rust` — enumsets                               | none; each bit stays an associated constant in `SCREAMING_SNAKE`                                                   |
| `ridl-backend-rust` — the interaction face                   | none; the face's own enums spell variants from member names through `camel_case`, which this design does not touch |
| `ridlc-gen-rust`                                             | none of its own; it calls `ridl_backend_rust::Backend` and inherits the change                                     |
| `ridl-backend-proto`                                         | none; keeps `ENUMNAME_VALUE` (`snake_case` upper-cased, prefixed)                                                  |
| `ridl-backend-flatbuffers`                                   | none; keeps the declared spelling                                                                                  |
| `ridl-backend-ts`                                            | none; keeps the declared spelling                                                                                  |
| `ridl-ir` codegen model                                      | `Spellings.pascal` added                                                                                           |
| `ridl-sem`                                                   | RIDL-149 over one enum's values (§5)                                                                               |
| `ridl-rt`, `ridl-loopback`, `ridlc-gen-model`                | none; none of them spells a generated enum variant                                                                 |
| `examples/cabin`                                             | `api::Health::WARN` becomes `api::Health::Warn` in `consumer/src/main.rs`                                          |
| `docs/technotes/ridl-rt-by-example.md`                       | `Health::WARN` becomes `Health::Warn` in its Rust fence; the ridl fence keeps `WARN`                               |
| The tests that name a generated variant                      | updated with the snapshots (§2's list); the checked-in face is regenerated                                         |
| The book                                                     | none; no page shows a generated variant. The plan re-greps before it ends                                          |
| C header                                                     | none; ADR-0016 names `crates/ridl-backend-rust/src/c_header.rs`, but no such file exists in the workspace today    |

**Why the wire backends keep the typl spelling.** A wire schema's names are part
of what a peer in another language reads. For proto, the proto3 JSON mapping and
the text format carry an enum value by its name, so a rename changes what a peer
reads. The proto spelling is also already the proto convention (`HEALTH_WARN`).
For FlatBuffers, the schema's value names become constants in every
`flatc`-generated consumer, and `flatc`'s own Rust output spells them as
associated constants, where `SCREAMING_SNAKE` is correct. Neither backend has
the defect #506 reports.

**Why the TypeScript backend keeps the typl spelling.** TypeScript has no
compiler warning for an enum member's case, so #506's defect does not exist
there, and both `SCREAMING_SNAKE` and PascalCase members are common TypeScript
style. The TypeScript backend is reworked in step 2 of `docs/ROADMAP.md`, on the
codegen model, where `Spellings.pascal` is available if that work chooses it.
Changing it now would break TypeScript consumers for no defect.

## 7. Tests

The plan turns each of these into a task step.

- **`name.rs` unit tests.** The outputs in §3's table as pinned values; the
  containment property (4) over an enumeration of short names, as ADR-0016's
  measurement used; and property 2 checked on every enumerated output.
- **The `pascal` fallback.** A model whose `pascal` fields are empty generates
  the same Rust as one whose fields are filled.
- **`check.rs` tests.** A `pascal_case`-only collision is reported with the §5.4
  message; a verbatim repeat is not reported as RIDL-149; a `reserved` value
  does not collide; two distinct values that do not collide pass; the message
  names `pascal_case`.
- **The compile proofs deny `non_camel_case_types`.** All seven rustc proofs of
  §2 gain `-D non_camel_case_types` — the two `corpus.rs` proofs that build
  their own command included, because `veh-common` holds multi-word values — and
  their comments, with the two `corpus.rs` doc comments, are rewritten. #451
  found that a deny flag on a fixture with no multi-word name tests nothing, so
  the plan names the fixtures that hold a multi-word variant (`veh-cluster`
  holds 8) and proves the flag bites by reverting one emit site and watching the
  proof fail.
- **Snapshots move in the same commit as the emit change,** as #506's "Done
  when" asks.
- **`interaction_face.rs`** loses its `clippy::upper_case_acronyms` allowance,
  and `just lint` shows it is no longer needed.
- **`just demo`** runs `examples/cabin` against the new spelling.
- **Mutation.** Each site of §2, reverted alone to `declared`, fails at least
  one test. The run uses `--no-fail-fast`, for the reason #451 records.

## 8. Documents this change amends

| Document                                                        | When                    | Change                                                                                  |
| --------------------------------------------------------------- | ----------------------- | --------------------------------------------------------------------------------------- |
| ADR-0016                                                        | this design's PR        | the 2026-09-26 amendment: `pascal_case` pinned, enum values join RIDL-149's namespaces  |
| `docs/decisions/README.md`                                      | this design's PR        | ADR-0016's summary gains the amendment                                                  |
| `docs/specification/ridl-language-reference.md` §16.4, RIDL-149 | the implementation's PR | the row gains the values of one enum, checked under `pascal_case`                       |
| ADR-0017 decision 5                                             | the implementation's PR | its enum-value half is done within one enum; cross-enum collisions stay with decision 4 |
| ADR-0017 "Alternatives considered", the RIDL-149 row            | the implementation's PR | "enum values still deferred" becomes taken, with the same scope                         |
| `docs/technotes/ridl-rt-by-example.md`                          | the implementation's PR | the Rust fence's `Health::WARN` becomes `Health::Warn`                                  |
| `model.proto` comment on `Spellings`                            | the implementation's PR | the `pascal` field, citing ADR-0016's 2026-09-26 amendment                              |

The language reference and ADR-0017 describe what is checked, so they change in
the commit that adds the check, not before it (ADR-0016 decision 4 asks for the
rule and its application to change together).

## 9. Alternatives considered

| Candidate                                                                       | Verdict  | Reason                                                                                                                                                                                                                                                             |
| ------------------------------------------------------------------------------- | -------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Option A — `#[allow(non_camel_case_types)]` on each generated enum              | rejected | the maintainer chose B on 2026-09-26; A leaves enum variants as the one generated identifier that does not follow the Rust convention, which is what #243 fixed for fields                                                                                         |
| Reuse `camel_case` (`Spellings.camel`)                                          | rejected | it gives `CHECKENGINE`, which silences the lint but is not PascalCase, and still draws `clippy::upper_case_acronyms`                                                                                                                                               |
| Change `camel_case` to lower-case each segment's tail                           | rejected | it would move names `camel_case` already projects — `HTTPServer` as a union arm gives `HTTPServer` today and would give `Httpserver` — a second breaking change with no defect behind it                                                                           |
| A transform for `SCREAMING_SNAKE` only (split on `_`, lower-case each tail)     | rejected | on a value outside the convention it gives a worse name (`checkEngine` → `Checkengine`), and its collision set does not contain `snake_case`'s (`checkEngine` and `check_engine` differ under it and collide under `snake_case`), so the check would need two keys |
| Key RIDL-149 on `pascal_case` and `snake_case` both, as for union arms          | rejected | the `pascal_case` collision set contains `snake_case`'s (§3 property 4), so the second key finds no new pair; it adds code and a message form that no input can reach alone                                                                                        |
| A backend-local check in the Rust backend, as proto's `names.claim`             | rejected | the transform is fixed by the family, not chosen by a target, so the collision is a property of the package (ADR-0016 decision 5); a backend check would let `ridlc check` pass a package `ridl build` then refuses                                                |
| Compute `pascal_case` in the Rust backend instead of carrying it in `Spellings` | rejected | the codegen model design's D-2 carries every pinned spelling so that a plugin does not reimplement a transform (ADR-0020 decision 8); the Rust backend reads only the model since stage P4                                                                         |
| Project the TypeScript and FlatBuffers backends too                             | rejected | neither has the defect; FlatBuffers value names reach every `flatc` consumer, and the TypeScript backend is reworked in step 2 (§6)                                                                                                                                |
| Project enumset bits too                                                        | rejected | a bit is emitted as an associated constant, where `SCREAMING_SNAKE` is the Rust convention                                                                                                                                                                         |
| Fold #554 (the verbatim repeat) into this change                                | rejected | it is a different rule with a different diagnostic, the sibling of #452; keeping it separate keeps RIDL-149's meaning — names distinct in source that collide after a transform                                                                                    |

## 10. Decisions taken on the maintainer's behalf

These are recorded on driftsys/ridl#506 for review.

1. The transform is `camel_case ∘ snake_case`, not a new algorithm (§3).
2. RIDL-149 is keyed on `pascal_case` alone for enum values (§5.2).
3. The TypeScript, proto and FlatBuffers backends keep their current spelling
   (§6).
4. `Spellings` gains `pascal` rather than the Rust backend computing it (§4).
5. The verbatim repeat is left to a new issue, driftsys/ridl#554 (§5.3).
6. The language reference and ADR-0017 change in the implementation's PR, and
   ADR-0016 in this one (§8).
7. An empty `Spellings.pascal` is defined as `camel_case(snake)`, so the new
   field is additive under the IR stability rule (§4).
