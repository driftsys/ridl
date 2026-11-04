# Encoding bench Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** Measure size, heap allocations and codec CPU load of proto3,
FlatBuffers and podbuf on the `veh-cluster` and `examples/cabin` corpus, on a
Pixel 5 and on host-independent instruction counts.

**Architecture:** A bench workspace at `bench/encodings/`, outside the root
workspace as `examples/cabin` is. Its crates link Rust crates that
`ridl build --emit rust` generates with all three encodings. One harness,
generic over `E: Encoding` through `Payload<E>`, runs the same encode, verify
and decode on every encoding and writes JSON with provenance. A report step
turns the JSON into the technote tables. `xtask` recipes drive each platform.
None of it joins `just build`.

**Tech Stack:** Rust (the pinned toolchain), `ridl-rt` `Payload<E>`,
`cargo zigbuild`, `adb`, `taskset`, `iai-callgrind`, QEMU `mps3-an536`.

**Spec:** `docs/wip/2026-11-04-encoding-bench-design.md`

## Global Constraints

- The load target is 4 000 updates per second sustained. The burst of 200
  messages in 10 ms is deferred by the owner.
- The measurement covers the codec only: `Payload::encode`, `Payload::verify`,
  `Payload::decode`. No transport.
- CPU load is derived from per-update CPU time in an unpaced busy loop; one
  paced run at 4 000 per second on the Pixel 5 checks it.
- Size and allocation counts are exact; time is reported as medians of at least
  30 repetitions with the spread.
- The three encodings are interleaved inside a run.
- Pixel 5 target: static `aarch64-unknown-linux-musl`, pushed to
  `/data/local/tmp`, pinned with `taskset` to one A55 core (cpu0 to cpu5) and to
  one A76 core (cpu6 or cpu7). The device is not rooted.
- The bench is not a member of `just build`, and nothing in it runs in CI.
- Prose is plain and literal; shipped docs name no story, stage or epic ids
  (`just story-id-check`). The technote lives in `docs/technotes/`.
- Conventional Commits with a scope from `.git-std.toml`.
- Phase 1 is Tasks 1 to 5. Phase 2 is Tasks 6 and 7.

## Review Focus

- A payload whose encoded size exceeds the stack buffer (`Payload::BUFFER` is
  `MAX_SIZE` bytes): the harness must not overflow or silently skip it.
- An encoding that refuses a corpus value (for example a string with U+0000 in
  podbuf): the harness must report the refusal for that cell, not panic or drop
  the row.
- A repetition disturbed by throttling: the report must show the spread, and a
  spread above 10 % of the median must be flagged in the table.
- The allocator counter must count only the measured call, not harness
  bookkeeping (result vectors, JSON writing).
- The compiler removing the measured work: every result must pass through
  `std::hint::black_box`.

---

### Task 1: Bench workspace, generation, and the size and allocation table for one payload

**Files:**

- Create: `bench/encodings/Cargo.toml` (own `[workspace]`),
  `bench/encodings/harness/` (crate `encoding-bench`),
  `bench/encodings/README.md`
- Create: `xtask/src/bench.rs`; Modify: `xtask/src/main.rs` (add `bench`),
  `justfile` (recipe `bench-host`, not in `build`)
- Test: `bench/encodings/harness/tests/counts.rs`

**Interfaces:**

- Produces: `fn measure_counts<T: Payload<E>, E: Encoding>(value: &T) -> Counts`
  where
  `Counts { size: usize, encode_allocs: u64, verify_allocs: u64, decode_allocs: u64 }`;
  `struct CountingAlloc` (a `#[global_allocator]` with `reset()` and
  `count() -> u64`); `cargo xtask bench gen` writes the generated crates under
  `bench/encodings/generated/` (git-ignored).
- Consumes: `ridl build <package> --emit rust --out-dir <dir>` and the manifest
  `[backend.rust] encodings` of `crates/ridlc/tests/corpus/veh-cluster`.

- [ ] **Step 1:** Write the failing test
      `counts_for_one_veh_cluster_signal_are_nonzero_size_and_zero_podbuf_allocs`:
      encode one `veh-cluster` signal under each encoding, assert `size > 0` for
      all three and `decode_allocs == 0` for a payload with no string or list
      under podbuf (state which payload and why in a comment).
