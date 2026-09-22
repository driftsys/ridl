//! The generated interaction-face descriptor layer (Lane M stage M3, Task 1,
//! `docs/design/interaction-face.md`; the approved design is
//! `docs/archive/2026-09-16-interaction-face-v0-design.md` §6).
//!
//! For each named interface in a package this module emits, over the
//! `ridl-rt` runtime crate named by its absolute path (`::ridl_rt::…`):
//!
//! - one unit struct per interface with an [`Interface`](::ridl_rt) impl
//!   carrying `CATALOG`, `NUMBER`, `PROVISIONAL`, `NAME` and `MEMBERS`, plus
//!   the `MAX_BUFFER_SIZE` and `EVENT_SOURCE_BUFFER_SIZE` associated
//!   constants;
//! - one unit struct per interaction with an `Interaction` impl (both required
//!   items, `type Iface` and `const MEMBER`) and the kind's trait — `Signal`,
//!   `Event`, `Command`, `Query` or `Fixed`.
//!
//! Since E11.14 it is reached from [`crate::generate_pipeline`], which is
//! what `ridl build --emit rust` calls, as well as from
//! [`crate::generate_face`]. It is not reached from [`crate::generate`],
//! whose output is unchanged and carries neither the descriptors nor the
//! face — which is what is unchanged about it, rather than its bytes (see
//! the entry points' own documentation).

use crate::clauses::{self, ClauseKind};
use crate::{Ctx, GenerateError, camel_of as camel, declared, ident, type_path};
use proc_macro2::{Literal, TokenStream};
use quote::quote;
use ridl_ir::codegen::v1;

/// The descriptor and face items for every named interface of the lowered
/// model, in source order.
///
/// The model's `interfaces` list is `Package::shapes()` order, so an interface
/// enumerated here is never accidentally the incomplete `Package::interfaces`
/// field (the shape-walk guard). A service's inline shape is skipped: its
/// identity name is the dotted service name, which is not a single Rust
/// identifier, and descriptors for an inline shape are a follow-up beyond M3.
pub(crate) fn interface_items(ctx: &Ctx) -> Result<Vec<TokenStream>, GenerateError> {
    let mut items = Vec::new();
    for interface in &ctx.model.interfaces {
        if declared_name(interface).is_none() {
            continue;
        }
        one_interface(ctx, interface, &mut items)?;
    }
    Ok(items)
}

/// The declared name of one interface, or `None` for a service's inline
/// shape, whose identity is the service's dotted name.
pub(crate) fn declared_name(interface: &v1::Interface) -> Option<&str> {
    match interface.identity.as_ref() {
        Some(v1::interface::Identity::Declared(name)) => Some(name.declared.as_str()),
        _ => None,
    }
}

/// The live interactions of one interface, in slot order. A reserved
/// tombstone occupies an ordinal and is no interaction.
pub(crate) fn interactions(interface: &v1::Interface) -> Vec<(u32, &v1::Interaction)> {
    interface
        .slots
        .iter()
        .filter_map(|slot| match slot.occupant.as_ref() {
            Some(v1::interaction_slot::Occupant::Interaction(interaction)) => {
                Some((slot.ordinal, interaction.as_ref()))
            }
            _ => None,
        })
        .collect()
}

/// The descriptor items of one interface, for the pipeline's per-interface
/// walk (E11.14 decision 2). [`interface_items`] is the whole-package walk;
/// this is one shape of it, so a caller that means to skip a refusing
/// interface can catch the refusal at the interface it belongs to.
pub(crate) fn one_interface_items(
    ctx: &Ctx,
    interface: &v1::Interface,
) -> Result<Vec<TokenStream>, GenerateError> {
    let mut items = Vec::new();
    one_interface(ctx, interface, &mut items)?;
    Ok(items)
}

