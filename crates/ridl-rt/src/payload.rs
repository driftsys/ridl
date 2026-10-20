//! Encoding, verifying and decoding a payload.
//!
//! A generated payload type implements [`Payload<E>`] once for each encoding
//! in which its largest encoding has a bound, and [`Codec<E>`] alone for an
//! encoding in which it has none. Every `Payload<E>` is a `Codec<E>` through
//! a blanket implementation. A codec's `verify` checks the structure of the
//! bytes and the typl constraints of the value before decoding. Map
//! uniqueness can require repeated key reads. Its `decode` takes a [`Ref`],
//! and only [`Ref::verify`] and [`Ref::encode`] build a `Ref`. So a value is
//! decoded only from bytes that were checked, or that its own encoder wrote.

use core::marker::PhantomData;

use crate::encoding::Encoding;

/// A type that can be carried as a payload in the encoding `E`, whose
/// largest encoding has a bound.
///
/// Generated code implements this trait and never calls its methods directly:
/// it calls [`Ref::verify`], [`Ref::encode`] and [`Ref::decode`]. A type
/// whose encoding has no bound implements [`Codec<E>`] instead.
pub trait Payload<E: Encoding>: Sized {
    /// The largest encoded size of any legal value, in bytes.
    const MAX_SIZE: usize;

    /// `[u8; Self::MAX_SIZE]`, stated as a type so that code generic over the
    /// encoding can hold one on the stack (a generic array length is not
    /// expressible on stable Rust).
    type Buffer: AsRef<[u8]> + AsMut<[u8]> + Copy;

    /// A zeroed [`Payload::Buffer`].
    const BUFFER: Self::Buffer;

    /// What a successful check or encode leaves behind: the bytes, an
    /// accessor over the bytes, or a parsed value.
    type View<'a>;

    /// Writes `self` into `out` and returns the bytes written.
    ///
    /// The bytes are a subslice of `out`, not necessarily a prefix of it: an
    /// encoding whose builder works backwards — FlatBuffers does — fills `out`
    /// from its end. A caller reads [`Encoded::bytes`] and passes it on rather
    /// than assuming where in `out` it sits.
    fn encode<'o>(&self, out: &'o mut [u8]) -> Result<Encoded<'o, Self::View<'o>>, EncodeError>;

    /// Checks the structure of `buf` and the typl constraints of the value it
    /// holds. Verifying map uniqueness can require repeated key reads.
    fn verify(buf: &[u8]) -> Result<Self::View<'_>, VerifyError>;

    /// Builds the value from a proof. It cannot fail, because the proof shows
    /// that the bytes were checked or encoded.
    fn decode(r: Ref<'_, Self, E>) -> Self;
}

/// A type that can be encoded, verified and decoded in the encoding `E`,
/// with or without a bound on its largest encoding.
///
/// Every [`Payload<E>`] is a `Codec<E>`, with [`BOUND`](Codec::BOUND) equal
/// to `Some(MAX_SIZE)`, through a blanket implementation. A type whose
/// encoding has no bound — a proto3 string with no length bound, or a
/// collection whose count has none — implements `Codec<E>` directly, with
/// `BOUND` equal to `None`: it has no `MAX_SIZE`, no `Buffer` and no
/// `BUFFER`, so generic code that holds a buffer on the stack cannot name
/// it, and a caller gives [`Ref::encode`] a buffer of its own. An encoder
/// that knows its size before it writes reports the exact length in
/// [`EncodeError::Capacity`] when the buffer is too short.
///
/// [`Ref`] is bounded by this trait, so the three operations work the same
/// way for a type with a bound and for one without.
pub trait Codec<E: Encoding>: Sized {
    /// What a successful check or encode leaves behind: the bytes, an
    /// accessor over the bytes, or a parsed value.
    type View<'a>;

    /// The largest encoded size of any legal value, in bytes, or `None` when
    /// no such bound exists.
    const BOUND: Option<usize>;

    /// Writes `self` into `out` and returns the bytes written. The bytes are
    /// a subslice of `out`, not necessarily a prefix of it; see
    /// [`Payload::encode`].
    fn encode<'o>(&self, out: &'o mut [u8]) -> Result<Encoded<'o, Self::View<'o>>, EncodeError>;

    /// Checks the structure of `buf` and the typl constraints of the value it
    /// holds.
    fn verify(buf: &[u8]) -> Result<Self::View<'_>, VerifyError>;

    /// Builds the value from a proof. It cannot fail, because the proof shows
    /// that the bytes were checked or encoded.
    fn decode(r: Ref<'_, Self, E>) -> Self;
}

impl<T: Payload<E>, E: Encoding> Codec<E> for T {
    type View<'a> = <T as Payload<E>>::View<'a>;

    const BOUND: Option<usize> = Some(<T as Payload<E>>::MAX_SIZE);

