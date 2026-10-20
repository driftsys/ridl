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

/// One row of `freeze/surfaces.toml`.
#[derive(Debug, PartialEq, Eq)]
pub struct Row {
    pub surface: String,
    pub baseline: String,
}

/// The contents of `freeze/surfaces.toml`.
#[derive(Debug, PartialEq, Eq)]
pub struct Surfaces {
    /// The surfaces that must have a row when a release is made.
    pub required: Vec<String>,
    pub rows: Vec<Row>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawFile {
    #[serde(default)]
    required: Vec<String>,
    #[serde(default)]
    surface: Vec<RawRow>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRow {
    name: String,
    baseline: Option<String>,
}

/// The names in `required` that have no row, in `required` order.
pub fn rows_missing(required: &[String], rows: &[Row]) -> Vec<String> {
    required
        .iter()
        .filter(|name| !rows.iter().any(|row| &row.surface == *name))
        .cloned()
        .collect()
}

/// Reads `required` and the `[[surface]]` tables of `freeze/surfaces.toml`, in
/// file order.
pub fn parse_surfaces(toml: &str) -> Result<Surfaces, String> {
    let file: RawFile = toml::from_str(toml).map_err(|e| format!("surfaces.toml: {e}"))?;
    let mut rows: Vec<Row> = Vec::new();
    for row in file.surface {
        let Some(baseline) = row.baseline else {
            return Err(format!(
                "surfaces.toml: surface `{}` has no `baseline`",
                row.name
            ));
        };
        // `git show :<path>` reads the index, and git reads a leading dash as
        // an option.
        if baseline.is_empty() || baseline.starts_with('-') {
            return Err(format!(
                "surfaces.toml: surface `{}` has the baseline `{baseline}`, which is not a git \
ref: write a tag or a commit",
                row.name
            ));
        }
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
    for (index, name) in file.required.iter().enumerate() {
        if file.required[..index].contains(name) {
            return Err(format!(
                "surfaces.toml: `{name}` appears twice in `required`"
            ));
        }
    }
    Ok(Surfaces {
        required: file.required,
        rows,
    })
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

/// Returns the reason an API surface's comparison can be skipped, or `None`
/// when it must run.
pub type SkipStep = fn(&Path, &Row) -> Result<Option<String>, String>;

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
    /// For a row, the reason the comparison can be skipped because nothing it
    /// reads changed since the baseline, or `None` when it must run. `None`
    /// for a surface whose comparison always runs.
    pub skip: Option<SkipStep>,
}

/// What the check did for one API surface.
#[derive(Debug, PartialEq, Eq)]
pub enum ApiOutcome {
    /// The surface was compared with its baseline.
    Compared,
    /// The comparison was skipped, for the reason given.
    Skipped(String),
    /// The surface has no row, so no comparison ran.
    NoRow,
}

/// The result of [`check_api_surfaces`].
#[derive(Debug, Default)]
pub struct ApiChecks {
    pub failures: Vec<String>,
    pub outcomes: Vec<(&'static str, ApiOutcome)>,
}

/// The passing line of the check: the number of surfaces, and which Rust API
/// comparisons ran and which were skipped.
pub fn summary(surfaces: usize, outcomes: &[(&str, ApiOutcome)]) -> String {
    let mut line = format!("freeze: {surfaces} surface(s) checked.");
    if outcomes.is_empty() {
        line.push_str(" No Rust API comparison ran.");
        return line;
    }
    let parts: Vec<String> = outcomes
        .iter()
        .map(|(name, outcome)| match outcome {
            ApiOutcome::Compared => format!("`{name}` compared with its baseline"),
            ApiOutcome::Skipped(reason) => format!("`{name}` skipped ({reason})"),
            ApiOutcome::NoRow => format!("`{name}` not compared (no row in surfaces.toml)"),
        })
        .collect();
    line.push_str(" Rust API comparisons: ");
    line.push_str(&parts.join("; "));
    line.push('.');
    line
}

/// The registry of frozen surfaces that are Rust APIs.
pub fn api_surfaces() -> &'static [ApiSurface] {
    &[
        ApiSurface {
            name: "generated-crate",
            update: Some(rust_api::generated_update),
            current: Some(rust_api::generated_current),
            compare: rust_api::generated_compare,
            skip: None,
        },
        ApiSurface {
            name: "runtime-crates",
            update: None,
            current: None,
            compare: rust_api::runtime_compare,
            skip: Some(rust_api::runtime_skip),
        },
    ]
}

const USAGE: &str = "usage: cargo xtask freeze <update|check> [--root <dir>] \
[--dumper-fixture <name>=<file>] [--fixtures-only] [--api-semver <name>] [--release]\n\
  --root <dir>                      repository root (default: the workspace root)\n\
  --dumper-fixture <name>=<file>    test-only: register a surface whose lines \
are read from <file>\n\
  --fixtures-only                   test-only: skip the registered dumpers and API \
checkers, which read the real repository\n\
  --api-semver <name>               test-only: with --fixtures-only, run the baseline \
comparison of the API surface <name> on the files under --root\n\
  --release                         fail when a surface listed in `required` has no row";

struct Options {
    root: PathBuf,
    fixtures: Vec<(String, PathBuf)>,
    fixtures_only: bool,
    api_semver: Vec<String>,
    release: bool,
}

fn parse_options(args: &[String]) -> Result<Options, String> {
    let mut root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
        .ok_or("cannot find the workspace root")?;
    let mut fixtures = Vec::new();
    let mut fixtures_only = false;
    let mut api_semver = Vec::new();
    let mut release = false;
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
            "--release" => release = true,
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
    if !api_semver.is_empty() && !fixtures_only {
        return Err(format!(
            "--api-semver is test-only and needs --fixtures-only: without it every API \
surface already runs\n{USAGE}"
        ));
    }
    Ok(Options {
        root,
        fixtures,
        fixtures_only,
        api_semver,
        release,
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

fn read_rows(root: &Path) -> Result<Surfaces, String> {
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
    update_api_surfaces(&active_api_surfaces(options), &options.root)
}

/// Runs the `update` step of each API surface whose flag is set.
fn update_api_surfaces(apis: &[(&ApiSurface, bool)], root: &Path) -> Result<(), String> {
    for (surface, regenerate) in apis {
        if let (true, Some(update)) = (*regenerate, surface.update) {
            update(root).map_err(|e| format!("surface `{}`: {e}", surface.name))?;
            println!("updated surface `{}`", surface.name);
        }
    }
    Ok(())
}

/// The failures of the API surfaces: the `current` step of each surface whose
/// flag is set, and the `compare` step of each surface that has a row and
/// whose `skip` step gives no reason. The outcomes say which surfaces were
/// compared.
fn check_api_surfaces(
    apis: &[(&'static ApiSurface, bool)],
    rows: &[Row],
    root: &Path,
) -> ApiChecks {
    let mut checks = ApiChecks::default();
    for (surface, regenerate) in apis {
        let name = surface.name;
        if let (true, Some(current)) = (*regenerate, surface.current) {
            match current(root) {
                Ok(lines) => checks
                    .failures
                    .extend(lines.into_iter().map(|l| format!("surface `{name}`: {l}"))),
                Err(error) => checks.failures.push(format!("surface `{name}`: {error}")),
            }
        }
        let Some(row) = rows.iter().find(|r| r.surface == name) else {
            checks.outcomes.push((name, ApiOutcome::NoRow));
            continue;
        };
        let skipped = match surface.skip.map(|skip| skip(root, row)) {
            Some(Ok(reason)) => reason,
            Some(Err(error)) => {
                checks.failures.push(error);
                continue;
            }
            None => None,
        };
        if let Some(reason) = skipped {
            checks.outcomes.push((name, ApiOutcome::Skipped(reason)));
            continue;
        }
        match (surface.compare)(root, row) {
            Ok(lines) => checks.failures.extend(lines),
            Err(error) => checks.failures.push(error),
        }
        checks.outcomes.push((name, ApiOutcome::Compared));
    }
    checks
}

/// Fails with an instruction to fetch the row's baseline when the ref does
/// not name a commit of the repository at `root`.
fn verify_baseline(root: &Path, row: &Row) -> Result<(), String> {
    let output = bin::git(root)
        .args(["rev-parse", "--verify", "--quiet", "--end-of-options"])
        .arg(format!("{}^{{commit}}", row.baseline))
        .output()
        .map_err(|e| format!("cannot run git: {e}"))?;
    if output.status.success() {
        return Ok(());
    }
    Err(format!(
        "surface `{}`: the baseline `{}` names no commit of this clone (row: name = \"{}\", \
baseline = \"{}\"). Fetch it with `git fetch origin {}`, or with `git fetch --tags` for a \
tag. A shallow clone has no history: in CI, check out with `fetch-depth: 0`.",
        row.surface, row.baseline, row.surface, row.baseline, row.baseline
    ))
}

/// `git show <baseline>:freeze/<surface>.txt` in `root`.
fn baseline_dump(root: &Path, row: &Row) -> Result<String, String> {
    verify_baseline(root, row)?;
    let spec = format!("{}:freeze/{}.txt", row.baseline, row.surface);
    let output = bin::git(root)
        .args(["show", &spec])
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
    let surfaces = read_rows(&options.root)?;
    let rows = &surfaces.rows;
    let mut failures: Vec<String> = Vec::new();

    for row in rows {
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

    for row in rows {
        if !surfaces.required.contains(&row.surface) {
            failures.push(format!(
                "surfaces.toml has a row for `{0}` but `{0}` is not in `required`: add it to \
`required`",
                row.surface
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

    let has_checker = |name: &String| {
        dumps.iter().any(|(dump, _)| dump == name) || apis.iter().any(|(api, _)| api.name == *name)
    };
    for name in &surfaces.required {
        if !has_checker(name) {
            failures.push(format!(
                "surfaces.toml lists `{name}` in `required` but no dumper or API checker has \
that name"
            ));
        }
    }
    for name in rows_missing(&surfaces.required, rows) {
        // A name with no checker was reported above, and no row can be added
        // for it.
        if !has_checker(&name) {
            continue;
        }
        if options.release {
            failures.push(format!(
                "surface `{name}` is required but has no row: a release cannot leave a frozen \
surface without a baseline (see freeze/README.md, \"Approve a break\")"
            ));
        } else {
            println!(
                "freeze: surface `{name}` is required but has no row; add it back with a \
baseline before the next release"
            );
        }
    }

    let api_checks = check_api_surfaces(&apis, rows, &options.root);
    failures.extend(api_checks.failures);

    if failures.is_empty() {
        println!(
            "{}",
            summary(dumps.len() + apis.len(), &api_checks.outcomes)
        );
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
        "update" if options.release => Err(format!("--release applies to `check` only\n{USAGE}")),
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
        let temp = std::env::temp_dir();
        let mut scratch_dirs = vec![
            temp.to_string_lossy().trim_end_matches('/').to_string(),
            bin::target_dir(root).to_string_lossy().into_owned(),
        ];
        if let Ok(canonical) = temp.canonicalize() {
            scratch_dirs.push(canonical.to_string_lossy().into_owned());
        }
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
            // The dumpers write scratch files under the temporary directory
            // and the cargo target directory, and a line must not name either.
            for scratch in &scratch_dirs {
                assert!(
                    !text.contains(scratch.as_str()),
                    "surface `{name}` has the path of a scratch directory, {scratch}"
                );
            }
            assert!(!text.contains('\\'), "surface `{name}` has a backslash");
        }
    }

    fn stub_current(_: &Path) -> Result<Vec<String>, String> {
        Ok(vec!["stale file".into()])
    }

    fn stub_compare(_: &Path, row: &Row) -> Result<Vec<String>, String> {
        Ok(vec![format!("break since `{}`", row.baseline)])
    }

    static STUB_UPDATES: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

    fn stub_update(_: &Path) -> Result<(), String> {
        STUB_UPDATES.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(())
    }

    const STUB: ApiSurface = ApiSurface {
        name: "stub",
        update: Some(stub_update),
        current: Some(stub_current),
        compare: stub_compare,
        skip: None,
    };

    fn stub_skip(_: &Path, row: &Row) -> Result<Option<String>, String> {
        Ok(Some(format!("nothing changed since `{}`", row.baseline)))
    }

    const SKIPPING: ApiSurface = ApiSurface {
        name: "skipping",
        update: None,
        current: None,
        compare: stub_compare,
        skip: Some(stub_skip),
    };

    #[test]
    fn check_api_surfaces_runs_current_when_regenerating() {
        let root = Path::new(".");
        assert_eq!(
            check_api_surfaces(&[(&STUB, true)], &[], root).failures,
            vec!["surface `stub`: stale file"]
        );
        assert!(
            check_api_surfaces(&[(&STUB, false)], &[], root)
                .failures
                .is_empty()
        );
    }

    #[test]
    fn check_api_surfaces_runs_compare_for_a_row_only() {
        let root = Path::new(".");
        let rows = [Row {
            surface: "stub".into(),
            baseline: "v1".into(),
        }];
        assert_eq!(
            check_api_surfaces(&[(&STUB, false)], &rows, root).failures,
            vec!["break since `v1`"]
        );
        let other = [Row {
            surface: "other".into(),
            baseline: "v1".into(),
        }];
        let checks = check_api_surfaces(&[(&STUB, false)], &other, root);
        assert!(checks.failures.is_empty());
        assert_eq!(checks.outcomes, vec![("stub", ApiOutcome::NoRow)]);
    }

    #[test]
    fn check_api_surfaces_skips_the_comparison_when_the_skip_step_gives_a_reason() {
        let root = Path::new(".");
        let rows = [Row {
            surface: "skipping".into(),
            baseline: "v1".into(),
        }];
        let checks = check_api_surfaces(&[(&SKIPPING, false)], &rows, root);
        assert!(checks.failures.is_empty(), "{:?}", checks.failures);
        assert_eq!(
            checks.outcomes,
            vec![(
                "skipping",
                ApiOutcome::Skipped("nothing changed since `v1`".into())
            )]
        );
    }

    fn stub_skip_none(_: &Path, _: &Row) -> Result<Option<String>, String> {
        Ok(None)
    }

    fn stub_skip_error(_: &Path, row: &Row) -> Result<Option<String>, String> {
        Err(format!("cannot read the baseline `{}`", row.baseline))
    }

    #[test]
    fn check_api_surfaces_compares_when_the_skip_step_gives_no_reason() {
        const SURFACE: ApiSurface = ApiSurface {
            skip: Some(stub_skip_none),
            ..STUB
        };
        let rows = [Row {
            surface: "stub".into(),
            baseline: "v1".into(),
        }];
        let checks = check_api_surfaces(&[(&SURFACE, false)], &rows, Path::new("."));
        assert_eq!(checks.failures, vec!["break since `v1`"]);
        assert_eq!(checks.outcomes, vec![("stub", ApiOutcome::Compared)]);
    }

    #[test]
    fn check_api_surfaces_fails_when_the_skip_step_fails() {
        const SURFACE: ApiSurface = ApiSurface {
            skip: Some(stub_skip_error),
            ..STUB
        };
        let rows = [Row {
            surface: "stub".into(),
            baseline: "v1".into(),
        }];
        let checks = check_api_surfaces(&[(&SURFACE, false)], &rows, Path::new("."));
        assert_eq!(checks.failures, vec!["cannot read the baseline `v1`"]);
        assert!(checks.outcomes.is_empty());
    }

    #[test]
    fn check_api_surfaces_records_a_compared_surface() {
        let rows = [Row {
            surface: "stub".into(),
            baseline: "v1".into(),
        }];
        let checks = check_api_surfaces(&[(&STUB, false)], &rows, Path::new("."));
        assert_eq!(checks.outcomes, vec![("stub", ApiOutcome::Compared)]);
    }

    #[test]
    fn summary_says_which_comparisons_ran_and_which_were_skipped() {
        let line = summary(
            11,
            &[
                ("generated-crate", ApiOutcome::Compared),
                (
                    "runtime-crates",
                    ApiOutcome::Skipped("no change since `v1`".into()),
                ),
                ("other", ApiOutcome::NoRow),
            ],
        );
        assert_eq!(
            line,
            "freeze: 11 surface(s) checked. Rust API comparisons: `generated-crate` compared \
with its baseline; `runtime-crates` skipped (no change since `v1`); `other` not compared (no \
row in surfaces.toml)."
        );
        assert_eq!(
            summary(9, &[]),
            "freeze: 9 surface(s) checked. No Rust API comparison ran."
        );
    }

    #[test]
    fn update_api_surfaces_runs_update_when_regenerating() {
        let root = Path::new(".");
        let before = STUB_UPDATES.load(std::sync::atomic::Ordering::SeqCst);
        update_api_surfaces(&[(&STUB, false)], root).unwrap();
        assert_eq!(
            STUB_UPDATES.load(std::sync::atomic::Ordering::SeqCst),
            before
        );
        update_api_surfaces(&[(&STUB, true)], root).unwrap();
        assert_eq!(
            STUB_UPDATES.load(std::sync::atomic::Ordering::SeqCst),
            before + 1
        );
    }

    fn options(fixtures_only: bool, api_semver: &[&str]) -> Options {
        Options {
            root: PathBuf::from("."),
            fixtures: Vec::new(),
            fixtures_only,
            api_semver: api_semver.iter().map(|s| s.to_string()).collect(),
            release: false,
        }
    }

    #[test]
    fn active_api_surfaces_runs_every_surface_with_its_steps() {
        let active = active_api_surfaces(&options(false, &[]));
        let names: Vec<&str> = active.iter().map(|(s, _)| s.name).collect();
        assert_eq!(names, vec!["generated-crate", "runtime-crates"]);
        assert!(active.iter().all(|(_, regenerate)| *regenerate));
        let generated = active[0].0;
        assert!(generated.update.is_some() && generated.current.is_some());
    }

    #[test]
    fn active_api_surfaces_under_fixtures_only_runs_the_named_comparison() {
        assert!(active_api_surfaces(&options(true, &[])).is_empty());
        let active = active_api_surfaces(&options(true, &["generated-crate"]));
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].0.name, "generated-crate");
        assert!(!active[0].1);
    }

    #[test]
    fn parse_options_rejects_api_semver_without_fixtures_only() {
        let args = |list: &[&str]| list.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let error = parse_options(&args(&["--api-semver", "generated-crate"]))
            .err()
            .unwrap();
        assert!(error.contains("--fixtures-only"), "{error}");
        assert!(
            parse_options(&args(&[
                "--fixtures-only",
                "--api-semver",
                "generated-crate"
            ]))
            .is_ok()
        );
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
    fn parse_surfaces_rejects_an_empty_or_dash_leading_baseline() {
        // `git show :freeze/x.txt` reads the index, and a leading dash is read
        // as an option.
        for baseline in ["", "-x", "--all"] {
            let error = parse_surfaces(&format!(
                "[[surface]]\nname = \"x\"\nbaseline = \"{baseline}\"\n"
            ))
            .unwrap_err();
            assert!(
                error.contains("surface `x`") && error.contains("git ref"),
                "{baseline}: {error}"
            );
        }
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
        .unwrap()
        .rows;
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

    #[test]
    fn parse_surfaces_reads_the_required_list() {
        let surfaces = parse_surfaces(
            "required = [\"keywords\", \"cli\"]\n\n[[surface]]\nname = \"keywords\"\nbaseline = \"v0.10.0\"\n",
        )
        .unwrap();
        assert_eq!(
            surfaces.required,
            vec!["keywords".to_string(), "cli".to_string()]
        );
        assert_eq!(surfaces.rows.len(), 1);
    }

    #[test]
    fn parse_surfaces_defaults_required_to_empty() {
        assert!(parse_surfaces("").unwrap().required.is_empty());
    }

    #[test]
    fn parse_surfaces_rejects_a_name_listed_twice_in_required() {
        let error = parse_surfaces("required = [\"cli\", \"cli\"]\n").unwrap_err();
        assert!(
            error.contains("`cli` appears twice in `required`"),
            "{error}"
        );
    }

    #[test]
    fn rows_missing_lists_required_surfaces_without_a_row() {
        let row = |name: &str| Row {
            surface: name.into(),
            baseline: "v0.10.0".into(),
        };
        let rows = vec![row("keywords"), row("cli")];
        let required = vec![
            "keywords".to_string(),
            "cli".to_string(),
            "mcp".to_string(),
            "msrv".to_string(),
        ];
        assert_eq!(
            rows_missing(&required, &rows),
            vec!["mcp".to_string(), "msrv".to_string()]
        );
    }
}
