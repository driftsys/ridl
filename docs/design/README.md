# Design records

Architecture — interfaces and components — plus detailed design, for a subsystem
whose shape does not fit a single decision record. A design record describes a
crate or component as built; the code and its tests are the source of truth, and
where the record and the code differ, the record is corrected. The binding
choices are in the decision records a design record cites. For requirements, see
[`../specification/`](../specification/); for the decisions behind a design's
choices, see [`../decisions/`](../decisions/).

- **ridl-rt.md** — the `ridl-rt` 0.1 runtime library as built: its six modules,
  the identity types, the interaction descriptors, the payload proof type, the
  ports, and the error module. The decisions behind its choices are
  [ADR-0021](../decisions/ADR-0021-ridl-rt-0.1-api-and-release.md).
- **interaction-face.md** — the generated `Client`/`Publisher`/`Provider`/
  `dispatch` face over `ridl-rt`, story E11.13's in-process MVP: the two emitter
  entry points, the descriptors, the clause translator, the settlement table,
  and every placeholder the story carries with the story that replaces it. The
  decisions behind its choices are
  [ADR-0023](../decisions/ADR-0023-interaction-face-generation.md).
