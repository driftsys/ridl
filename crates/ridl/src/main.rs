//! The `ridl` toolchain facade — the porcelain layer (concept note §8.1). The cargo/deno-style front door with humane
//! defaults: `PATH` defaults to the current directory.
//!
//! `ridl check` and `ridl build` delegate to the `ridlc` library face;
//! `ridl fmt` formats `.typl`, `.ridl` and `.rsdl` files, resolving the width
//! from `.editorconfig` per file. The exit
//! code is 0 clean, 1 on a diagnostic error (or, for `fmt --check`, a file that
//! would change), and 2 on an input/output or usage error.
//!
//! `ridl diff` compares two IR snapshots or source trees through the
//! `ridl-diff` engine. It carries its own exit contract — 0 compatible
//! or identical, 1 breaking, 2 error (concept note §9.1, ADR-0008 decision 9) —
//! and never touches `ridlc`'s source→IR boundary beyond compiling each side.
//!
//! `ridl test` runs the property suite over a workspace: the range
//! self-corpora derived from the range generators, and satisfiability sampling
//! of every `require` clause. It carries the same 0/1/2 exit contract, with 1
//! reserved for a self-corpus failure or an evaluation error.
//!
//! `ridl baseline` and `ridl check --baseline` are the desk-time half of that
//! engine (general form §6.3): `baseline` publishes one `.ir.json`
//! snapshot per package, and `check` compares the workspace against those
//! snapshots and warns (RIDL-407) when the ordinal of an interaction, a struct
//! field or a union arm moved. Both live here rather than in
//! `ridlc` because reading a workspace-local baseline is not part of the
//! source→IR function the tool qualification argument covers (ADR-0008
//! decision 9).
//!
//! `ridl lsp` and `ridl mcp` are the two stdio servers this one binary hosts:
//! the language server an editor drives (`ridl-lsp`) and the Model Context
//! Protocol server an agent drives (`ridl-mcp`). Both delegate every behavior
//! to their library and only wire the transport here, so one installed binary
//! serves the editor, the agent, and the command line.
//!
//! `ridl lock` writes a unit's `interfaces.lock` (lock design §5): plain, it
//! allocates a number to every interface that has none; with `--rename` or
//! `--retire`, it rewrites one unit's entries in place. It lives here
//! beside `ridl baseline` because it reads and writes a file in the workspace
//! that is not a source (`ridlc` gains no `lock` subcommand); the compile it
//! runs first is `ridlc`'s own.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

mod catalogs;
mod lock;
mod property;

use clap::{Parser, Subcommand};
use ridl_core::diag::{DiagCode, Diagnostic, FileId, Label, Severity, SourceMap, Span, render};
use ridl_core::interface_lock::LockKey;
use ridl_core::lint::{apply_lint_levels, lint_of};
use ridl_fmt::{FormatOptions, FormatOutcome, format};
use ridl_syntax::ast::{AstNode as _, HasName as _, InterfaceMember, Name, SourceFile};
use ridlc::diff_side::{
    first_nested_snapshot_dir, first_non_json_ir_in, ir_json_files, is_non_json_ir, is_source_dir,
    snapshot_files,
};
use ridlc::plugin::PluginSpec;
use ridlc::{ApplyLints, CliRun, Emit};
use rowan::{TextRange, TextSize};

#[derive(Parser)]
#[command(
    name = "ridl",
    about = "The RIDL toolchain",
    version = env!("RIDL_BUILD_VERSION")
)]
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
        /// (RIDL-407) on every interaction whose ordinal moved and every
        /// struct field or union arm change `ridl diff` gates on that
        /// concerns an ordinal: a member inserted, one removed, one moved in
        /// an edit that added or removed no member, and one appended beside
        /// such a change or to a result union. An append that is breaking
        /// only for its field's type moves no ordinal and draws no warning.
        /// Without the flag, `.ridl/baseline/` at the workspace root is used
        /// when it exists.
        #[arg(long, value_name = "DIR|FILE")]
        baseline: Option<PathBuf>,
        /// Output format for the report: text renders to stderr (the
        /// default); json goes to stdout instead — see the CLI reference
        /// (docs/book/cli-reference.md) for its schema; sarif writes one
        /// SARIF 2.1.0 log to stdout, for code-scanning viewers.
        #[arg(long, value_enum, default_value_t = CheckFormat::Text)]
        format: CheckFormat,
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
        /// A codegen plugin to run beside the emits, once per package:
        /// `<LANGUAGE>` runs `ridlc-gen-<LANGUAGE>` from PATH,
        /// `<LANGUAGE>=<PATH>` runs the executable at PATH. Repeatable.
        #[arg(long, value_name = "LANGUAGE[=PATH]")]
        plugin: Vec<PluginSpec>,
        /// Seconds a plugin may run per package before it is killed.
        #[arg(long, value_name = "SECONDS", default_value_t = ridlc::plugin::DEFAULT_TIMEOUT_SECONDS)]
        plugin_timeout: u64,
        /// Verify remote imports against `ridl.lock` without fetching or
        /// regenerating it (CI mode, ADR-0002 §7).
        #[arg(long)]
        frozen: bool,
        /// The deployment to carry in each codegen request. With one
        /// deployment in the workspace it is selected without this flag; with
        /// several, none is carried unless named.
        #[arg(long, value_name = "NAME")]
        deployment: Option<String>,
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
    /// Reformat `.typl`, `.ridl` and `.rsdl` files in place (defaults to the
    /// current directory).
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
    /// Allocate a number to every interface that has none and write each
    /// unit's `interfaces.lock`; with `--rename` or `--retire`, rewrite one
    /// unit's entries in place instead. Exit 0 when the file is written or
    /// nothing changes, 1 on a diagnostic error, 2 on a bad flag or a path or
    /// I/O failure. `ridl lock merge` is the git merge driver for the file.
    #[command(args_conflicts_with_subcommands = true)]
    Lock {
        /// A unit directory, a workspace root, or a file. A directory
        /// named `merge` is spelled `./merge`, since the bare word is the
        /// subcommand.
        #[arg(default_value = ".")]
        path: PathBuf,
        /// Rewrite the live entry OLD to hold the key NEW, keeping its number
        /// (repeatable). NEW must be a declaration without an entry.
        #[arg(long, value_name = "OLD=NEW")]
        rename: Vec<String>,
        /// Mark the live entry NAME retired, keeping its line and its number
        /// (repeatable). NAME must no longer be declared.
        #[arg(long, value_name = "NAME")]
        retire: Vec<String>,
        #[command(subcommand)]
        sub: Option<LockCommand>,
    },
    /// Run the language server over stdio: exit 0 on a clean shutdown, 2 on a
    /// transport error. Editors spawn this. Stdio is the only transport.
    Lsp {
        /// Select the stdio transport. Accepted because editor clients pass
        /// it, as the LSP specification recommends; omitting it changes
        /// nothing.
        #[arg(long)]
        stdio: bool,
        /// The editor's process id, which the LSP specification recommends a
        /// server accept. Ignored: the `initialize` request carries it too.
        #[arg(long = "clientProcessId", value_name = "PID")]
        client_process_id: Option<u32>,
    },
    /// Run the MCP server over stdio for an agent host: exit 0 on a clean
    /// shutdown, 2 on a transport error. It takes no flag of its own.
    Mcp,
    /// Print a catalog descriptor as strict JSON, after verifying it.
    Describe {
        /// The `<unit>.catalog.binfb` file `ridl build --emit catalog` wrote.
        path: PathBuf,
    },
}

/// The subcommands of `ridl lock`.
#[derive(Subcommand)]
enum LockCommand {
    /// The git merge driver for `interfaces.lock`: a three-way merge over
    /// entries matched by number, written to OURS. Exit 0 when the merge is
    /// clean, 1 when entries disagree (they are left between conflict markers
    /// of MARKER_SIZE, and the file is RIDL-410 until resolved), 2 when an
    /// input cannot be read or does not parse (OURS is left as it was).
    /// Register it with `.gitattributes` and `git config` as the CLI
    /// reference documents.
    Merge {
        /// The common ancestor's file (`%O`); an empty file reads as `next 1`.
        base: PathBuf,
        /// The current branch's file (`%A`); the result is written here.
        ours: PathBuf,
        /// The other branch's file (`%B`).
        theirs: PathBuf,
        /// The length of a conflict marker line (`%L`, 7 by default).
        #[arg(value_parser = clap::value_parser!(u16).range(1..))]
        marker_size: u16,
    },
}

/// The `ridl diff` output format — human-readable text or machine-readable
/// JSON with a stable schema.
#[derive(Clone, Copy, Debug, clap::ValueEnum)]
enum DiffFormat {
    Text,
    Json,
}

/// The `ridl check` output format.
#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
enum CheckFormat {
    Text,
    Json,
    Sarif,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli.command {
        Command::Check {
            path,
            frozen,
            baseline,
            format,
        } => run_check(&path, frozen, baseline.as_deref(), format),
        Command::Baseline { path, out } => run_baseline(&path, out.as_deref()),
        Command::Build {
            path,
            out_dir,
            emit,
            plugin,
            plugin_timeout,
            frozen,
            deployment,
        } => finish(ridlc::run_build_with(
            &path,
            &out_dir,
            &emit,
            &plugin,
            std::time::Duration::from_secs(plugin_timeout),
            frozen.into(),
            ApplyLints::Yes,
            deployment.as_deref(),
        )),
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
        Command::Lock {
            sub:
                Some(LockCommand::Merge {
                    base,
                    ours,
                    theirs,
                    marker_size,
                }),
            ..
        } => lock::run_lock_merge(&base, &ours, &theirs, usize::from(marker_size)),
        Command::Lock {
            path,
            rename,
            retire,
            sub: None,
        } => lock::run_lock(&path, &rename, &retire),
        Command::Lsp { .. } => run_lsp(),
        Command::Mcp => run_mcp(),
        Command::Describe { path } => run_describe(&path),
    }
}

/// `ridl describe`: read, verify (identifier, version, whole-buffer walk),
/// render. Every failure means the tool could not answer: exit 2 with the
/// cause named (ADR-0010 decision 1;
/// `docs/archive/2026-09-13-runtime-descriptors-design.md`, D-8).
fn run_describe(path: &Path) -> ExitCode {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(err) => {
            eprintln!("error: {}: {err}", path.display());
            return ExitCode::from(2);
        }
    };
    let catalog = match ridl_descriptor::verify(&bytes) {
        Ok(catalog) => catalog,
        Err(err) => {
            eprintln!("error: {}: {err}", path.display());
            return ExitCode::from(2);
        }
    };
    match ridl_descriptor::describe::to_json(catalog) {
        Ok(json) => {
            use std::io::Write as _;
            let text = serde_json::to_string_pretty(&json).expect("a JSON value serializes");
            let mut stdout = std::io::stdout().lock();
            match writeln!(stdout, "{text}").and_then(|()| stdout.flush()) {
                Ok(()) => ExitCode::SUCCESS,
                Err(err) => {
                    // A pipe whose reader has gone (EPIPE) is a write I/O
                    // failure: exit 2. A closed descriptor (`1>&-`) does not
                    // reach this branch: std treats it as a sink and the exit
                    // is 0. stderr can be closed too, and that failure is
                    // ignored.
                    let _ = writeln!(std::io::stderr(), "error: {}: {err}", path.display());
                    ExitCode::from(2)
                }
            }
        }
        Err(err) => {
            eprintln!(
                "error: {}: catalog descriptor is malformed: {err}",
                path.display()
            );
            ExitCode::from(2)
        }
    }
}

