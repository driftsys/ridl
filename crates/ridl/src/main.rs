//! The `ridl` toolchain facade — the porcelain layer (concept note §8.1,
//! docs/ROADMAP.md epic E1.13). The cargo/deno-style front door with humane
//! defaults: `PATH` defaults to the current directory.
//!
//! `ridl check` and `ridl build` delegate to the `ridlc` library face;
//! `ridl fmt` runs the `ridl-fmt` formatter over `.typl` files (E1.14). The exit
//! code is 0 clean, 1 on a diagnostic error (or, for `fmt --check`, a file that
//! would change), and 2 on an input/output or usage error.
//!
//! `ridl diff` compares two IR snapshots or source trees through the
//! `ridl-diff` engine (E2.8a). It carries its own exit contract — 0 compatible
//! or identical, 1 breaking, 2 error (concept note §9.1, ADR-0008 decision 9) —
//! and never touches `ridlc`'s source→IR boundary beyond compiling each side.
//!
//! `ridl test` runs the property suite over a workspace (E2.11a): the range
//! self-corpora derived from the E1.18 generators, and satisfiability sampling
//! of every `require` clause. It carries the same 0/1/2 exit contract, with 1
//! reserved for a self-corpus failure or an evaluation error.
//!
//! `ridl baseline` and `ridl check --baseline` are the desk-time half of that
//! engine (E2.9, general form §6.3): `baseline` publishes one `.ir.json`
//! snapshot per package, and `check` compares the workspace against those
//! snapshots and warns (RIDL-407) when an interaction's ordinal moved. Both live
//! here rather than in `ridlc` because reading a workspace-local baseline is not
//! part of the source→IR function the tool qualification argument covers
//! (ADR-0008 decision 9).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

mod property;

use clap::{Parser, Subcommand};
use ridl_core::diag::{DiagCode, Diagnostic, FileId, Severity, SourceMap, Span, render};
use ridl_fmt::{FormatOutcome, format};
use ridl_syntax::ast::{AstNode as _, HasName as _, InterfaceMember, Name, SourceFile};
use ridlc::{CliRun, Emit};
use rowan::{TextRange, TextSize};

#[derive(Parser)]
#[command(name = "ridl", about = "The RIDL toolchain", version)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Type-check a file, package, or workspace (defaults to the current
    /// directory).
    Check {
        #[arg(default_value = ".")]
        path: PathBuf,
        /// Verify remote imports against `ridl.lock` without fetching or
        /// regenerating it (CI mode, ADR-0002 §7).
        #[arg(long)]
        frozen: bool,
        /// Compare the checked workspace against a published baseline — a
        /// directory of `.ir.json` snapshots or one snapshot file — and warn
        /// (RIDL-407) on every interaction whose ordinal moved. Without the
        /// flag, `.ridl/baseline/` at the workspace root is used when it
        /// exists.
        #[arg(long, value_name = "DIR|FILE")]
        baseline: Option<PathBuf>,
    },
    /// Publish the current workspace as a baseline: one `<pkg-name>.ir.json`
    /// snapshot per package, written to `.ridl/baseline/` at the workspace
    /// root.
    Baseline {
        #[arg(default_value = ".")]
        path: PathBuf,
        /// Write the snapshots here instead of `.ridl/baseline/`.
        #[arg(long, value_name = "DIR")]
        out: Option<PathBuf>,
    },
    /// Compile to the selected artifacts (defaults to the current directory).
    Build {
        #[arg(default_value = ".")]
        path: PathBuf,
        #[arg(long, default_value = "out")]
        out_dir: PathBuf,
        #[arg(long, value_delimiter = ',', default_value = "rust")]
        emit: Vec<Emit>,
        /// Verify remote imports against `ridl.lock` without fetching or
        /// regenerating it (CI mode, ADR-0002 §7).
        #[arg(long)]
        frozen: bool,
    },
    /// Run the property suite over a workspace: the range self-corpora and the
    /// contract-clause sampling (ridl §13). Exit 0 when every run passes, 1 on
    /// a self-corpus failure or an evaluation error, 2 on a compile error.
    Test {
        #[arg(default_value = ".")]
        path: PathBuf,
        /// Random parameter tuples drawn per `require` clause (minimum 1). Each
        /// clause also runs its parameters' boundary corpus, which is drawn
        /// first and is not counted here, so the total per clause is larger.
        #[arg(long, default_value_t = 256)]
        samples: usize,
        /// Output format for the report.
        #[arg(long, value_enum, default_value = "text")]
        format: property::TestFormat,
    },
    /// Reformat `.typl` and `.ridl` files in place (defaults to the current
    /// directory).
    Fmt {
        #[arg(default_value = ".")]
        path: PathBuf,
        /// Do not write; exit 1 if any file would change.
        #[arg(long)]
        check: bool,
    },
    /// Compare two IR snapshots or source trees and classify the change:
    /// exit 0 compatible or identical, 1 breaking, 2 error.
    Diff {
        /// The baseline: an `.ir.json` snapshot, a `.typl`/`.ridl` file, a
        /// package directory, or a workspace root.
        old: Option<PathBuf>,
        /// The candidate, in the same forms as the baseline.
        new: Option<PathBuf>,
        /// Output format for the report.
        #[arg(long, value_enum, default_value = "text")]
        format: DiffFormat,
        /// Print the classification rule for one change category and exit,
        /// instead of comparing snapshots. Takes a category exactly as the
        /// report prints it, e.g. `timing_changed`.
        #[arg(long, value_name = "CATEGORY")]
        explain: Option<String>,
    },
}

