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
            // `codex exec` never asks for approval, so a tool that needs it is
            // refused; every ridl tool is read-only.
            let config = format!(
                "[mcp_servers.ridl]\ncommand = {ridl:?}\nargs = [\"mcp\"]\ndefault_tools_approval_mode = \"approve\"\n"
            );
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

/// Runs `invocation` and returns its exit code, or `None` when it has none.
/// With `handoff`, nothing is spawned: the invocation is written to a request
/// file next to `stdout_to`, and the call waits until the operator has run
/// the prompt with an agent of their own, written its output to `stdout_to`,
/// and created the done file. A handed-off run has no exit code.
pub fn execute(
    invocation: &Invocation,
    stdout_to: &Path,
    handoff: bool,
) -> Result<Option<i32>, String> {
    if !handoff {
        return run(invocation, stdout_to).map(|status| status.code());
    }
    let mut request = toml::Table::new();
    request.insert("cwd".into(), invocation.cwd.display().to_string().into());
    request.insert("output".into(), stdout_to.display().to_string().into());
    request.insert(
        "done".into(),
        done_path(stdout_to).display().to_string().into(),
    );
    request.insert(
        "args".into(),
        toml::Value::Array(invocation.args.iter().map(|a| a.clone().into()).collect()),
    );
    request.insert("prompt".into(), invocation.stdin.clone().into());
    let path = request_path(stdout_to);
    std::fs::write(&path, toml::to_string(&request).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    eprintln!("eval: handoff: {}", path.display());
    while !done_path(stdout_to).is_file() {
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
    Ok(None)
}

fn request_path(stdout_to: &Path) -> PathBuf {
    stdout_to.with_extension("request.toml")
}

fn done_path(stdout_to: &Path) -> PathBuf {
    stdout_to.with_extension("done")
}

/// The stored Codex login: `RIDL_EVAL_CODEX_LOGIN`, else `auth.json` in
/// `$CODEX_HOME`, else in `~/.codex`.
pub fn codex_login_path() -> PathBuf {
    if let Ok(path) = std::env::var("RIDL_EVAL_CODEX_LOGIN") {
        return PathBuf::from(path);
    }
    let home = std::env::var("CODEX_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".codex")
        });
    home.join("auth.json")
}

/// Copies the stored Codex login, and nothing else of the user's Codex home,
/// into a run's home.
pub fn copy_login(stored: &Path, home: &Path) -> Result<(), String> {
    std::fs::copy(stored, home.join("auth.json"))
        .map(|_| ())
        .map_err(|e| {
            format!(
                "no stored Codex login at {} ({e}); run `codex login`",
                stored.display()
            )
        })
}

/// Copies a run's login back when the run refreshed its token, so the stored
/// login is not left with a token the refresh revoked.
pub fn return_login(home: &Path, stored: &Path) -> Result<(), String> {
    let used = home.join("auth.json");
    let changed = std::fs::read(&used).ok() != std::fs::read(stored).ok();
    if changed && used.is_file() {
        std::fs::copy(&used, stored).map_err(|e| e.to_string())?;
    }
    Ok(())
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
    fn claude_runs_in_an_empty_home_with_only_the_ridl_server() {
        let inv = agent_invocation(Host::Claude, "m", &dirs(Path::new("/r")), "p");
        // `--bare` refuses the OAuth token `claude setup-token` prints.
        assert!(!inv.args.contains(&"--bare".to_owned()));
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
        // `codex exec` never asks, so a tool that needs approval is refused.
        assert!(
            std::fs::read_to_string(codex.home.join("config.toml"))
                .unwrap()
                .contains("default_tools_approval_mode = \"approve\"")
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
    fn a_codex_run_gets_only_the_stored_login_and_returns_a_refreshed_one() {
        let stored = scratch("stored").join("auth.json");
        std::fs::write(&stored, "old").unwrap();
        let home = scratch("home");
        copy_login(&stored, &home).unwrap();
        assert_eq!(
            std::fs::read_to_string(home.join("auth.json")).unwrap(),
            "old"
        );
        assert_eq!(std::fs::read_dir(&home).unwrap().count(), 1);
        std::fs::write(home.join("auth.json"), "refreshed").unwrap();
        return_login(&home, &stored).unwrap();
        assert_eq!(std::fs::read_to_string(&stored).unwrap(), "refreshed");
    }

    #[test]
    fn a_missing_stored_login_is_an_error() {
        let missing = scratch("none").join("auth.json");
        let error = copy_login(&missing, &scratch("home")).unwrap_err();
        assert!(error.contains("codex login"), "{error}");
    }

    #[test]
    fn a_handed_off_invocation_waits_for_the_operator_output() {
        let dir = scratch("handoff");
        let out = dir.join("t.jsonl");
        let invocation = Invocation {
            program: "claude".into(),
            args: vec!["-p".into()],
            env: vec![],
            cwd: dir.join("ws"),
            stdin: "the prompt".into(),
        };
        let operator = {
            let out = out.clone();
            std::thread::spawn(move || {
                let request = request_path(&out);
                while !request.is_file() {
                    std::thread::sleep(std::time::Duration::from_millis(20));
                }
                let text = std::fs::read_to_string(&request).unwrap();
                assert!(
                    text.contains("the prompt") && text.contains("/ws"),
                    "{text}"
                );
                std::fs::write(&out, "transcript").unwrap();
                std::fs::write(done_path(&out), "").unwrap();
            })
        };
        let code = execute(&invocation, &out, true).unwrap();
        operator.join().unwrap();
        assert_eq!(code, None);
        assert_eq!(std::fs::read_to_string(&out).unwrap(), "transcript");
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
