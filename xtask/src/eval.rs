//! `cargo xtask eval`: runs the eval tasks with and without the
//! ridl-authoring skill on each host, scores each run, and writes the results
//! under `evals/results/<date>/`.
//!
//! `cargo xtask eval [--task <id>]... [--host claude|codex]... [--runs <n>]
//! [--skill with|without|both] [--out <dir>] [--append] [--claude-handoff]
//! [--timeout <seconds>]`
//!
//! A run that cannot be scored is recorded with a `setup_error` and no
//! score, and the batch goes on: a "with" run that shows no evidence of
//! loading the skill, or a host that did not run (it could not be started,
//! exited with an error and printed nothing, or ran past `--timeout`, 1800
//! seconds by default, which also bounds the wait for a handed-off run). Such
//! a run is left out of the pass rates and the rubric means and listed in the
//! summary.
//!
//! The summary and the gate read every result under `--out`, so the eval
//! refuses an `--out` that already holds results, since they would be merged
//! into the new summary and gate. `--append` allows it, for adding runs to an
//! earlier invocation's results on purpose.
//!
//! Each host signs in with its CLI's own login. Codex runs get a copy of the
//! stored `auth.json`. Claude runs read `CLAUDE_CODE_OAUTH_TOKEN`, which
//! `claude setup-token` prints, or `ANTHROPIC_API_KEY`. With
//! `--claude-handoff`, no Claude command runs: each Claude agent run and each
//! Claude judgement is written as a request file, and the eval waits until the
//! operator has run it with an agent of their own (see `host::execute`).

mod host;
mod judge;
mod report;
mod score;
mod task;

use host::Host;
use std::path::{Path, PathBuf};

pub struct Options {
    pub tasks: Vec<String>,
    pub hosts: Vec<Host>,
    pub runs: u32,
    /// `true` runs with the skill, `false` without it.
    pub skill: Vec<bool>,
    pub out: Option<PathBuf>,
    /// Write into an `--out` that already holds results.
    pub append: bool,
    pub claude_handoff: bool,
    /// How long one agent run or judgement may take.
    pub timeout: std::time::Duration,
}

