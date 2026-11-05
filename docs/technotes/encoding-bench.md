# Encoding bench: what the three payload encodings cost

Informative. This note binds nothing. It reports a measurement of the three
payload encodings (FlatBuffers, proto3, podbuf) and states what each number
shows. The measuring code is in
[`bench/encodings/`](../../bench/encodings/README.md); the tracking issue is
driftsys/ridl#1117.

## Purpose

When a system sends many signal updates, the codec is part of the CPU load. The
bench puts one number on that cost for each payload of the corpus and each
encoding: the load, in percent of one core, of 4 000 updates per second. It also
reports the encoded size and the heap allocations of encode, verify and decode,
because a smaller buffer and a codec that does not allocate matter on the
targets that have a small memory and no allocator.

The bench measures the codec only: encode, verify and decode through
`Payload<E>` of `ridl-rt`. It excludes the transport, the socket and the
framing. It changes no codec.

## Method

- **Corpus.** Every signal of the `veh-cluster` corpus and of `examples/cabin`,
  and four stress payloads from `bench/encodings/stress/` (a long string, a list
  of structs, nested optionals, a map). A signal whose payload type the bench
  cannot build a value of is listed as not built.
- **Equal work.** One update is: encode a value into a stack buffer, verify the
  bytes, decode a value from the verified bytes. The code path is generic over
  the encoding (`Ref::<T, E>::encode`, `verify`, `decode`), so the three
  encodings run the same bench code. Every result goes through `black_box`.
- **Interleave.** The three encodings of a payload are timed in rotation inside
  the same run: sample `k` starts with encoding `k mod 3`. Frequency changes and
  thermal throttling therefore affect the three alike.
- **Warm-up and samples.** The batch size is calibrated so that a batch lasts at
  least 200 microseconds; 3 warm-up batches are discarded; 30 samples follow.
  The time of one update is the time of one batch divided by the batch size. The
  figure is the median of the 30 samples. The tables also give the fastest and
  the slowest sample and the range as a share of the median. A cell whose
  `(p95 - p05) / median` or `(max - min) / median` is above 10 % carries the
  marker `†`: its median is not stable.
- **Clock.** Thread CPU time (`CLOCK_THREAD_CPUTIME_ID`). It does not advance
  while the thread sleeps.
- **Headline: the derived figure.** The CPU load at 4 000 updates per second is
  derived from the unpaced cost: `median time of one update x 4 000 / 1 s`, in
  percent of one core. The CPU load at another rate is the time per update times
  that rate.
- **Paced figure.** A second run does one update per tick at 4 000 per second,
  sleeping to the next tick, and reads process CPU time (`getrusage`) over the
  run. It is the load a program that really waits between updates would show. It
  includes the cost of the sleep and of the wake-up of the thread. The tables
  give it next to the derived figure and their difference. The derived figure is
  the headline because the paced one is not stable and is far from the derived
  one: in the host tables the paced figure ranges from 0.6 % to 3.8 % for signal
  payloads whose derived load is below 0.02 %, so the update is a small part of
  what the paced run measures. A paced run cannot be split between the three
  encodings, so the paced runs are one cell after the other, not interleaved.
- **Pinning.** On the Pixel 5, `taskset` pins the run to one Cortex-A55 core
  (cpu2) and, in a second run, to one Cortex-A76 core (cpu6). The harness does
  not pin itself. The governor is `schedutil` and the phone is not rooted, so
  the frequency is not fixed; the bench records the governor and the frequency
  of each core before and after the run in a `.cpufreq.txt` file next to the
  result. On the development machine nothing is pinned.
- **Allocation counts.** The bench binary installs a global allocator that
  forwards to the system allocator and counts the calls to `alloc` made by the
  current thread. The default `alloc_zeroed` and `realloc` of `GlobalAlloc` call
  `alloc`, so they are counted. The counts for verify and for decode are taken
  separately: the decode column counts the allocations of `verify` followed by
  `decode`. The counts are the same on every platform. The stack buffer for
  encode is `T::BUFFER`, so the output buffer is not counted.
- **Provenance.** Each result file holds the date, the commit, the platform, the
  core label, the toolchain, and the governor and frequency of the core where
  the system lets the program read them.

## What verify and decode do in each encoding

The three do not do the same work. The sum "encode, verify, decode" is the same
job, but the share of work in each step differs, and a table of the sum hides
that.

- **podbuf** reads in place. `verify` is a call tree with one function per head
  type; it checks the preamble, the bounds and alignment of every descriptor's
  data, the string terminator and UTF-8, the order of map keys, booleans, enum
  values, the zero bytes of an absent optional, and the typl constraints. It
  allocates nothing and copies nothing. `decode` builds the owned value from the
  view that `verify` returned; it allocates only for `String`, `Vec` and maps.
  The bench does not use the in-place accessors, which are the reason podbuf
  exists ([the podbuf design](../archive/2026-10-24-podbuf-design.md), section
  1); a consumer that reads two fields of a large payload pays less than the
  bench shows. See [the podbuf codec record](../design/podbuf-codec.md).
- **FlatBuffers** `verify` walks the structure (the root offset, the vtables,
  the offsets, the string and vector headers) and checks the typl constraints it
  can reach, including the uniqueness of map keys by pairwise comparison
  (quadratic in the number of entries). It allocates nothing. `decode` builds
  the owned value from the verified bytes. `encode` builds from the end of the
  buffer towards the start, and, for a vector of tables or strings, collects the
  element positions in a `Vec` before it writes the vector (the generated
  encoder of the stress package does this), which is an allocation. See
  [the FlatBuffers codec record](../design/flatbuffers-codec.md).
- **proto3** `verify` walks the fields in order with a fixed-size table of seen
  fields per message, refuses a field written twice, a varint wider than its
  type and a string that is not UTF-8, skips unknown fields, and checks the same
  typl constraints. It allocates nothing. The view is the verified bytes with no
  field accessors, because proto3 is not a random-access format; so `decode`
  reads the message again from the bytes, and every non-optional nested field
  starts as the default sub-tree and is replaced when present. `encode` computes
  the exact length first and writes forwards. See
  [the proto3 codec record](../design/proto3-codec.md).

None of the three applies a length check in `verify` (ADR-0030 decision 19).

## What each number shows, and what it does not

- **Encoded size** is exact and the same on every platform. It is the size of
  the buffer after `encode`, including the 8-byte preamble of podbuf.
- **Heap allocations** are exact. They count `alloc` calls, not bytes, and not
  allocations in code the bench does not run. A count of zero means that the
  codec itself did not call the allocator for that payload.
- **Time per update** is a CPU time of encode, verify and decode on a warm cache
  with the same value every time. It does not show the cost of a cold cache, of
  branch mispredictions on varying data, or of a value that changes between
  updates. A payload that changes size or shape is not modelled.
- **CPU load at 4 000 updates per second (derived)** is the time per update
  scaled to the rate. It shows the codec's share of a core. It does not include
  the transport, the application, the scheduler or the timer.
- **Paced load** shows what a program that waits between updates measures; in
  the host tables it varies from 0.6 % to 3.8 % for payloads whose derived load
  is below 0.02 %, so it is dominated by something other than the codec (the
  cost of the timer and of the wake-up on an unpinned machine is the likely
  cause; it is not a measurement).
- **Ratios** divide each encoding by podbuf. A ratio above 1.00 means the
  encoding is larger or slower than podbuf on that payload. A ratio is computed
  only when both cells are measured.
- **The mix.** The equal-weight mix is a summary, not a result. It is given
  twice, because the corpus repeats payload types: several signals carry the
  same payload type, so a mix per signal gives those types more weight than a
  mix per distinct payload type does. The per-payload tables are the full
  result. A payload with a refused or not-built cell is in no mix and no ratio.
- **Refused and not built** are different. A refused cell means the encoding
  refused the value or the bytes it had just written. A not-built payload means
  the bench could not build a value of it, so no encoding was tried. A not-built
  payload is not an encoding refusal.
- **Floating point.** The bench does not isolate the cost of floating-point
  checks, which the podbuf design also puts out of scope.

## Relation to the podbuf design spike

[The podbuf design](../archive/2026-10-24-podbuf-design.md), section 12, holds a
spike: instruction counts of verify and encode on a Cortex-R52 under QEMU, for
five types, with a hand-written podbuf. This bench differs in three ways. It
measures the generated codecs, not a hand-written one. It reports CPU time and
allocations, not instruction counts, so it shows time on a core with caches and
a branch predictor, and the spike shows instructions. And its unit is the sum of
encode, verify and decode, where the spike gives verify and encode apart. The
two do not replace each other, and the bench does not repeat the spike's
instruction counts or its code-size figures.

## Host results

**Measured on the development machine; absolute times do not transfer to the
target.** The machine is an Apple Silicon Mac (macOS, aarch64) that is faster
than the target cores and has larger caches. The run is not pinned and the
machine may run other processes, which is why many cells carry the marker `†`.
The numbers show the shape of the work (which payloads are cheap, how the
encodings compare on this machine), not the load on a vehicle's computer.

- Date: 2026-11-05
- Commit: 48ff069e7ba476cdc4be21055075b978739e996b
- Command:
  `cargo run --release -p encoding-bench -- --out results/host-macos-arm64.json --core-label "development machine (macOS arm64, not pinned)" --duration 1 --paced`,
  then `cargo xtask bench report`
- Toolchain: rustc 1.98.1 (48a229cea 2026-09-01)
- Run: 4 000 updates per second, 30 samples per cell, 1 s of CPU time per cell
  and encoding, paced run included

What the tables show, without a recommendation:

- Size: proto3 is the smallest on every measured signal row and FlatBuffers the
  largest. The stress payloads follow the same order (proto3, podbuf,
  FlatBuffers), and the gap between proto3 and podbuf on the long string is
  small (2.7 %).
- Time: on this machine podbuf has the lowest median time on every measured
  signal row and on three of the four stress rows (list of structs, nested
  optionals, map). On the long string it has the highest. Many of the signal
  cells are flagged, so a ratio close to 1 would not be reliable; the time
  ratios to podbuf of the signal rows are 1.48 or more.
- Load: every measured signal payload costs less than 0.02 % of one core at 4
  000 updates per second on this machine; the largest figure of the stress
  payloads is 2.1 % (proto3, the map).
- Allocations: the codecs make no allocation for the signal payloads. Only the
  stress payloads allocate: decode allocates for every one of them, and encode
  allocates for FlatBuffers (list of structs, map) and podbuf (map); the cause
  of the podbuf map allocation is not established here.
- Paced against derived: the paced figure is 0.6 % to 3.8 % for the signal
  payloads and the derived figure is below 0.02 %. The paced figure is not a
  codec figure, as the method section explains.

The tables are the output of `cargo xtask bench report`, unedited except for the
heading levels. The host tables are a snapshot of one run at commit 48ff069e,
the base commit, taken before the report code existed. The Pixel 5 tables below
are a snapshot of one run at commit 69fdf811. The directory
`bench/encodings/results/` is git-ignored, so the JSON behind the tables is not
in the repository. Refresh the host tables by rerunning the host command given
above and the Pixel tables by rerunning `just bench-pixel`, then run
`cargo xtask bench
report`.

### Mix

