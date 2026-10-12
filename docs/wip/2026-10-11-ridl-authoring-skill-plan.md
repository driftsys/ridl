# The ridl-authoring skill and its evals — implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** ship the `ridl-authoring` skill (entry, rules, typl, ridl and rsdl
references) to Claude Code, Codex and Copilot, with gates that keep it
consistent with the compiler, and an eval runner that measures it with and
without the skill on Claude and GPT.

**Architecture:** the skill is Markdown under `skills/ridl-authoring/`. Three
tests in `crates/ridl/tests/` are its gates. A new `cargo xtask eval` subcommand
copies a task's workspace, runs `claude -p` or `codex exec` in an isolated
configuration, scores the run mechanically with the `ridl` binary, then has the
other vendor's model mark the rubric. The VS Code extension and the `editor-v*`
release workflow ship the skill.

**Tech Stack:** Rust (xtask: `serde`, `serde_json`, `toml`; no new dependency),
`pulldown-cmark` and `regex` in tests, the `claude` CLI (2.1.287 probed) and the
`codex` CLI (0.162.1 probed), the VS Code `chatSkills` and `chatInstructions`
contribution points.

**Spec:**
[`2026-10-11-ridl-authoring-skill-design.md`](2026-10-11-ridl-authoring-skill-design.md).
Read it first; this plan does not repeat its decisions (R-S-1 to R-S-4, D-1 to
D-9).

## Global Constraints

- The skill directory is `skills/ridl-authoring/`; its frontmatter `name` is
  `ridl-authoring` and equals the directory name.
- The skill's version is the workspace version in `Cargo.toml`
  (`workspace.package.version`), kept equal by `.git-std.toml`
  `[[version_files]]` and `crates/ridlc/tests/prose_version_pins.rs`.
- Every `typl`/`ridl`/`rsdl` fence under `skills/` follows the book's fence
  rules (`CONTRIBUTING.md`, "Writing examples in the book").
- Every diagnostic code written under `skills/` is a line of
  `freeze/diagnostics.txt`.
- CI never calls a model and needs no API key (D-8).
- An eval run's configuration contains only the `ridl` MCP server and, for a
  "with" run, the skill; no user skill, rule, memory or hook reaches the agent.
- An eval run's workspace is outside any git repository.
- Mechanical checks gate the rubric: a run that fails one scores zero on the
  rubric (spec 6.3).
- Rubric weights: **must** and **must not** count 2, **should** counts 1.
- Judge: GPT judges Claude runs; Claude judges GPT runs. The judge sees the
  prompt, the rubric, the final workspace and the agent's final message only.
- Judge check: 20 sampled runs labelled by the maintainer; agreement below 80%
  blocks the release record.
- Release record: `--runs 3`, 12 tasks, both hosts, with and without the skill.
- Release gate: per host, the mechanical pass rate with the skill is at least
  the rate without it, and the mean rubric score with the skill is at least the
  score without it.
- `engines.vscode` becomes `^1.109.0` and `@types/vscode` becomes `1.109.0` (VS
  Code 1.109.0 is the first release whose `chatPromptFilesContribution.ts`
  registers `chatSkills`; 1.108.2 does not).
- Prose is plain and literal English; no story ids in `skills/`, `crates/`,
  `xtask/` or `docs/book/` (`just story-id-check`).
- Commit scopes: `skills` (added in Task 3), `xtask`, `repo` (for `evals/`),
  `editors`, `ci`, `docs`, `adr`.

## Review Focus

1. **A "with" run in which the host never loads the skill.** Expected: the
   runner detects it and fails the run as a setup error, not as a skill result.
   Pinned in Task 5 (`skill_was_loaded`).
2. **A "without" run that still sees a skill or rule from the user's home
   directory or from a parent `AGENTS.md`/`CLAUDE.md`.** Expected: none reaches
   it. Pinned in Task 4 (the temporary workspace root is outside the repository,
   and the host home is a fresh directory) and checked in Task 5's probe.
3. **A judge reply that skips an item, repeats one, or is not JSON.** Expected:
   the run's rubric score is an error, recorded as such, never a partial score.
   Pinned in Task 6.
4. **A skill section whose reference link names a heading that does not exist.**
   Expected: the gate fails. `just link-check` checks only the file part. Pinned
   in Task 3.
5. **An agent that edits files outside its workspace or deletes the workspace.**
   Expected: the mechanical check fails with a clear message (`ridl check` error
   on a missing manifest) instead of a panic. Pinned in Task 5
   (`score_missing_workspace`).

---

## Pull requests

| PR | Tasks | Waits for                                                    |
| -- | ----- | ------------------------------------------------------------ |
| 1  | —     | this spec and this plan (docs only)                          |
| 2  | 1–9   | PR 1 merged                                                  |
| 3  | 10–13 | PR 2 merged, and lane L's gates on (Task 10 start condition) |

Each PR runs `/review` passes 1 and 2 before Sebastien merges it. Delegated
rulings go in the PR body as `R-S-<n>` (lanes plan rule 8).

---

### Task 1: A task may carry its own starting workspace

**Files:**

- Modify: `crates/ridl/tests/eval_corpus.rs` (`validate_task`, around l.191-266;
  the fixture helper `validate_task_fixture`, around l.338)
- Modify: `evals/README.md` (the task-format paragraph)

**Interfaces:**

- Produces: a task directory may hold `start/`, a workspace (a directory with
  `ridl.toml`). Rules: a `design` task may have `start/` and must omit `corpus`;
  a `review` or `evolve` task has exactly one of `corpus` and `start/`. Task 4
  reads this: the starting workspace of a task is `evals/corpus/<corpus>`, else
  `evals/tasks/<id>/start`, else empty.
- `xtask/src/calibrate.rs` `rubric_items` reads only `review` tasks and requires
  `corpus`; no review task gets `start/`, so it is unchanged.

- [ ] **Step 1: Write the failing tests**

Add to `crates/ridl/tests/eval_corpus.rs`, after the existing
`validate_task_fixture` tests:

```rust
fn task_fixture_dir(toml_text: &str, with_start: bool) -> std::path::PathBuf {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("the current time follows the epoch")
        .as_nanos();
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let sequence = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let id = format!("ridl-start-{}-{unique}-{sequence}", std::process::id());
    let dir = std::env::temp_dir().join(&id);
    std::fs::create_dir(&dir).expect("create the task fixture");
    std::fs::write(dir.join("task.toml"), toml_text.replace("{id}", &id))
        .expect("write task.toml");
    std::fs::write(dir.join("prompt.md"), "Do the task.\n").expect("write prompt.md");
    std::fs::write(dir.join("rubric.md"), "1. **must** do the task.\n").expect("write rubric.md");
    if with_start {
        std::fs::create_dir(dir.join("start")).expect("create start/");
        std::fs::write(
            dir.join("start/ridl.toml"),
            "[package]\nname = \"p\"\nversion = \"1.0.0\"\n",
        )
        .expect("write start/ridl.toml");
    }
    dir
}

const EVOLVE_NO_CORPUS: &str = "id = \"{id}\"\nkind = \"evolve\"\ntitle = \"t\"\n\n[expect]\ncompiles = true\nlints = []\ndiff = \"compatible\"\n";
const DESIGN: &str = "id = \"{id}\"\nkind = \"design\"\ntitle = \"t\"\n\n[expect]\ncompiles = true\nlints = []\n";

#[test]
fn an_evolve_task_may_start_from_its_own_workspace() {
    let dir = task_fixture_dir(EVOLVE_NO_CORPUS, true);
    assert!(validate_task(&dir).is_ok(), "{:?}", validate_task(&dir));
}

#[test]
fn an_evolve_task_needs_a_corpus_or_a_start_workspace() {
    let dir = task_fixture_dir(EVOLVE_NO_CORPUS, false);
    assert_eq!(
        validate_task(&dir).unwrap_err(),
        "a review or evolve task names exactly one of corpus and start/"
    );
}

#[test]
fn a_task_cannot_name_both_a_corpus_and_a_start_workspace() {
    let corpus = corpus_dirs()[0].file_name().unwrap().to_str().unwrap().to_owned();
    let text = EVOLVE_NO_CORPUS.replace("kind = \"evolve\"\n", &format!("kind = \"evolve\"\ncorpus = {corpus:?}\n"));
    let dir = task_fixture_dir(&text, true);
    assert_eq!(
        validate_task(&dir).unwrap_err(),
        "a review or evolve task names exactly one of corpus and start/"
    );
}

#[test]
fn a_design_task_may_start_from_its_own_workspace() {
    let dir = task_fixture_dir(DESIGN, true);
    assert!(validate_task(&dir).is_ok(), "{:?}", validate_task(&dir));
}

#[test]
fn a_start_directory_must_be_a_workspace() {
    let dir = task_fixture_dir(DESIGN, true);
    std::fs::remove_file(dir.join("start/ridl.toml")).unwrap();
    assert_eq!(validate_task(&dir).unwrap_err(), "start/ must hold a ridl.toml");
}

#[test]
fn every_start_workspace_checks_without_an_error() {
    let tasks = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../evals/tasks");
    for entry in std::fs::read_dir(&tasks).expect("read evals/tasks") {
        let start = entry.expect("read a task entry").path().join("start");
        if !start.is_dir() {
            continue;
        }
        let output = Command::new(env!("CARGO_BIN_EXE_ridl"))
            .args(["check", "--format", "json"])
            .arg(&start)
            .output()
            .expect("run ridl check");
        let report: serde_json::Value =
            serde_json::from_slice(&output.stdout).expect("ridl check prints JSON");
        let errors: Vec<_> = report
            .as_array()
            .expect("a JSON array")
            .iter()
            .filter(|d| d["severity"] == "error")
            .collect();
        assert!(errors.is_empty(), "{}: {errors:?}", start.display());
    }
}
```

- [ ] **Step 2: Run the tests and confirm they fail**

Run: `cargo test --locked -p ridl-cli --test eval_corpus start` Expected:
`an_evolve_task_may_start_from_its_own_workspace` and
`a_design_task_may_start_from_its_own_workspace` fail; the two "exactly one"
tests fail on the message (the current message is "corpus must be a string");
`a_start_directory_must_be_a_workspace` fails because a design task with
`start/` is not inspected yet; `every_start_workspace_checks_without_an_error`
passes (no `start/` exists yet).

- [ ] **Step 3: Implement**

In `validate_task`, replace the `if kind == "design" { … } else { … }` block
with:

```rust
let start = dir.join("start");
let has_start = start.is_dir();
if has_start {
    require(start.join("ridl.toml").is_file(), "start/ must hold a ridl.toml")?;
}
if kind == "design" {
    require(task.get("corpus").is_none(), "design task must omit corpus")?;
} else {
    let corpus = task.get("corpus");
    require(
        corpus.is_some() != has_start,
        "a review or evolve task names exactly one of corpus and start/",
    )?;
    if let Some(corpus) = corpus {
        let corpus = corpus.as_str().ok_or("corpus must be a string")?;
        require(
            corpus_dirs()
                .iter()
                .any(|p| p.file_name().and_then(|n| n.to_str()) == Some(corpus)),
            "corpus must name an existing workspace",
        )?;
    }
}
```

In `evals/README.md`, extend the task-format paragraph with one sentence after
"uses one directory per stable task ID, with `task.toml`, `prompt.md` and
`rubric.md`.":

```markdown
A task may also hold `start/`, a workspace the agent starts from; a design task
without it starts from an empty directory, and a review or evolve task names
exactly one of a `corpus` workspace and `start/`.
```

- [ ] **Step 4: Run the tests and confirm they pass**

Run: `cargo test --locked -p ridl-cli --test eval_corpus` Expected: all pass,
including the existing task tests.

- [ ] **Step 5: Commit**

```bash
git add crates/ridl/tests/eval_corpus.rs evals/README.md
git commit -m "feat(repo): let an eval task carry its own starting workspace"
```

---

### Task 2: Two rsdl eval tasks on the cabin workspace

**Files:**

- Create: `evals/tasks/design-0004/{task.toml,prompt.md,rubric.md}`,
  `evals/tasks/design-0004/start/{ridl.toml,cabin.ridl,system.rsdl}`
- Create: `evals/tasks/evolve-0003/{task.toml,prompt.md,rubric.md}`,
  `evals/tasks/evolve-0003/start/{ridl.toml,cabin.ridl,system.rsdl}`

**Interfaces:**

- Consumes: Task 1's `start/` rule.
- Produces: two tasks the runner (Task 4) picks up by directory.

