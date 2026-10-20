//! The size table per codec: `Sizes<E>`, `reservation` and `table_budget`.
//! Every number in a table derives from `<T as Payload<E>>::MAX_SIZE`, so
//! each row of the fixture below is written as that path, never as a number,
//! the way generated code writes it.

#![forbid(unsafe_code)]

use ridl_rt::contract::{
    reservation, table_budget, Bounds, CatalogHash, CatalogRef, Interaction, Interface,
    InterfaceNo, Kind, Member, Ordinal, PayloadInfo, Sizes,
};
use ridl_rt::encoding::{Encoding, FlatBuffers};
use ridl_rt::payload::{EncodeError, Encoded, Malformed, Payload, Ref, VerifyError};

const CABIN: CatalogRef = CatalogRef {
    name: "cabin",
    hash: CatalogHash([3; 32]),
};

/// Writes `value` into the front of `out`, padded to `len` bytes, and returns
/// the bytes written.
fn write_padded(value: u8, len: usize, out: &mut [u8]) -> Result<&[u8], EncodeError> {
    let available = out.len();
    let Some(front) = out.get_mut(..len) else {
        return Err(EncodeError::Capacity {
            needed: len,
            available,
        });
    };
    front.fill(0);
    front[0] = value;
    Ok(front)
}

fn read_padded(len: usize, buf: &[u8]) -> Result<&[u8], VerifyError> {
    if buf.len() == len {
        Ok(buf)
    } else {
        Err(VerifyError::Structure(Malformed::OutOfBounds))
    }
}

/// An event payload of 8 bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Warning(u8);

impl Payload<FlatBuffers> for Warning {
    const MAX_SIZE: usize = 8;
    type Buffer = [u8; 8];
    const BUFFER: [u8; 8] = [0; 8];
    type View<'a> = &'a [u8];

    fn encode<'o>(&self, out: &'o mut [u8]) -> Result<Encoded<'o, &'o [u8]>, EncodeError> {
        let bytes = write_padded(self.0, Self::MAX_SIZE, out)?;
        Ok(Encoded { bytes, view: bytes })
    }

    fn verify(buf: &[u8]) -> Result<&[u8], VerifyError> {
        read_padded(Self::MAX_SIZE, buf)
    }

    fn decode(r: Ref<'_, Self, FlatBuffers>) -> Self {
        Warning(r.bytes()[0])
    }
}

/// A call payload of 24 bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Window(u8);

impl Payload<FlatBuffers> for Window {
    const MAX_SIZE: usize = 24;
    type Buffer = [u8; 24];
    const BUFFER: [u8; 24] = [0; 24];
    type View<'a> = &'a [u8];

    fn encode<'o>(&self, out: &'o mut [u8]) -> Result<Encoded<'o, &'o [u8]>, EncodeError> {
        let bytes = write_padded(self.0, Self::MAX_SIZE, out)?;
        Ok(Encoded { bytes, view: bytes })
    }

    fn verify(buf: &[u8]) -> Result<&[u8], VerifyError> {
        read_padded(Self::MAX_SIZE, buf)
    }

    fn decode(r: Ref<'_, Self, FlatBuffers>) -> Self {
        Window(r.bytes()[0])
    }
}

const WARNING: Member = Member {
    ordinal: Ordinal(1),
    kind: Kind::Event,
    name: "warning",
    timing: None,
    payloads: &[PayloadInfo {
        type_name: "Warning",
    }],
};

const AVERAGE: Member = Member {
    ordinal: Ordinal(2),
    kind: Kind::Query,
    name: "average",
    timing: None,
    payloads: &[
        PayloadInfo {
            type_name: "Window",
        },
        PayloadInfo {
            type_name: "Warning",
        },
    ],
};

const SET_WINDOW: Member = Member {
    ordinal: Ordinal(3),
    kind: Kind::Command,
    name: "setWindow",
    timing: None,
    payloads: &[PayloadInfo {
        type_name: "Window",
    }],
};

/// `interface Cabin`: an event of 8 bytes, a query of 24 + 8 bytes and a
/// command of 24 bytes, so the three rows are distinct.
struct Cabin;

impl Interface for Cabin {
    const CATALOG: &'static CatalogRef = &CABIN;
    const NUMBER: InterfaceNo = InterfaceNo(1);
    const PROVISIONAL: bool = false;
    const NAME: &'static str = "Cabin";
    const MEMBERS: &'static [Member] = &[WARNING, AVERAGE, SET_WINDOW];
}

