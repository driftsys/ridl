# Developer experience and agent assistance — scope brief

Status: scope agreed with the maintainer on 2026-10-03. No design in this file
is approved. Each spec named below starts its own brainstorming session from
this brief: it proposes approaches, presents a design, writes the spec, and gets
the spec reviewed before a plan is written.

## Why

Two kinds of users need more help than the toolchain gives today.

- **Interface and service designers who work with an AI assistant** (Claude Opus
  or Sonnet, or GPT). Today `ridl mcp` exposes one tool, `ridl_check`. It checks
  one pasted source string against `ridl.std` only, so an import of any other
  package does not resolve and spans name a synthetic file. No skill or rules
  file exists; only
  [`skill-ridl-authoring-outline.md`](skill-ridl-authoring-outline.md) does.
- **Developers who write and consume RIDL**. typl §14 specifies doc comments
  (`///`, `/** */`, CommonMark, `[Type]` links, `@see`, `@labels`,
  `@deprecated`), but the compiler checks almost none of it. The TypeScript
  backend emits JSDoc with `@unit`, `@range`, `@bounds` and `@deprecated`. The
  Rust backend copies type docs. Its interaction face carries generated rustdoc
  on each type, trait and method, but no source doc comment reaches the face,
  and no rule is stated there. Generated internals carry rustdoc written for the
  generator's maintainers, which the user of the generated crate also sees.

## What the maintainer said

Track 1, design assistant:

- The assistant targets interface and service designers who use Claude Opus,
  Claude Sonnet, or GPT.
- It helps them use the right interaction kinds, helps enforce semantic
  consistency, and helps keep low coupling and high cohesion of services and
  interfaces.
- It serves both design from requirements and review and evolution of an
  existing workspace, with review and evolution weighted a little more.
- "Enforce" means two layers: what can be computed becomes compiler lints;
  judgment stays in the skill, which cites the lint results as evidence.

Track 2, code documentation:

- RIDL supports proper code documentation in the rustdoc or Dokka style.
- Source doc style is rustdoc-like Markdown, documented where each item is
  declared, plus a small set of tags only for what the model cannot infer
  (option C of the session). Anything the model already knows (types, units,
  ranges, bounds, errors, timing, contracts) is extracted, never written by
  hand.
- Generated code carries the documentation. The public API facade's docs state
  the validation rules each item enforces. The generated internals do not.
- Missing documentation is a warning by default on public declarations and their
  members, configurable per project in `ridl.toml`.
- Lints are exposed as SARIF.

## Assumptions to confirm in the specs

- The skill and rules are portable across the first-class hosts ADR-0005 names:
  Claude Code, Copilot and Codex. They carry evals, as ADR-0005 requires.
- Generated internals are hidden from documentation (`#[doc(hidden)]` or the
  equivalent in each language), and their maintainer notes move to the
  generator's source.
- The Kotlin generator is an out-of-repo plugin, so it can only render what the
  IR carries. The extracted rules therefore live in the IR as structured data,
  and each backend only formats them.
- `ridl check` has `text` and `json` output today (`CheckFormat` in
  `crates/ridl/src/main.rs`); `sarif` is new work.

## The split

Four specs. Each one is a separate brainstorming, spec and plan cycle.

### Spec 0 — the shared lint foundation

Both tracks depend on it, so it comes first and stays small.

- A lint registry: a stable name and a default level for each lint, separate
  from the existing error diagnostics.
- A `[lints]` table in `ridl.toml` to change a level per project (for example
  `missing-docs = "deny"`). This changes the manifest, so ADR-0002 governs it.
- `ridl check --format sarif`.
- The same lint results through `ridl-lsp` and `ridl-mcp`.

Open decisions: how a lint differs from a warning diagnostic today; whether
levels can also be set per package or per declaration; the SARIF rule metadata
(help text, links to the diagnostic catalogue); how ADR-0010's exit codes treat
a lint raised to `deny`.

### Spec 2a — documentation in the source

- The small tag set (candidates: `@since`, `@example`). It changes the language
  surface, so it is recorded in the language references and in an ADR, and is
  checked against ADR-0011, ADR-0012 and ADR-0015, which AGENTS.md names for any
  change to the language surface.
- Doc lints: missing docs, a broken `[Type]` link, an unknown or malformed tag,
  `@deprecated` without a reason.
- Language server: every doc comment, at every level that can carry one
  (package, interface, interaction, parameter, field, enum value, error arm),
  reaches the language server. Hover on a declaration or on any use of it shows
  the rendered doc together with the rules extracted for spec 2b (range, unit,
  bounds, contracts, timing, errors), so the editor and the generated facade
  state the same contract. Completion items and signature help carry the same
  doc. `[Type]` links get completion and go-to-definition, and a quick fix
  inserts a doc stub.

Open decisions: which declarations and members count as public for
`missing-docs`; whether doc comments are allowed on every member kind (call
parameters, error arms, stream elements); how a `[Type]` link resolves across
packages and imports.

### Spec 2b — documentation in generated code

- A documented contract in the IR: the rules extracted once from the model
  (range, step, unit, size bounds, `require` and `ensure`, timing, the errors a
  call can raise) plus the source docs.
- The Rust backend renders rustdoc and the TypeScript backend renders TSDoc, on
  the public facade only. The out-of-repo Kotlin plugin renders KDoc from the
  same IR.
- Generated internals are hidden and carry no maintainer notes.

Open decisions: the IR shape for the documented contract, and whether it changes
the IR's stability promise (`2026-09-22-ir-stability-design.md`); how rules are
worded so they read the same in each language; the relation to the validators
that [`typl-value-objects-design.md`](typl-value-objects-design.md) plans, which
enforce the same rules the docs state.

### Spec 1 — the design assistant

Three parts, in this order:

- **1a. Workspace-aware MCP server.** `ridl_check` by path (and by path plus
  unsaved source, the overlay model `ridl-lsp` uses), `ridl_explain`, lookup
  tools (`ridl_resolve`, `ridl_describe_type`, `ridl_list_interactions`, the
  roadmap's E8.7), and review tools (what depends on what, and what a change
  breaks through `ridl_diff`, E8.6). Registration for Claude Code (a `.mcp.json`
  in this repository; `crates/ridl-mcp/README.md` already documents
  `claude mcp add`).
- **1b. Design lints on the foundation.** Naming and unit consistency, one
  concept defined twice under different names, dependency cycles between
  packages, the dependencies of each package and its dependents, and interface
  cohesion (members that share no types).
- **1c. Skill, rules and evals.** Built from the existing outline, for design
  and for review, portable across the three hosts. The skill handles the
  decisions that need judgment (which interaction kind, where a service boundary
  goes, how to name a concept) and cites lint and tool output as evidence.

Open decisions: how a request maps onto a loaded workspace and how that
workspace is cached between calls (shared with #529, the language server's
workspace-member gap); the tool schemas; how 1b's metric thresholds are set so
the default levels do not report findings that a designer would dismiss; how
evals are scored beyond "it compiles" (an open question in ADR-0005).

## Order

1. Spec 0, the lint foundation.
2. Specs 2a, 2b and 1a in parallel. 2a and 2b can both change `ridl-ir` (the doc
   model and the documented contract), so their specs agree on that shape before
   either plan starts.
3. 1b and 1c, which need the foundation and the 1a tools.

## Related work

- The language server's open gaps: #529 (a workspace member does not see its
  sibling members) and #385 (rsdl-aware references, rename and completion).
