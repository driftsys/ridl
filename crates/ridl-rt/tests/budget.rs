//! The in-flight budget from the descriptors: `Member::reservation` and
//! `table_budget`. No specification defines this budget; it is derived from
//! `PayloadInfo::max_size` and `Member::payloads`, and each test cites the
//! descriptor field it reads.

#![forbid(unsafe_code)]

use ridl_rt::contract::{
    table_budget, CatalogHash, CatalogRef, EncodedSizes, Interface, InterfaceNo, Kind, Member,
    Ordinal, PayloadInfo, Unsized,
};
use ridl_rt::encoding::{FlatBuffers, Proto3, ReprC};

const CABIN: CatalogRef = CatalogRef {
    name: "cabin",
    hash: CatalogHash([3; 32]),
};

const fn sizes(proto3: u32, flatbuffers: u32, repr_c: u32) -> EncodedSizes {
    EncodedSizes {
        proto3: Some(proto3),
        flatbuffers: Some(flatbuffers),
        repr_c: Some(repr_c),
    }
}

const TEMPERATURE: Member = Member {
    ordinal: Ordinal(1),
    kind: Kind::Signal,
    name: "temperature",
    timing: None,
    payloads: &[PayloadInfo {
        type_name: "Celsius",
        max_size: sizes(5, 16, 4),
    }],
};

const SET_TARGET: Member = Member {
    ordinal: Ordinal(2),
    kind: Kind::Command,
    name: "setTarget",
    timing: None,
    payloads: &[PayloadInfo {
        type_name: "Target",
        max_size: sizes(7, 24, 8),
    }],
};

const HISTORY: Member = Member {
    ordinal: Ordinal(3),
    kind: Kind::Query,
    name: "history",
    timing: None,
    payloads: &[
        PayloadInfo {
            type_name: "HistoryRequest",
            max_size: sizes(11, 32, 12),
        },
        PayloadInfo {
            type_name: "HistoryReply",
            max_size: sizes(300, 520, 256),
        },
    ],
};

/// `interface Climate`, every payload sized in every encoding.
struct Climate;

impl Interface for Climate {
    const CATALOG: &'static CatalogRef = &CABIN;
    const NUMBER: InterfaceNo = InterfaceNo(1);
    const PROVISIONAL: bool = false;
    const NAME: &'static str = "Climate";
    const MEMBERS: &'static [Member] = &[TEMPERATURE, SET_TARGET, HISTORY];
}

/// A query whose reply has no FlatBuffers size.
const UNSIZED_REPLY: Member = Member {
    ordinal: Ordinal(4),
    kind: Kind::Query,
    name: "diagnose",
    timing: None,
    payloads: &[
        PayloadInfo {
            type_name: "DiagnoseRequest",
            max_size: sizes(2, 8, 1),
        },
        PayloadInfo {
            type_name: "DiagnoseReply",
            max_size: EncodedSizes {
                proto3: Some(40),
                flatbuffers: None,
                repr_c: None,
            },
        },
    ],
};

/// `interface Diagnostics`, one of whose payloads has no FlatBuffers size.
struct Diagnostics;

impl Interface for Diagnostics {
    const CATALOG: &'static CatalogRef = &CABIN;
    const NUMBER: InterfaceNo = InterfaceNo(2);
    const PROVISIONAL: bool = false;
    const NAME: &'static str = "Diagnostics";
    const MEMBERS: &'static [Member] = &[TEMPERATURE, UNSIZED_REPLY, SET_TARGET];
}

/// `Member::payloads` (one entry for every kind but a query) and
/// `PayloadInfo::max_size`: a one-payload member reserves that payload's size.
#[test]
fn a_one_payload_member_reserves_its_payload_size() {
    assert_eq!(TEMPERATURE.reservation::<FlatBuffers>(), Ok(16));
    assert_eq!(SET_TARGET.reservation::<FlatBuffers>(), Ok(24));
}

/// `Member::payloads` (two entries for a query, the request then the reply):
/// a query reserves the sum of both sizes.
#[test]
fn a_query_reserves_its_request_and_its_reply() {
    assert_eq!(HISTORY.reservation::<FlatBuffers>(), Ok(32 + 520));
}

/// `EncodedSizes` (one field per core encoding): each encoding reads its own
/// field.
#[test]
fn each_encoding_reads_its_own_size() {
    assert_eq!(HISTORY.reservation::<Proto3>(), Ok(11 + 300));
    assert_eq!(HISTORY.reservation::<FlatBuffers>(), Ok(32 + 520));
    assert_eq!(HISTORY.reservation::<ReprC>(), Ok(12 + 256));
}

/// `EncodedSizes` (a field is `None` when the toolchain cannot size the
/// payload): a `None` size is reported with the member and the payload it
/// belongs to, never replaced by a guess.
#[test]
fn an_unsized_payload_is_reported_with_its_member() {
    let err: Unsized = UNSIZED_REPLY.reservation::<FlatBuffers>().unwrap_err();
    assert_eq!(err.ordinal, Ordinal(4));
    assert_eq!(err.member, "diagnose");
    assert_eq!(err.type_name, "DiagnoseReply");
    assert_eq!(UNSIZED_REPLY.reservation::<Proto3>(), Ok(2 + 40));
}

/// `Member::payloads` holds `PayloadInfo`s in order, and a member with no
/// payload row reserves nothing.
#[test]
fn a_member_with_no_payload_reserves_nothing() {
    let bare = Member {
        payloads: &[],
        ..TEMPERATURE
    };
    assert_eq!(bare.reservation::<ReprC>(), Ok(0));
}

/// `Interface::MEMBERS`: a table budget is the sum of every member's
/// reservation.
#[test]
fn a_table_budget_sums_every_member() {
    assert_eq!(
        table_budget::<FlatBuffers>(Climate::MEMBERS),
        Ok(16 + 24 + 32 + 520)
    );
    assert_eq!(
        table_budget::<Proto3>(Climate::MEMBERS),
        Ok(5 + 7 + 11 + 300)
    );
}

/// `Interface::MEMBERS` and `EncodedSizes`: a table with an unsized member
/// has no budget; the first unsized member in ordinal order is reported.
#[test]
fn a_table_budget_reports_the_first_unsized_member() {
    let err = table_budget::<FlatBuffers>(Diagnostics::MEMBERS).unwrap_err();
    assert_eq!(err.ordinal, Ordinal(4));
    assert_eq!(err.member, "diagnose");
    assert_eq!(err.type_name, "DiagnoseReply");
    assert_eq!(
        table_budget::<Proto3>(Diagnostics::MEMBERS),
        Ok(5 + (2 + 40) + 7)
    );
}

/// `Interface::MEMBERS` may be empty: an interface with no member has a
/// budget of zero.
#[test]
fn an_empty_table_has_a_budget_of_zero() {
    assert_eq!(table_budget::<ReprC>(&[]), Ok(0));
}

/// `PayloadInfo::max_size` holds `u32` sizes; their sum is a `u64`, so a
/// member of two largest-size payloads does not overflow.
#[test]
fn the_sum_is_wider_than_one_size() {
    const LARGEST: Member = Member {
        payloads: &[
            PayloadInfo {
                type_name: "A",
                max_size: sizes(u32::MAX, u32::MAX, u32::MAX),
            },
            PayloadInfo {
                type_name: "B",
                max_size: sizes(u32::MAX, u32::MAX, u32::MAX),
            },
        ],
        ..HISTORY
    };
    assert_eq!(LARGEST.reservation::<Proto3>(), Ok(2 * u64::from(u32::MAX)));
}
