//! `ridl lock` — the one command that writes a unit's `interfaces.lock`
//! (lock design §5).
//!
//! Plain `ridl lock` allocates a number to every interface that has none —
//! every declared interface and every service's inline shape whose key has
//! no live entry — and writes each unit's own file, in its manifest
//! directory. It is the only form that allocates: a branch never allocates,
//! and the release recipe or the merge queue runs this on `main` (lock
//! design §4). `--rename OLD=NEW` and
//! `--retire NAME` rewrite one entry each, in place, and never allocate. They
//! are the fix RIDL-409 names, so they run with RIDL-409 present, and `PATH`
//! must then resolve to exactly one unit. Any other compile error exits 1
//! and writes nothing, whichever unit it is in (plan decision PD-6).
//!
//! The command compiles first, through [`ridlc::compile_workspace`] — no
//! network and no `ridl.lock` round trip — and reads each unit's directory
//! and its lock as loaded from a second [`load_workspace`] over the same
//! tree: `ridlc`'s output carries the checked IR and the diagnostics but no
//! package handle, and the two loads read the same files in the same order.
//! One line per change goes to stdout — `allocated Name N`,
//! `renamed Old New N`, `retired Name N` — prefixed with the unit
//! directory relative to `PATH` over more than one unit (plan decision
//! PD-13); diagnostics go to stderr (ADR-0010 decision 2). The exit codes
//! follow ADR-0010 decision 1: 0 when the file is written or there is nothing
//! to change, 1 on a diagnostic error over the source, 2 on a bad flag or a
//! path or I/O failure.
//!
//! `ridl lock merge BASE OURS THEIRS MARKER_SIZE` is the git merge driver for
//! the file (lock design §6): it reads the three sides, runs
//! [`interface_lock::merge`] — a three-way merge over entries matched by
//! number, with no file access — and writes the result to OURS. Exit 0 when
//! the merge is clean, 1 when entries disagree (OURS then holds git conflict
//! markers around only the disagreeing entries and is RIDL-410 until an
//! author resolves it), 2 when an input cannot be read or does not parse
//! (OURS is then left as it was).

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use ridl_core::diag::{DiagCode, Diagnostic, Severity, render};
use ridl_core::interface_lock::{self, InterfaceLock, InvalidLockKey, LockKey, MergeOutcome};
use ridl_core::lint::drop_allowed_by_default;
use ridl_core::{RidlDatabase, load_workspace};
use ridl_ir::v2;
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

/// Whether no error among `diagnostics` is anything but RIDL-409 — the one
/// error `--rename` and `--retire` run with, and the condition under which
/// `ridl check` still runs its desk check (lock design §4). True when there
/// is no error at all.
pub(crate) fn only_lock_orphans<'a>(diagnostics: impl IntoIterator<Item = &'a Diagnostic>) -> bool {
    diagnostics.into_iter().all(|diagnostic| {
        diagnostic.severity != Severity::Error || diagnostic.code == DiagCode::RIDL_409
    })
}

/// Runs `ridl lock`: plain allocation over every unit under `path`, or
/// the `--rename` and `--retire` edits over the one unit there.
pub fn run_lock(path: &Path, renames: &[String], retires: &[String]) -> ExitCode {
    // A flag that does not parse is refused before anything is compiled.
    let renames: Vec<(LockKey, LockKey)> =
        match renames.iter().map(|flag| parse_rename(flag)).collect() {
            Ok(renames) => renames,
            Err(message) => return usage_error(&message),
        };
    let retires: Vec<LockKey> = match retires
        .iter()
        .map(|flag| parse_key("--retire", flag))
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

    let irs: Vec<v2::Package> = output
        .checked
        .into_iter()
        .map(|checked| checked.ir)
        .collect();
    let mut units = match locked_units(path, irs, output.report_scope.as_deref()) {
        Ok(units) => units,
        Err(err) => {
            eprintln!("error: {err}");
            return ExitCode::from(2);
        }
    };

    let outcome = if editing {
        edit(path, &mut units, &renames, &retires)
    } else {
        allocate(path, &mut units)
    };
    if let Err(code) = outcome {
        return code;
    }

    // The edits are the fix RIDL-409 names, so an edit run does not render
    // the diagnostic it was asked to resolve; everything else — a warning, a
    // lint — renders as `ridl check` renders it.
    let shown: Vec<Diagnostic> = if editing {
        output
            .diagnostics
            .into_iter()
            .filter(|diagnostic| diagnostic.code != DiagCode::RIDL_409)
            .collect()
    } else {
        output.diagnostics
    };
    eprint!("{}", render(&shown, &output.sources));
    ExitCode::SUCCESS
}

