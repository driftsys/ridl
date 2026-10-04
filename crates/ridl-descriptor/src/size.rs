//! The size state of a payload, per core encoding (spec D-6, as re-baselined
//! on 2026-10-03): absent (no row), bounded with a byte count, or unbounded
//! with a cause (driver §4 answer 5). A bound is an upper bound: the largest
//! legal value of the payload encodes to at most that many bytes under the
//! projection the wire backend emits (ADR-0017 for proto3, ADR-0019 for
//! FlatBuffers). Envelope and framing are excluded (spec D-7). `repr(C)` has
//! no state until E11.12 (driftsys/ridl#317) defines the layout.
//!
//! The descriptor defines no wire shape (§4 answer 6): only a payload that is
//! one named type is sized, through the projections —
//! `ridl_ir::projection::flatbuffers::max_size` (ADR-0019 decision 8) in
//! `flatbuffers`, and ADR-0017's projection in `proto3`. This module holds the
//! context both sizers resolve names with, the string capacity, and the leaf
//! model `proto3` counts over. The FlatBuffers sizer is a placeholder until
//! Task 7 of the catalog descriptor plan lands. A request of zero or several
//! parameters, an inline `T | E` reply and a stream payload (§4 answer 10)
//! are absent until a record defines their encoding.

pub(crate) mod flatbuffers;
pub(crate) mod proto3;

use ridl_ir::projection::flatbuffers::Packages;
use ridl_ir::projection::proto3::{self as proto3_projection, Scalar};
use ridl_ir::v2::{
    ArrayType, Constraint, Decl, FieldType, MapType, Package, Param, PrimitiveType, ReturnType,
    StructDef, TupleType, TypeDef, UnionDef, backing, decl, field_type, return_type,
};

use crate::{Encoding, UnboundedCause};

/// Name resolution over a package and the packages it imports, and the
/// projection's view of the same scope.
pub struct Ctx<'a> {
    packages: Packages<'a>,
    /// Every package of the scope, the root first, then `others` in order.
    /// A package in `others` with the root's name is kept here, but
    /// `packages_for` finds the root first, so the later one is never
    /// returned.
    scope: Vec<&'a Package>,
    /// For each package of `scope`, at the same position, the packages
    /// beside it, in `scope` order: what `packages_for` roots a `Packages`
    /// with. A package is never beside a package of its own name, so a
    /// package of `others` with the root's name is dropped from the root's
    /// list. In the list of any other package the root comes first, so the
    /// root shadows that package in name lookups.
    others_of: Vec<Vec<&'a Package>>,
}

impl<'a> Ctx<'a> {
    pub fn new(package: &'a Package, others: &'a [&'a Package]) -> Self {
        let scope: Vec<&'a Package> = std::iter::once(package)
            .chain(others.iter().copied())
            .collect();
        let others_of = scope
            .iter()
            .map(|home| {
                scope
                    .iter()
                    .copied()
                    .filter(|candidate| candidate.name != home.name)
                    .collect()
            })
            .collect();
        Self {
            packages: Packages { package, others },
            scope,
            others_of,
        }
    }

    /// The declaration `name` refers to, read from `home`, and the package
    /// that declares it — the package a bare name inside that declaration
    /// then resolves against. This is the projection's own name rule
    /// (`Packages::resolve`): a bare `Name` is looked up in `home`, a
    /// `pkg.Name` in the package called `pkg`, whichever package that is.
    pub fn resolve(&self, home: &'a Package, name: &str) -> Option<(&'a Decl, &'a Package)> {
        self.packages.resolve(home, name)
    }

    /// The projection's view of the scope, rooted at the package the catalog
    /// is built for: the home of a payload's type name.
    pub fn packages(&self) -> Packages<'a> {
        self.packages
    }

    /// The projection's view of the scope rooted at `declaring`, the package
    /// [`Ctx::resolve`] returned a declaration with. `max_size` resolves a
    /// bare name inside a declaration against the `package` of the
    /// `Packages` it is given, so a declaration from an imported package is
    /// sized with this, not with [`Ctx::packages`] — the way the codegen
    /// lowering roots its bound (`Lowering::payload` in `ridl-ir`). `None`
    /// when `declaring` is not a package of this scope.
    pub fn packages_for(&self, declaring: &Package) -> Option<Packages<'_>> {
        let position = self
            .scope
            .iter()
            .position(|candidate| candidate.name == declaring.name)?;
        Some(Packages {
            package: self.scope[position],
            others: &self.others_of[position],
        })
    }
}

/// What is being sized.
pub enum PayloadShape<'a> {
    /// A signal or event payload: a type name.
    Named(&'a str),
    /// A fixed payload.
    Field(&'a FieldType),
    /// A command or query request: the parameters.
    Params(&'a [Param]),
    /// A query response.
    Return(&'a ReturnType),
}

/// The one named type a payload is, or `None` when the payload is not one
/// named type and so has absent sizes (driver §4 answers 6 and 10).
pub fn named_payload<'a>(shape: &PayloadShape<'a>) -> Option<&'a str> {
    match shape {
        PayloadShape::Named(name) => Some(name),
        PayloadShape::Field(ty) => named_field(ty),
        PayloadShape::Params([single]) => single.r#type.as_ref().and_then(named_field),
        PayloadShape::Params(_) => None,
        PayloadShape::Return(ret) => match ret.kind.as_ref()? {
            return_type::Kind::Value(ty) => named_field(ty),
            return_type::Kind::Fallible(_) => None,
        },
    }
}

fn named_field(ty: &FieldType) -> Option<&str> {
    match ty.kind.as_ref()? {
        field_type::Kind::Named(name) => Some(name),
        _ => None,
    }
}

/// One payload's state under one encoding; the row is absent when the
/// toolchain computed no state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SizeState {
    Bounded(u32),
    Unbounded(UnboundedCause),
}

/// The state of the named type `type_name` under `encoding`: `None` when
/// this toolchain computes no state (the `repr(C)` layout is undefined, a
/// name does not resolve, or the projection has no root form for the type).
///
/// The `FlatBuffers` arm reaches a placeholder that panics until Task 7 of
/// the catalog descriptor plan replaces it; no caller outside this module's
/// tests exists before then. The `Proto3` and `ReprC` arms are final.
pub fn size_state(type_name: &str, ctx: &Ctx<'_>, encoding: Encoding) -> Option<SizeState> {
    match encoding {
        Encoding::Proto3 => proto3::state(type_name, ctx),
        Encoding::FlatBuffers => flatbuffers::state(type_name, ctx),
        Encoding::ReprC => None,
    }
}

