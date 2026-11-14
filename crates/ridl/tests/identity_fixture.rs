//! The published identity fixture must match the compiler's output.
use std::path::Path;
use std::process::Command;

#[test]
fn identity_descriptor_matches_its_sources() {
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../ridl-rt-conformance/fixtures/identity");
    let out = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_ridl"))
        .arg("build")
        .arg(&fixture)
        .args(["--emit", "deployment", "--out-dir"])
        .arg(out.path())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let generated = std::fs::read(out.path().join("alpha.bench.deployment.binfb")).unwrap();
    ridl_descriptor::deployment::verify(&generated).unwrap();
    assert_eq!(
        generated,
        std::fs::read(fixture.join("identity.deployment.binfb")).unwrap()
    );
}
