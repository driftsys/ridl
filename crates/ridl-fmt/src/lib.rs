//! `ridl fmt` — the CST-based formatter for typl and ridl declarations
//! (docs/ROADMAP.md epic E1.14, general form §5, typl reference §15.2).
//!
//! The formatter parses `text` with [`ridl_syntax::parse`] and rewrites the
//! lossless rowan tree into the canonical tight style. It is:
//!
//! - **CST-based** — it walks the concrete syntax tree, so it sees every token
//!   including trivia (whitespace, comments, doc comments);
//! - **trivia-aware** — comments and doc comments are preserved and re-anchored
//!   to what they precede; an inline trailing comment stays on its line;
//! - **total** — every syntactically valid input formats; a file with parse
//!   errors is never reformatted (fmt must not eat broken code, so it returns
//!   [`FormatOutcome::ParseErrors`] untouched);
//! - **idempotent** — `format(format(x)) == format(x)`.
//!
//! # The style, on the record
//!
//! General form §5 fixes the tight `name: Type` colon everywhere and forbids
//! column alignment. This module implements that decision plus the separator
//! and spacing rules the plan lists:
//!
//! - tight `name: Type` (one space after the colon, none before) in every
//!   position — declarations, tuple fields, map types, union arms, enumset
//!   derivation;
//! - one blank line between top-level definitions, none at the start of the
//!   file, exactly one trailing newline; the package line and the imports form
//!   a contiguous header block, then a blank line before the definitions;
//! - newline separators are canonical inside braces — separator commas are
//!   removed, one member per line, two-space indentation, no trailing comma;
//! - constraint spacing `[0.0..250.0 step 0.5]` — no spaces around `..`, single
//!   spaces around `step` and `match`, tight brackets;
//! - collections `[T; 8]` and `[K: V; 0..32]` — semicolon then space, tight
//!   colon; tuples `(min: Speed, max: Speed)` — comma then space;
//! - initialisers ` = value` spaced on both sides of `=`, likewise enum values
//!   `NAME = 0`.
//! - interfaces use the same brace-body layout; value and callable interactions
//!   have tight type colons, comma-separated parameter lists, spaced fallible
//!   returns and tight stream types and timing annotations;
//! - timing precedes an interaction's attribute block. Inline attributes have
//!   bracket padding and comma separators; predicates force one attribute per
//!   line. Binary expression operators have spaces, prefixes and member access
//!   stay tight, and source parentheses remain. Attribute value lists can break;
//! - inline services reuse interface bodies; named services keep required shape
//!   commas and remove the optional trailing comma. An overlong shape list breaks
//!   after the colon, one shape per line with commas between shapes;
//! - systems, components and distributions use one member per line. Component
//!   keywords and references have single spaces, and declaration and member
//!   attributes share the width rules. An overlong header keeps its opening
//!   brace on the attribute block's closing line. Deployments place their `for`
//!   reference before attributes and nest machine blocks one level down. Machine
//!   bodies use the same member layout, preserving source blank lines between
//!   machines.
//!
//! The pure entry point takes [`FormatOptions`], defaulting to a 100-character
//! code width, measured in Unicode scalar values including indentation. Tuple
//! types and parameter lists break one item per line, with commas between items,
//! when their code line exceeds the width. The last breakable construct on an overlong line
//! breaks first; lines are measured again after each break. Nested tuples break
//! only after their enclosing tuple. Trailing comments never cause a break;
//! unbreakable text stays over the limit. `None` disables line breaking.
//! The formatter reads no files or environment.
//! With the default `editorconfig` feature, `FormatOptions::for_path` reads
//! only `max_line_length` from the matching EditorConfig files. An integer sets
//! the width, `off` disables it, and missing, unset, or invalid values use 100.
//! Indentation settings are ignored. The pure entry point still reads no file.
//! The CLI resolves options for each file; the LSP resolves them from the
//! document path and formats the current buffer. Client indentation options
//! remain ignored.
//!
//! # What order is *not* changed
//!
//! Source order is wire identity (typl reference §7.4): a formatter must never
//! change ordinals. This formatter normalises whitespace and separators, and
//! places timing before attributes in an interaction annotation pair (D-4).
//! It never reorders declarations, imports, fields, enum values, or union arms.
//! Input whose first item is not the `package` declaration is a missing-package
//! parse error (FORM-104), so it is returned untouched and never reformatted;
//! the never-reorder guarantee applies to the inputs that do format.
//!
//! # Comments are never dropped
//!
//! Comments and doc comments survive everywhere. At declaration and member
//! boundaries they are re-anchored: a leading comment leads the item it
//! precedes, and an inline trailing comment stays on its line — including a
//! comment on the opening-brace line of a block. A comment embedded *inside* a
//! single-line construct — between the brackets of a constraint or a
//! collection, the parentheses of a tuple, or the tokens of one declaration —
//! cannot be reflowed into the tight style without risking its meaning, so the
//! enclosing construct is emitted verbatim from source instead of being
//! re-synthesised. Inline comments between interaction annotations are an
//! exception: they stay with the preceding annotation when timing moves first.
//! If moving an annotation line comment would consume another trailing comment,
//! the whole member stays verbatim. Line comments retain their newline.
//! This leaves the node structure
//! and the non-trivia token set unchanged, and stays idempotent.
//! The property harness checks all three implemented profiles at widths 100,
//! 60 and 40, comparing node entry and exit, token identity, and comment text.

use ridl_syntax::{
    Profile, SyntaxKind, SyntaxNode,
    ast::{AstNode, SourceFile},
};
use rowan::NodeOrToken;
use std::collections::HashSet;

#[cfg(feature = "editorconfig")]
mod editorconfig;

/// Options for the pure formatter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormatOptions {
    /// Maximum code characters per line, or `None` for no limit.
    pub max_line_length: Option<usize>,
}

impl Default for FormatOptions {
    fn default() -> Self {
        Self {
            max_line_length: Some(100),
        }
    }
}

/// The outcome of formatting one source text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FormatOutcome {
    /// The canonical rendering of a syntactically valid input.
    Formatted(String),
    /// The parse diagnostics of a broken input, which is left unformatted.
    ///
    /// The interface returns [`ridl_syntax::SyntaxError`] rather than the coded
    /// `Diagnostic` model (ADR-0004 §5): the diagnostics framework (task E1.10)
    /// is not a dependency of this crate yet. The CLI facade (task E1.13) maps
    /// each `SyntaxError` — which already carries its stable code — into a
    /// `Diagnostic` at the boundary.
    ParseErrors(Vec<ridl_syntax::SyntaxError>),
}

/// Formats `text` into the canonical tight style, parsing it under `profile`
/// (`Profile::Typl` for a `.typl` file, `Profile::Ridl` for a `.ridl` file,
/// `Profile::Rsdl` for a `.rsdl` file — the formatting rules themselves are
/// profile-independent).
///
/// A syntactically valid input yields [`FormatOutcome::Formatted`]; an input
/// with any parse error yields [`FormatOutcome::ParseErrors`] and is not
/// rewritten.
pub fn format(text: &str, profile: Profile, options: &FormatOptions) -> FormatOutcome {
    let parse = ridl_syntax::parse(text, profile);
    if !parse.errors().is_empty() {
        return FormatOutcome::ParseErrors(parse.errors().to_vec());
    }
    let Some(file) = SourceFile::cast(parse.syntax()) else {
        // `parse` always roots a `SourceFile`; this arm cannot be reached, but
        // returning the input unchanged is the honest fallback.
        return FormatOutcome::Formatted(text.to_string());
    };
    FormatOutcome::Formatted(format_source_file(&file, options))
}

// --- vertical layout -----------------------------------------------------

/// The role a rendered block plays in its container's vertical spacing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BlockKind {
    /// A `package` or `import` line — the contiguous file header.
    Header,
    /// A top-level definition.
    Def,
    /// A member of a brace block (field, reserved entry, enum value, enumset
    /// bit, union arm).
    Member,
    /// A run of comments with no structural node to lead (only ever the last
    /// block of a container).
    CommentOnly,
}

/// One rendered unit of a container: its physical lines plus the spacing
/// metadata the container needs to place blank lines around it.
struct Block {
    kind: BlockKind,
    /// Whether the source had a blank line immediately before this block.
    gap_blank: bool,
    /// The fully-indented physical lines; an empty string is a blank line.
    lines: Vec<String>,
}

/// Formats the whole file: the container laid out at indent zero, joined with
/// exactly one trailing newline and no leading blank line.
fn format_source_file(file: &SourceFile, options: &FormatOptions) -> String {
    let elements: Vec<_> = file.syntax().children_with_tokens().collect();
    let lines = layout_container(&elements, 0, true, options);
    let mut out = lines.join("\n");
    out.push('\n');
    out
}

/// Lays out one container — the source file, or the members between a block's
/// braces — into physical lines.
///
/// The single forward pass folds leading comments into the block of the node
/// they precede, keeps an inline trailing comment on the line of the node it
/// follows, and drops separator commas. `is_source_file` selects the vertical
/// spacing policy: the file forces one blank line between definitions, a brace
/// block spaces members only where the source did.
fn layout_container(
    elements: &[SyntaxElement],
    indent: usize,
    is_source_file: bool,
    options: &FormatOptions,
) -> Vec<String> {
    let ind = indent_str(indent);
    let blocks: Vec<Block> = collect_container(elements)
        .into_iter()
        .map(|block| {
            let mut lines = Vec::new();
            for (i, comment) in block.leading.iter().enumerate() {
                if i > 0 && comment.blank_before {
                    lines.push(String::new());
                }
                lines.push(format!("{ind}{}", comment.text));
            }
            if block.blank_before_node {
                lines.push(String::new());
            }
            if let Some(node) = &block.node {
                if !block.trailing.is_empty() && has_moved_annotation_line_comment(node) {
                    lines.push(format!("{ind}{}", node.text()));
                } else {
                    lines.extend(format_element(node, indent, options));
                }
            }
            for comment in &block.trailing {
                if let Some(line) = lines.last_mut() {
                    line.push(' ');
                    line.push_str(comment);
                }
            }
            Block {
                kind: block.kind,
                gap_blank: block.gap_blank,
                lines,
            }
        })
        .collect();

    let mut out: Vec<String> = Vec::new();
    for (i, block) in blocks.iter().enumerate() {
        if i > 0 {
            let blanks = blanks_between(is_source_file, &blocks[i - 1], block);
            for _ in 0..blanks {
                out.push(String::new());
            }
        }
        out.extend(block.lines.iter().cloned());
    }
    out
}

/// One source container unit before its node is rendered. Attribute bodies and
/// brace bodies share this collector, including leading and trailing comments.
struct ContainerBlock {
    kind: BlockKind,
    gap_blank: bool,
    leading: Vec<PendingComment>,
    blank_before_node: bool,
    node: Option<SyntaxNode>,
    trailing: Vec<String>,
}

fn collect_container(elements: &[SyntaxElement]) -> Vec<ContainerBlock> {
    let mut blocks: Vec<ContainerBlock> = Vec::new();
    let mut pending: Vec<PendingComment> = Vec::new();
    let mut nl_run = 0usize;
    let mut can_trail = false;
    for element in elements {
        match element {
            NodeOrToken::Token(token) if token.kind() == SyntaxKind::Whitespace => {
                nl_run += token.text().matches('\n').count();
            }
            NodeOrToken::Token(token) if is_comment(token.kind()) => {
                let text = token.text().trim_end().to_string();
                if can_trail && nl_run == 0 && pending.is_empty() {
                    if let Some(last) = blocks.last_mut() {
                        last.trailing.push(text);
                    }
                } else {
                    pending.push(PendingComment {
                        blank_before: nl_run >= 2,
                        text,
                    });
                    can_trail = false;
                }
                nl_run = 0;
            }
            NodeOrToken::Token(token) if token.kind() == SyntaxKind::Comma => {
                if nl_run > 0 {
                    can_trail = false;
                }
                // A separator line is not a blank line. Retain an existing
                // source blank line without attaching later comments backward.
                nl_run = if nl_run >= 2 { 2 } else { 0 };
            }
            NodeOrToken::Node(node) => {
                let gap_blank = pending.first().map_or(nl_run >= 2, |c| c.blank_before);
                let blank_before_node = !pending.is_empty() && nl_run >= 2;
                blocks.push(ContainerBlock {
                    kind: block_kind(node.kind()),
                    gap_blank,
                    leading: std::mem::take(&mut pending),
                    blank_before_node,
                    node: Some(node.clone()),
                    trailing: Vec::new(),
                });
                nl_run = 0;
                can_trail = true;
            }
            _ => nl_run = 0,
        }
    }
    if !pending.is_empty() {
        blocks.push(ContainerBlock {
            kind: BlockKind::CommentOnly,
            gap_blank: pending[0].blank_before,
            leading: pending,
            blank_before_node: false,
            node: None,
            trailing: Vec::new(),
        });
    }
    blocks
}

/// A comment waiting to lead the next node, with whether the source placed a
/// blank line before it.
struct PendingComment {
    blank_before: bool,
    text: String,
}

/// The number of blank lines to place between two adjacent blocks.
fn blanks_between(is_source_file: bool, prev: &Block, cur: &Block) -> usize {
    if is_source_file {
        if prev.kind == BlockKind::Header && cur.kind == BlockKind::Header {
            // The package line and the imports form one contiguous header.
            0
        } else if cur.kind == BlockKind::CommentOnly {
            usize::from(cur.gap_blank)
        } else {
            // One blank line between every pair of top-level definitions.
            1
        }
    } else {
        // Inside a brace block a blank line appears only where the source had
        // one; members are already one per line.
        usize::from(cur.gap_blank)
    }
}

/// The vertical role of a container child by its node kind.
fn block_kind(kind: SyntaxKind) -> BlockKind {
    match kind {
        SyntaxKind::PackageDecl | SyntaxKind::Import => BlockKind::Header,
        SyntaxKind::TypeDef
        | SyntaxKind::ConstDef
        | SyntaxKind::StructDef
        | SyntaxKind::EnumDef
        | SyntaxKind::EnumSetDef
        | SyntaxKind::UnionDef
        | SyntaxKind::InterfaceDef
        | SyntaxKind::ServiceDef
        | SyntaxKind::SystemDef
        | SyntaxKind::ComponentDef
        | SyntaxKind::DistributionDef
        | SyntaxKind::DeploymentDef => BlockKind::Def,
        _ => BlockKind::Member,
    }
}

