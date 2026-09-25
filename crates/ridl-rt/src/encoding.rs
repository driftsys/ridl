//! The payload encodings (ADR-0020 decisions 1 and 5).
//!
//! An encoding is a marker type. A payload type implements `Payload<E>` once
//! for each encoding its package was built with, so one type can carry several
//! codecs. The set of encodings is closed: [`Encoding`] is sealed, so a new
//! encoding is a decision recorded in an ADR and not an `impl` in another
//! crate.
//!
//! The marker types do not depend on a cargo feature. A runtime can name every
//! encoding without linking a codec.

use crate::contract::EncodedSizes;

mod sealed {
    pub trait Sealed {}
}

/// A payload encoding. Only the three marker types of this module implement
/// it.
///
/// A crate outside `ridl-rt` names an encoding through this trait:
///
/// ```
/// fn name<E: ridl_rt::encoding::Encoding>() -> &'static str {
///     E::NAME
/// }
/// assert_eq!(name::<ridl_rt::encoding::ReprC>(), "repr-c");
/// ```
///
/// and cannot add an encoding:
///
/// ```compile_fail,E0277
/// struct Json;
///
/// impl ridl_rt::encoding::Encoding for Json {
///     const NAME: &'static str = "json";
///     fn max_size(sizes: &ridl_rt::contract::EncodedSizes) -> Option<u32> {
///         sizes.proto3
///     }
/// }
/// ```
///
/// and cannot implement the sealing trait, because its module is private:
///
/// ```compile_fail,E0603
/// struct Json;
///
/// impl ridl_rt::encoding::sealed::Sealed for Json {}
/// ```
pub trait Encoding: sealed::Sealed + 'static {
    /// The encoding's name, spelled as its cargo feature is spelled.
    const NAME: &'static str;

    /// This encoding's field of `sizes`: a payload's largest encoded size in
    /// this encoding, or `None` when the toolchain cannot size it
    /// ([`EncodedSizes`]).
    ///
    /// A required item, so an encoding added without saying which field it
    /// reads does not compile.
    fn max_size(sizes: &EncodedSizes) -> Option<u32>;
}

/// FlatBuffers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FlatBuffers;

/// proto3.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Proto3;

/// `repr(C)`, the fixed-layout encoding of ADR-0020 decision 1.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReprC;

impl sealed::Sealed for FlatBuffers {}
impl sealed::Sealed for Proto3 {}
impl sealed::Sealed for ReprC {}

impl Encoding for FlatBuffers {
    const NAME: &'static str = "flatbuffers";

    fn max_size(sizes: &EncodedSizes) -> Option<u32> {
        sizes.flatbuffers
    }
}

impl Encoding for Proto3 {
    const NAME: &'static str = "proto3";

    fn max_size(sizes: &EncodedSizes) -> Option<u32> {
        sizes.proto3
    }
}

impl Encoding for ReprC {
    const NAME: &'static str = "repr-c";

    fn max_size(sizes: &EncodedSizes) -> Option<u32> {
        sizes.repr_c
    }
}

#[cfg(test)]
mod tests {
    use super::{sealed, Encoding, FlatBuffers, Proto3, ReprC};
    use crate::contract::EncodedSizes;

    /// A fourth encoding, for this test only. It implements every required
    /// item of `Encoding`, so a required item added to the trait fails to
    /// compile this module.
    struct Fourth;

    impl sealed::Sealed for Fourth {}

    impl Encoding for Fourth {
        const NAME: &'static str = "fourth";

        fn max_size(_: &EncodedSizes) -> Option<u32> {
            None
        }
    }

    const SIZES: EncodedSizes = EncodedSizes {
        proto3: Some(1),
        flatbuffers: Some(2),
        repr_c: Some(3),
    };

    #[test]
    fn name_and_max_size_are_the_only_required_items() {
        assert_eq!(Fourth::NAME, "fourth");
        assert_eq!(Fourth::max_size(&SIZES), None);
    }

    /// `EncodedSizes` holds one field per core encoding, and each encoding
    /// reads its own.
    #[test]
    fn each_encoding_reads_its_own_size_field() {
        assert_eq!(Proto3::max_size(&SIZES), Some(1));
        assert_eq!(FlatBuffers::max_size(&SIZES), Some(2));
        assert_eq!(ReprC::max_size(&SIZES), Some(3));
    }

    #[test]
    fn each_name_is_the_cargo_feature_name() {
        assert_eq!(FlatBuffers::NAME, "flatbuffers");
        assert_eq!(Proto3::NAME, "proto3");
        assert_eq!(ReprC::NAME, "repr-c");
    }
}
