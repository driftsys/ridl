//! The generated interaction-face descriptor layer
//! (`docs/design/interaction-face.md`; the approved design is
//! `docs/archive/2026-09-16-interaction-face-v0-design.md` §6).
//!
//! For each named interface in a package this module emits, over the
//! `ridl-rt` runtime crate named by its absolute path (`::ridl_rt::…`):
//!
//! - one unit struct per interface with an [`Interface`](::ridl_rt) impl
//!   carrying `CATALOG`, `NUMBER`, `PROVISIONAL`, `NAME` and `MEMBERS`, and
//!   one size table per encoding of the list — a `Sizes<E>` impl, every item
//!   a const expression over the codec's own `<T as Payload<E>>::MAX_SIZE`,
//!   which the face reads for its claim and event buffers. Every payload has
//!   that bound in every encoding of the list: a declaration without one
//!   refuses the package before any descriptor is rendered, and an induced
//!   tuple without one refuses its interface ([`single_param_type`],
//!   [`query_reply_type`]);
//! - one unit struct per interaction with an `Interaction` impl (the three
//!   required items, `type Iface`, `const MEMBER` and `const ROW`) and the
//!   kind's trait — `Signal`, `Event`, `Command`, `Query` or `Fixed`.
//!   A member with a stream, as a parameter or as the reply, has no `Command`
//!   or `Query` impl until the face carries streams, so a generic caller
//!   cannot send it without its stream; its `MEMBERS` row is written all the
//!   same, and every row lists the member's streams.
//!
//! It is reached from [`crate::generate_pipeline`], which is
//! what `ridl build --emit rust` calls, as well as from
//! [`crate::generate_face_with`]. It is not reached from [`crate::generate`],
//! whose output is unchanged and carries neither the descriptors nor the
//! face — which is what is unchanged about it, rather than its bytes (see
//! the entry points' own documentation).

use crate::clauses::{self, ClauseKind};
use crate::{Ctx, GenerateError, WireEncoding, camel_of as camel, declared, ident, type_path};
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
/// identifier, and descriptors for an inline shape are not emitted.
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
/// walk (interaction-face design rule 2). [`interface_items`] is the whole-package walk;
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
    let first = items.len();
    one_interface_public(ctx, interface, items)?;
    if crate::face::is_internal(interface) {
        for item in &mut items[first..] {
            *item = crate::allow_dead_code_if_internal(
                interface.visibility,
                package_private_structs(std::mem::take(item)),
            );
        }
    }
    Ok(())
}

