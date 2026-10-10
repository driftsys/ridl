//! Completion.
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
//!   keywords and the `internal` / `error` modifiers;
//! - at an interaction-start position (directly inside an `interface` body or a
//!   service's inline shape) → the five ridl interaction keywords plus
//!   `reserved`;
//! - inside a doc comment, after an open `[` → what the doc-link resolver can
//!   reach (ADR-0026): the names in scope and the known packages, a package's
//!   public declarations after its path, a declaration's members after its
//!   name; and after `@` at the start of a doc line → the four doc tags.
//!   Anywhere else inside a doc comment nothing is offered — it is prose.
//!
//! An item that names a declaration carries the declaration's doc as its
//! `documentation`, rendered as hover renders it but without the signature
//! or the Contract list, to keep the item short (ADR-0026).
//!
//! In an `.rsdl` file the doc comment and `import` contexts are the same, and
//! the other contexts are the rsdl slots (rsdl reference §3, §4), each offering
//! what the checker would bind there:
//!
//! - at the top level → the four rsdl declaration keywords;
//! - at the start of a component line → `offers` and `requires`; in a
//!   deployment body → `machine`;
//! - after `offers` → every service of the catalog; after `requires` → the
//!   interfaces the package sees and the services with an inline shape;
//! - on a member line of a `system` or a `distribution` → the components and
//!   the services no component offers (RSDL-504); on the placement line of a
//!   `machine`, the declared instances `Name.inst` too;
//! - after `for` → the systems.
//!
//! A component or a system of the file's own package is offered by its bare
//! name, one of another package by its qualified name. When the part already
//! written qualifies the reference with the own package, an own-package item
//! is matched and written by its qualified name.
//!
//! The context is decided from the token to the left of the cursor and the
//! identifier the cursor is completing, not from a well-formed tree — the same
//! discipline the resolver and navigation use.

use lsp_types as lt;
use ridl_core::db::InputFile;
use ridl_core::package::{Package, Workspace, service_catalog};
use ridl_sem::{
    ConstValue, LinkTarget, SymbolKind, check_system, const_value, doc_link_members,
    resolve_doc_link, resolve_package,
};
use ridl_syntax::ast::{AstNode, Import, SourceFile};
use ridl_syntax::{SyntaxKind, SyntaxNode, SyntaxToken};
use rowan::{TextRange, TextSize, TokenAtOffset};

use crate::doc;
use crate::hover::SymbolDocs;
use crate::nav::source_file;

/// The five typl primitives, offered wherever a named type may appear.
const PRIMITIVES: &[&str] = &["boolean", "integer", "float", "string", "bytes"];

/// The definition keywords and modifiers offered at a definition-start position.
const DEFINITION_KEYWORDS: &[&str] = &[
    "type", "const", "struct", "enum", "enumset", "union", "internal", "error",
];

/// The interaction keywords offered at an interaction-start position: the five
/// ridl kinds (ridl §4–§8) plus the `reserved` tombstone (ridl §11). Nothing
/// else may appear in an interface body — a typl declaration there is RIDL-107.
const INTERACTION_KEYWORDS: &[&str] = &["signal", "event", "command", "query", "fixed", "reserved"];

/// The rsdl declaration keywords offered at the top level of an `.rsdl` file
/// (rsdl reference §3).
const RSDL_DEFINITION_KEYWORDS: &[&str] = &["system", "component", "distribution", "deployment"];

/// The keywords that start a line of a component body (rsdl reference §3.2).
const COMPONENT_LINE_KEYWORDS: &[&str] = &["offers", "requires"];

/// The keyword that starts a line of a deployment body (rsdl reference §3.4).
const DEPLOYMENT_LINE_KEYWORDS: &[&str] = &["machine"];

