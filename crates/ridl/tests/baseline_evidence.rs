//! Missing remote evidence and independent repair identity checks.
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

fn setup(dir: &TempDir, home: &TempDir, encoding: &str) {
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
    let backend = format!("[backend.rust]\nencodings = [{encoding}]\n");
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

fn catalog_hash(out: &Path, home: &Path, unit: &str) -> serde_json::Value {
    let file = out.join(format!("{unit}.catalog.binfb"));
    let (code, stdout, stderr) = ridl(out, home, &["describe", file.to_str().unwrap()]);
    assert_eq!(code, 0, "{stderr}");
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    json["hash"].clone()
}

/// History heads equal descriptor hashes for direct and transitive remote type
/// references, and a compatible publication preserves the prior head.
#[test]
fn a_history_head_equals_the_descriptor_hash_of_a_unit_reaching_a_remote_type() {
    let dir = TempDir::new("remote-history-hash");
    let home = TempDir::new("remote-history-hash-home");
    let out = TempDir::new("remote-history-hash-out");
    let out2 = TempDir::new("remote-history-hash-out2");
    setup(&dir, &home, "\"podbuf\"");
    let baseline = dir.path().join(".ridl/baseline");
    let heads = ["veh.cluster", "veh.body"].map(|unit| {
        let history = ridl_core::catalog_history::parse(
            &std::fs::read_to_string(baseline.join(format!("{unit}.catalogs"))).unwrap(),
        )
        .unwrap();
        (unit, history.hashes[0])
    });
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
    for (unit, head) in &heads {
        assert_eq!(
            catalog_hash(out.path(), home.path(), unit),
            serde_json::json!(head),
            "{unit}: the history head equals the descriptor hash"
        );
        assert!(
            compatible(out.path(), home.path(), unit).is_empty(),
            "{unit}: the published tree lists no earlier catalog"
        );
    }

    let old_cluster_head = serde_json::json!(heads[0].1);
    dir.write(
        "cluster/cluster.ridl",
        &CLUSTER.replace("@10ms }", "@10ms\n signal extra: Envelope @20ms }"),
    );
    command(dir.path(), home.path(), &["lock"], 0);
    command(
        dir.path(),
        home.path(),
        &[
            "build",
            "--emit",
            "catalog",
            "--out-dir",
            out2.path().to_str().unwrap(),
        ],
        0,
    );
    assert_eq!(
        compatible(out2.path(), home.path(), "veh.cluster"),
        vec![old_cluster_head.clone()],
        "the changed tree lists the published hash as compatible"
    );
    let new = catalog_hash(out2.path(), home.path(), "veh.cluster");
    assert_ne!(new, old_cluster_head);

    command(dir.path(), home.path(), &["baseline"], 0);
    let history = ridl_core::catalog_history::parse(
        &std::fs::read_to_string(baseline.join("veh.cluster.catalogs")).unwrap(),
    )
    .unwrap();
    assert_eq!(
        serde_json::json!(history.hashes),
        serde_json::json!([new, old_cluster_head]),
        "publication prepends the new descriptor hash to the published history"
    );
}

#[test]
fn no_encoding_baseline_accepts_missing_remote_evidence() {
    let dir = TempDir::new("non-podbuf-evidence");
    let home = TempDir::new("non-podbuf-evidence-home");
    setup(&dir, &home, "");
    let baseline = dir.path().join(".ridl/baseline");
    std::fs::remove_file(baseline.join("context.json")).unwrap();
    command(dir.path(), home.path(), &["baseline"], 0);
    assert!(baseline.join("context.json").is_file());
}

fn append_body_signal(dir: &TempDir) {
    dir.write(
        "body/body.ridl",
        &BODY.replace("@10ms }", "@10ms\n signal extra: Envelope @20ms }"),
    );
}

fn missing_evidence_refuses_publication(encoding: &str) {
    let dir = TempDir::new(&format!("{encoding}-missing-publication"));
    let home = TempDir::new(&format!("{encoding}-missing-publication-home"));
    setup(&dir, &home, &format!("\"{encoding}\""));
    let baseline = dir.path().join(".ridl/baseline");
    std::fs::remove_file(baseline.join("context.json")).unwrap();
    let before = inventory(&baseline);
    let stderr = command(dir.path(), home.path(), &["baseline"], 2);
    assert!(
        stderr.contains("cannot publish unit `veh.body`"),
        "{stderr}"
    );
    assert!(stderr.contains("historical remote context"), "{stderr}");
    assert!(stderr.contains("--repair-context"), "{stderr}");
    assert!(stderr.contains("--reset-history"), "{stderr}");
    assert_eq!(inventory(&baseline), before);
}

#[test]
fn flatbuffers_baseline_refuses_missing_remote_evidence() {
    missing_evidence_refuses_publication("flatbuffers");
}

#[test]
fn proto3_baseline_refuses_missing_remote_evidence() {
    missing_evidence_refuses_publication("proto3");
}

fn missing_evidence_omits_compatible_catalogs(encoding: &str) {
    let dir = TempDir::new(&format!("{encoding}-missing-build"));
    let home = TempDir::new(&format!("{encoding}-missing-build-home"));
    let out = TempDir::new(&format!("{encoding}-missing-build-out"));
    setup(&dir, &home, &format!("\"{encoding}\""));
    append_body_signal(&dir);
    command(dir.path(), home.path(), &["lock"], 0);
    command(dir.path(), home.path(), &["baseline"], 0);
    let baseline = dir.path().join(".ridl/baseline");
    std::fs::remove_file(baseline.join("context.json")).unwrap();
    // A legacy local-only head would carry the earlier catalog without an evidence check.
    let old =
        ridlc::load_diff_side(&mut ridl_core::RidlDatabase::default(), &baseline, &[]).unwrap();
    let std = ridlc::std_ir();
    let local: Vec<_> = old.packages.iter().collect();
    let scope = ridlc::catalog_scope(&local, Some(&std));
    let hash = ridl_ir::catalog_hash::catalog_hash("veh.body", &scope);
    let hash: String = hash.iter().map(|byte| format!("{byte:02x}")).collect();
    let previous = std::fs::read_to_string(baseline.join("veh.body.catalogs")).unwrap();
    let (_, earlier) = previous.split_once('\n').unwrap();
    assert!(!earlier.is_empty());
    std::fs::write(
        baseline.join("veh.body.catalogs"),
        format!("{hash}\n{earlier}"),
    )
    .unwrap();
    let before = inventory(&baseline);
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
        assert!(
            compatible(out.path(), home.path(), unit).is_empty(),
            "{unit}"
        );
        assert!(
            stderr.contains(&format!("cannot establish compatibility for unit `{unit}`")),
            "{stderr}"
        );
    }
    assert!(stderr.contains("historical remote context"), "{stderr}");
    assert_eq!(inventory(&baseline), before);
}