/// Rewrites every `pub struct` among `items` to `pub(crate) struct`.
///
/// The descriptors of an `internal` interface implement `ridl_rt` traits whose
/// associated types name the interface's payload types, which may be
/// `pub(crate)`. A public descriptor could then name a private type (E0446),
/// so the descriptor takes the visibility of the interface. Every emitted item
/// parses, because `render` parses the whole output; one that does not is left
/// alone here and reported there.
fn package_private_structs(items: TokenStream) -> TokenStream {
    let Ok(mut file) = syn::parse2::<syn::File>(items.clone()) else {
        return items;
    };
    for item in &mut file.items {
        if let syn::Item::Struct(item) = item {
            item.vis = syn::parse_quote! { pub(crate) };
            // A crate-visible descriptor is an internal, so it carries no
            // doc and needs no `#[doc(hidden)]`.
            item.attrs.retain(|attr| !attr.path().is_ident("doc"));
        }
    }
    quote! { #file }
}

/// The descriptors of one interface, every struct public.
fn one_interface_public(
    ctx: &Ctx,
    interface: &v1::Interface,
    items: &mut Vec<TokenStream>,
) -> Result<(), GenerateError> {
    let catalog_name = catalog_name(ctx);
    let iface_name = declared_name(interface).unwrap_or_default();
    let iface_ident = ident(iface_name);

    // Reserved tombstones get no row, so a row's index is not its ordinal
    // minus one — the index is its position among the live interactions,
    // which the lowering states as `Interaction.row`.
    let interactions = interactions(interface);

    let mut member_rows: Vec<TokenStream> = Vec::new();
    let mut sizes = TablePayloads::default();
    let mut interaction_items: Vec<TokenStream> = Vec::new();

    for (ordinal, interaction) in &interactions {
        member_rows.push(member_row(ctx, *ordinal, interaction)?);
        interaction_items.push(interaction_item(
            ctx,
            &iface_ident,
            interaction,
            interaction.row as usize,
            &mut sizes,
        )?);
    }

    let catalog_hash = catalog_hash(ctx)?;
    let number = Literal::u32_unsuffixed(interface.number);
    let provisional = interface.provisional;

    // One size table per encoding of the list, FlatBuffers first. Each is one
    // reservation row per member of `MEMBERS`, in order, and the two buffer
    // sizes, every one a const expression over `<T as Payload<E>>::MAX_SIZE`,
    // the codec's own bound; the face holds the table's claim and event
    // buffers. The doc states what the descriptor is for its user, not how
    // the table is computed.
    let mut tables: Vec<TokenStream> = Vec::new();
    let mut table_docs: Vec<String> = Vec::new();
    if ctx.encodings.contains(&WireEncoding::FlatBuffers) {
        tables.push(sizes_table(&iface_ident, WireEncoding::FlatBuffers, &sizes));
        table_docs.push(table_doc(iface_name, "FlatBuffers", "FlatBuffers"));
    }
    if ctx.encodings.contains(&WireEncoding::Proto3) {
        tables.push(sizes_table(&iface_ident, WireEncoding::Proto3, &sizes));
        table_docs.push(table_doc(iface_name, "proto3", "Proto3"));
    }
    if ctx.encodings.contains(&WireEncoding::Podbuf) {
        tables.push(sizes_table(&iface_ident, WireEncoding::Podbuf, &sizes));
        table_docs.push(table_doc(iface_name, "podbuf", "Podbuf"));
    }
    let mut interface_doc = format!(
        "Descriptor for interface `{iface_name}`: its catalog, number, name and \
         members, as `ridl_rt::contract::Interface`."
    );
    for paragraph in &table_docs {
        interface_doc.push_str("\n\n");
        interface_doc.push_str(paragraph);
    }

    items.push(quote! {
        #[doc = #interface_doc]
        pub struct #iface_ident;
    });

    items.push(quote! {
        impl ::ridl_rt::contract::Interface for #iface_ident {
            const CATALOG: &'static ::ridl_rt::contract::CatalogRef =
                &::ridl_rt::contract::CatalogRef {
                    name: #catalog_name,
                    hash: ::ridl_rt::contract::CatalogHash([#(#catalog_hash),*]),
                };
            const NUMBER: ::ridl_rt::contract::InterfaceNo =
                ::ridl_rt::contract::InterfaceNo(#number);
            const PROVISIONAL: ::core::primitive::bool = #provisional;
            const NAME: &'static ::core::primitive::str = #iface_name;
            const MEMBERS: &'static [::ridl_rt::contract::Member] = &[
                #(#member_rows),*
            ];
        }
    });

    items.extend(tables);
    items.extend(interaction_items);
    Ok(())
}

/// The payloads an interface's size tables are computed over, by type name,
/// gathered once and rendered once per encoding.
#[derive(Default)]
struct TablePayloads {
    /// One row per member of `MEMBERS`, in order: the member's payloads —
    /// one for most kinds, the request and then the reply for a query, none
    /// for a fixed.
    rows: Vec<Vec<String>>,
    /// The call argument and reply payloads, which the claim buffer holds.
    calls: Vec<String>,
    /// The event payloads, which the event buffer holds.
    events: Vec<String>,
}

/// The path of `encoding`'s marker type, as every site of the output names it.
fn encoding_path(encoding: WireEncoding) -> TokenStream {
    match encoding {
        WireEncoding::FlatBuffers => quote! { ::ridl_rt::encoding::FlatBuffers },
        WireEncoding::Proto3 => quote! { ::ridl_rt::encoding::Proto3 },
        WireEncoding::Podbuf => quote! { ::ridl_rt::encoding::Podbuf },
    }
}

/// The interface doc's paragraph for its size table in one encoding, named
/// `label` in prose and `marker` in the path.
fn table_doc(iface_name: &str, label: &str, marker: &str) -> String {
    format!(
        "Its size table in {label} is `<{iface_name} as Sizes<{marker}>>`: \
         the space each member reserves, and the largest buffer a call or an \
         event of this interface needs."
    )
}

/// The `Sizes<E>` implementation of one interface in `encoding`. The rows and
/// the two maxima are const expressions over the codec's `MAX_SIZE`, never a
/// number this emitter would have to keep equal to it.
///
/// A size this table names in an array length is reached through the whole
/// path `<Self as Sizes<E>>::`, not through `Self::`: the interface may
/// implement `Sizes` in a second encoding, and then `Self::MAX_BUFFER_SIZE`
/// names no one table (rustc E0283).
fn sizes_table(
    iface_ident: &proc_macro2::Ident,
    encoding: WireEncoding,
    sizes: &TablePayloads,
) -> TokenStream {
    let marker = encoding_path(encoding);
    let reservations: Vec<TokenStream> = sizes
        .rows
        .iter()
        .map(|payloads| {
            let paths: Vec<TokenStream> = payloads
                .iter()
                .map(|name| max_size_path(encoding, name))
                .collect();
            reservation_row(&paths)
        })
        .collect();
    let max_buffer = max_size_const(
        &sizes
            .calls
            .iter()
            .map(|name| max_size_path(encoding, name))
            .collect::<Vec<_>>(),
    );
    let event_buffer = max_size_const(
        &sizes
            .events
            .iter()
            .map(|name| max_size_path(encoding, name))
            .collect::<Vec<_>>(),
    );
    let table = quote! {
        <Self as ::ridl_rt::contract::Sizes<#marker>>
    };
    // A podbuf buffer starts on an 8-byte boundary, so its claim and event
    // buffers are `Aligned` arrays; the other encodings take plain arrays.
    let (claim_buffer, event_buffer_type, claim_zero, event_zero) = if encoding
        == WireEncoding::Podbuf
    {
        (
            quote! { ::ridl_rt::podbuf::Aligned<[::core::primitive::u8; #table::MAX_BUFFER_SIZE]> },
            quote! { ::ridl_rt::podbuf::Aligned<[::core::primitive::u8; #table::EVENT_SOURCE_BUFFER_SIZE]> },
            quote! { ::ridl_rt::podbuf::Aligned::<[::core::primitive::u8; #table::MAX_BUFFER_SIZE]>::zeroed() },
            quote! { ::ridl_rt::podbuf::Aligned::<[::core::primitive::u8; #table::EVENT_SOURCE_BUFFER_SIZE]>::zeroed() },
        )
    } else {
        (
            quote! { [::core::primitive::u8; #table::MAX_BUFFER_SIZE] },
            quote! { [::core::primitive::u8; #table::EVENT_SOURCE_BUFFER_SIZE] },
            quote! { [0u8; #table::MAX_BUFFER_SIZE] },
            quote! { [0u8; #table::EVENT_SOURCE_BUFFER_SIZE] },
        )
    };
    quote! {
        impl ::ridl_rt::contract::Sizes<#marker> for #iface_ident {
            const RESERVATIONS: &'static [::core::primitive::u64] = &[
                #(#reservations),*
            ];
            const MAX_BUFFER_SIZE: ::core::primitive::usize = #max_buffer;
            const EVENT_SOURCE_BUFFER_SIZE: ::core::primitive::usize = #event_buffer;
            type ClaimBuffer = #claim_buffer;
            type EventBuffer = #event_buffer_type;
            const CLAIM_BUFFER: Self::ClaimBuffer = #claim_zero;
            const EVENT_BUFFER: Self::EventBuffer = #event_zero;
        }
    }
}

/// The name of one interaction's descriptor, `<Interface><Member>`: the
/// interface's identifier followed by the member's `camel_case`. The face
/// names the descriptor through this function too, and the claim table
/// (`crate::claims`) claims the name it returns, so the three cannot spell it
/// differently.
pub(crate) fn descriptor_ident(
    iface_ident: &proc_macro2::Ident,
    interaction: &v1::Interaction,
) -> proc_macro2::Ident {
    ident(&format!(
        "{iface_ident}{}",
        camel(interaction.name.as_ref())
    ))
}

/// The `Member` row for one interaction.
fn member_row(
    ctx: &Ctx,
    slot: u32,
    interaction: &v1::Interaction,
) -> Result<TokenStream, GenerateError> {
    let ordinal = Literal::u32_unsuffixed(slot);
    let member = declared(interaction.name.as_ref());
    let name = member;
    let timing = timing_tokens(interaction.timing.as_ref());

    let (kind, timing, payloads) = match interaction.shape.as_ref() {
        Some(v1::interaction::Shape::Signal(signal)) => (
            quote! { ::ridl_rt::contract::Kind::Signal },
            timing,
            vec![payload_info(signal.payload.as_ref())],
        ),
        Some(v1::interaction::Shape::Event(event)) => (
            quote! { ::ridl_rt::contract::Kind::Event },
            timing,
            vec![payload_info(event.payload.as_ref())],
        ),
        Some(v1::interaction::Shape::Command(command)) => (
            quote! { ::ridl_rt::contract::Kind::Command },
            timing,
            vec![payload_named(single_param_type(ctx, command, member)?)],
        ),
        Some(v1::interaction::Shape::Query(query)) => (
            quote! { ::ridl_rt::contract::Kind::Query },
            timing,
            vec![
                payload_named(query_param_type(ctx, query, member)?),
                match stream_reply(interaction) {
                    Some(spelled) => payload_named(&spelled),
                    None => payload_named(query_reply_type(ctx, query, member)?),
                },
            ],
        ),
        Some(v1::interaction::Shape::Fixed(fixed)) => (
            quote! { ::ridl_rt::contract::Kind::Fixed },
            quote! { ::core::option::Option::None },
            vec![payload_info(Some(fixed_payload(fixed, member)?))],
        ),
        None => return Err(not_an_interaction(member)),
    };

    let streams = stream_infos(interaction);

    Ok(quote! {
        ::ridl_rt::contract::Member {
            ordinal: ::ridl_rt::contract::Ordinal(#ordinal),
            kind: #kind,
            name: #name,
            timing: #timing,
            payloads: &[ #(#payloads),* ],
            streams: #streams,
        }
    })
}

/// One interaction's unit struct, its `Interaction` impl, and its kind trait
/// impl. Appends the interaction's reservation row to the interface's size
/// tables, and its argument, reply and event payloads to the two
/// buffer-constant worklists.
fn interaction_item(
    ctx: &Ctx,
    iface_ident: &proc_macro2::Ident,
    interaction: &v1::Interaction,
    row_index: usize,
    sizes: &mut TablePayloads,
) -> Result<TokenStream, GenerateError> {
    let member = declared(interaction.name.as_ref());
    let mut row: Vec<String> = Vec::new();
    let struct_ident = descriptor_ident(iface_ident, interaction);
    let index = Literal::usize_unsuffixed(row_index);

    let kind_impl = match interaction.shape.as_ref() {
        Some(v1::interaction::Shape::Signal(signal)) => {
            let reference = payload_reference(signal.payload.as_ref());
            let payload = type_path(reference);
            row.push(reference.to_string());
            // `init()` returns the payload type's default, which is the
            // channel init (ridl §4.4) for a signal with no `= value`
            // override — the fixture's case. A declared init override is a
            // follow-up; the payload type derives `Default` here. The call
            // is the trait's path rather than `T::default()`, which an enum
            // set bit named `default` would capture (`docs/technotes/rust-backend-name-collisions.md`, X-16).
            quote! {
                impl ::ridl_rt::contract::Signal for #struct_ident {
                    type Payload = #payload;
                    fn init() -> Self::Payload {
                        <#payload as ::core::default::Default>::default()
                    }
                }
            }
        }
        Some(v1::interaction::Shape::Event(event)) => {
            let reference = payload_reference(event.payload.as_ref());
            let payload = type_path(reference);
            row.push(reference.to_string());
            sizes.events.push(reference.to_string());
            quote! {
                impl ::ridl_rt::contract::Event for #struct_ident {
                    type Payload = #payload;
                }
            }
        }
        Some(v1::interaction::Shape::Fixed(fixed)) => {
            let payload = type_path(payload_reference(Some(fixed_payload(fixed, member)?)));
            // A fixed is provisioned, never in flight, so it reserves nothing:
            // its row has no payload.
            quote! {
                impl ::ridl_rt::contract::Fixed for #struct_ident {
                    type Payload = #payload;
                }
            }
        }
        // A member with a stream, as a parameter or as the reply, gets no
        // `Command` or `Query` impl until the face carries streams: a generic
        // caller of such an impl could send the call without its stream. Its
        // row of `MEMBERS` and its size-table row are written all the same;
        // a streamed reply has no reply type and no row of the table.
        Some(v1::interaction::Shape::Command(command)) if has_stream(interaction) => {
            let arg_name = single_param_type(ctx, command, member)?;
            row.push(arg_name.to_string());
            sizes.calls.push(arg_name.to_string());
            TokenStream::new()
        }
        Some(v1::interaction::Shape::Query(query)) if has_stream(interaction) => {
            let arg_name = query_param_type(ctx, query, member)?;
            row.push(arg_name.to_string());
            sizes.calls.push(arg_name.to_string());
            if stream_reply(interaction).is_none() {
                let reply_name = query_reply_type(ctx, query, member)?;
                row.push(reply_name.to_string());
                sizes.calls.push(reply_name.to_string());
            }
            TokenStream::new()
        }
        Some(v1::interaction::Shape::Command(command)) => {
            let arg_name = single_param_type(ctx, command, member)?;
            let args = type_path(arg_name);
            row.push(arg_name.to_string());
            sizes.calls.push(arg_name.to_string());
            let require = clauses::translate(
                ctx,
                &command.clauses,
                ClauseKind::Require,
                &command.params,
                None,
            )?;
            let args_param = binding("args", require.uses_args);
            let require_expr = require.expr;
            // `require` evaluates the `require` clauses. It is a method of a
            // hidden descriptor, so it carries no doc: see `clauses.rs` for
            // the one clause form the translation covers.
            quote! {
                impl ::ridl_rt::contract::Command for #struct_ident {
                    type Args = #args;
                    fn require(#args_param: &Self::Args) -> ::core::result::Result<(), ()> {
                        #require_expr
                    }
                }
            }
        }
        Some(v1::interaction::Shape::Query(query)) => {
            let arg_name = query_param_type(ctx, query, member)?;
            let reply_name = query_reply_type(ctx, query, member)?;
            let args = type_path(arg_name);
            let reply = type_path(reply_name);
            row.push(arg_name.to_string());
            row.push(reply_name.to_string());
            sizes.calls.push(arg_name.to_string());
            sizes.calls.push(reply_name.to_string());

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
            // `require` and `ensure` evaluate the clauses of their kind, and
            // carry no doc for the reason the command arm gives.
            quote! {
                impl ::ridl_rt::contract::Query for #struct_ident {
                    type Args = #args;
                    type Reply = #reply;
                    fn require(#require_param: &Self::Args) -> ::core::result::Result<(), ()> {
                        #require_expr
                    }
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
    sizes.rows.push(row);

    // A member descriptor is an internal: a user's code names the interface
    // descriptor and the face, not this. It stays `pub`, under
    // `#[doc(hidden)]`, because it implements the `ridl_rt::contract`
    // traits whose `MEMBER` a runtime or a test outside the crate reads, and
    // because an event's descriptor is named by nothing in the crate, so
    // `pub(crate)` would draw `dead_code`. The descriptors of an `internal`
    // interface are `pub(crate)` instead (`package_private_structs`).
    Ok(quote! {
        #[doc(hidden)]
        pub struct #struct_ident;

        impl ::ridl_rt::contract::Interaction for #struct_ident {
            type Iface = #iface_ident;
            const MEMBER: &'static ::ridl_rt::contract::Member =
                &<#iface_ident as ::ridl_rt::contract::Interface>::MEMBERS[#index];
            const ROW: ::core::primitive::usize = #index;
        }

        #kind_impl
    })
}

/// The `PayloadInfo` of one payload of the model: its type name. The
/// payload's size is not here but in the interface's size table, as the
/// member's row over the codec's own `MAX_SIZE` ([`reservation_row`]).
fn payload_info(payload: Option<&v1::Payload>) -> TokenStream {
    payload_named(payload_reference(payload))
}

/// [`payload_info`] for a payload already read as a type name, which is how a
/// call's argument and reply are read: either may be an induced struct, which
/// no `Payload` of the model names (driftsys/ridl#713).
fn payload_named(type_name: &str) -> TokenStream {
    quote! {
        ::ridl_rt::contract::PayloadInfo {
            type_name: #type_name,
        }
    }
}

/// The `streams` slice of one member's row: one `StreamInfo` per stream of
/// the interaction, in the model's order (the reply stream first, then each
/// stream parameter). The element is the package-qualified name under
/// `Elements` and `""` under `Bytes` and `Text`.
fn stream_infos(interaction: &v1::Interaction) -> TokenStream {
    let entries = interaction.streams.iter().map(|stream| {
        let no = Literal::u32_unsuffixed(stream.no);
        let (form, element) = match v1::StreamForm::try_from(stream.form) {
            Ok(v1::StreamForm::Bytes) => (quote! { Bytes }, String::new()),
            Ok(v1::StreamForm::Text) => (quote! { Text }, String::new()),
            _ => (quote! { Elements }, stream_element(stream)),
        };
        quote! {
            ::ridl_rt::contract::StreamInfo {
                no: ::ridl_rt::contract::StreamNo(#no),
                form: ::ridl_rt::contract::StreamForm::#form,
                element: #element,
            }
        }
    });
    quote! { &[ #(#entries),* ] }
}

/// The package-qualified canonical name of a stream's element type, the form
/// the catalog descriptor writes. A reference no package declares keeps its
/// canonical string.
fn stream_element(stream: &v1::StreamShape) -> String {
    let Some(element) = stream.element.as_ref() else {
        return String::new();
    };
    if element.resolved && !element.package.is_empty() {
        let name = element
            .reference
            .rsplit_once('.')
            .map_or(element.reference.as_str(), |(_, name)| name);
        format!("{}.{name}", element.package)
    } else {
        element.reference.clone()
    }
}

/// The spelling of a query's reply when it is a stream (`<T>`, `<bytes>` or
/// `<string>`), or `None` when the reply is not a stream. A streamed reply
/// has no payload type, so it has no `Reply` and no row of the size table.
/// The element is spelled as the IR reference text, as a payload is.
fn stream_reply(interaction: &v1::Interaction) -> Option<String> {
    let stream = interaction.streams.iter().find(|stream| stream.no == 0)?;
    let inner = match v1::StreamForm::try_from(stream.form) {
        Ok(v1::StreamForm::Bytes) => "bytes".to_string(),
        Ok(v1::StreamForm::Text) => "string".to_string(),
        _ => stream
            .element
            .as_ref()
            .map(|element| element.reference.clone())
            .unwrap_or_default(),
    };
    Some(format!("<{inner}>"))
}

/// One row of the size table: the sum, as a `u64`, of the `MAX_SIZE` paths
/// of a member's payloads — one for most kinds, the request and then the
/// reply for a query — or `0u64` for a fixed, which has none.
fn reservation_row(sizes: &[TokenStream]) -> TokenStream {
    if sizes.is_empty() {
        return quote! { 0u64 };
    }
    quote! { #(#sizes as ::core::primitive::u64)+* }
}

/// The `<T as Payload<E>>::MAX_SIZE` const-evaluable path for a payload type
/// in `encoding`. Buffer sizes are written in terms of it, never as a
/// literal, so a buffer is sized by the codec's own bound rather than by a
/// number this emitter would have to keep equal to it. The encoding is named
/// by its full path at each site, as the codec names it in its own
/// implementations; the package carries no alias for it (the
/// `docs/technotes/rust-backend-name-collisions.md`, decision 5).
fn max_size_path(encoding: WireEncoding, type_name: &str) -> TokenStream {
    let path = type_path(type_name);
    let marker = encoding_path(encoding);
    quote! {
        <#path as ::ridl_rt::payload::Payload<#marker>>::MAX_SIZE
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
        return quote! { ::core::option::Option::None };
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
        ::core::option::Option::Some(
            ::ridl_rt::contract::Timing { mode: #mode, min: #min, max: #max },
        )
    }
}

fn duration_tokens(micros: Option<&str>) -> TokenStream {
    match micros {
        None => quote! { ::core::option::Option::None },
        Some(text) => {
            let value = parse_micros(text);
            let literal = Literal::i64_unsuffixed(value);
            quote! { ::core::option::Option::Some(::ridl_rt::sample::Duration(#literal)) }
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

/// The canonical IR text of the type one payload names.
fn payload_reference(payload: Option<&v1::Payload>) -> &str {
    payload
        .and_then(|payload| payload.r#type.as_ref())
        .map(|reference| reference.reference.as_str())
        .unwrap_or_default()
}

/// The call's argument type: the single declared named parameter, or the
/// induced argument struct the lowering named for every other parameter list,
/// zero parameters included. A stream parameter adds no field to it. Refuses a
/// parameter with no declared type.
pub(crate) fn single_param_type<'a>(
    ctx: &Ctx<'a>,
    command: &'a v1::CommandShape,
    member: &str,
) -> Result<&'a str, GenerateError> {
    if let Some(payload) = command.request.as_ref() {
        return Ok(payload_reference(Some(payload)));
    }
    induced_name(
        ctx,
        command.request_tuple.as_ref(),
        command.request_sizes.as_ref(),
    )?
    .ok_or_else(|| no_single_param(member))
}

/// [`single_param_type`] for a query, which carries the same request shape.
pub(crate) fn query_param_type<'a>(
    ctx: &Ctx<'a>,
    query: &'a v1::QueryShape,
    member: &str,
) -> Result<&'a str, GenerateError> {
    if let Some(payload) = query.request.as_ref() {
        return Ok(payload_reference(Some(payload)));
    }
    induced_name(
        ctx,
        query.request_tuple.as_ref(),
        query.request_sizes.as_ref(),
    )?
    .ok_or_else(|| no_single_param(member))
}

/// The Rust name of the induced tuple `reference` names, when it names one.
///
/// A tuple with no finite bound in an encoding the list names is refused,
/// because that encoding's codec emits no `Payload` for it, and a
/// descriptor row would name a `MAX_SIZE` that does not exist. The refusal
/// is per interface: the pipeline skips the interface with a note and
/// generates the rest of the package (interaction-face design rule 2). With
/// `flatbuffers` in the list the bound read is the FlatBuffers root's; with
/// `proto3` in the list it is the proto3 size state of the interaction's
/// payload, `sizes`, and the refusal names the cause the sizer gave; with
/// `podbuf` in the list it is the podbuf size state of the same payload.
fn induced_name<'a>(
    ctx: &Ctx<'a>,
    reference: Option<&v1::TupleRef>,
    sizes: Option<&v1::PayloadSizes>,
) -> Result<Option<&'a str>, GenerateError> {
    let Some(reference) = reference else {
        return Ok(None);
    };
    let name = ctx.tuple_name(reference.index);
    if ctx.encodings.contains(&WireEncoding::FlatBuffers) {
        let root = ctx.model.flatbuffers.as_ref().and_then(|projection| {
            projection
                .roots
                .iter()
                .find(|root| root.tuple == Some(reference.index))
        });
        if !matches!(
            root.and_then(|root| root.bound.as_ref()),
            Some(v1::fb_root::Bound::MaxSize(_))
        ) {
            return Err(GenerateError {
                message: format!(
                    "`{}.{name}` has no finite FlatBuffers bound",
                    ctx.package_name()
                ),
            });
        }
    }
    if ctx.encodings.contains(&WireEncoding::Proto3) {
        let state = sizes.and_then(|sizes| sizes.proto3.as_ref());
        if let Err(cause) = crate::codec_proto3::bound_state(state) {
            return Err(GenerateError {
                message: format!(
                    "`{}.{name}` has no finite proto3 bound: {cause}",
                    ctx.package_name()
                ),
            });
        }
    }
    if ctx.encodings.contains(&WireEncoding::Podbuf) {
        let state = sizes.and_then(|sizes| sizes.podbuf.as_ref());
        if !matches!(
            state.and_then(|state| state.state.as_ref()),
            Some(v1::size_state::State::Bounded(_))
        ) {
            return Err(GenerateError {
                message: format!(
                    "`{}.{name}` has no finite podbuf bound: its largest buffer does not fit a \
                     u32 offset, or an array or map of it passes a descriptor limit",
                    ctx.package_name()
                ),
            });
        }
    }
    Ok((!name.is_empty()).then_some(name))
}

/// The model's `Catalog.package`: the name of the unit the package belongs
/// to, which is `CatalogRef.name`. Read from the model, not from
/// `Scope.package`, which names the source package and is what the module
/// and path names are built from.
pub(crate) fn catalog_name<'a>(ctx: &Ctx<'a>) -> &'a str {
    ctx.model
        .catalog
        .as_ref()
        .map(|catalog| catalog.package.as_str())
        .unwrap_or_default()
}

/// The model's `Catalog.hash` (ADR-0014 decision 15) as 32 `u8` literals,
/// read from the model and never recomputed, because a plugin receives only
/// the model. A model whose hash is missing or not 32 bytes long is
/// malformed: it is refused, not padded.
pub(crate) fn catalog_hash(ctx: &Ctx) -> Result<Vec<Literal>, GenerateError> {
    let hash = ctx
        .model
        .catalog
        .as_ref()
        .map(|catalog| catalog.hash.as_slice())
        .unwrap_or_default();
    if hash.len() != 32 {
        return Err(GenerateError {
            message: format!(
                "malformed codegen model: `Catalog.hash` is {} bytes, not 32",
                hash.len()
            ),
        });
    }
    Ok(hash
        .iter()
        .map(|byte| Literal::u8_unsuffixed(*byte))
        .collect())
}

/// Whether the interaction has a stream, as a parameter or as its reply. The
/// face leaves such a member out; its descriptor row is written all the same.
pub(crate) fn has_stream(interaction: &v1::Interaction) -> bool {
    !interaction.streams.is_empty()
}

/// Whether a query replies with a stream: its stream number 0.
pub(crate) fn has_stream_reply(interaction: &v1::Interaction) -> bool {
    interaction.streams.iter().any(|stream| stream.no == 0)
}

/// Why a call has no argument type: a parameter with no declared type.
fn no_single_param(member: &str) -> GenerateError {
    GenerateError {
        message: format!("interaction `{member}` declares a parameter with no type"),
    }
}

/// A query's reply type: the declared named type, or the induced reply struct
/// the lowering named for a tuple reply. A fallible `T | E` reply induces
/// nothing and is refused.
pub(crate) fn query_reply_type<'a>(
    ctx: &Ctx<'a>,
    query: &'a v1::QueryShape,
    member: &str,
) -> Result<&'a str, GenerateError> {
    if let Some(payload) = query.reply_payload.as_ref() {
        return Ok(payload_reference(Some(payload)));
    }
    if let Some(name) = induced_name(ctx, query.reply_tuple.as_ref(), query.reply_sizes.as_ref())? {
        return Ok(name);
    }
    match query.reply.as_ref().and_then(|reply| reply.kind.as_ref()) {
        Some(v1::reply::Kind::Value(_)) => Err(GenerateError {
            message: format!("query `{member}`'s reply must be a named type or a tuple"),
        }),
        _ => Err(GenerateError {
            message: format!(
                "query `{member}`'s reply is a fallible `T | E`, which the face does not carry"
            ),
        }),
    }
}

/// A `fixed`'s payload, a named type, which is the only form the face carries.
fn fixed_payload<'a>(
    fixed: &'a v1::FixedShape,
    member: &str,
) -> Result<&'a v1::Payload, GenerateError> {
    match fixed.named.as_ref() {
        Some(named) => Ok(named),
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
    use crate::{WireEncoding, generate_face_with};
    use ridl_ir::v2;

    /// `max_size_const` must compute the maximum of its inputs, not their sum,
    /// minimum, first, or last — design §6 requires `MAX_BUFFER_SIZE` and
    /// `EVENT_SOURCE_BUFFER_SIZE` to be the maximum over the relevant
    /// `<T as Payload<FlatBuffers>>::MAX_SIZE` values.
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
            limit: None,
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
            links: Vec::new(),
            see: Vec::new(),
            since: Vec::new(),
        };

        let fixed = v2::Decl {
            limit: None,
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
            links: Vec::new(),
            see: Vec::new(),
            since: Vec::new(),
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
            links: Vec::new(),
            see: Vec::new(),
            since: Vec::new(),
        };

        let package = v2::Package {
            name: "p".to_string(),
            decls: vec![payload],
            interfaces: vec![interface],
            services: Vec::new(),
            retired: Vec::new(),
            unit: String::new(),
        };

        let source = generate_face_with(&package, &[WireEncoding::FlatBuffers])
            .expect("generate_face_with")
            .rust_source;
        let dense: String = source.chars().filter(|c| !c.is_whitespace()).collect();

        assert!(dense.contains("pubstructPanelBrightness;"));
        assert!(dense.contains("impl::ridl_rt::contract::FixedforPanelBrightness"));
        assert!(dense.contains("typePayload=Provisioned;"));
        assert!(dense.contains("::ridl_rt::contract::Kind::Fixed"));
        // A fixed carries no timing.
        assert!(dense.contains("name:\"brightness\",timing:::core::option::Option::None"));
        // A fixed is not a call or an event, so both buffer constants are 0.
        assert!(dense.contains("MAX_BUFFER_SIZE:::core::primitive::usize=0usize"));
        assert!(dense.contains("EVENT_SOURCE_BUFFER_SIZE:::core::primitive::usize=0usize"));
    }
}
