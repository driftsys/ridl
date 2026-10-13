//! Mechanical scoring of one run: the final workspace checks without an
//! error, its design lints match the task, an evolve task's diff verdict
//! matches, and the agent checked its workspace, with the `ridl_check` MCP
//! tool or a shell `ridl check` (recorded as `verify_via`).

use super::host::{Host, RunDirs, ridl_binary};
use super::task::{Kind, Task};
use std::collections::BTreeSet;
use std::process::Command;

/// The shipped design lints, the only lints a task's `expect.lints` names.
pub const DESIGN_LINTS: [&str; 3] = [
    "inconsistent-unit",
    "duplicate-shape",
    "low-cohesion-interface",
];

#[derive(Clone, Debug)]
pub struct Mechanical {
    pub no_error: bool,
    pub lints_match: bool,
    /// `Some` only for an evolve task.
    pub diff_matches: Option<bool>,
    pub verify_loop: bool,
    /// How the agent checked its workspace: `mcp`, `cli`, `both` or `none`.
    pub verify_via: &'static str,
    /// Why a check failed.
    pub notes: Vec<String>,
}

impl Mechanical {
    pub fn passed(&self) -> bool {
        self.no_error && self.lints_match && self.diff_matches != Some(false) && self.verify_loop
    }
}

fn json_lines(transcript: &str) -> impl Iterator<Item = serde_json::Value> + '_ {
    transcript
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
}

/// Every JSON object in `value`, at any depth, `value` itself included.
fn objects(value: &serde_json::Value) -> Vec<&serde_json::Map<String, serde_json::Value>> {
    let mut found = Vec::new();
    let mut stack = vec![value];
    while let Some(v) = stack.pop() {
        match v {
            serde_json::Value::Object(map) => {
                found.push(map);
                stack.extend(map.values());
            }
            serde_json::Value::Array(items) => stack.extend(items),
            _ => {}
        }
    }
    found
}

fn str_field<'a>(
    map: &'a serde_json::Map<String, serde_json::Value>,
    key: &str,
) -> Option<&'a str> {
    map.get(key).and_then(serde_json::Value::as_str)
}

/// The nested `data` object of a Copilot session event, if `map` is one.
fn copilot_data(
    map: &serde_json::Map<String, serde_json::Value>,
) -> Option<&serde_json::Map<String, serde_json::Value>> {
    map.get("data").and_then(serde_json::Value::as_object)
}

/// True when the transcript holds a tool call whose name is `ridl_check`, or
/// ends with `__ridl_check` (Claude names an MCP tool `mcp__<server>__<tool>`),
/// or, for Copilot, a `tool.execution_start` event whose `toolName` or
/// `mcpToolName` is `ridl_check`. Text that only mentions the name does not
/// count, nor does a call refused before it reached the server.
pub fn used_ridl_check(transcript: &str) -> bool {
    json_lines(transcript).any(|line| {
        objects(&line).into_iter().any(|map| {
            let is_call = matches!(str_field(map, "type"), Some("tool_use" | "mcp_tool_call"));
            let named = ["name", "tool"]
                .iter()
                .filter_map(|key| str_field(map, key))
                .any(|n| n == "ridl_check" || n.ends_with("__ridl_check"));
            // Codex records a call its approval policy refused with a null
            // result; that call never reached the server.
            let refused = map.get("result").is_some_and(serde_json::Value::is_null)
                && map.get("error").is_some_and(|e| !e.is_null());
            let copilot_call = str_field(map, "type") == Some("tool.execution_start")
                && copilot_data(map).is_some_and(|data| {
                    ["toolName", "mcpToolName"]
                        .iter()
                        .filter_map(|key| str_field(data, key))
                        .any(|n| n == "ridl_check")
                });
            (is_call && named && !refused) || copilot_call
        })
    })
}

/// Whether a shell command names the skill's directory, however the agent
/// spelt the path to it: `skills/ridl-authoring` as a variable's value, or
/// `skills/.system/../ridl-authoring/SKILL.md`.
fn names_the_skill_directory(command: &str) -> bool {
    command.contains("skills/ridl-authoring") || command.contains("ridl-authoring/")
}

