//! The narrow, total contract-clause translator (Lane M stage M3, plan
//! "Contract clause bodies — a narrow, total translator").
//!
//! The IR carries a clause only as canonical ridl text in `Contract.source`,
//! not as an expression tree — E5.1 is the story that replaces the text with
//! one. So this translator accepts exactly one expression form and refuses
//! every other:
//!
//! - `<subject> <comparison> <numeric literal>`, where `<subject>` is the
//!   interaction's single declared parameter, or `result` on an `ensure`
//!   clause, and `<comparison>` is one of `<`, `<=`, `>`, `>=`, `==`, `!=`.
//!   The subject's type must be a named scalar over an integer or a float.
//!
//! It emits `args.0 <op> <literal>` for a parameter and `reply.0 <op>
//! <literal>` for `result`, reaching the newtype's public field, with the
//! literal in the subject's Rust numeric type. Several clauses of one kind are
//! conjoined with `&&`. Any other clause form is refused with a
//! [`GenerateError`] rather than dropped: a dropped clause would generate a
//! provider that accepts arguments its own contract forbids.

use crate::{Ctx, GenerateError, ScalarBacking, numeric_tokens, same_package_scalar_backing};
use proc_macro2::TokenStream;
use quote::quote;
use ridl_ir::v2;

/// Which clause kind a translation covers.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ClauseKind {
    Require,
    Ensure,
}

/// The translated body of one `require`/`ensure` method, and which of that
/// method's parameters the body reads — so the caller can name an unread
/// parameter `_args`/`_reply` and stay clippy-clean.
#[derive(Debug)]
pub(crate) struct ClauseBody {
    /// The method body expression: `Ok(())` when there is no clause of the
    /// kind, else `if <conjunction> { Ok(()) } else { Err(()) }`.
    pub expr: TokenStream,
    /// True when the body reads the `args` parameter.
    pub uses_args: bool,
    /// True when the body reads the `reply` parameter.
    pub uses_reply: bool,
}

