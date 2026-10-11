//! `cargo xtask freeze update|check`: the freeze harness.
//!
//! Each frozen surface has a dumper that returns one line per fact. `update`
//! writes the sorted, deduplicated lines to `freeze/<surface>.txt`. `check`
//! regenerates every dump and fails when it differs from the committed file,
//! and fails when a line of a surface's dump at its baseline ref is missing
//! from the dump at HEAD. A line that was added passes.
//!
//! The Rust API surfaces have no line dump. Each has a checker in a second
//! registry, [`api_surfaces`], which compares the API with `cargo
//! semver-checks` instead (`freeze/rust_api.rs`).

mod bin;
mod cli;
mod diagnostics;
mod keywords;
mod manifest;
mod mcp;
mod msrv;
mod protocol;
mod rust_api;
mod sources;
mod wire;

use serde::Deserialize;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

/// One row of `freeze/surfaces.toml`.
#[derive(Debug, PartialEq, Eq)]
pub struct Row {
    pub surface: String,
    pub baseline: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawFile {
    #[serde(default)]
    surface: Vec<RawRow>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRow {
    name: String,
    baseline: Option<String>,
}

/// Reads the `[[surface]]` tables of `freeze/surfaces.toml`, in file order.
pub fn parse_surfaces(toml: &str) -> Result<Vec<Row>, String> {
    let file: RawFile = toml::from_str(toml).map_err(|e| format!("surfaces.toml: {e}"))?;
    let mut rows: Vec<Row> = Vec::new();
    for row in file.surface {
        let Some(baseline) = row.baseline else {
            return Err(format!(
                "surfaces.toml: surface `{}` has no `baseline`",
                row.name
            ));
        };
        if rows.iter().any(|r| r.surface == row.name) {
            return Err(format!(
                "surfaces.toml: surface `{}` appears twice",
                row.name
            ));
        }
        rows.push(Row {
            surface: row.name,
            baseline,
        });
    }
    Ok(rows)
}

/// The lines of `baseline` that are not in `head`, compared as sets of
/// non-empty lines, sorted.
pub fn missing_lines(baseline: &str, head: &str) -> Vec<String> {
    let head: BTreeSet<&str> = head.lines().filter(|l| !l.is_empty()).collect();
    let baseline: BTreeSet<&str> = baseline.lines().filter(|l| !l.is_empty()).collect();
    baseline
        .into_iter()
        .filter(|l| !head.contains(l))
        .map(str::to_string)
        .collect()
}

/// A dumper takes the repository root and returns lines. The harness sorts
/// and deduplicates them.
pub type Dumper = fn(&Path) -> Result<Vec<String>, String>;

/// The registry of frozen surfaces.
pub fn dumpers() -> &'static [(&'static str, Dumper)] {
    &[
        ("accepted-sources", sources::dump),
        ("cli", cli::dump),
        ("codegen-protocol", protocol::dump),
        ("diagnostics", diagnostics::dump),
        ("keywords", keywords::dump),
        ("manifest", manifest::dump),
        ("mcp", mcp::dump),
        ("msrv", msrv::dump),
        ("wire", wire::dump),
    ]
}

/// Rewrites an API surface's committed files under `freeze/`.
pub type UpdateStep = fn(&Path) -> Result<(), String>;

/// Returns the failures of an API surface's committed files against a
/// regeneration.
pub type CurrentStep = fn(&Path) -> Result<Vec<String>, String>;

/// A frozen surface that is compared with `cargo semver-checks`, not as lines.
pub struct ApiSurface {
    pub name: &'static str,
    /// Rewrites the surface's committed files under `freeze/`. `None` for a
    /// surface that has no committed files.
    pub update: Option<UpdateStep>,
    /// The failures of the committed files against a regeneration. Runs with
    /// or without a row. `None` for a surface that has no committed files.
    pub current: Option<CurrentStep>,
    /// The failures of HEAD against the row's baseline. Runs for a row only.
    pub compare: fn(&Path, &Row) -> Result<Vec<String>, String>,
}

