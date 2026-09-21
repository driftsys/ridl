# ADR-0023 — The generated interaction face: entry point, clause translator, and call signatures

## Status

Accepted — 2026-09-19. Scope: four decisions taken while implementing story
E11.13, the MVP of the generated `Client`/`Publisher`/`Provider`/`dispatch` face
ADR-0018 decision 15 restores as the runtime layer's "phase 2". Two settle a
mechanism the approved design left unnamed; two supersede or close a gap in that
design's example signatures. All four bind every later story that extends the
Rust backend's interaction face, until superseded: E5.1 (the clause translator),
Epic 10 (the entry-point split), and any later language backend that follows
this precedent (ADR-0020 decision 7). The 2026-09-20 amendment below, which adds
decision 5 and amends decision 4, binds them on the same terms.

Written from lane M's stage M3, on delegated authority — the approved design,
now
[`2026-09-16-interaction-face-v0-design.md`](../archive/2026-09-16-interaction-face-v0-design.md),
did not name these mechanisms, and implementation could not proceed without
them. Sebastien delegated the decisions for the session; each is recorded with
its reasoning so it can be read and overturned. The as-built architecture these
decisions produced is
[the interaction-face design record](../design/interaction-face.md); this record
is the reasoning behind the choices that had more than one defensible answer.

