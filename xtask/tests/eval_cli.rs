//! `cargo xtask eval` over fake host CLIs: no model is called.
#![cfg(unix)]

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

fn script(dir: &Path, name: &str, body: &str) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    path
}

/// A fresh directory with fake `claude`, `codex` and `ridl` scripts, and the
/// `tmp/` directory the eval's run directories are made in. The fake agents
/// write a `ridl.toml` when the workspace has none and print one tool-call
/// line naming `ridl_check`; `claude_body` replaces the fake Claude agent's
/// script when given. Every fake appends its command line to `calls.log`. A
/// fake judge saves the prompt it read as `<host>-judge-prompt.txt`.
///
/// Files in the directory change what a fake prints: `check.json` is the
/// output of `ridl check` on a workspace with a `ridl.toml`, `diff-verdict`
/// the verdict of `ridl diff`, and `codex-verdict` the reply of the Codex
/// judge. The fake `ridl diff` prints nothing and exits 3 unless its third and
/// fourth arguments are the task's starting workspace and the run's workspace.
/// The directory is removed when the value is dropped.
struct Fakes {
    dir: PathBuf,
    claude: PathBuf,
    codex: PathBuf,
    ridl: PathBuf,
}

impl Drop for Fakes {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

impl Fakes {
    /// The command lines the fakes ran, one per line.
    fn calls(&self) -> String {
        std::fs::read_to_string(self.dir.join("calls.log")).unwrap_or_default()
    }

    /// Every path under the eval's temporary directory.
    fn scratch(&self) -> Vec<PathBuf> {
        let mut found = Vec::new();
        let mut stack = vec![self.dir.join("tmp")];
        while let Some(dir) = stack.pop() {
            for entry in std::fs::read_dir(&dir).unwrap().flatten() {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path.clone());
                }
                found.push(path);
            }
        }
        found
    }
}

/// Reads the prompt from stdin and sets `$items` to a verdict entry marking
/// every rubric item the prompt holds as met, so a fake host can act as the
/// judge. `$out` is the value of the `-o` argument, if any.
const JUDGE_ITEMS: &str = r#"[ "$1" = --version ] && { echo "fake-host 1.0"; exit 0; }
prompt=$(cat)
items=$(printf '%s\n' "$prompt" | grep -E '^[0-9]+\. \*\*' | awk -F. '{printf "%s{\"n\":%s,\"met\":true,\"reason\":\"r\"}", (NR>1?",":""), $1}')
out=""; prev=""; for a in "$@"; do [ "$prev" = "-o" ] && out="$a"; prev="$a"; done"#;

fn fakes_with(claude_body: Option<&str>) -> Fakes {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let dir = std::env::temp_dir().join(format!(
        "xtask-eval-fakes-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("tmp")).unwrap();
    let d = dir.display();
    let claude = script(
        &dir,
        "claude",
        &format!(
            "echo \"claude $*\" >> '{d}/calls.log'\n{JUDGE_ITEMS}\ncase \"$*\" in *--json-schema*) printf '%s' \"$prompt\" > '{d}/claude-judge-prompt.txt'; echo \"{{\\\"type\\\":\\\"result\\\",\\\"structured_output\\\":{{\\\"items\\\":[$items]}}}}\"; exit 0;; esac\n{}",
            claude_body.unwrap_or(
                r#"[ -f ridl.toml ] || printf '[package]\n' > ridl.toml; echo '{"type":"assistant","message":{"content":[{"type":"tool_use","name":"mcp__ridl__ridl_check","input":{}}]}}'; echo '{"type":"result","result":"Done."}'"#,
            )
        ),
    );
    let codex = script(
        &dir,
        "codex",
        &format!(
            r#"echo "codex $*" >> '{d}/calls.log'
{JUDGE_ITEMS}
case "$*" in *--output-schema*)
  printf '%s' "$prompt" > '{d}/codex-judge-prompt.txt'
  if [ -f '{d}/codex-verdict' ]; then cat '{d}/codex-verdict' > "$out"; else echo "{{\"items\":[$items]}}" > "$out"; fi
  exit 0;;
esac
[ -f ridl.toml ] || printf '[package]\n' > ridl.toml
echo 'The Codex final answer.' > "$out"
echo '{{"type":"item.completed","item":{{"type":"mcp_tool_call","server":"ridl","tool":"ridl_check"}}}}'"#
        ),
    );
    let ridl = script(
        &dir,
        "ridl",
        &format!(
            r#"echo "ridl $*" >> '{d}/calls.log'
case "$1" in
check) if [ -f "$4/ridl.toml" ]; then if [ -f '{d}/check.json' ]; then cat '{d}/check.json'; else echo '[]'; fi; else echo "error: no ridl.toml in $4" >&2; exit 2; fi;;
diff)
  case "$4" in */evals/corpus/*|*/start) ;; *) exit 3;; esac
  case "$5" in */ws) ;; *) exit 3;; esac
  verdict=compatible; [ -f '{d}/diff-verdict' ] && verdict=$(cat '{d}/diff-verdict')
  echo "{{\"verdict\":\"$verdict\",\"changes\":[]}}";;
