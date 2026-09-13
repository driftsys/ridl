# rsdl Language Reference

**Reactive System Description Language** — the architecture layer of the RIDL
family: which components a system is made of, which services each offers and
which interfaces each requires, what ships together, and where each copy runs.

Version: 0.2.0 — Draft

> **Provenance and supersession.** This document replaces the rsdl reference
> v0.1.0 as a whole. v0.1.0 described components as situated reactions, wired
> them with application notation, placed them on capability-class targets,
> derived transport and posture, and shipped them in bundles; every one of those
> constructs is retired here, and v0.1.0 is kept for provenance at
> [`../archive/rsdl-language-reference-v0.1.md`](../archive/rsdl-language-reference-v0.1.md).
> The language stated here was settled in the rsdl rewrite decisions of
> 2026-09-12 and 2026-09-13 (decisions D-1 to D-11), over the topology
> vocabulary of 2026-09-08. Doc comments (`/** … */`, `///`) are CommonMark, as
> everywhere in the family.

---

## Table of Contents

1. [Scope and Position in the Family](#1-scope-and-position-in-the-family)
2. [Files, Packages, Imports and Keywords](#2-files-packages-imports-and-keywords)
3. [The Five Declarations](#3-the-five-declarations)
4. [Member Lines and Body Lines](#4-member-lines-and-body-lines)
5. [Attributes](#5-attributes)
6. [The Implicit Component](#6-the-implicit-component)
7. [Instances and Redundancy](#7-instances-and-redundancy)
8. [Resolution](#8-resolution)
9. [Placement](#9-placement)
10. [Crossing Kinds](#10-crossing-kinds)
11. [Grants](#11-grants)
12. [Reserved and Deferred](#12-reserved-and-deferred)
13. [What the Lowering Produces](#13-what-the-lowering-produces)
14. [`ridl diff` at the System](#14-ridl-diff-at-the-system)
15. [Conventions](#15-conventions)
16. [Diagnostics](#16-diagnostics)
17. [Open Questions](#17-open-questions)

- [Appendix A — Full Example](#appendix-a--full-example)
- [Appendix B — Formal Grammar (EBNF)](#appendix-b--formal-grammar-ebnf)
- [Appendix C — Glossary](#appendix-c--glossary)

---

## 1. Scope and Position in the Family

### 1.1 What rsdl describes

rsdl answers one question: _what is this system made of, and where does each
part run?_ A `.rsdl` file names the components of a system, the services each
component offers and the interfaces each component requires, the distributions
that ship them, and, per deployment, the machine that hosts each copy of each
component. The compiler lowers that description to the facts a runtime reads
(§13): who produces each service, every link between a consumer and a producer
with its crossing kind, the routing table, the grants, and the system's external
boundary.

rsdl records nothing of its own. The contracts are ridl's: a `service`, its
interfaces and their members are declared in `.ridl` files, and the numbers that
identify an interface on the wire come from the per-package lock file that ridl
maintains (§13). Behaviour is rmdl's and is not referenced in this release
(§12). What rsdl adds is structure — which parts exist, how they relate, and
where they run — and every fact it lowers is derived from that structure.

### 1.2 Position in the family

rsdl is the apex of the family lattice, `typl ← {ridl, rmdl} ← rsdl`. It is the
one layer that can never stand alone: a component offers services and requires
interfaces that only ridl can declare. rsdl composes by import and never by
inclusion, with the family's import rules unchanged (§2).

### 1.3 Three trees, not one hierarchy

A component appears in three trees, related to each in a different way:

| Tree         | Shape                               | What it is for         |
| ------------ | ----------------------------------- | ---------------------- |
| distribution | distribution → components           | version, sign, install |
| runtime      | machine → component instance        | execute                |
| contract     | service → interface → member (ridl) | publish and address    |

The nouns, each answering one question:

| Noun         | Answers                                                     | Layer |
| ------------ | ----------------------------------------------------------- | ----- |
| distribution | what is versioned, signed and shipped                       | rsdl  |
| machine      | what software is installed on; the unit of placement        | rsdl  |
| component    | one execution context; offers services, requires interfaces | rsdl  |
| service      | what is addressed on the wire                               | ridl  |
| interface    | what is contracted                                          | ridl  |
| member       | one typed interaction                                       | ridl  |
| catalog      | a package's interfaces, their numbers, one hash             | ridl  |

**Only what is addressed has identity on the wire.** A service's dotted name and
a catalog do; distribution, machine, component and instance names never travel.
They are build-time and deployment-time names, and a runtime artifact that
carried one would couple two layers that are separated on purpose.

### 1.4 The component and what rsdl does not declare

A component is one execution context. rsdl declares no process, no pump kind, no
priority and no other scheduling fact (§12). Whether two components on one
machine share an address space is the backend's or the host program's decision;
a crossing between two components keeps the same semantics whichever transport
carries it, so that decision is an optimisation and never a change of meaning.

rsdl does not declare the transport over a crossing, or the network and machine
fabric. Both are the implementer's configuration (§10). A backend that needs a
fact rsdl does not own reads it from a namespaced attribute key (§5).

### 1.5 One system per workspace

A workspace declares at most one `system` (§3.1). The system is the closure
every completeness check quantifies over, every `deployment` is `for` it, and
`ridl diff` compares at it (§14).

---

## 2. Files, Packages, Imports and Keywords

An `.rsdl` file is a restricted profile of the family grammar (concept note §4):
it holds the five declarations of §3 and nothing else. A type, an interface or a
service declared in an `.rsdl` file is RSDL-604; the catalog is ridl's
(vocabulary V-15) and the vocabulary is typl's.

- **Package.** Every `.rsdl` file begins with one `package` declaration (typl
  §3.1). A package is a directory (ADR-0002) and may hold `.typl`, `.ridl` and
  `.rsdl` files together. A package may hold several `.rsdl` files: a bench
  deployment commonly lives in its own file.
- **Imports** follow typl §3.2 unchanged: a single named import
  (`import veh.adas.LaneAssist`) or a fully qualified name in place. There are
  no wildcards, no relative imports and no re-exports (TYPL-003). An unused
  import is TYPL-007. A **service** is named by its global dotted name
  (`veh.adas.cruise`) and is never imported (ridl §14.5).
- **An import makes names usable and does nothing else.** It generates code for
  the importer (ADR-0002) but never wires, grants or places; only a `requires`
  line does (vocabulary V-06).
- **One system per workspace.** A workspace declares at most one `system`
  (§3.1); a second is RSDL-601.
- **Modifiers.** The five declarations take no `internal` modifier in this
  release: the closure is workspace-wide, so every declaration it names has to
  be visible from the system's package. `internal` before an rsdl keyword is a
  parse error (FORM-102).

**Keywords** the rsdl profile uses, all full words, no colon anywhere:

```
system  component  distribution  deployment  machine  offers  requires  for
```

plus typl's `package`, `import` and `as`. Every family keyword is reserved in
every profile (typl §1.4). The v0.1 words `provides`, `instance`, `assurance`,
`target`, `place`, `on`, `transport`, `bundle`, `time`, `base`, `redundant`,
`supervise` and `degraded`, and the `<-` wiring arrow, are retired; a retired
word leaves the registry unless another profile uses it (`let` stays, rmdl's),
and leaving the registry makes it a legal identifier in every profile — a
compatible widening. `requires` (a component line) and the predicate attribute
key `require` (general form §4.3) never share a position: `require` is
recognised only inside `[ ]`, and no rsdl declaration or line admits it.

**Case carries the role** (general form R7):

| Spelling           | Role                                                 | Example              |
| ------------------ | ---------------------------------------------------- | -------------------- |
| `CamelCase`        | component, system, distribution, deployment, machine | `AdasHpc`            |
| `camelCase`        | instance — a member of its component                 | `Cruise.primary`     |
| `lowercase.dotted` | package, service                                     | `veh.adas.cruise`    |
| `SCREAMING_SNAKE`  | label, tier value                                    | `ASIL_B`, `PLATFORM` |

The unit instance's name `Unit` (§7) is never written in source, so R7 does not
govern it.

---

## 3. The Five Declarations

Every declaration is the family's container shape (general form §2, Shape 3):
keyword, CamelCase name, optional relation clause, optional attribute block,
body. Only `deployment` has a relation clause, `for`, in the slot `states` and
`realizes` use elsewhere (R5). Attributes sit between the name (or the clause)
and the opening brace (general form §4.4). Bodies follow R8: newline and comma
interchangeable, trailing comma legal, no semicolons. Doc comments (typl §14)
may precede any declaration or body line.

| Declaration    | Clause       | Body holds                                            | rsdl-owned keys                                 |
| -------------- | ------------ | ----------------------------------------------------- | ----------------------------------------------- |
| `system`       | —            | member lines: components, lone services (§4)          | `labels`, `deprecated`; one slot reserved (§12) |
| `component`    | —            | `offers` and `requires` lines (§4)                    | `instances`, `external`, `labels`, `deprecated` |
| `distribution` | —            | member lines: components, lone services               | `tier`, `labels`, `deprecated`                  |
| `deployment`   | `for System` | `machine` declarations                                | `labels`, `deprecated`                          |
| `machine`      | —            | placement lines: components, instances, lone services | `external`, `labels`, `deprecated`              |

Every declaration and every line also takes backend keys (§5). A key not in a
declaration's row is FORM-107; a key no row and no backend namespace defines is
FORM-106; a key twice in one block is FORM-108.

### 3.1 `system`

A `system` is the **closure**: the named set of components a deployment has to
account for, and nothing else. It has no wiring, no boundary of its own and no
composite body; the external boundary is derived from its `external` components
and machines (§13).

```rsdl
system Vehicle { Cruise, Lane, Panel, Backend, veh.diag.access }
```

- A member line names a declared component or a lone service (§6). A line that
  names neither is RSDL-602; a name listed twice in one body is RSDL-603.
- The closure is the listed components plus the implicit components of the
  listed services. A service is in the closure when a closure component offers
  it; an interface is in the closure when a closure service lists it.
- A component the closure does not list is not lowered. No diagnostic.
- Every completeness check of §8, §9 and §3.3 quantifies over the closure; every
  `deployment` is `for` the system; `ridl diff` compares at it (§14).
- A workspace with no `system` lowers nothing and draws no diagnostic; a second
  `system` is RSDL-601.

### 3.2 `component`

A component is one execution context (vocabulary V-07). It **offers** services
and **requires** interfaces; the verbs differ because the sides differ (V-05): a
service is the unit of publication and addressing, an interface the unit of
contract, and half a service cannot be offered.

```rsdl
component Cruise [ instances = (primary, backup) ] {
  offers   veh.adas.cruise
  requires LaneAssist
}
component Lane    { offers veh.adas.lane }
component Panel   { requires CruiseControl, requires LaneAssist }
component Backend [ external ] { requires CruiseControl }
```

- `offers` takes one service reference. A name that is not a service is
  RSDL-310.
- `requires` takes one interface reference. An inline-shape service's dotted
  name is the name of its one interface (ridl §14.5, group identity), so
  `requires veh.diag.access` is legal. A service with a list of shapes after
  `requires` is RSDL-311, and the diagnostic lists its interfaces; a name that
  is neither an interface nor an inline-shape service is RSDL-312.
- One reference per line. A second reference on the same line is a new body item
  (R8), so `requires A, B` is a parse error at `B` (FORM-102); write
  `requires A, requires B`.
- The same service on two `offers` lines, or the same interface on two
  `requires` lines, is RSDL-309.
- A component that requires an interface listed by a service it offers is
  RSDL-308: the consumer would sit in its own provider set (§8). Reopens with
  arbitration (§12).
- A component with no lines is legal and lowers to an instance with no links.
- `instances` gives the component named copies (§7); `external` marks a
  component with no implementation in this workspace (§9, §13). An external
  component has the same body and keys as any other.

### 3.3 `distribution`

A distribution is what is versioned, signed and shipped together (vocabulary
§1). Its body is bare member lines (§4): components and lone services. It
declares no machine and no dependency; both are derived (§13).

```rsdl
distribution Adas [ tier = PLATFORM ]    { Cruise, Lane, veh.diag.access }
distribution Hmi  [ tier = APPLICATION ] { Panel }
```

- A member line names a closure component or a lone service whose implicit
  component is in the closure. A name outside the closure, or unknown, is
  RSDL-903; a name listed twice in one body is RSDL-906.
- When the workspace declares at least one distribution, every implemented
  closure component is in exactly one: in none is RSDL-904, in two is RSDL-905.
  An external component is in none; listing one is RSDL-907. A workspace with no
  distribution derives no installation and no dependency.
- `tier` is optional, `PLATFORM` or `APPLICATION` (§5). A `PLATFORM`
  distribution holding a component whose `requires` resolves (§8) to a component
  in an `APPLICATION` distribution is RSDL-901; a distribution without `tier`,
  on either side, is exempt.
- These rules are stated for components. A later `asset` declaration lists
  additively with no grammar change (§12) and brings its own rule.

### 3.4 `deployment`

A deployment is one placement of the system. It is a top-level declaration,
`for` the system, and contains `machine` declarations and nothing else.

```rsdl
deployment Production for Vehicle {
  machine AdasHpc [ labels = (ASIL_B) ] { Cruise.primary, Lane, veh.diag.access }
  machine Cockpit { Cruise.backup, Panel [ linux.cpuset = (2, 3) ] }
  machine Cloud   [ external ] { Backend }
}
```

- `for` names the workspace's system, bare or qualified. A name that resolves to
  no declared `system` is RSDL-704.
- Deployment names are unique in the workspace: two deployments with one name is
  RSDL-708. Several deployments `for` one system are the normal case: a
  production topology and a bench topology differ only here.
- A deployment declares no transport, no fabric and no time base; those are
  configuration (§10, §12).

### 3.5 `machine`

A machine is the unit of placement: it lists the instances it hosts (§9).
Machines are CamelCase because a machine has a body and is a container (R7). A
machine is declared only inside a deployment; its name is unique within that
deployment, and a duplicate is RSDL-705. The same name in two deployments names
two machines.

- A placement line names a component (every instance), one instance, or a lone
  service (its unit instance). The rules are §9.
- `external` marks a machine outside the built system — a cloud endpoint, a peer
  vehicle, a bus node the workspace does not build. An external machine hosts
  external components only (RSDL-707, §9).

---

## 4. Member Lines and Body Lines

Two line forms occur in bodies, both references and neither a declaration:

- **The bare member line** — a reference alone, as ridl's `service` lists its
  interfaces (ridl §14.5). It is the body item of `system`, `distribution` and
  `machine`. Its kind is the kind of the declaration the name resolves to
  (D-11), so no kind keyword is written.
- **The keyword-plus-reference line** — `offers X`, `requires X`, as ridl's
  `reserved Name` and the file-level `import`. It is the body item of
  `component`.

Either form may carry the attribute block at end of line (general form §4.4); a
line takes backend keys only (§5). A line's attribute node is the line within
its container (§13).

**The reference.** A reference is a dotted name of one or more segments, and its
role is read from case (R7), then confirmed by lookup:

| Form            | Names                                         | Where legal                      |
| --------------- | --------------------------------------------- | -------------------------------- |
| `Name`          | a component in scope; in `for`, the system    | every body; `for`                |
| `Name.inst`     | one declared instance of a component          | `machine`                        |
| `pkg.Name`      | a component (or the system) by qualified name | as `Name`                        |
| `pkg.Name.inst` | one instance, qualified                       | `machine`                        |
| `pkg.service`   | a service by its global dotted name           | `offers`, `requires`, every body |

A CamelCase segment names a component (or, in `for`, the system); a camelCase
segment after a CamelCase one names an instance; a name of lowercase segments
only is a service, with the leading segments read as its package path where the
lookup needs one. In `requires`, `Name` and `pkg.Name` name an interface and
`pkg.service` names an inline-shape service (§3.2). A reference whose case says
one role and whose lookup finds another, or nothing, draws the unknown-name code
of its slot: RSDL-602 (`system`), RSDL-903 (`distribution`), RSDL-702
(`machine`), RSDL-310 to RSDL-312 (`component`), RSDL-704 (`for`).

---

## 5. Attributes

Every declaration and every line takes the family `[ ]` block (general form
§4.2). rsdl uses two of its three forms — the flag and the assignment; no rsdl
key takes the predicate form. The principle (D-6): **an rsdl-owned key is
allow-listed per declaration kind and has a machine consumer; every other key
belongs to a backend and is carried uninterpreted.**

| Key           | Form                            | Legal on                   | Consumer                                               |
| ------------- | ------------------------------- | -------------------------- | ------------------------------------------------------ |
| `instances`   | `= (a, b, …)` camelCase names   | `component`                | the instance set, §7                                   |
| `external`    | flag                            | `component`, `machine`     | placement and the surface set, §9, §13                 |
| `tier`        | `= PLATFORM` or `= APPLICATION` | `distribution`             | RSDL-901, §3.3                                         |
| `labels`      | `= (LABEL, …)`                  | the five declarations      | assurance profiles (typl §14.3, general form §4.7)     |
| `deprecated`  | `= "reason"`                    | the five declarations      | the family lint (general form §4.7); rsdl adds no rule |
| `backend.key` | flag or assignment              | every declaration and line | that backend, uninterpreted                            |

- `instances` must be a parenthesised list of one or more camelCase names: `()`
  and `instances = solo` are RSDL-305. `external` is a flag and takes no value:
  `external = true` is RSDL-313. `tier` takes exactly the two values; any other
  is RSDL-908.
- A key not in the table is FORM-106; a key on a declaration or line its row
  does not name is FORM-107 — `instances` on a line, `deprecated` on a line,
  `labels` on a line, `tier` on a component; a key twice in one block is
  FORM-108. Lines take backend keys only.
- **A backend key is `backend.key`** — `someip.serviceId`, `linux.cpuset`,
  `rust.crate` — a camelCase namespace, a dot, a camelCase key (typl Appendix
  E's `camelCase_id`, which admits no underscore). The compiler carries it into
  the attribute map of its node (§13) as declared: flag or value, never
  interpreted. A backend declares the keys it consumes through the plugin
  contract and validates its own namespace; its findings are the backend's. A
  namespace no configured backend claims is RSDL-804, a warning: the lowering
  proceeds and the key is carried.
- `labels` on a declaration is carried per node beside the map, as the family's
  own fact; an rsdl-owned key is consumed into the fact it feeds and does not
  appear in the map.

This block is the single "later" mechanism (§12): the certified-level lint, a
fabric fact, a tag-based transport's service number, a component's
implementation all arrive as keys, earned by a consumer that reads them.

---

## 6. The Implicit Component

A `service` declared in ridl that no declared component offers may stand for an
**implicit component**: a single-service component of the same name, with the
unit instance (§7) and nothing else — no `requires`, no `instances`, not
external.

```rsdl
system Vehicle       { Cruise, Lane, veh.diag.access }
distribution Adas    { Cruise, Lane, veh.diag.access }
machine AdasHpc      { Cruise.primary, Lane, veh.diag.access }
```

- The derived component name is the service's dotted name. It appears only in
  those lists, in diagnostics and in the IR; nothing else ever names it, so it
  is not an addressed identity (vocabulary V-02).
- A service name stands for a component only when **no declared component in the
  workspace** offers the service. Listing a service that a declared component
  offers is RSDL-504, and the diagnostic names the offerer.
- An implicit component is implemented (§3.3 applies: it is in exactly one
  distribution when any exists). An external or stub provider needs an explicit
  `component` with the `external` flag.
- A system of hand-written services is a list of service names and nothing else
  (D-10).

The rule behind this, stated once: **derive the unaddressed noun from the
addressed one, never the reverse.** The service is what the wire addresses (ridl
§14.5); the component is derived from it. An implicit _service_ named after a
component would make the component name an address, which V-02 forbids; an
interface declared inline in a component would have no name of its own and no
catalog (D-1).

---

## 7. Instances and Redundancy

`instances` on a component names its copies. Instances are named, never
numbered, so a name tells a team what the copy is for.

```rsdl
component Cruise [ instances = (primary, backup) ] { offers veh.adas.cruise }
```

- **The unit instance.** A component without `instances` has exactly one
  instance, named `Unit` in the IR and in diagnostics (`Cruise.Unit`) and
  written in source by the bare component name: `Cruise` in a machine body
  places `Cruise.Unit`. Declaring `instances` replaces the unit instance; it
  does not add to it. An implicit component (§6) has its unit instance only.
- **`Unit` is never written.** `Cruise.Unit` in a body and `Unit` in an
  `instances` list are both RSDL-307. R7 keeps a declared instance camelCase, so
  the CamelCase `Unit` cannot collide with one.
- Instance names are unique within a component; a duplicate is RSDL-306.
  `Cruise.primary` and `Lane.primary` are two instances of two components.
- Instances share the component's offers and requires, and are
  **indistinguishable on the wire** (V-02): no instance identifier travels.
- **Redundancy is derived.** An offering component with more than one instance
  is that service's **redundant provider set**. The lowering derives it, lowers
  every link to every member of the set (§13), and reports each `requires` that
  resolves to such a set as RSDL-409 — a warning, "not yet realizable", because
  the runtime has no arbitration yet (failover, voting, two writers on one
  member; §12). Emission is not blocked.
- **Limit, deliberate.** Instances share their offers, so a left sensor and a
  right sensor offering two different services are two component declarations,
  not two instances of one.

---

## 8. Resolution

A `requires` line names an interface and never a provider. The lowering resolves
it in three steps, each total over the closure:

1. **interface → owning service**: the one closure service whose shape list
   contains the interface (an inline-shape service owns its one interface).
2. **service → offering component**: the one closure component with an `offers`
   line naming the service.
3. **component → instances → machines**: every instance of that component, and
   the machine each is placed on in the deployment at hand (§9).

Two closure rules make the lookup total, both errors:

- **Every service in the closure is offered by exactly one closure component.**
  Two is RSDL-502 — redundancy is a derived case of one component (§7), never a
  second offerer. A service no closure component offers is not in the closure at
  all.
- **Every interface a closure component requires is listed by exactly one
  closure service.** None is RSDL-403 (missing provider; the diagnostic names a
  component outside the closure that offers a listing service, when one exists).
  Two closure services listing one interface is RSDL-408, raised for every
  interface they list, required or not, because the routing key has no service
  in it and the interface would have two owners.

The result of resolution, per `requires`, is one **link per (consumer instance,
producer instance)**, each with its crossing kind (§10). A component with two
instances requiring a service offered by a component with two instances yields
four links. Two components may require the same interface; each resolves
independently.

Every `requires` is mandatory in this release, with one provider (D-11): a
required interface that cannot be resolved blocks lowering for every deployment.
`optional` is reserved (§12).

---

## 9. Placement

Placement is membership: a `machine` body lists what it hosts. Nothing is placed
by a clause, and a service's machine is never declared — it is derived through
resolution (§8).

```rsdl
machine AdasHpc { Cruise.primary, Lane, veh.diag.access }
machine Cockpit { Cruise.backup, Panel [ linux.cpuset = (2, 3) ] }
```

- A bare component name places **every instance** of that component — the unit
  instance when it declares no `instances`, every declared instance otherwise.
  `Cruise.primary` places one. A lone service name places its implicit
  component's unit instance. An attribute block on a bare component line applies
  to each placement it makes.
- **Every instance of every closure component is in exactly one machine per
  deployment.** An instance no machine of the deployment lists is RSDL-701. An
  instance reached twice in one deployment — listed on two machines, or listed
  twice on one, or reached by `Cruise` and again by `Cruise.primary` — is
  RSDL-706.
- A placement line naming a component, instance or service outside the closure,
  or a name that resolves to nothing, is RSDL-702.
- **External components and machines.** An external component is placed like any
  other: an external system is an `external` component on an `external` machine,
  not an exception to RSDL-701. An external machine hosts external components
  only; an implemented component on one is RSDL-707. An external component may
  sit on an on-board machine — that is a stub (vocabulary V-20), and its links
  are then same-machine or different-machine crossings (§10).
- Placement is per deployment. The same component is placed once in `Production`
  and once again, elsewhere, in `Bench`; neither placement is visible to the
  contract (vocabulary V-21).

---

## 10. Crossing Kinds

Every link (§8) carries one of three **crossing kinds**, derived from the two
machines its endpoints are placed on in the deployment:

| Crossing            | When                                                       |
| ------------------- | ---------------------------------------------------------- |
| `same machine`      | both instances are on one machine                          |
| `off-board`         | the machines differ and at least one of them is `external` |
| `different machine` | the machines differ and neither is external                |

- The kind is all the physical layer a link carries (D-5). The transport over a
  crossing, and the network and machine fabric — VLANs, VM placement, CPU
  clusters — are the implementer's configuration, outside the language. If they
  ever enter rsdl they arrive as attributes on `machine` or a block under
  `deployment`, earned by a backend that reads them (§5).
- Whether two components on one machine share an address space is the backend's
  or the host program's decision, not rsdl's (D-2): rsdl declares no process and
  carries no pump kind, priority or other scheduling fact. Under vocabulary V-09
  a same-machine crossing optimised into a direct channel keeps the semantics of
  the store and the queue.
- A link with an `external` component at both endpoints is not lowered: it
  crosses nothing the workspace builds. A link with an external component at
  exactly one endpoint is lowered with its kind and enters the surface set
  (§13); its enforcement is the external side's, not this runtime's.
- Posture — static bus signals against a discovered service — is not derived in
  this release (§12).

---

## 11. Grants

A grant is derived per component, never written. The lowering resolves every
`requires` line through its one owning service (§8); the grant is **the set of
catalog regions its requirements reach** (vocabulary §6, D-9). The region an
interface reaches is the catalog that numbers it — the catalog of the package
that declares the interface (vocabulary V-16, V-17), whichever package declares
the owning service.

- A consumer names interfaces, so it is granted only the regions of the
  interfaces it names — least privilege — and a service that gains or splits
  interfaces does not touch its consumers' grants (V-05).
- A producer's write side is not a second list: it is read from the producers
  fact (§13) — the offering component reaches the regions of every interface its
  services list.
- A backend that groups components — into processes, or Classic components into
  OS-Applications — unions the grants per group (D-2).
- An external component's grant is derived and lowered with the `external` flag;
  enforcing it is outside this runtime.

---

## 12. Reserved and Deferred

**Posture.** A ridl `service` is posture-neutral: the same declaration can be
realized as static bus frames (signal-based, Classic) or as a discovered service
(service-oriented, Adaptive). Deriving that posture per deployment from
placement would let one contract deploy both ways, and would give a Classic to
Adaptive migration with no contract rewrite. A `command` or `query` member
cannot be realized on a pure static bus, because a bus carries dataflow and not
calls. **rsdl derives no posture in this release.** Posture derivation, the
timing-feasibility check RSDL-801 and the RPC-on-a-static-bus constraint
RSDL-803 reopen together with a bus-class backend; the two codes stay reserved
until then (§16).

**Behaviour.** A component's implementation is one fact in rsdl, present or not
(`external`, §3.2). What the implementation is — a crate, a language, later an
rmdl model — is a backend key (§5). The binding of an rmdl reaction to the
members of an interface, which v0.1 wrote in application notation, is not in
this release and returns when rmdl is scheduled.

**Cardinality on `requires`.** Every `requires` is mandatory with one provider
(D-11). An `optional` flag on the `requires` line is the attribute-first path
when variant handling arrives; it will be the one rsdl-owned key legal on a
line.

**Redundancy arbitration.** A redundant provider set (§7) is derived and
reported not yet realizable (RSDL-409). Failover, voting and two writers on one
member are engine questions; a redundant pair placed in one machine is a profile
lint later. RSDL-308 reopens with arbitration.

**The assurance-profile slot.** One attribute key on `system` is reserved for
the assurance profile in force (v0.1's `assurance automotive`). The key is
profile-gated and not named in this release; `assurance` is no longer a keyword.

**The certified-level lint.** A machine is certified for a level; a component
whose `labels` demand a higher level, placed on it, is a lint under the
automotive profile, via `labels` — out of this release (D-2).

**A partition under `machine`.** rsdl declares no process (D-2). What reopens a
grouping noun is a consumer whose safety argument needs a mixed-level placement
rejected at design time; the shape would then be an optional partition grouping
under `machine`, with one implicit partition when none is declared.

**Assets.** A distribution's member lines carry no kind keyword, so a later
`asset` declaration lists additively (D-11); it reopens the member-kind question
only if a member kind ever has to exist without a declaration.

---

## 13. What the Lowering Produces

The lowering runs over the closure once and then once per deployment. It
produces **facts**, stated here as facts and not as a schema; a runtime's
descriptor is an emitter over them and is specified with the runtime. Inputs
from outside rsdl, cited once: the interface numbers, read from each package's
generated lock file (the ridl reference specifies the lock file); the member
ordinals, from position (ridl §11); and the **catalog hash** per catalog,
computed by ridl over the interfaces, their numbers and every type they reach
(D-8) — embedded here as an input, never computed by rsdl.

Per deployment:

- **Producers** — for every service in the closure: the offering component, its
  instances, and the machine each instance is placed on; a redundant provider
  set is visible as more than one instance and carries the not-yet-realizable
  marker of RSDL-409.
- **The link set** — for every `requires` line, one link per (consumer instance,
  producer instance): the interface, its owning service, the two instances,
  their machines and the crossing kind (§10). A link with two external endpoints
  is absent.
- **The routing table** — (catalog, interface number, member ordinal) to the
  producing instances and their machines, for every member of every interface a
  closure service lists.
- **The permission list** — per component, the set of catalog regions its
  requirements reach (§11). The write side is read from the producers fact.
- **The surface set** — the system's external boundary: every link with an
  external component at exactly one endpoint, with its interface and its
  direction (the external side consumes, or offers). Off-board stays a crossing
  kind on the link; the surface set is the boundary read from the consumer's
  side.
- **The region map** — one region per catalog the closure reaches: the catalog's
  name (its package name, V-16), its hash, and the interfaces of that catalog
  that closure services list.
- **The attribute map per node** — every backend key, as declared, on every node
  that has an attribute site: a declaration (`system`, `deployment`, `machine`,
  `component`, `distribution`), a member line of a `system` or a `distribution`,
  a placement line (where an instance's placement carries its keys), an `offers`
  line, or a `requires` line (the link's site). `labels` ride beside the map per
  declaration.
- **Distribution installation and dependency** — per distribution, the machines
  hosting at least one instance of its components (installation, which differs
  per deployment); and the distributions it depends on: `A` depends on `B` when
  a component in `A` requires an interface whose owning service a component in
  `B` offers (dependency, the same in every deployment). RSDL-901 is checked
  over this fact. Absent when the workspace declares no distribution.
- **The catalog hashes** of every catalog in the region map, as received.

**Errors and warnings.** An error in the closure — RSDL-3xx, 4xx, 5xx, 6xx or
9xx — blocks lowering for every deployment. An error in one deployment —
RSDL-7xx — blocks lowering for that deployment only. A warning (RSDL-409,
RSDL-804) never blocks: the facts are produced and carry the warned condition.

---

## 14. `ridl diff` at the System

`ridl diff` compares at the system (D-10): the unit of comparison is the
closure's contracts — the services in the closure, the interfaces they list, and
their members — classified by the ridl categories. **It classifies contracts
only** (D-11).

- A change in a deployment — an instance moved to another machine, a machine
  added, removed or made `external` — changes every link derived from the old
  placement and leaves the contract untouched. `ridl diff` lists it under its
  own heading, **placement changed**, with no verdict: neither compatible nor
  breaking.
- A change to the closure or to a component's lines — a component added to or
  removed from the system, an `offers` or `requires` line added or removed,
  `instances` changed, a component made `external` — is listed under a second
  heading, **composition changed**, likewise with no verdict, because it changes
  the derived links and not the contract.
- Which of these changes are breaking, and for whom, is decided by the stability
  policy (roadmap E4.5a), not here.

---

## 15. Conventions

- Name a component for the capability it runs (`Cruise`, `LaneAssistant`), an
  instance for its role (`primary`, `backup`), a machine for what it is
  (`AdasHpc`, `Cockpit`), and a service by its dotted global name
  (`veh.adas.cruise`).
- Let a lone service stand for its component (§6). Write an explicit `component`
  when one is needed: it offers more than one service, it only consumes, it
  requires interfaces, it has instances, or it is external.
- Require the narrowest interface that serves the consumer. A consumer that
  requires one interface is unaffected when the service that owns it gains or
  splits interfaces, and its grant stays the smallest one (§11).
- Declare instances only for copies that offer the same services (§7). Two parts
  that offer different services are two component declarations.
- Keep each bench or test topology as its own `deployment`, in its own file. One
  system, several deployments.
- Keep transport, network and machine fabric out of rsdl. If a backend needs one
  of those facts, it arrives as that backend's namespaced key (§5).

---

## 16. Diagnostics

Coded `RSDL-`, same lifecycle rules as typl §16: codes are never renumbered, a
retired code is kept in the table and never reused. Grouped by hundreds:

    3xx  component and instances       6xx  system and workspace
    4xx  resolution                     7xx  placement — deployment and machine
    5xx  services                       8xx  transport, posture and backend keys
                                        9xx  distribution

An `.rsdl` file also draws the namespaces no profile owns, `FORM-` and `MANI-`,
tabulated once in the family overview §7. The attribute-block rules FORM-106
(unknown key), FORM-107 (key not allowed on this declaration kind) and FORM-108
(duplicate key) cover the rsdl-owned keys of §5; no `RSDL-` code is minted for
them. Codes not shown in any table were never allocated.

### 16.1 Codes in force

| Code     | Rule                                                                                                                                                               | Severity                     | Section |
| -------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ---------------------------- | ------- |
| RSDL-305 | `instances` is not a parenthesised list of one or more camelCase names (`()`, `instances = solo`)                                                                  | error                        | §5, §7  |
| RSDL-306 | duplicate instance name in one component                                                                                                                           | error                        | §7      |
| RSDL-307 | `Unit` written in source — as a declared instance name, or as the instance segment of a reference                                                                  | error                        | §7      |
| RSDL-308 | a component requires an interface listed by a service it offers                                                                                                    | error                        | §3.2    |
| RSDL-309 | the same service on two `offers` lines, or the same interface on two `requires` lines, of one component                                                            | error                        | §3.2    |
| RSDL-310 | an `offers` line names something that is not a service, or nothing                                                                                                 | error                        | §3.2    |
| RSDL-311 | a `requires` line names a service whose shape is a list of interfaces — the diagnostic lists them                                                                  | error                        | §3.2    |
| RSDL-312 | a `requires` line names something that is neither an interface nor an inline-shape service, or nothing                                                             | error                        | §3.2    |
| RSDL-313 | `external` written with a value — it is a flag                                                                                                                     | error                        | §5      |
| RSDL-403 | a closure component requires an interface that no closure service lists (missing provider)                                                                         | error                        | §8      |
| RSDL-408 | an interface listed by two services of the closure — raised for every interface they list                                                                          | error                        | §8      |
| RSDL-409 | a `requires` resolves to a redundant provider set — an offering component with more than one instance                                                              | warning (not yet realizable) | §7, §8  |
| RSDL-502 | two closure components offer one service                                                                                                                           | error                        | §8      |
| RSDL-504 | a member line names a service by its name while a declared component offers it — names the offerer                                                                 | error                        | §6      |
| RSDL-601 | more than one `system` in the workspace                                                                                                                            | error                        | §3.1    |
| RSDL-602 | a `system` member line names nothing that is a component or a service                                                                                              | error                        | §3.1    |
| RSDL-603 | a name listed twice in one `system` body                                                                                                                           | error                        | §3.1    |
| RSDL-604 | a declaration of another profile (a type, an interface, a service) in an `.rsdl` file                                                                              | error                        | §2      |
| RSDL-701 | an instance of a closure component with no placement in a deployment                                                                                               | error                        | §9      |
| RSDL-702 | a placement line names a component, instance or service outside the closure, or a name that resolves to nothing                                                    | error                        | §9      |
| RSDL-704 | `deployment … for Y` where `Y` resolves to no declared `system`                                                                                                    | error                        | §3.4    |
| RSDL-705 | two machines with one name in one deployment                                                                                                                       | error                        | §3.5    |
| RSDL-706 | an instance placed twice in one deployment — also `Cruise` together with `Cruise.primary`                                                                          | error                        | §9      |
| RSDL-707 | an `external` machine lists an implemented component                                                                                                               | error                        | §9      |
| RSDL-708 | two deployments with one name in the workspace                                                                                                                     | error                        | §3.4    |
| RSDL-804 | a backend key whose namespace no configured backend claims                                                                                                         | warning                      | §5      |
| RSDL-901 | a `PLATFORM` distribution holds a component whose `requires` resolves into an `APPLICATION` distribution (tier inversion); a distribution without `tier` is exempt | error                        | §3.3    |
| RSDL-903 | a distribution member line names something outside the closure, or nothing                                                                                         | error                        | §3.3    |
| RSDL-904 | an implemented closure component in no distribution, while the workspace declares at least one                                                                     | error                        | §3.3    |
| RSDL-905 | a component listed by two distributions                                                                                                                            | error                        | §3.3    |
| RSDL-906 | a name listed twice in one distribution body                                                                                                                       | error                        | §3.3    |
| RSDL-907 | an `external` component listed by a distribution                                                                                                                   | error                        | §3.3    |
| RSDL-908 | `tier` value other than `PLATFORM` or `APPLICATION`                                                                                                                | error                        | §5      |

### 16.2 Reserved

| Code     | Reserved for                                                                      | Reopens with |
| -------- | --------------------------------------------------------------------------------- | ------------ |
| RSDL-801 | derived transport cannot meet a link's contract timing (feasibility)              | posture, §12 |
| RSDL-803 | a `command`/`query` member forced into the static posture (a bus carries no call) | posture, §12 |

### 16.3 Retired

Each code below was in force in v0.1 and is retired with the construct it
guarded. The numbers are never reused.

| Code     | v0.1 rule, in brief                                        | Reason retired                                                        |
| -------- | ---------------------------------------------------------- | --------------------------------------------------------------------- |
| RSDL-301 | applied name is neither a model nor a component            | application notation dropped; a component applies nothing             |
| RSDL-302 | provided output flow with no defining equation             | no flows or equations in a component body                             |
| RSDL-303 | provided service not fully covered by equations            | a component offers a service whole; coverage is the implementation's  |
| RSDL-304 | applied model signature mismatches the wired signals       | no wiring; a model is rmdl's and is not applied here                  |
| RSDL-401 | conditional or dynamic instantiation                       | instances are a static attribute (§7); nothing is instantiated        |
| RSDL-402 | wire connects flows of differing type                      | no wires; a link is derived from `requires` (§8)                      |
| RSDL-404 | provided output defined more than once                     | no outputs defined in source                                          |
| RSDL-405 | kind-crossing wire other than event to command             | no wiring; event-to-command needs a model to emit                     |
| RSDL-407 | instantaneous cycle among models within one leaf           | no models composed in a component                                     |
| RSDL-501 | consumer source distinguishes static wire from discovery   | no posture exists to distinguish (§12)                                |
| RSDL-503 | published service name collides                            | the name is ridl's; RIDL-140 alone reports it                         |
| RSDL-703 | placement onto a target whose capability class cannot host | targets and capability classes replaced by named machines (§3.5)      |
| RSDL-802 | no transport-policy entry for a (locality × kind)          | transport is configuration, not a table in the grammar (D-5)          |
| RSDL-902 | bundle includes an instance placed on a different target   | a distribution spans machines; installation is derived (§13)          |
| RSDL-950 | reserved resilience keyword used                           | `redundant`, `supervise`, `degraded` retired; redundancy derived (§7) |

---

## 17. Open Questions

Each open question of v0.1.0 §13, with its disposition, then the questions this
version adds.

1. **Composition versus deployment boundary** — answered. `system` and
   `component` state what exists and how it relates; `deployment` and `machine`
   state where it runs. The time base that v0.1.0 declared in a deployment is
   retired.
2. **Dynamic topology and orchestration** — open. This version describes a
   static set of instances with static placement. Starting, stopping and moving
   workloads at runtime needs a dynamic layer that is not designed.
3. **Transport and posture policy** — moved. Transport is the implementer's
   configuration (§10); posture is reserved (§12).
4. **Service discovery semantics** — narrowed. More than one provider is now a
   derived redundant provider set (§7), whose arbitration waits for the runtime
   (§12). Versioned services and consumer preference remain open.
5. **End-to-end timing composition** — deferred with timing feasibility and
   RSDL-801 (§12).
6. **Distribution dependency and versioning** — narrowed. Dependencies between
   distributions are derived (§3.3). Version ranges and coordinated updates of
   several distributions remain open.
7. **Resilience realization** — open. Redundancy is derived (§7); failover,
   voting and supervision wait for the failure-management specification (ridl
   §10.4). The v0.1.0 words `redundant`, `supervise` and `degraded` are retired.
8. **Global service catalog scoping** — unchanged. Service names stay one flat
   global namespace (RIDL-140).
9. **Service-number allocation** — answered. The routing key holds a catalog, an
   interface and a member, and no service (§13). A tag-based transport that
   needs a service number reads it from its own backend key (§5).
10. **One owning service per interface across a workspace** — open. rsdl checks
    that no two services in the closure list one interface (§8). Whether ridl
    should reject it across a whole workspace is a question for the ridl
    finalization.
11. **A member kind without a declaration** — open. A distribution lists bare
    references whose kind comes from their declarations (§3.3). If a member kind
    ever has to exist without a declaration, such as an asset named by a file
    path, the member line needs a kind keyword.
12. **What a breaking deployment change is** — open. `ridl diff` lists a
    deployment change under its own heading with no verdict (§14); the verdicts
    are decided with the IR stability policy.

---

## Appendix A — Full Example

A small vehicle: two ridl packages carry the contracts, one rsdl package carries
the closure, two distributions and two deployments. Every rule of §3 to §13
holds over these files.

**Contracts** — `veh/common/common.typl` and `veh/adas/adas.ridl`, shown
briefly:

```typl
package veh.common

type Speed: km/h [0.0..250.0 step 0.5]
enum LeverCmd { SET = 1, CANCEL = 2, RESUME = 3 }
struct FaultReport { count: integer [0..255] }
```

```ridl
package veh.adas

import veh.common.Speed
import veh.common.LeverCmd

interface CruiseControl {
  signal  engaged: boolean @[100ms..1s]
  signal  target: Speed @[100ms..1s]
  command setLever(cmd: LeverCmd) @[..50ms]
}
interface LaneAssist {
  signal active: boolean @[100ms..1s]
}

service veh.adas.cruise : CruiseControl
service veh.adas.lane   : LaneAssist
```

`veh/diag/diag.ridl` declares one inline-shape service; its dotted name is the
name of its one interface (§3.2):

```ridl
package veh.diag

import veh.common.FaultReport

service veh.diag.access {
  query readFaults(): FaultReport @[..100ms]
}
```

**The closure and the distributions** — `veh/system/system.rsdl`:

```rsdl
package veh.system

import veh.adas.CruiseControl
import veh.adas.LaneAssist

/// Adaptive cruise, two copies: one per compute node.
component Cruise [ instances = (primary, backup), labels = (ASIL_B) ] {
  offers   veh.adas.cruise
  requires LaneAssist
}

component Lane { offers veh.adas.lane }

component Panel {
  requires CruiseControl
  requires LaneAssist
}

/// The fleet backend. No implementation in this workspace.
component Backend [ external ] {
  requires CruiseControl
  requires veh.diag.access
}

system Vehicle { Cruise, Lane, Panel, Backend, veh.diag.access }

distribution Adas [ tier = PLATFORM ]    { Cruise, Lane, veh.diag.access }
distribution Hmi  [ tier = APPLICATION ] { Panel }
```

**Production** — `veh/system/production.rsdl`:

```rsdl
package veh.system

deployment Production for Vehicle {
  machine AdasHpc [ labels = (ASIL_B) ] { Cruise.primary, Lane, veh.diag.access }
  machine Cockpit { Cruise.backup, Panel [ linux.cpuset = (2, 3) ] }
  machine Cloud   [ external ] { Backend }
}
```

**Bench** — `veh/system/bench.rsdl`, one machine, the backend stubbed on it:

```rsdl
package veh.system

deployment Bench for Vehicle {
  machine DevBox { Cruise, Lane, Panel, veh.diag.access, Backend }
}
```

What the lowering derives, and what it reports:

- **Closure**: `Cruise`, `Lane`, `Panel`, `Backend`, and the implicit component
  `veh.diag.access` (§6). Services in the closure: `veh.adas.cruise` (offered by
  `Cruise`), `veh.adas.lane` (`Lane`), `veh.diag.access` (its implicit
  component). Interfaces: `CruiseControl`, `LaneAssist`, and `veh.diag.access`
  as its own interface. Each has one owning service and one offering component
  (§8).
- **Instances**: `Cruise.primary`, `Cruise.backup`, `Lane.Unit`, `Panel.Unit`,
  `Backend.Unit`, `veh.diag.access.Unit` (§7).
- **Links, Production** (§8, §10): `Cruise.primary → Lane.Unit` same machine;
  `Cruise.backup → Lane.Unit` different machine; `Panel.Unit →
  Cruise.primary`
  different machine; `Panel.Unit → Cruise.backup` same machine;
  `Panel.Unit → Lane.Unit` different machine; `Backend.Unit →
  Cruise.primary`,
  `Backend.Unit → Cruise.backup` and `Backend.Unit →
  veh.diag.access.Unit`
  off-board. The three `Backend` links are the surface set (§13): the external
  side consumes.
- **Links, Bench**: every link is same machine; the `Backend` links stay in the
  surface set, because the flag and not the machine defines the boundary.
- **Warnings**: RSDL-409 twice, for `Panel`'s and `Backend`'s
  `requires
  CruiseControl` — `Cruise` is a redundant provider set. RSDL-804
  for `linux.cpuset` when no `linux` backend is configured; the key is carried
  on `Panel`'s placement line in `Cockpit` either way.
- **Grants** (§11): `Cruise` reaches the `veh.adas` region (for `LaneAssist`);
  `Panel` reaches `veh.adas`; `Backend` reaches `veh.adas` and `veh.diag`,
  lowered with the `external` flag. `Lane` requires nothing and holds no
  consumer grant; its write side is read from the producers fact.
- **Distributions** (§3.3, §13): `Hmi` depends on `Adas` (`Panel` requires
  interfaces `Cruise` and `Lane` offer); `Adas` depends on nothing. No tier
  inversion. `Backend`, external, is in no distribution. In `Production`, `Adas`
  is installed on `AdasHpc` and `Cockpit`, `Hmi` on `Cockpit`; in `Bench`, both
  on `DevBox`.

Two edits that would each draw one error: listing `veh.adas.cruise` in `Vehicle`
(RSDL-504, `Cruise` offers it); writing `Cruise` in place of `Cruise.primary` on
`AdasHpc` (RSDL-706, `Cruise.backup` is reached twice).

---

## Appendix B — Formal Grammar (EBNF)

The rsdl **profile grammar** — the restriction of the family grammar accepted in
`.rsdl` files. `package`, `import`, `doc_comment`, `sep`, `qualified_id`,
`CamelCase_id`, `camelCase_id`, `SCREAMING_SNAKE_ID` and `literal` are typl
Appendix E's; `attr_block` is the family's single production (general form
§4.2), restated here with the value form `instances` needs. A declaration of any
other profile is RSDL-604.

```ebnf
file             = package { import } { declaration } ;

declaration      = component_def | system_def | distribution_def | deployment_def ;
                 (* no modifier — §2; machine_def occurs only inside a deployment *)

(* ---------- The five declarations — Shape 3 ---------- *)

component_def    = doc_comment? "component" CamelCase_id attr_block?
                   "{" { component_line sep? } "}" ;
component_line   = doc_comment? ( "offers" reference | "requires" reference ) attr_block? ;
                 (* one reference per line — §3.2; `offers` a service,
                    `requires` an interface or an inline-shape service *)

system_def       = doc_comment? "system" CamelCase_id attr_block?
                   "{" { member_line sep? } "}" ;
distribution_def = doc_comment? "distribution" CamelCase_id attr_block?
                   "{" { member_line sep? } "}" ;
member_line      = doc_comment? reference attr_block? ;
                 (* a component, or a lone service — §4, §6 *)

deployment_def   = doc_comment? "deployment" CamelCase_id "for" reference attr_block?
                   "{" { machine_def sep? } "}" ;
                 (* `for` names the system — §3.4; R5: clause, then attributes *)
machine_def      = doc_comment? "machine" CamelCase_id attr_block?
                   "{" { placement_line sep? } "}" ;
placement_line   = doc_comment? reference attr_block? ;
                 (* a component, one instance, or a lone service — §9 *)

(* ---------- References — one production, the role read from case ---------- *)

reference        = qualified_id ;
                 (* Name · Name.inst · pkg.Name · pkg.Name.inst · pkg.service — §4.
                    A CamelCase segment names a component (the system, after `for`);
                    a camelCase segment after it names an instance; lowercase
                    segments only name a service. Lookup confirms the role. *)

(* ---------- Attribute block — general form §4.2, flag and assignment forms ---------- *)

attr_block       = "[" { attribute sep? } "]" ;
attribute        = key | key "=" attr_value ;
key              = camelCase_id                       (* an rsdl-owned key — §5 *)
                 | camelCase_id "." camelCase_id ;    (* backend.key — §5 *)
attr_value       = literal | SCREAMING_SNAKE_ID
                 | "(" [ list_item { "," list_item } [ "," ] ] ")" ;
list_item        = literal | SCREAMING_SNAKE_ID | camelCase_id ;
                 (* camelCase_id admits instance names in `instances = (…)` — §7.
                    general form §4.2's const_value admits literals and constant
                    references only; this is the one widening rsdl needs *)
```

---

## Appendix C — Glossary

Family terms are defined in the typl, ridl and rmdl glossaries and mean the same
here. rsdl-specific:

| Term                       | Definition                                                                                                                              |
| -------------------------- | --------------------------------------------------------------------------------------------------------------------------------------- |
| **attribute map**          | the namespaced backend keys of one node — a declaration or a line — carried into the lowered facts uninterpreted (§5, §13)              |
| **backend key**            | an attribute key namespaced by a backend name, `backend.key`, that the compiler carries and the backend validates (§5)                  |
| **closure**                | the components a system lists, including the implicit components of listed services; every completeness check quantifies over it (§3.1) |
| **component**              | one execution context; offers services, requires interfaces, may be external, has instances (§3.2)                                      |
| **crossing kind**          | where a link runs: same machine, different machine, or off-board (§10)                                                                  |
| **deployment**             | one placement of the system: a named set of machines, `for` the system (§3.4)                                                           |
| **distribution**           | what is versioned, signed and shipped together; its installation and dependencies are derived (§3.3)                                    |
| **external**               | a flag: on a component, no implementation exists in this system; on a machine, it lies outside the system's boundary (§3.2, §3.5)       |
| **grant**                  | the set of catalog regions a component's requirements reach, resolved through each interface's owning service (§11)                     |
| **implicit component**     | the single-service component a lone service stands for, named by the service's name (§6)                                                |
| **instance**               | one named copy of a component; instances share the component's offers and requires and are indistinguishable on the wire (§7)           |
| **link**                   | one consumer instance reaching one producer instance through a `requires`, with its crossing kind (§8, §10)                             |
| **machine**                | what software is installed on; lists the instances it hosts (§3.5)                                                                      |
| **member line**            | a bare reference in a `system`, `distribution` or `machine` body; a reference, not a declaration (§4)                                   |
| **offers / requires**      | a component's two body lines: it offers a service and requires an interface (§3.2, §4)                                                  |
| **placement**              | membership of an instance in a machine body; every instance of the closure is in exactly one machine per deployment (§9)                |
| **posture**                | how a service is realized on the wire, static or discovered; reserved, not derived in this release (§12)                                |
| **redundant provider set** | the instances of an offering component that has more than one; derived, and reported as not yet realizable (§7)                         |
| **surface set**            | per deployment, the links with an external component at exactly one end: the system's external boundary (§13)                           |
| **system**                 | the closure: the components a deployment has to account for, and nothing else (§3.1)                                                    |
| **tier**                   | an optional attribute of a distribution, `PLATFORM` or `APPLICATION`, checked for inversion (§3.3)                                      |
| **unit instance**          | the one instance of a component without `instances`, named `Unit` in the IR and in diagnostics and never written in source (§7)         |

---

_End of rsdl Language Reference v0.2.0 — Draft._