/// The `ridl diff` output format — human-readable text or machine-readable
/// JSON with a stable schema.
#[derive(Clone, Copy, Debug, clap::ValueEnum)]
enum DiffFormat {
    Text,
    Json,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli.command {
        Command::Check {
            path,
            frozen,
            baseline,
        } => run_check(&path, frozen, baseline.as_deref()),
        Command::Baseline { path, out } => run_baseline(&path, out.as_deref()),
        Command::Build {
            path,
            out_dir,
            emit,
            frozen,
        } => finish(ridlc::run_build(&path, &out_dir, &emit, frozen.into())),
        Command::Test {
            path,
            samples,
            format,
        } => property::run(&path, samples, format),
        Command::Fmt { path, check } => run_fmt(&path, check),
        Command::Diff {
            old,
            new,
            format,
            explain,
        } => match explain {
            Some(category) => run_explain(&category),
            None => match (old, new) {
                (Some(old), Some(new)) => run_diff(&old, &new, format),
                _ => {
                    eprintln!(
                        "error: `ridl diff` needs both an old and a new input, \
                         or `--explain <CATEGORY>`"
                    );
                    ExitCode::from(2)
                }
            },
        },
    }
}

/// Prints the classification rule for one change category — the table of
/// ADR-0008 decision 14 as text, and the CI-facing documentation of record until
/// the E4 error index publishes it. An unknown category is a usage error: exit 2
/// with the valid words listed.
fn run_explain(category: &str) -> ExitCode {
    match ridl_diff::category_from_word(category) {
        Some(category) => {
            println!("{}", ridl_diff::category_word(category));
            println!("{}", ridl_diff::explain(category));
            ExitCode::SUCCESS
        }
        None => {
            eprintln!("error: unknown change category `{category}`");
            eprintln!("the categories `ridl diff` reports are:");
            for known in ridl_diff::CATEGORIES {
                eprintln!("  {}", ridl_diff::category_word(known));
            }
            ExitCode::from(2)
        }
    }
}

/// Compares the `old` and `new` inputs and renders the report to stdout,
/// returning the diff exit code: 2 on an I/O or compile error while loading
/// either side, 1 when the change is breaking, 0 when it is compatible or the
/// two are identical.
fn run_diff(old: &Path, new: &Path, format: DiffFormat) -> ExitCode {
    let old_packages = match load_diff_side(old) {
        Ok(packages) => packages,
        Err(code) => return code,
    };
    let new_packages = match load_diff_side(new) {
        Ok(packages) => packages,
        Err(code) => return code,
    };

    let report = ridl_diff::diff_sets(&old_packages, &new_packages);
    // `render_text` already terminates every line, so it prints as is; the JSON
    // rendering has no trailing newline and gets one.
    match format {
        DiffFormat::Text => print!("{}", ridl_diff::render_text(&report)),
        DiffFormat::Json => println!("{}", ridl_diff::render_json(&report)),
    }

    match report.verdict {
        ridl_diff::Verdict::Breaking => ExitCode::FAILURE,
        ridl_diff::Verdict::Compatible | ridl_diff::Verdict::Identical => ExitCode::SUCCESS,
    }
}

