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
