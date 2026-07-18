//! Thin typed accessors over the rowan tree (docs/ROADMAP.md epic E0.3).
//!
//! These are hand-written for the E0 subset. From E1 they are generated from an
//! `ungrammar` description (ADR-0004 §2). Every accessor walks child kinds and
//! returns `Option`/empty iterators on a malformed tree — it never panics.

use crate::syntax_kind::{SyntaxKind, SyntaxNode, SyntaxToken};

/// A parsed `type`/`const` range: inclusive bounds and an optional step.
#[derive(Debug, Clone, PartialEq)]
pub struct RangeSpec {
    pub min: f64,
    pub max: f64,
    pub step: Option<f64>,
}

/// The whole parsed file.
pub struct SourceFile(SyntaxNode);

impl SourceFile {
    pub fn cast(node: SyntaxNode) -> Option<Self> {
        (node.kind() == SyntaxKind::SourceFile).then_some(Self(node))
    }

    pub fn type_decls(&self) -> impl Iterator<Item = TypeDecl> + '_ {
        self.0.children().filter_map(TypeDecl::cast)
    }

    pub fn const_decls(&self) -> impl Iterator<Item = ConstDecl> + '_ {
        self.0.children().filter_map(ConstDecl::cast)
    }
}

/// A `type Name: unit_expr range?` declaration.
pub struct TypeDecl(SyntaxNode);

impl TypeDecl {
    pub fn cast(node: SyntaxNode) -> Option<Self> {
        (node.kind() == SyntaxKind::TypeDecl).then_some(Self(node))
    }

    /// The declared name, e.g. `"Speed"`.
    pub fn name(&self) -> Option<String> {
        let name = child_node(&self.0, SyntaxKind::Name)?;
        ident_text(&name)
    }

    /// The unit expression text, e.g. `"km/h"`.
    pub fn unit(&self) -> Option<String> {
        let unit = child_node(&self.0, SyntaxKind::UnitExpr)?;
        Some(significant_text(&unit))
    }

    /// The range, e.g. `{ min: 0.0, max: 250.0, step: Some(0.5) }`.
    pub fn range(&self) -> Option<RangeSpec> {
        let range = child_node(&self.0, SyntaxKind::Range)?;
        let mut literals = range.children().filter(|n| n.kind() == SyntaxKind::Literal);
        let min = literal_value(&literals.next()?)?;
        let max = literal_value(&literals.next()?)?;
        let step = literals.next().and_then(|node| literal_value(&node));
        Some(RangeSpec { min, max, step })
    }
}

/// A `const Name: Type = number` declaration.
pub struct ConstDecl(SyntaxNode);

impl ConstDecl {
    pub fn cast(node: SyntaxNode) -> Option<Self> {
        (node.kind() == SyntaxKind::ConstDecl).then_some(Self(node))
    }

    /// The declared name, e.g. `"MAX_SPEED"`.
    pub fn name(&self) -> Option<String> {
        let name = child_node(&self.0, SyntaxKind::Name)?;
        ident_text(&name)
    }

    /// The referenced type name, e.g. `"Speed"`. It is the bare `Ident` token
    /// directly under the `ConstDecl` node; the const's own name lives inside a
    /// `Name` node, so it is never mistaken for the type reference.
    pub fn type_name(&self) -> Option<String> {
        child_tokens(&self.0)
            .find(|token| token.kind() == SyntaxKind::Ident)
            .map(|token| token.text().to_string())
    }

    /// The literal value, e.g. `250.0`.
    pub fn value(&self) -> Option<f64> {
        let literal = child_node(&self.0, SyntaxKind::Literal)?;
        literal_value(&literal)
    }
}

// --- shared helpers ------------------------------------------------------

/// The first child node of the given kind.
fn child_node(node: &SyntaxNode, kind: SyntaxKind) -> Option<SyntaxNode> {
    node.children().find(|n| n.kind() == kind)
}

/// The direct token children of a node (skipping child nodes).
fn child_tokens(node: &SyntaxNode) -> impl Iterator<Item = SyntaxToken> + '_ {
    node.children_with_tokens()
        .filter_map(|element| element.into_token())
}

/// The text of the first `Ident` token directly under `node`.
fn ident_text(node: &SyntaxNode) -> Option<String> {
    child_tokens(node)
        .find(|token| token.kind() == SyntaxKind::Ident)
        .map(|token| token.text().to_string())
}

/// The concatenation of every non-trivia token directly under `node` — the
/// node's text with leading/trailing whitespace and comments removed.
fn significant_text(node: &SyntaxNode) -> String {
    child_tokens(node)
        .filter(|token| !token.kind().is_trivia())
        .map(|token| token.text().to_string())
        .collect()
}

/// The `f64` value of the number token inside a `Literal` node, or `None` if it
/// is absent or unparseable.
fn literal_value(node: &SyntaxNode) -> Option<f64> {
    child_tokens(node)
        .find(|token| {
            matches!(
                token.kind(),
                SyntaxKind::IntNumber | SyntaxKind::FloatNumber
            )
        })
        .and_then(|token| token.text().parse::<f64>().ok())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse;

    const FIXTURE: &str = include_str!("../fixtures/walking_skeleton.typl");

    fn source_file() -> SourceFile {
        SourceFile::cast(parse(FIXTURE).syntax()).expect("root is a SourceFile")
    }

    #[test]
    fn type_decl_accessors_match_the_fixture() {
        let decl = source_file()
            .type_decls()
            .next()
            .expect("the fixture has one type declaration");
        assert_eq!(decl.name().as_deref(), Some("Speed"));
        assert_eq!(decl.unit().as_deref(), Some("km/h"));
        assert_eq!(
            decl.range(),
            Some(RangeSpec {
                min: 0.0,
                max: 250.0,
                step: Some(0.5),
            }),
        );
    }

    #[test]
    fn const_decl_accessors_match_the_fixture() {
        let decl = source_file()
            .const_decls()
            .next()
            .expect("the fixture has one const declaration");
        assert_eq!(decl.name().as_deref(), Some("MAX_SPEED"));
        assert_eq!(decl.type_name().as_deref(), Some("Speed"));
        assert_eq!(decl.value(), Some(250.0));
    }
}
