//! The `ridlc` compile pipeline as a library.
//!
//! [`compile`] runs the pipeline end to end over a single source file: it wraps
//! the source in a single-file synthetic package, resolves it
//! ([`resolve_package`]), checks and lowers it to IR v2 ([`check_package`]),
//! runs the semantic workspace passes ([`check_workspace`]) and design lints
//! ([`check_design_lints`]), and generates Rust source. The function is total:
//! it never panics. Every parser,
//! resolver, and checker diagnostic is a coded [`Diagnostic`]
//! collected into [`CompileOutput::diagnostics`]; if the Rust backend fails,
//! its error joins that list and [`CompileOutput::rust_source`] is left
//! empty. The caller (the CLI or a test) renders the diagnostics against
//! [`CompileOutput::sources`] and decides what a non-empty diagnostic list
//! means.
//!
//! [`check_source`] is [`compile`]'s check-only sibling over the same
//! single-file front end — every check, no Rust generation — returned as a
//! [`CliRun`] rather than a [`CompileOutput`]. It is the single-file oracle
//! behind `ridl mcp`'s `ridl_check` tool. `ridl check --format json` does not
//! call it: it goes through [`run_check`], which loads a workspace from disk,
//! with its `interfaces.lock` and `ridl.lock` files. Once the workspace is
//! loaded, the two faces run the same passes, and they render the result with
//! the same `ridl_core::diag::to_json`.
//!
//! [`compile_workspace`] is the same pipeline over the loaded package model —
//! a `.typl` file, a package directory, or a workspace root ([`ridl_core::load_workspace`])
//! — returning the per-package IR and the merged, render-ready diagnostics
//! (load + parse + resolve + check). It is the library face the language server
//! drives; it reads the remote units of the manifest's `[imports]` from
//! `vendor/`, then the cache, and never fetches or writes `ridl.lock`.
//!
//! [`run_check`] and [`run_build`] are the stable command drivers shared by the
//! `ridlc` plumbing binary and the `ridl` porcelain facade (concept note §8.1):
//! they load the same workspace with the caller's [`Frozen`] mode — a fetch
//! and the `ridl.lock` round trip under [`Frozen::No`] — and, for `build`,
//! write the selected [`Emit`] artifacts. [`run_build_with`]
//! is `run_build` plus the codegen plugins of `--plugin`, run through the
//! process host in [`plugin`] (ADR-0020 decision 10); every code emit but
//! [`Emit::Catalog`], and every plugin, is reached through one contract,
//! [`codegen::Backend`], over the request [`codegen_request`] builds
//! (ADR-0020 decision 9). [`Emit::Catalog`] writes its file directly, with
//! the bytes `ridl_descriptor::lower` returns, and a lowering failure stops
//! the build with exit code 2.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::time::Duration;

pub mod deps;
mod design_lints;
mod link_encodings;
pub use design_lints::{check_design_lints, cohesion_groups};
pub mod diff_side;
pub use diff_side::{DiffSide, DiffSideError, load_diff_side};
pub mod plugin;

/// Serializes the write of a file this test binary then executes against
/// every child it spawns (driftsys/ridl#729).
///
/// `execve` fails with `ETXTBSY` when the file is open for writing in any
/// process. A plugin test writes a shell script and then runs it, and the
/// descriptor of that write lives for the length of the write. A child
/// forked in that instant by another test inherits the descriptor — the
/// descriptor is close-on-exec, but it is closed at the child's `exec`, not
/// at its `fork` — and holds it until then, so the plugin test's own `exec`
/// can land in that gap and fail.
///
/// The write side is taken while the script is written; the read side is
/// taken across every other `Command` this binary spawns. So a script write
/// excludes every fork, while forks do not exclude each other and a `rustc`
/// proof still runs beside another.
///
/// All three spawn sites in `src/` take the read side: the two `rustc`
/// proofs here, and the plugin host itself, which takes it at its own spawn
/// under `cfg(test)` rather than at each call site, so a test that runs a
/// plugin is covered however it is written. A test under `tests/` is a
/// separate process and inherits nothing.
///
/// This cannot be exercised on macOS, which does not raise `ETXTBSY` for a
/// file open for writing — not even one the exec'ing process holds itself —
/// so no test pins it and the window is removed by construction instead.
#[cfg(test)]
pub(crate) static SPAWNS: std::sync::RwLock<()> = std::sync::RwLock::new(());

/// The read side of [`SPAWNS`], held across a `Command` spawn.
#[cfg(test)]
pub(crate) fn spawning() -> std::sync::RwLockReadGuard<'static, ()> {
    SPAWNS
        .read()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

use ridl_core::TimingDefaults;
use ridl_core::db::InputFile;
use ridl_core::diag::{
    DiagCode, Diagnostic, FileId, Severity, SourceMap, Span, house_style_message, remap_diagnostics,
};
use ridl_core::lint::{LintScopes, apply_lint_levels, drop_allowed_by_default};
use ridl_core::package::{Package, PackageOrigin, Workspace};
use ridl_core::{
    Cache, Frozen, LoadedWorkspace, MaterializedUnit, RemoteAccess, RidlDatabase,
    load_workspace_with_remotes, parse_file, std_package,
};
use ridl_ir::codegen::{self, v1};
use ridl_sem::{
    CheckedPackage, CheckedSystem, CheckedWorkspace, Resolution, check_package, check_workspace,
    lower_system, resolve_package, unclaimed_backend_keys,
};
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

/// The checked front end of one source text: parser, resolver, and checker
/// diagnostics, the source map they are remapped onto, and the IR the checker
/// produced.
struct FrontEnd {
    diagnostics: Vec<Diagnostic>,
    sources: SourceMap,
    /// The one file the source was interned as — the backend's error arm
    /// points its diagnostic at it.
    file: FileId,
    ir: ridl_ir::v2::Package,
}

/// Parses, resolves, and checks `text` (registered under `path`) as a
/// single-file synthetic package named from its `package` declaration, falling
/// back to the path's file stem — the loader's single-file rule. The
/// profile follows `path`'s extension. The package is the one member of a
/// synthetic workspace, and [`check_loaded`] runs over it the passes
/// `ridl check` runs after its load: parser → resolver → checker, then the
/// workspace-wide passes, with the spans remapped onto this function's own
/// [`SourceMap`]. What `ridl check` reads from disk is not here: the package
/// has no `interfaces.lock`, so RIDL-409 and RIDL-410 do not arise, no import
/// is materialized against `ridl.lock`, and no `.ridl/baseline/` snapshot is
/// compared.
fn front_end(path: &str, text: &str) -> FrontEnd {
    let mut db = RidlDatabase::default();
    let std = std_package(&mut db);
    let input = InputFile::new(&db, path.to_string(), text.to_string());

    let mut sources = SourceMap::new();
    let file = sources.file_id(path, text);

    let ast = SourceFile::cast(parse_file(&db, input).syntax())
        .expect("parser roots every tree in a SourceFile");
    let package_name = declared_package_name(&ast).unwrap_or_else(|| module_name_from_path(path));
    let pkg = Package::new(
        &db,
        package_name.clone(),
        package_name,
        vec![input],
        PackageOrigin::WorkspaceMember,
        BTreeMap::new(),
        TimingDefaults::default(),
        None,
    );
    let workspace = Workspace::new(&db, vec![pkg], BTreeMap::new());

    let mut compiled = check_loaded(
        &db,
        std,
        LoadedWorkspace {
            workspace,
            diagnostics: Vec::new(),
            sources,
            // No manifest: the registry defaults apply (ADR-0024
            // decision 10).
            lints: LintScopes::default(),
            // No manifest: no header file.
            codegen_header: None,
            backends: BTreeMap::new(),
            report_scope: None,
            // A source text has no directory, so no unit is recorded.
            units: BTreeMap::new(),
            // No manifest: no root package name.
            root_package: None,
            // No manifest: no `[imports]` and no remote unit.
            imports: BTreeMap::new(),
            remote_units: BTreeMap::new(),
        },
    );
    // The callers, `check_source` and `compile`, report their diagnostics, so
    // the levels apply here, with the empty scopes: every lint gets its
    // registry default (ADR-0024 decision 6).
    apply_lint_levels(
        &mut compiled.diagnostics,
        &compiled.sources,
        &compiled.lints,
    );
    let ir = compiled
        .checked
        .into_iter()
        .next()
        .expect("the synthetic workspace has one package")
        .ir;
    FrontEnd {
        diagnostics: compiled.diagnostics,
        sources: compiled.sources,
        file,
        ir,
    }
}

/// Checks `text` (registered under `path`) without running any backend: the
/// single-file oracle behind `ridl mcp`'s `ridl_check` tool. `ridl check
/// --format json` calls [`run_check`] instead, which loads a workspace from
/// disk; once it is loaded, the two run the same passes, through one private
/// driver. The source has no `interfaces.lock` and no `ridl.lock`, so the
/// checks of those two files do not run here.
pub fn check_source(path: &str, text: &str) -> CliRun {
    let front = front_end(path, text);
    CliRun {
        diagnostics: front.diagnostics,
        sources: front.sources,
        lints: LintScopes::default(),
        usage_error: false,
    }
}

