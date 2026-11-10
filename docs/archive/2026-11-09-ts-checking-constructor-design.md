# The TypeScript checking constructor — design note

Status: approved by the owner on 2026-11-09, with every recommendation of
section 7 accepted. Implementation follows the tasks of section 6. Section 2.2
was corrected after the first task's review.

Satisfies: ADR-0013 decision 8 (a language backend emits a constructor that
checks a type's bounds, and one that does not, for the decoder).

Related: driftsys/ridl#923 (this work), task A2 of
[the TypeScript lane driver](../wip/2026-11-08-typescript-lane-driver.md)
(section 2, decision 2), ADR-0020 decision 7 (the codec is the generated Rust
compiled to wasm), D-11 and D-13 of
[the codec table record](../design/ts-codec-table.md), driftsys/ridl#597
(pattern meaning differs between engines), driftsys/ridl#926 (whether a widened
constraint is compatible for a receiver), typl reference §2.7, §4.3, §5.1 to
§5.5 and §5.7.

## 1. The problem

`ridl-backend-ts` emits a constrained named scalar as a branded type:

```ts
/** @range [-40..85] */
export type Temperature = number & { readonly __ridl: "veh.cabin.Temperature" };
```

The only way to get a `Temperature` today is a type assertion
(`20 as Temperature`). Nothing checks the range, the step, the length or the
pattern when a TypeScript program builds the value. The wasm codec's `verify`
checks the payload later, when it is received or when a stream function sends
it, and `encode<Name>` builds whatever it is given (D-11). So a TypeScript
program can hold a value outside its typl contract with no error until a peer
refuses it.

The Rust backend emits `new`, which checks and returns an error, and
`new_unchecked`, which does not. The TypeScript backend should emit the same
pair.

## 2. Decisions

### 2.1 The emitted shape

For every named scalar the domain module `<base>.ts` emits, the module also
emits a free function next to the type:

```ts
/**
 * Builds a `Temperature`. Throws a `ConstraintError` when `value` breaks
 * the range [-40..85].
 */
export function makeTemperature(value: number): Temperature {
  if (!Number.isFinite(value)) throw rangeError("veh.cabin.Temperature", value);
  if (value < -40) throw rangeError("veh.cabin.Temperature", value);
  if (value > 85) throw rangeError("veh.cabin.Temperature", value);
  return value as Temperature;
}

/**
 * Builds a `Temperature` without a check, for a value already known to
 * satisfy its constraints, such as a value the codec has verified.
 */
export function makeTemperatureUnchecked(value: number): Temperature {
  return value as Temperature;
}
```

