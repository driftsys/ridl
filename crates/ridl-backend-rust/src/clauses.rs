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
//! <literal>` for `result`, with the literal in the subject's Rust numeric
//! type. The newtype's field is private, and the read is still legal: the
//! subject is always a same-package named scalar, and the clause is emitted
//! inside a descriptor `impl` block at package module scope, the same module
//! that declares the type, where a private field is visible. Several clauses
//! of one kind are conjoined with `&&`. Any other clause form is refused with a
//! [`GenerateError`] rather than dropped: a dropped clause would generate a
//! provider that accepts arguments its own contract forbids.
//!
//! Since stage P4 the parser and the subject resolution are the lowering's
//! (`ridl_ir::codegen`, design note D-9): the model carries either the
//! accepted comparison or the reason the clause was refused, verbatim, and
//! what is left here is the rendering — and the one rule this printer keeps
//! for itself, that a subject whose scalar the scope resolved in another
//! package is refused all the same, because this backend resolves nothing
//! across packages (design note §3.5).

use crate::{Ctx, GenerateError, ScalarBacking, class_backing, numeric_tokens};
use proc_macro2::TokenStream;
use quote::quote;
use ridl_ir::codegen::v1;

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

/// Renders every clause of `kind` on one interaction into a method body.
///
/// `params` is the interaction's declared parameters (M3 restricts a call to
/// one). `reply` is the query's reply reference, present only for a query's
/// `ensure`, so `result` can be resolved and refused everywhere else.
pub(crate) fn translate(
    ctx: &Ctx,
    clauses: &[v1::Clause],
    kind: ClauseKind,
    params: &[v1::Param],
    reply: Option<&v1::TypeRef>,
) -> Result<ClauseBody, GenerateError> {
    let wanted = match kind {
        ClauseKind::Require => v1::ContractKind::Require,
        ClauseKind::Ensure => v1::ContractKind::Ensure,
    };

    let mut predicates: Vec<TokenStream> = Vec::new();
    let mut uses_args = false;
    let mut uses_reply = false;

    for clause in clauses {
        if v1::ContractKind::try_from(clause.kind).ok() != Some(wanted) {
            continue;
        }
        let rendered = render_one(ctx, clause, params, reply)?;
        match rendered.subject {
            Subject::Arg => uses_args = true,
            Subject::Reply => uses_reply = true,
        }
        predicates.push(rendered.expr);
    }

    let expr = match predicates.into_iter().reduce(|a, b| quote! { #a && #b }) {
        None => quote! { ::core::result::Result::Ok(()) },
        Some(conjunction) => quote! {
            if #conjunction {
                ::core::result::Result::Ok(())
            } else {
                ::core::result::Result::Err(())
            }
        },
    };

    Ok(ClauseBody {
        expr,
        uses_args,
        uses_reply,
    })
}

/// Which method parameter a rendered clause reaches.
enum Subject {
    Arg,
    Reply,
}

struct Rendered {
    expr: TokenStream,
    subject: Subject,
}

/// The Rust operator of one accepted comparison.
fn operator(op: i32) -> TokenStream {
    match v1::ComparisonOp::try_from(op).unwrap_or(v1::ComparisonOp::Unspecified) {
        v1::ComparisonOp::Lt => quote! { < },
        v1::ComparisonOp::Le => quote! { <= },
        v1::ComparisonOp::Gt => quote! { > },
        v1::ComparisonOp::Ge => quote! { >= },
        v1::ComparisonOp::Eq => quote! { == },
        // `Unspecified` is unreachable from the lowering, which writes one of
        // the six it accepted. Kept total.
        v1::ComparisonOp::Ne | v1::ComparisonOp::Unspecified => quote! { != },
    }
}

