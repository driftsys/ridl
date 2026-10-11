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

    /// The host whose model judges a run on this host.
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

/// The pinned model of each host, from `evals/runner.toml`.
pub struct Models {
    pub claude: String,
    pub codex: String,
}

pub fn load_models(root: &Path) -> Result<Models, String> {
    let path = root.join("evals/runner.toml");
    let text =
        std::fs::read_to_string(&path).map_err(|e| format!("read {}: {e}", path.display()))?;
    let value: toml::Value = toml::from_str(&text).map_err(|e| e.to_string())?;
    let model = |host: &str| {
        value
            .get(host)
            .and_then(|t| t.get("model"))
            .and_then(toml::Value::as_str)
            .map(str::to_owned)
            .ok_or_else(|| format!("evals/runner.toml lacks {host}.model"))
    };
    Ok(Models {
        claude: model("claude")?,
        codex: model("codex")?,
    })
}

/// The directories and files of one run, all under its base directory.
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
pub fn prepare_run(
    base: &Path,
    task: &Task,
    host: Host,
    with_skill: bool,
    skill: &Path,
) -> Result<RunDirs, String> {
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
            let config = serde_json::json!({
                "mcpServers": { "ridl": { "command": ridl, "args": ["mcp"] } }
            });
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

/// A host command line, ready to run.
pub struct Invocation {
    pub program: String,
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
    pub cwd: PathBuf,
    pub stdin: String,
}

/// The command that runs one agent session on `prompt`, without a user,
/// with every permission granted inside its workspace.
pub fn agent_invocation(host: Host, model: &str, dirs: &RunDirs, prompt: &str) -> Invocation {
    let s = |v: &str| v.to_owned();
    let path = |p: &Path| p.display().to_string();
    let base = dirs.home.parent().expect("a run base directory");
    let (args, env) = match host {
        Host::Claude => (
            vec![
                s("-p"),
                s("--bare"),
                s("--model"),
                s(model),
                s("--mcp-config"),
                path(&base.join("mcp.json")),
                s("--strict-mcp-config"),
                s("--permission-mode"),
                s("bypassPermissions"),
                s("--output-format"),
                s("stream-json"),
                s("--verbose"),
                s("--no-session-persistence"),
            ],
            vec![(s("CLAUDE_CONFIG_DIR"), path(&dirs.home))],
        ),
        Host::Codex => (
            vec![
                s("exec"),
                s("--json"),
                s("--skip-git-repo-check"),
                s("-m"),
                s(model),
                s("-s"),
                s("workspace-write"),
                s("-c"),
                s("approval_policy=\"never\""),
                s("-C"),
                path(&dirs.workspace),
                s("-o"),
                path(&dirs.last_message),
                s("-"),
            ],
            vec![(s("CODEX_HOME"), path(&dirs.home))],
        ),
    };
    Invocation {
        program: host.binary(),
        args,
        env,
        cwd: dirs.workspace.clone(),
        stdin: prompt.to_owned(),
    }
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
        assert!(
            inv.args
                .windows(2)
                .any(|w| w == ["--mcp-config", "/r/mcp.json"])
        );
        assert!(
            inv.env
                .contains(&("CLAUDE_CONFIG_DIR".to_owned(), "/r/home".to_owned()))
        );
        assert_eq!(inv.cwd, Path::new("/r/ws"));
        assert_eq!(inv.stdin, "p");
    }

    #[test]
    fn codex_runs_in_its_own_home_without_a_git_check() {
        let inv = agent_invocation(Host::Codex, "m", &dirs(Path::new("/r")), "p");
        assert!(inv.args.contains(&"--skip-git-repo-check".to_owned()));
        assert!(inv.args.contains(&"--json".to_owned()));
        assert!(inv.args.windows(2).any(|w| w == ["-m", "m"]));
        assert!(inv.args.windows(2).any(|w| w == ["-o", "/r/last.md"]));
        assert!(
            inv.env
                .contains(&("CODEX_HOME".to_owned(), "/r/home".to_owned()))
        );
    }

    #[test]
    fn the_judge_of_a_host_is_the_other_host() {
        assert_eq!(Host::Claude.other(), Host::Codex);
        assert_eq!(Host::Codex.other(), Host::Claude);
    }

    fn scratch(label: &str) -> PathBuf {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!(
            "xtask-eval-host-{label}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn fixture_task(base: &Path) -> Task {
        let start = base.join("start");
        std::fs::create_dir_all(&start).unwrap();
        std::fs::write(start.join("ridl.toml"), "[package]\n").unwrap();
        Task {
            id: "evolve-0009".into(),
            kind: super::super::task::Kind::Evolve,
            dir: base.to_owned(),
            start: Some(start),
            expect_lints: vec![],
            expect_diff: Some("compatible".into()),
        }
    }

    fn fixture_skill(base: &Path) -> PathBuf {
        let skill = base.join("skill");
        std::fs::create_dir_all(&skill).unwrap();
        std::fs::write(skill.join("SKILL.md"), "---\nname: ridl-authoring\n---\n").unwrap();
        skill
    }

    #[test]
    fn a_with_run_installs_the_skill_where_each_host_reads_it() {
        let src = scratch("src");
        let task = fixture_task(&src);
        let skill = fixture_skill(&src);
        let claude = prepare_run(&scratch("claude"), &task, Host::Claude, true, &skill).unwrap();
        assert!(
            claude
                .workspace
                .join(".claude/skills/ridl-authoring/SKILL.md")
                .is_file()
        );
        assert!(claude.workspace.join("ridl.toml").is_file());
        let codex = prepare_run(&scratch("codex"), &task, Host::Codex, true, &skill).unwrap();
        assert!(codex.home.join("skills/ridl-authoring/SKILL.md").is_file());
        assert!(
            std::fs::read_to_string(codex.home.join("config.toml"))
                .unwrap()
                .contains("[mcp_servers.ridl]")
        );
    }

    #[test]
    fn a_without_run_has_no_skill_and_an_empty_home() {
        let src = scratch("src");
        let task = fixture_task(&src);
        let skill = fixture_skill(&src);
        let claude = prepare_run(&scratch("claude"), &task, Host::Claude, false, &skill).unwrap();
        assert!(!claude.workspace.join(".claude").exists());
        assert_eq!(std::fs::read_dir(&claude.home).unwrap().count(), 0);
        let codex = prepare_run(&scratch("codex"), &task, Host::Codex, false, &skill).unwrap();
        assert!(!codex.home.join("skills").exists());
    }

    #[test]
    fn a_run_directory_is_outside_any_git_repository() {
        let mut dir = Some(std::env::temp_dir());
        while let Some(d) = dir {
            assert!(!d.join(".git").exists(), "{} holds .git", d.display());
            dir = d.parent().map(Path::to_owned);
        }
    }
}
