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

## Writing examples in the book

Every fenced block in `docs/book/` whose info string starts with `ridl` or
`typl` is **compiled by `crates/ridl/tests/book_examples.rs`**, which runs under
`cargo test --workspace`. A verified block must be a complete, self-contained
package file — it declares its own `package`, and blocks sharing a package name
are staged side by side, so one can `import` from another regardless of which
comes first in the book.

Mark a block that is deliberately not compilable — a fragment quoted out of its
file, a counter-example, a shape the language rejects — with `ignore`:

````markdown
```ridl,ignore
signal currentSpeed : Speed @10ms      // a fragment, not a whole file
```
````

Mark a block that draws a diagnostic **on purpose** with the code it expects.
Repeat the marker for several codes; prefer fixing the example over allowing a
code, because an allowance claims the surrounding prose explains the diagnostic:

````markdown
```ridl,allow=RIDL-406,allow=TYPL-115
```
````

mdBook renders all of these the same way, so no marker costs syntax
highlighting.

The harness is fail-closed. Each of these is an error rather than a silent skip:

- a verified block with no `package` declaration;
- an unrecognised marker, an empty `allow=`, or `ignore` and `allow=` on the
  same fence;
- **any** diagnostic the block did not name — error, warning, note, or one of
  the uncoded diagnostics, which can never be allowed because they have no code
  to name;
- an `allow=` naming a code the block does **not** draw, so a marker cannot
  outlive the example it was written for;
- an `import` naming a package no block declares, a name no block in that
  package provides, or the block's own package. The compiler resolves the
  package and stops — an unresolved _name_ inside a package it found draws no
  diagnostic — so the harness checks it rather than trusting the gap;
- a fence the scanner cannot read (see below);
- an unclosed fence, which would swallow every example after it;
- a book with no verified blocks at all.

A package name is a **book-wide** namespace, not a per-chapter one. Two chapters
that both declare `package veh.demo` are staged into one directory and collide
(`TYPL-009`) on every declaration they repeat, so give each chapter its own
package prefix.

### Where a fence may sit

Fences are three or more backticks or tildes, closed by at least as long a run
of the same marker. **Indentation is unrestricted**: a fence inside a list item,
at any nesting depth, is verified like any other — including from step 10 of an
ordered list, where the content column passes the three that CommonMark allows a
top-level fence.

Two placements the scanner declines, and **fails the book rather than
skipping**:

- a fence inside a **block quote**, because reading it means tracking block
  structure;
- a language word that is not exactly `ridl` or `typl` — `RIDL`, `ridl{.class}`.

mdBook renders both as `class="language-ridl"`, so a reader believes them. If
you hit this, move the fence out of the block quote or fix the language word.
The refusal is deliberate: guessing at CommonMark block structure is how the
harness would go back to failing silently.

A `ridl` fence quoted inside a longer fence — as in the samples above — is
documentation rather than an example, and is left alone.

Failures name the Markdown file and the exact line, because each block is staged
with the line offset it has in its source file.

The harness checks syntax, never claims. Prose about delivery, timing behaviour,
or provider-side enforcement describes a specification, not this workspace —
there is no runtime here. Write it so a reader cannot mistake the two.

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