fn render_one(
    ctx: &Ctx,
    clause: &v1::Clause,
    params: &[v1::Param],
    reply: Option<&v1::TypeRef>,
) -> Result<Rendered, GenerateError> {
    let source = clause.source.as_str();
    let comparison = match clause.translation.as_ref() {
        Some(v1::clause::Translation::Comparison(comparison)) => comparison,
        // The lowering states the reason it refused, and this printer prints
        // it: a refusal reaches the generated source as the note
        // `skipped_interface_note` writes.
        Some(v1::clause::Translation::Refused(reason)) => return Err(refuse(source, reason)),
        None => return Err(refuse(source, "no accepted comparison")),
    };

    match comparison.subject.as_ref() {
        Some(v1::comparison::Subject::Result(_)) => {
            let backing = reply
                .and_then(|reference| scalar_backing(ctx, reference))
                .ok_or_else(|| refuse(source, "`result` is not a named integer or float scalar"))?;
            let value = literal_tokens(&comparison.literal, backing).ok_or_else(|| {
                refuse(
                    source,
                    "the literal does not match the reply's numeric type",
                )
            })?;
            let op = operator(comparison.op);
            Ok(Rendered {
                expr: quote! { reply.0 #op #value },
                subject: Subject::Reply,
            })
        }
        Some(v1::comparison::Subject::Param(index)) => {
            let backing = params
                .get(*index as usize)
                .and_then(param_named_type)
                .and_then(|reference| scalar_backing(ctx, reference))
                .ok_or_else(|| {
                    refuse(
                        source,
                        "the parameter is not a named integer or float scalar",
                    )
                })?;
            let value = literal_tokens(&comparison.literal, backing).ok_or_else(|| {
                refuse(
                    source,
                    "the literal does not match the parameter's numeric type",
                )
            })?;
            let op = operator(comparison.op);
            Ok(Rendered {
                expr: quote! { args.0 #op #value },
                subject: Subject::Arg,
            })
        }
        None => Err(refuse(source, "no accepted comparison")),
    }
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

/// The backing of a **same-package** named scalar, restricted to the integer
/// and float classes the accepted form allows.
///
/// The lowering resolves a reference over the whole scope, and this backend
/// resolves nothing across packages: a subject whose scalar is declared in
/// another package is refused here, which is what it was before stage P4 and
/// what byte identity requires. Lifting it is a change to what the generated
/// crate contains, made on its own (design note §9 item 6).
fn scalar_backing(ctx: &Ctx, reference: &v1::TypeRef) -> Option<ScalarBacking> {
    let Some(v1::declaration::Kind::Scalar(sc)) = ctx.local(reference)?.kind.as_ref() else {
        return None;
    };
    match class_backing(sc.class) {
        backing @ (ScalarBacking::Integer | ScalarBacking::Float) => Some(backing),
        _ => None,
    }
}

fn param_named_type(param: &v1::Param) -> Option<&v1::TypeRef> {
    match param.r#type.as_ref()?.kind.as_ref()? {
        v1::r#type::Kind::Named(reference) => Some(reference),
        _ => None,
    }
}

/// The opening of every refusal this translator raises.
///
/// `skipped_interface_note` reads it to tell a clause gap from a call-shape
/// one, because the refusal that was actually raised is the only thing that
/// says which gap an interface was skipped for — reading the interface
/// instead reports the wrong owner when it has a call-shape gap on one
/// interaction and an untranslatable clause on another, which the corpus's
/// own `veh.cluster.VehicleStatus` does. Sharing the constant is what keeps a
/// rewording from reclassifying silently: change this and the note's match
/// changes with it.
pub(crate) const CLAUSE_REFUSAL: &str = "cannot translate contract clause";

fn refuse(source: &str, reason: &str) -> GenerateError {
    GenerateError {
        message: format!("{CLAUSE_REFUSAL} `{source}`: {reason}"),
    }
}

#[cfg(test)]
mod tests {
    use super::{ClauseKind, translate};
    use crate::Ctx;
    use ridl_ir::codegen::v1;
    use ridl_ir::v2;

    /// A package declaring one integer scalar, one float scalar and one
    /// interface whose single call carries the parameters and the clauses
    /// under test. The clause translation is a model fact since stage P4, so
    /// a test states the source the lowering reads and reads the translation
    /// back out of the model.
    fn call_package(
        params: Vec<v2::Param>,
        contracts: Vec<v2::Contract>,
        reply: Option<&str>,
    ) -> v2::Package {
        let kind = match reply {
            Some(reply) => v2::decl::Kind::QueryDef(v2::QueryDef {
                params,
                return_type: Some(v2::ReturnType {
                    kind: Some(v2::return_type::Kind::Value(v2::FieldType {
                        optional: false,
                        kind: Some(v2::field_type::Kind::Named(reply.to_string())),
                    })),
                }),
                timing: None,
                contracts,
            }),
            None => v2::decl::Kind::CommandDef(v2::CommandDef {
                params,
                timing: None,
                contracts,
            }),
        };
        v2::Package {
            name: "p".to_string(),
            decls: vec![
                scalar_decl("Level", v2::PrimitiveType::Integer),
                scalar_decl("Rate", v2::PrimitiveType::Float),
            ],
            interfaces: vec![v2::Interface {
                name: "Iface".to_string(),
                visibility: v2::Visibility::Public as i32,
                doc: String::new(),
                labels: Vec::new(),
                deprecated: None,
                number: 1,
                provisional: false,
                interactions: vec![v2::Decl {
                    name: "call".to_string(),
                    visibility: v2::Visibility::Public as i32,
                    is_error: false,
                    doc: String::new(),
                    labels: Vec::new(),
                    deprecated: None,
                    ordinal: 1,
                    kind: Some(kind),
                }],
            }],
            services: Vec::new(),
            retired: Vec::new(),
        }
    }

    /// The lowered parameters, clauses and reply reference of the one call.
    fn shape_of(model: &v1::Model) -> (&[v1::Param], &[v1::Clause], Option<&v1::TypeRef>) {
        let slot = &model.interfaces[0].slots[0];
        let Some(v1::interaction_slot::Occupant::Interaction(interaction)) = slot.occupant.as_ref()
        else {
            panic!("the lowered interface carries no interaction");
        };
        match interaction.shape.as_ref() {
            Some(v1::interaction::Shape::Command(command)) => {
                (&command.params, &command.clauses, None)
            }
            Some(v1::interaction::Shape::Query(query)) => (
                &query.params,
                &query.clauses,
                query
                    .reply_payload
                    .as_ref()
                    .and_then(|payload| payload.r#type.as_ref()),
            ),
            _ => panic!("the lowered interaction is neither a command nor a query"),
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
        let package = call_package(
            vec![param("level", "Level")],
            vec![clause(v2::ContractKind::Require, "level < 100")],
            None,
        );
        let model = ridl_ir::codegen::lower(&package, &[]);
        let ctx = Ctx::new(&package, &model);
        let (params, clauses, reply) = shape_of(&model);

        let body = translate(&ctx, clauses, ClauseKind::Require, params, reply).expect("accepted");
        assert!(body.uses_args);
        assert!(!body.uses_reply);
        assert_eq!(
            dense(&body.expr),
            "ifargs.0<100{::core::result::Result::Ok(())}else{::core::result::Result::Err(())}"
        );
    }

    /// Pins `Comparison::Le`'s emitted operator: `<=`, not `<` or any other
    /// token. Before this test, `<=` appeared in no test and no fixture, so a
    /// mutation emitting `<` for `Comparison::Le` passed the whole suite.
    #[test]
    fn a_le_comparison_emits_the_le_operator() {
        let package = call_package(
            vec![param("level", "Level")],
            vec![clause(v2::ContractKind::Require, "level <= 100")],
            None,
        );
        let model = ridl_ir::codegen::lower(&package, &[]);
        let ctx = Ctx::new(&package, &model);
        let (params, clauses, reply) = shape_of(&model);

        let body = translate(&ctx, clauses, ClauseKind::Require, params, reply).expect("accepted");
        assert_eq!(
            dense(&body.expr),
            "ifargs.0<=100{::core::result::Result::Ok(())}else{::core::result::Result::Err(())}"
        );
    }

    /// Pins `Comparison::Eq`'s emitted operator: `==`, not `!=` or any other
    /// token. Before this test, `==` appeared in no test and no fixture, so a
    /// mutation swapping `Comparison::Eq` and `Comparison::Ne` passed the whole
    /// suite.
    #[test]
    fn an_eq_comparison_emits_the_eq_operator() {
        let package = call_package(
            vec![param("level", "Level")],
            vec![clause(v2::ContractKind::Require, "level == 100")],
            None,
        );
        let model = ridl_ir::codegen::lower(&package, &[]);
        let ctx = Ctx::new(&package, &model);
        let (params, clauses, reply) = shape_of(&model);

        let body = translate(&ctx, clauses, ClauseKind::Require, params, reply).expect("accepted");
        assert_eq!(
            dense(&body.expr),
            "ifargs.0==100{::core::result::Result::Ok(())}else{::core::result::Result::Err(())}"
        );
    }

    /// Pins `Comparison::Ne`'s emitted operator: `!=`, not `==` or any other
    /// token. Before this test, `!=` appeared in no test and no fixture, so a
    /// mutation swapping `Comparison::Eq` and `Comparison::Ne` passed the whole
    /// suite.
    #[test]
    fn a_ne_comparison_emits_the_ne_operator() {
        let package = call_package(
            vec![param("level", "Level")],
            vec![clause(v2::ContractKind::Require, "level != 100")],
            None,
        );
        let model = ridl_ir::codegen::lower(&package, &[]);
        let ctx = Ctx::new(&package, &model);
        let (params, clauses, reply) = shape_of(&model);

        let body = translate(&ctx, clauses, ClauseKind::Require, params, reply).expect("accepted");
        assert_eq!(
            dense(&body.expr),
            "ifargs.0!=100{::core::result::Result::Ok(())}else{::core::result::Result::Err(())}"
        );
    }

    #[test]
    fn a_float_backed_result_clause_emits_a_float_literal() {
        let package = call_package(
            vec![param("window", "Level")],
            vec![clause(v2::ContractKind::Ensure, "result >= 0")],
            Some("Rate"),
        );
        let model = ridl_ir::codegen::lower(&package, &[]);
        let ctx = Ctx::new(&package, &model);
        let (params, clauses, reply) = shape_of(&model);

        let body = translate(&ctx, clauses, ClauseKind::Ensure, params, reply).expect("accepted");
        assert!(body.uses_reply);
        assert!(!body.uses_args);
        // Rate is float-backed, so the literal is a float.
        assert_eq!(
            dense(&body.expr),
            "ifreply.0>=0.0{::core::result::Result::Ok(())}else{::core::result::Result::Err(())}"
        );
    }

    #[test]
    fn several_clauses_of_one_kind_are_conjoined() {
        let package = call_package(
            vec![param("level", "Level")],
            vec![
                clause(v2::ContractKind::Require, "level < 100"),
                clause(v2::ContractKind::Require, "level >= 0"),
            ],
            None,
        );
        let model = ridl_ir::codegen::lower(&package, &[]);
        let ctx = Ctx::new(&package, &model);
        let (params, clauses, reply) = shape_of(&model);

        let body = translate(&ctx, clauses, ClauseKind::Require, params, reply).expect("accepted");
        assert_eq!(
            dense(&body.expr),
            "ifargs.0<100&&args.0>=0{::core::result::Result::Ok(())}else{::core::result::Result::Err(())}"
        );
    }

    #[test]
    fn no_clause_of_a_kind_emits_ok() {
        let package = call_package(
            vec![param("level", "Level")],
            vec![clause(v2::ContractKind::Require, "level < 100")],
            Some("Level"),
        );
        let model = ridl_ir::codegen::lower(&package, &[]);
        let ctx = Ctx::new(&package, &model);
        let (params, clauses, reply) = shape_of(&model);

        // No ensure clause present.
        let body = translate(&ctx, clauses, ClauseKind::Ensure, params, reply).expect("empty");
        assert!(!body.uses_args);
        assert!(!body.uses_reply);
        assert_eq!(dense(&body.expr), "::core::result::Result::Ok(())");
    }

    #[test]
    fn a_compound_clause_is_refused() {
        let package = call_package(
            vec![param("level", "Level")],
            vec![clause(
                v2::ContractKind::Require,
                "level < 100 || level == 0",
            )],
            None,
        );
        let model = ridl_ir::codegen::lower(&package, &[]);
        let ctx = Ctx::new(&package, &model);
        let (params, clauses, reply) = shape_of(&model);

        let error =
            translate(&ctx, clauses, ClauseKind::Require, params, reply).expect_err("refused");
        assert!(error.message.contains("cannot translate contract clause"));
    }

    #[test]
    fn a_unit_literal_is_refused() {
        let package = call_package(
            vec![param("window", "Level")],
            vec![clause(v2::ContractKind::Require, "window > 0ms")],
            None,
        );
        let model = ridl_ir::codegen::lower(&package, &[]);
        let ctx = Ctx::new(&package, &model);
        let (params, clauses, reply) = shape_of(&model);

        translate(&ctx, clauses, ClauseKind::Require, params, reply).expect_err("refused");
    }

    #[test]
    fn a_result_subject_outside_an_ensure_is_refused() {
        let package = call_package(
            vec![param("level", "Level")],
            // `result` with no reply type in scope (a require clause) is
            // refused.
            vec![clause(v2::ContractKind::Require, "result >= 0")],
            None,
        );
        let model = ridl_ir::codegen::lower(&package, &[]);
        let ctx = Ctx::new(&package, &model);
        let (params, clauses, reply) = shape_of(&model);

        translate(&ctx, clauses, ClauseKind::Require, params, reply).expect_err("refused");
    }

    #[test]
    fn a_fractional_literal_on_an_integer_subject_is_refused() {
        let package = call_package(
            vec![param("level", "Level")],
            vec![clause(v2::ContractKind::Require, "level < 1.5")],
            None,
        );
        let model = ridl_ir::codegen::lower(&package, &[]);
        let ctx = Ctx::new(&package, &model);
        let (params, clauses, reply) = shape_of(&model);

        translate(&ctx, clauses, ClauseKind::Require, params, reply).expect_err("refused");
    }
}
