# ADR-0015 — QoS absorption, RPC bounds, and the interface as the unit

## Status

Accepted — 2026-08-04. Scope: the contract-level timing surface for RPC, the
coherence rule, and the composition of interfaces into a service. It is not
epic-scoped: it binds the language surface until superseded, in the way ADR-0011
and ADR-0012 do.

Supersedes nothing. It answers ridl §17.5 open question 5 (QoS), closes ridl
§17.3 open question 3 (the grouping construct), and supplies a better answer to
ridl §17.2 (interaction-set reuse) than the candidate recorded there.

The reasoning trail is two design notes —
[`docs/wip/2026-08-03-rpc-response-bound-design.md`](../wip/2026-08-03-rpc-response-bound-design.md),
which decides the composition in its §11.1, and
[`docs/wip/2026-08-03-multi-interface-services-design.md`](../wip/2026-08-03-multi-interface-services-design.md),
which is the design pass that decision called for. Both are ratified here.

**Why one record rather than two.** The response-bound note names ADR-0015 as
carrying "the absorption principle, the RPC bounds, the coherence rule, and the
kind-aware diff direction", and leaves the composition to a design pass. That
pass produced no independent subject: the coherence rule is stated at the
_interface_ grain, and at the one-interface-per-service restriction the
interface grain and the service grain pick out the same set, so the rule cannot
be exercised until the restriction is lifted. Composition is what makes
decisions 9 and 11 — the coherence rule and the generation unit — mean anything.
Splitting them would put a decision in one record and its precondition in
another. `docs/ROADMAP.md` records two ADRs falling out of the four design
notes, and this is the second of the two.

**Decision 20 is new here, not ratified from a note.** Neither note carries an
IR section for the `Service` message; the multi-interface note's §2.4 lists the
typed-AST, checker, lowering, backend, and diff costs and stops there. The
retirement of the `oneof` field numbers follows from the grammar change and is
recorded so the implementer does not have to decide it mid-story.

This ADR was accepted under the delegated authority recorded in
[ADR-0005](ADR-0005-agent-enablement.md)'s working model — the notes were
written for review, and execution of roadmap stories E9.4 to E9.6 needs the
decisions fixed rather than pending.

## Context

The question that produced both notes: can ridl be the single source of truth
for generating a signal store and an event/command/query dispatcher, and if the
backend carries QoS, must ridl grow a QoS surface?

One test decides every candidate below. It is the deletion test of general form
§4.1 applied to policy rather than to syntax:

> A fact belongs in the contract if a store or a dispatcher cannot be generated
> without it. A fact that reaches no generated code is not a contract term,
> whatever it is called.

Mapping the DDS policy set against ridl as it stands, ten of the thirteen
contract-bearing policies are already covered, several more strongly than DDS
provides them: the interaction kind is the reliability class (§3), §4.4 is
transient-local durability, §9's `max` is DEADLINE and LIFESPAN, §9's `min` is
TIME_BASED_FILTER, §4.2 is OWNERSHIP by construction, §3.1's envelope is
DESTINATION_ORDER, and typl's bounded collections are RESOURCE_LIMITS. Of the
remaining three, history is deliberately out of scope (§4, §5.1), persistent
durability is deferred as debt by ADR-0008 decision 3, and LATENCY_BUDGET wants
no analogue because the DDS specification declines to make it a commitment.

Exactly one row is a genuine gap, and it is not a DDS QoS policy at all: the
**RPC reply timeout**, which lives in DDS-RPC, and whose relatives are gRPC's
deadline, SOME/IP's configured timeout, and AIDL's transaction timeout. ridl §9
gives pub/sub a contract-level view of transport health and §10.3 says so
directly; RPC gets nothing. A `signal` carries a staleness bound that defines
"late" for the observability plane, while a `query` that must answer within 50
ms has nowhere to say so, and every caller invents its own timeout.

Separately, a service admits exactly one named shape or one inline shape. That
restriction makes the coherence group, the generation unit, and the service
indistinguishable, so no claim about any of the three can be exercised.

## Decision

