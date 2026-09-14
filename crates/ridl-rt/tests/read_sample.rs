//! Roadmap story E11.0's done-when: a hand-written program links `ridl-rt`
//! and reads a sample with its provenance.
//!
//! The program is `examples/read_sample.rs`, compiled here as a module so the
//! example and this test cannot differ.

#![forbid(unsafe_code)]

#[allow(dead_code)]
#[path = "../examples/read_sample.rs"]
mod read_sample;

use read_sample::{Drivetrain, Memory, Speed, walk};
use ridl_rt::contract::{Interface, Ordinal};
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

#[test]
fn set_and_invalidate_on_an_unknown_ordinal_return_the_contract_error() {
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
}