#[test]
fn flatbuffers_build_omits_catalogs_with_missing_remote_evidence() {
    missing_evidence_omits_compatible_catalogs("flatbuffers");
}

#[test]
fn proto3_build_omits_catalogs_with_missing_remote_evidence() {
    missing_evidence_omits_compatible_catalogs("proto3");
}

fn flatbuffers_carries_history(remote: bool) {
    let dir = TempDir::new(if remote { "known-remote" } else { "no-remote" });
    let home = TempDir::new(if remote {
        "known-remote-home"
    } else {
        "no-remote-home"
    });
    let out = TempDir::new(if remote {
        "known-remote-out"
    } else {
        "no-remote-out"
    });
    setup(&dir, &home, "\"flatbuffers\"");
    if !remote {
        dir.write(
            "cluster/cluster.ridl",
            "package veh.cluster\nstruct Envelope { reading: integer }\ninterface Probe { signal reading: Envelope @10ms }\n",
        );
        command(dir.path(), home.path(), &["lock"], 0);
        command(dir.path(), home.path(), &["baseline", "--reset-history"], 0);
    }
    let baseline = dir.path().join(".ridl/baseline");
    let prior = std::fs::read_to_string(baseline.join("veh.body.catalogs")).unwrap();
    append_body_signal(&dir);
    command(dir.path(), home.path(), &["lock"], 0);
    command(dir.path(), home.path(), &["baseline"], 0);
    let published = std::fs::read_to_string(baseline.join("veh.body.catalogs")).unwrap();
    let prior_hashes = ridl_core::catalog_history::parse(&prior).unwrap().hashes;
    let published_hashes = ridl_core::catalog_history::parse(&published)
        .unwrap()
        .hashes;
    assert_eq!(published_hashes[1..], prior_hashes);
    if !remote {
        std::fs::remove_file(baseline.join("context.json")).unwrap();
    }
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
    assert!(!stderr.contains("cannot establish"), "{stderr}");
    let earlier = ridl_core::catalog_history::parse(&prior).unwrap().hashes[0];
    assert_eq!(
        compatible(out.path(), home.path(), "veh.body"),
        vec![serde_json::json!(earlier)]
    );
    command(dir.path(), home.path(), &["baseline"], 0);
    assert_eq!(
        std::fs::read_to_string(baseline.join("veh.body.catalogs")).unwrap(),
        published
    );
}

