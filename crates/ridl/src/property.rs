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
use ridl_sem::testgen;

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

    let reports: Vec<PackageReport> = output
        .checked
        .iter()
        .map(|checked| run_package(&checked.ir, samples))
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

fn run_package(package: &v2::Package, samples: usize) -> PackageReport {
    let vocabulary = vocabulary(package);
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
                contracts.push(run_contract(package, &vocabulary, params, clause, samples));
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
    package: &v2::Package,
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
        match generator_for(package, param) {
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
            &seed(&package.name, &clause.observer_id, &clause.source),
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
}

impl Generator {
    /// The values this parameter is always tried at, before any random draw:
    /// its range's boundary corpus, plus zero when the range spans it.
    ///
    /// Zero earns its place because it is the value contracts most often turn
    /// on — a guard against a zero divisor, a "must be positive" precondition —
    /// and a range like `[-10..10]` reaches it only by chance otherwise.
    fn boundary_corpus(&self) -> Vec<Value> {
        let (mut corpus, min, max, backing) = match self {
            Generator::Int(range) => (
                testgen::boundary_values(range)
                    .into_iter()
                    .map(integer)
                    .collect::<Vec<_>>(),
                &range.min,
                &range.max,
                NumericBacking::Integer,
            ),
            Generator::Float(range) => (
                testgen::float_boundary_values(range),
                &range.min,
                &range.max,
                NumericBacking::Float,
            ),
        };
        let zero = ExactValue(BigRational::from_integer(BigInt::from(0)));
        if ridl_sem::scalar::range_accepts(&zero, Some(min), Some(max))
            && !corpus.iter().any(|held| held.0 == zero.0)
        {
            corpus.push(zero);
        }
        corpus
            .into_iter()
            .map(|value| Value::Num(value, backing))
            .collect()
    }
}

fn generator_for(package: &v2::Package, param: &v2::Param) -> Option<Generator> {
    let Some(v2::field_type::Kind::Named(reference)) = param.r#type.as_ref()?.kind.as_ref() else {
        return None;
    };
    let type_def = named_type(package, reference)?;
    let constraint = type_def.constraint.as_ref()?;
    let min = ExactValue::parse(constraint.min.as_deref()?)?;
    let max = ExactValue::parse(constraint.max.as_deref()?)?;
    match type_def.width.as_ref()? {
        v2::type_def::Width::IntWidth(_) => Some(Generator::Int(IntRange { min, max })),
        v2::type_def::Width::FloatWidth(_) => Some(Generator::Float(FloatRange {
            min,
            max,
            step: constraint.step.as_deref().and_then(ExactValue::parse),
        })),
    }
}

/// Draws one value, exactly. An integer draw is exact already; a float draw
/// comes back as `f64` from the E1.18 strategy and is carried into the exact
/// domain through its shortest round-tripping decimal, which is a finite
/// decimal and therefore an exact rational.
///
/// **Every drawn value is checked against the range it was drawn from.** The
/// float path is not range-preserving: it rounds both bounds to `f64` to build
/// the strategy, and reconstructing the drawn value can land just outside the
/// declared interval when the bounds need more than about fifteen significant
/// digits. Presenting such a value as an ordinary sample inverts verdicts — a
/// `require v < min`, which no legal value satisfies, is reported satisfied —
/// so an out-of-range draw is discarded here instead. The guard costs one
/// comparison and turns a silently wrong answer into a missing sample.
fn draw(generator: &Generator, runner: &mut TestRunner) -> Option<Value> {
    match generator {
        Generator::Int(range) => {
            let value = testgen::int_values(range).new_tree(runner).ok()?.current();
            let value = integer(value);
            in_range(value, &range.min, &range.max, NumericBacking::Integer)
        }
        Generator::Float(range) => {
            let value = testgen::float_values(range)
                .new_tree(runner)
                .ok()?
                .current();
            if !value.is_finite() {
                return None;
            }
            let value = ExactValue::parse(&format!("{value}"))?;
            in_range(value, &range.min, &range.max, NumericBacking::Float)
        }
    }
}

