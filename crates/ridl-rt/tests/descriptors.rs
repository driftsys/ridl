//! The interaction descriptors, written the way generated code writes them:
//! from another crate, as constants.

#![forbid(unsafe_code)]

use ridl_rt::contract::{
    CatalogHash, CatalogRef, Command, EncodedSizes, Event, Fixed, Interaction, Interface,
    InterfaceNo, Kind, Member, Ordinal, PayloadInfo, Query, Signal, Timing, TimingMode,
};
use ridl_rt::error::Contract;
use ridl_rt::sample::Duration;

const VEHICLE: CatalogRef = CatalogRef {
    name: "vehicle",
    hash: CatalogHash([7; 32]),
};

/// Test data, not sizes computed from an encoding.
const SIZES: EncodedSizes = EncodedSizes {
    proto3: Some(8),
    flatbuffers: Some(16),
    repr_c: Some(4),
};

/// `interface Drivetrain` with one member of each kind.
struct Drivetrain;

impl Interface for Drivetrain {
    const CATALOG: &'static CatalogRef = &VEHICLE;
    const NUMBER: InterfaceNo = InterfaceNo(1);
    const PROVISIONAL: bool = false;
    const NAME: &'static str = "Drivetrain";
    const MEMBERS: &'static [Member] = &[
        Member {
            ordinal: Ordinal(1),
            kind: Kind::Signal,
            name: "speed",
            timing: Some(Timing {
                mode: TimingMode::Range,
                min: Some(Duration(20_000)),
                max: Some(Duration(500_000)),
            }),
            payloads: &[PayloadInfo {
                type_name: "Speed",
                max_size: SIZES,
            }],
        },
        Member {
            ordinal: Ordinal(2),
            kind: Kind::Event,
            name: "shiftDone",
            timing: Some(Timing {
                mode: TimingMode::Range,
                min: Some(Duration(50_000)),
                max: Some(Duration(500_000)),
            }),
            payloads: &[PayloadInfo {
                type_name: "Gear",
                max_size: SIZES,
            }],
        },
        Member {
            ordinal: Ordinal(3),
            kind: Kind::Command,
            name: "setGear",
            timing: Some(Timing {
                mode: TimingMode::Range,
                min: None,
                max: Some(Duration(50_000)),
            }),
            payloads: &[PayloadInfo {
                type_name: "SetGearArgs",
                max_size: SIZES,
            }],
        },
        Member {
            ordinal: Ordinal(4),
            kind: Kind::Query,
            name: "averageSpeed",
            timing: None,
            payloads: &[
                PayloadInfo {
                    type_name: "AverageSpeedArgs",
                    max_size: SIZES,
                },
                PayloadInfo {
                    type_name: "Speed",
                    max_size: SIZES,
                },
            ],
        },
        Member {
            ordinal: Ordinal(5),
            kind: Kind::Fixed,
            name: "wheelCount",
            timing: None,
            payloads: &[PayloadInfo {
                type_name: "u8",
                max_size: SIZES,
            }],
        },
    ];
}

struct SpeedSignal;
impl Interaction for SpeedSignal {
    type Iface = Drivetrain;
    const MEMBER: &'static Member = &Drivetrain::MEMBERS[0];
}
impl Signal for SpeedSignal {
    type Payload = u16;
    fn init() -> u16 {
        0
    }
}

struct ShiftDoneEvent;
impl Interaction for ShiftDoneEvent {
    type Iface = Drivetrain;
    const MEMBER: &'static Member = &Drivetrain::MEMBERS[1];
}
impl Event for ShiftDoneEvent {
    type Payload = u8;
}

struct SetGearCommand;
impl Interaction for SetGearCommand {
    type Iface = Drivetrain;
    const MEMBER: &'static Member = &Drivetrain::MEMBERS[2];
}
impl Command for SetGearCommand {
    type Args = u8;
    fn require(args: &u8) -> Result<(), ()> {
        if *args <= 6 { Ok(()) } else { Err(()) }
    }
}

