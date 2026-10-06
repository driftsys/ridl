//! The public evaluation corpus must compile, fit its source budget, and
//! record the origin and interaction rule of every workspace.

use std::path::{Path, PathBuf};
use std::process::Command;

fn corpus_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../evals/corpus")
}

fn corpus_dirs() -> Vec<PathBuf> {
    let root = corpus_root();
    if !root.exists() {
        return Vec::new();
    }
    let mut dirs: Vec<_> = std::fs::read_dir(&root)
        .expect("read the corpus root")
        .map(|entry| entry.expect("read a corpus entry"))
        .filter(|entry| entry.file_type().expect("read a corpus file type").is_dir())
        .map(|entry| entry.path())
        .collect();
    dirs.sort();
    dirs
}

/// Counts all physical source lines, including comments and blank lines.
fn source_lines(dir: &Path) -> usize {
    std::fs::read_dir(dir)
        .unwrap_or_else(|error| panic!("read {}: {error}", dir.display()))
        .map(|entry| {
            let entry = entry.expect("read a corpus entry");
            let path = entry.path();
            let kind = entry.file_type().expect("read a corpus file type");
            if kind.is_dir() {
                source_lines(&path)
            } else if kind.is_file()
                && matches!(
                    path.extension().and_then(|extension| extension.to_str()),
                    Some("typl" | "ridl" | "rsdl")
                )
            {
                std::fs::read_to_string(&path)
                    .unwrap_or_else(|error| panic!("read {}: {error}", path.display()))
                    .lines()
                    .count()
            } else {
                0
            }
        })
        .sum()
}

#[test]
fn the_corpus_contains_exactly_the_selected_workspaces() {
    let names: Vec<_> = corpus_dirs()
        .iter()
        .map(|dir| {
            dir.file_name()
                .expect("a corpus directory name")
                .to_str()
                .expect("a UTF-8 corpus directory name")
                .to_owned()
        })
        .collect();
    assert_eq!(names, ["mavlink", "ros2", "vss"]);
}

#[test]
fn source_lines_counts_physical_lines_in_nested_source_files() {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("the current time follows the epoch")
        .as_nanos();
    let root =
        std::env::temp_dir().join(format!("ridl-source-lines-{}-{unique}", std::process::id()));
    let nested = root.join("nested/deeper");
    std::fs::create_dir_all(&nested).expect("create the line-count fixture");
    std::fs::write(
        root.join("types.typl"),
        "\n// A comment\npackage fixture\ntype Value : integer\n",
    )
    .expect("write the type fixture");
    std::fs::write(
        root.join("nested/contract.ridl"),
        "// A comment\n\ninterface Contract {}\n",
    )
    .expect("write the interface fixture");
    std::fs::write(nested.join("system.rsdl"), "// A comment\n\n")
        .expect("write the system fixture");
    std::fs::write(
        nested.join("ignored.txt"),
        "one\ntwo\nthree\n\n// ignored\n",
    )
    .expect("write the ignored fixture");

    let total = source_lines(&root);
    let nested_total = source_lines(&root.join("nested"));
    std::fs::remove_dir_all(&root).expect("remove the line-count fixture");
    assert_eq!(total, 9, "count every source line and ignore other files");
    assert_eq!(nested_total, 5, "count all supported nested source files");
}

#[test]
fn every_corpus_workspace_checks_without_an_error() {
    let dirs = corpus_dirs();
    assert!(!dirs.is_empty(), "the evaluation corpus must not be empty");
    for dir in dirs {
        let output = Command::new(env!("CARGO_BIN_EXE_ridl"))
            .args(["check", "--format", "json"])
            .arg(&dir)
            .output()
            .expect("run ridl check on a corpus workspace");
        let diagnostics: Vec<serde_json::Value> = serde_json::from_slice(&output.stdout)
            .unwrap_or_else(|error| {
                panic!(
                    "{}: invalid diagnostic JSON: {error}\nstdout: {}\nstderr: {}",
                    dir.display(),
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr),
                )
            });
        assert!(
            diagnostics
                .iter()
                .all(|diagnostic| diagnostic["severity"] != "error"),
            "{} has error diagnostics: {diagnostics:#?}",
            dir.display(),
        );
        assert!(
            output.status.success(),
            "{}: ridl check exited {}\nstderr: {}",
            dir.display(),
            output.status,
            String::from_utf8_lossy(&output.stderr),
        );
    }
}