// --- element dispatch ----------------------------------------------------

type SyntaxElement = NodeOrToken<SyntaxNode, ridl_syntax::SyntaxToken>;

/// Formats one structural node into its physical lines, indented at `indent`.
fn format_element(node: &SyntaxNode, indent: usize, options: &FormatOptions) -> Vec<String> {
    let ind = indent_str(indent);
    let line = |s: String| vec![format!("{ind}{s}")];
    // A comment wedged directly among a single-line element's own tokens (for
    // example between a field name and its colon) would be dropped by the
    // token-stitching synthesis; emit the whole element verbatim so no comment
    // is ever lost. Inline comments between interaction annotations travel with
    // their preceding annotation when timing moves first. Brace-block definitions
    // are excluded: their direct comments are the between-member comments that
    // `layout_container` places,
    // and their header-region comments are handled by `format_block_def`.
    if is_single_line_element(node)
        && has_direct_comment(node)
        && !has_only_inline_annotation_comments(node)
    {
        return vec![format!("{ind}{}", node.text())];
    }
    match node.kind() {
        SyntaxKind::PackageDecl => line(format!(
            "package {}",
            child_tight(node, SyntaxKind::QualifiedName)
        )),
        SyntaxKind::Import => line(format_import(node)),
        SyntaxKind::TypeDef => line(format_type_def(node)),
        SyntaxKind::ConstDef => line(format_const_def(node)),
        SyntaxKind::EnumSetDef => {
            if has_token(node, SyntaxKind::LBrace) {
                format_block_def(node, indent, "enumset", options)
            } else {
                line(format_enumset_derived(node))
            }
        }
        SyntaxKind::StructDef => format_block_def(node, indent, "struct", options),
        SyntaxKind::EnumDef => format_block_def(node, indent, "enum", options),
        SyntaxKind::UnionDef => format_block_def(node, indent, "union", options),
        SyntaxKind::InterfaceDef => format_block_def(node, indent, "interface", options),
        SyntaxKind::ServiceDef if has_token(node, SyntaxKind::LBrace) => {
            format_block_def(node, indent, "service", options)
        }
        SyntaxKind::ServiceDef => render_layout(&format_named_service(node), indent, options),
        SyntaxKind::SignalDef
        | SyntaxKind::EventDef
        | SyntaxKind::FixedDef
        | SyntaxKind::CommandDef
        | SyntaxKind::QueryDef => render_layout(&format_interaction(node), indent, options),
        SyntaxKind::SystemDef => format_block_def(node, indent, "system", options),
        SyntaxKind::ComponentDef => format_block_def(node, indent, "component", options),
        SyntaxKind::DistributionDef => format_block_def(node, indent, "distribution", options),
        SyntaxKind::DeploymentDef => format_block_def(node, indent, "deployment", options),
        SyntaxKind::MachineDef => format_block_def(node, indent, "machine", options),
        SyntaxKind::MemberLine | SyntaxKind::ComponentLine => {
            render_layout(&format_rsdl_line(node), indent, options)
        }
        SyntaxKind::AttrBlock => render_layout(&format_attr_block(node), indent, options),
        SyntaxKind::FieldDef => render_layout(&format_field_def(node), indent, options),
        SyntaxKind::ReservedEntry => line(format_reserved_entry(node)),
        SyntaxKind::EnumValue | SyntaxKind::EnumSetBit => line(format_value_assignment(node)),
        SyntaxKind::UnionArm => line(format_union_arm(node)),
        // Preserve nodes without a layout rule so their source remains lossless.
        _ => line(node.text().to_string()),
    }
}

// --- single-line definitions --------------------------------------------

fn format_import(node: &SyntaxNode) -> String {
    let mut out = format!("import {}", child_tight(node, SyntaxKind::QualifiedName));
    if let Some(alias) = child_node(node, SyntaxKind::Name) {
        out.push_str(" as ");
        out.push_str(&tight_text(&alias));
    }
    out
}

fn format_type_def(node: &SyntaxNode) -> String {
    let mut out = format!(
        "{}type {}: {}",
        modifiers_prefix(node),
        child_tight(node, SyntaxKind::Name),
        format_backing(node),
    );
    if let Some(constraint) = child_node(node, SyntaxKind::Constraint) {
        out.push(' ');
        out.push_str(&format_constraint(&constraint));
    }
    if let Some(init) = child_node(node, SyntaxKind::InitValue)
        && let Some(literal) = child_node(&init, SyntaxKind::Literal)
    {
        out.push_str(" = ");
        out.push_str(&tight_text(&literal));
    }
    out
}

/// The backing of a `type` — a primitive keyword or a tight UCUM expression.
fn format_backing(node: &SyntaxNode) -> String {
    if let Some(primitive) = child_node(node, SyntaxKind::PrimitiveType) {
        primitive_keyword(&primitive)
    } else if let Some(unit) = child_node(node, SyntaxKind::UnitExpr) {
        tight_text(&unit)
    } else {
        String::new()
    }
}

fn format_const_def(node: &SyntaxNode) -> String {
    let mut out = format!(
        "{}const {}",
        modifiers_prefix(node),
        child_tight(node, SyntaxKind::Name)
    );
    if let Some(type_ref) = child_node(node, SyntaxKind::PathType) {
        out.push_str(": ");
        out.push_str(&tight_text(&type_ref));
    }
    out.push_str(" = ");
    if let Some(literal) = child_node(node, SyntaxKind::Literal) {
        out.push_str(&tight_text(&literal));
    }
    out
}

fn format_enumset_derived(node: &SyntaxNode) -> String {
    format!(
        "{}enumset {}: {}",
        modifiers_prefix(node),
        child_tight(node, SyntaxKind::Name),
        child_tight(node, SyntaxKind::PathType),
    )
}

// --- brace-block definitions --------------------------------------------

/// Formats a `struct`, `enum`, `union`, standalone `enumset`, `interface`,
/// or inline `service`:
/// the header, the members at the next indent, and the closing brace. An empty
/// body renders as `{}` on the header line. A comment on the opening-brace line
/// stays on that line; a comment in the header region is preserved verbatim.
fn format_block_def(
    node: &SyntaxNode,
    indent: usize,
    keyword: &str,
    options: &FormatOptions,
) -> Vec<String> {
    let ind = indent_str(indent);
    let header_prefix = block_header_prefix(node, keyword);
    let brace_separator = match &header_prefix {
        Layout::Text(text) if text.ends_with('\n') => ind.as_str(),
        _ => " ",
    };
    let mut header = vec![header_prefix, Layout::Text(brace_separator.into())];
    let all_members = elements_between_braces(node);
    let (brace_comment, members) = split_brace_line_comment(&all_members);
    let member_lines = layout_container(members, indent + 1, false, options);

    if member_lines.is_empty() && brace_comment.is_none() {
        header.push(Layout::Text("{}".into()));
        return render_layout(&Layout::Concat(header), indent, options);
    }

    header.push(Layout::Text("{".into()));
    if let Some(comment) = brace_comment {
        header.push(Layout::TrailingComment(comment));
    }
    let mut out = render_layout(&Layout::Concat(header), indent, options);
    out.extend(member_lines);
    out.push(format!("{ind}}}"));
    out
}

/// The header layout before the opening brace, including header attributes.
/// A direct header comment keeps the region verbatim; attributes otherwise
/// expose their break positions to the same renderer as member attributes.
fn block_header_prefix(node: &SyntaxNode, keyword: &str) -> Layout {
    let mut verbatim = String::new();
    let mut has_comment = false;
    let mut ends_in_line_comment = false;
    for element in node.children_with_tokens() {
        match element {
            NodeOrToken::Token(t) if t.kind() == SyntaxKind::LBrace => break,
            NodeOrToken::Token(t) => {
                has_comment |= is_comment(t.kind());
                if t.kind() != SyntaxKind::Whitespace {
                    ends_in_line_comment = is_line_comment(&t);
                }
                verbatim.push_str(t.text());
            }
            NodeOrToken::Node(n) => {
                ends_in_line_comment = false;
                verbatim.push_str(&n.text().to_string());
            }
        }
    }
    if has_comment {
        let mut header = verbatim.trim_end().to_string();
        if ends_in_line_comment {
            header.push('\n');
        }
        Layout::Text(header)
    } else {
        let mut parts = vec![Layout::Text(format!(
            "{}{keyword} {}",
            modifiers_prefix(node),
            child_tight(
                node,
                if node.kind() == SyntaxKind::ServiceDef {
                    SyntaxKind::DottedName
                } else {
                    SyntaxKind::Name
                }
            )
        ))];
        if node.kind() == SyntaxKind::DeploymentDef {
            parts.push(Layout::Text(" for ".into()));
            parts.push(Layout::Text(
                child_node(node, SyntaxKind::Reference)
                    .map(|reference| reference_text(&reference))
                    .unwrap_or_default(),
            ));
        }
        if let Some(attributes) = child_node(node, SyntaxKind::AttrBlock) {
            parts.push(Layout::Text(" ".into()));
            parts.push(format_attr_block(&attributes));
        }
        Layout::Concat(parts)
    }
}

/// A component keyword and reference, or a bare member reference, followed by
/// the shared attribute layout. Commented references retain their own text.
fn format_rsdl_line(node: &SyntaxNode) -> Layout {
    let mut parts = Vec::new();
    if node.kind() == SyntaxKind::ComponentLine {
        let keyword = node
            .children_with_tokens()
            .filter_map(NodeOrToken::into_token)
            .find(|token| matches!(token.kind(), SyntaxKind::OffersKw | SyntaxKind::RequiresKw))
            .map(|token| token.text().to_string())
            .unwrap_or_default();
        parts.push(Layout::Text(format!("{keyword} ")));
    }
    if let Some(reference) = child_node(node, SyntaxKind::Reference) {
        parts.push(Layout::Text(reference_text(&reference)));
    }
    if let Some(attributes) = child_node(node, SyntaxKind::AttrBlock) {
        parts.push(Layout::Text(" ".into()));
        parts.push(format_attr_block(&attributes));
    }
    Layout::Concat(parts)
}

/// Tight references share one comment-preserving fallback in headers and bodies.
fn reference_text(reference: &SyntaxNode) -> String {
    if contains_comment(reference) {
        reference.text().to_string()
    } else {
        tight_text(reference)
    }
}

/// Splits off a comment that sits on the opening-brace line — before the first
/// newline — so it can stay on that line rather than move above the first
/// member. Returns the comment text and the remaining member elements.
fn split_brace_line_comment(elements: &[SyntaxElement]) -> (Option<String>, &[SyntaxElement]) {
    let mut i = 0;
    while let Some(element) = elements.get(i) {
        match element {
            NodeOrToken::Token(t) if t.kind() == SyntaxKind::Whitespace => {
                if t.text().contains('\n') {
                    break;
                }
                i += 1;
            }
            NodeOrToken::Token(t) if is_comment(t.kind()) => {
                return (Some(t.text().trim_end().to_string()), &elements[i + 1..]);
            }
            _ => break,
        }
    }
    (None, elements)
}

/// The container children strictly between the block's first `{` and its
/// closing `}` — member nodes, separator commas, and trivia.
fn elements_between_braces(node: &SyntaxNode) -> Vec<SyntaxElement> {
    elements_between_delimiters(node, SyntaxKind::LBrace, SyntaxKind::RBrace)
}

fn elements_between_delimiters(
    node: &SyntaxNode,
    open: SyntaxKind,
    close: SyntaxKind,
) -> Vec<SyntaxElement> {
    let mut out = Vec::new();
    let mut inside = false;
    for element in node.children_with_tokens() {
        match &element {
            NodeOrToken::Token(t) if t.kind() == open && !inside => inside = true,
            NodeOrToken::Token(t) if t.kind() == close => break,
            _ if inside => out.push(element),
            _ => {}
        }
    }
    out
}

// --- members -------------------------------------------------------------

fn format_field_def(node: &SyntaxNode) -> Layout {
    let mut parts = vec![
        Layout::Text(format!("{}: ", child_tight(node, SyntaxKind::Name))),
        field_type(node),
    ];
    if let Some(init) = child_node(node, SyntaxKind::InitValue)
        && let Some(literal) = child_node(&init, SyntaxKind::Literal)
    {
        parts.push(Layout::Text(format!(" = {}", tight_text(&literal))));
    }
    Layout::Concat(parts)
}

fn format_reserved_entry(node: &SyntaxNode) -> String {
    let target = child_node(node, SyntaxKind::Name)
        .or_else(|| child_node(node, SyntaxKind::Literal))
        .map(|n| tight_text(&n))
        .unwrap_or_default();
    format!("reserved {target}")
}

fn format_value_assignment(node: &SyntaxNode) -> String {
    let value = child_node(node, SyntaxKind::Literal)
        .map(|n| tight_text(&n))
        .unwrap_or_default();
    format!("{} = {value}", child_tight(node, SyntaxKind::Name))
}

fn format_union_arm(node: &SyntaxNode) -> String {
    format!(
        "{}: {}",
        child_tight(node, SyntaxKind::Name),
        child_tight(node, SyntaxKind::PathType),
    )
}

// --- named services ------------------------------------------------------

fn format_named_service(node: &SyntaxNode) -> Layout {
    if contains_comment(node) {
        return Layout::Text(node.text().to_string());
    }
    Layout::Concat(vec![
        Layout::Text(format!(
            "service {}:",
            child_tight(node, SyntaxKind::DottedName)
        )),
        Layout::Shapes(
            node.children()
                .filter(|child| child.kind() == SyntaxKind::PathType)
                .map(|shape| tight_text(&shape))
                .collect(),
        ),
    ])
}

// --- interface members ---------------------------------------------------