    fn encode<'o>(&self, out: &'o mut [u8]) -> Result<Encoded<'o, Self::View<'o>>, EncodeError> {
        <T as Payload<E>>::encode(self, out)
    }

    fn verify(buf: &[u8]) -> Result<Self::View<'_>, VerifyError> {
        <T as Payload<E>>::verify(buf)
    }

    fn decode(r: Ref<'_, Self, E>) -> Self {
        <T as Payload<E>>::decode(r)
    }
}

/// A view that holds the verified bytes it reads.
///
/// A generated view — the `<T>FbView<'a>` a FlatBuffers `Payload::verify`
/// hands back — implements this trait rather than carrying `bytes` as an
/// inherent method, so that a field whose accessor is named `bytes` does not
/// meet it: a trait method has its own namespace. The accessor is inherent
/// and wins the dot call; the buffer is reached through the trait's path.
///
/// ```
/// use ridl_rt::payload::View;
///
/// struct Raw<'a> {
///     buf: &'a [u8],
/// }
///
/// impl<'a> Raw<'a> {
///     /// A field accessor that happens to share the trait method's name.
///     fn bytes(&self) -> usize {
///         self.buf.len()
///     }
/// }
///
/// impl<'a> View<'a> for Raw<'a> {
///     fn bytes(&self) -> &'a [u8] {
///         self.buf
///     }
/// }
///
/// let raw = Raw { buf: &[1, 2, 3] };
/// let count: usize = raw.bytes();
/// let buffer: &[u8] = View::bytes(&raw);
/// assert_eq!(count, 3);
/// assert_eq!(buffer, &[1, 2, 3]);
/// ```
pub trait View<'a> {
    /// The verified bytes this view reads.
    fn bytes(&self) -> &'a [u8];
}

/// The result of [`Payload::encode`] or [`Codec::encode`]: the bytes written
/// and their view. It is data, not a proof.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Encoded<'a, V> {
    /// The encoded bytes, a subslice of the output buffer.
    pub bytes: &'a [u8],
    /// The view of those bytes.
    pub view: V,
}

/// A proof that some bytes passed `T::verify` or were written by `T::encode`.
///
/// The fields are private and [`Ref::verify`] and [`Ref::encode`] are the only
/// constructors. `T` is any [`Codec<E>`], so a `Ref` serves a type with a
/// bound on its encoding and one without alike. A crate outside `ridl-rt`
/// builds a `Ref` by checking:
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
pub struct Ref<'a, T: Codec<E>, E: Encoding> {
    bytes: &'a [u8],
    view: <T as Codec<E>>::View<'a>,
    _e: PhantomData<E>,
}

impl<'a, T: Codec<E>, E: Encoding> Ref<'a, T, E> {
    /// Checks `buf` with `T::verify` and returns the proof.
    pub fn verify(buf: &'a [u8]) -> Result<Self, VerifyError> {
        let view = <T as Codec<E>>::verify(buf)?;
        Ok(Ref {
            bytes: buf,
            view,
            _e: PhantomData,
        })
    }

    /// Encodes `value` into `out` with `T::encode` and returns the proof.
    ///
    /// Constraint checks are those performed by `T::encode`; this wrapper
    /// performs no additional verification. Received bytes are checked by
    /// [`Self::verify`].
    pub fn encode(value: &T, out: &'a mut [u8]) -> Result<Self, EncodeError> {
        let Encoded { bytes, view } = <T as Codec<E>>::encode(value, out)?;
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
    pub fn view(&self) -> &<T as Codec<E>>::View<'a> {
        &self.view
    }

    /// Moves the view out.
    pub fn into_view(self) -> <T as Codec<E>>::View<'a> {
        self.view
    }

    /// Builds the value with `T::decode`.
    pub fn decode(self) -> T {
        <T as Codec<E>>::decode(self)
    }
}

/// An encode that failed.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EncodeError {
    /// The output buffer is shorter than the encoding.
    Capacity {
        /// The bytes the encoding needs: a lower bound, not always the whole
        /// requirement.
        ///
        /// An encoder that knows its size before it writes reports the whole
        /// encoding. One that builds incrementally — the FlatBuffers encoder
        /// does, because a child's size is known only once it is written —
        /// reports what it needed at the point it gave up, which is at least
        /// `available + 1` and at most the whole encoding. Either way a
        /// caller that grows the buffer to `needed` has made progress, and
        /// one that wants a buffer that always suffices uses
        /// [`Payload::MAX_SIZE`] — when the type has one: for a type whose
        /// [`Codec::BOUND`] is `None`, growing to `needed` is the only way.
        needed: usize,
        /// The bytes the output buffer has.
        available: usize,
    },
    /// The value breaks a typl constraint: `INVALID_VALUE`, ridl §10.2.
    ///
    /// The mirror of [`VerifyError::Contract`]: the encoder refused the value
    /// and reports the rule the receiver's `verify` would have reported for
    /// the same value, so the two sides name one violation. A bounded
    /// collection is the shape this reaches: it is a plain field of the
    /// generated type, so a value longer than its declared bound is
    /// representable, and the encoder is where it is refused.
    Contract(Violation),
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
    /// A varint is longer than ten bytes, has a tenth byte above 1, or holds
    /// a value wider than the scalar of its field: a `uint32` above
    /// `u32::MAX`, or a `sint32` or an enum outside `i32`.
    ///
    /// Protobuf parsers truncate an over-wide varint to the field's width.
    /// This crate's proto3 codec refuses it, because the bytes do not hold a
    /// value of the field's type.
    Varint,
    /// A tag names field number 0, a wire type that proto3 does not have (3,
    /// 4, 6 or 7), or a wire type that the known field's type does not take.
    WireType,
    /// A field that is not repeated appears twice, or a union message carries
    /// two arms, or one arm twice.
    ///
    /// Protobuf's own rule reads a repeated singular field as "last one wins",
    /// a repeated message field as a merge of every occurrence, and two
    /// `oneof` arms as "last one wins". This crate's proto3 codec refuses all
    /// three, because a payload is one value written once and no generated
    /// writer emits a field twice.
    Duplicate,
}

