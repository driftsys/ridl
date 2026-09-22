//! The narrow contract-clause translation, as a model fact (design note D-9).
//!
//! The IR carries a clause only as canonical ridl text (`Contract.source`);
//! E5.1 is the story that replaces the text with an expression tree. Until
//! then the accepted form is
//! `<subject> <comparison> <numeric literal>`, and whether a clause has that
//! form depends on the subject's resolved scalar class — a fact only a
//! resolver holds. So the translation itself is what the model carries, and a
//! refusal carries the reason verbatim, because a printer prints it into
//! generated source.
//!
//! This is `ridl-backend-rust`'s `clauses.rs` parser and subject resolution,
//! with the emission removed.

use super::facts::scalar_class;
use super::resolve::Scope;
use super::v1;
use crate::v2;

/// The translation of one clause: the accepted comparison, or the reason it
/// was refused.
pub(crate) fn translate(
    scope: Scope<'_>,
    home: &v2::Package,
    source: &str,
    params: &[v2::Param],
    reply_named: Option<&str>,
) -> v1::clause::Translation {
    match translate_accepted(scope, home, source, params, reply_named) {
        Ok(comparison) => v1::clause::Translation::Comparison(comparison),
        Err(reason) => v1::clause::Translation::Refused(reason),
    }
}

fn translate_accepted(
    scope: Scope<'_>,
    home: &v2::Package,
    source: &str,
    params: &[v2::Param],
    reply_named: Option<&str>,
) -> Result<v1::Comparison, String> {
    let (subject, op, literal) = parse(source)?;

    if subject == "result" {
        let named = reply_named.ok_or("`result` is only a subject on a query's `ensure` clause")?;
        let class = numeric_class(scope, home, named)
            .ok_or("`result` is not a named integer or float scalar")?;
        if !literal_matches(&literal, class) {
            return Err("the literal does not match the reply's numeric type".to_string());
        }
        return Ok(comparison(
            op,
            literal,
            class,
            v1::comparison::Subject::Result(true),
        ));
    }

    let [param] = params else {
        return Err("a translated clause needs exactly one declared parameter".to_string());
    };
    if param.name != subject {
        return Err("the subject is not the interaction's declared parameter".to_string());
    }
    let named = param_named_type(param).ok_or("the parameter is not a named type")?;
    let class = numeric_class(scope, home, named)
        .ok_or("the parameter is not a named integer or float scalar")?;
    if !literal_matches(&literal, class) {
        return Err("the literal does not match the parameter's numeric type".to_string());
    }
    Ok(comparison(
        op,
        literal,
        class,
        v1::comparison::Subject::Param(0),
    ))
}

fn comparison(
    op: v1::ComparisonOp,
    literal: String,
    class: v1::ScalarClass,
    subject: v1::comparison::Subject,
) -> v1::Comparison {
    v1::Comparison {
        op: op as i32,
        literal,
        literal_class: class as i32,
        subject: Some(subject),
    }
}

/// Parses `<subject> <comparison> <numeric literal>`, refusing every other
/// shape. The two-character comparisons are tried before the one-character
/// ones so `<=` is not read as `<`.
fn parse(source: &str) -> Result<(String, v1::ComparisonOp, String), String> {
    let text = source.trim();
    let comparisons = [
        ("<=", v1::ComparisonOp::Le),
        (">=", v1::ComparisonOp::Ge),
        ("==", v1::ComparisonOp::Eq),
        ("!=", v1::ComparisonOp::Ne),
        ("<", v1::ComparisonOp::Lt),
        (">", v1::ComparisonOp::Gt),
    ];
    for (symbol, op) in comparisons {
        let Some(position) = text.find(symbol) else {
            continue;
        };
        let left = text[..position].trim();
        let right = text[position + symbol.len()..].trim();
        if is_identifier(left) && is_number(right) {
            return Ok((left.to_string(), op, right.to_string()));
        }
        return Err("not `<subject> <comparison> <numeric literal>`".to_string());
    }
    Err("no accepted comparison".to_string())
}

fn is_identifier(text: &str) -> bool {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) if first.is_ascii_alphabetic() || first == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn is_number(text: &str) -> bool {
    !text.is_empty()
        && text
            .chars()
            .all(|c| c.is_ascii_digit() || matches!(c, '.' | '-' | '+' | 'e' | 'E'))
        && text.parse::<f64>().is_ok()
}

/// An integer literal on an integer subject, any literal on a float subject.
fn literal_matches(value: &str, class: v1::ScalarClass) -> bool {
    match class {
        v1::ScalarClass::Integer => !value.contains(['.', 'e', 'E']),
        v1::ScalarClass::Float => true,
        _ => false,
    }
}

/// The class of a named scalar the subject resolves to, restricted to the two
/// the accepted form allows.
fn numeric_class(scope: Scope<'_>, home: &v2::Package, reference: &str) -> Option<v1::ScalarClass> {
    let (decl, _) = scope.resolve(home, reference)?;
    let v2::decl::Kind::TypeDef(td) = decl.kind.as_ref()? else {
        return None;
    };
    match scalar_class(td) {
        class @ (v1::ScalarClass::Integer | v1::ScalarClass::Float) => Some(class),
        _ => None,
    }
}

fn param_named_type(param: &v2::Param) -> Option<&str> {
    match param.r#type.as_ref()?.kind.as_ref()? {
        v2::field_type::Kind::Named(name) => Some(name),
        _ => None,
    }
}
