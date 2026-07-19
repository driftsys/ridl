//! Name resolution (docs/ROADMAP.md epic E1.4).
//!
//! [`resolve_package`] builds a package's local view of names (its own
//! declarations, the implicit `ridl.std` names, and the alias-aware imports)
//! and reports the module diagnostics ADR-0002 §2 and §5–6 define —
//! wildcard/relative imports (TYPL-003), cross-package cycles (TYPL-004),
//! conflicting imports (TYPL-006), unused imports (TYPL-007), needless
//! aliases (TYPL-008), and duplicate declarations (TYPL-009). It resolves
//! package references in the fixed order of ADR-0002 §5: workspace member →
//! the package's own `[imports]` → the workspace `[imports]` → error.
//!
//! The E0.5 single-file resolver lived here until the checker moved onto the
//! package model (E1.7a); the package checker (`check`) is its replacement.
//!
//! Reads the `family.ungram`-generated typed AST (`ridl_syntax::ast`).

use std::collections::hash_map::Entry;
use std::collections::{BTreeMap, HashMap, HashSet};

use ridl_core::db::InputFile;
use ridl_core::diag::{DiagCode, Diagnostic, FileId, Severity, SourceMap, Span};
use ridl_core::package::{Package, Workspace, package_of};
use ridl_core::parse_file;
use ridl_syntax::ast::{
    AstNode, Definition, HasModifiers, HasName, Import, InterfaceDef, QualifiedName, SourceFile,
};
use ridl_syntax::{SyntaxKind, SyntaxNode};
use rowan::TextRange;

/// The kind of a declared name — one variant per typl definition keyword,
/// plus the ridl `interface` declaration (E2.1b).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SymbolKind {
    Type,
    Const,
    Struct,
    Enum,
    EnumSet,
    Union,
    Interface,
}

// ==========================================================================
// The E1.4 package resolver
// ==========================================================================

/// One name visible inside a package, resolved to its defining declaration.
///
/// `name` and `package` name the declaration at its **definition site** — for
/// an aliased import the map key is the local alias while `name`/`package`
/// still point at the original declaration, so an editor can jump to the real
/// source. `file`/`range` are that declaration's site. `internal` records the
/// `internal` modifier (TYPL-005 exposure is enforced by the checker in E1.7);
/// `is_error` records the `error` modifier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Symbol {
    pub name: String,
    pub package: String,
    pub kind: SymbolKind,
    pub internal: bool,
    pub is_error: bool,
    pub file: InputFile,
    pub range: TextRange,
}

/// A package's resolved local view: every name it can reference by a bare
/// identifier (its own declarations, `ridl.std`, and imported names, first
/// wins) plus the module diagnostics resolution raised.
///
/// The diagnostics' spans carry a [`FileId`] indexing `pkg.files(db)` in order
/// (every resolver diagnostic points into one of the package's own files); a
/// renderer reconstructs the source map by interning `pkg.files(db)` in the
/// same order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Resolution {
    pub symbols: HashMap<String, Symbol>,
    pub diagnostics: Vec<Diagnostic>,
}

/// Resolves the names visible inside `pkg` (ADR-0002 §2, §5–6; typl §3.2–3.4).
///
/// `std` is the embedded `ridl.std` package. It is threaded in as a parameter
/// rather than reached through `ws` because `ridl.std` is deliberately absent
/// from [`Workspace::packages`] and its constructor
/// ([`std_package`](ridl_core::std_package)) needs `&mut RidlDatabase`, which a
/// tracked query cannot hold. Every package implicitly imports all of
/// `ridl.std` (typl §3.2).
#[salsa::tracked(returns(clone))]
pub fn resolve_package(
    db: &dyn salsa::Database,
    ws: Workspace,
    pkg: Package,
    std: Package,
) -> Resolution {
    let files = pkg.files(db);
    let package_name = pkg.name(db).clone();

    // Resolver diagnostics all point into the package's own files; their
    // FileId indexes pkg.files(db) in order.
    let mut sources = SourceMap::new();
    let file_ids: Vec<FileId> = files
        .iter()
        .map(|file| sources.file_id(file.path(db), file.text(db)))
        .collect();

    let mut diagnostics = Vec::new();
    let mut symbols: HashMap<String, Symbol> = HashMap::new();

    // 1. Local declarations, in source order across both declaration shapes
    //    (typl definitions and ridl interfaces). First declaration wins
    //    everywhere (ADR-0007 decision 6); a later duplicate is TYPL-009 at
    //    its own name.
    for (index, file) in files.iter().enumerate() {
        let source = source_file(db, *file);
        for declaration in declarations(&source) {
            let Some(name) = declaration.name() else {
                continue;
            };
            let range = declaration.name_range();
            match symbols.entry(name.clone()) {
                Entry::Vacant(entry) => {
                    entry.insert(declaration.symbol(name, &package_name, *file, range));
                }
                Entry::Occupied(_) => diagnostics.push(diagnostic(
                    DiagCode::TYPL_009,
                    Severity::Error,
                    file_ids[index],
                    range,
                    format!("duplicate declaration of `{name}`"),
                )),
            }
        }
    }
    // Local names win over imports, which win over the implicit `ridl.std`.
    let locals: HashSet<String> = symbols.keys().cloned().collect();

    // 2. Imports: collect, then analyse collisions/usage/cycles.
    let records = collect_imports(db, ws, pkg, &file_ids, &mut diagnostics);
    let used = collect_used_names(db, files);
    apply_imports(&records, &locals, &used, &mut symbols, &mut diagnostics);
    detect_cycles(db, ws, pkg, &records, &mut diagnostics);

    // 3. Implicit `ridl.std` names, where nothing already binds.
    for (name, symbol) in declared_symbols(db, std) {
        if symbol.internal {
            continue;
        }
        symbols.entry(name).or_insert(symbol);
    }

    Resolution {
        symbols,
        diagnostics,
    }
}