struct AverageSpeedQuery;
impl Interaction for AverageSpeedQuery {
    type Iface = Drivetrain;
    const MEMBER: &'static Member = &Drivetrain::MEMBERS[3];
}
impl Query for AverageSpeedQuery {
    type Args = u32;
    type Reply = u16;
    fn require(window: &u32) -> Result<(), ()> {
        if *window > 0 { Ok(()) } else { Err(()) }
    }
    fn ensure(_window: &u32, reply: &u16) -> Result<(), ()> {
        if *reply <= 300 { Ok(()) } else { Err(()) }
    }
}

struct WheelCountFixed;
impl Interaction for WheelCountFixed {
    type Iface = Drivetrain;
    const MEMBER: &'static Member = &Drivetrain::MEMBERS[4];
}
impl Fixed for WheelCountFixed {
    type Payload = u8;
}

fn member_of<I: Interaction>() -> Member {
    *I::MEMBER
}

#[test]
fn each_kind_of_descriptor_reaches_its_member_through_the_interaction_bound() {
    assert_eq!(member_of::<SpeedSignal>(), Drivetrain::MEMBERS[0]);
    assert_eq!(member_of::<ShiftDoneEvent>(), Drivetrain::MEMBERS[1]);
    assert_eq!(member_of::<SetGearCommand>(), Drivetrain::MEMBERS[2]);
    assert_eq!(member_of::<AverageSpeedQuery>(), Drivetrain::MEMBERS[3]);
    assert_eq!(member_of::<WheelCountFixed>(), Drivetrain::MEMBERS[4]);
}

#[test]
fn an_interaction_reaches_its_interface_and_catalog() {
    fn identity<I: Interaction>() -> (&'static CatalogRef, InterfaceNo, Ordinal) {
        (
            <I::Iface as Interface>::CATALOG,
            <I::Iface as Interface>::NUMBER,
            I::MEMBER.ordinal,
        )
    }
    assert_eq!(
        identity::<AverageSpeedQuery>(),
        (&VEHICLE, InterfaceNo(1), Ordinal(4))
    );
}

#[test]
fn a_member_ordinal_is_usable_as_a_match_pattern() {
    const SPEED: Ordinal = SpeedSignal::MEMBER.ordinal;
    const SET_GEAR: Ordinal = SetGearCommand::MEMBER.ordinal;
    let route = |ord: Ordinal| match ord {
        SPEED => "speed",
        SET_GEAR => "setGear",
        _ => "other",
    };
    assert_eq!(route(Ordinal(1)), "speed");
    assert_eq!(route(Ordinal(3)), "setGear");
    assert_eq!(route(Ordinal(2)), "other");
}

#[test]
fn contract_clauses_run_through_a_generic_bound() {
    fn admit<C: Command>(args: &C::Args) -> Result<(), ()> {
        C::require(args)
    }
    /// The mapping generated dispatch writes: a failed `require` is
    /// `PreconditionFailed`, a failed `ensure` is `ContractBroken`, and
    /// `require` is evaluated first.
    fn answer<Q: Query>(args: &Q::Args, reply: &Q::Reply) -> Result<(), Contract> {
        Q::require(args).map_err(|()| Contract::PreconditionFailed)?;
        Q::ensure(args, reply).map_err(|()| Contract::ContractBroken)
    }
    assert_eq!(admit::<SetGearCommand>(&3), Ok(()));
    assert_eq!(admit::<SetGearCommand>(&9), Err(()));
    assert_eq!(answer::<AverageSpeedQuery>(&10, &120), Ok(()));
    assert_eq!(
        answer::<AverageSpeedQuery>(&0, &120),
        Err(Contract::PreconditionFailed)
    );
    assert_eq!(
        answer::<AverageSpeedQuery>(&10, &400),
        Err(Contract::ContractBroken)
    );
    assert_eq!(
        answer::<AverageSpeedQuery>(&0, &400),
        Err(Contract::PreconditionFailed)
    );
    assert_eq!(SpeedSignal::init(), 0);
}