Equal-weight mix, per signal: each signal counts once, so a payload type that
several signals share counts once for each of them. Only signals whose three
encodings are measured are counted; the last column gives how many.

| Metric                                                 | FlatBuffers | proto3 | podbuf | FlatBuffers / podbuf | proto3 / podbuf | Units |
| ------------------------------------------------------ | ----------- | ------ | ------ | -------------------- | --------------- | ----- |
| Size (bytes)                                           | 24.5        | 3.6    | 16.0   | 1.53                 | 0.23            | 15    |
| Heap allocations per update (encode + verify + decode) | 0.0         | 0.0    | 0.0    | n/a                  | n/a             | 15    |
| Time per update (median ns)                            | 33.6        | 29.5   | 16.6   | 2.03                 | 1.78            | 15    |
| CPU % of one core at 4000 updates per second (derived) | 0.0134      | 0.0118 | 0.0066 | 2.03                 | 1.78            | 15    |

Equal-weight mix, per payload type: each payload type counts once, however many
signals share it (the value of a type is the mean over its signals). Only
payload types whose signals are measured in all three encodings are counted; the
last column gives how many.

| Metric                                                 | FlatBuffers | proto3 | podbuf | FlatBuffers / podbuf | proto3 / podbuf | Units |
| ------------------------------------------------------ | ----------- | ------ | ------ | -------------------- | --------------- | ----- |
| Size (bytes)                                           | 25.3        | 3.4    | 16.0   | 1.58                 | 0.21            | 6     |
| Heap allocations per update (encode + verify + decode) | 0.0         | 0.0    | 0.0    | n/a                  | n/a             | 6     |
| Time per update (median ns)                            | 32.0        | 28.1   | 15.7   | 2.04                 | 1.79            | 6     |
| CPU % of one core at 4000 updates per second (derived) | 0.0128      | 0.0112 | 0.0063 | 2.04                 | 1.79            | 6     |

### Per payload

#### Encoded size

Encoded size in bytes of one payload value.

| Payload                                 | FlatBuffers | proto3    | podbuf    |
| --------------------------------------- | ----------- | --------- | --------- |
| veh.cluster.VehicleStatus.currentSpeed  | 24          | 5         | 16        |
| veh.cluster.VehicleStatus.engineTemp    | 24          | 5         | 16        |
| veh.cluster.VehicleStatus.warnings      | 24          | 2         | 16        |
| veh.cluster.WheelHistory.wheelTicks     | 24          | 2         | 16        |
| veh.cluster.WheelHistory.wheelTemp      | 24          | 5         | 16        |
| veh.cluster.WheelHistory.wheelSlipRate  | 24          | 5         | 16        |
| veh.cluster.WheelSummary.wheelHealth    | 24          | 2         | 16        |
| veh.cluster.wheels.ticks                | 24          | 2         | 16        |
| veh.cluster.WheelDiagnostics.wheelTicks | not built   | not built | not built |
| veh.hvac.cabin.cabinTemp                | 24          | 5         | 16        |
| veh.hvac.cabin.targetTemp               | 24          | 5         | 16        |
| veh.hvac.cabin.blowerDuty               | 24          | 5         | 16        |
| veh.hvac.cabin.fanLevel                 | 24          | 2         | 16        |
| veh.hvac.cabin.compressorLoad           | 24          | 5         | 16        |
| veh.cabin.Cabin.temperature             | 24          | 2         | 16        |
| veh.cabin.Horn.active                   | 32          | 2         | 16        |
| bench.stress.Stress.longString          | 544         | 515       | 529       |
| bench.stress.Stress.itemList            | 600         | 221       | 400       |
| bench.stress.Stress.nestedOptional      | 80          | 17        | 49        |
| bench.stress.Stress.map                 | 600         | 204       | 384       |

#### Heap allocations

Heap allocations of one encode / verify / decode.

| Payload                                 | FlatBuffers | proto3     | podbuf     |
| --------------------------------------- | ----------- | ---------- | ---------- |
| veh.cluster.VehicleStatus.currentSpeed  | 0 / 0 / 0   | 0 / 0 / 0  | 0 / 0 / 0  |
| veh.cluster.VehicleStatus.engineTemp    | 0 / 0 / 0   | 0 / 0 / 0  | 0 / 0 / 0  |
| veh.cluster.VehicleStatus.warnings      | 0 / 0 / 0   | 0 / 0 / 0  | 0 / 0 / 0  |
| veh.cluster.WheelHistory.wheelTicks     | 0 / 0 / 0   | 0 / 0 / 0  | 0 / 0 / 0  |
| veh.cluster.WheelHistory.wheelTemp      | 0 / 0 / 0   | 0 / 0 / 0  | 0 / 0 / 0  |
| veh.cluster.WheelHistory.wheelSlipRate  | 0 / 0 / 0   | 0 / 0 / 0  | 0 / 0 / 0  |
| veh.cluster.WheelSummary.wheelHealth    | 0 / 0 / 0   | 0 / 0 / 0  | 0 / 0 / 0  |
| veh.cluster.wheels.ticks                | 0 / 0 / 0   | 0 / 0 / 0  | 0 / 0 / 0  |
| veh.cluster.WheelDiagnostics.wheelTicks | not built   | not built  | not built  |
| veh.hvac.cabin.cabinTemp                | 0 / 0 / 0   | 0 / 0 / 0  | 0 / 0 / 0  |
| veh.hvac.cabin.targetTemp               | 0 / 0 / 0   | 0 / 0 / 0  | 0 / 0 / 0  |
| veh.hvac.cabin.blowerDuty               | 0 / 0 / 0   | 0 / 0 / 0  | 0 / 0 / 0  |
| veh.hvac.cabin.fanLevel                 | 0 / 0 / 0   | 0 / 0 / 0  | 0 / 0 / 0  |
| veh.hvac.cabin.compressorLoad           | 0 / 0 / 0   | 0 / 0 / 0  | 0 / 0 / 0  |
| veh.cabin.Cabin.temperature             | 0 / 0 / 0   | 0 / 0 / 0  | 0 / 0 / 0  |
| veh.cabin.Horn.active                   | 0 / 0 / 0   | 0 / 0 / 0  | 0 / 0 / 0  |
| bench.stress.Stress.longString          | 0 / 0 / 1   | 0 / 0 / 1  | 0 / 0 / 1  |
| bench.stress.Stress.itemList            | 1 / 0 / 17  | 0 / 0 / 19 | 0 / 0 / 17 |
| bench.stress.Stress.nestedOptional      | 0 / 0 / 2   | 0 / 0 / 2  | 0 / 0 / 2  |
| bench.stress.Stress.map                 | 1 / 0 / 17  | 0 / 0 / 19 | 1 / 0 / 17 |

#### Time per update

CPU time of one update (encode, verify and decode), median in ns, with the
fastest and slowest sample and the range as a share of the median. † spread
above 10 %: the figure is not stable.

| Payload                                 | FlatBuffers                       | proto3                            | podbuf                            |
| --------------------------------------- | --------------------------------- | --------------------------------- | --------------------------------- |
| veh.cluster.VehicleStatus.currentSpeed  | 35.3 (33.9 to 42.6; 25 %) †       | 32.5 (32.0 to 33.1; 4 %)          | 21.9 (21.6 to 23.0; 6 %)          |
| veh.cluster.VehicleStatus.engineTemp    | 42.7 (38.8 to 55.1; 38 %) †       | 38.2 (37.2 to 59.9; 59 %) †       | 23.8 (22.1 to 34.6; 53 %) †       |
| veh.cluster.VehicleStatus.warnings      | 25.5 (24.6 to 30.8; 24 %) †       | 22.6 (22.4 to 24.1; 8 %)          | 9.8 (9.6 to 10.6; 10 %) †         |
| veh.cluster.WheelHistory.wheelTicks     | 25.3 (24.8 to 26.8; 8 %)          | 22.1 (21.9 to 23.7; 8 %)          | 9.7 (9.6 to 10.5; 9 %)            |
| veh.cluster.WheelHistory.wheelTemp      | 41.7 (38.8 to 62.0; 56 %) †       | 37.3 (37.0 to 51.5; 39 %) †       | 22.3 (22.0 to 29.0; 31 %) †       |
| veh.cluster.WheelHistory.wheelSlipRate  | 41.7 (40.1 to 46.7; 16 %) †       | 34.8 (33.5 to 36.4; 8 %)          | 22.4 (21.5 to 23.0; 7 %)          |
| veh.cluster.WheelSummary.wheelHealth    | 24.9 (24.8 to 25.9; 5 %)          | 22.1 (22.0 to 22.5; 3 %)          | 9.7 (9.6 to 10.0; 4 %)            |
| veh.cluster.wheels.ticks                | 25.2 (24.7 to 26.2; 6 %)          | 22.1 (22.0 to 22.6; 3 %)          | 9.7 (9.6 to 10.2; 6 %)            |
| veh.cluster.WheelDiagnostics.wheelTicks | not built                         | not built                         | not built                         |
| veh.hvac.cabin.cabinTemp                | 41.3 (40.3 to 61.6; 52 %) †       | 37.2 (35.9 to 43.8; 21 %) †       | 22.2 (21.4 to 23.7; 10 %) †       |
| veh.hvac.cabin.targetTemp               | 41.7 (38.6 to 50.2; 28 %) †       | 37.2 (37.1 to 47.4; 28 %) †       | 22.2 (22.0 to 23.2; 6 %)          |
| veh.hvac.cabin.blowerDuty               | 41.4 (38.4 to 45.1; 16 %) †       | 34.9 (34.7 to 35.5; 2 %)          | 22.4 (22.3 to 22.8; 2 %)          |
| veh.hvac.cabin.fanLevel                 | 24.9 (24.4 to 26.3; 7 %)          | 22.1 (21.9 to 22.4; 2 %)          | 9.6 (9.6 to 9.9; 3 %)             |
| veh.hvac.cabin.compressorLoad           | 41.6 (39.1 to 44.0; 12 %) †       | 34.8 (34.6 to 35.0; 1 %)          | 22.4 (22.2 to 22.9; 3 %)          |
| veh.cabin.Cabin.temperature             | 24.9 (24.7 to 26.3; 6 %)          | 22.8 (22.3 to 23.4; 5 %)          | 9.9 (9.8 to 10.5; 7 %)            |
| veh.cabin.Horn.active                   | 26.0 (25.7 to 27.6; 7 %)          | 22.1 (21.9 to 22.7; 3 %)          | 10.5 (10.5 to 11.3; 8 %)          |
| bench.stress.Stress.longString          | 164.9 (160.5 to 168.6; 5 %)       | 147.7 (146.4 to 150.4; 3 %)       | 182.4 (179.1 to 196.8; 10 %)      |
| bench.stress.Stress.itemList            | 1689.9 (1618.2 to 2053.3; 26 %) † | 2167.5 (2088.8 to 2438.7; 16 %) † | 1469.4 (1435.1 to 1624.3; 13 %) † |
| bench.stress.Stress.nestedOptional      | 201.9 (197.5 to 208.2; 5 %)       | 197.8 (192.3 to 204.0; 6 %)       | 143.9 (138.7 to 146.8; 6 %)       |
| bench.stress.Stress.map                 | 3155.9 (3100.1 to 3257.5; 5 %)    | 5245.6 (5133.1 to 5408.6; 5 %)    | 1728.3 (1696.9 to 1840.8; 8 %)    |

