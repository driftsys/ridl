//! `ridl test` — the property runner (epic E2 story E2.11a, roadmap E2.11).
//!
//! ridl §13 gives one contract clause four executions; this is the third,
//! "property test in CI". A typl range is a specification of a value domain, so
//! it is also a specification of how to sample that domain (ADR-0004 §9) — the
//! E1.18 strategies in `ridl_sem::testgen` turn a checked range into draws, and
//! this module spends them on the contract plane.
//!
//! Three sections per package, in report order:
//!
//! 1. **Range self-corpora.** Every constrained named type has its boundary
//!    corpus run against the constraint validator, and its violation corpus run
//!    against the same validator expecting rejection. This is a self-check of
//!    the toolchain rather than of the model: the corpora come from the
//!    generators and the bounds come from the lowered IR, so a disagreement is a
//!    checker or generator bug and fails the run.
//! 2. **`require` satisfiability sampling.** Every precondition whose reads are
//!    all generatable is evaluated over two sets of inputs: each parameter's
//!    **boundary corpus** — its range endpoints, plus zero when the range spans
//!    it — drawn first and deterministically, then `--samples` random draws.
//!    The two counts are reported apart, so an endpoint hit is distinguishable
//!    from an interior one. Nothing satisfied by either is reported as
//!    `suspect` — a **test-plane finding**, not a compile diagnostic: it is
//!    evidence worth a human's attention and not a rule the language enforces,
//!    so no diagnostic code is burned on it and the run still passes. A clause
//!    reading no parameter has nothing to vary and is evaluated once, reported
//!    as constant rather than as a sample count it did not earn.
//! 3. **`ensure` clauses.** Listed as observer stubs only. A postcondition needs
//!    a `result`, which needs a provider; executing one is the E5 oracle's job.
//!
//! **What section 2 does not try.** Boundary values are zipped across
//! parameters with wrapping, not combined exhaustively: every parameter reaches
//! each of its own endpoints, but no tuple pairs one parameter's endpoint with
//! another's. The corpus is therefore the diagonal, and the tuple count stays
//! linear rather than exponential in the parameter count. A clause satisfied
//! only at an off-diagonal corner — `a == 0 && b == 1000` over two
//! `[0..1000]` parameters — is reachable only if a random draw happens to land
//! there, so it can report `suspect` even though it is satisfiable. The report
//! is honest about what ran; this note is what it does not say. Exhaustive
//! combination is the obvious next lever if that shape turns out to matter.
//!
//! Exit codes: 0 when every run passes, 1 on a range self-corpus failure or an
//! evaluation error, 2 on a compile or I/O error.
//!
//! **Reproducibility.** Each clause is sampled from its own seed, derived by a
//! fixed hash of the package name, the observer id, and the canonical clause
//! text. The seed therefore depends only on the model, never on wall-clock
//! time, machine, or iteration order, so two runs of one workspace produce
//! byte-identical reports and a reported finding can be reproduced.

use std::collections::BTreeMap;
use std::path::Path;
use std::process::ExitCode;

use num_bigint::BigInt;
use num_rational::BigRational;
use proptest::strategy::{Strategy, ValueTree};
use proptest::test_runner::{Config, RngAlgorithm, TestRng, TestRunner};
use ridl_ir::v2;
use ridl_sem::expr::NumericBacking;
use ridl_sem::expr_eval::{EvalEnv, Value, eval_expr, parse_contract_expr};
use ridl_sem::scalar::{ExactValue, FloatRange, IntRange};
use ridl_sem::{Resolution, SymbolKind, testgen};

/// The `ridl test` output format.
#[derive(Clone, Copy, Debug, clap::ValueEnum)]
pub enum TestFormat {
    Text,
    Json,
}

/// The reason a `require` reading the enclosing interface's own signals is not
/// sampled. A signal is live state: it has no value at desk time, and observing
/// one is the E5 observer's job.
const LIVE_STATE_SKIP: &str = "skipped: reads live state — observer territory (E5)";

/// The finding raised when no drawn input satisfies a precondition.
const SUSPECT: &str = "suspect: no sampled input satisfies this precondition";

/// Appended to the finding for a clause reading more than one parameter.
///
/// Boundary values are zipped, not combined, so the corpus never pairs one
/// parameter's endpoint with another's. Without this note a `suspect` on such a
/// clause reads as a claim about the model when it may only be a limit of the
/// search.
const COMBINATION_CAVEAT: &str = "boundary combinations across parameters are not explored, so this may be a \
     limit of the sampling rather than of the model";

// ==========================================================================
// The report
// ==========================================================================

struct PackageReport {
    package: String,
    ranges: Vec<RangeReport>,
    contracts: Vec<ContractReport>,
}

struct RangeReport {
    type_name: String,
    status: RangeStatus,
}

enum RangeStatus {
    /// Every boundary sample was accepted and every violation sample rejected.
    Ok {
        boundary: usize,
        violations: usize,
    },
    Failed(String),
}

struct ContractReport {
    id: String,
    source: String,
    is_ensure: bool,
    status: ContractStatus,
}

