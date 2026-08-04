//! IR v2 interaction layer to TypeScript source (E2.6b, ADR-0008 decision 7).
//!
//! The ridl interaction layer of a [`v2::Package`] — interfaces, their
//! interactions, and services — is realized alongside the typl surface in the
//! same module. The TypeScript interaction mapping, fixed by this backend:
//!
//! - **A runtime-neutral vocabulary**, emitted once per module that carries
//!   any interaction: `Provenance`, `SignalHandle<T>`, `EventHandle<T>`, and
//!   `Result<T, E>`. It names no transport and imports nothing — a generated
//!   module compiles against any runtime that supplies these shapes.
//! - **Two faces per interface `X`** (ridl §14): `XConsumer` is what a
//!   consumer holds, `XProvider` is what a provider implements. An interface
//!   is one abstract shape, but the two sides of a binding do not see the
//!   same operations — a consumer reads and subscribes to a signal while a
//!   provider publishes it — so the shape is realized as two types rather
//!   than one type nobody can implement. Commands and queries keep one shape
//!   on both sides. A `fixed` appears on the consumer face only: it is
//!   provisioned externally and initiated by neither side (ridl §3, §8).
//! - **Fallible queries split mechanically** (ridl §10.1): the success arm is
//!   the reply payload and the error arm is declared vocabulary, so an inline
//!   `T | E` return becomes `Promise<Result<Ok, Err>>` — errors are data, and
//!   ridl has no `throws`.
//! - **Timing as data** — `export const xTiming` carries the resolved bounds
//!   (ridl §9) as **bigint microseconds**. The IR holds exact decimal
//!   microsecond strings (ADR-0008 decision 12); `number` would round a bound
//!   past 2^53 and `bigint` is the only TypeScript form that cannot, so the
//!   exactness the IR guarantees survives into the generated module.
//! - **Contracts as data** — `export const xContracts` carries the E2.5
//!   observer stubs, so a runtime can build the observers without parsing the
//!   generated source.
//! - **Services as data** — `export const services` maps each dotted global
//!   service name (ridl §14.5) to the interface behind it.
//! - **Wire identity as JSDoc tags** — `@ordinal` on every interaction and
//!   `@reserved` on the consumer face for every tombstone (ridl §11).
//!   TypeScript has no native place for either, so they follow the same
//!   custom-tag route as `@provenance`, `@transportIdentity`, and `@init`.
//!   The interaction layer deliberately diverges here from the typl
//!   struct-field rule, which drops a tombstone silently (typl §7.4): an
//!   interaction ordinal is the identity a binding is keyed on — transport
//!   ids derive from it and `ridl diff` rejects any change that shifts or
//!   reuses one — so erasing it would leave a TypeScript consumer unable to
//!   reconstruct the wire identity the Rust backend states. The Rust
//!   interaction layer made the same divergence first; this follows it.
//!
//! **Visibility.** An `internal interface` emits four module-local shapes —
//! both faces and both consts, each without `export` — never an exported one,
//! the same package-private mapping the typl surface gives an `internal`
//! declaration (ADR-0002 §8, ADR-0008 decision 7). Exporting the API of a
//! declaration the keyword hides would defeat the modifier. The vocabulary and
//! the service map are package-level and stay exported. There is no
//! TypeScript counterpart to the Rust backend's generated tuple structs: a
//! tuple is an inline object type here (typl §11), so it has no name to hide.
//!
//! Transport failures are not modeled in the emitted types. A rejected
//! promise is Stratum 3 — an `infrastructure failure — detected, undeclared`
//! (gf §6.4), never "undefined behavior": the runtime detects it, the
//! contract language does not describe it. Generated comments carry that
//! wording verbatim.
//!
//! The emitter shares the disciplines of the typl-surface emitter in
//! [`crate`]: a plain string emitter, deterministic stable source-order
//! output, and total — every failure is a [`GenerateError`], never a panic
//! (ADR-0004 section 5).

use crate::{
    Ctx, GenerateError, deprecated_tags, export_kw, is_integer_form, jsdoc, kind_ts, ts_string,
};
use ridl_ir::v2;
use std::collections::BTreeMap;

/// The Stratum-3 wording, verbatim from gf §6.4. The em dash is U+2014 and
/// the phrase is normative: Stratum 3 is never described as undefined
/// behavior.
const STRATUM_3: &str = "infrastructure failure \u{2014} detected, undeclared";

/// The runtime-neutral vocabulary, emitted once per module that carries any
/// interaction. The names are fixed by this backend and are the contract
/// between a generated module and its runtime.
const VOCABULARY_NAMES: [&str; 4] = ["Provenance", "SignalHandle", "EventHandle", "Result"];

