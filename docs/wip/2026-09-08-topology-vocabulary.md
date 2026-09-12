# Topology vocabulary — the nouns, and what each one owns

Status: working note, 2026-09-08. Fixes the words for the layer between a
contract and the hardware it runs on: what is shipped, what runs it, what
executes, what is addressed, what is contracted. Extracted from
[`2026-09-08-roadmap-simplification.md`](2026-09-08-roadmap-simplification.md)
rules S-34 to S-50, which reached these by argument; this note states the
results and the tests they have to pass.

Why it exists: three vocabularies describe this layer and they disagree — rsdl
§1.2 and §3, AUTOSAR (Classic and Adaptive, differently), and the first
consumer's topology naming of 2026-08-10. Each is right about something and none
covers the whole range, so the words were being re-derived from scratch every
few weeks.

Nothing here is ratified. Where it contradicts rsdl's reference, the reference
is the record and this note is the proposal.

## 1. The nouns

    distribution  what is versioned, signed and shipped
    machine       what it is installed on
    process       what is isolated, started, placed, and assured
    component     what steps — one reaction, one rate, one pump
    service       what is addressed on the wire
    interface     what is contracted
    member        one typed interaction: signal, event, command, query, fixed
    catalog       an id space and a hash over a set of interfaces

    V-01  MUST   Each noun answers exactly one question, and each question
                 has exactly one noun. A noun that answers two is carrying
                 a fusion that will need undoing.
    V-02  MUST   Only what is **addressed** has identity on the wire.
                 `service` does; catalog does, as a per-connection slot.
                 Distribution, machine, process and component identifiers
                 never travel. They are build-time and deployment-time
                 concepts, and a runtime artifact that names one has
                 coupled two layers that were separated on purpose.

V-02 is the rule that keeps recurring. It was decided independently for
`machine_id` (the first consumer's topology naming, because placement is what
changes between v1 and v2), for the catalog name (`catalog-abi.md` ABI-02,
because a global catalog-id registry is the coordination independent catalogs
exist to avoid), and for bundles and components here. Four derivations of one
rule.

## 2. Three trees, not one hierarchy

A component appears in all three, related three different ways. Conflating any
two of them is the standard way this layer goes wrong.

    distribution   distribution -> components        version, sign, install
    runtime        machine -> process -> component   execute
    contract       service -> interface -> member    publish

    V-03  MUST   A distribution is not a process. One is what you ship,
                 the other is what runs; their cadences differ, and OTA
                 works on the first while scheduling works on the second.
    V-04  MUST   A component is the unit of execution only. It is not the
                 unit of distribution — shipping a thread is absurd — and
                 it is not the unit of isolation (§4).

AAOS already works this way: an APK or an APEX holds many execution units.
AUTOSAR Adaptive too: SoftwareCluster ships, Process runs.

## 3. The asymmetry — offer a service, consume an interface

    V-05  MUST   A component **offers** zero or more services and
                 **requires** interfaces. The two sides carry different
                 types and it is not a stylistic choice.

A service is the unit of publication, addressing and placement; half a service
cannot be offered. An interface is the unit of contract, and a consumer needs a
contract rather than a publication. Requiring a whole service would make every
unrelated interface in it the consumer's business, which `ridl diff` would then
report as affecting them.

Three things follow. **Least privilege**: the interfacing rules grant a consumer
only the regions of the interfaces it names. **Immunity to recomposition**: a
service that gains or splits interfaces does not touch its consumers. **A total
lookup**: the first consumer's rule that an interface has exactly one owning
service makes interface -> service -> machine resolve without ambiguity, which
is what makes consuming an interface well defined at all.

    V-06  MUST   Generation follows **imports**; wiring follows
                 **requires**. What code exists in a package follows that
                 package's interface imports (ADR-0002), which is what
                 keeps a package compilable alone. What is wired and what
                 access is granted follows a component's `requires`, at
                 service granularity. These are two questions and they were
                 conflated twice before being separated.

OSGi separates the same two with `Import-Package` at resolve time and the
service registry at run time. It reached that split decades earlier under the
same pressures.