#### CPU load, derived

CPU load in percent of one core at 4000 updates per second, derived from the
median time of one update (the headline figure).

| Payload                                 | FlatBuffers | proto3    | podbuf    |
| --------------------------------------- | ----------- | --------- | --------- |
| veh.cluster.VehicleStatus.currentSpeed  | 0.0141      | 0.0130    | 0.0088    |
| veh.cluster.VehicleStatus.engineTemp    | 0.0171      | 0.0153    | 0.0095    |
| veh.cluster.VehicleStatus.warnings      | 0.0102      | 0.0091    | 0.0039    |
| veh.cluster.WheelHistory.wheelTicks     | 0.0101      | 0.0089    | 0.0039    |
| veh.cluster.WheelHistory.wheelTemp      | 0.0167      | 0.0149    | 0.0089    |
| veh.cluster.WheelHistory.wheelSlipRate  | 0.0167      | 0.0139    | 0.0090    |
| veh.cluster.WheelSummary.wheelHealth    | 0.0100      | 0.0088    | 0.0039    |
| veh.cluster.wheels.ticks                | 0.0101      | 0.0088    | 0.0039    |
| veh.cluster.WheelDiagnostics.wheelTicks | not built   | not built | not built |
| veh.hvac.cabin.cabinTemp                | 0.0165      | 0.0149    | 0.0089    |
| veh.hvac.cabin.targetTemp               | 0.0167      | 0.0149    | 0.0089    |
| veh.hvac.cabin.blowerDuty               | 0.0166      | 0.0139    | 0.0090    |
| veh.hvac.cabin.fanLevel                 | 0.0100      | 0.0088    | 0.0039    |
| veh.hvac.cabin.compressorLoad           | 0.0166      | 0.0139    | 0.0089    |
| veh.cabin.Cabin.temperature             | 0.0100      | 0.0091    | 0.0040    |
| veh.cabin.Horn.active                   | 0.0104      | 0.0088    | 0.0042    |
| bench.stress.Stress.longString          | 0.0660      | 0.0591    | 0.0730    |
| bench.stress.Stress.itemList            | 0.6760      | 0.8670    | 0.5878    |
| bench.stress.Stress.nestedOptional      | 0.0808      | 0.0791    | 0.0575    |
| bench.stress.Stress.map                 | 1.2623      | 2.0982    | 0.6913    |

#### Ratios to podbuf

Each encoding divided by podbuf: above 1.00 the encoding is larger or slower
than podbuf, below 1.00 smaller or faster. A refused or not-built cell has no
ratio and is marked.

| Payload                                 | Size: FlatBuffers / podbuf | Size: proto3 / podbuf | Time: FlatBuffers / podbuf | Time: proto3 / podbuf | CPU: FlatBuffers / podbuf | CPU: proto3 / podbuf |
| --------------------------------------- | -------------------------- | --------------------- | -------------------------- | --------------------- | ------------------------- | -------------------- |
| veh.cluster.VehicleStatus.currentSpeed  | 1.50                       | 0.31                  | 1.61                       | 1.48                  | 1.61                      | 1.48                 |
| veh.cluster.VehicleStatus.engineTemp    | 1.50                       | 0.31                  | 1.80                       | 1.61                  | 1.80                      | 1.61                 |
| veh.cluster.VehicleStatus.warnings      | 1.50                       | 0.12                  | 2.62                       | 2.32                  | 2.62                      | 2.32                 |
| veh.cluster.WheelHistory.wheelTicks     | 1.50                       | 0.12                  | 2.62                       | 2.28                  | 2.62                      | 2.28                 |
| veh.cluster.WheelHistory.wheelTemp      | 1.50                       | 0.31                  | 1.87                       | 1.67                  | 1.87                      | 1.67                 |
| veh.cluster.WheelHistory.wheelSlipRate  | 1.50                       | 0.31                  | 1.86                       | 1.55                  | 1.86                      | 1.55                 |
| veh.cluster.WheelSummary.wheelHealth    | 1.50                       | 0.12                  | 2.58                       | 2.28                  | 2.58                      | 2.28                 |
| veh.cluster.wheels.ticks                | 1.50                       | 0.12                  | 2.61                       | 2.28                  | 2.61                      | 2.28                 |
| veh.cluster.WheelDiagnostics.wheelTicks | not built                  | not built             | not built                  | not built             | not built                 | not built            |
| veh.hvac.cabin.cabinTemp                | 1.50                       | 0.31                  | 1.86                       | 1.68                  | 1.86                      | 1.68                 |
| veh.hvac.cabin.targetTemp               | 1.50                       | 0.31                  | 1.87                       | 1.67                  | 1.87                      | 1.67                 |
| veh.hvac.cabin.blowerDuty               | 1.50                       | 0.31                  | 1.85                       | 1.56                  | 1.85                      | 1.56                 |
| veh.hvac.cabin.fanLevel                 | 1.50                       | 0.12                  | 2.59                       | 2.29                  | 2.59                      | 2.29                 |
| veh.hvac.cabin.compressorLoad           | 1.50                       | 0.31                  | 1.86                       | 1.56                  | 1.86                      | 1.56                 |
| veh.cabin.Cabin.temperature             | 1.50                       | 0.12                  | 2.52                       | 2.30                  | 2.52                      | 2.30                 |
| veh.cabin.Horn.active                   | 2.00                       | 0.12                  | 2.49                       | 2.11                  | 2.49                      | 2.11                 |
| bench.stress.Stress.longString          | 1.03                       | 0.97                  | 0.90                       | 0.81                  | 0.90                      | 0.81                 |
| bench.stress.Stress.itemList            | 1.50                       | 0.55                  | 1.15                       | 1.48                  | 1.15                      | 1.48                 |
| bench.stress.Stress.nestedOptional      | 1.63                       | 0.35                  | 1.40                       | 1.37                  | 1.40                      | 1.37                 |
| bench.stress.Stress.map                 | 1.56                       | 0.53                  | 1.83                       | 3.04                  | 1.83                      | 3.04                 |

#### CPU load, paced and derived

CPU load in percent of one core at 4000 updates per second: paced (process CPU
time over wall time, includes the cost of sleep and wake-up) next to the derived
figure, and their difference.

| Payload                                | Encoding    | Paced % | Derived % | Difference |
| -------------------------------------- | ----------- | ------- | --------- | ---------- |
| veh.cluster.VehicleStatus.currentSpeed | FlatBuffers | 1.0405  | 0.0141    | 1.0264     |
| veh.cluster.VehicleStatus.currentSpeed | proto3      | 1.0098  | 0.0130    | 0.9968     |
| veh.cluster.VehicleStatus.currentSpeed | podbuf      | 1.3919  | 0.0088    | 1.3831     |
| veh.cluster.VehicleStatus.engineTemp   | FlatBuffers | 1.0695  | 0.0171    | 1.0524     |
| veh.cluster.VehicleStatus.engineTemp   | proto3      | 0.9204  | 0.0153    | 0.9051     |
| veh.cluster.VehicleStatus.engineTemp   | podbuf      | 0.8322  | 0.0095    | 0.8227     |
| veh.cluster.VehicleStatus.warnings     | FlatBuffers | 0.8448  | 0.0102    | 0.8346     |
| veh.cluster.VehicleStatus.warnings     | proto3      | 0.8133  | 0.0091    | 0.8043     |
| veh.cluster.VehicleStatus.warnings     | podbuf      | 0.8228  | 0.0039    | 0.8189     |
| veh.cluster.WheelHistory.wheelTicks    | FlatBuffers | 0.7481  | 0.0101    | 0.7380     |
| veh.cluster.WheelHistory.wheelTicks    | proto3      | 0.8921  | 0.0089    | 0.8833     |
| veh.cluster.WheelHistory.wheelTicks    | podbuf      | 0.7408  | 0.0039    | 0.7369     |
| veh.cluster.WheelHistory.wheelTemp     | FlatBuffers | 0.8066  | 0.0167    | 0.7900     |
| veh.cluster.WheelHistory.wheelTemp     | proto3      | 0.8560  | 0.0149    | 0.8411     |
| veh.cluster.WheelHistory.wheelTemp     | podbuf      | 1.4569  | 0.0089    | 1.4480     |
| veh.cluster.WheelHistory.wheelSlipRate | FlatBuffers | 0.8381  | 0.0167    | 0.8215     |
| veh.cluster.WheelHistory.wheelSlipRate | proto3      | 0.6154  | 0.0139    | 0.6015     |
| veh.cluster.WheelHistory.wheelSlipRate | podbuf      | 0.7400  | 0.0090    | 0.7311     |
| veh.cluster.WheelSummary.wheelHealth   | FlatBuffers | 1.1875  | 0.0100    | 1.1775     |
| veh.cluster.WheelSummary.wheelHealth   | proto3      | 1.4631  | 0.0088    | 1.4543     |
| veh.cluster.WheelSummary.wheelHealth   | podbuf      | 1.0538  | 0.0039    | 1.0499     |
| veh.cluster.wheels.ticks               | FlatBuffers | 1.0046  | 0.0101    | 0.9945     |
| veh.cluster.wheels.ticks               | proto3      | 1.0873  | 0.0088    | 1.0785     |
| veh.cluster.wheels.ticks               | podbuf      | 1.6294  | 0.0039    | 1.6255     |
| veh.hvac.cabin.cabinTemp               | FlatBuffers | 0.8451  | 0.0165    | 0.8286     |
| veh.hvac.cabin.cabinTemp               | proto3      | 1.1397  | 0.0149    | 1.1248     |
| veh.hvac.cabin.cabinTemp               | podbuf      | 1.9324  | 0.0089    | 1.9235     |
| veh.hvac.cabin.targetTemp              | FlatBuffers | 0.9348  | 0.0167    | 0.9181     |
| veh.hvac.cabin.targetTemp              | proto3      | 2.8729  | 0.0149    | 2.8580     |
| veh.hvac.cabin.targetTemp              | podbuf      | 3.7780  | 0.0089    | 3.7691     |
| veh.hvac.cabin.blowerDuty              | FlatBuffers | 0.7274  | 0.0166    | 0.7109     |
| veh.hvac.cabin.blowerDuty              | proto3      | 0.8834  | 0.0139    | 0.8695     |
| veh.hvac.cabin.blowerDuty              | podbuf      | 1.6802  | 0.0090    | 1.6712     |
| veh.hvac.cabin.fanLevel                | FlatBuffers | 0.8600  | 0.0100    | 0.8501     |
| veh.hvac.cabin.fanLevel                | proto3      | 1.3967  | 0.0088    | 1.3879     |
| veh.hvac.cabin.fanLevel                | podbuf      | 1.8507  | 0.0039    | 1.8468     |
| veh.hvac.cabin.compressorLoad          | FlatBuffers | 0.7659  | 0.0166    | 0.7493     |
| veh.hvac.cabin.compressorLoad          | proto3      | 0.7387  | 0.0139    | 0.7248     |
| veh.hvac.cabin.compressorLoad          | podbuf      | 1.3793  | 0.0089    | 1.3704     |
| veh.cabin.Cabin.temperature            | FlatBuffers | 1.3589  | 0.0100    | 1.3490     |
| veh.cabin.Cabin.temperature            | proto3      | 0.9291  | 0.0091    | 0.9199     |
| veh.cabin.Cabin.temperature            | podbuf      | 0.8309  | 0.0040    | 0.8270     |
| veh.cabin.Horn.active                  | FlatBuffers | 2.2207  | 0.0104    | 2.2103     |
| veh.cabin.Horn.active                  | proto3      | 1.0965  | 0.0088    | 1.0876     |
| veh.cabin.Horn.active                  | podbuf      | 0.8066  | 0.0042    | 0.8024     |
| bench.stress.Stress.longString         | FlatBuffers | 2.5878  | 0.0660    | 2.5218     |
| bench.stress.Stress.longString         | proto3      | 4.0451  | 0.0591    | 3.9861     |
| bench.stress.Stress.longString         | podbuf      | 3.9891  | 0.0730    | 3.9161     |
| bench.stress.Stress.itemList           | FlatBuffers | 3.8120  | 0.6760    | 3.1360     |
| bench.stress.Stress.itemList           | proto3      | 8.6413  | 0.8670    | 7.7743     |
| bench.stress.Stress.itemList           | podbuf      | 2.4541  | 0.5878    | 1.8663     |
| bench.stress.Stress.nestedOptional     | FlatBuffers | 2.4756  | 0.0808    | 2.3948     |
| bench.stress.Stress.nestedOptional     | proto3      | 4.3709  | 0.0791    | 4.2918     |
| bench.stress.Stress.nestedOptional     | podbuf      | 4.3562  | 0.0575    | 4.2986     |
| bench.stress.Stress.map                | FlatBuffers | 6.4847  | 1.2623    | 5.2224     |
| bench.stress.Stress.map                | proto3      | 12.3541 | 2.0982    | 10.2559    |
| bench.stress.Stress.map                | podbuf      | 4.2652  | 0.6913    | 3.5739     |

