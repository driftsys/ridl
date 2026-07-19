//! The typed AST over the rowan tree (docs/ROADMAP.md epic E1.2a, ADR-0007
//! decision 1).
//!
//! The node structs and their mechanical accessors live in the `generated`
//! module,
//! written by `cargo xtask codegen` from `family.ungram` and committed; an
//! xtask drift test keeps the two in sync. This module adds the hand-written
//! layer the generator does not cover:
//!
//! - the [`AstNode`] trait and the [`AstChildren`] iterator;
//! - the enums over pure node alternations ([`Definition`], [`Backing`],
//!   [`StructMember`], [`FieldType`]);
//! - the [`HasName`] / [`HasModifiers`] / [`HasDocComments`] trait trio
//!   shared by the six definition kinds;
//! - the position-sensitive accessors whose roles are told apart by anchor
//!   tokens ([`Constraint::min`] and friends, [`ConstDef::regex`]).
//!
//! Every accessor walks child kinds and returns `Option`/empty iterators on
//! a malformed tree — it never panics.

mod generated;

pub use generated::*;

use std::marker::PhantomData;

use crate::syntax_kind::{RidlLanguage, SyntaxKind, SyntaxNode, SyntaxToken};

/// A typed view over a [`SyntaxNode`] of a known kind.
pub trait AstNode: Sized {
    /// Wraps `syntax` if its kind matches this type; `None` otherwise.
    fn cast(syntax: SyntaxNode) -> Option<Self>;

    /// The underlying untyped node.
    fn syntax(&self) -> &SyntaxNode;
}

/// An iterator over the children of a node that cast to `N`, in source
/// order.
#[derive(Debug, Clone)]
pub struct AstChildren<N> {
    inner: rowan::SyntaxNodeChildren<RidlLanguage>,
    _marker: PhantomData<N>,
}

impl<N> AstChildren<N> {
    fn new(parent: &SyntaxNode) -> Self {
        Self {
            inner: parent.children(),
            _marker: PhantomData,
        }
    }
}

impl<N: AstNode> Iterator for AstChildren<N> {
    type Item = N;

    fn next(&mut self) -> Option<N> {
        self.inner.by_ref().find_map(N::cast)
    }
}

/// Child-lookup helpers shared by the generated accessors and the
/// hand-written ones below.
mod support {
    use super::{AstChildren, AstNode};
    use crate::syntax_kind::{SyntaxKind, SyntaxNode, SyntaxToken};

    /// The first child of `parent` that casts to `N`.
    pub(super) fn child<N: AstNode>(parent: &SyntaxNode) -> Option<N> {
        parent.children().find_map(N::cast)
    }

    /// The `n`-th child of `parent` that casts to `N` (0-based).
    pub(super) fn nth_child<N: AstNode>(parent: &SyntaxNode, n: usize) -> Option<N> {
        parent.children().filter_map(N::cast).nth(n)
    }

    /// All children of `parent` that cast to `N`, in source order.
    pub(super) fn children<N: AstNode>(parent: &SyntaxNode) -> AstChildren<N> {
        AstChildren::new(parent)
    }

    /// The first direct token child of `parent` with the given kind.
    pub(super) fn token(parent: &SyntaxNode, kind: SyntaxKind) -> Option<SyntaxToken> {
        parent
            .children_with_tokens()
            .filter_map(|element| element.into_token())
            .find(|token| token.kind() == kind)
    }
}

// --- enums over node alternations (`family.ungram` alternation rules) ------

/// One top-level definition — the `Definition` alternation.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Definition {
    Type(TypeDef),
    Const(ConstDef),
    Struct(StructDef),
    Enum(EnumDef),
    EnumSet(EnumSetDef),
    Union(UnionDef),
}

impl AstNode for Definition {
    fn cast(syntax: SyntaxNode) -> Option<Self> {
        match syntax.kind() {
            SyntaxKind::TypeDef => TypeDef::cast(syntax).map(Self::Type),
            SyntaxKind::ConstDef => ConstDef::cast(syntax).map(Self::Const),
            SyntaxKind::StructDef => StructDef::cast(syntax).map(Self::Struct),
            SyntaxKind::EnumDef => EnumDef::cast(syntax).map(Self::Enum),
            SyntaxKind::EnumSetDef => EnumSetDef::cast(syntax).map(Self::EnumSet),
            SyntaxKind::UnionDef => UnionDef::cast(syntax).map(Self::Union),
            _ => None,
        }
    }

    fn syntax(&self) -> &SyntaxNode {
        match self {
            Self::Type(it) => it.syntax(),
            Self::Const(it) => it.syntax(),
            Self::Struct(it) => it.syntax(),
            Self::Enum(it) => it.syntax(),
            Self::EnumSet(it) => it.syntax(),
            Self::Union(it) => it.syntax(),
        }
    }
}

/// The backing of a `type` definition — the `Backing` alternation
/// (typl reference §4–§5).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Backing {
    Primitive(PrimitiveType),
    Unit(UnitExpr),
}

impl AstNode for Backing {
    fn cast(syntax: SyntaxNode) -> Option<Self> {
        match syntax.kind() {
            SyntaxKind::PrimitiveType => PrimitiveType::cast(syntax).map(Self::Primitive),
            SyntaxKind::UnitExpr => UnitExpr::cast(syntax).map(Self::Unit),
            _ => None,
        }
    }

