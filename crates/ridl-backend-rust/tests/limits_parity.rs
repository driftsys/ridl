//! The default ceilings `ridl-rt` exports are the checker's (ADR-0030
//! decision 16). `ridl_core::limits` is the one source of a limit for the
//! checker and the manifest reader; `ridl_rt::contract` repeats the three
//! values for a transport, which resolves a member with no limit in its
//! descriptor row to the default ceiling of its kind. The two crates do not
//! depend on each other, so this crate, which depends on both, holds the
//! test that keeps them equal.

use ridl_core::limits as core;
use ridl_core::manifest::LimitKind;
use ridl_rt::contract::{self as rt, Kind};

#[test]
fn each_ceiling_of_ridl_rt_equals_the_checkers() {
    assert_eq!(
        rt::SIGNAL_EVENT_CEILING,
        core::SIGNAL_EVENT_CEILING as usize
    );
    assert_eq!(rt::CALL_CEILING, core::CALL_CEILING as usize);
    assert_eq!(rt::FIXED_CEILING, core::FIXED_CEILING as usize);
}

#[test]
fn each_kinds_default_ceiling_equals_the_checkers() {
    let kinds = [
        (Kind::Signal, LimitKind::Signal),
        (Kind::Event, LimitKind::Event),
        (Kind::Command, LimitKind::Command),
        (Kind::Query, LimitKind::Query),
        (Kind::Fixed, LimitKind::Fixed),
    ];
    for (rt_kind, core_kind) in kinds {
        assert_eq!(
            rt::default_ceiling(rt_kind),
            core::default_ceiling(core_kind) as usize,
            "{rt_kind:?}"
        );
    }
}
