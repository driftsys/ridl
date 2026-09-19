# ADR-0023 — The generated interaction face: entry point, clause translator, and call signatures

## Status

Accepted — 2026-09-19. Scope: four decisions taken while implementing story
E11.13, the MVP of the generated `Client`/`Publisher`/`Provider`/`dispatch` face
ADR-0018 decision 15 restores as the runtime layer's "phase 2". Two settle a
mechanism the approved design left unnamed; two supersede or close a gap in that
design's example signatures. All four bind every later story that extends the
Rust backend's interaction face, until superseded: E5.1 (the clause translator),
Epic 10 (the entry-point split), and any later language backend that follows
this precedent (ADR-0020 decision 7).

Written from lane M's stage M3, on delegated authority — the approved design,
now
[`2026-09-16-interaction-face-v0-design.md`](../archive/2026-09-16-interaction-face-v0-design.md),
did not name these mechanisms, and implementation could not proceed without
them. Sebastien delegated the decisions for the session; each is recorded with
its reasoning so it can be read and overturned. The as-built architecture these
decisions produced is
[the interaction-face design record](../design/interaction-face.md); this record
is the reasoning behind the choices that had more than one defensible answer.

## Context

The approved M1 design (archived at
[`2026-09-16-interaction-face-v0-design.md`](../archive/2026-09-16-interaction-face-v0-design.md))
fixed the face's shape — a `Client` generic over exactly the ports an interface
needs, a `Publisher`, a `Provider` trait, a total `dispatch` — but left three
things unresolved once implementation started: how a `require`/`ensure` clause's
source text becomes a Rust expression (the IR carries only text,
`Contract.source`, not an expression tree — E5.1 adds the tree), where the
generated face's code is emitted from given that two of the compiler's own tests
bind what the pipeline entry point `generate` may produce, and whether a
`Provider` method's argument is taken by value or by reference (the design's own
example signatures used by-value parameters, which turned out not to compile). A
fourth gap — what error type a consumer-side `Client` call returns — was simply
never stated.

## Decision

1. **The Rust backend gets a narrow, total contract-clause translator; it
   refuses every clause form it does not accept, and never drops one.** The
   accepted form is `<subject> <comparison> <numeric literal>`, where
   `<subject>` is the interaction's single declared parameter (or `result` on a
   query's `ensure`) and `<comparison>` is one of `< <= > >= == !=`, over an
   integer- or float-backed named scalar. It emits `args.0 <op> <literal>` or
   `reply.0 <op> <literal>`, conjoining several clauses of one kind with `&&`. A
   clause outside this form is refused with a `GenerateError`, because dropping
   it would generate a provider that accepts arguments its own contract forbids
   — a worse failure than refusing to generate at all. `generate_face` is
   already total over errors, so the refusal costs no new mechanism. This is
   deliberately the smallest thing that makes the design's two clause-driven
   settlement rows (`PreconditionFailed`, `ContractBroken`) real; it is not a
   first instalment of E5, and E5.1 replaces it with a translator driven by the
   structured expression tree the IR does not carry yet.

2. **The face is emitted from a companion entry point, `generate_face`;
   `generate` is left producing exactly what it produced before this story.**
   `generate(package)` is the pipeline's entry point (`ridl --emit rust`), and
   two of `crates/ridlc`'s own tests bind what it may emit:
   `corpus_entries_compile_to_reviewed_snapshots` calls it over every clean
   corpus entry, several of which carry contract clauses the translator of
   decision 1 must refuse (`window > 0ms`, `result >= 0.0`,
   `level < HANDLE_MAX`), so folding the translator into `generate` would turn
   every one of those entries into an error; and
   `veh_cluster_generated_rust_compiles_with_rustc` compiles `generate`'s corpus
   output with `rustc` and passes no `--extern` flag at all, so a `generate`
   that emitted `::ridl_rt::…` would fail to compile there. The second
   constraint is deliberate and belongs to Epic 10 Task 3, which withholds
   `--extern
   ridl_rt=<path>` from that proof until the task that first makes
   it correct to grant it — E11.13 taking that flag now would remove that
   proof's detector ahead of the story that earns it. `generate_face(package)`
   therefore emits what `generate` emits, plus the descriptor and face items,
   and is the only caller of the clause translator and the only entry point
   whose output names the runtime. This follows the precedent
   [ADR-0017](ADR-0017-proto3-projection-rules.md) decision 1 set for exactly
   this situation: `generate_with(package, others)`, with `generate(package)`
   retained as `generate_with(package, &[])` — later wire backends inherit that
   API, and this is the analogous, less committed choice against
   [ADR-0020](ADR-0020-third-encoding-runtime-layering-and-plugin-system.md)
   decision 7, which replaces a backend's entry point with
   `generate(CodegenRequest) -> CodegenResponse` in a later story: less is wired
   into a signature that record already schedules for replacement. The cost,
   accepted for this story: `ridl --emit rust` does not yet emit the face, which
   is correct rather than a shortfall, because ADR-0018 decision 15 places the
   face behind the frame specification and the transport, and nothing in E11.13
   ships a runtime for a pipeline consumer to link against.

3. **A `Provider` method takes its argument by reference, not by value.** A
   query cannot be implemented as the M1 design's settled example writes it
   (`fn average_speed(&mut self, window: Duration) -> Speed;`, by value):
   `dispatch` must still hold the arguments when it evaluates a query's `ensure`
   clause after the provider method returns (the clause translator of decision 1
   emits `args.0` for exactly this), and a by-value parameter would have moved
   them — the generated payload types implement neither `Copy` nor `Clone`. A
   command has no such constraint — in the generated dispatch the provider call
   is the last use of the decoded argument, so by value would compile — but it
   takes its argument by reference too, so that one rule describes the whole
   trait rather than one method of it. This decision **supersedes** the M1
   design §6 example signatures' parameter shape only; the design's separate
   argument that a command returns nothing and a query returns its declared
   reply (because `Rejected`, the example's return type, is not a `ridl-rt` type
   and a command has no failure the application reports, ridl §6.1) is kept
   exactly.