#[test]
fn flatbuffers_without_remote_reach_carries_history_without_context() {
    flatbuffers_carries_history(false);
}

#[test]
fn flatbuffers_with_remote_context_carries_compatible_history() {
    flatbuffers_carries_history(true);
}

// The new Reading changes kind but retains Word, so no remote declaration is missing.
fn publication_with_podbuf_kind_change(encoding: &str, expected: i32) {
    let dir = TempDir::new(&format!("{encoding}-kind-change"));
    let home = TempDir::new(&format!("{encoding}-kind-change-home"));
    setup(&dir, &home, &format!("\"{encoding}\""));
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
                    "package veh.common\ntype Word: integer [0..9223372036854775807]\nenum Reading { OFF = 0 }\n",
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
    let stderr = command(dir.path(), home.path(), &["baseline"], expected);
    if expected == 0 {
        assert!(!stderr.contains("missing or unusable"), "{stderr}");
    } else {
        assert!(
            stderr.contains("current workspace's remote evidence is missing or unusable"),
            "{stderr}"
        );
        assert_eq!(inventory(&baseline), before);
    }
}

#[test]
fn flatbuffers_publication_ignores_podbuf_kind_change_issues() {
    publication_with_podbuf_kind_change("flatbuffers", 0);
}

#[test]
fn proto3_publication_ignores_podbuf_kind_change_issues() {
    publication_with_podbuf_kind_change("proto3", 0);
}

#[test]
fn podbuf_publication_refuses_kind_change_issues() {
    // Mutation: filter context issues to Missing even when podbuf is enabled.
    publication_with_podbuf_kind_change("podbuf", 2);
}

#[test]
fn flatbuffers_publication_ignores_unlayable_historical_podbuf_heads() {
    let dir = TempDir::new("flatbuffers-unlayable");
    let home = TempDir::new("flatbuffers-unlayable-home");
    setup(&dir, &home, "\"flatbuffers\"");
    let baseline = dir.path().join(".ridl/baseline");
    let old =
        ridlc::load_diff_side(&mut ridl_core::RidlDatabase::default(), &baseline, &[]).unwrap();
    let mut context = ridlc::baseline_context::read_context(&baseline, &old.packages)
        .unwrap()
        .unwrap();
    let original = context.remote_context();
    let reading = context.remote_units[0].packages[0]
        .decls
        .iter_mut()
        .find(|decl| decl.name == "Reading")
        .unwrap();
    let Some(ridl_ir::v2::decl::Kind::StructDef(def)) = &mut reading.kind else {
        panic!("Reading is a struct");
    };
    let Some(ridl_ir::v2::struct_member::Member::Field(field)) = &mut def.members[0].member else {
        panic!("Reading has a field");
    };
    field.r#type.as_mut().unwrap().kind = Some(ridl_ir::v2::field_type::Kind::Named(
        "veh.common.Reading".into(),
    ));
    write_context(&baseline, &context).unwrap();
    let old_refs: Vec<_> = old.packages.iter().collect();
    let historical = context.remote_context();
    let old_context: Vec<_> = historical.packages.iter().collect();
    let new_context: Vec<_> = original.packages.iter().collect();
    let (_, issues) = ridl_diff::diff_refs_with_contexts_partial(
        &old_refs,
        &old_refs,
        &old_context,
        &new_context,
        &["flatbuffers"],
    );
    assert!(
        issues
            .iter()
            .any(|issue| issue.reason == ridl_diff::ContextIssueReason::Unlayable)
    );
    assert!(
        !issues
            .iter()
            .any(|issue| issue.reason == ridl_diff::ContextIssueReason::Missing)
    );
    command(dir.path(), home.path(), &["baseline"], 0);
}

