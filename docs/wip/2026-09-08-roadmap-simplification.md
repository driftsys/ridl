# Simplifying the roadmap and the feature set

Status: working note, 2026-09-08, **revised three times the same day after
review**. S-01 to S-17 are the first pass. S-18 to S-23 carry the first
revision: the language order is typl -> ridl -> rsdl -> rmdl (S-18) and the
engine — store, seqlock, sans-IO core, platform traits — parks as a block
(S-19). S-24 to S-28 carry the second: rsdl joins the first release, rmdl is
finalised as a draft and not implemented, code generation narrows to Rust, and
every other language arrives through the plugin protocol rather than through a
backend in this repository. S-29 to S-35 carry the third: typl and ridl are
stabilised against a real contract and its author's feedback rather than against
a date, the generated gateway is held off, and the deployment relations — which
service comes from which component, publishes which interface, on which machine,
and which service consumes it — are scoped by **D-1**, a decision between rsdl
v1 and an out-of-band descriptor. S-34 records why that decision is not open:
rsdl §3.1 defines a component by the rmdl model it applies, so the concept the
requirement leans on hardest is downstream of the language S-25 defers. S-36 to
S-50 carry the fourth: the rsdl vocabulary rules, later consolidated into
[`2026-09-08-topology-vocabulary.md`](2026-09-08-topology-vocabulary.md), and
S-36 reopens D-1. Rule numbers are cited by
[`2026-09-08-ridl-rt-design.md`](2026-09-08-ridl-rt-design.md) §0 and do not
move; where a later rule supersedes an earlier one it says so.

**Name.** That note takes `ridl-rt` for the **library** every generated package
links. This note used it for the **engine**. The library keeps the name; the
parked block is `ridl-engine` throughout, as that note's §0 asks. Where the text
below says `ridl-rt` in the parked sense, read `ridl-engine`.

Proposes a cut to [`docs/ROADMAP.md`](../ROADMAP.md) — which epics stay
scheduled, which are parked with their identifiers intact, what the cut adds,
and what evidence reopens a parked item.

Scope: the plan, not the design. Nothing here retracts a specification, an ADR
or a language reference. Every record stays where it is; what changes is which
of them has work scheduled against it, and what the release labels mean.

Nothing here is ratified. If it is adopted it amends
[ADR-0004](../decisions/ADR-0004-implementation-sequencing-and-stack.md)'s
sequencing a second time, the way the current roadmap already amends it once.

## 1. What the plan costs

Fifty stories have landed: E0 (9), E1 (19), E2 (13), and E9.1 to E9.9 (9). **One
hundred and two remain.** Priced at the roadmap's own sizing — S is days, M is
one to two weeks, L is three to six weeks, taken here as 0.5 / 1.5 / 4.5 weeks —
that is:

| Block              | Stories | Weeks     |
| ------------------ | ------- | --------- |
| E3 boundary model  | 6       | 10.0      |
| E4 ecosystem       | 7       | 14.5      |
| E9 remainder       | 2       | 2.0       |
| E10 value objects  | 11      | 20.5      |
| E11 runtime core   | 11      | 30.5      |
| E12 tooling plane  | 10      | 33.0      |
| E8 agents          | 17      | 19.0      |
| **V1 subtotal**    | **64**  | **129.5** |
| E5 rmdl            | 13      | 36.5      |
| E6 rsdl            | 11      | 20.5      |
| E13 gateway        | 5       | 13.5      |
| E7 rxdl + tail     | 9       | 21.5      |
| **V2/V3 subtotal** | **38**  | **92.0**  |
| **Total**          | **102** | **221.5** |

E8's seventeen stories are counted in the V1 row although five of them ride V2
epics; moving them changes no conclusion below.

Two hundred and twenty person-weeks is four person-years. The sizes are rough
and the throughput multiplier from agent-assisted work is unknown, but the
multiplier applies to every row equally: what the table settles is not the
calendar, it is the **ratio** between what is scheduled and what one author can
carry. Four person-years of scheduled work against one part-time author is not a
plan, it is an inventory.

The inventory is also unevenly earned. E12 is 33 weeks — the largest single
block in V1 — for a tooling plane with no named user. E11, which is the thing
that makes the compiler's output runnable, is 30.5. The plan spends as much on
tools for the platform as on the platform.

## 2. The tension the plan is carrying

The roadmap is serving two goals that want opposite things, and it never chooses
between them.

**Goal A — a public language platform.** Error-index website, browser
playground, getting-started tutorial, package registry, plugin protocol,
`ridl doc`, crates.io names reserved, an mdBook published on every push. This
goal is served by breadth: more targets, more surface, more on-ramps, and a V1.0
that a stranger can adopt unaided.

**Goal B — one system's SSOT.** The first runtime's bus for the consumer's
package, in the consumer's programme, is the only consumer that exists. It needs
typl types with their constraints, ridl interfaces, a stable IR, the two wire
projections, the trait layer that
[`2026-09-08-ridl-abi-design.md`](../archive/2026-09-08-ridl-abi-design.md)
proposes, a Rust emitter and a Kotlin emitter. This goal is served by depth:
fewer things, finished, against a real runtime that will find the defects a
corpus cannot.

Goal A's items are cheap to _write_ and expensive to _maintain against a moving
language_; every one of them drifts each time the surface changes, and the
surface has changed twice this year (ADR-0012, ADR-0018). Goal B's items are
expensive to write once and then stop moving, because a runtime pins them.

The evidence that the plan has not chosen is in the release labels. Line 111
calls V2 "the system platform" and line 116 calls V3 "the executable platform";
the heading at line 709 calls **V2** "The Executable Platform", and the
milestone summary at line 922 does too. Meanwhile V2's own definition says the
architecture becomes "describable and checkable ... with nothing executing yet"
while E13, inside V2, ships a gateway whose exit criterion is traffic crossing
between two encodings. V3 has no heading at all — E5b and E7b sit under the V2
one. A taxonomy that names the same release two ways and contradicts its own
scope in the next paragraph has stopped carrying information.

**The proposal is to choose B for the next release, explicitly, and say so.**
Not because A is wrong, but because A is unaffordable before the language stops
moving, and only B stops it moving.

## 3. Principles

    P-1  One consumer orders the backlog. A story the first runtime does
         not need, and
         that no second named consumer has asked for, is not scheduled.
         It is listed, with its identifier, under "parked".

    P-2  A release executes something. A milestone whose exit criterion is
         that documents type-check is a research result, not a release, and
         does not get a version number.

    P-3  Schedule the extension point, never the extension. The roadmap
         already applies this test to domains — "would the core need it if
         no domain existed" — and does not apply it to the tooling plane,
         the ecosystem, or the gateway, all of which are extensions of a
         seam that is itself unbuilt.

    P-4  A specification is not scope. rmdl, rsdl and rxdl are 2,569 lines
         of written reference. Keeping them as specifications costs nothing.
         Keeping them as 33 numbered stories in a sequenced plan costs the
         plan its shape.

    P-5  Postpone what would otherwise be designed twice. Where two layers
         answer the same question, they get one design, written when the
         harder of the two is understood — not when the easier one is
         convenient.

    P-6  Language order follows what the next layer needs, not the
         reference lattice. The lattice typl <- {ridl, rmdl} <- rsdl is a
         reference relation: it says rsdl *may* name a model, not that
         rsdl cannot be built before one exists.

P-3 is the one that does the most work, and the roadmap states it about domains
in its own opening section. Applied evenly it moves E12 from ten products to one
seam, E4 from five on-ramps to one policy, and E13 out of the plan entirely — a
gateway is two codecs and a routing decision, and neither codec is written.

