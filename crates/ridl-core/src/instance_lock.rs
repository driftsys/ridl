//! The `instances.lock` file: the line table that gives every instance of a
//! component a number.
//!
//! It has the form of [`interface_lock`](crate::interface_lock)'s table, with
//! a different key. The key is `component/instance`: the component's dotted
//! name, a `/`, then the instance name. A singleton instance is named `Unit`.
//!
//! ```text
//! # instances.lock — written by ridl lock; do not edit by hand.
//! next 5
//! veh.adas.Cruise/primary 1
//! veh.adas.Cruise/backup 2
//! veh.ego.status/Unit 3
//! veh.adas.Lane/left 4 retired
//! ```
//!
//! A number is at least 1 and at most [`MAX_NUMBER`]. This module is pure text
//! in, table out, and compiles without the `fs` feature; only [`read`] and
//! [`write`] touch the filesystem.

use core::fmt;
use core::str::FromStr;
#[cfg(feature = "fs")]
use std::path::Path;

use crate::interface_lock::{
    InvalidLockKey, LockEditError, LockError, LockTable, TableKey, is_ident, parse_table,
};

/// The file's name inside the directory that holds the system.
pub const FILE_NAME: &str = "instances.lock";

/// The first line of every written file.
pub const HEADER: &str = "# instances.lock — written by ridl lock; do not edit by hand.";

/// The highest number the file may hold.
pub const MAX_NUMBER: u32 = 2_147_483_647;

/// One entry's key: which instance of which component the entry numbers.
///
/// Its text form (`Display`, `FromStr`) is `component/instance`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct InstanceKey {
    /// The component's dotted name.
    pub component: String,
    /// The instance name: `Unit`, or an identifier that starts with an ASCII
    /// lowercase letter.
    pub instance: String,
}

impl fmt::Display for InstanceKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.component, self.instance)
    }
}

impl FromStr for InstanceKey {
    type Err = InvalidLockKey;

    /// Accepts exactly one `/`; a left side of one or more identifiers joined
    /// by `.`; and a right side that is `Unit` or an identifier whose first
    /// byte is an ASCII lowercase letter.
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let invalid = || {
            InvalidLockKey(format!(
                "`{text}` is not an instance key: expected `component/instance`, a dotted \
                 component name, a `/`, and `Unit` or a name that starts with a lowercase letter"
            ))
        };
        let (component, instance) = text.split_once('/').ok_or_else(invalid)?;
        let instance_ok = instance == "Unit"
            || (is_ident(instance) && instance.as_bytes()[0].is_ascii_lowercase());
        if component.split('.').all(is_ident) && instance_ok {
            Ok(InstanceKey {
                component: component.to_string(),
                instance: instance.to_string(),
            })
        } else {
            Err(invalid())
        }
    }
}

impl TableKey for InstanceKey {
    const FILE_NAME: &'static str = FILE_NAME;
    const HEADER: &'static str = HEADER;
    const MAX_NUMBER: u32 = MAX_NUMBER;
}

/// The parsed table of `instances.lock`.
pub type InstanceLock = LockTable<InstanceKey>;

/// Parses the text of an `instances.lock`; the malformed shapes are those of
/// [`interface_lock::parse`](crate::interface_lock::parse), with the number
/// range of [`MAX_NUMBER`].
pub fn parse(text: &str) -> Result<InstanceLock, LockError> {
    parse_table(text)
}

impl LockTable<InstanceKey> {
    /// Moves every live and retired entry `old/*` to `new/*`, keeping each
    /// number. Returns the old key, the new key and the number of each moved
    /// entry, in file order. Nothing changes on error: `NoEntryUnder(old)`
    /// when no entry is keyed under `old`, and `KeyAlreadyLive` when a live
    /// `new/x` exists for a live `old/x`.
    pub fn rename_component(
        &mut self,
        old: &str,
        new: &str,
    ) -> Result<Vec<(InstanceKey, InstanceKey, u32)>, LockEditError<InstanceKey>> {
        if !self.has_component(old, true) {
            return Err(LockEditError::NoEntryUnder(old.to_string()));
        }
        for entry in self
            .entries
            .iter()
            .filter(|entry| !entry.retired && entry.key.component == old)
        {
            let target = InstanceKey {
                component: new.to_string(),
                instance: entry.key.instance.clone(),
            };
            if self.live(&target).is_some() {
                return Err(LockEditError::KeyAlreadyLive(target));
            }
        }
        let mut moved = Vec::new();
        for entry in self
            .entries
            .iter_mut()
            .filter(|entry| entry.key.component == old)
        {
            let before = entry.key.clone();
            entry.key.component = new.to_string();
            moved.push((before, entry.key.clone(), entry.number));
        }
        Ok(moved)
    }

