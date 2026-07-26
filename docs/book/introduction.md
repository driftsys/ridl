# RIDL

**One platform, five languages, one grammar.** RIDL is a family of languages
for modeling component-based reactive systems: a shared vocabulary layer and
four description languages over it, sharing one grammar, one toolchain, and one
intermediate representation (IR).

## The family

| Language | Expands to | Describes | Audience |
| --- | --- | --- | --- |
| **typl** | type language | data — types, ranges, units, constants | data architects |
| **ridl** | reactive interface description language | system interactions (`signal` / `event` / `command` / `query` / `final`) | service teams |
| **uxdl** | user-experience description language | user interactions (`display` / `input` / `action` / `fetch` / `fixed`) | UX / frontend engineers |
| **rmdl** | reactive model description language | behaviour — synchronous / functional compute | control / algorithm engineers |
| **rsdl** | reactive system description language | architecture — components, wiring, deployment | integrators |

The dependency lattice is `typl ← {ridl, uxdl, rmdl} ← rsdl`: typl is the only
standalone member, and rsdl is the apex that composes the others.

## What is built

Two layers of the family have a working toolchain in this repository:

- **typl** — the vocabulary layer (epic E1): compiler, `ridl fmt`, an LSP
  server, and a VS Code extension.
- **ridl** — the interface layer over it (epic E2): the five interaction kinds,
  timing annotations, contracts, interfaces and services, a TypeScript code
  generator beside the Rust one, and `ridl diff`.

**uxdl, rmdl, and rsdl are specified but not built.** Their language references
are complete enough to design against, but no compiler accepts them and nothing
in this book describes them as usable. They are sequenced in the roadmap.

[Getting started](getting-started.md) walks through what you can run today.

## Where the specifications live

This book is the reader's entry point. The normative documents live in the
repository:

- **Specifications** — `docs/specification/`: the family overview, the five
  language references, and the expr-core specification (the shared grammar of
  `require` / `ensure` clauses).
- **Work in progress** — `docs/wip/`: the pre-ADR concept note, the
  cross-profile general-form working spec, and the authoring-skill outline.
- **Decisions** — `docs/decisions/`: the architecture decision records —
  ADR-0002 (module system), ADR-0004 (implementation sequencing and stack),
  ADR-0005 (agent enablement), ADR-0006 (walking-skeleton execution),
  ADR-0007 (E1 execution), and ADR-0008 (E2 execution).
- **Technotes** — `docs/technotes/`: informative architecture notes, which bind
  nothing.
- **Roadmap** — `docs/ROADMAP.md`: the epics, stories, and the V1/V2 release
  split.
- **Archive** — `docs/archive/`: superseded documents, and the plans of the
  epics that have landed.

Browse them on GitHub:
<https://github.com/driftsys/ridl/tree/main/docs>.

## Status

All specifications are working drafts: typl, uxdl, rmdl, rsdl and the
expr-core specification at v0.1.0, ridl at v0.2.0. The toolchain has no
published release — build it from a clone, as
[Getting started](getting-started.md) describes.
