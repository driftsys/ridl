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
  one: the paced figure of the signal payloads ranges from 1.00 % to 4.51 % in
  the host tables, from 8.6 % to 11.6 % on the Pixel 5 A55 core and from 2.0 %
  to 2.2 % on the A76 core, where the derived load is at most 0.0142 %, 0.181 %
  and 0.061 %, so the update is a small part of what the paced run measures. A
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
  the signal payloads it varies from 1.00 % to 4.51 % in the host tables, where
  the derived load is at most 0.0142 %, so it is dominated by something other
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

## Alternatives considered

- **Per-operation microbenchmarks only** (in the style of criterion). Not the
  headline, because the question is the CPU load under a sustained rate, which
  includes cache and allocator effects. The time per operation and per update is
  still reported, so the load at any rate can be computed.
- **Measuring through `ridl-transport-ws`.** Not done: the bench covers the
  codec only. The result can be extended to the transport later.
- **Host-only measurement.** Not enough: the absolute times of the development
  machine do not transfer to a target-class core, so the bench also runs on the
  Pixel 5.
- **Hardware counters on the Pixel 5.** Not used: the phone is not rooted, and
  the counters are usually unavailable without root.

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
67 of the 81 update cells and 141 of the 243 operation cells (and 5 of the 8
access cells). The numbers show the shape of the work, not the load on a
vehicle's computer. One run is a snapshot: the host run was not repeated under
the same settings, so this note does not give the run-to-run difference of an
unpinned host.

- Date: 2026-11-08
- Commit: ec63ebf8
- Command:
  `cargo run --release -p encoding-bench -- --out results/host-macos-arm64.json --core-label "development machine (macOS arm64, not pinned)" --duration 0.5 --paced`,
  then `cargo xtask bench report`
- Toolchain: rustc 1.98.1 (48a229cea 2026-09-01)
- Run: 4 000 updates per second, 30 samples per cell, 0.5 s of CPU time per
  operation, cell and encoding, paced run included, fixed values (no `--vary`)

What the tables show, without a recommendation:

- Size: proto3 is the smallest on all 15 signal rows (2 or 5 bytes, against 16
  for podbuf and 24 for FlatBuffers, 32 for `veh.cabin.Horn.active`). It is also
  the smallest on 11 of the 12 stress rows. The exception is `bigSamples`, where
  podbuf (3 016 bytes) and FlatBuffers (3 024 bytes) are smaller than proto3 (4
  089 bytes). FlatBuffers is the largest on the other 11 stress rows. The gap
  between proto3 and podbuf is small on the strings (2.7 % on `longString`, 515
  and 529 bytes; 0.5 % on `bigString`, 3 003 and 3 017 bytes) and large on
  structs, maps and sparse messages (`deepSparse`: 13 and 182 bytes; `bigMap`: 1
  676 and 2 896 bytes; `variant`: 16 and 57 bytes).
- Time per update: podbuf has the lowest median on all 15 signal rows and on 10
  of the 12 stress rows. On `deepSparse` its lead over proto3 is 0.02 % (187.30
  against 187.35 ns), and both cells are flagged. proto3 has the lowest median
  on `longString` and `bigString`, where podbuf has the highest (162 ns against
  158 ns for FlatBuffers and 150 ns for proto3; 646 ns against 477 ns and 447
  ns). The largest gaps are on `bigSamples` (podbuf 1.8 us, FlatBuffers 3.5 us,
  proto3 13.9 us) and `bigMap` (podbuf 10.8 us, FlatBuffers 87.6 us, proto3
  179.2 us).
- Load: the largest load of a signal payload is 0.0142 % of one core at 4 000
  updates per second (FlatBuffers, `veh.cluster.WheelHistory.wheelTemp`). The
  largest of all cells is 71.7 % (proto3, `bigMap`, 179.2 us per update); the
  next is 35.0 % (FlatBuffers, `bigMap`). No other cell is above 10 %.