/// Compiles `text` (registered under `path`) end to end.
///
/// The pipeline is `parse_file` (through the salsa database) →
/// `resolve_package` → `check_package` → `check_workspace` →
/// `check_design_lints` → `generate`.
/// Diagnostics are concatenated in that order: parser errors first, then
/// resolver, then checker, then semantic workspace passes, RSDL-804 and design
/// lints, then any Rust backend error. The source becomes a single-file synthetic package
/// named from its `package` declaration, falling back to the path's file stem
/// — the loader's single-file rule.
///
/// The package-scoped passes stamp their spans with a [`FileId`] indexing the
/// package's files in order; [`remap_diagnostics`] rewrites them onto the
/// [`SourceMap`] `front_end` created, before they are merged.
pub fn compile(path: &str, text: &str) -> CompileOutput {
    let FrontEnd {
        mut diagnostics,
        sources,
        file,
        ir,
    } = front_end(path, text);

    let rust_source = match ridl_backend_rust::generate(&ir) {
        // The Rust backend returns Rust plus a C header; this pre-CLI plumbing
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
        package: ir,
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
// Workspace compile — the library face for the CLIs and the LSP.
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
/// IR so the language server can serve it.
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
    /// Manifest import maps, in the same order as `checked`.
    pub imports: Vec<BTreeMap<String, String>>,
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
    /// The lowered rsdl system (rsdl reference §13), or `None` when the
    /// workspace declares no `system` or an error in its closure blocks the
    /// lowering. A deployment an RSDL-7xx error blocks is absent from it.
    pub system: Option<ridl_ir::v2::System>,
    /// The name of every `deployment` the source declares, in declaration
    /// order — including one an RSDL-7xx error blocked, which `system` leaves
    /// out. [`select_deployment`] counts these, not the lowered system's, to
    /// decide whether the workspace has exactly one deployment.
    pub declared_deployments: Vec<String>,
    /// The diagnostics with the severities the emit sites chose. The lint
    /// levels of `lints` are not applied here (ADR-0024 decision 8): a
    /// consumer that reports to a person or an agent applies them itself.
    pub diagnostics: Vec<Diagnostic>,
    pub sources: SourceMap,
    /// The lint scopes the loader resolved from every `[lints]` table, for a
    /// consumer that reports `diagnostics` (ADR-0024 decision 6).
    pub lints: LintScopes,
    /// The member directory the entry lies in
    /// ([`LoadedWorkspace::report_scope`]). `diagnostics` covers the whole
    /// workspace; a consumer that reports them passes this to
    /// [`retain_in_report_scope`].
    pub report_scope: Option<PathBuf>,
    /// The remote units the load took (ADR-0002 §5), keyed by unit name;
    /// `checked` holds their packages beside the consumer's. A consumer that
    /// publishes, locks, tests or diffs the workspace's own packages tells
    /// them apart with [`is_remote`].
    pub remote_units: BTreeMap<String, MaterializedUnit>,
}

/// The lowered IR of the built-in `ridl.std` package (typl Appendix A), checked
/// on its own, the same pass [`compile_workspace`] runs for
/// [`WorkspaceOutput::std_ir`]. A consumer that holds IR snapshots and no
/// compiled workspace uses it to resolve a `ridl.std` reference: `ridl diff`
/// passes it to `ridl_diff::diff_sets_in` as context, since no snapshot
/// carries `ridl.std` (driftsys/ridl#598).
pub fn std_ir() -> ridl_ir::v2::Package {
    let mut db = RidlDatabase::default();
    let std = std_package(&mut db);
    let workspace = Workspace::new(&db, Vec::new(), BTreeMap::new());
    check_package(&db, workspace, std, std).ir
}

/// Loads the workspace reachable from `entry`, then resolves and checks every
/// package in it.
///
/// `entry` is a `.typl` file (single-file mode), a package directory, or a
/// workspace root — whatever [`ridl_core::load_workspace`] accepts. The
/// remote units of the manifest's `[imports]` are read from `vendor/`, then
/// the cache, under [`Frozen::Yes`]: nothing is fetched and `ridl.lock` is
/// never written, so the language server can drive this on every edit
/// without touching the filesystem beyond the load. A pin neither holds is
/// MANI-104; the command drivers ([`run_check`], [`run_build`]) are the ones
/// that fetch.
///
/// `Err` is reserved for a filesystem failure while loading (the entry does not
/// exist, or a file cannot be read); every content problem is a [`Diagnostic`].
pub fn compile_workspace(db: &mut RidlDatabase, entry: &Path) -> std::io::Result<WorkspaceOutput> {
    compile_workspace_with(db, entry, &[]).map_err(load_io_error)
}

fn load_io_error(error: ridl_core::LoadError) -> std::io::Error {
    match error {
        ridl_core::LoadError::Io(e) => e,
        _ => unreachable!("no overlay error is possible without overlays"),
    }
}

/// Compiles a workspace with unsaved source overlays, without writes or
/// fetches: the remote units are read under [`Frozen::Yes`], as
/// [`compile_workspace`] reads them.
pub fn compile_workspace_with(
    db: &mut RidlDatabase,
    entry: &Path,
    overlays: &[ridl_core::Overlay],
) -> Result<WorkspaceOutput, ridl_core::LoadError> {
    compile_workspace_with_access(
        db,
        entry,
        overlays,
        RemoteAccess::new(Frozen::Yes, &Cache::user_default()),
    )
}

/// [`compile_workspace_with`] with the remote access the caller chooses:
/// `ridl vendor` passes [`RemoteAccess::refresh`], which reads the cache,
/// fetches what it does not hold, and rewrites `ridl.lock`.
pub fn compile_workspace_with_access(
    db: &mut RidlDatabase,
    entry: &Path,
    overlays: &[ridl_core::Overlay],
    access: RemoteAccess<'_>,
) -> Result<WorkspaceOutput, ridl_core::LoadError> {
    let Compiled {
        workspace,
        std,
        checked,
        resolutions,
        imports,
        system,
        diagnostics,
        sources,
        lints,
        report_scope,
        remote_units,
        ..
    } = load_and_check(db, entry, overlays, access)?;
    // System lowering resolves references into the standard package.
    let std_ir = check_package(&*db, workspace, std, std).ir;
    let packages: Vec<&ridl_ir::v2::Package> = checked.iter().map(|package| &package.ir).collect();
    let declared_deployments = declared_deployments(&system);
    let system = lower_workspace_system(&system, &packages, &std_ir);
    Ok(WorkspaceOutput {
        checked,
        resolutions,
        imports,
        std_ir,
        system,
        declared_deployments,
        diagnostics,
        sources,
        lints,
        report_scope,
        remote_units,
    })
}

/// Keeps the diagnostics a command reports for an entry inside a workspace
/// member: those whose primary span is in a file under `scope`, the member
/// directory ([`LoadedWorkspace::report_scope`]), and those on the workspace
/// root's `ridl.toml` — the `ridl.toml` of a directory above `scope` — because
/// the root's `[imports]`, `[lints]` and `members` govern the member, so a
/// problem there (MANI-007, for one) is the member's to see. A diagnostic
/// with no file path is kept, and so is one in a file under any of
/// `remote_dirs`, the directories a remote unit is read from
/// ([`remote_dirs`]): a remote unit's error is every member's to see,
/// whatever the scope. With `scope` `None` every diagnostic is kept. The
/// whole workspace is still loaded and checked; this only narrows what is
/// reported (ADR-0024 decision 9, as ADR-0026 amends it).
pub fn retain_in_report_scope(
    diagnostics: &mut Vec<Diagnostic>,
    sources: &SourceMap,
    scope: Option<&Path>,
    remote_dirs: &[PathBuf],
) {
    diagnostics.retain(|diagnostic| in_report_scope(diagnostic, sources, scope, remote_dirs));
}

/// Whether [`retain_in_report_scope`] keeps `diagnostic`.
fn in_report_scope(
    diagnostic: &Diagnostic,
    sources: &SourceMap,
    scope: Option<&Path>,
    remote_dirs: &[PathBuf],
) -> bool {
    let Some(scope) = scope else {
        return true;
    };
    match sources.path(diagnostic.primary.file) {
        Some(path) => {
            let path = Path::new(path);
            path.starts_with(scope)
                || is_manifest_above(path, scope)
                || remote_dirs.iter().any(|dir| path.starts_with(dir))
        }
        None => true,
    }
}

/// The directories a remote unit is read from, for
/// [`retain_in_report_scope`]: the directory of each loaded remote unit, the
/// `vendor/` directory of the manifest root governing `entry`
/// ([`ridl_core::VENDOR_DIR`]; none in single-file mode), and the root of
/// `cache`. The last two hold the tree of a unit that was materialized but
/// not loaded — one whose manifest drew MANI-014, spanned at that manifest,
/// is in no entry of `remote_units`.
pub fn remote_dirs(
    remote_units: &BTreeMap<String, MaterializedUnit>,
    cache: &Cache,
    entry: &Path,
) -> Vec<PathBuf> {
    remote_units
        .values()
        .map(|unit| unit.dir.clone())
        .chain(manifest_root_of(entry).map(|root| root.join(ridl_core::VENDOR_DIR)))
        .chain(std::iter::once(cache.root.clone()))
        .collect()
}

/// Whether `package` belongs to a loaded remote unit.
pub fn is_remote(
    package: &ridl_ir::v2::Package,
    remote_units: &BTreeMap<String, MaterializedUnit>,
) -> bool {
    remote_units.contains_key(ridl_ir::v2::unit_of(package))
}

/// Whether `path` is the `ridl.toml` of a directory at or above `scope`: the
/// workspace root's manifest, for a member scope. A sibling member's manifest
/// is not, because its directory is beside `scope`, not above it.
fn is_manifest_above(path: &Path, scope: &Path) -> bool {
    path.file_name().is_some_and(|name| name == "ridl.toml")
        && path.parent().is_some_and(|dir| scope.starts_with(dir))
}

/// A build artifact `ridlc build --emit` can write for each package.
#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
pub enum Emit {
    /// Idiomatic Rust source, written to `<base>.rs`.
    ///
    /// In package and workspace mode a build also writes `lib.rs` and
    /// `Cargo.toml` into the output directory, once per build rather than once
    /// per package: generated code names a cross-package reference as
    /// `crate::veh::…`, so the flat per-package files only resolve from inside
    /// one crate. `lib.rs` is the module tree that makes them reachable at
    /// those paths and `Cargo.toml` is what makes the directory a crate at
    /// all. Single-file mode writes neither — it writes only `<stem>.rs`,
    /// the same asymmetry documented on [`Emit::TypeScript`].
    ///
    /// Two cases write neither file, and each is a build that writes nothing
    /// at all rather than a partial crate: a build that draws an
    /// error-severity diagnostic, from the compile or from a backend
    /// ([`write_crate_files`]), and a build whose output directory already
    /// holds a `lib.rs` or a `Cargo.toml` that ridlc did not write
    /// ([`crate_file_refusals`]).
    ///
    /// A package `veh.common` alongside a type named `common` in package
    /// `veh` is one legal naming case that needs a special path: the child
    /// package's module hides the type at `veh::common`, so generated code
    /// names the type as `crate::veh::__ridl_package::common`, and a consumer
    /// can write the same path (driftsys/ridl#416).
    Rust,
    /// The lowered IR v2 as exact-decimal JSON, written to `<base>.ir.json`,
    /// and the lowered system to `<pkg.Name>.system.json`.
    IrJson,
    /// The lowered IR v2 as prototext, written to `<base>.ir.txtpb`, and the
    /// lowered system to `<pkg.Name>.system.txtpb`.
    ///
    /// The inspection encoding (ADR-0014 decisions 4 and 9): emittable, but
    /// not a recommended interchange form.
    IrText,
    /// The lowered IR v2 as protobuf binary, written to `<base>.ir.binpb`,
    /// and the lowered system to `<pkg.Name>.system.binpb`.
    ///
    /// A derived encoding (ADR-0014 decisions 4 and 9, the latter amended
    /// 2026-09-22): the compact form, whose reader stops 100 message levels
    /// below the root where the canonical JSON has no such bound.
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
    /// The FlatBuffers schema, written to `<base>.fbs`.
    ///
    /// A wire backend: the typl surface plus the interaction identity table,
    /// and nothing above them (ADR-0013 decision 2).
    Flatbuffers,
    /// The lowered codegen model (`ridl.codegen.v1`) as canonical protobuf
    /// JSON, written to `<base>.codegen.json`.
    ///
    /// It is the payload a codegen request carries (ADR-0020 decisions 8 and
    /// 9), the same JSON value as the request's `model`, so a plugin's
    /// fixture is a file `ridlc` wrote. It is classified with the code emits
    /// rather than the IR dumps: the model is lowered over the same scope a
    /// code emit reads, `ridl.std` included, and `ridl baseline` publishes
    /// only `.ir.json` artifacts.
    CodegenModel,
    /// The whole codegen request a build hands a backend, as canonical
    /// protobuf JSON, written to `<base>.codegen-request.json`.
    ///
    /// The artifact a plugin author debugs against: the request their plugin
    /// would receive for this package, without running the plugin. Its
    /// `model` field is the value [`Emit::CodegenModel`] writes, so the two
    /// emits state one fact.
    ///
    /// Its options come from `[backend.codegen-request]`, and unlike the
    /// model backend it accepts them rather than refusing them: `options` is
    /// a field of the request, and an emit that refused options could never
    /// write a request with any. It is classified with the code emits for
    /// the same reason [`Emit::CodegenModel`] is.
    CodegenRequest,
    /// The catalog descriptor an engine reads, written to
    /// `<unit>.catalog.binfb` for every unit that declares an interface or a
    /// service with an inline body: a FlatBuffers file of the unit's
    /// interfaces, their members and their catalog hash.
    Catalog,
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
            Emit::Rust
            | Emit::TypeScript
            | Emit::Proto
            | Emit::Flatbuffers
            | Emit::CodegenModel
            | Emit::CodegenRequest
            // `Catalog` is classed as a code emit although it goes through no
            // backend: a code emit keeps `ridl.std` in the `others` that
            // `run_build` passes, so the descriptor's catalog hash covers a
            // `ridl.std` type a payload names, as the hash the Rust face
            // carries does.
            | Emit::Catalog => None,
            Emit::IrJson => Some(".ir.json"),
            Emit::IrText => Some(".ir.txtpb"),
            Emit::IrBinary => Some(".ir.binpb"),
        }
    }

    /// The suffix the lowered rsdl system is written under for this emit, or
    /// `None` for an emit that writes no system (rsdl reference §13, ADR-0014
    /// decision 4).
    ///
    /// A second table beside [`Emit::ir_dump_suffix`] rather than a widening
    /// of it: `ridl`'s snapshot surface reads every `.ir.json` file as a
    /// package and `ridl baseline` publishes only those, so the `.system.`
    /// infix is what keeps a system out of a baseline. The same wildcard-free
    /// `match` and the same two lints apply, for the same reason.
    #[deny(
        clippy::wildcard_enum_match_arm,
        clippy::match_wildcard_for_single_variants
    )]
    pub const fn system_dump_suffix(self) -> Option<&'static str> {
        match self {
            Emit::Rust
            | Emit::TypeScript
            | Emit::Proto
            | Emit::Flatbuffers
            | Emit::CodegenModel
            | Emit::CodegenRequest
            | Emit::Catalog => None,
            Emit::IrJson => Some(".system.json"),
            Emit::IrText => Some(".system.txtpb"),
            Emit::IrBinary => Some(".system.binpb"),
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

    /// The name of the `[backend.<name>]` table this emit reads, or `None`
    /// for an emit that is not a backend: an IR dump, or the catalog
    /// descriptor, which takes no option. An in-tree backend's table is named
    /// after its language; the codegen model's is named `codegen-model`,
    /// after its `--emit` value, although the backend itself reports the
    /// language `model`. The codegen request's table is named
    /// `codegen-request` under the same rule, and its backend reports the
    /// language `request`.
    pub(crate) fn backend_table(self) -> Option<&'static str> {
        // The same discipline as `ir_dump_suffix`: a new emit must be
        // classified here, or this crate's test targets do not build.
        #[deny(
            clippy::wildcard_enum_match_arm,
            clippy::match_wildcard_for_single_variants
        )]
        match self {
            Emit::Rust => Some("rust"),
            Emit::TypeScript => Some("typescript"),
            Emit::Proto => Some("proto"),
            Emit::Flatbuffers => Some("flatbuffers"),
            Emit::CodegenModel => Some("codegen-model"),
            Emit::CodegenRequest => Some("codegen-request"),
            Emit::Catalog | Emit::IrJson | Emit::IrText | Emit::IrBinary => None,
        }
    }

    /// Whether this emit's backend can carry a payload codec, which is what
    /// RSDL-807 compares a deployment's links against. Only the Rust backend
    /// reads an `encodings` list today: a schema emit (`proto`,
    /// `flatbuffers`) is independent of the codecs a package carries, the
    /// TypeScript and codegen-model backends refuse the option, and the
    /// codegen-request backend writes its options into the request rather
    /// than emitting a codec from them, so a warning naming their table
    /// would ask for a key the table cannot act on. The
    /// TypeScript emit joins this arm when its backend emits a codec.
    pub(crate) fn carries_codec(self) -> bool {
        #[deny(
            clippy::wildcard_enum_match_arm,
            clippy::match_wildcard_for_single_variants
        )]
        match self {
            Emit::Rust => true,
            Emit::TypeScript
            | Emit::Proto
            | Emit::Flatbuffers
            | Emit::CodegenModel
            | Emit::CodegenRequest
            | Emit::Catalog
            | Emit::IrJson
            | Emit::IrText
            | Emit::IrBinary => false,
        }
    }
}

