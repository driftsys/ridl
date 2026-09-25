//! `Clock`: the time envelopes are stamped from.

use ridl_rt::port::Clock;
use ridl_rt::sample::Duration;

use crate::{Factory, later, runtime};

/// The clock does not move while real time passes, and
/// [`Factory::advance`] moves it by exactly the amount given. A clock that
/// read wall-clock time could not hold the first half, and every test that
/// states a timestamp relies on both.
pub fn the_clock_is_hand_driven_not_wall_clock<F: Factory>() {
    let mut rt = runtime::<F>();
    let before = rt.now();
    std::thread::sleep(std::time::Duration::from_millis(5));
    assert_eq!(rt.now(), before, "the clock must not read wall-clock time");

    F::advance(&mut rt, Duration(1_000));
    assert_eq!(
        rt.now(),
        later(before, 1_000),
        "advance moves the hand-driven clock by exactly the given amount"
    );
}
