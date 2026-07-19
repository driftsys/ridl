//! Hover content (docs/ROADMAP.md epic E1.15b, ADR-0004 §10).
//!
//! Hover on a type reference or a declaration renders the declaration's IR —
//! qualified name, kind, backing and canonical UCUM unit, constraint, derived
//! wire width, init value, doc comment, labels, and deprecation — pulled from
//! the [`CheckedPackage`](ridl_sem::CheckedPackage) IR, the richest source,
//! keyed by the symbol [`symbol_at`] resolves. Hover on a
//! struct field instead shows the field's derived ordinal (general form §6.3
//! groundwork; the ordinal is typl §7.4), read straight from the IR so it counts
//! reserved tombstones exactly as codegen does.

use ridl_core::db::InputFile;
use ridl_core::package::{Package, Workspace, package_of};
use ridl_ir::v1;
use ridl_sem::{Symbol, SymbolKind, check_package};
use ridl_syntax::SyntaxKind;
use ridl_syntax::ast::{AstNode, StructDef};
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
pub(crate) fn field_ordinal(ir: &v1::Package, struct_name: &str, field_name: &str) -> Option<u32> {
    let decl = ir.decls.iter().find(|decl| decl.name == struct_name)?;
    let Some(v1::decl::Kind::StructDef(struct_def)) = &decl.kind else {
        return None;
    };
    struct_def
        .members
        .iter()
        .find_map(|member| match &member.member {
            Some(v1::struct_member::Member::Field(field)) if field.name == field_name => {
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
fn render_decl(qualified: &str, decl: &v1::Decl) -> String {
    let mut lines = String::new();
    lines.push_str("```typl\n");
    lines.push_str(&signature(qualified, decl));
    lines.push_str("\n```");

    if let Some(v1::decl::Kind::TypeDef(type_def)) = &decl.kind
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
fn signature(qualified: &str, decl: &v1::Decl) -> String {
    let modifiers = declaration_modifiers(decl);
    match &decl.kind {
        Some(v1::decl::Kind::TypeDef(type_def)) => {
            format!(
                "{modifiers}type {qualified}{}{}{}",
                backing(type_def.backing.as_ref()),
                constraint(type_def.constraint.as_ref()),
                init(type_def.init.as_ref(), type_def.declared_init.as_ref()),
            )
        }
        Some(v1::decl::Kind::ConstDef(const_def)) => {
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
        Some(v1::decl::Kind::StructDef(_)) => format!("{modifiers}struct {qualified}"),
        Some(v1::decl::Kind::EnumDef(_)) => format!("{modifiers}enum {qualified}"),
        Some(v1::decl::Kind::EnumSetDef(_)) => format!("{modifiers}enumset {qualified}"),
        Some(v1::decl::Kind::UnionDef(_)) => format!("{modifiers}union {qualified}"),
        None => qualified.to_string(),
    }
}

/// The `internal` / `error` modifier prefix (with a trailing space) for a
/// declaration's signature.
fn declaration_modifiers(decl: &v1::Decl) -> String {
    let mut prefix = String::new();
    if decl.visibility == v1::Visibility::Internal as i32 {
        prefix.push_str("internal ");
    }
    if decl.is_error {
        prefix.push_str("error ");
    }
    prefix
}

/// The backing clause of a type (`: km/h`, `: integer`), or the empty string
/// when the backing is missing.
fn backing(backing: Option<&v1::Backing>) -> String {
    match backing.and_then(|backing| backing.kind.as_ref()) {
        Some(v1::backing::Kind::Unit(unit)) => format!(" : {unit}"),
        Some(v1::backing::Kind::Primitive(primitive)) => {
            format!(" : {}", primitive_name(*primitive))
        }
        None => String::new(),
    }
}

/// The constraint clause of a type (`[0.0..250.0 step 0.5]`, `[0..256]`,
/// `[..100]`, `match /.../`), or the empty string when there is no constraint.
fn constraint(constraint: Option<&v1::Constraint>) -> String {
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
fn init(init: Option<&v1::InitValue>, declared: Option<&String>) -> String {
    if let Some(declared) = declared {
        return format!(" = {declared}");
    }
    match init.and_then(|init| init.value.as_ref()) {
        Some(value) => format!(" = {value}"),
        None => String::new(),
    }
}

/// The display name of a derived wire width.
fn width_name(width: &v1::type_def::Width) -> &'static str {
    match width {
        v1::type_def::Width::IntWidth(int_width) => int_width_name(*int_width),
        v1::type_def::Width::FloatWidth(float_width) => float_width_name(*float_width),
    }
}

/// The lowercase display name of a primitive type.
fn primitive_name(primitive: i32) -> &'static str {
    match v1::PrimitiveType::try_from(primitive) {
        Ok(v1::PrimitiveType::Boolean) => "boolean",
        Ok(v1::PrimitiveType::Integer) => "integer",
        Ok(v1::PrimitiveType::Float) => "float",
        Ok(v1::PrimitiveType::String) => "string",
        Ok(v1::PrimitiveType::Bytes) => "bytes",
        _ => "?",
    }
}

/// The lowercase display name of an integer wire width.
fn int_width_name(width: i32) -> &'static str {
    match v1::IntWidth::try_from(width) {
        Ok(v1::IntWidth::U8) => "u8",
        Ok(v1::IntWidth::I8) => "i8",
        Ok(v1::IntWidth::U16) => "u16",
        Ok(v1::IntWidth::I16) => "i16",
        Ok(v1::IntWidth::U32) => "u32",
        Ok(v1::IntWidth::I32) => "i32",
        Ok(v1::IntWidth::U64) => "u64",
        Ok(v1::IntWidth::I64) => "i64",
        _ => "?",
    }
}

/// The lowercase display name of a float wire width.
fn float_width_name(width: i32) -> &'static str {
    match v1::FloatWidth::try_from(width) {
        Ok(v1::FloatWidth::F32) => "f32",
        Ok(v1::FloatWidth::F64) => "f64",
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
