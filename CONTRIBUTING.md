# Contributing to RIDL

Thanks for your interest in contributing. This document covers the mechanics of
getting a change in. For the design itself, start with the family overview
(`docs/specification/ridl-family-overview.md`) and the decision records under
`docs/decisions/`.

## Getting set up

```sh
git clone https://github.com/driftsys/ridl
cd ridl
./bootstrap
```

`bootstrap` installs [git-std](https://github.com/driftsys/git-std) and
[prim](https://github.com/driftsys/prim), then hands off to `git std bootstrap`,
which wires up the repo-local git hooks in `.githooks/`. `just check` and
`just compile` additionally use `markdownlint-cli` (via `npx`) and
[mdBook](https://rust-lang.github.io/mdBook/) — install those with your package
manager as needed.

## Workflow

1. Branch from `main`.
2. Make your change. Documents are living records — a decision that changes one
   is recorded in it directly; don't silently diverge.
3. Write commits as [Conventional Commits](https://www.conventionalcommits.org)
   (`feat:`, `fix:`, `docs:`, `chore:`, …), with a scope when it aids clarity
   (e.g. `docs(rmdl): …`, `docs(adr): …`). `git-std` lints commit messages
   against `.git-std.toml` and drives changelog generation from them. The
   configured scopes are the five languages (`typl`/`ridl`/`uxdl`/`rmdl`/
   `rsdl`), `family`, `roadmap`, `adr`, the compiler workspace scopes
   (`ridl-syntax`/`ridl-core`/`ridl-sem`/`ridl-ir`/`ridlc`/`ridl-lsp`/
   `backends`/`tools`/`xtask`/`editors`), and the repo-wide scopes
   (`repo`/`docs`/`ci`/`hooks`/`deps`).
4. Run `just verify` before opening a PR — commit-message lint over your branch
   range, then `just build`.
5. Open a PR. CI runs the same gates.

## Recipes

Run `just --list` for the full set. The common ones:

| recipe         | what it does                                           |
| -------------- | ------------------------------------------------------ |
| `just fmt`     | reformat the connective tissue with prim, fix Markdown |
| `just check`   | lint gate — `prim --check` + markdownlint, no writes   |
| `just compile` | compile the Rust workspace                             |
| `just test`    | run the Rust workspace test suite                      |
| `just build`   | `compile` + `test` + `check` — the full local gate     |
| `just verify`  | commit-message lint + `build` — run before a PR        |
| `just book`    | serve the mdBook docs locally                          |
| `just release` | `git std bump` — version, changelog, tag               |

## Reporting issues

Use GitHub issues on this repository. For security-sensitive reports, see
`SECURITY.md` instead of filing a public issue.
