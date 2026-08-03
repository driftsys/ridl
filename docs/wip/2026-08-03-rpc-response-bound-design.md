# The RPC Response Bound, and the QoS Absorption Principle

| Field      | Value                                                                                                                                                                                    |
| ---------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Status     | design, for review — nothing ratified                                                                                                                                                    |
| Date       | 2026-08-03                                                                                                                                                                               |
| Origin     | evaluating ridl as the SSOT for a system bus whose backend carries QoS, under the requirement that a signal store and an event/command/query dispatcher be generatable from the IR alone |
| Scope      | one language change; one normative rule; three findings that need no syntax; two drift fixes                                                                                             |
| Supersedes | nothing — this is the first pass at ridl §17.5 open question 5                                                                                                                           |

A bare section reference — §9, §4.4, §17.5 — is to the **ridl Language
Reference**. References to this document are always marked _above_ or _below_.

## 1. The question, and the test used to answer it

Can ridl be the single source of truth for generating a signal store and an
event/command/query dispatcher — and if the backend carries QoS, must ridl grow
a QoS surface?

One test decides every candidate below:

> A fact belongs in the contract if a store or a dispatcher cannot be generated
> without it. A fact that reaches no generated code is not a contract term,
> whatever it is called.

This is the deletion test of general form §4.1 applied to policy rather than to
syntax. It is stricter than "is it a property of the service", and it is what
separates the one change this document proposes from the several it rejects.

## 2. Finding — ridl absorbs QoS, it does not exclude it

ridl §17.5 open question 5 records QoS as "deliberately absent from the contract
— timing bounds are the contract-level QoS, the rest is transport/deployment
(rsdl)", and asks whether that boundary survives the first DDS binding.

It survives. But "deliberately absent" understates what ridl does, and reading
it literally is what produces the recurring request for a QoS block. Mapping the
DDS policy set against ridl as it stands:

| DDS policy                     | ridl                                                                                   | verdict                         |
| ------------------------------ | -------------------------------------------------------------------------------------- | ------------------------------- |
| RELIABILITY                    | the interaction kind is the reliability class (§3)                                     | covered by construction         |
| DURABILITY (TRANSIENT_LOCAL)   | §4.4 last-value guarantee; events explicitly carry none (§5.1)                         | covered                         |
| DEADLINE                       | §9 `max` staleness bound                                                               | covered                         |
| LIVELINESS                     | §9 freshness SLO, §10.3 detection                                                      | covered                         |
| TIME_BASED_FILTER              | §9 `min` rate floor, derived as debounce                                               | covered                         |
| LIFESPAN                       | §9 `max`, derived as event TTL (§5.2)                                                  | covered                         |
| OWNERSHIP / OWNERSHIP_STRENGTH | §4.2 — exactly one owning provider per flow                                            | covered by construction         |
| DESTINATION_ORDER              | §3.1 envelope timestamp and per-channel sequence number                                | covered                         |
| RESOURCE_LIMITS (payload)      | typl bounded collections and length bounds                                             | covered, in the type system     |
| PRESENTATION (coherent_access) | implicit at the provider — §4 below; struct idiom (§17.3) for the delivery case        | covered, see §4 below           |
| HISTORY (KEEP_LAST N)          | §4 signal is latest-value; §5.1 rules out event replay                                 | out of scope, deliberately      |
| DURABILITY (PERSISTENT)        | `persist`, reserved by ADR-0008 decision 3                                             | reserved                        |
| LATENCY_BUDGET                 | a delivery-delay hint permitting batching; the spec is explicit it is not a commitment | no analogue needed, none wanted |
| **RPC reply timeout**          | **absent on `command` and `query`** — see the note below                               | **gap — §3 below**              |

Ten of the thirteen DDS policies listed are covered, several more strongly than
DDS provides them; of the remaining three, one is deliberately out of scope, one
is already reserved, and one wants no analogue. The table is the
contract-bearing subset — PARTITION, TRANSPORT_PRIORITY, the writer/reader
lifecycle policies and the reader-side content filters are omitted as deployment
or reader-local concerns, which is the same judgement §17.5 makes and this note
does not revisit.