### Payloads not built

The bench could not build a value of these payloads, so no encoding was tried on
them. This is not an encoding refusal.

| Payload                                 | Payload type | Reason                                                                                                           |
| --------------------------------------- | ------------ | ---------------------------------------------------------------------------------------------------------------- |
| veh.cluster.WheelDiagnostics.wheelTicks | RawTickCount | the payload type RawTickCount is internal to its package, so a value cannot be built outside the generated crate |

### Refused cells

None.

## Pixel 5

**Measured on a Pixel 5 (aarch64, Linux, run over `adb`), pinned with `taskset`
to one core: cpu2, a Cortex-A55 core, and, in a second run, cpu6, a Cortex-A76
core.** Date 2026-11-05, commit 69fdf811, rustc 1.98.1 (48a229cea 2026-09-01), 4
000 updates per second, 30 samples per cell, 2 s of CPU time per cell and
encoding, paced run included. 57 cells were measured and 3 were not built (the
one payload `RawTickCount`, for each encoding), in each run. No cell was
refused. The commands were `just bench-pixel`, then `cargo xtask bench report`.

### What the device data show, and what they do not

- **Frequency.** The phone is not rooted, the governor is `schedutil` and the
  frequency is not fixed. The sidecar files read the frequency of each core
  before and after a run; the two reads do not show the frequency during the
  run. The A55 run read cpu2 at 1 363 200 kHz before and after. The A76 run read
  cpu6 at 652 800 kHz before and 1 900 800 kHz after. The result files record
  the frequency of the core at the start of the program: 1 804 800 kHz in the
  A55 run and 1 728 000 kHz in the A76 run. These start-of-program values differ
  from the sidecar reads of the pinned core, and this note asserts no cause. The
  frequency during the runs is therefore not known; it may have varied.
- **Stability.** No cell carries the spread marker. The largest
  `(max - min) / median` of any cell is 2.0 % in the A55 run and 3.6 % in the
  A76 run, and every cell has 30 samples. The tables therefore show no
  instability in the samples. They do not show that the frequency was constant,
  because the samples of one cell are taken over a short time.
- **Load.** The codec-only load at 4 000 updates per second is a small fraction
  of one core for the signal payloads: 0.044 % to 0.183 % on the A55 core and
  0.015 % to 0.061 % on the A76 core. The largest load of any cell is the proto3
  encoding of the map stress payload: 16.02 % of the A55 core (40.1 us per
  update) and 4.90 % of the A76 core (12.3 us per update). The largest load
  among the signal payloads is FlatBuffers on `veh.hvac.cabin.blowerDuty` on the
  A55 core (0.183 %, 458.1 ns per update) and FlatBuffers on
  `veh.cluster.VehicleStatus.engineTemp` on the A76 core (0.061 %, 152.5 ns per
  update).
- **Paced against derived.** The paced figure is above the derived figure in
  every cell of both runs. For the signal payloads the paced figure is 6.6 % to
  11.2 % on the A55 core and 2.0 % to 4.2 % on the A76 core, where the derived
  figure is 0.044 % to 0.183 % and 0.015 % to 0.061 %. The paced runs measure
  process CPU time over a run that sleeps between updates; the tables do not
  show what part of that time is the codec, and this note does not assert a
  cause. The derived figure is the one that the codec determines.
- **Comparison of the encodings.** On the 15 measured signal payloads podbuf has
  the lowest median time per update on both cores, and the time ratios to podbuf
  of those payloads are 1.34 or more for FlatBuffers and 1.13 or more for proto3
  on the A55 core (1.41 and 1.12 on the A76 core). The same holds on three of
  the four stress payloads (list of structs, nested optionals, map). It does not
  hold on the long string, where podbuf has the highest time on both cores.
  proto3 is the smallest in size on every signal payload. There is no ordering
  of the encodings that holds on every payload, so this note gives no verdict
  between them. Which one to use depends on the weight of size against CPU time
  and on the payloads of the system, which the bench does not decide.
- **The burst is not covered.** A burst of 200 messages in 10 ms was not run.
  The time per update in the tables lets a reader compute a burst of
  back-to-back updates. Worked example, derived and not measured: the heaviest
  signal payload is FlatBuffers on `veh.hvac.cabin.blowerDuty`, 458.1 ns per
  update on the A55 core; 200 updates cost 200 x 458.1 ns = 91.6 us of CPU time,
  which is 0.92 % of the 10 ms window. This assumes that every update costs what
  the warm, repeated update of the bench costs; it does not include the
  transport, queueing, cold caches or a lower core frequency.

### Cortex-A55 (cpu2)

#### Mix

Equal-weight mix, per signal: each signal counts once, so a payload type that
several signals share counts once for each of them. Only signals whose three
encodings are measured are counted; the last column gives how many.

| Metric                                                 | FlatBuffers | proto3 | podbuf | FlatBuffers / podbuf | proto3 / podbuf | Units |
| ------------------------------------------------------ | ----------- | ------ | ------ | -------------------- | --------------- | ----- |
| Size (bytes)                                           | 24.5        | 3.6    | 16.0   | 1.53                 | 0.23            | 15    |
| Heap allocations per update (encode + verify + decode) | 0.0         | 0.0    | 0.0    | n/a                  | n/a             | 15    |
| Time per update (median ns)                            | 366.5       | 314.6  | 233.0  | 1.57                 | 1.35            | 15    |
| CPU % of one core at 4000 updates per second (derived) | 0.1466      | 0.1258 | 0.0932 | 1.57                 | 1.35            | 15    |

Equal-weight mix, per payload type: each payload type counts once, however many
signals share it (the value of a type is the mean over its signals). Only
payload types whose signals are measured in all three encodings are counted; the
last column gives how many.

| Metric                                                 | FlatBuffers | proto3 | podbuf | FlatBuffers / podbuf | proto3 / podbuf | Units |
| ------------------------------------------------------ | ----------- | ------ | ------ | -------------------- | --------------- | ----- |
| Size (bytes)                                           | 25.3        | 3.4    | 16.0   | 1.58                 | 0.21            | 6     |
| Heap allocations per update (encode + verify + decode) | 0.0         | 0.0    | 0.0    | n/a                  | n/a             | 6     |
| Time per update (median ns)                            | 354.9       | 302.7  | 218.1  | 1.63                 | 1.39            | 6     |
| CPU % of one core at 4000 updates per second (derived) | 0.1420      | 0.1211 | 0.0872 | 1.63                 | 1.39            | 6     |

#### Per payload

##### Encoded size

Encoded size in bytes of one payload value.

| Payload                                 | FlatBuffers | proto3    | podbuf    |
| --------------------------------------- | ----------- | --------- | --------- |
| veh.cluster.VehicleStatus.currentSpeed  | 24          | 5         | 16        |
| veh.cluster.VehicleStatus.engineTemp    | 24          | 5         | 16        |
| veh.cluster.VehicleStatus.warnings      | 24          | 2         | 16        |
| veh.cluster.WheelHistory.wheelTicks     | 24          | 2         | 16        |
| veh.cluster.WheelHistory.wheelTemp      | 24          | 5         | 16        |
| veh.cluster.WheelHistory.wheelSlipRate  | 24          | 5         | 16        |
| veh.cluster.WheelSummary.wheelHealth    | 24          | 2         | 16        |
| veh.cluster.wheels.ticks                | 24          | 2         | 16        |
| veh.cluster.WheelDiagnostics.wheelTicks | not built   | not built | not built |
| veh.hvac.cabin.cabinTemp                | 24          | 5         | 16        |
| veh.hvac.cabin.targetTemp               | 24          | 5         | 16        |
| veh.hvac.cabin.blowerDuty               | 24          | 5         | 16        |
| veh.hvac.cabin.fanLevel                 | 24          | 2         | 16        |
| veh.hvac.cabin.compressorLoad           | 24          | 5         | 16        |
| veh.cabin.Cabin.temperature             | 24          | 2         | 16        |
| veh.cabin.Horn.active                   | 32          | 2         | 16        |
| bench.stress.Stress.longString          | 544         | 515       | 529       |
| bench.stress.Stress.itemList            | 600         | 221       | 400       |
| bench.stress.Stress.nestedOptional      | 80          | 17        | 49        |
| bench.stress.Stress.map                 | 600         | 204       | 384       |

##### Heap allocations

Heap allocations of one encode / verify / decode.