/// Emits the interaction-layer blocks for a package: the vocabulary, the two
/// faces plus the timing and contract data of every interface, the generated
/// interfaces of inline service shapes, and the service map.
///
/// Returns an empty vector for a package with no interfaces and no services —
/// a typl-only module carries no interaction vocabulary.
pub(crate) fn emit_package(ctx: &Ctx, package: &v2::Package) -> Result<Vec<String>, GenerateError> {
    if package.interfaces.is_empty() && package.services.is_empty() {
        return Ok(Vec::new());
    }
    check_name_collisions(package)?;

    let mut blocks = vec![vocabulary()];

    // Every interface shape, named and inline alike — `Package::shapes`, not
    // `package.interfaces`, which is not the complete set. A named interface is
    // its own identity: the type name and the identity name are the same
    // string. An inline service shape carries no name of its own (ridl §14.5);
    // its generated interface is named after the service, the note saying so
    // rides in the faces' own docs rather than as a detached comment, and its
    // identity is the service's DOTTED name — never the mangled type name and
    // never `Interface.name`, which is "" by construction for an inline shape.
    for shape in package.shapes() {
        let type_name = match shape.service {
            Some(service) => inline_interface_name(&service.name),
            None => shape.name.to_string(),
        };
        let note = shape
            .service
            .map(|service| inline_shape_note(&service.name, &type_name));
        let names = Names {
            r#type: &type_name,
            identity: shape.name,
            visibility: shape.visibility(),
        };
        emit_interface(ctx, names, shape.interface, note.as_deref(), &mut blocks)?;
    }
    if !package.services.is_empty() {
        blocks.push(emit_services(package)?);
    }
    Ok(blocks)
}

// ---------------------------------------------------------------------------
// The vocabulary.
// ---------------------------------------------------------------------------

fn vocabulary() -> String {
    format!(
        "\
/**
 * What a signal read is standing on (ridl §4.5). `'init'` — the provider has
 * not published yet and the value is the channel init (ridl §4.4); `'live'` —
 * a published value; `'invalid'` — the channel is in the invalid state
 * because a published payload violated its constraints, and the value is the
 * last good one. A read always yields a value; provenance is what says
 * whether to act on it. Invalidity is never hidden from a subscriber.
 */
export type Provenance = 'init' | 'live' | 'invalid';

/**
 * A consumer's handle on a signal (ridl §4): continuous state, never empty —
 * the channel holds a value from the moment it exists, so a read always
 * succeeds and a subscription delivers immediately (ridl §4.4).
 * `subscribe` returns the function that cancels it.
 */
export interface SignalHandle<T> {{
  read(): {{ value: T; provenance: Provenance }};
  subscribe(fn: (value: T, provenance: Provenance) => void): () => void;
}}

/**
 * A consumer's handle on an event (ridl §5): discrete occurrences, with no
 * readable current value — an event that has not occurred has nothing to
 * read. `subscribe` returns the function that cancels it.
 */
export interface EventHandle<T> {{
  subscribe(fn: (occurrence: T) => void): () => void;
}}

/**
 * The result of a fallible query (ridl §7, gf §6.1): a declared Stratum-1
 * outcome the provider owns. A transport failure is not an `error` here — it
 * rejects the promise instead, an {STRATUM_3} (gf §6.4).
 */
export type Result<T, E> =
  | {{ ok: true; value: T }}
  | {{ ok: false; error: E }};
"
    )
}

// ---------------------------------------------------------------------------
// Interface faces.
// ---------------------------------------------------------------------------

fn emit_interface(
    ctx: &Ctx,
    names: Names,
    interface: &v2::Interface,
    note: Option<&str>,
    blocks: &mut Vec<String>,
) -> Result<(), GenerateError> {
    blocks.push(emit_face(ctx, names, interface, note, Face::Consumer)?);
    blocks.push(emit_face(ctx, names, interface, note, Face::Provider)?);
    blocks.push(emit_timing(names, interface)?);
    blocks.push(emit_contracts(names, interface)?);
    Ok(())
}

/// The two names an interface is generated under, which are not always the
/// same string.
///
/// - `type` names the generated TypeScript interfaces (`{type}Consumer`,
///   `{type}Provider`) and the generated consts. It must be a TypeScript
///   identifier, so an inline service shape's is the mangled
///   `Service_veh_adas_logs`.
/// - `identity` is what the interface is called everywhere OUTSIDE this
///   module: the first component of a fallible return's transport identity
///   (ADR-0008 decision 4) and the prefix of its observer-stub ids. For a
///   named interface it is the interface name; for an inline service shape it
///   is the service's **dotted global name**.
///
/// Keeping these apart is load-bearing, not cosmetic. An inline shape's
/// `Interface.name` is `""` by construction (ridl §14.5), so deriving the
/// identity from it would emit `#3:Ok|Err` — which is not an identity at all,
/// since two different services with a fallible query at the same ordinal
/// over the same arms would collide. It would also disagree with two things
/// that already exist: `ridl diff` keys a service's interactions on the
/// dotted name (`crates/ridl-diff/src/walk.rs`, `diff_services`), and the observer
/// stubs lowered into this very module are scoped to the dotted name
/// (`ridl-sem`, `lower_service_inline`, E2.5). One value, three consumers —
/// they have to agree.
///
/// `visibility` is the third value, and it is here for the same reason: it is
/// the AUTHORITATIVE visibility, taken from [`v2::InterfaceShape::visibility`]
/// rather than off the `Interface` this module is handed. An inline shape's
/// own `Interface.visibility` is `VISIBILITY_UNSPECIFIED` by construction and
/// the owning `Service` carries the real one, so reading it here rather than
/// at the leaf keeps the emitted `export` correct by derivation instead of by
/// the coincidence that [`export_kw`] maps `UNSPECIFIED` and `PUBLIC` to the
/// same keyword.
#[derive(Debug, Clone, Copy)]
struct Names<'a> {
    r#type: &'a str,
    identity: &'a str,
    visibility: i32,
}

