# Design: measure the three payload encodings (driftsys/ridl#1117, E11.22)

Status: draft for approval. This is working memory under `docs/wip/`. The
durable result is a technote under `docs/technotes/`, written when the work
lands.

## Intent

The owner needs to know what each payload encoding costs when a system sends
many signal updates. The three encodings are proto3, FlatBuffers and podbuf. The
decision this informs (a guess, confirmed in conversation as "what matters is
the CPU % when there are many signal updates") is which encoding is the better
choice for a signal-heavy automotive target, and whether podbuf pays for itself.

The headline result is **CPU load**: the percentage of one core consumed while
the codec processes a stream of signal updates at a fixed rate. Encoded size and
heap allocations per operation are results of equal weight. Speed comes first,
then size.

Absolute times measured on an Apple Silicon Mac do not transfer to the target,
because that machine is faster and has larger caches. The measurement therefore
runs on real target-class cores and on host-independent instruction counts.

## Goals

- For every payload of the corpus and each of the three encodings, report:
  - encoded size in bytes;
  - heap allocations per encode, per decode and per verify;
  - CPU time per encode, decode and verify, and the CPU % of one core that a
    sustained stream of 4 000 updates per second costs.
- Rerun every table with one command per platform.
- State in a technote what each number does and does not show.

## Non-goals

- The transport. The measurement covers the codec only: encode, decode and
  verify. It excludes `ridl-transport-ws`, the socket and the framing.
- Cortex-R52 cycle counts. They need a board with the DWT cycle counter and stay
  out of scope.
- Changing any codec. This work measures; a finding that suggests a codec change
  becomes a separate issue.
- Making the bench part of `just build`.

## Anti-goals

- A table that reads well but compares unequal work (for example, podbuf
  verifying in place against proto3 decoding into owned values without saying
  so). The technote must state what each encoding's verify and decode do.
- Numbers from the Mac presented as target numbers.

## Corpus

The `veh-cluster` corpus (`crates/ridlc/tests/corpus/veh-cluster`) and
`examples/cabin`. The corpus leans toward small, frequent signal payloads, which
is the workload the owner cares about. A few stress payloads are added: long
strings, lists of structs, nested optionals and maps. Which signals are the
representative set is left to the builder (see "Left to the builder").

## Platforms and phasing

| Platform                                 | Gives                                                                                       |
| ---------------------------------------- | ------------------------------------------------------------------------------------------- |
| Pixel 5 (Cortex-A55 and Cortex-A76)      | wall-clock and process CPU time on real cores; the A55 is close to automotive A-class cores |
| AArch64 Linux container, `iai-callgrind` | deterministic instruction counts and simulated cache misses, independent of the host        |
| QEMU `mps3-an536` (Cortex-R52)           | ARMv8-R instruction counts for the `no_std` build; no time                                  |

Phase 1: size and allocations for the whole corpus, then CPU load on the Pixel
5. Phase 2: Valgrind counts, then QEMU R52. Phase 1 is the first deliverable and
is useful alone.

The Pixel 5 is connected to the owner's machine over `adb` (serial
`11181FDD4002MY`, Android 14, aarch64). Cores 0 to 5 are Cortex-A55 at up to 1.8
GHz, core 6 is a Cortex-A76 at up to 2.2 GHz, and core 7 is the prime Cortex-A76
at up to 2.4 GHz. The governor is `schedutil`. The device is not rooted, so the
frequency cannot be fixed and hardware counters are usually unavailable.

## Method constraints

- **Size and allocations are exact.** Count allocations with a counting global
  allocator in the bench binary. These counts are the same on every platform.
- **CPU load is a sustained run.** Process the updates at a fixed rate for a
  fixed time, pinned with `taskset` to one A55 core and, separately, to one A76
  core. Read process CPU time (`getrusage`) over the run. Also report the CPU
  time per update, so the CPU % at any other rate is computable.
- **The three encodings are interleaved** inside the run so that thermal
  throttling and governor changes affect them equally.
- **Warm up, then report medians over many iterations**, with the spread.
- **Equal work per encoding.** Each iteration does the same logical job per
  encoding: encode a value, verify the bytes, decode a value. The bench is
  generic over the encoding through `Payload<E>`, so the same code path runs for
  all three.
