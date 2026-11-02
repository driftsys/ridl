# Instance and component identity — decisions with the owner

Working note for driftsys/ridl#1176, after the owner's review of PR #1179 on
2026-11-02. It replaces ruling R-4 of the deployment descriptor design
([the archived design](../archive/2026-11-01-deployment-descriptor-design.md)),
which left the attach number to a backend key that a person types on the
placement line. The owner rejected that: the toolchain must give the number.

The next session designs the MVP below from this note, folds it into #1179 or a
follow-up pull request, and then gardens this note.

## Vocabulary agreed

- A **system** holds the components, the services they offer, the interfaces
  they require, and each component's instances. These facts are the same in
  every deployment of the system.
- A **deployment** places every instance of the system on a machine. A system
  can have several deployments (a bench, a vehicle line).
- A **component** is a program. An **instance** is one running copy of it; a
  component that declares no `instances` has one instance, `Unit`.

So an identity given to a component or an instance is a system fact. It is the
same in every deployment, and generated code can carry it without being tied to
one deployment.

## Decisions

1. **The component's identity is its qualified rsdl name**, `package.Name` (the
   service's dotted name for the implicit component of a lone service). ridl
   assigns no component number. The deployment descriptor already carries the
   qualified name (`Component.name`).
2. **The platform principal is implicit: derived from the qualified name, with
   an override.** An Android package or APEX name and a Linux SELinux domain are
   computed from `package.Name` by a fixed transform, so a workspace declares
   nothing. A backend attribute on the component declaration overrides the
   derived value only where a product needs a different name. **Not in the
   MVP.**
3. **ridl numbers every instance and locks the numbers (MVP).** The number is
   what a client states at attach. It is stable when sources are reordered, and
   a removed instance's number is never reused, as `interfaces.lock` does for
   interfaces.
4. **The number reaches the runtime and the program (MVP)**: in the deployment
   descriptor's `Instance`, in the codegen deployment section's `Instance`, and
   as generated Rust constants per component instance that do not depend on a
   deployment.
5. **Service numbers are not in the MVP.** No attach check needs them: the owner
   of an interface is reached through the instance.

## Questions for the MVP design

- The lock file: its name and place (beside `interfaces.lock`, or one per
  system), its format, and the command that writes it (`ridl lock` or the
  existing lock update). A number without a lock entry is written and flagged
  provisional, as an interface number is.
- The number's type and range: the runtime's attach field is a signed 32-bit
  integer, so 1 to 2^31 − 1, starting at 1.
- Numbering scope: one sequence per system, or one per component (then the
  attach identity is the pair component and instance). The owner's runtime
  authenticates the component from the platform principal, so a per-component
  sequence may be enough; decide with decision 2 in view.
- Where the generated constants go: the Rust backend generates per IR package;
  the system is declared in an rsdl package.
- `ridl diff`: a changed instance number is breaking.

## Later

- Decision 2's transform and overrides, per platform.
- Service numbers, for a transport that puts a service id on the wire.
- Kotlin and TypeScript constants (the Kotlin generator is outside this
  repository).
- A manifest claim for an external runtime's attribute namespace (ADR-0022 open
  item 3).