#[test]
fn the_corpus_stays_inside_its_budget() {
    let root = corpus_root();
    for (name, budget) in [("ros2", 2_500), ("mavlink", 2_500), ("vss", 1_000)] {
        let dir = root.join(name);
        if dir.exists() {
            let lines = source_lines(&dir);
            assert!(
                lines <= budget,
                "{name}: {lines} source lines exceed {budget}"
            );
        }
    }
    let total: usize = corpus_dirs().iter().map(|dir| source_lines(dir)).sum();
    assert!(
        total <= 6_000,
        "{total} source lines exceed the 6,000-line corpus budget"
    );
}

#[test]
fn every_corpus_workspace_records_its_provenance() {
    for dir in corpus_dirs() {
        for filename in ["PROVENANCE.md", "LICENSE"] {
            let path = dir.join(filename);
            let text = std::fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
            assert!(
                !text.trim().is_empty(),
                "{} must not be empty",
                path.display()
            );
            if filename == "PROVENANCE.md" {
                for header in ["Upstream:", "Revision:", "Licence:", "Kind rule:"] {
                    assert!(
                        text.lines().any(|line| line.starts_with(header)),
                        "{} is missing {header}",
                        path.display(),
                    );
                }
            }
        }
    }
}

fn task_root() -> PathBuf {
    corpus_root()
        .parent()
        .expect("the evals directory")
        .join("tasks")
}

fn validate_task(dir: &Path) -> Result<serde_json::Value, String> {
    let read = |name: &str| std::fs::read_to_string(dir.join(name)).map_err(|e| e.to_string());
    let task: serde_json::Value = toml::from_str(&read("task.toml")?).map_err(|e| e.to_string())?;
    let require = |condition: bool, message: &str| {
        if condition {
            Ok(())
        } else {
            Err(message.to_owned())
        }
    };
    require(
        task["id"].as_str() == dir.file_name().and_then(|n| n.to_str()),
        "id must equal directory name",
    )?;
    let kind = task["kind"].as_str().ok_or("kind must be a string")?;
    require(
        matches!(kind, "review" | "evolve" | "design"),
        "unknown task kind",
    )?;
    require(
        task["title"].as_str().is_some_and(|s| !s.trim().is_empty()),
        "title must be nonempty",
    )?;
    if kind == "design" {
        require(task.get("corpus").is_none(), "design task must omit corpus")?;
    } else {
        let corpus = task["corpus"].as_str().ok_or("corpus must be a string")?;
        require(
            corpus_dirs()
                .iter()
                .any(|p| p.file_name().and_then(|n| n.to_str()) == Some(corpus)),
            "corpus must name an existing workspace",
        )?;
    }
    require(
        task["expect"]["compiles"].is_boolean(),
        "expect.compiles must be boolean",
    )?;
    let lints = task["expect"]["lints"]
        .as_array()
        .ok_or("expect.lints must be an array")?;
    for lint in lints {
        let name = lint.as_str().ok_or("lint name must be a string")?;
        require(
            ridl_core::diag::ALL_CATALOGS
                .iter()
                .flat_map(|(_, entries)| *entries)
                .any(|entry| entry.lint == Some(name)),
            "lint name must be in the catalogue",
        )?;
    }
    if kind == "evolve" {
        let _verdict = match task["expect"]["diff"].as_str() {
            Some("identical") => ridl_diff::Verdict::Identical,
            Some("compatible") => ridl_diff::Verdict::Compatible,
            Some("breaking") => ridl_diff::Verdict::Breaking,
            _ => return Err("expect.diff must name a diff verdict".to_owned()),
        };
    } else {
        require(
            task["expect"].get("diff").is_none(),
            "only evolve tasks may name a diff verdict",
        )?;
    }
    for name in ["prompt.md", "rubric.md"] {
        let content = read(name)?;
        require(
            !content.trim().is_empty(),
            "prompt and rubric must be nonempty",
        )?;
        if name == "rubric.md" {
            validate_rubric(&content)?;
        }
    }
    Ok(task)
}