/// The render-ready result of a [`run_check`] or [`run_build`] command, or of
/// [`check_source`] (not a command): the merged diagnostics and the source
/// map they point into.
pub struct CliRun {
    pub diagnostics: Vec<Diagnostic>,
    pub sources: SourceMap,
    /// The lint scopes the loader resolved: empty for [`check_source`], and
    /// the loaded scopes for [`run_check`] and [`run_build_with`]. A caller
    /// that adds a lint diagnostic after the run returns applies them once
    /// more over the whole list (ADR-0024 decision 6).
    pub lints: LintScopes,
    /// Whether the run failed on a bad flag value rather than on anything in
    /// the sources: exit code 2 (ADR-0010 decision 1). The reason is the last
    /// diagnostic of `diagnostics`, so a caller renders the list as usual and
    /// reads this only for the exit code. `run_check` and [`check_source`]
    /// never set it.
    pub usage_error: bool,
}

/// Whether [`run_build_with`] applies the lint levels of the loaded `[lints]`
/// tables to its diagnostics before the emit gate (ADR-0024 decisions 6
/// and 8). `ridl build` and `ridlc build`, which report to a person, pass
/// [`Yes`](ApplyLints::Yes); `ridl baseline`, which publishes a snapshot,
/// passes [`No`](ApplyLints::No), so a lint at `deny` does not block the
/// publication.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplyLints {
    Yes,
    No,
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

/// Runs `check`: loads the workspace at `entry` with the remote units of its
/// `[imports]` — materialized against `ridl.lock`, which a clean non-frozen
/// run regenerates — then resolves and checks every package, and applies the
/// lint levels of every `[lints]` table (ADR-0024 decision 6). Returns the
/// diagnostics and the source map for rendering; for an entry inside a
/// workspace member, the compile diagnostics are those of the member's files
/// and of the remote units ([`retain_in_report_scope`]).
pub fn run_check(entry: &Path, frozen: Frozen) -> std::io::Result<CliRun> {
    let mut db = RidlDatabase::default();
    let cache = Cache::user_default();
    let Compiled {
        mut diagnostics,
        sources,
        lints,
        report_scope,
        remote_units,
        ..
    } = load_and_check(&mut db, entry, &[], RemoteAccess::new(frozen, &cache))
        .map_err(load_io_error)?;
    // An entry inside a member reports on the member only (ADR-0024 decision
    // 9, as ADR-0026 amends it).
    retain_in_report_scope(
        &mut diagnostics,
        &sources,
        report_scope.as_deref(),
        &remote_dirs(&remote_units, &cache, entry),
    );
    apply_lint_levels(&mut diagnostics, &sources, &lints);
    Ok(CliRun {
        diagnostics,
        sources,
        lints,
        usage_error: false,
    })
}

/// Runs `build`: everything [`run_check`] does, plus it writes the selected
/// `emits` for every checked package into `out_dir` — but only when the whole
/// run produced no error-severity diagnostic, whether from the compile or from
/// the load of a remote unit (a manifest, lockfile, or fetch error, MANI-1xx).
/// An error-bearing build renders its diagnostics and exits non-zero without
/// writing any artifact (C1). A Rust build whose output directory already
/// holds a `lib.rs` or a `Cargo.toml` that ridlc did not write is refused on
/// the same terms, before the per-package write loop runs, so it writes no
/// artifact either ([`crate_file_refusals`]).
///
/// The one exception is an RSDL-7xx error, which blocks only its own deployment
/// (rsdl reference §13): the build writes every artifact, leaves that
/// deployment out of the system, and still exits non-zero.
///
/// When the workspace declares a `system`, each IR dump emit also writes the
/// lowered system beside the package IR, named after the system's qualified
/// name ([`Emit::system_dump_suffix`]).
///
/// The artifact base name is the file stem in single-file mode (preserving the
/// E0 `<input-stem>.rs` contract) and the full dotted package name otherwise, so
/// a workspace build writing several packages into one directory never has two
/// packages collide on a file name. Each package is generated on its own; a
/// cross-package derivable `Default` therefore needs the referenced package's
/// generated code compiled alongside it (documented, not linked here).
pub fn run_build(
    entry: &Path,
    out_dir: &Path,
    emits: &[Emit],
    frozen: Frozen,
) -> std::io::Result<CliRun> {
    run_build_with(
        entry,
        out_dir,
        emits,
        &[],
        Duration::from_secs(plugin::DEFAULT_TIMEOUT_SECONDS),
        frozen,
        ApplyLints::Yes,
        None,
        &BTreeMap::new(),
    )
}

/// [`run_build`] with codegen plugins: each `--plugin` value is resolved to
/// an executable before the build ([`plugin::resolve`]) — a plugin that
/// cannot be found is an error diagnostic that, like a compile error,
/// suppresses every artifact — and then run once per package the code emits
/// are written for, `ridl.std` included, through the process host
/// ([`plugin::run`]) with `plugin_timeout` as its limit. A plugin's files
/// are written under `out_dir` exactly as an in-tree backend's are
/// ([`write_response`]).
///
/// With [`ApplyLints::Yes`], the lint levels of every `[lints]` table are
/// applied before the emit gate, so a lint at `deny` is an error that
/// suppresses every artifact; with [`ApplyLints::No`], the diagnostics keep
/// the severities the emit sites chose (ADR-0024 decision 8).
///
/// `deployment` names the deployment of the workspace's system that every
/// codegen request carries ([`select_deployment`] holds the rule). A name the
/// system does not declare is a bad flag value: the returned run carries the
/// reason as its last diagnostic and sets [`CliRun::usage_error`], which the
/// command reports with exit code 2 (ADR-0010 decision 1). When the build has
/// already drawn an error of its own, that error takes precedence: the run
/// carries it, writes nothing, and exits 1.
///
/// `compatible` is, per unit name, the hashes of the earlier catalogs a
/// consumer built against still works with: each unit's list is written as
/// given into its catalog descriptor ([`write_catalogs`]) and into the
/// `catalog.compatible` of its codegen request ([`codegen_request`]), and a
/// unit absent from the map gets an empty list. The list never feeds the
/// catalog hash. This function reads no baseline and computes no list: the
/// `ridl` facade reads the published chain and passes the result here, so
/// that `ridlc` stays a function of the sources alone (ADR-0008 decisions 9
/// and 14); `ridlc build` passes an empty map. [`run_build_computing`] is the
/// variant that computes the list from the build's own compile.
#[expect(
    clippy::too_many_arguments,
    reason = "the build's options, passed once from each command"
)]
pub fn run_build_with(
    entry: &Path,
    out_dir: &Path,
    emits: &[Emit],
    plugins: &[plugin::PluginSpec],
    plugin_timeout: Duration,
    frozen: Frozen,
    apply_lints: ApplyLints,
    deployment: Option<&str>,
    compatible: &BTreeMap<String, Vec<[u8; 32]>>,
) -> std::io::Result<CliRun> {
    run_build_inner::<std::io::Error>(
        entry,
        out_dir,
        emits,
        plugins,
        plugin_timeout,
        frozen,
        apply_lints,
        deployment,
        compatible,
        None,
    )
}

/// What a build gives the function that computes the earlier catalogs each
/// unit is compatible with: every checked package of the workspace's own
/// units, the `ridl.std` package, and whether the workspace names any
/// `[imports]`. The packages of a remote unit are not among the packages
/// given.
///
/// `E` is the error the function reports, chosen by the caller; it defaults to
/// [`std::io::Error`].
pub type CompatibleCatalogsFn<'a, E = std::io::Error> = dyn FnMut(
        &[&ridl_ir::v2::Package],
        &ridl_ir::v2::Package,
        bool,
    ) -> Result<BTreeMap<String, Vec<[u8; 32]>>, E>
    + 'a;

/// [`run_build_with`], with the list computed from the build's own compile
/// instead of given. `compute` is called once, right after the compile — the
/// load, which materializes the manifest's imports, and the check — and
/// before a plugin is resolved or any lint level is applied, with the
/// checked packages of the workspace's own units, the `ridl.std` package and
/// whether the workspace names `[imports]`. It is not called when the
/// compile has an error diagnostic, a remote unit's or a MANI-1xx one
/// included, whatever the build later writes. Its
/// result is used as `run_build_with` uses its `compatible` argument, and an
/// error it returns is returned as the error of the build, unchanged. An
/// input or output error of the build itself is converted with `E::from`.
/// With `None`, no unit has a list, and the caller names `E` explicitly
/// (`run_build_computing::<std::io::Error>`), as `E` cannot be inferred.
/// This is how the `ridl` facade reuses the build's compile instead of
/// compiling the workspace a second time.
#[expect(
    clippy::too_many_arguments,
    reason = "the build's options, passed once from each command"
)]
pub fn run_build_computing<E: From<std::io::Error>>(
    entry: &Path,
    out_dir: &Path,
    emits: &[Emit],
    plugins: &[plugin::PluginSpec],
    plugin_timeout: Duration,
    frozen: Frozen,
    apply_lints: ApplyLints,
    deployment: Option<&str>,
    compute: Option<&mut CompatibleCatalogsFn<'_, E>>,
) -> Result<CliRun, E> {
    run_build_inner(
        entry,
        out_dir,
        emits,
        plugins,
        plugin_timeout,
        frozen,
        apply_lints,
        deployment,
        &BTreeMap::new(),
        compute,
    )
}

