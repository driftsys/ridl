//! Hover content (docs/ROADMAP.md epic E1.15b, E2.10b; ADR-0004 §10).
//!
//! Hover on a type reference or a declaration renders the declaration's IR —
//! qualified name, kind, backing and canonical UCUM unit, constraint, derived
//! wire width, init value, doc comment, labels, and deprecation — pulled from
//! the [`CheckedPackage`](ridl_sem::CheckedPackage) IR, the richest source,
//! keyed by the symbol [`symbol_at`] resolves. Hover on a
//! struct field instead shows the field's derived ordinal (general form §6.3
//! groundwork; the ordinal is typl §7.4), read straight from the IR so it counts
//! reserved tombstones exactly as codegen does.
//!
//! The ridl layer (E2.10b) adds three more anchors, all read from the same
//! checked IR so the editor can never disagree with codegen:
//!
//! - **An interaction** renders its kind, resolved payload, §11 ordinal, and —
//!   for a signal or an event — its *resolved* timing: mode, bounds, and the
//!   per-kind reading general form §6.2 derives from the declaring keyword
//!   rather than from the annotation. A state value that arrives late is
//!   refreshed and a fast one debounced; a stale occurrence is discarded and a
//!   fast one throttled. When the interaction carried no annotation the
//!   configured default was resolved at compile time, and the hover says so —
//!   "untimed" does not exist past the parser (ridl §9.1).
//! - **A fallible return** (`T | E`, general form §6.1) names both arms and
//!   closes with the ridl §10 strata note. Stratum 3 is never called undefined
//!   behavior: the runtime detects those failures, the contract language merely
//!   does not declare them (general form §6.4).
//! - **A service** names its interface shape — or reports its inline one — and
//!   states the ridl §14.5 posture neutrality, because a service declaration
//!   says nothing about how it is realized on the wire.

use ridl_core::db::InputFile;
use ridl_core::package::{Package, Workspace, package_of};
use ridl_ir::v2;
use ridl_sem::{Symbol, SymbolKind, check_package};
use ridl_syntax::ast::{AstNode, HasName, InterfaceDef, InterfaceMember, ServiceDef, StructDef};
use ridl_syntax::{SyntaxKind, SyntaxNode};
use rowan::{TextRange, TextSize};

use crate::nav::{self, symbol_at};

/// Rendered hover content plus the source range it describes.
#[derive(Debug, Clone)]
pub struct HoverInfo {
    /// CommonMark markdown for the LSP hover popup.
    pub markdown: String,
    /// The reference or name span the hover is anchored to.
    pub range: TextRange,
}

/// Builds the hover for the cursor at `offset` in `file` (a file of `pkg`), or
/// `None` when the cursor is not on something with hover content.
pub fn hover(
    db: &dyn salsa::Database,
    ws: Workspace,
    std: Package,
    pkg: Package,
    file: InputFile,
    offset: TextSize,
) -> Option<HoverInfo> {
    // A struct field shows its ordinal, not a symbol — resolve that first.
    if let Some(info) = field_hover(db, ws, std, pkg, file, offset) {
        return Some(info);
    }
    // Interaction and service anchors are not symbols either: an interaction
    // name lives inside an interface body and a service name in the workspace
    // catalog, so `symbol_at` would return `None` for both.
    if let Some(info) = interaction_hover(db, ws, std, pkg, file, offset) {
        return Some(info);
    }
    if let Some(info) = service_hover(db, ws, pkg, std, file, offset) {
        return Some(info);
    }

    let located = symbol_at(db, ws, std, pkg, file, offset)?;
    let markdown = symbol_markdown(db, ws, std, pkg, &located.symbol);
    Some(HoverInfo {
        markdown,
        range: located.reference,
    })
}

/// The hover for a struct field: `field \`name\` — ordinal \`#N\``, with the
/// ordinal read from the lowered IR (which counts reserved tombstones). Returns
/// `None` when the cursor is not on a struct field's name.
fn field_hover(
    db: &dyn salsa::Database,
    ws: Workspace,
    std: Package,
    pkg: Package,
    file: InputFile,
    offset: TextSize,
) -> Option<HoverInfo> {
    let source = nav::source_file(db, file);
    let token = nav::identifier_at(source.syntax(), offset)?;
    let name_node = token.parent()?;
    if name_node.kind() != SyntaxKind::Name {
        return None;
    }
    let field_node = name_node.parent()?;
    if field_node.kind() != SyntaxKind::FieldDef {
        return None;
    }
    let field_name = token.text().to_string();
    let struct_node = field_node
        .ancestors()
        .find(|node| node.kind() == SyntaxKind::StructDef)?;
    let struct_name = StructDef::cast(struct_node)?
        .name()?
        .ident_token()?
        .text()
        .to_string();

    let ir = &check_package(db, ws, pkg, std).ir;
    let ordinal = field_ordinal(ir, &struct_name, &field_name)?;
    Some(HoverInfo {
        markdown: format!("field `{field_name}` — ordinal `#{ordinal}`"),
        range: name_node.text_range(),
    })
}