pub fn parse(args: &[String]) -> Result<Options, String> {
    let mut options = Options {
        tasks: Vec::new(),
        hosts: Vec::new(),
        runs: 1,
        skill: vec![true, false],
        out: None,
        append: false,
        claude_handoff: false,
        timeout: std::time::Duration::from_secs(1800),
    };
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        let mut value = || {
            iter.next()
                .cloned()
                .ok_or_else(|| format!("{arg} needs a value"))
        };
        match arg.as_str() {
            "--task" => options.tasks.push(value()?),
            "--host" => options.hosts.push(match value()?.as_str() {
                "claude" => Host::Claude,
                "codex" => Host::Codex,
                other => return Err(format!("unknown host: {other}")),
            }),
            "--runs" => {
                options.runs = value()?
                    .parse()
                    .map_err(|_| "--runs takes a positive number".to_owned())?
            }
            "--skill" => {
                options.skill = match value()?.as_str() {
                    "with" => vec![true],
                    "without" => vec![false],
                    "both" => vec![true, false],
                    other => {
                        return Err(format!("--skill takes with, without or both, not {other}"));
                    }
                }
            }
            "--out" => options.out = Some(PathBuf::from(value()?)),
            "--append" => options.append = true,
            "--timeout" => {
                let seconds: u64 = value()?
                    .parse()
                    .map_err(|_| "--timeout takes a number of seconds".to_owned())?;
                if seconds == 0 {
                    return Err("--timeout takes a number of seconds".to_owned());
                }
                options.timeout = std::time::Duration::from_secs(seconds);
            }
            "--claude-handoff" => options.claude_handoff = true,
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
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("the repository root")
        .to_owned();
    match args.first().map(String::as_str) {
        Some("sample") => return sample_command(&root, &args[1..]),
        Some("agreement") => return agreement_command(&args[1..]),
        _ => {}
    }
    let options = parse(args)?;
    let tasks = task::load_tasks(&root, &options.tasks)?;
    let models = host::load_models(&root)?;
    check_logins(options.claude_handoff)?;
    let out = options
        .out
        .clone()
        .unwrap_or_else(|| root.join("evals/results").join(today()));
    if !options.append && report::has_results(&out)? {
        return Err(format!(
            "{} already holds results, which the summary and the gate would merge with this run's; pass another --out, or --append to add to them",
            out.display()
        ));
    }
    let skill = root.join("skills/ridl-authoring");
    let versions: Vec<(Host, String)> = options
        .hosts
        .iter()
        .map(|&h| (h, host_version(h)))
        .collect();
    for task in &tasks {
        let prompt =
            std::fs::read_to_string(task.dir.join("prompt.md")).map_err(|e| e.to_string())?;
        for &host in &options.hosts {
            for &with_skill in &options.skill {
                for n in 1..=options.runs {
                    let label = format!(
                        "{}-{}-{n}",
                        host.name(),
                        if with_skill { "with" } else { "without" }
                    );
                    let base = host::scratch_dir(&task.id, &label)?;
                    let dirs = host::prepare_run(&base, task, host, with_skill, &skill)?;
                    let login = (host == Host::Codex)
                        .then(|| host::LentLogin::lend(&host::codex_login_path(), &dirs.home))
                        .transpose()?;
                    let invocation =
                        host::agent_invocation(host, host.model(&models), &dirs, &prompt);
                    let handoff = options.claude_handoff && host == Host::Claude;
                    let outcome =
                        host::execute(&invocation, &dirs.transcript, handoff, options.timeout);
                    drop(login);
                    eprintln!("eval: {} {label}: {outcome:?}", task.id);
                    let transcript = std::fs::read_to_string(&dirs.transcript).unwrap_or_default();
                    let setup_error = setup_error(&outcome, &transcript, host, with_skill);
                    let code = match outcome {
                        Ok(host::Outcome::Exited(code)) => code,
                        _ => None,
                    };
                    let mut result = toml::Table::new();
                    result.insert("task".into(), task.id.clone().into());
                    result.insert("host".into(), host.name().into());
                    // A handed-off run ran on the operator's agent, not on
                    // the pinned model.
                    let model = if handoff {
                        "handoff"
                    } else {
                        host.model(&models)
                    };
                    result.insert("model".into(), model.into());
                    result.insert("handoff".into(), handoff.into());
                    let version = versions
                        .iter()
                        .find(|(h, _)| *h == host)
                        .map(|(_, v)| v.clone())
                        .unwrap_or_default();
                    result.insert("host_version".into(), version.into());
                    result.insert("skill".into(), with_skill.into());
                    result.insert("run".into(), i64::from(n).into());
                    result.insert("agent_exit".into(), i64::from(code.unwrap_or(-1)).into());
                    if let Some(reason) = setup_error {
                        eprintln!("eval: {} {label}: setup error: {reason}", task.id);
                        result.insert("setup_error".into(), reason.into());
                    } else {
                        let mechanical = score::score_mechanical(task, &dirs, &transcript);
                        result.insert("mechanical".into(), mechanical_table(&mechanical).into());
                        let rubric = if mechanical.passed() {
                            judge_run(task, host, &models, &dirs, &transcript, &label, &options)?
                        } else {
                            let mut skipped = toml::Table::new();
                            skipped.insert("score".into(), 0.0.into());
                            skipped.insert("skipped".into(), "mechanical checks failed".into());
                            skipped
                        };
                        result.insert("rubric".into(), rubric.into());
                    }
                    let result_dir = out.join(&task.id);
                    std::fs::create_dir_all(&result_dir).map_err(|e| e.to_string())?;
                    std::fs::write(
                        result_dir.join(format!("{label}.toml")),
                        toml::to_string(&result).map_err(|e| e.to_string())?,
                    )
                    .map_err(|e| e.to_string())?;
                    let transcripts = out.join("transcripts").join(&task.id);
                    std::fs::create_dir_all(&transcripts).map_err(|e| e.to_string())?;
                    std::fs::write(transcripts.join(format!("{label}.jsonl")), &transcript)
                        .map_err(|e| e.to_string())?;
                    // The result and the transcript are under `out`; nothing
                    // in the run's directory is read again.
                    std::fs::remove_dir_all(&base).map_err(|e| e.to_string())?;
                }
            }
        }
    }
    report::write_summary(&out)?;
    for gate in report::gate(&out)? {
        eprintln!(
            "eval: gate {} {} (pass rate {:.2} with, {:.2} without; rubric {:.2} with, {:.2} without)",
            gate.host,
            gate.verdict(),
            gate.pass_with,
            gate.pass_without,
            gate.rubric_with,
            gate.rubric_without
        );
    }
    Ok(())
}

/// Why a run cannot be scored, or `None` when it can: the host could not be
/// started, ran past the timeout, or exited with an error and wrote no
/// transcript, or a "with" run shows no evidence that the host loaded the
/// skill.
fn setup_error(
    outcome: &Result<host::Outcome, String>,
    transcript: &str,
    host: Host,
    with_skill: bool,
) -> Option<String> {
    match outcome {
        Err(e) => return Some(format!("the host did not run: {e}")),
        Ok(host::Outcome::TimedOut) => return Some("the host ran past the timeout".to_owned()),
        Ok(host::Outcome::Exited(code)) if *code != Some(0) && transcript.trim().is_empty() => {
            return Some(format!(
                "the host exited with {code:?} and wrote no transcript"
            ));
        }
        _ => {}
    }
    (with_skill && !score::skill_was_loaded(host, transcript))
        .then(|| "the host did not load the skill".to_owned())
}

/// The first line `<host> --version` prints, recorded in each result file so
/// a record says which build of the host produced it.
fn host_version(host: Host) -> String {
    std::process::Command::new(host.binary())
        .arg("--version")
        .stdin(std::process::Stdio::null())
        .output()
        .ok()
        .and_then(|out| {
            String::from_utf8_lossy(&out.stdout)
                .lines()
                .next()
                .map(|l| l.trim().to_owned())
        })
        .unwrap_or_else(|| "unknown".to_owned())
}

/// The agreement below which the judge's rubric marks are not trusted.
const AGREEMENT_FLOOR: f64 = 0.8;

/// `cargo xtask eval sample --from <dir> [--size <n>]`: writes
/// `<dir>/judge-check/labels.toml`, one `[[run]]` per sampled judged run
/// (spread across hosts and skill arms, see `report::sample`),
/// with the rubric items, the judge's marks and an empty `human` array for
/// the maintainer to fill.
fn sample_command(root: &Path, args: &[String]) -> Result<(), String> {
    let (mut from, mut size) = (None, 20usize);
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        let value = iter.next().ok_or_else(|| format!("{arg} needs a value"))?;
        match arg.as_str() {
            "--from" => from = Some(PathBuf::from(value)),
            "--size" => {
                size = value
                    .parse()
                    .map_err(|_| "--size takes a positive number".to_owned())?
            }
            other => return Err(format!("unknown argument: {other}")),
        }
    }
    let from = from.ok_or("sample needs --from <results dir>")?;
    let mut runs = Vec::new();
    for key in report::sample(&report::judged_runs(&from)?, size) {
        let (task, label) = key.split_once('/').expect("a run key is task/label");
        let result: toml::Value = toml::from_str(
            &std::fs::read_to_string(from.join(task).join(format!("{label}.toml")))
                .map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        let rubric = std::fs::read_to_string(root.join("evals/tasks").join(task).join("rubric.md"))
            .map_err(|e| e.to_string())?;
        let items: Vec<toml::Value> = judge::rubric_items(&rubric)?
            .into_iter()
            .map(|i| i.text.into())
            .collect();
        let mut run = toml::Table::new();
        run.insert("task".into(), task.into());
        run.insert("label".into(), label.into());
        run.insert("items".into(), toml::Value::Array(items));
        let met = result
            .get("rubric")
            .and_then(|r| r.get("met"))
            .ok_or_else(|| format!("{key}: the result has no rubric.met"))?;
        run.insert("judge".into(), met.clone());
        run.insert("human".into(), toml::Value::Array(Vec::new()));
        runs.push(toml::Value::Table(run));
    }
    let mut labels = toml::Table::new();
    labels.insert("run".into(), toml::Value::Array(runs));
    let dir = from.join("judge-check");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join("labels.toml");
    std::fs::write(&path, toml::to_string(&labels).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    println!("{}", path.display());
    Ok(())
}

/// `cargo xtask eval agreement <labels.toml>`: compares the maintainer's
/// labels with the judge's marks, writes `agreement.md` next to the labels
/// file, prints the agreement, and exits 1 when it is below the floor.
fn agreement_command(args: &[String]) -> Result<(), String> {
    let [path] = args else {
        return Err("agreement takes one labels.toml path".to_owned());
    };
    let path = Path::new(path);
    let labels: toml::Value = toml::from_str(
        &std::fs::read_to_string(path).map_err(|e| format!("read {}: {e}", path.display()))?,
    )
    .map_err(|e| e.to_string())?;
    let flags = |v: &toml::Value| -> Vec<bool> {
        v.as_array()
            .map(|a| a.iter().filter_map(toml::Value::as_bool).collect())
            .unwrap_or_default()
    };
    let runs = labels
        .get("run")
        .and_then(toml::Value::as_array)
        .ok_or("labels.toml has no [[run]]")?;
    let pairs: Vec<(Vec<bool>, Vec<bool>)> = runs
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let field = |key: &str| {
                r.get(key)
                    .map(flags)
                    .ok_or_else(|| format!("run {} has no {key} array", i + 1))
            };
            Ok((field("human")?, field("judge")?))
        })
        .collect::<Result<_, String>>()?;
    let value = report::agreement(&pairs)?;
    let mut text = String::from(
        "# Judge check\n\n| Task | Run | Items agreed | Items |\n| --- | --- | --- | --- |\n",
    );
    for (run, (human, judge)) in runs.iter().zip(&pairs) {
        let agreed = human.iter().zip(judge).filter(|(h, j)| h == j).count();
        text.push_str(&format!(
            "| {} | {} | {agreed} | {} |\n",
            run.get("task")
                .and_then(toml::Value::as_str)
                .unwrap_or_default(),
            run.get("label")
                .and_then(toml::Value::as_str)
                .unwrap_or_default(),
            judge.len()
        ));
    }
    // Rounded down to one decimal, so a value below the floor never shows as
    // the floor. The small offset keeps a value at the floor, such as 0.8,
    // from showing as 79.9 through a binary rounding error.
    let percent = (value * 1000.0 + 1e-6).floor() / 10.0;
    text.push_str(&format!(
        "\nAgreement: {percent:.1}% (floor {:.1}%).\n",
        AGREEMENT_FLOOR * 100.0
    ));
    let dir = path.parent().unwrap_or(Path::new("."));
    std::fs::write(dir.join("agreement.md"), text).map_err(|e| e.to_string())?;
    println!("eval: judge agreement {percent:.1}%");
    if value < AGREEMENT_FLOOR {
        std::process::exit(1);
    }
    Ok(())
}