/// The value or callable member, with timing before attributes. Each parser
/// accepted slot is retained, including slots narrowed by the checker.
fn format_interaction(node: &SyntaxNode) -> Layout {
    let (keyword, callable) = match node.kind() {
        SyntaxKind::SignalDef => ("signal", false),
        SyntaxKind::EventDef => ("event", false),
        SyntaxKind::FixedDef => ("fixed", false),
        SyntaxKind::CommandDef => ("command", true),
        SyntaxKind::QueryDef => ("query", true),
        _ => return Layout::Text(node.text().to_string()),
    };
    let mut parts = vec![Layout::Text(format!(
        "{keyword} {}",
        child_tight(node, SyntaxKind::Name)
    ))];
    if callable {
        if let Some(params) = child_node(node, SyntaxKind::ParamList) {
            parts.push(format_param_list(&params));
        }
        if let Some(result) = child_node(node, SyntaxKind::ReturnType) {
            parts.push(Layout::Text(": ".into()));
            parts.push(format_return_type(&result));
        }
    } else {
        parts.push(Layout::Text(": ".into()));
        parts.push(field_type(node));
    }
    if let Some(init) = child_node(node, SyntaxKind::InitValue) {
        if contains_comment(&init) {
            parts.push(Layout::Text(format!(" {}", init.text())));
        } else if let Some(literal) = child_node(&init, SyntaxKind::Literal) {
            parts.push(Layout::Text(format!(" = {}", tight_text(&literal))));
        }
    }
    if let Some(timing) = child_node(node, SyntaxKind::Timing) {
        parts.push(Layout::Text(format!(" {}", tight_text(&timing))));
        parts.extend(annotation_comments(node, SyntaxKind::Timing, false));
    }
    if let Some(attrs) = child_node(node, SyntaxKind::AttrBlock) {
        if !matches!(parts.last(), Some(Layout::LineBreak)) {
            parts.push(Layout::Text(" ".into()));
        }
        parts.push(format_attr_block(&attrs));
        parts.extend(annotation_comments(node, SyntaxKind::AttrBlock, true));
    }
    Layout::Concat(parts)
}

/// Inline comments between annotations belong to the preceding annotation.
/// Other direct comments retain the existing whole-member verbatim path.
fn has_only_inline_annotation_comments(node: &SyntaxNode) -> bool {
    if !matches!(
        node.kind(),
        SyntaxKind::SignalDef
            | SyntaxKind::EventDef
            | SyntaxKind::FixedDef
            | SyntaxKind::CommandDef
            | SyntaxKind::QueryDef
    ) || child_node(node, SyntaxKind::Timing).is_none()
        || child_node(node, SyntaxKind::AttrBlock).is_none()
    {
        return false;
    }
    let mut owner = None;
    let mut inline = false;
    for element in node.children_with_tokens() {
        match element {
            NodeOrToken::Node(child) => {
                owner = Some(child.kind());
                inline = true;
            }
            NodeOrToken::Token(token) if is_comment(token.kind()) => {
                if !inline
                    || token.text().contains('\n')
                    || !matches!(owner, Some(SyntaxKind::Timing | SyntaxKind::AttrBlock))
                {
                    return false;
                }
            }
            NodeOrToken::Token(token) if token.text().contains('\n') => inline = false,
            _ => {}
        }
    }
    true
}

/// Moving an attribute's line comment past timing would consume any later
/// trailing comment. The container retains this whole member when one exists.
fn has_moved_annotation_line_comment(node: &SyntaxNode) -> bool {
    if !has_only_inline_annotation_comments(node) {
        return false;
    }
    let mut owner = None;
    for element in node.children_with_tokens() {
        match element {
            NodeOrToken::Node(child) if child.kind() == SyntaxKind::Timing => return false,
            NodeOrToken::Node(child) => owner = Some(child.kind()),
            NodeOrToken::Token(token)
                if owner == Some(SyntaxKind::AttrBlock) && is_line_comment(&token) =>
            {
                return true;
            }
            _ => {}
        }
    }
    false
}

fn annotation_comments(node: &SyntaxNode, annotation: SyntaxKind, trailing: bool) -> Vec<Layout> {
    let mut owner = None;
    let mut comments = Vec::new();
    for element in node.children_with_tokens() {
        match element {
            NodeOrToken::Node(child) => owner = Some(child.kind()),
            NodeOrToken::Token(token) if owner == Some(annotation) && is_comment(token.kind()) => {
                let text = token.text().trim_end().to_string();
                comments.push(if trailing || is_line_comment(&token) {
                    Layout::TrailingComment(text)
                } else {
                    Layout::Text(format!(" {text}"))
                });
                if !trailing && is_line_comment(&token) {
                    comments.push(Layout::LineBreak);
                }
            }
            _ => {}
        }
    }
    comments
}

fn format_param_list(node: &SyntaxNode) -> Layout {
    if contains_comment(node) {
        return Layout::Text(node.text().to_string());
    }
    Layout::Tuple(
        node.children()
            .filter(|n| n.kind() == SyntaxKind::Param)
            .map(|param| {
                Layout::Concat(vec![
                    Layout::Text(format!("{}: ", child_tight(&param, SyntaxKind::Name))),
                    field_type(&param),
                ])
            })
            .collect(),
    )
}

fn format_return_type(node: &SyntaxNode) -> Layout {
    if contains_comment(node) {
        return Layout::Text(node.text().to_string());
    }
    if let Some(fallible) = child_node(node, SyntaxKind::FallibleType) {
        let mut parts = Vec::new();
        for (index, path) in fallible.children().enumerate() {
            if index > 0 {
                parts.push(Layout::Text(" | ".into()));
            }
            parts.push(Layout::Text(tight_text(&path)));
        }
        Layout::Concat(parts)
    } else {
        field_type(node)
    }
}

// --- attributes and predicate expressions --------------------------------

fn format_attr_block(node: &SyntaxNode) -> Layout {
    let force_block = node.children().any(|attr| {
        has_token(&attr, SyntaxKind::RequireKw) || has_token(&attr, SyntaxKind::EnsureKw)
    });
    if !force_block && contains_comment(node) {
        return Layout::Text(node.text().to_string());
    }
    let elements = elements_between_delimiters(node, SyntaxKind::LBracket, SyntaxKind::RBracket);
    let blocks = collect_container(&elements)
        .into_iter()
        .map(|block| AttributeLayout {
            gap_blank: block.gap_blank,
            leading: block.leading,
            blank_before_node: block.blank_before_node,
            layout: block.node.as_ref().map(format_attribute),
            trailing: block.trailing,
        })
        .collect();
    Layout::Attributes {
        blocks,
        force_block,
    }
}

fn format_attribute(node: &SyntaxNode) -> Layout {
    if contains_comment(node) {
        return Layout::Text(node.text().to_string());
    }
    if has_token(node, SyntaxKind::RequireKw) || has_token(node, SyntaxKind::EnsureKw) {
        let keyword = if has_token(node, SyntaxKind::RequireKw) {
            "require"
        } else {
            "ensure"
        };
        let expr = node
            .children()
            .next()
            .map(|n| format_expr(&n))
            .unwrap_or_default();
        return Layout::Text(format!("{keyword} {expr}"));
    }
    let key = node
        .children()
        .filter(|n| n.kind() == SyntaxKind::Name)
        .map(|n| tight_text(&n))
        .collect::<Vec<_>>()
        .join(".");
    match child_node(node, SyntaxKind::AttrValue) {
        Some(value) => Layout::Concat(vec![
            Layout::Text(format!("{key} = ")),
            format_attr_value(&value),
        ]),
        None => Layout::Text(key),
    }
}

fn format_attr_value(node: &SyntaxNode) -> Layout {
    if contains_comment(node) {
        return Layout::Text(node.text().to_string());
    }
    if has_token(node, SyntaxKind::LParen) {
        Layout::Tuple(
            node.children()
                .filter(|n| n.kind() == SyntaxKind::AttrValue)
                .map(|n| format_attr_value(&n))
                .collect(),
        )
    } else {
        Layout::Text(tight_text(node))
    }
}

/// Synthesize only spacing: every source expression node and parenthesis stays.
fn format_expr(node: &SyntaxNode) -> String {
    match node.kind() {
        SyntaxKind::BinaryExpr => {
            let mut children = node.children();
            let left = children.next().map(|n| format_expr(&n)).unwrap_or_default();
            let right = children.next().map(|n| format_expr(&n)).unwrap_or_default();
            let operator = node
                .children_with_tokens()
                .filter_map(NodeOrToken::into_token)
                .find(|t| !t.kind().is_trivia())
                .map(|t| t.text().to_string())
                .unwrap_or_default();
            format!("{left} {operator} {right}")
        }
        SyntaxKind::PrefixExpr => {
            let operator = node
                .children_with_tokens()
                .filter_map(NodeOrToken::into_token)
                .find(|t| !t.kind().is_trivia())
                .map(|t| t.text().to_string())
                .unwrap_or_default();
            let operand = node
                .children()
                .next()
                .map(|n| format_expr(&n))
                .unwrap_or_default();
            format!("{operator}{operand}")
        }
        SyntaxKind::MemberExpr => {
            let left = node
                .children()
                .next()
                .map(|n| format_expr(&n))
                .unwrap_or_default();
            let member = node
                .children_with_tokens()
                .filter_map(NodeOrToken::into_token)
                .find(|t| t.kind() == SyntaxKind::Ident)
                .map(|t| t.text().to_string())
                .unwrap_or_default();
            format!("{left}.{member}")
        }
        SyntaxKind::ParenExpr => {
            let inner = node
                .children()
                .next()
                .map(|n| format_expr(&n))
                .unwrap_or_default();
            format!("({inner})")
        }
        _ => tight_text(node),
    }
}

// --- field types ---------------------------------------------------------

/// The first field-type child of `node`, with tuple break positions retained.
fn field_type(node: &SyntaxNode) -> Layout {
    node.children()
        .find(|c| is_field_type(c.kind()))
        .map(|c| format_field_type(&c))
        .unwrap_or_else(|| Layout::Text(String::new()))
}

fn is_field_type(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::PathType
            | SyntaxKind::PrimitiveType
            | SyntaxKind::TupleType
            | SyntaxKind::ArrayType
            | SyntaxKind::MapType
            | SyntaxKind::OptionalType
            | SyntaxKind::StreamType
    )
}

/// A canonical text fragment, concatenation, or breakable tuple, attribute
/// block, or service shape list. Text also carries verbatim constructs.
enum Layout {
    Text(String),
    TrailingComment(String),
    LineBreak,
    Concat(Vec<Layout>),
    Tuple(Vec<Layout>),
    Shapes(Vec<String>),
    Attributes {
        blocks: Vec<AttributeLayout>,
        force_block: bool,
    },
}

struct AttributeLayout {
    gap_blank: bool,
    leading: Vec<PendingComment>,
    blank_before_node: bool,
    layout: Option<Layout>,
    trailing: Vec<String>,
}

/// An outermost unbroken construct on a rendered physical line. Nested tuples
/// and attribute blocks become candidates when their parent breaks.
struct BreakCandidate {
    id: usize,
    line: usize,
}

#[derive(Default)]
struct Rendering {
    text: String,
    line: usize,
    next_id: usize,
    candidates: Vec<BreakCandidate>,
    code_columns: std::collections::HashMap<usize, usize>,
}

impl Rendering {
    fn push(&mut self, text: &str) {
        self.line += text.matches('\n').count();
        self.text.push_str(text);
    }

    fn current_indent(&self) -> usize {
        self.text
            .rsplit('\n')
            .next()
            .unwrap_or_default()
            .chars()
            .take_while(|c| *c == ' ')
            .count()
    }

    fn trailing_comment(&mut self, comment: &str) {
        let columns = self
            .text
            .rsplit('\n')
            .next()
            .unwrap_or_default()
            .chars()
            .count();
        self.code_columns.entry(self.line).or_insert(columns);
        self.push(" ");
        self.push(comment);
    }

    fn layout(&mut self, layout: &Layout, broken: &HashSet<usize>, inside_inline: bool) {
        match layout {
            Layout::Text(text) => self.push(text),
            Layout::TrailingComment(text) => self.trailing_comment(text),
            Layout::LineBreak => {
                let indent = self.current_indent();
                self.push("\n");
                self.push(&" ".repeat(indent));
            }
            Layout::Concat(parts) => {
                for part in parts {
                    self.layout(part, broken, inside_inline);
                }
            }
            Layout::Attributes {
                blocks,
                force_block,
            } => {
                let id = self.next_id;
                self.next_id += 1;
                if *force_block || broken.contains(&id) {
                    let indent = self.current_indent();
                    self.push("[");
                    for (index, block) in blocks.iter().enumerate() {
                        if index > 0 && block.gap_blank {
                            self.push("\n");
                        }
                        for (i, comment) in block.leading.iter().enumerate() {
                            if i > 0 && comment.blank_before {
                                self.push("\n");
                            }
                            self.push("\n");
                            self.push(&" ".repeat(indent + 2));
                            self.push(&comment.text);
                        }
                        if block.blank_before_node {
                            self.push("\n");
                        }
                        if let Some(item) = &block.layout {
                            self.push("\n");
                            self.push(&" ".repeat(indent + 2));
                            self.layout(item, broken, false);
                        }
                        for comment in &block.trailing {
                            self.trailing_comment(comment);
                        }
                    }
                    self.push("\n");
                    self.push(&" ".repeat(indent));
                    self.push("]");
                } else {
                    if !inside_inline && !blocks.is_empty() {
                        self.candidates.push(BreakCandidate {
                            id,
                            line: self.line,
                        });
                    }
                    self.push("[ ");
                    for (i, block) in blocks.iter().enumerate() {
                        if i > 0 {
                            self.push(", ");
                        }
                        if let Some(item) = &block.layout {
                            self.layout(item, broken, true);
                        }
                    }
                    self.push(" ]");
                }
            }
            Layout::Shapes(items) => {
                let id = self.next_id;
                self.next_id += 1;
                if broken.contains(&id) {
                    let indent = self.current_indent();
                    for (index, item) in items.iter().enumerate() {
                        self.push("\n");
                        self.push(&" ".repeat(indent + 2));
                        self.push(item);
                        if index + 1 < items.len() {
                            self.push(",");
                        }
                    }
                } else {
                    if !inside_inline && !items.is_empty() {
                        self.candidates.push(BreakCandidate {
                            id,
                            line: self.line,
                        });
                    }
                    self.push(" ");
                    self.push(&items.join(", "));
                }
            }
            Layout::Tuple(items) => {
                let id = self.next_id;
                self.next_id += 1;
                if broken.contains(&id) {
                    let indent = self.current_indent();
                    self.push("(");
                    for (i, item) in items.iter().enumerate() {
                        self.push("\n");
                        self.push(&" ".repeat(indent + 2));
                        self.layout(item, broken, false);
                        if i + 1 < items.len() {
                            self.push(",");
                        }
                    }
                    self.push("\n");
                    self.push(&" ".repeat(indent));
                    self.push(")");
                } else {
                    if !inside_inline && !items.is_empty() {
                        self.candidates.push(BreakCandidate {
                            id,
                            line: self.line,
                        });
                    }
                    self.push("(");
                    for (i, item) in items.iter().enumerate() {
                        if i > 0 {
                            self.push(", ");
                        }
                        self.layout(item, broken, true);
                    }
                    self.push(")");
                }
            }
        }
    }
}