- Per operation, over the 15 signals (the median over the signals of each
  signal's median): encode 18.7 ns for FlatBuffers, 8.9 ns for proto3 and 6.2 ns
  for podbuf; verify 13.1, 14.6 and 11.6 ns; decode 2.7, 5.7 and 1.1 ns. The
  decode times are below 10 ns, so they are upper bounds that are not comparable
  (method section). Many operation cells are flagged on this machine
  (FlatBuffers encode: 13 of 15 signals; proto3 encode: 10 of 15), so the ratios
  below are not reliable to better than the flagged spread.
- Access: reading one number through the view of verified bytes takes 11.3 ns
  with podbuf and 5 113 ns with FlatBuffers on `bigItemList`, and 15.5 ns and 3
  217 ns on `deepNested`: the FlatBuffers accessor of a list decodes the whole
  list. On `deepSparse` and `deepFull` the access takes 6.7 and 7.2 ns with
  podbuf (upper bounds, below 10 ns) and 17.4 and 18.3 ns with FlatBuffers. The
  podbuf cells of `bigItemList`, `deepNested` and `deepSparse` and the
  FlatBuffers cells of `deepSparse` and `deepFull` are flagged, so the 6.7 ns
  podbuf figure of `deepSparse` is flagged as well as below 10 ns.
- Map keys: from `map` (at most 16 entries) to `bigMap` (at most 120 entries,
  7.5 times as many) the verify time grows 45.8 times for FlatBuffers and 46.9
  times for proto3, and 7.6 times for podbuf. On `bigMap` verify is 90 % of the
  update for FlatBuffers, 81 % for proto3 and 27 % for podbuf. The codec records
  of FlatBuffers and proto3 state that the uniqueness check of map keys compares
  pairs of keys, which is quadratic in the number of entries; the growth is
  consistent with that, and the tables do not isolate the check.
- Large integer list: on `bigSamples` the podbuf verify takes 6.1 ns (flagged,
  an upper bound), against 1 619 ns for FlatBuffers and 1 523 ns for proto3; the
  proto3 encode takes 10 233 ns, against 844 ns for podbuf and 550 ns for
  FlatBuffers. The element type `Sample` has the full range of a 16-bit unsigned
  integer, so podbuf verify has no per-element range to check there. The time
  that this takes on the Pixel 5 is in the section on the podbuf load change.
- Allocations: the codecs make no allocation for the signal payloads, and verify
  makes none for any payload. Decode allocates for every stress payload (1 for
  the strings, `variant` and `deepSparse`; 101 to 126 for `bigItemList` and
  `bigMap`). Encode allocates for FlatBuffers (once for `itemList`, `map`,
  `bigItemList`, `bigSamples` and `bigMap`; 11 times for `deepNested`), for
  podbuf (once for `map` and `bigMap`) and for proto3 never; the cause of the
  podbuf allocations is not established here.
- Paced against derived: the paced figure is 1.00 % to 4.51 % for the signal
  payloads and the derived figure is at most 0.0142 %; the paced figure is above
  the derived one in every cell. The paced figure is not a codec figure, as the
  method section explains.

### Fixed and changing values

A second host run with `--vary` (64 distinct values per call for the seven
payloads that have a pool) was made at commit fc7885e3, one commit before this
one, without the paced run. It is not part of the tables. Against the run above,
the median time of the 251 operation cells (the 243 of encode, verify and decode
and the 8 of access) differs by a median of 0.7 %, and the largest difference is
71 % (podbuf verify of `bigSamples`, 6.1 ns and 10.4 ns, both near the 10 ns
limit). The next largest are 39 % (podbuf encode of `bigMap`) and 34 %
(FlatBuffers verify of `veh.cabin.Cabin.temperature`, 8.2 ns and 5.4 ns). The
median time of one update differs by a median of 0.8 % over the 81 cells, with
17 % at most. For the 71 cells of the seven payloads that have a pool the median
difference is 1.1 %; for the 180 cells of the other payloads, which repeat the
same value in both runs, it is 0.7 % and the largest is 34 %. The difference
between the two modes on this unpinned host is therefore of the size of the
difference between two runs of the same mode, and the data do not show a cost of
changing values.

The tables are the output of `cargo xtask bench report`, unedited except for the
heading levels. The host tables and the Pixel 5 tables below are each a snapshot
of one run, all three at commit ec63ebf8. The directory
`bench/encodings/results/` is git-ignored, so the JSON behind the tables is not
in the repository. Refresh the host tables by rerunning the command above and
the Pixel tables by rerunning `just bench-pixel`, then run
`cargo xtask bench report`.

### Mix

Equal-weight mix, per signal: each signal counts once, so a payload type that
several signals share counts once for each of them. Only signals whose three
encodings are measured are counted; the last column gives how many.

| Metric                                                 | FlatBuffers | proto3 | podbuf | FlatBuffers / podbuf | proto3 / podbuf | Units |
| ------------------------------------------------------ | ----------- | ------ | ------ | -------------------- | --------------- | ----- |
| Size (bytes)                                           | 24.5        | 3.6    | 16.0   | 1.53                 | 0.23            | 15    |
| Heap allocations per update (encode + verify + decode) | 0.0         | 0.0    | 0.0    | n/a                  | n/a             | 15    |
| Time per update (median ns)                            | 32.7        | 25.1   | 14.5   | 2.26                 | 1.74            | 15    |
| CPU % of one core at 4000 updates per second (derived) | 0.0131      | 0.0100 | 0.0058 | 2.26                 | 1.74            | 15    |

Equal-weight mix, per payload type: each payload type counts once, however many
signals share it (the value of a type is the mean over its signals). Only
payload types whose signals are measured in all three encodings are counted; the
last column gives how many.

| Metric                                                 | FlatBuffers | proto3 | podbuf | FlatBuffers / podbuf | proto3 / podbuf | Units |
| ------------------------------------------------------ | ----------- | ------ | ------ | -------------------- | --------------- | ----- |
| Size (bytes)                                           | 25.3        | 3.4    | 16.0   | 1.58                 | 0.21            | 6     |
| Heap allocations per update (encode + verify + decode) | 0.0         | 0.0    | 0.0    | n/a                  | n/a             | 6     |
| Time per update (median ns)                            | 32.7        | 24.6   | 13.8   | 2.37                 | 1.78            | 6     |
| CPU % of one core at 4000 updates per second (derived) | 0.0131      | 0.0098 | 0.0055 | 2.37                 | 1.78            | 6     |

Equal-weight mix, per signal: each signal counts once, so a payload type that
several signals share counts once for each of them. Only signals whose three
encodings are measured are counted; the last column gives how many. The rows are
per operation. A time under about 10 ns includes harness cost and is an upper
bound, so a ratio whose podbuf time is under 10 ns is not comparable.

| Metric                                                         | FlatBuffers | proto3 | podbuf | FlatBuffers / podbuf | proto3 / podbuf | Units |
| -------------------------------------------------------------- | ----------- | ------ | ------ | -------------------- | --------------- | ----- |
| Encode: time per operation (median ns)                         | 18.7        | 7.9    | 6.2    | 3.01                 | 1.27            | 15    |
| Encode: CPU % of one core at 4000 updates per second (derived) | 0.0075      | 0.0032 | 0.0025 | 3.01                 | 1.27            | 15    |
| Encode: heap allocations                                       | 0.0         | 0.0    | 0.0    | n/a                  | n/a             | 15    |
| Verify: time per operation (median ns)                         | 11.1        | 11.4   | 7.4    | 1.51                 | 1.55            | 15    |
| Verify: CPU % of one core at 4000 updates per second (derived) | 0.0044      | 0.0046 | 0.0029 | 1.51                 | 1.55            | 15    |
| Verify: heap allocations                                       | 0.0         | 0.0    | 0.0    | n/a                  | n/a             | 15    |
| Decode: time per operation (median ns)                         | 2.9         | 5.9    | 0.9    | 3.26                 | 6.56            | 15    |
| Decode: CPU % of one core at 4000 updates per second (derived) | 0.0012      | 0.0023 | 0.0004 | 3.26                 | 6.56            | 15    |
| Decode: heap allocations                                       | 0.0         | 0.0    | 0.0    | n/a                  | n/a             | 15    |

Equal-weight mix, per payload type: each payload type counts once, however many
signals share it (the value of a type is the mean over its signals). Only
payload types whose signals are measured in all three encodings are counted; the
last column gives how many. The rows are per operation. A time under about 10 ns
includes harness cost and is an upper bound, so a ratio whose podbuf time is
under 10 ns is not comparable.

| Metric                                                         | FlatBuffers | proto3 | podbuf | FlatBuffers / podbuf | proto3 / podbuf | Units |
| -------------------------------------------------------------- | ----------- | ------ | ------ | -------------------- | --------------- | ----- |
| Encode: time per operation (median ns)                         | 18.7        | 7.8    | 6.2    | 3.02                 | 1.25            | 6     |
| Encode: CPU % of one core at 4000 updates per second (derived) | 0.0075      | 0.0031 | 0.0025 | 3.02                 | 1.25            | 6     |
| Encode: heap allocations                                       | 0.0         | 0.0    | 0.0    | n/a                  | n/a             | 6     |
| Verify: time per operation (median ns)                         | 11.0        | 10.8   | 6.7    | 1.66                 | 1.62            | 6     |
| Verify: CPU % of one core at 4000 updates per second (derived) | 0.0044      | 0.0043 | 0.0027 | 1.66                 | 1.62            | 6     |
| Verify: heap allocations                                       | 0.0         | 0.0    | 0.0    | n/a                  | n/a             | 6     |
| Decode: time per operation (median ns)                         | 2.9         | 6.0    | 0.9    | 3.17                 | 6.52            | 6     |
| Decode: CPU % of one core at 4000 updates per second (derived) | 0.0012      | 0.0024 | 0.0004 | 3.17                 | 6.52            | 6     |
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
| bench.stress.Stress.bigString           | 3032        | 3003      | 3017      |
| bench.stress.Stress.bigItemList         | 4024        | 1542      | 2516      |
| bench.stress.Stress.bigSamples          | 3024        | 4089      | 3016      |
| bench.stress.Stress.bigMap              | 4344        | 1676      | 2896      |
| bench.stress.Stress.deepNested          | 2264        | 760       | 1372      |
| bench.stress.Stress.variant             | 88          | 16        | 57        |
| bench.stress.Stress.deepSparse          | 240         | 13        | 182       |
| bench.stress.Stress.deepFull            | 408         | 178       | 296       |

#### Heap allocations

Heap allocations of one encode / verify / decode.

| Payload                                 | FlatBuffers | proto3      | podbuf      |
| --------------------------------------- | ----------- | ----------- | ----------- |
| veh.cluster.VehicleStatus.currentSpeed  | 0 / 0 / 0   | 0 / 0 / 0   | 0 / 0 / 0   |
| veh.cluster.VehicleStatus.engineTemp    | 0 / 0 / 0   | 0 / 0 / 0   | 0 / 0 / 0   |
| veh.cluster.VehicleStatus.warnings      | 0 / 0 / 0   | 0 / 0 / 0   | 0 / 0 / 0   |
| veh.cluster.WheelHistory.wheelTicks     | 0 / 0 / 0   | 0 / 0 / 0   | 0 / 0 / 0   |
| veh.cluster.WheelHistory.wheelTemp      | 0 / 0 / 0   | 0 / 0 / 0   | 0 / 0 / 0   |
| veh.cluster.WheelHistory.wheelSlipRate  | 0 / 0 / 0   | 0 / 0 / 0   | 0 / 0 / 0   |
| veh.cluster.WheelSummary.wheelHealth    | 0 / 0 / 0   | 0 / 0 / 0   | 0 / 0 / 0   |
| veh.cluster.wheels.ticks                | 0 / 0 / 0   | 0 / 0 / 0   | 0 / 0 / 0   |
| veh.cluster.WheelDiagnostics.wheelTicks | not built   | not built   | not built   |
| veh.hvac.cabin.cabinTemp                | 0 / 0 / 0   | 0 / 0 / 0   | 0 / 0 / 0   |
| veh.hvac.cabin.targetTemp               | 0 / 0 / 0   | 0 / 0 / 0   | 0 / 0 / 0   |
| veh.hvac.cabin.blowerDuty               | 0 / 0 / 0   | 0 / 0 / 0   | 0 / 0 / 0   |
| veh.hvac.cabin.fanLevel                 | 0 / 0 / 0   | 0 / 0 / 0   | 0 / 0 / 0   |
| veh.hvac.cabin.compressorLoad           | 0 / 0 / 0   | 0 / 0 / 0   | 0 / 0 / 0   |
| veh.cabin.Cabin.temperature             | 0 / 0 / 0   | 0 / 0 / 0   | 0 / 0 / 0   |
| veh.cabin.Horn.active                   | 0 / 0 / 0   | 0 / 0 / 0   | 0 / 0 / 0   |
| bench.stress.Stress.longString          | 0 / 0 / 1   | 0 / 0 / 1   | 0 / 0 / 1   |
| bench.stress.Stress.itemList            | 1 / 0 / 17  | 0 / 0 / 19  | 0 / 0 / 17  |
| bench.stress.Stress.nestedOptional      | 0 / 0 / 2   | 0 / 0 / 2   | 0 / 0 / 2   |
| bench.stress.Stress.map                 | 1 / 0 / 17  | 0 / 0 / 19  | 1 / 0 / 17  |
| bench.stress.Stress.bigString           | 0 / 0 / 1   | 0 / 0 / 1   | 0 / 0 / 1   |
| bench.stress.Stress.bigItemList         | 1 / 0 / 101 | 0 / 0 / 106 | 0 / 0 / 101 |
| bench.stress.Stress.bigSamples          | 1 / 0 / 1   | 0 / 0 / 10  | 0 / 0 / 1   |
| bench.stress.Stress.bigMap              | 1 / 0 / 121 | 0 / 0 / 126 | 1 / 0 / 121 |
| bench.stress.Stress.deepNested          | 11 / 0 / 61 | 0 / 0 / 63  | 0 / 0 / 61  |
| bench.stress.Stress.variant             | 0 / 0 / 1   | 0 / 0 / 1   | 0 / 0 / 1   |
| bench.stress.Stress.deepSparse          | 0 / 0 / 1   | 0 / 0 / 1   | 0 / 0 / 1   |
| bench.stress.Stress.deepFull            | 0 / 0 / 15  | 0 / 0 / 15  | 0 / 0 / 15  |

#### Time per update

CPU time of one update (encode, verify and decode), in ns: the sum of the
medians of the three operations, with the sum of their fastest and the sum of
their slowest sample (an envelope, not a percentile) and its range as a share of
the sum. † spread above 10 %: the figure is not stable.

| Payload                                 | FlatBuffers                          | proto3                                  | podbuf                               |
| --------------------------------------- | ------------------------------------ | --------------------------------------- | ------------------------------------ |
| veh.cluster.VehicleStatus.currentSpeed  | 34.9 (33.8 to 36.3; 7 %)             | 29.4 (28.9 to 32.8; 13 %) †             | 18.8 (18.4 to 19.6; 6 %)             |
| veh.cluster.VehicleStatus.engineTemp    | 34.2 (33.4 to 35.8; 7 %)             | 29.4 (29.1 to 30.4; 4 %)                | 19.2 (19.0 to 20.0; 5 %)             |
| veh.cluster.VehicleStatus.warnings      | 31.3 (25.9 to 34.3; 27 %) †          | 19.8 (19.6 to 21.3; 9 %) †              | 8.9 (8.8 to 10.1; 15 %) †            |
| veh.cluster.WheelHistory.wheelTicks     | 30.0 (29.3 to 33.2; 13 %) †          | 19.5 (19.4 to 20.4; 6 %)                | 8.9 (8.8 to 9.2; 4 %) †              |
| veh.cluster.WheelHistory.wheelTemp      | 35.5 (33.7 to 54.1; 58 %) †          | 29.8 (29.4 to 41.1; 39 %) †             | 19.5 (19.2 to 27.1; 41 %) †          |
| veh.cluster.WheelHistory.wheelSlipRate  | 34.9 (34.0 to 41.2; 21 %) †          | 29.9 (29.4 to 34.0; 15 %) †             | 19.5 (19.2 to 33.4; 73 %) †          |
| veh.cluster.WheelSummary.wheelHealth    | 30.1 (29.5 to 33.8; 14 %) †          | 19.5 (19.4 to 21.6; 12 %) †             | 8.9 (8.8 to 9.2; 5 %)                |
| veh.cluster.wheels.ticks                | 29.8 (28.2 to 34.1; 20 %) †          | 19.5 (19.4 to 23.6; 22 %) †             | 8.9 (8.8 to 9.9; 12 %) †             |
| veh.cluster.WheelDiagnostics.wheelTicks | not built                            | not built                               | not built                            |
| veh.hvac.cabin.cabinTemp                | 34.8 (33.6 to 37.2; 10 %) †          | 29.5 (29.3 to 30.9; 5 %)                | 19.3 (19.2 to 19.7; 3 %)             |
| veh.hvac.cabin.targetTemp               | 34.7 (33.7 to 39.1; 15 %) †          | 29.7 (29.4 to 33.7; 14 %) †             | 19.3 (19.2 to 20.3; 5 %) †           |
| veh.hvac.cabin.blowerDuty               | 34.8 (33.9 to 37.3; 10 %) †          | 29.7 (29.4 to 31.2; 6 %)                | 19.4 (19.2 to 20.4; 6 %)             |
| veh.hvac.cabin.fanLevel                 | 29.8 (26.0 to 33.0; 23 %) †          | 19.5 (19.4 to 21.9; 13 %) †             | 8.8 (8.7 to 8.9; 2 %)                |
| veh.hvac.cabin.compressorLoad           | 34.8 (34.1 to 38.5; 13 %) †          | 29.7 (29.5 to 31.5; 7 %) †              | 19.4 (19.3 to 20.8; 8 %)             |
| veh.cabin.Cabin.temperature             | 30.2 (27.0 to 35.7; 29 %) †          | 20.5 (20.3 to 24.5; 20 %) †             | 8.9 (8.8 to 9.7; 10 %) †             |
| veh.cabin.Horn.active                   | 31.5 (30.4 to 33.9; 11 %) †          | 21.2 (20.5 to 23.2; 13 %) †             | 9.5 (9.4 to 10.5; 11 %) †            |
| bench.stress.Stress.longString          | 157.6 (151.0 to 188.5; 24 %) †       | 149.5 (142.1 to 200.9; 39 %) †          | 162.1 (157.1 to 180.9; 15 %) †       |
| bench.stress.Stress.itemList            | 1633.8 (1576.2 to 1790.1; 13 %) †    | 2105.8 (2040.0 to 2347.9; 15 %) †       | 1338.1 (1287.5 to 1486.5; 15 %) †    |
| bench.stress.Stress.nestedOptional      | 188.3 (179.5 to 223.5; 23 %) †       | 193.0 (181.6 to 261.0; 41 %) †          | 132.3 (127.1 to 145.4; 14 %) †       |
| bench.stress.Stress.map                 | 3018.0 (2939.0 to 3461.8; 17 %) †    | 5052.3 (4896.7 to 5349.8; 9 %)          | 1530.3 (1468.1 to 1639.1; 11 %) †    |
| bench.stress.Stress.bigString           | 476.6 (471.4 to 533.9; 13 %) †       | 446.5 (440.9 to 491.5; 11 %) †          | 646.3 (635.4 to 748.5; 17 %) †       |
| bench.stress.Stress.bigItemList         | 9054.1 (8831.0 to 9873.9; 12 %) †    | 12266.8 (11939.7 to 13816.3; 15 %) †    | 7531.2 (7261.9 to 9039.1; 24 %) †    |
| bench.stress.Stress.bigSamples          | 3469.4 (3374.4 to 4247.1; 25 %) †    | 13919.5 (13652.1 to 15324.7; 12 %) †    | 1819.6 (1766.2 to 2134.1; 20 %) †    |
| bench.stress.Stress.bigMap              | 87557.8 (86042.0 to 94522.1; 10 %) † | 179159.7 (173225.3 to 195554.5; 12 %) † | 10785.1 (10394.6 to 12220.4; 17 %) † |
| bench.stress.Stress.deepNested          | 5886.7 (5724.4 to 6435.8; 12 %) †    | 7254.4 (7041.5 to 8025.7; 14 %) †       | 4483.2 (4359.3 to 4612.0; 6 %) †     |
| bench.stress.Stress.variant             | 133.4 (128.7 to 147.6; 14 %) †       | 119.7 (116.7 to 127.0; 9 %) †           | 75.9 (74.4 to 82.1; 10 %) †          |
| bench.stress.Stress.deepSparse          | 268.4 (257.4 to 301.5; 16 %) †       | 187.3 (180.9 to 231.6; 27 %) †          | 187.3 (183.1 to 234.2; 27 %) †       |
| bench.stress.Stress.deepFull            | 1319.0 (1272.8 to 1453.0; 14 %) †    | 1562.9 (1527.2 to 1762.0; 15 %) †       | 1171.5 (1137.2 to 1221.2; 7 %) †     |

#### CPU load, derived

CPU load in percent of one core at 4000 updates per second, derived from the
median time of one update (the headline figure).

| Payload                                 | FlatBuffers | proto3    | podbuf    |
| --------------------------------------- | ----------- | --------- | --------- |
| veh.cluster.VehicleStatus.currentSpeed  | 0.0139      | 0.0118    | 0.0075    |
| veh.cluster.VehicleStatus.engineTemp    | 0.0137      | 0.0118    | 0.0077    |
| veh.cluster.VehicleStatus.warnings      | 0.0125      | 0.0079    | 0.0035    |
| veh.cluster.WheelHistory.wheelTicks     | 0.0120      | 0.0078    | 0.0035    |
| veh.cluster.WheelHistory.wheelTemp      | 0.0142      | 0.0119    | 0.0078    |
| veh.cluster.WheelHistory.wheelSlipRate  | 0.0140      | 0.0120    | 0.0078    |
| veh.cluster.WheelSummary.wheelHealth    | 0.0120      | 0.0078    | 0.0035    |
| veh.cluster.wheels.ticks                | 0.0119      | 0.0078    | 0.0036    |
| veh.cluster.WheelDiagnostics.wheelTicks | not built   | not built | not built |
| veh.hvac.cabin.cabinTemp                | 0.0139      | 0.0118    | 0.0077    |
| veh.hvac.cabin.targetTemp               | 0.0139      | 0.0119    | 0.0077    |
| veh.hvac.cabin.blowerDuty               | 0.0139      | 0.0119    | 0.0078    |
| veh.hvac.cabin.fanLevel                 | 0.0119      | 0.0078    | 0.0035    |
| veh.hvac.cabin.compressorLoad           | 0.0139      | 0.0119    | 0.0078    |
| veh.cabin.Cabin.temperature             | 0.0121      | 0.0082    | 0.0036    |
| veh.cabin.Horn.active                   | 0.0126      | 0.0085    | 0.0038    |
| bench.stress.Stress.longString          | 0.0630      | 0.0598    | 0.0649    |
| bench.stress.Stress.itemList            | 0.6535      | 0.8423    | 0.5352    |
| bench.stress.Stress.nestedOptional      | 0.0753      | 0.0772    | 0.0529    |
| bench.stress.Stress.map                 | 1.2072      | 2.0209    | 0.6121    |
| bench.stress.Stress.bigString           | 0.1906      | 0.1786    | 0.2585    |
| bench.stress.Stress.bigItemList         | 3.6216      | 4.9067    | 3.0125    |
| bench.stress.Stress.bigSamples          | 1.3878      | 5.5678    | 0.7278    |
| bench.stress.Stress.bigMap              | 35.0231     | 71.6639   | 4.3140    |
| bench.stress.Stress.deepNested          | 2.3547      | 2.9018    | 1.7933    |
| bench.stress.Stress.variant             | 0.0534      | 0.0479    | 0.0304    |
| bench.stress.Stress.deepSparse          | 0.1074      | 0.0749    | 0.0749    |
| bench.stress.Stress.deepFull            | 0.5276      | 0.6252    | 0.4686    |

#### Ratios to podbuf

Each encoding divided by podbuf: above 1.00 the encoding is larger or slower
than podbuf, below 1.00 smaller or faster. The CPU ratios equal the time ratios,
because the derived CPU load is proportional to the median time. A refused or
not-built cell has no ratio and is marked.

| Payload                                 | Size: FlatBuffers / podbuf | Size: proto3 / podbuf | Time: FlatBuffers / podbuf | Time: proto3 / podbuf | CPU: FlatBuffers / podbuf | CPU: proto3 / podbuf |
| --------------------------------------- | -------------------------- | --------------------- | -------------------------- | --------------------- | ------------------------- | -------------------- |
| veh.cluster.VehicleStatus.currentSpeed  | 1.50                       | 0.31                  | 1.85                       | 1.56                  | 1.85                      | 1.56                 |
| veh.cluster.VehicleStatus.engineTemp    | 1.50                       | 0.31                  | 1.78                       | 1.53                  | 1.78                      | 1.53                 |
| veh.cluster.VehicleStatus.warnings      | 1.50                       | 0.12                  | 3.53                       | 2.24                  | 3.53                      | 2.24                 |
| veh.cluster.WheelHistory.wheelTicks     | 1.50                       | 0.12                  | 3.39                       | 2.20                  | 3.39                      | 2.20                 |
| veh.cluster.WheelHistory.wheelTemp      | 1.50                       | 0.31                  | 1.82                       | 1.53                  | 1.82                      | 1.53                 |
| veh.cluster.WheelHistory.wheelSlipRate  | 1.50                       | 0.31                  | 1.79                       | 1.54                  | 1.79                      | 1.54                 |
| veh.cluster.WheelSummary.wheelHealth    | 1.50                       | 0.12                  | 3.40                       | 2.21                  | 3.40                      | 2.21                 |
| veh.cluster.wheels.ticks                | 1.50                       | 0.12                  | 3.36                       | 2.20                  | 3.36                      | 2.20                 |
| veh.cluster.WheelDiagnostics.wheelTicks | not built                  | not built             | not built                  | not built             | not built                 | not built            |
| veh.hvac.cabin.cabinTemp                | 1.50                       | 0.31                  | 1.81                       | 1.53                  | 1.81                      | 1.53                 |
| veh.hvac.cabin.targetTemp               | 1.50                       | 0.31                  | 1.80                       | 1.54                  | 1.80                      | 1.54                 |
| veh.hvac.cabin.blowerDuty               | 1.50                       | 0.31                  | 1.79                       | 1.53                  | 1.79                      | 1.53                 |
| veh.hvac.cabin.fanLevel                 | 1.50                       | 0.12                  | 3.39                       | 2.22                  | 3.39                      | 2.22                 |
| veh.hvac.cabin.compressorLoad           | 1.50                       | 0.31                  | 1.79                       | 1.53                  | 1.79                      | 1.53                 |
| veh.cabin.Cabin.temperature             | 1.50                       | 0.12                  | 3.39                       | 2.31                  | 3.39                      | 2.31                 |
| veh.cabin.Horn.active                   | 2.00                       | 0.12                  | 3.31                       | 2.24                  | 3.31                      | 2.24                 |
| bench.stress.Stress.longString          | 1.03                       | 0.97                  | 0.97                       | 0.92                  | 0.97                      | 0.92                 |
| bench.stress.Stress.itemList            | 1.50                       | 0.55                  | 1.22                       | 1.57                  | 1.22                      | 1.57                 |
| bench.stress.Stress.nestedOptional      | 1.63                       | 0.35                  | 1.42                       | 1.46                  | 1.42                      | 1.46                 |
| bench.stress.Stress.map                 | 1.56                       | 0.53                  | 1.97                       | 3.30                  | 1.97                      | 3.30                 |
| bench.stress.Stress.bigString           | 1.00                       | 1.00                  | 0.74                       | 0.69                  | 0.74                      | 0.69                 |
| bench.stress.Stress.bigItemList         | 1.60                       | 0.61                  | 1.20                       | 1.63                  | 1.20                      | 1.63                 |
| bench.stress.Stress.bigSamples          | 1.00                       | 1.36                  | 1.91                       | 7.65                  | 1.91                      | 7.65                 |
| bench.stress.Stress.bigMap              | 1.50                       | 0.58                  | 8.12                       | 16.61                 | 8.12                      | 16.61                |
| bench.stress.Stress.deepNested          | 1.65                       | 0.55                  | 1.31                       | 1.62                  | 1.31                      | 1.62                 |
| bench.stress.Stress.variant             | 1.54                       | 0.28                  | 1.76                       | 1.58                  | 1.76                      | 1.58                 |
| bench.stress.Stress.deepSparse          | 1.32                       | 0.07                  | 1.43                       | 1.00                  | 1.43                      | 1.00                 |
| bench.stress.Stress.deepFull            | 1.38                       | 0.60                  | 1.13                       | 1.33                  | 1.13                      | 1.33                 |

#### Time per operation, encode

CPU time of the encode operation of one update, in ns: the median of 30 samples
of a batch of this operation alone, with the fastest and the slowest sample and
the range as a share of the median. † spread above 10 %: the figure is not
stable.

| Payload                                 | FlatBuffers                       | proto3                               | podbuf                            |
| --------------------------------------- | --------------------------------- | ------------------------------------ | --------------------------------- |
| veh.cluster.VehicleStatus.currentSpeed  | 19.1 (18.6 to 19.9; 7 %)          | 9.2 (9.0 to 9.8; 8 %)                | 6.1 (6.0 to 6.4; 7 %)             |
| veh.cluster.VehicleStatus.engineTemp    | 18.4 (17.7 to 19.6; 10 %)         | 8.9 (8.8 to 9.4; 7 %)                | 6.1 (6.1 to 6.3; 4 %)             |
| veh.cluster.VehicleStatus.warnings      | 18.5 (18.2 to 21.0; 16 %) †       | 6.5 (6.4 to 7.4; 15 %) †             | 6.1 (6.1 to 6.4; 5 %)             |
| veh.cluster.WheelHistory.wheelTicks     | 18.5 (18.1 to 21.2; 17 %) †       | 6.4 (6.4 to 6.6; 4 %)                | 6.1 (6.1 to 6.3; 3 %)             |
| veh.cluster.WheelHistory.wheelTemp      | 19.4 (18.0 to 32.8; 76 %) †       | 9.1 (8.8 to 10.6; 19 %) †            | 6.2 (6.1 to 9.5; 55 %) †          |
| veh.cluster.WheelHistory.wheelSlipRate  | 18.7 (18.0 to 20.9; 15 %) †       | 9.1 (8.8 to 9.8; 11 %) †             | 6.5 (6.4 to 7.6; 19 %) †          |
| veh.cluster.WheelSummary.wheelHealth    | 18.5 (18.3 to 21.4; 17 %) †       | 6.4 (6.4 to 7.2; 13 %) †             | 6.1 (6.1 to 6.4; 5 %)             |
| veh.cluster.wheels.ticks                | 18.5 (17.7 to 21.4; 20 %) †       | 6.4 (6.4 to 8.8; 39 %) †             | 6.2 (6.1 to 6.6; 9 %)             |
| veh.cluster.WheelDiagnostics.wheelTicks | not built                         | not built                            | not built                         |
| veh.hvac.cabin.cabinTemp                | 18.9 (17.8 to 20.1; 12 %) †       | 9.0 (8.9 to 9.4; 6 %)                | 6.1 (6.1 to 6.4; 4 %)             |
| veh.hvac.cabin.targetTemp               | 18.8 (17.9 to 20.4; 13 %) †       | 9.0 (8.9 to 10.4; 17 %) †            | 6.2 (6.1 to 6.4; 4 %)             |
| veh.hvac.cabin.blowerDuty               | 18.7 (17.9 to 19.8; 10 %) †       | 9.0 (8.9 to 9.6; 8 %)                | 6.4 (6.4 to 6.7; 5 %)             |
| veh.hvac.cabin.fanLevel                 | 18.5 (18.1 to 21.4; 18 %) †       | 6.4 (6.4 to 7.4; 15 %) †             | 6.1 (6.1 to 6.2; 2 %)             |
| veh.hvac.cabin.compressorLoad           | 18.7 (18.1 to 20.0; 10 %) †       | 9.0 (8.9 to 9.9; 11 %) †             | 6.5 (6.4 to 6.8; 6 %)             |
| veh.cabin.Cabin.temperature             | 18.7 (18.6 to 23.9; 28 %) †       | 6.8 (6.7 to 8.4; 25 %) †             | 6.2 (6.1 to 6.6; 7 %)             |
| veh.cabin.Horn.active                   | 18.8 (18.1 to 20.1; 11 %) †       | 6.9 (6.4 to 8.4; 29 %) †             | 6.2 (6.1 to 6.5; 6 %)             |
| bench.stress.Stress.longString          | 36.9 (35.2 to 45.5; 28 %) †       | 18.0 (17.8 to 28.6; 59 %) †          | 33.4 (33.1 to 39.7; 20 %) †       |
| bench.stress.Stress.itemList            | 430.6 (415.3 to 498.3; 19 %) †    | 409.7 (400.5 to 464.7; 16 %) †       | 200.1 (196.2 to 226.4; 15 %) †    |
| bench.stress.Stress.nestedOptional      | 64.5 (61.6 to 73.5; 18 %) †       | 38.9 (38.2 to 48.9; 28 %) †          | 29.2 (28.1 to 31.5; 12 %) †       |
| bench.stress.Stress.map                 | 424.0 (411.2 to 511.2; 24 %) †    | 774.6 (751.4 to 825.3; 10 %)         | 336.1 (325.0 to 356.2; 9 %)       |
| bench.stress.Stress.bigString           | 82.1 (80.2 to 101.9; 26 %) †      | 42.1 (41.6 to 47.7; 15 %) †          | 157.4 (153.0 to 189.2; 23 %) †    |
| bench.stress.Stress.bigItemList         | 2234.5 (2199.5 to 2357.7; 7 %)    | 2550.8 (2487.4 to 2710.8; 9 %)       | 987.1 (956.4 to 1143.8; 19 %) †   |
| bench.stress.Stress.bigSamples          | 550.0 (543.7 to 718.0; 32 %) †    | 10232.6 (10018.8 to 11080.9; 10 %) † | 843.9 (832.8 to 908.1; 9 %)       |
| bench.stress.Stress.bigMap              | 2876.4 (2809.1 to 3116.4; 11 %) † | 25901.0 (25486.9 to 28619.6; 12 %) † | 2245.3 (2191.2 to 2601.1; 18 %) † |
| bench.stress.Stress.deepNested          | 1695.5 (1621.0 to 1751.2; 8 %)    | 1467.9 (1444.6 to 2131.5; 47 %) †    | 603.9 (578.0 to 645.1; 11 %) †    |
| bench.stress.Stress.variant             | 62.9 (59.8 to 68.2; 13 %) †       | 27.3 (26.9 to 28.7; 7 %)             | 23.7 (23.0 to 25.4; 10 %) †       |
| bench.stress.Stress.deepSparse          | 73.7 (69.4 to 82.9; 18 %) †       | 46.2 (45.0 to 57.6; 27 %) †          | 31.1 (29.9 to 33.9; 13 %) †       |
| bench.stress.Stress.deepFull            | 285.3 (279.8 to 302.2; 8 %)       | 274.0 (270.9 to 346.8; 28 %) †       | 172.0 (165.6 to 183.6; 10 %) †    |

#### Time per operation, verify

CPU time of the verify operation of one update, in ns: the median of 30 samples
of a batch of this operation alone, with the fastest and the slowest sample and
the range as a share of the median. † spread above 10 %: the figure is not
stable.

| Payload                                 | FlatBuffers                       | proto3                                  | podbuf                            |
| --------------------------------------- | --------------------------------- | --------------------------------------- | --------------------------------- |
| veh.cluster.VehicleStatus.currentSpeed  | 13.1 (12.7 to 13.5; 7 %)          | 14.6 (14.3 to 17.2; 19 %) †             | 11.6 (11.3 to 12.1; 6 %)          |
| veh.cluster.VehicleStatus.engineTemp    | 13.1 (13.0 to 13.5; 3 %)          | 14.9 (14.8 to 15.3; 4 %)                | 12.0 (11.9 to 12.6; 6 %)          |
| veh.cluster.VehicleStatus.warnings      | 9.6 (4.5 to 9.9; 56 %) †          | 7.2 (7.1 to 7.6; 6 %)                   | 2.1 (2.1 to 3.1; 47 %) †          |
| veh.cluster.WheelHistory.wheelTicks     | 8.3 (8.0 to 8.7; 9 %)             | 7.2 (7.2 to 7.8; 9 %)                   | 2.1 (2.1 to 2.2; 2 %)             |
| veh.cluster.WheelHistory.wheelTemp      | 13.4 (13.1 to 17.6; 34 %) †       | 15.1 (14.9 to 21.8; 45 %) †             | 12.1 (12.0 to 16.1; 34 %) †       |
| veh.cluster.WheelHistory.wheelSlipRate  | 13.5 (13.3 to 16.8; 26 %) †       | 15.2 (15.0 to 17.6; 17 %) †             | 11.9 (11.8 to 24.4; 106 %) †      |
| veh.cluster.WheelSummary.wheelHealth    | 8.3 (8.0 to 9.1; 13 %) †          | 7.3 (7.2 to 8.5; 18 %) †                | 2.1 (2.1 to 2.2; 6 %)             |
| veh.cluster.wheels.ticks                | 8.1 (7.3 to 8.6; 16 %) †          | 7.2 (7.2 to 8.2; 14 %) †                | 2.1 (2.1 to 2.6; 21 %) †          |
| veh.cluster.WheelDiagnostics.wheelTicks | not built                         | not built                               | not built                         |
| veh.hvac.cabin.cabinTemp                | 13.2 (13.1 to 14.3; 9 %)          | 15.0 (14.9 to 15.6; 5 %)                | 12.0 (12.0 to 12.2; 2 %)          |
| veh.hvac.cabin.targetTemp               | 13.2 (13.1 to 14.2; 8 %)          | 15.1 (15.0 to 17.4; 16 %) †             | 12.1 (12.0 to 12.5; 4 %)          |
| veh.hvac.cabin.blowerDuty               | 13.4 (13.3 to 14.2; 7 %)          | 15.1 (15.0 to 15.7; 5 %)                | 11.9 (11.8 to 12.6; 7 %)          |
| veh.hvac.cabin.fanLevel                 | 8.1 (4.8 to 8.4; 44 %) †          | 7.2 (7.2 to 8.4; 17 %) †                | 2.1 (2.1 to 2.2; 3 %)             |
| veh.hvac.cabin.compressorLoad           | 13.4 (13.3 to 15.4; 16 %) †       | 15.1 (15.0 to 15.6; 4 %)                | 11.9 (11.8 to 12.8; 9 %)          |
| veh.cabin.Cabin.temperature             | 8.2 (5.2 to 8.4; 39 %) †          | 7.6 (7.5 to 9.3; 24 %) †                | 2.2 (2.1 to 2.5; 15 %) †          |
| veh.cabin.Horn.active                   | 9.7 (9.4 to 10.3; 9 %)            | 7.3 (7.2 to 7.7; 7 %)                   | 2.1 (2.1 to 2.7; 29 %) †          |
| bench.stress.Stress.longString          | 60.2 (58.5 to 66.1; 13 %) †       | 64.6 (62.6 to 81.3; 29 %) †             | 72.9 (72.3 to 79.3; 10 %)         |
| bench.stress.Stress.itemList            | 309.2 (299.1 to 333.4; 11 %) †    | 483.6 (473.0 to 604.1; 27 %) †          | 311.7 (305.1 to 331.0; 8 %)       |
| bench.stress.Stress.nestedOptional      | 38.2 (36.7 to 40.7; 10 %) †       | 51.8 (49.1 to 54.8; 11 %) †             | 33.1 (32.1 to 34.8; 8 %)          |
| bench.stress.Stress.map                 | 1715.2 (1681.2 to 2041.9; 21 %) † | 3087.7 (3009.3 to 3289.3; 9 %)          | 384.7 (372.2 to 405.0; 9 %)       |
| bench.stress.Stress.bigString           | 281.0 (278.8 to 291.3; 4 %)       | 284.3 (281.3 to 297.9; 6 %)             | 374.4 (369.9 to 387.4; 5 %)       |
| bench.stress.Stress.bigItemList         | 1676.9 (1606.5 to 2056.2; 27 %) † | 2774.3 (2716.6 to 3036.1; 12 %) †       | 1840.5 (1737.5 to 1987.2; 14 %) † |
| bench.stress.Stress.bigSamples          | 1618.6 (1601.1 to 1791.0; 12 %) † | 1523.3 (1511.6 to 1980.1; 31 %) †       | 6.1 (6.0 to 8.0; 33 %) †          |
| bench.stress.Stress.bigMap              | 78570.5 (77325.0 to 84073.3; 9 %) | 144733.3 (139689.1 to 156187.3; 11 %) † | 2940.9 (2862.9 to 3181.2; 11 %) † |
| bench.stress.Stress.deepNested          | 982.7 (960.6 to 1023.7; 6 %)      | 1563.1 (1521.3 to 1599.1; 5 %)          | 975.9 (943.9 to 998.2; 6 %)       |
| bench.stress.Stress.variant             | 28.0 (27.1 to 33.8; 24 %) †       | 35.7 (35.0 to 39.1; 11 %) †             | 22.7 (22.3 to 24.4; 9 %)          |
| bench.stress.Stress.deepSparse          | 74.0 (71.2 to 89.8; 25 %) †       | 38.2 (36.7 to 44.3; 20 %) †             | 91.7 (89.9 to 109.2; 21 %) †      |
| bench.stress.Stress.deepFull            | 261.5 (247.8 to 281.3; 13 %) †    | 382.9 (369.8 to 435.5; 17 %) †          | 285.8 (276.8 to 295.3; 6 %)       |

#### Time per operation, decode

CPU time of the decode operation of one update, in ns: the median of 30 samples
of a batch of this operation alone, with the fastest and the slowest sample and
the range as a share of the median. † spread above 10 %: the figure is not
stable.

| Payload                                 | FlatBuffers                       | proto3                             | podbuf                            |
| --------------------------------------- | --------------------------------- | ---------------------------------- | --------------------------------- |
| veh.cluster.VehicleStatus.currentSpeed  | 2.7 (2.6 to 2.8; 6 %)             | 5.6 (5.5 to 5.9; 5 %)              | 1.1 (1.1 to 1.1; 5 %)             |
| veh.cluster.VehicleStatus.engineTemp    | 2.7 (2.6 to 2.7; 3 %)             | 5.6 (5.6 to 5.7; 3 %)              | 1.1 (1.1 to 1.1; 4 %)             |
| veh.cluster.VehicleStatus.warnings      | 3.2 (3.2 to 3.3; 4 %)             | 6.1 (6.1 to 6.4; 4 %)              | 0.6 (0.6 to 0.6; 4 %)             |
| veh.cluster.WheelHistory.wheelTicks     | 3.2 (3.2 to 3.3; 3 %)             | 5.9 (5.8 to 6.0; 3 %)              | 0.6 (0.6 to 0.7; 29 %) †          |
| veh.cluster.WheelHistory.wheelTemp      | 2.7 (2.7 to 3.7; 41 %) †          | 5.6 (5.6 to 8.7; 56 %) †           | 1.1 (1.1 to 1.6; 38 %) †          |
| veh.cluster.WheelHistory.wheelSlipRate  | 2.7 (2.7 to 3.6; 35 %) †          | 5.6 (5.6 to 6.6; 18 %) †           | 1.1 (1.1 to 1.4; 27 %) †          |
| veh.cluster.WheelSummary.wheelHealth    | 3.2 (3.2 to 3.3; 3 %)             | 5.9 (5.8 to 6.0; 3 %)              | 0.6 (0.6 to 0.6; 4 %)             |
| veh.cluster.wheels.ticks                | 3.2 (3.2 to 4.0; 27 %) †          | 5.9 (5.8 to 6.6; 13 %) †           | 0.6 (0.6 to 0.7; 17 %) †          |
| veh.cluster.WheelDiagnostics.wheelTicks | not built                         | not built                          | not built                         |
| veh.hvac.cabin.cabinTemp                | 2.7 (2.7 to 2.8; 7 %)             | 5.6 (5.6 to 5.8; 4 %)              | 1.1 (1.1 to 1.1; 3 %)             |
| veh.hvac.cabin.targetTemp               | 2.7 (2.7 to 4.4; 65 %) †          | 5.6 (5.6 to 5.9; 5 %)              | 1.1 (1.1 to 1.4; 28 %) †          |
| veh.hvac.cabin.blowerDuty               | 2.7 (2.7 to 3.2; 21 %) †          | 5.6 (5.6 to 5.9; 5 %)              | 1.1 (1.1 to 1.1; 4 %)             |
| veh.hvac.cabin.fanLevel                 | 3.2 (3.2 to 3.3; 4 %)             | 5.9 (5.8 to 6.1; 5 %)              | 0.5 (0.5 to 0.5; 2 %)             |
| veh.hvac.cabin.compressorLoad           | 2.7 (2.7 to 3.1; 16 %) †          | 5.6 (5.6 to 6.0; 8 %)              | 1.1 (1.1 to 1.2; 4 %)             |
| veh.cabin.Cabin.temperature             | 3.2 (3.2 to 3.5; 9 %)             | 6.2 (6.1 to 6.8; 10 %) †           | 0.5 (0.5 to 0.7; 27 %) †          |
| veh.cabin.Horn.active                   | 3.0 (2.9 to 3.5; 20 %) †          | 7.0 (6.9 to 7.1; 3 %)              | 1.2 (1.2 to 1.3; 10 %) †          |
| bench.stress.Stress.longString          | 60.4 (57.3 to 76.9; 32 %) †       | 66.9 (61.6 to 91.1; 44 %) †        | 55.9 (51.8 to 61.9; 18 %) †       |
| bench.stress.Stress.itemList            | 894.0 (861.7 to 958.4; 11 %) †    | 1212.5 (1166.5 to 1279.1; 9 %)     | 826.3 (786.3 to 929.0; 17 %) †    |
| bench.stress.Stress.nestedOptional      | 85.6 (81.2 to 109.3; 33 %) †      | 102.4 (94.2 to 157.4; 62 %) †      | 70.0 (66.9 to 79.1; 17 %) †       |
| bench.stress.Stress.map                 | 878.8 (846.6 to 908.7; 7 %)       | 1190.0 (1136.0 to 1235.1; 8 %)     | 809.5 (770.9 to 877.9; 13 %) †    |
| bench.stress.Stress.bigString           | 113.5 (112.4 to 140.7; 25 %) †    | 120.2 (117.9 to 145.9; 23 %) †     | 114.5 (112.5 to 171.8; 52 %) †    |
| bench.stress.Stress.bigItemList         | 5142.7 (5025.1 to 5460.1; 8 %)    | 6941.7 (6735.7 to 8069.5; 19 %) †  | 4703.6 (4568.0 to 5908.2; 28 %) † |
| bench.stress.Stress.bigSamples          | 1300.7 (1229.5 to 1738.0; 39 %) † | 2163.6 (2121.7 to 2263.8; 7 %)     | 969.7 (927.4 to 1218.1; 30 %) †   |
| bench.stress.Stress.bigMap              | 6110.9 (5908.0 to 7332.4; 23 %) † | 8525.4 (8049.3 to 10747.6; 32 %) † | 5598.9 (5340.5 to 6438.1; 20 %) † |
| bench.stress.Stress.deepNested          | 3208.5 (3142.8 to 3660.9; 16 %) † | 4223.5 (4075.6 to 4295.1; 5 %)     | 2903.4 (2837.5 to 2968.8; 5 %)    |
| bench.stress.Stress.variant             | 42.6 (41.8 to 45.6; 9 %)          | 56.7 (54.8 to 59.2; 8 %)           | 29.6 (29.1 to 32.3; 11 %) †       |
| bench.stress.Stress.deepSparse          | 120.7 (116.8 to 128.9; 10 %) †    | 102.9 (99.2 to 129.8; 30 %) †      | 64.5 (63.2 to 91.1; 43 %) †       |
| bench.stress.Stress.deepFull            | 772.2 (745.3 to 869.5; 16 %) †    | 906.1 (886.5 to 979.7; 10 %) †     | 713.7 (694.8 to 742.3; 7 %)       |

#### Time per operation, access

CPU time of reading one number, deep in the payload, through the view of
verified bytes, in ns: a count, or for the sparse payloads the length of a
string field. The median of 30 samples, with the fastest and the slowest sample
and the range as a share of the median. It stands for the decode when a consumer
needs one field, and it is not part of the time per update. The FlatBuffers
accessor of a list returns the decoded list, so there the access of a list
element costs the decode of the list. proto3 generates no accessors, so its
cells are n/a. † spread above 10 %: the figure is not stable.

| Payload                         | FlatBuffers                    | proto3            | podbuf                      |
| ------------------------------- | ------------------------------ | ----------------- | --------------------------- |
| bench.stress.Stress.bigItemList | 5113.2 (4949.3 to 5297.2; 7 %) | n/a: no accessors | 11.3 (11.1 to 12.6; 13 %) † |
| bench.stress.Stress.deepNested  | 3217.4 (3129.7 to 3353.1; 7 %) | n/a: no accessors | 15.5 (13.9 to 21.3; 47 %) † |
| bench.stress.Stress.deepSparse  | 17.4 (17.0 to 26.2; 52 %) †    | n/a: no accessors | 6.7 (6.6 to 8.1; 22 %) †    |
| bench.stress.Stress.deepFull    | 18.3 (17.7 to 24.7; 38 %) †    | n/a: no accessors | 7.2 (7.2 to 7.5; 5 %)       |

#### CPU load per operation

CPU load in percent of one core at 4000 updates per second of each operation
alone, derived from the median time of that operation. The three operations of
an encoding add up to its derived load of one update.

| Payload                                 | Encode: FlatBuffers | Encode: proto3 | Encode: podbuf | Verify: FlatBuffers | Verify: proto3 | Verify: podbuf | Decode: FlatBuffers | Decode: proto3 | Decode: podbuf |
| --------------------------------------- | ------------------- | -------------- | -------------- | ------------------- | -------------- | -------------- | ------------------- | -------------- | -------------- |
| veh.cluster.VehicleStatus.currentSpeed  | 0.0076              | 0.0037         | 0.0025         | 0.0052              | 0.0058         | 0.0046         | 0.0011              | 0.0022         | 0.0004         |
| veh.cluster.VehicleStatus.engineTemp    | 0.0074              | 0.0036         | 0.0025         | 0.0053              | 0.0060         | 0.0048         | 0.0011              | 0.0022         | 0.0004         |
| veh.cluster.VehicleStatus.warnings      | 0.0074              | 0.0026         | 0.0025         | 0.0038              | 0.0029         | 0.0009         | 0.0013              | 0.0025         | 0.0002         |
| veh.cluster.WheelHistory.wheelTicks     | 0.0074              | 0.0026         | 0.0025         | 0.0033              | 0.0029         | 0.0009         | 0.0013              | 0.0024         | 0.0002         |
| veh.cluster.WheelHistory.wheelTemp      | 0.0078              | 0.0036         | 0.0025         | 0.0053              | 0.0060         | 0.0049         | 0.0011              | 0.0022         | 0.0005         |
| veh.cluster.WheelHistory.wheelSlipRate  | 0.0075              | 0.0037         | 0.0026         | 0.0054              | 0.0061         | 0.0048         | 0.0011              | 0.0023         | 0.0004         |
| veh.cluster.WheelSummary.wheelHealth    | 0.0074              | 0.0026         | 0.0025         | 0.0033              | 0.0029         | 0.0009         | 0.0013              | 0.0023         | 0.0002         |
| veh.cluster.wheels.ticks                | 0.0074              | 0.0026         | 0.0025         | 0.0033              | 0.0029         | 0.0009         | 0.0013              | 0.0024         | 0.0002         |
| veh.cluster.WheelDiagnostics.wheelTicks | not built           | not built      | not built      | not built           | not built      | not built      | not built           | not built      | not built      |
| veh.hvac.cabin.cabinTemp                | 0.0076              | 0.0036         | 0.0025         | 0.0053              | 0.0060         | 0.0048         | 0.0011              | 0.0022         | 0.0004         |
| veh.hvac.cabin.targetTemp               | 0.0075              | 0.0036         | 0.0025         | 0.0053              | 0.0060         | 0.0048         | 0.0011              | 0.0023         | 0.0004         |
| veh.hvac.cabin.blowerDuty               | 0.0075              | 0.0036         | 0.0026         | 0.0054              | 0.0060         | 0.0048         | 0.0011              | 0.0022         | 0.0004         |
| veh.hvac.cabin.fanLevel                 | 0.0074              | 0.0026         | 0.0025         | 0.0032              | 0.0029         | 0.0009         | 0.0013              | 0.0023         | 0.0002         |
| veh.hvac.cabin.compressorLoad           | 0.0075              | 0.0036         | 0.0026         | 0.0054              | 0.0060         | 0.0048         | 0.0011              | 0.0022         | 0.0004         |
| veh.cabin.Cabin.temperature             | 0.0075              | 0.0027         | 0.0025         | 0.0033              | 0.0030         | 0.0009         | 0.0013              | 0.0025         | 0.0002         |
| veh.cabin.Horn.active                   | 0.0075              | 0.0028         | 0.0025         | 0.0039              | 0.0029         | 0.0009         | 0.0012              | 0.0028         | 0.0005         |
| bench.stress.Stress.longString          | 0.0148              | 0.0072         | 0.0134         | 0.0241              | 0.0259         | 0.0292         | 0.0242              | 0.0267         | 0.0223         |
| bench.stress.Stress.itemList            | 0.1722              | 0.1639         | 0.0800         | 0.1237              | 0.1934         | 0.1247         | 0.3576              | 0.4850         | 0.3305         |
| bench.stress.Stress.nestedOptional      | 0.0258              | 0.0155         | 0.0117         | 0.0153              | 0.0207         | 0.0132         | 0.0343              | 0.0409         | 0.0280         |
| bench.stress.Stress.map                 | 0.1696              | 0.3099         | 0.1344         | 0.6861              | 1.2351         | 0.1539         | 0.3515              | 0.4760         | 0.3238         |
| bench.stress.Stress.bigString           | 0.0328              | 0.0168         | 0.0629         | 0.1124              | 0.1137         | 0.1498         | 0.0454              | 0.0481         | 0.0458         |
| bench.stress.Stress.bigItemList         | 0.8938              | 1.0203         | 0.3949         | 0.6708              | 1.1097         | 0.7362         | 2.0571              | 2.7767         | 1.8815         |
| bench.stress.Stress.bigSamples          | 0.2200              | 4.0930         | 0.3376         | 0.6475              | 0.6093         | 0.0024         | 0.5203              | 0.8654         | 0.3879         |
| bench.stress.Stress.bigMap              | 1.1506              | 10.3604        | 0.8981         | 31.4282             | 57.8933        | 1.1764         | 2.4444              | 3.4102         | 2.2396         |
| bench.stress.Stress.deepNested          | 0.6782              | 0.5871         | 0.2415         | 0.3931              | 0.6252         | 0.3903         | 1.2834              | 1.6894         | 1.1614         |
| bench.stress.Stress.variant             | 0.0252              | 0.0109         | 0.0095         | 0.0112              | 0.0143         | 0.0091         | 0.0170              | 0.0227         | 0.0118         |
| bench.stress.Stress.deepSparse          | 0.0295              | 0.0185         | 0.0124         | 0.0296              | 0.0153         | 0.0367         | 0.0483              | 0.0412         | 0.0258         |
| bench.stress.Stress.deepFull            | 0.1141              | 0.1096         | 0.0688         | 0.1046              | 0.1532         | 0.1143         | 0.3089              | 0.3624         | 0.2855         |

#### Ratios per operation

Time of each operation of each encoding divided by the time of the same
operation in podbuf: above 1.00 the encoding is slower than podbuf in that
operation, below 1.00 faster. A refused or not-built cell has no ratio and is
marked. A time under about 10 ns includes harness cost and is an upper bound, so
a ratio whose podbuf time is under 10 ns is not comparable.

| Payload                                 | Encode: FlatBuffers / podbuf | Encode: proto3 / podbuf | Verify: FlatBuffers / podbuf | Verify: proto3 / podbuf | Decode: FlatBuffers / podbuf | Decode: proto3 / podbuf |
| --------------------------------------- | ---------------------------- | ----------------------- | ---------------------------- | ----------------------- | ---------------------------- | ----------------------- |
| veh.cluster.VehicleStatus.currentSpeed  | 3.11                         | 1.50                    | 1.13                         | 1.26                    | 2.45                         | 5.14                    |
| veh.cluster.VehicleStatus.engineTemp    | 3.00                         | 1.45                    | 1.09                         | 1.24                    | 2.45                         | 5.15                    |
| veh.cluster.VehicleStatus.warnings      | 3.02                         | 1.05                    | 4.49                         | 3.37                    | 5.44                         | 10.46                   |
| veh.cluster.WheelHistory.wheelTicks     | 3.01                         | 1.04                    | 3.90                         | 3.38                    | 5.51                         | 10.13                   |
| veh.cluster.WheelHistory.wheelTemp      | 3.14                         | 1.47                    | 1.10                         | 1.24                    | 2.34                         | 4.89                    |
| veh.cluster.WheelHistory.wheelSlipRate  | 2.89                         | 1.41                    | 1.13                         | 1.27                    | 2.45                         | 5.16                    |
| veh.cluster.WheelSummary.wheelHealth    | 3.02                         | 1.05                    | 3.90                         | 3.39                    | 5.51                         | 10.09                   |
| veh.cluster.wheels.ticks                | 3.00                         | 1.04                    | 3.81                         | 3.38                    | 5.52                         | 10.11                   |
| veh.cluster.WheelDiagnostics.wheelTicks | not built                    | not built               | not built                    | not built               | not built                    | not built               |
| veh.hvac.cabin.cabinTemp                | 3.08                         | 1.46                    | 1.10                         | 1.24                    | 2.46                         | 5.16                    |
| veh.hvac.cabin.targetTemp               | 3.05                         | 1.46                    | 1.10                         | 1.25                    | 2.47                         | 5.17                    |
| veh.hvac.cabin.blowerDuty               | 2.91                         | 1.40                    | 1.13                         | 1.27                    | 2.46                         | 5.17                    |
| veh.hvac.cabin.fanLevel                 | 3.01                         | 1.05                    | 3.81                         | 3.38                    | 6.23                         | 11.42                   |
| veh.hvac.cabin.compressorLoad           | 2.89                         | 1.40                    | 1.13                         | 1.27                    | 2.41                         | 5.05                    |
| veh.cabin.Cabin.temperature             | 3.03                         | 1.09                    | 3.80                         | 3.51                    | 5.89                         | 11.31                   |
| veh.cabin.Horn.active                   | 3.05                         | 1.13                    | 4.52                         | 3.40                    | 2.49                         | 5.89                    |
| bench.stress.Stress.longString          | 1.11                         | 0.54                    | 0.83                         | 0.89                    | 1.08                         | 1.20                    |
| bench.stress.Stress.itemList            | 2.15                         | 2.05                    | 0.99                         | 1.55                    | 1.08                         | 1.47                    |
| bench.stress.Stress.nestedOptional      | 2.21                         | 1.33                    | 1.15                         | 1.56                    | 1.22                         | 1.46                    |
| bench.stress.Stress.map                 | 1.26                         | 2.31                    | 4.46                         | 8.03                    | 1.09                         | 1.47                    |
| bench.stress.Stress.bigString           | 0.52                         | 0.27                    | 0.75                         | 0.76                    | 0.99                         | 1.05                    |
| bench.stress.Stress.bigItemList         | 2.26                         | 2.58                    | 0.91                         | 1.51                    | 1.09                         | 1.48                    |
| bench.stress.Stress.bigSamples          | 0.65                         | 12.13                   | 266.48                       | 250.78                  | 1.34                         | 2.23                    |
| bench.stress.Stress.bigMap              | 1.28                         | 11.54                   | 26.72                        | 49.21                   | 1.09                         | 1.52                    |
| bench.stress.Stress.deepNested          | 2.81                         | 2.43                    | 1.01                         | 1.60                    | 1.11                         | 1.45                    |
| bench.stress.Stress.variant             | 2.66                         | 1.15                    | 1.23                         | 1.58                    | 1.44                         | 1.92                    |
| bench.stress.Stress.deepSparse          | 2.37                         | 1.49                    | 0.81                         | 0.42                    | 1.87                         | 1.60                    |
| bench.stress.Stress.deepFull            | 1.66                         | 1.59                    | 0.92                         | 1.34                    | 1.08                         | 1.27                    |

#### CPU load, paced and derived

CPU load in percent of one core at 4000 updates per second: paced (process CPU
time over wall time, includes the cost of sleep and wake-up) next to the derived
figure, and their difference.

| Payload                                | Encoding    | Paced % | Derived % | Difference |
| -------------------------------------- | ----------- | ------- | --------- | ---------- |
| veh.cluster.VehicleStatus.currentSpeed | FlatBuffers | 1.1911  | 0.0139    | 1.1771     |
| veh.cluster.VehicleStatus.currentSpeed | proto3      | 1.3373  | 0.0118    | 1.3256     |
| veh.cluster.VehicleStatus.currentSpeed | podbuf      | 1.6334  | 0.0075    | 1.6259     |
| veh.cluster.VehicleStatus.engineTemp   | FlatBuffers | 1.0544  | 0.0137    | 1.0407     |
| veh.cluster.VehicleStatus.engineTemp   | proto3      | 1.2097  | 0.0118    | 1.1979     |
| veh.cluster.VehicleStatus.engineTemp   | podbuf      | 1.1677  | 0.0077    | 1.1600     |
| veh.cluster.VehicleStatus.warnings     | FlatBuffers | 1.1370  | 0.0125    | 1.1245     |
| veh.cluster.VehicleStatus.warnings     | proto3      | 1.2619  | 0.0079    | 1.2540     |
| veh.cluster.VehicleStatus.warnings     | podbuf      | 1.0852  | 0.0035    | 1.0817     |
| veh.cluster.WheelHistory.wheelTicks    | FlatBuffers | 1.0794  | 0.0120    | 1.0674     |
| veh.cluster.WheelHistory.wheelTicks    | proto3      | 1.5328  | 0.0078    | 1.5250     |
| veh.cluster.WheelHistory.wheelTicks    | podbuf      | 2.7842  | 0.0035    | 2.7807     |
| veh.cluster.WheelHistory.wheelTemp     | FlatBuffers | 1.0948  | 0.0142    | 1.0806     |
| veh.cluster.WheelHistory.wheelTemp     | proto3      | 1.3703  | 0.0119    | 1.3584     |
| veh.cluster.WheelHistory.wheelTemp     | podbuf      | 1.2049  | 0.0078    | 1.1971     |
| veh.cluster.WheelHistory.wheelSlipRate | FlatBuffers | 1.5050  | 0.0140    | 1.4910     |
| veh.cluster.WheelHistory.wheelSlipRate | proto3      | 1.5932  | 0.0120    | 1.5813     |
| veh.cluster.WheelHistory.wheelSlipRate | podbuf      | 1.3015  | 0.0078    | 1.2937     |
| veh.cluster.WheelSummary.wheelHealth   | FlatBuffers | 1.0750  | 0.0120    | 1.0630     |
| veh.cluster.WheelSummary.wheelHealth   | proto3      | 1.5880  | 0.0078    | 1.5802     |
| veh.cluster.WheelSummary.wheelHealth   | podbuf      | 1.8453  | 0.0035    | 1.8418     |
| veh.cluster.wheels.ticks               | FlatBuffers | 1.0346  | 0.0119    | 1.0227     |
| veh.cluster.wheels.ticks               | proto3      | 1.5690  | 0.0078    | 1.5612     |
| veh.cluster.wheels.ticks               | podbuf      | 1.7563  | 0.0036    | 1.7527     |
| veh.hvac.cabin.cabinTemp               | FlatBuffers | 0.9994  | 0.0139    | 0.9855     |
| veh.hvac.cabin.cabinTemp               | proto3      | 1.2443  | 0.0118    | 1.2325     |
| veh.hvac.cabin.cabinTemp               | podbuf      | 1.9576  | 0.0077    | 1.9498     |
| veh.hvac.cabin.targetTemp              | FlatBuffers | 1.0556  | 0.0139    | 1.0417     |
| veh.hvac.cabin.targetTemp              | proto3      | 1.0750  | 0.0119    | 1.0631     |
| veh.hvac.cabin.targetTemp              | podbuf      | 1.2784  | 0.0077    | 1.2707     |
| veh.hvac.cabin.blowerDuty              | FlatBuffers | 1.2605  | 0.0139    | 1.2466     |
| veh.hvac.cabin.blowerDuty              | proto3      | 2.0936  | 0.0119    | 2.0817     |
| veh.hvac.cabin.blowerDuty              | podbuf      | 1.1951  | 0.0078    | 1.1873     |
| veh.hvac.cabin.fanLevel                | FlatBuffers | 1.2031  | 0.0119    | 1.1912     |
| veh.hvac.cabin.fanLevel                | proto3      | 4.5069  | 0.0078    | 4.4991     |
| veh.hvac.cabin.fanLevel                | podbuf      | 4.3525  | 0.0035    | 4.3490     |
| veh.hvac.cabin.compressorLoad          | FlatBuffers | 1.0510  | 0.0139    | 1.0371     |
| veh.hvac.cabin.compressorLoad          | proto3      | 1.0878  | 0.0119    | 1.0759     |
| veh.hvac.cabin.compressorLoad          | podbuf      | 1.2639  | 0.0078    | 1.2561     |
| veh.cabin.Cabin.temperature            | FlatBuffers | 1.0042  | 0.0121    | 0.9921     |
| veh.cabin.Cabin.temperature            | proto3      | 1.3739  | 0.0082    | 1.3657     |
| veh.cabin.Cabin.temperature            | podbuf      | 1.1114  | 0.0036    | 1.1079     |
| veh.cabin.Horn.active                  | FlatBuffers | 1.1863  | 0.0126    | 1.1737     |
| veh.cabin.Horn.active                  | proto3      | 1.2301  | 0.0085    | 1.2216     |
| veh.cabin.Horn.active                  | podbuf      | 1.3755  | 0.0038    | 1.3717     |
| bench.stress.Stress.longString         | FlatBuffers | 1.5260  | 0.0630    | 1.4630     |
| bench.stress.Stress.longString         | proto3      | 4.1014  | 0.0598    | 4.0416     |
| bench.stress.Stress.longString         | podbuf      | 4.7246  | 0.0649    | 4.6598     |
| bench.stress.Stress.itemList           | FlatBuffers | 1.9335  | 0.6535    | 1.2800     |
| bench.stress.Stress.itemList           | proto3      | 2.5442  | 0.8423    | 1.7019     |
| bench.stress.Stress.itemList           | podbuf      | 2.4538  | 0.5352    | 1.9185     |
| bench.stress.Stress.nestedOptional     | FlatBuffers | 1.2505  | 0.0753    | 1.1752     |
| bench.stress.Stress.nestedOptional     | proto3      | 1.4876  | 0.0772    | 1.4104     |
| bench.stress.Stress.nestedOptional     | podbuf      | 1.3243  | 0.0529    | 1.2714     |
| bench.stress.Stress.map                | FlatBuffers | 2.8721  | 1.2072    | 1.6649     |
| bench.stress.Stress.map                | proto3      | 3.6540  | 2.0209    | 1.6331     |
| bench.stress.Stress.map                | podbuf      | 2.1504  | 0.6121    | 1.5383     |
| bench.stress.Stress.bigString          | FlatBuffers | 1.6186  | 0.1906    | 1.4280     |
| bench.stress.Stress.bigString          | proto3      | 2.6826  | 0.1786    | 2.5040     |
| bench.stress.Stress.bigString          | podbuf      | 3.5272  | 0.2585    | 3.2686     |
| bench.stress.Stress.bigItemList        | FlatBuffers | 5.4982  | 3.6216    | 1.8766     |
| bench.stress.Stress.bigItemList        | proto3      | 8.6233  | 4.9067    | 3.7166     |
| bench.stress.Stress.bigItemList        | podbuf      | 6.7231  | 3.0125    | 3.7106     |
| bench.stress.Stress.bigSamples         | FlatBuffers | 3.5218  | 1.3878    | 2.1340     |
| bench.stress.Stress.bigSamples         | proto3      | 9.2164  | 5.5678    | 3.6486     |
| bench.stress.Stress.bigSamples         | podbuf      | 2.5406  | 0.7278    | 1.8127     |
| bench.stress.Stress.bigMap             | FlatBuffers | 35.2927 | 35.0231   | 0.2695     |
| bench.stress.Stress.bigMap             | proto3      | 72.8873 | 71.6639   | 1.2234     |
| bench.stress.Stress.bigMap             | podbuf      | 6.3502  | 4.3140    | 2.0361     |
| bench.stress.Stress.deepNested         | FlatBuffers | 4.4233  | 2.3547    | 2.0686     |
| bench.stress.Stress.deepNested         | proto3      | 10.8541 | 2.9018    | 7.9523     |
| bench.stress.Stress.deepNested         | podbuf      | 12.7068 | 1.7933    | 10.9135    |
| bench.stress.Stress.variant            | FlatBuffers | 1.3511  | 0.0534    | 1.2977     |
| bench.stress.Stress.variant            | proto3      | 5.4465  | 0.0479    | 5.3986     |
| bench.stress.Stress.variant            | podbuf      | 2.7759  | 0.0304    | 2.7455     |
| bench.stress.Stress.deepSparse         | FlatBuffers | 1.2101  | 0.1074    | 1.1027     |
| bench.stress.Stress.deepSparse         | proto3      | 1.2351  | 0.0749    | 1.1601     |
| bench.stress.Stress.deepSparse         | podbuf      | 1.5840  | 0.0749    | 1.5091     |
| bench.stress.Stress.deepFull           | FlatBuffers | 2.3015  | 0.5276    | 1.7739     |
| bench.stress.Stress.deepFull           | proto3      | 4.6319  | 0.6252    | 4.0067     |
| bench.stress.Stress.deepFull           | podbuf      | 2.8442  | 0.4686    | 2.3757     |

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
core.** Date 2026-11-08, commit ec63ebf8, rustc 1.98.1 (48a229cea 2026-09-01), 4
000 updates per second, 30 samples per cell, 2 s of CPU time per operation, cell
and encoding, paced run included. 81 cells were measured and 3 were not built
(the one payload `RawTickCount`, for each encoding), in each run. No cell was
refused. The commands were `just bench-pixel`, then `cargo xtask bench report`.
Each core was measured once, so each table is a snapshot of one run.

### What the device data show, and what they do not

- **Frequency.** The phone is not rooted, the governor is `schedutil` and the
  frequency is not fixed. The sidecar files read the frequency of each core
  before and after a run; the two reads do not show the frequency during the
  run. The A55 run read cpu2 at 1 804 800 kHz before and 1 363 200 kHz after.
  The A76 run read cpu6 at 1 728 000 kHz before and 2 208 000 kHz after. The
  result files record the frequency of the core at the start of the program: 1
  804 800 kHz in the A55 run, the same as the sidecar read, and 1 094 400 kHz in
  the A76 run, which differs from the sidecar read of 1 728 000 kHz; this note
  asserts no cause. The frequency during the runs is therefore not known; it may
  have varied.
- **Stability.** No cell carries the spread marker, for the update, for each
  operation and for the access. The largest `(max - min) / median` of any
  operation cell is 4.2 % in the A55 run and 6.5 % in the A76 run, and every
  cell has 30 samples. The tables therefore show no instability in the samples.
  They do not show that the frequency was constant, because the samples of one
  cell are taken over a short time.
- **Load.** The codec-only load at 4 000 updates per second is a small fraction
  of one core for the signal payloads: 0.042 % to 0.181 % on the A55 core and
  0.013 % to 0.061 % on the A76 core. The largest load among the signal payloads
  is FlatBuffers on `veh.hvac.cabin.compressorLoad` on the A55 core (0.181 %,
  451.9 ns per update) and FlatBuffers on `veh.hvac.cabin.cabinTemp` on the A76
  core (0.061 %, 153.3 ns per update). The largest load of any cell is the
  proto3 encoding of `bigMap`: 628.0 % of the A55 core and 184.8 % of the A76
  core; the FlatBuffers cell of `bigMap` is 291.8 % and 106.6 %. A derived load
  above 100 % means that one update takes longer than the 250 us between two
  updates, so one core cannot run 4 000 such updates per second. Other cells
  above 10 %: on the A55 core 10 more cells (proto3 `map`; `bigItemList`,
  `deepNested` in all three encodings; `bigSamples` in FlatBuffers and proto3;
  podbuf `bigMap`), on the A76 core proto3 `bigItemList` and `bigSamples`.
- **Paced against derived.** The paced figure is above the derived figure in 79
  of the 81 cells of each run. The two exceptions are FlatBuffers and proto3 on
  `bigMap`, where the derived figure is above 100 % and the paced figure is 98.4
  % and 98.7 % (A55) and 99.8 % and 99.8 % (A76): the paced run cannot reach the
  rate. For the signal payloads the paced figure is 8.6 % to 11.6 % on the A55
  core and 2.0 % to 2.2 % on the A76 core, where the derived figure is 0.042 %
  to 0.181 % and 0.013 % to 0.061 %. The paced runs measure process CPU time
  over a run that sleeps between updates; the tables do not show what part of
  that time is the codec, and this note does not assert a cause. The derived
  figure is the one that the codec determines.
- **Comparison of the encodings, per update.** On the 15 measured signal
  payloads podbuf has the lowest median time per update on both cores. Of the 12
  stress payloads, podbuf has the lowest on 10 on the A55 core and on 9 on the
  A76 core. proto3 has the lowest on `longString` and `bigString` on both cores,
  where podbuf has the highest, and on `deepSparse` on the A76 core (592 ns
  against 662 ns for podbuf); on the A55 core podbuf is 1.5 % ahead on
  `deepSparse` (2 099 ns against 2 131 ns). proto3 is the smallest in size on
  every signal payload and on 11 of the 12 stress payloads. There is no ordering
  of the encodings that holds on every payload, so this note gives no verdict
  between them.
- **The burst is not covered.** A burst of 200 messages in 10 ms was not run.
  The time per update in the tables lets a reader compute a burst of
  back-to-back updates. Worked example, derived and not measured: the heaviest
  signal payload on the A55 core is FlatBuffers on
  `veh.hvac.cabin.compressorLoad`, 451.9 ns per update; 200 updates cost 200 x
  451.9 ns = 90.4 us of CPU time, which is 0.90 % of the 10 ms window. This
  assumes that every update costs what the warm, repeated update of the bench
  costs; it does not include the transport, queueing, cold caches or a lower
  core frequency.

### What the split into operations shows, and what it does not

The numbers are those of the per-operation tables below.
`veh.cluster.VehicleStatus.currentSpeed` is used as an example signal: a figure
given for `currentSpeed` is that payload's own value, not a statistic. A figure
given "over the signals" is the median over the 15 signals of each signal's
median.

- **Where the time goes.** On the A55 core `currentSpeed` costs 441.6 ns per
  update for FlatBuffers (encode 180.4, verify 220.2, decode 41.0 ns), 377.0 ns
  for proto3 (76.4, 236.2, 64.3) and 286.3 ns for podbuf (74.5, 196.3, 15.5).
  Verify is the largest share for proto3 (63 %) and podbuf (69 %) and 50 % for
  FlatBuffers. On the A76 core `currentSpeed` costs 150.0 ns for FlatBuffers
  (70.8, 64.8, 14.4), 127.5 ns for proto3 (34.1, 69.4, 24.0) and 83.4 ns for
  podbuf (23.5, 55.1, 4.8); there verify is the largest share for proto3 (54 %)
  and podbuf (66 %), and encode is the largest operation for FlatBuffers (47 %).
- **Verify.** Over the signals, verify takes 220.2 ns (FlatBuffers), 236.2 ns
  (proto3) and 196.3 ns (podbuf) on the A55 core, and 64.8, 69.4 and 55.1 ns on
  the A76 core. The verify time of one encoding varies a lot between payloads:
  podbuf verify is 25.5 ns on the cheapest signal and 205.2 ns on the dearest on
  the A55 core. The data do not show what makes a payload cheap or dear to
  verify.
- **Encode.** FlatBuffers encode is 2.41 to 2.68 times podbuf encode on the 15
  signals on the A55 core and 2.91 to 3.29 times on the A76 core. proto3 encode
  is 1.01 to 1.20 times podbuf encode on the A55 core and 1.07 to 2.16 times on
  the A76 core. On the long string the order differs: proto3 encode is 232 ns
  (A55), against 924 ns for FlatBuffers and 997 ns for podbuf. On the big string
  (A55) it is 751 ns for proto3, 4 212 ns for FlatBuffers and 5 164 ns for
  podbuf; podbuf verify is also the largest there (3 770 ns against 2 707 and 2
  747 ns), and podbuf decode is the smallest (1 360 ns against 1 578 and 1 628
  ns). On the A76 core the order on both strings is the same.
- **Decode.** Over the signals, decode takes 41.0 ns (FlatBuffers), 64.3 ns
  (proto3) and 15.5 ns (podbuf) on the A55 core, and 14.4, 24.0 and 4.7 ns on
  the A76 core. proto3 decode is 4.14 to 10.08 times podbuf decode on the A55
  core and 4.50 to 16.28 times on the A76 core. The largest ratios come from the
  signals where the podbuf decode is 7.2 ns (A55) or 2.0 ns (A76), below the 10
  ns under which a decode time is an upper bound and not comparable. The codec
  record says that proto3 decode reads the message again from the bytes; these
  tables do not show whether that is the reason for the ratio.
- **Stress payloads.** On the map and on `bigMap`, verify is the largest
  operation for FlatBuffers and proto3 on both cores: on the A55 core 14.0 us
  and 27.1 us for `map`, 676.8 us and 1 334.6 us for `bigMap`, against 24.9 us
  for podbuf on `bigMap`. From `map` to `bigMap` the verify time grows 48.5 and
  49.2 times for FlatBuffers and proto3 and 7.4 times for podbuf on the A55 core
  (46.4, 46.3 and 7.5 times on the A76 core), for 7.5 times the maximum number
  of entries; this is consistent with the quadratic comparison of keys that the
  codec records state. On the list of structs podbuf has the lowest verify time
  on both cores (2.4 us on the A55 core, against 2.5 us for FlatBuffers and 4.3
  us for proto3).
- **Access.** One number read through the view of verified bytes takes, on the
  A55 core, 134.0 ns with podbuf and 25 148 ns with FlatBuffers on `bigItemList`
  (188 times), 130.3 ns and 15 383 ns on `deepNested` (118 times), 71.6 ns and
  139.2 ns on `deepSparse`, and 76.0 ns and 143.8 ns on `deepFull`. On the A76
  core the podbuf figures are 43.4, 46.8, 24.5 and 25.5 ns, and the FlatBuffers
  figures 8 238, 5 031, 61.6 and 64.4 ns. The podbuf access time does not follow
  the size or the element count of the payload in these four rows: `deepNested`
  (1 372 bytes) takes about as long as `bigItemList` (2 516 bytes), and
  `deepSparse` (182 bytes) about as long as `deepFull` (296 bytes). Four rows do
  not establish the access time in general. The podbuf decode of `bigItemList`
  is 167 times its access on the A55 core and that of `deepFull` is 43 times.
  The FlatBuffers accessor of a list returns the decoded list, so its access of
  a list element costs about the decode of the list.
- **Large integer list.** On `bigSamples` the verify takes 70 ns with podbuf on
  the A55 core and 22.5 ns on the A76 core, against 9 222 ns and 4 113 ns for
  FlatBuffers and 14 076 ns and 4 441 ns for proto3. The proto3 encode of this
  payload takes 102 448 ns on the A55 core and 29 543 ns on the A76 core,
  against 9 287 ns and 4 149 ns for podbuf. The next section gives the podbuf
  figures before the load change.
- **Allocations per operation.** Verify made no allocation for any payload. The
  allocations of the stress payloads are in encode and decode as the host
  observations state; the counts are the same in every run.
- **No verdict.** No ordering of the encodings holds for every operation on
  every payload: for example podbuf encode is the slowest of the three on the
  long string, while it is faster than FlatBuffers encode on every signal. The
  tables give the data per operation, and this note draws no conclusion beyond
  the lines above.

### Effect of the podbuf load change

Pull request #1259 (driftsys/ridl) changed how `ridl-rt` reads and writes a
podbuf scalar. The tables below compare the podbuf medians of two Pixel 5 runs:
the run of 2026-11-06 at commit 4843b087, before that change, and the run of
2026-11-08 at commit ec63ebf8, after it. Between the two commits the podbuf code
changed in that pull request, and in a field name of `PodbufForm`
(driftsys/ridl#1256). The medians are `per_op_ns[op].median` of each file; the
sum is encode plus verify plus decode. A value marked `*` is below 10 ns, so it
is an upper bound that is not comparable. No cell of these rows is flagged in
either run.

Cortex-A55 (cpu2), podbuf, median time in ns (before, after):

| Payload                         | Encode before | Encode after | Verify before | Verify after | Decode before | Decode after | Sum before | Sum after | Sum change |
| ------------------------------- | ------------- | ------------ | ------------- | ------------ | ------------- | ------------ | ---------- | --------- | ---------- |
| veh.hvac.cabin.cabinTemp        | 75.7          | 75.1         | 228.5         | 203.5        | 37.2          | 15.5         | 341.3      | 294.2     | -14 %      |
| veh.cabin.Horn.active           | 72.2          | 71.2         | 29.9          | 25.5         | 16.1          | 16.1         | 118.3      | 112.7     | -5 %       |
| bench.stress.Stress.longString  | 998.8         | 997.2        | 785.2         | 758.0        | 407.0         | 387.6        | 2191.0     | 2142.8    | -2 %       |
| bench.stress.Stress.itemList    | 1984.5        | 2021.8       | 3010.9        | 2422.8       | 3928.6        | 3451.2       | 8924.0     | 7895.8    | -12 %      |
| bench.stress.Stress.map         | 2841.9        | 2775.8       | 4581.9        | 3383.0       | 3839.7        | 3400.8       | 11263.5    | 9559.7    | -15 %      |
| bench.stress.Stress.bigString   | 5165.7        | 5164.2       | 3800.0        | 3770.1       | 1380.3        | 1359.6       | 10346.0    | 10293.9   | -1 %       |
| bench.stress.Stress.bigItemList | 12773.9       | 12997.0      | 22049.9       | 18561.8      | 25025.7       | 22404.5      | 59849.4    | 53963.3   | -10 %      |
| bench.stress.Stress.bigSamples  | 14277.3       | 9287.2       | 20059.5       | 69.9         | 27672.0       | 14346.8      | 62008.8    | 23703.9   | -62 %      |
| bench.stress.Stress.bigMap      | 20422.5       | 19955.9      | 34137.5       | 24941.3      | 25869.6       | 22666.1      | 80429.6    | 67563.3   | -16 %      |

Cortex-A76 (cpu6), podbuf, median time in ns (before, after):

| Payload                         | Encode before | Encode after | Verify before | Verify after | Decode before | Decode after | Sum before | Sum after | Sum change |
| ------------------------------- | ------------- | ------------ | ------------- | ------------ | ------------- | ------------ | ---------- | --------- | ---------- |
| veh.hvac.cabin.cabinTemp        | 23.4          | 23.6         | 68.8          | 58.3         | 13.2          | 4.7*         | 105.4      | 86.6      | -18 %      |
| veh.cabin.Horn.active           | 23.7          | 23.5         | 8.7*          | 8.2*         | 6.1*          | 6.2*         | 38.4       | 37.9      | -1 %       |
| bench.stress.Stress.longString  | 386.2         | 385.2        | 281.7         | 273.6        | 120.9         | 115.7        | 788.8      | 774.5     | -2 %       |
| bench.stress.Stress.itemList    | 671.9         | 669.0        | 1031.0        | 843.1        | 1260.3        | 1114.9       | 2963.3     | 2627.0    | -11 %      |
| bench.stress.Stress.map         | 953.9         | 936.5        | 1621.9        | 1140.4       | 1257.7        | 1077.6       | 3833.5     | 3154.5    | -18 %      |
| bench.stress.Stress.bigString   | 1973.5        | 1981.7       | 1409.4        | 1423.3       | 425.6         | 430.1        | 3808.5     | 3835.1    | +1 %       |
| bench.stress.Stress.bigItemList | 4147.3        | 4101.2       | 7017.2        | 5518.6       | 7894.9        | 7027.2       | 19059.3    | 16647.0   | -13 %      |
| bench.stress.Stress.bigSamples  | 5508.0        | 4149.1       | 6161.8        | 22.5         | 9036.9        | 5044.8       | 20706.7    | 9216.4    | -55 %      |
| bench.stress.Stress.bigMap      | 7169.8        | 7006.8       | 11551.2       | 8588.6       | 8655.2        | 7433.4       | 27376.2    | 23028.8   | -16 %      |

What the tables show:

- On `bigSamples` the podbuf sum falls by 62 % on the A55 core and by 55 % on
  the A76 core. The verify falls from 20 059 ns to 70 ns (A55) and from 6 162 ns
  to 22.5 ns (A76). The element type has the full range of a 16-bit unsigned
  integer, so verify has no per-element range to check, and its time no longer
  follows the 1 500 elements. The encode and the decode of this payload also
  fall (A55: 14 277 to 9 287 ns and 27 672 to 14 347 ns).
- On the signal that holds a 4-byte value (`veh.hvac.cabin.cabinTemp`) the sum
  falls by 14 % on the A55 core and by 18 % on the A76 core, mostly in verify
  and decode; encode does not change. The row with a 1-byte value
  (`veh.cabin.Horn.active`) falls by 5 % and 1 %; its A76 verify and decode are
  below 10 ns.
- On the list and map payloads (`itemList`, `map`, `bigItemList`, `bigMap`) the
  sum falls by 10 % to 18 % on both cores, with verify and decode lower and
  encode close to unchanged.
- On the strings (`longString`, `bigString`) the sum changes by -2 % to +1 %.
- As a control, the update medians of FlatBuffers and proto3, whose code did not
  change, differ between the two runs by a median of 0.0 % on both cores. On the
  A55 core the range is -0.7 % to +1.2 %. On the A76 core the range is -1.7 % to
  +36.4 %: the proto3 encode of `veh.cabin.Horn.active` is 24.6 ns in the first
  run and 50.5 ns in the second. The A76 result files record a core frequency of
  1 478 400 kHz at the start of the first run and 1 094 400 kHz at the start of
  the second, and this note asserts no cause. A difference of a few percent on
  one small cell of the A76 core is therefore inside what the two runs of
  unchanged code differ by.

### Effect of the podbuf string path change

Two changes to the podbuf string path are compared here with the run of
2026-11-08 at commit ec63ebf8, the "after" columns of the previous section: the
NUL scan of a string (`podbuf::has_nul`) and the one-pass write of a string or
bytes block (`Writer::tail_data`). The new run is of 2026-11-09 at commit
2f5152d4. The medians are `per_op_ns[op].median` of each file. No cell of these
rows is flagged in the new run.

podbuf, median time in ns (before, after):

| Core | Payload                        | Encode before | Encode after | Verify before | Verify after | Decode before | Decode after |
| ---- | ------------------------------ | ------------- | ------------ | ------------- | ------------ | ------------- | ------------ |
| A55  | bench.stress.Stress.longString | 997.2         | 336.5        | 758.0         | 678.7        | 387.6         | 388.3        |
| A55  | bench.stress.Stress.bigString  | 5164.2        | 1365.2       | 3770.1        | 3335.3       | 1359.6        | 1361.3       |
| A76  | bench.stress.Stress.longString | 385.2         | 73.8         | 273.6         | 225.3        | 115.7         | 111.0        |
| A76  | bench.stress.Stress.bigString  | 1981.7        | 247.7        | 1423.3        | 1143.7       | 430.1         | 378.7        |

Encode is 3 to 8 times faster and verify 10 to 20 % faster. Decode does not
change.

Most of the encode gain comes from `Writer::tail_data`. `Writer::tail` zeroes
the bytes it reserves and the encoder then copied the string over them. The
bench binary is a static musl build, and in a separate measurement on cpu6 its
`memset` of 3,000 bytes took about 1,400 ns while a copy of the same bytes took
about 110 ns. The encode times before the change therefore include a `memset`
cost that depends on the C library of the build, and a device build with a
different C library can show a smaller gain. proto3 writes each byte once and
does not zero a buffer.

Verify keeps the UTF-8 validation of `core::str::from_utf8`, which ADR-0031
requires, so the change in verify comes from the NUL scan alone.

### Cortex-A55 (cpu2)

#### Mix

Equal-weight mix, per signal: each signal counts once, so a payload type that
several signals share counts once for each of them. Only signals whose three
encodings are measured are counted; the last column gives how many.

| Metric                                                 | FlatBuffers | proto3 | podbuf | FlatBuffers / podbuf | proto3 / podbuf | Units |
| ------------------------------------------------------ | ----------- | ------ | ------ | -------------------- | --------------- | ----- |
| Size (bytes)                                           | 24.5        | 3.6    | 16.0   | 1.53                 | 0.23            | 15    |
| Heap allocations per update (encode + verify + decode) | 0.0         | 0.0    | 0.0    | n/a                  | n/a             | 15    |
| Time per update (median ns)                            | 362.9       | 313.2  | 206.4  | 1.76                 | 1.52            | 15    |
| CPU % of one core at 4000 updates per second (derived) | 0.1452      | 0.1253 | 0.0825 | 1.76                 | 1.52            | 15    |

Equal-weight mix, per payload type: each payload type counts once, however many
signals share it (the value of a type is the mean over its signals). Only
payload types whose signals are measured in all three encodings are counted; the
last column gives how many.

| Metric                                                 | FlatBuffers | proto3 | podbuf | FlatBuffers / podbuf | proto3 / podbuf | Units |
| ------------------------------------------------------ | ----------- | ------ | ------ | -------------------- | --------------- | ----- |
| Size (bytes)                                           | 25.3        | 3.4    | 16.0   | 1.58                 | 0.21            | 6     |
| Heap allocations per update (encode + verify + decode) | 0.0         | 0.0    | 0.0    | n/a                  | n/a             | 6     |
| Time per update (median ns)                            | 351.3       | 301.4  | 193.6  | 1.81                 | 1.56            | 6     |
| CPU % of one core at 4000 updates per second (derived) | 0.1405      | 0.1206 | 0.0775 | 1.81                 | 1.56            | 6     |

Equal-weight mix, per signal: each signal counts once, so a payload type that
several signals share counts once for each of them. Only signals whose three
encodings are measured are counted; the last column gives how many. The rows are
per operation. A time under about 10 ns includes harness cost and is an upper
bound, so a ratio whose podbuf time is under 10 ns is not comparable.

| Metric                                                         | FlatBuffers | proto3 | podbuf | FlatBuffers / podbuf | proto3 / podbuf | Units |
| -------------------------------------------------------------- | ----------- | ------ | ------ | -------------------- | --------------- | ----- |
| Encode: time per operation (median ns)                         | 180.2       | 79.6   | 73.7   | 2.44                 | 1.08            | 15    |
| Encode: CPU % of one core at 4000 updates per second (derived) | 0.0721      | 0.0319 | 0.0295 | 2.44                 | 1.08            | 15    |
| Encode: heap allocations                                       | 0.0         | 0.0    | 0.0    | n/a                  | n/a             | 15    |
| Verify: time per operation (median ns)                         | 143.9       | 166.4  | 120.3  | 1.20                 | 1.38            | 15    |
| Verify: CPU % of one core at 4000 updates per second (derived) | 0.0576      | 0.0666 | 0.0481 | 1.20                 | 1.38            | 15    |
| Verify: heap allocations                                       | 0.0         | 0.0    | 0.0    | n/a                  | n/a             | 15    |
| Decode: time per operation (median ns)                         | 38.8        | 67.2   | 12.3   | 3.15                 | 5.46            | 15    |
| Decode: CPU % of one core at 4000 updates per second (derived) | 0.0155      | 0.0269 | 0.0049 | 3.15                 | 5.46            | 15    |
| Decode: heap allocations                                       | 0.0         | 0.0    | 0.0    | n/a                  | n/a             | 15    |

Equal-weight mix, per payload type: each payload type counts once, however many
signals share it (the value of a type is the mean over its signals). Only
payload types whose signals are measured in all three encodings are counted; the
last column gives how many. The rows are per operation. A time under about 10 ns
includes harness cost and is an upper bound, so a ratio whose podbuf time is
under 10 ns is not comparable.

| Metric                                                         | FlatBuffers | proto3 | podbuf | FlatBuffers / podbuf | proto3 / podbuf | Units |
| -------------------------------------------------------------- | ----------- | ------ | ------ | -------------------- | --------------- | ----- |
| Encode: time per operation (median ns)                         | 181.0       | 79.3   | 73.2   | 2.47                 | 1.08            | 6     |
| Encode: CPU % of one core at 4000 updates per second (derived) | 0.0724      | 0.0317 | 0.0293 | 2.47                 | 1.08            | 6     |
| Encode: heap allocations                                       | 0.0         | 0.0    | 0.0    | n/a                  | n/a             | 6     |
| Verify: time per operation (median ns)                         | 131.4       | 154.2  | 107.7  | 1.22                 | 1.43            | 6     |
| Verify: CPU % of one core at 4000 updates per second (derived) | 0.0526      | 0.0617 | 0.0431 | 1.22                 | 1.43            | 6     |
| Verify: heap allocations                                       | 0.0         | 0.0    | 0.0    | n/a                  | n/a             | 6     |
| Decode: time per operation (median ns)                         | 38.9        | 67.8   | 12.8   | 3.05                 | 5.32            | 6     |
| Decode: CPU % of one core at 4000 updates per second (derived) | 0.0156      | 0.0271 | 0.0051 | 3.05                 | 5.32            | 6     |
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
| bench.stress.Stress.bigString           | 3032        | 3003      | 3017      |
| bench.stress.Stress.bigItemList         | 4024        | 1542      | 2516      |
| bench.stress.Stress.bigSamples          | 3024        | 4089      | 3016      |
| bench.stress.Stress.bigMap              | 4344        | 1676      | 2896      |
| bench.stress.Stress.deepNested          | 2264        | 760       | 1372      |
| bench.stress.Stress.variant             | 88          | 16        | 57        |
| bench.stress.Stress.deepSparse          | 240         | 13        | 182       |
| bench.stress.Stress.deepFull            | 408         | 178       | 296       |

##### Heap allocations

Heap allocations of one encode / verify / decode.

| Payload                                 | FlatBuffers | proto3      | podbuf      |
| --------------------------------------- | ----------- | ----------- | ----------- |
| veh.cluster.VehicleStatus.currentSpeed  | 0 / 0 / 0   | 0 / 0 / 0   | 0 / 0 / 0   |
| veh.cluster.VehicleStatus.engineTemp    | 0 / 0 / 0   | 0 / 0 / 0   | 0 / 0 / 0   |
| veh.cluster.VehicleStatus.warnings      | 0 / 0 / 0   | 0 / 0 / 0   | 0 / 0 / 0   |
| veh.cluster.WheelHistory.wheelTicks     | 0 / 0 / 0   | 0 / 0 / 0   | 0 / 0 / 0   |
| veh.cluster.WheelHistory.wheelTemp      | 0 / 0 / 0   | 0 / 0 / 0   | 0 / 0 / 0   |
| veh.cluster.WheelHistory.wheelSlipRate  | 0 / 0 / 0   | 0 / 0 / 0   | 0 / 0 / 0   |
| veh.cluster.WheelSummary.wheelHealth    | 0 / 0 / 0   | 0 / 0 / 0   | 0 / 0 / 0   |
| veh.cluster.wheels.ticks                | 0 / 0 / 0   | 0 / 0 / 0   | 0 / 0 / 0   |
| veh.cluster.WheelDiagnostics.wheelTicks | not built   | not built   | not built   |
| veh.hvac.cabin.cabinTemp                | 0 / 0 / 0   | 0 / 0 / 0   | 0 / 0 / 0   |
| veh.hvac.cabin.targetTemp               | 0 / 0 / 0   | 0 / 0 / 0   | 0 / 0 / 0   |
| veh.hvac.cabin.blowerDuty               | 0 / 0 / 0   | 0 / 0 / 0   | 0 / 0 / 0   |
| veh.hvac.cabin.fanLevel                 | 0 / 0 / 0   | 0 / 0 / 0   | 0 / 0 / 0   |
| veh.hvac.cabin.compressorLoad           | 0 / 0 / 0   | 0 / 0 / 0   | 0 / 0 / 0   |
| veh.cabin.Cabin.temperature             | 0 / 0 / 0   | 0 / 0 / 0   | 0 / 0 / 0   |
| veh.cabin.Horn.active                   | 0 / 0 / 0   | 0 / 0 / 0   | 0 / 0 / 0   |
| bench.stress.Stress.longString          | 0 / 0 / 1   | 0 / 0 / 1   | 0 / 0 / 1   |
| bench.stress.Stress.itemList            | 1 / 0 / 17  | 0 / 0 / 19  | 0 / 0 / 17  |
| bench.stress.Stress.nestedOptional      | 0 / 0 / 2   | 0 / 0 / 2   | 0 / 0 / 2   |
| bench.stress.Stress.map                 | 1 / 0 / 17  | 0 / 0 / 19  | 1 / 0 / 17  |
| bench.stress.Stress.bigString           | 0 / 0 / 1   | 0 / 0 / 1   | 0 / 0 / 1   |
| bench.stress.Stress.bigItemList         | 1 / 0 / 101 | 0 / 0 / 106 | 0 / 0 / 101 |
| bench.stress.Stress.bigSamples          | 1 / 0 / 1   | 0 / 0 / 10  | 0 / 0 / 1   |
| bench.stress.Stress.bigMap              | 1 / 0 / 121 | 0 / 0 / 126 | 1 / 0 / 121 |
| bench.stress.Stress.deepNested          | 11 / 0 / 61 | 0 / 0 / 63  | 0 / 0 / 61  |
| bench.stress.Stress.variant             | 0 / 0 / 1   | 0 / 0 / 1   | 0 / 0 / 1   |
| bench.stress.Stress.deepSparse          | 0 / 0 / 1   | 0 / 0 / 1   | 0 / 0 / 1   |
| bench.stress.Stress.deepFull            | 0 / 0 / 15  | 0 / 0 / 15  | 0 / 0 / 15  |

##### Time per update

CPU time of one update (encode, verify and decode), in ns: the sum of the
medians of the three operations, with the sum of their fastest and the sum of
their slowest sample (an envelope, not a percentile) and its range as a share of
the sum. † spread above 10 %: the figure is not stable.

| Payload                                 | FlatBuffers                          | proto3                                  | podbuf                            |
| --------------------------------------- | ------------------------------------ | --------------------------------------- | --------------------------------- |
| veh.cluster.VehicleStatus.currentSpeed  | 441.6 (441.3 to 442.5; 0 %)          | 377.0 (376.9 to 377.2; 0 %)             | 286.3 (285.9 to 287.0; 0 %)       |
| veh.cluster.VehicleStatus.engineTemp    | 449.8 (449.1 to 450.8; 0 %)          | 386.2 (386.1 to 386.4; 0 %)             | 294.1 (293.9 to 295.0; 0 %)       |
| veh.cluster.VehicleStatus.warnings      | 259.8 (259.4 to 260.6; 0 %)          | 232.7 (232.6 to 233.1; 0 %)             | 105.0 (104.8 to 105.8; 1 %)       |
| veh.cluster.WheelHistory.wheelTicks     | 261.2 (260.7 to 262.4; 1 %)          | 230.7 (230.7 to 230.8; 0 %)             | 105.7 (105.0 to 106.1; 1 %)       |
| veh.cluster.WheelHistory.wheelTemp      | 449.6 (449.1 to 450.5; 0 %)          | 386.2 (386.1 to 386.8; 0 %)             | 294.1 (293.9 to 294.4; 0 %)       |
| veh.cluster.WheelHistory.wheelSlipRate  | 451.8 (451.3 to 453.3; 0 %)          | 386.0 (385.9 to 386.5; 0 %)             | 295.7 (295.6 to 296.5; 0 %)       |
| veh.cluster.WheelSummary.wheelHealth    | 261.3 (260.8 to 263.1; 1 %)          | 230.7 (230.7 to 230.9; 0 %)             | 105.7 (105.2 to 106.5; 1 %)       |
| veh.cluster.wheels.ticks                | 261.3 (260.7 to 262.5; 1 %)          | 230.7 (230.7 to 230.9; 0 %)             | 105.7 (105.3 to 106.3; 1 %)       |
| veh.cluster.WheelDiagnostics.wheelTicks | not built                            | not built                               | not built                         |
| veh.hvac.cabin.cabinTemp                | 449.7 (449.2 to 450.5; 0 %)          | 386.2 (386.1 to 386.9; 0 %)             | 294.2 (293.9 to 294.7; 0 %)       |
| veh.hvac.cabin.targetTemp               | 449.8 (449.1 to 450.6; 0 %)          | 386.2 (386.1 to 386.4; 0 %)             | 294.1 (293.9 to 294.9; 0 %)       |
| veh.hvac.cabin.blowerDuty               | 451.8 (451.3 to 453.2; 0 %)          | 386.0 (385.9 to 386.3; 0 %)             | 295.8 (295.5 to 296.4; 0 %)       |
| veh.hvac.cabin.fanLevel                 | 261.4 (261.0 to 262.9; 1 %)          | 230.7 (230.7 to 230.9; 0 %)             | 105.9 (105.2 to 106.6; 1 %)       |
| veh.hvac.cabin.compressorLoad           | 451.9 (451.3 to 453.3; 0 %)          | 386.0 (385.9 to 386.4; 0 %)             | 295.8 (295.5 to 296.2; 0 %)       |
| veh.cabin.Cabin.temperature             | 260.9 (260.6 to 261.6; 0 %)          | 237.5 (237.2 to 237.9; 0 %)             | 104.8 (104.6 to 105.6; 1 %)       |
| veh.cabin.Horn.active                   | 281.5 (277.8 to 286.2; 3 %)          | 225.5 (225.3 to 225.8; 0 %)             | 112.7 (112.6 to 113.3; 1 %)       |
| bench.stress.Stress.longString          | 2059.0 (2056.4 to 2063.9; 0 %)       | 1458.1 (1454.2 to 1463.4; 1 %)          | 2142.8 (2141.4 to 2145.2; 0 %)    |
| bench.stress.Stress.itemList            | 10342.3 (10331.1 to 10365.9; 0 %)    | 13991.1 (13976.6 to 14017.7; 0 %)       | 7895.8 (7890.6 to 7907.5; 0 %)    |
| bench.stress.Stress.nestedOptional      | 1462.2 (1452.6 to 1470.6; 1 %)       | 1622.5 (1615.7 to 1631.9; 1 %)          | 982.7 (979.4 to 986.8; 1 %)       |
| bench.stress.Stress.map                 | 21487.2 (21472.8 to 21517.8; 0 %)    | 39675.4 (39638.3 to 39712.7; 0 %)       | 9559.7 (9542.7 to 9582.9; 0 %)    |
| bench.stress.Stress.bigString           | 8496.6 (8491.3 to 8502.9; 0 %)       | 5125.7 (5119.2 to 5136.6; 0 %)          | 10293.9 (10289.2 to 10303.9; 0 %) |
| bench.stress.Stress.bigItemList         | 65358.1 (65064.6 to 65668.2; 1 %)    | 88378.8 (88290.8 to 88500.3; 0 %)       | 53963.3 (53875.8 to 54130.6; 0 %) |
| bench.stress.Stress.bigSamples          | 35406.0 (35397.5 to 35455.6; 0 %)    | 140353.2 (140259.4 to 140457.9; 0 %)    | 23703.9 (23699.7 to 23719.7; 0 %) |
| bench.stress.Stress.bigMap              | 729608.4 (729342.8 to 730903.7; 0 %) | 1570108.6 (1569149.7 to 1571502.5; 0 %) | 67563.3 (67514.0 to 67647.5; 0 %) |
| bench.stress.Stress.deepNested          | 41239.4 (41187.9 to 41316.8; 0 %)    | 51044.9 (51003.2 to 51090.3; 0 %)       | 30365.7 (30289.6 to 30514.7; 1 %) |
| bench.stress.Stress.variant             | 1342.6 (1338.7 to 1345.8; 1 %)       | 1237.0 (1236.8 to 1237.9; 0 %)          | 817.2 (816.8 to 818.2; 0 %)       |
| bench.stress.Stress.deepSparse          | 2869.3 (2864.2 to 2877.4; 0 %)       | 2130.9 (2128.4 to 2133.8; 0 %)          | 2099.2 (2097.6 to 2101.9; 0 %)    |
| bench.stress.Stress.deepFull            | 8530.3 (8523.2 to 8543.4; 0 %)       | 11225.9 (11209.6 to 11239.8; 0 %)       | 7327.9 (7319.9 to 7340.7; 0 %)    |

##### CPU load, derived

CPU load in percent of one core at 4000 updates per second, derived from the
median time of one update (the headline figure).

| Payload                                 | FlatBuffers | proto3    | podbuf    |
| --------------------------------------- | ----------- | --------- | --------- |
| veh.cluster.VehicleStatus.currentSpeed  | 0.1766      | 0.1508    | 0.1145    |
| veh.cluster.VehicleStatus.engineTemp    | 0.1799      | 0.1545    | 0.1176    |
| veh.cluster.VehicleStatus.warnings      | 0.1039      | 0.0931    | 0.0420    |
| veh.cluster.WheelHistory.wheelTicks     | 0.1045      | 0.0923    | 0.0423    |
| veh.cluster.WheelHistory.wheelTemp      | 0.1798      | 0.1545    | 0.1176    |
| veh.cluster.WheelHistory.wheelSlipRate  | 0.1807      | 0.1544    | 0.1183    |
| veh.cluster.WheelSummary.wheelHealth    | 0.1045      | 0.0923    | 0.0423    |
| veh.cluster.wheels.ticks                | 0.1045      | 0.0923    | 0.0423    |
| veh.cluster.WheelDiagnostics.wheelTicks | not built   | not built | not built |
| veh.hvac.cabin.cabinTemp                | 0.1799      | 0.1545    | 0.1177    |
| veh.hvac.cabin.targetTemp               | 0.1799      | 0.1545    | 0.1176    |
| veh.hvac.cabin.blowerDuty               | 0.1807      | 0.1544    | 0.1183    |
| veh.hvac.cabin.fanLevel                 | 0.1046      | 0.0923    | 0.0424    |
| veh.hvac.cabin.compressorLoad           | 0.1807      | 0.1544    | 0.1183    |
| veh.cabin.Cabin.temperature             | 0.1044      | 0.0950    | 0.0419    |
| veh.cabin.Horn.active                   | 0.1126      | 0.0902    | 0.0451    |
| bench.stress.Stress.longString          | 0.8236      | 0.5832    | 0.8571    |
| bench.stress.Stress.itemList            | 4.1369      | 5.5964    | 3.1583    |
| bench.stress.Stress.nestedOptional      | 0.5849      | 0.6490    | 0.3931    |
| bench.stress.Stress.map                 | 8.5949      | 15.8701   | 3.8239    |
| bench.stress.Stress.bigString           | 3.3986      | 2.0503    | 4.1176    |
| bench.stress.Stress.bigItemList         | 26.1433     | 35.3515   | 21.5853   |
| bench.stress.Stress.bigSamples          | 14.1624     | 56.1413   | 9.4815    |
| bench.stress.Stress.bigMap              | 291.8434    | 628.0434  | 27.0253   |
| bench.stress.Stress.deepNested          | 16.4958     | 20.4180   | 12.1463   |
| bench.stress.Stress.variant             | 0.5370      | 0.4948    | 0.3269    |
| bench.stress.Stress.deepSparse          | 1.1477      | 0.8524    | 0.8397    |
| bench.stress.Stress.deepFull            | 3.4121      | 4.4904    | 2.9311    |

##### Ratios to podbuf

Each encoding divided by podbuf: above 1.00 the encoding is larger or slower
than podbuf, below 1.00 smaller or faster. The CPU ratios equal the time ratios,
because the derived CPU load is proportional to the median time. A refused or
not-built cell has no ratio and is marked.

| Payload                                 | Size: FlatBuffers / podbuf | Size: proto3 / podbuf | Time: FlatBuffers / podbuf | Time: proto3 / podbuf | CPU: FlatBuffers / podbuf | CPU: proto3 / podbuf |
| --------------------------------------- | -------------------------- | --------------------- | -------------------------- | --------------------- | ------------------------- | -------------------- |
| veh.cluster.VehicleStatus.currentSpeed  | 1.50                       | 0.31                  | 1.54                       | 1.32                  | 1.54                      | 1.32                 |
| veh.cluster.VehicleStatus.engineTemp    | 1.50                       | 0.31                  | 1.53                       | 1.31                  | 1.53                      | 1.31                 |
| veh.cluster.VehicleStatus.warnings      | 1.50                       | 0.12                  | 2.47                       | 2.22                  | 2.47                      | 2.22                 |
| veh.cluster.WheelHistory.wheelTicks     | 1.50                       | 0.12                  | 2.47                       | 2.18                  | 2.47                      | 2.18                 |
| veh.cluster.WheelHistory.wheelTemp      | 1.50                       | 0.31                  | 1.53                       | 1.31                  | 1.53                      | 1.31                 |
| veh.cluster.WheelHistory.wheelSlipRate  | 1.50                       | 0.31                  | 1.53                       | 1.31                  | 1.53                      | 1.31                 |
| veh.cluster.WheelSummary.wheelHealth    | 1.50                       | 0.12                  | 2.47                       | 2.18                  | 2.47                      | 2.18                 |
| veh.cluster.wheels.ticks                | 1.50                       | 0.12                  | 2.47                       | 2.18                  | 2.47                      | 2.18                 |
| veh.cluster.WheelDiagnostics.wheelTicks | not built                  | not built             | not built                  | not built             | not built                 | not built            |
| veh.hvac.cabin.cabinTemp                | 1.50                       | 0.31                  | 1.53                       | 1.31                  | 1.53                      | 1.31                 |
| veh.hvac.cabin.targetTemp               | 1.50                       | 0.31                  | 1.53                       | 1.31                  | 1.53                      | 1.31                 |
| veh.hvac.cabin.blowerDuty               | 1.50                       | 0.31                  | 1.53                       | 1.30                  | 1.53                      | 1.30                 |
| veh.hvac.cabin.fanLevel                 | 1.50                       | 0.12                  | 2.47                       | 2.18                  | 2.47                      | 2.18                 |
| veh.hvac.cabin.compressorLoad           | 1.50                       | 0.31                  | 1.53                       | 1.31                  | 1.53                      | 1.31                 |
| veh.cabin.Cabin.temperature             | 1.50                       | 0.12                  | 2.49                       | 2.27                  | 2.49                      | 2.27                 |
| veh.cabin.Horn.active                   | 2.00                       | 0.12                  | 2.50                       | 2.00                  | 2.50                      | 2.00                 |
| bench.stress.Stress.longString          | 1.03                       | 0.97                  | 0.96                       | 0.68                  | 0.96                      | 0.68                 |
| bench.stress.Stress.itemList            | 1.50                       | 0.55                  | 1.31                       | 1.77                  | 1.31                      | 1.77                 |
| bench.stress.Stress.nestedOptional      | 1.63                       | 0.35                  | 1.49                       | 1.65                  | 1.49                      | 1.65                 |
| bench.stress.Stress.map                 | 1.56                       | 0.53                  | 2.25                       | 4.15                  | 2.25                      | 4.15                 |
| bench.stress.Stress.bigString           | 1.00                       | 1.00                  | 0.83                       | 0.50                  | 0.83                      | 0.50                 |
| bench.stress.Stress.bigItemList         | 1.60                       | 0.61                  | 1.21                       | 1.64                  | 1.21                      | 1.64                 |
| bench.stress.Stress.bigSamples          | 1.00                       | 1.36                  | 1.49                       | 5.92                  | 1.49                      | 5.92                 |
| bench.stress.Stress.bigMap              | 1.50                       | 0.58                  | 10.80                      | 23.24                 | 10.80                     | 23.24                |
| bench.stress.Stress.deepNested          | 1.65                       | 0.55                  | 1.36                       | 1.68                  | 1.36                      | 1.68                 |
| bench.stress.Stress.variant             | 1.54                       | 0.28                  | 1.64                       | 1.51                  | 1.64                      | 1.51                 |
| bench.stress.Stress.deepSparse          | 1.32                       | 0.07                  | 1.37                       | 1.02                  | 1.37                      | 1.02                 |
| bench.stress.Stress.deepFull            | 1.38                       | 0.60                  | 1.16                       | 1.53                  | 1.16                      | 1.53                 |

##### Time per operation, encode

CPU time of the encode operation of one update, in ns: the median of 30 samples
of a batch of this operation alone, with the fastest and the slowest sample and
the range as a share of the median. † spread above 10 %: the figure is not
stable.

| Payload                                 | FlatBuffers                       | proto3                               | podbuf                            |
| --------------------------------------- | --------------------------------- | ------------------------------------ | --------------------------------- |
| veh.cluster.VehicleStatus.currentSpeed  | 180.4 (180.2 to 181.1; 1 %)       | 76.4 (76.3 to 76.5; 0 %)             | 74.5 (74.1 to 75.1; 1 %)          |
| veh.cluster.VehicleStatus.engineTemp    | 181.3 (180.7 to 182.1; 1 %)       | 76.0 (76.0 to 76.1; 0 %)             | 75.1 (74.9 to 75.7; 1 %)          |
| veh.cluster.VehicleStatus.warnings      | 175.5 (175.2 to 176.2; 1 %)       | 84.8 (84.8 to 85.0; 0 %)             | 71.1 (71.0 to 71.9; 1 %)          |
| veh.cluster.WheelHistory.wheelTicks     | 177.5 (177.0 to 178.5; 1 %)       | 84.9 (84.8 to 84.9; 0 %)             | 73.0 (72.3 to 73.3; 1 %)          |
| veh.cluster.WheelHistory.wheelTemp      | 181.2 (180.7 to 181.8; 1 %)       | 76.0 (76.0 to 76.1; 0 %)             | 75.0 (74.9 to 75.3; 1 %)          |
| veh.cluster.WheelHistory.wheelSlipRate  | 181.2 (180.7 to 182.4; 1 %)       | 76.0 (76.0 to 76.0; 0 %)             | 75.0 (74.9 to 75.5; 1 %)          |
| veh.cluster.WheelSummary.wheelHealth    | 177.5 (177.1 to 179.2; 1 %)       | 84.8 (84.8 to 85.0; 0 %)             | 73.0 (72.5 to 73.8; 2 %)          |
| veh.cluster.wheels.ticks                | 177.6 (177.0 to 178.6; 1 %)       | 84.8 (84.8 to 84.9; 0 %)             | 73.0 (72.6 to 73.5; 1 %)          |
| veh.cluster.WheelDiagnostics.wheelTicks | not built                         | not built                            | not built                         |
| veh.hvac.cabin.cabinTemp                | 181.3 (180.8 to 181.8; 1 %)       | 76.0 (76.0 to 76.4; 1 %)             | 75.1 (74.9 to 75.5; 1 %)          |
| veh.hvac.cabin.targetTemp               | 181.3 (180.7 to 182.0; 1 %)       | 76.0 (76.0 to 76.0; 0 %)             | 75.0 (74.9 to 75.6; 1 %)          |
| veh.hvac.cabin.blowerDuty               | 181.1 (180.7 to 182.3; 1 %)       | 76.0 (76.0 to 76.1; 0 %)             | 75.1 (74.9 to 75.6; 1 %)          |
| veh.hvac.cabin.fanLevel                 | 177.6 (177.2 to 179.1; 1 %)       | 84.8 (84.8 to 84.9; 0 %)             | 73.2 (72.5 to 73.8; 2 %)          |
| veh.hvac.cabin.compressorLoad           | 181.2 (180.7 to 182.5; 1 %)       | 76.0 (76.0 to 76.1; 0 %)             | 75.1 (74.9 to 75.4; 1 %)          |
| veh.cabin.Cabin.temperature             | 176.6 (176.3 to 177.3; 1 %)       | 86.1 (85.8 to 86.4; 1 %)             | 71.5 (71.3 to 72.3; 1 %)          |
| veh.cabin.Horn.active                   | 191.0 (187.5 to 195.5; 4 %)       | 76.0 (76.0 to 76.0; 0 %)             | 71.2 (71.0 to 71.7; 1 %)          |
| bench.stress.Stress.longString          | 924.1 (922.4 to 926.5; 0 %)       | 232.1 (230.6 to 234.4; 2 %)          | 997.2 (996.7 to 997.9; 0 %)       |
| bench.stress.Stress.itemList            | 3784.7 (3778.1 to 3800.9; 1 %)    | 3365.8 (3364.8 to 3370.1; 0 %)       | 2021.8 (2019.8 to 2025.4; 0 %)    |
| bench.stress.Stress.nestedOptional      | 628.9 (624.1 to 631.4; 1 %)       | 433.5 (431.9 to 434.5; 1 %)          | 262.1 (261.6 to 262.6; 0 %)       |
| bench.stress.Stress.map                 | 3797.2 (3792.6 to 3805.6; 0 %)    | 6518.7 (6513.5 to 6523.2; 0 %)       | 2775.8 (2774.4 to 2777.9; 0 %)    |
| bench.stress.Stress.bigString           | 4211.8 (4209.3 to 4213.9; 0 %)    | 751.1 (749.6 to 753.0; 0 %)          | 5164.2 (5162.6 to 5168.4; 0 %)    |
| bench.stress.Stress.bigItemList         | 22571.3 (22465.2 to 22662.8; 1 %) | 21583.6 (21579.1 to 21603.2; 0 %)    | 12997.0 (12983.0 to 13017.2; 0 %) |
| bench.stress.Stress.bigSamples          | 10177.6 (10173.4 to 10192.3; 0 %) | 102447.8 (102425.2 to 102513.3; 0 %) | 9287.2 (9284.9 to 9291.4; 0 %)    |
| bench.stress.Stress.bigMap              | 25658.5 (25610.4 to 25735.9; 0 %) | 192681.4 (192650.1 to 192815.1; 0 %) | 19955.9 (19941.1 to 19973.3; 0 %) |
| bench.stress.Stress.deepNested          | 15809.6 (15791.1 to 15839.5; 0 %) | 13109.7 (13106.1 to 13118.9; 0 %)    | 7127.6 (7124.3 to 7137.5; 0 %)    |
| bench.stress.Stress.variant             | 606.5 (602.8 to 608.9; 1 %)       | 318.4 (318.3 to 318.5; 0 %)          | 294.2 (293.9 to 294.8; 0 %)       |
| bench.stress.Stress.deepSparse          | 888.1 (884.3 to 893.2; 1 %)       | 510.6 (509.9 to 511.1; 0 %)          | 520.1 (519.9 to 520.3; 0 %)       |
| bench.stress.Stress.deepFull            | 2424.0 (2421.0 to 2429.1; 0 %)    | 2568.8 (2568.2 to 2571.8; 0 %)       | 1730.3 (1726.4 to 1734.6; 0 %)    |

##### Time per operation, verify

CPU time of the verify operation of one update, in ns: the median of 30 samples
of a batch of this operation alone, with the fastest and the slowest sample and
the range as a share of the median. † spread above 10 %: the figure is not
stable.

| Payload                                 | FlatBuffers                          | proto3                                  | podbuf                            |
| --------------------------------------- | ------------------------------------ | --------------------------------------- | --------------------------------- |
| veh.cluster.VehicleStatus.currentSpeed  | 220.2 (220.1 to 220.3; 0 %)          | 236.2 (236.2 to 236.4; 0 %)             | 196.3 (196.3 to 196.4; 0 %)       |
| veh.cluster.VehicleStatus.engineTemp    | 227.4 (227.4 to 227.6; 0 %)          | 245.9 (245.8 to 246.0; 0 %)             | 203.5 (203.5 to 203.8; 0 %)       |
| veh.cluster.VehicleStatus.warnings      | 48.2 (48.2 to 48.3; 0 %)             | 77.5 (77.3 to 77.6; 0 %)                | 25.5 (25.5 to 25.5; 0 %)          |
| veh.cluster.WheelHistory.wheelTicks     | 48.3 (48.2 to 48.3; 0 %)             | 76.5 (76.5 to 76.6; 0 %)                | 25.5 (25.5 to 25.5; 0 %)          |
| veh.cluster.WheelHistory.wheelTemp      | 227.4 (227.4 to 227.5; 0 %)          | 245.9 (245.8 to 246.3; 0 %)             | 203.5 (203.5 to 203.6; 0 %)       |
| veh.cluster.WheelHistory.wheelSlipRate  | 229.6 (229.6 to 229.8; 0 %)          | 245.7 (245.7 to 246.1; 0 %)             | 205.2 (205.2 to 205.4; 0 %)       |
| veh.cluster.WheelSummary.wheelHealth    | 48.2 (48.2 to 48.3; 0 %)             | 76.5 (76.5 to 76.6; 0 %)                | 25.5 (25.5 to 25.5; 0 %)          |
| veh.cluster.wheels.ticks                | 48.2 (48.2 to 48.3; 0 %)             | 76.5 (76.5 to 76.6; 0 %)                | 25.5 (25.5 to 25.5; 0 %)          |
| veh.cluster.WheelDiagnostics.wheelTicks | not built                            | not built                               | not built                         |
| veh.hvac.cabin.cabinTemp                | 227.4 (227.4 to 227.6; 0 %)          | 245.9 (245.8 to 246.1; 0 %)             | 203.5 (203.5 to 203.6; 0 %)       |
| veh.hvac.cabin.targetTemp               | 227.4 (227.4 to 227.5; 0 %)          | 245.9 (245.8 to 246.0; 0 %)             | 203.5 (203.5 to 203.7; 0 %)       |
| veh.hvac.cabin.blowerDuty               | 229.6 (229.6 to 229.7; 0 %)          | 245.7 (245.7 to 245.8; 0 %)             | 205.2 (205.2 to 205.2; 0 %)       |
| veh.hvac.cabin.fanLevel                 | 48.2 (48.2 to 48.3; 0 %)             | 76.5 (76.5 to 76.7; 0 %)                | 25.5 (25.5 to 25.6; 0 %)          |
| veh.hvac.cabin.compressorLoad           | 229.6 (229.6 to 229.8; 0 %)          | 245.7 (245.6 to 245.9; 0 %)             | 205.2 (205.2 to 205.3; 0 %)       |
| veh.cabin.Cabin.temperature             | 48.8 (48.8 to 48.8; 0 %)             | 78.7 (78.7 to 78.9; 0 %)                | 26.1 (26.1 to 26.1; 0 %)          |
| veh.cabin.Horn.active                   | 50.5 (50.5 to 50.5; 0 %)             | 76.9 (76.7 to 77.0; 0 %)                | 25.5 (25.5 to 25.5; 0 %)          |
| bench.stress.Stress.longString          | 669.2 (668.8 to 670.0; 0 %)          | 708.6 (707.3 to 709.9; 0 %)             | 758.0 (757.5 to 758.7; 0 %)       |
| bench.stress.Stress.itemList            | 2512.5 (2509.0 to 2515.9; 0 %)       | 4263.3 (4254.9 to 4273.2; 0 %)          | 2422.8 (2420.4 to 2429.7; 0 %)    |
| bench.stress.Stress.nestedOptional      | 317.5 (314.9 to 319.5; 1 %)          | 469.0 (466.6 to 472.4; 1 %)             | 288.4 (286.9 to 289.8; 1 %)       |
| bench.stress.Stress.map                 | 13957.6 (13949.2 to 13977.2; 0 %)    | 27104.2 (27075.8 to 27129.9; 0 %)       | 3383.0 (3377.1 to 3391.2; 0 %)    |
| bench.stress.Stress.bigString           | 2706.7 (2704.3 to 2709.2; 0 %)       | 2747.0 (2743.3 to 2752.7; 0 %)          | 3770.1 (3767.4 to 3774.0; 0 %)    |
| bench.stress.Stress.bigItemList         | 17549.5 (17471.1 to 17718.5; 1 %)    | 28585.9 (28528.9 to 28649.0; 0 %)       | 18561.8 (18534.5 to 18605.4; 0 %) |
| bench.stress.Stress.bigSamples          | 9221.8 (9220.9 to 9233.6; 0 %)       | 14075.8 (14073.9 to 14087.4; 0 %)       | 69.9 (69.9 to 69.9; 0 %)          |
| bench.stress.Stress.bigMap              | 676830.1 (676619.3 to 678020.8; 0 %) | 1334612.1 (1333704.2 to 1335847.1; 0 %) | 24941.3 (24914.4 to 24987.3; 0 %) |
| bench.stress.Stress.deepNested          | 10050.5 (10037.8 to 10068.0; 0 %)    | 15569.3 (15547.0 to 15578.8; 0 %)       | 9843.8 (9790.7 to 9950.5; 2 %)    |
| bench.stress.Stress.variant             | 325.6 (325.4 to 325.7; 0 %)          | 380.7 (380.6 to 380.9; 0 %)             | 250.1 (250.0 to 250.3; 0 %)       |
| bench.stress.Stress.deepSparse          | 764.6 (764.1 to 765.6; 0 %)          | 393.0 (392.8 to 393.3; 0 %)             | 893.0 (891.6 to 894.8; 0 %)       |
| bench.stress.Stress.deepFull            | 2328.1 (2325.6 to 2331.4; 0 %)       | 3352.2 (3345.3 to 3357.5; 0 %)          | 2349.1 (2346.1 to 2354.8; 0 %)    |

##### Time per operation, decode

CPU time of the decode operation of one update, in ns: the median of 30 samples
of a batch of this operation alone, with the fastest and the slowest sample and
the range as a share of the median. † spread above 10 %: the figure is not
stable.

| Payload                                 | FlatBuffers                       | proto3                            | podbuf                            |
| --------------------------------------- | --------------------------------- | --------------------------------- | --------------------------------- |
| veh.cluster.VehicleStatus.currentSpeed  | 41.0 (41.0 to 41.1; 0 %)          | 64.3 (64.3 to 64.4; 0 %)          | 15.5 (15.5 to 15.5; 0 %)          |
| veh.cluster.VehicleStatus.engineTemp    | 41.1 (41.0 to 41.1; 0 %)          | 64.3 (64.3 to 64.3; 0 %)          | 15.5 (15.5 to 15.5; 0 %)          |
| veh.cluster.VehicleStatus.warnings      | 36.0 (36.0 to 36.1; 0 %)          | 70.4 (70.4 to 70.5; 0 %)          | 8.3 (8.3 to 8.3; 0 %)             |
| veh.cluster.WheelHistory.wheelTicks     | 35.5 (35.5 to 35.6; 0 %)          | 69.3 (69.3 to 69.3; 0 %)          | 7.2 (7.2 to 7.2; 0 %)             |
| veh.cluster.WheelHistory.wheelTemp      | 41.0 (41.0 to 41.2; 0 %)          | 64.3 (64.3 to 64.4; 0 %)          | 15.5 (15.5 to 15.5; 0 %)          |
| veh.cluster.WheelHistory.wheelSlipRate  | 41.1 (41.0 to 41.1; 0 %)          | 64.3 (64.3 to 64.4; 0 %)          | 15.5 (15.5 to 15.5; 0 %)          |
| veh.cluster.WheelSummary.wheelHealth    | 35.5 (35.5 to 35.6; 0 %)          | 69.3 (69.3 to 69.4; 0 %)          | 7.2 (7.2 to 7.2; 0 %)             |
| veh.cluster.wheels.ticks                | 35.5 (35.5 to 35.6; 0 %)          | 69.3 (69.3 to 69.4; 0 %)          | 7.2 (7.2 to 7.2; 0 %)             |
| veh.cluster.WheelDiagnostics.wheelTicks | not built                         | not built                         | not built                         |
| veh.hvac.cabin.cabinTemp                | 41.1 (41.0 to 41.1; 0 %)          | 64.3 (64.3 to 64.4; 0 %)          | 15.5 (15.5 to 15.5; 0 %)          |
| veh.hvac.cabin.targetTemp               | 41.1 (41.0 to 41.2; 0 %)          | 64.3 (64.3 to 64.4; 0 %)          | 15.5 (15.5 to 15.5; 0 %)          |
| veh.hvac.cabin.blowerDuty               | 41.1 (41.0 to 41.1; 0 %)          | 64.3 (64.3 to 64.4; 0 %)          | 15.5 (15.5 to 15.5; 0 %)          |
| veh.hvac.cabin.fanLevel                 | 35.5 (35.5 to 35.6; 0 %)          | 69.3 (69.3 to 69.3; 0 %)          | 7.2 (7.2 to 7.2; 0 %)             |
| veh.hvac.cabin.compressorLoad           | 41.1 (41.0 to 41.1; 0 %)          | 64.3 (64.3 to 64.4; 0 %)          | 15.5 (15.5 to 15.5; 0 %)          |
| veh.cabin.Cabin.temperature             | 35.5 (35.5 to 35.5; 0 %)          | 72.6 (72.6 to 72.7; 0 %)          | 7.2 (7.2 to 7.2; 0 %)             |
| veh.cabin.Horn.active                   | 40.1 (39.9 to 40.3; 1 %)          | 72.6 (72.6 to 72.7; 0 %)          | 16.1 (16.1 to 16.1; 0 %)          |
| bench.stress.Stress.longString          | 465.8 (465.2 to 467.4; 0 %)       | 517.4 (516.3 to 519.1; 1 %)       | 387.6 (387.2 to 388.7; 0 %)       |
| bench.stress.Stress.itemList            | 4045.1 (4043.9 to 4049.1; 0 %)    | 6362.0 (6356.9 to 6374.4; 0 %)    | 3451.2 (3450.4 to 3452.4; 0 %)    |
| bench.stress.Stress.nestedOptional      | 515.8 (513.6 to 519.8; 1 %)       | 720.1 (717.2 to 725.1; 1 %)       | 432.2 (431.0 to 434.5; 1 %)       |
| bench.stress.Stress.map                 | 3732.4 (3731.1 to 3735.0; 0 %)    | 6052.5 (6049.1 to 6059.6; 0 %)    | 3400.8 (3391.1 to 3413.8; 1 %)    |
| bench.stress.Stress.bigString           | 1578.0 (1577.7 to 1579.8; 0 %)    | 1627.6 (1626.3 to 1630.9; 0 %)    | 1359.6 (1359.2 to 1361.5; 0 %)    |
| bench.stress.Stress.bigItemList         | 25237.3 (25128.3 to 25286.9; 1 %) | 38209.3 (38182.8 to 38248.1; 0 %) | 22404.5 (22358.4 to 22508.0; 1 %) |
| bench.stress.Stress.bigSamples          | 16006.6 (16003.2 to 16029.7; 0 %) | 23829.7 (23760.3 to 23857.3; 0 %) | 14346.8 (14344.9 to 14358.5; 0 %) |
| bench.stress.Stress.bigMap              | 27119.9 (27113.1 to 27147.0; 0 %) | 42815.1 (42795.4 to 42840.3; 0 %) | 22666.1 (22658.4 to 22687.0; 0 %) |
| bench.stress.Stress.deepNested          | 15379.3 (15359.0 to 15409.3; 0 %) | 22365.9 (22350.1 to 22392.7; 0 %) | 13394.3 (13374.6 to 13426.7; 0 %) |
| bench.stress.Stress.variant             | 410.5 (410.4 to 411.1; 0 %)       | 537.9 (537.8 to 538.5; 0 %)       | 272.9 (272.9 to 273.2; 0 %)       |
| bench.stress.Stress.deepSparse          | 1216.6 (1215.8 to 1218.5; 0 %)    | 1227.3 (1225.7 to 1229.3; 0 %)    | 686.2 (686.0 to 686.8; 0 %)       |
| bench.stress.Stress.deepFull            | 3778.2 (3776.7 to 3782.9; 0 %)    | 5304.9 (5296.2 to 5310.5; 0 %)    | 3248.4 (3247.4 to 3251.2; 0 %)    |

##### Time per operation, access

CPU time of reading one number, deep in the payload, through the view of
verified bytes, in ns: a count, or for the sparse payloads the length of a
string field. The median of 30 samples, with the fastest and the slowest sample
and the range as a share of the median. It stands for the decode when a consumer
needs one field, and it is not part of the time per update. The FlatBuffers
accessor of a list returns the decoded list, so there the access of a list
element costs the decode of the list. proto3 generates no accessors, so its
cells are n/a. † spread above 10 %: the figure is not stable.

| Payload                         | FlatBuffers                       | proto3            | podbuf                      |
| ------------------------------- | --------------------------------- | ----------------- | --------------------------- |
| bench.stress.Stress.bigItemList | 25148.0 (25065.5 to 25227.9; 1 %) | n/a: no accessors | 134.0 (133.7 to 134.3; 0 %) |
| bench.stress.Stress.deepNested  | 15382.6 (15357.2 to 15410.3; 0 %) | n/a: no accessors | 130.3 (130.3 to 130.5; 0 %) |
| bench.stress.Stress.deepSparse  | 139.2 (139.2 to 139.3; 0 %)       | n/a: no accessors | 71.6 (71.5 to 71.6; 0 %)    |
| bench.stress.Stress.deepFull    | 143.8 (143.7 to 144.0; 0 %)       | n/a: no accessors | 76.0 (76.0 to 76.1; 0 %)    |

##### CPU load per operation

CPU load in percent of one core at 4000 updates per second of each operation
alone, derived from the median time of that operation. The three operations of
an encoding add up to its derived load of one update.

| Payload                                 | Encode: FlatBuffers | Encode: proto3 | Encode: podbuf | Verify: FlatBuffers | Verify: proto3 | Verify: podbuf | Decode: FlatBuffers | Decode: proto3 | Decode: podbuf |
| --------------------------------------- | ------------------- | -------------- | -------------- | ------------------- | -------------- | -------------- | ------------------- | -------------- | -------------- |
| veh.cluster.VehicleStatus.currentSpeed  | 0.0722              | 0.0306         | 0.0298         | 0.0881              | 0.0945         | 0.0785         | 0.0164              | 0.0257         | 0.0062         |
| veh.cluster.VehicleStatus.engineTemp    | 0.0725              | 0.0304         | 0.0300         | 0.0910              | 0.0984         | 0.0814         | 0.0164              | 0.0257         | 0.0062         |
| veh.cluster.VehicleStatus.warnings      | 0.0702              | 0.0339         | 0.0285         | 0.0193              | 0.0310         | 0.0102         | 0.0144              | 0.0282         | 0.0033         |
| veh.cluster.WheelHistory.wheelTicks     | 0.0710              | 0.0339         | 0.0292         | 0.0193              | 0.0306         | 0.0102         | 0.0142              | 0.0277         | 0.0029         |
| veh.cluster.WheelHistory.wheelTemp      | 0.0725              | 0.0304         | 0.0300         | 0.0910              | 0.0984         | 0.0814         | 0.0164              | 0.0257         | 0.0062         |
| veh.cluster.WheelHistory.wheelSlipRate  | 0.0725              | 0.0304         | 0.0300         | 0.0918              | 0.0983         | 0.0821         | 0.0164              | 0.0257         | 0.0062         |
| veh.cluster.WheelSummary.wheelHealth    | 0.0710              | 0.0339         | 0.0292         | 0.0193              | 0.0306         | 0.0102         | 0.0142              | 0.0277         | 0.0029         |
| veh.cluster.wheels.ticks                | 0.0710              | 0.0339         | 0.0292         | 0.0193              | 0.0306         | 0.0102         | 0.0142              | 0.0277         | 0.0029         |
| veh.cluster.WheelDiagnostics.wheelTicks | not built           | not built      | not built      | not built           | not built      | not built      | not built           | not built      | not built      |
| veh.hvac.cabin.cabinTemp                | 0.0725              | 0.0304         | 0.0300         | 0.0910              | 0.0984         | 0.0814         | 0.0164              | 0.0257         | 0.0062         |
| veh.hvac.cabin.targetTemp               | 0.0725              | 0.0304         | 0.0300         | 0.0910              | 0.0983         | 0.0814         | 0.0164              | 0.0257         | 0.0062         |
| veh.hvac.cabin.blowerDuty               | 0.0725              | 0.0304         | 0.0301         | 0.0918              | 0.0983         | 0.0821         | 0.0164              | 0.0257         | 0.0062         |
| veh.hvac.cabin.fanLevel                 | 0.0711              | 0.0339         | 0.0293         | 0.0193              | 0.0306         | 0.0102         | 0.0142              | 0.0277         | 0.0029         |
| veh.hvac.cabin.compressorLoad           | 0.0725              | 0.0304         | 0.0300         | 0.0918              | 0.0983         | 0.0821         | 0.0164              | 0.0257         | 0.0062         |
| veh.cabin.Cabin.temperature             | 0.0707              | 0.0344         | 0.0286         | 0.0195              | 0.0315         | 0.0104         | 0.0142              | 0.0291         | 0.0029         |
| veh.cabin.Horn.active                   | 0.0764              | 0.0304         | 0.0285         | 0.0202              | 0.0308         | 0.0102         | 0.0160              | 0.0291         | 0.0064         |
| bench.stress.Stress.longString          | 0.3696              | 0.0928         | 0.3989         | 0.2677              | 0.2834         | 0.3032         | 0.1863              | 0.2069         | 0.1551         |
| bench.stress.Stress.itemList            | 1.5139              | 1.3463         | 0.8087         | 1.0050              | 1.7053         | 0.9691         | 1.6180              | 2.5448         | 1.3805         |
| bench.stress.Stress.nestedOptional      | 0.2516              | 0.1734         | 0.1048         | 0.1270              | 0.1876         | 0.1153         | 0.2063              | 0.2880         | 0.1729         |
| bench.stress.Stress.map                 | 1.5189              | 2.6075         | 1.1103         | 5.5830              | 10.8417        | 1.3532         | 1.4930              | 2.4210         | 1.3603         |
| bench.stress.Stress.bigString           | 1.6847              | 0.3004         | 2.0657         | 1.0827              | 1.0988         | 1.5081         | 0.6312              | 0.6511         | 0.5438         |
| bench.stress.Stress.bigItemList         | 9.0285              | 8.6335         | 5.1988         | 7.0198              | 11.4344        | 7.4247         | 10.0949             | 15.2837        | 8.9618         |
| bench.stress.Stress.bigSamples          | 4.0710              | 40.9791        | 3.7149         | 3.6887              | 5.6303         | 0.0280         | 6.4027              | 9.5319         | 5.7387         |
| bench.stress.Stress.bigMap              | 10.2634             | 77.0726        | 7.9824         | 270.7320            | 533.8448       | 9.9765         | 10.8479             | 17.1260        | 9.0665         |
| bench.stress.Stress.deepNested          | 6.3238              | 5.2439         | 2.8510         | 4.0202              | 6.2277         | 3.9375         | 6.1517              | 8.9463         | 5.3577         |
| bench.stress.Stress.variant             | 0.2426              | 0.1273         | 0.1177         | 0.1302              | 0.1523         | 0.1000         | 0.1642              | 0.2152         | 0.1092         |
| bench.stress.Stress.deepSparse          | 0.3552              | 0.2042         | 0.2080         | 0.3059              | 0.1572         | 0.3572         | 0.4866              | 0.4909         | 0.2745         |
| bench.stress.Stress.deepFull            | 0.9696              | 1.0275         | 0.6921         | 0.9312              | 1.3409         | 0.9396         | 1.5113              | 2.1220         | 1.2994         |

##### Ratios per operation

Time of each operation of each encoding divided by the time of the same
operation in podbuf: above 1.00 the encoding is slower than podbuf in that
operation, below 1.00 faster. A refused or not-built cell has no ratio and is
marked. A time under about 10 ns includes harness cost and is an upper bound, so
a ratio whose podbuf time is under 10 ns is not comparable.

| Payload                                 | Encode: FlatBuffers / podbuf | Encode: proto3 / podbuf | Verify: FlatBuffers / podbuf | Verify: proto3 / podbuf | Decode: FlatBuffers / podbuf | Decode: proto3 / podbuf |
| --------------------------------------- | ---------------------------- | ----------------------- | ---------------------------- | ----------------------- | ---------------------------- | ----------------------- |
| veh.cluster.VehicleStatus.currentSpeed  | 2.42                         | 1.03                    | 1.12                         | 1.20                    | 2.64                         | 4.14                    |
| veh.cluster.VehicleStatus.engineTemp    | 2.42                         | 1.01                    | 1.12                         | 1.21                    | 2.64                         | 4.14                    |
| veh.cluster.VehicleStatus.warnings      | 2.47                         | 1.19                    | 1.89                         | 3.04                    | 4.33                         | 8.47                    |
| veh.cluster.WheelHistory.wheelTicks     | 2.43                         | 1.16                    | 1.89                         | 3.00                    | 4.92                         | 9.62                    |
| veh.cluster.WheelHistory.wheelTemp      | 2.42                         | 1.01                    | 1.12                         | 1.21                    | 2.64                         | 4.14                    |
| veh.cluster.WheelHistory.wheelSlipRate  | 2.42                         | 1.01                    | 1.12                         | 1.20                    | 2.64                         | 4.14                    |
| veh.cluster.WheelSummary.wheelHealth    | 2.43                         | 1.16                    | 1.89                         | 3.00                    | 4.92                         | 9.62                    |
| veh.cluster.wheels.ticks                | 2.43                         | 1.16                    | 1.89                         | 3.00                    | 4.93                         | 9.62                    |
| veh.cluster.WheelDiagnostics.wheelTicks | not built                    | not built               | not built                    | not built               | not built                    | not built               |
| veh.hvac.cabin.cabinTemp                | 2.41                         | 1.01                    | 1.12                         | 1.21                    | 2.64                         | 4.14                    |
| veh.hvac.cabin.targetTemp               | 2.42                         | 1.01                    | 1.12                         | 1.21                    | 2.64                         | 4.14                    |
| veh.hvac.cabin.blowerDuty               | 2.41                         | 1.01                    | 1.12                         | 1.20                    | 2.64                         | 4.14                    |
| veh.hvac.cabin.fanLevel                 | 2.43                         | 1.16                    | 1.89                         | 3.00                    | 4.92                         | 9.62                    |
| veh.hvac.cabin.compressorLoad           | 2.41                         | 1.01                    | 1.12                         | 1.20                    | 2.64                         | 4.14                    |
| veh.cabin.Cabin.temperature             | 2.47                         | 1.20                    | 1.87                         | 3.02                    | 4.92                         | 10.08                   |
| veh.cabin.Horn.active                   | 2.68                         | 1.07                    | 1.98                         | 3.01                    | 2.49                         | 4.52                    |
| bench.stress.Stress.longString          | 0.93                         | 0.23                    | 0.88                         | 0.93                    | 1.20                         | 1.33                    |
| bench.stress.Stress.itemList            | 1.87                         | 1.66                    | 1.04                         | 1.76                    | 1.17                         | 1.84                    |
| bench.stress.Stress.nestedOptional      | 2.40                         | 1.65                    | 1.10                         | 1.63                    | 1.19                         | 1.67                    |
| bench.stress.Stress.map                 | 1.37                         | 2.35                    | 4.13                         | 8.01                    | 1.10                         | 1.78                    |
| bench.stress.Stress.bigString           | 0.82                         | 0.15                    | 0.72                         | 0.73                    | 1.16                         | 1.20                    |
| bench.stress.Stress.bigItemList         | 1.74                         | 1.66                    | 0.95                         | 1.54                    | 1.13                         | 1.71                    |
| bench.stress.Stress.bigSamples          | 1.10                         | 11.03                   | 131.96                       | 201.41                  | 1.12                         | 1.66                    |
| bench.stress.Stress.bigMap              | 1.29                         | 9.66                    | 27.14                        | 53.51                   | 1.20                         | 1.89                    |
| bench.stress.Stress.deepNested          | 2.22                         | 1.84                    | 1.02                         | 1.58                    | 1.15                         | 1.67                    |
| bench.stress.Stress.variant             | 2.06                         | 1.08                    | 1.30                         | 1.52                    | 1.50                         | 1.97                    |
| bench.stress.Stress.deepSparse          | 1.71                         | 0.98                    | 0.86                         | 0.44                    | 1.77                         | 1.79                    |
| bench.stress.Stress.deepFull            | 1.40                         | 1.48                    | 0.99                         | 1.43                    | 1.16                         | 1.63                    |

##### CPU load, paced and derived

CPU load in percent of one core at 4000 updates per second: paced (process CPU
time over wall time, includes the cost of sleep and wake-up) next to the derived
figure, and their difference.

| Payload                                | Encoding    | Paced % | Derived % | Difference |
| -------------------------------------- | ----------- | ------- | --------- | ---------- |
| veh.cluster.VehicleStatus.currentSpeed | FlatBuffers | 10.1302 | 0.1766    | 9.9535     |
| veh.cluster.VehicleStatus.currentSpeed | proto3      | 10.0209 | 0.1508    | 9.8701     |
| veh.cluster.VehicleStatus.currentSpeed | podbuf      | 10.3223 | 0.1145    | 10.2078    |
| veh.cluster.VehicleStatus.engineTemp   | FlatBuffers | 9.9742  | 0.1799    | 9.7942     |
| veh.cluster.VehicleStatus.engineTemp   | proto3      | 9.7597  | 0.1545    | 9.6052     |
| veh.cluster.VehicleStatus.engineTemp   | podbuf      | 10.4119 | 0.1176    | 10.2942    |
| veh.cluster.VehicleStatus.warnings     | FlatBuffers | 10.6350 | 0.1039    | 10.5311    |
| veh.cluster.VehicleStatus.warnings     | proto3      | 10.7401 | 0.0931    | 10.6470    |
| veh.cluster.VehicleStatus.warnings     | podbuf      | 10.2501 | 0.0420    | 10.2081    |
| veh.cluster.WheelHistory.wheelTicks    | FlatBuffers | 9.5643  | 0.1045    | 9.4598     |
| veh.cluster.WheelHistory.wheelTicks    | proto3      | 9.7998  | 0.0923    | 9.7075     |
| veh.cluster.WheelHistory.wheelTicks    | podbuf      | 9.8540  | 0.0423    | 9.8117     |
| veh.cluster.WheelHistory.wheelTemp     | FlatBuffers | 10.8636 | 0.1798    | 10.6838    |
| veh.cluster.WheelHistory.wheelTemp     | proto3      | 11.0381 | 0.1545    | 10.8836    |
| veh.cluster.WheelHistory.wheelTemp     | podbuf      | 9.5011  | 0.1176    | 9.3835     |
| veh.cluster.WheelHistory.wheelSlipRate | FlatBuffers | 10.0426 | 0.1807    | 9.8619     |
| veh.cluster.WheelHistory.wheelSlipRate | proto3      | 10.7779 | 0.1544    | 10.6235    |
| veh.cluster.WheelHistory.wheelSlipRate | podbuf      | 10.2457 | 0.1183    | 10.1274    |
| veh.cluster.WheelSummary.wheelHealth   | FlatBuffers | 9.0357  | 0.1045    | 8.9311     |
| veh.cluster.WheelSummary.wheelHealth   | proto3      | 9.2599  | 0.0923    | 9.1676     |
| veh.cluster.WheelSummary.wheelHealth   | podbuf      | 8.8938  | 0.0423    | 8.8515     |
| veh.cluster.wheels.ticks               | FlatBuffers | 10.0353 | 0.1045    | 9.9308     |
| veh.cluster.wheels.ticks               | proto3      | 9.8397  | 0.0923    | 9.7475     |
| veh.cluster.wheels.ticks               | podbuf      | 9.3654  | 0.0423    | 9.3232     |
| veh.hvac.cabin.cabinTemp               | FlatBuffers | 10.3066 | 0.1799    | 10.1267    |
| veh.hvac.cabin.cabinTemp               | proto3      | 10.1798 | 0.1545    | 10.0253    |
| veh.hvac.cabin.cabinTemp               | podbuf      | 9.5523  | 0.1177    | 9.4346     |
| veh.hvac.cabin.targetTemp              | FlatBuffers | 11.5923 | 0.1799    | 11.4124    |
| veh.hvac.cabin.targetTemp              | proto3      | 11.6098 | 0.1545    | 11.4553    |
| veh.hvac.cabin.targetTemp              | podbuf      | 10.6491 | 0.1176    | 10.5315    |
| veh.hvac.cabin.blowerDuty              | FlatBuffers | 9.7570  | 0.1807    | 9.5763     |
| veh.hvac.cabin.blowerDuty              | proto3      | 9.7749  | 0.1544    | 9.6205     |
| veh.hvac.cabin.blowerDuty              | podbuf      | 9.5958  | 0.1183    | 9.4774     |
| veh.hvac.cabin.fanLevel                | FlatBuffers | 9.4500  | 0.1046    | 9.3455     |
| veh.hvac.cabin.fanLevel                | proto3      | 9.3779  | 0.0923    | 9.2856     |
| veh.hvac.cabin.fanLevel                | podbuf      | 8.6035  | 0.0424    | 8.5611     |
| veh.hvac.cabin.compressorLoad          | FlatBuffers | 11.6476 | 0.1807    | 11.4668    |
| veh.hvac.cabin.compressorLoad          | proto3      | 11.6080 | 0.1544    | 11.4536    |
| veh.hvac.cabin.compressorLoad          | podbuf      | 10.2380 | 0.1183    | 10.1197    |
| veh.cabin.Cabin.temperature            | FlatBuffers | 9.6481  | 0.1044    | 9.5437     |
| veh.cabin.Cabin.temperature            | proto3      | 9.9673  | 0.0950    | 9.8723     |
| veh.cabin.Cabin.temperature            | podbuf      | 9.6728  | 0.0419    | 9.6309     |
| veh.cabin.Horn.active                  | FlatBuffers | 9.7222  | 0.1126    | 9.6096     |
| veh.cabin.Horn.active                  | proto3      | 10.2431 | 0.0902    | 10.1529    |
| veh.cabin.Horn.active                  | podbuf      | 9.6051  | 0.0451    | 9.5600     |
| bench.stress.Stress.longString         | FlatBuffers | 15.4300 | 0.8236    | 14.6063    |
| bench.stress.Stress.longString         | proto3      | 14.4365 | 0.5832    | 13.8532    |
| bench.stress.Stress.longString         | podbuf      | 16.0990 | 0.8571    | 15.2419    |
| bench.stress.Stress.itemList           | FlatBuffers | 25.8069 | 4.1369    | 21.6699    |
| bench.stress.Stress.itemList           | proto3      | 29.7525 | 5.5964    | 24.1560    |
| bench.stress.Stress.itemList           | podbuf      | 23.8875 | 3.1583    | 20.7292    |
| bench.stress.Stress.nestedOptional     | FlatBuffers | 12.4347 | 0.5849    | 11.8498    |
| bench.stress.Stress.nestedOptional     | proto3      | 12.1679 | 0.6490    | 11.5189    |
| bench.stress.Stress.nestedOptional     | podbuf      | 11.7792 | 0.3931    | 11.3861    |
| bench.stress.Stress.map                | FlatBuffers | 39.7410 | 8.5949    | 31.1462    |
| bench.stress.Stress.map                | proto3      | 62.1099 | 15.8701   | 46.2397    |
| bench.stress.Stress.map                | podbuf      | 25.1021 | 3.8239    | 21.2782    |
| bench.stress.Stress.bigString          | FlatBuffers | 36.9535 | 3.3986    | 33.5549    |
| bench.stress.Stress.bigString          | proto3      | 33.9050 | 2.0503    | 31.8547    |
| bench.stress.Stress.bigString          | podbuf      | 40.9185 | 4.1176    | 36.8010    |
| bench.stress.Stress.bigItemList        | FlatBuffers | 49.0258 | 26.1433   | 22.8826    |
| bench.stress.Stress.bigItemList        | proto3      | 56.7203 | 35.3515   | 21.3688    |
| bench.stress.Stress.bigItemList        | podbuf      | 59.4630 | 21.5853   | 37.8777    |
| bench.stress.Stress.bigSamples         | FlatBuffers | 54.7517 | 14.1624   | 40.5893    |
| bench.stress.Stress.bigSamples         | proto3      | 71.9430 | 56.1413   | 15.8017    |
| bench.stress.Stress.bigSamples         | podbuf      | 44.1358 | 9.4815    | 34.6543    |
| bench.stress.Stress.bigMap             | FlatBuffers | 98.3555 | 291.8434  | -193.4879  |
| bench.stress.Stress.bigMap             | proto3      | 98.7357 | 628.0434  | -529.3077  |
| bench.stress.Stress.bigMap             | podbuf      | 47.5137 | 27.0253   | 20.4884    |
| bench.stress.Stress.deepNested         | FlatBuffers | 49.6305 | 16.4958   | 33.1347    |
| bench.stress.Stress.deepNested         | proto3      | 55.6321 | 20.4180   | 35.2141    |
| bench.stress.Stress.deepNested         | podbuf      | 55.5682 | 12.1463   | 43.4219    |
| bench.stress.Stress.variant            | FlatBuffers | 12.9105 | 0.5370    | 12.3735    |
| bench.stress.Stress.variant            | proto3      | 12.9799 | 0.4948    | 12.4851    |
| bench.stress.Stress.variant            | podbuf      | 11.3852 | 0.3269    | 11.0583    |
| bench.stress.Stress.deepSparse         | FlatBuffers | 21.0477 | 1.1477    | 19.9000    |
| bench.stress.Stress.deepSparse         | proto3      | 17.9107 | 0.8524    | 17.0584    |
| bench.stress.Stress.deepSparse         | podbuf      | 17.6467 | 0.8397    | 16.8071    |
| bench.stress.Stress.deepFull           | FlatBuffers | 26.5293 | 3.4121    | 23.1172    |
| bench.stress.Stress.deepFull           | proto3      | 29.0340 | 4.4904    | 24.5436    |
| bench.stress.Stress.deepFull           | podbuf      | 27.5735 | 2.9311    | 24.6423    |

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
| Time per update (median ns)                            | 128.8       | 105.9  | 61.9   | 2.08                 | 1.71            | 15    |
| CPU % of one core at 4000 updates per second (derived) | 0.0515      | 0.0424 | 0.0248 | 2.08                 | 1.71            | 15    |

Equal-weight mix, per payload type: each payload type counts once, however many
signals share it (the value of a type is the mean over its signals). Only
payload types whose signals are measured in all three encodings are counted; the
last column gives how many.

| Metric                                                 | FlatBuffers | proto3 | podbuf | FlatBuffers / podbuf | proto3 / podbuf | Units |
| ------------------------------------------------------ | ----------- | ------ | ------ | -------------------- | --------------- | ----- |
| Size (bytes)                                           | 25.3        | 3.4    | 16.0   | 1.58                 | 0.21            | 6     |
| Heap allocations per update (encode + verify + decode) | 0.0         | 0.0    | 0.0    | n/a                  | n/a             | 6     |
| Time per update (median ns)                            | 126.0       | 107.8  | 58.5   | 2.16                 | 1.84            | 6     |
| CPU % of one core at 4000 updates per second (derived) | 0.0504      | 0.0431 | 0.0234 | 2.16                 | 1.84            | 6     |

Equal-weight mix, per signal: each signal counts once, so a payload type that
several signals share counts once for each of them. Only signals whose three
encodings are measured are counted; the last column gives how many. The rows are
per operation. A time under about 10 ns includes harness cost and is an upper
bound, so a ratio whose podbuf time is under 10 ns is not comparable.

| Metric                                                         | FlatBuffers | proto3 | podbuf | FlatBuffers / podbuf | proto3 / podbuf | Units |
| -------------------------------------------------------------- | ----------- | ------ | ------ | -------------------- | --------------- | ----- |
| Encode: time per operation (median ns)                         | 70.4        | 30.5   | 23.5   | 2.99                 | 1.30            | 15    |
| Encode: CPU % of one core at 4000 updates per second (derived) | 0.0282      | 0.0122 | 0.0094 | 2.99                 | 1.30            | 15    |
| Encode: heap allocations                                       | 0.0         | 0.0    | 0.0    | n/a                  | n/a             | 15    |
| Verify: time per operation (median ns)                         | 44.4        | 48.2   | 34.6   | 1.28                 | 1.39            | 15    |
| Verify: CPU % of one core at 4000 updates per second (derived) | 0.0178      | 0.0193 | 0.0138 | 1.28                 | 1.39            | 15    |
| Verify: heap allocations                                       | 0.0         | 0.0    | 0.0    | n/a                  | n/a             | 15    |
| Decode: time per operation (median ns)                         | 14.0        | 27.1   | 3.7    | 3.76                 | 7.27            | 15    |
| Decode: CPU % of one core at 4000 updates per second (derived) | 0.0056      | 0.0109 | 0.0015 | 3.76                 | 7.27            | 15    |
| Decode: heap allocations                                       | 0.0         | 0.0    | 0.0    | n/a                  | n/a             | 15    |

Equal-weight mix, per payload type: each payload type counts once, however many
signals share it (the value of a type is the mean over its signals). Only
payload types whose signals are measured in all three encodings are counted; the
last column gives how many. The rows are per operation. A time under about 10 ns
includes harness cost and is an upper bound, so a ratio whose podbuf time is
under 10 ns is not comparable.

| Metric                                                         | FlatBuffers | proto3 | podbuf | FlatBuffers / podbuf | proto3 / podbuf | Units |
| -------------------------------------------------------------- | ----------- | ------ | ------ | -------------------- | --------------- | ----- |
| Encode: time per operation (median ns)                         | 71.4        | 35.9   | 23.5   | 3.04                 | 1.53            | 6     |
| Encode: CPU % of one core at 4000 updates per second (derived) | 0.0286      | 0.0144 | 0.0094 | 3.04                 | 1.53            | 6     |
| Encode: heap allocations                                       | 0.0         | 0.0    | 0.0    | n/a                  | n/a             | 6     |
| Verify: time per operation (median ns)                         | 40.4        | 44.9   | 31.0   | 1.31                 | 1.45            | 6     |
| Verify: CPU % of one core at 4000 updates per second (derived) | 0.0162      | 0.0180 | 0.0124 | 1.31                 | 1.45            | 6     |
| Verify: heap allocations                                       | 0.0         | 0.0    | 0.0    | n/a                  | n/a             | 6     |
| Decode: time per operation (median ns)                         | 14.2        | 27.0   | 4.0    | 3.57                 | 6.78            | 6     |
| Decode: CPU % of one core at 4000 updates per second (derived) | 0.0057      | 0.0108 | 0.0016 | 3.57                 | 6.78            | 6     |
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
| bench.stress.Stress.bigString           | 3032        | 3003      | 3017      |
| bench.stress.Stress.bigItemList         | 4024        | 1542      | 2516      |
| bench.stress.Stress.bigSamples          | 3024        | 4089      | 3016      |
| bench.stress.Stress.bigMap              | 4344        | 1676      | 2896      |
| bench.stress.Stress.deepNested          | 2264        | 760       | 1372      |
| bench.stress.Stress.variant             | 88          | 16        | 57        |
| bench.stress.Stress.deepSparse          | 240         | 13        | 182       |
| bench.stress.Stress.deepFull            | 408         | 178       | 296       |

##### Heap allocations

Heap allocations of one encode / verify / decode.

| Payload                                 | FlatBuffers | proto3      | podbuf      |
| --------------------------------------- | ----------- | ----------- | ----------- |
| veh.cluster.VehicleStatus.currentSpeed  | 0 / 0 / 0   | 0 / 0 / 0   | 0 / 0 / 0   |
| veh.cluster.VehicleStatus.engineTemp    | 0 / 0 / 0   | 0 / 0 / 0   | 0 / 0 / 0   |
| veh.cluster.VehicleStatus.warnings      | 0 / 0 / 0   | 0 / 0 / 0   | 0 / 0 / 0   |
| veh.cluster.WheelHistory.wheelTicks     | 0 / 0 / 0   | 0 / 0 / 0   | 0 / 0 / 0   |
| veh.cluster.WheelHistory.wheelTemp      | 0 / 0 / 0   | 0 / 0 / 0   | 0 / 0 / 0   |
| veh.cluster.WheelHistory.wheelSlipRate  | 0 / 0 / 0   | 0 / 0 / 0   | 0 / 0 / 0   |
| veh.cluster.WheelSummary.wheelHealth    | 0 / 0 / 0   | 0 / 0 / 0   | 0 / 0 / 0   |
| veh.cluster.wheels.ticks                | 0 / 0 / 0   | 0 / 0 / 0   | 0 / 0 / 0   |
| veh.cluster.WheelDiagnostics.wheelTicks | not built   | not built   | not built   |
| veh.hvac.cabin.cabinTemp                | 0 / 0 / 0   | 0 / 0 / 0   | 0 / 0 / 0   |
| veh.hvac.cabin.targetTemp               | 0 / 0 / 0   | 0 / 0 / 0   | 0 / 0 / 0   |
| veh.hvac.cabin.blowerDuty               | 0 / 0 / 0   | 0 / 0 / 0   | 0 / 0 / 0   |
| veh.hvac.cabin.fanLevel                 | 0 / 0 / 0   | 0 / 0 / 0   | 0 / 0 / 0   |
| veh.hvac.cabin.compressorLoad           | 0 / 0 / 0   | 0 / 0 / 0   | 0 / 0 / 0   |
| veh.cabin.Cabin.temperature             | 0 / 0 / 0   | 0 / 0 / 0   | 0 / 0 / 0   |
| veh.cabin.Horn.active                   | 0 / 0 / 0   | 0 / 0 / 0   | 0 / 0 / 0   |
| bench.stress.Stress.longString          | 0 / 0 / 1   | 0 / 0 / 1   | 0 / 0 / 1   |
| bench.stress.Stress.itemList            | 1 / 0 / 17  | 0 / 0 / 19  | 0 / 0 / 17  |
| bench.stress.Stress.nestedOptional      | 0 / 0 / 2   | 0 / 0 / 2   | 0 / 0 / 2   |
| bench.stress.Stress.map                 | 1 / 0 / 17  | 0 / 0 / 19  | 1 / 0 / 17  |
| bench.stress.Stress.bigString           | 0 / 0 / 1   | 0 / 0 / 1   | 0 / 0 / 1   |
| bench.stress.Stress.bigItemList         | 1 / 0 / 101 | 0 / 0 / 106 | 0 / 0 / 101 |
| bench.stress.Stress.bigSamples          | 1 / 0 / 1   | 0 / 0 / 10  | 0 / 0 / 1   |
| bench.stress.Stress.bigMap              | 1 / 0 / 121 | 0 / 0 / 126 | 1 / 0 / 121 |
| bench.stress.Stress.deepNested          | 11 / 0 / 61 | 0 / 0 / 63  | 0 / 0 / 61  |
| bench.stress.Stress.variant             | 0 / 0 / 1   | 0 / 0 / 1   | 0 / 0 / 1   |
| bench.stress.Stress.deepSparse          | 0 / 0 / 1   | 0 / 0 / 1   | 0 / 0 / 1   |
| bench.stress.Stress.deepFull            | 0 / 0 / 15  | 0 / 0 / 15  | 0 / 0 / 15  |

##### Time per update

CPU time of one update (encode, verify and decode), in ns: the sum of the
medians of the three operations, with the sum of their fastest and the sum of
their slowest sample (an envelope, not a percentile) and its range as a share of
the sum. † spread above 10 %: the figure is not stable.

| Payload                                 | FlatBuffers                          | proto3                               | podbuf                            |
| --------------------------------------- | ------------------------------------ | ------------------------------------ | --------------------------------- |
| veh.cluster.VehicleStatus.currentSpeed  | 150.0 (149.0 to 151.4; 2 %)          | 127.5 (126.0 to 129.4; 3 %)          | 83.4 (83.1 to 83.8; 1 %)          |
| veh.cluster.VehicleStatus.engineTemp    | 153.1 (151.8 to 154.2; 2 %)          | 120.6 (119.3 to 122.9; 3 %)          | 86.4 (86.0 to 87.2; 1 %)          |
| veh.cluster.VehicleStatus.warnings      | 101.1 (100.2 to 102.7; 2 %)          | 102.0 (99.8 to 102.8; 3 %)           | 33.7 (33.6 to 34.2; 2 %)          |
| veh.cluster.WheelHistory.wheelTicks     | 100.2 (99.8 to 101.1; 1 %)           | 82.6 (82.4 to 83.3; 1 %)             | 33.8 (33.5 to 34.0; 1 %)          |
| veh.cluster.WheelHistory.wheelTemp      | 153.2 (151.3 to 154.0; 2 %)          | 120.8 (120.0 to 123.4; 3 %)          | 86.6 (86.3 to 87.2; 1 %)          |
| veh.cluster.WheelHistory.wheelSlipRate  | 152.2 (151.1 to 153.9; 2 %)          | 119.8 (118.8 to 121.2; 2 %)          | 86.8 (84.6 to 88.3; 4 %)          |
| veh.cluster.WheelSummary.wheelHealth    | 100.2 (99.8 to 100.8; 1 %)           | 82.6 (82.4 to 83.2; 1 %)             | 33.8 (33.5 to 34.1; 2 %)          |
| veh.cluster.wheels.ticks                | 100.2 (99.8 to 100.9; 1 %)           | 82.6 (82.4 to 83.3; 1 %)             | 33.8 (33.4 to 34.0; 2 %)          |
| veh.cluster.WheelDiagnostics.wheelTicks | not built                            | not built                            | not built                         |
| veh.hvac.cabin.cabinTemp                | 153.3 (152.1 to 153.9; 1 %)          | 120.9 (119.8 to 123.9; 3 %)          | 86.6 (86.2 to 87.5; 1 %)          |
| veh.hvac.cabin.targetTemp               | 152.8 (152.0 to 155.1; 2 %)          | 120.7 (119.8 to 122.5; 2 %)          | 86.7 (86.3 to 87.4; 1 %)          |
| veh.hvac.cabin.blowerDuty               | 152.2 (151.5 to 153.4; 1 %)          | 120.0 (117.9 to 121.5; 3 %)          | 85.8 (84.6 to 88.1; 4 %)          |
| veh.hvac.cabin.fanLevel                 | 100.2 (99.8 to 101.1; 1 %)           | 82.6 (82.4 to 83.3; 1 %)             | 33.8 (33.5 to 34.2; 2 %)          |
| veh.hvac.cabin.compressorLoad           | 152.2 (151.3 to 153.0; 1 %)          | 119.8 (119.0 to 121.2; 2 %)          | 85.3 (84.5 to 88.3; 4 %)          |
| veh.cabin.Cabin.temperature             | 101.4 (100.6 to 102.7; 2 %)          | 84.6 (83.9 to 85.2; 2 %)             | 33.8 (33.6 to 34.2; 2 %)          |
| veh.cabin.Horn.active                   | 109.9 (108.8 to 110.9; 2 %)          | 101.2 (100.3 to 102.4; 2 %)          | 37.9 (36.9 to 38.0; 3 %)          |
| bench.stress.Stress.longString          | 717.6 (713.1 to 726.2; 2 %)          | 436.4 (434.0 to 439.9; 1 %)          | 774.5 (771.6 to 777.1; 1 %)       |
| bench.stress.Stress.itemList            | 3451.6 (3444.6 to 3461.7; 0 %)       | 4554.6 (4525.0 to 4578.0; 1 %)       | 2627.0 (2623.9 to 2632.1; 0 %)    |
| bench.stress.Stress.nestedOptional      | 457.4 (455.1 to 460.1; 1 %)          | 478.6 (477.5 to 480.3; 1 %)          | 301.9 (300.6 to 302.5; 1 %)       |
| bench.stress.Stress.map                 | 7897.5 (7878.3 to 7915.9; 0 %)       | 12383.5 (12228.6 to 12453.9; 2 %)    | 3154.5 (3149.6 to 3164.5; 0 %)    |
| bench.stress.Stress.bigString           | 3110.5 (3101.5 to 3156.0; 2 %)       | 1737.9 (1733.0 to 1744.9; 1 %)       | 3835.1 (3815.1 to 3849.4; 1 %)    |
| bench.stress.Stress.bigItemList         | 21075.7 (21039.5 to 21217.9; 1 %)    | 27943.5 (27855.9 to 28091.2; 1 %)    | 16647.0 (16619.2 to 16705.0; 1 %) |
| bench.stress.Stress.bigSamples          | 12482.5 (12478.8 to 12761.4; 2 %)    | 41329.0 (40934.6 to 41513.3; 1 %)    | 9216.4 (9211.9 to 9261.9; 1 %)    |
| bench.stress.Stress.bigMap              | 266427.4 (266218.8 to 267594.8; 1 %) | 462071.7 (460843.0 to 463221.2; 1 %) | 23028.8 (23002.3 to 23061.7; 0 %) |
| bench.stress.Stress.deepNested          | 13048.9 (13025.6 to 13070.8; 0 %)    | 16380.7 (16310.5 to 16499.3; 1 %)    | 9454.3 (9440.0 to 9473.0; 0 %)    |
| bench.stress.Stress.variant             | 428.3 (426.3 to 430.0; 1 %)          | 367.5 (365.3 to 372.0; 2 %)          | 248.8 (248.1 to 250.6; 1 %)       |
| bench.stress.Stress.deepSparse          | 907.5 (903.2 to 910.8; 1 %)          | 592.4 (590.5 to 594.9; 1 %)          | 662.0 (660.7 to 663.6; 0 %)       |
| bench.stress.Stress.deepFull            | 2867.6 (2862.0 to 2873.1; 0 %)       | 3413.3 (3403.4 to 3444.1; 1 %)       | 2446.2 (2443.4 to 2450.7; 0 %)    |

##### CPU load, derived

CPU load in percent of one core at 4000 updates per second, derived from the
median time of one update (the headline figure).

| Payload                                 | FlatBuffers | proto3    | podbuf    |
| --------------------------------------- | ----------- | --------- | --------- |
| veh.cluster.VehicleStatus.currentSpeed  | 0.0600      | 0.0510    | 0.0333    |
| veh.cluster.VehicleStatus.engineTemp    | 0.0612      | 0.0482    | 0.0346    |
| veh.cluster.VehicleStatus.warnings      | 0.0404      | 0.0408    | 0.0135    |
| veh.cluster.WheelHistory.wheelTicks     | 0.0401      | 0.0330    | 0.0135    |
| veh.cluster.WheelHistory.wheelTemp      | 0.0613      | 0.0483    | 0.0346    |
| veh.cluster.WheelHistory.wheelSlipRate  | 0.0609      | 0.0479    | 0.0347    |
| veh.cluster.WheelSummary.wheelHealth    | 0.0401      | 0.0330    | 0.0135    |
| veh.cluster.wheels.ticks                | 0.0401      | 0.0331    | 0.0135    |
| veh.cluster.WheelDiagnostics.wheelTicks | not built   | not built | not built |
| veh.hvac.cabin.cabinTemp                | 0.0613      | 0.0483    | 0.0346    |
| veh.hvac.cabin.targetTemp               | 0.0611      | 0.0483    | 0.0347    |
| veh.hvac.cabin.blowerDuty               | 0.0609      | 0.0480    | 0.0343    |
| veh.hvac.cabin.fanLevel                 | 0.0401      | 0.0331    | 0.0135    |
| veh.hvac.cabin.compressorLoad           | 0.0609      | 0.0479    | 0.0341    |
| veh.cabin.Cabin.temperature             | 0.0406      | 0.0338    | 0.0135    |
| veh.cabin.Horn.active                   | 0.0440      | 0.0405    | 0.0152    |
| bench.stress.Stress.longString          | 0.2870      | 0.1746    | 0.3098    |
| bench.stress.Stress.itemList            | 1.3806      | 1.8218    | 1.0508    |
| bench.stress.Stress.nestedOptional      | 0.1830      | 0.1914    | 0.1208    |
| bench.stress.Stress.map                 | 3.1590      | 4.9534    | 1.2618    |
| bench.stress.Stress.bigString           | 1.2442      | 0.6952    | 1.5341    |
| bench.stress.Stress.bigItemList         | 8.4303      | 11.1774   | 6.6588    |
| bench.stress.Stress.bigSamples          | 4.9930      | 16.5316   | 3.6866    |
| bench.stress.Stress.bigMap              | 106.5710    | 184.8287  | 9.2115    |
| bench.stress.Stress.deepNested          | 5.2196      | 6.5523    | 3.7817    |
| bench.stress.Stress.variant             | 0.1713      | 0.1470    | 0.0995    |
| bench.stress.Stress.deepSparse          | 0.3630      | 0.2370    | 0.2648    |
| bench.stress.Stress.deepFull            | 1.1470      | 1.3653    | 0.9785    |

##### Ratios to podbuf

Each encoding divided by podbuf: above 1.00 the encoding is larger or slower
than podbuf, below 1.00 smaller or faster. The CPU ratios equal the time ratios,
because the derived CPU load is proportional to the median time. A refused or
not-built cell has no ratio and is marked.

| Payload                                 | Size: FlatBuffers / podbuf | Size: proto3 / podbuf | Time: FlatBuffers / podbuf | Time: proto3 / podbuf | CPU: FlatBuffers / podbuf | CPU: proto3 / podbuf |
| --------------------------------------- | -------------------------- | --------------------- | -------------------------- | --------------------- | ------------------------- | -------------------- |
| veh.cluster.VehicleStatus.currentSpeed  | 1.50                       | 0.31                  | 1.80                       | 1.53                  | 1.80                      | 1.53                 |
| veh.cluster.VehicleStatus.engineTemp    | 1.50                       | 0.31                  | 1.77                       | 1.40                  | 1.77                      | 1.40                 |
| veh.cluster.VehicleStatus.warnings      | 1.50                       | 0.12                  | 3.00                       | 3.02                  | 3.00                      | 3.02                 |
| veh.cluster.WheelHistory.wheelTicks     | 1.50                       | 0.12                  | 2.97                       | 2.45                  | 2.97                      | 2.45                 |
| veh.cluster.WheelHistory.wheelTemp      | 1.50                       | 0.31                  | 1.77                       | 1.39                  | 1.77                      | 1.39                 |
| veh.cluster.WheelHistory.wheelSlipRate  | 1.50                       | 0.31                  | 1.75                       | 1.38                  | 1.75                      | 1.38                 |
| veh.cluster.WheelSummary.wheelHealth    | 1.50                       | 0.12                  | 2.97                       | 2.44                  | 2.97                      | 2.44                 |
| veh.cluster.wheels.ticks                | 1.50                       | 0.12                  | 2.97                       | 2.44                  | 2.97                      | 2.44                 |
| veh.cluster.WheelDiagnostics.wheelTicks | not built                  | not built             | not built                  | not built             | not built                 | not built            |
| veh.hvac.cabin.cabinTemp                | 1.50                       | 0.31                  | 1.77                       | 1.40                  | 1.77                      | 1.40                 |
| veh.hvac.cabin.targetTemp               | 1.50                       | 0.31                  | 1.76                       | 1.39                  | 1.76                      | 1.39                 |
| veh.hvac.cabin.blowerDuty               | 1.50                       | 0.31                  | 1.77                       | 1.40                  | 1.77                      | 1.40                 |
| veh.hvac.cabin.fanLevel                 | 1.50                       | 0.12                  | 2.97                       | 2.45                  | 2.97                      | 2.45                 |
| veh.hvac.cabin.compressorLoad           | 1.50                       | 0.31                  | 1.79                       | 1.41                  | 1.79                      | 1.41                 |
| veh.cabin.Cabin.temperature             | 1.50                       | 0.12                  | 3.00                       | 2.50                  | 3.00                      | 2.50                 |
| veh.cabin.Horn.active                   | 2.00                       | 0.12                  | 2.90                       | 2.67                  | 2.90                      | 2.67                 |
| bench.stress.Stress.longString          | 1.03                       | 0.97                  | 0.93                       | 0.56                  | 0.93                      | 0.56                 |
| bench.stress.Stress.itemList            | 1.50                       | 0.55                  | 1.31                       | 1.73                  | 1.31                      | 1.73                 |
| bench.stress.Stress.nestedOptional      | 1.63                       | 0.35                  | 1.51                       | 1.59                  | 1.51                      | 1.59                 |
| bench.stress.Stress.map                 | 1.56                       | 0.53                  | 2.50                       | 3.93                  | 2.50                      | 3.93                 |
| bench.stress.Stress.bigString           | 1.00                       | 1.00                  | 0.81                       | 0.45                  | 0.81                      | 0.45                 |
| bench.stress.Stress.bigItemList         | 1.60                       | 0.61                  | 1.27                       | 1.68                  | 1.27                      | 1.68                 |
| bench.stress.Stress.bigSamples          | 1.00                       | 1.36                  | 1.35                       | 4.48                  | 1.35                      | 4.48                 |
| bench.stress.Stress.bigMap              | 1.50                       | 0.58                  | 11.57                      | 20.06                 | 11.57                     | 20.06                |
| bench.stress.Stress.deepNested          | 1.65                       | 0.55                  | 1.38                       | 1.73                  | 1.38                      | 1.73                 |
| bench.stress.Stress.variant             | 1.54                       | 0.28                  | 1.72                       | 1.48                  | 1.72                      | 1.48                 |
| bench.stress.Stress.deepSparse          | 1.32                       | 0.07                  | 1.37                       | 0.89                  | 1.37                      | 0.89                 |
| bench.stress.Stress.deepFull            | 1.38                       | 0.60                  | 1.17                       | 1.40                  | 1.17                      | 1.40                 |

##### Time per operation, encode

CPU time of the encode operation of one update, in ns: the median of 30 samples
of a batch of this operation alone, with the fastest and the slowest sample and
the range as a share of the median. † spread above 10 %: the figure is not
stable.

| Payload                                 | FlatBuffers                    | proto3                            | podbuf                         |
| --------------------------------------- | ------------------------------ | --------------------------------- | ------------------------------ |
| veh.cluster.VehicleStatus.currentSpeed  | 70.8 (70.4 to 71.2; 1 %)       | 34.1 (33.8 to 35.0; 3 %)          | 23.5 (23.4 to 23.8; 2 %)       |
| veh.cluster.VehicleStatus.engineTemp    | 68.8 (68.2 to 69.3; 2 %)       | 25.4 (25.2 to 25.9; 3 %)          | 23.6 (23.5 to 24.1; 2 %)       |
| veh.cluster.VehicleStatus.warnings      | 71.6 (70.8 to 73.0; 3 %)       | 50.6 (48.8 to 51.1; 5 %)          | 23.5 (23.4 to 23.9; 2 %)       |
| veh.cluster.WheelHistory.wheelTicks     | 70.7 (70.3 to 71.3; 1 %)       | 27.3 (27.2 to 27.7; 2 %)          | 23.4 (23.2 to 23.5; 1 %)       |
| veh.cluster.WheelHistory.wheelTemp      | 69.0 (68.3 to 69.3; 2 %)       | 25.4 (25.2 to 25.8; 2 %)          | 23.6 (23.5 to 24.0; 2 %)       |
| veh.cluster.WheelHistory.wheelSlipRate  | 68.7 (68.2 to 70.4; 3 %)       | 25.4 (25.2 to 25.6; 2 %)          | 23.6 (23.5 to 24.0; 3 %)       |
| veh.cluster.WheelSummary.wheelHealth    | 70.7 (70.3 to 71.0; 1 %)       | 27.3 (27.2 to 27.8; 2 %)          | 23.4 (23.2 to 23.6; 2 %)       |
| veh.cluster.wheels.ticks                | 70.7 (70.3 to 71.1; 1 %)       | 27.3 (27.2 to 27.8; 2 %)          | 23.5 (23.2 to 23.5; 1 %)       |
| veh.cluster.WheelDiagnostics.wheelTicks | not built                      | not built                         | not built                      |
| veh.hvac.cabin.cabinTemp                | 69.0 (68.5 to 69.3; 1 %)       | 25.4 (25.2 to 26.1; 4 %)          | 23.6 (23.4 to 24.2; 3 %)       |
| veh.hvac.cabin.targetTemp               | 68.7 (68.4 to 70.5; 3 %)       | 25.3 (25.2 to 25.7; 2 %)          | 23.6 (23.5 to 24.0; 2 %)       |
| veh.hvac.cabin.blowerDuty               | 68.8 (68.4 to 69.6; 2 %)       | 25.6 (25.1 to 25.8; 3 %)          | 23.6 (23.5 to 23.9; 2 %)       |
| veh.hvac.cabin.fanLevel                 | 70.6 (70.3 to 71.3; 1 %)       | 27.3 (27.2 to 27.8; 2 %)          | 23.5 (23.3 to 23.7; 2 %)       |
| veh.hvac.cabin.compressorLoad           | 68.8 (68.2 to 69.4; 2 %)       | 25.4 (25.2 to 25.7; 2 %)          | 23.6 (23.5 to 24.0; 2 %)       |
| veh.cabin.Cabin.temperature             | 71.7 (71.0 to 72.7; 2 %)       | 35.3 (35.1 to 35.4; 1 %)          | 23.5 (23.4 to 23.8; 2 %)       |
| veh.cabin.Horn.active                   | 77.3 (76.3 to 78.2; 2 %)       | 50.5 (49.9 to 51.4; 3 %)          | 23.5 (22.7 to 23.6; 4 %)       |
| bench.stress.Stress.longString          | 358.0 (355.4 to 364.1; 2 %)    | 59.3 (58.8 to 59.8; 2 %)          | 385.2 (384.4 to 386.0; 0 %)    |
| bench.stress.Stress.itemList            | 1227.0 (1223.5 to 1233.7; 1 %) | 1067.1 (1049.1 to 1073.2; 2 %)    | 669.0 (667.5 to 671.4; 1 %)    |
| bench.stress.Stress.nestedOptional      | 194.1 (192.8 to 195.3; 1 %)    | 135.7 (135.3 to 136.3; 1 %)       | 82.5 (82.3 to 82.7; 0 %)       |
| bench.stress.Stress.map                 | 1239.7 (1237.2 to 1243.8; 1 %) | 2112.8 (2105.6 to 2122.5; 1 %)    | 936.5 (934.0 to 944.0; 1 %)    |
| bench.stress.Stress.bigString           | 1629.5 (1626.0 to 1657.0; 2 %) | 199.9 (199.3 to 201.4; 1 %)       | 1981.7 (1969.5 to 1989.8; 1 %) |
| bench.stress.Stress.bigItemList         | 7199.1 (7188.1 to 7219.1; 0 %) | 6829.2 (6818.9 to 6887.1; 1 %)    | 4101.2 (4098.5 to 4126.1; 1 %) |
| bench.stress.Stress.bigSamples          | 3546.4 (3544.8 to 3548.4; 0 %) | 29543.1 (29493.8 to 29587.1; 0 %) | 4149.1 (4146.8 to 4193.3; 1 %) |
| bench.stress.Stress.bigMap              | 8474.6 (8450.0 to 8503.1; 1 %) | 64122.0 (64096.8 to 64428.2; 1 %) | 7006.8 (7004.4 to 7012.6; 0 %) |
| bench.stress.Stress.deepNested          | 4741.1 (4733.0 to 4747.0; 0 %) | 4142.4 (4127.3 to 4203.5; 2 %)    | 2291.1 (2287.5 to 2294.1; 0 %) |
| bench.stress.Stress.variant             | 195.9 (195.2 to 196.3; 1 %)    | 103.5 (102.7 to 104.4; 2 %)       | 91.3 (90.9 to 92.6; 2 %)       |
| bench.stress.Stress.deepSparse          | 300.8 (299.3 to 301.8; 1 %)    | 149.6 (149.2 to 149.9; 1 %)       | 164.9 (164.7 to 165.2; 0 %)    |
| bench.stress.Stress.deepFull            | 842.5 (838.5 to 846.3; 1 %)    | 750.5 (745.5 to 773.7; 4 %)       | 563.1 (562.5 to 564.7; 0 %)    |

##### Time per operation, verify

CPU time of the verify operation of one update, in ns: the median of 30 samples
of a batch of this operation alone, with the fastest and the slowest sample and
the range as a share of the median. † spread above 10 %: the figure is not
stable.

| Payload                                 | FlatBuffers                          | proto3                               | podbuf                         |
| --------------------------------------- | ------------------------------------ | ------------------------------------ | ------------------------------ |
| veh.cluster.VehicleStatus.currentSpeed  | 64.8 (64.2 to 65.7; 2 %)             | 69.4 (68.3 to 70.4; 3 %)             | 55.1 (55.0 to 55.2; 0 %)       |
| veh.cluster.VehicleStatus.engineTemp    | 69.9 (69.3 to 70.6; 2 %)             | 71.4 (70.4 to 73.1; 4 %)             | 58.1 (57.9 to 58.5; 1 %)       |
| veh.cluster.VehicleStatus.warnings      | 16.2 (16.1 to 16.2; 0 %)             | 23.1 (22.8 to 23.2; 1 %)             | 8.2 (8.2 to 8.3; 1 %)          |
| veh.cluster.WheelHistory.wheelTicks     | 16.2 (16.2 to 16.3; 0 %)             | 22.1 (22.0 to 22.3; 1 %)             | 8.3 (8.3 to 8.4; 1 %)          |
| veh.cluster.WheelHistory.wheelTemp      | 69.9 (68.7 to 70.3; 2 %)             | 71.5 (71.1 to 73.5; 3 %)             | 58.3 (58.1 to 58.5; 1 %)       |
| veh.cluster.WheelHistory.wheelSlipRate  | 69.1 (68.7 to 69.2; 1 %)             | 70.6 (70.0 to 71.6; 2 %)             | 58.5 (56.5 to 59.5; 5 %)       |
| veh.cluster.WheelSummary.wheelHealth    | 16.2 (16.2 to 16.3; 0 %)             | 22.1 (22.0 to 22.2; 1 %)             | 8.3 (8.2 to 8.4; 2 %)          |
| veh.cluster.wheels.ticks                | 16.2 (16.2 to 16.3; 0 %)             | 22.1 (22.0 to 22.2; 1 %)             | 8.3 (8.2 to 8.4; 3 %)          |
| veh.cluster.WheelDiagnostics.wheelTicks | not built                            | not built                            | not built                      |
| veh.hvac.cabin.cabinTemp                | 69.9 (69.3 to 70.2; 1 %)             | 71.5 (70.8 to 73.7; 4 %)             | 58.3 (58.1 to 58.6; 1 %)       |
| veh.hvac.cabin.targetTemp               | 69.8 (69.3 to 70.2; 1 %)             | 71.5 (70.7 to 72.7; 3 %)             | 58.3 (58.1 to 58.7; 1 %)       |
| veh.hvac.cabin.blowerDuty               | 69.1 (68.9 to 69.5; 1 %)             | 70.6 (69.3 to 71.7; 3 %)             | 57.5 (56.4 to 59.5; 5 %)       |
| veh.hvac.cabin.fanLevel                 | 16.2 (16.2 to 16.2; 0 %)             | 22.1 (22.0 to 22.2; 1 %)             | 8.3 (8.2 to 8.4; 2 %)          |
| veh.hvac.cabin.compressorLoad           | 69.1 (68.8 to 69.2; 1 %)             | 70.6 (70.1 to 71.5; 2 %)             | 57.0 (56.4 to 59.6; 6 %)       |
| veh.cabin.Cabin.temperature             | 16.3 (16.3 to 16.4; 1 %)             | 22.4 (22.1 to 22.5; 2 %)             | 8.3 (8.3 to 8.4; 1 %)          |
| veh.cabin.Horn.active                   | 17.2 (17.2 to 17.2; 0 %)             | 22.8 (22.7 to 23.1; 2 %)             | 8.2 (8.2 to 8.3; 1 %)          |
| bench.stress.Stress.longString          | 225.2 (223.6 to 227.1; 2 %)          | 235.3 (234.0 to 237.7; 2 %)          | 273.6 (272.6 to 274.4; 1 %)    |
| bench.stress.Stress.itemList            | 882.4 (880.4 to 884.2; 0 %)          | 1491.4 (1487.2 to 1496.2; 1 %)       | 843.1 (842.8 to 843.4; 0 %)    |
| bench.stress.Stress.nestedOptional      | 107.4 (106.9 to 107.8; 1 %)          | 145.8 (145.3 to 146.5; 1 %)          | 93.8 (93.5 to 94.0; 1 %)       |
| bench.stress.Stress.map                 | 5354.9 (5340.4 to 5368.1; 1 %)       | 8298.7 (8156.9 to 8350.3; 2 %)       | 1140.4 (1139.1 to 1142.0; 0 %) |
| bench.stress.Stress.bigString           | 1048.6 (1046.3 to 1052.7; 1 %)       | 1085.4 (1083.2 to 1088.8; 1 %)       | 1423.3 (1418.9 to 1426.9; 1 %) |
| bench.stress.Stress.bigItemList         | 5611.6 (5592.2 to 5727.2; 2 %)       | 9008.6 (8972.0 to 9037.1; 1 %)       | 5518.6 (5514.3 to 5536.9; 0 %) |
| bench.stress.Stress.bigSamples          | 4113.0 (4112.6 to 4113.6; 0 %)       | 4441.2 (4431.0 to 4442.7; 0 %)       | 22.5 (22.4 to 23.1; 3 %)       |
| bench.stress.Stress.bigMap              | 248564.4 (248391.9 to 249695.0; 1 %) | 384086.2 (382929.2 to 384884.5; 1 %) | 8588.6 (8586.1 to 8590.4; 0 %) |
| bench.stress.Stress.deepNested          | 3267.3 (3255.8 to 3274.8; 1 %)       | 5113.9 (5082.0 to 5148.9; 1 %)       | 2996.4 (2991.3 to 3001.2; 0 %) |
| bench.stress.Stress.variant             | 104.6 (103.8 to 105.5; 2 %)          | 108.0 (107.1 to 110.3; 3 %)          | 77.1 (76.8 to 77.5; 1 %)       |
| bench.stress.Stress.deepSparse          | 248.0 (247.0 to 248.8; 1 %)          | 110.3 (109.6 to 111.4; 2 %)          | 312.4 (312.1 to 312.6; 0 %)    |
| bench.stress.Stress.deepFull            | 821.7 (821.0 to 822.6; 0 %)          | 1117.3 (1114.0 to 1123.7; 1 %)       | 863.7 (862.7 to 864.6; 0 %)    |

##### Time per operation, decode

CPU time of the decode operation of one update, in ns: the median of 30 samples
of a batch of this operation alone, with the fastest and the slowest sample and
the range as a share of the median. † spread above 10 %: the figure is not
stable.

| Payload                                 | FlatBuffers                    | proto3                            | podbuf                         |
| --------------------------------------- | ------------------------------ | --------------------------------- | ------------------------------ |
| veh.cluster.VehicleStatus.currentSpeed  | 14.4 (14.4 to 14.4; 0 %)       | 24.0 (23.9 to 24.1; 1 %)          | 4.8 (4.7 to 4.8; 1 %)          |
| veh.cluster.VehicleStatus.engineTemp    | 14.4 (14.3 to 14.4; 0 %)       | 23.8 (23.7 to 23.9; 1 %)          | 4.7 (4.7 to 4.7; 1 %)          |
| veh.cluster.VehicleStatus.warnings      | 13.4 (13.3 to 13.5; 1 %)       | 28.3 (28.2 to 28.6; 1 %)          | 2.0 (2.0 to 2.0; 4 %)          |
| veh.cluster.WheelHistory.wheelTicks     | 13.3 (13.3 to 13.5; 2 %)       | 33.2 (33.2 to 33.3; 0 %)          | 2.0 (2.0 to 2.0; 0 %)          |
| veh.cluster.WheelHistory.wheelTemp      | 14.4 (14.3 to 14.4; 0 %)       | 23.9 (23.7 to 24.1; 2 %)          | 4.7 (4.7 to 4.7; 1 %)          |
| veh.cluster.WheelHistory.wheelSlipRate  | 14.4 (14.3 to 14.4; 0 %)       | 23.9 (23.7 to 24.0; 2 %)          | 4.7 (4.7 to 4.7; 1 %)          |
| veh.cluster.WheelSummary.wheelHealth    | 13.3 (13.3 to 13.5; 1 %)       | 33.2 (33.2 to 33.3; 0 %)          | 2.0 (2.0 to 2.0; 0 %)          |
| veh.cluster.wheels.ticks                | 13.3 (13.3 to 13.5; 2 %)       | 33.3 (33.2 to 33.3; 0 %)          | 2.0 (2.0 to 2.0; 0 %)          |
| veh.cluster.WheelDiagnostics.wheelTicks | not built                      | not built                         | not built                      |
| veh.hvac.cabin.cabinTemp                | 14.4 (14.3 to 14.4; 0 %)       | 23.9 (23.8 to 24.1; 1 %)          | 4.7 (4.7 to 4.7; 1 %)          |
| veh.hvac.cabin.targetTemp               | 14.4 (14.3 to 14.4; 0 %)       | 23.9 (23.8 to 24.1; 1 %)          | 4.7 (4.7 to 4.7; 1 %)          |
| veh.hvac.cabin.blowerDuty               | 14.4 (14.3 to 14.4; 0 %)       | 23.9 (23.6 to 24.0; 2 %)          | 4.7 (4.7 to 4.7; 1 %)          |
| veh.hvac.cabin.fanLevel                 | 13.3 (13.3 to 13.5; 2 %)       | 33.3 (33.2 to 33.3; 0 %)          | 2.0 (2.0 to 2.0; 0 %)          |
| veh.hvac.cabin.compressorLoad           | 14.4 (14.3 to 14.4; 0 %)       | 23.9 (23.7 to 24.0; 1 %)          | 4.7 (4.7 to 4.7; 1 %)          |
| veh.cabin.Cabin.temperature             | 13.4 (13.4 to 13.6; 2 %)       | 26.9 (26.7 to 27.2; 2 %)          | 2.0 (2.0 to 2.0; 1 %)          |
| veh.cabin.Horn.active                   | 15.4 (15.3 to 15.4; 1 %)       | 27.9 (27.7 to 27.9; 1 %)          | 6.2 (6.1 to 6.2; 2 %)          |
| bench.stress.Stress.longString          | 134.4 (134.1 to 135.0; 1 %)    | 141.9 (141.1 to 142.3; 1 %)       | 115.7 (114.6 to 116.6; 2 %)    |
| bench.stress.Stress.itemList            | 1342.2 (1340.6 to 1343.8; 0 %) | 1996.1 (1988.7 to 2008.5; 1 %)    | 1114.9 (1113.7 to 1117.3; 0 %) |
| bench.stress.Stress.nestedOptional      | 155.9 (155.4 to 157.0; 1 %)    | 197.1 (196.9 to 197.5; 0 %)       | 125.6 (124.8 to 125.8; 1 %)    |
| bench.stress.Stress.map                 | 1302.9 (1300.8 to 1303.9; 0 %) | 1972.0 (1966.2 to 1981.2; 1 %)    | 1077.6 (1076.5 to 1078.5; 0 %) |
| bench.stress.Stress.bigString           | 432.4 (429.2 to 446.4; 4 %)    | 452.6 (450.6 to 454.7; 1 %)       | 430.1 (426.7 to 432.8; 1 %)    |
| bench.stress.Stress.bigItemList         | 8264.9 (8259.2 to 8271.6; 0 %) | 12105.7 (12064.9 to 12167.1; 1 %) | 7027.2 (7006.5 to 7042.0; 1 %) |
| bench.stress.Stress.bigSamples          | 4823.1 (4821.4 to 5099.5; 6 %) | 7344.7 (7009.7 to 7483.5; 6 %)    | 5044.8 (5042.7 to 5045.5; 0 %) |
| bench.stress.Stress.bigMap              | 9388.4 (9376.9 to 9396.7; 0 %) | 13863.5 (13817.1 to 13908.5; 1 %) | 7433.4 (7411.8 to 7458.7; 1 %) |
| bench.stress.Stress.deepNested          | 5040.6 (5036.8 to 5048.9; 0 %) | 7124.4 (7101.1 to 7146.8; 1 %)    | 4166.8 (4161.3 to 4177.7; 0 %) |
| bench.stress.Stress.variant             | 127.8 (127.3 to 128.2; 1 %)    | 156.0 (155.4 to 157.3; 1 %)       | 80.4 (80.4 to 80.5; 0 %)       |
| bench.stress.Stress.deepSparse          | 358.7 (356.9 to 360.2; 1 %)    | 332.5 (331.7 to 333.5; 1 %)       | 184.7 (183.9 to 185.8; 1 %)    |
| bench.stress.Stress.deepFull            | 1203.4 (1202.5 to 1204.2; 0 %) | 1545.5 (1543.9 to 1546.7; 0 %)    | 1019.4 (1018.2 to 1021.5; 0 %) |

##### Time per operation, access

CPU time of reading one number, deep in the payload, through the view of
verified bytes, in ns: a count, or for the sparse payloads the length of a
string field. The median of 30 samples, with the fastest and the slowest sample
and the range as a share of the median. It stands for the decode when a consumer
needs one field, and it is not part of the time per update. The FlatBuffers
accessor of a list returns the decoded list, so there the access of a list
element costs the decode of the list. proto3 generates no accessors, so its
cells are n/a. † spread above 10 %: the figure is not stable.

| Payload                         | FlatBuffers                    | proto3            | podbuf                   |
| ------------------------------- | ------------------------------ | ----------------- | ------------------------ |
| bench.stress.Stress.bigItemList | 8238.2 (8234.5 to 8251.3; 0 %) | n/a: no accessors | 43.4 (43.2 to 43.7; 1 %) |
| bench.stress.Stress.deepNested  | 5030.5 (5027.1 to 5038.1; 0 %) | n/a: no accessors | 46.8 (46.4 to 47.1; 1 %) |
| bench.stress.Stress.deepSparse  | 61.6 (61.1 to 61.9; 1 %)       | n/a: no accessors | 24.5 (24.0 to 24.6; 2 %) |
| bench.stress.Stress.deepFull    | 64.4 (63.8 to 65.2; 2 %)       | n/a: no accessors | 25.5 (25.3 to 25.8; 2 %) |

##### CPU load per operation

CPU load in percent of one core at 4000 updates per second of each operation
alone, derived from the median time of that operation. The three operations of
an encoding add up to its derived load of one update.

| Payload                                 | Encode: FlatBuffers | Encode: proto3 | Encode: podbuf | Verify: FlatBuffers | Verify: proto3 | Verify: podbuf | Decode: FlatBuffers | Decode: proto3 | Decode: podbuf |
| --------------------------------------- | ------------------- | -------------- | -------------- | ------------------- | -------------- | -------------- | ------------------- | -------------- | -------------- |
| veh.cluster.VehicleStatus.currentSpeed  | 0.0283              | 0.0136         | 0.0094         | 0.0259              | 0.0278         | 0.0221         | 0.0058              | 0.0096         | 0.0019         |
| veh.cluster.VehicleStatus.engineTemp    | 0.0275              | 0.0102         | 0.0095         | 0.0280              | 0.0286         | 0.0232         | 0.0057              | 0.0095         | 0.0019         |
| veh.cluster.VehicleStatus.warnings      | 0.0286              | 0.0202         | 0.0094         | 0.0065              | 0.0092         | 0.0033         | 0.0053              | 0.0113         | 0.0008         |
| veh.cluster.WheelHistory.wheelTicks     | 0.0283              | 0.0109         | 0.0094         | 0.0065              | 0.0088         | 0.0033         | 0.0053              | 0.0133         | 0.0008         |
| veh.cluster.WheelHistory.wheelTemp      | 0.0276              | 0.0101         | 0.0095         | 0.0279              | 0.0286         | 0.0233         | 0.0057              | 0.0096         | 0.0019         |
| veh.cluster.WheelHistory.wheelSlipRate  | 0.0275              | 0.0102         | 0.0094         | 0.0276              | 0.0282         | 0.0234         | 0.0057              | 0.0095         | 0.0019         |
| veh.cluster.WheelSummary.wheelHealth    | 0.0283              | 0.0109         | 0.0094         | 0.0065              | 0.0088         | 0.0033         | 0.0053              | 0.0133         | 0.0008         |
| veh.cluster.wheels.ticks                | 0.0283              | 0.0109         | 0.0094         | 0.0065              | 0.0088         | 0.0033         | 0.0053              | 0.0133         | 0.0008         |
| veh.cluster.WheelDiagnostics.wheelTicks | not built           | not built      | not built      | not built           | not built      | not built      | not built           | not built      | not built      |
| veh.hvac.cabin.cabinTemp                | 0.0276              | 0.0102         | 0.0095         | 0.0280              | 0.0286         | 0.0233         | 0.0057              | 0.0096         | 0.0019         |
| veh.hvac.cabin.targetTemp               | 0.0275              | 0.0101         | 0.0095         | 0.0279              | 0.0286         | 0.0233         | 0.0057              | 0.0096         | 0.0019         |
| veh.hvac.cabin.blowerDuty               | 0.0275              | 0.0102         | 0.0094         | 0.0276              | 0.0282         | 0.0230         | 0.0057              | 0.0096         | 0.0019         |
| veh.hvac.cabin.fanLevel                 | 0.0283              | 0.0109         | 0.0094         | 0.0065              | 0.0089         | 0.0033         | 0.0053              | 0.0133         | 0.0008         |
| veh.hvac.cabin.compressorLoad           | 0.0275              | 0.0101         | 0.0094         | 0.0276              | 0.0282         | 0.0228         | 0.0057              | 0.0096         | 0.0019         |
| veh.cabin.Cabin.temperature             | 0.0287              | 0.0141         | 0.0094         | 0.0065              | 0.0090         | 0.0033         | 0.0054              | 0.0108         | 0.0008         |
| veh.cabin.Horn.active                   | 0.0309              | 0.0202         | 0.0094         | 0.0069              | 0.0091         | 0.0033         | 0.0061              | 0.0111         | 0.0025         |
| bench.stress.Stress.longString          | 0.1432              | 0.0237         | 0.1541         | 0.0901              | 0.0941         | 0.1094         | 0.0538              | 0.0567         | 0.0463         |
| bench.stress.Stress.itemList            | 0.4908              | 0.4268         | 0.2676         | 0.3530              | 0.5965         | 0.3372         | 0.5369              | 0.7984         | 0.4459         |
| bench.stress.Stress.nestedOptional      | 0.0776              | 0.0543         | 0.0330         | 0.0430              | 0.0583         | 0.0375         | 0.0624              | 0.0789         | 0.0502         |
| bench.stress.Stress.map                 | 0.4959              | 0.8451         | 0.3746         | 2.1419              | 3.3195         | 0.4562         | 0.5212              | 0.7888         | 0.4310         |
| bench.stress.Stress.bigString           | 0.6518              | 0.0800         | 0.7927         | 0.4194              | 0.4342         | 0.5693         | 0.1730              | 0.1810         | 0.1720         |
| bench.stress.Stress.bigItemList         | 2.8797              | 2.7317         | 1.6405         | 2.2447              | 3.6034         | 2.2075         | 3.3060              | 4.8423         | 2.8109         |
| bench.stress.Stress.bigSamples          | 1.4186              | 11.8172        | 1.6596         | 1.6452              | 1.7765         | 0.0090         | 1.9292              | 2.9379         | 2.0179         |
| bench.stress.Stress.bigMap              | 3.3898              | 25.6488        | 2.8027         | 99.4258             | 153.6345       | 3.4354         | 3.7554              | 5.5454         | 2.9734         |
| bench.stress.Stress.deepNested          | 1.8964              | 1.6569         | 0.9164         | 1.3069              | 2.0456         | 1.1986         | 2.0162              | 2.8498         | 1.6667         |
| bench.stress.Stress.variant             | 0.0784              | 0.0414         | 0.0365         | 0.0418              | 0.0432         | 0.0308         | 0.0511              | 0.0624         | 0.0322         |
| bench.stress.Stress.deepSparse          | 0.1203              | 0.0599         | 0.0660         | 0.0992              | 0.0441         | 0.1249         | 0.1435              | 0.1330         | 0.0739         |
| bench.stress.Stress.deepFull            | 0.3370              | 0.3002         | 0.2252         | 0.3287              | 0.4469         | 0.3455         | 0.4814              | 0.6182         | 0.4078         |

##### Ratios per operation

Time of each operation of each encoding divided by the time of the same
operation in podbuf: above 1.00 the encoding is slower than podbuf in that
operation, below 1.00 faster. A refused or not-built cell has no ratio and is
marked. A time under about 10 ns includes harness cost and is an upper bound, so
a ratio whose podbuf time is under 10 ns is not comparable.

| Payload                                 | Encode: FlatBuffers / podbuf | Encode: proto3 / podbuf | Verify: FlatBuffers / podbuf | Verify: proto3 / podbuf | Decode: FlatBuffers / podbuf | Decode: proto3 / podbuf |
| --------------------------------------- | ---------------------------- | ----------------------- | ---------------------------- | ----------------------- | ---------------------------- | ----------------------- |
| veh.cluster.VehicleStatus.currentSpeed  | 3.02                         | 1.45                    | 1.18                         | 1.26                    | 3.03                         | 5.04                    |
| veh.cluster.VehicleStatus.engineTemp    | 2.91                         | 1.07                    | 1.20                         | 1.23                    | 3.04                         | 5.05                    |
| veh.cluster.VehicleStatus.warnings      | 3.05                         | 2.16                    | 1.96                         | 2.80                    | 6.66                         | 14.11                   |
| veh.cluster.WheelHistory.wheelTicks     | 3.02                         | 1.16                    | 1.96                         | 2.66                    | 6.52                         | 16.25                   |
| veh.cluster.WheelHistory.wheelTemp      | 2.92                         | 1.07                    | 1.20                         | 1.23                    | 3.04                         | 5.06                    |
| veh.cluster.WheelHistory.wheelSlipRate  | 2.91                         | 1.07                    | 1.18                         | 1.21                    | 3.07                         | 5.11                    |
| veh.cluster.WheelSummary.wheelHealth    | 3.01                         | 1.16                    | 1.96                         | 2.66                    | 6.53                         | 16.27                   |
| veh.cluster.wheels.ticks                | 3.01                         | 1.16                    | 1.96                         | 2.66                    | 6.52                         | 16.28                   |
| veh.cluster.WheelDiagnostics.wheelTicks | not built                    | not built               | not built                    | not built               | not built                    | not built               |
| veh.hvac.cabin.cabinTemp                | 2.92                         | 1.08                    | 1.20                         | 1.23                    | 3.04                         | 5.06                    |
| veh.hvac.cabin.targetTemp               | 2.91                         | 1.07                    | 1.20                         | 1.23                    | 3.04                         | 5.07                    |
| veh.hvac.cabin.blowerDuty               | 2.92                         | 1.08                    | 1.20                         | 1.23                    | 3.07                         | 5.12                    |
| veh.hvac.cabin.fanLevel                 | 3.01                         | 1.16                    | 1.96                         | 2.67                    | 6.52                         | 16.27                   |
| veh.hvac.cabin.compressorLoad           | 2.91                         | 1.07                    | 1.21                         | 1.24                    | 3.07                         | 5.11                    |
| veh.cabin.Cabin.temperature             | 3.06                         | 1.50                    | 1.96                         | 2.69                    | 6.74                         | 13.52                   |
| veh.cabin.Horn.active                   | 3.29                         | 2.15                    | 2.09                         | 2.77                    | 2.49                         | 4.50                    |
| bench.stress.Stress.longString          | 0.93                         | 0.15                    | 0.82                         | 0.86                    | 1.16                         | 1.23                    |
| bench.stress.Stress.itemList            | 1.83                         | 1.60                    | 1.05                         | 1.77                    | 1.20                         | 1.79                    |
| bench.stress.Stress.nestedOptional      | 2.35                         | 1.64                    | 1.14                         | 1.55                    | 1.24                         | 1.57                    |
| bench.stress.Stress.map                 | 1.32                         | 2.26                    | 4.70                         | 7.28                    | 1.21                         | 1.83                    |
| bench.stress.Stress.bigString           | 0.82                         | 0.10                    | 0.74                         | 0.76                    | 1.01                         | 1.05                    |
| bench.stress.Stress.bigItemList         | 1.76                         | 1.67                    | 1.02                         | 1.63                    | 1.18                         | 1.72                    |
| bench.stress.Stress.bigSamples          | 0.85                         | 7.12                    | 183.06                       | 197.67                  | 0.96                         | 1.46                    |
| bench.stress.Stress.bigMap              | 1.21                         | 9.15                    | 28.94                        | 44.72                   | 1.26                         | 1.87                    |
| bench.stress.Stress.deepNested          | 2.07                         | 1.81                    | 1.09                         | 1.71                    | 1.21                         | 1.71                    |
| bench.stress.Stress.variant             | 2.15                         | 1.13                    | 1.36                         | 1.40                    | 1.59                         | 1.94                    |
| bench.stress.Stress.deepSparse          | 1.82                         | 0.91                    | 0.79                         | 0.35                    | 1.94                         | 1.80                    |
| bench.stress.Stress.deepFull            | 1.50                         | 1.33                    | 0.95                         | 1.29                    | 1.18                         | 1.52                    |

##### CPU load, paced and derived

CPU load in percent of one core at 4000 updates per second: paced (process CPU
time over wall time, includes the cost of sleep and wake-up) next to the derived
figure, and their difference.

| Payload                                | Encoding    | Paced % | Derived % | Difference |
| -------------------------------------- | ----------- | ------- | --------- | ---------- |
| veh.cluster.VehicleStatus.currentSpeed | FlatBuffers | 2.1046  | 0.0600    | 2.0446     |
| veh.cluster.VehicleStatus.currentSpeed | proto3      | 2.1327  | 0.0510    | 2.0817     |
| veh.cluster.VehicleStatus.currentSpeed | podbuf      | 2.0457  | 0.0333    | 2.0124     |
| veh.cluster.VehicleStatus.engineTemp   | FlatBuffers | 2.2219  | 0.0612    | 2.1606     |
| veh.cluster.VehicleStatus.engineTemp   | proto3      | 2.1099  | 0.0482    | 2.0617     |
| veh.cluster.VehicleStatus.engineTemp   | podbuf      | 2.0258  | 0.0346    | 1.9913     |
| veh.cluster.VehicleStatus.warnings     | FlatBuffers | 2.0728  | 0.0404    | 2.0323     |
| veh.cluster.VehicleStatus.warnings     | proto3      | 2.0503  | 0.0408    | 2.0096     |
| veh.cluster.VehicleStatus.warnings     | podbuf      | 1.9572  | 0.0135    | 1.9437     |
| veh.cluster.WheelHistory.wheelTicks    | FlatBuffers | 2.0475  | 0.0401    | 2.0074     |
| veh.cluster.WheelHistory.wheelTicks    | proto3      | 2.0381  | 0.0330    | 2.0051     |
| veh.cluster.WheelHistory.wheelTicks    | podbuf      | 1.9701  | 0.0135    | 1.9566     |
| veh.cluster.WheelHistory.wheelTemp     | FlatBuffers | 2.1234  | 0.0613    | 2.0621     |
| veh.cluster.WheelHistory.wheelTemp     | proto3      | 2.1028  | 0.0483    | 2.0545     |
| veh.cluster.WheelHistory.wheelTemp     | podbuf      | 2.0105  | 0.0346    | 1.9759     |
| veh.cluster.WheelHistory.wheelSlipRate | FlatBuffers | 2.1643  | 0.0609    | 2.1034     |
| veh.cluster.WheelHistory.wheelSlipRate | proto3      | 2.1429  | 0.0479    | 2.0950     |
| veh.cluster.WheelHistory.wheelSlipRate | podbuf      | 2.0504  | 0.0347    | 2.0157     |
| veh.cluster.WheelSummary.wheelHealth   | FlatBuffers | 2.0635  | 0.0401    | 2.0234     |
| veh.cluster.WheelSummary.wheelHealth   | proto3      | 2.0421  | 0.0330    | 2.0091     |
| veh.cluster.WheelSummary.wheelHealth   | podbuf      | 1.9845  | 0.0135    | 1.9710     |
| veh.cluster.wheels.ticks               | FlatBuffers | 2.0517  | 0.0401    | 2.0117     |
| veh.cluster.wheels.ticks               | proto3      | 2.0414  | 0.0331    | 2.0083     |
| veh.cluster.wheels.ticks               | podbuf      | 1.9901  | 0.0135    | 1.9766     |
| veh.hvac.cabin.cabinTemp               | FlatBuffers | 2.1494  | 0.0613    | 2.0880     |
| veh.hvac.cabin.cabinTemp               | proto3      | 2.1104  | 0.0483    | 2.0621     |
| veh.hvac.cabin.cabinTemp               | podbuf      | 2.0171  | 0.0346    | 1.9824     |
| veh.hvac.cabin.targetTemp              | FlatBuffers | 2.1540  | 0.0611    | 2.0928     |
| veh.hvac.cabin.targetTemp              | proto3      | 2.1407  | 0.0483    | 2.0924     |
| veh.hvac.cabin.targetTemp              | podbuf      | 2.0442  | 0.0347    | 2.0096     |
| veh.hvac.cabin.blowerDuty              | FlatBuffers | 2.1504  | 0.0609    | 2.0895     |
| veh.hvac.cabin.blowerDuty              | proto3      | 2.1409  | 0.0480    | 2.0928     |
| veh.hvac.cabin.blowerDuty              | podbuf      | 1.9881  | 0.0343    | 1.9538     |
| veh.hvac.cabin.fanLevel                | FlatBuffers | 2.0501  | 0.0401    | 2.0101     |
| veh.hvac.cabin.fanLevel                | proto3      | 2.0495  | 0.0331    | 2.0164     |
| veh.hvac.cabin.fanLevel                | podbuf      | 2.1140  | 0.0135    | 2.1005     |
| veh.hvac.cabin.compressorLoad          | FlatBuffers | 2.1526  | 0.0609    | 2.0917     |
| veh.hvac.cabin.compressorLoad          | proto3      | 2.1561  | 0.0479    | 2.1082     |
| veh.hvac.cabin.compressorLoad          | podbuf      | 2.0606  | 0.0341    | 2.0265     |
| veh.cabin.Cabin.temperature            | FlatBuffers | 2.1982  | 0.0406    | 2.1576     |
| veh.cabin.Cabin.temperature            | proto3      | 2.1529  | 0.0338    | 2.1190     |
| veh.cabin.Cabin.temperature            | podbuf      | 2.0552  | 0.0135    | 2.0417     |
| veh.cabin.Horn.active                  | FlatBuffers | 2.1439  | 0.0440    | 2.0999     |
| veh.cabin.Horn.active                  | proto3      | 2.1394  | 0.0405    | 2.0989     |
| veh.cabin.Horn.active                  | podbuf      | 2.0477  | 0.0152    | 2.0326     |
| bench.stress.Stress.longString         | FlatBuffers | 4.4005  | 0.2870    | 4.1135     |
| bench.stress.Stress.longString         | proto3      | 3.9718  | 0.1746    | 3.7972     |
| bench.stress.Stress.longString         | podbuf      | 4.5128  | 0.3098    | 4.2030     |
| bench.stress.Stress.itemList           | FlatBuffers | 8.0062  | 1.3806    | 6.6256     |
| bench.stress.Stress.itemList           | proto3      | 9.1055  | 1.8218    | 7.2837     |
| bench.stress.Stress.itemList           | podbuf      | 6.6590  | 1.0508    | 5.6083     |
| bench.stress.Stress.nestedOptional     | FlatBuffers | 2.8207  | 0.1830    | 2.6377     |
| bench.stress.Stress.nestedOptional     | proto3      | 2.7735  | 0.1914    | 2.5821     |
| bench.stress.Stress.nestedOptional     | podbuf      | 2.5924  | 0.1208    | 2.4717     |
| bench.stress.Stress.map                | FlatBuffers | 13.9097 | 3.1590    | 10.7508    |
| bench.stress.Stress.map                | proto3      | 19.5479 | 4.9534    | 14.5945    |
| bench.stress.Stress.map                | podbuf      | 7.4485  | 1.2618    | 6.1867     |
| bench.stress.Stress.bigString          | FlatBuffers | 13.4504 | 1.2442    | 12.2062    |
| bench.stress.Stress.bigString          | proto3      | 11.8433 | 0.6952    | 11.1481    |
| bench.stress.Stress.bigString          | podbuf      | 14.7141 | 1.5341    | 13.1800    |
| bench.stress.Stress.bigItemList        | FlatBuffers | 37.7135 | 8.4303    | 29.2832    |
| bench.stress.Stress.bigItemList        | proto3      | 44.4793 | 11.1774   | 33.3019    |
| bench.stress.Stress.bigItemList        | podbuf      | 29.7617 | 6.6588    | 23.1029    |
| bench.stress.Stress.bigSamples         | FlatBuffers | 20.4111 | 4.9930    | 15.4181    |
| bench.stress.Stress.bigSamples         | proto3      | 63.1438 | 16.5316   | 46.6122    |
| bench.stress.Stress.bigSamples         | podbuf      | 16.4683 | 3.6866    | 12.7817    |
| bench.stress.Stress.bigMap             | FlatBuffers | 99.7907 | 106.5710  | -6.7803    |
| bench.stress.Stress.bigMap             | proto3      | 99.7856 | 184.8287  | -85.0431   |
| bench.stress.Stress.bigMap             | podbuf      | 38.3788 | 9.2115    | 29.1673    |
| bench.stress.Stress.deepNested         | FlatBuffers | 24.0642 | 5.2196    | 18.8446    |
| bench.stress.Stress.deepNested         | proto3      | 26.9283 | 6.5523    | 20.3760    |
| bench.stress.Stress.deepNested         | podbuf      | 17.8461 | 3.7817    | 14.0644    |
| bench.stress.Stress.variant            | FlatBuffers | 2.7385  | 0.1713    | 2.5671     |
| bench.stress.Stress.variant            | proto3      | 2.6954  | 0.1470    | 2.5484     |
| bench.stress.Stress.variant            | podbuf      | 2.5769  | 0.0995    | 2.4774     |
| bench.stress.Stress.deepSparse         | FlatBuffers | 4.6775  | 0.3630    | 4.3145     |
| bench.stress.Stress.deepSparse         | proto3      | 3.9284  | 0.2370    | 3.6914     |
| bench.stress.Stress.deepSparse         | podbuf      | 3.9945  | 0.2648    | 3.7297     |
| bench.stress.Stress.deepFull           | FlatBuffers | 7.2549  | 1.1470    | 6.1079     |
| bench.stress.Stress.deepFull           | proto3      | 7.8183  | 1.3653    | 6.4530     |
| bench.stress.Stress.deepFull           | podbuf      | 6.8719  | 0.9785    | 5.8934     |

#### Payloads not built

The bench could not build a value of these payloads, so no encoding was tried on
them. This is not an encoding refusal.

| Payload                                 | Payload type | Reason                                                                                                           |
| --------------------------------------- | ------------ | ---------------------------------------------------------------------------------------------------------------- |
| veh.cluster.WheelDiagnostics.wheelTicks | RawTickCount | the payload type RawTickCount is internal to its package, so a value cannot be built outside the generated crate |

#### Refused cells

None.
