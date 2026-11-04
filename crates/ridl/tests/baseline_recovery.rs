//! Explicit recovery preserves historical artifacts and publication guards.
mod cli_reference_support;
mod remote_support;
use remote_support::{Stub, TempDir, ridl, unit_tar};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
const OLD: &str =
    "package veh.common\ntype Level: integer [0..100]\nstruct Reading { level: Level }\n";
const GROWN: &str = "package veh.common\ntype Level: integer [0..100]\ntype Wide: integer [0..9223372036854775807]\nstruct Reading { level: Level, extra: Wide }\n";
const LOCAL: &str = "package app\nimport veh.common.Reading\nstruct Envelope { reading: Reading }\ninterface Probe { signal reading: Envelope @10ms }\n";
fn stub(source: &str) -> Stub {
    Stub::serving(unit_tar(&[
        (
            "ridl.toml",
            "[package]\nname = \"veh.common\"\nversion = \"1.0.0\"\n",
        ),
        ("common.typl", source),
    ]))
}
fn manifest(dir: &TempDir, url: &str) {
    dir.write("ridl.toml", &format!("[package]\nname = \"app\"\nversion = \"1.0.0\"\n[imports]\n\"veh.common\" = \"{url}\"\n[backend.rust]\nencodings = [\"podbuf\"]\n"));
}
fn command(root: &Path, home: &Path, args: &[&str], expected: i32) -> String {
    let (code, _, stderr) = ridl(root, home, args);
    assert_eq!(code, expected, "{args:?}: {stderr}");
    stderr
}
fn setup(dir: &TempDir, home: &TempDir, url: &str) -> PathBuf {
    manifest(dir, url);
    dir.write("app.ridl", LOCAL);
    command(dir.path(), home.path(), &["check"], 0);
    command(dir.path(), home.path(), &["lock"], 0);
    command(dir.path(), home.path(), &["baseline"], 0);
    dir.path().join(".ridl/baseline")
}
fn inventory(dir: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn visit(root: &Path, dir: &Path, files: &mut BTreeMap<PathBuf, Vec<u8>>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                visit(root, &path, files);
            } else {
                files.insert(
                    path.strip_prefix(root).unwrap().into(),
                    std::fs::read(path).unwrap(),
                );
            }
        }
    }
    let mut files = BTreeMap::new();
    visit(dir, dir, &mut files);
    files
}
// Run the reference's recovery command forms against real source and output
// directories; dropping --out would recover the wrong baseline and fail the
// existing preservation and history assertions.
fn recovery_example(flag: &str, source: &Path, baseline: &Path) -> Vec<String> {
    let page = cli_reference_support::page("cli-reference.md");
    let blocks = cli_reference_support::fences(&page);
    let commands: Vec<_> = blocks
        .iter()
        .filter(|block| block.info == "sh")
        .flat_map(|block| block.body.lines())
        .map(|line| line.split_whitespace().collect::<Vec<_>>())
        .filter(|args| args.starts_with(&["ridl", "baseline"]) && args.contains(&flag))
        .collect();
    assert_eq!(
        commands.len(),
        1,
        "one executable recovery example for {flag}"
    );
    commands[0]
        .iter()
        .skip(1)
        .map(|arg| match *arg {
            "HISTORICAL_SOURCE" | "CURRENT_SOURCE" => source.to_str().unwrap().to_string(),
            "BASELINE" => baseline.to_str().unwrap().to_string(),
            other => other.to_string(),
        })
        .collect()
}
fn context(dir: &Path) -> ridlc::baseline_context::BaselineContext {
    let local =
        ridl_ir::v2::from_json(&std::fs::read_to_string(dir.join("app.ir.json")).unwrap()).unwrap();
    ridlc::baseline_context::read_context(dir, &[local])
        .unwrap()
        .unwrap()
}
fn assert_usable(dir: &TempDir, home: &TempDir, baseline: &Path) {
    command(
        dir.path(),
        home.path(),
        &[
            "diff",
            baseline.to_str().unwrap(),
            ".",
            "--encoding",
            "podbuf",
        ],
        0,
    );
    command(
        dir.path(),
        home.path(),
        &["build", "--emit", "catalog", "--out-dir", "built"],
        0,
    );
    command(dir.path(), home.path(), &["baseline"], 0);
}
#[test]
fn legacy_baseline_reset_unblocks_diff_and_preserves_prior_bytes() {
    let dir = TempDir::new("reset");
    let home = TempDir::new("reset-home");
    let old = stub(OLD);
    let baseline = setup(&dir, &home, old.url());
    let old_pin = context(&baseline).remote_context().pins["veh.common"].clone();
    std::fs::remove_file(baseline.join("context.json")).unwrap();
    let prior = inventory(&baseline);
    let new = stub(GROWN);
    manifest(&dir, new.url());
    command(dir.path(), home.path(), &["check"], 0);
    command(
        dir.path(),
        home.path(),
        &[
            "diff",
            baseline.to_str().unwrap(),
            ".",
            "--encoding",
            "podbuf",
        ],
        2,
    );
    command(dir.path(), home.path(), &["baseline"], 2);
    let args = recovery_example("--reset-history", dir.path(), &baseline);
    let stderr = command(
        dir.path(),
        home.path(),
        &args.iter().map(String::as_str).collect::<Vec<_>>(),
        0,
    );
    let backup = dir.path().join(".ridl/baseline.previous.1");
    assert!(stderr.contains(backup.to_str().unwrap()));
    assert_eq!(inventory(&backup), prior);
    let new_pin = context(&baseline).remote_context().pins["veh.common"].clone();
    assert_eq!(new_pin.url, new.url());
    assert_ne!(new_pin.tree, old_pin.tree);
    assert_eq!(
        std::fs::read_to_string(baseline.join("app.catalogs"))
            .unwrap()
            .lines()
            .filter(|l| !l.starts_with('#') && !l.is_empty())
            .count(),
        1
    );
    assert_usable(&dir, &home, &baseline);
}
#[test]
fn historical_repair_refuses_changed_local_snapshots_and_preserves_baseline() {
    let dir = TempDir::new("repair-mismatch");
    let home = TempDir::new("repair-mismatch-home");
    let old = stub(OLD);
    let baseline = setup(&dir, &home, old.url());
    std::fs::remove_file(baseline.join("context.json")).unwrap();
    let prior = inventory(&baseline);
    dir.write(
        "app.ridl",
        &LOCAL.replace(
            "struct Envelope { reading: Reading }",
            "struct Envelope { reading: Reading, other: Reading }",
        ),
    );
    let stderr = command(
        dir.path(),
        home.path(),
        &["baseline", "--repair-context"],
        2,
    );
    assert!(
        stderr.contains("historical") && stderr.contains("snapshot"),
        "{stderr}"
    );
    assert_eq!(inventory(&baseline), prior);
    assert!(!dir.path().join(".ridl/baseline.previous.1").exists());
}
#[test]
fn historical_repair_preserves_snapshot_bytes_and_uses_frozen_pins() {
    let dir = TempDir::new("repair");
    let home = TempDir::new("repair-home");
    let old = stub(OLD);
    let baseline = setup(&dir, &home, old.url());
    let old_context = context(&baseline);
    command(dir.path(), home.path(), &["vendor"], 0);
    std::fs::remove_file(baseline.join("context.json")).unwrap();
    let snapshot = baseline.join("app.ir.json");
    let value: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&snapshot).unwrap()).unwrap();
    std::fs::write(&snapshot, serde_json::to_vec(&value).unwrap()).unwrap();
    let prior = inventory(&baseline);
    let lock = std::fs::read(dir.path().join("ridl.lock")).unwrap();
    let cache = inventory(home.path());
    let vendor = inventory(&dir.path().join("vendor"));
    old.reset_hits();
    let stderr = command(
        dir.path(),
        home.path(),
        &["baseline", "--repair-context"],
        0,
    );
    assert!(
        stderr.contains("historical") && stderr.contains("lockfile"),
        "{stderr}"
    );
    assert_eq!(old.hits(), 0);
    assert_eq!(inventory(home.path()), cache);
    assert_eq!(inventory(&dir.path().join("vendor")), vendor);
    assert_eq!(std::fs::read(dir.path().join("ridl.lock")).unwrap(), lock);
    assert_eq!(
        inventory(&dir.path().join(".ridl/baseline.previous.1")),
        prior
    );
    assert_eq!(
        std::fs::read(&snapshot).unwrap(),
        prior[Path::new("app.ir.json")]
    );
    assert_eq!(context(&baseline), old_context);
    assert_usable(&dir, &home, &baseline);
}

