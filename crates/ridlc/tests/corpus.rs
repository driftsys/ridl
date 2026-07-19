//! The full-pipeline corpus (docs/ROADMAP.md epic E1.18, ADR-0007 decision 3).
//!
//! Each directory under `corpus/` is a real package (or workspace) with its own
//! `ridl.toml`. This runner compiles every entry end to end — load, parse,
//! resolve, check and lower to IR v2, generate Rust — and snapshots three
//! artifacts per entry: the rendered diagnostics, the IR v2 JSON, and the
//! generated Rust. `insta::glob!` drives one iteration per entry.
//!
//! The diagnostics are snapshotted for every entry. The IR JSON and generated
//! Rust are snapshotted only for an entry that compiles without error
//! diagnostics: those are the full-pipeline golden. For an entry crafted to
//! carry errors (the diagnostic showcase), lowering is partial and code
//! generation over broken IR is not meaningful, so both are reduced to a
//! one-line note and the diagnostics snapshot is that entry's artifact of
//! record.
//!
//! The three entries stake out the surface:
//!
//! - `veh-common/` — the typl reference Appendix B example as a real package;
//!   it compiles clean, so its snapshots are the end-to-end golden for a
//!   realistic package (units, constants, enums, a result union, tuples,
//!   collections, reserved fields, an internal type).
//! - `diag-showcase/` — a package crafted so its compile emits one instance of
//!   every diagnostic that a single package's source and manifest can trigger.
//!   Its diagnostics snapshot is the de-facto diagnostic index until the error
//!   index website (E4.2). The codes that need more than one package, a broken
//!   or workspace manifest, or the network are listed in `diag-showcase/NOTES`
//!   and are not exercised here.
//! - `workspace-two-members/` — a two-member workspace with a cross-member
//!   import, exercising the resolver's workspace-member resolution path.
//!
//! Paths handed to the loader are relative to the crate directory (the working
//! directory `cargo test` sets), so the file paths that appear in the rendered
//! diagnostics stay portable across machines.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use ridl_core::db::InputFile;
use ridl_core::diag::{
    DiagCode, Diagnostic, FileId, Severity, SourceMap, Span, house_style_message,
    remap_diagnostics, render,
};
use ridl_core::package::Package;
use ridl_core::{RidlDatabase, load_workspace, parse_file, std_package};
use ridl_sem::{check_package, resolve_package};

/// The three snapshotted artifacts of one compiled corpus entry.
struct Compiled {
    /// The rendered diagnostics, or a placeholder when the entry compiles
    /// clean.
    diagnostics: String,
    /// The IR v2 JSON of every package in the entry (one section per package),
    /// or a one-line note when the entry has error diagnostics.
    ir_json: String,
    /// The generated Rust of every package in the entry (one section per
    /// package), or a one-line note when the entry has error diagnostics.
    rust: String,
}

