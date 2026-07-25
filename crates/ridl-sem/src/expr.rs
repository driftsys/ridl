//! The contract-expression checker — the guaranteed subset of the `expr` core
//! (`docs/specification/expr-core-specification.md`, ridl §13; epic E2 story
//! E2.4).
//!
//! Three things live here, over the task 4 expression AST:
//!
//! - [`check_contract_expr`] — the typing rules of the expr-core specification
//!   §5 over the reference environment of §6. Every form outside the subset is
//!   RIDL-306 with a message naming it (§8); an `ensure` that never reads
//!   `result` is RIDL-305 (warning).
//! - [`canonical_expr_text`] — the one-line rendering lowered into IR
//!   `Contract.source`, stable across source formatting.
//! - [`collect_refs`] — the resolved reads of one clause, the input to the
//!   task 12 observer stubs.

use std::collections::HashMap;

use ridl_core::diag::{DiagCode, Diagnostic, FileId, Severity, Span};
use ridl_syntax::SyntaxKind;
use ridl_syntax::ast::{self, AstNode};
use rowan::TextRange;

use crate::resolve::{Resolution, Symbol, SymbolKind};
use crate::scalar::ExactValue;

// ==========================================================================
// Types
// ==========================================================================

/// The primitive a numeric type is backed by (typl §4). Carried by the numeric
/// domain because the `%` rule is a rule about the operand's **type** and not
/// about how a literal happens to be spelled (expr-core §5.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NumericBacking {
    Integer,
    Float,
}

/// The five type domains a subset expression inhabits (expr-core §5.1), plus
/// the carrier for a declared value whose type is outside them.
///
/// Every named reference inside a domain is the **fully qualified**
/// `package.Name` form ([`qualified_ref`]) — never a bare name and never an
/// import alias — so two references unify exactly when they name one
/// declaration (typl §5.7 nominal typing).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExprType {
    Boolean,
    /// A numeric value: the canonical named-type reference, or the empty
    /// string for a bare numeric literal, which unifies with any numeric
    /// operand (expr-core §5.2). The backing decides the `%` rule.
    Numeric(String, NumericBacking),
    Duration,
    /// An enum-typed value; the canonical enum reference.
    EnumType(String),
    /// A tuple-typed value — a tuple-returning query's `result`. The string is
    /// the field encoding built by [`ExprType::tuple`]; it is opaque outside
    /// this module.
    Tuple(String),
    /// A declared value whose type is outside the five domains — a
    /// string-backed type, a struct, a union, a stream. It is carried rather
    /// than dropped so that naming it reports the real form (expr-core §8
    /// wants a message naming the offending form) instead of claiming the name
    /// does not resolve. The string describes the declared type.
    Unsupported(String),
}

impl ExprType {
    /// The tuple domain over `fields`, in declaration order. A field whose
    /// declared type is outside the five domains is carried as untypeable
    /// (`None`) — naming it in an expression is RIDL-306, not a missing field.
    ///
    /// The encoding is `name=CODE` pairs joined by `;`, where `CODE` is `B`
    /// (boolean), `Ni:ref`/`Nf:ref` (integer- and float-backed numeric — `ref`
    /// may be empty), `D` (duration), `E:ref` (enum), or `U:declared` (outside
    /// the subset). A tuple field is a named type, so the encoding never
    /// nests.
    pub fn tuple(fields: &[(String, Option<ExprType>)]) -> ExprType {
        let encoded = fields
            .iter()
            .map(|(name, field)| format!("{name}={}", encode_field(field.as_ref())))
            .collect::<Vec<_>>()
            .join(";");
        ExprType::Tuple(encoded)
    }

    /// The human rendering used in diagnostics.
    pub fn describe(&self) -> String {
        match self {
            ExprType::Boolean => "boolean".to_string(),
            ExprType::Numeric(name, _) if name.is_empty() => "a numeric literal".to_string(),
            ExprType::Numeric(name, _) => format!("`{name}`"),
            ExprType::Duration => "duration".to_string(),
            ExprType::EnumType(name) => format!("`{name}`"),
            ExprType::Tuple(_) => "a tuple".to_string(),
            ExprType::Unsupported(declared) => format!("`{declared}`"),
        }
    }

    /// The type of the tuple field `name` — `None` when the tuple has no such
    /// field. A field outside the subset decodes as
    /// [`ExprType::Unsupported`].
    fn tuple_field(encoded: &str, name: &str) -> Option<ExprType> {
        encoded
            .split(';')
            .filter(|entry| !entry.is_empty())
            .find_map(|entry| {
                let (field, code) = entry.split_once('=')?;
                (field == name).then(|| decode_field(code))?
            })
    }
}

fn encode_field(field: Option<&ExprType>) -> String {
    match field {
        Some(ExprType::Boolean) => "B".to_string(),
        Some(ExprType::Numeric(name, NumericBacking::Integer)) => format!("Ni:{name}"),
        Some(ExprType::Numeric(name, NumericBacking::Float)) => format!("Nf:{name}"),
        Some(ExprType::Duration) => "D".to_string(),
        Some(ExprType::EnumType(name)) => format!("E:{name}"),
        Some(ExprType::Unsupported(declared)) => format!("U:{declared}"),
        // A tuple field is a named type, so a nested tuple is unreachable
        // through the grammar; it encodes as unsupported rather than panicking.
        Some(ExprType::Tuple(_)) => "U:a tuple".to_string(),
        None => "U:a type outside the guaranteed subset".to_string(),
    }
}

fn decode_field(code: &str) -> Option<ExprType> {
    match code {
        "B" => Some(ExprType::Boolean),
        "D" => Some(ExprType::Duration),
        _ => match code.split_once(':') {
            Some(("Ni", name)) => {
                Some(ExprType::Numeric(name.to_string(), NumericBacking::Integer))
            }
            Some(("Nf", name)) => Some(ExprType::Numeric(name.to_string(), NumericBacking::Float)),
            Some(("E", name)) => Some(ExprType::EnumType(name.to_string())),
            Some(("U", declared)) => Some(ExprType::Unsupported(declared.to_string())),
            _ => None,
        },
    }
}

/// One enum type as a contract expression needs it: the canonical reference
/// plus the member names `Enum.MEMBER` may name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnumDecl {
    pub reference: String,
    pub members: Vec<String>,
}