enum ContractStatus {
    /// At least one input satisfied the clause. The boundary and random counts
    /// are reported apart so a reader can tell an endpoint hit from an interior
    /// one.
    Ok {
        satisfied_boundary: usize,
        satisfied_random: usize,
        boundary: usize,
        random: usize,
        /// Draws thrown away for landing outside their own range — see
        /// [`draw`]. Non-zero only on a range whose bounds exceed `f64`
        /// precision.
        discarded: usize,
    },
    /// No input satisfied it — neither the injected boundary corpus nor any
    /// random draw. The test-plane finding.
    Suspect {
        boundary: usize,
        random: usize,
        discarded: usize,
        /// How many parameters the clause reads. Above one, the finding carries
        /// the combination caveat: the boundary corpus is the diagonal, so a
        /// clause satisfied only at an off-diagonal corner can land here.
        params: usize,
    },
    /// A clause reading no parameter: one value, not a sample count.
    Constant {
        holds: bool,
    },
    Skipped(String),
    /// Evaluation did not produce a verdict; the run fails.
    Error(String),
    /// An `ensure`, listed and never evaluated.
    ObserverStub,
}

impl ContractStatus {
    fn word(&self) -> &'static str {
        match self {
            ContractStatus::Ok { .. } => "ok",
            ContractStatus::Constant { holds: true } => "constant-true",
            ContractStatus::Constant { holds: false } => "constant-false",
            ContractStatus::Suspect { .. } => "suspect",
            ContractStatus::Skipped(_) => "skipped",
            ContractStatus::Error(_) => "error",
            ContractStatus::ObserverStub => "observer-stub",
        }
    }
}

impl PackageReport {
    /// Whether anything in this package fails the run. A `suspect` finding does
    /// not: it is a report about the model, not a failure of the toolchain.
    fn failed(&self) -> bool {
        self.ranges
            .iter()
            .any(|range| matches!(range.status, RangeStatus::Failed(_)))
            || self
                .contracts
                .iter()
                .any(|contract| matches!(contract.status, ContractStatus::Error(_)))
    }
}

// ==========================================================================
// Entry point
// ==========================================================================