#[test]
fn recovery_flags_are_exclusive_and_preserve_every_artifact() {
    let dir = TempDir::new("exclusive");
    let home = TempDir::new("exclusive-home");
    let old = stub(OLD);
    let baseline = setup(&dir, &home, old.url());
    let before = inventory(&baseline);
    command(
        dir.path(),
        home.path(),
        &["baseline", "--repair-context", "--reset-history"],
        2,
    );
    assert_eq!(inventory(&baseline), before);
}

#[test]
fn malformed_context_refuses_repair_but_reset_preserves_and_replaces_it() {
    for malformed in ["not json", "{}"] {
        let dir = TempDir::new("bad-context");
        let home = TempDir::new("bad-context-home");
        let old = stub(OLD);
        let baseline = setup(&dir, &home, old.url());
        std::fs::write(baseline.join("context.json"), malformed).unwrap();
        std::fs::write(baseline.join("app.catalogs"), "invalid history").unwrap();
        let before = inventory(&baseline);
        let stderr = command(
            dir.path(),
            home.path(),
            &["baseline", "--repair-context"],
            2,
        );
        assert!(stderr.contains("context"), "{stderr}");
        assert_eq!(inventory(&baseline), before);
        command(dir.path(), home.path(), &["baseline", "--reset-history"], 0);
        assert_eq!(
            inventory(&dir.path().join(".ridl/baseline.previous.1")),
            before
        );
        context(&baseline);
        assert_usable(&dir, &home, &baseline);
    }
}

