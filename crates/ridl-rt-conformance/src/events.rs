//! Events: subscription, delivery, a short buffer, and the sender's sequence
//! number (`EventSink` and `EventSource`).

use ridl_rt::port::{EventSink, EventSource, ReadError};

use crate::{Factory, IFACE, ORD, OTHER, runtime};

/// A raised occurrence reaches a subscribed source, whole.
pub fn an_event_raise_and_receive_round_trips<F: Factory>() {
    let mut rt = runtime::<F>();
    rt.subscribe(IFACE, &[ORD]).expect("subscribe");
    rt.raise(IFACE, ORD, &[5, 6]).expect("raise");

    let mut out = [0u8; 8];
    let occurrence = rt
        .next(&mut out)
        .expect("next")
        .expect("an occurrence is waiting");
    assert_eq!(occurrence.iface, IFACE);
    assert_eq!(occurrence.ord, ORD);
    assert_eq!(&out[..occurrence.len], &[5, 6]);
}

/// A late joiner receives nothing retroactive on an event.
pub fn an_occurrence_raised_before_the_subscription_is_not_delivered<F: Factory>() {
    let mut rt = runtime::<F>();
    rt.raise(IFACE, ORD, &[1]).expect("raise");
    rt.subscribe(IFACE, &[ORD]).expect("subscribe");

    let mut out = [0u8; 8];
    assert!(
        rt.next(&mut out).expect("next").is_none(),
        "a late joiner receives nothing retroactive on an event"
    );
}

/// `unsubscribe` also stops delivery of what is already queued.
pub fn unsubscribe_stops_delivery_of_what_is_already_queued<F: Factory>() {
    let mut rt = runtime::<F>();
    rt.subscribe(IFACE, &[ORD]).expect("subscribe");
    rt.raise(IFACE, ORD, &[1]).expect("raise");
    rt.unsubscribe(IFACE, &[ORD]);

    let mut out = [0u8; 8];
    assert!(rt.next(&mut out).expect("next").is_none());
}

/// Two subscribed sources each receive a copy of one occurrence, and
/// consuming one copy does not consume the other.
pub fn two_sources_each_receive_their_own_copy_of_one_occurrence<F: Factory>() {
    let mut rt = runtime::<F>();
    let mut second = F::source(&rt);
    rt.subscribe(IFACE, &[ORD]).expect("subscribe");
    second.subscribe(IFACE, &[ORD]).expect("subscribe");

    rt.raise(IFACE, ORD, &[8]).expect("raise");

    let mut out = [0u8; 8];
    assert_eq!(
        rt.next(&mut out).expect("next").expect("waiting").len,
        1,
        "the first source consumes its own copy"
    );
    assert_eq!(
        second.next(&mut out).expect("next").expect("waiting").len,
        1,
        "and does not consume the second source's"
    );
}

/// `ReadError::Short` does not consume the occurrence: the next call returns
/// the same one.
pub fn a_short_buffer_leaves_the_occurrence_for_the_next_call<F: Factory>() {
    let mut rt = runtime::<F>();
    rt.subscribe(IFACE, &[ORD]).expect("subscribe");
    rt.raise(IFACE, ORD, &[1, 2, 3]).expect("raise");

    let mut short = [0u8; 1];
    assert_eq!(rt.next(&mut short), Err(ReadError::Short { needed: 3 }));

    let mut out = [0u8; 8];
    let occurrence = rt.next(&mut out).expect("next").expect("still waiting");
    assert_eq!(&out[..occurrence.len], &[1, 2, 3]);
}

/// One sink raising on two of its events, with a consumer subscribed to one of
/// them. A counter per sink rather than per channel would number this
/// consumer's two occurrences 1 and 3, and `EventSource::next` states that a
/// gap in seq is a loss — so the consumer would read a loss that did not
/// happen.
pub fn a_sink_sequence_number_counts_one_channel_publications<F: Factory>() {
    let mut rt = runtime::<F>();
    rt.subscribe(IFACE, &[ORD]).expect("subscribe");

    rt.raise(IFACE, ORD, &[1]).expect("raise");
    rt.raise(IFACE, OTHER, &[2]).expect("raise");
    rt.raise(IFACE, ORD, &[3]).expect("raise");

    let mut out = [0u8; 8];
    let mut seqs = Vec::new();
    while let Some(occurrence) = rt.next(&mut out).expect("next") {
        seqs.push(occurrence.envelope.seq);
    }
    assert_eq!(
        seqs,
        vec![1, 2],
        "no gap: the other event has its own counter"
    );
}