/// Whether the transcript shows that the host made the skill available.
/// Claude: the `init` line lists `ridl-authoring` among its skills, the
/// agent called the `Skill` tool on it, read its `SKILL.md`, or ran a
/// command on its directory. A handed-off run, whose transcript has no
/// `init` line, shows only the last two. Codex: the agent read a file of the
/// skill's directory with a command. Copilot: a `skill.invoked` event names
/// it or its `SKILL.md` path. Text that only mentions the skill does not
/// count.
pub fn skill_was_loaded(host: Host, transcript: &str) -> bool {
    json_lines(transcript).any(|line| {
        objects(&line).into_iter().any(|map| match host {
            Host::Claude => {
                let listed = str_field(map, "subtype") == Some("init")
                    && map
                        .get("skills")
                        .and_then(serde_json::Value::as_array)
                        .is_some_and(|skills| skills.iter().any(|s| s == "ridl-authoring"));
                let called = str_field(map, "type") == Some("tool_use")
                    && str_field(map, "name") == Some("Skill")
                    && map
                        .get("input")
                        .and_then(|input| input.get("skill"))
                        .is_some_and(|s| s == "ridl-authoring");
                let read = str_field(map, "type") == Some("tool_use")
                    && str_field(map, "name") == Some("Read")
                    && map
                        .get("input")
                        .and_then(|input| input.get("file_path"))
                        .and_then(serde_json::Value::as_str)
                        .is_some_and(|p| p.ends_with("ridl-authoring/SKILL.md"));
                let shell = str_field(map, "type") == Some("tool_use")
                    && str_field(map, "name") == Some("Bash")
                    && map
                        .get("input")
                        .and_then(|input| input.get("command"))
                        .and_then(serde_json::Value::as_str)
                        .is_some_and(names_the_skill_directory);
                listed || called || read || shell
            }
            Host::Codex => {
                str_field(map, "type") == Some("command_execution")
                    && str_field(map, "command").is_some_and(names_the_skill_directory)
            }
            Host::Copilot => {
                str_field(map, "type") == Some("skill.invoked")
                    && copilot_data(map).is_some_and(|data| {
                        str_field(data, "name") == Some("ridl-authoring")
                            || str_field(data, "path")
                                .is_some_and(|p| p.ends_with("ridl-authoring/SKILL.md"))
                    })
            }
        })
    })
}

/// The number of Error diagnostics in `ridl check --format json` output, and
/// the design lints it reports.
pub fn errors_and_lints(check_json: &str) -> Result<(usize, BTreeSet<String>), String> {
    let report: serde_json::Value = serde_json::from_str(check_json)
        .map_err(|_| format!("ridl check printed no JSON: {}", check_json.trim()))?;
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
                notes.push(format!(
                    "{e} {}",
                    String::from_utf8_lossy(&out.stderr).trim()
                ));
                (false, false)
            }
        },
        Err(e) => {
            notes.push(format!("ridl check did not run: {e}"));
            (false, false)
        }
    };
    let diff_matches = (task.kind == Kind::Evolve).then(|| {
        let verdict = task.start.as_ref().and_then(|start| {
            Command::new(ridl_binary())
                .args(["diff", "--format", "json"])
                .arg(start)
                .arg(&dirs.workspace)
                .output()
                .ok()
                .and_then(|o| serde_json::from_slice::<serde_json::Value>(&o.stdout).ok())
                .and_then(|v| v["verdict"].as_str().map(str::to_owned))
        });
        let matches = verdict.is_some() && verdict.as_deref() == task.expect_diff.as_deref();
        if !matches {
            notes.push(format!(
                "diff verdict {verdict:?}, expected {:?}",
                task.expect_diff
            ));
        }
        matches
    });
    let verify_via = verify_via(transcript);
    let verify_loop = verify_via != "none";
    if !verify_loop {
        notes.push("the agent neither called ridl_check nor ran `ridl check`".to_owned());
    }
    Mechanical {
        no_error,
        lints_match,
        diff_matches,
        verify_loop,
        verify_via,
        notes,
    }
}

/// True when the transcript holds a shell command that runs `ridl check`:
/// a Claude `Bash` call, a Codex `command_execution`, or a Copilot
/// `tool.execution_start` event naming its `bash` tool. `ridl checker`,
/// `myridl check` and `echo "ridl check"` do not count, nor does text that
/// only mentions it.
pub fn ran_ridl_check_command(transcript: &str) -> bool {
    json_lines(transcript).any(|line| {
        objects(&line).into_iter().any(|map| {
            let command = match str_field(map, "type") {
                Some("tool_use") if str_field(map, "name") == Some("Bash") => map
                    .get("input")
                    .and_then(|input| input.get("command"))
                    .and_then(serde_json::Value::as_str),
                Some("command_execution") => str_field(map, "command"),
                Some("tool.execution_start") => copilot_data(map)
                    .filter(|data| str_field(data, "toolName") == Some("bash"))
                    .and_then(|data| data.get("arguments"))
                    .and_then(|args| args.get("command"))
                    .and_then(serde_json::Value::as_str),
                _ => None,
            };
            command.is_some_and(runs_ridl_check)
        })
    })
}

