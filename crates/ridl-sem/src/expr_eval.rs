//! The contract-expression evaluator — total evaluation of the guaranteed
//! subset over the exact domains of `docs/specification/expr-core-specification.md`
//! §7 (epic E2 story E2.11a).
//!
//! This is the value half of the contract plane: [`crate::expr`] types a clause,
//! this module runs it. It is the shared engine behind `ridl test`'s property
//! runs today and the E5 reference oracle later, which is why §7 makes the
//! domains normative — the two must agree exactly.
//!
//! Three properties are load-bearing:
//!
//! - **Total.** [`eval_expr`] runs over generated input and over whatever the
//!   parser accepted, so it never panics: no `unwrap`, no indexing, no
//!   arithmetic that can overflow, and a depth guard against a pathologically
//!   nested expression. Every failure is an [`EvalError`].
//! - **Exact.** Numbers are arbitrary-precision rationals ([`ExactValue`]), never
//!   floats, so `0.1 + 0.2 == 0.3` holds. Durations are exact microsecond
//!   counts, so `1s == 1000ms`.
//! - **One fault.** Division by zero is the single defined evaluation fault
//!   (§7); it surfaces as [`EvalError::DivisionByZero`], never as a panic and
//!   never as a silently substituted value.

use num_bigint::{BigInt, Sign};
use num_rational::BigRational;
use ridl_syntax::SyntaxKind;
use ridl_syntax::ast::{self, AstNode};

use crate::expr::NumericBacking;
use crate::scalar::ExactValue;
use crate::timing::duration_literal_us;

// ==========================================================================
// Types
// ==========================================================================

/// A value of the guaranteed subset (expr-core §7).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    Bool(bool),
    /// An exact rational — never a float (§7) — together with the backing of
    /// the type it came from.
    ///
    /// The backing is carried because two §7 rules are rules about an operand's
    /// **type** and not about the value it happens to hold: `/` truncates over
    /// integer-backed operands and divides exactly over float-backed ones, and
    /// `%` requires integer-backed operands. The value alone cannot answer
    /// either — `1.0 / 3.0` is a division of two float-backed operands whose
    /// values are both integral, and it must yield exactly one third, not zero.
    Num(ExactValue, NumericBacking),
    /// An exact microsecond count (§7): `1s`, `1000ms`, and `1000000us` are one
    /// value.
    Dur(ExactValue),
    /// An enum member: the canonical `package.Enum` reference and the member's
    /// declared value. Equality is member equality within one enum type.
    EnumVal(String, i64),
    /// A tuple-returning query's `result`, in declaration order.
    Tuple(Vec<(String, Value)>),
}

/// The environment one clause evaluates against — the value counterpart of
/// [`crate::expr::ContractScope`].
///
/// `consts` resolves every *named* value that is not a parameter and not
/// `result`: a package constant under its bare name, and an enum member under
/// its dotted `Enum.MEMBER` spelling. Routing both through one closure keeps
/// the environment open — `ridl test` fills it from the IR, the E5 oracle will
/// fill it from a provider — without this module needing to know either.
///
/// A `require` that reads the enclosing interface's own signals has no entry
/// here on purpose: a signal is live state, so `ridl test` reports such a
/// clause as skipped rather than evaluating it. Naming one anyway is an
/// [`EvalError::UnboundRef`], not a wrong answer.
pub struct EvalEnv<'a> {
    pub params: &'a [(String, Value)],
    pub result: Option<Value>,
    pub consts: &'a dyn Fn(&str) -> Option<Value>,
}

/// Why an evaluation did not produce a value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvalError {
    /// A name the environment does not bind.
    UnboundRef(String),
    /// An operand outside the operator's domain, or a form the guaranteed
    /// subset does not admit. The string describes what was found.
    TypeMismatch(String),
    /// `/` or `%` with a zero divisor — the single defined evaluation fault
    /// (expr-core §7).
    DivisionByZero,
}

impl std::fmt::Display for EvalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EvalError::UnboundRef(name) => write!(f, "`{name}` is not bound in this environment"),
            EvalError::TypeMismatch(what) => write!(f, "{what}"),
            EvalError::DivisionByZero => write!(f, "division by zero"),
        }
    }
}

/// The deepest expression nesting [`eval_expr`] will walk before giving up.
///
/// Evaluation recurses over the tree, so an adversarially nested expression
/// would otherwise exhaust the stack — a panic by another name. Exceeding the
/// limit is an [`EvalError::TypeMismatch`] like any other refusal.
///
/// The value matches the parser's own `MAX_TYPE_DEPTH`, which bounds the
/// expression grammar too: nothing the parser accepts nests deeper than this,
/// so the guard never refuses a tree that could legitimately arrive, and a
/// binary chain — which the parser builds iteratively and does not cap — is
/// stopped well inside the stack. It must stay **below** the frame budget, not
/// merely above what contracts are written with: an earlier 256 exhausted a
/// 2 MB stack before the guard could fire, because a debug-build
/// `eval`/`eval_binary` pair is several kilobytes of frame.
const MAX_DEPTH: u32 = 128;

