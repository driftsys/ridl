//! The results summary and the release gate over the result files of one
//! eval directory.

use std::collections::BTreeMap;
use std::path::Path;

/// One result file, reduced to what the summary and the gate read.
struct Run {
    key: String,
    task: String,
    host: String,
    skill: bool,
    passed: bool,
    /// The run could not be scored; it is listed and left out of every
    /// rate and mean.
    setup_error: bool,
    /// `None` when the judge reply was an error; such a run is listed and
    /// left out of the rubric means.
    score: Option<f64>,
}

fn runs(out: &Path) -> Result<Vec<Run>, String> {
    let mut runs = Vec::new();
    let entries = std::fs::read_dir(out).map_err(|e| format!("read {}: {e}", out.display()))?;
    for task_dir in entries.flatten().map(|e| e.path()).filter(|p| p.is_dir()) {
        let Some(task) = task_dir.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if task == "transcripts" || task == "judge-check" {
            continue;
        }
        for file in std::fs::read_dir(&task_dir)
            .map_err(|e| e.to_string())?
            .flatten()
        {
            let path = file.path();
            if path.extension().and_then(|e| e.to_str()) != Some("toml") {
                continue;
            }
            let text = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
            let value: toml::Value =
                toml::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
            let label = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or_default();
            runs.push(Run {
                key: format!("{task}/{label}"),
                task: task.to_owned(),
                host: value
                    .get("host")
                    .and_then(toml::Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
                skill: value
                    .get("skill")
                    .and_then(toml::Value::as_bool)
                    .unwrap_or(false),
                passed: value
                    .get("mechanical")
                    .and_then(|m| m.get("passed"))
                    .and_then(toml::Value::as_bool)
                    .unwrap_or(false),
                setup_error: value.get("setup_error").is_some(),
                score: value
                    .get("rubric")
                    .and_then(|r| r.get("score"))
                    .and_then(toml::Value::as_float),
            });
        }
    }
    runs.sort_by(|a, b| a.key.cmp(&b.key));
    Ok(runs)
}

/// Whether `out` holds at least one result file.
pub fn has_results(out: &Path) -> Result<bool, String> {
    Ok(out.is_dir() && !runs(out)?.is_empty())
}

fn mean(values: impl Iterator<Item = f64>) -> f64 {
    let (sum, count) = values.fold((0.0, 0usize), |(s, c), v| (s + v, c + 1));
    if count == 0 { 0.0 } else { sum / count as f64 }
}

/// The means of one side (with or without the skill) of a set of runs.
fn side(runs: &[&Run], skill: bool) -> (f64, f64) {
    let side: Vec<&&Run> = runs
        .iter()
        .filter(|r| r.skill == skill && !r.setup_error)
        .collect();
    (
        mean(side.iter().map(|r| if r.passed { 1.0 } else { 0.0 })),
        mean(side.iter().filter_map(|r| r.score)),
    )
}

/// The number of runs of one side that have a rubric score.
fn scored(runs: &[&Run], skill: bool) -> usize {
    runs.iter()
        .filter(|r| r.skill == skill && r.score.is_some())
        .count()
}

pub struct HostGate {
    pub host: String,
    pub pass_with: f64,
    pub pass_without: f64,
    pub rubric_with: f64,
    pub rubric_without: f64,
    /// Runs with the skill that have a rubric score.
    pub scored_with: usize,
    /// Runs without the skill that have a rubric score.
    pub scored_without: usize,
}

impl HostGate {
    /// Whether both sides have at least one scored run. A mean over no run
    /// is 0.0, which would let a side with no evidence pass the gate.
    pub fn decided(&self) -> bool {
        self.scored_with > 0 && self.scored_without > 0
    }

    /// The release gate: both sides have scored runs, and the skill is not
    /// worse on either score.
    pub fn holds(&self) -> bool {
        self.decided()
            && self.pass_with >= self.pass_without
            && self.rubric_with >= self.rubric_without
    }

    /// `holds`, `fails` or `undecided`, for the summary and the run's output.
    pub fn verdict(&self) -> &'static str {
        if !self.decided() {
            "undecided (a side has no scored run)"
        } else if self.holds() {
            "holds"
        } else {
            "fails"
        }
    }
}

/// One gate per host that has results, in host-name order.
pub fn gate(out: &Path) -> Result<Vec<HostGate>, String> {
    let runs = runs(out)?;
    let mut by_host: BTreeMap<&str, Vec<&Run>> = BTreeMap::new();
    for run in &runs {
        by_host.entry(run.host.as_str()).or_default().push(run);
    }
    Ok(by_host
        .into_iter()
        .map(|(host, runs)| {
            let (pass_with, rubric_with) = side(&runs, true);
            let (pass_without, rubric_without) = side(&runs, false);
            HostGate {
                host: host.to_owned(),
                pass_with,
                pass_without,
                rubric_with,
                rubric_without,
                scored_with: scored(&runs, true),
                scored_without: scored(&runs, false),
            }
        })
        .collect())
}

