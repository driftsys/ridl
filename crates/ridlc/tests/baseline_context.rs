use ridl_core::{MaterializedUnit, RidlDatabase};
use ridl_ir::v2::{self, Package};
use ridlc::baseline_context::{BaselineContext, capture_context, read_context, write_context};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

fn package(name: &str, unit: &str, references: &[&str]) -> Package {
    Package {
        name: name.into(),
        unit: unit.into(),
        decls: references
            .iter()
            .enumerate()
            .map(|(i, reference)| v2::Decl {
                name: format!("VALUE_{i}"),
                kind: Some(v2::decl::Kind::ConstDef(v2::ConstDef {
                    type_ref: Some((*reference).into()),
                    ..Default::default()
                })),
                ..Default::default()
            })
            .collect(),
        ..Default::default()
    }
}
struct Fixture {
    own: Vec<Package>,
    remotes: Vec<Package>,
    units: BTreeMap<String, MaterializedUnit>,
    trees: tempfile::TempDir,
}
impl Fixture {
    fn new() -> Self {
        let trees = tempfile::tempdir().unwrap();
        let units = [("alpha", "a"), ("beta", "b"), ("other", "c")]
            .into_iter()
            .map(|(name, hash)| {
                let dir = trees.path().join(name);
                fs::create_dir(&dir).unwrap();
                (
                    name.into(),
                    MaterializedUnit {
                        url: format!("https://example.test/{name}.tar"),
                        dir,
                        tree: hash.repeat(64),
                    },
                )
            })
            .collect();
        Self {
            own: vec![
                package("app", "app", &["dep.a.Payload"]),
                package("app.extra", "app", &[]),
            ],
            // The reference in dep.a is part of a declaration unrelated to
            // Payload. Storing the whole package requires following it too.
            remotes: vec![
                package("dep.a", "alpha", &["dep.b.Payload"]),
                package("dep.b", "beta", &[]),
                package("dep.unused", "other", &[]),
            ],
            units,
            trees,
        }
    }
    fn capture(&self) -> BaselineContext {
        capture_context(
            &self.own.iter().collect::<Vec<_>>(),
            &self.remotes.iter().collect::<Vec<_>>(),
            &self.units,
        )
        .unwrap()
    }
    fn snapshots(&self, dir: &Path) {
        for local in &self.own {
            fs::write(
                dir.join(format!("{}.ir.json", local.name)),
                v2::to_json_pretty(local).unwrap(),
            )
            .unwrap();
        }
    }
}
fn load(path: &Path) -> Result<ridlc::diff_side::DiffSide, ridlc::diff_side::DiffSideError> {
    ridlc::diff_side::load_diff_side(&mut RidlDatabase::default(), path, &[])
}

// Dropping remote evidence on directory load loses both offline resolution
// and ownership. Keeping every loaded remote instead leaks unrelated packages.
#[test]
fn context_round_trip_keeps_ownership_and_pin_evidence() {
    let fixture = Fixture::new();
    let context = fixture.capture();
    assert_eq!(
        context
            .local_packages
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        ["app", "app.extra"]
    );
    for local in &fixture.own {
        let canonical = v2::to_json_pretty(local).unwrap();
        assert_eq!(
            context.local_packages[&local.name],
            format!("{:x}", Sha256::digest(canonical.as_bytes()))
        );
    }
    assert_eq!(
        context
            .remote_units
            .iter()
            .map(|u| u.unit.as_str())
            .collect::<Vec<_>>(),
        ["alpha", "beta"]
    );
    let dir = tempfile::tempdir().unwrap();
    fixture.snapshots(dir.path());
    write_context(dir.path(), &context).unwrap();
    let first = fs::read(dir.path().join("context.json")).unwrap();
    let restored = read_context(dir.path(), &fixture.own).unwrap().unwrap();
    let mut reversed = restored.clone();
    reversed.remote_units.reverse();
    write_context(dir.path(), &reversed).unwrap();
    assert_eq!(first, fs::read(dir.path().join("context.json")).unwrap());
    assert_eq!(restored, context);
    let text = String::from_utf8(first).unwrap();
    assert!(!text.contains(fixture.trees.path().to_str().unwrap()));
    let side = load(dir.path()).unwrap();
    assert_eq!(
        side.packages
            .iter()
            .map(|p| p.name.as_str())
            .collect::<Vec<_>>(),
        ["app.extra", "app"]
    );
    assert_eq!(
        side.remote_context
            .as_ref()
            .unwrap()
            .packages
            .iter()
            .map(|p| p.name.as_str())
            .collect::<Vec<_>>(),
        ["dep.a", "dep.b"]
    );
    let remote = side.remote_context.unwrap();
    assert_eq!(remote.pins["alpha"].url, "https://example.test/alpha.tar");
    assert_eq!(remote.pins["beta"].tree, "b".repeat(64));
}