- [ ] **Step 1: Create the starting workspaces**

For each of the two tasks, copy `examples/cabin/cabin.ridl` and
`examples/cabin/system.rsdl` into `start/`, then:

- delete the leading `//` comment block of `cabin.ridl` (it names a test file in
  this repository, which an agent must not see) and the two lines
  `// ANCHOR: schema` and `// ANCHOR_END: schema`;
- write `start/ridl.toml` as:

```toml
[package]
name = "veh.cabin"
version = "1.0.0"
```

Run: `cargo run --locked -p ridl-cli -- check evals/tasks/design-0004/start` and
the same for `evolve-0003`. Expected: no Error.

- [ ] **Step 2: Write `design-0004`**

`task.toml`:

```toml
id = "design-0004"
kind = "design"
title = "Add a seat-heating subsystem to the cabin system"

[expect]
compiles = true
lints = []
```

`prompt.md`:

```markdown
The workspace holds the cabin of a vehicle: a package with two interfaces, a
service, and a system that deploys three components on two machines. Add seat
heating. The driver panel sets a heating level for each front seat and shows
the level each seat has reached. The heating controller reports a fault when a
seat heater fails. The controller runs on the same machine as the climate
component. Keep every existing declaration as it is. Provide a workspace that
checks without an error, and explain where you placed the new interface
boundary and why.
```

`rubric.md`:

```markdown
1. **must** add a new interface for seat heating rather than add its
   interactions to `Cabin` or `Horn`.

2. **must** model the level each seat has reached as a signal and the heater
   fault as an event.

3. **must** model setting a level as a command whose parameters identify the
   seat and the level.

4. **must** add a component that offers a service for the new interface, list
   it in the `Vehicle` system, and place it on the `Hpc` machine of the `Bench`
   deployment.

5. **must** make the `Panel` component require the new interface.

6. **should** bound the heating level and the seat identity with typl types
   that carry a range or an enum, not a bare primitive.

7. **must not** change, reorder or remove an existing declaration, field,
   interaction or component.
```

- [ ] **Step 3: Write `evolve-0003`**

`task.toml`:

```toml
id = "evolve-0003"
kind = "evolve"
title = "Let the telemetry component read the horn compatibly"

[expect]
compiles = true
lints = []
diff = "compatible"
```

`prompt.md`:

```markdown
The workspace holds the cabin of a vehicle and the system that deploys it. The
telemetry component on the gateway machine must now also record the horn's
health. Change the workspace so that it does, without breaking any existing
consumer of the cabin package. Show that the change is compatible, and explain
which part of the change a deployed consumer can observe.
```

`rubric.md`:

```markdown
1. **must** make the `Telemetry` component require the `Horn` interface.

2. **must** keep every existing type, interface, interaction, service,
   component, system and deployment declaration unchanged apart from that
   requirement.

3. **must** show a `ridl diff` result, or the result of the `ridl_diff` tool,
   that classifies the change as compatible.

4. **should** explain that the change adds a consumer of an existing service
   and does not change any interface.

5. **must not** add a second `Horn` provider or move a component to another
   machine.
```

- [ ] **Step 4: Check each task with a reference solution**

In the scratchpad (not in the repository), copy each `start/`, apply a solution
by hand that meets every **must** item, and run:

```bash
ridl check --format json <solution>
ridl diff --format json evals/tasks/evolve-0003/start <solution>   # evolve only
```

Expected: no Error; for `evolve-0003` the verdict is `"compatible"`. If the
verdict is not `compatible`, change the prompt and rubric so the intended change
is compatible, and repeat. Keep the solutions in the scratchpad for the
reviewer; do not commit them (the runner copies only `start/`, and a committed
answer would sit next to the task).

- [ ] **Step 5: Run the format tests**

Run: `cargo test --locked -p ridl-cli --test eval_corpus` Expected: all pass,
including `every_start_workspace_checks_without_an_error`.

- [ ] **Step 6: Independent review and maintainer approval (blocking)**

ADR-0027 requires an independent review of a new task and the maintainer's
approval of the task set. Dispatch one reviewer (a fresh subagent) with the two
task directories, the two scratchpad solutions and the ADR-0027 task rules; it
reports whether each prompt states the request without rubric hints, whether
each rubric item is one sentence, verifiable and marked, and whether the
solutions meet every **must**. Then ask Sebastien to approve the two tasks. Do
not start Task 5's probe on these tasks before he approves.

- [ ] **Step 7: Commit**

```bash
git add evals/tasks/design-0004 evals/tasks/evolve-0003
git commit -m "feat(repo): add two rsdl eval tasks on the cabin workspace"
```

---

### Task 3: The skill's skeleton and its three gates

**Files:**

- Create: `skills/ridl-authoring/SKILL.md`, `rules.md`, `typl.md`, `ridl.md`,
  `rsdl.md`
- Create: `crates/ridl/tests/skill_gates.rs`
- Modify: `crates/ridl/tests/book_examples.rs` (add one `#[test]` and a
  `skill_root()` helper next to `book_root()`, around l.909)
- Modify: `.git-std.toml` (add the `skills` scope; add one `[[version_files]]`
  entry)
- Modify: `crates/ridlc/tests/prose_version_pins.rs` (`EXPECTED_ENTRIES` grows
  from 3 to 4)
- Modify: `CONTRIBUTING.md` (one paragraph under "Writing examples in the book")

**Interfaces:**

- Produces: the five files with headings only, except one compiled fence in
  `SKILL.md`. Tasks 10 and 11 fill them.
- Produces: `skill_root() -> PathBuf` in `book_examples.rs` and the test
  `skill_examples_compile`.

- [ ] **Step 1: Write the skeleton**

`skills/ridl-authoring/SKILL.md`:

````markdown
---
name: ridl-authoring
description: >-
  Write, review and evolve RIDL-family contracts: typl types, ridl interfaces
  and rsdl systems in a workspace with a ridl.toml. Use when creating or
  editing .typl, .ridl or .rsdl files or a ridl.toml, when turning a
  requirement into an interface or a type, or when reviewing such a change.
metadata:
  version: "0.9.0"
---

# Authoring RIDL

## The model in one minute

```typl
package skill.intro

/// Cabin air temperature, in degrees Celsius.
type Temperature : Cel [-40.0..85.0 step 0.5]
```

## The verify loop

## Which reference to read
````

`rules.md`:

```markdown
---
applyTo: "**/*.typl,**/*.ridl,**/*.rsdl,**/ridl.toml"
description: Constraints that every RIDL-family source must meet.
---

# Rules
```

`typl.md`, `ridl.md`, `rsdl.md`: one line each, `# typl`, `# ridl`, `# rsdl`.

Set `metadata.version` to the current `workspace.package.version` of
`Cargo.toml` (0.9.0 when this plan was written). Run
`cargo run --locked -p ridl-cli -- check` on a scratch copy of the fence to
confirm the unit `Cel` and the range are accepted; if not, change the fence to
one the checker accepts.

- [ ] **Step 2: Write the failing fence test**

In `crates/ridl/tests/book_examples.rs`, after `book_root()`:

```rust
/// The root of `skills/ridl-authoring/`, whose fences follow the book's rules
/// and form a workspace of their own.
fn skill_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../skills/ridl-authoring")
        .canonicalize()
        .expect("skills/ridl-authoring must exist")
}

#[test]
fn skill_examples_compile() {
    match verify_book(&skill_root()) {
        Ok(count) => assert!(count > 0, "the skill holds no verified example"),
        Err(report) => panic!("{report}"),
    }
}
```

Run:
`cargo test --locked -p ridl-cli --test book_examples skill_examples_compile`
Expected: PASS if Step 1's fence is valid. Then break the fence (write
`type Temperature : Cel` with no range) and run again. Expected: FAIL naming
`SKILL.md`. Restore the fence.

- [ ] **Step 3: Write the failing gate tests**

`crates/ridl/tests/skill_gates.rs`:

```rust
//! The gates that keep `skills/ridl-authoring/` consistent with the compiler:
//! every diagnostic code it cites is in the catalogue dump, every section of a
//! reference file links the heading of the language reference it summarises,
//! and the frontmatter names the skill after its directory.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

const ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");

fn skill_dir() -> PathBuf {
    Path::new(ROOT).join("skills/ridl-authoring")
}

fn skill_files() -> Vec<PathBuf> {
    let mut files: Vec<_> = std::fs::read_dir(skill_dir())
        .expect("read skills/ridl-authoring")
        .map(|e| e.expect("read an entry").path())
        .filter(|p| p.extension().is_some_and(|x| x == "md"))
        .collect();
    files.sort();
    files
}

/// The codes `freeze/diagnostics.txt` lists: the first word of each line.
fn catalogue_codes() -> BTreeSet<String> {
    std::fs::read_to_string(Path::new(ROOT).join("freeze/diagnostics.txt"))
        .expect("read freeze/diagnostics.txt")
        .lines()
        .filter_map(|line| line.split_whitespace().next().map(str::to_owned))
        .collect()
}

fn cited_codes(text: &str) -> BTreeSet<String> {
    regex::Regex::new(r"\b(?:TYPL|RIDL|RSDL|FORM|MANI)-\d{3}\b")
        .expect("a valid pattern")
        .find_iter(text)
        .map(|m| m.as_str().to_owned())
        .collect()
}

fn unknown_codes(text: &str, catalogue: &BTreeSet<String>) -> Vec<String> {
    cited_codes(text).into_iter().filter(|c| !catalogue.contains(c)).collect()
}

#[test]
fn every_cited_code_is_in_the_catalogue() {
    let catalogue = catalogue_codes();
    let mut failures = Vec::new();
    for file in skill_files() {
        let text = std::fs::read_to_string(&file).expect("read a skill file");
        for code in unknown_codes(&text, &catalogue) {
            failures.push(format!("{}: {code}", file.display()));
        }
    }
    assert!(failures.is_empty(), "codes not in freeze/diagnostics.txt:\n{}", failures.join("\n"));
}

#[test]
fn an_unknown_code_is_reported() {
    let catalogue: BTreeSet<String> = ["TYPL-101".to_owned()].into();
    assert_eq!(unknown_codes("TYPL-101 and RIDL-999", &catalogue), ["RIDL-999"]);
}

/// The GitHub-style anchor of a heading: lowercase, punctuation other than
/// `-` and `_` removed, spaces replaced with `-`.
fn anchor(heading: &str) -> String {
    heading
        .trim()
        .to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == ' ' || *c == '-' || *c == '_')
        .map(|c| if c == ' ' { '-' } else { c })
        .collect()
}

fn anchors_of(markdown: &str) -> BTreeSet<String> {
    markdown
        .lines()
        .filter_map(|l| l.trim_start().strip_prefix('#'))
        .map(|l| anchor(l.trim_start_matches('#')))
        .collect()
}

/// For each `## ` section of a reference file: the link targets
/// `../../docs/specification/<file>#<anchor>` it holds.
fn section_links(text: &str) -> Vec<(String, Vec<(String, String)>)> {
    let link = regex::Regex::new(r"\]\((\.\./\.\./docs/specification/[^)#]+)#([^)]+)\)")
        .expect("a valid pattern");
    let mut sections: Vec<(String, Vec<(String, String)>)> = Vec::new();
    for line in text.lines() {
        if let Some(title) = line.strip_prefix("## ") {
            sections.push((title.to_owned(), Vec::new()));
        } else if let Some((_, links)) = sections.last_mut() {
            for c in link.captures_iter(line) {
                links.push((c[1].to_owned(), c[2].to_owned()));
            }
        }
    }
    sections
}

fn section_failures(file: &Path, text: &str) -> Vec<String> {
    let mut failures = Vec::new();
    for (title, links) in section_links(text) {
        if links.is_empty() {
            failures.push(format!("{}: section `{title}` links no language reference heading", file.display()));
        }
        for (target, fragment) in links {
            let path = file.parent().unwrap().join(&target);
            let Ok(reference) = std::fs::read_to_string(&path) else {
                failures.push(format!("{}: `{target}` does not exist", file.display()));
                continue;
            };
            if !anchors_of(&reference).contains(&fragment) {
                failures.push(format!("{}: `{target}#{fragment}` names no heading", file.display()));
            }
        }
    }
    failures
}