/// Start with the inline rendering. Break the last available construct on an
/// overlong line, render again, and stop when no overlong line can break.
/// Layout trailing comments are excluded by the recorded code-column count;
/// container trailing comments are attached after rendering.
fn render_layout(layout: &Layout, indent: usize, options: &FormatOptions) -> Vec<String> {
    let mut broken = HashSet::new();
    loop {
        let mut rendered = Rendering::default();
        rendered.push(&indent_str(indent));
        rendered.layout(layout, &broken, false);
        let candidate = options.max_line_length.and_then(|width| {
            rendered
                .text
                .split('\n')
                .enumerate()
                .find_map(|(line, text)| {
                    if rendered
                        .code_columns
                        .get(&line)
                        .copied()
                        .unwrap_or_else(|| text.chars().count())
                        <= width
                    {
                        return None;
                    }
                    rendered
                        .candidates
                        .iter()
                        .rev()
                        .find(|c| c.line == line)
                        .map(|c| c.id)
                })
        });
        match candidate {
            Some(id) => {
                broken.insert(id);
            }
            None => return rendered.text.split('\n').map(str::to_string).collect(),
        }
    }
}

/// Renders a field, tuple-field, or collection-element type, recursing through
/// tuples, arrays, maps, and optionals. Comments retain the whole type verbatim.
fn format_field_type(node: &SyntaxNode) -> Layout {
    if contains_comment(node) {
        return Layout::Text(node.text().to_string());
    }
    match node.kind() {
        SyntaxKind::PathType | SyntaxKind::StreamType => Layout::Text(tight_text(node)),
        SyntaxKind::PrimitiveType => {
            let mut out = primitive_keyword(node);
            if let Some(constraint) = child_node(node, SyntaxKind::Constraint) {
                out.push(' ');
                out.push_str(&format_constraint(&constraint));
            }
            Layout::Text(out)
        }
        SyntaxKind::OptionalType => {
            Layout::Concat(vec![field_type(node), Layout::Text("?".into())])
        }
        SyntaxKind::TupleType => Layout::Tuple(
            node.children()
                .filter(|c| c.kind() == SyntaxKind::TupleField)
                .map(|f| {
                    Layout::Concat(vec![
                        Layout::Text(format!("{}: ", child_tight(&f, SyntaxKind::Name))),
                        field_type(&f),
                    ])
                })
                .collect(),
        ),
        SyntaxKind::ArrayType => Layout::Concat(vec![
            Layout::Text("[".into()),
            field_type(node),
            Layout::Text(format!("; {}]", child_tight(node, SyntaxKind::Bound))),
        ]),
        SyntaxKind::MapType => {
            let mut types = node.children().filter(|c| is_field_type(c.kind()));
            let mut next = || {
                types
                    .next()
                    .map(|c| format_field_type(&c))
                    .unwrap_or_else(|| Layout::Text(String::new()))
            };
            Layout::Concat(vec![
                Layout::Text("[".into()),
                next(),
                Layout::Text(": ".into()),
                next(),
                Layout::Text(format!("; {}]", child_tight(node, SyntaxKind::Bound))),
            ])
        }
        _ => Layout::Text(tight_text(node)),
    }
}

// --- constraints ---------------------------------------------------------

/// Renders a `[ … ]` constraint: tight brackets, no spaces around `..`, single
/// spaces around `step` and `match`. Scalar endpoints are direct `Literal`
/// children; a length bound is a `Bound` child. A constraint carrying a comment
/// is emitted verbatim so the comment survives (the never-drop-a-comment rule).
fn format_constraint(node: &SyntaxNode) -> String {
    if contains_comment(node) {
        return node.text().to_string();
    }
    let mut out = String::new();
    for element in node.children_with_tokens() {
        match element {
            NodeOrToken::Node(n) => out.push_str(&tight_text(&n)),
            NodeOrToken::Token(t) => match t.kind() {
                SyntaxKind::LBracket => out.push('['),
                SyntaxKind::RBracket => out.push(']'),
                SyntaxKind::DotDot => out.push_str(".."),
                SyntaxKind::StepKw => push_keyword(&mut out, "step"),
                SyntaxKind::MatchKw => push_keyword(&mut out, "match"),
                _ => {}
            },
        }
    }
    out
}

/// Appends a constraint keyword with a leading space, unless it opens the
/// constraint (a bare `[match P]`), then a trailing space for its operand.
fn push_keyword(out: &mut String, keyword: &str) {
    if !out.ends_with('[') {
        out.push(' ');
    }
    out.push_str(keyword);
    out.push(' ');
}

// --- token helpers -------------------------------------------------------

/// The two-space indentation for `level`.
fn indent_str(level: usize) -> String {
    "  ".repeat(level)
}

fn is_comment(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::LineComment | SyntaxKind::BlockComment | SyntaxKind::DocComment
    )
}

/// Documentation comments share a token kind for line and block forms.
fn is_line_comment(token: &ridl_syntax::SyntaxToken) -> bool {
    is_comment(token.kind()) && token.text().starts_with("//")
}

/// The prefix of `internal` / `error` modifiers, in source order, each with a
/// trailing space. Modifiers are surface, not wire identity, so source order is
/// preserved rather than normalised.
fn modifiers_prefix(node: &SyntaxNode) -> String {
    let mut out = String::new();
    for element in node.children_with_tokens() {
        if let NodeOrToken::Token(t) = element {
            match t.kind() {
                SyntaxKind::InternalKw | SyntaxKind::ErrorKw => {
                    out.push_str(t.text());
                    out.push(' ');
                }
                _ => {}
            }
        }
    }
    out
}

/// The primitive keyword token text of a `PrimitiveType`.
fn primitive_keyword(node: &SyntaxNode) -> String {
    node.children_with_tokens()
        .filter_map(|e| e.into_token())
        .find(|t| {
            matches!(
                t.kind(),
                SyntaxKind::BooleanKw
                    | SyntaxKind::IntegerKw
                    | SyntaxKind::FloatKw
                    | SyntaxKind::StringKw
                    | SyntaxKind::BytesKw
            )
        })
        .map(|t| t.text().to_string())
        .unwrap_or_default()
}

/// The first child node of `kind`.
fn child_node(node: &SyntaxNode, kind: SyntaxKind) -> Option<SyntaxNode> {
    node.children().find(|c| c.kind() == kind)
}

/// Whether `node` has a direct child token of `kind`.
fn has_token(node: &SyntaxNode, kind: SyntaxKind) -> bool {
    node.children_with_tokens()
        .any(|e| matches!(e, NodeOrToken::Token(t) if t.kind() == kind))
}

/// The first child node of `kind`, rendered as its tight token concatenation,
/// or the empty string when absent.
fn child_tight(node: &SyntaxNode, kind: SyntaxKind) -> String {
    child_node(node, kind)
        .map(|n| tight_text(&n))
        .unwrap_or_default()
}

/// Concatenates every non-trivia token under `node`, with no separators — the
/// canonical rendering of an atom (qualified name, unit expression, literal,
/// length bound) whose parts never take internal spacing. An atom carrying a
/// comment is emitted verbatim so the comment survives.
fn tight_text(node: &SyntaxNode) -> String {
    if contains_comment(node) {
        return node.text().to_string();
    }
    node.descendants_with_tokens()
        .filter_map(|e| e.into_token())
        .filter(|t| !t.kind().is_trivia())
        .map(|t| t.text().to_string())
        .collect()
}

/// Whether any token anywhere under `node` is a comment.
fn contains_comment(node: &SyntaxNode) -> bool {
    node.descendants_with_tokens()
        .filter_map(|e| e.into_token())
        .any(|t| is_comment(t.kind()))
}

/// Whether any direct child token of `node` is a comment — a comment wedged
/// among the node's own tokens, which token-stitching synthesis would drop.
fn has_direct_comment(node: &SyntaxNode) -> bool {
    node.children_with_tokens()
        .any(|e| matches!(e, NodeOrToken::Token(t) if is_comment(t.kind())))
}

/// Whether direct comments need the single-line fallback. Brace definitions
/// (including standalone enumsets and inline services) and attribute blocks
/// handle direct comments in their own renderers.
fn is_single_line_element(node: &SyntaxNode) -> bool {
    match node.kind() {
        SyntaxKind::StructDef
        | SyntaxKind::EnumDef
        | SyntaxKind::UnionDef
        | SyntaxKind::InterfaceDef
        | SyntaxKind::SystemDef
        | SyntaxKind::ComponentDef
        | SyntaxKind::DistributionDef
        | SyntaxKind::DeploymentDef
        | SyntaxKind::MachineDef
        | SyntaxKind::AttrBlock => false,
        SyntaxKind::EnumSetDef | SyntaxKind::ServiceDef => !has_token(node, SyntaxKind::LBrace),
        _ => true,
    }
}