impl EnumDecl {
    fn has_member(&self, name: &str) -> bool {
        self.members.iter().any(|member| member == name)
    }
}

/// The package-level declarations a contract expression may name, **resolved**
/// (expr-core §6 items 4 and 5).
///
/// A [`Resolution`] maps a name to a symbol — its kind and defining package —
/// which is not enough to type a contract: a constant's declared type and an
/// enum's member list both live in the declaration. Carrying them resolved is
/// what lets a constant type nominally (typl §5.7) and an unknown enum member
/// be rejected.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ContractVocabulary {
    /// Constants by the name the file binds, with the type of the declared
    /// value.
    pub consts: HashMap<String, ExprType>,
    /// Enum types by the name the file binds.
    pub enums: HashMap<String, EnumDecl>,
}

/// The reference environment one `require`/`ensure` clause resolves against
/// (expr-core §6), in resolution order.
///
/// The clause kind is carried by the shape of the scope, which is exactly how
/// §6 scopes the two clauses: an `ensure` has `result` set and no signals (the
/// ridl §13 table scopes it to `result` and parameters); a `require` has no
/// `result` and carries the enclosing interface's own signals.
///
/// A parameter, signal, or `result` whose declared type is outside the five
/// domains is present and typed [`ExprType::Unsupported`] rather than absent,
/// so naming it reports what it is instead of claiming it does not resolve.
pub struct ContractScope<'a> {
    pub params: &'a [(String, ExprType)],
    /// `Some` for an `ensure` on a query — the clause-kind discriminator.
    pub result: Option<ExprType>,
    /// The enclosing interface's own signals — populated for a `require` only.
    pub signals: &'a [(String, ExprType)],
    /// Constants and enum types, resolved to their declarations.
    pub vocabulary: &'a ContractVocabulary,
    /// The package view, for classifying a name that is none of the above.
    pub resolution: &'a Resolution,
}

/// The references one clause reads, resolved — the input to the task 12
/// observer stubs. Each list keeps first-mention order and holds no duplicate.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExprRefs {
    pub params: Vec<String>,
    pub signals: Vec<String>,
    pub uses_result: bool,
    pub consts: Vec<String>,
    /// The enum types named as the head of an `Enum.MEMBER` access. Not a
    /// *read* — an enum type has no value, so it is absent from the observer
    /// stub's read set — but it is a package declaration the clause names, and
    /// the clause is published verbatim, so the visibility check
    /// ([`crate::check`], TYPL-005) needs it. Kept here rather than recomputed
    /// by the caller so that one walk decides what a clause binds.
    pub enum_types: Vec<String>,
}

/// The canonical reference of a resolved symbol: always the fully qualified
/// `package.Name`. The checker's IR-facing `canonical_ref` shortens a
/// same-package reference to its bare name; type identity here must not depend
/// on which package is being checked, so this form is used throughout
/// [`ExprType`].
pub fn qualified_ref(symbol: &Symbol) -> String {
    format!("{}.{}", symbol.package, symbol.name)
}

// ==========================================================================
// Type checking
// ==========================================================================

/// Type-checks one `require`/`ensure` expression against the expr-core
/// specification §5–§6.
///
/// Returns the root type — `Some(ExprType::Boolean)` for a clause that
/// checks, `None` when any error was raised — and the diagnostics: RIDL-306
/// (error) for every form outside the guaranteed subset (§8), RIDL-305
/// (warning) for an `ensure` that never references `result`.
///
/// The diagnostics carry [`FileId::DETACHED`]: this checker sees one
/// expression and not the file it came from, so the caller stamps the real
/// file id on the way into its own diagnostic list.
///
/// Names resolve against [`ContractScope`], whose vocabulary carries resolved
/// declarations: a constant is typed by its declaration and unifies nominally
/// like any other named value, and an enum member is checked against the
/// enum's member list.
pub fn check_contract_expr(
    expr: &ast::Expr,
    scope: &ContractScope,
) -> (Option<ExprType>, Vec<Diagnostic>) {
    let mut diagnostics = Vec::new();
    let inferred = infer(expr, scope, &mut diagnostics);
    let root = match inferred {
        Some(ExprType::Boolean) => Some(ExprType::Boolean),
        Some(other) => {
            error(
                &mut diagnostics,
                expr.syntax().text_range(),
                format!(
                    "contract expression is {}, not a predicate — the root of a `require`/`ensure` must be boolean (expr-core §5.3)",
                    other.describe()
                ),
            );
            None
        }
        None => None,
    };
    // RIDL-305 reads a well-typed clause; a broken one is not also suspicious.
    if root.is_some() && scope.result.is_some() && !collect_refs(expr, scope).uses_result {
        diagnostics.push(diagnostic(
            DiagCode::RIDL_305,
            Severity::Warning,
            expr.syntax().text_range(),
            "`ensure` references no `result` — a postcondition that observes nothing of the result (ridl §13)".to_string(),
        ));
    }
    (root, diagnostics)
}

fn infer(expr: &ast::Expr, scope: &ContractScope, out: &mut Vec<Diagnostic>) -> Option<ExprType> {
    match expr {
        ast::Expr::Literal(literal) => infer_literal(literal, out),
        ast::Expr::Paren(paren) => infer(&paren.inner()?, scope, out),
        ast::Expr::Path(path) => infer_path(path, scope, out),
        ast::Expr::Prefix(prefix) => infer_prefix(prefix, scope, out),
        ast::Expr::Member(member) => infer_member(member, scope, out),
        ast::Expr::Binary(binary) => infer_binary(binary, scope, out),
    }
}

fn infer_literal(literal: &ast::LiteralExpr, out: &mut Vec<Diagnostic>) -> Option<ExprType> {
    // A missing token is a parse error, already reported.
    let token = literal.token()?;
    match token.kind() {
        SyntaxKind::IntNumber => Some(ExprType::Numeric(String::new(), NumericBacking::Integer)),
        SyntaxKind::FloatNumber => Some(ExprType::Numeric(String::new(), NumericBacking::Float)),
        SyntaxKind::Duration => Some(ExprType::Duration),
        SyntaxKind::TrueKw | SyntaxKind::FalseKw => Some(ExprType::Boolean),
        SyntaxKind::String | SyntaxKind::Regex => {
            error(
                out,
                token.text_range(),
                "string or regex literal in a contract expression — no operator of the guaranteed subset works over strings or bytes (expr-core §5.3)".to_string(),
            );
            None
        }
        _ => None,
    }
}