- [ ] **Step 2:** Run `cargo test --manifest-path bench/encodings/Cargo.toml` →
      FAIL (nothing defined).
- [ ] **Step 3:** Implement `CountingAlloc` (wraps `System`, counts `alloc`
      calls with an atomic), `measure_counts`, and `xtask bench gen` (runs
      `ridl build` for the veh-cluster corpus with all three encodings into
      `bench/encodings/generated/veh-cluster`).
- [ ] **Step 4:** Run the test → PASS.
- [ ] **Step 5:** Add `bench/encodings/generated/` to `.gitignore`; confirm
      `just gate-parity` still passes (the new recipe is outside `build`).
- [ ] **Step 6:** Commit
      `feat(xtask): bench workspace and size and allocation counts for one payload`.

### Task 2: The whole corpus and the stress payloads

**Files:**

- Create: `bench/encodings/stress/ridl.toml` and `stress/*.ridl` (a package of
  the stress shapes: long string, list of structs, nested optionals, map, each
  as a signal-sized or smaller payload), `bench/encodings/harness/src/corpus.rs`
- Modify: `xtask/src/bench.rs` (generate `examples/cabin`, the corpus crate, and
  the stress package)
- Test: `bench/encodings/harness/tests/corpus.rs`

**Interfaces:**

- Produces: `fn corpus() -> Vec<CorpusEntry>` where
  `CorpusEntry { name: &'static str, kind: Kind /* Signal | Stress */, run: &'static dyn Fn(&mut Recorder) }`,
  one entry per payload, each building a representative value by hand;
  `struct Recorder` collects one row per `(payload, encoding)`.
- Consumes: Task 1's `measure_counts`.

- [ ] **Step 1:** Write the failing test
      `every_signal_of_the_corpus_has_an_entry`: list the signal payloads of
      `veh-cluster` and `examples/cabin` from the catalog (use `ridl-mcp` or the
      generated catalog accessors) and assert each name appears in `corpus()`.
- [ ] **Step 2:** Run it → FAIL.
- [ ] **Step 3:** Write the stress package and the entries. A cell that an
      encoding refuses (an encode error) is recorded as `Refused { reason }`,
      not a panic.
- [ ] **Step 4:** Add the test `a_refused_cell_is_recorded_not_dropped` using a
      value that one encoding refuses.
- [ ] **Step 5:** Run the tests → PASS; run `cargo xtask bench host` and check
      the size and allocation table prints for every row.
- [ ] **Step 6:** Commit
      `feat(xtask): bench the whole corpus and the stress payloads`.

### Task 3: The timing harness, sustained load

**Files:**

- Create: `bench/encodings/harness/src/timing.rs`,
  `bench/encodings/harness/src/main.rs` (CLI: `--out result.json`,
  `--core-label`, `--duration`, `--paced`)
- Test: `bench/encodings/harness/tests/timing.rs`

**Interfaces:**

