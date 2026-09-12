# rsdl rewrite — the decisions of 2026-09-12

Status: working note, 2026-09-12. Settles the points the rsdl rewrite (design
note §3.1) has to fix before a new reference can be written, and amends
[`2026-09-08-topology-vocabulary.md`](2026-09-08-topology-vocabulary.md) §7
where the two disagree. Each decision records its alternatives. Nothing here is
ratified; until the rewrite lands, the rsdl reference is the record and this
note is the proposal. Read after the vocabulary note and
[`2026-09-12-release-scope-and-plugin-system-design.md`](2026-09-12-release-scope-and-plugin-system-design.md)
§3.1, §3.8 and §3.13.

Why it exists: a recap of rsdl against the vocabulary note found five points the
note answers twice or not at all — the process noun, what is placed, posture,
the lock's content, and cross-catalog references — plus a handful of smaller
ones. They were settled one at a time, and one new mechanism (instances) and one
principle (attributes as the backend escape hatch) came out of the discussion.
Recorded here so the rewrite starts from a file.

## 1. The shape, after these decisions

    distribution  what is versioned, signed and shipped
    machine       what it is installed on; the unit of placement
    component     what steps — one execution context; offers services,
                  requires interfaces; has instances
    service       what is addressed on the wire                     (ridl)
    interface     what is contracted                                (ridl)
    member        one typed interaction                             (ridl)
    catalog       an id space and a hash over a package's interfaces (ridl)

`process` leaves the list for rsdl's purposes (D-2). The three trees of the
vocabulary note §2 stand, with the runtime tree shortened to
`machine -> component`. V-02 stands unchanged: only what is addressed has
identity on the wire; distribution, machine, component and instance identifiers
never travel.

Four declarations, no recorded artifact of rsdl's own:

    system        the closure: which components are in scope, the external
                  boundary, the assurance profile slot
    component     offers services, requires interfaces, may be external
    distribution  contains components, depends on distributions, carries
                  a tier
    deployment    named, several per system: declares machines, places
                  instances on them

Every declaration and every placement line takes the family attribute block
(D-6). The interface-id registry the lowering reads is a ridl artifact (D-7).

## 2. Decisions

### D-1 A lone service stands for an implicit component

**Decision.** A `service` declared in ridl that no declared `component` offers
stands for an implicit single-service component of the same name, and `place`
accepts the service name directly. The derived component name appears only in
placement lines, diagnostics and the IR. An implicit component is implemented;
an external or stub provider needs an explicit `component` with the `external`
flag (D-9).

**Rejected.** (a) An implicit _service_ named after a component: the service
name is what the attach list, the manifests and the security labels use, so a
name derived from the component makes the component name travel, which V-02
forbids and which was decided four separate times. (b) Interfaces or
interactions declared inline in a `component`: the catalog is ridl's (V-15,
V-16) and `ridl diff` would have to read rsdl to classify a contract change;
generation follows imports and a consumer would import an rsdl file (V-06); and
an inline interface has no name of its own, so a consumer would name the
component and lose immunity to recomposition (vocabulary note §3). This is also
the v0.1 inline-member gradient the vocabulary note dropped: every crossing now
has a routing key of slot, interface and member, so a private member outside a
catalog no longer exists.

**Rule.** Derive the unaddressed noun from the addressed one, never the reverse.

### D-2 No process declaration, no scheduling facts

**Decision.** rsdl declares no process and carries no pump kind, priority or
other scheduling fact. Placement is an instance of a component on a machine,
nothing finer. Crossing kinds in rsdl are three: same machine, different
machine, off-board. The grant list is derived per component from its requires
closure.

**Consequences.** Whether two components on one machine share an address space
is the backend's or the host program's decision; under V-09 the direct-channel
case is an optimisation with identical semantics, so rsdl loses nothing by not
naming it. A backend that groups components into processes, or Classic into
OS-Applications, unions the grants per group — one join later. V-11's
mixed-level check leaves rsdl: a machine is certified for a level, and a
component that demands a higher level placed on it is a lint under the
automotive profile (via `labels`, D-6), out of this release. V-10's "failure
containment belongs to the process" becomes a statement about the target.

**Rejected.** A `process` block nested under `machine`, carrying the assurance
level, with pump and priority on the placement line. What a process is differs
per target — Classic has none, Adaptive has Process, baremetal has one address
space — and scheduling is the backend's and the engine's. What would reopen a
grouping noun: a consumer whose safety argument needs the mix rejected at design
time. Then the addition is an optional partition grouping under `machine` with a
target-neutral name and a single implicit partition when none is declared.