#[cfg(test)]
#[path = "../tests/support/invariants.rs"]
mod test_invariants;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rsdl_container_bodies_use_one_member_per_line() {
        for width in [100, 60, 40] {
            for (source, expected) in [
                (
                    "system Vehicle { Cruise, Lane, Panel, Backend, veh.diag.access }",
                    "system Vehicle {\n  Cruise\n  Lane\n  Panel\n  Backend\n  veh.diag.access\n}",
                ),
                (
                    "distribution Adas [tier=PLATFORM] { Cruise, Lane, veh.diag.access, }",
                    "distribution Adas [ tier = PLATFORM ] {\n  Cruise\n  Lane\n  veh.diag.access\n}",
                ),
                (
                    "component Panel { requires  CruiseControl, requires LaneAssist, }",
                    "component Panel {\n  requires CruiseControl\n  requires LaneAssist\n}",
                ),
                (
                    "component Solo [instances=solo] {}",
                    "component Solo [ instances = solo ] {}",
                ),
                ("system Empty {}", "system Empty {}"),
                ("distribution Empty {}", "distribution Empty {}"),
                (
                    "system S { veh . topology . Cruise . primary [linux.cpuset=(2)] }",
                    "system S {\n  veh.topology.Cruise.primary [\n    linux.cpuset = (2)\n  ]\n}",
                ),
            ] {
                // The dotted member's attribute block fits at the wider widths.
                let expected = if source.starts_with("system S") && width > 40 {
                    "system S {\n  veh.topology.Cruise.primary [ linux.cpuset = (2) ]\n}"
                } else {
                    expected
                };
                assert_profile_format(
                    &format!("package p\n{source}\n"),
                    &format!("package p\n\n{expected}\n"),
                    Profile::Rsdl,
                    &FormatOptions {
                        max_line_length: Some(width),
                    },
                );
            }
        }
    }

    #[test]
    fn rsdl_component_header_width_counts_the_opening_brace() {
        let source = "package p\ncomponent Cruise [instances=(primary,backup,),deprecated=\"use Cruise2\",rust.crate=\"cruise\",someip.serviceId=4660,linux.realtime,] { offers veh.adas.cruise }\n";
        let block = "package p\n\ncomponent Cruise [\n  instances = (primary, backup)\n  deprecated = \"use Cruise2\"\n  rust.crate = \"cruise\"\n  someip.serviceId = 4660\n  linux.realtime\n] {\n  offers veh.adas.cruise\n}\n";
        let inline = "package p\n\ncomponent Cruise [ instances = (primary, backup), deprecated = \"use Cruise2\", rust.crate = \"cruise\", someip.serviceId = 4660, linux.realtime ] {\n  offers veh.adas.cruise\n}\n";
        for (width, expected) in [(143, block), (144, inline)] {
            assert_profile_format(
                source,
                expected,
                Profile::Rsdl,
                &FormatOptions {
                    max_line_length: Some(width),
                },
            );
        }
        let empty_source = source.replace("{ offers veh.adas.cruise }", "{}");
        for (width, expected) in [
            (144, block.replace("{\n  offers veh.adas.cruise\n}", "{}")),
            (145, inline.replace("{\n  offers veh.adas.cruise\n}", "{}")),
        ] {
            assert_profile_format(
                &empty_source,
                &expected,
                Profile::Rsdl,
                &FormatOptions {
                    max_line_length: Some(width),
                },
            );
        }
    }

    #[test]
    fn rsdl_container_comments_keep_their_positions() {
        for (source, expected) in [
            (
                "component  Cruise { // brace\n offers   veh.adas.cruise\n // next\n requires LaneAssist\n}",
                "component Cruise { // brace\n  offers veh.adas.cruise\n  // next\n  requires LaneAssist\n}",
            ),
            (
                "component  Cruise /* header */ [ external ] { requires LaneAssist }",
                "component  Cruise /* header */ [ external ] {\n  requires LaneAssist\n}",
            ),
            (
                "component  Cruise // header\n{ offers veh.adas.cruise }",
                "component  Cruise // header\n{\n  offers veh.adas.cruise\n}",
            ),
            (
                "component Cruise [instances= /* value */ (primary,backup)] { requires LaneAssist }",
                "component Cruise [instances= /* value */ (primary,backup)] {\n  requires LaneAssist\n}",
            ),
            (
                "component C { offers a /* ref */ .b [external] }",
                "component C {\n  offers a /* ref */ .b [ external ]\n}",
            ),
            (
                "system S { Cruise /* line */ [external] }",
                "system S {\n  Cruise /* line */ [external]\n}",
            ),
        ] {
            for width in [100, 60, 40] {
                assert_profile_format(
                    &format!("package p\n{source}\n"),
                    &format!("package p\n\n{expected}\n"),
                    Profile::Rsdl,
                    &FormatOptions {
                        max_line_length: Some(width),
                    },
                );
            }
        }
        let comment = "x".repeat(120);
        for width in [100, 60, 40] {
            assert_profile_format(
                &format!("package p\ncomponent C [external] {{ // {comment}\n requires A\n}}\n"),
                &format!(
                    "package p\n\ncomponent C [ external ] {{ // {comment}\n  requires A\n}}\n"
                ),
                Profile::Rsdl,
                &FormatOptions {
                    max_line_length: Some(width),
                },
            );
        }
    }

    #[test]
    fn rsdl_machine_body_reuses_shared_member_lines() {
        let source = "package p\ndeployment Bench for S { machine DevBox { Cruise.primary, Panel [linux.cpuset=(2,3)], } }\n";
        let parsed = ridl_syntax::parse(source, Profile::Rsdl);
        assert!(parsed.errors().is_empty());
        let machine = parsed
            .syntax()
            .descendants()
            .find(|node| node.kind() == SyntaxKind::MachineDef)
            .unwrap();
        for width in [100, 60, 40] {
            let options = FormatOptions {
                max_line_length: Some(width),
            };
            let expected = vec![
                "  machine DevBox {",
                "    Cruise.primary",
                "    Panel [ linux.cpuset = (2, 3) ]",
                "  }",
            ];
            let actual = format_block_def(&machine, 1, "machine", &options);
            assert_eq!(actual, expected);
            let output = format!(
                "package p\ndeployment Bench for S {{\n{}\n}}\n",
                actual.join("\n")
            );
            assert_eq!(
                test_invariants::syntax_structure(source, Profile::Rsdl),
                test_invariants::syntax_structure(&output, Profile::Rsdl)
            );
            assert_eq!(
                test_invariants::content_tokens(source, Profile::Rsdl),
                test_invariants::content_tokens(&output, Profile::Rsdl)
            );
            let parsed = ridl_syntax::parse(&output, Profile::Rsdl);
            assert!(parsed.errors().is_empty());
            let machine = parsed
                .syntax()
                .descendants()
                .find(|node| node.kind() == SyntaxKind::MachineDef)
                .unwrap();
            assert_eq!(format_block_def(&machine, 1, "machine", &options), actual);
        }
    }

    #[test]
    fn rsdl_broken_system_is_left_unformatted() {
        let source = "package p\nsystem Broken { Cruise\n";
        assert!(matches!(
            format(source, Profile::Rsdl, &FormatOptions::default()),
            FormatOutcome::ParseErrors(_)
        ));
    }

    #[test]
    fn rsdl_container_goldens_keep_structure_and_comments() {
        for (source, expected) in [
            (
                include_str!("../test_data/input/component.rsdl"),
                include_str!("../test_data/formatted/component.rsdl"),
            ),
            (
                include_str!("../test_data/input/rsdl_attribute_positions.rsdl"),
                include_str!("../test_data/formatted/rsdl_attribute_positions.rsdl"),
            ),
            (
                include_str!("../test_data/input/deployment.rsdl"),
                include_str!("../test_data/formatted/deployment.rsdl"),
            ),
        ] {
            assert_profile_format(source, expected, Profile::Rsdl, &FormatOptions::default());
        }
    }

    #[test]
    fn rsdl_deployment_places_nested_machine_members_on_separate_lines() {
        let source = "package p\ndeployment Production for Vehicle {\n machine AdasHpc [labels=(ASIL_B)] {Cruise.primary, Lane, veh.diag.access}\n machine Cockpit {Cruise.backup, Panel [linux.cpuset=(2,3)]}\n machine Cloud [external] { Backend }\n}\n";
        let expected = "package p\n\ndeployment Production for Vehicle {\n  machine AdasHpc [ labels = (ASIL_B) ] {\n    Cruise.primary\n    Lane\n    veh.diag.access\n  }\n  machine Cockpit {\n    Cruise.backup\n    Panel [ linux.cpuset = (2, 3) ]\n  }\n  machine Cloud [ external ] {\n    Backend\n  }\n}\n";
        for width in [100, 60, 40] {
            let expected = if width == 40 {
                expected.replace(
                    "  machine AdasHpc [ labels = (ASIL_B) ] {",
                    "  machine AdasHpc [\n    labels = (ASIL_B)\n  ] {",
                )
            } else {
                expected.to_string()
            };
            assert_profile_format(
                source,
                &expected,
                Profile::Rsdl,
                &FormatOptions {
                    max_line_length: Some(width),
                },
            );
        }
    }

    #[test]
    fn rsdl_deployment_header_keeps_for_before_breakable_attributes() {
        let source = "package p\ndeployment  Bench for veh .topology .Vehicle [labels=(QM)] {machine DevBox {}}\n";
        let inline = "package p\n\ndeployment Bench for veh.topology.Vehicle [ labels = (QM) ] {\n  machine DevBox {}\n}\n";
        let block = "package p\n\ndeployment Bench for veh.topology.Vehicle [\n  labels = (QM)\n] {\n  machine DevBox {}\n}\n";
        for (width, expected) in [(100, inline), (61, inline), (60, block), (40, block)] {
            assert_profile_format(
                source,
                expected,
                Profile::Rsdl,
                &FormatOptions {
                    max_line_length: Some(width),
                },
            );
        }
        assert_profile_format(
            "package p\ndeployment  Empty for veh .topology .Vehicle {}\n",
            "package p\n\ndeployment Empty for veh.topology.Vehicle {}\n",
            Profile::Rsdl,
            &FormatOptions::default(),
        );
    }

    #[test]
    fn rsdl_machine_gaps_and_between_machine_comments_follow_source() {
        for (source, expected) in [
            (
                "machine  A {}, machine  B {}",
                "  machine A {}\n  machine B {}",
            ),
            (
                "machine  A {}\n\nmachine  B {}",
                "  machine A {}\n\n  machine B {}",
            ),
            (
                "machine  A {}\n// second\nmachine  B {}",
                "  machine A {}\n  // second\n  machine B {}",
            ),
            (
                "machine  A {}\n\n/// second\n\nmachine  B {}",
                "  machine A {}\n\n  /// second\n\n  machine B {}",
            ),
        ] {
            for width in [100, 60, 40] {
                assert_profile_format(
                    &format!("package p\ndeployment  D for S {{\n{source}\n}}\n"),
                    &format!("package p\n\ndeployment D for S {{\n{expected}\n}}\n"),
                    Profile::Rsdl,
                    &FormatOptions {
                        max_line_length: Some(width),
                    },
                );
            }
        }
    }

    #[test]
    fn rsdl_deployment_and_machine_header_comments_keep_their_scope() {
        for (source, expected) in [
            (
                "deployment  D for /* clause */ S [labels=(QM)] { machine  M { C } }",
                "deployment  D for /* clause */ S [labels=(QM)] {\n  machine M {\n    C\n  }\n}",
            ),
            (
                "deployment  D for S // header\n{ machine  M { C } }",
                "deployment  D for S // header\n{\n  machine M {\n    C\n  }\n}",
            ),
            (
                "deployment D for S { machine  M /* header */ [external] { C } }",
                "deployment D for S {\n  machine  M /* header */ [external] {\n    C\n  }\n}",
            ),
            (
                "deployment D for S { machine  M // header\n { C } }",
                "deployment D for S {\n  machine  M // header\n  {\n    C\n  }\n}",
            ),
            (
                "deployment  D for a /* ref */ .S [labels=(QM)] { machine M {} }",
                "deployment D for a /* ref */ .S [ labels = (QM) ] {\n  machine M {}\n}",
            ),
        ] {
            for width in [100, 60, 40] {
                let expected = if source.contains("a /* ref */ .S") && width == 40 {
                    "deployment D for a /* ref */ .S [\n  labels = (QM)\n] {\n  machine M {}\n}"
                } else {
                    expected
                };
                assert_profile_format(
                    &format!("package p\n{source}\n"),
                    &format!("package p\n\n{expected}\n"),
                    Profile::Rsdl,
                    &FormatOptions {
                        max_line_length: Some(width),
                    },
                );
            }
        }
    }

    #[test]
    fn all_seven_declaration_kinds_have_canonical_dispatch() {
        for width in [100, 60, 40] {
            assert_profile_format(
                "package p\ncomponent  C {}\nsystem  S { C }\ndistribution  Dist { C }\ndeployment  D for S { machine  M { C } }\n",
                "package p\n\ncomponent C {}\n\nsystem S {\n  C\n}\n\ndistribution Dist {\n  C\n}\n\ndeployment D for S {\n  machine M {\n    C\n  }\n}\n",
                Profile::Rsdl,
                &FormatOptions {
                    max_line_length: Some(width),
                },
            );
            assert_profile_format(
                "package p\ninterface  I {signal  s:T}\nservice  veh.named : I,\nservice  veh.inline {signal  s:T}\n",
                "package p\n\ninterface I {\n  signal s: T\n}\n\nservice veh.named: I\n\nservice veh.inline {\n  signal s: T\n}\n",
                Profile::Ridl,
                &FormatOptions {
                    max_line_length: Some(width),
                },
            );
        }
    }

    #[test]
    fn rsdl_broken_deployment_is_left_unformatted() {
        assert!(matches!(
            format(
                "package p\ndeployment Broken {}\n",
                Profile::Rsdl,
                &FormatOptions::default()
            ),
            FormatOutcome::ParseErrors(_)
        ));
    }

    #[test]
    fn default_options_use_a_hundred_column_limit() {
        assert_eq!(FormatOptions::default().max_line_length, Some(100));
    }

    fn formatted(input: &str) -> String {
        match format(input, Profile::Typl, &FormatOptions::default()) {
            FormatOutcome::Formatted(s) => s,
            FormatOutcome::ParseErrors(errors) => {
                panic!("expected a formatted result, got parse errors: {errors:?}")
            }
        }
    }

    #[test]
    fn broken_input_returns_parse_errors_untouched() {
        // A missing `package` is FORM-104; a broken file is never rewritten.
        let outcome = format("type 123 :: [\n", Profile::Typl, &FormatOptions::default());
        let FormatOutcome::ParseErrors(errors) = outcome else {
            panic!("expected parse errors for broken input");
        };
        assert!(!errors.is_empty());
        for (input, profile) in [
            ("package p\ninterface Broken {\n", Profile::Ridl),
            (
                "package p\ncomponent Broken { requires A B }\n",
                Profile::Rsdl,
            ),
        ] {
            assert!(matches!(
                format(input, profile, &FormatOptions::default()),
                FormatOutcome::ParseErrors(_)
            ));
        }
    }

    #[test]
    fn missing_package_is_a_parse_error_not_a_format() {
        assert!(matches!(
            format(
                "type Speed: km/h\n",
                Profile::Typl,
                &FormatOptions::default()
            ),
            FormatOutcome::ParseErrors(_)
        ));
    }

    #[test]
    fn tight_colon_replaces_alignment() {
        let input = "package p\ntype Speed       : km/h [0.0..250.0 step 0.5] = 0.0\n";
        assert_eq!(
            formatted(input),
            "package p\n\ntype Speed: km/h [0.0..250.0 step 0.5] = 0.0\n",
        );
    }

    #[test]
    fn separator_commas_become_one_member_per_line() {
        let input = "package p\nenum Warning { LOW_FUEL = 0, CHECK_ENGINE = 1, }\n";
        assert_eq!(
            formatted(input),
            "package p\n\nenum Warning {\n  LOW_FUEL = 0\n  CHECK_ENGINE = 1\n}\n",
        );
    }

    #[test]
    fn repeated_separator_commas_collapse() {
        let input = "package p\nenum E { A = 0,,, B = 1 }\n";
        assert_eq!(
            formatted(input),
            "package p\n\nenum E {\n  A = 0\n  B = 1\n}\n",
        );
    }

    #[test]
    fn inline_trailing_comment_stays_on_its_line() {
        let input = "package p\nstruct S {\n  a : A   // note\n  b : B\n}\n";
        assert_eq!(
            formatted(input),
            "package p\n\nstruct S {\n  a: A // note\n  b: B\n}\n",
        );
    }

    #[test]
    fn leading_doc_comment_anchors_to_its_definition() {
        let input = "package p\n/// The speed.\ntype Speed: km/h [0.0..250.0]\n";
        assert_eq!(
            formatted(input),
            "package p\n\n/// The speed.\ntype Speed: km/h [0.0..250.0]\n",
        );
    }

    #[test]
    fn collections_and_maps_use_semicolon_space_and_tight_colon() {
        let input = "package p\nstruct S {\n  a : [Speed; 8]\n  b : [Label : Name; 0..32]\n}\n";
        assert_eq!(
            formatted(input),
            "package p\n\nstruct S {\n  a: [Speed; 8]\n  b: [Label: Name; 0..32]\n}\n",
        );
    }

    #[test]
    fn package_and_imports_are_a_contiguous_header() {
        let input = "package p\n\nimport a.B\nimport c.D\n\ntype X: integer [0..1]\n";
        assert_eq!(
            formatted(input),
            "package p\nimport a.B\nimport c.D\n\ntype X: integer [0..1]\n",
        );
    }

    #[test]
    fn source_order_is_never_changed() {
        // `Zebra` is declared before `Alpha`; the formatter must not reorder.
        let input = "package p\ntype Zebra: integer [0..1]\ntype Alpha: integer [0..1]\n";
        assert_eq!(
            formatted(input),
            "package p\n\ntype Zebra: integer [0..1]\n\ntype Alpha: integer [0..1]\n",
        );
        for (input, profile) in [
            (
                "package p\n\ninterface I {\n  signal zebra: A\n  signal alpha: B\n}\n",
                Profile::Ridl,
            ),
            (
                "package p\n\nsystem S {\n  Zebra\n  Alpha\n}\n",
                Profile::Rsdl,
            ),
        ] {
            assert_eq!(
                format(input, profile, &FormatOptions::default()),
                FormatOutcome::Formatted(input.to_string())
            );
        }
    }

    #[test]
    fn already_tight_output_is_idempotent() {
        let input = "package p\n\ntype Speed: km/h [0.0..250.0 step 0.5]\n";
        let once = formatted(input);
        assert_eq!(once, input);
        assert_eq!(formatted(&once), once);
    }

    #[test]
    fn empty_block_body_collapses_to_braces() {
        let input = "package p\nstruct Empty {\n}\n";
        assert_eq!(formatted(input), "package p\n\nstruct Empty {}\n");
    }

    #[test]
    fn block_comment_inside_a_constraint_is_preserved() {
        let input = "package p\ntype Frame: bytes [/* fixed */ 8]\n";
        let out = formatted(input);
        assert!(
            out.contains("/* fixed */"),
            "constraint comment dropped: {out}"
        );
        assert_eq!(out, "package p\n\ntype Frame: bytes [/* fixed */ 8]\n");
        assert_eq!(formatted(&out), out, "not a fixed point");
    }

    #[test]
    fn block_comment_inside_an_array_type_is_preserved() {
        let input = "package p\nstruct S {\n  readings : [Speed; /* per wheel */ 8]\n}\n";
        let out = formatted(input);
        assert!(
            out.contains("/* per wheel */"),
            "array comment dropped: {out}"
        );
        assert_eq!(
            out,
            "package p\n\nstruct S {\n  readings: [Speed; /* per wheel */ 8]\n}\n",
        );
        assert_eq!(formatted(&out), out, "not a fixed point");
    }

    #[test]
    fn block_comment_inside_a_tuple_type_is_preserved() {
        let input = "package p\nstruct S {\n  range : (min: Speed, /* hi */ max: Speed)\n}\n";
        let out = formatted(input);
        assert!(out.contains("/* hi */"), "tuple comment dropped: {out}");
        assert_eq!(
            out,
            "package p\n\nstruct S {\n  range: (min: Speed, /* hi */ max: Speed)\n}\n",
        );
        assert_eq!(formatted(&out), out, "not a fixed point");
    }

    #[test]
    fn line_comment_inside_a_construct_is_preserved() {
        // A line comment inside brackets runs to the end of its line, so the
        // whole array type is emitted verbatim to keep the comment in place.
        let input = "package p\nstruct S {\n  readings : [Speed; // per wheel\n  8]\n}\n";
        let out = formatted(input);
        assert!(out.contains("// per wheel"), "line comment dropped: {out}");
        assert_eq!(
            out,
            "package p\n\nstruct S {\n  readings: [Speed; // per wheel\n  8]\n}\n",
        );
        assert_eq!(formatted(&out), out, "not a fixed point");
    }

    #[test]
    fn comment_among_a_declarations_own_tokens_is_preserved() {
        // A comment between a name and its colon would be dropped by token
        // stitching; the declaration is emitted verbatim instead.
        let input = "package p\nconst X /* note */ : integer = 5\n";
        let out = formatted(input);
        assert!(out.contains("/* note */"), "spine comment dropped: {out}");
        assert_eq!(out, "package p\n\nconst X /* note */ : integer = 5\n");
        assert_eq!(formatted(&out), out, "not a fixed point");
    }

    #[test]
    fn comment_on_the_brace_line_stays_on_the_brace_line() {
        let input = "package p\nunion R {   // result union\n  ok : A\n  err : B\n}\n";
        assert_eq!(
            formatted(input),
            "package p\n\nunion R { // result union\n  ok: A\n  err: B\n}\n",
        );
    }

    fn assert_ridl_member(input: &str, expected: &str) {
        assert_profile_format(
            &format!("package p\ninterface I {{ {input} }}\n"),
            &format!("package p\n\ninterface I {{\n  {expected}\n}}\n"),
            Profile::Ridl,
            &FormatOptions::default(),
        );
    }

    #[test]
    fn ridl_struct_preserves_direct_and_array_stream_types() {
        assert_profile_format(
            "package p\nstruct Streams { a: <T> b: [<T>; 1..2] }\n",
            "package p\n\nstruct Streams {\n  a: <T>\n  b: [<T>; 1..2]\n}\n",
            Profile::Ridl,
            &FormatOptions::default(),
        );
    }

    #[test]
    fn ridl_streams_render_in_nested_collection_and_optional_positions() {
        assert_profile_format(
            "package p\nstruct S { a : < veh.T >? b : [<K>:<bytes>;1 .. 2] c : (a:<string>,b:[<T>?;2]) }\n",
            "package p\n\nstruct S {\n  a: <veh.T>?\n  b: [<K>: <bytes>; 1..2]\n  c: (a: <string>, b: [<T>?; 2])\n}\n",
            Profile::Ridl,
            &FormatOptions::default(),
        );
    }

    #[test]
    fn ridl_stream_comment_keeps_the_type_verbatim() {
        assert_profile_format(
            "package p\nstruct S { a : < T /* element */ > }\n",
            "package p\n\nstruct S {\n  a: < T /* element */ >\n}\n",
            Profile::Ridl,
            &FormatOptions::default(),
        );
    }

    #[test]
    fn ridl_interface_header_comment_is_preserved() {
        assert_profile_format(
            "package p\ninterface   I /* header */ { signal s : T }\n",
            "package p\n\ninterface   I /* header */ {\n  signal s: T\n}\n",
            Profile::Ridl,
            &FormatOptions::default(),
        );
    }

    #[test]
    fn ridl_signal_payload_init_and_four_timing_forms() {
        for (input, expected) in [
            (
                "signal  speed : Speed = LIMIT @ 10ms",
                "signal speed: Speed = LIMIT @10ms",
            ),
            (
                "signal raw : <SensorFrame> @[ 20ms .. 100ms ]",
                "signal raw: <SensorFrame> @[20ms..100ms]",
            ),
            (
                "signal maximum : integer[0..10] @[ .. 5s ]",
                "signal maximum: integer [0..10] @[..5s]",
            ),
            (
                "signal optional : Speed? @[20ms .. ]",
                "signal optional: Speed? @[20ms..]",
            ),
        ] {
            assert_ridl_member(input, expected);
        }
    }

    #[test]
    fn ridl_event_keeps_lenient_stream_and_init_slots() {
        assert_ridl_member(
            "event  calibrated : <bytes> = DEFAULT_CAL @[ 1ms .. 2ms ]",
            "event calibrated: <bytes> = DEFAULT_CAL @[1ms..2ms]",
        );
    }

    #[test]
    fn ridl_fixed_keeps_lenient_stream_init_and_timing_slots() {
        assert_ridl_member(
            "fixed  region : <string> = REGION_EU @ 1s",
            "fixed region: <string> = REGION_EU @1s",
        );
    }

    #[test]
    fn ridl_command_parameters_optional_return_and_timing() {
        assert_ridl_member(
            "command  upload ( data : <FwBlock>, span : (min:A,max:B), ) : Ack @[ .. 1s ]",
            "command upload(data: <FwBlock>, span: (min: A, max: B)): Ack @[..1s]",
        );
        assert_ridl_member("command  reset ( ) @ 50ms", "command reset() @50ms");
    }

    #[test]
    fn ridl_query_four_return_shapes() {
        for (input, expected) in [
            ("query a ( ) : Speed", "query a(): Speed"),
            (
                "query b( ): (min : Speed,max : Speed,)",
                "query b(): (min: Speed, max: Speed)",
            ),
            ("query c( ): < veh.LogLine >", "query c(): <veh.LogLine>"),
            (
                "query d( ) : CalReport|CalError",
                "query d(): CalReport | CalError",
            ),
        ] {
            assert_ridl_member(input, expected);
        }
    }

    #[test]
    fn ridl_reserved_members_keep_order_and_comments() {
        assert_profile_format(
            "package p\ninterface I { reserved  legacy, // first\n reserved 3 }\n",
            "package p\n\ninterface I {\n  reserved legacy // first\n  reserved 3\n}\n",
            Profile::Ridl,
            &FormatOptions::default(),
        );
    }

    #[test]
    fn ridl_empty_internal_interface_and_between_member_comments() {
        assert_profile_format(
            "package p\ninternal   interface Hidden { }\ninterface I { // body\n signal  zebra : A,\n\n // next\n event  alpha : B // trailing\n // end\n}\n",
            "package p\n\ninternal interface Hidden {}\n\ninterface I { // body\n  signal zebra: A\n\n  // next\n  event alpha: B // trailing\n  // end\n}\n",
            Profile::Ridl,
            &FormatOptions::default(),
        );
    }

    #[test]
    fn ridl_parameter_return_and_timing_comments_remain_verbatim() {
        assert_ridl_member(
            "command upload(data : A, /* preserve */ more:B,)",
            "command upload(data : A, /* preserve */ more:B,)",
        );
        assert_ridl_member(
            "command c(a:A, // parameter\n b : B)",
            "command c(a:A, // parameter\n b : B)",
        );
        assert_ridl_member(
            "query q(): (a:A, /* return */ b : B)",
            "query q(): (a:A, /* return */ b : B)",
        );
        assert_ridl_member(
            "signal s:T @[1ms /* timing */ .. 2ms]",
            "signal s: T @[1ms /* timing */ .. 2ms]",
        );
    }

    #[test]
    fn ridl_initializer_comment_is_preserved() {
        assert_ridl_member(
            "signal  s : T = /* initializer */ DEFAULT",
            "signal s: T = /* initializer */ DEFAULT",
        );
    }

    #[test]
    fn ridl_attribute_member_uses_timing_then_predicate_block() {
        assert_ridl_member(
            "query  q ( ) : T [ require result>0 ] @ 10ms",
            "query q(): T @10ms [\n    require result > 0\n  ]",
        );
    }

    #[test]
    fn ridl_inline_attribute_padding_assignments_and_flags() {
        assert_ridl_member(
            "signal  target : T [seed=LIMIT,persist,]",
            "signal target: T [ seed = LIMIT, persist ]",
        );
    }

    #[test]
    fn ridl_annotation_pair_has_one_canonical_order() {
        for source in [
            "query q():T [ persist ] @[ .. 5s ]",
            "query q():T @[ .. 5s ] [ persist ]",
        ] {
            assert_ridl_member(source, "query q(): T @[..5s] [ persist ]");
        }
    }

    #[test]
    fn ridl_inline_annotation_comments_stay_with_the_preceding_annotation() {
        for (source, expected) in [
            (
                "query q(): T [persist] /* note */ @10ms",
                "query q(): T @10ms [ persist ] /* note */",
            ),
            (
                "query q(): T @10ms /* note */ [persist]",
                "query q(): T @10ms /* note */ [ persist ]",
            ),
            (
                "query q(): T [persist] // note\n @10ms",
                "query q(): T @10ms [ persist ] // note",
            ),
            (
                "query q(): T @10ms // note\n [persist]",
                "query q(): T @10ms // note\n  [ persist ]",
            ),
            (
                "query q(): T [require result>0] /* note */ @10ms",
                "query q(): T @10ms [\n    require result > 0\n  ] /* note */",
            ),
        ] {
            assert_ridl_member(source, expected);
        }
    }

    #[test]
    fn ridl_colliding_annotation_line_comments_keep_the_member_verbatim() {
        for width in [100, 60, 40] {
            for prefix in [
                "signal  s : T",
                "event  e : T",
                "fixed  f : T = 1",
                "command  c ( ) : T",
                "query  q ( ):T",
            ] {
                for annotation in ["// attribute", "/// attribute"] {
                    for trailing in ["// member", "/* member */", "/* member\nmore */"] {
                        let member =
                            format!("{prefix} [persist] {annotation}\n  @ 10ms {trailing}");
                        assert_profile_format(
                            &format!("package p\ninterface I {{\n  {member}\n}}\n"),
                            &format!("package p\n\ninterface I {{\n  {member}\n}}\n"),
                            Profile::Ridl,
                            &FormatOptions {
                                max_line_length: Some(width),
                            },
                        );
                    }
                }
            }
        }
        assert_profile_format(
            "package p\ninterface I {\n  query  q():T [require ready] // attribute\n  @ 10ms // member\n}\n",
            "package p\n\ninterface I {\n  query  q():T [require ready] // attribute\n  @ 10ms // member\n}\n",
            Profile::Ridl,
            &FormatOptions::default(),
        );
    }

    #[test]
    fn ridl_noncolliding_annotation_comments_still_normalize() {
        for comment in ["/* attribute */", "/** attribute */"] {
            assert_profile_format(
                &format!(
                    "package p\ninterface I {{\n  query  q():T [persist] {comment} @ 10ms // member\n}}\n"
                ),
                &format!(
                    "package p\n\ninterface I {{\n  query q(): T @10ms [ persist ] {comment} // member\n}}\n"
                ),
                Profile::Ridl,
                &FormatOptions::default(),
            );
        }
        assert_profile_format(
            "package p\ninterface I {\n  query  q():T @ 10ms // timing\n  [persist] // member\n}\n",
            "package p\n\ninterface I {\n  query q(): T @10ms // timing\n  [ persist ] // member\n}\n",
            Profile::Ridl,
            &FormatOptions::default(),
        );
        assert_profile_format(
            "package p\ninterface I {\n  query  q():T @ 10ms /** timing */ [persist] // member\n}\n",
            "package p\n\ninterface I {\n  query q(): T @10ms /** timing */ [ persist ] // member\n}\n",
            Profile::Ridl,
            &FormatOptions::default(),
        );
    }

    #[test]
    fn review_timing_doc_line_comment_keeps_attributes_on_the_next_line() {
        for width in [100, 60, 40] {
            assert_profile_format(
                "package p\ninterface I {\n  query  q():T @ 10ms /// timing\n  [persist] // member\n}\n",
                "package p\n\ninterface I {\n  query q(): T @10ms /// timing\n  [ persist ] // member\n}\n",
                Profile::Ridl,
                &FormatOptions {
                    max_line_length: Some(width),
                },
            );
        }
    }

    #[test]
    fn review_inline_service_colliding_comments_keep_the_member_verbatim() {
        for width in [100, 60, 40] {
            for annotation in ["// attribute", "/// attribute"] {
                for trailing in ["// member", "/* member */", "/* member\nmore */"] {
                    assert_profile_format(
                        &format!(
                            "package p\nservice veh.control {{\n  query  q():T [persist] {annotation}\n  @ 10ms {trailing}\n}}\n"
                        ),
                        &format!(
                            "package p\n\nservice veh.control {{\n  query  q():T [persist] {annotation}\n  @ 10ms {trailing}\n}}\n"
                        ),
                        Profile::Ridl,
                        &FormatOptions {
                            max_line_length: Some(width),
                        },
                    );
                }
            }
        }
    }

    #[test]
    fn review_inline_service_noncolliding_comments_still_normalize() {
        for width in [100, 60, 40] {
            for comment in ["/* attribute */", "/** attribute */"] {
                assert_profile_format(
                    &format!(
                        "package p\nservice veh.control {{\n  query  q():T [persist] {comment} @ 10ms // member\n}}\n"
                    ),
                    &format!(
                        "package p\n\nservice veh.control {{\n  query q(): T @10ms [ persist ] {comment} // member\n}}\n"
                    ),
                    Profile::Ridl,
                    &FormatOptions {
                        max_line_length: Some(width),
                    },
                );
            }
        }
    }

    #[test]
    fn ridl_moved_annotation_comment_does_not_force_attribute_breaking() {
        let comment = "x".repeat(120);
        assert_profile_format(
            &format!("package p\ninterface I {{ query q(): T [persist] /* {comment} */ @10ms }}\n"),
            &format!(
                "package p\n\ninterface I {{\n  query q(): T @10ms [ persist ] /* {comment} */\n}}\n"
            ),
            Profile::Ridl,
            &FormatOptions {
                max_line_length: Some(40),
            },
        );
    }

    #[test]
    fn ridl_other_direct_member_comments_remain_verbatim() {
        assert_ridl_member(
            "query q /* name */ ():T [persist] @10ms",
            "query q /* name */ ():T [persist] @10ms",
        );
        assert_ridl_member(
            "query q():T [persist]\n /* standalone */ @10ms",
            "query q():T [persist]\n /* standalone */ @10ms",
        );
    }

    #[test]
    fn ridl_attribute_values_include_nested_and_empty_lists() {
        assert_ridl_member(
            "query q():T [labels=(A,(B,C,),(),),persist]",
            "query q(): T [ labels = (A, (B, C), ()), persist ]",
        );
    }

    #[test]
    fn ridl_predicates_space_every_operator_and_preserve_parentheses() {
        assert_ridl_member(
            "command set(position:P)[require position!=GearPosition.PARK||currentSpeed==0.0]",
            "command set(position: P) [\n    require position != GearPosition.PARK || currentSpeed == 0.0\n  ]",
        );
        assert_ridl_member(
            "query q():T[require (!engaged&&((a+b*c-d/e%f)>=-1.0))||x==y ensure (result).min+1<=MAX&&status!=State.BAD&&v>0&&w<9]",
            "query q(): T [\n    require (!engaged && ((a + b * c - d / e % f) >= -1.0)) || x == y\n    ensure (result).min + 1 <= MAX && status != State.BAD && v > 0 && w < 9\n  ]",
        );
    }

    #[test]
    fn ridl_predicate_expression_is_unbreakable() {
        let name = "A".repeat(120);
        assert_ridl_member(
            &format!("query q():T [require {name}>0]"),
            &format!("query q(): T [\n    require {name} > 0\n  ]"),
        );
    }

    #[test]
    fn ridl_block_attribute_comments_follow_the_container_rules() {
        assert_ridl_member(
            "query q():T [require x>0, // trailing\n\n // next\n ensure x>=0\n // end\n]",
            "query q(): T [\n    require x > 0 // trailing\n\n    // next\n    ensure x >= 0\n    // end\n  ]",
        );
    }

    #[test]
    fn ridl_comment_inside_an_attribute_keeps_that_attribute_verbatim() {
        assert_ridl_member(
            "query q():T [require  x /* predicate */ >0 ensure result>=0]",
            "query q(): T [\n    require  x /* predicate */ >0\n    ensure result >= 0\n  ]",
        );
    }

    #[test]
    fn ridl_inline_attribute_and_value_comments_remain_verbatim() {
        assert_ridl_member(
            "query q():T [persist, /* block */ labels=(A,B,),]",
            "query q(): T [persist, /* block */ labels=(A,B,),]",
        );
        assert_ridl_member(
            "query q():T [labels=(A, // value\n B)]",
            "query q(): T [labels=(A, // value\n B)]",
        );
    }

    #[test]
    fn ridl_inline_attribute_width_boundaries() {
        for width in [100, 60] {
            let options = if width == 100 {
                FormatOptions::default()
            } else {
                FormatOptions {
                    max_line_length: Some(width),
                }
            };
            for columns in [width - 1, width, width + 1] {
                let name = "a".repeat(columns - "  signal s: T [  ]".chars().count());
                let inline = format!("  signal s: T [ {name} ]");
                assert_eq!(inline.chars().count(), columns);
                let member = if columns <= width {
                    inline.clone()
                } else {
                    format!("  signal s: T [\n    {name}\n  ]")
                };
                assert_profile_format(
                    &format!("package p\ninterface I {{\n{inline}\n}}\n"),
                    &format!("package p\n\ninterface I {{\n{member}\n}}\n"),
                    Profile::Ridl,
                    &options,
                );
            }
        }
    }

    #[test]
    fn ridl_attribute_value_list_width_boundaries() {
        for width in [100, 60] {
            let options = if width == 100 {
                FormatOptions::default()
            } else {
                FormatOptions {
                    max_line_length: Some(width),
                }
            };
            for columns in [width - 1, width, width + 1] {
                let name = "A".repeat(columns - "    labels = (, B)".chars().count());
                let inline = format!("    labels = ({name}, B)");
                assert_eq!(inline.chars().count(), columns);
                let value = if columns <= width {
                    inline.clone()
                } else {
                    format!("    labels = (\n      {name},\n      B\n    )")
                };
                assert_profile_format(
                    &format!(
                        "package p\ninterface I {{\n  query q():T [require ready\n{inline}\n]\n}}\n"
                    ),
                    &format!(
                        "package p\n\ninterface I {{\n  query q(): T [\n    require ready\n{value}\n  ]\n}}\n"
                    ),
                    Profile::Ridl,
                    &options,
                );
            }
        }
    }

    #[test]
    fn ridl_nested_attribute_value_lists_break_outer_then_inner() {
        let name = "A".repeat(40);
        assert_profile_format(
            &format!(
                "package p\ninterface I {{ query q():T[require ready labels=(({name},B),C)] }}\n"
            ),
            &format!(
                "package p\n\ninterface I {{\n  query q(): T [\n    require ready\n    labels = (\n      (\n        {name},\n        B\n      ),\n      C\n    )\n  ]\n}}\n"
            ),
            Profile::Ridl,
            &FormatOptions {
                max_line_length: Some(40),
            },
        );
    }

    #[test]
    fn ridl_full_width_example_breaks_attributes_then_return_and_stops() {
        assert_profile_format(
            "package p\ninterface I { query getSpeedHistory(window: Duration, mode: Mode): (min: Speed, max: Speed, avg: Speed) @[..100ms] [ labels = (A, B) ] }\n",
            "package p\n\ninterface I {\n  query getSpeedHistory(window: Duration, mode: Mode): (\n    min: Speed,\n    max: Speed,\n    avg: Speed\n  ) @[..100ms] [\n    labels = (A, B)\n  ]\n}\n",
            Profile::Ridl,
            &FormatOptions {
                max_line_length: Some(60),
            },
        );
    }

    #[test]
    fn ridl_attribute_trailing_comments_do_not_force_value_breaks() {
        let comment = "x".repeat(120);
        assert_profile_format(
            &format!(
                "package p\ninterface I {{ query q():T [require ready\nlabels=(A,B) // {comment}\n] }}\n"
            ),
            &format!(
                "package p\n\ninterface I {{\n  query q(): T [\n    require ready\n    labels = (A, B) // {comment}\n  ]\n}}\n"
            ),
            Profile::Ridl,
            &FormatOptions {
                max_line_length: Some(40),
            },
        );
    }

    #[test]
    fn ridl_unlimited_width_keeps_attribute_and_value_lists_inline() {
        let name = "A".repeat(180);
        assert_profile_format(
            &format!("package p\ninterface I {{ query q():T[labels=({name},B),persist] }}\n"),
            &format!(
                "package p\n\ninterface I {{\n  query q(): T [ labels = ({name}, B), persist ]\n}}\n"
            ),
            Profile::Ridl,
            &FormatOptions {
                max_line_length: None,
            },
        );
    }

    #[test]
    fn ridl_attribute_width_one_keeps_the_tree_and_fixed_point() {
        assert_profile_format(
            "package p\ninterface I { query q():T[labels=(A,B)] }\n",
            "package p\n\ninterface I {\n  query q(): T [\n    labels = (\n      A,\n      B\n    )\n  ]\n}\n",
            Profile::Ridl,
            &FormatOptions {
                max_line_length: Some(1),
            },
        );
    }

    #[test]
    fn rsdl_attribute_renderer_tightens_dotted_keys_before_body_routing() {
        let input = "package p\ncomponent C [ linux . realtime, linux . cpuset=(2,3,), ] {}\n";
        let expected_attributes = "[ linux.realtime, linux.cpuset = (2, 3) ]";
        let render = |source: &str| {
            let parse = ridl_syntax::parse(source, Profile::Rsdl);
            assert!(parse.errors().is_empty(), "{:?}", parse.errors());
            let attributes = parse
                .syntax()
                .descendants()
                .find(|n| n.kind() == SyntaxKind::AttrBlock)
                .unwrap();
            format_element(&attributes, 0, &FormatOptions::default()).join("\n")
        };
        assert_eq!(render(input), expected_attributes);
        let expected = format!("package p\ncomponent C {expected_attributes} {{}}\n");
        assert_eq!(
            render(&expected),
            expected_attributes,
            "attribute fixed point"
        );
        assert_eq!(
            crate::test_invariants::syntax_structure(input, Profile::Rsdl),
            crate::test_invariants::syntax_structure(&expected, Profile::Rsdl)
        );
        assert_eq!(
            crate::test_invariants::content_tokens(input, Profile::Rsdl),
            crate::test_invariants::content_tokens(&expected, Profile::Rsdl)
        );
    }

    #[test]
    fn review_header_line_comments_keep_the_opening_brace_on_a_new_line() {
        for header in ["interface I", "service p.s"] {
            assert_profile_format(
                &format!("package p\n{header} /** header */ {{ signal s:T }}\n"),
                &format!("package p\n\n{header} /** header */ {{\n  signal s: T\n}}\n"),
                Profile::Ridl,
                &FormatOptions::default(),
            );
            assert_profile_format(
                &format!("package p\n{header} // header\n{{ signal s:T }}\n"),
                &format!("package p\n\n{header} // header\n{{\n  signal s: T\n}}\n"),
                Profile::Ridl,
                &FormatOptions::default(),
            );
            assert_profile_format(
                &format!("package p\n{header} /// header\n{{}}\n"),
                &format!("package p\n\n{header} /// header\n{{}}\n"),
                Profile::Ridl,
                &FormatOptions::default(),
            );
        }
        assert_profile_format(
            "package p\nstruct S // header\n{ x:integer }\n",
            "package p\n\nstruct S // header\n{\n  x: integer\n}\n",
            Profile::Typl,
            &FormatOptions::default(),
        );
    }

    #[test]
    fn review_separator_comments_remain_separate_in_bodies_and_attributes() {
        for header in ["interface I", "service p.s"] {
            assert_profile_format(
                &format!(
                    "package p\n{header} {{ signal a:T // first\n , /* second\nthird */\n signal b:U }}\n"
                ),
                &format!(
                    "package p\n\n{header} {{\n  signal a: T // first\n  /* second\nthird */\n  signal b: U\n}}\n"
                ),
                Profile::Ridl,
                &FormatOptions::default(),
            );
        }
        assert_ridl_member(
            "query q():T [require ready // first\n , /* second\nthird */\n ensure result]",
            "query q(): T [\n    require ready // first\n    /* second\nthird */\n    ensure result\n  ]",
        );
    }

    #[test]
    fn review_separator_lines_preserve_only_source_blank_lines() {
        for (separator, gap) in [("\n,\n", ""), ("\n\n,\n", "\n"), ("\n,\n\n", "\n")] {
            assert_ridl_member(
                &format!("query q():T [require ready{separator}ensure result]"),
                &format!("query q(): T [\n    require ready\n{gap}    ensure result\n  ]"),
            );
        }
        for separator in ["\n\n,", "\n,\n\n"] {
            assert_ridl_member(
                &format!("query q():T [require ready{separator} /* next */\nensure result]"),
                "query q(): T [\n    require ready\n\n    /* next */\n    ensure result\n  ]",
            );
        }
    }

    #[test]
    fn review_ensure_alone_forces_block_layout() {
        assert_ridl_member(
            "query q():T [ensure result>0]",
            "query q(): T [\n    ensure result > 0\n  ]",
        );
    }

    #[test]
    fn review_commented_value_list_in_a_predicate_block_is_preserved() {
        assert_ridl_member(
            "query q():T [require ready labels=(A, // value\n B)]",
            "query q(): T [\n    require ready\n    labels=(A, // value\n B)\n  ]",
        );
    }

    #[test]
    fn review_commented_parameter_and_return_fallback_is_limited_to_the_subtree() {
        assert_ridl_member(
            "query  q(a:A, /* parameter */ b : B) : T [persist] @ 10ms",
            "query q(a:A, /* parameter */ b : B): T @10ms [ persist ]",
        );
        assert_ridl_member(
            "query  q(): (a:A, /* return */ b : B) [persist] @ 10ms",
            "query q(): (a:A, /* return */ b : B) @10ms [ persist ]",
        );
    }

    #[test]
    fn review_multiline_annotation_comment_keeps_the_whole_member_verbatim() {
        assert_ridl_member(
            "query  q():T [persist] /* two\n lines */ @ 10ms",
            "query  q():T [persist] /* two\n lines */ @ 10ms",
        );
    }

    #[test]
    fn ridl_broken_attribute_block_returns_parse_errors() {
        assert!(matches!(
            format(
                "package p\ninterface I { query q():T [ require ] }\n",
                Profile::Ridl,
                &FormatOptions::default()
            ),
            FormatOutcome::ParseErrors(_)
        ));
    }

    #[test]
    fn ridl_broken_interface_returns_parse_errors() {
        assert!(matches!(
            format(
                "package p\ninterface I { signal s: T\n",
                Profile::Ridl,
                &FormatOptions::default()
            ),
            FormatOutcome::ParseErrors(_)
        ));
    }

    #[test]
    fn ridl_broken_services_return_parse_errors() {
        for input in [
            "package p\nservice p.s: First Second\n",
            "package p\nservice p.s: \n",
            "package p\nservice p.s { signal s: T\n",
        ] {
            let parsed = ridl_syntax::parse(input, Profile::Ridl);
            assert!(!parsed.errors().is_empty(), "the fixture must be malformed");
            assert_eq!(
                format(input, Profile::Ridl, &FormatOptions::default()),
                FormatOutcome::ParseErrors(parsed.errors().to_vec()),
                "malformed services retain the original diagnostics",
            );
        }
    }

    #[test]
    fn ridl_named_services_keep_required_commas_and_remove_trailing_commas() {
        for source in [
            "service veh.body.doors : DoorControl, DiagBlock",
            "service veh.body.doors : DoorControl, DiagBlock,",
        ] {
            assert_profile_format(
                &format!("package p\n{source}\n"),
                "package p\n\nservice veh.body.doors: DoorControl, DiagBlock\n",
                Profile::Ridl,
                &FormatOptions::default(),
            );
        }
    }

    #[test]
    fn ridl_named_service_shape_list_breaks_after_the_colon() {
        for source in [
            "service veh.body.composite : DoorControl, MotorControl",
            "service veh.body.composite : DoorControl, MotorControl,",
        ] {
            assert_profile_format(
                &format!("package p\n{source} // shapes\n"),
                "package p\n\nservice veh.body.composite:\n  DoorControl,\n  MotorControl // shapes\n",
                Profile::Ridl,
                &FormatOptions {
                    max_line_length: Some(40),
                },
            );
        }
    }

    #[test]
    fn ridl_service_shape_list_obeys_the_exact_width_and_off() {
        let inline = "service p.s: Alpha, Beta";
        for width in [Some(inline.chars().count()), None] {
            assert_profile_format(
                &format!("package p\n{inline}, // trailing comment beyond the width\n"),
                &format!("package p\n\n{inline} // trailing comment beyond the width\n"),
                Profile::Ridl,
                &FormatOptions {
                    max_line_length: width,
                },
            );
        }
        assert_profile_format(
            &format!("package p\n{inline}\n"),
            "package p\n\nservice p.s:\n  Alpha,\n  Beta\n",
            Profile::Ridl,
            &FormatOptions {
                max_line_length: Some(inline.chars().count() - 1),
            },
        );
    }

    #[test]
    fn ridl_service_shape_list_with_a_comment_remains_verbatim() {
        for source in [
            "service veh.body.doors : DoorControl, /* shape */ DiagBlock,",
            "service veh.body.doors : DoorControl, // shape\n DiagBlock,",
            "service veh.body.doors : veh /* path */ . DoorControl, DiagBlock,",
        ] {
            assert_profile_format(
                &format!("package p\n{source}\n"),
                &format!("package p\n\n{source}\n"),
                Profile::Ridl,
                &FormatOptions {
                    max_line_length: Some(40),
                },
            );
        }
    }

    #[test]
    fn ridl_inline_service_reuses_member_layout_and_comments() {
        for comma in ["", ","] {
            assert_profile_format(
                &format!(
                    "package p\nservice veh.hvac.cabin {{ signal  temperature : Temperature @[ 1s .. 10s ]{comma}\n // callable\n command setTarget(t : Temperature) [require t>0] @10ms{comma} }}\n"
                ),
                "package p\n\nservice veh.hvac.cabin {\n  signal temperature: Temperature @[1s..10s]\n  // callable\n  command setTarget(t: Temperature) @10ms [\n    require t > 0\n  ]\n}\n",
                Profile::Ridl,
                &FormatOptions::default(),
            );
        }
        assert_profile_format(
            "package p\nservice veh.empty {}\n",
            "package p\n\nservice veh.empty {}\n",
            Profile::Ridl,
            &FormatOptions::default(),
        );
    }

    #[test]
    fn ridl_inline_service_keeps_header_and_brace_comments() {
        assert_profile_format(
            "package p\nservice  veh.body /* header */ { // brace\n signal s:T }\n",
            "package p\n\nservice  veh.body /* header */ { // brace\n  signal s: T\n}\n",
            Profile::Ridl,
            &FormatOptions::default(),
        );
    }

    #[test]
    fn ridl_services_golden_preserves_structure_and_comments() {
        assert_profile_format(
            include_str!("../test_data/input/services.ridl"),
            include_str!("../test_data/formatted/services.ridl"),
            Profile::Ridl,
            &FormatOptions::default(),
        );
    }

    #[test]
    fn ridl_attributes_golden_preserves_structure_and_comments() {
        assert_profile_format(
            include_str!("../test_data/input/attributes.ridl"),
            include_str!("../test_data/formatted/attributes.ridl"),
            Profile::Ridl,
            &FormatOptions::default(),
        );
    }

    #[test]
    fn ridl_interface_golden_preserves_structure_and_comments() {
        assert_profile_format(
            include_str!("../test_data/input/interface.ridl"),
            include_str!("../test_data/formatted/interface.ridl"),
            Profile::Ridl,
            &FormatOptions::default(),
        );
    }

    #[test]
    fn ridl_parameter_and_tuple_return_width_boundaries() {
        for width in [100, 60] {
            let options = if width == 100 {
                FormatOptions::default()
            } else {
                FormatOptions {
                    max_line_length: Some(width),
                }
            };
            for columns in [width - 1, width, width + 1] {
                for is_return in [false, true] {
                    let prefix = if is_return {
                        "  query q(): "
                    } else {
                        "  command c"
                    };
                    let fixed = format!("{prefix}(a: , b: B)");
                    let name = "A".repeat(columns - fixed.chars().count());
                    let inline = format!("{prefix}(a: {name}, b: B)");
                    assert_eq!(inline.chars().count(), columns);
                    let member = if columns <= width {
                        inline.clone()
                    } else {
                        format!("{prefix}(\n    a: {name},\n    b: B\n  )")
                    };
                    assert_profile_format(
                        &format!("package p\ninterface I {{\n{inline}\n}}\n"),
                        &format!("package p\n\ninterface I {{\n{member}\n}}\n"),
                        Profile::Ridl,
                        &options,
                    );
                }
            }
        }
    }

    #[test]
    fn ridl_breaks_tuple_return_before_parameters_and_remeasures() {
        let input = "package p\ninterface I { query getSpeedHistory(window: Duration, mode: Mode): (min: Speed, max: Speed, avg: Speed) @[..100ms] }\n";
        let expected = "package p\n\ninterface I {\n  query getSpeedHistory(window: Duration, mode: Mode): (\n    min: Speed,\n    max: Speed,\n    avg: Speed\n  ) @[..100ms]\n}\n";
        assert_profile_format(
            input,
            expected,
            Profile::Ridl,
            &FormatOptions {
                max_line_length: Some(60),
            },
        );
        let expected = "package p\n\ninterface I {\n  query getSpeedHistory(\n    window: Duration,\n    mode: Mode\n  ): (\n    min: Speed,\n    max: Speed,\n    avg: Speed\n  ) @[..100ms]\n}\n";
        assert_profile_format(
            input,
            expected,
            Profile::Ridl,
            &FormatOptions {
                max_line_length: Some(40),
            },
        );
    }

    /// Every width fixture pins its rendering, fixed point, and full tree and
    /// content streams. Comments participate in the content stream only.
    fn assert_width_format(input: &str, expected: &str, options: &FormatOptions) {
        assert_profile_format(input, expected, Profile::Typl, options);
    }

    fn assert_profile_format(
        input: &str,
        expected: &str,
        profile: Profile,
        options: &FormatOptions,
    ) {
        let outcome = format(input, profile, options);
        assert_eq!(outcome, FormatOutcome::Formatted(expected.to_string()));
        assert_eq!(
            format(expected, profile, options),
            outcome,
            "not a fixed point"
        );
        assert_eq!(
            crate::test_invariants::syntax_structure(input, profile),
            crate::test_invariants::syntax_structure(expected, profile),
            "structure changed"
        );
        assert_eq!(
            crate::test_invariants::content_tokens(input, profile),
            crate::test_invariants::content_tokens(expected, profile),
            "content changed"
        );
    }

    #[test]
    fn tuple_width_boundaries_include_indentation() {
        for width in [100, 60] {
            let options = if width == 100 {
                FormatOptions::default()
            } else {
                FormatOptions {
                    max_line_length: Some(width),
                }
            };
            for columns in [width - 1, width, width + 1] {
                // The fixed part of this line has 19 characters, without A.
                let name = "A".repeat(columns - 19);
                let inline = format!("  pair: (a: {name}, b: B)");
                assert_eq!(inline.chars().count(), columns);
                let input = format!("package p\nstruct S {{\n{inline}\n}}\n");
                let field = if columns <= width {
                    inline
                } else {
                    format!("  pair: (\n    a: {name},\n    b: B\n  )")
                };
                let expected = format!("package p\n\nstruct S {{\n{field}\n}}\n");
                assert_width_format(&input, &expected, &options);
            }
        }
    }

    #[test]
    fn collection_tuples_break_and_keep_the_collection_suffix() {
        let options = FormatOptions {
            max_line_length: Some(20),
        };
        for (input, expected) in [
            (
                "package p\nstruct S { readings: [(a: A, b: B); 8] }\n",
                "package p\n\nstruct S {\n  readings: [(\n    a: A,\n    b: B\n  ); 8]\n}\n",
            ),
            (
                "package p\nstruct S { readings: [Key: (a: A, b: B); 8] }\n",
                "package p\n\nstruct S {\n  readings: [Key: (\n    a: A,\n    b: B\n  ); 8]\n}\n",
            ),
            (
                "package p\nstruct S { readings: (a: A, b: B)? }\n",
                "package p\n\nstruct S {\n  readings: (\n    a: A,\n    b: B\n  )?\n}\n",
            ),
        ] {
            assert_width_format(input, expected, &options);
        }
    }

    #[test]
    fn nested_tuples_break_outer_first_and_measure_again() {
        let input = "package p\nstruct S { pair: (inner: (a: A, b: B), tail: C) }\n";
        for (width, expected) in [
            (
                28,
                "package p\n\nstruct S {\n  pair: (\n    inner: (a: A, b: B),\n    tail: C\n  )\n}\n",
            ),
            (
                20,
                "package p\n\nstruct S {\n  pair: (\n    inner: (\n      a: A,\n      b: B\n    ),\n    tail: C\n  )\n}\n",
            ),
        ] {
            assert_width_format(
                input,
                expected,
                &FormatOptions {
                    max_line_length: Some(width),
                },
            );
        }
    }

    #[test]
    fn only_the_last_independent_tuple_breaks_when_that_is_enough() {
        assert_width_format(
            "package p\nstruct S { pairs: [(a: A, b: B): (c: C, d: D); 8] }\n",
            "package p\n\nstruct S {\n  pairs: [(a: A, b: B): (\n    c: C,\n    d: D\n  ); 8]\n}\n",
            &FormatOptions {
                max_line_length: Some(30),
            },
        );
    }

    #[test]
    fn a_trailing_comment_follows_the_broken_tuple_closer() {
        assert_width_format(
            "package p\nstruct S { pair: (a: A, b: B) // pair detail\n}\n",
            "package p\n\nstruct S {\n  pair: (\n    a: A,\n    b: B\n  ) // pair detail\n}\n",
            &FormatOptions {
                max_line_length: Some(19),
            },
        );
    }

    #[test]
    fn trailing_comments_do_not_cause_tuple_breaks() {
        let comment = "note".repeat(40);
        let input = format!("package p\nstruct S {{ pair: (a: A, b: B) // {comment}\n}}\n");
        let expected = format!("package p\n\nstruct S {{\n  pair: (a: A, b: B) // {comment}\n}}\n");
        assert_width_format(
            &input,
            &expected,
            &FormatOptions {
                max_line_length: Some(20),
            },
        );
    }

    #[test]
    fn unbreakable_strings_are_left_over_the_limit() {
        let value = "x".repeat(120);
        let input = format!("package p\nconst TEXT: string = \"{value}\"\n");
        let expected = format!("package p\n\nconst TEXT: string = \"{value}\"\n");
        assert_width_format(
            &input,
            &expected,
            &FormatOptions {
                max_line_length: Some(60),
            },
        );
    }

    #[test]
    fn no_width_limit_keeps_a_two_hundred_column_tuple_line() {
        let name = "A".repeat(181);
        let line = format!("  pair: (a: {name}, b: B)");
        assert_eq!(line.chars().count(), 200);
        let input = format!("package p\nstruct S {{\n{line}\n}}\n");
        let expected = format!("package p\n\nstruct S {{\n{line}\n}}\n");
        assert_width_format(
            &input,
            &expected,
            &FormatOptions {
                max_line_length: None,
            },
        );
    }

    #[test]
    fn width_one_breaks_every_tuple_and_keeps_the_tree() {
        assert_width_format(
            "package p\nstruct S { pair: (inner: (a: A, b: B), tail: C) }\n",
            "package p\n\nstruct S {\n  pair: (\n    inner: (\n      a: A,\n      b: B\n    ),\n    tail: C\n  )\n}\n",
            &FormatOptions {
                max_line_length: Some(1),
            },
        );
    }

    #[test]
    fn tuple_comments_keep_the_construct_verbatim_at_small_widths() {
        assert_width_format(
            "package p\nstruct S { pair: (a: A, /* note */ b: B) }\n",
            "package p\n\nstruct S {\n  pair: (a: A, /* note */ b: B)\n}\n",
            &FormatOptions {
                max_line_length: Some(1),
            },
        );
    }

    #[test]
    fn tuple_width_counts_unicode_scalars_in_the_initializer() {
        let input = "package p\nstruct S { pair: (a: A, b: B) = \"ééééé\" }\n";
        assert_width_format(
            input,
            "package p\n\nstruct S {\n  pair: (a: A, b: B) = \"ééééé\"\n}\n",
            &FormatOptions {
                max_line_length: Some(30),
            },
        );
        assert_width_format(
            input,
            "package p\n\nstruct S {\n  pair: (\n    a: A,\n    b: B\n  ) = \"ééééé\"\n}\n",
            &FormatOptions {
                max_line_length: Some(29),
            },
        );
    }

    #[test]
    fn a_broken_tuple_is_left_unformatted_at_every_width() {
        for width in [100, 60, 1] {
            assert!(matches!(
                format(
                    "package p\nstruct S { pair: (a: A }\n",
                    Profile::Typl,
                    &FormatOptions {
                        max_line_length: Some(width)
                    }
                ),
                FormatOutcome::ParseErrors(_)
            ));
        }
    }
}
