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
//! `flatbuffers`, and ADR-0017's projection in `proto3`. A request of zero or
//! several parameters, an inline `T | E` reply and a stream payload (§4
//! answer 10) are absent until a record defines their encoding.

pub(crate) mod flatbuffers;
pub(crate) mod proto3;

use ridl_ir::projection::flatbuffers::Packages;
use ridl_ir::v2::{
    ArrayType, Constraint, Decl, FieldType, FloatWidth, IntWidth, MapType, Package, Param,
    PrimitiveType, ReturnType, StructDef, TupleType, TypeDef, UnionDef, backing, decl, field_type,
    return_type, type_def,
};

use crate::{Encoding, UnboundedCause};

/// Name resolution over a package and the packages it imports, and the
/// projection's view of the same scope.
pub struct Ctx<'a> {
    packages: Packages<'a>,
}

impl<'a> Ctx<'a> {
    pub fn new(package: &'a Package, others: &'a [&'a Package]) -> Self {
        Self {
            packages: Packages { package, others },
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

    /// The projection's view of the scope, for `max_size`. Its `package` is
    /// the package the catalog is built for: the home of a payload's type
    /// name.
    pub fn packages(&self) -> Packages<'a> {
        self.packages
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
pub fn size_state(type_name: &str, ctx: &Ctx<'_>, encoding: Encoding) -> Option<SizeState> {
    match encoding {
        Encoding::Proto3 => proto3::state(type_name, ctx),
        Encoding::FlatBuffers => flatbuffers::state(type_name, ctx),
        Encoding::ReprC => None,
    }
}

/// The byte capacity of a `string`: its bound in scalar values times four
/// (release-scope design §3.11). `None` when the constraint carries no
/// `len_max`: the FlatBuffers projection charges nothing for a string without
/// a bound, and this function agrees with it on both the multiplier and the
/// absence (`string_max_bytes_agrees_with_the_flatbuffers_projection`). The
/// compiler writes typl §4.4's `[0..256]` default into `len_max`, so a
/// compiled package never reaches the `None`. No `match` narrowing (driver §4
/// answer 7; #665).
pub fn string_max_bytes(constraint: Option<&Constraint>) -> Option<u64> {
    constraint?.len_max?.checked_mul(4)
}

// The leaf model below is `pub(crate)` for the proto3 sizer of Task 6
// (driftsys/ridl#380). Until that task lands, its only callers are the tests,
// so each item carries `allow(dead_code)`; Task 6 removes the attributes.

/// A scalar as the proto3 projection spells it (`proto_scalar` in the proto
/// backend).
#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
pub(crate) enum Scalar {
    Bool,
    Int(IntWidth),
    Float(FloatWidth),
}

impl Scalar {
    /// The largest proto3 encoding of the value, tag excluded: a varint for
    /// the integers (`uint32`/`sint32` at most 5 bytes, `uint64`/`sint64`/
    /// `int64` at most 10), fixed for the floats. The proto backend emits the
    /// signed widths as `sint32`/`sint64` and an unspecified width as
    /// `int64`, so no width reaches the 10-byte encoding of a negative
    /// `int32`.
    #[allow(dead_code)]
    pub(crate) fn proto_max(self) -> u64 {
        match self {
            Self::Bool => 1,
            Self::Int(
                IntWidth::U8
                | IntWidth::U16
                | IntWidth::U32
                | IntWidth::I8
                | IntWidth::I16
                | IntWidth::I32,
            ) => 5,
            Self::Int(IntWidth::U64 | IntWidth::I64 | IntWidth::Unspecified) => 10,
            Self::Float(FloatWidth::F32) => 4,
            Self::Float(FloatWidth::F64 | FloatWidth::Unspecified) => 8,
        }
    }
}

/// A type resolved to what the projections size. A composite carries `home`,
/// the package a bare name inside it resolves against: the declaring package
/// of a named struct or union, the package of the field for an inline tuple,
/// array or map.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
pub(crate) enum Leaf<'a> {
    Scalar(Scalar),
    /// A string or bytes value: its maximum byte length. The leaf model
    /// counts in `u64`, as the projection does; the descriptor's sizes are
    /// `u32` (driver §4 answer 5), and Tasks 6 and 7 narrow the value when
    /// they write a row.
    Blob(u64),
    /// An enum: whether a member is negative, and the largest magnitude.
    Enum {
        negative: bool,
        max_magnitude: u64,
    },
    EnumSet(IntWidth),
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
#[allow(dead_code)]
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
#[allow(dead_code)]
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
        decl::Kind::EnumDef(def) => Some(Leaf::Enum {
            negative: def.values.iter().any(|v| v.value < 0),
            max_magnitude: def
                .values
                .iter()
                .map(|v| v.value.unsigned_abs())
                .max()
                .unwrap_or(0),
        }),
        decl::Kind::EnumSetDef(def) => Some(Leaf::EnumSet(
            IntWidth::try_from(def.width).unwrap_or(IntWidth::Unspecified),
        )),
        decl::Kind::ConstDef(_)
        | decl::Kind::SignalDef(_)
        | decl::Kind::EventDef(_)
        | decl::Kind::CommandDef(_)
        | decl::Kind::QueryDef(_)
        | decl::Kind::FixedDef(_)
        | decl::Kind::ReservedSlot(_) => None,
    }
}

/// A bare primitive at a field position. A bare `string` or `bytes` carries
/// no length bound, and the FlatBuffers projection charges nothing without
/// one, so neither has a leaf. The compiler keeps both out of a field
/// position (TYPL-208); a map key gets typl §4.4–§4.5's `[0..256]` default as
/// an inline scalar, which `leaf_of_type_def` sizes.
#[allow(dead_code)]
pub(crate) fn leaf_of_primitive<'a>(primitive: i32) -> Option<Leaf<'a>> {
    match PrimitiveType::try_from(primitive).ok()? {
        PrimitiveType::Boolean => Some(Leaf::Scalar(Scalar::Bool)),
        PrimitiveType::Integer => Some(Leaf::Scalar(Scalar::Int(IntWidth::Unspecified))),
        PrimitiveType::Float => Some(Leaf::Scalar(Scalar::Float(FloatWidth::Unspecified))),
        PrimitiveType::String | PrimitiveType::Bytes | PrimitiveType::Unspecified => None,
    }
}

fn leaf_of_type_def<'a>(def: &'a TypeDef) -> Option<Leaf<'a>> {
    let int_width = match def.width {
        Some(type_def::Width::IntWidth(w)) => {
            IntWidth::try_from(w).unwrap_or(IntWidth::Unspecified)
        }
        _ => IntWidth::Unspecified,
    };
    let float_width = match def.width {
        Some(type_def::Width::FloatWidth(w)) => {
            FloatWidth::try_from(w).unwrap_or(FloatWidth::Unspecified)
        }
        _ => FloatWidth::Unspecified,
    };
    match def.backing.as_ref()?.kind.as_ref()? {
        backing::Kind::Primitive(primitive) => match PrimitiveType::try_from(*primitive).ok()? {
            PrimitiveType::Boolean => Some(Leaf::Scalar(Scalar::Bool)),
            PrimitiveType::Integer => Some(Leaf::Scalar(Scalar::Int(int_width))),
            PrimitiveType::Float => Some(Leaf::Scalar(Scalar::Float(float_width))),
            PrimitiveType::String => string_max_bytes(def.constraint.as_ref()).map(Leaf::Blob),
            // Like a string, a `bytes` without a `len_max` is unsizable, as
            // the FlatBuffers projection answers for the same type.
            PrimitiveType::Bytes => def.constraint.as_ref()?.len_max.map(Leaf::Blob),
            PrimitiveType::Unspecified => None,
        },
        // A unit-backed scalar carries a derived width (typl §5.1), and the
        // backends project it by that width: integer when an integer width is
        // set, float when a float width is set. Without a width the proto
        // backend falls back to `string`, which has no bound, so there is no
        // leaf.
        backing::Kind::Unit(_) => match def.width {
            Some(type_def::Width::IntWidth(_)) => Some(Leaf::Scalar(Scalar::Int(int_width))),
            Some(type_def::Width::FloatWidth(_)) => Some(Leaf::Scalar(Scalar::Float(float_width))),
            None => None,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ridl_ir::projection::flatbuffers::max_size;
    use ridl_ir::v2::{
        Backing, CommandDef, ConstDef, EnumDef, EnumSetDef, EnumValue, EventDef, FallibleType,
        Field, FixedDef, Package, Param, PrimitiveType, QueryDef, Reserved, ReturnType, SignalDef,
        StreamType, StructMember, TupleField, field_type, return_type, stream_type, struct_member,
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
        // Each shape the projection answers `None` for has no leaf here, so
        // the proto3 column cannot claim a bound the FlatBuffers column
        // refuses for the same type.
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
            Some(Leaf::Scalar(s)) => assert_eq!(s.proto_max(), 10),
            other => panic!("integer is a scalar leaf, got {other:?}"),
        }
        // A bare `string` or `bytes` has no bound, so no leaf (the projection
        // answers `None` for the same field).
        assert!(leaf_of_primitive(PrimitiveType::String as i32).is_none());
        assert!(leaf_of_primitive(PrimitiveType::Bytes as i32).is_none());
    }

    #[test]
    fn every_primitive_is_a_leaf_or_unsizable() {
        assert!(matches!(
            leaf_of_primitive(PrimitiveType::Boolean as i32),
            Some(Leaf::Scalar(Scalar::Bool))
        ));
        assert!(matches!(
            leaf_of_primitive(PrimitiveType::Integer as i32),
            Some(Leaf::Scalar(Scalar::Int(IntWidth::Unspecified)))
        ));
        assert!(matches!(
            leaf_of_primitive(PrimitiveType::Float as i32),
            Some(Leaf::Scalar(Scalar::Float(FloatWidth::Unspecified)))
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
    fn proto_max_is_the_largest_encoding_the_proto_backend_emits() {
        // `proto_scalar` in `ridl-backend-proto`: the unsigned widths up to
        // 32 bits are `uint32`, `U64` is `uint64`, the signed widths up to 32
        // bits are `sint32`, `I64` is `sint64`, an unspecified width is
        // `int64`; `F32` is `float`, anything else `double`.
        assert_eq!(Scalar::Bool.proto_max(), 1);
        for width in [IntWidth::U8, IntWidth::U16, IntWidth::U32] {
            assert_eq!(Scalar::Int(width).proto_max(), 5, "{width:?} is a uint32");
        }
        for width in [IntWidth::I8, IntWidth::I16, IntWidth::I32] {
            assert_eq!(Scalar::Int(width).proto_max(), 5, "{width:?} is a sint32");
        }
        assert_eq!(Scalar::Int(IntWidth::U64).proto_max(), 10, "uint64");
        assert_eq!(Scalar::Int(IntWidth::I64).proto_max(), 10, "sint64");
        assert_eq!(Scalar::Int(IntWidth::Unspecified).proto_max(), 10, "int64");
        assert_eq!(Scalar::Float(FloatWidth::F32).proto_max(), 4);
        assert_eq!(Scalar::Float(FloatWidth::F64).proto_max(), 8);
        assert_eq!(Scalar::Float(FloatWidth::Unspecified).proto_max(), 8);
    }

    #[test]
    fn a_type_def_leaf_follows_its_backing_and_width() {
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
            Some(Leaf::Scalar(Scalar::Int(IntWidth::I16)))
        ));
        assert!(matches!(
            leaf(&type_def(prim(PrimitiveType::Integer), None, None)),
            Some(Leaf::Scalar(Scalar::Int(IntWidth::Unspecified)))
        ));
        assert!(matches!(
            leaf(&type_def(
                prim(PrimitiveType::Float),
                float_width(FloatWidth::F32),
                None
            )),
            Some(Leaf::Scalar(Scalar::Float(FloatWidth::F32)))
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

        let unit = || backing::Kind::Unit("km/h".to_owned());
        assert!(matches!(
            leaf(&type_def(unit(), int_width(IntWidth::U16), None)),
            Some(Leaf::Scalar(Scalar::Int(IntWidth::U16)))
        ));
        assert!(matches!(
            leaf(&type_def(unit(), float_width(FloatWidth::F64), None)),
            Some(Leaf::Scalar(Scalar::Float(FloatWidth::F64)))
        ));
        assert!(
            leaf(&type_def(unit(), None, None)).is_none(),
            "the proto backend emits `string` for a unit without a width"
        );
        assert!(leaf(&TypeDef::default()).is_none(), "no backing");
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
            Some(Leaf::Enum {
                negative: true,
                max_magnitude: 7
            })
        ));
        assert!(matches!(
            leaf_of_name("Mode", &home, &ctx),
            Some(Leaf::Enum {
                negative: false,
                max_magnitude: 5
            })
        ));
        assert!(matches!(
            leaf_of_name("Flags", &home, &ctx),
            Some(Leaf::EnumSet(IntWidth::U8))
        ));
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
            vec![decl(
                "Inner",
                Some(decl::Kind::StructDef(StructDef::default())),
            )],
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
                Some(Leaf::Enum {
                    negative: false,
                    max_magnitude: 1
                })
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
        let package = package("p", vec![]);
        let others: [&Package; 0] = [];
        let ctx = Ctx::new(&package, &others);
        assert_eq!(size_state("Point", &ctx, Encoding::ReprC), None);
    }
}