#[test]
fn copied_directory_resolves_without_vendor_or_cache() {
    let fixture = Fixture::new();
    let dir = tempfile::tempdir().unwrap();
    fixture.snapshots(dir.path());
    write_context(dir.path(), &fixture.capture()).unwrap();
    let copy = tempfile::tempdir().unwrap();
    for entry in fs::read_dir(dir.path()).unwrap() {
        let entry = entry.unwrap();
        fs::copy(entry.path(), copy.path().join(entry.file_name())).unwrap();
    }
    fixture.trees.close().unwrap();
    dir.close().unwrap();
    let side = load(copy.path()).unwrap();
    let remote = side.remote_context.unwrap();
    assert_eq!(remote.pins.len(), 2);
    assert_eq!(remote.packages.len(), 2);
}

#[test]
fn absent_evidence_is_legacy_and_standalone_files_ignore_siblings() {
    let fixture = Fixture::new();
    let dir = tempfile::tempdir().unwrap();
    fixture.snapshots(dir.path());
    let side = load(dir.path()).unwrap();
    assert!(side.remote_context.is_none());
    fs::write(dir.path().join("context.json"), "malformed").unwrap();
    assert!(load(dir.path()).is_err());
    let standalone = load(&dir.path().join("app.ir.json")).unwrap();
    assert!(standalone.remote_context.is_none());
}

#[test]
fn source_without_remotes_has_complete_empty_evidence() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join("p.typl"),
        "package p\ntype Value: integer\n",
    )
    .unwrap();
    fs::write(dir.path().join("ridl.toml"), "[package]\nname = \"p\"\n").unwrap();
    let context = load(dir.path()).unwrap().remote_context.unwrap();
    assert!(context.packages.is_empty());
    assert!(context.pins.is_empty());
}

#[test]
fn capture_rejects_duplicate_colliding_or_unproven_packages() {
    let fixture = Fixture::new();
    let own: Vec<_> = fixture.own.iter().collect();
    let remote: Vec<_> = fixture.remotes.iter().collect();
    let error = capture_context(&[own[0], own[0]], &remote, &fixture.units).unwrap_err();
    assert!(error.to_string().contains("duplicate package"));
    let error = capture_context(&own, &[remote[0], remote[0]], &fixture.units).unwrap_err();
    assert!(error.to_string().contains("duplicate package"));
    let error = capture_context(&own, &[own[0]], &fixture.units).unwrap_err();
    assert!(error.to_string().contains("collision"));
    let error = capture_context(&own, &remote, &BTreeMap::new()).unwrap_err();
    assert!(error.to_string().contains("no verified owning unit"));
    let error = capture_context(&own, &[remote[0]], &fixture.units).unwrap_err();
    assert!(error.to_string().contains("dep.b"));
    let mut units = fixture.units.clone();
    units.get_mut("alpha").unwrap().tree.clear();
    assert!(capture_context(&own, &remote, &units).is_err());
    units.get_mut("alpha").unwrap().tree = "x".repeat(64);
    assert!(capture_context(&own, &remote, &units).is_err());
    units.get_mut("alpha").unwrap().tree = "a".repeat(64);
    units.get_mut("alpha").unwrap().url.clear();
    assert!(capture_context(&own, &remote, &units).is_err());
}

