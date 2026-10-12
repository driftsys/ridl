//! The two agent hosts, their isolated configuration, and how a run invokes
//! them. A run gets a fresh host home, so no user skill, rule, memory or hook
//! reaches the agent, and a workspace outside any git repository, so no
//! `AGENTS.md` or `CLAUDE.md` above it is read.

use super::task::Task;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

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

    /// The variables of the eval's own environment that a run or a judgement
    /// on this host inherits. Every other variable is cleared, so a setting
    /// such as `ANTHROPIC_MODEL` or `CLAUDECODE` in the operator's shell does
    /// not change a result. The invocation sets its host home on top.
    pub fn inherited_env(self) -> &'static [&'static str] {
        match self {
            Host::Claude => &[
                "PATH",
                "HOME",
                "TMPDIR",
                "LANG",
                "CLAUDE_CODE_OAUTH_TOKEN",
                "ANTHROPIC_API_KEY",
            ],
            Host::Codex => &["PATH", "HOME", "TMPDIR", "LANG"],
        }
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
    /// The variables the invocation sets.
    pub env: Vec<(String, String)>,
    /// The variables it inherits from the eval's environment; no other
    /// variable reaches the host.
    pub inherit: &'static [&'static str],
    pub cwd: PathBuf,
    pub stdin: String,
}

/// The command that runs one agent session on `prompt`, without a user.
/// Codex runs with every permission inside its workspace
/// (`-s workspace-write`); Claude runs with `bypassPermissions`, which is not
/// limited to the workspace.
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
        inherit: host.inherited_env(),
        cwd: dirs.workspace.clone(),
        stdin: prompt.to_owned(),
    }
}

/// How a host command ended.
#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    /// The command exited, with its exit code, or `None` when a signal ended
    /// it.
    Exited(Option<i32>),
    /// The operator ran a handed-off invocation and created the done file.
    HandedOff,
    /// The command was still running, or the operator had not created the
    /// done file, when the timeout passed. A command is killed.
    TimedOut,
}