// ==========================================================================
// Evaluation
// ==========================================================================

/// Evaluates one guaranteed-subset expression against `env`.
///
/// Total: every input either yields a [`Value`] or an [`EvalError`]. The
/// expression is normally one [`crate::expr::check_contract_expr`] already
/// accepted, but nothing here relies on that: a half-parsed tree, an operand
/// outside an operator's domain, and an unbound name are all refused rather
/// than assumed away.
///
/// It does **not** re-check nominal typing. [`Value::Num`] carries a numeric
/// backing but not the named type it came from, so `speed + torque` over two
/// distinct float-backed types evaluates happily here — the checker rejects it
/// upstream as RIDL-306 (expr-core §5.2), which is where that rule lives. When
/// E5.1 lifts rmdl §3.3 scalar multiplication and its unit discipline,
/// [`Value::Num`] will need the type reference too; this shape is a way-station,
/// not the terminal one.
pub fn eval_expr(expr: &ast::Expr, env: &EvalEnv) -> Result<Value, EvalError> {
    eval(expr, env, 0)
}

fn eval(expr: &ast::Expr, env: &EvalEnv, depth: u32) -> Result<Value, EvalError> {
    if depth > MAX_DEPTH {
        return Err(EvalError::TypeMismatch(format!(
            "expression nests deeper than {MAX_DEPTH} levels"
        )));
    }
    match expr {
        ast::Expr::Literal(literal) => eval_literal(literal),
        ast::Expr::Paren(paren) => match paren.inner() {
            Some(inner) => eval(&inner, env, depth + 1),
            None => Err(malformed("an empty parenthesis")),
        },
        ast::Expr::Path(path) => eval_path(path, env),
        ast::Expr::Prefix(prefix) => eval_prefix(prefix, env, depth),
        ast::Expr::Member(member) => eval_member(member, env, depth),
        ast::Expr::Binary(binary) => eval_binary(binary, env, depth),
    }
}

fn eval_literal(literal: &ast::LiteralExpr) -> Result<Value, EvalError> {
    let Some(token) = literal.token() else {
        return Err(malformed("a literal with no token"));
    };
    let text = token.text();
    match token.kind() {
        SyntaxKind::IntNumber | SyntaxKind::FloatNumber => {
            // The spelling is what fixes a bare literal's backing: `2` is
            // integer-backed and `2.0` is float-backed, and `7 / 2` and
            // `7.0 / 2.0` are different divisions because of it (expr-core
            // §5.3, §7).
            let backing = if token.kind() == SyntaxKind::IntNumber {
                NumericBacking::Integer
            } else {
                NumericBacking::Float
            };
            match ExactValue::parse(text) {
                Some(value) => Ok(Value::Num(value, backing)),
                None => Err(EvalError::TypeMismatch(format!(
                    "`{text}` is not an exact decimal literal"
                ))),
            }
        }
        SyntaxKind::Duration => match duration_literal_us(text) {
            Some(us) => Ok(Value::Dur(us)),
            None => Err(EvalError::TypeMismatch(format!(
                "`{text}` is not a duration literal"
            ))),
        },
        SyntaxKind::TrueKw => Ok(Value::Bool(true)),
        SyntaxKind::FalseKw => Ok(Value::Bool(false)),
        SyntaxKind::String | SyntaxKind::Regex => Err(EvalError::TypeMismatch(
            "a string or regex literal — no operator of the guaranteed subset works over strings or bytes (expr-core §5.3)".to_string(),
        )),
        _ => Err(EvalError::TypeMismatch(format!(
            "`{text}` is not a literal of the guaranteed subset"
        ))),
    }
}

fn eval_path(path: &ast::PathExpr, env: &EvalEnv) -> Result<Value, EvalError> {
    let Some(token) = path.name_token() else {
        return Err(malformed("a path with no name"));
    };
    let name = token.text();
    if let Some(value) = lookup(env.params, name) {
        return Ok(value);
    }
    if name == "result" {
        return match &env.result {
            Some(value) => Ok(value.clone()),
            None => Err(EvalError::UnboundRef("result".to_string())),
        };
    }
    match (env.consts)(name) {
        Some(value) => Ok(value),
        None => Err(EvalError::UnboundRef(name.to_string())),
    }
}

