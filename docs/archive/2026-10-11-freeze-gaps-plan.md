# Freeze check gaps (#870, #868) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close two gaps in `just freeze-check` before the first baseline rows
are added to `freeze/surfaces.toml`: a property that moves between `oneOf`
branches of an MCP schema (#870), and a frozen surface whose row was removed for
an approved break and never added back (#868).

**Architecture:** Task 1 changes the MCP dumper (`xtask/src/freeze/mcp.rs`) so
that each `oneOf` branch is walked under its own path, `<pointer>/oneOf/<n>`.
Task 2 adds a top-level `required` list to `freeze/surfaces.toml`. On an
ordinary run, `cargo xtask freeze check` prints a notice for each listed surface
without a row. With `--release`, it fails instead, and the `release` recipe runs
that mode before `git std bump`.

**Tech Stack:** Rust (xtask binary crate), serde/toml, serde_json, just.

**Spec:** `docs/decisions/ADR-0029-stability-policy.md` (decisions 3 and 4),
issues driftsys/ridl#870 and driftsys/ridl#868. Both changes refine the gate
that ADR-0029 describes. Neither changes what is frozen.

## Global Constraints

- The gate stays one recipe, `freeze-check`, in `build` (ADR-0009). This plan
  adds no member to `build`; the release check runs from the `release` recipe.
- A line missing at HEAD that is present at the baseline fails the check. An
  added line passes.
- Regenerate dumps with `cargo xtask freeze update`; never edit a dump by hand.
- Commit scopes come from `.git-std.toml`: use `xtask`, `adr` and `docs`.
- Prose (comments, commit messages, docs) is plain and literal English, no
  idioms. Shipped docs name no story ids, lanes or epics.
- `freeze/surfaces.toml` has no `[[surface]]` rows on this branch, and this plan
  adds none. The rows come in a later pull request.

## Review Focus

1. A `oneOf` nested inside a `oneOf` branch: its paths must carry both indices,
   for example `/oneOf/1/properties/x/oneOf/0`.
2. A `oneOf` branch that is a `$ref`: the branch path must be used for the
   properties of the resolved target.
3. `required` names a surface that no dumper or API checker has: the check fails
   with the name. A typo must not turn into a silent "no row" notice.
4. `required` names the same surface twice: the check fails.
5. `--release` with an empty `required` list and no rows passes, so the release
   recipe works today.

---

### Task 1: Walk each `oneOf` branch under its own path (#870)

**Files:**

- Modify: `xtask/src/freeze/mcp.rs` (`walk`, about lines 72-90, and the module
  doc and the `walk` doc comment)
- Modify: `freeze/mcp.txt` (regenerated, never edited by hand)
- Modify: `freeze/README.md` and `docs/decisions/ADR-0029-stability-policy.md`
  only where they describe the MCP dump's path format. Run
  `grep -n "anyOf\|oneOf\|branch" freeze/README.md docs/decisions/ADR-0029-stability-policy.md`
  first. If neither file mentions how branches are dumped, add one sentence to
  the MCP paragraph of `freeze/README.md`.
- Test: the `#[cfg(test)] mod tests` in `xtask/src/freeze/mcp.rs`

**Interfaces:**

- Consumes: nothing from Task 2.
- Produces: the MCP dump format `<tool> <side> <pointer>/oneOf/<n>/...` for
  paths inside a `oneOf` branch. `anyOf` branches keep the parent pointer.
  `type_of` is unchanged: a `oneOf` node's type line still joins its branch
  types.

Every `anyOf` in `crates/ridl-mcp/tests/tools.json` wraps one object branch and
a `null` branch: it is how a nullable value is written. Merging those branches
onto the parent path is the right model, so `anyOf` is not indexed. `oneOf`
lists alternative shapes (today, the output of `ridl_explain`), so a property
that moves between branches is a change of shape.

- [ ] **Step 1: Write the failing tests.** Add to the tests module in
      `xtask/src/freeze/mcp.rs`. Use the helper the module's existing tests use
      to run `walk` over a literal schema. Read the module's tests first and
      reuse that helper; if there is none, call
      `walk("t", "input", "", &schema, &json!({}), 0, &mut lines)` directly.

```rust
#[test]
fn a_property_that_moves_between_one_of_branches_removes_a_line() {
    let before = json!({"oneOf": [
        {"type": "object", "properties": {"a": {"type": "string"}}},
        {"type": "object", "properties": {"b": {"type": "string"}}}
    ]});
    let after = json!({"oneOf": [
        {"type": "object", "properties": {}},
        {"type": "object", "properties": {"a": {"type": "string"}, "b": {"type": "string"}}}
    ]});
    let lines = |schema: &Value| {
        let mut lines = Vec::new();
        walk("t", "output", "", schema, &json!({}), 0, &mut lines);
        lines
    };
    let before = lines(&before);
    let after = lines(&after);
    assert!(before.contains(&"t output /oneOf/0/properties/a string".to_string()), "{before:?}");
    assert!(!after.contains(&"t output /oneOf/0/properties/a string".to_string()), "{after:?}");
}

#[test]
fn a_nested_one_of_carries_both_indices() {
    let schema = json!({"oneOf": [
        {"type": "object"},
        {"type": "object", "properties": {"x": {"oneOf": [
            {"type": "object", "properties": {"y": {"type": "integer"}}}
        ]}}}
    ]});
    let mut lines = Vec::new();
    walk("t", "output", "", &schema, &json!({}), 0, &mut lines);
    assert!(
        lines.contains(&"t output /oneOf/1/properties/x/oneOf/0/properties/y integer".to_string()),
        "{lines:?}"
    );
}

#[test]
fn a_one_of_branch_that_is_a_ref_uses_the_branch_path() {
    let schema = json!({"oneOf": [{"$ref": "#/$defs/A"}]});
    let defs = json!({"A": {"type": "object", "properties": {"z": {"type": "boolean"}}}});
    let mut lines = Vec::new();
    walk("t", "output", "", &schema, &defs, 0, &mut lines);
    assert!(lines.contains(&"t output /oneOf/0/properties/z boolean".to_string()), "{lines:?}");
}

#[test]
fn any_of_branches_stay_on_the_parent_path() {
    let schema = json!({"type": "object", "properties": {"n": {"anyOf": [
        {"type": "object", "properties": {"m": {"type": "string"}}},
        {"type": "null"}
    ]}}});
    let mut lines = Vec::new();
    walk("t", "input", "", &schema, &json!({}), 0, &mut lines);
    assert!(lines.contains(&"t input /properties/n/properties/m string".to_string()), "{lines:?}");
}
```

The expected strings assume the existing line format
`<tool> <side> <path> <type>`. If the module's helper or the line format
differs, keep each assertion's meaning and adjust only the spelling to the real
format.

- [ ] **Step 2: Run the tests and check that they fail.** Run:
      `cargo test -p xtask freeze::mcp` Expected: the three `oneOf` tests FAIL
      on their assertions, because the lines carry no `/oneOf/<n>`.
      `any_of_branches_stay_on_the_parent_path` PASSES already.

- [ ] **Step 3: Index the `oneOf` branches.** In `walk`, replace the single loop
      over `branches(node)` with two loops. Keep `branches()`, because `type_of`
      uses it.

```rust
for branch in node
    .get("anyOf")
    .and_then(Value::as_array)
    .into_iter()
    .flatten()
{
    walk(tool, side, pointer, branch, defs, depth + 1, lines);
}
for (index, branch) in node
    .get("oneOf")
    .and_then(Value::as_array)
    .into_iter()
    .flatten()
    .enumerate()
{
    let path = format!("{pointer}/oneOf/{index}");
    walk(tool, side, &path, branch, defs, depth + 1, lines);
}
```

Update the `walk` doc comment and the module doc: `anyOf` branches are walked on
the parent path, and each `oneOf` branch on `<pointer>/oneOf/<n>`. Reordering
`oneOf` branches is therefore a break.

- [ ] **Step 4: Run the tests and check that they pass.** Run:
      `cargo test -p xtask freeze::mcp` Expected: PASS.

- [ ] **Step 5: Regenerate the dump and check the change.** Run:
      `cargo xtask freeze update`, then `git diff --stat freeze/`. Expected:
      only `freeze/mcp.txt` changes. In it, only the `ridl_explain
      output`
      lines change: lines under the old pointer become lines under `/oneOf/0` or
      `/oneOf/1`, and the root `ridl_explain output` lines (the type line and
      the root keywords) stay. Read `git diff freeze/mcp.txt` and confirm that
      no other tool's lines moved.

- [ ] **Step 6: Document the format.** Edit the files named under **Files**.
      Read the diff of each docs file before committing.

- [ ] **Step 7: Run the checks.** Run `cargo test -p xtask`,
      `just freeze-check`, `just check`, `just link-check`. Expected: PASS.

- [ ] **Step 8: Commit.**

```bash
git add xtask/src/freeze/mcp.rs freeze/mcp.txt freeze/README.md docs/decisions/ADR-0029-stability-policy.md
git commit -m "fix(xtask): dump each oneOf branch of an MCP schema under its own path"
```

### Task 2: A `required` list of surfaces, checked at release (#868)

**Files:**

- Modify: `xtask/src/freeze.rs` (`Row`/`RawFile`/`parse_surfaces`, about lines
  29-82; `Options` and `parse_options`; `USAGE`; `check`, about lines 362-428;
  `read_rows`)
- Modify: `freeze/surfaces.toml` (add `required = []` and its comment)
- Modify: `justfile` (the `release` recipe, about line 2487)
- Modify: `freeze/README.md` ("Approve a break"),
  `docs/decisions/ADR-0029-stability-policy.md` (decision 4 and the Consequences
  bullet on the two-PR procedure), `CONTRIBUTING.md` (the "Changing a frozen
  surface" item on approving a break)
- Test: the unit tests in `xtask/src/freeze.rs` and `xtask/tests/freeze.rs`

**Interfaces:**

- Consumes: nothing from Task 1.
- Produces:
  - `pub struct Surfaces { pub required: Vec<String>, pub rows: Vec<Row> }`
  - `pub fn parse_surfaces(toml: &str) -> Result<Surfaces, String>`, which
    replaces the `Vec<Row>` return. Update every caller (`read_rows` and the
    tests); keep `read_rows`'s name, or rename it `read_surfaces` and update its
    callers.
  - `Options` gains `pub release: bool`, set by the flag `--release`.
  - `pub fn rows_missing(required: &[String], rows: &[Row]) -> Vec<String>`: the
    names in `required` with no row, in `required` order.

Behaviour:

- `required` is an optional top-level array of strings; absent means empty.
- A name listed twice fails parsing: "surfaces.toml: `<name>` appears twice in
  `required`".
- In `check`, a name in `required` that no dumper or API checker has is a
  failure: "surfaces.toml lists `<name>` in `required` but no dumper or API
  checker has that name". It fails with or without `--release`.
- For each name in `rows_missing`:
  - without `--release`, `check` prints to stdout
    `freeze: surface`<name>`is required but has no row; add it back with a
    baseline before the next release`
    and does not fail;
  - with `--release`, it is a failure:
    `surface`<name>`is required but has no row: a release cannot leave a
    frozen surface without a baseline (see freeze/README.md, "Approve a
    break")`.
- The success line stays `freeze: <n> surface(s) checked.`

- [ ] **Step 1: Write the failing unit tests** in the tests module of
      `xtask/src/freeze.rs`:

```rust
#[test]
fn parse_surfaces_reads_the_required_list() {
    let surfaces = parse_surfaces(
        "required = [\"keywords\", \"cli\"]\n\n[[surface]]\nname = \"keywords\"\nbaseline = \"v0.10.0\"\n",
    )
    .unwrap();
    assert_eq!(surfaces.required, vec!["keywords".to_string(), "cli".to_string()]);
    assert_eq!(surfaces.rows.len(), 1);
}

#[test]
fn parse_surfaces_defaults_required_to_empty() {
    assert!(parse_surfaces("").unwrap().required.is_empty());
}

#[test]
fn parse_surfaces_rejects_a_name_listed_twice_in_required() {
    let error = parse_surfaces("required = [\"cli\", \"cli\"]\n").unwrap_err();
    assert!(error.contains("`cli` appears twice in `required`"), "{error}");
}

#[test]
fn rows_missing_lists_required_surfaces_without_a_row() {
    let rows = vec![Row { surface: "keywords".into(), baseline: "v0.10.0".into() }];
    let required = vec!["keywords".to_string(), "cli".to_string(), "mcp".to_string()];
    assert_eq!(rows_missing(&required, &rows), vec!["cli".to_string(), "mcp".to_string()]);
}
```

Also update the existing `parse_surfaces` tests to read `.rows`.

- [ ] **Step 2: Write the failing behaviour tests** in `xtask/tests/freeze.rs`.
      Reuse the helper that the existing `check_fails_*` tests use to build a
      temporary git repository with a `freeze/` tree and run the binary through
      `env!("CARGO_BIN_EXE_xtask")`. Read those tests first, and follow their
      argument pattern (`--fixtures-only` and the root argument) exactly. Add:
  - `check_notes_a_required_surface_without_a_row`: `surfaces.toml` holds
    `required = ["<a fixture surface name>"]` and no rows. `freeze check` exits
    0, and stdout contains `is required but has no row`.
  - `release_check_fails_on_a_required_surface_without_a_row`: the same tree;
    `freeze check --release` exits non-zero, and stderr contains
    `a release
    cannot leave a frozen surface without a baseline`.
  - `release_check_passes_with_an_empty_required_list`: `required = []`, no
    rows; `freeze check --release` exits 0.
  - `check_fails_on_an_unknown_name_in_required`: `required = ["no-such"]`;
    `freeze check` exits non-zero, and stderr contains
    `lists`no-such`in`required`but no dumper`.

Use a surface name that the fixture-only dumpers produce. The existing tests
show which ones.

- [ ] **Step 3: Run the tests and check that they fail.** Run:
      `cargo test -p xtask` Expected: the new tests FAIL. The unit tests fail to
      compile until `Surfaces` exists; that is expected for this step only. Then
      add the `Surfaces` struct and `rows_missing` returning `Vec::new()`, and
      confirm that `rows_missing_lists_required_surfaces_without_a_row` fails on
      its assertion.

- [ ] **Step 4: Implement.**

```rust
/// The contents of `freeze/surfaces.toml`.
#[derive(Debug, PartialEq, Eq)]
pub struct Surfaces {
    /// The surfaces that must have a row when a release is made.
    pub required: Vec<String>,
    pub rows: Vec<Row>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawFile {
    #[serde(default)]
    required: Vec<String>,
    #[serde(default)]
    surface: Vec<RawRow>,
}

/// The names in `required` that have no row, in `required` order.
pub fn rows_missing(required: &[String], rows: &[Row]) -> Vec<String> {
    required
        .iter()
        .filter(|name| !rows.iter().any(|row| &row.surface == *name))
        .cloned()
        .collect()
}
```

In `parse_surfaces`, after the rows loop, reject a duplicate in `file.required`
with the message from **Behaviour**, then return
`Ok(Surfaces { required: file.required, rows })`.

In `check`, after the loop that rejects rows without a dumper, add:

```rust
    for name in &surfaces.required {
        if !dumps.iter().any(|(dump, _)| dump == name)
            && !apis.iter().any(|(api, _)| api.name == *name)
        {
            failures.push(format!(
                "surfaces.toml lists `{name}` in `required` but no dumper or API checker has \
that name"
            ));
        }
    }
    for name in rows_missing(&surfaces.required, &surfaces.rows) {
        if options.release {
            failures.push(format!(
                "surface `{name}` is required but has no row: a release cannot leave a frozen \
surface without a baseline (see freeze/README.md, \"Approve a break\")"
            ));
        } else {
            println!(
                "freeze: surface `{name}` is required but has no row; add it back with a \
baseline before the next release"
            );
        }
    }
```

Adapt the `rows` variable: `let surfaces = read_rows(&options.root)?;` and use
`surfaces.rows` where `rows` was used. Add `--release` to `parse_options` and to
`USAGE` ("--release fail when a surface listed in `required` has no row").
`update` ignores `--release`.

- [ ] **Step 5: Run the tests and check that they pass.** Run:
      `cargo test -p xtask` Expected: PASS.

- [ ] **Step 6: Wire the release recipe and the file.** In `justfile`, make the
      `release` recipe run the release check before the bump:

```just
release: toolchain-check
    cargo run --locked --package xtask -- freeze check --release
    git std bump
```

Keep any comment above the recipe, and add one line saying the release check
runs first. In `freeze/surfaces.toml`, add above the example:

```toml
# `required` lists the surfaces that must have a row when a release is made.
# `just freeze-check` prints a notice for a listed surface without a row; the
# `release` recipe fails on it.
required = []
```

The key must come before any `[[surface]]` table, because TOML reads a key after
a table header as part of that table.

- [ ] **Step 7: Document the guard.** In `freeze/README.md` ("Approve a break"),
      ADR-0029 decision 4 and its Consequences bullet, and `CONTRIBUTING.md`:
      the break PR removes the row and leaves the name in `required`. Until the
      follow-up PR adds the row back, `just
      freeze-check` prints a notice,
      and `just release` fails. Remove any sentence that says nothing guards the
      re-add. Read each docs diff before committing.

- [ ] **Step 8: Run the checks.** Run `cargo test -p xtask`,
      `just freeze-check`, `just gate-parity`, `just check`, `just link-check`,
      `just doc-path-check`, `just story-id-check`, and
      `cargo run --locked --package xtask -- freeze check --release`. Expected:
      PASS. The last one passes because `required` is empty.

- [ ] **Step 9: Commit**, as two commits:

```bash
git add xtask/src/freeze.rs xtask/tests/freeze.rs freeze/surfaces.toml justfile
git commit -m "feat(xtask): list the surfaces that must have a row, and check them at release"
git add freeze/README.md docs/decisions/ADR-0029-stability-policy.md CONTRIBUTING.md
git commit -m "docs(adr): state the release check that guards an approved break"
```

## Alternatives considered

- **#870, index `anyOf` branches too.** Every `anyOf` in `tools.json` is a
  nullable wrapper with one object branch. Indexing them would move every
  nullable property under `/anyOf/0` and make reordering `[T, null]` a break,
  with no gain.
- **#868, fail every `freeze-check` run while a required surface has no row.**
  This would turn `main` red between the two pull requests of an approved break
  and block unrelated pull requests. The risk the issue names is a release that
  ships a frozen surface without a baseline, and the release check covers
  exactly that.
- **#868, a CI job that counts merges since a row was removed.** Needs state
  outside the tree and is harder to test than a list in `surfaces.toml`.