#[test]
fn reset_does_not_bypass_corrupt_local_snapshots_or_publication_guards() {
    for (source, lock, snapshot, expected, diagnostic) in [
        (LOCAL, None, Some("not json"), 2, "snapshot"),
        (
            "package app\ntype Invalid: integer [10..1]\n",
            None,
            None,
            1,
            "TYPL-104",
        ),
        (LOCAL, Some("next 2\n"), None, 1, "RIDL-411"),
        (
            "package app\ninterface Probe {}\n",
            None,
            None,
            1,
            "RIDL-408",
        ),
        (
            "package app\ntype Value: integer [0..10]\n",
            Some("next 2\n"),
            None,
            1,
            "RIDL-412",
        ),
    ] {
        let dir = TempDir::new("reset-guard");
        let home = TempDir::new("reset-guard-home");
        let old = stub(OLD);
        let baseline = setup(&dir, &home, old.url());
        std::fs::remove_file(baseline.join("context.json")).unwrap();
        dir.write("app.ridl", source);
        if let Some(lock) = lock {
            dir.write("interfaces.lock", lock);
        }
        if let Some(snapshot) = snapshot {
            std::fs::write(baseline.join("app.ir.json"), snapshot).unwrap();
        }
        let before = inventory(&baseline);
        let stderr = command(
            dir.path(),
            home.path(),
            &["baseline", "--reset-history"],
            expected,
        );
        assert!(stderr.contains(diagnostic), "{stderr}");
        assert_eq!(inventory(&baseline), before);
        assert!(!dir.path().join(".ridl/baseline.previous.1").exists());
    }
}

#[test]
fn repair_requires_the_complete_local_inventory() {
    for change in ["extra-source", "missing-snapshot", "extra-snapshot"] {
        let dir = TempDir::new("inventory");
        let home = TempDir::new("inventory-home");
        let old = stub(OLD);
        let baseline = setup(&dir, &home, old.url());
        std::fs::remove_file(baseline.join("context.json")).unwrap();
        match change {
            "extra-source" => {
                dir.write(
                    "extra/extra.typl",
                    "package app.extra\ntype Extra: integer [0..10]\n",
                );
            }
            "missing-snapshot" => {
                std::fs::remove_file(baseline.join("app.ir.json")).unwrap();
            }
            _ => {
                let mut package = ridl_ir::v2::from_json(
                    &std::fs::read_to_string(baseline.join("app.ir.json")).unwrap(),
                )
                .unwrap();
                package.name = "app.extra".into();
                std::fs::write(
                    baseline.join("app.extra.ir.json"),
                    ridl_ir::v2::to_json_pretty(&package).unwrap(),
                )
                .unwrap();
            }
        }
        let before = inventory(&baseline);
        let stderr = command(
            dir.path(),
            home.path(),
            &["baseline", "--repair-context"],
            2,
        );
        assert!(
            stderr.contains("historical") && stderr.contains("snapshot"),
            "{stderr}"
        );
        assert_eq!(inventory(&baseline), before);
    }
}

