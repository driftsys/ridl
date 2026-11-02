//! `ridl lock` — the one command that writes a unit's `interfaces.lock`
//! (lock design §5) and the workspace's `instances.lock` (rsdl reference §7).
//!
//! Plain `ridl lock` allocates a number to every interface that has none —
//! every declared interface and every service's inline shape whose key has
//! no live entry — and writes each unit's own file, in its manifest
//! directory. It also allocates a number to every instance of the system
//! that has none, in the order of the provisional numbers the checker showed,
//! and writes `instances.lock` beside the root manifest, only when `PATH`
//! resolves to the root and only when it allocates. It is the only form that
//! allocates: a branch never allocates,
//! and the release recipe or the merge queue runs this on `main` (lock
//! design §4). `--rename OLD=NEW` and
//! `--retire NAME` rewrite entries in place and never allocate: one entry for
//! an interface or an instance, and every entry under the component for a
//! component. They are the fix RIDL-409 and RSDL-314 name, so they run with
//! either present.
//! A key with a `/` is an instance (`component/instance`); a key without one
//! is a component when `instances.lock` has an entry under it, else an
//! interface. An interface edit needs `PATH` to resolve to exactly one unit;
//! an instance or component edit needs it to resolve to the root. Any other
//! compile error exits 1 and writes nothing, whichever unit it is in (plan
//! decision PD-6).
//!
//! The command compiles first, through [`ridlc::compile_workspace`] — no
//! network and no `ridl.lock` round trip; a remote unit is read from the
//! cache so the consumer's imports resolve, and its packages are then left
//! out, because its `interfaces.lock` is its owner's — and reads each unit's
//! directory and its lock as loaded from a second [`load_workspace`] over
//! the same tree: `ridlc`'s output carries the checked IR and the
//! diagnostics but no package handle, and the two loads read the same files
//! in the same order.
//! One line per change goes to stdout — `allocated Name N`,
//! `renamed Old New N`, `retired Name N` — prefixed with the unit
//! directory relative to `PATH` over more than one unit (plan decision
//! PD-13); diagnostics go to stderr (ADR-0010 decision 2). The exit codes
//! follow ADR-0010 decision 1: 0 when the file is written or there is nothing
//! to change, 1 on a diagnostic error over the source, 2 on a bad flag or a
//! path or I/O failure.
//!
//! `ridl lock merge BASE OURS THEIRS MARKER_SIZE` is the git merge driver for
//! both files (lock design §6): it reads the three sides, runs
//! [`interface_lock::merge`] — a three-way merge over entries matched by
//! number, with no file access — and writes the result to OURS. Exit 0 when
//! the merge is clean, 1 when entries disagree (OURS then holds git conflict
//! markers around only the disagreeing entries and is RIDL-410 for
//! `interfaces.lock`, or RSDL-315 for `instances.lock`, until an author
//! resolves it), 2 when an input cannot be read or does not parse (OURS is
//! then left as it was).

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use ridl_core::diag::{DiagCode, Diagnostic, Severity, render};
use ridl_core::instance_lock::{self, InstanceKey, InstanceLock};
use ridl_core::interface_lock::{
    self, InterfaceLock, InvalidLockKey, LockKey, LockTable, MergeOutcome, TableKey,
};
use ridl_core::lint::drop_allowed_by_default;
use ridl_core::{RidlDatabase, load_workspace};
use ridl_ir::v2;
use ridl_sem::rsdl::InstanceNumbering;
use rowan::TextRange;

/// One unit of the run: its manifest directory, its lock as loaded — the
/// empty lock when the directory has none — and the checked IR of each of its
/// packages.
struct LockedUnit {
    name: String,
    dir: PathBuf,
    lock: InterfaceLock,
    packages: Vec<v2::Package>,
}

/// Whether no error among `diagnostics` is anything but RIDL-409 or RSDL-314
/// — the errors `--rename` and `--retire` run with, and the condition under
/// which `ridl check` still runs its desk check (lock design §4). True when
/// there is no error at all.
pub(crate) fn only_lock_orphans<'a>(diagnostics: impl IntoIterator<Item = &'a Diagnostic>) -> bool {
    diagnostics.into_iter().all(|diagnostic| {
        diagnostic.severity != Severity::Error
            || diagnostic.code == DiagCode::RIDL_409
            || diagnostic.code == DiagCode::RSDL_314
    })
}

