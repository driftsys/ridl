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
  the headline because the paced one measures mostly the timer, not the codec:
  on the development machine a 40 ns update gave about 0.96 % paced and about
  0.016 % derived, so more than 98 % of the paced figure was not the update. A
  paced run cannot be split between the three encodings, so the paced runs are
  one cell after the other, not interleaved.
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
  buffer towards the start, and allocates where the domain type already does: a
  vector of tables or strings needs each element's position first. See
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
- **Paced load** shows what a program that waits between updates measures; it is
  dominated by the cost of waiting on the development machine, as the tables
  show.
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
  largest. The stress payloads follow the same order, except that proto3 and
  podbuf are within 3 % of each other on the long string.
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
  allocates for FlatBuffers (list of structs, map) and podbuf (map).
- Paced against derived: the paced figure is 0.6 % to 3.8 % for the signal
  payloads and the derived figure is below 0.02 %. The difference is the cost of
  waiting, as the method section explains.

The tables are the output of `cargo xtask bench report`, unedited except for the
heading levels.

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

The device tables are not yet recorded. The Pixel 5 was not connected when this
note was written, so there is no figure for the Cortex-A55 and Cortex-A76 cores
here, and the host figures above must not be read as a substitute for them.

To produce them, connect the phone with USB debugging authorized, then run:

    just bench-pixel
    cargo xtask bench report

The first command cross-builds the harness, runs it pinned to an A55 core and to
an A76 core, and writes `pixel5-a55-<date>.json`, `pixel5-a76-<date>.json` and
the `.cpufreq.txt` files to `bench/encodings/results/`. The second prints the
tables for every file in that directory. Record the tables in this note, with
the date, the commit and the frequencies the `.cpufreq.txt` files report.
