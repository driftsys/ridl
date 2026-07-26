# RIDL

**One platform, five languages, one grammar.** RIDL is a family of languages for
modeling component-based reactive systems: a shared vocabulary layer and four
description languages over it, sharing one grammar, one toolchain, and one
intermediate representation (IR).

This repository collects the language specifications, the cross-cutting design
specs, the architecture decision records (ADRs), the implementation roadmap, and
the compiler workspace. Two layers are built: the typl v0.1 preview toolchain
(epic E1) — compiler, `ridl fmt`, an LSP server, and a VS Code extension — and
the ridl interface layer over it (epic E2) — the five interaction kinds, timing,
contracts, interfaces and services, a TypeScript backend beside the Rust one,
and `ridl diff`. The three remaining description languages are sequenced in the
roadmap.

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
crates/                         The compiler workspace (typl + ridl)
├── ridl-syntax/                Lexer, parser, lossless CST, generated typed AST
├── ridl-core/                  Salsa database, manifest, lockfile, fetch, diagnostics
├── ridl-sem/                   Resolver + checker (per-profile semantic passes)
├── ridl-ir/                    IR v2 protobuf schema + generated types
├── ridlc/                      Compiler driver (check / build / emit)
├── ridl/                       Porcelain facade (check / baseline / build / test / fmt / diff)
├── ridl-lsp/                   Language server (diagnostics, hover, goto, rename, inlay)
├── ridl-backend-rust/          Rust + extern-C code generation over the IR
├── ridl-backend-ts/            TypeScript code generation over the IR
├── ridl-diff/                  The `ridl diff` IR-snapshot compare engine + classifier
└── ridl-fmt/                   The `ridl fmt` engine (rowan-based)
editors/vscode/                 VS Code extension (TextMate grammars + LSP client)
xtask/                          Workspace automation (ungrammar codegen, drift checks)
Cargo.toml                      Cargo workspace root
rust-toolchain.toml             The pinned Rust toolchain (ADR-0009)
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
│   ├── rsdl-language-reference.md
│   └── expr-core-specification.md  The shared contract-term grammar (require / ensure, and rmdl's function layer)
├── wip/                        Pre-ADR drafts and working specs
│   ├── ridl-family-concept.md      Concept note — the family direction (pre-ADR)
│   ├── family-general-form.md      Cross-profile syntax, typing, and attribute rules
│   └── skill-ridl-authoring-outline.md
├── technotes/                  Informative architecture notes (bind nothing)
│   └── walking-skeleton-architecture.md   The RIDL toolchain, as built
├── archive/                    Superseded documents + landed epic plans
│   ├── ridl-language-reference-v0.1.md   Split into typl + ridl v0.2
│   ├── 2026-07-18-e0-walking-skeleton-plan.md
│   ├── 2026-07-18-e1-typl-tooling-spine-plan.md
│   └── 2026-07-19-e2-ridl-interface-layer-plan.md
└── decisions/                  Architecture Decision Records
    ├── ADR-0002-module-system.md
    ├── ADR-0004-implementation-sequencing-and-stack.md
    ├── ADR-0005-agent-enablement.md
    ├── ADR-0006-walking-skeleton-execution.md
    ├── ADR-0007-e1-execution.md
    ├── ADR-0008-e2-execution.md
    └── ADR-0009-toolchain-and-gate-parity.md
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
Markdown/JSON/YAML/TOML), then wires up the repo-local hooks in `.githooks/`. It
also installs the Rust toolchain `rust-toolchain.toml` pins, and reports any
other tool the gate needs — `just`, `rustup`, mdBook, markdownlint — that it
cannot find.

The task runner is [`just`](https://github.com/casey/just):

| recipe                 | what it does                                                                                                                                                  |
| ---------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `just`                 | list the recipes                                                                                                                                              |
| `just fmt`             | reformat the connective tissue with prim, fix Markdown                                                                                                        |
| `just check`           | lint gate — `prim --check` + markdownlint, no writes                                                                                                          |
| `just toolchain-check` | the running toolchain is the one `rust-toolchain.toml` pins                                                                                                   |
| `just gate-parity`     | CI invokes every member of `just build`                                                                                                                       |
| `just fmt-check`       | `cargo fmt --all --check` (no writes)                                                                                                                         |
| `just book-check`      | `mdbook build` — the docs book compiles                                                                                                                       |
| `just compile`         | compile the Rust workspace (`--locked`)                                                                                                                       |
| `just test`            | run the Rust workspace test suite (`--locked`)                                                                                                                |
| `just lint`            | `cargo clippy --workspace --all-targets -- -D warnings`                                                                                                       |
| `just wasm-check`      | `cargo check` for wasm32, `--no-default-features`                                                                                                             |
| `just build`           | `toolchain-check` + `gate-parity` + `fmt-check` + `book-check` + `compile` + `test` + `lint` + `wasm-check` + `check` — the local gate, which is what CI runs |
| `just verify`          | commit-message lint + `build` — run before a PR                                                                                                               |
| `just book`            | serve the mdBook docs locally                                                                                                                                 |
| `just release`         | `git std bump` — version, changelog, tag                                                                                                                      |

CI (`.github/workflows/ci.yml`) invokes these recipes rather than restating
their commands, so there is one definition of each (ADR-0009).

Commits are [Conventional Commits](https://www.conventionalcommits.org), linted
against `.git-std.toml`. See [`CONTRIBUTING.md`](CONTRIBUTING.md) and
[`AGENTS.md`](AGENTS.md).

## Status

All documents are working drafts (typl / ridl / uxdl / rmdl / rsdl at
v0.1–v0.2). The design is captured; the typl v0.1 preview toolchain (epic E1 —
compiler, `ridl fmt`, LSP, and VS Code extension) is built over the shared
grammar and IR, epic E2 added the ridl interface layer over the same grammar and
IR v2, and uxdl, rmdl, and rsdl are sequenced in the roadmap.

## A note on ADR numbering

The ADRs present here are 0002 and 0004–0008. ADR-0001 and ADR-0003 are not in
this repository — ADR-0003 ("the family decision") is noted as not-yet-written
in the family overview.

## License

[MIT](LICENSE) © driftsys