### D-3 `place` takes an instance of a component, only

**Decision.** A service's machine is derived, never declared: an interface has
one owning service, a service has one offering component, an instance of that
component has one machine per deployment. Two rules follow, both errors:

- every service in the closure is offered by exactly one component — zero is a
  missing provider, two is a conflict (redundancy is D-4's derived case, not an
  exception here);
- every instance of every component in the closure is placed exactly once per
  deployment — the v0.1 RSDL-701 rule kept, at instance grain. An external
  system is an `external` component placed on an external machine, so it is not
  an exception.

**Rejected.** Placing the service (vocabulary note §3 "the unit of publication,
addressing and placement"; roadmap-simplification S-35). A consumer-only
component offers nothing and still needs a machine before its crossing kinds
exist — the job S-35 said `component` had to earn. A component offering two
services must put both on one machine, because it is one execution context. The
one-service case loses no ergonomics under D-1.

**Amendment.** Vocabulary note §3: "the unit of publication and addressing".
Placement is listed once, under `deployment`.

### D-4 Multiplicity is an attribute; redundancy is derived

**Decision.** A component's instance count is an attribute, per
family-general-form §4.8 (attribute first) and R3 (brackets have two positions,
so no array syntax on a name):

    component Cruise offers veh.adas.cruise [ instances = 2 ]
    component Cruise offers veh.adas.cruise [ instances = (primary, backup) ]

- `instances = N` gives inferred, 1-based names: `Cruise.1`, `Cruise.2`. Adding
  an instance never renames an existing one.
- `instances = (a, b)` gives named instances: `Cruise.primary`.
- No attribute gives exactly one instance, the **unit instance**, named `Unit`
  in the IR and in diagnostics and spelled by the bare component name in source:
  `place Cruise on m` places `Cruise.Unit`. A lone service's implicit component
  (D-1) has its unit instance and nothing else. Declaring `instances` replaces
  the unit instance rather than adding to it.
- Instance names are unique within a component; a duplicate is an error, and
  `Unit` may not be declared explicitly.
- Instances share the component's offers and requires and are indistinguishable
  on the wire (V-02).
- Placement addresses instances through the dotted names and the prefix glob the
  vocabulary note already has: `place Cruise.primary on m`,
  `place Cruise.* on m`.
- **Redundancy is a derived fact.** A component with more than one instance that
  offers a service is that service's redundant provider set. D-3's "one offering
  component" stays. The lowering reports a redundant provider set as not yet
  realizable until the runtime has arbitration (failover, voting, and how a
  store with one writer slot per member takes two writers — engine questions,
  parked). A redundant pair placed on one machine is a profile lint later, not a
  grammar rule.

**Rejected.** A reserved `redundant` keyword on the `system` against the service
name (the v0.1 §10 stance). Multiplicity is the general mechanism and redundancy
one consequence of it; the keyword would have been a special case.

**Limit, deliberate.** Instances share their offers, so a left sensor and a
right sensor offering two different services are two component declarations, not
two instances. Per-instance service binding would bring back the
instance-versus-kind split dropped with the application notation and is its own
decision if ever wanted. Consumer-only multiplicity is legal for free.

### D-5 rsdl lowers the crossing kind; transport and topology are configuration

**Decision.** Each link in the system IR carries its crossing kind and its two
endpoints, nothing else about the physical layer. The transport over each
crossing, and the network and machine fabric — VLANs, VM placement, CPU clusters
— are the implementer's configuration. If they ever enter rsdl they arrive as
attributes on `machine` or a block under `deployment`, earned by a backend that
reads them (D-6).

**Posture** (v0.1 §8: static bus frames versus discovered service) is kept as a
word in a reserved section of the rewritten reference: a `service` stays
posture-neutral in ridl; rsdl derives no posture in this release; posture
derivation, RSDL-803 and RSDL-801 reopen with a bus-class backend, the target
ADR-0013's classification would make them real for. The family overview's
doctrine 18 keeps its text and re-cites the reserved section instead of "rsdl
§8".

**Rejected.** (a) A `transport { local, same_node, cross_node }` table in the
grammar: this release has one transport family, and the table would name choices
no backend can act on. (b) Dropping the word posture entirely: the
Classic-to-Adaptive migration argument is the reason for the contract-versus-
placement split and should stay findable outside the archive.

### D-6 Attributes are the backend escape hatch

**Decision.** Every rsdl declaration — `system`, `component`, `distribution`,
`deployment`, `machine` — and every placement line takes the family `[ ]`
attribute block with its three forms (family-general-form §4.2). Two kinds of
key:

- **rsdl-owned keys** are allow-listed per declaration kind, an unknown key is
  an error, and every key has a machine consumer (the deletion test, §4.1).
  `instances` (D-4) and `external` (D-9) are the first two.
- **Backend keys are namespaced by the backend name** — `someip.service_id`,
  `linux.cpuset`, `rust.crate`. The compiler carries a namespaced key into the
  IR without interpreting it: the system IR gains an **attribute map per node**,
  alongside the region map, link set, routing table, permission list, surface
  set and catalog hash of design note §3.13, and every extract a backend reads
  carries it through. The backend validates its own namespace — form and legal
  declaration kinds — through the plugin contract, which gains one line: a
  backend declares the keys it consumes. A namespace no configured backend
  claims is a warning, not an error, so emitting the IR never depends on which
  backends run.

This is the single "later" mechanism for three of the settled points: the
certified-level lint (D-2) is `labels` on a `machine` under the automotive
profile; the fabric facts (D-5) are backend keys on `machine` and on placements;
a tag-based transport's service number (D-7) is that backend's attribute on the
placement, hand-allocated by the integrator as AUTOSAR does, uniqueness checked
by the backend.

### D-7 One registry: interface ids per catalog, a ridl artifact

**Decision.** V-X2 is answered no: nothing routes by service on this transport —
the routing key is slot, interface, member — so no service number is allocated,
and ADR-0016 decision 8 stands as scoped (tag-based transports only, and those
use D-6). The one recorded thing is V-17's: interface ids within a catalog,
which after V-16 means per package.

- It is a **ridl artifact the compiler owns**, not an rsdl declaration. `lock`
  leaves the vocabulary note §7 list; the rsdl reference cites the registry
  once, as an input to the lowering, because the routing table cannot be emitted
  without the ids.
- **One checked-in file per package, next to the sources, not inside
  `ridl.lock`.** ADR-0002's lockfile is one per workspace, pins dependency
  versions by content hash, and is regenerated whenever resolution runs; the
  registry is per package, must never be regenerated, and ships with the package
  so every consumer sees the same ids. The filename is the rewrite's to pick.
- **Rules**, the ordinal model one level up: an id once allocated is never
  reused; a retired interface keeps its id as a tombstone; a rename keeps its
  id; a new interface gets the next id. Allocation happens only on an explicit
  command — a build that meets an unregistered interface fails instead of
  allocating silently, so two branches cannot hand out the same number.
  `ridl diff` reads the registry to classify.
- **The catalog hash is derived, never recorded.** Two builds of one package
  agree on it because they agree on the registry.

**Rejected.** (a) Recording service numbers too: ids nothing reads, and removing
a kind of id later is a wire change while adding one is additive. (b) Folding
the ids into `ridl.lock`: the one way to lose an allocation silently. (c) Silent
allocation on build: collisions surface at integration.

**Amendment.** Vocabulary note §7: "the interface-id registry per catalog, a
ridl artifact the lowering reads".

### D-8 Cross-catalog type references are allowed; the hash covers the closure

**Decision.** A payload in one catalog may name a type declared in another. Most
of this was already decided — ADR-0002 imports, ADR-0017's projection of a
foreign reference, inherited by the FlatBuffers backend — so V-X1 reduces to
what a catalog's hash covers: **its interfaces plus every type they reach,
transitively, wherever declared.** A change to a framework type then changes the
hash of every catalog that uses it, which is the truth about their wire shapes.
The routing key is untouched (types never travel); the permission boundary is
untouched (attaching a programme catalog's region never requires the
framework's, because the types are compiled into the consumer's view);
`ridl diff` classifies through the same closure. This lands as the disposition
of V-X1 in the ridl finalization; rsdl consumes the hash.

**Rejected.** Forbidding the reference and requiring every catalog to declare
the types it uses: copies a shared vocabulary into every programme package and
creates the drift the family exists to prevent.

### D-9 The smaller points

- **A distribution may span machines.** Installation is derived: a distribution
  is installed on every machine where one of its instances is placed, and the
  emitter reports the set. No `on machine` clause on `distribution`.
- **rsdl knows one fact about an implementation: present or not.** The flag
  attribute `[ external ]` marks a component with none (V-20's "may"); the
  default is implemented, so the common case writes nothing. What the
  implementation is — a crate, a language — is a backend key (D-6).
- **The grant, stated once.** A consumer names interfaces (V-05); the lowering
  resolves each through its one owning service to its catalog; the grant is the
  set of catalog regions reached (vocabulary note §6). V-06's "at service
  granularity" is reworded to "resolved through the owning service".
- **ridl §14.6** is rewritten to "a component offers services and requires
  interfaces", dropping "services or individual members". A ridl finalization
  edit.
- **One system per workspace** stays (ADR-0002, v0.1 §6). A workspace with two
  systems is a new decision when a consumer needs it.

### D-10 `system` confirmed as the closure

Recorded because it was asked. A `system` is the named set of components a
deployment has to account for, plus two attributes: the **external boundary**
(interfaces required from outside, services offered to outside — where off-board
is defined) and the **assurance profile** slot (profile-gated, reserved in this
release). It has no composite body and no wiring; the v0.1 root-component form
is gone with the application notation. Every completeness check quantifies over
it; every `deployment` is `for` one; `ridl diff` compares at it. Under D-1 a
system of hand-written services is a list of service names.

## 3. Amendments implied for existing records

- **Vocabulary note** (`2026-09-08-topology-vocabulary.md`): §1 noun list loses
  `process` for rsdl's purposes and V-10, V-11 become statements about the
  target and the profile (D-2); §3 "unit of publication and addressing" (D-3);
  V-06 reworded (D-9); §7 list becomes four declarations, `lock` replaced by a
  reference to the ridl registry (D-7); V-X1 and V-X2 answered (D-8, D-7).
- **Design note** (`2026-09-12-release-scope-and-plugin-system-design.md`): §3.8
  plugin contract gains "a backend declares the keys it consumes" (D-6); §3.13
  the system IR gains an attribute map per node (D-6); §4 "rsdl's noun set" is
  resolved — `component` stays, `process` goes (D-2, D-3).
- **Family overview** doctrine 18: re-cite the reserved posture section (D-5).
- **ridl reference**: §14.6 wording (D-9); V-X1 disposition (D-8); the registry
  file (D-7) — all part of the ridl finalization.
- **Roadmap**: the E6 stories left untouched by the re-scope are refiled against
  this note once the rewrite starts.

## 4. Open

- **V-X4** cardinality and policy on `requires` — arrives with variant handling.
- **V-X5** what a breaking deployment change is — E4.5a's stability policy.
- **V-X6** `Busy` and the purity of a step — the step contract.
- **RSDL-801** end-to-end and per-link timing feasibility — deferred with
  posture (D-5).
- **The registry filename** and the exact allocation command (D-7) — the rewrite
  picks them.

## 5. Illustrative example

Syntax is not fixed; the example shows the shape the decisions add up to.

    // contracts.ridl — the catalog veh.adas: interfaces and services
    interface CruiseControl { ... }
    interface LaneAssist    { ... }
    service veh.adas.cruise : CruiseControl
    service veh.adas.lane   : LaneAssist
    service veh.diag        { ... }             // one inline shape

    // system.rsdl
    component Cruise offers veh.adas.cruise [ instances = (primary, backup) ]
    component Lane   offers veh.adas.lane
    component Panel  requires CruiseControl, LaneAssist
    component Backend requires CruiseControl [ external ]

    system Vehicle {
      Cruise, Lane, Panel, Backend
      veh.diag                                  // implicit component, unit instance
    }

    distribution adas  [ tier = system ] { Cruise, Lane, veh.diag }
    distribution hmi   [ tier = app ]    { Panel }

    deployment Production for Vehicle {
      machine adas_hpc [ labels = (ASIL_B) ]
      machine cockpit
      machine cloud    [ external ]

      place Cruise.primary on adas_hpc
      place Cruise.backup  on cockpit
      place Lane           on adas_hpc
      place veh.diag       on adas_hpc
      place Panel          on cockpit  [ linux.cpuset = (2, 3) ]
      place Backend        on cloud
    }

What the lowering derives from it: three crossing kinds per link (Panel to Lane
is inter-machine, Lane to Cruise.primary is same-machine, Backend to Cruise is
off-board); a grant list per component as a set of catalog regions; installation
of `adas` on `adas_hpc` and `cockpit`; and a redundant provider set for
`veh.adas.cruise`, reported as not yet realizable.