**Amendment (2026-09-20) — decision 4 amended, and a fifth decision.** An
assessment of the face and the ports on 2026-09-20 found two costs the face
imposes on every use: a face borrows its port mutably for its whole life, so one
runtime value can be held by one face at a time and by none while `dispatch`
runs; and a `Correlation` does not record whether it names a command or a query,
so `ack` on a query's correlation returns `None` always. Decision 5 answers the
first and decision 4's amendment the second. Sebastien took both on 2026-09-20;
the working note they come from and his disposition of it are on
driftsys/ridl#429, as its items D-1 and D-4. Both change emitted code, and until
the face change that follows this record merges, this record describes a face
that `crates/ridl-backend-rust/src/face.rs` and the checked-in fixture do not
yet emit. It also describes shapes that
[the interaction-face design record](../design/interaction-face.md) and
[`ridl-rt` by example](../technotes/ridl-rt-by-example.md) still document in
their pre-amendment form; both are corrected in that same change, and until then
a reader who follows their cross-reference to decision 4 finds a success half
those documents do not yet show.

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

   **Consequence note (2026-09-21) — the CLI calls a third entry point, and the
   decision stands.** Story E11.14 (driftsys/ridl#444) gives the pipeline
   `generate_pipeline`, which emits the face and the descriptors beside the
   domain types and the codec, and `ridlc::run_build` calls that. So
   `ridl build --emit rust` does now emit the face, and the sentences below that
   say it does not are the state this decision was taken in, not the state
   today. The decision itself is not amended: `generate` still produces exactly
   what it produced before, unchanged and still bound by the two `crates/ridlc`
   tests named below; what changed is which entry point the pipeline calls, not
   what this one emits. The as-built record is the E11.14 section of
   [`interaction-face.md`](../design/interaction-face.md).

   The decision as taken, unamended, follows.

   `generate(package)` was the pipeline's entry point (`ridl --emit rust`), and
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

   **Amendment (2026-09-20) — the rule is unchanged by the derives, and its
   reason is restated.** Epic 10 task 6 emits the derive set the value-objects
   design's decision 7 fixes, which puts `Copy` and `Clone` on generated types
   whose closure admits them. The reason given above — "the generated payload
   types implement neither `Copy` nor `Clone`" — is true today and stops being
   true then, so it is replaced rather than left in place to become false.

The replacement is a cost rather than an impossibility. Decision 7 derives
`Clone` on every generated type and `Copy` on only some — `Copy` lands when the
transitive closure is `f64`, `i64` or `bool` only — so after task 6 a by-value
query signature can always be made to compile: the emitter copies the argument
where the payload is `Copy` and clones it otherwise, because `dispatch` still
reads it after the provider returns. What by reference buys is that it needs
neither, at every query call. One rule describes the whole trait instead, and by
reference costs nothing. The rule itself does not change, and neither does the
argument for it in the command case.

4. **A consumer-side `Client` call returns `Result<Correlation, SendError>`, not
   `CallError`** — the success half amended 2026-09-20, below. The M1 design
   fixed the `Client` shape — a query returning a `Correlation` a separate
   `*_reply` method polls — but never named the error type a send method
   returns; the `CallError` vocabulary in the design belongs to the settlement
   table, which is the provider side. `SendError` is what the `Caller` port
   itself returns from `command`/`query`, and it already carries `Contract`, so
   a `require` clause failing client-side is
   `SendError::Contract(Contract::PreconditionFailed)` with no invented
   conversion between two error types. This closes a gap the design left open
   rather than overriding a stated decision.

   **Amendment (2026-09-20) — the success half is the call's own correlation
   newtype.** Once the face change that follows this record lands, the success
   half is no longer the bare `ridl_rt::port::Correlation` but a `Copy` newtype
   the face emits per call — `SetLevelCorrelation(Correlation)` for a command,
   `AverageCorrelation(Correlation)` for a query — so a send method returns its
   own call's newtype, `average_reply` takes `AverageCorrelation`, and the
   interface-wide `ack` becomes one `set_level_ack(SetLevelCorrelation)` per
   command. The error half is unchanged: a send still returns `SendError`, for
   the reason above. A query's correlation then cannot be passed to an `ack`,
   and a command's cannot be passed to a `*_reply`; today both compile, and
   `Caller::ack` on a query's correlation returns `None` for every such
   correlation, with no part of the type expressing that it always will. The
   newtype is the face's, not the port's: `Correlation` and `ClaimId` in
   `ridl-rt` stay untyped `u64` newtypes
   ([ADR-0021](ADR-0021-ridl-rt-0.1-api-and-release.md)), because a port carries
   interface numbers, ordinals and bytes and never a payload type, while which
   interaction a correlation belongs to is a payload-shaped fact. Keeping it in
   the face also makes the change codegen-only: no runtime, and no consumer of
   `ridl-rt` that is not generated code, is touched.

5. **Amendment (2026-09-20) — a generated face holds its port by value and has
   no lifetime parameter.** `Client<P>` and `Publisher<W>` hold `P` and `W`
   rather than `&'a mut P` and `&'a mut W`, and `new` takes the port by value.
   The bounds are unchanged: a `Client` still carries exactly the port traits
   its interface needs, and the catalog check
   [ADR-0021](ADR-0021-ridl-rt-0.1-api-and-release.md) decision 3 places once at
   construction is still performed once in the constructor. What changes is what
   a face can be built over. A borrowed port still works —
   `Client::new(&mut port)` compiles with `P` inferred as `&mut Runtime`, under
   the forwarding impls of ADR-0021 decision 11 — and so now does an owned
   handle, a `Clone` handle, or any wrapper that forwards the port traits. Under
   the borrowed form a runtime implementing every port on one value can be held
   by one face at a time and by none while `dispatch` runs over it, which is why
   the round-trip tests build a face inside a block, drop it, and build another
   for the next step (`crates/ridl-backend-rust/tests/interaction_face.rs`); a
   component that reads a signal, sends a command and later polls the reply
   rebuilds its `Client` at each step. This decision **supersedes** the M1
   design's `Client<'a, P>` and `Publisher<'a, W>` shape. A face built over a
   borrow needs the forwarding impls of
   [ADR-0021](ADR-0021-ridl-rt-0.1-api-and-release.md) decision 11 to compile at
   all, because `&mut Runtime` implements no port trait without them, so the two
   changes land in that order even though the ratified note left their order
   open.

   **Correction (2026-09-21) — "still performed once in the constructor"
   describes a check that was never emitted.** The generated `new` is
   `Client { port }` and nothing else; the amendment above was right that the
   move to a by-value port neither added nor removed a catalog check, and wrong
   to say one was there to keep. Read the clause as: the constructor is still
   where the catalog check of
   [ADR-0021](ADR-0021-ridl-rt-0.1-api-and-release.md) decision 3 belongs, and
   holding the port by value is what leaves it somewhere to go. The check itself
   waits on E16.2 (driftsys/ridl#378), for the reason that record's own
   2026-09-21 amendment gives — until a catalog hash is computed rather than an
   all-zero placeholder, the comparison is two zeros — and the story that emits
   it takes the decision left open, which is what `new` does on a mismatch. That
   decision is an amendment to this record, and nothing here anticipates it:
   `new` returning `Self` today is the absence of a decision, not a decision
   that it is infallible. Recorded on driftsys/ridl#448.

## Alternatives considered

| Alternative                                                                             | Why not                                                                                                                                                                                                                                                                                                 |
| --------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Silently drop a clause the translator cannot express                                    | Generates a provider that accepts arguments its own contract forbids — a worse failure than refusing to generate. See decision 1.                                                                                                                                                                       |
| Wait for E5.1's expression-tree translator before generating any clause body            | Blocks the whole MVP on an unscheduled story; the design's settlement rows for `PreconditionFailed`/`ContractBroken` would stay unreachable in the interim.                                                                                                                                             |
| Fold the face's items into `generate` itself                                            | Breaks `corpus_entries_compile_to_reviewed_snapshots` (the translator refuses clauses the corpus already carries) and `veh_cluster_generated_rust_compiles_with_rustc` (which compiles `generate`'s output with no `--extern`). See decision 2.                                                         |
| Take the `--extern ridl_rt` flag in the pipeline proof now, ahead of Epic 10 Task 3     | Pre-empts an in-flight lane's own task and removes the detector that task's proof exists to provide.                                                                                                                                                                                                    |
| A `Provider` method takes its argument by value, as the M1 design's example showed      | Does not compile today, because `dispatch` reads the argument again after the provider returns and the generated payload types are neither `Copy` nor `Clone`; once the derive set lands it compiles only at the cost of a copy or a clone per query call. See decision 3 and its 2026-09-20 amendment. |
| Map a client-side `require` failure into `CallError`, reusing the settlement vocabulary | Invents a conversion no port performs; `SendError` is what `Caller` itself already returns and already carries `Contract`. See decision 4.                                                                                                                                                              |
| A stateless face, every generated method taking the port as an argument                 | Removes the borrow, but adds a parameter to every generated method and leaves nowhere for the once-at-construction catalog check of ADR-0021 decision 3. See decision 5.                                                                                                                                |
| Keep `Client<'a, P>`, and ask every runtime to hand out short-lived ports               | Moves the cost into every runtime rather than removing it, and still admits no owned handle and no wrapper. See decision 5.                                                                                                                                                                             |
| `Correlation<K>` in `ridl-rt`, typed by a marker `K`                                    | Types the port, which contradicts the rule that a port carries identity and bytes and never a payload type, and makes every runtime carry a type parameter it never reads. See decision 4's amendment.                                                                                                  |

## Consequences

- Positive: `generate`'s contract is untouched by this story — every existing
  consumer of `ridl --emit rust` and every corpus proof keeps behaving exactly
  as before E11.13, because the face's code is reachable only through
  `generate_face`. **(2026-09-21: `generate`'s contract is still untouched, but
  the face is no longer reachable only through `generate_face` — E11.14 added
  `generate_pipeline` and pointed the CLI at it. See the consequence note on
  decision 2.)**
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
  **(2026-09-21: closed. E11.14 made `ridl build --emit rust` emit the face, and
  `crates/ridlc/tests/cabin_example.rs` compiles and runs a consumer against the
  emitted crate. This bullet records the cost as it stood, not a live limit.)**
- Positive — added 2026-09-20: once these amendments land, a face can be built
  over an owned handle, a borrowed port or any wrapper that forwards the port
  traits, rather than only over a mutable borrow of a runtime's own value
  (decision 5); and a correlation cannot be passed to a method of the other
  kind, so generated code can no longer call `ack` with a query correlation
  (decision 4's amendment).
- Negative — added 2026-09-20: both amendments change the emitted face, so every
  consumer written against the E11.13 shape is edited once. The MVP is
  in-process and its only consumers are this backend's own tests and the
  checked-in fixture, which is why the change is taken now rather than after the
  first runtime is built against it. That runtime was story E11.9 when this was
  written and is story E11.15 since the split of 2026-09-20 (driftsys/ridl#445).
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
  face's generated code calls: `Payload`, the ports, `SendError`, `CallError`;
  decision 3 is the catalog check decision 5 keeps in the constructor, and
  decision 11 is the forwarding impls decision 5 depends on for the borrowed
  case
- driftsys/ridl#429 — the face-and-port ergonomics note, and the disposition
  that ratifies D-1 and D-4 as the 2026-09-20 amendment
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