#[test]
fn missing_evidence_affects_a_unit_reaching_another_units_declaration() {
    let dir = TempDir::new("reaching-evidence");
    let home = TempDir::new("reaching-evidence-home");
    let out = TempDir::new("reaching-evidence-out");
    setup(&dir, &home, "\"podbuf\"");
    // Mutation: omit the reachable-declaration branch of Judgment::unknown_side.
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
            stderr.contains(&format!("cannot establish compatibility for unit `{unit}`")),
            "the reaching unit and the owner both lack evidence: {stderr}"
        );
    }
}

#[test]
fn legacy_local_only_hash_does_not_carry_history_without_evidence() {
    let dir = TempDir::new("legacy-local-hash");
    let home = TempDir::new("legacy-local-hash-home");
    let out = TempDir::new("legacy-local-hash-out");
    setup(&dir, &home, "\"podbuf\"");
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
    setup(&dir, &home, "\"podbuf\"");
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

// The historical Reading reaches Word, which the current unit removes. The
// surviving consumer use therefore has a Missing issue on the new side.
fn setup_missing_current_remote_evidence(dir: &TempDir, home: &TempDir, encoding: &str) {
    setup(dir, home, &format!("\"{encoding}\""));
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
}

fn new_side_issue_names_current_evidence_without_historical_repair(encoding: &str) {
    let dir = TempDir::new(&format!("{encoding}-current-evidence"));
    let home = TempDir::new(&format!("{encoding}-current-evidence-home"));
    setup_missing_current_remote_evidence(&dir, &home, encoding);
    let baseline = dir.path().join(".ridl/baseline");
    let before = inventory(&baseline);
    let stderr = command(dir.path(), home.path(), &["baseline"], 2);
    assert!(
        stderr.contains("current workspace's remote evidence is missing or unusable"),
        "{stderr}"
    );
    assert!(!stderr.contains("historical remote context"), "{stderr}");
    assert!(!stderr.contains("--repair-context"), "{stderr}");
    assert_eq!(inventory(&baseline), before);
}

#[test]
fn flatbuffers_build_names_missing_current_remote_evidence_without_historical_repair() {
    // Mutation: always use the historical-context warning and repair advice.
    let dir = TempDir::new("flatbuffers-current-build");
    let home = TempDir::new("flatbuffers-current-build-home");
    let out = TempDir::new("flatbuffers-current-build-out");
    setup_missing_current_remote_evidence(&dir, &home, "flatbuffers");
    let baseline = dir.path().join(".ridl/baseline");
    let before = inventory(&baseline);
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
        assert!(
            stderr.contains(&format!("warning: cannot establish compatibility for unit `{unit}`: the current workspace's remote evidence is missing or unusable")),
            "{stderr}"
        );
        assert!(compatible(out.path(), home.path(), unit).is_empty());
    }
    assert!(
        stderr.contains("Restore usable current remote evidence"),
        "{stderr}"
    );
    assert!(!stderr.contains("historical remote context"), "{stderr}");
    assert!(!stderr.contains("--repair-context"), "{stderr}");
    assert_eq!(inventory(&baseline), before);
}

#[test]
fn baseline_new_side_issue_names_current_evidence_without_historical_repair() {
    // Mutation: use the historical-context refusal and repair advice for every side.
    new_side_issue_names_current_evidence_without_historical_repair("podbuf");
}

#[test]
fn flatbuffers_baseline_refuses_missing_current_remote_evidence() {
    // Mutation: ignore New-side issues when only FlatBuffers or proto3 is enabled.
    new_side_issue_names_current_evidence_without_historical_repair("flatbuffers");
}

#[test]
fn proto3_baseline_refuses_missing_current_remote_evidence() {
    // Mutation: ignore New-side issues when only FlatBuffers or proto3 is enabled.
    new_side_issue_names_current_evidence_without_historical_repair("proto3");
}