fn infer_path(
    path: &ast::PathExpr,
    scope: &ContractScope,
    out: &mut Vec<Diagnostic>,
) -> Option<ExprType> {
    let token = path.name_token()?;
    let name = token.text();
    let range = token.text_range();
    if let Some(found) = lookup(scope.params, name) {
        return in_domain(found, name, "parameter", range, out);
    }
    if name == "result" {
        return match &scope.result {
            Some(found) => in_domain(found.clone(), name, "query result", range, out),
            None => {
                error(
                    out,
                    range,
                    "`result` is not in scope here — `result` exists in an `ensure` on a query (expr-core §6)".to_string(),
                );
                None
            }
        };
    }
    if let Some(found) = lookup(scope.signals, name) {
        return in_domain(found, name, "signal", range, out);
    }
    if let Some(found) = scope.vocabulary.consts.get(name) {
        // A constant carries its declared type, so it unifies nominally like
        // any other named value (expr-core §5.2, typl §5.7).
        return in_domain(found.clone(), name, "constant", range, out);
    }
    match scope.resolution.symbols.get(name) {
        Some(symbol) if symbol.kind == SymbolKind::Enum => {
            error(
                out,
                range,
                format!(
                    "`{name}` names an enum type, which has no value — write `{name}.MEMBER` (expr-core §5.3)"
                ),
            );
            None
        }
        Some(_) => {
            error(
                out,
                range,
                format!(
                    "`{name}` names a declaration that is not a value of the guaranteed subset — a contract references parameters, `result`, the interface's own signals, constants, and enum types (expr-core §6)"
                ),
            );
            None
        }
        None => {
            error(out, range, unresolved_message(name, scope));
            None
        }
    }
}

/// Reports a reference whose declared type is outside the five domains
/// (expr-core §5.1) and passes every other type through. The reference is in
/// the environment — it is the **type** that is outside the subset — so the
/// message names the declared form rather than claiming the name is unknown.
fn in_domain(
    found: ExprType,
    name: &str,
    noun: &str,
    range: TextRange,
    out: &mut Vec<Diagnostic>,
) -> Option<ExprType> {
    match found {
        ExprType::Unsupported(declared) => {
            error(
                out,
                range,
                format!(
                    "`{name}` is a {noun} of type `{declared}`, which is outside the guaranteed subset — a contract expression types boolean, numeric, duration, enum, and tuple values (expr-core §5.1)"
                ),
            );
            None
        }
        typed => Some(typed),
    }
}

/// The message for a name the environment does not bind. In an `ensure` the
/// most common cause is a signal read, which ridl §13 scopes out, so the
/// message says so.
fn unresolved_message(name: &str, scope: &ContractScope) -> String {
    if scope.result.is_some() {
        format!(
            "`{name}` does not resolve here — an `ensure` sees `result`, the parameters, constants, and enum types only; the interface's own signals are readable in a `require` (ridl §13, expr-core §6)"
        )
    } else {
        format!(
            "`{name}` does not resolve here — a `require` sees the parameters, the interface's own signals, constants, and enum types (expr-core §6)"
        )
    }
}

fn infer_prefix(
    prefix: &ast::PrefixExpr,
    scope: &ContractScope,
    out: &mut Vec<Diagnostic>,
) -> Option<ExprType> {
    let operand = infer(&prefix.operand()?, scope, out);
    let token = prefix.op_token()?;
    let operand = operand?;
    match token.kind() {
        SyntaxKind::Bang => {
            if operand == ExprType::Boolean {
                Some(ExprType::Boolean)
            } else {
                error(
                    out,
                    prefix.syntax().text_range(),
                    format!(
                        "`!` requires a boolean operand, found {} (expr-core §5.3)",
                        operand.describe()
                    ),
                );
                None
            }
        }
        SyntaxKind::Minus => {
            if matches!(operand, ExprType::Numeric(..)) {
                Some(operand)
            } else {
                error(
                    out,
                    prefix.syntax().text_range(),
                    format!(
                        "unary `-` requires a numeric operand, found {} (expr-core §5.3)",
                        operand.describe()
                    ),
                );
                None
            }
        }
        _ => None,
    }
}

fn infer_member(
    member: &ast::MemberExpr,
    scope: &ContractScope,
    out: &mut Vec<Diagnostic>,
) -> Option<ExprType> {
    let base = member.base()?;
    let token = member.member_token()?;
    let name = token.text().to_string();
    let range = member.syntax().text_range();

    // The narrowing the parser leaves open: `postfix_expr` accepts any ident
    // after `.`, so `pkg.Name.MEMBER` parses into nested member access,
    // although expr-core §3.1 admits a tuple field or an enum member only.
    if is_camel_case(&name) {
        error(
            out,
            range,
            format!(
                "`.{name}` names a type, not a member — a qualified path such as `pkg.Name.MEMBER` is not expressible in a contract expression; references resolve through the file's imports (expr-core §3.1)"
            ),
        );
        return None;
    }

    if is_screaming_snake(&name) {
        if let Some(declared) = enum_type_head(&base, scope) {
            // An unknown member is as much a broken reference as an unknown
            // identifier: nothing downstream — the task 12 observers, the
            // E2.11 property runner — has a value to bind to it.
            if !declared.has_member(&name) {
                error(
                    out,
                    range,
                    format!(
                        "`{}` has no member `{name}` (expr-core §5.3)",
                        declared.reference
                    ),
                );
                return None;
            }
            return Some(ExprType::EnumType(declared.reference.clone()));
        }
        // The left of an enum member access is not an enum type name. The
        // base may itself be the offending form — `pkg.Name.MEMBER` reports
        // its CamelCase segment — so it is inferred first and its own
        // diagnostic stands alone.
        let reported = out.len();
        infer(&base, scope, out);
        if out.len() == reported {
            error(
                out,
                range,
                format!(
                    "`.{name}` is an enum member access, which needs an enum type name on the left (expr-core §5.3)"
                ),
            );
        }
        return None;
    }

    match infer(&base, scope, out)? {
        ExprType::Tuple(encoded) => match ExprType::tuple_field(&encoded, &name) {
            Some(field) => in_domain(field, &name, "tuple field", range, out),
            None => {
                error(
                    out,
                    range,
                    format!("the result tuple has no field `{name}` (expr-core §5.3)"),
                );
                None
            }
        },
        other => {
            error(
                out,
                range,
                format!(
                    "field access `.{name}` on {} — the guaranteed subset admits field access on a tuple-typed `result` only (expr-core §5.1)",
                    other.describe()
                ),
            );
            None
        }
    }
}