/// One key of a `--rename` or `--retire` flag: an instance (it holds a `/`),
/// or a name that is a component or an interface, decided against the locks.
enum FlagKey {
    Instance(InstanceKey),
    Name(LockKey),
}

/// The `--rename` flag, both sides of one kind.
enum RenameFlag {
    Instances(InstanceKey, InstanceKey),
    Names(LockKey, LockKey),
}

/// Runs `ridl lock`: plain allocation over every unit under `path`, or
/// the `--rename` and `--retire` edits over the one unit there.
pub fn run_lock(path: &Path, renames: &[String], retires: &[String]) -> ExitCode {
    // A flag that does not parse is refused before anything is compiled.
    let renames: Vec<RenameFlag> = match renames.iter().map(|flag| parse_rename(flag)).collect() {
        Ok(renames) => renames,
        Err(message) => return usage_error(&message),
    };
    let retires: Vec<FlagKey> = match retires
        .iter()
        .map(|flag| parse_flag_key("--retire", flag))
        .collect()
    {
        Ok(retires) => retires,
        Err(message) => return usage_error(&message),
    };
    let editing = !renames.is_empty() || !retires.is_empty();

    let mut db = RidlDatabase::default();
    let mut output = match ridlc::compile_workspace(&mut db, path) {
        Ok(output) => output,
        Err(err) => {
            eprintln!("error: {err}");
            return ExitCode::from(2);
        }
    };
    // A path inside a workspace member loads the whole workspace, and the
    // command reports on, allocates in and edits that member only (ADR-0024
    // decision 9, as ADR-0026 amends it).
    ridlc::retain_in_report_scope(
        &mut output.diagnostics,
        &output.sources,
        output.report_scope.as_deref(),
        &ridlc::remote_dirs(
            &output.remote_units,
            &ridl_core::Cache::user_default(),
            path,
        ),
    );
    // `ridl lock` applies no `[lints]` levels (ADR-0024 decision 8), so a lint
    // that is `allow` by default is left out of what it renders.
    drop_allowed_by_default(&mut output.diagnostics);
    let has_error = output
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == Severity::Error);
    if has_error && !(editing && only_lock_orphans(&output.diagnostics)) {
        eprint!("{}", render(&output.diagnostics, &output.sources));
        return ExitCode::FAILURE;
    }

    // A remote unit's `interfaces.lock` is its owner's: the command neither
    // allocates in it nor writes it, so its packages are left out here.
    let irs: Vec<v2::Package> = output
        .checked
        .into_iter()
        .filter(|checked| !ridlc::is_remote(&checked.ir, &output.remote_units))
        .map(|checked| checked.ir)
        .collect();
    let mut units = match locked_units(path, irs, output.report_scope.as_deref()) {
        Ok(units) => units,
        Err(err) => {
            eprintln!("error: {err}");
            return ExitCode::from(2);
        }
    };

    let instances = InstanceSide {
        numbering: &output.instances,
        dir: &output.instance_lock_dir,
        // Only a path that resolves to the root reads and writes the file.
        at_root: output.report_scope.is_none(),
    };
    let prefixed = units.len() > 1;
    let outcome = if editing {
        edit(path, &mut units, &instances, &renames, &retires)
    } else {
        allocate(path, &mut units).and_then(|()| allocate_instances(path, prefixed, &instances))
    };
    if let Err(code) = outcome {
        return code;
    }

    // The edits are the fix RIDL-409 and RSDL-314 name, so an edit run does
    // not render the diagnostic it was asked to resolve; everything else — a
    // warning, a lint — renders as `ridl check` renders it.
    let shown: Vec<Diagnostic> = if editing {
        output
            .diagnostics
            .into_iter()
            .filter(|diagnostic| {
                diagnostic.code != DiagCode::RIDL_409 && diagnostic.code != DiagCode::RSDL_314
            })
            .collect()
    } else {
        output.diagnostics
    };
    eprint!("{}", render(&shown, &output.sources));
    ExitCode::SUCCESS
}

