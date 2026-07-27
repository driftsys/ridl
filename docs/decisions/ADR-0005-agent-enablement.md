# ADR-0005: Agent Enablement — How AI Agents Understand and Author RIDL

## Status

Proposed.

Assumes ADR-0002 (module system) as accepted and ADR-0004 (implementation
sequencing and stack) as the direction of record, and depends on the concept
note's §8–§9 conclusions (compiler-as-library, stable IR, coded diagnostics, the
CI/test planes). ADR-0003 (the family decision) is assumed as direction but not
required by anything here.

## Context

RIDL is a contract language. Contract languages are written, evolved, and
reviewed — and increasingly that work is done by, or alongside, AI coding agents
(Claude Code, Cursor, Copilot, Cowork, and their successors). An agent that can
turn a natural-language spec into a correct `.ridl` interface, evolve a `.typl`
type without breaking the wire, or review a diff for contract soundness is a
first-class user of the platform, not a novelty. The question this ADR settles
is: **what does the platform build so that an agent produces correct, idiomatic
RIDL — for interfaces, types, and (later) behaviour?**

Three candidate surfaces are on the table, and they are routinely posed as
alternatives:

1. **LSP** — a language server, already planned as `ridl-lsp` (ADR-0004 §6).
2. **MCP** — a Model Context Protocol server exposing RIDL capabilities to an
   agent as callable tools.
3. **A skillset** — agent-facing knowledge and behaviour: rules (always-on
   constraints), skills (loaded task guidance), and subagents (specialized
   agents).

Two facts reframe the choice.

**Fact one — the agent's problem is not the human's problem.** A human in an
editor gets LSP feedback continuously while typing. An agent works in a loop:
_generate a draft → verify it → fix → (when editing) prove nothing broke._ Each
phase needs a different thing — knowledge to generate, a grounded oracle to
verify, a breaking-change gate to evolve. The three candidate surfaces are not
competitors; they serve different phases of that loop.

**Fact two — RIDL is new and niche, so a model has effectively zero training
data on it.** This inverts the usual priority. Wiring an agent for TypeScript is
mostly a verification problem; the model already knows the language. For RIDL
the model cannot even emit syntactically valid output from priors — it does not
know that there are no semicolons, that payloads must be named typl types, that
errors are data and there is no `throws`. The **knowledge layer is therefore the
highest-leverage and cheapest** piece, and it is the one none of the compiler
work produces on its own.

The decisive enabler is that ADR-0004 already chose the substrate an agent story
needs: **compiler-as-library** (so a new front-end consumer is a thin binary,
not a reparser), a **stable serializable IR** (queryable structured truth), a
**structured diagnostic SSOT with stable codes and fix-its** (ADR-0004 §5 —
ideal machine-consumable feedback), and **`ridl-diff` with defined exit codes**
(a deterministic evolution gate). The agent surfaces below are consumers of this
substrate, exactly like the LSP and the backends — not a parallel stack.

## Decision

### 1. Three layers, not three alternatives

Agent enablement is modeled as three stacked layers, each serving a phase of the
agent loop and each mapping onto one candidate surface:

| Layer          | Serves            | Surface                   | Answers                                         |
| -------------- | ----------------- | ------------------------- | ----------------------------------------------- |
| **Knowledge**  | _generate_        | skill + rules             | "how do I write valid, idiomatic RIDL?"         |
| **Capability** | _verify / evolve_ | MCP over the compiler     | "is this correct? did I break it? what exists?" |
| **Engine**     | (shared truth)    | `ridl-lsp` / `ridlc` / IR | the one compiler every surface consumes         |

The one-line routing test, adopted as doctrine: **if it teaches the agent how to
write RIDL, it is a skill; if it lets the agent check, query, or evolve RIDL, it
is an MCP tool; if it is for a human's editor, it is the LSP.**

The build order follows leverage-over-cost, not the list order above:
**knowledge first (zero compiler dependency), capability second (cheap given
ADR-0004), engine already funded for humans.**

### 2. Layer A — the skill and rules (build first; no compiler dependency)

Highest ROI, because the language is unknown to the model, and buildable today
in any agent host (Claude Code, Cursor, Cowork) with no platform code. Two
artifacts:

**A rules file** — short, always-on, the hard constraints an agent must never
violate. Distilled from the family doctrines and the diagnostics that are
_errors_. Illustratively: no semicolons; payloads/fields are always named typl
types (never inline `string`/`bytes`, never inline shapes); errors are data — no
`throws`, no exceptions, a fallible query returns a result union; commands never
return a value (use a query); no interface inheritance; upward references in the
lattice are compile errors; append-only evolution with `reserved` tombstones;
sigil poverty (words over symbols). Ten to twenty imperative "never/always"
lines.

**A skill** — loaded on demand, a _distilled, agent-optimized_ reference, not
the normative spec. Agents underperform on 40-page specs and excel with dense
decision tables plus worked examples. Its shape is stubbed in the companion
outline (see References). It carries the kind-selection tables (signal vs event
vs command vs query vs fixed; `case`/`when`/`match`), the doctrines as rules, a
**common-mistakes table keyed to diagnostic codes**, and three to four complete
`.rxdl` worked examples (cruise-control is the canonical seed).