esac"#
        ),
    );
    Fakes {
        dir,
        claude,
        codex,
        ridl,
    }
}

/// Which host logins the eval command finds.
#[derive(Clone, Copy)]
enum Logins {
    Both,
    CodexOnly,
    None,
}

fn command(fakes: &Fakes, args: &[&str], logins: Logins) -> Command {
    let stored = fakes.dir.join("auth.json");
    let mut command = Command::new(env!("CARGO_BIN_EXE_xtask"));
    command
        .arg("eval")
        .args(args)
        .env("RIDL_EVAL_CLAUDE", &fakes.claude)
        .env("RIDL_EVAL_CODEX", &fakes.codex)
        .env("RIDL_EVAL_RIDL", &fakes.ridl)
        .env("RIDL_EVAL_CODEX_LOGIN", &stored)
        .env("TMPDIR", fakes.dir.join("tmp"))
        .env_remove("ANTHROPIC_API_KEY")
        .env_remove("CLAUDE_CODE_OAUTH_TOKEN");
    match logins {
        Logins::Both => {
            std::fs::write(&stored, "{}").unwrap();
            command.env("CLAUDE_CODE_OAUTH_TOKEN", "fake");
        }
        Logins::CodexOnly => std::fs::write(&stored, "{}").unwrap(),
        Logins::None => {}
    }
    command
}

fn eval(fakes: &Fakes, args: &[&str], logins: bool) -> Output {
    let logins = if logins { Logins::Both } else { Logins::None };
    command(fakes, args, logins).output().unwrap()
}

#[test]
fn a_run_copies_its_transcript() {
    let fakes = fakes_with(None);
    let out = fakes.dir.join("results");
    let output = eval(
        &fakes,
        &[
            "--task",
            "design-0001",
            "--host",
            "claude",
            "--skill",
            "without",
            "--out",
            out.to_str().unwrap(),
        ],
        true,
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let transcript =
        std::fs::read_to_string(out.join("transcripts/design-0001/claude-without-1.jsonl"))
            .unwrap();
    assert!(transcript.contains("mcp__ridl__ridl_check"));
}

#[test]
fn a_codex_run_copies_its_transcript() {
    let fakes = fakes_with(None);
    let out = fakes.dir.join("results");
    let output = eval(
        &fakes,
        &[
            "--task",
            "design-0001",
            "--host",
            "codex",
            "--skill",
            "without",
            "--out",
            out.to_str().unwrap(),
        ],
        true,
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        out.join("transcripts/design-0001/codex-without-1.jsonl")
            .is_file()
    );
}

#[test]
fn a_missing_codex_login_stops_the_run_before_any_host_is_called() {
    let fakes = fakes_with(None);
    let output = eval(
        &fakes,
        &[
            "--task",
            "design-0001",
            "--host",
            "claude",
            "--out",
            fakes.dir.join("results").to_str().unwrap(),
        ],
        false,
    );
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("run `codex login`"));
    // Not even `--version`: no host command runs before both logins are found.
    assert_eq!(fakes.calls(), "");
}