## 4. The component

    V-07  MUST   A component is one execution context: a **sans-IO
                 synchronous step machine** driven by a **pump** that is
                 sync (a thread loop) or async (a runtime task). Which
                 pump, at what priority, is a deployment fact and never a
                 property of the step.
    V-08  MUST   A step performs no IO, never blocks, and always
                 terminates. The moment one awaits, neither pump works and
                 the step stops being testable without a runtime.
    V-09  MUST   A crossing between two components keeps identical
                 semantics whichever transport carries it, so an
                 intra-process crossing optimised into a direct channel
                 still behaves as the store and the queue do. Without this,
                 relocating a component becomes a code change, which is
                 what pulled ports exist to prevent.

**What a component supplies.** A synchronous model tree is blind twice over —
contract-blind (rmdl §7: a model names no service) and activation-blind (rmdl
§6.7: the scheduler has no clock, it has a timeline). A component fills exactly
those two gaps: a real interface, and an execution context. That is why rsdl
§1.3's three properties travel together, and it is also why the noun exists with
rmdl deferred: the two things the language must state need no model, and an
implementation is an optional attribute (§7).

    V-10  MUST   rsdl §1.3 assigns three properties to the component and
                 only one belongs there.

                     one reaction, one activation  -> component
                     failure containment           -> process; for freedom
                                                      from interference,
                                                      machine
                     independent deployment        -> distribution

                 Threads and runtime tasks in one address space do not
                 contain each other's faults, and nothing is deployed at
                 thread granularity. An assurance argument citing §1.3 for
                 freedom from interference between two components in one
                 process is citing a property that level does not have.
    V-11  MUST   Assurance level is a **process** attribute. Two components
                 at different levels cannot share an address space, and
                 placement must reject the mix. This is ADR-0018 open item
                 1 arriving as a checkable rule.

**Composites are dropped.** A composite — async between its children — is not
one reaction, so it is not a component under V-07; it is a grouping, and the
grouping that is actually needed is component-in-process-on-machine. The
recursion is AADL/SysML lineage. Hierarchical placement, the one thing it
bought, comes from dotted component names and a glob on the prefix.

    V-12  SHOULD Component boundaries are drawn by **timing** as much as by
                 concern. One component is one reaction and therefore one
                 rate: an interface at `@20ms` and one at `@500ms` in the
                 same component force the step to the tighter bound.

## 5. Machine, process, and what AUTOSAR calls them

    V-13  MUST   A **machine** is the unit of deployment and isolation,
                 covering a hypervisor host and its guests uniformly.
                 `vm` is rejected because the QNX host is a machine and
                 does not sit under the hypervisor; `host` and `guest` are
                 roles a machine holds, not kinds it belongs to.

Verified against the AP documentation: a Machine is "quasi a virtualized ECU-HW,
an entity where software can be deployed to", and one real ECU may run several.
Adaptive's deployment chain is Executable -> Process -> Machine, with machine
states deciding which processes are active.

**A target is not a machine.** rsdl §7 makes a target a _logical_ execution
context "named by capability class, never addressed", offering "node, ECU,
partition, container slot" — two hardware grains and two isolation grains,
undistinguished — and binds it to physical late, "potentially
runtime-discovered". Interfacing rules turn on isolation and containment, not on
capability class. When rsdl proper is written the two are probably two layers: a
target is what a deployment _requires_, a machine is what a build _has_, and
placement is the binding.

**`RTE` names the wrong layer.** It is Classic-only, and there it is generated
middleware for one ECU rather than a place. Its structural analogue here is
`ridl-rt` plus the emitted bindings. Adaptive has no RTE at all — ARA, a set of
linked APIs, replaces it, which is what `ridl-rt` is.

    this family   Adaptive               Classic            OSGi
    -----------   --------               -------            ----
    distribution  SoftwareCluster        - (ECU flash)      bundle
    shipped file  SoftwarePackage        -                  the JAR
    machine       Machine                ECU                -
    process       Process / Executable   OS-Application     -
    component     -                      OS Task            component (DS)
    service       ProvidedServiceInst.   -                  service registry
    interface     ServiceInterface       PortInterface      Java interface
    signal        field (get/set/notify) S/R data element   -
    event         event                  S/R, Trigger       event admin
    command       fire-and-forget method C/S op, no return  -
    query         method with return     C/S operation      -
    fixed         - (configuration)      ParameterInterface -
    codec + glue  ARA                    RTE (generated)    -

