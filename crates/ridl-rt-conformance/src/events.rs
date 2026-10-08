//! Events: subscription, delivery, a short buffer, and the sender's sequence
//! number (`EventSink` and `EventSource`).

use ridl_rt::port::{EventSink, EventSource, ReadError};

use crate::{runtime, Factory, IFACE, ORD, OTHER, TRACE_A, TRACE_B, TRACE_ZERO};

/// A raised occurrence reaches a subscribed source, whole.
pub fn an_event_raise_and_receive_round_trips<F: Factory>() {
    let mut rt = runtime::<F>();
    rt.subscribe(IFACE, &[ORD]).expect("subscribe");
    rt.raise(IFACE, ORD, &[5, 6], None).expect("raise");

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
    rt.raise(IFACE, ORD, &[1], None).expect("raise");
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
    rt.raise(IFACE, ORD, &[1], None).expect("raise");
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

    rt.raise(IFACE, ORD, &[8], None).expect("raise");

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
    rt.raise(IFACE, ORD, &[1, 2, 3], None).expect("raise");

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

    rt.raise(IFACE, ORD, &[1], None).expect("raise");
    rt.raise(IFACE, OTHER, &[2], None).expect("raise");
    rt.raise(IFACE, ORD, &[3], None).expect("raise");

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

/// The trace context an event is raised with arrives on the occurrence of
/// every subscribed source.
pub fn a_raised_events_context_arrives_on_every_subscribers_occurrence<F: Factory>() {
    let mut rt = runtime::<F>();
    let mut second = F::source(&rt);
    rt.subscribe(IFACE, &[ORD]).expect("subscribe");
    second.subscribe(IFACE, &[ORD]).expect("subscribe");

    rt.raise(IFACE, ORD, &[8], Some(TRACE_A)).expect("raise");

    let mut out = [0u8; 8];
    let first = rt.next(&mut out).expect("next").expect("waiting");
    assert_eq!(first.trace, Some(TRACE_A));
    let other = second.next(&mut out).expect("next").expect("waiting");
    assert_eq!(other.trace, Some(TRACE_A));
}

/// Two occurrences raised with different trace contexts each arrive with
/// their own: a source does not keep the first context for later ones. The
/// payloads are not empty.
pub fn two_occurrences_each_keep_their_own_context<F: Factory>() {
    let mut rt = runtime::<F>();
    rt.subscribe(IFACE, &[ORD]).expect("subscribe");

    rt.raise(IFACE, ORD, &[1], Some(TRACE_A)).expect("raise");
    rt.raise(IFACE, ORD, &[2], Some(TRACE_B)).expect("raise");

    let mut out = [0u8; 8];
    let first = rt.next(&mut out).expect("next").expect("waiting");
    assert_eq!(first.trace, Some(TRACE_A));
    let second = rt.next(&mut out).expect("next").expect("waiting");
    assert_eq!(second.trace, Some(TRACE_B));
}

/// An event raised without a trace context arrives without one.
pub fn an_event_raised_without_a_context_arrives_without_one<F: Factory>() {
    let mut rt = runtime::<F>();
    rt.subscribe(IFACE, &[ORD]).expect("subscribe");

    rt.raise(IFACE, ORD, &[8], None).expect("raise");

    let mut out = [0u8; 8];
    let occurrence = rt.next(&mut out).expect("next").expect("waiting");
    assert_eq!(occurrence.trace, None);
}

/// `ReadError::Short` does not drop the trace context: the occurrence the
/// next call returns still carries it. The payload is not empty, because an
/// empty one fits every buffer.
pub fn a_short_buffer_keeps_the_occurrences_context<F: Factory>() {
    let mut rt = runtime::<F>();
    rt.subscribe(IFACE, &[ORD]).expect("subscribe");
    rt.raise(IFACE, ORD, &[1, 2, 3], Some(TRACE_A))
        .expect("raise");

    let mut none = [0u8; 0];
    assert_eq!(rt.next(&mut none), Err(ReadError::Short { needed: 3 }));

    let mut out = [0u8; 8];
    let occurrence = rt.next(&mut out).expect("next").expect("still waiting");
    assert_eq!(occurrence.trace, Some(TRACE_A));
}

/// An occurrence raised with `TRACE_ZERO` arrives with it unchanged.
pub fn an_all_zero_occurrence_context_is_carried_unchanged<F: Factory>() {
    let mut rt = runtime::<F>();
    rt.subscribe(IFACE, &[ORD]).expect("subscribe");
    rt.raise(IFACE, ORD, &[8], Some(TRACE_ZERO)).expect("raise");

    let mut out = [0u8; 8];
    let occurrence = rt.next(&mut out).expect("next").expect("waiting");
    assert_eq!(occurrence.trace, Some(TRACE_ZERO));
}
