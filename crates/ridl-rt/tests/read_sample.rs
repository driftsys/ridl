//! Roadmap story E11.0's done-when: a hand-written program links `ridl-rt`
//! and reads a sample with its provenance.
//!
//! The program is `examples/read_sample.rs`, compiled here as a module so the
//! example and this test cannot differ.

#![forbid(unsafe_code)]

#[allow(dead_code)]
#[path = "../examples/read_sample.rs"]
mod read_sample;

use read_sample::{read_speed, walk, Drivetrain, Memory, Speed, SpeedSignal};
use ridl_rt::contract::{Interaction, Interface, InterfaceNo, Ordinal, Signal};
use ridl_rt::error::Contract;
use ridl_rt::payload::{Rule, Violation};
use ridl_rt::port::{SignalWriter, WriteError};
use ridl_rt::sample::{Cause, Detection, Duration, Envelope, Freshness, Provenance, Timestamp};

#[test]
fn a_hand_written_program_reads_a_signal_with_its_provenance() {
    let [at_init, live, stale, declared, detected, corrupt] = walk();

    // Before the first publication: the init value, `seq` 0, stamped when the
    // channel was created.
    assert_eq!(at_init.value, Speed(30));
    assert_eq!(at_init.provenance, Provenance::Init);
    assert_eq!(at_init.freshness, Freshness::Fresh);
    assert_eq!(
        at_init.envelope,
        Envelope {
            stamp: Timestamp(1_000_000),
            seq: 0
        }
    );
    assert!(!at_init.usable());

    // The first publication.
    assert_eq!(live.value, Speed(88));
    assert_eq!(live.provenance, Provenance::Live);
    assert_eq!(live.freshness, Freshness::Fresh);
    assert_eq!(
        live.envelope,
        Envelope {
            stamp: Timestamp(1_010_000),
            seq: 1
        }
    );
    assert!(live.usable());

    // 600 ms without a publication, against a 500 ms bound.
    assert_eq!(stale.value, Speed(88));
    assert_eq!(stale.provenance, Provenance::Live);
    assert_eq!(
        stale.freshness,
        Freshness::Stale {
            by: Duration(100_000)
        }
    );
    assert_eq!(
        stale.envelope,
        Envelope {
            stamp: Timestamp(1_010_000),
            seq: 1
        }
    );
    assert!(!stale.usable());

    // The provider declares the invalid state: the last good value stays.
    assert_eq!(declared.value, Speed(88));
    assert_eq!(declared.provenance, Provenance::Invalid(Cause::Declared));
    assert_eq!(declared.freshness, Freshness::Fresh);
    assert_eq!(
        declared.envelope,
        Envelope {
            stamp: Timestamp(1_610_000),
            seq: 2
        }
    );
    assert!(!declared.usable());

    // A published value outside the declared range: the accessor detects it
    // and substitutes the init value.
    assert_eq!(detected.value, Speed(30));
    assert_eq!(
        detected.provenance,
        Provenance::Invalid(Cause::Detected(Detection::InvalidValue(Violation {
            type_name: "Speed",
            rule: Rule::Range,
        })))
    );
    assert_eq!(detected.freshness, Freshness::Fresh);
    assert_eq!(
        detected.envelope,
        Envelope {
            stamp: Timestamp(1_610_000),
            seq: 3
        }
    );
    assert!(!detected.usable());

    // One published byte where the encoding needs two: the accessor cannot
    // decode it, reports the detection and substitutes the init value.
    assert_eq!(corrupt.value, Speed(30));
    assert_eq!(
        corrupt.provenance,
        Provenance::Invalid(Cause::Detected(Detection::Corrupt))
    );
    assert_eq!(corrupt.freshness, Freshness::Fresh);
    assert_eq!(
        corrupt.envelope,
        Envelope {
            stamp: Timestamp(1_610_000),
            seq: 4
        }
    );
    assert!(!corrupt.usable());
}

/// The `Init`-with-no-bytes arm the example's own doc comment says `walk`
/// never reaches: a `Memory` seeded with an empty init buffer reports `Init`
/// at length zero, exercising the arm this file's other tests do not.
#[test]
fn a_signal_seeded_with_no_init_bytes_reads_as_init_with_no_payload() {
    let runtime = Memory::new(Timestamp(0), &[]);
    let sample = read_speed(&runtime).expect("speed is in the catalog");
    assert_eq!(sample.value, SpeedSignal::init());
    assert_eq!(sample.provenance, Provenance::Init);
}

