//! The `<unit>.catalogs` files `ridl baseline` publishes beside the snapshots:
//! for every unit with an interface shape, the catalog hash of the baseline
//! being published, then the hashes of the earlier baselines a consumer built
//! against still works with, back to the last breaking change.

use std::collections::BTreeMap;
use std::path::Path;
use std::process::ExitCode;

use ridl_core::RidlDatabase;
use ridl_core::catalog_history::{CatalogHistory, FILE_SUFFIX, parse};
use ridl_diff::Verdict;
use ridl_ir::catalog_hash::catalog_hash;
use ridl_ir::v2::{Package, unit_of};

use crate::{default_baseline_dir, report_diff_side_error};

/// The earlier catalogs each unit of the workspace at `entry` is compatible
/// with, per unit name, for `ridl build` to write into the unit's descriptor
/// and codegen request.
///
/// The chain is read from `.ridl/baseline/` at the workspace root
/// ([`default_baseline_dir`]). With no such directory, or one holding no
/// `.ir.json` snapshot, there is no baseline and no unit has a list, as
/// `ridl check` treats it. Otherwise the published snapshots are compared
/// with the workspace, and each unit with an interface shape, a published
/// snapshot and a published `<unit>.catalogs` file gets the file's hashes
/// less the unit's current catalog hash when the unit's verdict is
/// compatible or identical, or when the current catalog hash is the file's
/// first hash: the catalog did not change, so every hash the file lists still
/// applies. Otherwise the unit's list is empty. The hashes are read from the
/// file and never recomputed from a snapshot; a file that cannot be read is
/// an error (exit 2), and so is a baseline that cannot be loaded. A build
/// with an error diagnostic gets no list.
pub(crate) fn compatible_catalogs(
    db: &mut RidlDatabase,
    entry: &Path,
) -> Result<BTreeMap<String, Vec<[u8; 32]>>, ExitCode> {
    let published = default_baseline_dir(entry);
    let mut compatible = BTreeMap::new();
    if !has_snapshot(&published)? {
        return Ok(compatible);
    }
    let old = ridlc::load_diff_side(db, &published, &[])
        .map_err(report_diff_side_error)?
        .packages;
    let Ok(output) = ridlc::compile_workspace(db, entry) else {
        return Ok(compatible);
    };
    if output
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == ridl_core::diag::Severity::Error)
    {
        return Ok(compatible);
    }
    let current: Vec<Package> = output
        .checked
        .into_iter()
        .map(|package| package.ir)
        .collect();
    let std = output.std_ir;
    let report = ridl_diff::diff_sets_in(&old, &current, std::slice::from_ref(&std));
    let current_refs: Vec<&Package> = current.iter().collect();
    let scope = ridlc::catalog_scope(&current_refs, Some(&std));
    for unit in shaped_units(&current) {
        // A unit with no published snapshot has no earlier baseline, so a
        // history file left under its name is not read, whatever the verdict.
        if !old.iter().any(|package| unit_of(package) == unit) {
            continue;
        }
        let Some(history) = read_history_if_present(&published, unit)? else {
            continue;
        };
        let current_hash = catalog_hash(unit, &scope);
        let carried = history.hashes.first() == Some(&current_hash)
            || matches!(
                ridl_diff::unit_verdict(&report, unit, &old, &current),
                Verdict::Compatible | Verdict::Identical
            );
        let hashes = if carried {
            history
                .hashes
                .into_iter()
                .filter(|hash| *hash != current_hash)
                .collect()
        } else {
            Vec::new()
        };
        compatible.insert(unit.to_string(), hashes);
    }
    Ok(compatible)
}

