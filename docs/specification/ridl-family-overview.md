# The RIDL Family — Overview and Index

**One platform, five languages, one grammar.** This is the entry-point document:
the map, the shared doctrines, the reading paths, the decision ledger, and the
index of open questions. It contains no normative language rules of its own —
every rule lives in exactly one reference, cited from here.

Status: living index — updated whenever a reference changes.

---

## 1. The Map

```
           typl            ← vocabulary: types, units, ranges, constants
    ┌─────────┴─────────┐
    ▼                   ▼
  ridl                rmdl    ← contracts at every boundary · behaviour
(+ rxdl spellings)
    └─────────┬─────────┘
              ▼
            rsdl             ← architecture: instances, wiring, deployment
```

One grammar, one toolchain, one IR; each language is a **profile** selected by
file extension (`.typl` `.ridl` `.rmdl` `.rsdl`, plus `.rxdl` the
**unrestricted** profile — any layer and any interaction domain). ridl describes
every boundary through five interaction families (ADR-0012); rxdl adds readable
spellings for the non-dispatch ones and no semantics; rmdl computes
contract-blind reactions; rsdl components bind them to contracts and wire
instances — rsdl never stands alone.

## 2. Document Inventory

| Document                       | Version   | Status            | Owns                                                                                                                                                                                             |
| ------------------------------ | --------- | ----------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Concept note — the RIDL family | draft     | direction-setting | motivation, cores, profiles, platform/repo/IR model, naming ledger                                                                                                                               |
| ADR-0002 — module system       | accepted  | normative         | `package`/`import`/`as`/`internal`, manifest, lockfile, resolver                                                                                                                                 |
| **typl Language Reference**    | 0.1 draft | normative         | vocabulary layer + family lexicon, keyword registry (§1.4), evolution model (§7.4)                                                                                                               |
| **ridl Language Reference**    | 0.2 draft | normative         | interaction layer + interact-core semantics: envelope, timing, init/invalid channels, errors, streams                                                                                            |
| **rxdl Language Reference**    | 0.1 draft | draft, E7a        | the unrestricted profile and the domain spellings; adds no semantics (ADR-0012)                                                                                                                  |
| **rmdl Language Reference**    | 0.1 draft | normative         | behaviour layer: functions, models, steps/timeline semantics, flow stdlib                                                                                                                        |
| **rsdl Language Reference**    | 0.1 draft | normative         | architecture layer: components (situated reactions), providing/requiring services, application-notation wiring, composition + deployment regions, transport/posture derivation, bundles          |
| **expr-core Specification**    | 0.1 draft | normative         | the full contract-term grammar: the guaranteed subset (V1, E2 — normative as implemented) + the function layer (V2, E5.1 — forward-looking), typing rules, evaluation domains, RIDL-306 boundary |
| ADR-0003 — the family decision | —         | **not started**   | freezes §1 and the ledger below in ADR form                                                                                                                                                      |
| IR specification               | —         | not started       | serialization, plugin protocol, diff categories, canonical encoding                                                                                                                              |
| ridl-rt runtime specification  | —         | not started       | scheduler/timeline, acks, quarantine, lag metrics, supervision hooks                                                                                                                             |

Superseded: RIDL Language Reference v0.1 (split into typl + ridl v0.2); markspec
typl/uxil (prior work, mapped in the typl/rxdl appendices); uxdl v0.1 (retired
by ADR-0012, archived — its content absorbed by ridl's boundary model and the
rxdl spellings).

## 3. Shared Doctrines — One Place, With Pointers

Each doctrine is normative **where cited**; this list is the index.

1. **One keyword, one concept** — family-wide reserved-word registry; the union
   of the per-profile keyword sections is the registry. _typl §1.4_
2. **Registry admission test: language, never runtime** — keywords name
   describable properties, never execution mechanisms; steps, acks, retries,
   async/await, scheduling are runtime vocabulary only. _typl §1.4_
3. **Sigil poverty** — `@` `?` `?:` `->`-arrow and brackets are nearly the whole
   sigil budget; words over symbols, for non-programmer audiences. _rmdl §4.3
   note, typl §17.7c_
4. **Vocabulary lives in typl** — payloads are named types everywhere; no layer
   defines shapes. Nominal typing makes unit safety real. _typl §5.7, ridl §3,
   rxdl §3_