| Payload                                 | FlatBuffers | proto3     | podbuf     |
| --------------------------------------- | ----------- | ---------- | ---------- |
| veh.cluster.VehicleStatus.currentSpeed  | 0 / 0 / 0   | 0 / 0 / 0  | 0 / 0 / 0  |
| veh.cluster.VehicleStatus.engineTemp    | 0 / 0 / 0   | 0 / 0 / 0  | 0 / 0 / 0  |
| veh.cluster.VehicleStatus.warnings      | 0 / 0 / 0   | 0 / 0 / 0  | 0 / 0 / 0  |
| veh.cluster.WheelHistory.wheelTicks     | 0 / 0 / 0   | 0 / 0 / 0  | 0 / 0 / 0  |
| veh.cluster.WheelHistory.wheelTemp      | 0 / 0 / 0   | 0 / 0 / 0  | 0 / 0 / 0  |
| veh.cluster.WheelHistory.wheelSlipRate  | 0 / 0 / 0   | 0 / 0 / 0  | 0 / 0 / 0  |
| veh.cluster.WheelSummary.wheelHealth    | 0 / 0 / 0   | 0 / 0 / 0  | 0 / 0 / 0  |
| veh.cluster.wheels.ticks                | 0 / 0 / 0   | 0 / 0 / 0  | 0 / 0 / 0  |
| veh.cluster.WheelDiagnostics.wheelTicks | not built   | not built  | not built  |
| veh.hvac.cabin.cabinTemp                | 0 / 0 / 0   | 0 / 0 / 0  | 0 / 0 / 0  |
| veh.hvac.cabin.targetTemp               | 0 / 0 / 0   | 0 / 0 / 0  | 0 / 0 / 0  |
| veh.hvac.cabin.blowerDuty               | 0 / 0 / 0   | 0 / 0 / 0  | 0 / 0 / 0  |
| veh.hvac.cabin.fanLevel                 | 0 / 0 / 0   | 0 / 0 / 0  | 0 / 0 / 0  |
| veh.hvac.cabin.compressorLoad           | 0 / 0 / 0   | 0 / 0 / 0  | 0 / 0 / 0  |
| veh.cabin.Cabin.temperature             | 0 / 0 / 0   | 0 / 0 / 0  | 0 / 0 / 0  |
| veh.cabin.Horn.active                   | 0 / 0 / 0   | 0 / 0 / 0  | 0 / 0 / 0  |
| bench.stress.Stress.longString          | 0 / 0 / 1   | 0 / 0 / 1  | 0 / 0 / 1  |
| bench.stress.Stress.itemList            | 1 / 0 / 17  | 0 / 0 / 19 | 0 / 0 / 17 |
| bench.stress.Stress.nestedOptional      | 0 / 0 / 2   | 0 / 0 / 2  | 0 / 0 / 2  |
| bench.stress.Stress.map                 | 1 / 0 / 17  | 0 / 0 / 19 | 1 / 0 / 17 |

##### Time per update

CPU time of one update (encode, verify and decode), median in ns, with the
fastest and slowest sample and the range as a share of the median. † spread
above 10 %: the figure is not stable.

| Payload                                 | FlatBuffers                       | proto3                            | podbuf                            |
| --------------------------------------- | --------------------------------- | --------------------------------- | --------------------------------- |
| veh.cluster.VehicleStatus.currentSpeed  | 446.8 (446.7 to 447.5; 0 %)       | 378.8 (378.8 to 379.1; 0 %)       | 333.3 (333.3 to 333.9; 0 %)       |
| veh.cluster.VehicleStatus.engineTemp    | 454.8 (454.7 to 457.0; 1 %)       | 387.6 (387.5 to 387.9; 0 %)       | 340.0 (339.9 to 340.3; 0 %)       |
| veh.cluster.VehicleStatus.warnings      | 261.5 (260.8 to 262.9; 1 %)       | 233.7 (233.4 to 233.9; 0 %)       | 111.0 (110.9 to 111.8; 1 %)       |
| veh.cluster.WheelHistory.wheelTicks     | 262.6 (261.8 to 263.4; 1 %)       | 231.8 (231.5 to 232.2; 0 %)       | 109.2 (109.0 to 109.8; 1 %)       |
| veh.cluster.WheelHistory.wheelTemp      | 454.8 (454.6 to 455.2; 0 %)       | 387.6 (387.5 to 387.8; 0 %)       | 340.0 (339.9 to 340.3; 0 %)       |
| veh.cluster.WheelHistory.wheelSlipRate  | 458.1 (457.9 to 460.4; 1 %)       | 387.7 (387.5 to 387.9; 0 %)       | 341.6 (341.6 to 342.8; 0 %)       |
| veh.cluster.WheelSummary.wheelHealth    | 262.5 (261.7 to 263.3; 1 %)       | 231.7 (231.6 to 232.1; 0 %)       | 109.1 (108.9 to 109.4; 0 %)       |
| veh.cluster.wheels.ticks                | 262.4 (261.7 to 263.2; 1 %)       | 231.8 (231.5 to 232.0; 0 %)       | 109.1 (109.0 to 109.4; 0 %)       |
| veh.cluster.WheelDiagnostics.wheelTicks | not built                         | not built                         | not built                         |
| veh.hvac.cabin.cabinTemp                | 454.8 (454.6 to 455.3; 0 %)       | 387.6 (387.5 to 387.7; 0 %)       | 340.0 (339.9 to 340.4; 0 %)       |
| veh.hvac.cabin.targetTemp               | 454.8 (454.6 to 455.3; 0 %)       | 387.7 (387.6 to 388.3; 0 %)       | 340.0 (339.9 to 340.3; 0 %)       |
| veh.hvac.cabin.blowerDuty               | 458.1 (457.8 to 458.6; 0 %)       | 387.7 (387.6 to 388.6; 0 %)       | 341.7 (341.6 to 342.1; 0 %)       |
| veh.hvac.cabin.fanLevel                 | 262.8 (261.8 to 264.1; 1 %)       | 231.8 (231.6 to 232.1; 0 %)       | 109.1 (108.9 to 109.5; 1 %)       |
| veh.hvac.cabin.compressorLoad           | 458.1 (457.8 to 458.8; 0 %)       | 387.7 (387.4 to 388.0; 0 %)       | 341.7 (341.6 to 342.2; 0 %)       |
| veh.cabin.Cabin.temperature             | 260.8 (259.6 to 262.2; 1 %)       | 239.1 (239.0 to 239.2; 0 %)       | 110.2 (110.1 to 110.5; 0 %)       |
| veh.cabin.Horn.active                   | 284.5 (282.2 to 288.0; 2 %)       | 226.1 (225.9 to 226.2; 0 %)       | 119.2 (119.1 to 119.4; 0 %)       |
| bench.stress.Stress.longString          | 2104.2 (2100.6 to 2109.5; 0 %)    | 1252.8 (1251.7 to 1254.9; 0 %)    | 2211.4 (2210.4 to 2212.6; 0 %)    |
| bench.stress.Stress.itemList            | 10329.7 (10325.0 to 10337.8; 0 %) | 14079.0 (14068.1 to 14098.5; 0 %) | 9064.4 (9062.2 to 9072.0; 0 %)    |
| bench.stress.Stress.nestedOptional      | 1488.9 (1483.0 to 1494.8; 1 %)    | 1624.3 (1621.0 to 1627.7; 0 %)    | 1162.3 (1160.8 to 1164.5; 0 %)    |
| bench.stress.Stress.map                 | 21513.0 (21502.4 to 21610.3; 1 %) | 40056.7 (40044.1 to 40079.8; 0 %) | 11386.7 (11379.1 to 11391.6; 0 %) |

##### CPU load, derived

CPU load in percent of one core at 4000 updates per second, derived from the
median time of one update (the headline figure).

| Payload                                 | FlatBuffers | proto3    | podbuf    |
| --------------------------------------- | ----------- | --------- | --------- |
| veh.cluster.VehicleStatus.currentSpeed  | 0.1787      | 0.1515    | 0.1333    |
| veh.cluster.VehicleStatus.engineTemp    | 0.1819      | 0.1551    | 0.1360    |
| veh.cluster.VehicleStatus.warnings      | 0.1046      | 0.0935    | 0.0444    |
| veh.cluster.WheelHistory.wheelTicks     | 0.1051      | 0.0927    | 0.0437    |
| veh.cluster.WheelHistory.wheelTemp      | 0.1819      | 0.1551    | 0.1360    |
| veh.cluster.WheelHistory.wheelSlipRate  | 0.1832      | 0.1551    | 0.1367    |
| veh.cluster.WheelSummary.wheelHealth    | 0.1050      | 0.0927    | 0.0436    |
| veh.cluster.wheels.ticks                | 0.1049      | 0.0927    | 0.0436    |
| veh.cluster.WheelDiagnostics.wheelTicks | not built   | not built | not built |
| veh.hvac.cabin.cabinTemp                | 0.1819      | 0.1551    | 0.1360    |
| veh.hvac.cabin.targetTemp               | 0.1819      | 0.1551    | 0.1360    |
| veh.hvac.cabin.blowerDuty               | 0.1833      | 0.1551    | 0.1367    |
| veh.hvac.cabin.fanLevel                 | 0.1051      | 0.0927    | 0.0436    |
| veh.hvac.cabin.compressorLoad           | 0.1832      | 0.1551    | 0.1367    |
| veh.cabin.Cabin.temperature             | 0.1043      | 0.0956    | 0.0441    |
| veh.cabin.Horn.active                   | 0.1138      | 0.0904    | 0.0477    |
| bench.stress.Stress.longString          | 0.8417      | 0.5011    | 0.8845    |
| bench.stress.Stress.itemList            | 4.1319      | 5.6316    | 3.6258    |
| bench.stress.Stress.nestedOptional      | 0.5956      | 0.6497    | 0.4649    |
| bench.stress.Stress.map                 | 8.6052      | 16.0227   | 4.5547    |

##### Ratios to podbuf

Each encoding divided by podbuf: above 1.00 the encoding is larger or slower
than podbuf, below 1.00 smaller or faster. The CPU ratios equal the time ratios,
because the derived CPU load is proportional to the median time. A refused or
not-built cell has no ratio and is marked.

| Payload                                 | Size: FlatBuffers / podbuf | Size: proto3 / podbuf | Time: FlatBuffers / podbuf | Time: proto3 / podbuf | CPU: FlatBuffers / podbuf | CPU: proto3 / podbuf |
| --------------------------------------- | -------------------------- | --------------------- | -------------------------- | --------------------- | ------------------------- | -------------------- |
| veh.cluster.VehicleStatus.currentSpeed  | 1.50                       | 0.31                  | 1.34                       | 1.14                  | 1.34                      | 1.14                 |
| veh.cluster.VehicleStatus.engineTemp    | 1.50                       | 0.31                  | 1.34                       | 1.14                  | 1.34                      | 1.14                 |
| veh.cluster.VehicleStatus.warnings      | 1.50                       | 0.12                  | 2.36                       | 2.10                  | 2.36                      | 2.10                 |
| veh.cluster.WheelHistory.wheelTicks     | 1.50                       | 0.12                  | 2.41                       | 2.12                  | 2.41                      | 2.12                 |
| veh.cluster.WheelHistory.wheelTemp      | 1.50                       | 0.31                  | 1.34                       | 1.14                  | 1.34                      | 1.14                 |
| veh.cluster.WheelHistory.wheelSlipRate  | 1.50                       | 0.31                  | 1.34                       | 1.13                  | 1.34                      | 1.13                 |
| veh.cluster.WheelSummary.wheelHealth    | 1.50                       | 0.12                  | 2.41                       | 2.12                  | 2.41                      | 2.12                 |
| veh.cluster.wheels.ticks                | 1.50                       | 0.12                  | 2.40                       | 2.12                  | 2.40                      | 2.12                 |
| veh.cluster.WheelDiagnostics.wheelTicks | not built                  | not built             | not built                  | not built             | not built                 | not built            |
| veh.hvac.cabin.cabinTemp                | 1.50                       | 0.31                  | 1.34                       | 1.14                  | 1.34                      | 1.14                 |
| veh.hvac.cabin.targetTemp               | 1.50                       | 0.31                  | 1.34                       | 1.14                  | 1.34                      | 1.14                 |
| veh.hvac.cabin.blowerDuty               | 1.50                       | 0.31                  | 1.34                       | 1.13                  | 1.34                      | 1.13                 |
| veh.hvac.cabin.fanLevel                 | 1.50                       | 0.12                  | 2.41                       | 2.13                  | 2.41                      | 2.13                 |
| veh.hvac.cabin.compressorLoad           | 1.50                       | 0.31                  | 1.34                       | 1.13                  | 1.34                      | 1.13                 |
| veh.cabin.Cabin.temperature             | 1.50                       | 0.12                  | 2.37                       | 2.17                  | 2.37                      | 2.17                 |
| veh.cabin.Horn.active                   | 2.00                       | 0.12                  | 2.39                       | 1.90                  | 2.39                      | 1.90                 |
| bench.stress.Stress.longString          | 1.03                       | 0.97                  | 0.95                       | 0.57                  | 0.95                      | 0.57                 |
| bench.stress.Stress.itemList            | 1.50                       | 0.55                  | 1.14                       | 1.55                  | 1.14                      | 1.55                 |
| bench.stress.Stress.nestedOptional      | 1.63                       | 0.35                  | 1.28                       | 1.40                  | 1.28                      | 1.40                 |
| bench.stress.Stress.map                 | 1.56                       | 0.53                  | 1.89                       | 3.52                  | 1.89                      | 3.52                 |