#[test]
fn repair_cannot_fetch_or_repin_a_changed_manifest() {
    let dir = TempDir::new("frozen");
    let home = TempDir::new("frozen-home");
    let old = stub(OLD);
    let baseline = setup(&dir, &home, old.url());
    std::fs::remove_file(baseline.join("context.json")).unwrap();
    let new = stub(GROWN);
    manifest(&dir, new.url());
    let before = inventory(dir.path());
    let cache = inventory(home.path());
    command(
        dir.path(),
        home.path(),
        &["baseline", "--repair-context"],
        1,
    );
    assert_eq!(new.hits(), 0);
    assert_eq!(inventory(dir.path()), before);
    assert_eq!(inventory(home.path()), cache);
}

#[test]
fn enriched_repair_refuses_different_verified_remote_pins() {
    let dir = TempDir::new("enriched-pin");
    let home = TempDir::new("enriched-pin-home");
    let old = stub(OLD);
    let baseline = setup(&dir, &home, old.url());
    let before = inventory(&baseline);
    let new = stub(GROWN);
    manifest(&dir, new.url());
    command(dir.path(), home.path(), &["check"], 0);
    let stderr = command(
        dir.path(),
        home.path(),
        &["baseline", "--repair-context"],
        2,
    );
    assert!(stderr.contains("frozen pins"), "{stderr}");
    assert_eq!(inventory(&baseline), before);
}

#[test]
fn recovery_skips_occupied_backup_names_and_preserves_unrelated_output() {
    let dir = TempDir::new("backup-names");
    let home = TempDir::new("backup-names-home");
    let old = stub(OLD);
    let baseline = setup(&dir, &home, old.url());
    let before = inventory(&baseline);
    dir.write(".ridl/baseline.previous.1/marker", "existing directory");
    dir.write(".ridl/baseline.previous.2", "existing file");
    #[cfg(unix)]
    std::os::unix::fs::symlink(
        "baseline.previous.1",
        dir.path().join(".ridl/baseline.previous.3"),
    )
    .unwrap();
    dir.write(".ridl/baseline/unrelated/marker", "unrelated bytes");
    let stderr = command(dir.path(), home.path(), &["baseline", "--reset-history"], 0);
    let index = if cfg!(unix) { 4 } else { 3 };
    let backup = dir.path().join(format!(".ridl/baseline.previous.{index}"));
    assert!(stderr.contains(backup.to_str().unwrap()), "{stderr}");
    assert_eq!(inventory(&backup), before);
    assert_eq!(
        std::fs::read_to_string(baseline.join("unrelated/marker")).unwrap(),
        "unrelated bytes"
    );
    assert_eq!(
        std::fs::read_to_string(dir.path().join(".ridl/baseline.previous.1/marker")).unwrap(),
        "existing directory"
    );
    assert_eq!(
        std::fs::read_to_string(dir.path().join(".ridl/baseline.previous.2")).unwrap(),
        "existing file"
    );
    #[cfg(unix)]
    assert_eq!(
        std::fs::read_link(dir.path().join(".ridl/baseline.previous.3")).unwrap(),
        Path::new("baseline.previous.1")
    );
}

#[test]
fn recovery_publication_blocker_preserves_old_bytes_and_staging() {
    for mode in ["--repair-context", "--reset-history"] {
        let dir = TempDir::new("blocked");
        let home = TempDir::new("blocked-home");
        let old = stub(OLD);
        let baseline = setup(&dir, &home, old.url());
        std::fs::remove_file(baseline.join("context.json")).unwrap();
        dir.write(".ridl/baseline/blocker.catalogs/marker", "blocker");
        let before = inventory(&baseline);
        let stderr = command(dir.path(), home.path(), &["baseline", mode], 2);
        assert!(stderr.contains("staged files remain"), "{stderr}");
        assert_eq!(inventory(&baseline), before);
        assert!(
            dir.path()
                .join(".ridl/.baseline.staging/context.json")
                .is_file()
        );
    }
}

