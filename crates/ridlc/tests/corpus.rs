//! The full-pipeline corpus (docs/ROADMAP.md epics E1.18 and E2.11b, ADR-0007
//! decision 3).
//!
//! Each directory under `corpus/` is a real package (or workspace) with its own
//! `ridl.toml`. This runner compiles every entry end to end — load, parse,
//! resolve, check, lower to IR v2, run the workspace-wide service catalog, and
//! generate Rust and TypeScript — and snapshots four artifacts per entry: the
//! rendered diagnostics, the IR v2 JSON, the generated Rust, and the generated
//! TypeScript. `insta::glob!` drives one iteration per entry.
//!
//! The diagnostics are snapshotted for every entry. The IR JSON and generated
//! code are snapshotted only for an entry that compiles without error
//! diagnostics: those are the full-pipeline golden. For an entry crafted to
//! carry errors (a diagnostic showcase), lowering is partial and code
//! generation over broken IR is not meaningful, so all three are reduced to a
//! one-line note and the diagnostics snapshot is that entry's artifact of
//! record.
//!
//! The entries stake out the surface. The typl layer (E1):
//!
//! - `veh-common/` — the typl reference Appendix B example as a real package;
//!   it compiles clean, so its snapshots are the end-to-end golden for a
//!   realistic package (units, constants, enums, a result union, tuples,
//!   collections, reserved fields, an internal type).
//! - `diag-showcase/` — a package crafted so its compile emits one instance of
//!   every typl-layer diagnostic that a single package's source and manifest
//!   can trigger. Its diagnostics snapshot is the de-facto typl diagnostic
//!   index until the error index website (E4.2). The codes that need more than
//!   one package, a broken or workspace manifest, or the network are listed in
//!   `diag-showcase/NOTES` and are not exercised there.
//! - `workspace-two-members/` — a two-member workspace with a cross-member
//!   import, exercising the resolver's workspace-member resolution path.
//!
//! The ridl interaction layer (E2):
//!
//! - `veh-cluster/` — the ridl reference Appendix A verbatim, plus authored
//!   §14.5 services. The golden `.ridl` package: both service forms, all five
//!   interaction kinds, every timing mode, contracts, streams, tombstones. See
//!   `veh-cluster/NOTES`.
//! - `ridl-diag-showcase/` — the ridl counterpart of `diag-showcase`: a
//!   two-member workspace emitting one instance of every implemented
//!   ridl-profile diagnostic. [`RIDL_PROFILE_CODES`] below is the machine-checked
//!   index; `ridl-diag-showcase/NOTES` is the prose one.
//! - `services-workspace/` — shapes in one member, services in another: the
//!   cross-package service path, including an inline shape whose whole
//!   vocabulary is imported. See `services-workspace/NOTES`.
//!
//! The malformed programs live in `tests/malformed/` and are driven by
//! `tests/totality.rs`: they are single files with no manifest, so the corpus
//! glob has nothing to do with them.
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
use ridl_core::package::{Package, service_catalog};
use ridl_core::{RidlDatabase, load_workspace, parse_file, std_package};
use ridl_sem::{check_package, resolve_package};