/// The table as generated code writes it: every item a const expression over
/// `MAX_SIZE`.
impl Sizes<FlatBuffers> for Cabin {
    const RESERVATIONS: &'static [u64] = &[
        <Warning as Payload<FlatBuffers>>::MAX_SIZE as u64,
        <Window as Payload<FlatBuffers>>::MAX_SIZE as u64
            + <Warning as Payload<FlatBuffers>>::MAX_SIZE as u64,
        <Window as Payload<FlatBuffers>>::MAX_SIZE as u64,
    ];
    const MAX_BUFFER_SIZE: usize = <Window as Payload<FlatBuffers>>::MAX_SIZE;
    const EVENT_SOURCE_BUFFER_SIZE: usize = <Warning as Payload<FlatBuffers>>::MAX_SIZE;
    type ClaimBuffer = [u8; Self::MAX_BUFFER_SIZE];
    type EventBuffer = [u8; Self::EVENT_SOURCE_BUFFER_SIZE];
    const CLAIM_BUFFER: Self::ClaimBuffer = [0u8; Self::MAX_BUFFER_SIZE];
    const EVENT_BUFFER: Self::EventBuffer = [0u8; Self::EVENT_SOURCE_BUFFER_SIZE];
}

/// The interaction descriptors, each carrying its row, as generated code
/// writes them.
struct CabinWarning;

impl Interaction for CabinWarning {
    type Iface = Cabin;
    const MEMBER: &'static Member = &Cabin::MEMBERS[0];
    const ROW: usize = 0;
}

struct CabinAverage;

impl Interaction for CabinAverage {
    type Iface = Cabin;
    const MEMBER: &'static Member = &Cabin::MEMBERS[1];
    const ROW: usize = 1;
}

struct CabinSetWindow;

impl Interaction for CabinSetWindow {
    type Iface = Cabin;
    const MEMBER: &'static Member = &Cabin::MEMBERS[2];
    const ROW: usize = 2;
}

/// `interface Huge`: two rows whose sum overflows a `u64`.
struct Huge;

impl Interface for Huge {
    const CATALOG: &'static CatalogRef = &CABIN;
    const NUMBER: InterfaceNo = InterfaceNo(2);
    const PROVISIONAL: bool = false;
    const NAME: &'static str = "Huge";
    const MEMBERS: &'static [Member] = &[SET_WINDOW, WARNING];
}

impl Sizes<FlatBuffers> for Huge {
    const RESERVATIONS: &'static [u64] = &[u64::MAX, 1];
    const MAX_BUFFER_SIZE: usize = 0;
    const EVENT_SOURCE_BUFFER_SIZE: usize = 0;
    type ClaimBuffer = [u8; 0];
    type EventBuffer = [u8; 0];
    const CLAIM_BUFFER: Self::ClaimBuffer = [];
    const EVENT_BUFFER: Self::EventBuffer = [];
}

/// `interface Empty`: no member, so no row.
struct Empty;

impl Interface for Empty {
    const CATALOG: &'static CatalogRef = &CABIN;
    const NUMBER: InterfaceNo = InterfaceNo(3);
    const PROVISIONAL: bool = false;
    const NAME: &'static str = "Empty";
    const MEMBERS: &'static [Member] = &[];
}

impl Sizes<FlatBuffers> for Empty {
    const RESERVATIONS: &'static [u64] = &[];
    const MAX_BUFFER_SIZE: usize = 0;
    const EVENT_SOURCE_BUFFER_SIZE: usize = 0;
    type ClaimBuffer = [u8; 0];
    type EventBuffer = [u8; 0];
    const CLAIM_BUFFER: Self::ClaimBuffer = [];
    const EVENT_BUFFER: Self::EventBuffer = [];
}

/// `interface Wide`: one query whose request and reply are each as large as a
/// `u32` can state. The row is written as the sum of the two sizes, each cast
/// to `u64` first, the way generated code writes a query's row.
struct Wide;

impl Interface for Wide {
    const CATALOG: &'static CatalogRef = &CABIN;
    const NUMBER: InterfaceNo = InterfaceNo(4);
    const PROVISIONAL: bool = false;
    const NAME: &'static str = "Wide";
    const MEMBERS: &'static [Member] = &[AVERAGE];
}

impl Sizes<FlatBuffers> for Wide {
    const RESERVATIONS: &'static [u64] = &[(u32::MAX as usize) as u64 + (u32::MAX as usize) as u64];
    const MAX_BUFFER_SIZE: usize = 0;
    const EVENT_SOURCE_BUFFER_SIZE: usize = 0;
    type ClaimBuffer = [u8; 0];
    type EventBuffer = [u8; 0];
    const CLAIM_BUFFER: Self::ClaimBuffer = [];
    const EVENT_BUFFER: Self::EventBuffer = [];
}

struct WideAverage;

impl Interaction for WideAverage {
    type Iface = Wide;
    const MEMBER: &'static Member = &Wide::MEMBERS[0];
    const ROW: usize = 0;
}

/// `Interaction::ROW` and `Sizes::RESERVATIONS`: an interaction's reservation
/// is its row of its interface's table. The query's row is the sum of its two
/// payloads, so a maximum over them (24) or the first alone fails.
#[test]
fn a_reservation_is_the_interactions_row() {
    assert_eq!(reservation::<CabinAverage, FlatBuffers>(), 32);
    assert_eq!(reservation::<CabinWarning, FlatBuffers>(), 8);
    assert_eq!(reservation::<CabinSetWindow, FlatBuffers>(), 24);
}

