//! Signals: staging, the invalid state, the envelope, and a short buffer
//! (`SignalWriter` and `SignalReader`).

use ridl_rt::port::{Clock, ReadError, SignalReader, SignalWriter};
use ridl_rt::sample::{Cause, Duration, Envelope, Provenance};

use crate::{Factory, IFACE, ORD, OTHER, later, runtime};

/// A committed value reads back, live.
pub fn a_signal_publish_and_read_round_trips<F: Factory>() {
    let mut rt = runtime::<F>();
    rt.set(IFACE, ORD, &[42]).expect("set staged");
    rt.commit();

    let mut out = [0u8; 8];
    let raw = rt.read(IFACE, ORD, &mut out).expect("read");
    assert_eq!(raw.provenance, Provenance::Live);
    assert_eq!(&out[..raw.len], &[42]);
}

/// Before its first publication a signal reads as `Init`, copies nothing, and
/// carries sequence number 0, stamped when the channel was created (ADR-0021
/// decision 5). The consumer's binding supplies the init value (ridl §4.4).
pub fn a_signal_with_no_publication_reads_as_init_and_copies_nothing<F: Factory>() {
    let rt = runtime::<F>();
    let start = rt.now();
    let mut out = [0u8; 8];
    let raw = rt.read(IFACE, ORD, &mut out).expect("read");
    assert_eq!(raw.provenance, Provenance::Init);
    assert_eq!(raw.len, 0);
    assert_eq!(
        raw.envelope,
        Envelope {
            stamp: start,
            seq: 0
        }
    );
}

/// Staging is private to the writer until `commit`.
pub fn a_staged_value_is_not_visible_until_commit<F: Factory>() {
    let mut rt = runtime::<F>();
    rt.set(IFACE, ORD, &[7]).expect("set staged");

    let mut out = [0u8; 8];
    assert_eq!(
        rt.read(IFACE, ORD, &mut out).expect("read").provenance,
        Provenance::Init,
        "staging is private to the writer until commit"
    );

    rt.commit();
    assert_eq!(
        rt.read(IFACE, ORD, &mut out).expect("read").provenance,
        Provenance::Live
    );
}

/// One commit stamps every signal it publishes with one time, read from the
/// runtime's clock.
pub fn one_commit_publishes_every_staged_signal_under_one_timestamp<F: Factory>() {
    let mut rt = runtime::<F>();
    let start = rt.now();
    F::advance(&mut rt, Duration(500));
    rt.set(IFACE, ORD, &[1]).expect("set");
    rt.set(IFACE, OTHER, &[2]).expect("set");
    rt.commit();

    let mut out = [0u8; 8];
    let first = rt.read(IFACE, ORD, &mut out).expect("read");
    let second = rt.read(IFACE, OTHER, &mut out).expect("read");
    assert_eq!(first.envelope.stamp, later(start, 500));
    assert_eq!(
        first.envelope.stamp, second.envelope.stamp,
        "one commit stamps every signal it publishes with one time"
    );
}

/// A signal's sequence number counts the publications of that channel, not
/// the commits of the writer.
pub fn a_channel_sequence_number_counts_that_channel_publications<F: Factory>() {
    let mut rt = runtime::<F>();
    for value in 1..=3u8 {
        rt.set(IFACE, ORD, &[value]).expect("set");
        rt.set(IFACE, OTHER, &[value]).expect("set");
        rt.commit();
    }

    let mut out = [0u8; 8];
    // The first publication is seq 1, because seq 0 is the envelope of a
    // channel with no publication (ADR-0021 decision 5).
    assert_eq!(rt.read(IFACE, ORD, &mut out).expect("read").envelope.seq, 3);
    assert_eq!(
        rt.read(IFACE, OTHER, &mut out).expect("read").envelope.seq,
        3
    );
}

/// `invalidate` publishes the invalid state with the declared cause and keeps
/// the last good value (ridl §4.5); a later `set` clears it.
pub fn invalidate_keeps_the_last_good_value_and_reports_the_declared_cause<F: Factory>() {
    let mut rt = runtime::<F>();
    rt.set(IFACE, ORD, &[9]).expect("set");
    rt.commit();
    rt.invalidate(IFACE, ORD).expect("invalidate staged");
    rt.commit();

    let mut out = [0u8; 8];
    let raw = rt.read(IFACE, ORD, &mut out).expect("read");
    assert_eq!(raw.provenance, Provenance::Invalid(Cause::Declared));
    assert_eq!(
        &out[..raw.len],
        &[9],
        "the invalid state keeps the last good value (ridl 4.5)"
    );

    rt.set(IFACE, ORD, &[10]).expect("set");
    rt.commit();
    let raw = rt.read(IFACE, ORD, &mut out).expect("read");
    assert_eq!(
        raw.provenance,
        Provenance::Live,
        "a set clears the invalid state"
    );
}