/// `ridl lsp`: the language server over stdio. Every behavior lives in
/// `ridl-lsp`; this wires the transport and maps the outcome onto the exit
/// codes of ADR-0010 decision 1 — 0 when the client shut the server down, 2
/// when the transport failed or ended before the handshake, which is the tool
/// being unable to answer rather than a negative answer.
fn run_lsp() -> ExitCode {
    let (connection, io_threads) = lsp_server::Connection::stdio();
    if let Err(err) =
        ridl_lsp::server::run_with_version(connection, Some(env!("RIDL_BUILD_VERSION")))
    {
        eprintln!("error: {err}");
        return ExitCode::from(2);
    }
    if let Err(err) = io_threads.join() {
        eprintln!("error: {err}");
        return ExitCode::from(2);
    }
    ExitCode::SUCCESS
}

/// `ridl mcp`: the Model Context Protocol server over stdio. `rmcp` is async,
/// so this builds the only Tokio runtime the binary ever has — no other
/// subcommand is async, and none pays for this one.
fn run_mcp() -> ExitCode {
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(err) => {
            eprintln!("error: {err}");
            return ExitCode::from(2);
        }
    };
    match runtime.block_on(ridl_mcp::serve_stdio_with_version(Some(env!(
        "RIDL_BUILD_VERSION"
    )))) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("error: {err}");
            ExitCode::from(2)
        }
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
    let mut db = ridl_core::RidlDatabase::default();
    let old_side = match ridlc::load_diff_side(&mut db, old, &[]) {
        Ok(side) => side,
        Err(error) => return report_diff_side_error(error),
    };
    let new_side = match ridlc::load_diff_side(&mut db, new, &[]) {
        Ok(side) => side,
        Err(error) => return report_diff_side_error(error),
    };

    // The verdict is the contracts' alone: the system's placement and
    // composition changes are listed under their headings with no verdict
    // (rsdl reference §14).
    // No snapshot carries `ridl.std`, so it is passed as context: a struct
    // field appended with a `ridl.std` type is then judged by that type's
    // declaration rather than reported breaking as unresolved
    // (driftsys/ridl#598).
    let report = ridl_diff::diff_workspaces(
        &old_side.packages,
        old_side.system.as_ref(),
        &new_side.packages,
        new_side.system.as_ref(),
        &[ridlc::std_ir()],
    );
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

fn report_diff_side_error(error: ridlc::DiffSideError) -> ExitCode {
    match error {
        ridlc::DiffSideError::Compile {
            diagnostics,
            sources,
        } => eprint!("{}", render(&diagnostics, &sources)),
        error => eprintln!("error: {error}"),
    }
    ExitCode::from(2)
}

// ==========================================================================
// The baseline-aware desk check (general form §6.3, ADR-0008 decision 9)
// ==========================================================================

/// The interaction change categories the desk check reports: the four that
/// move a live interaction's ordinal (ridl §11) — and no others. A struct
/// field's or union arm's change is read through [`MEMBER_CATEGORIES`]
/// instead, because its category alone does not say whether an ordinal
/// moved.
///
/// General form §6.3 asks for one thing at the desk — a reorder or an insertion
/// caught before CI, because declaration order is wire identity and a reorder
/// looks like tidying. The other breaking categories (a payload type change, a
/// narrowed constraint, a timing change) are already loud in review and stay
/// `ridl diff`'s job in CI: this is the §6.3 mitigation, not a second diff
/// gate. A service's list is a set (ADR-0015 decision 19 as amended on
/// 2026-09-15): its order is not an identity, so no service-level category
/// belongs here.
///
/// Every category listed here classifies
/// [`Breaking`](ridl_diff::Verdict::Breaking) in every direction, so the
/// category alone selects them.
const ORDINAL_CATEGORIES: [ridl_diff::Category; 4] = [
    ridl_diff::Category::InteractionInserted,
    ridl_diff::Category::InteractionReordered,
    ridl_diff::Category::InteractionRemoved,
    ridl_diff::Category::ReservedNameRedeclared,
];

/// The change categories a struct field or union arm can arrive under
/// (typl §7.4). `ridl_diff`'s composite comparison has no member-level
/// insertion or removal category: a member present on one side only is a
/// `DeclAdded` or `DeclRemoved` under the container's path, and the
/// classifier reads the two bodies to decide its direction — an append with
/// no other change is compatible, except to a result union, whose arms are
/// its transport identity (ADR-0008 decision 4); an insertion, a removal,
/// and an append beside a move or a removal are breaking. So the
/// category does not say whether an ordinal moved; the change's verdict
/// does, and it is the verdict `ridl diff` gates on. `desk_check` warns on
/// one of these exactly when the verdict is
/// [`Breaking`](ridl_diff::Verdict::Breaking) and the container is a struct
/// or union, so the desk and the gate cannot disagree about a member change
/// (driftsys/ridl#533).
///
/// The container check is what keeps an enum value's or enum-set bit's
/// change out: `MemberReordered` also covers their textual reorder, which
/// typl §8 and §9 make *not* a change — the value or bit takes its identity
/// from its explicit number — and `ridl-diff` still reports it,
/// conservatively (driftsys/ridl#397). The desk stays silent for it rather
/// than warn about a wire identity that never moved (driftsys/ridl#335); an
/// enum value added or removed stays `ridl diff`'s alone for the same
/// reason.
const MEMBER_CATEGORIES: [ridl_diff::Category; 3] = [
    ridl_diff::Category::DeclAdded,
    ridl_diff::Category::DeclRemoved,
    ridl_diff::Category::MemberReordered,
];

/// Runs `check` and, when a baseline is available and the compile produced no
/// error other than RIDL-409, the desk check on top of it.
///
/// The desk check only ever *adds* to the diagnostics — RIDL-407 warnings, and
/// the rename label on a RIDL-409 (lock design §4) — so `ridl check` keeps its
/// 0/1/2 exit contract: a reordered but otherwise clean workspace still exits
/// 0, and a workspace with an orphan lock entry still exits 1. It is skipped
/// entirely when the compile produced any other error — a diff against IR that
/// failed to check would report noise on top of the real problem — while
/// RIDL-409 stops nothing in lowering (an entry with no declaration has nothing
/// to lower), so the IR it runs over is whole. A lint raised to `deny` by
/// `[lints]` is an Error for the exit code but not a compile error, so it does
/// not skip the desk check either (see the desk-check text in
/// docs/book/cli-reference.md): the run reports the denied lint and the
/// RIDL-407 together.
fn run_check(path: &Path, frozen: bool, baseline: Option<&Path>, format: CheckFormat) -> ExitCode {
    let mut run = match ridlc::run_check(path, frozen.into()) {
        Ok(run) => run,
        Err(err) => {
            eprintln!("error: {err}");
            return ExitCode::from(2);
        }
    };

    // A lint at `deny` is an Error by level, not a compile error: the IR it
    // runs over is whole, so it does not stop the desk check (see
    // docs/book/cli-reference.md). The lint diagnostics are left out of the
    // gate here, at the `ridl check` call site only; `ridl lock` keeps the
    // unfiltered test.
    let compile_diagnostics = run
        .diagnostics
        .iter()
        .filter(|diagnostic| lint_of(diagnostic.code).is_none());
    if lock::only_lock_orphans(compile_diagnostics) {
        match baseline_location(path, baseline) {
            Ok(Some(location)) => {
                if let Err(code) = desk_check(path, &location, baseline.is_some(), &mut run) {
                    return code;
                }
            }
            Ok(None) => {}
            Err(code) => return code,
        }
    }

    // RIDL-407 is raised here, after `ridlc` applied the levels, so they are
    // applied once more over the whole run. The function is idempotent, so
    // the diagnostics `ridlc` already levelled do not change.
    apply_lint_levels(&mut run.diagnostics, &run.sources, &run.lints);

    finish_check(run, format)
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

    // The snapshot is published with the severities the emit sites chose: a
    // baseline is not a report to a person, so no `[lints]` level applies, and
    // a lint at `deny` does not block the publication (ADR-0024
    // decision 8).
    let mut run = match ridlc::run_build_with(
        path,
        &staging,
        &[Emit::IrJson],
        &[],
        std::time::Duration::from_secs(ridlc::plugin::DEFAULT_TIMEOUT_SECONDS),
        false.into(),
        ApplyLints::No,
        None,
    ) {
        Ok(run) => run,
        Err(err) => {
            let _ = std::fs::remove_dir_all(&staging);
            eprintln!("error: {err}");
            return ExitCode::from(2);
        }
    };

    // An error-bearing run publishes nothing, and the existing baseline stays
    // exactly as it was. The staging directory is discarded rather than left:
    // `ridlc` gates every emit on a clean compile except for an RSDL-7xx error,
    // which writes every artifact and still reports the error (rsdl reference
    // §13), so a staging directory may hold artifacts here.
    if run.has_error() {
        let _ = std::fs::remove_dir_all(&staging);
        return finish(Ok(run));
    }

    // The published baseline is the only record that a removed interaction's
    // ordinal was ever taken. Replacing it with a snapshot that drops the
    // interaction with no `reserved` tombstone destroys that record, and a
    // later append then reuses the ordinal with nothing to compare against.
    // The comparison happens here, against the directory publication is about
    // to overwrite (driftsys/ridl#315). The interface level is the lock's:
    // `interface_refusals` refuses a provisional interface number and a
    // published number the fresh snapshot neither carries nor retires (lock
    // design §8). Both gates run, so one run reports every refusal.
    let mut refused = false;
    for gate in [untombstoned_removals, interface_refusals] {
        match gate(path, &out_dir, &staging, &mut run) {
            Ok(hit) => refused |= hit,
            Err(code) => {
                let _ = std::fs::remove_dir_all(&staging);
                return code;
            }
        }
    }
    if refused {
        let _ = std::fs::remove_dir_all(&staging);
        return finish(Ok(run));
    }

    // The histories are written into the staging directory only once both
    // gates passed, so a refused run leaves every published history as it was.
    let mut db = ridl_core::RidlDatabase::default();
    if let Err(code) = catalogs::write_catalog_histories(&mut db, &staging, &out_dir) {
        let _ = std::fs::remove_dir_all(&staging);
        return code;
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

/// Compares the baseline about to be replaced against the snapshots just built
/// and records a RIDL-408 for every interaction the replacement would drop with
/// no `reserved` tombstone in its own slot, and for every live interaction the
/// replacement declares under a name the baseline retires. Returns whether any
/// was recorded.
///
/// The published snapshots are read flat from `out_dir`, which is exactly where
/// [`publish_baseline`] writes them. This deliberately does not go through
/// [`load_baseline`], whose discovery rules exist to interpret a user-supplied
/// `--baseline` path: inheriting them would let the comparison become a
/// comparison against nothing in the cases driftsys/ridl#235 describes, and a
/// gate that a directory layout can defeat is not a gate.
///
/// Only the interaction level is covered. The interface level is the lock's:
/// an interface's identity is its number in `interfaces.lock`, not a slot in
/// a service's list, so a removed or renumbered interface is refused by the
/// lock's own publication rules, not here. `ReservedNameRedeclared` is an
/// interaction-level category, and it is always refused: the walk emitted
/// the change from a tombstone it read on the published side, so the gate
/// refuses it without a second lookup. An earlier rule refused only when its
/// own lookup of the published container found the tombstone again, and
/// published whenever that lookup did not resolve — a declared interface and
/// an inline service sharing a name (driftsys/ridl#339 case 2), or an
/// interface renamed since publication (found in the review of the fix,
/// driftsys/ridl#528). The published IR is still read
/// for the message's ordinal and wording ([`untombstoned_removal_message`]),
/// never for the verdict.
///
/// The published side is read through [`load_published`], which refuses two
/// snapshots declaring one package (driftsys/ridl#339 case 1).
fn untombstoned_removals(
    entry: &Path,
    out_dir: &Path,
    staging: &Path,
    run: &mut CliRun,
) -> Result<bool, ExitCode> {
    if !out_dir.is_dir() {
        return Ok(false);
    }
    let published = load_published(out_dir)?;
    if published.is_empty() {
        return Ok(false);
    }
    let fresh = load_snapshots(
        &snapshot_files(staging).map_err(report_diff_side_error)?,
        None,
    )?;

    let report = ridl_diff::diff_sets(&published, &fresh);
    // Parsing every source file is wasted work on the common republish that
    // carries no refused change at all, so the index is built only once the
    // first one is actually met.
    let mut index: Option<DeclIndex> = None;
    let mut refusals = Vec::new();
    for change in &report.changes {
        let refused = matches!(
            change.category,
            ridl_diff::Category::InteractionRemoved | ridl_diff::Category::ReservedNameRedeclared
        );
        if !refused {
            continue;
        }
        let index = index.get_or_insert_with(|| DeclIndex::build(entry));
        refusals.push(Diagnostic {
            code: DiagCode::RIDL_408,
            severity: Severity::Error,
            message: untombstoned_removal_message(change, &published),
            primary: index.span_of(&change.path, &mut run.sources),
            labels: Vec::new(),
            fixits: Vec::new(),
        });
    }
    let refused = !refusals.is_empty();
    run.diagnostics.extend(refusals);
    Ok(refused)
}

/// The RIDL-408 message for one refused change, worded for the shape
/// `ridl_diff::walk`'s `diff_interface` emitted it in:
///
/// - a **redeclared name** is the one `ReservedNameRedeclared` shape the gate
///   refuses, told apart by its category;
/// - a **misplaced tombstone** — the source retires the interaction, but not
///   at its own ordinal — is told apart by `change.after`, which only this
///   `InteractionRemoved` shape carries (the tombstone's own ordinal);
/// - a **bare removal** and a **dropped tombstone** both carry no
///   `change.after`, so they are told apart by asking the published IR
///   itself whether it already reserved the name — never by reading the
///   words in `change.before`, which is display text `ridl_diff` owns and may
///   reword.
///
/// The ordinal each message names is read from the published IR too
/// ([`published_ordinal`]), for the same reason. The shape and the name come
/// from the diff path through [`shape_and_name`], as RIDL-407's do, so two
/// interfaces removing the same name draw two distinct messages.
fn untombstoned_removal_message(
    change: &ridl_diff::Change,
    published: &[ridl_ir::v2::Package],
) -> String {
    let (shape, name) = shape_and_name(&change.path);
    let in_shape = shape.map_or(String::new(), |shape| format!(" in `{shape}`"));
    let ordinal = published_ordinal(published, &change.path);
    let held = ordinal.map_or(String::new(), |ordinal| format!(" (ordinal {ordinal})"));
    let slot = ordinal.map_or("that ordinal".to_string(), |ordinal| {
        format!("ordinal {ordinal}")
    });
    if change.category == ridl_diff::Category::ReservedNameRedeclared {
        format!(
            "`{name}` is declared again{in_shape}, but the baseline being replaced retires that \
             name with `reserved`{held}. A tombstone is a permanent reservation (ridl §11): a \
             consumer still holding the old contract would read the new interaction as the \
             retired one. Give the new interaction a different name and keep `reserved {name}` \
             at {slot}."
        )
    } else if change.after.is_some() {
        format!(
            "The source retires `{name}`{in_shape} with a tombstone, but not at the ordinal the \
             interaction held{held}. A tombstone must hold the retired interaction's own ordinal \
             (ridl §11); otherwise the surviving interactions slide into the freed slot. Move \
             `reserved {name}` to {slot}."
        )
    } else if published_reserves(published, &change.path) {
        format!(
            "The baseline being replaced records `{name}`{in_shape} as retired{held}, but the \
             source has dropped the tombstone. A tombstone is a permanent reservation (ridl \
             §11). Put `reserved {name}` back at {slot}."
        )
    } else {
        format!(
            "`{name}` is gone from the source but the baseline being replaced still declares \
             it{in_shape}{held}. Publishing would free its ordinal for a later interaction to \
             reuse, with nothing left to record that it was ever taken. Retire it in place with \
             `reserved {name}`."
        )
    }
}

/// The interaction a `<package>/<container>/<name>` diff path names, as the
/// published IR declares it: the live declaration, or the `reserved <name>`
/// tombstone that retires it.
///
/// This reads the same shape `ridl_diff`'s own `live_interactions` and
/// `reserved_names` walks read (`Interface::interactions`, a `Decl` whose
/// `kind` says whether it is a `ReservedSlot`), so nothing the gate says about
/// the published side depends on the wording of a `Change`'s rendered
/// `before`/`after` text. The container is found through `Package::shapes`,
/// which yields a top-level interface and an inline-form service's own shape
/// alike — the two containers `ridl_diff`'s interaction walk is ever called
/// on. A named-form service is not a shape, so a service-level diff path
/// finds nothing here.
///
/// Every shape carrying the container's name is searched, and the first
/// interaction that matches is the answer. A declared `interface doors`
/// beside an inline-form `service doors` are two shapes under one identity
/// name — the walk keys them apart by form, but a diff path does not — and
/// reading only the first shape of that name answered from the interface
/// when the service was the one that declared the name (driftsys/ridl#339
/// case 2). Two shapes both declaring the name cannot be told apart from
/// the path; the walk's own key order puts the declared interface first.
fn published_interaction<'a>(
    published: &'a [ridl_ir::v2::Package],
    path: &str,
) -> Option<&'a ridl_ir::v2::Decl> {
    let mut parts = path.split('/');
    let (Some(pkg), Some(container), Some(name)) = (parts.next(), parts.next(), parts.next())
    else {
        return None;
    };
    let package = published.iter().find(|package| package.name == pkg)?;
    package
        .shapes()
        .filter(|shape| shape.name == container)
        .find_map(|shape| {
            shape
                .interface
                .interactions
                .iter()
                .find(|decl| match &decl.kind {
                    Some(ridl_ir::v2::decl::Kind::ReservedSlot(reserved)) => {
                        reserved.name.as_deref() == Some(name)
                    }
                    Some(_) => decl.name == name,
                    None => false,
                })
        })
}