    /// Marks every live entry `component/*` retired. Returns each key and
    /// number, in file order. `NoEntryUnder` when no entry under the component
    /// is live; nothing changes then.
    pub fn retire_component(
        &mut self,
        component: &str,
    ) -> Result<Vec<(InstanceKey, u32)>, LockEditError<InstanceKey>> {
        let mut retired = Vec::new();
        for entry in self
            .entries
            .iter_mut()
            .filter(|entry| !entry.retired && entry.key.component == component)
        {
            entry.retired = true;
            retired.push((entry.key.clone(), entry.number));
        }
        if retired.is_empty() {
            return Err(LockEditError::NoEntryUnder(component.to_string()));
        }
        Ok(retired)
    }

    /// Whether an entry is keyed under `component`; a retired entry counts
    /// only when `include_retired` is set.
    pub fn has_component(&self, component: &str, include_retired: bool) -> bool {
        self.entries
            .iter()
            .any(|entry| entry.key.component == component && (include_retired || !entry.retired))
    }
}

/// The raw text of `dir/instances.lock`, or `None` when the file does not
/// exist. Parsing is the caller's. Any other I/O failure is the error.
#[cfg(feature = "fs")]
pub fn read(dir: &Path) -> std::io::Result<Option<String>> {
    match std::fs::read_to_string(dir.join(FILE_NAME)) {
        Ok(text) => Ok(Some(text)),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err),
    }
}