##### CPU load, paced and derived

CPU load in percent of one core at 4000 updates per second: paced (process CPU
time over wall time, includes the cost of sleep and wake-up) next to the derived
figure, and their difference.

| Payload                                | Encoding    | Paced % | Derived % | Difference |
| -------------------------------------- | ----------- | ------- | --------- | ---------- |
| veh.cluster.VehicleStatus.currentSpeed | FlatBuffers | 9.5565  | 0.1787    | 9.3778     |
| veh.cluster.VehicleStatus.currentSpeed | proto3      | 9.9855  | 0.1515    | 9.8340     |
| veh.cluster.VehicleStatus.currentSpeed | podbuf      | 9.4294  | 0.1333    | 9.2961     |
| veh.cluster.VehicleStatus.engineTemp   | FlatBuffers | 10.0295 | 0.1819    | 9.8475     |
| veh.cluster.VehicleStatus.engineTemp   | proto3      | 9.4651  | 0.1551    | 9.3100     |
| veh.cluster.VehicleStatus.engineTemp   | podbuf      | 9.5322  | 0.1360    | 9.3963     |
| veh.cluster.VehicleStatus.warnings     | FlatBuffers | 10.3679 | 0.1046    | 10.2633    |
| veh.cluster.VehicleStatus.warnings     | proto3      | 9.5036  | 0.0935    | 9.4101     |
| veh.cluster.VehicleStatus.warnings     | podbuf      | 8.8492  | 0.0444    | 8.8048     |
| veh.cluster.WheelHistory.wheelTicks    | FlatBuffers | 9.5451  | 0.1051    | 9.4401     |
| veh.cluster.WheelHistory.wheelTicks    | proto3      | 9.1309  | 0.0927    | 9.0382     |
| veh.cluster.WheelHistory.wheelTicks    | podbuf      | 9.0331  | 0.0437    | 8.9894     |
| veh.cluster.WheelHistory.wheelTemp     | FlatBuffers | 10.8584 | 0.1819    | 10.6765    |
| veh.cluster.WheelHistory.wheelTemp     | proto3      | 10.1914 | 0.1551    | 10.0363    |
| veh.cluster.WheelHistory.wheelTemp     | podbuf      | 9.6988  | 0.1360    | 9.5628     |
| veh.cluster.WheelHistory.wheelSlipRate | FlatBuffers | 10.9278 | 0.1832    | 10.7445    |
| veh.cluster.WheelHistory.wheelSlipRate | proto3      | 10.5035 | 0.1551    | 10.3484    |
| veh.cluster.WheelHistory.wheelSlipRate | podbuf      | 9.6915  | 0.1367    | 9.5548     |
| veh.cluster.WheelSummary.wheelHealth   | FlatBuffers | 9.5976  | 0.1050    | 9.4926     |
| veh.cluster.WheelSummary.wheelHealth   | proto3      | 9.4815  | 0.0927    | 9.3888     |
| veh.cluster.WheelSummary.wheelHealth   | podbuf      | 9.2932  | 0.0436    | 9.2496     |
| veh.cluster.wheels.ticks               | FlatBuffers | 9.9544  | 0.1049    | 9.8495     |
| veh.cluster.wheels.ticks               | proto3      | 9.5159  | 0.0927    | 9.4232     |
| veh.cluster.wheels.ticks               | podbuf      | 8.7471  | 0.0436    | 8.7034     |
| veh.hvac.cabin.cabinTemp               | FlatBuffers | 6.5849  | 0.1819    | 6.4030     |
| veh.hvac.cabin.cabinTemp               | proto3      | 9.7867  | 0.1551    | 9.6316     |
| veh.hvac.cabin.cabinTemp               | podbuf      | 9.9173  | 0.1360    | 9.7813     |
| veh.hvac.cabin.targetTemp              | FlatBuffers | 10.7671 | 0.1819    | 10.5852    |
| veh.hvac.cabin.targetTemp              | proto3      | 10.7820 | 0.1551    | 10.6269    |
| veh.hvac.cabin.targetTemp              | podbuf      | 10.8128 | 0.1360    | 10.6768    |
| veh.hvac.cabin.blowerDuty              | FlatBuffers | 10.2632 | 0.1833    | 10.0799    |
| veh.hvac.cabin.blowerDuty              | proto3      | 9.8182  | 0.1551    | 9.6631     |
| veh.hvac.cabin.blowerDuty              | podbuf      | 9.6678  | 0.1367    | 9.5311     |
| veh.hvac.cabin.fanLevel                | FlatBuffers | 9.9915  | 0.1051    | 9.8864     |
| veh.hvac.cabin.fanLevel                | proto3      | 9.9058  | 0.0927    | 9.8131     |
| veh.hvac.cabin.fanLevel                | podbuf      | 9.1024  | 0.0436    | 9.0587     |
| veh.hvac.cabin.compressorLoad          | FlatBuffers | 9.9222  | 0.1832    | 9.7389     |
| veh.hvac.cabin.compressorLoad          | proto3      | 10.0290 | 0.1551    | 9.8739     |
| veh.hvac.cabin.compressorLoad          | podbuf      | 11.1676 | 0.1367    | 11.0310    |
| veh.cabin.Cabin.temperature            | FlatBuffers | 10.6216 | 0.1043    | 10.5173    |
| veh.cabin.Cabin.temperature            | proto3      | 9.5306  | 0.0956    | 9.4349     |
| veh.cabin.Cabin.temperature            | podbuf      | 10.0674 | 0.0441    | 10.0233    |
| veh.cabin.Horn.active                  | FlatBuffers | 9.7163  | 0.1138    | 9.6024     |
| veh.cabin.Horn.active                  | proto3      | 9.2755  | 0.0904    | 9.1850     |
| veh.cabin.Horn.active                  | podbuf      | 9.8233  | 0.0477    | 9.7756     |
| bench.stress.Stress.longString         | FlatBuffers | 16.6024 | 0.8417    | 15.7607    |
| bench.stress.Stress.longString         | proto3      | 15.2316 | 0.5011    | 14.7304    |
| bench.stress.Stress.longString         | podbuf      | 17.2357 | 0.8845    | 16.3511    |
| bench.stress.Stress.itemList           | FlatBuffers | 26.9315 | 4.1319    | 22.7996    |
| bench.stress.Stress.itemList           | proto3      | 30.2254 | 5.6316    | 24.5938    |
| bench.stress.Stress.itemList           | podbuf      | 24.8115 | 3.6258    | 21.1858    |
| bench.stress.Stress.nestedOptional     | FlatBuffers | 13.9099 | 0.5956    | 13.3143    |
| bench.stress.Stress.nestedOptional     | proto3      | 14.1788 | 0.6497    | 13.5291    |
| bench.stress.Stress.nestedOptional     | podbuf      | 12.7295 | 0.4649    | 12.2646    |
| bench.stress.Stress.map                | FlatBuffers | 39.1053 | 8.6052    | 30.5001    |
| bench.stress.Stress.map                | proto3      | 61.5943 | 16.0227   | 45.5716    |
| bench.stress.Stress.map                | podbuf      | 28.0513 | 4.5547    | 23.4967    |

#### Payloads not built

The bench could not build a value of these payloads, so no encoding was tried on
them. This is not an encoding refusal.

| Payload                                 | Payload type | Reason                                                                                                           |
| --------------------------------------- | ------------ | ---------------------------------------------------------------------------------------------------------------- |
| veh.cluster.WheelDiagnostics.wheelTicks | RawTickCount | the payload type RawTickCount is internal to its package, so a value cannot be built outside the generated crate |

#### Refused cells

None.

### Cortex-A76 (cpu6)

#### Mix

Equal-weight mix, per signal: each signal counts once, so a payload type that
several signals share counts once for each of them. Only signals whose three
encodings are measured are counted; the last column gives how many.

| Metric                                                 | FlatBuffers | proto3 | podbuf | FlatBuffers / podbuf | proto3 / podbuf | Units |
| ------------------------------------------------------ | ----------- | ------ | ------ | -------------------- | --------------- | ----- |
| Size (bytes)                                           | 24.5        | 3.6    | 16.0   | 1.53                 | 0.23            | 15    |
| Heap allocations per update (encode + verify + decode) | 0.0         | 0.0    | 0.0    | n/a                  | n/a             | 15    |
| Time per update (median ns)                            | 127.2       | 99.3   | 74.4   | 1.71                 | 1.33            | 15    |
| CPU % of one core at 4000 updates per second (derived) | 0.0509      | 0.0397 | 0.0297 | 1.71                 | 1.33            | 15    |

Equal-weight mix, per payload type: each payload type counts once, however many
signals share it (the value of a type is the mean over its signals). Only
payload types whose signals are measured in all three encodings are counted; the
last column gives how many.

| Metric                                                 | FlatBuffers | proto3 | podbuf | FlatBuffers / podbuf | proto3 / podbuf | Units |
| ------------------------------------------------------ | ----------- | ------ | ------ | -------------------- | --------------- | ----- |
| Size (bytes)                                           | 25.3        | 3.4    | 16.0   | 1.58                 | 0.21            | 6     |
| Heap allocations per update (encode + verify + decode) | 0.0         | 0.0    | 0.0    | n/a                  | n/a             | 6     |
| Time per update (median ns)                            | 123.8       | 95.7   | 69.7   | 1.78                 | 1.37            | 6     |
| CPU % of one core at 4000 updates per second (derived) | 0.0495      | 0.0383 | 0.0279 | 1.78                 | 1.37            | 6     |

#### Per payload

##### Encoded size

Encoded size in bytes of one payload value.