/// The 1-based ordinal of field `field_name` in struct `struct_name`, from the
/// lowered IR. Shared with the inlay-hint pass (E1.16), which renders the same
/// ordinal beside every field.
pub(crate) fn field_ordinal(ir: &v2::Package, struct_name: &str, field_name: &str) -> Option<u32> {
    let decl = ir.decls.iter().find(|decl| decl.name == struct_name)?;
    let Some(v2::decl::Kind::StructDef(struct_def)) = &decl.kind else {
        return None;
    };
    struct_def
        .members
        .iter()
        .find_map(|member| match &member.member {
            Some(v2::struct_member::Member::Field(field)) if field.name == field_name => {
                Some(field.ordinal)
            }
            _ => None,
        })
}

/// The hover markdown for a resolved symbol: the declaration's IR rendering when
/// the symbol lowered, or a minimal name-and-kind line as a fallback.
///
/// `pkg` is the package the cursor was in; it is preferred when its name matches
/// so a symbol declared in a standalone overlay (which `package_of` cannot find)
/// still renders its full IR — mirroring the checker's own `package_handle`.
fn symbol_markdown(
    db: &dyn salsa::Database,
    ws: Workspace,
    std: Package,
    pkg: Package,
    symbol: &Symbol,
) -> String {
    let qualified = format!("{}.{}", symbol.package, symbol.name);
    let target = if symbol.package == *pkg.name(db) {
        Some(pkg)
    } else if symbol.package == *std.name(db) {
        Some(std)
    } else {
        package_of(db, ws, symbol.package.clone())
    };
    if let Some(target) = target {
        let ir = check_package(db, ws, target, std).ir;
        if let Some(decl) = ir.decls.iter().find(|decl| decl.name == symbol.name) {
            return render_decl(&qualified, decl);
        }
    }
    format!("**`{qualified}`** — {}", symbol_kind(symbol.kind))
}

/// Renders one IR declaration as a hover markdown block: a fenced typl
/// signature line, then the derived width, doc comment, labels, and deprecation.
fn render_decl(qualified: &str, decl: &v2::Decl) -> String {
    let mut lines = String::new();
    lines.push_str("```typl\n");
    lines.push_str(&signature(qualified, decl));
    lines.push_str("\n```");

    if let Some(v2::decl::Kind::TypeDef(type_def)) = &decl.kind
        && let Some(width) = type_def.width.as_ref().map(width_name)
    {
        lines.push_str(&format!("\n\n**Width:** `{width}`"));
    }
    if !decl.doc.is_empty() {
        lines.push_str("\n\n");
        lines.push_str(&decl.doc);
    }
    if !decl.labels.is_empty() {
        lines.push_str(&format!("\n\n**Labels:** {}", decl.labels.join(", ")));
    }
    if let Some(reason) = &decl.deprecated {
        lines.push_str(&format!("\n\n**Deprecated:** {reason}"));
    }
    lines
}

/// The one-line typl signature of a declaration.
fn signature(qualified: &str, decl: &v2::Decl) -> String {
    let modifiers = declaration_modifiers(decl);
    match &decl.kind {
        Some(v2::decl::Kind::TypeDef(type_def)) => {
            format!(
                "{modifiers}type {qualified}{}{}{}",
                backing(type_def.backing.as_ref()),
                constraint(type_def.constraint.as_ref()),
                init(type_def.init.as_ref(), type_def.declared_init.as_ref()),
            )
        }
        Some(v2::decl::Kind::ConstDef(const_def)) => {
            if let Some(regex) = &const_def.regex {
                format!("{modifiers}const {qualified} = {regex}")
            } else {
                let type_ref = const_def
                    .type_ref
                    .as_deref()
                    .map(|name| format!(" : {name}"))
                    .unwrap_or_default();
                format!(
                    "{modifiers}const {qualified}{type_ref} = {}",
                    const_def.value
                )
            }
        }
        Some(v2::decl::Kind::StructDef(_)) => format!("{modifiers}struct {qualified}"),
        Some(v2::decl::Kind::EnumDef(_)) => format!("{modifiers}enum {qualified}"),
        Some(v2::decl::Kind::EnumSetDef(_)) => format!("{modifiers}enumset {qualified}"),
        Some(v2::decl::Kind::UnionDef(_)) => format!("{modifiers}union {qualified}"),
        // Interaction kinds ride `Interface.interactions`, never a package
        // decl; interaction hovers land with the E2 LSP tasks.
        Some(_) | None => qualified.to_string(),
    }
}

