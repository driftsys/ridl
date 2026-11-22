---
applyTo: "**/*.typl,**/*.ridl,**/*.rsdl,**/ridl.toml"
description: Constraints that every RIDL-family source must meet.
---

# Rules

- Always give `integer` an explicit range (`TYPL-101`).
- Always give `float` both a range and a `step` (`TYPL-102`).
- Always give `string` and `bytes` explicit bounds (`TYPL-103`).
- Never use a bare `string` or `bytes` as a field type; define a named `type`,
  or constrain the primitive inline (`TYPL-208`).
- Always give an array explicit bounds, `[T; min..max]` (`TYPL-201`).
- Always give a map explicit bounds, `[K: V; min..max]` (`TYPL-202`).
- Never write a union arm of a primitive type; arms reference named types only
  (`TYPL-204`).
- Never write a recursive struct, enum, or union, directly or transitively
  (`TYPL-206`).
- Never re-declare a name or value that a `reserved` entry lists (`TYPL-210`).
- Never publish a change that `ridl diff` classifies as breaking, such as a
  struct field or union arm inserted, reordered or removed, or an enum value
  removed or renumbered; compare with the baseline first
  ([the verdicts](https://driftsys.github.io/ridl/evolving-an-interface.html#the-verdicts))
  (`RIDL-407`).
- Never write a wildcard, relative, or re-exporting import (`TYPL-003`).
- Never use a semicolon; a newline or a comma separates, with an optional
  trailing comma (`FORM-102`).
- Never put an explicit return type on a `command` (`RIDL-104`).
- Never write a `query` that returns `()` (`RIDL-105`).
- Never use `require` or `ensure` on a `signal`, `event`, or `fixed`
  (`RIDL-301`).
- Never use `ensure` on a `command` (`RIDL-302`).
- Always give a fallible `query` a result union with a success arm (`RIDL-303`).
- Never put a strict periodic `@Xms` timing on anything but a `signal`
  (`RIDL-103`).
- Never redeclare an interaction under a name already `reserved` (`RIDL-401`).
- Never declare two interfaces in one `service` that share a member name
  (`RIDL-144`).