fn one_interface(
    ctx: &Ctx,
    interface: &v1::Interface,
    items: &mut Vec<TokenStream>,
) -> Result<(), GenerateError> {
    let package_name = ctx.package_name();
    let iface_name = declared_name(interface).unwrap_or_default();
    let iface_ident = ident(iface_name);

    // Reserved tombstones get no row, so a row's index is not its ordinal
    // minus one — the index is its position among the live interactions,
    // which the lowering states as `Interaction.row`.
    let interactions = interactions(interface);

    let mut member_rows: Vec<TokenStream> = Vec::new();
    let mut call_sizes: Vec<TokenStream> = Vec::new();
    let mut event_sizes: Vec<TokenStream> = Vec::new();
    let mut interaction_items: Vec<TokenStream> = Vec::new();

    for (ordinal, interaction) in &interactions {
        member_rows.push(member_row(*ordinal, interaction)?);
        interaction_items.push(interaction_item(
            ctx,
            &iface_ident,
            interaction,
            interaction.row as usize,
            &mut call_sizes,
            &mut event_sizes,
        )?);
    }

    let number = Literal::u32_unsuffixed(interface.number);
    let provisional = interface.provisional;
    let max_buffer = max_size_const(&call_sizes);
    let event_buffer = max_size_const(&event_sizes);

    let interface_doc = format!(
        "Descriptor for interface `{iface_name}`.\n\n\
         `CATALOG.hash` is the placeholder `CatalogHash([0u8; 32])` until E16.2 \
         (driftsys/ridl#378) computes the real catalog hash.\n\n\
         Every `PayloadInfo.max_size` field is `None`. The reading here is that \
         the toolchain cannot size the payload yet, not that the encoding \
         cannot carry it. `ridl-rt`'s own doc comment states the other reading; \
         E16.2 reconciles the two."
    );

    items.push(quote! {
        #[doc = #interface_doc]
        pub struct #iface_ident;
    });

    items.push(quote! {
        impl ::ridl_rt::contract::Interface for #iface_ident {
            const CATALOG: &'static ::ridl_rt::contract::CatalogRef =
                &::ridl_rt::contract::CatalogRef {
                    name: #package_name,
                    hash: ::ridl_rt::contract::CatalogHash([0u8; 32]),
                };
            const NUMBER: ::ridl_rt::contract::InterfaceNo =
                ::ridl_rt::contract::InterfaceNo(#number);
            const PROVISIONAL: bool = #provisional;
            const NAME: &'static str = #iface_name;
            const MEMBERS: &'static [::ridl_rt::contract::Member] = &[
                #(#member_rows),*
            ];
        }
    });

    let max_buffer_doc = "The largest argument or reply payload of this \
        interface, over `<T as Payload<Wire>>::MAX_SIZE`. A dispatch buffer \
        must be at least this large, because a reply is encoded into the same \
        buffer as the arguments. `0` when the interface declares no call.";
    let event_buffer_doc = "The largest event payload of this interface, over \
        `<T as Payload<Wire>>::MAX_SIZE`. `0` when the interface declares no \
        event.";

    items.push(quote! {
        impl #iface_ident {
            #[doc = #max_buffer_doc]
            pub const MAX_BUFFER_SIZE: usize = #max_buffer;
            #[doc = #event_buffer_doc]
            pub const EVENT_SOURCE_BUFFER_SIZE: usize = #event_buffer;
        }
    });

    items.extend(interaction_items);
    Ok(())
}

