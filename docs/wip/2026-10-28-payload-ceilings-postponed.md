# Payload ceilings — postponed points

Status: three points raised on 2026-10-28 while the ADR-0030 amendment of
2026-10-28 (driftsys/ridl#1109) was drafted. Sections 2 and 3 are done, in the
commit "docs(adr): state the ADR-0030 ceilings as numbers with a copy-time
rationale" on that pull request. Section 1 stays postponed, and is taken up
again when the condition in its section is met.

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

Done: decision 16 now states only the ceilings and the rules that use them. The
page arithmetic and the cell layout are in its rationale, as the explanation of
where the numbers come from, and do not bind an engine.

## 3. Measure before deciding

Done, measured on `main` at 6d9903db. The largest N for a `string [0..N]` in a
3,840-byte signal is 945 characters in FlatBuffers, 959 in proto3 and 955 in
podbuf. No shipped schema has a signal or an event near 3,840 bytes. The sources
that stop compiling are the two unbounded ROS 2 feedback events in
`evals/corpus/ros2/nav2_msgs` and the TypeScript codec fixture
`crates/ridl-backend-ts/tests/fixtures/codec/shape.ridl`. No number moved. The
results are in the Consequences of ADR-0030, and the follow-up work is in
driftsys/ridl#1108.