/// The registry of frozen surfaces that are Rust APIs.
pub fn api_surfaces() -> &'static [ApiSurface] {
    &[
        ApiSurface {
            name: "generated-crate",
            update: Some(rust_api::generated_update),
            current: Some(rust_api::generated_current),
            compare: rust_api::generated_compare,
        },
        ApiSurface {
            name: "runtime-crates",
            update: None,
            current: None,
            compare: rust_api::runtime_compare,
        },
    ]
}

const USAGE: &str = "usage: cargo xtask freeze <update|check> [--root <dir>] \
[--dumper-fixture <name>=<file>] [--fixtures-only] [--api-semver <name>]\n\
  --root <dir>                      repository root (default: the workspace root)\n\
  --dumper-fixture <name>=<file>    test-only: register a surface whose lines \
are read from <file>\n\
  --fixtures-only                   test-only: skip the registered dumpers and API \
checkers, which read the real repository\n\
  --api-semver <name>               test-only: with --fixtures-only, run the baseline \
comparison of the API surface <name> on the files under --root";

struct Options {
    root: PathBuf,
    fixtures: Vec<(String, PathBuf)>,
    fixtures_only: bool,
    api_semver: Vec<String>,
}

fn parse_options(args: &[String]) -> Result<Options, String> {
    let mut root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
        .ok_or("cannot find the workspace root")?;
    let mut fixtures = Vec::new();
    let mut fixtures_only = false;
    let mut api_semver = Vec::new();
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        let value = |iter: &mut std::slice::Iter<String>| {
            iter.next()
                .cloned()
                .ok_or_else(|| format!("{arg} needs a value\n{USAGE}"))
        };
        match arg.as_str() {
            "--root" => root = PathBuf::from(value(&mut iter)?),
            "--dumper-fixture" => {
                let spec = value(&mut iter)?;
                let (name, file) = spec.split_once('=').ok_or_else(|| {
                    format!("--dumper-fixture expects <name>=<file>, got `{spec}`")
                })?;
                fixtures.push((name.to_string(), PathBuf::from(file)));
            }
            "--fixtures-only" => fixtures_only = true,
            "--api-semver" => {
                let name = value(&mut iter)?;
                if !api_surfaces().iter().any(|s| s.name == name) {
                    return Err(format!("--api-semver: no API surface is named `{name}`"));
                }
                api_semver.push(name);
            }
            other => return Err(format!("unknown argument `{other}`\n{USAGE}")),
        }
    }
    Ok(Options {
        root,
        fixtures,
        fixtures_only,
        api_semver,
    })
}

fn normalise(mut lines: Vec<String>) -> String {
    lines.retain(|l| !l.is_empty());
    lines.sort();
    lines.dedup();
    lines.into_iter().map(|l| l + "\n").collect()
}

/// Every surface name with its dump, from the registry then the fixtures.
fn dump_all(options: &Options) -> Result<Vec<(String, String)>, String> {
    let mut dumps = Vec::new();
    let registered: &[(&str, Dumper)] = if options.fixtures_only {
        &[]
    } else {
        dumpers()
    };
    for (name, dumper) in registered {
        let lines = dumper(&options.root).map_err(|e| format!("surface `{name}`: {e}"))?;
        dumps.push((name.to_string(), normalise(lines)));
    }
    for (name, file) in &options.fixtures {
        let text = std::fs::read_to_string(file)
            .map_err(|e| format!("fixture `{name}`: cannot read {}: {e}", file.display()))?;
        dumps.push((
            name.clone(),
            normalise(text.lines().map(str::to_string).collect()),
        ));
    }
    Ok(dumps)
}

/// The API surfaces that run, each with whether its `update` and `current`
/// steps run: every registered surface with both steps, or under
/// `--fixtures-only` the surfaces `--api-semver` names, without them.
fn active_api_surfaces(options: &Options) -> Vec<(&'static ApiSurface, bool)> {
    api_surfaces()
        .iter()
        .filter_map(|surface| {
            if !options.fixtures_only {
                Some((surface, true))
            } else if options.api_semver.iter().any(|n| n == surface.name) {
                Some((surface, false))
            } else {
                None
            }
        })
        .collect()
}