The last row is not a DDS QoS policy, and an earlier draft of this note wrongly
labelled it LATENCY_BUDGET. Core DDS has no per-call timeout: DEADLINE is an
update-rate commitment on a data stream, which is ridl's `max` and is already
covered, and LATENCY_BUDGET is a batching hint the specification explicitly
declines to make a commitment. The reply timeout lives in **DDS-RPC**, and its
relatives are gRPC's deadline, SOME/IP's configured timeout, and AIDL's
transaction timeout.

**The principle, stated for the first time here:**

> ridl expresses QoS as **semantic obligation**, never as a transport knob. A
> binding maps each obligation onto its native QoS; a transport lacking the
> mechanism either satisfies the obligation by construction or fails at bind
> time.

This is what preserves the §1.1 transport-neutrality claim. The same contract
binds to SOME/IP, proto/gRPC, DBC, AIDL, MQTT, or an in-process broker without
carrying any of their vocabulary — because it carries none of their mechanisms,
only the outcomes a consumer may rely on. A QoS block would invert that: it
would put twenty-two orthogonal DDS degrees of freedom into a contract that must
also bind to CAN, which Appendix F already names as the lesson DDS teaches
against.

The wording of §17.5 should change from exclusion to absorption. The boundary is
correct; the description of it is not.

## 3. The change — RPC bounds, and a response bound in particular

### 3.1 Why this one is real

It is the single row where ridl is inconsistent with itself. §9 gives pub/sub a
contract-level view of transport health, and §10.3 says so in as many words:
"the freshness machinery (§9) is the contract-level view of transport health for
pub/sub." RPC gets nothing. A `signal` carries a staleness bound that defines
"late" for the observability plane; a `query` that must answer within 50 ms has
nowhere to say so, and every caller invents its own timeout.

Appendix F currently records the gRPC deadline as "≈ relocated — contract owns
freshness (§9), calls own their deadlines". That row is about gRPC's deadline,
which is a per-call caller parameter with no declared default and no
provider-side meaning. A declared per-interaction bound is a different object:
it is a provider obligation, and it is the same species of statement as a
signal's staleness bound.

**This is a deliberate divergence from the prior art, not a copy of it.** gRPC's
deadline bounds the whole call but is a per-call client parameter with no IDL
syntax — a `.proto` service definition cannot declare it. SOME/IP defines no
timeout field at all; client timeouts are configuration, and where AUTOSAR
declares timing it does so as Timing Extensions (TIMEX) constraints over event
chains in ARXML, separate from the service interface. So the industry precedent
is consistent: the interface language does not carry it, and where it is
declared it is a separate timing artifact. ridl carrying it in the contract is a
divergence that the ADR should argue for rather than assume — the argument being
§9, which already put the pub/sub half of exactly this in the contract.

### 3.2 Surface

No grammar change. `family.ungram` already carries `Timing?` on both
`CommandDef` and `QueryDef`, because ridl §9.2 deliberately admits the
annotation on every interaction kind "so that the narrowing is a semantic rule
with a semantic message; it is not a parse error". The following parses today
and is rejected by the checker:

```ridl
query   getAverageSpeed(window: Duration): Speed @[..100ms]
command setGear(position: GearPosition) @[20ms..50ms]
```

The range form is the only admitted form on an RPC, and the half-open spellings
§9 already defines carry the two partial cases — `@[..100ms]` is a response
bound with no throttle, `@[20ms..]` a throttle with no bound. With an attribute
block the annotation follows it, per the shipped production order
`AttrBlock? Timing?`:

```ridl
command setRange(min: Speed, max: Speed) [
  require min < max
] @[..50ms]
```

### 3.3 Semantics — the same two bounds, derived per kind

§9 already fixes one generic meaning for the two bounds and derives the per-kind
consequence from the declaring keyword. RPC extends that table rather than
carving an exception out of it:

> **`min` = rate floor** — the minimum interval between issues. **`max` =
> staleness bound** — the maximum age before the interaction is late.

