# Skill outline — _Authoring RIDL_ (agent knowledge layer)

Stub / table of contents for the agent-facing skill described in ADR-0005 Layer
A. This is the **shape**, distilled from the typl and ridl references — not the
finished skill. Sections marked _(seed filled)_ already contain the real
distilled content; the rest are placeholders showing what goes where. The
finished skill should be **generated from the reference docs** (ADR-0005 §2) so
it never drifts.

Target length when built: ~600–900 lines. Design principle: **dense decision
tables + worked examples over prose.** An agent uses this to _generate_; it
reaches for the MCP (`ridl_check`, `ridl_diff`) to _verify_.

---

## Skill metadata (frontmatter)

```yaml
name: authoring-ridl
description: >
  Write and evolve RIDL-family contracts — typl types, ridl interfaces,
  and (later) uxdl/rmdl/rsdl. Use whenever creating or editing .typl,
  .ridl, .uxdl, .rmdl, .rsdl, or .rxdl files, or turning a spec into an
  interface/type definition. Covers syntax, the five interaction kinds,
  errors-as-data, units/ranges, and append-only evolution.
when_to_use: >
  Triggered by mention of RIDL, typl, ridl, "interface"/"contract"/"signal"/
  "type" in a systems-modeling context, or any *.ridl/*.typl/*.rxdl file.
pairs_with: the ridl MCP server (verify with ridl_check, evolve with ridl_diff)
```

---

## 0. The 60-second model _(seed filled)_

- **One family, five profiles, one grammar.** `typl` = vocabulary (types, units,
  ranges, constants). `ridl` = system interactions. `uxdl` = user interactions.
  `rmdl` = behaviour. `rsdl` = architecture/wiring. Extension selects the
  profile; `.rxdl` accepts all layers (best for one-file examples).
- **Lattice (never reference upward):** `typl ← {ridl, uxdl, rmdl} ← rsdl`.
- **Payloads are always named typl types.** Interactions reference vocabulary;
  they never define shapes inline.
- **Errors are data.** No `throws`, no exceptions anywhere. A fallible query
  returns a _result union_.
- **Ranges + units are the truth**; wire width is derived by the compiler.
- **Evolution is append-only** with `reserved` tombstones; `ridl-diff` is the
  gate.

---

## 1. Hard rules — never violate _(seed filled — this is also the rules file)_

1. No semicolons. Newline or comma separates; trailing comma OK.
2. Every field/payload is a **named typl type** — never bare `string`/`bytes` as
   a field type (define a `type`); never an inline shape. (`TYPL-208`)
3. `integer`/`float`/`string`/`bytes` always carry explicit bounds; arrays and
   maps always carry bounds. (`TYPL-101/102/103/201/202`)
4. Errors are data: no `throws`. A query that can fail returns a **result
   union** (one success arm + one `error` arm). (`ridl §10.1`)
5. `command` never returns a value — use `query`. Command outcomes are observed
   as state (a signal). (`RIDL-104`)
6. `query` never returns `()` — use `command`. (`RIDL-105`)
7. No interface inheritance / no `extends`. Duplicate the interactions; let
   `ridl-diff` guard each contract. (`ridl §14.1`)
8. No upward references in the lattice — compile error.
9. Evolution: append new fields/interactions **at the end**; delete via
   `reserved <name>` tombstone; never reorder, never reuse an ordinal.
   (`TYPL-210`, `RIDL-401`)
10. Naming: `CamelCase` types, `camelCase` fields/interactions,
    `SCREAMING_SNAKE` enum values/constants, `lowercase.dot` packages.
11. Sigil poverty — the whole budget is `@ ? ?: -> [] <>`. Prefer words.
12. `require`/`ensure` only on `command`/`query`; `ensure` only on `query`.
    (`RIDL-301/302`)

---

## 2. typl — the vocabulary layer

### 2.1 Primitives and named types _(seed filled)_

Five primitives: `boolean`, `integer`, `float`, `string`, `bytes`. Never used
directly as field types (except stream element types) — wrap in a `type`.

