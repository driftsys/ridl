//! The instance numbers (rsdl reference §7) in the lowered system and in the
//! codegen deployment section, over the `instance-numbers` corpus entry and its
//! root `instances.lock`.

use std::path::Path;
use std::process::Command;

use ridl_core::db::RidlDatabase;

fn corpus_entry() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/corpus/instance-numbers")
}

/// The numbers of the component `name`, as `(instance, number, provisional)`.
fn numbers_of(system: &ridl_ir::v2::System, name: &str) -> Vec<(String, i32, bool)> {
    system
        .components
        .iter()
        .find(|component| component.qualified_name() == name)
        .unwrap_or_else(|| panic!("the system has no component `{name}`"))
        .numbers
        .iter()
        .map(|n| (n.instance.clone(), n.number, n.provisional))
        .collect()
}

#[test]
fn the_corpus_system_carries_the_locked_numbers() {
    let mut db = RidlDatabase::default();
    let output = ridlc::compile_workspace(&mut db, &corpus_entry()).expect("the entry loads");
    let system = output.system.expect("the entry lowers a system");
    assert_eq!(
        numbers_of(&system, "veh.adas.Cruise"),
        [
            ("primary".to_string(), 1, false),
            ("backup".to_string(), 2, false)
        ]
    );
    assert_eq!(
        numbers_of(&system, "veh.ego.status"),
        [("Unit".to_string(), 3, false)]
    );
}

#[test]
fn the_descriptor_and_the_section_carry_the_locked_number() {
    let out = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_ridlc"))
        .args(["build".as_ref(), corpus_entry().as_os_str()])
        .args(["--out-dir".as_ref(), out.path().as_os_str()])
        .args(["--emit", "deployment"])
        .output()
        .expect("the ridlc binary must run");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let bytes = std::fs::read_dir(out.path())
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| path.to_string_lossy().ends_with(".deployment.binfb"))
        .map(|path| std::fs::read(path).unwrap())
        .expect("a deployment descriptor is written");
    let view = ridl_descriptor::deployment::verify(&bytes).expect("the descriptor verifies");
    let descriptor = ridl_descriptor::deployment::Deployment::try_from(view).expect("converts");

    let mut db = RidlDatabase::default();
    let compiled = ridlc::compile_workspace(&mut db, &corpus_entry()).expect("the entry loads");
    let system = compiled.system.expect("the entry lowers a system");
    let section =
        ridl_ir::codegen::lower_deployment(&system, "Bench", &[]).expect("the section lowers");

    let lock = [("primary", 1), ("backup", 2), ("Unit", 3)];
    assert_eq!(descriptor.instances.len(), lock.len());
    assert_eq!(section.instances.len(), lock.len());
    for (name, locked) in lock {
        let from_descriptor = descriptor
            .instances
            .iter()
            .find(|instance| instance.instance == name)
            .unwrap_or_else(|| panic!("the descriptor has no instance {name}"));
        let from_section = section
            .instances
            .iter()
            .find(|instance| instance.instance == name)
            .unwrap_or_else(|| panic!("the section has no instance {name}"));
        assert_eq!(
            (from_descriptor.number, from_descriptor.provisional),
            (locked, false),
            "{name}"
        );
        assert_eq!(
            (from_descriptor.number, from_descriptor.provisional),
            (from_section.number, from_section.provisional),
            "{name}"
        );
    }
}