(The exact body is the implementer's; the example shows the contract.)

- `make<Name>(value: <base>): <Name>` checks and throws. It is emitted for every
  named scalar, constrained or not. For a type with no constraint it checks only
  what the backing requires (section 2.2) and is otherwise the brand conversion.
  This mirrors Rust, where a type with no constraint still has `new`.
- `make<Name>Unchecked(value: <base>): <Name>` does not check. It is emitted
  only for a type that `make<Name>` checks something for, as Rust emits
  `new_unchecked` only for a constrained type.
- Both functions follow the visibility of the type: exported for a public type,
  module-local for an `internal` one (ADR-0002 §8), as `init<Name>` does.
- The type itself does not change. The brand stays a structural intersection
  with a `__ridl` marker property.

**How it fits `flatc` and structural types.** `flatc`'s object API owns the
classes (`<Name>T`, `<Name>Box`), and the domain module's types are plain
structural types: interfaces, unions and branded primitives. A free function
returns a branded primitive and creates no class, so the plain-object model
(D-4) and the converters in `<base>.codec.ts` do not change shape. A value built
by `make<Name>` is a plain `number`, `bigint`, `string` or `Uint8Array` at run
time, so it is passed to `flatc`'s object API exactly as today.

**Why a free function named `make<Name>`.** typl declaration names are CamelCase
(typl §15.1), so a name that starts with a lower-case `make` cannot equal a
declared name, and `init<Name>` already sets the pattern of a lower-case prefix.
A companion value with the type's name (`Temperature.new`) was considered
(section 4); it does not tree-shake and gives a type and a value the same name,
which a reader of the generated code can confuse.

**The brand can still be asserted.** TypeScript cannot forbid
`x as
Temperature`. ADR-0013 decision 8 says the value "can be built only
through a constructor"; in TypeScript that holds for code that does not use a
type assertion, which is the language's limit, not this design's. The doc
comment of the type says to build it with `make<Name>`. A lint against
assertions is the consumer's choice.

### 2.2 Which constraints are checked

`make<Name>` checks, in this order, and stops at the first violation (the order
of the Rust `check`):

| Constraint                   | Backing                       | Check                                                                                                                                                                                                                                                                                                                                                     | `rule`    |
| ---------------------------- | ----------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------- |
| integrality (no typl syntax) | integer as `number`           | `Number.isSafeInteger(value)`; fractions, NaN, infinities and values outside the safe integer interval [-(2^53-1)..2^53-1] are refused. Checked for every number-backed integer, constrained or not; bigint bounds are checked separately below                                                                                                           | `range`   |
| finiteness                   | float with a `min` or a `max` | `Number.isFinite(value)`, as Rust's `is_finite` check                                                                                                                                                                                                                                                                                                     | `range`   |
| range minimum                | integer, float                | `value < min` is refused. A bigint always checks the effective minimum: the declared minimum intersected with -9223372036854775808 for I64 or 0 for U64, using an `n` literal; an omitted minimum uses that domain limit                                                                                                                                  | `range`   |
| range maximum                | integer, float                | `value > max` is refused. A bigint always checks the effective maximum: the declared maximum intersected with 9223372036854775807, using an `n` literal; an omitted maximum uses that domain limit. Rust can omit the i64::MAX comparison because its parameter is i64; TypeScript bigint has no such limit. A safe number integer cannot exceed i64::MAX | `range`   |
| step, integer                | integer                       | `(value - origin) % step !== 0` is refused; `origin` is `min`, or 0 when no minimum is declared. The arithmetic is in `bigint` for a 64-bit width and in `number` otherwise (both operands are safe integers after the integrality check)                                                                                                                 | `step`    |
| step, float                  | float                         | the representation rule of typl §4.3, at the wire precision: for a 32-bit float width the value is projected with `Math.fround` first. The compiler prepares the phase and step as a high part and a low part, as `scalar_step::float_invalid` does for Rust                                                                                              | `step`    |
| length minimum and maximum   | string                        | the number of Unicode code points, counted by `codePointLength` of `@driftsys/ridl-runtime`, not `value.length`, which counts UTF-16 code units. typl counts characters (§5.3) and Rust counts `chars()`                                                                                                                                                  | `length`  |
| length minimum and maximum   | bytes                         | `value.length` of the `Uint8Array`                                                                                                                                                                                                                                                                                                                        | `length`  |
| pattern                      | string                        | `PATTERN.test(value)` with a module-level `RegExp` built once from the IR's pattern source, without the `u` flag. On by default (section 2.3)                                                                                                                                                                                                             | `pattern` |

**Open ranges.** A bigint checks both effective bounds even when a declared
bound is omitted or the type has no range. Other backings check only declared
bounds, with the number integrality and float finiteness checks above. An
integer step with no `min` uses origin 0 (typl §4.3).

**Not checked by the constructor:** unit (no run-time meaning), collection
bounds and map-key uniqueness (these belong to fields, see section 2.5), and
`require` and `ensure` clauses (interaction rules, not type rules). The table in
the book chapter keeps saying so.

**The error.** A violation throws `ConstraintError`, a class that extends
`RangeError` and carries:

- `rule`: one of `"range"`, `"step"`, `"length"`, `"pattern"` — the same words
  as the runtime's `PayloadRule` and the codec's `RULES`;
- `type`: the qualified name of the type, such as `"veh.cabin.Temperature"`;
- `message`: `<type>: <value> breaks the <rule> rule <constraint text>`, for
  example `veh.cabin.Temperature: 90 breaks the range rule [-40..85]`. The value
  is written with `String(value)`; a string value longer than 64 code points is
  cut to 64 and marked with `…`, so that a large string does not fill a log, and
  a `Uint8Array` is written as its length only (`<n bytes>`).

Throwing, and not returning a result, matches the module's existing failures
(`RuleError` in the codec, `RangeError` for an absent field) and the way
TypeScript code reports a precondition failure.

**Where the class lives.** Recommended: `ConstraintError` is exported by
`@driftsys/ridl-runtime` and imported by each domain module that has a
`make<Name>` with a check. One class means one `instanceof` test for every
package. The cost is that a domain module with a constrained type now has one
import, where today it has none except other domain modules. This is open
question 1.

### 2.3 The pattern check is on by default

ADR-0013 decision 8 allows the pattern check to be optional, because Rust needs
a regular-expression engine in the consumer's build. TypeScript has `RegExp` in
every runtime, so that reason does not apply, and the check is emitted always,
with no option.

The check uses ECMA-262 semantics without the `u` flag. That is the semantics of
the checker (typl §2.7), so a value the checker accepts in a declared init is
accepted by `make<Name>`. It is not the semantics of the Rust `regex` crate,
which the wasm codec uses. For the constructs in the §2.7 table (`\d`, `\w`,
`\p{L}`, `\A`, `\z`, `\a`, `\<`, `[a&&b]`, `[[a]]`), `make<Name>` and the
codec's `verify` can disagree. That is driftsys/ridl#597, which exists today
between the checker and Rust; this design does not widen it, and the TypeScript
constructor follows the checker. Open question 2 asks whether to add an option
to turn the check off.

### 2.4 Relation to the wasm codec's check

There are two places a constraint is checked, and each has one job:

| Where                                     | When                                                        | What it checks                                                                                                | Engine                     |
| ----------------------------------------- | ----------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------- | -------------------------- |
| `make<Name>` in `<base>.ts`               | when TypeScript code builds a value                         | the named scalar's range, step, length, pattern                                                               | TypeScript, as section 2.2 |
| the wasm codec's `verify` (`ridl_verify`) | when bytes are received, and before a stream function sends | every rule of the payload: named and inline field constraints, collection bounds, uniqueness, UTF-8, variants | the generated Rust in wasm |

- `decode<Name>` in `<base>.codec.ts` is for bytes the codec has verified
  (D-11). It builds a constrained named scalar with `t.make<Name>Unchecked`
  where it writes `as t.<Name>` today, and keeps the type assertion for a type
  whose `make<Name>` checks nothing (`make<Name>Unchecked` is emitted for every
  type whose `make<Name>` checks anything, and an integer backing always checks
  integrality). No value is checked twice on the receive path.
- `encode<Name>` keeps D-11: it does not check, so that a test can build a
  payload the codec refuses. A value that came through `make<Name>` already
  satisfies its named-type rules; the rules `make<Name>` does not cover are
  still the codec's.
- The constructor does not call into wasm. A domain module must work with no
  wasm module loaded (a program that only builds values, or a test). The cost is
  two implementations of the same rule, kept equal by the differential test in
  section 3.

ADR-0020 decision 7 places the codec's checks in the generated Rust. The codec
table design rejected a checking `encode<Name>` because it would duplicate those
checks inside the codec. A constructor in the domain module is a different place
with a different purpose (ADR-0013 decision 8): it refuses a value when it is
built, not when it is encoded. The two decisions do not conflict, and the note
to ADR-0020 is not needed; D-11 stays as written.

### 2.5 Struct fields, collections and nested types

- **No struct constructor.** A struct stays an `interface`. A field whose type
  is a named scalar can only hold a value of that brand, which comes from
  `make<Name>` (or an assertion), so the field is checked when its value is
  built. This is the Rust model: a Rust struct has no checking constructor, its
  fields are typed by the named scalars.
- **Inline constraints are not checked at construction.** A field whose type is
  written inline (`speed: float [0..300]`) has no named type and therefore no
  constructor, in Rust and in TypeScript. Its rule is checked by the codec's
  `verify`. The book table says so.
- **Collections.** Array and map bounds, and map-key uniqueness, are not checked
  at construction, as in Rust (Rust checks them in `encode` and `verify`). An
  array of a named scalar holds values built by `make<Name>`.
- **A named scalar over a named scalar** (a type whose backing is another
  declared type) checks its own constraint, as lowered into its `TypeDef`; the
  IR has already resolved the effective constraint.
- **Init functions.** `init<Name>` keeps its type assertion: the init value is
  checked by the checker against the constraint at compile time (TYPL-109 for a
  pattern; range and step checks for numbers).
- **Constants** of a named type keep their literal form; the checker has checked
  them.

### 2.6 What changes in generated output, and whether it is breaking

Changed output:

- `<base>.ts`: new `make<Name>` and `make<Name>Unchecked` functions, a pattern
  constant per pattern-checked type (module-local), and an import of
  `ConstraintError` (if open question 1 keeps it in the runtime package) or one
  emitted class.
- `<base>.codec.ts`: `decode<Name>` and the private `unpack<Name>` converters
  call `t.make<Name>Unchecked` or `t.make<Name>` instead of `as t.<Name>`.
- The doc comment of a named scalar gains one sentence that names its
  constructor.

Breaking assessment:

- Every change is an addition for a consumer that compiles against the module.
  The types do not change; an assertion still compiles; the codec functions keep
  their signatures.
- The one source of a break is a name: a declared type named `ConstraintError`
  collides with the imported or emitted class. The backend refuses it, as the
  codec file refuses its reserved names, with an error that names both. If the
  class is imported under a namespace alias, that collision does not occur, and
  the alias joins the reserved names instead (open question 1).
- A consumer whose package now needs `@driftsys/ridl-runtime` and did not have
  it in its import map gets an unresolved import. That is a break of the build
  setup, not of the API, and is the reason to put A2 in the second release, as
  the owner plans.

Frozen surfaces and `ridl diff`:

- The TypeScript emit and `@driftsys/ridl-runtime` are not frozen surfaces
  (codec table record, "Frozen surfaces"). No dump under `freeze/` changes, and
  `freeze/surfaces.toml` does not change. If `ConstraintError` is added to the
  runtime, it is an addition to an unfrozen package.
- `ridl diff` does not change. Its `constraint_changed` rule already classifies
  a narrowed constraint as breaking; with this work the TypeScript output
  enforces the constraint, so the classification now has an effect in TypeScript
  code too. Whether a widened constraint is compatible for a receiver stays
  driftsys/ridl#926.

## 3. Test plan

Unit and snapshot tests in `crates/ridl-backend-ts`:

- One `insta` snapshot per backing and constraint form: integer `number` (range,
  open range, step), integer `bigint` (range with an `i64::MAX` maximum, step),
  float 64-bit and 32-bit (range, step), string (length, pattern), bytes
  (length), an unconstrained integer and an unconstrained string, an `internal`
  constrained type (functions not exported).
- The name collision with `ConstraintError` is refused with an error that names
  both sources.
- The existing snapshots are updated in the same change; a reviewer reads the
  diff of every one.

A new fixture `crates/ridl-backend-ts/tests/fixtures/ctor/` with a `.ridl`
package that declares one type per row of the table in section 2.2, and
`ctor_test.ts`, run by a new block of `_codec-ts-check` (`deno lint`,
`deno check`, `deno test`):

- for each type: a value at each bound passes; the value one step past each
  bound throws a `ConstraintError` with the expected `rule` and `type`; for the
  step rule, an on-grid and an off-grid value; for a 32-bit float, a value that
  is on the grid only after `Math.fround`;
- string length counts a code point outside the Basic Multilingual Plane
  (`"😀"`) as one character;
- NaN, `Infinity`, `1.5` and `2 ** 53` are refused for an integer backing;
- `make<Name>Unchecked` returns an out-of-range value unchanged.

The differential test, in `examples/cabin/web/check_test.ts`, which already
loads the wasm codec: for a list of values per constrained type of the cabin
package (each bound, one past each bound, off-grid values, NaN), the outcome of
`make<Name>` equals the outcome of `verify` over the encoded payload. This is
the test that keeps the TypeScript and Rust checks equal. Cabin needs a type
with a step and a string with a length bound for it to cover those rules; the
task adds them if they are missing (open question 4 asks whether to put the
differential test on a new fixture with its own wasm build instead).

The decoder change is tested in the existing codec fixtures: a round trip of a
constrained value gives back the same value, and a `deno check` of the generated
codec file shows that it calls `make<Name>Unchecked`.

Mutation checks, for the task reviewer: remove each emitted comparison in turn
(minimum, maximum, finiteness, integrality, step, length, pattern) from the
generated module in the fixture build and confirm `ctor_test.ts` fails for each
one; replace the code-point count with `value.length` and confirm the
astral-character test fails; drop the `Math.fround` projection and confirm the
32-bit float test fails.

## 4. Alternatives considered

| Alternative                                                                                  | Why not chosen                                                                                                                                                                                                                                            |
| -------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| A class per named scalar (`new Temperature(20)`, private constructor plus static `of`)       | A boxed value is not a `number`: arithmetic, comparison and JSON need `.value`; `flatc`'s object API and the converters take primitives, so every field would need unboxing; it breaks every consumer that uses the branded primitive today               |
| A companion object with the type's name (`Temperature.new(20)`, `Temperature.unchecked(20)`) | Reads closest to Rust, but one name is both a type and a value, the object does not tree-shake, and it is a new pattern in a module whose other helpers (`init<Name>`) are free functions. It is the second choice if the owner prefers the Rust spelling |
| A result-returning check (`{ ok: true, value } \| { ok: false, error }`)                     | Not how the module reports other failures; every caller must unwrap. A caller that wants a boolean can catch, and a non-throwing `is<Name>(value): value is <Name>` guard can be added later without changing this design (open question 3)               |
| A `unique symbol` brand so that an assertion is harder to write                              | An assertion through `unknown` still compiles, so it buys little, and it changes every existing type, which is a break for no gain in checking                                                                                                            |
| A constructor that calls the wasm codec's check                                              | The domain module would need a loaded wasm module to build any value, and the codec checks payloads, not single scalars, so it would need a new export per type. Rejected; the differential test keeps the two implementations equal instead              |
| A pattern check behind an option, off by default                                             | The reason for Rust's feature, the size of a regex engine, does not apply to TypeScript                                                                                                                                                                   |
| A checking struct constructor                                                                | Rust has none; named-scalar fields are already checked by their own constructors, and inline field constraints and collection bounds are the codec's in both languages                                                                                    |
| Leave TypeScript unchecked and amend ADR-0013 decision 8 to exempt it                        | The lane's exit criterion and #923 ask for the check; `ridl diff`'s breaking classification of a narrowed constraint would have nothing to enforce it in TypeScript                                                                                       |

## 5. Documents to change, with the code

- ADR-0013 decision 8, "As built": replace the sentence on `ridl-backend-ts`
  with: `ridl-backend-ts` emits `make<Name>`, which checks the range, step,
  length and pattern and throws a `ConstraintError`, and `make<Name>Unchecked`,
  which does not; `decode<Name>` uses the second. The pattern check is always
  on, with the checker's ECMA-262 semantics.
- `docs/book/generated-code.md`: the TypeScript column of "What the generated
  code checks" (range, step, length and pattern: "checked by `make<Name>`"; the
  rest unchanged); the paragraph that says the domain module does not validate
  values; a short section under "TypeScript" with one example of `make<Name>`,
  the error, and when to use `make<Name>Unchecked`. A TypeScript block in the
  book is not compiled, so the example is checked by reading against the
  fixture.
