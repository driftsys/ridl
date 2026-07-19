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
//!   than one type nobody can implement. Commands, queries, and finals keep
//!   one shape on both sides.
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

use crate::{Ctx, GenerateError, deprecated_tags, is_integer_form, jsdoc, kind_ts, ts_string};
use ridl_ir::v2;

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

    for interface in &package.interfaces {
        emit_interface(ctx, &interface.name, interface, None, &mut blocks)?;
    }
    // An inline service shape carries no name of its own (ridl §14.5); its
    // generated interface is named after the service, and the note saying so
    // rides in the faces' own docs rather than as a detached comment.
    for service in &package.services {
        if let Some(v2::service::Shape::Inline(inline)) = &service.shape {
            let name = inline_interface_name(&service.name);
            let note = inline_shape_note(&service.name, &name);
            emit_interface(ctx, &name, inline, Some(&note), &mut blocks)?;
        }
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
    name: &str,
    interface: &v2::Interface,
    note: Option<&str>,
    blocks: &mut Vec<String>,
) -> Result<(), GenerateError> {
    blocks.push(emit_face(ctx, name, interface, note, Face::Consumer)?);
    blocks.push(emit_face(ctx, name, interface, note, Face::Provider)?);
    blocks.push(emit_timing(name, interface)?);
    blocks.push(emit_contracts(name, interface)?);
    Ok(())
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
                 called, finals are read."
            }
            Face::Provider => {
                "The provider face of this interface (ridl §14): what code \
                 realizing it implements. Signals and events are published, \
                 commands and queries are handled, finals are supplied at \
                 binding initialization."
            }
        }
    }
}

fn emit_face(
    ctx: &Ctx,
    name: &str,
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
    let doc = jsdoc(
        "",
        &paragraphs.join("\n\n"),
        &deprecated_tags(interface.deprecated.as_deref()),
    );

    let mut members = String::new();
    for decl in &interface.interactions {
        members.push_str(&emit_member(ctx, &interface.name, decl, face)?);
    }

    let face_name = format!("{name}{}", face.suffix());
    if members.is_empty() {
        Ok(format!("{doc}export interface {face_name} {{}}\n"))
    } else {
        Ok(format!(
            "{doc}export interface {face_name} {{\n{members}}}\n"
        ))
    }
}

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
            let mut tags = signal_tags(signal, face);
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
            let doc = jsdoc(
                "  ",
                &decl.doc,
                &deprecated_tags(decl.deprecated.as_deref()),
            );
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
            let mut tags = vec![transport_tag()];
            tags.extend(deprecated_tags(decl.deprecated.as_deref()));
            let doc = jsdoc("  ", &body, &tags);
            Ok(format!("{doc}  {name}({params}): Promise<void>;\n"))
        }
        Some(v2::decl::Kind::QueryDef(query)) => {
            let params = params_ts(ctx, &query.params)?;
            let ret = return_ts(ctx, query.return_type.as_ref())?;
            let mut tags = Vec::new();
            if let Some(identity) = transport_identity(interface, decl.ordinal, query) {
                tags.push(identity);
            }
            tags.push(transport_tag());
            tags.extend(deprecated_tags(decl.deprecated.as_deref()));
            let doc = jsdoc("  ", &decl.doc, &tags);
            Ok(format!("{doc}  {name}({params}): {ret};\n"))
        }
        Some(v2::decl::Kind::FinalDef(final_def)) => {
            // A final is provisioned per software instance and immutable
            // within it (ridl §8), so it is a plain readonly property on both
            // faces — there is nothing to subscribe to and nothing to publish.
            let payload = match final_def.payload.as_ref() {
                Some(ft) => kind_ts(ctx, ft.kind.as_ref())?,
                None => "unknown".to_string(),
            };
            let doc = jsdoc(
                "  ",
                &decl.doc,
                &deprecated_tags(decl.deprecated.as_deref()),
            );
            Ok(format!("{doc}  readonly {name}: {payload};\n"))
        }
        // A reserved tombstone occupies an ordinal but emits no member
        // (ridl §11).
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

