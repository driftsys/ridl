//! `ridl fmt` — the CST-based formatter for the typl surface
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
//!
//! # What order is *not* changed
//!
//! Source order is wire identity (typl reference §7.4): a formatter must never
//! change ordinals. This formatter normalises whitespace and separators only —
//! it never reorders declarations, imports, fields, enum values, or union arms.
//! It also does not reorder a definition that appears before the `package`
//! line; it emits every element in source order.
//!
//! # A note on comments inside a one-line construct
//!
//! Comments survive at declaration and member boundaries (leading and inline
//! trailing). A comment embedded inside a single-line construct — between the
//! brackets of a constraint or a collection, or between the parentheses of a
//! tuple — is dropped, because that construct is re-emitted from its significant
//! tokens. Comments are trivia, so dropping one leaves the node structure and
//! the non-trivia token set unchanged, and the result is still idempotent. The
//! typl style places no comments there, so this does not arise in practice.

use ridl_syntax::{
    SyntaxKind, SyntaxNode,
    ast::{AstNode, SourceFile},
};
use rowan::NodeOrToken;

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

/// Formats `text` into the canonical tight style.
///
/// A syntactically valid input yields [`FormatOutcome::Formatted`]; an input
/// with any parse error yields [`FormatOutcome::ParseErrors`] and is not
/// rewritten.
pub fn format(text: &str) -> FormatOutcome {
    let parse = ridl_syntax::parse(text);
    if !parse.errors().is_empty() {
        return FormatOutcome::ParseErrors(parse.errors().to_vec());
    }
    let Some(file) = SourceFile::cast(parse.syntax()) else {
        // `parse` always roots a `SourceFile`; this arm cannot be reached, but
        // returning the input unchanged is the honest fallback.
        return FormatOutcome::Formatted(text.to_string());
    };
    FormatOutcome::Formatted(format_source_file(&file))
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
fn format_source_file(file: &SourceFile) -> String {
    let elements: Vec<_> = file.syntax().children_with_tokens().collect();
    let lines = layout_container(&elements, 0, true);
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
) -> Vec<String> {
    let ind = indent_str(indent);
    let mut blocks: Vec<Block> = Vec::new();
    // Comments seen since the last block, waiting to lead the next node.
    let mut pending: Vec<PendingComment> = Vec::new();
    // Newlines in the whitespace run since the last non-whitespace element.
    let mut nl_run = 0usize;
    // Whether a comment on the current line would trail an existing block.
    let mut can_trail = false;

    for element in elements {
        match element {
            NodeOrToken::Token(token) if token.kind() == SyntaxKind::Whitespace => {
                nl_run += token.text().matches('\n').count();
            }
            NodeOrToken::Token(token) if is_comment(token.kind()) => {
                let text = token.text().trim_end().to_string();
                if can_trail && nl_run == 0 && pending.is_empty() {
                    if let Some(last) = blocks.last_mut()
                        && let Some(line) = last.lines.last_mut()
                    {
                        line.push(' ');
                        line.push_str(&text);
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
                // A separator comma is dropped; the previous member keeps a
                // trailing comment on its line.
                nl_run = 0;
            }
            NodeOrToken::Node(node) => {
                let kind = block_kind(node.kind());
                let gap_blank = match pending.first() {
                    Some(first) => first.blank_before,
                    None => nl_run >= 2,
                };
                let mut lines = Vec::new();
                for (i, comment) in pending.iter().enumerate() {
                    if i > 0 && comment.blank_before {
                        lines.push(String::new());
                    }
                    lines.push(format!("{ind}{}", comment.text));
                }
                if !pending.is_empty() && nl_run >= 2 {
                    lines.push(String::new());
                }
                lines.extend(format_element(node, indent));
                blocks.push(Block {
                    kind,
                    gap_blank,
                    lines,
                });
                pending.clear();
                nl_run = 0;
                can_trail = true;
            }
            _ => {
                // No other token appears as a direct container child: the file
                // holds only nodes and trivia, a brace block holds members,
                // separator commas, and trivia.
                nl_run = 0;
            }
        }
    }

    if !pending.is_empty() {
        let gap_blank = pending[0].blank_before;
        let mut lines = Vec::new();
        for (i, comment) in pending.iter().enumerate() {
            if i > 0 && comment.blank_before {
                lines.push(String::new());
            }
            lines.push(format!("{ind}{}", comment.text));
        }
        blocks.push(Block {
            kind: BlockKind::CommentOnly,
            gap_blank,
            lines,
        });
    }

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
        | SyntaxKind::UnionDef => BlockKind::Def,
        _ => BlockKind::Member,
    }
}

// --- element dispatch ----------------------------------------------------

type SyntaxElement = NodeOrToken<SyntaxNode, ridl_syntax::SyntaxToken>;

/// Formats one structural node into its physical lines, indented at `indent`.
fn format_element(node: &SyntaxNode, indent: usize) -> Vec<String> {
    let ind = indent_str(indent);
    let line = |s: String| vec![format!("{ind}{s}")];
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
                format_block_def(node, indent, "enumset")
            } else {
                line(format_enumset_derived(node))
            }
        }
        SyntaxKind::StructDef => format_block_def(node, indent, "struct"),
        SyntaxKind::EnumDef => format_block_def(node, indent, "enum"),
        SyntaxKind::UnionDef => format_block_def(node, indent, "union"),
        SyntaxKind::FieldDef => line(format_field_def(node)),
        SyntaxKind::ReservedEntry => line(format_reserved_entry(node)),
        SyntaxKind::EnumValue | SyntaxKind::EnumSetBit => line(format_value_assignment(node)),
        SyntaxKind::UnionArm => line(format_union_arm(node)),
        // Unreachable for an error-free tree; the honest fallback is the node's
        // own text so no source is lost.
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

/// Formats a `struct` / `enum` / `union` / standalone `enumset` — the header,
/// the members laid out at the next indent, and the closing brace. An empty
/// body renders as `{}` on the header line.
fn format_block_def(node: &SyntaxNode, indent: usize, keyword: &str) -> Vec<String> {
    let ind = indent_str(indent);
    let header = format!(
        "{ind}{}{keyword} {} {{",
        modifiers_prefix(node),
        child_tight(node, SyntaxKind::Name),
    );
    let members = elements_between_braces(node);
    let member_lines = layout_container(&members, indent + 1, false);
    if member_lines.is_empty() {
        return vec![format!(
            "{ind}{}{keyword} {} {{}}",
            modifiers_prefix(node),
            child_tight(node, SyntaxKind::Name),
        )];
    }
    let mut out = Vec::with_capacity(member_lines.len() + 2);
    out.push(header);
    out.extend(member_lines);
    out.push(format!("{ind}}}"));
    out
}

/// The container children strictly between the block's first `{` and its
/// closing `}` — member nodes, separator commas, and trivia.
fn elements_between_braces(node: &SyntaxNode) -> Vec<SyntaxElement> {
    let mut out = Vec::new();
    let mut inside = false;
    for element in node.children_with_tokens() {
        match &element {
            NodeOrToken::Token(t) if t.kind() == SyntaxKind::LBrace && !inside => inside = true,
            NodeOrToken::Token(t) if t.kind() == SyntaxKind::RBrace => break,
            _ if inside => out.push(element),
            _ => {}
        }
    }
    out
}

// --- members -------------------------------------------------------------

fn format_field_def(node: &SyntaxNode) -> String {
    let mut out = format!(
        "{}: {}",
        child_tight(node, SyntaxKind::Name),
        field_type(node)
    );
    if let Some(init) = child_node(node, SyntaxKind::InitValue)
        && let Some(literal) = child_node(&init, SyntaxKind::Literal)
    {
        out.push_str(" = ");
        out.push_str(&tight_text(&literal));
    }
    out
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

// --- field types ---------------------------------------------------------

/// The first field-type child of `node`, rendered inline.
fn field_type(node: &SyntaxNode) -> String {
    node.children()
        .find(|c| is_field_type(c.kind()))
        .map(|c| format_field_type(&c))
        .unwrap_or_default()
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
    )
}

/// Renders a field, tuple-field, or collection-element type inline, recursing
/// through tuples, arrays, maps, and optionals.
fn format_field_type(node: &SyntaxNode) -> String {
    match node.kind() {
        SyntaxKind::PathType => tight_text(node),
        SyntaxKind::PrimitiveType => {
            let mut out = primitive_keyword(node);
            if let Some(constraint) = child_node(node, SyntaxKind::Constraint) {
                out.push(' ');
                out.push_str(&format_constraint(&constraint));
            }
            out
        }
        SyntaxKind::OptionalType => {
            let inner = node
                .children()
                .find(|c| is_field_type(c.kind()))
                .map(|c| format_field_type(&c))
                .unwrap_or_default();
            format!("{inner}?")
        }
        SyntaxKind::TupleType => {
            let fields: Vec<String> = node
                .children()
                .filter(|c| c.kind() == SyntaxKind::TupleField)
                .map(|f| {
                    format!(
                        "{}: {}",
                        child_tight(&f, SyntaxKind::Name),
                        f.children()
                            .find(|c| is_field_type(c.kind()))
                            .map(|c| format_field_type(&c))
                            .unwrap_or_default(),
                    )
                })
                .collect();
            format!("({})", fields.join(", "))
        }
        SyntaxKind::ArrayType => {
            let element = node
                .children()
                .find(|c| is_field_type(c.kind()))
                .map(|c| format_field_type(&c))
                .unwrap_or_default();
            format!("[{element}; {}]", child_tight(node, SyntaxKind::Bound))
        }
        SyntaxKind::MapType => {
            let mut types = node.children().filter(|c| is_field_type(c.kind()));
            let key = types
                .next()
                .map(|c| format_field_type(&c))
                .unwrap_or_default();
            let value = types
                .next()
                .map(|c| format_field_type(&c))
                .unwrap_or_default();
            format!("[{key}: {value}; {}]", child_tight(node, SyntaxKind::Bound))
        }
        _ => tight_text(node),
    }
}

// --- constraints ---------------------------------------------------------

/// Renders a `[ … ]` constraint: tight brackets, no spaces around `..`, single
/// spaces around `step` and `match`. Scalar endpoints are direct `Literal`
/// children; a length bound is a `Bound` child.
fn format_constraint(node: &SyntaxNode) -> String {
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
/// length bound) whose parts never take internal spacing.
fn tight_text(node: &SyntaxNode) -> String {
    node.descendants_with_tokens()
        .filter_map(|e| e.into_token())
        .filter(|t| !t.kind().is_trivia())
        .map(|t| t.text().to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn formatted(input: &str) -> String {
        match format(input) {
            FormatOutcome::Formatted(s) => s,
            FormatOutcome::ParseErrors(errors) => {
                panic!("expected a formatted result, got parse errors: {errors:?}")
            }
        }
    }

    #[test]
    fn broken_input_returns_parse_errors_untouched() {
        // A missing `package` is FORM-104; a broken file is never rewritten.
        let outcome = format("type 123 :: [\n");
        let FormatOutcome::ParseErrors(errors) = outcome else {
            panic!("expected parse errors for broken input");
        };
        assert!(!errors.is_empty());
    }

    #[test]
    fn missing_package_is_a_parse_error_not_a_format() {
        assert!(matches!(
            format("type Speed: km/h\n"),
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
}