    fn syntax(&self) -> &SyntaxNode {
        match self {
            Self::Primitive(it) => it.syntax(),
            Self::Unit(it) => it.syntax(),
        }
    }
}

/// One member of a `struct` body — the `StructMember` alternation
/// (typl reference §7).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum StructMember {
    Field(FieldDef),
    Reserved(ReservedEntry),
}

impl AstNode for StructMember {
    fn cast(syntax: SyntaxNode) -> Option<Self> {
        match syntax.kind() {
            SyntaxKind::FieldDef => FieldDef::cast(syntax).map(Self::Field),
            SyntaxKind::ReservedEntry => ReservedEntry::cast(syntax).map(Self::Reserved),
            _ => None,
        }
    }

    fn syntax(&self) -> &SyntaxNode {
        match self {
            Self::Field(it) => it.syntax(),
            Self::Reserved(it) => it.syntax(),
        }
    }
}

/// The type of a field, tuple field, or collection element — the
/// `FieldType` alternation (typl reference §7, §11–§12).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum FieldType {
    Path(PathType),
    Primitive(PrimitiveType),
    Tuple(TupleType),
    Array(ArrayType),
    Map(MapType),
    Optional(OptionalType),
}

impl AstNode for FieldType {
    fn cast(syntax: SyntaxNode) -> Option<Self> {
        match syntax.kind() {
            SyntaxKind::PathType => PathType::cast(syntax).map(Self::Path),
            SyntaxKind::PrimitiveType => PrimitiveType::cast(syntax).map(Self::Primitive),
            SyntaxKind::TupleType => TupleType::cast(syntax).map(Self::Tuple),
            SyntaxKind::ArrayType => ArrayType::cast(syntax).map(Self::Array),
            SyntaxKind::MapType => MapType::cast(syntax).map(Self::Map),
            SyntaxKind::OptionalType => OptionalType::cast(syntax).map(Self::Optional),
            _ => None,
        }
    }

    fn syntax(&self) -> &SyntaxNode {
        match self {
            Self::Path(it) => it.syntax(),
            Self::Primitive(it) => it.syntax(),
            Self::Tuple(it) => it.syntax(),
            Self::Array(it) => it.syntax(),
            Self::Map(it) => it.syntax(),
            Self::Optional(it) => it.syntax(),
        }
    }
}

/// One member of an `interface` body — the `InterfaceMember` alternation
/// (ridl reference §14.0–§14.1, epic E2.1a).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum InterfaceMember {
    Signal(SignalDef),
    Event(EventDef),
    Command(CommandDef),
    Query(QueryDef),
    Final(FinalDef),
    Reserved(ReservedEntry),
}

impl AstNode for InterfaceMember {
    fn cast(syntax: SyntaxNode) -> Option<Self> {
        match syntax.kind() {
            SyntaxKind::SignalDef => SignalDef::cast(syntax).map(Self::Signal),
            SyntaxKind::EventDef => EventDef::cast(syntax).map(Self::Event),
            SyntaxKind::CommandDef => CommandDef::cast(syntax).map(Self::Command),
            SyntaxKind::QueryDef => QueryDef::cast(syntax).map(Self::Query),
            SyntaxKind::FinalDef => FinalDef::cast(syntax).map(Self::Final),
            SyntaxKind::ReservedEntry => ReservedEntry::cast(syntax).map(Self::Reserved),
            _ => None,
        }
    }

    fn syntax(&self) -> &SyntaxNode {
        match self {
            Self::Signal(it) => it.syntax(),
            Self::Event(it) => it.syntax(),
            Self::Command(it) => it.syntax(),
            Self::Query(it) => it.syntax(),
            Self::Final(it) => it.syntax(),
            Self::Reserved(it) => it.syntax(),
        }
    }
}

/// The type of a command or query parameter — the `ParamType` alternation
/// (ridl reference §6.1, §7.1, §12): a field type or a stream `<T>`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ParamType {
    Field(FieldType),
    Stream(StreamType),
}

impl AstNode for ParamType {
    fn cast(syntax: SyntaxNode) -> Option<Self> {
        if syntax.kind() == SyntaxKind::StreamType {
            StreamType::cast(syntax).map(Self::Stream)
        } else {
            FieldType::cast(syntax).map(Self::Field)
        }
    }

    fn syntax(&self) -> &SyntaxNode {
        match self {
            Self::Field(it) => it.syntax(),
            Self::Stream(it) => it.syntax(),
        }
    }
}

/// One expression of the guaranteed subset — the `Expr` alternation
/// (expr-core specification §3.1, ridl reference §13, epic E2.4).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Expr {
    Binary(BinaryExpr),
    Prefix(PrefixExpr),
    Member(MemberExpr),
    Path(PathExpr),
    Paren(ParenExpr),
    Literal(LiteralExpr),
}

impl AstNode for Expr {
    fn cast(syntax: SyntaxNode) -> Option<Self> {
        match syntax.kind() {
            SyntaxKind::BinaryExpr => BinaryExpr::cast(syntax).map(Self::Binary),
            SyntaxKind::PrefixExpr => PrefixExpr::cast(syntax).map(Self::Prefix),
            SyntaxKind::MemberExpr => MemberExpr::cast(syntax).map(Self::Member),
            SyntaxKind::PathExpr => PathExpr::cast(syntax).map(Self::Path),
            SyntaxKind::ParenExpr => ParenExpr::cast(syntax).map(Self::Paren),
            SyntaxKind::LiteralExpr => LiteralExpr::cast(syntax).map(Self::Literal),
            _ => None,
        }
    }