/// Whether the published IR already retires the interaction a diff path names
/// — a `reserved <name>` tombstone already present in the baseline being
/// replaced.
fn published_reserves(published: &[ridl_ir::v2::Package], path: &str) -> bool {
    published_interaction(published, path)
        .is_some_and(|decl| matches!(&decl.kind, Some(ridl_ir::v2::decl::Kind::ReservedSlot(_))))
}

/// The ordinal the interaction a diff path names holds in the published IR,
/// live or retired — the slot every RIDL-408 remedy tells the author to keep.
fn published_ordinal(published: &[ridl_ir::v2::Package], path: &str) -> Option<u32> {
    published_interaction(published, path).map(|decl| decl.ordinal)
}

/// The interface level of the publication gate — the lock's own rules (lock
/// design §8; the design's §4 table, `ridl baseline` column): a RIDL-411 for
/// every interface in the fresh snapshots whose number is provisional, and a
/// RIDL-412 for every interface the baseline being replaced holds under a
/// non-zero number that the fresh snapshots neither carry nor retire. Returns
/// whether any was recorded.
///
/// RIDL-411 reads the fresh snapshots alone, so a first publication is
/// refused too: a provisional number is no identity, and a snapshot holding
/// one records nothing a later comparison can hold the interface to. RIDL-412
/// reads the same `diff_sets` report the RIDL-408 gate walks, keeping the
/// interface-level `DeclRemoved` changes: `ridl_diff` matches interfaces by
/// number, so such a change is a number the fresh side carries under no name
/// and does not list as retired, and the package-level `DeclRemoved` of a
/// package whose whole unit is gone from the fresh set, which loses every
/// shape the package held ([`dropped_numbers`]). A published `number` 0
/// predates the lock and was matched by name, so its removal is not refused
/// (plan decision PD-9). RIDL-412 is a lock line deleted by hand, or a package
/// or unit removed without retiring its numbers: a live entry with no
/// declaration fails the build with RIDL-409 before publication.
///
/// The published snapshots are read flat from `out_dir` through
/// [`load_published`], as [`untombstoned_removals`] reads them and for the
/// same reason.
fn interface_refusals(
    entry: &Path,
    out_dir: &Path,
    staging: &Path,
    run: &mut CliRun,
) -> Result<bool, ExitCode> {
    let fresh = load_snapshots(
        &snapshot_files(staging).map_err(report_diff_side_error)?,
        None,
    )?;
    let mut index: Option<DeclIndex> = None;
    let mut refusals = Vec::new();
    for package in &fresh {
        for shape in package.shapes() {
            if !shape.interface.provisional {
                continue;
            }
            let index = index.get_or_insert_with(|| DeclIndex::build(entry));
            refusals.push(Diagnostic {
                code: DiagCode::RIDL_411,
                severity: Severity::Error,
                message: provisional_number_message(package, &shape, entry),
                primary: index.shape_span(&package.name, shape.name, &mut run.sources),
                labels: Vec::new(),
                fixits: Vec::new(),
            });
        }
    }

    if out_dir.is_dir() {
        let published = load_published(out_dir)?;
        if !published.is_empty() {
            let report = ridl_diff::diff_sets(&published, &fresh);
            for change in &report.changes {
                for (package, shape) in dropped_numbers(change, &published, &fresh) {
                    let unit = published_unit(package, &fresh);
                    let gone = ridl_ir::v2::packages_of_unit(unit, &fresh).next().is_none();
                    refusals.push(Diagnostic {
                        code: DiagCode::RIDL_412,
                        severity: Severity::Error,
                        message: dropped_number_message(package, &shape, unit, gone),
                        primary: detached_span(),
                        labels: Vec::new(),
                        fixits: Vec::new(),
                    });
                }
            }
        }
    }

    let refused = !refusals.is_empty();
    run.diagnostics.extend(refusals);
    Ok(refused)
}

/// The RIDL-411 message: the lock key the entry would carry, the provisional
/// number the checker showed, and the command that records it.
fn provisional_number_message(
    package: &ridl_ir::v2::Package,
    shape: &ridl_ir::v2::InterfaceShape<'_>,
    entry: &Path,
) -> String {
    let key = lock::shape_key(package, shape);
    format!(
        "`{key}` has a provisional interface number ({}) in unit `{}`: no entry in \
         `interfaces.lock` records it, and a provisional number is no identity. Run `ridl lock \
         {}` to allocate and record the number, then publish.",
        shape.interface.number,
        ridl_ir::v2::unit_of(package),
        entry.display()
    )
}

/// The unit a published package is compared in. A snapshot written before
/// the IR carried `unit` has an empty field, and its own name is not a unit
/// of the fresh set when the package is a subpackage: the migration to one
/// lock per unit re-publishes such a baseline, so the legacy snapshot is read
/// in the unit of the fresh package of the same name. A package the fresh
/// set no longer declares keeps [`ridl_ir::v2::unit_of`].
fn published_unit<'a>(
    package: &'a ridl_ir::v2::Package,
    fresh: &'a [ridl_ir::v2::Package],
) -> &'a str {
    if package.unit.is_empty()
        && let Some(current) = fresh.iter().find(|current| current.name == package.name)
    {
        return ridl_ir::v2::unit_of(current);
    }
    ridl_ir::v2::unit_of(package)
}