- Produces: `fn per_update_cpu_ns(entry, encoding) -> Stat` with
  `Stat { median_ns, p05_ns, p95_ns, reps }`;
  `fn sustained_cpu_percent(per_update_ns: f64, rate_per_s: f64) -> f64`;
  `result JSON with a`provenance` block (date, commit, platform, core label,
  toolchain, observed frequency when readable).
- Consumes: Task 2's `corpus()`.

- [ ] **Step 1:** Write failing tests:
      `sustained_cpu_percent(250_000.0, 4000.0) == 100.0`;
      `spread_above_ten_percent_is_flagged`.
- [ ] **Step 2:** Run → FAIL.
- [ ] **Step 3:** Implement the three functions and the measurement loop. Per
      update the loop does encode, verify, decode on the same value, through
      `black_box`. One CPU-time sample is the thread CPU time
      (`clock_gettime(CLOCK_THREAD_CPUTIME_ID)`) over a batch; at least 30
      samples; the three encodings are interleaved in rotation per sample.
      Warm-up is a fixed number of batches the builder chooses.
- [ ] **Step 4:** Add `--paced`: a loop that sleeps to the next tick at 4 000
      per second; report process CPU time (`getrusage`) over the run next to the
      derived figure and the difference.
- [ ] **Step 5:** Run the tests → PASS; run the binary on the host and check the
      JSON.
- [ ] **Step 6:** Commit `feat(xtask): bench timing harness for sustained load`.

### Task 4: The Pixel 5 recipe

**Files:**

- Modify: `xtask/src/bench.rs` (subcommand `bench pixel`), `justfile`
  (`bench-pixel`)
- Create: `bench/encodings/README.md` section "Pixel 5"

**Interfaces:**

- Consumes: Task 3's binary and its flags. Produces:
  `bench/encodings/results/pixel5-<core>-<date>.json` (git-ignored; the technote
  carries the tables).

- [ ] **Step 1:** Implement `bench pixel`: fail with a clear message when
      `adb devices` lists no device;
      `cargo zigbuild --release --target aarch64-unknown-linux-musl`; `adb push`
      to `/data/local/tmp`; run under `adb shell taskset <mask>` for one A55
      core and for one A76 core; `adb pull` the JSON; also record the governor
      and each core's frequency before and after.
- [ ] **Step 2:** Verify by running `just bench-pixel` with the phone attached:
      expected two result files, each with provenance naming the core and
      `reps >= 30`. Record the command output in the PR description.
- [ ] **Step 3:** Verify the no-device path: run with the phone unplugged (or
      `ANDROID_SERIAL=none`) and expect the explicit error and a non-zero exit.
- [ ] **Step 4:** Commit `feat(xtask): run the encoding bench on a Pixel 5`.

### Task 5: The report and the technote (closes phase 1)

**Files:**

- Create: `bench/encodings/report/` (a small bin that reads result JSON and
  writes Markdown tables), `docs/technotes/encoding-bench.md`
- Modify: `docs/technotes/README.md` (index entry), `xtask/src/bench.rs`
  (`bench report`)
- Test: `bench/encodings/report/tests/tables.rs`

**Interfaces:**

- Consumes: result JSON from Tasks 3 and 4. Produces: per-payload tables and
  ratio tables (each encoding relative to podbuf).

- [ ] **Step 1:** Failing test
      `ratio_table_divides_by_the_podbuf_column_and_marks_refused_cells`.
- [ ] **Step 2:** Run → FAIL; implement; run → PASS.
- [ ] **Step 3:** Run all of Tasks 1 to 4 and write the technote: date, commit,
      platform, method; the size, allocation and CPU % tables for sustained 4
      000 per second on A55 and A76; what each encoding's verify and decode do;
      what each number does and does not show; the link to
      `docs/archive/2026-10-24-podbuf-design.md`. No verdict unless unambiguous.
- [ ] **Step 4:** Run `just fmt`, `just check`, `just link-check`,
      `just doc-path-check`, `just story-id-check`.
- [ ] **Step 5:** Commit `docs(docs): technote with the encoding bench results`.

### Task 6: Instruction counts under Valgrind (phase 2)

**Files:** Create `bench/encodings/iai/` (crate using `iai-callgrind`), a
container definition under `bench/encodings/iai/`; Modify `xtask/src/bench.rs`
(`bench iai`).

**Interfaces:** Consumes Task 2's `corpus()` entries (one iai function per
`(payload, encoding, operation)`). Produces instruction counts and simulated
cache misses per cell.

- [ ] **Step 1:** Spike: confirm `iai-callgrind` and Valgrind run in an AArch64
      Linux container on the Mac; record the working commands in the README.
- [ ] **Step 2:** Implement the iai functions for the signal payloads of the
      corpus; verify counts are identical across two runs (deterministic).
- [ ] **Step 3:** Add the counts to the technote and the report; commit
      `feat(xtask): instruction counts for the encoding bench`.

### Task 7: Cortex-R52 instruction counts under QEMU (phase 2)

**Files:** Create `bench/encodings/r52/` (`no_std` crate for
`armv8r-none-eabihf`), `xtask` subcommand `bench r52`.

**Interfaces:** Consumes the generated codecs with default features off (no
heap). Produces instruction counts per cell for the encodings that build without
a heap.

- [ ] **Step 1:** Spike: boot a `no_std` binary on
      `qemu-system-arm -M mps3-an536` and read the instruction count with QEMU's
      instruction-count plugin; record the commands.
- [ ] **Step 2:** Run the codecs that build without `std` and `alloc`; record
      the encodings that cannot build as "needs a heap" in the technote.
- [ ] **Step 3:** Add the counts to the technote; commit
      `feat(xtask): Cortex-R52 instruction counts for the encoding bench`.
