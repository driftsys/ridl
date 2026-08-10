# Sans-IO and Readiness, Applied to a Signal

This note is informative — it binds nothing. The decision it explains is
[ADR-0018](../decisions/ADR-0018-runtime-core-and-generated-surface.md) decision
2, and the stories that build it are epic E11 in
[`docs/ROADMAP.md`](../ROADMAP.md). Where this note and the ADR disagree, the
ADR is normative and this note is stale.

It exists because "sans-IO" and "readiness state machine" are the two terms in
ADR-0018 that a reader is most likely to have met only in passing, and because
what they mean for a **signal** in particular is not obvious from the general
description. There is no runtime in this workspace yet; everything below
describes the design E11 builds, not code that runs.

## Sans-IO in one paragraph

A sans-IO core implements a protocol and performs no input or output. It owns no
socket, no thread, no timer and no clock. Everything reaches it as an argument
and leaves it as a return value. In `ridl-rt` that is four calls:

    on_bytes(bytes)         bytes that arrived, from wherever
    poll(now) -> Deadline   do bounded work; say when to call me next
    pending_out()           bytes that want to be sent
    consume_out(n)          n of them were actually sent

The caller — a driving loop — owns the socket, the clock and the scheduling. It
reads bytes and hands them in, calls `poll` with the current time, takes
whatever came out and writes it, then sleeps until the deadline `poll` returned
or until the socket is readable, whichever is sooner.

## Readiness rather than blocking

A **readiness state machine** answers two questions and never waits:

1. What can be done with what is known right now?
2. If nothing else arrives, when is the next moment I have work to do?

That second answer is the `Deadline`. It is the whole reason the core does not
need a timer of its own: instead of arming one, it tells the caller when to come
back, and the caller — which already owns a clock — decides how to wait.

Compare the three common shapes:

| Shape             | Who owns the clock and socket | Needs an executor |
| ----------------- | ----------------------------- | ----------------- |
| Blocking          | The core, inside a call       | No, needs threads |
| `async`/`await`   | The executor                  | Yes               |
| Sans-IO readiness | The caller                    | No                |

The asymmetry recorded in ADR-0018 decision 2 is what settles it: **a sans-IO
core can present an async face; an async core cannot present a sans-IO face.**
Wrapping readiness in `async` is a thin adapter — await the socket, call `poll`,
sleep to the deadline. Going the other way needs an executor, which is the
dependency being avoided. So one core drives unchanged from an epoll loop, an
RTOS task, a browser callback, or a Tokio task, and the platform ladder does not
fork the protocol implementation at the point where the executor runs out.

Two consequences worth naming. A `poll` does bounded work, so a worst-case
execution time is arguable rather than a matter of trusting a runtime. And a
test needs no executor, no socket and no real time — which is E11.4's exit
criterion, stated that way on purpose: if a test needs real time, the core is
not sans-IO.

## What this means for a signal

A signal is the case where readiness is least about bytes and most about time,
which is why it is worth working through separately.

**A signal is a slot, not a stream.** ridl §4.4 says the channel is never empty:
there is an init value before anything is published, and afterwards the last
value stands until it is replaced. So the core does not hold a queue of signal
messages waiting to be consumed. It holds one slot per signal, in the region for
its interface, at an offset derived from the ordinal (E11.2).

**Publication is a write plus a stamp.** A provider writes the slot under the
seqlock discipline, the core stamps the envelope (ridl §3.1 — sender timestamp
and per-channel sequence), bumps the counters, and produces outbound frames for
whoever subscribed. Those frames land in `pending_out`. Nothing blocks, and
nothing is sent by the core itself.

**Reading is a read.** A consumer reads the slot. Provenance — `init`, `live` or
`invalid` (ridl §4.5) — and staleness are properties computed at read time
against `now`, not messages that were delivered earlier and remembered.