/// The published shapes a `DeclRemoved` change loses, when the number each
/// held is one the lock allocated (not 0) and no package of its unit
/// ([`published_unit`]) in the fresh set declares or retires — the RIDL-412
/// shape.
///
/// An interface-level change has a two-segment path and the walk's `interface`
/// marker as its `before`; it loses that one shape. A service's own
/// `DeclRemoved` carries `service` there and loses nothing. A package-level
/// change has one segment and `package <name>` as its `before`: the whole
/// package is gone while its unit has no package in the fresh set (the walk
/// reports each shape of a package whose unit stays on its own line), so it
/// loses every shape the published package holds.
fn dropped_numbers<'a>(
    change: &ridl_diff::Change,
    published: &'a [ridl_ir::v2::Package],
    fresh: &[ridl_ir::v2::Package],
) -> Vec<(&'a ridl_ir::v2::Package, ridl_ir::v2::InterfaceShape<'a>)> {
    if change.category != ridl_diff::Category::DeclRemoved {
        return Vec::new();
    }
    let mut parts = change.path.split('/');
    let (Some(pkg), name, None) = (parts.next(), parts.next(), parts.next()) else {
        return Vec::new();
    };
    let Some(package) = published.iter().find(|package| package.name == pkg) else {
        return Vec::new();
    };
    let lost: Vec<_> = match (name, change.before.as_deref()) {
        (Some(name), Some("interface")) => package
            .shapes()
            .filter(|shape| shape.name == name)
            .take(1)
            .collect(),
        (None, Some(before)) if before.starts_with("package ") => package.shapes().collect(),
        _ => return Vec::new(),
    };
    lost.into_iter()
        .filter(|shape| number_is_lost(package, shape, fresh))
        .map(|shape| (package, shape))
        .collect()
}

/// Whether the number a published shape holds is one the lock allocated and
/// no package of its unit in the fresh set declares or retires.
fn number_is_lost(
    package: &ridl_ir::v2::Package,
    shape: &ridl_ir::v2::InterfaceShape<'_>,
    fresh: &[ridl_ir::v2::Package],
) -> bool {
    let number = shape.interface.number;
    if number == 0 {
        return false;
    }
    // The number is kept when any package of the unit declares it — a
    // rename across packages of one unit keeps it, and so does the
    // re-numbering of a legacy baseline whose numbers ran per package — or
    // retires it.
    let kept = ridl_ir::v2::packages_of_unit(published_unit(package, fresh), fresh).any(|member| {
        member.retired.iter().any(|entry| entry.number == number)
            || member
                .shapes()
                .any(|shape| !shape.interface.provisional && shape.interface.number == number)
    });
    !kept
}

/// The RIDL-412 message: the name and number the baseline holds, the unit
/// the gate compared it in, and the line that restores the record.
fn dropped_number_message(
    package: &ridl_ir::v2::Package,
    shape: &ridl_ir::v2::InterfaceShape<'_>,
    unit: &str,
    unit_gone: bool,
) -> String {
    // The key is spelled relative to `unit`, which for a legacy snapshot is
    // not the unit the package itself names.
    let key = if shape.is_inline() {
        lock::shape_key(package, shape)
    } else {
        LockKey::Interface(ridl_ir::v2::relative_name(unit, &package.name, shape.name))
    };
    let number = shape.interface.number;
    if unit_gone {
        // The unit's `interfaces.lock` left with the unit, so there is no
        // line to restore: the override is to publish from an empty baseline.
        return format!(
            "`{key}` holds interface number {number} in the baseline being replaced, in unit \
             `{unit}`, but the workspace no longer has that unit. Publishing would lose the only \
             record that the number was allocated. When the unit is removed on purpose, delete \
             the snapshots of unit `{unit}` from `.ridl/baseline/`: a first publication holds no \
             number to lose. Otherwise restore the unit and its `interfaces.lock` from version \
             control."
        );
    }
    format!(
        "`{key}` holds interface number {number} in the baseline being replaced, in unit \
         `{unit}`, but the fresh snapshot neither declares that number nor retires it. \
         Publishing would lose the only record that the number was allocated, and `next` could \
         hand it to a later interface. Restore the line `{key} {number}` in the unit's \
         `interfaces.lock` from version control — `{key} {number} retired` when the interface is \
         gone."
    )
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
/// declares, and replaces the `.catalogs` set with the staged one. Only
/// `.ir.json` and `.catalogs` files are touched: `out_dir` may be a directory
/// a user pointed `--out` at, and nothing else in it is this command's to
/// delete.
///
/// The steps run in this order:
///
/// 1. Every published `.catalogs` file is removed.
/// 2. The fresh snapshots move in, each rename replacing the stale file of the
///    same name.
/// 3. The fresh `.catalogs` files move in.
/// 4. The stale snapshots no fresh one replaced are removed.
///
/// A failure part-way — a rename refused, a disk that fills — leaves
/// `out_dir` holding one snapshot per package, some fresh and some stale,
/// which the next run compares against package by package. The other order
/// for snapshots, delete then move, left `out_dir` empty after the same
/// failure, and an empty directory is a first publication to
/// [`untombstoned_removals`]: the next run would have skipped the gate.
///
/// The histories are removed first so that a failure never leaves a history
/// beside a snapshot it does not describe. A history left from the replaced
/// baseline beside a fresh snapshot lets the next run carry hashes past a
/// breaking change; a fresh history beside a replaced snapshot lists a
/// catalog that is not published. After a failure, a unit has its fresh
/// history or none, and a unit with none starts its chain again.
fn publish_baseline(staging: &Path, out_dir: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(out_dir)?;
    for history in catalogs_files(out_dir)? {
        std::fs::remove_file(history)?;
    }
    let mut published = BTreeSet::new();
    for fresh in ir_json_files(staging)? {
        let name = fresh
            .file_name()
            .expect("a listed snapshot path has a file name")
            .to_os_string();
        std::fs::rename(&fresh, out_dir.join(&name))?;
        published.insert(name);
    }
    for fresh in catalogs_files(staging)? {
        let name = fresh
            .file_name()
            .expect("a listed history path has a file name")
            .to_os_string();
        std::fs::rename(&fresh, out_dir.join(&name))?;
    }
    for stale in ir_json_files(out_dir)? {
        if stale
            .file_name()
            .is_some_and(|name| !published.contains(name))
        {
            std::fs::remove_file(stale)?;
        }
    }
    std::fs::remove_dir_all(staging)
}

/// Every `*.catalogs` file directly in `dir`.
fn catalogs_files(dir: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_file()
            && path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.ends_with(ridl_core::catalog_history::FILE_SUFFIX))
        {
            files.push(path);
        }
    }
    files.sort();
    Ok(files)
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
        // A prototext or binary IR artifact is refused by name — baselines
        // stay `.ir.json` (ADR-0014 decision 5) — before the snapshot loader
        // can report it as malformed JSON, which misdiagnoses the mistake.
        Some(explicit) if is_non_json_ir(explicit) => {
            eprintln!(
                "error: the baseline `{}` is not an `.ir.json` snapshot: a baseline stays \
                 `.ir.json` (ADR-0014 decision 5); publish one with `ridl baseline`",
                explicit.display()
            );
            Err(ExitCode::from(2))
        }
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
/// the one the compile loads from ([`ridl_core::find_root`]), so an entry
/// inside a workspace member uses the workspace root's baseline, falling back
/// to `entry`'s own directory when there is no manifest anywhere above it
/// (single-file mode).
fn default_baseline_dir(entry: &Path) -> PathBuf {
    let start = if entry.is_file() {
        entry.parent().unwrap_or(Path::new(".")).to_path_buf()
    } else {
        entry.to_path_buf()
    };
    ridl_core::find_root(&start)
        .unwrap_or(start)
        .join(".ridl")
        .join("baseline")
}

/// Compares the checked workspace against the baseline at `location`, appends
/// a RIDL-407 warning for every ordinal-affecting change, and adds the rename
/// label to every RIDL-409 whose orphan entry has exactly one same-shape
/// candidate ([`rename_labels`]).
///
/// The workspace is compiled a second time here, through
/// [`ridlc::compile_workspace`], because `run_check` renders diagnostics but
/// does not hand back the IR. The cost is paid only when a baseline is actually
/// present, and never on a run that failed for anything but RIDL-409.
/// `explicit` — whether
/// `location` came from a `--baseline` flag rather than auto-discovery — is
/// passed straight through to [`load_baseline`], which it uses to tell an
/// explicit `--baseline` holding no snapshot (a refusal) from an
/// auto-discovered directory holding none (a silent skip).
fn desk_check(
    entry: &Path,
    location: &Path,
    explicit: bool,
    run: &mut CliRun,
) -> Result<(), ExitCode> {
    let baseline = load_baseline(location, explicit)?;
    if baseline.is_empty() {
        return Ok(());
    }

    let mut db = ridl_core::RidlDatabase::default();
    let output = match ridlc::compile_workspace(&mut db, entry) {
        Ok(output) => output,
        Err(err) => {
            eprintln!("error: {}: {err}", entry.display());
            return Err(ExitCode::from(2));
        }
    };
    // The caller's gate read the diagnostics of the entry's member only. The
    // desk check compares the whole workspace, so it runs only when no member
    // has an error other than RIDL-409. These diagnostics carry the emit
    // severities, so a lint is never an error here.
    if !lock::only_lock_orphans(&output.diagnostics) {
        return Ok(());
    }
    let report_scope = output.report_scope;
    let current: Vec<ridl_ir::v2::Package> = output
        .checked
        .into_iter()
        .map(|checked| checked.ir)
        .collect();

    // `ridl.std` is context, as in `run_diff`, so the desk reads the verdict
    // the gate gives (driftsys/ridl#598).
    let std = [ridlc::std_ir()];
    let report = ridl_diff::diff_sets_in(&baseline, &current, &std);
    let index = DeclIndex::build(entry);
    let mut warnings = Vec::new();
    for change in &report.changes {
        let message = if ORDINAL_CATEGORIES.contains(&change.category) {
            drift_message(change)
        } else if MEMBER_CATEGORIES.contains(&change.category)
            && change.verdict == ridl_diff::Verdict::Breaking
        {
            let Some(drift) = member_drift(change, &baseline, &current, &std) else {
                continue;
            };
            member_message(change, drift)
        } else {
            continue;
        };
        warnings.push(Diagnostic {
            code: DiagCode::RIDL_407,
            severity: Severity::Warning,
            message,
            primary: index.span_of(&change.path, &mut run.sources),
            labels: Vec::new(),
            fixits: Vec::new(),
        });
    }
    // An entry inside a member reports on the member only (ADR-0024 decision
    // 9, as ADR-0026 amends it).
    ridlc::retain_in_report_scope(&mut warnings, &run.sources, report_scope.as_deref());
    run.diagnostics.extend(warnings);
    rename_labels(&baseline, &current, &index, run);
    Ok(())
}