Three observations from the table. **signal ↔ field** is the strongest
correspondence and the one proto3 cannot express, which is why ADR-0013 records
DDS mapping cleanly and proto3 not. **`fixed` has no counterpart** anywhere. And
**protection domain is the row where the standards have something this model
lacks** — Classic's OS-Application is a first-class grouping for memory and
timing protection, which is ADR-0018 open item 1 word for word.

    V-14  MUST   An SWC is not this model's component. In Classic an SWC's
                 runnables are distributed across tasks at integration —
                 n:m by construction — where V-07 asserts 1:1. Fusing the
                 design unit and the execution context is right here, since
                 the n:m freedom exists to pack runnables onto small
                 microcontrollers, but the word must not be borrowed.

## 6. Catalogs

`interfaces/catalog-abi.md` §2 and §5 already settle this; restated because it
interlocks with the rest.

A catalog has three identities and only one appears in a frame: **name**
(reverse-DNS, used by attach, SELinux and manifests), **hash** (u64 over the
compiled catalog — this is the version), and **slot** (u8, assigned per
connection, the only one in the frame). Interface ids are allocated **within** a
catalog and the routing key is (slot, interface, member), which is what makes
catalogs independent with no global allocator — ABI-03.

    V-15  MUST   A catalog is declared **in ridl**, because it owns an id
                 space and a hash and both are compiler concerns, and
                 because `ridl diff` must classify a change to it. It
                 cannot come from a build flag: membership from a glob
                 renumbers silently and ABI-01's "one name, one set of
                 contents" has nothing to anchor to.
    V-16  SHOULD One package, one catalog, named for the package. ADR-0002
                 already makes package↔directory normative and package
                 names are already reverse-DNS, so the rule adds no syntax.
                 Catalog granularity then equals package granularity, and
                 ABI-07's guidance — finer costs file descriptors, coarser
                 costs a permission boundary that cannot be drawn later —
                 says erring fine is the safe direction.
    V-17  MUST   Interface ids within a catalog are **allocated and
                 recorded**, in a lockfile-shaped artifact per package.
                 Declaration order across a multi-file package is not a
                 derivation that survives a rename. This is ADR-0016
                 decision 8's mechanism one scope down, and it stays per
                 package, so there is still no global registry.
    V-18  MUST   A generation filter produces a **view**; it never produces
                 a catalog. A view is which contracts one consumer links,
                 derived from its requires-closure, and may differ per
                 consumer. A catalog is an identity with an id space, a
                 hash, a region and a label, identical for everyone.
    V-19  SHOULD Filter a view by **reachability**, not by name pattern. A
                 glob can silently omit an interface a component requires,
                 changes output when an unrelated package is added, and is
                 not minimal. The requires/offers graph already computes
                 the right closure.

Consequences already in the ABI: one region per catalog, each its own memfd with
its own SELinux label (ABI-07); the attach list is the grant list, so the
permission boundary, the mapping set and the routing namespace are one list
rather than three that drift (ABI-08); and a shared catalog is never specialised
per deployment — programme content is a separate catalog alongside (ABI-09),
which is exactly how a framework catalog and two programme catalogs compose.

## 7. What rsdl is left with

Four declarations, plus one recorded artifact.

    system        the closure — which components are in scope, the
                  assurance profile, the external boundary. Without it the
                  completeness checks have no set to quantify over and the
                  v1/v2 story has nothing to vary against.
    component     one execution context (§4): offers services, requires
                  interfaces, may have an implementation.
    distribution  what is versioned and shipped: contains components
                  (assets later), depends on other distributions, carries
                  a tier.
    deployment    named, several per system: declares machines, places
                  components on them.

    lock          the service-number registry — allocated and recorded,
                  never derived (ADR-0016 decision 8).

    V-20  MUST   `may have an implementation` keeps its "may". A component
                 with none is how an external system, a stub, or a
                 not-yet-written component is named so its interfaces
                 resolve — which is the one job rsdl §6's system root did
                 that nothing else does.
    V-21  MUST   Offer/consume and placement stay in **separate regions**.
                 Which service provides which interface does not change
                 between v1 and v2; which machine it runs on does.
                 Flattening them duplicates the composition per deployment
                 and lets the copies drift.

