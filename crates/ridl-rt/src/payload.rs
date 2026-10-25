//! Encoding, verifying and decoding a payload.
//!
//! A generated payload type implements [`Payload<E>`] once for each encoding.
//! Its `verify` checks the structure of the bytes and the typl constraints of
//! the value before decoding. Map uniqueness can require repeated key reads.
//! Its `decode` takes a [`Ref`], and only
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

/// The result of [`Payload::encode`]: the bytes written and their view. It is
/// data, not a proof.
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
/// and cannot build one from its fields. The error code on the `compile_fail`
/// block is documentation only, because rustdoc does not check it;
/// `crates/ridl-rt/tests/seal.rs` asserts E0451 for the forgery and E0616 for
/// a read of a field:
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
    ///
    /// Constraint checks are those performed by `T::encode`; this wrapper
    /// performs no additional verification. Received bytes are checked by
    /// [`Self::verify`].
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
        /// [`Payload::MAX_SIZE`].
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
    /// The generated proto3 codec refuses it, through this crate's narrowing
    /// helpers, because the bytes do not hold a value of the field's type.
    Varint,
    /// A tag names field number 0, a wire type that proto3 does not have (3,
    /// 4, 6 or 7), or a wire type that the known field's type does not take.
    WireType,
    /// A field that is not repeated appears twice, or a union message carries
    /// two arms, or one arm twice.
    ///
    /// Protobuf's own rule reads a repeated singular field as "last one wins",
    /// a repeated message field as a merge of every occurrence, and two
    /// `oneof` arms as "last one wins". The generated proto3 codec refuses all
    /// three, with this variant, because a payload is one value written once
    /// and no generated writer emits a field twice.
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

/// Why a transcode failed.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TranscodeError {
    /// The input bytes did not verify under the source encoding.
    Verify(VerifyError),
    /// The decoded value did not encode under the target encoding.
    Encode(EncodeError),
}

impl From<VerifyError> for TranscodeError {
    fn from(e: VerifyError) -> Self {
        TranscodeError::Verify(e)
    }
}

impl From<EncodeError> for TranscodeError {
    fn from(e: EncodeError) -> Self {
        TranscodeError::Encode(e)
    }
}

/// Re-encodes the payload `bytes` from the encoding `S` into the encoding `D`.
///
/// This is the domain-mediated reference path: it verifies `bytes` under `S`,
/// decodes the value, and encodes the value under `D`. A generated function
/// that calls it may later replace its body with a faster one that gives the
/// same result.
///
/// The encoded bytes are a subslice of `out` (a suffix of `out` for
/// FlatBuffers). An `out` that is too small is [`EncodeError::Capacity`]. The
/// input has already verified under `S` when the encode runs, so an
/// [`EncodeError::Contract`] means that the two codecs disagree on a typl
/// constraint; the function reports it.
///
/// `S` cannot be inferred, so a caller writes at least
/// `transcode::<_, Proto3, _>`.
pub fn transcode<'o, T, S, D>(
    bytes: &[u8],
    out: &'o mut [u8],
) -> Result<Ref<'o, T, D>, TranscodeError>
where
    T: Payload<S> + Payload<D>,
    S: Encoding,
    D: Encoding,
{
    let value: T = Ref::<T, S>::verify(bytes)?.decode();
    Ok(Ref::<T, D>::encode(&value, out)?)
}

#[cfg(test)]
mod tests {
    use core::marker::PhantomData;

    use super::{
        transcode, EncodeError, Encoded, Malformed, Payload, Ref, Rule, TranscodeError,
        VerifyError, Violation,
    };
    use crate::encoding::{Proto3, ReprC};

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

    /// A speed for the transcode tests. Under `ReprC` it is two little-endian
    /// bytes at the front of the buffer, and every value is legal. Under
    /// `Proto3` it is a marker byte then two little-endian bytes at the end
    /// of the buffer, and a value above 300 is refused.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    struct Speed(u16);

    const SPEED_MAX: u16 = 300;

    impl Payload<ReprC> for Speed {
        const MAX_SIZE: usize = 2;
        type Buffer = [u8; 2];
        const BUFFER: [u8; 2] = [0; 2];
        type View<'a> = &'a [u8];

        fn encode<'o>(&self, out: &'o mut [u8]) -> Result<Encoded<'o, &'o [u8]>, EncodeError> {
            if out.len() < 2 {
                return Err(EncodeError::Capacity {
                    needed: 2,
                    available: out.len(),
                });
            }
            out[..2].copy_from_slice(&self.0.to_le_bytes());
            let bytes: &'o [u8] = &out[..2];
            Ok(Encoded { bytes, view: bytes })
        }