```ridl
type Speed   : km/h    [0.0..250.0 step 0.5]        // unit type (UCUM)
type Counter : integer [0..65535]
type Gain    : float   [0.0..1.0 step 0.01]
type Vin     : string  [17 match VIN_PATTERN]
type Frame   : bytes   [8]
```

- **Unit types** use UCUM (`km/h`, `Cel`, `N.m`, `/min`, `bar`, `V`, `%`).
- **Ranges** are closed/inclusive; `step` = quantization.
- **Nominal typing**: `Speed` and `Torque` never interchange; no implicit
  conversion. This is the point — it makes unit safety real.

### 2.2 Constants, ranges, `step`, `wire`, init values

- `const NAME : Type = value`; reusable in bounds, `match`, defaults.
- `wire` clause pins wire-width headroom (evolution floor).
- Init values: declared `[ default = v ]` or derived (table in §5.8 of ref).

### 2.3 Composites — struct / enum / enumset / union / tuple / collections

- struct = **closed** set of named fields; `?` = optional (absence, no null).
- enum = integer-backed; enumset = bitfield; union = tagged, arms are named
  types only.
- collections `[T; N]`, `[T; min..max]`, maps `[K:V; min..max]` — bounds
  mandatory. No recursion.

### 2.4 Errors as vocabulary _(seed filled)_

```ridl
error enum CalError { SENSOR_UNAVAILABLE = 0, VEHICLE_MOVING = 1 }
union CalOutcome { ok: CalReport, err: CalError }   // Result<CalReport,CalError>
```

- `error` modifier valid on `enum`/`struct`/`union` only.
- **Result union** = exactly two arms, one success + one error. The family's
  entire functional-error mechanism; no generics, no exceptions.

### 2.5 Packages, imports, visibility

- One `package` per file, mirrors directory. `import pkg.Name [as Alias]` — no
  wildcards, no relative, no re-export. `internal` = package-private.

---

## 3. ridl — the interaction layer

### 3.1 The five kinds — the load-bearing decision table _(seed filled)_

| Kind      | Pattern     | Use when                                                            | Initiator          | Timing                  | Can fail (functionally)?          |
| --------- | ----------- | ------------------------------------------------------------------- | ------------------ | ----------------------- | --------------------------------- |
| `signal`  | pub/sub     | continuous **state**; latest sample matters, may miss intermediates | provider publishes | `@Xms` or `@[min..max]` | no — payload may be an error type |
| `event`   | pub/sub     | discrete **occurrence**; every one matters, queued                  | provider publishes | `@[min..max]` only      | no                                |
| `command` | RPC         | fire-and-forget **action**; no reply                                | consumer calls     | none                    | no (has no return)                |
| `query`   | RPC         | **request/response**; reply mandatory                               | consumer calls     | none                    | yes — via result-union return     |
| `fixed`   | provisioned | value held for the software-instance lifetime, cacheable            | neither            | none                    | no                                |

**The one distinction to get right:** state → `signal`; occurrence → `event`. If
you are tempted to publish an "event" carrying full current state, it is a
`signal`. Observable command outcomes come back as **state (a signal)**, never a
return value (CQRS: commands mutate, state reports).

### 3.2 Timing `@` _(seed filled)_

| Bound       | Signal                                     | Event                   |
| ----------- | ------------------------------------------ | ----------------------- |
| lower `min` | debounce (suppress faster than min)        | throttle (min interval) |
| upper `max` | refresh ceiling (republish even unchanged) | TTL (discard if stale)  |

`@Xms` = strict periodic, **signal only** (drives rmdl clocks). Untimed →
default `@[100ms..1000ms]` + warning. `@Xms` on an event is an error
(`RIDL-103`).

### 3.3 Errors — three strata _(seed filled)_

- **Stratum 1 (functional)** — data: result-union return on a query. The only
  one you write.
- **Stratum 2 (contract)** — implicit: `INVALID_VALUE`, `PRECONDITION_FAILED`,
  `CONTRACT_BROKEN`, `UNKNOWN_INTERACTION`. Derived from typl constraints +
  `require`/`ensure`; never declared.
