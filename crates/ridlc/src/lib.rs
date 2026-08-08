//! The `ridlc` compile pipeline as a library (docs/ROADMAP.md epics E0.9,
//! E1.10, E1.7a, E1.13).
//!
//! [`compile`] runs the pipeline end to end over a single source file: it wraps
//! the source in a single-file synthetic package, resolves it
//! ([`resolve_package`]), checks and lowers it to IR v2 ([`check_package`]),
//! and generates Rust source. The function is total: it never panics. Every
//! parser, resolver, and checker diagnostic is a coded [`Diagnostic`]
//! collected into [`CompileOutput::diagnostics`]; if the Rust backend fails,
//! its error joins that list and [`CompileOutput::rust_source`] is left
//! empty. The caller (the CLI or a test) renders the diagnostics against
//! [`CompileOutput::sources`] and decides what a non-empty diagnostic list
//! means.
//!
//! [`compile_workspace`] is the same pipeline over the loaded package model —
//! a `.typl` file, a package directory, or a workspace root ([`load_workspace`])
//! — returning the per-package IR and the merged, render-ready diagnostics
//! (load + parse + resolve + check). It is the library face the language server
//! (E1.15) drives; it performs no network or lockfile side effects.
//!
//! [`run_check`] and [`run_build`] are the stable command drivers shared by the
//! `ridlc` plumbing binary and the `ridl` porcelain facade (concept note §8.1):
//! they add the remote-import lockfile round trip on top of `compile_workspace`
//! and, for `build`, write the selected [`Emit`] artifacts.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use ridl_core::db::InputFile;
use ridl_core::diag::{
    DiagCode, Diagnostic, FileId, Severity, SourceMap, Span, house_style_message, remap_diagnostics,
};
use ridl_core::package::{Package, PackageOrigin, Workspace, service_catalog};
use ridl_core::{
    Cache, Frozen, LoadedWorkspace, RidlDatabase, load_workspace, materialize_imports, parse_file,
    read_lockfile, std_package, write_lockfile,
};
use ridl_sem::{CheckedPackage, Resolution, check_package, resolve_package};
use ridl_syntax::ast::{AstNode as _, SourceFile};
use rowan::TextRange;

/// The result of [`compile`]: the generated Rust source, the lowered IR v2
/// package, every coded diagnostic, and the source map the diagnostics point
/// into (for rendering).
pub struct CompileOutput {
    pub rust_source: String,
    pub package: ridl_ir::v2::Package,
    pub diagnostics: Vec<Diagnostic>,
    pub sources: SourceMap,
}

/// Compiles `text` (registered under `path`) end to end.
///
/// The pipeline is `parse_file` (through the salsa database) →
/// `resolve_package` → `check_package` → `generate`. Diagnostics are
/// concatenated in that order: parser errors first, then resolver, then
/// checker, then any Rust backend error. The source becomes a single-file
/// synthetic package named from its `package` declaration, falling back to
/// the path's file stem — the loader's single-file rule (E1.3).
///
/// The package-scoped passes stamp their spans with a [`FileId`] indexing the
/// package's files in order; [`remap_diagnostics`] rewrites them onto this
/// function's own [`SourceMap`] before they are merged.
pub fn compile(path: &str, text: &str) -> CompileOutput {
    let mut db = RidlDatabase::default();
    let std = std_package(&mut db);
    let input = InputFile::new(&db, path.to_string(), text.to_string());
    let parse = parse_file(&db, input);

    let mut sources = SourceMap::new();
    let file = sources.file_id(path, text);

    // Parser diagnostics carry the raw message; `house_style_message` polishes
    // the `expect`-path Debug forms into the backticked house style (issue #102).
    let mut diagnostics: Vec<Diagnostic> = parse
        .errors()
        .iter()
        .map(|error| {
            error_diagnostic(
                error.code,
                ridl_core::diag::house_style_message(&error.message),
                file,
                error.range,
            )
        })
        .collect();

    let ast = SourceFile::cast(parse.syntax()).expect("parser roots every tree in a SourceFile");
    let package_name = declared_package_name(&ast).unwrap_or_else(|| module_name_from_path(path));
    let pkg = Package::new(
        &db,
        package_name,
        vec![input],
        PackageOrigin::WorkspaceMember,
        BTreeMap::new(),
        None,
    );
    let ws = Workspace::new(&db, vec![pkg], BTreeMap::new());

    let resolution = resolve_package(&db, ws, pkg, std);
    let checked = check_package(&db, ws, pkg, std);

    // The single package file is the file interned above.
    let render_ids = vec![file];
    diagnostics.extend(remap_diagnostics(resolution.diagnostics, &render_ids));
    diagnostics.extend(remap_diagnostics(checked.diagnostics, &render_ids));

    let rust_source = match ridl_backend_rust::generate(&checked.ir) {
        // The E1.12 backend returns Rust plus a C header; this pre-CLI plumbing
        // path keeps only the Rust source. Task 20 wires the C header emit.
        Ok(generated) => generated.rust_source,
        Err(err) => {
            // The backend does not carry source ranges yet, so its diagnostic
            // has no code and points at the file start.
            diagnostics.push(error_diagnostic(
                "",
                err.message,
                file,
                TextRange::default(),
            ));
            String::new()
        }
    };

    CompileOutput {
        rust_source,
        package: checked.ir,
        diagnostics,
        sources,
    }
}