/// The rename hint (lock design §4; plan decision PD-5). For every RIDL-409
/// the compile produced, when exactly one declaration without an entry in
/// the same unit has the orphan entry's shape in the published baseline,
/// a secondary label goes on that diagnostic, at the candidate's declaration,
/// naming the one `ridl lock <unit> --rename Old=New`. Nothing otherwise — no
/// baseline package, no candidate of that shape, or several — and never a
/// second diagnostic. The orphan's key is read from the lock line the
/// diagnostic points at (its span is the entry's line, plan decision PD-3),
/// and its unit from the lock file's directory through the index.
fn rename_labels(
    baseline: &[ridl_ir::v2::Package],
    current: &[ridl_ir::v2::Package],
    index: &DeclIndex,
    run: &mut CliRun,
) {
    let orphans: Vec<(usize, LockKey, String)> = run
        .diagnostics
        .iter()
        .enumerate()
        .filter(|(_, diagnostic)| diagnostic.code == DiagCode::RIDL_409)
        .filter_map(|(position, diagnostic)| {
            let (key, dir) = orphan_entry(&run.sources, diagnostic)?;
            Some((position, key, dir))
        })
        .collect();
    for (position, old, dir) in orphans {
        // The lock sits in the unit's manifest directory, which need not
        // declare a package itself; the unit holds the orphan entry's shape
        // under its catalog name, in any of its packages.
        let Some(unit) = index.unit_of_dir(&dir, current) else {
            continue;
        };
        let Some((published_package, published)) = ridl_ir::v2::packages_of_unit(unit, baseline)
            .find_map(|member| {
                member
                    .shapes()
                    .find(|shape| lock::shape_key(member, shape) == old)
                    .map(|shape| (member, shape))
            })
        else {
            continue;
        };
        let published_members = shape_members((published_package, published.interface));
        let candidates: Vec<(&ridl_ir::v2::Package, ridl_ir::v2::InterfaceShape<'_>)> =
            ridl_ir::v2::packages_of_unit(unit, current)
                .flat_map(|fresh| {
                    let published_members = &published_members;
                    fresh
                        .shapes()
                        .filter(move |shape| {
                            shape.interface.provisional
                                && same_shape(published_members, (fresh, shape.interface))
                        })
                        .map(move |shape| (fresh, shape))
                })
                .collect();
        let [(candidate_package, candidate)] = candidates.as_slice() else {
            continue;
        };
        let new = lock::shape_key(candidate_package, candidate);
        let span = index.shape_span(&candidate_package.name, candidate.name, &mut run.sources);
        run.diagnostics[position].labels.push(Label {
            span,
            message: format!(
                "same shape as `{old}` in the published baseline: run `ridl lock {dir} --rename \
                 {old}={new}`"
            ),
        });
    }
}

/// The orphan entry a RIDL-409 points at: its key, read from the first field
/// of the lock line under the diagnostic's span, and the unit's manifest
/// directory — the lock file's parent — as the message names it (plan
/// decision PD-4).
fn orphan_entry(sources: &SourceMap, diagnostic: &Diagnostic) -> Option<(LockKey, String)> {
    let path = sources.path(diagnostic.primary.file)?;
    let text = sources.text(diagnostic.primary.file)?;
    let range = diagnostic.primary.range;
    let line = text.get(usize::from(range.start())..usize::from(range.end()))?;
    let key = line.split(' ').next()?.parse().ok()?;
    Some((key, directory_of(path)))
}

/// Whether two interface bodies are the same shape (lock design §4): their
/// `interactions` lists compare equal once each interaction's `doc`, `labels`
/// and `deprecated` are blanked on both sides. Every other field of an
/// interaction — name, kind, ordinal, payload, timing, parameters, return,
/// contracts, visibility — and every `reserved` tombstone must match. The
/// `Interface`'s own fields — name, visibility, doc, number, provisional flag
/// — are not members and are not compared: the baseline's interface is frozen
/// and the candidate is provisional, so whole values would never match.
///
/// The published side comes prepared by [`shape_members`], once per orphan
/// entry; the candidate comes with the package that declares it, because the
/// two may sit in different packages of the unit.
fn same_shape(
    published: &[ridl_ir::v2::Decl],
    new: (&ridl_ir::v2::Package, &ridl_ir::v2::Interface),
) -> bool {
    shape_members(new) == published
}

/// An interface's members in the form [`same_shape`] compares: each
/// interaction with its `doc`, `labels` and `deprecated` blanked and each
/// type reference in its canonical `pkg.Name` form
/// ([`ridl_ir::catalog_hash::canonicalize_refs`]), resolved against
/// `package`, the package that declares the interface. So a payload written
/// bare in its own package matches the same type written qualified in
/// another.
fn shape_members(
    (package, interface): (&ridl_ir::v2::Package, &ridl_ir::v2::Interface),
) -> Vec<ridl_ir::v2::Decl> {
    interface
        .interactions
        .iter()
        .cloned()
        .map(|mut decl| {
            decl.doc = String::new();
            decl.labels = Vec::new();
            decl.deprecated = None;
            ridl_ir::catalog_hash::canonicalize_refs(&mut decl, package);
            decl
        })
        .collect()
}

/// The directory a file path sits in, as a string: its parent, or `.` when
/// the path has none — the form the loader records a unit's manifest
/// directory in and the RIDL-409 message names it in.
fn directory_of(path: &str) -> String {
    match Path::new(path).parent() {
        Some(dir) if !dir.as_os_str().is_empty() => dir.to_string_lossy().into_owned(),
        _ => ".".to_string(),
    }
}

/// What a struct field's or union arm's change is, once the two bodies are
/// read: the words a [`MemberReordered`](ridl_diff::Category::MemberReordered),
/// [`DeclAdded`](ridl_diff::Category::DeclAdded) or
/// [`DeclRemoved`](ridl_diff::Category::DeclRemoved) change does not carry
/// on its own, and the RIDL-407 message needs (driftsys/ridl#533). The diff
/// reports no reorder beside an addition or a removal, so the lists a
/// variant carries name the siblings whose ordinal changed in the same
/// edit: the warning for the added or removed member is the one place that
/// says so.
#[derive(Clone, Debug, PartialEq, Eq)]
enum MemberDrift {
    /// A member the baseline does not declare, at an ordinal the baseline
    /// already assigns or retires: `holder` is the live member the baseline
    /// declares there, or `None` for a `reserved` entry. `moved` names the
    /// surviving members whose ordinal changed.
    Inserted {
        ordinal: u32,
        holder: Option<String>,
        moved: Vec<String>,
    },
    /// A member declared after every ordinal the baseline assigns or
    /// retires, in an edit that also moved, removed or inserted another
    /// member, which makes the classifier report the addition as breaking.
    /// The warning stands for the siblings it names. `refused_absent` says
    /// the member is a struct field that is breaking on its own too: a
    /// reader of this version refuses a payload of the baseline, which does
    /// not carry the field (driftsys/ridl#598). An append with none of those
    /// siblings beside it draws no warning, even when it is breaking for its
    /// type: no ordinal moved.
    Appended {
        moved: Vec<String>,
        gone: Vec<String>,
        inserted: Vec<String>,
        refused_absent: bool,
    },
    /// An arm appended to a union that is, or becomes, a result union, with
    /// no other change: breaking because a result union's arms are its
    /// transport identity (ADR-0008 decision 4).
    ResultArm,
    /// A member the baseline declares at `ordinal` that the body no longer
    /// holds there. `tombstone` is the ordinal of a `reserved` entry under
    /// its name elsewhere in the body, and `shifted` names the surviving
    /// members whose ordinal changed.
    Removed {
        ordinal: u32,
        tombstone: Option<u32>,
        shifted: Vec<String>,
    },
    /// A member the baseline declares whose ordinal a `reserved` entry now
    /// holds. `name_kept` says the entry carries the member's name; a bare
    /// `reserved N`, or an entry under another name, holds the slot but
    /// leaves the name free to redeclare. `ridl diff` still reports it as a
    /// breaking removal: its composite comparison matches by name and does
    /// not read the `reserved` list (the limit `diff_composite` records), and
    /// the desk repeats the gate rather than disagree with it.
    Retired { name_kept: bool },
    /// A surviving member whose ordinal changed while it kept its place
    /// among the live members: a `reserved` entry above it was added, moved
    /// or removed. Decided per member, so a swap and a tombstone in one
    /// edit report the swapped members as moved and the rest as shifted.
    Shifted,
    /// A surviving member whose place among the live members changed.
    Moved,
}

/// A struct or union body as the desk check reads it (typl §7.4).
struct CompositeBody {
    /// Each live member with its ordinal, in declaration order.
    live: Vec<(String, u32)>,
    /// Each `reserved` entry's retired name, if it carries one, and ordinal.
    reserved: Vec<(Option<String>, u32)>,
    /// A union whose arms are its transport identity (ADR-0008 decision 4).
    result: bool,
}

impl CompositeBody {
    /// The highest ordinal the body assigns or retires — the classifier's
    /// mark for an append: a new member above it keeps every ordinal, though
    /// an appended non-optional struct field whose type excludes 0 is still
    /// breaking on its own (driftsys/ridl#598).
    fn high_water(&self) -> Option<u32> {
        self.live
            .iter()
            .map(|(_, ordinal)| *ordinal)
            .chain(self.reserved.iter().map(|(_, ordinal)| *ordinal))
            .max()
    }

    fn ordinal_of(&self, member: &str) -> Option<u32> {
        self.live
            .iter()
            .find(|(name, _)| name == member)
            .map(|(_, ordinal)| *ordinal)
    }

    /// The members live in both bodies whose ordinal differs, in this
    /// body's declaration order.
    fn moved_since(&self, baseline: &CompositeBody) -> Vec<String> {
        self.live
            .iter()
            .filter(|(name, ordinal)| {
                baseline
                    .ordinal_of(name)
                    .is_some_and(|before| before != *ordinal)
            })
            .map(|(name, _)| name.clone())
            .collect()
    }
}

/// The body of the struct or union `container` in `package`, or `None` for
/// any other declaration kind — an enum, an enum set, a type or a constant
/// — which is what keeps them out of the desk check.
fn composite_body(
    packages: &[ridl_ir::v2::Package],
    package: &str,
    container: &str,
) -> Option<CompositeBody> {
    use ridl_ir::v2::decl::Kind;
    use ridl_ir::v2::struct_member::Member;
    let decl = packages
        .iter()
        .find(|candidate| candidate.name == package)?
        .decls
        .iter()
        .find(|decl| decl.name == container)?;
    match decl.kind.as_ref()? {
        Kind::StructDef(def) => {
            let mut live = Vec::new();
            let mut reserved = Vec::new();
            for member in &def.members {
                match &member.member {
                    Some(Member::Field(field)) => live.push((field.name.clone(), field.ordinal)),
                    Some(Member::Reserved(entry)) => {
                        reserved.push((entry.name.clone(), entry.ordinal));
                    }
                    None => {}
                }
            }
            Some(CompositeBody {
                live,
                reserved,
                result: false,
            })
        }
        Kind::UnionDef(def) => Some(CompositeBody {
            live: def
                .arms
                .iter()
                .map(|arm| (arm.name.clone(), arm.ordinal))
                .collect(),
            reserved: def
                .reserved
                .iter()
                .map(|entry| (entry.name.clone(), entry.ordinal))
                .collect(),
            result: def.is_result,
        }),
        _ => None,
    }
}

/// Reads the two bodies a member-level change names and says what the
/// change is, or `None` when the path is not `<package>/<container>/<member>`
/// with a struct or union at `<container>` on both sides. The one exception
/// is a removal whose container is no longer a struct or union in the
/// workspace: it is reported as a bare removal. `std` is the built-in
/// `ridl.std`, which a field type may name ([`ridl_diff::absence_refused`]).
fn member_drift(
    change: &ridl_diff::Change,
    baseline: &[ridl_ir::v2::Package],
    current: &[ridl_ir::v2::Package],
    std: &[ridl_ir::v2::Package],
) -> Option<MemberDrift> {
    let mut parts = change.path.split('/');
    let (Some(package), Some(container), Some(member), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return None;
    };
    match change.category {
        ridl_diff::Category::DeclAdded => {
            let before = composite_body(baseline, package, container)?;
            let after = composite_body(current, package, container)?;
            let ordinal = after.ordinal_of(member)?;
            let moved = after.moved_since(&before);
            Some(if before.high_water().is_some_and(|mark| ordinal <= mark) {
                let holder = before
                    .live
                    .iter()
                    .find(|(_, held)| *held == ordinal)
                    .map(|(name, _)| name.clone());
                MemberDrift::Inserted {
                    ordinal,
                    holder,
                    moved,
                }
            } else {
                let gone: Vec<String> = before
                    .live
                    .iter()
                    .filter(|(name, _)| after.ordinal_of(name).is_none())
                    .map(|(name, _)| name.clone())
                    .collect();
                let inserted: Vec<String> = after
                    .live
                    .iter()
                    .filter(|(name, held)| {
                        name != member
                            && before.ordinal_of(name).is_none()
                            && before.high_water().is_some_and(|mark| *held <= mark)
                    })
                    .map(|(name, _)| name.clone())
                    .collect();
                let alone = moved.is_empty() && gone.is_empty() && inserted.is_empty();
                if alone && (before.result || after.result) {
                    MemberDrift::ResultArm
                } else if alone {
                    // A struct field appended alone and still reported breaking
                    // is breaking for its type: a reader of the new version
                    // refuses an old payload, which does not carry the field
                    // (driftsys/ridl#598). No ordinal moved, so it is not the
                    // drift RIDL-407 reports; `ridl diff` gates it.
                    return None;
                } else {
                    MemberDrift::Appended {
                        moved,
                        gone,
                        inserted,
                        refused_absent: ridl_diff::absence_refused(
                            current, std, package, container, member,
                        ),
                    }
                }
            })
        }
        ridl_diff::Category::DeclRemoved => {
            let before = composite_body(baseline, package, container)?;
            let ordinal = before.ordinal_of(member)?;
            let after = composite_body(current, package, container);
            // `Some(None)` is a bare `reserved N` at the ordinal and `None`
            // is no entry there: the two outer cases branch differently
            // below, so this is not flattened.
            let holder = after.as_ref().and_then(|after| {
                after
                    .reserved
                    .iter()
                    .find(|(_, held)| *held == ordinal)
                    .map(|(name, _)| name.clone())
            });
            Some(match holder {
                Some(name) => MemberDrift::Retired {
                    name_kept: name.as_deref() == Some(member),
                },
                None => {
                    let tombstone = after.as_ref().and_then(|after| {
                        after
                            .reserved
                            .iter()
                            .find(|(name, _)| name.as_deref() == Some(member))
                            .map(|(_, held)| *held)
                    });
                    MemberDrift::Removed {
                        ordinal,
                        tombstone,
                        shifted: after
                            .map(|after| after.moved_since(&before))
                            .unwrap_or_default(),
                    }
                }
            })
        }
        ridl_diff::Category::MemberReordered => {
            let before = composite_body(baseline, package, container)?;
            let after = composite_body(current, package, container)?;
            let place =
                |body: &CompositeBody| body.live.iter().position(|(name, _)| name == member);
            Some(if place(&before) == place(&after) {
                MemberDrift::Shifted
            } else {
                MemberDrift::Moved
            })
        }
        _ => None,
    }
}

/// `` `a` ``, `` `a` and `b` ``, `` `a`, `b` and `c` ``.
fn quoted_list(names: &[String]) -> String {
    match names {
        [] => String::new(),
        [one] => format!("`{one}`"),
        [head @ .., last] => {
            let head: Vec<String> = head.iter().map(|name| format!("`{name}`")).collect();
            format!("{} and `{last}`", head.join(", "))
        }
    }
}

/// The RIDL-407 message for a struct field's or union arm's change, in the
/// register [`drift_message`] sets: the member and the body it is declared
/// in, the consequence under typl §7.4, and the edit that keeps the baseline
/// intact.
fn member_message(change: &ridl_diff::Change, drift: MemberDrift) -> String {
    let (shape, name) = shape_and_name(&change.path);
    let in_shape = shape.map_or(String::new(), |shape| format!(" in `{shape}`"));
    match drift {
        MemberDrift::Inserted {
            ordinal,
            holder,
            moved,
        } => {
            let holder = match holder {
                Some(holder) => format!("assigns to `{holder}`"),
                None => "retires".to_string(),
            };
            let moved = if moved.is_empty() {
                String::new()
            } else {
                format!(
                    "; {} also changed ordinal in this edit, which `ridl diff` does not \
                     report beside an addition",
                    quoted_list(&moved)
                )
            };
            format!(
                "`{name}` takes ordinal {ordinal}{in_shape}, which the published baseline \
                 {holder}. A struct field or union arm keeps its ordinal for ever (typl §7.4): \
                 a member inserted above an existing one shifts every later wire identity, and \
                 one placed in a retired slot revives it{moved} — declare it at the end of the \
                 body instead",
            )
        }
        MemberDrift::Appended {
            moved,
            gone,
            inserted,
            refused_absent,
        } => {
            let mut reasons = Vec::new();
            if !moved.is_empty() {
                reasons.push(format!("{} changed ordinal", quoted_list(&moved)));
            }
            if !gone.is_empty() {
                reasons.push(format!("{} is no longer declared", quoted_list(&gone)));
            }
            if !inserted.is_empty() {
                reasons.push(format!("{} was inserted", quoted_list(&inserted)));
            }
            let by_type = if refused_absent {
                format!(
                    ". `{name}` is also breaking on its own: its type does not allow the value \
                     0, so a reader built against this version refuses a payload of the \
                     baseline, which does not carry the field (typl §7.4) — declare it \
                     optional, with `?` after its type"
                )
            } else {
                String::new()
            };
            format!(
                "`{name}` is declared{in_shape} after every ordinal the published baseline \
                 assigns or retires, and `ridl diff` still reports the addition as breaking \
                 because {} in the same edit. The diff reports no reorder beside an addition or \
                 a removal, so this warning stands for that change: a struct field or union arm \
                 keeps its ordinal for ever (typl §7.4) — put the other members back where the \
                 baseline has them; this one stays at the end{by_type}",
                reasons.join(" and "),
            )
        }
        MemberDrift::ResultArm => format!(
            "`{name}` is declared{in_shape} after every ordinal the published baseline assigns \
             or retires, and `ridl diff` still reports the addition as breaking: a result \
             union's arms are its transport identity (ADR-0008 decision 4), so an arm added \
             or removed changes what a consumer built against the baseline must handle. This \
             warning repeats the gate rather than disagree with it",
        ),
        MemberDrift::Removed {
            ordinal,
            tombstone,
            shifted,
        } => {
            let tombstone = match tombstone {
                Some(held) => {
                    format!(", and the `reserved {name}` entry sits at ordinal {held}, not there")
                }
                None => String::new(),
            };
            let consequence = if shifted.is_empty() {
                "which a later member could take".to_string()
            } else {
                format!(
                    "and {} slid into a wire identity that is not its own",
                    quoted_list(&shifted)
                )
            };
            format!(
                "`{name}` is gone{in_shape} but the published baseline declares it at ordinal \
                 {ordinal}{tombstone}. Deleting the line frees its ordinal (typl §7.4), \
                 {consequence} — retire it in place with `reserved {name}` at ordinal {ordinal}, \
                 which holds the slot for ever",
            )
        }
        MemberDrift::Retired { name_kept: true } => format!(
            "`{name}` is retired{in_shape} with `reserved`, which keeps its ordinal \
             (typl §7.4), and `ridl diff` still reports the retirement as breaking: it \
             matches a struct field or union arm by name and does not yet read the body's \
             `reserved` entries, so it gates a tombstoned removal like a bare one. This \
             warning repeats the gate rather than disagree with it",
        ),
        MemberDrift::Retired { name_kept: false } => format!(
            "`{name}` is gone{in_shape}, and a `reserved` entry holds its ordinal without its \
             name: the slot is kept (typl §7.4), but the name is not retired and could be \
             redeclared with a new meaning — write `reserved {name}` instead. `ridl diff` \
             still reports the removal as breaking, because it matches a struct field or \
             union arm by name and does not yet read the body's `reserved` entries; this \
             warning repeats the gate rather than disagree with it",
        ),
        MemberDrift::Shifted => format!(
            "`{name}` has not moved{in_shape}, but its ordinal has changed since the published \
             baseline{}: a `reserved` entry above it was added, moved or removed. A tombstone \
             holds an ordinal exactly as a live member does (typl §7.4), so a consumer built \
             against the baseline would read this slot as a different member — put the \
             `reserved` entries back where the baseline has them and add new ones at the end",
            baseline_position(change, "ordinal"),
        ),
        MemberDrift::Moved => format!(
            "`{name}` has moved{in_shape} since the published baseline{}. Declaration order is \
             the wire identity of a struct field or union arm (typl §7.4), so a consumer built \
             against the baseline would read this slot as a different member — put the members \
             back in the baseline's order and add new ones at the end",
            baseline_position(change, "ordinal"),
        ),
    }
}

/// The RIDL-407 message for one ordinal-affecting interaction change; a
/// struct field's or union arm's is [`member_message`].
///
/// Written for the reader of a `.ridl` file, not for a reader of the diff
/// report. It names the member and the shape it is declared in — the words
/// in the source — rather than the slash-separated diff path, states the one
/// consequence that makes the warning worth reading (declaration order is the
/// wire identity, ridl §11), and names the edit that keeps the baseline intact. It used to
/// read `interaction ordinal changed against the baseline:
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
            baseline_position(change, "position"),
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
        // `ORDINAL_CATEGORIES` is the caller's filter and holds exactly the
        // categories of the arms above. Another category reaching here would
        // be a filter that grew without its messages, so this says only what
        // it can defend — and says it without the raw category token, which
        // is the vocabulary this code exists to keep out of the message.
        _ => format!(
            "`{name}`{in_shape} changed against the published baseline in a way that moves a \
             wire identity (ridl §11)"
        ),
    }
}