#[test]
fn type_only_units_support_both_recovery_modes() {
    for mode in ["--repair-context", "--reset-history"] {
        let dir = TempDir::new("type-only");
        let home = TempDir::new("type-only-home");
        let old = stub(OLD);
        manifest(&dir, old.url());
        dir.write(
            "app.typl",
            "package app\nimport veh.common.Reading\nstruct Envelope { reading: Reading }\n",
        );
        command(dir.path(), home.path(), &["baseline"], 0);
        let baseline = dir.path().join(".ridl/baseline");
        std::fs::remove_file(baseline.join("context.json")).unwrap();
        let before = inventory(&baseline);
        command(dir.path(), home.path(), &["baseline", mode], 0);
        assert_eq!(
            inventory(&dir.path().join(".ridl/baseline.previous.1")),
            before
        );
        context(&baseline);
        assert!(!baseline.join("app.catalogs").exists());
        assert_usable(&dir, &home, &baseline);
    }
}

#[test]
fn compatible_publication_discards_only_its_new_backup() {
    let dir = TempDir::new("compatible-cleanup");
    let home = TempDir::new("compatible-cleanup-home");
    let old = stub(OLD);
    let baseline = setup(&dir, &home, old.url());
    command(dir.path(), home.path(), &["baseline", "--reset-history"], 0);
    let retained = inventory(&dir.path().join(".ridl/baseline.previous.1"));
    let stderr = command(dir.path(), home.path(), &["baseline"], 0);
    assert_eq!(
        inventory(&dir.path().join(".ridl/baseline.previous.1")),
        retained
    );
    assert!(!dir.path().join(".ridl/baseline.previous.2").exists());
    assert!(!stderr.contains("preserved the prior managed baseline"));
    context(&baseline);
}

#[test]
fn repair_from_historical_source_with_explicit_output_discards_uncertified_history() {
    let dir = TempDir::new("historical-source");
    let home = TempDir::new("historical-source-home");
    let old = stub(OLD);
    let baseline = setup(&dir, &home, old.url());
    let target = TempDir::new("repair-target");
    for (name, bytes) in inventory(&baseline) {
        std::fs::write(target.path().join(name), bytes).unwrap();
    }
    std::fs::remove_file(target.path().join("context.json")).unwrap();
    std::fs::write(
        target.path().join("app.catalogs"),
        "invalid uncertified chain",
    )
    .unwrap();
    let original = inventory(target.path());
    let source = inventory(dir.path());
    let args = recovery_example("--repair-context", dir.path(), target.path());
    let stderr = command(
        home.path(),
        home.path(),
        &args.iter().map(String::as_str).collect::<Vec<_>>(),
        0,
    );
    let name = target.path().file_name().unwrap().to_str().unwrap();
    let backup = target.path().with_file_name(format!("{name}.previous.1"));
    assert!(stderr.contains(backup.to_str().unwrap()));
    assert_eq!(inventory(&backup), original);
    assert_eq!(inventory(dir.path()), source);
    command(
        dir.path(),
        home.path(),
        &["build", "--emit", "catalog", "--out-dir", "built"],
        0,
    );
    let (code, stdout, stderr) = ridl(
        dir.path(),
        home.path(),
        &["describe", "built/app.catalog.binfb"],
    );
    assert_eq!(code, 0, "{stderr}");
    let descriptor: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let hash: String = descriptor["hash"]
        .as_array()
        .unwrap()
        .iter()
        .map(|byte| format!("{:02x}", byte.as_u64().unwrap()))
        .collect();
    let history = std::fs::read_to_string(target.path().join("app.catalogs")).unwrap();
    let hashes: Vec<_> = history
        .lines()
        .filter(|line| !line.starts_with('#') && !line.is_empty())
        .collect();
    assert_eq!(hashes, [hash]);
    context(target.path());
    command(
        dir.path(),
        home.path(),
        &[
            "diff",
            target.path().to_str().unwrap(),
            ".",
            "--encoding",
            "podbuf",
        ],
        0,
    );
    std::fs::remove_dir_all(backup).unwrap();
}

#[test]
fn type_only_break_retains_the_old_baseline() {
    let dir = TempDir::new("type-break");
    let home = TempDir::new("type-break-home");
    let old = stub(OLD);
    manifest(&dir, old.url());
    dir.write(
        "app.typl",
        "package app\nimport veh.common.Reading\nstruct Envelope { reading: Reading }\n",
    );
    command(dir.path(), home.path(), &["baseline"], 0);
    let before = inventory(&dir.path().join(".ridl/baseline"));
    let new = stub(GROWN);
    manifest(&dir, new.url());
    command(dir.path(), home.path(), &["baseline"], 0);
    assert_eq!(
        inventory(&dir.path().join(".ridl/baseline.previous.1")),
        before
    );
}

