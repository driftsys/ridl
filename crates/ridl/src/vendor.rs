//! `ridl vendor` — copy every remote unit of the workspace's `[imports]`
//! into `vendor/` at the manifest root (ADR-0002 §7), so that a `--frozen`
//! build needs no network and no cache.
//!
//! The command loads the workspace from the manifest root with
//! [`RemoteAccess::refresh`]: each unit is read from the cache or fetched,
//! never from `vendor/`, which is what the command writes, and `ridl.lock`
//! is rewritten as a non-frozen `ridl check` rewrites it. When the load and
//! the check draw no error, [`write_vendor`] brings `vendor/` to exactly the
//! loaded units. One line per change goes to stdout, diagnostics go to
//! stderr (ADR-0010 decision 2). The exit codes follow ADR-0010 decision 1:
//! 0 when `vendor/` was written or there was nothing to change, 1 on any
//! diagnostic error, 2 on a missing path, a path with no manifest, a fetch
//! failure (MANI-101), or an I/O failure. Nothing in `vendor/` changes on
//! exit 1 or on a fetch failure.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use ridl_core::diag::{DiagCode, Severity, render};
use ridl_core::lint::drop_allowed_by_default;
use ridl_core::{Cache, RemoteAccess, RidlDatabase, VENDOR_DIR, find_root, write_vendor};

/// Runs `ridl vendor` over the manifest root that governs `path`.
pub fn run(path: &Path) -> ExitCode {
    // A missing path is refused before root discovery, which would otherwise
    // walk up from it and vendor the project around it.
    if !path.exists() {
        eprintln!("error: `{}` does not exist", path.display());
        return ExitCode::from(2);
    }
    let Some(root) = manifest_root(path) else {
        eprintln!(
            "error: no `ridl.toml` found at or above `{}`",
            path.display()
        );
        return ExitCode::from(2);
    };
    let cache = Cache::user_default();
    let mut db = RidlDatabase::default();
    let mut output = match ridlc::compile_workspace_with_access(
        &mut db,
        &root,
        &[],
        RemoteAccess::refresh(&cache),
    ) {
        Ok(output) => output,
        Err(err) => {
            eprintln!("error: {err}");
            return ExitCode::from(2);
        }
    };
    // `ridl vendor` applies no `[lints]` levels (ADR-0024 decision 8), so a
    // lint that is `allow` by default is left out of what it renders.
    drop_allowed_by_default(&mut output.diagnostics);
    let has_error = |code: Option<DiagCode>| {
        output.diagnostics.iter().any(|diagnostic| match code {
            Some(code) => diagnostic.code == code,
            None => diagnostic.severity == Severity::Error,
        })
    };
    // A fetch failure means the command could not answer (exit 2); any other
    // error is a negative answer (exit 1). Nothing is written in either case.
    let refused = if has_error(Some(DiagCode::MANI_101)) {
        Some(ExitCode::from(2))
    } else if has_error(None) {
        Some(ExitCode::FAILURE)
    } else {
        None
    };
    if let Some(code) = refused {
        eprint!("{}", render(&output.diagnostics, &output.sources));
        return code;
    }
    let report = match write_vendor(&root, &output.remote_units) {
        Ok(report) => report,
        Err(err) => {
            eprint!("{}", render(&output.diagnostics, &output.sources));
            eprintln!(
                "error: cannot write `{}`: {err}",
                root.join(VENDOR_DIR).display()
            );
            return ExitCode::from(2);
        }
    };
    // The load saw the directory of a unit no longer imported as a stray
    // source tree (MANI-018); the write has removed it, so that warning
    // describes nothing that is left.
    let removed: Vec<PathBuf> = report
        .removed
        .iter()
        .map(|name| root.join(VENDOR_DIR).join(name))
        .collect();
    output.diagnostics.retain(|diagnostic| {
        diagnostic.code != DiagCode::MANI_018
            || !output
                .sources
                .path(diagnostic.primary.file)
                .is_some_and(|path| removed.iter().any(|dir| Path::new(path).starts_with(dir)))
    });
    eprint!("{}", render(&output.diagnostics, &output.sources));
    for name in &report.written {
        println!("vendored {name}");
    }
    for name in &report.removed {
        println!("removed {name}");
    }
    if report.written.is_empty() && report.removed.is_empty() {
        println!("{VENDOR_DIR}/ is up to date; nothing to change");
    }
    ExitCode::SUCCESS
}

/// The manifest root that governs `path` (ADR-0002 §4 root discovery), or
/// `None` when no `ridl.toml` is at or above it.
fn manifest_root(path: &Path) -> Option<PathBuf> {
    if path.is_file() {
        path.parent().and_then(find_root)
    } else {
        find_root(path)
    }
}
