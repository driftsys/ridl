//! `Payload<E>` and the `Ref` proof, used as a generated package uses them:
//! from another crate, with no `unsafe`.

#![forbid(unsafe_code)]

use std::cell::Cell;

use ridl_rt::encoding::{Proto3, ReprC};
use ridl_rt::payload::{
    EncodeError, Encoded, Malformed, Payload, Ref, Rule, VerifyError, Violation,
};

thread_local! {
    /// How many times `<Speed as Payload<ReprC>>::verify` ran on this thread.
    /// Thread-local, because the test harness runs each test on its own
    /// thread.
    static VERIFY_CALLS: Cell<usize> = const { Cell::new(0) };
}

/// A typl-style scalar: speed in km/h, declared in the range 0 to 300.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Speed(u16);

const SPEED_RANGE: Violation = Violation {
    type_name: "Speed",
    rule: Rule::Range,
};

fn check(value: u16) -> Result<(), VerifyError> {
    if value > 300 {
        Err(VerifyError::Contract(SPEED_RANGE))
    } else {
        Ok(())
    }
}

fn read_le(buf: &[u8]) -> Result<u16, VerifyError> {
    let bytes: [u8; 2] = buf
        .try_into()
        .map_err(|_| VerifyError::Structure(Malformed::OutOfBounds))?;
    Ok(u16::from_le_bytes(bytes))
}

fn write_le(value: u16, out: &mut [u8]) -> Result<&[u8], EncodeError> {
    let available = out.len();
    let Some(front) = out.get_mut(..2) else {
        return Err(EncodeError::Capacity {
            needed: 2,
            available,
        });
    };
    front.copy_from_slice(&value.to_le_bytes());
    Ok(front)
}

/// A codec whose view is the checked bytes.
impl Payload<ReprC> for Speed {
    const MAX_SIZE: usize = 2;
    type View<'a> = &'a [u8];

    fn encode<'o>(&self, out: &'o mut [u8]) -> Result<Encoded<'o, &'o [u8]>, EncodeError> {
        let bytes = write_le(self.0, out)?;
        Ok(Encoded { bytes, view: bytes })
    }

    fn verify(buf: &[u8]) -> Result<&[u8], VerifyError> {
        VERIFY_CALLS.with(|calls| calls.set(calls.get() + 1));
        check(read_le(buf)?)?;
        Ok(buf)
    }

    fn decode(r: Ref<'_, Self, ReprC>) -> Self {
        let b = r.bytes();
        Speed(u16::from_le_bytes([b[0], b[1]]))
    }
}

/// The parsed value a second codec keeps. It is not `Copy`, as a parsed view
/// that owns a string would not be.
#[derive(Debug, PartialEq, Eq)]
struct Parsed {
    value: u16,
}

/// A second codec, for this test only: it writes the same little-endian bytes
/// as above, keeps a parsed view instead of a byte view, and declares a
/// different `MAX_SIZE`, so the `MAX_SIZE` test below can tell the two
/// constants apart.
impl Payload<Proto3> for Speed {
    const MAX_SIZE: usize = 3;
    type View<'a> = Parsed;

    fn encode<'o>(&self, out: &'o mut [u8]) -> Result<Encoded<'o, Parsed>, EncodeError> {
        let bytes = write_le(self.0, out)?;
        Ok(Encoded {
            bytes,
            view: Parsed { value: self.0 },
        })
    }

    fn verify(buf: &[u8]) -> Result<Parsed, VerifyError> {
        let value = read_le(buf)?;
        check(value)?;
        Ok(Parsed { value })
    }

    fn decode(r: Ref<'_, Self, Proto3>) -> Self {
        Speed(r.into_view().value)
    }
}

#[test]
fn verified_bytes_decode_to_the_value() {
    let proof = Ref::<Speed, ReprC>::verify(&[0x2c, 0x01]).expect("300 is in range");
    assert_eq!(proof.bytes(), &[0x2c, 0x01]);
    assert_eq!(proof.decode(), Speed(300));
}

#[test]
fn encoded_bytes_decode_without_a_second_check() {
    let mut out = [0u8; 4];
    let before = VERIFY_CALLS.with(Cell::get);
    let proof = Ref::<Speed, ReprC>::encode(&Speed(88), &mut out).expect("4 bytes is enough");
    assert_eq!(proof.bytes(), &[88, 0]);
    assert_eq!(proof.decode(), Speed(88));
    assert_eq!(
        VERIFY_CALLS.with(Cell::get),
        before,
        "verify ran during encode or decode"
    );
}

#[test]
fn a_value_outside_its_range_fails_the_contract_check() {
    let bytes = 301u16.to_le_bytes();
    let result = Ref::<Speed, ReprC>::verify(&bytes);
    assert_eq!(result.err(), Some(VerifyError::Contract(SPEED_RANGE)));
}

#[test]
fn a_truncated_buffer_fails_the_structure_check() {
    let result = Ref::<Speed, ReprC>::verify(&[0x2c]);
    assert_eq!(
        result.err(),
        Some(VerifyError::Structure(Malformed::OutOfBounds))
    );
}

#[test]
fn a_short_output_buffer_is_a_capacity_error() {
    let mut out = [0u8; 1];
    let result = Ref::<Speed, ReprC>::encode(&Speed(1), &mut out);
    assert_eq!(
        result.err(),
        Some(EncodeError::Capacity {
            needed: 2,
            available: 1
        })
    );
}

#[test]
fn a_parsed_view_is_lent_then_moved_out() {
    let proof = Ref::<Speed, Proto3>::verify(&[0x2c, 0x01]).expect("300 is in range");
    assert_eq!(proof.view(), &Parsed { value: 300 });
    assert_eq!(proof.decode(), Speed(300));
}

#[test]
fn a_type_with_two_codecs_names_each_max_size_through_its_encoding() {
    assert_eq!(<Speed as Payload<ReprC>>::MAX_SIZE, 2);
    assert_eq!(<Speed as Payload<Proto3>>::MAX_SIZE, 3);
}

#[test]
fn every_payload_error_is_copy_and_owns_nothing() {
    fn owns_nothing<T: Copy + Eq + core::fmt::Debug + 'static>() {}
    owns_nothing::<EncodeError>();
    owns_nothing::<VerifyError>();
    owns_nothing::<Malformed>();
    owns_nothing::<Violation>();
    owns_nothing::<Rule>();
}