/// Which side of a binding a face is generated for (ridl §14).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Face {
    Consumer,
    Provider,
}

impl Face {
    fn suffix(self) -> &'static str {
        match self {
            Face::Consumer => "Consumer",
            Face::Provider => "Provider",
        }
    }

    fn summary(self) -> &'static str {
        match self {
            Face::Consumer => {
                "The consumer face of this interface (ridl §14): what code \
                 holding a binding to it sees. Signals are read and \
                 subscribed, events are subscribed, commands and queries are \
                 called, fixed values are read."
            }
            Face::Provider => {
                "The provider face of this interface (ridl §14): what code \
                 realizing it implements. Signals and events are published, \
                 commands and queries are handled. Fixed values are absent here — \
                 they are provisioned externally (ridl §8) and initiated by \
                 neither side (ridl §3), so they appear on the consumer face \
                 only."
            }
        }
    }
}

fn emit_face(
    ctx: &Ctx,
    names: Names,
    interface: &v2::Interface,
    note: Option<&str>,
    face: Face,
) -> Result<String, GenerateError> {
    // The declared doc first, then any generated note, then the summary of
    // what this face is — most specific to most general.
    let mut paragraphs: Vec<String> = Vec::new();
    if !interface.doc.is_empty() {
        paragraphs.push(interface.doc.clone());
    }
    if let Some(note) = note {
        paragraphs.push(wrap(note));
    }
    paragraphs.push(wrap(face.summary()));
    // The retired ordinals, then the deprecation — the Rust backend's order
    // for the same two pieces of interface-level metadata.
    let mut tags = reserved_tags(interface, face);
    tags.extend(deprecated_tags(interface.deprecated.as_deref()));
    let doc = jsdoc("", &paragraphs.join("\n\n"), &tags);

    let mut members = String::new();
    for decl in &interface.interactions {
        // The IDENTITY name, not the type name: a member's transport identity
        // is what the interface is called outside this module.
        members.push_str(&emit_member(ctx, names.identity, decl, face)?);
    }

    let face_name = format!("{type_name}{}", face.suffix(), type_name = names.r#type);
    let export = export_kw(names.visibility);
    if members.is_empty() {
        Ok(format!("{doc}{export}interface {face_name} {{}}\n"))
    } else {
        Ok(format!(
            "{doc}{export}interface {face_name} {{\n{members}}}\n"
        ))
    }
}

/// Emits one member of a face.
///
/// Two kinds of malformed IR are treated differently here, deliberately.
/// Something the checker cannot produce and that has no honest rendering is a
/// [`GenerateError`] — an unresolved timing mode, a stream element that is
/// neither string nor bytes, a typl declaration inside an interface. But an
/// absent optional sub-message (`StreamType.element`, `FixedDef.payload`,
/// `Param.type`, `QueryDef.return_type`) falls back to `unknown` or
/// `Promise<void>` instead. Every one of those is rejected upstream — a void
/// query is RIDL-105, a payload-less fixed is RIDL-106 — so neither branch is
/// reachable from a compiled package. The split is about what a hand-built or
/// truncated IR deserves: `unknown` is honest about missing information and
/// keeps the emitter total, while a guessed *type* would be a silent lie that
/// compiles. Nothing downstream depends on these fallbacks.
fn emit_member(
    ctx: &Ctx,
    interface: &str,
    decl: &v2::Decl,
    face: Face,
) -> Result<String, GenerateError> {
    let name = &decl.name;
    match &decl.kind {
        Some(v2::decl::Kind::SignalDef(signal)) => {
            let payload = ctx.type_ref(&signal.payload);
            let mut tags = vec![ordinal_tag(decl.ordinal)];
            tags.extend(signal_tags(signal, face));
            tags.extend(deprecated_tags(decl.deprecated.as_deref()));
            let doc = jsdoc("  ", &decl.doc, &tags);
            let shape = match face {
                Face::Consumer => format!("SignalHandle<{payload}>"),
                Face::Provider => format!("{{ publish(value: {payload}): void }}"),
            };
            Ok(format!("{doc}  {name}: {shape};\n"))
        }
        Some(v2::decl::Kind::EventDef(event)) => {
            let payload = ctx.type_ref(&event.payload);
            let mut tags = vec![ordinal_tag(decl.ordinal)];
            tags.extend(deprecated_tags(decl.deprecated.as_deref()));
            let doc = jsdoc("  ", &decl.doc, &tags);
            let shape = match face {
                Face::Consumer => format!("EventHandle<{payload}>"),
                Face::Provider => format!("{{ publish(occurrence: {payload}): void }}"),
            };
            Ok(format!("{doc}  {name}: {shape};\n"))
        }
        Some(v2::decl::Kind::CommandDef(command)) => {
            let params = params_ts(ctx, &command.params)?;
            // ridl §6.1: the runtime protocol carries a delivery
            // acknowledgment beneath every command, but it is not a return
            // value — the promise resolves with nothing.
            let note = wrap(
                "A command is fire-and-forget (ridl §6): it declares no result, and an \
                 observable outcome travels back as state, not as a return value. The \
                 promise settles on the runtime's delivery acknowledgment, which \"carries \
                 no functional payload, never reaches the contract surface, and is not \
                 application-visible as a return value\" (ridl §6.1).",
            );
            let body = if decl.doc.is_empty() {
                note
            } else {
                format!("{}\n\n{note}", decl.doc)
            };
            let mut tags = vec![ordinal_tag(decl.ordinal), transport_tag()];
            tags.extend(deprecated_tags(decl.deprecated.as_deref()));
            let doc = jsdoc("  ", &body, &tags);
            Ok(format!("{doc}  {name}({params}): Promise<void>;\n"))
        }
        Some(v2::decl::Kind::QueryDef(query)) => {
            let params = params_ts(ctx, &query.params)?;
            let ret = return_ts(ctx, query.return_type.as_ref())?;
            let mut tags = vec![ordinal_tag(decl.ordinal)];
            if let Some(identity) = transport_identity(interface, decl.ordinal, query) {
                tags.push(identity);
            }
            tags.push(transport_tag());
            tags.extend(deprecated_tags(decl.deprecated.as_deref()));
            let doc = jsdoc("  ", &decl.doc, &tags);
            Ok(format!("{doc}  {name}({params}): {ret};\n"))
        }
        // A fixed is provisioned externally — build, factory, FOTA — and is
        // immutable for the software-instance lifetime (ridl §8). It appears
        // on the CONSUMER face only, as a plain readonly accessor.
        //
        // The provider face omits it because a provider has no role in it.
        // The §3 interaction-model table gives every kind an initiator —
        // "provider publishes" for signal and event, "consumer calls" for
        // command and query — and gives `fixed` "neither (provisioned)", the
        // one kind that names no side. §14.6 says what providing a service
        // means ("produces its signals/events, accepts its commands/queries")
        // and lists four kinds, not five. A provider cannot publish a fixed
        // (there is no channel), cannot answer a call for one (§8: reading it
        // is "free of the query machinery"), and cannot write one (it is
        // written once, elsewhere). Emitting `readonly vin: Vin` on the
        // provider face could only mean "the implementer furnishes this",
        // which is an obligation §8 places on the provisioning plane instead.
        //
        // The wire mapping does not contradict this: Appendix B realizes a
        // fixed as a SOME/IP getter, but a binding serves that from the
        // provisioning source, the same way it serves a signal getter from
        // its last-value cache — a transport detail, not an application API.
        Some(v2::decl::Kind::FixedDef(fixed_def)) if face == Face::Consumer => {
            let payload = match fixed_def.payload.as_ref() {
                Some(ft) => kind_ts(ctx, ft.kind.as_ref())?,
                None => "unknown".to_string(),
            };
            let mut tags = vec![ordinal_tag(decl.ordinal)];
            tags.extend(deprecated_tags(decl.deprecated.as_deref()));
            let doc = jsdoc("  ", &decl.doc, &tags);
            Ok(format!("{doc}  readonly {name}: {payload};\n"))
        }
        Some(v2::decl::Kind::FixedDef(_)) => Ok(String::new()),
        // A reserved tombstone holds an ordinal and declares no member, so it
        // has no member doc to carry it; it is recorded on the face instead,
        // by [`reserved_tags`] (ridl §11).
        Some(v2::decl::Kind::ReservedSlot(_)) | None => Ok(String::new()),
        // A typl declaration kind inside an interface is an IR
        // inconsistency: interfaces hold interactions only (ridl §14).
        Some(_) => Err(GenerateError::Unrepresentable(format!(
            "interface {interface}: member {name} is a typl declaration, but an \
             interface holds interactions only (ridl §14)"
        ))),
    }
}

/// The JSDoc tag carrying the Stratum-3 wording verbatim (gf §6.4). Every
/// promise-returning member gets it: the rejection channel is real and
/// detected, and it is deliberately absent from the declared types.
fn transport_tag() -> String {
    format!("@remarks A rejected promise is an {STRATUM_3} (gf §6.4).")
}

/// The wire identity of one interaction (ridl §11): the 1-based declaration
/// order across every interaction of the enclosing interface, one sequence
/// regardless of kind, tombstones counted. A tag-based transport derives its
/// numeric ids from it (SOME/IP method and event ids, Appendix B), and
/// `ridl diff` rejects any change that shifts or reuses one, so it is the
/// identity a consumer binds to rather than a presentation detail.
///
/// It is emitted on **every** interaction, on every face that carries the
/// interaction, rather than only where a reader could not count it out. A
/// member's position in a generated face is not its ordinal, in three
/// independent ways: the provider face omits `fixed` members (ridl §3, §8), so
/// the two faces of one interface number differently; a `reserved` tombstone
/// declares no member at all, so every ordinal after one is displaced; and the
/// consumer face is the only place the tombstones are recorded, so the
/// provider face carries no local evidence of the gap. Emitting the tag only
/// where counting fails would make its absence mean "count from one", a rule a
/// reader can apply only after establishing the very fact the tag states. The
/// Rust backend names the ordinal in every interaction's doc comment for the
/// same reason.
fn ordinal_tag(ordinal: u32) -> String {
    format!("@ordinal {ordinal}")
}

/// The retired ordinals of an interface, in declaration order (ridl §11).
///
/// A tombstone is the one interaction that generates no member — it exists to
/// hold an ordinal against reuse — so it has no member doc of its own and is
/// recorded on the face instead. Without it a TypeScript consumer sees a gap
/// in the `@ordinal` sequence with nothing saying the gap is deliberate, which
/// is precisely the wire identity `ridl diff` guards.
///
/// The wording is the Rust backend's verbatim, and so is the placement: this
/// is interface-level metadata, recorded once, on the consumer face — the face
/// a binding consumer reads — rather than repeated on both.
fn reserved_tags(interface: &v2::Interface, face: Face) -> Vec<String> {
    if face != Face::Consumer {
        return Vec::new();
    }
    interface
        .interactions
        .iter()
        .filter_map(|decl| match &decl.kind {
            Some(v2::decl::Kind::ReservedSlot(slot)) => Some(reserved_tag(slot)),
            _ => None,
        })
        .collect()
}

/// One tombstone. `reserved legacyTicks` retires a named interaction and
/// `reserved 3` retires a bare ordinal (typl §7.4 grammar, ridl §11); the
/// nameless form has nothing to quote, so it states the ordinal alone rather
/// than an empty pair of backticks.
fn reserved_tag(slot: &v2::Reserved) -> String {
    let ordinal = slot.ordinal;
    match slot.name.as_deref() {
        Some(name) if !name.is_empty() => {
            format!("@reserved ordinal {ordinal} (`{name}`) — retired, never reused.")
        }
        _ => format!("@reserved ordinal {ordinal} — retired, never reused."),
    }
}

/// The tags of a signal. `@init` is the resolved channel init (ridl §4.4) and
/// belongs to both faces — it is what the channel holds before the provider
/// publishes. `@provenance` is a consumer's concern only: a provider
/// publishes values, it does not read them back with a provenance.
fn signal_tags(signal: &v2::SignalDef, face: Face) -> Vec<String> {
    let mut tags = Vec::new();
    // A canonical init text can be empty, and two unrelated values produce it:
    // the empty string of a string- or bytes-backed payload, and the empty set
    // of an enum-set payload (`ridl-sem`'s `string_init` and the `EnumSet` arm
    // of its named-init resolver, both typl §5.8). This emitter sees the text,
    // not the payload's declaration, so it cannot render either one without
    // guessing which it holds — and a guessed literal that compiles is worse
    // than none. It states nothing instead, the way an absent value is already
    // treated, rather than emitting a tag with nothing after it. The Rust
    // backend collapses the same case onto one sentence. Telling the two apart
    // is a lowering question — a type-appropriate literal in the IR — not a
    // rendering one.
    if let Some(value) = signal
        .init
        .as_ref()
        .and_then(|i| i.value.as_deref())
        .filter(|value| !value.is_empty())
    {
        tags.push(format!("@init {value}"));
    }
    if face == Face::Consumer {
        tags.push(
            "@provenance `'init'` until the provider's first publication (ridl §4.4).".to_string(),
        );
    }
    tags
}

/// The synthesized transport identity of an inline `T | E` return, from the
/// single IR derivation (ADR-0008 decision 4) — never hand-built here, so
/// every consumer of the identity agrees.
fn transport_identity(interface: &str, ordinal: u32, query: &v2::QueryDef) -> Option<String> {
    let Some(v2::return_type::Kind::Fallible(fallible)) =
        query.return_type.as_ref().and_then(|rt| rt.kind.as_ref())
    else {
        return None;
    };
    Some(format!(
        "@transportIdentity {}",
        v2::fallible_transport_identity(interface, ordinal, fallible)
    ))
}

/// The parameter list of a command or query (ridl §6, §7). A stream
/// parameter takes an `AsyncIterable` (ridl §12); an optional parameter uses
/// the `name?:` form.
fn params_ts(ctx: &Ctx, params: &[v2::Param]) -> Result<String, GenerateError> {
    let mut rendered = Vec::with_capacity(params.len());
    for param in params {
        let (optional, ty) = match param.r#type.as_ref() {
            Some(ft) => (ft.optional, interaction_type_ts(ctx, ft)?),
            None => (false, "unknown".to_string()),
        };
        let marker = if optional { "?" } else { "" };
        rendered.push(format!("{}{marker}: {ty}", param.name));
    }
    Ok(rendered.join(", "))
}

