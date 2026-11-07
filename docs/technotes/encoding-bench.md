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
  and twelve stress payloads from `bench/encodings/stress/`: a long string, a
  list of structs, nested optionals and a map; the same four shapes at several
  kilobytes (`bigString`, `bigItemList`, `bigSamples`, `bigMap`); a list of rows
  that hold a pair and a list of cells (`deepNested`); a union beside an open
  enum (`variant`); and a three-layer struct of 30 optional fields, set at two
  fields (`deepSparse`) and at all fields (`deepFull`). A signal whose payload
  type the bench cannot build a value of is listed as not built.
- **Equal work.** One update is: encode a value into a stack buffer, verify the
  bytes, decode a value from the verified bytes. The code path is generic over
  the encoding (`Ref::<T, E>::encode`, `verify`, `decode`), so the three
  encodings run the same bench code. Every result goes through `black_box`.
- **Operations timed apart.** Each of the three operations of each encoding has
  its own runner, and a batch of a runner repeats that one operation. Encode
  writes the value into the same buffer each time. Verify checks the same
  already-encoded bytes each time. Decode builds a value from a proof that
  `Ref::verify` built once, before any batch. `Ref::decode` takes the proof by
  value and the proof cannot be cloned, so each decode reads a bitwise copy of
  it. That is sound for the corpus because every view of the corpus holds only
  borrowed slices and integers, and an assertion on `needs_drop` stops the run
  for a payload whose proof owns data that is dropped. The bytes of the proof
  are leaked: one buffer per value of the pool, so one buffer without `--vary`
  and 64 buffers of up to the maximum encoded size with it, per payload and
  encoding. Without `--vary` every batch repeats one value. With `--vary`, for
  the seven payloads that have a pool, the k-th repetition of a batch works on
  value `k mod 64` of 64 values of one shape and different content, with the
  encoded buffer and the proof of that value. The time of one update is the sum
  of the medians of its three operations.
- **Access.** For four payloads (`bigItemList`, `deepNested`, `deepSparse`,
  `deepFull`) FlatBuffers and podbuf have a fourth runner that reads one number
  through the view of verified bytes: a count of a list element, a count of a
  cell, or the length of a string field. It is not part of the time of one
  update. proto3 generates no accessors, so it has no access runner.
- **Interleave.** The nine runners of a payload (three encodings, three
  operations; eleven for a payload with an access runner in two encodings) are
  timed in rotation inside the same run: sample `k` starts with runner `k mod 9`
  (`k mod 11`). Frequency changes and thermal throttling therefore affect them
  alike.
- **Warm-up and samples.** The batch size is calibrated so that a batch lasts at
  least 200 microseconds; 3 warm-up batches are discarded; 30 samples follow.
  The time of one operation is the time of one batch divided by the batch size.
  The figure is the median of the 30 samples. The tables also give the fastest
  and the slowest sample and the range as a share of the median. For one update
  the median is the sum of the three operation medians, and its fastest and
  slowest sample are the sums of the three operations' fastest and slowest
  samples: an envelope of summed bounds, not a percentile of summed samples.
  `--duration` is the CPU time of the samples of one operation of one encoding
  of one payload, so one cell takes three times that (four times for the
  FlatBuffers and podbuf cells of a payload with an access runner); the Pixel
  runs used 2 s, the host run 0.5 s. A cell whose `(p95 - p05) / median` or
  `(max - min) / median` is above 10 % carries the marker `†`: its median is not
  stable.
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
  one: the paced figure of the signal payloads ranges from 0.68 % to 5.90 % in
  the host tables, from 8.9 % to 11.8 % on the Pixel 5 A55 core and from 1.9 %
  to 4.2 % on the A76 core, where the derived load is at most 0.0146 %, 0.180 %
  and 0.064 %, so the update is a small part of what the paced run measures. A
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
  `alloc`, so they are counted. The three counts are taken separately: encode,
  verify and decode each count their own call alone. The counts are the same on
  every platform. The stack buffer for encode is `T::BUFFER`, so the output
  buffer is not counted.
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
  The access runner of four payloads uses the in-place accessors, which are the
  reason podbuf exists
  ([the podbuf design](../archive/2026-10-24-podbuf-design.md), section 1); the
  other operations do not use them, so a consumer that reads two fields of a
  large payload pays less than the update figure shows. See
  [the podbuf codec record](../design/podbuf-codec.md).
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
- **Time per operation** is the CPU time of encode, of verify or of decode
  alone, on a warm cache. Without `--vary` it is the same value every time. The
  decode of a small scalar payload takes 0.5 ns to 7 ns on the host, and the
  podbuf decode of the cheapest payloads takes 7.2 ns on the A55 core and 2.0 ns
  on the A76 core. Any per-operation figure includes some harness cost (the
  `black_box` calls around the input and the result, and for decode the copy of
  the proof), so a figure below about 10 ns is an upper bound and is not
  comparable between encodings. The limit of 10 ns was established for decode,
  where the copy of the proof adds to the cost; the harness cost of encode and
  verify was not measured separately.
- **Time per update** is the sum of the three operation medians, on a warm cache
  and, without `--vary`, with the same value every time. It does not show the
  cost of a cold cache. Without `--vary` it does not show the cost of branch
  mispredictions on varying data or of a value that changes between updates.
  With `--vary`, seven payloads change content between repetitions. A payload
  that changes size or shape is not modelled.
- **CPU load at 4 000 updates per second (derived)** is the time per update
  scaled to the rate. It shows the codec's share of a core. It does not include
  the transport, the application, the scheduler or the timer.
- **Paced load** shows what a program that waits between updates measures; for
  the signal payloads it varies from 0.68 % to 5.90 % in the host tables, where
  the derived load is at most 0.0146 %, so it is dominated by something other
  than the codec (the cost of the timer and of the wake-up on an unpinned
  machine is the likely cause; it is not a measurement).
- **Ratios** divide each encoding by podbuf. A ratio above 1.00 means the
  encoding is larger or slower than podbuf on that payload. A ratio per
  operation divides by the podbuf time of the same operation. A ratio is
  computed only when both cells are measured.
- **The mix.** The equal-weight mix is a summary, not a result. It is given per
  operation as well as for the update, and each is given twice, because the
  corpus repeats payload types: several signals carry the same payload type, so
  a mix per signal gives those types more weight than a mix per distinct payload
  type does. The per-payload tables are the full result. A payload with a
  refused or not-built cell is in no mix and no ratio.
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
a branch predictor, and the spike shows instructions. And it reports each of
encode, verify and decode, and their sum, where the spike gives verify and
encode apart. The two do not replace each other, and the bench does not repeat
the spike's instruction counts or its code-size figures.

## Host results

**Measured on the development machine; absolute times do not transfer to the
target.** The machine is an Apple Silicon Mac (macOS, aarch64) that is faster
than the target cores and has larger caches. The run is not pinned and the
machine may run other processes, which is why many cells carry the marker `†`:
23 of the 57 update cells and 34 of the 171 operation cells. The numbers show
the shape of the work, not the load on a vehicle's computer.

- Date: 2026-11-05
- Commit: d2bde651 (the measuring code is that of 56ddf829; the later commits
  change comments and the report)
- Command:
  `cargo run --release -p encoding-bench -- --out results/host-macos-arm64.json --core-label "development machine (macOS arm64, not pinned)" --duration 0.5 --paced`,
  then `cargo xtask bench report`
- Toolchain: rustc 1.98.1 (48a229cea 2026-09-01)
- Run: 4 000 updates per second, 30 samples per cell, 0.5 s of CPU time per
  operation, cell and encoding, paced run included

What the tables show, without a recommendation:

- Size: proto3 is the smallest on every measured signal row and FlatBuffers the
  largest. The stress payloads follow the same order (proto3, podbuf,
  FlatBuffers), and the gap between proto3 and podbuf on the long string is
  small (2.7 %).
- Time per update: podbuf has the lowest median on every measured signal row and
  on three of the four stress rows (list of structs, nested optionals, map). On
  the long string it has the highest (172 ns against 149 and 161 ns).
- Load: the largest load of a signal payload is 0.0146 % of one core at 4 000
  updates per second (FlatBuffers, `veh.cluster.WheelHistory.wheelSlipRate`).
  The largest of all cells is 2.06 % (proto3, the map stress payload).
