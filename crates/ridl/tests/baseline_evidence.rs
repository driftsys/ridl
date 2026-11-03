//! Missing historical evidence and independent repair identity checks.
mod remote_support;

use remote_support::{TempDir, ridl, unit_tar};
use ridl_core::lock::{LockEntry, Lockfile, write_lockfile};
use ridlc::baseline_context::{BaselineContext, write_context};
use std::collections::BTreeMap;
use std::path::Path;

const URL: &str = "https://example.test/veh.common.tar";
const CLUSTER: &str = "package veh.cluster
import veh.common.Reading
struct Envelope { reading: Reading }
interface Probe { signal reading: Envelope @10ms }
";
const BODY: &str = "package veh.body
import veh.cluster.Envelope
interface Probe { signal reading: Envelope @10ms }
";

fn command(root: &Path, home: &Path, args: &[&str], expected: i32) -> String {
    let (code, _, stderr) = ridl(root, home, args);
    assert_eq!(code, expected, "{args:?}: {stderr}");
    stderr
}

fn setup(dir: &TempDir, home: &TempDir, podbuf: bool) {
    let cache = ridl_core::cache::Cache {
        root: home.path().join(".ridl/cache"),
    };
    let (sha256, unpacked) = cache
        .store(
            URL,
            &unit_tar(&[
                ("ridl.toml", "[package]\nname = \"veh.common\"\n"),
                (
                    "common.typl",
                    "package veh.common\ntype Word: integer [0..9223372036854775807]\nstruct Reading { value: Word }\n",
                ),
            ]),
        )
        .unwrap();
    write_lockfile(
        &dir.path().join("ridl.lock"),
        &Lockfile {
            entries: BTreeMap::from([(
                URL.into(),
                LockEntry {
                    sha256,
                    tree: Some(ridl_core::tree_hash(&unpacked).unwrap()),
                },
            )]),
        },
    )
    .unwrap();
    let backend = if podbuf {
        "[backend.rust]\nencodings = [\"podbuf\"]\n"
    } else {
        "[backend.rust]\nencodings = [\"flatbuffers\"]\n"
    };
    dir.write(
        "ridl.toml",
        &format!(
            "[workspace]\nmembers = [\"cluster\", \"body\"]\n[imports]\n\"veh.common\" = \"{URL}\"\n{backend}"
        ),
    );
    for unit in ["cluster", "body"] {
        dir.write(
            &format!("{unit}/ridl.toml"),
            &format!("[package]\nname = \"veh.{unit}\"\n"),
        );
    }
    dir.write("cluster/cluster.ridl", CLUSTER);
    dir.write("body/body.ridl", BODY);
    command(dir.path(), home.path(), &["lock"], 0);
    command(dir.path(), home.path(), &["baseline"], 0);
}

fn inventory(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    std::fs::read_dir(dir)
        .unwrap()
        .map(|entry| {
            let path = entry.unwrap().path();
            (
                path.file_name().unwrap().to_str().unwrap().to_owned(),
                std::fs::read(path).unwrap(),
            )
        })
        .collect()
}

fn compatible(out: &Path, home: &Path, unit: &str) -> Vec<serde_json::Value> {
    let file = out.join(format!("{unit}.catalog.binfb"));
    let (code, stdout, stderr) = ridl(out, home, &["describe", file.to_str().unwrap()]);
    assert_eq!(code, 0, "{stderr}");
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    json["compatible"].as_array().unwrap().clone()
}

#[test]
fn non_podbuf_baseline_accepts_missing_remote_evidence() {
    let dir = TempDir::new("non-podbuf-evidence");
    let home = TempDir::new("non-podbuf-evidence-home");
    setup(&dir, &home, false);
    let baseline = dir.path().join(".ridl/baseline");
    std::fs::remove_file(baseline.join("context.json")).unwrap();
    command(dir.path(), home.path(), &["baseline"], 0);
    assert!(baseline.join("context.json").is_file());
}