/// The `Member` row for one interaction.
fn member_row(slot: u32, interaction: &v1::Interaction) -> Result<TokenStream, GenerateError> {
    let ordinal = Literal::u32_unsuffixed(slot);
    let member = declared(interaction.name.as_ref());
    let name = member;
    let timing = timing_tokens(interaction.timing.as_ref());

    let (kind, timing, payloads) = match interaction.shape.as_ref() {
        Some(v1::interaction::Shape::Signal(signal)) => (
            quote! { ::ridl_rt::contract::Kind::Signal },
            timing,
            vec![payload_info(payload_reference(signal.payload.as_ref()))],
        ),
        Some(v1::interaction::Shape::Event(event)) => (
            quote! { ::ridl_rt::contract::Kind::Event },
            timing,
            vec![payload_info(payload_reference(event.payload.as_ref()))],
        ),
        Some(v1::interaction::Shape::Command(command)) => (
            quote! { ::ridl_rt::contract::Kind::Command },
            timing,
            vec![payload_info(single_param_type(command, member)?)],
        ),
        Some(v1::interaction::Shape::Query(query)) => (
            quote! { ::ridl_rt::contract::Kind::Query },
            timing,
            vec![
                payload_info(query_param_type(query, member)?),
                payload_info(query_reply_type(query, member)?),
            ],
        ),
        Some(v1::interaction::Shape::Fixed(fixed)) => (
            quote! { ::ridl_rt::contract::Kind::Fixed },
            quote! { None },
            vec![payload_info(fixed_payload_type(fixed, member)?)],
        ),
        None => return Err(not_an_interaction(member)),
    };

    Ok(quote! {
        ::ridl_rt::contract::Member {
            ordinal: ::ridl_rt::contract::Ordinal(#ordinal),
            kind: #kind,
            name: #name,
            timing: #timing,
            payloads: &[ #(#payloads),* ],
        }
    })
}

/// One interaction's unit struct, its `Interaction` impl, and its kind trait
/// impl. Appends the interaction's argument, reply and event payload sizes to
/// the interface's two buffer-constant worklists.
fn interaction_item(
    ctx: &Ctx,
    iface_ident: &proc_macro2::Ident,
    interaction: &v1::Interaction,
    row_index: usize,
    call_sizes: &mut Vec<TokenStream>,
    event_sizes: &mut Vec<TokenStream>,
) -> Result<TokenStream, GenerateError> {
    let member = declared(interaction.name.as_ref());
    let struct_ident = ident(&format!(
        "{iface_ident}{}",
        camel(interaction.name.as_ref())
    ));
    let index = Literal::usize_unsuffixed(row_index);

    let kind_impl = match interaction.shape.as_ref() {
        Some(v1::interaction::Shape::Signal(signal)) => {
            let payload = type_path(payload_reference(signal.payload.as_ref()));
            // `init()` returns the payload type's default, which is the
            // channel init (ridl §4.4) for a signal with no `= value`
            // override — the fixture's case. A declared init override is a
            // follow-up; the payload type derives `Default` here.
            quote! {
                impl ::ridl_rt::contract::Signal for #struct_ident {
                    type Payload = #payload;
                    fn init() -> Self::Payload {
                        #payload::default()
                    }
                }
            }
        }
        Some(v1::interaction::Shape::Event(event)) => {
            let reference = payload_reference(event.payload.as_ref());
            let payload = type_path(reference);
            event_sizes.push(max_size_path(reference));
            quote! {
                impl ::ridl_rt::contract::Event for #struct_ident {
                    type Payload = #payload;
                }
            }
        }
        Some(v1::interaction::Shape::Fixed(fixed)) => {
            let payload = type_path(fixed_payload_type(fixed, member)?);
            quote! {
                impl ::ridl_rt::contract::Fixed for #struct_ident {
                    type Payload = #payload;
                }
            }
        }
        Some(v1::interaction::Shape::Command(command)) => {
            let arg_name = single_param_type(command, member)?;
            let args = type_path(arg_name);
            call_sizes.push(max_size_path(arg_name));
            let require = clauses::translate(
                ctx,
                &command.clauses,
                ClauseKind::Require,
                &command.params,
                None,
            )?;
            let args_param = binding("args", require.uses_args);
            let require_expr = require.expr;
            let require_doc = clause_doc("require");
            quote! {
                impl ::ridl_rt::contract::Command for #struct_ident {
                    type Args = #args;
                    #[doc = #require_doc]
                    fn require(#args_param: &Self::Args) -> ::core::result::Result<(), ()> {
                        #require_expr
                    }
                }
            }
        }
        Some(v1::interaction::Shape::Query(query)) => {
            let arg_name = query_param_type(query, member)?;
            let reply_name = query_reply_type(query, member)?;
            let args = type_path(arg_name);
            let reply = type_path(reply_name);
            call_sizes.push(max_size_path(arg_name));
            call_sizes.push(max_size_path(reply_name));

            let require = clauses::translate(
                ctx,
                &query.clauses,
                ClauseKind::Require,
                &query.params,
                None,
            )?;
            let ensure = clauses::translate(
                ctx,
                &query.clauses,
                ClauseKind::Ensure,
                &query.params,
                query
                    .reply_payload
                    .as_ref()
                    .and_then(|payload| payload.r#type.as_ref()),
            )?;

            let require_param = binding("args", require.uses_args);
            let ensure_args = binding("args", ensure.uses_args);
            let ensure_reply = binding("reply", ensure.uses_reply);
            let require_expr = require.expr;
            let ensure_expr = ensure.expr;
            let require_doc = clause_doc("require");
            let ensure_doc = clause_doc("ensure");
            quote! {
                impl ::ridl_rt::contract::Query for #struct_ident {
                    type Args = #args;
                    type Reply = #reply;
                    #[doc = #require_doc]
                    fn require(#require_param: &Self::Args) -> ::core::result::Result<(), ()> {
                        #require_expr
                    }
                    #[doc = #ensure_doc]
                    fn ensure(
                        #ensure_args: &Self::Args,
                        #ensure_reply: &Self::Reply,
                    ) -> ::core::result::Result<(), ()> {
                        #ensure_expr
                    }
                }
            }
        }
        None => return Err(not_an_interaction(member)),
    };

    Ok(quote! {
        pub struct #struct_ident;

        impl ::ridl_rt::contract::Interaction for #struct_ident {
            type Iface = #iface_ident;
            const MEMBER: &'static ::ridl_rt::contract::Member =
                &<#iface_ident as ::ridl_rt::contract::Interface>::MEMBERS[#index];
        }

        #kind_impl
    })
}

