# Design records

Architecture — interfaces and components — plus detailed design, for a subsystem
whose shape does not fit a single decision record. Normative: a design record
binds the crate or component it describes, the way a decision record binds the
choice it fixes. For requirements, see [`../specification/`](../specification/);
for the decisions behind a design's choices, see
[`../decisions/`](../decisions/).

- **ridl-rt.md** — the `ridl-rt` 0.1 runtime library as built: its six modules,
  the identity types, the interaction descriptors, the payload proof type, the
  ports, and the error module. The decisions behind its choices are
  [ADR-0021](../decisions/ADR-0021-ridl-rt-0.1-api-and-release.md).