/// The `internal` / `error` modifier prefix (with a trailing space) for a
/// declaration's signature.
fn declaration_modifiers(decl: &v2::Decl) -> String {
    let mut prefix = String::new();
    if decl.visibility == v2::Visibility::Internal as i32 {
        prefix.push_str("internal ");
    }
    if decl.is_error {
        prefix.push_str("error ");
    }
    prefix
}

/// The backing clause of a type (`: km/h`, `: integer`), or the empty string
/// when the backing is missing.
fn backing(backing: Option<&v2::Backing>) -> String {
    match backing.and_then(|backing| backing.kind.as_ref()) {
        Some(v2::backing::Kind::Unit(unit)) => format!(" : {unit}"),
        Some(v2::backing::Kind::Primitive(primitive)) => {
            format!(" : {}", primitive_name(*primitive))
        }
        None => String::new(),
    }
}

/// The constraint clause of a type (`[0.0..250.0 step 0.5]`, `[0..256]`,
/// `[..100]`, `match /.../`), or the empty string when there is no constraint.
fn constraint(constraint: Option<&v2::Constraint>) -> String {
    let Some(constraint) = constraint else {
        return String::new();
    };
    // An open-ended range lowers with one bound absent (`[..100]`, `[0..]`);
    // render the present side and leave the other empty (ADR-0004 §10).
    if constraint.min.is_some() || constraint.max.is_some() {
        let min = constraint.min.as_deref().unwrap_or("");
        let max = constraint.max.as_deref().unwrap_or("");
        let step = constraint
            .step
            .as_deref()
            .map(|step| format!(" step {step}"))
            .unwrap_or_default();
        return format!(" [{min}..{max}{step}]");
    }
    if constraint.len_min.is_some() || constraint.len_max.is_some() {
        let min = constraint.len_min.unwrap_or(0);
        let max = constraint.len_max.unwrap_or(0);
        return format!(" [{min}..{max}]");
    }
    if let Some(pattern) = &constraint.pattern {
        return format!(" match {pattern}");
    }
    if let Some(pattern_const) = &constraint.pattern_const {
        return format!(" match {pattern_const}");
    }
    String::new()
}

/// The init clause of a type (`= 0.0`): the declared init when present,
/// otherwise the resolved derived value, otherwise the empty string.
fn init(init: Option<&v2::InitValue>, declared: Option<&String>) -> String {
    if let Some(declared) = declared {
        return format!(" = {declared}");
    }
    match init.and_then(|init| init.value.as_ref()) {
        Some(value) => format!(" = {value}"),
        None => String::new(),
    }
}

/// The display name of a derived wire width.
fn width_name(width: &v2::type_def::Width) -> &'static str {
    match width {
        v2::type_def::Width::IntWidth(int_width) => int_width_name(*int_width),
        v2::type_def::Width::FloatWidth(float_width) => float_width_name(*float_width),
    }
}

/// The lowercase display name of a primitive type.
fn primitive_name(primitive: i32) -> &'static str {
    match v2::PrimitiveType::try_from(primitive) {
        Ok(v2::PrimitiveType::Boolean) => "boolean",
        Ok(v2::PrimitiveType::Integer) => "integer",
        Ok(v2::PrimitiveType::Float) => "float",
        Ok(v2::PrimitiveType::String) => "string",
        Ok(v2::PrimitiveType::Bytes) => "bytes",
        _ => "?",
    }
}

