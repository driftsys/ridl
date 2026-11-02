//! The instance numbers (rsdl reference §7) in the lowered system and in the
//! codegen deployment section, over the `instance-numbers` corpus entry and its
//! root `instances.lock`.

use std::path::Path;

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
