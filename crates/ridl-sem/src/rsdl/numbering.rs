//! The instance numbers (rsdl reference §7): every instance of every closure
//! component takes one number, once for the whole workspace, from the root
//! `instances.lock`.
//!
//! An instance with a live entry in the lock keeps the entry's number. An
//! instance with no entry takes a provisional number, counted from the lock's
//! `next` in the byte order of its key, so the numbering is the same on every
//! machine until `ridl lock` writes the entries.

use std::collections::{BTreeMap, BTreeSet};

use ridl_core::diag::{DiagCode, Diagnostic, FileId, Severity, Span};
use ridl_core::instance_lock::{FILE_NAME, InstanceKey, InstanceLock, MAX_NUMBER};
use ridl_core::interface_lock::LockEntry;
use rowan::{TextRange, TextSize};

use super::CheckedSystem;

/// The numbers of the instances of a system, and what the lock holds that the
/// system does not.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InstanceNumbering {
    /// The number of each instance of the closure, with whether it is
    /// provisional (the lock has no live entry for the key). Every number is
    /// between 1 and 2147483647.
    pub numbers: BTreeMap<InstanceKey, (i32, bool)>,
    /// The live entries whose component or instance is not in the system, in
    /// file order. Empty when the closure has errors, because the instances
    /// are then not known.
    pub orphans: Vec<LockEntry<InstanceKey>>,
    /// The instances that would take a number above 2147483647. They are not
    /// in `numbers`.
    pub overflow: Vec<InstanceKey>,
    /// Whether some instance has no live entry — the condition that makes
    /// RSDL-314 name `--rename` beside `--retire`.
    pub any_provisional: bool,
}

/// Numbers every instance of `system`'s closure against `lock` (rsdl §7). An
/// absent lock is an empty table with `next 1`. A workspace with no closure has
/// an empty numbering.
pub fn number_instances(system: &CheckedSystem, lock: Option<&InstanceLock>) -> InstanceNumbering {
    let Some(closure) = &system.closure else {
        return InstanceNumbering::default();
    };
    let keys: BTreeSet<InstanceKey> = closure
        .components
        .iter()
        .flat_map(|component| {
            let name = component.qualified_name();
            component.instances.iter().map(move |instance| InstanceKey {
                component: name.clone(),
                instance: instance.clone(),
            })
        })
        .collect();

    let mut numbering = InstanceNumbering::default();
    let mut provisional = Vec::new();
    for key in &keys {
        match lock.and_then(|lock| lock.live(key)) {
            // A live entry's number is at most `MAX_NUMBER`, which is `i32::MAX`.
            Some(entry) => {
                numbering
                    .numbers
                    .insert(key.clone(), (entry.number as i32, false));
            }
            None => provisional.push(key.clone()),
        }
    }
    // The order of the key text, not of the key's fields: `veh.a.b/x` comes
    // before `veh.a/x` because `.` is below `/`.
    provisional.sort_by_cached_key(ToString::to_string);
    let mut next = u64::from(lock.map_or(1, |lock| lock.next));
    for key in provisional {
        numbering.any_provisional = true;
        if next > u64::from(MAX_NUMBER) {
            numbering.overflow.push(key);
        } else {
            numbering.numbers.insert(key, (next as i32, true));
            next += 1;
        }
    }

    if let Some(lock) = lock
        && !system.closure_has_errors
    {
        numbering.orphans = lock
            .entries
            .iter()
            .filter(|entry| !entry.retired && !keys.contains(&entry.key))
            .cloned()
            .collect();
    }
    numbering
}

