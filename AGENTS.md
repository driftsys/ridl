# AGENTS.md — RIDL

RIDL is a family of languages for modeling component-based reactive systems:
**one platform, four languages, one grammar, one intermediate representation.**
A shared vocabulary layer (`typl`) plus three description languages over it
(`ridl`, `rmdl`, `rsdl`), sharing one toolchain and one IR. ADR-0012 retired
`uxdl` as a family member and gave `ridl` a boundary model instead.

This repository holds the specifications, the architecture decision records
(ADRs), the implementation roadmap, and the compiler workspace: eleven crates
under `crates/` — `ridl-syntax`, `ridl-core`, `ridl-sem`, `ridl-ir`, `ridlc`,
`ridl`, `ridl-lsp`, `ridl-backend-rust`, `ridl-backend-ts`, `ridl-diff`, and
`ridl-fmt` — plus `xtask` at the root and the `editors/vscode` extension. The
typl v0.1 toolchain (epic E1) and the ridl interface layer over it (epic E2) are
built; the boundary model (epic E3), `rmdl`, and `rsdl` are sequenced in the
roadmap. See `docs/technotes/walking-skeleton-architecture.md` for the as-built
map.

**Read these before doing anything else in this repo:**

- `docs/specification/ridl-family-overview.md` — the entry point: the map, the
  shared doctrines (indexed once), the decision ledger, and the open-question
  index. Start here.
- `docs/wip/ridl-family-concept.md` — the concept note: motivation, cores,
  profiles, the platform/IR model, the naming ledger (pre-ADR).
- `docs/wip/family-general-form.md` — the surface rules shared by every profile:
  the three declaration shapes, the nine surface invariants, the attribute model
  (pre-ADR working spec).
- `docs/specification/{typl,ridl,rmdl,rsdl}-language-reference.md` — the four
  language references, plus `rxdl-language-reference.md` for the unrestricted
  profile and the domain spellings (a spelling layer, not a language — it adds
  no semantics). The retired `uxdl` reference is at
  `docs/archive/uxdl-language-reference-v0.1.md`; read it as prior work, never
  as the current design.
- `docs/decisions/` — ADR-0002 (module system), ADR-0004 (sequencing and stack),
  ADR-0005 (agent enablement), ADR-0006 (E0 execution), ADR-0007 (E1 execution),
  ADR-0008 (E2 execution — read its `## Status` before editing it), ADR-0009
  (toolchain pin and gate parity — binds every contributor, not one epic),
  ADR-0010 (CLI conventions — binds every subcommand), ADR-0011 (the
  provisioned-constant keyword is `fixed` — supersedes ADR-0008 decision 5),
  ADR-0012 (the interaction boundary model — retires uxdl, gives ridl five
  interaction families and their correspondence obligations; binds the language
  surface).