/// `OLD=NEW`, both instance keys or both interface-or-component names.
fn parse_rename(flag: &str) -> Result<RenameFlag, String> {
    let Some((old, new)) = flag.split_once('=') else {
        return Err(format!("--rename takes OLD=NEW, got `{flag}`"));
    };
    match (
        parse_flag_key("--rename", old)?,
        parse_flag_key("--rename", new)?,
    ) {
        (FlagKey::Instance(old), FlagKey::Instance(new)) => Ok(RenameFlag::Instances(old, new)),
        (FlagKey::Name(old), FlagKey::Name(new)) => Ok(RenameFlag::Names(old, new)),
        _ => Err(format!(
            "--rename {flag}: a `/` on one side only; name two instances \
             (`component/instance`) or two names"
        )),
    }
}

/// One key as a flag spells it: an instance when it holds a `/`, else an
/// interface name, or `service:` and a dotted name, which may be a component.
fn parse_flag_key(flag: &str, text: &str) -> Result<FlagKey, String> {
    if text.contains('/') {
        text.parse()
            .map(FlagKey::Instance)
            .map_err(|InvalidLockKey(reason)| format!("{flag}: {reason}"))
    } else {
        text.parse()
            .map(FlagKey::Name)
            .map_err(|InvalidLockKey(reason)| format!("{flag}: {reason}"))
    }
}

/// A bad flag: exit 2 with the reason.
fn usage_error(message: &str) -> ExitCode {
    eprintln!("error: {message}");
    ExitCode::from(2)
}

/// Groups the checked packages by unit and pairs each unit with its manifest
/// directory and its lock as loaded. With a `scope` (the member directory a
/// path inside a workspace member names), only the units whose directory is
/// under it are kept.
fn locked_units(
    entry: &Path,
    irs: Vec<v2::Package>,
    scope: Option<&Path>,
) -> std::io::Result<Vec<LockedUnit>> {
    let mut db = RidlDatabase::default();
    let loaded = load_workspace(&mut db, entry)?;
    let handles = loaded.workspace.packages(&db).clone();
    // Units keep the order in which the workspace first lists one of their
    // packages.
    let mut by_unit: Vec<(String, Vec<v2::Package>)> = Vec::new();
    for ir in irs {
        let unit = v2::unit_of(&ir);
        match by_unit.iter_mut().find(|(name, _)| name == unit) {
            Some((_, packages)) => packages.push(ir),
            None => by_unit.push((unit.to_string(), vec![ir])),
        }
    }
    let mut units = Vec::new();
    for (name, packages) in by_unit {
        let Some(manifest_dir) = loaded.units.get(&name) else {
            return Err(std::io::Error::other(format!(
                "no directory is recorded for unit `{name}`"
            )));
        };
        let dir = if manifest_dir.as_os_str().is_empty() {
            PathBuf::from(".")
        } else {
            manifest_dir.clone()
        };
        if scope.is_some_and(|scope| !dir.starts_with(scope)) {
            continue;
        }
        // Every package of a unit carries the unit's one lock.
        let lock = handles
            .iter()
            .find(|handle| packages.iter().any(|ir| ir.name == *handle.name(&db)))
            .and_then(|handle| handle.lock(&db).as_ref().map(|lock| lock.lock.clone()))
            .unwrap_or_default();
        units.push(LockedUnit {
            name,
            dir,
            lock,
            packages,
        });
    }
    Ok(units)
}

/// Plain `ridl lock`: allocates every provisional shape of every package of
/// each unit, in the order of the numbers the checker showed as provisional —
/// byte order of the catalog name from `next` — so the number written is the
/// number the unit was checked with. A unit with nothing to allocate is not
/// written, so a file that already holds every declaration stays byte for
/// byte as it is.
fn allocate(entry: &Path, units: &mut [LockedUnit]) -> Result<(), ExitCode> {
    let prefixed = units.len() > 1;
    for unit in units.iter_mut() {
        let mut provisional: Vec<(u32, LockKey)> = unit
            .packages
            .iter()
            .flat_map(|package| {
                package
                    .shapes()
                    .filter(|shape| shape.interface.provisional)
                    .map(|shape| (shape.interface.number, shape_key(package, &shape)))
            })
            .collect();
        if provisional.is_empty() {
            continue;
        }
        provisional.sort();
        let prefix = if prefixed {
            format!("{}: ", relative_dir(entry, &unit.dir))
        } else {
            String::new()
        };
        let mut lines = Vec::with_capacity(provisional.len());
        for (_, key) in provisional {
            let number = unit
                .lock
                .allocate(key.clone())
                .expect("a provisional shape has no live entry: the checker read this lock");
            lines.push(format!("{prefix}allocated {key} {number}"));
        }
        write(unit)?;
        for line in lines {
            println!("{line}");
        }
    }
    Ok(())
}

