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
}

impl Encoding for Proto3 {
    const NAME: &'static str = "proto3";
}

impl Encoding for ReprC {
    const NAME: &'static str = "repr-c";
}

#[cfg(test)]
mod tests {
    use super::{sealed, Encoding, FlatBuffers, Proto3, ReprC};

    /// A fourth encoding, for this test only. It implements every required
    /// item of `Encoding`, so a required item added to the trait fails to
    /// compile this module.
    struct Fourth;

    impl sealed::Sealed for Fourth {}

    impl Encoding for Fourth {
        const NAME: &'static str = "fourth";
    }

    #[test]
    fn name_is_the_only_required_item() {
        assert_eq!(Fourth::NAME, "fourth");
    }

    #[test]
    fn each_name_is_the_cargo_feature_name() {
        assert_eq!(FlatBuffers::NAME, "flatbuffers");
        assert_eq!(Proto3::NAME, "proto3");
        assert_eq!(ReprC::NAME, "repr-c");
    }
}