/// The byte capacity of a `string`: its bound in scalar values times four
/// (release-scope design §3.11). `None` when the constraint carries no
/// `len_max`: the FlatBuffers projection answers `None` for a whole type that
/// holds a string without a bound, and this function agrees with it on both
/// the multiplier and the absence
/// (`string_max_bytes_agrees_with_the_flatbuffers_projection`). The
/// compiler writes typl §4.4's `[0..256]` default into `len_max`, so a
/// compiled package never reaches the `None`. No `match` narrowing (driver §4
/// answer 7; #665).
pub fn string_max_bytes(constraint: Option<&Constraint>) -> Option<u64> {
    constraint?.len_max?.checked_mul(4)
}

// The leaf model below is what the proto3 sizer counts over (`proto3`). The
// FlatBuffers sizer does not read it: the projection's `max_size` sizes a
// whole declaration.

/// A type resolved to what the proto3 sizer counts. A composite carries
/// `home`, the package a bare name inside it resolves against: the declaring
/// package of a named struct or union, the package of the field for an inline
/// tuple, array or map.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Leaf<'a> {
    /// A scalar with a largest encoding, as the proto backend spells it
    /// (`ridl_ir::projection::proto3`): never `string` or `bytes`, which are
    /// a [`Leaf::Blob`] when a constraint bounds them. An enum set is the
    /// scalar of its width (`enum_set_field_type` in the proto backend).
    Scalar(Scalar),
    /// A string or bytes value: its maximum byte length. The leaf model
    /// counts in `u64`, as the FlatBuffers projection does; the descriptor's
    /// sizes are `u32` (driver §4 answer 5), and the proto3 sizer narrows the
    /// value when it writes a row.
    Blob(u64),
    /// An enum: its smallest and largest member values, both 0 when it has
    /// no member. The proto backend refuses a value outside proto3's int32
    /// range (`emit_enum`), which the sizer reproduces from these two.
    Enum {
        min: i64,
        max: i64,
    },
    Struct {
        def: &'a StructDef,
        home: &'a Package,
    },
    Union {
        def: &'a UnionDef,
        home: &'a Package,
    },
    Tuple {
        def: &'a TupleType,
        home: &'a Package,
    },
    Array {
        def: &'a ArrayType,
        home: &'a Package,
    },
    Map {
        def: &'a MapType,
        home: &'a Package,
    },
}

/// The leaf of a field type written in `home`.
pub(crate) fn leaf_of_field_type<'a>(
    ty: &'a FieldType,
    home: &'a Package,
    ctx: &Ctx<'a>,
) -> Option<Leaf<'a>> {
    match ty.kind.as_ref()? {
        field_type::Kind::Named(name) => leaf_of_name(name, home, ctx),
        field_type::Kind::Primitive(primitive) => leaf_of_primitive(*primitive),
        field_type::Kind::InlineScalar(def) => leaf_of_type_def(def),
        field_type::Kind::Tuple(def) => Some(Leaf::Tuple { def, home }),
        field_type::Kind::Array(def) => Some(Leaf::Array { def, home }),
        field_type::Kind::Map(def) => Some(Leaf::Map { def, home }),
        // A stream has absent sizes (driver §4 answer 10).
        field_type::Kind::Stream(_) => None,
    }
}

/// The leaf of a name written in `home`.
pub(crate) fn leaf_of_name<'a>(name: &str, home: &'a Package, ctx: &Ctx<'a>) -> Option<Leaf<'a>> {
    let (decl, declaring) = ctx.resolve(home, name)?;
    match decl.kind.as_ref()? {
        decl::Kind::TypeDef(def) => leaf_of_type_def(def),
        decl::Kind::StructDef(def) => Some(Leaf::Struct {
            def,
            home: declaring,
        }),
        decl::Kind::UnionDef(def) => Some(Leaf::Union {
            def,
            home: declaring,
        }),
        decl::Kind::EnumDef(def) => {
            let values = || def.values.iter().map(|v| v.value);
            Some(Leaf::Enum {
                min: values().min().unwrap_or(0),
                max: values().max().unwrap_or(0),
            })
        }
        // The scalar of the width alone, as the proto backend resolves an
        // enum set at each use site (`enum_set_field_type`).
        decl::Kind::EnumSetDef(def) => {
            Some(Leaf::Scalar(proto3_projection::enum_set_scalar(def.width)))
        }
        decl::Kind::ConstDef(_)
        | decl::Kind::SignalDef(_)
        | decl::Kind::EventDef(_)
        | decl::Kind::CommandDef(_)
        | decl::Kind::QueryDef(_)
        | decl::Kind::FixedDef(_)
        | decl::Kind::ReservedSlot(_) => None,
    }
}

/// A bare primitive at a field position, as the proto backend projects it
/// (`proto_primitive`): an integer is `int64`, a float `double`. A bare
/// `string` or `bytes` carries no length bound, and the FlatBuffers
/// projection answers `None` for a whole type that holds one, so neither has
/// a leaf. The compiler keeps both out of a field position (TYPL-208); a map
/// key gets typl §4.4–§4.5's `[0..256]` default as an inline scalar, which
/// `leaf_of_type_def` sizes.
pub(crate) fn leaf_of_primitive<'a>(primitive: i32) -> Option<Leaf<'a>> {
    match proto3_projection::primitive(primitive) {
        Scalar::String | Scalar::Bytes => None,
        scalar => Some(Leaf::Scalar(scalar)),
    }
}