/// Loads one side of a diff into a set of resolved packages.
///
/// Three input forms, in order:
///
/// 1. an `.ir.json` file — deserialized directly;
/// 2. a directory holding `.ir.json` files — deserialized as a snapshot set.
///    This is the form `.ridl/baseline/` takes, and an N-package workspace
///    publishes N snapshots, so `ridl diff .ridl/baseline .` has to read the
///    whole directory. Falling through to a compile here would silently diff
///    the current source against itself and always report `identical`
///    (ADR-0008 decision 14: `ridl diff` reads the workspace-local baseline);
/// 3. anything else — a `.typl`/`.ridl` file, a package directory, or a
///    workspace root — compiled in process through `ridlc::compile_workspace`.
///    A directory with no `.ir.json` in it is source, and takes this path.
///
/// A read, parse, or compile error renders to stderr and yields exit code 2 —
/// `ridl diff` never emits a diff report over a snapshot it could not build.
fn load_diff_side(entry: &Path) -> Result<Vec<ridl_ir::v2::Package>, ExitCode> {
    if is_ir_json(entry) {
        return load_snapshots(&[entry.to_path_buf()]);
    }

    if entry.is_dir() {
        let snapshots = snapshot_files(entry)?;
        if !snapshots.is_empty() {
            return load_snapshots(&snapshots);
        }
    }

    let mut db = ridl_core::RidlDatabase::default();
    match ridlc::compile_workspace(&mut db, entry) {
        Ok(output) => {
            if output
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.severity == ridl_core::diag::Severity::Error)
            {
                eprint!("{}", render(&output.diagnostics, &output.sources));
                return Err(ExitCode::from(2));
            }
            Ok(output
                .checked
                .into_iter()
                .map(|checked| checked.ir)
                .collect())
        }
        Err(err) => {
            eprintln!("error: {}: {err}", entry.display());
            Err(ExitCode::from(2))
        }
    }
}

/// Whether `path` is an `.ir.json` snapshot (a file whose name ends `.ir.json`)
/// rather than a source input.
fn is_ir_json(path: &Path) -> bool {
    path.is_file()
        && path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.ends_with(".ir.json"))
}

// ==========================================================================
// The baseline-aware desk check (E2.9, general form §6.3, ADR-0008 decision 9)
// ==========================================================================

/// The change categories the desk check reports: the four that move a live
/// interaction's ordinal, and no others.
///
/// General form §6.3 asks for one thing at the desk — a reorder or an insertion
/// caught before CI, because declaration order is wire identity and a reorder
/// looks like tidying. The other breaking categories (a payload type change, a
/// narrowed constraint, a timing change) are already loud in review and stay
/// `ridl diff`'s job in CI: this is the §6.3 mitigation, not a second diff gate.
///
/// All four classify [`Breaking`](ridl_diff::Verdict::Breaking) in every
/// direction, so the category alone selects them.
const ORDINAL_CATEGORIES: [ridl_diff::Category; 4] = [
    ridl_diff::Category::InteractionInserted,
    ridl_diff::Category::InteractionReordered,
    ridl_diff::Category::InteractionRemoved,
    ridl_diff::Category::ReservedNameRedeclared,
];

/// Runs `check` and, when the compile is clean and a baseline is available,
/// the desk check on top of it.
///
/// The desk check only ever *adds warnings*: `ridl check` keeps its 0/1/2 exit
/// contract, so a reordered but otherwise clean workspace still exits 0. It is
/// also skipped entirely when the compile produced an error — a diff against
/// IR that failed to check would report noise on top of the real problem.
fn run_check(path: &Path, frozen: bool, baseline: Option<&Path>) -> ExitCode {
    let mut run = match ridlc::run_check(path, frozen.into()) {
        Ok(run) => run,
        Err(err) => {
            eprintln!("error: {err}");
            return ExitCode::from(2);
        }
    };

    if !run.has_error() {
        match baseline_location(path, baseline) {
            Ok(Some(location)) => {
                if let Err(code) = desk_check(path, &location, &mut run) {
                    return code;
                }
            }
            Ok(None) => {}
            Err(code) => return code,
        }
    }

    finish(Ok(run))
}

