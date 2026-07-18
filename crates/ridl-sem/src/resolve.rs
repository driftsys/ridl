//! The trivial single-file resolver (docs/ROADMAP.md epic E0.5): collects
//! every declared `type`/`const` name, then checks that every const's type
//! reference names a declared `type`. No imports, no cross-file resolution —
//! that lands in a later epic. Composite definitions (`struct`, `enum`,
//! `enumset`, `union`) are not resolved yet either: the full resolver is E1
//! scope (docs/ROADMAP.md, task E1 resolver).
//!
//! Reads the `typl.ungram`-generated typed AST (`ridl_syntax::ast`) — ported
//! from the E0 accessor layer in E1.2b.

use std::collections::HashMap;
use std::collections::hash_map::Entry;

use ridl_syntax::ast::{AstNode, ConstDef, Definition, HasName, SourceFile};
use ridl_syntax::{SyntaxKind, SyntaxNode};
use rowan::TextRange;

/// The kind of a declared name.
#[derive(Debug, Clone, PartialEq)]
pub enum SymbolKind {
    Type,
    Const,
}

/// A resolution diagnostic. It carries the stable diagnostic code it maps to in
/// the coded model (`ridl_core::diag`, E1.10) and the source range of the
/// offending name — the empty code string means no typl §16 code exists for the
/// rule yet (the unknown-type-name check, rehomed by a later task).
#[derive(Debug, Clone, PartialEq)]
pub struct ResolveError {
    pub message: String,
    pub code: &'static str,
    pub range: TextRange,
}

/// The result of [`resolve`]: every declared name plus any diagnostics.
pub struct Resolution {
    pub symbols: HashMap<String, SymbolKind>,
    pub diagnostics: Vec<ResolveError>,
}

/// Resolves names within a single file, no imports: collect declared names,
/// then verify every const's type reference names a declared `type`.
pub fn resolve(file: &SourceFile) -> Resolution {
    let mut symbols = HashMap::new();
    let mut diagnostics = Vec::new();

    // Pass 1: declaration passes run types then consts; on a name collision
    // the earlier pass wins and a duplicate diagnostic is reported —
    // source-position order lands with the full resolver in E1.
    for definition in file.definitions() {
        if let Definition::Type(decl) = definition
            && let Some(name) = declared_name(&decl)
        {
            declare(
                &mut symbols,
                &mut diagnostics,
                name,
                SymbolKind::Type,
                name_range(&decl),
            );
        }
    }
    for definition in file.definitions() {
        if let Definition::Const(decl) = definition
            && let Some(name) = declared_name(&decl)
        {
            declare(
                &mut symbols,
                &mut diagnostics,
                name,
                SymbolKind::Const,
                name_range(&decl),
            );
        }
    }

    // Pass 2: every const's type reference must name a declared `type`.
    for definition in file.definitions() {
        let Definition::Const(decl) = definition else {
            continue;
        };
        let Some(type_name) = const_type_name(&decl) else {
            // Untyped regex constant, or a malformed tree the parser already
            // reported a syntax error for.
            continue;
        };
        let range = decl
            .type_ref()
            .map(|type_ref| type_ref.syntax().text_range())
            .unwrap_or_default();
        match symbols.get(&type_name) {
            Some(SymbolKind::Type) => {}
            // No typl §16 code covers "used a constant as a type" or an unknown
            // type name yet; both surface through the resolver import rules and
            // the checker's lowering skips, and a later task rehomes them
            // (ADR-0007 decision 10). They render as plain, coded-model messages.
            Some(SymbolKind::Const) => diagnostics.push(ResolveError {
                message: format!("expected a type, but `{type_name}` names a constant"),
                code: "",
                range,
            }),
            None => diagnostics.push(ResolveError {
                message: format!("unknown type name `{type_name}`"),
                code: "",
                range,
            }),
        }
    }

    Resolution {
        symbols,
        diagnostics,
    }
}

/// Inserts `name` into `symbols` unless it is already declared, in which case
/// the earlier call (from the earlier declaration pass) wins and this later
/// declaration is reported as a duplicate (TYPL-009, typl reference §16.1) at
/// its own `range`.
fn declare(
    symbols: &mut HashMap<String, SymbolKind>,
    diagnostics: &mut Vec<ResolveError>,
    name: String,
    kind: SymbolKind,
    range: TextRange,
) {
    match symbols.entry(name) {
        Entry::Occupied(entry) => diagnostics.push(ResolveError {
            message: format!("duplicate declaration of `{}`", entry.key()),
            code: "TYPL-009",
            range,
        }),
        Entry::Vacant(entry) => {
            entry.insert(kind);
        }
    }
}

// --- shared AST helpers (also used by the checker) ------------------------

/// The declared name of a definition, or `None` on a malformed tree.
pub(crate) fn declared_name(definition: &impl HasName) -> Option<String> {
    Some(definition.name()?.ident_token()?.text().to_string())
}

/// The source range of a definition's declared name, or an empty range on a
/// malformed tree (unreachable for a definition whose [`declared_name`] is
/// present).
fn name_range(definition: &impl HasName) -> TextRange {
    definition
        .name()
        .map(|name| name.syntax().text_range())
        .unwrap_or_default()
}

