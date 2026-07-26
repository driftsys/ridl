# ADR-0009: Toolchain Pin and Gate Parity

## Status

Accepted — 2026-07-26. Scope: the build and check tooling, not any language or
crate. This ADR is not epic-scoped: it binds every contributor and every future
epic until it is superseded. It follows the ADR-0006 / ADR-0007 / ADR-0008
pattern of recording an agent-taken decision for after-the-fact maintainer
review.

It refines ADR-0008 decision 11, which names five commands as the merge gate, by
fixing what those commands run against. It changes none of them.

## Context

ADR-0006 decision 8 made the gate local while CI is stuck, and ADR-0008 decision
11 carried that forward. Issue #182 then found that two of decision 11's five
members were not reachable from `just build`, and PR #184 wired them in. Review
of #184 found the deeper problem: the local gate and `.github/workflows/ci.yml`
were two independent copies of the same command set, and they had drifted in
four places. Review of the commit that fixed those four found a fifth. Every one
of the five let a tree that is clean locally fail CI, or the reverse.

1. **No toolchain pin.** There was no `rust-toolchain.toml` and no
   `rustfmt.toml`. CI installed `dtolnay/rust-toolchain@stable`; a contributor
   ran whatever their rustup default was. rustfmt output and clippy lints change
   between releases, so "the local gate matches CI" was true of the command text
   and false of what the commands measured. Measured on this repository's Rust
   sources, which this branch does not touch: `cargo fmt --all --check` exits 0
   under rustfmt 1.9.0-stable (rustc 1.95.0) and exits 1 with 3385 diff lines
   under rustfmt 1.8.0-nightly, on the same tree.

2. **`--locked` missing locally.** CI ran `cargo build --workspace --locked` and
   `cargo test --workspace --locked`; `just compile` and `just test` omitted the
   flag. A manifest edit that leaves `Cargo.lock` stale was therefore repaired
   silently on the contributor's machine and rejected on the runner.

3. **`mdbook build` ran in CI and nowhere else.** The justfile's only mdBook
   line was `mdbook serve`, which starts a server and cannot be a gate member. A
   `SUMMARY.md` that mdBook cannot parse was unreportable before pushing.

4. **markdownlint was invoked two ways.** CI ran `npx --yes markdownlint-cli`
   with `--ignore book --ignore node_modules`; the local recipe ran the
   installed binary with no flags and relied on `.markdownlintignore`. That
   file's own header explains why the flag form does not work and states that
   the list lives in the file instead — yet CI still passed the flags. The two
   exclusion lists were free to disagree, and did: with `book/probe.md` and
   `node_modules/probe.md` holding one hard tab each, and those two directories
   removed from `.markdownlintignore`, CI's command exits 0 and the local
   command exits 1 on the same tree.

5. **The commit-message lint was written down twice, and the copies had already
   drifted.** The `convco` job ran
   `git std lint --range
   origin/$base_ref..HEAD`; the `verify` recipe ran
   `origin/main..HEAD`. They agreed only for PRs based on `main`. This one was
   not in the review that opened this ADR — it was found in review of the commit
   that first stated the rule against exactly this, in the same workflow file
   that stated it, and in the one place `gate-parity` cannot see, because
   `verify` is not a dependency of `build`.

The five are one defect with five instances: a gate command written down twice
drifts. Closing the instances without closing the mechanism guarantees a sixth —
and the fifth instance is what that sentence looks like when it is not a
prediction.

## Decision

1. **The Rust toolchain is pinned to an exact version in
   `rust-toolchain.toml`.** `channel = "1.95.0"`,
   `components = ["rustfmt", "clippy"]`, `targets = ["wasm32-unknown-unknown"]`,
   `profile = "minimal"`. An exact version, not a channel alias: `stable` is a
   moving target and pinning it would leave the gap this decision exists to
   close. The workspace was verified clean under 1.95.0 — build, test,
   `cargo fmt --all --check`, clippy with `-D warnings`, and the wasm32 check —
   before the pin was written.

2. **The pin lives in that file and in no other.** CI does not name a version;
   it runs `rustup show`, which installs the channel, the components, and the
   targets that file names. `dtolnay/rust-toolchain@stable` was removed rather
   than reconfigured. The action installs a toolchain and runs `rustup default`
   on it, and a `rust-toolchain.toml` directory override outranks the rustup
   default — so with the file present the action would have installed a `stable`
   toolchain on every run that no cargo invocation then used. That is a second
   declaration of the toolchain, contradicting the first and having no effect:
   the shape of defect this ADR exists to remove. The cost of removing it is
   that CI now relies on the runner image providing rustup, where the action
   installed it when absent; a runner without rustup fails the job loudly rather
   than running an unpinned toolchain.