/// What the instance lock edits and allocation read: the numbering the
/// checker computed, the directory of the root's `instances.lock`, and
/// whether `PATH` resolved to the root.
struct InstanceSide<'a> {
    numbering: &'a InstanceNumbering,
    dir: &'a Path,
    at_root: bool,
}

impl InstanceSide<'_> {
    fn file(&self) -> PathBuf {
        self.dir.join(instance_lock::FILE_NAME)
    }

    /// The lock as it is on disk: the empty table when there is no file.
    fn load(&self) -> Result<InstanceLock, ExitCode> {
        let fail = |message: String| {
            eprintln!("error: {message}");
            ExitCode::from(2)
        };
        let text = instance_lock::read(self.dir)
            .map_err(|err| fail(format!("cannot read {}: {err}", self.file().display())))?;
        match text {
            None => Ok(InstanceLock::default()),
            Some(text) => instance_lock::parse(&text)
                .map_err(|err| fail(format!("{}: {}", self.file().display(), err.message))),
        }
    }

    /// Writes the lock; an I/O failure is exit 2.
    fn write(&self, lock: &InstanceLock) -> Result<(), ExitCode> {
        instance_lock::write(self.dir, lock).map_err(|err| {
            eprintln!("error: cannot write {}: {err}", self.file().display());
            ExitCode::from(2)
        })
    }

    /// Whether the system has an instance of `component`.
    fn has_component(&self, component: &str) -> bool {
        self.numbering
            .numbers
            .keys()
            .any(|key| key.component == component)
    }

    /// Whether the system has `key` with no live entry.
    fn is_provisional(&self, key: &InstanceKey) -> bool {
        matches!(self.numbering.numbers.get(key), Some((_, true)))
    }
}

/// Plain `ridl lock`, instance half: gives every provisional instance its
/// number, in ascending order of the provisional number the checker showed, so
/// the number written is the number checked, and writes the root
/// `instances.lock` only when it allocates. Output lines are prefixed when the
/// interface lines are (`prefixed`). A path inside a member allocates nothing.
fn allocate_instances(
    entry: &Path,
    prefixed: bool,
    instances: &InstanceSide<'_>,
) -> Result<(), ExitCode> {
    if !instances.at_root {
        return Ok(());
    }
    let mut provisional: Vec<(i32, &InstanceKey)> = instances
        .numbering
        .numbers
        .iter()
        .filter(|(_, (_, provisional))| *provisional)
        .map(|(key, (number, _))| (*number, key))
        .collect();
    if provisional.is_empty() {
        return Ok(());
    }
    provisional.sort();
    let mut lock = instances.load()?;
    let prefix = if prefixed {
        format!("{}: ", relative_dir(entry, instances.dir))
    } else {
        String::new()
    };
    let mut lines = Vec::with_capacity(provisional.len());
    for (_, key) in provisional {
        let number = lock
            .allocate(key.clone())
            .map_err(|err| usage_error(&format!("{key}: {err}")))?;
        lines.push(format!("{prefix}allocated {key} {number}"));
    }
    instances.write(&lock)?;
    for line in lines {
        println!("{line}");
    }
    Ok(())
}

/// One edit, resolved against the locks.
enum Edit {
    RenameInstance(InstanceKey, InstanceKey),
    RenameComponent(String, String),
    RetireInstance(InstanceKey),
    RetireComponent(String),
    RenameInterface(LockKey, LockKey),
    RetireInterface(LockKey),
}

/// Resolves a name without a `/`: a component when `instance_lock` has an
/// entry under it (`include_retired` counts a retired one), else an interface;
/// both is exit 2 naming both files.
fn resolve_name(
    flag: &str,
    name: &LockKey,
    include_retired: bool,
    units: &[LockedUnit],
    instances: &InstanceSide<'_>,
    instance_lock: &InstanceLock,
) -> Result<Option<String>, ExitCode> {
    let LockKey::Interface(component) = name else {
        return Ok(None);
    };
    if !instance_lock.has_component(component, include_retired) {
        return Ok(None);
    }
    if let [unit] = units
        && unit.lock.live(name).is_some()
    {
        return Err(usage_error(&format!(
            "{flag} {name}: `{name}` is both a component in `{}` and an interface in `{}`; \
             edit its instances one by one as `{name}/INSTANCE`",
            instances.file().display(),
            unit.dir.join(interface_lock::FILE_NAME).display()
        )));
    }
    Ok(Some(component.clone()))
}

