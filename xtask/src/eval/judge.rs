//! The rubric judge. A run on one host is judged by the other host's model,
//! which sees the prompt, the rubric, the final workspace and the agent's
//! final message, and nothing that says whether the skill was loaded.

use super::host::{self, Host, RunDirs};
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug)]
pub enum Marker {
    Must,
    Should,
    MustNot,
}

#[derive(Debug)]
pub struct Item {
    pub n: u32,
    pub marker: Marker,
    pub text: String,
}

/// The numbered items of a rubric, in the format
/// `crates/ridl/tests/eval_corpus.rs` validates: `N. **marker** text`, with
/// unindented numbers and indented continuation lines.
pub fn rubric_items(rubric: &str) -> Result<Vec<Item>, String> {
    let mut items: Vec<Item> = Vec::new();
    for line in rubric.lines() {
        let numbered = (!line.starts_with(' '))
            .then(|| line.split_once(". "))
            .flatten()
            .and_then(|(n, rest)| n.parse::<u32>().ok().map(|n| (n, rest)));
        match numbered {
            Some((n, rest)) => {
                let (marker, text) = if let Some(t) = rest.strip_prefix("**must not** ") {
                    (Marker::MustNot, t)
                } else if let Some(t) = rest.strip_prefix("**must** ") {
                    (Marker::Must, t)
                } else if let Some(t) = rest.strip_prefix("**should** ") {
                    (Marker::Should, t)
                } else {
                    return Err(format!("rubric item {n} has no marker"));
                };
                items.push(Item {
                    n,
                    marker,
                    text: text.to_owned(),
                });
            }
            None => {
                if let Some(last) = items.last_mut().filter(|_| !line.trim().is_empty()) {
                    last.text.push(' ');
                    last.text.push_str(line.trim());
                }
            }
        }
    }
    if items.is_empty() {
        return Err("the rubric has no item".to_owned());
    }
    Ok(items)
}