fn eval_prefix(prefix: &ast::PrefixExpr, env: &EvalEnv, depth: u32) -> Result<Value, EvalError> {
    let Some(operand) = prefix.operand() else {
        return Err(malformed("a prefix operator with no operand"));
    };
    let Some(token) = prefix.op_token() else {
        return Err(malformed("a prefix expression with no operator"));
    };
    let value = eval(&operand, env, depth + 1)?;
    match token.kind() {
        SyntaxKind::Bang => match value {
            Value::Bool(held) => Ok(Value::Bool(!held)),
            other => Err(EvalError::TypeMismatch(format!(
                "`!` requires a boolean operand, found {}",
                describe(&other)
            ))),
        },
        SyntaxKind::Minus => match value {
            Value::Num(held, backing) => Ok(Value::Num(ExactValue(-held.0), backing)),
            other => Err(EvalError::TypeMismatch(format!(
                "unary `-` requires a numeric operand, found {}",
                describe(&other)
            ))),
        },
        _ => Err(malformed("an unknown prefix operator")),
    }
}

fn eval_member(member: &ast::MemberExpr, env: &EvalEnv, depth: u32) -> Result<Value, EvalError> {
    let Some(base) = member.base() else {
        return Err(malformed("a member access with no base"));
    };
    let Some(token) = member.member_token() else {
        return Err(malformed("a member access with no member name"));
    };
    let name = token.text();

    // `Enum.MEMBER`: the base is a bare name that no value in scope shadows, so
    // the whole dotted spelling is what the environment is asked for.
    if let ast::Expr::Path(path) = &base
        && let Some(head) = path.name_token()
    {
        let head = head.text();
        let shadowed = lookup(env.params, head).is_some() || head == "result";
        if !shadowed && let Some(value) = (env.consts)(&format!("{head}.{name}")) {
            return Ok(value);
        }
    }

    match eval(&base, env, depth + 1)? {
        Value::Tuple(fields) => match fields.into_iter().find(|(field, _)| field == name) {
            Some((_, value)) => Ok(value),
            None => Err(EvalError::UnboundRef(format!("result.{name}"))),
        },
        other => Err(EvalError::TypeMismatch(format!(
            "field access `.{name}` on {} — the guaranteed subset admits field access on a tuple-typed `result` only",
            describe(&other)
        ))),
    }
}

fn eval_binary(binary: &ast::BinaryExpr, env: &EvalEnv, depth: u32) -> Result<Value, EvalError> {
    let Some(token) = binary.op_token() else {
        return Err(malformed("a binary expression with no operator"));
    };
    let (Some(lhs), Some(rhs)) = (binary.lhs(), binary.rhs()) else {
        return Err(malformed("a binary expression missing an operand"));
    };
    let op = token.text().to_string();

    // `&&` and `||` short-circuit left to right (expr-core §7). With division
    // by zero as a defined fault this is observable — `x != 0 && 1 / x > 2`
    // must not fault — so the right operand is not touched until the left
    // demands it.
    match token.kind() {
        SyntaxKind::AmpAmp | SyntaxKind::PipePipe => {
            let left = expect_bool(eval(&lhs, env, depth + 1)?, &op)?;
            let short_circuit = token.kind() == SyntaxKind::PipePipe;
            if left == short_circuit {
                return Ok(Value::Bool(left));
            }
            let right = expect_bool(eval(&rhs, env, depth + 1)?, &op)?;
            return Ok(Value::Bool(right));
        }
        _ => {}
    }

    let left = eval(&lhs, env, depth + 1)?;
    let right = eval(&rhs, env, depth + 1)?;

    match token.kind() {
        SyntaxKind::EqEq | SyntaxKind::Neq => {
            let equal = equality(&left, &right, &op)?;
            Ok(Value::Bool(if token.kind() == SyntaxKind::EqEq {
                equal
            } else {
                !equal
            }))
        }
        SyntaxKind::Lt | SyntaxKind::Le | SyntaxKind::Gt | SyntaxKind::Ge => {
            let (left, right) = ordered_pair(&left, &right, &op)?;
            Ok(Value::Bool(match token.kind() {
                SyntaxKind::Lt => left < right,
                SyntaxKind::Le => left <= right,
                SyntaxKind::Gt => left > right,
                _ => left >= right,
            }))
        }
        SyntaxKind::Plus | SyntaxKind::Minus | SyntaxKind::Star => {
            let (left, right, backing) = numeric_pair(&left, &right, &op)?;
            let value = match token.kind() {
                SyntaxKind::Plus => left + right,
                SyntaxKind::Minus => left - right,
                _ => left * right,
            };
            Ok(Value::Num(ExactValue(value), backing))
        }
        SyntaxKind::Slash => {
            let (left, right, backing) = numeric_pair(&left, &right, &op)?;
            if is_zero(&right) {
                return Err(EvalError::DivisionByZero);
            }
            // Integer-backed operands truncate toward zero (C17, rmdl §4.1);
            // float-backed operands divide exactly (expr-core §7). The rule
            // reads the operands' backing, not their values: `1.0 / 3.0` is a
            // float-backed division of two integral values and yields exactly
            // one third.
            if backing == NumericBacking::Integer {
                // Truncating to integers first would turn a divisor such as 0.5
                // into a zero divisor and panic inside `BigInt`, and would
                // silently discard the dividend's fractional part. An
                // integer-backed value carrying a fraction is an inconsistent
                // value, so it is refused rather than rounded away.
                let (left, right) = integral_pair(left, right, &op)?;
                let quotient = left / right;
                return Ok(Value::Num(
                    ExactValue(BigRational::from_integer(quotient)),
                    backing,
                ));
            }
            Ok(Value::Num(ExactValue(left / right), backing))
        }
        SyntaxKind::Percent => {
            let (left, right, backing) = numeric_pair(&left, &right, &op)?;
            if is_zero(&right) {
                return Err(EvalError::DivisionByZero);
            }
            // `%` is a rule about the operands' declared backing, not about how
            // a literal is spelled (expr-core §5.3): `6.0 % 3.0` is as much a
            // refusal as `6.0 % 0.5`, although both values are integral.
            if backing != NumericBacking::Integer {
                return Err(EvalError::TypeMismatch(
                    "`%` requires integer-backed operands (expr-core §5.3)".to_string(),
                ));
            }
            // C17 remainder: the result takes the dividend's sign, which is
            // exactly `BigInt`'s `%`. Both operands must genuinely be integers
            // first — see the division above for why truncating here would both
            // panic and lie.
            let (left, right) = integral_pair(left, right, &op)?;
            let remainder = left % right;
            Ok(Value::Num(
                ExactValue(BigRational::from_integer(remainder)),
                backing,
            ))
        }
        _ => Err(malformed("an unknown binary operator")),
    }
}

