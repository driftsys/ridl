//! Name-resolution navigation: the shared cursor-to-symbol lookup that hover,
//! goto-definition, and find-references all consume (docs/ROADMAP.md epic
//! E1.15b).
//!
//! [`symbol_at`] maps a byte offset in a file to the identifier token under it
//! and resolves that token to its declared [`Symbol`] — through imports,
//! qualified references, and the implicit `ridl.std` — reusing the resolver's
//! own package-local view ([`resolve_package`]). Goto-definition returns the
//! symbol's declaration site; [`find_references`] walks every file of every
//! loaded package and keeps the references that resolve to the same symbol,
//! so the result is name-resolution based, never a textual match. The task 24
//! rename feature reuses [`symbol_at`] for the same reason.

use ridl_core::db::{InputFile, parse_file};
use ridl_core::package::{Package, Workspace, package_of};
use ridl_sem::{Resolution, Symbol, resolve_package};
use ridl_syntax::ast::{AstNode, Import, QualifiedName, SourceFile};
use ridl_syntax::{SyntaxKind, SyntaxNode, SyntaxToken};
use rowan::{TextRange, TextSize, TokenAtOffset};

/// A cursor resolved to its declared symbol.
///
/// `symbol` is the declaration the cursor names, at its definition site
/// (`symbol.file` + `symbol.range`). `reference` is the span of the reference
/// the cursor sits in, within the file the cursor was in — what goto-definition
/// highlights and what rename rewrites in place.
#[derive(Debug, Clone)]
pub struct Located {
    pub symbol: Symbol,
    pub reference: TextRange,
}

/// Resolves the identifier at `offset` in `file` (a file of `pkg`) to its
/// declared [`Symbol`], or `None` when the cursor is not on a resolvable
/// reference.
///
/// `std` is the embedded `ridl.std` package, threaded in exactly as
/// [`resolve_package`] takes it. A cursor on a type reference, a named-constant
/// reference, or a declaration's own name resolves; a cursor on a field name,
/// enum value, or union arm does not (those are not top-level symbols — hover
/// reads their ordinal directly from the IR instead).
pub fn symbol_at(
    db: &dyn salsa::Database,
    ws: Workspace,
    std: Package,
    pkg: Package,
    file: InputFile,
    offset: TextSize,
) -> Option<Located> {
    let source = source_file(db, file);
    let token = identifier_at(source.syntax(), offset)?;
    let reference = reference_at(&token)?;
    let resolution = resolve_package(db, ws, pkg, std);
    let symbol = resolve_reference(db, ws, std, pkg, &resolution, &reference)?;
    Some(Located {
        symbol,
        reference: reference.range,
    })
}

/// Resolves a cursor sitting on the imported-symbol segment of an `import`
/// statement to the declared [`Symbol`] the import binds.
///
/// [`symbol_at`] returns `None` inside an import path — `reference_at` skips
/// tokens under a [`SyntaxKind::Import`] ancestor, because an import line is not
/// a reference the resolver walks. Rename still has to start from the import
/// line (the cursor is often there), so this entry point resolves the final
/// path segment (`import veh.common.Speed` → the `Speed` in `veh.common`) the
/// same way a qualified reference resolves, and reports the segment span rename
/// rewrites in place. A cursor on a package-path segment (`common`) is not an
/// imported symbol and yields `None` — package rename is out of scope.
pub fn import_at(
    db: &dyn salsa::Database,
    ws: Workspace,
    std: Package,
    pkg: Package,
    file: InputFile,
    offset: TextSize,
) -> Option<Located> {
    let source = source_file(db, file);
    let token = identifier_at(source.syntax(), offset)?;
    let import = token.parent_ancestors().find_map(Import::cast)?;
    let qualified = import.qualified_name()?;
    // The imported symbol is the last non-`.` token of the path; the alias
    // (`as VS`) is a separate `Name` child, so it is never this token.
    let last = last_segment_token(&qualified)?;
    if last.text_range() != token.text_range() {
        return None;
    }
    let reference = Reference {
        segments: qualified_segments(&qualified),
        range: last.text_range(),
    };
    let resolution = resolve_package(db, ws, pkg, std);
    let symbol = resolve_reference(db, ws, std, pkg, &resolution, &reference)?;
    Some(Located {
        symbol,
        reference: last.text_range(),
    })
}