/// A named or inline scalar, as the proto backend projects it
/// (`proto_scalar`): by its width when one is set, otherwise by its backing.
/// A `string` or `bytes` is bounded by the `len_max` its constraint carries.
/// An integer or float backing without a width, a unit backing without a
/// derived width (typl §5.1) and a type def with no backing all project to
/// `string`, which no constraint bounds, so none has a leaf — the FlatBuffers
/// projection's `scalar_charge` answers `None` for the same types.
fn leaf_of_type_def<'a>(def: &'a TypeDef) -> Option<Leaf<'a>> {
    match proto3_projection::scalar(def) {
        // Like a string, a `bytes` without a `len_max` is unsizable, as the
        // FlatBuffers projection answers for the same type.
        Scalar::Bytes => def.constraint.as_ref()?.len_max.map(Leaf::Blob),
        Scalar::String => match def.backing.as_ref()?.kind.as_ref()? {
            backing::Kind::Primitive(primitive) if *primitive == PrimitiveType::String as i32 => {
                string_max_bytes(def.constraint.as_ref()).map(Leaf::Blob)
            }
            backing::Kind::Primitive(_) | backing::Kind::Unit(_) => None,
        },
        scalar => Some(Leaf::Scalar(scalar)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ridl_ir::projection::flatbuffers::max_size;
    use ridl_ir::v2::{
        Backing, CommandDef, ConstDef, EnumDef, EnumSetDef, EnumValue, EventDef, FallibleType,
        Field, FixedDef, FloatWidth, IntWidth, Package, Param, PrimitiveType, QueryDef, Reserved,
        ReturnType, SignalDef, StreamType, StructMember, TupleField, field_type, return_type,
        stream_type, struct_member, type_def,
    };

    fn constraint(len_max: Option<u64>, pattern: Option<&str>) -> Constraint {
        Constraint {
            len_max,
            pattern: pattern.map(str::to_owned),
            ..Default::default()
        }
    }

    fn named(name: &str) -> FieldType {
        FieldType {
            optional: false,
            kind: Some(field_type::Kind::Named(name.to_owned())),
        }
    }

    fn primitive(primitive: PrimitiveType) -> FieldType {
        FieldType {
            optional: false,
            kind: Some(field_type::Kind::Primitive(primitive as i32)),
        }
    }

    fn type_def(
        backing: backing::Kind,
        width: Option<type_def::Width>,
        constraint: Option<Constraint>,
    ) -> TypeDef {
        TypeDef {
            backing: Some(Backing {
                kind: Some(backing),
            }),
            constraint,
            width,
            ..Default::default()
        }
    }

    fn string_def(len_max: Option<u64>) -> TypeDef {
        type_def(
            backing::Kind::Primitive(PrimitiveType::String as i32),
            None,
            len_max.map(|n| constraint(Some(n), None)),
        )
    }

    fn bytes_def(len_max: Option<u64>) -> TypeDef {
        type_def(
            backing::Kind::Primitive(PrimitiveType::Bytes as i32),
            None,
            len_max.map(|n| constraint(Some(n), None)),
        )
    }

    fn decl(name: &str, kind: Option<decl::Kind>) -> Decl {
        Decl {
            name: name.to_owned(),
            kind,
            ..Default::default()
        }
    }

    /// A struct of one field, at ordinal 1 (typl ordinals start at 1).
    fn one_field_struct(ty: FieldType) -> StructDef {
        StructDef {
            members: vec![StructMember {
                member: Some(struct_member::Member::Field(Field {
                    name: "f".to_owned(),
                    ordinal: 1,
                    r#type: Some(ty),
                    ..Default::default()
                })),
            }],
            fixed_layout: false,
        }
    }

    fn package(name: &str, decls: Vec<Decl>) -> Package {
        Package {
            name: name.to_owned(),
            decls,
            ..Default::default()
        }
    }

    fn enum_def(values: &[i64]) -> EnumDef {
        EnumDef {
            values: values
                .iter()
                .map(|&value| EnumValue {
                    name: format!("V{}", value.unsigned_abs()),
                    value,
                    doc: String::new(),
                })
                .collect(),
            reserved: Vec::new(),
        }
    }

    #[test]
    fn a_string_counts_four_bytes_per_scalar_value_whatever_its_pattern() {
        assert_eq!(string_max_bytes(None), None, "no constraint: unsizable");
        assert_eq!(
            string_max_bytes(Some(&constraint(None, None))),
            None,
            "no `len_max`: unsizable"
        );
        assert_eq!(
            string_max_bytes(Some(&constraint(Some(17), None))),
            Some(68)
        );
        // No `match` narrowing in E16 (driver §4 answer 7; driftsys/ridl#665).
        assert_eq!(
            string_max_bytes(Some(&constraint(Some(17), Some("^[A-Z0-9]{17}$")))),
            Some(68)
        );
        assert_eq!(
            string_max_bytes(Some(&constraint(Some(u64::MAX), None))),
            None,
            "the multiplication does not wrap"
        );
    }

    #[test]
    fn string_max_bytes_agrees_with_the_flatbuffers_projection() {
        // `max_size` adds the buffer header, the box table and the string's
        // own offset, terminator and alignment slack around the characters,
        // none of which depends on the length bound. The difference between
        // a 17-character string and a 0-character string is the per-character
        // charge alone, which is what `string_max_bytes` must agree with.
        let seventeen = decl("Seventeen", Some(decl::Kind::TypeDef(string_def(Some(17)))));
        let zero = decl("Zero", Some(decl::Kind::TypeDef(string_def(Some(0)))));
        let package = package("p", vec![seventeen.clone(), zero.clone()]);
        let others: [&Package; 0] = [];
        let ctx = Ctx::new(&package, &others);
        let bound_17 = max_size(ctx.packages(), &seventeen).expect("a bounded string");
        let bound_0 = max_size(ctx.packages(), &zero).expect("a bounded string");
        assert_eq!(
            Some(bound_17 - bound_0),
            string_max_bytes(Some(&constraint(Some(17), None)))
        );
        assert!(matches!(
            leaf_of_name("Seventeen", &package, &ctx),
            Some(Leaf::Blob(68))
        ));
    }

    #[test]
    fn an_unbounded_string_or_bytes_is_unsizable_as_in_the_projection() {
        // For an unbounded string or bytes, the leaf model answers `None`
        // exactly where the projection answers `None` for the type that holds
        // it, so the proto3 column cannot claim a bound the FlatBuffers column
        // refuses for the same type. The relation does not hold elsewhere: for
        // a bare `integer` or `float` at a field position, and for an enum set
        // of unspecified width, the projection answers `None` while proto3
        // emits `int64` or `double`, so a leaf exists there. A string or bytes
        // whose bound exceeds the projection's `MAX_ENCODABLE` also has a
        // leaf, while the projection answers `None`.
        let string_no_bound = decl("S", Some(decl::Kind::TypeDef(string_def(None))));
        let bytes_no_bound = decl("B", Some(decl::Kind::TypeDef(bytes_def(None))));
        let bytes_bound = decl("B32", Some(decl::Kind::TypeDef(bytes_def(Some(32)))));
        let bare_string = decl(
            "HoldsString",
            Some(decl::Kind::StructDef(one_field_struct(primitive(
                PrimitiveType::String,
            )))),
        );
        let bare_bytes = decl(
            "HoldsBytes",
            Some(decl::Kind::StructDef(one_field_struct(primitive(
                PrimitiveType::Bytes,
            )))),
        );
        let bare_bool = decl(
            "HoldsBool",
            Some(decl::Kind::StructDef(one_field_struct(primitive(
                PrimitiveType::Boolean,
            )))),
        );
        let package = package(
            "p",
            vec![
                string_no_bound.clone(),
                bytes_no_bound.clone(),
                bytes_bound.clone(),
                bare_string.clone(),
                bare_bytes.clone(),
                bare_bool.clone(),
            ],
        );
        let others: [&Package; 0] = [];
        let ctx = Ctx::new(&package, &others);

        assert_eq!(max_size(ctx.packages(), &string_no_bound), None);
        assert!(leaf_of_name("S", &package, &ctx).is_none());
        assert_eq!(max_size(ctx.packages(), &bytes_no_bound), None);
        assert!(leaf_of_name("B", &package, &ctx).is_none());
        assert!(max_size(ctx.packages(), &bytes_bound).is_some());
        assert!(matches!(
            leaf_of_name("B32", &package, &ctx),
            Some(Leaf::Blob(32))
        ));

        // The bare primitives sit in a struct, the one position `max_size`
        // reaches a field from; the boolean shows the struct itself is sizable.
        assert!(max_size(ctx.packages(), &bare_bool).is_some());
        assert_eq!(max_size(ctx.packages(), &bare_string), None);
        assert!(leaf_of_field_type(&primitive(PrimitiveType::String), &package, &ctx).is_none());
        assert_eq!(max_size(ctx.packages(), &bare_bytes), None);
        assert!(leaf_of_field_type(&primitive(PrimitiveType::Bytes), &package, &ctx).is_none());
    }

    #[test]
    fn only_a_payload_that_is_one_named_type_is_sized() {
        let stream = FieldType {
            optional: false,
            kind: Some(field_type::Kind::Stream(StreamType {
                element: Some(stream_type::Element::Named("Point".to_owned())),
            })),
        };
        let primitive = primitive(PrimitiveType::Integer);
        let one = [Param {
            name: "at".to_owned(),
            r#type: Some(named("Point")),
        }];
        let two = [
            one[0].clone(),
            Param {
                name: "flag".to_owned(),
                r#type: Some(primitive.clone()),
            },
        ];
        let untyped_param = [Param {
            name: "at".to_owned(),
            r#type: None,
        }];
        let value = ReturnType {
            kind: Some(return_type::Kind::Value(named("Point"))),
        };
        let fallible = ReturnType {
            kind: Some(return_type::Kind::Fallible(FallibleType {
                ok: "Point".to_owned(),
                err: "Coord".to_owned(),
            })),
        };
        let streamed = ReturnType {
            kind: Some(return_type::Kind::Value(stream.clone())),
        };
        let untyped = ReturnType { kind: None };

        assert_eq!(named_payload(&PayloadShape::Named("Point")), Some("Point"));
        assert_eq!(
            named_payload(&PayloadShape::Field(&named("Point"))),
            Some("Point")
        );
        assert_eq!(
            named_payload(&PayloadShape::Field(&primitive)),
            None,
            "not a named type"
        );
        assert_eq!(
            named_payload(&PayloadShape::Field(&stream)),
            None,
            "a stream: §4 answer 10"
        );
        assert_eq!(named_payload(&PayloadShape::Params(&one)), Some("Point"));
        assert_eq!(
            named_payload(&PayloadShape::Params(&two)),
            None,
            "several parameters: §4 answer 6"
        );
        assert_eq!(
            named_payload(&PayloadShape::Params(&[])),
            None,
            "zero parameters"
        );
        assert_eq!(
            named_payload(&PayloadShape::Params(&untyped_param)),
            None,
            "one parameter with no type"
        );
        assert_eq!(named_payload(&PayloadShape::Return(&value)), Some("Point"));
        assert_eq!(
            named_payload(&PayloadShape::Return(&fallible)),
            None,
            "an inline `T | E`: §4 answer 6"
        );
        assert_eq!(named_payload(&PayloadShape::Return(&streamed)), None);
        assert_eq!(named_payload(&PayloadShape::Return(&untyped)), None);
    }

    #[test]
    fn a_primitive_leaf_has_its_proto3_width() {
        match leaf_of_primitive(PrimitiveType::Integer as i32) {
            Some(Leaf::Scalar(s)) => assert_eq!(s.max_encoded_len(), Some(10)),
            other => panic!("integer is a scalar leaf, got {other:?}"),
        }
        // A bare `string` or `bytes` has no bound, so no leaf (the projection
        // answers `None` for the same field).
        assert!(leaf_of_primitive(PrimitiveType::String as i32).is_none());
        assert!(leaf_of_primitive(PrimitiveType::Bytes as i32).is_none());
    }

    #[test]
    fn every_primitive_is_a_leaf_or_unsizable() {
        // The scalar is the proto backend's (`proto_primitive`): `int64` for
        // a bare integer, `double` for a bare float.
        assert!(matches!(
            leaf_of_primitive(PrimitiveType::Boolean as i32),
            Some(Leaf::Scalar(Scalar::Bool))
        ));
        assert!(matches!(
            leaf_of_primitive(PrimitiveType::Integer as i32),
            Some(Leaf::Scalar(Scalar::Int64))
        ));
        assert!(matches!(
            leaf_of_primitive(PrimitiveType::Float as i32),
            Some(Leaf::Scalar(Scalar::Double))
        ));
        assert!(leaf_of_primitive(PrimitiveType::String as i32).is_none());
        assert!(leaf_of_primitive(PrimitiveType::Bytes as i32).is_none());
        assert!(leaf_of_primitive(PrimitiveType::Unspecified as i32).is_none());
        assert!(
            leaf_of_primitive(99).is_none(),
            "an enum tag the IR does not define"
        );
    }

    #[test]
    fn a_type_def_leaf_follows_its_backing_and_width() {
        // The scalar is the proto backend's (`proto_scalar`); the table itself
        // is pinned in `ridl_ir::projection::proto3`. What this test pins is
        // which types have a leaf at all, and which are a `Blob`.
        let prim = |p: PrimitiveType| backing::Kind::Primitive(p as i32);
        let int_width = |w: IntWidth| Some(type_def::Width::IntWidth(w as i32));
        let float_width = |w: FloatWidth| Some(type_def::Width::FloatWidth(w as i32));
        fn leaf(def: &TypeDef) -> Option<Leaf<'_>> {
            leaf_of_type_def(def)
        }

        assert!(matches!(
            leaf(&type_def(prim(PrimitiveType::Boolean), None, None)),
            Some(Leaf::Scalar(Scalar::Bool))
        ));
        assert!(matches!(
            leaf(&type_def(
                prim(PrimitiveType::Integer),
                int_width(IntWidth::I16),
                None
            )),
            Some(Leaf::Scalar(Scalar::Sint32))
        ));
        assert!(
            leaf(&type_def(prim(PrimitiveType::Integer), None, None)).is_none(),
            "an integer backing without a width projects to `string`, which no constraint bounds"
        );
        assert!(
            leaf(&type_def(prim(PrimitiveType::Float), None, None)).is_none(),
            "a float backing without a width projects to `string`, which no constraint bounds"
        );
        assert!(matches!(
            leaf(&type_def(
                prim(PrimitiveType::Float),
                float_width(FloatWidth::F32),
                None
            )),
            Some(Leaf::Scalar(Scalar::Float))
        ));
        assert!(matches!(leaf(&string_def(Some(17))), Some(Leaf::Blob(68))));
        assert!(
            leaf(&string_def(None)).is_none(),
            "no `len_max`: unsizable, as in the projection"
        );
        assert!(matches!(leaf(&bytes_def(Some(32))), Some(Leaf::Blob(32))));
        assert!(
            leaf(&bytes_def(None)).is_none(),
            "no `len_max`: unsizable, as in the projection"
        );
        assert!(leaf(&type_def(prim(PrimitiveType::Unspecified), None, None)).is_none());

        // `ridl-sem` never emits a backing that disagrees with its width, but
        // `from_binary` decodes an IR file without validating it. The width
        // decides the leaf, as the backends project by the width first.
        assert!(matches!(
            leaf(&type_def(
                prim(PrimitiveType::Boolean),
                int_width(IntWidth::U16),
                None
            )),
            Some(Leaf::Scalar(Scalar::Uint32))
        ));
        assert!(matches!(
            leaf(&type_def(
                prim(PrimitiveType::Integer),
                float_width(FloatWidth::F32),
                None
            )),
            Some(Leaf::Scalar(Scalar::Float))
        ));
        assert!(
            matches!(
                leaf(&type_def(
                    prim(PrimitiveType::String),
                    int_width(IntWidth::U8),
                    Some(constraint(Some(17), None))
                )),
                Some(Leaf::Scalar(Scalar::Uint32))
            ),
            "a string backing with a width is the width's scalar, not a blob"
        );

        let unit = || backing::Kind::Unit("km/h".to_owned());
        assert!(matches!(
            leaf(&type_def(unit(), int_width(IntWidth::U16), None)),
            Some(Leaf::Scalar(Scalar::Uint32))
        ));
        assert!(matches!(
            leaf(&type_def(unit(), float_width(FloatWidth::F64), None)),
            Some(Leaf::Scalar(Scalar::Double))
        ));
        assert!(
            leaf(&type_def(unit(), None, None)).is_none(),
            "the proto backend emits `string` for a unit without a width"
        );
        assert!(leaf(&TypeDef::default()).is_none(), "no backing");
        assert!(
            leaf(&type_def(
                prim(PrimitiveType::Bytes),
                None,
                Some(constraint(None, None))
            ))
            .is_none(),
            "a constraint without `len_max`: unsizable"
        );
        assert!(
            matches!(
                leaf(&type_def(
                    prim(PrimitiveType::Integer),
                    Some(type_def::Width::IntWidth(99)),
                    None
                )),
                Some(Leaf::Scalar(Scalar::Int64))
            ),
            "an integer width tag the IR does not define is `int64`, as the backend emits"
        );
        assert!(
            matches!(
                leaf(&type_def(
                    prim(PrimitiveType::Float),
                    Some(type_def::Width::FloatWidth(99)),
                    None
                )),
                Some(Leaf::Scalar(Scalar::Double))
            ),
            "a float width tag the IR does not define is `double`, as the backend emits"
        );
    }

    #[test]
    fn every_declaration_kind_resolves_to_a_leaf_or_none() {
        let home = package(
            "p",
            vec![
                decl("Speed", Some(decl::Kind::TypeDef(string_def(Some(3))))),
                decl("Point", Some(decl::Kind::StructDef(StructDef::default()))),
                decl("Shape", Some(decl::Kind::UnionDef(UnionDef::default()))),
                decl("Gear", Some(decl::Kind::EnumDef(enum_def(&[-3, 0, 7])))),
                decl("Mode", Some(decl::Kind::EnumDef(enum_def(&[0, 5])))),
                decl(
                    "Flags",
                    Some(decl::Kind::EnumSetDef(EnumSetDef {
                        width: IntWidth::U8 as i32,
                        ..Default::default()
                    })),
                ),
                decl("MAX", Some(decl::Kind::ConstDef(ConstDef::default()))),
                decl("speed", Some(decl::Kind::SignalDef(SignalDef::default()))),
                decl("tick", Some(decl::Kind::EventDef(EventDef::default()))),
                decl("go", Some(decl::Kind::CommandDef(CommandDef::default()))),
                decl("ask", Some(decl::Kind::QueryDef(QueryDef::default()))),
                decl("fixed", Some(decl::Kind::FixedDef(FixedDef::default()))),
                decl("gone", Some(decl::Kind::ReservedSlot(Reserved::default()))),
                decl("Untyped", None),
                decl("Empty", Some(decl::Kind::EnumDef(enum_def(&[])))),
                decl(
                    "OddFlags",
                    Some(decl::Kind::EnumSetDef(EnumSetDef {
                        width: 99,
                        ..Default::default()
                    })),
                ),
            ],
        );
        let other = package(
            "q",
            vec![decl(
                "Thing",
                Some(decl::Kind::UnionDef(UnionDef::default())),
            )],
        );
        let others = [&other];
        let ctx = Ctx::new(&home, &others);

        assert!(matches!(
            leaf_of_name("Speed", &home, &ctx),
            Some(Leaf::Blob(12))
        ));
        assert!(matches!(
            leaf_of_name("Point", &home, &ctx),
            Some(Leaf::Struct { home: h, .. }) if h.name == "p"
        ));
        assert!(matches!(
            leaf_of_name("Shape", &home, &ctx),
            Some(Leaf::Union { home: h, .. }) if h.name == "p"
        ));
        assert!(matches!(
            leaf_of_name("Gear", &home, &ctx),
            Some(Leaf::Enum { min: -3, max: 7 })
        ));
        assert!(matches!(
            leaf_of_name("Mode", &home, &ctx),
            Some(Leaf::Enum { min: 0, max: 5 })
        ));
        assert!(
            matches!(
                leaf_of_name("Flags", &home, &ctx),
                Some(Leaf::Scalar(Scalar::Uint32))
            ),
            "an enum set is its width's scalar, as `enum_set_field_type` emits"
        );
        assert!(
            matches!(
                leaf_of_name("Empty", &home, &ctx),
                Some(Leaf::Enum { min: 0, max: 0 })
            ),
            "an enum with no member has magnitude 0"
        );
        assert!(
            matches!(
                leaf_of_name("OddFlags", &home, &ctx),
                Some(Leaf::Scalar(Scalar::Int64))
            ),
            "a width tag the IR does not define is `int64`, as the backend emits"
        );
        for name in [
            "MAX", "speed", "tick", "go", "ask", "fixed", "gone", "Untyped",
        ] {
            assert!(
                leaf_of_name(name, &home, &ctx).is_none(),
                "{name} has no leaf"
            );
        }
        assert!(
            leaf_of_name("Nowhere", &home, &ctx).is_none(),
            "an unresolved name"
        );
        assert!(
            leaf_of_name("q.Nowhere", &home, &ctx).is_none(),
            "an unresolved member of a known package"
        );
        assert!(
            leaf_of_name("r.Thing", &home, &ctx).is_none(),
            "an unknown package"
        );

        // The projection's name rule: a `pkg.Name` resolves in the package
        // called `pkg`, which may be the root package itself; a bare name
        // resolves in the home package only.
        assert!(matches!(
            leaf_of_name("p.Point", &home, &ctx),
            Some(Leaf::Struct { home: h, .. }) if h.name == "p"
        ));
        assert!(matches!(
            leaf_of_name("q.Thing", &home, &ctx),
            Some(Leaf::Union { home: h, .. }) if h.name == "q"
        ));
        assert!(
            leaf_of_name("Thing", &home, &ctx).is_none(),
            "a foreign declaration does not resolve by its bare name from p"
        );
        assert!(matches!(
            leaf_of_name("Thing", &other, &ctx),
            Some(Leaf::Union { home: h, .. }) if h.name == "q"
        ));
        assert_eq!(ctx.packages().package.name, "p");
        assert_eq!(ctx.packages().others.len(), 1);
    }

    #[test]
    fn a_bare_name_inside_an_imported_declaration_resolves_in_its_own_package() {
        // `q.Thing` holds a field `Inner`, bare, so relative to q; the root
        // package declares a different `Inner`. The field must resolve to q's.
        let root = package(
            "p",
            vec![
                decl("Inner", Some(decl::Kind::StructDef(StructDef::default()))),
                decl(
                    "OnlyInRoot",
                    Some(decl::Kind::StructDef(StructDef::default())),
                ),
            ],
        );
        let imported = package(
            "q",
            vec![
                decl(
                    "Thing",
                    Some(decl::Kind::StructDef(one_field_struct(named("Inner")))),
                ),
                decl("Inner", Some(decl::Kind::EnumDef(enum_def(&[0, 1])))),
            ],
        );
        let others = [&imported];
        let ctx = Ctx::new(&root, &others);

        let (def, home) = match leaf_of_name("q.Thing", &root, &ctx) {
            Some(Leaf::Struct { def, home }) => (def, home),
            other => panic!("q.Thing is a struct leaf, got {other:?}"),
        };
        assert_eq!(
            home.name, "q",
            "the composite carries its declaring package"
        );
        let field = match &def.members[0].member {
            Some(struct_member::Member::Field(field)) => field.r#type.as_ref().unwrap(),
            other => panic!("one field, got {other:?}"),
        };
        assert!(
            matches!(
                leaf_of_field_type(field, home, &ctx),
                Some(Leaf::Enum { min: 0, max: 1 })
            ),
            "resolved against q, the field is q's enum `Inner`"
        );
        // The same field read against the root package would be the root's
        // struct `Inner`: the home the leaf carries is what keeps the two
        // apart.
        assert!(matches!(
            leaf_of_field_type(field, &root, &ctx),
            Some(Leaf::Struct { home: h, .. }) if h.name == "p"
        ));
        assert!(
            leaf_of_name("OnlyInRoot", &imported, &ctx).is_none(),
            "a bare name only the root declares does not resolve from q"
        );
        assert!(matches!(
            leaf_of_name("OnlyInRoot", &root, &ctx),
            Some(Leaf::Struct { home: h, .. }) if h.name == "p"
        ));
    }

    #[test]
    fn packages_for_lists_the_root_first_then_the_others_in_order() {
        let root = package("p", vec![]);
        let second = package("q", vec![]);
        let third = package("r", vec![]);
        // Same name as the root, with a declaration the root lacks.
        let same_name = package(
            "p",
            vec![decl(
                "Shadowed",
                Some(decl::Kind::StructDef(StructDef::default())),
            )],
        );
        let others = [&second, &third, &same_name];
        let ctx = Ctx::new(&root, &others);
        let names = |packages: Packages<'_>| -> Vec<String> {
            std::iter::once(packages.package)
                .chain(packages.others.iter().copied())
                .map(|package| package.name.clone())
                .collect()
        };

        assert_eq!(names(ctx.packages_for(&root).unwrap()), ["p", "q", "r"]);
        assert_eq!(
            names(ctx.packages_for(&second).unwrap()),
            ["q", "p", "r", "p"]
        );
        assert_eq!(
            names(ctx.packages_for(&third).unwrap()),
            ["r", "p", "q", "p"]
        );
        // The root's own view drops the package that has the root's name, and
        // the root shadows it in every other view.
        assert!(
            ctx.packages_for(&root)
                .unwrap()
                .others
                .iter()
                .all(|other| !std::ptr::eq(*other, &same_name))
        );
        assert!(ctx.resolve(&second, "p.Shadowed").is_none());
    }

    #[test]
    fn every_field_type_kind_resolves_to_a_leaf_or_none() {
        let home = package(
            "p",
            vec![decl(
                "Point",
                Some(decl::Kind::StructDef(StructDef::default())),
            )],
        );
        let others: [&Package; 0] = [];
        let ctx = Ctx::new(&home, &others);
        let field = |kind: field_type::Kind| FieldType {
            optional: false,
            kind: Some(kind),
        };

        assert!(matches!(
            leaf_of_field_type(&named("Point"), &home, &ctx),
            Some(Leaf::Struct { home: h, .. }) if h.name == "p"
        ));
        assert!(matches!(
            leaf_of_field_type(&primitive(PrimitiveType::Boolean), &home, &ctx),
            Some(Leaf::Scalar(Scalar::Bool))
        ));
        assert!(matches!(
            leaf_of_field_type(
                &field(field_type::Kind::InlineScalar(Box::new(string_def(Some(
                    2
                ))))),
                &home,
                &ctx
            ),
            Some(Leaf::Blob(8))
        ));
        assert!(matches!(
            leaf_of_field_type(
                &field(field_type::Kind::Tuple(TupleType {
                    fields: vec![TupleField {
                        name: "x".to_owned(),
                        r#type: Some(named("Point")),
                    }],
                })),
                &home,
                &ctx
            ),
            Some(Leaf::Tuple { home: h, .. }) if h.name == "p"
        ));
        assert!(matches!(
            leaf_of_field_type(
                &field(field_type::Kind::Array(Box::new(ArrayType {
                    element: Some(Box::new(named("Point"))),
                    min: 0,
                    max: 4,
                }))),
                &home,
                &ctx
            ),
            Some(Leaf::Array { home: h, .. }) if h.name == "p"
        ));
        assert!(matches!(
            leaf_of_field_type(
                &field(field_type::Kind::Map(Box::new(MapType {
                    key: Some(Box::new(named("Point"))),
                    value: Some(Box::new(named("Point"))),
                    min: 0,
                    max: 4,
                }))),
                &home,
                &ctx
            ),
            Some(Leaf::Map { home: h, .. }) if h.name == "p"
        ));
        assert!(
            leaf_of_field_type(
                &field(field_type::Kind::Stream(StreamType {
                    element: Some(stream_type::Element::Named("Point".to_owned())),
                })),
                &home,
                &ctx
            )
            .is_none(),
            "a stream has absent sizes (driver §4 answer 10)"
        );
        assert!(
            leaf_of_field_type(
                &FieldType {
                    optional: false,
                    kind: None
                },
                &home,
                &ctx
            )
            .is_none(),
            "no kind"
        );
    }

    #[test]
    fn the_repr_c_column_is_absent() {
        // `Point` resolves, so a `ReprC` arm routed to a sizer would reach
        // that sizer: the proto3 sizer answers a bound, and the FlatBuffers
        // placeholder panics until Task 7 replaces it. Either way such a
        // routing fails this test.
        let package = package(
            "p",
            vec![decl(
                "Point",
                Some(decl::Kind::StructDef(one_field_struct(primitive(
                    PrimitiveType::Boolean,
                )))),
            )],
        );
        let others: [&Package; 0] = [];
        let ctx = Ctx::new(&package, &others);
        assert_eq!(size_state("Point", &ctx, Encoding::ReprC), None);
    }

    #[test]
    fn max_size_over_an_imported_declaration_is_rooted_at_its_package() {
        // `q.Thing { f: Inner }` names q's `Inner`, a one-boolean struct;
        // the root package declares a wider `Inner`. A `Packages` rooted at
        // the root would size q's `Thing` with the root's `Inner`.
        let root = package(
            "p",
            vec![decl(
                "Inner",
                Some(decl::Kind::StructDef(one_field_struct(FieldType {
                    optional: false,
                    kind: Some(field_type::Kind::InlineScalar(Box::new(string_def(Some(
                        100,
                    ))))),
                }))),
            )],
        );
        let imported = package(
            "q",
            vec![
                decl(
                    "Thing",
                    Some(decl::Kind::StructDef(one_field_struct(named("Inner")))),
                ),
                decl(
                    "Inner",
                    Some(decl::Kind::StructDef(one_field_struct(primitive(
                        PrimitiveType::Boolean,
                    )))),
                ),
            ],
        );
        let others = [&imported];
        let ctx = Ctx::new(&root, &others);

        let (thing, declaring) = ctx.resolve(&root, "q.Thing").expect("q.Thing resolves");
        assert_eq!(declaring.name, "q");
        let rooted = ctx.packages_for(declaring).expect("q is in scope");
        assert_eq!(rooted.package.name, "q");
        assert_eq!(rooted.others.len(), 1);
        assert_eq!(rooted.others[0].name, "p");

        let expected = max_size(
            Packages {
                package: &imported,
                others: &[&root],
            },
            thing,
        )
        .expect("a one-boolean struct inside a struct is bounded");
        assert_eq!(max_size(rooted, thing), Some(expected));
        assert_ne!(
            max_size(ctx.packages(), thing),
            Some(expected),
            "rooted at p, the bare `Inner` is p's wider struct"
        );

        let at_root = ctx.packages_for(&root).expect("the root is in scope");
        assert_eq!(at_root.package.name, "p");
        assert_eq!(at_root.others.len(), 1);
        assert!(
            ctx.packages_for(&package("r", vec![])).is_none(),
            "a package outside the scope"
        );
    }

    #[test]
    fn an_inline_composite_leaf_carries_the_package_of_its_field() {
        let root = package("p", vec![]);
        let imported = package(
            "q",
            vec![decl(
                "Point",
                Some(decl::Kind::StructDef(StructDef::default())),
            )],
        );
        let others = [&imported];
        let ctx = Ctx::new(&root, &others);
        let field = |kind: field_type::Kind| FieldType {
            optional: false,
            kind: Some(kind),
        };
        let tuple = field(field_type::Kind::Tuple(TupleType {
            fields: vec![TupleField {
                name: "x".to_owned(),
                r#type: Some(named("Point")),
            }],
        }));
        let array = field(field_type::Kind::Array(Box::new(ArrayType {
            element: Some(Box::new(named("Point"))),
            min: 0,
            max: 4,
        })));
        let map = field(field_type::Kind::Map(Box::new(MapType {
            key: Some(Box::new(named("Point"))),
            value: Some(Box::new(named("Point"))),
            min: 0,
            max: 4,
        })));

        assert!(matches!(
            leaf_of_field_type(&tuple, &imported, &ctx),
            Some(Leaf::Tuple { home, .. }) if home.name == "q"
        ));
        assert!(matches!(
            leaf_of_field_type(&array, &imported, &ctx),
            Some(Leaf::Array { home, .. }) if home.name == "q"
        ));
        assert!(matches!(
            leaf_of_field_type(&map, &imported, &ctx),
            Some(Leaf::Map { home, .. }) if home.name == "q"
        ));
    }
}