/// Writes `lock` to `dir/instances.lock`, replacing any existing file.
#[cfg(feature = "fs")]
pub fn write(dir: &Path, lock: &InstanceLock) -> std::io::Result<()> {
    std::fs::write(dir.join(FILE_NAME), lock.render())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interface_lock::{MergeOutcome, merge};

    const EXAMPLE: &str = "\
# instances.lock — written by ridl lock; do not edit by hand.
next 5
veh.adas.Cruise/primary 1
veh.adas.Cruise/backup 2
veh.ego.status/Unit 3
veh.adas.Lane/left 4 retired
";

    #[test]
    fn the_example_parses_and_renders_byte_for_byte() {
        let lock = parse(EXAMPLE).unwrap();
        assert_eq!(lock.next, 5);
        assert_eq!(lock.entries.len(), 4);
        assert_eq!(lock.render(), EXAMPLE);
    }

    #[test]
    fn a_key_is_a_dotted_component_a_slash_and_an_instance() {
        for good in [
            "veh.adas.Cruise/primary",
            "veh.ego.status/Unit",
            "a.B/x1",
            "a.B/unit",
        ] {
            assert!(good.parse::<InstanceKey>().is_ok(), "{good}");
        }
        for bad in [
            "veh.adas.Cruise",
            "veh.adas.Cruise/",
            "/primary",
            "a.B/x/y",
            "veh..C/x",
            "veh.C/Primary",
            "veh.C/_x",
            "veh.C/pri-mary",
        ] {
            assert!(bad.parse::<InstanceKey>().is_err(), "{bad}");
        }
    }

    #[test]
    fn a_number_outside_the_range_is_malformed() {
        let header = format!("{HEADER}\n");
        assert!(parse(&format!("{header}next 2147483648\na.B/x 2147483647\n")).is_ok());
        assert!(parse(&format!("{header}next 2147483649\n")).is_err());
        assert!(parse(&format!("{header}next 2147483649\na.B/x 2147483648\n")).is_err());
        assert!(parse(&format!("{header}next 2\na.B/x 0\n")).is_err());
    }

    #[test]
    fn allocate_refuses_a_number_above_the_range() {
        let mut lock = parse(&format!("{HEADER}\nnext 2147483648\na.B/x 2147483647\n")).unwrap();
        assert_eq!(
            lock.allocate("a.B/y".parse().unwrap()),
            Err(LockEditError::Exhausted)
        );
    }

    #[test]
    fn a_component_rename_moves_every_live_and_retired_entry() {
        let mut lock = parse(EXAMPLE).unwrap();
        let moved = lock
            .rename_component("veh.adas.Lane", "veh.adas.Keep")
            .unwrap();
        assert_eq!(moved.len(), 1);
        let mut lock = parse(EXAMPLE).unwrap();
        lock.rename_component("veh.adas.Cruise", "veh.adas.Speed")
            .unwrap();
        assert!(
            lock.render()
                .contains("veh.adas.Speed/primary 1\nveh.adas.Speed/backup 2\n")
        );
        assert_eq!(lock.next, 5);
    }

    #[test]
    fn a_component_rename_changes_nothing_on_error() {
        let mut lock = parse(EXAMPLE).unwrap();
        assert_eq!(
            lock.rename_component("veh.none.X", "veh.adas.Y"),
            Err(LockEditError::NoEntryUnder("veh.none.X".into()))
        );
        lock.entries.push(crate::interface_lock::LockEntry {
            key: "veh.adas.Speed/primary".parse().unwrap(),
            number: 9,
            retired: false,
            range: Default::default(),
        });
        let before = lock.clone();
        assert_eq!(
            lock.rename_component("veh.adas.Cruise", "veh.adas.Speed"),
            Err(LockEditError::KeyAlreadyLive(
                "veh.adas.Speed/primary".parse().unwrap()
            ))
        );
        assert_eq!(lock, before);
    }

    #[test]
    fn a_component_retire_retires_every_live_entry() {
        let mut lock = parse(EXAMPLE).unwrap();
        assert_eq!(lock.retire_component("veh.adas.Cruise").unwrap().len(), 2);
        assert!(
            lock.render()
                .contains("veh.adas.Cruise/primary 1 retired\n")
        );
        assert_eq!(
            lock.retire_component("veh.adas.Lane"),
            Err(LockEditError::NoEntryUnder("veh.adas.Lane".into()))
        );
    }

    #[test]
    fn has_component_counts_a_retired_entry_only_on_request() {
        let lock = parse(EXAMPLE).unwrap();
        assert!(lock.has_component("veh.adas.Lane", true));
        assert!(!lock.has_component("veh.adas.Lane", false));
        assert!(!lock.has_component("veh.adas.Nope", true));
    }

    #[test]
    fn a_single_key_rename_retire_and_live_work_on_the_instance_table() {
        let mut lock = parse(EXAMPLE).unwrap();
        let old: InstanceKey = "veh.ego.status/Unit".parse().unwrap();
        let new: InstanceKey = "veh.ego.state/Unit".parse().unwrap();
        assert_eq!(lock.rename(&old, new.clone()), Ok(3));
        assert!(lock.live(&new).is_some());
        assert_eq!(lock.retire(&new), Ok(3));
        assert!(lock.live(&new).is_none());
    }

    #[test]
    fn merge_returns_exhausted_when_renumbering_has_no_number_left() {
        let base = parse(&format!("{HEADER}\nnext 1\n")).unwrap();
        let ours = parse(&format!(
            "{HEADER}\nnext {}\nveh.C/a 1\n",
            InstanceKey::MAX_NUMBER + 1
        ))
        .unwrap();
        let theirs = parse(&format!("{HEADER}\nnext 2\nveh.C/b 1\n")).unwrap();
        let outcome = std::panic::catch_unwind(|| merge(&base, &ours, &theirs, 7));
        assert!(
            matches!(outcome, Ok(MergeOutcome::Exhausted)),
            "{outcome:?}"
        );
    }

    #[test]
    fn merge_renumbers_to_the_last_available_number() {
        let base = parse(&format!("{HEADER}\nnext 1\n")).unwrap();
        let ours = parse(&format!(
            "{HEADER}\nnext {}\nveh.C/a 1\n",
            InstanceKey::MAX_NUMBER
        ))
        .unwrap();
        let theirs = parse(&format!("{HEADER}\nnext 2\nveh.C/b 1\n")).unwrap();
        let outcome = merge(&base, &ours, &theirs, 7);
        assert!(matches!(outcome, MergeOutcome::Clean(_)), "{outcome:?}");
        let MergeOutcome::Clean(merged) = outcome else {
            unreachable!()
        };
        assert_eq!(
            merged.live(&"veh.C/b".parse().unwrap()).unwrap().number,
            InstanceKey::MAX_NUMBER
        );
        assert_eq!(merged.live(&"veh.C/a".parse().unwrap()).unwrap().number, 1);
        assert_eq!(merged.next, InstanceKey::MAX_NUMBER + 1);
        assert!(
            parse(&merged.render()).is_ok(),
            "the final number and exhausted next parse"
        );
    }

    #[test]
    fn two_instance_allocations_merge_with_the_instance_header() {
        let base = parse(&format!("{HEADER}\nnext 2\nveh.C/a 1\n")).unwrap();
        let ours = parse(&format!("{HEADER}\nnext 3\nveh.C/a 1\nveh.C/b 2\n")).unwrap();
        let theirs = parse(&format!("{HEADER}\nnext 3\nveh.C/a 1\nveh.C/c 2\n")).unwrap();
        let merged = match merge(&base, &ours, &theirs, 7) {
            MergeOutcome::Clean(merged) => merged,
            MergeOutcome::Conflict { text } => panic!("unexpected conflict:\n{text}"),
            MergeOutcome::Exhausted => panic!("unexpected number exhaustion"),
        };
        assert_eq!(
            merged.render(),
            format!("{HEADER}\nnext 4\nveh.C/a 1\nveh.C/b 2\nveh.C/c 3\n")
        );
    }
}