| Bound                   | `signal` (state) | `event` (occurrence) | `command` / `query` (call)                            |
| ----------------------- | ---------------- | -------------------- | ----------------------------------------------------- |
| `min` — rate floor      | debounce         | throttle             | **call throttle** — the caller must not call faster   |
| `max` — staleness bound | refresh ceiling  | TTL                  | **response bound** — the provider must respond within |

So `@[20ms..50ms]` on a query reads exactly as it reads everywhere else: not
faster than 20 ms, not later than 50 ms.

**`min` on an RPC constrains the caller, not the provider.** That is not an
inconsistency — §9's `min` always constrains whoever initiates, and on an RPC
the initiator is the consumer. It is enforceable at the provider's admission
point, and it is what a rate-limiting binding already implements. It is also the
one place where the asymmetry has a consequence; see §3.6 below.

**`max` is a response bound, not a delivery bound.** What responding means is
derived per kind:

| Kind      | Responding is                                                                      |
| --------- | ---------------------------------------------------------------------------------- |
| `query`   | the reply                                                                          |
| `command` | acceptance — the acknowledgment of §6.1 — since §6.1 promises no execution outcome |

A pure _delivery_ bound was considered and rejected. Delivery latency is a
property of the link, not of the interaction: every interaction crossing one
connection has the same delivery leg, so a per-interaction delivery bound would
declare the same number on every declaration in the package and carry no
information — it fails the test in §1 above. Response time is what varies per
interaction, because what the provider does varies: `getAverageSpeed` over a
one-second window and `calibrate` on an axle differ by orders of magnitude. It
is the only per-interaction number available, which is why gRPC, DDS-RPC, and
AIDL all bound the whole call rather than a delivery leg.

The `command` row is the narrower one, and the spec should say so rather than
gloss it: acceptance covers admission and queueing at the provider but **not**
execution. Bounding execution is not available here, because §6.1 rules that the
contract promises no outcome.

That a command's bound touches Stratum 3 is not a new concession. §10.3 already
rules that "the freshness machinery (§9) is the contract-level view of transport
health for pub/sub"; a response bound is that same view for RPC. What stays
undeclared is outcome, exactly as §6.1 has it.

**Warned, but never defaulted.** An RPC without a declared response bound draws
a **warning** (RIDL-112, §3.4 below), and an active profile may escalate that to
an error — the same two-step §9.1 already gives an untimed signal or event, so a
safety-graded package can require every RPC to state its obligation.

What it does **not** get is a default. §9.1 defaults an untimed signal or event
but never defaults a strict period, on the grounds that an isochronous rate is
always an explicit engineering decision. A response bound is the stronger case
of the same argument, for two reasons. There is no plausible generic value —
`getAverageSpeed` and `calibrate` differ by orders of magnitude because what the
provider does differs — so any default would be arbitrary. And a defaulted
response bound is worse than none at all: it is a provider obligation callers
size their own timeouts against, so inventing one manufactures a promise nobody
made. Absent therefore means undeclared in the IR, and this change stays clear
of the "changing the configured default is a contract change" machinery.

The warning is about `max` specifically, not about the annotation. `@[20ms..]`
declares a throttle and no response bound, so it warns exactly as a bare
undecorated RPC does. A missing `min` draws nothing — an unbounded call rate is
the sensible default and is what every RPC has today.

**Strict periodic stays signal-only.** `@Xms` on a `command` or `query` is an
error. A caller is not isochronous by contract, and §9 already admits the
strict-periodic mode on `signal` alone.

### 3.4 Diagnostics

One code is minted and two existing rules move, one in each direction.

RIDL-112 is **minted**: _`command` or `query` with no declared response bound_ —
severity warning, escalated to error where the active profile requires it. It is
the RPC counterpart of RIDL-100, and it is deliberately not RIDL-100 itself,
whose text turns on a default having been applied — which is exactly what an RPC
does not get (§3.3 above).