/// The dotted name of the file's `package` declaration, with trivia between
/// its tokens dropped; `None` when no declaration parses (the parser already
/// reported FORM-104).
fn declared_package_name(ast: &SourceFile) -> Option<String> {
    let name = ast.package_decl()?.qualified_name()?;
    let text: String = name
        .syntax()
        .descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .filter(|token| !token.kind().is_trivia())
        .map(|token| token.text().to_string())
        .collect();
    (!text.is_empty()).then_some(text)
}

/// Builds an error-severity [`Diagnostic`] from a pass's `code`, `message`,
/// and source `range`.
fn error_diagnostic(
    code: &'static str,
    message: String,
    file: FileId,
    range: TextRange,
) -> Diagnostic {
    Diagnostic {
        code: DiagCode(code),
        severity: Severity::Error,
        message,
        primary: Span { file, range },
        labels: Vec::new(),
        fixits: Vec::new(),
    }
}

/// Derives a module name from the input path's file stem, e.g.
/// `walking_skeleton.typl` becomes `walking_skeleton`; falls back to `module`.
pub fn module_name_from_path(path: &str) -> String {
    std::path::Path::new(path)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("module")
        .to_string()
}

// ==========================================================================
// Workspace compile — the library face for the CLIs and the LSP (E1.13).
// ==========================================================================

/// The result of [`compile_workspace`]: the checked, lowered IR for every
/// package in the loaded workspace, the merged render-ready diagnostics, and the
/// source map the diagnostics point into.
///
/// `diagnostics` gathers every diagnostic the workspace produced — the loader's
/// (manifest and package↔directory law), the parser's, the resolver's, and the
/// checker's — each already remapped onto `sources`, so a caller renders them
/// with [`render`](ridl_core::diag::render()) and keys the exit code on the
/// presence of an [`Error`](Severity::Error). `checked` carries the per-package
/// IR so the language server can serve it (E1.15).
///
/// `resolutions` and `std_ir` exist so a consumer can resolve a name the way the
/// checker did rather than the way one package's `decls` happen to spell it
/// (ADR-0008 decision 15). A lowered IR reference is canonical — bare for a
/// same-package declaration, `package.Name` across packages — so `checked` plus
/// `std_ir` is the complete set of packages a canonical reference can name, and
/// `resolutions` is the only place the **written** name of an import (its alias)
/// is mapped back onto the declaration it stands for.
pub struct WorkspaceOutput {
    pub checked: Vec<CheckedPackage>,
    /// Each checked package's resolved local name view, in `checked` order and
    /// one per entry — the two are filled in the same loop, so index `i` of one
    /// always describes index `i` of the other.
    ///
    /// Positional rather than keyed by package name **because a package name is
    /// not a key**: two workspace members may declare the same `[package] name`,
    /// which the toolchain currently accepts with no diagnostic at all, and a
    /// name-keyed map would then silently hand one member the other's view.
    ///
    /// Each entry is what the resolver built (the package's own declarations,
    /// `ridl.std`, and the alias-aware imports, ADR-0002 §5). Its keys are
    /// **local** spellings, so `import fleet.legacy.DoorFault as LegacyFault` is
    /// keyed `LegacyFault` while the [`Symbol`](ridl_sem::Symbol) it maps to
    /// still names `fleet.legacy.DoorFault`. Resolving a written name any other
    /// way — by scanning packages for a matching declared name, say — mis-binds
    /// under an alias and under a cross-package name collision.
    pub resolutions: Vec<Resolution>,
    /// The lowered IR of the built-in `ridl.std` package (typl Appendix A).
    ///
    /// `ridl.std` is deliberately absent from
    /// [`Workspace::packages`](ridl_core::package::Workspace::packages) and is
    /// threaded through the passes as a parameter, so it never appears in
    /// `checked`. A canonical reference can still name it — every package
    /// implicitly imports all of it (typl §3.2) — and without its IR a consumer
    /// resolving `ridl.std.Duration` or `ridl.std.Timestamp` finds nothing.
    ///
    /// Its own diagnostics are not merged into `diagnostics`: the source is
    /// compiled into the binary and version-locked to it, so a finding there is
    /// a compiler defect and not a statement about the user's workspace. It is
    /// covered by `ridl-core`'s own asset tests.
    pub std_ir: ridl_ir::v2::Package,
    pub diagnostics: Vec<Diagnostic>,
    pub sources: SourceMap,
}