- **Stratum 3 (transport)** — invisible to the language; runtime-supplied.

### 3.4 Contracts — `require` / `ensure`

- `require` (pre) on command/query; `ensure` (post, over `result`) on query.
- Side-effect-free; reference params, `result`, constants, the interface's own
  signals.

### 3.5 Interface vs service, streams, evolution

- `interface` = abstract shape (no identity). `service` = global named published
  declaration, addressed `service.member`.
- Streams `<T>` only in command/query params and query returns; never on
  signal/event.
- Evolution: ordinals by declaration order, append-only, `reserved` tombstones.

---

## 4. uxdl / rmdl / rsdl — later profiles _(placeholder)_

- uxdl: `view` binds a service; `display <- signal`, `activate -> command`.
  (vocabulary still unsettled — mark examples non-normative)
- rmdl: `model realizes` interface; `last`/`init`, `case`/`when`/`match`,
  `now`/`dt`; emits events, never calls.
- rsdl: `instance`, `bind`, `deploy on … transport …` (manifest-shaped first).

---

## 5. Common mistakes → diagnostic codes _(seed filled)_

The agent's self-check table — each row is a mistake the model is likely to make
from general-language priors, and the code the compiler will raise.

| Likely mistake (from priors)          | Correct RIDL                                | Code              |
| ------------------------------------- | ------------------------------------------- | ----------------- |
| ending a line with `;`                | drop it                                     | — (parse)         |
| `field : string` inline               | define `type Name : string [1..64 match …]` | `TYPL-208`        |
| `integer` with no range               | `integer [min..max]`                        | `TYPL-101`        |
| array `[T]` with no bound             | `[T; min..max]`                             | `TYPL-201`        |
| `throws SomeError` / exceptions       | result-union return                         | `RIDL-303` idea   |
| `command … : ResultType`              | make it a `query`, or observe state         | `RIDL-104`        |
| `query … : ()`                        | make it a `command`                         | `RIDL-105`        |
| `interface B extends A`               | duplicate interactions; no inheritance      | `ridl §14.1`      |
| `@10ms` on an `event`                 | events use `@[min..max]` only               | `RIDL-103`        |
| reordering/inserting fields           | append at end; `reserved` for deletes       | `TYPL-210` / diff |
| `require` on a `signal`               | signals validate via typl constraints only  | `RIDL-301`        |
| putting a timestamp/seq in a payload  | envelope already carries it                 | `RIDL-406`        |
| union arm of a primitive              | arms are named types                        | `TYPL-204`        |
| recursive struct                      | model with bounded array + indices          | `TYPL-206`        |
| unit mismatch (`Torque` into `Speed`) | nominal — never assignable                  | `typl §5.7`       |

---

## 6. Worked examples _(seed: reuse the cruise-control chain)_

One complete `.rxdl` per example, each round-tripped through `ridl_check` before
inclusion. Minimum set:

1. **Vocabulary only** (`.typl`) — units, enums, a result union, a struct.
2. **Interface** (`.ridl`) — the five kinds, timing, a fallible query, a
   `reserved` tombstone. (Use `veh.cluster.VehicleStatus` from the ref.)
3. **One-file system** (`.rxdl`) — cruise-control: types + interface + view +
   model + wiring, showing the layers meeting.
4. **An evolution edit** — add a field + tombstone a removed one, and the
   `ridl_diff` output proving it is compatible.

---

## 7. Verify loop (how the skill hands off to the MCP) _(seed filled)_

Always close the loop — do not trust generated RIDL:

1. Write the `.typl`/`.ridl`/`.rxdl`.
2. `ridl_check(source)` → fix every error using the returned coded fix-its;
   `ridl_explain(code)` for anything unclear.
3. For edits to a published contract: `ridl_diff(old, new)` → must be exit 0
   (compatible) unless a break is intended and acknowledged.
4. Ground new work in existing symbols first: `ridl_list_interactions`,
   `ridl_describe_type`, `ridl_resolve` — never invent symbol names.

```
```