Dropped from rsdl's reference: composites and the component recursion; the
application-notation wiring layer (§4 — instances, fused provides,
destructuring, `let` intermediates, cycle rules); event→command wiring, which
needs a model to emit; declared redundancy; capability-class targets, replaced
by named machines; and RSDL-801 feasibility, deferred. Test topology comes free
as another `deployment` block.

The derivation that makes it worth writing: from `place` plus the offers and
requires edges, the emitter computes the **interfacing rules** — for each
(interface, consumer), the crossing kind, the transport, the access grant, and
whether `Access::CHECKED` may be relaxed. Crossing kinds are four, not three:
intra-process, inter-process on one machine, inter-machine, off-board.

## 8. Rejected names

Kept because the reasons outlive the decisions, in the shape of the 2026-08-10
naming pass.

    apex          Android's updatable-system-module container, mounted at
                  /apex. Live in this exact domain. Also rsdl's lattice
                  apex.
    apk           Android application package.
    package       typl/ridl keyword, where package↔directory is normative
                  (ADR-0002); also the Java/Android package name of the
                  catalog itself; also APK. The in-band descriptor puts a
                  distribution message in the same IR as the source
                  package, which is a permanent tax.
    bundle        clean, and rsdl §9 already defines it with RSDL-901 and
                  RSDL-902 attached — but `distribution` reads better to
                  the reviewers this has to survive.
    pack          short, and XPK shares its morphology, but collides with
                  pack-the-verb (SDF atlases) and reads informal.
    cluster       AUTOSAR Adaptive's own word (SoftwareCluster) and
                  unusable: cluster is the instrument display. The rare
                  case where the convention fails the collision test.
    module        Android mainline modules, Rust modules, ADR-0002's module
                  system.
    RTE           Classic's generated per-ECU middleware. Names the layer
                  `ridl-rt` and the emitted bindings occupy.
    dist / comp   every keyword in the family is a full word, and rmdl §1.4
                  shows these are deliberated rather than truncated. `dist`
                  additionally reads as a build output directory.

## 9. Open

**V-X1 — cross-catalog references.** `catalog-abi.md` ABI-X2: may a payload in
one catalog name a type in another? "Easy at compiler-design time, awful to
retrofit. A question for ridl, not answerable here." A framework catalog whose
types two programme catalogs use hits this on day one, so V-16's
one-package-one-catalog rule makes it urgent rather than hypothetical.

**V-X2 — whether a service number is needed at all on this transport.** The
routing key is (slot, interface, member), with no service in it. ADR-0016
decision 8 scoped its allocation question to tag-based transports; if the
routing key holds, the question does not arise here and a piece of machinery
leaves the plan. Confirm against the spike repo's copy of the ABI, which is
ahead of the one read for this note.

**V-X3 — where a distribution's members and their kinds are declared.** Assets
join components later. Keep the member kind explicit in the grammar even while
`component` is the only one, so adding `asset` is additive; and do not write "a
member belongs to exactly one distribution" as an invariant, because the asset
technote's override surfaces will need resolution order. Android's Runtime
Resource Overlay is the prior art, on this platform.

**V-X4 — cardinality and policy on `requires`.** OSGi's Declarative Services
puts both on a reference: cardinality (0..1, 1..1, 0..n) and policy (static,
restart on change; dynamic, rebind live). This model has neither, and "this trim
has no parking-aids service" is an optional reference and nothing else. Variant
handling is where it arrives.

**V-X5 — what a breaking _deployment_ change is.** Moving a service to another
machine breaks every interfacing rule derived from the old placement and leaves
the contract untouched. Neither existing `ridl diff` category fits. Adding a
`requires` is a third case: compatible for that service's consumers, breaking
for its integrator. E4.5a's stability policy is where these are decided or
quietly missed.

**V-X6 — `Busy` and the purity of a step.** `SignalWriter::commit` cannot fail,
but `EventSink::raise` returns `Busy`, and a step that must never block cannot
wait it out. The step needs a representable answer — drop and let the `seq` gap
be the evidence, or carry "could not emit" in its own state. Decide once, in the
step contract.