/// Publishes the workspace at `path` as a baseline.
///
/// The compile and the write are `ridlc`'s own `build --emit ir-json`, so the
/// snapshot a desk compares against is byte for byte the snapshot CI compares
/// against. One `.ir.json` holds exactly one package, so an N-package workspace
/// writes N files, one per package name; `ridl check` matches them back up by
/// the package name inside each file, never by file name.
///
/// The baseline is regenerated **wholesale**: the published directory ends up
/// holding exactly the packages the workspace declares now, so renaming a
/// package leaves no snapshot behind under the old name. Publishing goes
/// through a staging directory to get that without risking the opposite
/// failure — clearing the directory up front would destroy a good baseline
/// whenever the workspace happens not to compile.
fn run_baseline(path: &Path, out: Option<&Path>) -> ExitCode {
    let out_dir = out
        .map(Path::to_path_buf)
        .unwrap_or_else(|| default_baseline_dir(path));
    let staging = staging_dir(&out_dir);
    let _ = std::fs::remove_dir_all(&staging);

    let run = match ridlc::run_build(path, &staging, &[Emit::IrJson], false.into()) {
        Ok(run) => run,
        Err(err) => {
            let _ = std::fs::remove_dir_all(&staging);
            eprintln!("error: {err}");
            return ExitCode::from(2);
        }
    };

    // An error-bearing run wrote nothing (`ridlc` gates every emit on a clean
    // compile), so there is nothing to publish and the existing baseline stays
    // exactly as it was.
    if run.has_error() {
        let _ = std::fs::remove_dir_all(&staging);
        return finish(Ok(run));
    }

    if let Err(err) = publish_baseline(&staging, &out_dir) {
        let _ = std::fs::remove_dir_all(&staging);
        eprintln!(
            "error: cannot publish the baseline to {}: {err}",
            out_dir.display()
        );
        return ExitCode::from(2);
    }

    finish(Ok(run))
}

/// The directory the snapshots are built into before they are published: a
/// hidden sibling of `out_dir`, so the move into place is a rename within one
/// filesystem.
fn staging_dir(out_dir: &Path) -> PathBuf {
    let name = out_dir
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("baseline");
    out_dir
        .parent()
        .unwrap_or(Path::new("."))
        .join(format!(".{name}.staging"))
}

/// Replaces the `.ir.json` set in `out_dir` with the freshly built one in
/// `staging`, dropping any snapshot whose package the workspace no longer
/// declares. Only `.ir.json` files are touched: `out_dir` may be a directory a
/// user pointed `--out` at, and nothing else in it is this command's to delete.
fn publish_baseline(staging: &Path, out_dir: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(out_dir)?;
    for stale in ir_json_files(out_dir)? {
        std::fs::remove_file(stale)?;
    }
    for fresh in ir_json_files(staging)? {
        let name = fresh
            .file_name()
            .expect("a listed snapshot path has a file name");
        std::fs::rename(&fresh, out_dir.join(name))?;
    }
    std::fs::remove_dir_all(staging)
}

/// Where to read the baseline from, if anywhere.
///
/// An explicit `--baseline` that does not exist is an input error (exit 2) —
/// asking for a baseline that is not there is a mistake worth hearing about.
/// Auto-discovery is the silent path: with no flag and no `.ridl/baseline/`
/// directory, `ridl check` behaves exactly as it did before this command
/// existed.
fn baseline_location(entry: &Path, flag: Option<&Path>) -> Result<Option<PathBuf>, ExitCode> {
    match flag {
        Some(explicit) if explicit.exists() => Ok(Some(explicit.to_path_buf())),
        Some(explicit) => {
            eprintln!(
                "error: the baseline `{}` does not exist",
                explicit.display()
            );
            Err(ExitCode::from(2))
        }
        None => {
            let default = default_baseline_dir(entry);
            Ok(default.is_dir().then_some(default))
        }
    }
}

/// `.ridl/baseline/` at the workspace root (ADR-0008 decision 14). The root is
/// the nearest directory at or above `entry` holding a `ridl.toml` — the same
/// root the compile scopes itself to — falling back to `entry`'s own directory
/// when there is no manifest anywhere above it (single-file mode).
fn default_baseline_dir(entry: &Path) -> PathBuf {
    let start = if entry.is_file() {
        entry.parent().unwrap_or(Path::new(".")).to_path_buf()
    } else {
        entry.to_path_buf()
    };
    let mut cursor = start.as_path();
    loop {
        if cursor.join("ridl.toml").is_file() {
            return cursor.join(".ridl").join("baseline");
        }
        match cursor.parent() {
            Some(parent) => cursor = parent,
            None => break,
        }
    }
    start.join(".ridl").join("baseline")
}