/// Writes `staging/<unit>.catalogs` for every unit of the snapshots in
/// `staging` that has at least one interface shape.
///
/// Each file holds the unit's catalog hash, computed over the staged
/// snapshots as `ridl build --emit catalog` computes it. When the unit's
/// verdict between the snapshots in `published` and the staged ones is
/// compatible or identical, or when the new catalog hash is the first hash of
/// `published/<unit>.catalogs` (the catalog did not change), the hashes of
/// that file follow, read as they are and never recomputed from a snapshot.
/// Otherwise the chain starts again from the one new hash. A hash is listed
/// once.
///
/// A unit with no snapshot in `published` has nothing to compare against, so
/// it starts a chain, and a history file left under its name is not read. A
/// published history file of a unit with a published snapshot that cannot be
/// read is an error (exit 2), and the caller publishes nothing.
pub(crate) fn write_catalog_histories(
    db: &mut RidlDatabase,
    staging: &Path,
    published: &Path,
) -> Result<(), ExitCode> {
    let fresh = ridlc::load_diff_side(db, staging, &[])
        .map_err(report_diff_side_error)?
        .packages;
    let old = if has_snapshot(published)? {
        ridlc::load_diff_side(db, published, &[])
            .map_err(report_diff_side_error)?
            .packages
    } else {
        Vec::new()
    };
    let std = ridlc::std_ir();
    let report = (!old.is_empty())
        .then(|| ridl_diff::diff_sets_in(&old, &fresh, std::slice::from_ref(&std)));
    let fresh_refs: Vec<&Package> = fresh.iter().collect();
    let scope = ridlc::catalog_scope(&fresh_refs, Some(&std));
    for unit in shaped_units(&fresh) {
        // A unit with no published snapshot has no earlier baseline, so a
        // history file left under its name is not carried, whatever the verdict.
        let published_before = old.iter().any(|package| unit_of(package) == unit);
        // The published file is read whatever the verdict, so a file that
        // cannot be read refuses the publication even when the chain restarts.
        let earlier = if published_before {
            read_history(published, unit)?
        } else {
            CatalogHistory::default()
        };
        let new_hash = catalog_hash(unit, &scope);
        let carried = published_before
            && (earlier.hashes.first() == Some(&new_hash)
                || report.as_ref().is_some_and(|report| {
                    matches!(
                        ridl_diff::unit_verdict(report, unit, &old, &fresh),
                        Verdict::Compatible | Verdict::Identical
                    )
                }));
        let mut history = if carried {
            earlier
        } else {
            CatalogHistory::default()
        };
        history.push_front(new_hash);
        write_history(staging, unit, &history)?;
    }
    Ok(())
}

/// Whether `dir` holds at least one `.ir.json` snapshot.
fn has_snapshot(dir: &Path) -> Result<bool, ExitCode> {
    if !dir.is_dir() {
        return Ok(false);
    }
    let files = ridlc::diff_side::ir_json_files(dir).map_err(|err| {
        eprintln!("error: cannot read {}: {err}", dir.display());
        ExitCode::from(2)
    })?;
    Ok(!files.is_empty())
}

/// The history `published/<unit>.catalogs` holds, or an empty one when the
/// file does not exist.
fn read_history(published: &Path, unit: &str) -> Result<CatalogHistory, ExitCode> {
    Ok(read_history_if_present(published, unit)?.unwrap_or_default())
}

/// The history `published/<unit>.catalogs` holds, or `None` when the file
/// does not exist. A file that cannot be read or parsed is an error (exit 2)
/// that names the file, and the line when the file is malformed.
fn read_history_if_present(
    published: &Path,
    unit: &str,
) -> Result<Option<CatalogHistory>, ExitCode> {
    let path = published.join(format!("{unit}{FILE_SUFFIX}"));
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(err) => {
            eprintln!("error: cannot read {}: {err}", path.display());
            return Err(ExitCode::from(2));
        }
    };
    parse(&text).map(Some).map_err(|err| {
        eprintln!(
            "error: {}: {err}; repair the file, or remove it to start the unit's chain again",
            path.display()
        );
        ExitCode::from(2)
    })
}

/// The units of `packages` that hold at least one interface shape, in name
/// order — the units `ridl build --emit catalog` writes a descriptor for.
fn shaped_units(packages: &[Package]) -> std::collections::BTreeSet<&str> {
    packages
        .iter()
        .filter(|package| package.shapes().next().is_some())
        .map(unit_of)
        .collect()
}

fn write_history(staging: &Path, unit: &str, history: &CatalogHistory) -> Result<(), ExitCode> {
    let path = staging.join(format!("{unit}{FILE_SUFFIX}"));
    std::fs::write(&path, history.render()).map_err(|err| {
        eprintln!("error: cannot write {}: {err}", path.display());
        ExitCode::from(2)
    })
}