/// The four snapshotted artifacts of one compiled corpus entry.
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
    /// The generated TypeScript of every package in the entry (one section per
    /// package), or a one-line note when the entry has error diagnostics.
    typescript: String,
    /// Every diagnostic as `(code, severity)`, in the same order the rendered
    /// snapshot lists them. The coverage tests read this rather than grepping
    /// the rendered text.
    coded: Vec<(String, Severity)>,
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

    // The workspace-wide service catalog (E2 task 8), driven exactly as
    // `ridlc::compile_workspace` drives it: its RIDL-140 duplicate-name
    // diagnostics span the whole workspace, so their FileIds index every file
    // in package-then-file order. That order is rebuilt here and remapped onto
    // the render source map. Without this the runner would compile a workspace
    // the real pipeline rejects and snapshot it as clean.
    let catalog = service_catalog(&db, workspace, std);
    if !catalog.diagnostics.is_empty() {
        let mut catalog_render_ids = Vec::new();
        for pkg in &packages {
            for file in pkg.files(&db) {
                catalog_render_ids.push(sources.file_id(file.path(&db), file.text(&db)));
            }
        }
        diagnostics.extend(remap_diagnostics(catalog.diagnostics, &catalog_render_ids));
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
    let (ir_json, rust, typescript) = if has_errors {
        let note = "(entry has error diagnostics; IR and generated code are omitted \
                    — see the diagnostics snapshot)\n"
            .to_string();
        (note.clone(), note.clone(), note)
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
        let typescript = checked_irs
            .iter()
            .map(|(name, ir)| {
                let generated =
                    ridl_backend_ts::generate(ir).expect("a clean entry's IR generates TypeScript");
                format!("// ===== package {name} =====\n{}", generated.source)
            })
            .collect::<Vec<_>>()
            .join("\n\n");
        (ir, rust, typescript)
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

    let coded: Vec<(String, Severity)> = diagnostics
        .iter()
        .map(|diagnostic| (diagnostic.code.as_str().to_string(), diagnostic.severity))
        .collect();

    let diagnostics = if diagnostics.is_empty() {
        "(no diagnostics — the entry compiles clean)\n".to_string()
    } else {
        render(&diagnostics, &sources)
    };

    Compiled {
        diagnostics,
        ir_json,
        rust,
        typescript,
        coded,
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
/// generated Rust and TypeScript. Each entry's four snapshots are suffixed with
/// the entry directory name (`@veh-common`, `@diag-showcase`, …).
#[test]
fn corpus_entries_compile_to_reviewed_snapshots() {
    insta::glob!("corpus", "*/ridl.toml", |manifest| {
        let (name, entry) = crate_relative_entry(manifest);
        let compiled = compile_entry(&entry);

        insta::with_settings!({ snapshot_suffix => name.clone() }, {
            insta::assert_snapshot!("diagnostics", compiled.diagnostics);
            insta::assert_snapshot!("ir", compiled.ir_json);
            insta::assert_snapshot!("rust", compiled.rust);
            insta::assert_snapshot!("typescript", compiled.typescript);
        });
    });
}

/// Every code the ridl profile defines (ridl reference §16, plus the shared
/// FORM/MANI/TYPL codes E2 added or folded in), paired with where the corpus
/// provokes it. `Showcase` means the `ridl-diag-showcase` entry emits it; every
/// other variant names the reason it cannot live there and where it does live.
///
/// This table is the machine-checked half of the diagnostic index: a code
/// listed as `Showcase` that stops firing fails
/// [`showcase_provokes_exactly_the_expected_codes`], and a code that has no
/// entry at all fails to compile this file.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Provoked {
    /// Emitted by the `ridl-diag-showcase` corpus entry.
    Showcase,
    /// Emitted elsewhere, by a fixture this crate does not compile. The first
    /// field is that fixture's path **from the repository root**, checked to
    /// exist by [`every_ridl_profile_code_has_a_living_example`]; the second is
    /// the reason the code cannot be part of a single compile of the showcase.
    ///
    /// The path is not decoration. Asserting only that the reason string is
    /// non-empty would be unfalsifiable — a `&'static str` literal is never
    /// empty — so the test that presents itself as the coverage index would
    /// pass with the fixture deleted.
    Elsewhere {
        fixture: &'static str,
        reason: &'static str,
    },
}

use Provoked::{Elsewhere, Showcase};

const RIDL_PROFILE_CODES: &[(&str, Provoked)] = &[
    ("RIDL-100", Showcase),
    ("RIDL-101", Showcase),
    ("RIDL-102", Showcase),
    ("RIDL-103", Showcase),
    ("RIDL-104", Showcase),
    ("RIDL-105", Showcase),
    ("RIDL-106", Showcase),
    ("RIDL-107", Showcase),
    ("RIDL-108", Showcase),
    ("RIDL-109", Showcase),
    ("RIDL-110", Showcase),
    ("RIDL-140", Showcase),
    ("RIDL-141", Showcase),
    ("RIDL-143", Showcase),
    ("RIDL-201", Showcase),
    ("RIDL-202", Showcase),
    ("RIDL-301", Showcase),
    ("RIDL-302", Showcase),
    ("RIDL-303", Showcase),
    ("RIDL-304", Showcase),
    ("RIDL-305", Showcase),
    ("RIDL-306", Showcase),
    ("RIDL-307", Showcase),
    ("RIDL-308", Showcase),
    ("RIDL-401", Showcase),
    ("RIDL-402", Showcase),
    ("RIDL-403", Showcase),
    ("RIDL-404", Showcase),
    ("RIDL-405", Showcase),
    ("RIDL-406", Showcase),
    (
        "RIDL-407",
        Elsewhere {
            fixture: "crates/ridl/tests/baseline-corpus/.ridl/baseline/corpus.baseline.ir.json",
            reason: "the baseline-aware desk check reads a workspace-local baseline, which is \
                     outside `ridlc`'s source-to-IR function, so it is never a compile \
                     diagnostic (ADR-0008 decisions 9 and 13). Provoked by the committed \
                     baseline corpus member, driven by \
                     `check_reports_ordinal_drift_against_the_committed_baseline` and \
                     `inline_shape_removal_renders_without_a_span` in \
                     `crates/ridl/tests/baseline_desk.rs`",
        },
    ),
    // The shared codes E2 added or folded into the ridl profile.
    ("TYPL-005", Showcase),
    ("FORM-106", Showcase),
    ("FORM-107", Showcase),
    ("FORM-108", Showcase),
    ("MANI-009", Showcase),
    ("TYPL-301", Showcase),
    (
        "TYPL-302",
        Elsewhere {
            fixture: "crates/ridlc/tests/corpus/diag-showcase/profile_boundary.typl",
            reason: "a duration literal or timing annotation in a typl context — already \
                     the artifact of record in the E1 `diag-showcase` entry, and repeating \
                     it here would duplicate the index entry rather than add one",
        },
    ),
    ("TYPL-303", Showcase),
    ("TYPL-304", Showcase),
];

/// The codes the showcase emits that are not ridl-profile codes: parse and
/// vocabulary diagnostics the deliberately broken source drags in. They are
/// listed so the exact-set assertion below stays exact — a new one appearing
/// means the showcase's source changed shape.
const SHOWCASE_INCIDENTAL_CODES: &[&str] = &[
    // The interface body cannot hold a `struct`, so the parser reports the
    // token before the checker reports RIDL-107; the interaction-boundary
    // narrowings (`narrowing.ridl`) report through it too.
    "FORM-102",
    // A rejected return type (`narrowing_returns.ridl`). Both this and
    // FORM-102 are raised from many places, which is why the narrowings have
    // their own message-level assertion — set equality over codes alone would
    // not notice one of them starting to accept.
    "FORM-101",
    // `Serial` carries a `match` pattern, so it has no derivable init — the
    // typl-level statement of the condition RIDL-109 escalates for a signal.
    "TYPL-115",
];

/// The diagnostic showcase emits exactly the expected codes: every ridl-profile
/// code marked [`Showcase`], plus the listed incidental ones, and nothing else.
///
/// Set *equality* is the assertion, not containment. A code that quietly stops
/// firing fails on the missing side; a code that starts firing where it was not
/// meant to fails on the extra side. Containment would only catch the first.
#[test]
fn showcase_provokes_exactly_the_expected_codes() {
    let compiled = compile_entry(Path::new("tests/corpus/ridl-diag-showcase"));

    let actual: std::collections::BTreeSet<&str> = compiled
        .coded
        .iter()
        .map(|(code, _)| code.as_str())
        .collect();
    let expected: std::collections::BTreeSet<&str> = RIDL_PROFILE_CODES
        .iter()
        .filter(|(_, where_)| *where_ == Showcase)
        .map(|(code, _)| *code)
        .chain(SHOWCASE_INCIDENTAL_CODES.iter().copied())
        .collect();

    let missing: Vec<&str> = expected.difference(&actual).copied().collect();
    let unexpected: Vec<&str> = actual.difference(&expected).copied().collect();
    assert!(
        missing.is_empty() && unexpected.is_empty(),
        "the diagnostic showcase drifted.\n  no longer provoked: {missing:?}\n  \
         newly provoked: {unexpected:?}",
    );
}

/// The repository root, reached from this crate's directory (the working
/// directory `cargo test` sets). An [`Elsewhere`] fixture lives in a sibling
/// crate, so its recorded path is root-relative.
fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .find(|dir| dir.join("Cargo.lock").is_file() && dir.join("justfile").is_file())
        .expect("the repository root is an ancestor of this crate")
        .to_path_buf()
}

/// Every ridl-profile diagnostic has a living example.
///
/// A code recorded as [`Showcase`] must be emitted by the showcase. A code
/// recorded as [`Elsewhere`] must name a fixture that **exists on disk** — not
/// merely carry a reason string. The reason-only version of this assertion was
/// unfalsifiable (`!reason.is_empty()` on a string literal), so the one test
/// whose entire job is to be the coverage index passed with the referenced
/// fixture deleted. Checking the path restores the property the name claims.
///
/// The fixture check is deliberately a file-existence check rather than a
/// re-run of the sibling crate's test: this crate cannot drive `ridl`'s binary,
/// and the named test in `crates/ridl/tests/baseline_desk.rs` is where the
/// diagnostic itself is pinned. What this assertion buys is that the two
/// cannot drift apart silently — deleting the fixture fails here, and the
/// reason string names exactly which test to look at.
#[test]
fn every_ridl_profile_code_has_a_living_example() {
    let showcase: std::collections::BTreeSet<String> =
        compile_entry(Path::new("tests/corpus/ridl-diag-showcase"))
            .coded
            .into_iter()
            .map(|(code, _)| code)
            .collect();
    let root = repository_root();

    for (code, provoked) in RIDL_PROFILE_CODES {
        match provoked {
            Showcase => assert!(
                showcase.contains(*code),
                "{code} is recorded as provoked by the showcase but the showcase does not emit it",
            ),
            Elsewhere { fixture, reason } => {
                assert!(
                    root.join(fixture).exists(),
                    "{code} is not provoked by the showcase, and the fixture recorded as its \
                     living example is gone: {fixture}\n  recorded reason: {reason}",
                );
                assert!(
                    !showcase.contains(*code),
                    "{code} is recorded as provoked elsewhere, but the showcase emits it now — \
                     move it to `Showcase`",
                );
            }
        }
    }
}

/// Severity is part of what a diagnostic promises: the reference §16 tables
/// classify each code, and a code that silently changes severity changes
/// whether a build fails. The showcase pins every code's severity.
#[test]
fn showcase_pins_every_severity() {
    use std::collections::BTreeMap;

    const EXPECTED: &[(&str, Severity)] = &[
        ("FORM-101", Severity::Error),
        ("FORM-102", Severity::Error),
        ("FORM-106", Severity::Error),
        ("FORM-107", Severity::Error),
        ("FORM-108", Severity::Error),
        ("MANI-009", Severity::Error),
        ("RIDL-100", Severity::Warning),
        ("RIDL-101", Severity::Error),
        ("RIDL-102", Severity::Error),
        ("RIDL-103", Severity::Error),
        ("RIDL-104", Severity::Error),
        ("RIDL-105", Severity::Error),
        ("RIDL-106", Severity::Error),
        ("RIDL-107", Severity::Error),
        ("RIDL-108", Severity::Warning),
        ("RIDL-109", Severity::Error),
        ("RIDL-110", Severity::Error),
        ("RIDL-140", Severity::Error),
        ("RIDL-141", Severity::Error),
        ("RIDL-143", Severity::Error),
        ("RIDL-201", Severity::Error),
        ("RIDL-202", Severity::Error),
        ("RIDL-301", Severity::Error),
        ("RIDL-302", Severity::Error),
        ("RIDL-303", Severity::Error),
        ("RIDL-304", Severity::Warning),
        ("RIDL-305", Severity::Warning),
        ("RIDL-306", Severity::Error),
        ("RIDL-307", Severity::Warning),
        ("RIDL-308", Severity::Warning),
        ("RIDL-401", Severity::Error),
        ("RIDL-402", Severity::Error),
        ("RIDL-403", Severity::Error),
        ("RIDL-404", Severity::Warning),
        ("RIDL-405", Severity::Info),
        ("RIDL-406", Severity::Info),
        ("TYPL-005", Severity::Error),
        ("TYPL-115", Severity::Info),
        ("TYPL-301", Severity::Error),
        ("TYPL-303", Severity::Error),
        ("TYPL-304", Severity::Error),
    ];

    let compiled = compile_entry(Path::new("tests/corpus/ridl-diag-showcase"));
    let mut seen: BTreeMap<&str, Severity> = BTreeMap::new();
    for (code, severity) in &compiled.coded {
        if let Some(previous) = seen.insert(code.as_str(), *severity) {
            assert_eq!(
                previous, *severity,
                "{code} was emitted at two different severities in one compile",
            );
        }
    }

    let expected: BTreeMap<&str, Severity> = EXPECTED.iter().copied().collect();
    assert_eq!(
        seen.keys().collect::<Vec<_>>(),
        expected.keys().collect::<Vec<_>>(),
        "the severity table and the showcase list different codes",
    );
    for (code, severity) in &expected {
        assert_eq!(
            seen.get(code),
            Some(severity),
            "{code} changed severity: the reference §16 tables classify it as {severity:?}",
        );
    }
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

/// Runs `rustc` over `source` as a library, returning whether it exited zero.
/// The assertion is on the **exit status**, not on empty stderr: the generated
/// code is valid but carries non-fatal lints (`non_camel_case_types` on a
/// screaming-case enum variant, dead code on an unused internal type), which
/// are warnings, not errors.
fn rustc_accepts(label: &str, source: &str) -> bool {
    let dir = std::env::temp_dir().join(format!(
        "ridlc_corpus_{label}_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock reads a time after the unix epoch")
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).expect("the temp dir is created");
    let source_path = dir.join(format!("{label}.rs"));
    let meta_path = dir.join(format!("{label}.rmeta"));
    std::fs::write(&source_path, source).expect("the generated source is written");

    let status = std::process::Command::new("rustc")
        .args([
            "--edition",
            "2024",
            "--crate-type",
            "lib",
            "--emit",
            "metadata",
            // The one lint denied by name (issue #161). A generated `pub` item
            // over a `pub(crate)` type is warn-by-default on current rustc, so
            // a plain exit-status check accepts it — which is why the corpus's
            // own compile proof did not notice a public interface carrying an
            // `internal` payload. A blanket `-D warnings` is not usable here:
            // the generated code carries unrelated non-fatal lints by design
            // (`non_camel_case_types` on a screaming-case enum variant, dead
            // code in a crate with no consumers), so denying everything would
            // fail for reasons that say nothing about visibility.
            "-D",
            "private-interfaces",
            "-D",
            "private-bounds",
        ])
        .arg("-o")
        .arg(&meta_path)
        .arg(&source_path)
        .status()
        .expect("rustc must be installed and runnable for this test to be meaningful");

    let succeeded = status.success();
    std::fs::remove_dir_all(&dir).ok();
    succeeded
}

/// The generated packages of `entry`, nested under their dotted module paths
/// and prefixed with a `ridl::std` stand-in, as one compilable crate root.
fn composed_source(entry: &Path) -> String {
    // The `ridl.std` types the interaction-layer entries reference. Newtypes
    // rather than aliases, so a wrong path in the generated code cannot
    // accidentally type-check against a primitive.
    const PRELUDE: &str = "\
pub mod ridl {
    pub mod std {
        #[derive(Default)] pub struct Name(pub String);
        #[derive(Default)] pub struct Message(pub String);
        #[derive(Default)] pub struct Label(pub String);
        #[derive(Default)] pub struct Timestamp(pub i64);
        #[derive(Default)] pub struct Version(pub String);
        #[derive(Default)] pub struct Duration(pub f64);
    }
}
";
    let mut tree = ModuleTree::default();
    for (name, body) in &generated_packages(entry) {
        let segments: Vec<&str> = name.split('.').collect();
        tree.insert(&segments, body.clone());
    }
    let mut composed = String::new();
    tree.render(&mut composed);
    format!("{PRELUDE}\n{composed}")
}

/// The generated Rust for `veh-cluster` compiles.
///
/// The two existing rustc proofs cover typl-only entries, so until this test
/// the interaction layer's generated Rust was only ever compared against a text
/// snapshot — a snapshot records what was emitted, not that it is valid Rust.
/// `veh-cluster` is the entry that exercises the whole interaction surface,
/// including the consumer and provider faces of a service's inline shape, the
/// synthesized tuple-return struct, and the `#[deprecated]` attribute.
#[test]
fn veh_cluster_generated_rust_compiles_with_rustc() {
    let source = composed_source(Path::new("tests/corpus/veh-cluster"));
    // Anti-vacuity: rustc accepts an empty crate, so the proof is only worth
    // something if the interaction layer is actually in the source it sees —
    // both faces of a named interface and both faces of an inline shape.
    for marker in [
        "pub trait VehicleStatusConsumer",
        "pub trait VehicleStatusProvider",
        "pub trait ServiceVehHvacCabinConsumer",
        "pub trait ServiceVehHvacCabinProvider",
    ] {
        assert!(
            source.contains(marker),
            "the composed source must contain `{marker}`, or this proof compiles nothing",
        );
    }
    let succeeded = rustc_accepts("veh_cluster", &source);
    assert!(
        succeeded,
        "the interaction layer's generated Rust for veh-cluster must compile, source:\n{source}"
    );
}

/// The generated Rust for `services-workspace` compiles when its two members
/// are composed as sibling modules. The interaction-layer counterpart of
/// [`workspace_two_members_composed_compiles_with_rustc`]: here the
/// cross-package references are made from inside interaction signatures and
/// from inside a service's inline shape, not only from struct fields.
#[test]
fn services_workspace_composed_compiles_with_rustc() {
    let source = composed_source(Path::new("tests/corpus/services-workspace"));
    for marker in [
        "pub trait DoorControlConsumer",
        "pub trait ServiceFleetVehicleMirrorsProvider",
        "crate::fleet::contracts::",
    ] {
        assert!(
            source.contains(marker),
            "the composed source must contain `{marker}`, or this proof compiles nothing",
        );
    }
    let succeeded = rustc_accepts("services_workspace", &source);
    assert!(
        succeeded,
        "the composed services-workspace Rust must compile, source:\n{source}"
    );
}

// ==========================================================================
// The TypeScript half of the compile proof.
//
// The E2 exit criterion is IR neutrality demonstrated by two backends, so a
// corpus that compiles one backend's output and only text-compares the other
// is weakest exactly where the criterion rests. These proofs close that gap.
// ==========================================================================

/// The checked IR of the package named `package` in the entry rooted at
/// `entry`. The snapshots render the same data as JSON; these tests read the
/// typed form so an assertion names a field rather than a substring.
fn checked_ir(entry: &Path, package: &str) -> ridl_ir::v2::Package {
    let mut db = RidlDatabase::default();
    let std = std_package(&mut db);
    let workspace = load_workspace(&mut db, entry)
        .expect("a corpus entry loads")
        .workspace;
    let packages: Vec<Package> = workspace.packages(&db).clone();
    let pkg = packages
        .iter()
        .find(|pkg| pkg.name(&db) == package)
        .unwrap_or_else(|| panic!("the entry has a package named {package}"));
    check_package(&db, workspace, *pkg, std).ir
}

/// The generated TypeScript of every package in the entry rooted at `entry`, as
/// `(package-name, typescript-source)` pairs, in workspace order — the
/// TypeScript counterpart of [`generated_packages`]. One file per package,
/// because the emitted cross-package imports are module specifiers
/// (`import * as fleet_contracts from './fleet.contracts'`) that only resolve
/// against a file of that name.
fn generated_typescript_packages(entry: &Path) -> Vec<(String, String)> {
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
            let generated = ridl_backend_ts::generate(&checked.ir)
                .expect("a clean entry's IR generates TypeScript");
            (name, generated.source)
        })
        .collect()
}