/// Compares the checked workspace against the baseline at `location` and
/// appends a RIDL-407 warning for every ordinal-affecting change.
///
/// The workspace is compiled a second time here, through
/// [`ridlc::compile_workspace`], because `run_check` renders diagnostics but
/// does not hand back the IR. The cost is paid only when a baseline is actually
/// present, and never on a run that already failed.
fn desk_check(entry: &Path, location: &Path, run: &mut CliRun) -> Result<(), ExitCode> {
    let baseline = load_baseline(location)?;
    if baseline.is_empty() {
        return Ok(());
    }

    let mut db = ridl_core::RidlDatabase::default();
    let current: Vec<ridl_ir::v2::Package> = match ridlc::compile_workspace(&mut db, entry) {
        Ok(output) => output
            .checked
            .into_iter()
            .map(|checked| checked.ir)
            .collect(),
        Err(err) => {
            eprintln!("error: {}: {err}", entry.display());
            return Err(ExitCode::from(2));
        }
    };

    let report = ridl_diff::diff_sets(&baseline, &current);
    let index = DeclIndex::build(entry);
    let mut warnings = Vec::new();
    for change in &report.changes {
        if !ORDINAL_CATEGORIES.contains(&change.category) {
            continue;
        }
        warnings.push(Diagnostic {
            code: DiagCode::RIDL_407,
            severity: Severity::Warning,
            message: drift_message(change),
            primary: index.span_of(&change.path, &mut run.sources),
            labels: Vec::new(),
            fixits: Vec::new(),
        });
    }
    run.diagnostics.extend(warnings);
    Ok(())
}

/// The RIDL-407 message for one ordinal-affecting change.
///
/// Written for the reader of a `.ridl` file, not for a reader of the diff
/// report. It names the interaction and the shape it is declared in — the words
/// in the source — rather than the slash-separated diff path, states the one
/// consequence that makes the warning worth reading (declaration order is the
/// wire identity, ridl §11), and names the edit that keeps the baseline intact.
/// It used to read `interaction ordinal changed against the baseline:
/// fx.audit/Motion/reset (interaction_reordered)`: "ordinal" is an IR word, the
/// path is a diff-report word, `interaction_reordered` is the enum variant's
/// own spelling, and between them they stated neither consequence nor remedy.
fn drift_message(change: &ridl_diff::Change) -> String {
    let (shape, name) = shape_and_name(&change.path);
    // "in `Motion`" when the shape is known, dropped when the path is not the
    // three-segment form every ordinal category emits.
    let in_shape = shape.map_or(String::new(), |shape| format!(" in `{shape}`"));
    match change.category {
        ridl_diff::Category::InteractionReordered => format!(
            "`{name}` has moved{in_shape} since the published baseline{}. Declaration order is \
             the wire identity of an interaction (ridl §11), so a consumer built against the \
             baseline would now bind this slot to a different interaction — put the declarations \
             back in the baseline's order and add new ones at the end",
            baseline_position(change),
        ),
        ridl_diff::Category::InteractionInserted => format!(
            "`{name}` is declared{in_shape} ahead of interactions the published baseline already \
             numbers. An interaction inserted above an existing one shifts every later wire \
             identity (ridl §11) — declare it at the end of the body instead",
        ),
        ridl_diff::Category::InteractionRemoved => format!(
            "`{name}` is gone{in_shape} but the published baseline still declares it. Deleting \
             the line frees its slot and every later interaction slides into a wire identity \
             that is not its own (ridl §11) — retire it in place with `reserved {name}`, which \
             holds the slot for ever",
        ),
        ridl_diff::Category::ReservedNameRedeclared => format!(
            "`{name}` is declared again{in_shape}, and the published baseline retires that name \
             with `reserved`. A retired name is a permanent wire reservation (ridl §11) — a \
             consumer still holding the old contract would read the new interaction as the \
             retired one, so give this interaction a different name",
        ),
        // `ORDINAL_CATEGORIES` is the caller's filter and holds exactly the four
        // arms above. A fifth category reaching here would be a filter that
        // grew without its messages, so this says only what it can defend — and
        // says it without the raw category token, which is the vocabulary this
        // code exists to keep out of the message.
        _ => format!(
            "`{name}`{in_shape} changed against the published baseline in a way that moves an \
             interaction's wire identity (ridl §11)"
        ),
    }
}