/// The tags of a signal. `@init` is the resolved channel init (ridl §4.4) and
/// belongs to both faces — it is what the channel holds before the provider
/// publishes. `@provenance` is a consumer's concern only: a provider
/// publishes values, it does not read them back with a provenance.
fn signal_tags(signal: &v2::SignalDef, face: Face) -> Vec<String> {
    let mut tags = Vec::new();
    if let Some(value) = signal.init.as_ref().and_then(|i| i.value.as_deref()) {
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
/// numeric form that holds them without rounding.
fn emit_timing(name: &str, interface: &v2::Interface) -> Result<String, GenerateError> {
    let mut entries = String::new();
    for decl in &interface.interactions {
        let timing = match &decl.kind {
            Some(v2::decl::Kind::SignalDef(signal)) => signal.timing.as_ref(),
            Some(v2::decl::Kind::EventDef(event)) => event.timing.as_ref(),
            _ => None,
        };
        let Some(timing) = timing else { continue };
        entries.push_str(&format!(
            "  {member}: {{ mode: {mode}, minUs: {min}, maxUs: {max}, \
             defaultApplied: {applied} }},\n",
            member = decl.name,
            mode = timing_mode(name, &decl.name, timing.mode)?,
            min = micros_literal(name, &decl.name, timing.min_us.as_deref())?,
            max = micros_literal(name, &decl.name, timing.max_us.as_deref())?,
            applied = timing.default_applied
        ));
    }

    let const_name = format!("{}Timing", lower_camel(name));
    let doc = "\
/**
 * Resolved timing (ridl §9): `minUs` is the rate floor, `maxUs` the
 * staleness bound, both in microseconds. `defaultApplied` marks a bound the
 * compiler resolved from the configured default rather than from source —
 * \"untimed\" does not exist beyond the parser (ridl §9.1).
 */
";
    if entries.is_empty() {
        Ok(format!("{doc}export const {const_name} = {{}} as const;\n"))
    } else {
        Ok(format!(
            "{doc}export const {const_name} = {{\n{entries}}} as const;\n"
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
fn emit_contracts(name: &str, interface: &v2::Interface) -> Result<String, GenerateError> {
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
                 params: [{params}] }},\n",
                id = ts_string(&contract.observer_id),
                kind = contract_kind(name, &decl.name, contract.kind)?,
                source = ts_string(&contract.source),
                signals = string_list(&contract.signal_refs),
                params = string_list(&contract.param_refs)
            ));
        }
    }

    let const_name = format!("{}Contracts", lower_camel(name));
    let doc = "\
/**
 * The require/ensure clauses of this interface, as data (ridl §13). `id` is
 * the observer-stub identity `<Interface>.<interaction>.<kind>[n]`; `source`
 * is the canonical expression text; `signals` and `params` name what the
 * expression reads.
 */
";
    if entries.is_empty() {
        Ok(format!("{doc}export const {const_name} = [] as const;\n"))
    } else {
        Ok(format!(
            "{doc}export const {const_name} = [\n{entries}] as const;\n"
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
/// this module — and `interface` names the generated TypeScript interface
/// behind it.
fn emit_services(package: &v2::Package) -> Result<String, GenerateError> {
    let mut entries = String::new();
    for service in &package.services {
        let interface = match &service.shape {
            Some(v2::service::Shape::InterfaceRef(reference)) => reference.clone(),
            Some(v2::service::Shape::Inline(_)) => inline_interface_name(&service.name),
            None => {
                return Err(GenerateError::Unrepresentable(format!(
                    "service {name}: no shape; a service publishes an interface reference \
                     or an inline shape (ridl §14.5)",
                    name = service.name
                )));
            }
        };
        entries.push_str(&format!(
            "  {key}: {{ interface: {interface} }},\n",
            key = ts_string(&service.name),
            interface = ts_string(&interface)
        ));
    }

    let doc = "\
/**
 * The services this package publishes (ridl §14.5). Each key is the dotted
 * global service name — the address a runtime resolves and the prefix of the
 * observer-stub ids — and `interface` names the generated TypeScript
 * interface behind it. The two are deliberately different spellings of
 * different things: the dotted name is the service's identity, while the
 * generated name is a TypeScript identifier, which cannot contain dots. The
 * faces of an entry named `Foo` are `FooConsumer` and `FooProvider`.
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

/// Rejects a package whose typl declarations would collide with a generated
/// interaction-layer name. TypeScript has one module namespace, so a
/// collision emits a module that does not compile; naming it here keeps the
/// backend honest rather than deferring the failure to `tsc`.
fn check_name_collisions(package: &v2::Package) -> Result<(), GenerateError> {
    for decl in &package.decls {
        if VOCABULARY_NAMES.contains(&decl.name.as_str()) {
            return Err(GenerateError::Unrepresentable(format!(
                "declaration {name} collides with the generated interaction vocabulary; \
                 a module carrying interactions reserves {names:?}",
                name = decl.name,
                names = VOCABULARY_NAMES
            )));
        }
        for interface in &package.interfaces {
            for suffix in ["Consumer", "Provider"] {
                if decl.name == format!("{}{suffix}", interface.name) {
                    return Err(GenerateError::Unrepresentable(format!(
                        "declaration {name} collides with the generated {suffix} face of \
                         interface {interface}",
                        name = decl.name,
                        interface = interface.name
                    )));
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