RIDL-106 **narrows**. Its current rule covers a timing annotation on `command`,
`query`, and `fixed`, plus an attribute block on `fixed`. It keeps `fixed` in
both halves and drops the two RPC kinds. Its message already carries a per-kind
`because` arm, so the text falls out.

RIDL-103 **widens**, from "strict periodic `@Xms` on `event`" to "strict
periodic `@Xms` on a kind other than `signal`". It is the same rule — the
isochronous mode belongs to state alone — now stated over the three kinds it
excludes instead of one. Widening a rule to more kinds is neither a renumber nor
a reuse, so it stays inside typl §16's lifecycle discipline, and it is the exact
mirror of RIDL-106's narrowing.

**RIDL-111 is not available**, so RIDL-112 is the first free code in the band:
ADR-0008 decision 21 allocated RIDL-111 to the interface-used-as-a-type error,
alongside RIDL-142 for the service-name-segment error, and decision 13's ledger
counts both. An earlier draft of this note proposed RIDL-112 for a different
purpose — rejecting the range form on an RPC — which admitting both bounds
removed the need for; the number is reused here for the missing-bound warning,
not carried over.

RIDL-101 (`X > Y`), RIDL-102 (zero or negative duration), and RIDL-108
(`@[X..X]`, a degenerate range) all apply to an RPC unchanged.

### 3.5 IR

`Timing` is reused unchanged, because with both bounds admitted all four of its
fields carry meaning for an RPC: `mode` is always `Range`, `min_us` is the call
throttle, `max_us` the response bound, and `default_applied` always false, since
RPC bounds are never defaulted.

```protobuf
message CommandDef {
  repeated Param params = 1;
  repeated Contract contracts = 2;
  // Declared bounds (ridl §9): min_us is the call throttle, max_us the
  // response bound. Absent when undeclared — never defaulted.
  Timing timing = 3;
}

message QueryDef {
  repeated Param params = 1;
  ReturnType return_type = 2;
  repeated Contract contracts = 3;
  Timing timing = 4;
}
```

Both field numbers are free. This supersedes an earlier draft of this note,
which proposed dedicated `budget_us` fields on the grounds that three of
`Timing`'s four fields were meaningless for a scalar bound. With both bounds
admitted they are all meaningful, and reusing `Timing` keeps one representation
of a timing bound in the IR rather than two.

### 3.6 diff

A **new `Category::RpcBoundChanged`**, not a reuse of `TimingChanged`. The
direction rule does not transfer, and that is what forces the separate category.

`classify.rs` states the existing convention: a guarantee "strengthens when
`min` rises or `max` falls and weakens when either moves the other way", and "a
bound added or removed … is breaking in both directions." That reads both bounds
from the consumer's frame, which is right while both constrain the provider —
true on a `signal` and on an `event`.

On an RPC, `min` constrains the **consumer**. Raising it withdraws a call rate
the caller was entitled to use, so its direction inverts:

| Change                 | `signal` / `event` | `command` / `query`                                   |
| ---------------------- | ------------------ | ----------------------------------------------------- |
| `min` raised           | compatible         | **breaking** — the caller may no longer call as often |
| `min` lowered          | breaking           | **compatible** — the caller is less constrained       |
| `max` raised           | breaking           | breaking — a weaker provider promise                  |
| `max` lowered          | compatible         | compatible — a stronger provider promise              |
| bound added or removed | breaking both ways | breaking both ways                                    |

**Why a separate category rather than a kind-aware arm inside `timing()`.** An
earlier draft of this note reused `TimingChanged` and added a branch on the
interaction kind, noting as a risk that the three matches over `Category` which
deny `clippy::wildcard_enum_match_arm` would not fire for a reused variant, so
only tests would catch a missed branch.

ADR-0012 decision 9 settles that the other way. Its rule is that an unclassified
change is "classified **breaking**, never compatible" — reporting compatible on
something the classifier does not actually understand is the failure mode to
design out. A missed branch inside `timing()` fails in exactly that direction:
it silently inherits the signal rule and calls a raised RPC `min`
**compatible**, when it is breaking. A distinct variant fails closed instead,
because the deny lint turns a missing arm into a compile error rather than a
wrong verdict.

