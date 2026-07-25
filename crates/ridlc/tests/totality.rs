//! The totality sweep (docs/ROADMAP.md epic E2.11b).
//!
//! `ridlc::compile` documents a hard invariant: "The function is total: it
//! never panics." Malformed source is the only way to test it, so the
//! malformed programs are first-class corpus members, kept in
//! `tests/malformed/` — outside `tests/corpus/`, because they are single files
//! with no manifest and the corpus glob would not know what to do with them.
//!
//! Two properties are asserted over every file in the sweep:
//!
//! 1. **`compile` returns.** The call is wrapped in `catch_unwind` so one
//!    panicking input names itself instead of aborting the run, and so every
//!    remaining input is still exercised.
//! 2. **No nameless declaration reaches the IR, and both backends survive it.**
//!    A member the parser recovered without a name keeps its ordinal slot but
//!    must not lower: an empty `Decl.name` is not an identifier any backend can
//!    emit. This is the defect issue #158 fixed, and the assertion is the one
//!    that would fail again if the fix were reverted — at both interaction
//!    sites, an `interface` body and a service's inline shape.
//!
//! The well-formed corpus entries are swept too. They cannot provoke the
//! recovery paths, but sweeping them costs nothing and means every `.ridl` and
//! `.typl` file in this crate's test data is covered by the invariant.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};

/// Every `.ridl` and `.typl` file under `dir`, recursively, in a deterministic
/// order.
fn source_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        let entries = std::fs::read_dir(&current).unwrap_or_else(|err| {
            panic!("test data directory {} is readable: {err}", dir.display())
        });
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if matches!(
                path.extension().and_then(|ext| ext.to_str()),
                Some("ridl" | "typl")
            ) {
                files.push(path);
            }
        }
    }
    files.sort();
    files
}

/// The interaction declarations of a compiled package: those inside named
/// interfaces and those inside a service's inline shape. The second half is
/// the one a consumer that walks `package.interfaces` alone silently misses,
/// which is why the walk is `Package::shapes`.
fn interactions(package: &ridl_ir::v2::Package) -> Vec<&ridl_ir::v2::Decl> {
    package
        .shapes()
        .flat_map(|shape| shape.interface.interactions.iter())
        .collect()
}

/// Compiles `path` and checks the two invariants, returning a failure
/// description or `None`.
fn sweep_one(path: &Path) -> Option<String> {
    let text = std::fs::read_to_string(path).ok()?;
    let name = path.display().to_string();

    let outcome = catch_unwind(AssertUnwindSafe(|| {
        let output = ridlc::compile(&name, &text);
        // Both backends run over whatever the checker produced. `compile`
        // already runs the Rust backend; the TypeScript backend is driven here
        // so the sweep covers it too. Both return errors as values, so a panic
        // is the only way either can fail this.
        let _ = ridl_backend_ts::generate(&output.package);
        output.package
    }));

    let package = match outcome {
        Ok(package) => package,
        Err(_) => return Some(format!("{name}: `ridlc::compile` panicked")),
    };

    for decl in &package.decls {
        if decl.name.is_empty() {
            return Some(format!(
                "{name}: a nameless package declaration reached the IR"
            ));
        }
    }
    for service in &package.services {
        if service.name.is_empty() {
            return Some(format!("{name}: a nameless service reached the IR"));
        }
    }
    for decl in interactions(&package) {
        // A `reserved` tombstone's `Decl` name is empty by design — the retired
        // name lives in `Reserved.name` (typl §7.4).
        if matches!(decl.kind, Some(ridl_ir::v2::decl::Kind::ReservedSlot(_))) {
            continue;
        }
        if decl.name.is_empty() {
            return Some(format!(
                "{name}: a nameless interaction reached the IR (issue #158)"
            ));
        }
    }
    None
}

/// Sweeps the malformed corpus and the well-formed corpus entries. Every
/// failure is collected so the message names all of them at once.
#[test]
fn compile_is_total_over_the_corpus() {
    let mut files = source_files(Path::new("tests/malformed"));
    files.extend(source_files(Path::new("tests/corpus")));
    // A tripwire against the sweep silently finding nothing — a wrong path, a
    // moved directory, a filter that stops matching. Kept just under the real
    // count so adding one corpus file does not trip it, and close enough that
    // losing a directory does.
    assert!(
        files.len() >= 50,
        "the sweep must actually find the corpus; found {} files, expected at least 50",
        files.len()
    );

    let failures: Vec<String> = files.iter().filter_map(|path| sweep_one(path)).collect();
    assert!(
        failures.is_empty(),
        "`ridlc::compile` is documented total; these inputs broke it:\n{}",
        failures.join("\n")
    );
}

/// The malformed corpus is only meaningful if it is actually malformed: a file
/// that quietly became valid stops testing the recovery paths and the sweep
/// would still pass. Every malformed member must draw at least one diagnostic.
///
/// `only_package.ridl`, `empty.ridl` and `whitespace_only.ridl` are the
/// exceptions — a file with nothing to parse beyond its package declaration is
/// legal, and they are in the sweep for the degenerate-input path, not for
/// their diagnostics.
#[test]
fn every_malformed_member_is_actually_malformed() {
    const LEGAL: &[&str] = &["only_package.ridl", "empty.ridl", "whitespace_only.ridl"];

    let mut silent = Vec::new();
    for path in source_files(Path::new("tests/malformed")) {
        let file_name = path
            .file_name()
            .expect("a swept path names a file")
            .to_string_lossy()
            .into_owned();
        if LEGAL.contains(&file_name.as_str()) {
            continue;
        }
        let text = std::fs::read_to_string(&path).expect("a malformed corpus member is readable");
        let output = ridlc::compile(&path.display().to_string(), &text);
        if output.diagnostics.is_empty() {
            silent.push(file_name);
        }
    }
    assert!(
        silent.is_empty(),
        "these malformed corpus members drew no diagnostic at all, so they no longer test anything: {silent:?}"
    );
}

/// An empty file and a file holding only its package declaration are the
/// degenerate inputs at the other end: legal, and they must compile to an
/// empty package rather than to anything at all.
#[test]
fn degenerate_inputs_compile_to_an_empty_package() {
    for (path, text) in [("empty.ridl", ""), ("only_package.ridl", "package app\n")] {
        let output = ridlc::compile(path, text);
        assert!(
            output.package.decls.is_empty()
                && output.package.interfaces.is_empty()
                && output.package.services.is_empty(),
            "{path} lowered something out of nothing"
        );
    }
}