#[expect(
    clippy::too_many_arguments,
    reason = "the build's options, passed once from each command"
)]
fn run_build_inner<E: From<std::io::Error>>(
    entry: &Path,
    out_dir: &Path,
    emits: &[Emit],
    plugins: &[plugin::PluginSpec],
    plugin_timeout: Duration,
    frozen: Frozen,
    apply_lints: ApplyLints,
    deployment: Option<&str>,
    given: &BTreeMap<String, Vec<[u8; 32]>>,
    compute: Option<&mut CompatibleCatalogsFn<'_, E>>,
) -> Result<CliRun, E> {
    let mut db = RidlDatabase::default();
    let cache = Cache::user_default();
    let Compiled {
        workspace,
        std,
        checked,
        system,
        mut diagnostics,
        mut sources,
        lints,
        codegen_header,
        backends,
        report_scope,
        root_package,
        remote_units,
        ..
    } = load_and_check(&mut db, entry, &[], RemoteAccess::new(frozen, &cache))
        .map_err(|err| E::from(load_io_error(err)))?;
    // The backends read names only; the spans served the loader's diagnostics.
    let backends: BTreeMap<String, Vec<String>> = backends
        .into_iter()
        .map(|(language, table)| {
            (
                language,
                table.encodings.into_iter().map(|(name, _)| name).collect(),
            )
        })
        .collect();

    // The earlier catalogs per unit, computed from this compile before
    // anything else can fail, so a history that cannot be read is reported
    // first. A compile with an error diagnostic gets no list from `compute`.
    // The list covers the workspace's own units: a remote unit's catalog is
    // its owner's to publish, so its packages are not given.
    let computed = match compute {
        Some(compute)
            if !diagnostics
                .iter()
                .any(|diagnostic| diagnostic.severity == Severity::Error) =>
        {
            let packages: Vec<&ridl_ir::v2::Package> = checked
                .iter()
                .map(|package| &package.ir)
                .filter(|ir| !is_remote(ir, &remote_units))
                .collect();
            let std_for_catalogs = check_package(&db, workspace, std, std).ir;
            let names_imports = !workspace.imports(&db).is_empty()
                || workspace
                    .packages(&db)
                    .iter()
                    .any(|package| !package.imports(&db).is_empty());
            Some(compute(&packages, &std_for_catalogs, names_imports)?)
        }
        _ => None,
    };
    let compatible = computed.as_ref().unwrap_or(given);

    // An entry inside a member reports on the member only (ADR-0024 decision
    // 9, as ADR-0026 amends it). The diagnostics of the other members are
    // kept aside: an error there still blocks every artifact below, because
    // the build writes the whole workspace.
    let remote_dirs = remote_dirs(&remote_units, &cache, entry);
    let mut outside_scope: Vec<Diagnostic> = diagnostics
        .iter()
        .filter(|diagnostic| {
            !in_report_scope(diagnostic, &sources, report_scope.as_deref(), &remote_dirs)
        })
        .cloned()
        .collect();
    retain_in_report_scope(
        &mut diagnostics,
        &sources,
        report_scope.as_deref(),
        &remote_dirs,
    );

    // The load materialized the remote imports and round-tripped the
    // lockfile, so any error that raised (a manifest, lockfile, or fetch
    // problem, MANI-1xx — for example a frozen build with no `ridl.lock`) is
    // already among the compile diagnostics, and suppresses code generation
    // below exactly like a compile error does.

    // A plugin that cannot be found is known before anything is generated,
    // and is reported the way a manifest error is: an error that joins the
    // compile diagnostics and suppresses every artifact, so a build with a
    // misspelled `--plugin` writes nothing rather than every artifact but
    // one.
    let mut resolved_plugins = Vec::with_capacity(plugins.len());
    for spec in plugins {
        match plugin::resolve(spec) {
            Ok(resolved) => resolved_plugins.push(resolved),
            Err(err) => diagnostics.push(error_diagnostic(
                "",
                err.to_string(),
                FileId::DETACHED,
                TextRange::default(),
            )),
        }
    }

    // RSDL-807: a link of the selected deployment carries an encoding the
    // emitted backend's `encodings` list does not name, once per backend the
    // build emits that can carry a codec — the Rust emit (`carries_codec`)
    // and each plugin. A schema emit's table cannot name an encoding, so it
    // is not compared. A backend named by both an emit and a plugin is
    // compared once, so its links draw one diagnostic rather than two
    // (driftsys/ridl#859). A manifest that declares `[backend.<name>]` with
    // an empty `encodings` list says that the backend emits no codec, and
    // that backend is not compared either; a backend with no table at all is
    // compared, so a build that never declared its encodings is still
    // reported (driftsys/ridl#853). Raised here, before the levels are
    // applied, so that `deny` makes it an error the emit gate below reads.
    // The deployment is the one `select_deployment` selects below; a name no
    // source declares draws nothing here and is reported there.
    if let Some(selected) = requested_or_only(&declared_deployments(&system), deployment) {
        let mut compared: BTreeSet<&str> = BTreeSet::new();
        let tables = emits
            .iter()
            .filter(|emit| emit.carries_codec())
            .filter_map(|emit| emit.backend_table())
            .chain(
                resolved_plugins
                    .iter()
                    .map(|plugin| plugin.language.as_str()),
            );
        for table in tables {
            if !compared.insert(table) {
                continue;
            }
            // `None` is a backend with no table: every link is compared, so a
            // build that declared no encodings is reported. `Some` with an
            // empty list is the declaration that the backend emits no codec.
            // A table written with no `encodings` key at all is the same
            // declaration: the manifest records the two forms alike
            // (`BackendTable::encodings`).
            let Some(encodings) = backends.get(table) else {
                diagnostics.extend(link_encodings::link_encoding_not_emitted(
                    &db,
                    &system,
                    selected,
                    table,
                    &[],
                    &mut sources,
                ));
                continue;
            };
            if encodings.is_empty() {
                continue;
            }
            diagnostics.extend(link_encodings::link_encoding_not_emitted(
                &db,
                &system,
                selected,
                table,
                encodings,
                &mut sources,
            ));
        }
    }

    // The levels are applied before the emit gate below, so a lint at `deny`
    // is an error by the time the gate reads the list and no artifact is
    // written for it. `ridl.lock` is already written by the load; the
    // lockfile is not an artifact (Task 4 of
    // docs/archive/2026-10-03-lint-foundation-plan.md).
    if apply_lints == ApplyLints::Yes {
        apply_lint_levels(&mut diagnostics, &sources, &lints);
        apply_lint_levels(&mut outside_scope, &sources, &lints);
    } else {
        drop_allowed_by_default(&mut diagnostics);
    }

    // A build must not emit artifacts for a workspace that failed: code
    // generation over error-bearing IR produces invalid or misleading output,
    // and a malformed IR could even crash a backend (C1). `check` never runs
    // codegen; `build` matches that by skipping every emit — code and IR
    // dumps alike — when any error-severity diagnostic is present, from the
    // compile or from materialization. Warnings and info do not gate.
    //
    // rsdl reference §13 narrows this for one class of error: an RSDL-7xx error
    // blocks the lowering of its own deployment only. Every package and the
    // system's other deployments are sound, so they are written, and the
    // error still makes the build exit 1.
    //
    // For an entry inside a member, the other members' diagnostics are not
    // reported, but they gate the same way. Any error among them makes the
    // build exit 1 with one detached error that says so: either nothing was
    // written, or (an RSDL-7xx error) a deployment was left out of the system.
    let succeeded = !diagnostics.iter().any(blocks_every_artifact)
        && !outside_scope.iter().any(blocks_every_artifact);
    if outside_scope
        .iter()
        .any(|diagnostic| diagnostic.severity == Severity::Error)
    {
        diagnostics.push(error_diagnostic(
            "",
            "another member of the workspace has an error, so this build wrote nothing or left \
             out what that error blocks; run `ridl check` on the workspace root to see it"
                .to_string(),
            FileId::DETACHED,
            TextRange::default(),
        ));
    }
    if succeeded {
        // `ridl.std` is deliberately absent from `checked` (it is not a
        // workspace member), so no loop over `checked` ever reaches it. A
        // consumer's generated code still references it, so the build
        // writes it whenever the workspace names something from it —
        // otherwise the raw output does not compile (issue #190). Computed
        // once, ahead of the per-package loop below, because the proto
        // backend needs it too (next paragraph) and this is the one
        // `check_package` call either use pays for.
        let packages: Vec<&ridl_ir::v2::Package> =
            checked.iter().map(|package| &package.ir).collect();
        // The workspace's own packages: a remote unit's are generated below
        // but get no catalog, which is its owner's to publish.
        let own: Vec<&ridl_ir::v2::Package> = packages
            .iter()
            .copied()
            .filter(|ir| !is_remote(ir, &remote_units))
            .collect();
        let references_std = references_std(&packages);
        // Every emit kind except the direct IR dumps. The reasoning, and the
        // exhaustive classification ADR-0014 decision 10 requires, live on
        // `Emit::is_ir_dump`.
        let code_emits: Vec<Emit> = emits
            .iter()
            .copied()
            .filter(|emit| !emit.is_ir_dump())
            .collect();
        // A plugin is a code emit by the same argument: what it generates
        // names `ridl.std`'s types by package path, so it is given the
        // standard package whenever the in-tree code emits are.
        let generates_code = !code_emits.is_empty() || !resolved_plugins.is_empty();
        let std_ir =
            (references_std && generates_code).then(|| check_package(&db, workspace, std, std).ir);

        // Every other package a package's cross-package reference might
        // name: every sibling in the workspace, plus `ridl.std` when
        // present. The proto3, FlatBuffers and Rust
        // backends each resolve a foreign reference themselves rather than
        // leaving it to the target language's own import statement, so each
        // reads this (`write_emits`'s doc comment). `Emit::Catalog` is a code
        // emit, so for the catalog descriptor `ridl.std` is here exactly when
        // `references_std` holds, the set `embed_catalog_hashes` hashes each
        // region over.
        let others = catalog_scope(&packages, std_ir.as_ref());

        // The lowered system, with the catalog hashes its regions carry. The
        // region hashes must equal the hashes a `--emit catalog` build
        // writes, so `ridl.std` is in their scope whenever a package
        // references it: the `std_ir` above when a code emit computed it,
        // otherwise checked here. Lowered once, for the deployment every
        // request carries and for the system dump emits below. An error in
        // the closure has already stopped the build above, so the lowering is
        // `None` here only when the workspace declares no `system`.
        let wants_system = generates_code
            || deployment.is_some()
            || emits.iter().any(|emit| emit.system_dump_suffix().is_some());
        let checked_std;
        let hash_std = match &std_ir {
            Some(std_ir) => Some(std_ir),
            None if references_std && wants_system => {
                checked_std = check_package(&db, workspace, std, std).ir;
                Some(&checked_std)
            }
            None => None,
        };
        let lowered_system = if wants_system {
            lower_system(&system, &packages).map(|mut lowered| {
                embed_catalog_hashes(&mut lowered, &packages, &catalog_scope(&packages, hash_std));
                lowered
            })
        } else {
            None
        };
        let selected = match select_deployment(
            lowered_system.as_ref(),
            &declared_deployments(&system),
            deployment,
            &catalog_scope(&packages, hash_std),
        ) {
            Ok(selected) => selected,
            // A deployment an RSDL-7xx error dropped from the system is
            // declared in the source, so its name is not unknown: the build
            // reports that error and writes nothing (rsdl reference §13).
            Err(_) if diagnostics.iter().any(|d| d.severity == Severity::Error) => {
                return Ok(CliRun {
                    diagnostics,
                    sources,
                    lints,
                    usage_error: false,
                });
            }
            // A name no source declares is a bad flag value (ADR-0010
            // decision 1): exit 2, before the output directory is created.
            // The diagnostics the compile already drew are kept and rendered
            // with it, as they are on every other failure of this function:
            // the flag value is wrong, which is no reason to hide a finding
            // about the sources.
            Err(err) => {
                diagnostics.push(error_diagnostic(
                    "",
                    err.to_string(),
                    FileId::DETACHED,
                    TextRange::default(),
                ));
                return Ok(CliRun {
                    diagnostics,
                    sources,
                    lints,
                    usage_error: true,
                });
            }
        };

        std::fs::create_dir_all(out_dir).map_err(E::from)?;
        let single_file = entry.is_file() && manifest_root_of(entry).is_none();
        let file_stem = module_name_from_path(&entry.to_string_lossy());

        // Whether this build owes the output directory a crate root and a
        // manifest: generated Rust names a cross-package reference as
        // `crate::veh::…` (`ridl_backend_rust::type_path`), so every package
        // it emits must land inside one crate. Single-file mode keeps writing
        // only `<stem>.rs`, matching the documented single-file asymmetry on
        // `Emit::TypeScript`.
        let writes_crate_files =
            !single_file && emits.iter().any(|emit| matches!(emit, Emit::Rust));

        // The overwrite gate runs before any write, not after the per-package
        // loop: a refusal is a build that produced nothing, and a loop that
        // had already written every `<package>.rs` into a hand-written crate
        // would contradict that. `lib.rs` and `Cargo.toml` are not
        // package-scoped names and `--out-dir` is any directory the caller
        // names, so this is the check that keeps a build from truncating
        // sources a person wrote.
        if writes_crate_files {
            let refusals = crate_file_refusals(out_dir)?;
            if !refusals.is_empty() {
                diagnostics.extend(refusals);
                return Ok(CliRun {
                    diagnostics,
                    sources,
                    lints,
                    usage_error: false,
                });
            }
        }

        for package in &checked {
            let base = if single_file {
                file_stem.clone()
            } else {
                package.ir.name.clone()
            };
            let unit_compatible = compatible
                .get(ridl_ir::v2::unit_of(&package.ir))
                .map_or(&[][..], Vec::as_slice);
            // A remote package is generated like a member's but is not part
            // of the consumer's contract snapshot: no IR dump is written for
            // it.
            let package_emits: Vec<Emit> = if is_remote(&package.ir, &remote_units) {
                emits
                    .iter()
                    .copied()
                    .filter(|emit| !emit.is_ir_dump())
                    .collect()
            } else {
                emits.to_vec()
            };
            write_emits(
                out_dir,
                &base,
                &package.ir,
                &others,
                unit_compatible,
                &package_emits,
                &resolved_plugins,
                plugin_timeout,
                selected.as_ref(),
                codegen_header.as_deref(),
                &backends,
                &mut diagnostics,
            )?;
        }

        if emits.contains(&Emit::Catalog) {
            write_catalogs(out_dir, &own, &others, compatible)?;
        }

        if let Some(std_ir) = &std_ir {
            write_emits(
                out_dir,
                "ridl.std",
                std_ir,
                &others,
                &[],
                &code_emits,
                &resolved_plugins,
                plugin_timeout,
                selected.as_ref(),
                codegen_header.as_deref(),
                &backends,
                &mut diagnostics,
            )?;
        }

        // `<out_dir>/lib.rs` builds the module tree over the flat per-package
        // files and `<out_dir>/Cargo.toml` is the manifest that makes the
        // directory a crate at all. `ridl.std` joins the module tree whenever
        // the build wrote `ridl.std.rs` above (issue #190), because generated
        // code names those types as `crate::ridl::std::…` too.
        if writes_crate_files {
            let mut package_names: Vec<String> = checked
                .iter()
                .map(|package| package.ir.name.clone())
                .collect();
            if std_ir.is_some() {
                package_names.push("ridl.std".to_string());
            }

            // The Rust backend's own table: the manifest enables one `ridl-rt`
            // feature per encoding the backend was asked to emit a codec for,
            // and none when it was asked for none.
            let rust_encodings: &[String] = backends.get("rust").map_or(&[], Vec::as_slice);
            write_crate_files(
                out_dir,
                &crate_name_for(root_package.as_deref()),
                root_package.as_deref(),
                &package_names,
                codegen_header.as_deref(),
                rust_encodings,
                &diagnostics,
            )?;
        }

        // The lowered system, beside the package IR, for each IR dump emit
        // (rsdl reference §13).
        if emits.iter().any(|emit| emit.system_dump_suffix().is_some())
            && let Some(lowered) = &lowered_system
        {
            write_system_emits(out_dir, lowered, emits, &mut diagnostics)?;
        }
    }

    Ok(CliRun {
        diagnostics,
        sources,
        lints,
        usage_error: false,
    })
}

/// The crate name for the generated `Cargo.toml`: the root manifest's
/// `[package]` name with every `.` replaced by `_`, since a dotted ridl
/// package name (`veh.common`) is not a legal Cargo package name. A
/// `[workspace]` manifest names no package, so it falls back to
/// `ridl_generated`; single-file mode never reaches this function (`run_build`
/// gates the call on `!single_file`).
///
/// The name comes from [`Compiled::root_package`], which the loader read from
/// the manifest it parsed, so the build does not find the root and parse the
/// manifest again (driftsys/ridl#760).
fn crate_name_for(root_package: Option<&str>) -> String {
    root_package
        .map(|name| name.replace('.', "_"))
        .unwrap_or_else(|| "ridl_generated".to_string())
}