- `docs/design/ts-codec-table.md`: the conversion table row "named scalar as a
  field" and "as a payload" (the conversion is `make<Name>Unchecked`, not a type
  assertion); the paragraph after the table, whose last sentence names
  driftsys/ridl#923, says what the constructor checks and that the decoder does
  not check twice; "Frozen surfaces" if `ConstraintError` is added to the
  runtime.
- The typl reference §2.7: one sentence that the TypeScript backend checks a
  pattern with the checker's semantics.
- `packages/ridl-runtime/README.md` and its changelog entry, if the class lives
  there.

## 6. Tasks

Each task is one pull request with one implementer; each depends on the one
before it.

1. **Error class and range.** `ConstraintError` (in the runtime package or the
   module, per open question 1); `make<Name>` and `make<Name>Unchecked` for
   integer (`number` and `bigint`) and float backings with the integrality,
   finiteness, minimum and maximum checks; the name refusal; snapshots; the
   `ctor` fixture and its `_codec-ts-check` block with the range cases.
2. **Step.** The integer step; the float step with the typl §4.3 rule and the
   `Math.fround` projection. The compile-time preparation of the phase and step
   (exact decimal to a high and a low binary64 part) is shared with
   `ridl-backend-rust`'s `scalar_step` rather than copied (open question 5).
   Fixture cases for each.