4. **A consumer-side `Client` call returns `Result<Correlation, SendError>`, not
   `CallError`.** The M1 design fixed the `Client` shape — a query returning a
   `Correlation` a separate `*_reply` method polls — but never named the error
   type a send method returns; the `CallError` vocabulary in the design belongs
   to the settlement table, which is the provider side. `SendError` is what the
   `Caller` port itself returns from `command`/`query`, and it already carries
   `Contract`, so a `require` clause failing client-side is
   `SendError::Contract(Contract::PreconditionFailed)` with no invented
   conversion between two error types. This closes a gap the design left open
   rather than overriding a stated decision.

## Alternatives considered

| Alternative                                                                             | Why not                                                                                                                                                                                                                                         |
| --------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Silently drop a clause the translator cannot express                                    | Generates a provider that accepts arguments its own contract forbids — a worse failure than refusing to generate. See decision 1.                                                                                                               |
| Wait for E5.1's expression-tree translator before generating any clause body            | Blocks the whole MVP on an unscheduled story; the design's settlement rows for `PreconditionFailed`/`ContractBroken` would stay unreachable in the interim.                                                                                     |
| Fold the face's items into `generate` itself                                            | Breaks `corpus_entries_compile_to_reviewed_snapshots` (the translator refuses clauses the corpus already carries) and `veh_cluster_generated_rust_compiles_with_rustc` (which compiles `generate`'s output with no `--extern`). See decision 2. |
| Take the `--extern ridl_rt` flag in the pipeline proof now, ahead of Epic 10 Task 3     | Pre-empts an in-flight lane's own task and removes the detector that task's proof exists to provide.                                                                                                                                            |
| A `Provider` method takes its argument by value, as the M1 design's example showed      | Does not compile: `dispatch` reads the argument again after the provider returns, and the generated payload types are neither `Copy` nor `Clone`. See decision 3.                                                                               |
| Map a client-side `require` failure into `CallError`, reusing the settlement vocabulary | Invents a conversion no port performs; `SendError` is what `Caller` itself already returns and already carries `Contract`. See decision 4.                                                                                                      |

## Consequences

- Positive: `generate`'s contract is untouched by this story — every existing
  consumer of `ridl --emit rust` and every corpus proof keeps behaving exactly
  as before E11.13, because the face's code is reachable only through
  `generate_face`.
- Positive: the entry-point split repeats a pattern the backend workspace
  already has one instance of (ADR-0017 decision 1), rather than inventing a
  second shape for the same problem.
- Positive: a `require`/`ensure` clause that cannot be translated fails loud, at
  generation time, with the clause's own source text in the message — never
  silently as a missing precondition check in the compiled face.
- Negative: `generate_face` only accepts packages whose contract clauses fit the
  narrow accepted form. Every corpus interface with a richer clause is refused
  by `generate_face` today (though never was reachable through `generate`, so
  nothing regresses); this narrows what `generate_face` can be pointed at until
  E5.1 lands.
- Negative: `ridl --emit rust` does not emit the face, so nothing outside this
  backend's own test tree can generate one yet. This is deliberate (ADR-0018
  decision 15) and is expected to be revisited once Epic 10 Task 3 lands the
  `--extern ridl_rt` proof `generate`'s own tests currently withhold.
- Neutral: decisions 3 and 4 are signature choices with no behavioural
  alternative once decision 1's translator and decision 2's clause-holding
  `dispatch` are fixed; they are recorded here because the M1 design's example
  code suggested a signature that does not compile, and a future reader should
  not rediscover that by trying it.

## References

- [ADR-0018](ADR-0018-runtime-core-and-generated-surface.md) decision 15 — the
  face as the runtime layer's phase 2, and its place after the frame
  specification and the transport
- [ADR-0017](ADR-0017-proto3-projection-rules.md) decision 1 — the
  `generate_with`/`generate` precedent this record's decision 2 follows
- [ADR-0020](ADR-0020-third-encoding-runtime-layering-and-plugin-system.md)
  decisions 1, 5, 6 and 7 — the closed encoding set, the crate's module list,
  the runtime layering, and the later `CodegenRequest`/`CodegenResponse` entry
  point decision 2 is the less-committed choice against
- [ADR-0021](ADR-0021-ridl-rt-0.1-api-and-release.md) — the `ridl-rt` API this
  face's generated code calls: `Payload`, the ports, `SendError`, `CallError`
- [the interaction-face design record](../design/interaction-face.md) — the
  as-built architecture these decisions produced
- [`2026-09-16-interaction-face-v0-design.md`](../archive/2026-09-16-interaction-face-v0-design.md)
  §6 — the example signatures decisions 3 and 4 supersede or close a gap in
- [`2026-09-17-interaction-face-v0-plan.md`](../archive/2026-09-17-interaction-face-v0-plan.md)
  — "Settled M2 decisions", the execution record these decisions were first
  written into
- `crates/ridl-backend-rust/src/clauses.rs`, `src/face.rs`, `src/lib.rs` — the
  translator, the face, and the two entry points as built
- `crates/ridlc` tests `corpus_entries_compile_to_reviewed_snapshots` and
  `veh_cluster_generated_rust_compiles_with_rustc` — the two tests decision 2's
  reasoning cites. The second compiles `generate`'s output through the shared
  `rustc_accepts` helper, which passes no `--extern` flag
