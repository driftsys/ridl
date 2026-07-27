# ADR-0010: CLI Conventions

## Status

Accepted — 2026-07-27. Scope: the command-line surface of `ridl` and `ridlc` —
exit codes, `--version`, `--help`, and doc-comment parity between the two
binaries' shared flags. Not epic-scoped: it binds the CLI contract for every
future subcommand until superseded, the way ADR-0009 binds the gate. It follows
the ADR-0006 / ADR-0007 / ADR-0008 / ADR-0009 pattern of recording an
agent-taken decision for after-the-fact maintainer review. This PR both records
this ADR and fixes the three defects it rules on, and closes issue
driftsys/ridl#194.

## Context

Issue driftsys/ridl#194, found while writing the book's CLI reference (PR
driftsys/ridl#193) and confirmed independently by running the binaries, named
three defects:

1. **`ridl fmt` fails open, in two places.** `ridl fmt /nonexistent` exited 0 —
   confirmed on this branch before the fix. Every other subcommand exits 2 on
   the same input. The deeper instance: `collect_source_files`
   (`crates/ridl/src/main.rs`) discarded a directory's `read_dir` error with
   `let Ok(entries) = … else { continue }`, so an unreadable directory reached
   mid-walk was silently treated as zero files. Confirmed before the fix: a tree
   holding one file that would be reformatted exits 1 under `ridl fmt --check`;
   `chmod 000` on the directory holding that file made the same command exit 0,
   with no output on either stream. `ridl fmt --check` is how the CI gate would
   have caught a would-reformat file — a permissions change on the tree it walks
   flipped that gate from failing to passing without saying why.