#[test]
fn missing_evidence_affects_a_unit_reaching_another_units_declaration() {
    let dir = TempDir::new("reaching-evidence");
    let home = TempDir::new("reaching-evidence-home");
    let out = TempDir::new("reaching-evidence-out");
    setup(&dir, &home, true);
    // Mutation: omit the reachable-declaration branch of Judgment::unknown.
    let baseline = dir.path().join(".ridl/baseline");
    let earlier = ridl_core::catalog_history::parse(
        &std::fs::read_to_string(baseline.join("veh.body.catalogs")).unwrap(),
    )
    .unwrap()
    .hashes[0];
    dir.write(
        "body/body.ridl",
        &BODY.replace("@10ms }", "@10ms\n signal extra: Envelope @20ms }"),
    );
    command(dir.path(), home.path(), &["lock"], 0);
    command(dir.path(), home.path(), &["baseline"], 0);
    let history = ridl_core::catalog_history::parse(
        &std::fs::read_to_string(baseline.join("veh.body.catalogs")).unwrap(),
    )
    .unwrap();
    assert_eq!(history.hashes.len(), 2);
    assert_eq!(history.hashes[1], earlier);
    let earlier = serde_json::json!(earlier);
    command(
        dir.path(),
        home.path(),
        &[
            "build",
            "--emit",
            "catalog",
            "--out-dir",
            out.path().to_str().unwrap(),
        ],
        0,
    );
    assert!(
        compatible(out.path(), home.path(), "veh.body").contains(&earlier),
        "the earlier hash is carried while evidence is present"
    );
    std::fs::remove_file(dir.path().join(".ridl/baseline/context.json")).unwrap();
    // A legacy local-only latest hash lets reachability alone decide whether to carry the earlier hash.
    let old =
        ridlc::load_diff_side(&mut ridl_core::RidlDatabase::default(), &baseline, &[]).unwrap();
    let std = ridlc::std_ir();
    let local: Vec<_> = old.packages.iter().collect();
    let scope = ridlc::catalog_scope(&local, Some(&std));
    let hash = ridl_ir::catalog_hash::catalog_hash("veh.body", &scope);
    let hash: String = hash.iter().map(|byte| format!("{byte:02x}")).collect();
    let previous = std::fs::read_to_string(baseline.join("veh.body.catalogs")).unwrap();
    let (_, earlier_history) = previous.split_once('\n').unwrap();
    std::fs::write(
        baseline.join("veh.body.catalogs"),
        format!("{hash}\n{earlier_history}"),
    )
    .unwrap();

    let stderr = command(
        dir.path(),
        home.path(),
        &[
            "build",
            "--emit",
            "catalog",
            "--out-dir",
            out.path().to_str().unwrap(),
        ],
        0,
    );
    for unit in ["veh.body", "veh.cluster"] {
        let carried = compatible(out.path(), home.path(), unit);
        if unit == "veh.body" {
            assert!(
                !carried.contains(&earlier),
                "missing evidence must end the reaching unit's earlier history: {carried:?}"
            );
        }
        assert!(carried.is_empty());
        assert!(
            stderr.contains(&format!(
                "cannot establish podbuf compatibility for unit `{unit}`"
            )),
            "the reaching unit and the owner both lack evidence: {stderr}"
        );
    }
}

