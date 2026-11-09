//! Per-unit catalog histories, judged with each baseline's own remote evidence.

use std::collections::BTreeMap;
use std::path::Path;
use std::process::ExitCode;

use ridl_core::RidlDatabase;
use ridl_core::catalog_history::{CatalogHistory, FILE_SUFFIX, parse};
use ridl_diff::{Change, Verdict};
use ridl_ir::catalog_hash::{catalog_hash, reachable_decls};
use ridl_ir::v2::{Package, unit_of};
use ridlc::baseline_context::RemoteContext;

use crate::{default_baseline_dir, report_diff_side_error};

/// Computes compatible hashes from the build's own compile. Historical scopes
/// never borrow current remote packages, including during canonical hashing.
pub(crate) fn compatible_catalogs(
    db: &mut RidlDatabase,
    entry: &Path,
    checked: &[&Package],
    std: &Package,
    names_imports: bool,
    current: &[&Package],
) -> Result<BTreeMap<String, Vec<[u8; 32]>>, ExitCode> {
    let published = default_baseline_dir(entry);
    let mut compatible = BTreeMap::new();
    if !holds_history_and_snapshot(&published)? {
        return Ok(compatible);
    }
    let old = ridlc::load_diff_side(db, &published, &[]).map_err(report_diff_side_error)?;
    let old_refs: Vec<_> = old.packages.iter().collect();
    let old_context = ridlc::diff_side_context(&old);
    let old_context: Vec<_> = old_context.iter().collect();
    let new_context: Vec<_> = std::iter::once(std)
        .chain(current.iter().copied())
        .collect();
    let judgment = Judgment::new(entry, &old_refs, checked, &old_context, &new_context);
    let packages: Vec<_> = checked.iter().chain(current).copied().collect();
    let scope = ridlc::catalog_scope(&packages, Some(std));
    let old_remotes = old
        .remote_context
        .as_ref()
        .map_or(&[][..], |context| context.packages.as_slice());
    let published_scope = complete_scope(&old_refs, std, old_remotes);
    let verdicts = ridl_diff::UnitVerdicts::from_refs(&judgment.report, &published_scope, &scope);
    for unit in shaped_units(checked.iter().copied()) {
        if !old.packages.iter().any(|package| unit_of(package) == unit) {
            continue;
        }
        let Some(history) = read_history_if_present(&published, unit)? else {
            continue;
        };
        let unknown = judgment.unknown(unit, &published_scope, &scope);
        if unknown {
            eprintln!(
                "warning: cannot establish compatibility for unit `{unit}`: historical remote context is missing or unusable; omitting its compatible catalogs. Repair the baseline from the exact historical source and verified pins with `ridl baseline HISTORICAL_SOURCE --repair-context`, or preserve it and start a new history with `ridl baseline CURRENT_SOURCE --reset-history`."
            );
        }
        let current_hash = catalog_hash(unit, &scope);
        let carried = !unknown
            && !judgment.podbuf_break(unit, &published_scope, &scope)
            && history.hashes.first() == Some(&catalog_hash(unit, &published_scope))
            && (history.hashes.first() == Some(&current_hash)
                || verdicts.verdict(unit) != Verdict::Breaking);
        compatible.insert(
            unit.to_string(),
            if carried {
                history
                    .hashes
                    .into_iter()
                    .filter(|hash| *hash != current_hash)
                    .collect()
            } else {
                Vec::new()
            },
        );
    }
    if names_imports && !compatible.is_empty() {
        eprintln!(
            "note: the workspace names `[imports]`; a remote unit's catalog is published by its owner, so a unit's list covers the workspace's own packages only"
        );
    }
    Ok(compatible)
}