/// Compiles the workspace at `path` and runs the property sections over every
/// package it declares.
pub fn run(path: &Path, samples: usize, format: TestFormat) -> ExitCode {
    // Sampling nothing is a usage error rather than a silent clamp: a run that
    // drew no values would report every sampled clause as unsatisfied and call
    // that a finding, which is worse than refusing the flag.
    if samples == 0 {
        eprintln!("error: `--samples` must be at least 1");
        return ExitCode::from(2);
    }

    let mut db = ridl_core::RidlDatabase::default();
    let output = match ridlc::compile_workspace(&mut db, path) {
        Ok(output) => output,
        Err(err) => {
            eprintln!("error: {}: {err}", path.display());
            return ExitCode::from(2);
        }
    };
    if output
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == ridl_core::diag::Severity::Error)
    {
        eprint!(
            "{}",
            ridl_core::diag::render(&output.diagnostics, &output.sources)
        );
        return ExitCode::from(2);
    }

    let names = Names::of(&output);
    // `checked` and `resolutions` are filled in one loop and are the same
    // length; `zip` therefore pairs each package with its own resolved name
    // view, which a lookup by package name could not do (see `Home`).
    let reports: Vec<PackageReport> = output
        .checked
        .iter()
        .zip(&output.resolutions)
        .map(|(checked, resolution)| {
            run_package(
                &Home {
                    ir: &checked.ir,
                    resolution,
                },
                &names,
                samples,
            )
        })
        .collect();

    match format {
        TestFormat::Text => print!("{}", render_text(&reports)),
        TestFormat::Json => println!("{}", render_json(&reports)),
    }

    if reports.iter().any(PackageReport::failed) {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

fn run_package(home: &Home, names: &Names, samples: usize) -> PackageReport {
    let package = home.ir;
    let vocabulary = names.vocabulary(home);
    let mut ranges = Vec::new();
    for decl in &package.decls {
        let Some(v2::decl::Kind::TypeDef(type_def)) = &decl.kind else {
            continue;
        };
        if let Some(status) = check_range(type_def) {
            ranges.push(RangeReport {
                type_name: decl.name.clone(),
                status,
            });
        }
    }

    // Every interface body in the package, which is NOT the same set as
    // `package.interfaces`: a `service` declared with an inline shape carries a
    // full `Interface` inside its own `shape` oneof, and the checker lowers its
    // `require`/`ensure` clauses into real contracts with observer ids. Walking
    // only `package.interfaces` would report a green run over untested
    // contracts — the one failure this command must not have. A service that
    // names an interface instead needs nothing here: the target is already in
    // `package.interfaces`, and running it twice would report it twice.
    let mut contracts = Vec::new();
    let inline_shapes =
        package
            .services
            .iter()
            .filter_map(|service| match service.shape.as_ref()? {
                v2::service::Shape::Inline(interface) => Some(interface),
                v2::service::Shape::InterfaceRef(_) => None,
            });
    for interface in package.interfaces.iter().chain(inline_shapes) {
        for interaction in &interface.interactions {
            let (params, clauses) = match &interaction.kind {
                Some(v2::decl::Kind::CommandDef(command)) => (&command.params, &command.contracts),
                Some(v2::decl::Kind::QueryDef(query)) => (&query.params, &query.contracts),
                _ => continue,
            };
            for clause in clauses {
                contracts.push(run_contract(
                    home,
                    names,
                    &vocabulary,
                    params,
                    clause,
                    samples,
                ));
            }
        }
    }

    PackageReport {
        package: package.name.clone(),
        ranges,
        contracts,
    }
}

// ==========================================================================
// Section 1 — the range self-corpora
// ==========================================================================

/// Runs the boundary and violation corpora of one constrained named type
/// against the constraint validator.
///
/// Returns `None` for a type this section does not cover: a range needs both
/// bounds and a derived numeric width, which is exactly the set of types the
/// E1.18 generators sample. A string's length bounds and pattern are a
/// different constraint shape with no value corpus behind them.
fn check_range(type_def: &v2::TypeDef) -> Option<RangeStatus> {
    let constraint = type_def.constraint.as_ref()?;
    let min = ExactValue::parse(constraint.min.as_deref()?)?;
    let max = ExactValue::parse(constraint.max.as_deref()?)?;

    // Both corpora come from the E1.18 generators rather than being built here:
    // the point of the section is that the generators and the checker agree, so
    // a corpus this file computed for itself would only ever confirm its own
    // arithmetic.
    let (boundary, violations) = match type_def.width.as_ref()? {
        v2::type_def::Width::IntWidth(_) => {
            let range = IntRange {
                min: min.clone(),
                max: max.clone(),
            };
            (
                testgen::boundary_values(&range)
                    .into_iter()
                    .map(integer)
                    .collect::<Vec<_>>(),
                testgen::violations(&range)
                    .into_iter()
                    .map(integer)
                    .collect::<Vec<_>>(),
            )
        }
        v2::type_def::Width::FloatWidth(_) => {
            let range = FloatRange {
                min: min.clone(),
                max: max.clone(),
                step: constraint.step.as_deref().and_then(ExactValue::parse),
            };
            (
                testgen::float_boundary_values(&range),
                testgen::float_violations(&range),
            )
        }
    };

    for value in &boundary {
        if !accepts(value, &min, &max) {
            return Some(RangeStatus::Failed(format!(
                "boundary value {} is rejected by the range [{}..{}]",
                value.to_decimal_string(),
                min.to_decimal_string(),
                max.to_decimal_string()
            )));
        }
    }
    for value in &violations {
        if accepts(value, &min, &max) {
            return Some(RangeStatus::Failed(format!(
                "violation value {} is accepted by the range [{}..{}]",
                value.to_decimal_string(),
                min.to_decimal_string(),
                max.to_decimal_string()
            )));
        }
    }
    Some(RangeStatus::Ok {
        boundary: boundary.len(),
        violations: violations.len(),
    })
}

/// The constraint validator — **the checker's own**
/// ([`ridl_sem::scalar::range_accepts`]), not a reimplementation of it. The
/// checker validates every declared init and constant against a range through
/// this same function, which is what lets a bug in it surface here as a failed
/// corpus run instead of being duplicated on both sides and cancelling out.
fn accepts(value: &ExactValue, min: &ExactValue, max: &ExactValue) -> bool {
    ridl_sem::scalar::range_accepts(value, Some(min), Some(max))
}

fn integer(value: i64) -> ExactValue {
    ExactValue(BigRational::from_integer(BigInt::from(value)))
}

// ==========================================================================
// Sections 2 and 3 — the contract clauses
// ==========================================================================

fn run_contract(
    home: &Home,
    names: &Names,
    vocabulary: &BTreeMap<String, Value>,
    params: &[v2::Param],
    clause: &v2::Contract,
    samples: usize,
) -> ContractReport {
    let is_ensure = clause.kind == v2::ContractKind::Ensure as i32;
    let report = |status| ContractReport {
        id: clause.observer_id.clone(),
        source: clause.source.clone(),
        is_ensure,
        status,
    };

    // Section 3: an `ensure` is listed, never run. Without a provider there is
    // no `result` to bind, so there is nothing to evaluate (E5's oracle).
    if is_ensure {
        return report(ContractStatus::ObserverStub);
    }

    if !clause.signal_refs.is_empty() {
        return report(ContractStatus::Skipped(LIVE_STATE_SKIP.to_string()));
    }

    // Every parameter the clause reads must be drawable, or the clause cannot
    // be sampled at all.
    let mut generators = Vec::new();
    for name in &clause.param_refs {
        let Some(param) = params.iter().find(|param| &param.name == name) else {
            return report(ContractStatus::Skipped(format!(
                "skipped: `{name}` is not a parameter of this interaction"
            )));
        };
        match names.generator_for(home, param) {
            Some(generator) => generators.push((name.clone(), generator)),
            None => {
                return report(ContractStatus::Skipped(format!(
                    "skipped: `{name}` has no generatable range"
                )));
            }
        }
    }

    let Some(expr) = parse_contract_expr(&clause.source) else {
        return report(ContractStatus::Error(format!(
            "the canonical clause text `{}` does not parse back",
            clause.source
        )));
    };

    // A clause that reads no parameter has nothing to vary: evaluating it N
    // times would repeat one answer N times and report `256/256`, which implies
    // a search that never happened. It is evaluated once and reported as the
    // constant it is.
    if generators.is_empty() {
        let env = EvalEnv {
            params: &[],
            result: None,
            consts: &|name: &str| vocabulary.get(name).cloned(),
        };
        return match eval_expr(&expr, &env) {
            Ok(Value::Bool(true)) => report(ContractStatus::Constant { holds: true }),
            Ok(Value::Bool(false)) => report(ContractStatus::Constant { holds: false }),
            Ok(_) => report(ContractStatus::Error(
                "the clause did not evaluate to a boolean".to_string(),
            )),
            Err(err) => report(ContractStatus::Error(format!(
                "{err} while evaluating `{}`",
                clause.source
            ))),
        };
    }

    let mut runner = TestRunner::new_with_rng(
        Config::default(),
        TestRng::from_seed(
            RngAlgorithm::ChaCha,
            &seed(&home.ir.name, &clause.observer_id, &clause.source),
        ),
    );

    let consts = |name: &str| vocabulary.get(name).cloned();

    // The boundary tuples come first, deterministically, before any random
    // draw. A uniform draw explores endpoints far too rarely to be relied on —
    // 256 draws over `[0..1000]` hit either endpoint only about a fifth of the
    // time — so a perfectly satisfiable `c == 1000` would otherwise be reported
    // as unsatisfiable. No sample count fixes that; injecting the corpus does.
    //
    // Parameters are zipped with wrapping rather than combined exhaustively:
    // every parameter reaches each of its own endpoints, and the tuple count
    // stays linear instead of exponential in the parameter count.
    let corpora: Vec<Vec<Value>> = generators
        .iter()
        .map(|(_, generator)| generator.boundary_corpus())
        .collect();
    let boundary_tuples = corpora.iter().map(Vec::len).max().unwrap_or(0);

    let mut satisfied_boundary = 0usize;
    let mut satisfied_random = 0usize;
    let mut drawn_random = 0usize;
    let mut discarded = 0usize;
    for index in 0..(boundary_tuples + samples) {
        let is_boundary = index < boundary_tuples;
        let mut bindings = Vec::with_capacity(generators.len());
        for (position, (name, generator)) in generators.iter().enumerate() {
            let value = if is_boundary {
                corpora
                    .get(position)
                    .filter(|corpus| !corpus.is_empty())
                    .map(|corpus| corpus[index % corpus.len()].clone())
            } else {
                draw(generator, &mut runner)
            };
            match value {
                Some(value) => bindings.push((name.clone(), value)),
                None if is_boundary => {
                    // A boundary corpus is computed exactly and is always in
                    // range, so an absent value here is a real defect.
                    return report(ContractStatus::Error(format!(
                        "cannot bind the boundary value for `{name}`"
                    )));
                }
                None => break,
            }
        }
        // `draw` discards a value that landed outside its own range (the f64
        // round-trip on a very tight range). The tuple is incomplete, so it is
        // skipped rather than evaluated against a missing binding — and rather
        // than reported as an error, since nothing is wrong with the model.
        if bindings.len() != generators.len() {
            discarded += 1;
            continue;
        }
        if !is_boundary {
            drawn_random += 1;
        }
        let env = EvalEnv {
            params: &bindings,
            result: None,
            consts: &consts,
        };
        match eval_expr(&expr, &env) {
            Ok(Value::Bool(true)) => {
                if is_boundary {
                    satisfied_boundary += 1;
                } else {
                    satisfied_random += 1;
                }
            }
            Ok(Value::Bool(false)) => {}
            Ok(_) => {
                return report(ContractStatus::Error(
                    "the clause did not evaluate to a boolean".to_string(),
                ));
            }
            Err(err) => {
                return report(ContractStatus::Error(format!(
                    "{err} while evaluating `{}`",
                    clause.source
                )));
            }
        }
    }

    // The reported counts are what actually RAN, so a run that discarded draws
    // does not claim to have tried them.
    if satisfied_boundary + satisfied_random == 0 {
        report(ContractStatus::Suspect {
            boundary: boundary_tuples,
            random: drawn_random,
            discarded,
            params: generators.len(),
        })
    } else {
        report(ContractStatus::Ok {
            satisfied_boundary,
            satisfied_random,
            boundary: boundary_tuples,
            random: drawn_random,
            discarded,
        })
    }
}

/// How to draw a value for one parameter, when its declared type is a named
/// type carrying a numeric range.
enum Generator {
    Int(IntRange),
    Float(FloatRange),
    /// `ridl.std.Duration` — the range in its declared `ms` domain. See
    /// [`DURATION`] for why this is not simply a float.
    Duration(FloatRange),
}

impl Generator {
    /// The values this parameter is always tried at, before any random draw:
    /// its range's boundary corpus, plus zero when the range spans it.
    ///
    /// Zero earns its place because it is the value contracts most often turn
    /// on — a guard against a zero divisor, a "must be positive" precondition —
    /// and a range like `[-10..10]` reaches it only by chance otherwise.
    fn boundary_corpus(&self) -> Vec<Value> {
        let (mut corpus, min, max) = match self {
            Generator::Int(range) => (
                testgen::boundary_values(range)
                    .into_iter()
                    .map(integer)
                    .collect::<Vec<_>>(),
                &range.min,
                &range.max,
            ),
            Generator::Float(range) | Generator::Duration(range) => (
                testgen::float_boundary_values(range),
                &range.min,
                &range.max,
            ),
        };
        let zero = ExactValue(BigRational::from_integer(BigInt::from(0)));
        if ridl_sem::scalar::range_accepts(&zero, Some(min), Some(max))
            && !corpus.iter().any(|held| held.0 == zero.0)
        {
            corpus.push(zero);
        }
        corpus.into_iter().map(|value| self.bind(value)).collect()
    }

    /// One value of this generator's own domain as the [`Value`] the evaluator
    /// binds. The domain is a property of the declared type, not of the number:
    /// a `ridl.std.Duration` becomes a [`Value::Dur`], everything else a
    /// [`Value::Num`] carrying its backing.
    fn bind(&self, drawn: ExactValue) -> Value {
        match self {
            Generator::Int(_) => Value::Num(drawn, NumericBacking::Integer),
            Generator::Float(_) => Value::Num(drawn, NumericBacking::Float),
            Generator::Duration(_) => Value::Dur(milliseconds_to_micros(drawn)),
        }
    }

    fn range(&self) -> (&ExactValue, &ExactValue) {
        match self {
            Generator::Int(range) => (&range.min, &range.max),
            Generator::Float(range) | Generator::Duration(range) => (&range.min, &range.max),
        }
    }
}

/// Draws one value, exactly. An integer draw is exact already; a float draw
/// comes back as `f64` from the E1.18 strategy and is carried into the exact
/// domain through its shortest round-tripping decimal, which is a finite
/// decimal and therefore an exact rational.
///
/// **Every drawn value is checked against the range it was drawn from**, with
/// the same `range_accepts` the checker and the range self-corpora use, so "in
/// range" means one thing across the toolchain. The float path is not
/// range-preserving: it rounds both bounds to `f64` to build the strategy, and
/// reconstructing the drawn value can land just outside the declared interval
/// when the bounds need more than about fifteen significant digits. Presenting
/// such a value as an ordinary sample inverts verdicts — a `require v < min`,
/// which no legal value satisfies, is reported satisfied — so an out-of-range
/// draw is discarded here instead. The guard costs one comparison and turns a
/// silently wrong answer into a missing sample.
fn draw(generator: &Generator, runner: &mut TestRunner) -> Option<Value> {
    let drawn = match generator {
        Generator::Int(range) => {
            integer(testgen::int_values(range).new_tree(runner).ok()?.current())
        }
        Generator::Float(range) | Generator::Duration(range) => {
            let value = testgen::float_values(range)
                .new_tree(runner)
                .ok()?
                .current();
            if !value.is_finite() {
                return None;
            }
            ExactValue::parse(&format!("{value}"))?
        }
    };
    let (min, max) = generator.range();
    ridl_sem::scalar::range_accepts(&drawn, Some(min), Some(max)).then(|| generator.bind(drawn))
}

// ==========================================================================
// Name resolution — the checker's view, not one package's `decls`
// ==========================================================================

/// The one inhabitant of the duration domain (expr-core §5.1).
///
/// The checker hard-codes this same reference
/// (`Checker::expr_type_of_path_in`): a parameter or constant of this type is
/// an `ExprType::Duration` and not an ordinary numeric, however its `ms`
/// backing reads. The runner has to agree, because the evaluator refuses to
/// order a [`Value::Dur`] against a [`Value::Num`] — binding a `Duration`
/// parameter as a number would turn `require window > 0ms`, a clause the
/// checker accepts, into an evaluation error and exit 1.
const DURATION: (&str, &str) = ("ridl.std", "Duration");

/// A `ridl.std.Duration` value, declared in milliseconds, as the exact
/// microsecond count [`Value::Dur`] holds (expr-core §7). Scaling exactly in
/// rationals, so no sampled value is rounded on the way in.
fn milliseconds_to_micros(value: ExactValue) -> ExactValue {
    ExactValue(value.0 * BigRational::from_integer(BigInt::from(1000)))
}

/// The package whose clauses are being run: its lowered IR and the resolved
/// local name view the checker built for it, paired as `ridlc` returns them.
///
/// The IR is carried rather than looked up by name, because **a package name is
/// not a key**. Two workspace members may declare the same `[package] name`,
/// which the toolchain currently accepts with no diagnostic at all, and
/// `resolve_package` binds a package's own declarations straight from its own
/// files before it considers any import (`resolve.rs`, step 1) — a bare
/// reference never goes through `package_of`. Resolving one by name would hand
/// the second member the first member's declarations and report the second
/// member's clauses against bounds it never declared.
struct Home<'a> {
    ir: &'a v2::Package,
    resolution: &'a Resolution,
}