1. **ridl expresses QoS as semantic obligation, never as a transport knob.** A
   binding maps each obligation onto its native QoS; a transport lacking the
   mechanism either satisfies the obligation by construction or fails at bind
   time.

   This is what preserves the §1.1 transport-neutrality claim. The same contract
   binds to SOME/IP, proto/gRPC, DBC, AIDL, MQTT, or an in-process broker
   without carrying any of their vocabulary, because it carries none of their
   mechanisms — only the outcomes a consumer may rely on. The wording of ridl
   §17.5 changes from **exclusion** to **absorption**: the boundary it draws is
   correct, the description of it is not. A QoS block would invert this, putting
   twenty-two orthogonal degrees of freedom into a contract that must also bind
   to CAN, which ridl Appendix F already names as the lesson DDS teaches
   against.

2. **`command` and `query` admit the range form of the §9 timing annotation, and
   only the range form.** No grammar change: `family.ungram` already carries
   `Timing?` on both `CommandDef` and `QueryDef`, because §9.2 deliberately
   admits the annotation on every kind so the narrowing is a semantic rule with
   a semantic message rather than a parse error. The half-open spellings §9
   already defines carry the two partial cases — `@[..100ms]` is a response
   bound with no throttle, `@[20ms..]` a throttle with no bound.

3. **The two bounds keep their generic §9 meaning, and the per-kind consequence
   is derived from the declaring keyword.** RPC extends the existing table
   rather than carving an exception out of it:

   | Bound                   | `signal` (state) | `event` (occurrence) | `command` / `query` (call)                            |
   | ----------------------- | ---------------- | -------------------- | ----------------------------------------------------- |
   | `min` — rate floor      | debounce         | throttle             | **call throttle** — the caller must not call faster   |
   | `max` — staleness bound | refresh ceiling  | TTL                  | **response bound** — the provider must respond within |

   `min` on an RPC constrains the **caller**, not the provider. That is not an
   inconsistency: §9's `min` always constrains whoever initiates, and on an RPC
   the initiator is the consumer. It is enforceable at the provider's admission
   point, and it is what a rate-limiting binding already implements.

   `max` is a **response** bound, not a delivery bound. What responding means is
   derived per kind: for a `query` it is the reply; for a `command` it is
   acceptance — the acknowledgment of §6.1 — since §6.1 promises no execution
   outcome. The specification must state the `command` case explicitly rather
   than gloss it: acceptance covers admission and queueing at the provider but
   **not** execution.

   A pure delivery bound was rejected. Delivery latency is a property of the
   link, so a per-interaction delivery bound would declare the same number on
   every declaration in the package and carry no information. Response time is
   what varies per interaction, which is why gRPC, DDS-RPC, and AIDL all bound
   the whole call.

4. **Warned, never defaulted.** An RPC with no declared response bound draws a
   warning, and an active profile may escalate that warning to an error — the
   same two-step §9.1 already gives an untimed signal or event. What an RPC does
   not get is a **default**. There is no plausible generic value, because what
   the provider does differs by orders of magnitude between interactions; and a
   defaulted response bound is worse than none, because it is a provider
   obligation that callers size their own timeouts against, so inventing one
   manufactures a promise nobody made. Absent therefore means undeclared in the
   IR, and this change stays clear of the "changing the configured default is a
   contract change" machinery.

   The warning is about `max` specifically, not about the annotation:
   `@[20ms..]` declares a throttle and no response bound, so it warns exactly as
   a bare undecorated RPC does. A missing `min` draws nothing, because an
   unbounded call rate is the sensible default and is what every RPC has today.

5. **Strict periodic stays signal-only.** `@Xms` on a `command` or `query` is an
   error. A caller is not isochronous by contract, and §9 already admits the
   strict-periodic mode on `signal` alone.

