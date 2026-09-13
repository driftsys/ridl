# rsdl rewrite — the decisions of 2026-09-12 and 2026-09-13

Status: working note, 2026-09-12, revised 2026-09-13. Settles the points the
rsdl rewrite (design note §3.1) has to fix before a new reference can be
written, fixes the language surface, and amends
[`2026-09-08-topology-vocabulary.md`](2026-09-08-topology-vocabulary.md) — §1,
§3, §7, the invariants V-05, V-06, V-10, V-11 and V-17, and the open items V-X1,
V-X2 and V-X3 — where the two disagree; §4 lists each amendment. Each decision
records its alternatives. Nothing here is ratified; until the rewrite lands, the
rsdl reference is the record and this note is the proposal. Read after the
vocabulary note and
[`2026-09-12-release-scope-and-plugin-system-design.md`](2026-09-12-release-scope-and-plugin-system-design.md)
§3.1, §3.8 and §3.13.

**Superseded as the record, 2026-09-13.** The rsdl reference v0.2.0
(`docs/specification/rsdl-language-reference.md`) states this note's language
and is now the record. Where the two differ, the reference governs. The
differences, each agreed with Sebastien on 2026-09-13:

- **Imports** follow typl §3.2 unchanged: `import veh.adas.LaneAssist` or a
  fully qualified name, and a service is named by its global dotted name. §2's
  `import veh.adas` and its rule that a component may name only what its file
  imports are withdrawn.
- **One reference per `offers` or `requires` line.** §2's
  `requires CruiseControl, LaneAssist` breaks R8, which makes a comma inside
  `{ }` a separator between body items.
- **One system per workspace** is stated without a citation: ADR-0002, which D-9
  cites, does not state it.
- **Rules this note leaves open are settled in the reference**: a bare component
  name in a machine body places every instance; an inline-shape service's name
  is the name of its one interface for `requires`; no two services in the
  closure list one interface; distribution membership; external machines and the
  surface set; a producer's write side is read from the producers fact, so D-9's
  grant wording stands, and the region an interface reaches is its own package's
  catalog, not the owning service's; an `.rsdl` file holds only the rsdl
  declarations; `ridl diff` lists closure and line changes under a second
  heading, "composition changed", beside D-11's "placement changed"; a backend
  key is spelled `someip.serviceId`, because typl's identifiers admit no
  underscore.

Why it exists: a recap of rsdl against the vocabulary note found five points the
note answers twice or not at all — the process noun, what is placed, posture,
the identity registry, and cross-catalog references — plus smaller ones. They
were settled one at a time. The identity question took two studies, run in a
fresh context against the current crates; their reports are summarised in D-7.
Recorded here so the rewrite starts from a file.

## 1. The shape

    distribution  what is versioned, signed and shipped
    machine       what it is installed on; the unit of placement
    component     one execution context; offers services, requires
                  interfaces; has instances
    service       what is addressed on the wire                     (ridl)
    interface     what is contracted                                (ridl)
    member        one typed interaction                             (ridl)
    catalog       a package's interfaces, their numbers, one hash   (ridl)

`process` leaves the list for rsdl's purposes (D-2). The three trees of the
vocabulary note §2 stand, with the runtime tree shortened to
`machine -> component`. V-02 stands unchanged: only what is addressed has
identity on the wire; distribution, machine, component and instance identifiers
never travel.

Five declarations, all containers, and nothing recorded by rsdl itself:

    system        the closure: the components a deployment must account for
    component     offers services, requires interfaces, may be external
    distribution  what ships together
    deployment    one placement of a system; contains machines
    machine       lists the instances it hosts

## 2. The language

