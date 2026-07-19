//! Completion (docs/ROADMAP.md epic E1.15c).
//!
//! Completion runs constantly, on text that is usually incomplete, so the
//! context detection reads the parse tree defensively and degrades to an empty
//! list rather than guessing. Four contexts are recognised:
//!
//! - after a `:` (a field, const, or type/enumset backing position) → the
//!   visible named types (locals, `ridl.std`, imports — the resolver's own view)
//!   plus the primitives, each annotated with a [`lt::CompletionItemKind`];
//! - after `import` (anywhere in the import path) → the known package names, and
//!   once a package path is complete, that package's public symbols;
//! - inside a constraint after the `match` keyword → the regex constants in
//!   scope (a `match` pattern is a regex literal or a named regex constant);
//! - at a definition-start position (the top level of a file) → the definition
//!   keywords and the `internal` / `error` modifiers.
//!
//! The context is decided from the token to the left of the cursor and the
//! identifier the cursor is completing, not from a well-formed tree — the same
//! discipline the resolver and navigation use.

use lsp_types as lt;
use ridl_core::db::InputFile;
use ridl_core::package::{Package, Workspace};
use ridl_sem::{ConstValue, SymbolKind, const_value, resolve_package};
use ridl_syntax::ast::{AstNode, Import, SourceFile};
use ridl_syntax::{SyntaxKind, SyntaxNode, SyntaxToken};
use rowan::{TextSize, TokenAtOffset};

use crate::nav::source_file;

/// The five typl primitives, offered wherever a named type may appear.
const PRIMITIVES: &[&str] = &["boolean", "integer", "float", "string", "bytes"];

/// The definition keywords and modifiers offered at a definition-start position.
const DEFINITION_KEYWORDS: &[&str] = &[
    "type", "const", "struct", "enum", "enumset", "union", "internal", "error",
];

/// The completion items for the cursor at `offset` in `file` (a file of `pkg`).
///
/// `packages` is the every-package universe (workspace members, standalone
/// overlays, and the embedded `ridl.std`) the import context offers names from;
/// `std` is threaded into resolution exactly as [`resolve_package`] takes it.
pub fn completion(
    db: &dyn salsa::Database,
    ws: Workspace,
    std: Package,
    pkg: Package,
    file: InputFile,
    offset: TextSize,
    packages: &[Package],
) -> Vec<lt::CompletionItem> {
    let source = source_file(db, file);
    let Some(context) = context(source.syntax(), offset) else {
        return Vec::new();
    };
    match context {
        Context::Type => type_completions(db, ws, std, pkg),
        Context::Import => import_completions(db, ws, std, packages, &source, offset),
        Context::Match => match_completions(db, ws, std, pkg),
        Context::DefinitionStart => keyword_completions(),
    }
}

/// The recognised completion positions.
enum Context {
    /// After a `:` — a type or backing is expected.
    Type,
    /// Inside an `import` statement — a package name or public symbol.
    Import,
    /// After the `match` keyword in a constraint — a regex constant.
    Match,
    /// At the top level of a file — a definition keyword or modifier.
    DefinitionStart,
}