/// Compiles the corpus entry rooted at `entry` end to end.
///
/// Every diagnostic the pipeline emits is collected onto one [`SourceMap`]:
/// the loader's manifest and package-law findings, each file's parser errors,
/// and the per-package resolver and checker diagnostics (whose package-relative
/// [`FileId`](ridl_core::diag::FileId)s are remapped onto the shared map). The
/// merged list is sorted into source order — by file, then span — so the
/// rendered output reads top to bottom and the snapshot is stable regardless of
/// pass evaluation order.
fn compile_entry(entry: &Path) -> Compiled {
    let mut db = RidlDatabase::default();
    let std = std_package(&mut db);
    let loaded = load_workspace(&mut db, entry).expect("a corpus entry loads");
    let workspace = loaded.workspace;
    let mut sources: SourceMap = loaded.sources;
    let mut diagnostics: Vec<Diagnostic> = loaded.diagnostics;

    let packages: Vec<Package> = workspace.packages(&db).clone();
    // The checked IR of each package, kept so code generation runs only after
    // the whole entry is known to be error-free.
    let mut checked_irs: Vec<(String, ridl_ir::v2::Package)> = Vec::new();

    for pkg in &packages {
        let name = pkg.name(&db).clone();
        let files: Vec<InputFile> = pkg.files(&db).clone();

        // The render ids for this package's files, in the same order the
        // package-scoped passes stamp their FileIds (pkg.files order).
        let render_ids: Vec<_> = files
            .iter()
            .map(|file| sources.file_id(file.path(&db), file.text(&db)))
            .collect();

        // Parser diagnostics: each file's SyntaxErrors carry their own FORM- or
        // TYPL-302 code and a real range; polish the message into the house
        // style exactly as `ridlc::compile` does.
        for (index, file) in files.iter().enumerate() {
            let file_id = render_ids[index];
            for error in parse_file(&db, *file).errors() {
                diagnostics.push(Diagnostic {
                    code: DiagCode(error.code),
                    severity: Severity::Error,
                    message: house_style_message(&error.message),
                    primary: Span {
                        file: file_id,
                        range: error.range,
                    },
                    labels: Vec::new(),
                    fixits: Vec::new(),
                });
            }
        }

        let resolution = resolve_package(&db, workspace, *pkg, std);
        let checked = check_package(&db, workspace, *pkg, std);
        diagnostics.extend(remap_diagnostics(resolution.diagnostics, &render_ids));
        diagnostics.extend(remap_diagnostics(checked.diagnostics, &render_ids));
        checked_irs.push((name, checked.ir));
    }

    // IR JSON and generated Rust are recorded only for an entry that compiles
    // without errors. For a clean entry these are the full-pipeline golden. For
    // an entry with error diagnostics (the diagnostic showcase), lowering is
    // partial and code generation over broken IR is not meaningful — and the
    // diagnostics snapshot is that entry's artifact of record — so both are
    // reduced to a one-line note. Warnings and info diagnostics do not gate.
    let has_errors = diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == Severity::Error);
    let (ir_json, rust) = if has_errors {
        let note = "(entry has error diagnostics; IR and generated Rust are omitted \
                    — see the diagnostics snapshot)\n"
            .to_string();
        (note.clone(), note)
    } else {
        let ir = checked_irs
            .iter()
            .map(|(name, ir)| {
                format!(
                    "// ===== package {name} =====\n{}",
                    ridl_ir::v2::to_json_pretty(ir)
                )
            })
            .collect::<Vec<_>>()
            .join("\n\n");
        let rust = checked_irs
            .iter()
            .map(|(name, ir)| {
                let generated =
                    ridl_backend_rust::generate(ir).expect("a clean entry's IR generates Rust");
                format!("// ===== package {name} =====\n{}", generated.rust_source)
            })
            .collect::<Vec<_>>()
            .join("\n\n");
        (ir, rust)
    };

    // Source order: by file, then by span start, then end, then code — a stable
    // total order independent of how the passes were evaluated. `FileId` is not
    // `Ord`, so files are ranked by first appearance in the collected list
    // (itself deterministic), which groups each file's diagnostics together.
    let mut file_rank: HashMap<FileId, usize> = HashMap::new();
    for diagnostic in &diagnostics {
        let next = file_rank.len();
        file_rank.entry(diagnostic.primary.file).or_insert(next);
    }
    diagnostics.sort_by(|a, b| {
        let key = |d: &Diagnostic| {
            (
                file_rank[&d.primary.file],
                d.primary.range.start(),
                d.primary.range.end(),
                d.code.as_str(),
            )
        };
        key(a).cmp(&key(b))
    });

    let diagnostics = if diagnostics.is_empty() {
        "(no diagnostics — the entry compiles clean)\n".to_string()
    } else {
        render(&diagnostics, &sources)
    };

    Compiled {
        diagnostics,
        ir_json,
        rust,
    }
}

/// The crate-relative path of the corpus entry whose manifest is `manifest`.
/// `insta::glob!` yields an absolute manifest path; the loader is handed the
/// crate-relative directory so the paths baked into the diagnostics stay
/// portable.
fn crate_relative_entry(manifest: &Path) -> (String, PathBuf) {
    let entry_dir = manifest
        .parent()
        .expect("a manifest has a parent directory");
    let name = entry_dir
        .file_name()
        .expect("a corpus entry has a directory name")
        .to_string_lossy()
        .into_owned();
    (name.clone(), Path::new("tests/corpus").join(name))
}

/// Compiles every corpus entry and snapshots its diagnostics, IR JSON, and
/// generated Rust. Each entry's three snapshots are suffixed with the entry
/// directory name (`@veh-common`, `@diag-showcase`, `@workspace-two-members`).
#[test]
fn corpus_entries_compile_to_reviewed_snapshots() {
    insta::glob!("corpus", "*/ridl.toml", |manifest| {
        let (name, entry) = crate_relative_entry(manifest);
        let compiled = compile_entry(&entry);

        insta::with_settings!({ snapshot_suffix => name.clone() }, {
            insta::assert_snapshot!("diagnostics", compiled.diagnostics);
            insta::assert_snapshot!("ir", compiled.ir_json);
            insta::assert_snapshot!("rust", compiled.rust);
        });
    });
}