/// Up to `size` runs, spread across hosts and skill arms: the runs are
/// grouped by the host and the arm their label names (`<host>-<arm>-<n>`),
/// the groups and the runs in each group are sorted, and the sample takes
/// one run from each group in turn until `size` are taken or every group is
/// empty. Deterministic, so a reviewer can reproduce the sample.
pub fn sample(runs: &[String], size: usize) -> Vec<String> {
    let mut groups: BTreeMap<(String, String), Vec<String>> = BTreeMap::new();
    for key in runs {
        let label = key.split_once('/').map_or(key.as_str(), |(_, l)| l);
        let mut parts = label.split('-');
        let host = parts.next().unwrap_or_default().to_owned();
        let arm = parts.next().unwrap_or_default().to_owned();
        groups.entry((host, arm)).or_default().push(key.clone());
    }
    let mut groups: Vec<std::vec::IntoIter<String>> = groups
        .into_values()
        .map(|mut g| {
            g.sort();
            g.into_iter()
        })
        .collect();
    let mut picked = Vec::new();
    while picked.len() < size {
        let before = picked.len();
        for group in &mut groups {
            if picked.len() == size {
                break;
            }
            picked.extend(group.next());
        }
        if picked.len() == before {
            break;
        }
    }
    picked
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

/// The keys (`<task>/<label>`) of the runs the judge marked.
pub fn judged_runs(out: &Path) -> Result<Vec<String>, String> {
    Ok(runs(out)?
        .into_iter()
        .filter(|r| r.score.is_some() && r.passed)
        .map(|r| r.key)
        .collect())
}

/// Writes `out/summary.md`: per host, one row per task with the pass rates
/// and the mean rubric scores with and without the skill, a row of means,
/// the runs whose judge reply was an error, and the runs with a setup error.
pub fn write_summary(out: &Path) -> Result<(), String> {
    let runs = runs(out)?;
    let mut text = String::from("# Eval results\n");
    for gate in gate(out)? {
        text.push_str(&format!(
            "\n## {}\n\n| Task | Pass rate with | Pass rate without | Rubric with | Rubric without |\n| --- | --- | --- | --- | --- |\n",
            gate.host
        ));
        let mut tasks: BTreeMap<&str, Vec<&Run>> = BTreeMap::new();
        for run in runs.iter().filter(|r| r.host == gate.host) {
            tasks.entry(run.task.as_str()).or_default().push(run);
        }
        for (task, task_runs) in tasks {
            let (pw, rw) = side(&task_runs, true);
            let (po, ro) = side(&task_runs, false);
            text.push_str(&format!(
                "| {task} | {pw:.2} | {po:.2} | {rw:.2} | {ro:.2} |\n"
            ));
        }
        text.push_str(&format!(
            "| **mean** | {:.2} | {:.2} | {:.2} | {:.2} |\n\nGate: {}.\n",
            gate.pass_with,
            gate.pass_without,
            gate.rubric_with,
            gate.rubric_without,
            gate.verdict()
        ));
    }
    let errors: Vec<&str> = runs
        .iter()
        .filter(|r| r.passed && r.score.is_none())
        .map(|r| r.key.as_str())
        .collect();
    if !errors.is_empty() {
        text.push_str("\n## Runs with a judge error\n\n");
        for key in errors {
            text.push_str(&format!("- {key}\n"));
        }
    }
    let setup: Vec<&str> = runs
        .iter()
        .filter(|r| r.setup_error)
        .map(|r| r.key.as_str())
        .collect();
    if !setup.is_empty() {
        text.push_str("\n## Runs with a setup error, left out of every rate and mean\n\n");
        for key in setup {
            text.push_str(&format!("- {key}\n"));
        }
    }
    std::fs::write(out.join("summary.md"), text).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(out: &Path, task: &str, label: &str, skill: bool, passed: bool, score: Option<f64>) {
        let dir = out.join(task);
        std::fs::create_dir_all(&dir).unwrap();
        let rubric = match score {
            Some(s) => format!("[rubric]\nscore = {s:?}\nmet = [true]\n"),
            None => "[rubric]\nerror = \"the judge reply is not JSON\"\n".to_owned(),
        };
        let host = label.split('-').next().unwrap();
        std::fs::write(
            dir.join(format!("{label}.toml")),
            format!(
                "task = \"{task}\"\nhost = \"{host}\"\nskill = {skill}\n[mechanical]\npassed = {passed}\n{rubric}"
            ),
        )
        .unwrap();
    }

    fn out(label: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("xtask-report-{label}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn the_gate_fails_when_the_rubric_score_is_lower_with_the_skill() {
        let out = out("lower");
        write(&out, "design-0001", "claude-with-1", true, true, Some(0.5));
        write(
            &out,
            "design-0001",
            "claude-without-1",
            false,
            true,
            Some(0.6),
        );
        let gates = gate(&out).unwrap();
        assert_eq!(gates.len(), 1);
        assert_eq!(gates[0].host, "claude");
        assert!(!gates[0].holds());
    }

    #[test]
    fn the_gate_holds_on_equal_scores() {
        let out = out("equal");
        write(&out, "design-0001", "codex-with-1", true, false, Some(0.0));
        write(
            &out,
            "design-0001",
            "codex-without-1",
            false,
            false,
            Some(0.0),
        );
        let gates = gate(&out).unwrap();
        assert!(gates[0].holds());
    }

    #[test]
    fn the_gate_fails_when_the_pass_rate_is_lower_with_the_skill() {
        let out = out("pass");
        write(&out, "design-0001", "claude-with-1", true, false, Some(0.0));
        write(
            &out,
            "design-0001",
            "claude-without-1",
            false,
            true,
            Some(0.0),
        );
        assert!(!gate(&out).unwrap()[0].holds());
    }

    #[test]
    fn a_run_with_a_judge_error_is_left_out_of_the_means_and_listed() {
        let out = out("error");
        write(&out, "design-0001", "claude-with-1", true, true, Some(1.0));
        write(&out, "design-0001", "claude-with-2", true, true, None);
        write(
            &out,
            "design-0001",
            "claude-without-1",
            false,
            true,
            Some(0.5),
        );
        let gates = gate(&out).unwrap();
        assert_eq!(gates[0].rubric_with, 1.0);
        write_summary(&out).unwrap();
        let summary = std::fs::read_to_string(out.join("summary.md")).unwrap();
        assert!(summary.contains("design-0001/claude-with-2"), "{summary}");
    }

    #[test]
    fn the_summary_has_one_table_per_host() {
        let out = out("summary");
        write(&out, "design-0001", "claude-with-1", true, true, Some(1.0));
        write(
            &out,
            "design-0001",
            "claude-without-1",
            false,
            false,
            Some(0.0),
        );
        write(&out, "design-0001", "codex-with-1", true, true, Some(1.0));
        write(
            &out,
            "design-0001",
            "codex-without-1",
            false,
            true,
            Some(0.5),
        );
        write_summary(&out).unwrap();
        let summary = std::fs::read_to_string(out.join("summary.md")).unwrap();
        assert!(
            summary.contains("## claude") && summary.contains("## codex"),
            "{summary}"
        );
        assert!(
            summary.contains("| design-0001 | 1.00 | 0.00 | 1.00 | 0.00 |"),
            "{summary}"
        );
    }

    /// The keys of `tasks` tasks with the four labels of one run each.
    fn four_labels(tasks: usize) -> Vec<String> {
        (1..=tasks)
            .flat_map(|t| {
                [
                    "claude-with-1",
                    "claude-without-1",
                    "codex-with-1",
                    "codex-without-1",
                ]
                .map(move |l| format!("design-{t:04}/{l}"))
            })
            .collect()
    }

    #[test]
    fn the_sample_spreads_across_hosts_and_skill_arms() {
        let runs = four_labels(5);
        let picked = sample(&runs, 6);
        assert_eq!(picked.len(), 6);
        for label in [
            "claude-with",
            "claude-without",
            "codex-with",
            "codex-without",
        ] {
            assert!(
                picked.iter().any(|k| k.contains(&format!("/{label}-"))),
                "{label} missing from {picked:?}"
            );
        }
        // Deterministic, and in group order on the first round.
        assert_eq!(sample(&runs, 6), picked);
        assert_eq!(picked[0], "design-0001/claude-with-1");
        assert_eq!(picked[1], "design-0001/claude-without-1");
        assert_eq!(picked[4], "design-0002/claude-with-1");
    }

    #[test]
    fn the_sample_sorts_the_runs_of_each_group() {
        let mut runs = four_labels(5);
        runs.reverse();
        let picked = sample(&runs, 4);
        assert_eq!(
            picked,
            [
                "design-0001/claude-with-1",
                "design-0001/claude-without-1",
                "design-0001/codex-with-1",
                "design-0001/codex-without-1"
            ]
        );
    }

    #[test]
    fn a_directory_with_no_result_file_has_no_results() {
        let out = out("no-results");
        assert!(!has_results(&out).unwrap());
        std::fs::create_dir_all(out.join("transcripts/design-0001")).unwrap();
        std::fs::write(
            out.join("transcripts/design-0001/claude-with-1.jsonl"),
            "{}",
        )
        .unwrap();
        assert!(!has_results(&out).unwrap());
        assert!(!has_results(&out.join("missing")).unwrap());
    }

    #[test]
    fn the_sample_stops_at_its_size_or_when_every_group_is_empty() {
        assert_eq!(sample(&four_labels(12), 20).len(), 20);
        assert_eq!(sample(&four_labels(2), 20).len(), 8);
        assert_eq!(sample(&four_labels(2), 0).len(), 0);
    }

    #[test]
    fn only_passed_runs_with_a_score_are_judged_runs() {
        let out = out("judged");
        write(&out, "design-0001", "claude-with-1", true, true, Some(1.0));
        write(&out, "design-0001", "claude-with-2", true, false, Some(0.0));
        write(&out, "design-0001", "claude-with-3", true, true, None);
        assert_eq!(judged_runs(&out).unwrap(), ["design-0001/claude-with-1"]);
    }

    #[test]
    fn the_summary_mean_row_and_gate_line_follow_the_columns() {
        let out = out("mean");
        write(&out, "design-0001", "claude-with-1", true, true, Some(0.75));
        write(
            &out,
            "design-0001",
            "claude-without-1",
            false,
            false,
            Some(0.25),
        );
        write_summary(&out).unwrap();
        let summary = std::fs::read_to_string(out.join("summary.md")).unwrap();
        assert!(
            summary.contains("| **mean** | 1.00 | 0.00 | 0.75 | 0.25 |\n\nGate: holds.\n"),
            "{summary}"
        );
    }

    #[test]
    fn a_result_file_missing_a_field_is_read_with_its_default() {
        let out = out("missing");
        std::fs::create_dir_all(out.join("design-0001")).unwrap();
        std::fs::write(
            out.join("design-0001/claude-with-1.toml"),
            "task = \"design-0001\"\n",
        )
        .unwrap();
        assert!(has_results(&out).unwrap());
        let gates = gate(&out).unwrap();
        assert_eq!(gates[0].host, "");
        assert_eq!(gates[0].pass_without, 0.0);
    }

    #[test]
    fn agreement_counts_items_not_runs() {
        assert_eq!(
            agreement(&[(vec![true, false], vec![true, true])]).unwrap(),
            0.5
        );
        assert_eq!(
            agreement(&[(vec![true], vec![true]), (vec![false], vec![false])]).unwrap(),
            1.0
        );
    }

    #[test]
    fn an_unlabelled_run_is_an_error() {
        assert_eq!(
            agreement(&[(vec![], vec![true])]).unwrap_err(),
            "run 1 has 0 human labels for 1 rubric items"
        );
    }

    #[test]
    fn a_run_with_a_setup_error_is_left_out_of_every_rate_and_listed() {
        let out = out("setup");
        write(&out, "design-0001", "claude-with-1", true, true, Some(1.0));
        write(
            &out,
            "design-0001",
            "claude-without-1",
            false,
            true,
            Some(0.5),
        );
        std::fs::write(
            out.join("design-0001/claude-with-2.toml"),
            "host = \"claude\"\nskill = true\nsetup_error = \"the host did not load the skill\"\n",
        )
        .unwrap();
        std::fs::write(
            out.join("design-0001/claude-without-2.toml"),
            "host = \"claude\"\nskill = false\nsetup_error = \"the host ran past the timeout\"\n",
        )
        .unwrap();
        let gates = gate(&out).unwrap();
        assert_eq!(gates[0].pass_with, 1.0);
        assert_eq!(gates[0].pass_without, 1.0);
        write_summary(&out).unwrap();
        let summary = std::fs::read_to_string(out.join("summary.md")).unwrap();
        assert!(
            summary.contains("- design-0001/claude-with-2\n- design-0001/claude-without-2\n"),
            "{summary}"
        );
    }

    #[test]
    fn the_gate_is_undecided_when_a_side_has_no_run() {
        let out = out("one-side");
        write(&out, "design-0001", "claude-with-1", true, true, Some(1.0));
        let gates = gate(&out).unwrap();
        assert!(!gates[0].decided());
        assert!(!gates[0].holds());
    }

    #[test]
    fn the_gate_is_undecided_when_no_run_of_a_side_was_scored() {
        let out = out("unscored");
        write(&out, "design-0001", "claude-with-1", true, true, Some(1.0));
        write(&out, "design-0001", "claude-without-1", false, true, None);
        let gates = gate(&out).unwrap();
        assert!(!gates[0].decided());
        assert!(!gates[0].holds());
    }
}
