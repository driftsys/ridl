//! The trivial single-file resolver (docs/ROADMAP.md epic E0.5): collects
//! every declared `type`/`const` name, then checks that every const's type
//! reference names a declared `type`. No imports, no cross-file resolution —
//! that lands in a later epic.

use std::collections::HashMap;
use std::collections::hash_map::Entry;

use ridl_syntax::SourceFile;

/// The kind of a declared name.
#[derive(Debug, Clone, PartialEq)]
pub enum SymbolKind {
    Type,
    Const,
}

/// A resolution diagnostic, e.g. an unknown type name.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolveError {
    pub message: String,
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

    // Pass 1: declare every type and const name. The first declaration of a
    // name wins; a later one of either kind is a duplicate.
    for decl in file.type_decls() {
        if let Some(name) = decl.name() {
            declare(&mut symbols, &mut diagnostics, name, SymbolKind::Type);
        }
    }
    for decl in file.const_decls() {
        if let Some(name) = decl.name() {
            declare(&mut symbols, &mut diagnostics, name, SymbolKind::Const);
        }
    }

    // Pass 2: every const's type reference must name a declared `type`.
    for decl in file.const_decls() {
        let Some(type_name) = decl.type_name() else {
            // Malformed tree; the parser already reported the syntax error.
            continue;
        };
        match symbols.get(&type_name) {
            Some(SymbolKind::Type) => {}
            Some(SymbolKind::Const) => diagnostics.push(ResolveError {
                message: format!("`{type_name}` is not a type"),
            }),
            None => diagnostics.push(ResolveError {
                message: format!("unknown type name `{type_name}`"),
            }),
        }
    }

    Resolution {
        symbols,
        diagnostics,
    }
}

/// Inserts `name` into `symbols` unless it is already declared, in which case
/// the first declaration wins and the duplicate is reported instead.
fn declare(
    symbols: &mut HashMap<String, SymbolKind>,
    diagnostics: &mut Vec<ResolveError>,
    name: String,
    kind: SymbolKind,
) {
    match symbols.entry(name) {
        Entry::Occupied(entry) => diagnostics.push(ResolveError {
            message: format!("duplicate declaration `{}`", entry.key()),
        }),
        Entry::Vacant(entry) => {
            entry.insert(kind);
        }
    }
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
    }

    #[test]
    fn const_type_reference_naming_a_const_yields_one_diagnostic() {
        let resolution = resolve_source(
            "type Speed: km/h\nconst MAX_SPEED: Speed = 250.0\nconst A: MAX_SPEED = 1.0\n",
        );
        assert_eq!(resolution.diagnostics.len(), 1);
        assert!(resolution.diagnostics[0].message.contains("MAX_SPEED"));
    }
}