/// Runs an invocation with its stdin, writes its stdout to `stdout_to`, and
/// kills it when it runs longer than `timeout`.
pub fn run(
    invocation: &Invocation,
    stdout_to: &Path,
    timeout: Duration,
) -> Result<Outcome, String> {
    use std::io::Write;
    let out = std::fs::File::create(stdout_to).map_err(|e| e.to_string())?;
    let mut child = Command::new(&invocation.program)
        .args(&invocation.args)
        .env_clear()
        .envs(
            invocation
                .inherit
                .iter()
                .filter_map(|k| std::env::var_os(k).map(|v| (*k, v))),
        )
        .envs(invocation.env.iter().map(|(k, v)| (k, v)))
        .current_dir(&invocation.cwd)
        .stdin(Stdio::piped())
        .stdout(out)
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|e| format!("spawn {}: {e}", invocation.program))?;
    // Written from another thread, so a command that never reads its stdin
    // cannot hold the timeout off. A command that exits before reading it
    // makes the write fail, which its exit status already reports.
    let mut stdin = child.stdin.take().expect("a piped stdin");
    let prompt = invocation.stdin.clone();
    std::thread::spawn(move || {
        let _ = stdin.write_all(prompt.as_bytes());
    });
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
            return Ok(Outcome::Exited(status.code()));
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Ok(Outcome::TimedOut);
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// Runs `invocation` and says how it ended. With `handoff`, nothing is
/// spawned: the invocation is written to a request file next to
/// `stdout_to`, and the call waits until the operator has run the prompt
/// with an agent of their own, written its output to `stdout_to`, and
/// created the done file, or until `timeout` passes. The request file holds
/// the invocation's working directory, arguments, environment and prompt.
pub fn execute(
    invocation: &Invocation,
    stdout_to: &Path,
    handoff: bool,
    timeout: Duration,
) -> Result<Outcome, String> {
    if !handoff {
        return run(invocation, stdout_to, timeout);
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
    request.insert(
        "env".into(),
        toml::Value::Table(
            invocation
                .env
                .iter()
                .map(|(k, v)| (k.clone(), v.clone().into()))
                .collect(),
        ),
    );
    request.insert("prompt".into(), invocation.stdin.clone().into());
    let path = request_path(stdout_to);
    // Written under another name and renamed, so an operator that sees the
    // request file never reads it half written.
    let partial = path.with_extension("partial");
    std::fs::write(
        &partial,
        toml::to_string(&request).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    std::fs::rename(&partial, &path).map_err(|e| e.to_string())?;
    eprintln!("eval: handoff: {}", path.display());
    let deadline = Instant::now() + timeout;
    while !done_path(stdout_to).is_file() {
        if Instant::now() >= deadline {
            return Ok(Outcome::TimedOut);
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    Ok(Outcome::HandedOff)
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
/// into a run's home, and returns the bytes it wrote.
pub fn copy_login(stored: &Path, home: &Path) -> Result<Vec<u8>, String> {
    let login = std::fs::read(stored).map_err(|e| {
        format!(
            "no stored Codex login at {} ({e}); run `codex login`",
            stored.display()
        )
    })?;
    std::fs::write(home.join("auth.json"), &login).map_err(|e| e.to_string())?;
    Ok(login)
}

/// Copies a run's login back when the run refreshed its token, so the stored
/// login is not left with a token the refresh revoked. The run's `auth.json`
/// is compared with `lent`, the bytes `copy_login` wrote: a copy the run did
/// not change is never written back, so a login refreshed meanwhile by
/// another Codex process is kept. A run that deleted its `auth.json` leaves
/// the stored login as it is.
pub fn return_login(home: &Path, stored: &Path, lent: &[u8]) -> Result<(), String> {
    let Ok(used) = std::fs::read(home.join("auth.json")) else {
        return Ok(());
    };
    if used != lent {
        std::fs::write(stored, used).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// The stored Codex login, lent to one run's home. Dropping it returns a
/// refreshed login and deletes the run's copy, also when the run ended with
/// an error or a panic.
pub struct LentLogin {
    home: PathBuf,
    stored: PathBuf,
    lent: Vec<u8>,
}

impl LentLogin {
    pub fn lend(stored: &Path, home: &Path) -> Result<LentLogin, String> {
        Ok(LentLogin {
            home: home.to_owned(),
            stored: stored.to_owned(),
            lent: copy_login(stored, home)?,
        })
    }
}

impl Drop for LentLogin {
    fn drop(&mut self) {
        if let Err(e) = return_login(&self.home, &self.stored, &self.lent) {
            eprintln!(
                "eval: could not return the Codex login to {}: {e}",
                self.stored.display()
            );
        }
        let _ = std::fs::remove_file(self.home.join("auth.json"));
    }
}

/// A fresh directory for one run or one judgement, under the system
/// temporary directory, so that no git repository, `AGENTS.md` or
/// `CLAUDE.md` is above it.
pub fn scratch_dir(task: &str, label: &str) -> Result<PathBuf, String> {
    let dir = std::env::temp_dir().join(format!("ridl-eval-{}-{task}-{label}", std::process::id()));
    if dir.exists() {
        std::fs::remove_dir_all(&dir).map_err(|e| e.to_string())?;
    }
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
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
        assert!(
            inv.args
                .windows(2)
                .any(|w| w == ["--permission-mode", "bypassPermissions"])
        );
    }

    #[test]
    fn codex_runs_in_its_own_home_without_a_git_check() {
        let inv = agent_invocation(Host::Codex, "m", &dirs(Path::new("/r")), "p");
        assert!(inv.args.contains(&"--skip-git-repo-check".to_owned()));
        assert!(inv.args.contains(&"--json".to_owned()));
        assert!(inv.args.windows(2).any(|w| w == ["-m", "m"]));
        assert!(inv.args.windows(2).any(|w| w == ["-o", "/r/last.md"]));
        assert!(inv.args.windows(2).any(|w| w == ["-C", "/r/ws"]));
        assert!(inv.args.windows(2).any(|w| w == ["-s", "workspace-write"]));
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
        let mcp: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(claude.home.parent().unwrap().join("mcp.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(
            mcp["mcpServers"]["ridl"],
            serde_json::json!({ "command": ridl_binary(), "args": ["mcp"] })
        );
        let codex = prepare_run(&scratch("codex"), &task, Host::Codex, true, &skill).unwrap();
        assert!(codex.home.join("skills/ridl-authoring/SKILL.md").is_file());
        assert!(
            std::fs::read_to_string(codex.home.join("config.toml"))
                .unwrap()
                .contains(&format!(
                    "[mcp_servers.ridl]\ncommand = {:?}\nargs = [\"mcp\"]\n",
                    ridl_binary()
                ))
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
        let stored_dir = scratch("stored");
        let stored = stored_dir.join("auth.json");
        std::fs::write(&stored, "old").unwrap();
        std::fs::write(stored_dir.join("config.toml"), "model = \"x\"\n").unwrap();
        let home = scratch("home");
        let lent = copy_login(&stored, &home).unwrap();
        assert_eq!(lent, b"old");
        assert_eq!(
            std::fs::read_to_string(home.join("auth.json")).unwrap(),
            "old"
        );
        assert_eq!(std::fs::read_dir(&home).unwrap().count(), 1);
        std::fs::write(home.join("auth.json"), "refreshed").unwrap();
        return_login(&home, &stored, &lent).unwrap();
        assert_eq!(std::fs::read_to_string(&stored).unwrap(), "refreshed");
    }

    #[test]
    fn an_unchanged_run_copy_keeps_a_login_refreshed_meanwhile() {
        let stored = scratch("stored").join("auth.json");
        std::fs::write(&stored, "old").unwrap();
        let home = scratch("home");
        let lent = copy_login(&stored, &home).unwrap();
        // Another Codex process refreshes the stored login during the run.
        std::fs::write(&stored, "refreshed elsewhere").unwrap();
        return_login(&home, &stored, &lent).unwrap();
        assert_eq!(
            std::fs::read_to_string(&stored).unwrap(),
            "refreshed elsewhere"
        );
    }

    #[test]
    fn a_deleted_run_copy_leaves_the_stored_login() {
        let stored = scratch("stored").join("auth.json");
        std::fs::write(&stored, "old").unwrap();
        let home = scratch("home");
        let lent = copy_login(&stored, &home).unwrap();
        std::fs::remove_file(home.join("auth.json")).unwrap();
        return_login(&home, &stored, &lent).unwrap();
        assert_eq!(std::fs::read_to_string(&stored).unwrap(), "old");
    }

    #[test]
    fn a_lent_login_is_returned_and_deleted_when_the_run_panics() {
        let stored = scratch("stored").join("auth.json");
        std::fs::write(&stored, "old").unwrap();
        let home = scratch("home");
        let run = std::panic::catch_unwind(|| {
            let _login = LentLogin::lend(&stored, &home).unwrap();
            std::fs::write(home.join("auth.json"), "refreshed").unwrap();
            panic!("the run failed");
        });
        assert!(run.is_err());
        assert_eq!(std::fs::read_to_string(&stored).unwrap(), "refreshed");
        assert!(!home.join("auth.json").exists());
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
            env: vec![("CLAUDE_CONFIG_DIR".into(), "/r/home".into())],
            inherit: &[],
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
                let request: toml::Table = toml::from_str(&text).unwrap();
                assert_eq!(request["prompt"].as_str(), Some("the prompt"), "{text}");
                assert!(request["cwd"].as_str().unwrap().ends_with("/ws"), "{text}");
                assert_eq!(
                    request["env"]["CLAUDE_CONFIG_DIR"].as_str(),
                    Some("/r/home"),
                    "{text}"
                );
                std::fs::write(&out, "transcript").unwrap();
                std::fs::write(done_path(&out), "").unwrap();
            })
        };
        let outcome = execute(&invocation, &out, true, Duration::from_secs(60)).unwrap();
        operator.join().unwrap();
        assert_eq!(outcome, Outcome::HandedOff);
        assert_eq!(std::fs::read_to_string(&out).unwrap(), "transcript");
        // The request was renamed into place, not copied.
        let partial = request_path(&out).with_extension("partial");
        assert!(!partial.exists(), "{} is left", partial.display());
    }

    #[test]
    fn a_run_directory_is_outside_any_git_repository() {
        let run = scratch_dir("design-0001", "outside-test").unwrap();
        let mut dir = Some(run.as_path());
        while let Some(d) = dir {
            for name in [".git", "AGENTS.md", "CLAUDE.md"] {
                assert!(!d.join(name).exists(), "{} holds {name}", d.display());
            }
            dir = d.parent();
        }
        std::fs::remove_dir_all(&run).unwrap();
    }

    #[test]
    fn a_handed_off_invocation_stops_waiting_at_the_timeout() {
        let dir = scratch("handoff-timeout");
        let invocation = Invocation {
            program: "claude".into(),
            args: vec![],
            env: vec![],
            inherit: &[],
            cwd: dir.clone(),
            stdin: "p".into(),
        };
        // On another thread, so a wait that ignores the timeout fails the
        // test instead of hanging it.
        let (send, receive) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let outcome = execute(
                &invocation,
                &dir.join("t.jsonl"),
                true,
                Duration::from_millis(300),
            );
            let _ = send.send(outcome);
        });
        let outcome = receive
            .recv_timeout(Duration::from_secs(10))
            .expect("the wait stops at the timeout");
        assert_eq!(outcome.unwrap(), Outcome::TimedOut);
    }

    #[test]
    fn a_command_that_never_reads_its_stdin_is_still_killed_at_the_timeout() {
        let dir = scratch("unread-stdin");
        // Far more than a pipe buffer holds, so a write on this thread would
        // block until the command read it.
        let invocation = Invocation {
            program: "sleep".into(),
            args: vec!["30".into()],
            env: vec![],
            inherit: &["PATH"],
            cwd: dir.clone(),
            stdin: "x".repeat(4 << 20),
        };
        let (send, receive) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = send.send(run(
                &invocation,
                &dir.join("out"),
                Duration::from_millis(300),
            ));
        });
        let outcome = receive
            .recv_timeout(Duration::from_secs(10))
            .expect("the run stops at the timeout");
        assert_eq!(outcome.unwrap(), Outcome::TimedOut);
    }

    #[test]
    fn a_command_is_killed_at_the_timeout() {
        let dir = scratch("run-timeout");
        let invocation = Invocation {
            program: "sleep".into(),
            args: vec!["30".into()],
            env: vec![],
            inherit: &["PATH"],
            cwd: dir.clone(),
            stdin: String::new(),
        };
        let start = Instant::now();
        let outcome = run(&invocation, &dir.join("out"), Duration::from_millis(300)).unwrap();
        assert_eq!(outcome, Outcome::TimedOut);
        assert!(start.elapsed() < Duration::from_secs(10));
    }
}