/// The return shape of a query (ridl §7). A plain value resolves a promise; a
/// fallible `T | E` resolves a [`Result`]; a stream is an `AsyncIterable`
/// directly — a stream is consumed as it arrives, so there is no single
/// moment for a promise to resolve (ridl §12).
fn return_ts(ctx: &Ctx, ret: Option<&v2::ReturnType>) -> Result<String, GenerateError> {
    match ret.and_then(|rt| rt.kind.as_ref()) {
        Some(v2::return_type::Kind::Fallible(fallible)) => Ok(format!(
            "Promise<Result<{ok}, {err}>>",
            ok = ctx.type_ref(&fallible.ok),
            err = ctx.type_ref(&fallible.err)
        )),
        Some(v2::return_type::Kind::Value(ft)) => {
            if matches!(ft.kind, Some(v2::field_type::Kind::Stream(_))) {
                interaction_type_ts(ctx, ft)
            } else {
                Ok(format!("Promise<{}>", interaction_type_ts(ctx, ft)?))
            }
        }
        // A query with no return shape carries nothing back; total rather
        // than a guess.
        None => Ok("Promise<void>".to_string()),
    }
}

/// The TypeScript type of an interaction-position field type: the typl
/// mapping, plus the ridl-owned stream container (ridl §12), which has no
/// typl-surface position and so is unmapped by the typl emitter.
fn interaction_type_ts(ctx: &Ctx, ft: &v2::FieldType) -> Result<String, GenerateError> {
    match &ft.kind {
        Some(v2::field_type::Kind::Stream(stream)) => {
            let element = match &stream.element {
                Some(v2::stream_type::Element::Named(name)) => ctx.type_ref(name),
                Some(v2::stream_type::Element::Primitive(prim)) => {
                    match v2::PrimitiveType::try_from(*prim)
                        .unwrap_or(v2::PrimitiveType::Unspecified)
                    {
                        v2::PrimitiveType::String => "string".to_string(),
                        v2::PrimitiveType::Bytes => "Uint8Array".to_string(),
                        // RIDL-202 admits STRING and BYTES only; anything
                        // else is an IR inconsistency, not a mapping gap.
                        other => {
                            return Err(GenerateError::Unrepresentable(format!(
                                "stream element primitive {other:?} is not a stream element \
                                 type; streams carry string or bytes (RIDL-202)"
                            )));
                        }
                    }
                }
                None => "unknown".to_string(),
            };
            Ok(format!("AsyncIterable<{element}>"))
        }
        _ => kind_ts(ctx, ft.kind.as_ref()),
    }
}