/// Loads the workspace reachable from `entry`, then resolves and checks every
/// package in it.
///
/// `entry` is a `.typl` file (single-file mode), a package directory, or a
/// workspace root — whatever [`load_workspace`] accepts. The function performs
/// no network or lockfile side effects: remote-import materialization and the
/// `ridl.lock` round trip live in the command drivers ([`run_check`],
/// [`run_build`]), so the language server can drive this on every edit without
/// touching the filesystem beyond the initial load.
///
/// `Err` is reserved for a filesystem failure while loading (the entry does not
/// exist, or a file cannot be read); every content problem is a [`Diagnostic`].
pub fn compile_workspace(db: &mut RidlDatabase, entry: &Path) -> std::io::Result<WorkspaceOutput> {
    let Compiled {
        workspace,
        std,
        checked,
        resolutions,
        diagnostics,
        sources,
    } = load_and_check(db, entry)?;
    // `ridl.std` is checked here rather than in `load_and_check` so the command
    // drivers, which never look at its IR, do not pay for the pass.
    let std_ir = check_package(&*db, workspace, std, std).ir;
    Ok(WorkspaceOutput {
        checked,
        resolutions,
        std_ir,
        diagnostics,
        sources,
    })
}

/// A build artifact `ridlc build --emit` can write for each package.
#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
pub enum Emit {
    /// Idiomatic Rust source, written to `<base>.rs`.
    Rust,
    /// The extern-C header, written to `<base>.h`.
    CHeader,
    /// The lowered IR v2 as exact-decimal JSON, written to `<base>.ir.json`.
    IrJson,
    /// The lowered IR v2 as prototext, written to `<base>.ir.txtpb`.
    ///
    /// The inspection encoding (ADR-0014 decisions 4 and 9): emittable, but
    /// not a recommended interchange form.
    IrText,
    /// The lowered IR v2 as protobuf binary, written to `<base>.ir.binpb`.
    ///
    /// The canonical interchange encoding (ADR-0014 decisions 4 and 9).
    IrBinary,
    /// Idiomatic TypeScript source, written to `<base>.ts`.
    ///
    /// The flag value is spelled `typescript` rather than the derived
    /// `type-script`: the language emits are named after the language, as
    /// `rust` is.
    ///
    /// The `.ts` extension is forced rather than chosen **in package and
    /// workspace mode**, where `base` is the package name: one generated module
    /// imports another as `./<package-name>`, which resolves only against a
    /// file named `<package-name>.ts`. Single-file mode names artifacts after
    /// the input file stem instead ([`run_build`]), so `ridlc build
    /// common.typl` writes `common.ts` for `package veh.common`, and a sibling
    /// module importing `./veh.common` would not resolve against it. The Rust
    /// emit carries the same asymmetry, and both keep it.
    #[value(name = "typescript")]
    TypeScript,
    /// The proto3 schema, written to `<base>.proto`.
    ///
    /// A wire backend: the typl surface plus the interaction identity table,
    /// and nothing above them (ADR-0013 decision 2).
    Proto,
}

