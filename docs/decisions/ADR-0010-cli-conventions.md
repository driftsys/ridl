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

## Decision

1. **The exit-code taxonomy is 0 / 1 / 2, and it is latent in the design already
   — this ADR names it rather than inventing it.**

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

   | Subcommand      | 0                                                        | 1                                                                    | 2                                                                                                          |
   | --------------- | -------------------------------------------------------- | -------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------- |
   | `ridl check`    | a clean package checks with no diagnostic                | a diagnostic error (`TYPL-104`, min exceeds max)                     | the given path does not exist                                                                              |
   | `ridl build`    | a clean package builds                                   | the same diagnostic error                                            | the given path does not exist                                                                              |
   | `ridl baseline` | a clean package publishes                                | a package with a diagnostic error publishes nothing                  | the given path does not exist                                                                              |
   | `ridl test`     | every `require` clause evaluates or is a documented skip | a clause whose evaluation faults (`100 / (d - d)`, division by zero) | the workspace does not compile, or the path does not exist                                                 |
   | `ridl fmt`      | nothing under `--check` would change                     | a file under `--check` would change, or has a parse error            | the path does not exist, or a directory the walk reaches is unreadable (fixed by this PR — see Decision 6) |
   | `ridl diff`     | the two sides are identical or compatible                | the change is breaking                                               | one side fails to compile, or the path does not exist                                                      |
   | `ridlc check`   | a clean package checks with no diagnostic                | the same diagnostic error                                            | the given path does not exist                                                                              |
   | `ridlc build`   | a clean package builds                                   | the same diagnostic error                                            | the given path does not exist                                                                              |

   The claim is scoped to these eight, constructed this way, on this date — not
   asserted as a property that holds by design of every subcommand a future PR
   might add. A ninth subcommand earns a row here when it is added and checked,
   not by inheriting this table.

2. **The clig.dev guidance that applies, quoted rather than paraphrased:**

   - "Return zero exit code on success, non-zero on failure." Met by every row
     of the table above.
   - "Exit codes are how scripts determine whether a program succeeded or
     failed, so you should report this correctly," recommending that
     implementers map "the non-zero exit codes to the most important failure
     modes." Decision 1's two-way split of non-zero (1 = a real negative answer,
     2 = the tool could not answer) is this repository's answer to that
     recommendation.
   - Display help on "-h or --help flags," and the standard version flag is
     "--version," with no short form recommended. `-h`/`--help` already worked
     on both binaries (clap's derived default). `--version` did not, before this
     PR; Decision 6 adds it.
   - "Send output to `stdout`. The primary output for your command should go to
     `stdout`. Anything that is machine readable should also go to `stdout`."
     `ridl diff`'s report (`render_text`/`render_json`) and `ridl test`'s report
     print to stdout; `ridl fmt`'s rewritten files are the primary output and
     are written to disk, which is the only primary output a formatter has
     besides its exit code.
   - "Send messaging to `stderr`. Log messages, errors, and so on should all be
     sent to `stderr`." Every diagnostic renderer in both binaries (`finish`,
     `render`) writes to stderr; every hand-written `error: …` line in this ADR
     and in the fixed code goes through `eprintln!`.
   - "Be consistent across subcommands. Use the same flag names for the same
     things, have similar output formatting, etc." `--frozen` (Decision 6 makes
     its documentation match, not only its behavior), `--out-dir`, and `--emit`
     are spelled and typed identically between `ridl build` and `ridlc build`;
     `path` is the positional argument's name in both binaries' `check` and
     `build`.