/// The lowercase display name of an integer wire width.
fn int_width_name(width: i32) -> &'static str {
    match v2::IntWidth::try_from(width) {
        Ok(v2::IntWidth::U8) => "u8",
        Ok(v2::IntWidth::I8) => "i8",
        Ok(v2::IntWidth::U16) => "u16",
        Ok(v2::IntWidth::I16) => "i16",
        Ok(v2::IntWidth::U32) => "u32",
        Ok(v2::IntWidth::I32) => "i32",
        Ok(v2::IntWidth::U64) => "u64",
        Ok(v2::IntWidth::I64) => "i64",
        _ => "?",
    }
}

/// The lowercase display name of a float wire width.
fn float_width_name(width: i32) -> &'static str {
    match v2::FloatWidth::try_from(width) {
        Ok(v2::FloatWidth::F32) => "f32",
        Ok(v2::FloatWidth::F64) => "f64",
        _ => "?",
    }
}

/// The one-word kind label of a resolver symbol — the fallback used when the
/// symbol did not lower to IR.
fn symbol_kind(kind: SymbolKind) -> &'static str {
    match kind {
        SymbolKind::Type => "type",
        SymbolKind::Const => "const",
        SymbolKind::Struct => "struct",
        SymbolKind::Enum => "enum",
        SymbolKind::EnumSet => "enumset",
        SymbolKind::Union => "union",
        SymbolKind::Interface => "interface",
    }
}

// --- the ridl interaction layer (E2.10b) ---------------------------------

/// The general form §6.4 wording for Stratum 3. Stratum 3 is fully detected by
/// the runtime — acks, timeouts, staleness — and merely absent from the
/// contract language; calling it undefined behavior would say the opposite of
/// what the design guarantees, so this sentence is normative and quoted
/// verbatim.
const STRATUM_THREE: &str = "infrastructure failure — detected, undeclared";

/// The ridl §14.5 note every service hover closes with.
const POSTURE_NOTE: &str = "Posture-neutral by design: this declaration says nothing about how the \
    contract is realized on the wire. rsdl and deployment choose the posture — static (its \
    signals and events packed into bus frames) or discovered (SOME/IP, DDS, uProtocol) — from \
    the same declaration (ridl §14.5).";

/// The hover for an interaction: the cursor on an interaction's name, or on the
/// `|` of its inline fallible return.
///
/// The rendering is driven entirely by the interaction's lowered `Decl`, found
/// by name inside the enclosing shape's IR, so the ordinal and the timing are
/// the ones codegen and `ridl diff` see — never re-derived from the tree.
fn interaction_hover(
    db: &dyn salsa::Database,
    ws: Workspace,
    std: Package,
    pkg: Package,
    file: InputFile,
    offset: TextSize,
) -> Option<HoverInfo> {
    let source = nav::source_file(db, file);
    let token = nav::identifier_at(source.syntax(), offset)?;
    let anchor = token.parent()?;
    // Two anchors reach the same rendering: the interaction's own name, and
    // the `T | E` of a fallible return (whose arms are part of that rendering).
    let (member, range) = match anchor.kind() {
        SyntaxKind::Name => {
            let member = InterfaceMember::cast(anchor.parent()?)?;
            (member, anchor.text_range())
        }
        SyntaxKind::FallibleType => {
            let member = anchor.ancestors().find_map(InterfaceMember::cast)?;
            (member, anchor.text_range())
        }
        _ => return None,
    };
    // A tombstone declares nothing to render; its ordinal rides the inlay hint.
    if matches!(member, InterfaceMember::Reserved(_)) {
        return None;
    }
    let name = member.name()?.ident_token()?.text().to_string();

    let ir = &check_package(db, ws, pkg, std).ir;
    let (owner, shape) = enclosing_shape(ir, member.syntax())?;
    let decl = shape.interactions.iter().find(|decl| {
        decl.name == name && !matches!(decl.kind, Some(v2::decl::Kind::ReservedSlot(_)))
    })?;

    Some(HoverInfo {
        markdown: render_interaction(db, ws, std, pkg, &owner, decl),
        range,
    })
}

/// The hover for a service declaration: the cursor on its dotted global name.
fn service_hover(
    db: &dyn salsa::Database,
    ws: Workspace,
    pkg: Package,
    std: Package,
    file: InputFile,
    offset: TextSize,
) -> Option<HoverInfo> {
    let source = nav::source_file(db, file);
    let token = nav::identifier_at(source.syntax(), offset)?;
    let dotted = token.parent()?;
    if dotted.kind() != SyntaxKind::DottedName {
        return None;
    }
    let service_node = dotted.parent()?;
    if service_node.kind() != SyntaxKind::ServiceDef {
        return None;
    }
    let name = non_empty(ServiceDef::cast(service_node)?.name()?.text())?;

    let ir = &check_package(db, ws, pkg, std).ir;
    let service = ir.services.iter().find(|service| service.name == name)?;
    Some(HoverInfo {
        markdown: render_service(service),
        range: dotted.text_range(),
    })
}

