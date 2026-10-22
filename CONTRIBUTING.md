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

`just build` also needs [Deno](https://deno.com), for the TypeScript packages
`packages/ridl-runtime` and `packages/ridl-transport-ws`. It also needs
[Node](https://nodejs.org) 22 or later with npm, for `just ts-npm-check`, which
builds each package for npm and imports it from Node. `bootstrap` does not
install Node or npm, and the recipe needs network access. When `deno` is not on
`PATH`, `bootstrap` installs the version that `.deno-version` pins into
`~/.deno/bin`, and you add that directory to `PATH`. When another version is
installed, `bootstrap` prints a note and leaves it in place; CI runs the pinned
one.

`just build` also needs
[`flatc`](https://github.com/google/flatbuffers/releases), the FlatBuffers
compiler, at the version in `.flatc-version`, for `just cabin-ws-demo`.
`bootstrap` does not install it; it checks the version and prints the release to
download. CI installs the same version.

`just test` needs Node on `PATH` too: one test,
`crates/ridlc/tests/wasm_module.rs`, builds a generated crate for
`wasm32-unknown-unknown` and runs it in Node. It fails, and does not skip, when
`node` is missing. The test also needs network access, because it builds the
module with `cargo rustc`, which fetches the crate's crates.io dependencies, and
the `wasm32-unknown-unknown` target, which `rust-toolchain.toml` installs. The
CI `rust` job installs Node with `actions/setup-node`.

The package's browser test, `just ts-browser-e2e`, is not part of `just build`,
so the build needs no browser. CI runs it. To run it locally you need a
Chromium-family browser, such as Google Chrome or Chromium; set `RIDL_BROWSER`
to its path when the recipe does not find it.

The gate needs four tools `bootstrap` does not install, because they come from
package managers it should not write into on your behalf:
[`just`](https://github.com/casey/just), [`rustup`](https://rustup.rs) (it is
what applies the toolchain pin), [mdBook](https://rust-lang.github.io/mdBook/),
and [`jq`](https://jqlang.org) (`just demo` filters clippy output with it).
`bootstrap` names each one it cannot find, with the command that installs it,
and exits non-zero. Nothing is skipped when a tool is missing — the recipe that
needs it fails and says which one (ADR-0009).

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
   configured scopes are the language scopes (`typl`/`ridl`/`rxdl`/`rmdl`/
   `rsdl`), `family`, `roadmap`, `adr`, the compiler workspace scopes
   (`ridl-syntax`/`ridl-core`/`ridl-sem`/`ridl-ir`/`ridlc`/`ridl-lsp`/
   `ridl-mcp`/`ridl-backend-rust`/`ridl-backend-ts`/`ridl-backend-proto`/
   `ridl-backend-flatbuffers`/`ridl-diff`/`ridl-fmt`/`ridl-rt`/`xtask`/`editors`),
   and the repo-wide scopes (`repo`/`docs`/`ci`/`hooks`/`deps`).
4. Run `just verify` before opening a PR — commit-message lint over your branch
   range, then `just build`.
5. Open a PR. CI runs these same recipes — it installs the tools a runner needs
   and then invokes `just check`, `just compile`, `just test`, and the rest, so
   there is one definition of each command rather than two that drift
   (ADR-0009). On a pull request, a `changes` job skips the Rust jobs when every
   changed file is documentation or editor tooling that no test reads, and skips
   `just check` when every changed file is Rust source. A push to `main` runs
   every job.

## Changing a frozen surface

Eleven surfaces are frozen. Nine are frozen from release 0.10.0. The `manifest`
and `cli` surfaces are frozen from the first release after it, because
`ridl vendor` landed after the 0.10.0 tag. `just freeze-check` compares each one
with a baseline git ref (ADR-0029). The dumps are in `freeze/`, and the baseline
of each surface is in `freeze/surfaces.toml`. Some parts of the frozen surfaces
are frozen by policy and checked by no tool: the formats of `interfaces.lock`
and `.ridl/baseline`, the exit codes of every subcommand other than
`ridl check`, the output of `ridl diff` and the `ridl-diff` categories, the
layouts of the `unsubscribe`, `read` and `occurrence` frames, which no test
vector of the frame specification §11.1 shows, the payload-presence rule of
§11.1 for a `publish` under `Init` or `Invalid(Declared)` and for a `response`
under `reply`, which no vector shows, and the codes of §4 and §9.6 that no
vector carries, among them the `kind` values 2 and 5 and the `attach` encodings
2 and 3.

1. Run `cargo xtask freeze update` after a change that touches a surface, and
   commit the changed dumps. An added line is not a break.
2. If `just freeze-check` reports a missing line or a Rust API break, the change
   breaks the surface. Before 0.10.0, delete that surface's row from
   `freeze/surfaces.toml` in the same pull request, put `!` in the title, and
   state the approval and the reason in the description. The pull request merges
   only on the maintainer's explicit approval. Leave the surface's name in the
   `required` list. A follow-up pull request adds the row again with the merge
   commit of the first as the `baseline`. The surface is unchecked between the
   two pull requests, `just freeze-check` prints a notice while the row is
   missing, and `just release` fails.
3. During 0.10.x, do not break a frozen surface. A security fix is the one
   exception, and its reason is recorded in ADR-0029.
4. To start checking a new surface, add its dump in one pull request. Add its
   row to `freeze/surfaces.toml` in a follow-up pull request, with the merge
   commit of the first as the `baseline`. That pull request also adds its name
   to `required` in `freeze/surfaces.toml`; the check fails when a row's name is
   not in `required`.
5. The accepted-sources and wire dumps read frozen copies of the packages under
   `examples/` and `evals/corpus/`, in `xtask/tests/freeze-fixtures/sources/`,
   so an edit to an example or to the corpus changes no dump. To refresh a copy
   on purpose, copy the package's files over it and run
   `cargo xtask freeze update`. A new copy adds lines; a replaced copy that
   removes a line is a break, handled as in step 2. The wire dump also reads the
   veh-cluster copy under `xtask/tests/freeze-fixtures/generated/`, so a refresh
   of that copy changes both the generated-crate and the wire surfaces.

## Writing examples in the book

Every fenced block in `docs/book/` whose info string starts with `ridl`, `typl`
or `rsdl` is **compiled by `crates/ridl/tests/book_examples.rs`**, which runs
under `cargo test --workspace`. A verified block must be a complete,
self-contained package file — it declares its own `package`, and blocks sharing
a package name are staged side by side, so one can `import` from another
regardless of which comes first in the book.

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
  to name. The one exception is TYPL-406 (`missing-docs`): the harness leaves it
  out of a block whose fence does not name `allow=TYPL-406`, so that an example
  can leave items undocumented; a block that names it is checked for it like any
  other code;
- an `allow=` naming a code the block does **not** draw, so a marker cannot
  outlive the example it was written for;
- an `import` naming a package no block declares, a name no block in that
  package provides, or the block's own package. The compiler resolves the
  package and stops — an unresolved _name_ inside a package it found draws no
  diagnostic — so the harness checks it rather than trusting the gap;
- a language word that is not exactly `ridl`, `typl` or `rsdl` — `RIDL`,
  `ridl{.class}` — which mdBook still renders as an example a reader believes,
  while the convention does not recognise it;
- a book with no verified blocks at all.

A package name is a **book-wide** namespace, not a per-chapter one. Two chapters
that both declare `package veh.demo` are staged into one directory and collide
(`TYPL-009`) on every declaration they repeat, so give each chapter its own
package prefix.

The whole book is staged as **one workspace**, so an `rsdl` block names the
services and interfaces of `ridl` blocks in any chapter, and the book holds
**exactly one `system` fence**: a workspace declares at most one `system`, and a
second is RSDL-601. Every other `rsdl` fence declares components, distributions
or deployments, or is marked `rsdl,ignore`. The same rule makes the book's
closure complete: every deployment in the book places every instance of the
book's system.

`ignore` suppresses every one of those objections. If the harness refuses a
block you did not mean as an example, that is the answer.

One gap to know about: the diagnostic-catalogue drift check scans `.rs` sources
only, so a code written in Markdown — including in an `allow=` marker — is not
checked against the catalogue. A typo there shows up as a stale-allowance
failure rather than an unknown-code one (driftsys/ridl#191).

### Where a fence may sit

**Anywhere mdBook reads one.** Extraction uses `pulldown-cmark`, the CommonMark
parser mdBook itself uses, **with mdBook's exact option set**, so a fence inside
a list item at any depth, inside a block quote, or inside an HTML block is an
ordinary example. So is one after an unclosed fence earlier in the file, which
CommonMark closes at the end of its container.

Both halves matter. `Options::all()` differs from mdBook's five flags in three
that move fences — a fence under a `[^1]:` footnote definition, or inside a
leading `---…---` or `+++…+++` block, becomes invisible — and that produced a
fail-open one round after the parser fixed three others. The option set is
pinned in `MDBOOK_OPTIONS` with two tests on it. If you change it, change it
because you checked mdBook, not because a flag looked harmless.

Two consequences worth knowing:

- a block indented four spaces is an _indented_ code block in CommonMark. It has
  no info string, mdBook gives it no language class, and the harness skips it —
  so indenting a fence is a way to show one without it being compiled.
- the info string's language word must be exactly `ridl` or `typl`. `RIDL` and
  `ridl{.class}` render as examples but are refused, because nothing would have
  compiled them.
- an `{{#include}}` line is mdBook's own preprocessor directive. This harness
  never runs that preprocessor — it reads a file's raw text, so `{{#include}}`
  is ordinary text to it, and a fence inside the included file is never
  extracted or compiled. An example that must be verified belongs in a file
  under `docs/book/` directly, not in a file reached only through
  `{{#include}}`.

Earlier versions of this harness scanned lines by hand and failed _open_ three
times over — each time on a fence that mdBook rendered and the harness never
saw. Do not replace the parser with pattern matching.

A `ridl` fence quoted inside a longer fence — as in the samples above — is
documentation rather than an example, and is left alone.

Failures name the Markdown file and the exact line, because each block is staged
with the line offset it has in its source file.

The harness checks syntax, never claims. Prose about delivery, timing behaviour,
or provider-side enforcement describes a specification, not this workspace: the
one runtime here is `ridl-loopback`, which runs in process and is not reachable
from anything the CLI emits. Write it so a reader cannot mistake the two.

The fences in `skills/ridl-authoring/` follow the same rules and are compiled by
the test `skill_examples_compile`, as one workspace separate from the book.
`crates/ridl/tests/skill_gates.rs` also checks that every diagnostic code the
skill cites is listed in `freeze/diagnostics.txt`, and that each section of a
reference file links an existing heading of a language reference.

## Recipes

Run `just --list` for the full set. The common ones:

| recipe                 | what it does                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| ---------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `just fmt`             | reformat the connective tissue with prim                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               |
| `just check`           | lint gate — `prim fmt --check` + `prim lint`, no writes                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| `just toolchain-check` | the running toolchain is the one `rust-toolchain.toml` pins                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            |
| `just gate-parity`     | CI invokes every member of `just build`, and `build` keeps the members the recipe lists                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| `just fmt-check`       | `cargo fmt --all --check` (no writes)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| `just book-check`      | `mdbook build` on a copy — catches a SUMMARY.md mdBook cannot parse                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| `just doc-path-check`  | every `docs/` file path named in a tracked file resolves — the bare paths `link-check` cannot see, in prose, in an inline code span and in a source comment. A directory citation is checked when it ends with `/`; the slashless form (`docs/book`) is not. A file that is not text is named rather than scanned. Skips `docs/archive/`, `docs/wip/` and `CHANGELOG.md`; a path belonging to another repository has to be written as a URL                                                                                                                                                                                                                                            |
| `just story-id-check`  | no story id such as `E16.5` and no plan name such as `epic E11`, `stage K3` or `lane M` in a tracked file under `crates/`, `xtask/`, `examples/`, `editors/vscode/src/`, `packages/`, `docs/book/`, `docs/design/` or `docs/technotes/`, nor in a `docs/specification/` file that the book includes; names file:line for each match. The roadmap, the backlog, the ADRs, the rest of the specifications, `docs/archive/` and `docs/wip/` may hold ids                                                                                                                                                                                                                                  |
| `just freeze-check`    | each committed dump under `freeze/` matches a regeneration, no line of a surface's dump at its baseline ref in `freeze/surfaces.toml` is missing, and the runtime and generated crates have no Rust API break in their own items against the baseline (`cargo semver-checks`, ADR-0029; `ridl-ir` types in `ridl-descriptor` signatures are not compared, #1010), every row's surface is in the `required` list, and `required` names no unknown surface; with `--release`, every name in `required` has a row                                                                                                                                                                         |
| `just compile`         | compile the Rust workspace (`--locked`)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| `just test`            | run the Rust workspace test suite (`--locked`), and `ridl-descriptor`'s tests with its default features off, and with only `dedup` on                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| `just lint`            | `cargo clippy --workspace --all-targets -- -D warnings`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| `just wasm-check`      | `cargo check` for wasm32, `--no-default-features`; and `ridl-descriptor` with its default features off, and with only `dedup` on, for a target with no standard library, and with its features on for wasm32; and the `ridl-transport-ws` codec with its default features off, for wasm32 and for a target with no standard library                                                                                                                                                                                                                                                                                                                                                    |
| `just compat-check`    | build and test `ridl-rt` and `ridl-rt-conformance` as edition 2021 with its minimum supported Rust version and as edition 2024 with the `rust-toolchain.toml` pin, build the packaged `ridl-loopback` and `ridl-transport-ws` (default features on and off) with that minimum, build the packaged `ridl-ir` and `ridl-descriptor` with the `rust-version` of `ridl-descriptor`, and check the crate `ridl build` emits for `examples/cabin` (built with `rustc`, with only the `std` feature on) and for the `veh-cluster` corpus (built with `cargo`, every feature on) as edition 2021 with the `ridl-rt` minimum, the first cell of the codegen build matrix (ADR-0021 decision 10) |
| `just demo`            | generate the cabin example's crate and run the program that links it — each round trip's value is matched, and a missing one or a non-zero exit fails; it also checks that no planus crate and no prost crate is in the generated crate's dependency graph, and that the generated crate, and one generated from the veh-cluster corpus, check with their default features off for a target with no standard library                                                                                                                                                                                                                                                                   |
| `just ts-check`        | in `packages/ridl-runtime` and `packages/ridl-transport-ws`, with Deno: `deno fmt --check`, `deno lint`, `deno check` for Deno and against the DOM library, the unit tests under `tests/`, and `deno publish --dry-run`                                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| `just ts-e2e`          | build the `loopback_server` example of `ridl-transport-ws` and run the package's Deno end-to-end test against it                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| `just cabin-ws-demo`   | generate cabin's crate and its FlatBuffers schema, run `flatc` over the schema, check and build `examples/cabin/server`, then run the Deno program of `examples/cabin/web` against it: the program decodes the server's temperature payloads with the `flatc` code and fails unless every value is inside [-40..85], every received value is 1 to 63 steps ahead of the previous one on the ring of the 126 values (85 is followed by -40; a repeated value is ignored), 85 is reached by such a step, and -40 is seen after 85 (the first sample counts for neither bound); needs Deno and `flatc` at the version in `.flatc-version`                                                 |
| `just ts-browser-e2e`  | the same build, then the package's end-to-end test in a headless Chromium-family browser (`RIDL_BROWSER` names one); runs in CI, and is not a member of `just build`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   |
| `just ts-npm-check`    | in `packages/ridl-runtime` and `packages/ridl-transport-ws`, build an npm package with dnt, verify the built manifest and tarball, install the tarball in a temporary directory, and import it by name from Node; needs Deno, Node and npm on `PATH` and network access; is a member of `just build`, not of `just pre-push`; `.github/workflows/npm-release.yml` publishes both packages to npm on a `v*` tag                                                                                                                                                                                                                                                                         |
| `just build`           | `toolchain-check` + `gate-parity` + `install-check` + `fmt-check` + `book-check` + `link-check` + `doc-path-check` + `story-id-check` + `ts-check` + `freeze-check` + `compile` + `test` + `lint` + `wasm-check` + `compat-check` + `demo` + `ts-e2e` + `cabin-ws-demo` + `ts-npm-check` + `check` — the full local gate                                                                                                                                                                                                                                                                                                                                                               |
| `just lint-commits`    | `git std lint` over the commits on top of a base branch                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| `just pre-push`        | `lint-commits` + the static-check members of `build`, `ts-check` included; skips `freeze-check`, `compile`, `lint`, `test`, `wasm-check`, `compat-check`, `demo`, `ts-e2e`, `cabin-ws-demo`, `ts-npm-check`, and `install-check` — wired as the pre-push hook                                                                                                                                                                                                                                                                                                                                                                                                                          |
| `just verify`          | `lint-commits` + `build` — run before a PR                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |
| `just book`            | serve the mdBook docs locally                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| `just release`         | the freeze check with `--release`, then `git std bump` — version, changelog, tag                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |

## Reporting issues

Use GitHub issues on this repository. For security-sensitive reports, see
`SECURITY.md` instead of filing a public issue.