/// The first line of a `Cargo.toml` that an earlier release generated. A build
/// still overwrites a file that begins with it ([`crate_file_refusals`]).
const OLD_CARGO_TOML_MARKER: &str = "# Generated by ridlc. Do not edit.";

/// The first line of a `lib.rs` that an earlier release generated, the
/// crate-root counterpart of [`OLD_CARGO_TOML_MARKER`].
const OLD_LIB_RS_MARKER: &str = "// Generated by ridlc. Do not edit.";

/// Writes `<out_dir>/lib.rs` and `<out_dir>/Cargo.toml` for a Rust emit, or
/// writes neither.
///
/// The two files are decided together and refused together, because a crate
/// root without a manifest and a manifest without a crate root are each worse
/// than neither. Two conditions stop both writes:
///
/// - **An error-severity diagnostic is present.** [`run_build`] computes its
///   emit gate before the per-package write loop, so that gate cannot see a
///   package whose Rust generation failed inside the loop: `write_emits`
///   records the failure as a diagnostic and skips that package's `.rs`. The
///   module tree is built from every checked package, so it would name a file
///   that was never written and the crate would not compile. Re-checking here,
///   after the loop, is what sees it.
/// - **An existing destination is not a generated file.** That check is
///   [`crate_file_refusals`], which [`run_build`] runs before it writes
///   anything at all — including the per-package sources — so a refusal is a
///   build that produced nothing.
///
/// The writes themselves are not atomic and are not made so: an I/O failure on
/// either one is returned as `Err`, which aborts the build with exit code 2,
/// and a failure on the manifest therefore leaves the crate root written.
/// Making the pair atomic would need a temporary directory and a rename, which
/// buys nothing a re-run does not: the next successful build overwrites both,
/// because both carry the marker that permits it.
fn write_crate_files(
    out_dir: &Path,
    crate_name: &str,
    manifest_package: Option<&str>,
    package_names: &[String],
    header: Option<&str>,
    encodings: &[String],
    diagnostics: &[Diagnostic],
) -> std::io::Result<()> {
    if diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == Severity::Error)
    {
        return Ok(());
    }

    let marker = codegen::generated_marker(manifest_package);
    let header = header.unwrap_or_default();
    std::fs::write(
        out_dir.join("lib.rs"),
        render_lib_rs(
            package_names,
            &codegen::comment_preamble(&marker, header, "//"),
        ),
    )?;
    std::fs::write(
        out_dir.join("Cargo.toml"),
        render_cargo_toml(
            crate_name,
            &codegen::comment_preamble(&marker, header, "#"),
            encodings,
        ),
    )?;
    Ok(())
}

/// The overwrite gate for the two crate files: one error diagnostic per
/// destination that exists and was not written by a previous build, and an
/// empty vector when both are safe to write.
///
/// `lib.rs` and `Cargo.toml` are not package-scoped names and `--out-dir` is
/// any directory the caller names, including the root of a hand-written crate.
/// `std::fs::write` truncates, so a build pointed at such a directory would
/// destroy sources the user wrote. A file that begins with neither the current
/// marker (`@generated by ridl`, behind the file's comment token) nor the
/// marker an earlier release wrote is treated as hand-written and is left
/// exactly as it is.
///
/// Both destinations are checked even once the first has refused, so a build
/// into a hand-written crate reports every file it declined rather than one at
/// a time.
fn crate_file_refusals(out_dir: &Path) -> std::io::Result<Vec<Diagnostic>> {
    let mut refusals = Vec::new();
    for (path, comment, old_marker) in [
        (out_dir.join("lib.rs"), "//", OLD_LIB_RS_MARKER),
        (out_dir.join("Cargo.toml"), "#", OLD_CARGO_TOML_MARKER),
    ] {
        if let Some(diagnostic) = refuse_overwrite(&path, comment, old_marker)? {
            refusals.push(diagnostic);
        }
    }
    Ok(refusals)
}

/// An error diagnostic when `path` exists and was not written by a previous
/// build, or `None` when the file is absent or begins with the current marker
/// (`comment` and [`codegen::GENERATED_MARKER_PREFIX`]) or with `old_marker`.
///
/// A file this cannot read is neither written nor refused: the read error is
/// returned as the [`std::io::Error`] it is, so [`run_build`] returns `Err`
/// and the command exits 2 (ADR-0010's exit code for an I/O failure). Turning
/// it into a diagnostic would exit 1 and would assert something about the
/// file's first line that was never established — an unreadable file's
/// contents are unknown, not known to be hand-written.
fn refuse_overwrite(
    path: &Path,
    comment: &str,
    old_marker: &str,
) -> std::io::Result<Option<Diagnostic>> {
    if !path.exists() {
        return Ok(None);
    }
    let marker = format!("{comment} {}", codegen::GENERATED_MARKER_PREFIX);
    let text = std::fs::read_to_string(path)?;
    if text.starts_with(&marker) || text.starts_with(old_marker) {
        return Ok(None);
    }
    Ok(Some(error_diagnostic(
        "",
        format!(
            "`{}` exists and does not start with `{marker}` or with `{old_marker}`, so ridlc did \
             not write it: a file that starts with neither marker was not generated by ridlc and \
             could be a hand-written one. This build wrote nothing at all — no crate root, no \
             generated manifest, and no package sources — and the existing file is unchanged. \
             Remove it, or build into a different --out-dir",
            path.display()
        ),
        FileId::DETACHED,
        TextRange::default(),
    )))
}