#[cfg(test)]
pub(crate) mod tests_support {
    use ridl_ir::v2::{
        ArrayType, Backing, ConstDef, Constraint, Decl, Field, FieldType, IntWidth, MapType,
        Package, PrimitiveType, StructDef, StructMember, TypeDef, UnionArm, UnionDef, backing,
        decl, field_type, struct_member, type_def,
    };

    fn scalar_def(
        primitive: PrimitiveType,
        width: Option<IntWidth>,
        constraint: Option<Constraint>,
    ) -> TypeDef {
        TypeDef {
            backing: Some(Backing {
                kind: Some(backing::Kind::Primitive(primitive as i32)),
            }),
            constraint,
            width: width.map(|w| type_def::Width::IntWidth(w as i32)),
            ..Default::default()
        }
    }

    pub(crate) fn inline(def: TypeDef) -> FieldType {
        FieldType {
            optional: false,
            kind: Some(field_type::Kind::InlineScalar(Box::new(def))),
        }
    }

    pub(crate) fn named(name: &str) -> FieldType {
        FieldType {
            optional: false,
            kind: Some(field_type::Kind::Named(name.to_owned())),
        }
    }

    pub(crate) fn array_u8_max4() -> FieldType {
        FieldType {
            optional: false,
            kind: Some(field_type::Kind::Array(Box::new(ArrayType {
                element: Some(Box::new(inline(scalar_def(
                    PrimitiveType::Integer,
                    Some(IntWidth::U8),
                    None,
                )))),
                min: 0,
                max: 4,
            }))),
        }
    }