/// `ROW` indexes the member `MEMBER` points at, for every descriptor of the
/// interface.
#[test]
fn a_row_indexes_the_descriptors_member() {
    fn row_is_member<X: Interaction>() {
        assert_eq!(
            <X::Iface as Interface>::MEMBERS[X::ROW],
            *X::MEMBER,
            "{}",
            X::MEMBER.name
        );
    }
    row_is_member::<CabinWarning>();
    row_is_member::<CabinAverage>();
    row_is_member::<CabinSetWindow>();
}

/// `table_budget` sums every row: three distinct rows, so a maximum or a
/// first row gives another number.
#[test]
fn a_table_budget_sums_every_row() {
    assert_eq!(table_budget::<Cabin, FlatBuffers>(), 64);
}

/// An interface with no member has a budget of zero.
#[test]
fn an_empty_table_has_a_budget_of_zero() {
    assert_eq!(table_budget::<Empty, FlatBuffers>(), 0);
}

/// A row is a `u64`, wider than one `u32` size, so a query whose two payloads
/// are each the largest size a `u32` states does not overflow its row.
#[test]
fn the_sum_is_wider_than_one_size() {
    assert_eq!(
        reservation::<WideAverage, FlatBuffers>(),
        2 * u64::from(u32::MAX)
    );
    assert_eq!(table_budget::<Wide, FlatBuffers>(), 2 * u64::from(u32::MAX));
}

/// The sum saturates rather than wrapping.
#[test]
fn a_table_budget_saturates() {
    assert_eq!(table_budget::<Huge, FlatBuffers>(), u64::MAX);
}

/// `table_budget` and `reservation` are `const fn`: a `static` table is sized
/// from them at compile time, with no allocator. The initializer repeats the
/// length, because `[0; _]` is not accepted by the oldest Rust this crate
/// supports.
#[test]
fn a_static_table_is_sized_from_the_budget() {
    static TABLE: [u8; table_budget::<Cabin, FlatBuffers>() as usize] =
        [0; table_budget::<Cabin, FlatBuffers>() as usize];
    static ROW: [u8; reservation::<CabinAverage, FlatBuffers>() as usize] =
        [0; reservation::<CabinAverage, FlatBuffers>() as usize];
    assert_eq!(TABLE.len(), 64);
    assert_eq!(ROW.len(), 32);
}

/// Holds both interface-wide buffers through the trait alone, as a face
/// generic over the encoding does.
fn hold<I: Sizes<E>, E: Encoding>() -> (usize, usize) {
    let mut claim = I::CLAIM_BUFFER;
    let mut event = I::EVENT_BUFFER;
    assert!(claim.as_ref().iter().all(|byte| *byte == 0));
    assert!(event.as_ref().iter().all(|byte| *byte == 0));
    (claim.as_mut().len(), event.as_mut().len())
}

/// `CLAIM_BUFFER` has `MAX_BUFFER_SIZE` bytes, `EVENT_BUFFER` has
/// `EVENT_SOURCE_BUFFER_SIZE` bytes, and both are zeroed.
#[test]
fn the_claim_and_event_buffers_have_the_interface_maxima() {
    assert_eq!(hold::<Cabin, FlatBuffers>(), (24, 8));
}

/// The registration pattern of a data-driven runtime: it takes the members
/// and the rows as two parallel slices at the one point where the interface
/// type is known.
fn register(members: &'static [Member], rows: &'static [u64]) -> u64 {
    assert_eq!(members.len(), rows.len());
    rows.iter().sum()
}

/// `RESERVATIONS` is a `&'static [u64]` parallel to `Interface::MEMBERS`.
#[test]
fn a_data_driven_runtime_registers_the_slice() {
    assert_eq!(
        register(Cabin::MEMBERS, <Cabin as Sizes<FlatBuffers>>::RESERVATIONS),
        64
    );
}

/// Every `Sizes<E>` is a `Bounds<E>` through the blanket implementation: row
/// `i` of the bounds is `Some` of row `i` of `RESERVATIONS`, for every
/// member, and an interface with no member has no row.
#[test]
fn a_sized_interface_has_every_bound() {
    fn bounds<I: Bounds<E>, E: Encoding>() -> Vec<Option<u64>> {
        (0..I::MEMBERS.len()).map(I::reservation_bound).collect()
    }
    assert_eq!(
        bounds::<Cabin, FlatBuffers>(),
        <Cabin as Sizes<FlatBuffers>>::RESERVATIONS
            .iter()
            .map(|row| Some(*row))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        bounds::<Cabin, FlatBuffers>(),
        vec![Some(8), Some(32), Some(24)]
    );
    assert_eq!(bounds::<Huge, FlatBuffers>(), vec![Some(u64::MAX), Some(1)]);
    assert_eq!(bounds::<Empty, FlatBuffers>(), vec![]);
}
