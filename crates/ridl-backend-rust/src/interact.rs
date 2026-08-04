//! IR v2 interfaces and services to Rust (E2 task 15) — the Rust half of the
//! E2 exit criterion.
//!
//! Every interface is realized as two traits: a **consumer** face and a
//! **provider** face, the ridl §10.1 binding split expressed in the type
//! system. The mapping mirrors the TypeScript backend's decisions construct for
//! construct, so the same IR produces the same contract in both languages:
//!
//! | ridl construct | consumer            | provider                     |
//! | -------------- | ------------------- | ---------------------------- |
//! | `signal`       | `SignalHandle<T>`   | `publish_x(value)`           |
//! | `event`        | `EventHandle<T>`    | `raise_x(occurrence)`        |
//! | `command`      | `async fn x() `     | `async fn on_x()`            |
//! | `query`        | `async fn x() -> R` | `async fn on_x() -> R`       |
//! | `fixed`        | `fn x() -> &T`      | none — provisioned externally |
//!
//! The emitted vocabulary (`Provenance`, `SignalHandle`, `EventHandle`,
//! `RidlStream`, and the metadata structs) is **dependency-free**: it names only
//! `core` paths, so the generated module compiles with `rustc` alone. That is
//! what makes the Appendix A compile proof meaningful — it proves the contract,
//! not a runtime.
//!
//! Three rules govern the awkward corners:
//!
//! - **A command never returns a value.** The delivery acknowledgment beneath
//!   it is runtime-internal (ridl §6.1) and is recorded in the doc comment
//!   rather than the signature.
//! - **Transport failure is undeclared.** Generated doc comments use the
//!   general-form §6.4 sentence verbatim — `infrastructure failure — detected,
//!   undeclared` — never a Rust error type, because Stratum 3 is invisible to
//!   the language.
//! - **Timing never rounds.** The IR carries exact decimal microseconds; this
//!   backend emits `u64` microseconds and returns a [`GenerateError`] for any
//!   bound that is not a whole number of microseconds, rather than truncating a
//!   bound that was written into the contract.
//!
//! **Visibility.** An `internal interface` generates four `pub(crate)` items —
//! both faces and both metadata constants — never a `pub` one, the same
//! package-private mapping the typl surface gives an `internal` declaration
//! (ADR-0002 §8, ADR-0008 decision 7). Publishing the API of a declaration the
//! keyword hides would defeat the modifier, and the struct a query's tuple
//! return induces is a fifth name the same declaration generates, so it carries
//! the same visibility (issue #167). The vocabulary and the service table are
//! package-level and stay `pub`: they are emitted once per module and belong to
//! no single declaration.

use crate::{
    GenerateError, InducedTuple, camel_case, deprecated_attr, doc_attrs, field_type_tokens, ident,
    primitive_tokens, type_path, vis_tokens,
};
use proc_macro2::{Literal, TokenStream};
use quote::quote;
use ridl_ir::v2;
use std::collections::{HashMap, HashSet};