/// The generated `Cargo.toml` body: a plain `format!`, not a template — see
/// Task 7's rationale for why no templating engine is warranted for two short
/// strings. `ridl-rt` is not optional: it is `no_std` with `std` off, has no
/// dependency of its own in any feature combination, and every generated named
/// scalar's constructor names `::ridl_rt::payload::Violation` from it.
///
/// The `ridl-rt` dependency enables one feature per entry of `encodings`, in
/// order — the manifest's `[backend.rust] encodings` list, whose names are
/// `ridl-rt`'s feature names — because the codec the Rust backend emits for
/// an encoding names the reading and writing helpers that feature gates. An
/// empty list, which is a types-only build, enables none. **A feature makes
/// the generated manifest unbuildable outside this repository until a
/// `ridl-rt` release carries the feature's contents**, which is the release
/// coupling design note D-12 records and the generated manifest settles;
/// nothing here can test it, because a `rustc` proof links `ridl-rt`'s
/// source rather than a release.
///
/// The `std` feature, on by default, forwards to `ridl-rt/std`: the generated
/// face's `blocking` module is under it and is `block_on` over the async
/// face, and `block_on` is what `ridl-rt`'s `std`
/// feature gates. A build with default features off has no `blocking` module,
/// links `ridl-rt` as `no_std`, and is itself `no_std` (the crate root
/// [`render_lib_rs`] writes declares it). `validate-pattern` does not turn
/// `std` on, so it adds no `blocking` module and leaves `ridl-rt` `no_std`;
/// the crate root links the standard library while it is on, because the
/// pattern check holds its `Regex` in a `std::sync::LazyLock`.
///
/// The `regex = "1.13"` requirement is the major and minor version of the
/// `regex` crate the checker compiles every `match` pattern with (TYPL-220).
/// An older `regex` could refuse a pattern the checker accepted, and the
/// generated `Regex::new(..).expect(..)` would then panic. A guard test in
/// `crates/ridlc/tests/` keeps its major and minor numbers equal to those of
/// the `regex` version the workspace's `Cargo.lock` resolves, which is the
/// version the checker links.
///
/// The manifest declares edition 2021 and no `rust-version`. Edition 2021 is
/// the edition ADR-0029 decision 5 promises the generated crate compiles as
/// with Rust 1.83, and cargo before 1.85 cannot read a manifest that declares
/// edition 2024. The compatibility gate builds with every feature on and
/// with default features off. The backend's compile proofs build the emitted
/// source as edition 2024 as well.
///
/// The `ridl-rt = "0.11"` requirement is a literal, not read from
/// `crates/ridl-rt/Cargo.toml`, because `ridlc` is an installed binary with no
/// access to this repository's sources at run time; a guard test
/// (`crates/ridlc/tests/`) keeps the two from drifting apart silently.
fn render_cargo_toml(crate_name: &str, preamble: &str, encodings: &[String]) -> String {
    const RIDL_RT_REQUIREMENT: &str = "0.11";
    let ridl_rt = if encodings.is_empty() {
        format!(r#"ridl-rt = {{ version = "{RIDL_RT_REQUIREMENT}" }}"#)
    } else {
        let features = encodings
            .iter()
            .map(|name| format!("\"{name}\""))
            .collect::<Vec<_>>()
            .join(", ");
        format!(r#"ridl-rt = {{ version = "{RIDL_RT_REQUIREMENT}", features = [{features}] }}"#)
    };
    format!(
        r#"{preamble}[package]
name = "{crate_name}"
version = "0.0.0"
edition = "2021"

[features]
default = ["validate-pattern", "std"]
# Enforce `match` patterns in generated constructors. The crate links the
# standard library while it is on. Disable on a target that cannot carry the
# regex dependency; range and length checks are unaffected.
validate-pattern = ["dep:regex"]
std = ["ridl-rt/std"]

[dependencies]
{ridl_rt}
regex = {{ version = "1.13", optional = true }}

[lib]
path = "lib.rs"
"#
    )
}

/// The `cfg` predicate under which the generated crate is `no_std`: neither
/// of the two features that need the standard library is on.
const NO_STD: &str = r#"not(any(feature = "std", feature = "validate-pattern"))"#;

/// Builds the module tree that makes the flat emitted files reachable at the
/// `crate::…` paths generated code already uses.
///
/// `veh.common` and `veh.adas` produce one `veh` module containing two leaves,
/// each pointing at its flat file. The output is deterministic because the
/// tree is a `BTreeMap` keyed per segment, so the order `package_names`
/// arrives in cannot change a byte of it.
///
/// Each segment is spelled through
/// [`module_segment`](ridl_backend_rust::module_segment), which is the same
/// spelling `ridl_backend_rust::type_path` gives a segment of a cross-package
/// reference: a package segment that is a Rust keyword (`veh.mod` is a legal
/// typl package name) must be escaped identically on both surfaces, or the
/// crate either does not parse or does not resolve.
fn render_lib_rs(package_names: &[String], preamble: &str) -> String {
    #[derive(Default)]
    struct Node {
        children: std::collections::BTreeMap<String, Node>,
        file: Option<String>,
    }

    let mut root = Node::default();
    for name in package_names {
        let mut node = &mut root;
        for segment in name.split('.') {
            node = node
                .children
                .entry(ridl_backend_rust::module_segment(segment))
                .or_default();
        }
        node.file = Some(format!("{name}.rs"));
    }

    fn render(node: &Node, depth: usize, out: &mut String) {
        let pad = "    ".repeat(depth);
        for (segment, child) in &node.children {
            match &child.file {
                Some(file) if child.children.is_empty() => {
                    out.push_str(&format!("{pad}#[path = \"{file}\"]\n"));
                    out.push_str(&format!("{pad}pub mod {segment};\n"));
                }
                _ => {
                    // An inline (non-leaf) module's own children would
                    // otherwise search a subdirectory named after every
                    // enclosing module (rustc's default module-path
                    // resolution for a module with no `#[path]`); anchoring
                    // this module at `.` keeps its children's own `#[path]`
                    // attributes resolving against the flat output directory.
                    out.push_str(&format!("{pad}#[path = \".\"]\n"));
                    out.push_str(&format!("{pad}pub mod {segment} {{\n"));
                    if let Some(file) = &child.file {
                        // `segment` is itself a package as well as a
                        // namespace for its children (`veh` alongside
                        // `veh.common`) — the `_` arm above would otherwise
                        // drop `segment`'s own file. It cannot simply be
                        // nested under its own name: generated code names a
                        // type in package `veh` as `crate::veh::Speed`, not
                        // `crate::veh::veh::Speed`, so the file is loaded as
                        // a module and re-exported, which puts its items at
                        // the path the references use.
                        //
                        // The re-export loses one name: a package `veh`
                        // declaring a type called `common` alongside a
                        // package `veh.common` has that type hidden by the
                        // explicit `pub mod common` below, because Rust has
                        // one type namespace per module (driftsys/ridl#416).
                        // `ridl_backend_rust::type_path` writes a reference
                        // to such a type as `crate::veh::__ridl_package::common`,
                        // so the module is `pub` for a consumer to write the
                        // same path, and `#[doc(hidden)]` because it is a
                        // second path to every item of `veh` and not one to
                        // use otherwise.
                        //
                        // No ridl name reaches `__ridl_package`. MANI-006
                        // holds only a manifest's own `[package] name` to
                        // `[a-z][a-z0-9]*`; a segment that comes from a
                        // subdirectory is not checked by it. But every file
                        // of a package declares that package's full name
                        // (TYPL-002), and the lexer reads each segment of a
                        // `package` declaration as an identifier, which
                        // starts with a letter, so a directory named
                        // `__ridl_package` never becomes a package. A
                        // declaration name is lexed the same way, and the
                        // keyword escape only appends `_`, so neither a type
                        // of `veh` nor a child package spells this module.
                        out.push_str(&format!("{pad}    #[path = \"{file}\"]\n"));
                        out.push_str(&format!("{pad}    #[doc(hidden)]\n"));
                        out.push_str(&format!("{pad}    pub mod __ridl_package;\n"));
                        out.push_str(&format!("{pad}    pub use __ridl_package::*;\n"));
                    }
                    render(child, depth + 1, out);
                    out.push_str(&format!("{pad}}}\n"));
                }
            }
        }
    }

    // The crate is `no_std` when neither feature that needs the standard
    // library is on: `std`, for the `blocking` module, and `validate-pattern`,
    // for the pattern check's `::std::sync::LazyLock`. Without the first line
    // the crate links the standard library whatever its features are, and
    // fails to build for a target that has none.
    //
    // The generated package files name `::std::string::String` and
    // `::std::vec::Vec`, and call `<[u8]>::to_vec`, all of which `alloc`
    // provides. In a `no_std` build, `alloc` is linked under the name `std`,
    // so those paths resolve to the same types in `alloc`, and the package
    // files stay the same in both modes and in single-file mode. The alias
    // exists so that a single-file consumer with the standard library keeps
    // compiling without an `extern crate alloc;` of its own, which
    // `::alloc::` paths in the package files would require of it.
    let mut out = format!(
        "{preamble}#![cfg_attr({NO_STD}, no_std)]\n\
         #![allow(clippy::derivable_impls, clippy::module_inception)]\n\n\
         #[cfg({NO_STD})]\n\
         extern crate alloc as std;\n\n"
    );
    render(&root, 0, &mut out);
    out
}

/// Lowers the workspace's system (rsdl reference §13) and embeds in each region
/// the catalog hash of its catalog. The hash is an input to the rsdl lowering,
/// which leaves it empty, so the driver fills it in.
///
/// The hash is `ridl_ir::catalog_hash::catalog_hash` over the region's package
/// and [`catalog_scope`], the same packages [`run_build`] gives
/// `ridl_descriptor::lower` for `--emit catalog`, so each region carries the
/// hash the catalog's descriptor carries (driftsys/ridl#367).
///
/// `packages` is every checked package of the workspace and `std_ir` the
/// lowered `ridl.std`. `None` when [`lower_system`] is.
pub fn lower_workspace_system(
    system: &CheckedSystem,
    packages: &[&ridl_ir::v2::Package],
    std_ir: &ridl_ir::v2::Package,
) -> Option<ridl_ir::v2::System> {
    let mut lowered = lower_system(system, packages)?;
    let others = catalog_scope(packages, Some(std_ir));
    embed_catalog_hashes(&mut lowered, packages, &others);
    Some(lowered)
}

/// The name of every `deployment` the checked model holds, in declaration
/// order.
///
/// This is the count the source states. [`lower_system`] leaves out a
/// deployment an RSDL-7xx error blocked (rsdl reference §13), so the lowered
/// system's list is shorter than this one on such a build, and counting it
/// would make a workspace that declares two deployments look like a workspace
/// that declares one.
fn declared_deployments(system: &CheckedSystem) -> Vec<String> {
    system
        .deployments
        .iter()
        .map(|deployment| deployment.name.name.clone())
        .collect()
}

/// Sets each region's hash of `lowered` to the catalog hash of its unit
/// (a region's catalog is the unit's name), computed over `others`, which
/// [`catalog_scope`] builds and which holds every package of the unit.
fn embed_catalog_hashes(
    lowered: &mut ridl_ir::v2::System,
    packages: &[&ridl_ir::v2::Package],
    others: &[&ridl_ir::v2::Package],
) {
    for region in &mut lowered.regions {
        assert!(
            ridl_ir::v2::members_of_unit(&region.catalog, packages)
                .next()
                .is_some(),
            "a region's catalog is a unit of the workspace"
        );
        region.hash = ridl_ir::catalog_hash::catalog_hash(&region.catalog, others).to_vec();
    }
}

/// Whether any package of the workspace names a declaration of `ridl.std`.
/// When it does, `ridl.std` is part of the build's [`catalog_scope`].
fn references_std(packages: &[&ridl_ir::v2::Package]) -> bool {
    packages
        .iter()
        .any(|package| ridl_ir::v2::referenced_packages(package).contains("ridl.std"))
}

/// The packages a package of the build is resolved against when the build
/// writes it: every checked package of the workspace in `packages`, then
/// `std_ir` (the lowered `ridl.std`) when it is given and a package of
/// `packages` names a declaration of `ridl.std`. A backend, the codegen model
/// and the catalog descriptor read it as `others`, and the catalog hash is
/// computed over it. The descriptor's hash and a region's hash are computed
/// over the same set of packages. The order of the list does not change the
/// hash, which keys every declaration by its qualified name.
///
/// [`run_build`] calls it, and the corpus harness calls it so that its
/// codegen snapshots are lowered over the same scope as a build.
pub fn catalog_scope<'a>(
    packages: &[&'a ridl_ir::v2::Package],
    std_ir: Option<&'a ridl_ir::v2::Package>,
) -> Vec<&'a ridl_ir::v2::Package> {
    let std_ir = std_ir.filter(|_| references_std(packages));
    packages.iter().copied().chain(std_ir).collect()
}

/// Whether `diagnostic` stops [`run_build`] writing any artifact: every error
/// except an RSDL-7xx one, which blocks only the lowering of its own
/// deployment (rsdl reference §13).
fn blocks_every_artifact(diagnostic: &Diagnostic) -> bool {
    diagnostic.severity == Severity::Error && !diagnostic.code.as_str().starts_with("RSDL-7")
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
    imports: Vec<BTreeMap<String, String>>,
    /// The checked rsdl model; its diagnostics are already in `diagnostics`.
    system: CheckedSystem,
    /// The diagnostics with the severities the emit sites chose; `lints` is
    /// not applied here (ADR-0024 decision 8).
    diagnostics: Vec<Diagnostic>,
    sources: SourceMap,
    /// The lint scopes the loader resolved, carried out unapplied for the
    /// entry points that report diagnostics (ADR-0024 decision 6).
    lints: LintScopes,
    /// The normalised `[codegen] header-file` text
    /// ([`LoadedWorkspace::codegen_header`]).
    codegen_header: Option<String>,
    /// The `[backend.<name>]` tables ([`LoadedWorkspace::backends`]).
    backends: BTreeMap<String, ridl_core::BackendTable>,
    /// The member directory the entry lies in
    /// ([`LoadedWorkspace::report_scope`]).
    report_scope: Option<PathBuf>,
    /// The root manifest's `[package]` name
    /// ([`LoadedWorkspace::root_package`]), read from the manifest the load
    /// already parsed rather than by finding the root and parsing it again
    /// (driftsys/ridl#760).
    root_package: Option<String>,
    /// The remote units the load took
    /// ([`LoadedWorkspace::remote_units`]); their packages are in `checked`.
    remote_units: BTreeMap<String, MaterializedUnit>,
}

/// Loads the workspace at `entry`, with the remote units of its `[imports]`
/// reached through `access`, and runs parse, resolve, and check over every
/// package, merging all diagnostics onto one [`SourceMap`].
fn load_and_check(
    db: &mut RidlDatabase,
    entry: &Path,
    overlays: &[ridl_core::Overlay],
    access: RemoteAccess<'_>,
) -> Result<Compiled, ridl_core::LoadError> {
    let std = std_package(db);
    let loaded = load_workspace_with_remotes(db, entry, overlays, access)?;
    Ok(check_loaded(db, std, loaded))
}

/// Runs parse, resolve, and check over every package of `loaded`, then the
/// workspace-wide passes, merging all diagnostics after the loader's onto the
/// loader's [`SourceMap`]. [`load_and_check`] calls it on a workspace read from
/// disk, and [`front_end`] on a one-file workspace built from a source text, so
/// once the workspace is loaded, `ridl check` and the `ridl_check` MCP tool run
/// the same passes.
fn check_loaded(db: &RidlDatabase, std: Package, loaded: LoadedWorkspace) -> Compiled {
    let LoadedWorkspace {
        workspace,
        mut diagnostics,
        mut sources,
        lints,
        codegen_header,
        backends,
        report_scope,
        units: _,
        root_package,
        imports: _,
        remote_units,
    } = loaded;

    let packages = workspace.packages(db).clone();
    let mut checked = Vec::with_capacity(packages.len());
    let mut resolutions: Vec<Resolution> = Vec::with_capacity(packages.len());
    let mut imports = Vec::with_capacity(packages.len());
    for pkg in &packages {
        // Intern this package's files into the render source map; their ids are
        // the render targets the package-relative pass diagnostics remap onto.
        let files = pkg.files(db).clone();
        let mut render_ids: Vec<FileId> = files
            .iter()
            .map(|file| sources.file_id(file.path(db), file.text(db)))
            .collect();
        // The checker stamps a lock diagnostic (RIDL-409) with the index after
        // the package's files, so the lock's own id goes last (plan decision
        // PD-12).
        if let Some(lock) = pkg.lock(db) {
            render_ids.push(sources.file_id(&lock.path, &lock.text));
        }

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
        imports.push(pkg.imports(db).clone());

        let checked_pkg = check_package(db, workspace, *pkg, std);
        diagnostics.extend(remap_diagnostics(
            checked_pkg.diagnostics.clone(),
            &render_ids,
        ));
        checked.push(checked_pkg);
    }

    // The workspace-wide passes carry FileIds indexing every source file in
    // package-then-file order. Rebuild that order onto the render source map
    // and remap, mirroring the per-package remap above.
    let CheckedWorkspace {
        diagnostics: workspace_diagnostics,
        system,
    } = check_workspace(db, workspace, std);
    if !workspace_diagnostics.is_empty() {
        let mut workspace_render_ids = Vec::new();
        for pkg in &packages {
            for file in pkg.files(db) {
                workspace_render_ids.push(sources.file_id(file.path(db), file.text(db)));
            }
        }
        diagnostics.extend(remap_diagnostics(
            workspace_diagnostics,
            &workspace_render_ids,
        ));
    }
    // RSDL-804 is raised here, not in the query, because only a driver knows
    // which backends are configured. No backend declares the namespaces it
    // consumes until the codegen plugin contract exists (ADR-0020), so the
    // command drivers claim none and every backend key draws the warning.
    diagnostics.extend(unclaimed_backend_keys(
        db,
        &system,
        &BTreeSet::new(),
        &mut sources,
    ));

    diagnostics.extend(check_design_lints(
        db,
        &packages,
        &checked,
        &resolutions,
        &mut sources,
    ));
    drop_remote_non_errors(&mut diagnostics, &sources, &remote_units);

    Compiled {
        workspace,
        std,
        checked,
        resolutions,
        imports,
        system,
        diagnostics,
        sources,
        lints,
        codegen_header,
        backends,
        report_scope,
        root_package,
        remote_units,
    }
}

/// A remote unit is a read-only dependency (ADR-0002 §5): every Warning and
/// Info whose primary span is in a file of a remote unit is dropped, before
/// any lint level is applied — so the consumer's `[lints]` never reach the
/// unit, wherever the cache lies — and every Error there is prefixed with the
/// unit name when the loader has not done so already. The loader applies the
/// same rule to its own findings; this applies it to the parser's, the
/// resolver's, the checker's and the workspace passes'.
fn drop_remote_non_errors(
    diagnostics: &mut Vec<Diagnostic>,
    sources: &SourceMap,
    remote_units: &BTreeMap<String, MaterializedUnit>,
) {
    diagnostics.retain_mut(|diagnostic| {
        let Some(path) = sources.path(diagnostic.primary.file) else {
            return true;
        };
        let path = Path::new(path);
        let Some((unit, _)) = remote_units
            .iter()
            .find(|(_, unit)| path.starts_with(&unit.dir))
        else {
            return true;
        };
        if diagnostic.severity != Severity::Error {
            return false;
        }
        let prefix = format!("remote unit `{unit}`: ");
        if !diagnostic.message.starts_with(&prefix) {
            diagnostic.message.insert_str(0, &prefix);
        }
        true
    });
}

/// The codegen request for one package (ADR-0020 decision 9; the IR
/// specification §7): the schema name and this toolchain's version first,
/// the model lowered over `others` — the same scope every code emit reads,
/// so the request's `model` is the same JSON value as the
/// `--emit codegen-model` artifact, one indentation level deeper — the
/// backend `options`, the artifact base `ridlc` names this package's files
/// after, and the selected `deployment` section. [`write_emits`] builds it once
/// per package and hands each in-tree backend and each plugin a copy whose
/// `options` are those of that backend's own `[backend.<name>]` table; a test
/// that wants the bytes a plugin sees builds it here.
pub fn codegen_request(
    base: &str,
    package: &ridl_ir::v2::Package,
    others: &[&ridl_ir::v2::Package],
    compatible: &[[u8; 32]],
    options: Vec<v1::BackendOption>,
    deployment: Option<v1::Deployment>,
    header: Option<&str>,
) -> v1::CodegenRequest {
    v1::CodegenRequest {
        schema: codegen::SCHEMA.to_string(),
        toolchain: env!("CARGO_PKG_VERSION").to_string(),
        model: Some(codegen::lower_with(package, others, compatible)),
        options,
        artifact_base: base.to_string(),
        deployment,
        generated_marker: codegen::generated_marker(Some(&package.name)),
        header: header.unwrap_or_default().to_string(),
    }
}

/// A `--deployment` name that no deployment of the system carries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownDeployment {
    /// The name the caller gave.
    pub requested: String,
    /// The names of the deployments the workspace declares, sorted.
    pub known: Vec<String>,
    /// Whether the workspace has a `system` at all. With `known` empty, this
    /// is what separates the two reasons no deployment can be named: no
    /// `system` to carry one, or a `system` with no `deployment` block. Both
    /// are reachable, and naming the wrong one sends a reader to the wrong
    /// file.
    pub has_system: bool,
}