/// The enum declaration when `base` is a bare name that resolves to an enum
/// type and is not shadowed by a value in scope.
fn enum_type_head<'a>(base: &ast::Expr, scope: &'a ContractScope) -> Option<&'a EnumDecl> {
    let ast::Expr::Path(path) = base else {
        return None;
    };
    let token = path.name_token()?;
    let name = token.text();
    if lookup(scope.params, name).is_some()
        || lookup(scope.signals, name).is_some()
        || scope.vocabulary.consts.contains_key(name)
    {
        return None;
    }
    scope.vocabulary.enums.get(name)
}

fn infer_binary(
    binary: &ast::BinaryExpr,
    scope: &ContractScope,
    out: &mut Vec<Diagnostic>,
) -> Option<ExprType> {
    // Both sides are inferred before either is judged, so one broken operand
    // does not hide a second error.
    let lhs = binary.lhs().and_then(|lhs| infer(&lhs, scope, out));
    let rhs = binary.rhs().and_then(|rhs| infer(&rhs, scope, out));
    let token = binary.op_token()?;
    let op = token.text().to_string();
    let range = binary.syntax().text_range();
    let (lhs, rhs) = (lhs?, rhs?);

    match token.kind() {
        SyntaxKind::PipePipe | SyntaxKind::AmpAmp => {
            if lhs == ExprType::Boolean && rhs == ExprType::Boolean {
                Some(ExprType::Boolean)
            } else {
                error(
                    out,
                    range,
                    format!(
                        "`{op}` requires boolean operands, found {} and {} (expr-core §5.3)",
                        lhs.describe(),
                        rhs.describe()
                    ),
                );
                None
            }
        }
        SyntaxKind::EqEq | SyntaxKind::Neq => {
            let unifies = match (&lhs, &rhs) {
                (ExprType::Boolean, ExprType::Boolean) => true,
                (ExprType::Duration, ExprType::Duration) => true,
                (ExprType::Numeric(left, _), ExprType::Numeric(right, _)) => {
                    unify_numeric(left, right).is_some()
                }
                (ExprType::EnumType(left), ExprType::EnumType(right)) => left == right,
                _ => false,
            };
            if unifies {
                Some(ExprType::Boolean)
            } else {
                error(
                    out,
                    range,
                    mismatch(&op, "operands of one domain", &lhs, &rhs),
                );
                None
            }
        }
        SyntaxKind::Lt | SyntaxKind::Le | SyntaxKind::Gt | SyntaxKind::Ge => {
            let ordered = match (&lhs, &rhs) {
                (ExprType::Duration, ExprType::Duration) => true,
                (ExprType::Numeric(left, _), ExprType::Numeric(right, _)) => {
                    unify_numeric(left, right).is_some()
                }
                _ => false,
            };
            if ordered {
                Some(ExprType::Boolean)
            } else if matches!((&lhs, &rhs), (ExprType::EnumType(_), ExprType::EnumType(_))) {
                error(
                    out,
                    range,
                    format!(
                        "`{op}` orders enum values — enums support `==` and `!=` only (expr-core §5.3)"
                    ),
                );
                None
            } else {
                error(
                    out,
                    range,
                    mismatch(&op, "operands of one ordered domain", &lhs, &rhs),
                );
                None
            }
        }
        SyntaxKind::Plus
        | SyntaxKind::Minus
        | SyntaxKind::Star
        | SyntaxKind::Slash
        | SyntaxKind::Percent => {
            let result = match (&lhs, &rhs) {
                (
                    ExprType::Numeric(left, left_backing),
                    ExprType::Numeric(right, right_backing),
                ) => unify_numeric(left, right)
                    .map(|unified| (unified, unify_backing(*left_backing, *right_backing))),
                _ => None,
            };
            let Some(result) = result else {
                if matches!(lhs, ExprType::Duration) || matches!(rhs, ExprType::Duration) {
                    error(
                        out,
                        range,
                        format!(
                            "`{op}` over a duration — duration supports comparison only in the guaranteed subset (expr-core §5.3)"
                        ),
                    );
                } else {
                    error(
                        out,
                        range,
                        mismatch(&op, "numeric operands of one type", &lhs, &rhs),
                    );
                }
                return None;
            };
            let (reference, backing) = result;
            // The `%` rule is about the operands' declared backing, not about
            // how a literal is spelled: `speed % window` over two float-backed
            // named types is as much an error as `speed % 0.5`.
            if token.kind() == SyntaxKind::Percent && backing != NumericBacking::Integer {
                error(
                    out,
                    range,
                    format!(
                        "`%` requires integer-backed operands, found {} and {} (expr-core §5.3)",
                        lhs.describe(),
                        rhs.describe()
                    ),
                );
                return None;
            }
            Some(ExprType::Numeric(reference, backing))
        }
        _ => None,
    }
}

/// Nominal unification of two numeric references (expr-core §5.2): a bare
/// literal (the empty reference) unifies with any numeric; two named types
/// unify only when they name one declaration.
fn unify_numeric(left: &str, right: &str) -> Option<String> {
    if left.is_empty() {
        Some(right.to_string())
    } else if right.is_empty() || left == right {
        Some(left.to_string())
    } else {
        None
    }
}

fn mismatch(op: &str, expected: &str, lhs: &ExprType, rhs: &ExprType) -> String {
    format!(
        "`{op}` requires {expected}, found {} and {} (expr-core §5.2)",
        lhs.describe(),
        rhs.describe()
    )
}