/// A dotted name's text, or `None` when the parser recovered the declaration
/// with no tokens at all. The text itself is [`DottedName::text`] — the key
/// `v2::Service.name` carries, so it is what the IR lookup matches on.
fn non_empty(text: String) -> Option<String> {
    (!text.is_empty()).then_some(text)
}

/// The interface shape a node sits inside, with the name to display for it: an
/// `interface` declaration's own name, or the dotted name of the service whose
/// inline shape holds the node (ridl §14.5). `None` when the enclosing shape
/// did not lower — a file that failed to parse past the header, most often.
fn enclosing_shape<'a>(
    ir: &'a v2::Package,
    node: &SyntaxNode,
) -> Option<(String, &'a v2::Interface)> {
    for ancestor in node.ancestors() {
        let name = match ancestor.kind() {
            SyntaxKind::InterfaceDef => InterfaceDef::cast(ancestor)?
                .name()?
                .ident_token()?
                .text()
                .to_string(),
            SyntaxKind::ServiceDef => non_empty(ServiceDef::cast(ancestor)?.name()?.text())?,
            _ => continue,
        };
        // One lookup for both: `Package::shapes` keys a named interface on its
        // own name and a service's inline shape on the dotted service name, so
        // a shape stored outside `Package.interfaces` is still found.
        let shape = ir.shapes().find(|shape| shape.name == name)?;
        return Some((name, shape.interface));
    }
    None
}

/// Renders one interaction: the signature, the §11 ordinal, the payload with
/// its typl detail, the resolved timing with its per-kind reading, the error
/// strata note for a fallible return, and the doc envelope.
fn render_interaction(
    db: &dyn salsa::Database,
    ws: Workspace,
    std: Package,
    pkg: Package,
    owner: &str,
    decl: &v2::Decl,
) -> String {
    let mut out = String::new();
    out.push_str("```ridl\n");
    out.push_str(&interaction_signature(owner, decl));
    out.push_str("\n```");
    out.push_str(&format!(
        "\n\n**Ordinal:** `#{}` — declaration order is wire identity (ridl §11)",
        decl.ordinal
    ));

    match &decl.kind {
        Some(v2::decl::Kind::SignalDef(signal)) => {
            out.push_str(&payload_line(db, ws, std, pkg, &signal.payload));
            if let Some(timing) = &signal.timing {
                out.push_str(&timing_line(timing, Reading::State));
            }
        }
        Some(v2::decl::Kind::EventDef(event)) => {
            out.push_str(&payload_line(db, ws, std, pkg, &event.payload));
            if let Some(timing) = &event.timing {
                out.push_str(&timing_line(timing, Reading::Occurrence));
            }
        }
        Some(v2::decl::Kind::FinalDef(final_def)) => {
            if let Some(named) = final_def.payload.as_ref().and_then(named_ref) {
                out.push_str(&payload_line(db, ws, std, pkg, named));
            }
        }
        Some(v2::decl::Kind::QueryDef(query)) => {
            if let Some(fallible) = query.return_type.as_ref().and_then(fallible_of) {
                out.push_str(&strata_note(fallible));
            }
        }
        _ => {}
    }

    if !decl.doc.is_empty() {
        out.push_str("\n\n");
        out.push_str(&decl.doc);
    }
    if !decl.labels.is_empty() {
        out.push_str(&format!("\n\n**Labels:** {}", decl.labels.join(", ")));
    }
    if let Some(reason) = &decl.deprecated {
        out.push_str(&format!("\n\n**Deprecated:** {reason}"));
    }
    out
}