/// Member equality within one domain (expr-core §7). Two enum values compare
/// equal when they name one member of one enum type.
fn equality(left: &Value, right: &Value, op: &str) -> Result<bool, EvalError> {
    match (left, right) {
        (Value::Bool(left), Value::Bool(right)) => Ok(left == right),
        // Equality is over the exact value, so an integer-backed and a
        // float-backed operand compare by number: `1 == 1.0` holds, exactly as
        // the checker's numeric unification admits it.
        (Value::Num(left, _), Value::Num(right, _)) => Ok(left == right),
        (Value::Dur(left), Value::Dur(right)) => Ok(left == right),
        (Value::EnumVal(left_type, left_value), Value::EnumVal(right_type, right_value)) => {
            if left_type != right_type {
                return Err(EvalError::TypeMismatch(format!(
                    "`{op}` compares `{left_type}` with `{right_type}` — enum equality is within one enum type"
                )));
            }
            Ok(left_value == right_value)
        }
        _ => Err(domain_mismatch(op, "operands of one domain", left, right)),
    }
}

/// The two operands of an ordering comparison, as exact values. Numbers and
/// durations order; nothing else does (expr-core §5.3).
fn ordered_pair<'a>(
    left: &'a Value,
    right: &'a Value,
    op: &str,
) -> Result<(&'a BigRational, &'a BigRational), EvalError> {
    match (left, right) {
        (Value::Num(left, _), Value::Num(right, _)) | (Value::Dur(left), Value::Dur(right)) => {
            Ok((&left.0, &right.0))
        }
        _ => Err(domain_mismatch(
            op,
            "operands of one ordered domain",
            left,
            right,
        )),
    }
}

/// The two operands of an arithmetic operator, with the backing of the result.
/// Arithmetic is numeric only — duration arithmetic is outside the guaranteed
/// subset (expr-core §5.3).
///
/// The result is float-backed as soon as either operand is, which is the same
/// rule the checker applies when it types the expression.
fn numeric_pair(
    left: &Value,
    right: &Value,
    op: &str,
) -> Result<(BigRational, BigRational, NumericBacking), EvalError> {
    match (left, right) {
        (Value::Num(left, left_backing), Value::Num(right, right_backing)) => {
            let backing = match (left_backing, right_backing) {
                (NumericBacking::Integer, NumericBacking::Integer) => NumericBacking::Integer,
                _ => NumericBacking::Float,
            };
            Ok((left.0.clone(), right.0.clone(), backing))
        }
        _ => Err(domain_mismatch(op, "numeric operands", left, right)),
    }
}

/// The two operands of an integer-backed `/` or `%` as exact integers.
///
/// An integer-backed [`Value::Num`] should always hold an integral rational —
/// the sampler and the literal reader both guarantee it — but `Value` is public
/// and its two fields are independent, so a caller can construct an
/// integer-backed half. Truncating that away would panic on a divisor between
/// zero and one and would silently change the dividend, so it is refused as the
/// inconsistent value it is.
fn integral_pair(
    left: BigRational,
    right: BigRational,
    op: &str,
) -> Result<(BigInt, BigInt), EvalError> {
    if !left.is_integer() || !right.is_integer() {
        return Err(EvalError::TypeMismatch(format!(
            "`{op}` has an integer-backed operand holding a non-integral value"
        )));
    }
    Ok((left.to_integer(), right.to_integer()))
}