/// The backing of an arithmetic result: float-backed as soon as either operand
/// is, so `%` rejects a float-backed operand wherever it appears.
fn unify_backing(left: NumericBacking, right: NumericBacking) -> NumericBacking {
    match (left, right) {
        (NumericBacking::Integer, NumericBacking::Integer) => NumericBacking::Integer,
        _ => NumericBacking::Float,
    }
}

fn lookup(bindings: &[(String, ExprType)], name: &str) -> Option<ExprType> {
    bindings
        .iter()
        .find(|(bound, _)| bound == name)
        .map(|(_, found)| found.clone())
}

fn is_screaming_snake(name: &str) -> bool {
    !name.is_empty()
        && name.starts_with(|c: char| c.is_ascii_uppercase())
        && !name.chars().any(|c| c.is_ascii_lowercase())
}

fn is_camel_case(name: &str) -> bool {
    name.starts_with(|c: char| c.is_ascii_uppercase())
        && name.chars().any(|c| c.is_ascii_lowercase())
}

fn error(out: &mut Vec<Diagnostic>, range: TextRange, message: String) {
    out.push(diagnostic(
        DiagCode::RIDL_306,
        Severity::Error,
        range,
        message,
    ));
}

fn diagnostic(code: DiagCode, severity: Severity, range: TextRange, message: String) -> Diagnostic {
    Diagnostic {
        code,
        severity,
        message,
        primary: Span {
            file: FileId::DETACHED,
            range,
        },
        labels: Vec::new(),
        fixits: Vec::new(),
    }
}

// ==========================================================================
// Canonical text
// ==========================================================================

/// The canonical one-line rendering of a contract expression — the text
/// lowered into IR `Contract.source`.
///
/// Single spaces around every binary operator, no space inside parentheses or
/// around `.`, and the minimal parentheses: the written ones are dropped and
/// re-inserted only where precedence or associativity requires them. The
/// rendering depends on the parse tree and not on the written spacing, so
/// reformatting a source file never shows up as a contract edit in
/// `ridl diff`. It is idempotent — re-parsing the rendering and rendering it
/// again yields the same text.
pub fn canonical_expr_text(expr: &ast::Expr) -> String {
    let mut out = String::new();
    render(expr, 0, &mut out);
    out
}

/// Binding power, loosest to tightest (expr-core §3.1).
fn precedence(kind: SyntaxKind) -> u8 {
    match kind {
        SyntaxKind::PipePipe => 1,
        SyntaxKind::AmpAmp => 2,
        SyntaxKind::EqEq
        | SyntaxKind::Neq
        | SyntaxKind::Lt
        | SyntaxKind::Le
        | SyntaxKind::Gt
        | SyntaxKind::Ge => 3,
        SyntaxKind::Plus | SyntaxKind::Minus => 4,
        SyntaxKind::Star | SyntaxKind::Slash | SyntaxKind::Percent => 5,
        _ => 0,
    }
}

const PREFIX_PRECEDENCE: u8 = 6;
const POSTFIX_PRECEDENCE: u8 = 7;
const PRIMARY_PRECEDENCE: u8 = 8;

/// Renders `expr` into `out`, wrapping it in parentheses when its binding
/// power is looser than the `min` the position demands.
fn render(expr: &ast::Expr, min: u8, out: &mut String) {
    match expr {
        // A written parenthesis carries no information the tree does not
        // already hold; the rendering re-derives it.
        ast::Expr::Paren(paren) => {
            if let Some(inner) = paren.inner() {
                render(&inner, min, out);
            }
        }
        ast::Expr::Binary(binary) => {
            let Some(token) = binary.op_token() else {
                return;
            };
            let power = precedence(token.kind());
            let wrap = power < min;
            if wrap {
                out.push('(');
            }
            if let Some(lhs) = binary.lhs() {
                render(&lhs, power, out);
            }
            out.push(' ');
            out.push_str(token.text());
            out.push(' ');
            if let Some(rhs) = binary.rhs() {
                // Every subset binary operator is left-associative, so the
                // right operand needs one more level to stay unparenthesized.
                render(&rhs, power + 1, out);
            }
            if wrap {
                out.push(')');
            }
        }
        ast::Expr::Prefix(prefix) => {
            let Some(token) = prefix.op_token() else {
                return;
            };
            let wrap = PREFIX_PRECEDENCE < min;
            if wrap {
                out.push('(');
            }
            out.push_str(token.text());
            if let Some(operand) = prefix.operand() {
                // A nested prefix keeps its parentheses: `- -a` and `--a` are
                // not the same token sequence.
                render(&operand, POSTFIX_PRECEDENCE, out);
            }
            if wrap {
                out.push(')');
            }
        }
        ast::Expr::Member(member) => {
            let wrap = POSTFIX_PRECEDENCE < min;
            if wrap {
                out.push('(');
            }
            if let Some(base) = member.base() {
                render(&base, POSTFIX_PRECEDENCE, out);
            }
            out.push('.');
            if let Some(token) = member.member_token() {
                out.push_str(token.text());
            }
            if wrap {
                out.push(')');
            }
        }
        ast::Expr::Path(path) => {
            let wrap = PRIMARY_PRECEDENCE < min;
            if wrap {
                out.push('(');
            }
            if let Some(token) = path.name_token() {
                out.push_str(token.text());
            }
            if wrap {
                out.push(')');
            }
        }
        ast::Expr::Literal(literal) => {
            if let Some(token) = literal.token() {
                out.push_str(&normalize_literal(token.kind(), token.text()));
            }
        }
    }
}

/// The canonical spelling of a literal token.
///
/// A numeric literal renders through the exact-scalar representation, so two
/// spellings of one value — `0.0` and `0.00`, `1.50` and `1.5` — render
/// identically and a respelling is not a contract edit (expr-core §7: values
/// are exact rationals). A literal written with a fractional part keeps one:
/// `250.0` does not collapse to `250`, which keeps the rendering close to what
/// the author wrote. A literal the exact parser cannot read is left verbatim.
///
/// Duration literals are not normalized across units — `1s` and `1000ms` are
/// one value but render as written; unifying them is a wider change than this
/// story's canonical-text rule.
fn normalize_literal(kind: SyntaxKind, text: &str) -> String {
    if !matches!(kind, SyntaxKind::IntNumber | SyntaxKind::FloatNumber) {
        return text.to_string();
    }
    let Some(value) = ExactValue::parse(text) else {
        return text.to_string();
    };
    let rendered = value.to_decimal_string();
    if text.contains('.') && !rendered.contains('.') {
        return format!("{rendered}.0");
    }
    rendered
}