fn validate_rubric(content: &str) -> Result<(), String> {
    let mut expected = 1;
    let mut item_depth = 0;
    // Markdown item offsets preserve authored numbers, even in adjacent lists;
    // wrapped text produces no item event and cannot hide another item.
    for (event, range) in pulldown_cmark::Parser::new(content).into_offset_iter() {
        match event {
            pulldown_cmark::Event::Start(pulldown_cmark::Tag::Item) => {
                if item_depth != 0 {
                    return Err("rubric items must not be nested".to_owned());
                }
                let line_start = content[..range.start].rfind('\n').map_or(0, |pos| pos + 1);
                let line = content[line_start..].lines().next().unwrap_or_default();
                let prefix = format!("{expected}. ");
                let item = line
                    .strip_prefix(&prefix)
                    .ok_or("rubric item numbers must be consecutive and unindented")?;
                if !["**must** ", "**should** ", "**must not** "]
                    .iter()
                    .any(|mark| item.starts_with(mark))
                {
                    return Err("rubric item must have a requirement marker".to_owned());
                }
                expected += 1;
                item_depth += 1;
            }
            pulldown_cmark::Event::End(pulldown_cmark::TagEnd::Item) => item_depth -= 1,
            pulldown_cmark::Event::Start(pulldown_cmark::Tag::List(Some(_))) => {}
            pulldown_cmark::Event::End(pulldown_cmark::TagEnd::List(_)) => {}
            _ if item_depth == 0 => {
                return Err("rubric content must belong to numbered items".to_owned());
            }
            _ => {}
        }
    }
    if expected == 1 {
        return Err("rubric must contain an item".to_owned());
    }
    Ok(())
}

#[test]
fn rubric_validation_accepts_adjacent_numbered_items() {
    assert!(
        validate_rubric("1. **must** preserve identity.\n2. **should** explain compatibility.\n")
            .is_ok()
    );
}

#[test]
fn rubric_validation_rejects_an_invalid_adjacent_marker() {
    assert!(
        validate_rubric("1. **must** preserve identity.\n2. **may** ignore compatibility.\n")
            .is_err()
    );
}

#[test]
fn rubric_validation_rejects_a_duplicate_adjacent_item_id() {
    assert!(
        validate_rubric("1. **must** preserve identity.\n1. **should** explain compatibility.\n")
            .is_err()
    );
}

#[test]
fn rubric_validation_accepts_indented_wrapped_continuations() {
    assert!(validate_rubric("1. **must** preserve every existing\n   interaction identity.\n\n2. **must not** change existing\n   wire numbers.\n").is_ok());
}

fn validate_task_fixture(kind: &str, lints: &str, diff: &str) -> Result<serde_json::Value, String> {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("the current time follows the epoch")
        .as_nanos();
    static NEXT_FIXTURE: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let sequence = NEXT_FIXTURE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let id = format!("ridl-task-{}-{unique}-{sequence}", std::process::id());
    let dir = std::env::temp_dir().join(&id);
    std::fs::create_dir(&dir).expect("create the isolated task fixture");
    let corpus = if kind == "design" {
        String::new()
    } else {
        let dirs = corpus_dirs();
        let name = dirs[0].file_name().unwrap().to_str().unwrap();
        format!("corpus = {name:?}\n")
    };
    std::fs::write(
        dir.join("task.toml"),
        format!("id = \"{id}\"\nkind = \"{kind}\"\ntitle = \"Task validation fixture\"\n{corpus}\n[expect]\ncompiles = true\nlints = {lints}\n{diff}\n"),
    )
    .expect("write task metadata with the tested expectations");
    std::fs::write(dir.join("prompt.md"), "Provide valid declarations.\n")
        .expect("write the fixture prompt");
    std::fs::write(
        dir.join("rubric.md"),
        "1. **must** provide valid declarations.\n",
    )
    .expect("write the fixture rubric");
    let result = validate_task(&dir);
    std::fs::remove_dir_all(&dir).expect("remove the isolated task fixture");
    result
}