/// `OLD=NEW`, both lock keys.
fn parse_rename(flag: &str) -> Result<(LockKey, LockKey), String> {
    let Some((old, new)) = flag.split_once('=') else {
        return Err(format!("--rename takes OLD=NEW, got `{flag}`"));
    };
    Ok((parse_key("--rename", old)?, parse_key("--rename", new)?))
}

/// One lock key as a flag spells it: an interface name, or `service:` and a
/// dotted name.
fn parse_key(flag: &str, text: &str) -> Result<LockKey, String> {
    text.parse()
        .map_err(|InvalidLockKey(reason)| format!("{flag}: {reason}"))
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

/// `--rename` and `--retire` over the one unit `entry` resolves to. Every
/// edit is validated and applied in memory first, so a refused flag writes
/// nothing; the file is written once, then each change is reported.
fn edit(
    entry: &Path,
    units: &mut [LockedUnit],
    renames: &[(LockKey, LockKey)],
    retires: &[LockKey],
) -> Result<(), ExitCode> {
    let [unit] = units else {
        return Err(usage_error(&format!(
            "`--rename` and `--retire` edit one unit's `{}`, but `{}` holds {} units; name the \
             unit directory",
            interface_lock::FILE_NAME,
            entry.display(),
            units.len()
        )));
    };
    let mut lines = Vec::new();
    for (old, new) in renames {
        // The new key must be a declaration without an entry — the shape the
        // checker numbered provisionally. A declaration with an entry, or a
        // name nothing declares, is not a rename target.
        if !is_provisional(&unit.packages, new) {
            return Err(usage_error(&format!(
                "--rename {old}={new}: `{new}` is not a declaration without an entry in unit `{}`",
                unit.name
            )));
        }
        let number = unit
            .lock
            .rename(old, new.clone())
            .map_err(|err| usage_error(&format!("--rename {old}={new}: {err}")))?;
        lines.push(format!("renamed {old} {new} {number}"));
    }
    for key in retires {
        if is_declared(&unit.packages, key) {
            return Err(usage_error(&format!(
                "--retire {key}: `{key}` is still declared in unit `{}`; remove the declaration \
                 first, or keep the entry live",
                unit.name
            )));
        }
        let number = unit
            .lock
            .retire(key)
            .map_err(|err| usage_error(&format!("--retire {key}: {err}")))?;
        lines.push(format!("retired {key} {number}"));
    }
    write(unit)?;
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
/// an empty OURS or THEIRS is a file that does not parse.
pub fn run_lock_merge(base: &Path, ours: &Path, theirs: &Path, marker_size: usize) -> ExitCode {
    let sides = (
        read_side(base, true),
        read_side(ours, false),
        read_side(theirs, false),
    );
    let (base, ours_lock, theirs_lock) = match sides {
        (Ok(base), Ok(ours), Ok(theirs)) => (base, ours, theirs),
        (Err(code), _, _) | (_, Err(code), _) | (_, _, Err(code)) => return code,
    };
    let (text, code) = match interface_lock::merge(&base, &ours_lock, &theirs_lock, marker_size) {
        MergeOutcome::Clean(merged) => (merged.render(), ExitCode::SUCCESS),
        MergeOutcome::Conflict { text } => {
            eprintln!(
                "error: {}: the two sides disagree; the disagreeing entries are between git \
                 conflict markers, and the file is malformed (RIDL-410) until they are resolved \
                 by hand",
                ours.display()
            );
            (text, ExitCode::FAILURE)
        }
    };
    if let Err(err) = std::fs::write(ours, text) {
        eprintln!("error: cannot write {}: {err}", ours.display());
        return ExitCode::from(2);
    }
    code
}

/// One side of the merge, parsed. A side that cannot be read, or that does
/// not parse, is exit 2 with the reason: the driver merges tables, and a side
/// that is not one is an input it cannot answer over.
fn read_side(path: &Path, empty_is_default: bool) -> Result<InterfaceLock, ExitCode> {
    let text = std::fs::read_to_string(path).map_err(|err| {
        eprintln!("error: cannot read {}: {err}", path.display());
        ExitCode::from(2)
    })?;
    if empty_is_default && text.trim().is_empty() {
        return Ok(InterfaceLock::default());
    }
    interface_lock::parse(&text).map_err(|err| {
        eprintln!(
            "error: {}:{}: {}",
            path.display(),
            line_of(&text, err.range),
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
