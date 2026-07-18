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

## Where the specifications live

This book is the reader's entry point. The normative documents live in the
repository:

- **Specifications** — `docs/specification/`: the family overview and the five
  language references.
- **Work in progress** — `docs/wip/`: the pre-ADR concept note, the
  cross-profile general-form working spec, and the authoring-skill outline.
- **Decisions** — `docs/decisions/`: the architecture decision records
  (ADR-0002, ADR-0004, ADR-0005).
- **Roadmap** — `docs/ROADMAP.md`: the epics, stories, and the V1/V2 release
  split.
- **Archive** — `docs/archive/`: superseded documents.

Browse them on GitHub:
<https://github.com/driftsys/ridl/tree/main/docs>.

## Status

All documents are working drafts (typl / ridl / uxdl / rmdl / rsdl at
v0.1–v0.2). The design is captured; the implementation is sequenced in the
roadmap but not yet built.