/// The shape and member name of a `<package>/<shape>/<interaction>` or
/// `<package>/<struct or union>/<member>` diff path. A path of any other
/// arity yields no shape and its last segment as the name, so the message
/// degrades to naming what it can rather than printing the raw path.
fn shape_and_name(path: &str) -> (Option<&str>, &str) {
    let parts: Vec<&str> = path.split('/').collect();
    match parts.as_slice() {
        [_package, shape, name] => (Some(shape), name),
        _ => (None, parts.last().copied().unwrap_or(path)),
    }
}

/// ` ({word} 2 there, {word} 4 here)` for a reorder whose two sides carry two
/// *different* slots, and the empty string otherwise.
///
/// `word` is the unit the caller's category renders the slot in: an
/// interaction reorder's detail carries no word of its own (the walk renders
/// a live reorder's sides as bare ordinals, `"2"`, and a tombstone's as
/// `"reserved at ordinal 2"`), so the caller names it "position"; a struct
/// field's or union arm's `MemberReordered` detail already reads `"ordinal
/// 2"`, so the caller names it "ordinal" to match. Either way the trailing
/// integer is what every spelling shares.
///
/// The equal case is dropped rather than printed. For an interaction, a
/// reorder is detected on *relative* order among the survivors, so an
/// interaction can change rank while its absolute slot stays put — an
/// insertion above it shifts the others past it — and "`doorClosed` has
/// moved (position 3 there, position 3 here)" would contradict itself in the
/// same breath. A struct field's or union arm's `MemberReordered` never
/// reaches this branch with equal sides: `diff_composite` emits that
/// category only when the member's own slot changed, so its `before` and
/// `after` always differ. Either way, the sentence about relative order
/// stands on its own; the numbers are a convenience that only helps when
/// they differ.
fn baseline_position(change: &ridl_diff::Change, word: &str) -> String {
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
            format!(" ({word} {was} there, {word} {now} here)")
        }
        _ => String::new(),
    }
}