| Payload                                 | FlatBuffers | proto3    | podbuf    |
| --------------------------------------- | ----------- | --------- | --------- |
| veh.cluster.VehicleStatus.currentSpeed  | 24          | 5         | 16        |
| veh.cluster.VehicleStatus.engineTemp    | 24          | 5         | 16        |
| veh.cluster.VehicleStatus.warnings      | 24          | 2         | 16        |
| veh.cluster.WheelHistory.wheelTicks     | 24          | 2         | 16        |
| veh.cluster.WheelHistory.wheelTemp      | 24          | 5         | 16        |
| veh.cluster.WheelHistory.wheelSlipRate  | 24          | 5         | 16        |
| veh.cluster.WheelSummary.wheelHealth    | 24          | 2         | 16        |
| veh.cluster.wheels.ticks                | 24          | 2         | 16        |
| veh.cluster.WheelDiagnostics.wheelTicks | not built   | not built | not built |
| veh.hvac.cabin.cabinTemp                | 24          | 5         | 16        |
| veh.hvac.cabin.targetTemp               | 24          | 5         | 16        |
| veh.hvac.cabin.blowerDuty               | 24          | 5         | 16        |
| veh.hvac.cabin.fanLevel                 | 24          | 2         | 16        |
| veh.hvac.cabin.compressorLoad           | 24          | 5         | 16        |
| veh.cabin.Cabin.temperature             | 24          | 2         | 16        |
| veh.cabin.Horn.active                   | 32          | 2         | 16        |
| bench.stress.Stress.longString          | 544         | 515       | 529       |
| bench.stress.Stress.itemList            | 600         | 221       | 400       |
| bench.stress.Stress.nestedOptional      | 80          | 17        | 49        |
| bench.stress.Stress.map                 | 600         | 204       | 384       |

##### Heap allocations

Heap allocations of one encode / verify / decode.

| Payload                                 | FlatBuffers | proto3     | podbuf     |
| --------------------------------------- | ----------- | ---------- | ---------- |
| veh.cluster.VehicleStatus.currentSpeed  | 0 / 0 / 0   | 0 / 0 / 0  | 0 / 0 / 0  |
| veh.cluster.VehicleStatus.engineTemp    | 0 / 0 / 0   | 0 / 0 / 0  | 0 / 0 / 0  |
| veh.cluster.VehicleStatus.warnings      | 0 / 0 / 0   | 0 / 0 / 0  | 0 / 0 / 0  |
| veh.cluster.WheelHistory.wheelTicks     | 0 / 0 / 0   | 0 / 0 / 0  | 0 / 0 / 0  |
| veh.cluster.WheelHistory.wheelTemp      | 0 / 0 / 0   | 0 / 0 / 0  | 0 / 0 / 0  |
| veh.cluster.WheelHistory.wheelSlipRate  | 0 / 0 / 0   | 0 / 0 / 0  | 0 / 0 / 0  |
| veh.cluster.WheelSummary.wheelHealth    | 0 / 0 / 0   | 0 / 0 / 0  | 0 / 0 / 0  |
| veh.cluster.wheels.ticks                | 0 / 0 / 0   | 0 / 0 / 0  | 0 / 0 / 0  |
| veh.cluster.WheelDiagnostics.wheelTicks | not built   | not built  | not built  |
| veh.hvac.cabin.cabinTemp                | 0 / 0 / 0   | 0 / 0 / 0  | 0 / 0 / 0  |
| veh.hvac.cabin.targetTemp               | 0 / 0 / 0   | 0 / 0 / 0  | 0 / 0 / 0  |
| veh.hvac.cabin.blowerDuty               | 0 / 0 / 0   | 0 / 0 / 0  | 0 / 0 / 0  |
| veh.hvac.cabin.fanLevel                 | 0 / 0 / 0   | 0 / 0 / 0  | 0 / 0 / 0  |
| veh.hvac.cabin.compressorLoad           | 0 / 0 / 0   | 0 / 0 / 0  | 0 / 0 / 0  |
| veh.cabin.Cabin.temperature             | 0 / 0 / 0   | 0 / 0 / 0  | 0 / 0 / 0  |
| veh.cabin.Horn.active                   | 0 / 0 / 0   | 0 / 0 / 0  | 0 / 0 / 0  |
| bench.stress.Stress.longString          | 0 / 0 / 1   | 0 / 0 / 1  | 0 / 0 / 1  |
| bench.stress.Stress.itemList            | 1 / 0 / 17  | 0 / 0 / 19 | 0 / 0 / 17 |
| bench.stress.Stress.nestedOptional      | 0 / 0 / 2   | 0 / 0 / 2  | 0 / 0 / 2  |
| bench.stress.Stress.map                 | 1 / 0 / 17  | 0 / 0 / 19 | 1 / 0 / 17 |

##### Time per update

CPU time of one update (encode, verify and decode), median in ns, with the
fastest and slowest sample and the range as a share of the median. † spread
above 10 %: the figure is not stable.

| Payload                                 | FlatBuffers                    | proto3                            | podbuf                         |
| --------------------------------------- | ------------------------------ | --------------------------------- | ------------------------------ |
| veh.cluster.VehicleStatus.currentSpeed  | 145.8 (144.5 to 147.0; 2 %)    | 116.3 (115.7 to 116.8; 1 %)       | 101.7 (100.4 to 103.4; 3 %)    |
| veh.cluster.VehicleStatus.engineTemp    | 152.5 (150.7 to 154.6; 3 %)    | 121.4 (119.8 to 122.1; 2 %)       | 107.5 (106.5 to 108.0; 1 %)    |
| veh.cluster.VehicleStatus.warnings      | 98.3 (97.9 to 98.4; 1 %)       | 77.3 (77.3 to 77.5; 0 %)          | 36.6 (36.2 to 36.9; 2 %)       |
| veh.cluster.WheelHistory.wheelTicks     | 99.2 (98.9 to 99.6; 1 %)       | 75.7 (75.6 to 76.0; 1 %)          | 37.0 (36.4 to 37.2; 2 %)       |
| veh.cluster.WheelHistory.wheelTemp      | 152.4 (150.8 to 154.3; 2 %)    | 121.2 (119.9 to 122.2; 2 %)       | 107.6 (107.1 to 108.9; 2 %)    |
| veh.cluster.WheelHistory.wheelSlipRate  | 151.1 (149.5 to 152.5; 2 %)    | 118.8 (118.1 to 119.6; 1 %)       | 106.5 (106.1 to 106.9; 1 %)    |
| veh.cluster.WheelSummary.wheelHealth    | 99.2 (98.9 to 100.1; 1 %)      | 75.7 (75.6 to 76.0; 1 %)          | 37.0 (36.2 to 37.2; 3 %)       |
| veh.cluster.wheels.ticks                | 99.2 (99.0 to 99.3; 0 %)       | 75.7 (75.6 to 76.0; 1 %)          | 37.0 (36.6 to 37.2; 2 %)       |
| veh.cluster.WheelDiagnostics.wheelTicks | not built                      | not built                         | not built                      |
| veh.hvac.cabin.cabinTemp                | 152.4 (151.3 to 153.8; 2 %)    | 121.1 (119.3 to 122.6; 3 %)       | 107.7 (107.0 to 108.3; 1 %)    |
| veh.hvac.cabin.targetTemp               | 152.3 (150.6 to 154.4; 2 %)    | 121.0 (120.3 to 122.3; 2 %)       | 107.6 (107.1 to 108.3; 1 %)    |
| veh.hvac.cabin.blowerDuty               | 151.0 (150.0 to 152.7; 2 %)    | 118.9 (118.3 to 120.3; 2 %)       | 106.5 (105.9 to 107.7; 2 %)    |
| veh.hvac.cabin.fanLevel                 | 99.2 (99.0 to 99.5; 0 %)       | 75.7 (75.6 to 75.9; 1 %)          | 37.0 (36.7 to 37.3; 2 %)       |
| veh.hvac.cabin.compressorLoad           | 151.2 (150.5 to 152.7; 1 %)    | 119.2 (118.6 to 119.9; 1 %)       | 106.3 (105.6 to 106.8; 1 %)    |
| veh.cabin.Cabin.temperature             | 97.8 (97.6 to 98.2; 1 %)       | 77.5 (77.4 to 77.8; 0 %)          | 36.8 (36.2 to 37.3; 3 %)       |
| veh.cabin.Horn.active                   | 107.2 (106.4 to 107.5; 1 %)    | 73.7 (73.6 to 73.8; 0 %)          | 42.9 (42.3 to 43.9; 4 %)       |
| bench.stress.Stress.longString          | 745.3 (744.2 to 746.9; 0 %)    | 405.6 (405.5 to 405.8; 0 %)       | 802.3 (801.3 to 803.1; 0 %)    |
| bench.stress.Stress.itemList            | 3466.1 (3464.1 to 3471.9; 0 %) | 4594.2 (4586.1 to 4597.9; 0 %)    | 2970.8 (2967.6 to 2977.8; 0 %) |
| bench.stress.Stress.nestedOptional      | 485.7 (484.8 to 486.8; 0 %)    | 495.7 (494.1 to 497.4; 1 %)       | 360.5 (357.5 to 362.0; 1 %)    |
| bench.stress.Stress.map                 | 7934.6 (7907.1 to 7950.3; 1 %) | 12257.2 (12206.0 to 12315.5; 1 %) | 3841.3 (3837.7 to 3867.8; 1 %) |

##### CPU load, derived

CPU load in percent of one core at 4000 updates per second, derived from the
median time of one update (the headline figure).

| Payload                                 | FlatBuffers | proto3    | podbuf    |
| --------------------------------------- | ----------- | --------- | --------- |
| veh.cluster.VehicleStatus.currentSpeed  | 0.0583      | 0.0465    | 0.0407    |
| veh.cluster.VehicleStatus.engineTemp    | 0.0610      | 0.0486    | 0.0430    |
| veh.cluster.VehicleStatus.warnings      | 0.0393      | 0.0309    | 0.0146    |
| veh.cluster.WheelHistory.wheelTicks     | 0.0397      | 0.0303    | 0.0148    |
| veh.cluster.WheelHistory.wheelTemp      | 0.0609      | 0.0485    | 0.0431    |
| veh.cluster.WheelHistory.wheelSlipRate  | 0.0604      | 0.0475    | 0.0426    |
| veh.cluster.WheelSummary.wheelHealth    | 0.0397      | 0.0303    | 0.0148    |
| veh.cluster.wheels.ticks                | 0.0397      | 0.0303    | 0.0148    |
| veh.cluster.WheelDiagnostics.wheelTicks | not built   | not built | not built |
| veh.hvac.cabin.cabinTemp                | 0.0609      | 0.0484    | 0.0431    |
| veh.hvac.cabin.targetTemp               | 0.0609      | 0.0484    | 0.0430    |
| veh.hvac.cabin.blowerDuty               | 0.0604      | 0.0476    | 0.0426    |
| veh.hvac.cabin.fanLevel                 | 0.0397      | 0.0303    | 0.0148    |
| veh.hvac.cabin.compressorLoad           | 0.0605      | 0.0477    | 0.0425    |
| veh.cabin.Cabin.temperature             | 0.0391      | 0.0310    | 0.0147    |
| veh.cabin.Horn.active                   | 0.0429      | 0.0295    | 0.0171    |
| bench.stress.Stress.longString          | 0.2981      | 0.1622    | 0.3209    |
| bench.stress.Stress.itemList            | 1.3864      | 1.8377    | 1.1883    |
| bench.stress.Stress.nestedOptional      | 0.1943      | 0.1983    | 0.1442    |
| bench.stress.Stress.map                 | 3.1738      | 4.9029    | 1.5365    |