2. **Neither binary accepted `--version`.** `ridl --version`, `ridl -V`, and
   `ridlc --version` all exited 2 (clap's "unexpected argument" usage error) —
   confirmed before the fix.
3. **`--frozen` had no description under `ridl`.** `ridl check --help` and
   `ridl build --help` rendered the flag with a blank line where its description
   belongs; `ridlc check --help` and `ridlc build --help` rendered the same flag
   with "Verify remote imports against `ridl.lock` without fetching or
   regenerating it (CI mode, ADR-0002 §7)." The flag is forwarded unchanged
   (`crates/ridl/src/main.rs` passes `frozen.into()` straight to the same
   `ridlc` functions), so the two binaries were documenting one flag two ways.

The three are one family: places where the CLI surface diverged from a
convention that was otherwise already in force. This ADR writes that convention
down — the exit-code taxonomy, which clig.dev guidance applies and which does
not, and the fail-closed rule this repository already states for its other
gate-relevant tooling — so a future subcommand can be checked against a recorded
rule instead of house custom, and so defect 1 (the one substantive behavior
change) has a stated reason rather than being read as a stray bug fix.

Review of the first draft of this ADR (before it merged) constructed further
`chmod 000` scenarios beyond the ones issue #194 raised, and found that the
fail-closed property this ADR set out to describe — a message naming the actual
cause, not only exit 2 — holds for fewer subcommands than the first draft
claimed. Decision 6 below is scoped to what that review proved; the two message
defects it found are recorded as issue driftsys/ridl#196 rather than fixed here.

## Decision

1. **The exit-code taxonomy is 0 / 1 / 2. Every subcommand fits it today; before
   this PR, `ridl fmt` had two exceptions to it — a missing path, and an
   unreadable directory reached mid-walk, which issue driftsys/ridl#194 itself
   calls "the serious one," because it is what let `ridl fmt --check` in CI flip
   from failing to passing silently — this ADR names the taxonomy, it does not
   invent it.**

   - **0** — succeeded, or the verdict is affirmative.
   - **1** — a real answer that happens to be negative: a breaking change
     (`ridl diff`), a test or evaluation failure (`ridl test`), a file that
     would be reformatted (`ridl fmt --check`), or a diagnostic error over the
     checked source (`check`, `build`, `baseline`).
   - **2** — the tool could not answer: a missing or unreadable path, a bad
     flag, or an I/O failure while reading or writing.

   Verified by direct construction against the built `ridl` and `ridlc` binaries
   on this branch (2026-07-27), one input per cell, across the eight subcommands
   the two binaries expose today:

   | Subcommand      | 0                                                                                                                                           | 1                                                                    | 2                                                                                                          |
   | --------------- | ------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------- |
   | `ridl check`    | a clean package checks with no diagnostic                                                                                                   | a diagnostic error (`TYPL-104`, min exceeds max)                     | the given path does not exist                                                                              |
   | `ridl build`    | a clean package builds                                                                                                                      | the same diagnostic error                                            | the given path does not exist                                                                              |
   | `ridl baseline` | a clean package publishes                                                                                                                   | a package with a diagnostic error publishes nothing                  | the given path does not exist                                                                              |
   | `ridl test`     | every `require` clause evaluates or is a documented skip                                                                                    | a clause whose evaluation faults (`100 / (d - d)`, division by zero) | the workspace does not compile, or the path does not exist                                                 |
   | `ridl fmt`      | nothing under `--check` would change                                                                                                        | a file under `--check` would change, or has a parse error            | the path does not exist, or a directory the walk reaches is unreadable (fixed by this PR — see Decision 6) |
   | `ridl diff`     | the two sides are identical or compatible; `--explain <CATEGORY>` also exits 0, printing the classification rule without comparing anything | the change is breaking                                               | one side fails to compile, or the path does not exist                                                      |
   | `ridlc check`   | a clean package checks with no diagnostic                                                                                                   | the same diagnostic error                                            | the given path does not exist                                                                              |
   | `ridlc build`   | a clean package builds                                                                                                                      | the same diagnostic error                                            | the given path does not exist                                                                              |

   The claim is scoped to these eight, constructed this way, on this date — not
   asserted as a property that holds by design of every subcommand a future PR
   might add. A ninth subcommand earns a row here when it is added and checked,
   not by inheriting this table.

2. **The clig.dev guidance that applies, quoted rather than paraphrased:**

   - "Return zero exit code on success, non-zero on failure. Exit codes are how
     scripts determine whether a program succeeded or failed, so you should
     report this correctly. Map the non-zero exit codes to the most important
     failure modes." Met exactly by the rows where non-zero really does mean the
     run failed: every subcommand's exit-2 column, the diagnostic-error exit-1
     cells of `check`, `build`, `baseline`, `ridlc check`, and `ridlc build`,
     and `ridl fmt`'s parse-error exit 1 (D1's table: "a file under `--check`
     would change, **or has a parse error**") — a file that does not parse is a
     failure in the ordinary sense, not a verdict. It is not met, and is not
     meant to be, by `ridl diff`'s breaking-change exit 1, `ridl fmt --check`'s
     would-reformat exit 1 specifically, or `ridl test`'s evaluation-fault exit
     1 — those three ran to completion and answered a question correctly;
     Decision 4 is where that distinction is made, and this bullet does not
     repeat it. Decision 1's two-way split of non-zero (1 = a real negative
     answer, 2 = the tool could not answer) is this repository's answer to the
     instruction to map non-zero exit codes to failure modes.
   - "Display help when passed `-h` or `--help` flags." Already true on both
     binaries before this PR (clap's derived default). The version guidance is
     two separate entries in clig's flag table: "`--version`: Version." and
     "`-v`: This can often mean either verbose or version. You might want to use
     `-d` for verbose and this for version, or for nothing to avoid confusion."
     `--version` did not work before this PR; Decision 7 adds it. Decision 3
     covers the short-form question clig hedges on rather than settles.
   - "Send output to `stdout`. The primary output for your command should go to
     `stdout`. Anything that is machine readable should also go to `stdout`—this
     is where piping sends things by default." `ridl diff`'s report
     (`render_text`/`render_json`) and `ridl test`'s report print to stdout.
     `ridl fmt`'s output is the rewritten file itself, written to disk rather
     than printed — the flag this bullet does not cover, since `ridl fmt` has no
     report to pipe.
   - "Send messaging to `stderr`. Log messages, errors, and so on should all be
     sent to `stderr`." Every diagnostic renderer in both binaries (`finish`,
     `render`) writes to stderr; every hand-written `error: …` line in this ADR
     and in the fixed code goes through `eprintln!`.
   - "Be consistent across subcommands. Use the same flag names for the same
     things, have similar output formatting, etc." `--frozen` is now spelled and
     documented identically across both binaries (Decision 7). `--out-dir` and
     `--emit` are spelled and typed identically between `ridl build` and
     `ridlc build`, but only `ridlc build --help` renders a description for
     either — `ridl build --help` shows both with no text, the same defect class
     `--frozen` had, three flags wide and not closed by this PR. `path` is the
     positional argument's name in both binaries' `check` and `build`,
     documented under `ridlc` ("The `.typl` file, package directory, or
     workspace root") and undocumented under `ridl`.

3. **`--version` gained the short form `-V`, which is not the flag clig hedges
   on.** clig's own guidance on a version short flag is ambivalent, not silent:
   "`-v`: This can often mean either verbose or version. You might want to use
   `-d` for verbose and this for version, or for nothing to avoid confusion."
   clap's `#[command(version)]` derives the _uppercase_ `-V`, not the lowercase
   `-v` that guidance is about, so the ambiguity it warns of — colliding with a
   verbose flag — never arises here; neither binary has a `-v`. `-V` was kept
   because issue driftsys/ridl#194 explicitly asked that `ridl -V` work, and
   suppressing a clap default would take extra code for nothing it would
   prevent.

4. **clig treats exit status as binary success/failure and says nothing about a
   multi-valued verdict.** It does not sanction the `diff(1)`/`grep(1)` reading,
   where a specific non-zero code is itself part of the answer rather than a
   report that the tool failed. `ridl diff`'s exit 1 (breaking),
   `ridl fmt --check`'s exit 1 (would reformat), and `ridl test`'s exit 1 (a
   clause failed to evaluate) all rest on that external precedent, not on clig —
   clig is silent here, not permissive. A script author who assumes clig's
   binary framing (non-zero always means "went wrong") and does not read the
   tool's own documentation will misread a `ridl diff` exit 1 as a crash; the
   tools' own `--help` and this ADR are what correct that, clig is not.

5. **ADR-0008 decision 9 already fixed `ridl diff`'s 0/1/2 as a stability
   contract — "plumbing-grade stability: stable flags, machine-readable output,
   and the defined exit codes 0 = compatible, 1 = breaking, 2 = error … the same
   contract class as `ridlc --frozen`." That decision stands; this ADR
   generalises the taxonomy it named around it, rather than reopening it.**
   Nothing here changes `ridl diff`'s contract, nor `ridlc --frozen`'s: decision
   9 names `ridlc --frozen` as that contract class's _reference exemplar_ — the
   thing `ridl diff` is compared to — so `ridlc --frozen`'s own behavior (ADR-
   0002 §7's CI mode: no fetch, no lockfile write, `MANI-103` on a missing lock)
   already carries the same plumbing-grade stability decision 9 grants
   `ridl diff`, independent of anything recorded here. What this ADR adds no
   guarantee to is the rest of the taxonomy: `ridl check`, `build`, `baseline`,
   `test`, and `fmt`, and `ridlc check`'s and `ridlc build`'s ordinary
   (non-`--frozen`) 0/1/2 behavior, follow the shape Decision 1's table records
   as current, verified fact, not as a stability contract — a future PR changing
   what `ridl test`'s exit 1 covers would not be breaking a contract the way
   changing `ridl diff`'s, or `ridlc --frozen`'s, would.

6. **Of the eight subcommands, only `ridl fmt` (fixed by this PR) reliably names
   both the actual unreadable path and the reason; `ridl diff` does so only for
   its own top-level argument; the other six report a wrong cause or none —
   recorded as issue driftsys/ridl#196, not fixed here.**

   Verified by constructing two `chmod 000` scenarios against the built binaries
   and running all eight subcommands against each: (a) the workspace root
   itself, which contains `ridl.toml`, made unreadable; (b) a subdirectory
   nested inside an otherwise-readable workspace made unreadable.

   - `ridl fmt` reports the actual unreadable directory and the OS error in both
     scenarios: `error: cannot read <dir>: <err>`. This is the fix this PR makes
     (below).
   - `ridl diff` does the same, but only when the unreadable directory is the
     argument passed to it directly. Its own `ir_json_files`/`snapshot_files`
     reader (`crates/ridl/src/main.rs`) calls `read_dir` on that argument before
     ever consulting the compiler, so scenario (a) reports
     `error: cannot read the snapshot directory <dir>: <err>`. When the
     unreadable directory is instead nested inside a diff argument that is
     itself readable (scenario b), `ridl diff` falls through to
     `ridlc::compile_workspace`, the same as the six subcommands below, and
     inherits their limitation: the message names the top-level argument, not
     the subdirectory that actually failed.
   - `ridl check`, `ridlc check`, `ridl build`, `ridlc build`, and
     `ridl baseline` all reach `ridl_core::load_workspace` or
     `ridlc::compile_workspace`, and their own error handling adds no path. In
     scenario (a) all five report
     `` error: no `ridl.toml` found at or
     above `<path>` `` — exit 2 is
     right, the cause is wrong: `find_manifest_root` cannot distinguish "no
     manifest here" from "cannot read this directory to look for one," and
     reports the former unconditionally. In scenario (b) all five report a bare
     `error: Permission denied (os error 13)`, naming no path at all.
   - `ridl test` reaches `ridlc::compile_workspace` the same way and wraps
     whatever error it gets with the top-level path it was given
     (`error: <path>: <err>`). In scenario (a) that only prepends its own path
     in front of the same wrong "no `ridl.toml` found" cause the five above
     report; in scenario (b) it names _a_ path — the workspace root — but not
     the subdirectory that actually failed, the same partial defect
     `ridl diff`'s compile-fallthrough has.

   So the fail-closed property Decision 1's table records — exit 2 on a path
   that cannot be read — holds across all eight. The stronger property this
   decision set out to describe, a message naming the actual cause, holds fully
   only for `ridl fmt`, holds for `ridl diff` only on its own top-level
   argument, is wrong for the other six in scenario (a), and in scenario (b) is
   either absent (`ridl check`, `ridlc check`, `ridl build`, `ridlc build`,
   `ridl baseline`) or present but pointing at the wrong path (`ridl test`,
   which names the workspace root rather than the subdirectory that actually
   failed). Those two message defects are real product defects the review of
   this ADR's first draft surfaced, not documentation scope, and are not closed
   by this PR: tracked as issue driftsys/ridl#196.

   `ridl fmt` itself is fixed here. `collect_source_files`'s walk stack popped a
   directory, called `std::fs::read_dir`, and on failure — a missing top-level
   path or an unreadable directory reached mid-walk — silently treated the
   failure as an empty listing rather than reporting it (issue
   driftsys/ridl#194). It now returns
   `Result<Vec<PathBuf>, (PathBuf,
   io::Error)>`, propagating the first
   `read_dir` failure with the directory it failed on; `run_fmt` turns that into
   `error: cannot read <dir>: <err>` and exit 2. One code path fixes both
   symptoms issue #194 named, because a missing top-level path and an unreadable
   subdirectory are the same failure — `read_dir` erroring — reached by two
   different callers of the same walk. Regression tests:
   `crates/ridl/tests/facade.rs`'s `fmt_on_a_missing_path_exits_two` and
   `fmt_on_an_unreadable_subdirectory_exits_two`, both confirmed to fail against
   `main`'s pre-fix behavior before this PR added them.

   This brings `ridl fmt` into line with a rule this repository already states
   for its other gate-relevant tooling, not a new rule invented for it.
   `CONTRIBUTING.md` states two fail-closed rules already: for the book's
   example harness, "The harness is fail-closed. Each of these is an error
   rather than a silent skip" (naming, among other cases, an unrecognised marker
   and a missing `package` declaration); and for `./bootstrap`'s tool detection,
   "Nothing is skipped when a tool is missing — the recipe that needs it fails
   and says which one (ADR-0009)." That parenthetical is `CONTRIBUTING.md`'s own
   citation, pointing at decisions 10 and 12, which guard `book-check`,
   `toolchain-check`, and `wasm-check` the same way and state "no guard
   downgrades to a warning or a skip." `ridl fmt`'s two fail-open paths were a
   CLI-surface instance of the same defect shape those two statements already
   rule out elsewhere in this toolchain.

   Pre-existing and out of scope: `collect_source_files`'s walk decides whether
   to descend into a child with a bare `child.is_dir()`, which follows a
   symlinked directory rather than refusing it the way
   `ridl_core::load_workspace`'s own walk does (guarded there by its
   `a_symlinked_directory_is_not_followed` test). A symlink cycle under
   `ridl
   fmt` therefore collects the same file repeatedly and stops only when
   a path hits the OS's length limit, silently — identically on `main`, before
   and after this PR. Not introduced and not fixed here.