/// Stages complete canonical chains. Returns whether any published unit has
/// a proved break, including units without an interface. Missing remote evidence
/// for a judged encoding requires explicit repair or reset and refuses normal
/// publication before anything is replaced.
pub(crate) fn write_catalog_histories(
    staging: &Path,
    published: &Path,
    old: &[Package],
    fresh: &[Package],
    current: &RemoteContext,
    entry: &Path,
) -> Result<bool, ExitCode> {
    let std = ridlc::std_ir();
    let old_remote = crate::baseline::read_context(published, old)?;
    let old_remote = old_remote.as_ref().map(|context| context.remote_context());
    let old_remotes = old_remote
        .as_ref()
        .map_or(&[][..], |context| context.packages.as_slice());
    let old_refs: Vec<_> = old.iter().collect();
    let fresh_refs: Vec<_> = fresh.iter().collect();
    let old_context: Vec<_> = std::iter::once(&std).chain(old_remotes.iter()).collect();
    let new_context: Vec<_> = std::iter::once(&std)
        .chain(current.packages.iter())
        .collect();
    let judgment = Judgment::new(entry, &old_refs, &fresh_refs, &old_context, &new_context);
    let scope = complete_scope(&fresh_refs, &std, &current.packages);
    let published_scope = complete_scope(&old_refs, &std, old_remotes);
    let verdicts = ridl_diff::UnitVerdicts::from_refs(&judgment.report, &published_scope, &scope);
    let published_units: std::collections::BTreeSet<_> = old.iter().map(unit_of).collect();
    for unit in &published_units {
        if let Some(side) = judgment.unknown_side(unit, &published_scope, &scope) {
            if side == ridl_diff::ContextSide::New {
                eprintln!(
                    "error: cannot publish unit `{unit}`: the current workspace's remote evidence is missing or unusable; the prior baseline is unchanged. Restore usable current remote evidence before publishing."
                );
                return Err(ExitCode::from(2));
            }
            eprintln!(
                "error: cannot publish unit `{unit}`: historical remote context is missing or unusable; the prior baseline is unchanged. Repair with `ridl baseline HISTORICAL_SOURCE --repair-context --out {}` using exact historical local snapshots and verified remote pins, or start a new history with `ridl baseline CURRENT_SOURCE --reset-history --out {}`.",
                published.display(),
                published.display()
            );
            return Err(ExitCode::from(2));
        }
    }
    let preserve = published_units
        .iter()
        .any(|unit| verdicts.verdict(unit) == Verdict::Breaking);
    for unit in shaped_units(fresh.iter()) {
        let published_before = old.iter().any(|package| unit_of(package) == unit);
        let earlier = if published_before {
            read_history(published, unit)?
        } else {
            CatalogHistory::default()
        };
        let breaking = verdicts.verdict(unit) == Verdict::Breaking;
        let new_hash = catalog_hash(unit, &scope);
        let carried = published_before
            && !judgment.podbuf_break(unit, &published_scope, &scope)
            && earlier.hashes.first() == Some(&catalog_hash(unit, &published_scope))
            && (earlier.hashes.first() == Some(&new_hash) || !breaking);
        let mut history = if carried {
            earlier
        } else {
            CatalogHistory::default()
        };
        history.push_front(new_hash);
        write_history(staging, unit, &history)?;
    }
    Ok(preserve)
}

/// Recovery starts a new chain at the current canonical hash. It does not
/// read or certify any earlier catalog-chain metadata.
pub(crate) fn write_fresh_histories(
    staging: &Path,
    fresh: &[Package],
    current: &RemoteContext,
) -> Result<(), ExitCode> {
    let std = ridlc::std_ir();
    let refs: Vec<_> = fresh.iter().collect();
    let scope = complete_scope(&refs, &std, &current.packages);
    for unit in shaped_units(fresh.iter()) {
        let mut history = CatalogHistory::default();
        history.push_front(catalog_hash(unit, &scope));
        write_history(staging, unit, &history)?;
    }
    Ok(())
}

fn complete_scope<'a>(
    local: &[&'a Package],
    std: &'a Package,
    remotes: &'a [Package],
) -> Vec<&'a Package> {
    let packages: Vec<_> = local.iter().copied().chain(remotes.iter()).collect();
    ridlc::catalog_scope(&packages, Some(std))
}

struct Judgment {
    report: ridl_diff::DiffReport,
    podbuf_report: ridl_diff::DiffReport,
    issues: Vec<(Change, ridl_diff::ContextSide)>,
}
impl Judgment {
    fn new(
        entry: &Path,
        old: &[&Package],
        new: &[&Package],
        old_context: &[&Package],
        new_context: &[&Package],
    ) -> Self {
        let encodings = crate::root_manifest_encodings(entry);
        let encodings: Vec<_> = encodings.iter().map(String::as_str).collect();
        let (report, issues) = ridl_diff::diff_refs_with_contexts_partial(
            old,
            new,
            old_context,
            new_context,
            &encodings,
        );
        let mut podbuf_report = report.clone();
        podbuf_report.changes.retain(|change| {
            matches!(
                change.category,
                ridl_diff::Category::PodbufInlineAppend
                    | ridl_diff::Category::PodbufRemoteInlineGrowth
                    | ridl_diff::Category::PodbufWidthChange
                    | ridl_diff::Category::PodbufStrictEnumAppend
            )
        });
        Self {
            report,
            podbuf_report,
            issues: if encodings
                .iter()
                .any(|encoding| matches!(*encoding, "podbuf" | "flatbuffers" | "proto3"))
            {
                // These records use only the change path accessors for attribution.
                issues
                    .into_iter()
                    .filter(|issue| {
                        encodings.contains(&"podbuf")
                            || issue.reason == ridl_diff::ContextIssueReason::Missing
                    })
                    .map(|issue| {
                        (
                            Change {
                                path: issue.path,
                                category: ridl_diff::Category::PodbufRemoteInlineGrowth,
                                verdict: Verdict::Breaking,
                                before: None,
                                after: None,
                            },
                            issue.side,
                        )
                    })
                    .collect()
            } else {
                Vec::new()
            },
        }
    }