6. **Diagnostics: one code minted, two rules moved in opposite directions.**

   - **RIDL-112 is minted** — `command` or `query` with no declared response
     bound. Severity warning, escalated to error where the active profile
     requires it. It is the RPC counterpart of RIDL-100 and deliberately not
     RIDL-100 itself, whose text turns on a default having been applied, which
     is exactly what an RPC does not get. RIDL-111 is unavailable: ADR-0008
     decision 21 allocated it to the interface-used-as-a-type error, so 112 is
     the first free code in the band.
   - **RIDL-106 narrows.** It currently covers a timing annotation on `command`,
     `query`, and `fixed`, plus an attribute block on `fixed`. It keeps `fixed`
     in both halves and drops the two RPC kinds.
   - **RIDL-103 widens**, from "strict periodic `@Xms` on `event`" to "strict
     periodic `@Xms` on a kind other than `signal`". It is the same rule — the
     isochronous mode belongs to state alone — now stated over the three kinds
     it excludes instead of one. Widening a rule to more kinds is neither a
     renumber nor a reuse, so it stays inside typl §16's lifecycle discipline,
     and it is the exact mirror of RIDL-106's narrowing.

   RIDL-101 (`X > Y`), RIDL-102 (zero or negative duration), and RIDL-108
   (`@[X..X]`, a degenerate range) apply to an RPC unchanged.

7. **IR: `Timing` is reused unchanged**, on two free field numbers —
   `CommandDef.timing = 3` and `QueryDef.timing = 4`. With both bounds admitted
   all four of `Timing`'s fields carry meaning for an RPC: `mode` is always
   `Range`, `min_us` is the call throttle, `max_us` the response bound, and
   `default_applied` always false, since RPC bounds are never defaulted. A
   dedicated scalar `budget_us` field was rejected: reusing `Timing` keeps one
   representation of a timing bound in the IR rather than two.

8. **diff: a new `Category::RpcBoundChanged`, not a kind-aware branch inside
   `TimingChanged`.** The direction rule does not transfer, and that is what
   forces the separate category. `classify.rs` reads both bounds from the
   consumer's frame, which is right while both constrain the provider — true on
   a `signal` and on an `event`. On an RPC, `min` constrains the consumer, so
   its direction inverts:

   | Change                 | `signal` / `event` | `command` / `query`                                   |
   | ---------------------- | ------------------ | ----------------------------------------------------- |
   | `min` raised           | compatible         | **breaking** — the caller may no longer call as often |
   | `min` lowered          | breaking           | **compatible** — the caller is less constrained       |
   | `max` raised           | breaking           | breaking — a weaker provider promise                  |
   | `max` lowered          | compatible         | compatible — a stronger provider promise              |
   | bound added or removed | breaking both ways | breaking both ways                                    |

   ADR-0012 decision 9 settles the form. Its rule is stated for attribute keys —
   a key with no diff category is classified breaking, never compatible — and
   the principle behind it is that reporting compatible on something the
   classifier does not actually understand is the failure mode to design out. A
   missed branch inside `timing()` fails in exactly that direction: it silently
   inherits the signal rule and calls a raised RPC `min` compatible, when it is
   breaking. A distinct variant fails closed instead, because the three matches
   over `Category` that deny `clippy::wildcard_enum_match_arm` turn a missing
   arm into a compile error rather than a wrong verdict. The cost is one more
   category in a surface that already carries twenty; the benefit is that the
   one part of this change no compiler could catch becomes the one part no
   compiler can miss.