fn expect_bool(value: Value, op: &str) -> Result<bool, EvalError> {
    match value {
        Value::Bool(held) => Ok(held),
        other => Err(EvalError::TypeMismatch(format!(
            "`{op}` requires boolean operands, found {}",
            describe(&other)
        ))),
    }
}

fn is_zero(value: &BigRational) -> bool {
    value.numer().sign() == Sign::NoSign
}

fn lookup(bindings: &[(String, Value)], name: &str) -> Option<Value> {
    bindings
        .iter()
        .find(|(bound, _)| bound == name)
        .map(|(_, value)| value.clone())
}

fn describe(value: &Value) -> String {
    match value {
        Value::Bool(_) => "a boolean".to_string(),
        Value::Num(_, _) => "a number".to_string(),
        Value::Dur(_) => "a duration".to_string(),
        Value::EnumVal(reference, _) => format!("`{reference}`"),
        Value::Tuple(_) => "a tuple".to_string(),
    }
}

fn domain_mismatch(op: &str, expected: &str, left: &Value, right: &Value) -> EvalError {
    EvalError::TypeMismatch(format!(
        "`{op}` requires {expected}, found {} and {}",
        describe(left),
        describe(right)
    ))
}

/// A tree shape the parser can produce but the grammar does not describe — a
/// half-built node left behind by a parse error. Refused like any other
/// unevaluable form, so a broken parse never reaches an `unwrap`.
fn malformed(what: &str) -> EvalError {
    EvalError::TypeMismatch(format!("{what} — the expression did not parse"))
}