#[test]
fn every_reference_section_links_an_existing_heading() {
    let mut failures = Vec::new();
    for name in ["typl.md", "ridl.md", "rsdl.md"] {
        let file = skill_dir().join(name);
        let text = std::fs::read_to_string(&file).expect("read a reference file");
        failures.extend(section_failures(&file, &text));
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn a_link_to_a_missing_heading_is_reported() {
    let file = skill_dir().join("typl.md");
    let text = "## Ranges\n\nSee [ranges](../../docs/specification/typl-language-reference.md#no-such-heading).\n";
    let failures = section_failures(&file, text);
    assert_eq!(failures.len(), 1, "{failures:?}");
    assert!(failures[0].contains("names no heading"));
}

#[test]
fn a_section_without_a_link_is_reported() {
    let file = skill_dir().join("typl.md");
    let failures = section_failures(&file, "## Ranges\n\nNo link here.\n");
    assert_eq!(failures.len(), 1, "{failures:?}");
    assert!(failures[0].contains("links no language reference heading"));
}

#[test]
fn the_frontmatter_names_the_skill_after_its_directory() {
    let text = std::fs::read_to_string(skill_dir().join("SKILL.md")).expect("read SKILL.md");
    let frontmatter = text
        .strip_prefix("---\n")
        .and_then(|rest| rest.split_once("\n---\n"))
        .map(|(front, _)| front)
        .expect("SKILL.md opens with a frontmatter block");
    assert!(frontmatter.lines().any(|l| l == "name: ridl-authoring"), "{frontmatter}");
}
```

Add `regex.workspace = true` to `[dev-dependencies]` of `crates/ridl/Cargo.toml`
if it is not already there (check with `grep -n regex crates/ridl/Cargo.toml`).

Run: `cargo test --locked -p ridl-cli --test skill_gates` Expected: the two
negative tests and the frontmatter test pass; the two whole-skill tests pass (no
code and no `##` section yet). Then add a temporary line `TYPL-999` to
`rules.md` and a section `## Ranges` with no link to `typl.md`, run again, and
confirm both whole-skill tests fail naming the file; remove both lines.

The anchor rule must match the one `prim lint` applies to in-document anchors,
which already checks the tables of contents of the language references. Pin it
against them:

```rust
#[test]
fn the_anchor_rule_matches_the_references_own_tables_of_contents() {
    let fragment = regex::Regex::new(r"\]\(#([^)]+)\)").expect("a valid pattern");
    for name in ["typl", "ridl", "rsdl"] {
        let path = Path::new(ROOT).join(format!("docs/specification/{name}-language-reference.md"));
        let text = std::fs::read_to_string(&path).expect("read a language reference");
        let anchors = anchors_of(&text);
        let fragments: Vec<_> = fragment.captures_iter(&text).map(|c| c[1].to_owned()).collect();
        assert!(!fragments.is_empty(), "{name}: no in-document link to test against");
        for f in fragments {
            assert!(anchors.contains(&f), "{name}: #{f} is not an anchor by this rule");
        }
    }
}
```

If it fails, fix `anchor` until it passes (for example, `### 1.2 The
\`.typl\`
profile`must give`12-the-typl-profile`).

- [ ] **Step 4: Version pin and scope**

In `.git-std.toml`, add `"skills",` to `scopes` after `"repo",` with the comment
`# the agent skill under skills/ (ADR-0005 Layer A)`, and append:

```toml
[[version_files]]
path = "skills/ridl-authoring/SKILL.md"
regex = '(?m)^  version: "([^"]+)"'
```

In `crates/ridlc/tests/prose_version_pins.rs`, change `EXPECTED_ENTRIES` to
`[(&str, &str); 4]` and append:

```rust
(
    "skills/ridl-authoring/SKILL.md",
    r#"(?m)^  version: "([^"]+)""#,
),
```

Run: `cargo test --locked -p ridlc --test prose_version_pins` Expected: PASS.
Change the version in `SKILL.md` to `0.0.1`, run again, expected FAIL; restore
it.

- [ ] **Step 5: CONTRIBUTING**

Under "Writing examples in the book" in `CONTRIBUTING.md`, add:

```markdown
The fences in `skills/ridl-authoring/` follow the same rules and are compiled
by the test `skill_examples_compile`, as one workspace separate from the book.
`crates/ridl/tests/skill_gates.rs` also checks that every diagnostic code the
skill cites is listed in `freeze/diagnostics.txt`, and that each section of a
reference file links an existing heading of a language reference.
```

- [ ] **Step 6: Run the gates and commit**

Run:
`cargo test --locked -p ridl-cli --test book_examples --test skill_gates && cargo test --locked -p ridlc --test prose_version_pins && just link-check && just doc-path-check && prim lint skills`
Expected: all pass.

```bash
git add skills crates/ridl/tests/skill_gates.rs crates/ridl/tests/book_examples.rs crates/ridl/Cargo.toml .git-std.toml crates/ridlc/tests/prose_version_pins.rs CONTRIBUTING.md Cargo.lock
git commit -m "feat(skills): add the ridl-authoring skill skeleton and its gates"
```

---

### Task 4: `cargo xtask eval` — tasks, isolated hosts and runs

**Files:**

- Create: `xtask/src/eval.rs` (dispatch and arguments)
- Create: `xtask/src/eval/task.rs`, `xtask/src/eval/host.rs`
- Create: `evals/runner.toml`
- Modify: `xtask/src/main.rs` (add `mod eval;` and the `Some("eval")` arm, next
  to `calibrate`; add `eval` to the usage line)
- Create: `xtask/tests/eval_cli.rs`
- Modify: `.gitignore` (add `evals/results/*/transcripts/`)

**Interfaces:**

- Consumes: Task 1's starting-workspace rule.
- Produces (used by Tasks 5-7):

```rust
// xtask/src/eval/task.rs
pub enum Kind { Design, Evolve, Review }
pub struct Task {
    pub id: String,
    pub kind: Kind,
    pub dir: PathBuf,            // evals/tasks/<id>
    pub start: Option<PathBuf>,  // evals/corpus/<corpus> or <dir>/start
    pub expect_lints: Vec<String>,
    pub expect_diff: Option<String>,
}
pub fn load_tasks(root: &Path, filter: &[String]) -> Result<Vec<Task>, String>;

// xtask/src/eval/host.rs
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Host { Claude, Codex }
impl Host {
    pub fn name(self) -> &'static str;          // "claude" | "codex"
    pub fn other(self) -> Host;                  // the judge's host
    pub fn binary(self) -> String;               // RIDL_EVAL_CLAUDE / RIDL_EVAL_CODEX, else "claude" / "codex"
}
pub struct Models { pub claude: String, pub codex: String }
pub fn load_models(root: &Path) -> Result<Models, String>;
pub struct RunDirs { pub home: PathBuf, pub workspace: PathBuf, pub transcript: PathBuf, pub last_message: PathBuf }
pub fn prepare_run(base: &Path, task: &Task, host: Host, with_skill: bool, skill: &Path) -> Result<RunDirs, String>;
pub struct Invocation { pub program: String, pub args: Vec<String>, pub env: Vec<(String, String)>, pub cwd: PathBuf, pub stdin: String }
pub fn agent_invocation(host: Host, model: &str, dirs: &RunDirs, prompt: &str) -> Invocation;
pub fn run(invocation: &Invocation, stdout_to: &Path) -> Result<std::process::ExitStatus, String>;
```

- [ ] **Step 1: `evals/runner.toml`**

```toml
# The models the eval runner pins (docs/wip spec section 6.2). Each result file
# records the model and the host CLI version it ran with. The judge of a run
# uses the other host's model.
[claude]
model = "claude-opus-5-5"

[codex]
model = "gpt-6-sol"
```

Ask Sebastien to confirm the two model ids before Task 12's release record;
record his answer as a delegated ruling if he delegates it.

- [ ] **Step 2: Write the failing unit tests**

At the bottom of `xtask/src/eval/host.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn dirs(base: &Path) -> RunDirs {
        RunDirs {
            home: base.join("home"),
            workspace: base.join("ws"),
            transcript: base.join("t.jsonl"),
            last_message: base.join("last.md"),
        }
    }

    #[test]
    fn claude_runs_bare_with_only_the_ridl_server() {
        let inv = agent_invocation(Host::Claude, "m", &dirs(Path::new("/r")), "p");
        assert!(inv.args.contains(&"--bare".to_owned()));
        assert!(inv.args.contains(&"--strict-mcp-config".to_owned()));
        assert!(inv.args.windows(2).any(|w| w == ["--model", "m"]));
        assert!(inv.env.contains(&("CLAUDE_CONFIG_DIR".to_owned(), "/r/home".to_owned())));
        assert_eq!(inv.cwd, Path::new("/r/ws"));
        assert_eq!(inv.stdin, "p");
    }

    #[test]
    fn codex_runs_in_its_own_home_without_a_git_check() {
        let inv = agent_invocation(Host::Codex, "m", &dirs(Path::new("/r")), "p");
        assert!(inv.args.contains(&"--skip-git-repo-check".to_owned()));
        assert!(inv.args.contains(&"--json".to_owned()));
        assert!(inv.args.windows(2).any(|w| w == ["-m", "m"]));
        assert!(inv.env.contains(&("CODEX_HOME".to_owned(), "/r/home".to_owned())));
    }

    #[test]
    fn the_judge_of_a_host_is_the_other_host() {
        assert_eq!(Host::Claude.other(), Host::Codex);
        assert_eq!(Host::Codex.other(), Host::Claude);
    }
}
```

At the bottom of `xtask/src/eval/task.rs`, tests that build a temporary root
with `evals/tasks/a-0001/{task.toml,prompt.md,rubric.md,start/ridl.toml}` and
`evals/corpus/c/ridl.toml` and assert:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn root() -> PathBuf {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!(
            "xtask-eval-task-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        let write = |rel: &str, text: &str| {
            let path = dir.join(rel);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        };
        write("evals/corpus/c/ridl.toml", "[workspace]\nmembers = []\n");
        write("evals/tasks/review-0001/task.toml", "id = \"review-0001\"\nkind = \"review\"\ncorpus = \"c\"\ntitle = \"t\"\n[expect]\ncompiles = true\nlints = [\"duplicate-shape\"]\n");
        write("evals/tasks/evolve-0009/task.toml", "id = \"evolve-0009\"\nkind = \"evolve\"\ntitle = \"t\"\n[expect]\ncompiles = true\nlints = []\ndiff = \"compatible\"\n");
        write("evals/tasks/evolve-0009/start/ridl.toml", "[package]\nname = \"p\"\nversion = \"1.0.0\"\n");
        write("evals/tasks/design-0009/task.toml", "id = \"design-0009\"\nkind = \"design\"\ntitle = \"t\"\n[expect]\ncompiles = true\nlints = []\n");
        dir
    }

    #[test]
    fn a_task_starts_from_its_corpus_its_start_directory_or_nothing() {
        let root = root();
        let tasks = load_tasks(&root, &[]).unwrap();
        let start = |id: &str| tasks.iter().find(|t| t.id == id).unwrap().start.clone();
        assert_eq!(start("review-0001"), Some(root.join("evals/corpus/c")));
        assert_eq!(start("evolve-0009"), Some(root.join("evals/tasks/evolve-0009/start")));
        assert_eq!(start("design-0009"), None);
    }

    #[test]
    fn the_filter_selects_tasks_by_id_and_rejects_an_unknown_one() {
        let root = root();
        let ids: Vec<_> = load_tasks(&root, &["design-0009".to_owned()]).unwrap().into_iter().map(|t| t.id).collect();
        assert_eq!(ids, ["design-0009"]);
        assert_eq!(
            load_tasks(&root, &["nope-0001".to_owned()]).unwrap_err(),
            "unknown task: nope-0001"
        );
    }
}
```

Run: `cargo test --locked -p xtask eval::` Expected: compile failure (the
modules do not exist).

- [ ] **Step 3: Implement `task.rs`**

```rust
//! Eval tasks under `evals/tasks/`: their kind, starting workspace and
//! mechanical expectations.

use std::path::{Path, PathBuf};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Design,
    Evolve,
    Review,
}

#[derive(Debug)]
pub struct Task {
    pub id: String,
    pub kind: Kind,
    pub dir: PathBuf,
    pub start: Option<PathBuf>,
    pub expect_lints: Vec<String>,
    pub expect_diff: Option<String>,
}

pub fn load_tasks(root: &Path, filter: &[String]) -> Result<Vec<Task>, String> {
    let tasks_dir = root.join("evals/tasks");
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(&tasks_dir)
        .map_err(|e| format!("read {}: {e}", tasks_dir.display()))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.join("task.toml").is_file())
        .collect();
    dirs.sort();
    let mut tasks = Vec::new();
    for dir in dirs {
        let task = load_task(root, &dir)?;
        if filter.is_empty() || filter.contains(&task.id) {
            tasks.push(task);
        }
    }
    for wanted in filter {
        if !tasks.iter().any(|t| &t.id == wanted) {
            return Err(format!("unknown task: {wanted}"));
        }
    }
    Ok(tasks)
}

