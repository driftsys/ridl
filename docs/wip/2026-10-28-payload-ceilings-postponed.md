# Payload ceilings — postponed points

Status: postponed by the owner on 2026-10-28. These three points were raised
while the ADR-0030 amendment of 2026-10-28 (driftsys/ridl#1109) was drafted.
None of them is acted on yet. Each one is taken up again when the condition in
its own section is met.

The owner settled, on the same day, what the amendment keeps: a default payload
ceiling for each kind (3,840 bytes for a signal and an event, 16,128 for a
command and a query, 65,280 for a memory-mapped fixed), a raise for one named
member through `[limits.interaction]`, and byte streams (ADR-0032,
driftsys/ridl#1115).

## 1. Platform configuration

The owner asked that the page size and the ceilings be configurable for a
platform. A proposed `[platform]` table in the workspace root's `ridl.toml`
would set the page size, the reserved bytes per cell, and the pages for each
kind, with the ADR's numbers as defaults. The effective ceilings would be
recorded in the IR and covered by the catalog hash, so that peers built for
different platforms do not attach.

Why it is postponed: there is one platform today. Defaults can become
configurable later without a break, because a workspace without the table keeps
the ADR's numbers. Configuring them now means deciding the allowed values, the
hashing rules and the interplay between machines before any second platform
needs them.

Taken up again when: a second platform needs different numbers. A system that
mixes targets needs a check that every payload fits every platform it reaches,
which rsdl can make per `machine` (the partition under `machine`,
driftsys/ridl#1085).

## 2. The cell layout in a normative record

The amendment's decision 16 states a cell layout: whole 4,096-byte pages, with
128 bytes reserved before the payload for a header and 128 bytes after it for a
trailer of end markers. That layout describes a store engine, and no store
engine is in this repository.

Proposal: ADR-0030 fixes only the numbers (3,840, 16,128 and 65,280 bytes) and
the rules that use them. The page arithmetic stays in the rationale, to explain
where the numbers come from, and does not bind an engine to a layout.

Taken up again when: the amendment is next revised, before it is merged.

## 3. Measure before deciding

No one has measured which sources newly fail under the draft ceilings: the
corpus under `evals/`, the examples, the test fixtures and the book. Nor is the
largest `string` bound that fits 3,840 bytes known for each codec (about 950
characters is an estimate). driftsys/ridl#1108 planned to measure both after the
decision.

Proposal: measure first, because the data may move a number, for example if real
signals cluster just above 3,840 bytes. The measurement needs no code change: a
temporary `[limits.kind]` table with the draft numbers in each unit makes
RIDL-151 report every payload above them. A fixed cannot be checked at 65,280
bytes this way, because MANI-022 refuses a value above 16,256 today.

Taken up again when: before the amendment is merged.