/// One import as the resolver understands it, after shape detection and §5
/// package resolution.
struct ImportRecord {
    /// The name the import binds locally — the alias, or the last path segment.
    local_name: String,
    /// The last path segment (what the local name would be without an alias).
    base_name: String,
    aliased: bool,
    /// The package path (every segment except the last).
    package_path: String,
    file_id: FileId,
    range: TextRange,
    /// The resolved declaration, when the path names a workspace member that
    /// exposes the imported symbol.
    bound: Option<Symbol>,
    /// The workspace member the path resolves to (for cycle detection).
    member: Option<Package>,
    /// The URL a remote import resolves to (not materialized until E1.6).
    remote_url: Option<String>,
    /// The path resolves nowhere (ADR-0002 §5 step 4).
    unknown: bool,
}

/// Collects every import of `pkg`, emitting TYPL-003 for wildcard/relative
/// imports and resolving each named import's package path per ADR-0002 §5.
fn collect_imports(
    db: &dyn salsa::Database,
    ws: Workspace,
    pkg: Package,
    file_ids: &[FileId],
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<ImportRecord> {
    let mut records = Vec::new();
    for (index, file) in pkg.files(db).iter().enumerate() {
        let source = source_file(db, *file);
        for import in source.imports() {
            let range = import.syntax().text_range();
            let file_id = file_ids[index];
            match import_shape(&import) {
                ImportShape::Wildcard => diagnostics.push(diagnostic(
                    DiagCode::TYPL_003,
                    Severity::Error,
                    file_id,
                    range,
                    "wildcard imports are not permitted".to_string(),
                )),
                ImportShape::Relative => diagnostics.push(diagnostic(
                    DiagCode::TYPL_003,
                    Severity::Error,
                    file_id,
                    range,
                    "relative imports are not permitted".to_string(),
                )),
                ImportShape::Malformed => {}
                ImportShape::Named {
                    package_path,
                    symbol_name,
                } => {
                    let aliased = import.as_token().is_some();
                    let local_name = import
                        .alias()
                        .and_then(|alias| alias.ident_token())
                        .map(|token| token.text().to_string())
                        .unwrap_or_else(|| symbol_name.clone());
                    let (bound, member, remote_url, unknown) =
                        match resolve_source(db, ws, pkg, &package_path) {
                            PackageSource::Member(member) => {
                                let bound = declared_symbols(db, member)
                                    .get(&symbol_name)
                                    .filter(|symbol| !symbol.internal)
                                    .cloned();
                                (bound, Some(member), None, false)
                            }
                            PackageSource::Remote(url) => (None, None, Some(url), false),
                            PackageSource::Unknown => (None, None, None, true),
                        };
                    records.push(ImportRecord {
                        local_name,
                        base_name: symbol_name,
                        aliased,
                        package_path,
                        file_id,
                        range,
                        bound,
                        member,
                        remote_url,
                        unknown,
                    });
                }
            }
        }
    }
    records
}

/// Analyses the collected imports: binds the winners, and raises the
/// collision (TYPL-006), needless-alias (TYPL-008), unused (TYPL-007),
/// unresolved-remote (E1.6 pending), and unresolved-package (§5 step 4)
/// diagnostics.
fn apply_imports(
    records: &[ImportRecord],
    locals: &HashSet<String>,
    used: &HashSet<String>,
    symbols: &mut HashMap<String, Symbol>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut local_name_counts: HashMap<&str, usize> = HashMap::new();
    let mut base_name_counts: HashMap<&str, usize> = HashMap::new();
    for record in records {
        *local_name_counts
            .entry(record.local_name.as_str())
            .or_default() += 1;
        *base_name_counts
            .entry(record.base_name.as_str())
            .or_default() += 1;
    }

    let mut seen_local: HashSet<&str> = HashSet::new();
    for record in records {
        let first_for_name = !seen_local.contains(record.local_name.as_str());

        if let Some(url) = &record.remote_url {
            // The import itself is valid; remote materialization is not yet
            // wired into the compile path, so the remote package is not
            // available here (a known limitation, tracked as epic debt).
            // Informational, no code.
            diagnostics.push(diagnostic(
                DiagCode::NONE,
                Severity::Info,
                record.file_id,
                record.range,
                format!(
                    "remote import `{}` is not yet available: `{url}` (remote materialization is not yet wired into the compile path)",
                    record.package_path
                ),
            ));
        }
        if record.unknown {
            diagnostics.push(diagnostic(
                DiagCode::NONE,
                Severity::Error,
                record.file_id,
                record.range,
                format!(
                    "unresolved import: `{}` is not a workspace member and not declared in `[imports]`",
                    record.package_path
                ),
            ));
        }

        // TYPL-006: two imports bind the same local name and neither is aliased
        // to a distinct one. The first keeps the name; each later one is flagged.
        let colliding = local_name_counts
            .get(record.local_name.as_str())
            .is_some_and(|count| *count >= 2);
        if colliding && !first_for_name {
            diagnostics.push(diagnostic(
                DiagCode::TYPL_006,
                Severity::Error,
                record.file_id,
                record.range,
                format!(
                    "conflicting import of `{}`: alias one of the colliding imports",
                    record.local_name
                ),
            ));
        }

        // TYPL-008: an alias that resolves no collision — its base name is not
        // shared by another import and does not clash with a local declaration.
        if record.aliased {
            let base_shared = base_name_counts
                .get(record.base_name.as_str())
                .is_some_and(|count| *count >= 2);
            if !base_shared && !locals.contains(&record.base_name) {
                diagnostics.push(diagnostic(
                    DiagCode::TYPL_008,
                    Severity::Warning,
                    record.file_id,
                    record.range,
                    format!(
                        "import alias `{}` is not needed: `{}` does not collide",
                        record.local_name, record.base_name
                    ),
                ));
            }
        }

        // TYPL-007: an import that binds a real symbol but is never referenced.
        if record.bound.is_some() && !used.contains(record.local_name.as_str()) {
            diagnostics.push(diagnostic(
                DiagCode::TYPL_007,
                Severity::Warning,
                record.file_id,
                record.range,
                format!("unused import `{}`", record.local_name),
            ));
        }

        // Bind the winner. A local declaration shadows an import; among
        // colliding imports the first wins.
        if let Some(symbol) = &record.bound
            && first_for_name
            && !locals.contains(&record.local_name)
        {
            symbols.insert(record.local_name.clone(), symbol.clone());
        }

        seen_local.insert(record.local_name.as_str());
    }
}

/// Emits TYPL-004 for every import of `pkg` that closes a cross-package cycle
/// (ADR-0002 §6): the imported member can reach `pkg` again through member
/// import edges.
fn detect_cycles(
    db: &dyn salsa::Database,
    ws: Workspace,
    pkg: Package,
    records: &[ImportRecord],
    diagnostics: &mut Vec<Diagnostic>,
) {
    for record in records {
        let Some(member) = record.member else {
            continue;
        };
        // Importing the package's own name is not a cross-package cycle.
        if member == pkg {
            continue;
        }
        if reaches(db, ws, member, pkg, &mut HashSet::new()) {
            diagnostics.push(diagnostic(
                DiagCode::TYPL_004,
                Severity::Error,
                record.file_id,
                record.range,
                format!(
                    "circular package import: `{}` and `{}` import each other",
                    pkg.name(db),
                    record.package_path
                ),
            ));
        }
    }
}

/// Whether `from` can reach `target` by following member import edges.
fn reaches(
    db: &dyn salsa::Database,
    ws: Workspace,
    from: Package,
    target: Package,
    visited: &mut HashSet<Package>,
) -> bool {
    if !visited.insert(from) {
        return false;
    }
    for edge in member_edges(db, ws, from) {
        if edge == target || reaches(db, ws, edge, target, visited) {
            return true;
        }
    }
    false
}

/// The workspace members `package` imports (its member import edges). Only
/// member edges can form an in-workspace cycle; remote and unresolved imports
/// cannot.
fn member_edges(db: &dyn salsa::Database, ws: Workspace, package: Package) -> Vec<Package> {
    let mut edges = Vec::new();
    for file in package.files(db) {
        let source = source_file(db, *file);
        for import in source.imports() {
            if let ImportShape::Named { package_path, .. } = import_shape(&import)
                && let Some(member) = package_of(db, ws, package_path)
                && member != package
            {
                edges.push(member);
            }
        }
    }
    edges
}

/// Where a package reference resolves, following ADR-0002 §5 in order.
enum PackageSource {
    Member(Package),
    Remote(String),
    Unknown,
}

/// Resolves `package_path` per ADR-0002 §5: workspace member (step 1) → the
/// referencing package's own `[imports]` (step 2) → the workspace `[imports]`
/// (step 3) → error (step 4).
fn resolve_source(
    db: &dyn salsa::Database,
    ws: Workspace,
    pkg: Package,
    package_path: &str,
) -> PackageSource {
    if let Some(member) = package_of(db, ws, package_path.to_string()) {
        return PackageSource::Member(member);
    }
    if let Some(url) = longest_import_prefix(pkg.imports(db), package_path) {
        return PackageSource::Remote(url);
    }
    if let Some(url) = longest_import_prefix(ws.imports(db), package_path) {
        return PackageSource::Remote(url);
    }
    PackageSource::Unknown
}

/// The URL of the longest `[imports]` alias that equals `package_path` or is a
/// dot-separated prefix of it (ADR-0002 §5, "any longer prefix that matches").
fn longest_import_prefix(map: &BTreeMap<String, String>, package_path: &str) -> Option<String> {
    map.iter()
        .filter(|(key, _)| {
            package_path == key.as_str()
                || package_path
                    .strip_prefix(key.as_str())
                    .is_some_and(|rest| rest.starts_with('.'))
        })
        .max_by_key(|(key, _)| key.len())
        .map(|(_, url)| url.clone())
}

/// The shape of an import as parsed.
enum ImportShape {
    /// A trailing `.` or `.*` — a wildcard (TYPL-003).
    Wildcard,
    /// A leading `.` or `..` — a relative import (TYPL-003).
    Relative,
    /// A well-formed `package.Symbol` reference.
    Named {
        package_path: String,
        symbol_name: String,
    },
    /// Otherwise unusable; the parser already reported the syntax error.
    Malformed,
}

/// Classifies an import by its path shape. Resolution reads the path from the
/// node text (not `QualifiedName::ident_token`), so keyword path segments
/// (`veh.integer`) resolve correctly.
fn import_shape(import: &Import) -> ImportShape {
    match import.qualified_name() {
        Some(qualified) => {
            // A dangling trailing `.` is the signature of `a.b.*` and `a.b.`
            // (the `*` recovers as a sibling error node).
            if last_significant_is_dot(&qualified) {
                return ImportShape::Wildcard;
            }
            let mut segments = qualified_segments(&qualified);
            if segments.len() < 2 {
                return ImportShape::Malformed;
            }
            let symbol_name = segments.pop().expect("length checked to be >= 2");
            ImportShape::Named {
                package_path: segments.join("."),
                symbol_name,
            }
        }
        None if import_has_relative_prefix(import) => ImportShape::Relative,
        None => ImportShape::Malformed,
    }
}

/// The dot-separated segments of a qualified name, by node text — keyword
/// segments (`integer`, `float`) are kept verbatim.
pub(crate) fn qualified_segments(qualified: &QualifiedName) -> Vec<String> {
    let mut segments = Vec::new();
    let mut current = String::new();
    for element in qualified.syntax().children_with_tokens() {
        let Some(token) = element.into_token() else {
            continue;
        };
        if token.kind().is_trivia() {
            continue;
        }
        if token.kind() == SyntaxKind::Dot {
            segments.push(std::mem::take(&mut current));
        } else {
            current.push_str(token.text());
        }
    }
    segments.push(current);
    segments
}

/// Whether the last non-trivia token of a qualified name is a `.` (a dangling
/// path, the mark of a wildcard import).
fn last_significant_is_dot(qualified: &QualifiedName) -> bool {
    qualified
        .syntax()
        .children_with_tokens()
        .filter_map(|element| element.into_token())
        .filter(|token| !token.kind().is_trivia())
        .last()
        .is_some_and(|token| token.kind() == SyntaxKind::Dot)
}

/// Whether the import's path recovered as a leading-`.`/`..` error node — a
/// relative import the grammar does not admit.
fn import_has_relative_prefix(import: &Import) -> bool {
    let mut sibling = import.syntax().next_sibling_or_token();
    while let Some(element) = sibling {
        match element {
            rowan::NodeOrToken::Token(token) if token.kind().is_trivia() => {
                sibling = token.next_sibling_or_token();
            }
            rowan::NodeOrToken::Node(node) if node.kind() == SyntaxKind::ErrorNode => {
                return node
                    .children_with_tokens()
                    .filter_map(|element| element.into_token())
                    .find(|token| !token.kind().is_trivia())
                    .is_some_and(|token| {
                        matches!(token.kind(), SyntaxKind::Dot | SyntaxKind::DotDot)
                    });
            }
            _ => return false,
        }
    }
    false
}

/// Every name referenced by a bare identifier in the package's bodies — the
/// single-segment type references and named-constant references. Qualified
/// references (`pkg.Type`) do not consume a bare import, so they are excluded.
fn collect_used_names(db: &dyn salsa::Database, files: &[InputFile]) -> HashSet<String> {
    let mut used = HashSet::new();
    for file in files {
        let source = source_file(db, *file);
        for node in source.syntax().descendants() {
            match node.kind() {
                // A named constant used as a bound or a `match` pattern is an
                // `Ident` inside a `Literal`.
                SyntaxKind::Literal => {
                    for token in node.children_with_tokens().filter_map(|e| e.into_token()) {
                        if token.kind() == SyntaxKind::Ident {
                            used.insert(token.text().to_string());
                        }
                    }
                }
                SyntaxKind::QualifiedName => {
                    if node
                        .ancestors()
                        .any(|a| matches!(a.kind(), SyntaxKind::PackageDecl | SyntaxKind::Import))
                    {
                        continue;
                    }
                    let qualified = QualifiedName::cast(node).expect("kind checked");
                    let mut segments = qualified_segments(&qualified);
                    if segments.len() == 1
                        && let Some(name) = segments.pop()
                    {
                        used.insert(name);
                    }
                }
                _ => {}
            }
        }
    }
    used
}

/// Every declaration in `package`, first wins, as bare-name symbols. Used for
/// `ridl.std` and for looking up an imported symbol in its defining package;
/// duplicate diagnostics for `package` are raised when `package` is resolved
/// in its own right, not here.
pub(crate) fn declared_symbols(
    db: &dyn salsa::Database,
    package: Package,
) -> HashMap<String, Symbol> {
    let package_name = package.name(db).clone();
    let mut symbols: HashMap<String, Symbol> = HashMap::new();
    for file in package.files(db) {
        let source = source_file(db, *file);
        for declaration in declarations(&source) {
            let Some(name) = declaration.name() else {
                continue;
            };
            let range = declaration.name_range();
            symbols
                .entry(name.clone())
                .or_insert_with(|| declaration.symbol(name, &package_name, *file, range));
        }
    }
    symbols
}

/// A named top-level declaration — a typl definition or a ridl interface
/// (E2.1b). [`declarations`] yields them in source order, so the first-wins
/// tiebreak (ADR-0007 decision 6) holds across the two shapes.
pub(crate) enum Declaration {
    Definition(Definition),
    Interface(InterfaceDef),
}

/// The declarations of a source file, in source order.
pub(crate) fn declarations(source: &SourceFile) -> impl Iterator<Item = Declaration> + use<> {
    source.syntax().children().filter_map(|node| {
        if let Some(definition) = Definition::cast(node.clone()) {
            return Some(Declaration::Definition(definition));
        }
        InterfaceDef::cast(node).map(Declaration::Interface)
    })
}

impl Declaration {
    fn name(&self) -> Option<String> {
        match self {
            Self::Definition(definition) => declared_name(definition),
            Self::Interface(interface) => declared_name(interface),
        }
    }

    fn name_range(&self) -> TextRange {
        match self {
            Self::Definition(definition) => name_range(definition),
            Self::Interface(interface) => name_range(interface),
        }
    }

    /// The [`Symbol`] this declaration binds.
    fn symbol(
        &self,
        name: String,
        package_name: &str,
        file: InputFile,
        range: TextRange,
    ) -> Symbol {
        let (kind, internal, is_error) = match self {
            Self::Definition(definition) => (
                kind_of(definition),
                definition.is_internal(),
                definition.is_error(),
            ),
            Self::Interface(interface) => (
                SymbolKind::Interface,
                interface.is_internal(),
                interface.is_error(),
            ),
        };
        Symbol {
            name,
            package: package_name.to_string(),
            kind,
            internal,
            is_error,
            file,
            range,
        }
    }
}

/// The parsed [`SourceFile`] of one input, through the memoized parse query.
pub(crate) fn source_file(db: &dyn salsa::Database, file: InputFile) -> SourceFile {
    let parse = parse_file(db, file);
    SourceFile::cast(parse.syntax()).expect("the parser roots every tree in a SourceFile")
}

/// The [`SymbolKind`] of a definition.
fn kind_of(definition: &Definition) -> SymbolKind {
    match definition {
        Definition::Type(_) => SymbolKind::Type,
        Definition::Const(_) => SymbolKind::Const,
        Definition::Struct(_) => SymbolKind::Struct,
        Definition::Enum(_) => SymbolKind::Enum,
        Definition::EnumSet(_) => SymbolKind::EnumSet,
        Definition::Union(_) => SymbolKind::Union,
    }
}

/// Builds a resolver [`Diagnostic`]; resolver diagnostics carry no secondary
/// labels or fix-its.
fn diagnostic(
    code: DiagCode,
    severity: Severity,
    file: FileId,
    range: TextRange,
    message: String,
) -> Diagnostic {
    Diagnostic {
        code,
        severity,
        message,
        primary: Span { file, range },
        labels: Vec::new(),
        fixits: Vec::new(),
    }
}

// --- shared AST helpers (also used by the package checker) ----------------

/// The declared name of a definition, or `None` on a malformed tree.
pub(crate) fn declared_name(definition: &impl HasName) -> Option<String> {
    Some(definition.name()?.ident_token()?.text().to_string())
}

/// The source range of a definition's declared name, or an empty range on a
/// malformed tree (unreachable for a definition whose [`declared_name`] is
/// present).
pub(crate) fn name_range(definition: &impl HasName) -> TextRange {
    definition
        .name()
        .map(|name| name.syntax().text_range())
        .unwrap_or_default()
}

/// The concatenation of every non-trivia token in `node`'s subtree — the
/// node's text with whitespace and comments removed.
pub(crate) fn significant_text(node: &SyntaxNode) -> String {
    node.descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .filter(|token| !token.kind().is_trivia())
        .map(|token| token.text().to_string())
        .collect()
}

#[cfg(test)]
mod package_tests {
    use super::*;
    use ridl_core::db::RidlDatabase;
    use ridl_core::package::PackageOrigin;
    use ridl_core::std_lib::std_package;
    use salsa::Setter;

    /// Interns one source file.
    fn input(db: &RidlDatabase, path: &str, text: &str) -> InputFile {
        InputFile::new(db, path.to_string(), text.to_string())
    }

    /// A single-file workspace-member package with no `[imports]`.
    fn package(db: &RidlDatabase, name: &str, text: &str) -> Package {
        Package::new(
            db,
            name.to_string(),
            vec![input(db, &format!("{name}.typl"), text)],
            PackageOrigin::WorkspaceMember,
            BTreeMap::new(),
            None,
        )
    }

    /// A single-file package carrying its own `[imports]` map.
    fn package_with_imports(
        db: &RidlDatabase,
        name: &str,
        text: &str,
        imports: &[(&str, &str)],
    ) -> Package {
        Package::new(
            db,
            name.to_string(),
            vec![input(db, &format!("{name}.typl"), text)],
            PackageOrigin::WorkspaceMember,
            imports
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
            None,
        )
    }

    fn imports_map(imports: &[(&str, &str)]) -> BTreeMap<String, String> {
        imports
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    /// The diagnostic codes of a resolution, in order.
    fn codes(resolution: &Resolution) -> Vec<&str> {
        resolution
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic.code.as_str())
            .collect()
    }

    #[test]
    fn resolution_order_prefers_member_then_package_then_workspace_imports() {
        // ADR-0002 §5, mutation-checked: `app` imports `veh.common.Speed`, and
        // `veh.common` is simultaneously a workspace member, aliased in `app`'s
        // own `[imports]`, and aliased in the workspace `[imports]`.
        let mut db = RidlDatabase::default();
        let std = std_package(&mut db);

        let app = package_with_imports(
            &db,
            "app",
            "package app\nimport veh.common.Speed\nstruct S { p: Speed }\n",
            &[("veh.common", "https://registry.example.com/pkg@v1")],
        );
        let veh = package(&db, "veh.common", "package veh.common\ntype Speed: km/h\n");
        let ws = Workspace::new(
            &db,
            vec![app, veh],
            imports_map(&[("veh.common", "https://registry.example.com/ws@v1")]),
        );

        // Step 1: the workspace member wins — `Speed` binds to the real
        // declaration, no unresolved-remote note.
        let member_first = resolve_package(&db, ws, app, std);
        assert!(
            codes(&member_first).is_empty(),
            "member resolution is clean, got: {:?}",
            member_first.diagnostics,
        );
        assert_eq!(
            member_first
                .symbols
                .get("Speed")
                .map(|s| s.package.as_str()),
            Some("veh.common"),
            "the member declaration is the binding",
        );

        // Step 2: drop the member — `app`'s own `[imports]` alias wins.
        ws.set_packages(&mut db).to(vec![app]);
        let package_import = resolve_package(&db, ws, app, std);
        assert!(
            !package_import.symbols.contains_key("Speed"),
            "a remote import binds nothing until E1.6",
        );
        assert!(
            package_import
                .diagnostics
                .iter()
                .any(|d| d.message.contains("registry.example.com/pkg@v1")),
            "the package `[imports]` URL is the resolved source, got: {:?}",
            package_import.diagnostics,
        );

        // Step 3: drop the package alias — the workspace `[imports]` wins.
        app.set_imports(&mut db).to(BTreeMap::new());
        let workspace_import = resolve_package(&db, ws, app, std);
        assert!(
            workspace_import
                .diagnostics
                .iter()
                .any(|d| d.message.contains("registry.example.com/ws@v1")),
            "the workspace `[imports]` URL is the resolved source, got: {:?}",
            workspace_import.diagnostics,
        );
    }

    #[test]
    fn duplicate_declaration_is_typl_009_on_the_later_and_the_first_wins() {
        // Task 13 asserts the lowered IR carries exactly the first declaration.
        let mut db = RidlDatabase::default();
        let std = std_package(&mut db);
        let text = "package app\ntype Speed: km/h\ntype Speed: m/s\n";
        let app = package(&db, "app", text);
        let ws = Workspace::new(&db, vec![app], BTreeMap::new());

        let resolution = resolve_package(&db, ws, app, std);
        assert_eq!(codes(&resolution), vec!["TYPL-009"]);

        let first = text.find("Speed").expect("first Speed") as u32;
        let second =
            (first as usize + 1 + text[first as usize + 1..].find("Speed").unwrap()) as u32;
        assert_eq!(
            u32::from(resolution.symbols["Speed"].range.start()),
            first,
            "the first declaration wins the binding",
        );
        assert_eq!(
            u32::from(resolution.diagnostics[0].primary.range.start()),
            second,
            "the later declaration is the one flagged",
        );
    }

    #[test]
    fn wildcard_import_is_typl_003() {
        let mut db = RidlDatabase::default();
        let std = std_package(&mut db);
        let app = package(&db, "app", "package app\nimport veh.common.*\n");
        let ws = Workspace::new(&db, vec![app], BTreeMap::new());

        let resolution = resolve_package(&db, ws, app, std);
        assert_eq!(codes(&resolution), vec!["TYPL-003"]);
        assert_eq!(resolution.diagnostics[0].severity, Severity::Error);
    }

    #[test]
    fn relative_import_is_typl_003() {
        let mut db = RidlDatabase::default();
        let std = std_package(&mut db);
        let app = package(&db, "app", "package app\nimport .common.Speed\n");
        let ws = Workspace::new(&db, vec![app], BTreeMap::new());

        let resolution = resolve_package(&db, ws, app, std);
        assert_eq!(codes(&resolution), vec!["TYPL-003"]);
    }

    #[test]
    fn cross_package_cycle_is_typl_004() {
        let mut db = RidlDatabase::default();
        let std = std_package(&mut db);
        let a = package(&db, "a", "package a\nimport b.T\nstruct S { x: T }\n");
        let b = package(&db, "b", "package b\nimport a.S\nstruct T { y: S }\n");
        let ws = Workspace::new(&db, vec![a, b], BTreeMap::new());

        let resolution = resolve_package(&db, ws, a, std);
        assert_eq!(codes(&resolution), vec!["TYPL-004"]);
        assert!(
            resolution.diagnostics[0].message.contains('a')
                && resolution.diagnostics[0].message.contains('b'),
            "the message names both packages in the cycle",
        );
    }

    #[test]
    fn conflicting_imports_without_alias_is_typl_006() {
        let mut db = RidlDatabase::default();
        let std = std_package(&mut db);
        let a = package(
            &db,
            "a",
            "package a\nimport x.Speed\nimport y.Speed\nstruct S { p: Speed }\n",
        );
        let x = package(&db, "x", "package x\ntype Speed: km/h\n");
        let y = package(&db, "y", "package y\ntype Speed: m/s\n");
        let ws = Workspace::new(&db, vec![a, x, y], BTreeMap::new());

        let resolution = resolve_package(&db, ws, a, std);
        assert_eq!(codes(&resolution), vec!["TYPL-006"]);
        // The first import keeps the name.
        assert_eq!(
            resolution.symbols["Speed"].package.as_str(),
            "x",
            "first import wins the binding",
        );
    }

    #[test]
    fn unused_import_is_typl_007() {
        let mut db = RidlDatabase::default();
        let std = std_package(&mut db);
        let a = package(&db, "a", "package a\nimport x.Speed\n");
        let x = package(&db, "x", "package x\ntype Speed: km/h\n");
        let ws = Workspace::new(&db, vec![a, x], BTreeMap::new());

        let resolution = resolve_package(&db, ws, a, std);
        assert_eq!(codes(&resolution), vec!["TYPL-007"]);
        assert_eq!(resolution.diagnostics[0].severity, Severity::Warning);
    }

    #[test]
    fn alias_without_collision_is_typl_008() {
        let mut db = RidlDatabase::default();
        let std = std_package(&mut db);
        let a = package(
            &db,
            "a",
            "package a\nimport x.Speed as VehSpeed\nstruct S { p: VehSpeed }\n",
        );
        let x = package(&db, "x", "package x\ntype Speed: km/h\n");
        let ws = Workspace::new(&db, vec![a, x], BTreeMap::new());

        let resolution = resolve_package(&db, ws, a, std);
        assert_eq!(codes(&resolution), vec!["TYPL-008"]);
        assert_eq!(resolution.diagnostics[0].severity, Severity::Warning);
    }

    #[test]
    fn needed_alias_resolves_a_collision_without_diagnostics() {
        // `x.Speed` unaliased plus `y.Speed as MarineSpeed` — the alias is
        // required, so neither TYPL-006 nor TYPL-008 fires.
        let mut db = RidlDatabase::default();
        let std = std_package(&mut db);
        let a = package(
            &db,
            "a",
            "package a\nimport x.Speed\nimport y.Speed as MarineSpeed\nstruct S { p: Speed, q: MarineSpeed }\n",
        );
        let x = package(&db, "x", "package x\ntype Speed: km/h\n");
        let y = package(&db, "y", "package y\ntype Speed: m/s\n");
        let ws = Workspace::new(&db, vec![a, x, y], BTreeMap::new());

        let resolution = resolve_package(&db, ws, a, std);
        assert!(
            codes(&resolution).is_empty(),
            "the alias is needed, so no diagnostic, got: {:?}",
            resolution.diagnostics,
        );
    }

    #[test]
    fn qualified_reference_resolves_without_an_import() {
        // typl §3.2: a fully-qualified reference needs no import statement, so
        // the resolver raises nothing for it.
        let mut db = RidlDatabase::default();
        let std = std_package(&mut db);
        let a = package(&db, "a", "package a\nstruct S { p: veh.common.Speed }\n");
        let veh = package(&db, "veh.common", "package veh.common\ntype Speed: km/h\n");
        let ws = Workspace::new(&db, vec![a, veh], BTreeMap::new());

        let resolution = resolve_package(&db, ws, a, std);
        assert!(
            resolution.diagnostics.is_empty(),
            "a qualified reference needs no import, got: {:?}",
            resolution.diagnostics,
        );
    }

    #[test]
    fn ridl_std_names_are_implicitly_available() {
        let mut db = RidlDatabase::default();
        let std = std_package(&mut db);
        let app = package(&db, "app", "package app\n");
        let ws = Workspace::new(&db, vec![app], BTreeMap::new());

        let resolution = resolve_package(&db, ws, app, std);
        let timestamp = resolution
            .symbols
            .get("Timestamp")
            .expect("ridl.std exposes `Timestamp`");
        assert_eq!(timestamp.kind, SymbolKind::Type);
        assert_eq!(timestamp.package.as_str(), "ridl.std");
        assert!(resolution.diagnostics.is_empty());
    }

    /// A single-file workspace-member package whose one file is `.ridl`.
    fn ridl_package(db: &RidlDatabase, name: &str, text: &str) -> Package {
        Package::new(
            db,
            name.to_string(),
            vec![input(db, &format!("{name}.ridl"), text)],
            PackageOrigin::WorkspaceMember,
            BTreeMap::new(),
            None,
        )
    }

    #[test]
    fn interface_names_enter_the_symbol_table() {
        let mut db = RidlDatabase::default();
        let std = std_package(&mut db);
        let app = ridl_package(&db, "app", "package app\ninterface Cruise { }\n");
        let ws = Workspace::new(&db, vec![app], BTreeMap::new());

        let resolution = resolve_package(&db, ws, app, std);
        let symbol = resolution.symbols.get("Cruise").expect("`Cruise` binds");
        assert_eq!(symbol.kind, SymbolKind::Interface);
        assert!(resolution.diagnostics.is_empty());
    }

    #[test]
    fn interface_duplicating_a_type_is_typl_009_and_the_type_wins() {
        // A duplicate against any declaration kind is TYPL-009, first wins
        // (ADR-0007 decision 6, unchanged for E2).
        let mut db = RidlDatabase::default();
        let std = std_package(&mut db);
        let app = ridl_package(
            &db,
            "app",
            "package app\ntype Cruise: m\ninterface Cruise { }\n",
        );
        let ws = Workspace::new(&db, vec![app], BTreeMap::new());

        let resolution = resolve_package(&db, ws, app, std);
        assert_eq!(codes(&resolution), vec!["TYPL-009"]);
        assert_eq!(
            resolution.symbols["Cruise"].kind,
            SymbolKind::Type,
            "the first declaration wins the binding",
        );
    }

    #[test]
    fn type_duplicating_an_interface_is_typl_009_and_the_interface_wins() {
        // The mirror order: first-wins must follow source order across the
        // two declaration shapes, not process one shape before the other.
        let mut db = RidlDatabase::default();
        let std = std_package(&mut db);
        let app = ridl_package(
            &db,
            "app",
            "package app\ninterface Cruise { }\ntype Cruise: m\n",
        );
        let ws = Workspace::new(&db, vec![app], BTreeMap::new());

        let resolution = resolve_package(&db, ws, app, std);
        assert_eq!(codes(&resolution), vec!["TYPL-009"]);
        assert_eq!(
            resolution.symbols["Cruise"].kind,
            SymbolKind::Interface,
            "the first declaration wins the binding",
        );
    }

    #[test]
    fn a_local_declaration_shadows_a_ridl_std_name() {
        let mut db = RidlDatabase::default();
        let std = std_package(&mut db);
        // `Timestamp` is a `ridl.std` type; a local `type Timestamp` shadows it.
        let app = package(&db, "app", "package app\ntype Timestamp: s\n");
        let ws = Workspace::new(&db, vec![app], BTreeMap::new());

        let resolution = resolve_package(&db, ws, app, std);
        assert_eq!(
            resolution.symbols["Timestamp"].package.as_str(),
            "app",
            "the local declaration wins over the implicit `ridl.std` name",
        );
    }
}