#[test]
fn repair_compares_remote_packages_canonically_independent_of_artifact_order() {
    let dir = TempDir::new("reordered-context");
    let home = TempDir::new("reordered-context-home");
    let old = Stub::serving(unit_tar(&[
        (
            "ridl.toml",
            "[package]\nname = \"veh.common\"\nversion = \"1.0.0\"\n",
        ),
        (
            "common.typl",
            "package veh.common\nimport veh.common.extra.Level\nstruct Reading { level: Level }\n",
        ),
        (
            "extra/extra.typl",
            "package veh.common.extra\ntype Level: integer [0..100]\n",
        ),
    ]));
    let baseline = setup(&dir, &home, old.url());
    let expected = context(&baseline);
    let path = baseline.join("context.json");
    let mut value: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    value["remote_units"][0]["packages"]
        .as_array_mut()
        .unwrap()
        .reverse();
    std::fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
    context(&baseline);
    command(
        dir.path(),
        home.path(),
        &["baseline", "--repair-context"],
        0,
    );
    assert_eq!(context(&baseline), expected);
}

fn local_workspace(dir: &TempDir) {
    dir.write(
        "ridl.toml",
        "[package]\nname = \"app\"\nversion = \"1.0.0\"\n",
    );
    dir.write("app.typl", "package app\ntype Value: integer [0..10]\n");
}

fn system_baseline(replace: bool) {
    let dir = TempDir::new("system-baseline");
    let home = TempDir::new("system-baseline-home");
    for (name, contents) in [
        (
            "ridl.toml",
            include_str!("../../../examples/cabin/ridl.toml"),
        ),
        (
            "cabin.ridl",
            include_str!("../../../examples/cabin/cabin.ridl"),
        ),
        (
            "system.rsdl",
            include_str!("../../../examples/cabin/system.rsdl"),
        ),
    ] {
        dir.write(name, contents);
    }
    command(dir.path(), home.path(), &["lock"], 0);
    command(dir.path(), home.path(), &["baseline"], 0);
    if replace {
        command(dir.path(), home.path(), &["baseline"], 0);
    }
    let files: Vec<_> = inventory(&dir.path().join(".ridl")).into_keys().collect();
    assert!(
        files
            .iter()
            .all(|path| !path.to_string_lossy().ends_with(".system.json")),
        "baseline publication must exclude system dumps: {files:?}"
    );
    assert!(
        std::fs::read_dir(dir.path().join(".ridl"))
            .unwrap()
            .all(|entry| !entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with(".baseline.staging")),
        "successful publication must remove staging directories"
    );
}

#[test]
fn first_system_baseline_excludes_system_dump_and_staging() {
    system_baseline(false);
}

#[test]
fn replacement_system_baseline_excludes_system_dump_and_staging() {
    system_baseline(true);
}

#[test]
fn interrupted_publication_refuses_before_staging_and_names_recovery() {
    let dir = TempDir::new("interrupted-publication");
    let home = TempDir::new("interrupted-publication-home");
    local_workspace(&dir);
    command(dir.path(), home.path(), &["baseline"], 0);
    let backup = dir.path().join(".ridl/baseline.previous.1");
    std::fs::create_dir(&backup).unwrap();
    dir.write(".ridl/.baseline.transaction", backup.to_str().unwrap());
    let prior = inventory(&dir.path().join(".ridl"));
    for flags in [
        vec!["baseline"],
        vec!["baseline", "--reset-history"],
        vec!["baseline", "--repair-context"],
    ] {
        let stderr = command(dir.path(), home.path(), &flags, 2);
        assert!(stderr.contains(backup.to_str().unwrap()), "{stderr}");
        assert!(
            stderr.contains("restore") && stderr.contains("remove") && stderr.contains("inspect"),
            "{stderr}"
        );
        assert_eq!(inventory(&dir.path().join(".ridl")), prior);
    }
    std::fs::remove_file(dir.path().join(".ridl/.baseline.transaction")).unwrap();
    command(dir.path(), home.path(), &["baseline"], 0);
    assert!(
        backup.exists(),
        "a backup without a marker permits publication"
    );
}