/// `touch` re-affirms the current value at the new time, as a publication of
/// the channel, without a new value.
pub fn touch_republishes_the_current_value_without_changing_it<F: Factory>() {
    let mut rt = runtime::<F>();
    let start = rt.now();
    rt.set(IFACE, ORD, &[4]).expect("set");
    rt.commit();
    F::advance(&mut rt, Duration(100));
    rt.touch(IFACE, ORD).expect("touch staged");
    rt.commit();

    let mut out = [0u8; 8];
    let raw = rt.read(IFACE, ORD, &mut out).expect("read");
    assert_eq!(&out[..raw.len], &[4], "touch carries no new value");
    assert_eq!(
        raw.envelope.stamp,
        later(start, 100),
        "touch re-affirms at the new time"
    );
    assert_eq!(raw.envelope.seq, 2, "touch is a publication of the channel");
}

/// `touch` re-affirms the current value. A `set` already staged is itself a
/// publication, so a touch after it adds nothing — and must not replace it,
/// which would discard the value this writer staged.
pub fn a_touch_does_not_discard_a_value_staged_before_it<F: Factory>() {
    let mut rt = runtime::<F>();
    rt.set(IFACE, ORD, &[7]).expect("set");
    rt.touch(IFACE, ORD).expect("touch");
    rt.commit();

    let mut out = [0u8; 8];
    let raw = rt.read(IFACE, ORD, &mut out).expect("read");
    assert_eq!(raw.provenance, Provenance::Live);
    assert_eq!(&out[..raw.len], &[7]);
}

/// The same rule for an `invalidate` staged before the touch.
pub fn a_touch_does_not_discard_an_invalidation_staged_before_it<F: Factory>() {
    let mut rt = runtime::<F>();
    rt.set(IFACE, ORD, &[1]).expect("set");
    rt.commit();
    rt.invalidate(IFACE, ORD).expect("invalidate");
    rt.touch(IFACE, ORD).expect("touch");
    rt.commit();

    let mut out = [0u8; 8];
    let raw = rt.read(IFACE, ORD, &mut out).expect("read");
    assert_eq!(raw.provenance, Provenance::Invalid(Cause::Declared));
    assert_eq!(&out[..raw.len], &[1]);
}

/// The other direction: a `set` is a newer decision about the channel than a
/// touch already staged, so it replaces it.
pub fn a_set_after_a_touch_replaces_it<F: Factory>() {
    let mut rt = runtime::<F>();
    rt.set(IFACE, ORD, &[1]).expect("set");
    rt.commit();
    rt.touch(IFACE, ORD).expect("touch");
    rt.set(IFACE, ORD, &[2]).expect("set");
    rt.commit();

    let mut out = [0u8; 8];
    let raw = rt.read(IFACE, ORD, &mut out).expect("read");
    assert_eq!(&out[..raw.len], &[2]);
}

/// A re-affirmation of nothing is nothing. Publishing here would put a
/// zero-length value on the channel as `Live`, which a consumer's binding
/// reads as a corrupt payload rather than as the init value it should see.
///
/// That such a commit also advances no generation is
/// [`a_commit_of_only_touches_on_unpublished_channels_changes_no_generation`](crate::scannable::a_commit_of_only_touches_on_unpublished_channels_changes_no_generation),
/// in the tests of the extension that has a generation.
pub fn touch_on_a_channel_with_no_publication_publishes_nothing<F: Factory>() {
    let mut rt = runtime::<F>();
    rt.touch(IFACE, ORD).expect("touch staged");
    rt.commit();

    let mut out = [0u8; 8];
    assert_eq!(
        rt.read(IFACE, ORD, &mut out).expect("read").provenance,
        Provenance::Init
    );
}

/// A read into a buffer shorter than the value reports the size it needs and
/// consumes nothing.
pub fn a_short_buffer_reports_what_the_read_needs_and_consumes_nothing<F: Factory>() {
    let mut rt = runtime::<F>();
    rt.set(IFACE, ORD, &[1, 2, 3, 4]).expect("set");
    rt.commit();

    let mut short = [0u8; 2];
    assert_eq!(
        rt.read(IFACE, ORD, &mut short),
        Err(ReadError::Short { needed: 4 })
    );

    let mut out = [0u8; 8];
    assert_eq!(rt.read(IFACE, ORD, &mut out).expect("read").len, 4);
}