/// The four doc tags offered after `@` at the start of a doc line
/// (typl §14.2, ADR-0026).
const DOC_TAGS: &[&str] = &["see", "since", "deprecated", "labels"];

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
    if let Some(items) = doc_completion(db, ws, std, pkg, packages, &source, offset) {
        return items;
    }
    let Some(context) = context(source.syntax(), offset) else {
        return Vec::new();
    };
    match context {
        Context::Type => type_completions(db, ws, std, pkg),
        Context::Import => import_completions(db, ws, std, packages, &source, offset),
        Context::Match => match_completions(db, ws, std, pkg),
        Context::DefinitionStart => keyword_completions(DEFINITION_KEYWORDS),
        Context::InteractionStart => keyword_completions(INTERACTION_KEYWORDS),
    }
}

/// The items inside a doc comment, or `None` when the cursor is not in one.
fn doc_completion(
    db: &dyn salsa::Database,
    ws: Workspace,
    std: Package,
    pkg: Package,
    packages: &[Package],
    source: &SourceFile,
    offset: TextSize,
) -> Option<Vec<lt::CompletionItem>> {
    let cursor = doc::in_doc_comment(source, offset)?;
    Some(if cursor.at_line_start_at {
        sorted(keyword_completions(DOC_TAGS))
    } else if let Some(prefix) = cursor.after_open_bracket {
        doc_link_completions(db, ws, std, pkg, packages, &prefix)
    } else {
        Vec::new()
    })
}

/// The completion items for the cursor at `offset` in `file`, an `.rsdl` file
/// of `pkg`, and the range an item replaces: the part of a reference already
/// written before the cursor, `None` outside a reference slot. An item whose
/// `filter_text` is set is written with that qualified name.
pub fn rsdl_completion(
    db: &dyn salsa::Database,
    ws: Workspace,
    std: Package,
    pkg: Package,
    file: InputFile,
    offset: TextSize,
    packages: &[Package],
) -> (Vec<lt::CompletionItem>, Option<TextRange>) {
    let source = source_file(db, file);
    if let Some(items) = doc_completion(db, ws, std, pkg, packages, &source, offset) {
        return (items, None);
    }
    let Some(cursor) = Cursor::at(source.syntax(), offset) else {
        return (Vec::new(), None);
    };
    if cursor.in_import() {
        return (
            import_completions(db, ws, std, packages, &source, offset),
            None,
        );
    }
    let Some(slot) = rsdl_slot(&cursor, offset) else {
        return (Vec::new(), None);
    };
    let keywords = |words| (keyword_completions(words), None);
    let start = reference_start(&cursor.left, offset);
    let replaced = Some(TextRange::new(start, offset));
    // Whether the part already written qualifies the reference with the
    // file's own package, as in `veh.topology.Cr`.
    let own_qualified = file
        .text(db)
        .get(usize::from(start)..usize::from(offset))
        .is_some_and(|written| written.starts_with(&format!("{}.", pkg.name(db))));
    let items = match slot {
        RsdlSlot::TopLevel => return keywords(RSDL_DEFINITION_KEYWORDS),
        RsdlSlot::ComponentLine => return keywords(COMPONENT_LINE_KEYWORDS),
        RsdlSlot::DeploymentLine => return keywords(DEPLOYMENT_LINE_KEYWORDS),
        RsdlSlot::Offers => service_items(db, ws, std, |_, _| true),
        RsdlSlot::Requires => {
            let resolution = resolve_package(db, ws, pkg, std);
            let mut items: Vec<lt::CompletionItem> = resolution
                .symbols
                .iter()
                .filter(|(_, symbol)| symbol.kind == SymbolKind::Interface)
                .map(|(name, symbol)| {
                    item(
                        name,
                        lt::CompletionItemKind::INTERFACE,
                        format!("{}.{}", symbol.package, symbol.name),
                    )
                })
                .collect();
            items.extend(service_items(db, ws, std, |_, entry| entry.inline));
            items
        }
        RsdlSlot::For => {
            let system = check_system(db, ws, std);
            let own = pkg.name(db);
            system
                .systems
                .iter()
                .map(|decl| {
                    rsdl_item(
                        own,
                        own_qualified,
                        &decl.package,
                        &decl.name.name,
                        lt::CompletionItemKind::MODULE,
                        format!("system {}.{}", decl.package, decl.name.name),
                    )
                })
                .collect()
        }
        RsdlSlot::Member { placement } => {
            let system = check_system(db, ws, std);
            let own = pkg.name(db);
            let mut items = Vec::new();
            for decl in &system.components {
                let name = &decl.name.name;
                let detail = format!("component {}.{name}", decl.package);
                if placement {
                    for instance in decl.instances.iter().flatten() {
                        items.push(rsdl_item(
                            own,
                            own_qualified,
                            &decl.package,
                            &format!("{name}.{}", instance.name),
                            lt::CompletionItemKind::FIELD,
                            format!("instance of {detail}"),
                        ));
                    }
                }
                items.push(rsdl_item(
                    own,
                    own_qualified,
                    &decl.package,
                    name,
                    lt::CompletionItemKind::CLASS,
                    detail,
                ));
            }
            // A service a declared component offers stands for no member
            // (RSDL-504).
            let offered = |name: &str| {
                system
                    .component_lines
                    .iter()
                    .any(|lines| lines.offers.iter().flatten().any(|service| service == name))
            };
            items.extend(service_items(db, ws, std, |name, _| !offered(name)));
            items
        }
    };
    (sorted(items), replaced)
}

