# Encoding bench

Measures the three payload encodings (FlatBuffers, proto3, podbuf) on every
signal payload of the veh-cluster corpus and of `examples/cabin`, and on the
stress payloads of `stress/` (a long string, a list of structs, nested
optionals, a map): encoded size and heap allocations of encode, verify and
decode, and the CPU time of one update. A payload that an encoding refuses gets
a `Refused` row with the reason; a payload the bench cannot build a value of
gets a `NotBuilt` row with the reason. It is its own cargo workspace, outside
the repository's.

    just bench-host

The recipe runs `cargo xtask bench host`, which writes the three generated
crates and `signals.txt` to `bench/encodings/generated/` (not in git), runs the
tests of `harness/`, and prints the table.

## Timing

The headline figure is the CPU load, in percent of one core, of 4 000 updates
per second. One update is encode, verify and decode through `Payload<E>`; the
harness times each of the three operations on its own and states their sum.
Without `--out` the binary prints the table only. With `--out` it also times
every `Measured` row and writes a JSON file; `Refused` and `NotBuilt` rows are
written with their reasons and are not timed.

    cargo run --release -p encoding-bench -- --out result.json --core-label A55

- `--out FILE` turns timing on and names the JSON file.
- `--core-label TEXT` names the core the run is pinned to (for example `A55`);
  the harness does not pin itself, use `taskset`.
- `--duration SECONDS` is the CPU time spent in the samples of one operation of
  one cell and encoding, so a payload takes three times this per encoding. The
  default is 0.5.
- `--paced` also runs each cell at 4 000 updates per second, sleeping to the
  next tick, and reads process CPU time (`getrusage`) over the run. The JSON
  holds that figure, the figure derived from the unpaced cost, and the
  difference. It needs `--out`.
- `--commit HASH` states the commit when the machine has no `git`.
- `--date YYYY-MM-DD` states the date of the run when the clock of the machine
  is wrong; without it the date is the UTC date of the clock.

CPU time comes from an unpaced loop: thread CPU time (`CLOCK_THREAD_CPUTIME_ID`)
over batches, 30 samples per operation, with every operation of the three
encodings run in rotation inside each sample. A batch repeats one operation
only: encode writes into the caller's buffer, verify checks a fixed set of
already-encoded bytes, and decode reads a proof that was built once before the
batch (the proof is copied bitwise for each call, because `Ref::decode` consumes
it; the harness refuses a payload whose proof owns data that is dropped). The
per-update figure is the sum of the three operation medians; its p05, p95, min
and max are the sums of the operation bounds, an envelope and not percentiles of
a summed sample. A stat is flagged when `(p95 - p05) / median` or
`(max - min) / median` is above 10 % (the range catches one disturbed sample,
which p95 of 30 samples cannot); the flag of the per-update figure is raised
when any operation is flagged. The paced run repeats the whole update.

JSON fields of a measured row, beyond the payload, its type and the counts:
`per_update_ns` (median, p05, p95, min, max, range_spread, reps, spread,
spread_flagged), `cpu_percent_at_rate` (the per-update median at the rate),
`per_op_ns` with `encode`, `verify` and `decode`, each with the same fields as
`per_update_ns` and its own `cpu_percent_at_rate`, and `paced` when asked for.
`encode_allocs`, `verify_allocs` and `decode_allocs` each count that call alone.
Each JSON row names its payload and its payload type, so that a report can
weight the signals or the distinct payload types equally. The provenance block
holds the date, the commit, the platform, the core label, the toolchain, and the
frequency and governor of the core when the system exposes them.

## Pixel 5

    just bench-pixel

The recipe runs `cargo xtask bench pixel`. It needs `adb`, `zig`,
`cargo-zigbuild`, the `aarch64-unknown-linux-musl` rustup target and a Pixel 5
with USB debugging enabled. The command fails before the build when
`adb devices` lists no ready device (with `ANDROID_SERIAL` set, when that serial
is not listed). The phone is not rooted and the recipe changes no setting of it:
it pushes the binary to `/data/local/tmp`, runs it, and reads files under
`/sys`.

The command cross-builds `encoding-bench` as a static binary, then runs it twice
under `adb shell taskset`: pinned to cpu2 (one of the A55 cores, cpu0 to cpu5,
up to 1.8 GHz), then to cpu6 (an A76 core, 2.2 GHz). Each run uses
`--duration 2 --paced`: 2 s is long enough that a `getrusage` tick of 10 ms is
0.5 % of the paced run. The commit is passed with `--commit`, because the phone
has no repository, and the host date with `--date`, because the clock of the
phone can be wrong. It pulls the results to `bench/encodings/results/` (not in
git):

- `pixel5-a55-<date>.json` and `pixel5-a76-<date>.json`, the harness output.
- `pixel5-a55-<date>.cpufreq.txt` and `pixel5-a76-<date>.cpufreq.txt`, the
  governor and the current frequency of each of the eight cores before and after
  the run.

Hardware counters are not available on the phone. The linker wrapper that the
command writes under `target/zig-linker/` removes `-Wl,--fix-cortex-a53-843419`,
which `zig cc` of zig 0.16 rejects.

## Report

    cargo xtask bench report

The command reads every `bench/encodings/results/*.json` file and prints
Markdown tables to standard output, one section per file: the equal-weight mix
per signal and per payload type, for the update and for each operation (time,
CPU load and heap allocations of encode, verify and decode), then per payload
the size, the allocations, the time per update, the derived CPU load, the ratios
to podbuf, the time per operation, the CPU load per operation, the ratios per
operation (each operation divided by the podbuf time of the same operation), and
the paced and derived CPU load side by side. Refused cells and payloads the
bench could not build are marked and listed apart, and are in no ratio and no
mix. The tables are written by `report/`, a program of this workspace. The
method and the results are in
[`docs/technotes/encoding-bench.md`](../../docs/technotes/encoding-bench.md).