impl Emit {
    /// Whether this artifact is a direct dump of the lowered IR, as opposed to
    /// code a backend generated from it.
    ///
    /// The distinction decides whether [`run_build`] writes the artifact for
    /// `ridl.std`. `ridl.std` is version-locked to the compiler binary
    /// (ADR-0007 decision 15), so it is not part of a workspace's contract
    /// snapshot — a baseline holds the packages the workspace *declares*.
    /// `ridl baseline` is `run_build` with `--emit ir-json`, and `ridl diff`
    /// compiles the other side without `ridl.std`, so writing
    /// `ridl.std.ir.json` would make every diff of an unedited workspace
    /// against its own baseline report `ridl.std` as a removed package. Issue
    /// #190 is about generated *code* failing to compile, and an IR dump is
    /// not code — prototext and binary are dumps by the identical argument
    /// (ADR-0014 decision 10).
    ///
    /// The classification itself is the `match` in [`Emit::ir_dump_suffix`]:
    /// an IR dump is exactly an emit that names an artifact suffix there, so
    /// this predicate and the suffix table cannot disagree.
    pub fn is_ir_dump(self) -> bool {
        self.ir_dump_suffix().is_some()
    }

    /// The artifact suffix of a direct IR dump — `Some(".ir.json")` for
    /// [`Emit::IrJson`] — or `None` for a code emit.
    ///
    /// This `match` is the one table mapping each IR encoding to its artifact
    /// suffix (ADR-0014 decision 4). [`write_emits`] names every IR artifact
    /// through it, and the snapshot surface in `ridl` recognises IR artifacts
    /// by iterating it ([`Emit::ir_dump_suffixes`]), so the writer and the
    /// recognition read one list rather than two that can drift apart (issue
    /// #218 item 4).
    ///
    /// ADR-0014 decision 10 requires the IR-dump classification to be
    /// exhaustive over [`Emit`] with no wildcard arm, so a new encoding left
    /// unclassified is a compile error rather than a spurious `ridl.std`
    /// artifact on every build — or, now that the suffix rides on the same
    /// `match`, an artifact the snapshot surface does not recognise. The two
    /// lints below reject the wildcard rustc's own `help:` text proposes for
    /// that error — the first when it covers several variants, the second
    /// when it covers exactly one, which is the case one unclassified new
    /// variant creates.
    #[deny(
        clippy::wildcard_enum_match_arm,
        clippy::match_wildcard_for_single_variants
    )]
    pub const fn ir_dump_suffix(self) -> Option<&'static str> {
        match self {
            Emit::Rust | Emit::CHeader | Emit::TypeScript | Emit::Proto => None,
            Emit::IrJson => Some(".ir.json"),
            Emit::IrText => Some(".ir.txtpb"),
            Emit::IrBinary => Some(".ir.binpb"),
        }
    }

    /// Every IR dump emit paired with its artifact suffix, in declaration
    /// order. The variant list comes from `clap`'s derive rather than a
    /// hand-kept array, so an encoding classified in [`Emit::ir_dump_suffix`]
    /// joins this iteration with no further wiring.
    pub fn ir_dump_suffixes() -> impl Iterator<Item = (Emit, &'static str)> {
        <Emit as clap::ValueEnum>::value_variants()
            .iter()
            .filter_map(|emit| emit.ir_dump_suffix().map(|suffix| (*emit, suffix)))
    }
}

/// The render-ready result of a [`run_check`] or [`run_build`] command: the
/// merged diagnostics and the source map they point into.
pub struct CliRun {
    pub diagnostics: Vec<Diagnostic>,
    pub sources: SourceMap,
}

impl CliRun {
    /// Whether any diagnostic is an [`Error`](Severity::Error) — the condition
    /// that drives exit code 1.
    pub fn has_error(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == Severity::Error)
    }
}

/// Runs `check`: loads, resolves, and checks the workspace at `entry`, then
/// materializes remote imports against `ridl.lock` (regenerating it on a clean
/// non-frozen run). Returns every diagnostic and the source map for rendering.
pub fn run_check(entry: &Path, frozen: Frozen) -> std::io::Result<CliRun> {
    let mut db = RidlDatabase::default();
    let Compiled {
        workspace,
        mut diagnostics,
        sources,
        ..
    } = load_and_check(&mut db, entry)?;
    diagnostics.extend(materialize_and_lock(&db, workspace, entry, frozen));
    Ok(CliRun {
        diagnostics,
        sources,
    })
}