**Provenance discipline.** The skill is _generated from the reference docs_, not
hand-written in parallel, so it cannot drift from the specs. Once `ridl
doc`
exists (ADR-0004 §10), the skill's tables and examples become one of its emit
targets — the same SSOT-from-IR principle the platform applies to bindings and
docs applies to agent knowledge.

### 3. Layer B — an MCP server over the compiler (build second; cheap by construction)

The MCP server is another IR-and-diagnostic consumer behind the same boundary
the backends and the LSP use — a thin binary over the shared crates (ADR-0004
"compiler is a library first"), never a second parser. This is why it is cheap
_for this platform specifically_: the hard parts (structured diagnostics, the
stable IR, `ridl-diff`) are already built for other reasons. Minimum viable tool
set:

| Tool                            | Returns                                                      | Backs the loop phase |
| ------------------------------- | ------------------------------------------------------------ | -------------------- |
| `ridl_check(source)`            | structured diagnostics — coded, spans, **fix-its verbatim**  | verify               |
| `ridl_explain(code)`            | the rustc-`--explain`-style entry (ADR-0004 §10 error index) | verify / learn       |
| `ridl_diff(a, b)`               | exit class 0/1/2 + the breaking-change list                  | evolve               |
| `ridl_describe_type(name)`      | range, unit, step, init, resolved wire width                 | ground               |
| `ridl_list_interactions(iface)` | interactions with kinds, ordinals, timing                    | ground               |
| `ridl_resolve(symbol)`          | package, kind, definition location                           | ground               |

Two rules make this effective. **Return the coded diagnostics with their
fix-its, unaltered** — agents are exceptional at consuming `TYPL-405`-style
codes with suggested fixes, which turns the platform's coded-diagnostic
investment into an agent superpower. And **expose `ridl_diff` as a first-class
tool** — it lets an agent edit a contract and _prove_ it did not break
compatibility (exit 0), which is the difference between a trustworthy autonomous
edit and a plausible-looking one.

The IR-query tools ground the agent in an _existing_ workspace so it references
real symbols instead of hallucinating names — the failure mode most likely for a
niche language.

### 4. Layer C — the LSP is the shared engine, not the agent's interface

`ridl-lsp` is built regardless, for humans in editors (ADR-0004 §6, §10). It is
**not** the agent's interface: agents do not natively speak LSP — they read
files and run commands. The agent-facing faces of the compiler are the **CLI**
(`ridlc`, `ridl check`, `ridl diff` — the CI face) and the **MCP** (the
interactive face). All three — LSP, CLI, MCP — are thin consumers of the _same_
salsa-driven compiler crates. The rule of record: **do not build a separate
agent code-path; the MCP is a sibling of the LSP over shared crates.** Work on
the structured-diagnostic SSOT and IR queries serves all three faces at once.

### 5. Evals are part of the deliverable, not an afterthought

Because the skill does the heavy lifting for an unknown language, its effect
must be _measured_, and regressions (from editing the skill or evolving the
language) must be caught. `ridl_check` is a free automatic oracle: an agent's
output either compiles clean or it does not. The platform maintains a small
**eval corpus of natural-language-spec → expected-RIDL** tasks, scored by "does
it compile" and, for evolution tasks, "does `ridl-diff` report the intended
category." This is a feedback loop most language projects cannot build; RIDL
can, for free, because of the compiler-as-oracle. The corpus lives beside the
snapshot-test corpus (ADR-0004 §9) and runs in the same CI plane.

### 6. Subagents come last — packaging, not foundation

