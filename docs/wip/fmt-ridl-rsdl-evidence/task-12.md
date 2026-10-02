# Task 12 evidence

Branch: `feat/387-fmt-completion`, Task 11 parent `8b135ba`. D-H34 records the
sweep and test placement; D-H33 records the main-targeted final PR group.

## Tests first

`cargo test -p ridl-cli --test book_examples formatter_fixed_point --locked`
exits 101 before any source sweep: three failures, zero passing selected tests.
Log: `task-12-red-fixed-points.log`. The failures are the exact noncanonical
spacing of cabin/baseline and all 32 verified book bodies. Parsing,
structure/content and second-pass checks already pass, so the failure proves the
canonical-source assertion rather than an invalid fixture.

The new book fixed-point test reuses the compiler harness's existing `classify`,
`fenced_blocks` and unchanged `MDBOOK_OPTIONS`. Both ordinary and diagnostic
allowance fences are selected, while ignored and included reference fences stay
outside the set. The shared formatter test-only invariant module compares every
node and token, including comments, at widths 100, 60 and 40.

## Sweep and verification

An external temporary copy of the existing parser exports 32 verified bodies,
all parse-valid, including six diagnostic-allowance fences. Each is staged with
its profile extension under a root EditorConfig with width 100. Commands:

- `cargo run --locked -p ridl-cli --bin ridl -- fmt /tmp/ridl-fmt-task12-sweep`:
  exit 0; `task-12-fence-format.log`.
- The same CLI `fmt` command over `examples/cabin/cabin.ridl` and
  `crates/ridl/tests/baseline-corpus/cluster.ridl`: exit 0; respective example
  and baseline format logs. Git's source inventory finds only the cabin file
  under tracked examples. No parser inputs are reformatted.
- `cargo run --locked -p ridl-cli --bin ridl -- fmt --check crates/ridl-fmt/test_data/formatted`:
  exit 0; `task-12-golden-check.log`. Existing goldens already are canonical.

Bodies are replaced in descending parser-reported source order, with an exact
original-body assertion before each replacement. A full parser inventory then
checks every fence info string, every ignored body and every other code body:
only the 32 verified source bodies and the regenerated tutorial text diagnostic
change. All `ignore`/`allow=` markers remain unchanged.

`cargo test -p ridl-cli --test book_examples --test baseline_desk --test baseline_gate --locked`
exits 0: 40 book tests, 49 desk tests and 10 baseline gate tests. Log:
`task-12-green-acceptance.log`. Book compilation still checks all declared
allowances; the new tests check canonical output, structure, content/comments
and second-pass equality. The committed baseline IR JSON is unchanged.

The actual baseline service moves to `cluster.ridl:48:9`; all four quoted
interaction lines now have tight colons. Running `ridl check .` on the formatted
tutorial vocabulary with reversed bounds exits 1 with the intended TYPL-104,
reporting `./veh/common/types.ridl:4:18`. Its text is pasted from actual output.
The six inline quotations and canonical-layout paragraph are corrected; the
existing formatter link and the reference's `frame : bytes [8]` quotation stay.

A temporary disk shortage prevented the first sweep command from starting.
`cargo clean` removed this work's disposable build caches, then the command was
rerun. Source changes and review records were preserved. All command logs remain
local and ignored; final workspace/demo and book/link acceptance follow.

Acceptance is complete: `just test` (workspace suite), `just demo`,
`just book-check`, `just link-check`, `just fmt-check` and `just check` all
exit 0. The demo verifies six round trips, including both blocking calls. Logs:
`task-12-workspace-test.log`, `task-12-demo.log`, `task-12-book-check.log` and
`task-12-link-check.log`.

The book chapters are deliberately in `.primignore`; prim leaves their source
fences byte-exact. Rust formatting and connective-tissue checks pass. The
post-sweep diff contains no parser corpus, specification/general-form document,
ignored book fence or baseline IR change. QUICK review follows the task commit.

## QUICK review and path-width correction

All three QUICK seats completed on `ad877b4`: native built-in bug review at
Terra/medium and restricted docs at Terra/high report no findings. The tests
seat at Terra/high reports that the fixed-point comparison must resolve the
path's width rather than assume 100. Its first run stopped on a full-disk output
error; the fresh retry completed, with actual model metadata retained locally.
Changed-path lists match the committed range.

The width finding is accepted. A temporary rooted EditorConfig sets width 40:
`source_fixed_point_reads_the_effective_path_width` first fails with exit 101 on
the expected assertion because the helper uses default options. After using
`FormatOptions::for_path`, it passes. Cabin and baseline use their real source
paths; book bodies use representative chapter paths with the fence's source
extension, solely for configuration lookup. No file is written there. The
independent structure, content/comment and second-pass checks at widths 100, 60
and 40 remain.

`CARGO_INCREMENTAL=0 cargo test -p ridl-cli --test book_examples --test baseline_desk --test baseline_gate --locked`
passes: 41 book, 49 desk and 10 gate tests. Local logs:
`task-12-quick-red-path-width.log` and `task-12-quick-green-path-width.log`. The
subsequent connective-tissue check identified only an unformatted Task 13
evidence paragraph; it was formatted and the check rerun successfully.

The advisory observation about unpinned prose quotations is handled by the
required manual book audit and actual diagnostic regeneration already recorded
above. No brittle prose-string tests are added. This is the QUICK pass's own
correction; it does not receive another QUICK pass. Final full review remains
required on the published PR.