/// The final path segment token of a qualified name — its last non-trivia,
/// non-`.` token. This is the imported symbol name in an `import` path and the
/// referenced name in a qualified reference (the only part rename rewrites).
pub(crate) fn last_segment_token(qualified: &QualifiedName) -> Option<SyntaxToken> {
    qualified
        .syntax()
        .children_with_tokens()
        .filter_map(|element| element.into_token())
        .filter(|token| !token.kind().is_trivia() && token.kind() != SyntaxKind::Dot)
        .last()
}

/// Every reference across `packages` that resolves to the same symbol as
/// `target` (same package and name), as `(file, span)` pairs in package and
/// source order.
///
/// The declaration site is not included — a references request that wants it
/// (LSP `context.includeDeclaration`) adds `target`'s own location itself. Each
/// reference is resolved through [`resolve_package`], so a name that shadows the
/// target in another package is correctly excluded.
pub fn find_references(
    db: &dyn salsa::Database,
    ws: Workspace,
    std: Package,
    packages: &[Package],
    target: &Symbol,
) -> Vec<(InputFile, TextRange)> {
    let mut out = Vec::new();
    for &pkg in packages {
        let resolution = resolve_package(db, ws, pkg, std);
        for &file in pkg.files(db) {
            let source = source_file(db, file);
            for reference in references_in(&source) {
                if let Some(symbol) = resolve_reference(db, ws, std, pkg, &resolution, &reference)
                    && symbol.package == target.package
                    && symbol.name == target.name
                {
                    out.push((file, reference.range));
                }
            }
        }
    }
    out
}

/// A name reference read off the tree: its dot-separated segments (a single
/// segment for a bare name or a named-constant reference; two or more for a
/// qualified `pkg.Name`) and the source span the whole reference occupies.
struct Reference {
    segments: Vec<String>,
    range: TextRange,
}

/// Resolves a [`Reference`] against the referencing package `pkg`'s
/// `resolution`.
///
/// A single-segment reference is looked up in the package-local view (locals,
/// imports, then `ridl.std`); a qualified reference resolves its package path
/// through ADR-0002 §5 and reads the named declaration from that package's own
/// resolution — never an alias or a re-export. The requesting package `pkg` is
/// preferred when its name matches the path, so a self-qualified reference in a
/// standalone overlay (which `package_of` cannot find) still resolves —
/// mirroring the checker's own `package_handle`.
fn resolve_reference(
    db: &dyn salsa::Database,
    ws: Workspace,
    std: Package,
    pkg: Package,
    resolution: &Resolution,
    reference: &Reference,
) -> Option<Symbol> {
    let (name, package_path) = reference.segments.split_last()?;
    if package_path.is_empty() {
        return resolution.symbols.get(name).cloned();
    }
    let target_path = package_path.join(".");
    let target = if target_path == *pkg.name(db) {
        pkg
    } else if target_path == *std.name(db) {
        std
    } else {
        package_of(db, ws, target_path.clone())?
    };
    let target_resolution = resolve_package(db, ws, target, std);
    target_resolution
        .symbols
        .get(name)
        .filter(|symbol| symbol.package == target_path)
        .cloned()
}

/// The reference the identifier `token` participates in: a type reference
/// (`QualifiedName`), a named-constant reference (an `Ident` inside a
/// `Literal`), or a declaration's own name (a `Name` under a definition). A
/// token inside a `package` or `import` path, or a member name (field, enum
/// value, union arm), is not a resolvable reference and yields `None`.
fn reference_at(token: &SyntaxToken) -> Option<Reference> {
    for node in token.parent_ancestors() {
        match node.kind() {
            SyntaxKind::QualifiedName => {
                if node
                    .ancestors()
                    .any(|a| matches!(a.kind(), SyntaxKind::PackageDecl | SyntaxKind::Import))
                {
                    return None;
                }
                let qualified = QualifiedName::cast(node.clone()).expect("kind checked");
                return Some(Reference {
                    segments: qualified_segments(&qualified),
                    range: node.text_range(),
                });
            }
            SyntaxKind::Literal => {
                // A named-constant reference used as a bound or a `match`
                // pattern is a bare `Ident` inside the literal.
                return Some(Reference {
                    segments: vec![token.text().to_string()],
                    range: token.text_range(),
                });
            }
            SyntaxKind::Name => {
                // A declaration's own name resolves to itself; a member name
                // (whose parent is not a top-level definition) does not.
                let parent = node.parent()?;
                if !is_definition(parent.kind()) {
                    return None;
                }
                return Some(Reference {
                    segments: vec![token.text().to_string()],
                    range: node.text_range(),
                });
            }
            _ => {}
        }
    }
    None
}