/// Has the other host's model mark the rubric of a run. A reply that cannot
/// be read becomes an `error` entry with no score, never a partial score.
fn judge_run(
    task: &task::Task,
    host: Host,
    models: &host::Models,
    dirs: &host::RunDirs,
    transcript: &str,
    label: &str,
    options: &Options,
) -> Result<toml::Table, String> {
    let judge = host.other();
    let read = |path: &Path| std::fs::read_to_string(path).map_err(|e| e.to_string());
    let rubric = read(&task.dir.join("rubric.md"))?;
    let items = judge::rubric_items(&rubric)?;
    let text = judge::judge_prompt(
        &read(&task.dir.join("prompt.md"))?,
        &rubric,
        &judge::workspace_files(&dirs.workspace),
        &judge::final_message(host, transcript, dirs),
    );
    let dir = host::scratch_dir(&task.id, &format!("{label}-judge"))?;
    std::fs::create_dir_all(dir.join("home")).map_err(|e| e.to_string())?;
    judge::write_schema(&dir)?;
    let login = (judge == Host::Codex)
        .then(|| host::LentLogin::lend(&host::codex_login_path(), &dir.join("home")))
        .transpose()?;
    let invocation = judge::judge_invocation(judge, judge.model(models), &dir, &text);
    let stdout = dir.join("stdout.json");
    let handoff = options.claude_handoff && judge == Host::Claude;
    let outcome = host::execute(&invocation, &stdout, handoff, options.timeout);
    drop(login);
    let reply = match outcome {
        Err(e) => Err(format!("the judge did not run: {e}")),
        Ok(host::Outcome::TimedOut) => Err("the judge ran past the timeout".to_owned()),
        Ok(_) => match judge {
            Host::Claude => read(&stdout).and_then(|out| judge::claude_reply(&out)),
            Host::Codex => read(&dir.join("verdict.json")),
        },
    };
    std::fs::remove_dir_all(&dir).map_err(|e| e.to_string())?;
    let mut table = toml::Table::new();
    table.insert("judge_host".into(), judge.name().into());
    let model = if handoff {
        "handoff"
    } else {
        judge.model(models)
    };
    table.insert("judge_model".into(), model.into());
    table.insert("handoff".into(), handoff.into());
    match reply.and_then(|reply| judge::parse_verdict(&reply, &items)) {
        Ok(met) => {
            table.insert(
                "met".into(),
                toml::Value::Array(met.iter().map(|m| (*m).into()).collect()),
            );
            table.insert("score".into(), judge::weighted_score(&items, &met).into());
        }
        Err(error) => {
            table.insert("error".into(), error.into());
        }
    }
    Ok(table)
}