#[test]
fn set_invalidate_and_touch_on_an_unknown_ordinal_return_the_contract_error() {
    let mut runtime = Memory::new(Timestamp(0), &[0, 0]);
    let unknown = Ordinal(99);

    assert_eq!(
        runtime.set(Drivetrain::NUMBER, unknown, &[1, 0]),
        Err(WriteError::Contract(Contract::UnknownInteraction))
    );
    assert_eq!(
        runtime.invalidate(Drivetrain::NUMBER, unknown),
        Err(WriteError::Contract(Contract::UnknownInteraction))
    );
    assert_eq!(
        runtime.touch(Drivetrain::NUMBER, unknown),
        Err(WriteError::Contract(Contract::UnknownInteraction))
    );
}

#[test]
fn touch_on_the_known_signal_reaffirms_without_changing_the_value() {
    let mut runtime = Memory::new(Timestamp(0), &30u16.to_le_bytes());
    let before = read_speed(&runtime).expect("speed is in the catalog");

    assert_eq!(
        runtime.touch(Drivetrain::NUMBER, SpeedSignal::MEMBER.ordinal),
        Ok(())
    );
    runtime.commit();
    let after = read_speed(&runtime).expect("speed is in the catalog");

    assert_eq!(after.value, before.value);
    assert_eq!(after.provenance, before.provenance);
    assert_eq!(after.envelope.seq, before.envelope.seq + 1);
}

#[test]
fn set_then_touch_then_commit_publishes_the_set_value() {
    let mut runtime = Memory::new(Timestamp(0), &30u16.to_le_bytes());
    let ord = SpeedSignal::MEMBER.ordinal;

    assert_eq!(
        runtime.set(Drivetrain::NUMBER, ord, &88u16.to_le_bytes()),
        Ok(())
    );
    assert_eq!(runtime.touch(Drivetrain::NUMBER, ord), Ok(()));
    runtime.commit();

    let after = read_speed(&runtime).expect("speed is in the catalog");
    assert_eq!(after.value, Speed(88));
    assert_eq!(after.provenance, Provenance::Live);
}

#[test]
fn a_rejected_call_on_an_unknown_ordinal_stages_nothing() {
    let mut runtime = Memory::new(Timestamp(0), &30u16.to_le_bytes());
    let unknown = Ordinal(99);
    let before = read_speed(&runtime).expect("speed is in the catalog");

    assert_eq!(
        runtime.set(Drivetrain::NUMBER, unknown, &[1, 0]),
        Err(WriteError::Contract(Contract::UnknownInteraction))
    );
    runtime.commit();
    let after_set = read_speed(&runtime).expect("speed is in the catalog");
    assert_eq!(after_set.provenance, before.provenance);
    assert_eq!(after_set.envelope.seq, before.envelope.seq);

    assert_eq!(
        runtime.invalidate(Drivetrain::NUMBER, unknown),
        Err(WriteError::Contract(Contract::UnknownInteraction))
    );
    runtime.commit();
    let after_invalidate = read_speed(&runtime).expect("speed is in the catalog");
    assert_eq!(after_invalidate.provenance, before.provenance);
    assert_eq!(after_invalidate.envelope.seq, before.envelope.seq);

    assert_eq!(
        runtime.touch(Drivetrain::NUMBER, unknown),
        Err(WriteError::Contract(Contract::UnknownInteraction))
    );
    runtime.commit();
    let after_touch = read_speed(&runtime).expect("speed is in the catalog");
    assert_eq!(after_touch.provenance, before.provenance);
    assert_eq!(after_touch.envelope.seq, before.envelope.seq);
}

#[test]
fn set_invalidate_and_touch_on_an_unknown_interface_return_the_contract_error() {
    let mut runtime = Memory::new(Timestamp(0), &30u16.to_le_bytes());
    let other_iface = InterfaceNo(2);
    let ord = SpeedSignal::MEMBER.ordinal;

    assert_eq!(
        runtime.set(other_iface, ord, &[1, 0]),
        Err(WriteError::Contract(Contract::UnknownInteraction))
    );
    assert_eq!(
        runtime.invalidate(other_iface, ord),
        Err(WriteError::Contract(Contract::UnknownInteraction))
    );
    assert_eq!(
        runtime.touch(other_iface, ord),
        Err(WriteError::Contract(Contract::UnknownInteraction))
    );
}