#[test]
fn enriched_input_refuses_invalid_inventories_and_provenance() {
    let fixture = Fixture::new();
    let valid = fixture.capture();
    let dir = tempfile::tempdir().unwrap();
    fixture.snapshots(dir.path());
    write_context(dir.path(), &valid).unwrap();
    let text = fs::read_to_string(dir.path().join("context.json")).unwrap();
    let valid: serde_json::Value = serde_json::from_str(&text).unwrap();
    let cases: Vec<(&str, serde_json::Value)> = vec![
        ("unsupported version", {
            let mut x = valid.clone();
            x["version"] = 2.into();
            x
        }),
        ("duplicate unit", {
            let mut x = valid.clone();
            let u = x["remote_units"][0].clone();
            x["remote_units"].as_array_mut().unwrap().push(u);
            x
        }),
        ("duplicate package", {
            let mut x = valid.clone();
            let p = x["remote_units"][0]["packages"][0].clone();
            x["remote_units"][0]["packages"]
                .as_array_mut()
                .unwrap()
                .push(p);
            x
        }),
        ("local/remote collision", {
            let mut x = valid.clone();
            x["remote_units"][0]["packages"][0]["name"] = "app".into();
            x
        }),
        ("outside reached closure", {
            let mut x = valid.clone();
            let mut extra = x["remote_units"][1]["packages"][0].clone();
            extra["name"] = "dep.unused".into();
            x["remote_units"][1]["packages"]
                .as_array_mut()
                .unwrap()
                .push(extra);
            x
        }),
        ("empty packages", {
            let mut x = valid.clone();
            x["remote_units"][0]["packages"] = serde_json::json!([]);
            x
        }),
        ("local in remote unit", {
            let mut x = valid.clone();
            x["remote_units"][0]["unit"] = "app".into();
            x["remote_units"][0]["packages"][0]["unit"] = "app".into();
            x
        }),
        ("wrong owning unit", {
            let mut x = valid.clone();
            x["remote_units"][0]["packages"][0]["unit"] = "wrong".into();
            x
        }),
        ("missing tree", {
            let mut x = valid.clone();
            x["remote_units"][0].as_object_mut().unwrap().remove("tree");
            x
        }),
        ("empty tree", {
            let mut x = valid.clone();
            x["remote_units"][0]["tree"] = "".into();
            x
        }),
        ("bad tree", {
            let mut x = valid.clone();
            x["remote_units"][0]["tree"] = "z".repeat(64).into();
            x
        }),
        ("absent url", {
            let mut x = valid.clone();
            x["remote_units"][0].as_object_mut().unwrap().remove("url");
            x
        }),
        ("empty url", {
            let mut x = valid.clone();
            x["remote_units"][0]["url"] = "".into();
            x
        }),
        ("missing fingerprint", {
            let mut x = valid.clone();
            x["local_packages"]
                .as_object_mut()
                .unwrap()
                .remove("app.extra");
            x
        }),
        ("extra fingerprint", {
            let mut x = valid.clone();
            x["local_packages"]["extra"] = "d".repeat(64).into();
            x
        }),
        ("stale fingerprint", {
            let mut x = valid.clone();
            x["local_packages"]["app"] = "0".repeat(64).into();
            x
        }),
        ("missing transitive context", {
            let mut x = valid.clone();
            x["remote_units"].as_array_mut().unwrap().pop();
            x
        }),
        ("malformed nested IR", {
            let mut x = valid.clone();
            x["remote_units"][0]["packages"][0]["unknownField"] = true.into();
            x
        }),
    ];
    for (name, value) in cases {
        fs::write(dir.path().join("context.json"), value.to_string()).unwrap();
        let result = load(dir.path());
        assert!(result.is_err(), "accepted {name}");
        let error = result.unwrap_err();
        let expected = match name {
            "local/remote collision" => Some("local/remote package collision `app`"),
            "outside reached closure" => {
                Some("remote context contains packages outside the reached closure")
            }
            "empty packages" => Some("missing or invalid pin provenance for `alpha`"),
            "local in remote unit" => Some("local package `app.extra` belongs to a remote unit"),
            _ => None,
        };
        if let Some(expected) = expected {
            let ridlc::diff_side::DiffSideError::Context(error) = error else {
                panic!("expected a context error for {name}: {error}");
            };
            assert_eq!(error.message, expected, "{name}");
        }
    }
    let duplicate = text.replacen(
        "\"local_packages\": {",
        &format!("\"local_packages\": {{\"app\":\"{}\",", "0".repeat(64)),
        1,
    );
    fs::write(dir.path().join("context.json"), duplicate).unwrap();
    assert!(
        load(dir.path())
            .unwrap_err()
            .to_string()
            .contains("duplicate local package")
    );
    fs::write(dir.path().join("context.json"), &text).unwrap();
    let mut changed = fixture.own[0].clone();
    changed.decls[0].doc = "changed".into();
    fs::write(
        dir.path().join("app.ir.json"),
        v2::to_json_pretty(&changed).unwrap(),
    )
    .unwrap();
    assert!(
        load(dir.path())
            .unwrap_err()
            .to_string()
            .contains("fingerprints")
    );
}

#[test]
fn nested_ir_uses_the_existing_interaction_class_guard() {
    let fixture = Fixture::new();
    let dir = tempfile::tempdir().unwrap();
    fixture.snapshots(dir.path());
    write_context(dir.path(), &fixture.capture()).unwrap();
    let mut json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(dir.path().join("context.json")).unwrap())
            .unwrap();
    json["remote_units"][0]["packages"][0]["interfaces"] = serde_json::json!([
        {"name":"Api", "interactions":[{"name":"Run", "commandDef":{"family":"FAMILY_INTENT", "shape":"SHAPE_NONE"}}]}
    ]);
    fs::write(dir.path().join("context.json"), json.to_string()).unwrap();
    assert!(
        load(dir.path())
            .unwrap_err()
            .to_string()
            .contains("invalid interaction class")
    );
}

