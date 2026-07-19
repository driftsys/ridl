# expr-core Specification

**The contract-term language of the RIDL family** — one expression grammar,
written once, surfacing in `require`/`ensure` contracts (ridl §13), in the uxdl
contract positions (`action`/`fetch`), and as the expression layer of rmdl
functions and models (rmdl §4, which shares this grammar verbatim). One grammar,
two layers: the **guaranteed subset** shipped with the interface layer (V1, epic
E2) and the **function layer** that extends it (V2, story E5.1).

Version: 0.1.0 — Draft

> **Provenance.** This document closes the "expr core specification — not
> started" row of the family overview §2 inventory and the pending grammar
> reference in ridl §13 and ridl Appendix C. It is written under ADR-0008
> decision 10: E2.4 implements only the guaranteed subset and rejects every
> other form with RIDL-306; this document fixes the full contract-term grammar
> that subset is verified against. The V1 sections are **normative as
> implemented**. The V2 sections are **forward-looking by design** — this is the
> one document whose purpose is to fix the shape of a layer that lands later
> (roadmap E5.1), the same posture as the rmdl and rsdl references.

---

## Table of Contents

1. [Scope and Position in the Family](#1-scope-and-position-in-the-family)
2. [The Layer Model](#2-the-layer-model)
3. [Grammar](#3-grammar)
4. [The Rejection List](#4-the-rejection-list)
5. [Typing Rules — the Guaranteed Subset](#5-typing-rules--the-guaranteed-subset)
6. [The Reference Environment](#6-the-reference-environment)
7. [Evaluation Domains](#7-evaluation-domains)
8. [The RIDL-306 Boundary](#8-the-ridl-306-boundary)
9. [Worked Examples — the ridl §13 Contracts](#9-worked-examples--the-ridl-13-contracts)
10. [Alternatives Considered](#10-alternatives-considered)

---

## 1. Scope and Position in the Family

`expr` is the family's single expression language. It is not a sixth profile: it
is a shared core that the profiles surface in their own positions:

- **ridl** — `require`/`ensure` predicate attributes on `command` and `query`
  (ridl §13; carrier production `attribute = "require" expr | "ensure" expr`,
  ridl Appendix C; the three-form attribute model is the family general form
  working spec §4.2)
- **uxdl** — the same predicate attributes on `action` and `fetch`
- **rmdl** — the expression layer of functions and models (rmdl §4 states: "The
  expression grammar below is shared verbatim with the `expr` core")
- **typl** — future `invariant` predicates on structs, and the const-evaluable
  subset in constraint bounds (typl §17.7)

One expression, multi-executable (family overview doctrine 14): a contract
written once in `expr` runs statically where decidable, as a CI property test,
as an online observer on live flows, and as a synchronous observer in the rmdl
reference oracle (ridl §13, concept note §9.2). Everything in this document
serves that multiplicity: the grammar is small, total, side-effect-free, and
exactly evaluable, so the same term means the same thing in every execution.

This document owns the grammar, the typing rules, the reference environment, and
the evaluation domains. The carrier positions — which declarations may carry
which predicate, and the diagnostics for misplacement (RIDL-301/302) — belong to
the profile references and are not restated here.

---

## 2. The Layer Model

The grammar has two layers, released in sequence:

| Layer                 | Release | Story      | Content                                                                                                                                                          |
| --------------------- | ------- | ---------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Guaranteed subset** | V1      | E2.4/E2.12 | comparison, boolean connectives, arithmetic, enum access, tuple-field access, duration comparison — the forms ridl §13 names as guaranteed-supported             |
| **Function layer**    | V2      | E5.1       | `let` bindings, `if`/`case`/`match` expressions, total function definitions and calls, bounded combinators (`all`, `any`, `count`) over typl bounded collections |

**Layer rule (normative).** The guaranteed subset is a **strict subset** of the
function layer: every expression legal in V1 is legal in V2, parses to the same
tree, types to the same type under the same environment, and evaluates to the
same value. E5.1 extends the grammar; it never changes the meaning of a subset
term. This is ADR-0008 decision 10's forward-compatibility requirement — the
subset is never a throwaway.

The asymmetry between the layers is deliberate:

- The **V1 productions, typing rules, and evaluation domains in this document
  are frozen** — the E2.4 checker (RIDL-306 boundary included) and the E2.11
  evaluator implement them, and a change here is a contract change.
- The **V2 productions are forward-looking**: E5.1 may refine them (and adds the
  totality checks, RMDL-1xx, that only make sense with function definitions),
  but it must preserve the layer rule above. The rmdl reference §3–§4 is the
  semantic elaboration of the V2 layer; where this document and a landed E5.1
  implementation would diverge, the divergence is resolved by editing this
  document in the open, never silently.

---

## 3. Grammar

### 3.1 The guaranteed subset — V1 (normative as implemented)

The subset productions, exactly as the E2 parser implements them
(precedence-climbing; one production per precedence level):

```ebnf
expr        = or_expr ;
or_expr     = and_expr { "||" and_expr } ;
and_expr    = cmp_expr { "&&" cmp_expr } ;
cmp_expr    = add_expr [ ( "==" | "!=" | "<" | "<=" | ">" | ">=" )
                         add_expr ] ;
add_expr    = mul_expr { ( "+" | "-" ) mul_expr } ;
mul_expr    = unary_expr { ( "*" | "/" | "%" ) unary_expr } ;
unary_expr  = [ "!" | "-" ] postfix_expr ;
postfix_expr= primary { "." ( camelCase_id | SCREAMING_SNAKE_ID ) } ;
primary     = literal | duration_lit | path_head | "(" expr ")" ;
path_head   = camelCase_id | CamelCase_id | SCREAMING_SNAKE_ID ;
```

`literal` is typl's (`int_lit | float_lit | string_lit | bool_lit`, typl
Appendix E); `duration_lit` is ridl's duration literal (`int_lit` plus a
`us`/`ms`/`s` suffix, ridl §2.1).

Structural rules fixed by these productions:

- **Precedence**, loosest to tightest: `||` — `&&` — comparison — additive (`+`
  `-`) — multiplicative (`*` `/` `%`) — unary (`!` `-`) — member access (`.`).
  So `a || b && c` parses as `a || (b && c)`, `!a && b` as `(!a) && b`, and
  `a + b * c < d` as `(a + (b * c)) < d`.
- **Comparison does not chain**: `cmp_expr` admits at most one comparison
  operator, so `a < b < c` is a parse error. Write `a < b && b < c`.
- **Member access admits member names only**: after `.` the grammar accepts a
  `camelCase_id` (tuple field) or a `SCREAMING_SNAKE_ID` (enum member), never a
  `CamelCase_id`. Qualified package paths (`pkg.Name.MEMBER`) are therefore not
  expressible — references resolve through the file's imports (ADR-0002), the
  same way type references do.
- **`<` is always comparison** inside an expression. The stream-type `<T>`
  reading of `<` exists only in parameter-type and return-type position; the two
  positions never overlap.
- **Zero durations are legal in expression position.** `require window > 0ms`
  (ridl §13) depends on `0ms` as a comparison operand. RIDL-102's prohibition of
  zero durations applies to timing annotations (ridl §9, §16.1), not to
  expression operands.

### 3.2 The function layer — V2 (E5.1, forward-looking)

The V2 layer only **adds alternatives** to the subset productions — no V1
production changes shape, which is what makes the layer rule (§2) hold by
construction. Every production below is marked `V2 (E5.1)` and is illegal until
E5.1 lands (§8):

```ebnf
(* V2 (E5.1) *) expr        = or_expr | if_expr | case_expr | block_expr ;
(* V2 (E5.1) *) if_expr     = "if" expr "then" expr "else" expr ;
(* V2 (E5.1) *) case_expr   = "case" expr "{" case_arm { case_arm } "}" ;
(* V2 (E5.1) *) case_arm    = case_pattern "->" expr ;
(* V2 (E5.1) *) case_pattern = SCREAMING_SNAKE_ID          (* enum member    *)
                             | camelCase_id camelCase_id   (* union arm bind *)
                             | "some" camelCase_id | "none" (* optionals     *)
                             | "else" ;                    (* catch-all      *)
(* V2 (E5.1) *) block_expr  = "{" { let_binding } expr "}" ;
(* V2 (E5.1) *) let_binding = "let" camelCase_id "=" expr ;
(* V2 (E5.1) *) cmp_expr    = add_expr [ cmp_op add_expr
                                       | "match" string_lit ] ;
(* V2 (E5.1) *) cmp_op      = "==" | "!=" | "<" | "<=" | ">" | ">=" ;
(* V2 (E5.1) *) primary     = literal | duration_lit | path_head
                            | "(" expr ")"
                            | call_expr | combinator_call ;
(* V2 (E5.1) *) call_expr   = camelCase_id "(" [ expr { "," expr } ] ")" ;
(* V2 (E5.1) *) combinator_call = ( "all" | "any" | "count" )
                                  "(" expr [ "," fn_value ] ")" ;
(* V2 (E5.1) *) fn_value    = camelCase_id
                            | "function" "(" param_names ")" "=" expr ;
(* V2 (E5.1) *) param_names = camelCase_id { "," camelCase_id } ;
(* V2 (E5.1) *) function_def = "function" camelCase_id
                               "(" [ fn_param { "," fn_param } ] ")"
                               ":" type_ref ( "=" expr | block_expr ) ;
(* V2 (E5.1) *) fn_param    = camelCase_id ":" type_ref ;
```

Reading notes on the V2 layer (semantics owned by the rmdl reference):

- `if` is an expression and `else` is mandatory — totality (rmdl §4.2,
  RMDL-107). `case` dispatches on enums, unions, and optionals, and must be
  exhaustive, with `else` as the explicit catch-all (rmdl §4.3, RMDL-108).
- `match` in expression position is the **pattern-conformance predicate**
  (`value match "pattern"` — boolean), the expression form of typl's `match`
  constraint (typl §17.7c). It is not value dispatch — that is `case`.
- `let` binds immutable locals; no reassignment, no shadowing (rmdl §3.1,
  RMDL-104).
- A `function_def` declares a named, pure, **total** computation; it surfaces at
  rmdl package level (rmdl §3.1), never inside a contract predicate. A contract
  predicate may **call** declared total functions (`call_expr`) — and nothing
  else (§4).
- The bounded combinators named here (`all`, `any`, `count`) are the
  contract-position set (roadmap E5.1). The full function-layer set — `map`,
  `fold`, `any`, `all`, `count` over typl bounded collections — is rmdl §4.5;
  anonymous `function(…) = expr` values are legal only as combinator arguments
  (RMDL-109). Every bound is a typl bound, so every iteration count is
  statically known.
- rmdl §4.4's optional forms (`?:` and the `some`/`none` patterns above) arrive
  with the same layer; the `?:` production is fixed by E5.1 together with the
  optional typing rules.
- Keyword ownership stays with the profile references: `require`/`ensure` are
  registry words activated by ridl (ridl §2.3); the V2 words (`function`, `let`,
  `if`, `then`, `else`, `case`, `match`, …) enter through rmdl §2 and the typl
  §1.4 registry.

---

## 4. The Rejection List

The following are excluded from `expr` **at every layer** — they are not pending
features, and E5.1 does not lift them. This list is normative and permanent:

1. **Recursion**, direct or mutual (RMDL-101 once the function layer lands).
   Total functions cannot be self-referential; per-call WCET stays decidable.
2. **Loops.** No loop statement exists at any layer. Iteration exists only as
   bounded combinators over typl bounded collections (rmdl §3.2, §4.5).
3. **`last` inside functions** — and with it `init`, `now`, and `dt`
   (RMDL-102/103). These are model-layer constructs; `expr` terms are timeless
   and stateless. A computation that needs elapsed time takes it as an ordinary
   parameter.
4. **Side effects.** No assignment, no mutation, no emission, no I/O in
   expression position. Side effects are emissions and belong to models (family
   overview doctrine 13, rmdl §5.7).
5. **Calls to anything except (V2) declared total functions.** No host
   functions, no runtime intrinsics, no FFI. The callable set is closed by
   declaration, so every execution surface — checker, property runner, observer,
   oracle — can evaluate any contract without a sandbox.

---

## 5. Typing Rules — the Guaranteed Subset

These rules are normative for V1; the E2.4 checker implements them. Every
violation in E2 surfaces as RIDL-306 (§8) with a message naming the offending
form.

### 5.1 Type domains

A subset expression types into one of five domains:

| Domain       | Inhabited by                                                                                           |
| ------------ | ------------------------------------------------------------------------------------------------------ |
| **boolean**  | `bool_lit`, comparison results, boolean connectives                                                    |
| **numeric**  | a value of one named numeric type (`Speed`), a bare `integer`/`float` value, or a bare numeric literal |
| **duration** | `duration_lit` (`500us`, `10ms`, `1s`) and values of the standard `Duration` type                      |
| **enum**     | an enum-typed value; `Enum.MEMBER` access                                                              |
| **tuple**    | a tuple-typed value with named fields (a tuple-returning query's `result`)                             |

### 5.2 Nominal typing — no implicit cross-type anything

Typing is nominal per typl §5.7. Two named types never unify, even with
identical constraints: `Speed + Torque`, `speed == torque`, and every other
cross-named-type form is a type error. A **bare numeric literal unifies with any
numeric operand** — this is the same rule that makes
`const MAX_SPEED : Speed = 250.0` legal — and a bare `integer`/`float` typed
value unifies with literals and with values of the same bare primitive, never
with a named type. The subset checker unifies literals nominally; it does not
evaluate range conformance of literal operands (the test plane exercises
values).

### 5.3 Operator rules

| Operator          | Operands                                                                                      | Result           |
| ----------------- | --------------------------------------------------------------------------------------------- | ---------------- |
| `==` `!=`         | two operands unifying in one domain: boolean, numeric of one type, duration, or one enum type | boolean          |
| `<` `<=` `>` `>=` | two operands unifying in one **ordered** domain: numeric of one type, or duration             | boolean          |
| `&&` `\|\|`       | boolean, boolean                                                                              | boolean          |
| `!`               | boolean                                                                                       | boolean          |
| `+` `-` `*` `/`   | numeric operands, nominally consistent (one named type; literals unify)                       | the operand type |
| `%`               | as arithmetic, and **integer-backed operands only**                                           | the operand type |
| unary `-`         | numeric                                                                                       | the operand type |
| `Enum.MEMBER`     | postfix `SCREAMING_SNAKE_ID` member on an enum type name in scope                             | that enum type   |
| `value.field`     | postfix `camelCase_id` member on a tuple-typed value, naming one of its fields                | the field's type |

Rules the table implies, stated explicitly:

- **The root of a `require`/`ensure` expression must be boolean.** A non-boolean
  root (`require 3`) is an error.
- **Duration supports comparison only.** ridl §13 guarantees duration
  _comparison_; duration arithmetic (`window + 10ms`) is outside the subset and
  arrives with the function layer's unit discipline (rmdl §3.3).
- **Enums support equality only.** Ordering enum members
  (`gear < GearPosition.DRIVE`) would depend on member numbering — an evolution
  hazard — and is not planned at any layer.
- **Arithmetic result type**: when any operand is a named type, every
  non-literal operand must be of that one named type and the result is that
  type; when all operands are literals, the result is a bare numeric literal
  value (which continues to unify).
- **Scalar multiplication across types** (`Speed * float`-typed value, rmdl
  §3.3) is not in the subset — in E2 every non-literal arithmetic operand must
  be of one type. The function layer lifts exactly the rmdl §3.3 cases.
- **String and bytes operands have no operator in the subset.** `string_lit`
  parses (it is a typl literal), but any operator over strings or bytes is
  outside the subset; the V2 `match` predicate is the planned string form.

---

## 6. The Reference Environment

A contract expression resolves names against a fixed environment — and **nothing
else**. There is no ambient scope, no global mutable state, no platform
introspection. For the ridl `require`/`ensure` position (the E2 implemented
carrier), the environment is, in resolution order:

1. **Parameters** of the enclosing `command`/`query` (`camelCase_id`).
2. **`result`** — the query's return value; **ensure only**. In a `require`,
   `result` does not exist.
3. **The enclosing interface's own signals** (`camelCase_id`); **require only**.
   A `require` reads the provider's latest published value (ridl §4.4's
   last-value guarantee makes this well-defined). The ridl §13 table scopes
   `ensure` to `result` and parameters, so a signal read in an `ensure` is an
   error.
4. **Constants** — package-local and imported `SCREAMING_SNAKE_ID` constants
   (ADR-0002 import rules).
5. **Enum types** (`CamelCase_id`), for `Enum.MEMBER` access.

Any other name is an unresolved reference (RIDL-306 in E2). Other carrier
positions (uxdl `action`/`fetch`; rmdl function and model contracts, rmdl §9.2)
define their own environments in their references, under the same
closed-environment principle.

---

## 7. Evaluation Domains

Evaluation semantics are normative for V1: the property runner (E2.11) and later
the rmdl reference oracle (E5) must agree bit-for-bit, so the domains are exact
— **no IEEE-754 arithmetic anywhere in the contract plane**.

- **Numeric values are exact rationals.** Literals, constants, and sampled
  values evaluate as exact rational numbers (the typl exactness discipline:
  ranges, steps, and constants are exact decimals — typl §4–§5 — carried by the
  toolchain's exact-scalar representation). `0.1 + 0.2 == 0.3` holds. Wire-width
  and float-representation concerns (typl §4.3) are boundary concerns; they do
  not leak into contract evaluation.
- **Durations are exact microsecond counts.** `1s` = `1000ms` = `1000000us`;
  duration comparison compares the counts. This matches platform time (`int64`
  microseconds, ridl §3.1) and the IR's exact-decimal microsecond timing rule
  (ADR-0008 decision 12).
- **Enum values** evaluate as the member's declared value within its enum type;
  equality is member equality.
- **Booleans**: `&&` and `||` evaluate left-to-right and **short-circuit**. With
  the division fault below, short-circuiting is observable
  (`x != 0.0 && 1.0 / x > 2.0` never faults), so it is normative.
- **Integer division and remainder** over integer-backed operands follow C17
  (rmdl §4.1): `/` truncates toward zero, `%` takes the dividend's sign.
  Division over float-backed operands is exact rational division.
- **Totality.** Evaluation of a well-typed subset expression terminates on every
  environment — there is nothing in the grammar that can diverge. The single
  evaluation fault is **division by zero** (`/` or `%` with a zero divisor),
  which surfaces as a defined fault (the rmdl §8 step-fault lineage: guarding
  divisors is the author's obligation), never as undefined behavior.

---

## 8. The RIDL-306 Boundary

In E2, **any expression form outside the guaranteed subset is RIDL-306** (error)
— one code for the whole boundary, with a message naming the offending form.
That includes, non-exhaustively:

| Form                                                     | Example                              |
| -------------------------------------------------------- | ------------------------------------ |
| any V2 production (§3.2)                                 | `if`, `let`, `case`, calls, `all(…)` |
| unresolved reference / name outside the environment (§6) | `require unknownName > 0`            |
| `result` in a `require`; a signal read in an `ensure`    | `ensure currentSpeed >= 0.0`         |
| cross-named-type arithmetic or comparison (§5.2)         | `require speed + window > 0`         |
| duration arithmetic (§5.3)                               | `require window + 10ms < 1s`         |
| `%` over non-integer-backed operands (§5.3)              | `require speed % 0.5 == 0.0`         |
| non-boolean root (§5.3)                                  | `require 3`                          |
| string/bytes operands; enum ordering (§5.3)              | `require name == "x"`                |

The boundary is **lifted per-form as E5.1 lands**: when a V2 form is
implemented, it leaves RIDL-306's scope; forms that remain illegal in the
function layer move to their profile-assigned codes (RMDL-1xx, rmdl §11.1). The
§4 rejection list is never lifted. Related but distinct: RIDL-305 (warning)
flags an `ensure` that never references `result` — a well-typed but suspicious
contract, not a boundary violation.

---

## 9. Worked Examples — the ridl §13 Contracts

Every guaranteed-subset example in the ridl reference, type-checked under §5 and
§6. The declarations are ridl §13/§14.0's:

- `command setRange(min: Speed, max: Speed)`
- `query getAverageSpeed(window: Duration): Speed`
- `command setGear(position: GearPosition)`, inside an interface that declares
  `signal currentSpeed : Speed`
- `const MAX_SPEED : Speed` (typl §5.7)

| Contract                                                         | Walk                                                                                                                                                                                                                   | Root    |
| ---------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------- |
| `require min < max`                                              | `min`, `max` — parameters, numeric `Speed`; ordering over one named type                                                                                                                                               | boolean |
| `require max <= MAX_SPEED`                                       | `max` — parameter `Speed`; `MAX_SPEED` — constant `Speed` (env item 4); ordering over one named type                                                                                                                   | boolean |
| `require window > 0ms`                                           | `window` — parameter `Duration`; `0ms` — duration literal (legal at zero in expression position, §3.1); duration ordering                                                                                              | boolean |
| `ensure result >= 0.0`                                           | `result` — `Speed` (ensure position, env item 2); `0.0` — literal, unifies with `Speed`; ordering. References `result`, so no RIDL-305                                                                                 | boolean |
| `require position != GearPosition.PARK \|\| currentSpeed == 0.0` | `position` — parameter, enum `GearPosition`; `GearPosition.PARK` — enum access, same enum; equality. `currentSpeed` — own signal (require position, env item 3), `Speed`; `0.0` unifies; equality. `\|\|` over boolean | boolean |

And the tuple-field form (the sixth guaranteed form, ridl §13; example shape
from the toolchain plan): given `query getRange(): (min: Speed, max: Speed)`,
the contract `ensure result.min >= 0.0` types as — `result` tuple (env item 2);
`.min` field access → `Speed`; literal unifies; ordering → boolean.

All six are well-typed under the subset rules with boolean roots.

---

## 10. Alternatives Considered

Recorded per the working-memory doctrine, so the rationale survives the session
that produced it:

1. **Quoted / string predicates** (`require "min < max"` — assertions as
   annotation text, the proto-custom-option / doc-comment-assertion style).
   Rejected: an unchecked string cannot be type-checked, canonicalized, or
   diffed; it fails the general-form deletion test (an attribute must have a
   machine consumer, and a machine cannot consume what it cannot parse); and it
   silently rots as the interface evolves. The whole value of `require`/
   `ensure` is that the compiler owns them.
2. **The full E5 grammar now** (implement functions, conditionals, and
   combinators in E2). Rejected: sequencing (ADR-0004, ADR-0008 decision 10).
   The function layer is L-sized, needs the totality checks (RMDL-1xx) that only
   make sense beside rmdl, and has no E2 consumer — the interface layer needs
   predicates, not programs. E2 ships the subset that the contract positions
   need; E5.1 extends it in place.
3. **An external assertion language** (OCL, JML-style clauses, CEL, or any
   embedded third-party expression language). Rejected: the one-grammar doctrine
   — one platform, five languages, **one grammar**, one IR. A second expression
   language would double the lexicon for non-programmer audiences, break the
   multi-execution story (the same term must run in the rmdl oracle, which
   speaks this grammar natively), and pull a foreign toolchain inside the ISO
   26262 tool-qualification boundary.