9. **The coherence rule, as normative prose in ridl §14:**

   > The signals of one **provided interface** are published coherently: the set
   > a provider publishes in one step is a set of values it held simultaneously.
   > A consumer reading two or more of them observes such a set wherever the
   > binding preserves the grouping; where a binding cannot, that is a
   > deploy-time constraint, not a weaker contract. The group's identity is the
   > **interface name** where a service names a shape, and the **service name**
   > where the shape is inline.

   The rule is about **production**, and the second sentence is what stops it
   over-promising: the three rules below establish that the provider's published
   set is simultaneous, and they establish nothing about what survives an
   arbitrary transport. Stating only the consumer half would promise delivery
   coherence that decision 10 immediately withdraws.

   The quoted text carries no reference to this record, so it transplants into
   the reference as it stands. Where §14 needs to point at the deploy-time
   constraint, it points at the transport prose decision 10 becomes, not here.

   It is **implicit, not declared.** Three rules the family already states
   produce it: §4.2 gives every flow exactly one owning provider; a provider
   computes its outputs in one step (rmdl's topological schedule); and each
   provider realizes the service it publishes as a whole (rsdl §5.3, RSDL-502).
   So the values a provider publishes in one step are a simultaneous state by
   construction. Signals publishing at different rates do not break this — in a
   given step some cells are written and others are not, but every value present
   is one that provider held at that step, so the observed set remains a state
   that existed. Declared redundancy (rsdl §10) does not weaken this: the rule
   holds of each provider's published set, which is what a consumer reads.

   **Corrected during E9.5.** An earlier wording said "a service is the
   published unit realized by one provider" and "every value present came from
   the same step". The first is contradicted by ridl §14.5, where two components
   providing one service is declared redundancy rather than a conflict; the
   second is false for a cell not written in the step being read, whose value
   the provider still held but computed earlier.

   Declaring `coherent` would declare a consequence of how the platform
   executes, which is what the general form §4.1 deletion test exists to reject.
   It would also be false in the one place it appeared to help: marking one
   interface coherent would imply the others are not, when all of them are.

10. **Production coherence and delivery coherence are different, and the
    difference is a demand on every transport mapping.** The provider producing
    a coherent set is implicit; whether a consumer observes it coherently
    depends on the binding — one versioned block on shared memory, GROUP-scope
    PRESENTATION with `coherent_access` on DDS, per-field only on SOME/IP,
    within one frame only on a static bus. This is a demand no binding author
    can currently discover, because nothing in the reference states it. Where a
    consumer needs the guarantee to survive an arbitrary binding, the answer is
    the struct idiom of §17.3 — one payload on one channel is atomic everywhere
    — which closes §17.3 open question 3. Where a binding cannot preserve it,
    that is a deploy-time constraint, with the exact precedent §14.5 sets for a
    statically deployed service's control API, and rsdl owns it (E6.8,
    RSDL-801/803).

11. **A provided interface is the generation unit; the service is the addressing
    unit.** From one provided interface: one **store** — its signals as one
    coherent block behind a single generation counter, each cell seeded with the
    payload's init value and carrying provenance and the envelope, evaluated
    against its own `max` staleness bound; and one **dispatcher** — its events,
    commands, and queries routed by the interface's single ordinal sequence,
    with typl constraints checked before the handler and the response bound
    applied around it. A service composing three interfaces generates three
    stores and three dispatchers under one logical name.

12. **A service may carry more than one interface.** The grammar becomes a
    comma-separated shape list, where a shape is a `PathType` or a
    `ReservedEntry`:

    ```ungram
    ServiceDef =
      'service' name:DottedName
      ( ':' shapes:ServiceShape (',' shapes:ServiceShape)* ','?
      | '{' (inline_members:InterfaceMember ','?)* '}' )

    ServiceShape =
      PathType
    | ReservedEntry
    ```

    Reusing `ReservedEntry` — already shared by typl's struct and union
    tombstones and by interface bodies — means **service-level `reserved` needs
    no new syntax at all**.

13. **Commas are required between shapes**, diverging from the family's optional
    comma convention, and the reason is structural: every other list in the
    grammar is terminated by a closing token — `}` for a body, `)` for a
    parameter list, `]` for an attribute block — and this one is not, because it
    ends where the next declaration begins. It still parses without them,
    because every top-level declaration starts with a keyword and a bare
    `CamelCase` identifier never does. But it parses greedily, so a mistyped
    declaration on a following line (`Struct Foo {` for `struct Foo {`) is
    absorbed as another shape and the error surfaces at the `{` with no
    connection to the mistake. A trailing comma stays optional.

14. **Named shapes or one inline shape, never both.** Mixing raises "which slot
    holds the inline shape" for no gain, and the either/or keeps `ServiceDef` a
    two-branch alternation.

