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

/// Copies the directory `from` into `to`, which exists.
fn copy_dir(from: &Path, to: &Path) {
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            std::fs::create_dir(&target).unwrap();
            copy_dir(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).unwrap();
        }
    }
}

/// Runs `ridlc build <entry> --out-dir <out> --emit <emit>`.
fn build(entry: &Path, out: &Path, emit: &str) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_ridlc"))
        .args(["build".as_ref(), entry.as_os_str()])
        .args(["--out-dir".as_ref(), out.as_os_str()])
        .args(["--emit", emit])
        .output()
        .expect("the ridlc binary must run")
}

#[test]
fn the_crate_holds_the_locked_constants() {
    let out = tempfile::tempdir().unwrap();
    let output = build(&corpus_entry(), out.path(), "rust");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let lib = std::fs::read_to_string(out.path().join("lib.rs")).unwrap();
    assert!(lib.contains("pub mod instances;"), "{lib}");
    let instances = std::fs::read_to_string(out.path().join("__ridl_instances.rs")).unwrap();
    assert!(
        instances.contains("pub const PRIMARY: i32 = 1;"),
        "{instances}"
    );
    assert!(
        instances.contains("pub const BACKUP: i32 = 2;"),
        "{instances}"
    );
    assert!(
        instances.contains("pub const UNIT: i32 = 3;"),
        "{instances}"
    );
    assert!(!instances.contains("Provisional:"), "{instances}");
}

#[test]
fn the_lock_changes_no_catalog_hash() {
    let catalogs = |with_lock: bool| {
        let entry = tempfile::tempdir().unwrap();
        copy_dir(&corpus_entry(), entry.path());
        if !with_lock {
            std::fs::remove_file(entry.path().join("instances.lock")).unwrap();
        }
        let out = tempfile::tempdir().unwrap();
        let output = build(entry.path(), out.path(), "catalog");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let mut files: Vec<(String, Vec<u8>)> = std::fs::read_dir(out.path())
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| path.to_string_lossy().ends_with(".catalog.binfb"))
            .map(|path| {
                (
                    path.file_name().unwrap().to_string_lossy().into_owned(),
                    std::fs::read(path).unwrap(),
                )
            })
            .collect();
        files.sort();
        files
    };
    let with_lock = catalogs(true);
    assert!(!with_lock.is_empty());
    assert_eq!(with_lock, catalogs(false));
}

/// The corpus entry with a fourth member, package `instances`, copied into a
/// temporary directory. With `with_system` false the entry's `system` file is
/// left out.
fn entry_with_instances_package(package: &str, with_system: bool) -> tempfile::TempDir {
    let entry = tempfile::tempdir().unwrap();
    copy_dir(&corpus_entry(), entry.path());
    if !with_system {
        std::fs::remove_file(entry.path().join("adas/car.rsdl")).unwrap();
    }
    std::fs::write(
        entry.path().join("ridl.toml"),
        "[workspace]\nmembers = [\"adas\", \"ego\", \"inst\"]\n",
    )
    .unwrap();
    std::fs::create_dir(entry.path().join("inst")).unwrap();
    std::fs::write(
        entry.path().join("inst/ridl.toml"),
        format!("[package]\nname = \"{package}\"\nversion = \"1.0.0\"\n"),
    )
    .unwrap();
    std::fs::write(
        entry.path().join("inst/types.typl"),
        format!("package {package}\n\ntype Flag: boolean\n"),
    )
    .unwrap();
    entry
}

#[test]
fn a_package_named_instances_is_refused_with_a_system() {
    let entry = entry_with_instances_package("instances", true);
    let out = tempfile::tempdir().unwrap();
    let output = build(entry.path(), out.path(), "rust");
    assert_eq!(output.status.code(), Some(1));
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        text.contains("package `instances` already uses that module name"),
        "{text}"
    );
}

/// The refusal covers every package whose first segment is `instances`, not
/// only the package named `instances`.
#[test]
fn a_package_starting_with_instances_is_refused_with_a_system() {
    let entry = entry_with_instances_package("instances.extra", true);
    let out = tempfile::tempdir().unwrap();
    let output = build(entry.path(), out.path(), "rust");
    assert_eq!(output.status.code(), Some(1));
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        text.contains("package `instances.extra` already uses that module name"),
        "{text}"
    );
}

/// The refusal is tied to the `instances` module, which only a workspace
/// that declares a system has: without one, package `instances` builds.
#[test]
fn a_package_named_instances_builds_without_a_system() {
    let entry = entry_with_instances_package("instances", false);
    let out = tempfile::tempdir().unwrap();
    let output = build(entry.path(), out.path(), "rust");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