##### Ratios to podbuf

Each encoding divided by podbuf: above 1.00 the encoding is larger or slower
than podbuf, below 1.00 smaller or faster. The CPU ratios equal the time ratios,
because the derived CPU load is proportional to the median time. A refused or
not-built cell has no ratio and is marked.

| Payload                                 | Size: FlatBuffers / podbuf | Size: proto3 / podbuf | Time: FlatBuffers / podbuf | Time: proto3 / podbuf | CPU: FlatBuffers / podbuf | CPU: proto3 / podbuf |
| --------------------------------------- | -------------------------- | --------------------- | -------------------------- | --------------------- | ------------------------- | -------------------- |
| veh.cluster.VehicleStatus.currentSpeed  | 1.50                       | 0.31                  | 1.43                       | 1.14                  | 1.43                      | 1.14                 |
| veh.cluster.VehicleStatus.engineTemp    | 1.50                       | 0.31                  | 1.42                       | 1.13                  | 1.42                      | 1.13                 |
| veh.cluster.VehicleStatus.warnings      | 1.50                       | 0.12                  | 2.69                       | 2.11                  | 2.69                      | 2.11                 |
| veh.cluster.WheelHistory.wheelTicks     | 1.50                       | 0.12                  | 2.68                       | 2.05                  | 2.68                      | 2.05                 |
| veh.cluster.WheelHistory.wheelTemp      | 1.50                       | 0.31                  | 1.42                       | 1.13                  | 1.42                      | 1.13                 |
| veh.cluster.WheelHistory.wheelSlipRate  | 1.50                       | 0.31                  | 1.42                       | 1.12                  | 1.42                      | 1.12                 |
| veh.cluster.WheelSummary.wheelHealth    | 1.50                       | 0.12                  | 2.68                       | 2.05                  | 2.68                      | 2.05                 |
| veh.cluster.wheels.ticks                | 1.50                       | 0.12                  | 2.68                       | 2.05                  | 2.68                      | 2.05                 |
| veh.cluster.WheelDiagnostics.wheelTicks | not built                  | not built             | not built                  | not built             | not built                 | not built            |
| veh.hvac.cabin.cabinTemp                | 1.50                       | 0.31                  | 1.42                       | 1.12                  | 1.42                      | 1.12                 |
| veh.hvac.cabin.targetTemp               | 1.50                       | 0.31                  | 1.41                       | 1.12                  | 1.41                      | 1.12                 |
| veh.hvac.cabin.blowerDuty               | 1.50                       | 0.31                  | 1.42                       | 1.12                  | 1.42                      | 1.12                 |
| veh.hvac.cabin.fanLevel                 | 1.50                       | 0.12                  | 2.68                       | 2.04                  | 2.68                      | 2.04                 |
| veh.hvac.cabin.compressorLoad           | 1.50                       | 0.31                  | 1.42                       | 1.12                  | 1.42                      | 1.12                 |
| veh.cabin.Cabin.temperature             | 1.50                       | 0.12                  | 2.66                       | 2.11                  | 2.66                      | 2.11                 |
| veh.cabin.Horn.active                   | 2.00                       | 0.12                  | 2.50                       | 1.72                  | 2.50                      | 1.72                 |
| bench.stress.Stress.longString          | 1.03                       | 0.97                  | 0.93                       | 0.51                  | 0.93                      | 0.51                 |
| bench.stress.Stress.itemList            | 1.50                       | 0.55                  | 1.17                       | 1.55                  | 1.17                      | 1.55                 |
| bench.stress.Stress.nestedOptional      | 1.63                       | 0.35                  | 1.35                       | 1.37                  | 1.35                      | 1.37                 |
| bench.stress.Stress.map                 | 1.56                       | 0.53                  | 2.07                       | 3.19                  | 2.07                      | 3.19                 |

##### CPU load, paced and derived

CPU load in percent of one core at 4000 updates per second: paced (process CPU
time over wall time, includes the cost of sleep and wake-up) next to the derived
figure, and their difference.

| Payload                                | Encoding    | Paced % | Derived % | Difference |
| -------------------------------------- | ----------- | ------- | --------- | ---------- |
| veh.cluster.VehicleStatus.currentSpeed | FlatBuffers | 3.8265  | 0.0583    | 3.7682     |
| veh.cluster.VehicleStatus.currentSpeed | proto3      | 4.1561  | 0.0465    | 4.1096     |
| veh.cluster.VehicleStatus.currentSpeed | podbuf      | 4.0873  | 0.0407    | 4.0466     |
| veh.cluster.VehicleStatus.engineTemp   | FlatBuffers | 4.1465  | 0.0610    | 4.0855     |
| veh.cluster.VehicleStatus.engineTemp   | proto3      | 4.1768  | 0.0486    | 4.1283     |
| veh.cluster.VehicleStatus.engineTemp   | podbuf      | 4.1003  | 0.0430    | 4.0573     |
| veh.cluster.VehicleStatus.warnings     | FlatBuffers | 2.2513  | 0.0393    | 2.2120     |
| veh.cluster.VehicleStatus.warnings     | proto3      | 2.3145  | 0.0309    | 2.2836     |
| veh.cluster.VehicleStatus.warnings     | podbuf      | 2.2899  | 0.0146    | 2.2753     |
| veh.cluster.WheelHistory.wheelTicks    | FlatBuffers | 3.3149  | 0.0397    | 3.2752     |
| veh.cluster.WheelHistory.wheelTicks    | proto3      | 3.4239  | 0.0303    | 3.3936     |
| veh.cluster.WheelHistory.wheelTicks    | podbuf      | 3.3116  | 0.0148    | 3.2968     |
| veh.cluster.WheelHistory.wheelTemp     | FlatBuffers | 4.1486  | 0.0609    | 4.0876     |
| veh.cluster.WheelHistory.wheelTemp     | proto3      | 4.2367  | 0.0485    | 4.1882     |
| veh.cluster.WheelHistory.wheelTemp     | podbuf      | 4.0903  | 0.0431    | 4.0472     |
| veh.cluster.WheelHistory.wheelSlipRate | FlatBuffers | 2.2247  | 0.0604    | 2.1643     |
| veh.cluster.WheelHistory.wheelSlipRate | proto3      | 2.2530  | 0.0475    | 2.2055     |
| veh.cluster.WheelHistory.wheelSlipRate | podbuf      | 2.1268  | 0.0426    | 2.0842     |
| veh.cluster.WheelSummary.wheelHealth   | FlatBuffers | 2.0558  | 0.0397    | 2.0161     |
| veh.cluster.WheelSummary.wheelHealth   | proto3      | 2.0771  | 0.0303    | 2.0469     |
| veh.cluster.WheelSummary.wheelHealth   | podbuf      | 2.0038  | 0.0148    | 1.9890     |
| veh.cluster.wheels.ticks               | FlatBuffers | 2.0589  | 0.0397    | 2.0193     |
| veh.cluster.wheels.ticks               | proto3      | 2.0912  | 0.0303    | 2.0610     |
| veh.cluster.wheels.ticks               | podbuf      | 2.0315  | 0.0148    | 2.0167     |
| veh.hvac.cabin.cabinTemp               | FlatBuffers | 2.1897  | 0.0609    | 2.1288     |
| veh.hvac.cabin.cabinTemp               | proto3      | 2.2135  | 0.0484    | 2.1651     |
| veh.hvac.cabin.cabinTemp               | podbuf      | 2.1166  | 0.0431    | 2.0735     |
| veh.hvac.cabin.targetTemp              | FlatBuffers | 2.1715  | 0.0609    | 2.1105     |
| veh.hvac.cabin.targetTemp              | proto3      | 2.1955  | 0.0484    | 2.1471     |
| veh.hvac.cabin.targetTemp              | podbuf      | 2.1434  | 0.0430    | 2.1003     |
| veh.hvac.cabin.blowerDuty              | FlatBuffers | 2.2717  | 0.0604    | 2.2112     |
| veh.hvac.cabin.blowerDuty              | proto3      | 2.2153  | 0.0476    | 2.1677     |
| veh.hvac.cabin.blowerDuty              | podbuf      | 2.1410  | 0.0426    | 2.0984     |
| veh.hvac.cabin.fanLevel                | FlatBuffers | 2.0617  | 0.0397    | 2.0220     |
| veh.hvac.cabin.fanLevel                | proto3      | 2.0814  | 0.0303    | 2.0512     |
| veh.hvac.cabin.fanLevel                | podbuf      | 2.0195  | 0.0148    | 2.0047     |
| veh.hvac.cabin.compressorLoad          | FlatBuffers | 2.2370  | 0.0605    | 2.1765     |
| veh.hvac.cabin.compressorLoad          | proto3      | 2.2068  | 0.0477    | 2.1591     |
| veh.hvac.cabin.compressorLoad          | podbuf      | 2.1317  | 0.0425    | 2.0891     |
| veh.cabin.Cabin.temperature            | FlatBuffers | 2.1416  | 0.0391    | 2.1025     |
| veh.cabin.Cabin.temperature            | proto3      | 2.1292  | 0.0310    | 2.0981     |
| veh.cabin.Cabin.temperature            | podbuf      | 2.0158  | 0.0147    | 2.0011     |
| veh.cabin.Horn.active                  | FlatBuffers | 2.1218  | 0.0429    | 2.0789     |
| veh.cabin.Horn.active                  | proto3      | 2.0840  | 0.0295    | 2.0545     |
| veh.cabin.Horn.active                  | podbuf      | 2.0318  | 0.0171    | 2.0146     |
| bench.stress.Stress.longString         | FlatBuffers | 4.3091  | 0.2981    | 4.0110     |
| bench.stress.Stress.longString         | proto3      | 3.9179  | 0.1622    | 3.7557     |
| bench.stress.Stress.longString         | podbuf      | 4.4935  | 0.3209    | 4.1726     |
| bench.stress.Stress.itemList           | FlatBuffers | 15.4686 | 1.3864    | 14.0822    |
| bench.stress.Stress.itemList           | proto3      | 18.1575 | 1.8377    | 16.3198    |
| bench.stress.Stress.itemList           | podbuf      | 13.8093 | 1.1883    | 12.6210    |
| bench.stress.Stress.nestedOptional     | FlatBuffers | 2.7288  | 0.1943    | 2.5345     |
| bench.stress.Stress.nestedOptional     | proto3      | 2.8823  | 0.1983    | 2.6841     |
| bench.stress.Stress.nestedOptional     | podbuf      | 2.6178  | 0.1442    | 2.4736     |
| bench.stress.Stress.map                | FlatBuffers | 13.7311 | 3.1738    | 10.5573    |
| bench.stress.Stress.map                | proto3      | 19.5051 | 4.9029    | 14.6022    |
| bench.stress.Stress.map                | podbuf      | 8.1194  | 1.5365    | 6.5829     |

#### Payloads not built

The bench could not build a value of these payloads, so no encoding was tried on
them. This is not an encoding refusal.

| Payload                                 | Payload type | Reason                                                                                                           |
| --------------------------------------- | ------------ | ---------------------------------------------------------------------------------------------------------------- |
| veh.cluster.WheelDiagnostics.wheelTicks | RawTickCount | the payload type RawTickCount is internal to its package, so a value cannot be built outside the generated crate |

#### Refused cells

None.
