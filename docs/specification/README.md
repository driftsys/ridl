# Specification

The normative language references for the RIDL family. Cross-profile working
specs and the pre-ADR concept note live in [`../wip/`](../wip/); superseded
documents in [`../archive/`](../archive/).

- **ridl-family-overview.md** — the entry point: the map, the shared doctrines
  (indexed once), the decision ledger, and the open-question index. Start here.
- **typl-language-reference.md** — the vocabulary layer: types, ranges, units,
  constants, composites, packages.
- **ridl-language-reference.md** — the system-interaction layer: `signal` /
  `event` / `command` / `query` / `fixed`, timing, errors, evolution, interfaces
  and services.
- **rxdl-language-reference.md** — the unrestricted profile and the domain
  spellings: `present` / `notify` / `measure` / `detect` / `actuate` / `trigger`
  and the intent operation shapes, over ridl's families. Adds no semantics of
  its own (ADR-0012). Replaces the uxdl reference, which is archived.
- **rmdl-language-reference.md** — the behaviour layer: functions, models,
  steps/timeline, the flow stdlib.
- **rsdl-language-reference.md** — the architecture layer: systems, components
  that offer services and require interfaces, distributions, deployments and
  machines; posture reserved.
- **expr-core-specification.md** — the cross-profile contract-term grammar: the
  guaranteed subset `require`/`ensure` uses today, the function layer rmdl
  extends it into, the typing rules, and the evaluation domains.
- **ir-specification.md** — the intermediate representation's encodings and the
  stability it promises: canonical protobuf JSON as the canonical encoding, what
  byte-identical rests on, the nesting bound in message levels and in JSON
  levels, which schema changes are additive and which are breaking, and how a
  breaking change is versioned. Partial: the plugin protocol and the diff
  categories are named but not yet owned here.
- **frame-specification.md** — the logical frame of the runtime: what crosses
  the boundary between a provider's runtime and a consumer's runtime per
  interaction kind — ordinal, kind, envelope, provenance, correlation, payload —
  in `ridl-rt`'s vocabulary; the invalid-payload behaviour; the rule that a
  binding is written from it alone; the WebSocket binding (E11.9) by name, and,
  for Binder on Android, the rule that a runtime binds the ports over its own
  binder contract, with no Binder layout specified (§11.2, which reverses the
  lane P driver's decision D-P5). Specified, not built: no runtime here speaks
  it.