        fn verify(buf: &[u8]) -> Result<&[u8], VerifyError> {
            if buf.len() == 2 {
                Ok(buf)
            } else {
                Err(VerifyError::Structure(Malformed::OutOfBounds))
            }
        }

        fn decode(r: Ref<'_, Self, ReprC>) -> Self {
            Speed(u16::from_le_bytes([r.bytes()[0], r.bytes()[1]]))
        }
    }

    impl Payload<Proto3> for Speed {
        const MAX_SIZE: usize = 3;
        type Buffer = [u8; 3];
        const BUFFER: [u8; 3] = [0; 3];
        type View<'a> = &'a [u8];

        fn encode<'o>(&self, out: &'o mut [u8]) -> Result<Encoded<'o, &'o [u8]>, EncodeError> {
            if self.0 > SPEED_MAX {
                return Err(EncodeError::Contract(Violation {
                    type_name: Some("Speed"),
                    rule: Rule::Range,
                }));
            }
            let len = out.len();
            if len < 3 {
                return Err(EncodeError::Capacity {
                    needed: 3,
                    available: len,
                });
            }
            let le = self.0.to_le_bytes();
            out[len - 3..].copy_from_slice(&[0x08, le[0], le[1]]);
            let bytes: &'o [u8] = &out[len - 3..];
            Ok(Encoded { bytes, view: bytes })
        }

        fn verify(buf: &[u8]) -> Result<&[u8], VerifyError> {
            if buf.len() != 3 || buf[0] != 0x08 {
                return Err(VerifyError::Structure(Malformed::WireType));
            }
            if u16::from_le_bytes([buf[1], buf[2]]) > SPEED_MAX {
                return Err(VerifyError::Contract(Violation {
                    type_name: Some("Speed"),
                    rule: Rule::Range,
                }));
            }
            Ok(buf)
        }

        fn decode(r: Ref<'_, Self, Proto3>) -> Self {
            Speed(u16::from_le_bytes([r.bytes()[1], r.bytes()[2]]))
        }
    }

    #[test]
    fn transcode_matches_direct_encode() {
        let mut source = [0u8; 8];
        let source = Ref::<Speed, ReprC>::encode(&Speed(120), &mut source).unwrap();

        let mut out = [0u8; 8];
        let moved = transcode::<Speed, ReprC, Proto3>(source.bytes(), &mut out).unwrap();

        let mut direct = [0u8; 8];
        let direct = Ref::<Speed, Proto3>::encode(&Speed(120), &mut direct).unwrap();
        assert_eq!(moved.bytes(), direct.bytes());
        assert_eq!(moved.decode(), Speed(120));
    }

    #[test]
    fn transcode_reverse_direction() {
        let mut source = [0u8; 8];
        let source = Ref::<Speed, Proto3>::encode(&Speed(250), &mut source).unwrap();

        let mut out = [0u8; 8];
        let moved = transcode::<Speed, Proto3, ReprC>(source.bytes(), &mut out).unwrap();

        let mut direct = [0u8; 8];
        let direct = Ref::<Speed, ReprC>::encode(&Speed(250), &mut direct).unwrap();
        assert_eq!(moved.bytes(), direct.bytes());
        assert_eq!(moved.decode(), Speed(250));
    }

    #[test]
    fn transcode_same_encoding_is_identity() {
        let mut source = [0u8; 8];
        let source = Ref::<Speed, Proto3>::encode(&Speed(77), &mut source).unwrap();

        let mut out = [0u8; 8];
        let moved = transcode::<Speed, Proto3, Proto3>(source.bytes(), &mut out).unwrap();
        assert_eq!(moved.bytes(), source.bytes());
    }

    #[test]
    fn transcode_refuses_bad_input() {
        let mut out = [0u8; 8];
        let result = transcode::<Speed, ReprC, Proto3>(&[1, 2, 3], &mut out).map(|r| r.bytes());
        assert!(matches!(result, Err(TranscodeError::Verify(_))));
    }

    #[test]
    fn transcode_reports_contract() {
        // `ReprC` accepts 301; `Proto3` refuses it.
        let mut out = [0u8; 8];
        let result =
            transcode::<Speed, ReprC, Proto3>(&301u16.to_le_bytes(), &mut out).map(|r| r.bytes());
        assert!(matches!(
            result,
            Err(TranscodeError::Encode(EncodeError::Contract(_)))
        ));
    }

    #[test]
    fn transcode_reports_capacity() {
        let mut out = [0u8; 1];
        let result =
            transcode::<Speed, ReprC, Proto3>(&120u16.to_le_bytes(), &mut out).map(|r| r.bytes());
        assert!(matches!(
            result,
            Err(TranscodeError::Encode(EncodeError::Capacity { .. }))
        ));
    }
}