/// The shape and interaction name of a `<package>/<shape>/<interaction>` diff
/// path. A path of any other arity yields no shape and its last segment as the
/// name, so the message degrades to naming what it can rather than printing the
/// raw path.
fn shape_and_name(path: &str) -> (Option<&str>, &str) {
    let parts: Vec<&str> = path.split('/').collect();
    match parts.as_slice() {
        [_package, shape, name] => (Some(shape), name),
        _ => (None, parts.last().copied().unwrap_or(path)),
    }
}

/// ` (position 2 there, position 4 here)` for a reorder whose two sides carry
/// two *different* positions, and the empty string otherwise.
///
/// The walk renders a live reorder's sides as bare ordinals (`"2"`) and a
/// tombstone's as `"reserved at ordinal 2"`, so the trailing integer is what
/// the two spellings share.
///
/// The equal case is dropped rather than printed. A reorder is detected on
/// *relative* order among the survivors, so an interaction can change rank
/// while its absolute ordinal stays put — an insertion above it shifts the
/// others past it — and "`doorClosed` has moved (position 3 there, position 3
/// here)" contradicts itself in the same breath. The sentence about relative
/// order stands on its own; the numbers are a convenience that only helps when
/// they differ.
fn baseline_position(change: &ridl_diff::Change) -> String {
    let position = |side: &Option<String>| -> Option<u32> {
        side.as_ref()?
            .rsplit(' ')
            .next()?
            .parse()
            .ok()
            .filter(|slot| *slot > 0)
    };
    match (position(&change.before), position(&change.after)) {
        (Some(was), Some(now)) if was != now => {
            format!(" (position {was} there, position {now} here)")
        }
        _ => String::new(),
    }
}

/// Loads the baseline packages: every `.ir.json` in a directory, in file-name
/// order, or the single file `location` names.
fn load_baseline(location: &Path) -> Result<Vec<ridl_ir::v2::Package>, ExitCode> {
    let files = if location.is_dir() {
        snapshot_files(location)?
    } else {
        vec![location.to_path_buf()]
    };
    load_snapshots(&files)
}

/// The `.ir.json` snapshots directly inside `dir`, in file-name order.
fn ir_json_files(dir: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)?
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| is_ir_json(path))
        .collect();
    files.sort();
    Ok(files)
}

/// [`ir_json_files`] with an unreadable directory turned into exit 2 — a
/// comparison against a directory that cannot be listed must not quietly become
/// a comparison against nothing.
fn snapshot_files(dir: &Path) -> Result<Vec<PathBuf>, ExitCode> {
    ir_json_files(dir).map_err(|err| {
        eprintln!(
            "error: cannot read the snapshot directory {}: {err}",
            dir.display()
        );
        ExitCode::from(2)
    })
}

/// Deserializes every snapshot in `files`. One that cannot be read or parsed is
/// exit 2 — a comparison against half a baseline would be a lie about what is
/// published.
fn load_snapshots(files: &[PathBuf]) -> Result<Vec<ridl_ir::v2::Package>, ExitCode> {
    let mut packages = Vec::new();
    for file in files {
        match ridl_diff::load_ir_json(file) {
            Ok(package) => packages.push(package),
            Err(err) => {
                eprintln!("error: {}: {err}", file.display());
                return Err(ExitCode::from(2));
            }
        }
    }
    Ok(packages)
}

/// Where every interaction and every interface shape of the current source tree
/// is declared, so a diff path can be pointed back at the code on the desk.
///
/// The diff engine reads only the IR, which carries no source locations, so the
/// span comes from a separate parse of the same tree. Matching is by name —
/// package, shape, interaction — which is exactly the identity the diff path
/// carries. "Shape" is an `interface` declaration or a service's inline body
/// (ridl §14.0, §14.5); the two are indexed together through
/// `SourceFile::shapes`, because a diff path names either one the same way.
#[derive(Default)]
struct DeclIndex {
    /// The text of each indexed file, by path: the renderer needs the text as
    /// well as the path to draw a snippet.
    texts: BTreeMap<String, String>,
    /// `(package, shape, interaction)` to the interaction's declaration.
    members: BTreeMap<(String, String, String), (String, TextRange)>,
    /// `(package, shape)` to the shape's declared name. This is the fallback
    /// for a removed interaction, whose own declaration no longer exists in the
    /// source being checked. A service's inline shape is keyed by the service's
    /// dotted name, exactly as its diff paths are.
    shapes: BTreeMap<(String, String), (String, TextRange)>,
}