## 4. The cut

    S-01  KEEP    E10, less its TypeScript half. E10.1-E10.7 and E10.10.
                  This is the constraint layer ridl-abi's `Constrained`
                  trait calls, and E11 depends on it.
    S-02  PARK    E10.8 (pattern validation), E10.9 (TypeScript factories),
                  E10.11 (cross-backend parity). E10.11 exists to compare
                  two language backends; it is rewritten by A-2, not
                  deleted, because the pair it compares changes.
    S-03  ADD     E11.0 — `ridl-abi`, the trait crate. This answers RA-X5:
                  it is not E11.10 grown, it is the story E11.1 and E11.4
                  both stand on, and it comes first.
    S-04  NARROW  E11.1 from a frame and control-plane specification to the
                  port and descriptor contract. The frame is a wire; the
                  first runtime has its own and needs no second one. What E11.1's exit
                  criterion actually asks for — a document a second
                  implementation can be written from — is S-03's crate.
    S-05  KEEP    E11.2 (store layout), E11.3 (platform traits), E11.4
                  (sans-IO core), E11.7 (FlatBuffers codec), E11.8 (proto3
                  codec and conformance).
    S-06  NARROW  E11.6 from a deployment-facts schema to the fact table the
                  emitter reads: placement, trust level, writer and readers
                  per interaction. `Access<S>::CHECKED` (RA-18) is derived
                  from trust levels, so the emitter cannot run without them;
                  everything else in that story is rsdl's, per ADR-0018
                  decision 17.
    S-07  PARK    E11.5 (ring depth derivation — needs the rsdl override),
                  E11.9 (socket binding and Deno client — a second transport
                  for a parked tooling plane), E11.11 (memory feasibility —
                  a deploy-time check with no deployment language).
                  E11.10 is absorbed by S-03.
    S-08  PARK    E12 entirely except E12.2 and E12.4, both narrowed to
                  desktop Rust tools over the reference runtime. E12.1's
                  TypeScript-plus-wasm substrate is not needed if the tools
                  are Rust; E12.3's descriptor engine is the seam and waits
                  on E4.5b; E12.5 to E12.10 are seven products.
    S-09  KEEP    E3.1 (attribute registry), E3.2 (fail-closed
                  classification), E3.3 (`family` and `shape` in the core
                  IR). E3.3 is a hard dependency of S-03: `Interaction::
                  FAMILY` is a const the emitter reads out of the IR.
    S-10  PARK    E3.4 (the four correspondence obligations), E3.5
                  (availability), E3.6 (LSP over families). These are E7's
                  preconditions and E7 is parked.
    S-11  SPLIT   E4.5 in two. **E4.5a** — the IR stability policy and the
                  canonical-encoding decision (driftsys/ridl#231) — stays on
                  the critical path, because E9.10's schema hash pins an
                  encoding and cannot wait. **E4.5b** — the plugin protocol
                  itself — is parked: all four of the consumers the roadmap
                  names for it (domain extensions, a third wire, the
                  gateway's descriptor, contract-generic tooling) are parked
                  by this note, so it is now an extension point with no
                  extension in sight.
    S-12  PARK    E4.1 to E4.4 (`ridl doc`, error index, tutorial,
                  playground). Keep E4.6 (`ridl init`, S) and E4.7
                  (governance CI, S) — E4.7 is what makes S-09's registry
                  enforceable and costs days.
    S-13  PARK    E13 (gateway), E5 (rmdl), E6 (rsdl), E7 (rxdl and the V3
                  tail). **Amended by S-18** (order), **S-24** (E6 returns
                  to the release) and **S-25** (E5's reference is
                  finalised, its implementation stays parked). Thirty-eight stories, ninety-two weeks, no named
                  consumer. Their specifications stay; the deployment-fact
                  subset of rsdl that the emitter genuinely needs is S-06.
    S-14  KEEP    E8.1, E8.2, E8.3 (rules and skill), E8.5, E8.6, E8.7 (MCP
                  server, verify/evolve, IR query). PARK E8.4, E8.8, E8.9,
                  E8.10, E8.11, E8.12 and the V2 tail E8.13 to E8.17. The
                  kept six are the cheapest leverage in the backlog for a
                  repo whose work is done by agents; the parked ones are
                  evals, portability and drift-proofing for surfaces that
                  are still moving.

    S-18  ORDER   The language order is **typl -> ridl -> rsdl -> rmdl**.
                  This amends S-13, which parked E5 and E6 without ordering
                  them, and it amends the current roadmap twice over: that
                  document already moved rsdl ahead of rmdl's *runtime*
                  (E5b) while leaving rmdl's *language* half (E5a) ahead of
                  rsdl, on the ground that E6.3 to E6.5 need rmdl's IR.
                  They do — and E6.6 to E6.11 do not. rsdl's deployment
                  region names targets, placement, protection domains, time
                  base, transport and posture, and mentions no model. That
                  half is the one with a consumer (S-06), so it is the half
                  that returns first, and rmdl is genuinely last.
    S-19  PARK    `ridl-engine` as a block: **E11.1 to E11.5, E11.9 and
                  E11.11** — the frame specification, the store layout, the
                  platform traits and their Linux implementation, the
                  sans-IO core with its seqlock discipline, the ring depth,
                  the socket binding, the memory check. This supersedes
                  S-04, S-05 and S-07, which kept five of them. The reason
                  is P-5, and 2026-09-08-ridl-rt-design.md §0 reaches it
                  from the design side with a better one: a store, a
                  seqlock and a session are **deployment** decisions, not
                  language ones, and execution is rmdl's subject. Take
                  that reason over P-5 where they differ — it says the
                  block is out of ridl's scope, not merely out of
                  sequence.
    S-20  PARK    E12.2 and E12.4 with it, superseding S-08's exception for
                  them. Both were specified as desktop tools over the
                  reference runtime; with no reference runtime there is
                  nothing to observe and nothing to stand in for. E12 parks
                  whole.
    S-21  MOVE    A-3, typl §17.11's width floor, off the critical path.
                  It was scheduled because widening a range shifts every
                  slot offset after it in **E11.2's store layout**; with
                  E11.2 parked, nothing in the plan resolves a width into
                  an offset. It returns to recorded debt where ADR-0013
                  decision 6 and ADR-0019's amendment left it, and it
                  returns to the plan with E11.2.
    S-22  RECAST  E11.7 and E11.8 as **backend** stories rather than
                  runtime ones, scoped to the payload codec alone — encode,
                  verify, decode against `Encode<W>` — with the store and
                  queue framing removed. A codec is generated code over a
                  projection that is already specified (ADR-0017,
                  ADR-0019); only its framing belonged to the runtime.
                  FlatBuffers drops L to M on that basis; proto3 stays L,
                  because byte-level conformance against `protoc` is the
                  expensive half and is unaffected.
    S-23  ADD     **SPIKE-1 — the activation model.** What a service's step
                  is, how a sans-IO core is driven, and how that relates to
                  rmdl's synchronous step. **Amended by S-24**: since the
                  engine leaves ridl's scope rather than its sequence, this
                  note is not ridl's to write and not on this plan. The
                  first runtime needs it now, because the consumer's
                  runtime is building the thing; ridl needs it when rmdl
                  lands. It moves to the consumer's repository and costs
                  this plan nothing.

    S-24  ADD     **rsdl to the first release.** The release is a first
                  version of typl, ridl **and rsdl**. Scope it to what
                  needs no model: E6.1 to E6.8 and E6.10, less the
                  rmdl-facing parts of E6.3 and E6.4. Defer E6.9 (bundles)
                  and E6.11 (test topology as a deployment — it needs a
                  test plane, which E12 parks). rsdl §7 is what S-06's
                  hand-written fact table is a stand-in for, so the table
                  stays as the bridge and is retired by E6.7.
    S-25  ADD     **rmdl as a finalised draft, not an implementation.** One
                  story: a consistency pass over the 1,378-line reference,
                  a Status that says Proposed, and the open questions
                  named. No grammar work, no IR nodes, no checker. This is
                  P-4 made explicit for the one language where the
                  specification is the deliverable — and it is what lets
                  S-19's engine be revisited against a written subject
                  rather than an imagined one.
    S-26  NARROW  **Code generation is Rust, finished.** E10's constrained
                  types, the FlatBuffers payload codec and the proto3 one
                  with its conformance — that is the whole of it. Then the
                  external-implementer seam, in the order below, because a
                  seam is specified against a finished thing or it is
                  specified against a guess.
    S-27  RETRACT **A-2. Kotlin is a plugin, not a backend.** This
                  repository generates Rust; every other language is
                  generated by a backend outside it, through E4.5b. So
                  **E4.5b returns to the critical path** — S-11 parked it
                  on the ground that all four of its named consumers had
                  parked, and this restores the first one. A-4 goes with
                  A-2: cross-backend parity has no second backend here.
                  The cost does not vanish, it moves: the Kotlin plugin is
                  still written, by the same author, in another repository
                  — and that is the point, because it is what proves the
                  protocol. E4.5b's exit criterion, "a third-party backend
                  consumes the IR", finally has a candidate.
    S-29  ADD     **The stabilisation loop, as the finish test for typl and
                  ridl.** "First version" has failed once as a finish test:
                  E2 shipped its exit criterion and ADR-0018 decision 15
                  retracted the layer that met it. Replace it with a loop
                  that has a subject — author the consumer's package
                  catalog (a real contract with real interfaces) as a
                  corpus package, compile it, and record
                  every diagnostic that fired wrongly, every construct that
                  had no spelling, and every place the author reached for
                  something the language does not have. Stabilised means
                  that list stops growing, not that a date passed. Sized M,
                  runs continuously, and seeds E8.9's eval corpus later.
    S-30  CONFIRM **The generated proto3/FlatBuffers gateway is held off.**
                  This is E13, which S-13 already parks — the confirmation
                  costs nothing and closes the question. **Read this rule
                  before acting on it**: it takes "gateway" as E13, the
                  transcoding bridge between the two encodings, which is
                  consistent with S-26 keeping both payload codecs. If what
                  is meant is the *codecs* — E11.7' and E11.8', the
                  generated encode/verify/decode against each wire — then
                  two stories and six weeks leave the release as well, the
                  exit criterion loses its codec clause, and a consumer
                  encodes with its own adapters (the first runtime already
                  has `har-signal`'s `Pod` and `Fb`). That is a coherent
                  release too, and a smaller one; it is not what this note
                  currently plans.
    S-31  NARROW  **rsdl v1 is scoped by its sentence, not by its
                  reference.** The requirement is: this service comes from
                  this component, publishes this interface, on this
                  machine; that service consumes it — because without those
                  relations the emitter cannot derive the **interfacing
                  rules**, which is the per-(interaction, reading machine)
                  matrix ADR-0018 decision 10 and RA-18 describe. That
                  needs seven stories, not eleven: E6.1' (component,
                  `provides`/`requires`, interface and service grains, leaf
                  only — composites defer), E6.3' (cross-layer resolution
                  to typl and ridl), E6.4' (service member completeness and
                  the single-provider check, which is what lets binding be
                  by interface identity rather than by wiring notation),
                  E6.6 (`system` root), E6.7' (deployment region: named
                  targets, complete placement RSDL-701, time base), E6.8'
                  (which connections cross which boundary, and their
                  posture — RSDL-801 feasibility defers), E6.10' (topology
                  emission: the artifact the emitter reads). Deferred
                  inside rsdl: E6.2's wiring notation, E6.5, E6.9, E6.11,
                  composites, declared redundancy, feasibility. Nine and a
                  half weeks rather than seventeen and a half. This
                  supersedes S-24's scoping. **Conditional on D-1 below
                  choosing path R, which S-34 argues it cannot.**
    S-32  DROP    **E11.6', the hand-written deployment fact table.**
                  2026-09-08-ridl-rt-design.md makes `Access::CHECKED`
                  default **true**, skipped only on a measurement. So
                  deployment facts **relax** generated checks; they never
                  enable them, and no emitter is blocked waiting for them.
                  The stopgap was scheduled against the opposite assumption.
                  rsdl §7 carries the facts when E6.7' lands, and until then
                  everything verifies — which is the right default anyway.

    S-33  DECIDE  **D-1 — a language or a descriptor for the deployment
                  relations.** The relations are the same either way; what
                  differs is where they are written. Path R is rsdl v1 as
                  S-31 scopes it, seven stories and 9.5 weeks. Path O is a
                  deployment descriptor the emitter reads, two stories and
                  3.0 weeks. This is a decision, not a rule, and it is
                  argued below. Recommendation: **path O, with one
                  constraint that is not negotiable — out-of-band
                  authoring, in-band representation.** S-34 hardens that
                  from a recommendation into the only available path.
    S-34  RECORD  **rsdl's vocabulary is not settled, and its centre is
                  downstream of rmdl.** Three findings, all textual:
                  (a) rsdl §3.1 and §3.2 define a component by what its
                  body applies — an rmdl **model** makes it a synchronous
                  leaf, sub-components make it a composite. The concept
                  S-31 needs most ("this service comes from this
                  component") is therefore defined in terms of the language
                  S-25 postpones. (b) rsdl §7 makes a **target** a
                  *logical* execution context "named by capability class,
                  never addressed", where what the requirement asks for is
                  machine identity — this interface, on this machine.
                  Reachable through §8's locality derivation, but not the
                  same statement. (c) ADR-0018 records the same doubt twice
                  already: open item 1, rsdl has no protection-domain
                  concept; open item 7, which deployment tiers are targets
                  at all. **Corrected 2026-09-08, later the same day: (a)
                  is overstated.** §3.2's *structural* definition is
                  rmdl-dependent for a leaf body, but §1.3 gives an
                  independent *role* definition — a component is the unit
                  of one reaction, of failure containment, and of
                  independent deployment, "the smallest thing placed,
                  replaced, or bundled" — with no reference to a model. For
                  deployment only the role definition is used, so the noun
                  is available now, with its body left undescribed until
                  rmdl exists. That is also the true situation:
                  implementations are hand-written Rust and Kotlin, not
                  models. So D-1 stays **recommended, not forced**, and
                  (b) and (c) still stand.
    S-35  ADOPT   **The descriptor uses the vocabulary that is settled.**
                  The consumer's topology naming of 2026-08-10 — **machine**
                  (where: the unit of deployment and isolation, covering
                  the QNX host and both guests uniformly), **service**
                  (who: a deployable unit on a machine, carrying a
                  `service_id`), **interface** (what: a contract with
                  exactly one owning service) — is three terms settled
                  against this domain, with `vm` explicitly rejected
                  because it does not cover the host that runs the
                  hypervisor. ridl already owns two of the three: §14's
                  `interface` and `service` are exactly (what) and (who).
                  So the descriptor adds **one** noun, `machine`, and one
                  relation, placement. `component` is not adopted until it
                  earns a job that `service` cannot do, and `target`
                  is not adopted at all: it is the **genus** and machine
                  is a species of it that rsdl never picks. §7 offers
                  "node, ECU, partition, container slot" as what a target
                  may be, which mixes two hardware units with two
                  isolation units; a machine is unambiguously the second
                  kind, and AUTOSAR Adaptive — where the word was taken
                  from — makes a Machine explicitly virtual or physical, so
                  it spans both without conflating them. A target is also
                  a *class* bound to a physical thing late, possibly at
                  runtime, where a machine is a named thing that exists at
                  integration: nobody discovers at run time whether the
                  consumer's service is on the QNX host. The one consequence to carry across:
                  `service_id` travels on the wire and `machine_id` does
                  not — placement is a lookup, never an identity, because
                  placement is what changes between v1 and v2.

    S-36  SCOPE   **The four concepts a simplified rsdl needs, and the two
                  it does not.** Needed: `system` (the closure the
                  completeness checks quantify over), `service` (which ridl
                  §14 already declares), `machine` (S-35), and
                  `deployment` + `place` as a **named, repeatable** region
                  over one system. Not needed: `component` and the
                  composition recursion. The reasoning is one question per
                  noun — what check fails without it. Nothing fails without
                  `component`: every deployment question is asked of the
                  thing that *runs*, and ridl's `service` is the candidate
                  for that. Nothing fails without the recursion: a
                  system-of-systems has no case here. Whereas without
                  `system` there is no set to quantify "every required
                  interface has exactly one provider" over, and without a
                  named `deployment` region the v1 and v2 placements of one
                  description cannot both exist — which is this programme's
                  central fact, not an edge case.
    S-37  RECORD  **Offer/consume and placement must stay in separate
                  regions.** Which service provides and requires which
                  interface does **not** change between the consumer's v1
                  and v2;
                  which machine it runs on does. rsdl §2's two-regions
                  structure is the part of that language that most clearly
                  earns its place here, and flattening the two into one
                  table duplicates the offer/consume rows per deployment
                  and lets them drift. Whatever D-1 chooses, keep the two
                  regions.
    S-38  AMEND   **ridl §14 must say whether a `service` runs.** Today it
                  declares: a published, globally addressable composition
                  of interfaces. S-36 asks it to also be the unit that is
                  placed on a machine, which fuses declaration and
                  execution the way the consumer's vocabulary already does
                  ("a deployable unit of software on a machine, has a
                  `service_id`") and AUTOSAR deliberately does not
                  (Executable -> Process -> Machine). Fusing is right here
                  and it is a **one-sentence amendment to ridl §14**, not a
                  new noun. The cases that would force them apart —
                  one process serving two services, or one service provided
                  redundantly by two processes (rsdl §5.3) — have no
                  instance in this topology. If one appears, the second
                  noun to add is the **runtime** one (process, executable),
                  never the design one (component), because every
                  deployment question is asked of what runs.

    S-39  SHAPE   **Offer a service; consume an interface.** The two sides
                  of the composition region carry different types. A
                  service is the unit of publication, addressing and
                  placement, so it is what is offered. An interface is the
                  unit of contract, so it is what is consumed — and the
                  consumer's rule that an interface has exactly one owning
                  service is what makes interface -> service -> machine a
                  total lookup. This also matches the wire verbs SOME/IP
                  uses (OfferService / FindService) and AUTOSAR AP's
                  provided- and required-**service-instance** split, though
                  AP requires the instance on both sides where this does
                  not. Verb choice is free — `provides`/`requires` aligns
                  with rsdl and AUTOSAR ports, `offers`/`consumes` with the
                  wire — but the **types** on each side are not.
                  Consequence: the offer side is already in the `.ridl`
                  files, so the only new fact in the composition region is
                  the consume edge.

    S-40  SHAPE   **Independent catalogs are catalog *fragments* under one
                  flat namespace, closed at system assembly.** ridl §14.5
                  and RIDL-140 make the service catalog "a flat global
                  namespace spanning packages", and ADR-0015 rejected
                  widening the address to `service.Interface.member`
                  precisely to keep it flat — so namespacing the catalog is
                  not available and should not be reinvented. What *is*
                  available is moving the **closure**: a `.ridl` package
                  declares services and interfaces by name and is a
                  fragment, compilable and releasable on its own; the
                  system description forms the union, and **RIDL-140
                  becomes a system-assembly check rather than a
                  package-compile one**. Independence is then a
                  distribution property, not a naming one, and flat
                  addressing survives intact.
    S-41  CLOSE   **The service number is allocated by the system
                  description.** ADR-0016 decision 8 already settles the
                  substance — hashing the name was studied and rejected,
                  declaration order does not exist to be counted, "the
                  number is a **deployment fact**", and the mechanism is
                  "allocation-and-record, a registry pinned in a
                  lockfile-shaped artifact", deferred to E6 with the rest
                  of deployment. D-1's descriptor **is** that artifact,
                  which means path O delivers a mechanism the record has
                  been carrying as deferred since August, rather than
                  inventing one. Rules that come with it: numbers are
                  append-only, never reused, and a rename keeps its number.
                  Note the record's own scoping — this binds tag-based
                  transports only, because proto and gRPC identity is
                  nominal, so it is the FlatBuffers/shared-memory path that
                  needs it.
    S-42  SHAPE   **Hash per interface, not per catalog.** ADR-0016
                  decision 7 warns that the schema hash answers "is this
                  the same contract" and not "are these compatible", so
                  "anything gating attach on hash equality is choosing
                  lockstep deployment and should say so". A catalog-wide
                  hash therefore makes every service in a catalog deploy in
                  lockstep with every other — which is exactly the
                  independence being asked for, lost. Hash at the
                  **interface**, which is already the unit ADR-0018 §8
                  gives a region to, and the lockstep shrinks to the
                  interface that actually changed. Corollary, and the same
                  move the consumer's vocabulary made for `machine_id`: **a
                  catalog identifier never travels on the wire.** A catalog
                  is a build-time distribution unit. If any runtime
                  artifact names one, the independence is already gone.

    S-43  SPLIT   **Generation follows imports; wiring follows a
                  component's `requires`.** Two questions were being
                  conflated. *What code exists* in a package follows that
                  package's interface **imports** (ADR-0002's module
                  system, already built), which is why a fragment still
                  compiles alone. *What is wired and what access is
                  granted* follows a component's `requires`, which is a
                  deployment-side fact at service granularity, which is
                  what least privilege needs. This supersedes the earlier
                  reasoning that put `requires` on `service` in ridl: the
                  compile-alone argument was answered by imports all along.
                  `service` therefore stays what ridl §14 says it is — the
                  published, posture-neutral catalog entry — and the
                  requires-edge stays where rsdl always had it, on the
                  component. Two tests confirm it: an emulator (E12.4) is a
                  second component offering one service with no `requires`,
                  which the fused form cannot express; and `ridl diff` over
                  a service stays a contract diff rather than mixing in
                  dependency changes.
    S-44  RECORD  **OSGi is the right prior art for the structure and the
                  wrong one for the dynamics.** Its structure is the split
                  this note arrived at independently: `Import-Package` /
                  `Export-Package` resolved at build time for type
                  visibility, and a separate **service registry** with
                  Declarative Services `@Reference` for what is wired —
                  exactly S-43's two questions, separated the same way,
                  decades earlier. A **bundle** is the distribution unit
                  and holds several components, which is S-40's fragment
                  and settles the question the previous entry left open:
                  one fragment may declare several services, so the
                  component noun earns its place over the package. Two
                  attributes OSGi puts on a reference and this family has
                  nowhere — **cardinality** (0..1 optional, 1..1 mandatory,
                  0..n) and **policy** (static, rebind on change, versus
                  dynamic) — are worth knowing about before variant
                  handling arrives, because "this trim has no parking-aids
                  service" is an optional reference and nothing else.
                  **Its dynamics are the part to refuse**: bundles that
                  install, start and update in a running framework, and a
                  resolver that runs at run time, are what make OSGi hard,
                  and they contradict ADR-0018's stamped catalog, static
                  region map and computed store layout. The one place they
                  do apply is **XPK**, whose plugin host already has a
                  lifecycle and a broker — so OSGi is prior art for the
                  plugin layer and an anti-pattern for the base catalog.
                  One vocabulary warning: OSGi's *service* is a registered
                  runtime object looked up by interface, closer to a
                  service **instance** than to ridl §14's declaration —
                  do not import that intuition with the structure.

    NOTE  S-34 to S-50 below reached the topology vocabulary by argument
          over several passes. The results are now stated as a reference in
          [`2026-09-08-topology-vocabulary.md`](2026-09-08-topology-vocabulary.md),
          rules V-01..V-21, which is the document to read and cite. These
          rules stay as the reasoning trail and as the roadmap consequences
          they were written for; where the two differ, the vocabulary note
          is later and wins.

    S-45  SETTLE  **Five nouns, three levels, no recursion.**

                      machine    the VM or host      AP: Machine
                      process    the service binary  AP: Process/Executable
                      component  one execution
                                 context — a thread
                                 or a runtime task   Classic: OS Task
                                                     AP: nothing
                      service    what a component
                                 publishes (n:1)     AP: ProvidedServiceInstance
                      interface  the contract        AP: ServiceInterface

                  A component is a **sans-IO synchronous step machine**
                  driven by a **pump** that is sync (a thread loop) or
                  async (a runtime task); which pump, and at what priority,
                  is a deployment fact and not a property of the step. This
                  replaces the recursion: containment is
                  component-in-process-on-machine, three distinct nouns
                  with one job each, never component-in-component. Crossing
                  kinds become four — intra-process, inter-process on one
                  machine, inter-machine, off-board — and the process level
                  is what distinguishes the first two, so the descriptor
                  needs it.
    S-46  CORRECT **rsdl §1.3's three properties belong to two different
                  levels.** It says a component is the unit of one
                  reaction, of failure containment, and of independent
                  deployment. At the granularity S-45 fixes, only the first
                  holds. Threads and runtime tasks in one address space do
                  **not** contain each other's faults — a panic may be
                  caught, an abort or a corrupted write is not — and
                  nothing is deployed at thread granularity either; binaries
                  are. So:

                      one reaction, one activation  -> component
                      failure containment           -> process, and for
                                                       freedom from
                                                       interference, machine
                      independent deployment        -> process

                  This matters beyond tidiness: an assurance argument that
                  cites §1.3 for freedom from interference between two
                  components in one process is citing a property that
                  level does not have.
    S-47  WARN    **Do not borrow the word `SWC`.** In Classic an SWC is a
                  design type whose runnables are distributed across tasks
                  at integration: one SWC's runnables may land in several
                  tasks and one task may hold several SWCs' runnables, so
                  SWC-to-execution-context is **n:m** by construction.
                  S-45 asserts 1:1. Fusing design unit and execution
                  context is the right call here — the n:m freedom exists
                  to pack runnables on small microcontrollers, which is not
                  this problem — but it should be a knowing fusion under a
                  word that does not already mean the unfused thing.
    S-48  RULE    **Two rules that keep the pump choice free.** (a) A step
                  does no IO, never blocks, and always terminates — the
                  moment one awaits or blocks, neither pump works and the
                  step stops being testable without a runtime. This is
                  rmdl §6.2's step contract and the totality requirement of
                  its function layer, which is the confirmation that rmdl
                  is the language for the *inside* of a step machine and
                  deliberately says nothing about the pump (§6.7, "the
                  scheduler has no clock, it has a timeline"). The parked
                  `ridl-engine` is exactly the pump and nothing more.
                  (b) A crossing between two components keeps identical
                  semantics whichever transport carries it, so an
                  intra-process crossing optimised into a direct channel
                  still behaves as the store and the queue do. Without that
                  rule, relocating a component to another process becomes a
                  code change, which is what the pulled ports exist to
                  prevent. Hazard to watch: thread-pumped and task-pumped
                  components in one process couple through the runtime, so
                  either segregate them or state that no async-pumped
                  component may block.

    S-49  STRESS  **Where S-45's model strains.** It holds; these are the
                  four places it costs something, none of them fatal.
                  (a) **Assurance follows the process, not the component**
                  (S-46). Two components at different assurance levels
                  cannot share an address space, so the level is a
                  *process* attribute and placement must reject a mix —
                  which is ADR-0018 open item 1 arriving as a checkable
                  rule rather than a gap.
                  (b) **`Busy` is where sans-IO purity meets reality.**
                  `SignalWriter::commit` cannot fail (RA-16), but
                  `EventSink::raise` returns `Busy`, and a step that must
                  never block cannot wait it out. So the step needs a
                  representable answer — drop and let the `seq` gap be the
                  evidence, or carry "could not emit" in its own state
                  (rmdl §8's step faults are the candidate). Decide it
                  once, in the step contract, not per component.
                  (c) **Timing bounds partition components.** One component
                  is one reaction and therefore one rate; an interface at
                  `@20ms` and one at `@500ms` in the same component force
                  the step to the tighter bound. Component boundaries are
                  drawn by timing as much as by concern, which is worth
                  knowing before they are drawn.
                  (d) **A late pump looks like staleness, not like an
                  overrun.** The envelope is stamped at publish from the
                  runtime clock (RA-05) and freshness is computed against
                  `Signal::MAX`, so a pump that misses its deadline
                  surfaces downstream as a stale signal rather than as a
                  scheduling fault. The pump should detect and report its
                  own overrun; otherwise the diagnosis lands on the wrong
                  component.

    S-50  SETTLE  **One unit, one job — and no unit holds two.** The test
                  the vocabulary has to pass is that each question has
                  exactly one answer and each noun answers exactly one
                  question.

                      what is versioned, signed, shipped   bundle
                      what is isolated, started, placed,
                        and carries the assurance level    process
                      what steps, at one rate, with a pump component
                      what is addressed on the wire        service
                      what is contracted                   interface

                  So a component is the unit of **execution only**. It is
                  not the unit of distribution — you ship a bundle, not a
                  thread, and making every step machine separately signed
                  and OTA-updatable is absurd overhead for what is a loop
                  with a step function. It is not the unit of isolation
                  either (S-46). In the consumer's target vehicle all of
                  these happen to
                  be 1:1 except components-per-process, which is the reason
                  the fusion is tempting and the reason it must stay a
                  fact about today's deployment rather than a law: bundle
                  and process differ in cadence (a bundle is replaced, a
                  process restarts), component and service differ so that
                  re-partitioning execution is not a contract change
                  (S-50 rests on the same argument as the 1:n decision).
                  The platform agrees: AAOS already ships APKs and APEXes
                  that hold many execution units, so fusing the two here
                  would fight the system underneath.

    S-28  CONFIRM **TypeScript through wasm is already the record.**
                  ADR-0018 decision 6: TypeScript and Kotlin receive
                  generated types and interfaces only, and the codec is the
                  Rust one, reached via wasm and via JNI respectively.
                  That is the load-bearing reason S-27 works — a language
                  plugin emits types and interfaces and binds the Rust
                  codec, which is plugin-sized work, where a full backend
                  with its own codec is not. Follow-on: the codec crate
                  needs the `wasm32` guard `just wasm-check` already
                  applies to the compiler crates (E1.19), or the wasm path
                  is asserted rather than checked.

E8 is the one place where P-1 does not decide, because the consumer of E8 is not
the first runtime, it is this repository. That is a legitimate second consumer
and it is named rather than assumed.

**Why the runtime parks (S-19, P-5).** E11.4's sans-IO core is specified as a
loop whose `poll` returns a deadline; E5.2 derives a topological schedule, E5.5
gives a step a logical-time context, and E5.12 is a scheduler that activates on
input and deadline and is quiescent when idle. Those are one scheduling model
described twice, from two ends, in two epics that the current plan runs eighteen
months apart. Whichever is written first decides the other, and the one written
first would be decided by transport convenience rather than by the semantics
that have to hold. The store follows the same logic one level down: a seqlock
discipline is a _consequence_ of when a reader may observe a generation, and
that is the activation model's answer, not an independent choice. So the block
waits on SPIKE-1 — a note, not a language.

The cost is stated plainly rather than argued away: with `ridl-rt` parked, **the
first runtime is the only implementation of the seven ports**, and RA-02 and
RA-03 claim they are defined once and implementable by any runtime. A trait
layer validated by one implementation is shaped by that implementation. The
partial mitigation is that the Kotlin client is a second _binding_ of the same
ports across a language boundary (the consumer's own records), which finds a
different class of defect than a second Rust runtime would, and finds it sooner.

**D-1 — a language or a descriptor (S-33).** The relations are the same either
way: service, its component if it has one, the interfaces it publishes, the
machine it runs on, and who consumes them. What differs is where they live.

    path R   rsdl v1 as S-31 scopes it       7 stories   9.5 weeks
    path O   a deployment descriptor read
             by the emitter, with its
             schema in the IR                2 stories   3.0 weeks

**S-34 largely settles this.** rsdl §3.1 defines a component _by the model it
applies_, so path R cannot be scoped without deciding what a leaf component is
with no rmdl to situate — which is exactly what S-25 defers. What follows is why
path O is also right on its own terms, independent of that.

Path O is the argument already accepted one level down. The engine parks because
deciding execution before rmdl exists is deciding it blind
(2026-09-08-ridl-rt-design.md §0). Designing rsdl's deployment surface before
anyone has written deployment facts for a real system by hand is the same
mistake one layer up: the language gets specified against an imagined authoring
experience. ADR-0018 decision 17 already anticipates the hand-written phase and
says rsdl becomes the authoring surface _later_. And a third language in flight
competes directly with what this steering put first — stabilising typl and ridl
against user feedback (S-29). Two languages can be stabilised at once; three
cannot.

**The constraint that makes path O safe: out-of-band authoring, in-band
representation.** The descriptor is authored in TOML beside `ridl.toml`, but it
is _represented_ as a message in the IR, populated from that file rather than
from a parser, and classified by `ridl diff` like everything else there. Without
that it is a second source of truth that drifts from the contracts it places —
which is the one failure this project exists to prevent, and a config file is
exactly where it happens. With it, path O is rsdl minus the grammar, the
resolver, the LSP and the diagnostics, and those are the parts rsdl should earn
rather than assume.

What path O costs: no surface syntax, no hover, no goto-definition, and errors
from a schema rather than from a checker. What it buys, besides six weeks, is
that rsdl §7 gets designed from a descriptor somebody has had to maintain — a
better input than an 815-line reference written before one existed. The risk is
the config file that grows a language; the guard is that the moment the
descriptor wants an expression, a reference that resolves, or a rule with a
diagnostic, it has become rsdl and should be rsdl.

**Is a target a machine? (S-35.)** Not quite, and the difference is the whole
reason to prefer the word. Three axes separate them.

    identity     a target is a logical role bound to physical late and
                 "potentially runtime-discovered" (§7); a machine is a
                 named thing fixed at integration — aaos-ivi, aaos-sdv,
                 the QNX host. Placement changing between v1 and v2 is a
                 known, planned event, not a discovery.
    isolation    a machine is defined as the unit of deployment AND
                 isolation. §7's target may be a node, an ECU, a partition
                 or a container slot — two hardware grains and two
                 isolation grains, undistinguished. This is ADR-0018 open
                 item 1 seen from the other side: rsdl has no
                 protection-domain concept, and that is exactly what a
                 machine is defined by.
    containment  a QNX host runs a hypervisor over two guests. Three
                 targets, or one target with three partitions? rsdl has no
                 containment relation between targets and cannot say.
                 The machine vocabulary answers it — three machines, with
                 host and guest as roles a machine holds relative to the
                 hypervisor rather than kinds it belongs to.

Interfacing rules turn on the second and third of those, not the first: what a
crossing costs depends on whether it stays inside one machine, hops the
hypervisor between two guests on one SoC, or leaves the board. So the descriptor
takes `machine`, and `target`'s capability-class abstraction is deferred with
the rest of rsdl — it is the right idea for a fleet with variants and an
abstraction with no consumer in one cockpit. When rsdl proper is written the two
probably become two layers rather than one word: a **target** is what a
deployment _requires_, a **machine** is what a build _has_, and placement is the
binding between them. Writing that down now is what lets the descriptor grow
into rsdl instead of being replaced by it.

**What AUTOSAR calls these things, and why not `RTE` (S-35).** The question
"would AUTOSAR say RTE" has a clean answer: only the Classic platform says it,
and there it is not a place. The **RTE is generated middleware for one ECU** —
the realisation of the Virtual Function Bus, derived from the system description
and the SWC-to-ECU mapping. It names a _code layer_, not an execution context.
Classic's execution context is the **ECU**, and its protection domain is the
**OS-Application**.

That matters here for a reason beyond taste: in this family the RTE's structural
analogue already exists and is called something else.

    AUTOSAR Classic          this family
    ---------------          -----------
    VFB + PortInterface      ridl: interface, service
    SWC                      (no deployment counterpart — see below)
    System + SystemMapping   the D-1 descriptor
    ECU                      machine
    OS-Application           the protection domain ADR-0018 open 1 lacks
    RTE (generated)          the emitted bindings + ridl-rt

So `RTE` would name the generated glue, which is exactly what ridl already
generates — the collision the consumer's naming pass recorded on 2026-08-10 when
it rejected the word, and a second collision with AUTOSAR Classic's own meaning
that would mislead an automotive reviewer.

The Adaptive platform is the closer reference, and it is where `machine` comes
from — **verified 2026-09-08 against the AP documentation**, not taken on
memory. A **Machine** is "quasi a virtualized ECU-HW, an entity where software
can be deployed to"; one real ECU may run several Machines, though the typical
mapping is one to one. That is exactly the QNX-host-plus-two-guests case, and it
settles the containment question the same way the consumer's naming pass did:
three machines, on one piece of hardware.

**Adaptive has no RTE at all.** ARA — the application-facing APIs of the
functional clusters, `ara::com`, `ara::exec` and the rest, shipped as C++
libraries — replaces Classic's code-generation-heavy RTE with direct calls. That
is structurally what `ridl-rt` is, which is the second reason the word `RTE`
names the wrong layer here. The deployment chain is **Executable → Process →
Machine**, with machine states (Startup, Running, Shutdown) deciding which
processes are active.

Its deployment manifests split along a line worth copying:

    Machine manifest             what a machine is —
                                 network, resources, states   ~ descriptor, 1
    Execution manifest           executables, processes,
                                 startup dependencies         ~ descriptor, 2
    Service Instance manifest    which service interface is
                                 bound how, on which machine  ~ interfacing rules
    Software Distribution        packaging and update         (UCM — not ours)

The design-time description sits outside that set, which is where ridl sits. The
load-bearing observation is the third row: AUTOSAR needed a **separate
manifest** to say which service instance is bound to which transport on which
machine, rather than deriving it from the interface description — which is
evidence for the shape D-1 path O proposes, from the standard closest to this
domain.

**And Adaptive keeps design and deployment nouns apart.** There is a component
type at design (`AdaptiveApplicationSwComponentType`), but nothing is deployed
by component: an Executable is built, a Process is its running instance, and the
Process is placed on a Machine. rsdl fuses the two — its `component` is the
design noun, the deployment noun and the recursion all at once (§3.2) — and that
fusion is precisely why it cannot be scoped for deployment alone (S-34).
Clause-level details vary by AUTOSAR release and should be checked against
whichever one is cited; the vocabulary above has been stable across them.

**What a simplified rsdl actually contains (S-36 to S-38).** Four concepts, and
the whole of it fits on a page:

    system Cockpit                       // the closure
      service disco  offers   Disco
      service hub    offers   Hub, Cluster
                     consumes Disco
      service hmi    consumes Hub         // an interface, not a service

    deployment V1 for Cockpit            // placement, region two
      machine qnx_host
      machine aaos_sdv
      machine aaos_ivi
      place disco on aaos_sdv
      place hub   on aaos_ivi
      place hmi   on aaos_ivi

    deployment V2 for Cockpit
      ...
      place hub   on qnx_host            // the only line that differs

**The two sides are not the same shape (S-39).** A service is _offered_ — it is
the published, addressable, placeable unit, and half a service cannot be
offered. An interface is _consumed_ — a consumer needs a contract, not a
publication, and requiring the whole service would make every unrelated
interface in it the consumer's business, which `ridl diff` would then classify
as affecting them. The asymmetry buys three things: least privilege, since the
interfacing rules grant a consumer only the regions of the interfaces it names;
immunity to recomposition, since a service that gains or splits interfaces
between v1 and v2 does not touch its consumers; and a total lookup, because the
consumer's vocabulary already fixes **one interface, exactly one owning
service**, so interface -> service -> machine resolves without ambiguity. That
rule is what makes consuming an interface well defined at all, and it is already
settled.

**Which makes the composition region thinner than it looks.** The offer side is
already SSOT in ridl §14 — a service _is_ its composition of interfaces, and the
`.ridl` files say so. What the description adds on that side is nothing. **The
only genuinely new fact in the composition region is the consume edge**, and the
rest is placement. Whether that earns a grammar is exactly what writing it once
will show.

The interfacing rules fall straight out: for each (interface, consumer) pair,
look up the provider's machine and the consumer's machine, and the crossing kind
— intra-machine, hypervisor, off-board — is determined, which is what decides
transport, whether `Access::CHECKED` may be relaxed, and which region a signal
lives in. No component, no recursion, no reference to a model, and the v1/v2
story is two `deployment` blocks over one unchanged composition.

**This reopens D-1, at much lower stakes.** S-34 argued path R was blocked
because rsdl's component is defined by the rmdl model it applies. Drop the
component (S-36) and that blocker is gone: four concepts have no rmdl in them
anywhere. What survives is the weaker caution — do not design a surface before
writing the facts by hand — so the decision becomes smaller and later rather
than settled. **Recommendation, revised: write the first descriptor in TOML for
the consumer's real target-vehicle topology, then decide.** A week of authoring
says more about whether this wants a grammar, diagnostics and an LSP than any
amount of arguing here, and the four concepts above are small enough that either
answer is cheap. What must not happen is the third option — a TOML file that
grows a grammar one field at a time without anyone deciding it did.

**How independent catalogs materialise (S-40 to S-42).** Three moves, none of
them a new concept:

    fragment    a .ridl package declares services and interfaces by NAME
                and no numbers — compiles, versions and ships alone
    union       the system description forms the catalog, allocates the
                service numbers, and is where RIDL-140's uniqueness is
                actually checked
    stamp       the schema hash is per interface, so a fragment that
                changes re-stamps only the interfaces it touched

Cross-fragment type sharing needs nothing new: ADR-0002's module system,
`ridl.toml` imports and the SHA-256-pinned lockfile already carry a typl package
from one fragment into another, and that is the only cross-fragment reference a
contract needs.

What this does **not** solve is who coordinates the allocation when two teams
add a service in the same week. For one cockpit with a handful of services a
central registry file is right and anything else is over-engineering. A third
party shipping interfaces through XPK is the case that would force a namespaced
or vendor-prefixed number instead, and ADR-0015's rejection of a wider address
is the constraint that makes that expensive — so it is worth knowing in advance
that plugin-supplied contracts are the thing that breaks this scheme, and not
discovering it at the first plugin.

**Is a VM a system? (S-34, S-35.)** No, and the question is worth answering in
the note because three vocabularies disagree about it. In rsdl §6 a `system` is
the **root component** — one per workspace, with an external boundary and an
assurance profile — so it is the whole thing under design, not a box that thing
runs on. A VM is closest to rsdl's `target`, except that §7 defines a target as
logical and never addressed, and the requirement wants it named and addressed.
In the consumer's vocabulary a VM is a **machine**, and `vm` was rejected as the
word precisely because the QNX host is a machine too and does not sit under the
hypervisor. So: a VM is a machine; the cockpit is the system; `host` and `guest`
are roles a machine holds relative to the hypervisor, and the consumer's service
changes sides at v2 without its machine changing what it is. Any descriptor that
conflates the first two will need re-authoring at v2, which is the concrete cost
of getting this wrong.

**Why rsdl before rmdl (S-18, P-6).** Three things want rsdl and none of them
wants a model. `Access<S>::CHECKED` (RA-18) is derived from the trust levels of
a deployment. ADR-0018 open item 1 records that rsdl has no protection-domain
concept, which is the boundary decision 10 derives from. And S-06's fact table
is a hand-written stand-in for exactly the region rsdl §7 specifies — ADR-0018
decision 17 says as much. rmdl, by contrast, wants the activation model that
SPIKE-1 owes, so it sorts behind both rsdl and the runtime. The lattice is
unharmed: a component may declare `provides` with no `realizes`, and rsdl checks
that.

## 5. What the cut adds

Four items, because a cut that only subtracts hides the work it displaced.

    A-1  `ridl-abi` as E11.0 (S-03). Sized M. Most of it is written in
         2026-09-08-ridl-abi-design.md already.
    A-2  `ridl-backend-kotlin`, sized L, on the critical path.
         **Retracted by S-27** — the finding stands and the answer moved:
         Kotlin is generated by a plugin outside this repository, which is
         P-3 applied to a backend the way this note applies it to
         everything else. A-2 scheduled the extension; S-27 schedules the
         extension point.
         **ADR-0018 decision 6 narrowed language scope to Rust, TypeScript
         and Kotlin, and the roadmap has no Kotlin story anywhere.** The
         only mention of Kotlin in ROADMAP.md is rung 3 of the platform
         ladder, calling it "a real target". Rung 3 is where the first
         runtime's client runs, and that client is what makes the seven
         ports a contract
         rather than one crate's internal shape. Kotlin takes the priority
         TypeScript holds today.
    A-3  typl §17.11's deferred width floor, as a story rather than a
         footnote. ADR-0018's Consequences call it a prerequisite;
         ADR-0013 decision 6 required it closed before FlatBuffers shipped
         and it did not close. It blocks E11.2 outright, and a prerequisite
         that blocks an L story is itself work. **Superseded by S-21**: it
         blocks E11.2 and E11.2 is parked, so it leaves the plan with it.
         The finding stands — a prerequisite that blocks an L story is
         work, not a footnote — and it is why the two travel together.
    A-4  E10.11's replacement (S-02): cross-backend parity over Rust and
         Kotlin instead of Rust and TypeScript.

A-2 is the finding worth arguing on its own. The plan spends 33 weeks on a
TypeScript-fronted tooling plane and zero on the backend for the one production
platform the ladder names, while an ADR has already decided Kotlin is in scope.
That is not a prioritisation error at story level; it is what happens when the
tooling plane inherits the priority the consumer should have.

## 6. The plan that survives

Under D-1 path O, which S-34 makes the only available one:

| Block                          | Stories | Weeks    |
| ------------------------------ | ------- | -------- |
| S-29 stabilisation loop        | 1       | 1.5      |
| E3.1-E3.3 (extension seam)     | 3       | 3.5      |
| E4.5a, E4.6, E4.7              | 3       | 2.5      |
| E4.5b plugin protocol (S-27)   | 1       | 4.5      |
| E9.10, E9.12                   | 2       | 2.0      |
| E10, Rust only                 | 8       | 13.0     |
| E11.0 `ridl-rt` library        | 1       | 1.5      |
| E11.7', E11.8' payload codecs  | 2       | 6.0      |
| Deployment descriptor (D-1 O)  | 2       | 3.0      |
| rmdl draft finalisation (S-25) | 1       | 1.5      |
| E8 knowledge + MCP             | 6       | 8.0      |
| **Total**                      | **30**  | **47.0** |

Path R would substitute rsdl v1 (S-31) for the descriptor: 35 stories and 53.5
weeks — though S-36 reprices it. Nine and a half weeks was rsdl v1 _with_
components, composites and wiring notation; a four-concept language is nearer
three or four, which is why D-1 is now a decision to take after the first
descriptor exists rather than before. If S-30's second reading is the intended
one and the payload codecs go too, subtract two stories and six weeks from
either.

Against the starting point of one hundred and two stories and two hundred and
twenty weeks, the release is under a third of the plan by count and about a
fifth by weight. Unlike the first pass, most of that came from deciding what the
release _is_ rather than from striking rows.

The exit criterion:

> typl and ridl each have a first version, stabilised against a real contract
> rather than against a date. A `.ridl` package plus a deployment descriptor
> generates Rust types carrying their constraints, both payload codecs, and the
> interfacing rules for each reading machine, against `ridl-rt`. The first
> runtime serves them across a process boundary on target hardware. And one
> backend outside this repository generates Kotlin from the IR.

Five clauses, five different people could check one each.

Sequence:

    E11.0 (ridl-rt) → E10 → E11.7' → E11.8' → E4.5b → Kotlin plugin, out of repo
       ╰──── S-29 stabilisation loop, continuous ────╯
       ╰──── descriptor → interfacing rules ────╯
       ╰──── E3.1-E3.3, E4.5a, E9.10, E9.12 ────╯
       ╰──── E8.1-E8.3, E8.5-E8.7 ────╯
       ╰──── rmdl draft finalisation ────╯

E11.0 leads: E10's constrained constructors implement `Constrained`, which the
library declares, and both codecs are written against `Payload<E: Encoding>`.
E4.5b follows the codecs, per S-26. The descriptor threads and is independent of
the codegen line until the interfacing rules are emitted.

**Release labels.** One label survives: **V1 — the contract platform**, defined
by the sentence above. V2 and V3 are withdrawn as labels, not as ambitions. The
parked epics are listed under "unscheduled" **in the order S-18 fixes** — rsdl
proper when the descriptor has earned it and S-34's vocabulary is settled,
rmdl's draft, then the engine and rmdl's implementation together — and the next
release is named when something reopens one of them. This removes the taxonomy
§2 shows has stopped agreeing with itself, and it removes the obligation to keep
two future releases coherent while neither has a consumer.

## 7. What reopens a parked item

Parking is not deletion, and the discipline that makes that true is the same one
the roadmap already applies to epic numbers: **identifiers are identity and are
never reused or renumbered.** Every parked story keeps its row, its `Done when`
and its size, under a heading that says it is unscheduled.

Each parked block gets one line saying what reopens it, so that reopening is an
observation rather than an argument:

| Parked            | Reopened by                                                                                                     |
| ----------------- | --------------------------------------------------------------------------------------------------------------- |
| E3.4-E3.6         | a contract at the person or world boundary that must diff                                                       |
| E4.1-E4.4         | a second organisation adopting the language                                                                     |
| E4.5b             | a wire beyond proto3 and FlatBuffers, or a domain plugin                                                        |
| E5 (rmdl impl)    | its draft finalised (S-25), rsdl shipped — last by S-18                                                         |
| E6 (rsdl proper)  | S-34's vocabulary settled, and the D-1 descriptor wanting an expression, a resolving reference, or a diagnostic |
| E7 (rxdl)         | E3.4 landing, plus a domain that wants its own spellings                                                        |
| `ridl-engine`     | rmdl — execution is its subject (S-19, ridl-rt note §0)                                                         |
| E12.1, E12.3      | E4.5b, plus a contract-generic consumer                                                                         |
| E12.2, E12.4      | the `ridl-rt` row above — there is nothing to observe without it                                                |
| E12.5-E12.10      | a test plane with an owner                                                                                      |
| E13 (gateway)     | two encodings in one deployed system                                                                            |
| E8 parked stories | a regression the kept six did not catch                                                                         |

E13's trigger is a deployed system with two encodings, which is what the
consumer's v2 with a CAN-facing service would be — so the gateway is parked
rather than dismissed. The `ridl-engine` row is the one that changed character
between the two revisions: it was a document this plan owed, and it is now a
language this plan has deliberately deferred, which is a longer wait and a
clearer one.

## 8. The roadmap document itself

    S-15  SPLIT   ROADMAP.md into a forward plan and a landed record. The
                  file is 926 lines, of which the status narratives for E1,
                  E2 and E9 — what landed, in which PR, with which
                  amendment — are several hundred. That material is
                  history and belongs beside the archived epic plans; the
                  roadmap should be readable in one screen of epics plus
                  exit criteria.
    S-16  DROP    the one-issue-per-story mirror. ROADMAP.md records that
                  the tracker drifted from it twice, both times silently,
                  and that the repair cost 44 closures, 12 retitles and 56
                  new issues in a single pass. For a one-author repository
                  that is pure overhead: keep one issue per **epic**,
                  carrying its exit criterion, and let the document hold
                  the stories it is already the source of truth for.
    S-17  NARROW  the platform ladder to the three rungs with a named
                  target — desktop, embedded Android, QNX 7.1 — and move
                  rungs 5, 6 and 7 to a sentence saying no target requires
                  them. Rung 5 is already recorded as speculative; this
                  extends the same honesty to the two above it, which are
                  the ones that carry the store's `repr(C)` alternative.

S-16 is the only item here that touches process rather than scope, and it is
included because the reconciliation it removes is the same class of cost as the
stories: work that exists to keep two representations of one plan agreeing,
where one representation would do.

## 9. Open

**SR-X1 — whether Goal A is deferred or abandoned.** This note defers it and
says so. If the intent is a public language with outside users on a fixed
horizon, the cut is wrong in its ordering, though not in its sizing: the
argument that 220 person-weeks exceeds the author does not change, so something
still parks. The question is which goal names the survivor.

**SR-X2 — whether E11.8 survives the same test as E12.** The proto3 codec is
kept because the first runtime's remote leg is proto3, but that leg is a v2
concern in the consumer's programme and the v1 path is FlatBuffers in shared
memory. If the remote leg slips, E11.8 (L) parks with it and the release loses
4.5 weeks and its byte-level conformance story. Decide with the first runtime's
v1/v2 split, not here.

**SR-X3 — TypeScript's status. Mostly closed by S-27 and S-28.** The backend
stays in tree, generates types and interfaces, and reaches the codec through
wasm — ADR-0018 decision 6, unchanged. What remains open is narrow: whether it
stays a first-party backend at all once E4.5b exists, or becomes the second
plugin and leaves, which would make this repository Rust-only in fact as well as
in policy. Deciding it early is cheap; deciding it after E4.5b's protocol is
written against one in-tree non-Rust backend is not.

**SR-X4 — where the width floor decision lands, when it returns.** S-21 takes
A-3 out with E11.2, which defers the question rather than answering it.
ADR-0019's amendment recorded always-widest at 2.2x and 2.6x measured cost and
left `ridl-diff` as the sole guard for v0.1; the store cannot take that deal.
Whether the fix is a typl surface change or a store-layout constraint decides
which record it amends, and that is decided with the store.

**SR-X5 — the second consumer.** P-1 rests on the first runtime being the only
one. If a second appears inside the cut's horizon, the ordering is re-derived
rather than patched, because two consumers with different needs is the situation
that produced §2's tension in the first place.

**SR-X6 — one implementation validates the trait layer.** Stated in §4 and
repeated here because it is the price of S-19: `ridl-rt` was the second
implementation RA-02 and RA-03 need, and it is parked. The Kotlin binding is a
partial substitute across a language boundary, not a second runtime. If the
ports turn out to be specific to the first runtime, the discovery comes late.

**SR-X7 — whether SPIKE-1 is one note or two, now that it is the first
runtime's.** §4 argues the sans-IO activation question and rmdl's synchronous
step are one question seen from two ends. S-23 moved the note to the first
runtime, which needs the first half now and does not need the second at all. If
they are one question, the first runtime answers it and ridl inherits the answer
for free when rmdl lands; if they are two, the consumer's runtime answers a
smaller one and ridl still owes the rest. Either way this plan pays nothing,
which is what makes S-23 the cheap move — but it also means nobody is watching
whether the two halves agree.

**SR-X8 — closed by S-34, the wrong way round.** The question was where rsdl
v1's boundary against rmdl falls. The answer is that it does not fall anywhere
useful: §3.1 defines a component _by_ the model it applies, so the boundary runs
through the concept rather than around it. What is left open is narrower and
belongs to the descriptor — whether it needs any noun between `service` and
`machine` at all, or whether ridl §14's `service` already is the deployable
unit, as the consumer's vocabulary says it is. Answer that by writing the first
descriptor, not before writing it.

**SR-X9 — what the Kotlin plugin proves, and what it does not.** S-27 makes it
E4.5b's first user and SR-X6's second data point at once, which is convenient
enough to be worth doubting. Written by the same author, against the same mental
model, in the same week, it tests the protocol's mechanics and not its
generality. That is still worth having; it is not the third-party validation
E4.5b's exit criterion is worded for.

**SR-X10 — the descriptor's own evolution gate.** S-33's constraint puts the
descriptor in the IR so `ridl diff` classifies it, which is what stops it being
a second source of truth. Nobody has said what a _breaking_ deployment change
is. Moving a service to another machine breaks every interfacing rule derived
from the old placement and leaves the contract untouched, so the two existing
categories do not obviously fit. E4.5a's stability policy is where that gets
decided, or quietly missed.

**SR-X11 — where the descriptor lives, and whether it is one file.** A machine
map is a workspace fact; which interfaces a service publishes is closer to a
package fact. `ridl.toml` already splits standalone and workspace modes
(ADR-0002), so there is a precedent and a place, and also a chance to put the
same fact in two files. Decide with the schema, not after.

**SR-X12 — whether `machine` alone distinguishes the crossings. AUTOSAR answers
it.** Two guests on one SoC and two boxes on a bus are both "different machines"
and the interfacing rule is not the same, so something must carry what a machine
is hosted _by_. Adaptive's own definition supplies the shape — a Machine is
"quasi a virtualized ECU-HW" and one real **ECU** may host several — so the
second noun is the hardware under the machine, and the relation is containment,
not a new kind. What is still open is only whether this descriptor needs it in
its first version: with one SoC in the consumer's target vehicle, every crossing
is either intra-machine or hypervisor, and the distinction has no case to decide
yet. Add it when a second box appears, and keep the field name aligned with the
containment relation rather than inventing one.