- `docs/ROADMAP.md` — the epics, stories, and the V1 (contract platform) / V2
  (executable platform) release split.

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

    just fmt             reformat connective tissue with prim + fix Markdown
    just check           lint gate — prim --check + markdownlint (no writes)
    just toolchain-check the running toolchain is the one rust-toolchain.toml pins
    just gate-parity     CI invokes every member of just build
    just fmt-check       cargo fmt --all --check (no writes; repair with cargo fmt --all)
    just book-check      mdbook build on a copy — catches a SUMMARY.md mdBook
                         cannot parse and a {{#include}} that does not resolve
                         (mdBook exits 0 on the second, so two checks read the
                         log and the rendered output)
    just link-check      every relative Markdown link resolves, over every
                         tracked .md — book-check cannot do this, because
                         mdBook exits 0 on an unresolved relative link
    just compile         compile the Rust workspace (--locked)
    just test            run the Rust workspace test suite (--locked)
    just lint            cargo clippy --workspace --all-targets -- -D warnings
    just wasm-check      cargo check for wasm32 with --no-default-features
    just build           toolchain-check + gate-parity + fmt-check + book-check +
                         link-check + compile + test + lint + wasm-check + check — the full
                         local gate: every member ADR-0008 decision 11 names, plus
                         the four CI checks ADR-0009 brought back to this side
    just lint-commits    git std lint over the commits on top of a base branch
                         (BASE defaults to main; CI passes the PR base branch)
    just verify          lint-commits, then build — run before a PR
    just book            serve the mdBook docs locally
    just book-build      render the book to ./book — what CI publishes to Pages
    just release         git std bump — version, changelog, tag
    just install         ./bootstrap — toolchain, git hooks, gate requirements

Full recipe set: `justfile`. The toolchain conventions come from
driftsys/git-std (commits, versioning, hooks) and driftsys/prim
(connective-tissue formatting).

**The justfile is the single definition of every gate command.**
`.github/workflows/ci.yml` installs tools and then invokes these recipes; what
remains in the workflow is tool installation and job plumbing, never a gate
command. Adding a check means adding a recipe, adding it to `build`, and adding
`run: just <recipe>` to the workflow — `just gate-parity` fails until the last
of those is done. When CI needs a variant of a check, give the recipe a
parameter and pass it (as `convco` does with `just lint-commits <base>`); do not
write a second copy of the command into the workflow (ADR-0009).

`gate-parity` covers only the members of `build`. `verify` and `lint-commits`
are outside its reach, which is where the workflow and the justfile last drifted
apart unnoticed — check those two by reading when you touch either file.

## Conventions

- **Conventional Commits**, linted by git-std against `.git-std.toml` — types
  and scopes are enumerated there. Never push directly to `main`; use a PR.
- **Every crate lives at `crates/<crate-name>/`** — the directory name equals
  the crate name. `xtask` at the root is the one exception. A new crate adds its
  own scope to `.git-std.toml`, which is an explicit list, not path-derived
  (issue #180).
- **prim owns the connective tissue** (Markdown/JSON/YAML/TOML) — it honors
  `.editorconfig` only, no per-tool config. `.primignore` is the escape hatch
  for files that must stay byte-exact.
- **markdownlint** enforces Markdown style (`.markdownlint.json`).
- **Every `ridl`/`typl` fenced block in `docs/book/` is compiled** by
  `crates/ridl/tests/book_examples.rs`, and must draw no diagnostic its fence
  does not name — nor name one it does not draw. A verified block declares its
  own `package` and is a whole file; a fragment is marked `` ```ridl,ignore ``;
  a deliberate diagnostic is marked `` ```ridl,allow=<CODE> ``. Package names
  are book-wide. Extraction uses `pulldown-cmark` with mdBook's exact option set
  (`MDBOOK_OPTIONS`), so a fence anywhere mdBook reads one _in that file_ is
  verified — do not replace it with pattern matching, and do not widen the
  options. **The one exception is `{{#include}}`**, which the harness does not
  expand: fences inside an included file are not compiled. That is what keeps
  the six Language reference chapters — thin wrappers over `docs/specification/`
  — out of the harness. A fence you want verified must sit in a `docs/book/`
  file directly. See `CONTRIBUTING.md`, "Writing examples in the book".
- **Diagnostic codes written in Markdown are unguarded.** The catalogue drift
  check (issue #189) scans `.rs` sources only, so a `TYPL-`/`RIDL-` code cited
  in `docs/` — including an `allow=<CODE>` fence marker — is not checked against
  the catalogue. Recorded on driftsys/ridl#191.
- **The book describes the system as built.** There is no runtime in this
  workspace, so prose about delivery, timing behaviour, or provider-side
  contract enforcement is describing the specification — say so where it
  appears.
- **Prose — comments, commit messages, docs, PR descriptions — is plain and
  literal**: no idioms, no figures of speech. Technical terms and acronyms stay
  as they are.
- Documents are prose, in Markdown, under `docs/`. The specs read as one system:
  doctrines are indexed once in the overview, cited from each reference — keep
  that discipline when editing.

<!-- git-std:bootstrap -->

## Post-clone setup

Run `./bootstrap` after `git clone` or `git worktree add`.