impl std::fmt::Display for UnknownDeployment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match (self.known.is_empty(), self.has_system) {
            (true, false) => write!(
                f,
                "no deployment named `{}`; the workspace declares no system, so no deployment can be named",
                self.requested
            ),
            (true, true) => write!(
                f,
                "no deployment named `{}`; the system declares no deployment, so no deployment can be named",
                self.requested
            ),
            (false, _) => write!(
                f,
                "no deployment named `{}`; the system declares: {}",
                self.requested,
                self.known.join(", ")
            ),
        }
    }
}

impl std::error::Error for UnknownDeployment {}

/// The deployment section every codegen request of one build carries.
///
/// `declared` is the name of every `deployment` the source declares, in any
/// order ([`WorkspaceOutput::declared_deployments`]). It decides how many
/// deployments the workspace has, and `system` — the lowered system, which
/// leaves out a deployment an RSDL-7xx error blocked (rsdl reference §13) —
/// supplies the facts of the one selected. Counting the lowered system's
/// deployments instead would carry one deployment silently when the source
/// declares two and an error dropped one.
///
/// With a `name`: the deployment of that name, or [`UnknownDeployment`] when
/// the lowered system has none of that name. A name the source declares but
/// the lowering dropped is such an error; the caller resolves that case by
/// reporting the error it already holds, which takes precedence over the
/// unknown name. The error reads `has_system` off `system`, which is also
/// `None` when an error in the closure blocked the lowering (rsdl reference
/// §13) — that case is an error, so the caller's precedence rule reports it
/// and the message is never rendered. Without a `name`: the deployment when
/// the source declares
/// exactly one, and `None` for several declarations or none, so that a request
/// for such a workspace has no deployment section at all. `None` as well when
/// the source declares exactly one and the lowering dropped it, which is an
/// RSDL-7xx error the caller already holds and reports.
///
/// `packages` holds every package of the workspace, `ridl.std` included.
pub fn select_deployment(
    system: Option<&ridl_ir::v2::System>,
    declared: &[String],
    name: Option<&str>,
    packages: &[&ridl_ir::v2::Package],
) -> Result<Option<v1::Deployment>, UnknownDeployment> {
    let lowered = |wanted: &str| {
        system.and_then(|system| codegen::lower_deployment(system, wanted, packages))
    };
    match name {
        Some(requested) => lowered(requested).map(Some).ok_or_else(|| {
            let mut known = declared.to_vec();
            known.sort();
            UnknownDeployment {
                requested: requested.to_string(),
                known,
                has_system: system.is_some(),
            }
        }),
        None => Ok(requested_or_only(declared, None).and_then(lowered)),
    }
}

/// The name of the deployment a build selects, before the lowering says
/// whether it exists: the requested `name`, or, with none, the one deployment
/// the source declares, and `None` for several declarations or none
/// ([`select_deployment`] holds the rule in full). RSDL-807 reads this to
/// know which deployment's links to compare, before the levels are applied.
fn requested_or_only<'a>(declared: &'a [String], name: Option<&'a str>) -> Option<&'a str> {
    match (name, declared) {
        (Some(requested), _) => Some(requested),
        (None, [only]) => Some(only.as_str()),
        (None, _) => None,
    }
}

/// Where a response came from, for the diagnostics and the refusals it
/// draws: an in-tree backend's message is reported as it is, because the
/// backend is part of `ridlc` and its messages are pinned by the corpus
/// snapshots; a plugin's is prefixed with the plugin's name, because the
/// plugin is not.
#[derive(Clone, Copy)]
enum Origin<'a> {
    InTree { language: &'a str },
    Plugin { name: &'a str },
}

impl Origin<'_> {
    fn describe(self) -> String {
        match self {
            Origin::InTree { language } => format!("the {language} backend"),
            Origin::Plugin { name } => format!("plugin `{name}`"),
        }
    }
}

/// Writes the files of one response under `out_dir`, or none of them.
///
/// The response's diagnostics are recorded first, each at its own severity,
/// an unset or unknown severity counting as an error (`plugin.proto`). Then,
/// if any is an error, no file is written — a backend that failed produced
/// no artifact, as before the contract. Otherwise every path is checked
/// against [`codegen::check_path`] before any file is written, so a
/// response with one path that would escape `out_dir` writes nothing at
/// all, and the refusal names its origin and the path. A path with
/// directories in it has them created.
fn write_response(
    out_dir: &Path,
    origin: Origin<'_>,
    response: &v1::CodegenResponse,
    diagnostics: &mut Vec<Diagnostic>,
) -> std::io::Result<()> {
    for diagnostic in &response.diagnostics {
        let severity = match v1::DiagnosticSeverity::try_from(diagnostic.severity) {
            Ok(v1::DiagnosticSeverity::Warning) => Severity::Warning,
            Ok(v1::DiagnosticSeverity::Info) => Severity::Info,
            Ok(v1::DiagnosticSeverity::Error | v1::DiagnosticSeverity::Unspecified) | Err(_) => {
                Severity::Error
            }
        };
        let message = match origin {
            Origin::InTree { .. } => diagnostic.message.clone(),
            Origin::Plugin { name } => format!("plugin `{name}`: {}", diagnostic.message),
        };
        diagnostics.push(Diagnostic {
            severity,
            ..error_diagnostic("", message, FileId::DETACHED, TextRange::default())
        });
    }
    if codegen::has_error(response) {
        return Ok(());
    }

    let mut refused = false;
    for file in &response.files {
        if let Err(reason) = codegen::check_path(&file.path) {
            refused = true;
            diagnostics.push(error_diagnostic(
                "",
                format!(
                    "{} returned a file path `{}` that ridlc will not write: {reason}",
                    origin.describe(),
                    file.path
                ),
                FileId::DETACHED,
                TextRange::default(),
            ));
        }
    }
    if refused {
        return Ok(());
    }

    for file in &response.files {
        let path = out_dir.join(&file.path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        match &file.content {
            Some(v1::generated_file::Content::Text(text)) => std::fs::write(&path, text)?,
            Some(v1::generated_file::Content::Binary(bytes)) => std::fs::write(&path, bytes)?,
            None => std::fs::write(&path, b"")?,
        }
    }
    Ok(())
}

/// Writes the catalog descriptor of every unit of the build that has at least
/// one interface shape, to `<unit>.catalog.binfb` in `out_dir`, in unit name
/// order; a unit with no interface shape gets no file. `packages` are the
/// build's checked packages, which name the units through
/// `ridl_ir::v2::unit_of`; `others` is the scope the catalogs are lowered
/// over ([`catalog_scope`]); `compatible` holds, per unit name, the earlier
/// catalog hashes the descriptor lists, a unit absent from it listing none.
/// A lowering failure is an internal error, not a
/// diagnostic: it is returned as an I/O error, which stops the build and
/// which the command reports with exit code 2 (ADR-0010 decision 1).
fn write_catalogs(
    out_dir: &Path,
    packages: &[&ridl_ir::v2::Package],
    others: &[&ridl_ir::v2::Package],
    compatible: &BTreeMap<String, Vec<[u8; 32]>>,
) -> std::io::Result<()> {
    let units: BTreeSet<&str> = packages
        .iter()
        .filter(|package| package.shapes().next().is_some())
        .map(|package| ridl_ir::v2::unit_of(package))
        .collect();
    for unit in units {
        let earlier = compatible.get(unit).map_or(&[][..], Vec::as_slice);
        let bytes = ridl_descriptor::lower(unit, others, earlier)
            .map_err(|err| std::io::Error::other(err.to_string()))?;
        std::fs::write(
            out_dir.join(format!("{unit}{}", ridl_descriptor::FILE_SUFFIX)),
            bytes,
        )?;
    }
    Ok(())
}

/// Writes the selected `emits`, then the `plugins`, for one package's IR
/// into `out_dir`.
///
/// Every code emit but [`Emit::Catalog`] goes through the backend contract
/// ([`codegen::Backend`], ADR-0020 decision 9): one [`codegen_request`] is
/// built for the package — the model lowered once — and each in-tree
/// backend is called, as a plugin would be, over a copy of it whose `options`
/// are those of the backend's own `[backend.<name>]` table, the response
/// written by [`write_response`]. The Rust backend is
/// [`ridl_backend_rust::Backend`] over
/// [`generate_pipeline`](ridl_backend_rust::generate_pipeline), which emits
/// the interaction face and the descriptors beside the domain types and the
/// codec when the package's `[backend.rust] encodings` names a codec, and
/// the domain types only otherwise; TypeScript, proto3 and FlatBuffers are
/// their own crates' `Backend`; `codegen-model` is [`codegen::ModelBackend`],
/// the model written back, and `codegen-request` is
/// [`codegen::RequestBackend`], the request itself written back. The Rust backend reads the request and never the raw IR, as
/// `codegen-model` does and as a plugin must; the other three still read the
/// raw IR, so each is constructed with a [`codegen::RawIr`] — the package and
/// `others`, the caller's full package list ([`run_build`]), which proto3 and
/// FlatBuffers read to resolve a cross-package reference themselves — and reads
/// that in place of the request's model. A backend that cannot render this
/// package answers with an error diagnostic and no file, and only its own
/// artifact is skipped.
///
/// [`Emit::Catalog`] calls no backend and writes nothing here: the catalogs
/// are per unit, not per package, and [`write_catalogs`] writes them once
/// after every package.
///
/// The `ir-json`, `ir-text` and `ir-binary` emits are direct IR dumps, not
/// backends: they need no request. When the package cannot be rendered in
/// that encoding (ADR-0014 decisions 12 and 14) the failure is a diagnostic
/// and no artifact is written; `ir-binary` has no failure path (decision 7).
///
/// Each plugin runs after the emits, over a copy of the same request with the
/// `options` of its own `[backend.<name>]` table, through the process host
/// ([`plugin::run`]); a host failure is an error diagnostic naming the plugin,
/// and a response is written as an in-tree backend's is.
///
/// `compatible` is the list of earlier catalog hashes of the package's unit,
/// carried into the request's `catalog.compatible` as given.
#[expect(
    clippy::too_many_arguments,
    reason = "the build's per-package facts, passed once from `run_build_with`"
)]
fn write_emits(
    out_dir: &Path,
    base: &str,
    ir: &ridl_ir::v2::Package,
    others: &[&ridl_ir::v2::Package],
    compatible: &[[u8; 32]],
    emits: &[Emit],
    plugins: &[plugin::Plugin],
    plugin_timeout: Duration,
    deployment: Option<&v1::Deployment>,
    header: Option<&str>,
    backends: &BTreeMap<String, Vec<String>>,
    diagnostics: &mut Vec<Diagnostic>,
) -> std::io::Result<()> {
    // One request per package, built only when something reads it: an IR
    // dump alone lowers nothing.
    let needs_request = !plugins.is_empty() || emits.iter().any(|emit| !emit.is_ir_dump());
    let request = needs_request.then(|| {
        codegen_request(
            base,
            ir,
            others,
            compatible,
            Vec::new(),
            deployment.cloned(),
            header,
        )
    });
    // Each backend is handed the request with its own `[backend.<name>]`
    // options; a table for another backend is not visible to it.
    let request_for = |language: &str| {
        request.as_ref().map(|request| v1::CodegenRequest {
            options: backend_options(backends, language),
            ..request.clone()
        })
    };
    let raw = codegen::RawIr {
        package: ir,
        others,
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
        let backend: Box<dyn codegen::Backend + '_> = match emit {
            Emit::Rust => Box::new(ridl_backend_rust::Backend),
            Emit::TypeScript => Box::new(ridl_backend_ts::Backend::new(raw)),
            Emit::Proto => Box::new(ridl_backend_proto::Backend::new(raw)),
            Emit::Flatbuffers => Box::new(ridl_backend_flatbuffers::Backend::new(raw)),
            Emit::CodegenModel => Box::new(codegen::ModelBackend),
            Emit::CodegenRequest => Box::new(codegen::RequestBackend),
            // Written once per build by `write_catalogs`, not per package.
            Emit::Catalog => continue,
            Emit::IrJson => match ridl_ir::v2::to_json_pretty(ir) {
                Ok(json) => {
                    std::fs::write(ir_dump_path(out_dir, base, *emit), json)?;
                    continue;
                }
                // The pbjson-generated writer has no nesting limit (ADR-0014
                // decision 14); its one remaining failure is an enum field
                // holding a discriminant outside the schema — a tool-level
                // failure with no source span, reported the way a backend's
                // refusal is, with no artifact written.
                Err(err) => {
                    diagnostics.push(error_diagnostic(
                        "",
                        err.to_string(),
                        FileId::DETACHED,
                        TextRange::default(),
                    ));
                    continue;
                }
            },
            // Prototext still transcodes through the descriptor pool and
            // keeps the recursion-limit failure mode JSON lost (ADR-0014
            // decisions 12 and 14): the failure is a diagnostic and no
            // artifact is written.
            Emit::IrText => match ridl_ir::v2::to_text_format(ir) {
                Ok(text) => {
                    std::fs::write(ir_dump_path(out_dir, base, *emit), text)?;
                    continue;
                }
                Err(err) => {
                    diagnostics.push(error_diagnostic(
                        "",
                        err.to_string(),
                        FileId::DETACHED,
                        TextRange::default(),
                    ));
                    continue;
                }
            },
            // Binary needs no descriptors and no transcode, so it has no
            // recursion-limit failure path (ADR-0014 decision 7). The
            // interaction class is checked here, because `to_binary` cannot
            // fail and the strict reader refuses an invalid combination.
            Emit::IrBinary => {
                if let Err(err) = ridl_ir::class::check_package(ir) {
                    diagnostics.push(error_diagnostic(
                        "",
                        err.to_string(),
                        FileId::DETACHED,
                        TextRange::default(),
                    ));
                    continue;
                }
                std::fs::write(
                    ir_dump_path(out_dir, base, *emit),
                    ridl_ir::v2::to_binary(ir),
                )?;
                continue;
            }
        };
        let table = emit
            .backend_table()
            .expect("a code emit reads a backend table");
        let request = request_for(table).expect("a code emit builds the request");
        let response = backend.generate(&request);
        write_response(
            out_dir,
            Origin::InTree {
                language: backend.language(),
            },
            &response,
            diagnostics,
        )?;
    }

    for plugin in plugins {
        let request = request_for(&plugin.language).expect("a plugin builds the request");
        let name = plugin.name();
        match plugin::run(plugin, &request, plugin_timeout) {
            Ok(response) => {
                write_response(
                    out_dir,
                    Origin::Plugin { name: &name },
                    &response,
                    diagnostics,
                )?;
            }
            Err(err) => diagnostics.push(error_diagnostic(
                "",
                err.to_string(),
                FileId::DETACHED,
                TextRange::default(),
            )),
        }
    }
    Ok(())
}