/// Translates every clause of `kind` on one interaction into a method body.
///
/// `params` is the interaction's declared parameters (M3 restricts a call to
/// one). `reply_named` is the query's reply named type, present only for a
/// query's `ensure`, so `result` can be resolved and refused everywhere else.
pub(crate) fn translate(
    ctx: &Ctx,
    contracts: &[v2::Contract],
    kind: ClauseKind,
    params: &[v2::Param],
    reply_named: Option<&str>,
) -> Result<ClauseBody, GenerateError> {
    let wanted = match kind {
        ClauseKind::Require => v2::ContractKind::Require,
        ClauseKind::Ensure => v2::ContractKind::Ensure,
    };

    let mut predicates: Vec<TokenStream> = Vec::new();
    let mut uses_args = false;
    let mut uses_reply = false;

    for contract in contracts {
        if v2::ContractKind::try_from(contract.kind).ok() != Some(wanted) {
            continue;
        }
        let clause = translate_one(ctx, &contract.source, params, reply_named)?;
        match clause.subject {
            Subject::Arg => uses_args = true,
            Subject::Reply => uses_reply = true,
        }
        predicates.push(clause.expr);
    }

    let expr = match predicates.into_iter().reduce(|a, b| quote! { #a && #b }) {
        None => quote! { Ok(()) },
        Some(conjunction) => quote! { if #conjunction { Ok(()) } else { Err(()) } },
    };

    Ok(ClauseBody {
        expr,
        uses_args,
        uses_reply,
    })
}

/// Which method parameter a translated clause reaches.
enum Subject {
    Arg,
    Reply,
}

struct Clause {
    expr: TokenStream,
    subject: Subject,
}

/// The six accepted comparisons.
#[derive(Clone, Copy)]
enum Comparison {
    Lt,
    Le,
    Gt,
    Ge,
    Eq,
    Ne,
}

impl Comparison {
    fn tokens(self) -> TokenStream {
        match self {
            Comparison::Lt => quote! { < },
            Comparison::Le => quote! { <= },
            Comparison::Gt => quote! { > },
            Comparison::Ge => quote! { >= },
            Comparison::Eq => quote! { == },
            Comparison::Ne => quote! { != },
        }
    }
}

fn translate_one(
    ctx: &Ctx,
    source: &str,
    params: &[v2::Param],
    reply_named: Option<&str>,
) -> Result<Clause, GenerateError> {
    let (subject, comparison, literal) = parse(source)?;

    if subject == "result" {
        let named = reply_named.ok_or_else(|| {
            refuse(
                source,
                "`result` is only a subject on a query's `ensure` clause",
            )
        })?;
        let backing = scalar_backing(ctx, named)
            .ok_or_else(|| refuse(source, "`result` is not a named integer or float scalar"))?;
        let value = literal_tokens(&literal, backing).ok_or_else(|| {
            refuse(
                source,
                "the literal does not match the reply's numeric type",
            )
        })?;
        let op = comparison.tokens();
        return Ok(Clause {
            expr: quote! { reply.0 #op #value },
            subject: Subject::Reply,
        });
    }

    // The subject must be the interaction's single declared parameter.
    let [param] = params else {
        return Err(refuse(
            source,
            "a translated clause needs exactly one declared parameter",
        ));
    };
    if param.name != subject {
        return Err(refuse(
            source,
            "the subject is not the interaction's declared parameter",
        ));
    }
    let Some(named) = param_named_type(param) else {
        return Err(refuse(source, "the parameter is not a named type"));
    };
    let backing = scalar_backing(ctx, named).ok_or_else(|| {
        refuse(
            source,
            "the parameter is not a named integer or float scalar",
        )
    })?;
    let value = literal_tokens(&literal, backing).ok_or_else(|| {
        refuse(
            source,
            "the literal does not match the parameter's numeric type",
        )
    })?;
    let op = comparison.tokens();
    Ok(Clause {
        expr: quote! { args.0 #op #value },
        subject: Subject::Arg,
    })
}

/// Parses `<subject> <comparison> <numeric literal>`, refusing every other
/// shape. The two-character comparisons are tried before the one-character
/// ones so `<=` is not read as `<`.
fn parse(source: &str) -> Result<(String, Comparison, String), GenerateError> {
    let text = source.trim();
    let comparisons = [
        ("<=", Comparison::Le),
        (">=", Comparison::Ge),
        ("==", Comparison::Eq),
        ("!=", Comparison::Ne),
        ("<", Comparison::Lt),
        (">", Comparison::Gt),
    ];
    for (symbol, comparison) in comparisons {
        let Some(position) = text.find(symbol) else {
            continue;
        };
        let left = text[..position].trim();
        let right = text[position + symbol.len()..].trim();
        if is_identifier(left) && is_number(right) {
            return Ok((left.to_string(), comparison, right.to_string()));
        }
        // The clause has a comparison but not the accepted operand shape:
        // refuse rather than try to read a different operator out of it.
        return Err(refuse(
            source,
            "not `<subject> <comparison> <numeric literal>`",
        ));
    }
    Err(refuse(source, "no accepted comparison"))
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

/// The literal tokens in the subject's Rust numeric type: an integer-backed
/// scalar takes an integer literal, a float-backed one a float literal. A
/// fractional literal on an integer subject is refused.
fn literal_tokens(value: &str, backing: ScalarBacking) -> Option<TokenStream> {
    match backing {
        ScalarBacking::Integer => {
            if value.contains(['.', 'e', 'E']) {
                return None;
            }
            Some(numeric_tokens(value, false))
        }
        ScalarBacking::Float => Some(numeric_tokens(value, true)),
        ScalarBacking::Boolean | ScalarBacking::String | ScalarBacking::Bytes => None,
    }
}

/// The backing of a same-package named scalar, restricted to the integer and
/// float classes the accepted form allows.
fn scalar_backing(ctx: &Ctx, reference: &str) -> Option<ScalarBacking> {
    match same_package_scalar_backing(ctx, reference)? {
        backing @ (ScalarBacking::Integer | ScalarBacking::Float) => Some(backing),
        _ => None,
    }
}

fn param_named_type(param: &v2::Param) -> Option<&str> {
    match param.r#type.as_ref()?.kind.as_ref()? {
        v2::field_type::Kind::Named(name) => Some(name),
        _ => None,
    }
}

fn refuse(source: &str, reason: &str) -> GenerateError {
    GenerateError {
        message: format!("cannot translate contract clause `{source}`: {reason}"),
    }
}

#[cfg(test)]
mod tests {
    use super::{ClauseKind, translate};
    use crate::Ctx;
    use ridl_ir::v2;

    /// A package declaring one integer scalar and one float scalar, so a
    /// subject's backing resolves.
    fn scalar_package() -> v2::Package {
        v2::Package {
            name: "p".to_string(),
            decls: vec![
                scalar_decl("Level", v2::PrimitiveType::Integer),
                scalar_decl("Rate", v2::PrimitiveType::Float),
            ],
            interfaces: Vec::new(),
            services: Vec::new(),
            retired: Vec::new(),
        }
    }

    fn scalar_decl(name: &str, prim: v2::PrimitiveType) -> v2::Decl {
        v2::Decl {
            name: name.to_string(),
            visibility: v2::Visibility::Public as i32,
            is_error: false,
            doc: String::new(),
            labels: Vec::new(),
            deprecated: None,
            ordinal: 0,
            kind: Some(v2::decl::Kind::TypeDef(v2::TypeDef {
                backing: Some(v2::Backing {
                    kind: Some(v2::backing::Kind::Primitive(prim as i32)),
                }),
                constraint: None,
                declared_init: None,
                init: None,
                width: None,
            })),
        }
    }

    fn param(name: &str, type_ref: &str) -> v2::Param {
        v2::Param {
            name: name.to_string(),
            r#type: Some(v2::FieldType {
                optional: false,
                kind: Some(v2::field_type::Kind::Named(type_ref.to_string())),
            }),
        }
    }

    fn clause(kind: v2::ContractKind, source: &str) -> v2::Contract {
        v2::Contract {
            kind: kind as i32,
            source: source.to_string(),
            signal_refs: Vec::new(),
            param_refs: Vec::new(),
            uses_result: false,
            observer_id: String::new(),
        }
    }

    fn dense(tokens: &proc_macro2::TokenStream) -> String {
        tokens
            .to_string()
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect()
    }

    #[test]
    fn an_accepted_parameter_clause_reaches_the_newtype_field() {
        let package = scalar_package();
        let ctx = Ctx::new(&package);
        let params = [param("level", "Level")];
        let contracts = [clause(v2::ContractKind::Require, "level < 100")];

        let body =
            translate(&ctx, &contracts, ClauseKind::Require, &params, None).expect("accepted");
        assert!(body.uses_args);
        assert!(!body.uses_reply);
        assert_eq!(dense(&body.expr), "ifargs.0<100{Ok(())}else{Err(())}");
    }

    /// Pins `Comparison::Le`'s emitted operator: `<=`, not `<` or any other
    /// token. Before this test, `<=` appeared in no test and no fixture, so a
    /// mutation emitting `<` for `Comparison::Le` passed the whole suite.
    #[test]
    fn a_le_comparison_emits_the_le_operator() {
        let package = scalar_package();
        let ctx = Ctx::new(&package);
        let params = [param("level", "Level")];
        let contracts = [clause(v2::ContractKind::Require, "level <= 100")];

        let body =
            translate(&ctx, &contracts, ClauseKind::Require, &params, None).expect("accepted");
        assert_eq!(dense(&body.expr), "ifargs.0<=100{Ok(())}else{Err(())}");
    }

    /// Pins `Comparison::Eq`'s emitted operator: `==`, not `!=` or any other
    /// token. Before this test, `==` appeared in no test and no fixture, so a
    /// mutation swapping `Comparison::Eq` and `Comparison::Ne` passed the whole
    /// suite.
    #[test]
    fn an_eq_comparison_emits_the_eq_operator() {
        let package = scalar_package();
        let ctx = Ctx::new(&package);
        let params = [param("level", "Level")];
        let contracts = [clause(v2::ContractKind::Require, "level == 100")];

        let body =
            translate(&ctx, &contracts, ClauseKind::Require, &params, None).expect("accepted");
        assert_eq!(dense(&body.expr), "ifargs.0==100{Ok(())}else{Err(())}");
    }

    /// Pins `Comparison::Ne`'s emitted operator: `!=`, not `==` or any other
    /// token. Before this test, `!=` appeared in no test and no fixture, so a
    /// mutation swapping `Comparison::Eq` and `Comparison::Ne` passed the whole
    /// suite.
    #[test]
    fn a_ne_comparison_emits_the_ne_operator() {
        let package = scalar_package();
        let ctx = Ctx::new(&package);
        let params = [param("level", "Level")];
        let contracts = [clause(v2::ContractKind::Require, "level != 100")];

        let body =
            translate(&ctx, &contracts, ClauseKind::Require, &params, None).expect("accepted");
        assert_eq!(dense(&body.expr), "ifargs.0!=100{Ok(())}else{Err(())}");
    }

    #[test]
    fn a_float_backed_result_clause_emits_a_float_literal() {
        let package = scalar_package();
        let ctx = Ctx::new(&package);
        let params = [param("window", "Level")];
        let contracts = [clause(v2::ContractKind::Ensure, "result >= 0")];

        let body = translate(&ctx, &contracts, ClauseKind::Ensure, &params, Some("Rate"))
            .expect("accepted");
        assert!(body.uses_reply);
        assert!(!body.uses_args);
        // Rate is float-backed, so the literal is a float.
        assert_eq!(dense(&body.expr), "ifreply.0>=0.0{Ok(())}else{Err(())}");
    }

    #[test]
    fn several_clauses_of_one_kind_are_conjoined() {
        let package = scalar_package();
        let ctx = Ctx::new(&package);
        let params = [param("level", "Level")];
        let contracts = [
            clause(v2::ContractKind::Require, "level < 100"),
            clause(v2::ContractKind::Require, "level >= 0"),
        ];

        let body =
            translate(&ctx, &contracts, ClauseKind::Require, &params, None).expect("accepted");
        assert_eq!(
            dense(&body.expr),
            "ifargs.0<100&&args.0>=0{Ok(())}else{Err(())}"
        );
    }

    #[test]
    fn no_clause_of_a_kind_emits_ok() {
        let package = scalar_package();
        let ctx = Ctx::new(&package);
        let params = [param("level", "Level")];
        let contracts = [clause(v2::ContractKind::Require, "level < 100")];

        // No ensure clause present.
        let body =
            translate(&ctx, &contracts, ClauseKind::Ensure, &params, Some("Level")).expect("empty");
        assert!(!body.uses_args);
        assert!(!body.uses_reply);
        assert_eq!(dense(&body.expr), "Ok(())");
    }

    #[test]
    fn a_compound_clause_is_refused() {
        let package = scalar_package();
        let ctx = Ctx::new(&package);
        let params = [param("level", "Level")];
        let contracts = [clause(
            v2::ContractKind::Require,
            "level < 100 || level == 0",
        )];

        let error =
            translate(&ctx, &contracts, ClauseKind::Require, &params, None).expect_err("refused");
        assert!(error.message.contains("cannot translate contract clause"));
    }

    #[test]
    fn a_unit_literal_is_refused() {
        let package = scalar_package();
        let ctx = Ctx::new(&package);
        let params = [param("window", "Level")];
        let contracts = [clause(v2::ContractKind::Require, "window > 0ms")];

        translate(&ctx, &contracts, ClauseKind::Require, &params, None).expect_err("refused");
    }

    #[test]
    fn a_result_subject_outside_an_ensure_is_refused() {
        let package = scalar_package();
        let ctx = Ctx::new(&package);
        let params = [param("level", "Level")];
        // `result` with no reply type in scope (a require clause) is refused.
        let contracts = [clause(v2::ContractKind::Require, "result >= 0")];

        translate(&ctx, &contracts, ClauseKind::Require, &params, None).expect_err("refused");
    }

    #[test]
    fn a_fractional_literal_on_an_integer_subject_is_refused() {
        let package = scalar_package();
        let ctx = Ctx::new(&package);
        let params = [param("level", "Level")];
        let contracts = [clause(v2::ContractKind::Require, "level < 1.5")];

        translate(&ctx, &contracts, ClauseKind::Require, &params, None).expect_err("refused");
    }
}
