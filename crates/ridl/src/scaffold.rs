//! `ridl init` and `ridl new` — write the files of a new unit.
//!
//! The default shape is one standalone package: a `ridl.toml` with a
//! `[package]` table and one source file, `<name>.ridl`, in the manifest
//! directory. `--workspace` writes a root manifest with a `[workspace]` table
//! that lists one member, and the member's directory holds the package. `ridl
//! new PATH` creates the directory first and refuses a path that exists;
//! `ridl init [PATH]` writes into an existing directory.
//!
//! The command plans every file before it writes any, and refuses with exit 2
//! and an `error:` line on stderr when a target file exists, when the name is
//! not a legal package name, or when a `ridl.toml` at or above the target
//! directory would claim it (MANI-013, or a workspace member nested in a
//! unit; ADR-0002 section 1). The search upward stops after the first
//! directory that holds `.git`, as root discovery does (ADR-0002 section 4).
//! Nothing is appended to an existing workspace, and nothing outside the
//! target files is written. Exit codes follow ADR-0010 decision 1: 0 when the
//! files are written, 2 when the command could not answer.

use std::path::{Component, Path, PathBuf};
use std::process::ExitCode;

/// What the command was asked to write.
pub struct Request<'a> {
    /// The target directory, as given.
    pub path: &'a Path,
    /// `ridl new` (create the directory, refuse an existing one) rather than
    /// `ridl init`.
    pub create: bool,
    /// Write a workspace root and one member instead of a standalone package.
    pub workspace: bool,
    /// The package name when it is not the directory's basename.
    pub name: Option<&'a str>,
}

/// Every directory and file the command will write, in the order written.
struct Plan {
    directories: Vec<PathBuf>,
    files: Vec<(PathBuf, String)>,
    kind: &'static str,
    name: String,
}

pub fn run(request: &Request<'_>) -> ExitCode {
    match plan(request).and_then(write) {
        Ok(plan) => {
            println!("Created {} {}", plan.kind, plan.name);
            ExitCode::SUCCESS
        }
        Err(message) => {
            eprintln!("error: {message}");
            ExitCode::from(2)
        }
    }
}

fn plan(request: &Request<'_>) -> Result<Plan, String> {
    let target = target_directory(request)?;
    let name = package_name(request, &target)?;
    refuse_enclosing_unit(&target)?;

    let package_directory = if request.workspace {
        target.join(&name)
    } else {
        target.clone()
    };
    let mut files = Vec::new();
    if request.workspace {
        files.push((
            target.join("ridl.toml"),
            format!("[workspace]\nmembers = [\"{name}\"]\n"),
        ));
    }
    files.push((
        package_directory.join("ridl.toml"),
        format!("[package]\nname = \"{name}\"\nversion = \"0.1.0\"\n"),
    ));
    files.push((
        package_directory.join(format!("{name}.ridl")),
        source(&name),
    ));

    for (file, _) in &files {
        if file.symlink_metadata().is_ok() {
            return Err(format!("`{}` already exists", file.display()));
        }
    }
    if request.workspace
        && let Ok(metadata) = package_directory.symlink_metadata()
        && !metadata.is_dir()
    {
        return Err(format!(
            "`{}` exists and is not a directory",
            package_directory.display()
        ));
    }

    Ok(Plan {
        directories: vec![package_directory],
        files,
        kind: if request.workspace {
            "workspace"
        } else {
            "package"
        },
        name,
    })
}

/// The absolute, normalised target directory. For `new` it must not exist;
/// for `init` it must be an existing directory.
fn target_directory(request: &Request<'_>) -> Result<PathBuf, String> {
    let path = request.path;
    if request.create {
        if path.symlink_metadata().is_ok() {
            return Err(format!("`{}` already exists", path.display()));
        }
        let absolute = std::path::absolute(path)
            .map_err(|error| format!("cannot resolve `{}`: {error}", path.display()))?;
        Ok(normalise(&absolute))
    } else {
        if !path.is_dir() {
            return Err(format!("`{}` is not a directory", path.display()));
        }
        path.canonicalize()
            .map_err(|error| format!("cannot resolve `{}`: {error}", path.display()))
    }
}

/// Resolves `..` components lexically.
fn normalise(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            other => out.push(other),
        }
    }
    out
}

fn package_name(request: &Request<'_>, target: &Path) -> Result<String, String> {
    if let Some(name) = request.name {
        return if ridl_core::is_valid_package_name(name) {
            Ok(name.to_string())
        } else {
            Err(illegal_name(&format!("`--name {name}`")))
        };
    }
    let basename = target.file_name().and_then(|name| name.to_str());
    match basename {
        Some(name) if ridl_core::is_valid_package_name(name) => Ok(name.to_string()),
        Some(name) => Err(format!(
            "the directory name `{name}` is not a legal package name; pass `--name`. {}",
            NAME_RULE
        )),
        None => Err(format!(
            "the target has no usable directory name; pass `--name`. {NAME_RULE}"
        )),
    }
}

const NAME_RULE: &str = "A package name is lowercase dot-separated segments, each a letter \
     followed by letters or digits (for example `veh.common`).";

fn illegal_name(what: &str) -> String {
    format!("{what} is not a legal package name. {NAME_RULE}")
}

/// Refuses when a `ridl.toml` sits in `target` or above it. The search stops
/// after the first directory that holds `.git`.
fn refuse_enclosing_unit(target: &Path) -> Result<(), String> {
    for directory in target.ancestors() {
        let manifest = directory.join("ridl.toml");
        if directory != target && manifest.is_file() {
            return Err(format!(
                "`{}` is inside the unit of `{}`; a manifest inside a unit is an error \
                 (MANI-013) and this command does not edit an existing workspace",
                target.display(),
                manifest.display()
            ));
        }
        if directory.join(".git").exists() {
            break;
        }
    }
    Ok(())
}

fn write(plan: Plan) -> Result<Plan, String> {
    use std::io::Write;
    for directory in &plan.directories {
        std::fs::create_dir_all(directory)
            .map_err(|error| format!("cannot create `{}`: {error}", directory.display()))?;
    }
    for (path, text) in &plan.files {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(|error| format!("cannot write `{}`: {error}", path.display()))?;
        file.write_all(text.as_bytes())
            .map_err(|error| format!("cannot write `{}`: {error}", path.display()))?;
    }
    Ok(plan)
}

fn source(name: &str) -> String {
    format!(
        "package {name}\n\
         \n\
         /// Road speed.\n\
         type Speed: km/h [0.0..250.0 step 0.5]\n\
         \n\
         /// Reports speed.\n\
         interface Hello {{\n\
         \x20 /// Current speed, every 10 ms.\n\
         \x20 signal speed: Speed @10ms\n\
         }}\n"
    )
}