/// `--rename` and `--retire`. Interface edits need `entry` to resolve to the
/// one unit; instance and component edits need it to resolve to the root.
/// Every edit is validated and applied in memory first, so a refused flag
/// writes nothing; each file is written once, then each change is reported.
fn edit(
    entry: &Path,
    units: &mut [LockedUnit],
    instances: &InstanceSide<'_>,
    renames: &[RenameFlag],
    retires: &[FlagKey],
) -> Result<(), ExitCode> {
    let mut instance_lock = instances.load()?;
    let mut edits = Vec::new();
    for rename in renames {
        edits.push(match rename {
            RenameFlag::Instances(old, new) => Edit::RenameInstance(old.clone(), new.clone()),
            RenameFlag::Names(old, new) => {
                match resolve_name("--rename", old, true, units, instances, &instance_lock)? {
                    Some(old) => match new {
                        LockKey::Interface(new) => Edit::RenameComponent(old, new.clone()),
                        LockKey::Service(_) => {
                            return Err(usage_error(&format!(
                                "--rename {old}={new}: `{new}` is not a component name"
                            )));
                        }
                    },
                    None => Edit::RenameInterface(old.clone(), new.clone()),
                }
            }
        });
    }
    for retire in retires {
        edits.push(match retire {
            FlagKey::Instance(key) => Edit::RetireInstance(key.clone()),
            FlagKey::Name(name) => {
                match resolve_name("--retire", name, false, units, instances, &instance_lock)? {
                    Some(component) => Edit::RetireComponent(component),
                    None => Edit::RetireInterface(name.clone()),
                }
            }
        });
    }
    let touches_instances = edits.iter().any(|edit| {
        matches!(
            edit,
            Edit::RenameInstance(..)
                | Edit::RenameComponent(..)
                | Edit::RetireInstance(_)
                | Edit::RetireComponent(_)
        )
    });
    let touches_interfaces = edits
        .iter()
        .any(|edit| matches!(edit, Edit::RenameInterface(..) | Edit::RetireInterface(_)));
    if touches_instances && !instances.at_root {
        return Err(usage_error(&format!(
            "`{}` belongs to the workspace root: run `ridl lock <root>`",
            instance_lock::FILE_NAME
        )));
    }
    if touches_interfaces && units.len() != 1 {
        return Err(usage_error(&format!(
            "an interface edit changes one unit's `{}`, but `{}` holds {} units; name the unit \
             directory",
            interface_lock::FILE_NAME,
            entry.display(),
            units.len()
        )));
    }

    let mut lines = Vec::new();
    for edit in &edits {
        match edit {
            Edit::RenameInstance(old, new) => {
                if !instances.is_provisional(new) {
                    return Err(usage_error(&format!(
                        "--rename {old}={new}: `{new}` is not an instance without an entry in \
                         the system"
                    )));
                }
                let number = instance_lock
                    .rename(old, new.clone())
                    .map_err(|err| usage_error(&format!("--rename {old}={new}: {err}")))?;
                lines.push(format!("renamed {old} {new} {number}"));
            }
            Edit::RenameComponent(old, new) => {
                for entry in instance_lock
                    .entries
                    .iter()
                    .filter(|entry| !entry.retired && entry.key.component == *old)
                {
                    let target = InstanceKey {
                        component: new.clone(),
                        instance: entry.key.instance.clone(),
                    };
                    if !instances.is_provisional(&target) {
                        return Err(usage_error(&format!(
                            "--rename {old}={new}: `{target}` is not an instance without an \
                             entry in the system"
                        )));
                    }
                }
                let moved = instance_lock
                    .rename_component(old, new)
                    .map_err(|err| usage_error(&format!("--rename {old}={new}: {err}")))?;
                for (before, after, number) in moved {
                    lines.push(format!("renamed {before} {after} {number}"));
                }
            }
            Edit::RetireInstance(key) => {
                if instances.numbering.numbers.contains_key(key) {
                    return Err(usage_error(&format!(
                        "--retire {key}: `{key}` is still an instance of the system; remove it \
                         first, or keep the entry live"
                    )));
                }
                let number = instance_lock
                    .retire(key)
                    .map_err(|err| usage_error(&format!("--retire {key}: {err}")))?;
                lines.push(format!("retired {key} {number}"));
            }
            Edit::RetireComponent(component) => {
                if instances.has_component(component) {
                    return Err(usage_error(&format!(
                        "--retire {component}: `{component}` still has instances in the system; \
                         remove it first, or keep the entries live"
                    )));
                }
                let retired = instance_lock
                    .retire_component(component)
                    .map_err(|err| usage_error(&format!("--retire {component}: {err}")))?;
                for (key, number) in retired {
                    lines.push(format!("retired {key} {number}"));
                }
            }
            Edit::RenameInterface(old, new) => {
                let unit = &mut units[0];
                // The new key must be a declaration without an entry — the
                // shape the checker numbered provisionally. A declaration
                // with an entry, or a name nothing declares, is not a rename
                // target.
                if !is_provisional(&unit.packages, new) {
                    return Err(usage_error(&format!(
                        "--rename {old}={new}: `{new}` is not a declaration without an entry in \
                         unit `{}`",
                        unit.name
                    )));
                }
                let number = unit
                    .lock
                    .rename(old, new.clone())
                    .map_err(|err| usage_error(&format!("--rename {old}={new}: {err}")))?;
                lines.push(format!("renamed {old} {new} {number}"));
            }
            Edit::RetireInterface(key) => {
                let unit = &mut units[0];
                if is_declared(&unit.packages, key) {
                    return Err(usage_error(&format!(
                        "--retire {key}: `{key}` is still declared in unit `{}`; remove the \
                         declaration first, or keep the entry live",
                        unit.name
                    )));
                }
                let number = unit
                    .lock
                    .retire(key)
                    .map_err(|err| usage_error(&format!("--retire {key}: {err}")))?;
                lines.push(format!("retired {key} {number}"));
            }
        }
    }
    if touches_interfaces {
        write(&units[0])?;
    }
    if touches_instances {
        instances.write(&instance_lock)?;
    }
    for line in lines {
        println!("{line}");
    }
    Ok(())
}

