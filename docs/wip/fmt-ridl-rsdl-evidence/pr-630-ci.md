# PR #630 timing-test correction

Date: 2026-10-02, Europe/Paris. Base:
`dce6aa60d8e469b33ee3e636318b3cf53163e7b8`, the open PR #630 head after PR #632
merged into its branch. The maintainer requested this CI correction.

## Failure and reproduction

[CI run 36956509025](https://github.com/driftsys/ridl/actions/runs/36956509025)
failed in `a_blocking_client_with_no_timeout_set_waits_for_the_provider`:
`answer == Ok(())` passed, but `waited >= LATE` failed. The helper started the
provider's 100 ms sleep before starting the caller's timer. A scheduling pause
in that interval subtracts from the time the assertion measures.

`cargo test -p ridl-backend-rust --test interaction_face
 a_blocking_client_with_no_timeout_set_waits_for_the_provider --locked`
passed on the base without instrumentation. Adding a temporary 20 ms pause
before the caller timer reproduced the same assertion failure, exit 101. Log:
`pr-630-red-scheduling.log`. The pause was removed after the experiment.

## Correction and verification

The provider now waits for the call's outcome-interest registration before
starting its delay, using the existing `RecordingPorts::outcome_waker` slot.
That registration happens after the caller timer starts. The same helper serves
all three tests for an unbounded call: default timeout, an unrepresentably large
timeout, and clearing a timeout. Their success and minimum-wait assertions stay
in place. No generated code, runtime API or production behavior changes.

The existing registration-wait pattern bounds the provider's polling attempts.
Moving the timer before thread launch would include setup in the measured wait;
synchronizing with the call instead retains the intended waiting assertion.

The identical temporary 20 ms caller pause passed with the corrected helper,
exit 0. Log: `pr-630-green-scheduling.log`. After restoring the source,
`cargo test -p ridl-backend-rust --test interaction_face --locked` passed all 77
tests, exit 0. Log: `pr-630-interaction-face.log`.

Local experiment logs are retained under this ignored evidence directory. The
commit, QUICK review, full gate and updated CI result will be recorded after
verification. Existing PR #630 ledgers already contain its two full review
passes; this correction receives a fresh QUICK review over its own diff.

## QUICK review and corrections

Initial correction commit: `d011d5651e58045c5acc3134e55fc93d730d19e1`.
`just verify` exited 0 on this head, including every member of `just build`.
Log: `pr-630-verify-d011d56.log`.

Three fresh QUICK contexts reviewed only `dce6aa60..d011d56`. Tests, restricted
code-to-prose docs and the general-purpose bugs wrapper ran actual
`gpt-5.6-terra` at high effort; its native built-in bugs worker ran actual
`gpt-5.6-terra` at explicit medium effort. All reported the three exact changed
paths. The wrapper initially reported a missing final output artifact; the
completed native artifact and its captured output were recovered and inspected.
The bugs finding below is from that actual artifact, not from the wrapper's
incorrect empty summary. No refuter, ledger or GitHub review comments were used.

- Tests: a temporary scheduling reproduction does not pin the correction in the
  committed suite. Corrected by adding
  `a_blocking_client_waits_the_full_provider_delay_after_starting_late`. A
  provider-start rendezvous plus a 200 ms caller pause exercises the ordering in
  the regular suite. Restoring the original immediate-delay behavior makes this
  new test fail at its minimum-wait assertion, exit 101; log:
  `pr-630-mutation-immediate-delay.log`.
- Bugs: panicking the provider when registration is absent can leave the
  unbounded caller blocked. Corrected by settling the call and unparking the
  caller before the parent reports the missing registration. A temporary
  mutation suppressing outcome registration left the old guard blocked after its
  child assertion; the owned experiment process group was stopped after eight
  seconds. The same mutation on the correction terminates with the parent
  assertion, exit 101. Logs: `pr-630-red-registration-guard.log` and
  `pr-630-green-registration-guard.log`. The mutation is restored.
- Docs: no code-to-prose finding.

After these corrections,
`cargo test -p ridl-backend-rust --test
interaction_face --locked` exited 0 with
all 78 tests passing; log: `pr-630-reviewed-fix.log`. Neither the caller's
timeout nor the success and minimum-wait assertions were weakened. QUICK
corrections receive no QUICK pass of their own. Full verification and enabled
push hooks run again on the final head. The latest exact head and gate/CI
results are retained in the local `pr-630-final-verification.log` and
`pr-630-ci-result.log` evidence files.
