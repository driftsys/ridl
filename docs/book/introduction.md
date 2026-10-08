# RIDL

**One platform, four languages, one grammar.** RIDL is a family of languages
for modeling component-based reactive systems: a shared vocabulary layer and
three description languages over it, sharing one grammar, one toolchain, and one
intermediate representation (IR).

## The family

| Language | Expands to | Describes | Audience |
| --- | --- | --- | --- |
| **typl** | type language | data — types, ranges, units, constants | data architects |
| **ridl** | reactive interface description language | interactions at every boundary — system, person, world (`signal` / `event` / `command` / `query` / `fixed`) | service, HMI, and sensor/actuator teams |
| **rmdl** | reactive model description language | behaviour — synchronous / functional compute | control / algorithm engineers |
| **rsdl** | reactive system description language | architecture — components, wiring, deployment | integrators |

The dependency lattice is `typl ← {ridl, rmdl} ← rsdl`: typl is the only
standalone member, and rsdl is the apex that composes the others. rxdl is not a
language — it is a file kind that lifts restrictions, plus spellings over ridl's
interaction families (ADR-0012).

## What is built

Three layers of the family have a working toolchain in this repository:

- **typl** — the vocabulary layer: compiler, `ridl fmt`, an LSP
  server, and a VS Code extension.
- **ridl** — the interface layer over it: the five interaction kinds,
  timing annotations, contracts, interfaces and services, a TypeScript code
  generator beside the Rust one, and `ridl diff`.
- **rsdl** — the architecture layer: components, the system,
  distributions and deployments, checked against the rsdl reference and lowered
  to the IR beside the package IR, and `ridl diff` at the system. See
  [Describing a system](rsdl.md).

**`ridl-rt`** is the `no_std` runtime library that a
generated ridl package links and a runtime implements: identity, time
and the envelope, samples, the payload traits, the interaction descriptors,
the ports, and the contract and transport errors. It has no dependency in any
feature combination. `ridl-loopback` implements its ports in
process, and nothing else does: no transport reaches a second process. Of the
payload codecs, only FlatBuffers is built: the Rust backend emits a FlatBuffers
codec for the payload types of a package, and proto3 and `repr(C)` have no codec.

`ridl build --emit` writes Rust source, TypeScript source, a proto3 schema, a
FlatBuffers schema, or the IR as JSON, with the lowered rsdl system beside it.

**rxdl and rmdl are specified but not built.** Their language references are
complete enough to design against, but no compiler accepts them and nothing in
this book describes them as usable. rmdl is parked, with no implementation
scheduled; rxdl keeps only its unrestricted profile, narrowed to types,
interfaces and wiring, scheduled in step 2.

**There is no runtime you can run a contract over.** The transport bindings and
the delivery semantics are specified and not implemented. Provider-side contract
enforcement is: the generated `serve` evaluates a `require` and an `ensure`
clause and settles the contract error, and the Rust backend's tests run that
over the in-process runtime below. Nothing the compiler emits links a runtime,
so none of it is reachable from a command in this book. `ridl-rt` (above) is the
library a runtime implements, not a runtime itself — it performs no I/O and links no codec. The
repository does hold one runtime, `ridl-loopback`: an in-process reference that
carries values between a provider and a consumer in one program, with no
transport, no wire format and a clock a test advances by hand. Nothing the
compiler emits links it yet, so it is reached only from a program written
against it by hand.

[Getting started](getting-started.md) walks through what you can run today.

## Where the specifications live

This book is the reader's entry point. The normative documents live in the
repository:

- **Specifications** — `docs/specification/`: the normative home of the family
  overview, the five language references, the expr-core specification (the
  shared grammar of `require` / `ensure` clauses), and the frame specification
  (what crosses a runtime boundary per interaction kind). Every one of them
  except the family overview is reproduced in this book's Language reference
  section — [typl](reference/typl.md), [ridl](reference/ridl.md),
  [rxdl](reference/rxdl.md), [rmdl](reference/rmdl.md),
  [rsdl](reference/rsdl.md), [expr-core](reference/expr-core.md),
  [frame](reference/frame.md).
- **Work in progress** — `docs/wip/`: the pre-ADR concept note, the
  cross-profile general-form working spec, and the authoring-skill outline.
- **Decisions** — `docs/decisions/`: the architecture decision records —
  ADR-0002 (module system), ADR-0004 (implementation sequencing and stack),
  ADR-0005 (agent enablement), ADR-0006 (walking-skeleton execution),
  ADR-0007 (E1 execution), ADR-0008 (E2 execution), and ADR-0009 (toolchain and
  gate parity).
- **Technotes** — `docs/technotes/`: informative architecture notes, which bind
  nothing.
- **Roadmap** — `docs/ROADMAP.md`: the forward plan — the two steps the
  2026-09-12 re-scope sets, the parked blocks with the observation that reopens
  each, and the milestone summary. What has already shipped is in
  `docs/archive/roadmap-landed-record.md`.
- **Archive** — `docs/archive/`: superseded documents, and the plans of the
  epics that have landed.

Browse them on GitHub:
<https://github.com/driftsys/ridl/tree/main/docs>.

## Status

All specifications are working drafts: typl, rxdl, rmdl and the expr-core
specification at v0.1.0, ridl and rsdl at v0.2.0. The toolchain is released
as binaries on GitHub and as crates on crates.io. This book describes the
`main` branch, which can be ahead of the newest release;
[Getting started](getting-started.md) says how to build it from a clone.