pub fn judge_prompt(
    prompt: &str,
    rubric: &str,
    workspace_files: &[(String, String)],
    final_message: &str,
) -> String {
    let mut text = String::from(
        "You grade one answer to a design request against a rubric. For each numbered rubric \
         item, decide whether the answer meets it. A \"must not\" item is met when the answer \
         does not do the thing it names. Judge only from the material below. Reply with JSON \
         only: {\"items\":[{\"n\":<number>,\"met\":<true|false>,\"reason\":\"<one sentence>\"}]}, \
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

/// The judge's mark for each rubric item, in item order. A reply that is not
/// JSON, skips an item, marks one twice or marks an unknown one is an error,
/// never a partial score.
pub fn parse_verdict(reply: &str, items: &[Item]) -> Result<Vec<bool>, String> {
    let value: serde_json::Value =
        serde_json::from_str(reply.trim()).map_err(|_| "the judge reply is not JSON".to_owned())?;
    let entries = value["items"]
        .as_array()
        .ok_or("the judge reply has no items array")?;
    let mut met: Vec<Option<bool>> = vec![None; items.len()];
    for entry in entries {
        let n = entry["n"].as_u64().ok_or("a judge entry has no number")? as u32;
        let flag = entry["met"]
            .as_bool()
            .ok_or_else(|| format!("judge entry {n} has no met flag"))?;
        let index = items
            .iter()
            .position(|i| i.n == n)
            .ok_or_else(|| format!("the judge marked unknown item {n}"))?;
        if met[index].replace(flag).is_some() {
            return Err(format!("the judge marked item {n} twice"));
        }
    }
    met.iter()
        .zip(items)
        .map(|(m, item)| m.ok_or_else(|| format!("the judge did not mark item {}", item.n)))
        .collect()
}

/// The weighted fraction of rubric items met: **must** and **must not**
/// count 2, **should** counts 1.
pub fn weighted_score(items: &[Item], met: &[bool]) -> f64 {
    let weight = |m: Marker| match m {
        Marker::Must | Marker::MustNot => 2.0,
        Marker::Should => 1.0,
    };
    let total: f64 = items.iter().map(|i| weight(i.marker)).sum();
    let got: f64 = items
        .iter()
        .zip(met)
        .filter(|(_, m)| **m)
        .map(|(i, _)| weight(i.marker))
        .sum();
    got / total
}

/// The JSON Schema of a verdict.
pub const SCHEMA: &str = r#"{"type":"object","additionalProperties":false,"required":["items"],"properties":{"items":{"type":"array","items":{"type":"object","additionalProperties":false,"required":["n","met","reason"],"properties":{"n":{"type":"integer"},"met":{"type":"boolean"},"reason":{"type":"string"}}}}}}"#;

/// Writes the verdict schema where the Codex judge reads it.
pub fn write_schema(dir: &Path) -> Result<(), String> {
    std::fs::write(dir.join("schema.json"), SCHEMA).map_err(|e| e.to_string())
}

/// The judge runs with no MCP server, in `dir`, an empty directory with its
/// own host home. The Claude judge has no tool; the Codex judge keeps its
/// shell tool in a read-only sandbox (`-s read-only`).
pub fn judge_invocation(judge: Host, model: &str, dir: &Path, prompt: &str) -> host::Invocation {
    let s = |v: &str| v.to_owned();
    let path = |p: PathBuf| p.display().to_string();
    let (args, env) = match judge {
        Host::Claude => (
            vec![
                s("-p"),
                s("--model"),
                s(model),
                s("--tools"),
                s(""),
                s("--json-schema"),
                s(SCHEMA),
                s("--output-format"),
                s("json"),
                s("--no-session-persistence"),
            ],
            vec![(s("CLAUDE_CONFIG_DIR"), path(dir.join("home")))],
        ),
        Host::Codex => (
            vec![
                s("exec"),
                s("--skip-git-repo-check"),
                s("-m"),
                s(model),
                s("-s"),
                s("read-only"),
                s("--output-schema"),
                path(dir.join("schema.json")),
                s("-o"),
                path(dir.join("verdict.json")),
                s("-C"),
                path(dir.to_owned()),
                s("-"),
            ],
            vec![(s("CODEX_HOME"), path(dir.join("home")))],
        ),
    };
    host::Invocation {
        program: judge.binary(),
        args,
        env,
        cwd: dir.to_owned(),
        stdin: prompt.to_owned(),
    }
}

/// The verdict a Claude judge printed with `--output-format json`: its
/// `structured_output` when present, else its `result` text.
pub fn claude_reply(stdout: &str) -> Result<String, String> {
    let value: serde_json::Value = serde_json::from_str(stdout.trim())
        .map_err(|_| "the Claude judge printed no JSON".to_owned())?;
    if let Some(structured) = value.get("structured_output").filter(|v| !v.is_null()) {
        return Ok(structured.to_string());
    }
    value["result"]
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| "the Claude judge printed no result".to_owned())
}

/// The final message of a Claude run: the `result` of its last `result` line.
/// A handed-off run's transcript has none: its final message is the
/// `message` of its last `SubagentHandback` call, else the text of its last
/// assistant message that holds text.
pub fn claude_final_message(transcript: &str) -> String {
    let lines: Vec<serde_json::Value> = transcript
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect();
    let result = lines
        .iter()
        .filter(|v| v["type"] == "result")
        .filter_map(|v| v["result"].as_str())
        .next_back();
    let handback = || {
        lines
            .iter()
            .filter(|v| v["type"] == "assistant")
            .flat_map(|v| v["message"]["content"].as_array().into_iter().flatten())
            .filter(|block| block["type"] == "tool_use" && block["name"] == "SubagentHandback")
            .filter_map(|block| block["input"]["message"].as_str().map(str::to_owned))
            .next_back()
    };
    let assistant_text = || {
        lines
            .iter()
            .filter(|v| v["type"] == "assistant")
            .filter_map(|v| {
                let text: Vec<&str> = v["message"]["content"]
                    .as_array()?
                    .iter()
                    .filter(|block| block["type"] == "text")
                    .filter_map(|block| block["text"].as_str())
                    .collect();
                (!text.is_empty()).then(|| text.join("\n"))
            })
            .next_back()
    };
    result
        .map(str::to_owned)
        .or_else(handback)
        .or_else(assistant_text)
        .unwrap_or_default()
}

/// The final message of a run on `host`.
pub fn final_message(host: Host, transcript: &str, dirs: &RunDirs) -> String {
    match host {
        Host::Claude => claude_final_message(transcript),
        Host::Codex => std::fs::read_to_string(&dirs.last_message).unwrap_or_default(),
    }
}

/// Every `.typl`, `.ridl` and `.rsdl` file and every `ridl.toml` under the
/// workspace, as (path relative to it, content), sorted by path. Hidden
/// directories, where a host installs the skill, are skipped.
pub fn workspace_files(workspace: &Path) -> Vec<(String, String)> {
    let mut files = Vec::new();
    let mut stack = vec![workspace.to_owned()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            if path.is_dir() {
                if !name.starts_with('.') {
                    stack.push(path);
                }
                continue;
            }
            let wanted = name == "ridl.toml"
                || matches!(
                    path.extension().and_then(|e| e.to_str()),
                    Some("typl" | "ridl" | "rsdl")
                );
            if !wanted {
                continue;
            }
            if let Ok(content) = std::fs::read_to_string(&path) {
                let relative = path
                    .strip_prefix(workspace)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .into_owned();
                files.push((relative, content));
            }
        }
    }
    files.sort();
    files
}

