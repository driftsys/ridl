//! The proto3 projection facts, as ADR-0017 fixes them.
//!
//! Two readers have to agree on the scalar a typl type projects to: the
//! `.proto` schema `ridl-backend-proto` writes, and the proto3 size bound
//! [`crate::projection::size`] derives. They share no emission code, so what
//! they share is this table: one function from a width, or from a backing
//! without a width, to the proto3 scalar, and the largest encoding of one
//! value of that scalar.
//!
//! Nothing here writes a line of schema text; the spelling the backend emits
//! is [`Scalar::as_str`], and the backend is the only caller that emits it.
//!
//! The two field-number limits proto3 fixes live here for the same reason:
//! the backend refuses a field number outside them (`check_field_number`),
//! and the sizer answers no bound for the same field.

use crate::v2;

/// The field numbers protobuf reserves for its own use (the proto3 language
/// guide, "Assigning field numbers").
pub const PROTO_RESERVED: std::ops::RangeInclusive<u32> = 19_000..=19_999;
/// The largest field number proto3 admits: 2^29 - 1.
pub const PROTO_MAX_FIELD_NUMBER: u32 = 536_870_911;

/// A proto3 scalar type, as the projection spells it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scalar {
    Bool,
    Int64,
    Uint32,
    Uint64,
    Sint32,
    Sint64,
    Float,
    Double,
    String,
    Bytes,
}

impl Scalar {
    /// Every scalar, in declaration order.
    pub const ALL: [Scalar; 10] = [
        Scalar::Bool,
        Scalar::Int64,
        Scalar::Uint32,
        Scalar::Uint64,
        Scalar::Sint32,
        Scalar::Sint64,
        Scalar::Float,
        Scalar::Double,
        Scalar::String,
        Scalar::Bytes,
    ];

    /// Whether proto3 admits the scalar as a map key: any integral or string
    /// type, never a floating-point type or `bytes` (the proto3 language
    /// guide, "Maps"). typl admits a broader set at a map key position
    /// (typl §12.2, TYPL-209), so a key that projects to a scalar outside
    /// this set is refused by the proto backend (`map_key_text`) and has no
    /// proto3 size.
    #[must_use]
    pub fn admitted_as_map_key(self) -> bool {
        match self {
            Self::Bool
            | Self::Int64
            | Self::Uint32
            | Self::Uint64
            | Self::Sint32
            | Self::Sint64
            | Self::String => true,
            Self::Float | Self::Double | Self::Bytes => false,
        }
    }

    /// The scalar's spelling in a `.proto` file.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Bool => "bool",
            Self::Int64 => "int64",
            Self::Uint32 => "uint32",
            Self::Uint64 => "uint64",
            Self::Sint32 => "sint32",
            Self::Sint64 => "sint64",
            Self::Float => "float",
            Self::Double => "double",
            Self::String => "string",
            Self::Bytes => "bytes",
        }
    }

    /// The largest encoding of one value of this scalar, its tag excluded: a
    /// varint for the integers (`uint32`/`sint32` at most 5 bytes;
    /// `int64`/`uint64`/`sint64` at most 10), fixed for the floats, one byte
    /// for `bool`. `None` for `string` and `bytes`, whose length is the
    /// value's own and is bounded only by a constraint the type carries.
    #[must_use]
    pub fn max_encoded_len(self) -> Option<u64> {
        match self {
            Self::Bool => Some(1),
            Self::Uint32 | Self::Sint32 => Some(5),
            Self::Int64 | Self::Uint64 | Self::Sint64 => Some(10),
            Self::Float => Some(4),
            Self::Double => Some(8),
            Self::String | Self::Bytes => None,
        }
    }
}