/// The type reference of a const (`Speed`, `pkg.Speed`, or a primitive
/// keyword), or `None` when the const declares no type or the reference is
/// malformed.
pub(crate) fn const_type_name(decl: &ConstDef) -> Option<String> {
    let text = significant_text(decl.type_ref()?.syntax());
    (!text.is_empty()).then_some(text)
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

/// The `f64` value of a numeric `Literal` node (including a leading `-`), or
/// `None` when the literal is not numeric (a string, regex, or constant
/// reference).
pub(crate) fn literal_f64(literal: &ridl_syntax::ast::Literal) -> Option<f64> {
    let has_number = literal
        .syntax()
        .descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .any(|token| {
            matches!(
                token.kind(),
                SyntaxKind::IntNumber | SyntaxKind::FloatNumber
            )
        });
    if !has_number {
        return None;
    }
    significant_text(literal.syntax()).parse::<f64>().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = include_str!("../../ridl-syntax/fixtures/walking_skeleton.typl");

    fn resolve_source(input: &str) -> Resolution {
        let parse = ridl_syntax::parse(input);
        let file = SourceFile::cast(parse.syntax()).expect("root is a SourceFile");
        resolve(&file)
    }

    #[test]
    fn fixture_resolves_with_no_diagnostics() {
        let resolution = resolve_source(FIXTURE);
        assert_eq!(resolution.symbols.get("Speed"), Some(&SymbolKind::Type));
        assert_eq!(
            resolution.symbols.get("MAX_SPEED"),
            Some(&SymbolKind::Const)
        );
        assert!(
            resolution.diagnostics.is_empty(),
            "expected no diagnostics, got: {:?}",
            resolution.diagnostics,
        );
    }

    #[test]
    fn unknown_type_name_yields_one_diagnostic() {
        let resolution = resolve_source("const X: Missing = 1.0\n");
        assert_eq!(resolution.diagnostics.len(), 1);
        assert!(resolution.diagnostics[0].message.contains("Missing"));
    }

    #[test]
    fn duplicate_declaration_yields_one_diagnostic() {
        let resolution = resolve_source("type Speed: km/h\ntype Speed: km/h\n");
        assert_eq!(resolution.diagnostics.len(), 1);
        assert!(resolution.diagnostics[0].message.contains("Speed"));
        assert_eq!(resolution.symbols.get("Speed"), Some(&SymbolKind::Type));
    }

    #[test]
    fn cross_kind_collision_reports_one_duplicate_and_the_type_wins() {
        // `Speed` appears as a const before it appears as a type in the
        // source; the declare pass still resolves it to the type, because
        // declaration passes run types then consts regardless of source
        // position (see the module doc comment on the declare pass).
        let resolution = resolve_source("const Speed: Speed = 1.0\ntype Speed: km/h\n");
        assert_eq!(resolution.diagnostics.len(), 1);
        assert!(resolution.diagnostics[0].message.contains("Speed"));
        assert_eq!(resolution.symbols.get("Speed"), Some(&SymbolKind::Type));
    }

    #[test]
    fn const_with_unparseable_type_name_is_skipped_silently() {
        // The parser already reports a syntax error for the missing type
        // reference; the resolver has nothing to add for it and must not
        // panic on the malformed tree.
        let resolution = resolve_source("const X: = 1.0\n");
        assert!(resolution.diagnostics.is_empty());
    }

    #[test]
    fn const_type_reference_naming_a_const_yields_one_diagnostic() {
        let resolution = resolve_source(
            "type Speed: km/h\nconst MAX_SPEED: Speed = 250.0\nconst A: MAX_SPEED = 1.0\n",
        );
        assert_eq!(resolution.diagnostics.len(), 1);
        assert!(resolution.diagnostics[0].message.contains("MAX_SPEED"));
    }

    #[test]
    fn empty_file_resolves_to_zero_diagnostics() {
        // The #102 empty-file resolver test: an empty input parses to an empty
        // `SourceFile`; the resolver produces no symbols and no diagnostics, and
        // does not panic on the near-empty tree.
        let resolution = resolve_source("");
        assert!(resolution.symbols.is_empty());
        assert!(resolution.diagnostics.is_empty());
    }

    #[test]
    fn duplicate_declaration_carries_the_typl_009_code() {
        let resolution = resolve_source("type Speed: km/h\ntype Speed: km/h\n");
        assert_eq!(resolution.diagnostics.len(), 1);
        assert_eq!(resolution.diagnostics[0].code, "TYPL-009");
        assert!(
            resolution.diagnostics[0]
                .message
                .contains("duplicate declaration of `Speed`"),
        );
    }

    #[test]
    fn untyped_regex_constant_resolves_without_diagnostics() {
        let resolution = resolve_source("package p\nconst VIN_PATTERN = /^[A-Z0-9]{17}$/\n");
        assert_eq!(
            resolution.symbols.get("VIN_PATTERN"),
            Some(&SymbolKind::Const)
        );
        assert!(resolution.diagnostics.is_empty());
    }
}