15. **Interface ids follow ridl §11's model exactly, one level up:** 1-based by
    declaration order, append-only, with insertion or reordering shifting ids
    and removal requiring a tombstone to hold the slot. **An inline shape is
    slot 1**, which makes the inline form a degenerate case of the general one
    rather than a separate construct.

    Making the inline shape slot 1 preserves numbering across "extract the
    inline shape into a named interface" but does **not** make that refactor
    compatible, and the specification must not imply that it does: ADR-0008
    decision 4 derives a fallible return's transport identity from the enclosing
    interface name, and an inline shape uses the service's dotted name instead,
    so extraction rewrites the identity of every fallible query in the shape.
    `ridl-diff` already classifies the switch as breaking and is right to.
    Changing that would be a wire-identity decision in its own right and must
    not be taken as a side effect of this one.

16. **Addressing stays flat.** Members remain `service.member`, and a member
    name duplicated across a service's interfaces is a compile error. This keeps
    the property §14.5 calls the point — a dotted member name is an unambiguous
    system-wide address — and leaves every address written today valid. The
    accepted cost is stated rather than discovered: **two independently written
    interfaces that share a member name cannot be composed into one service
    without renaming one of them.**

17. **Ordinals stay per-interface, and a binding separates the spaces by
    interface name.** Renumbering interactions across a service was rejected: an
    interface's wire identity would then depend on what else the service happens
    to carry, which is the exact coupling §14.1 rejected inheritance to avoid.
    Appendix B already maps a SOME/IP eventgroup to an interface, so a
    multi-interface service maps to several transport-level groupings under one
    logical name. Keying on the interface **name** rather than its list position
    also makes reordering the list invisible to transport identity.

18. **Diagnostics: three codes minted, two existing codes become per-element.**
    The service codes occupy 140 to 143 and RIDL-112 is minted by decision 6, so
    the free codes begin at 144.

    - **RIDL-144** — duplicate member name across a service's interfaces. Error:
      two interfaces both declaring `status` would give `service.status` two
      referents, which flat addressing cannot express.
    - **RIDL-145** — the same interface named twice in one service. Error, and
      worth its own code rather than falling through to RIDL-144, because
      listing a shape twice makes every member collide, so RIDL-144 alone would
      emit one diagnostic per member and bury the actual mistake.
    - **RIDL-146** — an interface re-declared under a service-level reserved
      name. Error. The analogue of RIDL-401 one level up: a tombstone retires a
      name permanently at the service level as inside an interface body.
    - **RIDL-141** (`service` names a type that is not an `interface`) and
      **RIDL-143** (`service` publishes an `internal` interface) apply per shape
      in the list rather than to a single reference. Neither rule changes; the
      span each reports against does.

19. **diff: five new categories, and `ServiceChanged` narrows.** The verdicts
    are inherited, not invented — each is the service-level reading of the rule
    `classify.rs` already applies to interactions:

    | Category                | Verdict                                                                                                                             |
    | ----------------------- | ----------------------------------------------------------------------------------------------------------------------------------- |
    | `ServiceShapeAppended`  | compatible when the slot it takes was never occupied; breaking when the slot was freed by an untombstoned removal and is now reused |
    | `ServiceShapeInserted`  | breaking always — every later id shifts                                                                                             |
    | `ServiceShapeReordered` | breaking always — ids move                                                                                                          |
    | `ServiceShapeRemoved`   | breaking always — the freed slot becomes reusable, so the id is no longer permanent                                                 |
    | `ServiceShapeRetired`   | compatible always — the sanctioned retirement: the slot stays occupied and every later id holds                                     |

    `ServiceShapeRetired` is the row to read twice. `InteractionRetired` is
    classified compatible today on the reasoning that `ridl-diff` judges **wire
    identity**, not source-level API surface: a consumer of the retired member
    breaks at compile time, but the identity model is intact and every other
    member's wire position holds. The service level inherits that reading
    unchanged.

    `ServiceChanged`'s present rule covers "a changed `interface_ref`, or a
    switch between an interface reference and an inline shape". The first half
    is superseded by the five categories; the second half stays, for the reason
    decision 15 gives. Distinct categories rather than branches follow the same
    ADR-0012 decision 9 argument as decision 8.