/// The services of the workspace catalog that `keep` accepts.
fn service_items(
    db: &dyn salsa::Database,
    ws: Workspace,
    std: Package,
    keep: impl Fn(&str, &ridl_core::package::CatalogEntry) -> bool,
) -> Vec<lt::CompletionItem> {
    service_catalog(db, ws, std)
        .entries
        .iter()
        .filter(|(name, entry)| keep(name, entry))
        .map(|(name, entry)| {
            item(
                name,
                lt::CompletionItemKind::MODULE,
                format!("service of {}", entry.package),
            )
        })
        .collect()
}

/// The item for `name`, an rsdl declaration of `package`, as a reference
/// written in the package `own` names it: bare in its own package, qualified
/// in another. When `own_qualified`, the part already written qualifies the
/// reference with the own package, and an item of the own package carries
/// the qualified name as its `filter_text`, so that it still matches and is
/// written qualified.
fn rsdl_item(
    own: &str,
    own_qualified: bool,
    package: &str,
    name: &str,
    kind: lt::CompletionItemKind,
    detail: String,
) -> lt::CompletionItem {
    let qualified = format!("{package}.{name}");
    if own != package {
        item(&qualified, kind, detail)
    } else if own_qualified {
        lt::CompletionItem {
            filter_text: Some(qualified),
            ..item(name, kind, detail)
        }
    } else {
        item(name, kind, detail)
    }
}

/// The start of the dotted reference the cursor is writing: the first of the
/// name segments and dots that run, with no space, up to `offset`. `offset`
/// itself when no segment is written yet.
///
/// A keyword is a segment only next to a dot (`veh.system.x`), so the
/// `offers`, `requires` or `for` the cursor sits right after is never part of
/// the reference.
fn reference_start(left: &SyntaxToken, offset: TextSize) -> TextSize {
    let mut start = offset;
    let mut precedes_dot = false;
    let mut current = Some(left.clone());
    while let Some(token) = current {
        let range = token.text_range();
        let dot = token.kind() == SyntaxKind::Dot;
        let word = token
            .text()
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_');
        let follows_dot = token
            .prev_token()
            .is_some_and(|previous| previous.kind() == SyntaxKind::Dot);
        let segment =
            dot || (word && (token.kind() == SyntaxKind::Ident || precedes_dot || follows_dot));
        if !segment || range.start() >= start || range.end() < start {
            break;
        }
        start = range.start();
        precedes_dot = dot;
        current = token.prev_token();
    }
    start
}

/// The rsdl slots completion recognises.
enum RsdlSlot {
    /// The top level of the file — a declaration keyword.
    TopLevel,
    /// The start of a component line — `offers` or `requires`.
    ComponentLine,
    /// The start of a deployment line — `machine`.
    DeploymentLine,
    /// The reference of an `offers` line — a service.
    Offers,
    /// The reference of a `requires` line — an interface, or a service with an
    /// inline shape.
    Requires,
    /// The reference after `for` — a system.
    For,
    /// A member line; `placement` for the member line of a `machine`.
    Member { placement: bool },
}