5. **Ranges are the semantic truth** — wire widths derived (count-based for
   floats, `int64`-capped for integers) and never written in source; quantized
   floats as scaled integers on CAN. An explicit width **floor** (a `wire`
   clause) is deferred; the `ridl-diff` gate guards width flips meanwhile. _typl
   §4–§5, §17.11_
6. **Errors are data** — `error` types + inline `T | E` returns; no `throws`, no
   exceptions anywhere; three strata: declared functional / derived contract /
   transport ("infrastructure failure — detected, undeclared"). _typl §10, ridl
   §10_
7. **The channel is never empty** — init values (declared or derived) seed every
   signal channel and every model memory; invalidity propagates as state, never
   silent quarantine. _typl §5.8, ridl §4.4–4.5, rmdl §5.3_
8. **The implicit envelope, sender-stamped** — timestamp + sequence number on
   every interaction instance; loss detectable, dedup possible, payloads never
   re-declare it. _ridl §3.1_
9. **One time, logical** — a single platform instant (TAI/PTP epoch,
   synchronized base assumed); `now`/`dt`/`time(f)` are language, wall-clock and
   datetime are runtime/presentation; models compute with the time of the
   _cause_. _ridl §3.1, rmdl §6.3_
10. **Reactive, not periodic** — the scheduler has no clock, it has a timeline
    projected from inputs and contracts; periodic is the degenerate case;
    declared `Clock`s are legitimate functional description. _rmdl §6, Appendix
    G_
11. **Timing is contract** — `@[min..max]` bounds are freshness SLOs, scheduler
    constraints, and debounce/TTL declarations at once; optional with
    configurable defaults `[100ms..1000ms]`. _ridl §9_
12. **One evolution model** — implicit ordinals + `reserved` tombstones +
    append-only, from struct fields to interface methods to view interactions;
    `ridl-diff` (plumbing-grade despite living in the facade) is the gate. _typl
    §7.4, ridl §11, concept note §9.1_
13. **Side effects are emissions** — models emit events; rsdl binds events to
    commands; nothing in behaviour calls anything. _rmdl §5.7_
14. **Contracts are multi-executable** — one `require`/`ensure`, five ways:
    static, property test, online observer, reference oracle, deductive proof.
    _ridl §13, rmdl §9_
15. **Failure detection is total, failure management is elsewhere** — safety/HA
    are properties (future, profile-gated), mechanisms are runtime/rsdl. _ridl
    §10.4_
16. **Models are contract-blind; components situate them** — a model is a pure
    reaction `(O,S)=M(I,S)`; a component wires it to real signals and
    provides/requires services; a view/model never names its peers. _rmdl §7,
    rsdl §3_
17. **Interface is the shape, service is its global published declaration** —
    `interface : service :: type : instance`; the service catalog is the SSOT;
    components provide/require services. _ridl §14_
18. **One contract, both postures** — a `service` is posture-neutral; rsdl
    derives static (signal/bus, Classic) vs discovered (service, Adaptive) per
    deployment, so one contract set ships signal-based _and_ service-oriented.
    This is the AUTOSAR-vs-SOA/SDV reconciliation. _rsdl §8_

## 4. Reading Paths by Audience

- **Data architect**: typl, end to end. Stop there — it stands alone.
- **Service / bus SSOT engineer**: typl §1–§10 → ridl, end to end (ridl §14 =
  interface vs service).
- **UX / frontend engineer**: typl §1–§10 → ridl §3–§4, §9–§10 (the core
  semantics, and the presentation/intent families) → rxdl, end to end.
- **Sensor / actuator engineer**: typl §1–§10 → ridl §3–§4, §9 (the acquisition
  and control families and their obligations) → rxdl §5.
- **Control / algorithm engineer**: typl §1–§10 → ridl §3, §9 → rmdl, end to end
  (Appendix G is your library).
- **Integrator / architect**: everything above at survey depth → rsdl, end to
  end (composition + deployment).
- **Auditor / safety assessor**: this document §3 → typl §1.4 + §7.4 → ridl §10,
  §14 → rmdl §3.2, §6.4, §8 → rsdl §5.3, §8, §10 → the diagnostics tables of
  each reference.

## 5. Decision Ledger (design sessions, July 2026)

Chronological; each recorded in full where cited.

