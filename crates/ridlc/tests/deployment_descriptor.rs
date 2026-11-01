//! `ridlc build --emit deployment`: one deployment descriptor per lowered
//! deployment, with the catalog descriptors of its regions embedded.

use std::path::{Path, PathBuf};
use std::process::Command;

use ridl_descriptor::deployment;

fn cabin() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/cabin")
}

/// Runs `ridlc build <entry> --out-dir <out> --emit <emits>` and returns
/// `(exit_code, stderr)`.
fn build(entry: &Path, out: &Path, emits: &str) -> (i32, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_ridlc"))
        .args(["build".as_ref(), entry.as_os_str()])
        .args(["--out-dir".as_ref(), out.as_os_str()])
        .args(["--emit", emits])
        .output()
        .expect("the ridlc binary must run");
    (
        output.status.code().expect("the process exits with a code"),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

/// The names of the files in `dir` that end with `suffix`, sorted.
fn files_ending(dir: &Path, suffix: &str) -> Vec<String> {
    let mut found: Vec<String> = std::fs::read_dir(dir)
        .expect("the output directory exists")
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|name| name.ends_with(suffix))
        .collect();
    found.sort();
    found
}

/// A copy of the cabin sources in a temporary directory, with `extra` added
/// to its `system.rsdl`.
fn cabin_copy(extra: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    for name in ["cabin.ridl", "ridl.toml", "system.rsdl"] {
        std::fs::copy(cabin().join(name), dir.path().join(name)).unwrap();
    }
    let system = dir.path().join("system.rsdl");
    let mut text = std::fs::read_to_string(&system).unwrap();
    text.push_str(extra);
    std::fs::write(&system, text).unwrap();
    dir
}

#[test]
fn cabin_writes_one_descriptor_per_deployment() {
    let out = tempfile::tempdir().unwrap();
    let (code, stderr) = build(&cabin(), out.path(), "deployment");
    assert_eq!(code, 0, "{stderr}");
    assert_eq!(
        files_ending(out.path(), deployment::FILE_SUFFIX),
        ["veh.cabin.Bench.deployment.binfb"]
    );

    let bytes = std::fs::read(out.path().join("veh.cabin.Bench.deployment.binfb")).unwrap();
    assert_eq!(&bytes[4..8], b"RDLD");
    let view = deployment::verify(&bytes).expect("the descriptor verifies");
    let json = deployment::to_json(view).expect("the descriptor prints");
    assert_eq!(json["name"], "Bench");
    assert_eq!(json["package"], "veh.cabin");
    let instances = json["instances"].as_array().unwrap();
    let climate = instances
        .iter()
        .find(|instance| instance["component"] == "veh.cabin.Climate")
        .expect("the Climate instance is described");
    let interfaces = json["interfaces"].as_array().unwrap();
    let offered: Vec<&str> = climate["offers"]
        .as_array()
        .unwrap()
        .iter()
        .map(|key| {
            interfaces
                .iter()
                .find(|interface| {
                    interface["catalog"] == key["catalog"] && interface["number"] == key["number"]
                })
                .expect("an offer names a deployed interface")["name"]
                .as_str()
                .unwrap()
        })
        .collect();
    assert!(offered.contains(&"Cabin"), "{offered:?}");
}

#[test]
fn the_embedded_catalog_equals_the_standalone_file() {
    let out = tempfile::tempdir().unwrap();
    let (code, stderr) = build(&cabin(), out.path(), "catalog,deployment");
    assert_eq!(code, 0, "{stderr}");
    let bytes = std::fs::read(out.path().join("veh.cabin.Bench.deployment.binfb")).unwrap();
    let view = deployment::verify(&bytes).unwrap();
    let json = deployment::to_json(view).unwrap();
    let catalogs = json["catalogs"].as_array().unwrap();
    assert!(!catalogs.is_empty());
    for catalog in catalogs {
        let name = catalog["name"].as_str().unwrap();
        let standalone = std::fs::read(out.path().join(format!("{name}.catalog.binfb"))).unwrap();
        assert_eq!(catalog["bytes_len"], standalone.len(), "{name}");
        // The embedded bytes are the standalone file: a window of the
        // deployment buffer equals it.
        assert!(
            bytes.windows(standalone.len()).any(|w| w == standalone),
            "the deployment embeds {name}.catalog.binfb unchanged"
        );
    }
}

#[test]
fn the_deployment_descriptor_does_not_change_the_catalog_file() {
    let alone = tempfile::tempdir().unwrap();
    let both = tempfile::tempdir().unwrap();
    assert_eq!(build(&cabin(), alone.path(), "catalog").0, 0);
    assert_eq!(build(&cabin(), both.path(), "catalog,deployment").0, 0);
    assert_eq!(
        std::fs::read(alone.path().join("veh.cabin.catalog.binfb")).unwrap(),
        std::fs::read(both.path().join("veh.cabin.catalog.binfb")).unwrap()
    );
}

#[test]
fn every_deployment_gets_its_own_descriptor() {
    let dir = cabin_copy(
        "\n/// A second deployment with everything on one machine.\n\
         deployment Single for Vehicle {\n  machine All { Climate, Panel, Telemetry }\n}\n",
    );
    let out = tempfile::tempdir().unwrap();
    let (code, stderr) = build(dir.path(), out.path(), "deployment");
    assert_eq!(code, 0, "{stderr}");
    assert_eq!(
        files_ending(out.path(), deployment::FILE_SUFFIX),
        [
            "veh.cabin.Bench.deployment.binfb",
            "veh.cabin.Single.deployment.binfb"
        ]
    );
}

#[test]
fn no_deployment_writes_no_file() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::copy(cabin().join("cabin.ridl"), dir.path().join("cabin.ridl")).unwrap();
    std::fs::copy(cabin().join("ridl.toml"), dir.path().join("ridl.toml")).unwrap();
    let out = tempfile::tempdir().unwrap();
    let (code, stderr) = build(dir.path(), out.path(), "deployment");
    assert_eq!(code, 0, "{stderr}");
    assert!(files_ending(out.path(), deployment::FILE_SUFFIX).is_empty());
}

#[test]
fn a_blocked_deployment_writes_no_file() {
    let dir = cabin_copy(
        "\n/// Leaves Telemetry on no machine.\n\
         deployment Partial for Vehicle {\n  machine Hpc { Climate, Panel }\n}\n",
    );
    let out = tempfile::tempdir().unwrap();
    let (code, stderr) = build(dir.path(), out.path(), "deployment");
    assert_eq!(code, 1, "{stderr}");
    assert!(stderr.contains("RSDL-701"), "{stderr}");
    // The error blocks its own deployment only: the sound one is written.
    assert_eq!(
        files_ending(out.path(), deployment::FILE_SUFFIX),
        ["veh.cabin.Bench.deployment.binfb"]
    );
}

#[test]
fn single_file_mode_writes_no_deployment_descriptor() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("lone.ridl");
    std::fs::copy(cabin().join("cabin.ridl"), &file).unwrap();
    let out = tempfile::tempdir().unwrap();
    let (code, stderr) = build(&file, out.path(), "deployment");
    assert_eq!(code, 0, "{stderr}");
    assert!(files_ending(out.path(), deployment::FILE_SUFFIX).is_empty());
}