3. **A version bump is a deliberate PR by the maintainer or a delegated agent,
   never a side effect.** Edit `channel`, run `just build`, repair whatever the
   new rustfmt and clippy report, and land the repairs in the same commit as the
   bump. Since CI installs from the file, the bump moves both sides at once, and
   a bump whose repairs are not in the same commit fails its own PR. There is no
   scheduled bump and no automation that raises one: the pin moves when someone
   decides it should.

4. **No `rustfmt.toml`.** The repository uses rustfmt's defaults, and with the
   toolchain pinned those defaults are fixed: the style edition follows the
   workspace's `edition = "2024"`. A configuration file holding no non-default
   setting would be a file to maintain that changes nothing. Adding one is how
   this repository would express a formatting choice it has not made.

5. **Local and CI run the same commands, not equivalent ones.** The justfile is
   the single definition of every gate command. CI installs the tools a runner
   needs and then invokes the recipes — `just check`, `just compile`,
   `just test`, and the rest — rather than restating their command lines. Two
   copies that agree today are one edit away from disagreeing, and the five gaps
   above are that edit having happened five times. What remains in the workflow
   is tool installation and job plumbing; no gate command is written there.

   When CI needs a variant of a check, the recipe takes a parameter and CI
   passes it. That is how the fifth instance was closed: the `convco` job held
   its own `git std lint --range origin/$base_ref..HEAD` and `verify` held
   `origin/main..HEAD`, so the two had **already** drifted and agreed only for
   PRs based on `main`. Both now invoke `just lint-commits`, whose base is a
   parameter defaulting to `main`. This one was found in review of the commit
   that stated the rule, in the same file that stated it — which is the argument
   for the rule, not against it.

