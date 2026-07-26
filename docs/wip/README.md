# Work in progress

Pre-ADR and preliminary documents — direction-setting drafts and working specs
that are not yet ratified as normative references. They graduate into
[`../specification/`](../specification/) (or an ADR under
[`../decisions/`](../decisions/)) as they settle.

Superpowers specs and plans live here while an epic runs and are archived
verbatim to [`../archive/`](../archive/) at epic close. None is open: the epic
E2 plan was archived by the E2 gardening pass on 2026-07-26.

- **ridl-family-concept.md** — the concept note: motivation, cores, profiles,
  the platform/IR model, the naming ledger. Explicitly pre-ADR (feeds the
  not-yet-written ADR-0003). Parts of it are aspirational rather than as-built —
  §8.1's repository tree shows `spec/`, `backends/`, `runtimes/`, and `tools/`
  directories that the workspace does not have (every crate lives at
  `crates/<crate-name>/`, issue #180). Read §8.1 for the plumbing/porcelain
  model it argues for, which did ship, not for the layout it draws.
- **family-general-form.md** — the cross-profile surface rules (three
  declaration shapes, nine invariants, the attribute model). A pre-ADR working
  spec, and the one document here that other records depend on: ADR-0008
  decision 1 made it authoritative for four points of the E2 interaction
  surface, and three documents under `../specification/` cite it by section —
  the ridl reference (§6.1, §6.4), the family overview (§4.3, for the FORM-106
  to FORM-108 rows), and the expr-core specification (§4.2). **Unresolved:** a
  document cited that way is normative in effect while its folder says it is
  not. Promoting it is a ratification decision, not a gardening move, so the E2
  gardening pass left it here and recorded the tension on issue #172.
- **skill-ridl-authoring-outline.md** — outline for the agent-authoring skill
  (see ADR-0005). Forward-looking: the skill it outlines is roadmap story E8.2,
  which has not been built.
