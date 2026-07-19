# AGENTS.md — RIDL

RIDL is a family of languages for modeling component-based reactive systems:
**one platform, five languages, one grammar, one intermediate representation.**
A shared vocabulary layer (`typl`) plus four description languages over it
(`ridl`, `uxdl`, `rmdl`, `rsdl`), sharing one toolchain and one IR.

This repository holds the specifications, the architecture decision records
(ADRs), the implementation roadmap, and the compiler workspace. The typl v0.1
toolchain (epic E1) is built — seven crates under `crates/` (`ridl-syntax`,
`ridl-core`, `ridl-sem`, `ridl-ir`, `ridlc`, `ridl`, `ridl-lsp`) plus
`backends/rust`, `tools/fmt`, `xtask`, and the `editors/vscode` extension; the
four description languages (`ridl`, `uxdl`, `rmdl`, `rsdl`) are sequenced in the
roadmap. See `docs/technotes/walking-skeleton-architecture.md` for the as-built
map.

**Read these before doing anything else in this repo:**

- `docs/specification/ridl-family-overview.md` — the entry point: the map, the
  shared doctrines (indexed once), the decision ledger, and the open-question
  index. Start here.
- `docs/wip/ridl-family-concept.md` — the concept note: motivation, cores,
  profiles, the platform/IR model, the naming ledger (pre-ADR).
- `docs/wip/family-general-form.md` — the surface rules shared by every profile:
  the three declaration shapes, the nine surface invariants, the attribute model
  (pre-ADR working spec).
- `docs/specification/{typl,ridl,uxdl,rmdl,rsdl}-language-reference.md` — the
  five language references.
- `docs/decisions/` — ADR-0002 (module system), ADR-0004 (sequencing and stack),
  ADR-0005 (agent enablement), ADR-0006 (E0 execution), ADR-0007 (E1 execution).
- `docs/ROADMAP.md` — the epics, stories, and the V1 (contract platform) / V2
  (executable platform) release split.

These are living records. A decision that changes one is recorded there directly
— don't silently diverge from it.

## The family

    typl   type language                 — data: types, ranges, units, constants
    ridl   interface description         — system interactions (signal/event/command/query/final)
    uxdl   user-experience description   — user interactions (display/input/action/fetch/fixed)
    rmdl   model description             — behaviour: functions + reactive models
    rsdl   system description            — architecture: components, wiring, deployment

Dependency lattice: `typl ← {ridl, uxdl, rmdl} ← rsdl`. typl is the only
standalone member; rsdl is the apex.

## Commands

    just fmt        reformat connective tissue with prim + fix Markdown
    just check      lint gate — prim --check + markdownlint (no writes)
    just compile    compile the Rust workspace
    just test       run the Rust workspace test suite
    just build      compile + check — the full local gate
    just verify     commit-message lint over the branch range, then build — run before a PR
    just book       serve the mdBook docs locally
    just release    git std bump — version, changelog, tag
    just install    ./bootstrap — installs git-std, prim, and git hooks

Full recipe set: `justfile`. The toolchain conventions come from
driftsys/git-std (commits, versioning, hooks) and driftsys/prim
(connective-tissue formatting).

## Conventions

- **Conventional Commits**, linted by git-std against `.git-std.toml` — types
  and scopes are enumerated there. Never push directly to `main`; use a PR.
- **prim owns the connective tissue** (Markdown/JSON/YAML/TOML) — it honors
  `.editorconfig` only, no per-tool config. `.primignore` is the escape hatch
  for files that must stay byte-exact.
- **markdownlint** enforces Markdown style (`.markdownlint.json`).
- Documents are prose, in Markdown, under `docs/`. The specs read as one system:
  doctrines are indexed once in the overview, cited from each reference — keep
  that discipline when editing.

<!-- git-std:bootstrap -->

## Post-clone setup

Run `./bootstrap` after `git clone` or `git worktree add`.