/// Every name a contract clause can reach, resolved the way the checker
/// resolved it.
///
/// `packages` indexes the lowered IR of every checked package **plus
/// `ridl.std`** by package name, and serves **cross-package references only**:
/// a lowered reference is canonical — bare within its own package,
/// `package.Name` across packages (the checker's `canonical_ref`) — so
/// splitting one at its last dot names the package to look in, with no search
/// and no chance of matching a same-named declaration elsewhere. A reference
/// naming the enclosing package is answered from [`Home::ir`] instead, never
/// from this index. Duplicate names resolve first-wins here, which is exactly
/// what [`package_of`](ridl_core::package::package_of) does and therefore what
/// the checker itself did when it lowered the reference: the runner's job is to
/// agree with the checker, not to improve on it.
///
/// `ridl.std` is in the index because it is deliberately absent from the
/// workspace's package list while every package implicitly imports all of it
/// (typl §3.2), and `Duration` and `Timestamp` both carry generatable ranges.
///
/// The **written** name of a value — what a clause's canonical source text
/// spells, and what the evaluator asks the environment for — is resolved
/// through [`Home::resolution`] and never through this index. Scanning packages
/// for a matching declared name instead would bind the wrong declaration under
/// an import alias (`import fleet.legacy.DoorFault as LegacyFault` is keyed
/// `LegacyFault`, while the symbol still names `fleet.legacy.DoorFault`) and
/// would have to guess between two packages exporting one name.
///
/// Reading the resolver's map is also what makes the runner agree with the
/// checker on the cases the resolver decides quietly. Two *imports* competing
/// for one local name are a hard error (TYPL-006), but an import colliding with
/// a **local** declaration is not diagnosed at all — the local declaration wins
/// and the import is silently shadowed. The runner inherits that outcome
/// because it reads the same map, rather than reproducing a rule it would have
/// to keep in step by hand.
struct Names<'a> {
    packages: BTreeMap<&'a str, &'a v2::Package>,
}