/// Decides the completion context from the tree around `offset`, or `None` when
/// the cursor is not in a position the server completes.
fn context(root: &SyntaxNode, offset: TextSize) -> Option<Context> {
    let left = left_token(root, offset)?;
    // The identifier the cursor is completing, if the left token is a partial
    // word the cursor sits inside.
    let word = (left.kind() == SyntaxKind::Ident && left.text_range().start() < offset)
        .then(|| left.clone());
    // The token that introduces this position: the significant token before the
    // partial word, or the significant token at or before the left token.
    let anchor = match &word {
        Some(word) => word
            .prev_token()
            .and_then(|token| significant_at_or_before(&token)),
        None => significant_at_or_before(&left),
    };

    // Import: anywhere inside an import statement (the keyword, the path, or the
    // trailing whitespace the parser attaches to the `Import` node).
    if left.parent_ancestors().any(is_import)
        || anchor
            .as_ref()
            .is_some_and(|token| token.kind() == SyntaxKind::ImportKw)
    {
        return Some(Context::Import);
    }

    if let Some(anchor) = &anchor {
        match anchor.kind() {
            SyntaxKind::Colon => return Some(Context::Type),
            SyntaxKind::MatchKw => return Some(Context::Match),
            // Naming positions: right after a definition keyword the user is
            // typing the declared name, so offer nothing.
            SyntaxKind::TypeKw
            | SyntaxKind::ConstKw
            | SyntaxKind::StructKw
            | SyntaxKind::EnumKw
            | SyntaxKind::EnumsetKw
            | SyntaxKind::UnionKw => return None,
            _ => {}
        }
    }

    // Definition start: the cursor sits at the top level of the file.
    if is_top_level(word.as_ref().unwrap_or(&left)) {
        return Some(Context::DefinitionStart);
    }
    None
}

/// The token to the left of `offset` — the one ending at or containing it.
fn left_token(root: &SyntaxNode, offset: TextSize) -> Option<SyntaxToken> {
    match root.token_at_offset(offset) {
        TokenAtOffset::None => None,
        TokenAtOffset::Single(token) => Some(token),
        TokenAtOffset::Between(left, _right) => Some(left),
    }
}

/// The nearest non-trivia token at or before `token`, walking the whole token
/// stream backwards.
fn significant_at_or_before(token: &SyntaxToken) -> Option<SyntaxToken> {
    let mut current = Some(token.clone());
    while let Some(token) = current {
        if !token.kind().is_trivia() {
            return Some(token);
        }
        current = token.prev_token();
    }
    None
}

/// Whether `token` sits at the top level of the file — its nearest node ancestor
/// is the `SourceFile`, seen through any recovery `ErrorNode` wrappers.
fn is_top_level(token: &SyntaxToken) -> bool {
    let mut node = token.parent();
    while let Some(current) = node {
        match current.kind() {
            SyntaxKind::SourceFile => return true,
            SyntaxKind::ErrorNode => node = current.parent(),
            _ => return false,
        }
    }
    false
}

/// Whether a syntax kind is an `import` statement node.
fn is_import(node: SyntaxNode) -> bool {
    node.kind() == SyntaxKind::Import
}

/// The visible named types plus the primitives, for a type/backing position.
fn type_completions(
    db: &dyn salsa::Database,
    ws: Workspace,
    std: Package,
    pkg: Package,
) -> Vec<lt::CompletionItem> {
    let resolution = resolve_package(db, ws, pkg, std);
    let mut items: Vec<lt::CompletionItem> = resolution
        .symbols
        .iter()
        .filter(|(_, symbol)| symbol.kind != SymbolKind::Const)
        .map(|(name, symbol)| {
            item(
                name,
                type_kind(symbol.kind),
                format!("{}.{}", symbol.package, symbol.name),
            )
        })
        .collect();
    for primitive in PRIMITIVES {
        items.push(item(
            primitive,
            lt::CompletionItemKind::KEYWORD,
            "primitive".to_string(),
        ));
    }
    sorted(items)
}

/// The known package names, plus — once a complete package path has been typed
/// — that package's public symbols.
fn import_completions(
    db: &dyn salsa::Database,
    ws: Workspace,
    std: Package,
    packages: &[Package],
    source: &SourceFile,
    offset: TextSize,
) -> Vec<lt::CompletionItem> {
    let mut items: Vec<lt::CompletionItem> = Vec::new();
    let mut seen: Vec<String> = Vec::new();
    for package in packages {
        let name = package.name(db).clone();
        if name.is_empty() || seen.contains(&name) {
            continue;
        }
        seen.push(name.clone());
        items.push(item(
            &name,
            lt::CompletionItemKind::MODULE,
            "package".to_string(),
        ));
    }

    // The package path completed before the cursor (everything but the segment
    // under the cursor). When it names a known package, offer its public
    // symbols.
    if let Some(prefix) = import_path_prefix(source, offset)
        && let Some(target) = packages.iter().find(|package| *package.name(db) == prefix)
    {
        let target_name = target.name(db).clone();
        let resolution = resolve_package(db, ws, *target, std);
        for (name, symbol) in &resolution.symbols {
            if symbol.package == target_name && !symbol.internal {
                items.push(item(
                    name,
                    symbol_kind(symbol.kind),
                    format!("{target_name}.{name}"),
                ));
            }
        }
    }
    sorted(items)
}