7. **`--version` and `-V` were added to both `Cli` structs via clap's
   `#[command(version)]`, and `--frozen`'s doc comment in `ridl` now reads word
   for word what `ridlc`'s already does.** `ridl`'s two `Command::Check` and
   `Command::Build` variants each gained the same doc comment carried on
   `ridlc`'s `frozen: bool` field: "Verify remote imports against `ridl.lock`
   without fetching or regenerating it (CI mode, ADR-0002 §7)." Both changes are
   additive: no existing flag, output, or exit code moved. `--out-dir`,
   `--emit`, and `path` under `ridl` remain undocumented in the same way
   `--frozen` was (Decision 2's consistency bullet) — this PR closes one defect
   in that family of four, not all four.

8. **`--version` prints the workspace version, `0.0.0`, on every build until a
   maintainer cuts a release — flagged, not fixed here.** ADR-0007 decision 14
   pins the workspace at `0.0.0`; no tag has been cut. `ridl --version` and
   `ridlc --version` therefore both print `<name> 0.0.0` today — confirmed by
   running the built binaries. Read next to the CLI reference (PR
   driftsys/ridl#193), where every transcript is version-sensitive and there is
   no published release to compare against, `0.0.0` answers "is this the version
   the docs were written against" with the same string every future unreleased
   build would give, which is not an answer at all.

   A build-metadata suffix would fix this without touching ADR-0007 decision
   14's package-version pin at all — semver reserves `+` for exactly this, and
   clap's `version` attribute accepts a `concat!` of two `env!`s:

   ```rust
   #[command(version = concat!(env!("CARGO_PKG_VERSION"), "+", env!("RIDL_BUILD")))]
   ```

   giving `ridl 0.0.0+g310ecf7` today, with `RIDL_BUILD` set by a small
   `build.rs` (a git-describe short hash, say) added to each binary crate. The
   snippet as shown always appends a `+` suffix, on a tagged release included —
   dropping it there cleanly would need the `build.rs` to leave `RIDL_BUILD`
   empty on a tagged commit and the version expression to omit the `+` when it
   is, a real detail left for whoever implements this rather than solved by the
   snippet above. That is a build-system change to two crates, and deserves its
   own PR and its own run of the full gate — `just wasm-check` included — rather
   than riding in on this one. Recorded here as the concrete deferred option,
   not implemented.

## Consequences

- Positive: `ridl fmt` now fails closed on a path it cannot read — exit 2 in
  both a missing top-level path and an unreadable directory reached mid-walk,
  each naming the actual cause. That is stronger than six of the other seven
  subcommands, and at least as strong as the seventh (`ridl diff`, whose own
  reader matches it only for its own top-level argument, not a nested failure —
  Decision 6). Closes the gap issue driftsys/ridl#194 found, on a repository
  whose own CI cannot currently confirm the fix: `ci.yml` triggers on every push
  to `main` and on every pull request, and does produce a run there — confirmed
  on this branch (2026-07-27T07:16:17Z) and on `main` the day before — but every
  job fails to start, before running a single check, because of a billing issue
  on the GitHub account, not because CI has stopped producing runs.
- Positive: `ridl --version`, `ridl -V`, and `ridlc --version` all work, so a
  reader of the CLI reference (PR driftsys/ridl#193) has a way to ask what they
  are running, even though the answer is not yet informative (see the next
  point).
- Positive: `--frozen` is documented identically under `ridl` and `ridlc`, so
  `ridl check --help` no longer sends a reader to `ridlc`'s `--help` (or the
  book) to learn what the flag it exposes does. Three more flags in the same
  family (`--out-dir`, `--emit`, `path`, all under `ridl`) remain undocumented
  and are not closed by this PR (Decision 7).
- Positive: the exit-code taxonomy and which clig.dev rules apply are written
  down once, in one place, rather than being re-derived by whoever next adds a
  subcommand or reviews one.
- Negative / accepted: `--version` reports `0.0.0` on every build until a
  maintainer cuts a release. A bug report that includes `ridl --version` output
  cannot be matched to a commit from that string alone. Not closed here — see
  Decision 8, which also records the concrete build-metadata-suffix fix as
  deferred rather than implemented.
- Negative / accepted: this ADR documents the 0/1/2 shape of `ridl check`,
  `build`, `baseline`, `test`, and `fmt`, and `ridlc`'s ordinary
  (non-`--frozen`) check/build behavior, as current, verified fact, not as a
  stability contract. `ridl diff` and `ridlc --frozen` are the two ADR-0008
  decision 9 already grants plumbing-grade stability to (Decision 5); a future
  change to, say, what `ridl test` exits 1 on would not be reverting a decision
  recorded here, only falsifying Decision 1's table, which a review of this ADR
  should then update.
- Negative / accepted: naming the actual unreadable path and cause, rather than
  only exiting 2, holds fully for one subcommand and partially for a second
  (Decision 6). The other six subcommands report a wrong cause or none, tracked
  as issue driftsys/ridl#196 and not fixed by this PR.
- Review hook: every decision above is reversible by editing the CLI flags,
  messages, or this file. The maintainer can veto any of them by reopening this
  ADR.

## References

- Issue driftsys/ridl#194 — the three defects, found writing the CLI reference
  (PR driftsys/ridl#193) and confirmed independently by running the binaries.
- Issue driftsys/ridl#196 — the two message-cause defects Decision 6 records and
  defers: a wrong cause when a workspace root is unreadable, no cause named when
  a nested subdirectory is.
- ADR-0002 — Module system; §7 is where `--frozen`'s CI-mode meaning comes from,
  and where `ridlc --frozen`'s own stability (Decision 5) is grounded.
- ADR-0007 — E1 execution; decision 14 pins the workspace version at `0.0.0`
  until a maintainer cuts a release, which is why Decision 8 above is not closed
  here.
- ADR-0008 — E2 execution; decision 9 is the `ridl diff` / `ridlc --frozen`
  stability contract this ADR generalises around without reopening (Decision 5
  above).
- ADR-0009 — Toolchain pin and gate parity; decisions 10 and 12 are the
  fail-closed standard `CONTRIBUTING.md` restates and `ridl fmt` was the one
  CLI-surface gap in (Decision 6 above).
- `CONTRIBUTING.md` — the two fail-closed statements quoted in Decision 6.
- [clig.dev](https://clig.dev) — the command-line interface guidelines quoted in
  Decisions 2 through 4.
- `crates/ridl/src/main.rs`, `crates/ridlc/src/main.rs` — the CLI surface this
  ADR governs.
