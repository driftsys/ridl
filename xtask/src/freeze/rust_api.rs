//! The Rust API surfaces, compared with `cargo semver-checks`.
//!
//! - `runtime-crates`: the published runtime crates, compared with the same
//!   crates at the baseline ref (`--baseline-rev`).
//! - `generated-crate`: the crates `ridl build --emit rust` writes for two
//!   source packages, committed under `freeze/generated/`. The sources are
//!   frozen copies under `xtask/tests/freeze-fixtures/generated/`, so an edit
//!   to `examples/` or to a test corpus does not change the frozen API. The committed tree
//!   must equal a regeneration, and for a row it is compared with the tree at
//!   the baseline ref (`--baseline-root`).
//!
//! `cargo semver-checks` runs with `CARGO_NET_OFFLINE=true`, so every
//! dependency must already be in the local cargo cache. Before the first run,
//! the check runs `cargo fetch --locked` with the network allowed, which puts
//! every package of `Cargo.lock` there, for every platform. The `runtime-crates`
//! baseline is built offline too, so when its `Cargo.lock` differs from the
//! working tree's, the check also fetches that lockfile's packages, from the
//! baseline's extracted tree. The `runtime-crates` comparison is skipped when
//! no input of it differs from the baseline ([`runtime_skip`]). The check also
//! runs with `--release-type minor`: a frozen surface accepts no break,
//! whatever version the crates carry. The lints of [`DENIED_LINTS`] are raised
//! to `deny` (see there).

use super::Row;
use super::bin;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;

/// The crates of the `runtime-crates` surface.
pub const RUNTIME_CRATES: [&str; 5] = [
    "ridl-rt",
    "ridl-rt-conformance",
    "ridl-loopback",
    "ridl-descriptor",
    "ridl-transport-ws",
];

/// The generated crates: the directory under `freeze/generated/` and the
/// source package `ridl build` compiles, relative to the repository root. The
/// sources are copies of `examples/cabin` and of
/// `crates/ridlc/tests/corpus/veh-cluster`, taken when the check was added.
/// Their manifests name `flatbuffers` alone, as they did when the copies were
/// taken, so the frozen dumps stay the FlatBuffers ones. The Rust backend
/// emits a codec for `flatbuffers` and for `proto3`, and rejects `podbuf`;
/// `examples/cabin` and the corpus name both codecs, and the copies do
/// not follow them.
const GENERATED: [(&str, &str); 2] = [
    ("cabin", "xtask/tests/freeze-fixtures/generated/cabin"),
    (
        "veh-cluster",
        "xtask/tests/freeze-fixtures/generated/veh-cluster",
    ),
];

/// The lints of cargo-semver-checks 0.51.0 whose required update is major and
/// whose default level is `warn`, so that a break they find would not fail
/// the check. The check raises each to `deny`: in the copied manifests of the
/// generated crates, and through `[workspace.metadata.cargo-semver-checks.lints]`
/// in the root `Cargo.toml` for the runtime crates.
pub const DENIED_LINTS: [&str; 10] = [
    "partial_ord_enum_struct_variant_fields_reordered",
    "partial_ord_enum_variants_reordered",
    "partial_ord_struct_fields_reordered",
    "pub_api_sealed_trait_method_receiver_added",
    "pub_api_sealed_trait_method_receiver_mut_ref_became_ref",
    "pub_api_sealed_trait_method_return_value_added",
    "pub_api_sealed_trait_method_target_feature_removed",
    "repr_c_enum_struct_variant_fields_reordered",
    "repr_c_plain_struct_fields_reordered",
    "union_field_added_with_non_pub_fields",
];

const GENERATED_DIR: &str = "freeze/generated";

/// The exit code of `cargo semver-checks` when it finds a break.
const BREAK_FOUND: i32 = 100;

/// Compares the runtime crates with the same crates at the row's baseline,
/// in one `cargo semver-checks` run.
pub fn runtime_compare(root: &Path, row: &Row) -> Result<Vec<String>, String> {
    fetch_dependencies(root)?;
    verify_runtime_packages(root)?;
    if baseline_lock_differs(root, row)? {
        let tree = scratch_dir(root, "baseline-tree")?;
        fetch_baseline_dependencies(root, row, &tree)?;
    }
    let command = runtime_command(root, row);
    let what = format!("the crates {}", RUNTIME_CRATES.join(", "));
    Ok(run_semver(command, &row.surface, &row.baseline, &what)?
        .into_iter()
        .collect())
}