/// Runs `build`: everything [`run_check`] does, plus it writes the selected
/// `emits` for every checked package into `out_dir` — but only when the whole
/// run produced no error-severity diagnostic, whether from the compile or from
/// remote-import materialization (a manifest, lockfile, or fetch error, MANI-1xx).
/// An error-bearing build renders its diagnostics and exits non-zero without
/// writing any artifact (C1).
///
/// The artifact base name is the file stem in single-file mode (preserving the
/// E0 `<input-stem>.rs` contract) and the full dotted package name otherwise, so
/// a workspace build writing several packages into one directory never has two
/// packages collide on a file name. Each package is generated on its own; a
/// cross-package derivable `Default` therefore needs the referenced package's
/// generated code compiled alongside it (documented, not linked here — E1.13).
pub fn run_build(
    entry: &Path,
    out_dir: &Path,
    emits: &[Emit],
    frozen: Frozen,
) -> std::io::Result<CliRun> {
    let mut db = RidlDatabase::default();
    let Compiled {
        workspace,
        std,
        checked,
        mut diagnostics,
        sources,
        ..
    } = load_and_check(&mut db, entry)?;

    // Materialize remote imports and round-trip the lockfile before the emit
    // gate, so any error it raises (a manifest, lockfile, or fetch problem,
    // MANI-1xx — for example a frozen build with no `ridl.lock`) joins the
    // compile diagnostics and suppresses code generation, exactly like a
    // compile error does.
    diagnostics.extend(materialize_and_lock(&db, workspace, entry, frozen));

    // A build must not emit artifacts for a workspace that failed: code
    // generation over error-bearing IR produces invalid or misleading output,
    // and a malformed IR could even crash a backend (C1). `check` never runs
    // codegen; `build` matches that by skipping every emit — code and IR
    // dumps alike — when any error-severity diagnostic is present, from the
    // compile or from materialization. Warnings and info do not gate.
    let succeeded = !diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == Severity::Error);
    if succeeded {
        std::fs::create_dir_all(out_dir)?;
        let single_file = entry.is_file() && manifest_root_of(entry).is_none();
        let file_stem = module_name_from_path(&entry.to_string_lossy());

        // `ridl.std` is deliberately absent from `checked` (it is not a
        // workspace member), so no loop over `checked` ever reaches it. A
        // consumer's generated code still references it, so the build
        // writes it whenever the workspace names something from it —
        // otherwise the raw output does not compile (issue #190). Computed
        // once, ahead of the per-package loop below, because the proto
        // backend needs it too (next paragraph) and this is the one
        // `check_package` call either use pays for.
        let references_std = checked
            .iter()
            .any(|package| ridl_ir::v2::referenced_packages(&package.ir).contains("ridl.std"));
        // Every emit kind except the direct IR dumps. The reasoning, and the
        // exhaustive classification ADR-0014 decision 10 requires, live on
        // `Emit::is_ir_dump`.
        let code_emits: Vec<Emit> = emits
            .iter()
            .copied()
            .filter(|emit| !emit.is_ir_dump())
            .collect();
        let std_ir = (references_std && !code_emits.is_empty())
            .then(|| check_package(&db, workspace, std, std).ir);

        // Every other package a package's cross-package reference might
        // name: every sibling in the workspace, plus `ridl.std` when
        // present — the proto backend resolves a foreign reference itself
        // rather than leaving it to the target language's own import
        // statement, so it is the one backend among the three that reads
        // this (`write_emits`'s doc comment). The other two ignore it.
        let others: Vec<&ridl_ir::v2::Package> = checked
            .iter()
            .map(|package| &package.ir)
            .chain(std_ir.iter())
            .collect();

        for package in &checked {
            let base = if single_file {
                file_stem.clone()
            } else {
                package.ir.name.clone()
            };
            write_emits(
                out_dir,
                &base,
                &package.ir,
                &others,
                emits,
                &mut diagnostics,
            )?;
        }

        if let Some(std_ir) = &std_ir {
            write_emits(
                out_dir,
                "ridl.std",
                std_ir,
                &others,
                &code_emits,
                &mut diagnostics,
            )?;
        }
    }

    Ok(CliRun {
        diagnostics,
        sources,
    })
}