fn load_task(root: &Path, dir: &Path) -> Result<Task, String> {
    let text = std::fs::read_to_string(dir.join("task.toml")).map_err(|e| e.to_string())?;
    let value: toml::Value = toml::from_str(&text).map_err(|e| format!("{}: {e}", dir.display()))?;
    let field = |name: &str| value.get(name).and_then(toml::Value::as_str);
    let id = field("id").ok_or("task lacks id")?.to_owned();
    let kind = match field("kind") {
        Some("design") => Kind::Design,
        Some("evolve") => Kind::Evolve,
        Some("review") => Kind::Review,
        other => return Err(format!("{id}: unknown kind {other:?}")),
    };
    let start = match field("corpus") {
        Some(corpus) => Some(root.join("evals/corpus").join(corpus)),
        None if dir.join("start").is_dir() => Some(dir.join("start")),
        None => None,
    };
    let expect = value.get("expect").ok_or_else(|| format!("{id}: no [expect]"))?;
    let expect_lints = expect
        .get("lints")
        .and_then(toml::Value::as_array)
        .ok_or_else(|| format!("{id}: no expect.lints"))?
        .iter()
        .filter_map(|v| v.as_str().map(str::to_owned))
        .collect();
    let expect_diff = expect.get("diff").and_then(toml::Value::as_str).map(str::to_owned);
    Ok(Task { id, kind, dir: dir.to_owned(), start, expect_lints, expect_diff })
}
```

- [ ] **Step 4: Implement `host.rs`**

```rust
//! The two agent hosts, their isolated configuration, and how a run invokes
//! them. A run gets a fresh host home, so no user skill, rule, memory or hook
//! reaches the agent, and a workspace outside any git repository, so no
//! `AGENTS.md` or `CLAUDE.md` above it is read.

use super::task::Task;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Host {
    Claude,
    Codex,
}

impl Host {
    pub fn name(self) -> &'static str {
        match self {
            Host::Claude => "claude",
            Host::Codex => "codex",
        }
    }

    pub fn other(self) -> Host {
        match self {
            Host::Claude => Host::Codex,
            Host::Codex => Host::Claude,
        }
    }

    /// The host's CLI. A test replaces it with `RIDL_EVAL_CLAUDE` or
    /// `RIDL_EVAL_CODEX`.
    pub fn binary(self) -> String {
        let (var, default) = match self {
            Host::Claude => ("RIDL_EVAL_CLAUDE", "claude"),
            Host::Codex => ("RIDL_EVAL_CODEX", "codex"),
        };
        std::env::var(var).unwrap_or_else(|_| default.to_owned())
    }

    pub fn model(self, models: &Models) -> &str {
        match self {
            Host::Claude => &models.claude,
            Host::Codex => &models.codex,
        }
    }
}

pub struct Models {
    pub claude: String,
    pub codex: String,
}

pub fn load_models(root: &Path) -> Result<Models, String> {
    let path = root.join("evals/runner.toml");
    let text = std::fs::read_to_string(&path).map_err(|e| format!("read {}: {e}", path.display()))?;
    let value: toml::Value = toml::from_str(&text).map_err(|e| e.to_string())?;
    let model = |host: &str| {
        value
            .get(host)
            .and_then(|t| t.get("model"))
            .and_then(toml::Value::as_str)
            .map(str::to_owned)
            .ok_or_else(|| format!("evals/runner.toml lacks {host}.model"))
    };
    Ok(Models { claude: model("claude")?, codex: model("codex")? })
}

pub struct RunDirs {
    pub home: PathBuf,
    pub workspace: PathBuf,
    pub transcript: PathBuf,
    pub last_message: PathBuf,
}

/// The `ridl` binary the MCP server and the scoring use. A test replaces it
/// with `RIDL_EVAL_RIDL`.
pub fn ridl_binary() -> String {
    std::env::var("RIDL_EVAL_RIDL").unwrap_or_else(|_| "ridl".to_owned())
}

/// Creates `<base>/home` and `<base>/ws`, copies the task's starting workspace
/// into `ws`, writes the host's MCP configuration, and for a "with" run copies
/// the skill to the host's skill directory.
pub fn prepare_run(base: &Path, task: &Task, host: Host, with_skill: bool, skill: &Path) -> Result<RunDirs, String> {
    let dirs = RunDirs {
        home: base.join("home"),
        workspace: base.join("ws"),
        transcript: base.join("transcript.jsonl"),
        last_message: base.join("last-message.md"),
    };
    std::fs::create_dir_all(&dirs.home).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&dirs.workspace).map_err(|e| e.to_string())?;
    if let Some(start) = &task.start {
        copy_tree(start, &dirs.workspace)?;
    }
    let ridl = ridl_binary();
    match host {
        Host::Claude => {
            let config = serde_json::json!({ "mcpServers": { "ridl": { "command": ridl, "args": ["mcp"] } } });
            std::fs::write(base.join("mcp.json"), config.to_string()).map_err(|e| e.to_string())?;
            if with_skill {
                copy_tree(skill, &dirs.workspace.join(".claude/skills/ridl-authoring"))?;
            }
        }
        Host::Codex => {
            let config = format!("[mcp_servers.ridl]\ncommand = {ridl:?}\nargs = [\"mcp\"]\n");
            std::fs::write(dirs.home.join("config.toml"), config).map_err(|e| e.to_string())?;
            if with_skill {
                copy_tree(skill, &dirs.home.join("skills/ridl-authoring"))?;
            }
        }
    }
    Ok(dirs)
}

pub struct Invocation {
    pub program: String,
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
    pub cwd: PathBuf,
    pub stdin: String,
}

pub fn agent_invocation(host: Host, model: &str, dirs: &RunDirs, prompt: &str) -> Invocation {
    let s = |v: &str| v.to_owned();
    let path = |p: &Path| p.display().to_string();
    let base = dirs.home.parent().expect("a run base directory");
    let (args, env) = match host {
        Host::Claude => (
            vec![
                s("-p"), s("--bare"), s("--model"), s(model),
                s("--mcp-config"), path(&base.join("mcp.json")), s("--strict-mcp-config"),
                s("--permission-mode"), s("bypassPermissions"),
                s("--output-format"), s("stream-json"), s("--verbose"),
                s("--no-session-persistence"),
            ],
            vec![(s("CLAUDE_CONFIG_DIR"), path(&dirs.home))],
        ),
        Host::Codex => (
            vec![
                s("exec"), s("--json"), s("--skip-git-repo-check"),
                s("-m"), s(model), s("-s"), s("workspace-write"),
                s("-c"), s("approval_policy=\"never\""),
                s("-C"), path(&dirs.workspace),
                s("-o"), path(&dirs.last_message), s("-"),
            ],
            vec![(s("CODEX_HOME"), path(&dirs.home))],
        ),
    };
    Invocation { program: host.binary(), args, env, cwd: dirs.workspace.clone(), stdin: prompt.to_owned() }
}

/// Runs an invocation with its stdin, and writes its stdout to `stdout_to`.
pub fn run(invocation: &Invocation, stdout_to: &Path) -> Result<ExitStatus, String> {
    use std::io::Write;
    let out = std::fs::File::create(stdout_to).map_err(|e| e.to_string())?;
    let mut child = Command::new(&invocation.program)
        .args(&invocation.args)
        .envs(invocation.env.iter().map(|(k, v)| (k, v)))
        .current_dir(&invocation.cwd)
        .stdin(Stdio::piped())
        .stdout(out)
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|e| format!("spawn {}: {e}", invocation.program))?;
    child
        .stdin
        .take()
        .expect("a piped stdin")
        .write_all(invocation.stdin.as_bytes())
        .map_err(|e| e.to_string())?;
    child.wait().map_err(|e| e.to_string())
}

fn copy_tree(from: &Path, to: &Path) -> Result<(), String> {
    std::fs::create_dir_all(to).map_err(|e| e.to_string())?;
    for entry in std::fs::read_dir(from).map_err(|e| format!("read {}: {e}", from.display()))? {
        let entry = entry.map_err(|e| e.to_string())?;
        let target = to.join(entry.file_name());
        if entry.file_type().map_err(|e| e.to_string())?.is_dir() {
            copy_tree(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), &target).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}
```

Authentication: `--bare` reads only `ANTHROPIC_API_KEY`, and a fresh
`CODEX_HOME` has no login. The runner (Step 5) fails before the first run when
`ANTHROPIC_API_KEY` or `OPENAI_API_KEY` is unset for a host it will call, and
for Codex runs `codex login --with-api-key` with `CODEX_HOME` set to the run's
home and the key on stdin, once per run home.

- [ ] **Step 5: Implement `eval.rs` (arguments and the run loop)**

```rust
//! `cargo xtask eval`: runs the eval tasks with and without the
//! ridl-authoring skill on each host, scores each run, and writes the results
//! under `evals/results/<date>/`.

mod host;
mod task;

use host::Host;
use std::path::{Path, PathBuf};

pub struct Options {
    pub tasks: Vec<String>,
    pub hosts: Vec<Host>,
    pub runs: u32,
    pub skill: Vec<bool>, // true = with the skill
    pub out: Option<PathBuf>,
}

pub fn parse(args: &[String]) -> Result<Options, String> {
    let mut options = Options { tasks: Vec::new(), hosts: Vec::new(), runs: 1, skill: vec![true, false], out: None };
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        let mut value = || iter.next().cloned().ok_or_else(|| format!("{arg} needs a value"));
        match arg.as_str() {
            "--task" => options.tasks.push(value()?),
            "--host" => options.hosts.push(match value()?.as_str() {
                "claude" => Host::Claude,
                "codex" => Host::Codex,
                other => return Err(format!("unknown host: {other}")),
            }),
            "--runs" => options.runs = value()?.parse().map_err(|_| "--runs takes a positive number".to_owned())?,
            "--skill" => options.skill = match value()?.as_str() {
                "with" => vec![true],
                "without" => vec![false],
                "both" => vec![true, false],
                other => return Err(format!("--skill takes with, without or both, not {other}")),
            },
            "--out" => options.out = Some(PathBuf::from(value()?)),
            other => return Err(format!("unknown argument: {other}")),
        }
    }
    if options.runs == 0 {
        return Err("--runs takes a positive number".to_owned());
    }
    if options.hosts.is_empty() {
        options.hosts = vec![Host::Claude, Host::Codex];
    }
    Ok(options)
}

pub fn run(args: &[String]) -> Result<(), String> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().expect("the repository root").to_owned();
    let options = parse(args)?;
    let tasks = task::load_tasks(&root, &options.tasks)?;
    let models = host::load_models(&root)?;
    check_keys()?;
    let out = options.out.clone().unwrap_or_else(|| root.join("evals/results").join(today()));
    let skill = root.join("skills/ridl-authoring");
    for task in &tasks {
        for &host in &options.hosts {
            for &with_skill in &options.skill {
                for n in 1..=options.runs {
                    let label = format!("{}-{}-{n}", host.name(), if with_skill { "with" } else { "without" });
                    let base = scratch_dir(&task.id, &label)?;
                    let dirs = host::prepare_run(&base, task, host, with_skill, &skill)?;
                    if host == Host::Codex {
                        codex_login(&dirs.home)?;
                    }
                    let prompt = std::fs::read_to_string(task.dir.join("prompt.md")).map_err(|e| e.to_string())?;
                    let invocation = host::agent_invocation(host, host.model(&models), &dirs, &prompt);
                    let status = host::run(&invocation, &dirs.transcript)?;
                    // Task 5 adds scoring and the result file here.
                    eprintln!("eval: {} {label}: agent exited {status}", task.id);
                    let transcripts = out.join("transcripts").join(&task.id);
                    std::fs::create_dir_all(&transcripts).map_err(|e| e.to_string())?;
                    std::fs::copy(&dirs.transcript, transcripts.join(format!("{label}.jsonl"))).map_err(|e| e.to_string())?;
                }
            }
        }
    }
    Ok(())
}

/// The judge of each host is the other host, so both keys are needed for
/// any run; a fake host in a test sets both.
fn check_keys() -> Result<(), String> {
    for var in ["ANTHROPIC_API_KEY", "OPENAI_API_KEY"] {
        if std::env::var(var).map_or(true, |v| v.is_empty()) {
            return Err(format!("{var} is not set; an eval run reads no stored login"));
        }
    }
    Ok(())
}