/// The one-line ridl signature of an interaction, with every reference in the
/// canonical form the IR stores — never an import alias (ridl §14.1).
fn interaction_signature(owner: &str, decl: &v2::Decl) -> String {
    let name = format!("{owner}.{}", decl.name);
    match &decl.kind {
        Some(v2::decl::Kind::SignalDef(signal)) => format!(
            "signal {name} : {}{}{}",
            signal.payload,
            signal
                .declared_init
                .as_ref()
                .map(|init| format!(" = {init}"))
                .unwrap_or_default(),
            timing_suffix(signal.timing.as_ref()),
        ),
        Some(v2::decl::Kind::EventDef(event)) => format!(
            "event {name} : {}{}",
            event.payload,
            timing_suffix(event.timing.as_ref()),
        ),
        Some(v2::decl::Kind::CommandDef(command)) => {
            format!("command {name}({})", params(&command.params))
        }
        Some(v2::decl::Kind::QueryDef(query)) => format!(
            "query {name}({}): {}",
            params(&query.params),
            query
                .return_type
                .as_ref()
                .map(return_text)
                .unwrap_or_else(|| "?".to_string()),
        ),
        Some(v2::decl::Kind::FinalDef(final_def)) => format!(
            "final {name} : {}",
            final_def
                .payload
                .as_ref()
                .map(field_type_text)
                .unwrap_or_else(|| "?".to_string()),
        ),
        _ => name,
    }
}

/// The `**Payload:**` line for a named payload reference, with the referenced
/// declaration's own typl detail — its unit and constraint for a scalar type,
/// its shape word for a composite. A payload that does not resolve contributes
/// the reference alone rather than a wrong reading.
fn payload_line(
    db: &dyn salsa::Database,
    ws: Workspace,
    std: Package,
    pkg: Package,
    canonical: &str,
) -> String {
    match decl_of_ref(db, ws, std, pkg, canonical)
        .as_ref()
        .and_then(type_summary)
    {
        Some(summary) => format!("\n\n**Payload:** `{canonical}` — {summary}"),
        None => format!("\n\n**Payload:** `{canonical}`"),
    }
}

/// Which per-kind reading general form §6.2 derives for an interaction: a
/// signal carries state, an event carries occurrences.
#[derive(Clone, Copy)]
enum Reading {
    State,
    Occurrence,
}

impl Reading {
    /// The derived consequence of the generic bounds. The annotation itself
    /// means one thing everywhere — `min` is a rate floor, `max` a staleness
    /// bound — and only the declaring keyword decides what happens at each
    /// edge (general form §6.2).
    fn text(self) -> &'static str {
        match self {
            Self::State => "min = rate floor (debounce), max = staleness bound (refresh ceiling)",
            Self::Occurrence => {
                "min = rate floor (throttle), max = staleness bound \
                 (TTL: stale occurrences discarded)"
            }
        }
    }
}

/// The `**Timing:**` line: the resolved mode, the resolved bounds, the note
/// that the configured default was applied when the source carried no
/// annotation, and the derived per-kind reading.
fn timing_line(timing: &v2::Timing, reading: Reading) -> String {
    let mode = match v2::TimingMode::try_from(timing.mode) {
        Ok(v2::TimingMode::StrictPeriodic) => "strict periodic",
        _ => "range",
    };
    let bounds = bounds_text(timing);
    let default = if timing.default_applied {
        format!(" (default {bounds} applied)")
    } else {
        String::new()
    };
    format!(
        "\n\n**Timing:** {mode} `{bounds}`{default} — {}",
        reading.text()
    )
}

/// The `@…` suffix of an interaction signature, for the resolved timing.
fn timing_suffix(timing: Option<&v2::Timing>) -> String {
    timing
        .map(|timing| format!(" @{}", bounds_text(timing)))
        .unwrap_or_default()
}

/// The duration units of ridl §2.1, coarsest first, with their microsecond
/// scale.
const DURATION_UNITS: &[(u128, &str)] = &[
    (3_600_000_000, "h"),
    (60_000_000, "min"),
    (1_000_000, "s"),
    (1_000, "ms"),
];

/// The resolved bounds in source form: the single period of a strict-periodic
/// annotation, or the `[min..max]` range with an absent side left empty.
///
/// Both bounds of a range render in one unit, so `[100ms..1000ms]` stays
/// readable as a ratio instead of becoming `[100ms..1s]`, which would make the
/// reader do the arithmetic to compare the two ends.
fn bounds_text(timing: &v2::Timing) -> String {
    let min = timing.min_us.as_deref();
    let max = timing.max_us.as_deref();
    let unit = common_unit(&[min, max]);
    if v2::TimingMode::try_from(timing.mode) == Ok(v2::TimingMode::StrictPeriodic) {
        // Strict periodic stores the one period in both bounds.
        return min
            .or(max)
            .map(|value| duration_text(value, unit))
            .unwrap_or_else(|| "?".to_string());
    }
    format!(
        "[{}..{}]",
        min.map(|value| duration_text(value, unit))
            .unwrap_or_default(),
        max.map(|value| duration_text(value, unit))
            .unwrap_or_default(),
    )
}