/// Maps a parser [`SyntaxError`](ridl_syntax::SyntaxError) to a coded
/// [`Diagnostic`] against `file`. The `ridl fmt` facade uses it to render the
/// parse errors of a file it refuses to reformat.
pub fn syntax_error_diagnostic(error: &ridl_syntax::SyntaxError, file: FileId) -> Diagnostic {
    error_diagnostic(
        error.code,
        house_style_message(&error.message),
        file,
        error.range,
    )
}

/// The loaded-and-checked workspace shared by [`compile_workspace`] and the
/// command drivers: the salsa [`Workspace`] and `ridl.std` handles, the
/// per-package checked IR and resolved name views, the merged diagnostics
/// remapped onto `sources`, and that source map.
struct Compiled {
    workspace: Workspace,
    std: Package,
    checked: Vec<CheckedPackage>,
    resolutions: Vec<Resolution>,
    diagnostics: Vec<Diagnostic>,
    sources: SourceMap,
}

/// Loads the workspace at `entry` and runs parse, resolve, and check over every
/// package, merging all diagnostics onto one [`SourceMap`].
fn load_and_check(db: &mut RidlDatabase, entry: &Path) -> std::io::Result<Compiled> {
    let std = std_package(db);
    let LoadedWorkspace {
        workspace,
        mut diagnostics,
        mut sources,
    } = load_workspace(db, entry)?;

    // The load and the queries are done; only shared database access remains.
    let db: &RidlDatabase = db;

    let packages = workspace.packages(db).clone();
    let mut checked = Vec::with_capacity(packages.len());
    let mut resolutions: Vec<Resolution> = Vec::with_capacity(packages.len());
    for pkg in &packages {
        // Intern this package's files into the render source map; their ids are
        // the render targets the package-relative pass diagnostics remap onto.
        let files = pkg.files(db).clone();
        let render_ids: Vec<FileId> = files
            .iter()
            .map(|file| sources.file_id(file.path(db), file.text(db)))
            .collect();

        // The loader keeps only manifest and law findings; the parser errors on
        // each file are collected here, like the single-file `compile` does.
        for (file, file_id) in files.iter().zip(&render_ids) {
            for error in parse_file(db, *file).errors() {
                diagnostics.push(error_diagnostic(
                    error.code,
                    house_style_message(&error.message),
                    *file_id,
                    error.range,
                ));
            }
        }

        let mut resolution = resolve_package(db, workspace, *pkg, std);
        diagnostics.extend(remap_diagnostics(
            std::mem::take(&mut resolution.diagnostics),
            &render_ids,
        ));
        resolutions.push(resolution);

        let checked_pkg = check_package(db, workspace, *pkg, std);
        diagnostics.extend(remap_diagnostics(
            checked_pkg.diagnostics.clone(),
            &render_ids,
        ));
        checked.push(checked_pkg);
    }

    // The workspace-wide service catalog (E2.13): its RIDL-140 duplicate-name
    // diagnostics span the whole workspace, so they carry FileIds indexing
    // every file in package-then-file order — the order `service_catalog`
    // interns them. Rebuild that order onto the render source map and remap,
    // mirroring the per-package remap above.
    let catalog = service_catalog(db, workspace, std);
    if !catalog.diagnostics.is_empty() {
        let mut catalog_render_ids = Vec::new();
        for pkg in &packages {
            for file in pkg.files(db) {
                catalog_render_ids.push(sources.file_id(file.path(db), file.text(db)));
            }
        }
        diagnostics.extend(remap_diagnostics(catalog.diagnostics, &catalog_render_ids));
    }

    Ok(Compiled {
        workspace,
        std,
        checked,
        resolutions,
        diagnostics,
        sources,
    })
}