fn mechanical_table(mechanical: &score::Mechanical) -> toml::Table {
    let mut table = toml::Table::new();
    table.insert("no_error".into(), mechanical.no_error.into());
    table.insert("lints_match".into(), mechanical.lints_match.into());
    if let Some(matches) = mechanical.diff_matches {
        table.insert("diff_matches".into(), matches.into());
    }
    table.insert("verify_loop".into(), mechanical.verify_loop.into());
    table.insert("verify_via".into(), mechanical.verify_via.into());
    table.insert("passed".into(), mechanical.passed().into());
    table.insert(
        "notes".into(),
        toml::Value::Array(mechanical.notes.iter().map(|n| n.clone().into()).collect()),
    );
    table
}

/// The judge of each host is the other host, so every run uses both hosts:
/// Codex needs its stored login, and Claude, unless it is handed off, needs a
/// token. No command runs before both are found.
fn check_logins(claude_handoff: bool) -> Result<(), String> {
    let stored = host::codex_login_path();
    if !stored.is_file() {
        return Err(format!(
            "no stored Codex login at {}; run `codex login`",
            stored.display()
        ));
    }
    let set = |var: &str| std::env::var(var).is_ok_and(|v| !v.is_empty());
    if !claude_handoff && !set("CLAUDE_CODE_OAUTH_TOKEN") && !set("ANTHROPIC_API_KEY") {
        return Err("CLAUDE_CODE_OAUTH_TOKEN is not set (run `claude setup-token`), nor ANTHROPIC_API_KEY; or pass --claude-handoff".to_owned());
    }
    Ok(())
}

/// Today's date as YYYY-MM-DD, from `date`, so xtask needs no date crate.
fn today() -> String {
    let out = std::process::Command::new("date")
        .arg("+%Y-%m-%d")
        .output()
        .expect("run date");
    String::from_utf8_lossy(&out.stdout).trim().to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_argument_runs_both_hosts_with_and_without_the_skill_once() {
        let options = parse(&[]).unwrap();
        assert_eq!(options.hosts, [Host::Claude, Host::Codex]);
        assert_eq!(options.skill, [true, false]);
        assert_eq!(options.runs, 1);
        assert!(!options.append && !options.claude_handoff);
        assert_eq!(options.timeout, std::time::Duration::from_secs(1800));
        assert!(options.tasks.is_empty() && options.out.is_none());
    }

    #[test]
    fn named_hosts_and_skill_sides_replace_the_defaults() {
        let args: Vec<String> = ["--host", "codex", "--skill", "with", "--runs", "3"]
            .map(str::to_owned)
            .into();
        let options = parse(&args).unwrap();
        assert_eq!(options.hosts, [Host::Codex]);
        assert_eq!(options.skill, [true]);
        assert_eq!(options.runs, 3);
    }
}