/// A `PayloadInfo` with all three encoded sizes absent (the M3 placeholder).
fn payload_info(type_name: &str) -> TokenStream {
    quote! {
        ::ridl_rt::contract::PayloadInfo {
            type_name: #type_name,
            max_size: ::ridl_rt::contract::EncodedSizes {
                proto3: None,
                flatbuffers: None,
                repr_c: None,
            },
        }
    }
}

/// The `<T as Payload<Wire>>::MAX_SIZE` const-evaluable path for a payload
/// type. Buffer sizes are written in terms of it, never as a literal, so a
/// buffer is sized by the codec's own bound rather than by a number this
/// emitter would have to keep equal to it. `Wire` is the package's own alias
/// (design note D-11), emitted at the same module scope these descriptors
/// are.
fn max_size_path(type_name: &str) -> TokenStream {
    let path = type_path(type_name);
    quote! {
        <#path as ::ridl_rt::payload::Payload<Wire>>::MAX_SIZE
    }
}

/// A const-evaluable maximum over the payload sizes, or `0` when there are
/// none.
fn max_size_const(sizes: &[TokenStream]) -> TokenStream {
    if sizes.is_empty() {
        return quote! { 0usize };
    }
    quote! {
        {
            let sizes = [ #(#sizes),* ];
            let mut max = 0usize;
            let mut index = 0usize;
            while index < sizes.len() {
                if sizes[index] > max {
                    max = sizes[index];
                }
                index += 1;
            }
            max
        }
    }
}

fn timing_tokens(timing: Option<&v1::Timing>) -> TokenStream {
    let Some(timing) = timing else {
        return quote! { None };
    };
    let mode = match v1::TimingMode::try_from(timing.mode).unwrap_or(v1::TimingMode::Unspecified) {
        v1::TimingMode::StrictPeriodic => {
            quote! { ::ridl_rt::contract::TimingMode::StrictPeriodic }
        }
        _ => quote! { ::ridl_rt::contract::TimingMode::Range },
    };
    let min = duration_tokens(timing.min_us.as_deref());
    let max = duration_tokens(timing.max_us.as_deref());
    quote! {
        Some(::ridl_rt::contract::Timing { mode: #mode, min: #min, max: #max })
    }
}

fn duration_tokens(micros: Option<&str>) -> TokenStream {
    match micros {
        None => quote! { None },
        Some(text) => {
            let value = parse_micros(text);
            let literal = Literal::i64_unsuffixed(value);
            quote! { Some(::ridl_rt::sample::Duration(#literal)) }
        }
    }
}

/// Parses an exact-decimal microsecond string to the `i64` microseconds a
/// `Duration` holds. A whole-microsecond value parses directly; a fractional
/// one is rounded, which the fixture never exercises.
fn parse_micros(text: &str) -> i64 {
    if let Ok(value) = text.parse::<i64>() {
        value
    } else if let Ok(value) = text.parse::<f64>() {
        value.round() as i64
    } else {
        0
    }
}

/// The method parameter binding: the given name when the body reads it, or the
/// underscored name when it does not, so an unread parameter draws no
/// `unused_variables` warning.
fn binding(name: &str, used: bool) -> TokenStream {
    let ident = if used {
        ident(name)
    } else {
        ident(&format!("_{name}"))
    };
    quote! { #ident }
}

fn clause_doc(kind: &str) -> String {
    format!(
        "Evaluates the `{kind}` clauses. This translation covers one clause \
         form — `<subject> <comparison> <numeric literal>` — and E5.1 replaces \
         it with one driven by the structured expression tree."
    )
}

/// The canonical IR text of the type one payload names.
fn payload_reference(payload: Option<&v1::Payload>) -> &str {
    payload
        .and_then(|payload| payload.r#type.as_ref())
        .map(|reference| reference.reference.as_str())
        .unwrap_or_default()
}

/// The single declared parameter's named type. M3 emits no induced argument
/// struct, so a call with any other parameter shape is refused (a recorded
/// follow-up).
///
/// The lowering states the request payload exactly when the call has that
/// shape, so its absence is the refusal, and the parameters say which of the
/// two reasons it is.
pub(crate) fn single_param_type<'a>(
    command: &'a v1::CommandShape,
    member: &str,
) -> Result<&'a str, GenerateError> {
    match command.request.as_ref() {
        Some(request) => Ok(payload_reference(Some(request))),
        None => Err(no_single_param(&command.params, member)),
    }
}

/// [`single_param_type`] for a query, which carries the same request payload.
pub(crate) fn query_param_type<'a>(
    query: &'a v1::QueryShape,
    member: &str,
) -> Result<&'a str, GenerateError> {
    match query.request.as_ref() {
        Some(request) => Ok(payload_reference(Some(request))),
        None => Err(no_single_param(&query.params, member)),
    }
}

/// Which of the two reasons a call has no request payload.
fn no_single_param(params: &[v1::Param], member: &str) -> GenerateError {
    if params.len() != 1 {
        return GenerateError {
            message: format!(
                "interaction `{member}` must declare exactly one parameter (M3 emits no \
                 induced argument struct)"
            ),
        };
    }
    GenerateError {
        message: format!("interaction `{member}`'s parameter must be a named type"),
    }
}

/// A query's reply named type. M3 replies with one declared type, so an inline
/// fallible or non-named return is refused.
pub(crate) fn query_reply_type<'a>(
    query: &'a v1::QueryShape,
    member: &str,
) -> Result<&'a str, GenerateError> {
    if let Some(payload) = query.reply_payload.as_ref() {
        return Ok(payload_reference(Some(payload)));
    }
    match query.reply.as_ref().and_then(|reply| reply.kind.as_ref()) {
        Some(v1::reply::Kind::Value(_)) => Err(GenerateError {
            message: format!("query `{member}`'s reply must be a named type"),
        }),
        _ => Err(GenerateError {
            message: format!("query `{member}`'s reply must be a single declared type"),
        }),
    }
}

/// A `fixed`'s payload named type. M3 provisions a named type.
fn fixed_payload_type<'a>(
    fixed: &'a v1::FixedShape,
    member: &str,
) -> Result<&'a str, GenerateError> {
    match fixed.named.as_ref() {
        Some(named) => Ok(payload_reference(Some(named))),
        None => Err(GenerateError {
            message: format!("fixed `{member}`'s payload must be a named type"),
        }),
    }
}