/// The proto3 scalar for a named or inline scalar (typl Appendix D). proto3
/// has no `uint8`/`uint16` — varint keeps small values small — so both widen
/// to `uint32`. A signed width means the declared range contains negatives,
/// and such a range takes `sint32`/`sint64`, because plain `int32` varint
/// costs 10 bytes for every negative value (ADR-0013 decision 4). A
/// quantized float keeps its native form: the scaled-integer encoding of typl
/// §4.3 belongs to CAN/DBC and to SOME/IP per deployment, and a wire backend
/// must not apply it unasked.
///
/// Without a width, a boolean, string or bytes backing projects by the
/// backing; every other case — an integer or float backing with no width, a
/// unit backing with no derived width, no backing at all — projects to
/// `string`. A unit backing implies the float primitive (typl §5.1), so its
/// width is derived and the `string` fallback does not reach it from a
/// compiled package.
#[must_use]
pub fn scalar(td: &v2::TypeDef) -> Scalar {
    match &td.width {
        Some(v2::type_def::Width::IntWidth(width)) => match v2::IntWidth::try_from(*width) {
            Ok(v2::IntWidth::U8 | v2::IntWidth::U16 | v2::IntWidth::U32) => Scalar::Uint32,
            Ok(v2::IntWidth::U64) => Scalar::Uint64,
            Ok(v2::IntWidth::I8 | v2::IntWidth::I16 | v2::IntWidth::I32) => Scalar::Sint32,
            Ok(v2::IntWidth::I64) => Scalar::Sint64,
            _ => Scalar::Int64,
        },
        Some(v2::type_def::Width::FloatWidth(width)) => match v2::FloatWidth::try_from(*width) {
            Ok(v2::FloatWidth::F32) => Scalar::Float,
            _ => Scalar::Double,
        },
        None => match td
            .backing
            .as_ref()
            .and_then(|backing| backing.kind.as_ref())
        {
            Some(v2::backing::Kind::Primitive(primitive)) => {
                match v2::PrimitiveType::try_from(*primitive) {
                    Ok(v2::PrimitiveType::Boolean) => Scalar::Bool,
                    Ok(v2::PrimitiveType::Bytes) => Scalar::Bytes,
                    _ => Scalar::String,
                }
            }
            _ => Scalar::String,
        },
    }
}

/// The proto3 scalar for an enum set of integer width `width`: the scalar of
/// that width alone, the way the proto backend resolves an enum set at each
/// use site (`enum_set_field_type`). A width tag the IR does not define is
/// `int64`, as for any other integer width it does not define.
#[must_use]
pub fn enum_set_scalar(width: i32) -> Scalar {
    scalar(&v2::TypeDef {
        width: Some(v2::type_def::Width::IntWidth(width)),
        ..Default::default()
    })
}