/// The coarsest duration unit that renders every present bound as a whole
/// number, or microseconds when no coarser unit divides them all. The IR's
/// exactness rule reaches the hover text: a bound is never rounded to make a
/// nicer unit fit.
fn common_unit(bounds: &[Option<&str>]) -> (u128, &'static str) {
    let values: Vec<u128> = bounds
        .iter()
        .flatten()
        .filter_map(|value| value.parse::<u128>().ok())
        .collect();
    if values.len() != bounds.iter().flatten().count() || values.is_empty() {
        return (1, "us");
    }
    DURATION_UNITS
        .iter()
        .copied()
        .find(|(scale, _)| {
            values
                .iter()
                .all(|value| *value >= *scale && value % scale == 0)
        })
        .unwrap_or((1, "us"))
}

/// Renders one exact-decimal microsecond bound in `unit`. A value the unit
/// cannot express exactly — a fractional microsecond count, which ridl §2.1
/// already flags at the source — falls back to raw microseconds.
fn duration_text(microseconds: &str, (scale, unit): (u128, &'static str)) -> String {
    match microseconds.parse::<u128>() {
        Ok(value) if value % scale == 0 => format!("{}{unit}", value / scale),
        _ => format!("{microseconds}us"),
    }
}

/// The ridl §10 error-strata note for an inline fallible return, closing with
/// the general form §6.4 wording for Stratum 3.
fn strata_note(fallible: &v2::FallibleType) -> String {
    format!(
        "\n\n**Returns:** `{ok}` on success, `{err}` on failure\
         \n\n**Errors (ridl §10):** Stratum 1 — the declared `{err}` arm is functional failure, \
         carried as data. Stratum 2 — a contract violation is derived from `require`/`ensure`, \
         never an error type. Stratum 3 — {STRATUM_THREE}.",
        ok = fallible.ok,
        err = fallible.err,
    )
}

/// Renders a service declaration: its shape, and the ridl §14.5 posture note.
fn render_service(service: &v2::Service) -> String {
    let mut out = String::new();
    out.push_str("```ridl\n");
    match &service.shape {
        Some(v2::service::Shape::InterfaceRef(interface)) => {
            out.push_str(&format!("service {} : {interface}", service.name));
        }
        _ => out.push_str(&format!("service {} {{ … }}", service.name)),
    }
    out.push_str("\n```");

    match &service.shape {
        Some(v2::service::Shape::InterfaceRef(interface)) => {
            out.push_str(&format!("\n\n**Interface:** `{interface}`"));
        }
        Some(v2::service::Shape::Inline(shape)) => {
            let count = shape.interactions.len();
            let plural = if count == 1 { "" } else { "s" };
            out.push_str(&format!(
                "\n\n**Shape:** inline — {count} interaction{plural}"
            ));
        }
        None => {}
    }

    out.push_str(&format!("\n\n{POSTURE_NOTE}"));
    if !service.doc.is_empty() {
        out.push_str("\n\n");
        out.push_str(&service.doc);
    }
    if !service.labels.is_empty() {
        out.push_str(&format!("\n\n**Labels:** {}", service.labels.join(", ")));
    }
    if let Some(reason) = &service.deprecated {
        out.push_str(&format!("\n\n**Deprecated:** {reason}"));
    }
    out
}

/// The comma-separated parameter list of a command or query.
fn params(params: &[v2::Param]) -> String {
    params
        .iter()
        .map(|param| {
            let type_text = param
                .r#type
                .as_ref()
                .map(field_type_text)
                .unwrap_or_else(|| "?".to_string());
            format!("{}: {type_text}", param.name)
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// The source form of a query's return shape (ridl §7).
fn return_text(return_type: &v2::ReturnType) -> String {
    match &return_type.kind {
        Some(v2::return_type::Kind::Value(value)) => field_type_text(value),
        Some(v2::return_type::Kind::Fallible(fallible)) => {
            format!("{} | {}", fallible.ok, fallible.err)
        }
        None => "?".to_string(),
    }
}

/// The inline fallible arms of a return shape, if it has them.
fn fallible_of(return_type: &v2::ReturnType) -> Option<&v2::FallibleType> {
    match &return_type.kind {
        Some(v2::return_type::Kind::Fallible(fallible)) => Some(fallible),
        _ => None,
    }
}

/// The canonical named reference a field type carries, if it is a plain named
/// type — the only shape whose typl detail a payload line can expand.
fn named_ref(field_type: &v2::FieldType) -> Option<&str> {
    match &field_type.kind {
        Some(v2::field_type::Kind::Named(name)) => Some(name),
        _ => None,
    }
}

/// The source form of a field type (typl §7, §11–§12; ridl §12 for streams).
fn field_type_text(field_type: &v2::FieldType) -> String {
    let optional = if field_type.optional { "?" } else { "" };
    let base = match &field_type.kind {
        Some(v2::field_type::Kind::Named(name)) => name.clone(),
        Some(v2::field_type::Kind::Primitive(primitive)) => primitive_name(*primitive).to_string(),
        Some(v2::field_type::Kind::InlineScalar(scalar)) => format!(
            "{}{}",
            backing(scalar.backing.as_ref()).trim_start_matches(" : "),
            constraint(scalar.constraint.as_ref()),
        ),
        Some(v2::field_type::Kind::Tuple(tuple)) => format!(
            "({})",
            tuple
                .fields
                .iter()
                .map(|field| {
                    let type_text = field
                        .r#type
                        .as_ref()
                        .map(field_type_text)
                        .unwrap_or_else(|| "?".to_string());
                    format!("{}: {type_text}", field.name)
                })
                .collect::<Vec<_>>()
                .join(", "),
        ),
        Some(v2::field_type::Kind::Array(array)) => format!(
            "[{}; {}..{}]",
            array
                .element
                .as_ref()
                .map(|element| field_type_text(element))
                .unwrap_or_else(|| "?".to_string()),
            array.min,
            array.max,
        ),
        Some(v2::field_type::Kind::Map(map)) => format!(
            "{{{}: {}; {}..{}}}",
            map.key
                .as_ref()
                .map(|key| field_type_text(key))
                .unwrap_or_else(|| "?".to_string()),
            map.value
                .as_ref()
                .map(|value| field_type_text(value))
                .unwrap_or_else(|| "?".to_string()),
            map.min,
            map.max,
        ),
        Some(v2::field_type::Kind::Stream(stream)) => match &stream.element {
            Some(v2::stream_type::Element::Named(name)) => format!("<{name}>"),
            Some(v2::stream_type::Element::Primitive(primitive)) => {
                format!("<{}>", primitive_name(*primitive))
            }
            None => "<?>".to_string(),
        },
        None => "?".to_string(),
    };
    format!("{base}{optional}")
}

/// The declaration a canonical type reference names, looked up through the
/// memoized check query of whichever package owns it. A bare reference is
/// same-package by the IR's canonical-form rule.
fn decl_of_ref(
    db: &dyn salsa::Database,
    ws: Workspace,
    std: Package,
    pkg: Package,
    canonical: &str,
) -> Option<v2::Decl> {
    let (package_path, name) = match canonical.rsplit_once('.') {
        Some((path, name)) => (path.to_string(), name.to_string()),
        None => (pkg.name(db).clone(), canonical.to_string()),
    };
    let target = if package_path == *pkg.name(db) {
        Some(pkg)
    } else if package_path == *std.name(db) {
        Some(std)
    } else {
        package_of(db, ws, package_path)
    }?;
    check_package(db, ws, target, std)
        .ir
        .decls
        .into_iter()
        .find(|decl| decl.name == name)
}

/// The compact typl detail of a declaration used as a payload: the backing and
/// constraint of a scalar type, or the shape word of a composite.
fn type_summary(decl: &v2::Decl) -> Option<String> {
    match &decl.kind {
        Some(v2::decl::Kind::TypeDef(type_def)) => {
            let text = format!(
                "{}{}",
                backing(type_def.backing.as_ref()).trim_start_matches(" : "),
                constraint(type_def.constraint.as_ref()),
            );
            let text = text.trim();
            (!text.is_empty()).then(|| format!("`{text}`"))
        }
        Some(v2::decl::Kind::StructDef(_)) => Some("struct".to_string()),
        Some(v2::decl::Kind::EnumDef(_)) => Some("enum".to_string()),
        Some(v2::decl::Kind::EnumSetDef(_)) => Some("enumset".to_string()),
        Some(v2::decl::Kind::UnionDef(_)) => Some("union".to_string()),
        _ => None,
    }
}