// ---------------------------------------------------------------------------
// Timing data.
// ---------------------------------------------------------------------------

/// The resolved timing of every timed interaction, as data (ridl §9). Bounds
/// are **bigint microseconds**: the IR carries exact decimal microsecond
/// strings (ADR-0008 decision 12), and `bigint` is the only TypeScript
/// numeric form that holds them without rounding. A command's or query's
/// entry carries its declared RPC bounds — the call throttle and the response
/// bound (ADR-0015 decision 3) — and is absent when undeclared, because an
/// RPC bound is never defaulted.
fn emit_timing(names: Names, interface: &v2::Interface) -> Result<String, GenerateError> {
    // Diagnostics name the interface the way the author wrote it, so the
    // identity name is the one that belongs in an error message.
    let owner = names.identity;
    let mut entries = String::new();
    for decl in &interface.interactions {
        let timing = match &decl.kind {
            Some(v2::decl::Kind::SignalDef(signal)) => signal.timing.as_ref(),
            Some(v2::decl::Kind::EventDef(event)) => event.timing.as_ref(),
            Some(v2::decl::Kind::CommandDef(command)) => command.timing.as_ref(),
            Some(v2::decl::Kind::QueryDef(query)) => query.timing.as_ref(),
            _ => None,
        };
        let Some(timing) = timing else { continue };
        entries.push_str(&format!(
            "  {member}: {{ mode: {mode}, minUs: {min}, maxUs: {max}, \
             defaultApplied: {applied} }},\n",
            member = decl.name,
            mode = timing_mode(owner, &decl.name, timing.mode)?,
            min = micros_literal(owner, &decl.name, timing.min_us.as_deref())?,
            max = micros_literal(owner, &decl.name, timing.max_us.as_deref())?,
            applied = timing.default_applied
        ));
    }

    let const_name = format!("{}Timing", lower_camel(names.r#type));
    let export = export_kw(names.visibility);
    let doc = "\
/**
 * Resolved timing (ridl §9): `minUs` is the rate floor, `maxUs` the
 * staleness bound, both in microseconds — on a command or query, the call
 * throttle and the response bound (ADR-0015 decision 3). `defaultApplied`
 * marks a bound the compiler resolved from the configured default rather
 * than from source; an RPC bound is never defaulted and its entry is
 * absent when undeclared.
 */
";
    if entries.is_empty() {
        Ok(format!(
            "{doc}{export}const {const_name} = {{}} as const;\n"
        ))
    } else {
        Ok(format!(
            "{doc}{export}const {const_name} = {{\n{entries}}} as const;\n"
        ))
    }
}

fn timing_mode(interface: &str, member: &str, mode: i32) -> Result<&'static str, GenerateError> {
    match v2::TimingMode::try_from(mode).unwrap_or(v2::TimingMode::Unspecified) {
        v2::TimingMode::StrictPeriodic => Ok("'strict-periodic'"),
        v2::TimingMode::Range => Ok("'range'"),
        v2::TimingMode::Unspecified => Err(GenerateError::Unrepresentable(format!(
            "interface {interface}: interaction {member} carries a timing with no mode; \
             timing is resolved at compile time (ridl §9.1), so an unresolved mode has \
             no faithful generated form"
        ))),
    }
}

/// A bigint microsecond literal, or `undefined` for the absent side of an
/// explicit half-open range. A bound that is not a plain integer has no
/// bigint form — emitting it as a `number` would trade the IR's exactness
/// for a silent rounding, so it is refused instead.
fn micros_literal(
    interface: &str,
    member: &str,
    value: Option<&str>,
) -> Result<String, GenerateError> {
    match value {
        None => Ok("undefined".to_string()),
        Some(value) if is_integer_form(value) => Ok(format!("{value}n")),
        Some(value) => Err(GenerateError::Unrepresentable(format!(
            "interface {interface}: interaction {member} has timing bound {value:?}, which \
             is not an exact integer count of microseconds and has no bigint literal form"
        ))),
    }
}

// ---------------------------------------------------------------------------
// Contract data.
// ---------------------------------------------------------------------------

/// The require/ensure clauses of every interaction, as data (ridl §13) — the
/// E2.5 observer stubs, so a runtime builds its observers from this table
/// rather than by parsing the generated source. `source` is the canonical
/// expression text; E5.1 replaces it with a structured tree.
fn emit_contracts(names: Names, interface: &v2::Interface) -> Result<String, GenerateError> {
    let owner = names.identity;
    let mut entries = String::new();
    for decl in &interface.interactions {
        let contracts: &[v2::Contract] = match &decl.kind {
            Some(v2::decl::Kind::CommandDef(command)) => &command.contracts,
            Some(v2::decl::Kind::QueryDef(query)) => &query.contracts,
            _ => &[],
        };
        for contract in contracts {
            entries.push_str(&format!(
                "  {{ id: {id}, kind: {kind}, source: {source}, signals: [{signals}], \
                 params: [{params}], usesResult: {uses_result} }},\n",
                id = ts_string(&contract.observer_id),
                kind = contract_kind(owner, &decl.name, contract.kind)?,
                source = ts_string(&contract.source),
                signals = string_list(&contract.signal_refs),
                params = string_list(&contract.param_refs),
                uses_result = contract.uses_result
            ));
        }
    }

    let const_name = format!("{}Contracts", lower_camel(names.r#type));
    let export = export_kw(names.visibility);
    let doc = "\
/**
 * The require/ensure clauses of this interface, as data (ridl §13). `id` is
 * the observer-stub identity `<Interface>.<interaction>.<kind>[n]`; `source`
 * is the canonical expression text; `signals` and `params` name what the
 * expression reads; `usesResult` says whether the clause reads the query's
 * result, which an `ensure` observer must know before it can be scheduled —
 * it cannot run until the result exists. That flag is carried rather than
 * inferred: `source` is text, so matching on it would misread a parameter
 * named `resultCode` or a field access `.result`.
 */
";
    if entries.is_empty() {
        Ok(format!("{doc}{export}const {const_name} = [] as const;\n"))
    } else {
        Ok(format!(
            "{doc}{export}const {const_name} = [\n{entries}] as const;\n"
        ))
    }
}

fn contract_kind(interface: &str, member: &str, kind: i32) -> Result<&'static str, GenerateError> {
    match v2::ContractKind::try_from(kind).unwrap_or(v2::ContractKind::Unspecified) {
        v2::ContractKind::Require => Ok("'require'"),
        v2::ContractKind::Ensure => Ok("'ensure'"),
        v2::ContractKind::Unspecified => Err(GenerateError::Unrepresentable(format!(
            "interface {interface}: interaction {member} carries a contract with no kind; \
             a clause is require or ensure (ridl §13)"
        ))),
    }
}

fn string_list(values: &[String]) -> String {
    values
        .iter()
        .map(|value| ts_string(value))
        .collect::<Vec<_>>()
        .join(", ")
}

// ---------------------------------------------------------------------------
// Services.
// ---------------------------------------------------------------------------

/// The published services of the package, as data (ridl §14.5). The key is
/// the dotted global service name — the service's identity everywhere outside
/// this module — and `interfaces` names the generated TypeScript interfaces
/// behind it, one entry per composed shape in slot order (ADR-0015 decision
/// 12). A `reserved` slot holds a retired interface id and contributes no
/// entry.
fn emit_services(package: &v2::Package) -> Result<String, GenerateError> {
    let mut entries = String::new();
    for service in &package.services {
        let mut interfaces = Vec::new();
        for slot in &service.shapes {
            match &slot.kind {
                Some(v2::service_shape::Kind::InterfaceRef(reference)) => {
                    interfaces.push(ts_string(reference));
                }
                Some(v2::service_shape::Kind::Inline(_)) => {
                    interfaces.push(ts_string(&inline_interface_name(&service.name)));
                }
                Some(v2::service_shape::Kind::Reserved(_)) => {}
                None => {
                    return Err(GenerateError::Unrepresentable(format!(
                        "service {name}: shape slot {id} carries no kind; a slot is an \
                         interface reference, an inline shape, or a tombstone (ridl §14.5)",
                        name = service.name,
                        id = slot.id
                    )));
                }
            }
        }
        entries.push_str(&format!(
            "  {key}: {{ interfaces: [{interfaces}] }},\n",
            key = ts_string(&service.name),
            interfaces = interfaces.join(", ")
        ));
    }

    let doc = "\
/**
 * The services this package publishes (ridl §14.5). Each key is the dotted
 * global service name — the address a runtime resolves and the prefix of the
 * observer-stub ids — and `interfaces` names the generated TypeScript
 * interfaces behind it, in shape-list slot order. The two are deliberately
 * different spellings of different things: the dotted name is the service's
 * identity, while a generated name is a TypeScript identifier, which cannot
 * contain dots. The faces of an entry named `Foo` are `FooConsumer` and
 * `FooProvider`.
 */
";
    Ok(format!(
        "{doc}export const services = {{\n{entries}}} as const;\n"
    ))
}

/// The generated interface name of a service's inline shape: `Service_` plus
/// the dotted service name with dots replaced by underscores.
fn inline_interface_name(service: &str) -> String {
    format!("Service_{}", service.replace('.', "_"))
}

/// The note carried in an inline shape's generated faces, so the difference
/// between the generated type name and the service's dotted address is
/// stated where a reader meets it rather than left to be inferred.
fn inline_shape_note(service: &str, generated: &str) -> String {
    format!(
        "This is the inline shape of service `{service}` (ridl §14.5). The shape is \
         anonymous in source, so its generated name is `{generated}` — a TypeScript \
         identifier derived from the service name, because a TypeScript identifier \
         cannot contain dots. The service's own identity stays the dotted \
         `{service}`: that is what the `services` map is keyed by, what a runtime \
         resolves, and what observer-stub ids are prefixed with. The generated name \
         and the dotted address are deliberately different things; neither \
         substitutes for the other."
    )
}

// ---------------------------------------------------------------------------
// Names.
// ---------------------------------------------------------------------------

/// The column generated prose wraps at. A JSDoc line adds three columns of
/// ` * ` prefix, so the rendered lines stay inside 80.
const WRAP_COLUMN: usize = 74;

/// Wraps generated prose on word boundaries. Only the sentences this emitter
/// writes are wrapped — a doc body authored in source passes through
/// untouched, since its line structure is the author's. A word longer than
/// the column (a long type reference) is left whole rather than broken.
fn wrap(text: &str) -> String {
    let mut lines: Vec<String> = Vec::new();
    for paragraph in text.split('\n') {
        let mut line = String::new();
        for word in paragraph.split_whitespace() {
            if line.is_empty() {
                line.push_str(word);
            } else if line.chars().count() + 1 + word.chars().count() <= WRAP_COLUMN {
                line.push(' ');
                line.push_str(word);
            } else {
                lines.push(std::mem::take(&mut line));
                line.push_str(word);
            }
        }
        lines.push(line);
    }
    lines.join("\n")
}

/// The lower-camel form of a generated const's stem: the first character
/// lowercased, the rest untouched.
fn lower_camel(name: &str) -> String {
    let mut chars = name.chars();
    match chars.next() {
        Some(first) => first.to_lowercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

/// Every name this module generates, mapped to a description of where it came
/// from. The description is what an error message quotes, so it names the
/// construct an author would recognize.
///
/// The set is derived from the same [`Names`] values emission uses, so a name
/// cannot be reserved here and spelled differently there.
fn generated_names(package: &v2::Package) -> Result<BTreeMap<String, String>, GenerateError> {
    let mut names: BTreeMap<String, String> = BTreeMap::new();

    // Claims `name` for `origin`, or reports the owner that holds it already.
    // Two generated names colliding would emit a module with a duplicated
    // declaration, so it is caught here rather than left to `tsc`.
    let claim = |names: &mut BTreeMap<String, String>, name: String, origin: String| {
        if let Some(previous) = names.get(&name) {
            return Err(GenerateError::Unrepresentable(format!(
                "the generated name {name} is claimed by both {previous} and {origin}"
            )));
        }
        names.insert(name, origin);
        Ok(())
    };

    for vocabulary in VOCABULARY_NAMES {
        claim(
            &mut names,
            vocabulary.to_string(),
            "the generated interaction vocabulary".to_string(),
        )?;
    }
    if !package.services.is_empty() {
        claim(
            &mut names,
            "services".to_string(),
            "the generated service map".to_string(),
        )?;
    }

    // Every interface, then every inline service shape — the same walk, the
    // same order and the same type names emission uses.
    let owners = package.shapes().map(|shape| match shape.service {
        Some(service) => (
            inline_interface_name(&service.name),
            format!("the inline shape of service {}", service.name),
        ),
        None => (
            shape.name.to_string(),
            format!("interface {name}", name = shape.name),
        ),
    });

    for (type_name, origin) in owners {
        let stem = lower_camel(&type_name);
        for (name, what) in [
            (format!("{type_name}Consumer"), "the consumer face"),
            (format!("{type_name}Provider"), "the provider face"),
            (format!("{stem}Timing"), "the timing table"),
            (format!("{stem}Contracts"), "the contract table"),
        ] {
            claim(&mut names, name, format!("{what} of {origin}"))?;
        }
    }
    Ok(names)
}

/// Rejects a package whose typl declarations would collide with a generated
/// interaction-layer name. TypeScript has one module namespace, so a
/// collision emits a module that does not compile; naming it here keeps the
/// backend honest rather than deferring the failure to `tsc`.
///
/// The check covers **every** generated name — the vocabulary, the service
/// map, and, per interface and per inline service shape, both faces and both
/// const stems. Covering only some of them would be unsound, because nothing
/// upstream keeps a typl name clear of the generated shapes: typl §15.1 is a
/// conventions table with no enforcing diagnostic, so `struct
/// vehicleStatusTiming` and `struct Service_veh_adas_logsConsumer` both
/// compile without a single ridl diagnostic. Left unchecked they reach `tsc`
/// as TS2451 (redeclared block-scoped variable) and TS2741 (two merged
/// `export interface` declarations), which is exactly the deferral this
/// function exists to prevent.
///
/// A collision between two generated names is caught as well — an interface
/// and an inline service shape can arrive at the same generated name — since
/// nothing rules that out either.
fn check_name_collisions(package: &v2::Package) -> Result<(), GenerateError> {
    // Building the set detects generated-vs-generated collisions on its own.
    let generated = generated_names(package)?;

    // Declaration-vs-generated, in declaration order so the first collision
    // in the source is the one reported.
    for decl in &package.decls {
        if let Some(origin) = generated.get(&decl.name) {
            return Err(GenerateError::Unrepresentable(format!(
                "declaration {name} collides with {origin}; a module carrying \
                 interactions reserves that name",
                name = decl.name
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
