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
//! not a legal package name or has a reserved word as a segment, when a
//! `ridl.toml` at or above the target directory would claim it, or when a
//! `ridl.toml` already sits below the package directory (a manifest inside a
//! unit is MANI-013, ADR-0002 section 1). The search upward does not stop at
//! a `.git` directory, because MANI-013 does not. It follows symbolic links,
//! so the search sees the directory the operating system resolves the path
//! to. Nothing is appended to an existing workspace, and nothing outside the
//! target files is written. The member files are written before the root
//! manifest, and a failed write removes the paths this command created, so a
//! refusal leaves no partial scaffold. Exit codes follow ADR-0010 decision 1:
//! 0 when the files are written, 2 when the command could not answer.

use std::io::Write;
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

/// Every directory and file the command will write.
struct Plan {
    /// The directory of the package; for a workspace, the member's directory.
    package_directory: PathBuf,
    /// The files in the order they are written. The root manifest of a
    /// workspace comes last.
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
    let mut files = vec![
        (
            package_directory.join("ridl.toml"),
            format!("[package]\nname = \"{name}\"\nversion = \"0.1.0\"\n"),
        ),
        (
            package_directory.join(format!("{name}.ridl")),
            source(&name),
        ),
    ];
    if request.workspace {
        files.push((
            target.join("ridl.toml"),
            format!("[workspace]\nmembers = [\"{name}\"]\n"),
        ));
    }

    for (file, _) in &files {
        if file.symlink_metadata().is_ok() {
            return Err(format!("`{}` already exists", file.display()));
        }
    }
    if package_directory.symlink_metadata().is_ok() && !package_directory.is_dir() {
        return Err(format!(
            "`{}` exists and is not a directory",
            package_directory.display()
        ));
    }
    refuse_manifest_below(&package_directory)?;

    Ok(Plan {
        package_directory,
        files,
        kind: if request.workspace {
            "workspace"
        } else {
            "package"
        },
        name,
    })
}

/// The absolute, normalised target directory, as the path was written. For
/// `new` it must not exist; for `init` it must be an existing directory.
fn target_directory(request: &Request<'_>) -> Result<PathBuf, String> {
    let path = request.path;
    let absolute = std::path::absolute(path)
        .map_err(|error| format!("cannot resolve `{}`: {error}", path.display()))?;
    let target = normalise(&absolute);
    if request.create {
        if target.symlink_metadata().is_ok() {
            return Err(format!("`{}` already exists", path.display()));
        }
    } else if !target.is_dir() {
        return Err(format!("`{}` is not a directory", path.display()));
    }
    Ok(target)
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
        return match name_problem(name) {
            None => Ok(name.to_string()),
            Some(NameProblem::Illegal) => Err(format!(
                "`--name {name}` is not a legal package name. {NAME_RULE}"
            )),
            Some(NameProblem::Reserved(word)) => Err(format!(
                "`--name {name}` uses the reserved word `{word}` as a segment"
            )),
        };
    }
    let Some(name) = target.file_name().and_then(|name| name.to_str()) else {
        return Err(format!(
            "the target has no usable directory name; pass `--name`. {NAME_RULE}"
        ));
    };
    match name_problem(name) {
        None => Ok(name.to_string()),
        Some(NameProblem::Illegal) => Err(format!(
            "the directory name `{name}` is not a legal package name; pass `--name`. {NAME_RULE}"
        )),
        Some(NameProblem::Reserved(word)) => Err(format!(
            "the directory name `{name}` uses the reserved word `{word}` as a segment; \
             pass `--name`"
        )),
    }
}

const NAME_RULE: &str = "A package name is lowercase dot-separated segments, each a letter \
     followed by letters or digits (for example `veh.common`).";

enum NameProblem {
    /// Not a legal package name (MANI-006).
    Illegal,
    /// Legal, but a segment is a reserved word, so a source file that
    /// declares the package would not parse.
    Reserved(String),
}

