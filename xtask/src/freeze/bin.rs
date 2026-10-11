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
    let target = std::env::var_os("CARGO_TARGET_DIR").unwrap_or_else(|| "target".into());
    Ok(root.join(target).join("debug").join(name))
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

/// The tracked files of `root` under `paths`, as paths relative to `root`
/// with `/` separators. A tracked file is never a git-ignored one.
pub fn tracked_files(root: &Path, paths: &[&str]) -> Result<Vec<String>, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["ls-files", "--"])
        .args(paths)
        .env_remove("GIT_DIR")
        .env_remove("GIT_INDEX_FILE")
        .env_remove("GIT_WORK_TREE")
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