fn validate_lint_fixture(lints: &str) -> Result<serde_json::Value, String> {
    validate_task_fixture("design", lints, "")
}

#[test]
fn task_diff_validation_accepts_every_recognized_evolve_verdict() {
    for verdict in ["identical", "compatible", "breaking"] {
        validate_task_fixture("evolve", "[]", &format!("diff = {verdict:?}"))
            .unwrap_or_else(|error| panic!("{verdict}: {error}"));
    }
}

#[test]
fn task_diff_validation_rejects_a_missing_evolve_verdict() {
    assert_eq!(
        validate_task_fixture("evolve", "[]", "").unwrap_err(),
        "expect.diff must name a diff verdict",
    );
}

#[test]
fn task_diff_validation_rejects_an_unknown_evolve_verdict() {
    assert_eq!(
        validate_task_fixture("evolve", "[]", "diff = \"added\"").unwrap_err(),
        "expect.diff must name a diff verdict",
    );
}

#[test]
fn task_diff_validation_rejects_a_verdict_on_each_non_evolve_kind() {
    for kind in ["review", "design"] {
        assert_eq!(
            validate_task_fixture(kind, "[]", "diff = \"compatible\"").unwrap_err(),
            "only evolve tasks may name a diff verdict",
        );
    }
}

#[test]
fn task_lint_validation_accepts_a_released_catalogue_name() {
    validate_lint_fixture("[\"unused-import\"]")
        .expect("the released unused-import lint must be accepted");
}

#[test]
fn task_lint_validation_rejects_an_unknown_name() {
    assert_eq!(
        validate_lint_fixture("[\"unknown-eval-fixture-lint\"]").unwrap_err(),
        "lint name must be in the catalogue",
    );
}

/// A candidate check that the calibration did not ship has no catalogue row,
/// so its name is rejected like any other unknown name.
#[test]
fn task_lint_validation_rejects_a_dropped_candidate_name() {
    for dropped in ["inconsistent-abbreviation", "package-fan-out"] {
        assert_eq!(
            validate_lint_fixture(&format!("[{dropped:?}]")).unwrap_err(),
            "lint name must be in the catalogue",
            "{dropped} must not be accepted",
        );
    }
}

#[test]
fn task_lint_validation_accepts_each_shipped_design_lint() {
    for shipped in [
        "inconsistent-unit",
        "duplicate-shape",
        "low-cohesion-interface",
    ] {
        validate_lint_fixture(&format!("[{shipped:?}]"))
            .unwrap_or_else(|error| panic!("{shipped}: {error}"));
    }
}

#[test]
fn task_lint_validation_rejects_a_non_string_entry() {
    assert_eq!(
        validate_lint_fixture("[7]").unwrap_err(),
        "lint name must be a string",
    );
}

#[test]
fn every_eval_task_is_well_formed() {
    let root = task_root();
    let mut dirs: Vec<_> = std::fs::read_dir(&root)
        .unwrap_or_else(|error| panic!("read {}: {error}", root.display()))
        .map(|entry| entry.expect("read a task entry"))
        .filter(|entry| entry.file_type().expect("read a task file type").is_dir())
        .map(|entry| entry.path())
        .collect();
    dirs.sort();
    let tasks: Vec<_> = dirs
        .iter()
        .map(|dir| validate_task(dir).unwrap_or_else(|error| panic!("{}: {error}", dir.display())))
        .collect();
    validate_task_set(&tasks).expect("the evaluation task set must preserve the approved seed");
}