fn codex_login(home: &Path) -> Result<(), String> {
    let invocation = host::Invocation {
        program: Host::Codex.binary(),
        args: vec!["login".into(), "--with-api-key".into()],
        env: vec![("CODEX_HOME".into(), home.display().to_string())],
        cwd: home.to_owned(),
        stdin: std::env::var("OPENAI_API_KEY").unwrap_or_default(),
    };
    let status = host::run(&invocation, &home.join("login.log"))?;
    if status.success() { Ok(()) } else { Err(format!("codex login failed: {status}")) }
}

/// A fresh directory under the system temporary directory, outside any git
/// repository.
fn scratch_dir(task: &str, label: &str) -> Result<PathBuf, String> {
    let dir = std::env::temp_dir().join(format!("ridl-eval-{}-{task}-{label}", std::process::id()));
    if dir.exists() {
        std::fs::remove_dir_all(&dir).map_err(|e| e.to_string())?;
    }
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

/// Today's date as YYYY-MM-DD, from `date`, so xtask needs no date crate.
fn today() -> String {
    let out = std::process::Command::new("date").arg("+%Y-%m-%d").output().expect("run date");
    String::from_utf8_lossy(&out.stdout).trim().to_owned()
}
```

Wire it into `xtask/src/main.rs` exactly as `calibrate` is wired:

```rust
mod eval;
// …
        Some("eval") => match eval::run(&std::env::args().skip(2).collect::<Vec<_>>()) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("eval: {e}");
                ExitCode::from(2)
            }
        },