3. **Length and pattern.** String length by code points, bytes length, the
   pattern constant and check; fixture cases.
4. **The decoder.** `<base>.codec.ts` calls `make<Name>Unchecked` or
   `make<Name>`; codec fixtures updated; the differential test in the cabin web
   check.
5. **Documents.** The changes of section 5, and the archive of this note.

## 7. Open questions for the owner

The owner accepted the recommendation of each question below on 2026-11-09:
`ConstraintError` lives in `@driftsys/ridl-runtime`; no option turns off the
pattern check; the `is<Name>` guard waits; the differential test runs in the
cabin web check; the float step preparation is shared with `ridl-backend-rust`;
`make<Name>` refuses a non-integer or unsafe integer under rule `range`; the
names are `make<Name>` and `make<Name>Unchecked`; this work stays in the second
release.

1. **Where `ConstraintError` lives.** Recommended: in `@driftsys/ridl-runtime`,
   imported by a domain module only when it has a check (one class for every
   package; adds a dependency to domain modules). The other choice is one class
   emitted per module (no import; `instanceof` works per package only, and a
   declared type named `ConstraintError` must be refused). Should the codec
   file's `RuleError` then become the same class?
2. **An option to turn off the pattern check.** Recommended: no option; the
   check is always on and follows the checker's semantics, with the
   driftsys/ridl#597 differences recorded. Say if a `[backend.typescript]` key
   is wanted.