/// Emits the interaction vocabulary, every interface, and the service table for
/// `package`. Returns an empty stream when the package declares neither an
/// interface nor a service, so a pure typl package generates exactly what it
/// did before this module existed.
///
/// Generated tuple types discovered in interaction positions are appended to
/// `tuples`, which the caller drains alongside the ones found in declarations.
pub(crate) fn emit(
    package: &v2::Package,
    tuples: &mut Vec<InducedTuple>,
) -> Result<TokenStream, GenerateError> {
    if package.interfaces.is_empty() && package.services.is_empty() {
        return Ok(quote! {});
    }

    // Where this module's tuple discoveries begin. Tuples found while emitting
    // the package's declarations are already in the list and are not this
    // module's to police.
    let tuples_start = tuples.len();
    // Every interface generated here, named and inline alike, each with the
    // construct an author would recognize it by — the set the collision check
    // has to cover, and the wording its diagnostics quote.
    let mut owners: Vec<(String, String)> = Vec::new();

    let mut items = vec![vocabulary()];

    // Every interface shape, named and inline alike — `Package::shapes`, not
    // `package.interfaces`, which is not the complete set. A named interface is
    // its own identity: the generated type name and the identity name are the
    // same string. A service with an inline shape declares an anonymous
    // interface, generated under a name derived from the service address so the
    // two forms produce the same shape of API; its identity stays the service's
    // DOTTED name — never the mangled type name, and never `Interface.name`,
    // which is empty by construction for an inline shape.
    for shape in package.shapes() {
        let (type_name, origin) = match shape.service {
            Some(service) => (
                inline_interface_name(&service.name),
                format!("the inline shape of service {}", service.name),
            ),
            None => (
                shape.name.to_string(),
                format!("interface {name}", name = shape.name),
            ),
        };
        let names = Names {
            r#type: &type_name,
            identity: shape.name,
            visibility: shape.visibility(),
        };
        items.push(emit_interface(names, shape.interface, tuples)?);
        owners.push((type_name, origin));
    }

    items.push(emit_services(package)?);

    // The tuple structs an interaction position generates are discovered
    // lazily: emitting one tuple's fields is what finds the level below it, and
    // `lib.rs` does that draining only after this function returns. Waiting is
    // not an option for the collision check, so the same walk runs here over
    // this module's own slice of the worklist, purely to learn the names.
    // `field_type_tokens` appends what it finds and `lib.rs` dedupes by name,
    // so pre-discovering changes which names exist, not which structs are
    // emitted.
    let mut seen: HashSet<String> = HashSet::new();
    let mut index = tuples_start;
    while index < tuples.len() {
        let induced = tuples[index].clone();
        index += 1;
        if !seen.insert(induced.name.clone()) {
            continue;
        }
        for field in &induced.tuple.fields {
            if let Some(field_type) = field.r#type.as_ref() {
                let hint = format!("{}{}", induced.name, camel_case(&field.name));
                // The tokens are discarded; the side effect on `tuples` is the
                // point.
                let _ = field_type_tokens(field_type, &hint, induced.visibility, tuples);
            }
        }
    }

    // Run last: every name this module generates is known only now.
    check_name_collisions(package, &owners, &tuples[tuples_start..])?;

    Ok(quote! { #(#items)* })
}

/// The two names an interface is generated under, which are not always the same
/// string.
///
/// Every consumer of either spelling is enumerated here, and the list is meant
/// to be checked against the code. Both defects this split has produced were
/// the same mistake — a consumer that was not on the list — so an addition to
/// the emitter that reaches for a name belongs in this comment first.
///
/// `type` — must be a Rust identifier, so an inline service shape's is the
/// mangled `ServiceVehAdasLogs`. It spells **four** things:
///
/// 1. the `{type}Consumer` and `{type}Provider` trait names;
/// 2. the `{TYPE}_TIMING` const;
/// 3. the `{TYPE}_CONTRACTS` const;
/// 4. the name hint for every struct generated from a tuple in an interaction
///    position — a query's tuple return, a tuple parameter, and their nested
///    array/map elements.
///
/// `identity` — what the interface is called everywhere OUTSIDE this module. It
/// is a dotted address for an inline shape and therefore **not** a Rust
/// identifier. It spells **three** things:
///
/// 1. the first component of a fallible return's transport identity (ADR-0008
///    decision 4);
/// 2. the interface component of an observer-stub id;
/// 3. the interface named in a `GenerateError`, which should read the way the
///    author wrote the contract.
///
/// Keeping the two apart is load-bearing rather than cosmetic, in both
/// directions. The mangled type name is a Rust spelling invented by this
/// backend; emitting it as a transport identity would disagree with `ridl diff`,
/// which keys a service's interactions on the dotted name
/// (`crates/ridl-diff/src/walk.rs`), and with the observer stubs lowered into this
/// very module, which are scoped to the dotted name (`ridl-sem`, E2.5). In the
/// other direction the dotted name contains `.`, so using it as a name hint
/// builds `Veh.adas.logsGetPairResult` — not an identifier at all.
/// `visibility` — the AUTHORITATIVE visibility of the shape, taken from
/// [`v2::InterfaceShape::visibility`] rather than off the `Interface` this
/// module is handed. An inline shape's own `Interface.visibility` is
/// `VISIBILITY_UNSPECIFIED` by construction (ridl §14.5); the owning `Service`
/// carries the real one. Reading it here rather than at the leaf keeps the
/// emitted visibility correct by derivation instead of by the coincidence that
/// [`vis_tokens`] maps `UNSPECIFIED` and `PUBLIC` to the same `pub`.
#[derive(Debug, Clone, Copy)]
struct Names<'a> {
    r#type: &'a str,
    identity: &'a str,
    visibility: i32,
}

// ---------------------------------------------------------------------------
// The interaction vocabulary — emitted once per module, dependency-free.
// ---------------------------------------------------------------------------

fn vocabulary() -> TokenStream {
    quote! {
        /// Where a signal's current value came from (ridl §4.4, §4.5).
        ///
        /// `Init` is the channel's seeded value, before the provider's first
        /// publication; `Live` is a published value; `Invalid` marks a value
        /// that violated the payload constraints — the malformed value is not
        /// delivered, but its invalidity is, so no subscriber silently holds
        /// stale last-good data.
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum Provenance {
            Init,
            Live,
            Invalid,
        }

        /// A signal channel: state that always holds a value (ridl §4.4).
        pub trait SignalHandle<T> {
            /// The current value and where it came from. Never fails: the
            /// channel is seeded with the init value at creation.
            fn read(&self) -> (T, Provenance);

            /// Registers a subscriber. It is called immediately with the
            /// current value, then on every change.
            fn subscribe(&mut self, f: Box<dyn FnMut(&T, Provenance)>);
        }

        /// An event channel: occurrences, which are not state (ridl §5). There
        /// is no `read` — an occurrence that has not happened has no value.
        pub trait EventHandle<T> {
            fn subscribe(&mut self, f: Box<dyn FnMut(&T)>);
        }

        /// A finite or unbounded sequence of payloads (ridl §12).
        ///
        /// Declared here rather than taken from a runtime crate so the
        /// generated module stays dependency-free; it is shaped so an adapter
        /// to `futures::Stream` is a blanket impl.
        pub trait RidlStream {
            type Item;

            fn poll_next(
                self: core::pin::Pin<&mut Self>,
                cx: &mut core::task::Context<'_>,
            ) -> core::task::Poll<Option<Self::Item>>;
        }

        /// How a timing annotation constrains an interaction (ridl §9).
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum TimingMode {
            StrictPeriodic,
            Range,
        }

        /// One interaction's resolved timing, in exact microseconds.
        ///
        /// `min_us` is the rate floor and `max_us` the staleness bound; a strict
        /// period carries the same value in both. On a `command` or `query` the
        /// same two bounds are the call throttle and the response bound
        /// (ridl §9, ADR-0015 decision 3). `default_applied` records that
        /// the contract was written without a `@` annotation and the configured
        /// default was resolved in (ridl §9.1) — always `false` for an RPC,
        /// whose bounds are never defaulted.
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub struct TimingConst {
            pub mode: TimingMode,
            pub min_us: Option<u64>,
            pub max_us: Option<u64>,
            pub default_applied: bool,
        }

        /// Which side of an interaction a contract clause constrains (ridl §13).
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum ContractKind {
            Require,
            Ensure,
        }

        /// One contract clause as data, for an observer to install.
        ///
        /// `id` is the observer address the IR assigns; it is a stable identity
        /// carried verbatim, not a Rust name. `uses_result` says whether the
        /// clause reads the query's result, which an `ensure` observer must
        /// know before it can be scheduled — it cannot run until the result
        /// exists. The flag is carried rather than inferred: `source` is text,
        /// so matching on it would misread a parameter named `resultCode` or a
        /// field access `.result`, and an `ensure` clause does not always read
        /// the result.
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub struct ContractStub {
            pub id: &'static str,
            pub kind: ContractKind,
            pub source: &'static str,
            pub signals: &'static [&'static str],
            pub params: &'static [&'static str],
            pub uses_result: bool,
        }
    }
}