#[cfg(test)]
mod tests {
    use super::*;

    const RUBRIC: &str = "1. **must** a.\n\n2. **should** b\n   continued.\n\n3. **must not** c.\n";

    #[test]
    fn rubric_items_keep_number_marker_and_continuation() {
        let items = rubric_items(RUBRIC).unwrap();
        assert_eq!(items.len(), 3);
        assert!(matches!(items[0].marker, Marker::Must));
        assert!(matches!(items[2].marker, Marker::MustNot));
        assert_eq!(items[1].text, "b continued.");
    }

    #[test]
    fn a_rubric_item_without_a_marker_is_an_error() {
        assert_eq!(
            rubric_items("1. **may** a.\n").unwrap_err(),
            "rubric item 1 has no marker"
        );
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
        assert_eq!(
            parse_verdict(missing, &items).unwrap_err(),
            "the judge did not mark item 2"
        );
        let repeated = r#"{"items":[{"n":1,"met":true},{"n":1,"met":false},{"n":2,"met":true},{"n":3,"met":true}]}"#;
        assert_eq!(
            parse_verdict(repeated, &items).unwrap_err(),
            "the judge marked item 1 twice"
        );
        let unknown = r#"{"items":[{"n":1,"met":true},{"n":2,"met":true},{"n":3,"met":true},{"n":4,"met":true}]}"#;
        assert_eq!(
            parse_verdict(unknown, &items).unwrap_err(),
            "the judge marked unknown item 4"
        );
        assert_eq!(
            parse_verdict("I think it is fine.", &items).unwrap_err(),
            "the judge reply is not JSON"
        );
        let no_flag = r#"{"items":[{"n":1,"met":true},{"n":2},{"n":3,"met":true}]}"#;
        assert_eq!(
            parse_verdict(no_flag, &items).unwrap_err(),
            "judge entry 2 has no met flag"
        );
    }

    #[test]
    fn the_judge_prompt_never_says_whether_the_skill_was_loaded() {
        let p = judge_prompt(
            "Do X.",
            RUBRIC,
            &[("a.typl".into(), "package p\n".into())],
            "Done.",
        );
        assert!(p.contains("Do X.") && p.contains("a.typl") && p.contains("Done."));
        assert!(p.contains("### a.typl\n\n```\npackage p\n"), "{p}");
        assert!(!p.to_lowercase().contains("skill"));
    }

    #[test]
    fn a_claude_run_is_judged_by_codex_read_only() {
        let inv = judge_invocation(Host::Codex, "g", Path::new("/j"), "p");
        assert!(inv.args.windows(2).any(|w| w == ["-s", "read-only"]));
        assert!(
            inv.args
                .windows(2)
                .any(|w| w == ["--output-schema", "/j/schema.json"])
        );
        assert!(inv.args.windows(2).any(|w| w == ["-o", "/j/verdict.json"]));
    }

    #[test]
    fn a_codex_run_is_judged_by_claude_with_no_tool() {
        let inv = judge_invocation(Host::Claude, "c", Path::new("/j"), "p");
        assert!(inv.args.windows(2).any(|w| w == ["--tools", ""]));
        assert!(inv.args.contains(&"--json-schema".to_owned()));
        assert!(!inv.args.contains(&"--mcp-config".to_owned()));
        assert!(
            inv.args
                .windows(2)
                .any(|w| w == ["--output-format", "json"])
        );
    }