/// The RSDL-314 diagnostics of `numbering`'s orphans, and the RSDL-315 of an
/// exhausted number range, on `lock_file`. `lock_dir` is the directory the
/// message gives to `ridl lock`. The orphan entries carry their own line
/// ranges, so the table they came from (`_lock`) is not read.
///
/// A component with live entries and no instance in the system draws one
/// RSDL-314, on its first entry. Every other orphan draws one on its own line.
pub fn instance_lock_diagnostics(
    numbering: &InstanceNumbering,
    _lock: &InstanceLock,
    lock_file: FileId,
    lock_dir: &str,
) -> Vec<Diagnostic> {
    let in_system: BTreeSet<&str> = numbering
        .numbers
        .keys()
        .chain(&numbering.overflow)
        .map(|key| key.component.as_str())
        .collect();
    let mut reported_components = BTreeSet::new();
    let mut diagnostics = Vec::new();
    for entry in &numbering.orphans {
        let component = entry.key.component.as_str();
        let message = if in_system.contains(component) {
            instance_message(&entry.key, lock_dir, numbering.any_provisional)
        } else if reported_components.insert(component) {
            component_message(component, lock_dir, numbering.any_provisional)
        } else {
            continue;
        };
        diagnostics.push(Diagnostic {
            code: DiagCode::RSDL_314,
            severity: Severity::Error,
            message,
            primary: Span {
                file: lock_file,
                range: entry.range,
            },
            labels: Vec::new(),
            fixits: Vec::new(),
        });
    }
    if !numbering.overflow.is_empty() {
        diagnostics.push(Diagnostic {
            code: DiagCode::RSDL_315,
            severity: Severity::Error,
            message: format!(
                "`{FILE_NAME}` has no number left: {} instances would take a number above {MAX_NUMBER}",
                numbering.overflow.len()
            ),
            primary: Span {
                file: lock_file,
                range: TextRange::empty(TextSize::from(0)),
            },
            labels: Vec::new(),
            fixits: Vec::new(),
        });
    }
    diagnostics
}

/// The RSDL-314 message for a live entry whose instance is not in the system,
/// though its component is. It mirrors the RIDL-409 message.
fn instance_message(key: &InstanceKey, dir: &str, any_provisional: bool) -> String {
    let component = &key.component;
    if any_provisional {
        format!(
            "`{key}` is a live entry of `{FILE_NAME}` with no instance in the system: run \
             `ridl lock {dir} --rename {key}={component}/new` when an instance without an entry \
             is this instance under a new name, or `ridl lock {dir} --retire {key}` when the \
             instance is gone"
        )
    } else {
        format!(
            "`{key}` is a live entry of `{FILE_NAME}` with no instance in the system: run \
             `ridl lock {dir} --retire {key}` to record that the instance is gone"
        )
    }
}

/// The RSDL-314 message for a component that has live entries and no instance
/// in the system.
fn component_message(component: &str, dir: &str, any_provisional: bool) -> String {
    if any_provisional {
        // `--rename` takes a fully qualified component on both sides, so the
        // example new name sits in the same package as the old one.
        let renamed = match component.rsplit_once('.') {
            Some((package, _)) => format!("{package}.New"),
            None => "New".to_owned(),
        };
        format!(
            "`{component}` has live entries in `{FILE_NAME}` and is not in the system: run \
             `ridl lock {dir} --rename {component}={renamed}` when the component was renamed, or \
             `ridl lock {dir} --retire {component}` when it is gone"
        )
    } else {
        format!(
            "`{component}` has live entries in `{FILE_NAME}` and is not in the system: run \
             `ridl lock {dir} --retire {component}` to record that the component is gone"
        )
    }
}

#[cfg(test)]
mod tests {
    use ridl_core::diag::SourceMap;
    use ridl_core::instance_lock::{self, HEADER};

    use super::super::tests::{SIZED_CLOSURE, check_topology};
    use super::*;

    fn sized_closure() -> CheckedSystem {
        let system = check_topology(&[("veh/topology/x.rsdl", SIZED_CLOSURE)]);
        assert!(!system.closure_has_errors, "{:?}", system.diagnostics);
        system
    }

    /// Two lone services whose names differ by a trailing `.x`: the key text
    /// `veh.cruise.x/Unit` is below `veh.cruise/Unit`.
    fn two_lone_services() -> CheckedSystem {
        let services = "package veh.topology\n\
                        service veh.cruise { signal a: boolean @[100ms..1s] }\n\
                        service veh.cruise.x { signal b: boolean @[100ms..1s] }\n";
        let system = "package veh.topology\nsystem S { veh.cruise, veh.cruise.x }\n";
        let system = check_topology(&[
            ("veh/topology/services.ridl", services),
            ("veh/topology/system.rsdl", system),
        ]);
        assert!(!system.closure_has_errors, "{:?}", system.diagnostics);
        system
    }

    /// `SIZED_CLOSURE` with a `system` member that names nothing (RSDL-602).
    fn closure_with_an_error() -> CheckedSystem {
        let text = SIZED_CLOSURE.replace(
            "{ Lane, Panel, Backend, Pair, veh.diag.access }",
            "{ Lane, Missing }",
        );
        let system = check_topology(&[("veh/topology/x.rsdl", text.as_str())]);
        assert!(system.closure_has_errors);
        system
    }