**The deadline is usually about time, not bytes.** This is the part specific to
signals. A signal carries a timing contract (ridl §9) whose two bounds are
`min`, the rate floor, and `max`, the staleness bound. On a signal — state, not
occurrence — `max` is a **refresh ceiling**: the provider must re-publish even
if nothing changed. So one declared bound gives the core two deadlines, and
neither of them involves a byte arriving:

- **provider side** — re-publish by `max`, unchanged value and all;
- **consumer side** — if nothing arrived by then, the value that was `live` is
  now stale.

Both are moments where the interesting thing is that **nothing happened**. A
purely queue-driven core has no natural way to notice a non-event; a readiness
core does, because "the earliest instant at which some signal owes a refresh or
goes stale" is exactly what `poll` returns as its deadline. The caller sleeps
until then, calls `poll`, and the core acts with no input at all.

So the loop is driven from two sides, and the second is the one that is easy to
forget:

- bytes arriving → `on_bytes` → a slot updates, subscribers get frames
- **the deadline expiring → `poll(now)` → a refresh is owed, or a slot goes
  stale, with no bytes at all**

`min` is the mirror image and needs no deadline: on a signal it is a
**debounce**, and a faster update is coalesced into the next sample rather than
held for later — which is the next point.

**Coalescing falls out of the slot.** If a signal is written three times between
two polls, a subscriber sees the last value once. That is not an optimisation
the core chose; it is what a last-value slot means, and it is why ADR-0018
decision 12 can say plainly that **signals need no ring depth — they are the
store**. A slot cannot have a backlog, so there is nothing to size.

The derived depth in E11.5 — `ceil((service_period + jitter) / rate_floor)` —
belongs to the kinds that do queue. What is worth carrying across from the
signal case is the reasoning rather than the formula: a depth is derivable at
all because a consumer can only fall behind by a bounded amount, set by how
often it is scheduled and how late that scheduling can be, and not by how fast a
producer might run.

**Coherence is per interface, not per signal.** The interface is the unit whose
signals are all published by one step, so the interface-level generation counter
is what a reader checks to know it saw one consistent set rather than a mix of
two (ridl §14, and ADR-0018 decision 8). Readiness does not change that; it just
means the check happens on the reading side, at read time, with no coordination
call.

## Where the analogy stops

Signals are the easy case because a slot has no backlog. The other four
interaction kinds are not all like this:

- an **event** is a discrete occurrence and every occurrence matters, so
  occurrences are **queued, not coalesced** (ridl §5) — which is the opposite of
  the signal case, and the reason a ring exists at all. `min` on an event is a
  throttle on the provider rather than a debounce (ridl §9, the bounds table).
- **`command` and `query`** share one mechanism, not two. Both take the range
  form only, where `min` is a call throttle on the **caller** and `max` is the
  response bound on the provider (ridl §9.3). What differs is only what
  responding means: for a `query` the reply, and for a `command` **acceptance**
  — the §6.1 delivery acknowledgment — because a command's bound covers
  admission and queueing and explicitly not execution. So the core tracks a
  deadline per in-flight call rather than per slot, but it is the same deadline
  in both cases.
- a **fixed** value is provisioned and never changes, so it contributes no
  deadline at all.

All four are still readiness — the core still answers "what now, and when next"
— but only the signal has the property that its most common transition needs no
input whatsoever. An event's queue drains when it drains, and an RPC's deadline
belongs to a call that some caller made.

## Related reading

- [ADR-0018](../decisions/ADR-0018-runtime-core-and-generated-surface.md)
  decisions 1, 2, 8 and 12 — the layering, sans-IO, the store and its counters,
  and the derived ring depth.
- [`docs/ROADMAP.md`](../ROADMAP.md) epic E11 — the stories, with E11.1 (the
  frame) and E11.2 (the store layout) blocking the rest.
- [ridl language reference](../specification/ridl-language-reference.md) §3.1
  (the envelope), §4.4 (last value), §4.5 (invalid propagation) and §9 (timing).
