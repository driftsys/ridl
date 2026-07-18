# RIDL

**One platform, five languages, one grammar.** RIDL is a family of languages for
modeling component-based reactive systems: a shared vocabulary layer and four
description languages over it, sharing one grammar, one toolchain, and one
intermediate representation (IR).

This repository collects the language specifications, the cross-cutting design
specs, the architecture decision records (ADRs), and the implementation roadmap.

## The family

| Language | Expands to                              | Describes                                                                | Audience                      |
| -------- | --------------------------------------- | ------------------------------------------------------------------------ | ----------------------------- |
| **typl** | type language                           | data — types, ranges, units, constants                                   | data architects               |
| **ridl** | reactive interface description language | system interactions (`signal` / `event` / `command` / `query` / `final`) | service teams                 |
| **uxdl** | user-experience description language    | user interactions (`display` / `input` / `action` / `fetch` / `fixed`)   | UX / frontend engineers       |
| **rmdl** | reactive model description language     | behaviour — synchronous / functional compute                             | control / algorithm engineers |
| **rsdl** | reactive system description language    | architecture — components, wiring, deployment                            | integrators                   |

The dependency lattice is `typl ← {ridl, uxdl, rmdl} ← rsdl`: typl is the only
standalone member, and rsdl is the apex that composes the others.

## Repository layout

```
docs/
├── ROADMAP.md                  Implementation backlog — epics, stories, V1/V2 release plan
├── getting-started.md          Getting started with RIDL
├── book/                       mdBook source (just book)
├── specification/              The normative language references + the family overview
│   ├── ridl-family-overview.md     Entry point: the map, shared doctrines, decision ledger, open questions
│   ├── typl-language-reference.md
│   ├── ridl-language-reference.md
│   ├── uxdl-language-reference.md
│   ├── rmdl-language-reference.md
│   └── rsdl-language-reference.md
├── wip/                        Pre-ADR drafts and working specs
│   ├── ridl-family-concept.md      Concept note — the family direction (pre-ADR)
│   ├── family-general-form.md      Cross-profile syntax, typing, and attribute rules
│   └── skill-ridl-authoring-outline.md
├── archive/                    Superseded documents
│   └── ridl-language-reference-v0.1.md   Split into typl + ridl v0.2
└── decisions/                  Architecture Decision Records
    ├── ADR-0002-module-system.md
    ├── ADR-0004-implementation-sequencing-and-stack.md
    └── ADR-0005-agent-enablement.md
```

## Where to start

- New to RIDL? Read
  [`docs/specification/ridl-family-overview.md`](docs/specification/ridl-family-overview.md)
  — the map and reading-path guide.
- Want the motivation and the big picture?
  [`docs/wip/ridl-family-concept.md`](docs/wip/ridl-family-concept.md).
- Building the toolchain? [`docs/ROADMAP.md`](docs/ROADMAP.md) and the ADRs
  under [`docs/decisions/`](docs/decisions/).

## Development

This repository follows the driftsys house style. After cloning, run
`./bootstrap` — it installs [git-std](https://github.com/driftsys/git-std)
(conventional commits, versioning, changelog, and git hooks) and
[prim](https://github.com/driftsys/prim) (the connective-tissue formatter for
Markdown/JSON/YAML/TOML), then wires up the repo-local hooks in `.githooks/`.

The task runner is [`just`](https://github.com/casey/just):

| recipe         | what it does                                           |
| -------------- | ------------------------------------------------------ |
| `just`         | list the recipes                                       |
| `just fmt`     | reformat the connective tissue with prim, fix Markdown |
| `just check`   | lint gate — `prim --check` + markdownlint, no writes   |
| `just compile` | compile the Rust workspace (no-op until epic E0 lands) |
| `just build`   | `compile` + `check` — the full local gate              |
| `just verify`  | commit-message lint + `build` — run before a PR        |
| `just book`    | serve the mdBook docs locally                          |
| `just release` | `git std bump` — version, changelog, tag               |

Commits are [Conventional Commits](https://www.conventionalcommits.org), linted
against `.git-std.toml`. See [`CONTRIBUTING.md`](CONTRIBUTING.md) and
[`AGENTS.md`](AGENTS.md).

## Status

All documents are working drafts (typl / ridl / uxdl / rmdl / rsdl at
v0.1–v0.2). The design is captured; the implementation is sequenced in the
roadmap but not yet built.

## A note on ADR numbering

The ADRs present here are 0002, 0004, and 0005. ADR-0001 and ADR-0003 are not in
this repository — ADR-0003 ("the family decision") is noted as not-yet-written
in the family overview.

## License

[MIT](LICENSE) © driftsys