    fn syntax(&self) -> &SyntaxNode {
        match self {
            Self::Binary(it) => it.syntax(),
            Self::Prefix(it) => it.syntax(),
            Self::Member(it) => it.syntax(),
            Self::Path(it) => it.syntax(),
            Self::Paren(it) => it.syntax(),
            Self::Literal(it) => it.syntax(),
        }
    }
}

// --- the shared definition traits ----------------------------------------

/// A node that declares a [`Name`].
pub trait HasName: AstNode {
    /// The declared name — the first `Name` child.
    fn name(&self) -> Option<Name> {
        support::child(self.syntax())
    }
}

/// A node that accepts the `internal` / `error` prefix modifiers
/// (general form §2; typl reference §3.3, §10.1).
pub trait HasModifiers: AstNode {
    /// `true` when the definition carries the `internal` modifier.
    fn is_internal(&self) -> bool {
        support::token(self.syntax(), SyntaxKind::InternalKw).is_some()
    }

    /// `true` when the definition carries the `error` modifier.
    fn is_error(&self) -> bool {
        support::token(self.syntax(), SyntaxKind::ErrorKw).is_some()
    }
}

/// A node that doc comments attach to (typl reference §14). Doc comments
/// are trivia tokens, so they sit before the node rather than inside it.
pub trait HasDocComments: AstNode {
    /// The `DocComment` tokens in the trivia run immediately preceding
    /// this node, in source order.
    fn doc_comments(&self) -> Vec<SyntaxToken> {
        let mut out = Vec::new();
        let mut cursor = self.syntax().prev_sibling_or_token();
        while let Some(rowan::NodeOrToken::Token(token)) = cursor {
            if !token.kind().is_trivia() {
                break;
            }
            if token.kind() == SyntaxKind::DocComment {
                out.push(token.clone());
            }
            cursor = token.prev_sibling_or_token();
        }
        out.reverse();
        out
    }
}

impl HasName for TypeDef {}
impl HasName for ConstDef {}
impl HasName for StructDef {}
impl HasName for EnumDef {}
impl HasName for EnumSetDef {}
impl HasName for UnionDef {}
impl HasName for Definition {}
impl HasName for InterfaceDef {}
impl HasName for SignalDef {}
impl HasName for EventDef {}
impl HasName for CommandDef {}
impl HasName for QueryDef {}
impl HasName for FinalDef {}
impl HasName for InterfaceMember {}

impl HasModifiers for TypeDef {}
impl HasModifiers for ConstDef {}
impl HasModifiers for StructDef {}
impl HasModifiers for EnumDef {}
impl HasModifiers for EnumSetDef {}
impl HasModifiers for UnionDef {}
impl HasModifiers for Definition {}
impl HasModifiers for InterfaceDef {}

impl HasDocComments for TypeDef {}
impl HasDocComments for ConstDef {}
impl HasDocComments for StructDef {}
impl HasDocComments for EnumDef {}
impl HasDocComments for EnumSetDef {}
impl HasDocComments for UnionDef {}
impl HasDocComments for Definition {}
impl HasDocComments for InterfaceDef {}
impl HasDocComments for SignalDef {}
impl HasDocComments for EventDef {}
impl HasDocComments for CommandDef {}
impl HasDocComments for QueryDef {}
impl HasDocComments for FinalDef {}
impl HasDocComments for InterfaceMember {}

// --- position-sensitive accessors ----------------------------------------

/// The role a scalar literal plays inside a [`Constraint`], decided by the
/// anchor token last seen before it (`..`, `step`, `match`).
#[derive(Clone, Copy, PartialEq, Eq)]
enum ScalarRole {
    Min,
    Max,
    Step,
    Pattern,
}

impl Constraint {
    /// The lower endpoint of a scalar range — the literal before `..`.
    pub fn min(&self) -> Option<Literal> {
        self.scalar(ScalarRole::Min)
    }

    /// The upper endpoint of a scalar range — the literal after `..`.
    pub fn max(&self) -> Option<Literal> {
        self.scalar(ScalarRole::Max)
    }

    /// The step of a scalar range — the literal after the `step` keyword
    /// (typl reference §4.3).
    pub fn step(&self) -> Option<Literal> {
        self.scalar(ScalarRole::Step)
    }

    /// The pattern after the `match` keyword — a regex literal or a named
    /// constant reference (typl reference §5.3).
    pub fn match_pattern(&self) -> Option<Literal> {
        self.scalar(ScalarRole::Pattern)
    }

    /// The first direct `Literal` child in the given role. The scalar
    /// endpoints of a constraint are all `Literal` nodes, so the anchor
    /// tokens between them decide which is which; the literals of a length
    /// bound sit inside a [`Bound`] child and never surface here.
    fn scalar(&self, wanted: ScalarRole) -> Option<Literal> {
        let mut role = ScalarRole::Min;
        for element in self.syntax().children_with_tokens() {
            match element {
                rowan::NodeOrToken::Token(token) => match token.kind() {
                    SyntaxKind::DotDot => role = ScalarRole::Max,
                    SyntaxKind::StepKw => role = ScalarRole::Step,
                    SyntaxKind::MatchKw => role = ScalarRole::Pattern,
                    _ => {}
                },
                rowan::NodeOrToken::Node(node) => {
                    if let Some(literal) = Literal::cast(node)
                        && role == wanted
                    {
                        return Some(literal);
                    }
                }
            }
        }
        None
    }
}

