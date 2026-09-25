//! The `ScannableSignals` extension: the interface generation and `scan`.
//!
//! A runtime may omit the extension, so every test here requires it of
//! `F::Runtime`, and `suite!` writes these tests only when asked for
//! `scannable`.

use ridl_rt::contract::InterfaceNo;
use ridl_rt::port::{ScannableSignals, SignalWriter, Watermark};

use crate::{Factory, IFACE, ORD, OTHER, blank_change, runtime};

/// A commit advances the generation of each interface it publishes to once,
/// however many of its signals it carries, and no other interface's.
pub fn a_commit_advances_the_interface_generation_once<F: Factory>()
where
    F::Runtime: ScannableSignals,
{
    let mut rt = runtime::<F>();
    assert_eq!(rt.generation(IFACE), 0, "no publication yet");

    rt.set(IFACE, ORD, &[1]).expect("set");
    rt.set(IFACE, OTHER, &[2]).expect("set");
    rt.commit();
    assert_eq!(
        rt.generation(IFACE),
        1,
        "two signals in one commit advance the generation once"
    );

    rt.set(IFACE, ORD, &[3]).expect("set");
    rt.commit();
    assert_eq!(rt.generation(IFACE), 2);
    assert_eq!(
        rt.generation(InterfaceNo(2)),
        0,
        "another interface is untouched"
    );
}

/// A commit whose every staged change is a `touch` on a channel with no
/// publication publishes nothing, so it advances no generation either.
pub fn a_commit_of_only_touches_on_unpublished_channels_changes_no_generation<F: Factory>()
where
    F::Runtime: ScannableSignals,
{
    let mut rt = runtime::<F>();
    rt.touch(IFACE, ORD).expect("touch staged");
    rt.commit();
    assert_eq!(
        rt.generation(IFACE),
        0,
        "a commit whose every staged change was such a touch changes nothing"
    );
}

/// `scan` reports each signal changed since a mark, moves the mark to the
/// current generation, and reports nothing more for a moved mark.
pub fn scan_reports_the_changes_since_a_mark_and_moves_it_forward<F: Factory>()
where
    F::Runtime: ScannableSignals,
{
    let mut rt = runtime::<F>();
    rt.set(IFACE, ORD, &[1]).expect("set");
    rt.set(IFACE, OTHER, &[2]).expect("set");
    rt.commit();

    let mut marks = [Watermark {
        iface: IFACE,
        generation: 0,
        seq: 0,
    }];
    let mut out = [blank_change(); 4];
    let written = rt.scan(&mut marks, &mut out);
    assert_eq!(written, 2);
    assert_eq!(out[0].ord, ORD);
    assert_eq!(out[1].ord, OTHER);
    assert_eq!(
        marks[0].generation, 1,
        "the mark moved to the current generation"
    );
    assert_eq!(marks[0].seq, 1);

    assert_eq!(
        rt.scan(&mut marks, &mut out),
        0,
        "a second scan with the moved mark reports nothing"
    );

    rt.set(IFACE, OTHER, &[3]).expect("set");
    rt.commit();
    let written = rt.scan(&mut marks, &mut out);
    assert_eq!(written, 1, "only the signal that changed since the mark");
    assert_eq!(out[0].ord, OTHER);
    assert_eq!(out[0].seq, 2);
}

/// An interface's changes are written all together or not at all, and a mark
/// whose changes did not fit is not moved.
pub fn scan_writes_an_interface_changes_all_together_or_not_at_all<F: Factory>()
where
    F::Runtime: ScannableSignals,
{
    let mut rt = runtime::<F>();
    rt.set(IFACE, ORD, &[1]).expect("set");
    rt.set(IFACE, OTHER, &[2]).expect("set");
    rt.commit();

    let mut marks = [Watermark {
        iface: IFACE,
        generation: 0,
        seq: 0,
    }];
    let mut one = [blank_change(); 1];
    assert_eq!(
        rt.scan(&mut marks, &mut one),
        0,
        "two changes do not fit in one entry, so none is written"
    );
    assert_eq!(marks[0].generation, 0, "and the mark is not moved");

    // Growing the output is what makes progress, which is the loop
    // `ScannableSignals::scan` documents.
    let mut two = [blank_change(); 2];
    assert_eq!(rt.scan(&mut marks, &mut two), 2);
    assert_eq!(marks[0].generation, 1);
}