/// The options a backend receives for its `[backend.<name>]` table: none when
/// the manifest has no table for `language` or the table names no encoding,
/// otherwise one `encodings` option whose value joins the names with `,` in
/// manifest order.
fn backend_options(
    backends: &BTreeMap<String, Vec<String>>,
    language: &str,
) -> Vec<v1::BackendOption> {
    match backends.get(language) {
        Some(names) if !names.is_empty() => vec![v1::BackendOption {
            key: ridl_core::manifest::ENCODINGS_KEY.to_string(),
            value: names.join(","),
        }],
        _ => Vec::new(),
    }
}

/// Writes the lowered system (rsdl reference §13) once for each IR dump in
/// `emits`, to `<out_dir>/<pkg.Name><suffix>` with the suffix
/// [`Emit::system_dump_suffix`] names. A system that cannot be rendered in an
/// encoding is recorded as a detached error diagnostic and no artifact is
/// written, as [`write_emits`] does for a package.
fn write_system_emits(
    out_dir: &Path,
    system: &ridl_ir::v2::System,
    emits: &[Emit],
    diagnostics: &mut Vec<Diagnostic>,
) -> std::io::Result<()> {
    for emit in emits {
        #[deny(
            clippy::wildcard_enum_match_arm,
            clippy::match_wildcard_for_single_variants
        )]
        let rendered = match emit {
            Emit::Rust
            | Emit::TypeScript
            | Emit::Proto
            | Emit::Flatbuffers
            | Emit::CodegenModel
            | Emit::CodegenRequest
            | Emit::Catalog => continue,
            Emit::IrJson => ridl_ir::v2::system_to_json_pretty(system).map(String::into_bytes),
            Emit::IrText => ridl_ir::v2::system_to_text_format(system).map(String::into_bytes),
            Emit::IrBinary => Ok(ridl_ir::v2::system_to_binary(system)),
        };
        let suffix = emit
            .system_dump_suffix()
            .expect("only IR dump emits write the system");
        match rendered {
            Ok(bytes) => std::fs::write(
                out_dir.join(format!("{}{suffix}", system.qualified_name())),
                bytes,
            )?,
            Err(err) => diagnostics.push(error_diagnostic(
                "",
                err.to_string(),
                FileId::DETACHED,
                TextRange::default(),
            )),
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

/// The manifest root governing `entry` — the root the loader loads from
/// ([`ridl_core::find_root`]), where `ridl.lock` lives. `None` means
/// single-file mode: a `.typl` file with no manifest anywhere up the tree.
fn manifest_root_of(entry: &Path) -> Option<PathBuf> {
    if entry.is_file() {
        entry.parent().and_then(ridl_core::find_root)
    } else if entry.is_dir() {
        ridl_core::find_root(entry)
    } else {
        None
    }
}

#[cfg(test)]
mod render_lib_rs_tests {
    use super::render_lib_rs;

    /// A package name that is a strict prefix of another (`veh` alongside
    /// `veh.common`) lands in `render_lib_rs`'s non-leaf arm, which — without
    /// the fix — emits an inline `pub mod veh { … }` for the dotted sibling
    /// and drops `veh`'s own file entirely. Both files must stay reachable,
    /// and `veh`'s items must sit at `crate::veh`, which is where generated
    /// code names them. Nesting the file under its own name keeps it
    /// reachable at `crate::veh::veh` and still fails every reference: this
    /// test compiles one to say so.
    #[test]
    fn a_package_name_that_is_a_prefix_of_another_keeps_both_files_reachable() {
        let names = ["veh".to_string(), "veh.common".to_string()];
        let lib = render_lib_rs(&names, "");

        assert!(
            lib.contains("#[path = \"veh.rs\"]"),
            "veh's own file must stay reachable, lib.rs was:\n{lib}"
        );
        assert!(
            lib.contains("#[path = \"veh.common.rs\"]"),
            "veh.common's file must stay reachable, lib.rs was:\n{lib}"
        );

        // The proof. `veh.common` names a type from `veh` the way the Rust
        // backend does, as `crate::veh::…`.
        let dir = tempfile::tempdir().expect("a temp dir is created");
        std::fs::write(dir.path().join("veh.rs"), "pub struct Speed(pub f64);\n")
            .expect("the prefix package is written");
        std::fs::write(
            dir.path().join("veh.common.rs"),
            "pub struct Reading(pub crate::veh::Speed);\n",
        )
        .expect("the dotted package is written");
        std::fs::write(dir.path().join("lib.rs"), &lib).expect("the crate root is written");

        // Held across the spawn: a fork here must not land inside another
        // test's write of a script it then executes (`SPAWNS`).
        let spawning = crate::spawning();
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
            .arg(dir.path().join("prefix.rmeta"))
            .arg(dir.path().join("lib.rs"))
            .status()
            .expect("rustc must be installed and runnable for this test to be meaningful");
        drop(spawning);
        assert!(
            status.success(),
            "a reference to the prefix package must resolve at crate::veh, lib.rs was:\n{lib}"
        );
    }

    /// Compiles `lib.rs` as a crate root over one package file, with `cfgs`
    /// passed to `rustc` and for `target` when one is given, and returns
    /// whether it compiled.
    ///
    /// The package file names the `String` and `Vec` paths and the `to_vec`
    /// call the backend emits, and, under `validate-pattern`, the
    /// `::std::sync::LazyLock` the pattern check holds its `Regex` in. It
    /// names no `ridl_rt` item, so no `ridl-rt` rlib is needed for any target.
    fn compiles_with(lib: &str, cfgs: &[&str], target: Option<&str>) -> bool {
        let dir = tempfile::tempdir().expect("a temp dir is created");
        std::fs::write(
            dir.path().join("veh.rs"),
            "pub struct Label(pub ::std::string::String, pub ::std::vec::Vec<u8>);\n\
             pub fn label() -> Label {\n\
                 Label(::std::string::String::from(\"a\"), b\"a\".to_vec())\n\
             }\n\
             #[cfg(feature = \"validate-pattern\")]\n\
             pub static PATTERN: ::std::sync::LazyLock<u8> = ::std::sync::LazyLock::new(|| 0);\n",
        )
        .expect("the package is written");
        std::fs::write(dir.path().join("lib.rs"), lib).expect("the crate root is written");
        let mut command = std::process::Command::new("rustc");
        command.args([
            "--edition",
            "2024",
            "--crate-type",
            "lib",
            "--emit",
            "metadata",
        ]);
        if let Some(target) = target {
            command.args(["--target", target]);
        }
        for cfg in cfgs {
            command.args(["--cfg", cfg]);
        }
        // Held across the spawn, as above.
        let spawning = crate::spawning();
        let compiled = command
            .arg("-o")
            .arg(dir.path().join("root.rmeta"))
            .arg(dir.path().join("lib.rs"))
            .status()
            .expect("rustc must be installed and runnable for this test to be meaningful")
            .success();
        drop(spawning);
        compiled
    }

    /// The crate root is `no_std` exactly when neither `std` nor
    /// `validate-pattern` is on, and links `alloc` as `std` under the same
    /// condition.
    ///
    /// - With no feature, it compiles for `thumbv7em-none-eabihf`, which has
    ///   no standard library (rust-toolchain.toml installs it): a root that
    ///   is not `no_std` there fails on the missing `std`, and a `no_std`
    ///   root without the alias fails on the `String` and `Vec` paths.
    /// - With `validate-pattern` on, it compiles with the `LazyLock` item:
    ///   a root that is `no_std` there, or that links `alloc` as `std` there,
    ///   resolves `::std::sync` in `alloc`, which has no `LazyLock`. So the
    ///   alias must share `no_std`'s condition: when it is wider, a
    ///   std-only path no longer reaches the standard library.
    /// - With `std` on, it compiles as an ordinary crate.
    ///
    /// The lines themselves, and their order, are pinned as literals.
    #[test]
    fn the_crate_root_is_no_std_exactly_without_std_and_validate_pattern() {
        let lib = render_lib_rs(&["veh".to_string()], "");
        assert!(
            compiles_with(&lib, &[], Some("thumbv7em-none-eabihf")),
            "no feature on, for a target with no standard library: lib.rs was:\n{lib}"
        );
        assert!(
            compiles_with(&lib, &[r#"feature="validate-pattern""#], None),
            "`validate-pattern` on: lib.rs was:\n{lib}"
        );
        assert!(
            compiles_with(&lib, &[r#"feature="std""#], None),
            "`std` on: lib.rs was:\n{lib}"
        );
        assert!(
            lib.starts_with(
                "#![cfg_attr(not(any(feature = \"std\", feature = \"validate-pattern\")), no_std)]\n\
                 #![allow(clippy::derivable_impls, clippy::module_inception)]\n\
                 \n\
                 #[cfg(not(any(feature = \"std\", feature = \"validate-pattern\")))]\n\
                 extern crate alloc as std;\n"
            ),
            "lib.rs was:\n{lib}"
        );
    }
}

#[cfg(test)]
mod emit_table_tests {
    use super::Emit;

    /// Each code emit reads the table named after its language, the codegen
    /// model reads `codegen-model`, and an emit that is not a backend reads
    /// none. The names are what a manifest's `[backend.<name>]` key spells.
    #[test]
    fn each_emit_reads_its_own_backend_table() {
        assert_eq!(Emit::Rust.backend_table(), Some("rust"));
        assert_eq!(Emit::TypeScript.backend_table(), Some("typescript"));
        assert_eq!(Emit::Proto.backend_table(), Some("proto"));
        assert_eq!(Emit::Flatbuffers.backend_table(), Some("flatbuffers"));
        assert_eq!(Emit::CodegenModel.backend_table(), Some("codegen-model"));
        assert_eq!(
            Emit::CodegenRequest.backend_table(),
            Some("codegen-request")
        );
        for emit in [Emit::Catalog, Emit::IrJson, Emit::IrText, Emit::IrBinary] {
            assert_eq!(emit.backend_table(), None, "{emit:?}");
        }
    }

    /// Only the Rust emit carries a codec, so RSDL-807 compares only its
    /// table among the in-tree emits.
    #[test]
    fn only_the_rust_emit_carries_a_codec() {
        assert!(Emit::Rust.carries_codec());
        for emit in [
            Emit::TypeScript,
            Emit::Proto,
            Emit::Flatbuffers,
            Emit::CodegenModel,
            Emit::CodegenRequest,
            Emit::Catalog,
            Emit::IrJson,
            Emit::IrText,
            Emit::IrBinary,
        ] {
            assert!(!emit.carries_codec(), "{emit:?}");
        }
    }
}

#[cfg(test)]
mod backend_options_tests {
    use std::collections::BTreeMap;

    use super::backend_options;

    fn tables() -> BTreeMap<String, Vec<String>> {
        BTreeMap::from([
            (
                "rust".to_string(),
                vec!["flatbuffers".to_string(), "proto3".to_string()],
            ),
            ("kotlin".to_string(), vec!["flatbuffers".to_string()]),
            ("proto".to_string(), Vec::new()),
        ])
    }

    fn encodings(language: &str) -> Vec<(String, String)> {
        backend_options(&tables(), language)
            .into_iter()
            .map(|option| (option.key, option.value))
            .collect()
    }

    #[test]
    fn the_rust_request_carries_the_rust_encodings_only() {
        let pair = |value: &str| {
            vec![(
                ridl_core::manifest::ENCODINGS_KEY.to_string(),
                value.to_string(),
            )]
        };
        assert_eq!(encodings("rust"), pair("flatbuffers,proto3"));
        assert_eq!(encodings("kotlin"), pair("flatbuffers"));
        assert!(encodings("typescript").is_empty());
        assert!(
            encodings("proto").is_empty(),
            "a table that names no encoding sends no option"
        );
    }
}