// Removing the directory guard accepts interrupted files; checking after context
// loading reports a context error instead of the publication recovery message.
#[test]
fn interrupted_publication_refuses_diff_against_the_baseline() {
    let dir = TempDir::new("interrupted-diff");
    let home = TempDir::new("interrupted-diff-home");
    local_workspace(&dir);
    command(dir.path(), home.path(), &["baseline"], 0);
    command(dir.path(), home.path(), &["diff", ".ridl/baseline", "."], 0);
    let backup = dir.path().join(".ridl/baseline.previous.1");
    std::fs::create_dir(&backup).unwrap();
    let marker = dir.write(".ridl/.baseline.transaction", backup.to_str().unwrap());
    for broken_context in [false, true] {
        if broken_context {
            dir.write(".ridl/baseline/context.json", "{broken");
        }
        let (code, stdout, stderr) =
            ridl(dir.path(), home.path(), &["diff", ".ridl/baseline", "."]);
        assert_eq!(code, 2, "{stderr}");
        assert!(stdout.is_empty(), "{stdout}");
        for expected in [
            "interrupted baseline publication",
            backup.to_str().unwrap(),
            marker.to_str().unwrap(),
        ] {
            assert!(stderr.contains(expected), "missing {expected}: {stderr}");
        }
        assert!(!stderr.contains("invalid baseline context"), "{stderr}");
    }
}

// Removing the guard accepts interrupted files; checking after snapshot loading
// reports a parse error instead of the publication recovery message.
#[test]
fn interrupted_publication_refuses_the_desk_check() {
    let dir = TempDir::new("interrupted-check");
    let home = TempDir::new("interrupted-check-home");
    local_workspace(&dir);
    command(dir.path(), home.path(), &["baseline"], 0);
    command(dir.path(), home.path(), &["check"], 0);
    let backup = dir.path().join(".ridl/baseline.previous.1");
    std::fs::create_dir(&backup).unwrap();
    let marker = dir.write(".ridl/.baseline.transaction", backup.to_str().unwrap());
    for broken_snapshot in [false, true] {
        if broken_snapshot {
            dir.write(".ridl/baseline/app.ir.json", "{broken");
        }
        for args in [vec!["check"], vec!["check", "--baseline", ".ridl/baseline"]] {
            let stderr = command(dir.path(), home.path(), &args, 2);
            for expected in [
                "interrupted baseline publication",
                backup.to_str().unwrap(),
                marker.to_str().unwrap(),
            ] {
                assert!(stderr.contains(expected), "missing {expected}: {stderr}");
            }
            assert!(!stderr.contains("cannot parse"), "{stderr}");
        }
    }
}

// Removing the shared directory guard permits catalog generation from an
// interrupted baseline that still has catalog history and snapshots.
#[test]
fn interrupted_publication_refuses_the_catalog_build() {
    let dir = TempDir::new("interrupted-build");
    let home = TempDir::new("interrupted-build-home");
    local_workspace(&dir);
    std::fs::remove_file(dir.path().join("app.typl")).unwrap();
    dir.write(
        "app.ridl",
        "package app\ntype Value: integer [0..10]\ninterface Probe { signal level: Value @10ms }\n",
    );
    command(dir.path(), home.path(), &["lock"], 0);
    command(dir.path(), home.path(), &["baseline"], 0);
    assert!(
        std::fs::read_dir(dir.path().join(".ridl/baseline"))
            .unwrap()
            .any(|entry| entry
                .unwrap()
                .path()
                .extension()
                .is_some_and(|ext| ext == "catalogs"))
    );
    let args = ["build", "--emit", "catalog", "--out-dir", "built"];
    command(dir.path(), home.path(), &args, 0);
    let backup = dir.path().join(".ridl/baseline.previous.1");
    std::fs::create_dir(&backup).unwrap();
    let marker = dir.write(".ridl/.baseline.transaction", backup.to_str().unwrap());
    let stderr = command(dir.path(), home.path(), &args, 2);
    for expected in [
        "interrupted baseline publication",
        backup.to_str().unwrap(),
        marker.to_str().unwrap(),
    ] {
        assert!(stderr.contains(expected), "missing {expected}: {stderr}");
    }
}