3. **`--version` gained the short form `-V` as well, which clig does not
   recommend and does not forbid.** clap's `#[command(version)]` derives both
   flags together; suppressing `-V` would take extra code for no requirement it
   serves, and issue driftsys/ridl#194 explicitly asked that `ridl -V` work.
   Kept, not fought.

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
   Nothing here changes `ridl diff`'s contract or extends plumbing-grade
   stability to a subcommand ADR-0008 did not grant it to. `ridl check`,
   `build`, `baseline`, `test`, and `fmt`, and both `ridlc` subcommands, follow
   the same 0/1/2 shape as a matter of recorded fact (Decision 1's table), not
   as a stability guarantee — a future PR changing what `ridl test`'s exit 1
   covers would not be breaking a contract the way changing `ridl diff`'s would.

6. **A CLI subcommand that is asked to read a path it cannot read fails closed:
   exit 2, with a message naming the path and the cause.** This was already true
   of `ridlc check`, `ridlc build`, `ridl check`, `ridl build`, `ridl baseline`,
   `ridl test`, and `ridl diff` — all seven reach `ridl_core::load_workspace` or
   `ridlc::compile_workspace`, whose missing-path branch has always returned an
   `io::Error` naming the path, rendered as ``error: `<path>` does not exist``.
   `ridl fmt` was the eighth and did not: `collect_source_files`' walk stack
   popped a directory, called `std::fs::read_dir`, and on failure — a missing
   top-level path or an unreadable directory reached mid-walk — silently treated
   the failure as an empty listing rather than reporting it. Fixed in this PR:
   `collect_source_files` now returns
   `Result<Vec<PathBuf>, (PathBuf,
   io::Error)>`, propagating the first
   `read_dir` failure with the directory it failed on; `run_fmt` turns that into
   `error: cannot read <dir>: <err>` and exit 2. One code path fixes both
   symptoms issue driftsys/ridl#194 named, because a missing top-level path and
   an unreadable subdirectory are the same failure — `read_dir` erroring —
   reached by two different callers of the same walk.

   This brings `ridl fmt` into line with a rule this repository already states
   for its other gate-relevant tooling, not a new rule invented for it.
   `CONTRIBUTING.md` states two fail-closed rules already: for the book's
   example harness, "The harness is fail-closed. Each of these is an error
   rather than a silent skip" (naming, among other cases, an unrecognised marker
   and a missing `package` declaration); and for `./bootstrap`'s tool detection,
   "Nothing is skipped when a tool is missing — the recipe that needs it fails
   and says which one," which `CONTRIBUTING.md` itself attributes to ADR-0009
   (decisions 10 and 12, which guard `book-check`, `toolchain-check`, and
   `wasm-check` the same way and state "no guard downgrades to a warning or a
   skip"). `ridl fmt`'s two fail-open paths were a CLI-surface instance of the
   same defect shape those two statements already rule out elsewhere in this
   toolchain.

7. **`--version` and `-V` were added to both `Cli` structs via clap's
   `#[command(version)]`, and `--frozen`'s doc comment in `ridl` now reads word
   for word what `ridlc`'s already does.** `ridl`'s two `Command::Check` and
   `Command::Build` variants each gained the same doc comment carried on
   `ridlc`'s `frozen: bool` field: "Verify remote imports against `ridl.lock`
   without fetching or regenerating it (CI mode, ADR-0002 §7)." Both changes are
   additive: no existing flag, output, or exit code moved.

8. **`--version` prints the workspace version, `0.0.0`, and that string is not
   fixed here.** ADR-0007 decision 14 pins the workspace at `0.0.0` until a
   maintainer runs `just release`; no tag has been cut. `ridl --version` and
   `ridlc --version` therefore both print `<name> 0.0.0` today — confirmed by
   running the built binaries. Read next to the CLI reference (PR
   driftsys/ridl#193), where every transcript is version-sensitive and there is
   no published release to compare against, `0.0.0` answers "is this the version
   the docs were written against" with the same string it would give for every
   future unreleased build, which is not an answer at all. Assigning a version
   string that actually distinguishes builds — a git-describe suffix, a build
   timestamp — is a release-process decision in ADR-0007 decision 14's
   territory, not a CLI-surface one, and is left to whoever next touches that
   decision rather than papered over here.

## Consequences

- Positive: `ridl fmt` now fails closed on a path it cannot read, the same shape
  as the other seven subcommands, closing the gap issue driftsys/ridl#194 found
  and the one this repository could not detect through its own CI (CI has
  produced no run since 2026-07-18).
- Positive: `ridl --version`, `ridl -V`, and `ridlc --version` all work, so a
  reader of the CLI reference (PR driftsys/ridl#193) has a way to ask what they
  are running, even though the answer is not yet informative (see the next
  point).
- Positive: `--frozen` is documented identically under `ridl` and `ridlc`, so
  `ridl check --help` no longer sends a reader to `ridlc`'s `--help` (or the
  book) to learn what the flag it exposes does.
- Positive: the exit-code taxonomy and which clig.dev rules apply are written
  down once, in one place, rather than being re-derived by whoever next adds a
  subcommand or reviews one.
- Negative / accepted: `--version` reports `0.0.0` on every build until a
  maintainer cuts a release. A bug report that includes `ridl --version` output
  cannot be matched to a commit from that string alone. Not closed here — see
  Decision 8.
- Negative / accepted: this ADR documents the 0/1/2 shape of seven subcommands
  as current, verified fact, not as a stability contract. Only `ridl diff`
  carries ADR-0008 decision 9's plumbing-grade guarantee; a future change to,
  say, what `ridl test` exits 1 on would not be reverting a decision recorded
  here, only falsifying Decision 1's table, which a review of this ADR should
  then update.
- Review hook: every decision above is reversible by editing the CLI flags,
  messages, or this file. The maintainer can veto any of them by reopening this
  ADR.

## References

- Issue driftsys/ridl#194 — the three defects, found writing the CLI reference
  (PR driftsys/ridl#193) and confirmed independently by running the binaries.
- ADR-0002 — Module system; §7 is where `--frozen`'s CI-mode meaning comes from.
- ADR-0007 — E1 execution; decision 14 pins the workspace version at `0.0.0`
  until a maintainer cuts a release, which is why Decision 8 above is not closed
  here.
- ADR-0008 — E2 execution; decision 9 is the `ridl diff` stability contract this
  ADR generalises around without reopening (Decision 5 above).
- ADR-0009 — Toolchain pin and gate parity; decisions 10 and 12 are the
  fail-closed standard `CONTRIBUTING.md` restates and `ridl fmt` was the one
  CLI-surface gap in (Decision 6 above).
- `CONTRIBUTING.md` — the two fail-closed statements quoted in Decision 6.
- [clig.dev](https://clig.dev) — the command-line interface guidelines quoted in
  Decisions 2 through 4.
- `crates/ridl/src/main.rs`, `crates/ridlc/src/main.rs` — the CLI surface this
  ADR governs.