/// Decides the rsdl slot from the tree around `offset`, or `None` when the
/// cursor is not in a position the server completes, such as the name of a
/// declaration or an attribute block.
fn rsdl_slot(cursor: &Cursor, offset: TextSize) -> Option<RsdlSlot> {
    // A keyword after a dot is a segment of a reference (`veh.offers`), not
    // the keyword that opens a slot.
    let keyword = cursor.anchor.as_ref().filter(|anchor| {
        anchor
            .prev_token()
            .is_none_or(|previous| previous.kind() != SyntaxKind::Dot)
    });
    match keyword.map(SyntaxToken::kind) {
        Some(SyntaxKind::OffersKw) => return Some(RsdlSlot::Offers),
        Some(SyntaxKind::RequiresKw) => return Some(RsdlSlot::Requires),
        Some(SyntaxKind::ForKw) => return Some(RsdlSlot::For),
        // Naming positions: the user is typing the declared name.
        Some(
            SyntaxKind::SystemKw
            | SyntaxKind::ComponentKw
            | SyntaxKind::DistributionKw
            | SyntaxKind::DeploymentKw
            | SyntaxKind::MachineKw
            | SyntaxKind::PackageKw,
        ) => return None,
        _ => {}
    }
    let token = cursor.word.as_ref().unwrap_or(&cursor.left);
    for node in token.parent_ancestors() {
        match node.kind() {
            SyntaxKind::ErrorNode | SyntaxKind::QualifiedName => {}
            SyntaxKind::Reference => {
                if node
                    .parent()
                    .is_some_and(|parent| parent.kind() == SyntaxKind::DeploymentDef)
                {
                    return Some(RsdlSlot::For);
                }
            }
            SyntaxKind::ComponentLine => {
                let keyword = node.first_token()?;
                // On the keyword itself, the line's start is being written.
                if keyword.text_range().contains_inclusive(offset) {
                    return Some(RsdlSlot::ComponentLine);
                }
                return match keyword.kind() {
                    SyntaxKind::OffersKw => Some(RsdlSlot::Offers),
                    SyntaxKind::RequiresKw => Some(RsdlSlot::Requires),
                    _ => None,
                };
            }
            SyntaxKind::MemberLine => {
                let placement = node
                    .parent()
                    .is_some_and(|parent| parent.kind() == SyntaxKind::MachineDef);
                return Some(RsdlSlot::Member { placement });
            }
            SyntaxKind::SystemDef | SyntaxKind::DistributionDef => {
                return in_body(&node, offset).then_some(RsdlSlot::Member { placement: false });
            }
            SyntaxKind::MachineDef => {
                return in_body(&node, offset).then_some(RsdlSlot::Member { placement: true });
            }
            SyntaxKind::ComponentDef => {
                return in_body(&node, offset).then_some(RsdlSlot::ComponentLine);
            }
            SyntaxKind::DeploymentDef => {
                return in_body(&node, offset).then_some(RsdlSlot::DeploymentLine);
            }
            SyntaxKind::SourceFile => return Some(RsdlSlot::TopLevel),
            _ => return None,
        }
    }
    None
}

/// Whether `offset` is inside the braces of the container `node`: after its
/// `{` and before its `}`, when it has one.
fn in_body(node: &SyntaxNode, offset: TextSize) -> bool {
    let tokens = || {
        node.children_with_tokens()
            .filter_map(|element| element.into_token())
    };
    let opened = tokens()
        .any(|token| token.kind() == SyntaxKind::LBrace && token.text_range().end() <= offset);
    let closed = tokens()
        .any(|token| token.kind() == SyntaxKind::RBrace && token.text_range().start() < offset);
    opened && !closed
}

/// The tokens around the cursor that completion reads its context from.
struct Cursor {
    /// The token to the left of the cursor.
    left: SyntaxToken,
    /// The identifier the cursor is completing, when the left token is a
    /// partial word the cursor sits inside.
    word: Option<SyntaxToken>,
    /// The token that introduces this position: the significant token before
    /// the partial word, or the significant token at or before the left token.
    anchor: Option<SyntaxToken>,
}

