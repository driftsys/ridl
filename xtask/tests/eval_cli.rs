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

/// A fresh directory with fake `claude`, `codex` and `ridl` scripts. The fake
/// agents write a `ridl.toml` when the workspace has none and print one
/// tool-call line naming `ridl_check`; `claude_body` replaces
/// the fake Claude agent's script when given.
struct Fakes {
    dir: PathBuf,
    claude: PathBuf,
    codex: PathBuf,
    ridl: PathBuf,
}

/// Reads the prompt from stdin and sets `$items` to a verdict entry marking
/// every rubric item the prompt holds as met, so a fake host can act as the
/// judge.
const JUDGE_ITEMS: &str = r#"prompt=$(cat)
items=$(printf '%s\n' "$prompt" | grep -E '^[0-9]+\. \*\*' | awk -F. '{printf "%s{\"n\":%s,\"met\":true,\"reason\":\"r\"}", (NR>1?",":""), $1}')"#;

fn fakes_with(claude_body: Option<&str>) -> Fakes {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let dir = std::env::temp_dir().join(format!(
        "xtask-eval-fakes-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let claude = script(
        &dir,
        "claude",
        &format!(
            "{JUDGE_ITEMS}\ncase \"$*\" in *--json-schema*) echo \"{{\\\"type\\\":\\\"result\\\",\\\"structured_output\\\":{{\\\"items\\\":[$items]}}}}\"; exit 0;; esac\n{}",
            claude_body.unwrap_or(
                r#"[ -f ridl.toml ] || printf '[package]\n' > ridl.toml; echo '{"type":"assistant","message":{"content":[{"type":"tool_use","name":"mcp__ridl__ridl_check","input":{}}]}}'; echo '{"type":"result","result":"Done."}'"#,
            )
        ),
    );
    let codex = script(
        &dir,
        "codex",
        &format!(
            "{JUDGE_ITEMS}\n{}",
            r#"[ "$1" = login ] && exit 0
case "$*" in *--output-schema*)
  out=""; prev=""; for a in "$@"; do [ "$prev" = "-o" ] && out="$a"; prev="$a"; done
  echo "{\"items\":[$items]}" > "$out"; exit 0;;
esac
[ -f ridl.toml ] || printf '[package]\n' > ridl.toml
echo '{"type":"item.completed","item":{"type":"mcp_tool_call","server":"ridl","tool":"ridl_check"}}'"#
        ),
    );
    let ridl = script(
        &dir,
        "ridl",
        r#"case "$1" in check) if [ -f "$4/ridl.toml" ]; then echo '[]'; else echo "error: no ridl.toml in $4" >&2; exit 2; fi;; diff) echo '{"verdict":"compatible","changes":[]}';; esac"#,
    );
    Fakes {
        dir,
        claude,
        codex,
        ridl,
    }
}

fn eval(fakes: &Fakes, args: &[&str], keys: bool) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_xtask"));
    command
        .arg("eval")
        .args(args)
        .env("RIDL_EVAL_CLAUDE", &fakes.claude)
        .env("RIDL_EVAL_CODEX", &fakes.codex)
        .env("RIDL_EVAL_RIDL", &fakes.ridl);
    if keys {
        command
            .env("ANTHROPIC_API_KEY", "fake")
            .env("OPENAI_API_KEY", "fake");
    } else {
        command
            .env_remove("ANTHROPIC_API_KEY")
            .env_remove("OPENAI_API_KEY");
    }
    command.output().unwrap()
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
fn an_unset_key_stops_the_run_before_any_host_is_called() {
    let marker_dir = fakes_with(None).dir;
    let fakes = fakes_with(Some(&format!("touch {}/called", marker_dir.display())));
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
    assert!(String::from_utf8_lossy(&output.stderr).contains("is not set"));
    assert!(!marker_dir.join("called").exists());
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
    assert_eq!(result["skill"].as_bool(), Some(false));
    let mechanical = &result["mechanical"];
    assert_eq!(mechanical["no_error"].as_bool(), Some(true));
    assert_eq!(mechanical["verify_loop"].as_bool(), Some(true));
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