Once Layers A and B exist, a specialized subagent (e.g. a `ridl-architect`) is
just their composition with a built-in verify loop for larger tasks ("design
this interface family; iterate until `ridl_check` is clean and `ridl_diff` is
compatible"). It is a convenience over the foundation, not part of it, and is
not built until the foundation is in place.

### 7. Language-design invariants that keep RIDL agent-legible

Several existing decisions are, incidentally, what make the language tractable
for agents; this ADR records them as constraints to _preserve_, not new work:

- **Every diagnostic stays actionable** (coded + fix-it). This is the single
  most important agent-facing property; regressing a diagnostic to an
  unstructured string degrades the MCP verify loop.
- **`.rxdl` (the total profile) is the canonical agent target and eval unit** —
  a whole system in one file, no cross-file resolution needed for small tasks,
  examples, and round-trip evals (concept note §4).
- **Sigil poverty / words-over-symbols** happens to maximize semantic signal per
  token for an LLM — keep it.
- **Coded diagnostics, `ridl-diff` categories, and the IR** are the three things
  the MCP surfaces; their stability policy (ADR-0004 open questions) is
  therefore also an _agent-contract_ stability policy.

### 8. Sequencing, mapped onto ADR-0004 phases

Agent enablement is not a separate program; it rides the existing phases:

| ADR-0004 phase            | Agent-enablement deliverable                                                                                                                                                                                                                        |
| ------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Phase 1 (typl)**        | v0 **skill + rules for typl** (types, ranges, units, evolution), generated from the typl reference; the eval corpus seeded with type-authoring tasks. Zero compiler dependency — can even precede Phase 1 code.                                     |
| **Phase 2 (ridl)**        | MCP server lands as a consumer of the now-real IR and `ridl diff`: `ridl_check`, `ridl_explain`, `ridl_diff`, IR-query tools. Skill extended to the `interact` core. This is the natural home because the diagnostic SSOT and diff gate exist here. |
| **Phase 3 (rmdl)**        | Skill extended to behaviour; MCP gains the reference-oracle/replay hooks (execute-and-diff), the strongest verify signal the platform has.                                                                                                          |
| **Phase 4–5 (uxdl/rsdl)** | Skill profiles for each layer; `ridl-architect` subagent once the family is whole.                                                                                                                                                                  |

## Consequences

### Positive

- The highest-leverage piece (knowledge) ships first and needs no compiler, so
  agents can author typl before the family is complete.
- The MCP reuses the IR, diagnostics, and `ridl-diff` already built for other
  consumers — near-zero marginal cost, and no divergent reparser (the "library
  first" promise extended to agents).
- Coded diagnostics with fix-its become directly agent-consumable, turning an
  existing investment into a differentiated capability.
- `ridl-diff` as a tool makes autonomous contract evolution _provable_, not
  hopeful — a safety property in a certification-adjacent domain.
- The eval corpus gives a measurable, automatable answer to "does the agent
  actually produce valid RIDL," and guards against skill/language drift.
- One engine, three faces (LSP/CLI/MCP) — an editor, CI, and an agent never
  disagree about what is valid.

### Negative / accepted trade-offs

- **The skill must be maintained against the specs.** Mitigated by generating it
  from the references and, later, from `ridl doc`; a hand-forked skill is
  explicitly disallowed.
- **The MCP couples an agent contract to IR and diagnostic-code stability.**
  Accepted — the same stability the LSP, backends, and `ridl-diff` already
  require; it raises the stakes on ADR-0004's IR-stability open question but
  adds no new axis.
- **Agent hosts and MCP/skill formats are evolving.** Accepted by keeping the
  _content_ (rules text, decision tables, the MCP tool semantics) as the SSOT
  and treating the host-specific packaging (Claude skill vs Cursor rules vs
  other) as a thin, regenerable shell.
- **Evals cost corpus curation.** Accepted; folded into the existing CI test
  plane rather than stood up separately.

## Open questions

- **Skill generation pipeline.** Is the skill emitted by `ridl doc` (one more IR
  consumer), or a curated document with generated tables spliced in? Decide once
  `ridl doc`'s emitter model is real (Phase 1 tail).
- **MCP transport and host coverage.** Which agent hosts are first-class (stdio
  MCP for Claude Code/Cowork/Cursor), and does the same binary back a CLI
  `ridl agent`-style entry? The interactively-authenticated-MCP caveat
  (headless/cron hosts may lack it) needs a fallback-to-CLI story.
- **Eval scoring beyond "compiles."** Compilation is necessary, not sufficient —
  idiomaticity (signal-vs-event choice, right error composition) needs a rubric
  or an LLM-judge with a golden set. Scope for Phase 2.
- **Rules-file portability.** One canonical rules text, N host formats — define
  the generator (or hand-maintain until a second host actually demands it).
- **Behaviour (rmdl) eval oracle.** The reference-oracle (execute-and-diff) is a
  far stronger signal than "type-checks" for generated behaviour; how it plugs
  into the eval corpus is a Phase 3 question.
- **Bridge/reflection exposure to agents.** The spy/control bridge (concept note
  §9.2) could be an MCP surface for live-system introspection; gated hard behind
  the same security/assurance model, deferred until the bridge exists.

## References

- ADR-0002: module system and package management.
- ADR-0004: implementation sequencing and stack — compiler-as-library, the IR,
  the diagnostic SSOT (§5), `ridl-diff`, the ecosystem rings (§10).
- Concept note — the RIDL family, §8 (IR, backends as consumers) and §9 (the CI
  and test planes, coded diagnostics, `ridl-diff`, deterministic replay).
- typl Language Reference §16 (diagnostics — the common-mistakes source), §7.4
  (evolution), §5 (ranges/units/width — the IR-query payload).
- ridl Language Reference §3–§10 (the `interact` core, timing, errors — the
  kind-selection tables), §11 (evolution), §16 (diagnostics).
- Companion: `docs/wip/skill-ridl-authoring-outline.md` — the distilled skill
  table of contents this ADR's Layer A refers to.

_Corrected (2026-07-26)._ The entry above read
`claude/skill-ridl-authoring-outline.md`, which named no file: no `claude/`
directory has existed at any commit in this repository, and the outline and this
ADR were added in the same initial commit, so the path was wrong when it was
written rather than overtaken by a later move. The target is not in doubt — the
outline opens by naming itself the table of contents for the skill this ADR's
Layer A describes, and it is the only such file — so the path is repaired here
rather than carried as debt. The correction is a repair, not a record of
something E2 changed: nothing about ADR-0005's decisions, Layer A, or the E8
story that builds the skill is touched.
