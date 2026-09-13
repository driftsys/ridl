# The lock file and `ridl lock`

Status: working note, 2026-09-13, design agreed in conversation with Sebastien
section by section. Nothing here is ratified. Stage L1 of lane L in
[`2026-09-13-step1-lanes-plan.md`](2026-09-13-step1-lanes-plan.md). Read after
[`2026-09-12-rsdl-rewrite-decisions.md`](2026-09-12-rsdl-rewrite-decisions.md)
D-7 and the two identity studies
([`2026-09-12-interface-id-study.md`](2026-09-12-interface-id-study.md),
[`2026-09-12-interface-id-study-2.md`](2026-09-12-interface-id-study-2.md)).

Satisfies: rsdl decisions D-7 (numbers live outside the source, at every level)
and the §4 amendment items about identity and the lock; driftsys/ridl#315 at the
interface level. Coordinated on driftsys/ridl#328.

## 1. Identity widths

Approved by Sebastien on 2026-09-13, before the rest of this note was written
(gate GW of the lanes plan, §5).

| Identity         | Width | Range and scope                                                                                                                                                                                               |
| ---------------- | ----- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| member ordinal   | `u32` | 1-based, per interface, from position in the body (ridl §11). 0 is never an ordinal: the IR writes 0 for a package-level declaration, which has none (`crates/ridl-ir/proto/ridl/ir/v2/ir.proto:84-87`).      |
| interface number | `u32` | 1-based, per catalog, which is one package (D-7). 0 is never allocated.                                                                                                                                       |
| service number   | none  | A service is identified by its published dotted name and is not in the routing key (D-7, V-X2). A transport that needs a service id gets it as that backend's own attribute (D-6), not as a runtime identity. |

The routing key is (catalog slot, interface number, member ordinal). The catalog
slot is assigned per connection and is outside this note
([`2026-09-08-topology-vocabulary.md`](2026-09-08-topology-vocabulary.md) §6
gives it as `u8`).

**Why `u32`.** Every consumer that exists already carries 32 bits: the checker's
ordinal counter, the IR's `uint32 ordinal`, the proto3 backend's
`<Interface>Ordinal` enum (int32 values, bounded by `check_field_number` at
`crates/ridl-backend-proto/src/lib.rs:1105`), the FlatBuffers backend's
`enum <Interface>Ordinal : uint`
(`crates/ridl-backend-flatbuffers/src/lib.rs:1222`), and the catalog descriptor
plan's schema and hash input
([`2026-09-13-catalog-descriptor-plan.md`](2026-09-13-catalog-descriptor-plan.md)
`:296`, `:306`, `:319`, and `entry.number.to_le_bytes()` at `:1327`). A narrower
width would save at most 4 bytes in a frame header, and would add a range check
at lowering, a diagnostic, and a second place where the bound can drift from the
records.

**Bounds narrower than `u32` belong to the backend that has them.** A target
whose identifier is narrower checks the bound where it projects, as the proto3
backend checks its field-number range today. For example, the SOME/IP protocol
specification gives a 16-bit method id whose top bit is the event flag, so a
SOME/IP binding accepts an ordinal below 32768. No record states that bound, and
no such backend exists yet; the check is that backend's when it is written.

**What changes.** Only the identity types of
[`2026-09-08-ridl-rt-design.md`](2026-09-08-ridl-rt-design.md) §2 (`:205-207`):
`Ordinal(pub u32)`, `InterfaceId(pub u32)` per catalog instead of per service,
and no `ServiceId`. Lane A's `ridl-rt` spec (stage A1) states the types, and it
supersedes those lines. The IR, both wire backends, and Tasks 1, 3 and 4 of the
catalog descriptor plan do not change.

**Not decided here.**

- How a frame writes the two numbers, fixed width or variable length, is E11.1's
  (driftsys/ridl#257).
- The catalog hash. The vocabulary note §6 gives a `u64`, the catalog descriptor
  plan uses SHA-256 (`:84`, `:327`), and the `ridl-rt` note gives each interface
  its own hash (`:463-468`). D-8 and the runtime descriptors design own that
  question.

**This decision is wrong if** E11.1 finds that a frame header must fit in 8
bytes or fewer in total, or must place the numbers in an existing 16-bit field
of a transport's own header. Then the runtime types become `u16` with a range
check at lowering, and the IR and the descriptor keep `uint32`.

### Alternatives considered

- **`u16` for both, no service number.** Rejected. The IR has no 16-bit scalar,
  so it would stay `uint32` and lowering would add a range check and a new
  diagnostic. The catalog descriptor plan's Tasks 1, 3 and 4 would change (the
  schema, the stated bound, and the hash input). The saving is at most 4 bytes
  per frame header.
- **`uint32` in the IR and the descriptor, `u16` in the runtime.** Rejected for
  now, and kept as the fallback named above. Two widths, a diagnostic at
  lowering, and a descriptor reader that must range-check what it loads.
- **A service number in the runtime identity** (the `ridl-rt` note's
  `ServiceId`). Rejected by D-7: recomposing a service would change it, a
  service may list another package's interface, and the routing key does not
  contain it.
