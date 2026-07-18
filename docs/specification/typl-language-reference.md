# typl Language Reference

**Type Language** — the vocabulary layer of the RIDL family: types, ranges,
units, constants, and composites, with package namespacing. typl is the root of
the family lattice — every other profile (`ridl`, `uxdl`, `rmdl`, `rsdl`) builds
on the vocabulary typl declares.

Version: 0.1.0 — Draft

> **Provenance.** This reference re-frames Language Reference §1–§11 of the RIDL
> Language Reference as the named foundation layer (per the RIDL family concept
> note, §7.1 Stage 1), incorporates the module system of ADR-0002, and mines the
> preliminary **markspec typl** DSL as prior work (see Appendix F). Where this
> document and the RIDL Language Reference v0.1 overlap, this document is
> authoritative for the vocabulary layer; the RIDL Language Reference retains
> authority for interactions.

---

## Table of Contents

1. [Scope and Position in the Family](#1-scope-and-position-in-the-family)
2. [Lexical Conventions](#2-lexical-conventions)
3. [Packages, Imports, and Visibility](#3-packages-imports-and-visibility)
4. [Primitives](#4-primitives)
5. [Type Definitions](#5-type-definitions)
6. [Constants](#6-constants)
7. [Structs](#7-structs)
8. [Enums](#8-enums)
9. [Enum Sets](#9-enum-sets)
10. [Unions](#10-unions)
11. [Tuples](#11-tuples)
12. [Collections](#12-collections)
13. [Comments](#13-comments)
14. [Doc Comments](#14-doc-comments)
15. [Conventions](#15-conventions)
16. [Diagnostics](#16-diagnostics)
17. [Open Questions](#17-open-questions)

- [Appendix A — Standard Library](#appendix-a--standard-library)
- [Appendix B — Full Example](#appendix-b--full-example)
- [Appendix C — Standards References](#appendix-c--standards-references)
- [Appendix D — Codegen Targets](#appendix-d--codegen-targets)
- [Appendix E — Formal Grammar (EBNF)](#appendix-e--formal-grammar-ebnf)
- [Appendix F — Prior Work: markspec typl](#appendix-f--prior-work-markspec-typl)
- [Appendix G — Coverage Analysis: JSON Schema and Other Schema Languages](#appendix-g--coverage-analysis-json-schema-and-other-schema-languages)
- [Appendix H — Glossary](#appendix-h--glossary)

---

## 1. Scope and Position in the Family

### 1.1 What typl is

typl is a **units-aware, range-constrained schema language**. It answers one
question: _what is the shared vocabulary?_ — the types, physical units, ranges,
constants, and composite shapes that every other concern in a component-based
reactive system describes things _with_.

typl is the only truly standalone member of the RIDL family. A pure typl package
is useful by itself: it generates data types, validators, and documentation
across every backend without any interface, behaviour, or deployment content.
Every other profile requires typl; typl requires nothing above it.

The name is deliberately `-L`, not `-DL`: typl does not _describe_ a system
aspect — it is the vocabulary the describing languages are written in.

### 1.2 The `.typl` profile

There is one grammar for the whole RIDL family; each language is a **profile** —
a restriction of that grammar selected by file extension. A `.typl` file accepts
**type declarations only**:

| Accepted in `.typl`                   | Rejected in `.typl` (belongs to)                                   |
| ------------------------------------- | ------------------------------------------------------------------ |
| `package`, `import`, `as`, `internal` | `interface`, `signal`, `event`, `command`, `query`, `final` (ridl) |
| `type`, `const`                       | user-interaction declarations (uxdl)                               |
| `struct`, `enum`, `enumset`, `union`  | `model`, `node`, behaviour operators (rmdl)                        |
| tuples, collections, `?` optionality  | instances, wiring, deployment (rsdl)                               |
| doc comments, `@labels`               | timing annotations `@Xms`, `@[min..max]` (time core)               |
|                                       | streams `<T>` (ridl — see §1.3)                                    |

typl declarations are also legal inside every other profile's files and inside
`.rxdl` (the total profile) — a `.ridl` file may declare the types its interface
uses. Profile purity is a **policy decision** declared per package in
`ridl.toml`, not a grammar limitation (concept note §4).

### 1.3 Profile boundary decisions

Three boundary calls are recorded here because they were genuinely undecided:

**Streams belong to ridl, not typl.** A stream `<T>` is not a data shape — it
has no bound, its framing is transport-decided, and it is only valid in
interaction position (`command`/`query` parameters and returns). That makes it
an _interaction container_, not a vocabulary item. typl owns the **element
type** (`type FwBlock : bytes [1..65536]`); ridl owns the `<T>` container. A
`<T>` appearing in a `.typl` file is a profile error (TYPL-301).

**Timing is not typl.** Duration literals (`10ms`, `@[20ms..1s]`) belong to the
family's `time` core, consumed by ridl/uxdl/rmdl. The family lexer recognises
duration tokens everywhere (one lexer, one registry), but no typl construct
accepts them. The stdlib `Duration` _type_ (a UCUM `ms` unit type) is unaffected
— it is an ordinary unit type.

**Interaction kinds are not typl kinds.** The markspec-typl precursor attached
kinds (`signal`, `event`, `command`, `state`, `stream`, `config`, `document`) to
type bindings. In the family these are split by layer:
`signal`/`event`/`command`/`query`/`final` are ridl's interaction keywords over
the `interact` core; `state` and stream processing are rmdl territory; `config`
maps to ridl `final` (provisioned constants); `document` is just a `struct`.
typl keeps only the pure vocabulary kinds: `type`, `const`, and the composites.
See Appendix F.

### 1.4 Keyword discipline

The family maintains **one reserved-word registry across all profiles** (concept
note §4.1). Every family keyword is reserved in every profile, including
keywords a profile does not accept — `signal` is not a valid identifier in a
`.typl` file even though `.typl` rejects signal declarations. This keeps `.rxdl`
unambiguous and keeps identifiers portable across layers.

Keywords **used** by the typl profile:

```
package  import  as  internal
type  const  struct  enum  enumset  union
boolean  integer  float  string  bytes
true  false  step  match  reserved  error
```

**Init values use bare `= value`** (§5.8), not a keyword —
`type Speed : km/h [0.0..250.0 step 0.5] = 0.0`, the universal default-value
idiom. There is **no `default` keyword** (retired) and **no `init` keyword** in
typl (`init` is rmdl's, where two equations per flow must be disambiguated). The
**`wire` clause and explicit width names** (`uint8`…`float64`) are **deferred to
v0.1's open questions** (§17.11): range-first inference plus `ridl-diff`
breaking-change detection cover the common case, and forward-compat
width-pinning is a niche refinement not worth ten keywords in v0.1.

Keywords **reserved family-wide** but rejected by `.typl` (current registry —
grows with the other profiles): ridl's `interface`, `service`, `signal`,
`event`, `command`, `query`, `final`; uxdl's `view`, `display`, `input`,
`action`, `activate`, `toggle`, `select`, `adjust`, `dismiss`, `fetch`, `fixed`,
`states`, `during`, plus its reserved set `navigate`, `scroll`, `drag`,
`observe`, `surface`, `agent`; the expr-core words `require`, `ensure`; rmdl's
`model`, `function`, `let`, `init`, `last`, `case`, `if`, `then`, `else`,
`when`, `emit` (it also surfaces `signal`/`event` in signatures — same concepts,
one registry entry each; `init` is rmdl's alone — the memory-seed keyword,
needed because a flow has two equations; typl/ridl express init as bare
`= value`; its ambient time values `now`/`dt` are _contextual identifiers, not
keywords_; `pre`, `->`-as-followed-by, `node`, `returns`, `realizes`, and a
surface `step` considered and rejected — models are contract-blind, so no
`realizes`; `step` remains typl's quantization keyword alone), plus its reserved
set `merge`, `current`, `state`, `transition`, `automaton`; and rsdl's
`component`, `system`, `deployment`, `provides`, `requires`, `instance`, `for`,
`assurance`, `target`, `place`, `on`, `transport`, `bundle`, `time`, `base` (it
also reuses `model` from rmdl,
`interface`/`service`/`signal`/`event`/`command`/`query` from ridl — same
concepts, one registry entry each; `composition`, `binding`, `wire`, `delegate`,
`publish`, `spk`, `apk` considered and rejected — components use application
notation and inline `provides`/`requires`), plus its reserved resilience set
`redundant`, `supervise`, `degraded`. (`node` and `returns` were considered and
rejected by rmdl — never reserved.) The per-profile keyword sections of each
language reference enumerate their own additions; the union of those sections
**is** the registry until the platform spec extracts it as a standalone
normative list.

**Registry admission test — language, never runtime** (family doctrine, audit
passed). Every registry entry names a _describable property_ — a shape, an
interaction, a requirement, an availability, an occurrence relation — never an
_execution mechanism_. Steps, scheduling, acks, retries, async/await,
quarantine, and clock domains are runtime vocabulary: they appear in semantics
prose and runtime specs, never as surface syntax. Borderline entries carry their
justification where declared (`now`/`dt`: logical time is contract-constrainable
— rmdl §6.3). A future `wire` clause (deferred, §17.11) would clear the same bar
— wire ABI is contract, not execution. Rejected for leaking: `throws`, a surface
`step`, library `time()`.

---

## 2. Lexical Conventions

The family has one lexer; these conventions bind family-wide. They are restated
here because typl is the foundation document.

### 2.1 Encoding

Source files are encoded in **UTF-8**
([Unicode 15.0](https://www.unicode.org/versions/Unicode15.0.0/),
[RFC 3629](https://www.rfc-editor.org/rfc/rfc3629)). Non-ASCII characters are
permitted only inside string literals and doc comments.

### 2.2 Whitespace, Line Endings, Separators

Whitespace (space, tab, CR, LF) is insignificant except as a token separator.
`LF` and `CRLF` are both accepted. **Newline and comma are interchangeable
separators** in all block constructs; trailing comma is permitted. **There are
no semicolons** — the ADR-0002 example `package veh.common.types;` is errata;
the grammar has none (concept note §4.1 errata).

### 2.3 Identifiers

Three ASCII identifier conventions:

| Form              | Pattern                          | Used for                                |
| ----------------- | -------------------------------- | --------------------------------------- |
| `CamelCase`       | starts uppercase, no separators  | types, structs, enums, enumsets, unions |
| `camelCase`       | starts lowercase, no separators  | fields, tuple fields                    |
| `SCREAMING_SNAKE` | uppercase, underscore separators | enum values, enumset bits, constants    |

Identifiers start with a letter; digits permitted after the first character;
underscore only in `SCREAMING_SNAKE`. Reserved words (§1.4) may not be used as
identifiers.

### 2.4 Integer Literals

Follow [ISO/IEC 9899:2018 (C17)](https://www.iso.org/standard/74528.html)
§6.4.4.1: decimal, no leading zeros, unary minus for negatives (`0`, `42`,
`-40`).

### 2.5 Floating-Point Literals

[IEEE 754-2019](https://ieeexplore.ieee.org/document/8766229) double precision.
Must include a decimal point (`0.0`, `3.14`, `-40.0`). Scientific notation is
not supported in v0.1. `NaN` and `Inf` are not permitted in range constraints.

### 2.6 String Literals

UTF-8 in double quotes. Escapes follow
[RFC 8259 §7](https://www.rfc-editor.org/rfc/rfc8259): `\"`, `\\`, `\n`, `\r`,
`\t`, `\uXXXX`.

### 2.7 Regex Literals

Enclosed in forward slashes; syntax follows
[ECMA-262](https://tc39.es/ecma262/#sec-regexp-regular-expression-objects).
Forward slashes inside the pattern are escaped `\/`.

```ridl
/^[A-HJ-NPR-Z0-9]{17}$/
```

### 2.8 Tokens recognised but not used by typl

Duration literals (`500us`, `10ms`, `1s`) and the `@` timing sigil are lexed
family-wide but rejected by the `.typl` profile (TYPL-302).

---

## 3. Packages, Imports, and Visibility

The module system is ADR-0002's `ns` core, restated normatively for the typl
profile. Four keywords: `package`, `import`, `as`, `internal`. Distribution
(manifest, lockfile, cache) is tooling, not language — see ADR-0002.

### 3.1 Package Declaration

Every file begins with exactly one `package` declaration — dot-separated
lowercase identifiers:

```ridl
package veh.common
```

**A package is a directory.** All files in the directory declare the same
package name, and the name mirrors the directory path relative to the manifest
root. Mismatch is a hard error (TYPL-002). The package — not the file — is the
unit of visibility, cycle checking, and codegen output.

### 3.2 Imports

Imports are qualified and named:

```ridl
import veh.common.Speed
import marine.nav.Speed as MarineSpeed   // alias — collision release valve
```

Per ADR-0002 there are **no wildcards, no relative imports, no re-exports**. All
three are hard errors.

> **Errata note.** RIDL Language Reference v0.1 §2.2 showed
> `import veh.common.*` as a linter warning. ADR-0002, accepted later, bans
> wildcards outright. This document follows ADR-0002: wildcard import is a
> **compile error** (TYPL-003). The RIDL Language Reference should be amended to
> match.

**Implicit imports** — always available: all definitions in the same package,
and all definitions in `ridl.std` (Appendix A).

**Qualified references** — any public definition may be referenced by fully
qualified name without an import:

```ridl
limit : veh.regulatory.SpeedLimit
```

### 3.3 Visibility

Two levels. Public is the default; `internal` is package-private:

```ridl
struct WheelTick { ... }               // public — visible to importers
internal struct RawWheelFrame { ... }  // package-private
```

`internal` may prefix any typl definition (`type`, `const`, `struct`, `enum`,
`enumset`, `union`). A public declaration must not expose an `internal` type in
its fields, arms, bounds constants, or backing (TYPL-005) — the contract surface
must be fully importable.

### 3.4 Cycles

Cycles **within** a package are permitted (mutually referencing declarations are
normal). Cycles **across** packages are a hard error (TYPL-004). Note that
_recursive composite shapes_ are separately restricted — see §7.3.

---

## 4. Primitives

Five primitive types. Primitives are lowercase keywords — visually distinct from
`CamelCase` named types. They serve as backing types in `type` definitions;
direct use as field types is restricted (§15.3).

| Primitive | Meaning                                                  | Constraint                        |
| --------- | -------------------------------------------------------- | --------------------------------- |
| `boolean` | logical true/false                                       | none                              |
| `integer` | whole number — width inferred from range                 | recommended — profile may require |
| `float`   | real number — width inferred from range and step         | recommended — profile may require |
| `string`  | character sequence — default `[0..256]` if unspecified   | recommended — profile may require |
| `bytes`   | opaque binary buffer — default `[0..256]` if unspecified | recommended — profile may require |

### 4.1 Boolean

No constraint syntax. Serialized as `uint8` on transport — `0` = false, `1` =
true.

### 4.2 Integer

Range constraint `[min..max]` strongly recommended; active profiles may require
it. Width is inferred **once by the compiler** from the declared range and
passed to all backends:

| Range                                              | Wire type |
| -------------------------------------------------- | --------- |
| `[0..255]`                                         | `uint8`   |
| `[-128..127]`                                      | `int8`    |
| `[0..65535]`                                       | `uint16`  |
| `[-32768..32767]`                                  | `int16`   |
| `[0..4294967295]`                                  | `uint32`  |
| `[-2147483648..2147483647]`                        | `int32`   |
| larger unsigned — up to `[0..9223372036854775807]` | `uint64`  |
| larger signed or no range                          | `int64`   |

**Integer domain.** Every integer range must fit within `[-2⁶³ .. 2⁶³−1]` — the
`int64` domain. Bounds outside it are a compile error (TYPL-111). This closes
the u64 hole: the language layer is always `int64` (Kotlin `Long`, C++
`int64_t`), which cannot hold the top half of full-range `uint64`. A value that
genuinely needs all 64 unsigned bits is modelled as `bytes [8]`.

**Language layer** — always `int64`. **Transport layer** — narrowest wire type
that safely contains the range. The resolved width is part of the contract:
widening a range across a width boundary changes the wire type, which is a
**breaking change** classified by `ridl-diff`. (An explicit width **floor** to
pre-empt such flips — a `wire` clause — is a deferred open question, §17.11.)

### 4.3 Float

Range and `step` strongly recommended; active profiles may require them. `step`
declares the value's quantization: valid values are `min + n·step`.

Width inference is **count-based**, not step-based. Let
`N = (max − min) / step + 1` — the number of distinct representable values:

| Condition                                                                       | Wire type |
| ------------------------------------------------------------------------------- | --------- |
| range + step declared, `N ≤ 2²⁴`, both bounds exactly representable in binary32 | `float32` |
| otherwise — no step, no range, `N > 2²⁴`, or bounds not representable           | `float64` |

> **Errata note.** RIDL Language Reference v0.1 §3.3 inferred float width from
> step alone (`step >= 0.001` → `float32`). That rule is unsound:
> `float [0.0..1000000.0 step 0.001]` satisfies it but requires 10⁹ distinct
> values — far beyond binary32's 24-bit significand. The count-based rule
> replaces it; the RIDL Language Reference should be amended.

**Quantized wire form — scaled integers.** Because `step` anchors values at
`min + n·step`, a quantized float is losslessly a **scaled integer**:
`raw = (value − min) / step`, an unsigned integer whose width follows from `N`
via the §4.2 table. Transports with a scaled-integer convention — CAN/DBC
always, SOME/IP optionally per deployment — encode quantized floats this way
(factor = step, offset = min). This is exactly AUTOSAR's LINEAR CompuMethod, so
the ARXML mapping is direct, and it sidesteps binary-float step fuzziness (0.001
has no exact binary32 representation) on the transports that care most. proto3
and FlatBuffers keep native `float`/`double`. See Appendix D.

**Language layer** — always `float64`. **Transport layer** — as inferred above.

### 4.4 String

Bound is in characters; encoding (ASCII, UTF-8, UTF-16) is a codegen concern per
target. Default `[0..256]` when unspecified (warning).

### 4.5 Bytes

Bound is in bytes; constraint syntax mirrors `string` but has no `match`.
Default `[0..256]` when unspecified (warning).

---

## 5. Type Definitions

The `type` keyword defines a **named scalar** — a constrained primitive or a
physical unit type. Named types carry domain meaning and are the primary
building blocks for all fields.

### 5.1 Physical Unit Types

A unit type associates a scalar with a unit from the **Unified Code for Units of
Measure (UCUM)** — the unit standard used by OpenTelemetry, HL7 FHIR, and ISO
11240:

```ridl
type Speed       : km/h  [0.0..250.0 step 0.5]
type Temperature : Cel   [-40.0..125.0 step 0.1]
type Torque      : N.m   [0.0..500.0 step 0.1]
type RPM         : /min  [0.0..8000.0 step 10.0]
type Pressure    : bar   [0.0..10.0 step 0.01]
type Voltage     : V     [0.0..48.0 step 0.1]
type Ratio       : %     [0.0..100.0 step 0.1]
```

The underlying primitive is `float`. UCUM expressions are **case-sensitive**.
Because a unit type carries unit, range, and step, downstream layers inherit
dimensional checking and range-driven saturation — rmdl computes with `Speed`,
not bare `real` (concept note §6).

Common automotive UCUM units: `km/h`, `Cel`, `N.m`, `/min`, `m/s2`, `bar`, `V`,
`A`, `W`, `%`.

### 5.2 Constrained Primitive Types

```ridl
type Counter : integer [0..65535]
type Gain    : float   [0.0..1.0 step 0.01]
type Vin     : string  [17 match VIN_PATTERN]
type Frame   : bytes   [8]
```

### 5.3 String Constraint Syntax

| Syntax                            | Meaning                 |
| --------------------------------- | ----------------------- |
| `string [N]`                      | fixed N characters      |
| `string [min..max]`               | min to max characters   |
| `string [N match PATTERN]`        | fixed N with validation |
| `string [min..max match PATTERN]` | range with validation   |

`match` references a named regex constant or an inline regex literal.

### 5.4 Bytes Constraint Syntax

| Syntax             | Meaning          |
| ------------------ | ---------------- |
| `bytes [N]`        | fixed N bytes    |
| `bytes [min..max]` | min to max bytes |

No `match` — bytes are opaque.

### 5.5 Range Bounds

Ranges are **closed** (inclusive) on both ends. Either bound may be omitted
(`[0..]`, `[..255]`), in which case the missing bound defaults to the widest
value the inferred width allows. Exclusive bounds are not supported in v0.1 (see
§17 and Appendix G).

### 5.6 Width Is Inferred, Never Written

typl v0.1 has **no surface syntax for wire width**. A range is the only width
control an author needs: `integer [0..255]` _is_ a `uint8`, `integer [0..65535]`
_is_ a `uint16`, and the compiler derives the narrowest safe width once (§4.2,
§4.3) and passes it to every backend. The ten concrete width names
(`uint8 … float64`) are not keywords, not primitives, and not writable anywhere
in a `.typl` source — writing one is a parse error (there is no production for
it).

Range-driven inference has one known hazard: widening `[0..250]` to `[0..300]`
silently flips the wire type `uint8 → uint16` — a wire ABI break from a
semantically innocent edit (hardest on FlatBuffers and CAN, invisible on proto3
varint). v0.1 handles this with the **`ridl-diff` gate alone**: it classifies
every resolved-width change as breaking, so the flip is caught in CI. An
_explicit width floor_ that would pre-empt the flip at the source — a `wire`
clause pinning headroom above the boundary — is deliberately deferred; see
§17.11.

### 5.7 Type Identity — Nominal

Every named type is **nominally distinct** — from its backing primitive and from
every other named type, even one with identical constraints. `Speed` and
`Torque` are both float-backed; they are not assignable to each other, and
neither accepts a bare `float`. A literal is accepted where a named type is
expected when it satisfies the type's constraints (this is how
`const MAX_SPEED : Speed = 250.0` works); a _value of another named type_ never
is. There is no implicit conversion anywhere in the family — explicit conversion
is expr/rmdl territory, where unit dimensions decide convertibility (§17.5).

Precedent: Ada derived types, Rust newtypes, Haskell `newtype`. Rationale:
nominal identity is what makes unit safety real — structural typing would let a
`Torque` flow into a `Speed` port because both are "float in a range", which is
precisely the class of error this language exists to prevent. Codegen realises
nominal types as distinct wrapper types where the target allows (Rust newtype,
Kotlin `value class`) and as documented aliases where it does not (proto3).

### 5.8 Init Values

Every type has an **init value** — the value a consumer holds before anything
real arrives (the interaction layer's signal channels start from it; AUTOSAR
`initValue` is the precedent). It is either declared or derived:

**Declared** — a bare `= value` suffix, valid on `type` definitions as well as
struct fields. There is no keyword: the value simply follows the constraint.

```ridl
type Speed : km/h [0.0..250.0 step 0.5] = 0.0
```

The bare `= value` form is the family's single init syntax at the vocabulary and
interaction layers (typl types, typl struct fields, ridl signal overrides). It
carries **no keyword** — not `default` (retired) and not `init` (which belongs
to rmdl alone, where `init x = value` seeds a memory recurrence and the keyword
is needed to disambiguate it from the equation).

**Derived** — when no `= value` is declared:

| Type                      | Derived init                                                                          |
| ------------------------- | ------------------------------------------------------------------------------------- |
| `boolean`                 | `false`                                                                               |
| numeric / unit type       | `0` (or `0.0`) if within the range, else `min`                                        |
| `string` / `bytes`        | empty if the bounds admit length 0; **not derivable** otherwise                       |
| type with `match` pattern | **not derivable** (a pattern-valid value cannot be synthesised) — declare a `= value` |
| `enum`                    | the value `0` if declared, else the lowest declared value                             |
| `enumset`                 | empty set                                                                             |
| `struct`                  | each field's init, recursively; optional fields absent                                |
| `union`                   | first arm's init                                                                      |
| tuple                     | each field's init                                                                     |
| collection                | `min`-bound count of element inits (empty when `min = 0`)                             |

A declared init value must satisfy the type's constraints (TYPL-109). A type
whose init is not derivable is simply marked so in the IR (TYPL-115, info) — it
becomes an error only where a consumer _requires_ an init (e.g. a ridl signal
payload, ridl §4.4) and none is declared.

---

## 6. Constants

`const` defines a named compile-time constant — package-scoped, importable, and
reusable in range constraints, `match` patterns, and init (`= value`)
declarations. Higher profiles also use constants in `require`/`ensure`
expressions.

### 6.1 Value Constants

```ridl
package veh.common

const MAX_SPEED      : Speed   = 250.0
const SPEED_LIMIT_EU : Speed   = 130.0
const MAX_GEAR       : integer = 6
const IDLE_RPM       : RPM     = 800.0
```

`SCREAMING_SNAKE` naming. The declared value must satisfy the constant's type
constraints (TYPL-108). Reuse:

```ridl
type Speed : km/h [0.0..MAX_SPEED step 0.5]

struct GearState {
  gear : integer [0..MAX_GEAR]
}
```

### 6.2 Regex Constants

A `const` may hold a regex literal, reusable in `match` constraints:

```ridl
const VIN_PATTERN   = /^[A-HJ-NPR-Z0-9]{17}$/
const ASCII_PATTERN = /^[\x20-\x7E]+$/
```

```ridl
type Vin   : string [17 match VIN_PATTERN]
type Email : string [1..254 match /^[^@]+@[^@]+\.[^@]+$/]
```

---

## 7. Structs

A `struct` is a named composite with a **fixed, closed set** of named fields.
There are no additional/unknown properties — a struct is closed by definition
(contrast JSON Schema's open-world default, Appendix G).

```ridl
struct DriverProfile {
  name     : Name
  speed    : Speed
  override : Speed?     // optional — may be absent
}
```

### 7.1 Optionality

The `?` suffix marks a field as optional — _may be absent_, which is distinct
from "present with a null value"; typl has no null. Maps to `optional` in
proto3, `Option<T>` in Rust, `T?` in Kotlin.

### 7.2 Field Init Values

A field may carry a bare `= value` suffix — its init value, the same syntax a
`type` uses (§5.8). There is no attribute block and no keyword; typl carries no
other field attribute at the vocabulary layer.

```ridl
struct RetryPolicy {
  attempts : integer [0..10] = 3
  backoff  : Duration        = 100.0
}
```

The init value must satisfy the field's type (TYPL-109). `require`/`ensure` are
expr-core attributes owned by higher profiles and are rejected in `.typl`
(TYPL-303).

### 7.3 No Recursive Shapes

A struct (or union) must not reference itself, directly or through any chain of
composite references. Recursion makes the wire size unbounded, which contradicts
the bounded-size guarantee every typl composite otherwise carries. Recursive
composite reference is a compile error in v0.1 (TYPL-206). If a genuine
tree/graph payload need arises, it must be modelled with explicit indices into a
bounded array. (Open to revisiting — §17.)

### 7.4 Field Identity and Evolution

Tag-based transports (proto3 field numbers, FlatBuffers field ids) need a stable
numeric identity per field. typl assigns it **implicitly, by declaration order**
(1-based), for struct fields and union arms alike. There is no explicit `= N`
tag syntax.

That makes evolution discipline part of the language:

- **Append-only.** New fields are added at the end of the struct or union.
  Inserting or reordering fields shifts the ordinals of everything after them —
  a wire break, rejected by `ridl-diff`.
- **Delete by tombstone.** A removed field keeps its ordinal slot occupied with
  `reserved`:

```ridl
struct DriverProfile {
  name     : Name
  reserved legacyChecksum      // was ordinal 2 — slot retired, never reused
  speed    : Speed
}
```

Re-declaring a field under a reserved name is a compile error (TYPL-210).
`reserved` is also valid in `enum` bodies with the retired integer value
(`reserved 3`) so a wire value is never reused with a new meaning.

- **`ridl-diff` is the enforcement point.** Ordinals are fully derivable from
  source — no sidecar state, no lockfile-assigned numbers. The CI plane
  (`ridl-diff` against the previous IR snapshot) rejects reorder, insertion, and
  un-tombstoned deletion in published packages.

- **Decoder rule for forward compatibility.** The flip side of append-only: a
  decoder built against version _n_ of a struct, receiving a payload from a
  version _n+k_ encoder, **must ignore unknown trailing fields** (and should
  preserve them when acting as a proxy — the proto3 unknown-field lesson). This
  is native on proto3/FlatBuffers; on positional transports it requires
  length-prefixed framing (SOME/IP TLV) or is simply unavailable (CAN) — where
  unavailable, mixed-version communication is a deployment error rsdl must
  surface (§17.9).

**Rationale.** Explicit proto-style tags were considered and rejected: they
impose a per-field numbering ritual on every struct — including the majority
bound to positional transports (CAN, SOME/IP) where the numbers mean nothing —
and gaps/vanity numbering must then be linted. Implicit ordinals keep the
surface small and the audit trail honest: the wire layout is readable from the
`.typl` file alone. The discipline this demands — append-only with tombstones —
is the discipline a contract language wants anyway. Trade-off on record:
reordering is legal _syntax_, and only `ridl-diff` makes it an error; publishing
without the CI gate is unprotected. Judged acceptable because `ridlc --frozen` +
`ridl-diff` are already mandatory in the ecosystem's CI plane.

---

## 8. Enums

An `enum` is a named set of discrete **integer-backed** values, for single-value
selection:

```ridl
enum GearPosition {
  PARK    = 0
  DRIVE   = 1
  REVERSE = 2
  NEUTRAL = 3
}
```

- Backing type is `integer`, implicit; all values explicitly assigned; values
  unique (TYPL-203)
- First value conventionally `= 0` for proto3 compatibility
- String-backed enums are not supported in v0.1 — see §17 and Appendix G

---

## 9. Enum Sets

An `enumset` is a named bitfield — multiple flags active simultaneously. Backing
width inferred from the highest bit position.

### 9.1 Standalone Form

```ridl
enumset WarningFlags {
  LOW_FUEL     = 0    // bit 0
  CHECK_ENGINE = 1    // bit 1
  DOOR_OPEN    = 2    // bit 2
  SEATBELT     = 3    // bit 3
}
```

### 9.2 Derived Form

Derives bit positions from an existing `enum` — preferred when both single-value
and set use are needed:

```ridl
enum Warning { LOW_FUEL = 0, CHECK_ENGINE = 1, DOOR_OPEN = 2, SEATBELT = 3 }

enumset WarningFlags : Warning
```

### 9.3 Width Inference

| Highest bit | Wire type |
| ----------- | --------- |
| 0..7        | `uint8`   |
| 8..15       | `uint16`  |
| 16..31      | `uint32`  |
| 32..63      | `uint64`  |

Language layer — always `int64`. Transport layer — narrowest inferred.

---

## 10. Unions

A `union` is a **discriminated (tagged)** union — exactly one arm active at any
time:

```ridl
union SensorResult {
  ok  : SensorReading
  err : SensorFault
}
```

- Arms reference **named types** only — primitives not permitted directly
  (TYPL-204)
- Untagged unions and intersections (JSON Schema `anyOf`/`allOf`) are
  deliberately not supported — see Appendix G
- Maps to `oneof` in proto3, `union` in FlatBuffers, `sealed class` in Kotlin,
  Rust `enum`

### 10.1 Error Types

The `error` modifier marks a named composite as **failure vocabulary**:

```ridl
error enum DiagError {
  FILTER_INVALID = 0
  STORAGE_BUSY   = 1
  ACCESS_DENIED  = 2
}

error struct CalFault {          // data-carrying failure
  code   : integer [0..255]
  detail : Message
}

error union ServiceFault {       // composes error types — all arms error-typed
  diag : DiagError
  cal  : CalFault
}
```

- `error` is valid on `enum`, `struct`, and `union` only (TYPL-212); an
  `error union`'s arms must all be error-typed (TYPL-214)
- An error type is otherwise an **ordinary type** — package-scoped, importable,
  nominally distinct, usable wherever data is legal (a fault-log struct may
  contain one; a signal may publish one). The marker adds exactly one bit of
  meaning: _this shape describes a failure_
- Codegen realises error types with the target's error material where it exists
  (Rust `std::error::Error` impl, Kotlin sealed failure hierarchies) and as
  plain data elsewhere

### 10.2 Result Unions

A union mixing error and non-error arms is a **result union** and must have
exactly two arms — one success arm (any non-error type) and one error arm (an
error type). Anything else that mixes the two kinds is malformed (TYPL-213).
Several failure kinds compose into one `error union` _before_ entering the
result union.

```ridl
union CalOutcome {
  ok  : CalReport
  err : CalFault
}
```

A result union is the family's `Result<T, E>` — achieved without generics,
consistent with the no-parametric-types decision. In pure typl a result union is
simply a union; the **interaction layer** (ridl §10.1) recognises it at
interaction positions and maps the error arm onto transport-native error
channels. This is the family's entire functional-error mechanism: failure is
data, declared once in the vocabulary — there is no `throws`, no exception, no
second channel anywhere in the family.

---

## 11. Tuples

A tuple is an anonymous composite of **named** fields used inline — as struct
fields at the typl layer; higher profiles additionally use tuples as interaction
parameters and query returns.

```ridl
struct SensorBounds {
  range : (min: Speed, max: Speed)
}
```

- Fields are always named — positional access is not permitted
- The empty tuple `()` is the unit type (its interaction role is defined by
  ridl)
- The same tuple shape used in multiple places draws a linter warning: extract a
  named `struct` (TYPL-205)

---

## 12. Collections

Collections are finite, **bounded** aggregates. All variable-size collections
require explicit bounds — no defaults, no exceptions (TYPL-201/202).

### 12.1 Array

```ridl
readings : [Speed; 8]          // fixed — exactly 8
faults   : [FaultCode; 0..32]  // bounded — 0 to 32
```

### 12.2 Map

Keys must be a named string type or primitive:

```ridl
metadata : [Label : Name; 0..32]
sensors  : [Label : Speed; 1..8]
```

### 12.3 Bound Rules

| Construct     | Syntax            | Bound                          |
| ------------- | ----------------- | ------------------------------ |
| Fixed array   | `[T; N]`          | exactly N — mandatory          |
| Bounded array | `[T; min..max]`   | min to max — mandatory         |
| Bounded map   | `[K:V; min..max]` | min to max entries — mandatory |

The stream container `<T>` is the deliberate exception to boundedness and lives
in the ridl profile (§1.3).

---

## 13. Comments

Discarded by the compiler.

```ridl
// line comment
/* block comment — nesting not supported */
```

---

## 14. Doc Comments

Attached to the immediately following definition; processed by documentation
generators and IDEs; no semantic effect. No blank line between a doc comment and
its definition (warning).

### 14.1 Syntax

```ridl
/// Current calibrated wheel radius
type WheelRadius : m [0.20..0.45 step 0.001]
```

Multi-line `/** ... */` with full [CommonMark](https://commonmark.org) markdown,
`[TypeName]` / `[pkg.TypeName]` reference links, and code blocks — as in the
RIDL Language Reference §15.

### 14.2 Tags

| Tag           | Value                                 | Valid on |
| ------------- | ------------------------------------- | -------- |
| `@see`        | qualified type name                   | all      |
| `@labels`     | comma-separated classification labels | all      |
| `@deprecated` | `"reason string"`                     | all      |

### 14.3 Labels

`@labels` carries free-form `SCREAMING_SNAKE` classification identifiers
(optionally with a parenthesised suffix, e.g. `MY_LABEL(D)`). The core language
defines no vocabulary — an external **profile** validates labels and enforces
combinations. The compiler passes labels through to generated metadata
unchanged.

---

## 15. Conventions

### 15.1 Naming

| Construct                                    | Convention        | Example                        |
| -------------------------------------------- | ----------------- | ------------------------------ |
| `type`, `struct`, `enum`, `enumset`, `union` | `CamelCase`       | `VehicleSpeed`, `WarningFlags` |
| Fields, tuple fields                         | `camelCase`       | `currentSpeed`                 |
| Enum values, enumset bits, constants         | `SCREAMING_SNAKE` | `PARK`, `MAX_SPEED`            |
| Packages                                     | `lowercase.dot`   | `veh.common`                   |

### 15.2 Separators

Newline and comma interchangeable in all block constructs; trailing comma
permitted; no semicolons.

### 15.3 Type Usage

- `string` and `bytes` must not be used directly as field types — always define
  a named `type` (the ridl profile grants streams an element-type exception;
  there is no exception inside typl)
- All variable-size types declare explicit bounds
- `union` arms reference named types only
- Prefer named types over inline primitives for all domain concepts

### 15.4 Imports

Explicit single-type imports only (wildcards are an error, §3.2). Aliases only
on genuine collision — linters flag gratuitous aliases.

---

## 16. Diagnostics

typl adopts **stable diagnostic codes** — a practice inherited from the markspec
typl catalogue (Appendix F). Codes are namespaced `TYPL-` and grouped by
hundreds: 0xx module, 1xx scalars & constants, 2xx composites, 3xx profile
boundary, 4xx documentation. Codes are never renumbered; retired codes are
marked deprecated and never reused.

### 16.1 Module (TYPL-0xx)

| Code     | Rule                                               | Severity |
| -------- | -------------------------------------------------- | -------- |
| TYPL-001 | more than one `package` declaration per file       | error    |
| TYPL-002 | package name does not mirror directory path        | error    |
| TYPL-003 | wildcard, relative, or re-exporting import         | error    |
| TYPL-004 | circular package imports                           | error    |
| TYPL-005 | public declaration exposes an `internal` type      | error    |
| TYPL-006 | conflicting imports without alias                  | error    |
| TYPL-007 | unused import                                      | warning  |
| TYPL-008 | alias without an actual collision                  | warning  |
| TYPL-009 | duplicate definition of the same name in a package | error    |

### 16.2 Scalars and Constants (TYPL-1xx)

| Code     | Rule                                                                                                       | Severity                                           |
| -------- | ---------------------------------------------------------------------------------------------------------- | -------------------------------------------------- |
| TYPL-101 | `integer` without range                                                                                    | warning; error if active profile requires          |
| TYPL-102 | `float` without range + `step`                                                                             | warning; error if active profile requires          |
| TYPL-103 | `string`/`bytes` without explicit bounds — default `[0..256]` applied                                      | warning; error if active profile requires          |
| TYPL-104 | range `min > max`                                                                                          | error                                              |
| TYPL-105 | `step` type mismatch, non-positive, or larger than the range                                               | error                                              |
| TYPL-106 | invalid regex syntax in `match` or `const`                                                                 | error                                              |
| TYPL-107 | regex contradicts declared character bound                                                                 | warning                                            |
| TYPL-108 | `const` value violates its declared type constraints                                                       | error                                              |
| TYPL-109 | init (`= value`) incompatible with the type/field constraints                                              | error                                              |
| TYPL-110 | unknown or malformed UCUM unit expression                                                                  | error                                              |
| TYPL-111 | integer range bound outside the `int64` domain `[-2⁶³..2⁶³−1]`                                             | error                                              |
| TYPL-112 | a concrete wire-width name (`uint8` … `float64`) written in source — width is inferred, not written (§5.6) | error                                              |
| TYPL-115 | type has no derivable init value and no declared `= value` (§5.8)                                          | info — escalated by consumers that require an init |

### 16.3 Composites (TYPL-2xx)

| Code     | Rule                                                                                                       | Severity |
| -------- | ---------------------------------------------------------------------------------------------------------- | -------- |
| TYPL-201 | array without explicit bounds                                                                              | error    |
| TYPL-202 | map without explicit bounds                                                                                | error    |
| TYPL-203 | enum values not unique / not explicitly assigned                                                           | error    |
| TYPL-204 | union arm with primitive type                                                                              | error    |
| TYPL-205 | same tuple shape used in multiple places                                                                   | warning  |
| TYPL-206 | recursive composite reference (direct or transitive)                                                       | error    |
| TYPL-207 | enumset bit positions not unique                                                                           | error    |
| TYPL-208 | `string`/`bytes` used directly as field type                                                               | error    |
| TYPL-209 | map key is not a named string type or primitive                                                            | error    |
| TYPL-210 | field, arm, or enum value re-declared under a `reserved` name or value                                     | error    |
| TYPL-211 | duplicate or dangling `reserved` entry (name/value never previously used)                                  | warning  |
| TYPL-212 | `error` modifier on a declaration other than `enum`, `struct`, `union`                                     | error    |
| TYPL-213 | union mixing error and non-error arms without the result-union shape (exactly one success + one error arm) | error    |
| TYPL-214 | `error union` containing a non-error-typed arm                                                             | error    |

### 16.4 Profile Boundary (TYPL-3xx)

Emitted when a `.typl` file (or a package declared `profile = "typl"` in
`ridl.toml`) contains constructs of a higher layer:

| Code     | Rule                                                           | Severity |
| -------- | -------------------------------------------------------------- | -------- |
| TYPL-301 | stream type `<T>` in typl context                              | error    |
| TYPL-302 | timing annotation or duration literal in typl context          | error    |
| TYPL-303 | `require`/`ensure` attribute in typl context                   | error    |
| TYPL-304 | interaction/behaviour/architecture declaration in typl context | error    |

### 16.5 Documentation (TYPL-4xx)

| Code     | Rule                                                  | Severity |
| -------- | ----------------------------------------------------- | -------- |
| TYPL-401 | unresolved `[TypeName]` reference in doc comment      | warning  |
| TYPL-402 | `@labels` identifier not recognised by active profile | info     |
| TYPL-403 | `@labels` combination invalid per active profile      | error    |
| TYPL-404 | blank line between doc comment and definition         | warning  |
| TYPL-405 | `@deprecated` without reason string                   | warning  |

---

## 17. Open Questions

1. **String-backed enums.** JSON Schema, TypeScript, and OpenAPI make string
   enums the common case; typl v0.1 supports only integer-backed enums (wire
   efficiency, DBC/CAN mapping). Candidate design:
   `enum Region : string { EU = "eu", US = "us" }` — integer wire form with a
   declared string rendering. Deferred.
2. **Exclusive range bounds.** `exclusiveMinimum`/`exclusiveMaximum` have no
   typl equivalent; closed bounds + `step` cover the practical automotive cases.
   Revisit if a real contract needs open intervals.
3. **`uniqueItems` for arrays.** Not expressible. A `set` collection
   (`{T; min..max}`?) would cover it; deferred until demanded.
4. **Recursion policy (§7.3).** Hard error today. If tree-shaped payloads become
   a real need, a depth-bounded recursion annotation is the likely shape.
5. **Unit conversion semantics.** UCUM units are currently _labels with
   dimensional identity_. Whether typl-core defines convertibility (`km/h` ↔
   `m/s`) for rmdl's benefit, or leaves conversion to codegen, is open.
6. **Scientific notation** in float literals (blocked at the family lexer level
   in v0.1).
7. **Value-rule expressions — deferred to the `expr` core by decision.** typl
   v0.1 constraints are deliberately closed-form: literal/const ranges, `step`,
   length bounds, and `match` patterns. Everything requiring a general
   expression language is deferred to the family `expr` core (concept note open
   question 6) and will arrive with `require`/`ensure`: **(a)** arithmetic in
   constraint bounds (`[0.0..MAX_SPEED * 0.5]` — v0.1 bounds accept only
   literals and plain constant references); **(b)** predicate constraints on a
   single value (`value % 2 == 0`); **(c)** regex matching as a predicate
   operator — declarative `match` on string types exists today; when expr lands,
   the **same `match` keyword** serves infix in predicates
   (`require vin match VIN_PATTERN`): one keyword, one concept (pattern
   conformance), two positions. (A `~` sigil was considered and rejected — the
   family is deliberately sigil-poor, and `match` reads as English to
   non-programmer audiences); **(d)** cross-field invariants on structs
   (`min <= max`) — the likely surface is a struct-level `invariant` block
   completing the Eiffel triad beside ridl's `require`/`ensure`. Rationale for
   deferral: closed-form constraints keep every typl type decidable for width
   inference and mechanically derivable into property-test generators; general
   expressions in constraint position need fencing rules (const-evaluable subset
   only) that belong to the expr-core specification, not here.
8. **Init and invalid sentinel values.** _Substantially resolved since drafted:_
   the init half became §5.8 (init values — a declared bare `= value` or
   derived), and the invalid half is handled at the interaction layer — ridl
   §4.5 propagates invalidity as channel state, with the SNA sentinel as its
   **CAN/AUTOSAR wire realisation**. What remains typl's: whether a type may
   _declare_ its wire sentinel explicitly (`[ invalid = 255 ]`) for brownfield
   DBC matching, rather than the codegen choosing one. Reduced-scope open
   question.
9. **Byte order.** CAN/DBC signals carry per-signal byte order (Intel/Motorola);
   SOME/IP defaults big-endian. typl is silent. Endianness is almost certainly a
   _transport/deployment_ property (rsdl or codegen profile), not a type
   property — but that decision needs writing down, and the §7.4 mixed-version
   decoder rule depends on transport framing too.
10. **Canonical (deterministic) encoding.** Signed payloads, content-hashing of
    data, and the replay/test plane all want a bit-reproducible encoding
    (protobuf's deterministic-marshal caveats are the cautionary tale; map
    ordering is the classic leak). Likely an IR-spec concern: define canonical
    field and map ordering per transport.
11. **Explicit wire-width floor — a deferred `wire` clause.** v0.1 has no
    surface syntax for wire width (§5.6): a range _is_ the width, inferred once
    by the compiler, and the ten concrete width names (`uint8 … float64`) are
    not writable anywhere. This leaves one hazard unhanded at the source:
    widening a range across a width boundary silently flips the wire type
    (`uint8 → uint16`), a wire ABI break that today only the `ridl-diff` CI gate
    catches. A future **`wire` clause** would let an author pin an explicit
    width _floor_ above the boundary —
    `type Counter : integer [0..250] wire uint16` — reserving evolution headroom
    so a later widening does not flip the transport encoding. Design constraints
    already settled for whenever it lands: the named width must be **at least**
    the inferred width and of the same signedness class (narrower or
    sign-incompatible is an error); it never changes the language-layer type
    (`int64`/`float64`) or the value constraints — the range still validates,
    only the transport encoding widens; and it is a floor, never a parallel type
    system (ranges remain the semantic truth). Deferred because the `ridl-diff`
    gate already prevents the silent break in CI, and the clause is pure
    evolution ergonomics — worth adding once real contracts have hit the flip in
    practice and can shape the exact rules (e.g. whether the floor should also
    be declarable per-field, and how it interacts with the scaled-integer wire
    form of quantized floats).

---

## Appendix A — Standard Library

`ridl.std` is a **pure typl package** — the proof that typl stands alone. It is
implicitly imported in every file of every profile. Contents (normative,
unchanged from RIDL Language Reference Appendix A):

```ridl
package ridl.std

// ---------- Regex Constants ----------

const UUID_PATTERN  = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/
const ULID_PATTERN  = /^[0-7][0-9A-HJKMNP-TV-Z]{25}$/
const VIN_PATTERN   = /^[A-HJ-NPR-Z0-9]{17}$/
const URI_PATTERN   = /^[a-zA-Z][a-zA-Z0-9+\-.]*:\/\/.+$/
const URL_PATTERN   = /^https?:\/\/.+$/
const EMAIL_PATTERN = /^[^@]+@[^@]+\.[^@]+$/
const IPV4_PATTERN  = /^(\d{1,3}\.){3}\d{1,3}$/
const IPV6_PATTERN  = /^[0-9a-f:]+$/
const ASCII_PATTERN = /^[\x20-\x7E]+$/

// ---------- Identity Types ----------

/// RFC 4122 UUID — 36 characters including hyphens
type Uuid : string [36 match UUID_PATTERN]

/// ULID — 26 characters, lexicographically sortable
type Ulid : string [26 match ULID_PATTERN]

/// ISO 3779 Vehicle Identification Number — exactly 17 characters
type Vin  : string [17 match VIN_PATTERN]

// ---------- Network Types ----------

/// RFC 3986 URI
type Uri   : string [1..2048 match URI_PATTERN]

/// HTTP/HTTPS URL
type Url   : string [1..2048 match URL_PATTERN]

/// Email address — RFC 5321
type Email : string [1..254 match EMAIL_PATTERN]

/// IPv4 address — dotted decimal notation
type IpV4  : string [7..15 match IPV4_PATTERN]

/// IPv6 address
type IpV6  : string [2..39 match IPV6_PATTERN]

// ---------- General String Types ----------

/// Short human-readable label — ASCII printable, max 64 characters
type Label   : string [1..64 match ASCII_PATTERN]

/// Human-readable name — max 128 characters
type Name    : string [1..128 match ASCII_PATTERN]

/// Short message or description — max 256 characters
type Message : string [1..256 match ASCII_PATTERN]

// ---------- Time Types ----------

/// Absolute point in time — platform time domain (ridl §3.1):
/// int64 microseconds since the PTP epoch, 1970-01-01 00:00:00 TAI.
/// Continuous and leap-second-free; convert to civil datetime only at
/// presentation edges.
type Timestamp : integer [0..9223372036854775807]

/// Elapsed time interval in milliseconds
type Duration  : ms [0.0..9223372036854775807]

/// Calendar date — ISO 8601 YYYY-MM-DD
type Date : string [10 match /^\d{4}-\d{2}-\d{2}$/]

/// Time of day — ISO 8601 HH:MM:SS
type TimeOfDay : string [8..15 match /^\d{2}:\d{2}:\d{2}/]

// ---------- Version ----------

/// Semantic version — SemVer 2.0.0
type Version : string [5..32 match /^\d+\.\d+\.\d+/]

// ---------- Locale ----------

/// ISO 3166-1 alpha-2 country code
type CountryCode : string [2 match /^[A-Z]{2}$/]

/// ISO 639-1 language code with optional region — e.g. "en", "en-US"
type LanguageCode : string [2..5 match /^[a-z]{2}(-[A-Z]{2})?$/]

// ---------- Crypto / Security ----------

/// SHA-256 hash — 32 bytes
type Sha256Hash  : bytes [32]

/// SHA-512 hash — 64 bytes
type Sha512Hash  : bytes [64]

/// Generic cryptographic signature — 64 bytes
type Signature   : bytes [64]

/// X.509 DER certificate
type Certificate : bytes [1..4096]
```

---

## Appendix B — Full Example

A complete pure-typl package — `veh/common/`, profile-pure, generating types
across every backend with no interface content:

```ridl
package veh.common

// ---------- Units and scalars ----------

/// Vehicle speed over ground
type Speed       : km/h [0.0..MAX_SPEED step 0.5]

/// Coolant / ambient temperature
type Temperature : Cel  [-40.0..125.0 step 0.1]

/// Engine crankshaft speed
type RPM         : /min [0.0..8000.0 step 10.0]

/// Normalised ratio
type Ratio       : %    [0.0..100.0 step 0.1]

type Counter : integer [0..65535]
type Gain    : float   [0.0..1.0 step 0.01]

// ---------- Constants ----------

const MAX_SPEED      : Speed   = 250.0
const SPEED_LIMIT_EU : Speed   = 130.0
const MAX_GEAR       : integer = 6
const IDLE_RPM       : RPM     = 800.0

// ---------- Enums ----------

enum GearPosition {
  PARK    = 0
  DRIVE   = 1
  REVERSE = 2
  NEUTRAL = 3
}

enum Warning {
  LOW_FUEL     = 0
  CHECK_ENGINE = 1
  DOOR_OPEN    = 2
  SEATBELT     = 3
}

enumset WarningFlags : Warning

// ---------- Composites ----------

struct SpeedLimitPayload {
  limit  : Speed
  actual : Speed
}

struct DriverProfile {
  name     : Name              // ridl.std
  speed    : Speed
  override : Speed?            // optional
  gears    : integer [0..MAX_GEAR] = 6
}

struct SensorBounds {
  range    : (min: Speed, max: Speed)
  readings : [Speed; 8]
  labels   : [Label; 1..16]
  meta     : [Label : Name; 0..32]
}

union SensorResult {           // result union — §10.2: one success arm, one error arm
  ok  : SensorReading
  err : SensorFault
}

struct SensorReading {
  value     : Speed
  timestamp : Timestamp        // ridl.std
}

error struct SensorFault {     // failure vocabulary — §10.1
  code    : Counter
  message : Message            // ridl.std
}

// package-private helper — not visible to importers, omitted from
// cross-package codegen artifacts
internal struct RawWheelFrame {
  ticks : Counter
  frame : bytes [8]
}
```

---

## Appendix C — Standards References

| Standard                                                                                                                                                         | Used for                          |
| ---------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------- |
| [Unicode 15.0](https://www.unicode.org/versions/Unicode15.0.0/)                                                                                                  | source encoding                   |
| [RFC 3629](https://www.rfc-editor.org/rfc/rfc3629)                                                                                                               | UTF-8                             |
| [RFC 8259](https://www.rfc-editor.org/rfc/rfc8259)                                                                                                               | string escapes                    |
| [ECMA-262](https://tc39.es/ecma262/)                                                                                                                             | regex literal syntax              |
| [CommonMark](https://commonmark.org)                                                                                                                             | doc comment markdown              |
| [IEEE 754-2019](https://ieeexplore.ieee.org/document/8766229)                                                                                                    | floating point                    |
| [ISO/IEC 9899:2018 (C17)](https://www.iso.org/standard/74528.html)                                                                                               | integer/float literal conventions |
| [UCUM](https://ucum.org/ucum)                                                                                                                                    | physical unit expressions         |
| [ISO 8601](https://www.iso.org/standard/70907.html) / [RFC 3339](https://www.rfc-editor.org/rfc/rfc3339)                                                         | time types                        |
| [RFC 3986](https://www.rfc-editor.org/rfc/rfc3986), [RFC 4122](https://www.rfc-editor.org/rfc/rfc4122)                                                           | Uri, Uuid                         |
| [SemVer 2.0.0](https://semver.org/)                                                                                                                              | Version                           |
| [ISO 3779](https://www.iso.org/standard/52200.html), [ISO 3166-1](https://www.iso.org/standard/72482.html), [ISO 639-1](https://www.iso.org/standard/22109.html) | Vin, CountryCode, LanguageCode    |
| [JSON Schema 2020-12](https://json-schema.org/specification)                                                                                                     | coverage benchmark (Appendix G)   |

---

## Appendix D — Codegen Targets

typl owns the **type layer** of every backend; interaction/behaviour codegen is
specified by the higher profiles. Width mapping is resolved once by the compiler
and passed to all backends (§4.2, §4.3, §9.3).

**Language layer** — always widest:

| Canonical   | Rust  | Kotlin   | C++       | TypeScript         |
| ----------- | ----- | -------- | --------- | ------------------ |
| any integer | `i64` | `Long`   | `int64_t` | `bigint`/`number`* |
| any float   | `f64` | `Double` | `double`  | `number`           |
| enumset     | `i64` | `Long`   | `int64_t` | `bigint`           |

*TypeScript uses `number` when the range fits in 2^53, `bigint` otherwise.

**Transport layer** — narrowest that safely fits:

| Canonical | proto3    | FlatBuffers | SOME/IP   | CAN/DBC        | AIDL     |
| --------- | --------- | ----------- | --------- | -------------- | -------- |
| `uint8`   | `uint32`* | `uint8`     | `UINT8`   | 8 bits         | `int`    |
| `uint16`  | `uint32`* | `uint16`    | `UINT16`  | 16 bits        | `int`    |
| `uint32`  | `uint32`  | `uint32`    | `UINT32`  | 32 bits        | `int`    |
| `int32`   | `int32`   | `int32`     | `SINT32`  | 32 bits signed | `int`    |
| `int64`   | `int64`   | `int64`     | `SINT64`  | 64 bits signed | `long`   |
| `float32` | `float`   | `float32`   | `FLOAT`   | 32 bits        | `float`  |
| `float64` | `double`  | `float64`   | `FLOAT64` | 64 bits        | `double` |

*proto3 has no `uint8`/`uint16` — varint encoding keeps small values small on
the wire.

### Per-Target Compatibility Notes

**proto3.** Unsigned 8/16-bit map to `uint32` varint — no wire penalty for small
values. Ranges containing negatives map to `sint32`/`sint64` (ZigZag encoding);
plain `int32` varint would cost 10 bytes for every negative value. Field numbers
are the typl ordinals (§7.4); `reserved` tombstones emit proto `reserved`
statements. Range widening never breaks proto wire (varint absorbs it) but
remains a contract-breaking change per `ridl-diff`.

**FlatBuffers.** Full `uint8..uint64` palette — the cleanest width mapping.
Field ids are the typl ordinals emitted as `(id: N)` attributes. FlatBuffers is
the most width-brittle target: any resolved-width change is a hard wire break —
the primary motivation for a future `wire` floor (§17.11); in v0.1 the
`ridl-diff` gate is the sole guard. A struct whose fields are all fixed-width
and non-optional may be emitted as a FlatBuffers `struct` (inline, zero
indirection) instead of a `table`; the IR carries a `fixed_layout` flag for
this.

**AUTOSAR Classic (ARXML).** The best structural fit: range → `DataConstr`,
quantized float → LINEAR `CompuMethod` (factor = step, offset = min) — the
scaled-integer wire form (§4.3) is native here. Two gaps become **profile
rules**: Classic sender-receiver records have no optional fields — a struct
containing `?` bound to a Classic/CAN transport is a codegen error, never
silently default-filled; and Classic has no native tagged union — a `union` on a
Classic-bound payload is a codegen error in v0.1 (a generated tag + padded-body
convention is a possible later opt-in). A normative **UCUM → AUTOSAR unit /
physical-dimension mapping table** is required and does not yet exist — open
item.

**CAN / DBC.** Signals are scaled integers by construction — factor, offset,
min, max, and bit length all derive from range + step. Strings, bytes, maps, and
anything unbounded cannot bind to CAN; bounded arrays flatten to indexed
signals. Optionality does not exist on CAN — same profile rule as Classic.

**SOME/IP.** Positional serialization — ordinals matter only as declaration
order, which §7.4 freezes anyway. Width flips shift the byte layout: breaking,
caught by `ridl-diff`. Quantized-float-as-scaled-integer is available as a
per-deployment choice (an rsdl concern, not a type property).

**Schema targets.** Because typl is a schema language, schema languages are
themselves backends: **JSON Schema 2020-12** (each named type → `$defs` entry;
ranges → `minimum`/`maximum`, step → `multipleOf`, match → `pattern`, structs →
closed objects with `additionalProperties: false`), **OpenAPI/AsyncAPI component
schemas**, **ARXML** (`<AR-PACKAGE>` chain, CompuMethods from unit types),
**DBC** (scaling/offset derived from range and step). The JSON Schema emitter is
the round-trip check on Appendix G.

---

## Appendix E — Formal Grammar (EBNF)

The typl **profile grammar** — the restriction of the family grammar accepted in
`.typl` files. Family constructs not derivable here (interactions, timing,
streams, behaviour, architecture) are rejected with TYPL-3xx.

```ebnf
(* Top-level *)
file          = package { import } { definition } ;

package       = "package" qualified_id ;

import        = "import" qualified_id [ "as" id ] ;
              (* no wildcards, no relative paths, no re-exports — ADR-0002 *)

definition    = [ "internal" ] [ "error" ] ( type_def | const_def | struct_def
                               | enum_def | enumset_def | union_def ) ;
              (* "error" valid on struct_def, enum_def, union_def only — §10.1 *)

(* ---------- Type ---------- *)

type_def      = doc_comment? "type" CamelCase_id ":" type_backing constraint? init_value? ;

init_value    = "=" literal ;                       (* declared init value, bare — §5.8 *)

type_backing  = "integer" | "float" | "string" | "bytes" | "boolean" | ucum_unit ;

ucum_unit     = (* UCUM expression: km/h, Cel, N.m, /min, %, bar, V, A ... *) ;

constraint    = "[" constraint_spec "]" ;

constraint_spec
              = scalar ".." scalar ( "step" scalar )?
              | scalar ".."
              | ".." scalar
              | int_lit
              | int_lit ".." int_lit
              | int_lit "match" pattern
              | int_lit ".." int_lit "match" pattern
              | "match" pattern
              ;

pattern       = regex_lit | SCREAMING_SNAKE_ID ;
scalar        = int_lit | float_lit | SCREAMING_SNAKE_ID ;   (* constants usable as bounds *)

(* ---------- Constants ---------- *)

const_def     = doc_comment? "const" SCREAMING_SNAKE_ID ( ":" type_ref )? "="
                ( literal | regex_lit ) ;

(* ---------- Struct ---------- *)

struct_def    = doc_comment? "struct" CamelCase_id "{" { ( field | reserved ) sep? } "}" ;
field         = doc_comment? camelCase_id ":" field_type init_value? ;
reserved      = "reserved" ( camelCase_id | int_lit ) ;   (* tombstone — §7.4 *)

field_type    = "boolean"
              | ( "integer" | "float" ) constraint?      (* inline constrained primitive;
                                                            string/bytes need a named type — §15.3 *)
              | type_ref
              | tuple_type
              | field_type "?"
              | "[" field_type ";" bound "]"                       (* array *)
              | "[" key_type ":" field_type ";" bound "]"          (* map *)
              ;

bound         = int_lit | int_lit ".." int_lit ;
key_type      = type_ref | "string" | "integer" ;
tuple_type    = "(" named_field { "," named_field } ")" ;
named_field   = camelCase_id ":" field_type ;
type_ref      = qualified_id ;                                     (* Type or pkg.Type *)

(* ---------- Enum ---------- *)

enum_def      = doc_comment? "enum" CamelCase_id "{" { ( enum_value | reserved ) sep? } "}" ;
enum_value    = doc_comment? SCREAMING_SNAKE_ID "=" int_lit ;

(* ---------- EnumSet ---------- *)

enumset_def   = doc_comment? "enumset" CamelCase_id "{" { enumset_bit sep? } "}"
              | doc_comment? "enumset" CamelCase_id ":" type_ref
              ;
enumset_bit   = doc_comment? SCREAMING_SNAKE_ID "=" int_lit ;

(* ---------- Union ---------- *)

union_def     = doc_comment? "union" CamelCase_id "{" { ( union_arm | reserved ) sep? } "}" ;
union_arm     = doc_comment? camelCase_id ":" type_ref ;

(* ---------- Doc Comments ---------- *)

doc_comment   = "/**" { doc_tag | markdown_text } "*/"
              | { "///" markdown_text newline } ;

doc_tag       = "@see" qualified_id
              | "@labels" label { "," label }
              | "@deprecated" string_lit ;

label         = SCREAMING_SNAKE_ID [ "(" SCREAMING_SNAKE_ID ")" ] ;

(* ---------- Separators, Identifiers, Literals ---------- *)

sep                = "," | newline ;
qualified_id       = id { "." id } ;
id                 = CamelCase_id | camelCase_id ;
CamelCase_id       = [A-Z][a-zA-Z0-9]* ;
camelCase_id       = [a-z][a-zA-Z0-9]* ;
SCREAMING_SNAKE_ID = [A-Z][A-Z0-9_]* ;

int_lit       = "-"? [0-9]+ ;
float_lit     = "-"? [0-9]+ "." [0-9]+ ;
string_lit    = '"' { utf8_char } '"' ;
regex_lit     = "/" { regex_char } "/" ;
bool_lit      = "true" | "false" ;
literal       = int_lit | float_lit | string_lit | bool_lit ;
```

---

## Appendix F — Prior Work: markspec typl

The first typl was drafted as a MarkSpec extension — a Type Specification DSL
binding `$Name` identifiers inside requirements entries to a kind and a shape
([markspec typl spec](https://driftsys.github.io/markspec/extensions/typl/)).
That work validated the core idea (a small, closed, range-first shape grammar
with a coded diagnostic catalogue) in a live toolchain (parser, LSP
hover/completion, JSON compile output with a corpus-level `typeRegistry`).
MarkSpec also carries **uxil**, the precursor of uxdl — relevant to the uxdl
vocabulary workshop, not to this document.

This specification is the successor, not a superset. The mapping:

| markspec typl                                           | family typl (this spec)                                             | Disposition                                                                                                                                                          |
| ------------------------------------------------------- | ------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `$Name : kind shape` bindings with sigil                | named declarations in packages (`type`, `const`, `struct`, …)       | sigil dropped — names live in the module system, not in prose                                                                                                        |
| kind vocabulary: `signal`, `event`, `command`, `stream` | **not typl** — ridl's interaction keywords over the `interact` core | moved up a layer                                                                                                                                                     |
| kind `state`                                            | **not typl** — rmdl territory                                       | moved up a layer                                                                                                                                                     |
| kind `config`                                           | ridl `final` (provisioned constant)                                 | moved up a layer — `final` confirmed over `config`: it names the consumer contract (immutable, cacheable), not the provider workflow; see concept-note naming ledger |
| kind `const`                                            | `const`                                                             | kept                                                                                                                                                                 |
| kind `value` (default)                                  | `type` / struct field                                               | kept, made explicit                                                                                                                                                  |
| kind `document`                                         | `struct`                                                            | folded in                                                                                                                                                            |
| kind `namespace` + `$.` relative refs + published tier  | `package` / `import` (ADR-0002 `ns` core)                           | superseded — packages give declared-once, absolute citation, and ownership corpus-wide                                                                               |
| `int[0..300]`, `float[0..1.0]` postfix ranges           | `integer [0..300]`, `float [0.0..1.0 step s]`                       | kept in spirit; family `name : Type [constraint]` syntax; `step` added                                                                                               |
| `string[3..6]`, `bytes[32]` length shapes               | `string [3..6]`, `bytes [32]`                                       | kept                                                                                                                                                                 |
| `pattern /re/` shape                                    | `match /re/` inside the string constraint                           | merged into the constraint; named regex constants added                                                                                                              |
| `'low' \| 'mid' \| 'high'` literal-union enums          | `enum` (integer-backed)                                             | **not carried over** — string enums are an open question (§17.1)                                                                                                     |
| `{ field: shape }` records                              | `struct`                                                            | kept; structs are named and closed                                                                                                                                   |
| `element[](min..max)` arrays                            | `[T; min..max]`                                                     | kept; bounds made mandatory                                                                                                                                          |
| `shape?` optional                                       | `field : T?`                                                        | kept                                                                                                                                                                 |
| typedef `type Name = shape`                             | `type Name : backing [constraint]` (scalars) / named composites     | split by composite kind                                                                                                                                              |
| entry-local scope tier                                  | not applicable — no prose entries; packages are the only scope      | superseded                                                                                                                                                           |
| TYPL-001…012 diagnostic catalogue                       | TYPL-0xx…4xx coded catalogue (§16)                                  | **practice adopted**, codes renumbered by area                                                                                                                       |
| four Markdown surfaces (fence, bullet, inline, table)   | not adopted — typl lives in `.typl` files                           | MarkSpec remains a _consumer_: a requirements corpus can cite typl symbols, and a future markspec bridge can validate `` ```ridl `` fences against a workspace       |

**What survives unchanged in spirit:** closed vocabularies over extensible ones,
ranges as first-class shape (not an afterthought annotation), a small shape
grammar a reader can hold in their head, and machine-checked diagnostics with
stable codes.

---

## Appendix G — Coverage Analysis: JSON Schema and Other Schema Languages

### G.1 Method

typl claims to be a schema language, so the benchmark is: can typl express what
a contract author reaches for in
[JSON Schema 2020-12](https://json-schema.org/specification) — the latest
published dialect (the in-progress "stable" rewrite does not change the
validation vocabulary materially)? The matrix covers the entire 2020-12
validation and annotation vocabulary. ✓ covered, ≈ covered differently (usually
stricter), ✗ not expressible (deliberate or open).

### G.2 JSON Schema 2020-12 coverage matrix

| JSON Schema keyword                                           | typl equivalent                                                                        | Status                                                                                                                                                                        |
| ------------------------------------------------------------- | -------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `type: boolean/integer/number/string`                         | `boolean`, `integer`, `float`, `string`                                                | ✓                                                                                                                                                                             |
| `type: null` / nullable                                       | `?` optionality (absence; typl has no null value)                                      | ≈ stricter                                                                                                                                                                    |
| `type: array`                                                 | `[T; N]`, `[T; min..max]`                                                              | ✓ bounds mandatory                                                                                                                                                            |
| `type: object` (free-form)                                    | `struct` (closed) or `[K:V; min..max]` map                                             | ≈ stricter                                                                                                                                                                    |
| `minimum` / `maximum`                                         | `[min..max]`                                                                           | ✓                                                                                                                                                                             |
| `exclusiveMinimum` / `exclusiveMaximum`                       | —                                                                                      | ✗ open question §17.2                                                                                                                                                         |
| `multipleOf`                                                  | `step` (quantization anchored at range min)                                            | ✓                                                                                                                                                                             |
| `minLength` / `maxLength`                                     | `string [min..max]`                                                                    | ✓                                                                                                                                                                             |
| `pattern`                                                     | `match /re/` or named regex `const`                                                    | ✓ + reusable named patterns                                                                                                                                                   |
| `format` (uuid, email, uri, ipv4, date, …)                    | `ridl.std` named types (`Uuid`, `Email`, `Uri`, `IpV4`, `Date`, …)                     | ✓ and normative (JSON Schema `format` is annotation-only by default)                                                                                                          |
| `contentEncoding` / `contentMediaType`                        | `bytes` + doc comment                                                                  | ≈ partial                                                                                                                                                                     |
| `minItems` / `maxItems`                                       | `[T; min..max]`                                                                        | ✓ mandatory                                                                                                                                                                   |
| `uniqueItems`                                                 | —                                                                                      | ✗ open question §17.3                                                                                                                                                         |
| `prefixItems` (positional tuple)                              | named tuples `(a: T, b: U)`                                                            | ≈ named, not positional — deliberate                                                                                                                                          |
| `contains` / `minContains` / `maxContains`                    | —                                                                                      | ✗ deliberate (validation logic, not shape)                                                                                                                                    |
| `properties` + `required`                                     | `struct` fields; optional via `?` (required is the default)                            | ✓ inverted default, stricter                                                                                                                                                  |
| `additionalProperties: false`                                 | implicit — structs are closed                                                          | ✓ by construction                                                                                                                                                             |
| `additionalProperties: <schema>` / `patternProperties`        | bounded map `[K:V; min..max]` with `match`-constrained key type                        | ≈                                                                                                                                                                             |
| `propertyNames`                                               | map key = named string type with `match`                                               | ✓                                                                                                                                                                             |
| `minProperties` / `maxProperties`                             | map bounds                                                                             | ✓                                                                                                                                                                             |
| `dependentRequired` / `dependentSchemas` / `if`/`then`/`else` | —                                                                                      | ✗ deliberate: conditional shape is undecidable for width inference and hostile to certification; cross-field invariants belong to `expr` contracts in higher profiles (§17.7) |
| `enum` (numeric)                                              | `enum`                                                                                 | ✓                                                                                                                                                                             |
| `enum` (string literals)                                      | —                                                                                      | ✗ open question §17.1                                                                                                                                                         |
| `const` (fixed value in schema position)                      | `const` declarations; fixed-value field via `[N]` exact forms                          | ≈                                                                                                                                                                             |
| `oneOf` (discriminated)                                       | `union` (tagged)                                                                       | ✓ stricter — tag is structural                                                                                                                                                |
| `anyOf` / `allOf` / `not`                                     | —                                                                                      | ✗ deliberate: open-world combinators break the faithful-codegen guarantee (no clean proto/FlatBuffers/DBC mapping)                                                            |
| `unevaluatedItems` / `unevaluatedProperties`                  | irrelevant — structs closed, arrays homogeneous by construction                        | ✓ by construction                                                                                                                                                             |
| `$ref` / `$defs`                                              | named types + qualified imports                                                        | ✓ stronger: versioned, package-scoped, cycle-checked                                                                                                                          |
| `$id` / `$anchor` / `$dynamicRef`                             | package + name is the identity                                                         | ≈ superseded by module system                                                                                                                                                 |
| recursive schemas                                             | —                                                                                      | ✗ deliberate (§7.3): unbounded wire size                                                                                                                                      |
| `title` / `description`                                       | doc comments                                                                           | ✓                                                                                                                                                                             |
| `default`                                                     | bare `= v` (init value, §5.8)                                                          | ✓ type-checked (JSON Schema does not validate `default`)                                                                                                                      |
| `deprecated`                                                  | `@deprecated "reason"`                                                                 | ✓ reason mandatory in practice                                                                                                                                                |
| `examples`                                                    | — (doc-comment code blocks)                                                            | ≈                                                                                                                                                                             |
| `readOnly` / `writeOnly`                                      | not a type property — direction lives on interactions (ridl `final`, signal direction) | ≈ relocated by design                                                                                                                                                         |
| vocabularies / meta-schemas / `$schema`                       | one closed grammar + label profiles                                                    | ✗ deliberate — no user-extensible validation vocabulary                                                                                                                       |

**Verdict.** typl covers the JSON Schema constructs that describe _data shape_ —
everything a codegen target can faithfully realise — and adds what JSON Schema
lacks: physical units, quantization (`step` as more than `multipleOf` — it
drives width inference), wire-width inference, bitfields (`enumset`), a real
module system with declared-once semantics, and type-checked init values. What
it refuses is JSON Schema's _validation-logic_ stratum (conditionals,
combinators, `contains`, unevaluated*) — deliberately, because those constructs
have no faithful mapping onto proto/FlatBuffers/CAN and defeat static width/size
guarantees. The honest gaps to track are the four in §17: string enums,
exclusive bounds, uniqueItems, and recursion.

### G.3 Other prior art consulted

| Language                           | What typl takes / how it differs                                                                                                                                                                                       |
| ---------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **ASN.1** (X.680) with constraints | the closest ancestor in spirit: subtype constraints (`INTEGER (0..255)`) driving encoding width (PER) is exactly typl's range→width inference. typl trades ASN.1's enormous surface for a closed 6-declaration grammar |
| **CDDL** (RFC 8610)                | compact shape DSL over CBOR; ranges and `.size`/`.regexp` control operators parallel typl constraints; CDDL allows unbounded recursion — typl does not                                                                 |
| **XSD**                            | facets (`minInclusive`, `pattern`, `length`) map to typl constraints; typl rejects XSD's inheritance/extension model in favour of composition                                                                          |
| **Protocol Buffers**               | the codegen contract baseline; typl is deliberately richer (ranges, units, bounds) precisely where proto is silent, and its emitter targets proto3 (Appendix D)                                                        |
| **FlatBuffers / Avro / Thrift**    | fixed-width scalars and required bounds validate typl's transport-layer mapping; Avro's named-types + namespace model parallels the package system                                                                     |
| **Franca IDL / AUTOSAR ARXML**     | automotive precedent: ARXML CompuMethods and data constraints are the target of typl's unit types and ranges; Franca's typeCollection ≈ a pure typl package                                                            |
| **DBC**                            | signal scaling/offset/min/max are derivable from unit type + range + step — the rest-bus generation path                                                                                                               |
| **TypeSpec** (Microsoft)           | a modern "one schema source, many emitters" language — same architecture as typl→IR→backends; TypeSpec chooses an open decorator/extensibility model where typl chooses a closed grammar with label profiles           |
| **CUE**                            | values-as-types and lattice unification are powerful but undecidable for width inference; typl keeps constraints first-class but static                                                                                |
| **Zod / valibot**                  | runtime-validator ergonomics (`.min()`, `.regex()`, branded types) confirm the demand for constraint-first typing; typl moves the same checks to compile time and codegen                                              |

---

## Appendix H — Glossary

| Term                               | Definition                                                                                                                                                                                                                                                          |
| ---------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **RIDL** (capitals)                | the platform and family name; **typl** is this language, the family's vocabulary layer and root of the lattice                                                                                                                                                      |
| **family**                         | the five languages — typl, ridl, uxdl, rmdl, rsdl — sharing one grammar, one toolchain, one IR                                                                                                                                                                      |
| **profile**                        | the restriction of the family grammar accepted by a file extension; `.typl` accepts type declarations only; `.rxdl` is the total profile accepting every layer                                                                                                      |
| **profile purity**                 | the `ridl.toml` policy restricting a package to one profile's declarations — enforced per package, not by the grammar                                                                                                                                               |
| **core**                           | a reusable semantic unit beneath the surface languages: `ns` (namespacing), `typl-core` (types), `expr` (predicates), `time` (timing), `interact` (interactions)                                                                                                    |
| **vocabulary layer**               | typl's role: the types, units, ranges, constants, and composites every other layer describes things _with_                                                                                                                                                          |
| **package**                        | the unit of namespace, visibility, versioning, and codegen — a directory whose files share one `package` declaration (ADR-0002)                                                                                                                                     |
| **workspace**                      | a coordinated set of packages with one lockfile and one `[workspace]` manifest                                                                                                                                                                                      |
| **manifest / lockfile**            | `ridl.toml` (declares imports, profile purity, defaults) / `ridl.lock` (pins remote imports by content hash)                                                                                                                                                        |
| **primitive**                      | one of the five built-in scalars: `boolean`, `integer`, `float`, `string`, `bytes` — backing material, restricted as direct field types                                                                                                                             |
| **named type**                     | a `type` declaration: a constrained primitive or unit type carrying domain meaning; the normal building block for fields                                                                                                                                            |
| **backing**                        | the primitive (or UCUM unit, itself float-backed) underneath a named type                                                                                                                                                                                           |
| **unit type**                      | a named type whose backing is a UCUM unit expression (`km/h`, `Cel`) — carries dimensional identity into every layer                                                                                                                                                |
| **UCUM**                           | Unified Code for Units of Measure — the unit vocabulary standard (also used by OpenTelemetry, HL7 FHIR)                                                                                                                                                             |
| **constraint**                     | the `[ ]` clause on a type: range, `step`, length, `match` — closed-form by design (no general expressions, §17.7)                                                                                                                                                  |
| **range**                          | closed (inclusive) `[min..max]` bounds; the semantic truth from which wire width is inferred                                                                                                                                                                        |
| **step**                           | quantization: valid values are `min + n·step`; drives float width inference and the scaled-integer wire form                                                                                                                                                        |
| **quantized float**                | a float with range + step — losslessly representable as a scaled integer (`raw = (value − min)/step`), the CAN/AUTOSAR wire form                                                                                                                                    |
| **`match`**                        | the constraint keyword binding a string type to a regex pattern (named `const` or inline literal)                                                                                                                                                                   |
| **width inference**                | the compiler deriving the narrowest safe wire type from a declared range/step — computed once, passed to all backends                                                                                                                                               |
| **language layer**                 | the generated in-code representation: always widest (`int64`/`float64`) so arithmetic cannot overflow storage                                                                                                                                                       |
| **transport layer**                | the on-wire representation: narrowest width the range provably fits                                                                                                                                                                                                 |
| **wire type**                      | a concrete transport width (`uint8` … `float64`); inferred from the range and never written in source — the names are not typl keywords (§5.6)                                                                                                                      |
| **width flip**                     | the evolution hazard where widening a range crosses a width boundary and silently changes the wire type — caught by `ridl-diff`; a source-level pre-emption (a deferred `wire` floor) is §17.11                                                                     |
| **nominal typing**                 | §5.7: every named type is distinct from its backing and from all other types; no implicit conversion — the foundation of unit safety                                                                                                                                |
| **struct**                         | a named composite with a fixed, **closed** set of named fields — no unknown properties, ever                                                                                                                                                                        |
| **optional (`?`)**                 | a field that may be _absent_ (typl has no null); inverted from JSON Schema — presence is the default                                                                                                                                                                |
| **init value**                     | the value a consumer holds before anything real arrives — declared via a bare `= value` suffix or derived per §5.8; the seed of every ridl signal channel. Carries no keyword at the typl/ridl layer (`default` retired; `init` is rmdl's memory-seed keyword only) |
| **enum / enumset**                 | closed set of integer-backed values (single selection) / named bitfield (multiple simultaneous flags, width from highest bit)                                                                                                                                       |
| **union**                          | discriminated (tagged) composite — exactly one named-type arm active; never untagged                                                                                                                                                                                |
| **error type**                     | an `enum`, `struct`, or `union` carrying the `error` modifier — failure vocabulary; ordinary data with one extra bit of meaning                                                                                                                                     |
| **result union**                   | a two-arm union of one success arm + one error-typed arm — the family's `Result<T, E>` without generics; the entire functional-error mechanism                                                                                                                      |
| **tuple**                          | anonymous composite of named fields used inline; `()` is the unit type                                                                                                                                                                                              |
| **bounded**                        | the invariant that every typl composite has a statically known maximum wire size — why collections require bounds and recursion is an error                                                                                                                         |
| **ordinal**                        | a field's/arm's implicit 1-based declaration-order identity — the source of proto field numbers and FlatBuffers ids (§7.4)                                                                                                                                          |
| **`reserved`**                     | tombstone keeping a retired field's ordinal (or enum wire value) occupied so it is never reused                                                                                                                                                                     |
| **append-only**                    | the evolution discipline: new fields at the end, deletions by tombstone, reorder = wire break caught by `ridl-diff`                                                                                                                                                 |
| **one-definition rule**            | a public name is declared exactly once, in exactly one package, importable everywhere — no re-exports, no wildcards                                                                                                                                                 |
| **`internal`**                     | package-private visibility; the opt-out from public-by-default                                                                                                                                                                                                      |
| **diagnostic code**                | a stable `TYPL-nnn` identifier for a compiler rule — never renumbered, never reused (practice inherited from markspec typl)                                                                                                                                         |
| **IR**                             | the stable serialized intermediate representation — resolved names, checked types with ranges/units/widths intact — consumed by every backend and tool                                                                                                              |
| **`ridlc` / `ridl` / `ridl-diff`** | the compiler (plumbing, pure source→IR) / the toolchain facade (porcelain) / the IR-comparison breaking-change gate (plumbing-grade despite living in the facade)                                                                                                   |
| **profile (assurance)**            | an external plug-in validating `@labels` vocabulary and escalating optional rules (explicit ranges/bounds) to errors — distinct from _grammar_ profile                                                                                                              |
| **SSOT**                           | single source of truth — one vocabulary from which types, validators, schemas, and documentation derive across every backend                                                                                                                                        |

---

_End of typl Language Reference v0.1.0 — Draft._