impl ConstDef {
    /// The regex token of a regex constant (typl reference §6.2), inside
    /// the value literal.
    pub fn regex(&self) -> Option<SyntaxToken> {
        self.value()?.regex_token()
    }
}

impl StreamType {
    /// The raw `string`/`bytes` element keyword (ridl reference §12.2) —
    /// the bare token child of the stream, present only when the element
    /// is not a named type ([`StreamType::element_type`]).
    pub fn element_primitive(&self) -> Option<SyntaxToken> {
        self.syntax()
            .children_with_tokens()
            .filter_map(|element| element.into_token())
            .find(|token| matches!(token.kind(), SyntaxKind::StringKw | SyntaxKind::BytesKw))
    }
}

impl Timing {
    /// The strict-periodic duration — `@10ms` (ridl reference §9). `None`
    /// for the range form, whose durations sit inside [`Timing::range`].
    pub fn duration(&self) -> Option<SyntaxToken> {
        support::token(self.syntax(), SyntaxKind::Duration)
    }
}

/// Which predicate keyword an [`Attribute`] carries (general form §4.2,
/// ridl reference §13).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PredicateKind {
    Require,
    Ensure,
}

impl Attribute {
    /// The predicate keyword of a `require`/`ensure` attribute; `None` for
    /// the flag and assignment forms.
    pub fn predicate_kind(&self) -> Option<PredicateKind> {
        if self.require_token().is_some() {
            Some(PredicateKind::Require)
        } else if self.ensure_token().is_some() {
            Some(PredicateKind::Ensure)
        } else {
            None
        }
    }
}

/// Whether `kind` is one of the binary operators of the guaranteed subset
/// (expr-core specification §3.1).
fn is_binary_op(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::PipePipe
            | SyntaxKind::AmpAmp
            | SyntaxKind::EqEq
            | SyntaxKind::Neq
            | SyntaxKind::Lt
            | SyntaxKind::Le
            | SyntaxKind::Gt
            | SyntaxKind::Ge
            | SyntaxKind::Plus
            | SyntaxKind::Minus
            | SyntaxKind::Star
            | SyntaxKind::Slash
            | SyntaxKind::Percent
    )
}

impl BinaryExpr {
    /// The operator token of this binary step — the one direct token child
    /// drawn from the subset operator set.
    pub fn op_token(&self) -> Option<SyntaxToken> {
        self.syntax()
            .children_with_tokens()
            .filter_map(|element| element.into_token())
            .find(|token| is_binary_op(token.kind()))
    }
}

impl PrefixExpr {
    /// The prefix operator — `!` or `-`.
    pub fn op_token(&self) -> Option<SyntaxToken> {
        self.syntax()
            .children_with_tokens()
            .filter_map(|element| element.into_token())
            .find(|token| matches!(token.kind(), SyntaxKind::Bang | SyntaxKind::Minus))
    }
}

impl MemberExpr {
    /// The member name after the `.` — a tuple field or an enum member.
    pub fn member_token(&self) -> Option<SyntaxToken> {
        self.ident_token()
    }
}

impl PathExpr {
    /// The referenced name — a parameter, `result`, a constant, or a type.
    pub fn name_token(&self) -> Option<SyntaxToken> {
        self.ident_token()
    }
}

impl LiteralExpr {
    /// The literal or duration value token.
    pub fn token(&self) -> Option<SyntaxToken> {
        self.syntax()
            .children_with_tokens()
            .filter_map(|element| element.into_token())
            .find(|token| !token.kind().is_trivia())
    }
}

impl TimingRange {
    /// The lower bound — the duration before `..`. `None` on `[..max]`.
    pub fn min(&self) -> Option<SyntaxToken> {
        self.endpoint(false)
    }

    /// The upper bound — the duration after `..`. `None` on `[min..]`.
    pub fn max(&self) -> Option<SyntaxToken> {
        self.endpoint(true)
    }