/// The regex constants in scope, for a `match` pattern position.
fn match_completions(
    db: &dyn salsa::Database,
    ws: Workspace,
    std: Package,
    pkg: Package,
) -> Vec<lt::CompletionItem> {
    let resolution = resolve_package(db, ws, pkg, std);
    let items = resolution
        .symbols
        .iter()
        .filter(|(_, symbol)| symbol.kind == SymbolKind::Const)
        .filter(|(name, _)| {
            matches!(
                const_value(db, ws, std, &resolution, name),
                Some(ConstValue::Regex(_))
            )
        })
        .map(|(name, _)| item(name, lt::CompletionItemKind::CONSTANT, "regex".to_string()))
        .collect();
    sorted(items)
}

/// The definition keywords and modifiers.
fn keyword_completions() -> Vec<lt::CompletionItem> {
    DEFINITION_KEYWORDS
        .iter()
        .map(|keyword| {
            item(
                keyword,
                lt::CompletionItemKind::KEYWORD,
                "keyword".to_string(),
            )
        })
        .collect()
}

/// The package path typed before the cursor's current segment — the completed
/// prefix of the import path. Reads the whole import `QualifiedName`, dropping
/// the final segment (the one under the cursor). Returns `None` when there is no
/// import path yet or nothing precedes the cursor's segment.
fn import_path_prefix(source: &SourceFile, offset: TextSize) -> Option<String> {
    let import = source
        .syntax()
        .descendants()
        .filter_map(Import::cast)
        .find(|import| import.syntax().text_range().contains_inclusive(offset))?;
    let qualified = import.qualified_name()?;
    let mut segments = crate::nav::qualified_segments(qualified.syntax());
    // Drop the final (partial) segment: `veh.common.` → the empty trailing
    // segment, `veh.common` → the `common` being typed.
    segments.pop();
    if segments.is_empty() {
        return None;
    }
    Some(segments.join("."))
}

/// The completion-item kind for a named type used in a type position.
fn type_kind(kind: SymbolKind) -> lt::CompletionItemKind {
    match kind {
        SymbolKind::Type => lt::CompletionItemKind::CLASS,
        SymbolKind::Struct => lt::CompletionItemKind::STRUCT,
        SymbolKind::Enum | SymbolKind::EnumSet => lt::CompletionItemKind::ENUM,
        SymbolKind::Union => lt::CompletionItemKind::INTERFACE,
        SymbolKind::Const => lt::CompletionItemKind::CONSTANT,
    }
}

/// The completion-item kind for any resolver symbol (import-symbol context).
fn symbol_kind(kind: SymbolKind) -> lt::CompletionItemKind {
    match kind {
        SymbolKind::Const => lt::CompletionItemKind::CONSTANT,
        other => type_kind(other),
    }
}

/// Builds one completion item.
fn item(label: &str, kind: lt::CompletionItemKind, detail: String) -> lt::CompletionItem {
    lt::CompletionItem {
        label: label.to_string(),
        kind: Some(kind),
        detail: Some(detail),
        ..Default::default()
    }
}

/// Sorts items by label for a deterministic list (resolution is a `HashMap`).
fn sorted(mut items: Vec<lt::CompletionItem>) -> Vec<lt::CompletionItem> {
    items.sort_by(|a, b| a.label.cmp(&b.label));
    items
}
