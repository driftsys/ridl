# rsdl Language Reference

**Reactive System Description Language** — the architecture layer of the RIDL
family: components, services, systems, and deployment. rsdl situates pure
reactions (rmdl) on the real signals of ridl services and assembles them into a
running, distributed reactive system.

Version: 0.1.0 — Draft

> **Provenance.** rsdl is the apex of the family lattice (concept note §2) — the
> only layer that references all four below it, and the one layer that can never
> stand alone. It composes **by import, never by inclusion**. Design guide: the
> [Reactive Manifesto](https://www.reactivemanifesto.org/) — whose
> _message-driven_ principle is the family's transport-neutrality, and whose
> _responsive / resilient / elastic_ pillars are what the architecture layer
> delivers. This is **one grammar with two regions**: _logical composition_
> (components, wiring — location-transparent, transport-free) and _physical
> deployment_ (targets, placement, posture, bundles). Doc comments (`/** … */`,
> `///`) are CommonMark, as everywhere in the family.

---

## Table of Contents

1. [Scope and Position in the Family](#1-scope-and-position-in-the-family)
2. [Composition and Deployment — Two Regions of One Grammar](#2-composition-and-deployment--two-regions-of-one-grammar)
3. [The Component](#3-the-component)
4. [Wiring — Application Notation](#4-wiring--application-notation)
5. [Providing and Requiring Services](#5-providing-and-requiring-services)
6. [Systems](#6-systems)
7. [Targets and Placement](#7-targets-and-placement)
8. [Transport and Posture](#8-transport-and-posture)
9. [Bundles](#9-bundles)
10. [Resilience — Reserved](#10-resilience--reserved)
11. [Conventions](#11-conventions)
12. [Diagnostics](#12-diagnostics)
13. [Open Questions](#13-open-questions)

- [Appendix A — Full Example](#appendix-a--full-example)
- [Appendix B — Deployment Targets](#appendix-b--deployment-targets)
- [Appendix C — Formal Grammar (EBNF)](#appendix-c--formal-grammar-ebnf)
- [Appendix D — Prior Art Survey](#appendix-d--prior-art-survey)
- [Appendix E — Coverage Analysis: Architecture & SDV Frameworks](#appendix-e--coverage-analysis-architecture--sdv-frameworks)
- [Appendix F — Glossary](#appendix-f--glossary)

---

## 1. Scope and Position in the Family

### 1.1 What rsdl is

rsdl answers: _how are reactions situated on real signals and assembled into a
system, and where does it run?_ It is the single source of truth for the
**architecture** — which components exist, which reactions they run, which
services they provide and require, how signals are wired, onto which targets
they are placed, and how they are bundled.

The organizing principle is **location transparency** (Reactive Manifesto,
_message-driven_): a component never knows whether the thing it talks to is in
the same process, another process, another ECU, or across the vehicle network.
rsdl resolves that — it places location-transparent components and lets the
transport (and communication _posture_, §8) of every connection be **derived**
from where the endpoints land. This is the family's transport-neutrality
extended from the single wire to the whole deployment.

### 1.2 The three roles, and what each layer owns

The family's cleanest result is that behaviour, contract, and situation are
three separate things:

| Concept                 | Layer | Is                                                                                                                   |
| ----------------------- | ----- | -------------------------------------------------------------------------------------------------------------------- |
| **model**               | rmdl  | a **pure reaction** `(O,S) = M(I,S)` — abstract flows, contract-blind                                                |
| **interface / service** | ridl  | the **contract** — a shape, and its global published declaration (ridl §14)                                          |
| **component**           | rsdl  | a reaction **situated in the system** — its inputs and outputs are _real signals_, and it provides/requires services |

rmdl computes; ridl declares contracts; rsdl connects. A component is what turns
a contract-blind reaction into system-connected behaviour, by wiring the
reaction's abstract flows to real signals and declaring which services it
provides and requires. `model : component :: values : signals` — both are
reactions, the model over abstract value-flows, the component over the system's
real signals.

### 1.3 The component boundary is the sync/async wall

The family's sync/async wall (concept note §5, rmdl §1.3) lands exactly at the
component boundary: **inside a component, models compose into one synchronous
reaction**; **between components, signals cross the asynchronous broker** (ridl
message semantics). A component is therefore the unit of: **one reaction** (the
granularity of a synchronous step), **failure containment** (a component fault
cannot corrupt another), and **independent deployment** (the smallest thing
placed, replaced, or bundled).

---

## 2. Composition and Deployment — Two Regions of One Grammar

rsdl has two declaration kinds, and the separation between them is load-bearing.

**Logical composition** — `component` and `system` declarations. The _what runs
and what connects_: reactions situated on signals, services provided and
required. Location-transparent, transport-free, hardware-free.

**Physical deployment** — `deployment` declarations, each `for` a named system.
The _where it runs and how it ships_: targets, placement, transport posture,
bundles. A deployment _references_ a composition and maps it; it never redefines
it.

Both are `.rsdl` grammar (the concept note's "manifest first" is collapsed
straight to grammar — settled). Only ADR-0002's package management (`ridl.toml`:
imports, versions, lockfile) stays config, as Cargo separates `Cargo.toml` from
source. **Package-manager config is TOML; architecture is a language.**

**The payoff — one composition, many deployments.** A single `system` drives
many `deployment`s: production, simulation, HIL, single-ECU vs distributed,
per-variant. This is the transport-transparency dividend, and it makes the test
plane (concept note §9.2) fall out for free — **a test topology is just another
`deployment`** with selected instances swapped for injectors or the reference
oracle. It is the AUTOSAR VFB-vs-ECU-extract and AADL component-vs-binding
split, made a first-class language distinction.

```
system Vehicle { … }                          ← logical composition (one)
     ├── deployment Production for Vehicle { … }
     ├── deployment Bench      for Vehicle { … }
     └── deployment Hil        for Vehicle { … }   (gw = injector)
```

---

## 3. The Component

### 3.1 The component is a situated reaction

A **component** has a signature of **real signals** (kinded, per ridl:
`signal`/`event`/`command`/`query`), and a body that situates one or more
reactions on those signals. Its boundary is declared with `provides` /
`requires`; its body is **application notation** (§4):

```
component SpeedFilter {
  requires signal raw : Speed
  provides signal filtered : Speed = LowPass(raw, 100ms) @[..100ms]
}
```

That is a complete component: a required input signal, and a provided output
signal defined by applying the `LowPass` reaction to it. No contract is named,
because nothing reuses this shape — the boundary is **inline** (§5.1).

### 3.2 Leaf and composite

A component's body applies either **models** (rmdl reactions — the component is
a synchronous _leaf_, one reaction) or **sub-components** (the component is a
_composite_, async between children). Both use the same application notation;
the compiler distinguishes by what is applied. This is the recursion that spans
leaf behaviour to the whole vehicle: a subsystem is a component, the system is a
component (§6), a system-of-systems is a component. AADL/SysML lineage.

- applying a **model** joins this component's one synchronous reaction (models
  compose sync, rmdl §5.5)
- applying a **sub-component** introduces an async child (a separate reaction
  across the broker)

### 3.3 Boundaries scale with reuse

A component's boundary is declared at whatever grain fits (ridl §14):

- **inline members** (`provides signal x : T`, `requires event e : U`) —
  private, one-off, not reused.
- a named **interface** (`provides CruiseControl`) — a reusable shape shared
  with other components/consumers.
- a **service** (`provides veh.adas.cruise`) — a global, published, SSOT-catalog
  contract (§5).

Promotion is mechanical: inline members are identical in syntax to interface
members, so extracting a reusable `interface` (ridl) is a cut-paste plus
dropping the `provides`/`requires` verb. You pay the naming cost only when reuse
earns it.

---

## 4. Wiring — Application Notation

A component body is a flat, top-to-bottom dataflow: `requires` are the input
holes, `provides … = …` are the defined outputs, `let` are intermediates, and
reactions are applied as functions of signals.

### 4.1 The forms

```
requires signal speed : Speed                          // an input hole (wired from outside)

let mid = SomeFilter(speed)                            // intermediate — anonymous instance

provides signal out : Speed = Controller(mid) @[..50ms] // provided output, defined + timed

(engaged, target) = CruiseController(current, brake, lever)   // multi-output, destructured

cruise = CruiseController(current, brake, lever)       // named instance; cruise.engaged, cruise.target
```

- **Application** `Reaction(args)` applies a model or sub-component to signals,
  producing signals. Each application site is an **instance** owning its state
  (rmdl §5.5).
- **Fused declaration+definition**: `provides signal x [: T] [= expr] [@timing]`
  — name, optional explicit type (keep it explicit at anything reused/published;
  inference from the RHS is fine for private internals), optional defining
  expression, optional timing. `= expr` and the type are each optional, covering
  fused, body-defined, and interface-grouped cases uniformly.
- **Naming an instance** (bare name on the LHS) binds the whole instance for
  field access and for placement (`place cruise on …`, §7). **Destructuring**
  (tuple LHS) binds the outputs directly; the instance is anonymous.
- **`requires` are holes**: declared, never defined in the body — the system
  supplies them by wiring (§5, §6).

### 4.2 Event → command

rmdl models emit events and never call (rmdl §5.7); the family's entire
side-effect story is rsdl wiring an emitted event to a command. That is an
ordinary wiring line with a kind-crossing:

```
cluster.chime <- cruise.disengaged        // event -> command: the occurrence invokes the action
```

An `event` may feed a `command` (RSDL-405); all other wires are same-kind. The
ack/retry machinery (ridl §6.1) belongs to this connection's transport, never to
the emitting model — which stays pure and replayable.

### 4.3 Composition may cycle

Unlike rmdl's synchronous causality (rmdl §5.4), a composite component **may
contain cycles** — two sub-components mutually consuming each other's outputs is
normal and safe, because every inter-component signal crosses the asynchronous
broker: no instantaneous dependency, no algebraic loop. Feedback topologies that
would be an instantaneous cycle _inside_ a model are legal _between_ components
(RSDL-407 guards only the synchronous case — a cycle among models within one
leaf).

---

## 5. Providing and Requiring Services

### 5.1 The gradient

Interfaces and services are ridl (the contract SSOT, ridl §14); _providing_ and
_requiring_ them is rsdl. A component relates to contracts three ways, in
increasing reach:

| Verb + object                        | Reach    | Meaning                                                                                         |
| ------------------------------------ | -------- | ----------------------------------------------------------------------------------------------- |
| `provides signal x : T` (inline)     | private  | a one-off output, not in any catalog                                                            |
| `provides CruiseControl` (interface) | internal | offers a reusable shape, wired within the composition                                           |
| `provides veh.adas.cruise` (service) | global   | implements a published SSOT service (ridl §14.5) — addressable, deployable static or discovered |

`requires` mirrors it: a component requires an inline member, an interface, or a
service. **Consumers never distinguish** how a requirement is satisfied —
statically wired, or discovered at runtime — that is a deployment choice (§8),
invisible to the source (RSDL-501).

### 5.2 Providing a service

To provide a service, a component implements its interface's members — produces
its signals/events, accepts its commands/queries — by situating a reaction:

```
service veh.adas.cruise : CruiseControl        // ridl — the global SSOT declaration

component Cruise provides veh.adas.cruise {
  requires signal current : Speed
  requires signal brake   : bool
  (engaged, target) = CruiseController(current, brake, setLever)   // fills the service's members
}
```

`engaged`, `target`, `setLever` come from the service's interface
(`CruiseControl`); the model supplies `engaged`/`target`, consumes `setLever` (a
command routed in as an event, rmdl §7). The component checks that its body
covers every member of the provided service (RSDL-303).

### 5.3 Redundancy

**Two components providing the same service is declared redundancy, not a
conflict.** A global service has one name; more than one provider must be marked
redundant (the reserved `redundant` construct, §10) so the runtime knows to
arbitrate (failover / voting). Without that declaration, two providers of one
service is a **build error** (RSDL-502) — an accidental second provider fails
the build rather than silently becoming a redundant pair, which is the
safety-correct default. The _shape_ stays single (`CruiseControl`); only the
providers multiply.

---

## 6. Systems

A **system** is the **root component** of a workspace — a composite whose
signature is the vehicle's _external_ boundary (V2X, diagnostics, charging),
carrying the one design-time global concern no inner component owns: the
assurance profile.

```
system Vehicle {
  provides veh.diag : DiagnosticAccess
  requires signal v2x : V2xFeed

  assurance automotive               // which @labels vocabulary is in force

  gw     = VehicleGateway()
  adas   = AdasSubsystem(gw.speed, gw.brake)
  panel  = Cluster(adas.cruise)
  panel.chime <- adas.disengaged     // event -> command
}
```

- **One system per workspace** (ADR-0002). The system is the compile root;
  `ridl-diff` treats the whole system contract as the comparison unit.
- The system being _itself a component_ is the fractal payoff: a
  system-of-systems instantiates systems as components. One grammar shape at
  every level.
- The system's body is composition (application notation over sub-components)
  exactly like any composite. Everything _physical_ — time base, targets,
  placement, transport, bundles — lives in `deployment` (§7–§9), because it
  varies per deployment; the system holds only the logical architecture plus the
  assurance profile.

---

## 7. Targets and Placement

**(Deployment region — grammar.)** A **deployment** maps a system onto targets,
`for` a named system. A **target** is a _logical_ execution context — node, ECU,
partition, container slot — named by capability class, never addressed:

```
deployment Production for Vehicle {

  time base ptp                       // the synchronized clock domain (ridl §3.1)

  target adas_hpc : hpc               // logical name : capability class
  target cockpit  : hpc
  target body_ecu : microcontroller

  place adas.*   on adas_hpc          // instance glob on target
  place panel    on cockpit
  place gw       on cockpit
  ...
}
```

- Targets carry _capability class and characteristics_ (compute class, memory,
  reachable transports, assurance level), never hardware addresses — a target's
  logical-to-physical binding is the most deployment-specific layer, potentially
  runtime-discovered in a dynamic SDV world (Ankaios-style orchestration).
- `place` is many-to-one and **complete**: every composition instance must be
  placed (RSDL-701). Globs place a subtree.
- Same system + a different `deployment` = a different physical realization,
  zero composition change (§2).

---

## 8. Transport and Posture

**The heart of rsdl's transport-neutrality, one level richer than a single
wire.** rsdl derives, per connection, not just the _transport_ but the
_communication posture_ — static (signal-based, Classic/bus) vs discovered
(service-oriented, Adaptive/SDV) — from placement, a policy, and the contract's
timing. Source never names either.

```
transport {
  local       : direct              // same partition — in-process
  same_node   : shm                 // shared memory / vsock
  cross_node  : someip              // or dds, or uprotocol
}
```

### 8.1 Posture derivation

A ridl `service` is posture-neutral (ridl §14.5). Its realization is chosen
here:

- a service consumed by a **static wire** (a `requires` bound to a specific
  provider in the composition) → **static posture**: its signals/events pack
  into bus frames (DBC/CAN-style), no discovery.
- a service consumed by **discovery** (a `requires` left to be resolved at
  runtime by name) → **discovered posture**: SOME/IP/DDS/uProtocol, service
  discovery.
- **Constraint from transport physics** (RSDL-803): a `command`/`query` member
  cannot be realized in the static posture — buses carry dataflow, not calls —
  so a service deployed statically realizes only its `signal`/`event` members;
  its control API forces the discovered posture. This is checked, not assumed.

### 8.2 Transport derivation and timing check

Given a connection and its endpoints' placement, the compiler selects the
mechanism, generates the binding, and **checks feasibility against the
contract**: a `signal @10ms` across a link whose worst-case latency exceeds 10ms
is a **deploy-time error** (RSDL-801). Responsiveness (Reactive Manifesto
pillar 1) becomes statically verifiable.

Consequences, each a family-philosophy payoff:

- **Source never names a transport or posture.** No SOME/IP IDs, IP addresses,
  eventgroup IDs, frame layouts, or serialization formats in any layer.
- **One contract, both worlds.** The same `service` compiles to a Classic
  signal-based deployment (DBC/bus frames, static) _and_ an Adaptive/SDV service
  deployment (SOME/IP/uProtocol, discovery) — rsdl chooses per deployment, no
  contract rewrite. The Classic→Adaptive migration path falls out for free.
- **Move an instance, regenerate, done.** Re-placing a component flips its
  connections' transport (and possibly posture) automatically, zero edits to
  endpoints, connection, contract, or reaction.
- **Transport IDs derive from ordinals** (ridl §11): a service's SOME/IP
  method/event IDs come from interaction ordinals, deterministically — readable
  from source, stable under `ridl-diff`.

---

## 9. Bundles

**(Deployment region — grammar.)** A **bundle** is the installable, versioned,
signed **distribution artifact** — the unit of independent FOTA to a target. It
groups the generated code of one or more components plus resources and manifest.

> **Naming — bundle, not package.** ADR-0002 owns _package_ as the
> source-namespace unit. The distribution artifact is a **bundle**, kept
> distinct exactly as Maven/Cargo separate namespace-package from published
> artifact.

```
bundle adas     on adas_hpc { adas.* }
bundle cockpit  on cockpit  { panel, gw }
```

A bundle groups deployables, targets one node, and carries a version and signing
chain. **One bundle concept** — the earlier `spk`/`apk` split is dropped
(Android's `.apk` is not ours to borrow, and platform-vs-application is not a
hard kind). Where a platform-vs-application distinction matters — different
trust, cadence, assurance — it is an **attribute** (`tier system` / `tier app`,
or simply the assurance level the bundle carries), not two coined artifact
types. The `tier` gates dependency direction where declared (an app-tier bundle
may require services a system-tier bundle provides, not the reverse — RSDL-901).

The build emits bundles; an orchestrator (Ankaios-class) places and
lifecycle-manages them at runtime. rsdl is the _source of truth_ the bundle
manifests are generated from — the legible spec above the orchestrator, not a
replacement for it.

---

## 10. Resilience — Reserved

**(Reserved vocabulary; realization deferred to the failure-management
specification, ridl §10.4.)** Resilience is the Reactive Manifesto's second
pillar and rsdl's largest eventual contribution, specified _with_ the deferred
failure-management work, not ahead of it. v0.1 reserves the vocabulary and fixes
its home:

- **`redundant`** — replicate a service across N providers/targets with a
  declared realization (hot standby, N-of-M voting, lockstep); the arbitration
  for §5.3's multiple providers. A property, realized by the runtime.
- **`supervise`** — a supervision tree: which component watches which, and the
  action on detected failure (restart, failover, degrade, safe-state). The
  Erlang/OTP + AUTOSAR-Adaptive State Management model; the Manifesto's
  _delegation of failure_.
- **`degraded`** — mode-dependent topology: an alternate wiring/placement active
  in a degraded mode.
- **containment** is already structural — the component boundary (§1.3) is the
  bulkhead; separate-node placement is isolation. No vocabulary needed.

These keywords are reserved family-wide (typl §1.4); their rules arrive with the
failure-management doc. Everything v0.1 ships — total failure detection (ridl
three strata), envelope loss detection, freshness SLOs, invalid propagation — is
the substrate resilience management stands on.

---

## 11. Conventions

- One system per workspace; subsystems as composite components, decomposed by
  contract boundary (Conway's law, made intentional).
- Name components as the capability they run (`Cruise`, `AdasSubsystem`);
  instances as their role (`cruise`, `frontRadar`); services by dotted global
  path (`veh.adas.cruise`).
- Inline members until reuse earns an `interface`; a `service` only when the
  contract is a system-wide SSOT capability others discover or share.
- Keep composition transport- and posture-free: if a `.rsdl`
  `component`/`system` mentions a bus, an IP, a frame, or a serialization
  format, a layer has leaked — push it to the deployment's `transport`.
- Keep deployments small and many: one composition, one per-variant/per-bench
  deployment.
- Declare `redundant` the moment a second component provides a service — never
  rely on it being inferred.

---

## 12. Diagnostics

Coded `RSDL-`, same lifecycle rules as typl §16, grouped by hundreds: 3xx
components & providing, 4xx wiring, 5xx services, 7xx placement, 8xx
transport/posture, 9xx bundles.

| Code     | Rule                                                                                                     | Severity                   |
| -------- | -------------------------------------------------------------------------------------------------------- | -------------------------- |
| RSDL-301 | applied name is neither a model nor a component                                                          | error                      |
| RSDL-302 | provided output flow with no defining equation and not covered by a provided service                     | error                      |
| RSDL-303 | provided service/interface not fully covered — a member with no defining equation                        | error                      |
| RSDL-304 | leaf component's applied model signature mismatches the wired signals (kind/type)                        | error                      |
| RSDL-401 | conditional or dynamic instantiation                                                                     | error                      |
| RSDL-402 | wire connects flows of differing type/contract                                                           | error                      |
| RSDL-403 | required member left unwired and not resolvable by discovery                                             | error                      |
| RSDL-404 | provided output defined more than once                                                                   | error                      |
| RSDL-405 | kind-crossing wire other than event→command                                                              | error                      |
| RSDL-407 | instantaneous cycle among models within one leaf component (the sync wall)                               | error                      |
| RSDL-501 | consumer source distinguishes static-wire from discovered-service (leak)                                 | error                      |
| RSDL-502 | two components provide the same service without a `redundant` declaration                                | error                      |
| RSDL-503 | published service name collides (also ridl RIDL-140)                                                     | error                      |
| RSDL-701 | composition instance with no placement                                                                   | error                      |
| RSDL-702 | placement references a non-existent instance                                                             | error                      |
| RSDL-703 | placement onto a target whose capability class cannot host the component                                 | error                      |
| RSDL-801 | derived transport cannot meet a connection's contract timing                                             | error                      |
| RSDL-802 | no transport-policy entry for a required (locality × kind) combination                                   | error                      |
| RSDL-803 | a `command`/`query` service member forced into the static posture (buses can't RPC)                      | error                      |
| RSDL-901 | a system-tier bundle requires a service provided only by an app-tier bundle (trust inversion)            | error                      |
| RSDL-902 | bundle includes an instance placed on a different target                                                 | error                      |
| RSDL-950 | reserved resilience keyword (`redundant`/`supervise`/`degraded`) used before the failure-management spec | error (not yet realizable) |

---

## 13. Open Questions

1. **Composition vs deployment boundary** — drawn at logical/physical, both
   grammar (§2). The time base is arguably composition-relevant (rmdl clocks
   depend on it) yet declared in deployment; confirm as real programs exercise
   it.
2. **Dynamic topology / orchestration** — v0.1 is static instances + static
   placement. SDV live workload orchestration (Ankaios: start/stop/move at
   runtime, mode-dependent activation) needs a dynamic layer: conditional
   instantiation, runtime placement, hot reconfiguration. Reserved; the biggest
   v0.2 chapter, and where `elastic` (Manifesto pillar 3) lands.
3. **Transport/posture policy expressiveness** — the (locality × kind) table
   covers common cases; per-connection overrides, QoS beyond timing (DDS
   reliability/history), mixed-criticality isolation, and explicit posture
   pinning need a richer policy language.
4. **Service discovery semantics** — versioned services, multiple providers,
   consumer preference, failover on provider loss. Ties to redundancy (§10) and
   needs the runtime spec.
5. **End-to-end timing composition** — RSDL-801 checks one connection; a _chain_
   has an end-to-end budget that must compose. Compositional latency analysis
   across the broker is research-adjacent; v0.1 checks per-connection only.
6. **Bundle dependency & versioning** — inter-bundle service dependencies,
   version ranges, coordinated atomic multi-bundle updates. Overlaps ADR-0002's
   diamond question, one layer up.
7. **Resilience realization** — the whole of §10, with the failure-management
   spec: supervision-tree semantics, redundancy realizations, degraded-mode
   topology, interaction with the assurance profile.
8. **Global service catalog scoping** — a flat global service namespace
   (RIDL-140) is the SSOT ideal, but very large programs may need catalog
   namespacing/versioning beyond the package system. Watch as catalogs grow.

---

## Appendix A — Full Example

Logical composition and two deployments — all `.rsdl` grammar — for a small
vehicle. Contracts are ridl; behaviour is rmdl; this file situates and deploys
them.

**Composition** (`veh/system.rsdl`):

```
package veh

import veh.common.Speed
import veh.adas.CruiseControl          // ridl interface (shape)
import veh.adas.cruise                 // ridl service : CruiseControl
import veh.adas.model.CruiseController // rmdl model (pure reaction)
import veh.cluster.model.ClusterLogic

// leaf component — situates the reaction, provides the global service
component Cruise provides veh.adas.cruise {
  requires signal current : Speed
  requires signal brake   : bool
  (engaged, target) = CruiseController(current, brake, setLever)
}

// leaf component — consumes the service, drives a display, chimes on disengage
component Cluster {
  requires veh.adas.cruise                       // requires the whole service
  provides signal lamp : bool = ClusterLogic(veh.adas.cruise.engaged)
  // event -> command wiring done at system level
}

// composite subsystem
component AdasSubsystem(signal speed: Speed, signal brake: bool) {
  cruise = Cruise(current: speed, brake: brake)
  provides veh.adas.cruise from cruise           // re-provide the child's service
}

// the system (root component)
system Vehicle {
  provides veh.diag : DiagnosticAccess
  assurance automotive

  gw    = VehicleGateway()                        // provides speed, brake from the bus
  adas  = AdasSubsystem(gw.speed, gw.brake)
  panel = Cluster()

  panel.chime <- adas.cruise.disengaged           // event -> command (§4.2)
}
```

**Deployment — production, distributed** (`veh/production.rsdl`):

```
deployment Production for Vehicle {
  time base ptp
  target adas_hpc : hpc
  target cockpit  : hpc

  place adas.* on adas_hpc
  place panel  on cockpit
  place gw     on cockpit

  transport { same_node : shm, cross_node : someip }

  bundle adas    on adas_hpc { adas.* }
  bundle cockpit on cockpit  { panel, gw }
}
```

**Deployment — HIL bench** (`veh/hil.rsdl`, same system):

```
deployment Hil for Vehicle {
  time base ptp
  target bench : hpc
  place adas.* on bench
  place panel  on bench
  place gw     on injector          // gw swapped for a rest-bus injector — test plane
  transport { same_node : direct }  // everything in one process on the bench
}
```

`veh.adas.cruise` deploys as a discovered SOME/IP service across ECUs in
production and as a direct in-process call on the bench — **the composition
never changed**, and its `setLever` command forces the discovered posture
wherever it is reachable (§8.1).

---

## Appendix B — Deployment Targets

rsdl is the source of truth the deployment ecosystem is generated _from_:

| Generated artifact                            | From                                                 | Consumer                           |
| --------------------------------------------- | ---------------------------------------------------- | ---------------------------------- |
| per-target binding/glue code                  | composition + placement + transport policy           | `ridl-rt` on each target           |
| DBC / bus frame layout (static posture)       | services in static posture + typl widths             | CAN/LIN stacks                     |
| SOME/IP service & eventgroup config, `.arxml` | services in discovered posture + ordinals (ridl §11) | AUTOSAR Adaptive/Classic           |
| uProtocol service definitions, UUris          | discovered services                                  | Eclipse uProtocol                  |
| DDS topic/QoS config                          | signal/event connections + timing                    | DDS middleware                     |
| Ankaios workload manifests                    | bundles + placement                                  | Eclipse Ankaios orchestrator       |
| VSS overlay                                   | signal services + wiring                             | COVESA VSS / Kuksa data broker     |
| test topology                                 | composition + a test deployment                      | the test plane (concept note §9.2) |

The principle throughout: rsdl **generates** the transport-, posture-,
orchestrator-, and stack-specific artifacts; it is never written in their terms.
The same composition targets DBC, SOME/IP, DDS, and uProtocol by choice of
deployment posture.

---

## Appendix C — Formal Grammar (EBNF)

Both regions are grammar. Shared productions per typl Appendix E.

```ebnf
definition    = [ "internal" ]
              ( typl_definition | component_def | system_def | deployment_def ) ;

(* ---------- Logical composition ---------- *)

component_def = doc_comment? "component" CamelCase_id [ sig_params ] [ provides_list ]
                "{" { comp_item sep? } "}" ;
system_def    = doc_comment? "system" CamelCase_id [ provides_list ]
                "{" [ assurance_decl ] { comp_item sep? } "}" ;

sig_params    = "(" param_list ")" ;                     (* signature inputs, kinded *)
param         = ( "signal" | "event" | "command" | "query" )? camelCase_id ":" type_ref ;
provides_list = "provides" contract_ref { "," contract_ref } ;
contract_ref  = qualified_id ;                           (* interface, or dotted service name *)
assurance_decl = "assurance" id ;

comp_item     = require_decl | provide_def | let_apply | wire_decl ;

require_decl  = "requires" ( member_decl | contract_ref ) ;
provide_def   = "provides" member_decl [ "=" flow_expr ] [ timing ]
              | "provides" contract_ref [ "from" camelCase_id ] ;   (* re-provide a child's *)
member_decl   = ( "signal" | "event" | "command" | "query" ) camelCase_id ":" type_ref
              | "command" camelCase_id "(" param_list ")" ;
let_apply     = "let" ( camelCase_id | "(" id_list ")" ) "=" application ;
wire_decl     = port_ref ( "<-" | "=" ) flow_expr ;      (* incl. event->command *)

flow_expr     = application | port_ref | expr ;
application   = CamelCase_id "(" arg_list ")" ;          (* apply a model or component *)
arg_list      = "" | arg { "," arg } ;
arg           = [ camelCase_id ":" ] flow_expr ;         (* positional or named *)
port_ref      = camelCase_id { "." camelCase_id } ;

(* ---------- Physical deployment ---------- *)

deployment_def = doc_comment? "deployment" CamelCase_id "for" CamelCase_id
                 "{" { deploy_item sep? } "}" ;
deploy_item   = timebase_decl | target_decl | place_decl | transport_decl | bundle_decl ;
timebase_decl = "time" "base" id ;
target_decl   = "target" camelCase_id ":" id ;
place_decl    = "place" instance_glob "on" camelCase_id ;
instance_glob = camelCase_id { "." camelCase_id } [ ".*" ] ;
transport_decl = "transport" "{" { route sep? } "}" ;
route         = locality ":" id ;
locality      = "local" | "same_node" | "cross_node" ;
bundle_decl   = "bundle" camelCase_id [ "tier" ( "system" | "app" ) ] "on" camelCase_id
                "{" instance_glob { "," instance_glob } "}" ;

reserved_resilience = "redundant" | "supervise" | "degraded" ;  (* §10 — not yet realizable *)
```

---

## Appendix D — Prior Art Survey

| Source                        | Taken / rejected                                                                                                                                                                                                                                                                                                                                                                                                                                                                              |
| ----------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Reactive Manifesto**        | the design spine: _message-driven_ = transport-neutrality/location transparency (§1.1, §8); _responsive_ = timing propagation + deploy-time latency check; _resilient_ = supervision/redundancy/containment (§10); _elastic_ = dynamic orchestration (§13.2)                                                                                                                                                                                                                                  |
| **AUTOSAR Classic**           | VFB = location-transparent logical wiring (composition); System Description + ECU-Extract = deployment mapping (generated per target); the signal-based **static posture** (§8.1) — but ARXML-drowned; rsdl is the legible source it is generated _from_                                                                                                                                                                                                                                      |
| **AUTOSAR Adaptive**          | the service-oriented model: ServiceInterface (ridl `interface`) vs Service Instance (ridl `service`); offer/find via ara::com = the **discovered posture**; the manifest trio = deployment + bundles; Execution & State Management = the `redundant`/`supervise` target                                                                                                                                                                                                                       |
| **AADL**                      | the academic ancestor: component types + features + connections + _binding properties_ (logical→physical). rsdl is "AADL's binding with the family's legibility and message-driven-by-default"; leaf-vs-composite = thread-vs-system                                                                                                                                                                                                                                                          |
| **SysML**                     | parts, ports, connectors, and block-is-recursive — validates the fractal component (system = component); rsdl is the executable, transport-resolving subset                                                                                                                                                                                                                                                                                                                                   |
| **Eclipse SDV**               | the contemporary convergence: **uProtocol** = transport-neutral messaging with UUri (the derived discovered posture); **Ankaios** = workload orchestrator (bundles placed/lifecycled at runtime); **Velocitas** = vehicle apps (app-tier bundles); **Kuksa/VSS** = the signal broker (the static-posture / global service catalog); **Blueprints** = reference deployments. SDV converges on this layering _without a unifying source language_ — rsdl is that language, above the frameworks |
| **Protocol Buffers / gRPC**   | proto `service` ≈ ridl `interface`; proto stops at the contract with no wiring/placement — rsdl fills exactly that gap. Service mesh (Envoy/Istio) = location transparency for cloud; rsdl+runtime is the automotive equivalent, _derived from source_ not sidecar-configured                                                                                                                                                                                                                 |
| **ROS 2**                     | nodes = leaf components; the node graph = composition; **launch files = deployments**; **packages = bundles**; DDS discovery = the discovered posture                                                                                                                                                                                                                                                                                                                                         |
| **Erlang/OTP**                | supervision trees + "let it crash" + process isolation = the §10 resilience model and containment-by-boundary; the mailbox = the async component wall                                                                                                                                                                                                                                                                                                                                         |
| **Lingua Franca (federated)** | reactors placed across federates with logical time — the closest synchronous-family precedent for distributing a reaction graph deterministically (ties to rmdl logical time)                                                                                                                                                                                                                                                                                                                 |

---

## Appendix E — Coverage Analysis: Architecture & SDV Frameworks

✓ covered, ≈ covered differently, ✗ not expressible (deliberate or open).

| Construct                                                          | rsdl equivalent                                            | Status                             |
| ------------------------------------------------------------------ | ---------------------------------------------------------- | ---------------------------------- |
| logical software architecture (VFB, AADL model)                    | composition (components applying reactions, wiring)        | ✓                                  |
| provided/required contracts                                        | `provides`/`requires` inline members, interfaces, services | ✓ three grains                     |
| composite/hierarchical components                                  | composite component (applies sub-components)               | ✓ recursive                        |
| deployment mapping (ECU extract, AADL binding, ROS launch)         | `deployment` (grammar)                                     | ✓ one-composition-many-deployments |
| signal-based (Classic) vs service-oriented (Adaptive)              | static vs discovered **posture**, derived (§8)             | ✓ one contract, both               |
| location-transparent transport                                     | derived transport (§8), timing-checked                     | ✓                                  |
| service discovery (SOME/IP-SD, uProtocol, DDS)                     | discovered posture on a `service` (§8.1)                   | ✓                                  |
| global signal/service catalog (VSS, uProtocol UUri)                | the ridl `service` catalog (SSOT)                          | ✓                                  |
| distribution artifacts (Adaptive manifest, ROS package, container) | bundles (§9)                                               | ✓ one concept                      |
| dynamic orchestration (Ankaios, K8s)                               | —                                                          | ✗ open §13.2 (elastic)             |
| supervision / redundancy / failover                                | reserved (§10)                                             | ≈ deferred                         |
| end-to-end timing budgets                                          | per-connection check only                                  | ≈ partial §13.5                    |
| QoS matrices (DDS 22 policies)                                     | timing in contract + transport policy                      | ≈ deliberate                       |
| back-pressure / flow control                                       | ridl interaction semantics (coalesce/TTL) + buffer config  | ✓ inherited                        |

**Verdict.** rsdl covers the logical-architecture and deployment-mapping working
set of AUTOSAR, AADL, and ROS 2, and the service/bundle model of SDV and
Adaptive, through one situated-reaction component, application-notation wiring,
and a clean composition/deployment split — while adding what none has as
_source_: a posture _and_ transport derived-and-timing-checked from placement,
so one contract set deploys signal-based _or_ service-oriented; one composition
driving many deployments (including the test plane) for free; and a single
legible language above the SDV framework stack. Honest gaps are v0.2+: dynamic
orchestration, resilience realization, end-to-end timing composition,
discovery-matching semantics — each reserved with its home fixed.

---

## Appendix F — Glossary

Family terms are defined in the typl/ridl/uxdl/rmdl glossaries and mean the same
here. rsdl-specific:

| Term                                          | Definition                                                                                                                                                                                                                 |
| --------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **component**                                 | a reaction _situated in the system_: real-signal boundary (`provides`/`requires`), a body that applies models (leaf, synchronous) or sub-components (composite, async); the architectural atom and the sync/async boundary |
| **application notation**                      | a component body written as `outputs = Reaction(inputs)` equations — models/components applied to signals producing signals; each site an instance owning its state                                                        |
| **provides / requires**                       | a component offering / needing a contract — an inline member, a named `interface`, or a global `service`; the object scales with reach (§5.1)                                                                              |
| **instance**                                  | an application site of a model or component; owns its state, identity, and (once deployed) placement                                                                                                                       |
| **wire**                                      | a connection of one flow to another (`a <- b` / `a = b`); carries the interaction's ridl semantics, adds none; event→command is the one kind-crossing                                                                      |
| **system**                                    | the root component of a workspace; external-boundary signature + the assurance profile; body is composition like any composite                                                                                             |
| **service** (rsdl view)                       | a ridl global published contract that a component _provides_; two providers = declared redundancy                                                                                                                          |
| **target**                                    | a logical execution context (node/ECU/partition), named by capability class, never addressed                                                                                                                               |
| **placement**                                 | the `place … on …` mapping of instances to targets in a `deployment`; complete; same system + different deployment = different realization                                                                                 |
| **posture**                                   | how a connection to a service is realized — static (signal/bus, Classic) or discovered (service, Adaptive) — derived from static-wire-vs-discovery + physics (§8.1)                                                        |
| **transport derivation**                      | computing each connection's transport and posture from placement × kind × policy, checked against timing; source names neither                                                                                             |
| **bundle**                                    | the installable, versioned, signed distribution artifact; one concept, optional `tier` (system/app); distinct from ADR-0002's source _package_                                                                             |
| **logical composition / physical deployment** | rsdl's two grammar regions: `component`/`system` (what runs and connects, transport-free) vs `deployment` (where it runs, how it ships); one composition drives many deployments                                           |
| **location transparency**                     | a component's indifference to where its peers run; the Manifesto's message-driven principle, and why the surface never leaks transport or posture                                                                          |

---

_End of rsdl Language Reference v0.1.0 — Draft._