3. **A non-throwing guard.** Should `is<Name>(value): value is <Name>` be
   emitted too, or left for later?
4. **Where the differential test runs.** Recommended: the cabin web check, which
   already loads wasm, after adding the missing constraint forms to cabin. The
   other choice is a new fixture with its own wasm build, which costs a longer
   recipe.
5. **Sharing the float step preparation.** Recommended: move the compile-time
   preparation from `ridl-backend-rust`'s `scalar_step` to a function both
   backends call (for example in `ridl-ir`'s codegen module), so that the two
   backends cannot prepare different constants. Copying it into
   `ridl-backend-ts` is smaller but duplicates a numerics routine.
6. **Integrality of an unconstrained integer.** Recommended: `make<Name>`
   refuses a non-integer or unsafe `number` for every integer-backed type, with
   rule `range`, because the Rust side cannot hold it. This makes `make<Name>`
   throw for a type that Rust's `new` accepts for every value; confirm, and
   confirm the rule name.
7. **Naming.** `make<Name>` and `make<Name>Unchecked`, or the companion object
   `<Name>.new` and `<Name>.unchecked`?
8. **The release.** The note finds A2 additive for the API and breaking only for
   a declared type named `ConstraintError` and for an import map without the
   runtime package. Keep it in the second release as planned?