fn dump_path(root: &Path, surface: &str) -> PathBuf {
    root.join("freeze").join(format!("{surface}.txt"))
}

fn read_rows(root: &Path) -> Result<Vec<Row>, String> {
    let path = root.join("freeze").join("surfaces.toml");
    let text = std::fs::read_to_string(&path)
        .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    parse_surfaces(&text)
}

fn update(options: &Options) -> Result<(), String> {
    let dir = options.root.join("freeze");
    std::fs::create_dir_all(&dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    for (name, text) in dump_all(options)? {
        let path = dump_path(&options.root, &name);
        std::fs::write(&path, text).map_err(|e| format!("cannot write {}: {e}", path.display()))?;
        println!("wrote {}", path.display());
    }
    for (surface, regenerate) in active_api_surfaces(options) {
        if let (true, Some(update)) = (regenerate, surface.update) {
            update(&options.root).map_err(|e| format!("surface `{}`: {e}", surface.name))?;
            println!("updated surface `{}`", surface.name);
        }
    }
    Ok(())
}

/// `git show <baseline>:freeze/<surface>.txt` in `root`.
fn baseline_dump(root: &Path, row: &Row) -> Result<String, String> {
    let spec = format!("{}:freeze/{}.txt", row.baseline, row.surface);
    let mut command = Command::new("git");
    command
        .arg("-C")
        .arg(root)
        .args(["show", &spec])
        .env_remove("GIT_DIR")
        .env_remove("GIT_INDEX_FILE")
        .env_remove("GIT_WORK_TREE");
    let output = command
        .output()
        .map_err(|e| format!("cannot run git: {e}"))?;
    if output.status.success() {
        return String::from_utf8(output.stdout).map_err(|e| format!("`{spec}` is not UTF-8: {e}"));
    }
    Err(format!(
        "surface `{}`: cannot read `{spec}` (baseline `{}`): {}",
        row.surface,
        row.baseline,
        String::from_utf8_lossy(&output.stderr).trim()
    ))
}

fn check(options: &Options) -> Result<(), String> {
    let dumps = dump_all(options)?;
    let apis = active_api_surfaces(options);
    let rows = read_rows(&options.root)?;
    let mut failures: Vec<String> = Vec::new();

    for row in &rows {
        if !dumps.iter().any(|(name, _)| *name == row.surface)
            && !apis.iter().any(|(api, _)| api.name == row.surface)
        {
            failures.push(format!(
                "surfaces.toml has a row for `{}` (baseline `{}`) but no dumper or API checker \
has that name",
                row.surface, row.baseline
            ));
        }
    }

    if let Ok(entries) = std::fs::read_dir(options.root.join("freeze")) {
        for entry in entries.flatten() {
            let file = entry.file_name().to_string_lossy().into_owned();
            let Some(name) = file.strip_suffix(".txt") else {
                continue;
            };
            if !dumps.iter().any(|(surface, _)| surface == name) {
                failures.push(format!(
                    "freeze/{file} is produced by no dumper: delete it, or register the dumper"
                ));
            }
        }
    }

    for (name, text) in &dumps {
        let path = dump_path(&options.root, name);
        match std::fs::read_to_string(&path) {
            Ok(committed) if committed == *text => {}
            Ok(_) | Err(_) => failures.push(format!(
                "freeze/{name}.txt is missing or differs from the regenerated dump: \
run `cargo xtask freeze update` and commit the result"
            )),
        }
        let Some(row) = rows.iter().find(|r| r.surface == *name) else {
            continue;
        };
        match baseline_dump(&options.root, row) {
            Ok(baseline) => {
                for line in missing_lines(&baseline, text) {
                    failures.push(format!(
                        "surface `{name}`: `{line}` is in the baseline `{}` and missing at HEAD \
(row: name = \"{name}\", baseline = \"{}\")",
                        row.baseline, row.baseline
                    ));
                }
            }
            Err(error) => failures.push(error),
        }
    }

    for (surface, regenerate) in &apis {
        let name = surface.name;
        if let (true, Some(current)) = (*regenerate, surface.current) {
            match current(&options.root) {
                Ok(lines) => {
                    failures.extend(lines.into_iter().map(|l| format!("surface `{name}`: {l}")))
                }
                Err(error) => failures.push(format!("surface `{name}`: {error}")),
            }
        }
        if let Some(row) = rows.iter().find(|r| r.surface == name) {
            match (surface.compare)(&options.root, row) {
                Ok(lines) => failures.extend(lines),
                Err(error) => failures.push(error),
            }
        }
    }

    if failures.is_empty() {
        println!("freeze: {} surface(s) checked.", dumps.len() + apis.len());
        Ok(())
    } else {
        Err(failures.join("\n"))
    }
}

pub fn run(args: &[String]) -> Result<(), String> {
    let Some((subcommand, rest)) = args.split_first() else {
        return Err(USAGE.to_string());
    };
    let options = parse_options(rest)?;
    match subcommand.as_str() {
        "update" => update(&options),
        "check" => check(&options),
        other => Err(format!("unknown subcommand `{other}`\n{USAGE}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dumps_are_sorted_and_relative() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let root_text = root.to_string_lossy().into_owned();
        for (name, dumper) in dumpers() {
            let text = normalise(dumper(root).unwrap());
            let lines: Vec<&str> = text.lines().collect();
            assert!(!lines.is_empty(), "surface `{name}` is empty");
            assert!(
                lines.windows(2).all(|w| w[0] < w[1]),
                "surface `{name}` is not sorted and unique"
            );
            assert!(
                !text.contains(&root_text),
                "surface `{name}` has an absolute path"
            );
            assert!(!text.contains('\\'), "surface `{name}` has a backslash");
        }
    }

    #[test]
    fn missing_lines_reports_a_removed_line() {
        assert_eq!(missing_lines("a\nb\n", "a\n"), vec!["b"]);
    }

    #[test]
    fn missing_lines_accepts_an_added_line() {
        assert!(missing_lines("a\n", "a\nb\n").is_empty());
    }

    #[test]
    fn missing_lines_reports_a_changed_line_as_removed() {
        assert_eq!(missing_lines("k = 1\n", "k = 2\n"), vec!["k = 1"]);
    }

    #[test]
    fn missing_lines_ignores_duplicates() {
        assert!(missing_lines("a\na\n", "a\n").is_empty());
    }

    #[test]
    fn missing_lines_ignores_empty_lines() {
        assert!(missing_lines("a\n\n", "a\n").is_empty());
    }

    #[test]
    fn parse_surfaces_rejects_unknown_names() {
        assert!(parse_surfaces("[[surfaces]]\nname = \"x\"\nbaseline = \"v1\"\n").is_err());
        assert!(parse_surfaces("[[surface]]\nname = \"x\"\nbaselnie = \"v1\"\n").is_err());
    }

    #[test]
    fn parse_surfaces_rejects_a_duplicate_surface() {
        let error = parse_surfaces(
            "[[surface]]\nname = \"x\"\nbaseline = \"v1\"\n\
             [[surface]]\nname = \"x\"\nbaseline = \"v2\"\n",
        )
        .unwrap_err();
        assert!(error.contains("`x`"), "{error}");
    }

    #[test]
    fn parse_surfaces_reads_rows() {
        let rows = parse_surfaces(
            "[[surface]]\nname = \"one\"\nbaseline = \"v1\"\n\n\
             [[surface]]\nname = \"two\"\nbaseline = \"v2\"\n",
        )
        .unwrap();
        assert_eq!(
            rows,
            vec![
                Row {
                    surface: "one".into(),
                    baseline: "v1".into()
                },
                Row {
                    surface: "two".into(),
                    baseline: "v2".into()
                },
            ]
        );
        let error = parse_surfaces("[[surface]]\nname = \"lonely\"\n").unwrap_err();
        assert!(error.contains("lonely"), "{error}");
    }
}
