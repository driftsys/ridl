//! The path to one type position, as the lowering and the podbuf walk both
//! spell it. The lowering records the path at which it lifts a tuple
//! (`lower::Path::key`), and the podbuf walk looks the tuple up by the path at
//! which it lays the position out (`podbuf::TupleKeys`), so the two have to
//! spell one position the same way. Each step is one variant here, built from
//! the lowering's position and from the walk's [`Source`] by the same type, so
//! a step that one walk spells and the other cannot is a missing variant,
//! never a second string format.

use std::fmt;

use crate::projection::podbuf::Source;
use crate::v2;

/// Where a path starts.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum Root {
    /// A declaration of the package being lowered, by its declared name.
    Declaration(String),
    /// A declaration of another package.
    Foreign { package: String, name: String },
    /// A named tuple of `Model.tuples`, by its index: the podbuf walk fills
    /// it as a root of its own, so a tuple inside it is reached under it.
    Tuple(u32),
}

impl Root {
    /// The root of the path to `decl` of `declaring`, from the package being
    /// lowered, `package`.
    pub fn of(decl: &v2::Decl, declaring: &v2::Package, package: &v2::Package) -> Self {
        if declaring.name == package.name {
            Self::Declaration(decl.name.clone())
        } else {
            Self::Foreign {
                package: declaring.name.clone(),
                name: decl.name.clone(),
            }
        }
    }
}

/// One step of a path: the root it starts from, then a struct field's name, a
/// tuple field's position, a map entry's `key` or `value`. An array element
/// adds nothing.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum Step {
    Root(Root),
    Field(String),
    Position(u32),
    Key,
    Value,
    /// An arm of a union whose type the projection did not find, by its
    /// ordinal. Only the podbuf walk spells it: the lowering reaches an arm
    /// through the arm's type, and the projection has already refused a union
    /// with an arm of no type, so no lookup under this step finds a tuple.
    Arm(u32),
}

impl Step {
    /// The step a field of a head adds to the path, when it adds one: a flags
    /// field, a union's tag and its arm descriptor add none.
    pub fn of(source: &Source) -> Option<Self> {
        match source {
            Source::Field { name, .. } => Some(Self::Field(name.clone())),
            Source::Position { position, .. } => Some(Self::Position(*position)),
            Source::Key => Some(Self::Key),
            Source::Value => Some(Self::Value),
            Source::Flags { .. } | Source::Tag | Source::Arm => None,
        }
    }
}

impl fmt::Display for Step {
    /// The step as an error message spells it: `Hub`, `pod.parts.Hub`,
    /// `tuple 3`, a field's name, a position's number, `key`, `value`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Root(Root::Declaration(name)) => f.write_str(name),
            Self::Root(Root::Foreign { package, name }) => write!(f, "{package}.{name}"),
            Self::Root(Root::Tuple(index)) => write!(f, "tuple {index}"),
            Self::Field(name) => f.write_str(name),
            Self::Position(position) => write!(f, "{position}"),
            Self::Key => f.write_str("key"),
            Self::Value => f.write_str("value"),
            Self::Arm(ordinal) => write!(f, "{ordinal}"),
        }
    }
}