20. **IR: the `Service` message reserves its retired field numbers.** The
    current `oneof shape { string interface_ref = 10; Interface inline = 11; }`
    does not survive the change, because a `oneof` member cannot be `repeated`.
    Field numbers 10 and 11 are **reserved** rather than reused, and the shape
    list takes fresh numbers. Nothing is published at version `0.0.0`, so the
    change costs no migration; reserving is the cheap move that keeps a field
    number's meaning permanent by construction rather than by the accident of
    nothing having consumed it yet. This is the treatment ADR-0008 decision 8
    applied when IR v2 took over v1's numbering, generalised into a rule; it is
    new in this record rather than ratified from a note.

21. **Event ring depth is derivable wherever both bounds are present, so it is
    not declarable.** An event's `min` is the throttle and its `max` is the TTL,
    so the number of occurrences alive at once is bounded by
    `depth = ceil(max / min)`: `@[50ms..500ms]` can never have more than ten
    live occurrences, because the eleventh cannot exist before the first has
    expired. This is a derived IR fact, the same move ridl already makes for
    wire widths from ranges. It fails only on a half-open range, where one bound
    is unset — and §9.1's defaults mean both bounds are present whenever timing
    was not written explicitly.

22. **History and replay stay out of the contract.** §4 makes a signal
    latest-value only and §5.1 rules out event replay. The strongest case put
    for contract-level history was crash forensics, which is the observability
    plane by definition: a forensic reader reconstructing a trace is not a
    contract consumer acting on occurrences, and the envelope already carries
    what such a trace needs. A consumer that genuinely needs the last N samples
    of a state value can say so with existing vocabulary — a lookback query
    whose bounded return type carries the depth
    (`query recentSpeed(): [Speed;
    1..8]`), where the bound is already an IR
    fact.

23. **The retry-schedule check belongs to a downstream tool, not to `ridlc`.**
    The response bound makes it checkable that a worst-case retry schedule fits
    inside the bound — three attempts at 8 ms and 200 ms cannot fit inside a 50
    ms bound, so the later attempts can never run and the policy states a
    schedule it cannot execute. Running that check means reading the contract's
    IR and a deployment file together. ADR-0008 decision 9 keeps `ridlc` a pure
    source-to-IR function precisely because that is the smallest surface to
    qualify under ISO 26262, and a cross-artifact rule that reads a deployment
    file would enlarge it for one check. The family already resolves this shape
    the same way at §14.5, enforcing at deploy time rather than compile time.

24. **Amendment (2026-08-04) — an interface name must be unique within a
    service, and a retargeted slot is breaking.** The E9.6 review found two
    fail-open defects in the compatibility classifier, and both trace to a hole
    in this record rather than to the implementation alone.

    Decision 17 keys a binding's ordinal spaces on the interface **name**, and
    decision 18 mints RIDL-145 for the same interface listed twice — keyed on
    the canonical **reference**. Nothing in between requires the _names_ of a
    service's shapes to differ. Two distinct interfaces from different packages
    may share a final segment (`fleet.c1.DiagBlock` and `fleet.c2.DiagBlock`),
    and decision 17 then makes them indistinguishable at the binding, while a
    diff walk that matches slots by name collapses them.

    Therefore:

    - **RIDL-147 is minted** — two shapes of one service whose interface names
      collide, even though their references differ. Error, because decision 17
      leaves the binding no way to tell the two ordinal spaces apart. RIDL-145
      keeps its own rule, which is the same reference listed twice; this is the
      different-reference, same-name case, and it needs its own message because
      the remedy differs — an alias cannot fix it, only a rename or a different
      composition.
    - **A slot whose reference changes is breaking.** Decision 19 says the five
      `ServiceShape*` categories supersede the "changed `interface_ref`" half of
      `ServiceChanged`. That means the categories must **cover** the retarget,
      not that the comparison disappears. A matched slot whose reference differs
      is a removal and a reuse of the freed slot, and it classifies breaking.

    Both defects reported **compatible by omission** on a change the classifier
    did not understand, which is the exact failure mode ADR-0012 decision 9
    exists to design out — and the first of them was a regression, because the
    superseded `ServiceChanged` comparison had reported it breaking.

## Alternatives considered