    /// The first duration token before (`after_dots == false`) or after
    /// (`after_dots == true`) the `..` anchor — both endpoints are
    /// `duration` tokens, so position relative to `..` decides the role.
    fn endpoint(&self, after_dots: bool) -> Option<SyntaxToken> {
        let mut seen_dots = false;
        for element in self.syntax().children_with_tokens() {
            let Some(token) = element.into_token() else {
                continue;
            };
            match token.kind() {
                SyntaxKind::DotDot => seen_dots = true,
                SyntaxKind::Duration if seen_dots == after_dots => return Some(token),
                _ => {}
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use rowan::GreenNodeBuilder;

    use super::*;
    use crate::syntax_kind::{SyntaxKind, SyntaxNode};

    fn token(builder: &mut GreenNodeBuilder<'static>, kind: SyntaxKind, text: &str) {
        builder.token(kind.into(), text);
    }

    fn literal(builder: &mut GreenNodeBuilder<'static>, kind: SyntaxKind, text: &str) {
        builder.start_node(SyntaxKind::Literal.into());
        token(builder, kind, text);
        builder.finish_node();
    }

    fn name(builder: &mut GreenNodeBuilder<'static>, text: &str) {
        builder.start_node(SyntaxKind::Name.into());
        token(builder, SyntaxKind::Ident, text);
        builder.finish_node();
    }

    fn path_type(builder: &mut GreenNodeBuilder<'static>, text: &str) {
        builder.start_node(SyntaxKind::PathType.into());
        builder.start_node(SyntaxKind::QualifiedName.into());
        token(builder, SyntaxKind::Ident, text);
        builder.finish_node();
        builder.finish_node();
    }

    /// `/// Calibrated top speed.` + `internal type Speed: km/h [0.0..250.0
    /// step 0.5] = 0.0`, built by hand — the full parser lands in task
    /// E1.2b.
    fn speed_source_file() -> SyntaxNode {
        let mut b = GreenNodeBuilder::new();
        b.start_node(SyntaxKind::SourceFile.into());
        token(&mut b, SyntaxKind::DocComment, "/// Calibrated top speed.");
        token(&mut b, SyntaxKind::Whitespace, "\n");
        b.start_node(SyntaxKind::TypeDef.into());
        token(&mut b, SyntaxKind::InternalKw, "internal");
        token(&mut b, SyntaxKind::Whitespace, " ");
        token(&mut b, SyntaxKind::TypeKw, "type");
        token(&mut b, SyntaxKind::Whitespace, " ");
        name(&mut b, "Speed");
        token(&mut b, SyntaxKind::Colon, ":");
        token(&mut b, SyntaxKind::Whitespace, " ");
        b.start_node(SyntaxKind::UnitExpr.into());
        token(&mut b, SyntaxKind::Ident, "km");
        token(&mut b, SyntaxKind::Slash, "/");
        token(&mut b, SyntaxKind::Ident, "h");
        b.finish_node();
        token(&mut b, SyntaxKind::Whitespace, " ");
        b.start_node(SyntaxKind::Constraint.into());
        token(&mut b, SyntaxKind::LBracket, "[");
        literal(&mut b, SyntaxKind::FloatNumber, "0.0");
        token(&mut b, SyntaxKind::DotDot, "..");
        literal(&mut b, SyntaxKind::FloatNumber, "250.0");
        token(&mut b, SyntaxKind::Whitespace, " ");
        token(&mut b, SyntaxKind::StepKw, "step");
        token(&mut b, SyntaxKind::Whitespace, " ");
        literal(&mut b, SyntaxKind::FloatNumber, "0.5");
        token(&mut b, SyntaxKind::RBracket, "]");
        b.finish_node();
        token(&mut b, SyntaxKind::Whitespace, " ");
        b.start_node(SyntaxKind::InitValue.into());
        token(&mut b, SyntaxKind::Eq, "=");
        token(&mut b, SyntaxKind::Whitespace, " ");
        literal(&mut b, SyntaxKind::FloatNumber, "0.0");
        b.finish_node();
        b.finish_node();
        b.finish_node();
        SyntaxNode::new_root(b.finish())
    }

    #[test]
    fn type_def_cast_round_trips() {
        let root = speed_source_file();
        assert_eq!(
            root.text().to_string(),
            "/// Calibrated top speed.\ninternal type Speed: km/h [0.0..250.0 step 0.5] = 0.0",
        );

        let file = SourceFile::cast(root).expect("root is a SourceFile");
        assert!(file.package_decl().is_none());
        assert_eq!(file.imports().count(), 0);

        let definitions: Vec<Definition> = file.definitions().collect();
        assert_eq!(definitions.len(), 1);
        let Definition::Type(type_def) = &definitions[0] else {
            panic!("expected Definition::Type, got {:?}", definitions[0]);
        };

        let name = type_def.name().expect("TypeDef has a Name");
        assert_eq!(
            name.ident_token().expect("Name wraps an Ident").text(),
            "Speed"
        );

        let Some(Backing::Unit(unit)) = type_def.backing() else {
            panic!("expected a unit backing");
        };
        assert_eq!(unit.syntax().text().to_string(), "km/h");

        let constraint = type_def.constraint().expect("TypeDef has a Constraint");
        assert_eq!(constraint.min().unwrap().syntax().text().to_string(), "0.0");
        assert_eq!(
            constraint.max().unwrap().syntax().text().to_string(),
            "250.0"
        );
        assert_eq!(
            constraint.step().unwrap().syntax().text().to_string(),
            "0.5"
        );
        assert!(constraint.match_pattern().is_none());
        assert!(constraint.len().is_none());

        let init = type_def.init_value().expect("TypeDef has an InitValue");
        assert_eq!(init.literal().unwrap().syntax().text().to_string(), "0.0");

        assert!(type_def.is_internal());
        assert!(!type_def.is_error());

        let docs = type_def.doc_comments();
        assert_eq!(docs.len(), 1);
        assert_eq!(docs[0].text(), "/// Calibrated top speed.");

        // The trait trio also answers through the `Definition` enum.
        let definition = &definitions[0];
        assert_eq!(
            definition.name().unwrap().ident_token().unwrap().text(),
            "Speed",
        );
        assert!(definition.is_internal());
        assert_eq!(definition.doc_comments().len(), 1);
    }

    /// `struct DriverProfile { name: Name / reserved legacyChecksum /
    /// speed: Speed? }` — the §7.4 tombstone example shape.
    fn driver_profile_source_file() -> SyntaxNode {
        let mut b = GreenNodeBuilder::new();
        b.start_node(SyntaxKind::SourceFile.into());
        b.start_node(SyntaxKind::StructDef.into());
        token(&mut b, SyntaxKind::StructKw, "struct");
        token(&mut b, SyntaxKind::Whitespace, " ");
        name(&mut b, "DriverProfile");
        token(&mut b, SyntaxKind::Whitespace, " ");
        token(&mut b, SyntaxKind::LBrace, "{");
        token(&mut b, SyntaxKind::Whitespace, "\n  ");
        b.start_node(SyntaxKind::FieldDef.into());
        name(&mut b, "name");
        token(&mut b, SyntaxKind::Colon, ":");
        token(&mut b, SyntaxKind::Whitespace, " ");
        path_type(&mut b, "Name");
        b.finish_node();
        token(&mut b, SyntaxKind::Whitespace, "\n  ");
        b.start_node(SyntaxKind::ReservedEntry.into());
        token(&mut b, SyntaxKind::ReservedKw, "reserved");
        token(&mut b, SyntaxKind::Whitespace, " ");
        name(&mut b, "legacyChecksum");
        b.finish_node();
        token(&mut b, SyntaxKind::Whitespace, "\n  ");
        b.start_node(SyntaxKind::FieldDef.into());
        name(&mut b, "speed");
        token(&mut b, SyntaxKind::Colon, ":");
        token(&mut b, SyntaxKind::Whitespace, " ");
        b.start_node(SyntaxKind::OptionalType.into());
        path_type(&mut b, "Speed");
        token(&mut b, SyntaxKind::Question, "?");
        b.finish_node();
        b.finish_node();
        token(&mut b, SyntaxKind::Whitespace, "\n");
        token(&mut b, SyntaxKind::RBrace, "}");
        b.finish_node();
        b.finish_node();
        SyntaxNode::new_root(b.finish())
    }

    #[test]
    fn struct_def_cast_round_trips() {
        let root = driver_profile_source_file();
        assert_eq!(
            root.text().to_string(),
            "struct DriverProfile {\n  name: Name\n  reserved legacyChecksum\n  speed: Speed?\n}",
        );

        let file = SourceFile::cast(root).expect("root is a SourceFile");
        let definitions: Vec<Definition> = file.definitions().collect();
        assert_eq!(definitions.len(), 1);
        let Definition::Struct(struct_def) = &definitions[0] else {
            panic!("expected Definition::Struct, got {:?}", definitions[0]);
        };

        assert_eq!(
            struct_def.name().unwrap().ident_token().unwrap().text(),
            "DriverProfile",
        );
        assert!(!struct_def.is_internal());
        assert!(!struct_def.is_error());

        let members: Vec<StructMember> = struct_def.members().collect();
        assert_eq!(members.len(), 3);

        let StructMember::Field(name_field) = &members[0] else {
            panic!("expected a field, got {:?}", members[0]);
        };
        assert_eq!(
            name_field.name().unwrap().ident_token().unwrap().text(),
            "name",
        );
        let Some(FieldType::Path(path)) = name_field.field_type() else {
            panic!("expected a path field type");
        };
        assert_eq!(path.syntax().text().to_string(), "Name");
        assert!(name_field.init_value().is_none());

        let StructMember::Reserved(tombstone) = &members[1] else {
            panic!("expected a reserved entry, got {:?}", members[1]);
        };
        assert_eq!(
            tombstone.name().unwrap().ident_token().unwrap().text(),
            "legacyChecksum",
        );
        assert!(tombstone.literal().is_none());

        let StructMember::Field(speed_field) = &members[2] else {
            panic!("expected a field, got {:?}", members[2]);
        };
        let Some(FieldType::Optional(optional)) = speed_field.field_type() else {
            panic!("expected an optional field type");
        };
        assert!(optional.question_token().is_some());
        let Some(FieldType::Path(inner)) = optional.field_type() else {
            panic!("expected a path inside the optional");
        };
        assert_eq!(inner.syntax().text().to_string(), "Speed");
    }

    #[test]
    fn const_def_regex_accessor_finds_the_pattern() {
        let mut b = GreenNodeBuilder::new();
        b.start_node(SyntaxKind::SourceFile.into());
        b.start_node(SyntaxKind::ConstDef.into());
        token(&mut b, SyntaxKind::ConstKw, "const");
        token(&mut b, SyntaxKind::Whitespace, " ");
        name(&mut b, "SPEED_PATTERN");
        token(&mut b, SyntaxKind::Whitespace, " ");
        token(&mut b, SyntaxKind::Eq, "=");
        token(&mut b, SyntaxKind::Whitespace, " ");
        literal(&mut b, SyntaxKind::Regex, r"/^\d+(\.\d+)?$/");
        b.finish_node();
        b.finish_node();
        let root = SyntaxNode::new_root(b.finish());

        let file = SourceFile::cast(root).expect("root is a SourceFile");
        let Some(Definition::Const(const_def)) = file.definitions().next() else {
            panic!("expected a const definition");
        };
        assert_eq!(
            const_def.name().unwrap().ident_token().unwrap().text(),
            "SPEED_PATTERN",
        );
        assert!(const_def.type_ref().is_none());
        assert_eq!(const_def.regex().unwrap().text(), r"/^\d+(\.\d+)?$/");
    }

    /// `error type Broken: boolean` — the grammar admits a misplaced
    /// `error` modifier on every definition kind, so the checker (not the
    /// parser) can reject it with TYPL-212; `is_error()` must see it.
    #[test]
    fn misplaced_error_modifier_is_held_and_found() {
        let mut b = GreenNodeBuilder::new();
        b.start_node(SyntaxKind::SourceFile.into());
        b.start_node(SyntaxKind::TypeDef.into());
        token(&mut b, SyntaxKind::ErrorKw, "error");
        token(&mut b, SyntaxKind::Whitespace, " ");
        token(&mut b, SyntaxKind::TypeKw, "type");
        token(&mut b, SyntaxKind::Whitespace, " ");
        name(&mut b, "Broken");
        token(&mut b, SyntaxKind::Colon, ":");
        token(&mut b, SyntaxKind::Whitespace, " ");
        b.start_node(SyntaxKind::PrimitiveType.into());
        token(&mut b, SyntaxKind::BooleanKw, "boolean");
        b.finish_node();
        b.finish_node();
        b.finish_node();
        let root = SyntaxNode::new_root(b.finish());

        let file = SourceFile::cast(root).expect("root is a SourceFile");
        let Some(Definition::Type(type_def)) = file.definitions().next() else {
            panic!("expected a type definition");
        };
        assert!(type_def.is_error());
        assert!(type_def.error_token().is_some());
        assert!(!type_def.is_internal());
    }

    /// Builds a bare `Constraint` node: `[` + the given body + `]`.
    fn constraint(build: impl FnOnce(&mut GreenNodeBuilder<'static>)) -> Constraint {
        let mut b = GreenNodeBuilder::new();
        b.start_node(SyntaxKind::Constraint.into());
        token(&mut b, SyntaxKind::LBracket, "[");
        build(&mut b);
        token(&mut b, SyntaxKind::RBracket, "]");
        b.finish_node();
        Constraint::cast(SyntaxNode::new_root(b.finish())).expect("Constraint casts")
    }

    /// A `Bound` node: one literal, or two literals around `..`.
    fn bound(builder: &mut GreenNodeBuilder<'static>, min: &str, max: Option<&str>) {
        builder.start_node(SyntaxKind::Bound.into());
        literal(builder, SyntaxKind::IntNumber, min);
        if let Some(max) = max {
            token(builder, SyntaxKind::DotDot, "..");
            literal(builder, SyntaxKind::IntNumber, max);
        }
        builder.finish_node();
    }

    /// Open ranges: the scalar accessors key on the anchor tokens, not on
    /// child positions, so `[..255]` has no min and `[0..]` has no max.
    #[test]
    fn open_ranges_anchor_the_scalar_roles() {
        // `[..255]` — the literal after a leading `..` is the max.
        let upper_only = constraint(|b| {
            token(b, SyntaxKind::DotDot, "..");
            literal(b, SyntaxKind::IntNumber, "255");
        });
        assert!(upper_only.min().is_none());
        assert_eq!(upper_only.max().unwrap().syntax().text().to_string(), "255");
        assert!(upper_only.step().is_none());
        assert!(upper_only.match_pattern().is_none());
        assert!(upper_only.len().is_none());

        // `[0..]` — the literal before a trailing `..` is the min.
        let lower_only = constraint(|b| {
            literal(b, SyntaxKind::IntNumber, "0");
            token(b, SyntaxKind::DotDot, "..");
        });
        assert_eq!(lower_only.min().unwrap().syntax().text().to_string(), "0");
        assert!(lower_only.max().is_none());
        assert!(lower_only.step().is_none());
        assert!(lower_only.match_pattern().is_none());
    }

    /// `[match PATTERN]` — a bare match pattern is not a scalar endpoint.
    #[test]
    fn bare_match_pattern_is_not_a_scalar_endpoint() {
        let bare_match = constraint(|b| {
            token(b, SyntaxKind::MatchKw, "match");
            token(b, SyntaxKind::Whitespace, " ");
            literal(b, SyntaxKind::Ident, "PATTERN");
        });
        assert!(bare_match.min().is_none());
        assert!(bare_match.max().is_none());
        assert!(bare_match.step().is_none());
        assert!(bare_match.len().is_none());
        assert_eq!(
            bare_match
                .match_pattern()
                .unwrap()
                .syntax()
                .text()
                .to_string(),
            "PATTERN",
        );
    }

    /// Length bounds nest their literals inside a `Bound` child: they never
    /// leak into the scalar accessors, and the `..` inside the bound does
    /// not move the anchor-token role tracker.
    #[test]
    fn length_bound_literals_do_not_leak_into_the_scalar_accessors() {
        // `[17 match /re/]` — exact length plus pattern.
        let exact_len = constraint(|b| {
            bound(b, "17", None);
            token(b, SyntaxKind::Whitespace, " ");
            token(b, SyntaxKind::MatchKw, "match");
            token(b, SyntaxKind::Whitespace, " ");
            literal(b, SyntaxKind::Regex, "/re/");
        });
        assert!(exact_len.min().is_none());
        assert!(exact_len.max().is_none());
        let len = exact_len.len().expect("the length bound is present");
        assert_eq!(len.min().unwrap().syntax().text().to_string(), "17");
        assert!(len.max().is_none());
        assert_eq!(
            exact_len
                .match_pattern()
                .unwrap()
                .syntax()
                .text()
                .to_string(),
            "/re/",
        );

        // `[3..6 match P]` — length range plus named pattern.
        let len_range = constraint(|b| {
            bound(b, "3", Some("6"));
            token(b, SyntaxKind::Whitespace, " ");
            token(b, SyntaxKind::MatchKw, "match");
            token(b, SyntaxKind::Whitespace, " ");
            literal(b, SyntaxKind::Ident, "P");
        });
        assert!(len_range.min().is_none());
        assert!(len_range.max().is_none());
        let len = len_range.len().expect("the length bound is present");
        assert_eq!(len.min().unwrap().syntax().text().to_string(), "3");
        assert_eq!(len.max().unwrap().syntax().text().to_string(), "6");
        assert_eq!(
            len_range
                .match_pattern()
                .unwrap()
                .syntax()
                .text()
                .to_string(),
            "P",
        );
    }

    /// The attribute and expression accessors (E2.4) over a parsed tree:
    /// the surface tasks 5, 11, 12, and 21 consume.
    #[test]
    fn attribute_and_expr_accessors_read_a_parsed_contract() {
        use crate::{Profile, parse};

        let input = "package p\ninterface I {\n  \
            command setGear(position: GearPosition) [\n    \
            require position != GearPosition.PARK || currentSpeed == 0.0\n    \
            someKey = 3\n  ]\n}\n";
        let parsed = parse(input, Profile::Ridl);
        assert!(parsed.errors().is_empty(), "got: {:?}", parsed.errors());

        let attr_block = parsed
            .syntax()
            .descendants()
            .find_map(AttrBlock::cast)
            .expect("an AttrBlock parses");
        let attributes: Vec<Attribute> = attr_block.attributes().collect();
        assert_eq!(attributes.len(), 2);

        // The predicate form: kind + expr, no key, no value.
        let predicate = &attributes[0];
        assert_eq!(predicate.predicate_kind(), Some(PredicateKind::Require));
        assert!(predicate.key().is_none());
        assert!(predicate.value().is_none());
        let Some(Expr::Binary(or)) = predicate.expr() else {
            panic!("expected a binary root, got {:?}", predicate.expr());
        };
        assert_eq!(or.op_token().unwrap().text(), "||");
        let Some(Expr::Binary(neq)) = or.lhs() else {
            panic!("expected a binary lhs, got {:?}", or.lhs());
        };
        assert_eq!(neq.op_token().unwrap().text(), "!=");
        let Some(Expr::Member(member)) = neq.rhs() else {
            panic!("expected a member rhs, got {:?}", neq.rhs());
        };
        assert_eq!(member.member_token().unwrap().text(), "PARK");
        let Some(Expr::Path(base)) = member.base() else {
            panic!("expected a path base, got {:?}", member.base());
        };
        assert_eq!(base.name_token().unwrap().text(), "GearPosition");
        let Some(Expr::Binary(eq)) = or.rhs() else {
            panic!("expected a binary rhs, got {:?}", or.rhs());
        };
        let Some(Expr::Literal(zero)) = eq.rhs() else {
            panic!("expected a literal rhs, got {:?}", eq.rhs());
        };
        assert_eq!(zero.token().unwrap().text(), "0.0");

        // The assignment form: key + value, no predicate, no expr.
        let assignment = &attributes[1];
        assert!(assignment.predicate_kind().is_none());
        assert!(assignment.expr().is_none());
        assert_eq!(
            assignment.key().unwrap().ident_token().unwrap().text(),
            "someKey",
        );
        let value = assignment.value().expect("the assignment has a value");
        assert_eq!(value.literal().unwrap().syntax().text().to_string(), "3");
    }

    #[test]
    fn enum_set_def_derived_form_exposes_the_backing_ref() {
        let mut b = GreenNodeBuilder::new();
        b.start_node(SyntaxKind::SourceFile.into());
        b.start_node(SyntaxKind::EnumSetDef.into());
        token(&mut b, SyntaxKind::EnumsetKw, "enumset");
        token(&mut b, SyntaxKind::Whitespace, " ");
        name(&mut b, "WarningFlags");
        token(&mut b, SyntaxKind::Colon, ":");
        token(&mut b, SyntaxKind::Whitespace, " ");
        b.start_node(SyntaxKind::PathType.into());
        b.start_node(SyntaxKind::QualifiedName.into());
        token(&mut b, SyntaxKind::Ident, "Warning");
        b.finish_node();
        b.finish_node();
        b.finish_node();
        b.finish_node();
        let root = SyntaxNode::new_root(b.finish());

        let file = SourceFile::cast(root).expect("root is a SourceFile");
        let Some(Definition::EnumSet(enum_set)) = file.definitions().next() else {
            panic!("expected an enumset definition");
        };
        assert_eq!(
            enum_set.name().unwrap().ident_token().unwrap().text(),
            "WarningFlags",
        );
        assert_eq!(
            enum_set.backing_ref().unwrap().syntax().text().to_string(),
            "Warning",
        );
        assert_eq!(enum_set.bits().count(), 0);
    }
}