fn validate_task_set(tasks: &[serde_json::Value]) -> Result<(), String> {
    if tasks.len() < 10 {
        return Err("at least ten evaluation tasks are required".to_owned());
    }
    let third_corpus_tasks = tasks.iter().filter(|task| task["corpus"] == "vss").count();
    if third_corpus_tasks * 3 > tasks.len() {
        return Err("at most one third of tasks may use the third corpus".to_owned());
    }
    // Seed IDs also identify the committed rubric items. Additional tasks may
    // extend the set without replacing or reclassifying those seed tasks.
    for (kind, count) in [("review", 5), ("evolve", 2), ("design", 3)] {
        for number in 1..=count {
            let id = format!("{kind}-{number:04}");
            let task = tasks
                .iter()
                .find(|task| task["id"] == id)
                .ok_or_else(|| format!("approved seed task {id} is missing"))?;
            if task["kind"] != kind {
                return Err(format!("approved seed task {id} must retain its kind"));
            }
        }
    }
    for dir in corpus_dirs() {
        let name = dir.file_name().unwrap().to_str().unwrap();
        if !tasks
            .iter()
            .any(|task| task["kind"] == "review" && task["corpus"] == name)
        {
            return Err(format!(
                "every selected corpus must retain a review task: {name}"
            ));
        }
    }
    Ok(())
}

fn approved_task_fixture() -> Vec<serde_json::Value> {
    let mut dirs: Vec<_> = std::fs::read_dir(task_root())
        .expect("read the approved tasks")
        .map(|entry| entry.expect("read a task entry").path())
        .filter(|path| path.is_dir())
        .collect();
    dirs.sort();
    dirs.iter()
        .map(|dir| validate_task(dir).expect("an approved task must be valid"))
        .collect()
}

#[test]
fn task_set_validation_rejects_replacing_a_seed_review_with_an_extra_design() {
    let mut tasks = approved_task_fixture();
    let task = tasks
        .iter_mut()
        .find(|task| task["id"] == "review-0005")
        .unwrap();
    task["id"] = "design-0004".into();
    task["kind"] = "design".into();
    task.as_object_mut().unwrap().remove("corpus");
    assert_eq!(
        validate_task_set(&tasks).unwrap_err(),
        "approved seed task review-0005 is missing"
    );
}

#[test]
fn task_set_validation_rejects_replacing_every_approved_seed_id() {
    for id in [
        "review-0001",
        "review-0002",
        "review-0003",
        "review-0004",
        "review-0005",
        "evolve-0001",
        "evolve-0002",
        "design-0001",
        "design-0002",
        "design-0003",
    ] {
        let mut tasks = approved_task_fixture();
        let original = tasks.iter().find(|task| task["id"] == id).unwrap().clone();
        // Retain the kind, corpus and expectations so that replacement leaves
        // the task count, corpus share and review coverage unchanged.
        let replacement = format!("{}-9001", original["kind"].as_str().unwrap());
        assert!(tasks.iter().all(|task| task["id"] != replacement));
        let task = tasks.iter_mut().find(|task| task["id"] == id).unwrap();
        task["id"] = replacement.into();
        let mut restored = task.clone();
        restored["id"] = id.into();
        assert_eq!(restored, original, "only the fixture ID may change");
        assert_eq!(
            validate_task_set(&tasks),
            Err(format!("approved seed task {id} is missing")),
            "replacement of {id} must be rejected",
        );
    }
}

#[test]
fn task_set_validation_rejects_changing_a_seed_kind_without_renaming() {
    for (id, kind) in [
        ("review-0005", "design"),
        ("evolve-0001", "review"),
        ("design-0001", "review"),
    ] {
        let mut tasks = approved_task_fixture();
        let task = tasks.iter_mut().find(|task| task["id"] == id).unwrap();
        task["kind"] = kind.into();
        if kind == "design" {
            task.as_object_mut().unwrap().remove("corpus");
        } else if task.get("corpus").is_none() {
            let dirs = corpus_dirs();
            task["corpus"] = dirs[0].file_name().unwrap().to_str().unwrap().into();
        }
        task["expect"].as_object_mut().unwrap().remove("diff");
        assert_eq!(
            validate_task_set(&tasks).unwrap_err(),
            format!("approved seed task {id} must retain its kind")
        );
    }
}

#[test]
fn task_set_validation_rejects_losing_review_coverage_for_each_corpus() {
    for dir in corpus_dirs() {
        let name = dir.file_name().unwrap().to_str().unwrap();
        let mut tasks = approved_task_fixture();
        for task in &mut tasks {
            if task["kind"] == "review" && task["corpus"] == name {
                let other = corpus_dirs().into_iter().find(|path| path != &dir).unwrap();
                task["corpus"] = other.file_name().unwrap().to_str().unwrap().into();
            }
        }
        assert_eq!(
            validate_task_set(&tasks).unwrap_err(),
            format!("every selected corpus must retain a review task: {name}")
        );
    }
}