| Candidate                                                 | Verdict  | Reason                                                                                                                                                                                                                                                                                                                      |
| --------------------------------------------------------- | -------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| A QoS block on the interaction                            | rejected | imports transport vocabulary into a contract that must also bind to transports lacking it; Appendix F records that 22 orthogonal policies on one topic is too many degrees of freedom                                                                                                                                       |
| The bound as an attribute — `[ deadline = 50ms ]`         | rejected | `@` is the family's timing sigil (general form R4), a response bound is a timing bound, and the grammar already admits `@` there; two spellings for one concept                                                                                                                                                             |
| A budget on `query` only                                  | rejected | leaves a command's acceptance unbounded for no reason other than reluctance to touch Stratum 3, which §10.3 already crosses for pub/sub                                                                                                                                                                                     |
| A sidecar policy file keyed by `service.member`           | rejected | correct for the per-deployment sizing of the window, permits, and retry row below and it is what rsdl replaces at E6, but wrong for the response bound specifically, which is the one value a cross-check needs against the contract                                                                                        |
| A dedicated scalar `budget_us` IR field                   | rejected | with both bounds admitted all four `Timing` fields carry meaning, so a second representation of a timing bound adds nothing                                                                                                                                                                                                 |
| `idempotent` as a contract term                           | rejected | reaches no generated store, dispatcher, or handler; §6.1's ack and sequence numbers already give duplicate suppression, and `@labels` carries review metadata                                                                                                                                                               |
| `history N` on `signal`                                   | rejected | contradicts §4's latest-value definition; the lookback query of decision 22 expresses the same requirement with existing vocabulary                                                                                                                                                                                         |
| `coherent` as an attribute                                | rejected | implicit — decision 9                                                                                                                                                                                                                                                                                                       |
| An ordering key                                           | deferred | needs a grammar widening (`AttrValue` admits no camelCase name) and has no consumer yet; revisit with evidence                                                                                                                                                                                                              |
| Window, permits, retry sizing in the contract             | rejected | per-deployment sizing, invisible to any peer; rsdl's territory at E6. A call throttle is a two-sided rate obligation; an in-flight window is per-consumer concurrency sizing — not the same thing                                                                                                                           |
| Compile-time mixins for interaction-set reuse (§17.2)     | rejected | mixins flatten, so one shared block folded into three interfaces gets three unrelated ordinal sets and editing it renumbers all three; composition leaves each ordinal space intact                                                                                                                                         |
| Renumbering interactions across a multi-interface service | rejected | an interface's wire identity would depend on what else the service carries — the coupling §14.1 rejected inheritance to avoid                                                                                                                                                                                               |
| Serializing the contract expression tree into the IR      | rejected | E5.1 owns that restructuring, and `docs/ROADMAP.md` records that the corpus does not yet exercise five of the subset's operators, so it would restructure ahead of its regression set. `parse_contract_expr` is already public in `ridl-sem`, so a Rust-hosted generator can recover the tree from the canonical text today |
| Growing the address to `service.Interface.member`         | rejected | composes unconditionally, but changes the shape of every existing address and abandons the flat namespace the catalog is built on                                                                                                                                                                                           |

## Consequences

- **Positive — ridl §17.5 open question 5 is answered, and §17.3 open question 3
  is closed.** The absorption principle replaces a description that invited a
  recurring request for a QoS block.
- **Positive — RPC gains the contract-level view of transport health that
  pub/sub has had since §9.** A caller stops inventing its own timeout, and a
  provider obligation becomes reviewable in a pull request.
- **Positive — the interface becomes a real grain.** Coherence, ordinals, and
  generation all key on it, which is what makes a store and a dispatcher
  generatable from the IR alone.
- **Positive — service-level `reserved` costs no new syntax**, because the shape
  list reuses `ReservedEntry`.
- **Negative — the typed-AST layer is regenerated and every consumer of a
  service's shape moves from one reference to a list**: the checker, the IR
  lowering, both backends, and `ridl-diff`'s walk.
- **Negative — six new diff categories across the two halves** (one for RPC
  bounds, five for shapes), each needing arms in three matches over `Category`.
- **Negative — a naming collision now blocks composition.** Two general-purpose
  interfaces that both declare `status` cannot be composed without a rename.