#[test]
fn a_finished_run_and_its_judgement_leave_no_scratch_directory() {
    let fakes = fakes_with(None);
    for host in ["claude", "codex"] {
        let out = fakes.dir.join(format!("results-{host}"));
        let output = eval(
            &fakes,
            &[
                "--task",
                "design-0001",
                "--host",
                host,
                "--skill",
                "without",
                "--out",
                out.to_str().unwrap(),
            ],
            true,
        );
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    assert_eq!(fakes.scratch(), Vec::<PathBuf>::new());
}

#[test]
fn a_run_that_stops_with_an_error_leaves_no_login_copy() {
    // The fake Codex agent never reads the skill, so the "with" run stops
    // with a setup error after the agent ran.
    let fakes = fakes_with(None);
    let output = eval(
        &fakes,
        &[
            "--task",
            "design-0001",
            "--host",
            "codex",
            "--skill",
            "with",
            "--out",
            fakes.dir.join("results").to_str().unwrap(),
        ],
        true,
    );
    assert_eq!(output.status.code(), Some(2));
    assert!(fakes.calls().contains("codex exec"), "{}", fakes.calls());
    let logins: Vec<_> = fakes
        .scratch()
        .into_iter()
        .filter(|p| p.ends_with("auth.json"))
        .collect();
    assert_eq!(logins, Vec::<PathBuf>::new());
}

#[test]
fn a_missing_claude_token_stops_the_run_unless_claude_is_handed_off() {
    let fakes = fakes_with(None);
    let out = fakes.dir.join("results");
    let args = [
        "--task",
        "design-0001",
        "--host",
        "codex",
        "--skill",
        "without",
        "--out",
        out.to_str().unwrap(),
    ];
    let output = command(&fakes, &args, Logins::CodexOnly).output().unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("claude setup-token"));
}

