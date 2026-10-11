//! `cargo xtask eval`: runs the eval tasks with and without the
//! ridl-authoring skill on each host, scores each run, and writes the results
//! under `evals/results/<date>/`.
//!
//! `cargo xtask eval [--task <id>]... [--host claude|codex]... [--runs <n>]
//! [--skill with|without|both] [--out <dir>]`

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
}

pub fn parse(args: &[String]) -> Result<Options, String> {
    let mut options = Options {
        tasks: Vec::new(),
        hosts: Vec::new(),
        runs: 1,
        skill: vec![true, false],
        out: None,
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
    let options = parse(args)?;
    let tasks = task::load_tasks(&root, &options.tasks)?;
    let models = host::load_models(&root)?;
    check_keys()?;
    let out = options
        .out
        .clone()
        .unwrap_or_else(|| root.join("evals/results").join(today()));
    let skill = root.join("skills/ridl-authoring");
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
                    let base = scratch_dir(&task.id, &label)?;
                    let dirs = host::prepare_run(&base, task, host, with_skill, &skill)?;
                    if host == Host::Codex {
                        codex_login(&dirs.home)?;
                    }
                    let invocation =
                        host::agent_invocation(host, host.model(&models), &dirs, &prompt);
                    let status = host::run(&invocation, &dirs.transcript)?;
                    eprintln!("eval: {} {label}: agent exited {status}", task.id);
                    let transcript = std::fs::read_to_string(&dirs.transcript).unwrap_or_default();
                    if with_skill && !score::skill_was_loaded(host, &transcript) {
                        return Err(format!(
                            "{} {label}: the host did not load the skill; this is a setup error, not a result",
                            task.id
                        ));
                    }
                    let mechanical = score::score_mechanical(task, &dirs, &transcript);
                    let mut result = toml::Table::new();
                    result.insert("task".into(), task.id.clone().into());
                    result.insert("host".into(), host.name().into());
                    result.insert("model".into(), host.model(&models).into());
                    result.insert("skill".into(), with_skill.into());
                    result.insert("run".into(), i64::from(n).into());
                    result.insert(
                        "agent_exit".into(),
                        i64::from(status.code().unwrap_or(-1)).into(),
                    );
                    result.insert("mechanical".into(), mechanical_table(&mechanical).into());
                    let rubric = if mechanical.passed() {
                        judge_run(task, host, &models, &dirs, &transcript, &label)?
                    } else {
                        let mut skipped = toml::Table::new();
                        skipped.insert("score".into(), 0.0.into());
                        skipped.insert("skipped".into(), "mechanical checks failed".into());
                        skipped
                    };
                    result.insert("rubric".into(), rubric.into());
                    let result_dir = out.join(&task.id);
                    std::fs::create_dir_all(&result_dir).map_err(|e| e.to_string())?;
                    std::fs::write(
                        result_dir.join(format!("{label}.toml")),
                        toml::to_string(&result).map_err(|e| e.to_string())?,
                    )
                    .map_err(|e| e.to_string())?;
                    let transcripts = out.join("transcripts").join(&task.id);
                    std::fs::create_dir_all(&transcripts).map_err(|e| e.to_string())?;
                    std::fs::copy(&dirs.transcript, transcripts.join(format!("{label}.jsonl")))
                        .map_err(|e| e.to_string())?;
                }
            }
        }
    }
    report::write_summary(&out)?;
    for gate in report::gate(&out)? {
        eprintln!(
            "eval: gate {} {} (pass rate {:.2} with, {:.2} without; rubric {:.2} with, {:.2} without)",
            gate.host,
            if gate.holds() { "holds" } else { "fails" },
            gate.pass_with,
            gate.pass_without,
            gate.rubric_with,
            gate.rubric_without
        );
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
    let dir = scratch_dir(&task.id, &format!("{label}-judge"))?;
    std::fs::create_dir_all(dir.join("home")).map_err(|e| e.to_string())?;
    judge::write_schema(&dir)?;
    if judge == Host::Codex {
        codex_login(&dir.join("home"))?;
    }
    let invocation = judge::judge_invocation(judge, judge.model(models), &dir, &text);
    let stdout = dir.join("stdout.json");
    host::run(&invocation, &stdout)?;
    let reply = match judge {
        Host::Claude => read(&stdout).and_then(|out| judge::claude_reply(&out)),
        Host::Codex => read(&dir.join("verdict.json")),
    };
    let mut table = toml::Table::new();
    table.insert("judge_host".into(), judge.name().into());
    table.insert("judge_model".into(), judge.model(models).into());
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
    table.insert("passed".into(), mechanical.passed().into());
    table.insert(
        "notes".into(),
        toml::Value::Array(mechanical.notes.iter().map(|n| n.clone().into()).collect()),
    );
    table
}

/// The judge of each host is the other host, so both keys are needed for any
/// run. An eval run reads no stored login: Claude runs with `--bare` and each
/// Codex run has a fresh home.
fn check_keys() -> Result<(), String> {
    for var in ["ANTHROPIC_API_KEY", "OPENAI_API_KEY"] {
        if std::env::var(var).map_or(true, |v| v.is_empty()) {
            return Err(format!(
                "{var} is not set; an eval run reads no stored login"
            ));
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
    if status.success() {
        Ok(())
    } else {
        Err(format!("codex login failed: {status}"))
    }
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
    let out = std::process::Command::new("date")
        .arg("+%Y-%m-%d")
        .output()
        .expect("run date");
    String::from_utf8_lossy(&out.stdout).trim().to_owned()
}
