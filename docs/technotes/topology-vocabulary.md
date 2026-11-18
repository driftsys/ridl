# Topology vocabulary: where the words came from

Why the nouns of the layer between a contract and the hardware are what they
are, how they map to AUTOSAR and OSGi, and which names were rejected.
Informative: the normative records are
[the rsdl language reference](../specification/rsdl-language-reference.md) §1.3
for the nouns and its other sections for what each one does,
[ADR-0022](../decisions/ADR-0022-rsdl-system-in-the-ir.md) for the IR carrier,
and [the ridl reference](../specification/ridl-language-reference.md) §11 and
§14.5 for the contract side. The working note this text was gardened from, with
its invariants `V-01` to `V-21` and its open items `V-X1` to `V-X6`, is
[`docs/archive/2026-09-08-topology-vocabulary.md`](../archive/2026-09-08-topology-vocabulary.md);
the reference cites those ids and they resolve there.

## Why a vocabulary was needed

Three vocabularies described this layer and they disagreed: the first rsdl
reference, AUTOSAR (Classic and Adaptive, each in its own way), and the naming
of the first consumer's topology. Each is right about part of the range and none
covers all of it, so the words were being derived again every few weeks. The
nouns were fixed once, each answering one question:

| Noun         | Answers                                         |
| ------------ | ----------------------------------------------- |
| distribution | what is versioned, signed and shipped           |
| machine      | what software is installed on                   |
| component    | one execution context                           |
| service      | what is addressed on the wire                   |
| interface    | what is contracted                              |
| member       | one typed interaction                           |
| catalog      | an id space and a hash over a set of interfaces |

One rule recurs across them: only what is addressed has identity on the wire. It
was reached independently for the machine identifier (placement is what changes
between two releases of a vehicle), for the catalog name (a global registry of
catalog ids is the coordination that independent catalogs exist to avoid), and
for components and distributions. The reference states it in §1.3.

## The mapping to other systems

| This family  | AUTOSAR Adaptive         | AUTOSAR Classic    | OSGi             |
| ------------ | ------------------------ | ------------------ | ---------------- |
| distribution | SoftwareCluster          | (ECU flash)        | bundle           |
| shipped file | SoftwarePackage          | -                  | the JAR          |
| machine      | Machine                  | ECU                | -                |
| process      | Process, Executable      | OS-Application     | -                |
| component    | -                        | OS Task            | component (DS)   |
| service      | ProvidedServiceInst.     | -                  | service registry |
| interface    | ServiceInterface         | PortInterface      | Java interface   |
| signal       | field (get, set, notify) | S/R data element   | -                |
| event        | event                    | S/R, Trigger       | event admin      |
| command      | fire-and-forget method   | C/S op, no return  | -                |
| query        | method with a return     | C/S operation      | -                |
| fixed        | - (configuration)        | ParameterInterface | -                |
| codec + glue | ARA                      | RTE (generated)    | -                |

Three observations from the table:

- A signal corresponds to an Adaptive field more closely than any other pair.
  proto3 cannot express it, which is one reason ADR-0013 records DDS as mapping
  cleanly and proto3 as not.
- `fixed` has no counterpart in either AUTOSAR variant.
- The protection domain is the row where the standards have something this model
  does not declare. Classic's OS-Application is a first-class grouping for
  memory and timing protection. rsdl declares no process (reference §1.4, §12),
  and a backend that groups components unions their grants per group (reference
  §11).

## Three decisions of wording

- **A machine is the unit of deployment and isolation.** It covers a hypervisor
  host and its guests alike. `vm` was rejected because the QNX host is a machine
  and does not sit under a hypervisor; `host` and `guest` are roles a machine
  holds, not kinds of machine. In the Adaptive documentation a Machine is "quasi
  a virtualized ECU-HW, an entity where software can be deployed to", and one
  real ECU may run several. A capability-class target, which the first rsdl
  reference used, is not a machine: it mixed two hardware grains with two
  isolation grains and bound to a physical node late. rsdl names machines.
- **An AUTOSAR SWC is not this model's component.** In Classic the runnables of
  an SWC are spread over tasks at integration time, an n:m relation, where a
  component here is one execution context. Fusing the design unit and the
  execution context is right for this model; the word must not be borrowed from
  AUTOSAR with its meaning.
- **`RTE` names the wrong layer.** It exists only in Classic, where it is
  middleware generated for one ECU and not a place. Its structural counterpart
  here is `ridl-rt` with the emitted bindings. Adaptive has no RTE; ARA, a set
  of linked APIs, takes its role, which is what `ridl-rt` is.

## Rejected names

The reasons outlive the decisions.

| Name           | Why not                                                                                                                                                                                                                      |
| -------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `apex`         | Android's updatable system module container, mounted at `/apex`, live in this domain; also the apex of the family lattice.                                                                                                   |
| `apk`          | Android application package.                                                                                                                                                                                                 |
| `package`      | A typl and ridl keyword whose directory mapping is normative (ADR-0002), the Java package name of a catalog, and an APK. An IR message that named a distribution this way would collide with the source package permanently. |
| `bundle`       | Clean, and the first rsdl reference defined it, but `distribution` reads better to the reviewers the language has to satisfy.                                                                                                |
| `pack`         | Collides with pack as a verb and reads informal.                                                                                                                                                                             |
| `cluster`      | AUTOSAR Adaptive's own word (SoftwareCluster), and unusable because a cluster is also the instrument display.                                                                                                                |
| `module`       | Android mainline modules, Rust modules and the ADR-0002 module system.                                                                                                                                                       |
| `RTE`          | Names the layer that `ridl-rt` and the emitted bindings occupy.                                                                                                                                                              |
| `dist`, `comp` | Every keyword in the family is a full word; `dist` also reads as a build output directory.                                                                                                                                   |