impl Cursor {
    fn at(root: &SyntaxNode, offset: TextSize) -> Option<Cursor> {
        let left = left_token(root, offset)?;
        let word = (left.kind() == SyntaxKind::Ident && left.text_range().start() < offset)
            .then(|| left.clone());
        let anchor = match &word {
            Some(word) => word
                .prev_token()
                .and_then(|token| significant_at_or_before(&token)),
            None => significant_at_or_before(&left),
        };
        Some(Cursor { left, word, anchor })
    }

    /// Whether the cursor is inside an `import` statement: the keyword, the
    /// path, or the trailing whitespace the parser attaches to the `Import`
    /// node.
    fn in_import(&self) -> bool {
        self.left.parent_ancestors().any(is_import)
            || self
                .anchor
                .as_ref()
                .is_some_and(|token| token.kind() == SyntaxKind::ImportKw)
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
    /// Directly inside an interface body or a service's inline shape — an
    /// interaction keyword or `reserved`.
    InteractionStart,
}

/// Decides the completion context from the tree around `offset`, or `None` when
/// the cursor is not in a position the server completes.
fn context(root: &SyntaxNode, offset: TextSize) -> Option<Context> {
    let cursor = Cursor::at(root, offset)?;
    if cursor.in_import() {
        return Some(Context::Import);
    }
    let Cursor { left, word, anchor } = cursor;

    if let Some(anchor) = &anchor {
        match anchor.kind() {
            SyntaxKind::Colon => return Some(Context::Type),
            SyntaxKind::MatchKw => return Some(Context::Match),
            // Naming positions: right after a definition or interaction
            // keyword the user is typing the declared name, so offer nothing.
            SyntaxKind::TypeKw
            | SyntaxKind::ConstKw
            | SyntaxKind::StructKw
            | SyntaxKind::EnumKw
            | SyntaxKind::EnumsetKw
            | SyntaxKind::UnionKw
            | SyntaxKind::InterfaceKw
            | SyntaxKind::ServiceKw
            | SyntaxKind::SignalKw
            | SyntaxKind::EventKw
            | SyntaxKind::CommandKw
            | SyntaxKind::QueryKw
            | SyntaxKind::FixedKw
            | SyntaxKind::ReservedKw => return None,
            _ => {}
        }
    }

    // Interaction start: the cursor sits directly in an interface body or a
    // service's inline shape.
    if is_interaction_start(word.as_ref().unwrap_or(&left)) {
        return Some(Context::InteractionStart);
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

/// Whether `token` sits directly inside an `interface` body or a service's
/// inline shape — its nearest node ancestor is the `InterfaceDef` or
/// `ServiceDef`, seen through any recovery `ErrorNode` wrappers. A token nested
/// deeper (inside an interaction's own node) is not at a member start.
fn is_interaction_start(token: &SyntaxToken) -> bool {
    let mut node = token.parent();
    while let Some(current) = node {
        match current.kind() {
            SyntaxKind::InterfaceDef | SyntaxKind::ServiceDef => return true,
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
    let mut docs = SymbolDocs::new(db, ws, std, pkg);
    let mut items: Vec<lt::CompletionItem> = resolution
        .symbols
        .iter()
        .filter(|(_, symbol)| symbol.kind != SymbolKind::Const)
        .map(|(name, symbol)| {
            documented(
                item(
                    name,
                    type_kind(symbol.kind),
                    format!("{}.{}", symbol.package, symbol.name),
                ),
                docs.get(symbol),
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
    let mut items = package_items(db, packages);

    // The package path completed before the cursor (everything but the segment
    // under the cursor). When it names a known package, offer its public
    // symbols.
    if let Some(prefix) = import_path_prefix(source, offset)
        && let Some(target) = packages.iter().find(|package| *package.name(db) == prefix)
    {
        items.extend(public_symbol_items(db, ws, std, *target));
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
    let mut docs = SymbolDocs::new(db, ws, std, pkg);
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
        .map(|(name, symbol)| {
            documented(
                item(name, lt::CompletionItemKind::CONSTANT, "regex".to_string()),
                docs.get(symbol),
            )
        })
        .collect();
    sorted(items)
}

/// The doc-link targets reachable after `[` and the `prefix` typed so far
/// (ADR-0026). The segment under the cursor is dropped; what precedes it
/// decides the list: nothing → the names in the package's view and the known
/// packages; a declaration's path → its members; a package's path → its
/// public declarations.
fn doc_link_completions(
    db: &dyn salsa::Database,
    ws: Workspace,
    std: Package,
    pkg: Package,
    packages: &[Package],
    prefix: &str,
) -> Vec<lt::CompletionItem> {
    let resolution = resolve_package(db, ws, pkg, std);
    let mut path: Vec<String> = prefix.split('.').map(str::to_string).collect();
    path.pop();
    let mut items: Vec<lt::CompletionItem> = Vec::new();
    if path.is_empty() {
        let mut docs = SymbolDocs::new(db, ws, std, pkg);
        for (name, symbol) in &resolution.symbols {
            items.push(documented(
                item(
                    name,
                    symbol_kind(symbol.kind),
                    format!("{}.{}", symbol.package, symbol.name),
                ),
                docs.get(symbol),
            ));
        }
        items.extend(package_items(db, packages));
        return sorted(items);
    }
    if let Ok(LinkTarget {
        symbol,
        member: None,
    }) = resolve_doc_link(db, ws, std, pkg, &resolution, &path)
    {
        let owner = format!("{}.{}", symbol.package, symbol.name);
        let kind = match symbol.kind {
            SymbolKind::Enum | SymbolKind::EnumSet => lt::CompletionItemKind::ENUM_MEMBER,
            SymbolKind::Interface => lt::CompletionItemKind::METHOD,
            _ => lt::CompletionItemKind::FIELD,
        };
        for member in doc_link_members(db, ws, std, pkg, &symbol) {
            items.push(item(&member, kind, format!("{owner}.{member}")));
        }
        return sorted(items);
    }
    let path = path.join(".");
    if let Some(target) = packages.iter().find(|package| *package.name(db) == path) {
        items.extend(public_symbol_items(db, ws, std, *target));
    }
    sorted(items)
}

/// One module item per distinct non-empty package name in `packages`.
fn package_items(db: &dyn salsa::Database, packages: &[Package]) -> Vec<lt::CompletionItem> {
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
    items
}

/// The public declarations of `target`, each with its doc.
fn public_symbol_items(
    db: &dyn salsa::Database,
    ws: Workspace,
    std: Package,
    target: Package,
) -> Vec<lt::CompletionItem> {
    let target_name = target.name(db).clone();
    let resolution = resolve_package(db, ws, target, std);
    let mut docs = SymbolDocs::new(db, ws, std, target);
    let mut items = Vec::new();
    for (name, symbol) in &resolution.symbols {
        if symbol.package == target_name && !symbol.internal {
            items.push(documented(
                item(
                    name,
                    symbol_kind(symbol.kind),
                    format!("{target_name}.{name}"),
                ),
                docs.get(symbol),
            ));
        }
    }
    items
}

/// A fixed keyword list, kind-annotated as keywords.
fn keyword_completions(keywords: &[&str]) -> Vec<lt::CompletionItem> {
    keywords
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
        SymbolKind::Interface => lt::CompletionItemKind::INTERFACE,
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

/// Sets an item's `documentation` to `doc`, as Markdown, when there is one.
fn documented(mut item: lt::CompletionItem, doc: Option<String>) -> lt::CompletionItem {
    item.documentation = doc.map(|value| {
        lt::Documentation::MarkupContent(lt::MarkupContent {
            kind: lt::MarkupKind::Markdown,
            value,
        })
    });
    item
}

/// Sorts items by label for a deterministic list (resolution is a `HashMap`).
fn sorted(mut items: Vec<lt::CompletionItem>) -> Vec<lt::CompletionItem> {
    items.sort_by(|a, b| a.label.cmp(&b.label));
    items
}