/// Writes the unit's lock to its manifest directory; an I/O failure is exit 2.
fn write(unit: &LockedUnit) -> Result<(), ExitCode> {
    interface_lock::write(&unit.dir, &unit.lock).map_err(|err| {
        eprintln!(
            "error: cannot write {}: {err}",
            unit.dir.join(interface_lock::FILE_NAME).display()
        );
        ExitCode::from(2)
    })
}

/// The lock key of one shape of `package`: `service:` and the service's
/// dotted name for an inline shape, else the interface's name in the catalog
/// of its unit (lock design §3), such as `cluster.Speed` for an interface of
/// a subpackage.
pub(crate) fn shape_key(package: &v2::Package, shape: &v2::InterfaceShape<'_>) -> LockKey {
    if shape.is_inline() {
        LockKey::Service(shape.name.to_string())
    } else {
        LockKey::Interface(package.catalog_name(shape))
    }
}

/// Whether a package of the unit declares the shape `key` names, with or
/// without an entry.
fn is_declared(packages: &[v2::Package], key: &LockKey) -> bool {
    packages.iter().any(|package| {
        package
            .shapes()
            .any(|shape| shape_key(package, &shape) == *key)
    })
}

/// Whether a package of the unit declares the shape `key` names and it has no
/// entry — the checker gave it a provisional number.
fn is_provisional(packages: &[v2::Package], key: &LockKey) -> bool {
    packages.iter().any(|package| {
        package
            .shapes()
            .any(|shape| shape.interface.provisional && shape_key(package, &shape) == *key)
    })
}

/// The unit directory relative to `entry`, for the output prefix over a
/// workspace (plan decision PD-13): `.` for `entry` itself, and the directory
/// as the loader recorded it when it is not under `entry`.
fn relative_dir(entry: &Path, dir: &Path) -> String {
    match dir.strip_prefix(entry) {
        Ok(relative) if relative.as_os_str().is_empty() => ".".to_string(),
        Ok(relative) => relative.display().to_string(),
        Err(_) => dir.display().to_string(),
    }
}