    fn podbuf_break(&self, unit: &str, old: &[&Package], new: &[&Package]) -> bool {
        ridl_diff::UnitVerdicts::from_refs(&self.podbuf_report, old, new).verdict(unit)
            == Verdict::Breaking
    }

    fn unknown(&self, unit: &str, old: &[&Package], new: &[&Package]) -> bool {
        self.unknown_side(unit, old, new).is_some()
    }

    fn unknown_side(
        &self,
        unit: &str,
        old: &[&Package],
        new: &[&Package],
    ) -> Option<ridl_diff::ContextSide> {
        let reached_old = reachable_decls(unit, old);
        let reached_new = reachable_decls(unit, new);
        self.issues.iter().find_map(|(issue, side)| {
            let package = issue.package();
            let declaration = issue.declaration().map(|name| format!("{package}.{name}"));
            (old.iter()
                .chain(new)
                .any(|p| p.name == package && unit_of(p) == unit)
                || declaration.is_some_and(|name| {
                    reached_old.contains_key(&name) || reached_new.contains_key(&name)
                }))
            .then_some(*side)
        })
    }
}

/// Whether `dir` holds at least one entry named `<unit>.catalogs` and at
/// least one `.ir.json` snapshot, from one listing of `dir`. An entry named
/// like a history file counts whatever it is, so that
/// [`read_history_if_present`] reports it; a directory entry that cannot be
/// read is an error (exit 2).
fn holds_history_and_snapshot(dir: &Path) -> Result<bool, ExitCode> {
    if !dir.is_dir() {
        return Ok(false);
    }
    let entries = ridlc::diff_side::files_matching_strict(dir, |_| true).map_err(|err| {
        eprintln!("error: cannot read {}: {err}", dir.display());
        ExitCode::from(2)
    })?;
    Ok(entries.iter().any(|path| has_history_name(path))
        && entries
            .iter()
            .any(|path| ridlc::diff_side::is_ir_json(path)))
}

/// Whether `path` is named like a `<unit>.catalogs` file.
fn has_history_name(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.ends_with(FILE_SUFFIX))
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
fn shaped_units<'a>(
    packages: impl Iterator<Item = &'a Package>,
) -> std::collections::BTreeSet<&'a str> {
    packages
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

#[cfg(test)]
mod tests {
    use super::*;
    use ridl_ir::v2::{Backing, PrimitiveType, TypeDef, backing, decl};

    #[test]
    fn a_proved_type_only_break_requires_preserving_prior_artifacts() {
        let root = std::env::temp_dir().join(format!(
            "ridl-type-only-preservation-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(root.join("published")).unwrap();
        std::fs::create_dir(root.join("staging")).unwrap();
        std::fs::write(
            root.join("ridl.toml"),
            "[package]\nname = \"types\"\nversion = \"1.0.0\"\n",
        )
        .unwrap();
        let fresh = Package {
            name: "types".to_string(),
            unit: "types".to_string(),
            ..Default::default()
        };
        let mut old = fresh.clone();
        old.decls.push(ridl_ir::v2::Decl {
            name: "Value".to_string(),
            kind: Some(decl::Kind::TypeDef(TypeDef {
                backing: Some(Backing {
                    kind: Some(backing::Kind::Primitive(PrimitiveType::Integer as i32)),
                }),
                ..Default::default()
            })),
            ..Default::default()
        });
        let preserve = write_catalog_histories(
            &root.join("staging"),
            &root.join("published"),
            &[old],
            &[fresh],
            &RemoteContext {
                packages: Vec::new(),
                pins: BTreeMap::new(),
            },
            &root,
        )
        .unwrap();
        assert!(
            preserve,
            "a removed declaration is a proved break even without an interface"
        );
        assert_eq!(std::fs::read_dir(root.join("staging")).unwrap().count(), 0);
        std::fs::remove_dir_all(root).unwrap();
    }
}