/// The proto3 scalar for a bare primitive at a field position: an integer
/// with no declared width is `int64`, a float `double`; a tag the IR does
/// not define is `string`.
#[must_use]
pub fn primitive(primitive: i32) -> Scalar {
    match v2::PrimitiveType::try_from(primitive) {
        Ok(v2::PrimitiveType::Boolean) => Scalar::Bool,
        Ok(v2::PrimitiveType::Integer) => Scalar::Int64,
        Ok(v2::PrimitiveType::Float) => Scalar::Double,
        Ok(v2::PrimitiveType::Bytes) => Scalar::Bytes,
        _ => Scalar::String,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::v2::{Backing, FloatWidth, IntWidth, PrimitiveType, TypeDef, backing, type_def};

    fn with_int_width(width: i32) -> TypeDef {
        TypeDef {
            width: Some(type_def::Width::IntWidth(width)),
            ..Default::default()
        }
    }

    fn with_float_width(width: i32) -> TypeDef {
        TypeDef {
            width: Some(type_def::Width::FloatWidth(width)),
            ..Default::default()
        }
    }

    fn with_backing(kind: backing::Kind) -> TypeDef {
        TypeDef {
            backing: Some(Backing { kind: Some(kind) }),
            ..Default::default()
        }
    }

    fn prim(primitive: PrimitiveType) -> backing::Kind {
        backing::Kind::Primitive(primitive as i32)
    }

    #[test]
    fn the_width_table_is_the_one_the_proto_backend_emits() {
        for width in [IntWidth::U8, IntWidth::U16, IntWidth::U32] {
            assert_eq!(scalar(&with_int_width(width as i32)).as_str(), "uint32");
        }
        assert_eq!(
            scalar(&with_int_width(IntWidth::U64 as i32)).as_str(),
            "uint64"
        );
        for width in [IntWidth::I8, IntWidth::I16, IntWidth::I32] {
            assert_eq!(scalar(&with_int_width(width as i32)).as_str(), "sint32");
        }
        assert_eq!(
            scalar(&with_int_width(IntWidth::I64 as i32)).as_str(),
            "sint64"
        );
        assert_eq!(
            scalar(&with_int_width(IntWidth::Unspecified as i32)).as_str(),
            "int64"
        );
        assert_eq!(
            scalar(&with_int_width(99)).as_str(),
            "int64",
            "an integer width tag the IR does not define"
        );
        assert_eq!(
            scalar(&with_float_width(FloatWidth::F32 as i32)).as_str(),
            "float"
        );
        assert_eq!(
            scalar(&with_float_width(FloatWidth::F64 as i32)).as_str(),
            "double"
        );
        assert_eq!(
            scalar(&with_float_width(FloatWidth::Unspecified as i32)).as_str(),
            "double"
        );
        assert_eq!(
            scalar(&with_float_width(99)).as_str(),
            "double",
            "a float width tag the IR does not define"
        );
    }

    #[test]
    fn without_a_width_the_backing_decides() {
        assert_eq!(
            scalar(&with_backing(prim(PrimitiveType::Boolean))).as_str(),
            "bool"
        );
        assert_eq!(
            scalar(&with_backing(prim(PrimitiveType::Bytes))).as_str(),
            "bytes"
        );
        assert_eq!(
            scalar(&with_backing(prim(PrimitiveType::String))).as_str(),
            "string"
        );
        assert_eq!(
            scalar(&with_backing(prim(PrimitiveType::Integer))).as_str(),
            "string",
            "an integer backing without a width"
        );
        assert_eq!(
            scalar(&with_backing(prim(PrimitiveType::Float))).as_str(),
            "string",
            "a float backing without a width"
        );
        assert_eq!(
            scalar(&with_backing(prim(PrimitiveType::Unspecified))).as_str(),
            "string"
        );
        assert_eq!(
            scalar(&with_backing(backing::Kind::Unit("km/h".to_owned()))).as_str(),
            "string",
            "a unit backing without a derived width"
        );
        assert_eq!(scalar(&TypeDef::default()).as_str(), "string", "no backing");
        // The width wins over a backing that disagrees with it.
        assert_eq!(
            scalar(&TypeDef {
                width: Some(type_def::Width::IntWidth(IntWidth::U16 as i32)),
                ..with_backing(prim(PrimitiveType::Boolean))
            })
            .as_str(),
            "uint32"
        );
    }

    #[test]
    fn a_bare_primitive_has_its_own_table() {
        assert_eq!(primitive(PrimitiveType::Boolean as i32).as_str(), "bool");
        assert_eq!(primitive(PrimitiveType::Integer as i32).as_str(), "int64");
        assert_eq!(primitive(PrimitiveType::Float as i32).as_str(), "double");
        assert_eq!(primitive(PrimitiveType::Bytes as i32).as_str(), "bytes");
        assert_eq!(primitive(PrimitiveType::String as i32).as_str(), "string");
        assert_eq!(
            primitive(PrimitiveType::Unspecified as i32).as_str(),
            "string"
        );
        assert_eq!(
            primitive(99).as_str(),
            "string",
            "a primitive tag the IR does not define"
        );
    }

    #[test]
    fn an_enum_set_is_the_scalar_of_its_width() {
        assert_eq!(enum_set_scalar(IntWidth::U8 as i32), Scalar::Uint32);
        assert_eq!(enum_set_scalar(IntWidth::U64 as i32), Scalar::Uint64);
        assert_eq!(enum_set_scalar(IntWidth::I16 as i32), Scalar::Sint32);
        assert_eq!(enum_set_scalar(IntWidth::Unspecified as i32), Scalar::Int64);
        assert_eq!(enum_set_scalar(99), Scalar::Int64);
    }

    #[test]
    fn the_map_key_set_is_the_integral_and_string_scalars() {
        let admitted: Vec<&str> = Scalar::ALL
            .iter()
            .filter(|s| s.admitted_as_map_key())
            .map(|s| s.as_str())
            .collect();
        assert_eq!(
            admitted,
            [
                "bool", "int64", "uint32", "uint64", "sint32", "sint64", "string"
            ]
        );
        assert_eq!(Scalar::ALL.len(), 10, "every scalar is listed");
    }

    #[test]
    fn the_largest_encoding_of_each_scalar() {
        assert_eq!(Scalar::Bool.max_encoded_len(), Some(1));
        assert_eq!(Scalar::Uint32.max_encoded_len(), Some(5));
        assert_eq!(Scalar::Sint32.max_encoded_len(), Some(5));
        assert_eq!(Scalar::Int64.max_encoded_len(), Some(10));
        assert_eq!(Scalar::Uint64.max_encoded_len(), Some(10));
        assert_eq!(Scalar::Sint64.max_encoded_len(), Some(10));
        assert_eq!(Scalar::Float.max_encoded_len(), Some(4));
        assert_eq!(Scalar::Double.max_encoded_len(), Some(8));
        assert_eq!(Scalar::String.max_encoded_len(), None);
        assert_eq!(Scalar::Bytes.max_encoded_len(), None);
    }
}