The cost is one more category in a surface that already carries twenty-one, and
the benefit is that the one part of this change no compiler could catch becomes
the one part no compiler can miss.

### 3.7 Backends

Both backends already emit `u64` microsecond constants for signal and event
timing, and both already return a `GenerateError` rather than truncate a bound
that is not a whole number of microseconds. An RPC's bounds are the same two
constants they already emit for a signal or an event, now present on two more
kinds, in both Rust and TypeScript.

## 4. The normative rule — coherence is implicit, and the interface is the unit

### 4.1 The rule

> The signals of one **provided interface** are published coherently: a consumer
> reading two or more of them observes values the provider held simultaneously.
> The group's identity is the **interface name** where a service names a shape,
> and the **service name** where the shape is inline.

Stating it at the interface grain rather than the service grain costs nothing
today and is the formulation that survives. `ServiceDef` currently admits
exactly one named shape or one inline shape, so the two grains pick out the same
set at 1:1; if a service is ever allowed to carry several interfaces — §11 open
question 1 below — the rule already scales without amendment, and the interface
becomes the grouping construct §17.3 was looking for at no cost in new syntax.

Identity matters as much as grouping, because a generator needs a stable name to
emit the block under, stamp a schema hash on, and place a region by. Both halves
supply one. §14.5 makes the dotted service name unique across the system
(RIDL-140) and addresses members as `service.member`, so a group is exactly an
address prefix; and an inline shape carries `Interface.name == ""` in the IR,
which is precisely the case where the service name is the only name available.
Nothing further has to be declared or invented.

A component providing a bare `interface` with no service still _produces_
coherent values, since the argument just below depends only on there being one
provider. But it has no global address, so it is not a nameable coherence group
and no downstream artifact can key on it. That asymmetry is a reason to publish
a service, not a gap in the rule.

### 4.2 Why it is implicit rather than declared

Three rules ridl and rmdl already state produce it:

