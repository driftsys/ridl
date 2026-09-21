# AGENTS.md — RIDL

RIDL is a family of languages for modeling component-based reactive systems:
**one platform, four languages, one grammar, one intermediate representation.**
A shared vocabulary layer (`typl`) plus three description languages over it
(`ridl`, `rmdl`, `rsdl`), sharing one toolchain and one IR. ADR-0012 retired
`uxdl` as a family member and gave `ridl` a boundary model instead.

This repository holds the specifications, the architecture decision records
(ADRs), the implementation roadmap, and the compiler workspace: sixteen crates
under `crates/` — `ridl-syntax`, `ridl-core`, `ridl-sem`, `ridl-ir`, `ridlc`,
`ridl`, `ridl-lsp`, `ridl-mcp`, `ridl-backend-rust`, `ridl-backend-ts`,
`ridl-backend-proto`, `ridl-backend-flatbuffers`, `ridl-diff`, `ridl-fmt`,
`ridl-rt`, and `ridl-loopback` — plus `xtask` at the root, the `editors/vscode`
extension, and `examples/`, whose worked examples are compiled and run by the
test suite rather than being prose. The typl v0.1 toolchain (epic E1), the ridl
interface layer over it (epic E2) and rsdl's checks, lowering and `ridl diff` at
the system (epic E6) are built; the boundary model (epic E3) is sequenced in the
roadmap, and `rmdl` stays a Proposed draft with no implementation. See
`docs/technotes/walking-skeleton-architecture.md` for the as-built map.

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
  surface), ADR-0013 (codegen backend scope — _proposed_, classifies a backend
  by what its target can represent), ADR-0014 (the IR's own encodings —
  canonical protobuf JSON, prototext, binary; binds the artifact every future
  backend consumes), ADR-0015 (QoS absorption, the RPC response bound, the
  coherence rule, and composition of interfaces into a service; binds the
  language surface), ADR-0016 (schema projection and the pinned name transform;
  binds every backend that projects identity onto a target namespace), ADR-0017
  (the proto3 projection — how a foreign reference projects, where constraint
  information goes, and totality over names as well as numbers; read its
  decision 1 before writing another wire backend, because `generate_with` is the
  API every later wire backend inherits), ADR-0018 (the runtime core, two
  encodings, and what the backends emit — _proposed_; retracts the interaction
  layer the language backends shipped and restores it as a later phase, retires
  the extern-C face, fixes proto3 and FlatBuffers as the two core encodings —
  the 2026-09-12 re-scope adds `repr(C)` as a third and ADR-0020 amends decision
  3 in place — moves the store and dispatcher into Epic 11, and resolves the
  service-block conflict between ADR-0013 decision 2 and ADR-0016 decision 10;
  binds every backend and the runtime, and carries a 2026-09-12 amendment on
  decisions 3, 6, 15, 16 and 17 plus a record-wide note that `ridl-rt` names the
  engine here and the library everywhere else — read it before writing anything
  about what a backend emits), ADR-0019 (the FlatBuffers projection — a union
  isolated in a wrapper table, a non-table union arm boxed, every struct a
  `table`, a map with no `(key)`, the target's own name scopes, and `= null` on
  a field whose enum declares no zero member; binds the FlatBuffers backend
  only, and carries a 2026-09-21 amendment adding decision 8, the root rule:
  every declaration has a root table, and a named scalar, an enum and an enum
  set are rooted in `table <Name>Box { value: … (id: 0); }` — read it before
  changing what the FlatBuffers backend emits for a declaration that is not a
  struct or a union), ADR-0020 (the third payload encoding, the runtime layering
  and the codegen plugin system — _proposed_; `repr(C)` joins proto3 and
  FlatBuffers and the encoding matrix settles the codec-in-wasm boundary as
  FlatBuffers, `ridl-rt` is one crate with one cargo feature per encoding and
  the runtimes live outside it, and a backend becomes an executable over
  `generate(CodegenRequest) -> CodegenResponse` fed by a lowering step in the
  compiler — read it before writing a backend or a runtime library, and read its
  **Documents amended** table: ADR-0018's decisions 3, 6 and 15 rest on this
  record, its decisions 16 and 17 on the re-scope's other decisions, and
  ADR-0013's target list and ADR-0007 decision 13 change with them — the last is
  the only one of these that changes shipped code), ADR-0021 (the `ridl-rt` 0.1
  API decisions and the 0.x breaking-change rule; binds every consumer of
  `ridl-rt` — the Rust codegen, the runtimes, and story E14.2; its 2026-09-20
  amendment adds decisions 11 and 12, the port-trait forwarding impls and the
  handle model a runtime presents; the crate's as-built design record is
  `docs/design/ridl-rt.md`), ADR-0022 (the rsdl system in the IR — where the
  lowered system lives, that it is its own artifact
  `<pkg.Name>.system.{json,txtpb,binpb}` written by the three IR dump emits,
  which facts of rsdl §13 the IR states and which it does not, that a build
  whose only errors are RSDL-7xx writes every artifact and still exits 1, and
  that `ridl diff`'s system headings carry no verdict; binds the IR every later
  consumer reads, the `ridl build` contract and `ridl diff`. The as-built
  implementation record is `docs/technotes/rsdl-implementation.md`), ADR-0023
  (the generated interaction face's entry point, clause translator, and call
  signatures — the Rust backend's `generate_face` companion entry point over the
  unchanged pipeline `generate`, a narrow contract-clause translator that
  refuses every clause form it does not accept, a `Provider` method taking its
  argument by reference, and a consumer-side call returning a correlation on
  success and `SendError` on failure; binds every later story that extends the
  Rust backend's interaction face. Its 2026-09-20 amendment makes that success
  half the call's own correlation newtype and adds decision 5, a face that holds
  its port by value with no lifetime parameter — neither emitted yet, both
  landing with the `ridl-backend-rust` face change that follows the record; the
  amendment's own reasoning is on driftsys/ridl#429. The as-built design record
  is `docs/design/interaction-face.md`).