/// Materializes every remote import of the workspace and round-trips
/// `ridl.lock` at the manifest root (ADR-0002 §5, §7).
///
/// Reads `ridl.lock`, calls [`materialize_imports`] with the given `frozen`
/// mode, and — on a clean non-frozen run — writes the regenerated lockfile back.
/// Under [`Frozen::Yes`] nothing is ever fetched and the lockfile is never
/// rewritten. Single-file mode (no manifest up the tree) and a workspace with no
/// remote imports both short-circuit to no diagnostics and no lockfile.
fn materialize_and_lock(
    db: &RidlDatabase,
    workspace: Workspace,
    entry: &Path,
    frozen: Frozen,
) -> Vec<Diagnostic> {
    let Some(root) = manifest_root_of(entry) else {
        return Vec::new();
    };

    // The union of every import pin — the workspace root's and each package's
    // (ADR-0002 §5). `materialize_imports` reads only the values (URLs) and
    // deduplicates them, so keying the map by URL keeps every distinct URL.
    let mut imports: BTreeMap<String, String> = BTreeMap::new();
    for url in workspace.imports(db).values() {
        imports.insert(url.clone(), url.clone());
    }
    for package in workspace.packages(db) {
        for url in package.imports(db).values() {
            imports.insert(url.clone(), url.clone());
        }
    }
    if imports.is_empty() {
        return Vec::new();
    }

    let lock_path = root.join("ridl.lock");
    let (lock, mut diagnostics) = read_lockfile(&lock_path);
    let (_resolved, regenerated, materialize_diags) =
        materialize_imports(&imports, lock.as_ref(), &Cache::user_default(), frozen);
    let had_error = materialize_diags
        .iter()
        .any(|diagnostic| diagnostic.severity == Severity::Error);
    diagnostics.extend(materialize_diags);

    // A frozen build never regenerates the lockfile (ADR-0002 §7); a failed run
    // must not overwrite the pins it could not verify.
    if frozen == Frozen::No
        && !had_error
        && let Err(err) = write_lockfile(&lock_path, &regenerated)
    {
        diagnostics.push(detached_warning(format!(
            "cannot write `{}`: {err}",
            lock_path.display()
        )));
    }
    diagnostics
}

/// Writes the selected `emits` for one package's IR into `out_dir`.
///
/// The Rust and C-header emits share one [`generate`](ridl_backend_rust::generate)
/// call; a codegen failure is recorded as a diagnostic and those two emits are
/// skipped. The `ir-json` and `ir-text` emits (direct IR dumps) follow the
/// same rule: when the package cannot be rendered in that encoding (ADR-0014
/// decisions 12 and 14), the failure is recorded as a diagnostic and no
/// artifact is written. The `ir-binary` dump has no failure path — binary
/// needs no descriptors and no transcode (ADR-0014 decision 7). The TypeScript
/// emit is a second, independent backend
/// ([`generate`](ridl_backend_ts::generate)) over the same IR, with its own
/// result type and its own failure path — a backend that cannot render this
/// package skips only its own artifact. The proto3 emit is a third,
/// independent backend on the same pattern
/// ([`generate_with`](ridl_backend_proto::generate_with)) — the one backend
/// among the three that resolves a cross-package reference itself rather than
/// leaving it to the target language's own import statement, so it is the
/// one that needs `others`, the caller's full package list (see
/// [`run_build`]).
fn write_emits(
    out_dir: &Path,
    base: &str,
    ir: &ridl_ir::v2::Package,
    others: &[&ridl_ir::v2::Package],
    emits: &[Emit],
    diagnostics: &mut Vec<Diagnostic>,
) -> std::io::Result<()> {
    let need_codegen = emits
        .iter()
        .any(|emit| matches!(emit, Emit::Rust | Emit::CHeader));
    let generated = if need_codegen {
        match ridl_backend_rust::generate(ir) {
            Ok(generated) => Some(generated),
            Err(err) => {
                diagnostics.push(error_diagnostic(
                    "",
                    err.message,
                    FileId::DETACHED,
                    TextRange::default(),
                ));
                None
            }
        }
    } else {
        None
    };

    let generated_ts = if emits.iter().any(|emit| matches!(emit, Emit::TypeScript)) {
        match ridl_backend_ts::generate(ir) {
            Ok(generated) => Some(generated),
            Err(ridl_backend_ts::GenerateError::Unrepresentable(message)) => {
                diagnostics.push(error_diagnostic(
                    "",
                    message,
                    FileId::DETACHED,
                    TextRange::default(),
                ));
                None
            }
        }
    } else {
        None
    };

    let generated_proto = if emits.iter().any(|emit| matches!(emit, Emit::Proto)) {
        match ridl_backend_proto::generate_with(ir, others) {
            Ok(generated) => Some(generated),
            Err(err) => {
                diagnostics.push(error_diagnostic(
                    "",
                    err.message,
                    FileId::DETACHED,
                    TextRange::default(),
                ));
                None
            }
        }
    } else {
        None
    };

    for emit in emits {
        // The wildcard-free discipline of `Emit::ir_dump_suffix` applies here
        // too: without it, a new emit could be classified in the table and
        // wildcarded out of the writer, which no test catches before this
        // crate's test targets build. Same lint pair, same reason.
        #[deny(
            clippy::wildcard_enum_match_arm,
            clippy::match_wildcard_for_single_variants
        )]
        match emit {
            Emit::Rust => {
                if let Some(generated) = &generated {
                    std::fs::write(out_dir.join(format!("{base}.rs")), &generated.rust_source)?;
                }
            }
            Emit::CHeader => {
                if let Some(generated) = &generated {
                    std::fs::write(out_dir.join(format!("{base}.h")), &generated.c_header)?;
                }
            }
            Emit::IrJson => match ridl_ir::v2::to_json_pretty(ir) {
                Ok(json) => {
                    std::fs::write(ir_dump_path(out_dir, base, *emit), json)?;
                }
                // The pbjson-generated writer has no nesting limit (ADR-0014
                // decision 14); its one remaining failure is an enum field
                // holding a discriminant outside the schema — a tool-level
                // failure with no source span, reported the way the
                // TypeScript backend's `Unrepresentable` is, with no artifact
                // written.
                Err(err) => {
                    diagnostics.push(error_diagnostic(
                        "",
                        err.to_string(),
                        FileId::DETACHED,
                        TextRange::default(),
                    ));
                }
            },
            // Prototext still transcodes through the descriptor pool and
            // keeps the recursion-limit failure mode JSON lost (ADR-0014
            // decisions 12 and 14): the failure is a diagnostic and no
            // artifact is written.
            Emit::IrText => match ridl_ir::v2::to_text_format(ir) {
                Ok(text) => {
                    std::fs::write(ir_dump_path(out_dir, base, *emit), text)?;
                }
                Err(err) => {
                    diagnostics.push(error_diagnostic(
                        "",
                        err.to_string(),
                        FileId::DETACHED,
                        TextRange::default(),
                    ));
                }
            },
            // Binary needs no descriptors and no transcode, so it has no
            // recursion-limit failure path (ADR-0014 decision 7).
            Emit::IrBinary => {
                std::fs::write(
                    ir_dump_path(out_dir, base, *emit),
                    ridl_ir::v2::to_binary(ir),
                )?;
            }
            Emit::TypeScript => {
                if let Some(generated) = &generated_ts {
                    std::fs::write(out_dir.join(format!("{base}.ts")), &generated.source)?;
                }
            }
            Emit::Proto => {
                if let Some(generated) = &generated_proto {
                    std::fs::write(
                        out_dir.join(format!("{base}.proto")),
                        &generated.proto_source,
                    )?;
                }
            }
        }
    }
    Ok(())
}