fn not_an_interaction(member: &str) -> GenerateError {
    GenerateError {
        message: format!("declaration `{member}` is not an interaction"),
    }
}

#[cfg(test)]
mod tests {
    use crate::generate_face;
    use ridl_ir::v2;

    /// `max_size_const` must compute the maximum of its inputs, not their sum,
    /// minimum, first, or last — design §6 requires `MAX_BUFFER_SIZE` and
    /// `EVENT_SOURCE_BUFFER_SIZE` to be the maximum over the relevant
    /// `<T as Payload<Wire>>::MAX_SIZE` values.
    ///
    /// `max_size_const` emits a const-evaluable block, not a literal number, so
    /// asserting on the emitted token text cannot distinguish "compute the
    /// maximum" from "compute the sum": both reductions are expressed by
    /// different token text for the same loop body, and the loop body's token
    /// text does not, by itself, say what number the loop computes. Splicing
    /// the emitted block into a real `const` and compiling and running it is
    /// what makes the actual computed number observable. `[3, 9, 1, 5]` was
    /// chosen so the maximum (9), the sum (18), the minimum (1), the first (3)
    /// and the last (5) are all pairwise distinct — three elements cannot do
    /// this: with only a first, a middle and a last position, one boundary
    /// position is always either the overall minimum or the overall maximum.
    #[test]
    fn max_size_const_computes_the_maximum_not_a_different_reduction() {
        use proc_macro2::Literal;
        use quote::quote;

        let sizes: Vec<super::TokenStream> = [3usize, 9usize, 1usize, 5usize]
            .into_iter()
            .map(|n| {
                let literal = Literal::usize_unsuffixed(n);
                quote! { #literal }
            })
            .collect();
        let computed = super::max_size_const(&sizes);

        let source = quote! {
            const COMPUTED: usize = #computed;
            fn main() {
                println!("{COMPUTED}");
            }
        }
        .to_string();

        let dir = tempfile::tempdir().expect("a temp dir is created");
        let source_path = dir.path().join("max_size_const.rs");
        std::fs::write(&source_path, &source).expect("the generated source is written");
        let binary_path = dir.path().join("max_size_const_bin");
        let status = std::process::Command::new("rustc")
            .args(["--edition", "2024", "--crate-type", "bin"])
            .arg("-o")
            .arg(&binary_path)
            .arg(&source_path)
            .status()
            .expect("rustc must be installed and runnable for this test to be meaningful");
        assert!(status.success(), "generated source must compile:\n{source}");

        let output = std::process::Command::new(&binary_path)
            .output()
            .expect("the compiled binary runs");
        let printed = String::from_utf8(output.stdout)
            .expect("stdout is utf8")
            .trim()
            .to_string();

        assert_eq!(
            printed, "9",
            "max_size_const over [3, 9, 1, 5] must compute the maximum (9); got {printed}"
        );
    }

    /// The fixture declares no `fixed` interaction, so the `Fixed` descriptor
    /// path is covered here over hand-built IR (design §7, plan Task 1).
    #[test]
    fn a_fixed_interaction_emits_a_fixed_descriptor() {
        let payload = v2::Decl {
            name: "Provisioned".to_string(),
            visibility: v2::Visibility::Public as i32,
            is_error: false,
            doc: String::new(),
            labels: Vec::new(),
            deprecated: None,
            ordinal: 0,
            kind: Some(v2::decl::Kind::TypeDef(v2::TypeDef {
                backing: Some(v2::Backing {
                    kind: Some(v2::backing::Kind::Primitive(
                        v2::PrimitiveType::Integer as i32,
                    )),
                }),
                constraint: None,
                declared_init: None,
                init: Some(v2::InitValue {
                    derivable: true,
                    value: Some("0".to_string()),
                }),
                width: Some(v2::type_def::Width::IntWidth(v2::IntWidth::U16 as i32)),
            })),
        };

        let fixed = v2::Decl {
            name: "brightness".to_string(),
            visibility: v2::Visibility::Public as i32,
            is_error: false,
            doc: String::new(),
            labels: Vec::new(),
            deprecated: None,
            ordinal: 1,
            kind: Some(v2::decl::Kind::FixedDef(v2::FixedDef {
                payload: Some(v2::FieldType {
                    optional: false,
                    kind: Some(v2::field_type::Kind::Named("Provisioned".to_string())),
                }),
            })),
        };

        let interface = v2::Interface {
            name: "Panel".to_string(),
            visibility: v2::Visibility::Public as i32,
            doc: String::new(),
            labels: Vec::new(),
            deprecated: None,
            interactions: vec![fixed],
            number: 7,
            provisional: true,
        };

        let package = v2::Package {
            name: "p".to_string(),
            decls: vec![payload],
            interfaces: vec![interface],
            services: Vec::new(),
            retired: Vec::new(),
        };

        let source = generate_face(&package).expect("generate_face").rust_source;
        let dense: String = source.chars().filter(|c| !c.is_whitespace()).collect();

        assert!(dense.contains("pubstructPanelBrightness;"));
        assert!(dense.contains("impl::ridl_rt::contract::FixedforPanelBrightness"));
        assert!(dense.contains("typePayload=Provisioned;"));
        assert!(dense.contains("::ridl_rt::contract::Kind::Fixed"));
        // A fixed carries no timing.
        assert!(dense.contains("name:\"brightness\",timing:None"));
        // A fixed is not a call or an event, so both buffer constants are 0.
        assert!(dense.contains("MAX_BUFFER_SIZE:usize=0usize"));
        assert!(dense.contains("EVENT_SOURCE_BUFFER_SIZE:usize=0usize"));
    }
}