/// A value that breaks a typl constraint.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Violation {
    /// The name of the typl type whose constraint failed. `None` only for a
    /// `contract(InvalidValue)` outcome decoded from a frame, because a frame
    /// carries the [`Rule`] alone (the frame specification §9.6). It is `Some`
    /// otherwise: when the generated code detected the violation, including a
    /// consumer's own detection on bytes received in a frame, and when an
    /// in-process transport such as `ridl-loopback` delivered it.
    pub type_name: Option<&'static str>,
    /// The kind of constraint that failed.
    pub rule: Rule,
}

/// The kind of typl constraint a [`Violation`] breaks.
///
/// A variant is added only together with a new frame version, because a
/// receiver cannot construct a variant it does not know (the frame
/// specification §9.6); the enum stays `#[non_exhaustive]`.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rule {
    /// A number is outside its declared range.
    Range,
    /// A number is not its range's lower bound plus a whole multiple of its
    /// declared step.
    Step,
    /// A string, a byte sequence or a collection is outside its declared
    /// length bounds.
    Length,
    /// A string does not match its declared pattern.
    Pattern,
    /// A discriminant names no declared variant.
    Variant,
    /// A map contains more than one entry with the same key.
    Unique,
}

#[cfg(test)]
mod tests {
    use core::marker::PhantomData;

    use super::{EncodeError, Encoded, Malformed, Payload, Ref, Rule, VerifyError};
    use crate::encoding::ReprC;

    /// A payload for this test only: its view is the bytes.
    struct Raw;

    impl Payload<ReprC> for Raw {
        const MAX_SIZE: usize = 0;
        type Buffer = [u8; 0];
        const BUFFER: [u8; 0] = [];
        type View<'a> = &'a [u8];

        fn encode<'o>(&self, out: &'o mut [u8]) -> Result<Encoded<'o, &'o [u8]>, EncodeError> {
            let bytes: &'o [u8] = &out[..0];
            Ok(Encoded { bytes, view: bytes })
        }

        fn verify(buf: &[u8]) -> Result<&[u8], VerifyError> {
            Ok(buf)
        }

        fn decode(_: Ref<'_, Self, ReprC>) -> Self {
            Raw
        }
    }

    /// The struct literal names every field, so a field added to `Ref` fails
    /// to compile this test. The `compile_fail` doctest on `Ref` cannot pin
    /// the field set, because it fails for any compile error.
    #[test]
    fn a_ref_is_built_from_exactly_its_three_fields() {
        let bytes: &[u8] = &[1, 2];
        let proof: Ref<'_, Raw, ReprC> = Ref {
            bytes,
            view: bytes,
            _e: PhantomData,
        };
        assert_eq!(proof.bytes(), bytes);
    }

    /// The match names every variant with no `_` arm, so a `Rule` variant
    /// added or removed fails this test to compile.
    #[test]
    fn rule_is_exactly_these_six_variants() {
        fn all(r: Rule) {
            match r {
                Rule::Range => {}
                Rule::Step => {}
                Rule::Length => {}
                Rule::Pattern => {}
                Rule::Variant => {}
                Rule::Unique => {}
            }
        }
        all(Rule::Range);
        all(Rule::Unique);
    }

    /// The three variants the proto3 codec adds exist on every feature set:
    /// `Malformed` is not gated, so a crate that matches on it sees the same
    /// enum whichever encodings it enables.
    #[test]
    fn malformed_has_the_three_proto3_variants() {
        let added = [Malformed::Varint, Malformed::WireType, Malformed::Duplicate];
        for (i, a) in added.iter().enumerate() {
            for (j, b) in added.iter().enumerate() {
                assert_eq!(a == b, i == j);
            }
            assert_ne!(*a, Malformed::OutOfBounds);
        }
    }
}