#[test]
fn enriched_directory_with_all_local_snapshots_removed_is_refused() {
    let fixture = Fixture::new();
    let dir = tempfile::tempdir().unwrap();
    write_context(dir.path(), &fixture.capture()).unwrap();
    let error = load(dir.path()).unwrap_err().to_string();
    assert!(error.contains("fingerprints"), "{error}");
}

#[test]
fn source_preserves_verified_remote_pins_without_writing_the_lock() {
    let dir = tempfile::tempdir().unwrap();
    let remote_dir = dir.path().join("vendor/dep");
    fs::create_dir_all(&remote_dir).unwrap();
    fs::write(
        remote_dir.join("ridl.toml"),
        "[package]\nname = \"dep\"\nversion = \"1.0.0\"\n",
    )
    .unwrap();
    fs::write(
        remote_dir.join("p.typl"),
        "package dep\ntype Value: integer\n",
    )
    .unwrap();
    let tree = ridl_core::tree_hash(&remote_dir).unwrap();
    let url = "https://example.test/dep.tar";
    fs::write(
        dir.path().join("ridl.toml"),
        format!("[package]\nname = \"app\"\nversion = \"1.0.0\"\n[imports]\ndep = \"{url}\"\n"),
    )
    .unwrap();
    fs::write(
        dir.path().join("app.typl"),
        "package app\nimport dep.Value\nstruct Holder {\n value: Value\n}\n",
    )
    .unwrap();
    let lock = ridl_core::lock::Lockfile {
        entries: [(
            url.to_string(),
            ridl_core::lock::LockEntry {
                sha256: "d".repeat(64),
                tree: Some(tree.clone()),
            },
        )]
        .into_iter()
        .collect(),
    };
    let lock_path = dir.path().join("ridl.lock");
    ridl_core::lock::write_lockfile(&lock_path, &lock).unwrap();
    let before = fs::read(&lock_path).unwrap();
    let side = load(dir.path()).unwrap();
    assert_eq!(side.packages.len(), 1);
    let context = side.remote_context.unwrap();
    assert_eq!(context.packages.len(), 1);
    assert_eq!(context.pins["dep"].tree, tree);
    assert_eq!(context.pins["dep"].url, url);
    assert_eq!(before, fs::read(lock_path).unwrap());
}

#[test]
fn nested_ir_preserves_the_ir_readers_depth_guard() {
    let fixture = Fixture::new();
    let dir = tempfile::tempdir().unwrap();
    fixture.snapshots(dir.path());
    write_context(dir.path(), &fixture.capture()).unwrap();
    let mut json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(dir.path().join("context.json")).unwrap())
            .unwrap();
    json["remote_units"][1]["packages"][0]["decls"] = serde_json::json!([
        {"name":"Nested", "structDef":{"members":[{"field":{"name":"value", "type":"NESTED_TYPE"}}]}}
    ]);
    let template = json.to_string();
    for (depth, accepted) in [(80, true), (510, false)] {
        let mut nested = r#"{"primitive":0}"#.to_string();
        for _ in 0..depth {
            nested = format!(r#"{{"array":{{"element":{nested}}}}}"#);
        }
        let text = template.replace("\"NESTED_TYPE\"", &nested);
        fs::write(dir.path().join("context.json"), text).unwrap();
        let result = load(dir.path());
        if accepted {
            assert!(result.is_ok(), "{result:?}");
        } else {
            assert!(result.unwrap_err().to_string().contains("deeper than 1000"));
        }
    }
}

#[cfg(unix)]
#[test]
fn dangling_context_link_is_malformed_evidence() {
    let fixture = Fixture::new();
    let dir = tempfile::tempdir().unwrap();
    fixture.snapshots(dir.path());
    std::os::unix::fs::symlink(
        dir.path().join("absent.json"),
        dir.path().join("context.json"),
    )
    .unwrap();
    assert!(
        load(dir.path())
            .unwrap_err()
            .to_string()
            .contains("context.json")
    );
}

// Treating an empty marker as absent accepts an interrupted snapshot directory.
#[test]
fn empty_transaction_marker_refuses_a_snapshot_directory_side() {
    let fixture = Fixture::new();
    let dir = tempfile::tempdir().unwrap();
    let baseline = dir.path().join("baseline");
    fs::create_dir(&baseline).unwrap();
    fixture.snapshots(&baseline);
    assert!(load(&baseline).is_ok());
    fs::write(dir.path().join(".baseline.transaction"), "").unwrap();
    let ridlc::diff_side::DiffSideError::InterruptedPublication(error) =
        load(&baseline).expect_err("an empty marker must refuse the snapshot directory")
    else {
        panic!("an empty marker must report an interrupted publication");
    };
    assert!(
        error.to_string().contains("backup path is unknown"),
        "{error}"
    );
}
