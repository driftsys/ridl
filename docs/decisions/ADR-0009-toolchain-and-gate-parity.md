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
four places. Every one of the four let a tree that is clean locally fail CI, or
the reverse.

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

The four are one defect with four instances: a gate command written down twice
drifts. Closing the four instances without closing the mechanism guarantees a
fifth.

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
   copies that agree today are one edit away from disagreeing, and the four gaps
   above are that edit having happened four times. The copies are gone, so there
   is nothing left to keep in sync. CI installs a pinned `just` (1.38.0, the
   release the recipes were verified against locally) for the same reason the
   Rust toolchain is pinned: an unpinned runner tool is a second toolchain
   nobody chose.

6. **`just gate-parity` guards the half that single-sourcing does not reach.**
   Single-sourcing the command text cannot stop a member from being dropped from
   CI, or added to `build` and never wired into CI — which is how `mdbook build`
   came to run in CI alone, and how `wasm-check` came to be a recipe nothing
   depended on (#182). The recipe reads `build`'s dependency list from the
   justfile and fails when any member is not invoked as `just <recipe>` in
   `.github/workflows/ci.yml`. It proves CI runs no less than the local gate; it
   does not prove CI runs nothing else.

7. **`--locked` is added where CI had it and nowhere else.** `just compile` and
   `just test` carry it; `fmt-check`, `lint`, and `wasm-check` do not, because
   CI never passed it to those. Parity is the rule in both directions — adding
   the flag where CI does not have it would be a new divergence, not a fix.

8. **`mdbook build` becomes a gate member, `just book-check`, and mdBook becomes
   a hard local requirement.** The cost was weighed and accepted: the build
   takes about 30 ms on this book, and the alternative — leaving the check in CI
   alone — is what made an unparseable `SUMMARY.md` unreportable before pushing.
   mdBook is not pinned, unlike the Rust toolchain: `mdbook build` is a
   pass-or-fail check whose verdict does not vary with patch version, where
   rustfmt and clippy produce version-shaped output; and rustup selects a
   toolchain per directory while mdBook has no equivalent, so pinning it would
   force a global downgrade on a contributor who already has a newer one.

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

11. **`just toolchain-check` verifies the pin is actually in force.** Without
    rustup, `rust-toolchain.toml` is read by nobody and ignored without a word,
    and the gate then measures a toolchain nobody chose. A `RUSTUP_TOOLCHAIN`
    variable or a `rustup override set` in the working directory does the same
    with rustup present. The recipe compares `rustc --version` against the
    `channel` in the file and fails on any mismatch, which covers all three.

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

## Consequences

- Positive: one toolchain, named in one file, installed the same way on both
  sides. One definition per gate command. Every gap in the list above now has a
  local check that fails on it, and each of those checks was run against a
  constructed failure before it was accepted.
- Positive: `just build` gained three members and about a second. The four
  members that need no compilation run first, so a wrong toolchain, an unwired
  CI job, a formatting regression, or an unparseable `SUMMARY.md` all report
  before a compile starts. Measured: 0.94 s for the four together.
- Negative / accepted: mdBook and `just` are now hard requirements for running
  the gate at all, and `./bootstrap` fails when they are absent. That is the
  intended trade — a named missing tool over a silently skipped check.
- Negative / accepted: CI installs `just` in four jobs, which is four copies of
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
  `just build`, and the review that found these four gaps.
- `justfile`, `.github/workflows/ci.yml`, `rust-toolchain.toml`, `bootstrap` —
  the files this ADR governs.