/// The paths whose content the `runtime-crates` comparison depends on: the
/// directory of each runtime crate, `crates/ridl-ir` (whose items the runtime
/// crates wrap or take in signatures), and the root manifest and lockfile
/// (which pin every dependency). `rust-toolchain.toml` and the sources of
/// `xtask` are not inputs: the comparison reads the rustdoc of the crates, and
/// neither changes an item of them.
fn runtime_inputs() -> Vec<String> {
    RUNTIME_CRATES
        .iter()
        .map(|name| format!("crates/{name}"))
        .chain(
            ["crates/ridl-ir", "Cargo.toml", "Cargo.lock"]
                .into_iter()
                .map(String::from),
        )
        .collect()
}

/// Whether `paths` are the same in the baseline and in the working tree:
/// `git diff --quiet <baseline> -- <paths>` reports no change, and no untracked
/// file is under `paths`. The working tree is compared, not HEAD, so an edit
/// that is not committed yet is not skipped. In CI the working tree is HEAD.
fn unchanged_since(root: &Path, baseline: &str, paths: &[String]) -> Result<bool, String> {
    let status = bin::git(root)
        .args(["diff", "--quiet", "--end-of-options", baseline, "--"])
        .args(paths)
        .status()
        .map_err(|e| format!("cannot run git: {e}"))?;
    match status.code() {
        Some(0) => {}
        Some(1) => return Ok(false),
        code => {
            return Err(format!(
                "`git diff --quiet {baseline}` failed (exit {})",
                code.map_or_else(|| "by signal".to_string(), |c| c.to_string())
            ));
        }
    }
    let output = bin::git(root)
        .args(["ls-files", "--others", "--exclude-standard", "--"])
        .args(paths)
        .output()
        .map_err(|e| format!("cannot run git: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "`git ls-files --others` failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(output.stdout.is_empty())
}

/// Whether the working tree's `Cargo.lock` differs from the baseline's, or is
/// untracked.
fn baseline_lock_differs(root: &Path, row: &Row) -> Result<bool, String> {
    super::verify_baseline(root, row)?;
    Ok(!unchanged_since(
        root,
        &row.baseline,
        &["Cargo.lock".to_string()],
    )?)
}

/// The reason to skip the `runtime-crates` comparison: no input of
/// [`runtime_inputs`] differs between the baseline and the working tree (and
/// none is untracked), so the comparison cannot find a break. `None` when one
/// differs. A baseline that names no commit is an error with the instruction
/// to fetch it.
pub fn runtime_skip(root: &Path, row: &Row) -> Result<Option<String>, String> {
    super::verify_baseline(root, row)?;
    if unchanged_since(root, &row.baseline, &runtime_inputs())? {
        Ok(Some(format!(
            "no runtime crate, `crates/ridl-ir`, `Cargo.toml` or `Cargo.lock` differs from the \
baseline `{}`",
            row.baseline
        )))
    } else {
        Ok(None)
    }
}

/// The names of `crates` that are not a package in the `cargo metadata
/// --no-deps` output `metadata`. `cargo semver-checks` skips a `--package`
/// that names no package, so a renamed crate would be compared with nothing.
fn missing_packages(crates: &[&str], metadata: &str) -> Result<Vec<String>, String> {
    let value: serde_json::Value =
        serde_json::from_str(metadata).map_err(|e| format!("cannot parse cargo metadata: {e}"))?;
    let packages = value["packages"]
        .as_array()
        .ok_or("cargo metadata has no `packages` array")?;
    Ok(crates
        .iter()
        .filter(|name| !packages.iter().any(|p| p["name"].as_str() == Some(**name)))
        .map(|name| name.to_string())
        .collect())
}

/// Fails when a name of [`RUNTIME_CRATES`] is not a package of the workspace.
fn verify_runtime_packages(root: &Path) -> Result<(), String> {
    let mut command = Command::new("cargo");
    command
        .current_dir(root)
        .args(["metadata", "--no-deps", "--format-version", "1"])
        .arg("--manifest-path")
        .arg(root.join("Cargo.toml"))
        .stdin(Stdio::null());
    bin::without_git_env(&mut command);
    let output = command
        .output()
        .map_err(|e| format!("cannot run cargo metadata: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "`cargo metadata` failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let missing = missing_packages(&RUNTIME_CRATES, &String::from_utf8_lossy(&output.stdout))?;
    if missing.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "RUNTIME_CRATES names {} which is not a package of the workspace, so cargo \
semver-checks would compare nothing for it: update RUNTIME_CRATES in \
xtask/src/freeze/rust_api.rs to the package name",
            missing.join(", ")
        ))
    }
}

fn runtime_command(root: &Path, row: &Row) -> Command {
    let mut command = semver_command(root);
    command.arg("--manifest-path").arg(root.join("Cargo.toml"));
    for name in RUNTIME_CRATES {
        command.args(["--package", name]);
    }
    command.args(["--baseline-rev", &row.baseline]);
    command
}

/// Writes the generated crates to `freeze/generated/`, replacing what is there.
pub fn generated_update(root: &Path) -> Result<(), String> {
    let dir = root.join(GENERATED_DIR);
    if dir.exists() {
        std::fs::remove_dir_all(&dir)
            .map_err(|e| format!("cannot remove {}: {e}", dir.display()))?;
    }
    generate(root, &dir)
}

/// Regenerates the crates into a temporary directory and reports every file
/// that differs from `freeze/generated/`.
pub fn generated_current(root: &Path) -> Result<Vec<String>, String> {
    differences_from_regeneration(root, &root.join(GENERATED_DIR), "regenerated")
}

/// Regenerates the crates into the scratch directory `scratch` and reports
/// every file of `committed` that differs from the regeneration.
fn differences_from_regeneration(
    root: &Path,
    committed: &Path,
    scratch: &str,
) -> Result<Vec<String>, String> {
    let scratch = scratch_dir(root, scratch)?;
    generate(root, &scratch)?;
    let fresh = read_tree(&scratch)?;
    let committed = read_tree(committed)?;
    Ok(tree_differences(&committed, &fresh)
        .into_iter()
        .map(|d| format!("{d}: run `cargo xtask freeze update` and commit the result"))
        .collect())
}

/// Compares each crate of `freeze/generated/` with the same crate in the tree
/// at the row's baseline. A crate of the baseline that is gone at HEAD fails;
/// a crate added since passes.
pub fn generated_compare(root: &Path, row: &Row) -> Result<Vec<String>, String> {
    fetch_dependencies(root)?;
    let head = scratch_dir(root, "head")?;
    copy_tree(&root.join(GENERATED_DIR), &head)?;
    let baseline = scratch_dir(root, "baseline")?;
    extract_baseline(root, row, &baseline, &[GENERATED_DIR])?;
    let baseline = baseline.join(GENERATED_DIR);
    let ridl_rt = root.join("crates").join("ridl-rt");
    let mut failures = Vec::new();
    for name in crate_dirs(&baseline)? {
        let what = format!("the crate {GENERATED_DIR}/{name}");
        if !head.join(&name).join("Cargo.toml").is_file() {
            failures.push(format!(
                "surface `{}`: {what} is in the baseline `{}` and missing at HEAD",
                row.surface, row.baseline
            ));
            continue;
        }
        for side in [&head, &baseline] {
            let manifest = side.join(&name).join("Cargo.toml");
            let text = std::fs::read_to_string(&manifest)
                .map_err(|e| format!("cannot read {}: {e}", manifest.display()))?;
            // A manifest that is not TOML is left as it is; cargo then reports it.
            if let Ok(rewritten) = standalone_manifest(&text, &ridl_rt) {
                std::fs::write(&manifest, rewritten)
                    .map_err(|e| format!("cannot write {}: {e}", manifest.display()))?;
            }
        }
        let mut command = semver_command(root);
        command
            .arg("--manifest-path")
            .arg(head.join(&name).join("Cargo.toml"))
            .arg("--baseline-root")
            .arg(baseline.join(&name));
        failures.extend(run_semver(command, &row.surface, &row.baseline, &what)?);
    }
    Ok(failures)
}

/// Runs `ridl build --emit rust` for each generated crate into `out/<name>`.
fn generate(root: &Path, out: &Path) -> Result<(), String> {
    let ridl = bin::binary(root, "ridl")?;
    for (name, source) in GENERATED {
        let target = out.join(name);
        let target_text = target.to_string_lossy();
        let run = bin::run(
            &ridl,
            root,
            &["build", source, "--emit", "rust", "--out-dir", &target_text],
        )?;
        if run.code != 0 {
            return Err(format!(
                "`ridl build {source} --emit rust` exited with {}:\n{}",
                run.code, run.stderr
            ));
        }
    }
    Ok(())
}

/// An empty directory `<target>/freeze-api/<name>`. Its path is the same on
/// every run, so the rustdoc builds under `<target>/freeze-api/target` are
/// reused.
fn scratch_dir(root: &Path, name: &str) -> Result<PathBuf, String> {
    let dir = bin::target_dir(root).join("freeze-api").join(name);
    if dir.exists() {
        std::fs::remove_dir_all(&dir)
            .map_err(|e| format!("cannot remove {}: {e}", dir.display()))?;
    }
    std::fs::create_dir_all(&dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    Ok(dir)
}

/// Every file under `dir`, keyed by its path relative to `dir` with `/`
/// separators. A missing `dir` is an empty tree.
fn read_tree(dir: &Path) -> Result<BTreeMap<String, Vec<u8>>, String> {
    let mut files = BTreeMap::new();
    if !dir.exists() {
        return Ok(files);
    }
    for path in bin::files_below(dir)? {
        let name = path
            .strip_prefix(dir)
            .map_err(|e| e.to_string())?
            .to_string_lossy()
            .replace('\\', "/");
        let bytes =
            std::fs::read(&path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        files.insert(name, bytes);
    }
    Ok(files)
}

/// One line per file that is missing from, extra in, or different in
/// `committed` compared with `fresh`.
fn tree_differences(
    committed: &BTreeMap<String, Vec<u8>>,
    fresh: &BTreeMap<String, Vec<u8>>,
) -> Vec<String> {
    let mut lines = Vec::new();
    for (path, bytes) in fresh {
        match committed.get(path) {
            None => lines.push(format!("{GENERATED_DIR}/{path} is missing")),
            Some(old) if old != bytes => lines.push(format!(
                "{GENERATED_DIR}/{path} differs from the regenerated file"
            )),
            Some(_) => {}
        }
    }
    for path in committed.keys().filter(|p| !fresh.contains_key(*p)) {
        lines.push(format!(
            "{GENERATED_DIR}/{path} is not written by `ridl build`"
        ));
    }
    lines
}

fn copy_tree(from: &Path, to: &Path) -> Result<(), String> {
    for (path, bytes) in read_tree(from)? {
        let target = to.join(&path);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("cannot create {}: {e}", parent.display()))?;
        }
        std::fs::write(&target, bytes)
            .map_err(|e| format!("cannot write {}: {e}", target.display()))?;
    }
    Ok(())
}

/// Extracts `paths` of the row's baseline into `dir` with `git archive`; no
/// path extracts the whole tree.
fn extract_baseline(root: &Path, row: &Row, dir: &Path, paths: &[&str]) -> Result<(), String> {
    super::verify_baseline(root, row)?;
    // The archive is written beside `dir`, so it is not part of the extracted tree.
    let archive = dir.with_extension("tar");
    let output = bin::git(root)
        .args(["archive", "--format=tar", "-o"])
        .arg(&archive)
        .arg(&row.baseline)
        .args(paths)
        .output()
        .map_err(|e| format!("cannot run git: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "surface `{}`: cannot read `{}:{}` (baseline `{}`): {}",
            row.surface,
            row.baseline,
            paths.join(" "),
            row.baseline,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let output = Command::new("tar")
        .arg("-xf")
        .arg(&archive)
        .arg("-C")
        .arg(dir)
        .output()
        .map_err(|e| format!("cannot run tar: {e}"))?;
    let _ = std::fs::remove_file(&archive);
    if !output.status.success() {
        return Err(format!(
            "cannot extract {}: {}",
            archive.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(())
}

/// The names of the directories under `dir` that hold a `Cargo.toml`, sorted.
fn crate_dirs(dir: &Path) -> Result<Vec<String>, String> {
    let entries =
        std::fs::read_dir(dir).map_err(|e| format!("cannot read {}: {e}", dir.display()))?;
    let mut names: Vec<String> = entries
        .flatten()
        .filter(|e| e.path().join("Cargo.toml").is_file())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    Ok(names)
}

/// The manifest of a copied generated crate, made to build outside the
/// workspace without the network: an empty `[workspace]` table, and the
/// `ridl-rt` dependency read from `ridl_rt` (HEAD's crate, for both sides),
/// and every lint of [`DENIED_LINTS`] at `deny`.
///
/// The dependency is replaced, not patched: `cargo semver-checks` builds the
/// crate as a dependency of a crate of its own, and cargo applies
/// `[patch]` only from the root manifest. The version requirement is dropped,
/// so a baseline that asks for an older `ridl-rt` still builds.
fn standalone_manifest(text: &str, ridl_rt: &Path) -> Result<String, String> {
    let mut manifest: toml::Table = toml::from_str(text).map_err(|e| e.to_string())?;
    if let Some(toml::Value::Table(dependencies)) = manifest.get_mut("dependencies")
        && let Some(dependency) = dependencies.get_mut("ridl-rt")
    {
        let mut table = match dependency {
            toml::Value::Table(table) => table.clone(),
            _ => toml::Table::new(),
        };
        table.remove("version");
        table.insert(
            "path".into(),
            toml::Value::String(ridl_rt.to_string_lossy().into_owned()),
        );
        *dependency = toml::Value::Table(table);
    }
    let lints: toml::Table = DENIED_LINTS
        .iter()
        .map(|lint| (lint.to_string(), toml::Value::String("deny".into())))
        .collect();
    let mut semver = toml::Table::new();
    semver.insert("lints".into(), toml::Value::Table(lints));
    let package = manifest
        .entry("package")
        .or_insert_with(|| toml::Value::Table(toml::Table::new()));
    if let toml::Value::Table(package) = package {
        let metadata = package
            .entry("metadata")
            .or_insert_with(|| toml::Value::Table(toml::Table::new()));
        if let toml::Value::Table(metadata) = metadata {
            metadata.insert("cargo-semver-checks".into(), toml::Value::Table(semver));
        }
    }
    manifest.insert("workspace".into(), toml::Value::Table(toml::Table::new()));
    toml::to_string(&manifest).map_err(|e| e.to_string())
}

/// `cargo fetch --locked` for the workspace at `root`, with the network
/// allowed: it downloads every package of `Cargo.lock`, for every platform.
/// `cargo semver-checks` runs offline and reads `cargo metadata`, which needs
/// all of them in the local cargo cache, and a build of the workspace
/// downloads only the packages of the host platform.
fn fetch_command(root: &Path) -> Command {
    let mut command = Command::new("cargo");
    command
        .current_dir(root)
        .args(["fetch", "--locked", "--manifest-path"])
        .arg(root.join("Cargo.toml"))
        .env_remove("CARGO_NET_OFFLINE");
    bin::without_git_env(&mut command);
    command
}

/// `cargo fetch --locked` against the lockfile of the row's baseline, with the
/// network allowed. The baseline's runtime crates are built offline, and a
/// package that only the baseline's `Cargo.lock` names (an exact pin or a
/// requirement that HEAD's lockfile no longer satisfies) is not in the cache
/// that [`fetch_dependencies`] fills. The baseline tree is extracted into a
/// directory `tree` first, because `cargo fetch` needs the manifests of every
/// workspace member.
fn fetch_baseline_dependencies(root: &Path, row: &Row, tree: &Path) -> Result<(), String> {
    extract_baseline(root, row, tree, &[])?;
    let output = fetch_command(tree)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("cannot run cargo fetch: {e}"))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "surface `{}`: `cargo fetch --locked` of the baseline `{}` failed, so the baseline \
cannot be built offline:\n{}",
            row.surface,
            row.baseline,
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

static FETCH: OnceLock<Result<(), String>> = OnceLock::new();

/// Runs [`fetch_command`] once per process. A `root` without a `Cargo.toml`
/// (the temporary repositories of the tests that hold only
/// `freeze/generated/`) has nothing to fetch.
fn fetch_dependencies(root: &Path) -> Result<(), String> {
    if !root.join("Cargo.toml").is_file() {
        return Ok(());
    }
    FETCH
        .get_or_init(|| {
            let output = fetch_command(root)
                .stdin(Stdio::null())
                .output()
                .map_err(|e| format!("cannot run cargo fetch: {e}"))?;
            if output.status.success() {
                Ok(())
            } else {
                Err(format!(
                    "`cargo fetch --locked` failed, so cargo semver-checks cannot run \
offline:\n{}",
                    String::from_utf8_lossy(&output.stderr).trim()
                ))
            }
        })
        .clone()
}

fn semver_command(root: &Path) -> Command {
    let mut command = Command::new("cargo");
    command
        .current_dir(root)
        .args(["semver-checks", "check-release", "--release-type", "minor"])
        .args(["--color", "never"])
        .env("CARGO_NET_OFFLINE", "true")
        .env(
            "CARGO_TARGET_DIR",
            bin::target_dir(root).join("freeze-api").join("target"),
        );
    bin::without_git_env(&mut command);
    command
}

/// Runs `command` with its standard output and standard error written to one
/// file, so that each finding follows the line that names its crate. Returns
/// one failure for a break, none for a pass, and an error for any other exit.
fn run_semver(
    mut command: Command,
    surface: &str,
    baseline: &str,
    what: &str,
) -> Result<Option<String>, String> {
    let log_path = std::env::temp_dir().join(format!(
        "xtask-freeze-semver-{}-{surface}.log",
        std::process::id()
    ));
    let log = std::fs::File::create(&log_path)
        .map_err(|e| format!("cannot create {}: {e}", log_path.display()))?;
    let log_err = log
        .try_clone()
        .map_err(|e| format!("cannot open {}: {e}", log_path.display()))?;
    let status = command
        .stdin(Stdio::null())
        .stdout(Stdio::from(log))
        .stderr(Stdio::from(log_err))
        .status()
        .map_err(|e| format!("cannot run cargo semver-checks: {e}"))?;
    let text = std::fs::read_to_string(&log_path).unwrap_or_default();
    let _ = std::fs::remove_file(&log_path);
    let findings = without_progress(&text);
    match status.code() {
        Some(0) => Ok(None),
        Some(BREAK_FOUND) => Ok(Some(format!(
            "surface `{surface}`: cargo semver-checks finds a break in {what} since the \
baseline `{baseline}`:\n{findings}"
        ))),
        code => Err(format!(
            "surface `{surface}`: cargo semver-checks failed on {what} (exit {}), so the \
API was not compared with the baseline `{baseline}`:\n{findings}",
            code.map_or_else(|| "by signal".to_string(), |c| c.to_string())
        )),
    }
}

/// `text` without the build progress lines of `cargo semver-checks`.
fn without_progress(text: &str) -> String {
    const PROGRESS: [&str; 6] = [
        "Building", "Built", "Parsing", "Parsed", "Cloning", "Finished",
    ];
    text.lines()
        .filter(|line| {
            let word = line.split_whitespace().next().unwrap_or("");
            !PROGRESS.contains(&word)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree(files: &[(&str, &str)]) -> BTreeMap<String, Vec<u8>> {
        files
            .iter()
            .map(|(p, t)| (p.to_string(), t.as_bytes().to_vec()))
            .collect()
    }

    #[test]
    fn generated_dump_is_current() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let differences = generated_current(root).unwrap();
        assert!(differences.is_empty(), "{differences:#?}");
    }

    #[test]
    fn a_generated_crate_that_differs_from_a_regeneration_fails() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let committed = scratch_dir(root, "edited-copy").unwrap();
        copy_tree(&root.join(GENERATED_DIR), &committed).unwrap();
        let lib = committed.join("cabin/lib.rs");
        let text = std::fs::read_to_string(&lib).unwrap();
        std::fs::write(&lib, format!("{text}\npub fn edited() {{}}\n")).unwrap();
        std::fs::remove_file(committed.join("veh-cluster/veh.common.rs")).unwrap();
        let differences =
            differences_from_regeneration(root, &committed, "regenerated-edited").unwrap();
        assert_eq!(
            differences,
            vec![
                "freeze/generated/cabin/lib.rs differs from the regenerated file: run `cargo \
xtask freeze update` and commit the result",
                "freeze/generated/veh-cluster/veh.common.rs is missing: run `cargo xtask \
freeze update` and commit the result",
            ]
        );
    }

    #[test]
    fn tree_differences_names_missing_extra_and_changed_files() {
        let committed = tree(&[
            ("a/Cargo.toml", "x"),
            ("a/lib.rs", "old"),
            ("a/gone.rs", ""),
        ]);
        let fresh = tree(&[("a/Cargo.toml", "x"), ("a/lib.rs", "new"), ("a/new.rs", "")]);
        assert_eq!(
            tree_differences(&committed, &fresh),
            vec![
                "freeze/generated/a/lib.rs differs from the regenerated file",
                "freeze/generated/a/new.rs is missing",
                "freeze/generated/a/gone.rs is not written by `ridl build`",
            ]
        );
        assert!(tree_differences(&fresh, &fresh).is_empty());
    }

    #[test]
    fn standalone_manifest_reads_ridl_rt_from_a_path() {
        let text = "[package]\nname = \"g\"\n\n[dependencies]\n\
ridl-rt = { version = \"0.8\", features = [\"flatbuffers\"] }\nregex = \"1\"\n";
        let manifest: toml::Table =
            toml::from_str(&standalone_manifest(text, Path::new("/r/crates/ridl-rt")).unwrap())
                .unwrap();
        let ridl_rt = &manifest["dependencies"]["ridl-rt"];
        assert_eq!(ridl_rt["path"].as_str(), Some("/r/crates/ridl-rt"));
        assert!(ridl_rt.get("version").is_none());
        assert_eq!(ridl_rt["features"][0].as_str(), Some("flatbuffers"));
        assert_eq!(manifest["dependencies"]["regex"].as_str(), Some("1"));
        assert!(manifest["workspace"].as_table().unwrap().is_empty());
        let lints = manifest["package"]["metadata"]["cargo-semver-checks"]["lints"]
            .as_table()
            .unwrap();
        assert_eq!(lints.len(), DENIED_LINTS.len());
        for lint in DENIED_LINTS {
            assert_eq!(lints[lint].as_str(), Some("deny"), "{lint}");
        }
    }

    #[test]
    fn missing_packages_names_each_crate_that_is_not_a_package() {
        let metadata = r#"{"packages":[{"name":"ridl-rt"},{"name":"xtask"}]}"#;
        assert_eq!(
            missing_packages(&["ridl-rt", "ridl-rt-old", "gone"], metadata).unwrap(),
            ["ridl-rt-old", "gone"]
        );
        assert!(missing_packages(&["ridl-rt"], metadata).unwrap().is_empty());
        assert!(missing_packages(&["ridl-rt"], "{}").is_err());
    }

    #[test]
    fn every_runtime_crate_is_a_package_of_this_workspace() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        verify_runtime_packages(root).unwrap();
    }

    #[test]
    fn runtime_command_checks_every_runtime_crate_against_the_baseline() {
        let row = Row {
            surface: "runtime-crates".into(),
            baseline: "v9".into(),
        };
        let command = runtime_command(Path::new("/r"), &row);
        let args: Vec<String> = command
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        let has = |flag: &str, value: &str| args.windows(2).any(|w| w[0] == flag && w[1] == value);
        for name in RUNTIME_CRATES {
            assert!(has("--package", name), "{args:?}");
        }
        assert!(has("--baseline-rev", "v9"), "{args:?}");
        assert!(has("--release-type", "minor"), "{args:?}");
        assert!(has("--manifest-path", "/r/Cargo.toml"), "{args:?}");
        assert!(
            command
                .get_envs()
                .any(|(k, v)| k == "CARGO_NET_OFFLINE" && v == Some("true".as_ref())),
            "the check must not read the network"
        );
    }

    #[test]
    fn fetch_command_fetches_the_locked_dependencies_with_the_network() {
        let command = fetch_command(Path::new("/r"));
        let args: Vec<String> = command
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            args,
            ["fetch", "--locked", "--manifest-path", "/r/Cargo.toml"]
        );
        // The variable is removed, so a CARGO_NET_OFFLINE of the caller does
        // not stop the fetch.
        assert!(
            command
                .get_envs()
                .any(|(k, v)| k == "CARGO_NET_OFFLINE" && v.is_none()),
            "the fetch must read the network"
        );
    }

    fn git_in(dir: &Path, args: &[&str]) {
        let status = bin::git(dir)
            .args(["-c", "user.name=t", "-c", "user.email=t@example.com"])
            .args(["-c", "commit.gpgsign=false"])
            .args(args)
            .status()
            .unwrap();
        assert!(status.success(), "git {args:?}");
    }

    /// A temporary directory that is removed when it is dropped.
    struct Scratch(PathBuf);

    impl std::ops::Deref for Scratch {
        type Target = Path;
        fn deref(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// A repository with one commit `base`, then one commit that changes
    /// `change` (a path relative to the repository).
    fn repo_with_change(name: &str, change: &str) -> Scratch {
        let dir = std::env::temp_dir().join(format!("xtask-freeze-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let dir = Scratch(dir);
        for path in runtime_inputs()
            .iter()
            .map(String::as_str)
            .chain(["notes/x"])
        {
            let file = if path.ends_with(".toml") || path.ends_with(".lock") {
                dir.join(path)
            } else {
                dir.join(path).join("f")
            };
            std::fs::create_dir_all(file.parent().unwrap()).unwrap();
            std::fs::write(file, "a").unwrap();
        }
        git_in(&dir, &["init", "-q", "-b", "main"]);
        git_in(&dir, &["add", "-A"]);
        git_in(&dir, &["commit", "-q", "-m", "base"]);
        git_in(&dir, &["tag", "base"]);
        std::fs::write(dir.join(change), "b").unwrap();
        git_in(&dir, &["add", "-A"]);
        git_in(&dir, &["commit", "-q", "-m", "change"]);
        dir
    }

    fn runtime_row() -> Row {
        Row {
            surface: "runtime-crates".into(),
            baseline: "base".into(),
        }
    }

    #[test]
    fn runtime_inputs_name_every_runtime_crate_ridl_ir_and_the_root_files() {
        let inputs = runtime_inputs();
        for name in RUNTIME_CRATES {
            assert!(inputs.contains(&format!("crates/{name}")), "{inputs:?}");
        }
        for path in ["crates/ridl-ir", "Cargo.toml", "Cargo.lock"] {
            assert!(inputs.contains(&path.to_string()), "{inputs:?}");
        }
    }

    #[test]
    fn runtime_skip_skips_when_only_other_files_changed() {
        let dir = repo_with_change("skip", "notes/x/f");
        let reason = runtime_skip(&dir, &runtime_row()).unwrap();
        assert!(reason.unwrap().contains("baseline `base`"));
        assert!(!baseline_lock_differs(&dir, &runtime_row()).unwrap());
    }

    #[test]
    fn runtime_skip_runs_the_comparison_when_a_listed_path_changed() {
        for changed in [
            "crates/ridl-descriptor/f",
            "crates/ridl-transport-ws/f",
            "crates/ridl-ir/f",
            "Cargo.toml",
            "Cargo.lock",
        ] {
            let dir = repo_with_change("run", changed);
            assert_eq!(
                runtime_skip(&dir, &runtime_row()).unwrap(),
                None,
                "{changed}"
            );
        }
    }

    #[test]
    fn runtime_skip_runs_the_comparison_for_an_uncommitted_change() {
        let dir = repo_with_change("dirty", "notes/x/f");
        std::fs::write(dir.join("crates/ridl-rt/f"), "edited").unwrap();
        assert_eq!(runtime_skip(&dir, &runtime_row()).unwrap(), None);
    }

    #[test]
    fn runtime_skip_runs_the_comparison_for_an_untracked_file() {
        let dir = repo_with_change("untracked", "notes/x/f");
        std::fs::write(dir.join("crates/ridl-ir/new.rs"), "x").unwrap();
        assert_eq!(runtime_skip(&dir, &runtime_row()).unwrap(), None);
    }

    #[test]
    fn runtime_skip_fails_for_a_baseline_that_names_no_commit() {
        let dir = repo_with_change("nobase", "notes/x/f");
        let row = Row {
            surface: "runtime-crates".into(),
            baseline: "missing".into(),
        };
        let error = runtime_skip(&dir, &row).unwrap_err();
        assert!(
            error.contains("names no commit") && error.contains("`git fetch origin missing`"),
            "{error}"
        );
    }

    #[test]
    fn the_baseline_lockfile_is_fetched_from_the_baseline_tree() {
        let dir = std::env::temp_dir().join(format!("xtask-freeze-{}-fetch", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let dir = Scratch(dir);
        let tree = Scratch(
            std::env::temp_dir().join(format!("xtask-freeze-{}-fetch-tree", std::process::id())),
        );
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::write(dir.join("src/lib.rs"), "").unwrap();
        std::fs::write(
            dir.join("Cargo.toml"),
            "[package]\nname = \"p\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        )
        .unwrap();
        std::fs::write(
            dir.join("Cargo.lock"),
            "version = 4\n\n[[package]]\nname = \"p\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();
        git_in(&dir, &["init", "-q", "-b", "main"]);
        git_in(&dir, &["add", "-A"]);
        git_in(&dir, &["commit", "-q", "-m", "base"]);
        git_in(&dir, &["tag", "base"]);
        // At HEAD the lockfile no longer matches the manifest. The fetch must
        // read the baseline's tree, not the working tree.
        std::fs::write(dir.join("Cargo.lock"), "garbage").unwrap();
        assert!(baseline_lock_differs(&dir, &runtime_row()).unwrap());
        std::fs::create_dir_all(&*tree).unwrap();
        fetch_baseline_dependencies(&dir, &runtime_row(), &tree).unwrap();
        assert!(!tree.with_extension("tar").exists());
        assert!(!tree.join("baseline.tar").exists());
        // A baseline whose lockfile is out of date for its manifest fails.
        std::fs::write(dir.join("Cargo.lock"), "").unwrap();
        git_in(&dir, &["add", "-A"]);
        git_in(&dir, &["commit", "-q", "-m", "empty lock"]);
        git_in(&dir, &["tag", "bad"]);
        let bad = Row {
            surface: "runtime-crates".into(),
            baseline: "bad".into(),
        };
        let error = fetch_baseline_dependencies(&dir, &bad, &tree).unwrap_err();
        assert!(error.contains("baseline `bad`"), "{error}");
    }

    #[test]
    fn runtime_crates_deny_the_lints_that_warn_by_default() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let read = |path: PathBuf| -> toml::Table {
            toml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap()
        };
        let workspace = read(root.join("Cargo.toml"));
        let lints = workspace["workspace"]["metadata"]["cargo-semver-checks"]["lints"]
            .as_table()
            .unwrap();
        let mut names: Vec<&str> = lints.keys().map(String::as_str).collect();
        names.sort();
        assert_eq!(names, DENIED_LINTS.to_vec());
        assert!(lints.values().all(|v| v.as_str() == Some("deny")));
        // The runtime crates are the crates that use the workspace's semver
        // lints, so a crate dropped from `RUNTIME_CRATES` fails here.
        let mut opted_in: Vec<String> = std::fs::read_dir(root.join("crates"))
            .unwrap()
            .flatten()
            .filter(|entry| {
                let path = entry.path().join("Cargo.toml");
                path.is_file()
                    && read(path)
                        .get("package")
                        .and_then(|p| p.get("metadata"))
                        .and_then(|m| m.get("cargo-semver-checks"))
                        .is_some()
            })
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .collect();
        opted_in.sort();
        let mut expected: Vec<&str> = RUNTIME_CRATES.to_vec();
        expected.sort();
        assert_eq!(opted_in, expected);
        for name in RUNTIME_CRATES {
            let manifest = read(root.join("crates").join(name).join("Cargo.toml"));
            assert_eq!(
                manifest["package"]["metadata"]["cargo-semver-checks"]["lints"]["workspace"]
                    .as_bool(),
                Some(true),
                "{name} does not use the workspace's semver lints"
            );
        }
    }

    #[test]
    fn without_progress_keeps_findings_and_crate_names() {
        let text = [
            "    Building ridl-rt v0.8.0 (current)",
            "       Built [   0.8s] (current)",
            "    Checking ridl-rt v0.8.0 -> v0.8.0 (assume minor change)",
            "--- failure struct_missing: pub struct removed or renamed ---",
            "    Finished [   2.0s] ridl-rt",
        ]
        .join("\n");
        assert_eq!(
            without_progress(&text),
            [
                "    Checking ridl-rt v0.8.0 -> v0.8.0 (assume minor change)",
                "--- failure struct_missing: pub struct removed or renamed ---",
            ]
            .join("\n")
        );
    }
}