/// The drawn value as a [`Value`], or `None` when it fell outside the range it
/// was drawn from. Uses the same `range_accepts` the checker and the range
/// self-corpora use, so "in range" means one thing across the toolchain.
fn in_range(
    value: ExactValue,
    min: &ExactValue,
    max: &ExactValue,
    backing: NumericBacking,
) -> Option<Value> {
    if ridl_sem::scalar::range_accepts(&value, Some(min), Some(max)) {
        Some(Value::Num(value, backing))
    } else {
        None
    }
}

/// The named types, constants, and enum members a clause may reference.
///
/// Constants are bound under their bare name and enum members under the dotted
/// `Enum.MEMBER` spelling the evaluator asks for.
///
/// **Known limitation — single package only.** This walks one package's own
/// `decls`, while the checker builds the same vocabulary from
/// `resolution.symbols`, which includes imports. A clause naming a constant
/// imported from another package therefore finds no binding here, evaluation
/// fails with an unbound reference, and the run exits 1 — on a workspace
/// `ridl check` accepts. Exit 1 means "the toolchain disagrees with itself", so
/// this reports a toolchain fault for a legal model. It fails loudly rather
/// than silently, which is the safe direction, but it is still wrong. The fix
/// is to hand this runner the checker's resolution rather than a single
/// package, which also closes the cross-package gap in [`named_type`]; both are
/// one debt item.
fn vocabulary(package: &v2::Package) -> BTreeMap<String, Value> {
    let mut bound = BTreeMap::new();
    for decl in &package.decls {
        match &decl.kind {
            Some(v2::decl::Kind::ConstDef(const_def)) => {
                let Some(value) = ExactValue::parse(&const_def.value) else {
                    continue;
                };
                let backing = const_def
                    .type_ref
                    .as_deref()
                    .and_then(|reference| const_backing(package, reference))
                    // A constant with no resolvable named type is typed by its
                    // spelling, exactly as a bare literal is.
                    .unwrap_or(if const_def.value.contains('.') {
                        NumericBacking::Float
                    } else {
                        NumericBacking::Integer
                    });
                bound.insert(decl.name.clone(), Value::Num(value, backing));
            }
            Some(v2::decl::Kind::EnumDef(enum_def)) => {
                let reference = format!("{}.{}", package.name, decl.name);
                for member in &enum_def.values {
                    bound.insert(
                        format!("{}.{}", decl.name, member.name),
                        Value::EnumVal(reference.clone(), member.value),
                    );
                }
            }
            _ => {}
        }
    }
    bound
}

fn const_backing(package: &v2::Package, reference: &str) -> Option<NumericBacking> {
    match reference {
        "integer" => return Some(NumericBacking::Integer),
        "float" => return Some(NumericBacking::Float),
        _ => {}
    }
    match named_type(package, reference)?.width.as_ref()? {
        v2::type_def::Width::IntWidth(_) => Some(NumericBacking::Integer),
        v2::type_def::Width::FloatWidth(_) => Some(NumericBacking::Float),
    }
}

/// The named type `reference` resolves to within `package`.
///
/// A canonical reference may be qualified (`veh.common.Speed`) or bare
/// (`Speed`); only the declared name is matched, and a type declared in another
/// package is simply not found here — its parameters are then reported as not
/// generatable rather than sampled against the wrong bounds.
fn named_type<'a>(package: &'a v2::Package, reference: &str) -> Option<&'a v2::TypeDef> {
    let name = reference.rsplit('.').next().unwrap_or(reference);
    if reference.contains('.') && !reference.starts_with(&format!("{}.", package.name)) {
        return None;
    }
    package.decls.iter().find_map(|decl| match &decl.kind {
        Some(v2::decl::Kind::TypeDef(type_def)) if decl.name == name => Some(type_def),
        _ => None,
    })
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
    }
    out
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
            serde_json::json!({
                "package": report.package,
                "contracts": contracts,
                "ranges": ranges,
            })
        })
        .collect();
    serde_json::Value::Array(packages).to_string()
}