Every declaration is the family's container shape (family-general-form §2, Shape
3): keyword, CamelCase name, optional relation clause, optional attribute block,
body. Every keyword is a full word. No colon is used.

    package veh.system
    import  veh.adas                  // names become usable; nothing is wired

    component Cruise [ instances = (primary, backup) ] {
      offers   veh.adas.cruise
      requires LaneAssist
    }
    component Lane    { offers veh.adas.lane }
    component Panel   { requires CruiseControl, LaneAssist }
    component Backend [ external ] { requires CruiseControl }

    system Vehicle { Cruise, Lane, Panel, Backend, veh.diag }

    distribution Adas [ tier = PLATFORM ]    { Cruise, Lane, veh.diag }
    distribution Hmi  [ tier = APPLICATION ] { Panel }

    deployment Production for Vehicle {
      machine AdasHpc [ labels = (ASIL_B) ] { Cruise.primary, Lane, veh.diag }
      machine Cockpit { Cruise.backup, Panel [ linux.cpuset = (2, 3) ] }
      machine Cloud   [ external ] { Backend }
    }

    deployment Bench for Vehicle {
      machine DevBox { Cruise, Lane, Panel, veh.diag, Backend }
    }

What a team has to know:

- Five declarations: `system`, `component`, `distribution`, `deployment`,
  `machine`.
- Two body lines inside a component: `offers` a service, `requires` an
  interface. The verbs differ because the sides differ (vocabulary note V-05):
  `offers LaneAssist` and `requires veh.adas.cruise` both read wrong, and that
  is the point. `provides` is retired. `requires` and the precondition attribute
  `require` never share a position, so the near-collision is accepted.
- One clause: `deployment X for Y`. Deployments are top-level so a bench
  topology can live in its own file.
- Three rsdl-owned attribute keys, `instances`, `external` and `tier`, plus the
  family's `labels`. Any other key is a backend's, namespaced `backend.key`
  (D-6).
- A system, a distribution and a machine list their members the same way. A
  member line is a reference, not a declaration, and may carry the attribute
  block.
- A lone service may stand in for its component (D-1). A component without
  `instances` has one copy, written by its bare name (D-4).
- Case carries the role, as everywhere in the family (R7): `Cruise` a component,
  `Cruise.primary` a declared instance (a member of its component, camelCase),
  `veh.adas.cruise` a service, `AdasHpc` a machine, `ASIL_B` a label, `PLATFORM`
  a tier value. The unit instance's name `Unit` is never written in source
  (D-4), so R7 does not govern it.
- `import` makes a package's names usable and nothing else. It generates code
  for the importer (ADR-0002) but never wires, grants or places; only a
  `requires` line does. A component may name only services and interfaces from
  packages its file imports; an unused import is a lint.

What the compiler derives, never written:

- which service an interface belongs to (ridl); which component offers it; which
  machine each instance is on;
- every link and its crossing kind: same machine, different machine, off-board;
- the grant per component, as the set of catalog regions its requirements reach;
- where each distribution is installed, and which distribution depends on which;
- the system's external boundary, from its external components and machines;
- a redundant provider set, from an offering component with more than one
  instance.

Retired from v0.1: `provides`, `target`, `place`, `on`, `transport`, `bundle`,
`time base`, `redundant`, `supervise`, `degraded`, `let`, `assurance`, and the
`<-` wiring arrow.