/// Loads the baseline packages: every `.ir.json` in a directory, in file-name
/// order, or the single file `location` names.
///
/// Three directory shapes are refused rather than read as an *empty*
/// baseline, because skipping any of them silently would report a clean desk
/// check that ran against nothing: one holding IR artifacts but no `.ir.json`
/// (issue #218 item 4), one whose `.ir.json` snapshots sit a level below it
/// (issue #230), and, when `explicit` is true, any other directory that
/// yields no snapshot at all (driftsys/ridl#235). A fourth is refused
/// whether `explicit` or not, by [`snapshot_files`] before the three above
/// are considered: a directory holding an entry named like a snapshot whose
/// metadata cannot be read (driftsys/ridl#339 case 3). A directory that fits
/// none of the four and was found by auto-discovery (`explicit` false) keeps
/// yielding an empty baseline — that is the ordinary "no baseline published
/// yet" state, and [`desk_check`] skips it silently.
fn load_baseline(location: &Path, explicit: bool) -> Result<Vec<ridl_ir::v2::Package>, ExitCode> {
    let files = if location.is_dir() {
        let snapshots = snapshot_files(location).map_err(report_diff_side_error)?;
        if snapshots.is_empty() {
            // What is directly inside is the more specific complaint, so it
            // is the one reported when a directory somehow has both.
            if let Some(witness) = first_non_json_ir_in(location) {
                return Err(refuse_artifact_directory(
                    location,
                    &witness,
                    "a baseline stays `.ir.json` (ADR-0014 decision 5); publish one with \
                     `ridl baseline`",
                ));
            }
            if let Some(nested) =
                first_nested_snapshot_dir(location).map_err(report_diff_side_error)?
            {
                return Err(refuse_nested_snapshot_directory(
                    location,
                    &nested,
                    &format!("pass `--baseline {}` instead", nested.display()),
                ));
            }
            // The two refusals above name a specific, fixable mistake. This one
            // catches every remaining way a directory yields no snapshot —
            // snapshots two or more levels down, or an empty directory — and
            // refuses rather than comparing against nothing. Auto-discovery is
            // exempt: with no flag, "no baseline published yet" is legitimate.
            if explicit {
                return Err(refuse_empty_baseline(location));
            }
        }
        snapshots
    } else {
        vec![location.to_path_buf()]
    };
    load_snapshots(&files, None)
}

/// An explicit `--baseline` path that holds no snapshot at the depth the loader
/// reads is an input error, not a silent pass. The caller asserted that a
/// baseline is there. A comparison against nothing reports no drift and exits
/// 0, which is indistinguishable from a clean check — the same failure shape
/// ADR-0010 decision 6 closed for `ridl fmt` (driftsys/ridl#235).
///
/// The remedy names the likely mistake first — the path is aimed above the
/// snapshots, which is #235's own case (`--baseline ws` where
/// `ws/.ridl/baseline/` holds them) — and publishing into the named directory
/// second. The second is omitted when `location` is a source tree, as
/// [`is_source_dir`] judges it: following that advice there would write
/// `.ir.json` snapshots into the source tree itself, which is not what
/// `ridl baseline --out` is for (driftsys/ridl#340).
fn refuse_empty_baseline(location: &Path) -> ExitCode {
    let publish_clause = if is_source_dir(location) {
        String::new()
    } else {
        format!(
            ", or publish a first baseline into `{}` with `ridl baseline --out {}`",
            location.display(),
            location.display(),
        )
    };
    eprintln!(
        "error: the baseline `{}` holds no `.ir.json` snapshot directly inside it; point \
         `--baseline` at the directory that holds the snapshots (`ridl baseline` publishes \
         them to `.ridl/baseline/` at the workspace root){publish_clause}",
        location.display(),
    );
    ExitCode::from(2)
}

/// Reports a directory whose `.ir.json` snapshots sit one level below it
/// rather than inside it, and yields exit 2 (issue #230).
///
/// Such a directory holds no IR artifact *directly*, so
/// [`refuse_artifact_directory`] cannot see it, and the snapshot scan reads it
/// as an *empty* set — indistinguishable from the ordinary "no baseline
/// published yet" state, which stays a silent pass under auto-discovery. The
/// snapshots are
/// described where they are rather than descended into: descending would
/// accept a layout `ridl baseline` never writes, and would have to choose
/// between subdirectories when more than one holds snapshots, silently
/// merging two unrelated baselines.
///
/// `nested` is the subdirectory the message names; `remedy` finishes the
/// message with the path the calling command should have been given.
fn refuse_nested_snapshot_directory(dir: &Path, nested: &Path, remedy: &str) -> ExitCode {
    eprintln!(
        "error: {}: no `.ir.json` snapshot directly inside, but the subdirectory `{}` holds one; \
         snapshots are read from one directory, never from the directories below it; {remedy}",
        dir.display(),
        nested
            .file_name()
            .unwrap_or(nested.as_os_str())
            .to_string_lossy()
    );
    ExitCode::from(2)
}

/// Reports a directory that holds IR artifacts but no `.ir.json` snapshot —
/// a snapshot directory in an encoding this surface refuses, not a source
/// tree or an unpublished baseline — and yields exit 2 (issue #218 item 4).
/// `witness` is the artifact the message names; `expectation` finishes the
/// message with what the calling command accepts and the remedy.
fn refuse_artifact_directory(dir: &Path, witness: &Path, expectation: &str) -> ExitCode {
    eprintln!(
        "error: {}: the directory holds IR artifacts (`{}`) but no `.ir.json` snapshot; \
         {expectation}",
        dir.display(),
        witness
            .file_name()
            .unwrap_or(witness.as_os_str())
            .to_string_lossy()
    );
    ExitCode::from(2)
}

/// The remedy [`load_published`] appends when the snapshot it cannot
/// parse is the published baseline `ridl baseline` is about to replace.
///
/// The file stays fail-closed rather than being overwritten: a baseline that
/// cannot be read cannot be shown safe to replace, and replacing it would
/// destroy whatever ordinal record it held without any report — the exact
/// failure the gate exists to prevent. The reader cannot tell a damaged file
/// from one a toolchain with a different IR schema wrote (`from_json` rejects
/// an unknown field, ADR-0014 decision 14, and a snapshot carries no schema
/// marker), so the remedy names both causes. Neither branch tells the author
/// to delete the record unread: the second has the toolchain that wrote the
/// snapshot check the source against it first, and only then replaces it.
const PUBLISHED_PARSE_REMEDY: &str = "the file is left as it is, because a record that cannot be \
     read cannot be shown safe to replace. If the file is damaged, restore it from version \
     control or resolve the merge conflict left in it. If a toolchain with a different IR schema \
     wrote it, check the source against it with that toolchain (`ridl check --baseline`), then \
     remove the file and run `ridl baseline` with this one";

/// Loads the published snapshots `ridl baseline` is about to replace, flat
/// from `out_dir`, with [`PUBLISHED_PARSE_REMEDY`] on a parse failure, and
/// refuses — exit 2 — when two of them declare one package.
///
/// Two files declaring one package are two records of the same ordinals, and
/// the gate has no rule for choosing between them: `ridl_diff::diff_sets`
/// keeps the one that sorts last, while a lookup by package name reads the
/// first, so a copy that already lacked an interaction let a bare removal
/// publish, and the publication then deleted the copy (driftsys/ridl#339
/// case 1). Neither file is chosen here, and neither is touched: the
/// message names the package and both files and leaves the choice to the
/// author, who can tell a stray copy from the published record. The
/// refusal is this gate's alone — `ridl check --baseline` and `ridl diff`
/// still read such a directory as `diff_sets` reads it.
fn load_published(out_dir: &Path) -> Result<Vec<ridl_ir::v2::Package>, ExitCode> {
    let files = snapshot_files(out_dir).map_err(report_diff_side_error)?;
    let packages = load_snapshots(&files, Some(PUBLISHED_PARSE_REMEDY))?;
    let mut first_file_of: BTreeMap<&str, &Path> = BTreeMap::new();
    for (file, package) in files.iter().zip(&packages) {
        if let Some(first) = first_file_of.insert(&package.name, file) {
            eprintln!(
                "error: two published snapshots declare the package `{}`: `{}` and `{}`; both \
                 files are left as they are, because the gate cannot tell which one is the \
                 published record. Remove the copy that is not the published record (restore \
                 the directory from version control if unsure), then run `ridl baseline` again",
                package.name,
                first.display(),
                file.display(),
            );
            return Err(ExitCode::from(2));
        }
    }
    Ok(packages)
}

/// Deserializes every snapshot in `files`, in the order given. One that
/// cannot be read or parsed is exit 2 — a comparison against half a baseline
/// would be a lie about what is published. This is shared by `ridl check
/// --baseline` (through [`load_baseline`], where the file may be the single
/// `.ir.json` the flag names) and `ridl baseline` (through [`load_published`]
/// for the published side, and directly for the freshly built side).
/// `ridl diff` uses the separate reader behind [`ridlc::load_diff_side`].
///
/// `parse_remedy`, when given, finishes the parse-error message. Only the
/// caller knows which file it handed over, so only the caller can say what to
/// do about it: the published baseline gets [`PUBLISHED_PARSE_REMEDY`], and
/// every other input gets the bare parse error, because "remove the file"
/// would be wrong advice for a diff input or a `--baseline` path.
fn load_snapshots(
    files: &[PathBuf],
    parse_remedy: Option<&str>,
) -> Result<Vec<ridl_ir::v2::Package>, ExitCode> {
    let mut packages = Vec::new();
    for file in files {
        match ridl_diff::load_ir_json(file) {
            Ok(package) => packages.push(package),
            Err(err @ ridl_diff::LoadError::Parse(_)) => {
                let remedy = parse_remedy.map_or(String::new(), |remedy| format!("; {remedy}"));
                eprintln!("error: {}: {err}{remedy}", file.display());
                return Err(ExitCode::from(2));
            }
            Err(err) => {
                eprintln!("error: {}: {err}", file.display());
                return Err(ExitCode::from(2));
            }
        }
    }
    Ok(packages)
}

/// Where every interaction, every struct field or union arm (typl §7.4), and
/// every interface shape of the current source tree is declared, so a diff
/// path can be pointed back at the code on the desk.
///
/// The diff engine reads only the IR, which carries no source locations, so the
/// span comes from a separate parse of the same tree. Matching is by name —
/// package, container, member — which is exactly the identity the diff path
/// carries. "Shape" is an `interface` declaration or a service's inline body
/// (ridl §14.0, §14.5); the two are indexed together through
/// `SourceFile::shapes`, because a diff path names either one the same way. A
/// named-form service is indexed as well: its shape-list elements under the
/// interface names its diff paths carry, and the service's dotted name as the
/// fallback for an element that is gone. A struct or union body is indexed
/// separately, from `SourceFile::definitions` (see [`Self::build`]).
#[derive(Default)]
struct DeclIndex {
    /// The text of each indexed file, by path: the renderer needs the text as
    /// well as the path to draw a snippet.
    texts: BTreeMap<String, String>,
    /// `(package, container, member)` to the member's declaration: an
    /// interaction inside an interface body, one element of a named-form
    /// service's shape list (keyed by the interface name the diff path
    /// carries), a struct field or union arm inside a struct or union body
    /// (typl §7.4), or a `reserved` entry in such a body, keyed by the name
    /// it retires, so a removed member's warning points at its tombstone.
    members: BTreeMap<(String, String, String), (String, TextRange)>,
    /// `(package, container)` to the container's declared name. This is the
    /// fallback for a removed member, whose own declaration no longer exists
    /// in the source being checked. A service — inline or named-form — is
    /// keyed by its dotted name, exactly as its diff paths are.
    shapes: BTreeMap<(String, String), (String, TextRange)>,
    /// The package each indexed directory declares, by the directory's path
    /// as [`directory_of`] spells it. A unit's `interfaces.lock` sits in the
    /// unit's manifest directory, so the packages declared in that directory
    /// and under it are the packages of the unit a RIDL-409 belongs to
    /// ([`Self::unit_of_dir`]).
    packages: BTreeMap<String, String>,
}