#[test]
fn legacy_local_only_hash_does_not_carry_history_without_evidence() {
    let dir = TempDir::new("legacy-local-hash");
    let home = TempDir::new("legacy-local-hash-home");
    let out = TempDir::new("legacy-local-hash-out");
    setup(&dir, &home, true);
    let baseline = dir.path().join(".ridl/baseline");
    std::fs::remove_file(baseline.join("context.json")).unwrap();
    let old =
        ridlc::load_diff_side(&mut ridl_core::RidlDatabase::default(), &baseline, &[]).unwrap();
    let std = ridlc::std_ir();
    let local: Vec<_> = old.packages.iter().collect();
    let scope = ridlc::catalog_scope(&local, Some(&std));
    let hash = ridl_ir::catalog_hash::catalog_hash("veh.cluster", &scope);
    let hash: String = hash.iter().map(|byte| format!("{byte:02x}")).collect();
    // An older tool could publish this hash without the remote declarations.
    std::fs::write(baseline.join("veh.cluster.catalogs"), format!("{hash}\n")).unwrap();
    command(
        dir.path(),
        home.path(),
        &[
            "build",
            "--emit",
            "catalog",
            "--out-dir",
            out.path().to_str().unwrap(),
        ],
        0,
    );
    assert!(
        compatible(out.path(), home.path(), "veh.cluster").is_empty(),
        "missing evidence prevents carrying even a matching local-only history"
    );
}

fn repair_refuses_changed_context(label: &str, change: impl FnOnce(&mut BaselineContext)) {
    let dir = TempDir::new(label);
    let home = TempDir::new(&format!("{label}-home"));
    setup(&dir, &home, true);
    let baseline = dir.path().join(".ridl/baseline");
    let side =
        ridlc::load_diff_side(&mut ridl_core::RidlDatabase::default(), &baseline, &[]).unwrap();
    let original = ridlc::baseline_context::read_context(&baseline, &side.packages)
        .unwrap()
        .unwrap();
    let mut historical = original.clone();
    change(&mut historical);
    let before = original.remote_context();
    let after = historical.remote_context();
    assert_eq!(before.pins == after.pins, label == "repair-packages-only");
    assert_eq!(
        before.packages == after.packages,
        label == "repair-pins-only"
    );
    write_context(&baseline, &historical).unwrap();
    let prior = inventory(&baseline);
    let stderr = command(
        dir.path(),
        home.path(),
        &["baseline", "--repair-context"],
        2,
    );
    assert!(
        stderr.contains("historical context does not match SOURCE and its frozen pins"),
        "{stderr}"
    );
    assert_eq!(inventory(&baseline), prior);
}

#[test]
fn repair_refuses_pins_mismatch_with_identical_packages() {
    repair_refuses_changed_context("repair-pins-only", |context| {
        context.remote_units[0].url = "https://example.test/other.tar".into();
    });
}

#[test]
fn repair_refuses_packages_mismatch_with_identical_pins() {
    repair_refuses_changed_context("repair-packages-only", |context| {
        context.remote_units[0].packages[0].decls[0].doc = "Historical documentation.".into();
    });
}

#[test]
fn baseline_new_side_issue_names_current_evidence_without_historical_repair() {
    // Mutation: use the historical-context refusal and repair advice for every side.
    let dir = TempDir::new("current-evidence");
    let home = TempDir::new("current-evidence-home");
    setup(&dir, &home, true);
    let baseline = dir.path().join(".ridl/baseline");
    let before = inventory(&baseline);
    let cache = ridl_core::cache::Cache {
        root: home.path().join(".ridl/cache"),
    };
    let (sha256, unpacked) = cache
        .store(
            URL,
            &unit_tar(&[
                ("ridl.toml", "[package]\nname = \"veh.common\"\n"),
                (
                    "common.typl",
                    "package veh.common\nenum Reading { OFF = 0 }\n",
                ),
            ]),
        )
        .unwrap();
    write_lockfile(
        &dir.path().join("ridl.lock"),
        &Lockfile {
            entries: BTreeMap::from([(
                URL.into(),
                LockEntry {
                    sha256,
                    tree: Some(ridl_core::tree_hash(&unpacked).unwrap()),
                },
            )]),
        },
    )
    .unwrap();
    let stderr = command(dir.path(), home.path(), &["baseline"], 2);
    assert!(
        stderr.contains("current workspace's remote evidence is missing or unusable"),
        "{stderr}"
    );
    assert!(!stderr.contains("historical inline context"), "{stderr}");
    assert!(!stderr.contains("--repair-context"), "{stderr}");
    assert_eq!(inventory(&baseline), before);
}