Consistency with the general form, stated plainly: the five declarations are
Shape 3 with `for` in the relation slot, as `states` and `realizes` are. Two
forms have precedent but no name in §2 and should be named there: the bare
member line (ridl's `service` lists its interfaces by bare name, ridl §14.5) and
the keyword-plus-reference line `offers X` / `requires X` (ridl's
`reserved Name` and the file-level `import` have that form). The general form's
Shape 3 table still carries an rsdl row for an `instance x: Type` manifest form
that the v0.1 reference does not have either; it should list the five containers
instead.

## 3. Decisions

### D-1 A lone service stands for an implicit component

**Decision.** A `service` declared in ridl that no declared `component` offers
stands for an implicit single-service component of the same name, listed by its
service name in a `system`, a `distribution` or a `machine`. The derived
component name appears only in those lists, in diagnostics and in the IR. An
implicit component is implemented; an external or stub provider needs an
explicit `component` with the `external` flag (D-9).

**Rejected.** (a) An implicit _service_ named after a component: the service is
the addressing unit (V-02, ridl §14.5) — its dotted name is published and
contributes the `service.member` addresses — so a name derived from the
component makes the component name an addressed identity, which V-02 forbids.
The attach list, the manifests and the security labels use the catalog name
(vocabulary note §6), not the service name. (b) Interfaces or interactions
declared inline in a `component`: the catalog is ridl's (V-15, V-16) and
`ridl diff` would have to read rsdl to classify a contract change; generation
follows imports and a consumer would import an rsdl file (V-06); an inline
interface has no name of its own, so a consumer would name the component and
lose immunity to recomposition. This is the v0.1 inline-member gradient the
vocabulary note dropped: every crossing now has a routing key, so a private
member outside a catalog no longer exists.

**Rule.** Derive the unaddressed noun from the addressed one, never the reverse.

### D-2 No process declaration, no scheduling facts

**Decision.** rsdl declares no process and carries no pump kind, priority or
other scheduling fact. Placement is an instance of a component on a machine,
nothing finer. Crossing kinds in rsdl are three: same machine, different
machine, off-board. The grant list is derived per component.

**Consequences.** Whether two components on one machine share an address space
is the backend's or the host program's decision; under V-09 the direct-channel
case is an optimisation with identical semantics. A backend that groups
components into processes, or Classic into OS-Applications, unions the grants
per group. V-11's mixed-level check leaves rsdl: a machine is certified for a
level, and a component that demands a higher level placed on it is a lint under
the automotive profile (via `labels`, D-6), out of this release. V-10's "failure
containment belongs to the process" becomes a statement about the target, and
the vocabulary note's noun list drops `process` for rsdl.

**Rejected.** A `process` block under `machine`, carrying the assurance level,
with pump and priority on the placement line. What a process is differs per
target — Classic has none, Adaptive has Process, baremetal has one address space
— and scheduling is the backend's and the engine's. What would reopen a grouping
noun: a consumer whose safety argument needs the mix rejected at design time;
then an optional partition grouping under `machine`, with a single implicit
partition when none is declared.

### D-3 A machine lists the instances it hosts

**Decision.** Placement is membership: a `machine` body lists instances. A
service's machine is derived, never declared: an interface has one owning
service, a service has one offering component, an instance of that component is
in one machine per deployment. Two rules, both errors:

- every service in the closure is offered by exactly one component — zero is a
  missing provider, two is a conflict (D-4 makes redundancy a derived case of
  one component, so it is not an exception);
- every instance of every component in the closure appears in exactly one
  machine per deployment — the v0.1 RSDL-701 rule at instance grain. An external
  system is an `external` component in an external machine, not an exception.

**Rejected.** (a) Placing the service (vocabulary note §3, the
roadmap-simplification note's S-35): a consumer-only component offers nothing
and still needs a machine before its crossing kinds exist, and a component
offering two services must put both on one machine. (b) `place X on M` lines:
two keywords for what membership says, and the list form makes `system`,
`distribution` and `machine` one sentence with three verbs (R9).

**Amendment.** Vocabulary note §3: "the unit of publication and addressing".

### D-4 Multiplicity is an attribute; redundancy is derived

**Decision.** `[ instances = (primary, backup) ]` on a component, per
family-general-form §4.8 (attribute first) and R3 (brackets have two positions,
so no array syntax on a name). Instances are named, never numbered.

- No attribute gives exactly one instance, the **unit instance**, named `Unit`
  in the IR and in diagnostics and written by the bare component name: `Cruise`
  in a machine body is `Cruise.Unit`. A lone service's implicit component (D-1)
  has its unit instance and nothing else. Declaring `instances` replaces the
  unit instance rather than adding to it.
- Instance names are unique within a component; a duplicate is an error, and
  `Unit` may not be declared explicitly. `Unit` is an IR and diagnostic name,
  not a source spelling: R7 keeps a declared instance name camelCase, so the
  CamelCase `Unit` cannot collide with one.
- Instances share the component's offers and requires and are indistinguishable
  on the wire (V-02).
- **Redundancy is a derived fact.** An offering component with more than one
  instance is that service's redundant provider set. The lowering reports it as
  not yet realizable until the runtime has arbitration (failover, voting, two
  writers on one member — engine questions, parked). A redundant pair in one
  machine is a profile lint later.

**Rejected.** (a) A reserved `redundant` keyword on the `system`: multiplicity
is the general mechanism and redundancy one consequence. (b) Inferred numeric
names `Cruise.1`: a numeric segment is new to R7, and a name tells a team what
the copy is for.

**Limit, deliberate.** Instances share their offers, so a left sensor and a
right sensor offering two different services are two component declarations.
Per-instance service binding would bring back the instance-versus-kind split
dropped with the application notation.

### D-5 rsdl lowers the crossing kind; transport and topology are configuration

**Decision.** Each link in the system IR carries its crossing kind and its two
endpoints, nothing else about the physical layer. The transport over each
crossing, and the network and machine fabric — VLANs, VM placement, CPU clusters
— are the implementer's configuration. If they ever enter rsdl they arrive as
attributes on `machine` or a block under `deployment`, earned by a backend that
reads them (D-6).

**Posture** (v0.1 §8) is kept as a word in a reserved section of the rewritten
reference: a `service` stays posture-neutral in ridl; rsdl derives no posture in
this release; posture derivation, RSDL-803 and RSDL-801 reopen with a bus-class
backend. The family overview's doctrine 18 keeps its first half (a `service` is
posture-neutral), marks the derivation deferred inline — the overview's own form
for a deferral, as doctrines 5 and 15 use it — and re-cites the reserved section
instead of "rsdl §8"; every other site that cites "rsdl §8" for posture
derivation does the same, and the open-question index entry, which cites rsdl
§13, is marked deferred (§4).

**Rejected.** (a) A `transport { ... }` table in the grammar: one transport
family exists. (b) Dropping the word posture: the Classic-to-Adaptive migration
argument should stay findable outside the archive.

### D-6 Attributes are the backend escape hatch

**Decision.** Every rsdl declaration and every member line takes the family
`[ ]` attribute block with its three forms (family-general-form §4.2).

- **rsdl-owned keys** are allow-listed per declaration kind; an unknown key is
  an error; every key has a machine consumer (§4.1). The keys: `instances`
  (component), `external` (component, machine), `tier` (distribution, values
  `PLATFORM` and `APPLICATION`, optional), and the family's `labels`.
- **Backend keys are namespaced by the backend name** — `someip.service_id`,
  `linux.cpuset`, `rust.crate`. The compiler carries a namespaced key into the
  IR uninterpreted: the system IR gains an **attribute map per node**, alongside
  the region map, link set, routing table, permission list, surface set and
  catalog hash of design note §3.13, and every extract a backend reads carries
  it. The backend validates its namespace through the plugin contract, which
  gains one line: a backend declares the keys it consumes. A namespace no
  configured backend claims is a warning.

This is the single "later" mechanism for the certified-level lint (D-2), the
fabric facts (D-5), a tag-based transport's service number (D-7), and a
component's implementation (D-9).

### D-7 Identity: numbers live outside the source, at every level

**Principle.** The family already decided for members that declaration order is
wire identity and that the number is shown by tooling, never written
(family-general-form §6.3: inlay hints, a baseline-aware compiler, no syntax).
Interfaces follow the same rule. Materialising numbers inline in the source is a
possible later feature, not a per-level choice: a one-way `materialize` command
that writes every frozen number — interface numbers from the lock and member
ordinals from position — into the source at every level at once, to settle
identity forever. Never for one level alone.

**What needs a number.** The runtime routes by catalog slot (per connection),
interface number within the catalog, and member ordinal within the interface.
Member ordinals come from position in the body (ridl §11). A service has no
number: it is identified by its published dotted name, and the routing key does
not contain it (V-X2 answered no; ADR-0016 decision 8 stands as scoped, and a
tag-based transport's service id is that backend's attribute, D-6). The
interface number is the only one with no source, because a package spans files
and no position across files survives a file rename or a move.

**Decision.** One generated lock file per package, inside the package directory,
checked in, written only by `ridl lock`:

    next 4
    CruiseControl  1
    LaneAssist     2   retired
    LaneKeeping    3

- An entry is never changed or removed; `next` is never lowered; a retired entry
  holds its number forever.
- **Floating until frozen.** Between locks a new interface has a provisional
  number for the build, after the frozen ones, in a deterministic order, and the
  IR marks it provisional. `ridl diff` treats a provisional number as no
  identity. `ridl baseline` refuses to publish a package with a provisional
  number or with a removed interface that has no retired entry. Branches never
  allocate, so branches never collide. The gate sits at publish rather than at
  the build because allocation is explicit here: a build-time refusal would
  force every branch to allocate, which is the collision this carrier exists to
  prevent. The study that recommended failing the build assumed a number stamped
  in the source, where allocation happens as the file is edited; the carrier
  changed, so the gate moved with it.
- **`ridl lock` is its own command**, run by the release recipe before the
  version bump, or by hand when stability is needed earlier. `ridl fmt` never
  assigns identity. A repository whose `main` is consumed directly runs the same
  command in its merge queue; only then does the file change in parallel, and
  only then is a three-way merge driver (`ridl lock merge`) needed — a union
  driver was measured to resurrect a renamed entry silently.
- **Rename of a frozen interface.** An LSP rename may update declaration and
  lock together; whether it does is an implementation choice (§5), so the
  guarantee is the build-side protocol, not the editor. In a plain editor the
  build sees one entry without a declaration and one declaration without an
  entry: with the same shape as the baseline's, member for member, it is a
  rename and the entry follows; otherwise the build asks once, "rename the
  entry" or "retire and allocate". Two orphans with identical shapes also ask.
- **The compiler folds the number into the IR**, and `ridl diff` matches
  interfaces by number: appended is compatible; renamed with the same number is
  compatible once ADR-0015 decision 17 keys binding ordinal spaces on (package,
  number) instead of the name — a ruling; retired with an entry is compatible; a
  number changed by hand is breaking. Interaction-level verdicts are unchanged.
- **A rename is compatible on the wire and breaking for generated code, and
  `ridl diff` reports it under its own heading.** Both wire backends derive a
  top-level name from the interface: `ridl-backend-proto` and
  `ridl-backend-flatbuffers` each emit an `<Interface>Ordinal` identity-table
  enum, claimed in the target's own name scope (ADR-0013 decision 3, under the
  scope obligation of ADR-0017 decision 4 and ADR-0019 decision 5). A
  same-number rename leaves every wire number untouched and still renames
  generated names, differently in each target: in proto3 the enum and every
  value in it, because that backend prefixes each value with the
  interface-derived name; in FlatBuffers the enum's own name alone, because
  FlatBuffers scopes values inside their enum and prefixes none of them
  (ADR-0019 decision 5). No projection property is broken — ADR-0016 decision
  6's stability clause is written over numbers, and ADR-0017 decision 4's
  totality over names refuses collisions rather than promising that a name
  persists — so neither projection record is amended. What changes is what the
  verdict means: `Compatible` would otherwise imply that nothing in a consumer's
  generated code was renamed, and here it does not. A rename therefore gets its
  own heading, as D-11 gives a deployment change one: compatible on the wire,
  source-breaking for a consumer of the generated identity table.