/// The artifact path of one IR dump: `<out_dir>/<base><suffix>`, with the
/// suffix drawn from [`Emit::ir_dump_suffix`] so the writer never spells an
/// extension the snapshot surface does not recognise (issue #218 item 4).
fn ir_dump_path(out_dir: &Path, base: &str, emit: Emit) -> PathBuf {
    let suffix = emit
        .ir_dump_suffix()
        .expect("only IR dump emits name an IR artifact");
    out_dir.join(format!("{base}{suffix}"))
}

/// The manifest root governing `entry` — the nearest directory at or above it
/// that holds a `ridl.toml`, where `ridl.lock` lives. `None` means single-file
/// mode: a `.typl` file with no manifest anywhere up the tree.
fn manifest_root_of(entry: &Path) -> Option<PathBuf> {
    if entry.is_file() {
        entry.parent().and_then(find_ridl_toml_root)
    } else if entry.is_dir() {
        find_ridl_toml_root(entry)
    } else {
        None
    }
}

/// The nearest directory at or above `dir` that contains a `ridl.toml`.
fn find_ridl_toml_root(dir: &Path) -> Option<PathBuf> {
    dir.ancestors()
        .find(|candidate| candidate.join("ridl.toml").is_file())
        .map(Path::to_path_buf)
}

/// A detached warning [`Diagnostic`] — no source span, for a problem (a failed
/// lockfile write) that concerns a file rather than a byte range.
fn detached_warning(message: String) -> Diagnostic {
    Diagnostic {
        code: DiagCode::NONE,
        severity: Severity::Warning,
        message,
        primary: Span {
            file: FileId::DETACHED,
            range: TextRange::default(),
        },
        labels: Vec::new(),
        fixits: Vec::new(),
    }
}
