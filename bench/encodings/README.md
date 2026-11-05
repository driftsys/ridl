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
per second. One update is encode, verify and decode through `Payload<E>`.
Without `--out` the binary prints the table only. With `--out` it also times
every `Measured` row and writes a JSON file; `Refused` and `NotBuilt` rows are
written with their reasons and are not timed.

    cargo run --release -p encoding-bench -- --out result.json --core-label A55

- `--out FILE` turns timing on and names the JSON file.
- `--core-label TEXT` names the core the run is pinned to (for example `A55`);
  the harness does not pin itself, use `taskset`.
- `--duration SECONDS` is the CPU time spent in the samples of one cell and
  encoding. The default is 0.5.
- `--paced` also runs each cell at 4 000 updates per second, sleeping to the
  next tick, and reads process CPU time (`getrusage`) over the run. The JSON
  holds that figure, the figure derived from the unpaced cost, and the
  difference. It needs `--out`.
- `--commit HASH` states the commit when the machine has no `git`.

Per-update CPU time comes from an unpaced loop: thread CPU time
(`CLOCK_THREAD_CPUTIME_ID`) over batches, 30 samples per cell, with the three
encodings run in rotation inside each sample. A cell whose
`(p95 - p05) / median` or `(max - min) / median` is above 10 % is flagged in the
JSON (the range catches one disturbed sample, which p95 of 30 samples cannot).
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
has no repository. It pulls the results to `bench/encodings/results/` (not in
git):

- `pixel5-a55-<date>.json` and `pixel5-a76-<date>.json`, the harness output.
- `pixel5-a55-<date>.cpufreq.txt` and `pixel5-a76-<date>.cpufreq.txt`, the
  governor and the current frequency of each of the eight cores before and after
  the run.

Hardware counters are not available on the phone. The linker wrapper that the
command writes under `target/zig-linker/` removes `-Wl,--fix-cortex-a53-843419`,
which `zig cc` of zig 0.16 rejects.