- **Neutral — this is a deliberate divergence from the prior art.** gRPC's
  deadline is a per-call client parameter with no IDL syntax, SOME/IP defines no
  timeout field, and AUTOSAR declares timing as separate TIMEX constraints. The
  argument for diverging is §9 itself, which already put the pub/sub half of
  exactly this in the contract.

## Documents to amend

| Document                       | Change                                                                                                                                 |
| ------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------- |
| ridl §9                        | the per-kind bound table gains its RPC column; a subsection fixes the response bound and its per-kind derivation                       |
| ridl §9.2                      | "Timing belongs to `signal` and `event`" widens to admit `command`/`query` in the range form; `fixed` still carries none               |
| ridl §11                       | interface ids within a service (decision 15)                                                                                           |
| ridl §14                       | the coherence rule as normative prose; the shape list, flat addressing, and the composition rules                                      |
| ridl §16.1                     | RIDL-106 narrows; RIDL-103 widens; RIDL-112 minted                                                                                     |
| ridl §16.4                     | RIDL-144 to RIDL-146 minted beside RIDL-140 to RIDL-143, where the service codes already sit; RIDL-141 and RIDL-143 become per-element |
| ridl §17.2, §17.3 q3, §17.5 q5 | answered or closed, with the reasoning                                                                                                 |
| ridl Appendix B                | rows for the response bound and for coherence per transport                                                                            |
| ridl Appendix F                | the gRPC-deadline row moves from "≈ relocated" to in-contract, with the per-call override staying Stratum 3                            |
| general form R5                | the postfix order contradicts the shipped grammar — recorded as roadmap story E9.12, outside the scope of E9.4 to E9.6                 |

## Open

1. **Whether a service-level tombstone should be checkable against history.**
   RIDL-146 stops a retired name being re-declared, but nothing verifies that a
   `reserved` entry names an interface the service ever carried. typl's
   tombstones are unchecked in the same way, so checking this one would
   introduce an inconsistency rather than inherit it.
2. **Whether the shape list should admit a visibility modifier per element.** A
   service is always public and RIDL-143 rejects publishing an `internal`
   interface, so the answer is probably no — but composition is where the
   question first has a reason to be asked.
3. **How a multi-interface service renders in `ridl doc`** (E4.1). One service
   with several contracts is a different table shape from one service with one,
   and the doc target has not been designed against it.

## References

- [`docs/wip/2026-08-03-rpc-response-bound-design.md`](../wip/2026-08-03-rpc-response-bound-design.md)
  and
  [`docs/wip/2026-08-03-multi-interface-services-design.md`](../wip/2026-08-03-multi-interface-services-design.md)
  — the design notes this record ratifies
- [ADR-0008](ADR-0008-e2-execution.md) — decision 4 (fallible transport
  identity), decision 8 (IR field numbers are the compatibility contract),
  decision 9 (`ridlc` is a pure source-to-IR function), decision 21 (the
  RIDL-111 and RIDL-142 allocations)
- [ADR-0012](ADR-0012-interaction-boundary-model.md) — decision 9 (fail-closed
  diff classification), which forces the distinct categories of decisions 8 and
  19
- [ADR-0014](ADR-0014-ir-encodings.md) — the IR's own encodings; a different
  subject that this record's IR changes ride on top of
- [`docs/specification/ridl-language-reference.md`](../specification/ridl-language-reference.md)
  — §3.1, §4.2, §4.4, §5.1, §5.2, §6.1, §9, §10.3, §11, §14.1, §14.5, §16.1,
  §16.4, §17.2, §17.3, §17.5, Appendix B, Appendix F
- [`docs/ROADMAP.md`](../ROADMAP.md) — E9.4, E9.5, E9.6 (the stories this record
  binds), E9.12 (the general form R5 drift), E6.8 (rsdl owns transport
  feasibility)
- `crates/ridl-ir/proto/ridl/ir/v2/ir.proto` — `CommandDef`, `QueryDef`,
  `Service`, `Timing`
- `crates/ridl-diff/src/classify.rs` — the direction convention decision 8
  diverges from