    #[test]
    fn the_claude_reply_is_its_structured_output_or_its_result() {
        let structured = r#"{"type":"result","structured_output":{"items":[]},"result":"x"}"#;
        assert_eq!(claude_reply(structured).unwrap(), r#"{"items":[]}"#);
        let plain = r#"{"type":"result","result":"{\"items\":[]}"}"#;
        assert_eq!(claude_reply(plain).unwrap(), r#"{"items":[]}"#);
        assert!(claude_reply("garbage").is_err());
        let null = r#"{"type":"result","structured_output":null,"result":"{\"items\":[]}"}"#;
        assert_eq!(claude_reply(null).unwrap(), r#"{"items":[]}"#);
    }

    #[test]
    fn the_final_message_of_a_claude_run_is_its_last_result() {
        let t = "{\"type\":\"result\",\"result\":\"first\"}\n{\"type\":\"assistant\"}\n{\"type\":\"result\",\"result\":\"last\"}\n";
        assert_eq!(claude_final_message(t), "last");
    }

    #[test]
    fn a_result_line_wins_over_a_handback_and_the_last_handback_wins() {
        let handback = |m: &str| {
            format!(
                r#"{{"type":"assistant","message":{{"content":[{{"type":"tool_use","name":"SubagentHandback","input":{{"message":"{m}"}}}}]}}}}"#
            )
        };
        let result = r#"{"type":"result","result":"the result"}"#;
        assert_eq!(
            claude_final_message(&format!("{result}\n{}\n", handback("handed back"))),
            "the result"
        );
        assert_eq!(
            claude_final_message(&format!("{}\n{}\n", handback("first"), handback("last"))),
            "last"
        );
    }

    #[test]
    fn a_transcript_with_no_result_ends_with_its_last_assistant_text() {
        let t = concat!(
            r#"{"type":"assistant","message":{"content":[{"type":"text","text":"first"}]}}"#,
            "\n",
            r#"{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Read","input":{}}]}}"#,
            "\n",
            r#"{"type":"assistant","message":{"content":[{"type":"text","text":"last"}]}}"#,
            "\n",
            r#"{"type":"user","message":{"content":[{"type":"tool_result","content":"x"}]}}"#,
            "\n"
        );
        assert_eq!(claude_final_message(t), "last");
    }

    #[test]
    fn a_subagent_transcript_ends_with_its_handback_message() {
        let t = concat!(
            r#"{"type":"assistant","message":{"content":[{"type":"text","text":"Let me move it there."}]}}"#,
            "\n",
            r#"{"type":"assistant","message":{"content":[{"type":"tool_use","name":"SubagentHandback","input":{"message":"The answer."}}]}}"#,
            "\n"
        );
        assert_eq!(claude_final_message(t), "The answer.");
    }

    #[test]
    fn every_object_of_the_schema_is_closed_and_requires_all_its_properties() {
        // OpenAI's strict structured output refuses any other schema.
        let schema: serde_json::Value = serde_json::from_str(SCHEMA).unwrap();
        let mut stack = vec![&schema];
        let mut objects = 0;
        while let Some(v) = stack.pop() {
            if v["type"] == "object" {
                objects += 1;
                assert_eq!(v["additionalProperties"], false, "{v}");
                let mut props: Vec<&String> = v["properties"].as_object().unwrap().keys().collect();
                let mut required: Vec<&str> = v["required"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|r| r.as_str().unwrap())
                    .collect();
                props.sort();
                required.sort();
                assert_eq!(props, required, "{v}");
                stack.extend(v["properties"].as_object().unwrap().values());
            }
            if v["type"] == "array" {
                stack.push(&v["items"]);
            }
        }
        assert_eq!(objects, 2);
    }

    #[test]
    fn the_claude_judge_reads_no_stored_configuration_but_accepts_a_cli_login() {
        let inv = judge_invocation(Host::Claude, "c", Path::new("/j"), "p");
        assert!(!inv.args.contains(&"--bare".to_owned()));
        assert!(
            inv.env
                .contains(&("CLAUDE_CONFIG_DIR".to_owned(), "/j/home".to_owned()))
        );
    }

    #[test]
    fn workspace_files_skip_the_installed_skill() {
        let dir = std::env::temp_dir().join(format!("xtask-judge-ws-{}", std::process::id()));
        std::fs::create_dir_all(dir.join(".claude/skills/x")).unwrap();
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        std::fs::write(dir.join("ridl.toml"), "[package]\n").unwrap();
        std::fs::write(dir.join("sub/a.ridl"), "package p\n").unwrap();
        std::fs::write(dir.join("notes.txt"), "no").unwrap();
        std::fs::write(dir.join(".claude/skills/x/b.typl"), "package q\n").unwrap();
        let names: Vec<_> = workspace_files(&dir).into_iter().map(|(p, _)| p).collect();
        assert_eq!(names, ["ridl.toml", "sub/a.ridl"]);
    }
}