- **Record provenance** with the results: date, commit, platform, core, the
  governor and the observed frequency where readable, and the toolchain.

## Where the work lives

- A bench crate outside the published crates, as `evals/` is, and not a member
  of `just build`. It links code generated from the corpus with `ridl build`.
- An `xtask` recipe per platform runs it: one command for the host (size and
  allocations), one for the Pixel 5, one for the container, one for QEMU.
- The Pixel 5 binary is a static `aarch64-unknown-linux-musl` build
  (`cargo zigbuild`), pushed with `adb push` to `/data/local/tmp`. The musl
  target is not yet installed on the owner's machine.
- The technote `docs/technotes/<name>.md` holds the tables, per payload and as
  ratios between the encodings, with the method and the limits.

## Prior art in the repository

The archived podbuf design (`docs/archive/2026-10-24-podbuf-design.md`) holds
earlier spike measurements of layout. The technote links to it and states how
the new numbers relate to it, instead of restating it.

## Left to the builder

- The exact set of signal payloads taken from the corpus, and the shape of the
  stress payloads.
- How the bench obtains values to encode (hand-built, generated from the types,
  or taken from the corpus tests).
- Whether the generated crate for the bench is produced at build time by the
  xtask recipe or checked in.
- The duration and warm-up length of a sustained run.
- The crate name and the xtask recipe names, within the repository conventions.
- How to run `iai-callgrind` and QEMU for phase 2.

## The load target (stated by the owner)

The system goal is **4 000 updates per second sustained, plus a burst of 200
messages in 10 ms**. The owner deferred the burst on 2026-11-04: this work
reports the sustained load only. The burst (an instantaneous rate of 20 000
messages per second) and any normalisation over a short window are a later
extension, and per-update CPU time is reported so that they can be computed.

## The unit of work (answered by the code)

All three encodings implement the same trait, `ridl_rt::payload::Payload<E>`,
for one generated value type, when the manifest lists them in
`[backend.rust]
encodings`. The bench is generic over `E` and measures three
calls:

- **encode**: `Payload::encode` writes the value into a caller buffer;
- **verify**: `Payload::verify` checks the structure of the bytes and the typl
  constraints (ranges, patterns, map uniqueness) and returns a view;
- **decode**: `Payload::decode` builds the owned value from the proof that
  `Ref::verify` or `Ref::encode` produced.

An update therefore costs encode on the sender side, and verify plus decode on
the receiver side. The technote reports the three separately and their sum. The
technote states what each encoding's verify does, because they differ (podbuf
checks in place, proto3 and FlatBuffers differ in how much they walk).

## Decisions made here that the owner did not state

These are the author's calls and are open to change:

- **CPU load is derived, then checked.** The primary number is the per-update
  CPU time from a busy loop (no pacing), multiplied by the rate. One paced run
  at the sustained rate on the Pixel 5 checks the derivation and shows the cost
  of timers and wake-ups; a paced run that disagrees with the derived figure is
  reported, not hidden.
- **The headline stream** is the equal-weight mix of the signal payloads of
  `veh-cluster` and `examples/cabin`. The per-payload table is the full result.
  The weights are a guess until the owner gives real rates.
- **Phase 1** is host size and allocations plus the Pixel 5 CPU load. It may
  merge as more than one pull request.
- **No verdict.** The technote reports numbers, ratios and caveats. It does not
  recommend an encoding unless the numbers make it unambiguous. A run counts
  when it has at least 30 sustained repetitions and reports their spread.
- **The phone is a dependency of the Pixel 5 recipe only.** The host recipe
  (size and allocations) and the container recipe run without it. Nothing here
  runs in CI.

## Alternatives considered

- **Per-operation microbenchmarks only** (criterion style). Rejected as the
  headline because the owner's question is CPU % under sustained load, which
  includes cache and allocator effects. Per-update cost is still reported.
- **Measuring through `ridl-transport-ws`.** Rejected for now: the owner asked
  for the codec only. The result can be extended later.
- **Host-only measurement on the Mac.** Rejected: the absolute times do not
  transfer to the target.
- **Hardware counters on the Pixel 5.** Not relied on: they are usually
  unavailable without root.