impl<'a> Names<'a> {
    fn of(output: &'a ridlc::WorkspaceOutput) -> Names<'a> {
        // First-wins on a duplicate package name — see the type's own note: a
        // cross-package reference was lowered through `package_of`, which picks
        // the first match by name, so this index reproduces the checker's
        // choice. A package's OWN declarations never come through here.
        let mut packages: BTreeMap<&str, &v2::Package> = BTreeMap::new();
        for checked in &output.checked {
            packages.entry(&checked.ir.name).or_insert(&checked.ir);
        }
        packages
            .entry(&output.std_ir.name)
            .or_insert(&output.std_ir);
        Names { packages }
    }

    /// Splits a canonical IR reference into the package that declares it and
    /// the declared name, reading a bare reference as `home`'s own. A declared
    /// name is a single identifier, so the last dot is always the separator.
    fn split<'r>(home: &'r str, reference: &'r str) -> (&'r str, &'r str) {
        match reference.rsplit_once('.') {
            Some((package, name)) => (package, name),
            None => (home, reference),
        }
    }

    /// The declaration `name` in `package`, read from the enclosing package's
    /// own IR when `package` names it and from the workspace index otherwise.
    fn decl(&self, home: &Home<'a>, package: &str, name: &str) -> Option<&'a v2::decl::Kind> {
        let target = if package == home.ir.name {
            home.ir
        } else {
            *self.packages.get(package)?
        };
        target
            .decls
            .iter()
            .find(|decl| decl.name == name)?
            .kind
            .as_ref()
    }

    /// The constants and enum members a clause in `home` may name, keyed by the
    /// spelling the clause writes: a constant under its local name, an enum
    /// member under the dotted `Enum.MEMBER` form the evaluator asks for.
    ///
    /// Built from `home`'s resolved symbols — its own declarations, `ridl.std`,
    /// and its imports under whatever local name they were bound to — and each
    /// symbol's value is then read out of the IR of the package that **declares**
    /// it. A constant's own declared type is likewise resolved in its declaring
    /// package, since that is the view its lowered `type_ref` is canonical in.
    fn vocabulary(&self, home: &Home<'a>) -> BTreeMap<String, Value> {
        let mut bound = BTreeMap::new();
        for (local, symbol) in &home.resolution.symbols {
            match symbol.kind {
                SymbolKind::Const => {
                    let Some(v2::decl::Kind::ConstDef(const_def)) =
                        self.decl(home, &symbol.package, &symbol.name)
                    else {
                        continue;
                    };
                    let Some(value) = self.const_value(home, &symbol.package, const_def) else {
                        continue;
                    };
                    bound.insert(local.clone(), value);
                }
                SymbolKind::Enum => {
                    let Some(v2::decl::Kind::EnumDef(enum_def)) =
                        self.decl(home, &symbol.package, &symbol.name)
                    else {
                        continue;
                    };
                    // The enum's identity is its fully qualified reference, not
                    // the local spelling: the evaluator compares two members by
                    // it, so an aliased import must still be the same enum.
                    let reference = ridl_sem::expr::qualified_ref(symbol);
                    for member in &enum_def.values {
                        bound.insert(
                            format!("{local}.{}", member.name),
                            Value::EnumVal(reference.clone(), member.value),
                        );
                    }
                }
                _ => {}
            }
        }
        bound
    }

    /// One constant's value in the domain its declared type puts it in, or
    /// `None` for a constant with no numeric value — a string, bytes, or regex
    /// constant, over which no operator of the guaranteed subset works.
    ///
    /// `declaring` is the package that **declares** the constant: its lowered
    /// `type_ref` is canonical in that package's view, not in the view of the
    /// package whose clause names it.
    fn const_value(
        &self,
        home: &Home<'a>,
        declaring: &str,
        const_def: &v2::ConstDef,
    ) -> Option<Value> {
        let value = ExactValue::parse(&const_def.value)?;
        // A constant with no declared type, or one whose type resolves to
        // nothing, is typed by its written spelling — exactly as a bare literal
        // is (expr-core §5.2).
        let spelled = if const_def.value.contains('.') {
            NumericBacking::Float
        } else {
            NumericBacking::Integer
        };
        let Some(type_ref) = const_def.type_ref.as_deref() else {
            return Some(Value::Num(value, spelled));
        };
        match type_ref {
            "integer" => return Some(Value::Num(value, NumericBacking::Integer)),
            "float" => return Some(Value::Num(value, NumericBacking::Float)),
            _ => {}
        }
        let (package, name) = Names::split(declaring, type_ref);
        // A `ridl.std.Duration` constant belongs to the duration domain exactly
        // as a parameter of that type does — see `DURATION`. Without this a
        // clause comparing one against a duration literal is an evaluation
        // error and exit 1, on a workspace the checker accepts.
        if (package, name) == DURATION {
            return Some(Value::Dur(milliseconds_to_micros(value)));
        }
        let backing = match self.decl(home, package, name) {
            Some(v2::decl::Kind::TypeDef(type_def)) => match type_def.width.as_ref() {
                Some(v2::type_def::Width::IntWidth(_)) => NumericBacking::Integer,
                Some(v2::type_def::Width::FloatWidth(_)) => NumericBacking::Float,
                None => spelled,
            },
            _ => spelled,
        };
        Some(Value::Num(value, backing))
    }

    /// How to draw values for one parameter, or `None` when its declared type
    /// carries no numeric range to draw from.
    fn generator_for(&self, home: &Home<'a>, param: &v2::Param) -> Option<Generator> {
        let Some(v2::field_type::Kind::Named(reference)) = param.r#type.as_ref()?.kind.as_ref()
        else {
            return None;
        };
        let (package, name) = Names::split(&home.ir.name, reference);
        let Some(v2::decl::Kind::TypeDef(type_def)) = self.decl(home, package, name) else {
            return None;
        };
        let constraint = type_def.constraint.as_ref()?;
        let min = ExactValue::parse(constraint.min.as_deref()?)?;
        let max = ExactValue::parse(constraint.max.as_deref()?)?;
        let step = constraint.step.as_deref().and_then(ExactValue::parse);
        if (package, name) == DURATION {
            return Some(Generator::Duration(FloatRange { min, max, step }));
        }
        match type_def.width.as_ref()? {
            v2::type_def::Width::IntWidth(_) => Some(Generator::Int(IntRange { min, max })),
            v2::type_def::Width::FloatWidth(_) => {
                Some(Generator::Float(FloatRange { min, max, step }))
            }
        }
    }
}

// ==========================================================================
// Seeding
// ==========================================================================

/// The 32-byte ChaCha seed for one clause, derived from the model alone.
///
/// FNV-1a is used rather than the standard-library hasher because the seed must
/// be stable across processes, machines, and toolchain versions: a finding is
/// only worth reporting if the reader can reproduce it.
fn seed(package: &str, observer_id: &str, source: &str) -> [u8; 32] {
    let mut seed = [0u8; 32];
    for (lane, chunk) in seed.chunks_mut(8).enumerate() {
        let material = format!("{package}\u{0}{observer_id}\u{0}{source}\u{0}{lane}");
        chunk.copy_from_slice(&fnv1a(material.as_bytes()).to_le_bytes());
    }
    seed
}

fn fnv1a(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

// ==========================================================================
// Rendering
// ==========================================================================

fn render_text(reports: &[PackageReport]) -> String {
    let mut out = String::new();
    for report in reports {
        out.push_str(&format!("package {}\n", report.package));

        out.push_str("  ranges\n");
        if report.ranges.is_empty() {
            out.push_str("    (no constrained named types)\n");
        }
        for range in &report.ranges {
            match &range.status {
                RangeStatus::Ok {
                    boundary,
                    violations,
                } => out.push_str(&format!(
                    "    {}  ok — {boundary} boundary accepted, {violations} violations rejected\n",
                    range.type_name
                )),
                RangeStatus::Failed(why) => {
                    out.push_str(&format!("    {}  FAILED — {why}\n", range.type_name));
                }
            }
        }

        for (heading, ensure) in [("requires", false), ("ensures", true)] {
            out.push_str(&format!("  {heading}\n"));
            let mut any = false;
            for contract in report
                .contracts
                .iter()
                .filter(|contract| contract.is_ensure == ensure)
            {
                any = true;
                out.push_str(&format!("    {}  {}\n", contract.id, describe(contract)));
            }
            if !any {
                out.push_str("    (none)\n");
            }
        }

        let summary = Summary::of(report);
        out.push_str(&format!("  {}\n", summary.line()));
        if let Some(warning) = summary.warning() {
            out.push_str(&format!("  {warning}\n"));
        }
    }
    out
}

/// What the run actually did, counted from the per-clause reports.
///
/// A per-clause `skipped` line is easy to miss, and a reader who sees an exit
/// code of 0 and no summary reasonably concludes the contracts were tested. A
/// package whose every `require` reads a signal, or whose every parameter is a
/// string or a composite, skips all of them and still exits 0. That must not
/// read as success, so the count is stated once per package and, when nothing
/// ran, said plainly.
///
/// This is derived from the existing reports; it adds no state and changes no
/// clause's status.
struct Summary {
    requires: usize,
    evaluated: usize,
    suspect: usize,
    /// Clauses that read no parameter and evaluated false. Counted apart
    /// because they are findings, not passes: an unsatisfiable constant
    /// precondition is the same news as a `suspect`, reached by a different
    /// route. Folding them into `evaluated` alone would let a package whose
    /// every clause is unsatisfiable print a summary that reads like a pass.
    constant_false: usize,
    skipped: usize,
    errors: usize,
    ensures: usize,
}

impl Summary {
    fn of(report: &PackageReport) -> Summary {
        let mut summary = Summary {
            requires: 0,
            evaluated: 0,
            suspect: 0,
            constant_false: 0,
            skipped: 0,
            errors: 0,
            ensures: 0,
        };
        for contract in &report.contracts {
            if contract.is_ensure {
                summary.ensures += 1;
                continue;
            }
            summary.requires += 1;
            match &contract.status {
                // A constant clause is evaluated once rather than sampled, but
                // it did produce a verdict, so it counts as tested.
                ContractStatus::Ok { .. } | ContractStatus::Constant { holds: true } => {
                    summary.evaluated += 1;
                }
                ContractStatus::Constant { holds: false } => {
                    summary.evaluated += 1;
                    summary.constant_false += 1;
                }
                ContractStatus::Suspect { .. } => {
                    summary.evaluated += 1;
                    summary.suspect += 1;
                }
                ContractStatus::Skipped(_) => summary.skipped += 1,
                ContractStatus::Error(_) => summary.errors += 1,
                ContractStatus::ObserverStub => {}
            }
        }
        summary
    }

    fn line(&self) -> String {
        let mut parts = vec![format!(
            "requires: {} total, {} evaluated",
            self.requires, self.evaluated
        )];
        if self.suspect > 0 {
            parts.push(format!("{} suspect", self.suspect));
        }
        if self.constant_false > 0 {
            parts.push(format!("{} constant-false", self.constant_false));
        }
        if self.skipped > 0 {
            parts.push(format!("{} skipped", self.skipped));
        }
        if self.errors > 0 {
            parts.push(format!("{} errored", self.errors));
        }
        format!(
            "summary — {}; ensures: {} listed",
            parts.join(", "),
            self.ensures
        )
    }

    /// The line that fires when a package declares preconditions and none of
    /// them were evaluated. A package with no `require` at all is silent: there
    /// is nothing it failed to test.
    fn warning(&self) -> Option<String> {
        (self.requires > 0 && self.evaluated == 0).then(|| {
            format!(
                "WARNING: no require clause was evaluated — this run tested no \
                 precondition ({} skipped of {})",
                self.skipped, self.requires
            )
        })
    }

    /// The machine-readable flag a CI consumer keys on. Same condition as
    /// [`Summary::warning`]: a reader of the JSON has exactly the problem a
    /// reader of the text does.
    fn nothing_evaluated(&self) -> bool {
        self.requires > 0 && self.evaluated == 0
    }
}

fn describe(contract: &ContractReport) -> String {
    match &contract.status {
        ContractStatus::Ok {
            satisfied_boundary,
            satisfied_random,
            boundary,
            random,
            discarded,
        } => {
            format!(
                "ok — {satisfied_boundary} boundary + {satisfied_random} random of {} satisfied{}  ({})",
                boundary + random,
                discarded_note(*discarded),
                contract.source
            )
        }
        ContractStatus::Suspect {
            boundary,
            random,
            discarded,
            params,
        } => {
            format!(
                "{SUSPECT} — 0/{} ({boundary} boundary + {random} random){}{}  ({})",
                boundary + random,
                discarded_note(*discarded),
                if *params > 1 {
                    format!("; {COMBINATION_CAVEAT}")
                } else {
                    String::new()
                },
                contract.source
            )
        }
        ContractStatus::Constant { holds } => format!(
            "{} — reads no parameter, evaluated once  ({})",
            if *holds {
                "ok, constant"
            } else {
                "constant FALSE"
            },
            contract.source
        ),
        ContractStatus::Skipped(why) => format!("{why}  ({})", contract.source),
        ContractStatus::Error(why) => format!("ERROR — {why}"),
        ContractStatus::ObserverStub => {
            format!("observer stub — not evaluated  ({})", contract.source)
        }
    }
}

/// The trailing note for draws thrown away as out of range, or nothing at all
/// when none were — the common case by far.
fn discarded_note(discarded: usize) -> String {
    if discarded == 0 {
        String::new()
    } else {
        format!(", {discarded} discarded as out of range")
    }
}

fn render_json(reports: &[PackageReport]) -> String {
    let packages: Vec<serde_json::Value> = reports
        .iter()
        .map(|report| {
            let ranges: Vec<serde_json::Value> = report
                .ranges
                .iter()
                .map(|range| match &range.status {
                    // The corpus sizes are reported, not just the verdict: a
                    // section that ran no corpus at all would otherwise be
                    // indistinguishable from one that ran and passed.
                    RangeStatus::Ok {
                        boundary,
                        violations,
                    } => serde_json::json!({
                        "type": range.type_name,
                        "status": "ok",
                        "boundary": boundary,
                        "violations": violations,
                    }),
                    RangeStatus::Failed(why) => serde_json::json!({
                        "type": range.type_name,
                        "status": "failed",
                        "detail": why,
                    }),
                })
                .collect();
            let contracts: Vec<serde_json::Value> = report
                .contracts
                .iter()
                .map(|contract| {
                    let (satisfied, samples, boundary, random, discarded) = match &contract.status {
                        ContractStatus::Ok {
                            satisfied_boundary,
                            satisfied_random,
                            boundary,
                            random,
                            discarded,
                        } => (
                            Some(satisfied_boundary + satisfied_random),
                            Some(boundary + random),
                            Some(*boundary),
                            Some(*random),
                            Some(*discarded),
                        ),
                        ContractStatus::Suspect {
                            boundary,
                            random,
                            discarded,
                            ..
                        } => (
                            Some(0),
                            Some(boundary + random),
                            Some(*boundary),
                            Some(*random),
                            Some(*discarded),
                        ),
                        _ => (None, None, None, None, None),
                    };
                    let detail = match &contract.status {
                        ContractStatus::Skipped(why) => Some(why.clone()),
                        ContractStatus::Error(why) => Some(why.clone()),
                        ContractStatus::Suspect { params, .. } if *params > 1 => {
                            Some(format!("{SUSPECT}; {COMBINATION_CAVEAT}"))
                        }
                        ContractStatus::Suspect { .. } => Some(SUSPECT.to_string()),
                        _ => None,
                    };
                    serde_json::json!({
                        "id": contract.id,
                        "status": contract.status.word(),
                        "satisfied": satisfied,
                        "samples": samples,
                        "boundary_samples": boundary,
                        "random_samples": random,
                        "discarded_samples": discarded,
                        "source": contract.source,
                        "detail": detail,
                    })
                })
                .collect();
            // The same statement the text summary makes. A machine reading the
            // report cannot see the per-clause `skipped` lines any more easily
            // than a human can, so `nothing_evaluated` is the single field a CI
            // consumer can key on to tell "all preconditions hold" apart from
            // "no precondition was tested".
            let summary = Summary::of(report);
            serde_json::json!({
                "package": report.package,
                "contracts": contracts,
                "ranges": ranges,
                "summary": {
                    "requires_total": summary.requires,
                    "requires_evaluated": summary.evaluated,
                    "requires_suspect": summary.suspect,
                    "requires_constant_false": summary.constant_false,
                    "requires_skipped": summary.skipped,
                    "requires_errored": summary.errors,
                    "ensures_listed": summary.ensures,
                    "nothing_evaluated": summary.nothing_evaluated(),
                },
            })
        })
        .collect();
    serde_json::Value::Array(packages).to_string()
}