impl DeclIndex {
    /// Indexes every `.typl`, `.ridl` and `.rsdl` file under
    /// [`index_root`]`(entry)` (an `.rsdl` file declares no shape and no
    /// service, so it adds nothing). A
    /// file that cannot be read is skipped rather than reported: the compile
    /// already ran clean over this tree, so anything unreadable here is outside
    /// what any caller of this index reports — neither the desk check nor the
    /// publication gate. An unreadable *directory* is not skipped in the same sense —
    /// `collect_source_files` fails on the first one it meets, and
    /// `unwrap_or_default` turns that into an empty index rather than a
    /// partial one — but the compile that already succeeded over this tree
    /// makes the case unreachable in practice, which is why it is not
    /// reported here either.
    fn build(entry: &Path) -> Self {
        let mut index = Self::default();
        for file in collect_source_files(&index_root(entry)).unwrap_or_default() {
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
            index.packages.insert(directory_of(&path), package.clone());

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

            // Named-form services (ridl §14.5). `SourceFile::shapes` yields
            // only inline-form services, so without this pass a service-level
            // diff path found nothing and rendered detached. Each listed
            // reference is indexed under its final segment, and the service's
            // dotted name is the fallback for one that is gone from the
            // source.
            for service in source
                .services()
                .filter(|service| service.colon_token().is_some())
            {
                let Some(dotted) = service.name() else {
                    continue;
                };
                let name = dotted.text();
                if name.is_empty() {
                    continue;
                }
                index.shapes.insert(
                    (package.clone(), name.clone()),
                    (path.clone(), dotted.syntax().text_range()),
                );
                for reference in service.shapes() {
                    let Some(final_segment) = final_ident(reference.syntax()) else {
                        continue;
                    };
                    index.members.insert(
                        (package.clone(), name.clone(), final_segment),
                        (path.clone(), reference.syntax().text_range()),
                    );
                }
            }

            // Struct and union bodies (typl §7.4): declaration order is wire
            // identity for a live field or arm, and `diff_composite`
            // addresses each one under `<package>/<name>/<member>` — the
            // same three-part path `span_of` already reads for an
            // interaction. `SourceFile::shapes` does not reach these bodies;
            // it walks interface bodies only, so they are indexed here, from
            // `SourceFile::definitions`. A `reserved` tombstone is indexed
            // under the name it retires, so a member retired in place points
            // at its tombstone; the container's own name goes into `shapes`
            // as the fallback for a member deleted outright, as an interface
            // shape's does.
            for definition in source.definitions() {
                match definition {
                    ridl_syntax::ast::Definition::Struct(def) => {
                        let Some(name_node) = def.name() else {
                            continue;
                        };
                        let Some(name) = name_text(&name_node) else {
                            continue;
                        };
                        index.shapes.insert(
                            (package.clone(), name.clone()),
                            (path.clone(), name_node.syntax().text_range()),
                        );
                        let members = def.members().filter_map(|member| match member {
                            ridl_syntax::ast::StructMember::Field(field) => {
                                let field_name = field.name().and_then(|n| name_text(&n))?;
                                Some((field_name, field.syntax().text_range()))
                            }
                            ridl_syntax::ast::StructMember::Reserved(entry) => {
                                let retired = entry.name().and_then(|n| name_text(&n))?;
                                Some((retired, entry.syntax().text_range()))
                            }
                        });
                        index.record_composite_members(&package, &name, &path, &text, members);
                    }
                    ridl_syntax::ast::Definition::Union(def) => {
                        let Some(name_node) = def.name() else {
                            continue;
                        };
                        let Some(name) = name_text(&name_node) else {
                            continue;
                        };
                        index.shapes.insert(
                            (package.clone(), name.clone()),
                            (path.clone(), name_node.syntax().text_range()),
                        );
                        let arms = def.arms().filter_map(|arm| {
                            let arm_name = arm.name().and_then(|n| name_text(&n))?;
                            Some((arm_name, arm.syntax().text_range()))
                        });
                        let tombstones = def.reserved().filter_map(|entry| {
                            let retired = entry.name().and_then(|n| name_text(&n))?;
                            Some((retired, entry.syntax().text_range()))
                        });
                        index.record_composite_members(
                            &package,
                            &name,
                            &path,
                            &text,
                            arms.chain(tombstones),
                        );
                    }
                    _ => {}
                }
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
                (
                    path.to_string(),
                    declaration_range(member.syntax().text_range(), text),
                ),
            );
        }
    }

    /// Records one struct's fields or one union's arms under `container`
    /// (typl §7.4) — the shared body of the two [`Self::build`] branches, so
    /// a struct and a union are indexed through one path instead of two
    /// near-identical ones.
    fn record_composite_members(
        &mut self,
        package: &str,
        container: &str,
        path: &str,
        text: &str,
        members: impl Iterator<Item = (String, TextRange)>,
    ) {
        for (name, range) in members {
            self.members.insert(
                (package.to_string(), container.to_string(), name),
                (path.to_string(), declaration_range(range, text)),
            );
        }
    }

    /// The span a `<package>/<shape>/<interaction>` or
    /// `<package>/<struct or union>/<member>` diff path points at: the
    /// member's declaration — or its `reserved` tombstone, for a struct
    /// field or union arm retired under its name — the container's name when
    /// the member itself is gone (a removal), and a detached span when
    /// neither is in the source — a detached diagnostic renders as the coded
    /// message alone.
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

    /// The unit whose manifest directory is `dir` — the parent of a lock
    /// file's path — read from `fresh`, the fresh package set: the unit of a
    /// package declared in `dir` or in a directory under it. A unit's tree
    /// holds no other manifest (MANI-013), so every such package belongs to
    /// the one unit. `None` when no indexed file of the fresh set sits there.
    fn unit_of_dir<'a>(&self, dir: &str, fresh: &'a [ridl_ir::v2::Package]) -> Option<&'a str> {
        self.packages
            .iter()
            .filter(|(path, _)| Path::new(path).starts_with(dir))
            .find_map(|(_, name)| fresh.iter().find(|package| package.name == *name))
            .map(ridl_ir::v2::unit_of)
    }

    /// The span of a shape's declared name — an `interface` declaration's
    /// name, or a service's dotted name for its inline shape — or a detached
    /// span when the source does not declare it.
    fn shape_span(&self, package: &str, shape: &str, sources: &mut SourceMap) -> Span {
        let Some((path, range)) = self.shapes.get(&(package.to_string(), shape.to_string())) else {
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

/// The directory tree [`DeclIndex::build`] indexes for `entry`: the root
/// [`ridl_core::load_workspace`] compiles from ([`ridl_core::find_root`]).
/// The compile covers the whole root whatever entry names it, so an entry at
/// a file or a subdirectory would otherwise leave a change in a file above or
/// beside it with a detached span, which no `[lints]` scope reaches. A file
/// with no manifest above it (single-file mode) is indexed alone.
fn index_root(entry: &Path) -> PathBuf {
    let dir = if entry.is_file() {
        entry.parent()
    } else {
        Some(entry)
    };
    dir.and_then(ridl_core::find_root)
        .unwrap_or_else(|| entry.to_path_buf())
}

/// A span pointing at no file at all.
fn detached_span() -> Span {
    Span {
        file: FileId::DETACHED,
        range: TextRange::empty(TextSize::new(0)),
    }
}

/// A declaration's own range, with trailing whitespace trimmed off: a node's
/// range can run to the start of the next line, and an underline that reaches
/// past the declaration reads as if the next one were implicated too. Shared
/// by every member kind the index spans — an interaction, a struct field, and
/// a union arm alike.
fn declaration_range(range: TextRange, text: &str) -> TextRange {
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

/// The final identifier segment of a path node — `DiagBlock` of
/// `fleet.c2.DiagBlock` — under which a listed reference is indexed.
fn final_ident(node: &ridl_syntax::SyntaxNode) -> Option<String> {
    node.descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .filter(|token| token.kind() == ridl_syntax::SyntaxKind::Ident)
        .last()
        .map(|token| token.text().to_string())
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

/// The exit-code rule every `check`/`build` run turns its outcome into: 2 on a
/// bad flag value (ADR-0010 decision 1), 1 when any diagnostic is an error, 0
/// otherwise. Shared by [`finish`] and [`finish_check`]'s JSON arm so the rule
/// is stated once. `check` never reports a bad flag value this way.
fn exit_code(run: &CliRun) -> ExitCode {
    if run.usage_error {
        ExitCode::from(2)
    } else if run.has_error() {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

/// Ends `ridl check`: text renders to stderr through [`finish`]; JSON and
/// SARIF print their contract to stdout and keep the same exit code. The
/// SARIF artifact URIs are relative to the working directory, not to the
/// checked path, so one log has one base (ADR-0024 decision 13). The
/// comparison is lexical: `current_dir` returns the physical path, so an
/// absolute entry that reaches the working directory through a symbolic link
/// is outside it and gives absolute `file://` URIs, while a relative entry is
/// joined onto the working directory and is under it. A working directory that
/// cannot be read gives no base: an absolute source path is an absolute
/// `file://` URI, a relative one stays relative.
fn finish_check(run: CliRun, format: CheckFormat) -> ExitCode {
    match format {
        CheckFormat::Text => finish(Ok(run)),
        CheckFormat::Json => {
            let json = ridl_core::diag::to_json(&run.diagnostics, &run.sources);
            println!(
                "{}",
                serde_json::to_string_pretty(&json).expect("diagnostics serialize")
            );
            exit_code(&run)
        }
        CheckFormat::Sarif => {
            let cwd = std::env::current_dir().ok();
            let log = ridl_core::diag::sarif::to_sarif(
                &run.diagnostics,
                &run.sources,
                cwd.as_deref(),
                env!("CARGO_PKG_VERSION"),
            );
            println!(
                "{}",
                serde_json::to_string_pretty(&log).expect("the SARIF log serializes")
            );
            exit_code(&run)
        }
    }
}

/// Renders a check/build run's diagnostics to stderr and turns the outcome into
/// an exit code: 2 on an I/O error or a bad flag value, 1 when any diagnostic
/// is an error, 0 otherwise.
fn finish(run: std::io::Result<CliRun>) -> ExitCode {
    match run {
        Ok(run) => {
            eprint!("{}", render(&run.diagnostics, &run.sources));
            exit_code(&run)
        }
        Err(err) => {
            eprintln!("error: {err}");
            ExitCode::from(2)
        }
    }
}

/// Formats every `.typl`, `.ridl` and `.rsdl` file under `path`, each parsed
/// under the profile its extension selects.
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
        match format(&text, profile, &FormatOptions::for_path(&file)) {
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

/// Every `.typl`, `.ridl` and `.rsdl` file under `path`: `path` itself when it
/// is a file, otherwise a recursive walk that skips hidden directories.
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
                .is_some_and(|ext| ext == "typl" || ext == "ridl" || ext == "rsdl")
            {
                files.push(child);
            }
        }
    }
    files.sort();
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A history rename that fails after the snapshots moved in leaves no
    /// history of the replaced baseline beside the fresh snapshots.
    #[test]
    fn an_interrupted_publication_leaves_no_replaced_history() {
        let root =
            std::env::temp_dir().join(format!("ridl-publish-interrupted-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let staging = root.join("staging");
        let out_dir = root.join("out");
        std::fs::create_dir_all(&staging).unwrap();
        std::fs::create_dir_all(out_dir.join("a.catalogs").join("blocker")).unwrap();
        for name in ["a.ir.json", "b.ir.json"] {
            std::fs::write(staging.join(name), "{}").unwrap();
        }
        for name in ["a.catalogs", "b.catalogs"] {
            std::fs::write(staging.join(name), "fresh\n").unwrap();
        }
        std::fs::write(out_dir.join("b.ir.json"), "{}").unwrap();
        std::fs::write(out_dir.join("b.catalogs"), "marker\n").unwrap();

        let result = publish_baseline(&staging, &out_dir);

        let left = std::fs::read_to_string(out_dir.join("b.catalogs")).unwrap_or_default();
        let _ = std::fs::remove_dir_all(&root);
        assert!(result.is_err(), "the history rename onto a directory fails");
        assert!(
            !left.contains("marker"),
            "the replaced history is gone: {left:?}"
        );
    }
}