| #  | Decision                                                                                                                                                                                                                                                                                                                                                                                               | Where                                  |
| -- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | -------------------------------------- |
| 1  | typl extracted as the vocabulary layer; streams stay in ridl                                                                                                                                                                                                                                                                                                                                           | typl §1                                |
| 2  | Coded diagnostics family-wide (markspec practice)                                                                                                                                                                                                                                                                                                                                                      | typl §16                               |
| 3  | Count-based float width rule (errata to v0.1); integer domain capped at `int64`; explicit `wire` floor clause _(later deferred — see #31)_                                                                                                                                                                                                                                                             | typl §4–§5, §17.11                     |
| 4  | Field identity: implicit ordinals + `reserved`, no explicit tags                                                                                                                                                                                                                                                                                                                                       | typl §7.4                              |
| 5  | Nominal typing; no recursion in composites; no generics (stdlib intrinsics excepted)                                                                                                                                                                                                                                                                                                                   | typl §5.7, §7.3, rmdl App. G           |
| 6  | `fixed` over `final` over `config` — one word for one concept at both boundaries (ADR-0011)                                                                                                                                                                                                                                                                                                            | concept note §10                       |
| 7  | ridl v0.2 supersedes v0.1's interaction half                                                                                                                                                                                                                                                                                                                                                           | ridl provenance                        |
| 8  | Last-value + init-value channels; invalid-state propagation (SNA = CAN realisation)                                                                                                                                                                                                                                                                                                                    | ridl §4.4–4.5                          |
| 9  | Implicit envelope, sender timestamping; system time on TAI/PTP epoch                                                                                                                                                                                                                                                                                                                                   | ridl §3.1                              |
| 10 | Errors-as-data: `error` types + result unions; `throws` removed; three strata; command ack beneath fire-and-forget                                                                                                                                                                                                                                                                                     | typl §10, ridl §6, §10                 |
| 11 | Timing optional with configurable default `[100ms..1000ms]`                                                                                                                                                                                                                                                                                                                                            | ridl §9.1                              |
| 12 | Flat interfaces (no `extends`); no in-language version                                                                                                                                                                                                                                                                                                                                                 | ridl §14, §11                          |
| 13 | Failure management out of scope; future safety/HA _properties_, profile-gated first                                                                                                                                                                                                                                                                                                                    | ridl §10.4                             |
| 14 | ~~uxdl: standalone `view` contracts; core-mirror kinds + action refinements~~ — **superseded by ADR-0012**: uxdl retired, ridl gains five interaction families and four correspondence obligations, rxdl carries the spellings                                                                                                                                                                         | ADR-0012, rxdl §1                      |
| 15 | rmdl: unified `model` (no `node`); `returns` dropped for `:`                                                                                                                                                                                                                                                                                                                                           | rmdl §1.4, §5.1                        |
| 16 | Total function layer, shared with expr core                                                                                                                                                                                                                                                                                                                                                            | rmdl §3                                |
| 17 | Reactive-not-periodic: runtime-scheduled steps; timeline; logical time; parallelism by causality; async/await never surface                                                                                                                                                                                                                                                                            | rmdl §6                                |
| 18 | `signal`/`event` flow kinds in model signatures; `when`/`emit`; event→command side effects (GRust adoption)                                                                                                                                                                                                                                                                                            | rmdl §5.1, §5.6–5.7                    |
| 19 | Memory: `last` + `init` (seed model) replacing `pre`/`->`; channel-init seeding                                                                                                                                                                                                                                                                                                                        | rmdl §5.3                              |
| 20 | Dispatch triad: `case` (value; expression + mode-equation) / `when` (occurrence) / `match` (pattern)                                                                                                                                                                                                                                                                                                   | rmdl §4.3, §5.2                        |
| 21 | Contracts on functions/models incl. deadline terms; deductive proof as fifth verification way                                                                                                                                                                                                                                                                                                          | rmdl §9.2                              |
| 22 | Surface `step` removed: `now`/`dt` ambient contextual values; `dt ≡ now - last now`                                                                                                                                                                                                                                                                                                                    | rmdl §6.3                              |
| 23 | Flow stdlib curated + named by three-axis review: Hold, Changes, RisingEdge/FallingEdge, Filter, Accumulate, Deadband, Prefer, Latch, Clock, Sample, Coalesce, Watchdog + control tier                                                                                                                                                                                                                 | rmdl App. G                            |
| 24 | Registry admission test (language vs runtime) adopted; `step` collision resolved to typl-only                                                                                                                                                                                                                                                                                                          | typl §1.4                              |
| 25 | ridl gains **`service`** = global published declaration of an `interface`; `interface` = abstract shape (`type : instance`); SSOT catalog, `service.member`, posture-neutral                                                                                                                                                                                                                           | ridl §14                               |
| 26 | **rmdl purified**: `realizes` dropped, §7 → "Models Are Contract-Blind"; a model is a pure reaction, contract binding is rsdl's                                                                                                                                                                                                                                                                        | rmdl §7                                |
| 27 | **Component = situated reaction**: real-signal boundary via `provides`/`requires`; body is application notation `out = Reaction(in)`; leaf (models) vs composite (sub-components); "composition"/"binding" keywords rejected                                                                                                                                                                           | rsdl §3–§4                             |
| 28 | **Posture derivation**: static (signal/bus, Classic) vs discovered (service, Adaptive) chosen at deployment from static-wire-vs-discovery + physics; one contract deploys both worlds; AUTOSAR vs SOA/SDV reconciled                                                                                                                                                                                   | rsdl §8                                |
| 29 | Redundancy = two components providing one service, **declared** (else build error); shape stays single                                                                                                                                                                                                                                                                                                 | rsdl §5.3                              |
| 30 | Bundle collapsed to one concept (spk/apk dropped — Android's, not ours); platform-vs-app is a `tier` attribute                                                                                                                                                                                                                                                                                         | rsdl §9                                |
| 31 | **Init syntax unified to bare `= value`** (typl types/fields, ridl signal overrides, uxdl display overrides); `default` keyword retired; `init` kept as rmdl's alone (memory seed `init x = value`). **`wire` clause + ten width names dropped** from v0.1 — width is range-inferred and unwritable; the explicit width **floor** deferred to typl §17.11 (`ridl-diff` gate covers the flip meanwhile) | typl §5.6/§5.8, ridl §4.4, typl §17.11 |
| 32 | **`ridl.std` scoped by a stated inclusion criterion**: only definitions fixed by a cross-industry standard whose meaning is domain-independent, because the package is implicitly imported into every file of every profile. The ISO 3779 `Vin` type and `VIN_PATTERN` constant are removed as automotive — inherited unexamined from RIDL v0.1, and the only members bound to one industry            | typl App. A                            |

## 6. Open Questions — Consolidated Index

By home; see each reference for full statements.

- **typl §17**: string-backed enums · exclusive bounds · uniqueItems · recursion
  policy · unit conversion algebra · scientific notation · expr-core deferrals
  (arithmetic bounds, predicates, infix `match`, invariants) · explicit wire
  sentinels · byte order home · canonical encoding · **explicit wire-width floor
  (deferred `wire` clause, §17.11)**
- **ridl §17**: selective broadcasts · interaction-set reuse ·
  actions/long-operations idiom · mid-stream invalid policy · reflection service
  · failure-management spec (with safety/HA properties direction) — the QoS
  boundary question is answered by the ADR-0015 absorption principle (ridl
  §17.5), and the signal-groups question is closed by the ADR-0015 coherence
  rule: the struct idiom is confirmed (ridl §17.3)
- **rxdl §11**: the intent occurrence keyword · the acquisition/query cell ·
  interaction citation paths (ridl's, recorded there) · journeys and screen flow
  · accessibility metadata
- **ADR-0012 open**: availability beyond `during` (five sources,
  consumer-evaluability) · absent versus disabled
- **rmdl §12**: multi-activation (`merge`/`current` reserved) · query behaviour
  (pure-function-over-state direction) · state-machine sugar · instance arrays ·
  saturate-vs-fault boundaries · stdlib freeze · WCET annotation home · unit
  algebra (shared with typl) · timeout/timer steps
- **rsdl §13**: composition/deployment boundary · dynamic topology/orchestration
  (elastic) · transport/posture policy expressiveness · service discovery
  matching · end-to-end timing composition · bundle dependency/versioning ·
  resilience realization · global service catalog scoping
- **Cross-cutting, unhomed**: IR stability policy (blocks `ridl-diff` contract
  and plugin protocol) · UCUM→AUTOSAR unit mapping table · bridge authentication
  (concept note)

## 7. Shared Diagnostic Namespaces — `FORM-` and `MANI-`

Diagnostic codes are namespaced and grouped by hundreds, never renumbered and
never reused (typl §16). **One namespace per profile**, tabulated in that
profile's own reference: `TYPL-` (typl §16), `RIDL-` (ridl §16), `RMDL-` (rmdl
§11), `RSDL-` (rsdl §12). Of those four, `TYPL-` and `RIDL-` are implemented in
the shipped toolchain; the other two are specified ahead of their layers.
**There is no `RXDL-` family and none should be minted** — every rule a domain
spelling can violate is a ridl rule (rxdl §10).

The two namespaces below belong to no profile. `FORM-` is the **shared family
grammar** — surface syntax, plus the attribute-block rules of general form §4.3
— and `MANI-` is the **manifest and distribution layer** (ADR-0002). Both are
tabulated once here and cited from each reference rather than restated: a
per-language copy would be five copies of one list, drifting apart as codes are
added.

`crates/ridl-core/src/diag.rs` is the single source of truth these two tables
mirror; `FORM_CATALOG` and `MANI_CATALOG` there carry the same codes and
severities.

### 7.1 Surface grammar (`FORM-`)

Lexical errors are `0xx`, parse errors `1xx`, and the attribute-block rules
`106`–`108`. Every code is an error.

| Code     | Rule                                                                                                                                                    | Severity |
| -------- | ------------------------------------------------------------------------------------------------------------------------------------------------------- | -------- |
| FORM-001 | invalid character                                                                                                                                       | error    |
| FORM-002 | unterminated string literal                                                                                                                             | error    |
| FORM-003 | unterminated regex literal                                                                                                                              | error    |
| FORM-004 | unterminated block comment                                                                                                                              | error    |
| FORM-005 | leading zeros in an integer literal                                                                                                                     | error    |
| FORM-101 | expected a specific token, or a construct the grammar admits here — most call sites name a token, the interaction positions name the shapes (ridl §7.1) | error    |
| FORM-102 | unexpected token — also the code for a literal the reference grammar does not admit, such as a fractional duration (ridl §2.1)                          | error    |
| FORM-103 | unclosed delimiter                                                                                                                                      | error    |
| FORM-104 | missing `package` declaration                                                                                                                           | error    |
| FORM-105 | reserved word used as an identifier                                                                                                                     | error    |
| FORM-106 | unknown attribute key — not a key the general form §4.3 table defines                                                                                   | error    |
| FORM-107 | attribute key not allowed on this declaration kind (general form §4.3)                                                                                  | error    |
| FORM-108 | duplicate attribute key in one `[ ]` block (general form §4.3)                                                                                          | error    |

### 7.2 Manifest and distribution (`MANI-`)

The manifest codes are `0xx`; the distribution codes — lockfile, cache, fetch —
are `1xx`.

| Code     | Rule                                                        | Severity |
| -------- | ----------------------------------------------------------- | -------- |
| MANI-001 | invalid manifest TOML                                       | error    |
| MANI-002 | manifest declares both `[package]` and `[workspace]`        | error    |
| MANI-003 | manifest declares neither `[package]` nor `[workspace]`     | error    |
| MANI-004 | nested workspace — a member manifest declares `[workspace]` | error    |
| MANI-005 | unknown manifest key                                        | warning  |
| MANI-006 | invalid package name — not lowercase dot-separated segments | error    |
| MANI-007 | invalid import URL                                          | error    |
| MANI-008 | workspace member directory is missing or has no `ridl.toml` | error    |
| MANI-009 | invalid `[defaults].timing` value (ridl §9.1)               | error    |
| MANI-101 | remote import fetch failed                                  | error    |
| MANI-102 | fetched content hash does not match the lockfile            | error    |
| MANI-103 | `--frozen`: no lockfile entry for a remote import           | error    |
| MANI-104 | `--frozen`: a lockfile-pinned import is not cached          | error    |

MANI-009 is the one manifest code the manifest layer does not raise: `ridl-core`
cannot depend on `ridl-sem`, so the manifest parser stores `[defaults].timing`
as an unparsed string and the checker validates it.

## 8. What "Consolidated" Means Here

The references stay separate **by design** — five audiences, five learnable
wholes, independent versioning, citable sections (the family's own §4 rationale;
the HTML/CSS/JS and JSON-Schema/OpenAPI precedents). This document is the single
entry point that makes the set navigable: doctrines indexed once, decisions
ledgered once, open questions inventoried once. A one-file _distribution_
(concatenated PDF of all references) is a build artifact for `ridl doc`, not an
authoring structure.

---

_Maintained alongside the references; update §2, §5, §6, §7 whenever a reference
changes._