- **Bodies stay positional.** Struct fields, union arms and interactions keep
  position and `reserved name`; an enum value keeps its explicit integer, and
  its tombstone is `reserved <value>`. A body gives one order; a per-field lock
  would conflict on every feature-branch merge; FlatBuffers needs dense ids.
  Their numbers are inlay hints, as decided.
- **A service's interface list becomes a set.** Order carries no wire meaning,
  so ADR-0015 decision 15's slot model on that list retires with RIDL-146 to
  RIDL-148, together with decision 18's RIDL-146 (RIDL-144 and RIDL-145 stay),
  decision 19's `ServiceShape*` diff categories and decision 24's rules behind
  RIDL-147 and RIDL-148; an interface has exactly one number, from its catalog.
- **The catalog hash is derived**, over the interfaces, their numbers, and the
  types they reach (D-8).

**Rejected, with the reason each received.** (a) A stamped `[ id = N ]`
attribute on the interface: rename-free and the smallest tooling, but the
family's first wire number in source, against §6.3. (b) A `catalog { ... }` list
block in source: position, so tidying the list is a wire break, and a merge
either conflicts at one anchor or renumbers silently (measured at the
service-list level today). (c) Position across files: turns file order into wire
identity. (d) A name hash: rejected in ADR-0016 decision 8. (e) `ridl.lock`: per
workspace, regenerated, does not ship with a package. (f) The `.ridl/baseline`
snapshot as the record: keyed by name, so a rename is a removal plus an
addition. (g) Stamping the service: recomposition changes an interface's number,
a service may list another package's interface, and an interface with no service
has none. (h) A hybrid of inferred and explicit numbers: unstable under a merge.

