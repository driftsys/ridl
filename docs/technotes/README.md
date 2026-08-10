# Technotes

Explanatory, informative notes. Nothing here is normative and nothing depends on
it — for decisions that bind downstream work, see
[`../decisions/`](../decisions/).

- **walking-skeleton-architecture.md** — the RIDL toolchain as built by epics E1
  and E2: the workspace map, the end-to-end pipeline contract, and the LSP
  overlay design the next profile's implementer needs, as they actually landed
  in the merged code. (The filename keeps its E0 origin; the E0 and E1-only
  versions are in git history.)
- **sans-io-and-readiness.md** — what "sans-IO" and "readiness state machine"
  mean in ADR-0018 decision 2, and what they mean for a signal in particular: a
  slot rather than a queue, and a transition whose most common trigger is a
  deadline expiring rather than bytes arriving. Describes the design epic E11
  builds, not code that runs.