#[test]
fn task_set_validation_accepts_future_additions() {
    let mut tasks = approved_task_fixture();
    let mut extra = tasks
        .iter()
        .find(|task| task["id"] == "design-0003")
        .unwrap()
        .clone();
    extra["id"] = "design-0004".into();
    tasks.push(extra);
    validate_task_set(&tasks).expect("additional tasks may extend the approved seed");
}

/// The design lints whose counts on the corpus are pinned, by lint name.
const DESIGN_LINTS: &[&str] = &[
    "inconsistent-unit",
    "duplicate-shape",
    "low-cohesion-interface",
];

/// Counts the design lint findings `ridl check` reports at default levels on
/// each corpus workspace, as `workspace -> lint name -> count`.
fn design_lint_counts()
-> std::collections::BTreeMap<String, std::collections::BTreeMap<String, usize>> {
    let mut counts = std::collections::BTreeMap::new();
    for dir in corpus_dirs() {
        let workspace = dir.file_name().unwrap().to_str().unwrap().to_owned();
        let output = Command::new(env!("CARGO_BIN_EXE_ridl"))
            .args(["check", "--format", "json"])
            .arg(&dir)
            .output()
            .expect("run ridl check on a corpus workspace");
        // A corpus workspace has no error diagnostic, so `ridl check` exits 0.
        // Any other status is a crash or a new error, and stderr says which.
        assert!(
            output.status.success(),
            "{workspace}: `ridl check` exited with {}\nstderr:\n{}",
            output.status,
            String::from_utf8_lossy(&output.stderr),
        );
        let diagnostics: Vec<serde_json::Value> = serde_json::from_slice(&output.stdout)
            .unwrap_or_else(|error| panic!("{workspace}: invalid diagnostic JSON: {error}"));
        let per_lint: std::collections::BTreeMap<String, usize> = DESIGN_LINTS
            .iter()
            .map(|lint| {
                let count = diagnostics
                    .iter()
                    .filter(|diagnostic| diagnostic["lint"] == *lint)
                    .count();
                ((*lint).to_owned(), count)
            })
            .collect();
        counts.insert(workspace, per_lint);
    }
    counts
}

/// Renders the counts in the layout of `evals/calibration/expected-counts.toml`.
fn render_counts(
    counts: &std::collections::BTreeMap<String, std::collections::BTreeMap<String, usize>>,
) -> String {
    let mut text = String::new();
    for (workspace, per_lint) in counts {
        text.push_str(&format!("[{workspace}]\n"));
        for (lint, count) in per_lint {
            text.push_str(&format!("{lint} = {count}\n"));
        }
        text.push('\n');
    }
    text
}

/// Pins the number of findings each shipped design lint reports on each corpus
/// workspace. A change to a check or to the language that changes a count
/// fails here, and its author updates `expected-counts.toml` deliberately.
#[test]
fn design_lint_counts_on_the_corpus_are_pinned() {
    let path = corpus_root()
        .parent()
        .expect("the evals directory")
        .join("calibration/expected-counts.toml");
    let actual = design_lint_counts();
    let table = render_counts(&actual);
    // A missing file reads as an empty table, so the first run fails with the
    // printed table; a file that exists but does not parse is its own error.
    let expected: std::collections::BTreeMap<String, std::collections::BTreeMap<String, usize>> =
        match std::fs::read_to_string(&path) {
            Ok(text) => toml::from_str(&text).unwrap_or_else(|error| {
                panic!("{}: invalid expected-counts TOML: {error}", path.display())
            }),
            Err(_) => Default::default(),
        };
    assert_eq!(
        actual,
        expected,
        "the design lint counts on the corpus changed.\n\
         Actual counts:\n\n{table}\n\
         If the change is intended, replace the content of {} with the table \
         above, and state the reason in the commit message.",
        path.display(),
    );
}