fn name_problem(name: &str) -> Option<NameProblem> {
    if !ridl_core::is_valid_package_name(name) {
        return Some(NameProblem::Illegal);
    }
    name.split('.')
        .find(|segment| ridl_syntax::keywords::is_reserved(segment))
        .map(|segment| NameProblem::Reserved(segment.to_string()))
}

/// Refuses when a `ridl.toml` sits in a directory above `target`. The
/// search does not stop at `.git`. It starts from the directory the
/// operating system resolves `target` to, so a symbolic link does not hide an
/// enclosing unit.
fn refuse_enclosing_unit(target: &Path) -> Result<(), String> {
    let resolved = resolve(target);
    for directory in resolved.ancestors().skip(1) {
        let manifest = directory.join("ridl.toml");
        if manifest.is_file() {
            return Err(format!(
                "`{}` is inside the unit of `{}`; a manifest inside a unit is an error \
                 (MANI-013) and this command does not edit an existing workspace",
                target.display(),
                manifest.display()
            ));
        }
    }
    Ok(())
}

/// Canonicalises the deepest ancestor of `path` that exists and appends the
/// components that do not exist yet.
fn resolve(path: &Path) -> PathBuf {
    let mut missing = Vec::new();
    let mut existing = path;
    loop {
        if let Ok(canonical) = existing.canonicalize() {
            return missing
                .iter()
                .rev()
                .fold(canonical, |resolved, part| resolved.join(part));
        }
        match (existing.file_name(), existing.parent()) {
            (Some(part), Some(parent)) => {
                missing.push(part.to_os_string());
                existing = parent;
            }
            _ => return path.to_path_buf(),
        }
    }
}

/// Refuses when a `ridl.toml` exists anywhere below `directory`, because the
/// new manifest would then enclose it (MANI-013). Symbolic links and `.git`
/// are not followed.
fn refuse_manifest_below(directory: &Path) -> Result<(), String> {
    let mut stack = vec![directory.to_path_buf()];
    while let Some(next) = stack.pop() {
        let entries = match std::fs::read_dir(&next) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(format!("cannot read `{}`: {error}", next.display())),
        };
        for entry in entries {
            let entry =
                entry.map_err(|error| format!("cannot read `{}`: {error}", next.display()))?;
            let path = entry.path();
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            if kind.is_dir() {
                if entry.file_name() != ".git" {
                    stack.push(path);
                }
            } else if next != directory && entry.file_name() == "ridl.toml" {
                return Err(format!(
                    "`{}` holds a manifest below it, `{}`; a manifest inside a unit is an \
                     error (MANI-013)",
                    directory.display(),
                    path.display()
                ));
            }
        }
    }
    Ok(())
}

fn write(plan: Plan) -> Result<Plan, String> {
    let first_missing = first_missing_ancestor(&plan.package_directory);
    let mut created = Vec::new();
    let result = write_files(&plan, &mut created);
    if result.is_err() {
        for path in created.iter().rev() {
            let _ = std::fs::remove_file(path);
        }
        if let Some(directory) = first_missing {
            let _ = std::fs::remove_dir_all(directory);
        }
    }
    result.map(|()| plan)
}

fn write_files(plan: &Plan, created: &mut Vec<PathBuf>) -> Result<(), String> {
    std::fs::create_dir_all(&plan.package_directory).map_err(|error| {
        format!(
            "cannot create `{}`: {error}",
            plan.package_directory.display()
        )
    })?;
    for (path, text) in &plan.files {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(|error| format!("cannot write `{}`: {error}", path.display()))?;
        created.push(path.clone());
        file.write_all(text.as_bytes())
            .map_err(|error| format!("cannot write `{}`: {error}", path.display()))?;
    }
    Ok(())
}

/// The shallowest ancestor of `directory` (or `directory` itself) that does
/// not exist yet, which `create_dir_all` will create.
fn first_missing_ancestor(directory: &Path) -> Option<PathBuf> {
    let mut missing = None;
    for ancestor in directory.ancestors() {
        if ancestor.symlink_metadata().is_ok() {
            break;
        }
        missing = Some(ancestor.to_path_buf());
    }
    missing
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
