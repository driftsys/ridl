# Workspace automation

Run the automation crate from the workspace root:

```sh
cargo xtask codegen
cargo xtask descriptor-codegen
cargo xtask calibrate --help
```

Calibration reads the workspace from the current directory, so run it from the
workspace root. Relative output paths are resolved from that directory.

The first two commands regenerate the typed AST and catalog descriptor
accessors. Their drift tests compare generated output with the committed files.

## Dump calibration findings

```sh
cargo xtask calibrate dump <out-dir>
```

Before creating any directory or invoking Cargo, this command resolves the
output destination, including existing symlink ancestors and missing path
components, and rejects a destination at or below the canonical `evals/corpus/`
directory. It also rejects symlinks in any existing output JSON file or anywhere
inside `.calibrate-target`. On Unix, it rejects hard links to files outside the
validated target tree and hard-linked output JSON files. Cargo may retain hard
links whose aliases are all inside the target tree. All of these checks run
before directory creation or Cargo. This keeps copied workspaces and build
output outside the corpus. Arrays are staged in the private temporary directory
and published by replacing each destination file.

This command builds `ridl-cli` with the locked dependency graph, using
`<out-dir>/.calibrate-target` as a separate build directory. It copies every
workspace under `evals/corpus/` into a temporary directory inside `<out-dir>`,
appends a `[lints]` table setting all five candidate checks to `warn`, and runs
`ridl check --format json`. It refuses a corpus manifest that already contains a
`[lints]` table. The original corpus files are read only. Temporary workspace
copies are removed when the command returns; the build directory remains for
subsequent runs.

The five output files are `<out-dir>/<lint-name>.json`. Each is an array of
records with `id`, `workspace`, `location`, `message`, and a typed `metric` for
thresholded checks. Unit and abbreviation records omit `metric`. Empty arrays
are valid. Diagnostic errors or malformed metadata fail the command before any
finding array is written.

IDs have the form
`<lint>:<workspace>:<relative-source-path>:<start-byte>-<end-byte>:<occurrence>`.
The primary JSON span's one-based Unicode character coordinates are converted
back to UTF-8 byte offsets using the copied source. Occurrence indices start at
zero for each check and primary range. Records are ordered by workspace, check,
relative path and range, preserving compiler order at equal ranges. IDs do not
contain message text or temporary paths. `location` is the relative source path
and one-based line number; temporary directory prefixes are removed from
messages that cite another source site.

The command uses the checks' existing search-start constants. Calibration does
not change production thresholds or levels. Copy every record unchanged into the
labelling and merge stages, retaining both shape kinds and both cohesion
coordinates.

## Derive levels and thresholds

The canonical command and its alias accept the same options:

```sh
cargo xtask calibrate derive
cargo xtask calibrate derive --write
cargo xtask calibrate --derive
cargo xtask calibrate --derive --write
```

Derivation reads all five `evals/calibration/<lint-name>.toml` label files,
review task manifests and their unchanged numbered rubrics, and
`evals/calibration/recall.toml`. It prints the summary; `--write` also writes
`evals/calibration/summary.md`. Missing or invalid inputs fail with exit code 2.
No catalogue row or production constant is updated automatically.

Each label file uses `[[finding]]` records. Preserve the dump's `id`,
`workspace`, `location`, `message`, and optional `[finding.metric]` table. Add
`claude`, `claude_reason`, `sol`, `sol_reason`, and `final`. Labels must be
`accept` or `dismiss`; both reviewers need nonempty reasons and every record
needs a final label. Agreement between reviewers must be preserved in `final`.
Use `finding = []` for a check with no findings. IDs must name existing source
files and valid byte ranges, agree with locations, and have contiguous
occurrence indices. Metric coordinates must agree with the diagnostic message.

The recall join is prepared after blind labelling and adjudication. It
classifies every numbered item in every review rubric, without editing the
rubric. Use `[[item]]` for the inventory:

```toml
[[item]]
id = "review-0001:1"
workspace = "<corpus-directory>"
kind = "issue"
reason = "This item describes a design issue."

[[item]]
id = "review-0001:2"
workspace = "<corpus-directory>"
kind = "alias"
canonical = "review-0001:1"
reason = "This item describes the same issue in the same workspace."

[[item]]
id = "review-0001:3"
workspace = "<corpus-directory>"
kind = "excluded"
reason = "This item constrains the answer rather than describing an issue."
```

The permitted kinds are `issue`, `alias`, and `excluded`. Only aliases carry
`canonical`. An alias must refer directly to an `issue` in the same workspace,
using the lexically first rubric item ID as canonical. Every item needs a
reason. IDs and workspace names must match the existing task manifests and
numbered rubrics. The example describes the schema; the actual inventory must
classify the original review items independently.

Include exactly one `[[check]]` for each of the five lint names. Each check must
have one `[[check.issue]]` row for every canonical issue, including issues it
cannot detect and applicable issues with no matching findings:

```toml
[[check]]
name = "package-fan-out"

[[check.issue]]
id = "review-0001:1"
applicable = true
reason = "This check can detect the issue, but this sample has no match."
findings = []
```

Inapplicable rows set `applicable = false`, give a reason, and have an empty
`findings` array. Applicable rows list matching finding IDs from that check in
the same workspace. Alias and excluded IDs do not get applicability rows. If
there are no canonical issues, use `issue = []` directly under each check.
Unknown IDs, duplicate rows, missing rows and cross-workspace matches fail.

The summary reports findings, accepted findings, precision, distinct detected
applicable issues, total applicable issues, recall and level for each candidate
threshold. Candidates include the search start and each observed metric boundary
that changes the retained set; intermediate thresholds retain the same findings.
Struct and enum thresholds are independent but share one check's precision and
level. Cohesion requires both a minimum group count and a minimum size for its
smallest group. Fan-out reports counts strictly above its maximum.

Precision of at least 80% permits Warning; 50% to below 80% permits Info; below
50% excludes a lint. Fewer than ten retained findings caps the level at Info.
Zero findings have undefined precision and cannot qualify. Select the highest
qualifying level, then the threshold retaining the most findings. Equal counts
prefer the lower field threshold then the lower variant threshold for shapes, or
the lower group count then the lower group size for cohesion. Recall counts each
applicable canonical issue once if any retained finding matches it, regardless
of the finding's label. A zero denominator prints `not applicable`. Recall does
not select or gate a threshold.