// ==========================================================================
// Reference collection
// ==========================================================================

/// The references `expr` reads, resolved against `scope` in the same order
/// [`check_contract_expr`] resolves them. The enum type name of an
/// `Enum.MEMBER` access is not a read and is not collected.
pub fn collect_refs(expr: &ast::Expr, scope: &ContractScope) -> ExprRefs {
    let mut refs = ExprRefs::default();
    walk_refs(expr, scope, &mut refs);
    refs
}

fn walk_refs(expr: &ast::Expr, scope: &ContractScope, refs: &mut ExprRefs) {
    match expr {
        ast::Expr::Literal(_) => {}
        ast::Expr::Paren(paren) => {
            if let Some(inner) = paren.inner() {
                walk_refs(&inner, scope, refs);
            }
        }
        ast::Expr::Prefix(prefix) => {
            if let Some(operand) = prefix.operand() {
                walk_refs(&operand, scope, refs);
            }
        }
        ast::Expr::Binary(binary) => {
            if let Some(lhs) = binary.lhs() {
                walk_refs(&lhs, scope, refs);
            }
            if let Some(rhs) = binary.rhs() {
                walk_refs(&rhs, scope, refs);
            }
        }
        ast::Expr::Member(member) => {
            let Some(base) = member.base() else {
                return;
            };
            let is_enum_access = member
                .member_token()
                .is_some_and(|token| is_screaming_snake(token.text()))
                && enum_type_head(&base, scope).is_some();
            if is_enum_access {
                // The head names an enum type. It is not a read, but it is a
                // package declaration this clause names — recorded for the
                // visibility check.
                if let ast::Expr::Path(path) = &base
                    && let Some(token) = path.name_token()
                {
                    push_once(&mut refs.enum_types, token.text());
                }
            } else {
                walk_refs(&base, scope, refs);
            }
        }
        ast::Expr::Path(path) => {
            let Some(token) = path.name_token() else {
                return;
            };
            let name = token.text();
            if lookup(scope.params, name).is_some() {
                push_once(&mut refs.params, name);
            } else if name == "result" && scope.result.is_some() {
                refs.uses_result = true;
            } else if lookup(scope.signals, name).is_some() {
                push_once(&mut refs.signals, name);
            } else if scope.vocabulary.consts.contains_key(name) {
                push_once(&mut refs.consts, name);
            }
        }
    }
}