/// The `ast::Expr` of a one-line contract expression, parsed as the `require`
/// clause of a minimal `ridl` file.
///
/// `v2::Contract` carries a clause as canonical text and not as a tree
/// ([`crate::expr::canonical_expr_text`]; the structured form is E5.1), so a
/// consumer that wants to evaluate a lowered contract parses it back. The
/// canonical rendering is idempotent, so this round-trips.
pub fn parse_contract_expr(source: &str) -> Option<ast::Expr> {
    let text = format!("package p\ninterface I {{\n  command c() [ require {source} ]\n}}\n");
    let parse = ridl_syntax::parse(&text, ridl_syntax::Profile::Ridl);
    parse
        .syntax()
        .descendants()
        .find_map(ast::Attribute::cast)
        .and_then(|attribute| attribute.expr())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(text: &str) -> ast::Expr {
        parse_contract_expr(text).unwrap_or_else(|| panic!("`{text}` must parse"))
    }

    /// An integer-backed number.
    fn int(text: &str) -> Value {
        Value::Num(
            ExactValue::parse(text).unwrap_or_else(|| panic!("`{text}` must parse")),
            NumericBacking::Integer,
        )
    }

    /// A float-backed number — what a param of a float-ranged named type binds
    /// to, whether or not the sampled value happens to be integral.
    fn num(text: &str) -> Value {
        Value::Num(
            ExactValue::parse(text).unwrap_or_else(|| panic!("`{text}` must parse")),
            NumericBacking::Float,
        )
    }

    fn dur(us: &str) -> Value {
        Value::Dur(ExactValue::parse(us).unwrap_or_else(|| panic!("`{us}` must parse")))
    }

    /// An environment binding `params`, with no `result` and no constants.
    fn env(params: &[(String, Value)]) -> EvalEnv<'_> {
        EvalEnv {
            params,
            result: None,
            consts: &|_| None,
        }
    }

    fn eval_in(text: &str, env: &EvalEnv) -> Result<Value, EvalError> {
        eval_expr(&parse(text), env)
    }

    /// Evaluates `text` in an empty environment.
    fn eval_bare(text: &str) -> Result<Value, EvalError> {
        eval_in(text, &env(&[]))
    }

    fn assert_true(text: &str) {
        assert_eq!(eval_bare(text), Ok(Value::Bool(true)), "`{text}`");
    }

    fn assert_false(text: &str) {
        assert_eq!(eval_bare(text), Ok(Value::Bool(false)), "`{text}`");
    }

    // --- the guaranteed subset, form by form ------------------------------

    #[test]
    fn comparison_over_numbers() {
        for text in ["1 < 2", "2.5 <= 2.5", "3 > 2", "3 >= 3", "1 == 1", "1 != 2"] {
            assert_true(text);
        }
        for text in ["2 < 1", "3 <= 2", "2 > 3", "2 >= 3", "1 == 2", "1 != 1"] {
            assert_false(text);
        }
    }

    #[test]
    fn boolean_connectives() {
        assert_true("true && true");
        assert_false("true && false");
        assert_true("false || true");
        assert_false("false || false");
        assert_true("!false");
        assert_false("!true");
    }

    #[test]
    fn boolean_connectives_short_circuit() {
        // Short-circuiting is observable through the division fault
        // (expr-core §7): neither clause may evaluate its right operand.
        let params = [("x".to_string(), num("0"))];
        let env = env(&params);
        assert_eq!(
            eval_in("x != 0 && 1 / x > 2", &env),
            Ok(Value::Bool(false)),
            "`&&` must not evaluate the right operand when the left is false"
        );
        assert_eq!(
            eval_in("x == 0 || 1 / x > 2", &env),
            Ok(Value::Bool(true)),
            "`||` must not evaluate the right operand when the left is true"
        );
        // Without the guard the same divisor does fault, which is what makes
        // the two assertions above meaningful.
        assert_eq!(eval_in("1 / x > 2", &env), Err(EvalError::DivisionByZero));
    }

    #[test]
    fn arithmetic_over_exact_numbers() {
        assert_true("1 + 2 == 3");
        assert_true("5 - 8 == -3");
        assert_true("3 * 4 == 12");
        assert_true("7.5 / 2.5 == 3.0");
        assert_true("- 3 + 1 == -2");
    }

    #[test]
    fn integer_division_truncates_toward_zero() {
        // C17 (rmdl §4.1, expr-core §7) over integer-backed operands.
        assert_true("7 / 2 == 3");
        assert_true("-7 / 2 == -3");
        assert_true("7 / -2 == -3");
    }

    #[test]
    fn remainder_takes_the_dividend_sign() {
        // C17 (rmdl §4.1, expr-core §7).
        assert_true("7 % 3 == 1");
        assert_true("-7 % 3 == -1");
        assert_true("7 % -3 == 1");
        assert_true("6 % 3 == 0");
    }

    #[test]
    fn remainder_reads_the_backing_not_the_value() {
        // `%` requires integer-backed operands (expr-core §5.3). The rule is
        // about the operands' type, so a float-backed operand is refused even
        // when its value is integral — `6.0 % 3.0` is as much a refusal as
        // `7.5 % 2`.
        for text in ["7.5 % 2 == 0", "6.0 % 3.0 == 0.0", "6 % 3.0 == 0.0"] {
            assert!(
                matches!(eval_bare(text), Err(EvalError::TypeMismatch(_))),
                "`{text}` must be refused, got {:?}",
                eval_bare(text)
            );
        }
        // A float-backed parameter holding an integral value is refused too.
        let params = [("speed".to_string(), num("6.0"))];
        assert!(matches!(
            eval_in("speed % 3 == 0", &env(&params)),
            Err(EvalError::TypeMismatch(_))
        ));
    }

    #[test]
    fn division_reads_the_backing_not_the_value() {
        // The case that makes the backing load-bearing: both operands are
        // integral, so a value-only rule would truncate. They are float-backed,
        // so the division is exact (expr-core §7).
        assert_true("1.0 / 3.0 * 3.0 == 1.0");
        assert_true("5.0 / 2.0 == 2.5");
        // Two float-backed parameters holding integral values divide exactly.
        let params = [("a".to_string(), num("5.0")), ("b".to_string(), num("2.0"))];
        assert_eq!(
            eval_in("a / b == 2.5", &env(&params)),
            Ok(Value::Bool(true))
        );
        // Two integer-backed parameters truncate.
        let ints = [("a".to_string(), int("5")), ("b".to_string(), int("2"))];
        assert_eq!(eval_in("a / b == 2", &env(&ints)), Ok(Value::Bool(true)));
    }

    #[test]
    fn enum_member_access_and_equality() {
        let gear = |name: &str| -> Option<Value> {
            match name {
                "GearPosition.PARK" => Some(Value::EnumVal("veh.common.GearPosition".into(), 0)),
                "GearPosition.DRIVE" => Some(Value::EnumVal("veh.common.GearPosition".into(), 1)),
                _ => None,
            }
        };
        let params = [(
            "position".to_string(),
            Value::EnumVal("veh.common.GearPosition".into(), 0),
        )];
        let env = EvalEnv {
            params: &params,
            result: None,
            consts: &gear,
        };
        assert_eq!(
            eval_in("position == GearPosition.PARK", &env),
            Ok(Value::Bool(true))
        );
        assert_eq!(
            eval_in("position != GearPosition.DRIVE", &env),
            Ok(Value::Bool(true))
        );
    }

    #[test]
    fn tuple_field_access() {
        let result = Value::Tuple(vec![
            ("min".to_string(), num("0.0")),
            ("max".to_string(), num("250.0")),
        ]);
        let env = EvalEnv {
            params: &[],
            result: Some(result),
            consts: &|_| None,
        };
        assert_eq!(eval_in("result.min >= 0.0", &env), Ok(Value::Bool(true)));
        assert_eq!(
            eval_in("result.max <= result.min", &env),
            Ok(Value::Bool(false))
        );
    }

    #[test]
    fn duration_comparison_is_exact_microseconds() {
        // 1s == 1000ms == 1000000us (expr-core §7).
        assert_true("1s == 1000ms");
        assert_true("1000ms == 1000000us");
        assert_true("10ms > 500us");
        assert_true("1.5ms == 1500us");
        let params = [("window".to_string(), dur("2000000"))];
        let env = env(&params);
        assert_eq!(eval_in("window > 0ms", &env), Ok(Value::Bool(true)));
        assert_eq!(eval_in("window == 2s", &env), Ok(Value::Bool(true)));
    }

    #[test]
    fn parentheses_and_precedence() {
        assert_true("(1 + 2) * 3 == 9");
        assert_true("1 + 2 * 3 == 7");
        assert_true("(true || false) && false == false");
        assert_true("!(1 > 2)");
        assert_true("((((1)))) == 1");
    }

    // --- exactness ---------------------------------------------------------

    #[test]
    fn arithmetic_is_exact_where_a_float_would_round() {
        // The canonical case: in IEEE-754, 0.1 + 0.2 is 0.30000000000000004 and
        // this comparison is false. Over exact rationals it holds (expr-core
        // §7). The float result is asserted alongside so the test states the
        // difference rather than merely assuming it.
        assert_ne!(0.1_f64 + 0.2_f64, 0.3_f64);
        assert_true("0.1 + 0.2 == 0.3");

        // A third that no binary float represents: (1/3)*3 is exactly 1 here.
        assert_true("1.0 / 3.0 * 3.0 == 1.0");

        // And a value past f64's 53-bit mantissa, where the two integers are
        // not even distinguishable as floats — 2^53 and 2^53 + 1 collapse onto
        // one f64. Over exact rationals they stay distinct.
        assert_eq!(9007199254740993.0_f64, 9007199254740992.0_f64);
        assert_true("9007199254740992 + 1 == 9007199254740993");
        assert_true("9007199254740992 + 1 != 9007199254740992");
    }

    #[test]
    fn division_stays_exact_rather_than_rounding() {
        // 0.3 / 0.1 is 2.9999999999999996 in f64, so this would be false.
        assert_ne!(0.3_f64 / 0.1_f64, 3.0_f64);
        assert_true("0.3 / 0.1 == 3.0");
    }

    // --- the single defined fault -----------------------------------------

    #[test]
    fn division_by_zero_is_the_defined_fault() {
        for text in [
            "1 / 0 > 0",
            "1.0 / 0.0 > 0.0",
            "1 % 0 == 0",
            "1 / (2 - 2) > 0",
        ] {
            assert_eq!(eval_bare(text), Err(EvalError::DivisionByZero), "`{text}`");
        }
    }

    #[test]
    fn division_by_a_zero_valued_parameter_faults() {
        let params = [("divisor".to_string(), num("0.0"))];
        assert_eq!(
            eval_in("100.0 / divisor > 1.0", &env(&params)),
            Err(EvalError::DivisionByZero)
        );
    }

    // --- the error variants ------------------------------------------------

    #[test]
    fn unbound_ref_for_a_name_the_environment_does_not_bind() {
        assert_eq!(
            eval_bare("missing > 0"),
            Err(EvalError::UnboundRef("missing".to_string()))
        );
    }

    #[test]
    fn unbound_ref_for_result_outside_an_ensure() {
        assert_eq!(
            eval_bare("result > 0"),
            Err(EvalError::UnboundRef("result".to_string()))
        );
    }

    #[test]
    fn unbound_ref_for_a_signal_read() {
        // A `require` reading a signal has no binding here — `ridl test`
        // reports such a clause as skipped rather than evaluating it, and a
        // caller that evaluates one anyway gets a refusal, not a wrong answer.
        assert_eq!(
            eval_bare("currentSpeed == 0.0"),
            Err(EvalError::UnboundRef("currentSpeed".to_string()))
        );
    }

    #[test]
    fn unbound_ref_for_an_absent_tuple_field() {
        let env = EvalEnv {
            params: &[],
            result: Some(Value::Tuple(vec![("min".to_string(), num("1"))])),
            consts: &|_| None,
        };
        assert_eq!(
            eval_in("result.max > 0", &env),
            Err(EvalError::UnboundRef("result.max".to_string()))
        );
    }

    #[test]
    fn type_mismatch_across_domains() {
        let params = [
            ("window".to_string(), dur("1000")),
            ("speed".to_string(), num("10.0")),
            ("flag".to_string(), Value::Bool(true)),
        ];
        let env = env(&params);
        for text in [
            // duration against a number
            "window > speed",
            // duration arithmetic — comparison only in the subset
            "window + window > window",
            // a boolean where a number is required
            "flag + 1 > 0",
            // a number where a boolean is required
            "speed && flag",
            // `!` on a number
            "!speed",
            // unary `-` on a duration
            "- window > window",
            // a string operand
            "speed == \"x\"",
        ] {
            assert!(
                matches!(eval_in(text, &env), Err(EvalError::TypeMismatch(_))),
                "`{text}` must be a type mismatch, got {:?}",
                eval_in(text, &env)
            );
        }
    }

    #[test]
    fn type_mismatch_for_field_access_on_a_non_tuple() {
        let params = [("speed".to_string(), num("10.0"))];
        assert!(matches!(
            eval_in("speed.min > 0.0", &env(&params)),
            Err(EvalError::TypeMismatch(_))
        ));
    }

    #[test]
    fn type_mismatch_across_two_enum_types() {
        let params = [
            ("a".to_string(), Value::EnumVal("p.A".into(), 0)),
            ("b".to_string(), Value::EnumVal("p.B".into(), 0)),
        ];
        assert!(matches!(
            eval_in("a == b", &env(&params)),
            Err(EvalError::TypeMismatch(_))
        ));
    }

    // --- totality ----------------------------------------------------------

    #[test]
    fn deep_nesting_is_refused_rather_than_exhausting_the_stack() {
        // A LEFT-NESTED BINARY CHAIN, not parenthesis nesting: the parser caps
        // type/paren depth at 128, below this guard's 256, so a paren tower can
        // never reach the guard and a test built on one would pass whatever
        // MAX_DEPTH said. `1 + 1 + 1 + …` nests one level per operator and does
        // reach it.
        // The ceiling is rowan's, not this module's: dropping a syntax tree
        // thousands of levels deep recurses inside the library and overflows
        // before anything here runs. These sizes are all far past MAX_DEPTH,
        // which is what the test is about.
        for terms in [300, 1000, 2000] {
            let chain = vec!["1"; terms].join(" + ");
            let text = format!("{chain} == 1");
            let Some(expr) = parse_contract_expr(&text) else {
                panic!("a {terms}-term chain must parse");
            };
            match eval_expr(&expr, &env(&[])) {
                Err(EvalError::TypeMismatch(message)) => assert!(
                    message.contains(&format!("nests deeper than {MAX_DEPTH}")),
                    "the depth guard is what refused it, not something else: {message}"
                ),
                other => panic!("a {terms}-term chain must hit the depth guard, got {other:?}"),
            }
        }
        // And just under the guard the same shape still evaluates, so the guard
        // is not simply refusing everything.
        let shallow = ["1"; 8].join(" + ");
        let expr = parse(&format!("{shallow} == 8"));
        assert_eq!(eval_expr(&expr, &env(&[])), Ok(Value::Bool(true)));
    }

    #[test]
    fn an_integer_backed_operand_holding_a_fraction_is_refused_not_panicked_on() {
        // `Value` is public and its value and backing are independent, so a
        // caller can build an integer-backed half. Truncating it to an integer
        // would make `a / half` a division by zero inside `BigInt` — a panic —
        // and would silently change the dividend. Both operators refuse it.
        let params = [
            ("a".to_string(), int("7")),
            (
                "half".to_string(),
                Value::Num(
                    ExactValue::parse("0.5").expect("0.5 parses"),
                    NumericBacking::Integer,
                ),
            ),
        ];
        let env = env(&params);
        for text in ["a / half > 1", "a % half == 0", "half / a > 1"] {
            assert!(
                matches!(eval_in(text, &env), Err(EvalError::TypeMismatch(_))),
                "`{text}` must be refused, got {:?}",
                eval_in(text, &env)
            );
        }
    }

    #[test]
    fn every_parseable_expression_yields_a_value_or_an_error() {
        // Totality over a corpus that mixes well-typed, ill-typed, unbound,
        // faulting, and half-parsed forms: the contract is that `eval_expr`
        // returns — it never panics.
        let params = [
            ("speed".to_string(), num("10.0")),
            ("count".to_string(), int("3")),
            ("window".to_string(), dur("1000")),
            ("flag".to_string(), Value::Bool(true)),
        ];
        let env = EvalEnv {
            params: &params,
            result: Some(Value::Tuple(vec![("min".to_string(), num("1"))])),
            consts: &|name| (name == "MAX").then(|| num("100")),
        };
        for text in [
            "speed > 0.0",
            "count % 2 == 0",
            "count / 0 == 0",
            "speed / (speed - speed) > 0.0",
            "window >= 1ms && flag",
            "result.min == 1",
            "result.absent == 1",
            "MAX > speed",
            "MISSING > speed",
            "speed +",
            "( ",
            "!!flag",
            "- - count == count",
            "speed == \"text\"",
            "flag == 1",
            "GearPosition.PARK == 1",
            "1 == 1 == true",
        ] {
            let Some(expr) = parse_contract_expr(text) else {
                continue;
            };
            // The assertion is that this call returns at all.
            let _ = eval_expr(&expr, &env);
        }
    }
}
