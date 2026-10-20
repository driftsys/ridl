//! The built `ridl` and `ridlc` binaries, shared by the dumpers that run the
//! command line.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

/// The output of one run of a binary.
pub struct Run {
    pub code: i32,
    pub stdout: String,
    pub stderr: String,
}

static BUILD: OnceLock<Result<(), String>> = OnceLock::new();

/// Builds `ridl` and `ridlc` once per process, then returns the path of
/// `name` in `${CARGO_TARGET_DIR:-target}/debug`, relative to `root`.
pub fn binary(root: &Path, name: &str) -> Result<PathBuf, String> {
    BUILD
        .get_or_init(|| {
            let output = Command::new("cargo")
                .current_dir(root)
                .args(["build", "--locked", "-p", "ridl-cli", "-p", "ridlc"])
                .output()
                .map_err(|e| format!("cannot run cargo: {e}"))?;
            if output.status.success() {
                Ok(())
            } else {
                Err(format!(
                    "cargo build of ridl-cli and ridlc failed:\n{}",
                    String::from_utf8_lossy(&output.stderr)
                ))
            }
        })
        .clone()?;
    Ok(target_dir(root).join("debug").join(name))
}

/// `${CARGO_TARGET_DIR:-target}`, relative to `root` when it is relative.
pub fn target_dir(root: &Path) -> PathBuf {
    let target = std::env::var_os("CARGO_TARGET_DIR").unwrap_or_else(|| "target".into());
    root.join(target)
}

/// Runs `exe` with `args` in `dir`.
pub fn run(exe: &Path, dir: &Path, args: &[&str]) -> Result<Run, String> {
    let output = Command::new(exe)
        .current_dir(dir)
        .args(args)
        .output()
        .map_err(|e| format!("cannot run {}: {e}", exe.display()))?;
    Ok(Run {
        code: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    })
}

/// Removes the variables a git hook exports from `command`'s environment, so
/// that a git command it runs reads the repository of its own directory.
pub fn without_git_env(command: &mut Command) -> &mut Command {
    command
        .env_remove("GIT_DIR")
        .env_remove("GIT_INDEX_FILE")
        .env_remove("GIT_WORK_TREE")
}

/// `git -C <root>`, without the variables a git hook exports.
pub fn git(root: &Path) -> Command {
    let mut command = Command::new("git");
    command.arg("-C").arg(root);
    without_git_env(&mut command);
    command
}

/// The tracked files of `root` under `paths`, as paths relative to `root`
/// with `/` separators. A tracked file is never a git-ignored one.
pub fn tracked_files(root: &Path, paths: &[&str]) -> Result<Vec<String>, String> {
    let output = git(root)
        .args(["ls-files", "--"])
        .args(paths)
        .output()
        .map_err(|e| format!("cannot run git: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "git ls-files failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::to_string)
        .collect())
}

/// The frozen copies of the example and corpus packages that the
/// accepted-sources and wire dumps read, relative to the repository root. The
/// wire dump also reads one copy outside it (`wire::UNION_SOURCE`).
/// Below it, each copy keeps the path of its original, for example
/// `examples/cabin`, so an edit to `examples/` or to `evals/corpus/` does not
/// change a dump.
pub const SOURCE_COPIES: &str = "xtask/tests/freeze-fixtures/sources";

/// The directories under `paths` that hold a tracked `ridl.toml`, sorted,
/// leaving out any directory below a `generated*` directory.
pub fn package_dirs(root: &Path, paths: &[&str]) -> Result<Vec<String>, String> {
    let mut dirs: Vec<String> = tracked_files(root, paths)?
        .iter()
        .filter_map(|file| file.strip_suffix("/ridl.toml"))
        .filter(|dir| !dir.split('/').any(|part| part.starts_with("generated")))
        .map(str::to_string)
        .collect();
    dirs.sort();
    Ok(dirs)
}

/// Every file below `dir`, at any depth, sorted.
pub fn files_below(dir: &Path) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();
    collect_files(dir, &mut files)?;
    files.sort();
    Ok(files)
}

fn collect_files(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries =
        std::fs::read_dir(dir).map_err(|e| format!("cannot read {}: {e}", dir.display()))?;
    for entry in entries {
        let path = entry
            .map_err(|e| format!("cannot read {}: {e}", dir.display()))?
            .path();
        if path.is_dir() {
            collect_files(&path, out)?;
        } else {
            out.push(path);
        }
    }
    Ok(())
}