**Defect found in the current crates, filed as driftsys/ridl#315.**
`ridl
baseline` publishes a removed interaction with no `reserved` tombstone (it
runs no desk check; RIDL-407 belongs to `ridl check`, which reads
`.ridl/baseline/` at the workspace root or the `--baseline` path, and is a
warning), a later append reuses the ordinal, and `ridl diff` reports the append
as compatible. The baseline gate above closes it at both levels. Swapping two
struct fields is reported breaking only through the `constraint_changed`
fallback; a reorder category is missing (driftsys/ridl#314).

### D-8 Cross-catalog type references are allowed; the hash covers the closure

**Decision.** A payload in one catalog may name a type declared in another —
ADR-0002 imports and ADR-0017's projection of a foreign reference already allow
it — and a catalog's hash covers its interfaces plus every type they reach,
transitively, wherever declared. A change to a framework type changes the hash
of every catalog that uses it. The routing key and the permission boundary are
untouched: types never travel, and attaching a programme catalog's region never
requires the framework's. `ridl diff` classifies through the same closure. Lands
as the disposition of V-X1 in the ridl finalization.

**Rejected.** Requiring every catalog to declare the types it uses: copies a
shared vocabulary into every package.

### D-9 The smaller points

- **A distribution may span machines.** Installation is derived from where its
  instances are placed. No `on machine` clause, and no `depends` clause:
  dependency between distributions is derived from a `requires` that resolves
  across a distribution boundary.
- **rsdl knows one fact about an implementation: present or not.** The flag
  `[ external ]` marks a component with none; the default is implemented. What
  the implementation is — a crate, a language, later a model — is a backend key
  (D-6).
- **The grant, stated once.** A consumer names interfaces (V-05); the lowering
  resolves each through its one owning service to its catalog; the grant is the
  set of catalog regions reached (vocabulary note §6). V-06's "at service
  granularity" is reworded to "resolved through the owning service".
- **ridl §14.6** is rewritten to "a component offers services and requires
  interfaces". A ridl finalization edit.
- **One system per workspace** stays (ADR-0002, v0.1 §6).
- **Machines are CamelCase** (`AdasHpc`), because a machine has a body and is a
  container (R7).

### D-10 `system` is the closure and nothing else

A `system` is the named set of components a deployment has to account for. It
has no composite body, no wiring, and no boundary of its own: the external
boundary is derived from its `external` components and machines. The assurance
profile slot is profile-gated and reserved. Every completeness check quantifies
over the closure; every `deployment` is `for` one; `ridl diff` compares at it.
Under D-1 a system of hand-written services is a list of service names.

### D-11 Dispositions of the items left open on 2026-09-12

- **Cardinality on `requires`** (V-X4): every `requires` is mandatory with one
  provider in this release. An `optional` flag on the line is the
  attribute-first path when variant handling arrives.
- **A breaking deployment change** (V-X5): `ridl diff` classifies contracts
  only; a deployment change is listed under its own "placement changed" heading
  with no verdict. E4.5a decides the verdicts.
- **`Busy` in a step** (V-X6): default to drop and count, the sequence gap is
  the evidence, the step stays pure; a component that cannot lose an event keeps
  it in its own state and retries. Lands in the runtime note.
- **Member kind in a distribution** (V-X3): no kind keyword. A member line is a
  bare reference, and its kind is the kind of the declaration the name resolves
  to (§2), so a later `asset` declaration lists additively with no grammar
  change. Reopens if a member kind ever has to exist without a declaration.
- **Timing feasibility** (RSDL-801): out of this release, reopens with posture.

## 4. Amendments implied for existing records

- **Vocabulary note**: §1 drops `process` for rsdl and V-10, V-11 become
  statements about the target and the profile (D-2); §3 "unit of publication and
  addressing" (D-3); V-05, V-06 reworded (D-9); §7 becomes the five containers
  with no `lock` line (D-7); V-17 "recorded in a generated lock file per
  package"; V-X1, V-X2 and V-X3 answered (D-8, D-7, D-11).
- **Design note**: §3.8 gains "a backend declares the keys it consumes" and
  §3.13 gains the attribute map per node (D-6); §4 "rsdl's noun set" resolved:
  `component` stays, `process` goes (D-2, D-3).
- **Family general form**: §2 Shape 3 table lists the five rsdl containers and
  names the member line and the keyword-plus-reference line (§2 of this note);
  §4.3 gains the three rsdl keys; §6.3 extends to interface numbers (D-7).
- **Family overview** doctrines 16 and 17: "provides/requires services" becomes
  "offers services and requires interfaces" (D-9); doctrine 18, the inventory
  row that gives rsdl "transport/posture derivation" and ledger row 28: posture
  derivation marked deferred, the reserved section re-cited in place of "rsdl
  §8"; the open-question index entry, under rsdl §13, marked deferred (D-5); the
  inventory row's "providing/requiring services", ledger row 27's
  `provides`/`requires` boundary and ledger row 29's declared redundancy:
  reworded to the new surface (D-3, D-4, D-9).
- **ridl reference**: §11 gains the baseline gate; §14.5 amended — its
  "components _provide_" wording (D-9), the "declared redundancy" paragraph
  under "Composing interfaces" (D-3 and D-4 reverse it: one offering component
  per service, redundancy derived from instances), and ADR-0015 decisions 15,
  17, 18 (RIDL-146 only), 19 and 24 (D-7); §14.6 wording and the glossary entry
  for _service_ (D-9); the posture sentences of §14.5, the transport and
  feasibility paragraph, and the glossary entries _service_ and _posture_
  re-cite the reserved section and mark the derivation deferred (D-5); V-X1
  disposition (D-8).
- **ADR-0010**: the `lock` subcommand and `lock merge` follow its conventions.
- **ADR-0017 and ADR-0019**: checked against D-7 and **unaffected**. Neither
  keys a rule on an interface's number — that identity is new here and belongs
  to the runtime and dispatch layer, never to a wire schema — and neither a
  retired entry nor a provisional number reaches either backend. Both do depend
  on an interface's name, through the identity table, and D-7 leaves naming
  untouched; the rename consequence is recorded in D-7 instead. Written down so
  the question is not reopened.
- **ADR-0011 and ADR-0018**: each cites rsdl §8 for a matter other than posture;
  both citations are re-pointed when the rewrite renumbers the section.
- **Roadmap**: the E6 stories left untouched by the re-scope are refiled against
  this note once the rewrite starts.

## 5. Open

Rulings only, none blocking the rewrite:

- the rename verdict on the name-keyed bindings of ADR-0015 decision 17, and
  whether removing an interface from a service is breaking (D-7);
- the width of the interface number in the frame, outside this repository;
- whether the LSP rename writes the lock, an implementation choice.

## 6. Studies

Two reports, written in a fresh context against the current crates, sit beside
this note as
[`2026-09-12-interface-id-study.md`](2026-09-12-interface-id-study.md) and
[`2026-09-12-interface-id-study-2.md`](2026-09-12-interface-id-study-2.md). The
first ranked the carriers and found the reuse defect above; the second simulated
the merges with git and found the union-driver resurrection and the "next free
number" hole that `next` closes. Their measurements are the basis of D-7's
rejections; the first's recommendation (the stamped attribute) was overridden by
the §6.3 consistency argument, recorded above; the second's lock model is D-7 as
written.