- §4.2 gives every flow exactly one owning provider.
- A provider computes its outputs in one step (rmdl's topological schedule).
- A service is the published unit realized by one provider.

So the values a provider publishes in one tick are a simultaneous state by
construction. Signals publishing at different rates do not break this: in a
given tick some cells are written and others are not, but every value present
came from the same step, so the observed set remains a state that existed.

Declaring `coherent` would therefore declare a consequence of how the platform
executes — which is what the general form §4.1 deletion test exists to reject.
It would also be a lie in the one place it appeared to help: marking one
interface coherent would imply the others are not, when in fact they all are.

### 4.3 The consequence that is not currently written down

Production coherence and **delivery** coherence are different. The provider
producing a coherent set is implicit; whether a consumer _observes_ it
coherently depends on the binding.

| Transport        | How coherence is preserved                            |
| ---------------- | ----------------------------------------------------- |
| shared memory    | one versioned block per group, one generation counter |
| DDS              | GROUP-scope PRESENTATION with `coherent_access`       |
| SOME/IP          | one notifier per field; preserved only within a field |
| static bus (CAN) | preserved only within one frame; not across frames    |

This makes coherence a **demand on every transport mapping**, in the same sense
§4.4's last-value guarantee already is — and it is a demand no binding author
can currently discover, because nothing in the reference states it.

Where a consumer needs the guarantee to survive an arbitrary binding, ridl
already has the answer, and §17.3 open question 3 records it: make the values
one struct, so they are one payload on one channel and atomic everywhere. This
document confirms that provisional "probably yes" and supplies the reasoning.

Where a binding cannot preserve it, that is a deploy-time constraint with exact
precedent: §14.5 already rules that "a service deployed statically realizes only
its `signal`/`event` members; its control API requires the discovered posture
(enforced at deploy time, rsdl §8)." Coherence is the second instance of that
pattern, and rsdl already owns transport feasibility (E6.8, RSDL-801/803).

### 4.4 What follows — the interface is the generation unit

Three rules converge on one boundary, and that convergence is what makes §1's
question answerable at all:

- **coherence** groups the signals of one provided interface (§4.1 above);
- **ordinals** run in a single sequence across all the interactions of an
  interface, regardless of kind (§11);
- **identity** is the interface name, or the service name for an inline shape
  (§4.1 above), and a binding keeps ordinal spaces apart by that name (§11.1
  below).

So a provided interface yields exactly two generated artifacts:

| From one provided interface | What is generated                                                                                                                                                                                                                                        |
| --------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| one **store**               | its signals, laid out as one coherent block behind a single generation counter; each cell seeded with the payload's init value (§4.4) and carrying provenance and the envelope (§3.1), evaluated against its own `max` staleness bound                   |
| one **dispatcher**          | its events, commands, and queries, routed by the interface's single ordinal sequence; per-event ring depth derived as `ceil(max / min)` (§5.1 below), typl constraints checked before the handler, and the response bound applied around it (§3.3 above) |

The **service** is the addressing unit, not the generation unit: it names the
interfaces and gives their members global addresses (§14.5). A service composing
three interfaces therefore generates three stores and three dispatchers under
one logical name — which is exactly what makes the composition worth having,
since each interface keeps its own ordinal space, its own coherence guarantee,
and its own wire identity, independent of what it is composed with.

## 5. Findings that need no syntax

### 5.1 Event ring depth is derivable, not declarable

A generator emitting an event dispatcher must size a queue. That looked like a
missing contract term; it is not.

An event's `min` is the throttle — the provider must not raise occurrences
faster than `min` (§5.2). Its `max` is the TTL — an occurrence older than `max`
is discarded by the binding (§5.2). The number of occurrences that can be alive
at once is therefore bounded:

```text
depth = ceil(max / min)
```

`@[50ms..500ms]` can never have more than ten live occurrences, because the
eleventh cannot exist before the first has expired. A deeper ring buffers
occurrences that are already stale by contract.

This is a derived IR fact, the same move ridl already makes for wire widths from
ranges and for resolved timing defaults. It fails only on a half-open range
(`@[20ms..]`, `@[..500ms]`), where one bound is unset and the product is
unbounded — and §9.1's defaults mean both bounds are present whenever timing was
not written explicitly.

### 5.2 History and replay stay out of the contract

§4 makes a signal latest-value only: "the latest sample is the truth;
intermediate samples may be missed." §5.1 rules out event replay in as many
words: "an occurrence that happened before you were listening did not happen _to
you_. Replaying history is the test/observability plane's job, not the
contract's."

The strongest case put for contract-level history was crash forensics —
replaying the events preceding a fault. That is the observability plane by
definition: a forensic reader reconstructing a trace is not a contract consumer
acting on occurrences. The envelope already carries what such a trace needs
(§3.1's sender timestamp and per-channel sequence number, which concept note
§9.3 uses for deterministic replay). A retention depth in the contract would put
a forensics knob on every event declaration, and the depth wanted for crash
analysis bears no relation to what any consumer needs.

A consumer that genuinely needs the last N samples of a state value can already
say so without new syntax — a lookback query whose bounded return type carries
the depth:

```ridl
signal currentSpeed  : Speed @10ms
query  recentSpeed() : [Speed; 1..8]
```

The `1..8` bound is already an IR fact, so a generator reading that package
knows the depth. This keeps `signal` meaning exactly what §4 says.

## 6. Deliberately out of scope

Each of these was considered and rejected against the §1 test.

| Candidate                  | Why not                                                                                                                                                                                                                                                                                              |
| -------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `idempotent`               | reaches no generated store, dispatcher, or handler. §6.1's ack and sequence numbers already give duplicate suppression; what remains is an unverifiable assertion, and `@labels` already carries review metadata                                                                                     |
| `history N` on `signal`    | contradicts §4's latest-value definition; the lookback query in §5.2 expresses the same requirement with existing vocabulary                                                                                                                                                                         |
| ring depth as syntax       | derivable — §5.1 above                                                                                                                                                                                                                                                                               |
| `coherent` as an attribute | implicit — §4 above                                                                                                                                                                                                                                                                                  |
| ordering key               | needs a grammar widening (`AttrValue` admits no camelCase name) and has no consumer yet; revisit with evidence                                                                                                                                                                                       |
| serializing the expr tree  | E5.1 owns the restructuring, and the ROADMAP records that the corpus does not yet exercise five of the subset's operators — restructuring ahead of its regression net. `parse_contract_expr` is already public in `ridl-sem`, so a Rust generator can recover the tree from the canonical text today |
| window, permits, retry     | per-deployment sizing, invisible to any peer; rsdl's territory (E6), sidecar until then                                                                                                                                                                                                              |

**The call throttle and the in-flight window are not the same thing**, and the
last row should not be read as contradicting §3.3 above. A throttle is a
declared _rate_ the provider commits to accepting and the caller commits to not
exceeding — a two-sided obligation, identical for every consumer, and the same
species of statement as a signal's `min`. An in-flight window is _concurrency_
sizing, chosen per consumer instance: a high-rate consumer and an occasional
diagnostic client have different needs and no reason to agree on one. One is a
contract term because both peers must know it; the other is local admission
control that neither peer can observe in the other.

## 7. Alternatives considered

**A QoS block on the interaction.** Rejected: it imports transport vocabulary
into a contract that must also bind to transports lacking it, and Appendix F
already records the DDS lesson that "22 orthogonal QoS policies on one topic is
too many degrees of freedom for a contract." §2's absorption principle is the
alternative that keeps §1.1 true.

**The bound as an attribute — `[ deadline = 50ms ]`.** Rejected: `@` is the
family's timing sigil (general form R4, "`@` is time"), a response bound is a
timing bound, and the grammar already admits `@` in that position. Spelling one
timing bound with `@` and another with an attribute key would split one concept
across two forms.

**Budget on `query` only.** Rejected: it leaves a command's acceptance unbounded
for no reason other than squeamishness about Stratum 3, which §10.3 already
crosses for pub/sub. See §3.3 above.

**A sidecar policy file keyed by `service.member`.** Still correct for what §6
moves out, and it is what rsdl replaces at E6. It is the wrong home for the
response bound specifically, because it is the one value a cross-check needs
against the contract (see §9 below).

## 8. Documents to amend

| Document        | Change                                                                                                                   |
| --------------- | ------------------------------------------------------------------------------------------------------------------------ |
| ridl §9         | the per-kind bound table gains its RPC column; a subsection fixes the response bound and its per-kind derivation         |
| ridl §9.2       | "Timing belongs to `signal` and `event`" widens to admit `command`/`query` in the range form; `fixed` still carries none |
| ridl §16.1      | RIDL-106 narrows to `fixed`; RIDL-103 widens to every kind but `signal`; RIDL-112 minted for the missing response bound  |
| ridl §14        | the coherence rule (§4.1 above) as normative prose, beside the service definition it keys on                             |
| ridl §17.3 q3   | closed — the struct idiom confirmed, with the reasoning                                                                  |
| ridl §17.5 q5   | answered — replaced by the absorption principle and the coverage table                                                   |
| ridl Appendix B | rows for the response bound and for coherence per transport                                                              |
| ridl Appendix F | gRPC-deadline row flips from "≈ relocated" to in-contract, with the per-call override staying Stratum 3                  |
| general form R5 | sentence order contradicts the shipped grammar — see §10 below                                                           |
| ADR-0013 (new)  | the absorption principle, the RPC bounds, the coherence rule, and the kind-aware diff direction                          |

## 9. What the response bound unlocks

One concrete payoff beyond consistency. A retry schedule and a latency bound
currently live in different files with no defined relationship, so nothing can
check that the worst-case schedule fits inside the bound — meaning the last
attempts of a retry policy may be unreachable and the policy silently lies. With
the response bound in the contract, that becomes a build-time assertion in
whatever consumes the IR alongside a deployment file. It also generalizes "do
not start a doomed attempt" from a runtime guard into something checkable before
the code ships.

## 10. Drift this surfaced

**General form R5 contradicts the shipped grammar.** R5 fixes the postfix order
as `… wire W → @timing → during S → [ attrs ] → = value → { body }`, putting
timing _before_ attributes and before an init value. `family.ungram` has
`AttrBlock? Timing?` on `CommandDef` and `QueryDef`, and `InitValue? Timing?` on
`SignalDef` — timing last in both. ridl §4.4's own example follows the grammar
(`signal targetSpeed : Speed = SPEED_LIMIT_EU @[20ms..500ms]`). The general form
is a pre-ADR working spec and the shipped grammar is the ratified behaviour, so
R5 is the stale one and should be corrected to put `@timing` last.

**`InterfaceDef` and `ServiceDef` take no `AttrBlock`,** while general form §4.7
already writes `interface CruiseControl [ labels = (SIL_2, SEC_2, PRIVATE) ]`.
Not blocking for this design — nothing here needs an interface-level attribute —
but it is the same grammar change the deferred `labels`/`deprecated` promotion
needs, and it is worth recording that both wait on one edit.

## 11. Decided here, and still open

### 11.1 Decided — a service may carry more than one interface

§4.1 above states the coherence group as a _provided interface_, which today
cannot be told apart from "the service": `ServiceDef` admits exactly one named
shape or one inline shape, so the two grains coincide at 1:1. **That restriction
should be lifted**, and two consequences are settled with it.

The case for it is that it answers two recorded open questions at once. It makes
the interface the grouping construct §17.3 wanted, at no cost in new syntax. And
it is a better answer to §17.2's interaction-set-reuse question than the
candidate recorded there: compile-time mixins _flatten_, so folding one shared
block into three interfaces gives its interactions three different ordinal sets
and editing it renumbers all three, whereas composition leaves each interface's
ordinal space intact and independent of what it is composed with.

**Addressing stays flat.** Members remain `service.member`, as §14.5 has it, and
a member name duplicated across a service's interfaces is a new compile error.
This keeps the flat global address the specification calls the point — what
makes a dotted member name an unambiguous system-wide address — and leaves every
address written today valid. The accepted cost is that two independently written
interfaces sharing a member name cannot be composed without renaming one.

**Ordinals stay per-interface, and the binding separates the spaces by interface
name.** Renumbering across a service was rejected: an interface's wire identity
would then depend on what else the service happens to carry, which is the exact
coupling §14.1 rejected inheritance to avoid. Appendix B already points the
other way, mapping a SOME/IP eventgroup to an interface, so a multi-interface
service maps to several transport-level groupings under one logical name. Keying
on the interface **name** rather than on its position in the list also makes
reordering the list cosmetic, so `ridl-diff` sees nothing — the property to
want.

This still needs its own design pass rather than riding on this one: the grammar
change to `ServiceDef`, the duplicate-member diagnostic, how RIDL-141 and
RIDL-143 apply per element, and the `ridl-diff` categories for adding and
removing an interface are all unaddressed here.

### 11.2 Decided — a package may require a response bound

An RPC with no declared response bound warns, and an active profile may escalate
the warning to an error. §3.3 and §3.4 above carry the decision; RIDL-112 is the
code. What an RPC does not get is a default — see §3.3 above for why a
manufactured provider obligation is worse than none.

### 11.3 Decided — a downstream tool checks that a retry schedule fits the bound

§9 above notes the check the response bound makes possible: three attempts at 8
ms and 200 ms cannot fit inside a 50 ms bound, so the later attempts can never
run and the policy is lying about them.

Running it means reading the contract's IR _and_ a deployment file together.
**The check belongs to a downstream tool, not to `ridlc`.** ADR-0008 decision 9
keeps `ridlc` a pure source-to-IR function precisely because that is the
smallest surface to qualify under ISO 26262, and a cross-artifact rule that
reads a deployment file would enlarge it for the sake of one check.

The family already resolves this shape the same way: §14.5 enforces at deploy
time, not at compile time, that "a service deployed statically realizes only its
`signal`/`event` members; its control API requires the discovered posture". A
rule needing both the contract and the topology runs where both are in hand,
which is one layer up.
