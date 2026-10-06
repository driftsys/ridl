# Design records

Architecture — interfaces and components — plus detailed design, for a subsystem
whose shape does not fit a single decision record. A design record describes a
crate or component as built; the code and its tests are the source of truth, and
where the record and the code differ, the record is corrected. The binding
choices are in the decision records a design record cites. For requirements, see
[`../specification/`](../specification/); for the decisions behind a design's
choices, see [`../decisions/`](../decisions/).

- **ridl-rt.md** — the `ridl-rt` 0.1 runtime library as built: its seven
  modules, the identity types, the interaction descriptors, the payload proof
  type, the ports, the correlation table, and the error module. The decisions
  behind its choices are
  [ADR-0021](../decisions/ADR-0021-ridl-rt-0.1-api-and-release.md).
- **ridl-loopback.md** — the in-process reference runtime as built: the crate
  and its one dependency, the one store behind one lock, the six role handles
  and the aggregate, the two signal extensions, the hand-driven clock, the per
  handle sequence counters, and what it cannot report until it has a catalog
  descriptor. The decisions behind its choices are
  [ADR-0020](../decisions/ADR-0020-third-encoding-runtime-layering-and-plugin-system.md)
  decision 6 and
  [ADR-0021](../decisions/ADR-0021-ridl-rt-0.1-api-and-release.md) decisions 5,
  11 and 12, with
  [ADR-0023](../decisions/ADR-0023-interaction-face-generation.md) decision 5
  for the face its handles are passed to.
- **flatbuffers-codec.md** — the generated `Payload<FlatBuffers>` implementation
  a package carries: where the projection facts both emitters read live, what
  `generate` emits per table, what `encode`, `verify`, `decode` and `MAX_SIZE`
  do and do not promise, determinism, and conformance against an independent
  implementation. The decisions behind its choices are
  [ADR-0019](../decisions/ADR-0019-flatbuffers-projection-rules.md) and
  [ADR-0020](../decisions/ADR-0020-third-encoding-runtime-layering-and-plugin-system.md)
  decisions 2, 5, 6 and 7.
- **catalog-descriptor.md** — the FlatBuffers catalog descriptor per package as
  built: the `RDLC` file `--emit catalog` writes, the append-only schema and its
  generated planus accessors, what a catalog contains, the catalog hash and the
  three artifacts that carry it, the size state per payload and encoding,
  verification before access, `ridl describe`, the port's catalog check, and the
  design's decisions D-1 to D-10 with what is not built (the system descriptor
  among them). The decisions behind its choices are
  [ADR-0014](../decisions/ADR-0014-ir-encodings.md) decision 15,
  [ADR-0020](../decisions/ADR-0020-third-encoding-runtime-layering-and-plugin-system.md)
  decision 5 as amended 2026-10-03, and
  [ADR-0023](../decisions/ADR-0023-interaction-face-generation.md) decision 8.
- **codegen-plugins.md** — the backend contract
  `generate(CodegenRequest) → CodegenResponse`, its in-process host over every
  in-tree backend, the process host behind `--plugin` (`ridlc-gen-<language>` on
  `PATH` or by path, the pipe, the timeout, the errors), the reference plugin
  `ridlc-gen-model`, and the parity test with what it does and does not prove —
  as built. The decisions behind its choices are
  [ADR-0020](../decisions/ADR-0020-third-encoding-runtime-layering-and-plugin-system.md)
  decisions 8 to 12, with decision 11 as amended 2026-09-22.
- **interaction-face.md** — the generated interaction face over `ridl-rt`: per
  interface, an async `Client` whose commands and queries return named futures,
  a `Publisher`, a `Provider` trait, and `serve`; and, under the emitted crate's
  `std` feature, a `blocking` module with the same client, and `serve` where
  there is one, as blocking calls with a timeout. The poll face underneath
  (`send_*`, `poll_*_ack`, `poll_*_reply`, `dispatch`) is `pub(crate)`. The
  record covers the two emitter entry points, the descriptors, the clause
  translator, the settlement table, and every remaining placeholder with the
  story that replaces it. The decisions behind its choices are
  [ADR-0023](../decisions/ADR-0023-interaction-face-generation.md), decision 6
  for the call surface.
- **mcp-workspace-tools.md** — the nine read-only tools of `ridl mcp` as built:
  the common rules (`path`, overlays, names, locations, the workspace status),
  what each tool reads, package coupling and interface cohesion metrics, the
  rsdl component uses, where the code lives (the overlay-aware loader in
  `ridl-core`, `compile_workspace_with` and `load_diff_side` in `ridlc`,
  `snapshot` and the lookups in `ridl-mcp`), the tool errors and notes, how
  path-mode `ridl_check` differs from `ridl check`, and the tests. The inputs
  and outputs of each tool are in the crate README. The decisions behind its
  choices are [ADR-0025](../decisions/ADR-0025-workspace-aware-mcp-tools.md) and
  [ADR-0005](../decisions/ADR-0005-agent-enablement.md).
- **design-lints.md** — the workspace design lints as built: the shared pass in
  `ridlc` that the compile and the language server both run, the site index, the
  three shipped checks and their threshold constants, the package dependency
  graph in `ridlc::deps` that `ridl_metrics` also reads, the `evals/` corpus and
  calibration records, `cargo xtask calibrate`, and the tests. The decisions
  behind its choices are
  [ADR-0027](../decisions/ADR-0027-design-lints-calibrated-on-a-corpus.md).