- Per operation, over the 15 signals (the median over the signals of each
  signal's median): encode 19.8 ns for FlatBuffers, 9.2 ns for proto3 and 6.6 ns
  for podbuf; verify 13.6, 15.2 and 13.5 ns; decode 2.8, 5.5 and 2.7 ns. The
  decode times are below 10 ns, so they are upper bounds that are not comparable
  (method section). Many operation cells are flagged on this machine
  (FlatBuffers encode: 7 of 15 signals), so the ratios below are not reliable to
  better than the flagged spread.
- Allocations: the codecs make no allocation for the signal payloads. Only the
  stress payloads allocate: decode allocates for every one of them, and encode
  allocates for FlatBuffers (list of structs, map) and podbuf (map); the cause
  of the podbuf map allocation is not established here.
- Paced against derived: the paced figure is 0.68 % to 5.90 % for the signal
  payloads and the derived figure is at most 0.0146 %. The paced figure is not a
  codec figure, as the method section explains.

The tables are the output of `cargo xtask bench report`, unedited except for the
heading levels. The host tables are a snapshot of one run at commit d2bde651.
The Pixel 5 tables below are a snapshot of one run at commit 56ddf829. The
directory `bench/encodings/results/` is git-ignored, so the JSON behind the
tables is not in the repository. Refresh the host tables by rerunning the
command above and the Pixel tables by rerunning `just bench-pixel`, then run
`cargo xtask bench report`.

### Mix

Equal-weight mix, per signal: each signal counts once, so a payload type that
several signals share counts once for each of them. Only signals whose three
encodings are measured are counted; the last column gives how many.

| Metric                                                 | FlatBuffers | proto3 | podbuf | FlatBuffers / podbuf | proto3 / podbuf | Units |
| ------------------------------------------------------ | ----------- | ------ | ------ | -------------------- | --------------- | ----- |
| Size (bytes)                                           | 24.5        | 3.6    | 16.0   | 1.53                 | 0.23            | 15    |
| Heap allocations per update (encode + verify + decode) | 0.0         | 0.0    | 0.0    | n/a                  | n/a             | 15    |
| Time per update (median ns)                            | 33.1        | 25.7   | 17.3   | 1.91                 | 1.48            | 15    |
| CPU % of one core at 4000 updates per second (derived) | 0.0133      | 0.0103 | 0.0069 | 1.91                 | 1.48            | 15    |

Equal-weight mix, per payload type: each payload type counts once, however many
signals share it (the value of a type is the mean over its signals). Only
payload types whose signals are measured in all three encodings are counted; the
last column gives how many.

| Metric                                                 | FlatBuffers | proto3 | podbuf | FlatBuffers / podbuf | proto3 / podbuf | Units |
| ------------------------------------------------------ | ----------- | ------ | ------ | -------------------- | --------------- | ----- |
| Size (bytes)                                           | 25.3        | 3.4    | 16.0   | 1.58                 | 0.21            | 6     |
| Heap allocations per update (encode + verify + decode) | 0.0         | 0.0    | 0.0    | n/a                  | n/a             | 6     |
| Time per update (median ns)                            | 33.1        | 25.1   | 16.3   | 2.03                 | 1.54            | 6     |
| CPU % of one core at 4000 updates per second (derived) | 0.0132      | 0.0100 | 0.0065 | 2.03                 | 1.54            | 6     |

Equal-weight mix, per signal: each signal counts once, so a payload type that
several signals share counts once for each of them. Only signals whose three
encodings are measured are counted; the last column gives how many. The rows are
per operation. A time under about 10 ns includes harness cost and is an upper
bound, so a ratio whose podbuf time is under 10 ns is not comparable.

| Metric                                                         | FlatBuffers | proto3 | podbuf | FlatBuffers / podbuf | proto3 / podbuf | Units |
| -------------------------------------------------------------- | ----------- | ------ | ------ | -------------------- | --------------- | ----- |
| Encode: time per operation (median ns)                         | 20.0        | 8.1    | 6.7    | 2.97                 | 1.20            | 15    |
| Encode: CPU % of one core at 4000 updates per second (derived) | 0.0080      | 0.0032 | 0.0027 | 2.97                 | 1.20            | 15    |
| Encode: heap allocations                                       | 0.0         | 0.0    | 0.0    | n/a                  | n/a             | 15    |
| Verify: time per operation (median ns)                         | 10.1        | 11.7   | 8.8    | 1.15                 | 1.34            | 15    |
| Verify: CPU % of one core at 4000 updates per second (derived) | 0.0041      | 0.0047 | 0.0035 | 1.15                 | 1.34            | 15    |
| Verify: heap allocations                                       | 0.0         | 0.0    | 0.0    | n/a                  | n/a             | 15    |
| Decode: time per operation (median ns)                         | 3.0         | 5.9    | 1.8    | 1.67                 | 3.28            | 15    |
| Decode: CPU % of one core at 4000 updates per second (derived) | 0.0012      | 0.0024 | 0.0007 | 1.67                 | 3.28            | 15    |
| Decode: heap allocations                                       | 0.0         | 0.0    | 0.0    | n/a                  | n/a             | 15    |

Equal-weight mix, per payload type: each payload type counts once, however many
signals share it (the value of a type is the mean over its signals). Only
payload types whose signals are measured in all three encodings are counted; the
last column gives how many. The rows are per operation. A time under about 10 ns
includes harness cost and is an upper bound, so a ratio whose podbuf time is
under 10 ns is not comparable.

| Metric                                                         | FlatBuffers | proto3 | podbuf | FlatBuffers / podbuf | proto3 / podbuf | Units |
| -------------------------------------------------------------- | ----------- | ------ | ------ | -------------------- | --------------- | ----- |
| Encode: time per operation (median ns)                         | 20.0        | 7.9    | 6.8    | 2.96                 | 1.16            | 6     |
| Encode: CPU % of one core at 4000 updates per second (derived) | 0.0080      | 0.0031 | 0.0027 | 2.96                 | 1.16            | 6     |
| Encode: heap allocations                                       | 0.0         | 0.0    | 0.0    | n/a                  | n/a             | 6     |
| Verify: time per operation (median ns)                         | 10.1        | 11.2   | 7.9    | 1.28                 | 1.42            | 6     |
| Verify: CPU % of one core at 4000 updates per second (derived) | 0.0040      | 0.0045 | 0.0031 | 1.28                 | 1.42            | 6     |
| Verify: heap allocations                                       | 0.0         | 0.0    | 0.0    | n/a                  | n/a             | 6     |
| Decode: time per operation (median ns)                         | 3.0         | 6.0    | 1.7    | 1.76                 | 3.54            | 6     |
| Decode: CPU % of one core at 4000 updates per second (derived) | 0.0012      | 0.0024 | 0.0007 | 1.76                 | 3.54            | 6     |
| Decode: heap allocations                                       | 0.0         | 0.0    | 0.0    | n/a                  | n/a             | 6     |

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

CPU time of one update (encode, verify and decode), in ns: the sum of the
medians of the three operations, with the sum of their fastest and the sum of
their slowest sample (an envelope, not a percentile) and its range as a share of
the sum. † spread above 10 %: the figure is not stable.

| Payload                                 | FlatBuffers                      | proto3                           | podbuf                            |
| --------------------------------------- | -------------------------------- | -------------------------------- | --------------------------------- |
| veh.cluster.VehicleStatus.currentSpeed  | 36.3 (35.6 to 37.4; 5 %)         | 29.9 (29.6 to 30.9; 4 %)         | 22.9 (22.7 to 23.4; 3 %)          |
| veh.cluster.VehicleStatus.engineTemp    | 36.1 (35.4 to 37.6; 6 %)         | 30.2 (29.8 to 30.8; 3 %)         | 24.0 (23.8 to 24.4; 3 %)          |
| veh.cluster.VehicleStatus.warnings      | 30.6 (27.5 to 35.2; 25 %) †      | 20.4 (20.3 to 23.0; 13 %) †      | 9.9 (9.9 to 10.1; 2 %)            |
| veh.cluster.WheelHistory.wheelTicks     | 29.7 (27.1 to 37.0; 33 %) †      | 20.2 (19.5 to 22.4; 15 %) †      | 10.0 (9.8 to 10.5; 7 %)           |
| veh.cluster.WheelHistory.wheelTemp      | 36.4 (35.6 to 37.5; 5 %)         | 30.2 (29.9 to 30.7; 3 %)         | 24.1 (23.6 to 24.6; 4 %)          |
| veh.cluster.WheelHistory.wheelSlipRate  | 36.5 (35.4 to 37.6; 6 %)         | 30.3 (30.1 to 30.9; 3 %)         | 23.5 (23.4 to 24.8; 6 %)          |
| veh.cluster.WheelSummary.wheelHealth    | 29.9 (25.8 to 35.6; 33 %) †      | 20.2 (20.0 to 23.4; 17 %) †      | 9.9 (9.9 to 10.2; 3 %)            |
| veh.cluster.wheels.ticks                | 28.1 (26.8 to 35.6; 31 %) †      | 20.3 (20.1 to 22.5; 12 %) †      | 9.9 (9.9 to 10.1; 2 %)            |
| veh.cluster.WheelDiagnostics.wheelTicks | not built                        | not built                        | not built                         |
| veh.hvac.cabin.cabinTemp                | 36.2 (35.8 to 37.1; 4 %)         | 30.2 (30.0 to 31.2; 4 %)         | 24.1 (24.0 to 24.6; 2 %)          |
| veh.hvac.cabin.targetTemp               | 36.2 (35.6 to 37.7; 6 %)         | 30.2 (30.0 to 31.1; 4 %)         | 24.0 (23.5 to 35.9; 51 %) †       |
| veh.hvac.cabin.blowerDuty               | 36.3 (35.4 to 38.1; 8 %)         | 30.3 (30.1 to 31.0; 3 %)         | 23.5 (23.4 to 24.2; 3 %)          |
| veh.hvac.cabin.fanLevel                 | 28.1 (27.3 to 35.7; 30 %) †      | 20.3 (20.1 to 22.2; 10 %) †      | 9.9 (9.8 to 10.0; 2 %)            |
| veh.hvac.cabin.compressorLoad           | 36.5 (35.5 to 37.8; 6 %)         | 30.3 (30.1 to 31.3; 4 %)         | 23.3 (23.2 to 23.7; 2 %)          |
| veh.cabin.Cabin.temperature             | 28.6 (27.6 to 35.4; 27 %) †      | 20.9 (20.9 to 22.1; 6 %) †       | 10.0 (9.9 to 10.1; 2 %)           |
| veh.cabin.Horn.active                   | 31.6 (27.7 to 33.0; 16 %) †      | 21.4 (20.7 to 23.0; 11 %) †      | 10.6 (10.3 to 10.8; 4 %)          |
| bench.stress.Stress.longString          | 160.8 (154.9 to 168.9; 9 %) †    | 148.7 (146.0 to 156.8; 7 %) †    | 171.6 (167.2 to 177.8; 6 %) †     |
| bench.stress.Stress.itemList            | 1638.8 (1596.3 to 1705.7; 7 %)   | 2162.2 (2105.1 to 2262.7; 7 %)   | 1453.8 (1422.6 to 1511.7; 6 %)    |
| bench.stress.Stress.nestedOptional      | 189.0 (182.0 to 197.8; 8 %)      | 192.2 (186.4 to 202.2; 8 %) †    | 141.6 (136.9 to 148.8; 8 %) †     |
| bench.stress.Stress.map                 | 3046.9 (2996.5 to 3172.1; 6 %) † | 5142.3 (5023.6 to 5503.7; 9 %) † | 1683.5 (1629.6 to 2001.0; 22 %) † |

#### CPU load, derived

CPU load in percent of one core at 4000 updates per second, derived from the
median time of one update (the headline figure).

| Payload                                 | FlatBuffers | proto3    | podbuf    |
| --------------------------------------- | ----------- | --------- | --------- |
| veh.cluster.VehicleStatus.currentSpeed  | 0.0145      | 0.0119    | 0.0092    |
| veh.cluster.VehicleStatus.engineTemp    | 0.0145      | 0.0121    | 0.0096    |
| veh.cluster.VehicleStatus.warnings      | 0.0123      | 0.0082    | 0.0040    |
| veh.cluster.WheelHistory.wheelTicks     | 0.0119      | 0.0081    | 0.0040    |
| veh.cluster.WheelHistory.wheelTemp      | 0.0145      | 0.0121    | 0.0096    |
| veh.cluster.WheelHistory.wheelSlipRate  | 0.0146      | 0.0121    | 0.0094    |
| veh.cluster.WheelSummary.wheelHealth    | 0.0120      | 0.0081    | 0.0040    |
| veh.cluster.wheels.ticks                | 0.0112      | 0.0081    | 0.0040    |
| veh.cluster.WheelDiagnostics.wheelTicks | not built   | not built | not built |
| veh.hvac.cabin.cabinTemp                | 0.0145      | 0.0121    | 0.0096    |
| veh.hvac.cabin.targetTemp               | 0.0145      | 0.0121    | 0.0096    |
| veh.hvac.cabin.blowerDuty               | 0.0145      | 0.0121    | 0.0094    |
| veh.hvac.cabin.fanLevel                 | 0.0112      | 0.0081    | 0.0040    |
| veh.hvac.cabin.compressorLoad           | 0.0146      | 0.0121    | 0.0093    |
| veh.cabin.Cabin.temperature             | 0.0115      | 0.0084    | 0.0040    |
| veh.cabin.Horn.active                   | 0.0127      | 0.0086    | 0.0043    |
| bench.stress.Stress.longString          | 0.0643      | 0.0595    | 0.0686    |
| bench.stress.Stress.itemList            | 0.6555      | 0.8649    | 0.5815    |
| bench.stress.Stress.nestedOptional      | 0.0756      | 0.0769    | 0.0566    |
| bench.stress.Stress.map                 | 1.2188      | 2.0569    | 0.6734    |

#### Ratios to podbuf

Each encoding divided by podbuf: above 1.00 the encoding is larger or slower
than podbuf, below 1.00 smaller or faster. The CPU ratios equal the time ratios,
because the derived CPU load is proportional to the median time. A refused or
not-built cell has no ratio and is marked.

| Payload                                 | Size: FlatBuffers / podbuf | Size: proto3 / podbuf | Time: FlatBuffers / podbuf | Time: proto3 / podbuf | CPU: FlatBuffers / podbuf | CPU: proto3 / podbuf |
| --------------------------------------- | -------------------------- | --------------------- | -------------------------- | --------------------- | ------------------------- | -------------------- |
| veh.cluster.VehicleStatus.currentSpeed  | 1.50                       | 0.31                  | 1.58                       | 1.30                  | 1.58                      | 1.30                 |
| veh.cluster.VehicleStatus.engineTemp    | 1.50                       | 0.31                  | 1.50                       | 1.26                  | 1.50                      | 1.26                 |
| veh.cluster.VehicleStatus.warnings      | 1.50                       | 0.12                  | 3.09                       | 2.06                  | 3.09                      | 2.06                 |
| veh.cluster.WheelHistory.wheelTicks     | 1.50                       | 0.12                  | 2.98                       | 2.03                  | 2.98                      | 2.03                 |
| veh.cluster.WheelHistory.wheelTemp      | 1.50                       | 0.31                  | 1.51                       | 1.25                  | 1.51                      | 1.25                 |
| veh.cluster.WheelHistory.wheelSlipRate  | 1.50                       | 0.31                  | 1.55                       | 1.29                  | 1.55                      | 1.29                 |
| veh.cluster.WheelSummary.wheelHealth    | 1.50                       | 0.12                  | 3.01                       | 2.03                  | 3.01                      | 2.03                 |
| veh.cluster.wheels.ticks                | 1.50                       | 0.12                  | 2.84                       | 2.05                  | 2.84                      | 2.05                 |
| veh.cluster.WheelDiagnostics.wheelTicks | not built                  | not built             | not built                  | not built             | not built                 | not built            |
| veh.hvac.cabin.cabinTemp                | 1.50                       | 0.31                  | 1.50                       | 1.25                  | 1.50                      | 1.25                 |
| veh.hvac.cabin.targetTemp               | 1.50                       | 0.31                  | 1.51                       | 1.26                  | 1.51                      | 1.26                 |
| veh.hvac.cabin.blowerDuty               | 1.50                       | 0.31                  | 1.54                       | 1.29                  | 1.54                      | 1.29                 |
| veh.hvac.cabin.fanLevel                 | 1.50                       | 0.12                  | 2.84                       | 2.05                  | 2.84                      | 2.05                 |
| veh.hvac.cabin.compressorLoad           | 1.50                       | 0.31                  | 1.57                       | 1.30                  | 1.57                      | 1.30                 |
| veh.cabin.Cabin.temperature             | 1.50                       | 0.12                  | 2.87                       | 2.10                  | 2.87                      | 2.10                 |
| veh.cabin.Horn.active                   | 2.00                       | 0.12                  | 2.98                       | 2.01                  | 2.98                      | 2.01                 |
| bench.stress.Stress.longString          | 1.03                       | 0.97                  | 0.94                       | 0.87                  | 0.94                      | 0.87                 |
| bench.stress.Stress.itemList            | 1.50                       | 0.55                  | 1.13                       | 1.49                  | 1.13                      | 1.49                 |
| bench.stress.Stress.nestedOptional      | 1.63                       | 0.35                  | 1.34                       | 1.36                  | 1.34                      | 1.36                 |
| bench.stress.Stress.map                 | 1.56                       | 0.53                  | 1.81                       | 3.05                  | 1.81                      | 3.05                 |

#### Time per operation, encode

CPU time of the encode operation of one update, in ns: the median of 30 samples
of a batch of this operation alone, with the fastest and the slowest sample and
the range as a share of the median. † spread above 10 %: the figure is not
stable.

| Payload                                 | FlatBuffers                    | proto3                      | podbuf                         |
| --------------------------------------- | ------------------------------ | --------------------------- | ------------------------------ |
| veh.cluster.VehicleStatus.currentSpeed  | 19.8 (19.1 to 20.6; 7 %)       | 9.2 (9.1 to 9.9; 9 %)       | 6.6 (6.5 to 6.7; 2 %)          |
| veh.cluster.VehicleStatus.engineTemp    | 19.8 (19.1 to 20.7; 8 %)       | 9.2 (9.1 to 9.4; 3 %)       | 6.6 (6.5 to 6.7; 3 %)          |
| veh.cluster.VehicleStatus.warnings      | 21.0 (19.2 to 23.2; 19 %) †    | 6.6 (6.6 to 8.7; 33 %) †    | 6.9 (6.8 to 7.0; 2 %)          |
| veh.cluster.WheelHistory.wheelTicks     | 21.1 (18.9 to 24.9; 28 %) †    | 6.7 (6.4 to 8.6; 33 %) †    | 6.9 (6.8 to 7.2; 6 %)          |
| veh.cluster.WheelHistory.wheelTemp      | 20.0 (19.3 to 20.8; 8 %)       | 9.2 (9.1 to 9.4; 3 %)       | 6.6 (6.6 to 6.7; 2 %)          |
| veh.cluster.WheelHistory.wheelSlipRate  | 19.9 (18.9 to 20.7; 9 %)       | 9.2 (9.1 to 9.5; 4 %)       | 6.6 (6.6 to 6.7; 2 %)          |
| veh.cluster.WheelSummary.wheelHealth    | 21.4 (17.6 to 23.4; 27 %) †    | 6.7 (6.6 to 8.3; 26 %) †    | 6.9 (6.9 to 7.1; 3 %)          |
| veh.cluster.wheels.ticks                | 19.5 (18.6 to 23.5; 25 %) †    | 6.7 (6.6 to 8.7; 31 %) †    | 6.9 (6.9 to 7.0; 2 %)          |
| veh.cluster.WheelDiagnostics.wheelTicks | not built                      | not built                   | not built                      |
| veh.hvac.cabin.cabinTemp                | 19.9 (19.5 to 20.5; 5 %)       | 9.2 (9.1 to 9.9; 9 %)       | 6.6 (6.6 to 6.7; 2 %)          |
| veh.hvac.cabin.targetTemp               | 19.8 (19.3 to 20.7; 7 %)       | 9.2 (9.1 to 9.5; 4 %)       | 6.6 (6.4 to 6.8; 6 %)          |
| veh.hvac.cabin.blowerDuty               | 19.7 (18.9 to 20.8; 10 %)      | 9.2 (9.1 to 9.5; 5 %)       | 6.6 (6.6 to 6.7; 2 %)          |
| veh.hvac.cabin.fanLevel                 | 19.6 (19.0 to 23.5; 23 %) †    | 6.8 (6.6 to 8.2; 24 %) †    | 6.9 (6.8 to 7.0; 2 %)          |
| veh.hvac.cabin.compressorLoad           | 19.9 (18.9 to 20.8; 9 %)       | 9.3 (9.1 to 9.7; 6 %)       | 6.6 (6.6 to 6.7; 2 %)          |
| veh.cabin.Cabin.temperature             | 19.8 (19.2 to 23.3; 21 %) †    | 6.9 (6.9 to 7.8; 13 %) †    | 6.9 (6.9 to 7.0; 2 %)          |
| veh.cabin.Horn.active                   | 19.4 (18.0 to 20.3; 12 %) †    | 6.7 (6.6 to 8.0; 21 %) †    | 6.9 (6.7 to 7.0; 5 %)          |
| bench.stress.Stress.longString          | 39.3 (37.0 to 40.7; 9 %)       | 18.4 (18.1 to 18.6; 3 %)    | 34.5 (33.9 to 34.7; 2 %)       |
| bench.stress.Stress.itemList            | 433.7 (426.8 to 448.6; 5 %)    | 417.6 (407.6 to 441.4; 8 %) | 208.3 (206.9 to 212.6; 3 %)    |
| bench.stress.Stress.nestedOptional      | 64.3 (61.9 to 67.3; 8 %)       | 40.0 (39.6 to 40.3; 2 %)    | 29.5 (28.9 to 31.9; 10 %) †    |
| bench.stress.Stress.map                 | 432.5 (425.3 to 482.1; 13 %) † | 799.5 (784.3 to 839.2; 7 %) | 337.9 (333.8 to 424.8; 27 %) † |

#### Time per operation, verify

CPU time of the verify operation of one update, in ns: the median of 30 samples
of a batch of this operation alone, with the fastest and the slowest sample and
the range as a share of the median. † spread above 10 %: the figure is not
stable.

| Payload                                 | FlatBuffers                    | proto3                            | podbuf                         |
| --------------------------------------- | ------------------------------ | --------------------------------- | ------------------------------ |
| veh.cluster.VehicleStatus.currentSpeed  | 13.8 (13.7 to 14.0; 2 %)       | 15.2 (15.1 to 15.4; 2 %)          | 13.5 (13.5 to 13.8; 2 %)       |
| veh.cluster.VehicleStatus.engineTemp    | 13.6 (13.5 to 14.0; 3 %)       | 15.5 (15.3 to 15.8; 3 %)          | 14.7 (14.6 to 15.0; 3 %)       |
| veh.cluster.VehicleStatus.warnings      | 6.3 (5.0 to 8.6; 57 %) †       | 7.4 (7.4 to 7.7; 5 %)             | 2.5 (2.5 to 2.5; 2 %)          |
| veh.cluster.WheelHistory.wheelTicks     | 5.3 (5.0 to 8.7; 71 %) †       | 7.5 (7.2 to 7.7; 6 %)             | 2.5 (2.5 to 2.7; 9 %)          |
| veh.cluster.WheelHistory.wheelTemp      | 13.6 (13.5 to 13.9; 2 %)       | 15.5 (15.3 to 15.7; 3 %)          | 14.7 (14.3 to 15.0; 5 %)       |
| veh.cluster.WheelHistory.wheelSlipRate  | 13.9 (13.7 to 14.1; 3 %)       | 15.6 (15.5 to 15.8; 2 %)          | 14.0 (13.9 to 15.1; 8 %)       |
| veh.cluster.WheelSummary.wheelHealth    | 5.3 (4.9 to 8.8; 73 %) †       | 7.5 (7.4 to 7.7; 4 %)             | 2.5 (2.5 to 2.5; 3 %)          |
| veh.cluster.wheels.ticks                | 5.2 (5.0 to 8.8; 73 %) †       | 7.5 (7.4 to 7.7; 3 %)             | 2.5 (2.5 to 2.5; 2 %)          |
| veh.cluster.WheelDiagnostics.wheelTicks | not built                      | not built                         | not built                      |
| veh.hvac.cabin.cabinTemp                | 13.6 (13.6 to 13.8; 2 %)       | 15.5 (15.4 to 15.7; 2 %)          | 14.7 (14.7 to 15.0; 2 %)       |
| veh.hvac.cabin.targetTemp               | 13.6 (13.6 to 14.1; 4 %)       | 15.5 (15.4 to 16.0; 4 %)          | 14.7 (14.4 to 25.4; 74 %) †    |
| veh.hvac.cabin.blowerDuty               | 13.8 (13.8 to 14.4; 4 %)       | 15.6 (15.5 to 15.8; 2 %)          | 14.0 (13.9 to 14.5; 4 %)       |
| veh.hvac.cabin.fanLevel                 | 5.2 (5.0 to 8.8; 72 %) †       | 7.5 (7.4 to 7.8; 5 %)             | 2.5 (2.5 to 2.5; 2 %)          |
| veh.hvac.cabin.compressorLoad           | 13.8 (13.8 to 14.1; 2 %)       | 15.6 (15.5 to 16.0; 3 %)          | 14.0 (13.9 to 14.2; 2 %)       |
| veh.cabin.Cabin.temperature             | 5.5 (5.2 to 8.8; 65 %) †       | 7.7 (7.7 to 7.9; 2 %)             | 2.5 (2.5 to 2.5; 2 %)          |
| veh.cabin.Horn.active                   | 9.2 (6.8 to 9.6; 31 %) †       | 7.5 (7.3 to 7.6; 5 %)             | 2.5 (2.5 to 2.5; 3 %)          |
| bench.stress.Stress.longString          | 61.3 (60.2 to 63.3; 5 %)       | 65.4 (64.6 to 67.4; 4 %)          | 78.6 (77.8 to 79.5; 2 %)       |
| bench.stress.Stress.itemList            | 302.8 (298.2 to 321.5; 8 %)    | 495.8 (483.1 to 523.5; 8 %)       | 359.0 (355.0 to 373.4; 5 %)    |
| bench.stress.Stress.nestedOptional      | 39.6 (37.9 to 40.2; 6 %)       | 51.4 (50.3 to 56.0; 11 %) †       | 38.5 (37.7 to 39.9; 6 %)       |
| bench.stress.Stress.map                 | 1759.3 (1734.6 to 1802.7; 4 %) | 3129.7 (3048.0 to 3364.2; 10 %) † | 485.6 (470.8 to 605.4; 28 %) † |

#### Time per operation, decode

CPU time of the decode operation of one update, in ns: the median of 30 samples
of a batch of this operation alone, with the fastest and the slowest sample and
the range as a share of the median. † spread above 10 %: the figure is not
stable.

| Payload                                 | FlatBuffers                 | proto3                         | podbuf                         |
| --------------------------------------- | --------------------------- | ------------------------------ | ------------------------------ |
| veh.cluster.VehicleStatus.currentSpeed  | 2.8 (2.7 to 2.8; 3 %)       | 5.5 (5.5 to 5.6; 2 %)          | 2.8 (2.7 to 2.9; 6 %)          |
| veh.cluster.VehicleStatus.engineTemp    | 2.8 (2.7 to 2.9; 4 %)       | 5.5 (5.4 to 5.6; 3 %)          | 2.7 (2.6 to 2.7; 4 %)          |
| veh.cluster.VehicleStatus.warnings      | 3.3 (3.3 to 3.4; 2 %)       | 6.3 (6.3 to 6.5; 4 %)          | 0.5 (0.5 to 0.6; 2 %)          |
| veh.cluster.WheelHistory.wheelTicks     | 3.3 (3.3 to 3.4; 3 %)       | 6.1 (5.9 to 6.2; 5 %)          | 0.6 (0.6 to 0.6; 9 %)          |
| veh.cluster.WheelHistory.wheelTemp      | 2.8 (2.7 to 2.8; 2 %)       | 5.5 (5.5 to 5.6; 2 %)          | 2.8 (2.7 to 2.9; 4 %)          |
| veh.cluster.WheelHistory.wheelSlipRate  | 2.8 (2.7 to 2.8; 3 %)       | 5.5 (5.5 to 5.6; 2 %)          | 2.9 (2.9 to 3.0; 3 %)          |
| veh.cluster.WheelSummary.wheelHealth    | 3.3 (3.3 to 3.4; 5 %)       | 6.1 (6.0 to 7.3; 22 %) †       | 0.6 (0.6 to 0.6; 2 %)          |
| veh.cluster.wheels.ticks                | 3.3 (3.3 to 3.4; 3 %)       | 6.1 (6.0 to 6.2; 2 %)          | 0.5 (0.5 to 0.5; 3 %)          |
| veh.cluster.WheelDiagnostics.wheelTicks | not built                   | not built                      | not built                      |
| veh.hvac.cabin.cabinTemp                | 2.8 (2.8 to 2.8; 2 %)       | 5.5 (5.5 to 5.6; 2 %)          | 2.8 (2.7 to 3.0; 8 %)          |
| veh.hvac.cabin.targetTemp               | 2.8 (2.7 to 2.8; 4 %)       | 5.5 (5.5 to 5.6; 3 %)          | 2.7 (2.7 to 3.7; 38 %) †       |
| veh.hvac.cabin.blowerDuty               | 2.8 (2.8 to 2.9; 7 %)       | 5.5 (5.5 to 5.6; 2 %)          | 2.9 (2.9 to 3.0; 3 %)          |
| veh.hvac.cabin.fanLevel                 | 3.3 (3.3 to 3.4; 2 %)       | 6.1 (6.0 to 6.2; 2 %)          | 0.5 (0.5 to 0.5; 2 %)          |
| veh.hvac.cabin.compressorLoad           | 2.8 (2.7 to 2.9; 5 %)       | 5.5 (5.5 to 5.6; 2 %)          | 2.7 (2.7 to 2.7; 3 %)          |
| veh.cabin.Cabin.temperature             | 3.3 (3.3 to 3.4; 2 %)       | 6.3 (6.3 to 6.4; 2 %)          | 0.6 (0.6 to 0.6; 3 %)          |
| veh.cabin.Horn.active                   | 3.0 (3.0 to 3.1; 2 %)       | 7.2 (6.9 to 7.5; 8 %)          | 1.2 (1.2 to 1.2; 3 %)          |
| bench.stress.Stress.longString          | 60.3 (57.7 to 64.9; 12 %) † | 64.9 (63.3 to 70.7; 11 %) †    | 58.5 (55.5 to 63.6; 14 %) †    |
| bench.stress.Stress.itemList            | 902.3 (871.3 to 935.6; 7 %) | 1248.8 (1214.4 to 1297.8; 7 %) | 886.6 (860.8 to 925.7; 7 %)    |
| bench.stress.Stress.nestedOptional      | 85.2 (82.2 to 90.3; 10 %)   | 100.8 (96.5 to 105.9; 9 %)     | 73.5 (70.2 to 77.0; 9 %)       |
| bench.stress.Stress.map                 | 855.1 (836.7 to 887.2; 6 %) | 1213.1 (1191.3 to 1300.3; 9 %) | 860.1 (825.0 to 970.9; 17 %) † |

#### CPU load per operation

CPU load in percent of one core at 4000 updates per second of each operation
alone, derived from the median time of that operation. The three operations of
an encoding add up to its derived load of one update.

| Payload                                 | Encode: FlatBuffers | Encode: proto3 | Encode: podbuf | Verify: FlatBuffers | Verify: proto3 | Verify: podbuf | Decode: FlatBuffers | Decode: proto3 | Decode: podbuf |
| --------------------------------------- | ------------------- | -------------- | -------------- | ------------------- | -------------- | -------------- | ------------------- | -------------- | -------------- |
| veh.cluster.VehicleStatus.currentSpeed  | 0.0079              | 0.0037         | 0.0026         | 0.0055              | 0.0061         | 0.0054         | 0.0011              | 0.0022         | 0.0011         |
| veh.cluster.VehicleStatus.engineTemp    | 0.0079              | 0.0037         | 0.0026         | 0.0054              | 0.0062         | 0.0059         | 0.0011              | 0.0022         | 0.0011         |
| veh.cluster.VehicleStatus.warnings      | 0.0084              | 0.0026         | 0.0028         | 0.0025              | 0.0030         | 0.0010         | 0.0013              | 0.0025         | 0.0002         |
| veh.cluster.WheelHistory.wheelTicks     | 0.0084              | 0.0027         | 0.0028         | 0.0021              | 0.0030         | 0.0010         | 0.0013              | 0.0024         | 0.0002         |
| veh.cluster.WheelHistory.wheelTemp      | 0.0080              | 0.0037         | 0.0026         | 0.0054              | 0.0062         | 0.0059         | 0.0011              | 0.0022         | 0.0011         |
| veh.cluster.WheelHistory.wheelSlipRate  | 0.0080              | 0.0037         | 0.0026         | 0.0055              | 0.0062         | 0.0056         | 0.0011              | 0.0022         | 0.0012         |
| veh.cluster.WheelSummary.wheelHealth    | 0.0085              | 0.0027         | 0.0028         | 0.0021              | 0.0030         | 0.0010         | 0.0013              | 0.0024         | 0.0002         |
| veh.cluster.wheels.ticks                | 0.0078              | 0.0027         | 0.0028         | 0.0021              | 0.0030         | 0.0010         | 0.0013              | 0.0024         | 0.0002         |
| veh.cluster.WheelDiagnostics.wheelTicks | not built           | not built      | not built      | not built           | not built      | not built      | not built           | not built      | not built      |
| veh.hvac.cabin.cabinTemp                | 0.0079              | 0.0037         | 0.0026         | 0.0054              | 0.0062         | 0.0059         | 0.0011              | 0.0022         | 0.0011         |
| veh.hvac.cabin.targetTemp               | 0.0079              | 0.0037         | 0.0026         | 0.0054              | 0.0062         | 0.0059         | 0.0011              | 0.0022         | 0.0011         |
| veh.hvac.cabin.blowerDuty               | 0.0079              | 0.0037         | 0.0026         | 0.0055              | 0.0062         | 0.0056         | 0.0011              | 0.0022         | 0.0012         |
| veh.hvac.cabin.fanLevel                 | 0.0078              | 0.0027         | 0.0028         | 0.0021              | 0.0030         | 0.0010         | 0.0013              | 0.0024         | 0.0002         |
| veh.hvac.cabin.compressorLoad           | 0.0080              | 0.0037         | 0.0026         | 0.0055              | 0.0062         | 0.0056         | 0.0011              | 0.0022         | 0.0011         |
| veh.cabin.Cabin.temperature             | 0.0079              | 0.0028         | 0.0028         | 0.0022              | 0.0031         | 0.0010         | 0.0013              | 0.0025         | 0.0002         |
| veh.cabin.Horn.active                   | 0.0078              | 0.0027         | 0.0028         | 0.0037              | 0.0030         | 0.0010         | 0.0012              | 0.0029         | 0.0005         |
| bench.stress.Stress.longString          | 0.0157              | 0.0074         | 0.0138         | 0.0245              | 0.0262         | 0.0314         | 0.0241              | 0.0259         | 0.0234         |
| bench.stress.Stress.itemList            | 0.1735              | 0.1670         | 0.0833         | 0.1211              | 0.1983         | 0.1436         | 0.3609              | 0.4995         | 0.3546         |
| bench.stress.Stress.nestedOptional      | 0.0257              | 0.0160         | 0.0118         | 0.0158              | 0.0206         | 0.0154         | 0.0341              | 0.0403         | 0.0294         |
| bench.stress.Stress.map                 | 0.1730              | 0.3198         | 0.1351         | 0.7037              | 1.2519         | 0.1942         | 0.3420              | 0.4852         | 0.3440         |

#### Ratios per operation

Time of each operation of each encoding divided by the time of the same
operation in podbuf: above 1.00 the encoding is slower than podbuf in that
operation, below 1.00 faster. A refused or not-built cell has no ratio and is
marked. A time under about 10 ns includes harness cost and is an upper bound, so
a ratio whose podbuf time is under 10 ns is not comparable.

| Payload                                 | Encode: FlatBuffers / podbuf | Encode: proto3 / podbuf | Verify: FlatBuffers / podbuf | Verify: proto3 / podbuf | Decode: FlatBuffers / podbuf | Decode: proto3 / podbuf |
| --------------------------------------- | ---------------------------- | ----------------------- | ---------------------------- | ----------------------- | ---------------------------- | ----------------------- |
| veh.cluster.VehicleStatus.currentSpeed  | 2.99                         | 1.39                    | 1.02                         | 1.12                    | 0.99                         | 1.98                    |
| veh.cluster.VehicleStatus.engineTemp    | 2.99                         | 1.40                    | 0.93                         | 1.05                    | 1.02                         | 2.04                    |
| veh.cluster.VehicleStatus.warnings      | 3.05                         | 0.96                    | 2.55                         | 3.00                    | 6.09                         | 11.69                   |
| veh.cluster.WheelHistory.wheelTicks     | 3.06                         | 0.97                    | 2.16                         | 3.01                    | 5.60                         | 10.25                   |
| veh.cluster.WheelHistory.wheelTemp      | 3.03                         | 1.40                    | 0.93                         | 1.05                    | 0.99                         | 1.98                    |
| veh.cluster.WheelHistory.wheelSlipRate  | 3.01                         | 1.40                    | 0.99                         | 1.11                    | 0.94                         | 1.88                    |
| veh.cluster.WheelSummary.wheelHealth    | 3.10                         | 0.97                    | 2.13                         | 3.01                    | 5.62                         | 10.29                   |
| veh.cluster.wheels.ticks                | 2.83                         | 0.98                    | 2.11                         | 3.02                    | 6.39                         | 11.72                   |
| veh.cluster.WheelDiagnostics.wheelTicks | not built                    | not built               | not built                    | not built               | not built                    | not built               |
| veh.hvac.cabin.cabinTemp                | 3.01                         | 1.40                    | 0.92                         | 1.05                    | 1.00                         | 1.99                    |
| veh.hvac.cabin.targetTemp               | 2.99                         | 1.39                    | 0.93                         | 1.05                    | 1.02                         | 2.04                    |
| veh.hvac.cabin.blowerDuty               | 2.98                         | 1.39                    | 0.99                         | 1.11                    | 0.94                         | 1.87                    |
| veh.hvac.cabin.fanLevel                 | 2.84                         | 0.98                    | 2.11                         | 3.01                    | 6.37                         | 11.68                   |
| veh.hvac.cabin.compressorLoad           | 3.01                         | 1.40                    | 0.99                         | 1.11                    | 1.02                         | 2.04                    |
| veh.cabin.Cabin.temperature             | 2.88                         | 1.00                    | 2.23                         | 3.11                    | 5.44                         | 10.43                   |
| veh.cabin.Horn.active                   | 2.80                         | 0.96                    | 3.68                         | 3.00                    | 2.54                         | 6.00                    |
| bench.stress.Stress.longString          | 1.14                         | 0.53                    | 0.78                         | 0.83                    | 1.03                         | 1.11                    |
| bench.stress.Stress.itemList            | 2.08                         | 2.00                    | 0.84                         | 1.38                    | 1.02                         | 1.41                    |
| bench.stress.Stress.nestedOptional      | 2.18                         | 1.35                    | 1.03                         | 1.33                    | 1.16                         | 1.37                    |
| bench.stress.Stress.map                 | 1.28                         | 2.37                    | 3.62                         | 6.45                    | 0.99                         | 1.41                    |

#### CPU load, paced and derived

CPU load in percent of one core at 4000 updates per second: paced (process CPU
time over wall time, includes the cost of sleep and wake-up) next to the derived
figure, and their difference.

| Payload                                | Encoding    | Paced % | Derived % | Difference |
| -------------------------------------- | ----------- | ------- | --------- | ---------- |
| veh.cluster.VehicleStatus.currentSpeed | FlatBuffers | 0.7294  | 0.0145    | 0.7149     |
| veh.cluster.VehicleStatus.currentSpeed | proto3      | 3.4135  | 0.0119    | 3.4015     |
| veh.cluster.VehicleStatus.currentSpeed | podbuf      | 4.5632  | 0.0092    | 4.5540     |
| veh.cluster.VehicleStatus.engineTemp   | FlatBuffers | 0.7192  | 0.0145    | 0.7048     |
| veh.cluster.VehicleStatus.engineTemp   | proto3      | 2.6525  | 0.0121    | 2.6404     |
| veh.cluster.VehicleStatus.engineTemp   | podbuf      | 5.1711  | 0.0096    | 5.1615     |
| veh.cluster.VehicleStatus.warnings     | FlatBuffers | 0.6868  | 0.0123    | 0.6746     |
| veh.cluster.VehicleStatus.warnings     | proto3      | 5.5160  | 0.0082    | 5.5079     |
| veh.cluster.VehicleStatus.warnings     | podbuf      | 5.8972  | 0.0040    | 5.8933     |
| veh.cluster.WheelHistory.wheelTicks    | FlatBuffers | 0.7438  | 0.0119    | 0.7320     |
| veh.cluster.WheelHistory.wheelTicks    | proto3      | 3.5011  | 0.0081    | 3.4930     |
| veh.cluster.WheelHistory.wheelTicks    | podbuf      | 4.0004  | 0.0040    | 3.9965     |
| veh.cluster.WheelHistory.wheelTemp     | FlatBuffers | 0.8543  | 0.0145    | 0.8397     |
| veh.cluster.WheelHistory.wheelTemp     | proto3      | 4.9738  | 0.0121    | 4.9617     |
| veh.cluster.WheelHistory.wheelTemp     | podbuf      | 5.1956  | 0.0096    | 5.1860     |
| veh.cluster.WheelHistory.wheelSlipRate | FlatBuffers | 1.2037  | 0.0146    | 1.1892     |
| veh.cluster.WheelHistory.wheelSlipRate | proto3      | 2.5334  | 0.0121    | 2.5213     |
| veh.cluster.WheelHistory.wheelSlipRate | podbuf      | 5.6919  | 0.0094    | 5.6825     |
| veh.cluster.WheelSummary.wheelHealth   | FlatBuffers | 0.6786  | 0.0120    | 0.6667     |
| veh.cluster.WheelSummary.wheelHealth   | proto3      | 3.1883  | 0.0081    | 3.1802     |
| veh.cluster.WheelSummary.wheelHealth   | podbuf      | 4.3485  | 0.0040    | 4.3445     |
| veh.cluster.wheels.ticks               | FlatBuffers | 0.9257  | 0.0112    | 0.9145     |
| veh.cluster.wheels.ticks               | proto3      | 4.3358  | 0.0081    | 4.3277     |
| veh.cluster.wheels.ticks               | podbuf      | 3.0351  | 0.0040    | 3.0311     |
| veh.hvac.cabin.cabinTemp               | FlatBuffers | 0.7869  | 0.0145    | 0.7724     |
| veh.hvac.cabin.cabinTemp               | proto3      | 4.1269  | 0.0121    | 4.1148     |
| veh.hvac.cabin.cabinTemp               | podbuf      | 5.1725  | 0.0096    | 5.1629     |
| veh.hvac.cabin.targetTemp              | FlatBuffers | 0.6922  | 0.0145    | 0.6778     |
| veh.hvac.cabin.targetTemp              | proto3      | 3.7435  | 0.0121    | 3.7315     |
| veh.hvac.cabin.targetTemp              | podbuf      | 5.7001  | 0.0096    | 5.6905     |
| veh.hvac.cabin.blowerDuty              | FlatBuffers | 0.8877  | 0.0145    | 0.8732     |
| veh.hvac.cabin.blowerDuty              | proto3      | 4.4744  | 0.0121    | 4.4623     |
| veh.hvac.cabin.blowerDuty              | podbuf      | 4.8693  | 0.0094    | 4.8599     |
| veh.hvac.cabin.fanLevel                | FlatBuffers | 0.7056  | 0.0112    | 0.6944     |
| veh.hvac.cabin.fanLevel                | proto3      | 4.7281  | 0.0081    | 4.7200     |
| veh.hvac.cabin.fanLevel                | podbuf      | 3.5302  | 0.0040    | 3.5263     |
| veh.hvac.cabin.compressorLoad          | FlatBuffers | 0.7006  | 0.0146    | 0.6860     |
| veh.hvac.cabin.compressorLoad          | proto3      | 0.6898  | 0.0121    | 0.6777     |
| veh.hvac.cabin.compressorLoad          | podbuf      | 2.6515  | 0.0093    | 2.6422     |
| veh.cabin.Cabin.temperature            | FlatBuffers | 0.8933  | 0.0115    | 0.8818     |
| veh.cabin.Cabin.temperature            | proto3      | 3.2807  | 0.0084    | 3.2723     |
| veh.cabin.Cabin.temperature            | podbuf      | 5.8331  | 0.0040    | 5.8291     |
| veh.cabin.Horn.active                  | FlatBuffers | 0.6874  | 0.0127    | 0.6748     |
| veh.cabin.Horn.active                  | proto3      | 4.4828  | 0.0086    | 4.4743     |
| veh.cabin.Horn.active                  | podbuf      | 4.6028  | 0.0043    | 4.5986     |
| bench.stress.Stress.longString         | FlatBuffers | 1.0207  | 0.0643    | 0.9564     |
| bench.stress.Stress.longString         | proto3      | 5.1095  | 0.0595    | 5.0501     |
| bench.stress.Stress.longString         | podbuf      | 1.3474  | 0.0686    | 1.2788     |
| bench.stress.Stress.itemList           | FlatBuffers | 1.4173  | 0.6555    | 0.7618     |
| bench.stress.Stress.itemList           | proto3      | 5.1889  | 0.8649    | 4.3241     |
| bench.stress.Stress.itemList           | podbuf      | 4.2464  | 0.5815    | 3.6648     |
| bench.stress.Stress.nestedOptional     | FlatBuffers | 0.8715  | 0.0756    | 0.7958     |
| bench.stress.Stress.nestedOptional     | proto3      | 4.5814  | 0.0769    | 4.5045     |
| bench.stress.Stress.nestedOptional     | podbuf      | 4.4643  | 0.0566    | 4.4077     |
| bench.stress.Stress.map                | FlatBuffers | 2.1383  | 1.2188    | 0.9195     |
| bench.stress.Stress.map                | proto3      | 6.7531  | 2.0569    | 4.6962     |
| bench.stress.Stress.map                | podbuf      | 6.5418  | 0.6734    | 5.8683     |

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
core.** Date 2026-11-05, commit 56ddf829, rustc 1.98.1 (48a229cea 2026-09-01), 4
000 updates per second, 30 samples per cell, 2 s of CPU time per operation, cell
and encoding, paced run included. 57 cells were measured and 3 were not built
(the one payload `RawTickCount`, for each encoding), in each run. No cell was
refused. The commands were `just bench-pixel`, then `cargo xtask bench report`.

### What the device data show, and what they do not

- **Frequency.** The phone is not rooted, the governor is `schedutil` and the
  frequency is not fixed. The sidecar files read the frequency of each core
  before and after a run; the two reads do not show the frequency during the
  run. The A55 run read cpu2 at 1 363 200 kHz before and 576 000 kHz after. The
  A76 run read cpu6 at 2 208 000 kHz before and 1 728 000 kHz after. The result
  files record the frequency of the core at the start of the program: 1 804 800
  kHz in the A55 run and 1 478 400 kHz in the A76 run. These start-of-program
  values differ from the sidecar reads of the pinned core, and this note asserts
  no cause. The frequency during the runs is therefore not known; it may have
  varied.
- **Stability.** No cell carries the spread marker, for the update and for each
  operation. The largest `(max - min) / median` of any operation cell is 2.6 %
  in the A55 run and 4.3 % in the A76 run, and every cell has 30 samples. The
  tables therefore show no instability in the samples. They do not show that the
  frequency was constant, because the samples of one cell are taken over a short
  time.
- **Load.** The codec-only load at 4 000 updates per second is a small fraction
  of one core for the signal payloads: 0.044 % to 0.180 % on the A55 core and
  0.015 % to 0.064 % on the A76 core. The largest load of any cell is the proto3
  encoding of the map stress payload: 15.95 % of the A55 core and 5.17 % of the
  A76 core. The largest load among the signal payloads is FlatBuffers on
  `veh.cluster.WheelHistory.wheelSlipRate` on the A55 core (0.180 %, 449.9 ns
  per update) and FlatBuffers on `veh.cluster.VehicleStatus.currentSpeed` on the
  A76 core (0.064 %, 160.9 ns per update).
- **Paced against derived.** The paced figure is above the derived figure in
  every cell of both runs. For the signal payloads the paced figure is 8.9 % to
  11.8 % on the A55 core and 1.9 % to 4.2 % on the A76 core, where the derived
  figure is 0.044 % to 0.180 % and 0.015 % to 0.064 %. The paced runs measure
  process CPU time over a run that sleeps between updates; the tables do not
  show what part of that time is the codec, and this note does not assert a
  cause. The derived figure is the one that the codec determines.
- **Comparison of the encodings, per update.** On the 15 measured signal
  payloads podbuf has the lowest median time per update on both cores. The same
  holds on three of the four stress payloads (list of structs, nested optionals,
  map). It does not hold on the long string, where podbuf has the highest time
  on both cores. proto3 is the smallest in size on every signal payload. There
  is no ordering of the encodings that holds on every payload, so this note
  gives no verdict between them. Which one to use depends on the weight of size
  against CPU time and on the payloads of the system, which the bench does not
  decide.
- **The burst is not covered.** A burst of 200 messages in 10 ms was not run.
  The time per update in the tables lets a reader compute a burst of
  back-to-back updates. Worked example, derived and not measured: the heaviest
  signal payload on the A55 core is FlatBuffers on
  `veh.cluster.WheelHistory.wheelSlipRate`, 449.9 ns per update; 200 updates
  cost 200 x 449.9 ns = 90.0 us of CPU time, which is 0.90 % of the 10 ms
  window. This assumes that every update costs what the warm, repeated update of
  the bench costs; it does not include the transport, queueing, cold caches or a
  lower core frequency.

### What the split into operations shows, and what it does not

The numbers are those of the per-operation tables below.
`veh.cluster.VehicleStatus.currentSpeed` is used as an example signal: a figure
given for `currentSpeed` is that payload's own value, not a statistic. A figure
given "over the signals" is the median over the 15 signals of each signal's
median.

- **Where the time goes.** On the A55 core `currentSpeed` costs 441.1 ns per
  update for FlatBuffers (encode 180.5, verify 220.2, decode 40.5 ns), 376.5 ns
  for proto3 (76.0, 236.2, 64.3) and 333.5 ns for podbuf (75.7, 220.7, 37.2).
  Verify is the largest share for proto3 (63 %) and podbuf (66 %) and 50 % for
  FlatBuffers. On the A76 core `currentSpeed` costs 160.9 ns for FlatBuffers
  (81.2, 65.4, 14.3), 117.3 ns for proto3 (26.4, 66.8, 24.0) and 106.2 ns for
  podbuf (29.0, 63.8, 13.3); there verify is the largest share for proto3 (57 %)
  and podbuf (60 %), and encode is the largest operation for FlatBuffers (50 %).
- **Verify.** Over the signals, verify takes 220.2 ns (FlatBuffers), 236.2 ns
  (proto3) and 220.7 ns (podbuf) on the A55 core, and 65.4, 66.8 and 63.8 ns on
  the A76 core. The verify time of one encoding varies a lot between payloads:
  podbuf verify is 30.5 ns on the cheapest signal and 230.7 ns on the dearest on
  the A55 core. The data do not show what makes a payload cheap or dear to
  verify.
- **Encode.** FlatBuffers encode is 2.39 to 2.60 times podbuf encode on the 15
  signals on the A55 core and 2.39 to 2.91 times on the A76 core. proto3 encode
  is 1.00 to 1.17 times podbuf encode on the A55 core and 0.87 to 1.05 times on
  the A76 core. On the long string the order differs: proto3 encode is 233 ns
  (A55), against 920 ns for FlatBuffers and 999 ns for podbuf.
- **Decode.** Over the signals, decode takes 40.5 ns (FlatBuffers), 64.3 ns
  (proto3) and 37.2 ns (podbuf) on the A55 core, and 14.3, 24.0 and 13.2 ns on
  the A76 core. proto3 decode is 1.73 to 10.08 times podbuf decode on the A55
  core and 1.80 to 13.49 times on the A76 core. The large ratios come from the
  signals where the podbuf decode is 7.2 ns (A55) or 2.0 ns (A76), below the 10
  ns under which a decode time is an upper bound and not comparable. The codec
  record says that proto3 decode reads the message again from the bytes; these
  tables do not show whether that is the reason for the ratio.
- **Stress payloads.** On the map, verify is the largest operation for all three
  encodings on both cores: on the A55 core 13.9 us for FlatBuffers, 27.3 us for
  proto3 and 4.6 us for podbuf. On the list of structs FlatBuffers has the
  lowest verify time (2.5 us on the A55 core, against 4.3 us for proto3 and 3.0
  us for podbuf).
- **Allocations per operation.** Verify made no allocation for any payload. The
  allocations of the stress payloads are in encode and decode as the host
  observations state.
- **No verdict.** No ordering of the encodings holds for every operation on
  every payload: for example podbuf encode is the slowest of the three on the
  long string, while it is faster than FlatBuffers encode on every signal. The
  tables give the data per operation, and this note draws no conclusion beyond
  the lines above.

### Cortex-A55 (cpu2)

#### Mix

Equal-weight mix, per signal: each signal counts once, so a payload type that
several signals share counts once for each of them. Only signals whose three
encodings are measured are counted; the last column gives how many.

| Metric                                                 | FlatBuffers | proto3 | podbuf | FlatBuffers / podbuf | proto3 / podbuf | Units |
| ------------------------------------------------------ | ----------- | ------ | ------ | -------------------- | --------------- | ----- |
| Size (bytes)                                           | 24.5        | 3.6    | 16.0   | 1.53                 | 0.23            | 15    |
| Heap allocations per update (encode + verify + decode) | 0.0         | 0.0    | 0.0    | n/a                  | n/a             | 15    |
| Time per update (median ns)                            | 361.9       | 313.2  | 233.9  | 1.55                 | 1.34            | 15    |
| CPU % of one core at 4000 updates per second (derived) | 0.1448      | 0.1253 | 0.0935 | 1.55                 | 1.34            | 15    |

Equal-weight mix, per payload type: each payload type counts once, however many
signals share it (the value of a type is the mean over its signals). Only
payload types whose signals are measured in all three encodings are counted; the
last column gives how many.

| Metric                                                 | FlatBuffers | proto3 | podbuf | FlatBuffers / podbuf | proto3 / podbuf | Units |
| ------------------------------------------------------ | ----------- | ------ | ------ | -------------------- | --------------- | ----- |
| Size (bytes)                                           | 25.3        | 3.4    | 16.0   | 1.58                 | 0.21            | 6     |
| Heap allocations per update (encode + verify + decode) | 0.0         | 0.0    | 0.0    | n/a                  | n/a             | 6     |
| Time per update (median ns)                            | 351.0       | 301.1  | 218.8  | 1.60                 | 1.38            | 6     |
| CPU % of one core at 4000 updates per second (derived) | 0.1404      | 0.1204 | 0.0875 | 1.60                 | 1.38            | 6     |

Equal-weight mix, per signal: each signal counts once, so a payload type that
several signals share counts once for each of them. Only signals whose three
encodings are measured are counted; the last column gives how many. The rows are
per operation. A time under about 10 ns includes harness cost and is an upper
bound, so a ratio whose podbuf time is under 10 ns is not comparable.

| Metric                                                         | FlatBuffers | proto3 | podbuf | FlatBuffers / podbuf | proto3 / podbuf | Units |
| -------------------------------------------------------------- | ----------- | ------ | ------ | -------------------- | --------------- | ----- |
| Encode: time per operation (median ns)                         | 179.5       | 79.6   | 74.1   | 2.42                 | 1.07            | 15    |
| Encode: CPU % of one core at 4000 updates per second (derived) | 0.0718      | 0.0318 | 0.0296 | 2.42                 | 1.07            | 15    |
| Encode: heap allocations                                       | 0.0         | 0.0    | 0.0    | n/a                  | n/a             | 15    |
| Verify: time per operation (median ns)                         | 143.9       | 166.4  | 135.9  | 1.06                 | 1.22            | 15    |
| Verify: CPU % of one core at 4000 updates per second (derived) | 0.0576      | 0.0666 | 0.0544 | 1.06                 | 1.22            | 15    |
| Verify: heap allocations                                       | 0.0         | 0.0    | 0.0    | n/a                  | n/a             | 15    |
| Decode: time per operation (median ns)                         | 38.5        | 67.2   | 23.8   | 1.61                 | 2.82            | 15    |
| Decode: CPU % of one core at 4000 updates per second (derived) | 0.0154      | 0.0269 | 0.0095 | 1.61                 | 2.82            | 15    |
| Decode: heap allocations                                       | 0.0         | 0.0    | 0.0    | n/a                  | n/a             | 15    |

Equal-weight mix, per payload type: each payload type counts once, however many
signals share it (the value of a type is the mean over its signals). Only
payload types whose signals are measured in all three encodings are counted; the
last column gives how many. The rows are per operation. A time under about 10 ns
includes harness cost and is an upper bound, so a ratio whose podbuf time is
under 10 ns is not comparable.

| Metric                                                         | FlatBuffers | proto3 | podbuf | FlatBuffers / podbuf | proto3 / podbuf | Units |
| -------------------------------------------------------------- | ----------- | ------ | ------ | -------------------- | --------------- | ----- |
| Encode: time per operation (median ns)                         | 180.7       | 79.0   | 74.1   | 2.44                 | 1.07            | 6     |
| Encode: CPU % of one core at 4000 updates per second (derived) | 0.0723      | 0.0316 | 0.0296 | 2.44                 | 1.07            | 6     |
| Encode: heap allocations                                       | 0.0         | 0.0    | 0.0    | n/a                  | n/a             | 6     |
| Verify: time per operation (median ns)                         | 131.7       | 154.3  | 121.9  | 1.08                 | 1.27            | 6     |
| Verify: CPU % of one core at 4000 updates per second (derived) | 0.0527      | 0.0617 | 0.0488 | 1.08                 | 1.27            | 6     |
| Verify: heap allocations                                       | 0.0         | 0.0    | 0.0    | n/a                  | n/a             | 6     |
| Decode: time per operation (median ns)                         | 38.6        | 67.8   | 22.8   | 1.69                 | 2.97            | 6     |
| Decode: CPU % of one core at 4000 updates per second (derived) | 0.0155      | 0.0271 | 0.0091 | 1.69                 | 2.97            | 6     |
| Decode: heap allocations                                       | 0.0         | 0.0    | 0.0    | n/a                  | n/a             | 6     |

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

CPU time of one update (encode, verify and decode), in ns: the sum of the
medians of the three operations, with the sum of their fastest and the sum of
their slowest sample (an envelope, not a percentile) and its range as a share of
the sum. † spread above 10 %: the figure is not stable.

| Payload                                 | FlatBuffers                       | proto3                            | podbuf                            |
| --------------------------------------- | --------------------------------- | --------------------------------- | --------------------------------- |
| veh.cluster.VehicleStatus.currentSpeed  | 441.1 (440.8 to 441.5; 0 %)       | 376.5 (376.5 to 376.7; 0 %)       | 333.5 (333.2 to 334.3; 0 %)       |
| veh.cluster.VehicleStatus.engineTemp    | 447.0 (446.4 to 448.2; 0 %)       | 385.9 (385.8 to 386.4; 0 %)       | 340.0 (339.7 to 340.8; 0 %)       |
| veh.cluster.VehicleStatus.warnings      | 263.1 (262.7 to 263.7; 0 %)       | 231.7 (231.6 to 231.8; 0 %)       | 112.0 (111.5 to 112.5; 1 %)       |
| veh.cluster.WheelHistory.wheelTicks     | 260.9 (260.6 to 261.4; 0 %)       | 231.3 (231.1 to 231.6; 0 %)       | 110.4 (110.2 to 111.2; 1 %)       |
| veh.cluster.WheelHistory.wheelTemp      | 447.2 (446.6 to 448.7; 0 %)       | 385.9 (385.8 to 386.1; 0 %)       | 340.1 (339.8 to 340.6; 0 %)       |
| veh.cluster.WheelHistory.wheelSlipRate  | 449.9 (449.2 to 451.1; 0 %)       | 386.5 (386.3 to 386.8; 0 %)       | 342.9 (342.5 to 343.9; 0 %)       |
| veh.cluster.WheelSummary.wheelHealth    | 260.9 (260.3 to 261.7; 1 %)       | 231.3 (231.1 to 231.6; 0 %)       | 110.4 (110.1 to 111.0; 1 %)       |
| veh.cluster.wheels.ticks                | 261.0 (260.3 to 262.0; 1 %)       | 231.3 (231.1 to 231.6; 0 %)       | 110.3 (110.2 to 110.8; 1 %)       |
| veh.cluster.WheelDiagnostics.wheelTicks | not built                         | not built                         | not built                         |
| veh.hvac.cabin.cabinTemp                | 446.9 (446.4 to 447.4; 0 %)       | 385.9 (385.8 to 386.1; 0 %)       | 340.0 (339.8 to 340.8; 0 %)       |
| veh.hvac.cabin.targetTemp               | 447.0 (446.4 to 448.0; 0 %)       | 385.9 (385.8 to 386.1; 0 %)       | 340.0 (339.7 to 340.6; 0 %)       |
| veh.hvac.cabin.blowerDuty               | 449.8 (449.3 to 450.8; 0 %)       | 386.5 (386.3 to 386.6; 0 %)       | 342.8 (342.5 to 343.9; 0 %)       |
| veh.hvac.cabin.fanLevel                 | 261.0 (260.5 to 261.8; 0 %)       | 231.3 (231.1 to 231.6; 0 %)       | 110.4 (110.2 to 110.7; 0 %)       |
| veh.hvac.cabin.compressorLoad           | 449.8 (449.1 to 450.5; 0 %)       | 386.5 (386.4 to 386.6; 0 %)       | 342.7 (342.5 to 343.5; 0 %)       |
| veh.cabin.Cabin.temperature             | 261.8 (261.3 to 262.8; 1 %)       | 236.8 (236.8 to 237.0; 0 %)       | 112.4 (112.0 to 112.8; 1 %)       |
| veh.cabin.Horn.active                   | 281.2 (278.9 to 284.0; 2 %)       | 224.5 (224.4 to 224.7; 0 %)       | 119.8 (119.2 to 120.8; 1 %)       |
| bench.stress.Stress.longString          | 2055.7 (2054.2 to 2059.5; 0 %)    | 1458.6 (1454.7 to 1462.3; 1 %)    | 2190.3 (2188.9 to 2192.7; 0 %)    |
| bench.stress.Stress.itemList            | 10318.7 (10305.2 to 10328.8; 0 %) | 13989.9 (13979.6 to 14007.8; 0 %) | 8943.0 (8940.7 to 8952.1; 0 %)    |
| bench.stress.Stress.nestedOptional      | 1439.5 (1432.3 to 1448.1; 1 %)    | 1597.8 (1594.1 to 1602.9; 1 %)    | 1108.1 (1104.7 to 1112.5; 1 %)    |
| bench.stress.Stress.map                 | 21421.6 (21405.9 to 21448.9; 0 %) | 39868.9 (39848.6 to 39891.0; 0 %) | 11268.0 (11256.4 to 11284.6; 0 %) |

##### CPU load, derived

CPU load in percent of one core at 4000 updates per second, derived from the
median time of one update (the headline figure).

| Payload                                 | FlatBuffers | proto3    | podbuf    |
| --------------------------------------- | ----------- | --------- | --------- |
| veh.cluster.VehicleStatus.currentSpeed  | 0.1764      | 0.1506    | 0.1334    |
| veh.cluster.VehicleStatus.engineTemp    | 0.1788      | 0.1544    | 0.1360    |
| veh.cluster.VehicleStatus.warnings      | 0.1052      | 0.0927    | 0.0448    |
| veh.cluster.WheelHistory.wheelTicks     | 0.1044      | 0.0925    | 0.0441    |
| veh.cluster.WheelHistory.wheelTemp      | 0.1789      | 0.1544    | 0.1360    |
| veh.cluster.WheelHistory.wheelSlipRate  | 0.1800      | 0.1546    | 0.1372    |
| veh.cluster.WheelSummary.wheelHealth    | 0.1044      | 0.0925    | 0.0442    |
| veh.cluster.wheels.ticks                | 0.1044      | 0.0925    | 0.0441    |
| veh.cluster.WheelDiagnostics.wheelTicks | not built   | not built | not built |
| veh.hvac.cabin.cabinTemp                | 0.1787      | 0.1544    | 0.1360    |
| veh.hvac.cabin.targetTemp               | 0.1788      | 0.1544    | 0.1360    |
| veh.hvac.cabin.blowerDuty               | 0.1799      | 0.1546    | 0.1371    |
| veh.hvac.cabin.fanLevel                 | 0.1044      | 0.0925    | 0.0442    |
| veh.hvac.cabin.compressorLoad           | 0.1799      | 0.1546    | 0.1371    |
| veh.cabin.Cabin.temperature             | 0.1047      | 0.0947    | 0.0450    |
| veh.cabin.Horn.active                   | 0.1125      | 0.0898    | 0.0479    |
| bench.stress.Stress.longString          | 0.8223      | 0.5834    | 0.8761    |
| bench.stress.Stress.itemList            | 4.1275      | 5.5960    | 3.5772    |
| bench.stress.Stress.nestedOptional      | 0.5758      | 0.6391    | 0.4432    |
| bench.stress.Stress.map                 | 8.5686      | 15.9476   | 4.5072    |

##### Ratios to podbuf

Each encoding divided by podbuf: above 1.00 the encoding is larger or slower
than podbuf, below 1.00 smaller or faster. The CPU ratios equal the time ratios,
because the derived CPU load is proportional to the median time. A refused or
not-built cell has no ratio and is marked.

| Payload                                 | Size: FlatBuffers / podbuf | Size: proto3 / podbuf | Time: FlatBuffers / podbuf | Time: proto3 / podbuf | CPU: FlatBuffers / podbuf | CPU: proto3 / podbuf |
| --------------------------------------- | -------------------------- | --------------------- | -------------------------- | --------------------- | ------------------------- | -------------------- |
| veh.cluster.VehicleStatus.currentSpeed  | 1.50                       | 0.31                  | 1.32                       | 1.13                  | 1.32                      | 1.13                 |
| veh.cluster.VehicleStatus.engineTemp    | 1.50                       | 0.31                  | 1.31                       | 1.13                  | 1.31                      | 1.13                 |
| veh.cluster.VehicleStatus.warnings      | 1.50                       | 0.12                  | 2.35                       | 2.07                  | 2.35                      | 2.07                 |
| veh.cluster.WheelHistory.wheelTicks     | 1.50                       | 0.12                  | 2.36                       | 2.10                  | 2.36                      | 2.10                 |
| veh.cluster.WheelHistory.wheelTemp      | 1.50                       | 0.31                  | 1.31                       | 1.13                  | 1.31                      | 1.13                 |
| veh.cluster.WheelHistory.wheelSlipRate  | 1.50                       | 0.31                  | 1.31                       | 1.13                  | 1.31                      | 1.13                 |
| veh.cluster.WheelSummary.wheelHealth    | 1.50                       | 0.12                  | 2.36                       | 2.09                  | 2.36                      | 2.09                 |
| veh.cluster.wheels.ticks                | 1.50                       | 0.12                  | 2.37                       | 2.10                  | 2.37                      | 2.10                 |
| veh.cluster.WheelDiagnostics.wheelTicks | not built                  | not built             | not built                  | not built             | not built                 | not built            |
| veh.hvac.cabin.cabinTemp                | 1.50                       | 0.31                  | 1.31                       | 1.14                  | 1.31                      | 1.14                 |
| veh.hvac.cabin.targetTemp               | 1.50                       | 0.31                  | 1.31                       | 1.14                  | 1.31                      | 1.14                 |
| veh.hvac.cabin.blowerDuty               | 1.50                       | 0.31                  | 1.31                       | 1.13                  | 1.31                      | 1.13                 |
| veh.hvac.cabin.fanLevel                 | 1.50                       | 0.12                  | 2.36                       | 2.10                  | 2.36                      | 2.10                 |
| veh.hvac.cabin.compressorLoad           | 1.50                       | 0.31                  | 1.31                       | 1.13                  | 1.31                      | 1.13                 |
| veh.cabin.Cabin.temperature             | 1.50                       | 0.12                  | 2.33                       | 2.11                  | 2.33                      | 2.11                 |
| veh.cabin.Horn.active                   | 2.00                       | 0.12                  | 2.35                       | 1.87                  | 2.35                      | 1.87                 |
| bench.stress.Stress.longString          | 1.03                       | 0.97                  | 0.94                       | 0.67                  | 0.94                      | 0.67                 |
| bench.stress.Stress.itemList            | 1.50                       | 0.55                  | 1.15                       | 1.56                  | 1.15                      | 1.56                 |
| bench.stress.Stress.nestedOptional      | 1.63                       | 0.35                  | 1.30                       | 1.44                  | 1.30                      | 1.44                 |
| bench.stress.Stress.map                 | 1.56                       | 0.53                  | 1.90                       | 3.54                  | 1.90                      | 3.54                 |

##### Time per operation, encode

CPU time of the encode operation of one update, in ns: the median of 30 samples
of a batch of this operation alone, with the fastest and the slowest sample and
the range as a share of the median. † spread above 10 %: the figure is not
stable.

| Payload                                 | FlatBuffers                    | proto3                         | podbuf                         |
| --------------------------------------- | ------------------------------ | ------------------------------ | ------------------------------ |
| veh.cluster.VehicleStatus.currentSpeed  | 180.5 (180.2 to 180.8; 0 %)    | 76.0 (76.0 to 76.0; 0 %)       | 75.7 (75.4 to 76.3; 1 %)       |
| veh.cluster.VehicleStatus.engineTemp    | 179.7 (179.1 to 180.8; 1 %)    | 76.5 (76.4 to 76.5; 0 %)       | 75.0 (74.7 to 75.7; 1 %)       |
| veh.cluster.VehicleStatus.warnings      | 177.2 (176.9 to 177.8; 0 %)    | 83.7 (83.7 to 83.8; 0 %)       | 73.1 (72.7 to 73.5; 1 %)       |
| veh.cluster.WheelHistory.wheelTicks     | 177.2 (176.8 to 177.6; 0 %)    | 84.4 (84.2 to 84.5; 0 %)       | 72.7 (72.5 to 73.5; 1 %)       |
| veh.cluster.WheelHistory.wheelTemp      | 179.9 (179.3 to 181.3; 1 %)    | 76.5 (76.4 to 76.5; 0 %)       | 75.0 (74.7 to 75.4; 1 %)       |
| veh.cluster.WheelHistory.wheelSlipRate  | 179.9 (179.1 to 180.9; 1 %)    | 76.5 (76.4 to 76.5; 0 %)       | 75.1 (74.7 to 76.0; 2 %)       |
| veh.cluster.WheelSummary.wheelHealth    | 177.2 (176.6 to 177.9; 1 %)    | 84.4 (84.2 to 84.6; 0 %)       | 72.7 (72.4 to 73.3; 1 %)       |
| veh.cluster.wheels.ticks                | 177.3 (176.5 to 178.2; 1 %)    | 84.3 (84.1 to 84.6; 1 %)       | 72.6 (72.5 to 73.1; 1 %)       |
| veh.cluster.WheelDiagnostics.wheelTicks | not built                      | not built                      | not built                      |
| veh.hvac.cabin.cabinTemp                | 179.6 (179.2 to 180.0; 0 %)    | 76.5 (76.4 to 76.6; 0 %)       | 74.9 (74.7 to 75.7; 1 %)       |
| veh.hvac.cabin.targetTemp               | 179.7 (179.1 to 180.6; 1 %)    | 76.5 (76.4 to 76.5; 0 %)       | 74.9 (74.7 to 75.4; 1 %)       |
| veh.hvac.cabin.blowerDuty               | 179.7 (179.2 to 180.5; 1 %)    | 76.5 (76.4 to 76.5; 0 %)       | 74.9 (74.7 to 75.7; 1 %)       |
| veh.hvac.cabin.fanLevel                 | 177.3 (176.8 to 178.0; 1 %)    | 84.4 (84.2 to 84.6; 0 %)       | 72.7 (72.5 to 73.0; 1 %)       |
| veh.hvac.cabin.compressorLoad           | 179.7 (179.0 to 180.4; 1 %)    | 76.5 (76.4 to 76.5; 0 %)       | 74.9 (74.7 to 75.4; 1 %)       |
| veh.cabin.Cabin.temperature             | 178.1 (177.6 to 178.9; 1 %)    | 86.5 (86.5 to 86.6; 0 %)       | 74.2 (73.8 to 74.5; 1 %)       |
| veh.cabin.Horn.active                   | 190.3 (188.0 to 193.0; 3 %)    | 74.9 (74.9 to 74.9; 0 %)       | 73.2 (72.6 to 74.1; 2 %)       |
| bench.stress.Stress.longString          | 920.5 (919.8 to 922.7; 0 %)    | 232.8 (231.3 to 234.0; 1 %)    | 998.8 (998.4 to 1000.0; 0 %)   |
| bench.stress.Stress.itemList            | 3743.8 (3736.5 to 3748.1; 0 %) | 3377.6 (3376.7 to 3379.5; 0 %) | 2009.3 (2008.2 to 2012.3; 0 %) |
| bench.stress.Stress.nestedOptional      | 613.7 (610.1 to 617.4; 1 %)    | 430.6 (430.0 to 431.5; 0 %)    | 273.6 (273.3 to 274.1; 0 %)    |
| bench.stress.Stress.map                 | 3744.3 (3734.7 to 3760.6; 1 %) | 6520.2 (6516.7 to 6524.3; 0 %) | 2845.2 (2843.1 to 2847.5; 0 %) |

##### Time per operation, verify

CPU time of the verify operation of one update, in ns: the median of 30 samples
of a batch of this operation alone, with the fastest and the slowest sample and
the range as a share of the median. † spread above 10 %: the figure is not
stable.

| Payload                                 | FlatBuffers                       | proto3                            | podbuf                         |
| --------------------------------------- | --------------------------------- | --------------------------------- | ------------------------------ |
| veh.cluster.VehicleStatus.currentSpeed  | 220.2 (220.1 to 220.2; 0 %)       | 236.2 (236.2 to 236.3; 0 %)       | 220.7 (220.7 to 220.8; 0 %)    |
| veh.cluster.VehicleStatus.engineTemp    | 226.8 (226.8 to 226.9; 0 %)       | 245.1 (245.1 to 245.5; 0 %)       | 227.9 (227.9 to 228.0; 0 %)    |
| veh.cluster.VehicleStatus.warnings      | 49.3 (49.3 to 49.4; 0 %)          | 77.6 (77.6 to 77.7; 0 %)          | 30.6 (30.5 to 30.7; 1 %)       |
| veh.cluster.WheelHistory.wheelTicks     | 48.2 (48.2 to 48.3; 0 %)          | 77.6 (77.6 to 77.8; 0 %)          | 30.5 (30.5 to 30.5; 0 %)       |
| veh.cluster.WheelHistory.wheelTemp      | 226.8 (226.8 to 226.9; 0 %)       | 245.1 (245.1 to 245.2; 0 %)       | 227.9 (227.9 to 228.0; 0 %)    |
| veh.cluster.WheelHistory.wheelSlipRate  | 229.6 (229.6 to 229.6; 0 %)       | 245.7 (245.7 to 245.9; 0 %)       | 230.7 (230.7 to 230.8; 0 %)    |
| veh.cluster.WheelSummary.wheelHealth    | 48.2 (48.2 to 48.3; 0 %)          | 77.6 (77.6 to 77.6; 0 %)          | 30.5 (30.5 to 30.5; 0 %)       |
| veh.cluster.wheels.ticks                | 48.2 (48.2 to 48.3; 0 %)          | 77.6 (77.6 to 77.7; 0 %)          | 30.5 (30.5 to 30.5; 0 %)       |
| veh.cluster.WheelDiagnostics.wheelTicks | not built                         | not built                         | not built                      |
| veh.hvac.cabin.cabinTemp                | 226.8 (226.8 to 226.9; 0 %)       | 245.1 (245.1 to 245.2; 0 %)       | 227.9 (227.9 to 227.9; 0 %)    |
| veh.hvac.cabin.targetTemp               | 226.8 (226.8 to 226.9; 0 %)       | 245.1 (245.1 to 245.2; 0 %)       | 227.9 (227.9 to 228.0; 0 %)    |
| veh.hvac.cabin.blowerDuty               | 229.6 (229.6 to 229.7; 0 %)       | 245.7 (245.6 to 245.7; 0 %)       | 230.7 (230.7 to 231.0; 0 %)    |
| veh.hvac.cabin.fanLevel                 | 48.2 (48.2 to 48.3; 0 %)          | 77.6 (77.6 to 77.7; 0 %)          | 30.5 (30.5 to 30.5; 0 %)       |
| veh.hvac.cabin.compressorLoad           | 229.6 (229.6 to 229.6; 0 %)       | 245.7 (245.7 to 245.7; 0 %)       | 230.7 (230.7 to 230.8; 0 %)    |
| veh.cabin.Cabin.temperature             | 48.2 (48.2 to 48.3; 0 %)          | 77.6 (77.6 to 77.7; 0 %)          | 31.1 (31.0 to 31.1; 0 %)       |
| veh.cabin.Horn.active                   | 51.6 (51.6 to 51.7; 0 %)          | 77.1 (77.1 to 77.2; 0 %)          | 30.6 (30.5 to 30.7; 0 %)       |
| bench.stress.Stress.longString          | 669.3 (669.0 to 670.1; 0 %)       | 708.0 (706.7 to 708.7; 0 %)       | 785.1 (784.6 to 785.6; 0 %)    |
| bench.stress.Stress.itemList            | 2509.5 (2504.8 to 2514.2; 0 %)    | 4254.6 (4248.5 to 4261.9; 0 %)    | 3002.6 (3002.0 to 3006.8; 0 %) |
| bench.stress.Stress.nestedOptional      | 313.4 (311.1 to 316.0; 2 %)       | 463.3 (462.8 to 463.9; 0 %)       | 362.4 (361.3 to 363.2; 1 %)    |
| bench.stress.Stress.map                 | 13928.4 (13923.2 to 13938.0; 0 %) | 27336.4 (27321.8 to 27350.2; 0 %) | 4588.7 (4584.5 to 4595.0; 0 %) |

##### Time per operation, decode

CPU time of the decode operation of one update, in ns: the median of 30 samples
of a batch of this operation alone, with the fastest and the slowest sample and
the range as a share of the median. † spread above 10 %: the figure is not
stable.

| Payload                                 | FlatBuffers                    | proto3                         | podbuf                         |
| --------------------------------------- | ------------------------------ | ------------------------------ | ------------------------------ |
| veh.cluster.VehicleStatus.currentSpeed  | 40.5 (40.5 to 40.5; 0 %)       | 64.3 (64.3 to 64.4; 0 %)       | 37.2 (37.1 to 37.2; 0 %)       |
| veh.cluster.VehicleStatus.engineTemp    | 40.5 (40.5 to 40.5; 0 %)       | 64.3 (64.3 to 64.3; 0 %)       | 37.2 (37.2 to 37.2; 0 %)       |
| veh.cluster.VehicleStatus.warnings      | 36.5 (36.5 to 36.5; 0 %)       | 70.3 (70.3 to 70.4; 0 %)       | 8.3 (8.3 to 8.3; 0 %)          |
| veh.cluster.WheelHistory.wheelTicks     | 35.5 (35.5 to 35.5; 0 %)       | 69.3 (69.3 to 69.4; 0 %)       | 7.2 (7.2 to 7.2; 0 %)          |
| veh.cluster.WheelHistory.wheelTemp      | 40.5 (40.5 to 40.5; 0 %)       | 64.3 (64.3 to 64.4; 0 %)       | 37.2 (37.1 to 37.2; 0 %)       |
| veh.cluster.WheelHistory.wheelSlipRate  | 40.5 (40.5 to 40.5; 0 %)       | 64.3 (64.3 to 64.4; 0 %)       | 37.2 (37.1 to 37.2; 0 %)       |
| veh.cluster.WheelSummary.wheelHealth    | 35.5 (35.5 to 35.5; 0 %)       | 69.3 (69.3 to 69.3; 0 %)       | 7.2 (7.2 to 7.2; 0 %)          |
| veh.cluster.wheels.ticks                | 35.5 (35.5 to 35.5; 0 %)       | 69.3 (69.3 to 69.3; 0 %)       | 7.2 (7.2 to 7.2; 0 %)          |
| veh.cluster.WheelDiagnostics.wheelTicks | not built                      | not built                      | not built                      |
| veh.hvac.cabin.cabinTemp                | 40.5 (40.5 to 40.5; 0 %)       | 64.3 (64.3 to 64.3; 0 %)       | 37.2 (37.2 to 37.2; 0 %)       |
| veh.hvac.cabin.targetTemp               | 40.5 (40.5 to 40.5; 0 %)       | 64.3 (64.3 to 64.3; 0 %)       | 37.2 (37.1 to 37.2; 0 %)       |
| veh.hvac.cabin.blowerDuty               | 40.5 (40.5 to 40.5; 0 %)       | 64.3 (64.3 to 64.4; 0 %)       | 37.2 (37.1 to 37.2; 0 %)       |
| veh.hvac.cabin.fanLevel                 | 35.5 (35.5 to 35.5; 0 %)       | 69.3 (69.3 to 69.4; 0 %)       | 7.2 (7.2 to 7.2; 0 %)          |
| veh.hvac.cabin.compressorLoad           | 40.5 (40.5 to 40.5; 0 %)       | 64.3 (64.3 to 64.3; 0 %)       | 37.2 (37.1 to 37.2; 0 %)       |
| veh.cabin.Cabin.temperature             | 35.5 (35.5 to 35.5; 0 %)       | 72.6 (72.6 to 72.7; 0 %)       | 7.2 (7.2 to 7.2; 0 %)          |
| veh.cabin.Horn.active                   | 39.4 (39.4 to 39.4; 0 %)       | 72.5 (72.4 to 72.6; 0 %)       | 16.1 (16.1 to 16.1; 0 %)       |
| bench.stress.Stress.longString          | 465.9 (465.4 to 466.7; 0 %)    | 517.7 (516.7 to 519.7; 1 %)    | 406.4 (406.0 to 407.1; 0 %)    |
| bench.stress.Stress.itemList            | 4065.4 (4063.9 to 4066.5; 0 %) | 6357.7 (6354.4 to 6366.4; 0 %) | 3931.1 (3930.5 to 3932.9; 0 %) |
| bench.stress.Stress.nestedOptional      | 512.3 (511.1 to 514.8; 1 %)    | 703.9 (701.3 to 707.4; 1 %)    | 472.2 (470.0 to 475.2; 1 %)    |
| bench.stress.Stress.map                 | 3748.9 (3748.1 to 3750.2; 0 %) | 6012.4 (6010.1 to 6016.4; 0 %) | 3834.1 (3828.9 to 3842.1; 0 %) |

##### CPU load per operation

CPU load in percent of one core at 4000 updates per second of each operation
alone, derived from the median time of that operation. The three operations of
an encoding add up to its derived load of one update.

| Payload                                 | Encode: FlatBuffers | Encode: proto3 | Encode: podbuf | Verify: FlatBuffers | Verify: proto3 | Verify: podbuf | Decode: FlatBuffers | Decode: proto3 | Decode: podbuf |
| --------------------------------------- | ------------------- | -------------- | -------------- | ------------------- | -------------- | -------------- | ------------------- | -------------- | -------------- |
| veh.cluster.VehicleStatus.currentSpeed  | 0.0722              | 0.0304         | 0.0303         | 0.0881              | 0.0945         | 0.0883         | 0.0162              | 0.0257         | 0.0149         |
| veh.cluster.VehicleStatus.engineTemp    | 0.0719              | 0.0306         | 0.0300         | 0.0907              | 0.0980         | 0.0912         | 0.0162              | 0.0257         | 0.0149         |
| veh.cluster.VehicleStatus.warnings      | 0.0709              | 0.0335         | 0.0292         | 0.0197              | 0.0311         | 0.0122         | 0.0146              | 0.0281         | 0.0033         |
| veh.cluster.WheelHistory.wheelTicks     | 0.0709              | 0.0338         | 0.0291         | 0.0193              | 0.0311         | 0.0122         | 0.0142              | 0.0277         | 0.0029         |
| veh.cluster.WheelHistory.wheelTemp      | 0.0720              | 0.0306         | 0.0300         | 0.0907              | 0.0980         | 0.0912         | 0.0162              | 0.0257         | 0.0149         |
| veh.cluster.WheelHistory.wheelSlipRate  | 0.0719              | 0.0306         | 0.0300         | 0.0918              | 0.0983         | 0.0923         | 0.0162              | 0.0257         | 0.0149         |
| veh.cluster.WheelSummary.wheelHealth    | 0.0709              | 0.0337         | 0.0291         | 0.0193              | 0.0311         | 0.0122         | 0.0142              | 0.0277         | 0.0029         |
| veh.cluster.wheels.ticks                | 0.0709              | 0.0337         | 0.0290         | 0.0193              | 0.0311         | 0.0122         | 0.0142              | 0.0277         | 0.0029         |
| veh.cluster.WheelDiagnostics.wheelTicks | not built           | not built      | not built      | not built           | not built      | not built      | not built           | not built      | not built      |
| veh.hvac.cabin.cabinTemp                | 0.0718              | 0.0306         | 0.0300         | 0.0907              | 0.0980         | 0.0912         | 0.0162              | 0.0257         | 0.0149         |
| veh.hvac.cabin.targetTemp               | 0.0719              | 0.0306         | 0.0300         | 0.0907              | 0.0980         | 0.0912         | 0.0162              | 0.0257         | 0.0149         |
| veh.hvac.cabin.blowerDuty               | 0.0719              | 0.0306         | 0.0300         | 0.0918              | 0.0983         | 0.0923         | 0.0162              | 0.0257         | 0.0149         |
| veh.hvac.cabin.fanLevel                 | 0.0709              | 0.0337         | 0.0291         | 0.0193              | 0.0311         | 0.0122         | 0.0142              | 0.0277         | 0.0029         |
| veh.hvac.cabin.compressorLoad           | 0.0719              | 0.0306         | 0.0300         | 0.0918              | 0.0983         | 0.0923         | 0.0162              | 0.0257         | 0.0149         |
| veh.cabin.Cabin.temperature             | 0.0712              | 0.0346         | 0.0297         | 0.0193              | 0.0311         | 0.0124         | 0.0142              | 0.0291         | 0.0029         |
| veh.cabin.Horn.active                   | 0.0761              | 0.0299         | 0.0293         | 0.0206              | 0.0308         | 0.0122         | 0.0157              | 0.0290         | 0.0064         |
| bench.stress.Stress.longString          | 0.3682              | 0.0931         | 0.3995         | 0.2677              | 0.2832         | 0.3140         | 0.1864              | 0.2071         | 0.1626         |
| bench.stress.Stress.itemList            | 1.4975              | 1.3510         | 0.8037         | 1.0038              | 1.7018         | 1.2011         | 1.6262              | 2.5431         | 1.5724         |
| bench.stress.Stress.nestedOptional      | 0.2455              | 0.1722         | 0.1094         | 0.1254              | 0.1853         | 0.1449         | 0.2049              | 0.2816         | 0.1889         |
| bench.stress.Stress.map                 | 1.4977              | 2.6081         | 1.1381         | 5.5714              | 10.9346        | 1.8355         | 1.4996              | 2.4049         | 1.5336         |

##### Ratios per operation

Time of each operation of each encoding divided by the time of the same
operation in podbuf: above 1.00 the encoding is slower than podbuf in that
operation, below 1.00 faster. A refused or not-built cell has no ratio and is
marked. A time under about 10 ns includes harness cost and is an upper bound, so
a ratio whose podbuf time is under 10 ns is not comparable.

| Payload                                 | Encode: FlatBuffers / podbuf | Encode: proto3 / podbuf | Verify: FlatBuffers / podbuf | Verify: proto3 / podbuf | Decode: FlatBuffers / podbuf | Decode: proto3 / podbuf |
| --------------------------------------- | ---------------------------- | ----------------------- | ---------------------------- | ----------------------- | ---------------------------- | ----------------------- |
| veh.cluster.VehicleStatus.currentSpeed  | 2.39                         | 1.00                    | 1.00                         | 1.07                    | 1.09                         | 1.73                    |
| veh.cluster.VehicleStatus.engineTemp    | 2.40                         | 1.02                    | 1.00                         | 1.08                    | 1.09                         | 1.73                    |
| veh.cluster.VehicleStatus.warnings      | 2.43                         | 1.15                    | 1.61                         | 2.54                    | 4.39                         | 8.45                    |
| veh.cluster.WheelHistory.wheelTicks     | 2.44                         | 1.16                    | 1.58                         | 2.55                    | 4.92                         | 9.62                    |
| veh.cluster.WheelHistory.wheelTemp      | 2.40                         | 1.02                    | 1.00                         | 1.08                    | 1.09                         | 1.73                    |
| veh.cluster.WheelHistory.wheelSlipRate  | 2.39                         | 1.02                    | 1.00                         | 1.06                    | 1.09                         | 1.73                    |
| veh.cluster.WheelSummary.wheelHealth    | 2.44                         | 1.16                    | 1.58                         | 2.55                    | 4.92                         | 9.62                    |
| veh.cluster.wheels.ticks                | 2.44                         | 1.16                    | 1.58                         | 2.55                    | 4.92                         | 9.62                    |
| veh.cluster.WheelDiagnostics.wheelTicks | not built                    | not built               | not built                    | not built               | not built                    | not built               |
| veh.hvac.cabin.cabinTemp                | 2.40                         | 1.02                    | 1.00                         | 1.08                    | 1.09                         | 1.73                    |
| veh.hvac.cabin.targetTemp               | 2.40                         | 1.02                    | 1.00                         | 1.08                    | 1.09                         | 1.73                    |
| veh.hvac.cabin.blowerDuty               | 2.40                         | 1.02                    | 1.00                         | 1.06                    | 1.09                         | 1.73                    |
| veh.hvac.cabin.fanLevel                 | 2.44                         | 1.16                    | 1.58                         | 2.55                    | 4.92                         | 9.62                    |
| veh.hvac.cabin.compressorLoad           | 2.40                         | 1.02                    | 1.00                         | 1.06                    | 1.09                         | 1.73                    |
| veh.cabin.Cabin.temperature             | 2.40                         | 1.17                    | 1.55                         | 2.50                    | 4.92                         | 10.08                   |
| veh.cabin.Horn.active                   | 2.60                         | 1.02                    | 1.69                         | 2.52                    | 2.45                         | 4.51                    |
| bench.stress.Stress.longString          | 0.92                         | 0.23                    | 0.85                         | 0.90                    | 1.15                         | 1.27                    |
| bench.stress.Stress.itemList            | 1.86                         | 1.68                    | 0.84                         | 1.42                    | 1.03                         | 1.62                    |
| bench.stress.Stress.nestedOptional      | 2.24                         | 1.57                    | 0.86                         | 1.28                    | 1.09                         | 1.49                    |
| bench.stress.Stress.map                 | 1.32                         | 2.29                    | 3.04                         | 5.96                    | 0.98                         | 1.57                    |

##### CPU load, paced and derived

CPU load in percent of one core at 4000 updates per second: paced (process CPU
time over wall time, includes the cost of sleep and wake-up) next to the derived
figure, and their difference.

| Payload                                | Encoding    | Paced % | Derived % | Difference |
| -------------------------------------- | ----------- | ------- | --------- | ---------- |
| veh.cluster.VehicleStatus.currentSpeed | FlatBuffers | 11.2955 | 0.1764    | 11.1190    |
| veh.cluster.VehicleStatus.currentSpeed | proto3      | 11.0139 | 0.1506    | 10.8633    |
| veh.cluster.VehicleStatus.currentSpeed | podbuf      | 10.7635 | 0.1334    | 10.6301    |
| veh.cluster.VehicleStatus.engineTemp   | FlatBuffers | 10.0742 | 0.1788    | 9.8954     |
| veh.cluster.VehicleStatus.engineTemp   | proto3      | 9.8549  | 0.1544    | 9.7005     |
| veh.cluster.VehicleStatus.engineTemp   | podbuf      | 9.5565  | 0.1360    | 9.4204     |
| veh.cluster.VehicleStatus.warnings     | FlatBuffers | 10.4819 | 0.1052    | 10.3767    |
| veh.cluster.VehicleStatus.warnings     | proto3      | 10.3880 | 0.0927    | 10.2954    |
| veh.cluster.VehicleStatus.warnings     | podbuf      | 9.9344  | 0.0448    | 9.8896     |
| veh.cluster.WheelHistory.wheelTicks    | FlatBuffers | 9.3452  | 0.1044    | 9.2409     |
| veh.cluster.WheelHistory.wheelTicks    | proto3      | 9.3381  | 0.0925    | 9.2456     |
| veh.cluster.WheelHistory.wheelTicks    | podbuf      | 9.0632  | 0.0441    | 9.0190     |
| veh.cluster.WheelHistory.wheelTemp     | FlatBuffers | 9.9475  | 0.1789    | 9.7686     |
| veh.cluster.WheelHistory.wheelTemp     | proto3      | 9.7634  | 0.1544    | 9.6091     |
| veh.cluster.WheelHistory.wheelTemp     | podbuf      | 9.5186  | 0.1360    | 9.3826     |
| veh.cluster.WheelHistory.wheelSlipRate | FlatBuffers | 11.1455 | 0.1800    | 10.9656    |
| veh.cluster.WheelHistory.wheelSlipRate | proto3      | 11.3677 | 0.1546    | 11.2131    |
| veh.cluster.WheelHistory.wheelSlipRate | podbuf      | 9.9970  | 0.1372    | 9.8598     |
| veh.cluster.WheelSummary.wheelHealth   | FlatBuffers | 9.4308  | 0.1044    | 9.3264     |
| veh.cluster.WheelSummary.wheelHealth   | proto3      | 9.2544  | 0.0925    | 9.1618     |
| veh.cluster.WheelSummary.wheelHealth   | podbuf      | 8.9546  | 0.0442    | 8.9104     |
| veh.cluster.wheels.ticks               | FlatBuffers | 9.8437  | 0.1044    | 9.7392     |
| veh.cluster.wheels.ticks               | proto3      | 9.4576  | 0.0925    | 9.3651     |
| veh.cluster.wheels.ticks               | podbuf      | 9.2800  | 0.0441    | 9.2358     |
| veh.hvac.cabin.cabinTemp               | FlatBuffers | 10.1125 | 0.1787    | 9.9338     |
| veh.hvac.cabin.cabinTemp               | proto3      | 9.9326  | 0.1544    | 9.7782     |
| veh.hvac.cabin.cabinTemp               | podbuf      | 9.9507  | 0.1360    | 9.8147     |
| veh.hvac.cabin.targetTemp              | FlatBuffers | 9.9647  | 0.1788    | 9.7859     |
| veh.hvac.cabin.targetTemp              | proto3      | 9.7662  | 0.1544    | 9.6119     |
| veh.hvac.cabin.targetTemp              | podbuf      | 9.7042  | 0.1360    | 9.5682     |
| veh.hvac.cabin.blowerDuty              | FlatBuffers | 11.7853 | 0.1799    | 11.6054    |
| veh.hvac.cabin.blowerDuty              | proto3      | 11.1044 | 0.1546    | 10.9498    |
| veh.hvac.cabin.blowerDuty              | podbuf      | 9.8181  | 0.1371    | 9.6810     |
| veh.hvac.cabin.fanLevel                | FlatBuffers | 9.3005  | 0.1044    | 9.1960     |
| veh.hvac.cabin.fanLevel                | proto3      | 9.1560  | 0.0925    | 9.0635     |
| veh.hvac.cabin.fanLevel                | podbuf      | 8.9558  | 0.0442    | 8.9116     |
| veh.hvac.cabin.compressorLoad          | FlatBuffers | 10.3067 | 0.1799    | 10.1268    |
| veh.hvac.cabin.compressorLoad          | proto3      | 9.7080  | 0.1546    | 9.5534     |
| veh.hvac.cabin.compressorLoad          | podbuf      | 9.4956  | 0.1371    | 9.3585     |
| veh.cabin.Cabin.temperature            | FlatBuffers | 9.0138  | 0.1047    | 8.9091     |
| veh.cabin.Cabin.temperature            | proto3      | 9.3229  | 0.0947    | 9.2282     |
| veh.cabin.Cabin.temperature            | podbuf      | 8.8859  | 0.0450    | 8.8409     |
| veh.cabin.Horn.active                  | FlatBuffers | 9.8550  | 0.1125    | 9.7426     |
| veh.cabin.Horn.active                  | proto3      | 9.4390  | 0.0898    | 9.3492     |
| veh.cabin.Horn.active                  | podbuf      | 8.8768  | 0.0479    | 8.8289     |
| bench.stress.Stress.longString         | FlatBuffers | 15.4818 | 0.8223    | 14.6595    |
| bench.stress.Stress.longString         | proto3      | 14.8755 | 0.5834    | 14.2920    |
| bench.stress.Stress.longString         | podbuf      | 17.3631 | 0.8761    | 16.4870    |
| bench.stress.Stress.itemList           | FlatBuffers | 27.0774 | 4.1275    | 22.9499    |
| bench.stress.Stress.itemList           | proto3      | 31.4697 | 5.5960    | 25.8737    |
| bench.stress.Stress.itemList           | podbuf      | 25.4001 | 3.5772    | 21.8229    |
| bench.stress.Stress.nestedOptional     | FlatBuffers | 12.6770 | 0.5758    | 12.1012    |
| bench.stress.Stress.nestedOptional     | proto3      | 12.3046 | 0.6391    | 11.6655    |
| bench.stress.Stress.nestedOptional     | podbuf      | 12.1868 | 0.4432    | 11.7436    |
| bench.stress.Stress.map                | FlatBuffers | 39.3388 | 8.5686    | 30.7702    |
| bench.stress.Stress.map                | proto3      | 62.1969 | 15.9476   | 46.2493    |
| bench.stress.Stress.map                | podbuf      | 27.2616 | 4.5072    | 22.7544    |

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
| Time per update (median ns)                            | 130.5       | 99.9   | 76.9   | 1.70                 | 1.30            | 15    |
| CPU % of one core at 4000 updates per second (derived) | 0.0522      | 0.0400 | 0.0308 | 1.70                 | 1.30            | 15    |

Equal-weight mix, per payload type: each payload type counts once, however many
signals share it (the value of a type is the mean over its signals). Only
payload types whose signals are measured in all three encodings are counted; the
last column gives how many.

| Metric                                                 | FlatBuffers | proto3 | podbuf | FlatBuffers / podbuf | proto3 / podbuf | Units |
| ------------------------------------------------------ | ----------- | ------ | ------ | -------------------- | --------------- | ----- |
| Size (bytes)                                           | 25.3        | 3.4    | 16.0   | 1.58                 | 0.21            | 6     |
| Heap allocations per update (encode + verify + decode) | 0.0         | 0.0    | 0.0    | n/a                  | n/a             | 6     |
| Time per update (median ns)                            | 128.1       | 96.4   | 72.1   | 1.78                 | 1.34            | 6     |
| CPU % of one core at 4000 updates per second (derived) | 0.0512      | 0.0386 | 0.0288 | 1.78                 | 1.34            | 6     |

Equal-weight mix, per signal: each signal counts once, so a payload type that
several signals share counts once for each of them. Only signals whose three
encodings are measured are counted; the last column gives how many. The rows are
per operation. A time under about 10 ns includes harness cost and is an upper
bound, so a ratio whose podbuf time is under 10 ns is not comparable.

| Metric                                                         | FlatBuffers | proto3 | podbuf | FlatBuffers / podbuf | proto3 / podbuf | Units |
| -------------------------------------------------------------- | ----------- | ------ | ------ | -------------------- | --------------- | ----- |
| Encode: time per operation (median ns)                         | 71.9        | 26.3   | 28.3   | 2.54                 | 0.93            | 15    |
| Encode: CPU % of one core at 4000 updates per second (derived) | 0.0288      | 0.0105 | 0.0113 | 2.54                 | 0.93            | 15    |
| Encode: heap allocations                                       | 0.0         | 0.0    | 0.0    | n/a                  | n/a             | 15    |
| Verify: time per operation (median ns)                         | 44.5        | 47.9   | 40.4   | 1.10                 | 1.19            | 15    |
| Verify: CPU % of one core at 4000 updates per second (derived) | 0.0178      | 0.0192 | 0.0161 | 1.10                 | 1.19            | 15    |
| Verify: heap allocations                                       | 0.0         | 0.0    | 0.0    | n/a                  | n/a             | 15    |
| Decode: time per operation (median ns)                         | 14.0        | 25.7   | 8.3    | 1.69                 | 3.09            | 15    |
| Decode: CPU % of one core at 4000 updates per second (derived) | 0.0056      | 0.0103 | 0.0033 | 1.69                 | 3.09            | 15    |
| Decode: heap allocations                                       | 0.0         | 0.0    | 0.0    | n/a                  | n/a             | 15    |

Equal-weight mix, per payload type: each payload type counts once, however many
signals share it (the value of a type is the mean over its signals). Only
payload types whose signals are measured in all three encodings are counted; the
last column gives how many. The rows are per operation. A time under about 10 ns
includes harness cost and is an upper bound, so a ratio whose podbuf time is
under 10 ns is not comparable.

| Metric                                                         | FlatBuffers | proto3 | podbuf | FlatBuffers / podbuf | proto3 / podbuf | Units |
| -------------------------------------------------------------- | ----------- | ------ | ------ | -------------------- | --------------- | ----- |
| Encode: time per operation (median ns)                         | 73.4        | 26.2   | 28.3   | 2.60                 | 0.93            | 6     |
| Encode: CPU % of one core at 4000 updates per second (derived) | 0.0294      | 0.0105 | 0.0113 | 2.60                 | 0.93            | 6     |
| Encode: heap allocations                                       | 0.0         | 0.0    | 0.0    | n/a                  | n/a             | 6     |
| Verify: time per operation (median ns)                         | 40.5        | 44.1   | 35.9   | 1.13                 | 1.23            | 6     |
| Verify: CPU % of one core at 4000 updates per second (derived) | 0.0162      | 0.0176 | 0.0144 | 1.13                 | 1.23            | 6     |
| Verify: heap allocations                                       | 0.0         | 0.0    | 0.0    | n/a                  | n/a             | 6     |
| Decode: time per operation (median ns)                         | 14.2        | 26.0   | 8.0    | 1.78                 | 3.27            | 6     |
| Decode: CPU % of one core at 4000 updates per second (derived) | 0.0057      | 0.0104 | 0.0032 | 1.78                 | 3.27            | 6     |
| Decode: heap allocations                                       | 0.0         | 0.0    | 0.0    | n/a                  | n/a             | 6     |

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

CPU time of one update (encode, verify and decode), in ns: the sum of the
medians of the three operations, with the sum of their fastest and the sum of
their slowest sample (an envelope, not a percentile) and its range as a share of
the sum. † spread above 10 %: the figure is not stable.

| Payload                                 | FlatBuffers                    | proto3                            | podbuf                         |
| --------------------------------------- | ------------------------------ | --------------------------------- | ------------------------------ |
| veh.cluster.VehicleStatus.currentSpeed  | 160.9 (158.0 to 162.1; 3 %)    | 117.3 (116.5 to 119.1; 2 %)       | 106.2 (106.0 to 106.7; 1 %)    |
| veh.cluster.VehicleStatus.engineTemp    | 154.8 (153.3 to 156.0; 2 %)    | 120.8 (119.8 to 121.7; 2 %)       | 110.8 (110.3 to 111.7; 1 %)    |
| veh.cluster.VehicleStatus.warnings      | 99.8 (98.9 to 101.2; 2 %)      | 77.9 (77.7 to 78.2; 1 %)          | 38.5 (38.3 to 38.5; 0 %)       |
| veh.cluster.WheelHistory.wheelTicks     | 100.6 (98.9 to 101.8; 3 %)     | 76.8 (76.6 to 77.0; 1 %)          | 38.0 (37.9 to 38.0; 0 %)       |
| veh.cluster.WheelHistory.wheelTemp      | 154.6 (153.3 to 156.2; 2 %)    | 120.8 (119.9 to 123.0; 3 %)       | 111.1 (110.2 to 111.9; 1 %)    |
| veh.cluster.WheelHistory.wheelSlipRate  | 153.0 (150.3 to 153.8; 2 %)    | 120.1 (119.2 to 121.5; 2 %)       | 111.4 (109.6 to 111.8; 2 %)    |
| veh.cluster.WheelSummary.wheelHealth    | 100.3 (99.2 to 101.4; 2 %)     | 76.8 (76.4 to 77.4; 1 %)          | 38.0 (37.9 to 38.0; 0 %)       |
| veh.cluster.wheels.ticks                | 100.0 (99.3 to 101.6; 2 %)     | 76.8 (76.5 to 77.0; 1 %)          | 38.0 (37.9 to 38.0; 0 %)       |
| veh.cluster.WheelDiagnostics.wheelTicks | not built                      | not built                         | not built                      |
| veh.hvac.cabin.cabinTemp                | 154.5 (153.2 to 156.4; 2 %)    | 120.8 (119.9 to 122.8; 2 %)       | 110.8 (110.3 to 111.3; 1 %)    |
| veh.hvac.cabin.targetTemp               | 154.7 (153.2 to 156.0; 2 %)    | 120.6 (119.8 to 122.5; 2 %)       | 110.8 (110.2 to 111.9; 1 %)    |
| veh.hvac.cabin.blowerDuty               | 152.2 (150.6 to 153.7; 2 %)    | 120.0 (119.3 to 120.7; 1 %)       | 111.2 (109.6 to 111.6; 2 %)    |
| veh.hvac.cabin.fanLevel                 | 99.7 (98.8 to 101.5; 3 %)      | 76.9 (76.5 to 77.3; 1 %)          | 38.0 (37.9 to 38.0; 0 %)       |
| veh.hvac.cabin.compressorLoad           | 152.2 (151.0 to 153.4; 2 %)    | 120.2 (119.1 to 121.6; 2 %)       | 110.7 (109.6 to 111.8; 2 %)    |
| veh.cabin.Cabin.temperature             | 109.9 (108.3 to 111.4; 3 %)    | 78.3 (78.0 to 78.8; 1 %)          | 38.2 (38.2 to 38.4; 0 %)       |
| veh.cabin.Horn.active                   | 109.7 (108.8 to 111.1; 2 %)    | 74.0 (73.8 to 74.7; 1 %)          | 42.7 (42.6 to 42.9; 1 %)       |
| bench.stress.Stress.longString          | 922.2 (919.5 to 927.2; 1 %)    | 433.8 (432.5 to 434.5; 0 %)       | 991.6 (989.0 to 994.1; 1 %)    |
| bench.stress.Stress.itemList            | 3596.3 (3586.0 to 3611.5; 1 %) | 4680.9 (4651.0 to 4693.9; 1 %)    | 3042.0 (3037.8 to 3052.2; 0 %) |
| bench.stress.Stress.nestedOptional      | 472.9 (469.4 to 476.4; 1 %)    | 481.6 (477.1 to 484.1; 1 %)       | 344.5 (343.8 to 346.2; 1 %)    |
| bench.stress.Stress.map                 | 7992.3 (7978.4 to 8012.8; 0 %) | 12923.0 (12858.3 to 12969.8; 1 %) | 3920.9 (3911.8 to 3934.2; 1 %) |

##### CPU load, derived

CPU load in percent of one core at 4000 updates per second, derived from the
median time of one update (the headline figure).

| Payload                                 | FlatBuffers | proto3    | podbuf    |
| --------------------------------------- | ----------- | --------- | --------- |
| veh.cluster.VehicleStatus.currentSpeed  | 0.0643      | 0.0469    | 0.0425    |
| veh.cluster.VehicleStatus.engineTemp    | 0.0619      | 0.0483    | 0.0443    |
| veh.cluster.VehicleStatus.warnings      | 0.0399      | 0.0312    | 0.0154    |
| veh.cluster.WheelHistory.wheelTicks     | 0.0402      | 0.0307    | 0.0152    |
| veh.cluster.WheelHistory.wheelTemp      | 0.0618      | 0.0483    | 0.0444    |
| veh.cluster.WheelHistory.wheelSlipRate  | 0.0612      | 0.0481    | 0.0445    |
| veh.cluster.WheelSummary.wheelHealth    | 0.0401      | 0.0307    | 0.0152    |
| veh.cluster.wheels.ticks                | 0.0400      | 0.0307    | 0.0152    |
| veh.cluster.WheelDiagnostics.wheelTicks | not built   | not built | not built |
| veh.hvac.cabin.cabinTemp                | 0.0618      | 0.0483    | 0.0443    |
| veh.hvac.cabin.targetTemp               | 0.0619      | 0.0482    | 0.0443    |
| veh.hvac.cabin.blowerDuty               | 0.0609      | 0.0480    | 0.0445    |
| veh.hvac.cabin.fanLevel                 | 0.0399      | 0.0307    | 0.0152    |
| veh.hvac.cabin.compressorLoad           | 0.0609      | 0.0481    | 0.0443    |
| veh.cabin.Cabin.temperature             | 0.0440      | 0.0313    | 0.0153    |
| veh.cabin.Horn.active                   | 0.0439      | 0.0296    | 0.0171    |
| bench.stress.Stress.longString          | 0.3689      | 0.1735    | 0.3966    |
| bench.stress.Stress.itemList            | 1.4385      | 1.8724    | 1.2168    |
| bench.stress.Stress.nestedOptional      | 0.1891      | 0.1927    | 0.1378    |
| bench.stress.Stress.map                 | 3.1969      | 5.1692    | 1.5684    |

##### Ratios to podbuf

Each encoding divided by podbuf: above 1.00 the encoding is larger or slower
than podbuf, below 1.00 smaller or faster. The CPU ratios equal the time ratios,
because the derived CPU load is proportional to the median time. A refused or
not-built cell has no ratio and is marked.

| Payload                                 | Size: FlatBuffers / podbuf | Size: proto3 / podbuf | Time: FlatBuffers / podbuf | Time: proto3 / podbuf | CPU: FlatBuffers / podbuf | CPU: proto3 / podbuf |
| --------------------------------------- | -------------------------- | --------------------- | -------------------------- | --------------------- | ------------------------- | -------------------- |
| veh.cluster.VehicleStatus.currentSpeed  | 1.50                       | 0.31                  | 1.52                       | 1.10                  | 1.52                      | 1.10                 |
| veh.cluster.VehicleStatus.engineTemp    | 1.50                       | 0.31                  | 1.40                       | 1.09                  | 1.40                      | 1.09                 |
| veh.cluster.VehicleStatus.warnings      | 1.50                       | 0.12                  | 2.59                       | 2.03                  | 2.59                      | 2.03                 |
| veh.cluster.WheelHistory.wheelTicks     | 1.50                       | 0.12                  | 2.65                       | 2.02                  | 2.65                      | 2.02                 |
| veh.cluster.WheelHistory.wheelTemp      | 1.50                       | 0.31                  | 1.39                       | 1.09                  | 1.39                      | 1.09                 |
| veh.cluster.WheelHistory.wheelSlipRate  | 1.50                       | 0.31                  | 1.37                       | 1.08                  | 1.37                      | 1.08                 |
| veh.cluster.WheelSummary.wheelHealth    | 1.50                       | 0.12                  | 2.64                       | 2.02                  | 2.64                      | 2.02                 |
| veh.cluster.wheels.ticks                | 1.50                       | 0.12                  | 2.63                       | 2.02                  | 2.63                      | 2.02                 |
| veh.cluster.WheelDiagnostics.wheelTicks | not built                  | not built             | not built                  | not built             | not built                 | not built            |
| veh.hvac.cabin.cabinTemp                | 1.50                       | 0.31                  | 1.39                       | 1.09                  | 1.39                      | 1.09                 |
| veh.hvac.cabin.targetTemp               | 1.50                       | 0.31                  | 1.40                       | 1.09                  | 1.40                      | 1.09                 |
| veh.hvac.cabin.blowerDuty               | 1.50                       | 0.31                  | 1.37                       | 1.08                  | 1.37                      | 1.08                 |
| veh.hvac.cabin.fanLevel                 | 1.50                       | 0.12                  | 2.63                       | 2.02                  | 2.63                      | 2.02                 |
| veh.hvac.cabin.compressorLoad           | 1.50                       | 0.31                  | 1.38                       | 1.09                  | 1.38                      | 1.09                 |
| veh.cabin.Cabin.temperature             | 1.50                       | 0.12                  | 2.88                       | 2.05                  | 2.88                      | 2.05                 |
| veh.cabin.Horn.active                   | 2.00                       | 0.12                  | 2.57                       | 1.73                  | 2.57                      | 1.73                 |
| bench.stress.Stress.longString          | 1.03                       | 0.97                  | 0.93                       | 0.44                  | 0.93                      | 0.44                 |
| bench.stress.Stress.itemList            | 1.50                       | 0.55                  | 1.18                       | 1.54                  | 1.18                      | 1.54                 |
| bench.stress.Stress.nestedOptional      | 1.63                       | 0.35                  | 1.37                       | 1.40                  | 1.37                      | 1.40                 |
| bench.stress.Stress.map                 | 1.56                       | 0.53                  | 2.04                       | 3.30                  | 2.04                      | 3.30                 |

##### Time per operation, encode

CPU time of the encode operation of one update, in ns: the median of 30 samples
of a batch of this operation alone, with the fastest and the slowest sample and
the range as a share of the median. † spread above 10 %: the figure is not
stable.

| Payload                                 | FlatBuffers                    | proto3                         | podbuf                         |
| --------------------------------------- | ------------------------------ | ------------------------------ | ------------------------------ |
| veh.cluster.VehicleStatus.currentSpeed  | 81.2 (79.0 to 82.1; 4 %)       | 26.4 (26.3 to 26.8; 2 %)       | 29.0 (29.0 to 29.0; 0 %)       |
| veh.cluster.VehicleStatus.engineTemp    | 69.6 (68.3 to 70.5; 3 %)       | 25.5 (25.1 to 25.9; 3 %)       | 29.0 (29.0 to 29.0; 0 %)       |
| veh.cluster.VehicleStatus.warnings      | 70.1 (69.3 to 71.4; 3 %)       | 27.5 (27.5 to 27.6; 1 %)       | 27.6 (27.6 to 27.6; 0 %)       |
| veh.cluster.WheelHistory.wheelTicks     | 71.1 (69.5 to 72.1; 4 %)       | 27.5 (27.4 to 27.6; 1 %)       | 27.2 (27.2 to 27.2; 0 %)       |
| veh.cluster.WheelHistory.wheelTemp      | 69.5 (68.4 to 70.7; 3 %)       | 25.4 (25.1 to 25.5; 2 %)       | 29.0 (29.0 to 29.0; 0 %)       |
| veh.cluster.WheelHistory.wheelSlipRate  | 70.5 (68.4 to 71.0; 4 %)       | 25.4 (25.1 to 25.6; 2 %)       | 29.0 (29.0 to 29.0; 0 %)       |
| veh.cluster.WheelSummary.wheelHealth    | 70.8 (69.8 to 71.7; 3 %)       | 27.5 (27.3 to 27.9; 2 %)       | 27.2 (27.2 to 27.2; 0 %)       |
| veh.cluster.wheels.ticks                | 70.6 (69.9 to 71.9; 3 %)       | 27.5 (27.3 to 27.5; 1 %)       | 27.2 (27.2 to 27.2; 0 %)       |
| veh.cluster.WheelDiagnostics.wheelTicks | not built                      | not built                      | not built                      |
| veh.hvac.cabin.cabinTemp                | 69.4 (68.2 to 70.6; 3 %)       | 25.4 (25.1 to 25.6; 2 %)       | 29.0 (29.0 to 29.0; 0 %)       |
| veh.hvac.cabin.targetTemp               | 69.6 (68.4 to 70.6; 3 %)       | 25.4 (25.2 to 25.5; 1 %)       | 29.0 (29.0 to 29.0; 0 %)       |
| veh.hvac.cabin.blowerDuty               | 69.6 (68.4 to 70.9; 3 %)       | 25.4 (25.1 to 25.8; 3 %)       | 29.0 (29.0 to 29.0; 0 %)       |
| veh.hvac.cabin.fanLevel                 | 70.3 (69.4 to 71.8; 3 %)       | 27.5 (27.4 to 27.8; 2 %)       | 27.2 (27.2 to 27.2; 0 %)       |
| veh.hvac.cabin.compressorLoad           | 69.6 (68.6 to 70.7; 3 %)       | 25.4 (25.1 to 25.5; 2 %)       | 29.0 (29.0 to 29.0; 0 %)       |
| veh.cabin.Cabin.temperature             | 80.4 (78.9 to 81.7; 3 %)       | 28.9 (28.8 to 29.3; 2 %)       | 27.6 (27.6 to 27.6; 0 %)       |
| veh.cabin.Horn.active                   | 76.9 (76.1 to 78.3; 3 %)       | 24.4 (24.3 to 24.9; 2 %)       | 28.1 (28.1 to 28.1; 0 %)       |
| bench.stress.Stress.longString          | 567.2 (565.8 to 568.5; 0 %)    | 57.1 (56.8 to 57.1; 1 %)       | 591.9 (591.5 to 592.5; 0 %)    |
| bench.stress.Stress.itemList            | 1386.1 (1384.7 to 1395.9; 1 %) | 1072.2 (1047.9 to 1078.6; 3 %) | 799.9 (799.7 to 801.1; 0 %)    |
| bench.stress.Stress.nestedOptional      | 210.2 (208.8 to 212.3; 2 %)    | 132.5 (128.4 to 134.2; 4 %)    | 98.5 (98.4 to 98.6; 0 %)       |
| bench.stress.Stress.map                 | 1387.1 (1385.2 to 1388.6; 0 %) | 2241.6 (2216.3 to 2249.8; 1 %) | 1089.6 (1085.3 to 1099.3; 1 %) |

##### Time per operation, verify

CPU time of the verify operation of one update, in ns: the median of 30 samples
of a batch of this operation alone, with the fastest and the slowest sample and
the range as a share of the median. † spread above 10 %: the figure is not
stable.

| Payload                                 | FlatBuffers                    | proto3                         | podbuf                         |
| --------------------------------------- | ------------------------------ | ------------------------------ | ------------------------------ |
| veh.cluster.VehicleStatus.currentSpeed  | 65.4 (64.8 to 65.7; 1 %)       | 66.8 (66.5 to 68.2; 3 %)       | 63.8 (63.7 to 64.3; 1 %)       |
| veh.cluster.VehicleStatus.engineTemp    | 70.8 (70.7 to 71.1; 1 %)       | 71.3 (70.8 to 71.7; 1 %)       | 68.6 (68.1 to 69.4; 2 %)       |
| veh.cluster.VehicleStatus.warnings      | 16.2 (16.2 to 16.2; 0 %)       | 21.9 (21.8 to 22.0; 1 %)       | 8.7 (8.6 to 8.8; 2 %)          |
| veh.cluster.WheelHistory.wheelTicks     | 16.2 (16.2 to 16.2; 0 %)       | 21.9 (21.8 to 22.0; 1 %)       | 8.7 (8.7 to 8.8; 1 %)          |
| veh.cluster.WheelHistory.wheelTemp      | 70.8 (70.6 to 71.2; 1 %)       | 71.4 (70.9 to 73.5; 4 %)       | 68.9 (68.0 to 69.6; 2 %)       |
| veh.cluster.WheelHistory.wheelSlipRate  | 68.3 (67.6 to 68.4; 1 %)       | 70.7 (70.3 to 71.8; 2 %)       | 69.1 (67.4 to 69.5; 3 %)       |
| veh.cluster.WheelSummary.wheelHealth    | 16.2 (16.2 to 16.2; 0 %)       | 21.9 (21.8 to 22.1; 1 %)       | 8.8 (8.7 to 8.8; 1 %)          |
| veh.cluster.wheels.ticks                | 16.2 (16.2 to 16.2; 0 %)       | 21.9 (21.8 to 22.0; 1 %)       | 8.8 (8.7 to 8.8; 1 %)          |
| veh.cluster.WheelDiagnostics.wheelTicks | not built                      | not built                      | not built                      |
| veh.hvac.cabin.cabinTemp                | 70.8 (70.7 to 71.5; 1 %)       | 71.4 (70.9 to 73.2; 3 %)       | 68.5 (68.0 to 69.1; 2 %)       |
| veh.hvac.cabin.targetTemp               | 70.8 (70.5 to 71.1; 1 %)       | 71.3 (70.7 to 72.9; 3 %)       | 68.5 (68.0 to 69.6; 2 %)       |
| veh.hvac.cabin.blowerDuty               | 68.3 (67.9 to 68.5; 1 %)       | 70.7 (70.3 to 70.9; 1 %)       | 69.0 (67.4 to 69.3; 3 %)       |
| veh.hvac.cabin.fanLevel                 | 16.2 (16.2 to 16.2; 0 %)       | 21.9 (21.8 to 22.1; 1 %)       | 8.7 (8.7 to 8.8; 1 %)          |
| veh.hvac.cabin.compressorLoad           | 68.3 (68.2 to 68.4; 0 %)       | 70.7 (70.1 to 72.0; 3 %)       | 68.5 (67.4 to 69.5; 3 %)       |
| veh.cabin.Cabin.temperature             | 16.2 (16.2 to 16.2; 0 %)       | 22.4 (22.4 to 22.5; 0 %)       | 8.5 (8.5 to 8.7; 2 %)          |
| veh.cabin.Horn.active                   | 17.3 (17.3 to 17.3; 0 %)       | 21.9 (21.8 to 22.0; 1 %)       | 8.5 (8.5 to 8.7; 2 %)          |
| bench.stress.Stress.longString          | 224.6 (223.9 to 225.1; 1 %)    | 236.5 (236.2 to 237.0; 0 %)    | 281.0 (279.9 to 281.8; 1 %)    |
| bench.stress.Stress.itemList            | 862.1 (856.9 to 864.3; 1 %)    | 1475.1 (1474.0 to 1477.2; 0 %) | 984.1 (980.5 to 986.8; 1 %)    |
| bench.stress.Stress.nestedOptional      | 106.2 (104.8 to 107.0; 2 %)    | 150.2 (150.1 to 150.4; 0 %)    | 108.7 (108.3 to 109.4; 1 %)    |
| bench.stress.Stress.map                 | 5303.8 (5292.5 to 5321.7; 1 %) | 8704.2 (8675.1 to 8730.1; 1 %) | 1596.7 (1595.7 to 1598.0; 0 %) |

##### Time per operation, decode

CPU time of the decode operation of one update, in ns: the median of 30 samples
of a batch of this operation alone, with the fastest and the slowest sample and
the range as a share of the median. † spread above 10 %: the figure is not
stable.

| Payload                                 | FlatBuffers                    | proto3                         | podbuf                         |
| --------------------------------------- | ------------------------------ | ------------------------------ | ------------------------------ |
| veh.cluster.VehicleStatus.currentSpeed  | 14.3 (14.3 to 14.3; 0 %)       | 24.0 (23.7 to 24.1; 1 %)       | 13.3 (13.3 to 13.5; 1 %)       |
| veh.cluster.VehicleStatus.engineTemp    | 14.3 (14.3 to 14.3; 0 %)       | 24.0 (23.8 to 24.0; 1 %)       | 13.3 (13.3 to 13.3; 0 %)       |
| veh.cluster.VehicleStatus.warnings      | 13.5 (13.5 to 13.6; 1 %)       | 28.5 (28.5 to 28.6; 0 %)       | 2.1 (2.1 to 2.1; 1 %)          |
| veh.cluster.WheelHistory.wheelTicks     | 13.3 (13.2 to 13.4; 2 %)       | 27.4 (27.3 to 27.4; 0 %)       | 2.0 (2.0 to 2.1; 1 %)          |
| veh.cluster.WheelHistory.wheelTemp      | 14.3 (14.3 to 14.3; 1 %)       | 24.0 (23.9 to 24.1; 0 %)       | 13.2 (13.2 to 13.3; 0 %)       |
| veh.cluster.WheelHistory.wheelSlipRate  | 14.3 (14.2 to 14.3; 0 %)       | 24.0 (23.9 to 24.1; 1 %)       | 13.2 (13.2 to 13.2; 1 %)       |
| veh.cluster.WheelSummary.wheelHealth    | 13.3 (13.2 to 13.4; 2 %)       | 27.4 (27.3 to 27.4; 0 %)       | 2.0 (2.0 to 2.0; 1 %)          |
| veh.cluster.wheels.ticks                | 13.2 (13.2 to 13.5; 2 %)       | 27.4 (27.4 to 27.4; 0 %)       | 2.0 (2.0 to 2.0; 1 %)          |
| veh.cluster.WheelDiagnostics.wheelTicks | not built                      | not built                      | not built                      |
| veh.hvac.cabin.cabinTemp                | 14.3 (14.3 to 14.3; 0 %)       | 24.0 (23.9 to 24.1; 1 %)       | 13.2 (13.2 to 13.3; 0 %)       |
| veh.hvac.cabin.targetTemp               | 14.3 (14.3 to 14.3; 1 %)       | 24.0 (23.8 to 24.1; 1 %)       | 13.3 (13.2 to 13.3; 0 %)       |
| veh.hvac.cabin.blowerDuty               | 14.3 (14.3 to 14.3; 0 %)       | 24.0 (23.9 to 24.1; 1 %)       | 13.2 (13.2 to 13.3; 1 %)       |
| veh.hvac.cabin.fanLevel                 | 13.3 (13.2 to 13.5; 2 %)       | 27.4 (27.4 to 27.4; 0 %)       | 2.0 (2.0 to 2.0; 1 %)          |
| veh.hvac.cabin.compressorLoad           | 14.3 (14.3 to 14.3; 0 %)       | 24.0 (23.9 to 24.1; 1 %)       | 13.2 (13.2 to 13.3; 1 %)       |
| veh.cabin.Cabin.temperature             | 13.3 (13.2 to 13.4; 1 %)       | 27.0 (26.8 to 27.0; 1 %)       | 2.0 (2.0 to 2.1; 1 %)          |
| veh.cabin.Horn.active                   | 15.4 (15.4 to 15.5; 0 %)       | 27.7 (27.7 to 27.8; 1 %)       | 6.1 (6.1 to 6.2; 2 %)          |
| bench.stress.Stress.longString          | 130.4 (129.8 to 133.5; 3 %)    | 140.1 (139.6 to 140.4; 1 %)    | 118.8 (117.6 to 119.8; 2 %)    |
| bench.stress.Stress.itemList            | 1348.2 (1344.3 to 1351.3; 1 %) | 2133.7 (2129.1 to 2138.1; 0 %) | 1258.0 (1257.6 to 1264.2; 1 %) |
| bench.stress.Stress.nestedOptional      | 156.4 (155.8 to 157.1; 1 %)    | 198.9 (198.6 to 199.5; 0 %)    | 137.4 (137.1 to 138.2; 1 %)    |
| bench.stress.Stress.map                 | 1301.4 (1300.7 to 1302.5; 0 %) | 1977.2 (1966.8 to 1989.9; 1 %) | 1234.6 (1230.8 to 1237.0; 0 %) |

##### CPU load per operation

CPU load in percent of one core at 4000 updates per second of each operation
alone, derived from the median time of that operation. The three operations of
an encoding add up to its derived load of one update.

| Payload                                 | Encode: FlatBuffers | Encode: proto3 | Encode: podbuf | Verify: FlatBuffers | Verify: proto3 | Verify: podbuf | Decode: FlatBuffers | Decode: proto3 | Decode: podbuf |
| --------------------------------------- | ------------------- | -------------- | -------------- | ------------------- | -------------- | -------------- | ------------------- | -------------- | -------------- |
| veh.cluster.VehicleStatus.currentSpeed  | 0.0325              | 0.0106         | 0.0116         | 0.0262              | 0.0267         | 0.0255         | 0.0057              | 0.0096         | 0.0053         |
| veh.cluster.VehicleStatus.engineTemp    | 0.0278              | 0.0102         | 0.0116         | 0.0283              | 0.0285         | 0.0274         | 0.0057              | 0.0096         | 0.0053         |
| veh.cluster.VehicleStatus.warnings      | 0.0280              | 0.0110         | 0.0111         | 0.0065              | 0.0087         | 0.0035         | 0.0054              | 0.0114         | 0.0008         |
| veh.cluster.WheelHistory.wheelTicks     | 0.0285              | 0.0110         | 0.0109         | 0.0065              | 0.0088         | 0.0035         | 0.0053              | 0.0110         | 0.0008         |
| veh.cluster.WheelHistory.wheelTemp      | 0.0278              | 0.0102         | 0.0116         | 0.0283              | 0.0286         | 0.0275         | 0.0057              | 0.0096         | 0.0053         |
| veh.cluster.WheelHistory.wheelSlipRate  | 0.0282              | 0.0102         | 0.0116         | 0.0273              | 0.0283         | 0.0277         | 0.0057              | 0.0096         | 0.0053         |
| veh.cluster.WheelSummary.wheelHealth    | 0.0283              | 0.0110         | 0.0109         | 0.0065              | 0.0088         | 0.0035         | 0.0053              | 0.0110         | 0.0008         |
| veh.cluster.wheels.ticks                | 0.0282              | 0.0110         | 0.0109         | 0.0065              | 0.0088         | 0.0035         | 0.0053              | 0.0110         | 0.0008         |
| veh.cluster.WheelDiagnostics.wheelTicks | not built           | not built      | not built      | not built           | not built      | not built      | not built           | not built      | not built      |
| veh.hvac.cabin.cabinTemp                | 0.0277              | 0.0102         | 0.0116         | 0.0283              | 0.0286         | 0.0274         | 0.0057              | 0.0096         | 0.0053         |
| veh.hvac.cabin.targetTemp               | 0.0278              | 0.0101         | 0.0116         | 0.0283              | 0.0285         | 0.0274         | 0.0057              | 0.0096         | 0.0053         |
| veh.hvac.cabin.blowerDuty               | 0.0278              | 0.0101         | 0.0116         | 0.0273              | 0.0283         | 0.0276         | 0.0057              | 0.0096         | 0.0053         |
| veh.hvac.cabin.fanLevel                 | 0.0281              | 0.0110         | 0.0109         | 0.0065              | 0.0088         | 0.0035         | 0.0053              | 0.0110         | 0.0008         |
| veh.hvac.cabin.compressorLoad           | 0.0279              | 0.0102         | 0.0116         | 0.0273              | 0.0283         | 0.0274         | 0.0057              | 0.0096         | 0.0053         |
| veh.cabin.Cabin.temperature             | 0.0322              | 0.0116         | 0.0111         | 0.0065              | 0.0090         | 0.0034         | 0.0053              | 0.0108         | 0.0008         |
| veh.cabin.Horn.active                   | 0.0308              | 0.0098         | 0.0112         | 0.0069              | 0.0087         | 0.0034         | 0.0062              | 0.0111         | 0.0024         |
| bench.stress.Stress.longString          | 0.2269              | 0.0228         | 0.2367         | 0.0899              | 0.0946         | 0.1124         | 0.0522              | 0.0561         | 0.0475         |
| bench.stress.Stress.itemList            | 0.5544              | 0.4289         | 0.3200         | 0.3448              | 0.5900         | 0.3937         | 0.5393              | 0.8535         | 0.5032         |
| bench.stress.Stress.nestedOptional      | 0.0841              | 0.0530         | 0.0394         | 0.0425              | 0.0601         | 0.0435         | 0.0626              | 0.0796         | 0.0549         |
| bench.stress.Stress.map                 | 0.5549              | 0.8967         | 0.4358         | 2.1215              | 3.4817         | 0.6387         | 0.5206              | 0.7909         | 0.4938         |

##### Ratios per operation

Time of each operation of each encoding divided by the time of the same
operation in podbuf: above 1.00 the encoding is slower than podbuf in that
operation, below 1.00 faster. A refused or not-built cell has no ratio and is
marked. A time under about 10 ns includes harness cost and is an upper bound, so
a ratio whose podbuf time is under 10 ns is not comparable.

| Payload                                 | Encode: FlatBuffers / podbuf | Encode: proto3 / podbuf | Verify: FlatBuffers / podbuf | Verify: proto3 / podbuf | Decode: FlatBuffers / podbuf | Decode: proto3 / podbuf |
| --------------------------------------- | ---------------------------- | ----------------------- | ---------------------------- | ----------------------- | ---------------------------- | ----------------------- |
| veh.cluster.VehicleStatus.currentSpeed  | 2.80                         | 0.91                    | 1.02                         | 1.05                    | 1.07                         | 1.80                    |
| veh.cluster.VehicleStatus.engineTemp    | 2.40                         | 0.88                    | 1.03                         | 1.04                    | 1.08                         | 1.81                    |
| veh.cluster.VehicleStatus.warnings      | 2.54                         | 1.00                    | 1.86                         | 2.51                    | 6.38                         | 13.49                   |
| veh.cluster.WheelHistory.wheelTicks     | 2.62                         | 1.01                    | 1.85                         | 2.50                    | 6.52                         | 13.47                   |
| veh.cluster.WheelHistory.wheelTemp      | 2.40                         | 0.88                    | 1.03                         | 1.04                    | 1.08                         | 1.81                    |
| veh.cluster.WheelHistory.wheelSlipRate  | 2.43                         | 0.88                    | 0.99                         | 1.02                    | 1.08                         | 1.81                    |
| veh.cluster.WheelSummary.wheelHealth    | 2.61                         | 1.01                    | 1.85                         | 2.51                    | 6.53                         | 13.48                   |
| veh.cluster.wheels.ticks                | 2.60                         | 1.01                    | 1.85                         | 2.50                    | 6.51                         | 13.45                   |
| veh.cluster.WheelDiagnostics.wheelTicks | not built                    | not built               | not built                    | not built               | not built                    | not built               |
| veh.hvac.cabin.cabinTemp                | 2.39                         | 0.88                    | 1.03                         | 1.04                    | 1.08                         | 1.81                    |
| veh.hvac.cabin.targetTemp               | 2.40                         | 0.87                    | 1.03                         | 1.04                    | 1.08                         | 1.81                    |
| veh.hvac.cabin.blowerDuty               | 2.40                         | 0.87                    | 0.99                         | 1.02                    | 1.08                         | 1.81                    |
| veh.hvac.cabin.fanLevel                 | 2.58                         | 1.01                    | 1.85                         | 2.51                    | 6.52                         | 13.47                   |
| veh.hvac.cabin.compressorLoad           | 2.40                         | 0.88                    | 1.00                         | 1.03                    | 1.08                         | 1.82                    |
| veh.cabin.Cabin.temperature             | 2.91                         | 1.05                    | 1.90                         | 2.63                    | 6.48                         | 13.16                   |
| veh.cabin.Horn.active                   | 2.74                         | 0.87                    | 2.03                         | 2.56                    | 2.54                         | 4.57                    |
| bench.stress.Stress.longString          | 0.96                         | 0.10                    | 0.80                         | 0.84                    | 1.10                         | 1.18                    |
| bench.stress.Stress.itemList            | 1.73                         | 1.34                    | 0.88                         | 1.50                    | 1.07                         | 1.70                    |
| bench.stress.Stress.nestedOptional      | 2.14                         | 1.35                    | 0.98                         | 1.38                    | 1.14                         | 1.45                    |
| bench.stress.Stress.map                 | 1.27                         | 2.06                    | 3.32                         | 5.45                    | 1.05                         | 1.60                    |

##### CPU load, paced and derived

CPU load in percent of one core at 4000 updates per second: paced (process CPU
time over wall time, includes the cost of sleep and wake-up) next to the derived
figure, and their difference.

| Payload                                | Encoding    | Paced % | Derived % | Difference |
| -------------------------------------- | ----------- | ------- | --------- | ---------- |
| veh.cluster.VehicleStatus.currentSpeed | FlatBuffers | 2.1041  | 0.0643    | 2.0398     |
| veh.cluster.VehicleStatus.currentSpeed | proto3      | 2.1146  | 0.0469    | 2.0677     |
| veh.cluster.VehicleStatus.currentSpeed | podbuf      | 2.0697  | 0.0425    | 2.0273     |
| veh.cluster.VehicleStatus.engineTemp   | FlatBuffers | 2.1016  | 0.0619    | 2.0397     |
| veh.cluster.VehicleStatus.engineTemp   | proto3      | 2.1612  | 0.0483    | 2.1128     |
| veh.cluster.VehicleStatus.engineTemp   | podbuf      | 2.0321  | 0.0443    | 1.9878     |
| veh.cluster.VehicleStatus.warnings     | FlatBuffers | 2.0897  | 0.0399    | 2.0498     |
| veh.cluster.VehicleStatus.warnings     | proto3      | 1.9924  | 0.0312    | 1.9613     |
| veh.cluster.VehicleStatus.warnings     | podbuf      | 1.9485  | 0.0154    | 1.9331     |
| veh.cluster.WheelHistory.wheelTicks    | FlatBuffers | 2.0577  | 0.0402    | 2.0175     |
| veh.cluster.WheelHistory.wheelTicks    | proto3      | 2.0299  | 0.0307    | 1.9992     |
| veh.cluster.WheelHistory.wheelTicks    | podbuf      | 1.9446  | 0.0152    | 1.9294     |
| veh.cluster.WheelHistory.wheelTemp     | FlatBuffers | 2.1251  | 0.0618    | 2.0633     |
| veh.cluster.WheelHistory.wheelTemp     | proto3      | 2.2424  | 0.0483    | 2.1940     |
| veh.cluster.WheelHistory.wheelTemp     | podbuf      | 2.0366  | 0.0444    | 1.9921     |
| veh.cluster.WheelHistory.wheelSlipRate | FlatBuffers | 2.1938  | 0.0612    | 2.1325     |
| veh.cluster.WheelHistory.wheelSlipRate | proto3      | 2.2098  | 0.0481    | 2.1618     |
| veh.cluster.WheelHistory.wheelSlipRate | podbuf      | 2.0840  | 0.0445    | 2.0395     |
| veh.cluster.WheelSummary.wheelHealth   | FlatBuffers | 2.0630  | 0.0401    | 2.0229     |
| veh.cluster.WheelSummary.wheelHealth   | proto3      | 2.0299  | 0.0307    | 1.9992     |
| veh.cluster.WheelSummary.wheelHealth   | podbuf      | 2.0327  | 0.0152    | 2.0175     |
| veh.cluster.wheels.ticks               | FlatBuffers | 2.1263  | 0.0400    | 2.0863     |
| veh.cluster.wheels.ticks               | proto3      | 2.1086  | 0.0307    | 2.0779     |
| veh.cluster.wheels.ticks               | podbuf      | 2.0177  | 0.0152    | 2.0025     |
| veh.hvac.cabin.cabinTemp               | FlatBuffers | 2.1748  | 0.0618    | 2.1130     |
| veh.hvac.cabin.cabinTemp               | proto3      | 2.1397  | 0.0483    | 2.0914     |
| veh.hvac.cabin.cabinTemp               | podbuf      | 2.1082  | 0.0443    | 2.0639     |
| veh.hvac.cabin.targetTemp              | FlatBuffers | 2.1955  | 0.0619    | 2.1336     |
| veh.hvac.cabin.targetTemp              | proto3      | 2.1362  | 0.0482    | 2.0879     |
| veh.hvac.cabin.targetTemp              | podbuf      | 2.1167  | 0.0443    | 2.0724     |
| veh.hvac.cabin.blowerDuty              | FlatBuffers | 2.1679  | 0.0609    | 2.1070     |
| veh.hvac.cabin.blowerDuty              | proto3      | 2.1431  | 0.0480    | 2.0950     |
| veh.hvac.cabin.blowerDuty              | podbuf      | 2.0961  | 0.0445    | 2.0516     |
| veh.hvac.cabin.fanLevel                | FlatBuffers | 2.1315  | 0.0399    | 2.0916     |
| veh.hvac.cabin.fanLevel                | proto3      | 2.0964  | 0.0307    | 2.0657     |
| veh.hvac.cabin.fanLevel                | podbuf      | 2.0304  | 0.0152    | 2.0153     |
| veh.hvac.cabin.compressorLoad          | FlatBuffers | 2.2036  | 0.0609    | 2.1427     |
| veh.hvac.cabin.compressorLoad          | proto3      | 2.1973  | 0.0481    | 2.1492     |
| veh.hvac.cabin.compressorLoad          | podbuf      | 2.1474  | 0.0443    | 2.1031     |
| veh.cabin.Cabin.temperature            | FlatBuffers | 4.0808  | 0.0440    | 4.0368     |
| veh.cabin.Cabin.temperature            | proto3      | 4.1529  | 0.0313    | 4.1216     |
| veh.cabin.Cabin.temperature            | podbuf      | 3.9824  | 0.0153    | 3.9671     |
| veh.cabin.Horn.active                  | FlatBuffers | 2.1413  | 0.0439    | 2.0974     |
| veh.cabin.Horn.active                  | proto3      | 2.1201  | 0.0296    | 2.0906     |
| veh.cabin.Horn.active                  | podbuf      | 2.0975  | 0.0171    | 2.0805     |
| bench.stress.Stress.longString         | FlatBuffers | 11.4172 | 0.3689    | 11.0483    |
| bench.stress.Stress.longString         | proto3      | 10.0772 | 0.1735    | 9.9037     |
| bench.stress.Stress.longString         | podbuf      | 11.9343 | 0.3966    | 11.5377    |
| bench.stress.Stress.itemList           | FlatBuffers | 9.3702  | 1.4385    | 7.9317     |
| bench.stress.Stress.itemList           | proto3      | 9.9258  | 1.8724    | 8.0534     |
| bench.stress.Stress.itemList           | podbuf      | 7.9568  | 1.2168    | 6.7400     |
| bench.stress.Stress.nestedOptional     | FlatBuffers | 2.7593  | 0.1891    | 2.5702     |
| bench.stress.Stress.nestedOptional     | proto3      | 2.8471  | 0.1927    | 2.6545     |
| bench.stress.Stress.nestedOptional     | podbuf      | 2.6711  | 0.1378    | 2.5333     |
| bench.stress.Stress.map                | FlatBuffers | 15.1831 | 3.1969    | 11.9862    |
| bench.stress.Stress.map                | proto3      | 21.2901 | 5.1692    | 16.1209    |
| bench.stress.Stress.map                | podbuf      | 9.1421  | 1.5684    | 7.5737     |

#### Payloads not built

The bench could not build a value of these payloads, so no encoding was tried on
them. This is not an encoding refusal.

| Payload                                 | Payload type | Reason                                                                                                           |
| --------------------------------------- | ------------ | ---------------------------------------------------------------------------------------------------------------- |
| veh.cluster.WheelDiagnostics.wheelTicks | RawTickCount | the payload type RawTickCount is internal to its package, so a value cannot be built outside the generated crate |

#### Refused cells

None.