// ---------------------------------------------------------------------------
// One interface: the consumer trait, the provider trait, and the metadata.
// ---------------------------------------------------------------------------

/// What one interaction contributes to the four generated items.
#[derive(Default)]
struct Emitted {
    consumer: Vec<TokenStream>,
    provider: Vec<TokenStream>,
    trait_doc: Vec<TokenStream>,
    timing: Vec<TokenStream>,
    contracts: Vec<TokenStream>,
}

fn emit_interface(
    names: Names,
    interface: &v2::Interface,
    tuples: &mut Vec<InducedTuple>,
) -> Result<TokenStream, GenerateError> {
    let mut out = Emitted::default();
    let name = names.r#type;
    // Every item this interface generates carries the shape's own visibility:
    // an `internal interface` is package-private in full, or the keyword would
    // hide the declaration and publish its API (ADR-0008 decision 7). The value
    // comes from `Names`, which took it from `InterfaceShape::visibility` — an
    // inline service shape's `Interface.visibility` is UNSPECIFIED and its
    // owning `Service` carries the authoritative one (ridl §14.5).
    let vis = vis_tokens(names.visibility);

    for decl in &interface.interactions {
        emit_interaction(names, decl, tuples, &mut out)?;
    }

    let consumer_name = ident(&format!("{}Consumer", camel_case(name)));
    let provider_name = ident(&format!("{}Provider", camel_case(name)));
    let doc = doc_attrs(&interface.doc);
    let deprecated = deprecated_attr(interface.deprecated.as_deref());
    let labels = label_doc(&interface.labels);
    let retired = &out.trait_doc;

    let consumer_doc = doc_attrs(&format!(
        "The consumer face of `{name}` — what a component that uses this \
         interface calls (ridl §10.1)."
    ));
    let provider_doc = doc_attrs(&format!(
        "The provider face of `{name}` — what a component that implements this \
         interface fulfils (ridl §10.1). A `fixed` has no entry here: it is \
         provisioned externally and populated at binding initialization \
         (ridl §8)."
    ));

    let consumer_methods = &out.consumer;
    let provider_methods = &out.provider;

    // `async fn` in a public trait is future-proofing-averse by default (the
    // caller cannot add bounds to the returned future). That is the intended
    // shape here — the generated trait describes a contract, not a library API
    // — so the lint is allowed rather than worked around.
    let traits = quote! {
        #doc
        #labels
        #(#retired)*
        #consumer_doc
        #deprecated
        #[allow(async_fn_in_trait)]
        #vis trait #consumer_name {
            #(#consumer_methods)*
        }

        #doc
        #provider_doc
        #deprecated
        #[allow(async_fn_in_trait)]
        #vis trait #provider_name {
            #(#provider_methods)*
        }
    };

    let timing_name = ident(&format!("{}_TIMING", screaming_snake(name)));
    let contracts_name = ident(&format!("{}_CONTRACTS", screaming_snake(name)));
    let timing_entries = &out.timing;
    let contract_entries = &out.contracts;
    let timing_doc = doc_attrs(&format!(
        "Resolved timing for every timed interaction of `{name}`, keyed by the \
         source name, in exact microseconds (ridl §9): rate floor and staleness \
         bound on a signal or event, call throttle and response bound on a \
         command or query (ADR-0015 decision 3)."
    ));
    let contracts_doc = doc_attrs(&format!(
        "Every `require` and `ensure` clause of `{name}` as data, in source \
         order, for an observer to install (ridl §13)."
    ));

    let metadata = quote! {
        #timing_doc
        #vis const #timing_name: &[(&str, TimingConst)] = &[
            #(#timing_entries),*
        ];

        #contracts_doc
        #vis const #contracts_name: &[ContractStub] = &[
            #(#contract_entries),*
        ];
    };

    Ok(quote! { #traits #metadata })
}

fn emit_interaction(
    names: Names,
    decl: &v2::Decl,
    tuples: &mut Vec<InducedTuple>,
    out: &mut Emitted,
) -> Result<(), GenerateError> {
    // The two spellings, bound once so every use below is explicit about which
    // one it wants. `interface` is the identity — diagnostics and the transport
    // identity — and `type_name` is what generated Rust names are built from.
    let interface = names.identity;
    let type_name = names.r#type;
    let ordinal = decl.ordinal;
    let source_name = &decl.name;
    let deprecated = deprecated_attr(decl.deprecated.as_deref());
    // The method identifier is formed inside each arm, never here: a reserved
    // slot is a named-free interaction (`Decl.name` is empty) and generates no
    // method at all, so building an identifier for it would be building one
    // from an empty string.
    let method = || ident(&snake_case(source_name));

    match &decl.kind {
        Some(v2::decl::Kind::SignalDef(signal)) => {
            let method = method();
            let payload = type_path(&signal.payload);
            let doc = doc_attrs(&signal_doc(decl, signal));
            out.consumer.push(quote! {
                #doc
                #deprecated
                fn #method(&mut self) -> &mut dyn SignalHandle<#payload>;
            });

            let publish = ident(&format!("publish_{}", snake_case(source_name)));
            let publish_doc = doc_attrs(&format!(
                "Publishes `{source_name}` — signal ordinal {ordinal} (ridl §4)."
            ));
            out.provider.push(quote! {
                #publish_doc
                #deprecated
                fn #publish(&mut self, value: #payload);
            });

            if let Some(spec) = &signal.timing {
                out.timing.push(timing_entry(interface, source_name, spec)?);
            }
        }

        Some(v2::decl::Kind::EventDef(event)) => {
            let method = method();
            let payload = type_path(&event.payload);
            let doc = doc_attrs(&doc_body(
                &decl.doc,
                &format!(
                    "event `{source_name}` — ordinal {ordinal} (ridl §5).\n\
                     An occurrence is not state: there is no last-value cache and \
                     no provenance."
                ),
            ));
            out.consumer.push(quote! {
                #doc
                #deprecated
                fn #method(&mut self) -> &mut dyn EventHandle<#payload>;
            });

            let raise = ident(&format!("raise_{}", snake_case(source_name)));
            let raise_doc = doc_attrs(&format!(
                "Raises `{source_name}` — event ordinal {ordinal} (ridl §5)."
            ));
            out.provider.push(quote! {
                #raise_doc
                #deprecated
                fn #raise(&mut self, occurrence: #payload);
            });

            if let Some(spec) = &event.timing {
                out.timing.push(timing_entry(interface, source_name, spec)?);
            }
        }

        Some(v2::decl::Kind::CommandDef(command)) => {
            let method = method();
            let params = param_tokens(names, source_name, &command.params, tuples)?;
            let doc = doc_attrs(&command_doc(decl, &command.contracts));
            out.consumer.push(quote! {
                #doc
                #deprecated
                async fn #method(&self, #(#params),*);
            });

            let handler = ident(&format!("on_{}", snake_case(source_name)));
            let handler_doc = doc_attrs(&format!(
                "Handles `{source_name}` — command ordinal {ordinal} (ridl §6). \
                 The binding has already validated the payload and the `require` \
                 clauses; a violating command never reaches this method \
                 (ridl §6.2)."
            ));
            out.provider.push(quote! {
                #handler_doc
                #deprecated
                async fn #handler(&mut self, #(#params),*);
            });

            push_contracts(
                interface,
                source_name,
                &command.contracts,
                &mut out.contracts,
            )?;

            // The declared RPC bounds ride the same timing table as a
            // signal's (ADR-0015): the two microsecond constants are the call
            // throttle and the response bound. Absent when undeclared.
            if let Some(spec) = &command.timing {
                out.timing.push(timing_entry(interface, source_name, spec)?);
            }
        }

        Some(v2::decl::Kind::QueryDef(query)) => {
            let method = method();
            let params = param_tokens(names, source_name, &query.params, tuples)?;
            let doc = doc_attrs(&query_doc(interface, decl, query));
            let handler = ident(&format!("on_{}", snake_case(source_name)));
            let handler_doc = doc_attrs(&format!(
                "Handles `{source_name}` — query ordinal {ordinal} (ridl §7)."
            ));

            match stream_return(query) {
                // A stream return is not awaited: the call hands back the
                // stream, and each item is awaited through `poll_next`
                // (ridl §12).
                Some(item) => {
                    let item = stream_item_tokens(interface, source_name, item)?;
                    out.consumer.push(quote! {
                        #doc
                        #deprecated
                        fn #method(&self, #(#params),*) -> impl RidlStream<Item = #item>;
                    });
                    out.provider.push(quote! {
                        #handler_doc
                        #deprecated
                        fn #handler(&mut self, #(#params),*) -> impl RidlStream<Item = #item>;
                    });
                }
                None => {
                    let ret = return_tokens(names, source_name, query, tuples);
                    out.consumer.push(quote! {
                        #doc
                        #deprecated
                        async fn #method(&self, #(#params),*) -> #ret;
                    });
                    out.provider.push(quote! {
                        #handler_doc
                        #deprecated
                        async fn #handler(&mut self, #(#params),*) -> #ret;
                    });
                }
            }

            push_contracts(interface, source_name, &query.contracts, &mut out.contracts)?;

            // As on a command: the declared RPC bounds ride the timing table
            // (ADR-0015).
            if let Some(spec) = &query.timing {
                out.timing.push(timing_entry(interface, source_name, spec)?);
            }
        }

        Some(v2::decl::Kind::FixedDef(fixed_def)) => {
            let method = method();
            let hint = format!("{}{}", camel_case(type_name), camel_case(source_name));
            let ty = fixed_def
                .payload
                .as_ref()
                .map(|ft| field_type_tokens(ft, &hint, names.visibility, tuples))
                .unwrap_or_else(|| quote! { () });
            let doc = doc_attrs(&doc_body(
                &decl.doc,
                &format!(
                    "fixed `{source_name}` — ordinal {ordinal} (ridl §8).\n\
                     Provisioned externally and immutable for the lifetime of the \
                     running software instance: read-only, free of the query \
                     machinery, and safe to cache unconditionally."
                ),
            ));
            out.consumer.push(quote! {
                #doc
                #deprecated
                fn #method(&self) -> &#ty;
            });
        }

        // A retired ordinal generates no method — it is recorded on the trait so
        // the gap in the ordinal sequence reads as intentional (ridl §3.4).
        Some(v2::decl::Kind::ReservedSlot(reserved)) => {
            let name = reserved.name.as_deref().unwrap_or("");
            let text = if name.is_empty() {
                format!(
                    "reserved ordinal {} — retired, never reused.",
                    reserved.ordinal
                )
            } else {
                format!(
                    "reserved ordinal {} (`{name}`) — retired, never reused.",
                    reserved.ordinal
                )
            };
            out.trait_doc.push(doc_attrs(&text));
        }

        // A typl declaration nested in an interface is vocabulary, already
        // emitted at package level; anything else is not an interaction.
        Some(_) | None => {}
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Parameters, returns, and streams.
// ---------------------------------------------------------------------------

fn param_tokens(
    names: Names,
    interaction: &str,
    params: &[v2::Param],
    tuples: &mut Vec<InducedTuple>,
) -> Result<Vec<TokenStream>, GenerateError> {
    let mut out = Vec::with_capacity(params.len());
    for p in params {
        let name = ident(&snake_case(&p.name));
        // A generated struct name is built from the TYPE name: the identity may
        // be a dotted address, which is not an identifier.
        let hint = format!(
            "{}{}{}",
            camel_case(names.r#type),
            camel_case(interaction),
            camel_case(&p.name)
        );
        let ty = match p.r#type.as_ref() {
            // A stream parameter is taken by `impl RidlStream` rather than a
            // concrete type: the caller owns the producer (ridl §12).
            Some(v2::FieldType {
                kind: Some(v2::field_type::Kind::Stream(stream)),
                ..
            }) => {
                let item = stream_item_tokens(names.identity, interaction, stream)?;
                quote! { impl RidlStream<Item = #item> }
            }
            Some(ft) => field_type_tokens(ft, &hint, names.visibility, tuples),
            None => quote! { () },
        };
        out.push(quote! { #name: #ty });
    }
    Ok(out)
}

/// The stream element of a query whose return is a stream, or `None`.
fn stream_return(query: &v2::QueryDef) -> Option<&v2::StreamType> {
    match query.return_type.as_ref()?.kind.as_ref()? {
        v2::return_type::Kind::Value(v2::FieldType {
            kind: Some(v2::field_type::Kind::Stream(stream)),
            ..
        }) => Some(stream),
        _ => None,
    }
}

fn return_tokens(
    names: Names,
    interaction: &str,
    query: &v2::QueryDef,
    tuples: &mut Vec<InducedTuple>,
) -> TokenStream {
    match query.return_type.as_ref().and_then(|r| r.kind.as_ref()) {
        // The failure vocabulary is closed and declared, so it maps to a native
        // `Result` — exhaustive by construction, no catch-all arm (ridl §10.1).
        Some(v2::return_type::Kind::Fallible(fallible)) => {
            let ok = type_path(&fallible.ok);
            let err = type_path(&fallible.err);
            quote! { Result<#ok, #err> }
        }
        Some(v2::return_type::Kind::Value(ft)) => {
            // The TYPE name, for the same reason as a parameter's hint.
            let hint = format!(
                "{}{}Result",
                camel_case(names.r#type),
                camel_case(interaction)
            );
            field_type_tokens(ft, &hint, names.visibility, tuples)
        }
        None => quote! { () },
    }
}

/// The Rust type of a stream's element.
///
/// A stream carries a named type or one of exactly two primitives: ridl §12.2
/// admits `string` and `bytes` only, enforced as RIDL-202. Any other primitive
/// is an inconsistent IR rather than a gap in this mapping, so it is refused
/// instead of being emitted as a type the contract never allowed.
fn stream_item_tokens(
    interface: &str,
    interaction: &str,
    stream: &v2::StreamType,
) -> Result<TokenStream, GenerateError> {
    match &stream.element {
        Some(v2::stream_type::Element::Named(name)) => Ok(type_path(name)),
        Some(v2::stream_type::Element::Primitive(prim)) => {
            match v2::PrimitiveType::try_from(*prim).unwrap_or(v2::PrimitiveType::Unspecified) {
                v2::PrimitiveType::String | v2::PrimitiveType::Bytes => Ok(primitive_tokens(*prim)),
                other => Err(GenerateError {
                    message: format!(
                        "{interface}.{interaction}: {other:?} is not a stream element type; \
                         a stream carries string or bytes (ridl §12.2, RIDL-202)"
                    ),
                }),
            }
        }
        None => Ok(quote! { () }),
    }
}

// ---------------------------------------------------------------------------
// Timing and contract metadata.
// ---------------------------------------------------------------------------

/// One `(source name, TimingConst)` entry.
///
/// The bounds are exact decimal microsecond strings in the IR. They are emitted
/// as `u64` microseconds: microsecond is the IR's own base unit and every legal
/// ridl duration is a whole number of them, so the conversion is exact. A bound
/// that is *not* a whole microsecond — which the IR can carry, because a bound
/// that was written is never silently dropped even when its form is rejected —
/// is refused here rather than truncated. Codegen must not quietly weaken a
/// timing contract.
fn timing_entry(
    interface: &str,
    interaction: &str,
    spec: &v2::Timing,
) -> Result<TokenStream, GenerateError> {
    let mode = match v2::TimingMode::try_from(spec.mode).unwrap_or(v2::TimingMode::Unspecified) {
        v2::TimingMode::StrictPeriodic => quote! { TimingMode::StrictPeriodic },
        v2::TimingMode::Range => quote! { TimingMode::Range },
        v2::TimingMode::Unspecified => {
            return Err(GenerateError {
                message: format!(
                    "{interface}.{interaction}: timing mode is unspecified; the IR resolves \
                     every signal and event to concrete bounds (ridl §9.1)"
                ),
            });
        }
    };

    let min = micros_tokens(interface, interaction, "min", spec.min_us.as_deref())?;
    let max = micros_tokens(interface, interaction, "max", spec.max_us.as_deref())?;
    let default_applied = spec.default_applied;

    Ok(quote! {
        (#interaction, TimingConst {
            mode: #mode,
            min_us: #min,
            max_us: #max,
            default_applied: #default_applied,
        })
    })
}

fn micros_tokens(
    interface: &str,
    interaction: &str,
    bound: &str,
    value: Option<&str>,
) -> Result<TokenStream, GenerateError> {
    let Some(text) = value else {
        return Ok(quote! { None });
    };
    let micros: u64 = text.parse().map_err(|_| GenerateError {
        message: format!(
            "{interface}.{interaction}: {bound} timing bound `{text}` is not a whole number of \
             microseconds representable as u64; the Rust backend refuses it rather than rounding \
             an exact contract bound"
        ),
    })?;
    let literal = Literal::u64_unsuffixed(micros);
    Ok(quote! { Some(#literal) })
}

fn push_contracts(
    interface: &str,
    interaction: &str,
    contracts: &[v2::Contract],
    out: &mut Vec<TokenStream>,
) -> Result<(), GenerateError> {
    for contract in contracts {
        let kind = match v2::ContractKind::try_from(contract.kind)
            .unwrap_or(v2::ContractKind::Unspecified)
        {
            v2::ContractKind::Require => quote! { ContractKind::Require },
            v2::ContractKind::Ensure => quote! { ContractKind::Ensure },
            // Guessing here would silently reclassify an observer: a
            // `require` is checked before the call and an `ensure` after,
            // so defaulting to either one installs the clause at the wrong
            // moment (ridl §13).
            v2::ContractKind::Unspecified => {
                return Err(GenerateError {
                    message: format!(
                        "{interface}.{interaction}: contract `{id}` carries no kind; a clause \
                             is require or ensure (ridl §13)",
                        id = contract.observer_id
                    ),
                });
            }
        };
        let id = contract.observer_id.as_str();
        let source = contract.source.as_str();
        let signals = contract.signal_refs.iter().map(String::as_str);
        let params = contract.param_refs.iter().map(String::as_str);
        let uses_result = contract.uses_result;
        out.push(quote! {
            ContractStub {
                id: #id,
                kind: #kind,
                source: #source,
                signals: &[#(#signals),*],
                params: &[#(#params),*],
                uses_result: #uses_result,
            }
        });
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Services.
// ---------------------------------------------------------------------------

/// The service table: every declared address mapped to the interface that
/// answers at it (ridl §11).
///
/// The address stays the raw dotted string the contract declares — it is the
/// deployment identity, not a Rust name — while the interface column holds the
/// generated type prefix. For an inline shape the two therefore differ on
/// purpose: `("veh.hvac.cabin", "ServiceVehHvacCabin")`.
fn emit_services(package: &v2::Package) -> Result<TokenStream, GenerateError> {
    if package.services.is_empty() {
        return Ok(quote! {});
    }

    let mut entries = Vec::with_capacity(package.services.len());
    for service in &package.services {
        let address = service.name.as_str();
        let interface = match &service.shape {
            Some(v2::service::Shape::InterfaceRef(name)) => camel_case(name),
            Some(v2::service::Shape::Inline(_)) => inline_interface_name(&service.name),
            // An empty interface column would name no interface at all, so the
            // table would claim an address answers and not say by what.
            None => {
                return Err(GenerateError {
                    message: format!(
                        "service {address}: no shape; a service publishes an interface reference \
                         or an inline shape (ridl §11)"
                    ),
                });
            }
        };
        entries.push(quote! { (#address, #interface) });
    }

    Ok(quote! {
        /// Every service this package declares: the deployment address and the
        /// interface that answers at it (ridl §11).
        ///
        /// The address is the contract's own dotted identity, carried verbatim;
        /// the interface name is the generated Rust type prefix. For an inline
        /// shape they differ by construction, and the address — not the type
        /// name — is the identity the wire and the observers use.
        pub const SERVICES: &[(&str, &str)] = &[
            #(#entries),*
        ];
    })
}

/// The generated interface name for a service's inline shape: `Service` plus
/// the CamelCase of each dotted segment — `veh.hvac.cabin` becomes
/// `ServiceVehHvacCabin`.
fn inline_interface_name(address: &str) -> String {
    let mut name = String::from("Service");
    for segment in address.split('.') {
        name.push_str(&camel_case(segment));
    }
    name
}

// ---------------------------------------------------------------------------
// Doc comment bodies.
// ---------------------------------------------------------------------------

/// The gf §6.4 wording for a Stratum 3 failure, used verbatim wherever a
/// generated comment mentions transport failure. It is not a Rust error type
/// and never becomes one: the language does not declare it.
const TRANSPORT_FAILURE: &str = "A transport failure is an infrastructure failure — detected, undeclared \
     (gf §6.4): it is carried by runtime types, never by this signature.";

/// Composes one interaction's doc comment: the contract's own doc comment
/// first, then the generated explanation, separated by a blank line only when
/// the contract wrote a doc comment at all. An interaction with no doc comment
/// must not generate a leading empty `///` line.
fn doc_body(source_doc: &str, generated: &str) -> String {
    if source_doc.is_empty() {
        generated.to_string()
    } else {
        format!("{source_doc}\n\n{generated}")
    }
}

fn signal_doc(decl: &v2::Decl, signal: &v2::SignalDef) -> String {
    let name = &decl.name;
    let ordinal = decl.ordinal;
    let init = match signal.init.as_ref().and_then(|i| i.value.as_deref()) {
        Some("") | None => "the payload type's init value".to_string(),
        Some(value) => format!("`{value}`"),
    };
    let declared = match signal.declared_init.as_deref() {
        Some(text) => format!("\nThe channel overrides the type's init with `{text}`."),
        None => String::new(),
    };
    doc_body(
        &decl.doc,
        &format!(
            "signal `{name}` — ordinal {ordinal} (ridl §4).\n\
             The channel is never empty: a read before the provider's first \
             publication yields {init}, and every read carries its provenance \
             (`Init`, `Live`, or `Invalid` — ridl §4.4, §4.5).{declared}"
        ),
    )
}

fn command_doc(decl: &v2::Decl, contracts: &[v2::Contract]) -> String {
    let name = &decl.name;
    let ordinal = decl.ordinal;
    doc_body(
        &decl.doc,
        &format!(
            "command `{name}` — ordinal {ordinal} (ridl §6).\n\
             Always returns `()`. A delivery acknowledgment travels beneath the \
             call — the receiving binding confirms received and accepted for \
             execution — but it carries no functional payload, never reaches the \
             contract surface, and is not application-visible as a return value \
             (ridl §6.1). Observable results travel back as state.\n\
             {TRANSPORT_FAILURE}{}",
            contract_doc(contracts)
        ),
    )
}

fn query_doc(interface: &str, decl: &v2::Decl, query: &v2::QueryDef) -> String {
    let name = &decl.name;
    let ordinal = decl.ordinal;
    let identity = match query.return_type.as_ref().and_then(|r| r.kind.as_ref()) {
        Some(v2::return_type::Kind::Fallible(fallible)) => format!(
            "\ntransport identity: {}",
            v2::fallible_transport_identity(interface, decl.ordinal, fallible)
        ),
        _ => String::new(),
    };
    doc_body(
        &decl.doc,
        &format!(
            "query `{name}` — ordinal {ordinal} (ridl §7).\n\
             {TRANSPORT_FAILURE}{identity}{}",
            contract_doc(&query.contracts)
        ),
    )
}

fn contract_doc(contracts: &[v2::Contract]) -> String {
    let mut text = String::new();
    for contract in contracts {
        let kind = match v2::ContractKind::try_from(contract.kind)
            .unwrap_or(v2::ContractKind::Unspecified)
        {
            v2::ContractKind::Ensure => "ensure",
            _ => "require",
        };
        text.push_str(&format!("\n{kind}: {}", contract.source));
    }
    text
}

fn label_doc(labels: &[String]) -> TokenStream {
    if labels.is_empty() {
        return quote! {};
    }
    doc_attrs(&format!("labels: {}", labels.join(", ")))
}

// ---------------------------------------------------------------------------
// Name conversion.
// ---------------------------------------------------------------------------

/// snake_case of a camelCase ridl name: `currentSpeed` becomes `current_speed`.
/// An underscore already present is kept, and a run of capitals is not split, so
/// the mapping is stable under repeated application.
pub(crate) fn snake_case(name: &str) -> String {
    let mut out = String::new();
    let mut prev_lower_or_digit = false;
    for ch in name.chars() {
        if ch == '_' {
            out.push('_');
            prev_lower_or_digit = false;
            continue;
        }
        if ch.is_ascii_uppercase() {
            if prev_lower_or_digit {
                out.push('_');
            }
            out.extend(ch.to_lowercase());
            prev_lower_or_digit = false;
        } else {
            out.push(ch);
            prev_lower_or_digit = ch.is_ascii_lowercase() || ch.is_ascii_digit();
        }
    }
    out
}

/// SCREAMING_SNAKE_CASE of a name, for the generated metadata constants.
fn screaming_snake(name: &str) -> String {
    snake_case(name).to_uppercase()
}

// ---------------------------------------------------------------------------
// Name collisions with the generated vocabulary.
// ---------------------------------------------------------------------------

/// The vocabulary types this module emits into every interaction-carrying
/// package. The names are fixed by this backend and are the contract between a
/// generated module and its runtime. All of them are types.
const VOCABULARY_NAMES: [&str; 8] = [
    "Provenance",
    "SignalHandle",
    "EventHandle",
    "RidlStream",
    "TimingMode",
    "TimingConst",
    "ContractKind",
    "ContractStub",
];

/// Which Rust namespace a name occupies. Rust resolves types and values
/// separately, so `struct VEHICLE_STATUS_TIMING` and
/// `const VEHICLE_STATUS_TIMING` coexist and only a same-namespace clash is an
/// error. Checking without this distinction would reject contracts that
/// compile perfectly well.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Namespace {
    Type,
    Value,
}

/// The namespace a declaration's generated item lands in: a constant becomes a
/// `const`, and every other typl declaration becomes a struct or an enum.
fn decl_namespace(decl: &v2::Decl) -> Namespace {
    match &decl.kind {
        Some(v2::decl::Kind::ConstDef(_)) => Namespace::Value,
        _ => Namespace::Type,
    }
}

/// Refuses a package whose generated module would declare one name twice.
///
/// Two distinct failures share this check, because both end as `error[E0428]`
/// in a file this backend handed to rustc:
///
/// 1. **A typl declaration collides with a generated name.** A package
///    declaring a type named `Provenance` emits that type *and* the vocabulary
///    enum under it.
/// 2. **Two generated names collide with each other.** Nothing upstream keeps
///    them apart: `interface vehicleStatus` and `interface VehicleStatus` are
///    distinct declarations that `camel_case` maps to one type name, and an
///    `interface ServiceVehAdasLogs` collides with the interface generated for
///    an inline `service veh.adas.logs`. Neither draws a ridl diagnostic.
///
/// Codegen has to name both itself rather than hand rustc a broken module: the
/// failure belongs to the contract, and a diagnostic naming the construct is
/// actionable where a downstream compiler error is not.
///
/// **Which names are known when this runs.** Everything the module emits at
/// module scope, which is what makes the check sound:
///
/// - the vocabulary types, including `ContractKind` — all in [`VOCABULARY_NAMES`];
/// - the `{Type}Consumer` / `{Type}Provider` faces and the `{TYPE}_TIMING` /
///   `{TYPE}_CONTRACTS` consts of **every** interface — `owners` carries the
///   inline service shapes as well as the declared ones, because
///   `package.interfaces` is not the complete set;
/// - the `SERVICES` const, when the package declares any service;
/// - **every** struct generated for a tuple in an interaction position, at any
///   depth. Two different mechanisms put them in `generated_tuples`:
///   `field_type_tokens` recurses synchronously through arrays, maps and
///   optionals, so those names appear during emission; a tuple nested directly
///   inside another tuple is found only by emitting the outer tuple's fields,
///   which the caller does after this module returns, so `emit` runs that walk
///   itself first. Without it the nested names would escape — "after emission"
///   is not the same as "after every name exists".
///
/// Only `package.decls` is scanned on the input side: a typl declaration nested
/// inside an interface is rejected upstream (RIDL-107), so package scope is the
/// whole of the input vocabulary.
/// Claims `name` in `namespace` for `origin`, or reports the construct holding
/// it already. Two generated names colliding would emit a module with a
/// duplicated declaration, so it is caught here rather than left to rustc.
fn claim(
    claimed: &mut HashMap<(String, Namespace), String>,
    generated: &mut Vec<(String, Namespace, String)>,
    name: String,
    namespace: Namespace,
    origin: String,
) -> Result<(), GenerateError> {
    if let Some(previous) = claimed.get(&(name.clone(), namespace)) {
        return Err(GenerateError {
            message: format!(
                "the generated name {name} is claimed by both {previous} and {origin}"
            ),
        });
    }
    claimed.insert((name.clone(), namespace), origin.clone());
    generated.push((name, namespace, origin));
    Ok(())
}

fn check_name_collisions(
    package: &v2::Package,
    owners: &[(String, String)],
    generated_tuples: &[InducedTuple],
) -> Result<(), GenerateError> {
    // (name, namespace) -> the construct that generated it. Building this map
    // detects generated-vs-generated collisions on its own.
    let mut claimed: HashMap<(String, Namespace), String> = HashMap::new();
    // The same entries in emission order, so the diagnostic a contract sees for
    // a declaration collision is stable.
    let mut generated: Vec<(String, Namespace, String)> = Vec::new();

    for name in VOCABULARY_NAMES {
        claim(
            &mut claimed,
            &mut generated,
            name.to_string(),
            Namespace::Type,
            "the generated interaction vocabulary".to_string(),
        )?;
    }

    if !package.services.is_empty() {
        claim(
            &mut claimed,
            &mut generated,
            "SERVICES".to_string(),
            Namespace::Value,
            "the generated service table".to_string(),
        )?;
    }

    for (type_name, origin) in owners {
        let camel = camel_case(type_name);
        let screaming = screaming_snake(type_name);
        for suffix in ["Consumer", "Provider"] {
            claim(
                &mut claimed,
                &mut generated,
                format!("{camel}{suffix}"),
                Namespace::Type,
                format!("the {suffix} face of {origin}"),
            )?;
        }
        claim(
            &mut claimed,
            &mut generated,
            format!("{screaming}_TIMING"),
            Namespace::Value,
            format!("the timing table of {origin}"),
        )?;
        claim(
            &mut claimed,
            &mut generated,
            format!("{screaming}_CONTRACTS"),
            Namespace::Value,
            format!("the contract table of {origin}"),
        )?;
    }

    for induced in generated_tuples {
        let name = &induced.name;
        // A tuple name can legitimately be discovered twice — the worklist is
        // deduplicated by the caller — so a repeat is not a collision.
        if claimed.contains_key(&(name.clone(), Namespace::Type)) {
            continue;
        }
        claim(
            &mut claimed,
            &mut generated,
            name.clone(),
            Namespace::Type,
            "a struct generated for a tuple in an interaction position".to_string(),
        )?;
    }

    for decl in &package.decls {
        let namespace = decl_namespace(decl);
        for (name, generated_namespace, what) in &generated {
            if *generated_namespace == namespace && decl.name == *name {
                return Err(GenerateError {
                    message: format!(
                        "declaration {name} collides with {what}; a module carrying interactions \
                         generates that name itself",
                    ),
                });
            }
        }
    }

    Ok(())
}
