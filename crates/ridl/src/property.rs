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
//!    all generatable is evaluated over N drawn parameter tuples, and the
//!    satisfied count is reported. Zero satisfied out of N is reported as
//!    `suspect` — a **test-plane finding**, not a compile diagnostic: it is
//!    evidence worth a human's attention and not a rule the language enforces,
//!    so no diagnostic code is burned on it and the run still passes.
//! 3. **`ensure` clauses.** Listed as observer stubs only. A postcondition needs
//!    a `result`, which needs a provider; executing one is the E5 oracle's job.
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
    /// At least one drawn input satisfied the clause.
    Ok {
        satisfied: usize,
        samples: usize,
    },
    /// No drawn input satisfied it — the test-plane finding.
    Suspect {
        samples: usize,
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
    // At least one draw: a run that samples nothing would report every clause
    // as unsatisfied and call it a finding.
    let samples = samples.max(1);

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

    let mut contracts = Vec::new();
    for interface in &package.interfaces {
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

    let (boundary, violations) = match type_def.width.as_ref()? {
        v2::type_def::Width::IntWidth(_) => {
            // The shipped E1.18 corpora: `min`, `min+1`, `max-1`, `max` must be
            // accepted, `min-1` and `max+1` rejected.
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
            // `testgen` ships boundary and violation corpora for integer ranges
            // only, so the float analogue is built here in the same shape and
            // in the same exact domain: the two bounds must be accepted, and
            // one step outside either must be rejected. A range with no step
            // steps by one.
            let step = constraint
                .step
                .as_deref()
                .and_then(ExactValue::parse)
                .map(|step| step.0)
                .unwrap_or_else(|| BigRational::from_integer(BigInt::from(1)));
            (
                vec![min.clone(), max.clone()],
                vec![ExactValue(&min.0 - &step), ExactValue(&max.0 + &step)],
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

/// The constraint validator: a closed range over exact values, which is the
/// same membership rule the checker applies to a declared init.
fn accepts(value: &ExactValue, min: &ExactValue, max: &ExactValue) -> bool {
    value.0 >= min.0 && value.0 <= max.0
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

    let mut runner = TestRunner::new_with_rng(
        Config::default(),
        TestRng::from_seed(
            RngAlgorithm::ChaCha,
            &seed(&package.name, &clause.observer_id, &clause.source),
        ),
    );

    let consts = |name: &str| vocabulary.get(name).cloned();
    let mut satisfied = 0usize;
    for _ in 0..samples {
        let mut bindings = Vec::with_capacity(generators.len());
        for (name, generator) in &generators {
            match draw(generator, &mut runner) {
                Some(value) => bindings.push((name.clone(), value)),
                None => {
                    return report(ContractStatus::Error(format!(
                        "cannot draw a value for `{name}`"
                    )));
                }
            }
        }
        let env = EvalEnv {
            params: &bindings,
            result: None,
            consts: &consts,
        };
        match eval_expr(&expr, &env) {
            Ok(Value::Bool(true)) => satisfied += 1,
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

    if satisfied == 0 {
        report(ContractStatus::Suspect { samples })
    } else {
        report(ContractStatus::Ok { satisfied, samples })
    }
}

/// How to draw a value for one parameter, when its declared type is a named
/// type carrying a numeric range.
enum Generator {
    Int(IntRange),
    Float(FloatRange),
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
fn draw(generator: &Generator, runner: &mut TestRunner) -> Option<Value> {
    match generator {
        Generator::Int(range) => {
            let value = testgen::int_values(range).new_tree(runner).ok()?.current();
            Some(Value::Num(integer(value), NumericBacking::Integer))
        }
        Generator::Float(range) => {
            let value = testgen::float_values(range)
                .new_tree(runner)
                .ok()?
                .current();
            if !value.is_finite() {
                return None;
            }
            Some(Value::Num(
                ExactValue::parse(&format!("{value}"))?,
                NumericBacking::Float,
            ))
        }
    }
}

/// The named types, constants, and enum members a clause may reference.
///
/// Constants are bound under their bare name and enum members under the dotted
/// `Enum.MEMBER` spelling the evaluator asks for.
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
        ContractStatus::Ok { satisfied, samples } => {
            format!(
                "ok — {satisfied}/{samples} satisfied  ({})",
                contract.source
            )
        }
        ContractStatus::Suspect { samples } => {
            format!("{SUSPECT} — 0/{samples}  ({})", contract.source)
        }
        ContractStatus::Skipped(why) => format!("{why}  ({})", contract.source),
        ContractStatus::Error(why) => format!("ERROR — {why}"),
        ContractStatus::ObserverStub => {
            format!("observer stub — not evaluated  ({})", contract.source)
        }
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
                    RangeStatus::Ok { .. } => serde_json::json!({
                        "type": range.type_name,
                        "status": "ok",
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
                    let (satisfied, samples) = match &contract.status {
                        ContractStatus::Ok { satisfied, samples } => {
                            (Some(*satisfied), Some(*samples))
                        }
                        ContractStatus::Suspect { samples } => (Some(0), Some(*samples)),
                        _ => (None, None),
                    };
                    let detail = match &contract.status {
                        ContractStatus::Skipped(why) => Some(why.clone()),
                        ContractStatus::Error(why) => Some(why.clone()),
                        ContractStatus::Suspect { .. } => Some(SUSPECT.to_string()),
                        _ => None,
                    };
                    serde_json::json!({
                        "id": contract.id,
                        "status": contract.status.word(),
                        "satisfied": satisfied,
                        "samples": samples,
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
