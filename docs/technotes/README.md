# Technotes

Explanatory, informative notes. Nothing here is normative and nothing depends on
it — for decisions that bind downstream work, see
[`../decisions/`](../decisions/).

- **walking-skeleton-architecture.md** — the RIDL toolchain as built by epics E1
  and E2: the workspace map, the end-to-end pipeline contract, and the LSP
  overlay design the next profile's implementer needs, as they actually landed
  in the merged code. (The filename keeps its E0 origin; the E0 and E1-only
  versions are in git history.)
- **toolchain-distribution.md** — how the `ridl` binary and the VS Code
  extension reach a machine: the two release trains and their disjoint tag
  namespaces, where the reported version comes from, the extension's three-tier
  binary resolution, and what the install scripts verify.
- **rsdl-implementation.md** — rsdl as built by roadmap epic E6: the
  workspace-level `check_system` query, the model entries that are in no source
  file (the implicit component, the unit instance), why RSDL-804 is raised by
  the drivers, the lowering and its gating and orderings, what `ridl build`
  writes, `ridl diff` at the system, `rsdl` fences in the book, and what is not
  built yet.
