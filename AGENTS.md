# AGENTS.md — RIDL

RIDL is a family of languages for modeling component-based reactive systems:
**one platform, four languages, one grammar, one intermediate representation.**
A shared vocabulary layer (`typl`) plus three description languages over it
(`ridl`, `rmdl`, `rsdl`), sharing one toolchain and one IR. ADR-0012 retired
`uxdl` as a family member and gave `ridl` a boundary model instead.

This repository holds the specifications, the architecture decision records
(ADRs), the implementation roadmap, and the compiler workspace: twenty-one
crates under `crates/` — `ridl-syntax`, `ridl-core`, `ridl-sem`, `ridl-ir`,
`ridl-descriptor` (the catalog descriptor's schema, accessors and verifier),
`ridlc`, `ridl`, `ridl-lsp`, `ridl-mcp`, `ridl-backend-rust`, `ridl-backend-ts`,
`ridl-backend-proto`, `ridl-backend-flatbuffers`, `ridl-diff`, `ridl-fmt`,
`ridl-rt`, `ridl-loopback`, `ridl-transport-ws` (the WebSocket binding of the
frame), `ridl-rt-conformance` (the port contract tests any runtime runs,
published for runtimes outside this repository), and `ridlc-gen-model` and
`ridlc-gen-rust` (the reference codegen plugins, test-only) — plus `xtask` at
the root, `packages/` (the TypeScript packages; `packages/ridl-transport-ws` is
the Deno and browser client of the WebSocket binding, and
`packages/ridl-runtime` loads the generated wasm codec), the `editors/vscode`
extension, `skills/` (the `ridl-authoring` agent skill, which the extension and
the editor release package), `evals/` (the public corpus, the calibration
records and the eval tasks for design evaluation, which `cargo xtask eval` runs,
outside the published crates and the book), and `examples/`, whose worked
examples are compiled and run by the test suite rather than being prose. The
typl v0.1 toolchain (epic E1), the ridl interface layer over it (epic E2) and
rsdl's checks, lowering and `ridl diff` at the system (epic E6) are built; the
boundary model (epic E3) is sequenced in the roadmap, and `rmdl` stays a
Proposed draft with no implementation. See
`docs/technotes/walking-skeleton-architecture.md` for the as-built map.

**Start from the map for your task, then read what the task touches.**

- `docs/technotes/walking-skeleton-architecture.md` — the as-built map of the
  workspace: which crate owns what. Read it before a code change in `crates/`.
- `docs/specification/ridl-family-overview.md` — the map of the specifications:
  the doctrines, the decision ledger, the open questions. Read it before editing
  a specification or an ADR; its footer lists the sections to update when a
  reference changes.
- `docs/specification/family-general-form.md` — the three declaration shapes,
  the nine surface invariants, the attribute model. Read it before changing the
  grammar, the parser, an attribute, or `ridl-fmt`.
- `docs/specification/{typl,ridl,rsdl}-language-reference.md` — read one before
  changing that language's parser, checks, or lowering; read
  `docs/specification/expr-core-specification.md` before changing expression
  evaluation. rmdl and rxdl have references but no implementation.
- `docs/decisions/` — the ADRs. `docs/decisions/README.md` summarises each one;
  the record's own `## Status` is authoritative for its status and amendments.
  Read the record itself before the matching work:
  - ADR-0002 — before changing packages, imports, visibility, the manifest, or
    the resolver and lockfile.
  - ADR-0005 (_proposed_) — before building agent-facing tooling: `ridl-mcp`,
    `ridl-lsp`, or a skill.
  - ADR-0008 — read its `## Status` before editing it.
  - ADR-0009 (toolchain pin and gate parity) and ADR-0010 (CLI conventions) —
    bind every contributor and every subcommand.
  - ADR-0011, ADR-0012 and ADR-0015 — before changing the language surface.
  - ADR-0013 (codegen backend scope, _proposed_), ADR-0014 (the IR's encodings)
    and ADR-0016 (the pinned name transform) — bind every backend.
  - ADR-0017 decision 1 — before writing another wire backend; `generate_with`
    is the API every later wire backend inherits.
  - ADR-0018 (_proposed_) — before writing anything about what a backend emits.
    In ADR-0018 `ridl-rt` names the engine; everywhere else it names the
    library.
  - ADR-0019 decision 8 — before changing what the FlatBuffers backend emits for
    a declaration that is not a struct or a union.
  - ADR-0020 (_proposed_) — before writing a backend, a runtime library, or the
    codegen plugin protocol; read its **Documents amended** table first.
  - ADR-0021 and `docs/design/ridl-rt.md` — before changing `ridl-rt` or
    anything that consumes it.
  - `docs/design/ridl-transport-ws.md` — before changing `ridl-transport-ws`.
  - ADR-0022 and `docs/technotes/rsdl-implementation.md` — before changing the
    IR's system artifact, the `ridl build` contract, or `ridl diff`.
  - ADR-0023 and `docs/design/interaction-face.md` — before extending the Rust
    backend's interaction face; its decision 6 before changing what the face
    emits for a call.
  - ADR-0024 and `docs/book/lints.md` — before adding or renaming a warning or
    info diagnostic code, changing the `[lints]` table, or changing which
    commands apply lint levels.
  - ADR-0025 and `docs/design/mcp-workspace-tools.md` — before changing a tool
    of `ridl-mcp`, its input or output schema, `tools.json`, or the overlay
    loader.
  - ADR-0026 and `docs/book/documenting.md` — before changing doc comments:
    their carriers, links, tags, the doc lints, the IR's doc fields,
    `ridl_ir::rules`, or how a workspace member finds its root.
  - ADR-0027 and `docs/design/design-lints.md` — before adding a design lint,
    changing the level or the threshold of one, or changing the corpus, the
    calibration records or an eval task under `evals/`.
  - ADR-0028 and `docs/book/generated-code.md` — before changing what a backend
    renders in a generated doc comment, or the doc fields of the codegen
    `Model`.
  - ADR-0029, `docs/book/stability.md` and `freeze/README.md` — before changing
    a frozen surface, a dump under `freeze/`, `freeze/surfaces.toml`, or the
    freeze check in `xtask/src/freeze.rs` and `xtask/src/freeze/`.
- `docs/ROADMAP.md` — the forward plan: the two steps it structures from the
  2026-09-12 re-scope's release scope (step 1, rsdl finalized plus the Rust
  runtime and codegen; step 2, TypeScript and the codegen plugin system), the
  parked blocks with the observation that reopens each, and the milestone
  summary. What has already shipped is in
  `docs/archive/roadmap-landed-record.md`. Read it before planning or picking up
  a story.

These are living records. A decision that changes one is recorded there directly
— don't silently diverge from it.

## The family

    typl   type language                 — data: types, ranges, units, constants
    ridl   interface description         — interactions at every boundary: system,
                                           person, world (ADR-0012)
    rmdl   model description             — behaviour: functions + reactive models
    rsdl   system description            — architecture: components, wiring, deployment

Dependency lattice: `typl ← {ridl, rmdl} ← rsdl`. typl is the only standalone
member; rsdl is the apex.

## Commands

    just fmt             reformat connective tissue with prim
    just check           lint gate — prim fmt --check + prim lint (no writes)
    just toolchain-check the running toolchain is the one rust-toolchain.toml pins
    just gate-parity     CI invokes every member of just build, and the members
                         of build match the expected list in the recipe
    just install-check   end-to-end test of install.sh (and install.ps1's dry
                         run) against a fixture release
    just fmt-check       cargo fmt --all --check (no writes; repair with cargo fmt --all)
    just book-check      mdbook build on a copy of docs/ and of the
                         examples/**/*.rs and examples/**/*.ridl sources —
                         catches a SUMMARY.md mdBook cannot parse, a
                         {{#include}} that does not resolve, an include whose
                         anchor has no ANCHOR or no ANCHOR_END in its file,
                         and a chapter file SUMMARY.md names that mdBook
                         creates instead of reporting as missing (mdBook exits
                         0 on the last three, so four checks read the log, the
                         rendered output, the included anchors, and the copy's
                         file list)
    just link-check      every relative Markdown link resolves, over every
                         tracked .md — book-check cannot do this, because
                         mdBook exits 0 on an unresolved relative link
    just doc-path-check  every docs/ file path named in a tracked file
                         resolves — the bare paths link-check cannot see, in
                         prose, in an inline code span, and in a source
                         comment. A directory citation is checked when it
                         ends with /; the slashless form (docs/book) is not.
                         Skips docs/archive/, docs/wip/ and CHANGELOG.md,
                         where such a path records what was true when it was
                         written (the changelog is generated from commit
                         history)
    just story-id-check  no story id (`E16.5`, `E2.8b`) and no plan name
                         (`epic E11`, `stage K3`, `lane M`, `E2 task 9`) in a
                         tracked file under crates/, xtask/, examples/,
                         editors/vscode/src/, packages/, docs/book/,
                         docs/design/ or docs/technotes/, nor in a
                         docs/specification/ file that the book includes —
                         names file:line for each match. The ROADMAP, the
                         backlog, the ADRs, the rest of the specifications,
                         docs/archive/ and docs/wip/ may hold ids
    just freeze-check    every committed dump under freeze/ matches the
                         regenerated one, and no line of a dump at its
                         baseline ref is missing now (the baseline refs are
                         in freeze/surfaces.toml; repair a stale dump with
                         cargo xtask freeze update). It also fails on a row
                         whose surface is not in the `required` list of
                         surfaces.toml, on an unknown or repeated name there,
                         and with --release on a required name without a row
    just compile         compile the Rust workspace (--locked)
    just test            run the Rust workspace test suite (--locked), and
                         ridl-descriptor's tests with its default
                         features off, and with only dedup on
    just lint            cargo clippy --workspace --all-targets -- -D warnings
    just wasm-check      cargo check for wasm32 with --no-default-features,
                         and ridl-descriptor with its default features off,
                         and with only dedup on, for a
                         target with no standard library, and with its
                         features on for wasm32, and the ridl-transport-ws
                         codec (default features off) for wasm32 and for
                         thumbv7em-none-eabihf
    just compat-check    build and test the packaged ridl-rt and
                         ridl-rt-conformance crates as edition
                         2021 with the rust-version in crates/ridl-rt/Cargo.toml
                         and as edition 2024 with the rust-toolchain.toml pin,
                         check both crates' LICENSE files, build the packaged
                         ridl-loopback and ridl-transport-ws (with its default
                         features on and off) at that rust-version, build the packaged
                         ridl-ir and ridl-descriptor at the rust-version in
                         crates/ridl-descriptor/Cargo.toml, and check the crate
                         ridl build emits for examples/cabin (built with
                         rustc, with only the std feature on) and for the
                         veh-cluster corpus (built with cargo, every feature
                         on and default features off) as edition 2021 with the
                         ridl-rt rust-version, the first cell of the codegen
                         build matrix (ADR-0021 decision 10)
    just demo            generate examples/cabin's crate with ridl build and run
                         the program that links it — each round trip's value
                         is matched, and a missing one or a non-zero exit
                         fails; it also checks that no planus crate and no
                         prost crate is in the generated crate's dependency
                         graph, and that the
                         generated crate, and one generated from the
                         veh-cluster corpus, check with their default features off
                         for thumbv7em-none-eabihf, a target with no standard
                         library; the corpus crate is also linted with clippy,
                         with its default features and with them off for that
                         target, under -D warnings, so any diagnostic fails
                         it, dead_code included; the lints that its lib.rs
                         allows stay allowed; both generated crates are also
                         built with cargo doc, denying
                         rustdoc::broken_intra_doc_links and
                         rustdoc::private_intra_doc_links.
                         examples/cabin is its own
                         cargo workspace, outside this one, and carries the fmt
                         and clippy checks for its consumer, which --all over
                         this workspace cannot reach
    just ts-check        in packages/ridl-runtime and packages/ridl-transport-ws,
                         with Deno:
                         deno fmt --check, deno lint, deno check for Deno
                         and against the DOM library, the unit tests under
                         tests/, and deno publish --dry-run. The Deno version
                         is pinned in .deno-version
    just ts-e2e          build the loopback_server example of
                         ridl-transport-ws and run the package's Deno
                         end-to-end test against it
    just cabin-ws-demo   generate cabin's crate and its FlatBuffers schema with
                         ridl build, run flatc over the schema, check the
                         format and lint of examples/cabin/server and build it,
                         then run the Deno program of examples/cabin/web
                         against it: the program decodes the server's
                         temperature payloads with the flatc code and fails
                         unless it sees the whole range [-40..85] in order.
                         Needs Deno and flatc at the version in .flatc-version
    just ts-browser-e2e  the same build, then the package's end-to-end test
                         in a headless browser; needs a Chromium-family
                         browser (RIDL_BROWSER names one). Runs in CI; not a
                         member of build, so a contributor's build needs no
                         browser
    just ts-npm-check    build packages/ridl-runtime and packages/ridl-transport-ws
                         into npm packages with dnt, verify the built manifest and tarball,
                         install the tarball in a temporary directory, and
                         import it by name from Node. Needs deno, node and npm
                         on PATH and network access. Not a member of pre-push.
                         .github/workflows/npm-release.yml publishes the
                         packages to npm on a v* tag
    just build           toolchain-check + gate-parity + install-check +
                         fmt-check + book-check + link-check + doc-path-check +
                         story-id-check + ts-check + freeze-check + compile +
                         test + lint + wasm-check + compat-check + demo +
                         ts-e2e + cabin-ws-demo + ts-npm-check + check — the full local gate: every member
                         ADR-0008 decision 11 names, the four CI checks
                         ADR-0009 brought back to this side, and
                         doc-path-check, story-id-check, freeze-check, demo,
                         ts-check, ts-e2e, cabin-ws-demo and ts-npm-check, which postdate
                         both
    just lint-commits    git std lint over the commits on top of a base branch
                         (BASE defaults to main; CI passes the PR base branch)
    just pre-push        lint-commits + the static-check members of build,
                         ts-check included; skips freeze-check, compile, lint,
                         test, wasm-check, compat-check, demo, ts-e2e,
                         cabin-ws-demo, ts-npm-check, and install-check — wired as the pre-push hook
    just verify          lint-commits, then build — run before a PR
    just book            serve the mdBook docs locally
    just book-build      render the book to ./book — what CI publishes to Pages
    just vscode-verify   compile, test, and package the VS Code extension as a
                         generic and a linux-x64 VSIX, with a placeholder
                         binary, and check each archive and the skill archive
                         that package-skill writes
    just package-vsix    compile the extension, then vsce package (optional
                         vsce-target)
    just package-vscode  build ridl for this machine and package the extension
                         (local testing)
    just release         freeze check --release, then git std bump — version,
                         changelog, tag
    just install         ./bootstrap — toolchain, git hooks, gate requirements

Full recipe set: `justfile`. The toolchain conventions come from
driftsys/git-std (commits, versioning, hooks) and driftsys/prim
(connective-tissue formatting).

**The justfile is the single definition of every gate command.**
`.github/workflows/ci.yml` installs tools and then invokes these recipes; what
remains in the workflow is tool installation and job plumbing, never a gate
command. Adding a check means adding a recipe, adding it to `build`, adding its
name to the `expected` list in the `gate-parity` recipe, and adding
`run: just <recipe>` to the workflow — `just gate-parity` fails until the last
two of those are done. When CI needs a variant of a check, give the recipe a
parameter and pass it (as `commit-lint` does with `just lint-commits <base>`);
do not write a second copy of the command into the workflow (ADR-0009).

`gate-parity` covers only the members of `build`. `verify`, `lint-commits`, and
`pre-push` are outside its reach, which is where the workflow and the justfile
last drifted apart unnoticed — check those by reading when you touch any of
them.

## Conventions

- **Conventional Commits**, linted by git-std against `.git-std.toml` — types
  and scopes are enumerated there. Never push directly to `main`; use a PR.
- **Every crate lives at `crates/<crate-name>/`** — the directory name equals
  the crate name. `xtask` at the root is one exception; `crates/ridl/`, whose
  manifest names its package `ridl-cli` because `ridl` is already an unrelated
  crate on crates.io, is the other — the compiled binary and every doc mention
  of the command still say `ridl`, only the crates.io publish identity differs.
  A new crate adds its own scope to `.git-std.toml`, which is an explicit list,
  not path-derived (issue #180).
- **prim owns the connective tissue** (Markdown/JSON/YAML/TOML) — it honors
  `.editorconfig` only, no per-tool config. `.primignore` is the escape hatch
  for files that must stay byte-exact.
- **`prim lint` enforces Markdown content rules at its floor tier** — the 12
  always-on defect rules (a broken in-document anchor, an undefined reference, a
  malformed table, and the like — not cross-file link resolution, which
  `just link-check` covers); this repo does not opt into prim's strict
  (convention) tier. prim has no autofixable content rules yet — `prim fix` is
  currently identical to `prim fmt` — so a floor-tier finding is repaired by
  hand.
- **Every `ridl`/`typl`/`rsdl` fenced block in `docs/book/` is compiled** by
  `crates/ridl/tests/book_examples.rs`, and must draw no diagnostic its fence
  does not name — nor name one it does not draw. The harness leaves TYPL-406
  (`missing-docs`) out of a block whose fence does not name it, so that an
  example can leave items undocumented. A verified block declares its own
  `package` and is a whole file; a fragment is marked `` ```ridl,ignore ``; a
  deliberate diagnostic is marked `` ```ridl,allow=<CODE> ``. Package names are
  book-wide, and the book is one workspace, so it holds exactly one `system`
  fence (RSDL-601). Extraction uses `pulldown-cmark` with mdBook's exact option
  set (`MDBOOK_OPTIONS`), so a fence anywhere mdBook reads one _in that file_ is
  verified — do not replace it with pattern matching, and do not widen the
  options. **The one exception is `{{#include}}`**, which the harness does not
  expand: fences inside an included file are not compiled. That is what keeps
  the eight Language reference chapters — thin wrappers over
  `docs/specification/` — out of the harness. A fence you want verified must sit
  in a `docs/book/` file directly. See `CONTRIBUTING.md`, "Writing examples in
  the book".
- **Diagnostic codes written in Markdown are unguarded.** The catalogue drift
  check (issue #189) scans `.rs` sources only, so a `TYPL-`/`RIDL-` code cited
  in `docs/` — including an `allow=<CODE>` fence marker — is not checked against
  the catalogue. Recorded on driftsys/ridl#191.
- **The book describes the system as built.** The one runtime in this workspace
  is `ridl-loopback`, which runs in process, is reached only from a test or a
  program that links it, and is not reachable from anything the CLI emits (the
  section "The face is emitted by `ridl build`" of
  `docs/design/interaction-face.md` says what the CLI does emit). So prose about
  delivery, timing behaviour, or provider-side contract enforcement is still
  describing the specification — say so where it appears.
- **Prose — comments, commit messages, docs, PR descriptions — is plain and
  literal**: no idioms, no figures of speech. Technical terms and acronyms stay
  as they are.
- **Shipped docs and rustdoc do not name stories, stages or epics** (`E16.5`),
  because status lives in `docs/ROADMAP.md` and the issue tracker, and a
  reference to a story goes stale when the story lands. State what the code does
  now; when a gap is real, link its tracking issue (`driftsys/ridl#N`). State a
  fact once, in the record that owns it, and link to that record from elsewhere
  instead of restating it. `just story-id-check` enforces the first rule for
  dotted story ids and for the plan names `epic E<n>`, `stage <Letter><n>[a-z]`,
  `lane <Letter>` and `E<n> task <n>`.
- Documents are prose, in Markdown, under `docs/`. The specs read as one system:
  doctrines are indexed once in the overview, cited from each reference — keep
  that discipline when editing.

<!-- git-std:bootstrap -->

## Post-clone setup

Run `./bootstrap` after `git clone` or `git worktree add`.
