//! Builds the document store example through the CLI's build driver, then
//! compiles and runs its consumer against the emitted crate with plain rustc.

use std::path::Path;
use std::process::Command;

use ridlc::Emit;

mod support;

#[test]
fn the_emitted_docstore_crate_round_trips_200_kb() {
    let example = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/docstore");
    assert!(
        example.join("ridl.toml").is_file(),
        "the docstore example must exist"
    );
    let out = tempfile::tempdir().expect("the output directory is created");
    let libs = tempfile::tempdir().expect("the library directory is created");
    let build = ridlc::run_build(&example, out.path(), &[Emit::Rust], false.into())
        .expect("the example build runs");
    assert!(
        build.diagnostics.is_empty(),
        "the example must have no diagnostics: {:?}",
        build.diagnostics
    );

    // Read the emitted manifest so the proof links the crate a user receives.
    let manifest: toml::Value = toml::from_str(
        &std::fs::read_to_string(out.path().join("Cargo.toml"))
            .expect("the generated manifest exists"),
    )
    .expect("the generated manifest is TOML");
    let name = manifest["package"]["name"]
        .as_str()
        .expect("the crate has a name");
    let rt = support::ridl_rt_rlib(libs.path());
    let loopback = support::ridl_loopback_rlib(libs.path(), &rt);
    let api = libs.path().join(format!("lib{name}.rlib"));
    let dependency = format!("dependency={}", libs.path().display());
    let emitted = Command::new("rustc")
        .args([
            "--edition=2024",
            "--crate-type=rlib",
            "--crate-name",
            name,
            "--cfg",
            "feature=\"std\"",
            "--extern",
        ])
        .arg(format!("ridl_rt={}", rt.display()))
        .args(["-L", &dependency])
        .arg(out.path().join("lib.rs"))
        .arg("-o")
        .arg(&api)
        .output()
        .expect("rustc runs");
    assert!(
        emitted.status.success(),
        "the emitted crate must compile: {}",
        String::from_utf8_lossy(&emitted.stderr)
    );
    let program = libs.path().join("docstore_consumer");
    let consumer = Command::new("rustc")
        .args([
            "--edition=2024",
            "--crate-name=docstore_consumer",
            "--extern",
        ])
        .arg(format!("{name}={}", api.display()))
        .arg("--extern")
        .arg(format!("ridl_rt={}", rt.display()))
        .arg("--extern")
        .arg(format!("ridl_loopback={}", loopback.display()))
        .args(["-L", &dependency])
        .arg(example.join("consumer/src/main.rs"))
        .arg("-o")
        .arg(&program)
        .output()
        .expect("rustc runs");
    assert!(
        consumer.status.success(),
        "the consumer must compile: {}",
        String::from_utf8_lossy(&consumer.stderr)
    );
    let run = Command::new(&program).output().expect("the consumer runs");
    let stdout = String::from_utf8_lossy(&run.stdout);
    assert!(
        run.status.success(),
        "the consumer must complete: {stdout}{}",
        String::from_utf8_lossy(&run.stderr)
    );
    for expected in [
        "docstore put: 204800 bytes",
        "docstore describe: 204800 bytes",
        "docstore round trip: 204800 bytes",
    ] {
        assert!(
            stdout.lines().any(|line| line == expected),
            "missing `{expected}` in:\n{stdout}"
        );
    }
}