```

and change the usage text to
`cargo xtask <codegen|descriptor-codegen|calibrate|freeze|eval>`.

Add to `.gitignore`:

```text
# Raw eval transcripts stay local; the result files are committed.
evals/results/*/transcripts/
```

- [ ] **Step 6: An end-to-end test with fake hosts**

`xtask/tests/eval_cli.rs` (unix only). It writes three shell scripts into a
temporary directory and points `RIDL_EVAL_CLAUDE`, `RIDL_EVAL_CODEX` and
`RIDL_EVAL_RIDL` at them:

```rust
//! `cargo xtask eval` over fake host CLIs: no model is called.
#![cfg(unix)]

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

fn script(dir: &Path, name: &str, body: &str) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    path
}

fn fakes() -> (PathBuf, PathBuf, PathBuf, PathBuf) {
    let dir = std::env::temp_dir().join(format!("xtask-eval-fakes-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    // The fake agent records its arguments and environment, then prints one
    // tool-use line naming ridl_check.
    let claude = script(&dir, "claude", r#"cat > /dev/null; echo "$@" > "$PWD/../args.txt"; env > "$PWD/../env.txt"; echo '{"type":"assistant","message":{"content":[{"type":"tool_use","name":"mcp__ridl__ridl_check","input":{}}]}}'"#);
    let codex = script(&dir, "codex", r#"cat > /dev/null; echo "$@" > "$CODEX_HOME/../args.txt"; echo '{"type":"item.completed","item":{"type":"mcp_tool_call","server":"ridl","tool":"ridl_check"}}'"#);
    let ridl = script(&dir, "ridl", r#"case "$1" in check) echo '[]';; diff) echo '{"verdict":"compatible","changes":[]}';; esac"#);
    (dir, claude, codex, ridl)
}

#[test]
fn a_run_copies_its_transcript_and_isolates_the_host() {
    let (dir, claude, codex, ridl) = fakes();
    let out = dir.join("results");
    let status = Command::new(env!("CARGO_BIN_EXE_xtask"))
        .args(["eval", "--task", "design-0001", "--host", "claude", "--skill", "without", "--out"])
        .arg(&out)
        .env("RIDL_EVAL_CLAUDE", &claude)
        .env("RIDL_EVAL_CODEX", &codex)
        .env("RIDL_EVAL_RIDL", &ridl)
        .env("ANTHROPIC_API_KEY", "fake")
        .env("OPENAI_API_KEY", "fake")
        .status()
        .unwrap();
    assert!(status.success());
    let transcript = std::fs::read_to_string(out.join("transcripts/design-0001/claude-without-1.jsonl")).unwrap();
    assert!(transcript.contains("mcp__ridl__ridl_check"));
}

#[test]
fn an_unset_key_stops_the_run_before_any_host_is_called() {
    let (dir, claude, codex, ridl) = fakes();
    let output = Command::new(env!("CARGO_BIN_EXE_xtask"))
        .args(["eval", "--task", "design-0001", "--host", "claude", "--out"])
        .arg(dir.join("results-nokey"))
        .env("RIDL_EVAL_CLAUDE", &claude)
        .env("RIDL_EVAL_CODEX", &codex)
        .env("RIDL_EVAL_RIDL", &ridl)
        .env_remove("ANTHROPIC_API_KEY")
        .env_remove("OPENAI_API_KEY")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("is not set"));
}

#[test]
fn an_unknown_task_is_an_error() {
    let output = Command::new(env!("CARGO_BIN_EXE_xtask"))
        .args(["eval", "--task", "nope-0001"])
        .env("ANTHROPIC_API_KEY", "fake")
        .env("OPENAI_API_KEY", "fake")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("unknown task: nope-0001"));
}
```

Run:
`cargo test --locked -p xtask eval && cargo test --locked -p xtask --test eval_cli`
Expected: all pass.

- [ ] **Step 7: Commit**

```bash
git add xtask/src/eval.rs xtask/src/eval xtask/src/main.rs xtask/tests/eval_cli.rs evals/runner.toml .gitignore
git commit -m "feat(xtask): run eval tasks on isolated agent hosts"
```

---

### Task 5: Probe the real hosts, then score runs mechanically

**Files:**

- Create: `xtask/src/eval/score.rs`
- Create:
  `xtask/tests/eval-fixtures/{claude-with.jsonl,claude-without.jsonl,codex-with.jsonl,codex-without.jsonl}`
- Modify: `xtask/src/eval.rs` (call scoring after each run)
- Modify: `xtask/src/eval/host.rs` (only if the probe shows a flag or path is
  wrong)

**Interfaces:**

- Consumes: `RunDirs`, `Task`, `Host`, `host::ridl_binary()` from Task 4.
- Produces (used by Tasks 6-7):

```rust
// xtask/src/eval/score.rs
pub const DESIGN_LINTS: [&str; 3] = ["inconsistent-unit", "duplicate-shape", "low-cohesion-interface"];
pub struct Mechanical {
    pub no_error: bool,
    pub lints_match: bool,
    pub diff_matches: Option<bool>,    // Some only for an evolve task
    pub verify_loop: bool,
    pub notes: Vec<String>,            // why a check failed
}
impl Mechanical { pub fn passed(&self) -> bool; }
pub fn used_ridl_check(transcript: &str) -> bool;
pub fn skill_was_loaded(host: Host, transcript: &str) -> bool;
pub fn errors_and_lints(check_json: &str) -> Result<(usize, BTreeSet<String>), String>;
pub fn score_mechanical(task: &Task, dirs: &RunDirs, transcript: &str) -> Mechanical;
```

- [ ] **Step 1: Probe (needs both API keys; Sebastien runs it or approves the
      spend)**

This step calls real models: 4 agent runs on `design-0001`. Ask Sebastien before
running it. Run:

```bash
cargo xtask eval --task design-0001 --runs 1 --out "$SCRATCH/probe"
```

For each of the four transcripts, record in the PR body:

1. Does the transcript show the `ridl` MCP server connected? (Claude: the
   `system`/`init` line lists `mcp_servers` with `ridl` and a `connected`
   status; Codex: an MCP startup event naming `ridl`.)
2. The exact JSON shape of a `ridl_check` tool call.
3. For a "with" run: the evidence that the host loaded `ridl-authoring` (Claude:
   the `init` line's skill list, or a `Skill` tool call naming it; Codex: a
   skill event or the skill file read). For a "without" run: no mention of it.
4. That no user rule, skill or memory appears (search the transcript for a
   string unique to `~/.claude/rules/` or `~/.codex/`, such as
   `international-english`).

If `--bare` stops Claude from discovering the project skill in
`.claude/skills/`, remove `--bare` from `agent_invocation`, keep
`CLAUDE_CONFIG_DIR`, add `--setting-sources project`, and repeat the probe. If
the Codex skill is not found in `$CODEX_HOME/skills/`, try
`<workspace>/.agents/skills/` and repeat. Update the unit tests of Task 4 to the
flags that work.

Copy one short excerpt of each transcript (the init line, one `ridl_check` call,
and the skill evidence; no model reasoning) into
`xtask/tests/eval-fixtures/<host>-<with|without>.jsonl`.

- [ ] **Step 2: Write the failing tests**

At the bottom of `score.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> String {
        std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/eval-fixtures").join(name),
        )
        .unwrap()
    }

    #[test]
    fn the_verify_loop_is_found_in_real_transcripts() {
        for name in ["claude-with.jsonl", "claude-without.jsonl", "codex-with.jsonl", "codex-without.jsonl"] {
            assert!(used_ridl_check(&fixture(name)), "{name}");
        }
    }

    #[test]
    fn a_transcript_without_a_ridl_check_call_fails_the_verify_loop() {
        assert!(!used_ridl_check(r#"{"type":"assistant","message":{"content":[{"type":"text","text":"I ran ridl_check"}]}}"#));
    }

    #[test]
    fn skill_loading_is_told_apart() {
        assert!(skill_was_loaded(Host::Claude, &fixture("claude-with.jsonl")));
        assert!(!skill_was_loaded(Host::Claude, &fixture("claude-without.jsonl")));
        assert!(skill_was_loaded(Host::Codex, &fixture("codex-with.jsonl")));
        assert!(!skill_was_loaded(Host::Codex, &fixture("codex-without.jsonl")));
    }

    #[test]
    fn errors_are_counted_and_only_design_lints_are_kept() {
        let json = r#"[
          {"code":"TYPL-101","severity":"error","message":"m","span":null,"labels":[],"fixes":[]},
          {"code":"TYPL-406","severity":"warning","lint":"missing-docs","message":"m","span":null,"labels":[],"fixes":[]},
          {"code":"TYPL-224","severity":"info","lint":"duplicate-shape","message":"m","span":null,"labels":[],"fixes":[]}
        ]"#;
        let (errors, lints) = errors_and_lints(json).unwrap();
        assert_eq!(errors, 1);
        assert_eq!(lints.into_iter().collect::<Vec<_>>(), ["duplicate-shape"]);
    }

    #[test]
    fn output_that_is_not_json_is_an_error() {
        assert!(errors_and_lints("error: no ridl.toml").is_err());
    }
}
```

and an integration test `score_missing_workspace` in `xtask/tests/eval_cli.rs`:
a fake `claude` that runs `rm -rf "$PWD"/*` before printing its line, run on
`evolve-0001` with `--host claude --skill without`; assert exit code 0 and that
the result file (Step 4) has `no_error = false` and a note containing
`ridl.toml`.

The fake `ridl` of Task 4 must then behave like the real one when the manifest
is missing: change its `check)` branch to
`[ -f "$3/ridl.toml" ] && echo '[]' || { echo "error: no ridl.toml in $3" >&2; exit 2; }`
(the argument order is `check --format json <dir>`).

Run: `cargo test --locked -p xtask eval::score` Expected: compile failure (no
`score.rs`).

- [ ] **Step 3: Implement `score.rs`**

```rust
//! Mechanical scoring of one run: the final workspace checks without an
//! error, its design lints match the task, an evolve task's diff verdict
//! matches, and the agent called `ridl_check`.

use super::host::{ridl_binary, Host, RunDirs};
use super::task::{Kind, Task};
use std::collections::BTreeSet;
use std::process::Command;

pub const DESIGN_LINTS: [&str; 3] = ["inconsistent-unit", "duplicate-shape", "low-cohesion-interface"];

pub struct Mechanical {
    pub no_error: bool,
    pub lints_match: bool,
    pub diff_matches: Option<bool>,
    pub verify_loop: bool,
    pub notes: Vec<String>,
}

impl Mechanical {
    pub fn passed(&self) -> bool {
        self.no_error && self.lints_match && self.diff_matches != Some(false) && self.verify_loop
    }
}

/// True when a JSON line of the transcript holds a tool call whose name is
/// `ridl_check` or ends with `__ridl_check` (Claude prefixes MCP tools with
/// `mcp__<server>__`). Text that only mentions the name does not count.
pub fn used_ridl_check(transcript: &str) -> bool {
    transcript
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .any(|value| has_tool_call(&value))
}

fn has_tool_call(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::Object(map) => {
            let names = ["name", "tool"].iter().filter_map(|k| map.get(*k).and_then(|v| v.as_str()));
            let is_call = map.get("type").and_then(|t| t.as_str()).is_some_and(|t| t == "tool_use" || t == "mcp_tool_call");
            (is_call && names.into_iter().any(|n| n == "ridl_check" || n.ends_with("__ridl_check")))
                || map.values().any(has_tool_call)
        }
        serde_json::Value::Array(items) => items.iter().any(has_tool_call),
        _ => false,
    }
}

/// Whether the transcript shows the host loaded the skill. The evidence each
/// host gives was recorded by the probe (Task 5 Step 1); match exactly that.
pub fn skill_was_loaded(host: Host, transcript: &str) -> bool {
    // Replace the body with the probe's evidence, for example for Claude a
    // `system`/`init` line whose `skills` array holds "ridl-authoring", and
    // for Codex the event the probe recorded.
    let _ = host;
    transcript.lines().any(|line| line.contains("\"ridl-authoring\""))
}

pub fn errors_and_lints(check_json: &str) -> Result<(usize, BTreeSet<String>), String> {
    let report: serde_json::Value =
        serde_json::from_str(check_json).map_err(|_| format!("ridl check printed no JSON: {}", check_json.trim()))?;
    let items = report.as_array().ok_or("ridl check JSON is not an array")?;
    let errors = items.iter().filter(|d| d["severity"] == "error").count();
    let lints = items
        .iter()
        .filter_map(|d| d["lint"].as_str())
        .filter(|l| DESIGN_LINTS.contains(l))
        .map(str::to_owned)
        .collect();
    Ok((errors, lints))
}

pub fn score_mechanical(task: &Task, dirs: &RunDirs, transcript: &str) -> Mechanical {
    let mut notes = Vec::new();
    let check = Command::new(ridl_binary())
        .args(["check", "--format", "json"])
        .arg(&dirs.workspace)
        .output();
    let (no_error, lints_match) = match check {
        Ok(out) => match errors_and_lints(&String::from_utf8_lossy(&out.stdout)) {
            Ok((errors, lints)) => {
                let expected: BTreeSet<String> = task.expect_lints.iter().cloned().collect();
                if errors > 0 {
                    notes.push(format!("ridl check reported {errors} error(s)"));
                }
                if lints != expected {
                    notes.push(format!("design lints {lints:?}, expected {expected:?}"));
                }
                (errors == 0, lints == expected)
            }
            Err(e) => {
                notes.push(format!("{e} {}", String::from_utf8_lossy(&out.stderr).trim()));
                (false, false)
            }
        },
        Err(e) => {
            notes.push(format!("ridl check did not run: {e}"));
            (false, false)
        }
    };
    let diff_matches = (task.kind == Kind::Evolve).then(|| {
        let start = task.start.as_ref().expect("an evolve task has a starting workspace");
        let out = Command::new(ridl_binary()).args(["diff", "--format", "json"]).arg(start).arg(&dirs.workspace).output();
        let verdict = out
            .ok()
            .and_then(|o| serde_json::from_slice::<serde_json::Value>(&o.stdout).ok())
            .and_then(|v| v["verdict"].as_str().map(str::to_owned));
        let matches = verdict.as_deref() == task.expect_diff.as_deref();
        if !matches {
            notes.push(format!("diff verdict {verdict:?}, expected {:?}", task.expect_diff));
        }
        matches
    });
    let verify_loop = used_ridl_check(transcript);
    if !verify_loop {
        notes.push("the agent made no ridl_check call".to_owned());
    }
    Mechanical { no_error, lints_match, diff_matches, verify_loop, notes }
}
```

Replace the body of `skill_was_loaded` with the exact evidence the probe
recorded, per host, so that a transcript that only mentions the name in text
does not count; the fixture tests pin it.

- [ ] **Step 4: Call scoring and write the result file**

In `eval.rs`, add `mod score;`, and replace the `// Task 5 adds scoring…` line
with:

```rust
let transcript = std::fs::read_to_string(&dirs.transcript).unwrap_or_default();
if with_skill && !score::skill_was_loaded(host, &transcript) {
    return Err(format!("{} {label}: the host did not load the skill; this is a setup error", task.id));
}
let mechanical = score::score_mechanical(task, &dirs, &transcript);
let result_dir = out.join(&task.id);
std::fs::create_dir_all(&result_dir).map_err(|e| e.to_string())?;
let result = toml::toml! {
    task = (task.id.clone())
    host = (host.name())
    model = (host.model(&models).to_owned())
    skill = with_skill
    run = (n as i64)
    agent_exit = (status.code().unwrap_or(-1) as i64)
    [mechanical]
    no_error = (mechanical.no_error)
    lints_match = (mechanical.lints_match)
    verify_loop = (mechanical.verify_loop)
    passed = (mechanical.passed())
    notes = (mechanical.notes.clone())
};
let mut result = toml::Value::Table(result);
if let Some(d) = mechanical.diff_matches {
    result["mechanical"].as_table_mut().unwrap().insert("diff_matches".into(), d.into());
}
std::fs::write(result_dir.join(format!("{label}.toml")), toml::to_string(&result).unwrap())
    .map_err(|e| e.to_string())?;
```

If the `toml!` macro is not available in the pinned `toml` version, build the
table with `toml::Table::new()` and `insert` calls instead.

- [ ] **Step 5: Run the tests**

Run:
`cargo test --locked -p xtask eval && cargo test --locked -p xtask --test eval_cli`
Expected: all pass. Mutate `used_ridl_check` to return `true` always and confirm
`a_transcript_without_a_ridl_check_call_fails_the_verify_loop` fails; restore.

- [ ] **Step 6: Commit**

```bash
git add xtask/src/eval.rs xtask/src/eval/score.rs xtask/src/eval/host.rs xtask/tests/eval_cli.rs xtask/tests/eval-fixtures
git commit -m "feat(xtask): score eval runs mechanically and detect the verify loop"
```

---

### Task 6: The cross-vendor judge and the summary

**Files:**

- Create: `xtask/src/eval/judge.rs`, `xtask/src/eval/report.rs`
- Modify: `xtask/src/eval.rs`

**Interfaces:**

- Consumes: `Mechanical`, `Task`, `Host`, `Models`, `RunDirs`.
- Produces:

```rust
// judge.rs
pub enum Marker { Must, Should, MustNot }
pub struct Item { pub n: u32, pub marker: Marker, pub text: String }
pub fn rubric_items(rubric: &str) -> Result<Vec<Item>, String>;
pub fn judge_prompt(prompt: &str, rubric: &str, workspace_files: &[(String, String)], final_message: &str) -> String;
pub fn parse_verdict(reply: &str, items: &[Item]) -> Result<Vec<bool>, String>;
pub fn weighted_score(items: &[Item], met: &[bool]) -> f64;
pub fn judge_invocation(judge: Host, model: &str, dir: &Path, prompt: &str) -> host::Invocation;
// report.rs
pub fn write_summary(out: &Path) -> Result<(), String>; // reads every <task>/<label>.toml under out
pub struct HostGate { pub host: String, pub pass_with: f64, pub pass_without: f64, pub rubric_with: f64, pub rubric_without: f64 }
impl HostGate { pub fn holds(&self) -> bool; }
pub fn gate(out: &Path) -> Result<Vec<HostGate>, String>;
```

- [ ] **Step 1: Write the failing tests**

In `judge.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    const RUBRIC: &str = "1. **must** a.\n\n2. **should** b.\n\n3. **must not** c.\n";

    #[test]
    fn rubric_items_keep_number_and_marker() {
        let items = rubric_items(RUBRIC).unwrap();
        assert_eq!(items.len(), 3);
        assert!(matches!(items[2].marker, Marker::MustNot));
    }

    #[test]
    fn the_score_weights_must_items_twice() {
        let items = rubric_items(RUBRIC).unwrap();
        assert_eq!(weighted_score(&items, &[true, false, true]), 4.0 / 5.0);
        assert_eq!(weighted_score(&items, &[false, true, false]), 1.0 / 5.0);
    }

    #[test]
    fn a_verdict_must_mark_every_item_once() {
        let items = rubric_items(RUBRIC).unwrap();
        let ok = r#"{"items":[{"n":1,"met":true},{"n":2,"met":false},{"n":3,"met":true}]}"#;
        assert_eq!(parse_verdict(ok, &items).unwrap(), [true, false, true]);
        let missing = r#"{"items":[{"n":1,"met":true},{"n":3,"met":true}]}"#;
        assert_eq!(parse_verdict(missing, &items).unwrap_err(), "the judge did not mark item 2");
        let repeated = r#"{"items":[{"n":1,"met":true},{"n":1,"met":false},{"n":2,"met":true},{"n":3,"met":true}]}"#;
        assert_eq!(parse_verdict(repeated, &items).unwrap_err(), "the judge marked item 1 twice");
        assert!(parse_verdict("I think it is fine.", &items).is_err());
    }

    #[test]
    fn the_judge_prompt_never_says_whether_the_skill_was_loaded() {
        let p = judge_prompt("Do X.", RUBRIC, &[("a.typl".into(), "package p\n".into())], "Done.");
        assert!(p.contains("Do X.") && p.contains("a.typl") && p.contains("Done."));
        assert!(!p.to_lowercase().contains("skill"));
    }

    #[test]
    fn a_claude_run_is_judged_by_codex_read_only() {
        let inv = judge_invocation(Host::Codex, "g", Path::new("/j"), "p");
        assert!(inv.args.windows(2).any(|w| w == ["-s", "read-only"]));
        assert!(inv.args.contains(&"--output-schema".to_owned()));
    }
}
```

In `report.rs`, a test that writes four result files under a temporary `out`
(claude with/without, passed and rubric values chosen so that "with" is lower on
the rubric) and asserts `gate(out)` returns a `HostGate` for `claude` whose
`holds()` is `false`, and that with equal values `holds()` is `true` (the gate
is "at least", not "greater than").

Run: `cargo test --locked -p xtask eval::judge eval::report` Expected: compile
failure.

- [ ] **Step 2: Implement `judge.rs`**

````rust
//! The rubric judge. A run on one host is judged by the other host's model,
//! which sees the prompt, the rubric, the final workspace and the agent's
//! final message, and nothing that says whether the skill was loaded.

use super::host::{self, Host};
use std::path::Path;

#[derive(Clone, Copy, Debug)]
pub enum Marker {
    Must,
    Should,
    MustNot,
}

pub struct Item {
    pub n: u32,
    pub marker: Marker,
    pub text: String,
}

/// The numbered items of a rubric. The format is the one
/// `crates/ridl/tests/eval_corpus.rs` validates: `N. **marker** text`, with
/// unindented numbers and indented continuation lines.
pub fn rubric_items(rubric: &str) -> Result<Vec<Item>, String> {
    let mut items: Vec<Item> = Vec::new();
    for line in rubric.lines() {
        let numbered = line.split_once(". ").and_then(|(n, rest)| n.parse::<u32>().ok().map(|n| (n, rest)));
        match numbered {
            Some((n, rest)) if !line.starts_with(' ') => {
                let (marker, text) = if let Some(t) = rest.strip_prefix("**must not** ") {
                    (Marker::MustNot, t)
                } else if let Some(t) = rest.strip_prefix("**must** ") {
                    (Marker::Must, t)
                } else if let Some(t) = rest.strip_prefix("**should** ") {
                    (Marker::Should, t)
                } else {
                    return Err(format!("rubric item {n} has no marker"));
                };
                items.push(Item { n, marker, text: text.to_owned() });
            }
            _ => {
                if let Some(last) = items.last_mut() {
                    if !line.trim().is_empty() {
                        last.text.push(' ');
                        last.text.push_str(line.trim());
                    }
                }
            }
        }
    }
    if items.is_empty() {
        return Err("the rubric has no item".to_owned());
    }
    Ok(items)
}

pub fn judge_prompt(prompt: &str, rubric: &str, workspace_files: &[(String, String)], final_message: &str) -> String {
    let mut text = String::from(
        "You grade one answer to a design request against a rubric. For each numbered rubric item, decide whether \
         the answer meets it. A \"must not\" item is met when the answer does not do the thing it names. Judge only \
         from the material below. Reply with JSON only: {\"items\":[{\"n\":<number>,\"met\":<true|false>,\"reason\":\"<one sentence>\"}]}, \
         one entry per rubric item.\n\n## The request\n\n",
    );
    text.push_str(prompt);
    text.push_str("\n\n## The rubric\n\n");
    text.push_str(rubric);
    text.push_str("\n\n## The final workspace\n");
    for (path, content) in workspace_files {
        text.push_str(&format!("\n### {path}\n\n```\n{content}\n```\n"));
    }
    text.push_str("\n## The answer's final message\n\n");
    text.push_str(final_message);
    text.push('\n');
    text
}

pub fn parse_verdict(reply: &str, items: &[Item]) -> Result<Vec<bool>, String> {
    let value: serde_json::Value =
        serde_json::from_str(reply.trim()).map_err(|_| "the judge reply is not JSON".to_owned())?;
    let entries = value["items"].as_array().ok_or("the judge reply has no items array")?;
    let mut met: Vec<Option<bool>> = vec![None; items.len()];
    for entry in entries {
        let n = entry["n"].as_u64().ok_or("a judge entry has no number")? as u32;
        let flag = entry["met"].as_bool().ok_or_else(|| format!("judge entry {n} has no met flag"))?;
        let index = items.iter().position(|i| i.n == n).ok_or_else(|| format!("the judge marked unknown item {n}"))?;
        if met[index].replace(flag).is_some() {
            return Err(format!("the judge marked item {n} twice"));
        }
    }
    met.iter()
        .zip(items)
        .map(|(m, item)| m.ok_or_else(|| format!("the judge did not mark item {}", item.n)))
        .collect()
}

pub fn weighted_score(items: &[Item], met: &[bool]) -> f64 {
    let weight = |m: Marker| match m {
        Marker::Must | Marker::MustNot => 2.0,
        Marker::Should => 1.0,
    };
    let total: f64 = items.iter().map(|i| weight(i.marker)).sum();
    let got: f64 = items.iter().zip(met).filter(|(_, m)| **m).map(|(i, _)| weight(i.marker)).sum();
    got / total
}

const SCHEMA: &str = r#"{"type":"object","required":["items"],"properties":{"items":{"type":"array","items":{"type":"object","required":["n","met","reason"],"properties":{"n":{"type":"integer"},"met":{"type":"boolean"},"reason":{"type":"string"}}}}}}"#;

/// The judge runs with no tool and no MCP server, in an empty directory.
pub fn judge_invocation(judge: Host, model: &str, dir: &Path, prompt: &str) -> host::Invocation {
    let s = |v: &str| v.to_owned();
    let (args, env) = match judge {
        Host::Claude => (
            vec![s("-p"), s("--bare"), s("--model"), s(model), s("--tools"), s(""),
                 s("--json-schema"), s(SCHEMA), s("--output-format"), s("json"), s("--no-session-persistence")],
            vec![(s("CLAUDE_CONFIG_DIR"), dir.join("home").display().to_string())],
        ),
        Host::Codex => {
            std::fs::write(dir.join("schema.json"), SCHEMA).expect("write the judge schema");
            (
                vec![s("exec"), s("--skip-git-repo-check"), s("-m"), s(model), s("-s"), s("read-only"),
                     s("--output-schema"), dir.join("schema.json").display().to_string(),
                     s("-o"), dir.join("verdict.json").display().to_string(), s("-C"), dir.display().to_string(), s("-")],
                vec![(s("CODEX_HOME"), dir.join("home").display().to_string())],
            )
        }
    };
    host::Invocation { program: judge.binary(), args, env, cwd: dir.to_owned(), stdin: prompt.to_owned() }
}
````

The reply: for Claude, the stdout JSON's `structured_output` field when present,
else its `result` field parsed as JSON; for Codex, the file `verdict.json`. Task
5's probe recorded the Claude output shape; if it differs, adapt the extraction
and add a test with the recorded shape.

- [ ] **Step 3: Call the judge from the run loop**

In `eval.rs`, after the mechanical score: when `mechanical.passed()`, collect
the workspace files (every `.typl`, `.ridl`, `.rsdl` and `ridl.toml` under
`dirs.workspace`, skipping `.claude/`, with paths relative to it), read the
final message (Claude: the `result` field of the last `"type":"result"` line of
the transcript; Codex: `dirs.last_message`), build the prompt, run the other
host as judge in a fresh scratch directory (log in first for Codex, as in Task
4), and write into the result file:

```toml
[rubric]
judge_host = "codex"
judge_model = "gpt-6-sol"
met = [true, false, true]
score = 0.8
```

When the run failed the mechanical checks, write `[rubric] score = 0.0` and
`skipped = "mechanical checks failed"`. When `parse_verdict` fails, write
`[rubric] error = "<message>"` and no score; `report::gate` counts such a run as
missing and the summary lists it, so a broken judge reply never becomes a
number.

- [ ] **Step 4: Implement `report.rs`**

`write_summary(out)` reads every `<task>/<label>.toml`, and writes
`out/summary.md` with one table per host: rows are tasks, columns are "pass rate
with", "pass rate without", "rubric with", "rubric without", and a final row
with the means; below it, the list of runs with a rubric error. `gate(out)`
returns one `HostGate` per host with the mean pass rate and mean rubric score
per side; `holds()` is
`pass_with >= pass_without && rubric_with >= rubric_without`. Call
`write_summary` at the end of `eval::run`, and print one line per host:
`eval: gate claude holds` or `eval: gate claude fails (…values…)`.

- [ ] **Step 5: Run the tests and extend the fake-host test**

Extend the fake `claude` and `codex` scripts of `xtask/tests/eval_cli.rs`: when
their arguments contain `--json-schema` (Claude) or `--output-schema` (Codex),
print (Claude) or write to the `-o` file (Codex) the verdict
`{"items":[{"n":1,"met":true,"reason":"r"}, …]}` for every item of
`design-0001`'s rubric (count them with
`grep -c '^[0-9]*\. ' evals/tasks/design-0001/rubric.md`). Add a test that runs
one "without" run on each host and asserts `summary.md` exists and the result
file holds `[rubric] score`.

Run:
`cargo test --locked -p xtask eval && cargo test --locked -p xtask --test eval_cli`
Expected: all pass. Mutate `holds()` to use `>` and confirm the report test
fails; restore.

- [ ] **Step 6: Commit**

```bash
git add xtask/src/eval.rs xtask/src/eval/judge.rs xtask/src/eval/report.rs xtask/tests/eval_cli.rs
git commit -m "feat(xtask): judge eval rubrics across vendors and report the gate"
```

---

### Task 7: The judge check

**Files:**

- Modify: `xtask/src/eval.rs` (subcommands `sample` and `agreement`)
- Modify: `xtask/src/eval/report.rs`
- Modify: `xtask/tests/eval_cli.rs`

**Interfaces:**

- `cargo xtask eval sample --from <results dir> --size 20` writes
  `<results dir>/judge-check/labels.toml`: one `[[run]]` per sampled run with
  `task`, `label`, the rubric item texts, and an empty `human = []` array for
  the maintainer to fill with `true`/`false` per item. Sampling is
  deterministic: sort all runs that have a judge verdict by `task/label`, take
  every `k`-th with `k = max(1, count / size)` until `size` are taken.
- `cargo xtask eval agreement <labels.toml>` compares `human` with the judge's
  `met` for each run, prints the per-item agreement as a percentage, writes
  `evals/results/judge-check.md` with the table and the overall value, and exits
  1 when it is below 80%, and 2 when any `human` array is empty or of the wrong
  length (an input error).

- [ ] **Step 1: Write the failing tests**

In `report.rs`:

```rust
    #[test]
    fn the_sample_takes_every_kth_run_in_order() {
        let runs: Vec<String> = (0..40).map(|i| format!("t/{i:02}")).collect();
        let picked = sample(&runs, 20);
        assert_eq!(picked.len(), 20);
        assert_eq!(picked[0], "t/00");
        assert_eq!(picked[1], "t/02");
        assert_eq!(sample(&runs[..10], 20).len(), 10);
    }

    #[test]
    fn agreement_counts_items_not_runs() {
        assert_eq!(agreement(&[(vec![true, false], vec![true, true])]).unwrap(), 0.5);
        assert_eq!(agreement(&[(vec![true], vec![true]), (vec![false], vec![false])]).unwrap(), 1.0);
    }

    #[test]
    fn an_unlabelled_run_is_an_error() {
        assert_eq!(
            agreement(&[(vec![], vec![true])]).unwrap_err(),
            "run 1 has 0 human labels for 1 rubric items"
        );
    }
```

In `eval_cli.rs`, a test that writes a `labels.toml` with one `[[run]]` whose
`human = []` and `judge = [true]`, runs `cargo xtask eval agreement <file>`, and
asserts exit code 2 and that stderr contains `0 human labels`.

- [ ] **Step 2: Run them**

Run: `cargo test --locked -p xtask eval::report` Expected: compile failure
(`sample` and `agreement` do not exist).

- [ ] **Step 3: Implement**

In `report.rs`:

```rust
/// Every k-th run in sorted order, k = max(1, count / size), until `size`
/// are taken. Deterministic, so a reviewer can reproduce the sample.
pub fn sample(runs: &[String], size: usize) -> Vec<String> {
    let mut sorted = runs.to_vec();
    sorted.sort();
    let k = (sorted.len() / size.max(1)).max(1);
    sorted.into_iter().step_by(k).take(size).collect()
}

/// The fraction of rubric items on which the maintainer and the judge agree,
/// over all runs. Each pair is (human, judge).
pub fn agreement(runs: &[(Vec<bool>, Vec<bool>)]) -> Result<f64, String> {
    let (mut same, mut total) = (0usize, 0usize);
    for (i, (human, judge)) in runs.iter().enumerate() {
        if human.len() != judge.len() {
            return Err(format!(
                "run {} has {} human labels for {} rubric items",
                i + 1,
                human.len(),
                judge.len()
            ));
        }
        same += human.iter().zip(judge).filter(|(h, j)| h == j).count();
        total += judge.len();
    }
    if total == 0 {
        return Err("no labelled item".to_owned());
    }
    Ok(same as f64 / total as f64)
}
```

In `eval.rs`, dispatch before `parse`:

```rust
match args.first().map(String::as_str) {
    Some("sample") => return sample_command(&args[1..]),
    Some("agreement") => return agreement_command(&args[1..]),
    _ => {}
}
```

`sample_command` reads `--from <dir>` and `--size <n>` (default 20), lists the
result files that have `[rubric] met`, calls `report::sample` on their
`<task>/<label>` keys, and writes `<dir>/judge-check/labels.toml` with one
`[[run]]` per pick: `task`, `label`, `items` (the rubric item texts from
`judge::rubric_items`), `judge` (the judge's `met`) and `human = []`.
`agreement_command` reads that file, calls `report::agreement` on
`(human, judge)` pairs, writes `evals/results/judge-check.md` (one row per run:
task, label, items agreed, items total; then the overall percentage), prints the
percentage, and returns `Err` (exit 2) on an input error. It exits 1 through
`std::process::exit(1)` when the value is below 0.8, after writing the file.

- [ ] **Step 4: Run the tests**

Run:
`cargo test --locked -p xtask eval && cargo test --locked -p xtask --test eval_cli`
Expected: all pass. Mutate the 0.8 comparison to 0.0 and confirm no test fails;
then add a CLI test with a labels file at 50% agreement that asserts exit code
1, and confirm the mutation now fails it; restore.

- [ ] **Step 5: Commit**

```bash
git add xtask/src/eval.rs xtask/src/eval/report.rs xtask/tests/eval_cli.rs
git commit -m "feat(xtask): sample eval runs for the judge check and measure agreement"
```

---

### Task 8: Ship the skill in the VS Code extension and the release

**Files:**

- Modify: `editors/vscode/package.json` (`engines.vscode`, `@types/vscode`,
  `contributes.chatSkills`, `contributes.chatInstructions`)
- Modify: `editors/vscode/package-lock.json` (by `npm install`)
- Modify: `.gitignore` (add `editors/vscode/skills/`)
- Modify: `justfile` (`package-vsix` copies the skill; `vscode-verify` checks
  it; a new recipe `package-skill`)
- Modify: `.github/workflows/vscode-release.yaml` (`package-vsix` job builds and
  uploads the zip; the `release` job's `files:` adds `artifacts/*.zip`)
- Modify: `docs/book/getting-started.md` ("Editor and agent setup"),
  `crates/ridl-mcp/README.md` ("Host configuration")

**Interfaces:**

- `just package-skill <out-dir>` writes
  `<out-dir>/ridl-authoring-<version>.zip`, whose top-level directory is
  `ridl-authoring/`, with `<version>` read from `SKILL.md`'s `metadata.version`.

- [ ] **Step 1: Find the `@types/vscode` version**

Run:
`npm view @types/vscode versions --json | tr -d ' \n' | grep -o '"1\.109\.[0-9]*"'`
Expected: at least `"1.109.0"`. Use the lowest 1.109.x listed.

- [ ] **Step 2: Write the failing archive check**

In the `vscode-verify` recipe, inside the loop after the existing checks, add:

```bash
for f in SKILL.md rules.md typl.md ridl.md rsdl.md; do
    grep -qx "extension/skills/ridl-authoring/$f" <<<"$listing" \
        || { echo "vscode-verify: $archive lacks skills/ridl-authoring/$f" >&2; exit 1; }
done
```

Run: `just vscode-verify` Expected: FAIL with "lacks
skills/ridl-authoring/SKILL.md".

- [ ] **Step 3: Package the skill**

In `package-vsix`, next to the line that copies `THIRD-PARTY-NOTICES.txt` into
`editors/vscode/bin/`, add:

```bash
rm -rf editors/vscode/skills
mkdir -p editors/vscode/skills
cp -R skills/ridl-authoring editors/vscode/skills/ridl-authoring
```

(Use the recipe's existing working-directory convention; if it runs inside
`editors/vscode`, the paths become `skills` and `../../skills/ridl-authoring`.)

In `editors/vscode/package.json`: set `"vscode": "^1.109.0"` and
`"@types/vscode": "<version from Step 1>"`, and add to `contributes`:

```json
"chatSkills": [
  { "path": "./skills/ridl-authoring/SKILL.md" }
],
"chatInstructions": [
  { "path": "./skills/ridl-authoring/rules.md" }
]
```

Run `npm install` in `editors/vscode` to update the lock file. Add
`editors/vscode/skills/` to `.gitignore` next to `editors/vscode/bin/`.

Add the recipe:

```just
# Zip the ridl-authoring skill for a release asset.
package-skill out:
    #!/usr/bin/env bash
    set -euo pipefail
    version="$(sed -n 's/^  version: "\(.*\)"$/\1/p' skills/ridl-authoring/SKILL.md)"
    test -n "$version" || { echo "package-skill: no metadata.version in SKILL.md" >&2; exit 1; }
    mkdir -p "{{out}}"
    out="$(cd "{{out}}" && pwd)"
    (cd skills && zip -qr "$out/ridl-authoring-$version.zip" ridl-authoring)
    echo "$out/ridl-authoring-$version.zip"
```

Run:
`just vscode-verify && just package-skill "$SCRATCH/zip" && unzip -l "$SCRATCH"/zip/*.zip`
Expected: `vscode-verify: packaged ok`; the zip lists `ridl-authoring/SKILL.md`
and the four other files.

- [ ] **Step 4: Release workflow**

In `.github/workflows/vscode-release.yaml`, in the `package-vsix` job, after the
VSIX is built, add a step `run: just package-skill dist` and include
`dist/*.zip` in that job's `actions/upload-artifact@v4` `path`. In the `release`
job's `softprops/action-gh-release@v2` `files:`, add the line `artifacts/*.zip`.
Run `just gate-parity` and `cargo test --locked -p ridl-cli --test ci_workflow`,
and fix what they report. The VS Code floor (1.109) is recorded in the new ADR
at gardening (Task 13).

- [ ] **Step 5: Docs**

In `docs/book/getting-started.md`, after the "**Agent hosts.**" paragraph:

```markdown
**The authoring skill.** Each editor release attaches
`ridl-authoring-<version>.zip`, a skill that teaches an agent to write and
review RIDL and to check its work with the `ridl` MCP server. For Claude Code,
unpack it into `~/.claude/skills/` or a project's `.claude/skills/`; for
Codex, into `~/.codex/skills/`. The VS Code extension already contains it for
GitHub Copilot, and needs VS Code 1.109 or later.
```

Add the same three facts to `crates/ridl-mcp/README.md` under "Host
configuration", one sentence per host subsection.

- [ ] **Step 6: Run and commit**

Run:
`just vscode-verify && just link-check && just book-check && prim lint docs/book/getting-started.md crates/ridl-mcp/README.md`
Expected: all pass.

```bash
git add editors/vscode/package.json editors/vscode/package-lock.json .gitignore justfile .github/workflows/vscode-release.yaml docs/book/getting-started.md crates/ridl-mcp/README.md
git commit -m "feat(editors): ship the ridl-authoring skill in the extension and the release"
```

The commit is not breaking for users of VS Code 1.109 or later; it removes
support for VS Code 1.101 to 1.108. Put that in the commit body and the PR body.

---

### Task 9: PR 2 verification

- [ ] Run `just verify` in the worktree. Expected: green.
- [ ] Open PR 2 with the delegated rulings `R-S-<n>` (the probe's flag changes,
      the two model ids if Sebastien delegated them, the anchor rule, the VS
      Code floor). Run `/review` passes 1 and 2, fix, and ask Sebastien to
      merge.

---

### Task 10: `rules.md` and `typl.md`

**Start condition:** PR 2 is merged, and `freeze/surfaces.toml` holds the rows
`keywords`, `accepted-sources`, `manifest` and `diagnostics` (lane L's gates).
Check with
`grep -E '^\[(keywords|accepted-sources|manifest|diagnostics)\]' freeze/surfaces.toml`
(or the row form the file uses). Do not start before.

**Files:**

- Modify: `skills/ridl-authoring/rules.md`, `skills/ridl-authoring/typl.md`,
  `skills/ridl-authoring/SKILL.md` (the verify loop and the routing sections)

**Sources:** `docs/wip/skill-ridl-authoring-outline.md` sections 0, 1, 2 and 5
and 7; `docs/specification/typl-language-reference.md`;
`docs/specification/ridl-family-overview.md` (the doctrines);
`freeze/diagnostics.txt`.

- [ ] **Step 1: `rules.md`.** 10 to 20 lines, each "Never …" or "Always …", each
      ending with the code it prevents (`(TYPL-101)`) or the doctrine it states
      (a link to its heading in the family overview). Drop the outline's `uxdl`
      and `.rxdl` content and every code that is not in
      `freeze/diagnostics.txt`.
- [ ] **Step 2: `typl.md`.** Sections: primitives and named types; units and
      ranges; constants; composites (struct, enum, enumset, union, collections);
      errors as data (the result union); packages, imports and visibility;
      evolution of a type; common mistakes (a table: mistake, correct form,
      code). Each section opens with a link to its heading in the typl
      reference, written as a book URL
      (`https://driftsys.github.io/ridl/<page>.html#<anchor>`): the skill gate
      rejects a relative `../../docs/` link, because it does not resolve where
      the skill is installed (maintainer ruling, 2026-10-12). Each example is a
      whole file with its own `package` (`skill.typl.<topic>`), or a fragment
      marked `,ignore`.
- [ ] **Step 3: `SKILL.md`.** Fill "The model in one minute" (five lines and the
      lattice), "The verify loop" (spec D-2: `ridl_check`, then `ridl_explain`,
      then `ridl_diff` for an edit to a published contract; the lookup tools
      first for an existing workspace; `ridl_metrics` and the design lints as
      evidence for a boundary or naming decision), and "Which reference to read"
      (one line per file). Keep `SKILL.md` under 150 lines.
- [ ] **Step 4: Gates.** Run
      `cargo test --locked -p ridl-cli --test book_examples --test skill_gates && just link-check && prim lint skills`.
      Expected: all pass.
- [ ] **Step 5: Draft eval.** With Sebastien's approval of the spend, run
      `cargo xtask eval --task design-0001 --task evolve-0001 --task review-0001 --runs 1`.
      Read the failed runs' notes and the rubric misses; change the skill where
      it taught nothing or taught wrongly; repeat at most three times. Do not
      commit these results.
- [ ] **Step 6: Commit**

```bash
git add skills/ridl-authoring
git commit -m "feat(skills): write the rules file and the typl reference of ridl-authoring"
```

---

### Task 11: `ridl.md` and `rsdl.md`

**Start condition:** Task 10 is done.

**Files:** `skills/ridl-authoring/ridl.md`, `skills/ridl-authoring/rsdl.md`.

**Sources:** the outline sections 3 and 6;
`docs/specification/ridl-language-reference.md`;
`docs/specification/rsdl-language-reference.md`; `examples/cabin`;
`docs/book/rsdl.md`.

- [ ] **Step 1: `ridl.md`.** Sections: the interaction kinds (the decision
      table: state or occurrence, action or request); timing; contracts
      (`require`, `ensure`); errors as data on a query; services; where to split
      an interface (cohesion, with `ridl_metrics` and RIDL-414 as evidence);
      evolution (append-only, ordinals, `reserved`, `ridl_diff`); common
      mistakes.
- [ ] **Step 2: `rsdl.md`.** Sections: components (`offers`, `requires`);
      systems; deployments and machines; evolving a system compatibly; common
      mistakes. The workspace of the skill's fences holds at most one `system`
      (RSDL-601), so exactly one fence in `rsdl.md` declares a `system`; the
      others are `,ignore` fragments or declare only components.
- [ ] **Step 3: Gates.** As in Task 10 Step 4.
- [ ] **Step 4: Draft eval.** As in Task 10 Step 5, with `design-0002`,
      `design-0004`, `evolve-0003`, `review-0002`.
- [ ] **Step 5: Commit**

```bash
git add skills/ridl-authoring
git commit -m "feat(skills): write the ridl and rsdl references of ridl-authoring"
```

---

### Task 12: The judge check and the release record

**Start condition:** Task 11 is done. Sebastien confirms the two model ids in
`evals/runner.toml` and approves the spend (144 agent runs and up to 144 judge
calls).

- [ ] **Step 1: Judge check.** Run `cargo xtask eval --runs 1` over all 12 tasks
      (48 runs), then
      `cargo xtask eval sample --from evals/results/<date> --size 20`. Ask
      Sebastien to fill `human` in `labels.toml`. Run
      `cargo xtask eval agreement evals/results/<date>/judge-check/labels.toml`.
      Expected: at least 80%. If lower, revise the instruction text in
      `judge_prompt` (Task 6), add a unit test for the case that went wrong, and
      repeat the sample and the labelling. Commit `evals/results/judge-check.md`
      and the labels file.
- [ ] **Step 2: Release record.** Run `cargo xtask eval --runs 3`. Expected:
      `eval: gate claude holds` and `eval: gate codex holds`. If a gate fails,
      stop and report to Sebastien with the summary; the skill does not ship
      until it is fixed (spec D-9).
- [ ] **Step 3: Copilot smoke check (by hand).** Install the VSIX built by
      `just package-vsix` in VS Code 1.109 or later, open a copy of an empty
      directory, ask Copilot the prompt of `design-0001`, and confirm in its
      references that `ridl-authoring` was used and that it called `ridl_check`.
      Record the outcome as one paragraph in `evals/results/<date>/summary.md`.
- [ ] **Step 4: Report the gains.** Add to `docs/book/getting-started.md`'s "The
      authoring skill" paragraph one sentence with the measured pass rates and
      rubric scores with and without the skill, per host, and a link to
      `evals/results/<date>/summary.md` (a relative link, which
      `just link-check` checks).
- [ ] **Step 5: Commit**

```bash
git add evals/results docs/book/getting-started.md
git commit -m "feat(repo): record the ridl-authoring release eval"
```

---

### Task 13: Gardening and PR 3

- [ ] Run the `sdd-gardening` skill. Its expected outputs:
  - a new ADR (next free number) holding D-1 to D-9, the judge check and the VS
    Code 1.109 floor;
  - ADR-0005: status Accepted; dated amendments R-S-1 to R-S-4, each linking the
    new ADR; the §8 note on `uxdl`; the open questions "Skill generation
    pipeline", "Eval scoring beyond compiles" and "Rules-file portability"
    marked resolved with links;
  - ADR-0027: one amendment line for the `start/` directory of a task;
  - `docs/decisions/README.md` rows;
  - `docs/ROADMAP.md`: E8.1, E8.2, E8.3 and E8.15 marked landed;
  - the spec, this plan and `docs/wip/skill-ridl-authoring-outline.md` moved to
    `docs/archive/`, after checking `docs/wip/README.md` for dispositions and
    grepping for citations of their paths.
- [ ] Run `just verify`. Open PR 3, closing #73, #74, #75 and #87. Run `/review`
      passes 1 and 2, fix, and ask Sebastien to merge.
- [ ] Update the memory file `release-0-10-lane-s-state.md`.