    pub(crate) fn map_str8_u32_max2() -> FieldType {
        let key = inline(scalar_def(
            PrimitiveType::String,
            None,
            Some(Constraint {
                len_max: Some(8),
                ..Default::default()
            }),
        ));
        let value = inline(scalar_def(
            PrimitiveType::Integer,
            Some(IntWidth::U32),
            None,
        ));
        FieldType {
            optional: false,
            kind: Some(field_type::Kind::Map(Box::new(MapType {
                key: Some(Box::new(key)),
                value: Some(Box::new(value)),
                min: 0,
                max: 2,
            }))),
        }
    }

    /// `Coord = integer [-1000..1000]` (i16); `Point { x: Coord, y: Coord }`;
    /// `Vin = string [17..17] match /^[A-HJ-NPR-Z0-9]{17}$/`;
    /// `Bag { tag: Vin, flags: [u8; 0..4], index: {string [0..8]: u32; 0..2} }`;
    /// `Shape = Point | Coord`; `const LIMIT`.
    pub(crate) fn fixture() -> Package {
        let field = |name: &str, ordinal: u32, ty: FieldType| StructMember {
            member: Some(struct_member::Member::Field(Field {
                name: name.to_owned(),
                ordinal,
                r#type: Some(ty),
                ..Default::default()
            })),
        };
        let arm = |name: &str, ordinal: u32, type_ref: &str| UnionArm {
            name: name.to_owned(),
            ordinal,
            type_ref: type_ref.to_owned(),
            ..Default::default()
        };
        Package {
            name: "p".to_owned(),
            decls: vec![
                Decl {
                    name: "Coord".to_owned(),
                    kind: Some(decl::Kind::TypeDef(scalar_def(
                        PrimitiveType::Integer,
                        Some(IntWidth::I16),
                        None,
                    ))),
                    ..Default::default()
                },
                Decl {
                    name: "Point".to_owned(),
                    kind: Some(decl::Kind::StructDef(StructDef {
                        members: vec![field("x", 1, named("Coord")), field("y", 2, named("Coord"))],
                        fixed_layout: false,
                    })),
                    ..Default::default()
                },
                Decl {
                    name: "Vin".to_owned(),
                    kind: Some(decl::Kind::TypeDef(scalar_def(
                        PrimitiveType::String,
                        None,
                        Some(Constraint {
                            len_min: Some(17),
                            len_max: Some(17),
                            pattern: Some("/^[A-HJ-NPR-Z0-9]{17}$/".to_owned()),
                            ..Default::default()
                        }),
                    ))),
                    ..Default::default()
                },
                Decl {
                    name: "Bag".to_owned(),
                    kind: Some(decl::Kind::StructDef(StructDef {
                        members: vec![
                            field("tag", 1, named("Vin")),
                            field("flags", 2, array_u8_max4()),
                            field("index", 3, map_str8_u32_max2()),
                        ],
                        fixed_layout: false,
                    })),
                    ..Default::default()
                },
                Decl {
                    name: "Shape".to_owned(),
                    kind: Some(decl::Kind::UnionDef(UnionDef {
                        arms: vec![arm("a", 1, "Point"), arm("b", 2, "Coord")],
                        ..Default::default()
                    })),
                    ..Default::default()
                },
                Decl {
                    name: "LIMIT".to_owned(),
                    kind: Some(decl::Kind::ConstDef(ConstDef {
                        type_ref: Some("Coord".to_owned()),
                        value: "7".to_owned(),
                        regex: None,
                    })),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }
    }
}