/// Every type reference and named-constant reference in the file, in source
/// order — the references [`find_references`] resolves. Declaration names and
/// `package`/`import` paths are excluded.
fn references_in(source: &SourceFile) -> Vec<Reference> {
    let mut out = Vec::new();
    for node in source.syntax().descendants() {
        match node.kind() {
            SyntaxKind::QualifiedName => {
                if node
                    .ancestors()
                    .any(|a| matches!(a.kind(), SyntaxKind::PackageDecl | SyntaxKind::Import))
                {
                    continue;
                }
                let qualified = QualifiedName::cast(node.clone()).expect("kind checked");
                out.push(Reference {
                    segments: qualified_segments(&qualified),
                    range: node.text_range(),
                });
            }
            SyntaxKind::Literal => {
                for token in node.children_with_tokens().filter_map(|e| e.into_token()) {
                    if token.kind() == SyntaxKind::Ident {
                        out.push(Reference {
                            segments: vec![token.text().to_string()],
                            range: token.text_range(),
                        });
                    }
                }
            }
            _ => {}
        }
    }
    out
}

/// The span of the final path segment inside a reference `range` in `file` —
/// the sub-range rename rewrites in place.
///
/// [`find_references`] returns the span of the whole reference: a single name
/// for a bare reference, but the whole `pkg.Name` for a qualified one. Rename
/// must touch only the final `Name` segment, so this narrows a qualified
/// reference to its last token and leaves a bare reference unchanged.
pub fn final_segment_range(
    db: &dyn salsa::Database,
    file: InputFile,
    range: TextRange,
) -> TextRange {
    let source = source_file(db, file);
    match source.syntax().covering_element(range) {
        rowan::NodeOrToken::Token(token) => token.text_range(),
        rowan::NodeOrToken::Node(node) => node
            .descendants_with_tokens()
            .filter_map(|element| element.into_token())
            .filter(|token| !token.kind().is_trivia() && token.kind() != SyntaxKind::Dot)
            .last()
            .map(|token| token.text_range())
            .unwrap_or(range),
    }
}

/// The identifier token at `offset`, preferring an `Ident` when the offset
/// lands between two tokens (the boundary of a name and its surrounding
/// punctuation or trivia).
pub(crate) fn identifier_at(root: &SyntaxNode, offset: TextSize) -> Option<SyntaxToken> {
    match root.token_at_offset(offset) {
        TokenAtOffset::None => None,
        TokenAtOffset::Single(token) => Some(token),
        TokenAtOffset::Between(left, right) => {
            if right.kind() == SyntaxKind::Ident {
                Some(right)
            } else if left.kind() == SyntaxKind::Ident {
                Some(left)
            } else {
                Some(right)
            }
        }
    }
}

/// The dot-separated segments of a qualified name, read by token so keyword
/// path segments (`veh.integer`) survive verbatim — the same rule the resolver
/// applies (typl §3.2).
fn qualified_segments(qualified: &QualifiedName) -> Vec<String> {
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

/// Whether a syntax kind is one of the six top-level definition kinds.
fn is_definition(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::TypeDef
            | SyntaxKind::ConstDef
            | SyntaxKind::StructDef
            | SyntaxKind::EnumDef
            | SyntaxKind::EnumSetDef
            | SyntaxKind::UnionDef
    )
}

/// The parsed [`SourceFile`] of one input, through the memoized parse query.
pub(crate) fn source_file(db: &dyn salsa::Database, file: InputFile) -> SourceFile {
    let parse = parse_file(db, file);
    SourceFile::cast(parse.syntax()).expect("the parser roots every tree in a SourceFile")
}