/// Runs the eval with `--claude-handoff`, and acts as the operator: for each
/// request the eval prints, writes `answer(request)` as the output and creates
/// the done file.
fn eval_handed_off(
    fakes: &Fakes,
    args: &[&str],
    answer: impl Fn(&toml::Table) -> String,
) -> Output {
    use std::io::BufRead;
    let mut child = command(fakes, args, Logins::CodexOnly)
        .arg("--claude-handoff")
        .stderr(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let mut stderr = String::new();
    for line in std::io::BufReader::new(child.stderr.take().unwrap()).lines() {
        let line = line.unwrap();
        if let Some(path) = line.strip_prefix("eval: handoff: ") {
            let request: toml::Table =
                toml::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
            std::fs::write(request["output"].as_str().unwrap(), answer(&request)).unwrap();
            std::fs::write(request["done"].as_str().unwrap(), "").unwrap();
        }
        stderr.push_str(&line);
        stderr.push('\n');
    }
    let mut output = child.wait_with_output().unwrap();
    output.stderr = stderr.into_bytes();
    output
}

#[test]
fn a_handed_off_claude_run_is_scored_and_judged_by_codex() {
    let fakes = fakes_with(Some("touch called"));
    let out = fakes.dir.join("results");
    let output = eval_handed_off(
        &fakes,
        &[
            "--task",
            "design-0001",
            "--host",
            "claude",
            "--skill",
            "with",
            "--out",
            out.to_str().unwrap(),
        ],
        |request| {
            let cwd = request["cwd"].as_str().unwrap();
            assert!(request["prompt"].as_str().unwrap().len() > 10);
            // The operator's agent needs the run's own configuration home.
            let home = request["env"]["CLAUDE_CONFIG_DIR"].as_str().unwrap();
            assert_eq!(Path::new(home), Path::new(cwd).with_file_name("home"));
            std::fs::write(format!("{cwd}/ridl.toml"), "[package]\n").unwrap();
            [
                format!(r#"{{"type":"assistant","message":{{"content":[{{"type":"tool_use","name":"Read","input":{{"file_path":"{cwd}/.claude/skills/ridl-authoring/SKILL.md"}}}}]}}}}"#),
                r#"{"type":"assistant","message":{"content":[{"type":"tool_use","name":"mcp__ridl__ridl_check","input":{}}]}}"#.to_owned(),
                r#"{"type":"assistant","message":{"content":[{"type":"text","text":"Done."}]}}"#.to_owned(),
            ]
            .join("\n")
        },
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(stderr.matches("eval: handoff: ").count(), 1, "{stderr}");
    let result = result(&out, "design-0001", "claude-with-1");
    assert_eq!(result["agent_exit"].as_integer(), Some(-1));
    // No claude process ran, so the pinned model is not claimed.
    assert_eq!(result["handoff"].as_bool(), Some(true));
    assert_eq!(result["model"].as_str(), Some("handoff"));
    assert_eq!(result["rubric"]["handoff"].as_bool(), Some(false));
    assert_eq!(
        result["rubric"]["judge_model"].as_str(),
        Some(pinned_model("codex").as_str())
    );
    assert_eq!(result["mechanical"]["passed"].as_bool(), Some(true));
    assert_eq!(result["rubric"]["judge_host"].as_str(), Some("codex"));
    assert_eq!(result["rubric"]["score"].as_float(), Some(1.0));
}

#[test]
fn the_claude_judge_of_a_codex_run_is_handed_off() {
    let fakes = fakes_with(None);
    let out = fakes.dir.join("results");
    let output = eval_handed_off(
        &fakes,
        &[
            "--task",
            "design-0001",
            "--host",
            "codex",
            "--skill",
            "without",
            "--out",
            out.to_str().unwrap(),
        ],
        |request| {
            let prompt = request["prompt"].as_str().unwrap();
            let items: Vec<String> = prompt
                .lines()
                .filter_map(|l| l.split_once(". **"))
                .filter_map(|(n, _)| n.parse::<u32>().ok())
                .map(|n| format!(r#"{{"n":{n},"met":true,"reason":"r"}}"#))
                .collect();
            format!(
                r#"{{"structured_output":{{"items":[{}]}}}}"#,
                items.join(",")
            )
        },
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(stderr.matches("eval: handoff: ").count(), 1, "{stderr}");
    let codex = result(&out, "design-0001", "codex-without-1");
    assert_eq!(codex["rubric"]["judge_host"].as_str(), Some("claude"));
    assert_eq!(codex["rubric"]["score"].as_float(), Some(1.0));
    assert_eq!(codex["handoff"].as_bool(), Some(false));
    assert_eq!(
        codex["model"].as_str(),
        Some(pinned_model("codex").as_str())
    );
    assert_eq!(codex["rubric"]["handoff"].as_bool(), Some(true));
    assert_eq!(codex["rubric"]["judge_model"].as_str(), Some("handoff"));
}

/// The model `evals/runner.toml` pins for a host.
fn pinned_model(host: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../evals/runner.toml");
    let runner: toml::Value = toml::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    runner[host]["model"].as_str().unwrap().to_owned()
}

#[test]
fn each_run_and_judgement_uses_and_records_its_host_pinned_model() {
    let fakes = fakes_with(None);
    let (claude, codex) = (pinned_model("claude"), pinned_model("codex"));
    assert_ne!(claude, codex);
    for host in ["claude", "codex"] {
        let out = fakes.dir.join(format!("results-{host}"));
        let output = eval(
            &fakes,
            &[
                "--task",
                "design-0001",
                "--host",
                host,
                "--skill",
                "without",
                "--out",
                out.to_str().unwrap(),
            ],
            true,
        );
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let result = result(&out, "design-0001", &format!("{host}-without-1"));
        let (model, judge_model) = if host == "claude" {
            (&claude, &codex)
        } else {
            (&codex, &claude)
        };
        assert_eq!(result["model"].as_str(), Some(model.as_str()));
        assert_eq!(result["handoff"].as_bool(), Some(false));
        assert_eq!(
            result["rubric"]["judge_model"].as_str(),
            Some(judge_model.as_str())
        );
        assert_eq!(result["rubric"]["handoff"].as_bool(), Some(false));
    }
    let calls = fakes.calls();
    let line = |prefix: &str, marker: &str| {
        calls
            .lines()
            .find(|l| l.starts_with(prefix) && l.contains(marker))
            .unwrap_or_else(|| panic!("no {prefix} call with {marker}:\n{calls}"))
            .to_owned()
    };
    let claude_agent = line("claude -p", "--mcp-config");
    let claude_judge = line("claude -p", "--json-schema");
    let codex_agent = line("codex exec", "--json");
    let codex_judge = line("codex exec", "--output-schema");
    assert!(
        claude_agent.contains(&format!("--model {claude} ")),
        "{claude_agent}"
    );
    assert!(
        codex_judge.contains(&format!("-m {codex} ")),
        "{codex_judge}"
    );
    assert!(
        codex_agent.contains(&format!("-m {codex} ")),
        "{codex_agent}"
    );
    assert!(
        claude_judge.contains(&format!("--model {claude} ")),
        "{claude_judge}"
    );
}

#[test]
fn an_out_directory_that_holds_results_needs_append() {
    let fakes = fakes_with(None);
    let out = fakes.dir.join("results");
    let args = |append: bool| {
        let mut args = vec![
            "--task",
            "design-0001",
            "--host",
            "claude",
            "--skill",
            "without",
            "--runs",
            "2",
            "--out",
            out.to_str().unwrap(),
        ];
        if append {
            args.push("--append");
        }
        args.into_iter().map(str::to_owned).collect::<Vec<_>>()
    };
    let run = |args: Vec<String>| {
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        eval(&fakes, &args, true)
    };
    let first = run(args(false));
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    assert!(out.join("design-0001/claude-without-1.toml").is_file());
    assert!(out.join("design-0001/claude-without-2.toml").is_file());
    let second = run(args(false));
    assert_eq!(second.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&second.stderr);
    assert!(
        stderr.contains("already holds results") && stderr.contains("--append"),
        "{stderr}"
    );
    let calls = fakes.calls().lines().count();
    let appended = run(args(true));
    assert!(
        appended.status.success(),
        "{}",
        String::from_utf8_lossy(&appended.stderr)
    );
    assert!(fakes.calls().lines().count() > calls);
}

#[test]
fn an_unknown_task_is_an_error() {
    let fakes = fakes_with(None);
    let output = eval(&fakes, &["--task", "nope-0001"], true);
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("unknown task: nope-0001"));
}

#[test]
fn an_unknown_argument_is_an_error() {
    let fakes = fakes_with(None);
    let output = eval(&fakes, &["--runs", "0"], true);
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("--runs takes a positive number"));
}

fn result(out: &Path, task: &str, label: &str) -> toml::Value {
    let text = std::fs::read_to_string(out.join(task).join(format!("{label}.toml")))
        .unwrap_or_else(|e| panic!("read the result of {task} {label}: {e}"));
    toml::from_str(&text).unwrap()
}

#[test]
fn a_run_writes_its_mechanical_score() {
    let fakes = fakes_with(None);
    let out = fakes.dir.join("results");
    let output = eval(
        &fakes,
        &[
            "--task",
            "evolve-0001",
            "--host",
            "claude",
            "--skill",
            "without",
            "--out",
            out.to_str().unwrap(),
        ],
        true,
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let result = result(&out, "evolve-0001", "claude-without-1");
    assert_eq!(result["task"].as_str(), Some("evolve-0001"));
    assert_eq!(result["host_version"].as_str(), Some("fake-host 1.0"));
    assert_eq!(result["skill"].as_bool(), Some(false));
    let mechanical = &result["mechanical"];
    assert_eq!(mechanical["no_error"].as_bool(), Some(true));
    assert_eq!(mechanical["verify_loop"].as_bool(), Some(true));
    assert_eq!(mechanical["verify_via"].as_str(), Some("mcp"));
    assert_eq!(mechanical["diff_matches"].as_bool(), Some(true));
    assert_eq!(mechanical["passed"].as_bool(), Some(true));
}

#[test]
fn a_deleted_manifest_fails_the_check_with_a_note() {
    let fakes = fakes_with(Some(
        r#"rm -f ridl.toml; echo '{"type":"tool_use","name":"mcp__ridl__ridl_check"}'"#,
    ));
    let out = fakes.dir.join("results");
    let output = eval(
        &fakes,
        &[
            "--task",
            "evolve-0001",
            "--host",
            "claude",
            "--skill",
            "without",
            "--out",
            out.to_str().unwrap(),
        ],
        true,
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let mechanical = result(&out, "evolve-0001", "claude-without-1")["mechanical"].clone();
    assert_eq!(mechanical["no_error"].as_bool(), Some(false));
    assert_eq!(mechanical["passed"].as_bool(), Some(false));
    let notes = mechanical["notes"].as_array().unwrap();
    assert!(
        notes
            .iter()
            .any(|n| n.as_str().unwrap().contains("ridl.toml")),
        "{notes:?}"
    );
}

#[test]
fn a_with_run_whose_host_did_not_load_the_skill_is_a_setup_error() {
    let fakes = fakes_with(None);
    let output = eval(
        &fakes,
        &[
            "--task",
            "design-0001",
            "--host",
            "claude",
            "--skill",
            "with",
            "--out",
            fakes.dir.join("results").to_str().unwrap(),
        ],
        true,
    );
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("did not load the skill"));
}

#[test]
fn a_passed_run_is_judged_by_the_other_host_and_summarised() {
    let fakes = fakes_with(None);
    let out = fakes.dir.join("results");
    for host in ["claude", "codex"] {
        let output = eval(
            &fakes,
            &[
                "--task",
                "design-0001",
                "--host",
                host,
                "--skill",
                "without",
                "--out",
                out.to_str().unwrap(),
                "--append",
            ],
            true,
        );
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let claude = result(&out, "design-0001", "claude-without-1");
    assert_eq!(claude["rubric"]["judge_host"].as_str(), Some("codex"));
    assert_eq!(claude["rubric"]["score"].as_float(), Some(1.0));
    let codex = result(&out, "design-0001", "codex-without-1");
    assert_eq!(codex["rubric"]["judge_host"].as_str(), Some("claude"));
    assert_eq!(codex["rubric"]["score"].as_float(), Some(1.0));
    let summary = std::fs::read_to_string(out.join("summary.md")).unwrap();
    assert!(summary.contains("## claude") && summary.contains("## codex"));
}

#[test]
fn a_failed_run_is_not_judged() {
    let fakes = fakes_with(Some(r#"echo '{"type":"result","result":"no tools used"}'"#));
    let out = fakes.dir.join("results");
    let output = eval(
        &fakes,
        &[
            "--task",
            "design-0001",
            "--host",
            "claude",
            "--skill",
            "without",
            "--out",
            out.to_str().unwrap(),
        ],
        true,
    );
    assert!(output.status.success());
    let rubric = result(&out, "design-0001", "claude-without-1")["rubric"].clone();
    assert_eq!(rubric["score"].as_float(), Some(0.0));
    assert_eq!(rubric["skipped"].as_str(), Some("mechanical checks failed"));
}

fn labels(fakes: &Fakes, human: &str, judge: &str) -> PathBuf {
    let path = fakes.dir.join("labels.toml");
    std::fs::write(
        &path,
        format!(
            "[[run]]\ntask = \"design-0001\"\nlabel = \"claude-with-1\"\nitems = [\"a\", \"b\"]\njudge = {judge}\nhuman = {human}\n"
        ),
    )
    .unwrap();
    path
}

/// A labels file of `items` items, on `agreed` of which the maintainer and
/// the judge agree.
fn labels_agreeing(fakes: &Fakes, items: usize, agreed: usize) -> PathBuf {
    let judge = vec!["true"; items].join(", ");
    let human: Vec<&str> = (0..items)
        .map(|i| if i < agreed { "true" } else { "false" })
        .collect();
    labels(
        fakes,
        &format!("[{}]", human.join(", ")),
        &format!("[{judge}]"),
    )
}

#[test]
fn an_unlabelled_run_is_an_input_error() {
    let fakes = fakes_with(None);
    let path = labels(&fakes, "[]", "[true, true]");
    let output = eval(&fakes, &["agreement", path.to_str().unwrap()], true);
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("0 human labels"));
}

#[test]
fn agreement_below_the_floor_fails() {
    let fakes = fakes_with(None);
    let path = labels(&fakes, "[true, false]", "[true, true]");
    let output = eval(&fakes, &["agreement", path.to_str().unwrap()], true);
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stdout).contains("50.0%"));
}

#[test]
fn agreement_at_the_floor_passes() {
    let fakes = fakes_with(None);
    let path = labels_agreeing(&fakes, 5, 4);
    let output = eval(&fakes, &["agreement", path.to_str().unwrap()], true);
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("80.0%"));
}

#[test]
fn agreement_just_below_the_floor_fails_and_never_shows_the_floor() {
    let fakes = fakes_with(None);
    // 1999 of 2500 is 79.96%, which rounding would show as 80.0%.
    let path = labels_agreeing(&fakes, 2500, 1999);
    let output = eval(&fakes, &["agreement", path.to_str().unwrap()], true);
    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("79.9%"), "{stdout}");
    let report = std::fs::read_to_string(fakes.dir.join("agreement.md")).unwrap();
    assert!(
        report.contains("Agreement: 79.9% (floor 80.0%)."),
        "{report}"
    );
}

#[test]
fn a_labels_file_without_human_labels_is_an_input_error() {
    let fakes = fakes_with(None);
    let path = fakes.dir.join("labels.toml");
    std::fs::write(
        &path,
        "[[run]]\ntask = \"design-0001\"\nlabel = \"claude-with-1\"\njudge = [true]\n",
    )
    .unwrap();
    let output = eval(&fakes, &["agreement", path.to_str().unwrap()], true);
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("has no human"));
}

/// Runs one task on one host without the skill into a fresh `--out`, and
/// returns that directory and the run's result.
fn run_one(fakes: &Fakes, task: &str, host: &str) -> (PathBuf, toml::Value) {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let out = fakes
        .dir
        .join(format!("results-{}", NEXT.fetch_add(1, Ordering::Relaxed)));
    let output = eval(
        fakes,
        &[
            "--task",
            task,
            "--host",
            host,
            "--skill",
            "without",
            "--out",
            out.to_str().unwrap(),
        ],
        true,
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let result = result(&out, task, &format!("{host}-without-1"));
    (out, result)
}

#[test]
fn an_error_diagnostic_fails_the_run() {
    let fakes = fakes_with(None);
    std::fs::write(
        fakes.dir.join("check.json"),
        r#"[{"code":"TYPL-101","severity":"error","message":"m"}]"#,
    )
    .unwrap();
    let (_, result) = run_one(&fakes, "design-0001", "claude");
    let mechanical = &result["mechanical"];
    assert_eq!(mechanical["no_error"].as_bool(), Some(false));
    assert_eq!(mechanical["lints_match"].as_bool(), Some(true));
    assert_eq!(mechanical["passed"].as_bool(), Some(false));
}

#[test]
fn design_lints_must_match_the_task() {
    let fakes = fakes_with(None);
    std::fs::write(
        fakes.dir.join("check.json"),
        r#"[{"code":"TYPL-224","severity":"info","lint":"low-cohesion-interface","message":"m"}]"#,
    )
    .unwrap();
    // review-0002 expects this lint; design-0001 expects none.
    let (_, expected) = run_one(&fakes, "review-0002", "claude");
    assert_eq!(expected["mechanical"]["lints_match"].as_bool(), Some(true));
    assert_eq!(expected["mechanical"]["passed"].as_bool(), Some(true));
    let (_, unexpected) = run_one(&fakes, "design-0001", "claude");
    assert_eq!(unexpected["mechanical"]["no_error"].as_bool(), Some(true));
    assert_eq!(
        unexpected["mechanical"]["lints_match"].as_bool(),
        Some(false)
    );
    assert_eq!(unexpected["mechanical"]["passed"].as_bool(), Some(false));
}

#[test]
fn an_evolve_run_must_reach_the_expected_diff_verdict() {
    let fakes = fakes_with(None);
    std::fs::write(fakes.dir.join("diff-verdict"), "breaking").unwrap();
    let (_, result) = run_one(&fakes, "evolve-0001", "claude");
    assert_eq!(result["mechanical"]["diff_matches"].as_bool(), Some(false));
    assert_eq!(result["mechanical"]["passed"].as_bool(), Some(false));
}

#[test]
fn a_judge_reply_that_cannot_be_read_is_an_error_with_no_score() {
    let fakes = fakes_with(None);
    std::fs::write(fakes.dir.join("codex-verdict"), "It looks fine to me.").unwrap();
    let (out, result) = run_one(&fakes, "design-0001", "claude");
    let rubric = result["rubric"].as_table().unwrap();
    assert_eq!(
        rubric.get("error").and_then(toml::Value::as_str),
        Some("the judge reply is not JSON")
    );
    assert!(rubric.get("score").is_none(), "{rubric:?}");
    assert!(rubric.get("met").is_none(), "{rubric:?}");
    let summary = std::fs::read_to_string(out.join("summary.md")).unwrap();
    assert!(
        summary.contains("## Runs with a judge error\n\n- design-0001/claude-without-1"),
        "{summary}"
    );
}

#[test]
fn the_judge_sees_the_workspace_files_and_the_final_message() {
    let fakes = fakes_with(Some(
        r#"printf '[package]\n' > ridl.toml; printf 'package seen.by.judge\n' > a.typl; echo '{"type":"assistant","message":{"content":[{"type":"tool_use","name":"mcp__ridl__ridl_check","input":{}}]}}'; echo '{"type":"result","result":"The Claude final answer."}'"#,
    ));
    run_one(&fakes, "design-0001", "claude");
    let prompt = std::fs::read_to_string(fakes.dir.join("codex-judge-prompt.txt")).unwrap();
    assert!(prompt.contains("### a.typl"), "{prompt}");
    assert!(prompt.contains("package seen.by.judge"), "{prompt}");
    assert!(prompt.contains("The Claude final answer."), "{prompt}");
}

#[test]
fn the_codex_final_message_reaches_the_judge() {
    let fakes = fakes_with(None);
    run_one(&fakes, "design-0001", "codex");
    let prompt = std::fs::read_to_string(fakes.dir.join("claude-judge-prompt.txt")).unwrap();
    assert!(prompt.contains("The Codex final answer."), "{prompt}");
}

#[test]
fn sample_writes_the_judged_runs_for_labelling() {
    let fakes = fakes_with(None);
    let (out, _) = run_one(&fakes, "design-0001", "claude");
    let output = eval(&fakes, &["sample", "--from", out.to_str().unwrap()], true);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let labels: toml::Value =
        toml::from_str(&std::fs::read_to_string(out.join("judge-check/labels.toml")).unwrap())
            .unwrap();
    let runs = labels["run"].as_array().unwrap();
    assert_eq!(runs.len(), 1);
    let run = &runs[0];
    assert_eq!(run["task"].as_str(), Some("design-0001"));
    assert_eq!(run["label"].as_str(), Some("claude-without-1"));
    let rubric = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../evals/tasks/design-0001/rubric.md"),
    )
    .unwrap();
    let count = rubric
        .lines()
        .filter(|l| {
            l.split_once(". **")
                .is_some_and(|(n, _)| n.parse::<u32>().is_ok())
        })
        .count();
    let items = run["items"].as_array().unwrap();
    assert_eq!(items.len(), count);
    // The first item's text, which continues on the next lines.
    let first = rubric.lines().next().unwrap().split_once("** ").unwrap().1;
    assert!(items[0].as_str().unwrap().starts_with(first), "{items:?}");
    assert_eq!(
        run["judge"].as_array().unwrap(),
        &vec![toml::Value::Boolean(true); count]
    );
    assert_eq!(run["human"].as_array().unwrap().len(), 0);
}

#[test]
fn sample_reports_a_result_without_marks_as_an_error() {
    let fakes = fakes_with(None);
    let out = fakes.dir.join("results");
    std::fs::create_dir_all(out.join("design-0001")).unwrap();
    std::fs::write(
        out.join("design-0001/claude-with-1.toml"),
        "host = \"claude\"\nskill = true\n[mechanical]\npassed = true\n[rubric]\nscore = 1.0\n",
    )
    .unwrap();
    let output = eval(&fakes, &["sample", "--from", out.to_str().unwrap()], true);
    assert_eq!(output.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("has no rubric.met"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