    /// A file id for the lock file, as the driver interns it.
    fn lock_file() -> FileId {
        SourceMap::new().file_id(FILE_NAME, "")
    }

    fn lock(body: &str) -> InstanceLock {
        instance_lock::parse(&format!("{HEADER}\n{body}")).expect("the lock parses")
    }

    fn key(text: &str) -> InstanceKey {
        text.parse().expect("the key parses")
    }

    #[test]
    fn with_no_lock_every_instance_is_provisional_in_key_byte_order() {
        let numbering = number_instances(&sized_closure(), None);
        let listed: Vec<(String, i32, bool)> = numbering
            .numbers
            .iter()
            .map(|(key, (n, p))| (key.to_string(), *n, *p))
            .collect();
        let mut by_number = listed.clone();
        by_number.sort_by_key(|(_, n, _)| *n);
        assert_eq!(
            by_number
                .iter()
                .map(|(k, n, _)| (k.as_str(), *n))
                .collect::<Vec<_>>(),
            [
                ("veh.diag.access/Unit", 1),
                ("veh.topology.Backend/Unit", 2),
                ("veh.topology.Lane/Unit", 3),
                ("veh.topology.Pair/backup", 4),
                ("veh.topology.Pair/primary", 5),
                ("veh.topology.Panel/Unit", 6),
            ]
        );
        assert!(listed.iter().all(|(_, _, provisional)| *provisional));
        assert!(numbering.any_provisional);
    }

    #[test]
    fn a_live_entry_freezes_its_number_and_the_rest_count_from_next() {
        let lock = lock("next 10\nveh.topology.Pair/primary 4\n");
        let numbering = number_instances(&sized_closure(), Some(&lock));
        assert_eq!(
            numbering.numbers[&key("veh.topology.Pair/primary")],
            (4, false)
        );
        assert_eq!(numbering.numbers[&key("veh.diag.access/Unit")], (10, true));
        assert_eq!(
            numbering.numbers[&key("veh.topology.Panel/Unit")],
            (14, true)
        );
    }

    #[test]
    fn a_retired_entry_does_not_freeze_its_key() {
        let lock = lock("next 8\nveh.topology.Pair/backup 7 retired\n");
        let numbering = number_instances(&sized_closure(), Some(&lock));
        assert!(numbering.numbers[&key("veh.topology.Pair/backup")].1);
        assert!(numbering.numbers.values().all(|(n, _)| *n != 7));
        assert!(numbering.orphans.is_empty());
    }

    #[test]
    fn the_provisional_order_is_the_key_text() {
        let numbering = number_instances(&two_lone_services(), None);
        assert_eq!(numbering.numbers[&key("veh.cruise.x/Unit")].0, 1);
        assert_eq!(numbering.numbers[&key("veh.cruise/Unit")].0, 2);
    }

    #[test]
    fn an_orphan_instance_and_an_orphan_component_draw_rsdl_314() {
        let lock = lock(
            "next 10\nveh.topology.Pair/spare 7\nveh.topology.Gone/a 8\nveh.topology.Gone/b 9\n",
        );
        let numbering = number_instances(&sized_closure(), Some(&lock));
        let diagnostics = instance_lock_diagnostics(&numbering, &lock, lock_file(), ".");
        assert_eq!(
            diagnostics
                .iter()
                .map(|d| d.code.as_str())
                .collect::<Vec<_>>(),
            ["RSDL-314", "RSDL-314"]
        );
        assert!(diagnostics.iter().any(|d| {
            d.message
                .contains("--rename veh.topology.Gone=veh.topology.New")
        }));
        assert!(
            diagnostics
                .iter()
                .any(|d| d.message.contains("--retire veh.topology.Pair/spare"))
        );
    }

    #[test]
    fn a_closure_with_errors_reports_no_orphan() {
        let numbering = number_instances(
            &closure_with_an_error(),
            Some(&lock("next 2\nveh.topology.Panel/Unit 1\n")),
        );
        assert!(numbering.orphans.is_empty());
    }

    #[test]
    fn numbers_above_the_range_are_overflow_and_rsdl_315() {
        let lock = lock("next 2147483647\n");
        let numbering = number_instances(&sized_closure(), Some(&lock));
        assert_eq!(numbering.overflow.len(), 5);
        let diagnostics = instance_lock_diagnostics(&numbering, &lock, lock_file(), ".");
        assert_eq!(
            diagnostics
                .iter()
                .map(|d| d.code.as_str())
                .collect::<Vec<_>>(),
            ["RSDL-315"]
        );
    }
}