impl DeclIndex {
    /// Indexes every `.typl` and `.ridl` file under `entry`. A file or
    /// directory that cannot be read is skipped rather than reported: the
    /// compile already ran clean over this tree, so anything unreadable here
    /// is not the desk check's business.
    fn build(entry: &Path) -> Self {
        let mut index = Self::default();
        for file in collect_source_files(entry).unwrap_or_default() {
            let Ok(text) = std::fs::read_to_string(&file) else {
                continue;
            };
            let path = file.to_string_lossy().into_owned();
            let parse = ridl_syntax::parse(&text, ridl_core::profile_of_path(&path));
            let Some(source) = SourceFile::cast(parse.syntax()) else {
                continue;
            };
            let Some(package) = package_name(&source) else {
                continue;
            };

            // Every interface shape, `interface` declarations and services'
            // inline shapes alike (`SourceFile::shapes`). A service's inline
            // shape is an interface body in every way that matters to wire
            // identity, and the diff paths it produces are keyed by the service
            // name — so it earns a fallback entry exactly as a named interface
            // does, or a removal from it renders with no span at all.
            for shape in source.shapes() {
                let Some(name) = shape.identity() else {
                    continue;
                };
                let Some(range) = shape.identity_range() else {
                    continue;
                };
                index
                    .shapes
                    .insert((package.clone(), name.clone()), (path.clone(), range));
                index.record_members(&package, &name, &path, &text, shape.members());
            }

            index.texts.insert(path, text);
        }
        index
    }

    /// Records one interface body's interactions.
    fn record_members(
        &mut self,
        package: &str,
        shape: &str,
        path: &str,
        text: &str,
        members: impl Iterator<Item = InterfaceMember>,
    ) {
        for member in members {
            let Some(name) = member.name() else { continue };
            let Some(member_name) = name_text(&name) else {
                continue;
            };
            self.members.insert(
                (package.to_string(), shape.to_string(), member_name),
                (path.to_string(), declaration_range(&member, text)),
            );
        }
    }

    /// The span a `<package>/<shape>/<interaction>` diff path points at: the
    /// interaction's declaration, the shape's name when the interaction itself
    /// is gone (a removal), and a detached span when neither is in the source —
    /// a detached diagnostic renders as the coded message alone.
    fn span_of(&self, diff_path: &str, sources: &mut SourceMap) -> Span {
        let mut parts = diff_path.split('/');
        let (Some(package), Some(shape), Some(member)) = (parts.next(), parts.next(), parts.next())
        else {
            return detached_span();
        };

        let key = (package.to_string(), shape.to_string(), member.to_string());
        let found = self
            .members
            .get(&key)
            .or_else(|| self.shapes.get(&(key.0, key.1)));
        let Some((path, range)) = found else {
            return detached_span();
        };
        let Some(text) = self.texts.get(path) else {
            return detached_span();
        };
        Span {
            file: sources.file_id(path, text),
            range: *range,
        }
    }
}

/// A span pointing at no file at all.
fn detached_span() -> Span {
    Span {
        file: FileId::DETACHED,
        range: TextRange::empty(TextSize::new(0)),
    }
}

/// The declaration's own range, with trailing whitespace trimmed off: a node's
/// range can run to the start of the next line, and an underline that reaches
/// past the declaration reads as if the next one were implicated too.
fn declaration_range(member: &InterfaceMember, text: &str) -> TextRange {
    let range = member.syntax().text_range();
    let start = usize::from(range.start());
    let end = usize::from(range.end()).min(text.len());
    let trimmed = text
        .get(start..end)
        .map(|slice| slice.trim_end().len())
        .unwrap_or(0);
    TextRange::at(range.start(), TextSize::new(trimmed as u32))
}

/// The package a source file declares.
fn package_name(source: &SourceFile) -> Option<String> {
    dotted_text(source.package_decl()?.qualified_name()?.syntax())
}