/// The generated Rust for the clean `veh-common` entry compiles end to end
/// through the whole pipeline. A minimal prelude stands in for the `ridl.std`
/// types the package imports, declared in the module path the cross-package
/// references resolve to (`ridl::std::…`).
///
/// The assertion is on the rustc **exit status**, not on empty stderr: the
/// generated code is valid but carries non-fatal lints (for example
/// `non_camel_case_types` on a screaming-case enum variant), which are warnings,
/// not errors.
#[test]
fn veh_common_generated_rust_compiles_with_rustc() {
    const PRELUDE: &str = "\
pub mod ridl {
    pub mod std {
        pub struct Name(pub String);
        pub struct Message(pub String);
        pub struct Label(pub String);
        pub struct Timestamp(pub i64);
        impl Default for Timestamp {
            fn default() -> Self {
                Timestamp(0)
            }
        }
    }
}
";
    let compiled = compile_entry(Path::new("tests/corpus/veh-common"));
    let source = format!("{PRELUDE}\n{}", compiled.rust);

    // A unique temp directory (removed at the end), mirroring the golden CLI
    // test rather than adding a `tempfile` dev-dependency.
    let dir = std::env::temp_dir().join(format!(
        "ridlc_corpus_rustc_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock reads a time after the unix epoch")
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).expect("the temp dir is created");
    let source_path = dir.join("veh_common.rs");
    let meta_path = dir.join("veh_common.rmeta");
    std::fs::write(&source_path, &source).expect("the generated source is written");

    let status = std::process::Command::new("rustc")
        .args([
            "--edition",
            "2024",
            "--crate-type",
            "lib",
            "--emit",
            "metadata",
        ])
        .arg("-o")
        .arg(&meta_path)
        .arg(&source_path)
        .status()
        .expect("rustc must be installed and runnable for this test to be meaningful");

    let succeeded = status.success();
    std::fs::remove_dir_all(&dir).ok();
    assert!(
        succeeded,
        "the pipeline's generated Rust for veh-common must compile, source:\n{source}"
    );
}

/// The generated Rust of every package in the entry rooted at `entry`, as
/// `(package-name, rust-source)` pairs, in workspace order.
fn generated_packages(entry: &Path) -> Vec<(String, String)> {
    let mut db = RidlDatabase::default();
    let std = std_package(&mut db);
    let workspace = load_workspace(&mut db, entry)
        .expect("a corpus entry loads")
        .workspace;
    let packages: Vec<Package> = workspace.packages(&db).clone();
    packages
        .iter()
        .map(|pkg| {
            let name = pkg.name(&db).clone();
            let checked = check_package(&db, workspace, *pkg, std);
            let generated = ridl_backend_rust::generate(&checked.ir)
                .expect("a clean entry's IR generates Rust");
            (name, generated.rust_source)
        })
        .collect()
}

/// A tree of Rust modules keyed by dotted package name, so several generated
/// packages compose as nested modules that share their common name prefixes
/// (`veh.common` and `veh.cluster` both nest under one `mod veh`).
#[derive(Default)]
struct ModuleTree {
    children: std::collections::BTreeMap<String, ModuleTree>,
    body: Option<String>,
}

impl ModuleTree {
    fn insert(&mut self, segments: &[&str], body: String) {
        match segments.split_first() {
            None => self.body = Some(body),
            Some((head, rest)) => self
                .children
                .entry((*head).to_string())
                .or_default()
                .insert(rest, body),
        }
    }

    fn render(&self, out: &mut String) {
        if let Some(body) = &self.body {
            out.push_str(body);
            out.push('\n');
        }
        for (name, child) in &self.children {
            out.push_str(&format!("pub mod {name} {{\n"));
            child.render(out);
            out.push_str("}\n");
        }
    }
}

/// The two-member workspace's generated Rust composes: nesting each package
/// under its dotted module path (`crate::veh::common`, `crate::veh::cluster`)
/// and compiling the whole together with `rustc` succeeds. This proves the
/// cross-package references are crate-anchored (`crate::veh::common::Speed`), so
/// a consumer can drop the generated packages in as sibling modules without
/// hand-injecting `use` items (I4).
#[test]
fn workspace_two_members_composed_compiles_with_rustc() {
    const PRELUDE: &str = "\
pub mod ridl {
    pub mod std {
        pub struct Name(pub String);
        pub struct Message(pub String);
        pub struct Label(pub String);
        pub struct Timestamp(pub i64);
    }
}
";
    let packages = generated_packages(Path::new("tests/corpus/workspace-two-members"));
    let mut tree = ModuleTree::default();
    for (name, body) in &packages {
        let segments: Vec<&str> = name.split('.').collect();
        tree.insert(&segments, body.clone());
    }
    let mut composed = String::new();
    tree.render(&mut composed);
    let source = format!("{PRELUDE}\n{composed}");

    let dir = std::env::temp_dir().join(format!(
        "ridlc_corpus_composed_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock reads a time after the unix epoch")
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).expect("the temp dir is created");
    let source_path = dir.join("composed.rs");
    let meta_path = dir.join("composed.rmeta");
    std::fs::write(&source_path, &source).expect("the composed source is written");

    let status = std::process::Command::new("rustc")
        .args([
            "--edition",
            "2024",
            "--crate-type",
            "lib",
            "--emit",
            "metadata",
        ])
        .arg("-o")
        .arg(&meta_path)
        .arg(&source_path)
        .status()
        .expect("rustc must be installed and runnable for this test to be meaningful");

    let succeeded = status.success();
    std::fs::remove_dir_all(&dir).ok();
    assert!(
        succeeded,
        "the composed workspace-two-members Rust must compile, source:\n{source}"
    );
}
