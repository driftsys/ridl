# Release 0.10 — lane W plan (wire and runtime)

**Brief:** section 4, "Lane W", of
[`2026-10-11-release-0.10-lanes-plan.md`](../wip/2026-10-11-release-0.10-lanes-plan.md).
**Spec:** section 5.3 of
[`2026-10-11-release-0.10-design.md`](../wip/2026-10-11-release-0.10-design.md).

Sebastien delegated every decision of this lane on 2026-10-11, the design of
step 1 included. Each ruling is recorded in section 1 as `R-W-<n>` and in the
body of the pull request that applies it.

Order: step 6 first, because it needs nothing; then #855 and #858; then the step
1 design (the WebSocket binding document), then the rest of step 2, step 4 and
step 5.

## 1. Rulings

- **R-W-1.** `ridl-loopback` moves to edition 2021 with `rust-version = "1.83"`,
  the minimum of `ridl-rt`. Measured: the packaged crate builds and its tests
  compile with Rust 1.83 once the edition is 2021; `ridl-rt` is its only
  dependency.
- **R-W-2.** `ridl-descriptor` declares `rust-version = "1.88"` and stays at
  edition 2024. ADR-0029 states this second minimum. Measured: `planus` is
  pinned `=1.3.0` (the generated accessors are tied to that version) and
  declares rust-version 1.88, in every feature set. Starting at 1.88 is the safe
  direction for a frozen surface: lowering a minimum later is an addition,
  raising one is a break. The consumers of `ridl-descriptor` are engines and
  tools, not the generated crate, which does not depend on it.
- **R-W-3.** `ridl-ir` gets no `rust-version` of its own. It reaches a user only
  through `ridl-descriptor`'s `std` feature, and `compat-check` builds it there
  at 1.88. It is not a runtime crate under ADR-0029.
- **R-W-4.** The generated crate with default features off does not build with
  Rust 1.83 (`f32::abs` and `f64::abs` are not in `core` there). That is a
  backend defect, filed as #874 for lane C. `compat-check` builds the
  veh-cluster crate at 1.83 with every feature on (what the spec asks); the
  `--no-default-features` cell is added when #874 is fixed.

## 2. Step 6 — minimum Rust version (issue #873)

One pull request, `feat/lane-w-msrv`. Every task runs `just compat-check` and
`just freeze-check` before its commit; Task 4 runs `just build`.

### Task 1 — `ridl-loopback` at the minimum (R-W-1)

- `crates/ridl-loopback/Cargo.toml`: `edition = "2021"` and
  `rust-version = "1.83"` as literals, not inherited. Fix any source that does
  not compile as edition 2021 (the recipe comment says the source already uses
  no let chain).
- `justfile`, `compat-check`: check that the manifest sets edition 2021 and the
  same rust-version as `ridl-rt`, the way it checks `ridl-rt-conformance`.
  Replace the relabelled copy: package `ridl-loopback` with the other two
  crates, extract it, and build the packaged manifest unchanged (only the
  `[workspace]` table and the `[patch.crates-io]` for the packaged `ridl-rt`
  added). Keep the `suite!` expansion: copy `tests/conformance.rs` into the
  extracted package and add the packaged `ridl-rt-conformance` as a path
  dev-dependency, because the package excludes that test. Build it with the
  minimum, as `cargo test --no-run`.
- `cargo xtask freeze update` for the `msrv` dump; the diff of `freeze/msrv.txt`
  is the two `ridl-loopback` lines.

### Task 2 — `ridl-descriptor` at 1.88 (R-W-2, R-W-3)

- `crates/ridl-descriptor/Cargo.toml`: `rust-version = "1.88"`, with a comment
  that names `planus` 1.3.0 as the reason.
- `justfile`, `compat-check`: package `ridl-ir` and `ridl-descriptor` (and any
  other workspace crate they pull in), extract them, patch the workspace
  dependencies to the extracted copies, and build the packaged `ridl-descriptor`
  manifest unchanged with the rust-version it declares (read from the manifest,
  as the recipe reads `ridl-rt`'s), in three feature sets: the defaults,
  `--no-default-features`, and `--no-default-features --features dedup`. Run its
  tests with the defaults if they are in the package. Resolve the standalone
  lockfile with the pin's cargo and
  `CARGO_RESOLVER_INCOMPATIBLE_RUST_VERSIONS=fallback`, so that a dependency
  release with a higher minimum is not picked; then build offline with the 1.88
  toolchain. Install that toolchain the way the recipe installs the minimum.
- `cargo xtask freeze update`: one new line,
  `ridl-descriptor rust-version=1.88`.

### Task 3 — the veh-cluster generated crate at the minimum

- `compat-check` emits the crate for `crates/ridlc/tests/corpus/veh-cluster`
  with the CLI it already builds, sets its edition to 2021 and its rust-version
  to the minimum if the emitted manifest does not, patches `ridl-rt` to the
  packaged copy, and runs `cargo +<minimum> build
  --all-features` on it. Keep
  the existing cabin cell.

### Task 4 — records

- ADR-0029: the Context bullet, the paragraph under the table of decision 1, the
  table row, and decision 5 say that the minimum of Rust 1.83 and edition 2021
  applies to `ridl-rt`, `ridl-rt-conformance` and `ridl-loopback`, and that
  `ridl-descriptor`'s minimum is Rust 1.88 (edition 2024); the generated crate
  compiles with 1.83 with every feature on. Add an amendment line to its Status.
- `docs/book/stability.md`, the `compat-check` row of `CONTRIBUTING.md`, the
  `compat-check` entry of `AGENTS.md`, and the `compat-check` comment in
  `justfile` say the same.
- `docs/design/ridl-rt.md` and ADR-0021: correct any sentence that the change
  makes untrue (the relabelled `ridl-loopback` copy); do not restate the policy.

### After the merge

A one-line pull request adds the `msrv` row to `freeze/surfaces.toml` with the
merge commit as `baseline` (lanes plan, rule 5).