fn push_once(list: &mut Vec<String>, name: &str) {
    if !list.iter().any(|held| held == name) {
        list.push(name.to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ridl_core::db::{InputFile, RidlDatabase};
    use ridl_syntax::Profile;
    use std::collections::HashMap;

    /// The `require` expression of `text`, parsed in a minimal ridl file.
    fn parse_expr(text: &str) -> ast::Expr {
        let source = format!("package app\ninterface I {{\n  command c() [ require {text} ]\n}}\n");
        let parse = ridl_syntax::parse(&source, Profile::Ridl);
        parse
            .syntax()
            .descendants()
            .find_map(ast::Attribute::cast)
            .and_then(|attribute| attribute.expr())
            .unwrap_or_else(|| panic!("`{text}` does not parse as a contract expression"))
    }

    /// The resolved vocabulary a contract may name: `MAX_SPEED : Speed`,
    /// `MAX_COUNT : Count` (integer-backed), and the `GearPosition` enum with
    /// its members.
    fn vocabulary() -> ContractVocabulary {
        let mut built = ContractVocabulary::default();
        built.consts.insert("MAX_SPEED".to_string(), speed());
        built.consts.insert("MAX_COUNT".to_string(), count());
        built.enums.insert(
            "GearPosition".to_string(),
            EnumDecl {
                reference: "veh.common.GearPosition".to_string(),
                members: vec!["PARK".to_string(), "DRIVE".to_string()],
            },
        );
        built
    }

    /// A resolution holding the Appendix A symbol view: `Speed` and `Torque`
    /// (types), `GearPosition` (enum), `MAX_SPEED` (constant), `DoorPayload`
    /// (struct).
    fn resolution(db: &RidlDatabase) -> Resolution {
        let file = InputFile::new(db, "veh/common.typl".to_string(), String::new());
        let mut symbols = HashMap::new();
        for (name, kind) in [
            ("Speed", SymbolKind::Type),
            ("Torque", SymbolKind::Type),
            ("GearPosition", SymbolKind::Enum),
            ("MAX_SPEED", SymbolKind::Const),
            ("DoorPayload", SymbolKind::Struct),
        ] {
            symbols.insert(
                name.to_string(),
                Symbol {
                    name: name.to_string(),
                    package: "veh.common".to_string(),
                    kind,
                    internal: false,
                    is_error: false,
                    file,
                    range: TextRange::default(),
                },
            );
        }
        Resolution {
            symbols,
            diagnostics: Vec::new(),
        }
    }

    fn speed() -> ExprType {
        ExprType::Numeric("veh.common.Speed".to_string(), NumericBacking::Float)
    }

    /// An integer-backed named type — the `%` rule's legal operand.
    fn count() -> ExprType {
        ExprType::Numeric("veh.common.Count".to_string(), NumericBacking::Integer)
    }

    fn torque() -> ExprType {
        ExprType::Numeric("veh.common.Torque".to_string(), NumericBacking::Float)
    }

    fn gear() -> ExprType {
        ExprType::EnumType("veh.common.GearPosition".to_string())
    }

    fn codes(diagnostics: &[Diagnostic]) -> Vec<&str> {
        diagnostics
            .iter()
            .map(|diagnostic| diagnostic.code.as_str())
            .collect()
    }

    // --- the ridl §13 examples --------------------------------------------

    #[test]
    fn ridl_13_examples_type_check_clean() {
        let db = RidlDatabase::default();
        let resolution = resolution(&db);
        let vocabulary = vocabulary();

        // `command setRange(min: Speed, max: Speed)`
        let set_range = [("min".to_string(), speed()), ("max".to_string(), speed())];
        for text in ["min < max", "max <= MAX_SPEED"] {
            let scope = ContractScope {
                params: &set_range,
                result: None,
                signals: &[],
                vocabulary: &vocabulary,
                resolution: &resolution,
            };
            let (ty, diagnostics) = check_contract_expr(&parse_expr(text), &scope);
            assert_eq!(ty, Some(ExprType::Boolean), "`{text}`");
            assert!(diagnostics.is_empty(), "`{text}`: {diagnostics:?}");
        }

        // `query getAverageSpeed(window: Duration): Speed`
        let window = [("window".to_string(), ExprType::Duration)];
        let require = ContractScope {
            params: &window,
            result: None,
            signals: &[],
            vocabulary: &vocabulary,
            resolution: &resolution,
        };
        let (ty, diagnostics) = check_contract_expr(&parse_expr("window > 0ms"), &require);
        assert_eq!(ty, Some(ExprType::Boolean));
        assert!(diagnostics.is_empty(), "{diagnostics:?}");

        let ensure = ContractScope {
            params: &window,
            result: Some(speed()),
            signals: &[],
            vocabulary: &vocabulary,
            resolution: &resolution,
        };
        let (ty, diagnostics) = check_contract_expr(&parse_expr("result >= 0.0"), &ensure);
        assert_eq!(ty, Some(ExprType::Boolean));
        assert!(diagnostics.is_empty(), "{diagnostics:?}");

        // `command setGear(position: GearPosition)` inside an interface that
        // declares `signal currentSpeed : Speed`.
        let position = [("position".to_string(), gear())];
        let signals = [("currentSpeed".to_string(), speed())];
        let set_gear = ContractScope {
            params: &position,
            result: None,
            signals: &signals,
            vocabulary: &vocabulary,
            resolution: &resolution,
        };
        let (ty, diagnostics) = check_contract_expr(
            &parse_expr("position != GearPosition.PARK || currentSpeed == 0.0"),
            &set_gear,
        );
        assert_eq!(ty, Some(ExprType::Boolean));
        assert!(diagnostics.is_empty(), "{diagnostics:?}");

        // The tuple form: `query getRange(): (min: Speed, max: Speed)`.
        let tuple = ExprType::tuple(&[
            ("min".to_string(), Some(speed())),
            ("max".to_string(), Some(speed())),
        ]);
        let range_ensure = ContractScope {
            params: &[],
            result: Some(tuple),
            signals: &[],
            vocabulary: &vocabulary,
            resolution: &resolution,
        };
        let (ty, diagnostics) =
            check_contract_expr(&parse_expr("result.min >= 0.0"), &range_ensure);
        assert_eq!(ty, Some(ExprType::Boolean));
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    // --- the RIDL-306 boundary --------------------------------------------

    #[test]
    fn ridl_306_unknown_reference() {
        let db = RidlDatabase::default();
        let resolution = resolution(&db);
        let vocabulary = vocabulary();
        let scope = ContractScope {
            params: &[],
            result: None,
            signals: &[],
            vocabulary: &vocabulary,
            resolution: &resolution,
        };
        let (ty, diagnostics) = check_contract_expr(&parse_expr("unknownName > 0"), &scope);
        assert_eq!(ty, None);
        assert_eq!(codes(&diagnostics), vec!["RIDL-306"]);
        assert!(
            diagnostics[0].message.contains("unknownName"),
            "the message names the offending form: {}",
            diagnostics[0].message
        );
    }

    #[test]
    fn ridl_306_cross_domain_arithmetic() {
        let db = RidlDatabase::default();
        let resolution = resolution(&db);
        let vocabulary = vocabulary();
        let params = [
            ("speed".to_string(), speed()),
            ("window".to_string(), ExprType::Duration),
        ];
        let scope = ContractScope {
            params: &params,
            result: None,
            signals: &[],
            vocabulary: &vocabulary,
            resolution: &resolution,
        };
        let (ty, diagnostics) = check_contract_expr(&parse_expr("speed + window > 0"), &scope);
        assert_eq!(ty, None);
        assert_eq!(codes(&diagnostics), vec!["RIDL-306"]);
    }

    #[test]
    fn ridl_306_cross_named_type_arithmetic() {
        let db = RidlDatabase::default();
        let resolution = resolution(&db);
        let vocabulary = vocabulary();
        let params = [
            ("speed".to_string(), speed()),
            ("torque".to_string(), torque()),
        ];
        let scope = ContractScope {
            params: &params,
            result: None,
            signals: &[],
            vocabulary: &vocabulary,
            resolution: &resolution,
        };
        let (ty, diagnostics) = check_contract_expr(&parse_expr("speed + torque > 0.0"), &scope);
        assert_eq!(ty, None);
        assert_eq!(codes(&diagnostics), vec!["RIDL-306"]);
    }

    #[test]
    fn ridl_306_non_boolean_root() {
        let db = RidlDatabase::default();
        let resolution = resolution(&db);
        let vocabulary = vocabulary();
        let scope = ContractScope {
            params: &[],
            result: None,
            signals: &[],
            vocabulary: &vocabulary,
            resolution: &resolution,
        };
        let (ty, diagnostics) = check_contract_expr(&parse_expr("3"), &scope);
        assert_eq!(ty, None);
        assert_eq!(codes(&diagnostics), vec!["RIDL-306"]);
        assert!(
            diagnostics[0].message.contains("boolean"),
            "{}",
            diagnostics[0].message
        );
    }

    #[test]
    fn ridl_306_signal_read_in_an_ensure() {
        // ridl §13 scopes an `ensure` to `result` and the parameters, so the
        // scope of an ensure carries no signals at all.
        let db = RidlDatabase::default();
        let resolution = resolution(&db);
        let vocabulary = vocabulary();
        let scope = ContractScope {
            params: &[],
            result: Some(speed()),
            signals: &[],
            vocabulary: &vocabulary,
            resolution: &resolution,
        };
        let (ty, diagnostics) = check_contract_expr(&parse_expr("currentSpeed >= 0.0"), &scope);
        assert_eq!(ty, None);
        assert_eq!(codes(&diagnostics), vec!["RIDL-306"]);
        assert!(
            diagnostics[0].message.contains("`require`"),
            "the message points at the require-only scope: {}",
            diagnostics[0].message
        );
    }

    #[test]
    fn ridl_306_qualified_member_chain() {
        // The parser accepts a CamelCase segment after `.`, so
        // `veh.common.GearPosition.PARK` parses into nested member access
        // although expr-core §3.1 declares it inexpressible.
        let db = RidlDatabase::default();
        let resolution = resolution(&db);
        let vocabulary = vocabulary();
        let params = [("position".to_string(), gear())];
        let scope = ContractScope {
            params: &params,
            result: None,
            signals: &[],
            vocabulary: &vocabulary,
            resolution: &resolution,
        };
        let (ty, diagnostics) = check_contract_expr(
            &parse_expr("position == veh.common.GearPosition.PARK"),
            &scope,
        );
        assert_eq!(ty, None);
        assert_eq!(codes(&diagnostics), vec!["RIDL-306"]);
        assert!(
            diagnostics[0].message.contains("GearPosition"),
            "the message names the offending segment: {}",
            diagnostics[0].message
        );
    }

    #[test]
    fn ridl_306_further_boundary_forms() {
        let db = RidlDatabase::default();
        let resolution = resolution(&db);
        let vocabulary = vocabulary();
        let params = [
            ("speed".to_string(), speed()),
            ("window".to_string(), ExprType::Duration),
            ("position".to_string(), gear()),
            ("flag".to_string(), ExprType::Boolean),
        ];
        for text in [
            // duration arithmetic (§5.3)
            "window + 10ms < 1s",
            // enum ordering (§5.3)
            "position < GearPosition.DRIVE",
            // `%` over a float-backed operand (§5.3)
            "speed % 0.5 == 0.0",
            // a string operand (§5.3)
            "speed == \"x\"",
            // a boolean where a number is required
            "flag + 1 > 0",
            // an enum type name in value position
            "GearPosition == 0",
            // field access on a non-tuple
            "speed.min >= 0.0",
            // `result` in a require
            "result >= 0.0",
        ] {
            let scope = ContractScope {
                params: &params,
                result: None,
                signals: &[],
                vocabulary: &vocabulary,
                resolution: &resolution,
            };
            let (ty, diagnostics) = check_contract_expr(&parse_expr(text), &scope);
            assert_eq!(ty, None, "`{text}` must not type-check");
            assert!(
                codes(&diagnostics).contains(&"RIDL-306"),
                "`{text}`: {diagnostics:?}"
            );
        }
    }

    #[test]
    fn ridl_305_ensure_without_result() {
        let db = RidlDatabase::default();
        let resolution = resolution(&db);
        let vocabulary = vocabulary();
        let params = [("window".to_string(), ExprType::Duration)];
        let scope = ContractScope {
            params: &params,
            result: Some(speed()),
            signals: &[],
            vocabulary: &vocabulary,
            resolution: &resolution,
        };
        let (ty, diagnostics) = check_contract_expr(&parse_expr("window > 0ms"), &scope);
        assert_eq!(ty, Some(ExprType::Boolean), "the clause is well-typed");
        assert_eq!(codes(&diagnostics), vec!["RIDL-305"]);
        assert_eq!(diagnostics[0].severity, Severity::Warning);
    }

    // --- canonical text ---------------------------------------------------

    #[test]
    fn canonical_text_normalizes_spacing_and_parentheses() {
        for (written, canonical) in [
            ("result>=0.0", "result >= 0.0"),
            ("( result ) >= ( 0.0 )", "result >= 0.0"),
            ("(min < max)", "min < max"),
            ("min<max&&max<=MAX_SPEED", "min < max && max <= MAX_SPEED"),
            ("a || (b && c)", "a || b && c"),
            ("(a || b) && c", "(a || b) && c"),
            ("a + b * c < d", "a + b * c < d"),
            ("(a + b) * c < d", "(a + b) * c < d"),
            ("a - (b - c) < d", "a - (b - c) < d"),
            ("a - b + c < d", "a - b + c < d"),
            ("!a && b", "!a && b"),
            ("!(a && b)", "!(a && b)"),
            (
                "position !=GearPosition . PARK||currentSpeed== 0.0",
                "position != GearPosition.PARK || currentSpeed == 0.0",
            ),
            ("result . min>=0.0", "result.min >= 0.0"),
        ] {
            let rendered = canonical_expr_text(&parse_expr(written));
            assert_eq!(rendered, canonical, "`{written}`");
            assert_eq!(
                canonical_expr_text(&parse_expr(&rendered)),
                rendered,
                "`{written}` renders idempotently"
            );
        }
    }

    // --- reference collection ---------------------------------------------

    #[test]
    fn collect_refs_reads_the_appendix_a_set_gear_require() {
        let db = RidlDatabase::default();
        let resolution = resolution(&db);
        let vocabulary = vocabulary();
        let params = [("position".to_string(), gear())];
        let signals = [("currentSpeed".to_string(), speed())];
        let scope = ContractScope {
            params: &params,
            result: None,
            signals: &signals,
            vocabulary: &vocabulary,
            resolution: &resolution,
        };
        let refs = collect_refs(
            &parse_expr("position != GearPosition.PARK || currentSpeed == 0.0"),
            &scope,
        );
        assert_eq!(refs.params, vec!["position".to_string()]);
        assert_eq!(refs.signals, vec!["currentSpeed".to_string()]);
        assert!(!refs.uses_result, "a require never reads `result`");
        assert!(refs.consts.is_empty(), "the enum type name is not a read");
    }

    #[test]
    fn collect_refs_reads_constants_and_result() {
        let db = RidlDatabase::default();
        let resolution = resolution(&db);
        let vocabulary = vocabulary();
        let params = [("max".to_string(), speed())];
        let scope = ContractScope {
            params: &params,
            result: Some(speed()),
            signals: &[],
            vocabulary: &vocabulary,
            resolution: &resolution,
        };
        let refs = collect_refs(&parse_expr("result <= MAX_SPEED && result >= max"), &scope);
        assert!(refs.uses_result);
        assert_eq!(refs.consts, vec!["MAX_SPEED".to_string()]);
        assert_eq!(refs.params, vec!["max".to_string()]);
    }
}