/// Finds a runnable tsc: the `tsc` binary on PATH first, then
/// `npx --no-install tsc` (network-free). Returns the program and its leading
/// arguments, or `None` when neither responds to `--version`.
///
/// Mirrors `discover_tsc` in `backends/typescript/src/tests.rs` exactly — that
/// helper is `pub(crate)` inside a `#[cfg(test)]` module and so is not
/// reachable from an integration test in another crate. Deliberately the same
/// mechanism rather than a second one: skip-if-absent is the established
/// convention for the TypeScript proofs in this workspace, which is why Node is
/// not a hard dependency of `cargo test`.
fn discover_tsc() -> Option<(String, Vec<String>)> {
    let candidates: [(&str, &[&str]); 2] = [("tsc", &[]), ("npx", &["--no-install", "tsc"])];
    for (program, prefix) in candidates {
        let probe = std::process::Command::new(program)
            .args(prefix)
            .arg("--version")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
        if matches!(probe, Ok(status) if status.success()) {
            return Some((
                program.to_string(),
                prefix.iter().map(|s| s.to_string()).collect(),
            ));
        }
    }
    None
}

/// Type-checks the generated TypeScript of `entry` under `tsc --strict`.
///
/// Two properties, in this order, and the order matters:
///
/// 1. **`markers` are present in the generated source.** This runs whether or
///    not tsc is discoverable, so the test still asserts something on a machine
///    with no Node — an empty or truncated emission fails here rather than
///    type-checking vacuously.
/// 2. **tsc accepts it**, when tsc is discoverable. When it is not, the test
///    prints a skip notice naming the entry and the reason, and returns. A
///    silent skip is indistinguishable from a pass, which is the failure family
///    this corpus exists to catch; run `cargo test -- --nocapture` to see the
///    notice.
fn type_check_entry(label: &str, entry: &Path, markers: &[&str]) {
    let packages = generated_typescript_packages(entry);
    let all: String = packages
        .iter()
        .map(|(_, source)| source.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    for marker in markers {
        assert!(
            all.contains(marker),
            "the generated TypeScript for {label} must contain `{marker}`, \
             or this proof type-checks nothing",
        );
    }

    let Some(tsc) = discover_tsc() else {
        println!(
            "SKIPPED tsc --strict for corpus entry `{label}`: no tsc binary discoverable \
             (`tsc` on PATH or `npx --no-install tsc`). The generated-source markers were \
             still checked; the TypeScript snapshot remains the gate."
        );
        return;
    };

    let dir = std::env::temp_dir().join(format!(
        "ridlc_corpus_tsc_{label}_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock reads a time after the unix epoch")
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).expect("the temp dir is created");

    // The hand-written stand-in for the generated `ridl.std` module, committed
    // beside this test rather than inlined, because it is shared by every entry
    // and is long enough that a reader should be able to open it.
    let prelude = std::fs::read_to_string("tests/tsc/ridl.std.ts")
        .expect("the ridl.std test stand-in is readable");
    std::fs::write(dir.join("ridl.std.ts"), prelude).expect("the stand-in is written");

    let mut module_paths = Vec::new();
    for (name, source) in &packages {
        let path = dir.join(format!("{name}.ts"));
        std::fs::write(&path, source).expect("the generated module is written");
        module_paths.push(path);
    }

    let status = std::process::Command::new(&tsc.0)
        .args(&tsc.1)
        .args([
            "--noEmit", "--strict", "--target", "es2020", "--module", "commonjs",
        ])
        .args(&module_paths)
        .status()
        .expect("the discovered tsc must be runnable");

    let succeeded = status.success();
    std::fs::remove_dir_all(&dir).ok();
    assert!(
        succeeded,
        "the generated TypeScript for {label} must type-check under `tsc --strict`, source:\n{all}"
    );
}

/// The generated TypeScript for `veh-cluster` type-checks under `tsc --strict`
/// — the counterpart of [`veh_cluster_generated_rust_compiles_with_rustc`], so
/// the entry that exercises the whole interaction surface is held to
/// compilation by both backends rather than to compilation by one and text
/// comparison by the other.
#[test]
fn veh_cluster_generated_typescript_type_checks() {
    type_check_entry(
        "veh-cluster",
        Path::new("tests/corpus/veh-cluster"),
        &[
            "export interface VehicleStatusConsumer",
            "export interface Service_veh_hvac_cabinProvider",
            "export const service_veh_hvac_cabinTiming",
            "export const service_veh_hvac_cabinContracts",
            "export const services",
        ],
    );
}

/// The generated TypeScript for `services-workspace` type-checks under
/// `tsc --strict`, including the cross-package module import the emitter writes
/// for an inline shape whose whole vocabulary lives in a sibling member.
#[test]
fn services_workspace_generated_typescript_type_checks() {
    type_check_entry(
        "services-workspace",
        Path::new("tests/corpus/services-workspace"),
        &[
            "import * as fleet_contracts from './fleet.contracts'",
            "export interface DoorControlProvider",
            "export interface Service_fleet_vehicle_mirrorsConsumer",
            "export const service_fleet_vehicle_mirrorsContracts",
        ],
    );
}

/// `internal` on an interface maps to the target's package-private mechanism
/// in **both** backends — ADR-0002 §8, ADR-0008 decision 7.
///
/// **This test pinned a defect until PR #160.** Between PRs #155/#156 and #160
/// the interaction layer dropped `internal` in both backends: the checker
/// recorded it (`Interface.visibility = 2`) and `ridl diff` modelled it, but
/// the Rust backend emitted `pub` and the TypeScript backend `export` for an
/// `internal interface`, so a package-private contract shape leaked into both
/// generated public surfaces. This entry is what made that visible — the
/// defect shipped in two merged PRs and was found by adding an
/// `internal interface` to the corpus, not by review of either backend. The
/// assertions below are now the regression guard for the repair.
///
/// `veh-cluster` carries both halves so this test can be exact: `internal
/// interface WheelDiagnostics` (the interaction layer) and `internal struct
/// RawWheelFrame` in the typl layer (the control, handled correctly
/// throughout). The control is what makes this a statement about the
/// interaction layer rather than about `internal` in general — and it is what
/// bounded the original diagnosis, since it proved the vocabulary layer was
/// already right.
///
/// All four generated names of an interface are asserted, per backend, because
/// the contracts constant was the one originally overlooked: the first report
/// named only the two faces and the timing constant.
#[test]
fn internal_on_an_interface_is_package_private_in_both_backends() {
    let compiled = compile_entry(Path::new("tests/corpus/veh-cluster"));

    // The control: the typl layer honours `internal` in both backends. If
    // either of these fails, the finding below is not about the interaction
    // layer and this whole test needs rereading.
    assert!(
        compiled.rust.contains("pub(crate) struct RawWheelFrame"),
        "control: an `internal` typl declaration must stay crate-private in Rust",
    );
    assert!(
        compiled.typescript.contains("\ninterface RawWheelFrame")
            && !compiled
                .typescript
                .contains("export interface RawWheelFrame"),
        "control: an `internal` typl declaration must stay unexported in TypeScript",
    );

    // Rust. `WheelDiagnostics` is declared `internal`, so all four of the names
    // it generates are `pub(crate)`.
    for item in [
        "pub(crate) trait WheelDiagnosticsConsumer",
        "pub(crate) trait WheelDiagnosticsProvider",
        "pub(crate) const WHEEL_DIAGNOSTICS_TIMING",
        "pub(crate) const WHEEL_DIAGNOSTICS_CONTRACTS",
    ] {
        assert!(
            compiled.rust.contains(item),
            "an `internal` interface must generate `{item}` (ADR-0008 decision 7)",
        );
    }
    // ... and none of them under the `pub` spelling this test used to pin.
    // `pub(crate) trait X` does not contain `pub trait X`, so these are genuine
    // negatives rather than restatements of the assertions above.
    for leaked in [
        "pub trait WheelDiagnosticsConsumer",
        "pub trait WheelDiagnosticsProvider",
        "pub const WHEEL_DIAGNOSTICS_TIMING",
        "pub const WHEEL_DIAGNOSTICS_CONTRACTS",
    ] {
        assert!(
            !compiled.rust.contains(leaked),
            "an `internal` interface must not generate `{leaked}` — that is the \
             pre-#160 defect this entry was added to catch",
        );
    }

    // TypeScript. The same four names, none of them exported.
    for item in [
        "\ninterface WheelDiagnosticsConsumer",
        "\ninterface WheelDiagnosticsProvider",
        "\nconst wheelDiagnosticsTiming",
        "\nconst wheelDiagnosticsContracts",
    ] {
        assert!(
            compiled.typescript.contains(item),
            "an `internal` interface must generate `{}` unexported (ADR-0008 decision 7)",
            item.trim_start(),
        );
    }
    for leaked in [
        "export interface WheelDiagnosticsConsumer",
        "export interface WheelDiagnosticsProvider",
        "export const wheelDiagnosticsTiming",
        "export const wheelDiagnosticsContracts",
    ] {
        assert!(
            !compiled.typescript.contains(leaked),
            "an `internal` interface must not generate `{leaked}` — that is the \
             pre-#160 defect this entry was added to catch",
        );
    }

    // The public shape beside it, so the two are known to differ in source and
    // to be identical in output — which is the finding, stated as an assertion
    // rather than left to a reader comparing two snapshot regions.
    assert!(
        compiled.rust.contains("pub trait WheelSummaryConsumer")
            && compiled
                .typescript
                .contains("export interface WheelSummaryConsumer"),
        "the public `WheelSummary` is the comparison point for `WheelDiagnostics`",
    );
}

// ==========================================================================
// Message-level and IR-level pins.
//
// The snapshots record everything below, but a snapshot is reviewed by a human
// and set equality over diagnostic codes is reviewed by a machine that cannot
// tell two FORM-102s apart. These tests name the property each construct is in
// the corpus for, so a regression reports which guarantee broke rather than
// which bytes changed.
// ==========================================================================

/// The interaction boundary narrows the typl type surface: optional in any
/// interaction position, a map in any position, and a collection in return
/// position are all rejected. E5's function signatures and E7's registry
/// inherit that rule, and it had no corpus instance before `narrowing.ridl`.
///
/// Asserted on the messages, not the codes. These paths report FORM-101 and
/// FORM-102, both of which the showcase raises from several other places, so
/// the set-equality guard cannot see one of them starting to accept — which is
/// precisely the silent-acceptance direction.
#[test]
fn showcase_pins_the_interaction_boundary_narrowings() {
    let rendered = compile_entry(Path::new("tests/corpus/ridl-diag-showcase")).diagnostics;

    for (construct, expected) in [
        (
            "optional signal payload",
            "error[FORM-102]: signal payload must be a named type",
        ),
        (
            "optional event payload",
            "error[FORM-102]: event payload must be a named type",
        ),
        (
            "optional command parameter",
            "error[FORM-102]: command parameter must be a named type or a stream",
        ),
        (
            "optional final payload",
            "error[FORM-102]: final payload must be a named type or an array",
        ),
        // The map parameter shares the command-parameter message; the count
        // assertion below is what distinguishes the two occurrences.
        (
            "map command parameter",
            "error[FORM-102]: command parameter must be a named type or a stream",
        ),
        (
            "collection return type",
            "error[FORM-101]: expected a return type",
        ),
    ] {
        assert!(
            rendered.contains(expected),
            "the interaction boundary must still reject a {construct}: `{expected}` is gone from \
             the showcase diagnostics",
        );
    }

    // Two command parameters are rejected — the optional one and the map one.
    // Without this the optional case could start being accepted and the map
    // case would keep the message alive.
    assert_eq!(
        rendered
            .matches("error[FORM-102]: command parameter must be a named type or a stream")
            .count(),
        2,
        "both the optional and the map command parameter must be rejected",
    );
}

/// A `reserved` tombstone holds its ordinal for ever, in both the named-form
/// (`reserved legacyWheelPhase`) and the nameless form (`reserved 4`), and in
/// both interaction stores — a named `interface` and a service's inline shape,
/// which lower through separate loops.
///
/// The load-bearing consumer is `tools/diff/src/classify.rs::slots`, which
/// folds tombstones into the ordinal sequence so a freed slot never looks free.
/// Without that, a new interaction reusing a retired wire identity classifies
/// as a clean append — the identity-reuse guarantee a registry gate rests on.
/// The assertion here is on the *gap*: the interaction after two tombstones
/// must carry ordinal 5, not ordinal 3.
#[test]
fn tombstones_hold_their_ordinals_in_both_interaction_stores() {
    use ridl_ir::v2::decl::Kind;

    let ir = checked_ir(Path::new("tests/corpus/veh-cluster"), "veh.cluster");

    // `Package::shapes` keys a named interface on its own name and an inline
    // shape on the owning service's dotted name, so one lookup finds both.
    let named = ir
        .shapes()
        .find(|shape| shape.name == "WheelHistory")
        .expect("the tombstone interface is in the corpus")
        .interface;
    let inline = ir
        .shapes()
        .find(|shape| shape.name == "veh.cluster.wheels")
        .expect("the tombstone inline shape is in the corpus")
        .interface;

    for (store, interface) in [("interface", named), ("inline shape", inline)] {
        let reserved: Vec<(u32, Option<String>, Option<i64>)> = interface
            .interactions
            .iter()
            .filter_map(|decl| match &decl.kind {
                Some(Kind::ReservedSlot(slot)) => {
                    Some((decl.ordinal, slot.name.clone(), slot.value))
                }
                _ => None,
            })
            .collect();
        assert_eq!(
            reserved.len(),
            2,
            "{store}: both tombstone forms must survive lowering, got {reserved:?}",
        );
        assert!(
            reserved.iter().any(|(_, name, _)| name.is_some()),
            "{store}: the named form must keep its retired name, got {reserved:?}",
        );
        assert!(
            reserved
                .iter()
                .any(|(ordinal, name, value)| name.is_none() && value == &Some(i64::from(*ordinal))),
            "{store}: the nameless form must keep the ordinal it protects, got {reserved:?}",
        );
    }

    // The gap, stated directly: two tombstones sit at ordinals 2 and 4, so the
    // last live interaction is 5. If a tombstone ever stopped consuming a slot
    // this would read 3 and a retired wire identity would be reissuable.
    let last = named
        .interactions
        .last()
        .expect("WheelHistory has interactions");
    assert_eq!(
        (last.name.as_str(), last.ordinal),
        ("wheelSlipRate", 5),
        "a tombstone must consume an ordinal slot",
    );
}

/// The synthesized transport identity of an inline `T | E` return has two
/// shapes, and a registry keys a wire contract on the string (ADR-0008
/// decision 4), so both belong in the golden record:
///
/// - **bare**, when both arms are declared in the same package;
/// - **fully qualified**, when they are imported from another package.
///
/// The third assertion is the stability property: an import *alias* is a
/// source-level convenience and must be canonicalized away, so a consumer
/// cannot change a wire identity by renaming its own import.
#[test]
fn transport_identity_carries_both_arm_spellings() {
    let rust = compile_entry(Path::new("tests/corpus/services-workspace")).rust;

    for (property, identity) in [
        (
            "same-package arms stay bare",
            "transport identity: DoorControl#4:DoorReport|DoorFault",
        ),
        (
            "cross-package arms are fully qualified",
            "transport identity: fleet.vehicle.mirrors#5:fleet.contracts.DoorReport|fleet.contracts.DoorFault",
        ),
        (
            "an import alias is canonicalized away",
            "transport identity: fleet.vehicle.mirrors#6:fleet.contracts.DoorReport|fleet.legacy.DoorFault",
        ),
    ] {
        assert!(
            rust.contains(identity),
            "{property}: `{identity}` is not in the generated Rust",
        );
    }

    // The alias is genuinely written in source — otherwise the third assertion
    // above proves nothing about canonicalization.
    let source = std::fs::read_to_string("tests/corpus/services-workspace/vehicle/publish.ridl")
        .expect("the publishing member is readable");
    assert!(
        source.contains("import fleet.legacy.DoorFault as LegacyFault")
            && source.contains(": DoorReport | LegacyFault"),
        "the aliased arm must be written through its alias in source",
    );
}

/// The guaranteed expression subset (ridl §13) is exercised end to end.
/// Comparison, boolean connectives, enum access, tuple-field access and
/// duration comparison were already in the corpus; conjunction, arithmetic and
/// a reference to a declared `const` were not. E5.1 restructures
/// `Contract.source` from canonical text into an expression tree with this
/// corpus as its regression set, so a form with no instance here is a form that
/// restructure would land without ever having been exercised.
#[test]
fn contract_clauses_cover_the_guaranteed_expression_subset() {
    let ir = checked_ir(Path::new("tests/corpus/veh-cluster"), "veh.cluster");
    let sources: Vec<String> = ir
        .shapes()
        .flat_map(|shape| shape.interface.interactions.iter())
        .flat_map(|decl| match &decl.kind {
            Some(ridl_ir::v2::decl::Kind::CommandDef(command)) => command.contracts.clone(),
            Some(ridl_ir::v2::decl::Kind::QueryDef(query)) => query.contracts.clone(),
            _ => Vec::new(),
        })
        .map(|contract| contract.source)
        .collect();

    for (form, needle) in [
        ("boolean conjunction", "level >= 0 && level <= 7"),
        ("boolean disjunction", "||"),
        ("arithmetic over a parameter", "level + 1 <= 7"),
        ("arithmetic over `result`", "result * 2 >= 0"),
        ("a reference to a declared `const`", "level <= MAX_FAN"),
        ("enum member access", "GearPosition.PARK"),
        ("tuple-field access on `result`", "result.min <= result.max"),
        ("duration comparison", "window > 0ms"),
    ] {
        assert!(
            sources.iter().any(|source| source.contains(needle)),
            "the corpus must exercise {form}: no contract clause contains `{needle}`",
        );
    }
}

/// **This test pins a defect, deliberately.**
///
/// typl Appendix E defines `reserved = "reserved" ( camelCase_id | int_lit )`,
/// but the parser accepts *any* literal in that position and the lowering keeps
/// only integers. `reserved "oldName"`, `reserved true` and `reserved 1.5` are
/// therefore accepted with **no diagnostic at all** and lower to
/// `Reserved { name: None, value: None }` — a slot that still holds its ordinal
/// (so the identity-reuse guarantee survives) but records nothing about what
/// was retired, which is what `ridl diff` needs to report a dangling tombstone
/// (TYPL-211).
///
/// Input outside the grammar accepted in silence is the failure family this
/// corpus exists to find, so it is recorded rather than worked around. The
/// corpus files themselves use only the two grammatical forms; this test drives
/// `compile` directly so the golden entries stay grammatical.
///
/// When the parser learns to reject these, flip the assertions to expect a
/// diagnostic and update `veh-cluster/NOTES`, section "a non-integer `reserved`
/// literal is accepted and discarded".
#[test]
fn reserved_accepts_ungrammatical_literals_and_discards_them() {
    for literal in ["\"oldName\"", "true", "1.5"] {
        let source = format!(
            "package app\ntype L: integer [0..7]\ninterface I {{\n  signal a : L @1s\n  \
             reserved {literal}\n  signal b : L @1s\n}}\n"
        );
        let output = ridlc::compile("app.ridl", &source);

        let codes: Vec<&str> = output
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic.code.as_str())
            .collect();
        assert!(
            codes.is_empty(),
            "the parser has learned to reject `reserved {literal}` — good. Flip this assertion \
             and update `veh-cluster/NOTES`. Got: {codes:?}",
        );

        let slot = output.package.interfaces[0]
            .interactions
            .iter()
            .find_map(|decl| match &decl.kind {
                Some(ridl_ir::v2::decl::Kind::ReservedSlot(slot)) => Some(slot),
                _ => None,
            })
            .expect("the tombstone lowers");
        assert_eq!(
            (slot.ordinal, slot.name.as_deref(), slot.value),
            (2, None, None),
            "`reserved {literal}` holds its ordinal but discards what was retired",
        );
    }
}
