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
[prim](https://github.com/driftsys/prim), hands off to `git std bootstrap`,
which wires up the repo-local git hooks in `.githooks/`, and then installs the
Rust toolchain that `rust-toolchain.toml` pins.

The gate needs four tools `bootstrap` does not install, because they come from
package managers it should not write into on your behalf:
[`just`](https://github.com/casey/just), [`rustup`](https://rustup.rs) (it is
what applies the toolchain pin), [mdBook](https://rust-lang.github.io/mdBook/),
and `markdownlint-cli`. `bootstrap` names each one it cannot find, with the
command that installs it, and exits non-zero. Nothing is skipped when a tool is
missing — the recipe that needs it fails and says which one (ADR-0009).

The Rust toolchain is pinned to an exact version in `rust-toolchain.toml`, so
`cargo fmt` and `cargo clippy` run the same release here as they do in CI.
Bumping it is a deliberate PR: edit `channel`, run `just build`, and land the
repairs the new release wants in the same commit.

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
   `ridl-backend-rust`/`ridl-backend-ts`/`ridl-diff`/`ridl-fmt`/`xtask`/
   `editors`), and the repo-wide scopes (`repo`/`docs`/`ci`/`hooks`/`deps`).
4. Run `just verify` before opening a PR — commit-message lint over your branch
   range, then `just build`.
5. Open a PR. CI runs these same recipes — it installs the tools a runner needs
   and then invokes `just check`, `just compile`, `just test`, and the rest, so
   there is one definition of each command rather than two that drift
   (ADR-0009).

## Recipes

Run `just --list` for the full set. The common ones:

| recipe                 | what it does                                                                                                                                |
| ---------------------- | ------------------------------------------------------------------------------------------------------------------------------------------- |
| `just fmt`             | reformat the connective tissue with prim, fix Markdown                                                                                      |
| `just check`           | lint gate — `prim --check` + markdownlint, no writes                                                                                        |
| `just toolchain-check` | the running toolchain is the one `rust-toolchain.toml` pins                                                                                 |
| `just gate-parity`     | CI invokes every member of `just build`                                                                                                     |
| `just fmt-check`       | `cargo fmt --all --check` (no writes)                                                                                                       |
| `just book-check`      | `mdbook build` on a copy — catches a SUMMARY.md mdBook cannot parse                                                                         |
| `just compile`         | compile the Rust workspace (`--locked`)                                                                                                     |
| `just test`            | run the Rust workspace test suite (`--locked`)                                                                                              |
| `just lint`            | `cargo clippy --workspace --all-targets -- -D warnings`                                                                                     |
| `just wasm-check`      | `cargo check` for wasm32, `--no-default-features`                                                                                           |
| `just build`           | `toolchain-check` + `gate-parity` + `fmt-check` + `book-check` + `compile` + `test` + `lint` + `wasm-check` + `check` — the full local gate |
| `just lint-commits`    | `git std lint` over the commits on top of a base branch                                                                                     |
| `just verify`          | `lint-commits` + `build` — run before a PR                                                                                                  |
| `just book`            | serve the mdBook docs locally                                                                                                               |
| `just release`         | `git std bump` — version, changelog, tag                                                                                                    |

## Reporting issues

Use GitHub issues on this repository. For security-sensitive reports, see
`SECURITY.md` instead of filing a public issue.