- `docs/ROADMAP.md` — the forward plan: the two steps it structures from the
  2026-09-12 re-scope's release scope (step 1, rsdl finalized plus the Rust
  runtime and codegen; step 2, TypeScript and the codegen plugin system), the
  parked blocks with the observation that reopens each, and the milestone
  summary. What has already shipped is in
  `docs/archive/roadmap-landed-record.md`.

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
    just gate-parity     CI invokes every member of just build
    just install-check   end-to-end test of install.sh (and install.ps1's dry
                         run) against a fixture release
    just fmt-check       cargo fmt --all --check (no writes; repair with cargo fmt --all)
    just book-check      mdbook build on a copy — catches a SUMMARY.md mdBook
                         cannot parse and a {{#include}} that does not resolve
                         (mdBook exits 0 on the second, so two checks read the
                         log and the rendered output)
    just link-check      every relative Markdown link resolves, over every
                         tracked .md — book-check cannot do this, because
                         mdBook exits 0 on an unresolved relative link
    just doc-path-check  every docs/ file path named in a tracked file
                         resolves — the bare paths link-check cannot see, in
                         prose, in an inline code span, and in a source
                         comment. A directory citation carries no extension,
                         so it is not checked. Skips docs/archive/ and
                         docs/wip/, where such a path records what was true
                         when it was written
    just compile         compile the Rust workspace (--locked)
    just test            run the Rust workspace test suite (--locked)
    just lint            cargo clippy --workspace --all-targets -- -D warnings
    just wasm-check      cargo check for wasm32 with --no-default-features
    just compat-check    build and test the packaged ridl-rt crate as edition
                         2021 with the rust-version in crates/ridl-rt/Cargo.toml
                         and as edition 2024 with the rust-toolchain.toml pin,
                         and check its LICENSE (ADR-0021 decision 10)
    just build           toolchain-check + gate-parity + install-check +
                         fmt-check + book-check + link-check + doc-path-check +
                         compile + test + lint + wasm-check + compat-check +
                         check — the full local gate: every member ADR-0008
                         decision 11 names, the four CI checks ADR-0009 brought
                         back to this side, and doc-path-check, which postdates
                         both
    just lint-commits    git std lint over the commits on top of a base branch
                         (BASE defaults to main; CI passes the PR base branch)
    just verify          lint-commits, then build — run before a PR
    just book            serve the mdBook docs locally
    just book-build      render the book to ./book — what CI publishes to Pages
    just vscode-verify   compile, test, and vsce package the VS Code extension
                         with no bundled binary
    just package-vsix    compile the extension, then vsce package (optional
                         vsce-target)
    just package-vscode  build ridl for this machine and package the extension
                         (local testing)
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
parameter and pass it (as `commit-lint` does with `just lint-commits <base>`);
do not write a second copy of the command into the workflow (ADR-0009).

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
- **`prim lint` enforces Markdown content rules at its floor tier** — the 12
  always-on defect rules (a broken in-document anchor, an undefined reference, a
  malformed table, and the like — not cross-file link resolution, which
  `just link-check` covers); this repo does not opt into prim's strict
  (convention) tier. prim has no autofixable content rules yet — `prim fix` is
  currently identical to `prim fmt` — so a floor-tier finding is repaired by
  hand.
- **Every `ridl`/`typl`/`rsdl` fenced block in `docs/book/` is compiled** by
  `crates/ridl/tests/book_examples.rs`, and must draw no diagnostic its fence
  does not name — nor name one it does not draw. A verified block declares its
  own `package` and is a whole file; a fragment is marked `` ```ridl,ignore ``;
  a deliberate diagnostic is marked `` ```ridl,allow=<CODE> ``. Package names
  are book-wide, and the book is one workspace, so it holds exactly one `system`
  fence (RSDL-601). Extraction uses `pulldown-cmark` with mdBook's exact option
  set (`MDBOOK_OPTIONS`), so a fence anywhere mdBook reads one _in that file_ is
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
- **The book describes the system as built.** The one runtime in this workspace
  is `ridl-loopback`, which runs in process, is reached only from a test or a
  program that links it, and is not reachable from anything the CLI emits (story
  E11.14). So prose about delivery, timing behaviour, or provider-side contract
  enforcement is still describing the specification — say so where it appears.
- **Prose — comments, commit messages, docs, PR descriptions — is plain and
  literal**: no idioms, no figures of speech. Technical terms and acronyms stay
  as they are.
- Documents are prose, in Markdown, under `docs/`. The specs read as one system:
  doctrines are indexed once in the overview, cited from each reference — keep
  that discipline when editing.

<!-- git-std:bootstrap -->

## Post-clone setup

Run `./bootstrap` after `git clone` or `git worktree add`.