/// The identifier a `Name` node carries.
fn name_text(name: &Name) -> Option<String> {
    Some(name.ident_token()?.text().to_string())
}

/// The dotted text of a qualified or dotted name node — its non-trivia tokens
/// joined, e.g. `veh.cluster`.
fn dotted_text(node: &ridl_syntax::SyntaxNode) -> Option<String> {
    let text: String = node
        .children_with_tokens()
        .filter_map(|element| element.into_token())
        .filter(|token| !token.kind().is_trivia())
        .map(|token| token.text().to_string())
        .collect();
    (!text.is_empty()).then_some(text)
}

/// Renders a check/build run's diagnostics to stderr and turns the outcome into
/// an exit code: 2 on an I/O error, 1 when any diagnostic is an error, 0
/// otherwise.
fn finish(run: std::io::Result<CliRun>) -> ExitCode {
    match run {
        Ok(run) => {
            eprint!("{}", render(&run.diagnostics, &run.sources));
            if run.has_error() {
                ExitCode::FAILURE
            } else {
                ExitCode::SUCCESS
            }
        }
        Err(err) => {
            eprintln!("error: {err}");
            ExitCode::from(2)
        }
    }
}

/// Formats every `.typl` and `.ridl` file under `path`, each parsed under the
/// profile its extension selects.
///
/// A file with parse errors is never rewritten (a formatter must not eat broken
/// code); its diagnostics render to stderr and the run exits 1. In `--check`
/// mode nothing is written and a file that would change also exits 1.
fn run_fmt(path: &Path, check: bool) -> ExitCode {
    let mut sources = SourceMap::new();
    let mut diagnostics = Vec::new();
    let mut any_would_change = false;
    let mut any_broken = false;

    let files = match collect_source_files(path) {
        Ok(files) => files,
        Err((dir, err)) => {
            eprintln!("error: cannot read {}: {err}", dir.display());
            return ExitCode::from(2);
        }
    };

    for file in files {
        let text = match std::fs::read_to_string(&file) {
            Ok(text) => text,
            Err(err) => {
                eprintln!("error: cannot read {}: {err}", file.display());
                return ExitCode::from(2);
            }
        };
        let profile = ridl_core::profile_of_path(&file.to_string_lossy());
        match format(&text, profile) {
            FormatOutcome::Formatted(formatted) => {
                if formatted != text {
                    any_would_change = true;
                    if !check && let Err(err) = std::fs::write(&file, &formatted) {
                        eprintln!("error: cannot write {}: {err}", file.display());
                        return ExitCode::from(2);
                    }
                }
            }
            FormatOutcome::ParseErrors(errors) => {
                any_broken = true;
                let file_id = sources.file_id(&file.to_string_lossy(), &text);
                for error in &errors {
                    diagnostics.push(ridlc::syntax_error_diagnostic(error, file_id));
                }
            }
        }
    }

    eprint!("{}", render(&diagnostics, &sources));
    if any_broken || (check && any_would_change) {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

/// Every `.typl` and `.ridl` file under `path`: `path` itself when it is a
/// file, otherwise a recursive walk that skips hidden directories.
///
/// A directory the walk cannot read — `path` itself, when it does not exist
/// or is not readable, or a subdirectory the walk descends into — is an error
/// rather than zero files: `Err` carries the directory `read_dir` failed on
/// and the underlying `io::Error`. The walk cannot tell "empty" from
/// "absent" or "unreadable" any other way, and treating those as zero files
/// is what let `ridl fmt` report success over a tree it never read.
fn collect_source_files(path: &Path) -> Result<Vec<PathBuf>, (PathBuf, std::io::Error)> {
    if path.is_file() {
        return Ok(vec![path.to_path_buf()]);
    }
    let mut files = Vec::new();
    let mut stack = vec![path.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = std::fs::read_dir(&dir).map_err(|err| (dir.clone(), err))?;
        for entry in entries.flatten() {
            let child = entry.path();
            if child.is_dir() {
                let hidden = child
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with('.'));
                if !hidden {
                    stack.push(child);
                }
            } else if child
                .extension()
                .is_some_and(|ext| ext == "typl" || ext == "ridl")
            {
                files.push(child);
            }
        }
    }
    files.sort();
    Ok(files)
}