/// Runs `ridl lock merge`: the git merge driver over `base`, `ours` and
/// `theirs`, writing the result to `ours`. An empty BASE — what git passes as
/// `%O` when both sides created the file — reads as `next 1` with no entries;
/// an empty OURS or THEIRS is a file that does not parse. The file is an
/// instance lock when the OURS text, else the THEIRS text, holds a line equal
/// to the instance lock header, and an interface lock otherwise.
pub fn run_lock_merge(base: &Path, ours: &Path, theirs: &Path, marker_size: usize) -> ExitCode {
    let texts = (read_text(base), read_text(ours), read_text(theirs));
    let (base_text, ours_text, theirs_text) = match texts {
        (Ok(base), Ok(ours), Ok(theirs)) => (base, ours, theirs),
        (Err(code), _, _) | (_, Err(code), _) | (_, _, Err(code)) => return code,
    };
    let has_header = |text: &str| text.lines().any(|line| line == instance_lock::HEADER);
    if has_header(&ours_text) || has_header(&theirs_text) {
        merge_texts::<InstanceKey>(
            (base, &base_text),
            (ours, &ours_text),
            (theirs, &theirs_text),
            marker_size,
            "RSDL-315",
        )
    } else {
        merge_texts::<LockKey>(
            (base, &base_text),
            (ours, &ours_text),
            (theirs, &theirs_text),
            marker_size,
            "RIDL-410",
        )
    }
}

/// The three-way merge of the three sides as tables of key type `K`, written
/// to OURS. `code` names the diagnostic a file with conflict markers draws.
fn merge_texts<K: TableKey>(
    base: (&Path, &str),
    ours: (&Path, &str),
    theirs: (&Path, &str),
    marker_size: usize,
    code: &str,
) -> ExitCode {
    let sides = (
        parse_side::<K>(base, true),
        parse_side::<K>(ours, false),
        parse_side::<K>(theirs, false),
    );
    let (base_lock, ours_lock, theirs_lock) = match sides {
        (Ok(base), Ok(ours), Ok(theirs)) => (base, ours, theirs),
        (Err(code), _, _) | (_, Err(code), _) | (_, _, Err(code)) => return code,
    };
    let ours_path = ours.0;
    let (text, exit) =
        match interface_lock::merge(&base_lock, &ours_lock, &theirs_lock, marker_size) {
            MergeOutcome::Clean(merged) => (merged.render(), ExitCode::SUCCESS),
            MergeOutcome::Conflict { text } => {
                eprintln!(
                    "error: {}: the two sides disagree; the disagreeing entries are between git \
                 conflict markers, and the file is malformed ({code}) until they are resolved \
                 by hand",
                    ours_path.display()
                );
                (text, ExitCode::FAILURE)
            }
        };
    if let Err(err) = std::fs::write(ours_path, text) {
        eprintln!("error: cannot write {}: {err}", ours_path.display());
        return ExitCode::from(2);
    }
    exit
}

/// The text of one side. A side that cannot be read is exit 2 with the
/// reason.
fn read_text(path: &Path) -> Result<String, ExitCode> {
    std::fs::read_to_string(path).map_err(|err| {
        eprintln!("error: cannot read {}: {err}", path.display());
        ExitCode::from(2)
    })
}

/// One side of the merge, parsed. A side that does not parse is exit 2 with
/// the reason: the driver merges tables, and a side that is not one is an
/// input it cannot answer over.
fn parse_side<K: TableKey>(
    (path, text): (&Path, &str),
    empty_is_default: bool,
) -> Result<LockTable<K>, ExitCode> {
    if empty_is_default && text.trim().is_empty() {
        return Ok(LockTable::default());
    }
    interface_lock::parse_table(text).map_err(|err| {
        eprintln!(
            "error: {}:{}: {}",
            path.display(),
            line_of(text, err.range),
            err.message
        );
        ExitCode::from(2)
    })
}

/// The 1-based line holding the start of `range` in `text`.
fn line_of(text: &str, range: TextRange) -> usize {
    let start = usize::from(range.start()).min(text.len());
    text[..start].matches('\n').count() + 1
}