6. **`just gate-parity` guards the half that single-sourcing does not reach.**
   Single-sourcing the command text cannot stop a member from being dropped from
   CI, or added to `build` and never wired into CI — which is how `mdbook build`
   came to run in CI alone, and how `wasm-check` came to be a recipe nothing
   depended on (#182). The recipe reads `build`'s dependency list from the
   justfile and fails when any member is not invoked by a `run:` step in
   `.github/workflows/ci.yml`.

   What it proves is narrow: that the text of each step is present. It does not
   prove the step is reached — dropping a job from the `ci` aggregate's
   `needs:`, narrowing `on:`, or adding an `if:` that never holds all leave it
   green while the gates diverge — and it says nothing about `verify` or
   `lint-commits`, which are not dependencies of `build`. That last blind spot
   is where the `lint-commits` drift in decision 5 lived unnoticed.

7. **`--locked` is added where CI had it and nowhere else.** `just compile` and
   `just test` carry it; `fmt-check`, `lint`, and `wasm-check` do not, because
   CI never passed it to those. Parity is the rule in both directions — adding
   the flag where CI does not have it would be a new divergence, not a fix.

8. **`mdbook build` becomes a gate member, `just book-check`, and mdBook becomes
   a hard local requirement.** The cost was weighed and accepted: the build is a
   small fraction of a second on this book, and the alternative — leaving the
   check in CI alone — is what made an unparseable `SUMMARY.md` unreportable
   before pushing. The recipe builds a **copy** of the book, because
   `mdbook build` writes into its own source: a `SUMMARY.md` naming a chapter
   file that does not exist makes mdBook create that file and exit 0. A check
   that mutates the tree it is checking is not a check.

   `mdbook` and `just` are pinned **on the CI side only** — `MDBOOK_VERSION` and
   `JUST_VERSION` in the workflow's `env:`, so both pins are visible in one
   place rather than buried in a curl URL. Nothing applies either pin to a
   contributor, who runs whatever their package manager gave them; the version
   CI installs and the version a contributor runs can differ, and do (CI 0.4.40,
   this branch verified on 0.4.52). That asymmetry is real and is not closed
   here. Decision 1's argument does not carry over to it: rustup applies
   `rust-toolchain.toml` to both sides at no cost and selects per directory,
   where mdBook and `just` have no such mechanism, so a contributor-side pin
   would mean forcing a global downgrade on someone who already has a newer
   build. What is claimed for `mdbook build` is only that its **verdict** is
   stable across patch versions in the narrow class it actually checks (decision
   13); that is an argument for tolerating the skew, not for calling it absent.

9. **`.markdownlintignore` is the exclusion list; the `--ignore` flags are
   removed from CI.** CI was the wrong side. The file is committed, both
   invocations auto-detect it, and its header already documents why the flag
   form is unreliable. With CI invoking `just check`, the flags have no place
   left to live.

10. **A tool the gate shells out to is guarded, and a missing tool fails the
    gate — it never skips a member.** `book-check` checks for `mdbook`,
    `toolchain-check` checks for `rustc`, and `wasm-check` already checked for
    `rustup` (PR #184). Each exits 1 with a message naming the tool and the
    command that installs it, rather than the bare exit 127 a missing binary
    produces. A gate that quietly weakens itself when a tool is absent is the
    defect this ADR is about, so no guard downgrades to a warning or a skip.

11. **`just toolchain-check` compares versions.** It reads the `channel` from
    `rust-toolchain.toml` and fails when `rustc --version` reports a different
    one. It does not detect an override as such, and should not be described as
    doing so: `RUSTUP_TOOLCHAIN=stable` passes today because `stable` is 1.95.0,
    which is the right answer — what the gate measures against is the compiler
    version, not the alias that selected it. What the comparison does catch is
    every way the version can end up wrong: no rustup, in which case the file is
    read by nobody and ignored without a word; a `RUSTUP_TOOLCHAIN` or a
    `rustup override` naming a different release; or a pin nobody installed. It
    also rejects a `channel` that is not an exact version, with that as the
    stated reason — a moving alias is the gap the pin exists to close, and the
    recipe previously blamed `RUSTUP_TOOLCHAIN` for it.

12. **`./bootstrap` installs the pinned toolchain and names every other tool the
    gate requires.** It keeps installing git-std and prim, and now runs
    `rustup show` when rustup is present, which installs the pin. It checks for
    `just`, `rustup`, `mdbook`, and `markdownlint`, reports each missing one
    with the command that installs it, and exits non-zero. It does not install
    those four: they come from three package ecosystems, and writing into a
    contributor's global npm, cargo, or system prefix is a choice this
    repository should not make for them. Naming them up front is the
    reconciliation — silence was the previous behaviour, and silence is what let
    mdBook become a gate requirement nothing told a contributor about.

13. **What `just book-check` catches is a `SUMMARY.md` parse check, not "the
    book compiles".** Measured against mdBook 0.4.52 on this book:
    `mdbook build` exits non-zero on a `SUMMARY.md` mdBook cannot parse (a
    suffix chapter followed by a list) and on a missing `SUMMARY.md`. It exits
    **0** on a chapter file that does not exist (creating it), on a broken
    `{{#include}}` (an ERROR line, then exit 0), on a `SUMMARY.md` holding no
    list items, on one that is not a summary at all, and on bad nesting. The
    recipe comment, `README.md`, `CONTRIBUTING.md`, and `AGENTS.md` say that
    rather than "the docs book compiles", which they said first and which is
    false. A recipe is described by what it fails on, not by what its name
    suggests.

## Consequences

- Positive: one toolchain, named in one file, installed the same way on both
  sides. One definition per gate command. Gaps 1 to 4 each gained a local check
  that fails on them, and every one of those checks was run against a
  constructed failure before it was accepted. Gap 5 gained no check — nothing
  watches `verify` — and is closed by single-sourcing alone.
- Positive: `just build` gained three members, all cheap. The four members that
  need no compilation run first, so a wrong toolchain, an unwired CI job, a
  formatting regression, or an unparseable `SUMMARY.md` all report before a
  compile starts. No wall-clock figure is quoted here: the first draft quoted
  two, and neither reproduced on a second machine.
- Negative / accepted: mdBook and `just` are now hard requirements for running
  the gate at all, and `./bootstrap` fails when they are absent. That is the
  intended trade — a named missing tool over a silently skipped check.
- Negative / accepted: mdBook and `just` are pinned on the CI side and unpinned
  on the contributor side (decision 8). The two sides can run different releases
  of both, and did while this ADR was being written.
- Negative / accepted: `gate-parity` watches only the members of `build`.
  `verify` and `lint-commits` are outside it, and that is where the fifth
  instance hid. Single-sourcing is what protects those two; no check does.
- Negative / accepted: CI installs `just` in five jobs, which is five copies of
  one download step. A composite action would remove the repetition and add a
  file that cannot be exercised while CI is stuck.
- Negative / accepted: pinning to an exact version means the repository no
  longer discovers a new stable release by failing. It discovers it when someone
  bumps the pin, and the bump PR carries whatever repairs the new release wants.
- Negative / accepted: markdownlint-cli is still unpinned on both sides — CI
  installs the latest, a contributor has whatever their package manager gave
  them. Its findings are style findings that `just fmt` repairs, so the exposure
  is a failing `just check` rather than a wrong artifact, and pinning it would
  need a version in a file that no ecosystem here reads. Recorded as known and
  not closed.
- Review hook: every decision above is reversible by editing one file. The
  maintainer can veto any of them by reopening this ADR.

## References

- ADR-0006 — E0 execution decisions; decision 8 made the gate local while CI is
  stuck.
- ADR-0007 — E1 execution decisions; decision 16 carried that gate forward.
- ADR-0008 — E2 execution decisions; decision 11 enumerates the gate's members,
  which this ADR does not change.
- Issue #182 and PR #184 — the two members that were not reachable from
  `just build`, and the review that found the first four of these gaps.
- `justfile`, `.github/workflows/ci.yml`, `rust-toolchain.toml`, `bootstrap` —
  the files this ADR governs.
