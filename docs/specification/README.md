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
- **rsdl-language-reference.md** — the architecture layer: components, services,
  systems, deployment, transport/posture.
- **expr-core-specification.md** — the cross-profile contract-term grammar: the
  guaranteed subset `require`/`ensure` uses today, the function layer rmdl
  extends it into, the typing rules, and the evaluation domains.
