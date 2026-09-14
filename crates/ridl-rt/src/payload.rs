//! Encoding, verifying and decoding a payload.
//!
//! A generated payload type implements [`Payload<E>`] once for each encoding.
//! Its `verify` checks the structure of the bytes and the typl constraints of
//! the value in one pass. Its `decode` takes a [`Ref`], and only
//! [`Ref::verify`] and [`Ref::encode`] build a `Ref`. So a value is decoded
//! only from bytes that were checked, or that its own encoder wrote.

use core::marker::PhantomData;

use crate::encoding::Encoding;

/// A type that can be carried as a payload in the encoding `E`.
///
/// Generated code implements this trait and never calls its methods directly:
/// it calls [`Ref::verify`], [`Ref::encode`] and [`Ref::decode`].
pub trait Payload<E: Encoding>: Sized {
    /// The largest encoded size of any legal value, in bytes.
    const MAX_SIZE: usize;

    /// What a successful check or encode leaves behind: the bytes, an
    /// accessor over the bytes, or a parsed value.
    type View<'a>;

    /// Writes `self` into the front of `out`.
    fn encode<'o>(&self, out: &'o mut [u8]) -> Result<Encoded<'o, Self::View<'o>>, EncodeError>;

    /// Checks the structure of `buf` and the typl constraints of the value it
    /// holds, in one pass.
    fn verify(buf: &[u8]) -> Result<Self::View<'_>, VerifyError>;

    /// Builds the value from a proof. It cannot fail, because the proof shows
    /// that the bytes were checked or encoded.
    fn decode(r: Ref<'_, Self, E>) -> Self;
}

/// The result of [`Payload::encode`]: the bytes written and their view. It is
/// data, not a proof.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Encoded<'a, V> {
    /// The encoded bytes, a prefix of the output buffer.
    pub bytes: &'a [u8],
    /// The view of those bytes.
    pub view: V,
}

/// A proof that some bytes passed `T::verify` or were written by `T::encode`.
///
/// The fields are private and [`Ref::verify`] and [`Ref::encode`] are the only
/// constructors. A crate outside `ridl-rt` builds a `Ref` by checking:
///
/// ```
/// use ridl_rt::encoding::ReprC;
/// use ridl_rt::payload::{Payload, Ref, VerifyError};
///
/// fn check<'a, T>(bytes: &'a [u8]) -> Result<Ref<'a, T, ReprC>, VerifyError>
/// where
///     T: Payload<ReprC, View<'a> = &'a [u8]>,
/// {
///     Ref::verify(bytes)
/// }
/// ```
///
/// and cannot build one from its fields:
///
/// ```compile_fail,E0451
/// use core::marker::PhantomData;
/// use ridl_rt::encoding::ReprC;
/// use ridl_rt::payload::{Payload, Ref};
///
/// fn forge<'a, T>(bytes: &'a [u8]) -> Ref<'a, T, ReprC>
/// where
///     T: Payload<ReprC, View<'a> = &'a [u8]>,
/// {
///     Ref { bytes, view: bytes, _e: PhantomData }
/// }
/// ```
pub struct Ref<'a, T: Payload<E>, E: Encoding> {
    bytes: &'a [u8],
    view: T::View<'a>,
    _e: PhantomData<E>,
}

impl<'a, T: Payload<E>, E: Encoding> Ref<'a, T, E> {
    /// Checks `buf` with `T::verify` and returns the proof.
    pub fn verify(buf: &'a [u8]) -> Result<Self, VerifyError> {
        let view = T::verify(buf)?;
        Ok(Ref {
            bytes: buf,
            view,
            _e: PhantomData,
        })
    }

    /// Encodes `value` into `out` with `T::encode` and returns the proof.
    pub fn encode(value: &T, out: &'a mut [u8]) -> Result<Self, EncodeError> {
        let Encoded { bytes, view } = value.encode(out)?;
        Ok(Ref {
            bytes,
            view,
            _e: PhantomData,
        })
    }

    /// The checked or encoded bytes.
    pub fn bytes(&self) -> &'a [u8] {
        self.bytes
    }

    /// Lends the view.
    pub fn view(&self) -> &T::View<'a> {
        &self.view
    }

    /// Moves the view out.
    pub fn into_view(self) -> T::View<'a> {
        self.view
    }

    /// Builds the value with `T::decode`.
    pub fn decode(self) -> T {
        T::decode(self)
    }
}

/// An encode that failed.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EncodeError {
    /// The output buffer is shorter than the encoding.
    Capacity {
        /// The bytes the encoding needs.
        needed: usize,
        /// The bytes the output buffer has.
        available: usize,
    },
}

/// A check that failed.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VerifyError {
    /// The bytes are not a well-formed encoding: a serialization failure, ridl
    /// §10.3.
    Structure(Malformed),
    /// The bytes are well formed and hold a value that breaks a typl
    /// constraint: `INVALID_VALUE`, ridl §10.2.
    Contract(Violation),
}

/// How the bytes of an encoding are malformed.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Malformed {
    /// An offset or a length points outside the buffer.
    OutOfBounds,
    /// A value is not at its required alignment.
    Unaligned,
    /// A required field is absent.
    MissingRequired,
    /// A string is not valid UTF-8.
    Utf8,
    /// A union's type does not match its value.
    Union,
    /// The nesting is deeper than the verifier's limit.
    TooDeep,
    /// The buffer holds more tables than the verifier's limit.
    TooManyTables,
    /// The buffer is larger than the verifier's limit.
    TooLarge,
}

/// A value that breaks a typl constraint.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Violation {
    /// The name of the typl type whose constraint failed.
    pub type_name: &'static str,
    /// The kind of constraint that failed.
    pub rule: Rule,
}

/// The kind of typl constraint a [`Violation`] breaks.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rule {
    /// A number is outside its declared range.
    Range,
    /// A string, a byte sequence or a collection is outside its declared
    /// length bounds.
    Length,
    /// A string does not match its declared pattern.
    Pattern,
    /// A discriminant names no declared variant.
    Variant,
    /// A declared invariant evaluates to false.
    Invariant,
}