/// Whether `command` runs `ridl check`: the words appear on their own, and
/// no `echo` or `printf` comes before them in the same simple command (the
/// text since the last `;`, `&`, `|`, `(` or line break), where they would
/// only be printed.
fn runs_ridl_check(command: &str) -> bool {
    let word = |c: char| c.is_alphanumeric() || c == '-' || c == '_' || c == '.';
    command.match_indices("ridl check").any(|(at, found)| {
        let before = command[..at].chars().next_back();
        let after = command[at + found.len()..].chars().next();
        let simple_command = command[..at]
            .rsplit([';', '&', '|', '(', '\n'])
            .next()
            .unwrap_or_default();
        let printed = simple_command
            .split(|c: char| c.is_whitespace() || c == '\'' || c == '"')
            .any(|w| w == "echo" || w == "printf");
        !before.is_some_and(word) && !after.is_some_and(word) && !printed
    })
}

/// How the agent checked its workspace: through the MCP tool, the CLI, both,
/// or neither. Either one is a verify loop; the result records which.
pub fn verify_via(transcript: &str) -> &'static str {
    match (
        used_ridl_check(transcript),
        ran_ridl_check_command(transcript),
    ) {
        (true, true) => "both",
        (true, false) => "mcp",
        (false, true) => "cli",
        (false, false) => "none",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CLAUDE_CALL: &str = r#"{"type":"assistant","message":{"content":[{"type":"tool_use","id":"t1","name":"mcp__ridl__ridl_check","input":{"path":"."}}]}}"#;
    const CODEX_CALL: &str = r#"{"type":"item.completed","item":{"id":"i1","type":"mcp_tool_call","server":"ridl","tool":"ridl_check","status":"completed"}}"#;
    const TEXT_ONLY: &str = r#"{"type":"assistant","message":{"content":[{"type":"text","text":"I ran ridl_check and mcp__ridl__ridl_check"}]}}"#;
    const CLAUDE_INIT_WITH: &str = r#"{"type":"system","subtype":"init","skills":["ridl-authoring"],"mcp_servers":[{"name":"ridl","status":"connected"}]}"#;
    const CLAUDE_INIT_WITHOUT: &str = r#"{"type":"system","subtype":"init","skills":[],"mcp_servers":[{"name":"ridl","status":"connected"}]}"#;
    const CLAUDE_SKILL_CALL: &str = r#"{"type":"assistant","message":{"content":[{"type":"tool_use","id":"t2","name":"Skill","input":{"skill":"ridl-authoring"}}]}}"#;
    const CODEX_SKILL_READ: &str = r#"{"type":"item.completed","item":{"id":"i2","type":"command_execution","command":"cat /h/skills/ridl-authoring/SKILL.md","status":"completed"}}"#;
    const CODEX_TEXT_MENTION: &str = r#"{"type":"item.completed","item":{"id":"i3","type":"agent_message","text":"I would use ridl-authoring/SKILL.md if I had it"}}"#;
    const COPILOT_CALL: &str = r#"{"type":"tool.execution_start","data":{"toolCallId":"c1","toolName":"ridl_check","mcpServerName":"ridl","mcpToolName":"ridl_check"}}"#;
    const COPILOT_SKILL_INVOKED: &str = r#"{"type":"skill.invoked","data":{"name":"ridl-authoring","path":"/r/ws/.github/skills/ridl-authoring/SKILL.md","content":"..."}}"#;
    const COPILOT_SHELL: &str = r#"{"type":"tool.execution_start","data":{"toolCallId":"c2","toolName":"bash","arguments":{"command":"ridl check ."}}}"#;

    #[test]
    fn a_tool_call_to_ridl_check_is_found_on_each_host() {
        assert!(used_ridl_check(CLAUDE_CALL));
        assert!(used_ridl_check(CODEX_CALL));
        assert!(used_ridl_check(COPILOT_CALL));
    }

    #[test]
    fn a_codex_call_refused_before_the_server_does_not_count() {
        let refused = r#"{"type":"item.completed","item":{"type":"mcp_tool_call","server":"ridl","tool":"ridl_check","result":null,"error":{"message":"MCP tool call requires approval, but approval policy is never"},"status":"failed"}}"#;
        let tool_error = r#"{"type":"item.completed","item":{"type":"mcp_tool_call","server":"ridl","tool":"ridl_check","result":{"content":[{"type":"text","text":"no `ridl.toml` found"}]},"error":null,"status":"failed"}}"#;
        assert!(!used_ridl_check(refused));
        assert!(used_ridl_check(tool_error));
    }

    #[test]
    fn a_shell_ridl_check_is_found_on_each_host() {
        let claude = r#"{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Bash","input":{"command":"cd /r/ws && ridl check ."}}]}}"#;
        let codex = r#"{"type":"item.completed","item":{"type":"command_execution","command":"/bin/bash -lc 'ridl check --format json .'"}}"#;
        let other = r#"{"type":"item.completed","item":{"type":"command_execution","command":"/bin/bash -lc 'ridl checker; ridl fmt .; myridl check'"}}"#;
        assert!(ran_ridl_check_command(claude));
        assert!(ran_ridl_check_command(codex));
        assert!(ran_ridl_check_command(COPILOT_SHELL));
        assert!(!ran_ridl_check_command(other));
        assert!(!ran_ridl_check_command(TEXT_ONLY));
        assert!(!ran_ridl_check_command(CLAUDE_CALL));
    }

    #[test]
    fn a_printed_ridl_check_does_not_count() {
        assert!(!runs_ridl_check(r#"echo "ridl check""#));
        assert!(!runs_ridl_check(r"printf 'ridl check\n'"));
        assert!(!runs_ridl_check(
            r#"/bin/bash -lc 'echo "run ridl check next"'"#
        ));
        assert!(runs_ridl_check("echo start; ridl check ."));
        assert!(runs_ridl_check(
            "/bin/bash -lc 'ridl check --format json .'"
        ));
        assert!(runs_ridl_check("cd /r/ws && /usr/local/bin/ridl check ."));
        // A new simple command starts after each of these separators.
        for command in [
            "echo a; ridl check .",
            "echo a && ridl check .",
            "echo a | ridl check .",
            "echo $(ridl check .)",
            "echo a\nridl check .",
        ] {
            assert!(runs_ridl_check(command), "{command}");
        }
    }

    #[test]
    fn only_a_tool_call_record_counts_as_a_call() {
        let result = r#"{"type":"tool_result","name":"mcp__ridl__ridl_check","content":"ok"}"#;
        assert!(!used_ridl_check(result));
    }

    #[test]
    fn a_call_is_refused_only_with_a_null_result_and_an_error() {
        let no_result_no_error =
            r#"{"type":"mcp_tool_call","tool":"ridl_check","result":null,"error":null}"#;
        let result_and_error = r#"{"type":"mcp_tool_call","tool":"ridl_check","result":{"content":[]},"error":{"message":"x"}}"#;
        assert!(used_ridl_check(no_result_no_error));
        assert!(used_ridl_check(result_and_error));
    }

    #[test]
    fn only_the_ridl_authoring_skill_counts_as_loaded() {
        let other_skill = r#"{"type":"system","subtype":"init","skills":["pdf"]}"#;
        assert!(!skill_was_loaded(Host::Claude, other_skill));
        let other_call = r#"{"type":"tool_use","name":"Skill","input":{"skill":"pdf"}}"#;
        assert!(!skill_was_loaded(Host::Claude, other_call));
        // A Codex record that is not a command execution does not count,
        // even when it carries a command naming the skill.
        let not_run = r#"{"type":"file_change","command":"cat /h/skills/ridl-authoring/SKILL.md"}"#;
        assert!(!skill_was_loaded(Host::Codex, not_run));
        let other_copilot_skill = r#"{"type":"skill.invoked","data":{"name":"pdf","path":"/h/skills/pdf/SKILL.md","content":"..."}}"#;
        assert!(!skill_was_loaded(Host::Copilot, other_copilot_skill));
    }

    #[test]
    fn copilot_skill_loading_is_told_apart() {
        assert!(skill_was_loaded(Host::Copilot, COPILOT_SKILL_INVOKED));
        assert!(!skill_was_loaded(Host::Copilot, TEXT_ONLY));
        assert!(!used_ridl_check(COPILOT_SHELL));
        assert!(used_ridl_check(COPILOT_CALL));
    }

    #[test]
    fn the_verify_loop_records_which_check_the_agent_used() {
        let shell = r#"{"type":"item.completed","item":{"type":"command_execution","command":"ridl check ."}}"#;
        assert_eq!(verify_via(CLAUDE_CALL), "mcp");
        assert_eq!(verify_via(shell), "cli");
        assert_eq!(verify_via(&format!("{CODEX_CALL}\n{shell}")), "both");
        assert_eq!(verify_via(TEXT_ONLY), "none");
    }

    #[test]
    fn a_transcript_that_only_mentions_ridl_check_fails_the_verify_loop() {
        assert!(!used_ridl_check(TEXT_ONLY));
        assert!(!used_ridl_check("not json at all ridl_check"));
    }

    #[test]
    fn claude_skill_loading_is_told_apart() {
        assert!(skill_was_loaded(Host::Claude, CLAUDE_INIT_WITH));
        assert!(skill_was_loaded(Host::Claude, CLAUDE_SKILL_CALL));
        assert!(!skill_was_loaded(Host::Claude, CLAUDE_INIT_WITHOUT));
        assert!(!skill_was_loaded(Host::Claude, TEXT_ONLY));
    }

    #[test]
    fn a_handed_off_claude_run_loads_the_skill_by_reading_it() {
        let read = r#"{"type":"assistant","message":{"content":[{"type":"tool_use","id":"t3","name":"Read","input":{"file_path":"/r/ws/.claude/skills/ridl-authoring/SKILL.md"}}]}}"#;
        let other = r#"{"type":"assistant","message":{"content":[{"type":"tool_use","id":"t4","name":"Read","input":{"file_path":"/r/ws/ridl.toml"}}]}}"#;
        let cat = r#"{"type":"assistant","message":{"content":[{"type":"tool_use","id":"t5","name":"Bash","input":{"command":"W=/r/ws/.claude/skills/ridl-authoring; cat $W/SKILL.md"}}]}}"#;
        let ls = r#"{"type":"assistant","message":{"content":[{"type":"tool_use","id":"t6","name":"Bash","input":{"command":"ls /r/ws"}}]}}"#;
        assert!(skill_was_loaded(Host::Claude, read));
        assert!(skill_was_loaded(Host::Claude, cat));
        assert!(!skill_was_loaded(Host::Claude, other));
        assert!(!skill_was_loaded(Host::Claude, ls));
    }

    #[test]
    fn a_skill_path_spelt_through_another_directory_still_counts() {
        let dotted = r#"{"type":"item.completed","item":{"type":"command_execution","command":"cat /h/skills/.system/../ridl-authoring/SKILL.md"}}"#;
        assert!(skill_was_loaded(Host::Codex, dotted));
    }

    /// Excerpts of the probe's real transcripts: Claude runs as subagents
    /// handed off by `--claude-handoff`, Codex runs through `codex exec`.
    /// `codex-with.jsonl` takes its `ridl_check` call from a separate Codex
    /// session, because neither probe run on Codex called the tool.
    #[test]
    fn the_recorded_transcripts_are_read_as_the_probe_saw_them() {
        let fixture = |name: &str| {
            std::fs::read_to_string(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("tests/eval-fixtures")
                    .join(name),
            )
            .unwrap()
        };
        let cases = [
            (Host::Claude, "claude-with.jsonl", true, true),
            (Host::Claude, "claude-without.jsonl", false, true),
            (Host::Codex, "codex-with.jsonl", true, true),
            (Host::Codex, "codex-without.jsonl", false, false),
        ];
        for (host, name, loaded, checked) in cases {
            let t = fixture(name);
            assert_eq!(skill_was_loaded(host, &t), loaded, "{name}");
            assert_eq!(used_ridl_check(&t), checked, "{name}");
        }
        let answer = super::super::judge::claude_final_message(&fixture("claude-with.jsonl"));
        assert!(answer.starts_with("The workspace compiles"), "{answer}");
    }

    #[test]
    fn codex_skill_loading_is_told_apart() {
        assert!(skill_was_loaded(Host::Codex, CODEX_SKILL_READ));
        assert!(!skill_was_loaded(Host::Codex, CODEX_TEXT_MENTION));
        assert!(!skill_was_loaded(Host::Codex, CODEX_CALL));
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

    #[test]
    fn a_run_passes_only_when_every_check_passes() {
        let all = Mechanical {
            no_error: true,
            lints_match: true,
            diff_matches: None,
            verify_loop: true,
            verify_via: "mcp",
            notes: vec![],
        };
        assert!(all.passed());
        assert!(
            !Mechanical {
                diff_matches: Some(false),
                ..all.clone()
            }
            .passed()
        );
        assert!(
            Mechanical {
                diff_matches: Some(true),
                ..all.clone()
            }
            .passed()
        );
        assert!(
            !Mechanical {
                verify_loop: false,
                ..all.clone()
            }
            .passed()
        );
        assert!(
            !Mechanical {
                no_error: false,
                ..all.clone()
            }
            .passed()
        );
        assert!(
            !Mechanical {
                lints_match: false,
                ..all
            }
            .passed()
        );
    }
}
